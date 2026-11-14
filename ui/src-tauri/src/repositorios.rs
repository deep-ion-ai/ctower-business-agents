//! Repositório de código e de issues de um projeto (#86): cadastro, prontidão e
//! análise a partir de uma issue.
//!
//! O cadastro guarda só o comando de credencial. O token é lido na hora de usar,
//! passa ao `jira`, ao `glab` ou ao `gh` só no ambiente do processo filho e não vai
//! a disco, a log nem à configuração do Claude.

use cba_core::analise::Fluxo;
use cba_core::dados::DiretorioDados;
use cba_index::Cadastro;
use cba_kb::frontmatter::Demanda;
use cba_repos::Repositorios;
use cba_repos::demanda::{Ferramentas, buscar_demanda};
use cba_repos::prontidao::{Prontidao, conferir};

use crate::analise::{AnaliseCriada, criar_analise_com_demanda};

fn cadastrado(dados: &DiretorioDados, projeto: &str) -> Result<cba_index::Projeto, String> {
    Cadastro::em(dados.projetos())
        .obter(projeto)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("o projeto \"{projeto}\" não está cadastrado"))
}

/// Grava os repositórios do projeto. O tipo de cada um e, no GitLab, host e
/// caminho não mudam depois do cadastro; o que ainda não foi cadastrado pode ser
/// acrescentado.
pub fn salvar_repositorios(
    dados: &DiretorioDados,
    projeto: &str,
    novos: Repositorios,
) -> Result<Repositorios, String> {
    let mut p = cadastrado(dados, projeto)?;
    p.repositorios.validar_troca(&novos)?;
    novos.validar()?;
    p.repositorios = novos;
    Cadastro::em(dados.projetos())
        .salvar(&p)
        .map_err(|e| e.to_string())?;
    Ok(p.repositorios)
}

/// Confere as integrações cadastradas, passo a passo.
pub fn prontidao(
    dados: &DiretorioDados,
    projeto: &str,
    ferramentas: &Ferramentas,
) -> Result<Prontidao, String> {
    Ok(conferir(
        &cadastrado(dados, projeto)?.repositorios,
        ferramentas,
    ))
}

/// Cria a análise com título e descrição da issue como entrada e o link dela em
/// `demanda.origem`.
pub fn criar_analise_de_issue(
    dados: &DiretorioDados,
    projeto: &str,
    fluxo: Fluxo,
    referencia: &str,
    ferramentas: &Ferramentas,
) -> Result<AnaliseCriada, String> {
    if fluxo == Fluxo::Reuniao {
        return Err(crate::demandas::FLUXO_REUNIAO.into());
    }
    let p = cadastrado(dados, projeto)?;
    let tracker = p
        .repositorios
        .tracker
        .as_ref()
        .ok_or("o projeto não tem repositório de issues cadastrado")?;
    let d = buscar_demanda(tracker, referencia, ferramentas).map_err(|e| e.to_string())?;
    let entrada = if d.descricao.trim().is_empty() {
        d.titulo.clone()
    } else {
        format!("{}\n\n{}", d.titulo, d.descricao.trim())
    };
    criar_analise_com_demanda(
        dados,
        projeto,
        fluxo,
        &entrada,
        Some(&d.titulo),
        Some(Demanda {
            id: None,
            origem: d.url,
            chave: Some(d.chave),
        }),
    )
}

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
pub async fn salvar_repositorios_app(
    projeto: String,
    repositorios: Repositorios,
) -> Result<Repositorios, String> {
    let dados = dados()?;
    bloqueante(move || salvar_repositorios(&dados, &projeto, repositorios)).await
}

#[tauri::command]
pub async fn prontidao_repositorios_app(projeto: String) -> Result<Prontidao, String> {
    let dados = dados()?;
    bloqueante(move || prontidao(&dados, &projeto, &Ferramentas::default())).await
}

#[tauri::command]
pub async fn criar_analise_de_demanda(
    projeto: String,
    fluxo: Fluxo,
    referencia: String,
) -> Result<AnaliseCriada, String> {
    let dados = dados()?;
    bloqueante(move || {
        criar_analise_de_issue(
            &dados,
            &projeto,
            fluxo,
            &referencia,
            &Ferramentas::default(),
        )
    })
    .await
}

#[cfg(all(test, unix))]
#[path = "repositorios_testes.rs"]
mod tests;
