//! Ferramentas de análise e validação de evidências.
//!
//! As ferramentas que o agente de IA usa para analisar o projeto e registrar evidências,
//! com a validação de cada evidência citada (spec, seção 6.1).
//!
//! - [`Registro`] e [`Ferramenta`]: cada ferramenta declara nome, descrição, schema de
//!   entrada e o que executar. O registro confere os argumentos contra o schema
//!   ([`schema`]) e exige o ambiente do projeto antes de executar. O servidor MCP
//!   (`cba-mcp`) só traduz o protocolo para o registro.
//! - [`registro_padrao`]: as ferramentas que o produto expõe, cada uma no seu
//!   módulo: [`analises`] (`listar_analises`), [`ciclo`] (`iniciar_analise`,
//!   `retomar_analise` e `concluir_analise`), [`afirmar`], [`perguntar`],
//!   [`sugestao`] (`registrar_sugestao`), [`termo`] (`propor_termo`) e
//!   [`reuniao`] (`ler_transcript`, `propor_papel`, `registrar_item_ata`,
//!   `registrar_resumo_ata` e `sugerir_demanda`, do fluxo "Analisar reunião").
//! - Consulta, só de leitura e paginada ([`paginacao`]): [`conhecimento`] (busca no
//!   índice da base), [`mapa`], [`estrutura`], [`banco`] (`schema`) e
//!   [`referencias`], a partir do mapa estrutural do projeto ([`fontes`]).
//! - [`roteiro`] e [`contexto`]: o roteiro do fluxo e o contexto da base que
//!   `iniciar_analise` e `retomar_analise` devolvem ao agente.
//! - [`prompts`]: os prompts prontos da tela da análise (aprofundar, resumir,
//!   critérios de aceite, conferir evidências, retomar).
//! - [`editavel`]: roteiros e prompts editáveis na base (`_templates/roteiros/` e
//!   `_templates/prompts/`), com o padrão embutido.
//! - [`evidencias`]: a validação de cada evidência citada contra o código atual
//!   (spec, seção 6.5) e o hash do trecho.
//! - [`documento`]: o documento da análise remontado a partir do estado a cada
//!   registro.
//! - [`aprovacao`]: a aprovação da análise pelo analista, que leva à base as
//!   afirmações confirmadas (regras) e os termos aceitos (glossário).
//! - [`revisao`]: a revisão do analista (decisões, edição do texto, prontidão para
//!   aprovar, aprovação com a escolha da área e exportação).
//! - [`revalidacao`]: a revalidação das evidências das regras e das análises por
//!   hash, que o app chama ao abrir o projeto e sob demanda (spec, seção 6.5).
//!
//! Papel na arquitetura: seção 5 de `docs/specs/2026-11-11-mvp-standalone.md`.

pub mod afirmar;
pub mod analises;
pub mod aprovacao;
pub mod banco;
pub mod ciclo;
mod comum;
pub mod conhecimento;
pub mod contexto;
pub mod documento;
pub mod editavel;
pub mod estrutura;
pub mod evidencias;
mod fontes;
pub mod mapa;
mod paginacao;
pub mod perguntar;
pub mod prompts;
pub mod referencias;
mod registro;
pub mod reuniao;
pub mod revalidacao;
pub mod revisao;
pub mod roteiro;
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
        .registrar(ciclo::iniciar_analise())
        .registrar(ciclo::retomar_analise())
        .registrar(conhecimento::conhecimento())
        .registrar(referencias::referencias())
        .registrar(mapa::mapa())
        .registrar(estrutura::estrutura())
        .registrar(banco::schema())
        .registrar(afirmar::afirmar())
        .registrar(perguntar::perguntar())
        .registrar(sugestao::registrar_sugestao())
        .registrar(termo::propor_termo())
        .registrar(reuniao::ler_transcript())
        .registrar(reuniao::propor_papel())
        .registrar(reuniao::registrar_item_ata())
        .registrar(reuniao::registrar_resumo_ata())
        .registrar(reuniao::sugerir_demanda())
        .registrar(ciclo::concluir_analise());
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
            "iniciar_analise",
            "retomar_analise",
            "conhecimento",
            "referencias",
            "mapa",
            "estrutura",
            "schema",
            "afirmar",
            "perguntar",
            "registrar_sugestao",
            "propor_termo",
            "ler_transcript",
            "propor_papel",
            "registrar_item_ata",
            "registrar_resumo_ata",
            "sugerir_demanda",
            "concluir_analise",
        ] {
            assert!(r.obter(nome).is_some(), "{nome}");
        }
        // As de registro alteram a análise.
        assert!(!r.obter("afirmar").unwrap().e_somente_leitura());
        assert!(!r.obter("iniciar_analise").unwrap().e_somente_leitura());
        assert!(!r.obter("concluir_analise").unwrap().e_somente_leitura());
        assert!(!r.obter("propor_papel").unwrap().e_somente_leitura());
        assert!(!r.obter("registrar_item_ata").unwrap().e_somente_leitura());
        assert!(!r.obter("registrar_resumo_ata").unwrap().e_somente_leitura());
        for consulta in [
            "retomar_analise",
            "conhecimento",
            "referencias",
            "mapa",
            "estrutura",
            "schema",
            "ler_transcript",
            "sugerir_demanda",
        ] {
            assert!(r.obter(consulta).unwrap().e_somente_leitura(), "{consulta}");
        }
        for f in r.ferramentas() {
            assert!(!f.descricao().is_empty(), "{}", f.nome());
            assert_eq!(f.schema()["type"], "object");
        }
    }
}
