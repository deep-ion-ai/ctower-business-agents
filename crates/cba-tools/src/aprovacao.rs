//! Aprovação da análise: a análise aprovada alimenta a base (spec, seção 7.4).
//!
//! [`aprovar_analise`] (ou [`aprovar`], com a base e as análises já abertas) faz,
//! nesta ordem:
//!
//! 1. confere que a análise está concluída (ou já aprovada) e que os ajustes do
//!    analista ([`OpcoesAprovacao`]) citam afirmações confirmadas;
//! 2. aprova a análise (concluída → aprovada) e remonta o documento dela;
//! 3. transforma as afirmações **confirmadas** pelo analista em regras
//!    (`regras/RN-<SIGLA>-<NNN>.md`), com as evidências e a origem (o documento da
//!    análise), montadas pelo template de regra;
//! 4. transforma os termos **aceitos** em entradas do glossário
//!    (`glossario/<termo>.md`), pelo template de termo;
//! 5. em [`aprovar_analise`], sincroniza o índice da base (falha vira aviso).
//!
//! # Área e sigla
//!
//! A área de cada regra é, na ordem: a do ajuste da afirmação
//! ([`AjusteDaRegra::area`]), a da aprovação ([`OpcoesAprovacao::area`]), a única
//! área declarada no `sistema.md` (quando há só uma) e, sem nenhuma delas,
//! [`AREA_GERAL`]. A área informada é procurada nas áreas do `sistema.md` pelo id
//! ou pelo nome (sem diferença de acentos e maiúsculas); fora delas, vale o slug do
//! texto. A sigla do identificador é a da área no `sistema.md` e, se a área não
//! declarar sigla (ou não estiver declarada), as três primeiras letras ou dígitos
//! do slug, em maiúsculas (`geral` → `GER`). A numeração é por sigla, como em
//! [`cba_kb::Serie::regra`].
//!
//! # Atualizar em vez de duplicar
//!
//! - Uma regra é a mesma quando tem **o mesmo conjunto de evidências**, comparadas
//!   pelo arquivo e pelo hash do trecho (as linhas podem ter mudado de lugar). A
//!   regra existente mantém o identificador, a área e a origem; o título, o
//!   enunciado, as evidências e o status passam a ser os da análise aprovada.
//!   Comparar o conjunto inteiro, e não uma evidência em comum, evita juntar duas
//!   regras diferentes que citam o mesmo trecho: na dúvida, a base ganha uma regra a
//!   mais, mas não perde uma.
//! - Afirmação confirmada sem evidência vira regra sem evidência; ela é a mesma
//!   regra quando tem a mesma origem e o mesmo título.
//! - Afirmações da mesma análise com a mesma chave viram uma regra só, com os
//!   enunciados em parágrafos.
//! - Um termo é o mesmo quando tem o mesmo nome (o mesmo arquivo,
//!   `glossario/<slug>.md`). O termo existente mantém o nome e a origem, soma os
//!   sinônimos e recebe a definição, onde vive e as evidências do termo aceito.
//! - Campos desconhecidos do frontmatter de um documento existente são mantidos.
//!   O corpo é remontado pelo template.
//! - Documento que ficaria igual não é gravado de novo ([`Acao::SemMudanca`]).
//!
//! Por isso a aprovação é idempotente: aprovar de novo uma análise já aprovada não
//! muda o estado e não duplica nada, e serve para completar uma aprovação que
//! falhou no meio ou para levar à base uma decisão tomada depois (um termo aceito
//! depois da aprovação, por exemplo).
//!
//! O status da regra é `confirmada`, ou `a-revalidar` se alguma evidência da
//! afirmação já estiver marcada como a revalidar.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::sync::Arc;

use cba_core::analise::{
    Afirmacao, Analise, EstadoAfirmacao, EstadoAnalise, EstadoEvidencia, EstadoTermo, TermoProposto,
};
use cba_core::analises::{self, Analises};
use cba_core::config::Config;
use cba_core::dados::DiretorioDados;
use cba_core::eventos::{DiarioEventos, Mudanca};
use cba_docs::{Conteudo, Montavel, Templates, Texto};
use cba_kb::frontmatter::{
    Area, Evidencia, FrontmatterRegra, FrontmatterTermo, StatusRegra, TipoDocumento,
};
use cba_kb::indice::Indice;
use cba_kb::{Base, CaminhoRelativo, Documento, Id, Serie, layout, slug};
use serde::{Deserialize, Serialize};

use crate::documento;

/// Área das regras quando nem o analista nem o `sistema.md` indicam uma.
pub const AREA_GERAL: &str = "geral";

/// Tamanho máximo, em caracteres, do título derivado do texto da afirmação.
pub const LIMITE_DO_TITULO: usize = 80;

/// Escolhas do analista na aprovação (a tela de revisão, #37).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpcoesAprovacao {
    /// Área das regras desta análise: id ou nome de uma área do `sistema.md`, ou
    /// uma área nova (vale o slug).
    #[serde(default)]
    pub area: Option<String>,
    /// Ajustes por afirmação confirmada, pelo número da afirmação.
    #[serde(default)]
    pub afirmacoes: BTreeMap<u32, AjusteDaRegra>,
}

/// Ajuste do analista na regra que uma afirmação confirmada vai virar.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AjusteDaRegra {
    /// Área da regra, no lugar de [`OpcoesAprovacao::area`].
    #[serde(default)]
    pub area: Option<String>,
    /// Título da regra, no lugar do derivado do texto ([`titulo_da_regra`]).
    #[serde(default)]
    pub titulo: Option<String>,
}

/// O que a aprovação fez com um documento da base.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Acao {
    Criada,
    Atualizada,
    /// O documento já estava como ficaria; nada foi gravado.
    SemMudanca,
}

/// Regra gravada (ou conferida) pela aprovação.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegraAprovada {
    pub id: Id,
    /// Arquivo, relativo à raiz do sistema na base (`regras/RN-FAT-001.md`).
    pub arquivo: CaminhoRelativo,
    pub area: String,
    /// Afirmações da análise que a regra reúne.
    pub afirmacoes: Vec<u32>,
    pub acao: Acao,
}

/// Termo do glossário gravado (ou conferido) pela aprovação.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TermoAprovado {
    /// Nome do termo no glossário.
    pub termo: String,
    /// Número do termo proposto na análise.
    pub numero: u32,
    /// Arquivo, relativo à raiz do sistema na base (`glossario/multa.md`).
    pub arquivo: CaminhoRelativo,
    pub acao: Acao,
}

/// Resultado da aprovação.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Aprovacao {
    /// A análise aprovada, como gravada.
    pub analise: Analise,
    /// A análise já estava aprovada (a aprovação só conferiu a base).
    pub ja_aprovada: bool,
    pub regras: Vec<RegraAprovada>,
    pub termos: Vec<TermoAprovado>,
    /// Problemas que não impediram a aprovação (template com marcador
    /// desconhecido, documento da análise não remontado, índice não sincronizado).
    pub avisos: Vec<String>,
}

/// Por que a aprovação foi recusada ou não terminou. Recusas (análise não
/// concluída, ajuste inválido) acontecem antes de qualquer gravação; uma falha ao
/// gravar na base deixa a análise aprovada, e chamar a aprovação de novo completa
/// o que faltou.
#[derive(Debug, thiserror::Error)]
pub enum ErroAprovacao {
    #[error(transparent)]
    Analise(#[from] analises::Erro),
    #[error(
        "a análise {id} está {estado} e não pode ser aprovada; o agente precisa \
         concluí-la antes"
    )]
    NaoConcluida { id: Id, estado: &'static str },
    #[error(
        "há um ajuste para a afirmação {0}, mas ela não está confirmada pelo analista \
         nesta análise; só afirmações confirmadas viram regras"
    )]
    AjusteSemAfirmacao(u32),
    #[error("a área `{0}` não tem letras nem dígitos; informe o id ou o nome de uma área")]
    AreaInvalida(String),
    #[error(transparent)]
    Base(#[from] cba_kb::Erro),
    #[error(transparent)]
    Montagem(#[from] cba_docs::ErroMontagem),
    #[error("a pasta da base de conhecimento ainda não foi escolhida")]
    BaseNaoConfigurada,
    #[error("não foi possível ler a configuração do app: {0}")]
    Config(std::io::Error),
}

fn nome_do_estado(estado: EstadoAnalise) -> &'static str {
    match estado {
        EstadoAnalise::Criada => "criada",
        EstadoAnalise::EmAndamento => "em andamento",
        EstadoAnalise::Concluida => "concluída",
        EstadoAnalise::Aprovada => "aprovada",
    }
}

/// Aprova a análise `id` do sistema `sistema` (slug) e leva à base as regras e os
/// termos, com a base da configuração do app e o aviso ao app pelo diário de
/// eventos. Depois, sincroniza o índice da base. Veja o módulo.
pub fn aprovar_analise(
    dados: &DiretorioDados,
    sistema: &str,
    id: &Id,
    opcoes: &OpcoesAprovacao,
) -> Result<Aprovacao, ErroAprovacao> {
    let config = Config::ler(dados).map_err(ErroAprovacao::Config)?;
    let raiz = config.base().ok_or(ErroAprovacao::BaseNaoConfigurada)?;
    let base = Base::abrir(raiz)?;
    let kb = base.sistema(sistema)?;
    let analises = Analises::new(kb, Arc::new(DiarioEventos::em(dados.eventos())));
    let mut aprovacao = aprovar(&base, &analises, id, opcoes)?;
    let sincronizado =
        Indice::abrir(dados.arquivo_do_indice(), &base).and_then(|mut i| i.sincronizar());
    if let Err(e) = sincronizado {
        tracing::warn!("índice não sincronizado depois da aprovação");
        aprovacao.avisos.push(format!(
            "o índice da base não foi atualizado ({e}); ele se atualiza na próxima \
             sincronização"
        ));
    }
    Ok(aprovacao)
}

/// Como [`aprovar_analise`], com a base e as análises do sistema já abertas, e
/// sem sincronizar o índice.
pub fn aprovar(
    base: &Base,
    analises: &Analises,
    id: &Id,
    opcoes: &OpcoesAprovacao,
) -> Result<Aprovacao, ErroAprovacao> {
    let kb = analises.sistema();
    let atual = analises.carregar(id)?;
    if !matches!(
        atual.estado,
        EstadoAnalise::Concluida | EstadoAnalise::Aprovada
    ) {
        return Err(ErroAprovacao::NaoConcluida {
            id: id.clone(),
            estado: nome_do_estado(atual.estado),
        });
    }
    let areas = kb.ler_sistema()?.frontmatter.areas;
    // Confere os ajustes antes de gravar qualquer coisa.
    planejar_regras(&atual, &areas, opcoes)?;

    let mut avisos = Vec::new();
    let ja_aprovada = atual.estado == EstadoAnalise::Aprovada;
    let analise = if ja_aprovada {
        atual
    } else {
        let mut aviso = None;
        let (_, analise) = analises.alterar_e(
            id,
            Mudanca::Estado,
            |a, _| a.aprovar(),
            |a| {
                if let Err(e) = documento::remontar(base, kb, a) {
                    tracing::warn!("documento da análise aprovada não remontado");
                    aviso = Some(format!(
                        "a análise foi aprovada, mas o documento dela não foi remontado ({e})"
                    ));
                }
            },
        )?;
        avisos.extend(aviso);
        analise
    };

    let (templates, avisos_templates) = Templates::da_base(base);
    avisos.extend(avisos_templates.iter().map(ToString::to_string));
    let mut gravacao = Gravacao {
        kb,
        templates: &templates,
        avisos,
    };
    let origem = analise.documento.clone();
    let plano = planejar_regras(&analise, &areas, opcoes)?;
    let regras = gravacao.regras(plano, &origem)?;
    let termos = gravacao.termos(&analise, &areas, opcoes, &origem)?;
    Ok(Aprovacao {
        analise,
        ja_aprovada,
        regras,
        termos,
        avisos: gravacao.avisos,
    })
}

// ---------------------------------------------------------------------------
// Plano das regras

/// Identidade de uma regra, para atualizar em vez de duplicar.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Chave {
    /// Conjunto de (arquivo, hash) das evidências.
    Evidencias(BTreeSet<(String, String)>),
    /// Regra sem evidência: o slug do título (e a origem, na comparação).
    Titulo(String),
}

impl Chave {
    fn de(evidencias: &[Evidencia], titulo: &str) -> Self {
        if evidencias.is_empty() {
            Self::Titulo(slug(titulo))
        } else {
            Self::Evidencias(
                evidencias
                    .iter()
                    .map(|e| (e.arquivo.as_str().to_owned(), e.hash.clone()))
                    .collect(),
            )
        }
    }

    /// Se a regra existente `f` é esta.
    fn corresponde(&self, f: &FrontmatterRegra, origem: &CaminhoRelativo) -> bool {
        match self {
            Self::Evidencias(_) => Self::de(&f.evidencias, &f.titulo) == *self,
            Self::Titulo(t) => {
                f.evidencias.is_empty()
                    && f.origem.as_ref() == Some(origem)
                    && slug(&f.titulo) == *t
            }
        }
    }
}

/// Regra que a aprovação vai criar ou atualizar.
#[derive(Debug, Clone)]
struct RegraPlanejada {
    chave: Chave,
    area: String,
    sigla: String,
    titulo: String,
    enunciados: Vec<String>,
    evidencias: Vec<Evidencia>,
    a_revalidar: bool,
    afirmacoes: Vec<u32>,
}

/// Área e sigla a partir do texto informado e das áreas do `sistema.md`.
fn resolver_area(areas: &[Area], texto: &str) -> Result<(String, String), ErroAprovacao> {
    let chave = slug(texto);
    if chave.is_empty() {
        return Err(ErroAprovacao::AreaInvalida(texto.to_owned()));
    }
    let declarada = areas
        .iter()
        .find(|a| a.id == chave || slug(&a.nome) == chave);
    Ok(match declarada {
        Some(a) => (
            a.id.clone(),
            a.sigla.clone().unwrap_or_else(|| sigla_do_slug(&a.id)),
        ),
        None => {
            let sigla = sigla_do_slug(&chave);
            (chave, sigla)
        }
    })
}

/// Sigla derivada do slug da área: as três primeiras letras ou dígitos, em
/// maiúsculas (`cobranca` → `COB`).
pub fn sigla_do_slug(slug: &str) -> String {
    slug.chars()
        .filter(char::is_ascii_alphanumeric)
        .take(3)
        .collect::<String>()
        .to_ascii_uppercase()
}

/// Título da regra derivado do texto da afirmação: a primeira frase, sem o ponto
/// final, cortada em [`LIMITE_DO_TITULO`] caracteres numa fronteira de palavra.
pub fn titulo_da_regra(texto: &str) -> String {
    let linha = texto
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("");
    let frase = linha.find(". ").map_or(linha, |i| &linha[..i]);
    let frase = frase.trim().trim_end_matches(['.', ':', ';']).trim_end();
    if frase.chars().count() <= LIMITE_DO_TITULO {
        return frase.to_owned();
    }
    let corte: String = frase.chars().take(LIMITE_DO_TITULO).collect();
    let corte = match corte.rfind(' ') {
        Some(i) if i > LIMITE_DO_TITULO / 2 => &corte[..i],
        _ => corte.as_str(),
    };
    format!("{}…", corte.trim_end_matches([',', ';', ':', ' ']))
}

fn evidencia_da_regra(e: &cba_core::analise::Evidencia) -> Evidencia {
    Evidencia {
        arquivo: e.arquivo.clone(),
        linhas: e.linhas,
        hash: e.hash.clone(),
        commit: e.commit.clone(),
    }
}

fn confirmadas(analise: &Analise) -> impl Iterator<Item = &Afirmacao> {
    analise
        .afirmacoes
        .iter()
        .filter(|a| a.estado == EstadoAfirmacao::Confirmada)
}

/// Texto informado pelo analista, sem espaços nas pontas, se não for vazio.
fn nao_vazio(texto: &Option<String>) -> Option<&str> {
    texto.as_deref().map(str::trim).filter(|t| !t.is_empty())
}

/// Regras a gravar, na ordem das afirmações, já agrupadas pela chave.
fn planejar_regras(
    analise: &Analise,
    areas: &[Area],
    opcoes: &OpcoesAprovacao,
) -> Result<Vec<RegraPlanejada>, ErroAprovacao> {
    if let Some(n) = opcoes
        .afirmacoes
        .keys()
        .find(|n| !confirmadas(analise).any(|a| a.numero == **n))
    {
        return Err(ErroAprovacao::AjusteSemAfirmacao(*n));
    }
    let area_padrao = match nao_vazio(&opcoes.area) {
        Some(a) => resolver_area(areas, a)?,
        None => match areas {
            [unica] => resolver_area(areas, &unica.id)?,
            _ => resolver_area(areas, AREA_GERAL)?,
        },
    };

    let mut plano: Vec<RegraPlanejada> = Vec::new();
    for a in confirmadas(analise) {
        let ajuste = opcoes.afirmacoes.get(&a.numero);
        let (area, sigla) = match ajuste.and_then(|j| nao_vazio(&j.area)) {
            Some(texto) => resolver_area(areas, texto)?,
            None => area_padrao.clone(),
        };
        let titulo = ajuste
            .and_then(|j| nao_vazio(&j.titulo))
            .map_or_else(|| titulo_da_regra(&a.texto), str::to_owned);
        let mut evidencias: Vec<Evidencia> = Vec::new();
        for e in a.evidencias.iter().map(evidencia_da_regra) {
            if !evidencias.contains(&e) {
                evidencias.push(e);
            }
        }
        let a_revalidar = a
            .evidencias
            .iter()
            .any(|e| e.estado == EstadoEvidencia::ARevalidar);
        let chave = Chave::de(&evidencias, &titulo);
        match plano.iter_mut().find(|p| p.chave == chave) {
            Some(p) => {
                p.enunciados.push(a.texto.clone());
                p.afirmacoes.push(a.numero);
                p.a_revalidar |= a_revalidar;
            }
            None => plano.push(RegraPlanejada {
                chave,
                area,
                sigla,
                titulo,
                enunciados: vec![a.texto.clone()],
                evidencias,
                a_revalidar,
                afirmacoes: vec![a.numero],
            }),
        }
    }
    Ok(plano)
}

// ---------------------------------------------------------------------------
// Gravação

struct Gravacao<'a> {
    kb: &'a cba_kb::Sistema,
    templates: &'a Templates,
    avisos: Vec<String>,
}

impl Gravacao<'_> {
    /// Monta o documento pelo template, guardando os avisos.
    fn montar<F: Montavel>(
        &mut self,
        conteudo: &Conteudo<F>,
    ) -> Result<Documento<F>, ErroAprovacao> {
        let montado = self.templates.montar(conteudo)?;
        self.avisos
            .extend(montado.avisos.iter().map(ToString::to_string));
        Ok(montado.documento)
    }

    /// Grava `novo` sobre o documento existente em `caminho`, mantendo os campos
    /// desconhecidos do frontmatter, só se o texto mudar.
    fn atualizar<F: Montavel>(
        &self,
        caminho: &CaminhoRelativo,
        mut existente: Documento<F>,
        novo: Documento<F>,
    ) -> Result<Acao, ErroAprovacao> {
        existente.frontmatter = novo.frontmatter;
        existente.corpo = novo.corpo;
        let texto = existente
            .para_texto()
            .map_err(|erro| cba_kb::Erro::Frontmatter {
                arquivo: self.kb.na_base(caminho),
                erro,
            })?;
        let antes = fs::read_to_string(self.kb.caminho(caminho)).unwrap_or_default();
        if antes == texto {
            return Ok(Acao::SemMudanca);
        }
        self.kb.gravar(caminho, &existente)?;
        Ok(Acao::Atualizada)
    }

    /// Regras existentes no sistema. Documentos ilegíveis ficam de fora, com aviso:
    /// não são comparados e nunca são sobrescritos (a regra nova ganha outro número).
    fn regras_existentes(
        &mut self,
    ) -> Result<Vec<(CaminhoRelativo, Documento<FrontmatterRegra>)>, ErroAprovacao> {
        let mut existentes = Vec::new();
        for caminho in self.kb.documentos()? {
            if caminho.partes().next() != Some(layout::REGRAS) {
                continue;
            }
            match self.kb.ler::<FrontmatterRegra>(&caminho) {
                Ok(doc) => existentes.push((caminho, doc)),
                Err(e) => {
                    tracing::warn!("regra ilegível ignorada na aprovação");
                    self.avisos
                        .push(format!("regra ignorada na comparação: {e}"));
                }
            }
        }
        Ok(existentes)
    }

    fn regras(
        &mut self,
        plano: Vec<RegraPlanejada>,
        origem: &CaminhoRelativo,
    ) -> Result<Vec<RegraAprovada>, ErroAprovacao> {
        let mut existentes = self.regras_existentes()?;
        let mut saida = Vec::new();
        for p in plano {
            let status = if p.a_revalidar {
                StatusRegra::ARevalidar
            } else {
                StatusRegra::Confirmada
            };
            let enunciado = p.enunciados.join("\n\n");
            let encontrada = existentes
                .iter()
                .position(|(_, d)| p.chave.corresponde(&d.frontmatter, origem));
            if let Some(i) = encontrada {
                let (caminho, existente) = existentes[i].clone();
                let f = &existente.frontmatter;
                let frontmatter = FrontmatterRegra {
                    tipo: TipoDocumento::Regra,
                    id: f.id.clone(),
                    titulo: p.titulo,
                    area: f.area.clone(),
                    status,
                    evidencias: p.evidencias,
                    origem: f.origem.clone().or_else(|| Some(origem.clone())),
                };
                let (id, area) = (f.id.clone(), f.area.clone());
                let mut conteudo = Conteudo::da_regra(frontmatter, enunciado);
                conteudo.caminho = caminho.clone();
                let novo = self.montar(&conteudo)?;
                let acao = self.atualizar(&caminho, existente, novo.clone())?;
                existentes[i].1 = novo;
                saida.push(RegraAprovada {
                    id,
                    arquivo: caminho,
                    area,
                    afirmacoes: p.afirmacoes,
                    acao,
                });
                continue;
            }

            let serie = Serie::regra(&p.sigla).map_err(cba_kb::Erro::from)?;
            let frontmatter = |id: &Id| FrontmatterRegra {
                tipo: TipoDocumento::Regra,
                id: id.clone(),
                titulo: p.titulo.clone(),
                area: p.area.clone(),
                status,
                evidencias: p.evidencias.clone(),
                origem: Some(origem.clone()),
            };
            // Monta antes com o próximo número livre, para recusar (sem gravar) um
            // template que não monta; a reserva sem colisão monta de novo.
            let provisorio = self.kb.proximo_id(&serie)?;
            let modelo = self.montar(&Conteudo::da_regra(
                frontmatter(&provisorio),
                enunciado.clone(),
            ))?;
            let templates = self.templates;
            let (id, caminho) = self.kb.criar_numerado(&serie, |id| {
                templates
                    .montar(&Conteudo::da_regra(frontmatter(id), enunciado.clone()))
                    .map(|m| m.documento)
                    .unwrap_or_else(|_| {
                        let mut doc = modelo.clone();
                        doc.frontmatter.id = id.clone();
                        doc
                    })
            })?;
            existentes.push((caminho.clone(), self.kb.ler(&caminho)?));
            saida.push(RegraAprovada {
                id,
                arquivo: caminho,
                area: p.area,
                afirmacoes: p.afirmacoes,
                acao: Acao::Criada,
            });
        }
        Ok(saida)
    }

    fn termos(
        &mut self,
        analise: &Analise,
        areas: &[Area],
        opcoes: &OpcoesAprovacao,
        origem: &CaminhoRelativo,
    ) -> Result<Vec<TermoAprovado>, ErroAprovacao> {
        let mut saida = Vec::new();
        for t in analise
            .termos
            .iter()
            .filter(|t| t.estado == EstadoTermo::Aceito)
        {
            saida.push(self.termo(t, areas, opcoes, origem)?);
        }
        Ok(saida)
    }

    fn termo(
        &mut self,
        t: &TermoProposto,
        areas: &[Area],
        opcoes: &OpcoesAprovacao,
        origem: &CaminhoRelativo,
    ) -> Result<TermoAprovado, ErroAprovacao> {
        let caminho = layout::termo(&t.termo).map_err(cba_kb::Erro::from)?;
        // A área do termo é opcional: texto sem letras nem dígitos fica sem área.
        let area = t
            .area
            .as_deref()
            .or(opcoes.area.as_deref())
            .and_then(|a| resolver_area(areas, a).ok())
            .map(|(id, _)| id);
        let existente: Option<Documento<FrontmatterTermo>> = if self.kb.existe(&caminho) {
            Some(self.kb.ler(&caminho)?)
        } else {
            None
        };
        let anterior = existente.as_ref().map(|d| &d.frontmatter);
        let nome = anterior.map_or_else(|| t.termo.clone(), |f| f.termo.clone());
        let mut sinonimos: Vec<String> = Vec::new();
        let mut vistos = BTreeSet::from([slug(&nome)]);
        for s in anterior
            .map(|f| f.sinonimos.as_slice())
            .unwrap_or_default()
            .iter()
            .chain(&t.sinonimos)
        {
            if vistos.insert(slug(s)) {
                sinonimos.push(s.clone());
            }
        }
        let frontmatter = FrontmatterTermo {
            tipo: TipoDocumento::Termo,
            termo: nome.clone(),
            sinonimos,
            area: area.or_else(|| anterior.and_then(|f| f.area.clone())),
            origem: anterior
                .and_then(|f| f.origem.clone())
                .or_else(|| Some(origem.clone())),
        };
        let mut conteudo = Conteudo::novo(frontmatter, caminho.clone());
        conteudo.textos.push(Texto {
            secao: "definicao".into(),
            texto: t.definicao.clone(),
            citacoes: t.evidencias.iter().map(documento::citacao).collect(),
        });
        if !t.onde_vive.is_empty() {
            let lista: Vec<String> = t.onde_vive.iter().map(|o| format!("- {o}")).collect();
            conteudo
                .textos
                .push(Texto::novo("onde_vive_no_codigo", lista.join("\n")));
        }
        let novo = self.montar(&conteudo)?;
        let acao = match existente {
            Some(doc) => self.atualizar(&caminho, doc, novo)?,
            None => {
                self.kb.criar(&caminho, &novo)?;
                Acao::Criada
            }
        };
        Ok(TermoAprovado {
            termo: nome,
            numero: t.numero,
            arquivo: caminho,
            acao,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(id: &str, nome: &str, sigla: Option<&str>) -> Area {
        Area {
            id: id.into(),
            nome: nome.into(),
            sigla: sigla.map(Into::into),
        }
    }

    #[test]
    fn titulo_derivado_da_primeira_frase() {
        assert_eq!(
            titulo_da_regra("A multa é de 2%. Ela vem de parâmetro."),
            "A multa é de 2%"
        );
        assert_eq!(
            titulo_da_regra("\n  Carência de 3 dias.\n"),
            "Carência de 3 dias"
        );
        let longo = "Passada a carência, a fatura em atraso recebe multa única de \
                     PERC_MULTA_ATRASO por cento sobre o valor original";
        let t = titulo_da_regra(longo);
        assert!(t.ends_with('…'), "{t}");
        assert!(t.chars().count() <= LIMITE_DO_TITULO + 1, "{t}");
        assert!(longo.starts_with(t.trim_end_matches('…')), "{t}");
    }

    #[test]
    fn area_e_sigla() {
        let areas = [
            area("cobranca", "Cobrança", Some("FAT")),
            area("parcelamento", "Parcelamento", None),
        ];
        let r = |t: &str| resolver_area(&areas, t).unwrap();
        assert_eq!(r("cobranca"), ("cobranca".into(), "FAT".into()));
        assert_eq!(r("Cobrança"), ("cobranca".into(), "FAT".into()));
        assert_eq!(r("parcelamento"), ("parcelamento".into(), "PAR".into()));
        assert_eq!(r("Nota Fiscal"), ("nota-fiscal".into(), "NOT".into()));
        assert_eq!(r(AREA_GERAL), ("geral".into(), "GER".into()));
        assert_eq!(r("TI"), ("ti".into(), "TI".into()));
        assert!(matches!(
            resolver_area(&areas, " ?? "),
            Err(ErroAprovacao::AreaInvalida(_))
        ));
        assert_eq!(sigla_do_slug("nota-fiscal"), "NOT");
    }
}
