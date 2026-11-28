//! Modo janela: o app desktop (Tauri).

use cba_core::dados::DiretorioDados;
use cba_session::clientes::{self, EstadoClientes};

use std::sync::Arc;

use crate::{
    acessos, alteracoes, ambiente_teste, analise, base, clones, cobertura, conducao, conexoes,
    configuracoes, consultas, conversa, demandas, docs_externos, documentacao_do_mapa,
    entregaveis_demanda, funcionalidades, inicio, issues_projeto, links_projeto, manual,
    migracao_conexoes, migracao_entregaveis, mudancas, painel_funcionalidade, pauta, primeiro_uso,
    projeto, prompts, prototipo, publicacao, rastreabilidade, registros, repositorios, resumo,
    reunioes, revisao, terminal, tokens, vigia, vincular_issues, workflows,
};

pub fn rodar() {
    // Sem log o app ainda funciona; o erro não tem para onde ir antes da janela.
    let dados = DiretorioDados::padrao().ok();
    if let Some(dados) = &dados {
        let _ = cba_core::log::iniciar(&dados.logs(), "app");
    }
    tracing::info!(versao = cba_core::VERSAO, "app iniciado");
    if let Some(dados) = &dados {
        // Cadastros de antes das conexões (#438): migra antes de a janela abrir;
        // uma falha volta ao backup e o erro aparece em Configurações › Conexões.
        migracao_conexoes::na_abertura(dados, &cba_repos::chaveiro::ChaveiroNativo);
        // Documento negocial de antes do catálogo de entregáveis (#465): vira a
        // v1.0 do documento vivo; uma falha volta ao backup e vai para o log.
        migracao_entregaveis::na_abertura(dados);
    }
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
        .manage(Arc::new(clones::CloneEmAndamento::default()))
        .manage(Arc::new(analise::EvidenciasVistas::default()))
        .invoke_handler(tauri::generate_handler![
            estado_clientes,
            terminal::listar_projetos,
            terminal::abrir_terminal_claude,
            terminal::escrever_terminal,
            terminal::redimensionar_terminal,
            terminal::enviar_prompt_terminal,
            terminal::encerrar_terminal,
            terminal::ultima_sessao_terminal,
            conexoes::descritores_conexao_app,
            conexoes::listar_conexoes_app,
            conexoes::salvar_conexao_app,
            conexoes::remover_conexao_app,
            conexoes::guardar_credencial_conexao_app,
            conexoes::testar_conexao_app,
            conexoes::erro_migracao_conexoes_app,
            primeiro_uso::situacao_app,
            primeiro_uso::escolher_cliente_app,
            primeiro_uso::avaliar_base_app,
            primeiro_uso::configurar_base_app,
            primeiro_uso::previa_projeto_app,
            primeiro_uso::nome_projeto_em_uso_app,
            primeiro_uso::cadastrar_projeto_app,
            primeiro_uso::concluir_primeiro_uso_app,
            analise::verificar_ambiente_projeto,
            analise::planejar_preparo_projeto,
            analise::preparar_ambiente_projeto,
            analise::listar_analises_projeto,
            analise::criar_analise_pergunta,
            analise::carregar_analise_projeto,
            analise::trecho_evidencia_projeto,
            analise::conferir_evidencias_analise,
            analise::link_forja_evidencia_projeto,
            conversa::opcoes_conversa_analise,
            conducao::conducao_disponivel_projeto,
            conducao::escolher_conducao_analise,
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
            alteracoes::migrar_configuracoes_claude,
            alteracoes::marcar_aviso_migracao_visto,
            inicio::inicio_app,
            resumo::resumo_projeto_app,
            resumo::listar_sessoes_projeto_app,
            inicio::criar_analise_fluxo,
            inicio::criar_analise_de_historias_app,
            inicio::criar_analise_de_cenarios_app,
            inicio::criar_analise_de_prototipo_app,
            cobertura::cobertura_projeto_app,
            mudancas::mudancas_projeto_app,
            mudancas::exportar_nota_de_mudancas_app,
            mudancas::pedido_de_explicacao_app,
            mudancas::propostas_de_mudanca_app,
            mudancas::confirmar_proposta_de_mudanca_app,
            mudancas::descartar_proposta_de_mudanca_app,
            funcionalidades::mapa_funcionalidades_projeto,
            funcionalidades::revisar_mapa_projeto,
            funcionalidades::pedido_de_mapeamento,
            funcionalidades::explicar_funcionalidade_app,
            funcionalidades::exportar_mapa_projeto,
            funcionalidades::religar_funcionalidades_app,
            documentacao_do_mapa::documentacao_do_mapa_app,
            documentacao_do_mapa::gerar_versoes_iniciais_app,
            documentacao_do_mapa::proposta_documento_vivo_app,
            documentacao_do_mapa::marcar_documento_vivo_revisado_app,
            documentacao_do_mapa::publicar_documento_vivo_app,
            documentacao_do_mapa::propor_atualizacao_documento_vivo_app,
            documentacao_do_mapa::descartar_proposta_documento_vivo_app,
            prototipo::html_da_tela_prototipo_app,
            prototipo::exportar_prototipo_app,
            tokens::tokens_css_projeto,
            tokens::extrair_tokens_css_projeto,
            tokens::informar_token_css_projeto,
            tokens::aprovar_tokens_css_projeto,
            painel_funcionalidade::painel_funcionalidade_app,
            painel_funcionalidade::criar_analise_da_funcionalidade_app,
            manual::anexar_print_manual_app,
            manual::remover_print_manual_app,
            manual::remover_passo_manual_app,
            manual::ler_print_manual_app,
            issues_projeto::issues_do_projeto_app,
            issues_projeto::analisar_impacto_da_issue_app,
            links_projeto::abrir_link_do_projeto_app,
            vincular_issues::issues_para_vincular_app,
            vincular_issues::confirmar_vinculos_app,
            vincular_issues::justificar_issue_app,
            vincular_issues::pedido_vinculo_ia_app,
            projeto::visao_projeto_app,
            projeto::revalidar_projeto_app,
            projeto::mapa_projeto_app,
            projeto::mapear_projeto_app,
            demandas::listar_demandas_app,
            demandas::atualizar_status_demandas_app,
            demandas::criar_demanda_app,
            demandas::encerrar_demanda_app,
            demandas::reabrir_demanda_app,
            demandas::painel_demanda_app,
            entregaveis_demanda::entregaveis_demanda_app,
            entregaveis_demanda::gerar_entregavel_app,
            entregaveis_demanda::editar_entregavel_app,
            entregaveis_demanda::marcar_entregavel_revisado_app,
            entregaveis_demanda::exportar_entregavel_app,
            entregaveis_demanda::publicar_entregavel_app,
            entregaveis_demanda::comentario_entregavel_app,
            registros::registros_demanda_app,
            registros::criar_registro_app,
            registros::editar_registro_app,
            registros::validar_premissa_app,
            registros::aceitar_proposta_app,
            registros::recusar_proposta_app,
            pauta::pauta_perguntas_app,
            pauta::definir_destinatario_app,
            pauta::registrar_resposta_app,
            pauta::exportar_pauta_app,
            rastreabilidade::matriz_demanda_app,
            rastreabilidade::dependentes_base_app,
            rastreabilidade::exportar_matriz_app,
            reunioes::previa_transcript_app,
            reunioes::criar_reuniao_app,
            reunioes::ata_reuniao_app,
            reunioes::confirmar_participantes_app,
            reunioes::prontidao_ata_app,
            reunioes::aprovar_ata_app,
            reunioes::analisar_proximo_passo_app,
            base::buscar_base,
            base::abrir_documento_base,
            base::salvar_documento_base,
            base::vigiar_base,
            base::parar_vigia_base,
            repositorios::salvar_repositorios_app,
            repositorios::prontidao_repositorios_app,
            repositorios::detectar_repositorios_app,
            publicacao::previa_publicacao_docs_app,
            publicacao::publicar_docs_app,
            docs_externos::situacao_docs_externos_app,
            docs_externos::sincronizar_docs_externos_app,
            clones::pasta_clones_app,
            clones::definir_pasta_clones_app,
            clones::salvar_token_app,
            clones::apagar_token_app,
            clones::tokens_guardados_app,
            clones::testar_acesso_app,
            clones::clonar_repositorios_app,
            clones::cancelar_clone_app,
            clones::testar_jira_app,
            clones::testar_demandas_app,
            acessos::hosts_acessos_app,
            acessos::adicionar_host_acesso_app,
            acessos::remover_host_acesso_app,
            acessos::definir_comando_host_app,
            acessos::estado_tokens_app,
            acessos::testar_token_host_app,
            acessos::testar_token_repositorio_app,
            ambiente_teste::ambiente_teste_app,
            ambiente_teste::salvar_ambiente_teste_app,
            ambiente_teste::remover_ambiente_teste_app,
            consultas::bancos_projeto_app,
            consultas::salvar_bancos_projeto_app,
            consultas::ligar_banco_analise_app,
            consultas::decidir_consulta_app,
            repositorios::criar_analise_de_demanda,
            configuracoes::configuracoes_app,
            configuracoes::info_base_app,
            configuracoes::abrir_base_app,
            configuracoes::avaliar_troca_base_app,
            configuracoes::trocar_base_app,
            configuracoes::previa_leitura_app,
            configuracoes::salvar_leitura_app,
            configuracoes::salvar_mascarar_demanda_app,
            configuracoes::entregaveis_projeto_app,
            configuracoes::fluxos_ocultos_app,
            configuracoes::salvar_entregaveis_app,
            configuracoes::desfazer_ambiente_projeto,
            configuracoes::descadastrar_projeto_app,
            configuracoes::textos_editaveis_app,
            configuracoes::abrir_texto_editavel,
            configuracoes::ler_texto_editavel,
            configuracoes::validar_texto_editavel,
            configuracoes::salvar_texto_editavel,
            configuracoes::restaurar_texto_padrao,
            configuracoes::previa_exportacao_texto,
            workflows::workflows_app,
            workflows::ler_workflow_app,
            workflows::validar_workflow_app,
            workflows::diferenca_workflow_app,
            workflows::duplicar_workflow_app,
            workflows::salvar_workflow_app,
            workflows::restaurar_workflow_app,
            workflows::levar_edicao_workflow_app,
            revisao::revisar_analise_projeto,
            revisao::prontidao_aprovacao_projeto,
            revisao::aprovar_analise_projeto,
            revisao::exportar_analise_projeto,
            revisao::diagramas_exportacao_projeto,
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
