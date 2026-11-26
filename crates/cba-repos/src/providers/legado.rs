//! Ponte entre os cadastros de hoje (rastreador com `credential_cmd`,
//! repositórios com token global por host, próprio ou por comando) e o
//! [resolvedor](super::Resolvedor) (#436).
//!
//! Cada cadastro vira uma [`Conexao`] implícita, e a credencial sai sempre pelo
//! resolvedor, com cache, nova tentativa e mascaramento. A migração dos
//! cadastros para conexões nomeadas (#438) troca essas conexões implícitas
//! pelas do cadastro do app.
//!
//! Quando o cadastro já referencia uma conexão das Configurações (`conexao`,
//! #437), a credencial sai dela; o caminho implícito fica só para os cadastros
//! que ainda não foram migrados.

use super::{Conexao, ErroCredencial, FonteCredencial, Resolvedor, conexao_referenciada, todos};
use crate::chaveiro::{Chaveiro, comando_global, conta_global, conta_propria};
use crate::codigo::{OrigemToken, RepositorioCodigo};
use crate::{GithubConfig, GitlabConfig, JiraConfig, Secret};
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

/// Credencial da conexão referenciada pelo cadastro (`id`), pelo resolvedor.
/// Erro em pt-BR quando a conexão não está mais cadastrada.
pub fn token_da_conexao(
    id: &str,
    chaveiro: &dyn Chaveiro,
    cli: Option<&std::path::Path>,
) -> Result<Option<Secret>, ErroCredencial> {
    let c = conexao_referenciada(id).map_err(ErroCredencial::Conexao)?;
    Resolvedor::global().resolver(&c, chaveiro, cli)
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
    if !g.conexao.trim().is_empty() {
        return token_da_conexao(&g.conexao, chaveiro, None);
    }
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
    if !j.conexao.trim().is_empty() {
        return token_da_conexao(&j.conexao, chaveiro, None);
    }
    if !j.credential_cmd.trim().is_empty() {
        return token_do_comando("Jira", &j.host(), &j.credential_cmd, chaveiro).map(Some);
    }
    token_do_host(&j.host(), "Jira", chaveiro)
}

/// Credencial do GitHub de issues: a da conexão, se referenciada; senão, o
/// token global do github.com. `cli` é o executável do `gh` (fonte `Cli`).
pub fn token_github(
    g: &GithubConfig,
    chaveiro: &dyn Chaveiro,
    cli: Option<&std::path::Path>,
) -> Result<Option<Secret>, ErroCredencial> {
    if !g.conexao.trim().is_empty() {
        return token_da_conexao(&g.conexao, chaveiro, cli);
    }
    token_do_host(crate::codigo::HOST_GITHUB, "GitHub", chaveiro)
}

/// Credencial de um repositório de código: o próprio (sobrescrita) tem
/// precedência sobre o global do host; com `token = comando`, o do comando.
///
/// `Ok(None)` com `Proprio` sem token guardado: quem chama dá o erro.
pub fn token_repositorio(
    repo: &RepositorioCodigo,
    chaveiro: &dyn Chaveiro,
) -> Result<Option<Secret>, ErroCredencial> {
    if !repo.conexao.trim().is_empty() {
        // O token próprio do repositório (`repo:`) é sobrescrita da conexão.
        let c = conexao_referenciada(&repo.conexao).map_err(ErroCredencial::Conexao)?;
        return Resolvedor::global().resolver_com_sobrescrita(
            &c,
            Some(&conta_propria(repo)),
            chaveiro,
            None,
        );
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ForgeKind;
    use crate::chaveiro::ChaveiroMemoria;
    use crate::providers::Catalogo;

    fn cadastrar(id: &str, tipo: &str, endereco: &str) {
        Catalogo::global().gravar(Conexao {
            id: id.into(),
            nome: id.into(),
            tipo: tipo.into(),
            endereco: endereco.into(),
            credencial: FonteCredencial::Chaveiro {
                conta: format!("conexao:{id}"),
            },
            opcoes: BTreeMap::new(),
        });
    }

    #[test]
    fn dois_projetos_na_mesma_conexao_trocam_o_token_juntos() {
        cadastrar("legado-gl-comum", "gitlab", "gitlab.example.com");
        cadastrar("legado-jira-comum", "jira", "https://jira.example.com");
        let c = ChaveiroMemoria::default();
        c.gravar("conexao:legado-gl-comum", &Secret::new("tok-gl-1"))
            .unwrap();
        c.gravar("conexao:legado-jira-comum", &Secret::new("tok-jira-1"))
            .unwrap();
        let repo = |caminho: &str| RepositorioCodigo {
            forja: ForgeKind::Gitlab,
            host: "gitlab.example.com".into(),
            caminho: caminho.into(),
            conexao: "legado-gl-comum".into(),
            // Cadastro antigo com comando: a conexão vale no lugar dele.
            token: OrigemToken::Comando,
            comando_credencial: "echo nao-usado".into(),
            ..Default::default()
        };
        let a = repo("g/projeto-a");
        let b = repo("g/projeto-b");
        let issues = GitlabConfig {
            host: "gitlab.example.com".into(),
            path: "g/projeto-a".into(),
            conexao: "legado-gl-comum".into(),
            ..Default::default()
        };
        let jira = |chave: &str| JiraConfig {
            url: "https://jira.example.com".into(),
            project_key: chave.into(),
            conexao: "legado-jira-comum".into(),
            ..Default::default()
        };
        let valor = |s: Option<Secret>| s.unwrap().expose().to_string();
        assert_eq!(valor(token_repositorio(&a, &c).unwrap()), "tok-gl-1");
        assert_eq!(valor(token_repositorio(&b, &c).unwrap()), "tok-gl-1");
        assert_eq!(valor(token_gitlab(&issues, &c).unwrap()), "tok-gl-1");
        assert_eq!(valor(token_jira(&jira("FAT"), &c).unwrap()), "tok-jira-1");

        c.gravar("conexao:legado-gl-comum", &Secret::new("tok-gl-2"))
            .unwrap();
        c.gravar("conexao:legado-jira-comum", &Secret::new("tok-jira-2"))
            .unwrap();
        assert_eq!(valor(token_repositorio(&a, &c).unwrap()), "tok-gl-2");
        assert_eq!(valor(token_repositorio(&b, &c).unwrap()), "tok-gl-2");
        assert_eq!(valor(token_gitlab(&issues, &c).unwrap()), "tok-gl-2");
        assert_eq!(valor(token_jira(&jira("FAT"), &c).unwrap()), "tok-jira-2");
        assert_eq!(valor(token_jira(&jira("COB"), &c).unwrap()), "tok-jira-2");

        // O token próprio do repositório é sobrescrita da conexão.
        c.gravar(&conta_propria(&b), &Secret::new("tok-so-do-b"))
            .unwrap();
        assert_eq!(valor(token_repositorio(&b, &c).unwrap()), "tok-so-do-b");
        assert_eq!(valor(token_repositorio(&a, &c).unwrap()), "tok-gl-2");
    }

    #[test]
    fn conexao_que_sumiu_e_erro_claro() {
        let c = ChaveiroMemoria::default();
        let g = GitlabConfig {
            host: "gitlab.example.com".into(),
            path: "g/p".into(),
            conexao: "legado-inexistente".into(),
            ..Default::default()
        };
        let e = token_gitlab(&g, &c).unwrap_err().to_string();
        assert!(e.contains("legado-inexistente"), "{e}");
        assert!(e.contains("Configurações › Conexões"), "{e}");
    }
}
