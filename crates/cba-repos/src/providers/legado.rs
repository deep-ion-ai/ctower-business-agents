//! Ponte entre os cadastros de hoje (rastreador com `credential_cmd`,
//! repositórios com token global por host, próprio ou por comando) e o
//! [resolvedor](super::Resolvedor) (#436).
//!
//! Cada cadastro vira uma [`Conexao`] implícita, e a credencial sai sempre pelo
//! resolvedor, com cache, nova tentativa e mascaramento. A migração dos
//! cadastros para conexões nomeadas (#438) troca essas conexões implícitas
//! pelas do cadastro do app.

use super::{Conexao, ErroCredencial, FonteCredencial, Resolvedor, todos};
use crate::chaveiro::{Chaveiro, comando_global, conta_global, conta_propria};
use crate::codigo::{OrigemToken, RepositorioCodigo};
use crate::{GitlabConfig, JiraConfig, Secret};
use std::collections::BTreeMap;

/// Tipo de conexão pelo rótulo das mensagens de hoje (`GitLab` → `gitlab`).
fn tipo_do_rotulo(rotulo: &str) -> &'static str {
    todos()
        .iter()
        .find(|d| d.rotulo().eq_ignore_ascii_case(rotulo) || d.tipo().eq_ignore_ascii_case(rotulo))
        .map_or("github", |d| d.tipo())
}

fn conexao(tipo: &str, endereco: &str, credencial: FonteCredencial) -> Conexao {
    Conexao {
        id: format!("implicita:{tipo}:{}", endereco.trim().to_ascii_lowercase()),
        nome: format!("{tipo} {}", endereco.trim()),
        tipo: tipo.to_string(),
        endereco: endereco.trim().to_string(),
        credencial,
        opcoes: BTreeMap::new(),
    }
}

/// Token global de um host: o guardado no chaveiro (`host:<host>`) ou, sem
/// ele, o que o comando do host (`comando:host:<host>`) imprime.
pub fn token_do_host(
    host: &str,
    rotulo: &str,
    chaveiro: &dyn Chaveiro,
) -> Result<Option<Secret>, ErroCredencial> {
    let tipo = tipo_do_rotulo(rotulo);
    let r = Resolvedor::global();
    let guardado = conexao(
        tipo,
        host,
        FonteCredencial::Chaveiro {
            conta: conta_global(host),
        },
    );
    if let Some(t) = r.resolver(&guardado, chaveiro, None)? {
        return Ok(Some(t));
    }
    match comando_global(host, chaveiro).map_err(ErroCredencial::Chaveiro)? {
        Some(cmd) => r.resolver(
            &conexao(tipo, host, FonteCredencial::Comando { cmd }),
            chaveiro,
            None,
        ),
        None => Ok(None),
    }
}

/// Credencial guardada pela própria CLI do provider (`gh auth token`).
pub fn token_cli(
    host: &str,
    rotulo: &str,
    cli: &std::path::Path,
    chaveiro: &dyn Chaveiro,
) -> Result<Secret, ErroCredencial> {
    let c = conexao(tipo_do_rotulo(rotulo), host, FonteCredencial::Cli);
    Resolvedor::global()
        .resolver(&c, chaveiro, Some(cli))?
        .ok_or_else(|| ErroCredencial::Conexao("sem token".into()))
}

/// Credencial de um comando do cadastro (`credential_cmd`).
pub fn token_do_comando(
    rotulo: &str,
    host: &str,
    cmd: &str,
    chaveiro: &dyn Chaveiro,
) -> Result<Secret, ErroCredencial> {
    let c = conexao(
        tipo_do_rotulo(rotulo),
        host,
        FonteCredencial::Comando {
            cmd: cmd.to_string(),
        },
    );
    Resolvedor::global()
        .resolver(&c, chaveiro, None)?
        .ok_or_else(|| ErroCredencial::Conexao("o comando de credencial não imprimiu nada".into()))
}

/// Credencial do GitLab de issues: o `credential_cmd`, se cadastrado; senão, o
/// token global do host.
pub fn token_gitlab(
    g: &GitlabConfig,
    chaveiro: &dyn Chaveiro,
) -> Result<Option<Secret>, ErroCredencial> {
    if !g.credential_cmd.trim().is_empty() {
        return token_do_comando("GitLab", &g.host, &g.credential_cmd, chaveiro).map(Some);
    }
    token_do_host(&g.host, "GitLab", chaveiro)
}

/// Credencial do Jira: o `credential_cmd`, se cadastrado; senão, o token do host.
pub fn token_jira(
    j: &JiraConfig,
    chaveiro: &dyn Chaveiro,
) -> Result<Option<Secret>, ErroCredencial> {
    if !j.credential_cmd.trim().is_empty() {
        return token_do_comando("Jira", &j.host(), &j.credential_cmd, chaveiro).map(Some);
    }
    token_do_host(&j.host(), "Jira", chaveiro)
}

/// Credencial de um repositório de código: o próprio (sobrescrita) tem
/// precedência sobre o global do host; com `token = comando`, o do comando.
///
/// `Ok(None)` com `Proprio` sem token guardado: quem chama dá o erro.
pub fn token_repositorio(
    repo: &RepositorioCodigo,
    chaveiro: &dyn Chaveiro,
) -> Result<Option<Secret>, ErroCredencial> {
    let rotulo = repo.forja.label();
    if repo.token == OrigemToken::Comando {
        return token_do_comando(rotulo, &repo.host, &repo.comando_credencial, chaveiro).map(Some);
    }
    let r = Resolvedor::global();
    let proprio = conta_propria(repo);
    // Só a sobrescrita, sem cair na credencial da conexão.
    let so_proprio = conexao(tipo_do_rotulo(rotulo), &repo.host, FonteCredencial::Nenhuma);
    if let Some(t) = r.resolver_com_sobrescrita(&so_proprio, Some(&proprio), chaveiro, None)? {
        return Ok(Some(t));
    }
    if repo.token == OrigemToken::Proprio {
        return Ok(None);
    }
    token_do_host(&repo.host, rotulo, chaveiro)
}
