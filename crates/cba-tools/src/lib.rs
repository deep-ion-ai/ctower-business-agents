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
//!   [`sugestao`] (`registrar_sugestao`), [`termo`] (`propor_termo`),
//!   [`memoria`] (`memorizar`, a memória de trabalho que a retomada devolve),
//!   [`reuniao`] (`ler_transcript`, `propor_papel`, `registrar_item_ata`,
//!   `registrar_resumo_ata` e `sugerir_demanda`, do fluxo "Analisar reunião"),
//!   [`parecer`] (`verificar_termos_vagos`, `registrar_achado`, `registrar_invest`
//!   e `registrar_veredito`, do fluxo "Revisar requisito") e
//!   [`historias`] (`registrar_historia` e `registrar_criterio`, do fluxo
//!   "Histórias e critérios", com as referências conferidas por [`fundamentos`]) e
//!   [`cenarios`] (`registrar_tabela_decisao` e `registrar_cenario`, do fluxo
//!   "Cenários de teste") e [`diagramas`] (`registrar_diagrama`, os diagramas
//!   pelos elementos, que o app transforma em Mermaid, #118) e [`mapear`]
//!   (`iniciar_mapeamento`, `pontos_de_entrada`, `registrar_funcionalidade`,
//!   `tratar_ponto_de_entrada` e `concluir_mapeamento`: o mapa de funcionalidades
//!   do projeto, #208, fora das análises) e [`mudancas`]
//!   (`relatorio_de_mudancas` e `propor_atualizacao_de_regra`: o que mudou no
//!   código desde a última análise e a explicação por regra, #124).
//! - Consulta, só de leitura e paginada ([`paginacao`]): [`conhecimento`] (busca no
//!   índice da base), [`mapa`], [`estrutura`], [`banco`] (`schema`, com os indícios
//!   de dado pessoal de [`dados_pessoais`]) e
//!   [`referencias`], a partir do mapa estrutural do projeto ([`fontes`]), e
//!   [`rastreabilidade`] (matriz da demanda e consulta reversa por regra ou
//!   arquivo, #121, que também exporta a matriz para Excel e Word).
//! - [`roteiro`] e [`contexto`]: o roteiro do fluxo e o contexto da base que
//!   `iniciar_analise` e `retomar_analise` devolvem ao agente.
//! - [`prompts`]: os prompts prontos da tela da análise (aprofundar, resumir,
//!   gerar histórias, conferir evidências, retomar).
//! - [`editavel`]: roteiros e prompts editáveis na base (`_templates/roteiros/` e
//!   `_templates/prompts/`), com o padrão embutido.
//! - [`evidencias`]: a validação de cada evidência citada contra o código atual
//!   (spec, seção 6.5) e o hash do trecho.
//! - [`documento`]: o documento da análise remontado a partir do estado a cada
//!   registro.
//! - [`aprovacao`]: a aprovação da análise pelo analista, que leva à base as
//!   afirmações confirmadas (regras) e os termos aceitos (glossário).
//! - [`vinculo`]: o vínculo da análise, das regras e dos termos com as
//!   funcionalidades do mapa (#287): a proposta na aprovação (sessão ou
//!   evidências) e a religação dos documentos aprovados antes.
//! - [`aprovacao_ata`]: a aprovação da ata de uma reunião (fluxo `reuniao`), com o
//!   segundo aviso e o descarte do transcript.
//! - [`revisao`]: a revisão do analista (decisões, edição do texto, prontidão para
//!   aprovar, aprovação com a escolha da área e exportação).
//! - [`docs_externos`]: a sincronização do repositório de docs (#277), que traz
//!   a documentação publicada para `externos/` como fonte externa rotulada; roda
//!   sob demanda (app) e em segundo plano ao iniciar uma análise.
//! - [`revalidacao`]: a revalidação das evidências das regras e das análises por
//!   hash, que o app chama ao abrir o projeto e sob demanda (spec, seção 6.5).
//! - [`publicacao`]: a publicação dos documentos aprovados no repositório da
//!   documentação, com a prévia das mudanças e o PR/MR (#276).
//!
//! Papel na arquitetura: seção 5 de `docs/specs/2026-11-11-mvp-standalone.md`.

pub mod afirmar;
pub mod analises;
pub mod aprovacao;
pub mod aprovacao_ata;
pub mod banco;
pub mod cenarios;
pub mod ciclo;
pub mod cobertura;
mod comum;
pub mod conhecimento;
pub mod contexto;
pub mod dados_pessoais;
pub mod diagramas;
pub mod docs_externos;
pub mod documentacao;
pub mod documento;
pub mod editavel;
pub mod estrutura;
pub mod evidencias;
mod fontes;
pub mod fundamentos;
pub mod historias;
pub mod mapa;
pub mod mapear;
pub mod memoria;
mod modelo_dados;
pub mod mudancas;
mod paginacao;
pub mod parecer;
pub mod pauta;
pub mod perguntar;
pub mod prompts;
pub mod publicacao;
pub mod rastreabilidade;
pub mod referencias;
mod registro;
pub mod registros;
pub mod reuniao;
pub mod revalidacao;
pub mod revisao;
pub mod roteiro;
pub mod schema;
pub mod sugestao;
pub mod termo;
pub mod vincular;
pub mod vinculo;

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
        .registrar(rastreabilidade::rastreabilidade())
        .registrar(mapa::mapa())
        .registrar(estrutura::estrutura())
        .registrar(banco::schema())
        .registrar(afirmar::afirmar())
        .registrar(perguntar::perguntar())
        .registrar(sugestao::registrar_sugestao())
        .registrar(termo::propor_termo())
        .registrar(memoria::memorizar())
        .registrar(reuniao::ler_transcript())
        .registrar(reuniao::propor_papel())
        .registrar(reuniao::registrar_item_ata())
        .registrar(reuniao::registrar_resumo_ata())
        .registrar(reuniao::sugerir_demanda())
        .registrar(parecer::verificar_termos_vagos())
        .registrar(parecer::registrar_achado())
        .registrar(parecer::registrar_invest())
        .registrar(parecer::registrar_veredito())
        .registrar(historias::registrar_historia())
        .registrar(historias::registrar_criterio())
        .registrar(cenarios::registrar_tabela_decisao())
        .registrar(cenarios::registrar_cenario())
        .registrar(diagramas::registrar_diagrama())
        .registrar(mapear::iniciar_mapeamento())
        .registrar(mapear::pontos_de_entrada())
        .registrar(mapear::registrar_funcionalidade())
        .registrar(mapear::tratar_ponto_de_entrada())
        .registrar(mapear::propor_vinculo_de_issue())
        .registrar(mapear::concluir_mapeamento())
        .registrar(mudancas::relatorio_de_mudancas())
        .registrar(mudancas::propor_atualizacao_de_regra())
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
            "rastreabilidade",
            "mapa",
            "estrutura",
            "schema",
            "afirmar",
            "perguntar",
            "registrar_sugestao",
            "propor_termo",
            "memorizar",
            "ler_transcript",
            "propor_papel",
            "registrar_item_ata",
            "registrar_resumo_ata",
            "sugerir_demanda",
            "verificar_termos_vagos",
            "registrar_achado",
            "registrar_invest",
            "registrar_veredito",
            "registrar_historia",
            "registrar_criterio",
            "registrar_tabela_decisao",
            "registrar_cenario",
            "registrar_diagrama",
            "iniciar_mapeamento",
            "pontos_de_entrada",
            "registrar_funcionalidade",
            "tratar_ponto_de_entrada",
            "propor_vinculo_de_issue",
            "concluir_mapeamento",
            "relatorio_de_mudancas",
            "propor_atualizacao_de_regra",
            "concluir_analise",
        ] {
            assert!(r.obter(nome).is_some(), "{nome}");
        }
        // As de registro alteram a análise.
        assert!(!r.obter("afirmar").unwrap().e_somente_leitura());
        assert!(!r.obter("memorizar").unwrap().e_somente_leitura());
        assert!(!r.obter("registrar_historia").unwrap().e_somente_leitura());
        assert!(!r.obter("registrar_criterio").unwrap().e_somente_leitura());
        assert!(!r.obter("registrar_cenario").unwrap().e_somente_leitura());
        assert!(
            !r.obter("registrar_tabela_decisao")
                .unwrap()
                .e_somente_leitura()
        );
        assert!(!r.obter("registrar_diagrama").unwrap().e_somente_leitura());
        assert!(
            !r.obter("registrar_funcionalidade")
                .unwrap()
                .e_somente_leitura()
        );
        assert!(
            !r.obter("tratar_ponto_de_entrada")
                .unwrap()
                .e_somente_leitura()
        );
        assert!(
            !r.obter("propor_vinculo_de_issue")
                .unwrap()
                .e_somente_leitura()
        );
        assert!(
            !r.obter("propor_atualizacao_de_regra")
                .unwrap()
                .e_somente_leitura()
        );
        assert!(!r.obter("iniciar_analise").unwrap().e_somente_leitura());
        assert!(!r.obter("concluir_analise").unwrap().e_somente_leitura());
        assert!(!r.obter("propor_papel").unwrap().e_somente_leitura());
        assert!(!r.obter("registrar_item_ata").unwrap().e_somente_leitura());
        assert!(!r.obter("registrar_resumo_ata").unwrap().e_somente_leitura());
        assert!(!r.obter("registrar_achado").unwrap().e_somente_leitura());
        assert!(!r.obter("registrar_invest").unwrap().e_somente_leitura());
        assert!(!r.obter("registrar_veredito").unwrap().e_somente_leitura());
        for consulta in [
            "retomar_analise",
            "conhecimento",
            "referencias",
            "rastreabilidade",
            "mapa",
            "estrutura",
            "schema",
            "ler_transcript",
            "sugerir_demanda",
            "verificar_termos_vagos",
            "iniciar_mapeamento",
            "pontos_de_entrada",
            "concluir_mapeamento",
            "relatorio_de_mudancas",
        ] {
            assert!(r.obter(consulta).unwrap().e_somente_leitura(), "{consulta}");
        }
        for f in r.ferramentas() {
            assert!(!f.descricao().is_empty(), "{}", f.nome());
            assert_eq!(f.schema()["type"], "object");
        }
    }
}
