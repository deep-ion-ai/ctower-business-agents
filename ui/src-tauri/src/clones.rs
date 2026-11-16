//! Clone inicial dos repositórios de código e tokens no chaveiro do sistema
//! (#179): pasta base dos clones, tokens globais por host e próprios de um
//! repositório, "Testar acesso" e o clone com progresso por repositório.
//!
//! O token chega da tela só para ser gravado no chaveiro ou usado no teste de
//! acesso; não volta para a tela, não vai para o cadastro nem para o log.

use std::path::{Path, PathBuf};

use cba_core::dados::DiretorioDados;
use cba_repos::chaveiro::{
    Chaveiro, conta_comando_global, conta_global, conta_propria, token_do_repositorio,
};
use cba_repos::clone::{Progresso, clonar, testar_acesso};
use cba_repos::demanda::Ferramentas;
use cba_repos::{RepositorioCodigo, Secret};
use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;

use crate::primeiro_uso::Preferencias;

/// Pasta base sugerida para os clones: `Documentos/ctower-business-repos`.
pub fn pasta_clones_sugerida() -> Option<PathBuf> {
    crate::primeiro_uso::base_sugerida()
        .and_then(|b| b.parent().map(|d| d.join("ctower-business-repos")))
}

/// Pasta base dos clones: a das preferências ou a sugerida.
pub fn pasta_clones(dados: &DiretorioDados) -> Result<Option<PathBuf>, String> {
    Ok(Preferencias::ler(dados)?
        .pasta_clones
        .or_else(pasta_clones_sugerida))
}

/// Grava a pasta base dos clones (caminho completo).
pub fn definir_pasta_clones(dados: &DiretorioDados, pasta: &str) -> Result<PathBuf, String> {
    let p = PathBuf::from(pasta.trim());
    if !p.is_absolute() {
        return Err(format!(
            "informe o caminho completo da pasta dos clones: {}",
            p.display()
        ));
    }
    let mut prefs = Preferencias::ler(dados)?;
    prefs.pasta_clones = Some(p.clone());
    prefs.gravar(dados)?;
    Ok(p)
}

/// Onde vale um token: o host inteiro (`caminho` vazio) ou só um repositório.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlvoToken {
    pub host: String,
    #[serde(default)]
    pub caminho: Option<String>,
}

impl AlvoToken {
    pub(crate) fn conta(&self) -> Result<String, String> {
        let host = self.host.trim();
        if host.is_empty() || host.contains('/') || host.contains(':') {
            return Err(format!("host inválido: {:?}", self.host));
        }
        Ok(match self.caminho.as_deref().map(str::trim) {
            Some(c) if !c.is_empty() => conta_propria(&RepositorioCodigo {
                host: host.into(),
                caminho: c.into(),
                ..Default::default()
            }),
            _ => conta_global(host),
        })
    }
}

pub fn salvar_token(chaveiro: &dyn Chaveiro, alvo: &AlvoToken, token: &str) -> Result<(), String> {
    let token = token.trim();
    if token.is_empty() {
        return Err("informe o token".into());
    }
    let conta = alvo.conta()?;
    chaveiro.gravar(&conta, &Secret::new(token))?;
    // O token guardado substitui o comando do host (#193).
    if conta == conta_global(&alvo.host) {
        chaveiro.apagar(&conta_comando_global(&alvo.host))?;
    }
    Ok(())
}

pub fn apagar_token(chaveiro: &dyn Chaveiro, alvo: &AlvoToken) -> Result<(), String> {
    chaveiro.apagar(&alvo.conta()?)
}

/// O token do alvo mudou: o último teste da seção Acessos deixa de valer.
fn esquecer_teste(alvo: &AlvoToken) -> Result<(), String> {
    crate::acessos::esquecer_teste(&dados()?, &alvo.conta()?)
}

/// Para cada alvo, se há token guardado (nunca o token).
pub fn tokens_guardados(chaveiro: &dyn Chaveiro, alvos: &[AlvoToken]) -> Result<Vec<bool>, String> {
    alvos
        .iter()
        .map(|a| Ok(chaveiro.ler(&a.conta()?)?.is_some()))
        .collect()
}

/// Testa o acesso de leitura. Com `token` (digitado e ainda não gravado), usa
/// ele; senão, o do repositório (próprio, global do host ou comando).
pub fn testar(
    f: &Ferramentas,
    repo: &RepositorioCodigo,
    token: Option<&str>,
) -> Result<(), String> {
    let token = match token.map(str::trim).filter(|t| !t.is_empty()) {
        Some(t) => Some(Secret::new(t)),
        None => token_do_repositorio(repo, f.chaveiro.as_ref()).map_err(|e| e.to_string())?,
    };
    testar_acesso(&f.git, repo, token.as_ref())
}

/// "Testar conexão" do Jira no cadastro: os passos da prontidão, com o token
/// digitado (ainda não gravado) ou o do chaveiro.
pub fn testar_jira(
    f: &Ferramentas,
    jira: &cba_repos::JiraConfig,
    token: Option<&str>,
) -> cba_repos::prontidao::Integracao {
    match token.map(str::trim).filter(|t| !t.is_empty()) {
        Some(t) => {
            let memoria = cba_repos::chaveiro::ChaveiroMemoria::default();
            let _ = memoria.gravar(&conta_global(&jira.host()), &Secret::new(t));
            let f = Ferramentas {
                chaveiro: std::sync::Arc::new(memoria),
                ..f.clone()
            };
            cba_repos::prontidao::prontidao_jira(jira, &f)
        }
        None => cba_repos::prontidao::prontidao_jira(jira, f),
    }
}

/// Evento do clone para a tela, por repositório (`indice` na lista pedida).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "tipo")]
pub enum EventoClone {
    Inicio {
        indice: usize,
    },
    Progresso {
        indice: usize,
        etapa: String,
        percentual: Option<u8>,
    },
    Concluido {
        indice: usize,
        pasta: PathBuf,
    },
    Falhou {
        indice: usize,
        erro: String,
    },
}

/// Resultado de um repositório: a pasta do clone (o repositório com `pasta`
/// preenchida) ou o erro.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultadoClone {
    pub repositorio: RepositorioCodigo,
    pub erro: Option<String>,
}

/// Clona cada repositório em `base/<nome>`, um de cada vez; a falha de um não
/// impede os outros.
pub fn clonar_todos(
    f: &Ferramentas,
    repos: &[RepositorioCodigo],
    base: &Path,
    avisar: &mut dyn FnMut(EventoClone),
) -> Vec<ResultadoClone> {
    let mut nomes = std::collections::HashSet::new();
    repos
        .iter()
        .enumerate()
        .map(|(indice, repo)| {
            avisar(EventoClone::Inicio { indice });
            let feito = (|| {
                repo.validar(indice + 1)?;
                if !nomes.insert(cba_repos::clone::nome_da_pasta(repo).to_lowercase()) {
                    return Err(format!(
                        "dois repositórios iriam para a mesma pasta ({}): clone-os em pastas \
                         base diferentes",
                        cba_repos::clone::nome_da_pasta(repo)
                    ));
                }
                let token =
                    token_do_repositorio(repo, f.chaveiro.as_ref()).map_err(|e| e.to_string())?;
                clonar(&f.git, repo, base, token.as_ref(), &mut |p: Progresso| {
                    avisar(EventoClone::Progresso {
                        indice,
                        etapa: p.etapa,
                        percentual: p.percentual,
                    })
                })
            })();
            match feito {
                Ok(pasta) => {
                    tracing::info!(repositorio = %repo.rotulo(), "clone concluído");
                    avisar(EventoClone::Concluido {
                        indice,
                        pasta: pasta.clone(),
                    });
                    ResultadoClone {
                        repositorio: RepositorioCodigo {
                            pasta: Some(pasta),
                            ..repo.clone()
                        },
                        erro: None,
                    }
                }
                Err(erro) => {
                    tracing::warn!(repositorio = %repo.rotulo(), "clone falhou");
                    avisar(EventoClone::Falhou {
                        indice,
                        erro: erro.clone(),
                    });
                    ResultadoClone {
                        repositorio: repo.clone(),
                        erro: Some(erro),
                    }
                }
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Comandos

fn dados() -> Result<DiretorioDados, String> {
    DiretorioDados::padrao().map_err(|e| format!("diretório de dados do app inacessível: {e}"))
}

async fn bloqueante<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| format!("falha interna: {e}"))?
}

#[tauri::command]
pub async fn pasta_clones_app() -> Result<Option<PathBuf>, String> {
    let dados = dados()?;
    bloqueante(move || pasta_clones(&dados)).await
}

#[tauri::command]
pub async fn definir_pasta_clones_app(pasta: String) -> Result<PathBuf, String> {
    let dados = dados()?;
    bloqueante(move || definir_pasta_clones(&dados, &pasta)).await
}

#[tauri::command]
pub async fn salvar_token_app(alvo: AlvoToken, token: String) -> Result<(), String> {
    bloqueante(move || {
        salvar_token(Ferramentas::default().chaveiro.as_ref(), &alvo, &token)?;
        esquecer_teste(&alvo)
    })
    .await
}

#[tauri::command]
pub async fn apagar_token_app(alvo: AlvoToken) -> Result<(), String> {
    bloqueante(move || {
        apagar_token(Ferramentas::default().chaveiro.as_ref(), &alvo)?;
        esquecer_teste(&alvo)
    })
    .await
}

#[tauri::command]
pub async fn tokens_guardados_app(alvos: Vec<AlvoToken>) -> Result<Vec<bool>, String> {
    bloqueante(move || tokens_guardados(Ferramentas::default().chaveiro.as_ref(), &alvos)).await
}

#[tauri::command]
pub async fn testar_acesso_app(
    repositorio: RepositorioCodigo,
    token: Option<String>,
) -> Result<(), String> {
    bloqueante(move || testar(&Ferramentas::default(), &repositorio, token.as_deref())).await
}

#[tauri::command]
pub async fn testar_jira_app(
    jira: cba_repos::JiraConfig,
    token: Option<String>,
) -> Result<cba_repos::prontidao::Integracao, String> {
    bloqueante(move || {
        Ok(testar_jira(
            &Ferramentas::default(),
            &jira,
            token.as_deref(),
        ))
    })
    .await
}

#[tauri::command]
pub async fn clonar_repositorios_app(
    repositorios: Vec<RepositorioCodigo>,
    pasta_base: String,
    canal: Channel<EventoClone>,
) -> Result<Vec<ResultadoClone>, String> {
    let base = PathBuf::from(pasta_base.trim());
    if !base.is_absolute() {
        return Err(format!(
            "informe o caminho completo da pasta dos clones: {}",
            base.display()
        ));
    }
    bloqueante(move || {
        Ok(clonar_todos(
            &Ferramentas::default(),
            &repositorios,
            &base,
            &mut |e| {
                let _ = canal.send(e);
            },
        ))
    })
    .await
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use cba_repos::chaveiro::ChaveiroMemoria;
    use cba_repos::{ForgeKind, OrigemToken};
    use std::process::Command;
    use std::sync::Arc;

    fn git(dir: &Path, args: &[&str]) {
        let s = Command::new("git")
            .args([
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .current_dir(dir)
            .status()
            .unwrap();
        assert!(s.success(), "git {args:?}");
    }

    fn origem(dir: &Path, nome: &str) -> String {
        let o = dir.join("origens").join(nome);
        std::fs::create_dir_all(&o).unwrap();
        git(&o, &["init", "-q"]);
        std::fs::write(o.join("leia-me.md"), nome).unwrap();
        git(&o, &["add", "-A"]);
        git(&o, &["commit", "-q", "-m", "base"]);
        format!("file://{}", o.display())
    }

    #[test]
    fn tokens_por_host_e_por_repositorio_sem_devolver_o_valor() {
        let c = ChaveiroMemoria::default();
        let global = AlvoToken {
            host: "gitlab.example.com".into(),
            caminho: None,
        };
        let proprio = AlvoToken {
            host: "github.com".into(),
            caminho: Some("o/web".into()),
        };
        salvar_token(&c, &global, " glpat-1 ").unwrap();
        assert_eq!(
            tokens_guardados(&c, &[global.clone(), proprio.clone()]).unwrap(),
            [true, false]
        );
        salvar_token(&c, &proprio, "ghp-2").unwrap();
        let repo = RepositorioCodigo {
            caminho: "O/Web".into(),
            token: OrigemToken::Proprio,
            ..Default::default()
        };
        assert_eq!(
            token_do_repositorio(&repo, &c).unwrap().unwrap().expose(),
            "ghp-2"
        );
        apagar_token(&c, &proprio).unwrap();
        assert_eq!(
            tokens_guardados(&c, std::slice::from_ref(&proprio)).unwrap(),
            [false]
        );
        assert!(salvar_token(&c, &global, "  ").is_err());
        let ruim = AlvoToken {
            host: "https://x".into(),
            caminho: None,
        };
        assert!(salvar_token(&c, &ruim, "t").is_err());
    }

    #[test]
    fn clona_gitlab_com_token_global_e_github_com_token_proprio() {
        let d = tempfile::tempdir().unwrap();
        let raiz = dunce::canonicalize(d.path()).unwrap();
        let chaveiro = Arc::new(ChaveiroMemoria::default());
        chaveiro
            .gravar(
                &conta_global("gitlab.example.com"),
                &Secret::new("glpat-global"),
            )
            .unwrap();
        let web = RepositorioCodigo {
            caminho: "o/web".into(),
            url_clone: origem(&raiz, "web"),
            token: OrigemToken::Proprio,
            ..Default::default()
        };
        chaveiro
            .gravar(&conta_propria(&web), &Secret::new("ghp-proprio"))
            .unwrap();
        let legado = RepositorioCodigo {
            forja: ForgeKind::Gitlab,
            host: "gitlab.example.com".into(),
            caminho: "g/legado".into(),
            url_clone: origem(&raiz, "legado"),
            ..Default::default()
        };
        let quebrado = RepositorioCodigo {
            caminho: "o/nada".into(),
            url_clone: format!("file://{}/nada", raiz.display()),
            ..Default::default()
        };
        let f = Ferramentas {
            chaveiro,
            ..Default::default()
        };
        let mut eventos = Vec::new();
        let base = raiz.join("clones");
        let r = clonar_todos(&f, &[legado, web, quebrado], &base, &mut |e| {
            eventos.push(e)
        });
        assert_eq!(
            r[0].repositorio.pasta.as_deref(),
            Some(base.join("legado").as_path())
        );
        assert_eq!(
            r[1].repositorio.pasta.as_deref(),
            Some(base.join("web").as_path())
        );
        assert!(r[2].erro.is_some() && r[2].repositorio.pasta.is_none());
        assert!(eventos.contains(&EventoClone::Inicio { indice: 2 }));
        assert!(
            eventos
                .iter()
                .any(|e| matches!(e, EventoClone::Falhou { indice: 2, .. }))
        );
        // Nenhum token no .git/config, nos eventos nem nos resultados.
        for nome in ["legado", "web"] {
            let cfg = std::fs::read_to_string(base.join(nome).join(".git/config")).unwrap();
            assert!(!cfg.contains("glpat") && !cfg.contains("ghp-"), "{cfg}");
        }
        let tudo = format!("{eventos:?}{r:?}{}", serde_json::to_string(&r).unwrap());
        assert!(!tudo.contains("glpat") && !tudo.contains("ghp-"), "{tudo}");
    }

    #[test]
    fn testa_o_jira_com_o_token_digitado_ou_o_do_chaveiro() {
        use std::os::unix::fs::PermissionsExt;
        let d = tempfile::tempdir().unwrap();
        let jira_bin = d.path().join("jira");
        std::fs::write(
            &jira_bin,
            "#!/bin/sh\ncase \"$1\" in version) echo 1.7;; me) [ \"$JIRA_API_TOKEN\" = tok ] && echo ana || exit 1;; serverinfo) echo 9;; esac\n",
        )
        .unwrap();
        std::fs::set_permissions(&jira_bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        let yml = d.path().join("jira.yml");
        std::fs::write(&yml, "server: https://jira.example.com\n").unwrap();
        let jira = cba_repos::JiraConfig {
            url: "https://jira.example.com".into(),
            project_key: "PROJ".into(),
            config_file: yml.display().to_string(),
            ..Default::default()
        };
        let chaveiro = Arc::new(ChaveiroMemoria::default());
        let f = Ferramentas {
            jira: jira_bin,
            chaveiro: chaveiro.clone(),
            ..Default::default()
        };
        assert!(!testar_jira(&f, &jira, None).pronta());
        let i = testar_jira(&f, &jira, Some("tok"));
        assert!(i.pronta(), "{i:?}");
        assert!(!format!("{i:?}").contains("tok\""));
        // O token digitado não foi gravado no chaveiro de verdade.
        assert!(!testar_jira(&f, &jira, None).pronta());
        chaveiro
            .gravar(&conta_global("jira.example.com"), &Secret::new("tok"))
            .unwrap();
        assert!(testar_jira(&f, &jira, None).pronta());
    }

    #[test]
    fn pasta_dos_clones_nas_preferencias() {
        let d = tempfile::tempdir().unwrap();
        let dados = DiretorioDados::em(d.path().join("dados"));
        assert_eq!(pasta_clones(&dados).unwrap(), pasta_clones_sugerida());
        assert!(definir_pasta_clones(&dados, "relativa").is_err());
        let p = d.path().join("clones");
        definir_pasta_clones(&dados, p.to_str().unwrap()).unwrap();
        assert_eq!(pasta_clones(&dados).unwrap(), Some(p));
    }
}
