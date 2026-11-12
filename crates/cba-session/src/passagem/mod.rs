//! Como a conversa abre em cada cliente e passa de um para o outro (spec, seções 4.2,
//! 4.7, 6.3 e 6.4).
//!
//! O analista conversa por um de três clientes: o Claude Code no terminal embutido do
//! app ([`crate::terminal`]), o Claude Desktop ou o Claude Code no terminal dele. Este
//! módulo monta, sem efeito colateral, o que o app oferece para cada um:
//!
//! | Cliente | Conversa nova (com o pedido) | Retomada pelo identificador |
//! |---|---|---|
//! | Terminal embutido | `claude --session-id <uuid> "<pedido>"` ([`crate::terminal`]) | `claude --resume <id>` |
//! | Claude Desktop | [`link_desktop`]: `claude://code/new?folder=<pasta>&q=<pedido>` | `claude --desktop --resume <id>`, num PTY ([`crate::terminal::desktop`]) |
//! | Terminal do analista | [`comando_terminal`] (`cd <pasta> && claude '<pedido>'`) ou [`link_terminal`] (`claude-cli://open?cwd=<pasta>&q=<pedido>`) | [`comando_terminal`] com `claude --resume <id>` |
//!
//! Do terminal embutido para o Desktop, com a sessão viva, o app digita `/desktop`
//! ([`crate::terminal::PROMPT_DESKTOP`]): o Claude Code salva a sessão, abre-a no
//! Desktop e **encerra o CLI**.
//!
//! # Fontes (consultadas em 2026-11-11)
//!
//! - Links `claude-cli://` (parâmetros `cwd` e `q`, `q` codificado como no
//!   `encodeURIComponent`, até 5.000 caracteres, `%0A` para quebra de linha; `cwd`
//!   absoluto, sem caminho de rede nem `..` nem caracteres invisíveis; o pedido é
//!   preenchido e nunca enviado sozinho): <https://code.claude.com/docs/en/deep-links>.
//! - Links `claude://code/new` (o Desktop preenche o pedido de `q`, URL-encoded, e
//!   nunca o envia sozinho): <https://code.claude.com/docs/en/desktop>, seção "Open an
//!   SSH session from a link". A página documenta só os parâmetros de SSH
//!   (`ssh_host`, `ssh_port`, `ssh_folder`) e `q`; o parâmetro `folder` de uma pasta
//!   local vem da prova de conceito da spec (seção 13) e precisa ser conferido à mão
//!   a cada versão do Desktop (#42).
//! - `/desktop` ("saves your session and opens it in the desktop app, then exits the
//!   CLI"; só macOS e Windows x64, com assinatura do Claude) e `claude --desktop
//!   --resume <session-id>` (Claude Code 2.1.285 ou mais novo; só o identificador, não
//!   o nome; não move uma sessão aberta noutro terminal): mesma página, seção "Coming
//!   from the CLI?", e <https://code.claude.com/docs/en/cli-reference>.
//! - `--resume` mantém o identificador da sessão: `--fork-session` é a opção que
//!   "create\[s\] a new session ID instead of reusing the original" (cli-reference).
//!
//! # Quem informa o cliente de uma sessão
//!
//! Os hooks não dizem qual cliente os disparou (o Desktop roda a própria cópia do
//! Claude Code, com os mesmos eventos), então o registro da sessão (#25) nasce com o
//! cliente `desconhecido`. Quem informa é o app, pelo que ele mesmo abriu
//! ([`ClientesDasSessoes`]): o terminal embutido marca o identificador que passou ao
//! `claude`; o `/desktop` e o `--desktop --resume` marcam a sessão como do Desktop; os
//! links (Desktop e terminal do analista), que não sabem o identificador, marcam a
//! análise, e a marca vale para a sessão ligada a ela que começar depois.

mod marcas;

#[cfg(test)]
mod tests;

use std::fmt::Write as _;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::clientes::Plataforma;
use crate::terminal::claude::limpar_prompt;

pub use marcas::ClientesDasSessoes;

/// Tamanho máximo do pedido num link (`q` do `claude-cli://`; o Desktop não documenta
/// limite e recebe o mesmo).
pub const MAX_PEDIDO: usize = 5_000;

/// Por onde o analista conversa com a IA.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ClienteConversa {
    /// Claude Code no terminal embutido do app (recomendado).
    TerminalEmbutido,
    /// Aba de código do Claude Desktop (macOS e Windows x64).
    ClaudeDesktop,
    /// Claude Code no terminal do próprio analista.
    TerminalDoAnalista,
}

impl ClienteConversa {
    /// Ordem padrão, quando o analista não escolheu.
    pub const TODOS: [ClienteConversa; 3] = [
        ClienteConversa::TerminalEmbutido,
        ClienteConversa::ClaudeDesktop,
        ClienteConversa::TerminalDoAnalista,
    ];

    /// O cliente de IA, como o estado da análise o guarda.
    pub fn cliente(self) -> cba_core::analise::Cliente {
        match self {
            ClienteConversa::ClaudeDesktop => cba_core::analise::Cliente::ClaudeDesktop,
            _ => cba_core::analise::Cliente::ClaudeCode,
        }
    }
}

/// Um cliente oferecido na tela da análise.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpcaoCliente {
    pub cliente: ClienteConversa,
    pub disponivel: bool,
    /// Por que não está disponível, em linguagem do analista.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub motivo: Option<String>,
}

/// Situação do Claude Desktop para a escolha.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Desktop {
    Instalado,
    NaoInstalado,
    /// Não existe para a plataforma (Linux).
    NaoSuportado,
}

/// Os três clientes, o preferido primeiro e os demais na ordem padrão. O Desktop
/// indisponível vai para o fim, com o motivo; se era o preferido, o terminal embutido
/// passa à frente.
pub fn opcoes(preferido: Option<ClienteConversa>, desktop: Desktop) -> Vec<OpcaoCliente> {
    let mut ordem: Vec<ClienteConversa> = preferido.into_iter().collect();
    ordem.extend(
        ClienteConversa::TODOS
            .iter()
            .filter(|c| Some(**c) != preferido),
    );
    let motivo_desktop = match desktop {
        Desktop::Instalado => None,
        Desktop::NaoInstalado => Some("o Claude Desktop não foi encontrado neste computador"),
        Desktop::NaoSuportado => Some("o Claude Desktop não existe para Linux"),
    };
    let mut lista: Vec<OpcaoCliente> = ordem
        .into_iter()
        .map(|cliente| {
            let motivo = (cliente == ClienteConversa::ClaudeDesktop)
                .then_some(motivo_desktop)
                .flatten();
            OpcaoCliente {
                cliente,
                disponivel: motivo.is_none(),
                motivo: motivo.map(str::to_owned),
            }
        })
        .collect();
    // Estável: os disponíveis mantêm a ordem entre si.
    lista.sort_by_key(|o| !o.disponivel);
    lista
}

/// Por que um link ou comando não pôde ser montado.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ErroPassagem {
    #[error("pasta do sistema inválida para o link: {0}")]
    PastaInvalida(String),
    #[error("o pedido passa de {MAX_PEDIDO} caracteres, o limite dos links do Claude")]
    PedidoLongo,
    #[error("identificador de sessão inválido (só letras, números, - e _)")]
    IdInvalido,
}

/// `claude://code/new?folder=<pasta>&q=<pedido>`: abre a aba de código do Claude
/// Desktop numa sessão nova, na pasta, com o pedido preenchido. O Desktop pede ao
/// analista que confirme a pasta e não envia o pedido sozinho (spec, seção 6.3).
/// Sem pedido, o link não leva `q`.
pub fn link_desktop(
    pasta: &Path,
    pedido: Option<&str>,
    plataforma: Plataforma,
) -> Result<String, ErroPassagem> {
    let pasta = pasta_do_link(pasta, plataforma)?;
    let mut link = format!("claude://code/new?folder={}", codificar(pasta, b"/"));
    if let Some(q) = pedido_do_link(pedido)? {
        let _ = write!(link, "&q={}", codificar(&q, b""));
    }
    Ok(link)
}

/// `claude-cli://open?cwd=<pasta>&q=<pedido>`: o Claude Code registrado no sistema
/// abre um terminal novo na pasta com o pedido preenchido, que o analista revisa e
/// envia com Enter. O registro do link acontece na primeira mensagem enviada numa
/// sessão interativa do `claude` naquela máquina.
pub fn link_terminal(
    pasta: &Path,
    pedido: Option<&str>,
    plataforma: Plataforma,
) -> Result<String, ErroPassagem> {
    let pasta = pasta_do_link(pasta, plataforma)?;
    let mut link = format!("claude-cli://open?cwd={}", codificar(pasta, b"/"));
    if let Some(q) = pedido_do_link(pedido)? {
        let _ = write!(link, "&q={}", codificar(&q, b""));
    }
    Ok(link)
}

/// O que o comando do terminal do analista faz.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcaoTerminal<'a> {
    /// Conversa nova com o pedido (que o `claude` envia ao abrir).
    Pedido(&'a str),
    /// `claude --resume <id>`.
    Retomar(&'a str),
}

/// Comando pronto para copiar no terminal do analista: entra na pasta e roda o
/// `claude`. No macOS e no Linux, para `sh`, `bash` e `zsh`
/// (`cd '/pasta' && claude 'pedido'`); no Windows, para o PowerShell, que é o terminal
/// padrão (`Set-Location -LiteralPath 'C:\pasta'; claude 'pedido'`). As aspas
/// simples não interpretam nada nos dois. Quebras de linha do pedido viram espaço.
pub fn comando_terminal(
    pasta: &Path,
    acao: AcaoTerminal<'_>,
    plataforma: Plataforma,
) -> Result<String, ErroPassagem> {
    let pasta = pasta
        .to_str()
        .ok_or_else(|| ErroPassagem::PastaInvalida("caminho com caracteres inválidos".into()))?;
    if pasta.is_empty() || pasta.chars().any(char::is_control) {
        return Err(ErroPassagem::PastaInvalida(
            "caminho vazio ou com caracteres de controle".into(),
        ));
    }
    let argumentos = match acao {
        AcaoTerminal::Retomar(id) => {
            if !crate::terminal::claude::id_valido(id) {
                return Err(ErroPassagem::IdInvalido);
            }
            format!(" --resume {id}")
        }
        AcaoTerminal::Pedido(pedido) => match limpar_prompt(pedido) {
            Some(p) => {
                let uma_linha = p.replace(['\n', '\t'], " ");
                format!(" {}", aspas(&uma_linha, plataforma))
            }
            None => String::new(),
        },
    };
    Ok(match plataforma {
        Plataforma::Windows => format!(
            "Set-Location -LiteralPath {}; claude{argumentos}",
            aspas(pasta, plataforma)
        ),
        _ => format!("cd {} && claude{argumentos}", aspas_se_preciso(pasta)),
    })
}

/// Entre aspas simples, que nem o `sh` nem o PowerShell interpretam.
fn aspas(texto: &str, plataforma: Plataforma) -> String {
    match plataforma {
        // No PowerShell, `'` (e as aspas simples tipográficas, que ele também aceita)
        // se repete dentro das aspas.
        Plataforma::Windows => {
            let mut s = String::from("'");
            for c in texto.chars() {
                if matches!(c, '\'' | '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}') {
                    s.push(c);
                }
                s.push(c);
            }
            s.push('\'');
            s
        }
        // No `sh`, a aspa simples fecha, entra escapada e reabre: `'\''`.
        _ => format!("'{}'", texto.replace('\'', r"'\''")),
    }
}

fn aspas_se_preciso(texto: &str) -> String {
    let simples = texto
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '-' | '~' | '+'))
        && !texto.starts_with('~');
    if simples {
        texto.to_owned()
    } else {
        aspas(texto, Plataforma::Linux)
    }
}

/// O pedido como vai no link: sem caracteres de controle (quebras de linha ficam),
/// sem espaços nas pontas, até [`MAX_PEDIDO`] caracteres. Vazio vira `None`.
fn pedido_do_link(pedido: Option<&str>) -> Result<Option<String>, ErroPassagem> {
    let Some(p) = pedido.and_then(limpar_prompt) else {
        return Ok(None);
    };
    // No link, o pedido não vira argumento: o espaço que protege um `-` inicial sai.
    let p = p.trim_start().to_owned();
    if p.chars().count() > MAX_PEDIDO {
        return Err(ErroPassagem::PedidoLongo);
    }
    Ok(Some(p))
}

/// A pasta como os links aceitam: absoluta, local (sem caminho de rede), sem `..` e
/// sem caracteres de controle, invisíveis ou bidirecionais (regras do `cwd` do
/// `claude-cli://`, aplicadas também ao Desktop).
fn pasta_do_link(pasta: &Path, plataforma: Plataforma) -> Result<&str, ErroPassagem> {
    let invalida = |m: &str| ErroPassagem::PastaInvalida(m.to_owned());
    let texto = pasta
        .to_str()
        .ok_or_else(|| invalida("caminho com caracteres inválidos"))?;
    // `\\?\C:\pasta` (caminho canônico do Windows) vira `C:\pasta`.
    let texto = match texto.strip_prefix(r"\\?\") {
        Some(resto) if plataforma == Plataforma::Windows && !resto.starts_with("UNC") => resto,
        _ => texto,
    };
    let absoluta = match plataforma {
        Plataforma::Windows => {
            let b = texto.as_bytes();
            b.len() >= 3
                && b[0].is_ascii_alphabetic()
                && b[1] == b':'
                && matches!(b[2], b'\\' | b'/')
        }
        _ => texto.starts_with('/'),
    };
    if !absoluta {
        return Err(invalida("o caminho precisa ser absoluto e local"));
    }
    if texto.starts_with("//") || texto.starts_with(r"\\") {
        return Err(invalida("caminhos de rede não são aceitos"));
    }
    if texto.split(['/', '\\']).any(|parte| parte == "..") {
        return Err(invalida("o caminho não pode ter `..`"));
    }
    if texto.chars().any(|c| c.is_control() || invisivel(c)) {
        return Err(invalida(
            "o caminho tem caracteres invisíveis ou de controle",
        ));
    }
    Ok(texto)
}

/// Caracteres de largura zero e de controle bidirecional.
fn invisivel(c: char) -> bool {
    matches!(c,
        '\u{00AD}'
        | '\u{061C}'
        | '\u{180E}'
        | '\u{200B}'..='\u{200F}'
        | '\u{202A}'..='\u{202E}'
        | '\u{2060}'..='\u{2064}'
        | '\u{2066}'..='\u{2069}'
        | '\u{FEFF}')
}

/// Codificação de URL em UTF-8: letras, dígitos e `-._~` ficam como estão, mais os
/// bytes de `manter`; o resto vira `%XX`. Com `manter` vazio, codifica pelo menos o
/// que o `encodeURIComponent` codifica (e também `!'()*`).
pub fn codificar(texto: &str, manter: &[u8]) -> String {
    let mut saida = String::with_capacity(texto.len());
    for b in texto.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) || manter.contains(&b) {
            saida.push(char::from(b));
        } else {
            let _ = write!(saida, "%{b:02X}");
        }
    }
    saida
}
