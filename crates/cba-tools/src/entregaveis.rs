//! Gerar entregável a partir da demanda (#463).
//!
//! O entregável nasce de uma **análise validada** da demanda (aprovada, de
//! impacto, explicação ou pergunta) e herda as regras, as evidências e as
//! decisões dela. O modelo (rascunhos versionados, revisão final, envios) está em
//! [`cba_core::entregaveis`]; o catálogo dos tipos, em [`cba_docs::entregaveis`].
//! Aqui fica o que depende do projeto:
//!
//! - **Base** ([`analises_validadas`], [`base_da_analise`]): as regras confirmadas
//!   da análise ([`regras_da_analise`]: as regras da base com origem nela ou com
//!   o mesmo conjunto de evidências de uma afirmação confirmada, o critério de
//!   "mesma regra" da aprovação), as decisões respondidas e as perguntas em
//!   aberto.
//! - **Geração** ([`gerar`]): só tipos habilitados no projeto e redigidos pelo
//!   agente. "Gerar" (ou "Gerar de novo") abre um rascunho novo e devolve o
//!   prompt da sessão, como no mapeamento: `iniciar_entregavel` dá o roteiro, o
//!   esquema e a base; `registrar_entregavel` registra cada item.
//! - **Rastreabilidade** (`registrar_entregavel`): os campos passam pelo esquema
//!   do tipo; cada regra citada precisa ser uma regra confirmada da análise de
//!   origem (ou, na versão inicial de um documento vivo, da funcionalidade): a
//!   regra inexistente, ou de fora, é recusada. A evidência do item é copiada da
//!   regra, não vem do agente. A resposta avisa as regras confirmadas ainda sem
//!   item ("RN-FAT-005 sem história").
//! - **Tipo vivo**: o item registrado vira a proposta de versão do documento da
//!   funcionalidade ([`crate::documentos_vivos::propor`]). Sem documento
//!   publicado, a demanda gera antes a v1.0 ([`Finalidade::VersaoInicial`]).
//! - **Revisão final obrigatória** ([`editar`], [`marcar_revisado`]): editar ou
//!   gerar de novo volta a rascunho; enviar e publicar exigem a revisão.
//! - **Destino** ([`exportar`], [`publicar`], [`preparar_comentario`]): o
//!   configurado para o tipo. Exportar grava o Markdown; publicar grava na base
//!   (o entregável por demanda em `demandas/<pasta>/entregaveis/`, o vivo como a
//!   versão nova do documento), de onde a publicação leva ao repositório de
//!   documentação; o comentário no rastreador sai pronto para colar na issue.
//! - [`vista_da_demanda`]: o que a seção "Entregáveis" da demanda mostra.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;

use cba_core::ambiente::Ambiente;
use cba_core::analise::{Analise, EstadoAfirmacao, EstadoAnalise, EstadoPergunta, Fluxo};
use cba_core::documentos_vivos::{DocumentosVivos, Dono, Marca, NovaProposta, Versao};
use cba_core::entregaveis::{
    EntregaveisDaDemanda, Entregavel, ErroArquivoEntregaveis, ErroEntregavel, Finalidade,
    NovoEntregavel, NovoItem, RegraCitada, Situacao,
};
use cba_core::funcionalidades::{Funcionalidade, MapaFuncionalidades};
use cba_docs::entregaveis::{self as catalogo, CicloDeVida, Geracao, TipoEntregavel};
use cba_index::Destino;
use cba_kb::frontmatter::{
    Evidencia, FrontmatterAnalise, FrontmatterEntregavel, FrontmatterRegra, StatusRegra,
};
use cba_kb::{
    CaminhoRelativo, DemandaLida, Documento, Id, IdDemanda, IdFuncionalidade, TipoDocumento,
    layout, slug,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::documentos_vivos::{self, DocumentoDoTipo, PedidoDeProposta};
use crate::evidencias::ArquivoDoProjeto;
use crate::registro::{ErroFerramenta, Ferramenta, Saida, ler_argumentos};

/// Fluxos cuja análise aprovada serve de base para um entregável: os que
/// investigam o código e confirmam regras.
pub const FLUXOS_DE_ORIGEM: [Fluxo; 3] = [Fluxo::Impacto, Fluxo::Explicar, Fluxo::Perguntar];

/// O que o histórico do documento vivo registra na versão inicial, se o agente
/// não disser outra coisa.
pub const MUDANCAS_DA_VERSAO_INICIAL: &str = "Versão inicial, gerada por engenharia reversa";

/// Linhas de código no máximo num trecho de evidência da vista.
const MAX_LINHAS_DO_TRECHO: usize = 40;

/// Caracteres do enunciado de cada regra no roteiro.
const MAX_ENUNCIADO: usize = 600;

/// Como redigir o tipo: o trecho próprio do roteiro, quando o tipo tem um. Vale
/// na demanda (`iniciar_entregavel`) e na versão inicial pelo mapa
/// (`iniciar_versao_inicial`).
pub fn guia_do_tipo(tipo: &str) -> Option<&'static str> {
    match tipo {
        catalogo::CASO_DE_USO => Some(include_str!("../textos/entregaveis/caso-de-uso.md")),
        catalogo::EPICO => Some(include_str!("../textos/entregaveis/epico.md")),
        _ => None,
    }
}

/// O tipo tem um item só, que registrar de novo substitui: o documento vivo e
/// o épico (as histórias vêm dentro dele).
pub fn item_unico(t: &TipoEntregavel) -> bool {
    t.ciclo == CicloDeVida::Vivo || t.id == catalogo::EPICO
}

fn interno(e: impl std::fmt::Display) -> ErroFerramenta {
    ErroFerramenta::Interno(e.to_string())
}

impl From<ErroArquivoEntregaveis> for ErroFerramenta {
    fn from(e: ErroArquivoEntregaveis) -> Self {
        ErroFerramenta::Interno(e.to_string())
    }
}

impl From<ErroEntregavel> for ErroFerramenta {
    fn from(e: ErroEntregavel) -> Self {
        ErroFerramenta::Recusa(e.to_string())
    }
}

// ---------------------------------------------------------------------------
// Base: a análise validada

/// A demanda, ou a recusa dizendo que ela não existe.
pub fn demanda(ambiente: &Ambiente, id: IdDemanda) -> Result<DemandaLida, ErroFerramenta> {
    ambiente.kb.demanda(id).map_err(|e| match e {
        cba_kb::Erro::DemandaInexistente { .. } => ErroFerramenta::Recusa(e.to_string()),
        outro => interno(outro),
    })
}

fn frontmatter(ambiente: &Ambiente, a: &Analise) -> Option<FrontmatterAnalise> {
    ambiente
        .kb
        .ler::<FrontmatterAnalise>(&a.documento)
        .ok()
        .map(|d| d.frontmatter)
}

/// Se a análise serve de base: aprovada e de um dos [`FLUXOS_DE_ORIGEM`].
pub fn validada(a: &Analise) -> bool {
    a.estado == EstadoAnalise::Aprovada && FLUXOS_DE_ORIGEM.contains(&a.fluxo)
}

/// As análises validadas da demanda, na ordem de criação.
pub fn analises_validadas(
    ambiente: &Ambiente,
    demanda: IdDemanda,
) -> Result<Vec<Analise>, ErroFerramenta> {
    let mut saida: Vec<Analise> = ambiente
        .analises
        .listar()
        .map_err(interno)?
        .into_iter()
        .filter(validada)
        .filter(|a| frontmatter(ambiente, a).is_some_and(|f| f.demanda.id == Some(demanda)))
        .collect();
    saida.sort_by_key(|a| a.criada_ms);
    Ok(saida)
}

/// Uma regra confirmada da base, para a base do entregável.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RegraDaBase {
    pub id: Id,
    pub titulo: String,
    /// O enunciado (o corpo do documento da regra), encurtado.
    pub enunciado: String,
    pub evidencias: Vec<Evidencia>,
}

/// As regras da base, lidas uma vez.
fn regras_da_base(kb: &cba_kb::Projeto) -> Vec<(FrontmatterRegra, String)> {
    let mut lista = Vec::new();
    for rel in kb.documentos().unwrap_or_default() {
        if rel.partes().next() != Some(layout::REGRAS) || !rel.as_str().ends_with(".md") {
            continue;
        }
        if let Ok(doc) = kb.ler::<FrontmatterRegra>(&rel) {
            lista.push((doc.frontmatter, doc.corpo));
        }
    }
    lista.sort_by_key(|(r, _)| r.id.to_string());
    lista
}

fn enunciado(corpo: &str, id: &Id) -> String {
    let linhas: Vec<&str> = corpo
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with("<!--"))
        .filter(|l| !(l.starts_with('#') && l.contains(&id.to_string())))
        .collect();
    let texto = linhas.join(" ");
    if texto.chars().count() > MAX_ENUNCIADO {
        let mut curto: String = texto.chars().take(MAX_ENUNCIADO).collect();
        curto.push('…');
        curto
    } else {
        texto
    }
}

fn da_base(r: &FrontmatterRegra, corpo: &str) -> RegraDaBase {
    RegraDaBase {
        id: r.id.clone(),
        titulo: r.titulo.clone(),
        enunciado: enunciado(corpo, &r.id),
        evidencias: r.evidencias.clone(),
    }
}

/// Conjunto de evidências comparável: arquivo e hash do trecho (as linhas
/// podem ter mudado de lugar), como na aprovação.
fn chave_das_evidencias<'a>(
    evidencias: impl Iterator<Item = (&'a CaminhoRelativo, &'a str)>,
) -> BTreeSet<(String, String)> {
    evidencias
        .map(|(a, h)| (a.as_str().to_owned(), h.to_owned()))
        .collect()
}

/// As regras confirmadas da análise: as regras não obsoletas da base com origem
/// no documento dela e as que têm o mesmo conjunto de evidências de uma
/// afirmação confirmada dela (a aprovação atualiza a regra existente em vez de
/// duplicar, e a origem antiga fica).
pub fn regras_da_analise(ambiente: &Ambiente, analise: &Analise) -> Vec<RegraDaBase> {
    regras_da_analise_em(&regras_da_base(&ambiente.kb), analise)
}

fn regras_da_analise_em(
    base: &[(FrontmatterRegra, String)],
    analise: &Analise,
) -> Vec<RegraDaBase> {
    let confirmadas: Vec<BTreeSet<(String, String)>> = analise
        .afirmacoes
        .iter()
        .filter(|a| a.estado == EstadoAfirmacao::Confirmada && !a.evidencias.is_empty())
        .map(|a| chave_das_evidencias(a.evidencias.iter().map(|e| (&e.arquivo, e.hash.as_str()))))
        .collect();
    base.iter()
        .filter(|(r, _)| r.status != StatusRegra::Obsoleta)
        .filter(|(r, _)| {
            r.origem.as_ref() == Some(&analise.documento)
                || (!r.evidencias.is_empty()
                    && confirmadas.contains(&chave_das_evidencias(
                        r.evidencias.iter().map(|e| (&e.arquivo, e.hash.as_str())),
                    )))
        })
        .map(|(r, corpo)| da_base(r, corpo))
        .collect()
}

/// As regras confirmadas da base ligadas à funcionalidade do mapa.
fn regras_da_funcionalidade_em(
    base: &[(FrontmatterRegra, String)],
    id: &IdFuncionalidade,
) -> Vec<RegraDaBase> {
    base.iter()
        .filter(|(r, _)| r.status != StatusRegra::Obsoleta && r.funcionalidades.contains(id))
        .map(|(r, corpo)| da_base(r, corpo))
        .collect()
}

/// Decisão para o negócio já respondida.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DecisaoRespondida {
    pub numero: u32,
    pub texto: String,
    pub resposta: String,
}

/// Pergunta (dúvida ou decisão) ainda em aberto.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PerguntaAberta {
    pub numero: u32,
    pub texto: String,
    pub decisao: bool,
}

/// A base de um entregável: a análise validada, com as regras confirmadas, as
/// decisões respondidas e as perguntas em aberto.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BaseDoEntregavel {
    pub analise: Id,
    pub titulo: String,
    pub fluxo: Fluxo,
    pub regras: Vec<RegraDaBase>,
    pub decisoes: Vec<DecisaoRespondida>,
    pub perguntas: Vec<PerguntaAberta>,
    /// Funcionalidades do mapa a que a análise se refere.
    pub funcionalidades: Vec<IdFuncionalidade>,
}

fn base_em(
    ambiente: &Ambiente,
    regras: &[(FrontmatterRegra, String)],
    a: &Analise,
) -> BaseDoEntregavel {
    let mut funcionalidades: Vec<IdFuncionalidade> = frontmatter(ambiente, a)
        .map(|f| f.funcionalidades)
        .unwrap_or_default();
    if let Some(f) = &a.funcionalidade
        && !funcionalidades.contains(f)
    {
        funcionalidades.insert(0, f.clone());
    }
    BaseDoEntregavel {
        analise: a.id.clone(),
        titulo: a.titulo.clone(),
        fluxo: a.fluxo,
        regras: regras_da_analise_em(regras, a),
        decisoes: a
            .perguntas
            .iter()
            .filter(|p| p.e_decisao())
            .filter_map(|p| match &p.estado {
                EstadoPergunta::Respondida { resposta } => Some(DecisaoRespondida {
                    numero: p.numero,
                    texto: p.texto.clone(),
                    resposta: resposta.clone(),
                }),
                _ => None,
            })
            .collect(),
        perguntas: a
            .perguntas
            .iter()
            .filter(|p| p.estado == EstadoPergunta::Aberta)
            .map(|p| PerguntaAberta {
                numero: p.numero,
                texto: p.texto.clone(),
                decisao: p.e_decisao(),
            })
            .collect(),
        funcionalidades,
    }
}

/// A base do entregável a partir da análise.
pub fn base_da_analise(ambiente: &Ambiente, a: &Analise) -> BaseDoEntregavel {
    base_em(ambiente, &regras_da_base(&ambiente.kb), a)
}

/// A análise validada da demanda, ou a recusa.
fn analise_validada(
    ambiente: &Ambiente,
    demanda: IdDemanda,
    id: &Id,
) -> Result<Analise, ErroFerramenta> {
    analises_validadas(ambiente, demanda)?
        .into_iter()
        .find(|a| &a.id == id)
        .ok_or_else(|| {
            ErroFerramenta::Recusa(format!(
                "a análise {id} não é uma análise validada da demanda {demanda}: o entregável \
                 parte de uma análise aprovada (de impacto, explicação ou pergunta) ligada à \
                 demanda"
            ))
        })
}

// ---------------------------------------------------------------------------
// Geração

fn tipo_do_catalogo(id: &str) -> Result<&'static TipoEntregavel, ErroFerramenta> {
    catalogo::tipo(id).ok_or_else(|| {
        ErroFerramenta::Argumentos(format!("o tipo `{id}` não está no catálogo de entregáveis"))
    })
}

/// Pedido de geração do analista ("Gerar", "Gerar de novo", "Gerar a v1.0").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PedidoDeGeracao {
    pub demanda: IdDemanda,
    pub tipo: String,
    pub analise: Id,
    /// Funcionalidade do mapa (só no tipo vivo).
    pub funcionalidade: Option<IdFuncionalidade>,
    pub finalidade: Finalidade,
}

/// Rascunho aberto, com o prompt da sessão do agente.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GeracaoAberta {
    pub numero: u32,
    pub versao: u32,
    pub prompt: String,
}

/// O prompt que abre a sessão do agente para o entregável.
pub fn prompt_de_geracao(demanda: IdDemanda, numero: u32) -> String {
    format!(
        "Gere o entregável E-{numero} da demanda {demanda} pelo ctower: chame a ferramenta \
         iniciar_entregavel do cba com demanda \"{demanda}\" e entregavel {numero} e siga o \
         roteiro devolvido."
    )
}

fn funcionalidade_aberta(
    mapa: &MapaFuncionalidades,
    id: &IdFuncionalidade,
) -> Result<Funcionalidade, ErroFerramenta> {
    mapa.funcionalidade(id)
        .filter(|f| f.aberta())
        .cloned()
        .ok_or_else(|| {
            ErroFerramenta::Recusa(format!(
                "a funcionalidade {id} não está no mapa (ou foi descartada ou juntada)"
            ))
        })
}

/// O documento vivo do tipo na funcionalidade: o caminho, a versão publicada e
/// o estado.
fn documento_do_tipo(
    ambiente: &Ambiente,
    f: &Funcionalidade,
    tipo: &str,
) -> Result<DocumentoDoTipo, ErroFerramenta> {
    let docs = DocumentosVivos::carregar(&ambiente.kb).map_err(interno)?;
    Ok(
        documentos_vivos::documentos_da_funcionalidade(&ambiente.escopo, &docs, f, &[tipo])
            .pop()
            .expect("um tipo pedido, um documento"),
    )
}

/// Abre o rascunho do entregável: confere o tipo (habilitado no projeto e
/// redigido pelo agente), a análise (validada, da demanda) e, no tipo vivo, a
/// funcionalidade e o documento dela; devolve o número, a versão do rascunho e
/// o prompt da sessão.
pub fn gerar(
    ambiente: &Ambiente,
    pedido: PedidoDeGeracao,
) -> Result<GeracaoAberta, ErroFerramenta> {
    let lida = demanda(ambiente, pedido.demanda)?;
    if !lida.aberta() {
        return Err(ErroFerramenta::Recusa(format!(
            "a demanda {} está encerrada; reabra-a para gerar um entregável",
            pedido.demanda
        )));
    }
    let tipo = tipo_do_catalogo(&pedido.tipo)?;
    let config = catalogo::efetiva(ambiente.projeto.entregaveis.as_ref());
    let habilitado = config.habilitado(tipo.id).ok_or_else(|| {
        ErroFerramenta::Recusa(format!(
            "{} não está habilitado no projeto; habilite-o nas configurações do projeto \
             (Entregáveis)",
            tipo.nome
        ))
    })?;
    if tipo.geracao != Geracao::Agente {
        return Err(ErroFerramenta::Recusa(format!(
            "{} é montado pelo app, sem IA, e a montagem ainda não está disponível na demanda",
            tipo.nome
        )));
    }
    let analise = analise_validada(ambiente, pedido.demanda, &pedido.analise)?;
    let (funcionalidade, finalidade) = match tipo.ciclo {
        CicloDeVida::PorDemanda => {
            if pedido.finalidade == Finalidade::VersaoInicial {
                return Err(ErroFerramenta::Recusa(format!(
                    "{} é um entregável por demanda: não tem versão inicial",
                    tipo.nome
                )));
            }
            (None, Finalidade::Demanda)
        }
        CicloDeVida::Vivo => {
            let id = pedido.funcionalidade.ok_or_else(|| {
                ErroFerramenta::Argumentos(format!(
                    "{} é um documento vivo: escolha a funcionalidade do mapa",
                    tipo.nome
                ))
            })?;
            let mapa = MapaFuncionalidades::carregar(&ambiente.kb).map_err(interno)?;
            let f = funcionalidade_aberta(&mapa, &id)?;
            let doc = documento_do_tipo(ambiente, &f, tipo.id)?;
            match (pedido.finalidade, doc.versao) {
                (Finalidade::Demanda, None) => {
                    return Err(ErroFerramenta::Recusa(format!(
                        "a funcionalidade {} ({}) ainda não tem {}: gere a v1.0 antes, e a \
                         demanda propõe a versão seguinte",
                        f.id, f.nome, tipo.nome
                    )));
                }
                (Finalidade::VersaoInicial, Some(v)) => {
                    return Err(ErroFerramenta::Recusa(format!(
                        "a funcionalidade {} já tem {} na versão {v}: um documento vivo nunca é \
                         recriado; gere a proposta da versão seguinte",
                        f.id, tipo.nome
                    )));
                }
                _ => {}
            }
            (Some(id), pedido.finalidade)
        }
    };
    let novo = NovoEntregavel {
        tipo: tipo.id.to_owned(),
        analise: analise.id.clone(),
        finalidade,
        funcionalidade,
        destino: habilitado.destino,
    };
    let (estado, numero) = EntregaveisDaDemanda::alterar(&ambiente.kb, &lida.pasta, |d, agora| {
        Ok::<_, ErroFerramenta>(d.gerar(novo, agora))
    })?;
    let e = estado.entregavel(numero).expect("acabou de gerar");
    // "Gerar de novo" num vivo já revisado: a proposta volta a rascunho também.
    if let Some(caminho) = &e.documento {
        tirar_revisao_da_proposta(ambiente, caminho)?;
    }
    Ok(GeracaoAberta {
        numero,
        versao: e.atual().versao,
        prompt: prompt_de_geracao(pedido.demanda, numero),
    })
}

/// A proposta do documento vivo, se revisada, volta a rascunho.
fn tirar_revisao_da_proposta(
    ambiente: &Ambiente,
    caminho: &CaminhoRelativo,
) -> Result<(), ErroFerramenta> {
    let docs = DocumentosVivos::carregar(&ambiente.kb).map_err(interno)?;
    if let Some(p) = docs
        .documento(caminho)
        .and_then(|d| d.proposta.as_ref())
        .filter(|p| p.revisada.is_some())
    {
        documentos_vivos::editar_proposta(ambiente, caminho, &p.texto.clone(), None)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Ferramentas do agente

fn ler_demanda(texto: &str) -> Result<IdDemanda, ErroFerramenta> {
    IdDemanda::analisar(texto.trim())
        .map_err(|e| ErroFerramenta::Argumentos(format!("`demanda`: {e}; use D-NNNN, como D-0012")))
}

/// O entregável da demanda e a pasta dela.
fn carregar(
    ambiente: &Ambiente,
    demanda_id: IdDemanda,
    numero: u32,
) -> Result<(DemandaLida, Entregavel), ErroFerramenta> {
    let lida = demanda(ambiente, demanda_id)?;
    let estado = EntregaveisDaDemanda::carregar(&ambiente.kb, &lida.pasta)?;
    let e = estado
        .entregavel(numero)
        .cloned()
        .ok_or(ErroEntregavel::Desconhecido(numero))?;
    Ok((lida, e))
}

pub(crate) fn descricao_do_campo(c: &catalogo::Campo) -> String {
    let tipo = match c.tipo {
        catalogo::TipoCampo::Texto => "uma linha",
        catalogo::TipoCampo::TextoLongo => "texto de um ou mais parágrafos",
        catalogo::TipoCampo::Lista => "lista de textos",
        catalogo::TipoCampo::Regras => "lista de regras RN-<SIGLA>-NNN",
        catalogo::TipoCampo::Etapas => {
            "lista de etapas, cada uma citando no texto a regra que a sustenta (RN-<SIGLA>-NNN)"
        }
        catalogo::TipoCampo::Historias => {
            "lista de histórias na ordem sugerida, cada uma um objeto com os campos abaixo"
        }
    };
    let obrigatorio = if c.obrigatorio {
        "obrigatório"
    } else {
        "opcional"
    };
    format!("- `{}` ({}): {tipo}, {obrigatorio}", c.nome, c.rotulo)
}

fn esquema_json(t: &TipoEntregavel) -> Value {
    Value::Array(
        t.campos
            .iter()
            .map(|c| {
                let mut v = serde_json::to_value(c).unwrap_or_default();
                if c.tipo == catalogo::TipoCampo::Historias {
                    v["campos"] = serde_json::to_value(catalogo::CAMPOS_DA_HISTORIA_DO_EPICO)
                        .unwrap_or_default();
                }
                v
            })
            .collect(),
    )
}

/// O esquema em Markdown, com os campos de cada história do épico.
pub(crate) fn esquema_em_markdown(t: &TipoEntregavel) -> String {
    let mut r = String::new();
    for c in t.campos {
        r.push_str(&descricao_do_campo(c));
        r.push('\n');
        if c.tipo == catalogo::TipoCampo::Historias {
            for h in catalogo::CAMPOS_DA_HISTORIA_DO_EPICO {
                let _ = writeln!(r, "  {}", descricao_do_campo(h));
            }
        }
    }
    r
}

fn linhas_das_regras(regras: &[RegraDaBase]) -> String {
    if regras.is_empty() {
        return "Nenhuma.".into();
    }
    regras
        .iter()
        .map(|r| {
            if r.enunciado.is_empty() {
                format!("- {}: {}", r.id, r.titulo)
            } else {
                format!("- {}: {}. {}", r.id, r.titulo, r.enunciado)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// O roteiro de `iniciar_entregavel`.
fn roteiro(
    demanda: IdDemanda,
    e: &Entregavel,
    t: &TipoEntregavel,
    base: &BaseDoEntregavel,
    funcionalidade: Option<&Funcionalidade>,
    regras_da_funcionalidade: &[RegraDaBase],
    publicado: Option<(Versao, &str)>,
) -> String {
    let mut r = format!(
        "# Entregável E-{} da demanda {demanda}: {}\n\n",
        e.numero, t.nome
    );
    let vivo = t.ciclo == CicloDeVida::Vivo;
    let _ = writeln!(
        r,
        "Você redige o **rascunho** (v{}) a partir da análise validada {} ({}). O analista faz \
         a revisão final no app: nada é enviado nem publicado sem ela.\n",
        e.atual().versao,
        base.analise,
        base.titulo
    );
    if let Some(f) = funcionalidade {
        let _ = writeln!(
            r,
            "Funcionalidade do mapa: {} · {} (área {}). {}",
            f.id, f.nome, f.area_nome, f.descricao
        );
        if !f.atores.is_empty() {
            let _ = writeln!(r, "Atores: {}.", f.atores.join(", "));
        }
        r.push('\n');
    }
    r.push_str("## Como fazer\n\n");
    match (vivo, e.finalidade) {
        (true, Finalidade::VersaoInicial) => r.push_str(
            "1. Este é o **documento vivo** da funcionalidade, na **versão inicial (1.0)**: \
             descreva o sistema como ele é hoje, pelo mapa e pelas regras, sem a mudança da \
             demanda (ela vira a versão seguinte, depois).\n\
             2. Registre o documento inteiro num só `registrar_entregavel`; registrar de novo \
             substitui o anterior.\n",
        ),
        (true, Finalidade::Demanda) => r.push_str(
            "1. Este é o **documento vivo** da funcionalidade: não o recrie. Parta da versão \
             publicada (abaixo) e proponha a versão seguinte com a mudança da demanda; o app \
             marca cada trecho como novo, alterado ou removido.\n\
             2. Registre o documento inteiro num só `registrar_entregavel`, com `mudancas` \
             dizendo em uma frase o que muda; registrar de novo substitui o anterior.\n",
        ),
        (false, _) if item_unico(t) => {
            let _ = writeln!(
                r,
                "1. O {} é um item só: registre-o inteiro num só `registrar_entregavel`; \
                 registrar de novo substitui o anterior.\n\
                 2. Juntas, as partes do item cobrem as regras confirmadas da análise.",
                t.nome.to_lowercase()
            );
        }
        (false, _) => {
            let _ = writeln!(
                r,
                "1. Registre cada item ({}) com `registrar_entregavel`, quantos forem \
                 necessários para cobrir as regras confirmadas.",
                t.nome.to_lowercase()
            );
        }
    }
    let _ = write!(
        r,
        "3. Os campos seguem o esquema abaixo; o app recusa campo desconhecido, obrigatório \
         vazio ou valor do tipo errado.\n\
         4. Cada item cita em `regras` só regras da lista abaixo (a evidência vem da regra, \
         não de você). Regra que não existe, ou que não é desta análise, é recusada.\n\
         5. A resposta de cada registro avisa as regras confirmadas ainda sem item: cubra-as, \
         ou diga ao analista por que ficaram de fora.\n\
         6. As decisões respondidas valem como decididas; as perguntas em aberto não viram \
         fato: deixe-as como pendência no texto.\n\
         7. Ao terminar, diga ao analista que o rascunho está na seção \"Entregáveis\" da \
         demanda {demanda}, para a revisão final.\n\n\
         Use `registrar_entregavel` com demanda \"{demanda}\" e entregavel {}.\n\n",
        e.numero
    );
    if let Some(guia) = guia_do_tipo(t.id) {
        r.push_str(guia.trim_end());
        r.push_str("\n\n");
    }
    r.push_str("## Esquema (campos)\n\n");
    r.push_str(&esquema_em_markdown(t));
    let _ = write!(
        r,
        "\n## Regras confirmadas da análise {}\n\n{}\n",
        base.analise,
        linhas_das_regras(&base.regras)
    );
    if e.finalidade == Finalidade::VersaoInicial {
        let _ = write!(
            r,
            "\n## Regras confirmadas da funcionalidade\n\n{}\n",
            linhas_das_regras(regras_da_funcionalidade)
        );
    }
    if !base.decisoes.is_empty() {
        r.push_str("\n## Decisões respondidas\n\n");
        for d in &base.decisoes {
            let _ = writeln!(r, "- {}. {} → {}", d.numero, d.texto, d.resposta);
        }
    }
    if !base.perguntas.is_empty() {
        r.push_str("\n## Perguntas em aberto\n\n");
        for p in &base.perguntas {
            let _ = writeln!(r, "- {}. {}", p.numero, p.texto);
        }
    }
    if let Some((versao, texto)) = publicado {
        let _ = write!(
            r,
            "\n## Versão publicada ({versao})\n\n````markdown\n{}\n````\n",
            texto.trim()
        );
    }
    r
}

#[derive(Deserialize)]
struct ArgsDoEntregavel {
    demanda: String,
    entregavel: u32,
}

fn propriedades_do_entregavel() -> serde_json::Map<String, Value> {
    let mut m = serde_json::Map::new();
    m.insert(
        "demanda".into(),
        json!({
            "type": "string", "minLength": 1, "maxLength": 20,
            "description": "Demanda, como D-0012."
        }),
    );
    m.insert(
        "entregavel".into(),
        json!({
            "type": "integer", "minimum": 1,
            "description": "Número do entregável na demanda (o 3 de E-3), como veio no prompt."
        }),
    );
    m
}

/// Texto da versão publicada do documento vivo do entregável, se houver.
fn publicado(ambiente: &Ambiente, e: &Entregavel, f: &Funcionalidade) -> Option<(Versao, String)> {
    let docs = DocumentosVivos::carregar(&ambiente.kb).ok()?;
    let dono = Dono::Funcionalidade(f.id.clone());
    if let Some(r) = docs.do_dono(&e.tipo, &dono).and_then(|d| d.publicada()) {
        return Some((r.versao, r.texto.clone()));
    }
    // O documento vinculado de antes do histórico (o negocial) conta como 1.0.
    let caminho = f.documento(&e.tipo)?;
    let texto = std::fs::read_to_string(ambiente.kb.caminho(caminho)).ok()?;
    let corpo = Documento::<cba_kb::frontmatter::FrontmatterLivre>::analisar(&texto)
        .map(|d| d.corpo)
        .unwrap_or(texto);
    Some((Versao::INICIAL, corpo.trim().to_owned()))
}

/// `iniciar_entregavel`: o roteiro, o esquema e a base do entregável.
pub fn iniciar_entregavel() -> Ferramenta {
    let mut props = propriedades_do_entregavel();
    let schema = json!({
        "type": "object",
        "properties": Value::Object(std::mem::take(&mut props)),
        "required": ["demanda", "entregavel"],
        "additionalProperties": false
    });
    Ferramenta::nova(
        "iniciar_entregavel",
        "Iniciar um entregável",
        "Só de leitura. Abre a redação de um entregável da demanda (história, épico, caso de \
         uso...), gerado pelo analista no app: devolve o roteiro, o esquema dos campos do tipo \
         e a base, a análise validada com as regras confirmadas, as decisões respondidas e as \
         perguntas em aberto (no documento vivo, também a versão publicada). Chame quando o \
         prompt pedir o entregável E-n da demanda D-NNNN; depois registre com \
         `registrar_entregavel`.",
        schema,
        |ambiente, args| {
            let args: ArgsDoEntregavel = ler_argumentos(args)?;
            let demanda_id = ler_demanda(&args.demanda)?;
            let (_, e) = carregar(ambiente, demanda_id, args.entregavel)?;
            let t = tipo_do_catalogo(&e.tipo)?;
            let analise = ambiente.analises.carregar(&e.analise)?;
            let regras = regras_da_base(&ambiente.kb);
            let base = base_em(ambiente, &regras, &analise);
            let mapa = MapaFuncionalidades::carregar(&ambiente.kb).map_err(interno)?;
            let f = e
                .funcionalidade
                .as_ref()
                .map(|id| funcionalidade_aberta(&mapa, id))
                .transpose()?;
            let da_funcionalidade = f
                .as_ref()
                .map(|f| regras_da_funcionalidade_em(&regras, &f.id))
                .unwrap_or_default();
            let pub_ = match (&f, e.finalidade) {
                (Some(f), Finalidade::Demanda) => publicado(ambiente, &e, f),
                _ => None,
            };
            let texto = roteiro(
                demanda_id,
                &e,
                t,
                &base,
                f.as_ref(),
                &da_funcionalidade,
                pub_.as_ref().map(|(v, t)| (*v, t.as_str())),
            );
            let dados = json!({
                "demanda": demanda_id,
                "entregavel": e.numero,
                "tipo": { "id": t.id, "nome": t.nome, "ciclo": t.ciclo, "campos": esquema_json(t) },
                "finalidade": e.finalidade,
                "rascunho": e.atual().versao,
                "base": base,
                "funcionalidade": f.as_ref().map(|f| json!({
                    "id": f.id, "nome": f.nome, "area": f.area_nome,
                    "descricao": f.descricao, "atores": f.atores,
                })),
                "regras_da_funcionalidade": da_funcionalidade,
                "publicado": pub_.as_ref().map(|(v, t)| json!({ "versao": v, "texto": t })),
            });
            Ok(Saida {
                texto,
                dados: Some(dados),
            })
        },
    )
    .somente_leitura()
}

#[derive(Deserialize)]
struct ArgsDoRegistro {
    demanda: String,
    entregavel: u32,
    campos: Value,
    #[serde(default)]
    mudancas: Option<String>,
}

/// Confere as regras citadas contra as permitidas e copia a evidência da regra.
fn conferir_regras(
    citadas: &[String],
    permitidas: &[RegraDaBase],
    base: &[(FrontmatterRegra, String)],
    origem: &str,
) -> Result<Vec<RegraCitada>, ErroFerramenta> {
    let lista = || {
        if permitidas.is_empty() {
            "nenhuma".to_owned()
        } else {
            permitidas
                .iter()
                .map(|r| r.id.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        }
    };
    citadas
        .iter()
        .map(|texto| {
            if let Some(r) = permitidas.iter().find(|r| r.id.to_string() == *texto) {
                return Ok(RegraCitada {
                    id: r.id.clone(),
                    titulo: r.titulo.clone(),
                    evidencias: r.evidencias.clone(),
                });
            }
            let motivo = match base.iter().find(|(r, _)| r.id.to_string() == *texto) {
                None => format!("a regra {texto} não existe na base deste projeto"),
                Some((r, _)) if r.status == StatusRegra::Obsoleta => {
                    format!("a regra {texto} está obsoleta")
                }
                Some(_) => format!("a regra {texto} não é uma regra confirmada {origem}"),
            };
            Err(ErroFerramenta::Recusa(format!(
                "{motivo}. Cada item cita só regras confirmadas {origem}: {}. A evidência \
                 vem da regra; não cite regra de fora nem invente uma",
                lista()
            )))
        })
        .collect()
}

/// Regras confirmadas da base do entregável que nenhum item cita.
fn sem_cobertura(permitidas: &[RegraDaBase], e: &Entregavel) -> Vec<String> {
    let citadas: Vec<String> = e
        .atual()
        .regras_citadas()
        .into_iter()
        .map(ToString::to_string)
        .collect();
    permitidas
        .iter()
        .map(|r| r.id.to_string())
        .filter(|id| !citadas.contains(id))
        .collect()
}

/// "RN-FAT-005 sem história", "RN-FAT-005 e RN-FAT-006 sem história".
pub fn aviso_de_cobertura(t: &TipoEntregavel, faltam: &[String]) -> Option<String> {
    let (ultima, resto) = faltam.split_last()?;
    let lista = if resto.is_empty() {
        ultima.clone()
    } else {
        format!("{} e {ultima}", resto.join(", "))
    };
    Some(format!("{lista} {}", t.sem_item()))
}

/// Arquivos citados nas evidências das regras, para o documento vivo saber
/// quando ficou desatualizado.
fn arquivos_das_regras(regras: &[RegraCitada]) -> Vec<String> {
    let mut arquivos: Vec<String> = Vec::new();
    for e in regras.iter().flat_map(|r| &r.evidencias) {
        let a = e.arquivo.as_str().to_owned();
        if !arquivos.contains(&a) {
            arquivos.push(a);
        }
    }
    arquivos
}

/// `registrar_entregavel`: registra um item no rascunho atual.
pub fn registrar_entregavel() -> Ferramenta {
    let mut props = propriedades_do_entregavel();
    props.insert(
        "campos".into(),
        json!({
            "type": "object",
            "description": "Os campos do item, como o esquema do tipo devolvido por iniciar_entregavel declara. `regras` lista as regras confirmadas da análise de origem que sustentam o item (RN-<SIGLA>-NNN)."
        }),
    );
    props.insert(
        "mudancas".into(),
        json!({
            "type": "string", "maxLength": 500,
            "description": "Só no documento vivo: o que muda em relação à versão publicada, em uma frase, para o histórico de revisões."
        }),
    );
    let schema = json!({
        "type": "object",
        "properties": Value::Object(props),
        "required": ["demanda", "entregavel", "campos"],
        "additionalProperties": false
    });
    Ferramenta::nova(
        "registrar_entregavel",
        "Registrar um item do entregável",
        "Registra um item no rascunho do entregável E-n da demanda (aberto por \
         `iniciar_entregavel`): os campos seguem o esquema do tipo, e cada regra citada em \
         `regras` precisa ser uma regra confirmada da análise de origem; regra inexistente ou \
         de fora é recusada, e a evidência vem da regra. No entregável por demanda, cada \
         chamada acrescenta um item; no documento vivo, o item é o documento inteiro e vira a \
         proposta de versão. A resposta avisa as regras confirmadas ainda sem item. O \
         entregável fica como rascunho até a revisão final do analista.",
        schema,
        |ambiente, args| {
            let args: ArgsDoRegistro = ler_argumentos(args)?;
            let demanda_id = ler_demanda(&args.demanda)?;
            registrar(
                ambiente,
                demanda_id,
                args.entregavel,
                args.campos,
                args.mudancas,
            )
        },
    )
}

fn registrar(
    ambiente: &Ambiente,
    demanda_id: IdDemanda,
    numero: u32,
    campos: Value,
    mudancas: Option<String>,
) -> Result<Saida, ErroFerramenta> {
    let (lida, e) = carregar(ambiente, demanda_id, numero)?;
    let t = tipo_do_catalogo(&e.tipo)?;
    if let Err(erros) = t.validar_campos(&campos) {
        let lista: Vec<String> = erros.iter().map(ToString::to_string).collect();
        return Err(ErroFerramenta::Recusa(format!(
            "os campos não seguem o esquema de {}: {}. Confira o esquema devolvido por \
             iniciar_entregavel",
            t.nome,
            lista.join("; ")
        )));
    }
    let analise = ambiente.analises.carregar(&e.analise)?;
    let base = regras_da_base(&ambiente.kb);
    let mut permitidas = regras_da_analise_em(&base, &analise);
    let mut origem = format!("da análise {}", analise.id);
    if e.finalidade == Finalidade::VersaoInicial
        && let Some(f) = &e.funcionalidade
    {
        for r in regras_da_funcionalidade_em(&base, f) {
            if !permitidas.iter().any(|p| p.id == r.id) {
                permitidas.push(r);
            }
        }
        origem = format!("da análise {} ou da funcionalidade {f}", analise.id);
    }
    let regras = conferir_regras(&t.regras_citadas(&campos), &permitidas, &base, &origem)?;
    let vivo = t.ciclo == CicloDeVida::Vivo;
    let titulos: Vec<(String, String)> = regras
        .iter()
        .map(|r| (r.id.to_string(), r.titulo.clone()))
        .collect();
    let texto = t.markdown_do_item(&campos, &titulos, if vivo { 1 } else { 2 });

    // O tipo vivo: o item é a proposta de versão do documento da funcionalidade.
    let mut documento = e.documento.clone();
    if vivo {
        let f = e.funcionalidade.clone().ok_or_else(|| {
            ErroFerramenta::Interno(format!("o entregável E-{numero} vivo sem funcionalidade"))
        })?;
        let mudancas = mudancas
            .map(|m| m.trim().to_owned())
            .filter(|m| !m.is_empty())
            .or_else(|| {
                (e.finalidade == Finalidade::VersaoInicial)
                    .then(|| MUDANCAS_DA_VERSAO_INICIAL.to_owned())
            })
            .ok_or_else(|| {
                ErroFerramenta::Argumentos(
                    "`mudancas`: diga em uma frase o que muda em relação à versão publicada, \
                     para o histórico de revisões"
                        .into(),
                )
            })?;
        let registrada = documentos_vivos::propor(
            ambiente,
            PedidoDeProposta {
                tipo: t.id.to_owned(),
                dono: Dono::Funcionalidade(f),
                caminho: e.documento.clone(),
                proposta: NovaProposta {
                    texto: texto.clone(),
                    mudancas,
                    demanda: Some(demanda_id),
                    analise: Some(e.analise.clone()),
                    arquivos: arquivos_das_regras(&regras),
                },
            },
        )?;
        documento = Some(registrada.caminho);
    }

    let rascunho = e.atual().versao;
    let (estado, item) = EntregaveisDaDemanda::alterar(&ambiente.kb, &lida.pasta, |d, agora| {
        let alvo = d.entregavel_mut(numero)?;
        // "Gerar de novo" no meio do registro: o item era do rascunho anterior.
        if alvo.atual().versao != rascunho {
            return Err(ErroFerramenta::Recusa(format!(
                "o analista gerou de novo o entregável E-{numero} (agora no rascunho v{}); \
                 chame iniciar_entregavel de novo e registre sobre o rascunho atual",
                alvo.atual().versao
            )));
        }
        let item = alvo.registrar_item(
            NovoItem {
                campos,
                regras,
                texto,
            },
            item_unico(t),
            agora,
        )?;
        alvo.documento = documento;
        Ok::<_, ErroFerramenta>(item)
    })?;
    let e = estado.entregavel(numero).expect("acabou de registrar");
    let faltam = sem_cobertura(&permitidas, e);
    let aviso = aviso_de_cobertura(t, &faltam);
    let mut texto = format!(
        "Item {item} registrado no rascunho v{} do entregável E-{numero} ({}).",
        e.atual().versao,
        t.nome
    );
    if let Some(c) = &e.documento {
        let _ = write!(texto, " Proposta de versão em `{c}`.");
    }
    match &aviso {
        Some(a) => {
            let _ = write!(
                texto,
                " Cobertura: {a}. Cubra essas regras ou diga ao analista por que ficaram de fora."
            );
        }
        None => texto.push_str(" Todas as regras confirmadas têm item."),
    }
    texto.push_str(" O entregável fica como rascunho até a revisão final do analista.");
    Ok(Saida {
        texto,
        dados: Some(json!({
            "entregavel": numero,
            "rascunho": e.atual().versao,
            "item": item,
            "documento": e.documento,
            "regras_sem_item": faltam,
            "aviso": aviso,
        })),
    })
}

// ---------------------------------------------------------------------------
// Revisão e destino (o app)

fn alterar_entregavel<T>(
    ambiente: &Ambiente,
    demanda_id: IdDemanda,
    numero: u32,
    f: impl FnOnce(&mut Entregavel, u64) -> Result<T, ErroFerramenta>,
) -> Result<(Entregavel, T), ErroFerramenta> {
    let lida = demanda(ambiente, demanda_id)?;
    let (estado, r) = EntregaveisDaDemanda::alterar(&ambiente.kb, &lida.pasta, |d, agora| {
        f(d.entregavel_mut(numero)?, agora)
    })?;
    let e = estado
        .entregavel(numero)
        .cloned()
        .expect("acabou de alterar");
    Ok((e, r))
}

/// Edição do analista no texto do rascunho atual: volta a rascunho. No tipo
/// vivo, a proposta de versão recebe o texto (e é marcada de novo).
pub fn editar(
    ambiente: &Ambiente,
    demanda_id: IdDemanda,
    numero: u32,
    texto: &str,
) -> Result<Entregavel, ErroFerramenta> {
    let (_, atual) = carregar(ambiente, demanda_id, numero)?;
    if let Some(c) = &atual.documento {
        documentos_vivos::editar_proposta(ambiente, c, texto, None)?;
    }
    let (e, ()) = alterar_entregavel(ambiente, demanda_id, numero, |e, agora| {
        Ok(e.editar(texto, agora)?)
    })?;
    Ok(e)
}

/// Revisão final do rascunho atual ("Marcar como revisado"). No tipo vivo, a
/// proposta do documento também.
pub fn marcar_revisado(
    ambiente: &Ambiente,
    demanda_id: IdDemanda,
    numero: u32,
    por: &str,
) -> Result<Entregavel, ErroFerramenta> {
    // A proposta primeiro: se ela foi descartada ou publicada (na visão
    // Documentação do mapa), o entregável não fica revisado com "Publicar"
    // habilitado sem nada a publicar.
    let (_, atual) = carregar(ambiente, demanda_id, numero)?;
    if let Some(c) = &atual.documento {
        documentos_vivos::marcar_revisada(ambiente, c, por)?;
    }
    let (e, ()) = alterar_entregavel(ambiente, demanda_id, numero, |e, agora| {
        Ok(e.marcar_revisado(por, agora)?)
    })?;
    Ok(e)
}

/// O texto que sai: o do rascunho atual, com a origem no fim.
fn texto_para_enviar(e: &Entregavel, demanda: IdDemanda, por: &str) -> String {
    let revisor = e.revisado.as_ref().map_or(por, |r| r.por.as_str());
    format!(
        "{}\n\n---\n\n_Entregável E-{} da demanda {demanda} (rascunho v{}), gerado a partir da \
         análise {} e revisado por {revisor}._\n",
        e.atual().texto().trim(),
        e.numero,
        e.atual().versao,
        e.analise
    )
}

/// Exporta o rascunho revisado em Markdown para `destino` e registra o envio.
pub fn exportar(
    ambiente: &Ambiente,
    demanda_id: IdDemanda,
    numero: u32,
    destino: &Path,
    por: &str,
) -> Result<Entregavel, ErroFerramenta> {
    let (_, atual) = carregar(ambiente, demanda_id, numero)?;
    atual.conferir_envio()?;
    let versao = atual.atual().versao;
    let destino = if destino.extension().is_some_and(|e| e == "md") {
        destino.to_path_buf()
    } else {
        destino.with_extension("md")
    };
    let destino = destino.as_path();
    let pasta = destino
        .parent()
        .filter(|p| p.is_dir())
        .and_then(|p| dunce::canonicalize(p).ok())
        .ok_or_else(|| {
            ErroFerramenta::Recusa(format!(
                "a pasta de {} não existe; escolha outra",
                destino.display()
            ))
        })?;
    if ambiente
        .escopo
        .pastas()
        .iter()
        .any(|p| pasta.starts_with(&p.caminho))
    {
        return Err(ErroFerramenta::Recusa(
            "o destino fica dentro das pastas do projeto; o app não grava no repositório. \
             Escolha outra pasta"
                .into(),
        ));
    }
    let texto = texto_para_enviar(&atual, demanda_id, por);
    std::fs::write(destino, texto.as_bytes()).map_err(|e| {
        ErroFerramenta::Recusa(format!(
            "não foi possível gravar `{}`: {e}",
            destino.display()
        ))
    })?;
    let onde = destino.display().to_string();
    let (e, _) = alterar_entregavel(ambiente, demanda_id, numero, |e, agora| {
        e.registrar_envio_feito(Destino::Exportar, &onde, versao, por, agora)?;
        Ok(())
    })?;
    Ok(e)
}

/// Caminho na base do entregável por demanda publicado:
/// `demandas/<pasta>/entregaveis/E-<n>-<tipo>.md`.
pub fn caminho_publicado(pasta: &CaminhoRelativo, e: &Entregavel) -> CaminhoRelativo {
    pasta
        .juntar(&format!(
            "{}/E-{}-{}.{}",
            layout::ENTREGAVEIS,
            e.numero,
            slug(&e.tipo),
            layout::EXTENSAO
        ))
        .expect("nome montado é caminho válido")
}

/// Publica o rascunho revisado na base: o entregável por demanda vira um
/// documento em `demandas/<pasta>/entregaveis/`, que a publicação leva ao
/// repositório de documentação; o vivo vira a versão nova do documento da
/// funcionalidade, com o histórico de revisões.
pub fn publicar(
    ambiente: &Ambiente,
    demanda_id: IdDemanda,
    numero: u32,
    por: &str,
) -> Result<Entregavel, ErroFerramenta> {
    let (lida, atual) = carregar(ambiente, demanda_id, numero)?;
    atual.conferir_envio()?;
    let versao = atual.atual().versao;
    let t = tipo_do_catalogo(&atual.tipo)?;
    let onde = if let Some(c) = &atual.documento {
        let p = documentos_vivos::publicar(ambiente, c)?;
        format!("{} (versão {})", p.caminho, p.revisao.versao)
    } else {
        let caminho = caminho_publicado(&lida.pasta, &atual);
        let revisor = atual.revisado.as_ref().map_or(por, |r| r.por.as_str());
        let doc = Documento::novo(
            FrontmatterEntregavel {
                tipo: TipoDocumento::Entregavel,
                titulo: format!("{} · {}", t.nome, lida.documento.frontmatter.titulo),
                entregavel: atual.tipo.clone(),
                demanda: demanda_id,
                analise: atual.analise.clone(),
                versao: atual.atual().versao,
                revisor: revisor.to_owned(),
                publicado: cba_core::tempo::hoje(),
            },
            format!("\n{}\n", atual.atual().texto().trim()),
        );
        ambiente.kb.gravar(&caminho, &doc).map_err(interno)?;
        caminho.to_string()
    };
    let (e, _) = alterar_entregavel(ambiente, demanda_id, numero, |e, agora| {
        e.registrar_envio_feito(Destino::RepositorioDocs, &onde, versao, por, agora)?;
        Ok(())
    })?;
    Ok(e)
}

/// Comentário pronto para a issue da demanda no rastreador.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComentarioPreparado {
    pub texto: String,
    /// Endereço da issue, quando a demanda veio do rastreador.
    pub link: Option<String>,
    pub chave: Option<String>,
}

/// Prepara o comentário do rascunho revisado para a issue da demanda e registra
/// o envio. O app copia o texto e abre a issue; não posta sozinho.
pub fn preparar_comentario(
    ambiente: &Ambiente,
    demanda_id: IdDemanda,
    numero: u32,
    por: &str,
) -> Result<(Entregavel, ComentarioPreparado), ErroFerramenta> {
    let (lida, atual) = carregar(ambiente, demanda_id, numero)?;
    atual.conferir_envio()?;
    let versao = atual.atual().versao;
    let f = &lida.documento.frontmatter;
    let link = f
        .origem
        .clone()
        .filter(|o| o.starts_with("http://") || o.starts_with("https://"));
    let chave = f.chave.clone();
    let onde = chave
        .clone()
        .or_else(|| link.clone())
        .unwrap_or_else(|| demanda_id.to_string());
    let comentario = ComentarioPreparado {
        texto: texto_para_enviar(&atual, demanda_id, por),
        link,
        chave,
    };
    let (e, _) = alterar_entregavel(ambiente, demanda_id, numero, |e, agora| {
        e.registrar_envio_feito(Destino::ComentarioRastreador, &onde, versao, por, agora)?;
        Ok(())
    })?;
    Ok((e, comentario))
}

// ---------------------------------------------------------------------------
// Vista da seção "Entregáveis" da demanda

/// Um tipo habilitado no projeto, como a demanda oferece.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TipoNaDemanda {
    pub id: String,
    pub nome: String,
    pub descricao: String,
    pub ciclo: CicloDeVida,
    pub geracao: Geracao,
    pub destino: Destino,
    pub padrao: bool,
    /// Por que não dá para gerar na demanda (o tipo montado pelo app).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indisponivel: Option<String>,
}

/// Evidência de uma regra citada, com o trecho do código.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenciaVista {
    pub arquivo: String,
    pub inicio: u32,
    pub fim: u32,
    /// As linhas citadas, lidas agora; sem elas, o arquivo não pôde ser lido.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trecho: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegraVista {
    pub id: String,
    pub titulo: String,
    pub evidencias: Vec<EvidenciaVista>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemVisto {
    pub numero: u32,
    pub texto: String,
    pub regras: Vec<RegraVista>,
}

/// Trecho da proposta de versão do documento vivo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrechoVisto {
    pub marca: Marca,
    pub texto: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anterior: Option<String>,
}

/// A proposta de versão do documento vivo do entregável.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PropostaVista {
    pub caminho: String,
    pub versao: Versao,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base: Option<Versao>,
    pub mudancas: String,
    pub trechos: Vec<TrechoVisto>,
    /// Versões publicadas: versão, data, demanda e o que mudou.
    pub historico: Vec<RevisaoVista>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RevisaoVista {
    pub versao: Versao,
    pub data: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub demanda: Option<String>,
    pub revisor: String,
    pub mudancas: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvioVisto {
    pub destino: Destino,
    pub onde: String,
    pub versao: u32,
    pub por: String,
    pub em_ms: u64,
}

/// Um entregável da demanda, como a seção mostra.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntregavelVisto {
    pub numero: u32,
    pub tipo: String,
    pub nome: String,
    pub ciclo: CicloDeVida,
    pub analise: String,
    pub finalidade: Finalidade,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub funcionalidade: Option<String>,
    pub destino: Destino,
    pub situacao: Situacao,
    /// Versão do rascunho atual e as anteriores, guardadas.
    pub versao: u32,
    pub versoes: Vec<u32>,
    pub texto: String,
    pub editado: bool,
    pub itens: Vec<ItemVisto>,
    /// Regras confirmadas sem item.
    pub sem_cobertura: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aviso_cobertura: Option<String>,
    /// O que conferir na revisão final.
    pub conferir: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revisado_por: Option<String>,
    pub envios: Vec<EnvioVisto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proposta: Option<PropostaVista>,
    /// O prompt para continuar a sessão do agente neste rascunho.
    pub prompt: String,
}

/// Documento vivo de um tipo numa funcionalidade.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentoVivoVisto {
    pub tipo: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caminho: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub versao: Option<Versao>,
    pub estado: String,
}

/// Funcionalidade que um tipo vivo pode documentar, com o estado dos documentos.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FuncionalidadeVista {
    pub id: String,
    pub nome: String,
    /// A análise validada se refere a ela.
    pub da_analise: bool,
    pub documentos: Vec<DocumentoVivoVisto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegraDaBaseVista {
    pub id: String,
    pub titulo: String,
}

/// Análise validada, como base da geração.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BaseVista {
    pub analise: String,
    pub titulo: String,
    pub fluxo: Fluxo,
    pub regras: Vec<RegraDaBaseVista>,
    pub decisoes: Vec<DecisaoRespondida>,
    pub perguntas: Vec<PerguntaAberta>,
    pub funcionalidades: Vec<String>,
}

/// O que a seção "Entregáveis" da demanda mostra.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VistaEntregaveis {
    pub demanda: String,
    pub aberta: bool,
    /// Sem análise validada, a seção não oferece a geração.
    pub analises: Vec<BaseVista>,
    pub tipos: Vec<TipoNaDemanda>,
    pub entregaveis: Vec<EntregavelVisto>,
    /// As funcionalidades abertas do mapa, com o estado dos documentos vivos
    /// dos tipos vivos habilitados; as da análise primeiro.
    pub funcionalidades: Vec<FuncionalidadeVista>,
}

fn trecho(ambiente: &Ambiente, e: &Evidencia) -> Option<String> {
    let aberto = ArquivoDoProjeto::abrir(&ambiente.escopo, e.arquivo.as_str()).ok()?;
    let inicio = e.linhas.inicio.max(1) as usize;
    let fim = (e.linhas.fim as usize).max(inicio);
    let linhas: Vec<&str> = aberto
        .texto
        .lines()
        .skip(inicio - 1)
        .take((fim - inicio + 1).min(MAX_LINHAS_DO_TRECHO))
        .collect();
    (!linhas.is_empty()).then(|| linhas.join("\n"))
}

fn regra_vista(ambiente: &Ambiente, r: &RegraCitada) -> RegraVista {
    RegraVista {
        id: r.id.to_string(),
        titulo: r.titulo.clone(),
        evidencias: r
            .evidencias
            .iter()
            .map(|e| EvidenciaVista {
                arquivo: e.arquivo.to_string(),
                inicio: e.linhas.inicio,
                fim: e.linhas.fim,
                trecho: trecho(ambiente, e),
            })
            .collect(),
    }
}

fn o_que_conferir(e: &Entregavel, t: &TipoEntregavel, aviso: Option<&str>) -> Vec<String> {
    let mut lista = vec![
        "Cada item cita as regras certas: confira o trecho de cada uma em \"Mostrar evidências\"."
            .to_owned(),
        "As decisões respondidas da análise estão refletidas no texto.".to_owned(),
        "As perguntas em aberto ficaram como pendência, e não como fato.".to_owned(),
    ];
    if t.ciclo == CicloDeVida::Vivo {
        lista.push(match e.finalidade {
            Finalidade::VersaoInicial => {
                "A versão inicial descreve o sistema como é hoje, sem a mudança da demanda."
                    .to_owned()
            }
            Finalidade::Demanda => {
                "Os trechos marcados como novos, alterados e removidos batem com a demanda."
                    .to_owned()
            }
        });
    }
    if let Some(a) = aviso {
        lista.push(format!("Cobertura: {a}."));
    }
    lista
}

fn proposta_vista(docs: &DocumentosVivos, caminho: &CaminhoRelativo) -> Option<PropostaVista> {
    let d = docs.documento(caminho)?;
    let p = d.proposta.as_ref()?;
    Some(PropostaVista {
        caminho: caminho.to_string(),
        versao: p.versao,
        base: p.base,
        mudancas: p.mudancas.clone(),
        trechos: p
            .trechos
            .iter()
            .map(|t| TrechoVisto {
                marca: t.marca,
                texto: t.texto.clone(),
                anterior: t.anterior.clone(),
            })
            .collect(),
        historico: d
            .historico
            .iter()
            .map(|r| RevisaoVista {
                versao: r.versao,
                data: r.data.to_string(),
                demanda: r.demanda.map(|d| d.to_string()),
                revisor: r.revisor.clone(),
                mudancas: r.mudancas.clone(),
            })
            .collect(),
    })
}

/// A seção "Entregáveis" da demanda: as análises validadas (a base), os tipos
/// habilitados no projeto, os entregáveis gerados (com a cobertura e o que
/// conferir) e as funcionalidades com o estado dos documentos vivos.
pub fn vista_da_demanda(
    ambiente: &Ambiente,
    demanda_id: IdDemanda,
) -> Result<VistaEntregaveis, ErroFerramenta> {
    let lida = demanda(ambiente, demanda_id)?;
    let regras = regras_da_base(&ambiente.kb);
    let validadas = analises_validadas(ambiente, demanda_id)?;
    let bases: Vec<BaseDoEntregavel> = validadas
        .iter()
        .map(|a| base_em(ambiente, &regras, a))
        .collect();

    let config = catalogo::efetiva(ambiente.projeto.entregaveis.as_ref());
    let tipos: Vec<TipoNaDemanda> = config
        .tipos
        .iter()
        .filter_map(|h| catalogo::tipo(&h.tipo).map(|t| (h, t)))
        .map(|(h, t)| TipoNaDemanda {
            id: t.id.to_owned(),
            nome: t.nome.to_owned(),
            descricao: t.descricao.to_owned(),
            ciclo: t.ciclo,
            geracao: t.geracao,
            destino: h.destino,
            padrao: config.padrao.as_deref() == Some(t.id),
            indisponivel: (t.geracao == Geracao::App).then(|| {
                "Montado pelo app, sem IA: a montagem na demanda chega numa próxima versão."
                    .to_owned()
            }),
        })
        .collect();

    let docs = DocumentosVivos::carregar(&ambiente.kb).map_err(interno)?;
    let mapa = MapaFuncionalidades::carregar(&ambiente.kb).map_err(interno)?;
    let vivos: Vec<&str> = tipos
        .iter()
        .filter(|t| t.ciclo == CicloDeVida::Vivo)
        .map(|t| t.id.as_str())
        .collect();
    let da_analise: BTreeSet<String> = bases
        .iter()
        .flat_map(|b| b.funcionalidades.iter().map(ToString::to_string))
        .collect();
    let mut funcionalidades: Vec<FuncionalidadeVista> = if vivos.is_empty() {
        Vec::new()
    } else {
        mapa.abertas()
            .map(|f| FuncionalidadeVista {
                id: f.id.to_string(),
                nome: f.nome.clone(),
                da_analise: da_analise.contains(&f.id.to_string()),
                documentos: documentos_vivos::documentos_da_funcionalidade(
                    &ambiente.escopo,
                    &docs,
                    f,
                    &vivos,
                )
                .into_iter()
                .map(|d| DocumentoVivoVisto {
                    tipo: d.tipo,
                    caminho: d.caminho.map(|c| c.to_string()),
                    versao: d.versao,
                    estado: d.estado.rotulo().to_owned(),
                })
                .collect(),
            })
            .collect()
    };
    funcionalidades.sort_by_key(|f| !f.da_analise);

    let estado = EntregaveisDaDemanda::carregar(&ambiente.kb, &lida.pasta)?;
    let mut entregaveis = Vec::new();
    for e in &estado.entregaveis {
        let Some(t) = catalogo::tipo(&e.tipo) else {
            continue;
        };
        let mut permitidas = validadas
            .iter()
            .find(|a| a.id == e.analise)
            .map(|a| regras_da_analise_em(&regras, a))
            .unwrap_or_default();
        if e.finalidade == Finalidade::VersaoInicial
            && let Some(f) = &e.funcionalidade
        {
            for r in regras_da_funcionalidade_em(&regras, f) {
                if !permitidas.iter().any(|p| p.id == r.id) {
                    permitidas.push(r);
                }
            }
        }
        let atual = e.atual();
        let faltam = if atual.vazio() {
            Vec::new()
        } else {
            sem_cobertura(&permitidas, e)
        };
        let aviso = aviso_de_cobertura(t, &faltam);
        entregaveis.push(EntregavelVisto {
            numero: e.numero,
            tipo: e.tipo.clone(),
            nome: t.nome.to_owned(),
            ciclo: t.ciclo,
            analise: e.analise.to_string(),
            finalidade: e.finalidade,
            funcionalidade: e.funcionalidade.as_ref().map(ToString::to_string),
            destino: e.destino,
            situacao: e.situacao(),
            versao: atual.versao,
            versoes: e.rascunhos.iter().map(|r| r.versao).collect(),
            texto: atual.texto(),
            editado: atual.texto_editado.is_some(),
            itens: atual
                .itens
                .iter()
                .map(|i| ItemVisto {
                    numero: i.numero,
                    texto: i.texto.clone(),
                    regras: i.regras.iter().map(|r| regra_vista(ambiente, r)).collect(),
                })
                .collect(),
            conferir: o_que_conferir(e, t, aviso.as_deref()),
            sem_cobertura: faltam,
            aviso_cobertura: aviso,
            revisado_por: e.revisado.as_ref().map(|r| r.por.clone()),
            envios: e
                .envios
                .iter()
                .map(|v| EnvioVisto {
                    destino: v.destino,
                    onde: v.onde.clone(),
                    versao: v.versao,
                    por: v.por.clone(),
                    em_ms: v.em_ms,
                })
                .collect(),
            proposta: e.documento.as_ref().and_then(|c| proposta_vista(&docs, c)),
            prompt: prompt_de_geracao(demanda_id, e.numero),
        });
    }

    Ok(VistaEntregaveis {
        demanda: demanda_id.to_string(),
        aberta: lida.aberta(),
        analises: bases
            .into_iter()
            .map(|b| BaseVista {
                analise: b.analise.to_string(),
                titulo: b.titulo,
                fluxo: b.fluxo,
                regras: b
                    .regras
                    .into_iter()
                    .map(|r| RegraDaBaseVista {
                        id: r.id.to_string(),
                        titulo: r.titulo,
                    })
                    .collect(),
                decisoes: b.decisoes,
                perguntas: b.perguntas,
                funcionalidades: b.funcionalidades.iter().map(ToString::to_string).collect(),
            })
            .collect(),
        tipos,
        entregaveis,
        funcionalidades,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aviso_de_cobertura_junta_as_regras() {
        let h = catalogo::tipo(catalogo::HISTORIA).unwrap();
        assert_eq!(aviso_de_cobertura(h, &[]), None);
        assert_eq!(
            aviso_de_cobertura(h, &["RN-FAT-005".into()]).as_deref(),
            Some("RN-FAT-005 sem história")
        );
        assert_eq!(
            aviso_de_cobertura(
                h,
                &[
                    "RN-FAT-004".into(),
                    "RN-FAT-005".into(),
                    "RN-FAT-006".into()
                ]
            )
            .as_deref(),
            Some("RN-FAT-004, RN-FAT-005 e RN-FAT-006 sem história")
        );
    }

    #[test]
    fn enunciado_tira_o_titulo_e_os_comentarios() {
        let id = Id::analisar("RN-FAT-001").unwrap();
        assert_eq!(
            enunciado(
                "\n# RN-FAT-001 · Multa\n\n<!-- nota -->\nA multa é de 2%.\n\nAo mês.\n",
                &id
            ),
            "A multa é de 2%. Ao mês."
        );
    }

    #[test]
    fn prompt_abre_a_sessao_pelo_iniciar_entregavel() {
        let p = prompt_de_geracao(IdDemanda::novo(3), 2);
        assert!(p.contains("iniciar_entregavel"), "{p}");
        assert!(p.contains("demanda \"D-0003\" e entregavel 2"), "{p}");
    }
}
