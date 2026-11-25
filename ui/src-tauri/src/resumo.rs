//! Comando `resumo_projeto_app` (#318, épico #317): as contagens do projeto e a lista
//! de atividades do analista numa chamada só, para a Visão geral, as Atividades e o
//! selo do menu (revisão de UX, seções 6.2 e 7.1).
//!
//! # Cache
//!
//! Abrir cada `A-NNNN.estado.json` a cada chamada custaria uma leitura e uma
//! desserialização por análise. O resumo guarda, por análise, os números que importam
//! (e a demanda ligada, que está no frontmatter do `A-NNNN.md`) e só relê o arquivo
//! cujo carimbo (data de modificação e tamanho) mudou. Regras, termos, demandas e
//! sessões têm cada um o seu carimbo de pasta; o que não mudou não é relido. Uma
//! chamada sem mudança custa só os `stat` dos arquivos.
//!
//! O diário de eventos (`<dados>/eventos/<projeto>.jsonl`) não serve de chave: a vigia
//! é a única que o lê e o esvazia, e ele só existe para avisar as telas abertas. O
//! carimbo dos arquivos vale para qualquer processo que grave (app, `cba mcp`, `cba
//! hook`), com a tela aberta ou não.
//!
//! # Sessões e histórico (#331)
//!
//! As sessões vivas vêm de todos os registros do projeto (`<dados>/sessoes`), ligadas
//! a uma análise ou não, e a atividade recente vem do histórico persistente
//! (`<dados>/historico`, [`cba_core::historico`]), que sobrevive a reiniciar o app.
//! O comando `listar_sessoes_projeto_app` devolve todas as sessões do projeto.
//!
//! # Fora do resumo
//!
//! O grupo "Ambiente a preparar" (seção 7.1) depende do `verificar_ambiente_projeto`,
//! que consulta o Claude instalado na máquina e já alimenta os selos de saúde: a tela
//! o junta a esta lista, como faz hoje.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::UNIX_EPOCH;

use cba_core::analise::{
    Analise, Cliente, EstadoAfirmacao, EstadoAnalise, EstadoPergunta, EstadoSessao, Fluxo,
};
use cba_core::dados::DiretorioDados;
use cba_core::eventos::{Evento, Mudanca};
use cba_core::historico::Historico;
use cba_core::tempo;
use cba_kb::frontmatter::{FrontmatterAnalise, StatusRegra};
use cba_kb::{Id, IdDemanda, layout};
use cba_session::passagem::ClientesDasSessoes;
use cba_session::sessoes::{RegistroSessao, Sessoes};
use cba_tools::revisao::pendencias;
use serde::Serialize;

use crate::analise::{SessaoVista, ambiente};
use crate::projeto;

/// Fluxos na ordem em que o resumo os lista (a mesma do enum).
const FLUXOS: [Fluxo; 9] = [
    Fluxo::Perguntar,
    Fluxo::Explicar,
    Fluxo::Impacto,
    Fluxo::Reuniao,
    Fluxo::Revisar,
    Fluxo::Historias,
    Fluxo::Cenarios,
    Fluxo::Manual,
    Fluxo::Prototipo,
];

// ---------------------------------------------------------------------------
// Resposta

/// Análises por estado.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PorEstado {
    pub criada: usize,
    pub em_andamento: usize,
    pub concluida: usize,
    pub aprovada: usize,
}

/// Análises de um fluxo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContagemFluxo {
    pub fluxo: Fluxo,
    pub total: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumoAnalises {
    pub total: usize,
    pub por_estado: PorEstado,
    /// Todos os fluxos, com zero quando não há análise (a tela não precisa conhecer a lista).
    pub por_fluxo: Vec<ContagemFluxo>,
}

/// Análise concluída, à espera da revisão do analista.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AguardandoRevisao {
    pub id: String,
    pub titulo: String,
    pub fluxo: Fluxo,
    pub atualizada_ms: u64,
    /// Afirmações confirmadas ou incorretas ("3 de 9 decididas").
    pub afirmacoes_decididas: usize,
    pub afirmacoes_total: usize,
    /// Itens que impedem a aprovação, pela conta única da aprovação (afirmações,
    /// termos, histórias, critérios, tabelas, cenários); o que é da análise como um
    /// todo não entra.
    pub pendencias: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PerguntasDaAnalise {
    pub analise: String,
    pub titulo: String,
    pub abertas: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PerguntasEmAberto {
    pub total: usize,
    /// Só as análises com pergunta aberta, da mais recente para a mais antiga.
    pub por_analise: Vec<PerguntasDaAnalise>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumoDemandas {
    /// Pelo estado efetivo: o conferido no rastreador (#292), quando há; senão, o local.
    pub abertas: usize,
    pub encerradas: usize,
    /// Demandas abertas sem nenhuma análise ligada.
    pub sem_analise: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumoBase {
    /// Regras vigentes (as obsoletas não contam), como no início.
    pub regras: usize,
    pub a_revalidar: usize,
    pub termos: usize,
}

/// Sessão mais recente de uma análise.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessaoDaAnalise {
    pub analise: String,
    #[serde(flatten)]
    pub sessao: SessaoVista,
}

/// Sessões, uma por análise (a mais recente), com a contagem por estado.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumoSessoes {
    pub total: usize,
    pub ativas: usize,
    pub aguardando_analista: usize,
    pub concluidas: usize,
    pub encerradas: usize,
    pub por_analise: Vec<SessaoDaAnalise>,
    /// Sessões vivas do projeto (ativas ou à espera do analista), ligadas a uma
    /// análise ou não, da mais recente para a mais antiga (#331).
    pub vivas: Vec<SessaoProjeto>,
}

/// Uma sessão de cliente de IA do projeto, como o registro dos hooks a descreve
/// (#331). Só identificadores, estados e instantes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessaoProjeto {
    pub id: String,
    pub estado: EstadoSessao,
    pub cliente: Cliente,
    /// Análise ligada, se a sessão já chamou uma ferramenta do `cba` com ela.
    pub analise: Option<String>,
    /// Título da análise ligada, quando ela existe.
    pub titulo: Option<String>,
    pub inicio_ms: u64,
    pub atualizada_ms: u64,
    pub fim_ms: Option<u64>,
}

/// Quantas atividades o resumo traz do histórico.
pub const ATIVIDADES_RECENTES: usize = 20;
/// Quantas linhas do histórico o resumo lê para montar as atividades recentes.
const LINHAS_LIDAS: usize = 2000;

/// Uma atividade recente do projeto, do histórico persistente (#331). Mudanças
/// seguidas do mesmo tipo no mesmo alvo viram uma linha só, com a quantidade.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AtividadeRecente {
    /// Instante da última mudança do grupo.
    pub quando_ms: u64,
    /// Análise (`A-0001`), reunião (`R-0001`) ou `mapa`.
    pub alvo: String,
    /// Título da análise, quando o alvo é uma análise que existe.
    pub titulo: Option<String>,
    pub mudanca: Mudanca,
    /// Estado da análise depois da mudança, quando o evento o traz.
    pub estado: Option<EstadoAnalise>,
    /// Quantas mudanças seguidas o grupo junta.
    pub quantas: usize,
}

/// Tipo de atividade, na ordem de prioridade (seção 7.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TipoAtividade {
    /// Sessão em `aguardando-analista`: responder ao agente.
    Responder,
    /// Análise concluída: revisar e aprovar.
    Revisar,
    /// Demanda aberta sem análise: analisar o impacto.
    DemandaSemAnalise,
    /// Regras com o código alterado: revalidar.
    Revalidar,
    /// Análise criada ou em andamento sem sessão viva: retomar.
    Retomar,
    /// Projeto sem nenhuma análise.
    PrimeiraAnalise,
}

/// Uma atividade que espera o analista. Os campos opcionais dependem do tipo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Atividade {
    pub tipo: TipoAtividade,
    /// Posição na lista do projeto (0 é a mais urgente).
    pub ordem: usize,
    pub analise: Option<String>,
    pub demanda: Option<String>,
    pub titulo: Option<String>,
    /// `revisar`: afirmações decididas e total.
    pub decididas: Option<usize>,
    pub total: Option<usize>,
    /// `revalidar`: regras a revalidar.
    pub regras: Option<usize>,
    /// Quando a análise mudou pela última vez.
    pub atualizada_ms: Option<u64>,
}

impl Atividade {
    fn nova(tipo: TipoAtividade) -> Self {
        Self {
            tipo,
            ordem: 0,
            analise: None,
            demanda: None,
            titulo: None,
            decididas: None,
            total: None,
            regras: None,
            atualizada_ms: None,
        }
    }
}

/// O resumo do projeto.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumoProjeto {
    pub projeto: String,
    pub nome: String,
    pub analises: ResumoAnalises,
    pub aguardando_revisao: Vec<AguardandoRevisao>,
    pub perguntas_em_aberto: PerguntasEmAberto,
    pub demandas: ResumoDemandas,
    pub base: ResumoBase,
    pub sessoes: ResumoSessoes,
    /// Na ordem do `proximoPasso`, por projeto.
    pub atividades: Vec<Atividade>,
    /// As últimas atividades do histórico persistente, da mais recente para a mais
    /// antiga (até [`ATIVIDADES_RECENTES`]). Vazia em projeto sem histórico.
    pub atividade_recente: Vec<AtividadeRecente>,
    /// O que o selo do menu mostra: quantas atividades esperam o analista.
    pub pedem_acao: usize,
    /// Quando o resumo foi montado (ms desde 1970).
    pub gerado_ms: u64,
}

// ---------------------------------------------------------------------------
// Carimbos

/// Data de modificação (ns) e tamanho de um arquivo.
type Carimbo = (u128, u64);

fn carimbo(caminho: &Path) -> Option<Carimbo> {
    let m = fs::metadata(caminho).ok()?;
    let ns = m
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos());
    Some((ns, m.len()))
}

/// Carimbos dos arquivos de `pasta` (e das subpastas, com `recursivo`) que passam
/// por `aceita`. Pasta ausente: nenhum arquivo.
fn carimbos(
    pasta: &Path,
    recursivo: bool,
    aceita: &dyn Fn(&str) -> bool,
    saida: &mut BTreeMap<PathBuf, Carimbo>,
) {
    let Ok(entradas) = fs::read_dir(pasta) else {
        return;
    };
    for e in entradas.flatten() {
        let caminho = e.path();
        let Ok(tipo) = e.file_type() else { continue };
        if tipo.is_dir() {
            if recursivo {
                carimbos(&caminho, true, aceita, saida);
            }
        } else if e.file_name().to_str().is_some_and(aceita)
            && let Some(c) = carimbo(&caminho)
        {
            saida.insert(caminho, c);
        }
    }
}

fn assinatura(mapa: &BTreeMap<PathBuf, Carimbo>) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    mapa.hash(&mut h);
    h.finish()
}

// ---------------------------------------------------------------------------
// Cache

/// O que o resumo guarda de cada análise.
#[derive(Debug, Clone)]
struct AnaliseCompacta {
    id: String,
    titulo: String,
    fluxo: Fluxo,
    estado: EstadoAnalise,
    atualizada_ms: u64,
    afirmacoes_total: usize,
    afirmacoes_decididas: usize,
    pendencias: usize,
    perguntas_abertas: usize,
    demanda: Option<IdDemanda>,
    /// Última sessão gravada na própria análise (vale quando não há registro da sessão).
    sessao_gravada: Option<SessaoVista>,
}

struct EntradaAnalise {
    estado: Carimbo,
    documento: Option<Carimbo>,
    dados: AnaliseCompacta,
}

struct DemandaCompacta {
    id: IdDemanda,
    titulo: String,
    aberta: bool,
}

#[derive(Default)]
struct CacheProjeto {
    analises: HashMap<String, EntradaAnalise>,
    base: Option<(u64, ResumoBase)>,
    demandas: Option<(u64, Vec<DemandaCompacta>)>,
    sessoes: Option<(u64, SessoesLidas)>,
    historico: Option<(u64, Vec<Evento>)>,
}

/// O que o resumo guarda dos registros de sessão.
struct SessoesLidas {
    por_analise: HashMap<String, SessaoVista>,
    /// Sessões vivas, sem o título (que vem das análises).
    vivas: Vec<SessaoProjeto>,
}

static CACHE: LazyLock<Mutex<HashMap<String, Arc<Mutex<CacheProjeto>>>>> =
    LazyLock::new(Default::default);

/// Cache do projeto, um por diretório de dados (os testes usam vários).
fn cache_do(dados: &DiretorioDados, projeto: &str) -> Arc<Mutex<CacheProjeto>> {
    let chave = format!("{}\u{0}{projeto}", dados.raiz().display());
    let mut todos = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    todos.entry(chave).or_default().clone()
}

fn compactar(analise: &Analise, demanda: Option<IdDemanda>) -> AnaliseCompacta {
    let decididas = analise
        .afirmacoes
        .iter()
        .filter(|a| {
            matches!(
                a.estado,
                EstadoAfirmacao::Confirmada | EstadoAfirmacao::Incorreta { .. }
            )
        })
        .count();
    AnaliseCompacta {
        id: analise.id.to_string(),
        titulo: analise.titulo.clone(),
        fluxo: analise.fluxo,
        estado: analise.estado,
        atualizada_ms: analise.atualizada_ms,
        afirmacoes_total: analise.afirmacoes.len(),
        afirmacoes_decididas: decididas,
        pendencias: pendencias(analise)
            .iter()
            .filter(|p| p.tipo != "analise")
            .count(),
        perguntas_abertas: analise
            .perguntas
            .iter()
            .filter(|p| p.estado == EstadoPergunta::Aberta)
            .count(),
        demanda,
        sessao_gravada: analise
            .sessoes
            .iter()
            .max_by_key(|s| s.inicio_ms)
            .map(|s| SessaoVista {
                id: s.id.clone(),
                estado: s.estado,
                cliente: s.cliente,
            }),
    }
}

// ---------------------------------------------------------------------------
// Montagem

/// Sessão viva: o agente trabalha ou espera o analista.
fn viva(s: &SessaoVista) -> bool {
    matches!(
        s.estado,
        EstadoSessao::Ativa | EstadoSessao::AguardandoAnalista
    )
}

/// Monta o resumo do projeto. Reaproveita o que não mudou desde a chamada anterior.
pub fn resumo_projeto(dados: &DiretorioDados, projeto: &str) -> Result<ResumoProjeto, String> {
    let amb = ambiente(dados, projeto)?;
    let kb = &amb.kb;
    let raiz = kb.raiz();
    let cache = cache_do(dados, projeto);
    let mut cache = cache.lock().unwrap_or_else(|e| e.into_inner());

    // Análises: relê só o estado cujo carimbo mudou.
    let pasta = raiz.join(layout::ANALISES);
    let mut arquivos = BTreeMap::new();
    carimbos(
        &pasta,
        false,
        &|n| n.ends_with(".json") || n.ends_with(".md"),
        &mut arquivos,
    );
    let mut vigentes: Vec<String> = Vec::new();
    for (caminho, estado) in &arquivos {
        let Some(id) = caminho
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| n.strip_suffix(".estado.json"))
        else {
            continue;
        };
        let Ok(id_analise) = Id::analisar(id) else {
            continue;
        };
        let documento = arquivos.get(&pasta.join(format!("{id}.md"))).copied();
        vigentes.push(id.to_owned());
        if let Some(e) = cache.analises.get(id)
            && e.estado == *estado
            && e.documento == documento
        {
            continue;
        }
        let Ok(analise) = amb.analises.carregar(&id_analise) else {
            // Estado ilegível: ignorado, como na lista de análises.
            cache.analises.remove(id);
            vigentes.pop();
            continue;
        };
        let demanda = kb
            .ler::<FrontmatterAnalise>(&analise.documento)
            .ok()
            .and_then(|d| d.frontmatter.demanda.id);
        cache.analises.insert(
            id.to_owned(),
            EntradaAnalise {
                estado: *estado,
                documento,
                dados: compactar(&analise, demanda),
            },
        );
    }
    cache.analises.retain(|id, _| vigentes.contains(id));
    let mut analises: Vec<AnaliseCompacta> =
        cache.analises.values().map(|e| e.dados.clone()).collect();
    // Da mais recente para a mais antiga; no empate, o número maior primeiro.
    analises.sort_by(|a, b| (b.atualizada_ms, &b.id).cmp(&(a.atualizada_ms, &a.id)));

    // Base: regras e glossário.
    let mut arq = BTreeMap::new();
    for p in [layout::REGRAS, layout::GLOSSARIO] {
        carimbos(&raiz.join(p), true, &|n| n.ends_with(".md"), &mut arq);
    }
    let sig = assinatura(&arq);
    let base = match &cache.base {
        Some((s, b)) if *s == sig => b.clone(),
        _ => {
            let visao = projeto::visao_projeto(dados, projeto)?;
            let b = ResumoBase {
                regras: visao
                    .regras
                    .iter()
                    .filter(|r| r.status != StatusRegra::Obsoleta)
                    .count(),
                a_revalidar: visao.a_revalidar(),
                termos: visao.termos.len(),
            };
            cache.base = Some((sig, b.clone()));
            b
        }
    };

    // Demandas.
    let mut arq = BTreeMap::new();
    carimbos(
        &raiz.join(layout::DEMANDAS),
        true,
        &|n| n == layout::ARQUIVO_DEMANDA,
        &mut arq,
    );
    let sig = assinatura(&arq);
    if cache.demandas.as_ref().is_none_or(|(s, _)| *s != sig) {
        let lidas = kb.demandas().map_err(|e| e.to_string())?;
        let lista = lidas
            .iter()
            .map(|d| DemandaCompacta {
                id: d.id,
                titulo: d.documento.frontmatter.titulo.clone(),
                aberta: d.aberta(),
            })
            .collect();
        cache.demandas = Some((sig, lista));
    }
    // Sessões: o registro mais recente de cada análise; sem registro, o gravado na análise.
    let mut arq = BTreeMap::new();
    carimbos(&dados.sessoes(), false, &|n| n.ends_with(".json"), &mut arq);
    let clientes = dados.raiz().join("terminal").join("clientes.json");
    if let Some(c) = carimbo(&clientes) {
        arq.insert(clientes, c);
    }
    let sig = assinatura(&arq);
    if cache.sessoes.as_ref().is_none_or(|(s, _)| *s != sig) {
        let registros = Sessoes::do_diretorio(dados)
            .do_projeto(projeto)
            .unwrap_or_default();
        let marcas = ClientesDasSessoes::no_diretorio(dados.raiz());
        let mut por_analise: HashMap<String, SessaoVista> = HashMap::new();
        let mut vivas = Vec::new();
        // Do mais recente para o mais antigo: a primeira de cada análise é a que vale.
        for r in registros {
            let cliente = marcas.cliente_de(&r);
            if matches!(
                r.estado,
                EstadoSessao::Ativa | EstadoSessao::AguardandoAnalista
            ) {
                vivas.push(sessao_do_registro(&r, cliente));
            }
            let Some(a) = r.analise.as_ref().map(|a| a.to_string()) else {
                continue;
            };
            por_analise.entry(a).or_insert_with(|| SessaoVista {
                cliente,
                id: r.id,
                estado: r.estado,
            });
        }
        cache.sessoes = Some((sig, SessoesLidas { por_analise, vivas }));
    }

    // Histórico: relido só quando um dos dois arquivos muda.
    let historico = Historico::em(dados.historico());
    let mut arq = BTreeMap::new();
    for caminho in [historico.anterior(projeto), historico.arquivo(projeto)] {
        if let Some(c) = carimbo(&caminho) {
            arq.insert(caminho, c);
        }
    }
    let sig = assinatura(&arq);
    if cache.historico.as_ref().is_none_or(|(s, _)| *s != sig) {
        let eventos = historico.ultimos(projeto, LINHAS_LIDAS).unwrap_or_default();
        cache.historico = Some((sig, eventos));
    }
    let titulos: HashMap<&str, &str> = analises
        .iter()
        .map(|a| (a.id.as_str(), a.titulo.as_str()))
        .collect();
    let atividade_recente = atividades_recentes(
        &cache.historico.as_ref().expect("preenchido acima").1,
        &titulos,
    );

    let lidas = &cache.sessoes.as_ref().expect("preenchido acima").1;
    let registradas = &lidas.por_analise;
    let demandas_lidas = &cache.demandas.as_ref().expect("preenchido acima").1;
    let sessao_de = |a: &AnaliseCompacta| {
        registradas
            .get(&a.id)
            .cloned()
            .or_else(|| a.sessao_gravada.clone())
    };

    // Contagens.
    let mut por_estado = PorEstado::default();
    let mut por_fluxo: Vec<ContagemFluxo> = FLUXOS
        .iter()
        .map(|&fluxo| ContagemFluxo { fluxo, total: 0 })
        .collect();
    let mut aguardando_revisao = Vec::new();
    let mut perguntas = PerguntasEmAberto::default();
    let mut sessoes = ResumoSessoes::default();
    let mut atividades = Vec::new();
    let mut paradas = Vec::new();
    for a in &analises {
        match a.estado {
            EstadoAnalise::Criada => por_estado.criada += 1,
            EstadoAnalise::EmAndamento => por_estado.em_andamento += 1,
            EstadoAnalise::Concluida => por_estado.concluida += 1,
            EstadoAnalise::Aprovada => por_estado.aprovada += 1,
        }
        if let Some(c) = por_fluxo.iter_mut().find(|c| c.fluxo == a.fluxo) {
            c.total += 1;
        }
        if a.perguntas_abertas > 0 {
            perguntas.total += a.perguntas_abertas;
            perguntas.por_analise.push(PerguntasDaAnalise {
                analise: a.id.clone(),
                titulo: a.titulo.clone(),
                abertas: a.perguntas_abertas,
            });
        }
        let sessao = sessao_de(a);
        if let Some(s) = &sessao {
            sessoes.total += 1;
            match s.estado {
                EstadoSessao::Ativa => sessoes.ativas += 1,
                EstadoSessao::AguardandoAnalista => sessoes.aguardando_analista += 1,
                EstadoSessao::Concluida => sessoes.concluidas += 1,
                EstadoSessao::Encerrada => sessoes.encerradas += 1,
            }
            sessoes.por_analise.push(SessaoDaAnalise {
                analise: a.id.clone(),
                sessao: s.clone(),
            });
        }
        let da_analise = |tipo| Atividade {
            analise: Some(a.id.clone()),
            titulo: Some(a.titulo.clone()),
            atualizada_ms: Some(a.atualizada_ms),
            ..Atividade::nova(tipo)
        };
        if a.estado != EstadoAnalise::Aprovada
            && sessao
                .as_ref()
                .is_some_and(|s| s.estado == EstadoSessao::AguardandoAnalista)
        {
            atividades.push((0, da_analise(TipoAtividade::Responder)));
        }
        if a.estado == EstadoAnalise::Concluida {
            aguardando_revisao.push(AguardandoRevisao {
                id: a.id.clone(),
                titulo: a.titulo.clone(),
                fluxo: a.fluxo,
                atualizada_ms: a.atualizada_ms,
                afirmacoes_decididas: a.afirmacoes_decididas,
                afirmacoes_total: a.afirmacoes_total,
                pendencias: a.pendencias,
            });
            atividades.push((
                1,
                Atividade {
                    decididas: Some(a.afirmacoes_decididas),
                    total: Some(a.afirmacoes_total),
                    ..da_analise(TipoAtividade::Revisar)
                },
            ));
        }
        if matches!(a.estado, EstadoAnalise::Criada | EstadoAnalise::EmAndamento)
            && !sessao.as_ref().is_some_and(viva)
        {
            paradas.push(da_analise(TipoAtividade::Retomar));
        }
    }
    let total_analises = analises.len();
    sessoes.vivas = lidas
        .vivas
        .iter()
        .map(|s| SessaoProjeto {
            titulo: s
                .analise
                .as_deref()
                .and_then(|a| titulos.get(a))
                .map(|t| (*t).to_owned()),
            ..s.clone()
        })
        .collect();

    // Demandas: abertas, encerradas e as abertas sem análise ligada.
    let com_analise: std::collections::HashSet<IdDemanda> =
        analises.iter().filter_map(|a| a.demanda).collect();
    let mut resumo_demandas = ResumoDemandas::default();
    let mut sem_analise: Vec<&DemandaCompacta> = Vec::new();
    for d in demandas_lidas {
        if !d.aberta {
            resumo_demandas.encerradas += 1;
            continue;
        }
        resumo_demandas.abertas += 1;
        if !com_analise.contains(&d.id) {
            resumo_demandas.sem_analise += 1;
            sem_analise.push(d);
        }
    }
    // As mais novas primeiro (o número da demanda cresce com o tempo).
    sem_analise.sort_by_key(|d| std::cmp::Reverse(d.id));
    for d in sem_analise {
        atividades.push((
            2,
            Atividade {
                demanda: Some(d.id.to_string()),
                titulo: Some(d.titulo.clone()),
                ..Atividade::nova(TipoAtividade::DemandaSemAnalise)
            },
        ));
    }
    if base.a_revalidar > 0 {
        atividades.push((
            3,
            Atividade {
                regras: Some(base.a_revalidar),
                ..Atividade::nova(TipoAtividade::Revalidar)
            },
        ));
    }
    atividades.extend(paradas.into_iter().map(|a| (4, a)));
    if total_analises == 0 {
        atividades.push((5, Atividade::nova(TipoAtividade::PrimeiraAnalise)));
    }
    // Estável: dentro de cada grupo fica a ordem de montagem (análise mais recente primeiro).
    atividades.sort_by_key(|(grupo, _)| *grupo);
    let atividades: Vec<Atividade> = atividades
        .into_iter()
        .enumerate()
        .map(|(ordem, (_, a))| Atividade { ordem, ..a })
        .collect();

    Ok(ResumoProjeto {
        projeto: amb.projeto.id.clone(),
        nome: amb.projeto.nome.clone(),
        analises: ResumoAnalises {
            total: total_analises,
            por_estado,
            por_fluxo,
        },
        aguardando_revisao,
        perguntas_em_aberto: perguntas,
        demandas: resumo_demandas,
        base,
        sessoes,
        pedem_acao: atividades.len(),
        atividades,
        atividade_recente,
        gerado_ms: tempo::agora_ms(),
    })
}

// ---------------------------------------------------------------------------
// Sessões e histórico (#331)

fn sessao_do_registro(r: &RegistroSessao, cliente: Cliente) -> SessaoProjeto {
    SessaoProjeto {
        id: r.id.clone(),
        estado: r.estado,
        cliente,
        analise: r.analise.as_ref().map(|a| a.to_string()),
        titulo: None,
        inicio_ms: r.inicio_ms,
        atualizada_ms: r.atualizada_ms,
        fim_ms: r.fim_ms,
    }
}

/// As atividades recentes a partir dos eventos do histórico (do mais antigo para o
/// mais recente): mudanças seguidas do mesmo tipo, no mesmo alvo e com o mesmo
/// estado viram uma linha. As mudanças de sessão ficam de fora: são as idas e
/// vindas entre "trabalhando" e "aguarda você", que o bloco das sessões já mostra.
pub fn atividades_recentes(
    eventos: &[Evento],
    titulos: &HashMap<&str, &str>,
) -> Vec<AtividadeRecente> {
    let mut grupos: Vec<AtividadeRecente> = Vec::new();
    for e in eventos.iter().filter(|e| e.mudanca != Mudanca::Sessao) {
        if let Some(g) = grupos.last_mut()
            && g.alvo == e.analise
            && g.mudanca == e.mudanca
            && g.estado == e.estado
        {
            g.quantas += 1;
            g.quando_ms = g.quando_ms.max(e.quando_ms);
            continue;
        }
        grupos.push(AtividadeRecente {
            quando_ms: e.quando_ms,
            alvo: e.analise.clone(),
            titulo: titulos.get(e.analise.as_str()).map(|t| (*t).to_owned()),
            mudanca: e.mudanca,
            estado: e.estado,
            quantas: 1,
        });
    }
    grupos.reverse();
    grupos.truncate(ATIVIDADES_RECENTES);
    grupos
}

/// Todas as sessões registradas do projeto, da mais recente para a mais antiga, com
/// o título da análise ligada.
pub fn sessoes_do_projeto(
    dados: &DiretorioDados,
    projeto: &str,
) -> Result<Vec<SessaoProjeto>, String> {
    let titulos: HashMap<String, String> = crate::analise::listar_analises(dados, projeto)?
        .into_iter()
        .map(|a| (a.id, a.titulo))
        .collect();
    let registros = Sessoes::do_diretorio(dados)
        .do_projeto(projeto)
        .map_err(|e| format!("não foi possível ler as sessões: {e}"))?;
    let marcas = ClientesDasSessoes::no_diretorio(dados.raiz());
    Ok(registros
        .iter()
        .map(|r| {
            let mut s = sessao_do_registro(r, marcas.cliente_de(r));
            s.titulo = s.analise.as_ref().and_then(|a| titulos.get(a)).cloned();
            s
        })
        .collect())
}

// ---------------------------------------------------------------------------
// Comando

#[tauri::command]
pub async fn listar_sessoes_projeto_app(projeto: String) -> Result<Vec<SessaoProjeto>, String> {
    let dados = DiretorioDados::padrao()
        .map_err(|e| format!("diretório de dados do app inacessível: {e}"))?;
    tauri::async_runtime::spawn_blocking(move || sessoes_do_projeto(&dados, &projeto))
        .await
        .map_err(|e| format!("falha interna: {e}"))?
}

#[tauri::command]
pub async fn resumo_projeto_app(projeto: String) -> Result<ResumoProjeto, String> {
    let dados = DiretorioDados::padrao()
        .map_err(|e| format!("diretório de dados do app inacessível: {e}"))?;
    tauri::async_runtime::spawn_blocking(move || resumo_projeto(&dados, &projeto))
        .await
        .map_err(|e| format!("falha interna: {e}"))?
}

#[cfg(test)]
#[path = "resumo_testes.rs"]
mod tests;
