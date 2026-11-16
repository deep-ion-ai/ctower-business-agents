//! Seção "Acessos" das configurações (#193): os tokens (PAT) do GitHub e do
//! GitLab guardados no chaveiro do sistema, globais por host ou próprios de um
//! repositório, o comando que imprime o token no lugar do token guardado, e o
//! resultado do último "Testar" (com a validade, quando a forja informa).
//!
//! Nada aqui devolve o token para a tela: só se há token, o comando (que não é
//! segredo) e o resultado do teste, cujas mensagens passam pelo `cite` do
//! `cba-repos` (o valor vira `***`). Os hosts acrescentados à mão e os testes
//! ficam nas preferências do app; o token e o comando, no chaveiro.

use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use cba_core::dados::DiretorioDados;
use cba_repos::acessos::{ErroVerificacao, verificar_token_host};
use cba_repos::chaveiro::{
    Chaveiro, comando_global, conta_comando_global, conta_global, token_do_repositorio,
    token_global,
};
use cba_repos::clone::testar_acesso;
use cba_repos::demanda::Ferramentas;
use cba_repos::{ForgeKind, RepositorioCodigo, Secret};
use serde::{Deserialize, Serialize};

use crate::clones::AlvoToken;
use crate::primeiro_uso::Preferencias;

/// Host acrescentado em "+ Adicionar host" (os dos projetos a tela já conhece).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostCadastrado {
    pub host: String,
    pub forja: ForgeKind,
}

/// Resultado do último "Testar" de um token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TesteToken {
    pub ok: bool,
    /// Quando (milissegundos desde 1970, UTC).
    pub quando: u64,
    /// Por que falhou; sem o token.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mensagem: Option<String>,
    /// Dono do token, quando a forja informa.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usuario: Option<String>,
    /// Validade (`AAAA-MM-DD`), quando a forja informa.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expira: Option<String>,
}

/// O que as preferências guardam dos acessos.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcessosSalvos {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hosts: Vec<HostCadastrado>,
    /// Conta no chaveiro (`host:…`, `repo:…`) → último teste.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub testes: BTreeMap<String, TesteToken>,
}

impl AcessosSalvos {
    pub fn vazio(&self) -> bool {
        self.hosts.is_empty() && self.testes.is_empty()
    }
}

/// Situação do token de um alvo, para a tela: nunca o token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EstadoToken {
    /// Há token guardado no chaveiro.
    pub guardado: bool,
    /// O comando que imprime o token global do host (só no alvo de host).
    pub comando: Option<String>,
    pub teste: Option<TesteToken>,
}

fn agora() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn host_valido(host: &str) -> Result<String, String> {
    AlvoToken {
        host: host.into(),
        caminho: None,
    }
    .conta()?;
    Ok(host.trim().to_ascii_lowercase())
}

fn alterar(dados: &DiretorioDados, f: impl FnOnce(&mut AcessosSalvos)) -> Result<(), String> {
    let mut prefs = Preferencias::ler(dados)?;
    f(&mut prefs.acessos);
    prefs.gravar(dados)
}

/// Hosts acrescentados à mão.
pub fn hosts_cadastrados(dados: &DiretorioDados) -> Result<Vec<HostCadastrado>, String> {
    Ok(Preferencias::ler(dados)?.acessos.hosts)
}

/// Acrescenta um host à lista (sem token ainda); repetido, troca a forja.
pub fn adicionar_host(
    dados: &DiretorioDados,
    host: &str,
    forja: ForgeKind,
) -> Result<HostCadastrado, String> {
    let host = host_valido(host)?;
    let novo = HostCadastrado { host, forja };
    alterar(dados, |a| {
        a.hosts.retain(|h| h.host != novo.host);
        a.hosts.push(novo.clone());
    })?;
    Ok(novo)
}

/// Remove o acesso do host: o token e o comando do chaveiro, o último teste e
/// o host da lista dos acrescentados. Os repositórios com token próprio não
/// mudam.
pub fn remover_host(
    dados: &DiretorioDados,
    chaveiro: &dyn Chaveiro,
    host: &str,
) -> Result<(), String> {
    let host = host_valido(host)?;
    chaveiro.apagar(&conta_global(&host))?;
    chaveiro.apagar(&conta_comando_global(&host))?;
    alterar(dados, |a| {
        a.hosts.retain(|h| h.host != host);
        a.testes.remove(&conta_global(&host));
    })?;
    tracing::info!(host = %host, "acesso do host removido");
    Ok(())
}

/// Usa um comando que imprime o token global do host, no lugar do token
/// guardado (que é apagado). O último teste deixa de valer.
pub fn definir_comando_host(
    dados: &DiretorioDados,
    chaveiro: &dyn Chaveiro,
    host: &str,
    comando: &str,
) -> Result<(), String> {
    let host = host_valido(host)?;
    let comando = comando.trim();
    if comando.is_empty() {
        return Err(
            "informe o comando que imprime o token (ex.: pass show github | head -1)".into(),
        );
    }
    chaveiro.gravar(&conta_comando_global(&host), &Secret::new(comando))?;
    chaveiro.apagar(&conta_global(&host))?;
    esquecer_teste(dados, &conta_global(&host))?;
    tracing::info!(host = %host, "comando do token do host definido");
    Ok(())
}

/// O token do alvo mudou: o último teste deixa de valer.
pub fn esquecer_teste(dados: &DiretorioDados, conta: &str) -> Result<(), String> {
    let prefs = Preferencias::ler(dados)?;
    if !prefs.acessos.testes.contains_key(conta) {
        return Ok(());
    }
    alterar(dados, |a| {
        a.testes.remove(conta);
    })
}

/// Para cada alvo: se há token guardado, o comando do host e o último teste.
pub fn estado_tokens(
    dados: &DiretorioDados,
    chaveiro: &dyn Chaveiro,
    alvos: &[AlvoToken],
) -> Result<Vec<EstadoToken>, String> {
    let testes = Preferencias::ler(dados)?.acessos.testes;
    alvos
        .iter()
        .map(|a| {
            let conta = a.conta()?;
            let global = a.caminho.as_deref().is_none_or(|c| c.trim().is_empty());
            Ok(EstadoToken {
                guardado: chaveiro.ler(&conta)?.is_some(),
                comando: if global {
                    comando_global(&a.host, chaveiro)?
                } else {
                    None
                },
                teste: testes.get(&conta).cloned(),
            })
        })
        .collect()
}

fn registrar(
    dados: &DiretorioDados,
    conta: String,
    teste: TesteToken,
) -> Result<TesteToken, String> {
    let t = teste.clone();
    alterar(dados, |a| {
        a.testes.insert(conta, t);
    })?;
    Ok(teste)
}

fn falhou(mensagem: String) -> TesteToken {
    TesteToken {
        ok: false,
        quando: agora(),
        mensagem: Some(mensagem),
        usuario: None,
        expira: None,
    }
}

/// Testa o token global do host (o guardado ou o do comando) na API da forja e
/// guarda o resultado. Sem a CLI da forja (`gh`, `glab`), testa pelo git num
/// repositório que usa o host (`exemplo`), quando houver.
pub fn testar_host(
    dados: &DiretorioDados,
    f: &Ferramentas,
    host: &str,
    forja: ForgeKind,
    exemplo: Option<&RepositorioCodigo>,
) -> Result<TesteToken, String> {
    let host = host_valido(host)?;
    let teste = match token_global(&host, forja.label(), f.chaveiro.as_ref()) {
        Err(e) => falhou(e.to_string()),
        Ok(None) => falhou(format!("nenhum token guardado para {host}")),
        Ok(Some(token)) => match verificar_token_host(forja, &host, &token, f) {
            Ok(v) => TesteToken {
                ok: true,
                quando: agora(),
                mensagem: None,
                usuario: v.usuario,
                expira: v.expira,
            },
            Err(ErroVerificacao::SemFerramenta(cli)) => match exemplo {
                Some(r) => match testar_acesso(&f.git, r, Some(&token)) {
                    Ok(()) => TesteToken {
                        ok: true,
                        quando: agora(),
                        mensagem: None,
                        usuario: None,
                        expira: None,
                    },
                    Err(e) => falhou(e),
                },
                None => falhou(format!(
                    "{cli} não encontrado: instale-o para testar o token de {host}"
                )),
            },
            Err(e) => falhou(format!("{}: {e}", forja.label())),
        },
    };
    tracing::info!(host = %host, ok = teste.ok, "token do host testado");
    registrar(dados, conta_global(&host), teste)
}

/// Testa o acesso de leitura de um repositório com o token dele (próprio,
/// comando ou global do host) e guarda o resultado na conta do token próprio.
pub fn testar_repositorio(
    dados: &DiretorioDados,
    f: &Ferramentas,
    repo: &RepositorioCodigo,
) -> Result<TesteToken, String> {
    let teste = match token_do_repositorio(repo, f.chaveiro.as_ref()) {
        Err(e) => falhou(e.to_string()),
        Ok(token) => match testar_acesso(&f.git, repo, token.as_ref()) {
            Ok(()) => TesteToken {
                ok: true,
                quando: agora(),
                mensagem: None,
                usuario: None,
                expira: None,
            },
            Err(e) => falhou(e),
        },
    };
    tracing::info!(repositorio = %repo.rotulo(), ok = teste.ok, "token do repositório testado");
    registrar(dados, cba_repos::chaveiro::conta_propria(repo), teste)
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
pub async fn hosts_acessos_app() -> Result<Vec<HostCadastrado>, String> {
    let dados = dados()?;
    bloqueante(move || hosts_cadastrados(&dados)).await
}

#[tauri::command]
pub async fn adicionar_host_acesso_app(
    host: String,
    forja: ForgeKind,
) -> Result<HostCadastrado, String> {
    let dados = dados()?;
    bloqueante(move || adicionar_host(&dados, &host, forja)).await
}

#[tauri::command]
pub async fn remover_host_acesso_app(host: String) -> Result<(), String> {
    let dados = dados()?;
    bloqueante(move || remover_host(&dados, Ferramentas::default().chaveiro.as_ref(), &host)).await
}

#[tauri::command]
pub async fn definir_comando_host_app(host: String, comando: String) -> Result<(), String> {
    let dados = dados()?;
    bloqueante(move || {
        definir_comando_host(
            &dados,
            Ferramentas::default().chaveiro.as_ref(),
            &host,
            &comando,
        )
    })
    .await
}

#[tauri::command]
pub async fn estado_tokens_app(alvos: Vec<AlvoToken>) -> Result<Vec<EstadoToken>, String> {
    let dados = dados()?;
    bloqueante(move || estado_tokens(&dados, Ferramentas::default().chaveiro.as_ref(), &alvos))
        .await
}

#[tauri::command]
pub async fn testar_token_host_app(
    host: String,
    forja: ForgeKind,
    exemplo: Option<RepositorioCodigo>,
) -> Result<TesteToken, String> {
    let dados = dados()?;
    bloqueante(move || {
        testar_host(
            &dados,
            &Ferramentas::default(),
            &host,
            forja,
            exemplo.as_ref(),
        )
    })
    .await
}

#[tauri::command]
pub async fn testar_token_repositorio_app(
    repositorio: RepositorioCodigo,
) -> Result<TesteToken, String> {
    let dados = dados()?;
    bloqueante(move || testar_repositorio(&dados, &Ferramentas::default(), &repositorio)).await
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use cba_repos::OrigemToken;
    use cba_repos::chaveiro::{ChaveiroMemoria, conta_propria};
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    fn script(dir: &Path, nome: &str, corpo: &str) -> PathBuf {
        let p = dir.join(nome);
        std::fs::write(&p, format!("#!/bin/sh\n{corpo}\n")).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p
    }

    fn global(host: &str) -> AlvoToken {
        AlvoToken {
            host: host.into(),
            caminho: None,
        }
    }

    #[test]
    fn testar_o_host_guarda_o_resultado_e_a_validade_sem_o_token() {
        let d = tempfile::tempdir().unwrap();
        let dados = DiretorioDados::em(d.path().join("dados"));
        let gh = script(
            d.path(),
            "gh",
            r#"[ "$GH_TOKEN" = "ghp-novo" ] || { echo "Bad credentials $GH_TOKEN (HTTP 401)" >&2; exit 1; }
printf 'HTTP/2.0 200 OK\ngithub-authentication-token-expiration: 2027-03-01 00:00:00 UTC\n\n{"login":"ana"}'"#,
        );
        let chaveiro = Arc::new(ChaveiroMemoria::default());
        let f = Ferramentas {
            gh,
            chaveiro: chaveiro.clone(),
            ..Default::default()
        };
        let sem = testar_host(&dados, &f, "github.com", ForgeKind::Github, None).unwrap();
        assert!(!sem.ok);
        chaveiro
            .gravar(&conta_global("github.com"), &Secret::new("ghp-velho"))
            .unwrap();
        let ruim = testar_host(&dados, &f, "github.com", ForgeKind::Github, None).unwrap();
        assert!(!ruim.ok);
        let msg = ruim.mensagem.clone().unwrap();
        assert!(
            msg.contains("Bad credentials") && !msg.contains("ghp-velho"),
            "{msg}"
        );
        // Trocar o token (como o "Trocar token" faz) vale na hora.
        chaveiro
            .gravar(&conta_global("github.com"), &Secret::new("ghp-novo"))
            .unwrap();
        let bom = testar_host(&dados, &f, "GitHub.com", ForgeKind::Github, None).unwrap();
        assert!(bom.ok);
        assert_eq!(bom.expira.as_deref(), Some("2027-03-01"));
        assert_eq!(bom.usuario.as_deref(), Some("ana"));
        let estado = estado_tokens(&dados, chaveiro.as_ref(), &[global("github.com")]).unwrap();
        assert!(estado[0].guardado);
        assert_eq!(estado[0].teste.as_ref(), Some(&bom));
        // Nada do que vai para a tela nem das preferências tem o token.
        let tudo = format!(
            "{}{}",
            serde_json::to_string(&estado).unwrap(),
            std::fs::read_to_string(Preferencias::arquivo(&dados)).unwrap()
        );
        assert!(!tudo.contains("ghp-"), "{tudo}");
    }

    #[test]
    fn comando_do_host_substitui_o_token_e_remover_apaga_tudo() {
        let d = tempfile::tempdir().unwrap();
        let dados = DiretorioDados::em(d.path().join("dados"));
        let c = ChaveiroMemoria::default();
        let h = adicionar_host(&dados, " GitLab.Example.com ", ForgeKind::Gitlab).unwrap();
        assert_eq!(h.host, "gitlab.example.com");
        adicionar_host(&dados, "gitlab.example.com", ForgeKind::Gitlab).unwrap();
        assert_eq!(hosts_cadastrados(&dados).unwrap().len(), 1);
        assert!(adicionar_host(&dados, "https://x", ForgeKind::Gitlab).is_err());

        c.gravar(&conta_global("gitlab.example.com"), &Secret::new("glpat"))
            .unwrap();
        assert!(definir_comando_host(&dados, &c, "gitlab.example.com", "  ").is_err());
        definir_comando_host(&dados, &c, "gitlab.example.com", "echo do-comando").unwrap();
        let e = estado_tokens(&dados, &c, &[global("gitlab.example.com")]).unwrap();
        assert!(!e[0].guardado);
        assert_eq!(e[0].comando.as_deref(), Some("echo do-comando"));
        // Os repositórios do host passam a usar o comando; o próprio continua valendo.
        let repo = RepositorioCodigo {
            forja: ForgeKind::Gitlab,
            host: "gitlab.example.com".into(),
            caminho: "g/app".into(),
            token: OrigemToken::Global,
            ..Default::default()
        };
        assert_eq!(
            token_do_repositorio(&repo, &c).unwrap().unwrap().expose(),
            "do-comando"
        );
        let proprio = RepositorioCodigo {
            caminho: "g/outro".into(),
            token: OrigemToken::Proprio,
            ..repo.clone()
        };
        c.gravar(&conta_propria(&proprio), &Secret::new("so-dele"))
            .unwrap();

        remover_host(&dados, &c, "gitlab.example.com").unwrap();
        assert!(hosts_cadastrados(&dados).unwrap().is_empty());
        assert!(token_do_repositorio(&repo, &c).unwrap().is_none());
        assert_eq!(
            token_do_repositorio(&proprio, &c)
                .unwrap()
                .unwrap()
                .expose(),
            "so-dele"
        );
    }

    #[test]
    fn testar_o_repositorio_registra_na_conta_do_token_proprio() {
        let d = tempfile::tempdir().unwrap();
        let dados = DiretorioDados::em(d.path().join("dados"));
        let c = Arc::new(ChaveiroMemoria::default());
        let f = Ferramentas {
            chaveiro: c.clone(),
            ..Default::default()
        };
        let repo = RepositorioCodigo {
            caminho: "o/web".into(),
            url_clone: format!("file://{}/nada", d.path().display()),
            token: OrigemToken::Proprio,
            ..Default::default()
        };
        // Sem o token próprio no chaveiro: falha, com a mensagem do cadastro.
        let t = testar_repositorio(&dados, &f, &repo).unwrap();
        assert!(!t.ok && t.mensagem.unwrap().contains("token só dele"));
        let alvo = AlvoToken {
            host: "github.com".into(),
            caminho: Some("o/web".into()),
        };
        let e = estado_tokens(&dados, c.as_ref(), &[alvo]).unwrap();
        assert!(e[0].teste.is_some() && e[0].comando.is_none());
    }
}
