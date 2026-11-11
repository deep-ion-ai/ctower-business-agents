//! Ferramentas de análise e validação de evidências.
//!
//! As ferramentas que o agente de IA usa para analisar o sistema e registrar evidências,
//! com a validação de cada evidência citada.
//!
//! Papel na arquitetura: seção 5 de `docs/specs/2026-11-11-mvp-standalone.md`.

/// Nome da crate, para diagnóstico.
pub const NOME: &str = env!("CARGO_PKG_NAME");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nome_da_crate() {
        assert_eq!(NOME, "cba-tools");
    }
}
