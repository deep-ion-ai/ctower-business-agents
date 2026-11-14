//! Avaliação dos fluxos (perguntar, explicar, impacto) contra o gabarito da fixture
//! (spec, critério de aceite 2; issue #33).
//!
//! Dois usos:
//!
//! - **Comparação** ([`comparacao`]): lê o estado de uma análise
//!   (`<base>/<projeto>/analises/A-NNNN.estado.json`) e mede, contra o gabarito, as
//!   regras encontradas, as evidências válidas, as afirmações `confirmado` sem
//!   evidência válida, as perguntas esperadas, a premissa corrigida e a abrangência.
//!   É determinística e testada no CI comum, com estados de exemplo.
//! - **Execução** ([`execucao`]): roda cada fluxo com o Claude Code real numa cópia
//!   da fixture, isolada da configuração do usuário, e compara o resultado. Usa a
//!   assinatura de quem roda e fica fora do CI comum (`docs/avaliacao.md`).
//!
//! O relatório ([`relatorio`]) sai em JSON e Markdown.

pub mod comparacao;
pub mod execucao;
pub mod gabarito;
pub mod pastas;
pub mod relatorio;
pub mod texto;

use std::path::PathBuf;

/// Raiz do repositório (a crate fica em `crates/cba-avaliacao`).
pub fn raiz_do_repositorio() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

/// Gabarito padrão: o do sistema fictício Faturamento Exemplo.
pub fn gabarito_padrao() -> PathBuf {
    raiz_do_repositorio()
        .join("fixtures")
        .join("gabaritos")
        .join("faturamento-exemplo.yaml")
}
