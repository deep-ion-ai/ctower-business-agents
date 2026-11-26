//! Estado das issues no rastreador, em lote, para conferir o status das
//! demandas (#292).
//!
//! Mesmas CLIs e credenciais de [`crate::demanda`], só leitura, com o token só
//! no ambiente do filho:
//!
//! - GitHub: `gh api graphql`, um `issueOrPullRequest` por número, até
//!   [`LOTE_GITHUB`] por consulta. Número que não existe vem `null` (e o `gh`
//!   sai com erro, mas com os dados no `stdout`): fica fora da resposta.
//! - GitLab: `glab api projects/:id/issues?iids[]=…`, até [`LOTE_GITLAB`] por
//!   chamada.
//! - Jira: `jira issue list --jql "key in (…)" --raw`; se a JQL falhar (uma chave
//!   que não existe derruba a consulta inteira), uma a uma com `jira issue view`.
//!   O estado sai da **categoria** do status (`new`, `indeterminate`, `done`),
//!   não do nome, que cada Jira escolhe.
//!
//! GitHub e GitLab não distinguem "em andamento": a issue é aberta ou encerrada.

use crate::demanda::{
    DemandaErro, Ferramentas, numero, rodar_json, texto, token_github, token_gitlab, token_jira,
};
use crate::process::{self, RunError, cite};
use crate::{GithubConfig, GitlabConfig, JiraConfig, Secret, TrackerConfig};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::process::Command;

/// Números por consulta GraphQL no GitHub.
pub const LOTE_GITHUB: usize = 50;
/// `iids[]` por chamada no GitLab (o máximo do `per_page`).
pub const LOTE_GITLAB: usize = 100;
/// Chaves por JQL no Jira.
pub const LOTE_JIRA: usize = 100;

/// Estado normalizado da issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EstadoIssue {
    Aberta,
    EmAndamento,
    Encerrada,
}

/// Estado lido de uma issue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EstadoLido {
    pub estado: EstadoIssue,
    /// Data de fechamento (`AAAA-MM-DD`), quando encerrada e o rastreador informa.
    pub encerrada_em: Option<String>,
}

/// Chave da issue como a conferência a usa (`#12` no GitHub e no GitLab,
/// `PROJ-12` no Jira), ou `None` se a chave não é deste rastreador.
pub fn chave_normalizada(tracker: &TrackerConfig, chave: &str) -> Option<String> {
    match tracker {
        TrackerConfig::Github(_) | TrackerConfig::Gitlab(_) => {
            numero(chave).ok().map(|n| format!("#{n}"))
        }
        TrackerConfig::Jira(j) => {
            let c = chave.trim().to_ascii_uppercase();
            j.number_from_key(&c).is_some().then_some(c)
        }
    }
}

/// O endereço da demanda (`origem`) é deste rastreador? Sem endereço (ou com um
/// que não é URL), vale a chave.
pub fn origem_do_rastreador(tracker: &TrackerConfig, origem: Option<&str>) -> bool {
    let Some(o) = origem.map(str::trim).filter(|o| o.contains("://")) else {
        return true;
    };
    let o = o.to_ascii_lowercase();
    let caminho = |c: &str| format!("/{}/", c.trim().trim_matches('/').to_ascii_lowercase());
    match tracker {
        TrackerConfig::Github(g) => o.contains(&caminho(&g.repo)),
        TrackerConfig::Gitlab(g) => o.contains(&caminho(&g.path)),
        TrackerConfig::Jira(j) => o.contains(&j.host().to_ascii_lowercase()),
    }
}

/// `AAAA-MM-DD` do começo de uma data ISO 8601.
fn data(iso: &str) -> Option<String> {
    let d = iso.trim().get(..10)?;
    let ok = d.bytes().enumerate().all(|(i, b)| match i {
        4 | 7 => b == b'-',
        _ => b.is_ascii_digit(),
    });
    ok.then(|| d.to_string())
}

fn lido(encerrada: bool, em_andamento: bool, fechada_em: &str) -> EstadoLido {
    if encerrada {
        EstadoLido {
            estado: EstadoIssue::Encerrada,
            encerrada_em: data(fechada_em),
        }
    } else {
        EstadoLido {
            estado: if em_andamento {
                EstadoIssue::EmAndamento
            } else {
                EstadoIssue::Aberta
            },
            encerrada_em: None,
        }
    }
}

/// Lê o estado das issues. As chaves que não são deste rastreador e as que o
/// rastreador não achou ficam fora da resposta, que vem indexada pela
/// [`chave_normalizada`].
pub fn conferir_estados(
    tracker: &TrackerConfig,
    chaves: &[String],
    f: &Ferramentas,
) -> Result<HashMap<String, EstadoLido>, DemandaErro> {
    let mut normalizadas: Vec<String> = chaves
        .iter()
        .filter_map(|c| chave_normalizada(tracker, c))
        .collect();
    normalizadas.sort();
    normalizadas.dedup();
    if normalizadas.is_empty() {
        return Ok(HashMap::new());
    }
    match tracker {
        TrackerConfig::Github(g) => estados_github(g, &normalizadas, f),
        TrackerConfig::Gitlab(g) => estados_gitlab(g, &normalizadas, f),
        TrackerConfig::Jira(j) => estados_jira(j, &normalizadas, f),
    }
}

fn numeros(chaves: &[String]) -> Vec<u64> {
    chaves.iter().filter_map(|c| numero(c).ok()).collect()
}

// ---------------------------------------------------------------------------
// GitHub

fn consulta_github(numeros: &[u64]) -> String {
    let campos: String = numeros
        .iter()
        .map(|n| {
            format!(
                "i{n}:issueOrPullRequest(number:{n}){{... on Issue{{state closedAt}} \
                 ... on PullRequest{{state closedAt}}}} "
            )
        })
        .collect();
    format!(
        "query($owner:String!,$name:String!){{repository(owner:$owner,name:$name){{{campos}}}}}"
    )
}

fn estados_github(
    g: &GithubConfig,
    chaves: &[String],
    f: &Ferramentas,
) -> Result<HashMap<String, EstadoLido>, DemandaErro> {
    let (dono, nome) = g
        .repo
        .trim()
        .split_once('/')
        .ok_or_else(|| DemandaErro::Referencia(g.repo.clone()))?;
    let token = token_github(g, f);
    let vazio = Secret::new("");
    let mut saida = HashMap::new();
    for lote in numeros(chaves).chunks(LOTE_GITHUB) {
        let mut c = Command::new(&f.gh);
        if let Some(t) = &token {
            c.env("GH_TOKEN", t.expose());
        }
        c.args([
            "api",
            "graphql",
            "-f",
            &format!("query={}", consulta_github(lote)),
            "-f",
            &format!("owner={dono}"),
            "-f",
            &format!("name={nome}"),
        ]);
        let v = rodar_graphql(c, token.as_ref().unwrap_or(&vazio))?;
        let repo = &v["data"]["repository"];
        if repo.is_null() {
            return Err(DemandaErro::Falhou {
                ferramenta: "gh",
                detalhe: format!("repositório {} não encontrado", g.repo.trim()),
            });
        }
        for n in lote {
            let i = &repo[format!("i{n}")];
            let estado = texto(i, &["state"]);
            if estado.is_empty() {
                continue;
            }
            saida.insert(
                format!("#{n}"),
                lido(
                    !estado.eq_ignore_ascii_case("open"),
                    false,
                    texto(i, &["closedAt"]),
                ),
            );
        }
    }
    Ok(saida)
}

/// Como [`rodar_json`], mas aceita a saída com erro do `gh api graphql` quando
/// ela traz `data` (resposta parcial: um número que não existe).
fn rodar_graphql(cmd: Command, token: &Secret) -> Result<Value, DemandaErro> {
    let ferramenta = "gh";
    let o = process::run(cmd, crate::demanda::PRAZO_CLI).map_err(|e| match e {
        RunError::NotFound => DemandaErro::SemFerramenta { ferramenta },
        e => DemandaErro::Falhou {
            ferramenta,
            detalhe: cite(&e.to_string(), Some(token)),
        },
    })?;
    match serde_json::from_slice::<Value>(&o.stdout) {
        Ok(v) if v.get("data").is_some_and(|d| !d.is_null()) => Ok(v),
        Ok(_) | Err(_) if !o.status.success() => Err(DemandaErro::Falhou {
            ferramenta,
            detalhe: cite(&String::from_utf8_lossy(&o.stderr), Some(token)),
        }),
        Ok(v) => Ok(v),
        Err(e) => Err(DemandaErro::Resposta {
            ferramenta,
            detalhe: e.to_string(),
        }),
    }
}

// ---------------------------------------------------------------------------
// GitLab

fn estados_gitlab(
    g: &GitlabConfig,
    chaves: &[String],
    f: &Ferramentas,
) -> Result<HashMap<String, EstadoLido>, DemandaErro> {
    let token = token_gitlab(g, f)?;
    let id = g.path.trim_matches('/').replace('/', "%2F");
    let mut saida = HashMap::new();
    for lote in numeros(chaves).chunks(LOTE_GITLAB) {
        let iids: String = lote.iter().map(|n| format!("&iids[]={n}")).collect();
        let mut c = Command::new(&f.glab);
        c.args([
            "api",
            &format!(
                "projects/{id}/issues?scope=all&per_page={}{iids}",
                LOTE_GITLAB
            ),
        ])
        .env("GITLAB_HOST", g.host.trim())
        .env("GITLAB_TOKEN", token.expose());
        let v = rodar_json("glab", c, &token)?;
        for i in v.as_array().map(Vec::as_slice).unwrap_or_default() {
            let Some(n) = i["iid"].as_u64() else { continue };
            saida.insert(
                format!("#{n}"),
                lido(
                    texto(i, &["state"]) == "closed",
                    false,
                    texto(i, &["closed_at"]),
                ),
            );
        }
    }
    Ok(saida)
}

// ---------------------------------------------------------------------------
// Jira

fn estado_jira(i: &Value) -> EstadoLido {
    let categoria = texto(i, &["fields", "status", "statusCategory", "key"]);
    let encerrada = match categoria {
        "" => !texto(i, &["fields", "resolution", "name"]).is_empty(),
        c => c == "done",
    };
    let fechada_em = [
        texto(i, &["fields", "resolutiondate"]),
        texto(i, &["fields", "statuscategorychangedate"]),
    ]
    .into_iter()
    .find(|d| !d.is_empty())
    .unwrap_or("");
    lido(encerrada, categoria == "indeterminate", fechada_em)
}

fn comando_jira(j: &JiraConfig, f: &Ferramentas, token: &Secret, args: &[&str]) -> Command {
    let mut c = Command::new(&f.jira);
    c.args(args).env("JIRA_API_TOKEN", token.expose());
    if !j.config_file.trim().is_empty() {
        c.env("JIRA_CONFIG_FILE", j.config_file.trim());
    }
    c
}

fn estados_jira(
    j: &JiraConfig,
    chaves: &[String],
    f: &Ferramentas,
) -> Result<HashMap<String, EstadoLido>, DemandaErro> {
    let token = token_jira(j, f)?;
    let mut saida = HashMap::new();
    for lote in chaves.chunks(LOTE_JIRA) {
        let jql = format!("key in ({})", lote.join(", "));
        let c = comando_jira(
            j,
            f,
            &token,
            &[
                "issue",
                "list",
                "--jql",
                &jql,
                "--raw",
                "--paginate",
                &format!("0:{}", lote.len()),
            ],
        );
        let issues = match rodar_json("jira", c, &token) {
            Ok(Value::Array(a)) => a,
            Ok(Value::Object(mut o)) => match o.remove("issues") {
                Some(Value::Array(a)) => a,
                _ => Vec::new(),
            },
            Ok(_) => Vec::new(),
            Err(e @ (DemandaErro::SemFerramenta { .. } | DemandaErro::Credencial(_))) => {
                return Err(e);
            }
            // Uma chave inexistente derruba a JQL: uma a uma.
            Err(_) => {
                let mut uma_a_uma = Vec::new();
                let mut ultimo_erro = None;
                for chave in lote {
                    let c = comando_jira(j, f, &token, &["issue", "view", chave, "--raw"]);
                    match rodar_json("jira", c, &token) {
                        Ok(v) => uma_a_uma.push(v),
                        Err(e) => ultimo_erro = Some(e),
                    }
                }
                // Nenhuma veio: o problema é o acesso, não uma chave.
                if uma_a_uma.is_empty()
                    && let Some(e) = ultimo_erro
                {
                    return Err(e);
                }
                uma_a_uma
            }
        };
        for i in &issues {
            let chave = texto(i, &["key"]).to_ascii_uppercase();
            if lote.contains(&chave) {
                saida.insert(chave, estado_jira(i));
            }
        }
    }
    Ok(saida)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::demanda::falsas::script;

    fn jira(url: &str) -> TrackerConfig {
        TrackerConfig::Jira(JiraConfig {
            url: url.into(),
            project_key: "PROJ".into(),
            credential_cmd: "echo tok-secreto".into(),
            ..Default::default()
        })
    }

    #[test]
    fn chave_e_origem_do_rastreador() {
        let gh = TrackerConfig::Github(GithubConfig {
            repo: "o/r".into(),
            ..Default::default()
        });
        assert_eq!(chave_normalizada(&gh, "#12").as_deref(), Some("#12"));
        assert_eq!(chave_normalizada(&gh, "12").as_deref(), Some("#12"));
        assert_eq!(chave_normalizada(&gh, "PROJ-1"), None);
        let j = jira("https://jira.example.com");
        assert_eq!(chave_normalizada(&j, "proj-3").as_deref(), Some("PROJ-3"));
        assert_eq!(chave_normalizada(&j, "#3"), None);
        assert!(origem_do_rastreador(&gh, None));
        assert!(origem_do_rastreador(&gh, Some("FAT-1")));
        assert!(origem_do_rastreador(
            &gh,
            Some("https://github.com/O/R/issues/12")
        ));
        assert!(!origem_do_rastreador(
            &gh,
            Some("https://github.com/o/outro/issues/12")
        ));
        assert!(origem_do_rastreador(
            &j,
            Some("https://jira.example.com/browse/PROJ-3")
        ));
    }

    #[test]
    fn github_em_lote_com_numero_inexistente() {
        let d = tempfile::tempdir().unwrap();
        // Resposta parcial: o #9 não existe; o gh sai com erro, mas com os dados.
        let gh = script(
            d.path(),
            "gh",
            r#"[ "$1 $2" = "api graphql" ] || exit 2
case "$4" in *i5:issueOrPullRequest*i7:issueOrPullRequest*i9:*) ;; *) exit 3;; esac
[ "$6 $8" = "owner=o name=r" ] || exit 4
echo '{"data":{"repository":{"i5":{"state":"CLOSED","closedAt":"2026-11-20T12:00:00Z"},"i7":{"state":"OPEN","closedAt":null},"i9":null}},"errors":[{"message":"Could not resolve"}]}'
echo "gh: Could not resolve" >&2
exit 1"#,
        );
        let f = Ferramentas {
            gh,
            ..Default::default()
        };
        let t = TrackerConfig::Github(GithubConfig {
            repo: "o/r".into(),
            ..Default::default()
        });
        let r = conferir_estados(
            &t,
            &["#5".into(), "7".into(), "#9".into(), "PROJ-1".into()],
            &f,
        )
        .unwrap();
        assert_eq!(r.len(), 2);
        assert_eq!(
            r["#5"],
            EstadoLido {
                estado: EstadoIssue::Encerrada,
                encerrada_em: Some("2026-11-20".into())
            }
        );
        assert_eq!(r["#7"].estado, EstadoIssue::Aberta);
        assert_eq!(r["#7"].encerrada_em, None);
    }

    #[test]
    fn github_sem_rede_e_erro() {
        let d = tempfile::tempdir().unwrap();
        let gh = script(d.path(), "gh", r#"echo "error connecting" >&2; exit 1"#);
        let f = Ferramentas {
            gh,
            ..Default::default()
        };
        let t = TrackerConfig::Github(GithubConfig {
            repo: "o/r".into(),
            ..Default::default()
        });
        let e = conferir_estados(&t, &["#1".into()], &f).unwrap_err();
        assert!(e.to_string().contains("error connecting"), "{e}");
    }

    #[test]
    fn gitlab_em_lote_com_token_no_ambiente() {
        let d = tempfile::tempdir().unwrap();
        let glab = script(
            d.path(),
            "glab",
            r#"[ "$GITLAB_TOKEN" = "tok" ] && [ "$GITLAB_HOST" = "gitlab.example.com" ] || exit 1
[ "$2" = "projects/g%2Fapp/issues?scope=all&per_page=100&iids[]=3&iids[]=4" ] || { echo "$2" >&2; exit 2; }
echo '[{"iid":3,"state":"closed","closed_at":"2026-10-01T08:00:00.000Z"},{"iid":4,"state":"opened","closed_at":null}]'"#,
        );
        let f = Ferramentas {
            glab,
            ..Default::default()
        };
        let t = TrackerConfig::Gitlab(GitlabConfig {
            host: "gitlab.example.com".into(),
            path: "g/app".into(),
            credential_cmd: "echo tok".into(),
            ..Default::default()
        });
        let r = conferir_estados(&t, &["#4".into(), "#3".into()], &f).unwrap();
        assert_eq!(r["#3"].estado, EstadoIssue::Encerrada);
        assert_eq!(r["#3"].encerrada_em.as_deref(), Some("2026-10-01"));
        assert_eq!(r["#4"].estado, EstadoIssue::Aberta);
    }

    #[test]
    fn jira_pela_categoria_e_uma_a_uma_quando_a_jql_falha() {
        let d = tempfile::tempdir().unwrap();
        // A JQL com PROJ-99 (inexistente) falha; a consulta uma a uma acha as outras.
        let jira_bin = script(
            d.path(),
            "jira",
            r#"[ "$JIRA_API_TOKEN" = "tok-secreto" ] || exit 1
if [ "$2" = "list" ]; then
  case "$4" in *PROJ-99*) echo "issue does not exist" >&2; exit 1;; esac
  echo '[{"key":"PROJ-1","fields":{"status":{"name":"Homologado","statusCategory":{"key":"done"}},"resolutiondate":"2026-11-02T10:00:00.000-0300"}},{"key":"PROJ-2","fields":{"status":{"name":"Fazendo","statusCategory":{"key":"indeterminate"}}}}]'
  exit 0
fi
case "$3" in
  PROJ-1) echo '{"key":"PROJ-1","fields":{"status":{"name":"Concluído","statusCategory":{"key":"done"}},"resolution":null,"statuscategorychangedate":"2026-11-03T10:00:00.000-0300"}}';;
  PROJ-2) echo '{"key":"PROJ-2","fields":{"status":{"name":"Done","statusCategory":{"key":"new"}}}}';;
  *) echo "não existe" >&2; exit 1;;
esac"#,
        );
        let f = Ferramentas {
            jira: jira_bin,
            ..Default::default()
        };
        let t = jira("https://jira.example.com");
        let r = conferir_estados(&t, &["proj-1".into(), "PROJ-2".into()], &f).unwrap();
        assert_eq!(r["PROJ-1"].estado, EstadoIssue::Encerrada);
        assert_eq!(r["PROJ-1"].encerrada_em.as_deref(), Some("2026-11-02"));
        assert_eq!(r["PROJ-2"].estado, EstadoIssue::EmAndamento);

        let r = conferir_estados(
            &t,
            &["PROJ-1".into(), "PROJ-2".into(), "PROJ-99".into()],
            &f,
        )
        .unwrap();
        assert_eq!(r.len(), 2);
        // O nome "Done" não conta: a categoria é `new`.
        assert_eq!(r["PROJ-2"].estado, EstadoIssue::Aberta);
        assert_eq!(r["PROJ-1"].encerrada_em.as_deref(), Some("2026-11-03"));
    }

    #[test]
    fn jira_inacessivel_nao_vaza_o_token() {
        let d = tempfile::tempdir().unwrap();
        let jira_bin = script(
            d.path(),
            "jira",
            r#"echo "401 com $JIRA_API_TOKEN" >&2; exit 1"#,
        );
        let f = Ferramentas {
            jira: jira_bin,
            ..Default::default()
        };
        let e = conferir_estados(&jira("https://jira.example.com"), &["PROJ-1".into()], &f)
            .unwrap_err()
            .to_string();
        assert!(e.contains("401") && !e.contains("tok-secreto"), "{e}");
    }
}
