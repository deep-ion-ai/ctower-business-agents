//! Sessões dos clientes de IA.
//!
//! Preparo do ambiente (MCP e hooks na configuração do cliente, [`preparo`]), links do Claude Desktop,
//! receptor dos hooks ([`hooks`]) e estado das sessões ([`sessoes`]), retomada de
//! sessões e detecção dos clientes de IA instalados e logados ([`clientes`]).
//!
//! Papel na arquitetura: seção 5 de `docs/specs/2026-11-11-mvp-standalone.md`.

pub mod clientes;
pub mod hooks;
pub mod preparo;
pub mod sessoes;

/// Nome da crate, para diagnóstico.
pub const NOME: &str = env!("CARGO_PKG_NAME");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nome_da_crate() {
        assert_eq!(NOME, "cba-session");
    }
}
