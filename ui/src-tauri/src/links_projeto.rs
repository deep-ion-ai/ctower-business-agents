//! Links de forja e de rastreador abertos no navegador do sistema (#398).
//!
//! A permissão `opener:allow-open-url` da janela só libera os links oficiais da
//! Anthropic. Os links do projeto (a issue no GitHub, no GitLab ou no Jira, o PR, a
//! linha da evidência) passam por aqui: o host da URL tem de ser o de um repositório
//! cadastrado no projeto, o do rastreador ou o de um repositório de código. Jira pode
//! estar num host próprio, então a lista vem do cadastro, não de uma constante.
//!
//! Nada é buscado pela rede: só se abre a URL no navegador padrão.

use cba_core::dados::DiretorioDados;
use cba_repos::TrackerConfig;
use tauri_plugin_opener::OpenerExt;
use url::Url;

use crate::repositorios::cadastrado;

/// Hosts que o projeto cadastrou: o do rastreador e o de cada repositório de código.
pub fn hosts_do_projeto(codigo_hosts: &[String], tracker: Option<&TrackerConfig>) -> Vec<String> {
    let mut hosts: Vec<String> = codigo_hosts
        .iter()
        .map(|h| h.trim().to_lowercase())
        .collect();
    match tracker {
        Some(TrackerConfig::Github(_)) => hosts.push("github.com".into()),
        Some(TrackerConfig::Gitlab(c)) => hosts.push(c.host.trim().to_lowercase()),
        Some(TrackerConfig::Jira(c)) => {
            if let Some(host) = Url::parse(c.url.trim())
                .ok()
                .and_then(|u| u.host_str().map(str::to_lowercase))
            {
                hosts.push(host);
            }
        }
        None => {}
    }
    hosts.retain(|h| !h.is_empty());
    hosts.sort();
    hosts.dedup();
    hosts
}

/// A URL, se for http(s) e o host dela estiver entre os do projeto; senão, o motivo.
pub fn link_permitido(hosts: &[String], link: &str) -> Result<Url, String> {
    let url = Url::parse(link.trim()).map_err(|_| "o endereço do link é inválido".to_string())?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("só se abre no navegador um link http ou https".into());
    }
    let host = url
        .host_str()
        .ok_or_else(|| "o link não tem host".to_string())?
        .to_lowercase();
    if !hosts.contains(&host) {
        return Err(format!(
            "o link é de {host}, que não é o rastreador nem um repositório deste projeto"
        ));
    }
    Ok(url)
}

/// Abre o link no navegador do sistema, se ele for de um host do projeto.
#[tauri::command]
pub async fn abrir_link_do_projeto_app(
    app: tauri::AppHandle,
    projeto: String,
    link: String,
) -> Result<(), String> {
    let dados = DiretorioDados::padrao()
        .map_err(|e| format!("diretório de dados do app inacessível: {e}"))?;
    let url = tauri::async_runtime::spawn_blocking(move || -> Result<Url, String> {
        let cadastro = cadastrado(&dados, &projeto)?;
        let codigo: Vec<String> = cadastro
            .repositorios
            .codigo
            .iter()
            .map(|r| r.host.clone())
            .collect();
        let hosts = hosts_do_projeto(&codigo, cadastro.repositorios.tracker.as_ref());
        link_permitido(&hosts, &link)
    })
    .await
    .map_err(|e| format!("falha interna: {e}"))??;
    app.opener()
        .open_url(url.as_str(), None::<&str>)
        .map_err(|e| format!("não foi possível abrir o link: {e}"))
}

#[cfg(test)]
mod testes {
    use super::*;
    use cba_repos::{GithubConfig, GitlabConfig, JiraConfig};

    fn hosts() -> Vec<String> {
        vec![
            "github.com".into(),
            "gitlab.exemplo.com".into(),
            "jira.exemplo.com".into(),
        ]
    }

    #[test]
    fn aceita_host_do_projeto_em_http_e_https() {
        assert!(link_permitido(&hosts(), "https://github.com/o/r/issues/41").is_ok());
        assert!(link_permitido(&hosts(), "http://jira.exemplo.com/browse/PROJ-1").is_ok());
        assert!(link_permitido(&hosts(), "  https://GitHub.com/o/r/pull/2  ").is_ok());
    }

    #[test]
    fn recusa_host_fora_do_projeto_e_truques_de_host() {
        for link in [
            "https://evil.com/github.com",
            "https://github.com.evil.com/x",
            "https://evil.com\\@github.com/x",
            "https://github.com@evil.com/x",
        ] {
            assert!(
                link_permitido(&hosts(), link).is_err(),
                "deveria recusar {link}"
            );
        }
    }

    #[test]
    fn recusa_esquema_que_nao_e_web() {
        assert!(link_permitido(&hosts(), "file:///etc/passwd").is_err());
        assert!(link_permitido(&hosts(), "javascript:alert(1)").is_err());
        assert!(link_permitido(&hosts(), "mailto:a@github.com").is_err());
    }

    #[test]
    fn recusa_texto_que_nao_e_url() {
        assert!(link_permitido(&hosts(), "github.com/o/r").is_err());
        assert!(link_permitido(&hosts(), "").is_err());
    }

    #[test]
    fn hosts_juntam_codigo_e_rastreador_de_cada_forma() {
        let codigo = vec!["GitLab.Exemplo.com".to_string()];
        let github = TrackerConfig::Github(GithubConfig {
            repo: "o/r".into(),
            ..Default::default()
        });
        assert_eq!(
            hosts_do_projeto(&codigo, Some(&github)),
            vec!["github.com", "gitlab.exemplo.com"]
        );

        let gitlab = TrackerConfig::Gitlab(GitlabConfig {
            host: "gitlab.empresa.com".into(),
            ..Default::default()
        });
        assert!(hosts_do_projeto(&[], Some(&gitlab)).contains(&"gitlab.empresa.com".to_string()));

        let jira = TrackerConfig::Jira(JiraConfig {
            url: "https://jira.empresa.com/".into(),
            ..Default::default()
        });
        assert_eq!(hosts_do_projeto(&[], Some(&jira)), vec!["jira.empresa.com"]);
        assert!(hosts_do_projeto(&[], None).is_empty());
    }
}
