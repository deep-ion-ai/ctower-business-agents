//! Ambiente de teste do sistema analisado: onde a aplicação roda para testes e
//! com quais usuários entrar, um por perfil de acesso. Serve para simular o uso
//! e não deixar a análise limitada ao código.
//!
//! Nesta primeira etapa, só aplicações web com login por usuário e senha. O
//! cadastro guarda a URL, os usuários e os perfis, **nunca a senha**: a senha de
//! cada usuário fica no chaveiro do sistema, na conta devolvida por
//! [`conta_senha`], ligada ao identificador estável do usuário (trocar o login
//! não perde a senha).

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

/// Tamanho máximo dos textos curtos (URL, usuário, perfil).
const MAX_CURTO: usize = 500;
/// Tamanho máximo dos textos livres (descrição do perfil, observações).
const MAX_LONGO: usize = 2000;
/// Limite de usuários por ambiente.
pub const MAX_USUARIOS: usize = 100;

/// Tipo da aplicação testada. Por ora, só web.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TipoAplicacao {
    #[default]
    Web,
}

/// Como se entra na aplicação. Por ora, só usuário e senha.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Autenticacao {
    #[default]
    UsuarioSenha,
}

/// Usuário de teste, sem a senha.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsuarioTeste {
    /// Identificador estável no ambiente (`u1`, `u2`…), dado pelo app ao salvar.
    /// Liga o usuário à senha no chaveiro.
    pub id: String,
    /// Login usado na tela de entrada.
    pub usuario: String,
    /// Perfil de acesso do usuário ("Atendente", "Gestor financeiro").
    pub perfil: String,
    /// O que o perfil pode fazer ou para que serve o usuário (opcional).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub descricao: String,
}

/// Ambiente de teste de um projeto.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AmbienteTeste {
    /// Endereço da aplicação (`https://…`), normalmente a página de entrada.
    pub url: String,
    #[serde(default)]
    pub tipo: TipoAplicacao,
    #[serde(default)]
    pub autenticacao: Autenticacao,
    /// Recados para quem for usar o ambiente (VPN, horário, dados de exemplo).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub observacoes: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub usuarios: Vec<UsuarioTeste>,
}

impl AmbienteTeste {
    /// Confere URL, usuários e perfis. Devolve a primeira recusa, em linguagem do
    /// analista.
    pub fn validar(&self) -> Result<(), String> {
        validar_url(&self.url)?;
        texto("as observações", &self.observacoes, MAX_LONGO)?;
        if self.usuarios.len() > MAX_USUARIOS {
            return Err(format!("no máximo {MAX_USUARIOS} usuários por ambiente"));
        }
        let mut ids = HashSet::new();
        let mut logins = HashSet::new();
        for (i, u) in self.usuarios.iter().enumerate() {
            let n = i + 1;
            if !id_valido(&u.id) {
                return Err(format!("usuário {n}: identificador inválido {:?}", u.id));
            }
            if !ids.insert(u.id.as_str()) {
                return Err(format!("usuário {n}: identificador repetido {:?}", u.id));
            }
            let login = u.usuario.trim();
            if login.is_empty() {
                return Err(format!("usuário {n}: informe o usuário (login)"));
            }
            texto(&format!("usuário {n}: o login"), login, MAX_CURTO)?;
            if !logins.insert(login.to_lowercase()) {
                return Err(format!("o usuário {login:?} aparece mais de uma vez"));
            }
            if u.perfil.trim().is_empty() {
                return Err(format!("usuário {login:?}: informe o perfil"));
            }
            texto(
                &format!("usuário {login:?}: o perfil"),
                &u.perfil,
                MAX_CURTO,
            )?;
            texto(
                &format!("usuário {login:?}: a descrição"),
                &u.descricao,
                MAX_LONGO,
            )?;
        }
        Ok(())
    }

    /// Cópia com as pontas dos textos aparadas.
    pub fn normalizado(&self) -> Self {
        Self {
            url: self.url.trim().to_string(),
            tipo: self.tipo,
            autenticacao: self.autenticacao,
            observacoes: self.observacoes.trim().to_string(),
            usuarios: self
                .usuarios
                .iter()
                .map(|u| UsuarioTeste {
                    id: u.id.trim().to_string(),
                    usuario: u.usuario.trim().to_string(),
                    perfil: u.perfil.trim().to_string(),
                    descricao: u.descricao.trim().to_string(),
                })
                .collect(),
        }
    }

    /// Perfis distintos, na ordem em que aparecem.
    pub fn perfis(&self) -> Vec<&str> {
        let mut vistos = HashSet::new();
        self.usuarios
            .iter()
            .map(|u| u.perfil.as_str())
            .filter(|p| vistos.insert(p.to_lowercase()))
            .collect()
    }

    /// Identificador livre para um usuário novo (`u<N>`, depois do maior em uso).
    pub fn proximo_id(&self) -> String {
        let maior = self
            .usuarios
            .iter()
            .filter_map(|u| u.id.strip_prefix('u')?.parse::<u32>().ok())
            .max()
            .unwrap_or(0);
        format!("u{}", maior + 1)
    }
}

/// Conta, no chaveiro do sistema, da senha do usuário `id` no ambiente de teste
/// do `projeto`.
pub fn conta_senha(projeto: &str, id: &str) -> String {
    format!("teste:{projeto}/{id}")
}

fn id_valido(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 32
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn texto(nome: &str, valor: &str, max: usize) -> Result<(), String> {
    if valor.chars().count() > max {
        return Err(format!("{nome} passa de {max} caracteres"));
    }
    if valor
        .chars()
        .any(|c| c.is_control() && c != '\n' && c != '\t' && c != '\r')
    {
        return Err(format!("{nome} tem caracteres de controle"));
    }
    Ok(())
}

/// URL `http` ou `https` com host, sem espaços e sem usuário ou senha embutidos
/// (a senha só vai para o chaveiro).
fn validar_url(url: &str) -> Result<(), String> {
    let url = url.trim();
    if url.is_empty() {
        return Err("informe a URL do ambiente de teste".into());
    }
    texto("a URL", url, MAX_CURTO)?;
    if url.chars().any(char::is_whitespace) {
        return Err("a URL não pode ter espaços".into());
    }
    let resto = match url.split_once("://") {
        Some((esquema, resto))
            if esquema.eq_ignore_ascii_case("https") || esquema.eq_ignore_ascii_case("http") =>
        {
            resto
        }
        _ => return Err("a URL precisa começar com https:// ou http://".into()),
    };
    let autoridade = resto.split(['/', '?', '#']).next().unwrap_or_default();
    if autoridade.contains('@') {
        return Err(
            "tire o usuário e a senha da URL: cadastre-os na lista de usuários de teste".into(),
        );
    }
    let host = autoridade
        .rsplit_once(':')
        .filter(|(_, porta)| !porta.is_empty() && porta.chars().all(|c| c.is_ascii_digit()))
        .map_or(autoridade, |(h, _)| h);
    if host.is_empty() || host.starts_with('.') || host.ends_with('.') {
        return Err("a URL precisa ter o endereço do servidor (https://servidor/…)".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usuario(id: &str, login: &str, perfil: &str) -> UsuarioTeste {
        UsuarioTeste {
            id: id.into(),
            usuario: login.into(),
            perfil: perfil.into(),
            descricao: String::new(),
        }
    }

    fn ambiente(url: &str, usuarios: Vec<UsuarioTeste>) -> AmbienteTeste {
        AmbienteTeste {
            url: url.into(),
            usuarios,
            ..Default::default()
        }
    }

    #[test]
    fn aceita_urls_web() {
        for url in [
            "https://hml.exemplo.com.br/login",
            "http://localhost:8080",
            "HTTPS://10.0.0.5:8443/app?x=1#y",
            "https://[::1]:8443/",
        ] {
            assert_eq!(ambiente(url, vec![]).validar(), Ok(()), "{url}");
        }
    }

    #[test]
    fn recusa_urls_invalidas_e_senha_na_url() {
        for (url, trecho) in [
            ("", "informe a URL"),
            ("hml.exemplo.com", "https://"),
            ("ftp://hml.exemplo.com", "https://"),
            ("https://", "endereço do servidor"),
            ("https://:8080/x", "endereço do servidor"),
            ("https://hml exemplo.com", "espaços"),
            (
                "https://ana:segredo@hml.exemplo.com",
                "tire o usuário e a senha",
            ),
        ] {
            let e = ambiente(url, vec![]).validar().unwrap_err();
            assert!(e.contains(trecho), "{url}: {e}");
            assert!(!e.contains("segredo"), "{e}");
        }
    }

    #[test]
    fn usuarios_precisam_de_login_perfil_e_ids_unicos() {
        let url = "https://hml.exemplo.com";
        let ok = ambiente(
            url,
            vec![
                usuario("u1", "ana", "Atendente"),
                usuario("u2", "bia", "Gestor"),
            ],
        );
        assert_eq!(ok.validar(), Ok(()));
        for (usuarios, trecho) in [
            (vec![usuario("u1", " ", "Atendente")], "informe o usuário"),
            (vec![usuario("u1", "ana", "")], "informe o perfil"),
            (
                vec![usuario("u1", "ana", "A"), usuario("u2", "ANA", "B")],
                "mais de uma vez",
            ),
            (
                vec![usuario("u1", "ana", "A"), usuario("u1", "bia", "B")],
                "identificador repetido",
            ),
            (vec![usuario("", "ana", "A")], "identificador inválido"),
            (vec![usuario("u/1", "ana", "A")], "identificador inválido"),
        ] {
            let e = ambiente(url, usuarios).validar().unwrap_err();
            assert!(e.contains(trecho), "{e}");
        }
    }

    #[test]
    fn perfis_proximo_id_e_conta() {
        let a = ambiente(
            "https://x.com",
            vec![
                usuario("u1", "ana", "Atendente"),
                usuario("u7", "bia", "Gestor"),
                usuario("u3", "caio", "atendente"),
            ],
        );
        assert_eq!(a.perfis(), vec!["Atendente", "Gestor"]);
        assert_eq!(a.proximo_id(), "u8");
        assert_eq!(AmbienteTeste::default().proximo_id(), "u1");
        assert_eq!(conta_senha("faturamento", "u1"), "teste:faturamento/u1");
    }

    #[test]
    fn serializa_sem_senha_e_com_padroes() {
        let a: AmbienteTeste = serde_json::from_str(
            r#"{"url":"https://x.com","usuarios":[{"id":"u1","usuario":"ana","perfil":"A"}]}"#,
        )
        .unwrap();
        assert_eq!(a.tipo, TipoAplicacao::Web);
        assert_eq!(a.autenticacao, Autenticacao::UsuarioSenha);
        let json = serde_json::to_string(&a).unwrap();
        assert!(json.contains(r#""tipo":"web""#), "{json}");
        assert!(json.contains(r#""autenticacao":"usuario_senha""#), "{json}");
        assert!(!json.contains("senha\":"), "{json}");
    }
}
