//! Modo janela: o app desktop (Tauri).

use cba_core::dados::DiretorioDados;

pub fn rodar() {
    // Sem log o app ainda funciona; o erro não tem para onde ir antes da janela.
    if let Ok(dados) = DiretorioDados::padrao() {
        let _ = cba_core::log::iniciar(&dados.logs(), "app");
    }
    tracing::info!(versao = cba_core::VERSAO, "app iniciado");
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o app");
}
