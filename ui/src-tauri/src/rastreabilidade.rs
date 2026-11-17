//! Comandos Tauri da rastreabilidade (#121): a matriz da demanda (aba do painel
//! da demanda), a consulta reversa a partir de uma regra ou de um arquivo (tela
//! da regra na base) e a exportação da matriz para Excel (`.xlsx`) e Word.
//!
//! Tudo vem do índice da base (`cba_kb::indice::Indice::matriz` e
//! `Indice::dependentes`), sincronizado antes de cada consulta: a matriz reflete
//! na hora uma história ou um cenário recém-registrado, e apagar o índice só faz a
//! próxima consulta reconstruí-lo.

use std::path::PathBuf;

use cba_core::config::Config;
use cba_core::dados::DiretorioDados;
use cba_kb::indice::{AlvoReverso, Dependentes, Indice, Matriz};
use cba_kb::{Base, IdDemanda};
use cba_tools::rastreabilidade::{self, FormatoMatriz};

use crate::analise::ambiente;

fn indice(dados: &DiretorioDados, base: &Base) -> Result<Indice, String> {
    let mut indice = Indice::abrir(dados.arquivo_do_indice(), base)
        .map_err(|e| format!("índice da base: {e}"))?;
    indice
        .sincronizar()
        .map_err(|e| format!("índice da base: {e}"))?;
    Ok(indice)
}

/// Matriz de rastreabilidade da demanda do projeto (cadastrado).
pub fn matriz_demanda(
    dados: &DiretorioDados,
    projeto: &str,
    demanda: &str,
) -> Result<Matriz, String> {
    let id = IdDemanda::analisar(demanda.trim()).map_err(|e| e.to_string())?;
    let amb = ambiente(dados, projeto)?;
    indice(dados, &amb.base)?
        .matriz(amb.kb.nome(), id)
        .map_err(|e| format!("índice da base: {e}"))
}

/// O que depende da regra ou do arquivo, no projeto da base (o nome da pasta do
/// projeto na base, como a tela da base mostra).
pub fn dependentes(
    dados: &DiretorioDados,
    projeto: &str,
    alvo: &AlvoReverso,
) -> Result<Dependentes, String> {
    let config = Config::ler(dados).map_err(|e| format!("configuração do app ilegível: {e}"))?;
    let raiz = config
        .base()
        .ok_or("a pasta da base de conhecimento ainda não foi escolhida")?;
    let base = Base::abrir(raiz).map_err(|e| e.to_string())?;
    indice(dados, &base)?
        .dependentes(projeto, alvo)
        .map_err(|e| format!("índice da base: {e}"))
}

/// Exporta a matriz da demanda em `destino` e devolve o caminho gravado.
pub fn exportar_matriz(
    dados: &DiretorioDados,
    projeto: &str,
    demanda: &str,
    formato: FormatoMatriz,
    destino: &str,
) -> Result<PathBuf, String> {
    let destino = PathBuf::from(destino.trim());
    if destino.as_os_str().is_empty() {
        return Err("escolha onde gravar a matriz".into());
    }
    let m = matriz_demanda(dados, projeto, demanda)?;
    let bytes = rastreabilidade::exportar(&m, formato).map_err(|e| e.to_string())?;
    cba_kb::arquivo::gravar(&destino, &bytes)
        .map_err(|e| format!("falha ao gravar {}: {e}", destino.display()))?;
    Ok(destino)
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
pub async fn matriz_demanda_app(projeto: String, demanda: String) -> Result<Matriz, String> {
    let dados = dados()?;
    bloqueante(move || matriz_demanda(&dados, &projeto, &demanda)).await
}

#[tauri::command]
pub async fn dependentes_base_app(
    projeto: String,
    alvo: AlvoReverso,
) -> Result<Dependentes, String> {
    let dados = dados()?;
    bloqueante(move || dependentes(&dados, &projeto, &alvo)).await
}

#[tauri::command]
pub async fn exportar_matriz_app(
    projeto: String,
    demanda: String,
    formato: FormatoMatriz,
    destino: String,
) -> Result<String, String> {
    let dados = dados()?;
    bloqueante(move || {
        exportar_matriz(&dados, &projeto, &demanda, formato, &destino)
            .map(|p| p.display().to_string())
    })
    .await
}

#[cfg(test)]
#[path = "rastreabilidade_testes.rs"]
mod tests;
