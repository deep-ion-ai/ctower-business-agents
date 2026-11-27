//! Comandos Tauri da publicação dos documentos aprovados no repositório (#276):
//! a prévia (o que muda na branch base do destino) e a publicação numa branch
//! própria com PR/MR. A lógica está em `cba_tools::publicacao`; o token do
//! repositório vem do chaveiro e só vai para o ambiente do `git` e da CLI.

use cba_core::dados::DiretorioDados;
use cba_repos::demanda::Ferramentas;
use cba_tools::publicacao::{self, Alvo, Previa, Publicacao};

use crate::analise::ambiente;

/// O que muda no destino se a documentação aprovada for publicada agora.
pub fn previa(
    dados: &DiretorioDados,
    projeto: &str,
    ferramentas: &Ferramentas,
) -> Result<Previa, String> {
    let a = ambiente(dados, projeto)?;
    publicacao::previa(
        ferramentas,
        &Alvo {
            projeto: &a.kb,
            nome: &a.projeto.nome,
            repositorios: &a.projeto.repositorios,
            escopo: Some(&a.escopo),
            mascarar_demanda: a.projeto.mascarar_demanda,
        },
    )
}

/// Publica a documentação aprovada e devolve o link do PR/MR.
pub fn publicar(
    dados: &DiretorioDados,
    projeto: &str,
    ferramentas: &Ferramentas,
) -> Result<Publicacao, String> {
    let a = ambiente(dados, projeto)?;
    publicacao::publicar(
        ferramentas,
        &Alvo {
            projeto: &a.kb,
            nome: &a.projeto.nome,
            repositorios: &a.projeto.repositorios,
            escopo: Some(&a.escopo),
            mascarar_demanda: a.projeto.mascarar_demanda,
        },
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
pub async fn previa_publicacao_docs_app(projeto: String) -> Result<Previa, String> {
    let dados = dados()?;
    bloqueante(move || previa(&dados, &projeto, &Ferramentas::default())).await
}

#[tauri::command]
pub async fn publicar_docs_app(projeto: String) -> Result<Publicacao, String> {
    let dados = dados()?;
    bloqueante(move || publicar(&dados, &projeto, &Ferramentas::default())).await
}

#[cfg(all(test, unix))]
#[path = "publicacao_testes.rs"]
mod tests;
