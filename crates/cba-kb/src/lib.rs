//! Base de conhecimento do sistema analisado.
//!
//! A base é uma pasta de Markdown escolhida pelo analista (spec, seções 7.1 e 7.2):
//!
//! ```text
//! <base>/
//!   _templates/
//!   <sistema>/
//!     sistema.md
//!     glossario/<termo>.md
//!     regras/RN-<AREA>-<NNN>.md
//!     areas/<area>/memoria.md
//!     funcionalidades/<area>/<tema>/
//!     analises/<id>.md
//!     sugestoes/<id>.md
//! ```
//!
//! O que esta crate oferece:
//!
//! - [`Base`] e [`Sistema`]: criação da estrutura, leitura e gravação de documentos
//!   por caminho relativo ([`CaminhoRelativo`], que recusa caminhos absolutos e `..`).
//! - [`Documento`]: Markdown com frontmatter YAML tipado ([`Frontmatter`]). Ler e
//!   gravar de novo preserva o texto; ao alterar campos, a ordem original e os
//!   campos desconhecidos são mantidos.
//! - Gravação atômica ([`arquivo`]): arquivo temporário na mesma pasta e renomeação.
//! - Identificadores legíveis ([`Id`], [`Serie`]): `A-0001`, `S-0001`, `RN-FAT-001`,
//!   reservados sem colisão por [`Sistema::criar_numerado`], mesmo com dois
//!   processos (app e servidor MCP) gravando ao mesmo tempo.
//!
//! Papel na arquitetura: seção 5 de `docs/specs/2026-11-11-mvp-standalone.md`.

pub mod arquivo;
mod base;
mod caminho;
mod documento;
mod erro;
pub mod frontmatter;
mod id;
pub mod layout;

pub use base::{Base, Sistema};
pub use caminho::{CaminhoInvalido, CaminhoRelativo, e_slug, slug};
pub use documento::{Documento, ErroFrontmatter};
pub use erro::{Erro, Resultado};
pub use frontmatter::{ErroCampo, Frontmatter, TipoDocumento};
pub use id::{Id, IdInvalido, Serie};

/// Tipos YAML do frontmatter livre ([`frontmatter::FrontmatterLivre`]).
pub mod yaml {
    pub use serde_norway::{Mapping, Number, Value};
}

/// Nome da crate, para diagnóstico.
pub const NOME: &str = env!("CARGO_PKG_NAME");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nome_da_crate() {
        assert_eq!(NOME, "cba-kb");
    }
}
