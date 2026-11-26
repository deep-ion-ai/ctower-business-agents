//! Tokens de acesso (PAT) no chaveiro do sistema (#179): Keychain no macOS,
//! Gerenciador de Credenciais no Windows, keyutils no Linux.
//!
//! Dois níveis: o token **global** de um host (`host:gitlab.example.com`), que
//! vale para todos os repositórios daquele host, e o token **próprio** de um
//! repositório (`repo:gitlab.example.com/grupo/app`), que tem precedência. O
//! comando que imprime o token continua como alternativa: no repositório
//! (`token = comando`, no cadastro) ou no host (#193), guardado no chaveiro em
//! `comando:host:<host>` no lugar do token global.
//!
//! O token nunca vai para o cadastro, para logs nem para mensagens: só sai do
//! chaveiro como [`Secret`] e vai para o ambiente do processo filho.

use crate::codigo::{OrigemToken, RepositorioCodigo};
use crate::providers::{ErroCredencial, Resolvedor, legado};
use crate::{CredentialError, Secret};
use std::collections::HashMap;
use std::fmt;
use std::sync::Mutex;

/// Serviço das entradas no chaveiro do sistema.
pub const SERVICO: &str = "ai.deep-ion.ctower-business-agents";

/// Onde os tokens ficam guardados.
pub trait Chaveiro: Send + Sync {
    /// O token da `conta`, se houver.
    fn ler(&self, conta: &str) -> Result<Option<Secret>, String>;
    fn gravar(&self, conta: &str, token: &Secret) -> Result<(), String>;
    /// Apaga o token; sem token, não faz nada.
    fn apagar(&self, conta: &str) -> Result<(), String>;
    /// O resolvedor pode guardar em cache o que este chaveiro devolve? O nativo
    /// pode (e esvazia o cache a cada gravação); os de teste, não.
    fn cacheavel(&self) -> bool {
        false
    }
}

/// Chaveiro do sistema operacional.
#[derive(Debug, Default, Clone, Copy)]
pub struct ChaveiroNativo;

impl ChaveiroNativo {
    fn entrada(conta: &str) -> Result<keyring::Entry, String> {
        keyring::Entry::new(SERVICO, conta).map_err(|e| erro_chaveiro(&e))
    }
}

/// Mensagem do erro do chaveiro, sem o conteúdo (o `keyring` não põe o
/// segredo nas mensagens, mas os dados brutos de `BadEncoding` ficam de fora).
fn erro_chaveiro(e: &keyring::Error) -> String {
    match e {
        keyring::Error::BadEncoding(_) => "token ilegível no chaveiro do sistema".into(),
        e => format!("chaveiro do sistema: {e}"),
    }
}

impl Chaveiro for ChaveiroNativo {
    fn ler(&self, conta: &str) -> Result<Option<Secret>, String> {
        match Self::entrada(conta)?.get_password() {
            Ok(t) => Ok(Some(Secret::new(t))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(erro_chaveiro(&e)),
        }
    }

    fn gravar(&self, conta: &str, token: &Secret) -> Result<(), String> {
        Resolvedor::global().esquecer_tudo();
        crate::providers::mascara::registrar(token);
        Self::entrada(conta)?
            .set_password(token.expose())
            .map_err(|e| erro_chaveiro(&e))
    }

    fn apagar(&self, conta: &str) -> Result<(), String> {
        Resolvedor::global().esquecer_tudo();
        match Self::entrada(conta)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(erro_chaveiro(&e)),
        }
    }

    fn cacheavel(&self) -> bool {
        true
    }
}

/// Chaveiro em memória, para os testes.
#[derive(Default)]
pub struct ChaveiroMemoria(Mutex<HashMap<String, String>>);

impl fmt::Debug for ChaveiroMemoria {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ChaveiroMemoria(***)")
    }
}

impl Chaveiro for ChaveiroMemoria {
    fn ler(&self, conta: &str) -> Result<Option<Secret>, String> {
        Ok(self
            .0
            .lock()
            .map_err(|e| e.to_string())?
            .get(conta)
            .map(|t| Secret::new(t.clone())))
    }

    fn gravar(&self, conta: &str, token: &Secret) -> Result<(), String> {
        self.0
            .lock()
            .map_err(|e| e.to_string())?
            .insert(conta.to_string(), token.expose().to_string());
        Ok(())
    }

    fn apagar(&self, conta: &str) -> Result<(), String> {
        self.0.lock().map_err(|e| e.to_string())?.remove(conta);
        Ok(())
    }
}

/// Conta do token global de um host.
pub fn conta_global(host: &str) -> String {
    format!("host:{}", host.trim().to_ascii_lowercase())
}

/// Conta do comando que imprime o token global de um host (#193): a
/// alternativa a guardar o token. O comando não é segredo, mas fica no chaveiro
/// junto do token para que todo lugar que lê o token global o encontre.
pub fn conta_comando_global(host: &str) -> String {
    format!("comando:{}", conta_global(host))
}

/// O comando que imprime o token global do host, se configurado.
pub fn comando_global(host: &str, chaveiro: &dyn Chaveiro) -> Result<Option<String>, String> {
    Ok(chaveiro
        .ler(&conta_comando_global(host))?
        .map(|c| c.expose().to_string())
        .filter(|c| !c.trim().is_empty()))
}

/// Token global do host: o guardado no chaveiro ou, sem ele, o que o comando
/// do host imprime, pelo resolvedor (#436). `None`: nenhum dos dois. `what`
/// nomeia o host nas mensagens do comando ("GitHub", "GitLab", "Jira").
pub fn token_global(
    host: &str,
    what: &'static str,
    chaveiro: &dyn Chaveiro,
) -> Result<Option<Secret>, ErroToken> {
    Ok(legado::token_do_host(host, what, chaveiro)?)
}

/// Conta do token próprio de um repositório.
pub fn conta_propria(repo: &RepositorioCodigo) -> String {
    format!("repo:{}", repo.chave())
}

/// Por que o token de um repositório não pôde ser lido.
#[derive(Debug, thiserror::Error)]
pub enum ErroToken {
    #[error("{0}")]
    Chaveiro(String),
    #[error(
        "o repositório {0} usa um token só dele, que não está no chaveiro: informe o token de novo"
    )]
    SemTokenProprio(String),
    #[error("{0}")]
    Comando(#[from] CredentialError),
}

impl From<ErroCredencial> for ErroToken {
    fn from(e: ErroCredencial) -> Self {
        match e {
            ErroCredencial::Comando(c) => ErroToken::Comando(c),
            e => ErroToken::Chaveiro(e.to_string()),
        }
    }
}

/// Token de um repositório: o próprio (se houver) tem precedência sobre o
/// global do host; com `token = comando`, o que o comando imprime. `None`: sem
/// token (repositório público, SSH ou credencial do próprio git do analista).
/// Sai pelo resolvedor (#436).
pub fn token_do_repositorio(
    repo: &RepositorioCodigo,
    chaveiro: &dyn Chaveiro,
) -> Result<Option<Secret>, ErroToken> {
    match legado::token_repositorio(repo, chaveiro)? {
        None if repo.token == OrigemToken::Proprio => {
            Err(ErroToken::SemTokenProprio(repo.rotulo()))
        }
        t => Ok(t),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ForgeKind;

    fn gl(caminho: &str, token: OrigemToken) -> RepositorioCodigo {
        RepositorioCodigo {
            forja: ForgeKind::Gitlab,
            host: "GitLab.example.com".into(),
            caminho: caminho.into(),
            token,
            ..Default::default()
        }
    }

    #[test]
    fn proprio_tem_precedencia_sobre_o_global() {
        let c = ChaveiroMemoria::default();
        let a = gl("g/a", OrigemToken::Global);
        let b = gl("g/b", OrigemToken::Global);
        assert!(token_do_repositorio(&a, &c).unwrap().is_none());
        c.gravar(
            &conta_global("gitlab.example.com"),
            &Secret::new("global-1"),
        )
        .unwrap();
        assert_eq!(
            token_do_repositorio(&a, &c).unwrap().unwrap().expose(),
            "global-1"
        );
        c.gravar(&conta_propria(&b), &Secret::new("so-do-b"))
            .unwrap();
        assert_eq!(
            token_do_repositorio(&b, &c).unwrap().unwrap().expose(),
            "so-do-b"
        );
        // Trocar o global vale para todos os que o usam.
        c.gravar(
            &conta_global("gitlab.example.com"),
            &Secret::new("global-2"),
        )
        .unwrap();
        assert_eq!(
            token_do_repositorio(&a, &c).unwrap().unwrap().expose(),
            "global-2"
        );
        assert_eq!(
            token_do_repositorio(&b, &c).unwrap().unwrap().expose(),
            "so-do-b"
        );
    }

    #[test]
    fn proprio_ausente_e_comando() {
        let c = ChaveiroMemoria::default();
        c.gravar(&conta_global("gitlab.example.com"), &Secret::new("g"))
            .unwrap();
        let e = token_do_repositorio(&gl("g/x", OrigemToken::Proprio), &c).unwrap_err();
        assert!(matches!(e, ErroToken::SemTokenProprio(_)), "{e}");
        #[cfg(unix)]
        {
            let r = RepositorioCodigo {
                comando_credencial: "echo do-comando".into(),
                ..gl("g/x", OrigemToken::Comando)
            };
            assert_eq!(
                token_do_repositorio(&r, &c).unwrap().unwrap().expose(),
                "do-comando"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn comando_do_host_vale_para_os_repositorios_do_host_sem_token_guardado() {
        let c = ChaveiroMemoria::default();
        let a = gl("g/a", OrigemToken::Global);
        c.gravar(
            &conta_comando_global("gitlab.example.com"),
            &Secret::new("echo do-comando-do-host"),
        )
        .unwrap();
        assert_eq!(
            comando_global("GitLab.example.com", &c).unwrap().as_deref(),
            Some("echo do-comando-do-host")
        );
        assert_eq!(
            token_do_repositorio(&a, &c).unwrap().unwrap().expose(),
            "do-comando-do-host"
        );
        // O token guardado tem precedência sobre o comando do host.
        c.gravar(
            &conta_global("gitlab.example.com"),
            &Secret::new("guardado"),
        )
        .unwrap();
        assert_eq!(
            token_do_repositorio(&a, &c).unwrap().unwrap().expose(),
            "guardado"
        );
    }

    #[test]
    fn contas_e_debug_sem_token() {
        assert_eq!(conta_global(" GitHub.com "), "host:github.com");
        assert_eq!(
            conta_comando_global("GitHub.com"),
            "comando:host:github.com"
        );
        assert_eq!(
            conta_propria(&gl("G/App", OrigemToken::Proprio)),
            "repo:gitlab.example.com/g/app"
        );
        let c = ChaveiroMemoria::default();
        c.gravar("x", &Secret::new("segredo")).unwrap();
        assert!(!format!("{c:?}").contains("segredo"));
        c.apagar("x").unwrap();
        c.apagar("x").unwrap();
        assert!(c.ler("x").unwrap().is_none());
    }
}
