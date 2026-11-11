//! Domínio, casos de uso e eventos do ctower-business-agents.
//!
//! É o centro do backend. Não depende do Tauri nem de UI, o que permite testes sem
//! interface e reaproveitar o núcleo fora do app.
//!
//! Papel na arquitetura: seção 5 de `docs/specs/2026-11-11-mvp-standalone.md`.

/// Nome da crate, para diagnóstico.
pub const NOME: &str = env!("CARGO_PKG_NAME");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nome_da_crate() {
        assert_eq!(NOME, "cba-core");
    }
}
