//! Rastreador (onde estão as issues) e forja (onde está o repositório de código)
//! de um projeto.
//!
//! Origem: `crates/ct-core/src/tracker.rs` e `crates/ct-ledger/src/hub/config.rs`
//! do ctower-agents. São enums marcados por `kind`; o `JiraConfig` e o
//! `GitlabConfig` são idênticos, campo a campo, aos de lá.
//!
//! O cadastro guarda só o **comando de credencial**, nunca o token.

use crate::ParseEnumError;
use crate::codigo::{RepositorioCodigo, repo_github_ok};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

str_enum!(TrackerKind, "tracker_kind", { Github => "github", Gitlab => "gitlab", Jira => "jira" });
str_enum!(ForgeKind, "forge_kind", { Github => "github", Gitlab => "gitlab" });

// Papel de um tipo de issue do Jira: `epic` vira épico, `bug` e `chore` valem
// como as labels do GitHub, e `ignore` tira a issue da lista.
str_enum!(JiraTypeRole, "jira_type_role", {
    Epic => "epic",
    Issue => "issue",
    Bug => "bug",
    Chore => "chore",
    Ignore => "ignore",
});

/// JQL do modo "somente a sprint ativa".
pub const ACTIVE_SPRINT_JQL: &str = "sprint in openSprints()";
/// Limite padrão de issues fora do modo sprint ativa.
pub const DEFAULT_MAX_ISSUES: u32 = 100;
/// Teto do limite de issues.
pub const MAX_ISSUES_CEILING: u32 = 5000;

fn default_max_issues() -> u32 {
    DEFAULT_MAX_ISSUES
}

/// Repositório GitHub (`owner/repo`), tanto de código quanto de issues.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GithubConfig {
    /// `owner/repo`.
    pub repo: String,
}

/// Configuração do Jira. Mesmos campos e regras do ctower-agents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct JiraConfig {
    /// URL do servidor, sem barra final (`https://jira.example.com`).
    pub url: String,
    /// Chave do projeto Jira (`PROJ`).
    pub project_key: String,
    /// Filtro JQL (obrigatório sem `active_sprint_only`); ignorado com ele.
    pub jql: String,
    /// Usa só a sprint ativa (`sprint in openSprints()`), sem limite.
    pub active_sprint_only: bool,
    /// Fora do modo sprint ativa, quantas issues do `jql`: as mais recentes por
    /// `updated`. Ausente = 100.
    #[serde(default = "default_max_issues")]
    pub max_issues: u32,
    /// Comando que imprime o token (`pass show jira | head -1`). Texto do
    /// cadastro, não secret.
    pub credential_cmd: String,
    /// Configuração do jira-cli; vazio = `~/.config/.jira/.config.yml`.
    pub config_file: String,
    /// Campo do Epic Link (`customfield_10601`).
    pub epic_link_field: String,
    /// Campo do Epic Name (`customfield_10604`).
    pub epic_name_field: String,
    /// Campo da sprint (`customfield_10600`); vazio = detectar em cada issue.
    pub sprint_field: String,
    /// Nome do tipo de issue → papel.
    pub type_map: BTreeMap<String, JiraTypeRole>,
}

impl Default for JiraConfig {
    fn default() -> Self {
        JiraConfig {
            url: String::new(),
            project_key: String::new(),
            jql: String::new(),
            // Projeto novo começa no modo sprint ativa.
            active_sprint_only: true,
            max_issues: DEFAULT_MAX_ISSUES,
            credential_cmd: String::new(),
            config_file: String::new(),
            epic_link_field: String::new(),
            epic_name_field: String::new(),
            sprint_field: String::new(),
            type_map: default_type_map(),
        }
    }
}

impl JiraConfig {
    /// O JQL efetivo: o da sprint ativa nesse modo, senão o `jql` do cadastro.
    pub fn effective_jql(&self) -> String {
        if self.active_sprint_only {
            ACTIVE_SPRINT_JQL.to_string()
        } else {
            self.jql.trim().to_string()
        }
    }

    /// Quantas issues; `None` = sem limite (modo sprint ativa).
    pub fn issue_limit(&self) -> Option<usize> {
        (!self.active_sprint_only).then(|| self.max_issues.clamp(1, MAX_ISSUES_CEILING) as usize)
    }

    /// Número da chave do projeto (`PROJ-5450` → `5450`); `None` se a chave é de
    /// outro projeto ou não tem número.
    pub fn number_from_key(&self, key: &str) -> Option<i64> {
        let n = key
            .strip_prefix(self.project_key.as_str())?
            .strip_prefix('-')?;
        if n.is_empty() || !n.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        n.parse().ok().filter(|n| *n > 0)
    }

    /// Host da URL (`https://jira.example.com:8443/x` → `jira.example.com`): a
    /// conta do token no chaveiro.
    pub fn host(&self) -> String {
        let u = self.url.trim();
        let resto = u.split_once("://").map_or(u, |(_, r)| r);
        let autoridade = resto.split('/').next().unwrap_or("");
        let autoridade = autoridade.rsplit_once('@').map_or(autoridade, |(_, h)| h);
        autoridade
            .split(':')
            .next()
            .unwrap_or("")
            .to_ascii_lowercase()
    }

    /// Link da issue: `<url>/browse/PROJ-5450`.
    pub fn issue_url(&self, key: &str) -> String {
        format!("{}/browse/{key}", self.url.trim_end_matches('/'))
    }
}

/// Configuração do GitLab (forja ou rastreador). Idêntica à do ctower-agents.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GitlabConfig {
    /// Host do GitLab (`gitlab.example.com`).
    pub host: String,
    /// Caminho do projeto (`grupo/projeto/app`).
    pub path: String,
    /// URL de clone como o usuário colou (HTTPS ou SSH).
    pub clone_url: String,
    /// Comando que imprime o token (`pass show gitlab | head -1`).
    pub credential_cmd: String,
}

/// Rastreador do projeto com a configuração dele.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TrackerConfig {
    Github(GithubConfig),
    Gitlab(GitlabConfig),
    Jira(JiraConfig),
}

/// Forja única do cadastro antigo (antes da #178). Só é lida, para migrar para
/// [`RepositorioCodigo`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ForgeConfig {
    Github(GithubConfig),
    Gitlab(GitlabConfig),
}

impl TrackerConfig {
    pub fn kind(&self) -> TrackerKind {
        match self {
            TrackerConfig::Github(_) => TrackerKind::Github,
            TrackerConfig::Gitlab(_) => TrackerKind::Gitlab,
            TrackerConfig::Jira(_) => TrackerKind::Jira,
        }
    }

    pub fn jira(&self) -> Option<&JiraConfig> {
        match self {
            TrackerConfig::Jira(c) => Some(c),
            _ => None,
        }
    }
}

impl ForgeConfig {
    pub fn kind(&self) -> ForgeKind {
        match self {
            ForgeConfig::Github(_) => ForgeKind::Github,
            ForgeConfig::Gitlab(_) => ForgeKind::Gitlab,
        }
    }

    pub fn gitlab(&self) -> Option<&GitlabConfig> {
        match self {
            ForgeConfig::Gitlab(c) => Some(c),
            _ => None,
        }
    }
}

impl ForgeKind {
    /// Nome da forja nas mensagens: `GitHub` ou `GitLab`.
    pub fn label(self) -> &'static str {
        match self {
            ForgeKind::Github => "GitHub",
            ForgeKind::Gitlab => "GitLab",
        }
    }
}

/// Mapa de tipos padrão do cadastro Jira.
pub fn default_type_map() -> BTreeMap<String, JiraTypeRole> {
    [
        ("Epic", JiraTypeRole::Epic),
        ("Épico", JiraTypeRole::Epic),
        ("Bug", JiraTypeRole::Bug),
        ("Erro", JiraTypeRole::Bug),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect()
}

/// Onde ficam o código e as issues de um projeto. Os dois são opcionais: sem
/// eles o app funciona como antes (só pastas locais).
///
/// O cadastro antigo tinha uma forja só (`forge`); ao ler, ela vira uma lista de
/// um repositório de código (#178). A gravação só escreve `codigo`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, from = "RepositoriosGravados")]
pub struct Repositorios {
    /// Repositórios de código (GitHub e GitLab, em qualquer combinação).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub codigo: Vec<RepositorioCodigo>,
    pub tracker: Option<TrackerConfig>,
}

/// Forma gravada, com o campo `forge` do cadastro antigo.
#[derive(Deserialize, Default)]
#[serde(default)]
struct RepositoriosGravados {
    codigo: Vec<RepositorioCodigo>,
    forge: Option<ForgeConfig>,
    tracker: Option<TrackerConfig>,
}

impl From<RepositoriosGravados> for Repositorios {
    fn from(g: RepositoriosGravados) -> Self {
        let mut codigo = g.codigo;
        if codigo.is_empty()
            && let Some(f) = g.forge
        {
            codigo.push(f.into());
        }
        Repositorios {
            codigo,
            tracker: g.tracker,
        }
    }
}

impl Repositorios {
    pub fn vazio(&self) -> bool {
        self.codigo.is_empty() && self.tracker.is_none()
    }

    /// Algum repositório de código cadastrado é o remoto `url`?
    pub fn codigo_aponta_para(&self, url: &str) -> bool {
        self.codigo.iter().any(|r| r.aponta_para(url))
    }

    /// Confere o cadastro sem rede.
    pub fn validar(&self) -> Result<(), String> {
        let mut chaves = std::collections::HashSet::new();
        for (i, r) in self.codigo.iter().enumerate() {
            r.validar(i + 1)?;
            if !chaves.insert(r.chave()) {
                return Err(format!("repositório de código repetido: {}", r.rotulo()));
            }
        }
        match &self.tracker {
            Some(TrackerConfig::Github(g)) => validar_repo_github(g, "issues")?,
            Some(TrackerConfig::Gitlab(g)) => validar_gitlab(g, "issues")?,
            Some(TrackerConfig::Jira(j)) => validar_jira(j)?,
            None => {}
        }
        Ok(())
    }

    /// Confere que o cadastro novo não muda o que a spec manda fixo depois do
    /// cadastro: cada repositório de código já cadastrado (forja, host e caminho)
    /// continua lá, e o tipo do rastreador e, no GitLab, host e caminho, não mudam.
    /// Repositórios de código novos podem ser acrescentados.
    pub fn validar_troca(&self, novo: &Repositorios) -> Result<(), String> {
        for a in &self.codigo {
            match novo.codigo.iter().find(|b| b.chave() == a.chave()) {
                None => {
                    return Err(format!(
                        "o repositório de código {} não pode ser removido nem trocado",
                        a.rotulo()
                    ));
                }
                Some(b) if b.forja != a.forja => {
                    return Err(
                        "o tipo do repositório de código não muda depois do cadastro".into(),
                    );
                }
                Some(_) => {}
            }
        }
        match (&self.tracker, &novo.tracker) {
            (Some(a), Some(b)) if a.kind() != b.kind() => {
                return Err("o tipo do repositório de issues não muda depois do cadastro".into());
            }
            (Some(TrackerConfig::Gitlab(a)), Some(TrackerConfig::Gitlab(b)))
                if (a.host.as_str(), a.path.as_str()) != (b.host.as_str(), b.path.as_str()) =>
            {
                return Err("host e caminho do GitLab não mudam depois do cadastro".into());
            }
            (Some(_), None) => {
                return Err("o repositório de issues não pode ser removido".into());
            }
            _ => {}
        }
        Ok(())
    }
}

fn validar_repo_github(g: &GithubConfig, o_que: &str) -> Result<(), String> {
    if repo_github_ok(g.repo.trim()) {
        Ok(())
    } else {
        Err(format!(
            "repositório GitHub de {o_que} inválido: {:?} (use dono/repositório)",
            g.repo
        ))
    }
}

fn validar_gitlab(g: &GitlabConfig, o_que: &str) -> Result<(), String> {
    if g.host.is_empty() || !g.path.contains('/') {
        return Err(format!(
            "repositório do GitLab de {o_que} inválido: host {:?}, caminho {:?}",
            g.host, g.path
        ));
    }
    // Sem comando de credencial, vale o token global do host no chaveiro (#179).
    Ok(())
}

/// Mesmas regras de `crates/ct-ledger/src/hub/config.rs` do ctower-agents.
fn validar_jira(j: &JiraConfig) -> Result<(), String> {
    let url = j.url.to_ascii_lowercase();
    if !(url.starts_with("https://") || url.starts_with("http://")) || url.len() < 10 {
        return Err(format!("URL do Jira inválida: {:?}", j.url));
    }
    if j.url.ends_with('/') {
        return Err(format!("URL do Jira sem a barra final: {:?}", j.url));
    }
    let key_ok = j.project_key.starts_with(|c: char| c.is_ascii_uppercase())
        && j.project_key
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
    if !key_ok {
        return Err(format!(
            "chave do projeto Jira inválida: {:?} (maiúsculas, como PROJ)",
            j.project_key
        ));
    }
    // Sem comando de credencial, vale o token do host do Jira no chaveiro (#180).
    if j.active_sprint_only {
        return Ok(());
    }
    if j.jql.trim().is_empty() {
        return Err(
            "informe o JQL (ex.: labels = minha-equipe AND updated >= -90d) \
                    ou marque \"somente a sprint ativa\""
                .into(),
        );
    }
    if j.max_issues == 0 || j.max_issues > MAX_ISSUES_CEILING {
        return Err(format!(
            "limite de issues inválido: {} (de 1 a {MAX_ISSUES_CEILING})",
            j.max_issues
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn jira() -> JiraConfig {
        JiraConfig {
            url: "https://jira.example.com".into(),
            project_key: "PROJ".into(),
            credential_cmd: "pass show jira | head -1".into(),
            ..Default::default()
        }
    }

    fn gitlab() -> GitlabConfig {
        GitlabConfig {
            host: "gitlab.example.com".into(),
            path: "grupo/projeto/app".into(),
            clone_url: "https://gitlab.example.com/grupo/projeto/app.git".into(),
            credential_cmd: "pass show gitlab | head -1".into(),
        }
    }

    fn github() -> GithubConfig {
        GithubConfig {
            repo: "deep-ion-ai/app".into(),
        }
    }

    fn cod(f: ForgeConfig) -> RepositorioCodigo {
        f.into()
    }

    fn repos(f: ForgeConfig, t: TrackerConfig) -> Repositorios {
        Repositorios {
            codigo: vec![cod(f)],
            tracker: Some(t),
        }
    }

    #[test]
    fn padroes_do_jira_novo() {
        let j = JiraConfig::default();
        assert!(j.active_sprint_only);
        assert_eq!(j.max_issues, 100);
        assert_eq!(j.type_map, default_type_map());
        assert_eq!(j.type_map.get("Erro"), Some(&JiraTypeRole::Bug));
    }

    #[test]
    fn aceita_todas_as_combinacoes() {
        let forjas = [ForgeConfig::Github(github()), ForgeConfig::Gitlab(gitlab())];
        let rastreadores = [
            TrackerConfig::Github(github()),
            TrackerConfig::Gitlab(gitlab()),
            TrackerConfig::Jira(jira()),
        ];
        for f in &forjas {
            for t in &rastreadores {
                repos(f.clone(), t.clone()).validar().unwrap();
            }
        }
    }

    #[test]
    fn jira_url_chave_e_credencial() {
        let ok = |j: JiraConfig| {
            Repositorios {
                codigo: vec![],
                tracker: Some(TrackerConfig::Jira(j)),
            }
            .validar()
        };
        assert!(ok(jira()).is_ok());
        for url in [
            "ftp://jira.example.com",
            "jira.example.com",
            "https://a",
            "https://jira.example.com/",
        ] {
            assert!(
                ok(JiraConfig {
                    url: url.into(),
                    ..jira()
                })
                .is_err(),
                "{url}"
            );
        }
        for k in ["", "proj", "1PROJ", "PR-J", "PROJ "] {
            assert!(
                ok(JiraConfig {
                    project_key: k.into(),
                    ..jira()
                })
                .is_err(),
                "{k:?}"
            );
        }
        assert!(
            ok(JiraConfig {
                project_key: "P_1".into(),
                ..jira()
            })
            .is_ok()
        );
        // Sem comando: o token vem do chaveiro.
        assert!(
            ok(JiraConfig {
                credential_cmd: "  ".into(),
                ..jira()
            })
            .is_ok()
        );
    }

    #[test]
    fn jira_jql_e_limite_fora_do_modo_sprint() {
        let ok = |j: JiraConfig| {
            Repositorios {
                codigo: vec![],
                tracker: Some(TrackerConfig::Jira(j)),
            }
            .validar()
        };
        let fora = JiraConfig {
            active_sprint_only: false,
            ..jira()
        };
        assert!(ok(fora.clone()).is_err(), "JQL obrigatório");
        let com_jql = JiraConfig {
            jql: "labels = web".into(),
            ..fora
        };
        assert!(ok(com_jql.clone()).is_ok());
        assert!(
            ok(JiraConfig {
                max_issues: 0,
                ..com_jql.clone()
            })
            .is_err()
        );
        assert!(
            ok(JiraConfig {
                max_issues: 5001,
                ..com_jql.clone()
            })
            .is_err()
        );
        assert!(
            ok(JiraConfig {
                max_issues: 5000,
                ..com_jql
            })
            .is_ok()
        );
        // No modo sprint ativa o JQL e o limite são ignorados.
        assert!(
            ok(JiraConfig {
                max_issues: 0,
                ..jira()
            })
            .is_ok()
        );
    }

    #[test]
    fn gitlab_e_github() {
        let f = |f: ForgeConfig| {
            Repositorios {
                codigo: vec![cod(f)],
                tracker: None,
            }
            .validar()
        };
        assert!(
            f(ForgeConfig::Gitlab(GitlabConfig {
                host: "".into(),
                ..gitlab()
            }))
            .is_err()
        );
        assert!(
            f(ForgeConfig::Gitlab(GitlabConfig {
                path: "app".into(),
                ..gitlab()
            }))
            .is_err()
        );
        assert!(
            f(ForgeConfig::Gitlab(GitlabConfig {
                credential_cmd: "".into(),
                ..gitlab()
            }))
            .is_err()
        );
        for r in ["", "app", "a/b/c", "a/", "/b", "a b/c"] {
            assert!(
                f(ForgeConfig::Github(GithubConfig { repo: r.into() })).is_err(),
                "{r:?}"
            );
        }
    }

    #[test]
    fn tipo_e_caminho_nao_mudam() {
        let base = repos(ForgeConfig::Github(github()), TrackerConfig::Jira(jira()));
        let troca_tipo = repos(ForgeConfig::Gitlab(gitlab()), TrackerConfig::Jira(jira()));
        assert!(base.validar_troca(&troca_tipo).is_err());
        let troca_tracker = repos(
            ForgeConfig::Github(github()),
            TrackerConfig::Github(github()),
        );
        assert!(base.validar_troca(&troca_tracker).is_err());
        // Mesmo tipo, outra configuração do Jira: pode.
        let jira2 = JiraConfig {
            jql: "x".into(),
            active_sprint_only: false,
            ..jira()
        };
        assert!(
            base.validar_troca(&repos(
                ForgeConfig::Github(github()),
                TrackerConfig::Jira(jira2)
            ))
            .is_ok()
        );
        // GitLab: host e caminho fixos; o comando de credencial pode mudar.
        let gl = repos(ForgeConfig::Gitlab(gitlab()), TrackerConfig::Jira(jira()));
        let outro_host = GitlabConfig {
            host: "outro.example.com".into(),
            ..gitlab()
        };
        assert!(
            gl.validar_troca(&repos(
                ForgeConfig::Gitlab(outro_host),
                TrackerConfig::Jira(jira())
            ))
            .is_err()
        );
        let outra_cred = GitlabConfig {
            credential_cmd: "pass show x".into(),
            ..gitlab()
        };
        assert!(
            gl.validar_troca(&repos(
                ForgeConfig::Gitlab(outra_cred),
                TrackerConfig::Jira(jira())
            ))
            .is_ok()
        );
        assert!(base.validar_troca(&Repositorios::default()).is_err());
        assert!(Repositorios::default().validar_troca(&base).is_ok());
    }

    #[test]
    fn varios_repositorios_de_codigo() {
        let dois = Repositorios {
            codigo: vec![
                cod(ForgeConfig::Github(github())),
                cod(ForgeConfig::Gitlab(gitlab())),
            ],
            tracker: None,
        };
        dois.validar().unwrap();
        assert!(dois.codigo_aponta_para("git@gitlab.example.com:grupo/projeto/app.git"));
        assert!(dois.codigo_aponta_para("https://github.com/deep-ion-ai/app"));
        assert!(!dois.codigo_aponta_para("https://github.com/deep-ion-ai/outro"));
        // Repetido (mesmo host e caminho, sem diferenciar maiúsculas): recusa.
        let mut repetido = dois.clone();
        let mut r = cod(ForgeConfig::Github(github()));
        r.caminho = "Deep-Ion-AI/App".into();
        repetido.codigo.push(r);
        assert!(repetido.validar().unwrap_err().contains("repetido"));
        // Acrescentar pode; remover ou trocar um já cadastrado, não.
        let um = Repositorios {
            codigo: vec![cod(ForgeConfig::Github(github()))],
            tracker: None,
        };
        assert!(um.validar_troca(&dois).is_ok());
        assert!(dois.validar_troca(&um).is_err());
        let mut com_branch = dois.clone();
        com_branch.codigo[1].branch = "develop".into();
        assert!(dois.validar_troca(&com_branch).is_ok());
    }

    #[test]
    fn cadastro_antigo_com_uma_forja_vira_lista_de_um() {
        let antigo = json!({
            "forge": {
                "kind": "gitlab",
                "host": "gitlab.example.com",
                "path": "grupo/projeto/app",
                "clone_url": "https://gitlab.example.com/grupo/projeto/app.git",
                "credential_cmd": "pass show gitlab | head -1"
            },
            "tracker": { "kind": "github", "repo": "deep-ion-ai/app" }
        });
        let r: Repositorios = serde_json::from_value(antigo).unwrap();
        assert_eq!(r.codigo, vec![cod(ForgeConfig::Gitlab(gitlab()))]);
        assert_eq!(r.tracker, Some(TrackerConfig::Github(github())));
        r.validar().unwrap();
        // Gravado de novo, só a forma nova; relido, igual.
        let v = serde_json::to_value(&r).unwrap();
        assert!(v.get("forge").is_none());
        assert_eq!(v["codigo"][0]["forja"], "gitlab");
        assert_eq!(v["codigo"][0]["token"], "comando");
        assert_eq!(serde_json::from_value::<Repositorios>(v).unwrap(), r);
        // Sem nada: vazio, e o campo some na gravação.
        let vazio: Repositorios = serde_json::from_value(json!({})).unwrap();
        assert!(vazio.vazio());
        assert_eq!(
            serde_json::to_value(&vazio).unwrap(),
            json!({ "tracker": null })
        );
    }

    #[test]
    fn serializa_marcado_por_kind_e_campos_ausentes_ficam_vazios() {
        let v = serde_json::to_value(TrackerConfig::Jira(jira())).unwrap();
        assert_eq!(v["kind"], "jira");
        assert_eq!(v["project_key"], "PROJ");
        assert_eq!(v["max_issues"], 100);
        assert_eq!(
            serde_json::from_value::<TrackerConfig>(v).unwrap(),
            TrackerConfig::Jira(jira())
        );
        let f: ForgeConfig =
            serde_json::from_value(json!({ "kind": "gitlab", "host": "gitlab.example.com" }))
                .unwrap();
        assert_eq!(f.gitlab().unwrap().path, "");
        let j: JiraConfig =
            serde_json::from_value(json!({ "project_key": "P", "jql": " a " })).unwrap();
        assert_eq!(j.max_issues, 100);
    }

    #[test]
    fn jql_limite_e_chave() {
        let j = jira();
        assert_eq!(j.effective_jql(), "sprint in openSprints()");
        assert_eq!(j.issue_limit(), None);
        let f = JiraConfig {
            active_sprint_only: false,
            jql: " a ".into(),
            max_issues: 1_000_000,
            ..j.clone()
        };
        assert_eq!(f.effective_jql(), "a");
        assert_eq!(f.issue_limit(), Some(5000));
        assert_eq!(j.number_from_key("PROJ-5450"), Some(5450));
        assert_eq!(j.host(), "jira.example.com");
        let com_porta = JiraConfig {
            url: "https://u@Jira.Example.com:8443/jira".into(),
            ..j.clone()
        };
        assert_eq!(com_porta.host(), "jira.example.com");
        for k in ["OUTRO-1", "PROJ5450", "PROJ-", "PROJ-12a", "PROJ-0"] {
            assert_eq!(j.number_from_key(k), None, "{k}");
        }
        assert_eq!(
            JiraConfig {
                url: "https://j.example.com/".into(),
                ..j
            }
            .issue_url("PROJ-1"),
            "https://j.example.com/browse/PROJ-1"
        );
    }
}
