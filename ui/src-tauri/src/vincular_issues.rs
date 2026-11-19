//! Vincular as issues do rastreador às funcionalidades do mapa (#270): a leitura
//! em lote por período, a sugestão sem IA e a confirmação do analista.
//!
//! A leitura é só de leitura no rastreador (`gh`/`jira`, token só no ambiente do
//! processo filho). Confirmar cria (ou reaproveita, pela `chave`) a demanda da base
//! e a liga à funcionalidade, com uma única gravação do mapa por lote.

use cba_core::dados::DiretorioDados;
use cba_core::funcionalidades::MapaFuncionalidades;
use cba_core::vinculos::{TextoDaIssue, sugerir};
use cba_kb::IdFuncionalidade;
use cba_repos::demanda::Ferramentas;
use cba_repos::listagem::{IssueListada, OrigemIssue, Periodo, listar_issues};
use cba_tools::vincular::{IssueConfirmada, confirmar_vinculos, demanda_da_issue};
use serde::{Deserialize, Serialize};

use crate::analise::ambiente;
use crate::repositorios::cadastrado;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SugestaoVista {
    pub funcionalidade: String,
    pub nome: String,
    pub pontuacao: u32,
    pub evidencias: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueVista {
    pub origem: OrigemIssue,
    pub chave: String,
    pub titulo: String,
    pub descricao: String,
    pub url: String,
    pub criada_em: String,
    pub estado: String,
    pub rotulos: Vec<String>,
    pub sugestoes: Vec<SugestaoVista>,
    /// Demanda da base que já representa a issue, se houver.
    pub demanda: Option<String>,
    /// Funcionalidades a que essa demanda já está ligada.
    pub ligada_a: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FuncionalidadeResumo {
    pub id: String,
    pub nome: String,
    pub area_nome: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VistaIssues {
    pub issues: Vec<IssueVista>,
    pub excedeu_teto: bool,
    pub desde: String,
    pub funcionalidades: Vec<FuncionalidadeResumo>,
}

/// Issue como a tela a devolve ao confirmar.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueEntrada {
    pub origem: OrigemIssue,
    pub chave: String,
    pub titulo: String,
    #[serde(default)]
    pub descricao: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub criada_em: String,
    #[serde(default)]
    pub estado: String,
    #[serde(default)]
    pub rotulos: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemConfirmado {
    pub issue: IssueEntrada,
    pub funcionalidades: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResultadoDoLote {
    pub criadas: Vec<String>,
    pub reaproveitadas: Vec<String>,
    pub vinculos_novos: usize,
    pub vinculos_existentes: usize,
    pub avisos: Vec<String>,
}

/// Lê as issues do período e sugere a funcionalidade de cada uma.
pub fn issues_para_vincular(
    dados: &DiretorioDados,
    projeto: &str,
    meses: Option<u32>,
    desde: Option<&str>,
    ferramentas: &Ferramentas,
) -> Result<VistaIssues, String> {
    let p = cadastrado(dados, projeto)?;
    let tracker = p
        .repositorios
        .tracker
        .as_ref()
        .ok_or("o projeto não tem repositório de issues cadastrado")?;
    let periodo = match desde.map(str::trim).filter(|d| !d.is_empty()) {
        Some(d) => Periodo::Desde(d.to_string()),
        None => Periodo::Meses(meses.unwrap_or(cba_repos::listagem::MESES_PADRAO)),
    };
    let lista = listar_issues(tracker, &periodo, None, ferramentas).map_err(|e| e.to_string())?;
    let ambiente = ambiente(dados, projeto)?;
    let mapa = MapaFuncionalidades::carregar(&ambiente.kb).map_err(|e| e.to_string())?;
    let demandas = ambiente.kb.demandas().map_err(|e| e.to_string())?;
    let issues = lista
        .issues
        .into_iter()
        .map(|i| {
            let sugestoes = sugerir(
                &mapa,
                &TextoDaIssue {
                    chave: i.chave.clone(),
                    titulo: i.titulo.clone(),
                    descricao: i.descricao.clone(),
                    arquivos: Vec::new(),
                },
            )
            .into_iter()
            .map(|s| SugestaoVista {
                funcionalidade: s.funcionalidade.to_string(),
                nome: s.nome,
                pontuacao: s.pontuacao,
                evidencias: s.evidencias,
            })
            .collect();
            let existente = demanda_da_issue(&demandas, &i);
            let ligada_a = existente
                .map(|d| {
                    mapa.abertas()
                        .filter(|f| f.demandas.contains(&d))
                        .map(|f| f.id.to_string())
                        .collect()
                })
                .unwrap_or_default();
            IssueVista {
                origem: i.origem,
                chave: i.chave,
                titulo: i.titulo,
                descricao: i.descricao,
                url: i.url,
                criada_em: i.criada_em,
                estado: i.estado,
                rotulos: i.rotulos,
                sugestoes,
                demanda: existente.map(|d| d.to_string()),
                ligada_a,
            }
        })
        .collect();
    let funcionalidades = mapa
        .ordenadas()
        .into_iter()
        .map(|f| FuncionalidadeResumo {
            id: f.id.to_string(),
            nome: f.nome.clone(),
            area_nome: f.area_nome.clone(),
        })
        .collect();
    Ok(VistaIssues {
        issues,
        excedeu_teto: lista.excedeu_teto,
        desde: lista.desde,
        funcionalidades,
    })
}

/// Confirma o lote do analista.
pub fn confirmar_lote(
    dados: &DiretorioDados,
    projeto: &str,
    itens: Vec<ItemConfirmado>,
) -> Result<ResultadoDoLote, String> {
    let ambiente = ambiente(dados, projeto)?;
    let mut lote = Vec::new();
    for item in itens {
        let funcionalidades = item
            .funcionalidades
            .iter()
            .map(|f| {
                IdFuncionalidade::analisar(f.trim())
                    .map_err(|_| format!("\"{f}\" não é uma funcionalidade (F-<SIGLA>-NNN)"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let i = item.issue;
        lote.push(IssueConfirmada {
            issue: IssueListada {
                origem: i.origem,
                chave: i.chave,
                titulo: i.titulo,
                descricao: i.descricao,
                url: i.url,
                criada_em: i.criada_em,
                estado: i.estado,
                rotulos: i.rotulos,
            },
            funcionalidades,
        });
    }
    let alterado = confirmar_vinculos(&ambiente, &lote).map_err(|e| e.to_string())?;
    for a in &alterado.avisos {
        tracing::warn!(aviso = %a, "vínculo de issues");
    }
    let r = alterado.resultado;
    Ok(ResultadoDoLote {
        criadas: r.criadas.iter().map(ToString::to_string).collect(),
        reaproveitadas: r.reaproveitadas.iter().map(ToString::to_string).collect(),
        vinculos_novos: r.vinculos_novos,
        vinculos_existentes: r.vinculos_existentes,
        avisos: alterado.avisos,
    })
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
pub async fn issues_para_vincular_app(
    projeto: String,
    meses: Option<u32>,
    desde: Option<String>,
) -> Result<VistaIssues, String> {
    let dados = dados()?;
    bloqueante(move || {
        issues_para_vincular(
            &dados,
            &projeto,
            meses,
            desde.as_deref(),
            &Ferramentas::default(),
        )
    })
    .await
}

#[tauri::command]
pub async fn confirmar_vinculos_app(
    projeto: String,
    itens: Vec<ItemConfirmado>,
) -> Result<ResultadoDoLote, String> {
    let dados = dados()?;
    bloqueante(move || confirmar_lote(&dados, &projeto, itens)).await
}

#[cfg(all(test, unix))]
#[path = "vincular_issues_testes.rs"]
mod tests;
