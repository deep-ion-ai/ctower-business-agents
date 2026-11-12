//! Retomada no Claude Desktop (`claude --desktop --resume <id>`) e passagem entre os
//! clientes, com PTY de verdade e um `claude` falso.
//!
//! O Desktop real só existe no macOS e no Windows e exige login; o que estes testes
//! não cobrem está na pendência da #42 (testes de fumaça por versão do Claude).

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use cba_session::clientes::{Ambiente, AmbienteReal, ErroExecucao, Plataforma, Saida};
use cba_session::preparo::{Alvo, ConfigClaude, Preparador};
use cba_session::terminal::{
    ErroAbertura, PedidoClaude, Retomada, executar_no_pty, lancamento, lancamento_desktop,
};

const LIMITE: Duration = Duration::from_secs(20);

/// Grava um script executável: `sh` no macOS e no Linux, `.cmd` no Windows.
fn script(pasta: &Path, nome: &str, sh: &str, cmd: &str) -> PathBuf {
    #[cfg(windows)]
    {
        let _ = sh;
        let caminho = pasta.join(format!("{nome}.cmd"));
        std::fs::write(
            &caminho,
            format!("@echo off\r\n{}\r\n", cmd.replace('\n', "\r\n")),
        )
        .unwrap();
        caminho
    }
    #[cfg(not(windows))]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = cmd;
        let caminho = pasta.join(nome);
        std::fs::write(&caminho, format!("#!/bin/sh\n{sh}\n")).unwrap();
        std::fs::set_permissions(&caminho, std::fs::Permissions::from_mode(0o755)).unwrap();
        caminho
    }
}

/// Ambiente de verdade com `PATH`, pasta do usuário e plataforma controlados pelo
/// teste. No Linux, onde o Desktop não existe, faz de conta que é macOS.
struct Isolado {
    path: OsString,
    home: PathBuf,
    plataforma: Plataforma,
}

impl Ambiente for Isolado {
    fn plataforma(&self) -> Plataforma {
        self.plataforma
    }
    fn var(&self, nome: &str) -> Option<OsString> {
        match nome {
            "PATH" => Some(self.path.clone()),
            "PATHEXT" => Some(".COM;.EXE;.BAT;.CMD".into()),
            _ => None,
        }
    }
    fn home(&self) -> Option<PathBuf> {
        Some(self.home.clone())
    }
    fn e_arquivo(&self, caminho: &Path) -> bool {
        AmbienteReal.e_arquivo(caminho)
    }
    fn e_pasta(&self, caminho: &Path) -> bool {
        AmbienteReal.e_pasta(caminho)
    }
    fn nomes_na_pasta(&self, pasta: &Path) -> Vec<OsString> {
        AmbienteReal.nomes_na_pasta(pasta)
    }
    fn executar(
        &self,
        programa: &Path,
        args: &[&str],
        limite: Duration,
    ) -> Result<Saida, ErroExecucao> {
        AmbienteReal.executar(programa, args, limite)
    }
}

struct Cenario {
    _tmp: tempfile::TempDir,
    projeto: cba_index::Projeto,
    alvo: Alvo,
    preparador: Preparador,
    ambiente: Isolado,
    bin: PathBuf,
}

/// Projeto cadastrado, ambiente preparado e uma pasta para o `claude` falso.
fn cenario() -> Cenario {
    let tmp = tempfile::tempdir().unwrap();
    let raiz = tmp.path().join("faturamento");
    let home = tmp.path().join("home");
    let bin = tmp.path().join("bin");
    for p in [&raiz, &home, &bin] {
        std::fs::create_dir_all(p).unwrap();
    }
    let projeto = cba_index::Projeto::novo("Faturamento Exemplo", [&raiz]).unwrap();
    let exe = tmp
        .path()
        .join(if cfg!(windows) { "cba.exe" } else { "cba" });
    let preparador = Preparador::novo(
        ConfigClaude::da_pasta_do_usuario(&home),
        exe,
        Plataforma::atual(),
    )
    .unwrap();
    let alvo = Alvo::do_projeto(&projeto).unwrap();
    preparador.preparar(&alvo).unwrap();
    let plataforma = match Plataforma::atual() {
        Plataforma::Linux => Plataforma::MacOs,
        p => p,
    };
    Cenario {
        _tmp: tmp,
        projeto,
        alvo,
        preparador,
        ambiente: Isolado {
            path: std::env::join_paths([&bin]).unwrap(),
            home,
            plataforma,
        },
        bin,
    }
}

/// `claude` falso: responde `--version` com `versao`; com `--desktop`, recusa sem
/// terminal (como o de verdade fazia na prova de conceito) e, com terminal, imprime o
/// que o Claude Code imprime e sai; sem `--desktop`, mostra os argumentos e sai.
fn claude_falso(bin: &Path, versao: &str) -> PathBuf {
    script(
        bin,
        "claude",
        &format!(
            r#"if [ "$1" = "--version" ]; then echo "{versao} (Claude Code)"; exit 0; fi
echo "args:$*"
echo "pasta:$(pwd -P)"
if [ "$1" != "--desktop" ]; then exit 0; fi
if [ ! -t 0 ] || [ ! -t 1 ]; then echo "sem terminal"; exit 3; fi
printf '\033[1mOpening session %s in Claude Desktop\033[0m\n' "$3""#
        ),
        &format!(
            "if \"%1\"==\"--version\" goto versao\n\
             echo args:%*\n\
             echo pasta:%CD%\n\
             if not \"%1\"==\"--desktop\" exit /b 0\n\
             echo Opening session %3 in Claude Desktop\n\
             exit /b 0\n\
             :versao\n\
             echo {versao}\n\
             exit /b 0"
        ),
    )
}

#[test]
fn sessao_do_terminal_embutido_continua_no_desktop_e_volta() {
    let c = cenario();
    claude_falso(&c.bin, "2.1.300");
    let id = "550e8400-e29b-41d4-a716-446655440000";

    // Terminal embutido: a conversa nova recebe o identificador do app.
    let nova = lancamento(
        &c.projeto,
        &PedidoClaude::default(),
        &c.preparador,
        &c.ambiente,
    )
    .unwrap();
    assert!(nova.id_sessao.is_some());

    // Desktop: a mesma sessão, pelo identificador, num PTY.
    let l = lancamento_desktop(
        &c.projeto,
        &Retomada::Sessao { id: id.into() },
        &c.preparador,
        &c.ambiente,
    )
    .unwrap();
    assert_eq!(l.id_sessao.as_deref(), Some(id));
    assert_eq!(l.comando.diretorio, c.alvo.diretorio);
    let r = executar_no_pty(l.comando, LIMITE).unwrap();
    assert!(r.aberto, "{r:?}");
    assert_eq!(r.codigo, Some(0));
    assert!(
        r.mensagem
            .contains(&format!("args:--desktop --resume {id}")),
        "{r:?}"
    );
    assert!(
        r.mensagem
            .contains(&format!("Opening session {id} in Claude Desktop")),
        "{r:?}"
    );
    assert!(!r.mensagem.contains('\x1b'), "{r:?}");
    assert!(!r.mensagem.contains("sem terminal"), "{r:?}");

    // De volta ao terminal embutido: `--resume` com o mesmo identificador.
    let volta = lancamento(
        &c.projeto,
        &PedidoClaude {
            retomada: Retomada::Sessao { id: id.into() },
            ..Default::default()
        },
        &c.preparador,
        &c.ambiente,
    )
    .unwrap();
    assert_eq!(volta.id_sessao.as_deref(), Some(id));
    let r = executar_no_pty(volta.comando, LIMITE).unwrap();
    assert!(r.mensagem.contains(&format!("args:--resume {id}")), "{r:?}");
}

#[test]
fn desktop_sem_sessao_conhecida_usa_continue() {
    let c = cenario();
    claude_falso(&c.bin, "2.1.285");
    let l =
        lancamento_desktop(&c.projeto, &Retomada::Continuar, &c.preparador, &c.ambiente).unwrap();
    assert_eq!(l.id_sessao, None);
    let r = executar_no_pty(l.comando, LIMITE).unwrap();
    assert!(r.mensagem.contains("args:--desktop --continue"), "{r:?}");
}

#[test]
fn recusa_versao_antiga_e_linux() {
    let mut c = cenario();
    claude_falso(&c.bin, "2.1.284");
    match lancamento_desktop(&c.projeto, &Retomada::Continuar, &c.preparador, &c.ambiente) {
        Err(ErroAbertura::DesktopIndisponivel {
            detalhe,
            orientacao,
        }) => {
            assert!(detalhe.contains("2.1.284"), "{detalhe}");
            assert!(detalhe.contains("2.1.285"), "{detalhe}");
            assert!(!orientacao.is_empty());
        }
        outro => panic!("esperava a recusa pela versão: {outro:?}"),
    }
    c.ambiente.plataforma = Plataforma::Linux;
    assert!(matches!(
        lancamento_desktop(&c.projeto, &Retomada::Continuar, &c.preparador, &c.ambiente),
        Err(ErroAbertura::DesktopIndisponivel { .. })
    ));
}

#[test]
fn falha_e_prazo_do_desktop_chegam_a_tela() {
    let c = cenario();
    let recusa = script(
        &c.bin,
        "recusa",
        "echo 'Session is open in another terminal'; exit 1",
        "echo Session is open in another terminal\nexit /b 1",
    );
    let comando = |programa: PathBuf| cba_session::terminal::Comando {
        programa,
        args: Vec::new(),
        diretorio: c.alvo.diretorio.clone(),
        env: Vec::new(),
        env_remover: Vec::new(),
    };
    let r = executar_no_pty(comando(recusa), LIMITE).unwrap();
    assert!(!r.aberto);
    assert_eq!(r.codigo, Some(1));
    assert!(
        r.mensagem.contains("Session is open in another terminal"),
        "{r:?}"
    );

    let lento = script(
        &c.bin,
        "lento",
        "echo pronto; sleep 60",
        "echo pronto\nping -n 60 127.0.0.1 >nul",
    );
    let r = executar_no_pty(comando(lento), Duration::from_secs(2)).unwrap();
    assert!(!r.aberto);
    assert_eq!(r.codigo, None);
    assert!(r.mensagem.contains("não terminou no prazo"), "{r:?}");
}
