//! Caminhos relativos à raiz da base ou do projeto.
//!
//! Todo caminho guardado num documento é relativo e usa `/` como separador, para a
//! base continuar válida em outra máquina ou sistema operacional (spec, seção 7.1).

use std::fmt;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Caminho relativo validado: sem raiz, sem unidade do Windows, sem `..`, com `/`
/// como separador e sem componentes vazios ou `.`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CaminhoRelativo(String);

/// Caminho recusado por [`CaminhoRelativo::novo`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("caminho inválido `{caminho}`: {motivo}")]
pub struct CaminhoInvalido {
    pub caminho: String,
    pub motivo: &'static str,
}

impl CaminhoRelativo {
    /// Valida e normaliza um caminho. Aceita `\` como separador (troca por `/`) e
    /// descarta componentes `.` e vazios.
    pub fn novo(caminho: &str) -> Result<Self, CaminhoInvalido> {
        let erro = |motivo| CaminhoInvalido {
            caminho: caminho.to_owned(),
            motivo,
        };
        if caminho.contains('\0') {
            return Err(erro("contém caractere nulo"));
        }
        let normalizado = caminho.replace('\\', "/");
        if normalizado.starts_with('/') {
            return Err(erro("caminho absoluto; use um caminho relativo à raiz"));
        }
        if normalizado.contains(':') {
            return Err(erro("contém `:` (unidade do Windows ou fluxo alternativo)"));
        }
        let mut partes = Vec::new();
        for parte in normalizado.split('/') {
            match parte {
                "" | "." => {}
                ".." => return Err(erro("contém `..`; o caminho precisa ficar dentro da raiz")),
                p => partes.push(p),
            }
        }
        if partes.is_empty() {
            return Err(erro("caminho vazio"));
        }
        Ok(Self(partes.join("/")))
    }

    /// Converte um caminho do sistema de arquivos já relativo (por exemplo, o
    /// resultado de `strip_prefix` da raiz).
    pub fn de_caminho(caminho: &Path) -> Result<Self, CaminhoInvalido> {
        let texto = caminho.to_string_lossy();
        let erro = |motivo| CaminhoInvalido {
            caminho: texto.to_string(),
            motivo,
        };
        let mut partes = Vec::new();
        for componente in caminho.components() {
            match componente {
                Component::Normal(p) => {
                    partes.push(p.to_str().ok_or_else(|| erro("nome que não é UTF-8"))?)
                }
                Component::CurDir => {}
                Component::ParentDir => {
                    return Err(erro("contém `..`; o caminho precisa ficar dentro da raiz"));
                }
                Component::RootDir | Component::Prefix(_) => {
                    return Err(erro("caminho absoluto; use um caminho relativo à raiz"));
                }
            }
        }
        Self::novo(&partes.join("/"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Componentes do caminho.
    pub fn partes(&self) -> impl Iterator<Item = &str> {
        self.0.split('/')
    }

    /// Último componente (nome do arquivo ou da pasta).
    pub fn nome(&self) -> &str {
        self.0.rsplit('/').next().unwrap_or(&self.0)
    }

    /// Pasta que contém o caminho, se houver.
    pub fn pasta(&self) -> Option<CaminhoRelativo> {
        self.0.rsplit_once('/').map(|(p, _)| Self(p.to_owned()))
    }

    /// Acrescenta um caminho relativo a este.
    pub fn juntar(&self, resto: &str) -> Result<Self, CaminhoInvalido> {
        let resto = Self::novo(resto)?;
        Ok(Self(format!("{}/{}", self.0, resto.0)))
    }

    /// Caminho no sistema de arquivos, abaixo de `raiz`.
    pub fn em(&self, raiz: &Path) -> PathBuf {
        let mut caminho = raiz.to_path_buf();
        caminho.extend(self.partes());
        caminho
    }
}

impl fmt::Display for CaminhoRelativo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for CaminhoRelativo {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl TryFrom<&str> for CaminhoRelativo {
    type Error = CaminhoInvalido;

    fn try_from(valor: &str) -> Result<Self, Self::Error> {
        Self::novo(valor)
    }
}

impl std::str::FromStr for CaminhoRelativo {
    type Err = CaminhoInvalido;

    fn from_str(valor: &str) -> Result<Self, Self::Err> {
        Self::novo(valor)
    }
}

impl Serialize for CaminhoRelativo {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for CaminhoRelativo {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let texto = String::deserialize(d)?;
        Self::novo(&texto).map_err(serde::de::Error::custom)
    }
}

/// Nome de arquivo ou pasta a partir de um texto: minúsculas sem acento, com `-`
/// no lugar de espaços e pontuação. `"Cobrança à Vista"` vira `"cobranca-a-vista"`.
pub fn slug(texto: &str) -> String {
    let mut saida = String::with_capacity(texto.len());
    let mut separar = false;
    for c in texto.chars().flat_map(char::to_lowercase).map(sem_acento) {
        if c.is_ascii_alphanumeric() {
            if separar && !saida.is_empty() {
                saida.push('-');
            }
            separar = false;
            saida.push(c);
        } else {
            separar = true;
        }
    }
    saida
}

/// Se o texto já é um slug: não vazio, só `a-z`, `0-9` e `-`, sem `-` nas pontas
/// nem repetido.
pub fn e_slug(texto: &str) -> bool {
    !texto.is_empty()
        && !texto.starts_with('-')
        && !texto.ends_with('-')
        && !texto.contains("--")
        && texto
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

pub(crate) fn sem_acento(c: char) -> char {
    match c {
        'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'í' | 'ì' | 'î' | 'ï' => 'i',
        'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
        'ú' | 'ù' | 'û' | 'ü' => 'u',
        'ç' => 'c',
        'ñ' => 'n',
        _ => c,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aceita_e_normaliza() {
        let c = CaminhoRelativo::novo("./db\\03_pkg_cobranca.sql").unwrap();
        assert_eq!(c.as_str(), "db/03_pkg_cobranca.sql");
        let c = CaminhoRelativo::novo("analises//A-0001.md").unwrap();
        assert_eq!(c.as_str(), "analises/A-0001.md");
        assert_eq!(c.nome(), "A-0001.md");
        assert_eq!(c.pasta().unwrap().as_str(), "analises");
        assert_eq!(
            CaminhoRelativo::novo("áreas/cobrança/memória.md")
                .unwrap()
                .as_str(),
            "áreas/cobrança/memória.md"
        );
    }

    #[test]
    fn recusa_absolutos_e_fuga_da_raiz() {
        for ruim in [
            "/etc/passwd",
            "\\Windows\\x",
            "C:\\Users\\x",
            "C:x",
            "\\\\servidor\\pasta",
            "//servidor/pasta",
            "../fora.md",
            "a/../../fora.md",
            "",
            ".",
            "a\0b",
        ] {
            assert!(CaminhoRelativo::novo(ruim).is_err(), "{ruim:?}");
        }
    }

    #[test]
    fn de_caminho_recusa_absoluto() {
        let absoluto = std::env::current_dir().unwrap();
        assert!(CaminhoRelativo::de_caminho(&absoluto).is_err());
        let relativo = Path::new("regras").join("RN-FAT-001.md");
        assert_eq!(
            CaminhoRelativo::de_caminho(&relativo).unwrap().as_str(),
            "regras/RN-FAT-001.md"
        );
    }

    #[test]
    fn em_monta_o_caminho_na_raiz() {
        let raiz = Path::new("base com espaço");
        let c = CaminhoRelativo::novo("regras/RN-FAT-001.md").unwrap();
        assert_eq!(
            c.em(raiz),
            raiz.join("regras").join("RN-FAT-001.md"),
            "usa o separador do sistema operacional"
        );
    }

    #[test]
    fn slug_sem_acentos() {
        assert_eq!(slug("Cobrança à Vista"), "cobranca-a-vista");
        assert_eq!(slug("  Nota Fiscal (NF-e)  "), "nota-fiscal-nf-e");
        assert_eq!(slug("Faturamento Exemplo"), "faturamento-exemplo");
        assert!(e_slug("faturamento-exemplo"));
        assert!(!e_slug("Faturamento"));
        assert!(!e_slug("_templates"));
        assert!(!e_slug("a--b"));
        assert!(!e_slug(""));
    }
}
