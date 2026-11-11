//! Sessões dos clientes de IA.
//!
//! Preparo do ambiente (MCP e hooks na configuração do cliente), links do Claude Desktop,
//! receptor dos hooks e retomada de sessões.
//!
//! Papel na arquitetura: seção 5 de `docs/specs/2026-11-11-mvp-standalone.md`.

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
