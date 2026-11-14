//! Comandos Tauri da tela inicial (spec, seção 10): os projetos cadastrados (com as
//! contagens de regras, termos e análises), as análises em andamento e as recentes
//! (com o estado da sessão vindo dos hooks) e a criação de uma análise de qualquer
//! fluxo pelos botões "perguntar", "explicar" e "impacto".

use cba_core::analise::{EstadoAnalise, Fluxo};
use cba_core::config::Config;
use cba_core::dados::DiretorioDados;
use cba_index::Cadastro;
use cba_kb::frontmatter::StatusRegra;
use serde::Serialize;

use crate::analise::{
    AnaliseCriada, AnaliseResumo, SessaoVista, ambiente, criar_analise_do_fluxo, sessao_recente,
};
use crate::projeto;

/// Análises recentes mostradas no início.
pub const RECENTES: usize = 10;

/// Projeto no início.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjetoInicio {
    pub id: String,
    pub nome: String,
    pub analises: usize,
    pub regras: usize,
    pub a_revalidar: usize,
    pub termos: usize,
    /// O projeto não pôde ser lido (pasta removida, base fora do lugar).
    pub erro: Option<String>,
}

/// Análise nas listas do início.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnaliseInicio {
    pub projeto: String,
    pub projeto_nome: String,
    #[serde(flatten)]
    pub analise: AnaliseResumo,
    /// Sessão mais recente ligada à análise (estado vindo dos hooks).
    pub sessao: Option<SessaoVista>,
}

/// O que a tela inicial mostra.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Inicio {
    pub base: Option<std::path::PathBuf>,
    pub projetos: Vec<ProjetoInicio>,
    /// Análises não aprovadas, da mais recente para a mais antiga.
    pub em_andamento: Vec<AnaliseInicio>,
    /// As [`RECENTES`] análises mais recentes, de todos os projetos.
    pub recentes: Vec<AnaliseInicio>,
}

/// Monta o início. Um projeto ilegível aparece com o erro e não esconde os outros.
pub fn inicio(dados: &DiretorioDados) -> Result<Inicio, String> {
    let config = Config::ler(dados).map_err(|e| format!("configuração do app ilegível: {e}"))?;
    let cadastrados = Cadastro::em(dados.projetos())
        .listar()
        .map_err(|e| e.to_string())?;
    let mut projetos = Vec::new();
    let mut todas = Vec::new();
    for s in cadastrados {
        let mut item = ProjetoInicio {
            id: s.id.clone(),
            nome: s.nome.clone(),
            analises: 0,
            regras: 0,
            a_revalidar: 0,
            termos: 0,
            erro: None,
        };
        if config.base.is_some() {
            let lido = (|| {
                let amb = ambiente(dados, &s.id)?;
                let analises = amb.analises.listar().map_err(|e| e.to_string())?;
                let visao = projeto::visao_projeto(dados, &s.id)?;
                Ok::<_, String>((analises, visao))
            })();
            match lido {
                Ok((analises, visao)) => {
                    item.analises = analises.len();
                    item.regras = visao
                        .regras
                        .iter()
                        .filter(|r| r.status != StatusRegra::Obsoleta)
                        .count();
                    item.a_revalidar = visao.a_revalidar();
                    item.termos = visao.termos.len();
                    for a in analises {
                        todas.push(AnaliseInicio {
                            projeto: s.id.clone(),
                            projeto_nome: s.nome.clone(),
                            sessao: sessao_recente(dados, &a),
                            analise: AnaliseResumo::from(&a),
                        });
                    }
                }
                Err(e) => item.erro = Some(e),
            }
        }
        projetos.push(item);
    }
    // Empate no instante (criadas no mesmo milissegundo): a de número maior antes.
    todas.sort_by_key(|a| std::cmp::Reverse((a.analise.atualizada_ms, a.analise.id.clone())));
    let em_andamento = todas
        .iter()
        .filter(|a| a.analise.estado != EstadoAnalise::Aprovada)
        .cloned()
        .collect();
    todas.truncate(RECENTES);
    Ok(Inicio {
        base: config.base,
        projetos,
        em_andamento,
        recentes: todas,
    })
}

// ---------------------------------------------------------------------------
// Comandos

fn dados() -> Result<DiretorioDados, String> {
    DiretorioDados::padrao().map_err(|e| format!("diretório de dados do app inacessível: {e}"))
}

async fn bloqueante<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| format!("falha interna: {e}"))?
}

#[tauri::command]
pub async fn inicio_app() -> Result<Inicio, String> {
    let dados = dados()?;
    bloqueante(move || inicio(&dados)).await
}

/// Cria a análise de um fluxo (perguntar, explicar, impacto) pelo início.
#[tauri::command]
pub async fn criar_analise_fluxo(
    projeto: String,
    fluxo: Fluxo,
    entrada: String,
    titulo: Option<String>,
) -> Result<AnaliseCriada, String> {
    let dados = dados()?;
    bloqueante(move || criar_analise_do_fluxo(&dados, &projeto, fluxo, &entrada, titulo.as_deref()))
        .await
}

#[cfg(test)]
#[path = "inicio_testes.rs"]
mod tests;
