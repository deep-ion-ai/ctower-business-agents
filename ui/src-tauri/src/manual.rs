//! Manual do usuário (#272) no app: gerar o manual a partir do painel da
//! funcionalidade, incluir, trocar e remover os prints dos passos, remover um
//! passo e ler um print para a prévia. A edição do texto dos passos, a dispensa
//! e a legenda do print vão pelas decisões da revisão
//! (`revisar_analise_projeto`). Cada mudança grava a análise e notifica a tela
//! pelo diário de eventos.

use cba_core::dados::DiretorioDados;
use cba_kb::{CaminhoRelativo, Id, IdFuncionalidade};
use cba_tools::manual::{self, NovoPrint};

use crate::analise::{AnaliseCriada, AnaliseResumo, ambiente, prompt_de_inicio};

fn id(analise: &str) -> Result<Id, String> {
    Id::analisar(analise.trim()).map_err(|_| format!("\"{analise}\" não é uma análise (A-NNNN)"))
}

/// Gera o manual da funcionalidade (ou a revisão do manual aprovado).
pub fn gerar_manual(
    dados: &DiretorioDados,
    projeto: &str,
    funcionalidade: &str,
) -> Result<AnaliseCriada, String> {
    let amb = ambiente(dados, projeto)?;
    let fid = IdFuncionalidade::analisar(funcionalidade.trim())
        .map_err(|_| format!("\"{funcionalidade}\" não é uma funcionalidade (F-<SIGLA>-NNN)"))?;
    let analise = manual::criar_manual(&amb, &fid).map_err(|e| e.to_string())?;
    Ok(AnaliseCriada {
        prompt_inicial: prompt_de_inicio(&analise.id),
        analise: AnaliseResumo::from(&analise),
    })
}

/// Inclui ou troca o print do passo.
pub fn anexar_print(
    dados: &DiretorioDados,
    projeto: &str,
    analise: &str,
    passo: u32,
    print: NovoPrint,
) -> Result<(), String> {
    let amb = ambiente(dados, projeto)?;
    manual::anexar_print(&amb, &id(analise)?, passo, print)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Remove o print do passo.
pub fn remover_print(
    dados: &DiretorioDados,
    projeto: &str,
    analise: &str,
    passo: u32,
) -> Result<(), String> {
    let amb = ambiente(dados, projeto)?;
    manual::remover_print(&amb, &id(analise)?, passo)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Remove o passo (e o print, se nenhum outro manual o usa).
pub fn remover_passo(
    dados: &DiretorioDados,
    projeto: &str,
    analise: &str,
    passo: u32,
) -> Result<(), String> {
    let amb = ambiente(dados, projeto)?;
    manual::remover_passo(&amb, &id(analise)?, passo)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Bytes do print citado no documento `documento` (relativo à raiz do projeto
/// na base) pelo destino `destino`, como está no Markdown; ou, sem documento, o
/// print pelo caminho relativo à raiz do projeto.
pub fn ler_print(
    dados: &DiretorioDados,
    projeto: &str,
    documento: Option<&str>,
    destino: &str,
) -> Result<Vec<u8>, String> {
    let amb = ambiente(dados, projeto)?;
    let arquivo = match documento {
        Some(d) => {
            let d = CaminhoRelativo::novo(d.trim()).map_err(|e| e.to_string())?;
            cba_docs::resolver(&d, destino)
                .ok_or_else(|| format!("{destino} não aponta para a base"))?
        }
        None => CaminhoRelativo::novo(destino.trim()).map_err(|e| e.to_string())?,
    };
    manual::ler_print(&amb.kb, &arquivo)
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
pub async fn anexar_print_manual_app(
    projeto: String,
    analise: String,
    passo: u32,
    print: NovoPrint,
) -> Result<(), String> {
    let dados = dados()?;
    bloqueante(move || anexar_print(&dados, &projeto, &analise, passo, print)).await
}

#[tauri::command]
pub async fn remover_print_manual_app(
    projeto: String,
    analise: String,
    passo: u32,
) -> Result<(), String> {
    let dados = dados()?;
    bloqueante(move || remover_print(&dados, &projeto, &analise, passo)).await
}

#[tauri::command]
pub async fn remover_passo_manual_app(
    projeto: String,
    analise: String,
    passo: u32,
) -> Result<(), String> {
    let dados = dados()?;
    bloqueante(move || remover_passo(&dados, &projeto, &analise, passo)).await
}

#[tauri::command]
pub async fn ler_print_manual_app(
    projeto: String,
    documento: Option<String>,
    destino: String,
) -> Result<tauri::ipc::Response, String> {
    let dados = dados()?;
    bloqueante(move || ler_print(&dados, &projeto, documento.as_deref(), &destino))
        .await
        .map(tauri::ipc::Response::new)
}
