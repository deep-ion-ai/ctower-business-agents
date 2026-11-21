//! Marcadores de ida e volta da documentação publicada (#288).
//!
//! As páginas legíveis ([`crate::gerar_documentacao`]) juntam vários documentos
//! da base numa página só (as regras de uma funcionalidade, o glossário). Para a
//! importação (#290) recortar cada documento de volta, cada um vem entre dois
//! comentários HTML, invisíveis no GitHub e no GitLab:
//!
//! ```text
//! <!-- cba:regra id=RN-COB-002 {"caminho":"regras/RN-COB-002.md","frontmatter":{...}} -->
//! <a id="rn-cob-002"></a>
//! ### RN-COB-002 · Multa de 2% sobre a parcela em atraso
//! …texto legível…
//! <!-- /cba -->
//! ```
//!
//! # Formato
//!
//! - **Abertura**, sozinha numa linha: `<!-- cba:<tipo> id=<id> <json> -->`.
//!   - `<tipo>`: o `tipo` do frontmatter do documento (`regra`, `termo`,
//!     `pergunta`, `analise-impacto`, `funcionalidade`, `sugestao`...), só
//!     `a-z`, `0-9` e `-`.
//!   - `<id>`: o identificador do documento, sem espaço, sem `--` e sem `<` ou
//!     `>`: o `id` do frontmatter (`RN-COB-002`, `A-0001`, `S-0003`); no termo,
//!     o nome do arquivo no glossário (`penalidade-multa-por-atraso`); no
//!     documento negocial ou caso de uso, `<area>/<tema>` (a pasta dele).
//!   - `<json>`: um objeto JSON compacto numa linha ([`Metadados`]): o caminho do
//!     documento na base (relativo à raiz do projeto) e o frontmatter completo,
//!     como objeto. Com o frontmatter e o texto do bloco, a importação
//!     reconstrói o documento (status, área, funcionalidades, evidências com
//!     hash e commit) para a busca. Para o comentário nunca fechar antes da hora,
//!     `<` e `>` saem como `<` e `>` e todo `--` como `--`
//!     (escapes válidos do JSON, que o leitor de JSON desfaz).
//! - **Fechamento**, sozinho numa linha: `<!-- /cba -->`.
//! - Os blocos não se aninham. Um documento tem um bloco só, na página
//!   canônica dele; nas outras páginas aparece só como link.
//! - Logo depois da abertura vem a âncora do documento (`<a id="..."></a>`), que
//!   [`Bloco::ancora`] devolve, para a origem `caminho#âncora`.
//!
//! [`Marcador::abertura`] escreve a abertura, [`Marcador::analisar`] lê e
//! [`blocos`] recorta uma página inteira.

use serde::{Deserialize, Serialize};

/// Fechamento de todo bloco.
pub const FECHAMENTO: &str = "<!-- /cba -->";

const PREFIXO: &str = "<!-- cba:";
const SUFIXO: &str = " -->";

/// Metadados de um documento no marcador de abertura.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Metadados {
    /// Caminho do documento na base, relativo à raiz do projeto
    /// (`regras/RN-COB-002.md`).
    pub caminho: String,
    /// Frontmatter do documento, como objeto JSON.
    pub frontmatter: serde_json::Value,
}

/// Marcador de abertura de um bloco.
#[derive(Debug, Clone, PartialEq)]
pub struct Marcador {
    pub tipo: String,
    pub id: String,
    pub metadados: Metadados,
}

/// Erro ao escrever ou ler um marcador.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ErroMarcador {
    #[error("tipo inválido no marcador: `{0}`")]
    Tipo(String),
    #[error("identificador inválido no marcador: `{0}`")]
    Id(String),
    #[error("a linha não é um marcador de abertura")]
    NaoEhAbertura,
    #[error("metadados ilegíveis no marcador: {0}")]
    Metadados(String),
    #[error("bloco {0} sem fechamento")]
    SemFechamento(String),
    #[error("bloco {0} aberto dentro de outro")]
    Aninhado(String),
}

fn tipo_valido(tipo: &str) -> bool {
    !tipo.is_empty()
        && tipo
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// Se `id` pode ir no marcador.
pub fn id_valido(id: &str) -> bool {
    !id.is_empty()
        && !id.contains("--")
        && !id
            .chars()
            .any(|c| c.is_whitespace() || c == '<' || c == '>' || c == '{')
}

impl Marcador {
    pub fn novo(
        tipo: impl Into<String>,
        id: impl Into<String>,
        metadados: Metadados,
    ) -> Result<Self, ErroMarcador> {
        let (tipo, id) = (tipo.into(), id.into());
        if !tipo_valido(&tipo) {
            return Err(ErroMarcador::Tipo(tipo));
        }
        if !id_valido(&id) {
            return Err(ErroMarcador::Id(id));
        }
        Ok(Self {
            tipo,
            id,
            metadados,
        })
    }

    /// A linha de abertura, sem a quebra de linha.
    pub fn abertura(&self) -> String {
        let json = serde_json::to_string(&self.metadados)
            .expect("metadados em JSON")
            .replace('<', "\\u003c")
            .replace('>', "\\u003e")
            .replace("--", "-\\u002d");
        format!("{PREFIXO}{} id={} {json}{SUFIXO}", self.tipo, self.id)
    }

    /// Lê uma linha de abertura (espaços nas pontas não contam).
    pub fn analisar(linha: &str) -> Result<Self, ErroMarcador> {
        let resto = linha
            .trim()
            .strip_prefix(PREFIXO)
            .and_then(|r| r.strip_suffix(SUFIXO))
            .ok_or(ErroMarcador::NaoEhAbertura)?;
        let (tipo, resto) = resto.split_once(' ').ok_or(ErroMarcador::NaoEhAbertura)?;
        let resto = resto
            .strip_prefix("id=")
            .ok_or(ErroMarcador::NaoEhAbertura)?;
        let (id, json) = resto.split_once(' ').ok_or(ErroMarcador::NaoEhAbertura)?;
        let metadados: Metadados =
            serde_json::from_str(json).map_err(|e| ErroMarcador::Metadados(e.to_string()))?;
        Self::novo(tipo, id, metadados)
    }
}

/// Se a linha é uma abertura de bloco (mesmo com metadados ilegíveis).
fn parece_abertura(linha: &str) -> bool {
    linha.trim().starts_with(PREFIXO)
}

/// Um documento recortado de uma página.
#[derive(Debug, Clone, PartialEq)]
pub struct Bloco {
    pub marcador: Marcador,
    /// Texto entre a abertura e o fechamento, sem as duas linhas.
    pub conteudo: String,
    /// Linha da abertura na página (1-indexada).
    pub linha: usize,
}

impl Bloco {
    /// Âncora do documento na página (`rn-cob-002`): o `<a id="..."></a>` que
    /// abre o conteúdo.
    pub fn ancora(&self) -> Option<&str> {
        let linha = self.conteudo.lines().find(|l| !l.trim().is_empty())?;
        let resto = linha.trim().strip_prefix("<a id=\"")?;
        let (id, fim) = resto.split_once('"')?;
        (fim == "></a>").then_some(id)
    }
}

/// Recorta os blocos marcados de uma página, na ordem. Texto fora dos blocos é
/// ignorado. Erra com bloco sem fechamento, bloco dentro de bloco ou abertura
/// ilegível.
pub fn blocos(pagina: &str) -> Result<Vec<Bloco>, ErroMarcador> {
    let mut saida = Vec::new();
    let mut atual: Option<(Marcador, usize, Vec<&str>)> = None;
    for (i, linha) in pagina.lines().enumerate() {
        if linha.trim() == FECHAMENTO {
            if let Some((marcador, n, linhas)) = atual.take() {
                saida.push(Bloco {
                    marcador,
                    conteudo: linhas.join("\n"),
                    linha: n,
                });
            }
            continue;
        }
        if parece_abertura(linha) {
            let m = Marcador::analisar(linha)?;
            if let Some((aberto, _, _)) = &atual {
                return Err(ErroMarcador::Aninhado(format!(
                    "{}:{} (dentro de {}:{})",
                    m.tipo, m.id, aberto.tipo, aberto.id
                )));
            }
            atual = Some((m, i + 1, Vec::new()));
            continue;
        }
        if let Some((_, _, linhas)) = &mut atual {
            linhas.push(linha);
        }
    }
    match atual {
        Some((m, _, _)) => Err(ErroMarcador::SemFechamento(format!("{}:{}", m.tipo, m.id))),
        None => Ok(saida),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn marcador() -> Marcador {
        Marcador::novo(
            "regra",
            "RN-COB-002",
            Metadados {
                caminho: "regras/RN-COB-002.md".into(),
                frontmatter: json!({
                    "tipo": "regra",
                    "id": "RN-COB-002",
                    "titulo": "Multa <de> 2% --> fim -- e ---",
                    "status": "confirmada",
                    "funcionalidades": ["F-COB-001"],
                    "evidencias": [{"arquivo": "src/a.sql", "linhas": "1-2", "hash": "ab", "commit": "c1"}],
                }),
            },
        )
        .unwrap()
    }

    #[test]
    fn ida_e_volta() {
        let m = marcador();
        let linha = m.abertura();
        assert!(linha.starts_with("<!-- cba:regra id=RN-COB-002 {"));
        // Nada fecha o comentário antes da hora.
        let miolo = &linha[4..linha.len() - 3];
        assert!(!miolo.contains("--"), "{linha}");
        assert!(!miolo.contains('>'), "{linha}");
        assert!(!linha.contains('\n'));
        assert_eq!(Marcador::analisar(&linha).unwrap(), m);
        assert_eq!(Marcador::analisar(&format!("  {linha}  ")).unwrap(), m);
    }

    #[test]
    fn recusa_tipo_e_id_invalidos() {
        let meta = marcador().metadados;
        assert!(Marcador::novo("Regra", "x", meta.clone()).is_err());
        assert!(Marcador::novo("regra", "a b", meta.clone()).is_err());
        assert!(Marcador::novo("regra", "a--b", meta.clone()).is_err());
        assert!(Marcador::novo("termo", "eventos/fuga", meta).is_ok());
        assert_eq!(
            Marcador::analisar("<!-- comentário -->"),
            Err(ErroMarcador::NaoEhAbertura)
        );
        assert!(matches!(
            Marcador::analisar("<!-- cba:regra id=X {nao json} -->"),
            Err(ErroMarcador::Metadados(_))
        ));
    }

    #[test]
    fn recorta_os_blocos_da_pagina() {
        let m = marcador();
        let pagina = format!(
            "# Página\n\nIntrodução.\n\n{}\n<a id=\"rn-cob-002\"></a>\n### RN-COB-002 · Multa\n\nTexto.\n{FECHAMENTO}\n\nFim.\n",
            m.abertura()
        );
        let b = blocos(&pagina).unwrap();
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].marcador, m);
        assert_eq!(b[0].linha, 5);
        assert_eq!(b[0].ancora(), Some("rn-cob-002"));
        assert_eq!(
            b[0].conteudo,
            "<a id=\"rn-cob-002\"></a>\n### RN-COB-002 · Multa\n\nTexto."
        );
        assert!(blocos("# Sem blocos\n").unwrap().is_empty());
    }

    #[test]
    fn erros_de_estrutura() {
        let a = marcador().abertura();
        assert!(matches!(
            blocos(&format!("{a}\ntexto\n")),
            Err(ErroMarcador::SemFechamento(_))
        ));
        assert!(matches!(
            blocos(&format!("{a}\n{a}\n{FECHAMENTO}\n")),
            Err(ErroMarcador::Aninhado(_))
        ));
    }
}
