//! Servidor MCP local.
//!
//! Expõe as ferramentas de `cba-tools` aos clientes de IA (Claude Code e Claude Desktop) por
//! MCP sobre stdio. É o centro do produto: todos os clientes escrevem na mesma análise.
//!
//! Papel na arquitetura: seção 5 de `docs/specs/2026-11-11-mvp-standalone.md`.

/// Nome da crate, para diagnóstico.
pub const NOME: &str = env!("CARGO_PKG_NAME");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nome_da_crate() {
        assert_eq!(NOME, "cba-mcp");
    }
}
