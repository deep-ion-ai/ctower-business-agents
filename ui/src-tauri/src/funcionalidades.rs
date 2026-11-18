//! Comandos Tauri do mapa de funcionalidades (#208): criar a análise de
//! mapeamento, ler o catálogo do projeto (a lista "Funcionalidades" da tela do
//! projeto), abrir a explicação de uma funcionalidade do catálogo e conferir a
//! completude dos pontos de entrada de uma análise de mapeamento.

use cba_core::catalogo::{Catalogo, ItemCatalogo, SituacaoNoCatalogo};
use cba_core::dados::DiretorioDados;
use cba_index::{PontoEntrada, TipoEntrada};
use cba_kb::{IdFuncionalidade, layout};
use serde::Serialize;

use crate::analise::{AnaliseCriada, AnaliseResumo, ambiente, prompt_de_inicio};

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

fn preenchido(t: Option<&str>) -> Option<String> {
    t.map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
}

/// Cria a análise de mapeamento (fluxo `mapear`) do projeto inteiro, de uma pasta
/// ou de uma área, com a orientação opcional do analista.
pub fn criar_analise_de_mapeamento(
    dados: &DiretorioDados,
    projeto: &str,
    pasta: Option<&str>,
    area: Option<&str>,
    entrada: Option<&str>,
) -> Result<AnaliseCriada, String> {
    let ambiente = ambiente(dados, projeto)?;
    let analise = cba_tools::mapear::criar_analise_de_mapeamento(
        &ambiente,
        preenchido(pasta),
        preenchido(area),
        preenchido(entrada),
        None,
    )
    .map_err(|e| e.to_string())?;
    Ok(AnaliseCriada {
        prompt_inicial: prompt_de_inicio(&analise.id),
        analise: AnaliseResumo::from(&analise),
    })
}

/// Cria a explicação (fluxo `explicar`) de uma funcionalidade do catálogo, com a
/// entrada preenchida e ligada ao item.
pub fn explicar_funcionalidade(
    dados: &DiretorioDados,
    projeto: &str,
    funcionalidade: &str,
) -> Result<AnaliseCriada, String> {
    let ambiente = ambiente(dados, projeto)?;
    let id = IdFuncionalidade::analisar(funcionalidade.trim())
        .map_err(|_| format!("\"{funcionalidade}\" não é uma funcionalidade (F-<SIGLA>-NNN)"))?;
    let analise = cba_tools::mapear::criar_explicacao(&ambiente, &id).map_err(|e| e.to_string())?;
    Ok(AnaliseCriada {
        prompt_inicial: prompt_de_inicio(&analise.id),
        analise: AnaliseResumo::from(&analise),
    })
}

/// Funcionalidade na lista da tela do projeto.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FuncionalidadeVista {
    pub id: String,
    pub nome: String,
    pub descricao: String,
    pub atores: Vec<String>,
    /// Nomes dos pontos de entrada.
    pub pontos: Vec<String>,
    pub situacao: SituacaoNoCatalogo,
    /// Documento negocial aprovado, relativo à raiz da base (para abrir na Base).
    pub documento: Option<String>,
    /// Análise de explicação ligada (a mais recente), se houver.
    pub explicacao: Option<String>,
}

impl From<&ItemCatalogo> for FuncionalidadeVista {
    fn from(i: &ItemCatalogo) -> Self {
        Self {
            id: i.id.to_string(),
            nome: i.nome.clone(),
            descricao: i.descricao.clone(),
            atores: i.atores.clone(),
            pontos: i.pontos.iter().map(|p| p.nome.clone()).collect(),
            situacao: i.situacao,
            documento: i.documento.as_ref().map(ToString::to_string),
            explicacao: i.explicacao.as_ref().map(ToString::to_string),
        }
    }
}

/// Área do catálogo, com as funcionalidades dela.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AreaVista {
    pub area: String,
    pub nome: String,
    pub funcionalidades: Vec<FuncionalidadeVista>,
}

/// O catálogo de funcionalidades do projeto, por área.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogoVista {
    pub areas: Vec<AreaVista>,
    pub total: usize,
    pub documentadas: usize,
    /// Documento do catálogo (`funcionalidades/catalogo.md`), relativo à base,
    /// quando existe.
    pub documento: Option<String>,
    /// Última análise de mapeamento aprovada.
    pub origem: Option<String>,
}

pub fn catalogo_funcionalidades(
    dados: &DiretorioDados,
    projeto: &str,
) -> Result<CatalogoVista, String> {
    let ambiente = ambiente(dados, projeto)?;
    let catalogo = Catalogo::carregar(&ambiente.kb).map_err(|e| e.to_string())?;
    let mut areas: Vec<AreaVista> = Vec::new();
    for item in catalogo.ordenadas() {
        let mut vista = FuncionalidadeVista::from(item);
        vista.documento = item
            .documento
            .as_ref()
            .map(|d| ambiente.kb.na_base(d).to_string());
        match areas.last_mut() {
            Some(a) if a.area == item.area => a.funcionalidades.push(vista),
            _ => areas.push(AreaVista {
                area: item.area.clone(),
                nome: item.area_nome.clone(),
                funcionalidades: vec![vista],
            }),
        }
    }
    let documento = layout::catalogo();
    Ok(CatalogoVista {
        total: catalogo.funcionalidades.len(),
        documentadas: catalogo
            .funcionalidades
            .iter()
            .filter(|i| i.documentada())
            .count(),
        documento: ambiente
            .kb
            .existe(&documento)
            .then(|| ambiente.kb.na_base(&documento).to_string()),
        origem: catalogo.origem.as_ref().map(ToString::to_string),
        areas,
    })
}

/// Ponto de entrada sem tratamento, para a tela da análise.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PontoVista {
    pub chave: String,
    pub tipo: TipoEntrada,
    pub nome: String,
    pub arquivo: String,
    pub linha: u32,
}

impl From<PontoEntrada> for PontoVista {
    fn from(p: PontoEntrada) -> Self {
        Self {
            chave: p.chave,
            tipo: p.tipo,
            nome: p.nome,
            arquivo: p.arquivo,
            linha: p.linhas.inicio,
        }
    }
}

/// Completude de uma análise de mapeamento: os pontos do escopo e os que ainda
/// não têm funcionalidade nem justificativa.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletudeVista {
    pub total: usize,
    pub sem_tratamento: Vec<PontoVista>,
}

pub fn completude_do_mapa(
    dados: &DiretorioDados,
    projeto: &str,
    analise: &str,
) -> Result<CompletudeVista, String> {
    let ambiente = ambiente(dados, projeto)?;
    let id = cba_kb::Id::analisar(analise.trim())
        .map_err(|_| format!("\"{analise}\" não é uma análise (A-NNNN)"))?;
    let analise = ambiente.analises.carregar(&id).map_err(|e| e.to_string())?;
    let (escopo, faltam) =
        cba_tools::mapear::completude(&ambiente, &analise).map_err(|e| e.to_string())?;
    Ok(CompletudeVista {
        total: escopo.len(),
        sem_tratamento: faltam.into_iter().map(PontoVista::from).collect(),
    })
}

// ---------------------------------------------------------------------------
// Comandos

#[tauri::command]
pub async fn criar_analise_de_mapeamento_app(
    projeto: String,
    pasta: Option<String>,
    area: Option<String>,
    entrada: Option<String>,
) -> Result<AnaliseCriada, String> {
    let dados = dados()?;
    bloqueante(move || {
        criar_analise_de_mapeamento(
            &dados,
            &projeto,
            pasta.as_deref(),
            area.as_deref(),
            entrada.as_deref(),
        )
    })
    .await
}

#[tauri::command]
pub async fn catalogo_funcionalidades_projeto(projeto: String) -> Result<CatalogoVista, String> {
    let dados = dados()?;
    bloqueante(move || catalogo_funcionalidades(&dados, &projeto)).await
}

#[tauri::command]
pub async fn explicar_funcionalidade_app(
    projeto: String,
    funcionalidade: String,
) -> Result<AnaliseCriada, String> {
    let dados = dados()?;
    bloqueante(move || explicar_funcionalidade(&dados, &projeto, &funcionalidade)).await
}

#[tauri::command]
pub async fn completude_mapa_analise(
    projeto: String,
    analise: String,
) -> Result<CompletudeVista, String> {
    let dados = dados()?;
    bloqueante(move || completude_do_mapa(&dados, &projeto, &analise)).await
}

#[cfg(test)]
#[path = "funcionalidades_testes.rs"]
mod tests;
