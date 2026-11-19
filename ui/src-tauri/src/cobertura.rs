//! Comando Tauri da cobertura da base (#125): o mapa de cobertura por área e por
//! pasta e a sugestão da próxima análise, na tela do projeto. O cálculo (sem IA)
//! está em `cba_tools::cobertura`.

use cba_core::dados::DiretorioDados;
use cba_tools::cobertura::{self, Cobertura};

use crate::analise::ambiente;

pub fn cobertura_do_projeto(dados: &DiretorioDados, projeto: &str) -> Result<Cobertura, String> {
    let ambiente = ambiente(dados, projeto)?;
    cobertura::cobertura_do_projeto(&ambiente)
}

#[tauri::command]
pub async fn cobertura_projeto_app(projeto: String) -> Result<Cobertura, String> {
    let dados = DiretorioDados::padrao()
        .map_err(|e| format!("diretório de dados do app inacessível: {e}"))?;
    tauri::async_runtime::spawn_blocking(move || cobertura_do_projeto(&dados, &projeto))
        .await
        .map_err(|e| format!("falha interna: {e}"))?
}

#[cfg(test)]
#[path = "cobertura_testes.rs"]
mod tests;
