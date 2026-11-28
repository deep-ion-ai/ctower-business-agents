//! Comandos Tauri da seção "Entregáveis" do painel da demanda (#463): a vista
//! (a análise validada de base, os tipos habilitados, os entregáveis gerados),
//! gerar e gerar de novo, editar, marcar como revisado e o destino (exportar,
//! publicar na base, comentário no rastreador).
//!
//! A lógica fica em [`cba_tools::entregaveis`]; aqui só se monta o ambiente do
//! projeto e se leva o trabalho para fora da thread da interface. Quem revisa e
//! envia é o usuário do sistema ([`crate::consultas::quem`]).

use std::path::PathBuf;

use cba_core::dados::DiretorioDados;
use cba_core::entregaveis::Finalidade;
use cba_kb::{Id, IdDemanda, IdFuncionalidade};
use cba_tools::entregaveis::{
    self, ComentarioPreparado, GeracaoAberta, PedidoDeGeracao, VistaEntregaveis,
};

use crate::analise::ambiente;

/// Quem aparece como revisor quando o sistema não diz o usuário.
pub const REVISOR_PADRAO: &str = "analista";

fn quem() -> String {
    crate::consultas::quem().unwrap_or_else(|| REVISOR_PADRAO.to_owned())
}

fn id_da_demanda(demanda: &str) -> Result<IdDemanda, String> {
    IdDemanda::analisar(demanda.trim()).map_err(|e| e.to_string())
}

pub fn vista(
    dados: &DiretorioDados,
    projeto: &str,
    demanda: &str,
) -> Result<VistaEntregaveis, String> {
    let amb = ambiente(dados, projeto)?;
    entregaveis::vista_da_demanda(&amb, id_da_demanda(demanda)?).map_err(|e| e.to_string())
}

pub fn gerar(
    dados: &DiretorioDados,
    projeto: &str,
    demanda: &str,
    tipo: &str,
    analise: &str,
    funcionalidade: Option<&str>,
    versao_inicial: bool,
) -> Result<GeracaoAberta, String> {
    let amb = ambiente(dados, projeto)?;
    let funcionalidade = funcionalidade
        .map(str::trim)
        .filter(|f| !f.is_empty())
        .map(|f| IdFuncionalidade::analisar(f).map_err(|e| e.to_string()))
        .transpose()?;
    entregaveis::gerar(
        &amb,
        PedidoDeGeracao {
            demanda: id_da_demanda(demanda)?,
            tipo: tipo.trim().to_owned(),
            analise: Id::analisar(analise.trim()).map_err(|e| e.to_string())?,
            funcionalidade,
            finalidade: if versao_inicial {
                Finalidade::VersaoInicial
            } else {
                Finalidade::Demanda
            },
        },
    )
    .map_err(|e| e.to_string())
}

pub fn editar(
    dados: &DiretorioDados,
    projeto: &str,
    demanda: &str,
    numero: u32,
    texto: &str,
) -> Result<(), String> {
    let amb = ambiente(dados, projeto)?;
    entregaveis::editar(&amb, id_da_demanda(demanda)?, numero, texto)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

pub fn marcar_revisado(
    dados: &DiretorioDados,
    projeto: &str,
    demanda: &str,
    numero: u32,
    por: &str,
) -> Result<(), String> {
    let amb = ambiente(dados, projeto)?;
    entregaveis::marcar_revisado(&amb, id_da_demanda(demanda)?, numero, por)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Exporta o Markdown e devolve o caminho gravado.
pub fn exportar(
    dados: &DiretorioDados,
    projeto: &str,
    demanda: &str,
    numero: u32,
    destino: &str,
    por: &str,
) -> Result<String, String> {
    let amb = ambiente(dados, projeto)?;
    let destino = PathBuf::from(destino.trim());
    if destino.as_os_str().is_empty() {
        return Err("escolha onde gravar o entregável".into());
    }
    let e = entregaveis::exportar(&amb, id_da_demanda(demanda)?, numero, &destino, por)
        .map_err(|e| e.to_string())?;
    Ok(e.envios.last().map(|v| v.onde.clone()).unwrap_or_default())
}

/// Publica na base e devolve onde ficou.
pub fn publicar(
    dados: &DiretorioDados,
    projeto: &str,
    demanda: &str,
    numero: u32,
    por: &str,
) -> Result<String, String> {
    let amb = ambiente(dados, projeto)?;
    let e = entregaveis::publicar(&amb, id_da_demanda(demanda)?, numero, por)
        .map_err(|e| e.to_string())?;
    Ok(e.envios.last().map(|v| v.onde.clone()).unwrap_or_default())
}

pub fn comentario(
    dados: &DiretorioDados,
    projeto: &str,
    demanda: &str,
    numero: u32,
    por: &str,
) -> Result<ComentarioPreparado, String> {
    let amb = ambiente(dados, projeto)?;
    entregaveis::preparar_comentario(&amb, id_da_demanda(demanda)?, numero, por)
        .map(|(_, c)| c)
        .map_err(|e| e.to_string())
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
pub async fn entregaveis_demanda_app(
    projeto: String,
    demanda: String,
) -> Result<VistaEntregaveis, String> {
    let dados = dados()?;
    bloqueante(move || vista(&dados, &projeto, &demanda)).await
}

#[tauri::command]
pub async fn gerar_entregavel_app(
    projeto: String,
    demanda: String,
    tipo: String,
    analise: String,
    funcionalidade: Option<String>,
    versao_inicial: bool,
) -> Result<GeracaoAberta, String> {
    let dados = dados()?;
    bloqueante(move || {
        gerar(
            &dados,
            &projeto,
            &demanda,
            &tipo,
            &analise,
            funcionalidade.as_deref(),
            versao_inicial,
        )
    })
    .await
}

#[tauri::command]
pub async fn editar_entregavel_app(
    projeto: String,
    demanda: String,
    numero: u32,
    texto: String,
) -> Result<(), String> {
    let dados = dados()?;
    bloqueante(move || editar(&dados, &projeto, &demanda, numero, &texto)).await
}

#[tauri::command]
pub async fn marcar_entregavel_revisado_app(
    projeto: String,
    demanda: String,
    numero: u32,
) -> Result<(), String> {
    let dados = dados()?;
    bloqueante(move || marcar_revisado(&dados, &projeto, &demanda, numero, &quem())).await
}

#[tauri::command]
pub async fn exportar_entregavel_app(
    projeto: String,
    demanda: String,
    numero: u32,
    destino: String,
) -> Result<String, String> {
    let dados = dados()?;
    bloqueante(move || exportar(&dados, &projeto, &demanda, numero, &destino, &quem())).await
}

#[tauri::command]
pub async fn publicar_entregavel_app(
    projeto: String,
    demanda: String,
    numero: u32,
) -> Result<String, String> {
    let dados = dados()?;
    bloqueante(move || publicar(&dados, &projeto, &demanda, numero, &quem())).await
}

#[tauri::command]
pub async fn comentario_entregavel_app(
    projeto: String,
    demanda: String,
    numero: u32,
) -> Result<ComentarioPreparado, String> {
    let dados = dados()?;
    bloqueante(move || comentario(&dados, &projeto, &demanda, numero, &quem())).await
}

#[cfg(test)]
mod tests {
    use cba_kb::NovaDemanda;

    use super::*;
    use crate::demandas::criar_demanda;
    use crate::teste_base::{PROJETO, cenario};

    #[test]
    fn demanda_sem_analise_validada_nao_gera() {
        let c = cenario();
        let d = criar_demanda(
            &c.dados,
            PROJETO,
            &NovaDemanda {
                titulo: "Parcelamento PJ".into(),
                descricao: None,
                chave: None,
                origem: None,
                tipo: None,
            },
        )
        .unwrap();
        let v = vista(&c.dados, PROJETO, &d.id).unwrap();
        assert!(v.analises.is_empty());
        assert!(v.entregaveis.is_empty());
        // Projeto sem configuração: os tipos que o app já gerava, histórias primeiro.
        assert_eq!(v.tipos[0].id, "historia-usuario");
        assert!(v.tipos[0].padrao);
        let erro = gerar(
            &c.dados,
            PROJETO,
            &d.id,
            "historia-usuario",
            "A-0001",
            None,
            false,
        )
        .unwrap_err();
        assert!(erro.contains("não é uma análise validada"), "{erro}");
        assert!(vista(&c.dados, PROJETO, "X-1").is_err());
        assert!(marcar_revisado(&c.dados, PROJETO, &d.id, 1, "ana").is_err());
    }
}
