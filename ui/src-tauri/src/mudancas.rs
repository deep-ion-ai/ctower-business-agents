//! Comandos Tauri do "O que mudou no código" (#124): o relatório entre dois pontos
//! do repositório, a nota de mudanças em Markdown, o pedido de explicação para o
//! agente e a confirmação ou o descarte das propostas de atualização de regra. A
//! lógica (sem IA) está em `cba_tools::mudancas`.

use cba_core::dados::DiretorioDados;
use cba_tools::mudancas::{
    self, AjusteDaProposta, Pedido, PropostaDeRegra, RegraAtualizada, RelatorioDeMudancas,
};

use crate::analise::ambiente;

fn pedido(de: Option<String>, ate: Option<String>) -> Pedido {
    Pedido { de, ate }
}

pub fn relatorio_do_projeto(
    dados: &DiretorioDados,
    projeto: &str,
    de: Option<String>,
    ate: Option<String>,
) -> Result<RelatorioDeMudancas, String> {
    let ambiente = ambiente(dados, projeto)?;
    mudancas::relatorio(&ambiente, &pedido(de, ate)).map_err(|e| e.to_string())
}

pub fn nota_do_projeto(
    dados: &DiretorioDados,
    projeto: &str,
    de: Option<String>,
    ate: Option<String>,
) -> Result<String, String> {
    let ambiente = ambiente(dados, projeto)?;
    let r = mudancas::relatorio(&ambiente, &pedido(de, ate)).map_err(|e| e.to_string())?;
    Ok(mudancas::nota_de_mudancas(
        &r,
        &mudancas::propostas(&ambiente),
    ))
}

pub fn propostas_do_projeto(
    dados: &DiretorioDados,
    projeto: &str,
) -> Result<Vec<PropostaDeRegra>, String> {
    Ok(mudancas::propostas(&ambiente(dados, projeto)?))
}

pub fn confirmar(
    dados: &DiretorioDados,
    projeto: &str,
    numero: u32,
    ajuste: AjusteDaProposta,
) -> Result<RegraAtualizada, String> {
    let ambiente = ambiente(dados, projeto)?;
    mudancas::confirmar_proposta(&ambiente, numero, &ajuste).map_err(|e| e.to_string())
}

pub fn descartar(dados: &DiretorioDados, projeto: &str, numero: u32) -> Result<(), String> {
    let ambiente = ambiente(dados, projeto)?;
    mudancas::descartar_proposta(&ambiente, numero).map_err(|e| e.to_string())
}

fn dados() -> Result<DiretorioDados, String> {
    DiretorioDados::padrao().map_err(|e| format!("diretório de dados do app inacessível: {e}"))
}

async fn em_segundo_plano<T: Send + 'static>(
    f: impl FnOnce(DiretorioDados) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    let dados = dados()?;
    tauri::async_runtime::spawn_blocking(move || f(dados))
        .await
        .map_err(|e| format!("falha interna: {e}"))?
}

#[tauri::command]
pub async fn mudancas_projeto_app(
    projeto: String,
    de: Option<String>,
    ate: Option<String>,
) -> Result<RelatorioDeMudancas, String> {
    em_segundo_plano(move |d| relatorio_do_projeto(&d, &projeto, de, ate)).await
}

/// Grava a nota de mudanças em `destino` (escolhido no diálogo de salvar) e
/// devolve o caminho gravado.
pub fn exportar_nota(
    dados: &DiretorioDados,
    projeto: &str,
    de: Option<String>,
    ate: Option<String>,
    destino: &str,
) -> Result<String, String> {
    let nota = nota_do_projeto(dados, projeto, de, ate)?;
    let caminho = std::path::Path::new(destino);
    cba_kb::arquivo::gravar(caminho, nota.as_bytes())
        .map_err(|e| format!("não foi possível gravar a nota em {destino}: {e}"))?;
    Ok(caminho.display().to_string())
}

#[tauri::command]
pub async fn exportar_nota_de_mudancas_app(
    projeto: String,
    de: Option<String>,
    ate: Option<String>,
    destino: String,
) -> Result<String, String> {
    em_segundo_plano(move |d| exportar_nota(&d, &projeto, de, ate, &destino)).await
}

#[tauri::command]
pub fn pedido_de_explicacao_app(de: Option<String>, ate: Option<String>) -> String {
    mudancas::pedido_de_explicacao(&pedido(de, ate))
}

#[tauri::command]
pub async fn propostas_de_mudanca_app(projeto: String) -> Result<Vec<PropostaDeRegra>, String> {
    em_segundo_plano(move |d| propostas_do_projeto(&d, &projeto)).await
}

#[tauri::command]
pub async fn confirmar_proposta_de_mudanca_app(
    projeto: String,
    numero: u32,
    titulo: Option<String>,
    enunciado: Option<String>,
) -> Result<RegraAtualizada, String> {
    em_segundo_plano(move |d| {
        confirmar(&d, &projeto, numero, AjusteDaProposta { titulo, enunciado })
    })
    .await
}

#[tauri::command]
pub async fn descartar_proposta_de_mudanca_app(projeto: String, numero: u32) -> Result<(), String> {
    em_segundo_plano(move |d| descartar(&d, &projeto, numero)).await
}

#[cfg(test)]
#[path = "mudancas_testes.rs"]
mod tests;
