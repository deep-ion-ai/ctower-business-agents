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
//! - [`memoria`]: a memória de trabalho do agente em cada análise, que a retomada
//!   devolve (#143).
//! - [`diagramas`]: diagramas registrados pelo agente (nós e arestas com
//!   evidências), que o app transforma em Mermaid na montagem (#118).
//! - [`funcionalidades`]: o mapa de funcionalidades do projeto (#208), um
//!   cadastro do projeto fora das análises, com identificadores estáveis.
//! - [`manual`]: passo a passo do manual do usuário de uma funcionalidade, com
//!   os prints de tela (fluxo `manual`, #272).
//! - [`prototipo`]: protótipo de telas a partir de uma análise de origem
//!   validada, com as telas afetadas e os elementos rastreados (fluxo
//!   `prototipo`, #381).
//! - [`prototipo_html`]: o protótipo HTML de cada tela (#385): conferência do
//!   corpo escrito pelo agente (estados, elementos, nada que execute ou vá à
//!   rede) e montagem do documento com os tokens aprovados ou o wireframe neutro.
//! - [`parecer`]: parecer de revisão de um requisito ou história (fluxo
//!   `revisar`): achados, verificação INVEST e veredito.
//! - [`qualidade`]: critérios de qualidade de requisitos (características da
//!   ISO/IEC/IEEE 29148, INVEST e a lista de referência de termos vagos).
//! - [`conducao`]: condução da análise (simples ou por workflow do Claude Code), o
//!   estado da execução do workflow e a contestação da verificação independente
//!   (#371).
//! - [`tokens`]: os tokens CSS do projeto analisado (#384), extraídos do código
//!   da interface e revisados pelo analista antes de o protótipo de telas usar.
//! - [`ambiente`]: identificação do projeto pelo diretório de trabalho do processo.
//! - [`config`], [`dados`] e [`log`]: configuração, diretório de dados e log.
//!
//! Dependências: `cba-kb` (base de conhecimento) e `cba-index` (cadastro e escopo do
//! projeto). As crates de cima (`cba-tools`, `cba-mcp`, o app) usam este núcleo.

pub mod ambiente;
pub mod analise;
pub mod analises;
pub mod banco;
pub mod cenarios;
pub mod conducao;
pub mod config;
pub mod consultas;
pub mod dados;
pub mod diagramas;
pub mod eventos;
pub mod funcionalidades;
pub mod historias;
pub mod historico;
pub mod log;
pub mod manual;
pub mod memoria;
pub mod parecer;
pub mod prototipo;
pub mod prototipo_html;
pub mod qualidade;
pub mod requisito;
pub mod tempo;
pub mod tokens;
pub mod vinculos;

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
