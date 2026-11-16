//! Clone inicial dos repositórios de código e "Testar acesso" (#179), pelo `git`
//! do analista.
//!
//! O token nunca vai para a URL, para os argumentos nem para o `.git/config`: um
//! credential helper passado com `-c` (vale só para aquela execução) lê o token
//! de uma variável de ambiente do processo filho. Sem token, o git usa o que o
//! analista já tem (chave SSH, credencial do próprio git). Nenhuma execução pede
//! nada no terminal: `GIT_TERMINAL_PROMPT=0` e SSH em `BatchMode`.
//!
//! Só o clone inicial: atualizar (fetch/pull) fica com o analista.

use crate::process::{self, RunError, cite};
use crate::{ForgeKind, RepositorioCodigo, Secret};
use serde::Serialize;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
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

/// Etapa do clone, para a barra de progresso.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Progresso {
    /// Etapa como o git escreve (`Receiving objects`), traduzida quando conhecida.
    pub etapa: String,
    /// 0 a 100, quando o git informa.
    pub percentual: Option<u8>,
}

/// O `git` com o token (se houver) só no ambiente e sem pedir nada no terminal.
fn comando_git(git: &Path, forja: ForgeKind, token: Option<&Secret>) -> Command {
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
fn explicar(stderr: &str, token: Option<&Secret>) -> String {
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

/// Valida o acesso de leitura antes de clonar: `git ls-remote --heads`.
pub fn testar_acesso(
    git: &Path,
    repo: &RepositorioCodigo,
    token: Option<&Secret>,
) -> Result<(), String> {
    let url = repo.url_efetiva();
    validar_url(&url)?;
    let mut c = comando_git(git, repo.forja, token);
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

/// Clona `repo` em `base/<nome>` e devolve a pasta. Se a pasta já é um clone
/// deste repositório, devolve-a sem clonar de novo; se existe com outra coisa,
/// recusa. Em caso de falha, apaga o que criou.
pub fn clonar(
    git: &Path,
    repo: &RepositorioCodigo,
    base: &Path,
    token: Option<&Secret>,
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
    let destino = base.join(nome_da_pasta(repo));
    if destino.exists() {
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
    let mut linhas: Vec<String> = Vec::new();
    let mut atual = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        let n = match stderr.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        for &b in &buf[..n] {
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
    if !atual.is_empty() {
        linhas.push(String::from_utf8_lossy(&atual).into_owned());
    }
    let status = filho.wait().map_err(|e| cite(&e.to_string(), token))?;
    if !status.success() {
        let _ = std::fs::remove_dir_all(&destino);
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
