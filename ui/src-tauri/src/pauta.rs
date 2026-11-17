//! Comandos Tauri da pauta de perguntas pendentes (#123): a pauta da demanda (ou
//! do projeto) agrupada por destinatário, com o texto de e-mail de cada grupo;
//! escolher o destinatário; registrar a resposta (que volta às análises e atas de
//! origem e pode virar regra informada ou validar uma premissa); e exportar a
//! pauta em Markdown ou Word.
//!
//! A lógica fica em [`cba_tools::pauta`]; aqui só se monta o ambiente do projeto
//! e se leva o trabalho para fora da thread da interface.

use std::path::PathBuf;

use cba_core::dados::DiretorioDados;
use cba_core::tempo;
use cba_docs::Formato;
use cba_kb::IdDemanda;
use cba_kb::informada::Destinatario;
use cba_tools::pauta::{self, Pauta, RespostaRecebida, RespostaRegistrada};
use serde::Serialize;

use crate::analise::ambiente;

/// A pauta como a tela mostra: a pauta e o texto de e-mail de cada grupo, na
/// ordem dos grupos.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PautaVista {
    #[serde(flatten)]
    pub pauta: Pauta,
    pub emails: Vec<String>,
}

fn id_da_demanda(demanda: Option<&str>) -> Result<Option<IdDemanda>, String> {
    demanda
        .map(str::trim)
        .filter(|d| !d.is_empty())
        .map(|d| IdDemanda::analisar(d).map_err(|e| e.to_string()))
        .transpose()
}

fn montar(dados: &DiretorioDados, projeto: &str, demanda: Option<&str>) -> Result<Pauta, String> {
    let amb = ambiente(dados, projeto)?;
    pauta::pauta(&amb, id_da_demanda(demanda)?, tempo::hoje()).map_err(|e| e.to_string())
}

/// Pauta de perguntas pendentes da demanda ou, sem ela, do projeto.
pub fn pauta_de_perguntas(
    dados: &DiretorioDados,
    projeto: &str,
    demanda: Option<&str>,
) -> Result<PautaVista, String> {
    let p = montar(dados, projeto, demanda)?;
    let emails = p.grupos.iter().map(|g| pauta::email(&p, g)).collect();
    Ok(PautaVista { pauta: p, emails })
}

/// Define (ou tira) o destinatário das perguntas.
pub fn definir_destinatario(
    dados: &DiretorioDados,
    projeto: &str,
    perguntas: &[String],
    destinatario: Option<Destinatario>,
) -> Result<(), String> {
    let amb = ambiente(dados, projeto)?;
    pauta::definir_destinatario(&amb, perguntas, destinatario).map_err(|e| e.to_string())
}

/// Registra a resposta e fecha as perguntas.
pub fn registrar_resposta(
    dados: &DiretorioDados,
    projeto: &str,
    resposta: &RespostaRecebida,
) -> Result<RespostaRegistrada, String> {
    let amb = ambiente(dados, projeto)?;
    pauta::registrar_resposta(&amb, resposta, tempo::hoje()).map_err(|e| e.to_string())
}

/// Exporta a pauta em `destino` e devolve o caminho gravado.
pub fn exportar_pauta(
    dados: &DiretorioDados,
    projeto: &str,
    demanda: Option<&str>,
    formato: Formato,
    destino: &str,
) -> Result<PathBuf, String> {
    let destino = PathBuf::from(destino.trim());
    if destino.as_os_str().is_empty() {
        return Err("escolha onde gravar a pauta".into());
    }
    let p = montar(dados, projeto, demanda)?;
    pauta::exportar(&p, &destino, formato).map_err(|e| e.to_string())?;
    Ok(destino)
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
pub async fn pauta_perguntas_app(
    projeto: String,
    demanda: Option<String>,
) -> Result<PautaVista, String> {
    let dados = dados()?;
    bloqueante(move || pauta_de_perguntas(&dados, &projeto, demanda.as_deref())).await
}

#[tauri::command]
pub async fn definir_destinatario_app(
    projeto: String,
    perguntas: Vec<String>,
    destinatario: Option<Destinatario>,
) -> Result<(), String> {
    let dados = dados()?;
    bloqueante(move || definir_destinatario(&dados, &projeto, &perguntas, destinatario)).await
}

#[tauri::command]
pub async fn registrar_resposta_app(
    projeto: String,
    resposta: RespostaRecebida,
) -> Result<RespostaRegistrada, String> {
    let dados = dados()?;
    bloqueante(move || registrar_resposta(&dados, &projeto, &resposta)).await
}

#[tauri::command]
pub async fn exportar_pauta_app(
    projeto: String,
    demanda: Option<String>,
    formato: Formato,
    destino: String,
) -> Result<String, String> {
    let dados = dados()?;
    bloqueante(move || {
        exportar_pauta(&dados, &projeto, demanda.as_deref(), formato, &destino)
            .map(|p| p.display().to_string())
    })
    .await
}

#[cfg(test)]
#[path = "pauta_testes.rs"]
mod tests;
