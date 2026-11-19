//! Comandos Tauri do mapa de funcionalidades (#208), a tela "Mapa de
//! funcionalidades" do menu lateral: ler o mapa do projeto (funcionalidades por
//! área, pontos justificados e a completude contra o mapa estrutural), aplicar a
//! revisão do analista, montar o pedido que abre a sessão de mapeamento no
//! terminal, abrir a explicação de uma funcionalidade e exportar o documento.
//! O mapa é um cadastro do projeto, não uma análise.

use std::collections::{BTreeMap, BTreeSet};

use cba_core::analise::Analise;

use cba_core::dados::DiretorioDados;
use cba_core::funcionalidades::{
    Funcionalidade, MapaFuncionalidades, MotivoSemFuncionalidade, PontoAtribuido,
    SituacaoFuncionalidade,
};
use cba_index::{PontoEntrada, TipoEntrada};
use cba_kb::{IdFuncionalidade, layout};
use cba_tools::mapear::{self, RevisaoDoMapa};
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

/// Ponto de entrada de uma funcionalidade.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PontoDaFuncionalidade {
    pub nome: String,
    pub chave: Option<String>,
    pub tipo: Option<TipoEntrada>,
    pub arquivo: String,
    pub linha_inicial: u32,
    pub linha_final: u32,
    pub trecho: String,
    /// O ponto não está na lista que o app acha (só a evidência o sustenta).
    pub fora_da_lista: bool,
}

impl From<&PontoAtribuido> for PontoDaFuncionalidade {
    fn from(p: &PontoAtribuido) -> Self {
        Self {
            nome: p.nome.clone(),
            chave: p.chave.clone(),
            tipo: p.tipo,
            arquivo: p.evidencia.arquivo.to_string(),
            linha_inicial: p.evidencia.linhas.inicio,
            linha_final: p.evidencia.linhas.fim,
            trecho: p.evidencia.trecho.clone(),
            fora_da_lista: p.chave.is_none(),
        }
    }
}

/// Funcionalidade do mapa, como a tela mostra.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FuncionalidadeVista {
    pub id: String,
    pub nome: String,
    pub area: String,
    pub area_nome: String,
    pub descricao: String,
    pub atores: Vec<String>,
    pub pontos: Vec<PontoDaFuncionalidade>,
    pub dados_lidos: Vec<String>,
    pub dados_gravados: Vec<String>,
    pub relacionadas: Vec<String>,
    /// `pendente` (a revisar), `aceita`, `descartada` ou `juntada`.
    pub situacao: SituacaoFuncionalidade,
    /// Na juntada: a funcionalidade que a recebeu.
    pub juntada_em: Option<String>,
    pub editada: bool,
    /// Documento negocial aprovado, relativo à raiz da base (para abrir na Base).
    pub documento: Option<String>,
    /// Análise de explicação ligada (a mais recente), se houver.
    pub explicacao: Option<AnaliseResumo>,
    /// Nenhum ponto de entrada dela existe mais no mapa estrutural.
    pub sem_ponto_de_entrada: bool,
}

/// Área do mapa, com as funcionalidades dela.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AreaVista {
    pub area: String,
    pub nome: String,
    pub funcionalidades: Vec<FuncionalidadeVista>,
}

/// Ponto do mapa estrutural (sem tratamento, na completude).
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

/// Ponto justificado sem funcionalidade.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JustificadoVista {
    pub chave: String,
    pub tipo: TipoEntrada,
    pub nome: String,
    pub arquivo: String,
    pub motivo: MotivoSemFuncionalidade,
    pub justificativa: String,
}

/// Pontos do mapa estrutural e os que ainda não têm funcionalidade nem
/// justificativa.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletudeVista {
    pub total: usize,
    pub sem_tratamento: Vec<PontoVista>,
}

/// O mapa de funcionalidades do projeto.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MapaVista {
    pub areas: Vec<AreaVista>,
    /// Descartadas e juntadas (o identificador não volta; a descartada pode
    /// voltar a revisar).
    pub fechadas: Vec<FuncionalidadeVista>,
    pub total: usize,
    pub pendentes: usize,
    pub documentadas: usize,
    pub justificados: Vec<JustificadoVista>,
    /// Sem ela, o mapa estrutural do projeto não pôde ser lido (`erro_completude`).
    pub completude: Option<CompletudeVista>,
    pub erro_completude: Option<String>,
    /// `funcionalidades/mapa.md`, relativo à base, quando existe.
    pub documento: Option<String>,
    pub versao: u64,
}

fn vista_da(
    f: &Funcionalidade,
    kb: &cba_kb::Projeto,
    chaves: Option<&BTreeSet<String>>,
    explicacoes: &BTreeMap<IdFuncionalidade, Analise>,
) -> FuncionalidadeVista {
    FuncionalidadeVista {
        id: f.id.to_string(),
        nome: f.nome.clone(),
        area: f.area.clone(),
        area_nome: f.area_nome.clone(),
        descricao: f.descricao.clone(),
        atores: f.atores.clone(),
        pontos: f.pontos.iter().map(PontoDaFuncionalidade::from).collect(),
        dados_lidos: f.dados_lidos.clone(),
        dados_gravados: f.dados_gravados.clone(),
        relacionadas: f.relacionadas.iter().map(ToString::to_string).collect(),
        situacao: f.situacao,
        juntada_em: f.juntada_em.as_ref().map(ToString::to_string),
        editada: f.editada,
        documento: f.documento.as_ref().map(|d| kb.na_base(d).to_string()),
        explicacao: explicacoes.get(&f.id).map(AnaliseResumo::from),
        sem_ponto_de_entrada: chaves.is_some_and(|c| mapear::sem_ponto_de_entrada(f, c)),
    }
}

fn vista(
    mapa: &MapaFuncionalidades,
    ambiente: &cba_core::ambiente::Ambiente,
    estrutural: Result<Vec<PontoEntrada>, String>,
) -> MapaVista {
    let kb = &ambiente.kb;
    let explicacoes = mapear::explicacoes(&ambiente.analises);
    let (completude, erro_completude, chaves) = match estrutural {
        Ok(pontos) => {
            let chaves: BTreeSet<String> = pontos.iter().map(|p| p.chave.clone()).collect();
            let faltam = mapear::nao_mapeados(mapa, &pontos)
                .into_iter()
                .cloned()
                .map(PontoVista::from)
                .collect();
            (
                Some(CompletudeVista {
                    total: pontos.len(),
                    sem_tratamento: faltam,
                }),
                None,
                Some(chaves),
            )
        }
        Err(e) => (None, Some(e), None),
    };
    let mut areas: Vec<AreaVista> = Vec::new();
    for f in mapa.ordenadas() {
        let v = vista_da(f, kb, chaves.as_ref(), &explicacoes);
        match areas.last_mut() {
            Some(a) if a.area == f.area => a.funcionalidades.push(v),
            _ => areas.push(AreaVista {
                area: f.area.clone(),
                nome: f.area_nome.clone(),
                funcionalidades: vec![v],
            }),
        }
    }
    let documento = layout::mapa_de_funcionalidades();
    let fechadas = mapa
        .funcionalidades
        .iter()
        .filter(|f| !f.aberta())
        .map(|f| vista_da(f, kb, None, &explicacoes))
        .collect();
    MapaVista {
        fechadas,
        total: mapa.abertas().count(),
        pendentes: mapa.pendentes(),
        documentadas: mapa.abertas().filter(|f| f.documento.is_some()).count(),
        justificados: mapa
            .pontos_justificados
            .iter()
            .map(|j| JustificadoVista {
                chave: j.chave.clone(),
                tipo: j.tipo,
                nome: j.nome.clone(),
                arquivo: j.arquivo.clone(),
                motivo: j.motivo,
                justificativa: j.justificativa.clone(),
            })
            .collect(),
        completude,
        erro_completude,
        documento: kb
            .existe(&documento)
            .then(|| kb.na_base(&documento).to_string()),
        versao: mapa.versao,
        areas,
    }
}

pub fn mapa_funcionalidades(dados: &DiretorioDados, projeto: &str) -> Result<MapaVista, String> {
    let ambiente = ambiente(dados, projeto)?;
    let mapa = MapaFuncionalidades::carregar(&ambiente.kb).map_err(|e| e.to_string())?;
    Ok(vista(
        &mapa,
        &ambiente,
        mapear::pontos_do_projeto(&ambiente),
    ))
}

pub fn revisar_mapa(
    dados: &DiretorioDados,
    projeto: &str,
    revisao: RevisaoDoMapa,
) -> Result<MapaVista, String> {
    let ambiente = ambiente(dados, projeto)?;
    let alterado = mapear::revisar_mapa(&ambiente.base, &ambiente.kb, dados, revisao)
        .map_err(|e| e.to_string())?;
    for a in &alterado.avisos {
        tracing::warn!(aviso = %a, "revisão do mapa");
    }
    Ok(vista(
        &alterado.mapa,
        &ambiente,
        mapear::pontos_do_projeto(&ambiente),
    ))
}

/// Cria a explicação (fluxo `explicar`) de uma funcionalidade do mapa, com a
/// entrada preenchida e ligada a ela.
pub fn explicar_funcionalidade(
    dados: &DiretorioDados,
    projeto: &str,
    funcionalidade: &str,
) -> Result<AnaliseCriada, String> {
    let ambiente = ambiente(dados, projeto)?;
    let id = IdFuncionalidade::analisar(funcionalidade.trim())
        .map_err(|_| format!("\"{funcionalidade}\" não é uma funcionalidade (F-<SIGLA>-NNN)"))?;
    let analise = mapear::criar_explicacao(&ambiente, &id).map_err(|e| e.to_string())?;
    Ok(AnaliseCriada {
        prompt_inicial: prompt_de_inicio(&analise.id),
        analise: AnaliseResumo::from(&analise),
    })
}

/// Exporta o documento do mapa em Markdown ou Word, com tudo (evidências e
/// trechos citados).
pub fn exportar_mapa(
    dados: &DiretorioDados,
    projeto: &str,
    destino: &str,
    formato: cba_tools::revisao::Formato,
) -> Result<String, String> {
    let ambiente = ambiente(dados, projeto)?;
    cba_tools::revisao::exportar_mapa(
        &ambiente,
        std::path::Path::new(destino),
        formato,
        &Default::default(),
    )
    .map(|p| p.display().to_string())
    .map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Comandos

#[tauri::command]
pub async fn mapa_funcionalidades_projeto(projeto: String) -> Result<MapaVista, String> {
    let dados = dados()?;
    bloqueante(move || mapa_funcionalidades(&dados, &projeto)).await
}

#[tauri::command]
pub async fn revisar_mapa_projeto(
    projeto: String,
    revisao: RevisaoDoMapa,
) -> Result<MapaVista, String> {
    let dados = dados()?;
    bloqueante(move || revisar_mapa(&dados, &projeto, revisao)).await
}

/// Pedido que abre a sessão de mapeamento no terminal do Claude.
#[tauri::command]
pub fn pedido_de_mapeamento(
    pasta: Option<String>,
    area: Option<String>,
    orientacao: Option<String>,
) -> String {
    mapear::pedido_de_mapeamento(pasta.as_deref(), area.as_deref(), orientacao.as_deref())
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
pub async fn exportar_mapa_projeto(
    projeto: String,
    destino: String,
    formato: cba_tools::revisao::Formato,
) -> Result<String, String> {
    let dados = dados()?;
    bloqueante(move || exportar_mapa(&dados, &projeto, &destino, formato)).await
}

#[cfg(test)]
#[path = "funcionalidades_testes.rs"]
mod tests;
