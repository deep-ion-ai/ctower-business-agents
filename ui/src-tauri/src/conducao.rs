//! Condução da análise pelo workflow do Claude Code na tela (spec
//! `docs/specs/2026-11-25-workflows-nas-analises.md`, seções 3.2, 3.6, 3.7 e 3.8;
//! #371).
//!
//! - **Nova análise:** [`conducao_disponivel`] diz se a opção Aprofundada (workflow)
//!   pode ser oferecida para o fluxo, com o motivo quando não pode (versão do
//!   `claude` anterior à mínima, workflows não instalados pelo preparo, Claude
//!   Desktop como cliente preferido), o número máximo de agentes e, depois da
//!   medição, a faixa de tokens. [`escolher_conducao`] grava a escolha na análise
//!   recém-criada e devolve o pedido que abre a conversa.
//! - **Pedidos:** [`prompt_de_inicio`], [`prompt_de_retomada`] (conversa nova),
//!   [`prompt_na_sessao`] (a sessão reaberta com `--resume` retoma o workflow pelo
//!   `runId`) e [`prompt_sem_workflow`] (o caminho de volta à condução simples),
//!   montados por [`cba_tools::conducao`].
//!
//! A versão efetiva do workflow (padrão, global ou do projeto) é a que o preparo
//! instalou: o app não a escolhe aqui (#372).

use cba_core::analise::{Analise, EstadoAnalise, Fluxo};
use cba_core::conducao::{Conducao, SituacaoWorkflow};
use cba_core::dados::DiretorioDados;
use cba_session::clientes::{self, AmbienteReal, VERSAO_MINIMA_WORKFLOWS};
use cba_session::preparo::Preparador;
use cba_tools::conducao as c;
use serde::Serialize;

use crate::analise;
use crate::primeiro_uso::{ClientePreferido, Preferencias};

/// Pedido que abre a conversa de uma análise criada pelo app.
pub fn prompt_de_inicio(a: &Analise) -> String {
    match workflow_da(a) {
        Some(nome) => c::pedido_de_inicio(&a.id, &nome),
        None => analise::prompt_de_inicio(&a.id),
    }
}

/// Pedido de uma conversa nova sobre a análise: com o workflow que não terminou, o
/// workflow roda de novo com `retomada` (spec, seção 3.7).
pub fn prompt_de_retomada(a: &Analise) -> String {
    match workflow_da(a).filter(|_| !workflow_concluido(a)) {
        Some(nome) => c::pedido_de_retomada(&a.id, &nome),
        None => analise::prompt_de_retomada(&a.id),
    }
}

/// Pedido para a sessão reaberta com `--resume` depois de interrompida no meio do
/// workflow: o mesmo `runId`, para o cache valer (R8). `None` quando não há
/// execução a retomar.
pub fn prompt_na_sessao(a: &Analise) -> Option<String> {
    let nome = workflow_da(a)?;
    let w = a.workflow.as_ref()?;
    let retomavel = matches!(
        w.situacao,
        SituacaoWorkflow::Rodando | SituacaoWorkflow::Interrompido
    );
    (retomavel && c::run_id_valido(&w.run_id) && a.estado != EstadoAnalise::Aprovada)
        .then(|| c::pedido_de_retomada_na_sessao(&a.id, &nome, &w.run_id))
}

/// Pedido para continuar pela condução simples, quando a análise é conduzida pelo
/// workflow e ele não está rodando.
pub fn prompt_sem_workflow(a: &Analise) -> Option<String> {
    let rodando = a.workflow.as_ref().is_some_and(|w| w.rodando());
    (a.conducao == Conducao::Workflow && !rodando && a.estado != EstadoAnalise::Aprovada)
        .then(|| c::pedido_sem_workflow(&a.id))
}

fn workflow_da(a: &Analise) -> Option<String> {
    (a.conducao == Conducao::Workflow)
        .then(|| c::workflow_do_fluxo(a.fluxo))
        .flatten()
}

fn workflow_concluido(a: &Analise) -> bool {
    a.workflow
        .as_ref()
        .is_some_and(|w| w.situacao == SituacaoWorkflow::Concluido)
}

/// O workflow da análise está rodando: o "Continuar no Desktop" fica bloqueado,
/// porque o `/desktop` encerra o CLI e a tarefa em segundo plano iria junto.
pub fn bloqueio_do_desktop(a: &Analise) -> Option<String> {
    a.workflow.as_ref().filter(|w| w.rodando()).map(|_| {
        "O workflow da análise está rodando no terminal: levar a conversa ao Desktop agora \
         interromperia o workflow. Espere ele terminar."
            .to_owned()
    })
}

/// Faixa de tokens da estimativa, depois da medição.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EstimativaVista {
    pub tokens_minimo: u64,
    pub tokens_maximo: u64,
    /// Razão em relação à condução simples do mesmo tipo ("cerca de 3× a simples").
    pub razao_simples: f64,
    pub minutos_minimo: u32,
    pub minutos_maximo: u32,
}

/// Se a condução pelo workflow pode ser oferecida para o fluxo (spec, seções 3.6 e
/// 3.8).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConducaoDisponivel {
    /// O fluxo tem workflow (só então a escolha aparece).
    pub tem_workflow: bool,
    /// `cba-<tipo>`.
    pub workflow: Option<String>,
    pub disponivel: bool,
    /// Por que não está disponível.
    pub motivo: Option<String>,
    /// O preparo do projeto não tem os workflows instalados: "Preparar de novo".
    pub preparar_de_novo: bool,
    /// Frentes de investigação (limite padrão).
    pub frentes: u32,
    /// Agentes no pior caso, com as frentes padrão.
    pub max_agentes: u32,
    /// Faixa de tokens e razão em relação à simples, depois da medição.
    pub estimativa: Option<EstimativaVista>,
}

/// A disponibilidade a partir do que se sabe (testável sem processo nem disco).
pub fn disponibilidade(
    fluxo: Fluxo,
    versao: Option<&str>,
    workflows_prontos: bool,
    desktop_preferido: bool,
) -> ConducaoDisponivel {
    let tipo = c::tipo_do_fluxo(fluxo);
    let frentes = c::FRENTES_PADRAO;
    let estimativa = tipo.and_then(c::estimativa).map(|e| {
        let (min, max) = e.faixa(frentes);
        EstimativaVista {
            tokens_minimo: min,
            tokens_maximo: max,
            razao_simples: e.razao_simples,
            minutos_minimo: e.minutos.0,
            minutos_maximo: e.minutos.1,
        }
    });
    let (m0, m1, m2) = VERSAO_MINIMA_WORKFLOWS;
    let motivo = if tipo.is_none() {
        Some("este tipo de análise ainda não tem workflow".to_owned())
    } else if let Some(v) = versao.filter(|v| clientes::anterior_a(v, VERSAO_MINIMA_WORKFLOWS)) {
        Some(format!(
            "o Claude Code {v} não roda os workflows das análises; atualize para a versão \
             {m0}.{m1}.{m2} ou mais nova"
        ))
    } else if !workflows_prontos {
        Some(
            "os workflows das análises não estão instalados neste projeto; prepare o \
             ambiente de novo (Configurações › Agente)"
                .to_owned(),
        )
    } else if desktop_preferido {
        Some(
            "a condução pelo workflow só foi verificada no terminal; o seu cliente \
             preferido é o Claude Desktop"
                .to_owned(),
        )
    } else {
        None
    };
    ConducaoDisponivel {
        tem_workflow: tipo.is_some(),
        workflow: tipo.map(cba_tools::workflows::nome),
        disponivel: motivo.is_none(),
        preparar_de_novo: tipo.is_some() && !workflows_prontos,
        motivo,
        frentes,
        max_agentes: c::agentes_no_maximo(frentes),
        estimativa,
    }
}

/// [`disponibilidade`] do projeto: a versão do `claude`, a verificação do preparo e
/// o cliente preferido. Bloqueia enquanto o `claude --version` roda.
pub fn conducao_disponivel(
    dados: &DiretorioDados,
    preparador: &Preparador,
    projeto: &str,
    fluxo: Fluxo,
) -> Result<ConducaoDisponivel, String> {
    if c::tipo_do_fluxo(fluxo).is_none() {
        return Ok(disponibilidade(fluxo, None, false, false));
    }
    let verificacao = preparador
        .verificar(&analise::alvo(dados, projeto)?)
        .map_err(|e| e.to_string())?;
    let versao = clientes::versao_claude_code(&AmbienteReal);
    let desktop = Preferencias::ler(dados)
        .ok()
        .and_then(|p| p.cliente)
        .is_some_and(|c| c == ClientePreferido::ClaudeDesktop);
    Ok(disponibilidade(
        fluxo,
        versao.as_deref(),
        verificacao.workflows_prontos,
        desktop,
    ))
}

/// Grava a condução escolhida em Nova análise numa análise ainda não aprovada e
/// devolve o pedido que abre a conversa.
pub fn escolher_conducao(
    dados: &DiretorioDados,
    projeto: &str,
    analise: &str,
    conducao: Conducao,
) -> Result<String, String> {
    let ambiente = analise::ambiente(dados, projeto)?;
    let id = cba_kb::Id::analisar(analise.trim()).map_err(|e| e.to_string())?;
    let (_, gravada) = ambiente
        .analises
        .alterar(&id, cba_core::eventos::Mudanca::Estado, |a, _| {
            if a.estado == EstadoAnalise::Aprovada {
                return Err(cba_core::analise::ErroModelo::AnaliseAprovada(a.id.clone()));
            }
            a.conducao = match conducao {
                Conducao::Workflow if c::tipo_do_fluxo(a.fluxo).is_some() => Conducao::Workflow,
                _ => Conducao::Simples,
            };
            Ok(())
        })
        .map_err(|e| e.to_string())?;
    Ok(prompt_de_inicio(&gravada))
}

/// Ao abrir uma análise conduzida pelo workflow, reinstala o workflow defasado
/// (padrão novo, cópia salva ou restaurada; spec, seção 3.3), sem novo
/// consentimento: só os itens que o preparo já instalou. Falhas só vão para o log:
/// a falta do workflow cai na condução simples.
pub fn reinstalar_se_preciso(dados: &DiretorioDados, projeto: &str, analise: &str) {
    let reinstalar = || -> Result<usize, String> {
        let vista = analise::carregar_analise(dados, projeto, analise)?;
        if vista.analise.conducao != Conducao::Workflow
            || vista.analise.estado == EstadoAnalise::Aprovada
        {
            return Ok(0);
        }
        let exe =
            std::env::current_exe().map_err(|e| format!("caminho do executável do app: {e}"))?;
        let preparador = Preparador::padrao(exe).map_err(|e| e.to_string())?;
        preparador
            .reinstalar_workflows(&analise::alvo(dados, projeto)?)
            .map(|a| a.len())
            .map_err(|e| e.to_string())
    };
    match reinstalar() {
        Ok(0) => {}
        Ok(n) => tracing::info!(arquivos = n, "workflow defasado reinstalado"),
        Err(e) => tracing::warn!(erro = %e, "workflow não reinstalado"),
    }
}

// ---------------------------------------------------------------------------
// Comandos

fn dados() -> Result<DiretorioDados, String> {
    DiretorioDados::padrao().map_err(|e| format!("diretório de dados do app inacessível: {e}"))
}

#[tauri::command]
pub async fn conducao_disponivel_projeto(
    projeto: String,
    fluxo: Fluxo,
) -> Result<ConducaoDisponivel, String> {
    let dados = dados()?;
    tauri::async_runtime::spawn_blocking(move || {
        let exe =
            std::env::current_exe().map_err(|e| format!("caminho do executável do app: {e}"))?;
        let preparador = Preparador::padrao(exe).map_err(|e| e.to_string())?;
        conducao_disponivel(&dados, &preparador, &projeto, fluxo)
    })
    .await
    .map_err(|e| format!("falha interna: {e}"))?
}

#[tauri::command]
pub async fn escolher_conducao_analise(
    projeto: String,
    analise: String,
    conducao: Conducao,
) -> Result<String, String> {
    let dados = dados()?;
    tauri::async_runtime::spawn_blocking(move || {
        escolher_conducao(&dados, &projeto, &analise, conducao)
    })
    .await
    .map_err(|e| format!("falha interna: {e}"))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use cba_core::conducao::ExecucaoWorkflow;
    use cba_kb::Serie;

    fn analise(conducao: Conducao) -> Analise {
        let mut a = Analise::nova(Serie::Analise.id(4), "p", Fluxo::Impacto, "t", "e", 1);
        a.conducao = conducao;
        a
    }

    fn execucao(situacao: SituacaoWorkflow) -> ExecucaoWorkflow {
        ExecucaoWorkflow {
            nome: "cba-impacto".into(),
            run_id: "wf_1-a".into(),
            sessao: "s".into(),
            situacao,
            ..Default::default()
        }
    }

    #[test]
    fn pedidos_por_conducao_e_situacao() {
        let simples = analise(Conducao::Simples);
        assert_eq!(
            prompt_de_inicio(&simples),
            analise::prompt_de_inicio(&simples.id)
        );
        assert_eq!(prompt_na_sessao(&simples), None);
        assert_eq!(prompt_sem_workflow(&simples), None);

        let mut w = analise(Conducao::Workflow);
        assert!(prompt_de_inicio(&w).contains("ferramenta Workflow com name cba-impacto"));
        assert!(prompt_de_retomada(&w).contains("args A-0004 retomada"));
        assert_eq!(prompt_na_sessao(&w), None);
        assert!(prompt_sem_workflow(&w).is_some());

        w.workflow = Some(execucao(SituacaoWorkflow::Rodando));
        assert!(
            prompt_na_sessao(&w)
                .unwrap()
                .contains("resumeFromRunId wf_1-a")
        );
        assert!(bloqueio_do_desktop(&w).is_some());
        assert_eq!(prompt_sem_workflow(&w), None);

        w.workflow = Some(execucao(SituacaoWorkflow::Interrompido));
        assert!(prompt_na_sessao(&w).is_some());
        assert_eq!(bloqueio_do_desktop(&w), None);

        w.workflow = Some(execucao(SituacaoWorkflow::Concluido));
        assert_eq!(prompt_na_sessao(&w), None);
        assert_eq!(prompt_de_retomada(&w), analise::prompt_de_retomada(&w.id));
    }

    #[test]
    fn disponibilidade_com_os_motivos() {
        let ok = disponibilidade(Fluxo::Impacto, Some("2.1.331"), true, false);
        assert!(ok.disponivel && ok.tem_workflow);
        assert_eq!(ok.max_agentes, 14);
        assert_eq!(ok.workflow.as_deref(), Some("cba-impacto"));

        let velha = disponibilidade(Fluxo::Impacto, Some("2.1.300"), true, false);
        assert!(!velha.disponivel);
        assert!(velha.motivo.unwrap().contains("2.1.331"));

        let sem = disponibilidade(Fluxo::Impacto, None, false, false);
        assert!(!sem.disponivel && sem.preparar_de_novo);

        let desktop = disponibilidade(Fluxo::Impacto, None, true, true);
        assert!(desktop.motivo.unwrap().contains("Desktop"));

        let pergunta = disponibilidade(Fluxo::Perguntar, Some("2.1.331"), true, false);
        assert!(!pergunta.tem_workflow && !pergunta.disponivel);
    }
}
