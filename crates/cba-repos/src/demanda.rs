//! Busca de uma issue (título e descrição) para iniciar uma análise.
//!
//! Pelas CLIs, sem HTTP próprio: `gh issue view`, `glab api` e
//! `jira issue view --raw` (JSON da API v2). O token chega ao processo filho só
//! no ambiente. Sem escrita no rastreador.

use crate::chaveiro::{Chaveiro, ChaveiroNativo, ErroToken};
use crate::codigo::HOST_GITHUB;
use crate::process::{self, RunError, cite};
use crate::providers::{Recusada, com_nova_tentativa, legado, texto_de_recusa};
use crate::{CredentialError, GithubConfig, GitlabConfig, JiraConfig, Secret, TrackerConfig};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

/// Prazo de uma chamada às CLIs.
pub const PRAZO_CLI: Duration = Duration::from_secs(60);

/// Binários e chaveiro usados; configuráveis para os testes usarem CLIs falsas
/// e um chaveiro em memória.
#[derive(Clone)]
pub struct Ferramentas {
    pub gh: PathBuf,
    pub glab: PathBuf,
    pub jira: PathBuf,
    pub git: PathBuf,
    /// Onde estão os tokens (PAT) globais por host e próprios de repositório.
    pub chaveiro: Arc<dyn Chaveiro>,
}

impl Default for Ferramentas {
    fn default() -> Self {
        Ferramentas {
            gh: "gh".into(),
            glab: "glab".into(),
            jira: "jira".into(),
            git: "git".into(),
            chaveiro: Arc::new(ChaveiroNativo),
        }
    }
}

impl fmt::Debug for Ferramentas {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Ferramentas")
            .field("gh", &self.gh)
            .field("glab", &self.glab)
            .field("jira", &self.jira)
            .field("git", &self.git)
            .finish_non_exhaustive()
    }
}

impl From<ErroToken> for DemandaErro {
    fn from(e: ErroToken) -> Self {
        match e {
            ErroToken::Comando(c) => DemandaErro::Credencial(c),
            e => DemandaErro::Chaveiro(e.to_string()),
        }
    }
}

/// Token do GitLab de issues: o do comando de credencial, se cadastrado; senão,
/// o token global do host (chaveiro ou comando do host, #179 e #193). Sai pelo
/// resolvedor (#436).
pub fn token_gitlab(g: &GitlabConfig, f: &Ferramentas) -> Result<Secret, DemandaErro> {
    legado::token_gitlab(g, f.chaveiro.as_ref())
        .map_err(ErroToken::from)?
        .ok_or_else(|| DemandaErro::SemToken(g.host.trim().to_string()))
}

/// Token do Jira: o do comando de credencial, se cadastrado; senão, o do host
/// do Jira no chaveiro (#180). Sai pelo resolvedor (#436).
pub fn token_jira(j: &JiraConfig, f: &Ferramentas) -> Result<Secret, DemandaErro> {
    legado::token_jira(j, f.chaveiro.as_ref())
        .map_err(ErroToken::from)?
        .ok_or_else(|| DemandaErro::SemToken(j.host()))
}

/// Token global do GitHub (chaveiro ou comando do host), se houver; sem ele,
/// vale o `gh auth`.
pub fn token_github(f: &Ferramentas) -> Option<Secret> {
    legado::token_do_host(HOST_GITHUB, "GitHub", f.chaveiro.as_ref())
        .ok()
        .flatten()
}

impl Recusada for DemandaErro {
    fn recusada(&self) -> bool {
        matches!(self, DemandaErro::Falhou { detalhe, .. } if texto_de_recusa(detalhe))
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
    #[error("{0}")]
    Chaveiro(String),
    #[error(
        "sem token para {0}: cadastre o token do host em Configurações › Acessos ou o comando \
         de credencial"
    )]
    SemToken(String),
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
pub(crate) fn rodar_json(
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

pub(crate) fn texto<'a>(v: &'a Value, caminho: &[&str]) -> &'a str {
    let mut cur = v;
    for k in caminho {
        cur = &cur[*k];
    }
    cur.as_str().unwrap_or("")
}

/// Número da issue em `123`, `#123` ou `!123`.
pub(crate) fn numero(ref_: &str) -> Result<u64, DemandaErro> {
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
    // Com token do GitHub no chaveiro, ele vai no ambiente (`GH_TOKEN`); sem ele,
    // o gh usa a própria autenticação (`gh auth login`).
    let v = com_nova_tentativa(
        || Ok(token_github(f)),
        |token| {
            let mut c = Command::new(&f.gh);
            if let Some(t) = token {
                c.env("GH_TOKEN", t.expose());
            }
            c.args([
                "issue",
                "view",
                &n.to_string(),
                "-R",
                g.repo.trim(),
                "--json",
                "title,body,url",
            ]);
            rodar_json("gh", c, token.unwrap_or(&Secret::new("")))
        },
    )?;
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
    let id = g.path.trim_matches('/').replace('/', "%2F");
    let v = com_nova_tentativa(
        || token_gitlab(g, f).map(Some),
        |token| {
            let token = token.ok_or_else(|| DemandaErro::SemToken(g.host.trim().into()))?;
            let mut c = Command::new(&f.glab);
            c.args(["api", &format!("projects/{id}/issues/{n}")])
                .env("GITLAB_HOST", g.host.trim())
                .env("GITLAB_TOKEN", token.expose());
            rodar_json("glab", c, token)
        },
    )?;
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
    let v = com_nova_tentativa(
        || token_jira(j, f).map(Some),
        |token| {
            let token = token.ok_or_else(|| DemandaErro::SemToken(j.host()))?;
            let mut c = Command::new(&f.jira);
            c.args(["issue", "view", &chave, "--raw"])
                .env("JIRA_API_TOKEN", token.expose());
            if !j.config_file.trim().is_empty() {
                c.env("JIRA_CONFIG_FILE", j.config_file.trim());
            }
            rodar_json("jira", c, token)
        },
    )?;
    Ok(Demanda {
        titulo: texto(&v, &["fields", "summary"]).to_string(),
        descricao: wiki_para_markdown(texto(&v, &["fields", "description"])),
        url: j.issue_url(&chave),
        chave,
    })
}

/// Converte o markup wiki do Jira (API v2) em Markdown: títulos `hN.`, `*negrito*`,
/// `_itálico_`, listas `*`/`#`, `{{código}}`, `{code}`, `[texto|url]` e `----`.
/// Cobre o que as demandas usam no dia a dia; o que não reconhece fica como veio.
pub fn wiki_para_markdown(wiki: &str) -> String {
    use regex::Regex;
    let titulo = Regex::new(r"^h([1-6])\.\s+(.*)$").unwrap();
    let lista = Regex::new(r"^([*#-]+)\s+(.*)$").unwrap();
    let negrito = Regex::new(r"(^|[\s(])\*([^*\s][^*]*?)\*($|[\s).,;:])").unwrap();
    let italico = Regex::new(r"(^|[\s(])_([^_\s][^_]*?)_($|[\s).,;:])").unwrap();
    let mono = Regex::new(r"\{\{(.+?)\}\}").unwrap();
    let link = Regex::new(r"\[([^\]|]+)\|([^\]]+)\]").unwrap();
    let inline = |l: &str| -> String {
        let l = mono.replace_all(l, "`$1`");
        let l = link.replace_all(&l, "[$1]($2)");
        let l = negrito.replace_all(&l, "$1**$2**$3");
        let l = negrito.replace_all(&l, "$1**$2**$3");
        let l = italico.replace_all(&l, "$1*$2*$3");
        italico.replace_all(&l, "$1*$2*$3").into_owned()
    };
    let mut out = Vec::new();
    let mut em_codigo = false;
    for linha in wiki.replace("\r\n", "\n").lines() {
        if linha.trim_start().starts_with("{code") || linha.trim() == "{noformat}" {
            out.push("```".to_string());
            em_codigo = !em_codigo;
        } else if em_codigo {
            out.push(linha.to_string());
        } else if linha.trim() == "----" {
            out.push("---".to_string());
        } else if let Some(c) = titulo.captures(linha) {
            let n: usize = c[1].parse().unwrap_or(1);
            out.push(format!("{} {}", "#".repeat(n), inline(&c[2])));
        } else if let Some(c) = lista.captures(linha) {
            let marca = &c[1];
            let nivel = marca.len() - 1;
            let sinal = if marca.ends_with('#') { "1." } else { "-" };
            out.push(format!("{}{} {}", "  ".repeat(nivel), sinal, inline(&c[2])));
        } else {
            out.push(inline(linha));
        }
    }
    out.join("\n")
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

#[cfg(all(test, unix))]
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

#[cfg(all(test, unix))]
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
    fn wiki_do_jira_vira_markdown() {
        let w = "h3. Descrição\r\n\r\nTexto com *negrito* e _itálico_ e {{cod}}.\r\n*1. Item*\r\n* um\r\n** dois\r\n# passo\r\n[site|http://x.y]\r\n{code}\r\na *b*\r\n{code}";
        let m = wiki_para_markdown(w);
        assert_eq!(
            m,
            "### Descrição\n\nTexto com **negrito** e *itálico* e `cod`.\n**1. Item**\n- um\n  - dois\n1. passo\n[site](http://x.y)\n```\na *b*\n```"
        );
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
    fn gitlab_e_github_usam_o_token_global_do_chaveiro() {
        use crate::chaveiro::{ChaveiroMemoria, conta_global};
        let d = tempfile::tempdir().unwrap();
        let chaveiro = Arc::new(ChaveiroMemoria::default());
        let f = Ferramentas {
            glab: script(
                d.path(),
                "glab",
                r#"[ "$GITLAB_TOKEN" = "do-chaveiro" ] || { echo "sem token" >&2; exit 1; }
echo '{"title":"T","description":"D","web_url":"u"}'"#,
            ),
            gh: script(
                d.path(),
                "gh",
                r#"[ "$GH_TOKEN" = "gh-do-chaveiro" ] || { echo "sem token" >&2; exit 1; }
echo '{"title":"T","body":"B","url":"u"}'"#,
            ),
            chaveiro: chaveiro.clone(),
            ..Default::default()
        };
        let g = GitlabConfig {
            host: "gitlab.example.com".into(),
            path: "g/app".into(),
            ..Default::default()
        };
        let e = buscar_gitlab(&g, "1", &f).unwrap_err();
        assert!(matches!(e, DemandaErro::SemToken(_)), "{e}");
        chaveiro
            .gravar(
                &conta_global("gitlab.example.com"),
                &Secret::new("do-chaveiro"),
            )
            .unwrap();
        assert_eq!(buscar_gitlab(&g, "1", &f).unwrap().titulo, "T");
        let gh = GithubConfig { repo: "o/r".into() };
        assert!(buscar_github(&gh, "1", &f).is_err());
        chaveiro
            .gravar(&conta_global("github.com"), &Secret::new("gh-do-chaveiro"))
            .unwrap();
        assert_eq!(buscar_github(&gh, "1", &f).unwrap().descricao, "B");
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
