//! Peças comuns aos artefatos de requisito derivados de uma análise (épico #127):
//! histórias com critérios de aceite (#115) e, depois, cenários de teste (#117).
//!
//! - [`RefItem`]: referência a um item numerado de uma análise do projeto
//!   (`A-0193/4`, a afirmação ou a pergunta 4 da análise A-0193).
//! - [`Fundamento`]: o que sustenta um item de requisito: uma afirmação de uma
//!   análise ou uma regra de negócio da base (`RN-…`). Guarda uma cópia do texto
//!   (ou do título) e das evidências no momento do registro, para o documento e a
//!   exportação mostrarem a ligação sem reler a origem. Quem confere que a
//!   referência existe é a ferramenta de registro (`cba_tools::fundamentos`).
//! - [`SituacaoItem`]: a decisão do analista sobre um item (pendente, aceito,
//!   descartado).
//!
//! Este módulo é só domínio, sem disco.

use std::fmt;

use cba_kb::frontmatter::Evidencia;
use cba_kb::{Id, Serie};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Item numerado de uma análise do projeto: a afirmação ou a pergunta `numero`
/// da análise `analise`. Escrito `A-0193/4`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RefItem {
    pub analise: Id,
    pub numero: u32,
}

/// Referência fora do formato `A-NNNN/n`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("`{0}` não é uma referência a um item de análise; use A-NNNN/n, como A-0193/4")]
pub struct RefInvalida(pub String);

impl RefItem {
    pub fn nova(analise: Id, numero: u32) -> Self {
        Self { analise, numero }
    }

    /// Lê `A-0193/4` (sem espaços; número a partir de 1).
    pub fn analisar(texto: &str) -> Result<Self, RefInvalida> {
        let erro = || RefInvalida(texto.to_owned());
        let (analise, numero) = texto.split_once('/').ok_or_else(erro)?;
        let analise = Id::analisar(analise).map_err(|_| erro())?;
        // Só dígitos, sem zero à esquerda (o número começa em 1).
        if analise.serie() != &Serie::Analise
            || numero.is_empty()
            || numero.starts_with('0')
            || !numero.bytes().all(|b| b.is_ascii_digit())
        {
            return Err(erro());
        }
        let numero: u32 = numero.parse().map_err(|_| erro())?;
        Ok(Self { analise, numero })
    }
}

impl fmt::Display for RefItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.analise, self.numero)
    }
}

impl Serialize for RefItem {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for RefItem {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let texto = String::deserialize(d)?;
        Self::analisar(&texto).map_err(serde::de::Error::custom)
    }
}

/// O que sustenta um item de requisito (um critério de aceite, um cenário de
/// teste): uma afirmação de uma análise ou uma regra de negócio da base. É o que
/// liga o requisito ao código, pelas evidências da afirmação ou da regra.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "tipo", rename_all = "kebab-case")]
pub enum Fundamento {
    /// Afirmação de uma análise do projeto.
    Afirmacao {
        #[serde(rename = "ref")]
        referencia: RefItem,
        /// Texto da afirmação no registro.
        texto: String,
        /// Evidências da afirmação no registro (sem o trecho, que fica na
        /// análise de origem).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        evidencias: Vec<Evidencia>,
    },
    /// Regra de negócio da base (`regras/RN-<SIGLA>-<NNN>.md`).
    Regra {
        id: Id,
        /// Título da regra no registro.
        titulo: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        evidencias: Vec<Evidencia>,
    },
}

impl Fundamento {
    /// Rótulo curto: `A-0193/4` ou `RN-FAT-007`.
    pub fn rotulo(&self) -> String {
        match self {
            Self::Afirmacao { referencia, .. } => referencia.to_string(),
            Self::Regra { id, .. } => id.to_string(),
        }
    }

    /// Texto da afirmação ou título da regra.
    pub fn texto(&self) -> &str {
        match self {
            Self::Afirmacao { texto, .. } => texto,
            Self::Regra { titulo, .. } => titulo,
        }
    }

    pub fn evidencias(&self) -> &[Evidencia] {
        match self {
            Self::Afirmacao { evidencias, .. } | Self::Regra { evidencias, .. } => evidencias,
        }
    }

    /// Mesma referência (a cópia do texto não conta).
    pub fn mesma_referencia(&self, outro: &Self) -> bool {
        match (self, outro) {
            (Self::Afirmacao { referencia: a, .. }, Self::Afirmacao { referencia: b, .. }) => {
                a == b
            }
            (Self::Regra { id: a, .. }, Self::Regra { id: b, .. }) => a == b,
            _ => false,
        }
    }
}

/// Tira referências repetidas, mantendo a ordem.
pub fn sem_repetidos(fundamentos: Vec<Fundamento>) -> Vec<Fundamento> {
    let mut saida: Vec<Fundamento> = Vec::with_capacity(fundamentos.len());
    for f in fundamentos {
        if !saida.iter().any(|s| s.mesma_referencia(&f)) {
            saida.push(f);
        }
    }
    saida
}

/// Decisão do analista sobre um item de requisito (um critério de aceite, um
/// cenário de teste).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SituacaoItem {
    /// Registrado pelo agente, sem decisão.
    #[default]
    Pendente,
    /// Aceito: vai para o documento aprovado.
    Aceito,
    /// Descartado: fica no estado e sai do documento.
    Descartado,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn referencia_a_item() {
        let r = RefItem::analisar("A-0193/4").unwrap();
        assert_eq!(r.analise.to_string(), "A-0193");
        assert_eq!(r.numero, 4);
        assert_eq!(r.to_string(), "A-0193/4");
        for ruim in [
            "A-0193",
            "A-0193/",
            "A-0193/0",
            "A-0193/04",
            "S-0001/1",
            "A-193/1",
            "4",
            "A-0193/a",
            " A-0193/4",
        ] {
            assert!(RefItem::analisar(ruim).is_err(), "{ruim:?}");
        }
        let json = serde_json::to_string(&r).unwrap();
        assert_eq!(json, "\"A-0193/4\"");
        assert_eq!(serde_json::from_str::<RefItem>(&json).unwrap(), r);
    }

    #[test]
    fn fundamento_no_formato_do_estado() {
        let f = Fundamento::Afirmacao {
            referencia: RefItem::analisar("A-0001/2").unwrap(),
            texto: "Pessoa física não parcela.".into(),
            evidencias: vec![],
        };
        let v = serde_json::to_value(&f).unwrap();
        assert_eq!(
            v,
            serde_json::json!({ "tipo": "afirmacao", "ref": "A-0001/2", "texto": "Pessoa física não parcela." })
        );
        assert_eq!(f.rotulo(), "A-0001/2");
        let r = Fundamento::Regra {
            id: Id::analisar("RN-FAT-007").unwrap(),
            titulo: "Limite de parcelas".into(),
            evidencias: vec![],
        };
        assert_eq!(r.rotulo(), "RN-FAT-007");
        let lista = sem_repetidos(vec![f.clone(), r.clone(), f.clone()]);
        assert_eq!(lista, vec![f, r]);
    }
}
