//! Domínio, casos de uso e eventos do ctower-business-agents.
//!
//! É o centro do backend. Não depende do Tauri nem de UI, o que permite testes sem
//! interface e reaproveitar o núcleo fora do app.
//!
//! Papel na arquitetura: seção 5 de `docs/specs/2026-11-11-mvp-standalone.md`.

pub mod dados;
pub mod log;

/// Nome da crate, para diagnóstico.
pub const NOME: &str = env!("CARGO_PKG_NAME");

/// Versão do projeto: a do workspace, a mesma da tag `vX.Y.Z`.
pub const VERSAO: &str = env!("CARGO_PKG_VERSION");

/// Nome do produto, como aparece para o usuário.
pub const PRODUTO: &str = "Ctower Business Agents";

/// Identificador do app: o mesmo `identifier` do `ui/src-tauri/tauri.conf.json`.
/// Dá nome ao diretório de dados ([`dados`]), como o Tauri faz com o dele.
pub const IDENTIFICADOR: &str = "ai.deep-ion.ctower-business-agents";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nome_da_crate() {
        assert_eq!(NOME, "cba-core");
    }
}
