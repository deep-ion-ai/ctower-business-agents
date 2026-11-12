//! Comandos Tauri dos clientes da conversa: Claude Desktop, terminal do analista e a
//! passagem entre eles (spec, seções 4.2, 4.7, 6.3 e 6.4; #28).
//!
//! A montagem dos links e dos comandos e a escolha do cliente ficam em
//! [`cba_session::passagem`]; o `claude --desktop --resume` em
//! [`cba_session::terminal::desktop`]. Aqui se junta o que a tela da análise precisa
//! (cliente preferido, sessão da análise, pasta do projeto) e se abrem os links.
//!
//! Os links `claude://` e `claude-cli://` são montados e abertos aqui, pelo plugin
//! opener do lado do Rust, a partir do projeto e da análise: a interface nunca passa
//! uma URL, e as permissões da janela (`capabilities/default.json`) continuam
//! liberando só os links oficiais em `https://`.

use cba_core::analise::{Cliente, EstadoAnalise};
use cba_core::dados::DiretorioDados;
use cba_core::tempo::agora_ms;
use cba_session::clientes::{self, AmbienteReal, InstalacaoClaudeDesktop, Plataforma};
use cba_session::passagem::{
    self, AcaoTerminal, ClienteConversa, ClientesDasSessoes, Desktop, OpcaoCliente,
};
use cba_session::terminal::{self, ErroAbertura, RelatorioErro, ResultadoDesktop, Retomada};
use serde::Serialize;
use tauri_plugin_opener::OpenerExt;

use crate::analise::{self, prompt_de_inicio, prompt_de_retomada};
use crate::primeiro_uso::{ClientePreferido, Preferencias};

/// O que a tela da análise oferece para conversar.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpcoesConversa {
    pub plataforma: Plataforma,
    /// O cliente escolhido no primeiro uso.
    pub preferido: Option<ClienteConversa>,
    /// Os três clientes, o preferido (se disponível) primeiro.
    pub opcoes: Vec<OpcaoCliente>,
    /// Sessão mais recente da análise, para retomar pelo identificador.
    pub sessao: Option<String>,
    /// Pedido de uma conversa nova: iniciar a análise recém-criada ou retomá-la.
    pub pedido: String,
    /// `claude://code/new?folder=…&q=<pedido>`.
    pub link_desktop: Option<String>,
    /// `claude-cli://open?cwd=…&q=<pedido>`.
    pub link_terminal: Option<String>,
    /// `cd <pasta> && claude '<pedido>'` (PowerShell no Windows).
    pub comando_terminal: Option<String>,
    /// `cd <pasta> && claude --resume <id>`, quando há sessão.
    pub comando_retomada: Option<String>,
    /// Por que os links ou comandos não puderam ser montados.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub problema: Option<String>,
}

fn preferido(dados: &DiretorioDados) -> Option<ClienteConversa> {
    // Preferências ilegíveis valem como "sem preferência": a tela continua usável.
    Preferencias::ler(dados)
        .ok()
        .and_then(|p| p.cliente)
        .map(|c| match c {
            ClientePreferido::TerminalEmbutido => ClienteConversa::TerminalEmbutido,
            ClientePreferido::ClaudeDesktop => ClienteConversa::ClaudeDesktop,
        })
}

/// O Claude Desktop instalado (detecção pelos locais de instalação, sem executar nada).
fn desktop_real() -> Desktop {
    match clientes::detectar_claude_desktop(&AmbienteReal).instalacao {
        InstalacaoClaudeDesktop::Encontrado { .. } => Desktop::Instalado,
        InstalacaoClaudeDesktop::NaoEncontrado => Desktop::NaoInstalado,
        InstalacaoClaudeDesktop::NaoSuportado => Desktop::NaoSuportado,
    }
}

/// O pedido de uma conversa nova: a análise recém-criada, sem sessão, ainda precisa
/// de `iniciar_analise`; as demais, de `retomar_analise`.
fn pedido(analise: &cba_core::analise::Analise, tem_sessao: bool) -> String {
    if analise.estado == EstadoAnalise::Criada && analise.sessoes.is_empty() && !tem_sessao {
        prompt_de_inicio(&analise.id)
    } else {
        prompt_de_retomada(&analise.id)
    }
}

pub fn opcoes_conversa(
    dados: &DiretorioDados,
    projeto: &str,
    analise: &str,
    desktop: Desktop,
    plataforma: Plataforma,
) -> Result<OpcoesConversa, String> {
    let vista = analise::carregar_analise(dados, projeto, analise)?;
    let alvo = analise::alvo(dados, projeto)?;
    let preferido = preferido(dados);
    let sessao = vista.sessao.map(|s| s.id);
    let pedido = pedido(&vista.analise, sessao.is_some());
    let pasta = &alvo.diretorio;
    let mut problema = None;
    let mut ok = |r: Result<String, passagem::ErroPassagem>| match r {
        Ok(s) => Some(s),
        Err(e) => {
            problema.get_or_insert_with(|| e.to_string());
            None
        }
    };
    let link_desktop = ok(passagem::link_desktop(pasta, Some(&pedido), plataforma));
    let link_terminal = ok(passagem::link_terminal(pasta, Some(&pedido), plataforma));
    let comando_terminal = ok(passagem::comando_terminal(
        pasta,
        AcaoTerminal::Pedido(&pedido),
        plataforma,
    ));
    let comando_retomada = sessao.as_deref().and_then(|id| {
        ok(passagem::comando_terminal(
            pasta,
            AcaoTerminal::Retomar(id),
            plataforma,
        ))
    });
    Ok(OpcoesConversa {
        plataforma,
        preferido,
        opcoes: passagem::opcoes(preferido, desktop),
        sessao,
        pedido,
        link_desktop,
        link_terminal,
        comando_terminal,
        comando_retomada,
        problema,
    })
}

/// O link do cliente (Desktop ou terminal do analista) para uma conversa nova sobre a
/// análise. Marca a análise com o cliente, para a sessão que nascer dele.
pub fn link_da_conversa(
    dados: &DiretorioDados,
    projeto: &str,
    analise: &str,
    cliente: ClienteConversa,
    plataforma: Plataforma,
) -> Result<String, String> {
    let opcoes = opcoes_conversa(dados, projeto, analise, Desktop::Instalado, plataforma)?;
    let link = match cliente {
        ClienteConversa::ClaudeDesktop => opcoes.link_desktop,
        ClienteConversa::TerminalDoAnalista => opcoes.link_terminal,
        ClienteConversa::TerminalEmbutido => {
            return Err("o terminal embutido não abre por link".into());
        }
    }
    .ok_or_else(|| {
        opcoes
            .problema
            .unwrap_or_else(|| "link indisponível".into())
    })?;
    if let Err(e) = ClientesDasSessoes::no_diretorio(dados.raiz()).marcar_analise(
        projeto,
        analise,
        cliente.cliente(),
        agora_ms(),
    ) {
        tracing::warn!(erro = %e, "não foi possível marcar o cliente da análise");
    }
    Ok(link)
}

/// Marca a sessão mais recente da análise com `cliente` (depois do `/desktop`, por
/// exemplo). Sem sessão ligada, marca a análise.
pub fn marcar_cliente(
    dados: &DiretorioDados,
    projeto: &str,
    analise: &str,
    cliente: Cliente,
) -> Result<(), String> {
    let vista = analise::carregar_analise(dados, projeto, analise)?;
    let marcas = ClientesDasSessoes::no_diretorio(dados.raiz());
    match vista.sessao {
        Some(s) => marcas.marcar_sessao(&s.id, cliente, agora_ms()),
        None => marcas.marcar_analise(projeto, analise, cliente, agora_ms()),
    }
    .map_err(|e| format!("não foi possível marcar o cliente da sessão: {e}"))
}

/// Sessão mais recente da análise, pelo registro dos hooks.
fn sessao_da_analise(
    dados: &DiretorioDados,
    projeto: &str,
    analise: &str,
) -> Result<Option<String>, String> {
    Ok(analise::carregar_analise(dados, projeto, analise)?
        .sessao
        .map(|s| s.id))
}

// ---------------------------------------------------------------------------
// Comandos

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

#[tauri::command]
pub async fn opcoes_conversa_analise(
    projeto: String,
    analise: String,
) -> Result<OpcoesConversa, String> {
    let dados = dados()?;
    bloqueante(move || {
        opcoes_conversa(
            &dados,
            &projeto,
            &analise,
            desktop_real(),
            Plataforma::atual(),
        )
    })
    .await
}

/// Abre o link de uma conversa nova no Desktop ou no terminal do analista.
#[tauri::command]
pub async fn abrir_conversa_por_link(
    app: tauri::AppHandle,
    projeto: String,
    analise: String,
    cliente: ClienteConversa,
) -> Result<(), String> {
    let dados = dados()?;
    let link = bloqueante(move || {
        link_da_conversa(&dados, &projeto, &analise, cliente, Plataforma::atual())
    })
    .await?;
    tracing::info!(?cliente, "abrindo a conversa por link");
    app.opener()
        .open_url(link, None::<&str>)
        .map_err(|e| format!("não foi possível abrir o link: {e}"))
}

/// Depois do `/desktop` no terminal embutido: a sessão passou ao Desktop.
#[tauri::command]
pub async fn conversa_passou_ao_desktop(projeto: String, analise: String) -> Result<(), String> {
    let dados = dados()?;
    bloqueante(move || marcar_cliente(&dados, &projeto, &analise, Cliente::ClaudeDesktop)).await
}

/// Abre no Desktop a sessão da análise (`claude --desktop --resume <id>`, num PTY), ou
/// a mais recente da pasta (`--continue`) se nenhuma estiver ligada à análise.
#[tauri::command]
pub async fn retomar_conversa_no_desktop(
    projeto: String,
    analise: String,
) -> Result<ResultadoDesktop, RelatorioErro> {
    let falha = |mensagem: String| ErroAbertura::Falha { mensagem }.relatorio();
    let dados = dados().map_err(falha)?;
    tauri::async_runtime::spawn_blocking(move || {
        let cadastrado = cba_index::Cadastro::em(dados.projetos())
            .obter(&projeto)
            .map_err(|e| falha(e.to_string()))?
            .ok_or_else(|| {
                ErroAbertura::ProjetoNaoCadastrado {
                    projeto: projeto.clone(),
                }
                .relatorio()
            })?;
        let retomada = match sessao_da_analise(&dados, &projeto, &analise).map_err(falha)? {
            Some(id) => Retomada::Sessao { id },
            None => Retomada::Continuar,
        };
        let exe = std::env::current_exe()
            .map_err(|e| falha(format!("caminho do executável do app: {e}")))?;
        let preparador =
            cba_session::preparo::Preparador::padrao(exe).map_err(|e| falha(e.to_string()))?;
        let l = terminal::lancamento_desktop(&cadastrado, &retomada, &preparador, &AmbienteReal)
            .map_err(ErroAbertura::relatorio)?;
        let id = l.id_sessao.clone();
        let r = terminal::executar_no_pty(l.comando, terminal::desktop::LIMITE_DESKTOP)
            .map_err(|e| falha(e.to_string()))?;
        tracing::info!(aberto = r.aberto, codigo = ?r.codigo, "claude --desktop");
        if r.aberto {
            let marcas = ClientesDasSessoes::no_diretorio(dados.raiz());
            let marcado = match id {
                Some(id) => marcas.marcar_sessao(&id, Cliente::ClaudeDesktop, agora_ms()),
                None => {
                    marcas.marcar_analise(&projeto, &analise, Cliente::ClaudeDesktop, agora_ms())
                }
            };
            if let Err(e) = marcado {
                tracing::warn!(erro = %e, "não foi possível marcar o cliente da sessão");
            }
        }
        Ok(r)
    })
    .await
    .map_err(|e| falha(format!("falha interna: {e}")))?
}

#[cfg(test)]
mod tests;
