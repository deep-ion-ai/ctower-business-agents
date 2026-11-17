//! Prontidão das integrações de um projeto: cada passo com a orientação quando
//! falha. Não bloqueia o uso do app sem as integrações.
//!
//! Passos do ctower-agents. Jira: jira-cli instalado, `.config.yml` presente,
//! `server` igual à `url`, comando de credencial, `jira me` e `jira serverinfo`.
//! GitLab: `glab version` e `glab api version`. GitHub: `gh` e `gh auth token`.

use crate::acessos::{ErroVerificacao, verificar_token_host, verificar_token_repositorio};
use crate::chaveiro::{conta_propria, token_do_repositorio};
use crate::clone::testar_acesso;
use crate::codigo::HOST_GITHUB;
use crate::demanda::{Ferramentas, PRAZO_CLI};
use crate::process::{RunError, cite, run};
use crate::{
    ForgeKind, GithubConfig, GitlabConfig, JiraConfig, OrigemToken, RepositorioCodigo,
    Repositorios, Secret, TrackerConfig,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Situacao {
    Ok,
    Falhou,
    /// Não rodou porque um passo anterior falhou.
    Pulado,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Passo {
    pub id: String,
    pub rotulo: String,
    pub situacao: Situacao,
    /// O que foi visto (versão, servidor); nunca a senha.
    pub detalhe: Option<String>,
    /// O que o analista faz para consertar, quando falha.
    pub orientacao: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Integracao {
    /// `Código (GitHub)`, `Issues (Jira)`…
    pub titulo: String,
    pub passos: Vec<Passo>,
}

impl Integracao {
    pub fn pronta(&self) -> bool {
        self.passos.iter().all(|p| p.situacao == Situacao::Ok)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Prontidao {
    pub integracoes: Vec<Integracao>,
}

impl Prontidao {
    pub fn pronta(&self) -> bool {
        self.integracoes.iter().all(Integracao::pronta)
    }
}

struct Construtor {
    passos: Vec<Passo>,
    falhou: bool,
}

impl Construtor {
    fn novo() -> Self {
        Construtor {
            passos: vec![],
            falhou: false,
        }
    }

    /// Roda o passo, a menos que um anterior já tenha falhado.
    fn passo(
        &mut self,
        id: &'static str,
        rotulo: &str,
        orientacao: &str,
        f: impl FnOnce() -> Result<Option<String>, String>,
    ) {
        if self.falhou {
            self.passos.push(Passo {
                id: id.into(),
                rotulo: rotulo.into(),
                situacao: Situacao::Pulado,
                detalhe: None,
                orientacao: None,
            });
            return;
        }
        match f() {
            Ok(detalhe) => self.passos.push(Passo {
                id: id.into(),
                rotulo: rotulo.into(),
                situacao: Situacao::Ok,
                detalhe,
                orientacao: None,
            }),
            Err(motivo) => {
                self.falhou = true;
                self.passos.push(Passo {
                    id: id.into(),
                    rotulo: rotulo.into(),
                    situacao: Situacao::Falhou,
                    detalhe: Some(motivo),
                    orientacao: Some(orientacao.into()),
                });
            }
        }
    }
}

/// Roda a CLI e devolve a primeira linha do `stdout`.
fn saida(c: Command, token: Option<&Secret>) -> Result<String, String> {
    let o = run(c, PRAZO_CLI).map_err(|e| match e {
        RunError::NotFound => "não encontrado".to_string(),
        e => cite(&e.to_string(), token),
    })?;
    if !o.status.success() {
        let err = cite(&String::from_utf8_lossy(&o.stderr), token);
        return Err(if err.is_empty() {
            "falhou sem mensagem".into()
        } else {
            err
        });
    }
    Ok(String::from_utf8_lossy(&o.stdout)
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .to_string())
}

fn arquivo_de_config_jira(j: &JiraConfig) -> PathBuf {
    if !j.config_file.trim().is_empty() {
        return PathBuf::from(j.config_file.trim());
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    home.join(".config/.jira/.config.yml")
}

/// O `server:` do `.config.yml` do jira-cli, sem barra final.
fn servidor_do_config(yml: &str) -> Option<String> {
    yml.lines().find_map(|l| {
        let v = l.strip_prefix("server:")?.trim().trim_matches(['"', '\'']);
        Some(v.trim_end_matches('/').to_string())
    })
}

pub fn prontidao_jira(j: &JiraConfig, f: &Ferramentas) -> Integracao {
    let mut b = Construtor::novo();
    let cfg = arquivo_de_config_jira(j);
    let cred: std::cell::RefCell<Option<Secret>> = Default::default();
    b.passo(
        "cli",
        "jira-cli instalado",
        "Instale o jira-cli (https://github.com/ankitpokhrel/jira-cli) e deixe-o no PATH.",
        || {
            let mut c = Command::new(&f.jira);
            c.arg("version");
            saida(c, None).map(Some)
        },
    );
    b.passo(
        "config",
        "Configuração do jira-cli presente",
        "Rode `jira init` (ou aponte o arquivo de configuração no cadastro).",
        || {
            cfg.is_file()
                .then(|| Some(cfg.display().to_string()))
                .ok_or_else(|| format!("{} não existe", cfg.display()))
        },
    );
    b.passo(
        "servidor",
        "Servidor da configuração igual à URL do cadastro",
        "Corrija o `server` do .config.yml ou a URL do cadastro para serem iguais.",
        || {
            let yml = std::fs::read_to_string(&cfg).map_err(|e| e.to_string())?;
            let s = servidor_do_config(&yml).ok_or("sem `server` na configuração")?;
            if s == j.url.trim().trim_end_matches('/') {
                Ok(Some(s))
            } else {
                Err(format!("a configuração usa {s}, o cadastro usa {}", j.url))
            }
        },
    );
    b.passo(
        "credencial",
        "Token do Jira disponível",
        "Informe o token do Jira (fica no chaveiro do sistema) ou confira o comando de \
         credencial; o cache do gpg-agent pode ter expirado.",
        || {
            let s = crate::demanda::token_jira(j, f).map_err(|e| e.to_string())?;
            *cred.borrow_mut() = Some(s);
            Ok(None)
        },
    );
    let comando = |arg: &str| {
        let mut c = Command::new(&f.jira);
        c.arg(arg);
        if let Some(t) = cred.borrow().as_ref() {
            c.env("JIRA_API_TOKEN", t.expose());
        }
        if !j.config_file.trim().is_empty() {
            c.env("JIRA_CONFIG_FILE", j.config_file.trim());
        }
        c
    };
    let token = cred.borrow();
    b.passo(
        "me",
        "Autenticação (jira me)",
        "O token foi recusado: gere outro no Jira e atualize o comando de credencial.",
        || saida(comando("me"), token.as_ref()).map(Some),
    );
    b.passo(
        "serverinfo",
        "Versão do Jira (jira serverinfo)",
        "Confira se a URL do Jira está acessível a partir desta máquina.",
        || saida(comando("serverinfo"), token.as_ref()).map(Some),
    );
    Integracao {
        titulo: "Issues (Jira)".into(),
        passos: b.passos,
    }
}

fn prontidao_glab(g: &GitlabConfig, f: &Ferramentas, titulo: &str) -> Integracao {
    let mut b = Construtor::novo();
    let cred: std::cell::RefCell<Option<Secret>> = Default::default();
    b.passo(
        "cli",
        "glab instalado",
        "Instale o glab (https://gitlab.com/gitlab-org/cli) e deixe-o no PATH.",
        || {
            let mut c = Command::new(&f.glab);
            c.arg("version");
            saida(c, None).map(Some)
        },
    );
    b.passo(
        "credencial",
        "Token do GitLab disponível",
        "Cadastre o token global do host em Configurações › Acessos, ou rode o comando de credencial \
         num terminal (o cache do gpg-agent pode ter expirado).",
        || {
            let s = crate::demanda::token_gitlab(g, f).map_err(|e| e.to_string())?;
            *cred.borrow_mut() = Some(s);
            Ok(None)
        },
    );
    let token = cred.borrow();
    b.passo(
        "token",
        "Token aceito pela API do GitLab",
        if g.credential_cmd.trim().is_empty() {
            "Gere outro token no GitLab (escopo `read_api`) e atualize o token global do host em \
             Configurações › Acessos."
        } else {
            "Gere outro token no GitLab (escopo `read_api`) e confira o comando de credencial."
        },
        || match token.as_ref() {
            Some(t) => verificar_token_host(ForgeKind::Gitlab, g.host.trim(), t, f)
                .map(|v| v.usuario.map(|u| format!("token de {u}")))
                .map_err(|e| e.to_string()),
            None => Ok(None),
        },
    );
    b.passo(
        "api",
        "API do GitLab responde (glab api version)",
        "Confira o host do cadastro e se o token tem o escopo `read_api`.",
        || {
            let mut c = Command::new(&f.glab);
            c.args(["api", "version"]).env("GITLAB_HOST", g.host.trim());
            if let Some(t) = token.as_ref() {
                c.env("GITLAB_TOKEN", t.expose());
            }
            saida(c, token.as_ref()).map(Some)
        },
    );
    Integracao {
        titulo: titulo.into(),
        passos: b.passos,
    }
}

fn prontidao_gh(g: &GithubConfig, f: &Ferramentas, titulo: &str) -> Integracao {
    let mut b = Construtor::novo();
    let cred: std::cell::RefCell<Option<Secret>> = Default::default();
    b.passo(
        "cli",
        "gh instalado",
        "Instale o GitHub CLI (https://cli.github.com) e deixe-o no PATH.",
        || {
            let mut c = Command::new(&f.gh);
            c.arg("--version");
            saida(c, None).map(Some)
        },
    );
    b.passo(
        "credencial",
        "gh autenticado (token do chaveiro ou gh auth token)",
        "Cadastre o token do github.com em Configurações › Acessos ou rode `gh auth login` num terminal.",
        || {
            if let Some(t) = crate::demanda::token_github(f) {
                *cred.borrow_mut() = Some(t);
                return Ok(Some("token global do github.com (chaveiro)".into()));
            }
            let mut c = Command::new(&f.gh);
            c.args(["auth", "token"]);
            // Só interessa que haja token; o valor não é guardado nem mostrado.
            saida(c, None).and_then(|t| {
                if t.is_empty() {
                    Err("sem token".into())
                } else {
                    Ok(None)
                }
            })
        },
    );
    let token = cred.borrow();
    b.passo(
        "token",
        "Token aceito pela API do GitHub",
        "Gere outro token no GitHub e atualize o token global do github.com em \
         Configurações › Acessos.",
        || match token.as_ref() {
            Some(t) => verificar_token_host(ForgeKind::Github, HOST_GITHUB, t, f)
                .map(|v| v.usuario.map(|u| format!("token de {u}")))
                .map_err(|e| e.to_string()),
            // Sem token no chaveiro, vale a autenticação do próprio gh: o passo
            // seguinte a confere.
            None => Ok(Some("autenticação do gh".into())),
        },
    );
    b.passo(
        "issues",
        "Repositório com as issues ligadas",
        "Ligue as Issues do repositório no GitHub (Settings › General › Features) ou escolha \
         outro repositório, e confira se o token tem acesso a ele.",
        || issues_ligadas_github(g, f, token.as_ref()),
    );
    Integracao {
        titulo: titulo.into(),
        passos: b.passos,
    }
}

/// O repositório das demandas existe, o token o enxerga e as issues estão ligadas
/// (`gh api repos/{repo}`, campo `has_issues`). Um fork vem com elas desligadas.
fn issues_ligadas_github(
    g: &GithubConfig,
    f: &Ferramentas,
    token: Option<&Secret>,
) -> Result<Option<String>, String> {
    let repo = g.repo.trim().trim_matches('/');
    if repo.is_empty()
        || !repo
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/'))
    {
        return Err(format!("repositório das demandas inválido: {repo}"));
    }
    let mut c = Command::new(&f.gh);
    c.args(["api", &format!("repos/{repo}")])
        .env("GH_HOST", HOST_GITHUB);
    if let Some(t) = token {
        c.env("GH_TOKEN", t.expose());
    }
    let o = run(c, PRAZO_CLI).map_err(|e| cite(&e.to_string(), token))?;
    if !o.status.success() {
        let err = cite(&String::from_utf8_lossy(&o.stderr), token);
        return Err(motivo_da_falha_do_github(repo, &err));
    }
    let v: serde_json::Value = serde_json::from_slice(&o.stdout)
        .map_err(|_| format!("resposta inesperada do GitHub para {repo}"))?;
    if v["has_issues"].as_bool() == Some(false) {
        return Err(format!("as issues estão desligadas em {repo}"));
    }
    Ok(Some(repo.to_string()))
}

/// A falha do `gh api repos/{repo}` em português: repositório que não existe (ou
/// que o token não enxerga), token sem acesso ou o texto do próprio `gh`.
fn motivo_da_falha_do_github(repo: &str, erro: &str) -> String {
    let e = erro.to_lowercase();
    if e.contains("404") || e.contains("not found") {
        format!(
            "repositório {repo} não encontrado (ele não existe ou o token não tem acesso a ele)"
        )
    } else if e.contains("401") || e.contains("403") || e.contains("bad credentials") {
        format!("sem acesso ao repositório {repo} com o token atual")
    } else if erro.trim().is_empty() {
        format!("o GitHub não devolveu {repo}")
    } else {
        erro.to_string()
    }
}

/// Repositório de código: git instalado, token (chaveiro, comando ou nenhum),
/// token aceito pela API da forja (#215) e acesso de leitura (`git ls-remote`).
fn prontidao_codigo(repo: &RepositorioCodigo, f: &Ferramentas) -> Integracao {
    let mut b = Construtor::novo();
    let cred: std::cell::RefCell<Option<Secret>> = Default::default();
    b.passo(
        "git",
        "git instalado",
        "Instale o git (https://git-scm.com) e deixe-o no PATH.",
        || {
            let mut c = Command::new(&f.git);
            c.arg("--version");
            saida(c, None).map(Some)
        },
    );
    b.passo(
        "credencial",
        "Token disponível",
        "Informe o token de novo (Configurações › Acessos ou o cadastro do repositório) ou \
         confira o comando de credencial.",
        || {
            let t = token_do_repositorio(repo, f.chaveiro.as_ref()).map_err(|e| e.to_string())?;
            let detalhe = match (&t, repo.token) {
                (None, _) => "sem token: chave SSH ou credencial do próprio git".to_string(),
                (Some(_), OrigemToken::Comando) => "comando de credencial".into(),
                (Some(_), _) if proprio_no_chaveiro(repo, f) => {
                    "token próprio do repositório (chaveiro)".into()
                }
                (Some(_), _) => format!("token global de {} (chaveiro)", repo.host.trim()),
            };
            *cred.borrow_mut() = t;
            Ok(Some(detalhe))
        },
    );
    let token = cred.borrow();
    let orientacao_token = match repo.token {
        OrigemToken::Comando => {
            "Gere outro token na forja (com leitura do repositório) e confira o comando de \
             credencial."
        }
        _ if proprio_no_chaveiro(repo, f) => {
            "Gere outro token na forja (com leitura do repositório) e informe-o no cadastro do \
             repositório."
        }
        _ => {
            "Gere outro token na forja (com leitura do repositório) e atualize o token global do \
             host em Configurações › Acessos."
        }
    };
    b.passo(
        "token",
        &format!("Token aceito pela API do {}", repo.forja.label()),
        orientacao_token,
        || match token.as_ref() {
            None => Ok(Some("sem token: nada a conferir na API".into())),
            Some(t) => match verificar_token_repositorio(repo, t, f) {
                Ok(()) => Ok(None),
                // Sem a CLI da forja, o passo seguinte confere o token pelo git.
                Err(ErroVerificacao::SemFerramenta(cli)) => {
                    Ok(Some(format!("{cli} não encontrado: conferido só pelo git")))
                }
                Err(ErroVerificacao::Recusado(m)) => Err(m),
            },
        },
    );
    b.passo(
        "acesso",
        "Acesso de leitura (git ls-remote)",
        "Confira a URL de clone e se o token tem permissão de leitura do repositório.",
        || testar_acesso(&f.git, repo, token.as_ref()).map(|()| None),
    );
    Integracao {
        titulo: format!("Código ({})", repo.rotulo()),
        passos: b.passos,
    }
}

/// O repositório tem token próprio guardado no chaveiro (o que vale antes do
/// global do host).
fn proprio_no_chaveiro(repo: &RepositorioCodigo, f: &Ferramentas) -> bool {
    repo.token != OrigemToken::Comando
        && matches!(f.chaveiro.ler(&conta_propria(repo)), Ok(Some(_)))
}

/// Prontidão só das demandas (o "Testar conexão" do cadastro e da página do
/// projeto): a mesma integração que [`conferir`] traz por último.
pub fn prontidao_tracker(t: &TrackerConfig, f: &Ferramentas) -> Integracao {
    match t {
        TrackerConfig::Github(g) => prontidao_gh(g, f, "Issues (GitHub)"),
        TrackerConfig::Gitlab(g) => prontidao_glab(g, f, "Issues (GitLab)"),
        TrackerConfig::Jira(j) => prontidao_jira(j, f),
    }
}

/// Confere as integrações cadastradas do projeto.
pub fn conferir(r: &Repositorios, f: &Ferramentas) -> Prontidao {
    let mut integracoes = Vec::new();
    for repo in &r.codigo {
        integracoes.push(prontidao_codigo(repo, f));
    }
    match &r.tracker {
        Some(TrackerConfig::Github(g)) => integracoes.push(prontidao_gh(g, f, "Issues (GitHub)")),
        Some(TrackerConfig::Gitlab(g)) => integracoes.push(prontidao_glab(g, f, "Issues (GitLab)")),
        Some(TrackerConfig::Jira(j)) => integracoes.push(prontidao_jira(j, f)),
        None => {}
    }
    Prontidao { integracoes }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::demanda::falsas::script;

    fn jira_cfg(dir: &std::path::Path, server: &str) -> JiraConfig {
        let yml = dir.join("jira.yml");
        std::fs::write(&yml, format!("installation: Local\nserver: {server}\n")).unwrap();
        JiraConfig {
            url: "https://jira.example.com".into(),
            project_key: "PROJ".into(),
            credential_cmd: "echo tok-secreto".into(),
            config_file: yml.display().to_string(),
            ..Default::default()
        }
    }

    fn jira_falso(dir: &std::path::Path) -> Ferramentas {
        let jira = script(
            dir,
            "jira",
            r#"case "$1" in
version) echo "jira version 1.7.0";;
me) [ "$JIRA_API_TOKEN" = "tok-secreto" ] && echo "ana" || { echo "401" >&2; exit 1; };;
serverinfo) echo "9.12.0";;
esac"#,
        );
        Ferramentas {
            jira,
            ..Default::default()
        }
    }

    #[test]
    fn jira_tudo_certo() {
        let d = tempfile::tempdir().unwrap();
        let i = prontidao_jira(
            &jira_cfg(d.path(), "https://jira.example.com/"),
            &jira_falso(d.path()),
        );
        assert!(i.pronta(), "{i:?}");
        assert_eq!(i.passos.len(), 6);
        assert_eq!(i.passos[5].detalhe.as_deref(), Some("9.12.0"));
        assert!(!format!("{i:?}").contains("tok-secreto"));
    }

    #[test]
    fn jira_servidor_diferente_para_nos_passos_seguintes() {
        let d = tempfile::tempdir().unwrap();
        let i = prontidao_jira(
            &jira_cfg(d.path(), "https://outro.example.com"),
            &jira_falso(d.path()),
        );
        let sit: Vec<_> = i.passos.iter().map(|p| p.situacao).collect();
        assert_eq!(
            sit,
            [
                Situacao::Ok,
                Situacao::Ok,
                Situacao::Falhou,
                Situacao::Pulado,
                Situacao::Pulado,
                Situacao::Pulado
            ]
        );
        assert!(i.passos[2].orientacao.is_some());
        assert!(!i.pronta());
    }

    #[test]
    fn jira_cli_ausente_e_credencial_que_falha() {
        let d = tempfile::tempdir().unwrap();
        let sem = Ferramentas {
            jira: d.path().join("nada"),
            ..Default::default()
        };
        let i = prontidao_jira(&jira_cfg(d.path(), "https://jira.example.com"), &sem);
        assert_eq!(i.passos[0].situacao, Situacao::Falhou);
        assert!(
            i.passos[0]
                .orientacao
                .as_deref()
                .unwrap()
                .contains("jira-cli")
        );

        let mut j = jira_cfg(d.path(), "https://jira.example.com");
        j.credential_cmd = "printf 'segredo-%s\\n' vazado; exit 2".into();
        let i = prontidao_jira(&j, &jira_falso(d.path()));
        assert_eq!(i.passos[3].situacao, Situacao::Falhou);
        assert!(!format!("{i:?}").contains("segredo-vazado"));
    }

    #[test]
    fn gitlab_e_github() {
        use crate::chaveiro::{Chaveiro, ChaveiroMemoria, conta_global, conta_propria};
        use crate::{ForgeKind, OrigemToken, RepositorioCodigo};
        let d = tempfile::tempdir().unwrap();
        let gh = script(
            d.path(),
            "gh",
            r#"case "$1 $2" in
"--version "*) echo "gh 2.60";;
"auth token") echo ghp_x;;
"api repos/o/web") [ "$GH_TOKEN" = so-do-web ] || { echo "gh: Bad credentials (HTTP 401)" >&2; exit 1; }; echo '{}';;
"api repos/o/r") echo '{"has_issues":true}';;
*) exit 2;;
esac"#,
        );
        let glab = script(
            d.path(),
            "glab",
            r#"case "$1 $2" in
"api projects/g%2Fp%2Fapp") [ "$GITLAB_TOKEN" = tok ] || { echo "glab: 401 Unauthorized (HTTP 401)" >&2; exit 1; }; echo '{}';;
*) exit 2;;
esac"#,
        );
        // git falso: o ls-remote só passa com o token esperado de cada repositório.
        let git = script(
            d.path(),
            "git",
            r#"case "$*" in
--version) echo "git version 2.45";;
*ls-remote*gitlab.example.com/g/p/app*) [ "$CBA_GIT_TOKEN" = tok ] || { echo "Authentication failed" >&2; exit 128; };;
*ls-remote*github.com/o/web*) [ "$CBA_GIT_TOKEN" = so-do-web ] || { echo "Authentication failed" >&2; exit 128; };;
*) exit 2;;
esac"#,
        );
        let chaveiro = std::sync::Arc::new(ChaveiroMemoria::default());
        let f = Ferramentas {
            gh,
            glab,
            git,
            chaveiro: chaveiro.clone(),
            ..Default::default()
        };
        let web = RepositorioCodigo {
            caminho: "o/web".into(),
            token: OrigemToken::Proprio,
            ..Default::default()
        };
        let r = Repositorios {
            codigo: vec![
                RepositorioCodigo {
                    forja: ForgeKind::Gitlab,
                    host: "gitlab.example.com".into(),
                    caminho: "g/p/app".into(),
                    url_clone: "https://gitlab.example.com/g/p/app.git".into(),
                    ..Default::default()
                },
                web.clone(),
            ],
            tracker: Some(TrackerConfig::Github(GithubConfig { repo: "o/r".into() })),
        };
        let p = conferir(&r, &f);
        assert!(!p.pronta());
        assert_eq!(p.integracoes.len(), 3);
        assert_eq!(p.integracoes[0].titulo, "Código (GitLab g/p/app)");
        assert_eq!(p.integracoes[0].passos[3].situacao, Situacao::Falhou);
        assert_eq!(p.integracoes[1].passos[1].situacao, Situacao::Falhou);
        chaveiro
            .gravar(&conta_global("gitlab.example.com"), &Secret::new("tok"))
            .unwrap();
        chaveiro
            .gravar(&conta_propria(&web), &Secret::new("so-do-web"))
            .unwrap();
        let p = conferir(&r, &f);
        assert!(p.pronta(), "{p:?}");
        let texto = format!("{p:?}");
        for segredo in ["ghp_x", "so-do-web", "\"tok\""] {
            assert!(!texto.contains(segredo), "{segredo}: {texto}");
        }
        // Sem nada cadastrado, nada a conferir.
        assert!(conferir(&Repositorios::default(), &f).pronta());
    }

    /// git falso que aceita qualquer coisa, como um repositório público lido sem
    /// mandar o token.
    fn git_publico(dir: &std::path::Path) -> std::path::PathBuf {
        script(
            dir,
            "git",
            r#"case "$*" in --version) echo "git version 2.45";; esac; exit 0"#,
        )
    }

    #[test]
    fn token_falso_em_repositorio_publico_falha_na_api() {
        use crate::RepositorioCodigo;
        use crate::chaveiro::{Chaveiro, ChaveiroMemoria, conta_global};
        let d = tempfile::tempdir().unwrap();
        let gh = script(
            d.path(),
            "gh",
            r#"[ "$GH_TOKEN" = ghp-bom ] || { echo "gh: Bad credentials (HTTP 401)" >&2; exit 1; }
[ "$GH_HOST" = github.com ] || exit 3
echo '{"login":"ana"}'"#,
        );
        let chaveiro = std::sync::Arc::new(ChaveiroMemoria::default());
        chaveiro
            .gravar(&conta_global("github.com"), &Secret::new("ghp-falso-123"))
            .unwrap();
        let f = Ferramentas {
            gh,
            git: git_publico(d.path()),
            chaveiro: chaveiro.clone(),
            ..Default::default()
        };
        let repo = RepositorioCodigo {
            caminho: "o/publico".into(),
            ..Default::default()
        };
        let i = prontidao_codigo(&repo, &f);
        let ids: Vec<_> = i
            .passos
            .iter()
            .map(|p| (p.id.as_str(), p.situacao))
            .collect();
        assert_eq!(
            ids,
            [
                ("git", Situacao::Ok),
                ("credencial", Situacao::Ok),
                ("token", Situacao::Falhou),
                ("acesso", Situacao::Pulado)
            ]
        );
        assert_eq!(
            i.passos[1].detalhe.as_deref(),
            Some("token global de github.com (chaveiro)")
        );
        let msg = i.passos[2].detalhe.clone().unwrap();
        assert!(msg.contains("recusou o token"), "{msg}");
        assert!(
            i.passos[2]
                .orientacao
                .as_deref()
                .unwrap()
                .contains("Acessos"),
            "{i:?}"
        );
        assert!(!format!("{i:?}").contains("ghp-falso-123"));

        // Com o token bom, passa.
        chaveiro
            .gravar(&conta_global("github.com"), &Secret::new("ghp-bom"))
            .unwrap();
        assert!(prontidao_codigo(&repo, &f).pronta());

        // Sem o gh, o passo da API não barra: o git confere o token.
        let sem_gh = Ferramentas {
            gh: d.path().join("nada"),
            ..f.clone()
        };
        let i = prontidao_codigo(&repo, &sem_gh);
        assert!(i.pronta(), "{i:?}");
        assert!(
            i.passos[2]
                .detalhe
                .as_deref()
                .unwrap()
                .contains("só pelo git")
        );
    }

    #[test]
    fn motivo_da_falha_do_github_em_portugues() {
        let m = motivo_da_falha_do_github("o/x", "gh: Not Found (HTTP 404)");
        assert!(m.contains("não encontrado"), "{m}");
        let m = motivo_da_falha_do_github("o/x", "gh: Forbidden (HTTP 403)");
        assert!(m.contains("sem acesso"), "{m}");
        assert!(motivo_da_falha_do_github("o/x", "").contains("não devolveu"));
    }

    #[test]
    fn github_issues_com_token_invalido_ou_desligadas() {
        use crate::chaveiro::{Chaveiro, ChaveiroMemoria, conta_global};
        let d = tempfile::tempdir().unwrap();
        let gh = script(
            d.path(),
            "gh",
            r#"case "$1 $2" in
"--version "*) echo "gh 2.60";;
"api -i") [ "$GH_TOKEN" = ghp-bom ] || { echo "gh: Bad credentials (HTTP 401)" >&2; exit 1; }; printf 'HTTP/2.0 200 OK\r\n\r\n{"login":"ana"}';;
"api repos/o/fork") echo '{"has_issues":false}';;
"api repos/o/r") echo '{"has_issues":true}';;
*) exit 2;;
esac"#,
        );
        let chaveiro = std::sync::Arc::new(ChaveiroMemoria::default());
        chaveiro
            .gravar(&conta_global("github.com"), &Secret::new("ghp-ruim"))
            .unwrap();
        let f = Ferramentas {
            gh,
            chaveiro: chaveiro.clone(),
            ..Default::default()
        };
        let cfg = |repo: &str| GithubConfig { repo: repo.into() };
        let i = prontidao_gh(&cfg("o/r"), &f, "Issues (GitHub)");
        assert_eq!(i.passos[2].id, "token");
        assert_eq!(i.passos[2].situacao, Situacao::Falhou);
        assert!(!format!("{i:?}").contains("ghp-ruim"));

        chaveiro
            .gravar(&conta_global("github.com"), &Secret::new("ghp-bom"))
            .unwrap();
        let i = prontidao_gh(&cfg("o/r"), &f, "Issues (GitHub)");
        assert!(i.pronta(), "{i:?}");
        assert_eq!(i.passos[2].detalhe.as_deref(), Some("token de ana"));

        let i = prontidao_gh(&cfg("o/fork"), &f, "Issues (GitHub)");
        assert_eq!(i.passos[3].id, "issues");
        assert_eq!(i.passos[3].situacao, Situacao::Falhou);
        assert!(
            i.passos[3]
                .detalhe
                .as_deref()
                .unwrap()
                .contains("desligadas"),
            "{i:?}"
        );
    }
}
