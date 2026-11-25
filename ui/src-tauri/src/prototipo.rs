//! Comandos Tauri do visualizador do protótipo de telas (#382): o HTML gravado
//! de uma tela, para o `iframe` isolado da análise (`sandbox` vazio, sem script
//! nem rede). A leitura confere que o arquivo fica na pasta do protótipo da
//! análise ([`cba_tools::prototipo::ler_html_da_tela`]).

use cba_core::dados::DiretorioDados;
use cba_kb::Id;

use crate::analise::ambiente;

fn dados() -> Result<DiretorioDados, String> {
    DiretorioDados::padrao().map_err(|e| format!("diretório de dados do app inacessível: {e}"))
}

/// O HTML da tela `TL-<numero>` da análise; `None` enquanto a tela não tem
/// protótipo gravado.
pub fn html_da_tela(
    dados: &DiretorioDados,
    projeto: &str,
    analise: &str,
    numero: u32,
) -> Result<Option<String>, String> {
    let amb = ambiente(dados, projeto)?;
    let id = Id::analisar(analise.trim()).map_err(|e| e.to_string())?;
    let a = amb.analises.carregar(&id).map_err(|e| e.to_string())?;
    cba_tools::prototipo::ler_html_da_tela(&amb.kb, &a, numero)
}

#[tauri::command]
pub async fn html_da_tela_prototipo_app(
    projeto: String,
    analise: String,
    tela: u32,
) -> Result<Option<String>, String> {
    let dados = dados()?;
    tauri::async_runtime::spawn_blocking(move || html_da_tela(&dados, &projeto, &analise, tela))
        .await
        .map_err(|e| format!("falha interna: {e}"))?
}

#[cfg(test)]
#[path = "prototipo_testes.rs"]
mod tests;
