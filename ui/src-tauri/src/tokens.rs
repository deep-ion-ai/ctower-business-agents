//! Comandos Tauri dos tokens CSS do projeto (#384), para o protótipo de telas
//! (#356): ler os tokens e o CSS, extrair do código da interface do projeto,
//! revisar um papel (informar o valor ou voltar ao do código) e aprovar. Os
//! tokens são um cadastro do projeto, não uma análise ([`cba_core::tokens`]).

use cba_core::dados::DiretorioDados;
use cba_core::tokens::{EstadoTokens, PapelToken, TokensProjeto};
use cba_index::TokenEstilo;
use cba_kb::layout;
use serde::Serialize;

use crate::analise::ambiente;

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

/// Tokens do projeto, como a tela mostra.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokensVista {
    /// `rascunho` (aguarda a revisão) ou `aprovado` (as telas usam).
    pub estado: EstadoTokens,
    pub tokens: Vec<TokenEstilo>,
    pub papeis: Vec<PapelToken>,
    /// O arquivo de variáveis CSS (`:root`).
    pub css: String,
    /// Papéis e tokens do código sem valor (a confirmar).
    pub a_confirmar: usize,
    pub extraido_ms: u64,
    pub atualizado_ms: u64,
    pub aprovado_ms: Option<u64>,
    /// O arquivo CSS, relativo à raiz da base.
    pub arquivo: String,
}

fn vista(kb: &cba_kb::Projeto, t: TokensProjeto) -> TokensVista {
    TokensVista {
        estado: t.estado,
        css: t.css(),
        a_confirmar: t.papeis_a_confirmar().count() + t.tokens_a_confirmar().count(),
        extraido_ms: t.extraido_ms,
        atualizado_ms: t.atualizado_ms,
        aprovado_ms: t.aprovado_ms,
        arquivo: kb.na_base(&layout::tokens_css()).to_string(),
        tokens: t.tokens,
        papeis: t.papeis,
    }
}

/// Os tokens do projeto; `None` antes da primeira extração.
pub fn tokens_do_projeto(
    dados: &DiretorioDados,
    projeto: &str,
) -> Result<Option<TokensVista>, String> {
    let amb = ambiente(dados, projeto)?;
    Ok(TokensProjeto::carregar(&amb.kb)
        .map_err(|e| e.to_string())?
        .map(|t| vista(&amb.kb, t)))
}

/// Extrai os tokens do código do projeto (de novo, se já houver): volta ao
/// rascunho e mantém os valores informados pelo analista.
pub fn extrair_tokens(dados: &DiretorioDados, projeto: &str) -> Result<TokensVista, String> {
    let amb = ambiente(dados, projeto)?;
    let t = TokensProjeto::extrair(&amb.kb, &amb.escopo).map_err(|e| e.to_string())?;
    Ok(vista(&amb.kb, t))
}

/// Revisão de um papel: com valor, o analista o informa; sem valor, volta ao do
/// código (ou a confirmar). Volta ao rascunho.
pub fn informar_token(
    dados: &DiretorioDados,
    projeto: &str,
    papel: &str,
    valor: Option<&str>,
) -> Result<TokensVista, String> {
    let amb = ambiente(dados, projeto)?;
    let valor = valor.map(str::trim).filter(|v| !v.is_empty());
    let (t, ()) = TokensProjeto::alterar(&amb.kb, |t, _| t.informar(papel, valor))
        .map_err(|e| e.to_string())?;
    Ok(vista(&amb.kb, t))
}

/// Aprovação do analista: as telas do protótipo passam a usar os tokens.
pub fn aprovar_tokens(dados: &DiretorioDados, projeto: &str) -> Result<TokensVista, String> {
    let amb = ambiente(dados, projeto)?;
    let (t, ()) = TokensProjeto::alterar(&amb.kb, |t, agora| {
        t.aprovar(agora);
        Ok(())
    })
    .map_err(|e| e.to_string())?;
    Ok(vista(&amb.kb, t))
}

#[tauri::command]
pub async fn tokens_css_projeto(projeto: String) -> Result<Option<TokensVista>, String> {
    let dados = dados()?;
    bloqueante(move || tokens_do_projeto(&dados, &projeto)).await
}

#[tauri::command]
pub async fn extrair_tokens_css_projeto(projeto: String) -> Result<TokensVista, String> {
    let dados = dados()?;
    bloqueante(move || extrair_tokens(&dados, &projeto)).await
}

#[tauri::command]
pub async fn informar_token_css_projeto(
    projeto: String,
    papel: String,
    valor: Option<String>,
) -> Result<TokensVista, String> {
    let dados = dados()?;
    bloqueante(move || informar_token(&dados, &projeto, &papel, valor.as_deref())).await
}

#[tauri::command]
pub async fn aprovar_tokens_css_projeto(projeto: String) -> Result<TokensVista, String> {
    let dados = dados()?;
    bloqueante(move || aprovar_tokens(&dados, &projeto)).await
}

#[cfg(test)]
#[path = "tokens_testes.rs"]
mod tests;
