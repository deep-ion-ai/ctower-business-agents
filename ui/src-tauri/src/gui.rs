//! Modo janela: o app desktop (Tauri).

use cba_core::dados::DiretorioDados;
use cba_session::clientes::{self, EstadoClientes};

pub fn rodar() {
    // Sem log o app ainda funciona; o erro não tem para onde ir antes da janela.
    if let Ok(dados) = DiretorioDados::padrao() {
        let _ = cba_core::log::iniciar(&dados.logs(), "app");
    }
    tracing::info!(versao = cba_core::VERSAO, "app iniciado");
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![estado_clientes])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o app");
}

/// Estado do Claude Code e do Claude Desktop (instalado, versão, logado) e o passo a
/// passo oficial do que falta. Os comandos do Claude Code podem levar alguns segundos,
/// então a detecção roda fora da thread da interface.
#[tauri::command]
async fn estado_clientes() -> Result<EstadoClientes, String> {
    tauri::async_runtime::spawn_blocking(clientes::detectar)
        .await
        .map_err(|e| format!("falha na detecção dos clientes: {e}"))
}
