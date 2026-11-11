//! Ferramentas de análise e validação de evidências.
//!
//! As ferramentas que o agente de IA usa para analisar o sistema e registrar evidências,
//! com a validação de cada evidência citada (spec, seção 6.1).
//!
//! - [`Registro`] e [`Ferramenta`]: cada ferramenta declara nome, descrição, schema de
//!   entrada e o que executar. O registro confere os argumentos contra o schema
//!   ([`schema`]) e exige o ambiente do sistema antes de executar. O servidor MCP
//!   (`cba-mcp`) só traduz o protocolo para o registro.
//! - [`registro_padrao`]: as ferramentas que o produto expõe, cada uma no seu
//!   módulo: [`analises`] (`listar_analises`), [`afirmar`], [`perguntar`],
//!   [`sugestao`] (`registrar_sugestao`) e [`termo`] (`propor_termo`).
//! - [`evidencias`]: a validação de cada evidência citada contra o código atual
//!   (spec, seção 6.5) e o hash do trecho.
//! - [`documento`]: o documento da análise remontado a partir do estado a cada
//!   registro.
//!
//! Papel na arquitetura: seção 5 de `docs/specs/2026-11-11-mvp-standalone.md`.

pub mod afirmar;
pub mod analises;
mod comum;
pub mod documento;
pub mod evidencias;
pub mod perguntar;
mod registro;
pub mod schema;
pub mod sugestao;
pub mod termo;

pub use registro::{
    ErroChamada, ErroFerramenta, Executar, Ferramenta, Registro, Saida, ler_argumentos,
};

/// Nome da crate, para diagnóstico.
pub const NOME: &str = env!("CARGO_PKG_NAME");

/// As ferramentas do produto, na ordem em que aparecem para o agente.
pub fn registro_padrao() -> Registro {
    let mut registro = Registro::new();
    registro
        .registrar(analises::listar_analises())
        .registrar(afirmar::afirmar())
        .registrar(perguntar::perguntar())
        .registrar(sugestao::registrar_sugestao())
        .registrar(termo::propor_termo());
    registro
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nome_da_crate() {
        assert_eq!(NOME, "cba-tools");
    }

    #[test]
    fn registro_padrao_tem_nomes_unicos_e_schemas_de_objeto() {
        let r = registro_padrao();
        for nome in [
            "listar_analises",
            "afirmar",
            "perguntar",
            "registrar_sugestao",
            "propor_termo",
        ] {
            assert!(r.obter(nome).is_some(), "{nome}");
        }
        // As de registro alteram a análise.
        assert!(!r.obter("afirmar").unwrap().e_somente_leitura());
        for f in r.ferramentas() {
            assert!(!f.descricao().is_empty(), "{}", f.nome());
            assert_eq!(f.schema()["type"], "object");
        }
    }
}
