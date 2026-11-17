//! Aba "Ambiente de teste" do projeto, nas configurações: a URL da aplicação em
//! teste, os usuários de teste e os perfis de cada um, para simular o uso e não
//! deixar a análise limitada ao código. Nesta etapa, só aplicações web com login
//! por usuário e senha, e nada disso vai ainda para o agente.
//!
//! O cadastro (`projeto.json`, no diretório de dados) guarda a URL, os usuários e
//! os perfis. A senha vai da tela direto para o chaveiro do sistema, na conta
//! [`conta_senha`] do usuário, e nunca volta para a tela, para o cadastro, para a
//! base nem para o log: a tela só sabe quais usuários têm senha guardada.

use std::collections::HashSet;
use std::fmt;

use cba_core::dados::DiretorioDados;
use cba_index::ambiente_teste::{AmbienteTeste, UsuarioTeste, conta_senha};
use cba_index::{Cadastro, Projeto};
use cba_repos::Secret;
use cba_repos::chaveiro::Chaveiro;
use cba_repos::demanda::Ferramentas;
use serde::{Deserialize, Serialize};

/// Tamanho máximo de uma senha.
const MAX_SENHA: usize = 1000;

/// Usuário como a tela o envia ao salvar.
#[derive(Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntradaUsuario {
    /// Identificador de um usuário já salvo; `None` num usuário novo.
    #[serde(default)]
    pub id: Option<String>,
    pub usuario: String,
    pub perfil: String,
    #[serde(default)]
    pub descricao: String,
    /// Senha nova. `None` ou vazia: mantém a guardada.
    #[serde(default)]
    pub senha: Option<String>,
}

impl fmt::Debug for EntradaUsuario {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EntradaUsuario")
            .field("id", &self.id)
            .field("usuario", &self.usuario)
            .field("perfil", &self.perfil)
            .field("senha", &self.senha.as_ref().map(|_| "***"))
            .finish()
    }
}

/// Ambiente como a tela o envia ao salvar.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntradaAmbiente {
    pub url: String,
    #[serde(default)]
    pub observacoes: String,
    #[serde(default)]
    pub usuarios: Vec<EntradaUsuario>,
}

/// O que a tela mostra do ambiente de teste: nunca a senha.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SituacaoAmbienteTeste {
    pub ambiente: Option<AmbienteTeste>,
    /// Identificadores dos usuários com senha guardada no chaveiro.
    pub com_senha: Vec<String>,
}

fn cadastrado(dados: &DiretorioDados, projeto: &str) -> Result<Projeto, String> {
    Cadastro::em(dados.projetos())
        .obter(projeto)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("o projeto \"{projeto}\" não está cadastrado"))
}

fn situacao_de(
    projeto: &Projeto,
    chaveiro: &dyn Chaveiro,
) -> Result<SituacaoAmbienteTeste, String> {
    let mut com_senha = Vec::new();
    if let Some(a) = &projeto.ambiente_teste {
        for u in &a.usuarios {
            if chaveiro.ler(&conta_senha(&projeto.id, &u.id))?.is_some() {
                com_senha.push(u.id.clone());
            }
        }
    }
    Ok(SituacaoAmbienteTeste {
        ambiente: projeto.ambiente_teste.clone(),
        com_senha,
    })
}

/// O ambiente de teste do projeto e quais usuários têm senha guardada.
pub fn situacao(
    dados: &DiretorioDados,
    chaveiro: &dyn Chaveiro,
    projeto: &str,
) -> Result<SituacaoAmbienteTeste, String> {
    situacao_de(&cadastrado(dados, projeto)?, chaveiro)
}

/// Grava o ambiente de teste. Usuário novo ganha um identificador; senha
/// informada vai para o chaveiro; a senha de quem saiu da lista é apagada.
///
/// A ordem protege o cadastro: primeiro valida tudo, depois grava as senhas,
/// então o cadastro e, só no fim, apaga as senhas de quem saiu. Se o cadastro
/// falhar, a senha nova de um usuário novo pode ficar órfã no chaveiro, mas
/// nada do que já estava salvo se perde.
pub fn salvar(
    dados: &DiretorioDados,
    chaveiro: &dyn Chaveiro,
    projeto: &str,
    entrada: EntradaAmbiente,
) -> Result<SituacaoAmbienteTeste, String> {
    let mut p = cadastrado(dados, projeto)?;
    let anterior = p.ambiente_teste.clone().unwrap_or_default();
    let antigos: HashSet<&str> = anterior.usuarios.iter().map(|u| u.id.as_str()).collect();

    let mut novo = AmbienteTeste {
        url: entrada.url,
        observacoes: entrada.observacoes,
        ..Default::default()
    };
    // Proximo identificador livre também entre os antigos, para não reaproveitar
    // o de quem acabou de sair (a senha dele ainda está no chaveiro).
    let mut reservados = anterior.clone();
    let mut senhas = Vec::new();
    for u in entrada.usuarios {
        let id = match u.id.as_deref().map(str::trim) {
            Some(id) if antigos.contains(id) => id.to_string(),
            _ => {
                let id = reservados.proximo_id();
                reservados.usuarios.push(UsuarioTeste {
                    id: id.clone(),
                    ..Default::default()
                });
                id
            }
        };
        if let Some(s) = u.senha.filter(|s| !s.is_empty()) {
            if s.chars().count() > MAX_SENHA {
                return Err(format!(
                    "a senha do usuário {:?} passa de {MAX_SENHA} caracteres",
                    u.usuario.trim()
                ));
            }
            senhas.push((id.clone(), Secret::new(s)));
        }
        novo.usuarios.push(UsuarioTeste {
            id,
            usuario: u.usuario,
            perfil: u.perfil,
            descricao: u.descricao,
        });
    }
    let novo = novo.normalizado();
    novo.validar()?;

    for (id, senha) in &senhas {
        chaveiro.gravar(&conta_senha(&p.id, id), senha)?;
    }
    p.ambiente_teste = Some(novo);
    Cadastro::em(dados.projetos())
        .salvar(&p)
        .map_err(|e| e.to_string())?;
    let atuais: HashSet<&str> = p
        .ambiente_teste
        .iter()
        .flat_map(|a| a.usuarios.iter().map(|u| u.id.as_str()))
        .collect();
    for id in antigos.difference(&atuais) {
        chaveiro.apagar(&conta_senha(&p.id, id))?;
    }
    tracing::info!(
        projeto = p.id,
        usuarios = atuais.len(),
        senhas_novas = senhas.len(),
        "ambiente de teste salvo"
    );
    situacao_de(&p, chaveiro)
}

/// Apaga do chaveiro as senhas de todos os usuários de teste do projeto.
pub fn apagar_senhas(projeto: &Projeto, chaveiro: &dyn Chaveiro) -> Result<(), String> {
    if let Some(a) = &projeto.ambiente_teste {
        for u in &a.usuarios {
            chaveiro.apagar(&conta_senha(&projeto.id, &u.id))?;
        }
    }
    Ok(())
}

/// Tira o ambiente de teste do projeto e apaga as senhas do chaveiro.
pub fn remover(
    dados: &DiretorioDados,
    chaveiro: &dyn Chaveiro,
    projeto: &str,
) -> Result<SituacaoAmbienteTeste, String> {
    let mut p = cadastrado(dados, projeto)?;
    let anterior = p.clone();
    p.ambiente_teste = None;
    Cadastro::em(dados.projetos())
        .salvar(&p)
        .map_err(|e| e.to_string())?;
    apagar_senhas(&anterior, chaveiro)?;
    tracing::info!(projeto = p.id, "ambiente de teste removido");
    Ok(SituacaoAmbienteTeste::default())
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
pub async fn ambiente_teste_app(projeto: String) -> Result<SituacaoAmbienteTeste, String> {
    let dados = dados()?;
    bloqueante(move || situacao(&dados, Ferramentas::default().chaveiro.as_ref(), &projeto)).await
}

#[tauri::command]
pub async fn salvar_ambiente_teste_app(
    projeto: String,
    ambiente: EntradaAmbiente,
) -> Result<SituacaoAmbienteTeste, String> {
    let dados = dados()?;
    bloqueante(move || {
        salvar(
            &dados,
            Ferramentas::default().chaveiro.as_ref(),
            &projeto,
            ambiente,
        )
    })
    .await
}

#[tauri::command]
pub async fn remover_ambiente_teste_app(projeto: String) -> Result<SituacaoAmbienteTeste, String> {
    let dados = dados()?;
    bloqueante(move || remover(&dados, Ferramentas::default().chaveiro.as_ref(), &projeto)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use cba_repos::chaveiro::ChaveiroMemoria;

    fn projeto_cadastrado(d: &std::path::Path) -> DiretorioDados {
        let dados = DiretorioDados::em(d.join("dados"));
        let pasta = d.join("codigo");
        std::fs::create_dir_all(&pasta).unwrap();
        let p = Projeto::novo("Faturamento", [&pasta]).unwrap();
        Cadastro::em(dados.projetos()).cadastrar(&p).unwrap();
        dados
    }

    fn entrada(id: Option<&str>, login: &str, perfil: &str, senha: Option<&str>) -> EntradaUsuario {
        EntradaUsuario {
            id: id.map(str::to_string),
            usuario: login.into(),
            perfil: perfil.into(),
            descricao: String::new(),
            senha: senha.map(str::to_string),
        }
    }

    fn ambiente(usuarios: Vec<EntradaUsuario>) -> EntradaAmbiente {
        EntradaAmbiente {
            url: " https://hml.exemplo.com/login ".into(),
            observacoes: "Precisa de VPN.".into(),
            usuarios,
        }
    }

    fn senha(c: &ChaveiroMemoria, id: &str) -> Option<String> {
        c.ler(&conta_senha("faturamento", id))
            .unwrap()
            .map(|s| s.expose().to_string())
    }

    #[test]
    fn salva_usuarios_com_senha_no_chaveiro_e_nunca_no_cadastro() {
        let d = tempfile::tempdir().unwrap();
        let dados = projeto_cadastrado(d.path());
        let c = ChaveiroMemoria::default();
        assert_eq!(
            situacao(&dados, &c, "faturamento").unwrap(),
            SituacaoAmbienteTeste::default()
        );

        let s = salvar(
            &dados,
            &c,
            "faturamento",
            ambiente(vec![
                entrada(None, " ana ", "Atendente", Some("senha-da-ana")),
                entrada(None, "bia", "Gestor", None),
            ]),
        )
        .unwrap();
        let a = s.ambiente.clone().unwrap();
        assert_eq!(a.url, "https://hml.exemplo.com/login");
        assert_eq!(
            a.usuarios
                .iter()
                .map(|u| (u.id.as_str(), u.usuario.as_str()))
                .collect::<Vec<_>>(),
            vec![("u1", "ana"), ("u2", "bia")]
        );
        assert_eq!(s.com_senha, vec!["u1"]);
        assert_eq!(senha(&c, "u1").as_deref(), Some("senha-da-ana"));

        // Nem o cadastro em disco nem o que vai para a tela têm a senha.
        let disco =
            std::fs::read_to_string(dados.projetos().join("faturamento/projeto.json")).unwrap();
        assert!(disco.contains("hml.exemplo.com") && !disco.contains("senha-da-ana"));
        assert!(!serde_json::to_string(&s).unwrap().contains("senha-da-ana"));
        assert_eq!(situacao(&dados, &c, "faturamento").unwrap(), s);
    }

    #[test]
    fn senha_vazia_mantem_trocar_login_mantem_e_quem_sai_perde_a_senha() {
        let d = tempfile::tempdir().unwrap();
        let dados = projeto_cadastrado(d.path());
        let c = ChaveiroMemoria::default();
        salvar(
            &dados,
            &c,
            "faturamento",
            ambiente(vec![
                entrada(None, "ana", "Atendente", Some("s-ana")),
                entrada(None, "bia", "Gestor", Some("s-bia")),
            ]),
        )
        .unwrap();

        // Ana troca de login sem informar senha; Bia sai; Caio entra.
        let s = salvar(
            &dados,
            &c,
            "faturamento",
            ambiente(vec![
                entrada(Some("u1"), "ana.souza", "Atendente", Some("")),
                entrada(None, "caio", "Auditor", Some("s-caio")),
            ]),
        )
        .unwrap();
        let a = s.ambiente.unwrap();
        assert_eq!(a.usuarios[0].id, "u1");
        assert_eq!(a.usuarios[0].usuario, "ana.souza");
        // O identificador de quem saiu não é reaproveitado.
        assert_eq!(a.usuarios[1].id, "u3");
        assert_eq!(senha(&c, "u1").as_deref(), Some("s-ana"));
        assert_eq!(senha(&c, "u2"), None);
        assert_eq!(senha(&c, "u3").as_deref(), Some("s-caio"));
        assert_eq!(s.com_senha, vec!["u1", "u3"]);

        // Um id desconhecido vindo da tela vira usuário novo.
        let s = salvar(
            &dados,
            &c,
            "faturamento",
            ambiente(vec![entrada(Some("u99"), "dani", "Gestor", None)]),
        )
        .unwrap();
        assert_eq!(s.ambiente.unwrap().usuarios[0].id, "u4");
    }

    #[test]
    fn recusa_invalido_sem_tocar_no_chaveiro_nem_no_cadastro() {
        let d = tempfile::tempdir().unwrap();
        let dados = projeto_cadastrado(d.path());
        let c = ChaveiroMemoria::default();
        let e = salvar(
            &dados,
            &c,
            "faturamento",
            ambiente(vec![entrada(None, "ana", "", Some("s-ana"))]),
        )
        .unwrap_err();
        assert!(e.contains("informe o perfil"), "{e}");
        assert_eq!(senha(&c, "u1"), None);
        assert!(
            situacao(&dados, &c, "faturamento")
                .unwrap()
                .ambiente
                .is_none()
        );

        let mut sem_url = ambiente(vec![]);
        sem_url.url = "https://ana:s@hml.exemplo.com".into();
        let e = salvar(&dados, &c, "faturamento", sem_url).unwrap_err();
        assert!(e.contains("tire o usuário e a senha"), "{e}");

        let e = salvar(&dados, &c, "nao-existe", ambiente(vec![])).unwrap_err();
        assert!(e.contains("não está cadastrado"), "{e}");
    }

    #[test]
    fn remover_apaga_o_ambiente_e_as_senhas() {
        let d = tempfile::tempdir().unwrap();
        let dados = projeto_cadastrado(d.path());
        let c = ChaveiroMemoria::default();
        salvar(
            &dados,
            &c,
            "faturamento",
            ambiente(vec![entrada(None, "ana", "Atendente", Some("s-ana"))]),
        )
        .unwrap();
        assert_eq!(
            remover(&dados, &c, "faturamento").unwrap(),
            SituacaoAmbienteTeste::default()
        );
        assert_eq!(senha(&c, "u1"), None);
        assert!(
            situacao(&dados, &c, "faturamento")
                .unwrap()
                .ambiente
                .is_none()
        );
    }

    #[test]
    fn debug_da_entrada_esconde_a_senha() {
        let e = entrada(None, "ana", "A", Some("segredo"));
        assert!(!format!("{e:?}").contains("segredo"));
    }
}
