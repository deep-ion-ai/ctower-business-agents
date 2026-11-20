//! Sugestão da funcionalidade de cada issue do rastreador (#270), sem IA.
//!
//! Pontua cada par issue × funcionalidade pelo texto: nome, descrição, área,
//! atores, nomes e arquivos dos pontos de entrada. Os arquivos alterados pelos
//! PRs que fecham a issue (GitHub) entram como `arquivos` e valem mais que o
//! texto. A comparação é por radical (prefixo), sem acento e sem caixa, para
//! "parcelar" casar com "parcelamento".

use std::collections::{BTreeMap, BTreeSet};

use cba_kb::IdFuncionalidade;
use serde::{Deserialize, Serialize};

use crate::funcionalidades::{Funcionalidade, MapaFuncionalidades};

/// Tamanho do radical comparado.
const RADICAL: usize = 6;
/// Menor palavra considerada.
const MIN_PALAVRA: usize = 4;
/// Pontuação mínima para sugerir.
pub const LIMIAR: u32 = 4;
/// No máximo, quantas funcionalidades por issue.
pub const MAX_SUGESTOES: usize = 3;

const PESO_NOME: u32 = 3;
const PESO_PONTO: u32 = 3;
const PESO_AREA: u32 = 2;
const PESO_ATOR: u32 = 2;
const PESO_DESCRICAO: u32 = 1;
const PESO_ARQUIVO: u32 = 6;

const IGNORADAS: &[&str] = &[
    "para", "como", "quando", "esta", "este", "isso", "mais", "deve", "pode", "fazer", "sobre",
    "ainda", "tela", "erro", "issue", "problema", "sistema", "usuario", "dados",
];

/// O que da issue interessa à sugestão.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextoDaIssue {
    pub chave: String,
    pub titulo: String,
    pub descricao: String,
    /// Arquivos alterados pelos PRs que fecham a issue.
    #[serde(default)]
    pub arquivos: Vec<String>,
}

/// Funcionalidade sugerida para uma issue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sugestao {
    pub funcionalidade: IdFuncionalidade,
    pub nome: String,
    pub pontuacao: u32,
    /// Por que: palavras que casaram e arquivos citados.
    pub evidencias: Vec<String>,
}

fn dobrar(c: char) -> char {
    match c {
        'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'í' | 'ì' | 'î' | 'ï' => 'i',
        'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
        'ú' | 'ù' | 'û' | 'ü' => 'u',
        'ç' => 'c',
        c => c,
    }
}

/// Radicais das palavras do texto (minúsculas, sem acento, `camelCase` e
/// `snake_case` separados).
fn radicais(texto: &str) -> BTreeSet<String> {
    let mut separado = String::with_capacity(texto.len() + 8);
    let mut anterior_minuscula = false;
    for c in texto.chars() {
        if c.is_uppercase() && anterior_minuscula {
            separado.push(' ');
        }
        anterior_minuscula = c.is_lowercase();
        separado.extend(c.to_lowercase().map(dobrar));
    }
    separado
        .split(|c: char| !c.is_alphanumeric())
        .filter(|p| p.chars().count() >= MIN_PALAVRA && !IGNORADAS.contains(p))
        .map(|p| p.chars().take(RADICAL).collect())
        .collect()
}

fn caminho_igual(a: &str, b: &str) -> bool {
    let (a, b) = (
        a.trim().trim_start_matches("./"),
        b.trim().trim_start_matches("./"),
    );
    !a.is_empty()
        && !b.is_empty()
        && (a == b || a.ends_with(&format!("/{b}")) || b.ends_with(&format!("/{a}")))
}

/// Pontua a issue contra uma funcionalidade; devolve a pontuação e as evidências.
fn pontuar(
    issue: &BTreeSet<String>,
    texto_da_issue: &str,
    arquivos: &[String],
    f: &Funcionalidade,
) -> (u32, Vec<String>) {
    // Melhor peso de cada radical da funcionalidade.
    let mut pesos: BTreeMap<String, (u32, &'static str)> = BTreeMap::new();
    let mut somar = |texto: &str, peso: u32, de: &'static str| {
        for r in radicais(texto) {
            let atual = pesos.entry(r).or_insert((0, de));
            if peso > atual.0 {
                *atual = (peso, de);
            }
        }
    };
    somar(&f.nome, PESO_NOME, "nome");
    somar(&f.area_nome, PESO_AREA, "área");
    somar(&f.area, PESO_AREA, "área");
    for a in &f.atores {
        somar(a, PESO_ATOR, "ator");
    }
    somar(&f.descricao, PESO_DESCRICAO, "descrição");
    for p in &f.pontos {
        somar(&p.nome, PESO_PONTO, "ponto de entrada");
    }

    let mut pontos = 0;
    let mut evidencias = Vec::new();
    for (r, (peso, de)) in &pesos {
        if issue.contains(r) {
            pontos += peso;
            evidencias.push(format!("{r}… ({de})"));
        }
    }
    let citado = texto_da_issue.to_lowercase();
    let mut vistos = BTreeSet::new();
    for p in &f.pontos {
        let arq = p.evidencia.arquivo.as_str();
        let no_texto = {
            let nome = arq.rsplit('/').next().unwrap_or(arq).to_lowercase();
            nome.len() >= 5 && citado.contains(&nome)
        };
        let em_pr = arquivos.iter().any(|a| caminho_igual(a, arq));
        if (no_texto || em_pr) && vistos.insert(arq.to_string()) {
            pontos += PESO_ARQUIVO;
            evidencias.push(format!(
                "arquivo {arq} ({})",
                if em_pr { "alterado por PR" } else { "citado" }
            ));
        }
    }
    (pontos, evidencias)
}

/// Funcionalidades sugeridas para a issue, da mais à menos pontuada. Vazio =
/// "sem funcionalidade". Só entram as abertas, com pelo menos [`LIMIAR`] pontos e
/// metade da melhor pontuação.
pub fn sugerir(mapa: &MapaFuncionalidades, issue: &TextoDaIssue) -> Vec<Sugestao> {
    let texto = format!("{}\n{}", issue.titulo, issue.descricao);
    let radicais_da_issue = radicais(&texto);
    let mut todas: Vec<Sugestao> = mapa
        .abertas()
        .filter_map(|f| {
            let (pontuacao, evidencias) = pontuar(&radicais_da_issue, &texto, &issue.arquivos, f);
            (pontuacao >= LIMIAR).then(|| Sugestao {
                funcionalidade: f.id.clone(),
                nome: f.nome.clone(),
                pontuacao,
                evidencias,
            })
        })
        .collect();
    todas.sort_by(|a, b| {
        b.pontuacao.cmp(&a.pontuacao).then_with(|| {
            a.funcionalidade
                .to_string()
                .cmp(&b.funcionalidade.to_string())
        })
    });
    if let Some(melhor) = todas.first().map(|s| s.pontuacao) {
        todas.retain(|s| s.pontuacao * 2 >= melhor);
    }
    todas.truncate(MAX_SUGESTOES);
    todas
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analise::{EstadoEvidencia, Evidencia};
    use crate::funcionalidades::{PontoAtribuido, SituacaoFuncionalidade};
    use cba_kb::CaminhoRelativo;
    use cba_kb::frontmatter::Linhas;

    fn func(n: u32, nome: &str, descricao: &str, arquivo: &str, ponto: &str) -> Funcionalidade {
        Funcionalidade {
            id: IdFuncionalidade::novo("FAT", n).unwrap(),
            nome: nome.into(),
            area: "cobranca".into(),
            area_nome: "Cobrança".into(),
            descricao: descricao.into(),
            atores: vec!["Atendente".into()],
            pontos: vec![PontoAtribuido {
                chave: Some(format!("{arquivo}#{ponto}")),
                tipo: None,
                nome: ponto.into(),
                evidencia: Evidencia {
                    arquivo: CaminhoRelativo::novo(arquivo).unwrap(),
                    linhas: Linhas::novo(1, 1).unwrap(),
                    trecho: "X".into(),
                    hash: "h".into(),
                    commit: None,
                    estado: EstadoEvidencia::Valida,
                },
            }],
            dados_lidos: vec![],
            dados_gravados: vec![],
            relacionadas: vec![],
            situacao: SituacaoFuncionalidade::Aceita,
            juntada_em: None,
            editada: false,
            documento: None,
            explicacao: None,
            demandas: vec![],
            sessao: None,
            registrada_ms: 1,
            atualizada_ms: 1,
        }
    }

    fn mapa() -> MapaFuncionalidades {
        let mut m = MapaFuncionalidades::default();
        m.funcionalidades.push(func(
            1,
            "Parcelar fatura",
            "Divide o valor da fatura em parcelas.",
            "db/parcelamento.sql",
            "PKG_COBRANCA.PRC_PARCELAR",
        ));
        m.funcionalidades.push(func(
            2,
            "Cancelar nota",
            "Cancela a nota fiscal emitida.",
            "db/cancelamento.sql",
            "PKG_NOTA.PRC_CANCELAR",
        ));
        m
    }

    fn issue(titulo: &str, descricao: &str) -> TextoDaIssue {
        TextoDaIssue {
            chave: "#1".into(),
            titulo: titulo.into(),
            descricao: descricao.into(),
            arquivos: vec![],
        }
    }

    #[test]
    fn issue_do_parcelamento_vai_para_a_funcionalidade_de_parcelamento() {
        let s = sugerir(
            &mapa(),
            &issue(
                "Erro no parcelamento",
                "Ao parcelar em 12x o valor das parcelas fica errado",
            ),
        );
        assert_eq!(s.len(), 1, "{s:?}");
        assert_eq!(s[0].funcionalidade.to_string(), "F-FAT-001");
        assert!(s[0].pontuacao >= LIMIAR && !s[0].evidencias.is_empty());
    }

    #[test]
    fn issue_sem_relacao_fica_sem_funcionalidade() {
        let s = sugerir(
            &mapa(),
            &issue("Trocar a cor do botão", "O azul está muito claro"),
        );
        assert!(s.is_empty(), "{s:?}");
    }

    #[test]
    fn arquivo_do_pr_ou_citado_no_texto_conta() {
        let mut i = issue("Ajuste", "mexe em db/cancelamento.sql");
        let s = sugerir(&mapa(), &i);
        assert_eq!(s[0].funcionalidade.to_string(), "F-FAT-002");
        i.descricao = String::new();
        i.arquivos = vec!["src/db/parcelamento.sql".into()];
        let s = sugerir(&mapa(), &i);
        assert_eq!(s[0].funcionalidade.to_string(), "F-FAT-001");
        assert!(s[0].evidencias.iter().any(|e| e.contains("PR")));
    }

    #[test]
    fn mais_de_uma_funcionalidade_e_descartadas_ficam_de_fora() {
        let mut m = mapa();
        let s = sugerir(
            &m,
            &issue("Parcelar e cancelar", "parcelar fatura e cancelar nota"),
        );
        assert_eq!(s.len(), 2, "{s:?}");
        m.funcionalidades[0].situacao = SituacaoFuncionalidade::Descartada;
        let s = sugerir(
            &m,
            &issue("Parcelar e cancelar", "parcelar fatura e cancelar nota"),
        );
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].funcionalidade.to_string(), "F-FAT-002");
    }
}
