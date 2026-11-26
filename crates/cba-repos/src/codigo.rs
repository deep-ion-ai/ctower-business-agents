//! Repositórios de código de um projeto (#178): um projeto pode ter vários, em
//! forjas diferentes (GitHub e GitLab).
//!
//! Cada repositório guarda de onde vem (forja, host e caminho), a URL de clone
//! como o analista colou, a pasta local (se houver), a branch e de onde vem o
//! token. O cadastro nunca guarda o token: só a origem dele (chaveiro do sistema,
//! global por host ou próprio do repositório, ou o comando que o imprime).

use crate::{ForgeConfig, ForgeKind, ParseEnumError};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;

str_enum!(OrigemToken, "origem_token", {
    Global => "global",
    Proprio => "proprio",
    Comando => "comando",
});

/// Host do GitHub na nuvem.
pub const HOST_GITHUB: &str = "github.com";

/// Um repositório de código do projeto.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RepositorioCodigo {
    pub forja: ForgeKind,
    /// Host sem esquema nem porta (`github.com`, `gitlab.example.com`).
    pub host: String,
    /// Caminho do repositório no host (`dono/repo`, `grupo/sub/app`).
    pub caminho: String,
    /// URL de clone como o analista colou (HTTPS ou SSH).
    pub url_clone: String,
    /// Pasta local do repositório. `None` (cadastro antigo, de uma forja só): o
    /// repositório vale para o projeto inteiro.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pasta: Option<PathBuf>,
    /// Branch do clone; vazio = a padrão do repositório.
    pub branch: String,
    /// De onde vem o token.
    pub token: OrigemToken,
    /// Comando que imprime o token, com `token = comando`. Texto do cadastro, não
    /// secret.
    pub comando_credencial: String,
    /// Conexão das Configurações que este repositório usa (#437). Com ela, o
    /// host e a credencial vêm da conexão; vazio = cadastro antigo.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub conexao: String,
}

impl Default for RepositorioCodigo {
    fn default() -> Self {
        RepositorioCodigo {
            forja: ForgeKind::Github,
            host: HOST_GITHUB.into(),
            caminho: String::new(),
            url_clone: String::new(),
            pasta: None,
            branch: String::new(),
            token: OrigemToken::Global,
            comando_credencial: String::new(),
            conexao: String::new(),
        }
    }
}

impl RepositorioCodigo {
    /// Deduz forja, host e caminho de uma URL de remoto/clone. `github.com` (e
    /// `ssh.github.com`) é GitHub; qualquer outro host, GitLab.
    pub fn da_url(url: &str) -> Option<RepositorioCodigo> {
        let (host, caminho) = host_e_caminho_original(url)?;
        let github =
            host.eq_ignore_ascii_case(HOST_GITHUB) || host.eq_ignore_ascii_case("ssh.github.com");
        Some(RepositorioCodigo {
            forja: if github {
                ForgeKind::Github
            } else {
                ForgeKind::Gitlab
            },
            host: if github { HOST_GITHUB.into() } else { host },
            caminho,
            url_clone: url.trim().to_string(),
            ..Default::default()
        })
    }

    /// URL de clone: a do cadastro ou, sem ela (cadastro antigo do GitHub), a
    /// HTTPS deduzida de host e caminho.
    pub fn url_efetiva(&self) -> String {
        let url = self.url_clone.trim();
        if url.is_empty() {
            format!(
                "https://{}/{}.git",
                self.host.trim(),
                self.caminho.trim().trim_matches('/')
            )
        } else {
            url.to_string()
        }
    }

    /// Identificador para comparar: `host/caminho` em minúsculas.
    pub fn chave(&self) -> String {
        format!(
            "{}/{}",
            self.host.trim().to_ascii_lowercase(),
            self.caminho.trim().trim_matches('/').to_ascii_lowercase()
        )
    }

    /// Nome curto para mensagens: `GitHub dono/repo`.
    pub fn rotulo(&self) -> String {
        format!("{} {}", self.forja.label(), self.caminho.trim())
    }

    /// Se a URL de um remoto do git aponta para este repositório.
    pub fn aponta_para(&self, url: &str) -> bool {
        let Some((host, caminho)) = host_e_caminho(url) else {
            return false;
        };
        let c = self.caminho.trim().trim_matches('/').to_ascii_lowercase();
        if c.is_empty() || caminho != c {
            return false;
        }
        let h = self.host.trim().to_ascii_lowercase();
        host == h
            || (self.forja == ForgeKind::Github && h == HOST_GITHUB && host == "ssh.github.com")
    }

    /// Link de `arquivo` (relativo à raiz do repositório) no `commit`, com as
    /// linhas `inicio..=fim` (1-indexadas). `None` sem caminho, host ou commit.
    pub fn link(&self, commit: &str, arquivo: &str, linhas: Option<(u32, u32)>) -> Option<String> {
        let commit = commit.trim();
        let host = self.host.trim().trim_matches('/');
        let repo = self.caminho.trim().trim_matches('/');
        if commit.is_empty() || arquivo.trim().is_empty() || host.is_empty() || repo.is_empty() {
            return None;
        }
        let (base, ancora) = match self.forja {
            ForgeKind::Github => (
                format!("https://{host}/{repo}/blob/{commit}"),
                linhas.map(|(i, f)| {
                    if f > i {
                        format!("#L{i}-L{f}")
                    } else {
                        format!("#L{i}")
                    }
                }),
            ),
            ForgeKind::Gitlab => (
                format!("https://{host}/{repo}/-/blob/{commit}"),
                linhas.map(|(i, f)| {
                    if f > i {
                        format!("#L{i}-{f}")
                    } else {
                        format!("#L{i}")
                    }
                }),
            ),
        };
        Some(format!(
            "{base}/{}{}",
            caminho_url(arquivo),
            ancora.unwrap_or_default()
        ))
    }

    /// Confere o cadastro sem rede. `n` é a posição (1-indexada) para a mensagem.
    pub fn validar(&self, n: usize) -> Result<(), String> {
        self.validar_como(&format!("repositório de código {n}"))
    }

    /// Como [`RepositorioCodigo::validar`], com `onde` no início das mensagens
    /// (`repositório da documentação`).
    pub fn validar_como(&self, onde: &str) -> Result<(), String> {
        match self.forja {
            ForgeKind::Github => {
                if !repo_github_ok(self.caminho.trim()) {
                    return Err(format!(
                        "{onde}: repositório GitHub inválido: {:?} (use dono/repositório)",
                        self.caminho
                    ));
                }
                if self.host.trim().is_empty() {
                    return Err(format!("{onde}: informe o host do GitHub (github.com)"));
                }
            }
            ForgeKind::Gitlab => {
                if self.host.trim().is_empty()
                    || !self.caminho.trim().trim_matches('/').contains('/')
                {
                    return Err(format!(
                        "{onde}: repositório do GitLab inválido: host {:?}, caminho {:?}",
                        self.host, self.caminho
                    ));
                }
            }
        }
        if self.host.contains('/') || self.host.contains(':') {
            return Err(format!(
                "{onde}: host inválido: {:?} (sem esquema nem porta, como gitlab.example.com)",
                self.host
            ));
        }
        if self.token == OrigemToken::Comando && self.comando_credencial.trim().is_empty() {
            return Err(format!(
                "{onde}: informe o comando que imprime o token (ex.: pass show {} | head -1)",
                if self.forja == ForgeKind::Github {
                    "github"
                } else {
                    "gitlab"
                }
            ));
        }
        if let Some(p) = &self.pasta
            && !p.is_absolute()
        {
            return Err(format!(
                "{onde}: informe o caminho completo da pasta: {}",
                p.display()
            ));
        }
        Ok(())
    }
}

/// Converte a forja única do cadastro antigo num repositório de código, sem
/// perda: o GitHub seguia o `gh auth` (token global do host); o GitLab, o
/// comando de credencial.
impl From<ForgeConfig> for RepositorioCodigo {
    fn from(f: ForgeConfig) -> Self {
        match f {
            ForgeConfig::Github(g) => RepositorioCodigo {
                forja: ForgeKind::Github,
                host: HOST_GITHUB.into(),
                caminho: g.repo.trim().to_string(),
                url_clone: String::new(),
                token: OrigemToken::Global,
                ..Default::default()
            },
            ForgeConfig::Gitlab(g) => RepositorioCodigo {
                forja: ForgeKind::Gitlab,
                host: g.host,
                caminho: g.path,
                url_clone: g.clone_url,
                token: OrigemToken::Comando,
                comando_credencial: g.credential_cmd,
                ..Default::default()
            },
        }
    }
}

/// O repositório que contém `absoluto` (a pasta mais funda que o contém) e o
/// caminho do arquivo relativo à pasta dele, com `/`. Sem pasta que contenha o
/// arquivo, vale o primeiro repositório sem pasta (cadastro antigo), com o
/// caminho `relativo_ao_projeto`.
pub fn repositorio_do_arquivo<'a>(
    repos: &'a [RepositorioCodigo],
    absoluto: Option<&Path>,
    relativo_ao_projeto: &str,
) -> Option<(&'a RepositorioCodigo, String)> {
    let por_pasta = absoluto.and_then(|abs| {
        repos
            .iter()
            .filter_map(|r| {
                let pasta = r.pasta.as_deref()?;
                let resto = abs.strip_prefix(pasta).ok()?;
                let partes: Option<Vec<&str>> =
                    resto.components().map(|c| c.as_os_str().to_str()).collect();
                Some((pasta.components().count(), r, partes?.join("/")))
            })
            .max_by_key(|(n, _, _)| *n)
            .map(|(_, r, c)| (r, c))
    });
    por_pasta.or_else(|| {
        repos
            .iter()
            .find(|r| r.pasta.is_none())
            .map(|r| (r, relativo_ao_projeto.replace('\\', "/")))
    })
}

/// Codifica o caminho para a URL, mantendo as barras.
pub(crate) fn caminho_url(p: &str) -> String {
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

pub(crate) fn repo_github_ok(r: &str) -> bool {
    let mut it = r.split('/');
    let ok = |s: &str| {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    };
    matches!((it.next(), it.next(), it.next()), (Some(a), Some(b), None) if ok(a) && ok(b))
}

/// Host (sem usuário nem porta) e caminho (sem `.git` nem barras nas pontas) de
/// uma URL de remoto do git, como escritos: `https://host/a/b.git`,
/// `ssh://git@host:22/a/b`, `git@host:a/b.git`.
fn host_e_caminho_original(url: &str) -> Option<(String, String)> {
    let url = url.trim();
    let (autoridade, caminho) = if let Some((_, resto)) = url.split_once("://") {
        resto.split_once('/')?
    } else {
        // Forma scp do ssh: [usuário@]host:caminho.
        let (antes, depois) = url.split_once(':')?;
        // `C:/pasta` é uma unidade do Windows, não um host (o git pensa igual).
        if antes.contains('/') || antes.len() == 1 {
            return None;
        }
        (antes, depois)
    };
    let host = autoridade.rsplit_once('@').map_or(autoridade, |(_, h)| h);
    let host = host.split_once(':').map_or(host, |(h, _)| h);
    let caminho = caminho.trim_matches('/');
    let caminho = caminho.strip_suffix(".git").unwrap_or(caminho);
    let caminho = caminho.trim_matches('/');
    if host.is_empty() || caminho.is_empty() {
        return None;
    }
    Some((host.to_string(), caminho.to_string()))
}

/// Como [`host_e_caminho_original`], em minúsculas, para comparar.
pub fn host_e_caminho(url: &str) -> Option<(String, String)> {
    host_e_caminho_original(url).map(|(h, c)| (h.to_ascii_lowercase(), c.to_ascii_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GithubConfig, GitlabConfig};

    fn gh(caminho: &str) -> RepositorioCodigo {
        RepositorioCodigo {
            caminho: caminho.into(),
            ..Default::default()
        }
    }

    fn gl(caminho: &str) -> RepositorioCodigo {
        RepositorioCodigo {
            forja: ForgeKind::Gitlab,
            host: "gitlab.example.com".into(),
            caminho: caminho.into(),
            ..Default::default()
        }
    }

    #[test]
    fn deduz_forja_host_e_caminho_da_url() {
        let r = RepositorioCodigo::da_url("git@github.com:Deep-Ion/App.git").unwrap();
        assert_eq!(
            (r.forja, r.host.as_str(), r.caminho.as_str()),
            (ForgeKind::Github, "github.com", "Deep-Ion/App")
        );
        assert_eq!(r.url_clone, "git@github.com:Deep-Ion/App.git");
        let r =
            RepositorioCodigo::da_url("https://user@gitlab.example.com/g/sub/app.git/").unwrap();
        assert_eq!(
            (r.forja, r.host.as_str(), r.caminho.as_str()),
            (ForgeKind::Gitlab, "gitlab.example.com", "g/sub/app")
        );
        let r = RepositorioCodigo::da_url("ssh://git@ssh.github.com:443/o/r").unwrap();
        assert_eq!(
            (r.forja, r.host.as_str()),
            (ForgeKind::Github, "github.com")
        );
        for ruim in ["", "app", "https://host", "/local/path", "C:/x/y"] {
            assert!(RepositorioCodigo::da_url(ruim).is_none(), "{ruim}");
        }
    }

    #[test]
    fn aponta_para_remotos_em_varias_formas() {
        let r = gh("o/r");
        for url in [
            "https://github.com/o/r.git",
            "git@github.com:O/R.git",
            "ssh://git@ssh.github.com:443/o/r",
        ] {
            assert!(r.aponta_para(url), "{url}");
        }
        assert!(!r.aponta_para("https://github.com/o/outro.git"));
        assert!(!r.aponta_para("https://gitlab.example.com/o/r.git"));
        let g = gl("g/p/app");
        assert!(g.aponta_para("git@gitlab.example.com:g/p/app.git"));
        assert!(!g.aponta_para("git@gitlab.example.com:g/p/outro.git"));
        assert!(!gh("").aponta_para("https://github.com/o/r.git"));
    }

    #[test]
    fn links_do_github_e_do_gitlab() {
        assert_eq!(
            gh("o/r").link("abc", "src/a b.rs", Some((3, 9))).unwrap(),
            "https://github.com/o/r/blob/abc/src/a%20b.rs#L3-L9"
        );
        assert_eq!(
            gh("o/r").link("abc", "a.rs", Some((3, 3))).unwrap(),
            "https://github.com/o/r/blob/abc/a.rs#L3"
        );
        assert_eq!(
            gl("g/p/app")
                .link("abc", "db/01.sql", Some((1, 4)))
                .unwrap(),
            "https://gitlab.example.com/g/p/app/-/blob/abc/db/01.sql#L1-4"
        );
        assert!(gh("o/r").link("", "a", None).is_none());
        assert!(gh("").link("c", "a", None).is_none());
    }

    #[test]
    fn migra_a_forja_antiga_sem_perda() {
        let r: RepositorioCodigo = ForgeConfig::Github(GithubConfig {
            repo: "o/r".into(),
            ..Default::default()
        })
        .into();
        assert_eq!(r, gh("o/r"));
        let r: RepositorioCodigo = ForgeConfig::Gitlab(GitlabConfig {
            host: "gitlab.example.com".into(),
            path: "g/p/app".into(),
            clone_url: "git@gitlab.example.com:g/p/app.git".into(),
            credential_cmd: "pass show gitlab | head -1".into(),
            ..Default::default()
        })
        .into();
        assert_eq!(r.forja, ForgeKind::Gitlab);
        assert_eq!(r.token, OrigemToken::Comando);
        assert_eq!(r.comando_credencial, "pass show gitlab | head -1");
        assert_eq!(r.url_clone, "git@gitlab.example.com:g/p/app.git");
        assert_eq!(r.pasta, None);
    }

    #[test]
    fn valida_cada_repositorio() {
        assert!(gh("o/r").validar(1).is_ok());
        assert!(gl("g/app").validar(1).is_ok());
        for r in ["", "app", "a/b/c", "a b/c"] {
            assert!(gh(r).validar(1).is_err(), "{r:?}");
        }
        assert!(gl("app").validar(2).unwrap_err().contains("código 2"));
        let sem_host = RepositorioCodigo {
            host: "".into(),
            ..gl("g/app")
        };
        assert!(sem_host.validar(1).is_err());
        let com_esquema = RepositorioCodigo {
            host: "https://gitlab.example.com".into(),
            ..gl("g/app")
        };
        assert!(com_esquema.validar(1).is_err());
        let comando_vazio = RepositorioCodigo {
            token: OrigemToken::Comando,
            ..gl("g/app")
        };
        assert!(comando_vazio.validar(1).is_err());
        let relativa = RepositorioCodigo {
            pasta: Some("rel/app".into()),
            ..gh("o/r")
        };
        assert!(relativa.validar(1).is_err());
    }

    #[test]
    fn escolhe_o_repositorio_pela_pasta_mais_funda() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        let repos = vec![
            RepositorioCodigo {
                pasta: Some(base.join("web")),
                ..gh("o/web")
            },
            RepositorioCodigo {
                pasta: Some(base.join("web/legado")),
                ..gl("g/legado")
            },
        ];
        let (r, c) =
            repositorio_do_arquivo(&repos, Some(&base.join("web/legado/db/a.sql")), "x").unwrap();
        assert_eq!((r.caminho.as_str(), c.as_str()), ("g/legado", "db/a.sql"));
        let (r, c) = repositorio_do_arquivo(&repos, Some(&base.join("web/src/a.ts")), "x").unwrap();
        assert_eq!((r.caminho.as_str(), c.as_str()), ("o/web", "src/a.ts"));
        assert!(repositorio_do_arquivo(&repos, Some(&base.join("fora/a")), "a").is_none());
        // Cadastro antigo: sem pasta, vale o caminho no projeto.
        let antigo = vec![gh("o/r")];
        let (r, c) = repositorio_do_arquivo(&antigo, None, "db\\a.sql").unwrap();
        assert_eq!((r.caminho.as_str(), c.as_str()), ("o/r", "db/a.sql"));
    }
}
