//! Base de conhecimento do sistema analisado.
//!
//! A base é uma pasta de Markdown escolhida pelo analista (spec, seções 7.1 e 7.2):
//!
//! ```text
//! <base>/
//!   _templates/
//!     papeis.yaml                  # papéis das pessoas, editável
//!   <projeto>/
//!     projeto.md
//!     regras-do-projeto.md         # regras do projeto para o agente (#31)
//!     glossario/<termo>.md
//!     regras/RN-<AREA>-<NNN>.md
//!     areas/<area>/memoria.md
//!     funcionalidades/<area>/<tema>/
//!     analises/<id>.md
//!     sugestoes/<id>.md
//!     demandas/D-NNNN-<slug>/demanda.md
//!     pessoas/<slug>.md            # catálogo de pessoas: nome, apelidos e papéis
//!     demandas/D-NNNN-<slug>/reunioes/R-NNNN.md            # ata
//!     demandas/D-NNNN-<slug>/reunioes/R-NNNN.estado.json   # estado da ata
//! ```
//!
//! O que esta crate oferece:
//!
//! - [`Base`] e [`Projeto`]: criação da estrutura, leitura e gravação de documentos
//!   por caminho relativo ([`CaminhoRelativo`], que recusa caminhos absolutos e `..`).
//! - [`Documento`]: Markdown com frontmatter YAML tipado ([`Frontmatter`]). Ler e
//!   gravar de novo preserva o texto; ao alterar campos, a ordem original e os
//!   campos desconhecidos são mantidos.
//! - Gravação atômica ([`arquivo`]): arquivo temporário na mesma pasta e renomeação.
//! - Identificadores legíveis ([`Id`], [`Serie`]): `A-0001`, `S-0001`, `RN-FAT-001`,
//!   reservados sem colisão por [`Projeto::criar_numerado`], mesmo com dois
//!   processos (app e servidor MCP) gravando ao mesmo tempo.
//!
//! - Catálogo de pessoas por projeto ([`PessoaLida`], [`CatalogoDePessoas`],
//!   [`normalizar_nome`]), com os papéis sugeridos de `_templates/papeis.yaml`
//!   ([`Papeis`]). Nunca guarda falas.
//!
//! - Índice SQLite com busca FTS5 ([`indice`]): espelho reconstruível da base, no
//!   diretório de dados do app.
//!
//! Papel na arquitetura: seção 5 de `docs/specs/2026-11-11-mvp-standalone.md`.

pub mod arquivo;
mod base;
mod caminho;
mod demandas;
mod documento;
mod erro;
pub mod frontmatter;
mod id;
pub mod indice;
pub mod layout;
mod papeis;
mod pessoas;
mod reunioes;

pub use base::{Base, PADRAO_REGRAS_DO_PROJETO, Projeto};
pub use caminho::{CaminhoInvalido, CaminhoRelativo, e_slug, slug};
pub use demandas::{DemandaLida, NovaDemanda};
pub use documento::{Documento, ErroFrontmatter};
pub use erro::{Erro, Resultado};
pub use frontmatter::{ErroCampo, Frontmatter, TipoDocumento};
pub use id::{Id, IdDemanda, IdHistoria, IdInvalido, IdReuniao, Serie};
pub use papeis::{AvisoPapeis, OrigemPapeis, PADRAO_PAPEIS, Papeis, Papel};
pub use pessoas::{
    Casamento, CatalogoDePessoas, NovaPessoa, PapelSugerido, Participante, PessoaLida,
    normalizar_nome, normalizar_texto, sem_dominio,
};
pub use reunioes::ArquivosDaReuniao;

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
