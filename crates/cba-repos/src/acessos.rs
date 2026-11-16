//! "Testar" o token global de um host (#193): pela API da forja, com as CLIs
//! (`gh api` e `glab api`), sem HTTP próprio. Além de dizer se o token vale,
//! traz a validade quando a forja a informa (o GitHub no cabeçalho
//! `github-authentication-token-expiration`; o GitLab em
//! `personal_access_tokens/self`).
//!
//! O token chega ao processo filho só no ambiente; as mensagens passam por
//! [`cite`], que troca o valor por `***`.

use crate::demanda::{Ferramentas, PRAZO_CLI};
use crate::process::{self, RunError, cite};
use crate::{ForgeKind, Secret};
use serde_json::Value;
use std::path::Path;
use std::process::Command;

/// O que o teste do token descobriu.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VerificacaoToken {
    /// Login do dono do token, quando a forja informa.
    pub usuario: Option<String>,
    /// Validade (`AAAA-MM-DD`); `None` = sem validade ou não informada.
    pub expira: Option<String>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ErroVerificacao {
    /// A CLI da forja não está instalada: quem chamou pode testar pelo git.
    #[error("{0} não encontrado: instale-o para testar o token pela API")]
    SemFerramenta(&'static str),
    #[error("{0}")]
    Recusado(String),
}

/// Testa o token na API da forja do `host`.
pub fn verificar_token_host(
    forja: ForgeKind,
    host: &str,
    token: &Secret,
    f: &Ferramentas,
) -> Result<VerificacaoToken, ErroVerificacao> {
    let host = host.trim().to_ascii_lowercase();
    match forja {
        ForgeKind::Github => verificar_github(&f.gh, &host, token),
        ForgeKind::Gitlab => verificar_gitlab(&f.glab, &host, token),
    }
}

/// Roda a CLI; devolve o `stdout` ou a mensagem do erro, sem o token.
fn rodar(
    ferramenta: &'static str,
    cmd: Command,
    token: &Secret,
) -> Result<String, ErroVerificacao> {
    let o = process::run(cmd, PRAZO_CLI).map_err(|e| match e {
        RunError::NotFound => ErroVerificacao::SemFerramenta(ferramenta),
        RunError::Timeout(_) => ErroVerificacao::Recusado(
            "o servidor não respondeu a tempo: confira o host e a rede".into(),
        ),
        e => ErroVerificacao::Recusado(cite(&e.to_string(), Some(token))),
    })?;
    if o.status.success() {
        return Ok(String::from_utf8_lossy(&o.stdout).into_owned());
    }
    let detalhe = cite(&String::from_utf8_lossy(&o.stderr), Some(token));
    let detalhe = if detalhe.is_empty() {
        cite(&String::from_utf8_lossy(&o.stdout), Some(token))
    } else {
        detalhe
    };
    Err(ErroVerificacao::Recusado(if detalhe.is_empty() {
        format!("{ferramenta} recusou o token")
    } else {
        format!("recusou o token: {detalhe}")
    }))
}

fn verificar_github(
    gh: &Path,
    host: &str,
    token: &Secret,
) -> Result<VerificacaoToken, ErroVerificacao> {
    let mut c = Command::new(gh);
    // `GH_TOKEN` no ambiente vence a autenticação do próprio gh (`gh auth login`).
    c.args(["api", "-i", "user"])
        .env("GH_HOST", host)
        .env("GH_TOKEN", token.expose());
    let saida = rodar("gh", c, token)?;
    let (cabecalhos, corpo) = separar_resposta(&saida);
    let expira = cabecalhos.lines().find_map(|l| {
        let (nome, valor) = l.split_once(':')?;
        nome.trim()
            .eq_ignore_ascii_case("github-authentication-token-expiration")
            .then(|| data(valor))
            .flatten()
    });
    let usuario = serde_json::from_str::<Value>(corpo)
        .ok()
        .and_then(|v| v["login"].as_str().map(str::to_string));
    Ok(VerificacaoToken { usuario, expira })
}

fn verificar_gitlab(
    glab: &Path,
    host: &str,
    token: &Secret,
) -> Result<VerificacaoToken, ErroVerificacao> {
    let chamar = |rota: &str| {
        let mut c = Command::new(glab);
        c.args(["api", rota])
            .env("GITLAB_HOST", host)
            .env("GITLAB_TOKEN", token.expose());
        rodar("glab", c, token).map(|s| serde_json::from_str::<Value>(&s).unwrap_or(Value::Null))
    };
    let usuario = chamar("user")?["username"].as_str().map(str::to_string);
    // Validade: só nos PATs e em GitLab 15.5+; sem ela, o teste continua valendo.
    let expira = match chamar("personal_access_tokens/self") {
        Ok(v) => {
            if v["revoked"].as_bool() == Some(true) || v["active"].as_bool() == Some(false) {
                return Err(ErroVerificacao::Recusado(
                    "o GitLab diz que o token foi revogado ou expirou".into(),
                ));
            }
            v["expires_at"].as_str().and_then(data)
        }
        Err(_) => None,
    };
    Ok(VerificacaoToken { usuario, expira })
}

/// Cabeçalhos e corpo de `gh api -i` (separados pela primeira linha vazia).
fn separar_resposta(s: &str) -> (&str, &str) {
    for sep in ["\r\n\r\n", "\n\n"] {
        if let Some(i) = s.find(sep) {
            return (&s[..i], &s[i + sep.len()..]);
        }
    }
    (s, "")
}

/// `AAAA-MM-DD` do começo de uma data (`2026-11-10 12:00:00 UTC`).
fn data(valor: &str) -> Option<String> {
    let v = valor.trim();
    let d = v.get(..10)?;
    let ok = d.bytes().enumerate().all(|(i, b)| match i {
        4 | 7 => b == b'-',
        _ => b.is_ascii_digit(),
    });
    ok.then(|| d.to_string())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::demanda::falsas::script;

    #[test]
    fn github_traz_o_login_e_a_validade_do_cabecalho() {
        let d = tempfile::tempdir().unwrap();
        let gh = script(
            d.path(),
            "gh",
            r#"[ "$GH_TOKEN" = "ghp-certo" ] || { echo "gh: Bad credentials ($GH_TOKEN) (HTTP 401)" >&2; exit 1; }
printf 'HTTP/2.0 200 OK\r\nGithub-Authentication-Token-Expiration: 2026-11-10 12:00:00 -0300\r\n\r\n{"login":"ana"}'"#,
        );
        let f = Ferramentas {
            gh,
            ..Default::default()
        };
        let v = verificar_token_host(
            ForgeKind::Github,
            "github.com",
            &Secret::new("ghp-certo"),
            &f,
        )
        .unwrap();
        assert_eq!(v.usuario.as_deref(), Some("ana"));
        assert_eq!(v.expira.as_deref(), Some("2026-11-10"));
        let e = verificar_token_host(
            ForgeKind::Github,
            "github.com",
            &Secret::new("ghp-errado"),
            &f,
        )
        .unwrap_err();
        let msg = e.to_string();
        assert!(
            msg.contains("Bad credentials") && !msg.contains("ghp-errado"),
            "{msg}"
        );
    }

    #[test]
    fn gitlab_traz_a_validade_e_aceita_servidor_sem_o_endpoint() {
        let d = tempfile::tempdir().unwrap();
        let glab = script(
            d.path(),
            "glab",
            r#"[ "$GITLAB_TOKEN" = "glpat-1" ] && [ "$GITLAB_HOST" = "gitlab.example.com" ] || { echo "401 Unauthorized" >&2; exit 1; }
case "$2" in
  user) echo '{"username":"bia"}';;
  personal_access_tokens/self) [ -n "$SEM_ENDPOINT" ] && { echo "404" >&2; exit 1; }; echo '{"active":true,"revoked":false,"expires_at":"2027-01-31"}';;
esac"#,
        );
        let f = Ferramentas {
            glab: glab.clone(),
            ..Default::default()
        };
        let v = verificar_token_host(
            ForgeKind::Gitlab,
            "GitLab.example.com",
            &Secret::new("glpat-1"),
            &f,
        )
        .unwrap();
        assert_eq!(v.usuario.as_deref(), Some("bia"));
        assert_eq!(v.expira.as_deref(), Some("2027-01-31"));
        assert!(matches!(
            verificar_token_host(
                ForgeKind::Gitlab,
                "gitlab.example.com",
                &Secret::new("x"),
                &f
            ),
            Err(ErroVerificacao::Recusado(_))
        ));
        let sem = script(
            d.path(),
            "glab-antigo",
            &format!("SEM_ENDPOINT=1 exec {} \"$@\"", glab.display()),
        );
        let f = Ferramentas {
            glab: sem,
            ..Default::default()
        };
        let v = verificar_token_host(
            ForgeKind::Gitlab,
            "gitlab.example.com",
            &Secret::new("glpat-1"),
            &f,
        )
        .unwrap();
        assert_eq!(v.expira, None);
    }

    #[test]
    fn sem_a_cli_e_datas() {
        let f = Ferramentas {
            gh: "/nao/existe/gh".into(),
            ..Default::default()
        };
        assert_eq!(
            verificar_token_host(ForgeKind::Github, "github.com", &Secret::new("t"), &f),
            Err(ErroVerificacao::SemFerramenta("gh"))
        );
        assert_eq!(data(" 2026-11-10T00:00Z"), Some("2026-11-10".into()));
        assert_eq!(data("ontem"), None);
    }
}
