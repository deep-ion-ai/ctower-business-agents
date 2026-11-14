//! Identificadores legíveis dos documentos (spec, seção 11): `A-0193` para análises,
//! `S-0001` para sugestões e `RN-<AREA>-<NNN>` para regras.
//!
//! A numeração é sequencial por série e por projeto. A reserva sem colisão está em
//! [`crate::Projeto::criar_numerado`].

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::caminho::CaminhoRelativo;
use crate::layout;

/// Série de numeração de um identificador.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Serie {
    /// `A-NNNN`, em `analises/`: análises de impacto, perguntas e explicações.
    Analise,
    /// `S-NNNN`, em `sugestoes/`: sugestões de alteração.
    Sugestao,
    /// `RN-<SIGLA>-NNN`, em `regras/`. A sigla tem de 1 a 10 caracteres `A-Z0-9`.
    Regra { sigla: String },
}

/// Identificador ou sigla recusados.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("identificador inválido `{valor}`: {motivo}")]
pub struct IdInvalido {
    pub valor: String,
    pub motivo: &'static str,
}

impl Serie {
    /// Série de regras com a sigla informada (por exemplo, `FAT`).
    pub fn regra(sigla: &str) -> Result<Self, IdInvalido> {
        if sigla.is_empty()
            || sigla.len() > 10
            || !sigla
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
        {
            return Err(IdInvalido {
                valor: sigla.to_owned(),
                motivo: "a sigla da regra tem de 1 a 10 caracteres A-Z ou 0-9",
            });
        }
        Ok(Self::Regra {
            sigla: sigla.to_owned(),
        })
    }

    /// Pasta do projeto onde ficam os documentos da série.
    pub fn pasta(&self) -> &'static str {
        match self {
            Self::Analise => layout::ANALISES,
            Self::Sugestao => layout::SUGESTOES,
            Self::Regra { .. } => layout::REGRAS,
        }
    }

    fn prefixo(&self) -> String {
        match self {
            Self::Analise => "A-".to_owned(),
            Self::Sugestao => "S-".to_owned(),
            Self::Regra { sigla } => format!("RN-{sigla}-"),
        }
    }

    fn digitos(&self) -> usize {
        match self {
            Self::Analise | Self::Sugestao => 4,
            Self::Regra { .. } => 3,
        }
    }

    /// Identificador de número `numero` (a partir de 1) nesta série.
    pub fn id(&self, numero: u32) -> Id {
        Id {
            serie: self.clone(),
            numero: numero.max(1),
        }
    }

    /// Número do identificador `texto`, se ele for desta série.
    pub fn numero(&self, texto: &str) -> Option<u32> {
        let id = Id::analisar(texto).ok()?;
        (id.serie == *self).then_some(id.numero)
    }
}

/// Identificador legível de um documento.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Id {
    serie: Serie,
    numero: u32,
}

impl Id {
    /// Lê um identificador (`A-0001`, `S-0012`, `RN-FAT-007`). O texto precisa
    /// estar na forma canônica: zeros à esquerda só até o mínimo de dígitos.
    pub fn analisar(texto: &str) -> Result<Self, IdInvalido> {
        let erro = |motivo| IdInvalido {
            valor: texto.to_owned(),
            motivo,
        };
        let (serie, numero) = if let Some(resto) = texto.strip_prefix("RN-") {
            let (sigla, numero) = resto
                .rsplit_once('-')
                .ok_or_else(|| erro("esperado RN-<SIGLA>-<NNN>"))?;
            (Serie::regra(sigla).map_err(|e| erro(e.motivo))?, numero)
        } else if let Some(numero) = texto.strip_prefix("A-") {
            (Serie::Analise, numero)
        } else if let Some(numero) = texto.strip_prefix("S-") {
            (Serie::Sugestao, numero)
        } else {
            return Err(erro("esperado A-NNNN, S-NNNN ou RN-<SIGLA>-NNN"));
        };
        if numero.is_empty() || !numero.bytes().all(|b| b.is_ascii_digit()) {
            return Err(erro("o número tem só dígitos"));
        }
        let n: u32 = numero.parse().map_err(|_| erro("número grande demais"))?;
        if n == 0 {
            return Err(erro("a numeração começa em 1"));
        }
        let id = Id { serie, numero: n };
        if id.to_string() != texto {
            return Err(erro("fora da forma canônica (dígitos a mais ou a menos)"));
        }
        Ok(id)
    }

    pub fn serie(&self) -> &Serie {
        &self.serie
    }

    pub fn numero(&self) -> u32 {
        self.numero
    }

    /// Arquivo do documento, relativo à raiz do projeto (`analises/A-0001.md`).
    pub fn arquivo(&self) -> CaminhoRelativo {
        CaminhoRelativo::novo(&format!("{}/{self}.md", self.serie.pasta()))
            .expect("pasta e identificador são caminhos válidos")
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{:0largura$}",
            self.serie.prefixo(),
            self.numero,
            largura = self.serie.digitos()
        )
    }
}

impl std::str::FromStr for Id {
    type Err = IdInvalido;

    fn from_str(texto: &str) -> Result<Self, Self::Err> {
        Self::analisar(texto)
    }
}

impl Serialize for Id {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Id {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let texto = String::deserialize(d)?;
        Self::analisar(&texto).map_err(serde::de::Error::custom)
    }
}

/// Identificador de uma demanda, `D-NNNN`, sequencial por projeto.
///
/// Fica fora de [`Serie`] porque a demanda é uma pasta
/// (`demandas/D-NNNN-<slug>/demanda.md`), não um arquivo `<pasta>/<id>.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IdDemanda(u32);

impl IdDemanda {
    /// Prefixo do identificador e do nome da pasta.
    pub const PREFIXO: &'static str = "D-";

    /// Identificador de número `numero` (a partir de 1).
    pub fn novo(numero: u32) -> Self {
        Self(numero.max(1))
    }

    pub fn numero(self) -> u32 {
        self.0
    }

    /// Lê um identificador na forma canônica (`D-0007`, `D-12345`).
    pub fn analisar(texto: &str) -> Result<Self, IdInvalido> {
        let erro = |motivo| IdInvalido {
            valor: texto.to_owned(),
            motivo,
        };
        let numero = texto
            .strip_prefix(Self::PREFIXO)
            .ok_or_else(|| erro("esperado D-NNNN"))?;
        if numero.is_empty() || !numero.bytes().all(|b| b.is_ascii_digit()) {
            return Err(erro("o número tem só dígitos"));
        }
        let n: u32 = numero.parse().map_err(|_| erro("número grande demais"))?;
        if n == 0 {
            return Err(erro("a numeração começa em 1"));
        }
        let id = Self(n);
        if id.to_string() != texto {
            return Err(erro("fora da forma canônica (dígitos a mais ou a menos)"));
        }
        Ok(id)
    }

    /// Identificador de uma pasta de demanda: `D-NNNN` ou `D-NNNN-<slug>`.
    pub fn de_pasta(nome: &str) -> Option<Self> {
        let resto = nome.strip_prefix(Self::PREFIXO)?;
        let fim = resto.find('-').unwrap_or(resto.len());
        let id = Self::analisar(&nome[..Self::PREFIXO.len() + fim]).ok()?;
        let slug = &resto[fim..];
        (slug.is_empty() || slug.strip_prefix('-').is_some_and(crate::caminho::e_slug))
            .then_some(id)
    }
}

impl fmt::Display for IdDemanda {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{:04}", Self::PREFIXO, self.0)
    }
}

impl std::str::FromStr for IdDemanda {
    type Err = IdInvalido;

    fn from_str(texto: &str) -> Result<Self, Self::Err> {
        Self::analisar(texto)
    }
}

impl Serialize for IdDemanda {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for IdDemanda {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let texto = String::deserialize(d)?;
        Self::analisar(&texto).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forma_canonica() {
        assert_eq!(Serie::Analise.id(193).to_string(), "A-0193");
        assert_eq!(Serie::Sugestao.id(1).to_string(), "S-0001");
        assert_eq!(Serie::Analise.id(12345).to_string(), "A-12345");
        let rn = Serie::regra("FAT").unwrap();
        assert_eq!(rn.id(7).to_string(), "RN-FAT-007");
        assert_eq!(rn.id(1000).to_string(), "RN-FAT-1000");
    }

    #[test]
    fn analisa_e_volta_igual() {
        for texto in ["A-0193", "S-0001", "RN-FAT-007", "A-12345", "RN-COB2-120"] {
            assert_eq!(Id::analisar(texto).unwrap().to_string(), texto);
        }
        let id = Id::analisar("RN-FAT-007").unwrap();
        assert_eq!(id.serie(), &Serie::regra("FAT").unwrap());
        assert_eq!(id.numero(), 7);
        assert_eq!(id.arquivo().as_str(), "regras/RN-FAT-007.md");
        assert_eq!(
            Id::analisar("A-0001").unwrap().arquivo().as_str(),
            "analises/A-0001.md"
        );
    }

    #[test]
    fn recusa_fora_do_formato() {
        for ruim in [
            "A-1",
            "A-00001",
            "A-0000",
            "B-0001",
            "RN-fat-001",
            "RN-FAT-01",
            "RN--001",
            "A-12a4",
            "",
        ] {
            assert!(Id::analisar(ruim).is_err(), "{ruim:?}");
        }
    }

    #[test]
    fn numero_so_da_mesma_serie() {
        let fat = Serie::regra("FAT").unwrap();
        assert_eq!(fat.numero("RN-FAT-012"), Some(12));
        assert_eq!(fat.numero("RN-COB-012"), None);
        assert_eq!(Serie::Analise.numero("A-0042"), Some(42));
        assert_eq!(Serie::Analise.numero("S-0042"), None);
    }

    #[test]
    fn id_de_demanda() {
        assert_eq!(IdDemanda::novo(7).to_string(), "D-0007");
        assert_eq!(IdDemanda::novo(12345).to_string(), "D-12345");
        assert_eq!(IdDemanda::analisar("D-0007").unwrap().numero(), 7);
        for ruim in ["D-7", "D-00007", "D-0000", "A-0001", "D-", "D-00a1"] {
            assert!(IdDemanda::analisar(ruim).is_err(), "{ruim:?}");
        }
        assert_eq!(IdDemanda::de_pasta("D-0003"), Some(IdDemanda::novo(3)));
        assert_eq!(
            IdDemanda::de_pasta("D-0003-nota-fiscal-de-servico"),
            Some(IdDemanda::novo(3))
        );
        for ruim in [
            "D-3-x",
            "D-0003-",
            "D-0003-Nota",
            "D-0003x",
            "analises",
            ".D-0003",
        ] {
            assert_eq!(IdDemanda::de_pasta(ruim), None, "{ruim:?}");
        }
    }
}
