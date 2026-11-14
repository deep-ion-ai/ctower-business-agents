//! Modo janela: o app desktop (Tauri).

use cba_core::dados::DiretorioDados;
use cba_session::clientes::{self, EstadoClientes};

use std::sync::Arc;

use crate::{
    alteracoes, analise, base, configuracoes, conversa, demandas, inicio, primeiro_uso, projeto,
    prompts, repositorios, revisao, terminal, vigia,
};

pub fn rodar() {
    // Sem log o app ainda funciona; o erro não tem para onde ir antes da janela.
    let dados = DiretorioDados::padrao().ok();
    if let Some(dados) = &dados {
        let _ = cba_core::log::iniciar(&dados.logs(), "app");
    }
    tracing::info!(versao = cba_core::VERSAO, "app iniciado");
    if let Some(dados) = dados {
        // Retenção dos transcripts das reuniões (#136): apaga os de prazo vencido,
        // fora da thread da interface. O aviso dos que vencem fica para as telas
        // da demanda e da reunião.
        let _ = std::thread::Builder::new()
            .name("cba-retencao-transcripts".into())
            .spawn(move || {
                cba_reunioes::aplicar_retencao_no_inicio(&dados);
            });
    }
    let sessoes = terminal::Estado::default();
    let vigias = Arc::new(vigia::Vigias::padrao());
    tauri::Builder::default()
        // Abre os links oficiais da orientação dos clientes no navegador (só os
        // domínios liberados em capabilities/default.json) e, pelo Rust, os links
        // `claude://` e `claude-cli://` montados em `conversa`.
        .plugin(tauri_plugin_opener::init())
        // Seletor de pastas do primeiro uso (base e pastas do projeto) e diálogo de
        // salvar da exportação.
        .plugin(tauri_plugin_dialog::init())
        .manage(sessoes.clone())
        .manage(vigias)
        .manage(Arc::new(base::VigiaBase::default()))
        .invoke_handler(tauri::generate_handler![
            estado_clientes,
            terminal::listar_projetos,
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
            primeiro_uso::previa_projeto_app,
            primeiro_uso::cadastrar_projeto_app,
            primeiro_uso::concluir_primeiro_uso_app,
            analise::verificar_ambiente_projeto,
            analise::planejar_preparo_projeto,
            analise::preparar_ambiente_projeto,
            analise::listar_analises_projeto,
            analise::criar_analise_pergunta,
            analise::carregar_analise_projeto,
            analise::trecho_evidencia_projeto,
            conversa::opcoes_conversa_analise,
            conversa::abrir_conversa_por_link,
            conversa::conversa_passou_ao_desktop,
            conversa::retomar_conversa_no_desktop,
            vigia::vigiar_analises,
            vigia::parar_vigia,
            prompts::prompts_prontos,
            alteracoes::alteracoes_da_analise_projeto,
            alteracoes::desfazer_alteracoes_projeto,
            alteracoes::manter_alteracoes_projeto,
            alteracoes::situacao_protecao_projeto,
            alteracoes::mudar_protecao_projeto,
            inicio::inicio_app,
            inicio::criar_analise_fluxo,
            projeto::visao_projeto_app,
            projeto::revalidar_projeto_app,
            projeto::mapa_projeto_app,
            projeto::mapear_projeto_app,
            demandas::listar_demandas_app,
            demandas::criar_demanda_app,
            demandas::encerrar_demanda_app,
            demandas::reabrir_demanda_app,
            demandas::painel_demanda_app,
            base::buscar_base,
            base::abrir_documento_base,
            base::vigiar_base,
            base::parar_vigia_base,
            repositorios::salvar_repositorios_app,
            repositorios::prontidao_repositorios_app,
            repositorios::criar_analise_de_demanda,
            configuracoes::configuracoes_app,
            configuracoes::avaliar_troca_base_app,
            configuracoes::trocar_base_app,
            configuracoes::previa_exclusoes_app,
            configuracoes::salvar_exclusoes_app,
            configuracoes::desfazer_ambiente_projeto,
            configuracoes::descadastrar_projeto_app,
            configuracoes::textos_editaveis_app,
            configuracoes::abrir_texto_editavel,
            configuracoes::ler_texto_editavel,
            configuracoes::validar_texto_editavel,
            configuracoes::salvar_texto_editavel,
            configuracoes::restaurar_texto_padrao,
            revisao::revisar_analise_projeto,
            revisao::prontidao_aprovacao_projeto,
            revisao::aprovar_analise_projeto,
            revisao::exportar_analise_projeto,
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
