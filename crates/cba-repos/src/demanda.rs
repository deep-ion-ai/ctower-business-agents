//! Busca de uma issue (título e descrição) para iniciar uma análise.
//!
//! Pelas CLIs, sem HTTP próprio: `gh issue view`, `glab api` e
//! `jira issue view --raw` (JSON da API v2). O token chega ao processo filho só
//! no ambiente. Sem escrita no rastreador.

use crate::process::{self, RunError, cite};
use crate::{
    CREDENTIAL_DEADLINE, CredentialError, GithubConfig, GitlabConfig, JiraConfig, Secret,
    TrackerConfig, run_credential,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

/// Prazo de uma chamada às CLIs.
pub const PRAZO_CLI: Duration = Duration::from_secs(60);

/// Binários usados; configuráveis para os testes usarem CLIs falsas.
#[derive(Debug, Clone)]
pub struct Ferramentas {
    pub gh: PathBuf,
    pub glab: PathBuf,
    pub jira: PathBuf,
}

impl Default for Ferramentas {
    fn default() -> Self {
        Ferramentas {
            gh: "gh".into(),
            glab: "glab".into(),
            jira: "jira".into(),
        }
    }
}

/// Issue lida do rastreador.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Demanda {
    /// `PROJ-123`, `#123` ou `!`… a referência como o analista a conhece.
    pub chave: String,
    pub titulo: String,
    pub descricao: String,
    pub url: String,
}

#[derive(Debug, thiserror::Error)]
pub enum DemandaErro {
    #[error("referência de issue inválida: {0:?}")]
    Referencia(String),
    #[error("{0}")]
    Credencial(#[from] CredentialError),
    #[error("{ferramenta} não encontrado: instale-o e confira a prontidão do projeto")]
    SemFerramenta { ferramenta: &'static str },
    #[error("{ferramenta} falhou: {detalhe}")]
    Falhou {
        ferramenta: &'static str,
        detalhe: String,
    },
    #[error("resposta de {ferramenta} ilegível: {detalhe}")]
    Resposta {
        ferramenta: &'static str,
        detalhe: String,
    },
}

/// Roda a CLI com o token só no ambiente do filho; devolve o JSON do `stdout`.
fn rodar_json(
    ferramenta: &'static str,
    cmd: Command,
    token: &Secret,
) -> Result<Value, DemandaErro> {
    let o = process::run(cmd, PRAZO_CLI).map_err(|e| match e {
        RunError::NotFound => DemandaErro::SemFerramenta { ferramenta },
        e => DemandaErro::Falhou {
            ferramenta,
            detalhe: cite(&e.to_string(), Some(token)),
        },
    })?;
    if !o.status.success() {
        return Err(DemandaErro::Falhou {
            ferramenta,
            detalhe: cite(&String::from_utf8_lossy(&o.stderr), Some(token)),
        });
    }
    serde_json::from_slice(&o.stdout).map_err(|e| DemandaErro::Resposta {
        ferramenta,
        detalhe: e.to_string(),
    })
}

fn texto<'a>(v: &'a Value, caminho: &[&str]) -> &'a str {
    let mut cur = v;
    for k in caminho {
        cur = &cur[*k];
    }
    cur.as_str().unwrap_or("")
}

/// Número da issue em `123`, `#123` ou `!123`.
fn numero(ref_: &str) -> Result<u64, DemandaErro> {
    ref_.trim()
        .trim_start_matches(['#', '!'])
        .parse::<u64>()
        .ok()
        .filter(|n| *n > 0)
        .ok_or_else(|| DemandaErro::Referencia(ref_.to_string()))
}

pub fn buscar_github(
    g: &GithubConfig,
    ref_: &str,
    f: &Ferramentas,
) -> Result<Demanda, DemandaErro> {
    let n = numero(ref_)?;
    // `gh auth token` é o token do ambiente do gh; o gh o usa sozinho, então
    // aqui não há credencial a injetar.
    let mut c = Command::new(&f.gh);
    c.args([
        "issue",
        "view",
        &n.to_string(),
        "-R",
        g.repo.trim(),
        "--json",
        "title,body,url",
    ]);
    let v = rodar_json("gh", c, &Secret::new(""))?;
    Ok(Demanda {
        chave: format!("#{n}"),
        titulo: texto(&v, &["title"]).to_string(),
        descricao: texto(&v, &["body"]).to_string(),
        url: texto(&v, &["url"]).to_string(),
    })
}

pub fn buscar_gitlab(
    g: &GitlabConfig,
    ref_: &str,
    f: &Ferramentas,
) -> Result<Demanda, DemandaErro> {
    let n = numero(ref_)?;
    let token = run_credential("GitLab", &g.credential_cmd, CREDENTIAL_DEADLINE)?;
    let id = g.path.trim_matches('/').replace('/', "%2F");
    let mut c = Command::new(&f.glab);
    c.args(["api", &format!("projects/{id}/issues/{n}")])
        .env("GITLAB_HOST", g.host.trim())
        .env("GITLAB_TOKEN", token.expose());
    let v = rodar_json("glab", c, &token)?;
    Ok(Demanda {
        chave: format!("#{n}"),
        titulo: texto(&v, &["title"]).to_string(),
        descricao: texto(&v, &["description"]).to_string(),
        url: texto(&v, &["web_url"]).to_string(),
    })
}

pub fn buscar_jira(j: &JiraConfig, ref_: &str, f: &Ferramentas) -> Result<Demanda, DemandaErro> {
    let chave = ref_.trim().to_ascii_uppercase();
    if j.number_from_key(&chave).is_none() {
        return Err(DemandaErro::Referencia(ref_.to_string()));
    }
    let token = run_credential("Jira", &j.credential_cmd, CREDENTIAL_DEADLINE)?;
    let mut c = Command::new(&f.jira);
    c.args(["issue", "view", &chave, "--raw"])
        .env("JIRA_API_TOKEN", token.expose());
    if !j.config_file.trim().is_empty() {
        c.env("JIRA_CONFIG_FILE", j.config_file.trim());
    }
    let v = rodar_json("jira", c, &token)?;
    Ok(Demanda {
        titulo: texto(&v, &["fields", "summary"]).to_string(),
        descricao: texto(&v, &["fields", "description"]).to_string(),
        url: j.issue_url(&chave),
        chave,
    })
}

/// Busca a issue no rastreador do projeto.
pub fn buscar_demanda(
    tracker: &TrackerConfig,
    ref_: &str,
    f: &Ferramentas,
) -> Result<Demanda, DemandaErro> {
    match tracker {
        TrackerConfig::Github(g) => buscar_github(g, ref_, f),
        TrackerConfig::Gitlab(g) => buscar_gitlab(g, ref_, f),
        TrackerConfig::Jira(j) => buscar_jira(j, ref_, f),
    }
}

#[cfg(test)]
pub(crate) mod falsas {
    //! CLIs falsas para os testes: scripts de shell numa pasta temporária.
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};

    pub fn script(dir: &Path, nome: &str, corpo: &str) -> PathBuf {
        let p = dir.join(nome);
        std::fs::write(&p, format!("#!/bin/sh\n{corpo}\n")).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use falsas::script;

    #[test]
    fn jira_traz_titulo_descricao_e_link_com_o_token_so_no_ambiente() {
        let d = tempfile::tempdir().unwrap();
        // O jira falso só responde se o token vier no ambiente.
        let jira = script(
            d.path(),
            "jira",
            r#"[ "$JIRA_API_TOKEN" = "tok-secreto" ] || { echo "sem token" >&2; exit 1; }
[ "$1 $2 $4" = "issue view --raw" ] || exit 2
echo "{\"key\":\"$3\",\"fields\":{\"summary\":\"Cancelar nota\",\"description\":\"Regras da tela\"}}""#,
        );
        let f = Ferramentas {
            jira,
            ..Default::default()
        };
        let j = JiraConfig {
            url: "https://jira.example.com".into(),
            project_key: "PROJ".into(),
            credential_cmd: "echo tok-secreto".into(),
            ..Default::default()
        };
        let dem = buscar_jira(&j, "proj-123", &f).unwrap();
        assert_eq!(dem.chave, "PROJ-123");
        assert_eq!(dem.titulo, "Cancelar nota");
        assert_eq!(dem.descricao, "Regras da tela");
        assert_eq!(dem.url, "https://jira.example.com/browse/PROJ-123");
        assert!(matches!(
            buscar_jira(&j, "OUTRO-1", &f),
            Err(DemandaErro::Referencia(_))
        ));
    }

    #[test]
    fn erro_do_jira_nao_vaza_o_token() {
        let d = tempfile::tempdir().unwrap();
        let jira = script(
            d.path(),
            "jira",
            r#"echo "401 com $JIRA_API_TOKEN" >&2; exit 1"#,
        );
        let f = Ferramentas {
            jira,
            ..Default::default()
        };
        let j = JiraConfig {
            url: "https://jira.example.com".into(),
            project_key: "PROJ".into(),
            credential_cmd: "echo tok-secreto".into(),
            ..Default::default()
        };
        let e = buscar_jira(&j, "PROJ-1", &f).unwrap_err().to_string();
        assert!(e.contains("401") && !e.contains("tok-secreto"), "{e}");
    }

    #[test]
    fn gitlab_usa_host_e_token_no_ambiente() {
        let d = tempfile::tempdir().unwrap();
        let glab = script(
            d.path(),
            "glab",
            r#"[ "$GITLAB_TOKEN" = "tok" ] && [ "$GITLAB_HOST" = "gitlab.example.com" ] || exit 1
[ "$2" = "projects/g%2Fp%2Fapp/issues/7" ] || exit 2
echo '{"title":"T","description":"D","web_url":"https://gitlab.example.com/g/p/app/-/issues/7"}'"#,
        );
        let f = Ferramentas {
            glab,
            ..Default::default()
        };
        let g = GitlabConfig {
            host: "gitlab.example.com".into(),
            path: "g/p/app".into(),
            credential_cmd: "echo tok".into(),
            ..Default::default()
        };
        let dem = buscar_gitlab(&g, "#7", &f).unwrap();
        assert_eq!((dem.titulo.as_str(), dem.descricao.as_str()), ("T", "D"));
        assert_eq!(dem.url, "https://gitlab.example.com/g/p/app/-/issues/7");
    }

    #[test]
    fn github_e_ferramenta_ausente() {
        let d = tempfile::tempdir().unwrap();
        let gh = script(
            d.path(),
            "gh",
            r#"[ "$3 $4 $5" = "5 -R o/r" ] || exit 2
echo '{"title":"T","body":"B","url":"https://github.com/o/r/issues/5"}'"#,
        );
        let f = Ferramentas {
            gh,
            ..Default::default()
        };
        let dem = buscar_github(&GithubConfig { repo: "o/r".into() }, "5", &f).unwrap();
        assert_eq!(dem.url, "https://github.com/o/r/issues/5");
        let sem = Ferramentas {
            gh: d.path().join("nao-existe"),
            ..Default::default()
        };
        assert!(matches!(
            buscar_github(&GithubConfig { repo: "o/r".into() }, "5", &sem),
            Err(DemandaErro::SemFerramenta { .. })
        ));
        assert!(matches!(
            buscar_github(&GithubConfig { repo: "o/r".into() }, "abc", &f),
            Err(DemandaErro::Referencia(_))
        ));
    }
}
