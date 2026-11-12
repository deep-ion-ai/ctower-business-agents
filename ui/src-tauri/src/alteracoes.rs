//! Comandos Tauri do aviso de alteração na pasta do projeto e da proteção opcional
//! contra edição (#29).
//!
//! O aviso fica na tela da análise: os arquivos que as sessões ligadas a ela
//! alteraram, com o diff, e as opções de desfazer (com confirmação na tela) e de
//! manter ([`cba_session::alteracoes`]). A proteção é uma opção do preparo do
//! ambiente e das configurações (#39): regras `deny` no `settings.json` do usuário e
//! o `Bash` negado pelo hook ([`cba_session::protecao`]).

use cba_core::dados::DiretorioDados;
use cba_index::Cadastro;
use cba_kb::Id;
use cba_session::alteracoes::{self, AvisoAlteracao, Pedido};
use cba_session::preparo::Preparador;
use cba_session::protecao::{self, SituacaoProtecao};

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

fn preparador() -> Result<Preparador, String> {
    let exe = std::env::current_exe().map_err(|e| format!("caminho do executável do app: {e}"))?;
    Preparador::padrao(exe).map_err(|e| e.to_string())
}

fn id_da_analise(analise: &str) -> Result<Id, String> {
    Id::analisar(analise.trim()).map_err(|e| e.to_string())
}

fn projeto_cadastrado(dados: &DiretorioDados, projeto: &str) -> Result<cba_index::Projeto, String> {
    Cadastro::em(dados.projetos())
        .obter(projeto)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("o projeto \"{projeto}\" não está cadastrado"))
}

// ---------------------------------------------------------------------------
// Aviso de alteração

/// Arquivos alterados pelas sessões da análise, com o diff.
pub fn alteracoes_da_analise(
    dados: &DiretorioDados,
    projeto: &str,
    analise: &str,
) -> Result<AvisoAlteracao, String> {
    alteracoes::da_analise(dados, projeto, &id_da_analise(analise)?).map_err(|e| e.to_string())
}

/// Desfaz (ou mantém) os arquivos pedidos. A confirmação é da tela.
pub fn resolver_alteracoes(
    dados: &DiretorioDados,
    projeto: &str,
    analise: &str,
    pedidos: &[Pedido],
    desfazer: bool,
) -> Result<Vec<String>, String> {
    if pedidos.is_empty() {
        return Ok(Vec::new());
    }
    alteracoes::resolver_na_analise(dados, projeto, &id_da_analise(analise)?, pedidos, desfazer)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn alteracoes_da_analise_projeto(
    projeto: String,
    analise: String,
) -> Result<AvisoAlteracao, String> {
    let dados = dados()?;
    bloqueante(move || alteracoes_da_analise(&dados, &projeto, &analise)).await
}

#[tauri::command]
pub async fn desfazer_alteracoes_projeto(
    projeto: String,
    analise: String,
    pedidos: Vec<Pedido>,
) -> Result<Vec<String>, String> {
    let dados = dados()?;
    bloqueante(move || resolver_alteracoes(&dados, &projeto, &analise, &pedidos, true)).await
}

#[tauri::command]
pub async fn manter_alteracoes_projeto(
    projeto: String,
    analise: String,
    pedidos: Vec<Pedido>,
) -> Result<Vec<String>, String> {
    let dados = dados()?;
    bloqueante(move || resolver_alteracoes(&dados, &projeto, &analise, &pedidos, false)).await
}

// ---------------------------------------------------------------------------
// Proteção opcional contra edição

pub fn situacao_protecao(
    dados: &DiretorioDados,
    preparador: &Preparador,
    projeto: &str,
) -> Result<SituacaoProtecao, String> {
    protecao::situacao(dados, preparador, &projeto_cadastrado(dados, projeto)?)
        .map_err(|e| e.to_string())
}

/// Liga ou desliga a proteção e devolve a situação nova.
pub fn mudar_protecao(
    dados: &DiretorioDados,
    preparador: &Preparador,
    projeto: &str,
    ligada: bool,
) -> Result<SituacaoProtecao, String> {
    let cadastrado = projeto_cadastrado(dados, projeto)?;
    if ligada {
        protecao::ligar(dados, preparador, &cadastrado).map_err(|e| e.to_string())
    } else {
        protecao::desligar(dados, preparador, projeto).map_err(|e| e.to_string())?;
        protecao::situacao(dados, preparador, &cadastrado).map_err(|e| e.to_string())
    }
}

/// Situação da proteção (para a tela de configurações, #39).
#[tauri::command]
pub async fn situacao_protecao_projeto(projeto: String) -> Result<SituacaoProtecao, String> {
    let dados = dados()?;
    bloqueante(move || situacao_protecao(&dados, &preparador()?, &projeto)).await
}

/// Liga (`ligada: true`) ou desliga a proteção. Só chamado com o consentimento do
/// analista na tela.
#[tauri::command]
pub async fn mudar_protecao_projeto(
    projeto: String,
    ligada: bool,
) -> Result<SituacaoProtecao, String> {
    let dados = dados()?;
    bloqueante(move || mudar_protecao(&dados, &preparador()?, &projeto, ligada)).await
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use cba_session::clientes::Plataforma;
    use cba_session::preparo::ConfigClaude;
    use cba_session::sessoes::{RegistroSessao, Sessoes};
    use serde_json::json;

    use super::*;

    fn fixture() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/faturamento-exemplo")
            .canonicalize()
            .unwrap()
    }

    #[test]
    fn protecao_liga_e_desliga_pelo_projeto_cadastrado() {
        let dir = tempfile::tempdir().unwrap();
        let raiz = dunce::canonicalize(dir.path()).unwrap();
        let dados = DiretorioDados::em(raiz.join("dados"));
        let projeto = cba_index::Projeto::novo("Faturamento Exemplo", [fixture()]).unwrap();
        Cadastro::em(dados.projetos()).cadastrar(&projeto).unwrap();
        let home = raiz.join("home");
        let config = ConfigClaude::da_pasta_do_usuario(&home);
        fs::create_dir_all(config.settings.parent().unwrap()).unwrap();
        fs::write(&config.settings, r#"{"theme": "dark"}"#).unwrap();
        let exe = raiz.join(if cfg!(windows) { "cba.exe" } else { "cba" });
        let preparador = Preparador::novo(config.clone(), exe, Plataforma::atual()).unwrap();

        let s = situacao_protecao(&dados, &preparador, &projeto.id).unwrap();
        assert!(!s.ligada);
        let s = mudar_protecao(&dados, &preparador, &projeto.id, true).unwrap();
        assert!(s.ligada && s.completa);
        let json: serde_json::Value =
            serde_json::from_slice(&fs::read(&config.settings).unwrap()).unwrap();
        assert_eq!(json["theme"], "dark");
        assert_eq!(json["permissions"]["deny"].as_array().unwrap().len(), 1);
        let s = mudar_protecao(&dados, &preparador, &projeto.id, false).unwrap();
        assert!(!s.ligada);
        let json: serde_json::Value =
            serde_json::from_slice(&fs::read(&config.settings).unwrap()).unwrap();
        assert_eq!(json, json!({"theme": "dark"}));
        assert!(situacao_protecao(&dados, &preparador, "outro").is_err());
    }

    #[test]
    fn aviso_vazio_sem_sessoes_e_pedido_vazio_nao_faz_nada() {
        let dir = tempfile::tempdir().unwrap();
        let dados = DiretorioDados::em(dir.path().join("dados"));
        let aviso = alteracoes_da_analise(&dados, "faturamento-exemplo", "A-0001").unwrap();
        assert!(aviso.arquivos.is_empty());
        assert!(alteracoes_da_analise(&dados, "faturamento-exemplo", "x").is_err());
        assert!(
            resolver_alteracoes(&dados, "faturamento-exemplo", "A-0001", &[], true)
                .unwrap()
                .is_empty()
        );
        // Pedido de uma sessão que não é da análise é recusado.
        Sessoes::do_diretorio(&dados)
            .alterar("s1", |_| {
                Some(RegistroSessao::novo("s1", "faturamento-exemplo", 1))
            })
            .unwrap();
        let pedido = Pedido {
            sessao: "s1".into(),
            pasta: fixture(),
            caminho: "db/01_tabelas.sql".into(),
            hash_atual: None,
        };
        assert!(
            resolver_alteracoes(&dados, "faturamento-exemplo", "A-0001", &[pedido], true).is_err()
        );
    }
}
