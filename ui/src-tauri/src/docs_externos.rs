//! Comandos Tauri da documentação externa (#277): a situação da cópia do
//! repositório de docs e a sincronização manual, pelo botão do cartão do destino
//! da documentação. A sincronização em si é a de
//! [`cba_tools::docs_externos::sincronizar`].

use cba_core::dados::DiretorioDados;
use cba_kb::externos::{Mascarado, Pulado};
use cba_repos::demanda::Ferramentas;
use cba_tools::docs_externos::{self, ErroSincronizacao};
use serde::Serialize;

use crate::analise::ambiente;

/// Situação da documentação externa, para o cartão do destino.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SituacaoDocs {
    /// O projeto tem destino da documentação.
    pub com_destino: bool,
    /// Repositório e commit da última sincronização bem-sucedida.
    pub repositorio: Option<String>,
    pub branch: Option<String>,
    pub commit: Option<String>,
    /// Quando foi a última sincronização bem-sucedida (ms desde 1970).
    pub sincronizado_ms: Option<u64>,
    /// Erro da última tentativa (as cópias anteriores continuam valendo).
    pub erro: Option<String>,
    /// Por que a cópia pode estar desatualizada.
    pub aviso: Option<String>,
    pub importados: usize,
    /// Já na base (publicados por ela): não copiados.
    pub ja_na_base: usize,
    pub pulados: Vec<Pulado>,
    pub mascarados: Vec<Mascarado>,
}

/// Situação atual, sem rede.
pub fn situacao(dados: &DiretorioDados, projeto: &str) -> Result<SituacaoDocs, String> {
    let amb = ambiente(dados, projeto)?;
    let s = docs_externos::situacao(&amb);
    let estado = s.estado.unwrap_or_default();
    let fonte = estado.fonte.clone();
    Ok(SituacaoDocs {
        com_destino: s.destino.is_some(),
        repositorio: fonte.as_ref().map(|f| f.repositorio.clone()),
        branch: fonte.as_ref().map(|f| f.branch.clone()),
        commit: fonte.map(|f| f.commit),
        sincronizado_ms: estado.sincronizado_ms,
        erro: estado.erro,
        aviso: s.aviso,
        importados: s.importados,
        ja_na_base: estado.ja_na_base.len(),
        pulados: estado.pulados,
        mascarados: estado.mascarados,
    })
}

/// Sincroniza agora e devolve a situação nova. Falha de rede ou de credencial
/// volta como erro, já registrada na situação.
pub fn sincronizar(
    dados: &DiretorioDados,
    projeto: &str,
    f: &Ferramentas,
) -> Result<SituacaoDocs, String> {
    let amb = ambiente(dados, projeto)?;
    match docs_externos::sincronizar(&amb, f) {
        Ok(s) => {
            if let Some(aviso) = s.aviso_do_indice {
                tracing::warn!(aviso, "índice não sincronizado depois dos docs externos");
            }
        }
        Err(ErroSincronizacao::EmAndamento) => {}
        Err(e) => return Err(e.to_string()),
    }
    situacao(dados, projeto)
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
pub async fn situacao_docs_externos_app(projeto: String) -> Result<SituacaoDocs, String> {
    let dados = dados()?;
    bloqueante(move || situacao(&dados, &projeto)).await
}

#[tauri::command]
pub async fn sincronizar_docs_externos_app(projeto: String) -> Result<SituacaoDocs, String> {
    let dados = dados()?;
    bloqueante(move || sincronizar(&dados, &projeto, &Ferramentas::default())).await
}
