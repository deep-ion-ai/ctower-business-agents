//! Modo janela: o app desktop (Tauri).

use cba_core::dados::DiretorioDados;
use cba_session::clientes::{self, EstadoClientes};

use std::sync::Arc;

use crate::{alteracoes, analise, primeiro_uso, prompts, terminal, vigia};

pub fn rodar() {
    // Sem log o app ainda funciona; o erro não tem para onde ir antes da janela.
    if let Ok(dados) = DiretorioDados::padrao() {
        let _ = cba_core::log::iniciar(&dados.logs(), "app");
    }
    tracing::info!(versao = cba_core::VERSAO, "app iniciado");
    let sessoes = terminal::Estado::default();
    let vigias = Arc::new(vigia::Vigias::padrao());
    tauri::Builder::default()
        // Abre os links oficiais da orientação dos clientes no navegador (só os
        // domínios liberados em capabilities/default.json).
        .plugin(tauri_plugin_opener::init())
        // Seletor de pastas do primeiro uso (base e pastas do sistema).
        .plugin(tauri_plugin_dialog::init())
        .manage(sessoes.clone())
        .manage(vigias)
        .invoke_handler(tauri::generate_handler![
            estado_clientes,
            terminal::listar_sistemas,
            terminal::abrir_terminal_claude,
            terminal::escrever_terminal,
            terminal::redimensionar_terminal,
            terminal::enviar_prompt_terminal,
            terminal::encerrar_terminal,
            terminal::ultima_sessao_terminal,
            primeiro_uso::situacao_app,
            primeiro_uso::escolher_cliente_app,
            primeiro_uso::avaliar_base_app,
            primeiro_uso::configurar_base_app,
            primeiro_uso::previa_sistema_app,
            primeiro_uso::cadastrar_sistema_app,
            primeiro_uso::concluir_primeiro_uso_app,
            analise::verificar_ambiente_sistema,
            analise::planejar_preparo_sistema,
            analise::preparar_ambiente_sistema,
            analise::listar_analises_sistema,
            analise::criar_analise_pergunta,
            analise::carregar_analise_sistema,
            analise::trecho_evidencia_sistema,
            vigia::vigiar_analises,
            vigia::parar_vigia,
            prompts::prompts_prontos,
            alteracoes::alteracoes_da_analise_sistema,
            alteracoes::desfazer_alteracoes_sistema,
            alteracoes::manter_alteracoes_sistema,
            alteracoes::situacao_protecao_sistema,
            alteracoes::mudar_protecao_sistema,
        ])
        .build(tauri::generate_context!())
        .expect("erro ao iniciar o app")
        .run(move |_, evento| {
            // Ao fechar o app, nenhum `claude` fica rodando sem janela.
            if let tauri::RunEvent::Exit = evento {
                sessoes.encerrar_todas();
            }
        });
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
