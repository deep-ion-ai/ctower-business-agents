//! Comando Tauri dos prompts prontos da tela da análise.
//!
//! A lógica fica em [`cba_tools::prompts`]; aqui só se lê o diretório de dados e se
//! devolve a lista à janela, que envia o texto escolhido ao terminal
//! (`enviar_prompt_terminal`).

use cba_core::dados::DiretorioDados;
use cba_tools::prompts::{self, PromptPronto};

/// Os prompts prontos da análise `analise` (A-NNNN) do sistema cadastrado
/// `sistema`, com as variáveis preenchidas e o texto da base quando ela tiver um.
/// Lê arquivos, então roda fora da thread da interface.
#[tauri::command]
pub async fn prompts_prontos(
    sistema: String,
    analise: String,
) -> Result<Vec<PromptPronto>, String> {
    let dados = DiretorioDados::padrao()
        .map_err(|e| format!("diretório de dados do app inacessível: {e}"))?;
    tauri::async_runtime::spawn_blocking(move || {
        prompts::da_analise(&dados, &sistema, &analise).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("falha interna ao montar os prompts prontos: {e}"))?
}
