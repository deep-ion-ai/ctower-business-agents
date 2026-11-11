//! Entrada da montagem: os registros estruturados de um documento.
//!
//! O agente não escreve o documento; ele registra afirmações, perguntas e
//! sugestões pelas ferramentas MCP (spec, seções 6.1 e 7.3). Quem monta o
//! documento converte esses registros num [`Conteudo`] e chama
//! [`crate::Templates::montar`]. O texto do agente fica só dentro dos campos.

use cba_kb::frontmatter::{Evidencia, FrontmatterRegra, Linhas};
use cba_kb::{CaminhoRelativo, Id};
use serde::{Deserialize, Serialize};

/// Tudo o que entra num documento: o frontmatter tipado, o caminho do documento e
/// os registros, cada um ligado a uma seção do template pelo nome do marcador
/// (`como_e_hoje`, `resposta`, `trecho_atual`...).
///
/// A ordem dos vetores é a ordem de registro e é mantida no documento.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Conteudo<F> {
    pub frontmatter: F,
    /// Onde o documento fica, relativo à raiz do sistema (`analises/A-0193.md`).
    /// Os links para outros documentos da base são calculados a partir dele, para
    /// o documento ter só caminhos relativos.
    pub caminho: CaminhoRelativo,
    /// Textos livres por seção (a demanda como veio, a pergunta, o enunciado de
    /// uma regra, um trecho de código). Saem como parágrafos, antes das
    /// afirmações da seção.
    #[serde(default)]
    pub textos: Vec<Texto>,
    /// Afirmações registradas com `afirmar`. Saem como lista, com o selo de
    /// confiança e as notas de rodapé das evidências.
    #[serde(default)]
    pub afirmacoes: Vec<Afirmacao>,
    /// Perguntas em aberto (para o solicitante), na seção `perguntas`.
    #[serde(default)]
    pub perguntas: Vec<Pergunta>,
    /// Sugestões de alteração registradas na análise, na seção `sugestoes`, com
    /// link para o documento de cada uma.
    #[serde(default)]
    pub sugestoes: Vec<ReferenciaSugestao>,
}

impl<F> Conteudo<F> {
    /// Conteúdo sem registros.
    pub fn novo(frontmatter: F, caminho: CaminhoRelativo) -> Self {
        Self {
            frontmatter,
            caminho,
            textos: Vec::new(),
            afirmacoes: Vec::new(),
            perguntas: Vec::new(),
            sugestoes: Vec::new(),
        }
    }
}

impl Conteudo<FrontmatterRegra> {
    /// Conteúdo de uma regra: o enunciado, com as evidências do frontmatter (com
    /// hash e commit) como notas de rodapé. O caminho é o padrão da regra
    /// (`regras/RN-FAT-001.md`).
    pub fn da_regra(frontmatter: FrontmatterRegra, enunciado: impl Into<String>) -> Self {
        let citacoes = frontmatter.evidencias.iter().map(Citacao::from).collect();
        let caminho = frontmatter.id.arquivo();
        let mut conteudo = Self::novo(frontmatter, caminho);
        conteudo.textos.push(Texto {
            secao: "enunciado".into(),
            texto: enunciado.into(),
            citacoes,
        });
        conteudo
    }
}

/// Texto livre de uma seção, em Markdown.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Texto {
    /// Marcador da seção (`demanda_como_veio`).
    pub secao: String,
    pub texto: String,
    /// Trechos de código que sustentam o texto, como notas de rodapé.
    #[serde(default)]
    pub citacoes: Vec<Citacao>,
}

impl Texto {
    pub fn novo(secao: impl Into<String>, texto: impl Into<String>) -> Self {
        Self {
            secao: secao.into(),
            texto: texto.into(),
            citacoes: Vec::new(),
        }
    }
}

/// Afirmação registrada pelo agente (spec, seção 6.5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Afirmacao {
    /// Marcador da seção (`como_e_hoje`).
    pub secao: String,
    pub texto: String,
    pub confianca: Confianca,
    #[serde(default)]
    pub evidencias: Vec<Citacao>,
}

/// Nível de confiança de uma afirmação, mostrado como selo no documento.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confianca {
    /// Evidência direta.
    Confirmado,
    /// Dedução a partir das evidências listadas.
    Inferido,
    /// Sem evidência suficiente.
    AConfirmar,
}

impl Confianca {
    /// Texto do selo (`a confirmar`).
    pub fn rotulo(self) -> &'static str {
        match self {
            Self::Confirmado => "confirmado",
            Self::Inferido => "inferido",
            Self::AConfirmar => "a confirmar",
        }
    }
}

/// Trecho de código citado: caminho relativo à raiz do sistema e intervalo de
/// linhas. O hash e o commit, quando conhecidos, aparecem na nota de rodapé.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Citacao {
    pub arquivo: CaminhoRelativo,
    pub linhas: Linhas,
    #[serde(default)]
    pub hash: Option<String>,
    #[serde(default)]
    pub commit: Option<String>,
}

impl Citacao {
    pub fn nova(arquivo: CaminhoRelativo, linhas: Linhas) -> Self {
        Self {
            arquivo,
            linhas,
            hash: None,
            commit: None,
        }
    }

    /// `arquivo:linhas`, com uma linha só quando o intervalo tem uma linha
    /// (`db/02_parametros.sql:7`).
    pub fn local(&self) -> String {
        if self.linhas.inicio == self.linhas.fim {
            format!("{}:{}", self.arquivo, self.linhas.inicio)
        } else {
            format!("{}:{}", self.arquivo, self.linhas)
        }
    }
}

impl From<&Evidencia> for Citacao {
    fn from(e: &Evidencia) -> Self {
        Self {
            arquivo: e.arquivo.clone(),
            linhas: e.linhas,
            hash: Some(e.hash.clone()),
            commit: e.commit.clone(),
        }
    }
}

/// Pergunta em aberto, registrada com `perguntar`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pergunta {
    pub texto: String,
}

/// Sugestão de alteração citada num documento.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferenciaSugestao {
    pub id: Id,
    pub titulo: String,
    /// Arquivo do sistema que a sugestão altera.
    #[serde(default)]
    pub arquivo: Option<CaminhoRelativo>,
}

/// Bloco de código cercado, com uma cerca mais longa que qualquer sequência de
/// crases do código, para o trecho nunca fechar o bloco antes da hora.
pub fn bloco_de_codigo(codigo: &str, linguagem: &str) -> String {
    let mut maior = 0;
    let mut atual = 0;
    for c in codigo.chars() {
        if c == '`' {
            atual += 1;
            maior = maior.max(atual);
        } else {
            atual = 0;
        }
    }
    let cerca = "`".repeat((maior + 1).max(3));
    let codigo = codigo.strip_suffix('\n').unwrap_or(codigo);
    format!("{cerca}{linguagem}\n{codigo}\n{cerca}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cerca_maior_que_as_crases_do_codigo() {
        assert_eq!(bloco_de_codigo("x := 1;\n", "sql"), "```sql\nx := 1;\n```");
        assert_eq!(bloco_de_codigo("a ```` b", ""), "`````\na ```` b\n`````");
    }

    #[test]
    fn local_da_citacao() {
        let c = Citacao::nova(
            CaminhoRelativo::novo("db/02_parametros.sql").unwrap(),
            Linhas::novo(7, 7).unwrap(),
        );
        assert_eq!(c.local(), "db/02_parametros.sql:7");
        let c = Citacao::nova(c.arquivo, Linhas::novo(6, 7).unwrap());
        assert_eq!(c.local(), "db/02_parametros.sql:6-7");
    }

    #[test]
    fn confianca_em_snake_case() {
        let c: Confianca = serde_norway::from_str("a_confirmar").unwrap();
        assert_eq!(c, Confianca::AConfirmar);
        assert_eq!(c.rotulo(), "a confirmar");
    }
}
