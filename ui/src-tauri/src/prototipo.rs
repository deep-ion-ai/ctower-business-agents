//! Comandos Tauri do visualizador do protótipo de telas (#382): o HTML gravado
//! de uma tela, para o `iframe` isolado da análise (`sandbox` vazio, sem script
//! nem rede). A leitura confere que o arquivo fica na pasta do protótipo da
//! análise ([`cba_tools::prototipo::ler_html_da_tela`]).
//!
//! E a exportação do protótipo numa pasta HTML para compartilhar (#380,
//! [`cba_tools::prototipo_exportacao`]): a tela escolhe a pasta de destino e,
//! se a pasta do protótipo já existir lá, pede confirmação antes de gravar por
//! cima.

use std::path::Path;

use cba_core::dados::DiretorioDados;
use cba_kb::Id;
use cba_tools::prototipo_exportacao::{ErroExportacaoPrototipo, exportar_prototipo};
use serde::Serialize;

use crate::analise::ambiente;

fn dados() -> Result<DiretorioDados, String> {
    DiretorioDados::padrao().map_err(|e| format!("diretório de dados do app inacessível: {e}"))
}

/// O HTML da tela `TL-<numero>` da análise; `None` enquanto a tela não tem
/// protótipo gravado.
pub fn html_da_tela(
    dados: &DiretorioDados,
    projeto: &str,
    analise: &str,
    numero: u32,
) -> Result<Option<String>, String> {
    let amb = ambiente(dados, projeto)?;
    let id = Id::analisar(analise.trim()).map_err(|e| e.to_string())?;
    let a = amb.analises.carregar(&id).map_err(|e| e.to_string())?;
    cba_tools::prototipo::ler_html_da_tela(&amb.kb, &a, numero)
}

#[tauri::command]
pub async fn html_da_tela_prototipo_app(
    projeto: String,
    analise: String,
    tela: u32,
) -> Result<Option<String>, String> {
    let dados = dados()?;
    tauri::async_runtime::spawn_blocking(move || html_da_tela(&dados, &projeto, &analise, tela))
        .await
        .map_err(|e| format!("falha interna: {e}"))?
}

/// Resultado da exportação, para a tela.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "situacao", rename_all = "snake_case")]
pub enum ExportacaoPrototipo {
    /// Pasta gravada: o caminho e quantas telas foram com e sem protótipo.
    Exportada {
        pasta: String,
        telas: usize,
        #[serde(rename = "semPrototipo")]
        sem_prototipo: usize,
    },
    /// A pasta já existe no destino: nada foi gravado; a tela pede confirmação
    /// e chama de novo com `substituir`.
    PastaExiste { pasta: String },
}

/// Exporta o protótipo da análise numa pasta dentro de `destino`.
pub fn exportar(
    dados: &DiretorioDados,
    projeto: &str,
    analise: &str,
    destino: &str,
    substituir: bool,
) -> Result<ExportacaoPrototipo, String> {
    let destino = destino.trim();
    if destino.is_empty() {
        return Err("escolha a pasta de destino".into());
    }
    let amb = ambiente(dados, projeto)?;
    let id = Id::analisar(analise.trim()).map_err(|e| e.to_string())?;
    let a = amb.analises.carregar(&id).map_err(|e| e.to_string())?;
    match exportar_prototipo(&amb.kb, &a, Path::new(destino), substituir) {
        Ok(feita) => Ok(ExportacaoPrototipo::Exportada {
            pasta: feita.pasta.display().to_string(),
            telas: feita.telas,
            sem_prototipo: feita.sem_prototipo,
        }),
        Err(ErroExportacaoPrototipo::PastaExiste(pasta)) => Ok(ExportacaoPrototipo::PastaExiste {
            pasta: pasta.display().to_string(),
        }),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub async fn exportar_prototipo_app(
    projeto: String,
    analise: String,
    destino: String,
    substituir: bool,
) -> Result<ExportacaoPrototipo, String> {
    let dados = dados()?;
    tauri::async_runtime::spawn_blocking(move || {
        exportar(&dados, &projeto, &analise, &destino, substituir)
    })
    .await
    .map_err(|e| format!("falha interna: {e}"))?
}

#[cfg(test)]
#[path = "prototipo_testes.rs"]
mod tests;
