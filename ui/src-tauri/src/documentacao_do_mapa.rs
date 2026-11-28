//! Comandos Tauri da visão "Documentação" do mapa (#462): a tabela dos
//! documentos vivos por funcionalidade e tipo, com a cobertura por tipo, a
//! geração da versão inicial em lote e a revisão da proposta (ler, marcar como
//! revisada, publicar, descartar).
//!
//! As contas e as regras ficam em [`cba_tools::documentacao_do_mapa`] e
//! [`cba_tools::documentos_vivos`]; aqui só se monta o ambiente do projeto e
//! se converte o erro para a tela.

use cba_core::dados::DiretorioDados;
use cba_core::documentos_vivos::{DocumentosVivos, Dono, Proposta, Versao};
use cba_kb::{CaminhoRelativo, IdFuncionalidade};
use cba_tools::documentacao_do_mapa::{self, Lote, TabelaDeDocumentacao};
use cba_tools::documentos_vivos::{self, Publicacao};
use serde::Serialize;

use crate::analise::ambiente;

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

/// Caminho de um documento vivo, como a tabela o mostra (relativo ao projeto
/// na base).
fn caminho(texto: &str) -> Result<CaminhoRelativo, String> {
    CaminhoRelativo::novo(texto.trim()).map_err(|e| format!("caminho inválido `{texto}`: {e}"))
}

/// Quem revisa: o usuário do sistema.
fn revisor() -> String {
    crate::consultas::quem().unwrap_or_else(|| "analista".to_owned())
}

pub fn tabela(dados: &DiretorioDados, projeto: &str) -> Result<TabelaDeDocumentacao, String> {
    let ambiente = ambiente(dados, projeto)?;
    documentacao_do_mapa::tabela(&ambiente).map_err(|e| e.to_string())
}

pub fn gerar_em_lote(
    dados: &DiretorioDados,
    projeto: &str,
    tipo: &str,
    funcionalidades: &[String],
) -> Result<Lote, String> {
    let ambiente = ambiente(dados, projeto)?;
    documentacao_do_mapa::gerar_em_lote(&ambiente, tipo, funcionalidades).map_err(|e| e.to_string())
}

/// A proposta de um documento, para a revisão.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PropostaVista {
    pub caminho: String,
    pub tipo: String,
    pub nome_do_tipo: String,
    /// A funcionalidade dona, quando é de uma.
    pub funcionalidade: Option<String>,
    /// A versão publicada, se há.
    pub publicada: Option<Versao>,
    pub proposta: Proposta,
}

pub fn proposta(
    dados: &DiretorioDados,
    projeto: &str,
    documento: &str,
) -> Result<PropostaVista, String> {
    let ambiente = ambiente(dados, projeto)?;
    let caminho = caminho(documento)?;
    let estado = DocumentosVivos::carregar(&ambiente.kb).map_err(|e| e.to_string())?;
    let d = estado
        .documento(&caminho)
        .ok_or_else(|| format!("não há documento vivo em `{caminho}`"))?;
    let proposta = d
        .proposta
        .clone()
        .ok_or_else(|| format!("o documento `{caminho}` não tem proposta pendente"))?;
    Ok(PropostaVista {
        caminho: caminho.to_string(),
        nome_do_tipo: cba_docs::entregaveis::tipo(&d.tipo)
            .map_or_else(|| d.tipo.clone(), |t| t.nome.to_owned()),
        tipo: d.tipo.clone(),
        funcionalidade: match &d.dono {
            Dono::Funcionalidade(f) => Some(f.to_string()),
            Dono::Area(_) => None,
        },
        publicada: d.versao_publicada(),
        proposta,
    })
}

pub fn marcar_revisada(
    dados: &DiretorioDados,
    projeto: &str,
    documento: &str,
) -> Result<Proposta, String> {
    let ambiente = ambiente(dados, projeto)?;
    documentos_vivos::marcar_revisada(&ambiente, &caminho(documento)?, &revisor())
        .map_err(|e| e.to_string())
}

pub fn publicar(
    dados: &DiretorioDados,
    projeto: &str,
    documento: &str,
) -> Result<Publicacao, String> {
    let ambiente = ambiente(dados, projeto)?;
    let p =
        documentos_vivos::publicar(&ambiente, &caminho(documento)?).map_err(|e| e.to_string())?;
    for a in &p.avisos {
        tracing::warn!(aviso = %a, "publicação do documento vivo");
    }
    Ok(p)
}

/// "Propor atualização" de um documento montado pelo app.
pub fn propor_atualizacao(
    dados: &DiretorioDados,
    projeto: &str,
    tipo: &str,
    funcionalidade: &str,
) -> Result<String, String> {
    let ambiente = ambiente(dados, projeto)?;
    let id = IdFuncionalidade::analisar(funcionalidade.trim())
        .map_err(|_| format!("\"{funcionalidade}\" não é uma funcionalidade (F-<SIGLA>-NNN)"))?;
    documentacao_do_mapa::propor_atualizacao(&ambiente, tipo, &id)
        .map(|r| r.caminho.to_string())
        .map_err(|e| e.to_string())
}

pub fn descartar(dados: &DiretorioDados, projeto: &str, documento: &str) -> Result<(), String> {
    let ambiente = ambiente(dados, projeto)?;
    documentos_vivos::descartar_proposta(&ambiente, &caminho(documento)?).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Comandos

#[tauri::command]
pub async fn documentacao_do_mapa_app(projeto: String) -> Result<TabelaDeDocumentacao, String> {
    let dados = dados()?;
    bloqueante(move || tabela(&dados, &projeto)).await
}

#[tauri::command]
pub async fn gerar_versoes_iniciais_app(
    projeto: String,
    tipo: String,
    funcionalidades: Vec<String>,
) -> Result<Lote, String> {
    let dados = dados()?;
    bloqueante(move || gerar_em_lote(&dados, &projeto, &tipo, &funcionalidades)).await
}

#[tauri::command]
pub async fn proposta_documento_vivo_app(
    projeto: String,
    documento: String,
) -> Result<PropostaVista, String> {
    let dados = dados()?;
    bloqueante(move || proposta(&dados, &projeto, &documento)).await
}

#[tauri::command]
pub async fn marcar_documento_vivo_revisado_app(
    projeto: String,
    documento: String,
) -> Result<Proposta, String> {
    let dados = dados()?;
    bloqueante(move || marcar_revisada(&dados, &projeto, &documento)).await
}

#[tauri::command]
pub async fn publicar_documento_vivo_app(
    projeto: String,
    documento: String,
) -> Result<Publicacao, String> {
    let dados = dados()?;
    bloqueante(move || publicar(&dados, &projeto, &documento)).await
}

/// Devolve o caminho do documento, com a proposta nova.
#[tauri::command]
pub async fn propor_atualizacao_documento_vivo_app(
    projeto: String,
    tipo: String,
    funcionalidade: String,
) -> Result<String, String> {
    let dados = dados()?;
    bloqueante(move || propor_atualizacao(&dados, &projeto, &tipo, &funcionalidade)).await
}

#[tauri::command]
pub async fn descartar_proposta_documento_vivo_app(
    projeto: String,
    documento: String,
) -> Result<(), String> {
    let dados = dados()?;
    bloqueante(move || descartar(&dados, &projeto, &documento)).await
}

#[cfg(test)]
#[path = "documentacao_do_mapa_testes.rs"]
mod tests;
