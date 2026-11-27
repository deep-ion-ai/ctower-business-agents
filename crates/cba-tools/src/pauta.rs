//! Pauta de perguntas pendentes (#123): as perguntas para o solicitante de todas
//! as análises e as dúvidas das atas aprovadas, de um projeto ou de uma demanda,
//! juntas e agrupadas por destinatário, com a origem, a idade e o que cada uma
//! bloqueia; a resposta registrada no app volta para cada pergunta de origem.
//!
//! ## Pauta
//!
//! - **De onde vêm as perguntas:** as abertas de cada análise (do projeto ou
//!   ligadas à demanda, como em [`crate::registros::demanda_da_analise`]) e as
//!   dúvidas sem resposta das atas aprovadas (as em rascunho ainda mudam). Cada
//!   uma tem uma referência estável: `A-0001/2` (a pergunta 2 da análise) ou
//!   `R-0001/DV-1` (a dúvida 1 da ata).
//! - **Destinatário:** o que o analista escolheu na pauta (uma pessoa do catálogo
//!   ou um papel) e, na dúvida da ata sem escolha, a pessoa do catálogo que casa
//!   com o responsável do item. Sem nenhum, a pergunta fica em "sem destinatário".
//! - **Perguntas iguais:** com o mesmo destinatário e o mesmo texto (sem
//!   diferença de acentos, maiúsculas e pontuação) ou quase o mesmo (80% das
//!   palavras em comum), viram um item só da pauta, sem perder as originais
//!   ([`ItemDaPauta::perguntas`]): a resposta fecha todas.
//! - **Idade:** dias desde o registro da pergunta (ou desde a reunião); a partir
//!   de [`DIAS_PARA_ANTIGA`], a pergunta é antiga e fica em destaque.
//! - **O que bloqueia:** os critérios de aceite das histórias que dependem da
//!   pergunta (`HU-0001 · CA-2`).
//! - **Decisões para o negócio** (#452): a pergunta da análise registrada como
//!   decisão leva as opções, com o efeito de cada uma, e a recomendação
//!   ([`DecisaoDaPauta`]). Em cada grupo, as decisões vêm primeiro; uma decisão
//!   nunca se junta a uma dúvida. A resposta é a opção escolhida pelo analista.
//!
//! ## Resposta
//!
//! [`registrar_resposta`] fecha as perguntas escolhidas com o texto, quem
//! respondeu, a data e a fonte. Antes de gravar, confere que todas existem e
//! estão em aberto e que o destino da resposta é válido; as análises são
//! alteradas sob uma só trava ([`cba_core::analises::Analises::alterar_varias`]),
//! todas ou nenhuma, e cada ata sob a trava dela. Depois, conforme o destino:
//!
//! - **só registrar;**
//! - **virar regra:** cria `regras/RN-<SIGLA>-<NNN>.md` sem evidência de código,
//!   com a origem "informada por <pessoa>" no frontmatter
//!   ([`cba_kb::informada::RegraInformada`]), no status e no início do
//!   enunciado. É a exceção visível ao princípio "sem evidência, não é fato";
//! - **validar (ou invalidar) uma premissa** da demanda (#122), com a resposta
//!   como "como foi validada" e as perguntas de origem.

use std::collections::{BTreeMap, BTreeSet};

use cba_core::ambiente::Ambiente;
use cba_core::analise::{Analise, EstadoPergunta, Fluxo, OpcaoDecisao, Pergunta};
use cba_core::analises;
use cba_core::eventos::Mudanca;
use cba_core::requisito::RefItem;
use cba_core::tempo;
use cba_docs::{
    Conteudo, EstadoAta, Formato, IdItemAta, ItemAta, OpcoesExportacao, Templates, TipoItem,
};
use cba_kb::frontmatter::{Data, FrontmatterRegra, StatusAta, StatusRegra};
use cba_kb::indice::Indice;
use cba_kb::informada::{Destinatario, Informacao, RegraInformada, RespostaInformada};
use cba_kb::registros::{StatusPremissa, ValidacaoDePremissa};
use cba_kb::{
    CaminhoRelativo, Categoria, Id, IdDemanda, IdRegistro, IdReuniao, PessoaLida, Serie,
    TipoDocumento, normalizar_texto,
};
use serde::{Deserialize, Serialize};

use crate::aprovacao::{AREA_GERAL, resolver_area, titulo_da_regra};
use crate::registros::demanda_da_analise;

/// Dias a partir dos quais uma pergunta sem resposta é antiga.
pub const DIAS_PARA_ANTIGA: i64 = 14;

/// Parte das palavras em comum (Jaccard) a partir da qual duas perguntas para o
/// mesmo destinatário são a mesma.
const SEMELHANCA_MINIMA: f64 = 0.8;

/// Rótulo do grupo das perguntas sem destinatário.
pub const SEM_DESTINATARIO: &str = "Sem destinatário";

/// Erro ao montar a pauta ou registrar uma resposta.
#[derive(Debug, thiserror::Error)]
pub enum ErroPauta {
    #[error(transparent)]
    Base(#[from] cba_kb::Erro),
    #[error(transparent)]
    Analise(#[from] analises::Erro),
    #[error(transparent)]
    Ata(#[from] cba_docs::ErroAta),
    #[error(transparent)]
    Montagem(#[from] cba_docs::ErroMontagem),
    #[error(transparent)]
    Exportacao(#[from] cba_docs::ErroExportacao),
    #[error(
        "`{0}` não é uma pergunta da pauta; use A-NNNN/n (pergunta da análise) ou \
         R-NNNN/DV-n (dúvida da ata)"
    )]
    Referencia(String),
    #[error("a pergunta {0} não está em aberto (já foi respondida ou descartada)")]
    NaoPendente(String),
    #[error("escolha ao menos uma pergunta para fechar com a resposta")]
    SemPerguntas,
    #[error("o campo `{0}` não pode ficar vazio")]
    CampoVazio(&'static str),
    #[error("{0} não é uma premissa; escolha uma premissa da demanda")]
    NaoEPremissa(String),
    #[error(transparent)]
    Regra(#[from] crate::aprovacao::ErroAprovacao),
}

// ---------------------------------------------------------------------------
// Referências

/// Pergunta da pauta: a pergunta de uma análise ou a dúvida de uma ata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Referencia {
    Analise(u32, u32),
    Ata(IdReuniao, IdItemAta),
}

impl Referencia {
    fn analisar(texto: &str) -> Result<Self, ErroPauta> {
        let texto = texto.trim();
        let erro = || ErroPauta::Referencia(texto.to_owned());
        if let Ok(r) = RefItem::analisar(texto) {
            return Ok(Self::Analise(r.analise.numero(), r.numero));
        }
        let (reuniao, item) = texto.split_once('/').ok_or_else(erro)?;
        let reuniao = IdReuniao::analisar(reuniao).map_err(|_| erro())?;
        let item = IdItemAta::analisar(item)
            .filter(|i| i.tipo == TipoItem::Duvida)
            .ok_or_else(erro)?;
        Ok(Self::Ata(reuniao, item))
    }

    fn id_da_analise(numero: u32) -> Id {
        Serie::Analise.id(numero)
    }
}

// ---------------------------------------------------------------------------
// Pauta

/// De onde vem uma pergunta da pauta.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "tipo", rename_all = "camelCase")]
pub enum FontePergunta {
    /// Pergunta para o solicitante de uma análise.
    Analise {
        id: String,
        titulo: String,
        fluxo: Fluxo,
    },
    /// Dúvida de uma ata aprovada, com a data da reunião.
    Ata { id: String, data: String },
}

/// Pergunta pendente, como foi registrada na origem.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PerguntaDaPauta {
    /// `A-0001/2` ou `R-0001/DV-1`.
    pub referencia: String,
    pub fonte: FontePergunta,
    pub texto: String,
    /// O que o sistema faz hoje: a afirmação "a confirmar" que originou a
    /// pergunta, quando há.
    pub contexto: Option<String>,
    pub destinatario: Option<Destinatario>,
    /// Tema da pergunta: a seção da afirmação de origem ou o título da origem.
    pub tema: String,
    /// Desde quando está em aberto (`AAAA-MM-DD`).
    pub desde: String,
    pub idade_dias: i64,
    /// Critérios de aceite que dependem da pergunta (`HU-0001 · CA-2`).
    pub bloqueia: Vec<String>,
    /// Documento de origem, relativo à raiz da base.
    pub documento: String,
    /// As opções, quando a pergunta é uma decisão para o negócio (#452).
    pub decisao: Option<DecisaoDaPauta>,
}

/// Decisão para o negócio na pauta (#452): as opções, com o efeito de cada uma,
/// e a recomendação do agente, se houver.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisaoDaPauta {
    pub opcoes: Vec<OpcaoDecisao>,
    pub recomendacao: Option<String>,
}

/// Item da pauta: uma pergunta ou várias iguais, para o mesmo destinatário.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemDaPauta {
    /// Texto da pauta: o da pergunta mais antiga.
    pub texto: String,
    pub destinatario: Option<Destinatario>,
    /// Perguntas de origem, da mais antiga para a mais nova.
    pub perguntas: Vec<PerguntaDaPauta>,
    /// Idade da pergunta mais antiga, em dias.
    pub idade_dias: i64,
    /// Pendente há [`DIAS_PARA_ANTIGA`] dias ou mais.
    pub antiga: bool,
    /// As opções da decisão, quando o item é uma decisão para o negócio (as da
    /// pergunta mais antiga).
    pub decisao: Option<DecisaoDaPauta>,
}

/// Grupo da pauta: as perguntas de um destinatário.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrupoDaPauta {
    pub destinatario: Option<Destinatario>,
    /// "Maria Silva (PO)", "PO" ou [`SEM_DESTINATARIO`].
    pub rotulo: String,
    pub itens: Vec<ItemDaPauta>,
}

/// Pessoa do catálogo, para escolher o destinatário e quem respondeu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PessoaDaPauta {
    pub slug: String,
    pub nome: String,
    pub papel: Option<String>,
}

/// Premissa da demanda que a resposta pode validar (#122).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PremissaDaPauta {
    /// `PR-0001`.
    pub id: String,
    pub titulo: String,
    pub status: StatusPremissa,
}

/// A pauta de perguntas pendentes de um projeto ou de uma demanda.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pauta {
    /// Título da pauta: a demanda (`D-0001: título`) ou o projeto.
    pub titulo: String,
    /// Grupos por destinatário: pessoas e papéis em ordem alfabética, "sem
    /// destinatário" no fim.
    pub grupos: Vec<GrupoDaPauta>,
    /// Quantas perguntas de origem estão pendentes.
    pub total: usize,
    /// Quantas delas são antigas.
    pub antigas: usize,
    pub pessoas: Vec<PessoaDaPauta>,
    /// Papéis do catálogo de papéis.
    pub papeis: Vec<String>,
    /// Premissas da demanda (na pauta de uma demanda), para validar com a
    /// resposta.
    pub premissas: Vec<PremissaDaPauta>,
    /// `AAAA-MM-DD`.
    pub gerada_em: String,
}

/// Rótulo do destinatário, com o nome e o papel da pessoa do catálogo.
fn rotulo_do_destinatario(d: Option<&Destinatario>, pessoas: &[PessoaLida]) -> String {
    match d {
        None => SEM_DESTINATARIO.to_owned(),
        Some(Destinatario::Papel { papel }) => papel.clone(),
        Some(Destinatario::Pessoa { pessoa }) => match pessoas.iter().find(|p| &p.slug == pessoa) {
            Some(p) => match p.papel_sugerido() {
                Some(s) => format!("{} ({})", p.nome(), s.papel),
                None => p.nome().to_owned(),
            },
            None => pessoa.clone(),
        },
    }
}

/// Palavras com mais de duas letras do texto normalizado.
fn palavras(texto: &str) -> BTreeSet<String> {
    normalizar_texto(texto)
        .split(' ')
        .filter(|p| p.chars().count() > 2)
        .map(str::to_owned)
        .collect()
}

/// Se as duas perguntas são a mesma (veja o módulo).
fn mesma_pergunta(a: &str, b: &str) -> bool {
    if normalizar_texto(a) == normalizar_texto(b) {
        return true;
    }
    let (pa, pb) = (palavras(a), palavras(b));
    if pa.is_empty() || pb.is_empty() {
        return false;
    }
    let comuns = pa.intersection(&pb).count() as f64;
    let todas = pa.union(&pb).count() as f64;
    comuns / todas >= SEMELHANCA_MINIMA
}

/// Critérios de aceite por pergunta de que dependem (`A-0001/2` →
/// `HU-0001 · CA-2`).
fn bloqueios(analises: &[Analise]) -> BTreeMap<String, Vec<String>> {
    let mut mapa: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for a in analises {
        for h in &a.historias {
            for c in &h.criterios {
                if let Some(p) = &c.pergunta {
                    mapa.entry(p.to_string()).or_default().push(format!(
                        "{} · {}",
                        h.id,
                        c.rotulo()
                    ));
                }
            }
        }
    }
    mapa
}

fn idade(desde: Data, hoje: Data) -> i64 {
    tempo::dias_entre(desde, hoje).max(0)
}

fn pergunta_da_analise(
    amb: &Ambiente,
    analise: &Analise,
    p: &Pergunta,
    bloqueios: &BTreeMap<String, Vec<String>>,
    hoje: Data,
) -> PerguntaDaPauta {
    let referencia = format!("{}/{}", analise.id, p.numero);
    let afirmacao = p
        .afirmacao
        .and_then(|n| analise.afirmacoes.iter().find(|a| a.numero == n));
    let desde = tempo::data_utc(p.registrada_ms);
    PerguntaDaPauta {
        fonte: FontePergunta::Analise {
            id: analise.id.to_string(),
            titulo: analise.titulo.clone(),
            fluxo: analise.fluxo,
        },
        texto: p.texto.clone(),
        contexto: afirmacao.map(|a| a.texto.clone()),
        destinatario: p.destinatario.clone(),
        tema: afirmacao
            .map(|a| a.secao.replace('_', " "))
            .unwrap_or_else(|| analise.titulo.clone()),
        desde: desde.to_string(),
        idade_dias: idade(desde, hoje),
        bloqueia: bloqueios.get(&referencia).cloned().unwrap_or_default(),
        documento: amb.kb.na_base(&analise.documento).as_str().to_owned(),
        referencia,
        decisao: p.e_decisao().then(|| DecisaoDaPauta {
            opcoes: p.opcoes.clone(),
            recomendacao: p.recomendacao.clone(),
        }),
    }
}

fn pergunta_da_ata(
    amb: &Ambiente,
    estado: &EstadoAta,
    item: &ItemAta,
    ata: &CaminhoRelativo,
    pessoas: &[PessoaLida],
    hoje: Data,
) -> PerguntaDaPauta {
    let destinatario = item.destinatario.clone().or_else(|| {
        let responsavel = item.responsavel.as_deref()?;
        pessoas
            .iter()
            .find(|p| p.casa_com(responsavel))
            .map(|p| Destinatario::Pessoa {
                pessoa: p.slug.clone(),
            })
    });
    PerguntaDaPauta {
        referencia: format!("{}/{}", estado.id, item.id),
        fonte: FontePergunta::Ata {
            id: estado.id.to_string(),
            data: estado.data.to_string(),
        },
        texto: item.texto.clone(),
        contexto: None,
        destinatario,
        tema: match &estado.objetivo {
            Some(o) if !o.trim().is_empty() => format!("Reunião {}: {}", estado.id, o.trim()),
            _ => format!("Reunião {}", estado.id),
        },
        desde: estado.data.to_string(),
        idade_dias: idade(estado.data, hoje),
        bloqueia: Vec::new(),
        documento: amb.kb.na_base(ata).as_str().to_owned(),
        decisao: None,
    }
}

/// Perguntas pendentes do projeto (ou da demanda), uma por origem.
fn pendentes(
    amb: &Ambiente,
    demanda: Option<IdDemanda>,
    pessoas: &[PessoaLida],
    hoje: Data,
) -> Result<Vec<PerguntaDaPauta>, ErroPauta> {
    let analises = amb.analises.listar()?;
    let bloqueios = bloqueios(&analises);
    let mut saida = Vec::new();
    for a in &analises {
        if a.perguntas
            .iter()
            .all(|p| p.estado != EstadoPergunta::Aberta)
        {
            continue;
        }
        if demanda.is_some() && demanda_da_analise(amb, a) != demanda {
            continue;
        }
        for p in a
            .perguntas
            .iter()
            .filter(|p| p.estado == EstadoPergunta::Aberta)
        {
            saida.push(pergunta_da_analise(amb, a, p, &bloqueios, hoje));
        }
    }
    let reunioes = match demanda {
        Some(d) => amb.kb.reunioes_da_demanda(d)?,
        None => amb.kb.reunioes()?,
    };
    for arquivos in reunioes {
        let estado = match EstadoAta::ler(&amb.kb, arquivos.id) {
            Ok((estado, _)) => estado,
            Err(erro) => {
                tracing::warn!(reuniao = %arquivos.id, %erro, "ata ilegível ignorada na pauta");
                continue;
            }
        };
        if estado.status != StatusAta::Aprovada {
            continue;
        }
        for item in estado
            .itens_do_tipo(TipoItem::Duvida)
            .filter(|i| i.resposta.is_none())
        {
            saida.push(pergunta_da_ata(
                amb,
                &estado,
                item,
                &arquivos.ata,
                pessoas,
                hoje,
            ));
        }
    }
    Ok(saida)
}

/// Junta as perguntas iguais para o mesmo destinatário em itens da pauta.
fn agrupar_itens(mut perguntas: Vec<PerguntaDaPauta>) -> Vec<ItemDaPauta> {
    // As decisões primeiro (#452); em cada parte, da mais antiga para a mais
    // nova: o texto da pauta é o da mais antiga.
    perguntas.sort_by(|a, b| {
        b.decisao
            .is_some()
            .cmp(&a.decisao.is_some())
            .then_with(|| b.idade_dias.cmp(&a.idade_dias))
            .then_with(|| a.referencia.cmp(&b.referencia))
    });
    let mut itens: Vec<ItemDaPauta> = Vec::new();
    for p in perguntas {
        let igual = itens.iter_mut().find(|i| {
            i.destinatario == p.destinatario
                // Decisões só se juntam com as mesmas opções: a resposta é o
                // texto de uma delas, gravado em todas as perguntas do item.
                && i.decisao.as_ref().map(|d| &d.opcoes) == p.decisao.as_ref().map(|d| &d.opcoes)
                && mesma_pergunta(&i.texto, &p.texto)
        });
        match igual {
            Some(i) => i.perguntas.push(p),
            None => itens.push(ItemDaPauta {
                texto: p.texto.clone(),
                destinatario: p.destinatario.clone(),
                idade_dias: p.idade_dias,
                antiga: p.idade_dias >= DIAS_PARA_ANTIGA,
                decisao: p.decisao.clone(),
                perguntas: vec![p],
            }),
        }
    }
    itens
}

/// Monta a pauta de perguntas pendentes do projeto ou, com `demanda`, só da
/// demanda. Análises e atas ilegíveis ficam de fora (com aviso no log).
pub fn pauta(amb: &Ambiente, demanda: Option<IdDemanda>, hoje: Data) -> Result<Pauta, ErroPauta> {
    let titulo = match demanda {
        Some(d) => {
            let lida = amb.kb.demanda(d)?;
            format!("{d}: {}", lida.documento.frontmatter.titulo)
        }
        None => amb
            .kb
            .ler_projeto()
            .map(|p| p.frontmatter.nome)
            .unwrap_or_else(|_| amb.kb.nome().to_owned()),
    };
    let pessoas = amb.kb.pessoas().unwrap_or_default();
    let perguntas = pendentes(amb, demanda, &pessoas, hoje)?;
    let total = perguntas.len();
    let antigas = perguntas
        .iter()
        .filter(|p| p.idade_dias >= DIAS_PARA_ANTIGA)
        .count();

    let mut grupos: Vec<GrupoDaPauta> = Vec::new();
    for item in agrupar_itens(perguntas) {
        match grupos
            .iter_mut()
            .find(|g| g.destinatario == item.destinatario)
        {
            Some(g) => g.itens.push(item),
            None => grupos.push(GrupoDaPauta {
                rotulo: rotulo_do_destinatario(item.destinatario.as_ref(), &pessoas),
                destinatario: item.destinatario.clone(),
                itens: vec![item],
            }),
        }
    }
    grupos.sort_by(|a, b| {
        a.destinatario
            .is_none()
            .cmp(&b.destinatario.is_none())
            .then_with(|| normalizar_texto(&a.rotulo).cmp(&normalizar_texto(&b.rotulo)))
    });

    let premissas = match demanda {
        Some(d) => amb
            .kb
            .registros_da_demanda(d)?
            .iter()
            .filter(|r| r.id.categoria == Categoria::Premissa)
            .map(|r| PremissaDaPauta {
                id: r.id.to_string(),
                titulo: r.frontmatter().titulo.clone(),
                status: r.frontmatter().status.unwrap_or_default(),
            })
            .collect(),
        None => Vec::new(),
    };
    let (papeis, _) = cba_kb::Papeis::da_base(&amb.base);
    Ok(Pauta {
        titulo,
        grupos,
        total,
        antigas,
        pessoas: pessoas
            .iter()
            .map(|p| PessoaDaPauta {
                slug: p.slug.clone(),
                nome: p.nome().to_owned(),
                papel: p.papel_sugerido().map(|s| s.papel),
            })
            .collect(),
        papeis: papeis.papeis().iter().map(|p| p.nome.clone()).collect(),
        premissas,
        gerada_em: hoje.to_string(),
    })
}

// ---------------------------------------------------------------------------
// Destinatário

/// Perguntas por análise (número da análise → números das perguntas).
type PorAnalise = BTreeMap<u32, Vec<u32>>;
/// Dúvidas por ata.
type PorAta = BTreeMap<IdReuniao, Vec<IdItemAta>>;

/// Referências separadas por análise e por ata, sem repetição.
fn separar(referencias: &[String]) -> Result<(PorAnalise, PorAta), ErroPauta> {
    let mut analises: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    let mut atas: BTreeMap<IdReuniao, Vec<IdItemAta>> = BTreeMap::new();
    for r in referencias {
        match Referencia::analisar(r)? {
            Referencia::Analise(a, n) => {
                let v = analises.entry(a).or_default();
                if !v.contains(&n) {
                    v.push(n);
                }
            }
            Referencia::Ata(r, i) => {
                let v = atas.entry(r).or_default();
                if !v.contains(&i) {
                    v.push(i);
                }
            }
        }
    }
    Ok((analises, atas))
}

fn erro_da_ata(reuniao: IdReuniao, item: IdItemAta, e: cba_docs::ErroAta) -> ErroPauta {
    match e {
        cba_docs::ErroAta::DuvidaRespondida(_)
        | cba_docs::ErroAta::ItemInexistente(_)
        | cba_docs::ErroAta::NaoEDuvida(_) => ErroPauta::NaoPendente(format!("{reuniao}/{item}")),
        outro => ErroPauta::Ata(outro),
    }
}

fn erro_da_analise(numero: u32, e: analises::Erro) -> ErroPauta {
    match e {
        analises::Erro::Modelo(cba_core::analise::ErroModelo::ItemDecidido {
            numero: n, ..
        })
        | analises::Erro::Modelo(cba_core::analise::ErroModelo::ItemDesconhecido {
            numero: n,
            ..
        }) => ErroPauta::NaoPendente(format!("{}/{n}", Referencia::id_da_analise(numero))),
        outro => ErroPauta::Analise(outro),
    }
}

/// Define (ou tira, com `None`) o destinatário das perguntas em aberto.
pub fn definir_destinatario(
    amb: &Ambiente,
    referencias: &[String],
    destinatario: Option<Destinatario>,
) -> Result<(), ErroPauta> {
    let (analises, atas) = separar(referencias)?;
    let ids: Vec<Id> = analises
        .keys()
        .map(|n| Referencia::id_da_analise(*n))
        .collect();
    if !ids.is_empty() {
        amb.analises
            .alterar_varias(&ids, Mudanca::Pergunta, |a, _| {
                for n in &analises[&a.id.numero()] {
                    a.definir_destinatario(*n, destinatario.clone())?;
                }
                Ok(())
            })
            .map_err(|e| match ids.first() {
                Some(id) => erro_da_analise(id.numero(), e),
                None => ErroPauta::Analise(e),
            })?;
    }
    let (templates, _) = Templates::da_base(&amb.base);
    for (reuniao, itens) in atas {
        templates.alterar_ata(&amb.kb, reuniao, |estado| {
            for i in &itens {
                estado
                    .definir_destinatario(*i, destinatario.clone())
                    .map_err(|e| erro_da_ata(reuniao, *i, e))?;
            }
            Ok::<_, ErroPauta>(())
        })?;
        crate::reuniao::notificar_ata(amb, reuniao);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Resposta

/// O que fazer com a resposta, além de fechar as perguntas.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(tag = "tipo", rename_all = "kebab-case")]
pub enum DestinoDaResposta {
    /// Só registrar a resposta nas perguntas.
    #[default]
    SoRegistrar,
    /// Virar uma regra informada (sem evidência de código).
    Regra {
        /// Título da regra; sem ele, a primeira frase da resposta.
        #[serde(default)]
        titulo: Option<String>,
        /// Área da regra (id ou nome de uma área do `projeto.md`); sem ela, a
        /// única área do projeto ou `geral`.
        #[serde(default)]
        area: Option<String>,
    },
    /// Validar ou invalidar uma premissa da demanda (#122).
    Premissa {
        /// `PR-0001`.
        premissa: String,
        status: StatusPremissa,
    },
}

/// Resposta recebida, como o analista registra no app.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RespostaRecebida {
    pub texto: String,
    /// Nome de quem respondeu.
    pub por: String,
    /// Slug da pessoa no catálogo, se for uma pessoa cadastrada.
    #[serde(default)]
    pub pessoa: Option<String>,
    /// Data da resposta; sem ela, hoje.
    #[serde(default)]
    pub em: Option<Data>,
    /// De onde veio a resposta (reunião, e-mail, conversa).
    #[serde(default)]
    pub fonte: String,
    /// Perguntas que a resposta fecha (`A-0001/2`, `R-0001/DV-1`).
    pub perguntas: Vec<String>,
    #[serde(default)]
    pub destino: DestinoDaResposta,
}

/// Regra criada a partir da resposta.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegraCriada {
    /// `RN-FAT-001`.
    pub id: String,
    /// Relativo à raiz da base.
    pub documento: String,
}

/// O que o registro da resposta fez.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RespostaRegistrada {
    /// Perguntas fechadas.
    pub fechadas: Vec<String>,
    pub regra: Option<RegraCriada>,
    /// Premissa validada (ou invalidada).
    pub premissa: Option<String>,
    /// Problemas que não impediram o registro (índice não sincronizado).
    pub avisos: Vec<String>,
}

fn nao_vazio(texto: &str, campo: &'static str) -> Result<String, ErroPauta> {
    let t = texto.trim();
    if t.is_empty() {
        Err(ErroPauta::CampoVazio(campo))
    } else {
        Ok(t.to_owned())
    }
}

/// Confere, antes de gravar, que as dúvidas das atas existem e estão em aberto.
fn conferir_atas(
    amb: &Ambiente,
    atas: &BTreeMap<IdReuniao, Vec<IdItemAta>>,
) -> Result<(), ErroPauta> {
    for (reuniao, itens) in atas {
        let (estado, _) = EstadoAta::ler(&amb.kb, *reuniao)?;
        for i in itens {
            let pendente = estado
                .item(*i)
                .is_some_and(|item| item.tipo == TipoItem::Duvida && item.resposta.is_none());
            if !pendente {
                return Err(ErroPauta::NaoPendente(format!("{reuniao}/{i}")));
            }
        }
    }
    Ok(())
}

/// Confere, antes de gravar, que as perguntas das análises existem e estão em
/// aberto (a gravação confere de novo, sob a trava).
fn conferir_analises(amb: &Ambiente, analises: &BTreeMap<u32, Vec<u32>>) -> Result<(), ErroPauta> {
    for (a, numeros) in analises {
        let id = Referencia::id_da_analise(*a);
        let analise = amb.analises.carregar(&id)?;
        for n in numeros {
            let aberta = analise
                .perguntas
                .iter()
                .any(|p| p.numero == *n && p.estado == EstadoPergunta::Aberta);
            if !aberta {
                return Err(ErroPauta::NaoPendente(format!("{id}/{n}")));
            }
        }
    }
    Ok(())
}

/// Registra a resposta: fecha as perguntas e, conforme o destino, cria a regra
/// informada ou valida a premissa. Veja o módulo.
pub fn registrar_resposta(
    amb: &Ambiente,
    resposta: &RespostaRecebida,
    hoje: Data,
) -> Result<RespostaRegistrada, ErroPauta> {
    let texto = nao_vazio(&resposta.texto, "texto")?;
    let informacao = Informacao {
        por: nao_vazio(&resposta.por, "por")?,
        pessoa: resposta
            .pessoa
            .as_deref()
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .map(str::to_owned),
        em: resposta.em.unwrap_or(hoje),
        fonte: resposta.fonte.trim().to_owned(),
    };
    if resposta.perguntas.is_empty() {
        return Err(ErroPauta::SemPerguntas);
    }
    let (analises, atas) = separar(&resposta.perguntas)?;
    let referencias: Vec<String> = analises
        .iter()
        .flat_map(|(a, ns)| {
            ns.iter()
                .map(move |n| format!("{}/{n}", Referencia::id_da_analise(*a)))
        })
        .chain(
            atas.iter()
                .flat_map(|(r, is)| is.iter().map(move |i| format!("{r}/{i}"))),
        )
        .collect();

    // Tudo conferido antes de gravar qualquer coisa.
    conferir_analises(amb, &analises)?;
    conferir_atas(amb, &atas)?;
    let premissa = match &resposta.destino {
        DestinoDaResposta::Premissa { premissa, .. } => {
            let id = IdRegistro::analisar(premissa.trim())
                .map_err(|_| ErroPauta::NaoEPremissa(premissa.trim().to_owned()))?;
            if id.categoria != Categoria::Premissa {
                return Err(ErroPauta::NaoEPremissa(id.to_string()));
            }
            amb.kb.registro(id)?;
            Some(id)
        }
        _ => None,
    };
    let plano_regra = match &resposta.destino {
        DestinoDaResposta::Regra { titulo, area } => Some(planejar_regra(
            amb,
            &texto,
            titulo.as_deref(),
            area.as_deref(),
        )?),
        _ => None,
    };

    // Fecha as perguntas: as análises todas sob uma trava, cada ata sob a dela.
    let ids: Vec<Id> = analises
        .keys()
        .map(|n| Referencia::id_da_analise(*n))
        .collect();
    let mut origem: Option<CaminhoRelativo> = None;
    let mut textos: Vec<(String, String)> = Vec::new();
    if !ids.is_empty() {
        let gravadas = amb
            .analises
            .alterar_varias(&ids, Mudanca::Pergunta, |a, _| {
                for n in &analises[&a.id.numero()] {
                    a.responder_pergunta(*n, &texto, informacao.clone())?;
                }
                Ok(())
            })
            .map_err(|e| erro_da_analise(ids[0].numero(), e))?;
        for a in &gravadas {
            origem.get_or_insert_with(|| a.documento.clone());
            for n in &analises[&a.id.numero()] {
                if let Some(p) = a.perguntas.iter().find(|p| p.numero == *n) {
                    textos.push((format!("{}/{n}", a.id), p.texto.clone()));
                }
            }
        }
    }
    let (templates, _) = Templates::da_base(&amb.base);
    for (reuniao, itens) in &atas {
        let (_, estado, _) = templates.alterar_ata(&amb.kb, *reuniao, |estado| {
            for i in itens {
                estado
                    .responder_duvida(
                        *i,
                        RespostaInformada {
                            texto: texto.clone(),
                            informacao: informacao.clone(),
                        },
                    )
                    .map_err(|e| erro_da_ata(*reuniao, *i, e))?;
            }
            Ok::<_, ErroPauta>(())
        })?;
        crate::reuniao::notificar_ata(amb, *reuniao);
        if origem.is_none() {
            origem = amb.kb.reuniao(*reuniao).ok().map(|a| a.ata);
        }
        for i in itens {
            if let Some(item) = estado.item(*i) {
                textos.push((format!("{reuniao}/{i}"), item.texto.clone()));
            }
        }
    }

    let mut registrada = RespostaRegistrada {
        fechadas: referencias.clone(),
        regra: None,
        premissa: None,
        avisos: Vec::new(),
    };
    if let Some(plano) = plano_regra {
        let informada = RegraInformada {
            informacao: informacao.clone(),
            perguntas: referencias.clone(),
        };
        let (id, arquivo, avisos) = criar_regra(
            amb,
            &templates,
            plano,
            &texto,
            &textos,
            informada,
            origem.clone(),
        )?;
        registrada.avisos.extend(avisos);
        registrada.regra = Some(RegraCriada {
            id: id.to_string(),
            documento: amb.kb.na_base(&arquivo).as_str().to_owned(),
        });
        let sincronizado = Indice::abrir(amb.dados.arquivo_do_indice(), &amb.base)
            .and_then(|mut i| i.sincronizar());
        if let Err(e) = sincronizado {
            tracing::warn!("índice não sincronizado depois da regra informada");
            registrada.avisos.push(format!(
                "o índice da base não foi atualizado ({e}); ele se atualiza na próxima \
                 sincronização"
            ));
        }
    }
    if let (Some(id), DestinoDaResposta::Premissa { status, .. }) = (premissa, &resposta.destino) {
        let validacao = ValidacaoDePremissa {
            status: *status,
            como: Some(format!(
                "informado por {}: {}",
                informacao.descricao(),
                texto
            )),
            pergunta: Some(referencias.join(", ")),
        };
        crate::registros::validar_premissa(amb, id, &validacao, hoje).map_err(|e| match e {
            crate::registros::ErroRegistros::Base(b) => ErroPauta::Base(b),
            crate::registros::ErroRegistros::Analise(a) => ErroPauta::Analise(a),
            outro => ErroPauta::NaoEPremissa(outro.to_string()),
        })?;
        registrada.premissa = Some(id.to_string());
    }
    Ok(registrada)
}

/// Área, sigla e título da regra informada, conferidos antes de gravar.
struct RegraPlanejada {
    area: String,
    sigla: String,
    titulo: String,
}

fn planejar_regra(
    amb: &Ambiente,
    texto: &str,
    titulo: Option<&str>,
    area: Option<&str>,
) -> Result<RegraPlanejada, ErroPauta> {
    let areas = amb.kb.ler_projeto()?.frontmatter.areas;
    let area = area
        .map(str::trim)
        .filter(|a| !a.is_empty())
        .map(str::to_owned)
        .or_else(|| (areas.len() == 1).then(|| areas[0].id.clone()))
        .unwrap_or_else(|| AREA_GERAL.to_owned());
    let (area, sigla) = resolver_area(&areas, &area)?;
    let titulo = titulo
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| titulo_da_regra(texto));
    Ok(RegraPlanejada {
        area,
        sigla,
        titulo,
    })
}

/// Cria a regra informada pelo template de regra, com o próximo número da sigla.
fn criar_regra(
    amb: &Ambiente,
    templates: &Templates,
    plano: RegraPlanejada,
    resposta: &str,
    perguntas: &[(String, String)],
    informada: RegraInformada,
    origem: Option<CaminhoRelativo>,
) -> Result<(Id, CaminhoRelativo, Vec<String>), ErroPauta> {
    let mut enunciado = format!("{}\n\n{resposta}", informada.aviso());
    if !perguntas.is_empty() {
        enunciado.push_str("\n\nPerguntas que a resposta fechou:\n");
        for (r, t) in perguntas {
            enunciado.push_str(&format!("\n- {r}: {}", t.trim()));
        }
    }
    let frontmatter = |id: &Id| FrontmatterRegra {
        tipo: TipoDocumento::Regra,
        id: id.clone(),
        titulo: plano.titulo.clone(),
        area: plano.area.clone(),
        status: StatusRegra::Confirmada,
        evidencias: Vec::new(),
        origem: origem.clone(),
        informada: Some(informada.clone()),
        historico: Vec::new(),
        funcionalidades: Vec::new(),
    };
    let serie = Serie::regra(&plano.sigla).map_err(cba_kb::Erro::from)?;
    // Monta antes com o próximo número livre, para recusar (sem gravar) um
    // template que não monta.
    let provisorio = amb.kb.proximo_id(&serie)?;
    let modelo = templates.montar(&Conteudo::da_regra(
        frontmatter(&provisorio),
        enunciado.clone(),
    ))?;
    let avisos = modelo.avisos.iter().map(ToString::to_string).collect();
    let (id, arquivo) = amb.kb.criar_numerado(&serie, |id| {
        templates
            .montar(&Conteudo::da_regra(frontmatter(id), enunciado.clone()))
            .map(|m| m.documento)
            .unwrap_or_else(|_| {
                let mut doc = modelo.documento.clone();
                doc.frontmatter.id = id.clone();
                doc
            })
    })?;
    Ok((id, arquivo, avisos))
}

// ---------------------------------------------------------------------------
// Exportação

fn origem_curta(p: &PerguntaDaPauta) -> String {
    match &p.fonte {
        FontePergunta::Analise { titulo, .. } => format!("{} ({titulo})", p.referencia),
        FontePergunta::Ata { data, .. } => format!("{} (reunião de {data})", p.referencia),
    }
}

/// Uma linha, sem quebras.
fn linha(texto: &str) -> String {
    texto.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A pauta como documento de reunião, em Markdown: por destinatário, cada
/// pergunta com o que o sistema faz hoje, a origem, a idade, o que bloqueia e
/// espaço para a resposta.
pub fn markdown(pauta: &Pauta) -> String {
    let mut md = format!("# Pauta de perguntas: {}\n\n", linha(&pauta.titulo));
    md.push_str(&format!(
        "Gerada em {}. {} pergunta(s) em aberto, {} pendente(s) há {DIAS_PARA_ANTIGA} dias \
         ou mais.\n",
        pauta.gerada_em, pauta.total, pauta.antigas
    ));
    if pauta.grupos.is_empty() {
        md.push_str("\nNenhuma pergunta em aberto.\n");
        return md;
    }
    let mut numero = 0;
    for g in &pauta.grupos {
        md.push_str(&format!("\n## {}\n", linha(&g.rotulo)));
        for item in &g.itens {
            numero += 1;
            let rotulo = if item.decisao.is_some() {
                "Decisão: "
            } else {
                ""
            };
            md.push_str(&format!(
                "\n### {numero}. {rotulo}{}\n\n",
                linha(&item.texto)
            ));
            if let Some(d) = &item.decisao {
                for (i, o) in d.opcoes.iter().enumerate() {
                    md.push_str(&format!(
                        "- **Opção {}:** {} — efeito: {}\n",
                        i + 1,
                        linha(&o.texto),
                        linha(&o.efeito)
                    ));
                }
                if let Some(r) = &d.recomendacao {
                    md.push_str(&format!("- **Recomendação:** {}\n", linha(r)));
                }
            }
            let contextos: BTreeSet<String> = item
                .perguntas
                .iter()
                .filter_map(|p| p.contexto.as_deref())
                .map(linha)
                .collect();
            for c in contextos {
                md.push_str(&format!("- **O que o sistema faz hoje:** {c}\n"));
            }
            let origens: Vec<String> = item.perguntas.iter().map(origem_curta).collect();
            md.push_str(&format!("- **Origem:** {}\n", origens.join("; ")));
            let antiga = if item.antiga { " (antiga)" } else { "" };
            md.push_str(&format!(
                "- **Em aberto há:** {} dia(s){antiga}\n",
                item.idade_dias
            ));
            let bloqueia: BTreeSet<&String> =
                item.perguntas.iter().flat_map(|p| &p.bloqueia).collect();
            if !bloqueia.is_empty() {
                let lista: Vec<&str> = bloqueia.iter().map(|s| s.as_str()).collect();
                md.push_str(&format!("- **Bloqueia:** {}\n", lista.join(", ")));
            }
            md.push_str("- **Resposta:** \n");
        }
    }
    md
}

/// Texto de e-mail com as perguntas de um grupo, sem referências internas, para
/// copiar.
pub fn email(pauta: &Pauta, grupo: &GrupoDaPauta) -> String {
    let saudacao = match &grupo.destinatario {
        Some(Destinatario::Pessoa { .. }) => {
            let nome = grupo.rotulo.split(" (").next().unwrap_or(&grupo.rotulo);
            format!("Olá, {nome},")
        }
        _ => "Olá,".to_owned(),
    };
    let mut texto = format!(
        "{saudacao}\n\nPara seguir com {}, preciso confirmar alguns pontos:\n",
        linha(&pauta.titulo)
    );
    for (i, item) in grupo.itens.iter().enumerate() {
        texto.push_str(&format!("\n{}. {}", i + 1, linha(&item.texto)));
        if let Some(c) = item.perguntas.iter().find_map(|p| p.contexto.as_deref()) {
            texto.push_str(&format!("\n   Hoje o sistema: {}", linha(c)));
        }
        if let Some(d) = &item.decisao {
            for (j, o) in d.opcoes.iter().enumerate() {
                texto.push_str(&format!(
                    "\n   Opção {}: {} — efeito: {}",
                    j + 1,
                    linha(&o.texto),
                    linha(&o.efeito)
                ));
            }
            if let Some(r) = &d.recomendacao {
                texto.push_str(&format!("\n   Recomendação: {}", linha(r)));
            }
        }
    }
    texto.push_str("\n\nObrigado.\n");
    texto
}

/// Exporta a pauta como Markdown ou Word em `destino`.
pub fn exportar(
    pauta: &Pauta,
    destino: &std::path::Path,
    formato: Formato,
) -> Result<(), ErroPauta> {
    cba_docs::gravar_exportacao(
        destino,
        &markdown(pauta),
        formato,
        &OpcoesExportacao::default(),
        None,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn referencias_da_pauta() {
        assert_eq!(
            Referencia::analisar("A-0001/2").unwrap(),
            Referencia::Analise(1, 2)
        );
        assert!(matches!(
            Referencia::analisar(" R-0003/DV-1 ").unwrap(),
            Referencia::Ata(_, _)
        ));
        for ruim in ["A-0001", "R-0001/D-1", "S-0001/2", "x", "R-0001/DV-0"] {
            assert!(Referencia::analisar(ruim).is_err(), "{ruim}");
        }
    }

    #[test]
    fn perguntas_iguais_ou_quase() {
        assert!(mesma_pergunta(
            "O parcelamento vale para PJ?",
            "o parcelamento vale para pj"
        ));
        assert!(mesma_pergunta(
            "Qual é o limite de parcelas para pessoa jurídica?",
            "Qual o limite de parcelas para pessoa jurídica?"
        ));
        assert!(!mesma_pergunta(
            "Qual o limite de parcelas para PJ?",
            "Qual a multa por atraso da fatura?"
        ));
    }
}
