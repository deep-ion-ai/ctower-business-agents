//! Detecção com um `claude` falso, executado de verdade pelo [`AmbienteReal`].
//!
//! O binário falso é um script (shell no macOS e no Linux, `.cmd` no Windows) que
//! responde a `--version` e a `auth status --json`. O CI não precisa do Claude.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use cba_session::clientes::{
    Ambiente, AmbienteReal, ErroExecucao, InstalacaoClaudeCode, LoginClaudeCode, OrigemCaminho,
    Plataforma, Saida, detectar_claude_code,
};

/// Sistema de verdade para arquivos e processos, com `PATH` e pasta do usuário
/// controlados pelo teste.
struct Isolado {
    path: OsString,
    home: PathBuf,
}

impl Ambiente for Isolado {
    fn plataforma(&self) -> Plataforma {
        AmbienteReal.plataforma()
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

/// Cria um `claude` falso em `pasta` que responde `status` a `auth status --json`.
fn claude_falso(pasta: &Path, status: &str, codigo_status: i32) -> PathBuf {
    #[cfg(windows)]
    {
        let caminho = pasta.join("claude.cmd");
        let script = format!(
            "@echo off\r\n\
             if \"%~1\"==\"--version\" goto versao\r\n\
             if \"%~1\"==\"auth\" goto auth\r\n\
             exit /b 3\r\n\
             :versao\r\n\
             echo 9.8.7 ^(Claude Code^)\r\n\
             exit /b 0\r\n\
             :auth\r\n\
             echo {status}\r\n\
             exit /b {codigo_status}\r\n"
        );
        std::fs::write(&caminho, script).unwrap();
        caminho
    }
    #[cfg(not(windows))]
    {
        use std::os::unix::fs::PermissionsExt;
        let caminho = pasta.join("claude");
        let script = format!(
            "#!/bin/sh\n\
             case \"$1\" in\n\
             --version) echo '9.8.7 (Claude Code)'; exit 0 ;;\n\
             auth) echo '{status}'; exit {codigo_status} ;;\n\
             esac\n\
             exit 3\n"
        );
        std::fs::write(&caminho, script).unwrap();
        std::fs::set_permissions(&caminho, std::fs::Permissions::from_mode(0o755)).unwrap();
        caminho
    }
}

fn isolado(bin: &Path, home: &Path) -> Isolado {
    Isolado {
        path: std::env::join_paths([bin]).unwrap(),
        home: home.to_path_buf(),
    }
}

#[test]
fn claude_falso_logado() {
    let tmp = tempfile::tempdir().unwrap();
    let caminho = claude_falso(
        tmp.path(),
        r#"{"loggedIn":true,"authMethod":"claude.ai","apiProvider":"firstParty"}"#,
        0,
    );
    let estado = detectar_claude_code(&isolado(tmp.path(), tmp.path()));
    assert_eq!(
        estado.instalacao,
        InstalacaoClaudeCode::Encontrado {
            caminho,
            origem: OrigemCaminho::Path,
            versao: Some("9.8.7".into()),
        }
    );
    assert_eq!(
        estado.login,
        LoginClaudeCode::Logado {
            metodo: Some("claude.ai".into()),
            provedor: Some("firstParty".into()),
        }
    );
}

#[test]
fn claude_falso_sem_login() {
    let tmp = tempfile::tempdir().unwrap();
    claude_falso(tmp.path(), r#"{"loggedIn":false,"authMethod":"none"}"#, 1);
    let estado = detectar_claude_code(&isolado(tmp.path(), tmp.path()));
    assert_eq!(estado.login, LoginClaudeCode::NaoLogado);
}

#[test]
fn sem_claude_nenhum_comando_e_executado() {
    let tmp = tempfile::tempdir().unwrap();
    let estado = detectar_claude_code(&isolado(tmp.path(), tmp.path()));
    assert_eq!(estado.instalacao, InstalacaoClaudeCode::NaoEncontrado);
    assert_eq!(estado.login, LoginClaudeCode::NaoVerificado);
}

#[test]
fn programa_inexistente_nao_inicia() {
    let tmp = tempfile::tempdir().unwrap();
    let r = AmbienteReal.executar(&tmp.path().join("nao-existe"), &[], Duration::from_secs(5));
    assert!(matches!(r, Err(ErroExecucao::NaoIniciou(_))), "{r:?}");
}

#[cfg(unix)]
#[test]
fn comando_que_trava_e_encerrado_no_limite() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = tempfile::tempdir().unwrap();
    let caminho = tmp.path().join("lento");
    std::fs::write(&caminho, "#!/bin/sh\nexec sleep 30\n").unwrap();
    std::fs::set_permissions(&caminho, std::fs::Permissions::from_mode(0o755)).unwrap();
    let inicio = std::time::Instant::now();
    let r = AmbienteReal.executar(&caminho, &[], Duration::from_millis(300));
    assert_eq!(r, Err(ErroExecucao::TempoEsgotado));
    assert!(inicio.elapsed() < Duration::from_secs(10));
}
