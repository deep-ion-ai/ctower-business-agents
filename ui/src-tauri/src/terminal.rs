//! Comandos Tauri do terminal embutido com o Claude Code.
//!
//! A lógica fica em [`cba_session::terminal`]; aqui só se liga a sessão à janela: a
//! saída e o fim chegam à interface por um `Channel` de cada terminal, e a entrada, o
//! tamanho e o encerramento vêm por comandos. Tudo o que pode bloquear (leitura da
//! configuração do Claude, início do processo, pausa do envio de prompt) roda fora da
//! thread da interface.

use std::sync::Arc;

use cba_core::dados::DiretorioDados;
use cba_index::Cadastro;
use cba_session::clientes::AmbienteReal;
use cba_session::preparo::Preparador;
use cba_session::terminal::{
    self, ErroAbertura, Evento, PedidoClaude, RelatorioErro, Sessoes, Tamanho, UltimaSessao,
    UltimasSessoes,
};
use serde::Serialize;
use tauri::State;
use tauri::ipc::Channel;

/// Terminais abertos pela janela.
pub type Estado = Arc<Sessoes>;

/// Sistema cadastrado, como a tela de escolha mostra.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SistemaResumo {
    pub id: String,
    pub nome: String,
}

fn dados() -> Result<DiretorioDados, String> {
    DiretorioDados::padrao().map_err(|e| format!("diretório de dados do app inacessível: {e}"))
}

async fn bloqueante<T: Send + 'static>(
    f: impl FnOnce() -> T + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| format!("falha interna do terminal: {e}"))
}

/// Sistemas cadastrados, para escolher onde abrir o terminal.
#[tauri::command]
pub async fn listar_sistemas() -> Result<Vec<SistemaResumo>, String> {
    let dados = dados()?;
    bloqueante(move || {
        Cadastro::em(dados.sistemas())
            .listar()
            .map(|lista| {
                lista
                    .into_iter()
                    .map(|s| SistemaResumo {
                        id: s.id,
                        nome: s.nome,
                    })
                    .collect()
            })
            .map_err(|e| e.to_string())
    })
    .await?
}

/// Abre o `claude` num terminal novo, na pasta do sistema. Recusa com orientação se o
/// ambiente não está preparado ou o Claude Code não está instalado. Devolve o
/// identificador do terminal; a saída e o fim chegam por `canal`.
#[tauri::command]
pub async fn abrir_terminal_claude(
    sessoes: State<'_, Estado>,
    sistema: String,
    pedido: Option<PedidoClaude>,
    tamanho: Tamanho,
    canal: Channel<Evento>,
) -> Result<u64, RelatorioErro> {
    let falha = |mensagem: String| ErroAbertura::Falha { mensagem }.relatorio();
    let dados = dados().map_err(falha)?;
    let sessoes = Arc::clone(&sessoes);
    let pedido = pedido.unwrap_or_default();
    bloqueante(move || {
        let cadastrado = Cadastro::em(dados.sistemas())
            .obter(&sistema)
            .map_err(|e| falha(e.to_string()))?
            .ok_or_else(|| {
                ErroAbertura::SistemaNaoCadastrado {
                    sistema: sistema.clone(),
                }
                .relatorio()
            })?;
        let exe = std::env::current_exe()
            .map_err(|e| falha(format!("caminho do executável do app: {e}")))?;
        let preparador = Preparador::padrao(exe).map_err(|e| falha(e.to_string()))?;
        let lancamento = terminal::lancamento(&cadastrado, &pedido, &preparador, &AmbienteReal)
            .map_err(ErroAbertura::relatorio)?;
        let id = sessoes
            .abrir(lancamento.comando, tamanho, move |evento| {
                // A janela pode ter sido fechada; não há o que fazer com o erro.
                let _ = canal.send(evento);
            })
            .map_err(|e| falha(e.to_string()))?;
        if let Err(e) =
            UltimasSessoes::no_diretorio(dados.raiz()).registrar(&sistema, lancamento.id_sessao)
        {
            tracing::warn!(erro = %e, "não foi possível registrar a última sessão do terminal");
        }
        Ok(id)
    })
    .await
    .map_err(falha)?
}

/// Entrada do teclado (o que o xterm.js entrega em `onData`).
#[tauri::command]
pub async fn escrever_terminal(
    sessoes: State<'_, Estado>,
    id: u64,
    dados: String,
) -> Result<(), String> {
    let sessoes = Arc::clone(&sessoes);
    bloqueante(move || sessoes.escrever(id, dados.as_bytes()))
        .await?
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn redimensionar_terminal(
    sessoes: State<'_, Estado>,
    id: u64,
    tamanho: Tamanho,
) -> Result<(), String> {
    sessoes
        .redimensionar(id, tamanho)
        .map_err(|e| e.to_string())
}

/// Envia um prompt pronto à sessão viva, como se o analista o tivesse digitado
/// (inclusive `/desktop`, para continuar no Claude Desktop).
#[tauri::command]
pub async fn enviar_prompt_terminal(
    sessoes: State<'_, Estado>,
    id: u64,
    texto: String,
) -> Result<(), String> {
    let sessoes = Arc::clone(&sessoes);
    bloqueante(move || sessoes.enviar_prompt(id, &texto))
        .await?
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn encerrar_terminal(sessoes: State<'_, Estado>, id: u64) -> Result<(), String> {
    sessoes.encerrar(id).map_err(|e| e.to_string())
}

/// A última sessão do terminal embutido do sistema, para oferecer a retomada.
#[tauri::command]
pub async fn ultima_sessao_terminal(sistema: String) -> Result<Option<UltimaSessao>, String> {
    let dados = dados()?;
    bloqueante(move || UltimasSessoes::no_diretorio(dados.raiz()).obter(&sistema)).await
}
