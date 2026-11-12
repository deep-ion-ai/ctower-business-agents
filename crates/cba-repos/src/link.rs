//! Links para o forge: arquivo, linhas e commit de uma evidência.

use crate::ForgeConfig;

/// Codifica o caminho para a URL, mantendo as barras.
fn caminho_url(p: &str) -> String {
    let mut out = String::with_capacity(p.len());
    for b in p.replace('\\', "/").trim_start_matches('/').bytes() {
        if b.is_ascii_alphanumeric() || b"/._-~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Link da evidência no commit: `arquivo`, `inicio..=fim` (1-indexadas).
/// `None` se a forja não tem o dado necessário (repositório ou commit vazio).
pub fn link_evidencia(
    forge: &ForgeConfig,
    commit: &str,
    arquivo: &str,
    linhas: Option<(u32, u32)>,
) -> Option<String> {
    let commit = commit.trim();
    if commit.is_empty() || arquivo.trim().is_empty() {
        return None;
    }
    let (base, ancora) = match forge {
        ForgeConfig::Github(g) => {
            let repo = g.repo.trim().trim_matches('/');
            if repo.is_empty() {
                return None;
            }
            let a = linhas.map(|(i, f)| {
                if f > i {
                    format!("#L{i}-L{f}")
                } else {
                    format!("#L{i}")
                }
            });
            (format!("https://github.com/{repo}/blob/{commit}"), a)
        }
        ForgeConfig::Gitlab(g) => {
            let (host, path) = (g.host.trim(), g.path.trim().trim_matches('/'));
            if host.is_empty() || path.is_empty() {
                return None;
            }
            let a = linhas.map(|(i, f)| {
                if f > i {
                    format!("#L{i}-{f}")
                } else {
                    format!("#L{i}")
                }
            });
            (format!("https://{host}/{path}/-/blob/{commit}"), a)
        }
    };
    Some(format!(
        "{base}/{}{}",
        caminho_url(arquivo),
        ancora.unwrap_or_default()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GithubConfig, GitlabConfig};

    #[test]
    fn github_e_gitlab() {
        let gh = ForgeConfig::Github(GithubConfig { repo: "o/r".into() });
        assert_eq!(
            link_evidencia(&gh, "abc123", "src/a b.rs", Some((3, 9))).unwrap(),
            "https://github.com/o/r/blob/abc123/src/a%20b.rs#L3-L9"
        );
        assert_eq!(
            link_evidencia(&gh, "abc123", "a.rs", Some((3, 3))).unwrap(),
            "https://github.com/o/r/blob/abc123/a.rs#L3"
        );
        assert_eq!(
            link_evidencia(&gh, "abc123", "a.rs", None).unwrap(),
            "https://github.com/o/r/blob/abc123/a.rs"
        );
        let gl = ForgeConfig::Gitlab(GitlabConfig {
            host: "gitlab.example.com".into(),
            path: "g/p/app".into(),
            ..Default::default()
        });
        assert_eq!(
            link_evidencia(&gl, "abc123", "db/01.sql", Some((1, 4))).unwrap(),
            "https://gitlab.example.com/g/p/app/-/blob/abc123/db/01.sql#L1-4"
        );
        assert!(link_evidencia(&gl, "", "a", None).is_none());
        assert!(
            link_evidencia(
                &ForgeConfig::Github(GithubConfig::default()),
                "c",
                "a",
                None
            )
            .is_none()
        );
    }
}
