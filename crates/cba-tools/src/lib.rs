//! Ferramentas de análise e validação de evidências.
//!
//! As ferramentas que o agente de IA usa para analisar o sistema e registrar evidências,
//! com a validação de cada evidência citada (spec, seção 6.1).
//!
//! - [`Registro`] e [`Ferramenta`]: cada ferramenta declara nome, descrição, schema de
//!   entrada e o que executar. O registro confere os argumentos contra o schema
//!   ([`schema`]) e exige o ambiente do sistema antes de executar. O servidor MCP
//!   (`cba-mcp`) só traduz o protocolo para o registro.
//! - [`registro_padrao`]: as ferramentas que o produto expõe. As issues seguintes
//!   acrescentam as suas aqui (#20: `afirmar`, `perguntar`, `registrar_sugestao`,
//!   `propor_termo`; #21: `iniciar_analise`, `retomar_analise`, `concluir_analise`).
//!
//! Papel na arquitetura: seção 5 de `docs/specs/2026-11-11-mvp-standalone.md`.

pub mod analises;
mod registro;
pub mod schema;

pub use registro::{
    ErroChamada, ErroFerramenta, Executar, Ferramenta, Registro, Saida, ler_argumentos,
};

/// Nome da crate, para diagnóstico.
pub const NOME: &str = env!("CARGO_PKG_NAME");

/// As ferramentas do produto, na ordem em que aparecem para o agente.
pub fn registro_padrao() -> Registro {
    let mut registro = Registro::new();
    registro.registrar(analises::listar_analises());
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
        assert!(r.obter("listar_analises").is_some());
        for f in r.ferramentas() {
            assert!(!f.descricao().is_empty(), "{}", f.nome());
            assert_eq!(f.schema()["type"], "object");
        }
    }
}
