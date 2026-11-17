//! Comandos Tauri das premissas, decisões, riscos e restrições da demanda (#122):
//! a aba do painel da demanda (os registros, as propostas pendentes do agente e o
//! catálogo de pessoas para escolher quem), criar e editar à mão, validar uma
//! premissa e aceitar ou recusar uma proposta.
//!
//! A lógica fica em `cba_kb::registros` (arquivos, numeração, histórico,
//! validação de premissa) e em [`cba_tools::registros`] (propostas); aqui só se
//! monta o ambiente do projeto, se converte para a vista da tela e se leva o
//! trabalho para fora da thread da interface.

use cba_core::dados::DiretorioDados;
use cba_core::tempo;
use cba_kb::registros::{
    EventoRegistro, Nivel, StatusPremissa, TipoRestricao, Validacao, ValidacaoDePremissa,
};
use cba_kb::{Categoria, DadosRegistro, IdDemanda, IdRegistro, NovoRegistro, RegistroLido};
use cba_tools::registros::{self as propostas, Proposta};
use serde::Serialize;

use crate::analise::ambiente;

/// Registro como a tela mostra.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistroVista {
    /// `PR-0001`.
    pub id: String,
    pub categoria: Categoria,
    pub titulo: String,
    pub detalhe: Option<String>,
    pub quem: Option<String>,
    /// Slug da pessoa no catálogo.
    pub pessoa: Option<String>,
    /// Documento de origem (ata ou análise), relativo à raiz da base.
    pub origem: Option<String>,
    /// `R-0001/D-1`, `A-0001/3`.
    pub referencia: Option<String>,
    /// `AAAA-MM-DD`.
    pub criado: String,
    pub status: Option<StatusPremissa>,
    pub validacao: Option<Validacao>,
    pub decidido_em: Option<String>,
    pub alternativas: Vec<String>,
    pub probabilidade: Option<Nivel>,
    pub impacto: Option<Nivel>,
    pub resposta: Option<String>,
    pub restricao: Option<TipoRestricao>,
    pub historico: Vec<EventoRegistro>,
    /// O registro, relativo à raiz da base (para abrir na base).
    pub documento: String,
}

/// Pessoa do catálogo, para escolher quem assumiu, decidiu ou responde.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PessoaVista {
    pub slug: String,
    pub nome: String,
    pub papel: Option<String>,
}

/// O que a aba de premissas, decisões, riscos e restrições mostra.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistrosDaDemanda {
    /// Em ordem de identificador (premissas, decisões, riscos, restrições).
    pub registros: Vec<RegistroVista>,
    /// Propostas do agente à espera do analista.
    pub propostas: Vec<Proposta>,
    pub pessoas: Vec<PessoaVista>,
    /// Pasta dos registros, relativa à raiz da base.
    pub pasta: String,
}

fn vista(kb: &cba_kb::Projeto, r: &RegistroLido) -> RegistroVista {
    let f = r.frontmatter();
    RegistroVista {
        id: r.id.to_string(),
        categoria: f.categoria(),
        titulo: f.titulo.clone(),
        detalhe: r.detalhe(),
        quem: f.quem.clone(),
        pessoa: f.pessoa.clone(),
        origem: f.origem.as_ref().map(|o| kb.na_base(o).as_str().to_owned()),
        referencia: f.referencia.clone(),
        criado: f.criado.to_string(),
        status: f.status,
        validacao: f.validacao.clone(),
        decidido_em: f.decidido_em.map(|d| d.to_string()),
        alternativas: f.alternativas.clone(),
        probabilidade: f.probabilidade,
        impacto: f.impacto,
        resposta: f.resposta.clone(),
        restricao: f.restricao,
        historico: f.historico.clone(),
        documento: kb.na_base(&r.arquivo).as_str().to_owned(),
    }
}

fn id_da_demanda(demanda: &str) -> Result<IdDemanda, String> {
    IdDemanda::analisar(demanda.trim()).map_err(|e| e.to_string())
}

fn id_do_registro(registro: &str) -> Result<IdRegistro, String> {
    IdRegistro::analisar(registro.trim()).map_err(|e| e.to_string())
}

/// Registros, propostas pendentes e pessoas do catálogo da demanda.
pub fn registros_da_demanda(
    dados: &DiretorioDados,
    projeto: &str,
    demanda: &str,
) -> Result<RegistrosDaDemanda, String> {
    let amb = ambiente(dados, projeto)?;
    let kb = &amb.kb;
    let id = id_da_demanda(demanda)?;
    let registros = kb
        .registros_da_demanda(id)
        .map_err(|e| e.to_string())?
        .iter()
        .map(|r| vista(kb, r))
        .collect();
    let propostas = propostas::propostas(&amb, id).map_err(|e| e.to_string())?;
    let pessoas = kb
        .pessoas()
        .unwrap_or_default()
        .into_iter()
        .map(|p| PessoaVista {
            nome: p.nome().to_owned(),
            papel: p.papel_sugerido().map(|s| s.papel),
            slug: p.slug,
        })
        .collect();
    let pasta = kb.pasta_dos_registros(id).map_err(|e| e.to_string())?;
    Ok(RegistrosDaDemanda {
        registros,
        propostas,
        pessoas,
        pasta: kb.na_base(&pasta).as_str().to_owned(),
    })
}

/// Cria um registro à mão.
pub fn criar_registro(
    dados: &DiretorioDados,
    projeto: &str,
    demanda: &str,
    categoria: Categoria,
    campos: DadosRegistro,
) -> Result<RegistroVista, String> {
    let amb = ambiente(dados, projeto)?;
    let novo = NovoRegistro {
        categoria,
        dados: campos,
        origem: None,
        referencia: None,
        evento: "criado à mão".into(),
    };
    let r = amb
        .kb
        .criar_registro(id_da_demanda(demanda)?, &novo, tempo::hoje())
        .map_err(|e| e.to_string())?;
    Ok(vista(&amb.kb, &r))
}

/// Edita os campos de um registro.
pub fn editar_registro(
    dados: &DiretorioDados,
    projeto: &str,
    registro: &str,
    campos: DadosRegistro,
) -> Result<RegistroVista, String> {
    let amb = ambiente(dados, projeto)?;
    let r = amb
        .kb
        .editar_registro(id_do_registro(registro)?, &campos, tempo::hoje())
        .map_err(|e| e.to_string())?;
    Ok(vista(&amb.kb, &r))
}

/// Valida, invalida ou volta a premissa para não validada.
pub fn validar_premissa(
    dados: &DiretorioDados,
    projeto: &str,
    registro: &str,
    validacao: ValidacaoDePremissa,
) -> Result<RegistroVista, String> {
    let amb = ambiente(dados, projeto)?;
    let r = propostas::validar_premissa(&amb, id_do_registro(registro)?, &validacao, tempo::hoje())
        .map_err(|e| e.to_string())?;
    Ok(vista(&amb.kb, &r))
}

/// Aceita uma proposta do agente, com os ajustes do analista.
pub fn aceitar_proposta(
    dados: &DiretorioDados,
    projeto: &str,
    demanda: &str,
    referencia: &str,
    ajustes: Option<DadosRegistro>,
) -> Result<RegistroVista, String> {
    let amb = ambiente(dados, projeto)?;
    let r = propostas::aceitar_proposta(
        &amb,
        id_da_demanda(demanda)?,
        referencia,
        ajustes,
        tempo::hoje(),
    )
    .map_err(|e| e.to_string())?;
    Ok(vista(&amb.kb, &r))
}

/// Recusa uma proposta do agente.
pub fn recusar_proposta(
    dados: &DiretorioDados,
    projeto: &str,
    demanda: &str,
    referencia: &str,
) -> Result<(), String> {
    let amb = ambiente(dados, projeto)?;
    propostas::recusar_proposta(&amb, id_da_demanda(demanda)?, referencia)
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
pub async fn registros_demanda_app(
    projeto: String,
    demanda: String,
) -> Result<RegistrosDaDemanda, String> {
    let dados = dados()?;
    bloqueante(move || registros_da_demanda(&dados, &projeto, &demanda)).await
}

#[tauri::command]
pub async fn criar_registro_app(
    projeto: String,
    demanda: String,
    categoria: Categoria,
    campos: DadosRegistro,
) -> Result<RegistroVista, String> {
    let dados = dados()?;
    bloqueante(move || criar_registro(&dados, &projeto, &demanda, categoria, campos)).await
}

#[tauri::command]
pub async fn editar_registro_app(
    projeto: String,
    registro: String,
    campos: DadosRegistro,
) -> Result<RegistroVista, String> {
    let dados = dados()?;
    bloqueante(move || editar_registro(&dados, &projeto, &registro, campos)).await
}

#[tauri::command]
pub async fn validar_premissa_app(
    projeto: String,
    registro: String,
    validacao: ValidacaoDePremissa,
) -> Result<RegistroVista, String> {
    let dados = dados()?;
    bloqueante(move || validar_premissa(&dados, &projeto, &registro, validacao)).await
}

#[tauri::command]
pub async fn aceitar_proposta_app(
    projeto: String,
    demanda: String,
    referencia: String,
    ajustes: Option<DadosRegistro>,
) -> Result<RegistroVista, String> {
    let dados = dados()?;
    bloqueante(move || aceitar_proposta(&dados, &projeto, &demanda, &referencia, ajustes)).await
}

#[tauri::command]
pub async fn recusar_proposta_app(
    projeto: String,
    demanda: String,
    referencia: String,
) -> Result<(), String> {
    let dados = dados()?;
    bloqueante(move || recusar_proposta(&dados, &projeto, &demanda, &referencia)).await
}

#[cfg(test)]
#[path = "registros_testes.rs"]
mod tests;
