//! Workflows do Claude Code das análises (spec
//! `docs/specs/2026-11-25-workflows-nas-analises.md`, seção 3.1, épico #368).
//!
//! O padrão de cada tipo de análise é embutido no app
//! (`crates/cba-tools/textos/workflows/<tipo>.js`, com `include_str!`, como os
//! roteiros) e se chama `cba-<tipo>`, para não colidir com os workflows do projeto.
//! O preparo (`cba_session::preparo`) instala a versão **efetiva** de cada tipo
//! ([`WorkflowEfetivo`]) em `<raiz>/.claude/workflows/cba-<tipo>.js`.
//!
//! A versão efetiva vem de [`efetivos`]: a cópia do projeto em
//! `<base>/_templates/workflows/<tipo>.js`, a global em
//! `<dados do app>/workflows/<tipo>.js` ou o padrão, nessa precedência (#372,
//! [`copias`]). As cópias são editadas em Configurações › Workflows e validadas ao
//! salvar ([`validar`]).

use serde::Serialize;

/// Prefixo do nome dos workflows do app (`cba-impacto`).
pub const PREFIXO: &str = "cba-";

/// Tipos de análise com workflow padrão, na ordem em que aparecem.
pub const TIPOS: &[&str] = &["impacto", "revisar"];

/// Texto padrão embutido do workflow de `tipo`.
pub fn padrao(tipo: &str) -> Option<&'static str> {
    match tipo {
        "impacto" => Some(include_str!("../textos/workflows/impacto.js")),
        "revisar" => Some(include_str!("../textos/workflows/revisar.js")),
        _ => None,
    }
}

/// Nome do workflow de `tipo` (`cba-<tipo>`): o que vai em `meta.name`, no nome do
/// arquivo instalado e nas regras de permissão.
pub fn nome(tipo: &str) -> String {
    format!("{PREFIXO}{tipo}")
}

/// De onde veio a versão efetiva de um workflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum OrigemWorkflow {
    /// Padrão embutido no app.
    Padrao,
    /// Cópia para todos os projetos (`<dados do app>/workflows/<tipo>.js`).
    Global,
    /// Cópia do projeto (`<base>/_templates/workflows/<tipo>.js`).
    Projeto,
}

impl OrigemWorkflow {
    /// Nome curto, como vai no cabeçalho do arquivo instalado.
    pub fn nome(self) -> &'static str {
        match self {
            Self::Padrao => "padrao",
            Self::Global => "global",
            Self::Projeto => "projeto",
        }
    }

    /// Lê o nome curto de [`OrigemWorkflow::nome`].
    pub fn do_nome(nome: &str) -> Option<Self> {
        match nome {
            "padrao" => Some(Self::Padrao),
            "global" => Some(Self::Global),
            "projeto" => Some(Self::Projeto),
            _ => None,
        }
    }
}

/// Versão de um workflow que o preparo instala.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowEfetivo {
    /// Tipo de análise (`impacto`).
    pub tipo: String,
    pub origem: OrigemWorkflow,
    /// Conteúdo do script, sem o cabeçalho que o preparo acrescenta.
    #[serde(skip)]
    pub conteudo: String,
}

impl WorkflowEfetivo {
    /// O padrão embutido de `tipo`.
    pub fn padrao(tipo: &str) -> Option<Self> {
        padrao(tipo).map(|texto| Self {
            tipo: tipo.to_string(),
            origem: OrigemWorkflow::Padrao,
            conteudo: texto.to_string(),
        })
    }

    /// `cba-<tipo>`.
    pub fn nome(&self) -> String {
        nome(&self.tipo)
    }
}

/// Os padrões embutidos de todos os [`TIPOS`].
pub fn padroes() -> Vec<WorkflowEfetivo> {
    TIPOS
        .iter()
        .filter_map(|t| WorkflowEfetivo::padrao(t))
        .collect()
}

pub mod copias;
pub mod validacao;

pub use copias::{Copia, Resolucao, SituacaoCopia, efetivos, resolver};
pub use validacao::{Fase, Problema, Validacao, validar};

#[cfg(test)]
mod tests {
    use super::*;

    /// `export const meta = {...}` literal no início, com o nome certo e as fases da
    /// spec (seção 3.1).
    #[test]
    fn padroes_comecam_pelo_meta_com_as_fases() {
        for (tipo, fases) in [
            (
                "impacto",
                ["Preparar", "Investigar", "Verificar", "Consolidar"],
            ),
            ("revisar", ["Separar", "Avaliar", "Verificar", "Consolidar"]),
        ] {
            let texto = padrao(tipo).unwrap();
            assert!(texto.starts_with("export const meta = {\n"));
            let fim = texto.find("\n}\n").unwrap();
            let meta = &texto[..fim];
            assert!(meta.contains(&format!("name: 'cba-{tipo}'")), "{tipo}");
            assert!(meta.contains("description: '"));
            for fase in fases {
                assert!(meta.contains(&format!("title: '{fase}'")), "{tipo}: {fase}");
            }
            // Sem o que quebra a retomada e lendo os args (spec, seção 3.5).
            for proibido in ["Date.now()", "Math.random()", "new Date()"] {
                assert!(!texto.contains(proibido), "{tipo}: {proibido}");
            }
            assert!(texto.contains("typeof args === 'string'"), "{tipo}");
        }
    }

    /// O revisar registra pelas ferramentas da seção 3.1: achados, perguntas e
    /// afirmações na avaliação, a contestação na verificação e o INVEST e o veredito
    /// na consolidação.
    #[test]
    fn padrao_de_revisar_usa_as_ferramentas_do_parecer() {
        let texto = padrao("revisar").unwrap();
        for ferramenta in [
            "verificar_termos_vagos",
            "registrar_achado",
            "perguntar",
            "afirmar",
            "contestar_achado",
            "registrar_invest",
            "registrar_veredito",
        ] {
            assert!(texto.contains(ferramenta), "{ferramenta}");
        }
        assert!(!texto.contains("contestar_afirmacao"));
    }

    /// O impacto consulta a origem de cada regra na fase Investigar (#450): o
    /// histórico do trecho, a chave da demanda de origem em como_e_hoje e a
    /// pergunta se a regra continua valendo.
    #[test]
    fn padrao_de_impacto_consulta_a_origem_das_regras() {
        let texto = padrao("impacto").unwrap();
        let investigar = texto.find("Fase Investigar").unwrap();
        let verificar = texto.find("Fase Verificar").unwrap();
        let fase = &texto[investigar..verificar];
        assert!(fase.contains("historico_do_trecho"), "{fase}");
        assert!(fase.contains("continua valendo"), "{fase}");
        assert!(texto.contains("origens do afirmar"));
    }

    /// Numa demanda sobre um estado errado, o impacto fecha o conjunto de quem
    /// grava o estado (#451): uma frente própria no Preparar, a árvore de
    /// chamadores no Investigar e a lista conferida no Verificar.
    #[test]
    fn padrao_de_impacto_fecha_os_caminhos_que_gravam_o_estado() {
        let texto = padrao("impacto").unwrap();
        let preparar = texto.find("Fase Preparar").unwrap();
        let investigar = texto.find("Fase Investigar").unwrap();
        let verificar = texto.find("Fase Verificar").unwrap();
        let consolidar = texto.find("Fase Consolidar").unwrap();
        assert!(texto[preparar..investigar].contains("Caminhos que gravam <o estado>"));
        let fase = &texto[investigar..verificar];
        assert!(fase.contains("profundidade 2 ou 3"), "{fase}");
        assert!(fase.contains("\"Caminhos que gravam <estado>: "), "{fase}");
        assert!(fase.contains("o que valida e o que não valida"), "{fase}");
        let fase = &texto[verificar..consolidar];
        assert!(fase.contains("confira se a lista ficou fechada"), "{fase}");
    }

    #[test]
    fn padroes_tem_um_por_tipo_com_o_nome_do_app() {
        let todos = padroes();
        assert_eq!(todos.len(), TIPOS.len());
        assert_eq!(todos[0].nome(), "cba-impacto");
        assert_eq!(todos[1].nome(), "cba-revisar");
        assert_eq!(todos[0].origem, OrigemWorkflow::Padrao);
        assert!(padrao("perguntar").is_none());
        for o in [
            OrigemWorkflow::Padrao,
            OrigemWorkflow::Global,
            OrigemWorkflow::Projeto,
        ] {
            assert_eq!(OrigemWorkflow::do_nome(o.nome()), Some(o));
        }
    }
}
