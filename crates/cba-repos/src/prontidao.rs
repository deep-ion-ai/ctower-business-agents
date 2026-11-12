//! Prontidão das integrações de um projeto: cada passo com a orientação quando
//! falha. Não bloqueia o uso do app sem as integrações.
//!
//! Passos do ctower-agents. Jira: jira-cli instalado, `.config.yml` presente,
//! `server` igual à `url`, comando de credencial, `jira me` e `jira serverinfo`.
//! GitLab: `glab version` e `glab api version`. GitHub: `gh` e `gh auth token`.

use crate::demanda::{Ferramentas, PRAZO_CLI};
use crate::process::{RunError, cite, run};
use crate::{
    CREDENTIAL_DEADLINE, ForgeConfig, GithubConfig, GitlabConfig, JiraConfig, Repositorios, Secret,
    TrackerConfig, run_credential,
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
        "Comando de credencial responde",
        "Rode o comando de credencial num terminal; o cache do gpg-agent pode ter expirado.",
        || {
            let s = run_credential("Jira", &j.credential_cmd, CREDENTIAL_DEADLINE)
                .map_err(|e| e.to_string())?;
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
        "Comando de credencial responde",
        "Rode o comando de credencial num terminal; o cache do gpg-agent pode ter expirado.",
        || {
            let s = run_credential("GitLab", &g.credential_cmd, CREDENTIAL_DEADLINE)
                .map_err(|e| e.to_string())?;
            *cred.borrow_mut() = Some(s);
            Ok(None)
        },
    );
    let token = cred.borrow();
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

fn prontidao_gh(_g: &GithubConfig, f: &Ferramentas, titulo: &str) -> Integracao {
    let mut b = Construtor::novo();
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
        "gh autenticado (gh auth token)",
        "Rode `gh auth login` num terminal.",
        || {
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
    Integracao {
        titulo: titulo.into(),
        passos: b.passos,
    }
}

/// Confere as integrações cadastradas do projeto.
pub fn conferir(r: &Repositorios, f: &Ferramentas) -> Prontidao {
    let mut integracoes = Vec::new();
    match &r.forge {
        Some(ForgeConfig::Github(g)) => integracoes.push(prontidao_gh(g, f, "Código (GitHub)")),
        Some(ForgeConfig::Gitlab(g)) => integracoes.push(prontidao_glab(g, f, "Código (GitLab)")),
        None => {}
    }
    match &r.tracker {
        Some(TrackerConfig::Github(g)) => integracoes.push(prontidao_gh(g, f, "Issues (GitHub)")),
        Some(TrackerConfig::Gitlab(g)) => integracoes.push(prontidao_glab(g, f, "Issues (GitLab)")),
        Some(TrackerConfig::Jira(j)) => integracoes.push(prontidao_jira(j, f)),
        None => {}
    }
    Prontidao { integracoes }
}

#[cfg(test)]
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
        let d = tempfile::tempdir().unwrap();
        let glab = script(
            d.path(),
            "glab",
            r#"case "$1" in version) echo "glab 1.50";; api) [ "$GITLAB_TOKEN" = tok ] && echo '{"version":"17"}' || exit 1;; esac"#,
        );
        let gh = script(
            d.path(),
            "gh",
            r#"case "$1" in --version) echo "gh 2.60";; auth) echo ghp_x;; esac"#,
        );
        let f = Ferramentas {
            glab,
            gh,
            ..Default::default()
        };
        let r = Repositorios {
            forge: Some(ForgeConfig::Gitlab(GitlabConfig {
                host: "gitlab.example.com".into(),
                path: "g/p/app".into(),
                credential_cmd: "echo tok".into(),
                ..Default::default()
            })),
            tracker: Some(TrackerConfig::Github(GithubConfig { repo: "o/r".into() })),
        };
        let p = conferir(&r, &f);
        assert!(p.pronta(), "{p:?}");
        assert_eq!(p.integracoes.len(), 2);
        assert!(!format!("{p:?}").contains("ghp_x"));
        // Sem nada cadastrado, nada a conferir.
        assert!(conferir(&Repositorios::default(), &f).pronta());
    }
}
