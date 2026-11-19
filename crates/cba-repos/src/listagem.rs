//! Leitura em lote das issues do rastreador por período (#270).
//!
//! Mesmas CLIs e credenciais de [`crate::demanda`]: `gh issue list` e
//! `jira issue list --raw`. Só leitura; o token chega ao filho só no ambiente.

use crate::demanda::{
    DemandaErro, Ferramentas, rodar_json, texto, token_github, token_jira, wiki_para_markdown,
};
use crate::{GithubConfig, JiraConfig, Secret, TrackerConfig};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

/// Período padrão, em meses.
pub const MESES_PADRAO: u32 = 6;
/// Teto padrão de issues por leitura (GitHub; o Jira usa o `max_issues` do cadastro).
pub const TETO_PADRAO: usize = 200;

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
    Jira,
}

impl OrigemIssue {
    pub fn nome(self) -> &'static str {
        match self {
            OrigemIssue::Github => "github",
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

pub fn listar_github(
    g: &GithubConfig,
    periodo: &Periodo,
    teto: usize,
    f: &Ferramentas,
) -> Result<Listagem, DemandaErro> {
    let desde = periodo.data_a_partir_de(hoje())?;
    let teto = teto.max(1);
    let token = token_github(f);
    let mut c = Command::new(&f.gh);
    if let Some(t) = &token {
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
        "number,title,body,url,createdAt,state,labels",
    ]);
    let v = rodar_json("gh", c, token.as_ref().unwrap_or(&Secret::new("")))?;
    let mut issues: Vec<IssueListada> = lista(v, "gh")?
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

/// JQL da listagem: projeto + criação + o `jql` do cadastro (sem `ORDER BY`).
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
    jql.push_str(" ORDER BY created DESC");
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
    let mut c = Command::new(&f.jira);
    c.args([
        "issue",
        "list",
        "--jql",
        &jql_da_listagem(j, &desde),
        "--raw",
        "--paginate",
        &format!("0:{}", teto + 1),
    ])
    .env("JIRA_API_TOKEN", token.expose());
    if !j.config_file.trim().is_empty() {
        c.env("JIRA_CONFIG_FILE", j.config_file.trim());
    }
    let v = rodar_json("jira", c, &token)?;
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

/// Lê as issues do rastreador do projeto. GitLab fica fora deste escopo.
pub fn listar_issues(
    tracker: &TrackerConfig,
    periodo: &Periodo,
    teto: Option<usize>,
    f: &Ferramentas,
) -> Result<Listagem, DemandaErro> {
    match tracker {
        TrackerConfig::Github(g) => listar_github(g, periodo, teto.unwrap_or(TETO_PADRAO), f),
        TrackerConfig::Jira(j) => listar_jira(j, periodo, teto, f),
        TrackerConfig::Gitlab(_) => Err(DemandaErro::Falhou {
            ferramenta: "glab",
            detalhe: "a listagem de issues do GitLab ainda não é suportada".into(),
        }),
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
        let g = GithubConfig { repo: "o/r".into() };
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
    fn jira_monta_jql_com_periodo_e_cadastro() {
        let d = tempfile::tempdir().unwrap();
        let jira = script(
            d.path(),
            "jira",
            r#"[ "$JIRA_API_TOKEN" = "tok" ] || exit 1
[ "$1 $2 $3" = "issue list --jql" ] || exit 2
echo "$4" | grep -q '^project = PROJ AND created >= "2025-11-' || exit 3
echo "$4" | grep -q 'AND (labels = fatura)' || exit 4
echo "$4" | grep -q 'ORDER BY created DESC$' || exit 5
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
    fn gitlab_nao_e_suportado_e_nada_vaza() {
        let t = TrackerConfig::Gitlab(Default::default());
        assert!(listar_issues(&t, &Periodo::default(), None, &Ferramentas::default()).is_err());
    }
}
