//! Abrir no Claude Desktop uma sessão do Claude Code: `claude --desktop --resume <id>`
//! (spec, seção 4.7).
//!
//! Pela documentação (<https://code.claude.com/docs/en/cli-reference> e
//! <https://code.claude.com/docs/en/desktop>, seção "Coming from the CLI?",
//! consultadas em 2026-11-11), `claude --desktop` abre o Desktop na pasta atual e sai
//! sem iniciar sessão no terminal; com `--continue` abre a conversa mais recente da
//! pasta e com `--resume <session-id>` a sessão indicada (só o identificador, não o
//! nome). Não aceita prompt nem outras opções, exige o Claude Code 2.1.285 ou mais
//! novo, assinatura do Claude e macOS ou Windows x64, e não move uma sessão aberta
//! noutro terminal ou rodando em segundo plano. O comando imprime
//! `Opening session <session-id> in Claude Desktop` e termina.
//!
//! A prova de conceito (spec, seção 13) mostrou que o comando só funciona com um
//! terminal (PTY), então o app o roda num [`Terminal`] sem mostrá-lo, espera o fim e
//! devolve a saída como texto para a tela.

use std::ffi::OsStr;
use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde::Serialize;

use super::PedidoClaude;
use super::claude::{ErroAbertura, Lancamento, Retomada, id_valido, montar};
use super::pty::{Comando, ErroTerminal, Evento, Tamanho, Terminal};
use crate::clientes::{self, Ambiente, Plataforma, orientacao};
use crate::preparo::{Alvo, Preparador};

/// Versão mínima do Claude Code com `--desktop`.
pub const VERSAO_MINIMA: (u64, u64, u64) = (2, 1, 285);

/// Quanto esperar o `claude --desktop` terminar.
pub const LIMITE_DESKTOP: Duration = Duration::from_secs(60);

/// Quantos caracteres da saída vão para a tela.
const MAX_MENSAGEM: usize = 2_000;

/// Como o `claude --desktop` terminou.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultadoDesktop {
    /// Saiu com código 0: a sessão foi entregue ao Desktop.
    pub aberto: bool,
    /// Código de saída; `None` se o processo não terminou no prazo e foi encerrado.
    pub codigo: Option<u32>,
    /// O que o `claude` escreveu, sem as sequências de controle do terminal.
    pub mensagem: String,
}

/// Confere o ambiente e monta o `claude --desktop` para o sistema. `Retomada::Nova`
/// abre o Desktop na pasta, sem sessão.
///
/// Recusa no Linux, sem o ambiente preparado, sem o Claude Code instalado ou com uma
/// versão anterior a [`VERSAO_MINIMA`] (quando a versão é conhecida).
pub fn lancamento_desktop(
    sistema: &cba_index::Sistema,
    retomada: &Retomada,
    preparador: &Preparador,
    ambiente: &dyn Ambiente,
) -> Result<Lancamento, ErroAbertura> {
    let plataforma = ambiente.plataforma();
    if plataforma == Plataforma::Linux {
        return Err(ErroAbertura::DesktopIndisponivel {
            detalhe: "o Claude Desktop com a aba de código só existe para macOS e Windows".into(),
            orientacao: Vec::new(),
        });
    }
    // O mesmo pedido do terminal embutido, sem prompt, confere o ambiente e o `claude`.
    let base = super::claude::lancamento(sistema, &PedidoClaude::default(), preparador, ambiente)?;
    let claude = base.comando.programa.clone();
    let versao = ambiente
        .executar(&claude, &["--version"], clientes::LIMITE_VERSAO)
        .ok()
        .filter(|s| s.codigo == Some(0))
        .and_then(|s| clientes::versao_da_saida(&s.stdout));
    if let Some(v) = versao.as_deref()
        && numeros(v).is_some_and(|n| n < VERSAO_MINIMA)
    {
        return Err(ErroAbertura::DesktopIndisponivel {
            detalhe: format!(
                "o Claude Code {v} não abre sessões no Desktop; é preciso a versão {}.{}.{} ou mais nova",
                VERSAO_MINIMA.0, VERSAO_MINIMA.1, VERSAO_MINIMA.2
            ),
            orientacao: orientacao::claude_code_verificar(plataforma),
        });
    }
    let alvo = Alvo::do_sistema(sistema).map_err(|e| ErroAbertura::Falha {
        mensagem: e.to_string(),
    })?;
    montar_desktop(
        &claude,
        &alvo.diretorio,
        retomada,
        plataforma,
        ambiente.var("PATH").as_deref(),
    )
}

/// Monta o `claude --desktop` em `diretorio` (o mesmo `PATH` e o mesmo ambiente do
/// terminal embutido).
pub fn montar_desktop(
    claude: &Path,
    diretorio: &Path,
    retomada: &Retomada,
    plataforma: Plataforma,
    path: Option<&OsStr>,
) -> Result<Lancamento, ErroAbertura> {
    let mut l = montar(
        claude,
        diretorio,
        &PedidoClaude::default(),
        plataforma,
        path,
        String::new,
    )?;
    let (args, id): (Vec<&str>, Option<String>) = match retomada {
        Retomada::Nova => (vec!["--desktop"], None),
        Retomada::Continuar => (vec!["--desktop", "--continue"], None),
        Retomada::Sessao { id } => {
            if !id_valido(id) {
                return Err(ErroAbertura::PedidoInvalido {
                    mensagem: "identificador de sessão inválido (só letras, números, - e _)".into(),
                });
            }
            (vec!["--desktop", "--resume", id.as_str()], Some(id.clone()))
        }
    };
    l.comando.args = args.into_iter().map(Into::into).collect();
    l.id_sessao = id;
    Ok(l)
}

/// Roda `comando` num PTY sem tela até ele terminar (ou até `limite`, quando é
/// encerrado) e devolve a saída. Responde à consulta de posição do cursor
/// (`ESC[6n`), que o ConPTY do Windows faz ao abrir.
pub fn executar_no_pty(
    comando: Comando,
    limite: Duration,
) -> Result<ResultadoDesktop, ErroTerminal> {
    let (tx, rx) = mpsc::channel::<Evento>();
    let tx = std::sync::Mutex::new(tx);
    let terminal = Terminal::abrir(comando, Tamanho::default(), move |e| {
        let _ = tx.lock().unwrap().send(e);
    })?;
    let prazo = Instant::now() + limite;
    let mut texto = String::new();
    let mut respondidas = 0;
    let mut codigo = None;
    loop {
        let restante = prazo.saturating_duration_since(Instant::now());
        if restante.is_zero() {
            let _ = terminal.encerrar();
            break;
        }
        match rx.recv_timeout(restante.min(Duration::from_millis(200))) {
            Ok(Evento::Saida { texto: t }) => {
                texto.push_str(&t);
                let pedidas = texto.matches("\x1b[6n").count();
                while pedidas > respondidas {
                    let _ = terminal.escrever(b"\x1b[1;1R");
                    respondidas += 1;
                }
            }
            Ok(Evento::Encerrado { fim }) => {
                codigo = Some(if fim.sinal.is_some() { 1 } else { fim.codigo });
                break;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    let mut mensagem = sem_controle(&texto);
    if codigo.is_none() {
        if !mensagem.is_empty() {
            mensagem.push('\n');
        }
        mensagem.push_str("O Claude Code não terminou no prazo e foi encerrado.");
    }
    Ok(ResultadoDesktop {
        aberto: codigo == Some(0),
        codigo,
        mensagem,
    })
}

/// Tira as sequências de controle do terminal (CSI, OSC e escapes de dois
/// caracteres) e os demais caracteres de controle, junta as linhas não vazias e
/// guarda o fim, até [`MAX_MENSAGEM`] caracteres.
pub fn sem_controle(texto: &str) -> String {
    let mut limpo = String::with_capacity(texto.len());
    let mut chars = texto.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\x1b' => match chars.next() {
                // CSI: parâmetros e intermediários até o byte final (0x40..=0x7E).
                Some('[') => {
                    for c in chars.by_ref() {
                        if ('\x40'..='\x7e').contains(&c) {
                            break;
                        }
                    }
                }
                // OSC: até BEL ou ESC \.
                Some(']') => {
                    while let Some(c) = chars.next() {
                        if c == '\x07' {
                            break;
                        }
                        if c == '\x1b' {
                            chars.next_if_eq(&'\\');
                            break;
                        }
                    }
                }
                _ => {}
            },
            '\n' => limpo.push('\n'),
            c if c.is_control() => {}
            c => limpo.push(c),
        }
    }
    let linhas: Vec<&str> = limpo
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    let junto = linhas.join("\n");
    let total = junto.chars().count();
    if total <= MAX_MENSAGEM {
        junto
    } else {
        junto.chars().skip(total - MAX_MENSAGEM).collect()
    }
}

/// `2.1.285` → `(2, 1, 285)`; o que vem depois de `-` ou `+` é ignorado.
fn numeros(versao: &str) -> Option<(u64, u64, u64)> {
    let nucleo = versao.split(['-', '+']).next()?;
    let mut partes = nucleo.split('.').map(|p| p.parse::<u64>().ok());
    Some((
        partes.next()??,
        partes.next()??,
        partes.next().flatten().unwrap_or(0),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argumentos_do_desktop() {
        let args = |r: &Retomada| -> Vec<String> {
            montar_desktop(
                Path::new("/home/a/.local/bin/claude"),
                Path::new("/sistemas/fat"),
                r,
                Plataforma::MacOs,
                None,
            )
            .unwrap()
            .comando
            .args
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
        };
        assert_eq!(args(&Retomada::Nova), ["--desktop"]);
        assert_eq!(args(&Retomada::Continuar), ["--desktop", "--continue"]);
        assert_eq!(
            args(&Retomada::Sessao {
                id: "550e8400-e29b-41d4-a716-446655440000".into()
            }),
            [
                "--desktop",
                "--resume",
                "550e8400-e29b-41d4-a716-446655440000"
            ]
        );
        let l = montar_desktop(
            Path::new("/home/a/.local/bin/claude"),
            Path::new("/sistemas/fat"),
            &Retomada::Sessao { id: "abc".into() },
            Plataforma::MacOs,
            Some(OsStr::new("/usr/bin")),
        )
        .unwrap();
        assert_eq!(l.id_sessao.as_deref(), Some("abc"));
        assert_eq!(l.comando.diretorio, Path::new("/sistemas/fat"));
        assert!(
            l.comando
                .env
                .iter()
                .any(|(k, v)| k == "PATH" && v == "/home/a/.local/bin:/usr/bin")
        );
        assert!(matches!(
            montar_desktop(
                Path::new("/c"),
                Path::new("/s"),
                &Retomada::Sessao { id: "a b".into() },
                Plataforma::MacOs,
                None
            ),
            Err(ErroAbertura::PedidoInvalido { .. })
        ));
    }

    #[test]
    fn versoes() {
        assert_eq!(numeros("2.1.285"), Some((2, 1, 285)));
        assert_eq!(numeros("2.1.300-beta.1"), Some((2, 1, 300)));
        assert_eq!(numeros("3.0"), Some((3, 0, 0)));
        assert_eq!(numeros("x"), None);
        assert!(numeros("2.1.284").unwrap() < VERSAO_MINIMA);
        assert!(numeros("2.2.0").unwrap() >= VERSAO_MINIMA);
    }

    #[test]
    fn saida_sem_sequencias_de_controle() {
        let bruto = "\x1b[?25l\x1b[1;1H\x1b]0;claude\x07Opening session abc in \x1b[1mClaude Desktop\x1b[0m\r\n\r\n\x1b[6n  fim \x1b]8;;x\x1b\\\r\n";
        assert_eq!(
            sem_controle(bruto),
            "Opening session abc in Claude Desktop\nfim"
        );
        let longo = "a".repeat(MAX_MENSAGEM + 10) + "fim";
        let m = sem_controle(&longo);
        assert_eq!(m.chars().count(), MAX_MENSAGEM);
        assert!(m.ends_with("fim"));
    }
}
