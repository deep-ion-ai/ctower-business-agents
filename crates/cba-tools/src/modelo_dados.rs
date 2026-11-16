//! Conferências do modelo de dados conceitual (#120), no registro de um diagrama
//! do tipo `dados` (`registrar_diagrama`):
//!
//! - **Chave estrangeira:** a relação com origem `chave_estrangeira` precisa da
//!   chave declarada nos scripts do banco, entre as duas tabelas, num sentido ou no
//!   outro (mapa estrutural do projeto, como a ferramenta `schema` mostra). Sem ela,
//!   o registro é recusado: a relação que só aparece num join do código é `join`,
//!   inferida. Sem mapa ou com tabela fora dos scripts, não há como conferir: a
//!   relação fica como veio, com aviso.
//! - **Tabelas e colunas:** a tabela que não está nos scripts e o atributo cuja
//!   coluna não está na tabela geram aviso; o atributo que parece coluna técnica
//!   (chave, `ID_...`) também, porque o modelo conceitual mostra só os de negócio.
//! - **Glossário:** cada entidade é ligada ao termo do glossário que vive na
//!   tabela: os termos da base (a seção "Onde vive no código" de
//!   `glossario/<termo>.md`) e os propostos na análise (`onde_vive`), menos os
//!   recusados. O rótulo da entidade precisa ser o termo ou um sinônimo para a
//!   ligação valer; senão, o aviso diz o nome do glossário. Tabela sem termo gera
//!   aviso para propor um com `propor_termo`.

use cba_core::ambiente::Ambiente;
use cba_core::analise::{Analise, EstadoTermo};
use cba_core::diagramas::{NovoDiagrama, OrigemRelacao};
use cba_index::{Achado, Mapa, TipoReferencia};
use cba_kb::frontmatter::FrontmatterTermo;
use cba_kb::{CaminhoRelativo, Documento, layout, slug};

use crate::registro::ErroFerramenta;

/// Termo do glossário candidato a nome de uma entidade.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TermoDoGlossario {
    pub termo: String,
    pub sinonimos: Vec<String>,
    /// Onde o termo vive no código (itens de "Onde vive no código" ou `onde_vive`).
    pub onde_vive: Vec<String>,
}

impl TermoDoGlossario {
    fn nomeia(&self, rotulo: &str) -> bool {
        let s = slug(rotulo);
        slug(&self.termo) == s || self.sinonimos.iter().any(|x| slug(x) == s)
    }

    fn vive_em(&self, tabela: &str) -> bool {
        self.onde_vive.iter().any(|o| menciona_tabela(o, tabela))
    }
}

/// O texto cita a tabela como um todo: o nome como palavra inteira (sem fazer
/// parte de outro nome, como `FATURA` em `LOG_FATURA`) e sem coluna depois
/// (`FATURA.STATUS` é a coluna, não a tabela). Ignora maiúsculas.
pub(crate) fn menciona_tabela(texto: &str, tabela: &str) -> bool {
    let tabela = nome_curto(tabela);
    if tabela.is_empty() {
        return false;
    }
    let texto = texto.to_uppercase();
    let palavra = |c: char| c.is_alphanumeric() || c == '_' || c == '$' || c == '#';
    let mut inicio = 0;
    while let Some(p) = texto[inicio..].find(&tabela) {
        let ini = inicio + p;
        let fim = ini + tabela.len();
        let antes = texto[..ini].chars().next_back();
        let depois = texto[fim..].chars().next();
        let depois2 = texto[fim..].chars().nth(1);
        let separada = !antes.is_some_and(palavra) && !depois.is_some_and(palavra);
        let coluna = depois == Some('.') && depois2.is_some_and(palavra);
        if separada && !coluna {
            return true;
        }
        inicio = fim;
    }
    false
}

/// Nome da tabela sem o esquema, em maiúsculas (`APP.FATURA` → `FATURA`).
fn nome_curto(nome: &str) -> String {
    nome.trim()
        .rsplit('.')
        .next()
        .unwrap_or_default()
        .to_uppercase()
}

/// Itens da seção "Onde vive no código" de um termo da base.
fn onde_vive_do_corpo(corpo: &str) -> Vec<String> {
    let mut dentro = false;
    let mut itens = Vec::new();
    for linha in corpo.lines() {
        let l = linha.trim();
        if let Some(titulo) = l.strip_prefix("## ") {
            dentro = slug(titulo) == "onde-vive-no-codigo";
            continue;
        }
        if dentro && let Some(item) = l.strip_prefix("- ").or_else(|| l.strip_prefix("* ")) {
            itens.push(item.trim().to_owned());
        }
    }
    itens
}

/// Termos do glossário da base e os propostos na análise (não recusados).
pub(crate) fn termos(ambiente: &Ambiente, analise: &Analise) -> Vec<TermoDoGlossario> {
    let mut termos = Vec::new();
    if let Ok(pasta) = CaminhoRelativo::novo(layout::GLOSSARIO)
        && let Ok(entradas) = std::fs::read_dir(ambiente.kb.caminho(&pasta))
    {
        let mut nomes: Vec<String> = entradas
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".md") && !n.starts_with('.'))
            .collect();
        nomes.sort();
        for nome in nomes {
            let Ok(caminho) = CaminhoRelativo::novo(&format!("{}/{nome}", layout::GLOSSARIO))
            else {
                continue;
            };
            let Ok(doc) = ambiente.kb.ler::<FrontmatterTermo>(&caminho) else {
                continue;
            };
            let doc: Documento<FrontmatterTermo> = doc;
            termos.push(TermoDoGlossario {
                termo: doc.frontmatter.termo.clone(),
                sinonimos: doc.frontmatter.sinonimos.clone(),
                onde_vive: onde_vive_do_corpo(&doc.corpo),
            });
        }
    }
    for t in analise
        .termos
        .iter()
        .filter(|t| t.estado != EstadoTermo::Recusado)
    {
        termos.push(TermoDoGlossario {
            termo: t.termo.clone(),
            sinonimos: t.sinonimos.clone(),
            onde_vive: t.onde_vive.clone(),
        });
    }
    termos
}

/// Liga cada entidade ao termo do glossário que vive na tabela e devolve os avisos
/// (nome diferente do glossário, tabela sem termo).
pub(crate) fn ligar_ao_glossario(
    novo: &mut NovoDiagrama,
    termos: &[TermoDoGlossario],
) -> Vec<String> {
    let mut avisos = Vec::new();
    for no in &mut novo.nos {
        let Some(entidade) = no.entidade.as_mut() else {
            continue;
        };
        let candidatos: Vec<&TermoDoGlossario> = termos
            .iter()
            .filter(|t| t.vive_em(&entidade.tabela))
            .collect();
        let por_nome = termos.iter().find(|t| t.nomeia(&no.rotulo));
        if let Some(t) = candidatos.iter().find(|t| t.nomeia(&no.rotulo)) {
            entidade.termo = Some(t.termo.clone());
        } else if let Some(t) = candidatos.first() {
            entidade.termo = None;
            avisos.push(format!(
                "a entidade `{}` (tabela {}) não usa o nome do glossário: o termo que vive \
                 nessa tabela é \"{}\"; registre de novo com esse rótulo (`substituir`)",
                no.rotulo, entidade.tabela, t.termo
            ));
        } else if let Some(t) = por_nome {
            // O termo existe, mas não diz que vive nesta tabela.
            entidade.termo = Some(t.termo.clone());
        } else {
            entidade.termo = None;
            avisos.push(format!(
                "a tabela {} não tem termo no glossário: se `{}` é um conceito do negócio, \
                 proponha o termo com `propor_termo` (onde_vive: \"{}\")",
                entidade.tabela, no.rotulo, entidade.tabela
            ));
        }
    }
    avisos
}

fn tabelas<'a>(mapa: &'a Mapa, nome: &str) -> Vec<Achado<'a>> {
    let curto = nome_curto(nome);
    mapa.buscar(&curto)
        .into_iter()
        .filter(|a| a.simbolo.tipo.eh_schema() && nome_curto(&a.simbolo.qualificado) == curto)
        .collect()
}

/// A tabela (ou uma coluna dela) declara chave estrangeira para `destino`.
fn chave_de(mapa: &Mapa, origem: &Achado<'_>, destino: &str) -> bool {
    let destino = nome_curto(destino);
    let mut ids = vec![origem.id];
    ids.extend(mapa.filhos(origem.id).iter().map(|f| f.id));
    ids.iter().any(|id| {
        mapa.referencias_de(*id).iter().any(|u| {
            u.referencia.tipo == TipoReferencia::ChaveEstrangeira
                && nome_curto(&u.referencia.alvo) == destino
        })
    })
}

/// Chave estrangeira entre as duas tabelas, num sentido ou no outro. `None` quando
/// uma delas não está nos scripts (não há como conferir).
pub(crate) fn tem_chave_estrangeira(mapa: &Mapa, a: &str, b: &str) -> Option<bool> {
    let ta = tabelas(mapa, a);
    let tb = tabelas(mapa, b);
    if ta.is_empty() || tb.is_empty() {
        return None;
    }
    Some(ta.iter().any(|t| chave_de(mapa, t, b)) || tb.iter().any(|t| chave_de(mapa, t, a)))
}

/// Coluna com cara de técnica: chave artificial (`ID`, `ID_...`, `..._ID`).
fn parece_tecnica(coluna: &str) -> bool {
    let c = nome_curto(coluna);
    c == "ID" || c.starts_with("ID_") || c.ends_with("_ID")
}

/// Confere tabelas, colunas e chaves estrangeiras contra o mapa do projeto.
/// Recusa a relação `chave_estrangeira` sem a chave nos scripts; o resto vira aviso.
pub(crate) fn conferir_no_schema(
    mapa: Option<&Mapa>,
    novo: &NovoDiagrama,
) -> Result<Vec<String>, ErroFerramenta> {
    let mut avisos = Vec::new();
    let tabela_do_no = |chave: &str| {
        novo.nos
            .iter()
            .find(|n| n.chave.trim() == chave.trim())
            .and_then(|n| n.entidade.as_ref())
            .map(|e| e.tabela.trim().to_owned())
    };
    for no in &novo.nos {
        let Some(e) = &no.entidade else { continue };
        let tecnicas: Vec<&str> = e
            .atributos
            .iter()
            .map(|a| a.coluna.trim())
            .filter(|c| parece_tecnica(c))
            .collect();
        if !tecnicas.is_empty() {
            avisos.push(format!(
                "a entidade `{}` tem coluna(s) com cara de técnica ({}); o modelo conceitual \
                 mostra só os atributos de negócio: tire chaves e colunas de controle",
                no.rotulo,
                tecnicas.join(", ")
            ));
        }
        let Some(mapa) = mapa else { continue };
        let achadas = tabelas(mapa, &e.tabela);
        if achadas.is_empty() {
            avisos.push(format!(
                "a tabela {} não está nos scripts de banco do projeto; confira o nome com \
                 `schema`",
                e.tabela.trim()
            ));
            continue;
        }
        let colunas: Vec<String> = achadas
            .iter()
            .flat_map(|t| mapa.filhos(t.id))
            .map(|c| nome_curto(&c.simbolo.nome))
            .collect();
        let fora: Vec<&str> = e
            .atributos
            .iter()
            .map(|a| a.coluna.trim())
            .filter(|c| !colunas.contains(&nome_curto(c)))
            .collect();
        if !colunas.is_empty() && !fora.is_empty() {
            avisos.push(format!(
                "coluna(s) que não estão na tabela {}: {}; confira com `schema`",
                e.tabela.trim(),
                fora.join(", ")
            ));
        }
    }
    for (i, aresta) in novo.arestas.iter().enumerate() {
        let Some(r) = aresta.relacao else { continue };
        if r.origem != OrigemRelacao::ChaveEstrangeira {
            continue;
        }
        let (Some(de), Some(para)) = (tabela_do_no(&aresta.de), tabela_do_no(&aresta.para)) else {
            continue;
        };
        match mapa.and_then(|m| tem_chave_estrangeira(m, &de, &para)) {
            Some(true) => {}
            Some(false) => {
                return Err(ErroFerramenta::Recusa(format!(
                    "a relação {} (`{}` → `{}`) diz `chave_estrangeira`, mas os scripts do \
                     banco não declaram chave estrangeira entre {de} e {para}. Se a relação \
                     vem de um join ou de uma gravação no código, use `origem: \"join\"`, \
                     `confianca: \"inferido\"` e a evidência do trecho",
                    i + 1,
                    aresta.de.trim(),
                    aresta.para.trim()
                )));
            }
            None => avisos.push(format!(
                "a chave estrangeira entre {de} e {para} não pôde ser conferida (tabela fora \
                 dos scripts ou mapa indisponível); cite a evidência da constraint"
            )),
        }
    }
    Ok(avisos)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mencao_da_tabela_inteira() {
        assert!(menciona_tabela("tabela FATURA", "FATURA"));
        assert!(menciona_tabela("APP.FATURA (db/01_tabelas.sql)", "fatura"));
        assert!(menciona_tabela("`FATURA`", "APP.FATURA"));
        assert!(!menciona_tabela("LOG_FATURA", "FATURA"));
        assert!(!menciona_tabela("FATURA.STATUS", "FATURA"));
        assert!(menciona_tabela("FATURA. Outra frase", "FATURA"));
        assert!(!menciona_tabela("FATURAMENTO", "FATURA"));
        assert!(menciona_tabela("LOG_FATURA e FATURA", "FATURA"));
    }

    #[test]
    fn onde_vive_da_secao_do_termo() {
        let corpo = "# Fatura\n\n## Definição de negócio\n\n- FATURA não conta\n\n\
                     ## Onde vive no código\n\n- tabela FATURA\n- `Fatura.java`\n\n## Evidências\n\n- x\n";
        assert_eq!(
            onde_vive_do_corpo(corpo),
            vec!["tabela FATURA", "`Fatura.java`"]
        );
    }

    #[test]
    fn coluna_tecnica() {
        assert!(parece_tecnica("ID_CLIENTE"));
        assert!(parece_tecnica("fatura_id"));
        assert!(!parece_tecnica("DT_EMISSAO"));
        assert!(!parece_tecnica("VALIDO"));
    }
}
