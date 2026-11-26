//! Leitura completa da issue da demanda (#351): além de título e descrição, a
//! situação, os responsáveis, os rótulos, os comentários (do mais antigo ao mais
//! recente, em Markdown), os anexos (nome e tipo, sem baixar) e as issues
//! ligadas. É o que a ferramenta `ler_issue_da_demanda` devolve ao agente.
//!
//! Mesmo caminho de [`super::buscar_demanda`]: as CLIs com o token do resolvedor
//! só no ambiente do filho e a nova tentativa em caso de recusa. O resultado é
//! cortado por [`LimitesIssue`], com o corte anotado em [`IssueLida::avisos`],
//! para não estourar o contexto do agente.

use std::process::Command;

use regex::Regex;
use serde::Serialize;
use serde_json::Value;

use super::{
    DemandaErro, Ferramentas, numero, rodar_json, texto, token_github, token_gitlab, token_jira,
    wiki_para_markdown,
};
use crate::providers::com_nova_tentativa;
use crate::{GithubConfig, GitlabConfig, JiraConfig, Secret, TrackerConfig};

/// Até quanto da issue vai para o agente.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LimitesIssue {
    /// Comentários mais recentes, no máximo.
    pub comentarios: usize,
    /// Caracteres de cada comentário.
    pub caracteres_por_comentario: usize,
    /// Caracteres dos comentários somados (os mais antigos saem primeiro).
    pub caracteres_dos_comentarios: usize,
    /// Caracteres da descrição.
    pub caracteres_da_descricao: usize,
}

impl Default for LimitesIssue {
    fn default() -> Self {
        LimitesIssue {
            comentarios: 30,
            caracteres_por_comentario: 4_000,
            caracteres_dos_comentarios: 40_000,
            caracteres_da_descricao: 20_000,
        }
    }
}

/// Comentário da issue, com o texto em Markdown.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Comentario {
    pub autor: String,
    /// Como o rastreador devolve (ISO 8601).
    pub data: String,
    pub texto: String,
    /// O texto passou do limite e foi cortado.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub cortado: bool,
}

/// Anexo listado (não baixado).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Anexo {
    pub nome: String,
    /// Tipo MIME, quando o rastreador informa ou a extensão indica.
    pub tipo: String,
}

/// Issue ligada à da demanda.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IssueLigada {
    pub chave: String,
    /// Como o rastreador nomeia a ligação ("blocks", "relates_to", "is blocked by").
    pub relacao: String,
    pub titulo: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub url: String,
}

/// A issue da demanda lida do rastreador, já dentro dos limites.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IssueLida {
    pub chave: String,
    pub titulo: String,
    /// Em Markdown (o wiki do Jira é convertido).
    pub descricao: String,
    pub url: String,
    pub situacao: String,
    pub responsaveis: Vec<String>,
    pub rotulos: Vec<String>,
    /// Do mais antigo ao mais recente.
    pub comentarios: Vec<Comentario>,
    /// Comentários na issue (pode ser maior que os devolvidos).
    pub total_de_comentarios: usize,
    pub anexos: Vec<Anexo>,
    pub ligadas: Vec<IssueLigada>,
    /// Cortes e leituras parciais, ditos ao agente.
    pub avisos: Vec<String>,
}

/// Lê a issue `ref_` no rastreador do projeto, com comentários, anexos e ligadas.
pub fn ler_issue(
    tracker: &TrackerConfig,
    ref_: &str,
    limites: &LimitesIssue,
    f: &Ferramentas,
) -> Result<IssueLida, DemandaErro> {
    let bruta = match tracker {
        TrackerConfig::Github(g) => ler_github(g, ref_, f)?,
        TrackerConfig::Gitlab(g) => ler_gitlab(g, ref_, f)?,
        TrackerConfig::Jira(j) => ler_jira(j, ref_, limites, f)?,
    };
    Ok(bruta.limitar(limites))
}

/// O que veio do rastreador, antes dos limites.
#[derive(Default)]
struct Bruta {
    chave: String,
    titulo: String,
    descricao: String,
    url: String,
    situacao: String,
    responsaveis: Vec<String>,
    rotulos: Vec<String>,
    /// Do mais antigo ao mais recente.
    comentarios: Vec<Comentario>,
    /// Total informado pelo rastreador, quando maior que os comentários lidos.
    total: Option<usize>,
    anexos: Vec<Anexo>,
    ligadas: Vec<IssueLigada>,
    avisos: Vec<String>,
}

impl Bruta {
    fn limitar(mut self, l: &LimitesIssue) -> IssueLida {
        let mut avisos = std::mem::take(&mut self.avisos);
        let (descricao, cortou) = cortar(&self.descricao, l.caracteres_da_descricao);
        if cortou {
            avisos.push(format!(
                "a descrição foi cortada em {} caracteres",
                l.caracteres_da_descricao
            ));
        }
        let lidos = self.comentarios.len();
        let total = self.total.unwrap_or(lidos).max(lidos);
        let mut comentarios = self.comentarios;
        if comentarios.len() > l.comentarios {
            comentarios.drain(..comentarios.len() - l.comentarios);
        }
        let mut longos = 0;
        for c in &mut comentarios {
            let (t, cortou) = cortar(&c.texto, l.caracteres_por_comentario);
            if cortou {
                c.texto = t;
                c.cortado = true;
                longos += 1;
            }
        }
        let tamanho = |cs: &[Comentario]| cs.iter().map(|c| c.texto.chars().count()).sum::<usize>();
        while comentarios.len() > 1 && tamanho(&comentarios) > l.caracteres_dos_comentarios {
            comentarios.remove(0);
        }
        if comentarios.len() < total {
            avisos.push(format!(
                "comentários cortados: vieram os {} mais recentes de {total}; os mais antigos \
                 ficaram de fora (leia-os na issue, pelo link, se precisar)",
                comentarios.len()
            ));
        }
        if longos > 0 {
            avisos.push(format!(
                "{longos} comentário(s) passaram de {} caracteres e foram cortados (marcados \
                 com `cortado`)",
                l.caracteres_por_comentario
            ));
        }
        IssueLida {
            chave: self.chave,
            titulo: self.titulo,
            descricao,
            url: self.url,
            situacao: self.situacao,
            responsaveis: self.responsaveis,
            rotulos: self.rotulos,
            comentarios,
            total_de_comentarios: total,
            anexos: self.anexos,
            ligadas: self.ligadas,
            avisos,
        }
    }
}

/// Os `max` primeiros caracteres, com a marca do corte.
fn cortar(t: &str, max: usize) -> (String, bool) {
    if t.chars().count() <= max {
        return (t.to_string(), false);
    }
    let mut s: String = t.chars().take(max).collect();
    s.push_str("\n\n[… cortado]");
    (s, true)
}

fn textos(v: &Value, campo: &[&str]) -> Vec<String> {
    v.as_array()
        .into_iter()
        .flatten()
        .map(|x| texto(x, campo).to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Anexos citados no Markdown (`[nome](…/uploads/…)` no GitLab,
/// `…/user-attachments/…` no GitHub): os rastreadores guardam o anexo como link.
fn anexos_no_markdown<'a>(textos: impl IntoIterator<Item = &'a str>) -> Vec<Anexo> {
    let link = Regex::new(r"!?\[([^\]]*)\]\(([^)\s]+)\)").unwrap();
    let mut vistos = std::collections::BTreeSet::new();
    let mut anexos = Vec::new();
    for t in textos {
        for c in link.captures_iter(t) {
            let url = &c[2];
            if !(url.contains("/uploads/") || url.contains("/user-attachments/")) {
                continue;
            }
            if !vistos.insert(url.to_string()) {
                continue;
            }
            let arquivo = url.rsplit('/').next().unwrap_or(url);
            let nome = if c[1].trim().is_empty() {
                arquivo.to_string()
            } else {
                c[1].trim().to_string()
            };
            let tipo = tipo_pela_extensao(&nome)
                .or_else(|| tipo_pela_extensao(arquivo))
                .unwrap_or(if c[0].starts_with('!') {
                    "image/*"
                } else {
                    "desconhecido"
                });
            anexos.push(Anexo {
                nome,
                tipo: tipo.to_string(),
            });
        }
    }
    anexos
}

fn tipo_pela_extensao(nome: &str) -> Option<&'static str> {
    let ext = nome.rsplit_once('.')?.1.to_ascii_lowercase();
    Some(match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "pdf" => "application/pdf",
        "txt" | "log" => "text/plain",
        "csv" => "text/csv",
        "md" => "text/markdown",
        "json" => "application/json",
        "xml" => "application/xml",
        "zip" => "application/zip",
        "doc" | "docx" => "application/msword",
        "xls" | "xlsx" => "application/vnd.ms-excel",
        "ppt" | "pptx" => "application/vnd.ms-powerpoint",
        "mp4" => "video/mp4",
        _ => return None,
    })
}

fn ler_github(g: &GithubConfig, ref_: &str, f: &Ferramentas) -> Result<Bruta, DemandaErro> {
    let n = numero(ref_)?;
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
                "title,body,url,state,assignees,labels,comments",
            ]);
            rodar_json("gh", c, token.unwrap_or(&Secret::new("")))
        },
    )?;
    let comentarios: Vec<Comentario> = v["comments"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|c| Comentario {
            autor: texto(c, &["author", "login"]).to_string(),
            data: texto(c, &["createdAt"]).to_string(),
            texto: texto(c, &["body"]).to_string(),
            cortado: false,
        })
        .collect();
    let descricao = texto(&v, &["body"]).to_string();
    let anexos = anexos_no_markdown(
        std::iter::once(descricao.as_str()).chain(comentarios.iter().map(|c| c.texto.as_str())),
    );
    Ok(Bruta {
        chave: format!("#{n}"),
        titulo: texto(&v, &["title"]).to_string(),
        url: texto(&v, &["url"]).to_string(),
        situacao: texto(&v, &["state"]).to_string(),
        responsaveis: textos(&v["assignees"], &["login"]),
        rotulos: textos(&v["labels"], &["name"]),
        descricao,
        comentarios,
        anexos,
        // O GitHub não tem ligação entre issues na API do `gh issue view`; as
        // referências (#N) ficam no texto.
        ..Default::default()
    })
}

fn ler_gitlab(g: &GitlabConfig, ref_: &str, f: &Ferramentas) -> Result<Bruta, DemandaErro> {
    let n = numero(ref_)?;
    let id = g.path.trim_matches('/').replace('/', "%2F");
    let api = |rota: String| {
        com_nova_tentativa(
            || token_gitlab(g, f).map(Some),
            |token| {
                let token = token.ok_or_else(|| DemandaErro::SemToken(g.host.trim().into()))?;
                let mut c = Command::new(&f.glab);
                c.args(["api", &rota])
                    .env("GITLAB_HOST", g.host.trim())
                    .env("GITLAB_TOKEN", token.expose());
                rodar_json("glab", c, token)
            },
        )
    };
    let v = api(format!("projects/{id}/issues/{n}"))?;
    let mut avisos = Vec::new();
    // Os 100 mais recentes (a API pagina de 100 em 100); o limite corta depois.
    let notas = api(format!(
        "projects/{id}/issues/{n}/notes?sort=desc&order_by=created_at&per_page=100"
    ))?;
    let notas = notas.as_array().cloned().unwrap_or_default();
    let pagina_cheia = notas.len() >= 100;
    let mut comentarios: Vec<Comentario> = notas
        .iter()
        .filter(|n| !n["system"].as_bool().unwrap_or(false))
        .map(|c| Comentario {
            autor: texto(c, &["author", "username"]).to_string(),
            data: texto(c, &["created_at"]).to_string(),
            texto: texto(c, &["body"]).to_string(),
            cortado: false,
        })
        .collect();
    comentarios.reverse();
    if pagina_cheia {
        avisos.push(
            "a issue tem mais de 100 notas; só as 100 mais recentes foram consideradas".into(),
        );
    }
    let ligadas = match api(format!("projects/{id}/issues/{n}/links")) {
        Ok(l) => l
            .as_array()
            .into_iter()
            .flatten()
            .map(|i| IssueLigada {
                chave: match texto(i, &["references", "full"]) {
                    "" => format!("#{}", i["iid"].as_u64().unwrap_or(0)),
                    r => r.to_string(),
                },
                relacao: texto(i, &["link_type"]).to_string(),
                titulo: texto(i, &["title"]).to_string(),
                url: texto(i, &["web_url"]).to_string(),
            })
            .collect(),
        Err(e) => {
            avisos.push(format!("issues ligadas não lidas: {e}"));
            Vec::new()
        }
    };
    let descricao = texto(&v, &["description"]).to_string();
    let anexos = anexos_no_markdown(
        std::iter::once(descricao.as_str()).chain(comentarios.iter().map(|c| c.texto.as_str())),
    );
    let mut responsaveis = textos(&v["assignees"], &["username"]);
    if responsaveis.is_empty() {
        responsaveis = textos(&Value::Array(vec![v["assignee"].clone()]), &["username"]);
    }
    Ok(Bruta {
        chave: format!("#{n}"),
        titulo: texto(&v, &["title"]).to_string(),
        url: texto(&v, &["web_url"]).to_string(),
        situacao: texto(&v, &["state"]).to_string(),
        responsaveis,
        rotulos: v["labels"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|r| r.as_str().map(str::to_string))
            .collect(),
        descricao,
        comentarios,
        anexos,
        ligadas,
        avisos,
        total: None,
    })
}

/// Texto de um campo do Jira: wiki (API v2) vira Markdown; ADF (API v3) vira o
/// texto dos nós, em parágrafos.
fn texto_jira(v: &Value) -> String {
    match v {
        Value::String(s) => wiki_para_markdown(s),
        Value::Object(_) => {
            fn juntar(v: &Value, out: &mut String) {
                if let Some(t) = v["text"].as_str() {
                    out.push_str(t);
                }
                for filho in v["content"].as_array().into_iter().flatten() {
                    juntar(filho, out);
                }
                if matches!(
                    v["type"].as_str(),
                    Some("paragraph" | "heading" | "listItem" | "codeBlock")
                ) {
                    out.push_str("\n\n");
                }
            }
            let mut out = String::new();
            juntar(v, &mut out);
            out.trim_end().to_string()
        }
        _ => String::new(),
    }
}

fn ler_jira(
    j: &JiraConfig,
    ref_: &str,
    limites: &LimitesIssue,
    f: &Ferramentas,
) -> Result<Bruta, DemandaErro> {
    let chave = ref_.trim().to_ascii_uppercase();
    if j.number_from_key(&chave).is_none() {
        return Err(DemandaErro::Referencia(ref_.to_string()));
    }
    let v = com_nova_tentativa(
        || token_jira(j, f).map(Some),
        |token| {
            let token = token.ok_or_else(|| DemandaErro::SemToken(j.host()))?;
            let mut c = Command::new(&f.jira);
            c.args([
                "issue",
                "view",
                &chave,
                "--raw",
                "--comments",
                &limites.comentarios.to_string(),
            ])
            .env("JIRA_API_TOKEN", token.expose());
            if !j.config_file.trim().is_empty() {
                c.env("JIRA_CONFIG_FILE", j.config_file.trim());
            }
            rodar_json("jira", c, token)
        },
    )?;
    let campos = &v["fields"];
    let comentarios: Vec<Comentario> = campos["comment"]["comments"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|c| Comentario {
            autor: texto(c, &["author", "displayName"]).to_string(),
            data: texto(c, &["created"]).to_string(),
            texto: texto_jira(&c["body"]),
            cortado: false,
        })
        .collect();
    let total = campos["comment"]["total"]
        .as_u64()
        .and_then(|t| usize::try_from(t).ok());
    let anexos = campos["attachment"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|a| Anexo {
            nome: texto(a, &["filename"]).to_string(),
            tipo: match texto(a, &["mimeType"]) {
                "" => tipo_pela_extensao(texto(a, &["filename"]))
                    .unwrap_or("desconhecido")
                    .to_string(),
                t => t.to_string(),
            },
        })
        .collect();
    let ligadas = campos["issuelinks"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|l| {
            let (lado, relacao) = if l["outwardIssue"].is_object() {
                ("outwardIssue", texto(l, &["type", "outward"]))
            } else if l["inwardIssue"].is_object() {
                ("inwardIssue", texto(l, &["type", "inward"]))
            } else {
                return None;
            };
            let chave = texto(&l[lado], &["key"]).to_string();
            Some(IssueLigada {
                url: j.issue_url(&chave),
                relacao: relacao.to_string(),
                titulo: texto(&l[lado], &["fields", "summary"]).to_string(),
                chave,
            })
        })
        .collect();
    Ok(Bruta {
        titulo: texto(campos, &["summary"]).to_string(),
        descricao: texto_jira(&campos["description"]),
        url: j.issue_url(&chave),
        situacao: texto(campos, &["status", "name"]).to_string(),
        responsaveis: textos(
            &Value::Array(vec![campos["assignee"].clone()]),
            &["displayName"],
        ),
        rotulos: campos["labels"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|r| r.as_str().map(str::to_string))
            .collect(),
        comentarios,
        total,
        anexos,
        ligadas,
        chave,
        avisos: Vec::new(),
    })
}

#[cfg(test)]
mod tests_puros {
    use super::*;

    fn comentario(t: &str) -> Comentario {
        Comentario {
            autor: "ana".into(),
            data: "2026-11-01".into(),
            texto: t.into(),
            cortado: false,
        }
    }

    #[test]
    fn limites_mantem_os_mais_recentes_e_avisam_o_corte() {
        let bruta = Bruta {
            descricao: "d".repeat(50),
            comentarios: (1..=5).map(|i| comentario(&format!("c{i}"))).collect(),
            total: Some(7),
            ..Default::default()
        };
        let l = LimitesIssue {
            comentarios: 3,
            caracteres_por_comentario: 10,
            caracteres_dos_comentarios: 100,
            caracteres_da_descricao: 20,
        };
        let i = bruta.limitar(&l);
        let textos: Vec<_> = i.comentarios.iter().map(|c| c.texto.as_str()).collect();
        assert_eq!(textos, ["c3", "c4", "c5"]);
        assert_eq!(i.total_de_comentarios, 7);
        assert!(i.descricao.ends_with("[… cortado]"));
        assert!(i.avisos.iter().any(|a| a.contains("3 mais recentes de 7")));
        assert!(i.avisos.iter().any(|a| a.contains("descrição")));
    }

    #[test]
    fn comentario_longo_e_soma_grande_sao_cortados() {
        let bruta = Bruta {
            comentarios: vec![comentario(&"a".repeat(30)), comentario(&"b".repeat(30))],
            ..Default::default()
        };
        let l = LimitesIssue {
            comentarios: 10,
            caracteres_por_comentario: 20,
            caracteres_dos_comentarios: 40,
            caracteres_da_descricao: 100,
        };
        let i = bruta.limitar(&l);
        assert_eq!(i.comentarios.len(), 1, "o mais antigo sai pelo teto");
        assert!(i.comentarios[0].cortado && i.comentarios[0].texto.starts_with('b'));
        assert!(i.avisos.iter().any(|a| a.contains("1 mais recentes de 2")));
        assert!(i.avisos.iter().any(|a| a.contains("`cortado`")));
    }

    #[test]
    fn anexos_saem_dos_links_de_upload() {
        let a = anexos_no_markdown([
            "veja ![tela](/uploads/abc/tela.png) e [planilha.xlsx](/uploads/x/planilha.xlsx)",
            "[site](https://exemplo.com) [log](https://github.com/user-attachments/files/1/erro.log)",
        ]);
        let pares: Vec<_> = a
            .iter()
            .map(|a| (a.nome.as_str(), a.tipo.as_str()))
            .collect();
        assert_eq!(
            pares,
            [
                ("tela", "image/png"),
                ("planilha.xlsx", "application/vnd.ms-excel"),
                ("log", "text/plain"),
            ]
        );
    }

    #[test]
    fn adf_do_jira_vira_texto() {
        let v = serde_json::json!({"type":"doc","content":[
            {"type":"paragraph","content":[{"type":"text","text":"Olá"}]},
            {"type":"paragraph","content":[{"type":"text","text":"mundo"}]}
        ]});
        assert_eq!(texto_jira(&v), "Olá\n\nmundo");
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::demanda::falsas::script;

    fn jira_cfg() -> JiraConfig {
        JiraConfig {
            url: "https://jira.example.com".into(),
            project_key: "FAT".into(),
            credential_cmd: "echo tok-jira".into(),
            ..Default::default()
        }
    }

    #[test]
    fn jira_traz_comentarios_em_markdown_anexos_e_ligadas() {
        let d = tempfile::tempdir().unwrap();
        let jira = script(
            d.path(),
            "jira",
            r#"[ "$JIRA_API_TOKEN" = "tok-jira" ] || { echo "sem token" >&2; exit 1; }
[ "$1 $2 $3 $4 $5" = "issue view FAT-12 --raw --comments" ] || { echo "args: $*" >&2; exit 2; }
cat <<'FIM'
{"key":"FAT-12","fields":{"summary":"Cancelar nota","description":"h2. Regra\n*negrito*",
"status":{"name":"Em análise"},"assignee":{"displayName":"Ana Exemplo"},"labels":["faturamento"],
"comment":{"total":2,"comments":[
 {"author":{"displayName":"Bruno Exemplo"},"created":"2026-11-01T10:00:00.000-0300","body":"Primeiro {{campo}}"},
 {"author":{"displayName":"Ana Exemplo"},"created":"2026-11-02T10:00:00.000-0300","body":"Só *notas de serviço*"}]},
"attachment":[{"filename":"tela.png","mimeType":"image/png"}],
"issuelinks":[{"type":{"inward":"is blocked by","outward":"blocks"},"outwardIssue":{"key":"FAT-3","fields":{"summary":"Outra"}}}]}}
FIM"#,
        );
        let f = Ferramentas {
            jira,
            ..Default::default()
        };
        let tracker = TrackerConfig::Jira(jira_cfg());
        let i = ler_issue(&tracker, "fat-12", &LimitesIssue::default(), &f).unwrap();
        assert_eq!(i.chave, "FAT-12");
        assert_eq!(i.descricao, "## Regra\n**negrito**");
        assert_eq!(i.situacao, "Em análise");
        assert_eq!(i.responsaveis, ["Ana Exemplo"]);
        assert_eq!(i.rotulos, ["faturamento"]);
        assert_eq!(i.comentarios.len(), 2);
        assert_eq!(i.comentarios[0].autor, "Bruno Exemplo");
        assert_eq!(i.comentarios[0].texto, "Primeiro `campo`");
        assert_eq!(i.comentarios[1].texto, "Só **notas de serviço**");
        assert_eq!(i.anexos[0].nome, "tela.png");
        assert_eq!(i.ligadas[0].chave, "FAT-3");
        assert_eq!(i.ligadas[0].relacao, "blocks");
        assert_eq!(i.ligadas[0].url, "https://jira.example.com/browse/FAT-3");
        assert!(i.avisos.is_empty(), "{:?}", i.avisos);
    }

    #[test]
    fn jira_sem_comentarios_e_sem_rede() {
        let d = tempfile::tempdir().unwrap();
        let f = Ferramentas {
            jira: script(
                d.path(),
                "jira",
                r#"echo '{"key":"FAT-1","fields":{"summary":"T","description":null,"comment":{"total":0,"comments":[]}}}'"#,
            ),
            ..Default::default()
        };
        let tracker = TrackerConfig::Jira(jira_cfg());
        let i = ler_issue(&tracker, "FAT-1", &LimitesIssue::default(), &f).unwrap();
        assert!(i.comentarios.is_empty() && i.descricao.is_empty());
        assert_eq!(i.total_de_comentarios, 0);

        let sem_rede = Ferramentas {
            jira: script(
                d.path(),
                "jira-off",
                r#"echo "dial tcp: lookup jira.example.com: no such host ($JIRA_API_TOKEN)" >&2; exit 1"#,
            ),
            ..Default::default()
        };
        let e = ler_issue(&tracker, "FAT-1", &LimitesIssue::default(), &sem_rede)
            .unwrap_err()
            .to_string();
        assert!(e.contains("no such host") && !e.contains("tok-jira"), "{e}");
    }

    #[test]
    fn github_traz_comentarios_rotulos_e_responsaveis_com_o_token_so_no_ambiente() {
        use crate::chaveiro::{Chaveiro, ChaveiroMemoria, conta_global};
        let d = tempfile::tempdir().unwrap();
        let chaveiro = std::sync::Arc::new(ChaveiroMemoria::default());
        chaveiro
            .gravar(&conta_global("github.com"), &Secret::new("gh-tok-123"))
            .unwrap();
        let gh = script(
            d.path(),
            "gh",
            r#"[ "$GH_TOKEN" = "gh-tok-123" ] || { echo "sem token" >&2; exit 1; }
case "$*" in *gh-tok*) echo "token no argumento" >&2; exit 3;; esac
[ "$3 $4 $5 $6" = "5 -R o/r --json" ] || exit 2
cat <<'FIM'
{"title":"T","body":"Corpo ![print](https://github.com/user-attachments/assets/a1.png)","url":"https://github.com/o/r/issues/5","state":"OPEN",
"assignees":[{"login":"ana"}],"labels":[{"name":"bug"}],
"comments":[{"author":{"login":"bruno"},"createdAt":"2026-11-01T00:00:00Z","body":"Mudou: só serviço"}]}
FIM"#,
        );
        let f = Ferramentas {
            gh,
            chaveiro,
            ..Default::default()
        };
        let tracker = TrackerConfig::Github(GithubConfig { repo: "o/r".into() });
        let i = ler_issue(&tracker, "#5", &LimitesIssue::default(), &f).unwrap();
        assert_eq!(i.situacao, "OPEN");
        assert_eq!(i.responsaveis, ["ana"]);
        assert_eq!(i.rotulos, ["bug"]);
        assert_eq!(i.comentarios[0].autor, "bruno");
        assert_eq!(i.comentarios[0].texto, "Mudou: só serviço");
        assert_eq!(i.anexos[0].tipo, "image/png");
    }

    #[test]
    fn github_sem_comentarios_e_sem_rede() {
        let d = tempfile::tempdir().unwrap();
        let f = Ferramentas {
            gh: script(
                d.path(),
                "gh",
                r#"echo '{"title":"T","body":"","url":"u","state":"CLOSED","assignees":[],"labels":[],"comments":[]}'"#,
            ),
            ..Default::default()
        };
        let tracker = TrackerConfig::Github(GithubConfig { repo: "o/r".into() });
        let i = ler_issue(&tracker, "5", &LimitesIssue::default(), &f).unwrap();
        assert!(i.comentarios.is_empty() && i.avisos.is_empty());
        let off = Ferramentas {
            gh: script(
                d.path(),
                "gh-off",
                r#"echo "error connecting to api.github.com" >&2; exit 1"#,
            ),
            ..Default::default()
        };
        let e = ler_issue(&tracker, "5", &LimitesIssue::default(), &off).unwrap_err();
        assert!(e.to_string().contains("error connecting"), "{e}");
    }

    fn gitlab_cfg() -> GitlabConfig {
        GitlabConfig {
            host: "gitlab.example.com".into(),
            path: "g/app".into(),
            credential_cmd: "echo tok-gl".into(),
            ..Default::default()
        }
    }

    #[test]
    fn gitlab_traz_notas_sem_as_do_sistema_ligadas_e_anexos() {
        let d = tempfile::tempdir().unwrap();
        let glab = script(
            d.path(),
            "glab",
            r#"[ "$GITLAB_TOKEN" = "tok-gl" ] || { echo "401 Unauthorized" >&2; exit 1; }
case "$2" in
  "projects/g%2Fapp/issues/7") echo '{"title":"T","description":"Ver [planilha.csv](/uploads/a/planilha.csv)","web_url":"https://gitlab.example.com/g/app/-/issues/7","state":"opened","assignees":[{"username":"ana"}],"labels":["urgente"]}';;
  "projects/g%2Fapp/issues/7/notes?sort=desc&order_by=created_at&per_page=100") echo '[{"author":{"username":"ana"},"created_at":"2026-11-03","body":"novo","system":false},{"author":{"username":"bot"},"created_at":"2026-11-02","body":"changed label","system":true},{"author":{"username":"bruno"},"created_at":"2026-11-01","body":"antigo","system":false}]';;
  "projects/g%2Fapp/issues/7/links") echo '[{"iid":3,"title":"Outra","web_url":"https://gitlab.example.com/g/app/-/issues/3","link_type":"blocks","references":{"full":"g/app#3"}}]';;
  *) echo "rota $2" >&2; exit 2;;
esac"#,
        );
        let f = Ferramentas {
            glab,
            ..Default::default()
        };
        let tracker = TrackerConfig::Gitlab(gitlab_cfg());
        let i = ler_issue(&tracker, "!7", &LimitesIssue::default(), &f).unwrap();
        let textos: Vec<_> = i.comentarios.iter().map(|c| c.texto.as_str()).collect();
        assert_eq!(textos, ["antigo", "novo"], "do mais antigo ao mais recente");
        assert_eq!(i.situacao, "opened");
        assert_eq!(i.responsaveis, ["ana"]);
        assert_eq!(i.rotulos, ["urgente"]);
        assert_eq!(i.ligadas[0].chave, "g/app#3");
        assert_eq!(i.anexos[0].tipo, "text/csv");
    }

    #[test]
    fn gitlab_sem_comentarios_e_sem_rede() {
        let d = tempfile::tempdir().unwrap();
        let glab = script(
            d.path(),
            "glab",
            r#"case "$2" in
  *notes*|*links) echo '[]';;
  *) echo '{"title":"T","description":"","web_url":"u","state":"closed","assignees":[],"labels":[]}';;
esac"#,
        );
        let tracker = TrackerConfig::Gitlab(gitlab_cfg());
        let f = Ferramentas {
            glab,
            ..Default::default()
        };
        let i = ler_issue(&tracker, "7", &LimitesIssue::default(), &f).unwrap();
        assert!(i.comentarios.is_empty() && i.ligadas.is_empty() && i.avisos.is_empty());
        let off = Ferramentas {
            glab: script(
                d.path(),
                "glab-off",
                r#"echo "dial tcp: lookup gitlab.example.com: no such host" >&2; exit 1"#,
            ),
            ..Default::default()
        };
        let e = ler_issue(&tracker, "7", &LimitesIssue::default(), &off).unwrap_err();
        assert!(matches!(e, DemandaErro::Falhou { .. }), "{e}");
    }
}
