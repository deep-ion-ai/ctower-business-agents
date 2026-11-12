//! Comandos Tauri da revisão, aprovação e exportação da análise (#37; spec,
//! princípios, seções 4.3, 7.4 e 10).
//!
//! A lógica fica em [`cba_tools::revisao`] (decisões, edição do texto, prontidão,
//! aprovação com a escolha da área e exportação); aqui só se monta o ambiente do
//! sistema e se leva o trabalho para fora da thread da interface. A tela relê a
//! análise pelo aviso do diário de eventos, como em qualquer registro, mas cada
//! decisão também devolve a vista atualizada, para a resposta ser imediata.

use std::path::PathBuf;

use cba_core::dados::DiretorioDados;
use cba_kb::Id;
use cba_tools::aprovacao::{Acao, OpcoesAprovacao};
use cba_tools::revisao::{self, Decisao, Prontidao};
use serde::{Deserialize, Serialize};

use crate::analise::{VistaAnalise, ambiente, carregar_analise};

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

fn id(analise: &str) -> Result<Id, String> {
    Id::analisar(analise.trim()).map_err(|e| e.to_string())
}

/// Aplica a decisão do analista e devolve a análise como a tela mostra.
pub fn revisar(
    dados: &DiretorioDados,
    sistema: &str,
    analise: &str,
    decisao: Decisao,
) -> Result<VistaAnalise, String> {
    let amb = ambiente(dados, sistema)?;
    revisao::revisar(&amb, &id(analise)?, decisao).map_err(|e| e.to_string())?;
    carregar_analise(dados, sistema, analise)
}

pub fn prontidao(
    dados: &DiretorioDados,
    sistema: &str,
    analise: &str,
) -> Result<Prontidao, String> {
    let amb = ambiente(dados, sistema)?;
    revisao::prontidao(&amb, &id(analise)?).map_err(|e| e.to_string())
}

/// Regra levada à base pela aprovação.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegraVista {
    pub id: String,
    /// Relativo à raiz do sistema na base (`regras/RN-FAT-001.md`).
    pub arquivo: String,
    pub area: String,
    pub afirmacoes: Vec<u32>,
    pub acao: Acao,
}

/// Termo levado ao glossário pela aprovação.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TermoVista {
    pub termo: String,
    pub arquivo: String,
    pub acao: Acao,
}

/// Resultado da aprovação, como a tela mostra.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AprovacaoVista {
    pub ja_aprovada: bool,
    pub regras: Vec<RegraVista>,
    pub termos: Vec<TermoVista>,
    pub avisos: Vec<String>,
}

pub fn aprovar(
    dados: &DiretorioDados,
    sistema: &str,
    analise: &str,
    opcoes: &OpcoesAprovacao,
) -> Result<AprovacaoVista, String> {
    let amb = ambiente(dados, sistema)?;
    let feita =
        revisao::aprovar_revisada(&amb, &id(analise)?, opcoes).map_err(|e| e.to_string())?;
    tracing::info!(
        regras = feita.regras.len(),
        termos = feita.termos.len(),
        "análise aprovada"
    );
    Ok(AprovacaoVista {
        ja_aprovada: feita.ja_aprovada,
        regras: feita
            .regras
            .into_iter()
            .map(|r| RegraVista {
                id: r.id.to_string(),
                arquivo: r.arquivo.to_string(),
                area: r.area,
                afirmacoes: r.afirmacoes,
                acao: r.acao,
            })
            .collect(),
        termos: feita
            .termos
            .into_iter()
            .map(|t| TermoVista {
                termo: t.termo,
                arquivo: t.arquivo.to_string(),
                acao: t.acao,
            })
            .collect(),
        avisos: feita.avisos,
    })
}

/// Pedido de exportação da tela.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PedidoExportacao {
    /// Caminho escolhido no diálogo de salvar.
    pub destino: PathBuf,
    pub formato: cba_tools::revisao::Formato,
    #[serde(default)]
    pub opcoes: cba_tools::revisao::OpcoesExportacao,
}

/// Exporta o documento e devolve o caminho gravado.
pub fn exportar(
    dados: &DiretorioDados,
    sistema: &str,
    analise: &str,
    pedido: &PedidoExportacao,
) -> Result<PathBuf, String> {
    let amb = ambiente(dados, sistema)?;
    revisao::exportar_analise(
        &amb,
        &id(analise)?,
        &pedido.destino,
        pedido.formato,
        &pedido.opcoes,
    )
    .map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Comandos

#[tauri::command]
pub async fn revisar_analise_sistema(
    sistema: String,
    analise: String,
    decisao: Decisao,
) -> Result<VistaAnalise, String> {
    let dados = dados()?;
    bloqueante(move || revisar(&dados, &sistema, &analise, decisao)).await
}

#[tauri::command]
pub async fn prontidao_aprovacao_sistema(
    sistema: String,
    analise: String,
) -> Result<Prontidao, String> {
    let dados = dados()?;
    bloqueante(move || prontidao(&dados, &sistema, &analise)).await
}

#[tauri::command]
pub async fn aprovar_analise_sistema(
    sistema: String,
    analise: String,
    opcoes: OpcoesAprovacao,
) -> Result<AprovacaoVista, String> {
    let dados = dados()?;
    bloqueante(move || aprovar(&dados, &sistema, &analise, &opcoes)).await
}

#[tauri::command]
pub async fn exportar_analise_sistema(
    sistema: String,
    analise: String,
    pedido: PedidoExportacao,
) -> Result<String, String> {
    let dados = dados()?;
    bloqueante(move || {
        exportar(&dados, &sistema, &analise, &pedido).map(|p| p.display().to_string())
    })
    .await
}

#[cfg(test)]
#[path = "revisao_testes.rs"]
mod testes;
