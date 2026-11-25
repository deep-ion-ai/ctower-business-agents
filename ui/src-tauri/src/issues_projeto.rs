//! Issues do rastreador do projeto (#322): a lista da área Demandas, com cache,
//! busca e filtros.
//!
//! A lista vem do cache em `<dados>/issues` ([`cba_repos::cache_issues`]). Com
//! cache, abrir a lista não chama `gh` nem `jira`: o cache vencido é relido numa
//! thread, que depois confere o estado das demandas ligadas (o mesmo cache de
//! conferência da #292) e avisa a tela pelo evento [`EVENTO_ATUALIZADAS`]. Sem
//! cache, a primeira leitura é síncrona. "Atualizar" relê na hora. Sem rede,
//! a lista mostra a última leitura com o aviso.
//!
//! Para cada issue: a demanda da base que a representa, as funcionalidades a que
//! ela está ligada e a situação na base (sem análise, em andamento, em revisão,
//! aprovada), pela análise mais recente da demanda ou da chave da issue.
//!
//! Nada é escrito no rastreador.

use std::collections::BTreeSet;
use std::path::PathBuf;

use cba_core::analise::{Analise, EstadoAnalise, Fluxo};
use cba_core::dados::DiretorioDados;
use cba_core::funcionalidades::MapaFuncionalidades;
use cba_core::tempo;
use cba_kb::IdDemanda;
use cba_kb::frontmatter::FrontmatterAnalise;
use cba_repos::TrackerConfig;
use cba_repos::cache_issues::{self, CacheIssues, FiltroIssues, VALIDADE_PADRAO};
use cba_repos::demanda::Ferramentas;
use cba_repos::listagem::{IssueListada, OrigemIssue};
use cba_tools::vincular::{demanda_da_issue, demanda_da_issue_ou_nova};
use serde::{Deserialize, Serialize};

use crate::analise::{AnaliseCriada, ambiente};
use crate::repositorios::cadastrado;

/// Evento mandado à tela quando a releitura em segundo plano termina; a carga é
/// o slug do projeto.
pub const EVENTO_ATUALIZADAS: &str = "issues-do-projeto-atualizadas";

/// Situação da issue na base.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SituacaoNaBase {
    SemAnalise,
    /// Análise criada ou com o agente trabalhando.
    EmAndamento,
    /// Análise concluída, aguardando a revisão do analista.
    EmRevisao,
    Aprovada,
}

impl SituacaoNaBase {
    fn da_analise(estado: EstadoAnalise) -> Self {
        match estado {
            EstadoAnalise::Criada | EstadoAnalise::EmAndamento => SituacaoNaBase::EmAndamento,
            EstadoAnalise::Concluida => SituacaoNaBase::EmRevisao,
            EstadoAnalise::Aprovada => SituacaoNaBase::Aprovada,
        }
    }
}

/// Filtro da tela: o do rastreador mais a situação na base.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FiltroLista {
    #[serde(flatten)]
    pub issues: FiltroIssues,
    pub situacao: Option<SituacaoNaBase>,
}

/// Issue na lista.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueDoProjeto {
    pub origem: OrigemIssue,
    pub chave: String,
    pub titulo: String,
    pub descricao: String,
    pub url: String,
    pub criada_em: String,
    /// `aberta` ou `fechada`, como o rastreador disse na última leitura.
    pub estado: String,
    pub rotulos: Vec<String>,
    pub situacao: SituacaoNaBase,
    /// Análise que define a situação (a mais recente).
    pub analise: Option<String>,
    /// Todas as análises ligadas à issue (pela demanda ou pela chave), da mais
    /// recente para a mais antiga (#350).
    pub analises: Vec<String>,
    /// Demanda da base que representa a issue.
    pub demanda: Option<String>,
    /// Funcionalidades a que essa demanda está ligada.
    pub funcionalidades: Vec<String>,
}

/// O que a lista mostra.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListaIssues {
    /// As que passam no filtro, na ordem do rastreador (as mais novas primeiro).
    pub issues: Vec<IssueDoProjeto>,
    /// Issues da última leitura, antes do filtro.
    pub total: usize,
    /// Rótulos de todas as issues lidas, para o filtro.
    pub rotulos: Vec<String>,
    /// Instante da última leitura que deu certo (ISO 8601, UTC).
    pub lida_em: String,
    /// Há releitura em segundo plano; o evento avisa quando terminar.
    pub atualizando: bool,
    /// A última tentativa de leitura falhou (sem rede, sem token): a lista é a
    /// anterior.
    pub aviso: Option<String>,
    pub excedeu_teto: bool,
    /// Data de corte da leitura (`AAAA-MM-DD`).
    pub desde: String,
}

/// Como abrir a lista.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Leitura {
    /// Do cache; vencido, relê em segundo plano.
    Cache,
    /// Relê o rastreador agora ("Atualizar").
    Forcada,
}

fn tracker_do_projeto(dados: &DiretorioDados, projeto: &str) -> Result<TrackerConfig, String> {
    cadastrado(dados, projeto)?
        .repositorios
        .tracker
        .ok_or_else(|| "o projeto não tem repositório de issues cadastrado".into())
}

fn arquivo_do_cache(dados: &DiretorioDados, projeto: &str) -> PathBuf {
    cache_issues::arquivo(&dados.issues(), projeto)
}

/// Monta a lista. `ao_atualizar` roda na thread da releitura em segundo plano,
/// quando ela termina (o app avisa a tela; os testes esperam por ele).
pub fn listar(
    dados: &DiretorioDados,
    projeto: &str,
    filtro: &FiltroLista,
    leitura: Leitura,
    ferramentas: &Ferramentas,
    ao_atualizar: impl FnOnce() + Send + 'static,
) -> Result<ListaIssues, String> {
    let tracker = tracker_do_projeto(dados, projeto)?;
    let arquivo = arquivo_do_cache(dados, projeto);
    let em_cache = cache_issues::ler(&arquivo, &tracker);
    let (cache, aviso_agora) = match (leitura, em_cache) {
        (Leitura::Cache, Some(c)) => {
            if c.vencido(VALIDADE_PADRAO, cache_issues::agora_ms()) {
                reler_em_segundo_plano(dados, projeto, &tracker, ferramentas, ao_atualizar);
            }
            (c, None)
        }
        // Sem cache (ou "Atualizar"): lê agora. Outra releitura em curso não
        // impede esta: a gravação é atômica e a última vence.
        (_, anterior) => match cache_issues::atualizar(&arquivo, &tracker, ferramentas) {
            Ok(c) => {
                conferir_ligadas(dados, projeto, &c, ferramentas);
                (c, None)
            }
            Err(erro) => match anterior {
                Some(c) => (c, Some(erro.to_string())),
                None => return Err(format!("as issues não puderam ser lidas: {erro}")),
            },
        },
    };
    montar(dados, projeto, &cache, filtro, aviso_agora, &arquivo)
}

fn reler_em_segundo_plano(
    dados: &DiretorioDados,
    projeto: &str,
    tracker: &TrackerConfig,
    ferramentas: &Ferramentas,
    ao_atualizar: impl FnOnce() + Send + 'static,
) {
    let arquivo = arquivo_do_cache(dados, projeto);
    let Some(reserva) = cache_issues::reservar(&arquivo) else {
        return;
    };
    let (dados, projeto, tracker, ferramentas) = (
        dados.clone(),
        projeto.to_string(),
        tracker.clone(),
        ferramentas.clone(),
    );
    let iniciada = std::thread::Builder::new()
        .name("cba-issues-do-projeto".into())
        .spawn(move || {
            match cache_issues::atualizar(&arquivo, &tracker, &ferramentas) {
                Ok(c) => conferir_ligadas(&dados, &projeto, &c, &ferramentas),
                Err(erro) => tracing::warn!(%erro, "issues do projeto não relidas"),
            }
            drop(reserva);
            ao_atualizar();
        });
    if let Err(erro) = iniciada {
        tracing::warn!(%erro, "releitura das issues não iniciada");
    }
}

/// Confere com o rastreador o estado das demandas ligadas às issues lidas,
/// com o cache curto da #292 (a demanda conferida há pouco não é relida).
fn conferir_ligadas(dados: &DiretorioDados, projeto: &str, c: &CacheIssues, f: &Ferramentas) {
    let Ok(amb) = ambiente(dados, projeto) else {
        return;
    };
    let Ok(demandas) = amb.kb.demandas() else {
        return;
    };
    let ids: Vec<IdDemanda> = c
        .listagem
        .issues
        .iter()
        .filter_map(|i| demanda_da_issue(&demandas, i))
        .collect();
    if ids.is_empty() {
        return;
    }
    if let Err(erro) = crate::demandas::conferir_status(dados, projeto, Some(&ids), false, f) {
        tracing::warn!(%erro, "estado das demandas ligadas às issues não conferido");
    }
}

/// Análise ligada, com a demanda (`id`) e a chave da issue de onde veio.
struct AnaliseLigada {
    id: String,
    estado: EstadoAnalise,
    criada_ms: u64,
    demanda: Option<IdDemanda>,
    origem: String,
    chave: Option<String>,
}

fn analises_ligadas(amb: &cba_core::ambiente::Ambiente) -> Vec<AnaliseLigada> {
    let analises: Vec<Analise> = match amb.analises.listar() {
        Ok(a) => a,
        Err(erro) => {
            tracing::warn!(%erro, "análises ilegíveis na lista de issues");
            return Vec::new();
        }
    };
    analises
        .into_iter()
        .filter_map(|a| {
            let doc = amb.kb.ler::<FrontmatterAnalise>(&a.documento).ok()?;
            let d = doc.frontmatter.demanda;
            Some(AnaliseLigada {
                id: a.id.to_string(),
                estado: a.estado,
                criada_ms: a.criada_ms,
                demanda: d.id,
                origem: d.origem,
                chave: d.chave,
            })
        })
        .collect()
}

/// A origem gravada na análise aponta o rastreador da issue: o nome dele (`jira`)
/// ou o link da issue, que é o que a análise criada direto da issue grava
/// (`criar_analise_de_issue`). `texto` (demanda colada) nunca casa (#350).
fn da_mesma_origem(origem: &str, i: &IssueListada) -> bool {
    let o = origem.trim();
    o.eq_ignore_ascii_case(i.origem.nome())
        || o.eq_ignore_ascii_case(i.url.trim())
        || o.starts_with("http://")
        || o.starts_with("https://")
}

fn montar(
    dados: &DiretorioDados,
    projeto: &str,
    cache: &CacheIssues,
    filtro: &FiltroLista,
    aviso_agora: Option<String>,
    arquivo: &std::path::Path,
) -> Result<ListaIssues, String> {
    let amb = ambiente(dados, projeto)?;
    let demandas = amb.kb.demandas().map_err(|e| e.to_string())?;
    // Mapa ilegível não impede a lista: as issues ficam sem funcionalidade.
    let mapa = MapaFuncionalidades::carregar(&amb.kb)
        .inspect_err(|erro| tracing::warn!(%erro, "mapa ilegível na lista de issues"))
        .ok();
    let analises = analises_ligadas(&amb);
    let vista = |i: &IssueListada| -> IssueDoProjeto {
        let demanda = demanda_da_issue(&demandas, i);
        let mut ligadas: Vec<&AnaliseLigada> = analises
            .iter()
            .filter(|a| {
                (demanda.is_some() && a.demanda == demanda)
                    || (da_mesma_origem(&a.origem, i)
                        && a.chave
                            .as_deref()
                            .is_some_and(|c| c.trim().eq_ignore_ascii_case(&i.chave)))
            })
            .collect();
        ligadas.sort_by_key(|a| std::cmp::Reverse(a.criada_ms));
        let mais_recente = ligadas.first().copied();
        let funcionalidades = match (demanda, &mapa) {
            (Some(d), Some(m)) => m
                .abertas()
                .filter(|f| f.demandas.contains(&d))
                .map(|f| f.id.to_string())
                .collect(),
            _ => Vec::new(),
        };
        IssueDoProjeto {
            origem: i.origem,
            chave: i.chave.clone(),
            titulo: i.titulo.clone(),
            descricao: i.descricao.clone(),
            url: i.url.clone(),
            criada_em: i.criada_em.clone(),
            estado: i.estado.clone(),
            rotulos: i.rotulos.clone(),
            situacao: mais_recente.map_or(SituacaoNaBase::SemAnalise, |a| {
                SituacaoNaBase::da_analise(a.estado)
            }),
            analise: mais_recente.map(|a| a.id.clone()),
            analises: ligadas.iter().map(|a| a.id.clone()).collect(),
            demanda: demanda.map(|d| d.to_string()),
            funcionalidades,
        }
    };
    let issues = cache
        .listagem
        .issues
        .iter()
        .filter(|i| filtro.issues.aceita(i))
        .map(vista)
        .filter(|v| filtro.situacao.is_none_or(|s| v.situacao == s))
        .collect();
    let rotulos: BTreeSet<String> = cache
        .listagem
        .issues
        .iter()
        .flat_map(|i| i.rotulos.iter().cloned())
        .collect();
    Ok(ListaIssues {
        issues,
        total: cache.listagem.issues.len(),
        rotulos: rotulos.into_iter().collect(),
        lida_em: tempo::instante_iso(cache.lida_em_ms),
        atualizando: cache_issues::em_leitura(arquivo),
        aviso: aviso_agora.or_else(|| cache.falha.as_ref().map(|f| f.mensagem.clone())),
        excedeu_teto: cache.listagem.excedeu_teto,
        desde: cache.listagem.desde.clone(),
    })
}

/// O que "Analisar impacto" fez.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImpactoIniciado {
    /// Demanda da base que representa a issue (`D-0001`).
    pub demanda: String,
    /// A demanda nasceu agora; senão, foi reaproveitada.
    pub demanda_criada: bool,
    pub criada: AnaliseCriada,
}

/// "Analisar impacto" (#323): cria a demanda da base com a chave e o endereço da
/// issue, ou reaproveita a que existe, e cria a análise de impacto ligada a ela,
/// com o título e a descrição da issue como entrada. A issue é a da última leitura
/// da lista (a que o analista está vendo); nada é escrito no rastreador.
///
/// A issue encerrada no rastreador não é analisada: a demanda dela já nasceria
/// encerrada e a base só liga análise nova a demanda aberta.
pub fn analisar_impacto(
    dados: &DiretorioDados,
    projeto: &str,
    origem: OrigemIssue,
    chave: &str,
) -> Result<ImpactoIniciado, String> {
    let tracker = tracker_do_projeto(dados, projeto)?;
    let cache = cache_issues::ler(&arquivo_do_cache(dados, projeto), &tracker)
        .ok_or("a lista de issues ainda não foi lida; use Atualizar e tente de novo")?;
    let chave = chave.trim();
    let issue = cache
        .listagem
        .issues
        .iter()
        .find(|i| i.origem == origem && i.chave.eq_ignore_ascii_case(chave))
        .ok_or_else(|| {
            format!("a issue {chave} não está na lista; use Atualizar e tente de novo")
        })?;
    if issue.estado != "aberta" {
        return Err(format!(
            "a issue {} está encerrada no rastreador; a análise de impacto parte de uma issue aberta",
            issue.chave
        ));
    }
    let amb = ambiente(dados, projeto)?;
    let mut existentes = amb.kb.demandas().map_err(|e| e.to_string())?;
    let (id, demanda_criada) =
        demanda_da_issue_ou_nova(&amb, &mut existentes, issue).map_err(|e| e.to_string())?;
    let entrada = if issue.descricao.trim().is_empty() {
        issue.titulo.clone()
    } else {
        format!("{}\n\n{}", issue.titulo, issue.descricao.trim())
    };
    let criada = crate::demandas::criar_analise_ligada(
        dados,
        projeto,
        &id.to_string(),
        Fluxo::Impacto,
        &entrada,
        Some(&issue.titulo),
    )?;
    Ok(ImpactoIniciado {
        demanda: id.to_string(),
        demanda_criada,
        criada,
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

/// Lista das issues do projeto. Com cache, não espera o rastreador; `forcar`
/// ("Atualizar") relê na hora.
#[tauri::command]
pub async fn issues_do_projeto_app(
    app: tauri::AppHandle,
    projeto: String,
    filtro: Option<FiltroLista>,
    forcar: Option<bool>,
) -> Result<ListaIssues, String> {
    use tauri::Emitter;
    let dados = dados()?;
    let leitura = if forcar.unwrap_or(false) {
        Leitura::Forcada
    } else {
        Leitura::Cache
    };
    bloqueante(move || {
        let slug = projeto.clone();
        listar(
            &dados,
            &projeto,
            &filtro.unwrap_or_default(),
            leitura,
            &Ferramentas::default(),
            move || {
                // A janela pode ter sido fechada; não há o que fazer com o erro.
                let _ = app.emit(EVENTO_ATUALIZADAS, slug);
            },
        )
    })
    .await
}
/// "Analisar impacto" na linha da issue (#323): demanda na base, análise de
/// impacto ligada e o pedido que abre a conversa.
#[tauri::command]
pub async fn analisar_impacto_da_issue_app(
    projeto: String,
    origem: OrigemIssue,
    chave: String,
) -> Result<ImpactoIniciado, String> {
    let dados = dados()?;
    bloqueante(move || analisar_impacto(&dados, &projeto, origem, &chave)).await
}

#[cfg(all(test, unix))]
#[path = "issues_projeto_testes.rs"]
mod tests;
