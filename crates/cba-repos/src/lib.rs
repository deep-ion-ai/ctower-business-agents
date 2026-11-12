//! Repositório de código (forja) e de issues (rastreador) de um projeto.
//!
//! Modelo, validação, execução segura do comando de credencial, prontidão das
//! ferramentas de linha de comando (`gh`, `glab`, `jira`) e busca de uma issue.
//! Sem Tauri e sem HTTP próprio: o acesso é pelas CLIs.
//!
//! Origem: copiado do Control Tower (`deep-ion-ai/ctower-agents`, crates
//! `ct-core`, `ct-proc` e `ct-ledger`), com os mesmos tipos, nomes de campo e
//! regras de validação do Jira. Diferenças deste repositório: o GitLab também
//! é rastreador (GitLab Issues) e o GitHub guarda o `owner/repo`, porque aqui
//! o projeto não é o repositório.

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("valor inválido para {kind}: {value}")]
pub struct ParseEnumError {
    pub kind: &'static str,
    pub value: String,
}

/// Enum persistido como string. Mantém `as_str`, `Display`, `FromStr` e serde alinhados.
macro_rules! str_enum {
    ($name:ident, $kind:literal, { $($variant:ident => $s:literal),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub enum $name {
            $( #[serde(rename = $s)] $variant ),+
        }

        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$variant),+];

            pub fn as_str(self) -> &'static str {
                match self { $($name::$variant => $s),+ }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl FromStr for $name {
            type Err = ParseEnumError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $($s => Ok($name::$variant),)+
                    _ => Err(ParseEnumError { kind: $kind, value: s.to_string() }),
                }
            }
        }
    };
}

mod credential;
pub mod demanda;
pub mod link;
pub mod process;
pub mod prontidao;
mod secret;
pub mod tracker;

pub use credential::{CREDENTIAL_DEADLINE, CredentialError, run_credential};
pub use secret::Secret;
pub use tracker::*;
