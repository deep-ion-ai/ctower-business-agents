//! Condução da análise: simples (a sessão segue o roteiro) ou por um workflow do
//! Claude Code, e a contestação da verificação independente (spec
//! `docs/specs/2026-11-25-workflows-nas-analises.md`, seções 3.1 e 3.4; #371).
//!
//! O workflow é outra forma de conduzir, não outro tipo de análise: o documento, a
//! revisão e as ferramentas de registro são os mesmos. A análise guarda só a condução
//! escolhida ([`Conducao`]), o estado da execução ([`ExecucaoWorkflow`], gravado pelo
//! receptor de hooks) e, quando a sessão volta à condução simples, o motivo.
//!
//! A verificação independente do workflow não apaga nada: ela contesta a afirmação
//! (ou o achado) com uma [`Contestacao`], e quem decide é o analista.

use serde::{Deserialize, Serialize};

use crate::analise::Evidencia;

/// Como a sessão conduz a análise.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Conducao {
    /// A sessão segue o roteiro do fluxo (a referência: todo tipo funciona assim).
    #[default]
    Simples,
    /// A sessão chama o workflow `cba-<tipo>`, que investiga com agentes em
    /// paralelo e verificação independente; a sessão conclui.
    Workflow,
}

impl Conducao {
    pub fn simples(&self) -> bool {
        *self == Self::Simples
    }
}

/// Situação de uma execução de workflow, pelos hooks.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SituacaoWorkflow {
    /// Lançado e ainda sem o aviso de fim.
    #[default]
    Rodando,
    /// O Claude Code avisou o fim com `completed`.
    Concluido,
    /// O Claude Code avisou o fim com erro.
    Falhou,
    /// A sessão acabou (ou o workflow foi parado) antes do fim: cabe a retomada.
    Interrompido,
}

/// Uma fase do workflow, com os agentes iniciados e concluídos nela.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FaseWorkflow {
    pub titulo: String,
    #[serde(default)]
    pub iniciados: u32,
    #[serde(default)]
    pub concluidos: u32,
}

/// Uso informado pelo Claude Code no fim da execução (`<usage>` do aviso de fim).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsoWorkflow {
    #[serde(default)]
    pub agentes: u32,
    #[serde(default)]
    pub agentes_com_erro: u32,
    /// Tokens dos subagentes (`subagent_tokens`).
    #[serde(default)]
    pub tokens: u64,
    #[serde(default)]
    pub ferramentas: u32,
    #[serde(default)]
    pub duracao_ms: u64,
}

/// Execução do workflow de uma análise, como os hooks a mostram (spec, seção 3.4).
///
/// As fases vêm do `meta.phases` do script e do `journal.jsonl` da execução, que é
/// formato interno do Claude Code: quando a leitura falha, ficam só as contagens
/// totais ([`ExecucaoWorkflow::iniciados`], [`ExecucaoWorkflow::concluidos`]).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecucaoWorkflow {
    /// `cba-<tipo>`.
    pub nome: String,
    /// Origem da versão instalada (`padrao`, `global`, `projeto`), quando o script
    /// tem o cabeçalho do app.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origem: Option<String>,
    /// SHA-256 do script, do cabeçalho do app. Um hash diferente na retomada quer
    /// dizer que o script mudou (spec, seção 3.7).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    /// `runId` da execução, para a retomada na mesma sessão (`resumeFromRunId`).
    pub run_id: String,
    /// `taskId` da tarefa em segundo plano, que o aviso de fim cita.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tarefa: Option<String>,
    /// Sessão que lançou o workflow (a retomada pelo `runId` só vale nela).
    pub sessao: String,
    #[serde(default)]
    pub situacao: SituacaoWorkflow,
    #[serde(default)]
    pub fases: Vec<FaseWorkflow>,
    /// Agentes iniciados e concluídos, pelos hooks `SubagentStart`/`SubagentStop`.
    #[serde(default)]
    pub iniciados: u32,
    #[serde(default)]
    pub concluidos: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uso: Option<UsoWorkflow>,
    /// Retomada de uma execução anterior (`resumeFromRunId`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub retomada: bool,
    pub iniciado_ms: u64,
    pub atualizado_ms: u64,
}

impl ExecucaoWorkflow {
    pub fn rodando(&self) -> bool {
        self.situacao == SituacaoWorkflow::Rodando
    }
}

/// De onde veio uma contestação.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OrigemContestacao {
    /// A fase Verificar de um workflow: um agente cético, independente do que
    /// registrou, tentou refutar o item.
    #[default]
    VerificacaoIndependente,
}

impl OrigemContestacao {
    pub fn rotulo(self) -> &'static str {
        match self {
            Self::VerificacaoIndependente => "verificação independente",
        }
    }
}

/// Contestação de uma afirmação ou de um achado (`contestar_afirmacao`,
/// `contestar_achado`). Marca o item como a revalidar e fica ao lado dele até o
/// analista decidir; nada é apagado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Contestacao {
    #[serde(default)]
    pub origem: OrigemContestacao,
    /// Por que o item não se sustenta, em linguagem de negócio.
    pub motivo: String,
    /// Trechos do código que mostram o contrário, quando há.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidencias: Vec<Evidencia>,
    #[serde(default)]
    pub sessao: Option<String>,
    pub registrada_ms: u64,
}
