//! Sessões dos clientes de IA.
//!
//! Preparo do ambiente (MCP e hooks na configuração do cliente, [`preparo`]), links do
//! Claude Desktop e do terminal do analista e a passagem entre clientes
//! ([`passagem`]), receptor dos hooks ([`hooks`]) e estado das sessões ([`sessoes`]),
//! retomada de sessões, detecção dos clientes de IA instalados e logados ([`clientes`]),
//! o terminal embutido com o Claude Code ([`terminal`]), o aviso de alteração na pasta
//! do projeto ([`alteracoes`]) e a proteção opcional contra edição ([`protecao`]).
//!
//! Papel na arquitetura: seção 5 de `docs/specs/2026-11-11-mvp-standalone.md`.

pub mod alteracoes;
pub mod clientes;
pub mod hooks;
pub mod passagem;
pub mod preparo;
pub mod protecao;
pub mod sessoes;
pub mod terminal;

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
