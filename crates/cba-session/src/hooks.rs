//! Receptor dos hooks do Claude Code (e da aba de código do Claude Desktop).
//!
//! O cliente de IA executa `cba hook <evento>` com o JSON do evento na entrada padrão
//! e lê a resposta na saída padrão (spec, seções 5 e 6.2). O receptor acompanha o
//! estado das sessões ([`crate::sessoes`]) e o leva para a análise ligada a cada uma
//! ([`cba_core::analise::Sessao`]), de onde o app o mostra ao vivo.
//!
//! # Regras
//!
//! - **Nunca bloqueia.** A resposta é sempre código 0 com um JSON válido (`{}`, ou o
//!   contexto do `SessionStart`). Entrada inválida, evento desconhecido, base
//!   inacessível ou falha ao gravar só vão para o log.
//! - **Rápido** (meta: menos de 200 ms). Os hooks ficam no escopo de usuário (#24) e
//!   disparam em todas as sessões do analista; fora das pastas dos sistemas
//!   cadastrados, o receptor só lê o cadastro e sai, sem gravar nada. Dentro, só
//!   grava quando algo muda (o `PreToolUse` de uma sessão já ativa não grava).
//! - **Sem conteúdo no log** (spec, seção 9): o log registra o evento, a duração e o
//!   desfecho, nunca o JSON recebido, o identificador da sessão ou caminhos.
//!
//! # Qual sistema
//!
//! Uma sessão já registrada continua no sistema do registro. Uma nova é do sistema
//! cadastrado cuja pasta contém o `cwd` do evento ou, se não houver, o
//! `CLAUDE_PROJECT_DIR` do processo (a pasta em que a sessão começou). Uma worktree
//! do git conta como o repositório principal (`raiz_principal` de
//! [`cba_index::detectar_git`]). Os hooks não recebem o `CBA_SISTEMA` do servidor
//! MCP.
//!
//! # Estados
//!
//! | Evento | Estado da sessão |
//! |---|---|
//! | `SessionStart` | ativa (reabre uma encerrada: `claude --resume`) |
//! | `UserPromptSubmit`, `PreToolUse` | ativa |
//! | `Notification` de permissão, de pergunta ao analista ou sem tipo | aguardando o analista |
//! | `Notification` de outros tipos (`idle_prompt`, `auth_success`...) | sem mudança |
//! | `Stop` | concluída (o agente terminou a resposta) |
//! | `SessionEnd` | encerrada |
//!
//! # Ligação com a análise
//!
//! O `PreToolUse` de uma ferramenta do servidor `cba` (`mcp__cba__<ferramenta>`)
//! com o argumento `analise` (`A-NNNN`), normalmente `iniciar_analise` ou
//! `retomar_analise`, liga a sessão àquela análise: o registro passa a citá-la e a
//! análise ganha a sessão ([`cba_core::analise::Analise::iniciar_sessao`]), com o
//! `session_id` guardado para a retomada (#28). Daí em diante, cada mudança de
//! estado vai também para a análise, que notifica o app pelo diário de eventos.
//!
//! Fontes (documentação oficial, <https://code.claude.com/docs/en/hooks.md>,
//! consultada em 2026-11-11): campos comuns (`session_id`, `cwd`, `hook_event_name`),
//! `source` do `SessionStart`, `notification_type` do `Notification`, `reason` do
//! `SessionEnd`, nome `mcp__<servidor>__<ferramenta>` no `PreToolUse`, hooks em
//! paralelo, `CLAUDE_PROJECT_DIR` e `hookSpecificOutput.additionalContext` do
//! `SessionStart`.

use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use cba_core::analise::{Analise, Cliente, ErroModelo, EstadoSessao};
use cba_core::analises::Analises;
use cba_core::config::Config;
use cba_core::dados::DiretorioDados;
use cba_core::eventos::{DiarioEventos, Mudanca};
use cba_core::tempo;
use cba_kb::{Base, Id, Serie};
use serde_json::{Value, json};

use crate::preparo::NOME_SERVIDOR;
use crate::sessoes::{Alteracao, RegistroSessao, Sessoes, id_valido};

/// Eventos que o app registra na configuração do cliente (spec, seção 6.2).
pub const EVENTOS: &[&str] = &[
    "SessionStart",
    "UserPromptSubmit",
    "PreToolUse",
    "Notification",
    "Stop",
    "SessionEnd",
];

/// Variável de ambiente com a pasta em que a sessão começou (documentada pelo
/// Claude Code para os comandos de hook).
pub const VAR_PROJETO: &str = "CLAUDE_PROJECT_DIR";

/// Tipos de `Notification` em que o cliente espera uma ação do analista.
const NOTIFICACOES_DE_ESPERA: &[&str] = &[
    "permission_prompt",
    "elicitation_dialog",
    "elicitation_url_dialog",
    "agent_needs_input",
];

/// O que o processo do hook devolve ao cliente.
#[derive(Debug, Clone, PartialEq)]
pub struct Resposta {
    /// JSON escrito na saída padrão.
    pub saida: Value,
    /// Código de saída do processo.
    pub codigo: i32,
}

impl Resposta {
    /// `{}` com código 0: o cliente segue normalmente.
    pub fn seguir() -> Self {
        Self {
            saida: Value::Object(Default::default()),
            codigo: 0,
        }
    }

    /// Segue normalmente e acrescenta `texto` ao contexto do agente
    /// (`hookSpecificOutput.additionalContext`; vale para `SessionStart` e
    /// `UserPromptSubmit`).
    pub fn com_contexto(evento: &str, texto: &str) -> Self {
        Self {
            saida: json!({
                "hookSpecificOutput": {
                    "hookEventName": evento,
                    "additionalContext": texto,
                }
            }),
            codigo: 0,
        }
    }
}

/// O que o receptor precisa do processo: o diretório de dados e a pasta do projeto.
#[derive(Debug, Clone, Default)]
pub struct Contexto {
    /// Diretório de dados do app; sem ele, o receptor só responde.
    pub dados: Option<DiretorioDados>,
    /// `CLAUDE_PROJECT_DIR`, quando definido.
    pub diretorio_projeto: Option<PathBuf>,
}

impl Contexto {
    /// Contexto do processo atual (`CBA_DATA_DIR` ou o diretório padrão, e
    /// `CLAUDE_PROJECT_DIR`).
    pub fn do_processo() -> Self {
        Self {
            dados: DiretorioDados::padrao().ok(),
            diretorio_projeto: std::env::var_os(VAR_PROJETO)
                .filter(|v| !v.is_empty())
                .map(PathBuf::from),
        }
    }
}

/// Início de uma sessão num sistema cadastrado, para o contexto do `SessionStart`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InicioDeSessao {
    /// Slug do sistema.
    pub sistema: String,
    /// Análise já ligada à sessão (numa retomada).
    pub analise: Option<Id>,
    /// `source` do evento (`startup`, `resume`, `clear`, `compact`, `fork`).
    pub origem: Option<String>,
}

/// Texto que o `SessionStart` acrescenta ao contexto do agente numa sessão de
/// sistema cadastrado: os limites fixos e as regras do sistema (spec, seção 8).
///
/// Ponto de extensão da #31; por enquanto não há conteúdo, e a resposta é `{}`.
pub fn contexto_inicial(_inicio: &InicioDeSessao) -> Option<String> {
    None
}

/// Como terminou o tratamento de um evento (para o log, sem identificadores).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Desfecho {
    /// Entrada ilegível ou incoerente.
    EntradaInvalida(&'static str),
    /// Evento fora de [`EVENTOS`].
    EventoDesconhecido,
    /// Sem diretório de dados.
    SemDados,
    /// Sem `session_id` utilizável.
    SemSessao,
    /// Sessão fora das pastas dos sistemas cadastrados: nada gravado.
    ForaDeSistema,
    /// Sessão de um sistema; `gravou` diz se o registro mudou.
    Sessao {
        inicio: Option<InicioDeSessao>,
        gravou: bool,
    },
    /// Falha ao ler ou gravar; a sessão segue assim mesmo.
    Falha(&'static str),
}

impl Desfecho {
    /// Nome curto, para o log.
    pub fn tipo(&self) -> &'static str {
        match self {
            Self::EntradaInvalida(motivo) => motivo,
            Self::EventoDesconhecido => "evento_desconhecido",
            Self::SemDados => "sem_dados",
            Self::SemSessao => "sem_sessao",
            Self::ForaDeSistema => "fora_de_sistema",
            Self::Sessao { gravou: true, .. } => "registrado",
            Self::Sessao { gravou: false, .. } => "sem_mudanca",
            Self::Falha(motivo) => motivo,
        }
    }
}

/// Lê o JSON do evento em `entrada`, atualiza o estado da sessão e decide a
/// resposta.
pub fn tratar(evento: &str, mut entrada: impl Read, contexto: &Contexto) -> Resposta {
    let inicio = Instant::now();
    let mut texto = String::new();
    let desfecho = match entrada.read_to_string(&mut texto) {
        Err(_) => Desfecho::EntradaInvalida("entrada_ilegivel"),
        Ok(_) => match serde_json::from_str::<Value>(&texto) {
            Err(_) => Desfecho::EntradaInvalida("json_invalido"),
            Ok(dados) => receber(evento, &dados, contexto),
        },
    };

    let resposta = match &desfecho {
        Desfecho::Sessao {
            inicio: Some(inicio),
            ..
        } => match contexto_inicial(inicio) {
            Some(texto) => Resposta::com_contexto(evento, &texto),
            None => Resposta::seguir(),
        },
        _ => Resposta::seguir(),
    };

    let duracao_us = inicio.elapsed().as_micros() as u64;
    let desfecho_tipo = desfecho.tipo();
    match desfecho {
        Desfecho::EntradaInvalida(_) | Desfecho::EventoDesconhecido | Desfecho::Falha(_) => {
            tracing::warn!(evento, duracao_us, desfecho = desfecho_tipo, "hook")
        }
        _ => tracing::info!(evento, duracao_us, desfecho = desfecho_tipo, "hook"),
    }
    resposta
}

/// Atalho para o modo `cba hook`: lê da entrada padrão, com o contexto do processo.
pub fn tratar_stdin(evento: &str) -> Resposta {
    tratar(evento, io::stdin().lock(), &Contexto::do_processo())
}

fn texto_de<'a>(dados: &'a Value, campo: &str) -> Option<&'a str> {
    dados.get(campo).and_then(Value::as_str)
}

/// Estado a que o evento leva a sessão, se algum.
fn destino(evento: &str, dados: &Value) -> Option<EstadoSessao> {
    match evento {
        "SessionStart" | "UserPromptSubmit" | "PreToolUse" => Some(EstadoSessao::Ativa),
        "Notification" => match texto_de(dados, "notification_type") {
            None => Some(EstadoSessao::AguardandoAnalista),
            Some(t) if NOTIFICACOES_DE_ESPERA.contains(&t) => {
                Some(EstadoSessao::AguardandoAnalista)
            }
            Some(_) => None,
        },
        "Stop" => Some(EstadoSessao::Concluida),
        "SessionEnd" => Some(EstadoSessao::Encerrada),
        _ => None,
    }
}

/// Análise citada por uma ferramenta do servidor `cba` no `PreToolUse`.
fn analise_da_ferramenta(dados: &Value) -> Option<Id> {
    let ferramenta = texto_de(dados, "tool_name")?;
    ferramenta
        .strip_prefix("mcp__")?
        .strip_prefix(NOME_SERVIDOR)?
        .strip_prefix("__")?;
    let id = Id::analisar(dados.get("tool_input")?.get("analise")?.as_str()?).ok()?;
    (id.serie() == &Serie::Analise).then_some(id)
}

fn receber(evento: &str, dados: &Value, contexto: &Contexto) -> Desfecho {
    if !EVENTOS.contains(&evento) {
        return Desfecho::EventoDesconhecido;
    }
    // O cliente repete o nome do evento no JSON; divergência indica configuração
    // errada, e é melhor não mexer no estado.
    if texto_de(dados, "hook_event_name").is_some_and(|n| n != evento) {
        return Desfecho::EntradaInvalida("evento_divergente");
    }
    let Some(dados_app) = &contexto.dados else {
        return Desfecho::SemDados;
    };
    let Some(id) = texto_de(dados, "session_id").filter(|id| id_valido(id)) else {
        return Desfecho::SemSessao;
    };
    let cwd = texto_de(dados, "cwd").map(PathBuf::from);
    let sessoes = Sessoes::do_diretorio(dados_app);

    // Caminho rápido: sessão já conhecida não precisa do cadastro.
    let sistema = match sessoes.obter(id) {
        Err(_) => return Desfecho::Falha("sessoes_ilegiveis"),
        Ok(Some(r)) => r.sistema,
        Ok(None) => {
            let candidatos = cwd.iter().chain(contexto.diretorio_projeto.iter());
            match sistema_de(candidatos, dados_app) {
                Ok(Some(s)) => s,
                Ok(None) => return Desfecho::ForaDeSistema,
                Err(motivo) => return Desfecho::Falha(motivo),
            }
        }
    };

    let agora = tempo::agora_ms();
    let destino = destino(evento, dados);
    let ligar = (evento == "PreToolUse")
        .then(|| analise_da_ferramenta(dados))
        .flatten();
    let origem = (evento == "SessionStart")
        .then(|| texto_de(dados, "source").map(str::to_owned))
        .flatten();
    let motivo_fim = (evento == "SessionEnd")
        .then(|| texto_de(dados, "reason").map(str::to_owned))
        .flatten();
    let diretorio = contexto.diretorio_projeto.clone().or(cwd);

    let resultado = sessoes.alterar_com(
        id,
        |anterior| {
            let mut r = anterior.clone().unwrap_or_else(|| {
                let mut r = RegistroSessao::novo(id, &sistema, agora);
                r.diretorio = diretorio;
                r
            });
            if origem.is_some() {
                r.origem = origem;
            }
            if let Some(d) = destino
                && r.mudar_estado(d, evento == "SessionStart", agora)
                && d == EstadoSessao::Encerrada
            {
                r.motivo_fim = motivo_fim;
            }
            if let Some(a) = ligar {
                r.analise = Some(a);
            }
            let mudou = anterior.as_ref() != Some(&r);
            mudou.then(|| {
                r.atualizada_ms = agora;
                r
            })
        },
        |alteracao| levar_para_analises(alteracao, dados_app),
    );
    match resultado {
        Err(_) => Desfecho::Falha("gravacao_da_sessao"),
        Ok((alteracao, ())) => {
            let atual = alteracao.novo.as_ref().or(alteracao.anterior.as_ref());
            let inicio = (evento == "SessionStart").then(|| InicioDeSessao {
                sistema: sistema.clone(),
                analise: atual.and_then(|r| r.analise.clone()),
                origem: atual.and_then(|r| r.origem.clone()),
            });
            Desfecho::Sessao {
                inicio,
                gravou: alteracao.mudou(),
            }
        }
    }
}

/// Sistema cadastrado da primeira pasta de `pastas` que pertence a algum.
fn sistema_de<'a>(
    pastas: impl Iterator<Item = &'a PathBuf>,
    dados: &DiretorioDados,
) -> Result<Option<String>, &'static str> {
    let mut sistemas = None;
    for pasta in pastas {
        // O cadastro só é lido se houver uma pasta para conferir, e uma vez só.
        if sistemas.is_none() {
            let lista = cba_index::Cadastro::em(dados.sistemas())
                .listar()
                .map_err(|_| "cadastro_ilegivel")?;
            if lista.is_empty() {
                return Ok(None);
            }
            sistemas = Some(lista);
        }
        if let Some(s) = sistema_da_pasta(pasta, sistemas.as_deref().unwrap_or_default()) {
            return Ok(Some(s));
        }
    }
    Ok(None)
}

/// Sistema cadastrado que contém `pasta`, vencendo a pasta cadastrada mais
/// específica. Numa worktree do git, vale o caminho correspondente no repositório
/// principal. Não lê o escopo nem as exclusões dos sistemas: só os caminhos.
pub fn sistema_da_pasta(pasta: &Path, sistemas: &[cba_index::Sistema]) -> Option<String> {
    let pasta = dunce::canonicalize(pasta).ok()?;
    if let Some(s) = mais_especifico(&pasta, sistemas) {
        return Some(s);
    }
    let git = cba_index::detectar_git(&pasta)?;
    if !git.worktree {
        return None;
    }
    let relativo = pasta.strip_prefix(&git.raiz).ok()?;
    mais_especifico(&git.raiz_principal.join(relativo), sistemas)
}

fn mais_especifico(pasta: &Path, sistemas: &[cba_index::Sistema]) -> Option<String> {
    sistemas
        .iter()
        .flat_map(|s| s.pastas.iter().map(move |p| (s, &p.caminho)))
        .filter(|(_, caminho)| pasta.starts_with(caminho))
        .max_by_key(|(_, caminho)| caminho.components().count())
        .map(|(s, _)| s.id.clone())
}

/// Leva a mudança do registro para as análises: a sessão entra na análise ligada
/// (ou muda de estado nela) e sai como concluída de uma análise anterior. Falhas só
/// vão para o log: o registro da sessão já está gravado.
fn levar_para_analises(alteracao: &Alteracao, dados: &DiretorioDados) {
    let Some(novo) = &alteracao.novo else {
        return;
    };
    let anterior = alteracao.anterior.as_ref();
    let analise_anterior = anterior.and_then(|r| r.analise.as_ref());
    let mudou_estado = anterior.is_none_or(|r| r.estado != novo.estado);
    let religou = analise_anterior != novo.analise.as_ref();
    let Some(analise) = &novo.analise else {
        return;
    };
    if !mudou_estado && !religou {
        return;
    }
    let analises = match analises_do_sistema(dados, &novo.sistema) {
        Ok(a) => a,
        Err(motivo) => {
            tracing::warn!(motivo, "estado da sessão não levado à análise");
            return;
        }
    };
    if religou && let Some(antiga) = analise_anterior {
        alterar_analise(&analises, antiga, |a, agora| {
            if a.sessoes.iter().any(|s| s.id == novo.id) {
                a.atualizar_sessao(&novo.id, EstadoSessao::Concluida, agora)
            } else {
                Ok(())
            }
        });
    }
    alterar_analise(&analises, analise, |a, agora| {
        sincronizar(a, &novo.id, novo.cliente, novo.estado, agora)
    });
}

fn alterar_analise(
    analises: &Analises,
    id: &Id,
    alterar: impl FnOnce(&mut Analise, u64) -> Result<(), ErroModelo>,
) {
    if let Err(e) = analises.alterar(id, Mudanca::Sessao, alterar) {
        tracing::warn!(
            erro = cba_core::analises::tipo_do_erro(&e),
            "estado da sessão não levado à análise"
        );
    }
}

/// Põe a sessão `id` no estado `destino` dentro da análise, ligando-a se ainda não
/// estiver (ou reabrindo-a, se estava encerrada).
pub fn sincronizar(
    analise: &mut Analise,
    id: &str,
    cliente: Cliente,
    destino: EstadoSessao,
    agora_ms: u64,
) -> Result<(), ErroModelo> {
    let atual = analise
        .sessoes
        .iter()
        .find(|s| s.id == id)
        .map(|s| s.estado);
    match atual {
        Some(e) if e == destino => Ok(()),
        Some(e) if e != EstadoSessao::Encerrada || destino == EstadoSessao::Encerrada => {
            analise.atualizar_sessao(id, destino, agora_ms)
        }
        // Ausente ou encerrada voltando: liga (ou reabre) e então muda.
        _ => {
            analise.iniciar_sessao(id, cliente, agora_ms)?;
            if destino == EstadoSessao::Ativa {
                Ok(())
            } else {
                analise.atualizar_sessao(id, destino, agora_ms)
            }
        }
    }
}

/// Análises do sistema na base configurada, sem montar o ambiente inteiro (sem
/// escopo nem exclusões, e sem criar nada na base).
fn analises_do_sistema(dados: &DiretorioDados, sistema: &str) -> Result<Analises, &'static str> {
    let config = Config::ler(dados).map_err(|_| "config_ilegivel")?;
    let raiz = config.base().ok_or("base_nao_configurada")?;
    let base = Base::abrir(raiz).map_err(|_| "base_inacessivel")?;
    let kb = base.sistema(sistema).map_err(|_| "sistema_fora_da_base")?;
    Ok(Analises::new(
        kb,
        Arc::new(DiarioEventos::em(dados.eventos())),
    ))
}

#[cfg(test)]
mod tests;
