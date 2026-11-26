//! Mudanças em andamento (#441): os PRs (GitHub) e MRs (GitLab) abertos num
//! repositório de código e os arquivos que cada um altera.
//!
//! `gh pr list` e `glab mr list` não filtram por arquivo: a lista dos abertos
//! (os mais recentes, até um limite) sai de `gh api …/pulls` e
//! `glab api …/merge_requests`, página a página, e os arquivos de cada um de
//! `gh api …/pulls/N/files` e `glab api …/merge_requests/N/changes`. O token do
//! repositório sai do resolvedor ([`crate::chaveiro::token_do_repositorio`]) e
//! vai só no ambiente da CLI. Nada é escrito na forja.

use std::collections::HashMap;
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use regex::Regex;
use serde::Serialize;
use serde_json::Value;

use crate::chaveiro::{ErroToken, token_do_repositorio};
use crate::demanda::{DemandaErro, Ferramentas, rodar_json, texto};
use crate::estado::chave_normalizada;
use crate::providers::com_nova_tentativa;
use crate::{ForgeKind, RepositorioCodigo, Secret, TrackerConfig};

/// PRs/MRs abertos lidos por repositório, no máximo, quando não informado.
pub const LIMITE_PADRAO: usize = 50;

/// Itens por página nas chamadas à API.
pub const POR_PAGINA: usize = 50;

/// Páginas de arquivos de um PR, no máximo (o GitHub devolve até 3000 arquivos).
const PAGINAS_DE_ARQUIVOS: usize = 30;

/// PR ou MR aberto.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MudancaAberta {
    /// `#12` no GitHub, `!12` no GitLab.
    pub numero: String,
    pub titulo: String,
    pub branch: String,
    pub autor: String,
    /// Criação, como a API devolve (ISO 8601).
    pub data: String,
    pub url: String,
    /// Commit da ponta, para o cache dos arquivos.
    #[serde(skip)]
    pub ponta: String,
    #[serde(skip)]
    pub iid: u64,
}

/// Lista dos abertos, com o aviso de corte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Abertas {
    pub mudancas: Vec<MudancaAberta>,
    /// Havia mais abertos que o limite.
    pub cortada: bool,
}

/// Arquivos alterados num PR/MR (caminhos relativos ao repositório).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArquivosAlterados {
    pub arquivos: Vec<String>,
    /// A forja não devolveu a lista inteira.
    pub cortada: bool,
}

fn token(repo: &RepositorioCodigo, f: &Ferramentas) -> Result<Option<Secret>, DemandaErro> {
    token_do_repositorio(repo, f.chaveiro.as_ref()).map_err(|e| match e {
        ErroToken::SemTokenProprio(r) => DemandaErro::SemToken(r),
        e => DemandaErro::from(e),
    })
}

/// Chama `gh api` ou `glab api` com o token do repositório no ambiente.
fn api(repo: &RepositorioCodigo, rota: &str, f: &Ferramentas) -> Result<Value, DemandaErro> {
    com_nova_tentativa(
        || token(repo, f),
        |t| {
            let (ferramenta, mut c) = match repo.forja {
                ForgeKind::Github => {
                    let mut c = Command::new(&f.gh);
                    c.env("GH_HOST", repo.host.trim());
                    if let Some(t) = t {
                        c.env("GH_TOKEN", t.expose());
                    }
                    ("gh", c)
                }
                ForgeKind::Gitlab => {
                    let mut c = Command::new(&f.glab);
                    c.env("GITLAB_HOST", repo.host.trim());
                    if let Some(t) = t {
                        c.env("GITLAB_TOKEN", t.expose());
                    }
                    ("glab", c)
                }
            };
            c.args(["api", rota]);
            rodar_json(ferramenta, c, t.unwrap_or(&Secret::new("")))
        },
    )
}

fn id_gitlab(repo: &RepositorioCodigo) -> String {
    repo.caminho.trim_matches('/').replace('/', "%2F")
}

/// Os PRs/MRs abertos mais recentes do repositório, até `limite`.
pub fn abertas(
    repo: &RepositorioCodigo,
    limite: usize,
    por_pagina: usize,
    f: &Ferramentas,
) -> Result<Abertas, DemandaErro> {
    let por_pagina = por_pagina.clamp(1, 100);
    let mut mudancas = Vec::new();
    let mut pagina = 1;
    loop {
        let rota = match repo.forja {
            ForgeKind::Github => format!(
                "repos/{}/pulls?state=open&sort=created&direction=desc&per_page={por_pagina}&page={pagina}",
                repo.caminho.trim_matches('/')
            ),
            ForgeKind::Gitlab => format!(
                "projects/{}/merge_requests?state=opened&order_by=created_at&sort=desc&per_page={por_pagina}&page={pagina}",
                id_gitlab(repo)
            ),
        };
        let v = api(repo, &rota, f)?;
        let itens = v.as_array().cloned().unwrap_or_default();
        let n = itens.len();
        mudancas.extend(itens.iter().map(|i| mudanca(repo.forja, i)));
        if n < por_pagina || mudancas.len() > limite {
            break;
        }
        pagina += 1;
    }
    let cortada = mudancas.len() > limite;
    mudancas.truncate(limite);
    Ok(Abertas { mudancas, cortada })
}

fn mudanca(forja: ForgeKind, i: &Value) -> MudancaAberta {
    match forja {
        ForgeKind::Github => {
            let n = i["number"].as_u64().unwrap_or(0);
            MudancaAberta {
                numero: format!("#{n}"),
                titulo: texto(i, &["title"]).into(),
                branch: texto(i, &["head", "ref"]).into(),
                autor: texto(i, &["user", "login"]).into(),
                data: texto(i, &["created_at"]).into(),
                url: texto(i, &["html_url"]).into(),
                ponta: texto(i, &["head", "sha"]).into(),
                iid: n,
            }
        }
        ForgeKind::Gitlab => {
            let n = i["iid"].as_u64().unwrap_or(0);
            MudancaAberta {
                numero: format!("!{n}"),
                titulo: texto(i, &["title"]).into(),
                branch: texto(i, &["source_branch"]).into(),
                autor: texto(i, &["author", "username"]).into(),
                data: texto(i, &["created_at"]).into(),
                url: texto(i, &["web_url"]).into(),
                ponta: texto(i, &["sha"]).into(),
                iid: n,
            }
        }
    }
}

/// Arquivos que o PR/MR altera (o caminho novo e, se renomeado, o antigo).
pub fn arquivos(
    repo: &RepositorioCodigo,
    m: &MudancaAberta,
    por_pagina: usize,
    f: &Ferramentas,
) -> Result<ArquivosAlterados, DemandaErro> {
    let mut arquivos = Vec::new();
    match repo.forja {
        ForgeKind::Github => {
            let por_pagina = por_pagina.clamp(1, 100);
            for pagina in 1..=PAGINAS_DE_ARQUIVOS {
                let rota = format!(
                    "repos/{}/pulls/{}/files?per_page={por_pagina}&page={pagina}",
                    repo.caminho.trim_matches('/'),
                    m.iid
                );
                let v = api(repo, &rota, f)?;
                let itens = v.as_array().cloned().unwrap_or_default();
                for i in &itens {
                    arquivos.push(texto(i, &["filename"]).to_string());
                    let antigo = texto(i, &["previous_filename"]);
                    if !antigo.is_empty() {
                        arquivos.push(antigo.to_string());
                    }
                }
                if itens.len() < por_pagina {
                    return Ok(ArquivosAlterados {
                        arquivos,
                        cortada: false,
                    });
                }
            }
            Ok(ArquivosAlterados {
                arquivos,
                cortada: true,
            })
        }
        ForgeKind::Gitlab => {
            let rota = format!(
                "projects/{}/merge_requests/{}/changes",
                id_gitlab(repo),
                m.iid
            );
            let v = api(repo, &rota, f)?;
            for c in v["changes"].as_array().into_iter().flatten() {
                let novo = texto(c, &["new_path"]);
                let antigo = texto(c, &["old_path"]);
                arquivos.push(novo.to_string());
                if !antigo.is_empty() && antigo != novo {
                    arquivos.push(antigo.to_string());
                }
            }
            let cortada = v["overflow"].as_bool().unwrap_or(false)
                || v["changes_count"]
                    .as_str()
                    .is_some_and(|c| c.ends_with('+'));
            Ok(ArquivosAlterados { arquivos, cortada })
        }
    }
}

/// Chaves de demanda citadas no título e na branch: `PROJ-123` (Jira), `#123`
/// e o número no começo do nome da branch (`feat/123-…`). Com o rastreador do
/// projeto, só as que valem nele.
pub fn chaves_de_demanda(
    titulo: &str,
    branch: &str,
    tracker: Option<&TrackerConfig>,
) -> Vec<String> {
    let jira = Regex::new(r"(?i)\b([a-z][a-z0-9_]+-\d+)\b").unwrap();
    let cerquilha = Regex::new(r"#(\d+)\b").unwrap();
    let da_branch = Regex::new(r"^(?:[^/]+/)*(\d+)(?:[-_]|$)").unwrap();
    let mut candidatas = Vec::new();
    for t in [titulo, branch] {
        candidatas.extend(jira.captures_iter(t).map(|c| c[1].to_ascii_uppercase()));
        candidatas.extend(cerquilha.captures_iter(t).map(|c| format!("#{}", &c[1])));
    }
    if let Some(c) = da_branch.captures(branch) {
        candidatas.push(format!("#{}", &c[1]));
    }
    let mut chaves: Vec<String> = Vec::new();
    for c in candidatas {
        let c = match tracker {
            Some(t) => match chave_normalizada(t, &c) {
                // `#N` só vale como número; a chave do Jira precisa do projeto.
                Some(n) if c.starts_with('#') || !n.starts_with('#') => n,
                _ => continue,
            },
            None => c,
        };
        if !chaves.contains(&c) {
            chaves.push(c);
        }
    }
    chaves
}

/// Cache curto, por processo (uma sessão do agente): a lista dos abertos por
/// repositório vale [`Cache::validade`]; os arquivos de um PR/MR valem enquanto
/// a ponta dele não mudar.
#[derive(Debug)]
pub struct Cache {
    validade: Duration,
    listas: Mutex<HashMap<(String, usize), (Instant, Abertas)>>,
    arquivos: Mutex<HashMap<(String, u64, String), ArquivosAlterados>>,
}

impl Default for Cache {
    fn default() -> Self {
        Cache::com_validade(Duration::from_secs(300))
    }
}

impl Cache {
    pub fn com_validade(validade: Duration) -> Self {
        Cache {
            validade,
            listas: Mutex::default(),
            arquivos: Mutex::default(),
        }
    }

    /// [`abertas`], pelo cache.
    pub fn abertas(
        &self,
        repo: &RepositorioCodigo,
        limite: usize,
        por_pagina: usize,
        f: &Ferramentas,
    ) -> Result<Abertas, DemandaErro> {
        let chave = (repo.chave(), limite);
        if let Some((quando, a)) = self.listas.lock().unwrap().get(&chave)
            && quando.elapsed() < self.validade
        {
            return Ok(a.clone());
        }
        let a = abertas(repo, limite, por_pagina, f)?;
        self.listas
            .lock()
            .unwrap()
            .insert(chave, (Instant::now(), a.clone()));
        Ok(a)
    }

    /// [`arquivos`], pelo cache (pela ponta do PR/MR).
    pub fn arquivos(
        &self,
        repo: &RepositorioCodigo,
        m: &MudancaAberta,
        por_pagina: usize,
        f: &Ferramentas,
    ) -> Result<ArquivosAlterados, DemandaErro> {
        let chave = (repo.chave(), m.iid, m.ponta.clone());
        if !m.ponta.is_empty()
            && let Some(a) = self.arquivos.lock().unwrap().get(&chave)
        {
            return Ok(a.clone());
        }
        let a = arquivos(repo, m, por_pagina, f)?;
        if !m.ponta.is_empty() {
            self.arquivos.lock().unwrap().insert(chave, a.clone());
        }
        Ok(a)
    }
}

#[cfg(test)]
mod tests_puros {
    use super::*;
    use crate::{GithubConfig, JiraConfig};

    #[test]
    fn chaves_do_titulo_e_da_branch() {
        assert_eq!(
            chaves_de_demanda("FAT-12: cancelar nota (#7)", "feat/41-cancelar", None),
            ["FAT-12", "#7", "#41"]
        );
        let jira = TrackerConfig::Jira(JiraConfig {
            url: "https://jira.example.com".into(),
            project_key: "FAT".into(),
            ..Default::default()
        });
        assert_eq!(
            chaves_de_demanda("Ajuste UTF-8 e OUTRO-3", "feature/fat-12-nota", Some(&jira)),
            ["FAT-12"]
        );
        let gh = TrackerConfig::Github(GithubConfig {
            repo: "o/r".into(),
            ..Default::default()
        });
        assert_eq!(
            chaves_de_demanda("FAT-12 nota", "41-nota", Some(&gh)),
            ["#41"]
        );
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::demanda::falsas::script;

    fn repo_gh() -> RepositorioCodigo {
        RepositorioCodigo {
            caminho: "o/faturamento".into(),
            token: crate::OrigemToken::Comando,
            comando_credencial: "echo tok-repo".into(),
            ..Default::default()
        }
    }

    /// `gh` falso: a lista de abertos com `total` PRs, em páginas, e 3 arquivos
    /// por PR; conta as chamadas em `chamadas`.
    fn gh_paginado(dir: &std::path::Path, total: usize) -> std::path::PathBuf {
        let n = dir.join("chamadas");
        script(
            dir,
            "gh",
            &format!(
                r#"echo "$2" >> "{n}"
[ "$GH_TOKEN" = "tok-repo" ] && [ "$GH_HOST" = "github.com" ] || {{ echo "401 Bad credentials" >&2; exit 1; }}
case "$*" in *tok-repo*) echo "token no argumento" >&2; exit 3;; esac
total={total}
case "$2" in
  repos/o/faturamento/pulls/*/files*)
    num=$(echo "$2" | sed 's|repos/o/faturamento/pulls/\([0-9]*\)/files.*|\1|')
    case "$2" in *page=1) echo "[{{\"filename\":\"src/nota.rs\"}},{{\"filename\":\"src/pr$num.rs\",\"previous_filename\":\"src/velho$num.rs\"}}]";; *) echo "[]";; esac;;
  repos/o/faturamento/pulls\?*)
    pp=$(echo "$2" | sed 's|.*per_page=\([0-9]*\).*|\1|'); p=$(echo "$2" | sed 's|.*page=\([0-9]*\)$|\1|')
    ini=$(( (p - 1) * pp + 1 )); fim=$(( p * pp )); [ $fim -gt $total ] && fim=$total
    sep=""; printf "["
    i=$ini; while [ $i -le $fim ]; do
      printf '%s{{"number":%d,"title":"FAT-%d nota","head":{{"ref":"feat/%d-x","sha":"s%d"}},"user":{{"login":"ana"}},"created_at":"2026-11-0%dT00:00:00Z","html_url":"https://github.com/o/faturamento/pull/%d"}}' "$sep" $i $i $i $i 1 $i
      sep=","; i=$((i + 1))
    done
    echo "]";;
  *) echo "rota $2" >&2; exit 2;;
esac"#,
                n = n.display()
            ),
        )
    }

    fn chamadas(dir: &std::path::Path) -> Vec<String> {
        std::fs::read_to_string(dir.join("chamadas"))
            .unwrap_or_default()
            .lines()
            .map(str::to_string)
            .collect()
    }

    #[test]
    fn github_pagina_ate_o_limite_e_avisa_o_corte() {
        let d = tempfile::tempdir().unwrap();
        let f = Ferramentas {
            gh: gh_paginado(d.path(), 7),
            ..Default::default()
        };
        let a = abertas(&repo_gh(), 5, 3, &f).unwrap();
        assert_eq!(a.mudancas.len(), 5);
        assert!(a.cortada);
        assert_eq!(a.mudancas[0].numero, "#1");
        assert_eq!(a.mudancas[0].branch, "feat/1-x");
        assert_eq!(a.mudancas[0].autor, "ana");
        assert_eq!(a.mudancas[0].url, "https://github.com/o/faturamento/pull/1");
        assert_eq!(chamadas(d.path()).len(), 2, "duas páginas de 3");

        let todas = abertas(&repo_gh(), 50, 3, &f).unwrap();
        assert_eq!(todas.mudancas.len(), 7);
        assert!(!todas.cortada);
    }

    #[test]
    fn github_arquivos_com_renomeado_e_cache_pela_ponta() {
        let d = tempfile::tempdir().unwrap();
        let f = Ferramentas {
            gh: gh_paginado(d.path(), 2),
            ..Default::default()
        };
        let cache = Cache::default();
        let a = cache.abertas(&repo_gh(), 50, 50, &f).unwrap();
        let arq = cache.arquivos(&repo_gh(), &a.mudancas[1], 2, &f).unwrap();
        assert_eq!(arq.arquivos, ["src/nota.rs", "src/pr2.rs", "src/velho2.rs"]);
        assert!(!arq.cortada);
        let antes = chamadas(d.path()).len();
        cache.abertas(&repo_gh(), 50, 50, &f).unwrap();
        cache.arquivos(&repo_gh(), &a.mudancas[1], 2, &f).unwrap();
        assert_eq!(chamadas(d.path()).len(), antes, "do cache");
    }

    #[test]
    fn sem_rede_da_erro_sem_o_token() {
        let d = tempfile::tempdir().unwrap();
        let f = Ferramentas {
            gh: script(
                d.path(),
                "gh",
                r#"echo "error connecting to api.github.com $GH_TOKEN" >&2; exit 1"#,
            ),
            ..Default::default()
        };
        let e = abertas(&repo_gh(), 50, 50, &f).unwrap_err().to_string();
        assert!(
            e.contains("error connecting") && !e.contains("tok-repo"),
            "{e}"
        );
    }

    #[test]
    fn gitlab_lista_mrs_e_arquivos_das_mudancas() {
        let d = tempfile::tempdir().unwrap();
        let glab = script(
            d.path(),
            "glab",
            r#"[ "$GITLAB_TOKEN" = "tok-gl" ] && [ "$GITLAB_HOST" = "gitlab.example.com" ] || { echo "401 Unauthorized" >&2; exit 1; }
case "$2" in
  "projects/g%2Ffaturamento/merge_requests?state=opened&order_by=created_at&sort=desc&per_page=50&page=1")
    echo '[{"iid":9,"title":"Nota de serviço","source_branch":"FAT-12-servico","author":{"username":"bruno"},"created_at":"2026-11-03T00:00:00Z","web_url":"https://gitlab.example.com/g/faturamento/-/merge_requests/9","sha":"abc"}]';;
  "projects/g%2Ffaturamento/merge_requests/9/changes")
    echo '{"changes":[{"old_path":"app/nota.rb","new_path":"app/nota.rb"}],"overflow":false}';;
  *) echo "rota $2" >&2; exit 2;;
esac"#,
        );
        let f = Ferramentas {
            glab,
            ..Default::default()
        };
        let repo = RepositorioCodigo {
            forja: ForgeKind::Gitlab,
            host: "gitlab.example.com".into(),
            caminho: "g/faturamento".into(),
            token: crate::OrigemToken::Comando,
            comando_credencial: "echo tok-gl".into(),
            ..Default::default()
        };
        let a = abertas(&repo, 50, 50, &f).unwrap();
        assert_eq!(a.mudancas[0].numero, "!9");
        assert_eq!(a.mudancas[0].branch, "FAT-12-servico");
        let arq = arquivos(&repo, &a.mudancas[0], 50, &f).unwrap();
        assert_eq!(arq.arquivos, ["app/nota.rb"]);
    }
}
