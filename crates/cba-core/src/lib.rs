//! Domínio, casos de uso e eventos do ctower-business-agents.
//!
//! É o centro do backend. Não depende do Tauri nem de UI, o que permite testes sem
//! interface e reaproveitar o núcleo fora do app.
//!
//! Papel na arquitetura: seção 5 de `docs/specs/2026-11-11-mvp-standalone.md`.
//!
//! - [`analise`]: modelo da análise (análise, sessão, afirmação, evidência, pergunta,
//!   sugestão) e as transições de estado.
//! - [`analises`]: persistência na base, com trava entre processos e notificação ao
//!   app ([`eventos`]).
//! - [`parecer`]: parecer de revisão de um requisito ou história (fluxo
//!   `revisar`): achados, verificação INVEST e veredito.
//! - [`qualidade`]: critérios de qualidade de requisitos (características da
//!   ISO/IEC/IEEE 29148, INVEST e a lista de referência de termos vagos).
//! - [`ambiente`]: identificação do projeto pelo diretório de trabalho do processo.
//! - [`config`], [`dados`] e [`log`]: configuração, diretório de dados e log.
//!
//! Dependências: `cba-kb` (base de conhecimento) e `cba-index` (cadastro e escopo do
//! projeto). As crates de cima (`cba-tools`, `cba-mcp`, o app) usam este núcleo.

pub mod ambiente;
pub mod analise;
pub mod analises;
pub mod config;
pub mod dados;
pub mod eventos;
pub mod log;
pub mod parecer;
pub mod qualidade;
pub mod tempo;

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
