//! Documento Markdown com frontmatter YAML.
//!
//! Formato: a primeira linha é `---`, segue o YAML do frontmatter, uma linha `---`
//! fecha e o resto é o corpo em Markdown.
//!
//! Preservação ao gravar de novo um documento lido:
//!
//! - sem mudança no frontmatter, o cabeçalho é gravado byte a byte como foi lido
//!   (comentários, aspas, estilo e fim de linha incluídos);
//! - com mudança, o YAML é gerado de novo na ordem original dos campos, com os
//!   campos novos no fim e os campos desconhecidos mantidos (comentários do
//!   frontmatter se perdem nesse caso). Campos opcionais vazios (`null`, lista
//!   vazia) que não estavam no arquivo não são acrescentados;
//! - o corpo é sempre gravado como está em [`Documento::corpo`].

use std::fmt;

use crate::frontmatter::{ErroCampo, Frontmatter, TipoDocumento};
use crate::yaml::{Mapping, Value};

const DELIMITADOR: &str = "---";

/// Documento da base: frontmatter tipado e corpo em Markdown.
#[derive(Debug, Clone, PartialEq)]
pub struct Documento<F> {
    pub frontmatter: F,
    pub corpo: String,
    original: Option<Original>,
}

/// Como o frontmatter estava no arquivo lido.
#[derive(Debug, Clone, PartialEq)]
struct Original {
    /// Texto do início do arquivo até a linha `---` de fechamento, inclusive.
    cabecalho: String,
    mapa: Mapping,
}

/// Frontmatter inválido. A posição do arquivo é acrescentada por [`crate::Erro`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErroFrontmatter {
    /// Campo com problema (`status`, `demanda.origem`, `evidencias[0].linhas`), se
    /// o erro for de um campo.
    pub campo: Option<String>,
    /// Linha no arquivo (a partir de 1), quando conhecida.
    pub linha: Option<usize>,
    pub mensagem: String,
}

impl fmt::Display for ErroFrontmatter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(linha) = self.linha {
            write!(f, "linha {linha}: ")?;
        }
        if let Some(campo) = &self.campo {
            write!(f, "campo `{campo}`: ")?;
        }
        f.write_str(&self.mensagem)
    }
}

impl std::error::Error for ErroFrontmatter {}

impl ErroFrontmatter {
    fn geral(mensagem: impl Into<String>) -> Self {
        Self {
            campo: None,
            linha: None,
            mensagem: mensagem.into(),
        }
    }

    fn de_campo(yaml: &str, campo: String, mensagem: impl Into<String>) -> Self {
        let linha = linha_do_campo(yaml, &campo);
        Self {
            campo: Some(campo),
            linha,
            mensagem: mensagem.into(),
        }
    }
}

impl<F: Frontmatter> Documento<F> {
    /// Documento novo, ainda não gravado.
    pub fn novo(frontmatter: F, corpo: impl Into<String>) -> Self {
        Self {
            frontmatter,
            corpo: corpo.into(),
            original: None,
        }
    }

    /// Lê um documento a partir do texto do arquivo.
    pub fn analisar(texto: &str) -> Result<Self, ErroFrontmatter> {
        let partes = separar(texto)?;
        let valor: Value = serde_norway::from_str(partes.yaml).map_err(|e| ErroFrontmatter {
            campo: None,
            // +1 pela linha `---` de abertura.
            linha: e.location().map(|l| l.line() + 1),
            mensagem: format!("YAML inválido: {e}"),
        })?;
        let mapa = match valor {
            Value::Mapping(m) => m,
            Value::Null => Mapping::new(),
            _ => {
                return Err(ErroFrontmatter::geral(
                    "o frontmatter precisa ser um mapa de campos (`campo: valor`)",
                ));
            }
        };
        conferir_tipo::<F>(&mapa, partes.yaml)?;
        let frontmatter: F = serde_path_to_error::deserialize(Value::Mapping(mapa.clone()))
            .map_err(|e| {
                let caminho = e.path().to_string();
                let mensagem = e.inner().to_string();
                let campo = campo_do_erro(&caminho, &mensagem);
                ErroFrontmatter::de_campo(partes.yaml, campo, mensagem)
            })?;
        frontmatter
            .validar()
            .map_err(|ErroCampo { campo, mensagem }| {
                ErroFrontmatter::de_campo(partes.yaml, campo, mensagem)
            })?;
        Ok(Self {
            frontmatter,
            corpo: partes.corpo.to_owned(),
            original: Some(Original {
                cabecalho: partes.cabecalho.to_owned(),
                mapa,
            }),
        })
    }

    /// Texto do arquivo. Valida o frontmatter antes, para nunca gravar um documento
    /// que não seria lido de volta.
    pub fn para_texto(&self) -> Result<String, ErroFrontmatter> {
        self.frontmatter
            .validar()
            .map_err(|ErroCampo { campo, mensagem }| ErroFrontmatter {
                campo: Some(campo),
                linha: None,
                mensagem,
            })?;
        let novo = match serde_norway::to_value(&self.frontmatter) {
            Ok(Value::Mapping(m)) => m,
            Ok(_) => {
                return Err(ErroFrontmatter::geral(
                    "o frontmatter precisa ser um mapa de campos",
                ));
            }
            Err(e) => return Err(ErroFrontmatter::geral(e.to_string())),
        };
        let mapa = match &self.original {
            Some(original) => {
                let mesclado = mesclar(&original.mapa, novo, F::PRESERVA_DESCONHECIDOS);
                if mesclado == original.mapa {
                    return Ok(format!("{}{}", original.cabecalho, self.corpo));
                }
                mesclado
            }
            None => novo,
        };
        let yaml = if mapa.is_empty() {
            String::new()
        } else {
            serde_norway::to_string(&mapa).map_err(|e| ErroFrontmatter::geral(e.to_string()))?
        };
        Ok(format!(
            "{DELIMITADOR}\n{yaml}{DELIMITADOR}\n{}",
            self.corpo
        ))
    }
}

struct Partes<'a> {
    cabecalho: &'a str,
    yaml: &'a str,
    corpo: &'a str,
}

fn separar(texto: &str) -> Result<Partes<'_>, ErroFrontmatter> {
    let sem_bom = texto.strip_prefix('\u{feff}').unwrap_or(texto);
    let inicio_yaml = match linha_delimitadora(sem_bom) {
        Some(fim) => texto.len() - sem_bom.len() + fim,
        None => {
            return Err(ErroFrontmatter {
                campo: None,
                linha: Some(1),
                mensagem: "documento sem frontmatter: a primeira linha precisa ser `---`".into(),
            });
        }
    };
    let mut posicao = inicio_yaml;
    while posicao < texto.len() {
        if let Some(fim) = linha_delimitadora(&texto[posicao..]) {
            return Ok(Partes {
                cabecalho: &texto[..posicao + fim],
                yaml: &texto[inicio_yaml..posicao],
                corpo: &texto[posicao + fim..],
            });
        }
        posicao = match texto[posicao..].find('\n') {
            Some(i) => posicao + i + 1,
            None => texto.len(),
        };
    }
    Err(ErroFrontmatter {
        campo: None,
        linha: None,
        mensagem: "frontmatter sem a linha `---` de fechamento".into(),
    })
}

/// Se `texto` começa com uma linha `---`, devolve o tamanho dela com a quebra.
fn linha_delimitadora(texto: &str) -> Option<usize> {
    let resto = texto.strip_prefix(DELIMITADOR)?;
    let (linha, quebra) = match resto.find('\n') {
        Some(i) => (&resto[..i], 1),
        None => (resto, 0),
    };
    linha
        .trim_end_matches([' ', '\t', '\r'])
        .is_empty()
        .then_some(DELIMITADOR.len() + linha.len() + quebra)
}

fn conferir_tipo<F: Frontmatter>(mapa: &Mapping, yaml: &str) -> Result<(), ErroFrontmatter> {
    if F::TIPOS.is_empty() {
        return Ok(());
    }
    let esperados = F::TIPOS
        .iter()
        .map(|t| t.como_str())
        .collect::<Vec<_>>()
        .join(", ");
    let erro = |m: String| ErroFrontmatter::de_campo(yaml, "tipo".into(), m);
    let tipo = match mapa.get("tipo") {
        None => {
            return Err(erro(format!(
                "campo obrigatório ausente; esperado: {esperados}"
            )));
        }
        Some(Value::String(t)) => t,
        Some(_) => return Err(erro("precisa ser um texto".into())),
    };
    match TipoDocumento::de_str(tipo) {
        Some(t) if F::TIPOS.contains(&t) => Ok(()),
        Some(_) => Err(erro(format!(
            "documento do tipo `{tipo}`; esperado: {esperados}"
        ))),
        None => Err(erro(format!("tipo desconhecido `{tipo}`"))),
    }
}

/// Campo do erro a partir do caminho do `serde_path_to_error`. Num campo ausente,
/// o caminho aponta para o mapa que o contém e o nome vem na mensagem do serde.
fn campo_do_erro(caminho: &str, mensagem: &str) -> String {
    let base = if caminho == "." { "" } else { caminho };
    let ausente = mensagem
        .strip_prefix("missing field `")
        .and_then(|r| r.split('`').next());
    match ausente {
        Some(nome) if base.is_empty() => nome.to_owned(),
        Some(nome) => format!("{base}.{nome}"),
        None if base.is_empty() => "(raiz)".to_owned(),
        None => base.to_owned(),
    }
}

/// Linha (no arquivo) onde começa o campo de primeiro nível de `campo`, se achar.
fn linha_do_campo(yaml: &str, campo: &str) -> Option<usize> {
    let raiz = campo.split(['.', '[']).next()?;
    yaml.lines()
        .position(|l| {
            l.strip_prefix(raiz)
                .is_some_and(|r| r.trim_start().starts_with(':'))
        })
        // +1 por começar em 1 e +1 pela linha `---` de abertura.
        .map(|i| i + 2)
}

/// Mapa novo na ordem do original: campos existentes no lugar de antes, campos
/// novos no fim. Mapas aninhados são mesclados do mesmo jeito.
fn mesclar(original: &Mapping, mut novo: Mapping, preserva_desconhecidos: bool) -> Mapping {
    let mut saida = Mapping::with_capacity(original.len().max(novo.len()));
    for (chave, antes) in original {
        match novo.shift_remove(chave) {
            Some(depois) => {
                let valor = match (antes, depois) {
                    (Value::Mapping(a), Value::Mapping(d)) => {
                        Value::Mapping(mesclar(a, d, preserva_desconhecidos))
                    }
                    (_, d) => d,
                };
                saida.insert(chave.clone(), valor);
            }
            None if preserva_desconhecidos => {
                saida.insert(chave.clone(), antes.clone());
            }
            None => {}
        }
    }
    for (chave, valor) in novo {
        // Campo opcional vazio que não estava no arquivo continua fora dele, para
        // regravar um documento sem mudanças não acrescentar `campo: null`.
        if !vazio(&valor) {
            saida.insert(chave, valor);
        }
    }
    saida
}

fn vazio(valor: &Value) -> bool {
    match valor {
        Value::Null => true,
        Value::Sequence(s) => s.is_empty(),
        Value::Mapping(m) => m.is_empty(),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontmatter::{
        Abrangencia, FrontmatterAnalise, FrontmatterLivre, FrontmatterRegra, StatusAnalise,
        StatusRegra,
    };

    const ANALISE: &str = "\
---
tipo: analise-impacto
id: A-0193
titulo: Parcelamento da fatura para pessoa jurídica
sistema: faturamento-exemplo
demanda: {origem: texto, chave: null}
status: rascunho            # rascunho | em-revisao | aprovada
abrangencia: media
base: {branch: develop, commit: 1a2b3c4d}
criado: 2026-11-11
analista: Ana Exemplo
---

# Parcelamento da fatura

Corpo com acentuação: ção, ã, é.
";

    const REGRA: &str = "\
---
tipo: regra
id: RN-FAT-007
titulo: Juros de atraso contam a partir do primeiro dia
area: cobranca
status: confirmada          # confirmada | a-revalidar | obsoleta
evidencias:
  - arquivo: db/multa.sql
    linhas: 10-10
    hash: 9f3c2a
    commit: 1a2b3c4d
origem: analises/A-0193.md
---
Enunciado.
";

    #[test]
    fn le_os_exemplos_da_spec() {
        let a = Documento::<FrontmatterAnalise>::analisar(ANALISE).unwrap();
        assert_eq!(a.frontmatter.id.to_string(), "A-0193");
        assert_eq!(a.frontmatter.tipo, TipoDocumento::AnaliseImpacto);
        assert_eq!(a.frontmatter.abrangencia, Some(Abrangencia::Media));
        assert_eq!(a.frontmatter.demanda.chave, None);
        assert_eq!(
            a.frontmatter.base.as_ref().unwrap().commit.as_deref(),
            Some("1a2b3c4d")
        );
        assert!(a.corpo.starts_with("\n# Parcelamento"));

        let r = Documento::<FrontmatterRegra>::analisar(REGRA).unwrap();
        assert_eq!(r.frontmatter.status, StatusRegra::Confirmada);
        assert_eq!(r.frontmatter.evidencias[0].linhas.to_string(), "10-10");
        assert_eq!(
            r.frontmatter.origem.as_ref().unwrap().as_str(),
            "analises/A-0193.md"
        );
        assert_eq!(r.corpo, "Enunciado.\n");
    }

    #[test]
    fn regravar_sem_mudanca_preserva_o_texto() {
        for texto in [ANALISE, REGRA] {
            let livre = Documento::<FrontmatterLivre>::analisar(texto).unwrap();
            assert_eq!(livre.para_texto().unwrap(), texto);
        }
        let a = Documento::<FrontmatterAnalise>::analisar(ANALISE).unwrap();
        assert_eq!(a.para_texto().unwrap(), ANALISE);
        let r = Documento::<FrontmatterRegra>::analisar(REGRA).unwrap();
        assert_eq!(r.para_texto().unwrap(), REGRA);
        let crlf = ANALISE.replace('\n', "\r\n");
        let a = Documento::<FrontmatterAnalise>::analisar(&crlf).unwrap();
        assert_eq!(a.para_texto().unwrap(), crlf);
        let bom = format!("\u{feff}{REGRA}");
        let r = Documento::<FrontmatterRegra>::analisar(&bom).unwrap();
        assert_eq!(r.para_texto().unwrap(), bom);
    }

    fn chaves(texto: &str) -> Vec<String> {
        let doc = Documento::<FrontmatterLivre>::analisar(texto).unwrap();
        doc.frontmatter
            .0
            .keys()
            .map(|k| k.as_str().unwrap().to_owned())
            .collect()
    }

    #[test]
    fn alterar_preserva_a_ordem_e_os_campos_desconhecidos() {
        // Ordem diferente da estrutura e um campo que ela não conhece.
        let texto = "\
---
titulo: Multa
id: A-0001
extra_da_organizacao: [x, y]
tipo: pergunta
sistema: faturamento-exemplo
status: rascunho
demanda:
  chave: null
  origem: texto
  nota: campo aninhado desconhecido
criado: 2026-11-11
---
corpo
";
        let mut doc = Documento::<FrontmatterAnalise>::analisar(texto).unwrap();
        doc.frontmatter.status = StatusAnalise::EmRevisao;
        doc.frontmatter.demanda.chave = Some("FAT-12".into());
        doc.frontmatter.analista = Some("Ana Exemplo".into());
        let novo = doc.para_texto().unwrap();
        assert_eq!(
            chaves(&novo),
            [
                "titulo",
                "id",
                "extra_da_organizacao",
                "tipo",
                "sistema",
                "status",
                "demanda",
                "criado",
                // Campo novo vai para o fim; opcionais vazios não entram.
                "analista",
            ]
        );
        assert!(novo.contains("status: em-revisao"), "{novo}");
        assert!(novo.ends_with("---\ncorpo\n"), "{novo}");
        let demanda = Documento::<FrontmatterLivre>::analisar(&novo).unwrap();
        let demanda = demanda.frontmatter.0.get("demanda").unwrap();
        let demanda: Vec<_> = demanda
            .as_mapping()
            .unwrap()
            .keys()
            .map(|k| k.as_str().unwrap())
            .collect();
        assert_eq!(demanda, ["chave", "origem", "nota"]);
        let relido = Documento::<FrontmatterAnalise>::analisar(&novo).unwrap();
        assert_eq!(relido.frontmatter, doc.frontmatter);
        // Uma segunda gravação sem mudança mantém o texto.
        assert_eq!(relido.para_texto().unwrap(), novo);
    }

    #[test]
    fn livre_permite_remover_campo() {
        let mut doc = Documento::<FrontmatterLivre>::analisar(REGRA).unwrap();
        doc.frontmatter.0.shift_remove("origem");
        let novo = doc.para_texto().unwrap();
        assert!(!novo.contains("origem"), "{novo}");
    }

    #[test]
    fn documento_novo_na_ordem_da_estrutura() {
        let a = Documento::<FrontmatterAnalise>::analisar(ANALISE).unwrap();
        let novo = Documento::novo(a.frontmatter.clone(), "corpo\n");
        let texto = novo.para_texto().unwrap();
        assert!(
            texto.starts_with("---\ntipo: analise-impacto\nid: A-0193\n"),
            "{texto}"
        );
        assert!(texto.contains("criado: 2026-11-11\n"), "{texto}");
        assert!(texto.contains("titulo: Parcelamento da fatura para pessoa jurídica\n"));
        let relido = Documento::<FrontmatterAnalise>::analisar(&texto).unwrap();
        assert_eq!(relido.frontmatter, a.frontmatter);
        assert_eq!(relido.corpo, "corpo\n");
    }

    #[test]
    fn texto_que_parece_numero_volta_como_texto() {
        let r = Documento::<FrontmatterRegra>::analisar(REGRA).unwrap();
        let mut f = r.frontmatter;
        f.evidencias[0].commit = Some("12345678".into());
        f.evidencias[0].hash = "1e10".into();
        f.titulo = "true".into();
        let texto = Documento::novo(f.clone(), "").para_texto().unwrap();
        let relido = Documento::<FrontmatterRegra>::analisar(&texto).unwrap();
        assert_eq!(relido.frontmatter, f, "{texto}");
    }

    #[test]
    fn commit_sem_aspas_so_com_digitos() {
        let texto = REGRA.replace("commit: 1a2b3c4d", "commit: 12345678");
        let r = Documento::<FrontmatterRegra>::analisar(&texto).unwrap();
        assert_eq!(
            r.frontmatter.evidencias[0].commit.as_deref(),
            Some("12345678")
        );
    }

    fn erro<F: Frontmatter + fmt::Debug>(texto: &str) -> ErroFrontmatter {
        Documento::<F>::analisar(texto).unwrap_err()
    }

    #[test]
    fn erros_apontam_o_campo() {
        let e = erro::<FrontmatterAnalise>(&ANALISE.replace("status: rascunho", "status: pronta"));
        assert_eq!(e.campo.as_deref(), Some("status"));
        assert_eq!(e.linha, Some(7));
        assert!(e.to_string().contains("campo `status`"), "{e}");

        let e = erro::<FrontmatterAnalise>(
            &ANALISE.replace("titulo: Parcelamento da fatura para pessoa jurídica\n", ""),
        );
        assert_eq!(e.campo.as_deref(), Some("titulo"));

        let e = erro::<FrontmatterAnalise>(
            &ANALISE.replace("{origem: texto, chave: null}", "{chave: null}"),
        );
        assert_eq!(e.campo.as_deref(), Some("demanda.origem"));

        let e = erro::<FrontmatterAnalise>(&ANALISE.replace("2026-11-11", "11/11/2026"));
        assert_eq!(e.campo.as_deref(), Some("criado"));

        let e = erro::<FrontmatterRegra>(&REGRA.replace("linhas: 10-10", "linhas: 10-2"));
        assert_eq!(e.campo.as_deref(), Some("evidencias[0].linhas"));

        let e = erro::<FrontmatterRegra>(&REGRA.replace("db/multa.sql", "/etc/multa.sql"));
        assert_eq!(e.campo.as_deref(), Some("evidencias[0].arquivo"));
        assert!(e.mensagem.contains("absoluto"), "{e}");

        let e = erro::<FrontmatterRegra>(&REGRA.replace("RN-FAT-007", "A-0007"));
        assert_eq!(e.campo.as_deref(), Some("id"));
        assert_eq!(e.linha, Some(3));
    }

    #[test]
    fn erros_de_tipo() {
        let e = erro::<FrontmatterRegra>(ANALISE);
        assert_eq!(e.campo.as_deref(), Some("tipo"));
        assert!(e.mensagem.contains("analise-impacto"), "{e}");
        let e = erro::<FrontmatterRegra>(&REGRA.replace("tipo: regra", "tipo: receita"));
        assert!(e.mensagem.contains("desconhecido"), "{e}");
        let e = erro::<FrontmatterRegra>(&REGRA.replace("tipo: regra\n", ""));
        assert_eq!(e.campo.as_deref(), Some("tipo"));
    }

    #[test]
    fn erros_de_estrutura() {
        let e = erro::<FrontmatterLivre>("# Sem frontmatter\n");
        assert!(e.mensagem.contains("sem frontmatter"), "{e}");
        let e = erro::<FrontmatterLivre>("---\ntipo: regra\n");
        assert!(e.mensagem.contains("fechamento"), "{e}");
        let e = erro::<FrontmatterLivre>("---\ntipo: [regra\n---\n");
        assert!(e.mensagem.contains("YAML"), "{e}");
        assert!(e.linha.is_some(), "{e:?}");
        let e = erro::<FrontmatterLivre>("---\n- a\n- b\n---\n");
        assert!(e.mensagem.contains("mapa"), "{e}");
    }

    #[test]
    fn delimitadores() {
        // `----` e `--- x` não fecham; `---` com espaços no fim fecha.
        let doc =
            Documento::<FrontmatterLivre>::analisar("---\na: 1\n---  \n---- corpo\n").unwrap();
        assert_eq!(doc.corpo, "---- corpo\n");
        let doc = Documento::<FrontmatterLivre>::analisar("---\na: 1\n---").unwrap();
        assert_eq!(doc.corpo, "");
        let doc = Documento::<FrontmatterLivre>::analisar("---\n---\ncorpo").unwrap();
        assert!(doc.frontmatter.0.is_empty());
        assert_eq!(doc.para_texto().unwrap(), "---\n---\ncorpo");
    }
}
