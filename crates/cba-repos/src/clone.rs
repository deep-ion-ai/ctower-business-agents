//! Clone inicial dos repositórios de código e "Testar acesso" (#179), pelo `git`
//! do analista.
//!
//! O token nunca vai para a URL, para os argumentos nem para o `.git/config`: um
//! credential helper passado com `-c` (vale só para aquela execução) lê o token
//! de uma variável de ambiente do processo filho. Sem token, o git usa o que o
//! analista já tem (chave SSH, credencial do próprio git). Nenhuma execução pede
//! nada no terminal: `GIT_TERMINAL_PROMPT=0` e SSH em `BatchMode`.
//!
//! Só o clone inicial; a atualização do clone e a escrita (branch própria, push e
//! PR/MR) ficam em [`crate::escrita`] (#275).

use crate::process::{self, RunError, cite};
use crate::{ForgeKind, RepositorioCodigo, Secret};
use serde::Serialize;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Duration;

/// Prazo do "Testar acesso".
pub const PRAZO_TESTE: Duration = Duration::from_secs(60);

/// Credential helper da execução: responde só ao `get`, com usuário e token do
/// ambiente. O valor fica na linha de comando; o token, não.
const HELPER: &str = "credential.helper=!f() { test \"$1\" = get || exit 0; \
                      echo \"username=${CBA_GIT_USUARIO}\"; echo \"password=${CBA_GIT_TOKEN}\"; }; f";

/// Usuário HTTP que acompanha um PAT em cada forja.
fn usuario_http(forja: ForgeKind) -> &'static str {
    match forja {
        ForgeKind::Github => "x-access-token",
        ForgeKind::Gitlab => "oauth2",
    }
}

/// Mensagem do erro de um clone interrompido pelo analista ([`clonar`]).
pub const CLONE_CANCELADO: &str = "clone cancelado";

/// O erro é o de um clone cancelado?
pub fn foi_cancelado(erro: &str) -> bool {
    erro == CLONE_CANCELADO
}

/// Etapa do clone, para a barra de progresso.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Progresso {
    /// Etapa como o git escreve (`Receiving objects`), traduzida quando conhecida.
    pub etapa: String,
    /// 0 a 100, quando o git informa.
    pub percentual: Option<u8>,
}

/// O `git` com o token (se houver) só no ambiente e sem pedir nada no terminal.
pub(crate) fn comando_git(git: &Path, forja: ForgeKind, token: Option<&Secret>) -> Command {
    let mut c = Command::new(git);
    c.env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "never")
        .env_remove("GIT_ASKPASS")
        .env_remove("SSH_ASKPASS");
    if std::env::var_os("GIT_SSH_COMMAND").is_none() {
        c.env("GIT_SSH_COMMAND", "ssh -o BatchMode=yes");
    }
    if let Some(t) = token {
        // A primeira opção zera os helpers do analista; a segunda põe o nosso.
        c.args(["-c", "credential.helper=", "-c", HELPER])
            .env("CBA_GIT_USUARIO", usuario_http(forja))
            .env("CBA_GIT_TOKEN", t.expose());
    }
    c
}

/// Base64 (RFC 4648, com `=`) para o cabeçalho `Authorization: Basic`.
fn base64(dados: &[u8]) -> String {
    const ALFABETO: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(dados.len().div_ceil(3) * 4);
    for bloco in dados.chunks(3) {
        let b = [
            bloco[0],
            *bloco.get(1).unwrap_or(&0),
            *bloco.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= bloco.len() {
                s.push(ALFABETO[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                s.push('=');
            }
        }
    }
    s
}

/// Faz o git mandar o token já no primeiro pedido HTTP (#215), por
/// `http.extraHeader` em `GIT_CONFIG_KEY_n`/`GIT_CONFIG_VALUE_n`: só no ambiente
/// do processo filho, nunca nos argumentos. Sem isso, o git só usa a credencial
/// quando o servidor responde 401, e num repositório público um token falso
/// passaria no teste. Acrescenta às entradas que o ambiente já tiver.
fn enviar_token_de_saida(c: &mut Command, forja: ForgeKind, token: &Secret) {
    let n = std::env::var("GIT_CONFIG_COUNT")
        .ok()
        .and_then(|v| v.trim().parse::<u32>().ok())
        .unwrap_or(0);
    let credencial = format!("{}:{}", usuario_http(forja), token.expose());
    c.env("GIT_CONFIG_COUNT", (n + 1).to_string())
        .env(format!("GIT_CONFIG_KEY_{n}"), "http.extraHeader")
        .env(
            format!("GIT_CONFIG_VALUE_{n}"),
            format!("Authorization: Basic {}", base64(credencial.as_bytes())),
        );
}

/// Confere a URL de clone: preenchida, sem senha ou token embutido e sem cara de
/// opção do git.
pub fn validar_url(url: &str) -> Result<(), String> {
    let url = url.trim();
    if url.is_empty() {
        return Err("informe a URL de clone (HTTPS ou SSH)".into());
    }
    if url.starts_with('-') {
        return Err(format!("URL de clone inválida: {url}"));
    }
    if let Some((_, resto)) = url.split_once("://") {
        let autoridade = resto.split('/').next().unwrap_or("");
        if let Some((usuario, _)) = autoridade.rsplit_once('@')
            && usuario.contains(':')
        {
            return Err(
                "a URL de clone tem uma senha ou token embutido: tire-o da URL e informe o \
                 token no campo próprio"
                    .into(),
            );
        }
    }
    Ok(())
}

/// Mensagem clara para a falha do git, sem o token.
pub(crate) fn explicar(stderr: &str, token: Option<&Secret>) -> String {
    let t = stderr.to_ascii_lowercase();
    let motivo = if t.contains("authentication failed")
        || t.contains("could not read username")
        || t.contains("could not read password")
        || t.contains("terminal prompts disabled")
        || t.contains("403")
        || t.contains("401")
    {
        Some("acesso negado: confira o token (precisa de permissão de leitura do repositório)")
    } else if t.contains("permission denied (publickey") || t.contains("host key verification") {
        Some("o SSH recusou a conexão: confira a sua chave SSH ou use a URL HTTPS com um token")
    } else if t.contains("repository not found")
        || t.contains("not found")
        || t.contains("does not appear to be a git repository")
        || t.contains("404")
    {
        Some("repositório não encontrado (ou o token não tem acesso a ele)")
    } else if t.contains("could not resolve host") || t.contains("could not resolve hostname") {
        Some("host não encontrado: confira a URL e a rede")
    } else if t.contains("remote branch") && t.contains("not found") {
        Some("branch não encontrada no repositório")
    } else {
        None
    };
    let detalhe = cite(stderr, token);
    match motivo {
        Some(m) if detalhe.is_empty() => m.to_string(),
        Some(m) => format!("{m} ({detalhe})"),
        None if detalhe.is_empty() => "o git falhou sem mensagem".into(),
        None => detalhe,
    }
}

/// Valida o acesso de leitura antes de clonar: `git ls-remote --heads`. Com
/// token, ele vai já no primeiro pedido: um token recusado faz o teste falhar
/// mesmo num repositório público. O teste pela API da forja fica com
/// [`crate::acessos::testar_acesso_repositorio`].
pub fn testar_acesso(
    git: &Path,
    repo: &RepositorioCodigo,
    token: Option<&Secret>,
) -> Result<(), String> {
    let url = repo.url_efetiva();
    validar_url(&url)?;
    let mut c = comando_git(git, repo.forja, token);
    if let Some(t) = token {
        enviar_token_de_saida(&mut c, repo.forja, t);
    }
    c.args(["ls-remote", "--heads", "--", &url]);
    let o = process::run(c, PRAZO_TESTE).map_err(|e| match e {
        RunError::NotFound => "git não encontrado: instale o git e deixe-o no PATH".to_string(),
        RunError::Timeout(_) => "o servidor não respondeu a tempo: confira a URL e a rede".into(),
        e => cite(&e.to_string(), token),
    })?;
    if o.status.success() {
        Ok(())
    } else {
        Err(explicar(&String::from_utf8_lossy(&o.stderr), token))
    }
}

/// Nome da subpasta do clone: o último trecho do caminho (`grupo/app` → `app`).
pub fn nome_da_pasta(repo: &RepositorioCodigo) -> String {
    let nome = repo
        .caminho
        .trim()
        .trim_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or("")
        .trim_end_matches(".git");
    let limpo: String = nome
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '-'
            }
        })
        .collect();
    let limpo = limpo.trim_matches('.').to_string();
    if limpo.is_empty() {
        "repositorio".into()
    } else {
        limpo
    }
}

/// Lê uma linha de progresso do git (`Receiving objects:  45% (9/20)`).
fn progresso(linha: &str) -> Option<Progresso> {
    let linha = linha.trim().trim_start_matches("remote:").trim();
    let (etapa, resto) = linha.split_once(':')?;
    let percentual = resto
        .trim()
        .split_once('%')
        .and_then(|(n, _)| n.trim().parse::<u8>().ok())
        .filter(|n| *n <= 100);
    let etapa = match etapa.trim() {
        "Cloning into" => return None,
        "Enumerating objects" => "Listando objetos",
        "Counting objects" => "Contando objetos",
        "Compressing objects" => "Comprimindo objetos",
        "Receiving objects" => "Recebendo objetos",
        "Resolving deltas" => "Resolvendo diferenças",
        "Updating files" => "Criando os arquivos",
        "Checking out files" => "Criando os arquivos",
        _ if percentual.is_none() => return None,
        e => e,
    };
    Some(Progresso {
        etapa: etapa.to_string(),
        percentual,
    })
}

/// Mata o git do clone e os processos que ele abriu (`git-remote-https`,
/// `index-pack`), que ficam no mesmo grupo.
fn matar_clone(filho: &mut std::process::Child) {
    #[cfg(unix)]
    // SAFETY: só sinaliza o grupo criado no `spawn` (`process_group(0)`).
    unsafe {
        libc::kill(-(filho.id() as libc::pid_t), libc::SIGKILL);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        let _ = Command::new("taskkill")
            .args(["/PID", &filho.id().to_string(), "/T", "/F"])
            .creation_flags(0x0800_0000)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let _ = filho.kill();
}

/// Apaga o que o clone criou em `destino`. Uma pasta que já existia (vazia) fica,
/// só sem o que o git pôs nela; uma pasta nova é removida inteira.
fn limpar_clone(destino: &Path, existia: bool) {
    if !existia {
        let _ = std::fs::remove_dir_all(destino);
        return;
    }
    if let Ok(entradas) = std::fs::read_dir(destino) {
        for e in entradas.flatten() {
            let p = e.path();
            if e.file_type().is_ok_and(|t| t.is_dir()) {
                let _ = std::fs::remove_dir_all(&p);
            } else {
                let _ = std::fs::remove_file(&p);
            }
        }
    }
}

/// Clona `repo` em `base/<nome>` e devolve a pasta. Se a pasta já é um clone
/// deste repositório, devolve-a sem clonar de novo; se existe com outra coisa,
/// recusa. Em caso de falha ou de cancelamento (`cancelar` ligado, o erro é
/// [`CLONE_CANCELADO`]), mata o git e apaga só o que este clone criou.
pub fn clonar(
    git: &Path,
    repo: &RepositorioCodigo,
    base: &Path,
    token: Option<&Secret>,
    cancelar: &AtomicBool,
    ao_progredir: &mut dyn FnMut(Progresso),
) -> Result<PathBuf, String> {
    let url = repo.url_efetiva();
    validar_url(&url)?;
    if !base.is_absolute() {
        return Err(format!(
            "informe o caminho completo da pasta base: {}",
            base.display()
        ));
    }
    if cancelar.load(Ordering::SeqCst) {
        return Err(CLONE_CANCELADO.into());
    }
    let destino = base.join(nome_da_pasta(repo));
    let existia = destino.exists();
    if existia {
        let mesmo = destino.join(".git").exists()
            && cba_remotos(&destino)
                .iter()
                .any(|u| repo.aponta_para(u) || u.trim() == url);
        if mesmo {
            return Ok(destino);
        }
        let vazia = std::fs::read_dir(&destino)
            .map(|mut d| d.next().is_none())
            .unwrap_or(false);
        if !vazia {
            return Err(format!(
                "a pasta {} já existe e não é um clone de {}: escolha outra pasta base",
                destino.display(),
                repo.rotulo()
            ));
        }
    }
    std::fs::create_dir_all(base)
        .map_err(|e| format!("não foi possível criar {}: {e}", base.display()))?;
    let mut c = comando_git(git, repo.forja, token);
    c.args(["clone", "--progress"]);
    if !repo.branch.trim().is_empty() {
        c.args(["--branch", repo.branch.trim()]);
    }
    c.arg("--")
        .arg(&url)
        .arg(&destino)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        // Grupo próprio: o cancelamento mata o git e os filhos dele.
        c.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        c.creation_flags(0x0800_0000);
    }
    let mut filho = c.spawn().map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => {
            "git não encontrado: instale o git e deixe-o no PATH".into()
        }
        _ => cite(&e.to_string(), token),
    })?;
    let mut stderr = filho.stderr.take().expect("stderr com pipe");
    // O stderr é lido numa thread, para o laço abaixo poder olhar o cancelamento
    // enquanto o git não escreve nada.
    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    let leitor = std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match stderr.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if tx.send(buf[..n].to_vec()).is_err() {
                        break;
                    }
                }
            }
        }
    });
    let mut linhas: Vec<String> = Vec::new();
    let mut atual = Vec::new();
    loop {
        if cancelar.load(Ordering::SeqCst) {
            matar_clone(&mut filho);
            let _ = filho.wait();
            let _ = leitor.join();
            limpar_clone(&destino, existia);
            return Err(CLONE_CANCELADO.into());
        }
        let pedaco = match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(p) => p,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        };
        for b in pedaco {
            if b == b'\r' || b == b'\n' {
                let linha = String::from_utf8_lossy(&atual).into_owned();
                atual.clear();
                if linha.trim().is_empty() {
                    continue;
                }
                if let Some(p) = progresso(&linha) {
                    ao_progredir(p);
                } else {
                    linhas.push(linha);
                }
            } else {
                atual.push(b);
            }
        }
    }
    let _ = leitor.join();
    if !atual.is_empty() {
        linhas.push(String::from_utf8_lossy(&atual).into_owned());
    }
    let status = filho.wait().map_err(|e| cite(&e.to_string(), token))?;
    if cancelar.load(Ordering::SeqCst) {
        limpar_clone(&destino, existia);
        return Err(CLONE_CANCELADO.into());
    }
    if !status.success() {
        limpar_clone(&destino, existia);
        return Err(explicar(&linhas.join("\n"), token));
    }
    Ok(destino)
}

/// URLs dos remotos de um clone, lidas do `.git/config`.
fn cba_remotos(raiz: &Path) -> Vec<String> {
    let Ok(config) = std::fs::read_to_string(raiz.join(".git").join("config")) else {
        return Vec::new();
    };
    config
        .lines()
        .filter_map(|l| {
            let (k, v) = l.trim().split_once('=')?;
            k.trim()
                .eq_ignore_ascii_case("url")
                .then(|| v.trim().to_string())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_sem_token_embutido() {
        assert!(validar_url("https://gitlab.example.com/g/app.git").is_ok());
        assert!(validar_url("https://usuario@gitlab.example.com/g/app.git").is_ok());
        assert!(validar_url("git@github.com:o/r.git").is_ok());
        assert!(
            validar_url("https://oauth2:glpat-x@gitlab.example.com/g/app.git")
                .unwrap_err()
                .contains("token embutido")
        );
        assert!(validar_url("--upload-pack=x").is_err());
        assert!(validar_url(" ").is_err());
    }

    #[test]
    fn nome_da_pasta_do_clone() {
        let r = |c: &str| RepositorioCodigo {
            caminho: c.into(),
            ..Default::default()
        };
        assert_eq!(nome_da_pasta(&r("grupo/sub/app")), "app");
        assert_eq!(nome_da_pasta(&r("o/r.git")), "r");
        assert_eq!(nome_da_pasta(&r("o/meu repo")), "meu-repo");
        assert_eq!(nome_da_pasta(&r("o/..")), "repositorio");
    }

    #[test]
    fn linhas_de_progresso() {
        assert_eq!(
            progresso("Receiving objects:  45% (9/20), 1.2 MiB | 3 MiB/s"),
            Some(Progresso {
                etapa: "Recebendo objetos".into(),
                percentual: Some(45)
            })
        );
        assert_eq!(
            progresso("remote: Counting objects: 100% (3/3), done."),
            Some(Progresso {
                etapa: "Contando objetos".into(),
                percentual: Some(100)
            })
        );
        assert_eq!(progresso("Cloning into '/tmp/x'..."), None);
        assert_eq!(progresso("fatal: repository not found"), None);
    }

    #[test]
    fn base64_do_cabecalho() {
        let casos = [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
            ("oauth2:glpat-x", "b2F1dGgyOmdscGF0LXg="),
        ];
        for (de, para) in casos {
            assert_eq!(base64(de.as_bytes()), para, "{de}");
        }
    }

    #[test]
    fn mensagens_claras_sem_token() {
        let t = Secret::new("glpat-segredo");
        let m = explicar(
            "remote: HTTP Basic: Access denied\nfatal: Authentication failed for glpat-segredo",
            Some(&t),
        );
        assert!(m.starts_with("acesso negado"), "{m}");
        assert!(!m.contains("glpat-segredo"), "{m}");
        assert!(explicar("fatal: repository 'x' not found", None).contains("não encontrado"));
        assert!(explicar("ssh: Could not resolve hostname x", None).contains("host"));
    }

    #[cfg(unix)]
    mod com_git {
        use super::*;
        use crate::demanda::falsas::script;

        fn git(dir: &Path, args: &[&str]) {
            let s = Command::new("git")
                .args([
                    "-c",
                    "user.name=t",
                    "-c",
                    "user.email=t@t",
                    "-c",
                    "commit.gpgsign=false",
                    "-c",
                    "init.defaultBranch=main",
                ])
                .args(args)
                .current_dir(dir)
                .status()
                .unwrap();
            assert!(s.success(), "git {args:?}");
        }

        /// Repositório de origem local com um commit, servido por `file://`.
        fn origem(dir: &Path) -> String {
            let o = dir.join("origem");
            std::fs::create_dir_all(&o).unwrap();
            git(&o, &["init", "-q"]);
            std::fs::write(o.join("leia-me.md"), "x\n").unwrap();
            git(&o, &["add", "-A"]);
            git(&o, &["commit", "-q", "-m", "base"]);
            git(&o, &["branch", "develop"]);
            format!("file://{}", o.display())
        }

        #[test]
        fn clona_testa_acesso_e_nao_grava_o_token() {
            let d = tempfile::tempdir().unwrap();
            let base = dunce::canonicalize(d.path()).unwrap();
            let url = origem(&base);
            let repo = RepositorioCodigo {
                forja: ForgeKind::Gitlab,
                host: "gitlab.example.com".into(),
                caminho: "grupo/app".into(),
                url_clone: url.clone(),
                branch: "develop".into(),
                ..Default::default()
            };
            let token = Secret::new("glpat-nao-pode-aparecer");
            testar_acesso(Path::new("git"), &repo, Some(&token)).unwrap();
            let mut etapas = Vec::new();
            let pasta = clonar(
                Path::new("git"),
                &repo,
                &base.join("clones"),
                Some(&token),
                &AtomicBool::new(false),
                &mut |p| etapas.push(p),
            )
            .unwrap();
            assert_eq!(pasta, base.join("clones/app"));
            assert!(pasta.join("leia-me.md").exists());
            let config = std::fs::read_to_string(pasta.join(".git/config")).unwrap();
            assert!(!config.contains("glpat"), "{config}");
            assert!(!config.contains("CBA_GIT_TOKEN"), "{config}");
            assert!(config.contains("[branch \"develop\"]"), "{config}");
            // Clonar de novo o mesmo repositório não faz nada.
            assert_eq!(
                clonar(
                    Path::new("git"),
                    &repo,
                    &base.join("clones"),
                    None,
                    &AtomicBool::new(false),
                    &mut |_| {}
                )
                .unwrap(),
                pasta
            );
            // Pasta ocupada por outra coisa: recusa.
            let outro = RepositorioCodigo {
                caminho: "outro/app".into(),
                url_clone: "https://gitlab.example.com/outro/app.git".into(),
                ..repo.clone()
            };
            assert!(
                clonar(
                    Path::new("git"),
                    &outro,
                    &base.join("clones"),
                    None,
                    &AtomicBool::new(false),
                    &mut |_| {}
                )
                .unwrap_err()
                .contains("já existe")
            );
            // Branch inexistente: erro claro e nada fica para trás.
            let sem_branch = RepositorioCodigo {
                branch: "nao-existe".into(),
                ..repo.clone()
            };
            let e = clonar(
                Path::new("git"),
                &sem_branch,
                &base.join("outros"),
                None,
                &AtomicBool::new(false),
                &mut |_| {},
            )
            .unwrap_err();
            assert!(e.contains("nao-existe"), "{e}");
            assert!(!base.join("outros/app").exists());
            assert!(
                testar_acesso(
                    Path::new("git"),
                    &RepositorioCodigo {
                        url_clone: format!("file://{}/nada", base.display()),
                        ..repo
                    },
                    None
                )
                .is_err()
            );
        }

        /// git falso que começa o clone (cria arquivos na pasta de destino), avisa
        /// o progresso e fica esperando, como um clone lento, com um processo filho.
        fn git_lento(dir: &Path) -> PathBuf {
            script(
                dir,
                "git",
                r#"for a; do destino="$a"; done
mkdir -p "$destino/.git"
echo parcial > "$destino/.git/HEAD"
echo "$$" > "$destino/../pid-do-git"
printf 'Receiving objects:  10%% (1/10)\r' >&2
sleep 60
exit 0"#,
            )
        }

        fn repo_de_teste() -> RepositorioCodigo {
            RepositorioCodigo {
                forja: ForgeKind::Gitlab,
                host: "gitlab.example.com".into(),
                caminho: "g/app".into(),
                url_clone: "https://gitlab.example.com/g/app.git".into(),
                ..Default::default()
            }
        }

        fn processo_vivo(pid: &str) -> bool {
            Command::new("kill")
                .args(["-0", pid.trim()])
                .stderr(Stdio::null())
                .status()
                .is_ok_and(|s| s.success())
        }

        #[test]
        fn cancelar_mata_o_git_e_apaga_a_pasta_parcial() {
            let d = tempfile::tempdir().unwrap();
            let base = dunce::canonicalize(d.path()).unwrap().join("clones");
            std::fs::create_dir_all(&base).unwrap();
            // Outra pasta da base, que o clone não criou: tem de continuar lá.
            std::fs::create_dir_all(base.join("outro")).unwrap();
            std::fs::write(base.join("outro/dado.txt"), "meu").unwrap();
            let git = git_lento(d.path());
            let repo = repo_de_teste();
            let cancelar = std::sync::Arc::new(AtomicBool::new(false));
            let (c2, b2) = (cancelar.clone(), base.clone());
            let (tx, rx) = mpsc::channel();
            let t = std::thread::spawn(move || {
                clonar(&git, &repo, &b2, None, &c2, &mut |p| {
                    let _ = tx.send(p);
                })
            });
            // Espera o clone começar de verdade e cancela.
            let p = rx.recv_timeout(Duration::from_secs(20)).unwrap();
            assert_eq!(p.percentual, Some(10));
            assert!(base.join("app/.git/HEAD").exists());
            let pid = std::fs::read_to_string(base.join("pid-do-git")).unwrap();
            let inicio = std::time::Instant::now();
            cancelar.store(true, Ordering::SeqCst);
            let r = t.join().unwrap();
            assert!(
                inicio.elapsed() < Duration::from_secs(20),
                "demorou a parar"
            );
            let erro = r.unwrap_err();
            assert!(foi_cancelado(&erro), "{erro}");
            assert!(!base.join("app").exists(), "sobrou a pasta parcial");
            assert!(!processo_vivo(&pid), "o git continua rodando");
            // O que não era do clone ficou.
            assert_eq!(
                std::fs::read_to_string(base.join("outro/dado.txt")).unwrap(),
                "meu"
            );
        }

        #[test]
        fn cancelar_numa_pasta_vazia_que_ja_existia_a_mantem_vazia() {
            let d = tempfile::tempdir().unwrap();
            let base = dunce::canonicalize(d.path()).unwrap().join("clones");
            std::fs::create_dir_all(base.join("app")).unwrap();
            let git = git_lento(d.path());
            let cancelar = std::sync::Arc::new(AtomicBool::new(false));
            let (c2, b2) = (cancelar.clone(), base.clone());
            let (tx, rx) = mpsc::channel();
            let t = std::thread::spawn(move || {
                clonar(&git, &repo_de_teste(), &b2, None, &c2, &mut |p| {
                    let _ = tx.send(p);
                })
            });
            rx.recv_timeout(Duration::from_secs(20)).unwrap();
            cancelar.store(true, Ordering::SeqCst);
            assert!(foi_cancelado(&t.join().unwrap().unwrap_err()));
            assert!(base.join("app").is_dir());
            assert_eq!(std::fs::read_dir(base.join("app")).unwrap().count(), 0);
        }

        #[test]
        fn cancelado_antes_de_comecar_nao_mexe_em_nada() {
            let d = tempfile::tempdir().unwrap();
            let base = dunce::canonicalize(d.path()).unwrap();
            // A pasta de destino é de outra coisa: cancelar não pode apagá-la.
            std::fs::create_dir_all(base.join("app")).unwrap();
            std::fs::write(base.join("app/meu.txt"), "meu").unwrap();
            let e = clonar(
                &git_lento(d.path()),
                &repo_de_teste(),
                &base,
                None,
                &AtomicBool::new(true),
                &mut |_| {},
            )
            .unwrap_err();
            assert!(foi_cancelado(&e));
            assert!(base.join("app/meu.txt").exists());
        }

        #[test]
        fn token_so_no_ambiente_do_git() {
            let d = tempfile::tempdir().unwrap();
            // git falso: confere que o token não está nos argumentos e que o
            // helper o entrega pelo ambiente.
            let git = script(
                d.path(),
                "git",
                r#"case "$*" in *glpat*) echo "token nos argumentos" >&2; exit 9;; esac
[ "$CBA_GIT_TOKEN" = "glpat-x" ] && [ "$CBA_GIT_USUARIO" = "oauth2" ] || { echo "sem token" >&2; exit 8; }
[ "$GIT_TERMINAL_PROMPT" = "0" ] || exit 7
# ls-remote: o token vai logo no primeiro pedido, também só pelo ambiente.
case "$*" in *ls-remote*)
  [ "$GIT_CONFIG_KEY_0" = "http.extraHeader" ] && [ "$GIT_CONFIG_VALUE_0" = "Authorization: Basic b2F1dGgyOmdscGF0LXg=" ] || { echo "token não vai de saída" >&2; exit 6; };;
esac
exit 0"#,
            );
            let repo = RepositorioCodigo {
                forja: ForgeKind::Gitlab,
                host: "gitlab.example.com".into(),
                caminho: "g/app".into(),
                url_clone: "https://gitlab.example.com/g/app.git".into(),
                ..Default::default()
            };
            testar_acesso(&git, &repo, Some(&Secret::new("glpat-x"))).unwrap();
            let e = testar_acesso(&git, &repo, None).unwrap_err();
            assert!(e.contains("sem token"), "{e}");
        }
    }
}
