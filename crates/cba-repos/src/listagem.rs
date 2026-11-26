//! Leitura em lote das issues do rastreador por período (#270).
//!
//! Mesmas CLIs e credenciais de [`crate::demanda`]: `gh issue list` e
//! `jira issue list --raw`. Só leitura; o token chega ao filho só no ambiente.

use crate::demanda::{
    DemandaErro, Ferramentas, rodar_json, texto, token_github, token_gitlab, token_jira,
    wiki_para_markdown,
};
use crate::{GithubConfig, GitlabConfig, JiraConfig, Secret, TrackerConfig};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

/// Período padrão, em meses.
pub const MESES_PADRAO: u32 = 6;
/// Teto padrão de issues por leitura (GitHub; o Jira usa o `max_issues` do cadastro).
pub const TETO_PADRAO: usize = 200;
/// Máximo de itens por página da API do GitLab.
const PAGINA_GITLAB: usize = 100;
/// Mais que isso por chamada o `jira issue list --paginate` recusa.
const PAGINA_JIRA: usize = 100;

/// Desde quando as issues foram criadas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Periodo {
    /// Últimos N meses, contados a partir de hoje.
    Meses(u32),
    /// Data explícita `AAAA-MM-DD`.
    Desde(String),
}

impl Default for Periodo {
    fn default() -> Self {
        Periodo::Meses(MESES_PADRAO)
    }
}

impl Periodo {
    /// Data `AAAA-MM-DD` de corte, dado o dia de hoje.
    pub fn data_a_partir_de(&self, hoje: (i32, u32, u32)) -> Result<String, DemandaErro> {
        match self {
            Periodo::Desde(d) => {
                let d = d.trim();
                let ok = d.len() == 10
                    && d.bytes().enumerate().all(|(i, b)| match i {
                        4 | 7 => b == b'-',
                        _ => b.is_ascii_digit(),
                    })
                    && matches!(d[5..7].parse::<u32>(), Ok(1..=12))
                    && matches!(d[8..10].parse::<u32>(), Ok(1..=31));
                if ok {
                    Ok(d.to_string())
                } else {
                    Err(DemandaErro::Referencia(format!("data inválida: {d:?}")))
                }
            }
            Periodo::Meses(n) => {
                let (a, m, d) = hoje;
                let total = a * 12 + (m as i32 - 1) - (*n).min(1200) as i32;
                let (a, m) = (total.div_euclid(12), total.rem_euclid(12) as u32 + 1);
                Ok(format!("{a:04}-{m:02}-{:02}", d.min(dias_do_mes(a, m))))
            }
        }
    }
}

fn dias_do_mes(a: i32, m: u32) -> u32 {
    match m {
        2 if (a % 4 == 0 && a % 100 != 0) || a % 400 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Hoje (UTC) como (ano, mês, dia).
pub fn hoje() -> (i32, u32, u32) {
    let dias = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| (d.as_secs() / 86_400) as i64);
    // Civil-from-days (Howard Hinnant).
    let z = dias + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let a = (yoe + era * 400) as i32 + i32::from(m <= 2);
    (a, m, d)
}

/// Origem da issue na base de demandas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrigemIssue {
    Github,
    Gitlab,
    Jira,
}

impl OrigemIssue {
    pub fn nome(self) -> &'static str {
        match self {
            OrigemIssue::Github => "github",
            OrigemIssue::Gitlab => "gitlab",
            OrigemIssue::Jira => "jira",
        }
    }
}

/// Issue lida em lote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueListada {
    pub origem: OrigemIssue,
    /// `#123` ou `PROJ-123`.
    pub chave: String,
    pub titulo: String,
    pub descricao: String,
    pub url: String,
    /// Data de criação como veio (ISO 8601).
    pub criada_em: String,
    /// `aberta` ou `fechada`.
    pub estado: String,
    pub rotulos: Vec<String>,
    /// Arquivos alterados pelos PRs que fecham a issue (só GitHub); ajudam a
    /// sugerir a funcionalidade.
    #[serde(default)]
    pub arquivos: Vec<String>,
}

/// Resultado de uma leitura: issues (até o teto) e se havia mais.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Listagem {
    pub issues: Vec<IssueListada>,
    /// O período devolveu mais issues que o teto; as excedentes não vieram.
    pub excedeu_teto: bool,
    /// Data de corte usada (`AAAA-MM-DD`).
    pub desde: String,
}

fn lista(v: Value, ferramenta: &'static str) -> Result<Vec<Value>, DemandaErro> {
    match v {
        Value::Array(a) => Ok(a),
        // `jira issue list --raw` pode devolver `{"issues": [...]}`.
        Value::Object(mut o) => match o.remove("issues") {
            Some(Value::Array(a)) => Ok(a),
            _ => Err(DemandaErro::Resposta {
                ferramenta,
                detalhe: "esperava uma lista de issues".into(),
            }),
        },
        _ => Err(DemandaErro::Resposta {
            ferramenta,
            detalhe: "esperava uma lista de issues".into(),
        }),
    }
}

/// Teto de PRs consultados por listagem para ler os arquivos alterados.
const TETO_PRS: usize = 60;

fn listar_github_json(
    g: &GithubConfig,
    desde: &str,
    teto: usize,
    campos: &str,
    f: &Ferramentas,
    token: Option<&Secret>,
) -> Result<Value, DemandaErro> {
    let mut c = Command::new(&f.gh);
    if let Some(t) = token {
        c.env("GH_TOKEN", t.expose());
    }
    c.args([
        "issue",
        "list",
        "-R",
        g.repo.trim(),
        "--state",
        "all",
        "--search",
        &format!("created:>={desde}"),
        "--limit",
        &(teto + 1).to_string(),
        "--json",
        campos,
    ]);
    rodar_json("gh", c, token.unwrap_or(&Secret::new("")))
}

/// Arquivos alterados por um PR (`gh pr view --json files`); vazio se falhar.
fn arquivos_do_pr(
    g: &GithubConfig,
    numero: u64,
    f: &Ferramentas,
    token: Option<&Secret>,
) -> Vec<String> {
    let mut c = Command::new(&f.gh);
    if let Some(t) = token {
        c.env("GH_TOKEN", t.expose());
    }
    c.args([
        "pr",
        "view",
        &numero.to_string(),
        "-R",
        g.repo.trim(),
        "--json",
        "files",
    ]);
    let Ok(v) = rodar_json("gh", c, token.unwrap_or(&Secret::new(""))) else {
        return Vec::new();
    };
    v["files"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|x| texto(x, &["path"]).to_string())
                .filter(|p| !p.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

pub fn listar_github(
    g: &GithubConfig,
    periodo: &Periodo,
    teto: usize,
    f: &Ferramentas,
) -> Result<Listagem, DemandaErro> {
    let desde = periodo.data_a_partir_de(hoje())?;
    let teto = teto.max(1);
    let token = token_github(g, f);
    let mut c = Command::new(&f.gh);
    if let Some(t) = &token {
        c.env("GH_TOKEN", t.expose());
    }
    let campos = "number,title,body,url,createdAt,state,labels";
    // `closedByPullRequestsReferences` não existe em todas as versões do `gh`:
    // sem ele a listagem continua, só sem os arquivos dos PRs.
    let com_prs = format!("{campos},closedByPullRequestsReferences");
    let (v, tem_prs) = match listar_github_json(g, &desde, teto, &com_prs, f, token.as_ref()) {
        Ok(v) => (v, true),
        Err(_) => (
            listar_github_json(g, &desde, teto, campos, f, token.as_ref())?,
            false,
        ),
    };
    let brutas = lista(v, "gh")?;
    let mut issues: Vec<IssueListada> = brutas
        .iter()
        .map(|i| IssueListada {
            origem: OrigemIssue::Github,
            chave: format!("#{}", i["number"].as_u64().unwrap_or(0)),
            titulo: texto(i, &["title"]).to_string(),
            descricao: texto(i, &["body"]).to_string(),
            url: texto(i, &["url"]).to_string(),
            criada_em: texto(i, &["createdAt"]).to_string(),
            estado: if texto(i, &["state"]).eq_ignore_ascii_case("open") {
                "aberta"
            } else {
                "fechada"
            }
            .into(),
            rotulos: i["labels"]
                .as_array()
                .map(|l| {
                    l.iter()
                        .map(|r| texto(r, &["name"]).to_string())
                        .filter(|n| !n.is_empty())
                        .collect()
                })
                .unwrap_or_default(),
            arquivos: Vec::new(),
        })
        .collect();
    if tem_prs {
        // Um `gh pr view` por PR, no máximo `TETO_PRS`; falha de um PR não derruba a listagem.
        let mut consultados = 0usize;
        for (issue, bruta) in issues.iter_mut().zip(&brutas).take(teto) {
            let prs: Vec<u64> = bruta["closedByPullRequestsReferences"]
                .as_array()
                .map(|a| a.iter().filter_map(|p| p["number"].as_u64()).collect())
                .unwrap_or_default();
            for n in prs {
                if consultados >= TETO_PRS {
                    break;
                }
                consultados += 1;
                issue
                    .arquivos
                    .extend(arquivos_do_pr(g, n, f, token.as_ref()));
            }
            issue.arquivos.sort();
            issue.arquivos.dedup();
        }
    }
    let excedeu_teto = issues.len() > teto;
    issues.truncate(teto);
    Ok(Listagem {
        issues,
        excedeu_teto,
        desde,
    })
}

pub fn listar_gitlab(
    g: &GitlabConfig,
    periodo: &Periodo,
    teto: usize,
    f: &Ferramentas,
) -> Result<Listagem, DemandaErro> {
    let desde = periodo.data_a_partir_de(hoje())?;
    let teto = teto.max(1);
    let token = token_gitlab(g, f)?;
    let id = g.path.trim_matches('/').replace('/', "%2F");
    // Lê páginas de até 100 até ter teto + 1 (o a mais mostra que passou do teto).
    let querido = teto + 1;
    let mut brutas: Vec<Value> = Vec::new();
    let mut pagina_n = 1usize;
    while brutas.len() < querido {
        let por_pagina = PAGINA_GITLAB.min(querido - brutas.len());
        let mut c = Command::new(&f.glab);
        c.args([
            "api",
            &format!(
                "projects/{id}/issues?state=all&order_by=created_at&sort=desc\
                 &created_after={desde}T00:00:00Z&per_page={por_pagina}&page={pagina_n}"
            ),
        ])
        .env("GITLAB_HOST", g.host.trim())
        .env("GITLAB_TOKEN", token.expose());
        let pagina = lista(rodar_json("glab", c, &token)?, "glab")?;
        let n = pagina.len();
        brutas.extend(pagina);
        if n < por_pagina {
            break;
        }
        pagina_n += 1;
    }
    let mut issues: Vec<IssueListada> = brutas
        .iter()
        .map(|i| IssueListada {
            origem: OrigemIssue::Gitlab,
            chave: format!("#{}", i["iid"].as_u64().unwrap_or(0)),
            titulo: texto(i, &["title"]).to_string(),
            descricao: texto(i, &["description"]).to_string(),
            url: texto(i, &["web_url"]).to_string(),
            criada_em: texto(i, &["created_at"]).to_string(),
            estado: if texto(i, &["state"]).eq_ignore_ascii_case("opened") {
                "aberta"
            } else {
                "fechada"
            }
            .into(),
            rotulos: i["labels"]
                .as_array()
                .map(|l| {
                    l.iter()
                        .filter_map(|r| r.as_str().map(str::to_string))
                        .filter(|n| !n.is_empty())
                        .collect()
                })
                .unwrap_or_default(),
            arquivos: Vec::new(),
        })
        .collect();
    let excedeu_teto = issues.len() > teto;
    issues.truncate(teto);
    Ok(Listagem {
        issues,
        excedeu_teto,
        desde,
    })
}

/// JQL da listagem: projeto + criação + o `jql` do cadastro (sem `ORDER BY`: o
/// `jira issue list` acrescenta o próprio, por `created` decrescente, e um segundo
/// faria a consulta falhar).
fn jql_da_listagem(j: &JiraConfig, desde: &str) -> String {
    let mut jql = format!(
        "project = {} AND created >= \"{desde}\"",
        j.project_key.trim()
    );
    let extra = if j.active_sprint_only {
        String::new()
    } else {
        let e = j.jql.trim();
        let corte = e.to_ascii_lowercase().find("order by").unwrap_or(e.len());
        e[..corte].trim().to_string()
    };
    if !extra.is_empty() {
        jql.push_str(&format!(" AND ({extra})"));
    }
    jql
}

pub fn listar_jira(
    j: &JiraConfig,
    periodo: &Periodo,
    teto: Option<usize>,
    f: &Ferramentas,
) -> Result<Listagem, DemandaErro> {
    let desde = periodo.data_a_partir_de(hoje())?;
    let teto = teto
        .unwrap_or(j.max_issues.clamp(1, crate::tracker::MAX_ISSUES_CEILING) as usize)
        .max(1);
    let token = token_jira(j, f)?;
    let jql = jql_da_listagem(j, &desde);
    // O jira-cli aceita no máximo 100 por chamada (`--paginate <de>:<limite>`):
    // lê em blocos até ter teto + 1 (o a mais mostra que o período passou do teto).
    let querido = teto + 1;
    let mut brutas: Vec<Value> = Vec::new();
    while brutas.len() < querido {
        let limite = (querido - brutas.len()).min(PAGINA_JIRA);
        let mut c = Command::new(&f.jira);
        c.args([
            "issue",
            "list",
            "--jql",
            &jql,
            "--raw",
            "--paginate",
            &format!("{}:{limite}", brutas.len()),
        ])
        .env("JIRA_API_TOKEN", token.expose());
        if !j.config_file.trim().is_empty() {
            c.env("JIRA_CONFIG_FILE", j.config_file.trim());
        }
        let pagina = lista(rodar_json("jira", c, &token)?, "jira")?;
        let n = pagina.len();
        brutas.extend(pagina);
        if n < limite {
            break;
        }
    }
    let v = Value::Array(brutas);
    let mut issues: Vec<IssueListada> = lista(v, "jira")?
        .iter()
        .map(|i| {
            let chave = texto(i, &["key"]).to_string();
            let fechada = !texto(i, &["fields", "resolution", "name"]).is_empty()
                || texto(i, &["fields", "status", "statusCategory", "key"]) == "done";
            IssueListada {
                origem: OrigemIssue::Jira,
                titulo: texto(i, &["fields", "summary"]).to_string(),
                descricao: wiki_para_markdown(texto(i, &["fields", "description"])),
                url: j.issue_url(&chave),
                criada_em: texto(i, &["fields", "created"]).to_string(),
                estado: if fechada { "fechada" } else { "aberta" }.into(),
                rotulos: i["fields"]["labels"]
                    .as_array()
                    .map(|l| {
                        l.iter()
                            .filter_map(|r| r.as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default(),
                arquivos: Vec::new(),
                chave,
            }
        })
        .collect();
    let excedeu_teto = issues.len() > teto;
    issues.truncate(teto);
    Ok(Listagem {
        issues,
        excedeu_teto,
        desde,
    })
}

/// Lê as issues do rastreador do projeto (GitHub, GitLab ou Jira).
pub fn listar_issues(
    tracker: &TrackerConfig,
    periodo: &Periodo,
    teto: Option<usize>,
    f: &Ferramentas,
) -> Result<Listagem, DemandaErro> {
    match tracker {
        TrackerConfig::Github(g) => listar_github(g, periodo, teto.unwrap_or(TETO_PADRAO), f),
        TrackerConfig::Jira(j) => listar_jira(j, periodo, teto, f),
        TrackerConfig::Gitlab(g) => listar_gitlab(g, periodo, teto.unwrap_or(TETO_PADRAO), f),
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::demanda::falsas::script;

    #[test]
    fn periodo_em_meses_e_data_explicita() {
        let p = Periodo::default();
        assert_eq!(p.data_a_partir_de((2026, 11, 19)).unwrap(), "2026-05-19");
        assert_eq!(
            Periodo::Meses(12).data_a_partir_de((2026, 11, 19)).unwrap(),
            "2025-11-19"
        );
        assert_eq!(
            Periodo::Meses(9).data_a_partir_de((2026, 5, 31)).unwrap(),
            "2025-08-31"
        );
        assert_eq!(
            Periodo::Meses(1).data_a_partir_de((2026, 3, 31)).unwrap(),
            "2026-02-28"
        );
        assert_eq!(
            Periodo::Desde("2025-01-02".into())
                .data_a_partir_de((2026, 1, 1))
                .unwrap(),
            "2025-01-02"
        );
        assert!(
            Periodo::Desde("ontem".into())
                .data_a_partir_de(hoje())
                .is_err()
        );
        let (a, m, d) = hoje();
        assert!(a >= 2025 && (1..=12).contains(&m) && (1..=31).contains(&d));
    }

    #[test]
    fn github_filtra_por_data_com_teto_e_estados() {
        let d = tempfile::tempdir().unwrap();
        // Só responde se o filtro de data e o limite (teto + 1) forem pedidos.
        let gh = script(
            d.path(),
            "gh",
            r#"case "$*" in *"--state all"*) ;; *) exit 2;; esac
case "$*" in *"created:>=20"*) ;; *) exit 3;; esac
case "$*" in *"--limit 3 "*) ;; *) exit 4;; esac
echo '[{"number":3,"title":"C","body":"c","url":"u3","createdAt":"2026-10-01T00:00:00Z","state":"OPEN","labels":[{"name":"bug"}]},{"number":2,"title":"B","body":"b","url":"u2","createdAt":"2026-09-01T00:00:00Z","state":"CLOSED","labels":[]},{"number":1,"title":"A","body":"a","url":"u1","createdAt":"2026-08-01T00:00:00Z","state":"CLOSED","labels":[]}]'"#,
        );
        let f = Ferramentas {
            gh,
            ..Default::default()
        };
        let g = GithubConfig {
            repo: "o/r".into(),
            ..Default::default()
        };
        let l = listar_github(&g, &Periodo::default(), 2, &f).unwrap();
        assert!(l.excedeu_teto);
        assert_eq!(l.issues.len(), 2);
        assert_eq!(l.issues[0].chave, "#3");
        assert_eq!(l.issues[0].estado, "aberta");
        assert_eq!(l.issues[0].rotulos, vec!["bug"]);
        assert_eq!(l.issues[1].estado, "fechada");
        let todas = listar_github(&g, &Periodo::default(), 3, &f);
        // Com teto 3 o limite pedido muda para 4: a CLI falsa recusa.
        assert!(todas.is_err());
    }

    #[test]
    fn github_traz_os_arquivos_dos_prs_que_fecham_a_issue() {
        let d = tempfile::tempdir().unwrap();
        let gh = script(
            d.path(),
            "gh",
            r#"if [ "$1" = "pr" ]; then
  case "$3" in
    7) echo '{"files":[{"path":"db/parcelamento.sql"},{"path":"src/a.java"}]}';;
    8) echo '{"files":[{"path":"src/a.java"}]}';;
    *) exit 9;;
  esac
  exit 0
fi
echo '[{"number":5,"title":"A","body":"a","url":"u5","createdAt":"2026-10-01T00:00:00Z","state":"CLOSED","labels":[],"closedByPullRequestsReferences":[{"number":7},{"number":8}]},{"number":4,"title":"B","body":"b","url":"u4","createdAt":"2026-09-01T00:00:00Z","state":"OPEN","labels":[],"closedByPullRequestsReferences":[]}]'"#,
        );
        let f = Ferramentas {
            gh,
            ..Default::default()
        };
        let g = GithubConfig {
            repo: "o/r".into(),
            ..Default::default()
        };
        let l = listar_github(&g, &Periodo::default(), 10, &f).unwrap();
        assert_eq!(
            l.issues[0].arquivos,
            vec!["db/parcelamento.sql", "src/a.java"]
        );
        assert!(l.issues[1].arquivos.is_empty());
    }

    #[test]
    fn github_sem_o_campo_de_prs_lista_sem_arquivos() {
        let d = tempfile::tempdir().unwrap();
        // `gh` antigo: recusa o campo novo, aceita os demais.
        let gh = script(
            d.path(),
            "gh",
            r#"case "$*" in *closedByPullRequestsReferences*) echo "Unknown JSON field" >&2; exit 1;; esac
echo '[{"number":5,"title":"A","body":"a","url":"u5","createdAt":"2026-10-01T00:00:00Z","state":"OPEN","labels":[]}]'"#,
        );
        let f = Ferramentas {
            gh,
            ..Default::default()
        };
        let g = GithubConfig {
            repo: "o/r".into(),
            ..Default::default()
        };
        let l = listar_github(&g, &Periodo::default(), 10, &f).unwrap();
        assert_eq!(l.issues.len(), 1);
        assert!(l.issues[0].arquivos.is_empty());
    }

    #[test]
    fn jira_monta_jql_com_periodo_e_cadastro() {
        let d = tempfile::tempdir().unwrap();
        let jira = script(
            d.path(),
            "jira",
            r#"[ "$JIRA_API_TOKEN" = "tok" ] || exit 1
[ "$1 $2 $3" = "issue list --jql" ] || exit 2
echo "$4" | grep -q '^project = PROJ AND created >= "2025-11-' || exit 3
echo "$4" | grep -q 'AND (labels = fatura)' || exit 4
echo "$4" | grep -qi 'order by' && exit 5
[ "$5 $6 $7" = "--raw --paginate 0:11" ] || exit 6
echo '[{"key":"PROJ-9","fields":{"summary":"S","description":"h3. T","created":"2026-01-01T00:00:00.000+0000","labels":["x"],"resolution":null,"status":{"statusCategory":{"key":"new"}}}},{"key":"PROJ-8","fields":{"summary":"S2","description":"","created":"2026-01-02","resolution":{"name":"Done"}}}]'"#,
        );
        let f = Ferramentas {
            jira,
            ..Default::default()
        };
        let j = JiraConfig {
            url: "https://jira.example.com".into(),
            project_key: "PROJ".into(),
            jql: "labels = fatura ORDER BY updated".into(),
            active_sprint_only: false,
            max_issues: 10,
            credential_cmd: "echo tok".into(),
            ..Default::default()
        };
        let l = listar_jira(&j, &Periodo::Desde("2025-11-01".into()), None, &f).unwrap();
        assert!(!l.excedeu_teto);
        assert_eq!(l.issues.len(), 2);
        assert_eq!(l.issues[0].chave, "PROJ-9");
        assert_eq!(l.issues[0].descricao, "### T");
        assert_eq!(l.issues[0].estado, "aberta");
        assert_eq!(l.issues[0].url, "https://jira.example.com/browse/PROJ-9");
        assert_eq!(l.issues[1].estado, "fechada");
        // Teto menor que o devolvido: aviso.
        let jira2 = script(
            d.path(),
            "jira2",
            r#"echo '[{"key":"PROJ-3","fields":{}},{"key":"PROJ-2","fields":{}}]'"#,
        );
        let f2 = Ferramentas {
            jira: jira2,
            ..Default::default()
        };
        let l = listar_jira(&j, &Periodo::default(), Some(1), &f2).unwrap();
        assert!(l.excedeu_teto);
        assert_eq!(l.issues.len(), 1);
    }

    #[test]
    fn jira_pagina_em_blocos_de_ate_100() {
        let d = tempfile::tempdir().unwrap();
        // Recusa limite fora de 1..=100, como o jira-cli; devolve 100 e depois 50.
        let jira = script(
            d.path(),
            "jira",
            r#"p="$7"; de="${p%%:*}"; lim="${p##*:}"
[ "$lim" -ge 1 ] && [ "$lim" -le 100 ] || { echo "invalid argument for paginate" >&2; exit 1; }
n=$((150 - de)); [ "$n" -gt "$lim" ] && n=$lim
i=0; out=""
while [ "$i" -lt "$n" ]; do
  [ -n "$out" ] && out="$out,"
  out="$out{\"key\":\"PROJ-$((de + i + 1))\",\"fields\":{}}"
  i=$((i + 1))
done
echo "[$out]""#,
        );
        let f = Ferramentas {
            jira,
            ..Default::default()
        };
        let j = JiraConfig {
            url: "https://jira.example.com".into(),
            project_key: "PROJ".into(),
            credential_cmd: "echo tok".into(),
            ..Default::default()
        };
        let l = listar_jira(&j, &Periodo::default(), Some(120), &f).unwrap();
        assert_eq!(l.issues.len(), 120);
        assert!(l.excedeu_teto);
        assert_eq!(l.issues[119].chave, "PROJ-120");
        let l = listar_jira(&j, &Periodo::default(), Some(200), &f).unwrap();
        assert_eq!(l.issues.len(), 150);
        assert!(!l.excedeu_teto);
    }

    #[test]
    fn gitlab_lista_com_periodo_teto_e_estados() {
        let d = tempfile::tempdir().unwrap();
        let glab = script(
            d.path(),
            "glab",
            r#"[ "$GITLAB_TOKEN" = "tok" ] && [ "$GITLAB_HOST" = "gitlab.example.com" ] || exit 1
[ "$1" = "api" ] || exit 2
case "$2" in projects/g%2Fp/issues\?state=all*) ;; *) exit 3;; esac
case "$2" in *"created_after=20"*) ;; *) exit 4;; esac
case "$2" in *"per_page=3&page=1") ;; *) exit 5;; esac
echo '[{"iid":3,"title":"C","description":"c","web_url":"u3","created_at":"2026-10-01T00:00:00Z","state":"opened","labels":["bug"]},{"iid":2,"title":"B","description":"b","web_url":"u2","created_at":"2026-09-01T00:00:00Z","state":"closed","labels":[]},{"iid":1,"title":"A","description":"a","web_url":"u1","created_at":"2026-08-01T00:00:00Z","state":"closed","labels":[]}]'"#,
        );
        let f = Ferramentas {
            glab,
            ..Default::default()
        };
        let g = GitlabConfig {
            host: "gitlab.example.com".into(),
            path: "g/p".into(),
            credential_cmd: "echo tok".into(),
            ..Default::default()
        };
        let l = listar_gitlab(&g, &Periodo::default(), 2, &f).unwrap();
        assert!(l.excedeu_teto);
        assert_eq!(l.issues.len(), 2);
        assert_eq!(l.issues[0].chave, "#3");
        assert_eq!(l.issues[0].origem, OrigemIssue::Gitlab);
        assert_eq!(l.issues[0].estado, "aberta");
        assert_eq!(l.issues[0].rotulos, vec!["bug"]);
        assert_eq!(l.issues[1].estado, "fechada");
        let t = TrackerConfig::Gitlab(g);
        assert!(listar_issues(&t, &Periodo::default(), Some(2), &f).is_ok());
    }

    #[test]
    fn gitlab_pagina_em_blocos_de_ate_100() {
        let d = tempfile::tempdir().unwrap();
        let glab = script(
            d.path(),
            "glab",
            r#"pg="${2##*page=}"; pp="${2##*per_page=}"; pp="${pp%%&*}"
[ "$pp" -ge 1 ] && [ "$pp" -le 100 ] || exit 1
de=$(( (pg - 1) * 100 ))
n=$((150 - de)); [ "$n" -gt "$pp" ] && n=$pp; [ "$n" -lt 0 ] && n=0
i=0; out=""
while [ "$i" -lt "$n" ]; do
  [ -n "$out" ] && out="$out,"
  out="$out{\"iid\":$((de + i + 1)),\"state\":\"opened\"}"
  i=$((i + 1))
done
echo "[$out]""#,
        );
        let f = Ferramentas {
            glab,
            ..Default::default()
        };
        let g = GitlabConfig {
            host: "gitlab.example.com".into(),
            path: "g/p".into(),
            credential_cmd: "echo tok".into(),
            ..Default::default()
        };
        let l = listar_gitlab(&g, &Periodo::default(), 120, &f).unwrap();
        assert_eq!(l.issues.len(), 120);
        assert!(l.excedeu_teto);
        let l = listar_gitlab(&g, &Periodo::default(), 200, &f).unwrap();
        assert_eq!(l.issues.len(), 150);
        assert!(!l.excedeu_teto);
    }

    #[test]
    fn gitlab_sem_token_falha() {
        let t = TrackerConfig::Gitlab(Default::default());
        assert!(listar_issues(&t, &Periodo::default(), None, &Ferramentas::default()).is_err());
    }
}
