//! Aprovação da análise: a análise aprovada alimenta a base (spec, seção 7.4).
//!
//! [`aprovar_analise`] (ou [`aprovar`], com a base e as análises já abertas) faz,
//! nesta ordem:
//!
//! 1. confere que a análise está concluída (ou já aprovada) e que os ajustes do
//!    analista ([`OpcoesAprovacao`]) citam afirmações confirmadas;
//! 2. aprova a análise (concluída → aprovada) e remonta o documento dela;
//! 3. transforma as afirmações **confirmadas** pelo analista e escolhidas para
//!    virar regra ([`OpcoesAprovacao::regras`]; sem a escolha, as de fora da
//!    resposta e dos exemplos) em regras
//!    (`regras/RN-<SIGLA>-<NNN>.md`), com as evidências e a origem (o documento da
//!    análise), montadas pelo template de regra;
//! 4. transforma os termos **aceitos** em entradas do glossário
//!    (`glossario/<termo>.md`), pelo template de termo;
//! 5. em [`aprovar_analise`], sincroniza o índice da base (falha vira aviso).
//!
//! # Funcionalidades do mapa (#287)
//!
//! A análise aprovada guarda no frontmatter as funcionalidades do mapa a que se
//! refere: as escolhidas na janela de aprovação ([`OpcoesAprovacao::funcionalidades`])
//! ou, sem a escolha, as propostas ([`crate::vinculo::propostas`]). As regras e os
//! termos herdam: o documento criado ou vindo desta análise fica com a lista dela;
//! o que veio de outra análise soma as duas.
//!
//! # Área e sigla
//!
//! A área de cada regra é, na ordem: a do ajuste da afirmação
//! ([`AjusteDaRegra::area`]), a da aprovação ([`OpcoesAprovacao::area`]), a da
//! primeira funcionalidade vinculada à análise (#287), a única área declarada no
//! `projeto.md` (quando há só uma) e, sem nenhuma delas, [`AREA_GERAL`]. A área informada é procurada nas áreas do `projeto.md` pelo id
//! ou pelo nome (sem diferença de acentos e maiúsculas); fora delas, vale o slug do
//! texto. A sigla do identificador é a da área no `projeto.md` e, se a área não
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
use cba_core::funcionalidades::MapaFuncionalidades;
use cba_docs::{Conteudo, Montavel, Templates, Texto};
use cba_kb::frontmatter::{
    Area, Evidencia, FrontmatterRegra, FrontmatterTermo, StatusRegra, TipoDocumento,
};
use cba_kb::indice::Indice;
use cba_kb::{Base, CaminhoRelativo, Documento, Id, IdFuncionalidade, Serie, layout, slug};
use serde::{Deserialize, Serialize};

use crate::{documento, vinculo};

/// Área das regras quando nem o analista nem o `projeto.md` indicam uma.
pub const AREA_GERAL: &str = "geral";

/// Campo do frontmatter com as funcionalidades do mapa (#287).
const CAMPO_FUNCIONALIDADES: &str = "funcionalidades";

/// Tamanho máximo, em caracteres, do título derivado do texto da afirmação.
pub const LIMITE_DO_TITULO: usize = 80;

/// Escolhas do analista na aprovação (a tela de revisão, #37).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpcoesAprovacao {
    /// Área das regras desta análise: id ou nome de uma área do `projeto.md`, ou
    /// uma área nova (vale o slug).
    #[serde(default)]
    pub area: Option<String>,
    /// Ajustes por afirmação confirmada, pelo número da afirmação.
    #[serde(default)]
    pub afirmacoes: BTreeMap<u32, AjusteDaRegra>,
    /// Afirmações confirmadas que viram regra, pelo número. Sem a escolha
    /// (`None`), viram regra as confirmadas fora das seções de resposta e de
    /// exemplo ([`vira_regra_por_padrao`]).
    #[serde(default)]
    pub regras: Option<BTreeSet<u32>>,
    /// Funcionalidades do mapa a que a análise se refere, confirmadas ou trocadas
    /// pelo analista (#287). As regras e os termos herdam. Sem a escolha
    /// (`None`), valem as propostas ([`vinculo::propostas`]): as já gravadas no
    /// documento, a da sessão ou as sugeridas pelas evidências.
    #[serde(default)]
    pub funcionalidades: Option<Vec<IdFuncionalidade>>,
}

/// Seções cujas afirmações não viram regra sem a escolha do analista: a resposta
/// direta à pergunta e os exemplos montados sobre as regras.
pub const SECOES_SEM_REGRA: &[&str] = &["resposta", "exemplo"];

/// Se a afirmação da seção `secao` vira regra quando o analista não escolhe.
pub fn vira_regra_por_padrao(secao: &str) -> bool {
    !SECOES_SEM_REGRA.contains(&secao.trim())
}

/// Como [`vira_regra_por_padrao`], no fluxo da análise: no manual do usuário
/// (#272) o texto é para o usuário final, e nada vira regra sem a escolha do
/// analista.
pub fn vira_regra_por_padrao_no(fluxo: cba_core::analise::Fluxo, secao: &str) -> bool {
    fluxo != cba_core::analise::Fluxo::Manual && vira_regra_por_padrao(secao)
}

impl OpcoesAprovacao {
    /// Se a afirmação confirmada `a` vira regra com estas escolhas.
    pub fn vira_regra(&self, a: &Afirmacao) -> bool {
        match &self.regras {
            Some(escolhidas) => escolhidas.contains(&a.numero),
            None => vira_regra_por_padrao(&a.secao),
        }
    }

    /// Como [`Self::vira_regra`], no fluxo da análise
    /// ([`vira_regra_por_padrao_no`]).
    pub fn vira_regra_no(&self, fluxo: cba_core::analise::Fluxo, a: &Afirmacao) -> bool {
        match &self.regras {
            Some(escolhidas) => escolhidas.contains(&a.numero),
            None => vira_regra_por_padrao_no(fluxo, &a.secao),
        }
    }
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
    /// Arquivo, relativo à raiz do projeto na base (`regras/RN-FAT-001.md`).
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
    /// Arquivo, relativo à raiz do projeto na base (`glossario/multa.md`).
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
    /// Numa explicação ligada a uma funcionalidade do mapa (#208): o documento
    /// negocial gravado em `funcionalidades/<area>/<tema>/`. No manual do usuário
    /// (#272), o `manual.md` gravado na mesma pasta.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub documento_negocial: Option<CaminhoRelativo>,
    /// Funcionalidades do mapa gravadas na análise e herdadas pelas regras e pelos
    /// termos (#287).
    #[serde(default)]
    pub funcionalidades: Vec<IdFuncionalidade>,
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
        "há um ajuste ou uma escolha para a afirmação {0}, mas ela não está confirmada pelo analista \
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

/// Aprova a análise `id` do projeto `projeto` (slug) e leva à base as regras e os
/// termos, com a base da configuração do app e o aviso ao app pelo diário de
/// eventos. Depois, sincroniza o índice da base. Veja o módulo.
pub fn aprovar_analise(
    dados: &DiretorioDados,
    projeto: &str,
    id: &Id,
    opcoes: &OpcoesAprovacao,
) -> Result<Aprovacao, ErroAprovacao> {
    let config = Config::ler(dados).map_err(ErroAprovacao::Config)?;
    let raiz = config.base().ok_or(ErroAprovacao::BaseNaoConfigurada)?;
    let base = Base::abrir(raiz)?;
    let kb = base.projeto(projeto)?;
    let analises = Analises::new(kb, Arc::new(DiarioEventos::do_diretorio(dados)));
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

/// Como [`aprovar_analise`], com a base e as análises do projeto já abertas, e
/// sem sincronizar o índice.
pub fn aprovar(
    base: &Base,
    analises: &Analises,
    id: &Id,
    opcoes: &OpcoesAprovacao,
) -> Result<Aprovacao, ErroAprovacao> {
    let kb = analises.projeto();
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
    let areas = kb.ler_projeto()?.frontmatter.areas;
    let mut avisos = Vec::new();
    let mapa = MapaFuncionalidades::carregar(kb).unwrap_or_else(|e| {
        tracing::warn!("mapa de funcionalidades ilegível na aprovação");
        avisos.push(format!(
            "o mapa de funcionalidades não pôde ser lido ({e}); a análise fica sem \
             funcionalidade"
        ));
        MapaFuncionalidades::default()
    });
    let (funcionalidades, fora) = funcionalidades_da_aprovacao(kb, &atual, opcoes, &mapa);
    avisos.extend(fora.iter().map(|id| {
        format!("a funcionalidade {id} não está no mapa (ou foi descartada) e ficou de fora")
    }));
    let area_sugerida = vinculo::area_das(&mapa, &funcionalidades);
    // Confere os ajustes antes de gravar qualquer coisa.
    planejar_regras(&atual, &areas, opcoes, area_sugerida.as_deref())?;

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
    if let Err(e) = vinculo::gravar_na_analise(kb, &analise.documento, &funcionalidades) {
        tracing::warn!("funcionalidades não gravadas no documento da análise");
        avisos.push(format!(
            "as funcionalidades não foram gravadas no documento da análise ({e})"
        ));
    }
    let mut gravacao = Gravacao {
        kb,
        templates: &templates,
        avisos,
        funcionalidades: &funcionalidades,
        area_sugerida: area_sugerida.as_deref(),
    };
    let origem = analise.documento.clone();
    let plano = planejar_regras(&analise, &areas, opcoes, area_sugerida.as_deref())?;
    let regras = gravacao.regras(plano, &origem)?;
    let termos = gravacao.termos(&analise, &areas, opcoes, &origem)?;
    let mut avisos = gravacao.avisos;
    let documento_negocial = match crate::mapear::ligar_explicacao(base, kb, &analise) {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!("documento negocial não ligado ao mapa de funcionalidades");
            avisos.push(format!(
                "o documento negocial não foi ligado ao mapa de funcionalidades ({e})"
            ));
            None
        }
    };
    let documento_negocial = match crate::manual::ligar_manual(kb, &analise) {
        Ok(Some(d)) => Some(d),
        Ok(None) => documento_negocial,
        Err(e) => {
            tracing::warn!("manual do usuário não gravado na pasta da funcionalidade");
            avisos.push(format!(
                "o manual não foi gravado na pasta da funcionalidade ({e})"
            ));
            documento_negocial
        }
    };
    Ok(Aprovacao {
        analise,
        ja_aprovada,
        regras,
        termos,
        documento_negocial,
        funcionalidades,
        avisos,
    })
}

/// Funcionalidades que a aprovação grava (#287): as escolhidas pelo analista
/// ([`OpcoesAprovacao::funcionalidades`]) ou, sem a escolha, as propostas
/// ([`vinculo::propostas`]). Juntadas valem pela de destino; as que não estão no
/// mapa voltam no segundo item, para o aviso.
pub fn funcionalidades_da_aprovacao(
    kb: &cba_kb::Projeto,
    analise: &Analise,
    opcoes: &OpcoesAprovacao,
    mapa: &MapaFuncionalidades,
) -> (Vec<IdFuncionalidade>, Vec<IdFuncionalidade>) {
    match &opcoes.funcionalidades {
        Some(escolhidas) => vinculo::normalizar(mapa, escolhidas),
        None => {
            let (propostas, _) =
                vinculo::propostas(analise, &vinculo::do_documento(kb, analise), mapa);
            (propostas, Vec::new())
        }
    }
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
            // A regra informada por uma pessoa (#123) não é a da afirmação.
            Self::Titulo(t) => {
                f.evidencias.is_empty()
                    && f.informada.is_none()
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

/// Área e sigla a partir do texto informado e das áreas do `projeto.md`.
pub(crate) fn resolver_area(
    areas: &[Area],
    texto: &str,
) -> Result<(String, String), ErroAprovacao> {
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

/// Título da regra derivado do texto da afirmação, sem markdown: o trecho em
/// negrito que abre a afirmação (`**Origem dos parâmetros.** As multas…` →
/// "Origem dos parâmetros") ou, sem ele, a primeira frase. Sai sem o ponto final e,
/// se passar de [`LIMITE_DO_TITULO`] caracteres, cortado sem terminar no meio da
/// frase: numa fronteira de oração ou, sem ela, com reticências (#287).
pub fn titulo_da_regra(texto: &str) -> String {
    let linha = texto
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("");
    let linha = sem_marcador_de_bloco(&sem_notas_e_links(linha));
    let titulo = match negrito_inicial(&linha) {
        Some(n) => sem_enfase(n),
        None => {
            let limpa = sem_enfase(&linha);
            let frase = limpa.find(". ").map_or(limpa.as_str(), |i| &limpa[..i]);
            frase.to_owned()
        }
    };
    let titulo = titulo.trim().trim_end_matches(['.', ':', ';']).trim_end();
    cortar(titulo)
}

/// Título informado pelo analista, sem markdown (negrito, código, notas de rodapé,
/// links). Não é cortado.
pub fn limpar_titulo(titulo: &str) -> String {
    let linha = sem_marcador_de_bloco(&sem_notas_e_links(titulo.trim()));
    sem_enfase(&linha).trim().to_owned()
}

/// Fronteiras de oração em que um título longo pode terminar: o corte fica antes
/// delas e o título fica completo, sem reticências.
const FRONTEIRAS_DE_ORACAO: &[&str] = &[
    ", ",
    "; ",
    ": ",
    " — ",
    " – ",
    " (",
    " e ",
    " ou ",
    " mas ",
    " que ",
    " quando ",
    " se ",
    " caso ",
    " desde que ",
    " exceto ",
    " salvo ",
    " enquanto ",
    " porque ",
];

/// Palavras que não terminam um título cortado em fronteira de palavra (artigos,
/// preposições, conjunções): "…da parcela em aberto mais" vira "…da parcela em
/// aberto…".
const PALAVRAS_DE_LIGACAO: &[&str] = &[
    "a", "o", "as", "os", "um", "uma", "uns", "umas", "de", "da", "do", "das", "dos", "em", "no",
    "na", "nos", "nas", "num", "numa", "ao", "aos", "à", "às", "por", "pela", "pelo", "pelas",
    "pelos", "para", "pra", "com", "sem", "sob", "sobre", "entre", "até", "após", "e", "ou", "mas",
    "que", "se", "mais", "menos", "muito", "como", "quando", "caso", "cujo", "cuja", "seu", "sua",
    "seus", "suas", "este", "esta", "esse", "essa", "não",
];

/// Menor oração aceita no corte por fronteira, em caracteres: abaixo disso, o
/// título perderia o sentido, e o corte vai para a fronteira de palavra.
const MINIMO_DA_ORACAO: usize = LIMITE_DO_TITULO * 3 / 8;

/// Byte em que começa o caractere `n` de `texto` (ou o fim, se ele for menor).
fn byte_do_caractere(texto: &str, n: usize) -> usize {
    texto.char_indices().nth(n).map_or(texto.len(), |(i, _)| i)
}

/// Sem pontuação, espaços e aberturas soltas no fim.
fn sem_pontuacao_final(texto: &str) -> &str {
    texto.trim_end_matches([',', ';', ':', '.', ' ', '-', '—', '–', '(', '…'])
}

/// Corta em [`LIMITE_DO_TITULO`] caracteres sem terminar no meio da frase (#287):
/// de preferência antes da última fronteira de oração que cabe
/// ([`FRONTEIRAS_DE_ORACAO`]); sem ela, na última fronteira de palavra, sem as
/// palavras de ligação soltas no fim, e com reticências ("…").
fn cortar(titulo: &str) -> String {
    if titulo.chars().count() <= LIMITE_DO_TITULO {
        return titulo.to_owned();
    }
    let janela = &titulo[..byte_do_caractere(titulo, LIMITE_DO_TITULO)];
    let oracao = FRONTEIRAS_DE_ORACAO
        .iter()
        .filter_map(|f| janela.rfind(f))
        .filter(|&i| titulo[..i].chars().count() >= MINIMO_DA_ORACAO)
        .max();
    if let Some(i) = oracao {
        let corte = sem_pontuacao_final(&titulo[..i]);
        if !corte.is_empty() {
            return corte.to_owned();
        }
    }
    // Um caractere a menos, para as reticências caberem no limite.
    let janela = &titulo[..byte_do_caractere(titulo, LIMITE_DO_TITULO - 1)];
    let mut corte = match janela.rfind(' ') {
        Some(i) if i > 0 => &janela[..i],
        _ => janela,
    };
    loop {
        corte = sem_pontuacao_final(corte);
        match corte.rsplit_once(' ') {
            Some((antes, ultima))
                if PALAVRAS_DE_LIGACAO.contains(&ultima.to_lowercase().as_str())
                    && antes.chars().count() >= MINIMO_DA_ORACAO =>
            {
                corte = antes;
            }
            _ => break,
        }
    }
    format!("{corte}…")
}

/// Sem as notas de rodapé (`[^1]`) e com os links (`[texto](url)`) trocados pelo texto.
fn sem_notas_e_links(texto: &str) -> String {
    let mut saida = String::with_capacity(texto.len());
    let mut resto = texto;
    while let Some(i) = resto.find('[') {
        saida.push_str(&resto[..i]);
        let depois = &resto[i..];
        let Some(fim) = depois.find(']') else {
            saida.push_str(depois);
            return saida;
        };
        let dentro = &depois[1..fim];
        let apos = &depois[fim + 1..];
        if dentro.starts_with('^') {
            // Nota de rodapé: some, com o espaço que a separava (se houver).
            resto = apos;
        } else if let Some(url) = apos.strip_prefix('(')
            && let Some(f) = url.find(')')
        {
            saida.push_str(dentro);
            resto = &url[f + 1..];
        } else {
            saida.push('[');
            resto = &depois[1..];
        }
    }
    saida.push_str(resto);
    saida
}

/// Sem o marcador de título, citação ou item de lista no começo da linha.
fn sem_marcador_de_bloco(linha: &str) -> String {
    let mut l = linha.trim_start();
    l = l.trim_start_matches('#').trim_start();
    l = l.trim_start_matches('>').trim_start();
    for marcador in ["- ", "* ", "+ "] {
        if let Some(r) = l.strip_prefix(marcador) {
            l = r.trim_start();
            break;
        }
    }
    // Item de lista numerada (`1. `, `12) `): o número do passo não é título (#499).
    let digitos = l.len() - l.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    if digitos > 0
        && let Some(r) = l[digitos..]
            .strip_prefix(". ")
            .or_else(|| l[digitos..].strip_prefix(") "))
    {
        l = r.trim_start();
    }
    l.to_owned()
}

/// Conteúdo do negrito que abre a linha (`**Título.** resto` → `Título.`).
fn negrito_inicial(linha: &str) -> Option<&str> {
    ["**", "__"].iter().find_map(|m| {
        let resto = linha.strip_prefix(m)?;
        let fim = resto.find(m)?;
        let dentro = resto[..fim].trim();
        (!dentro.is_empty()).then_some(dentro)
    })
}

/// Sem os marcadores de negrito (`**`, `__`) e de código (`` ` ``).
fn sem_enfase(texto: &str) -> String {
    texto.replace("**", "").replace("__", "").replace('`', "")
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
    area_da_funcionalidade: Option<&str>,
) -> Result<Vec<RegraPlanejada>, ErroAprovacao> {
    if let Some(n) = opcoes
        .afirmacoes
        .keys()
        .find(|n| !confirmadas(analise).any(|a| a.numero == **n))
    {
        return Err(ErroAprovacao::AjusteSemAfirmacao(*n));
    }
    if let Some(n) = opcoes
        .regras
        .iter()
        .flatten()
        .find(|n| !confirmadas(analise).any(|a| a.numero == **n))
    {
        return Err(ErroAprovacao::AjusteSemAfirmacao(*n));
    }
    let area_da_funcionalidade = area_da_funcionalidade
        .map(str::trim)
        .filter(|a| !slug(a).is_empty());
    let area_padrao = match (nao_vazio(&opcoes.area), area_da_funcionalidade) {
        (Some(a), _) | (None, Some(a)) => resolver_area(areas, a)?,
        (None, None) => match areas {
            [unica] => resolver_area(areas, &unica.id)?,
            _ => resolver_area(areas, AREA_GERAL)?,
        },
    };

    let mut plano: Vec<RegraPlanejada> = Vec::new();
    for a in confirmadas(analise).filter(|a| opcoes.vira_regra_no(analise.fluxo, a)) {
        let ajuste = opcoes.afirmacoes.get(&a.numero);
        let (area, sigla) = match ajuste.and_then(|j| nao_vazio(&j.area)) {
            Some(texto) => resolver_area(areas, texto)?,
            None => area_padrao.clone(),
        };
        let titulo = ajuste
            .and_then(|j| nao_vazio(&j.titulo))
            .map(limpar_titulo)
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| titulo_da_regra(&a.texto));
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
    kb: &'a cba_kb::Projeto,
    templates: &'a Templates,
    avisos: Vec<String>,
    /// Funcionalidades da análise, que as regras e os termos herdam (#287).
    funcionalidades: &'a [IdFuncionalidade],
    /// Área da primeira funcionalidade, para o termo sem área.
    area_sugerida: Option<&'a str>,
}

impl Gravacao<'_> {
    /// Funcionalidades de um documento existente depois desta aprovação: as da
    /// análise, se ele veio dela (o analista pode ter trocado); somadas às dele,
    /// se veio de outra (a regra serve às duas).
    fn funcionalidades_de(
        &self,
        existentes: &[IdFuncionalidade],
        origem_do_documento: Option<&CaminhoRelativo>,
        origem: &CaminhoRelativo,
    ) -> Vec<IdFuncionalidade> {
        if origem_do_documento == Some(origem) {
            self.funcionalidades.to_vec()
        } else {
            vinculo::somar(existentes, self.funcionalidades)
        }
    }
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

    /// Regras existentes no projeto. Documentos ilegíveis ficam de fora, com aviso:
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
                let (caminho, mut existente) = existentes[i].clone();
                let f = &existente.frontmatter;
                let frontmatter = FrontmatterRegra {
                    tipo: TipoDocumento::Regra,
                    id: f.id.clone(),
                    titulo: p.titulo,
                    area: f.area.clone(),
                    status,
                    evidencias: p.evidencias,
                    origem: f.origem.clone().or_else(|| Some(origem.clone())),
                    informada: f.informada.clone(),
                    historico: f.historico.clone(),
                    funcionalidades: self.funcionalidades_de(
                        &f.funcionalidades,
                        f.origem.as_ref(),
                        origem,
                    ),
                };
                let (id, area) = (f.id.clone(), f.area.clone());
                let mut conteudo = Conteudo::da_regra(frontmatter, enunciado);
                conteudo.caminho = caminho.clone();
                let novo = self.montar(&conteudo)?;
                if novo.frontmatter.funcionalidades.is_empty() {
                    existente.tirar_campo(CAMPO_FUNCIONALIDADES);
                }
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
                informada: None,
                historico: Vec::new(),
                funcionalidades: self.funcionalidades.to_vec(),
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
            // Sem área informada, fica a do termo existente e, sem ela, a da
            // funcionalidade da análise (#287).
            area: area
                .or_else(|| anterior.and_then(|f| f.area.clone()))
                .or_else(|| {
                    self.area_sugerida
                        .and_then(|a| resolver_area(areas, a).ok())
                        .map(|(id, _)| id)
                }),
            origem: anterior
                .and_then(|f| f.origem.clone())
                .or_else(|| Some(origem.clone())),
            funcionalidades: match anterior {
                Some(f) => self.funcionalidades_de(&f.funcionalidades, f.origem.as_ref(), origem),
                None => self.funcionalidades.to_vec(),
            },
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
            Some(mut doc) => {
                if novo.frontmatter.funcionalidades.is_empty() {
                    doc.tirar_campo(CAMPO_FUNCIONALIDADES);
                }
                self.atualizar(&caminho, doc, novo)?
            }
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
        // Sem fronteira de oração depois de um começo com sentido: reticências.
        let longo = "Passada a carência, a fatura em atraso recebe multa única de \
                     PERC_MULTA_ATRASO por cento sobre o valor original";
        let t = titulo_da_regra(longo);
        assert!(t.ends_with('…'), "{t}");
        assert!(t.chars().count() <= LIMITE_DO_TITULO, "{t}");
        assert!(longo.starts_with(t.trim_end_matches('…')), "{t}");
        assert!(!t.ends_with(" …"), "{t}");
    }

    /// #499: passo de fluxo numerado ("1. O operador…") não vira o título "1".
    #[test]
    fn titulo_ignora_o_numero_do_passo() {
        assert_eq!(
            titulo_da_regra(
                "1. O operador abre a tela de reembolso. Na instalação padrão são dois tipos."
            ),
            "O operador abre a tela de reembolso"
        );
        assert_eq!(
            titulo_da_regra("- 12) O sistema grava o pedido. Depois aprova."),
            "O sistema grava o pedido"
        );
        // Número que é parte da frase continua.
        assert_eq!(
            titulo_da_regra("30 dias de carência. Depois, multa."),
            "30 dias de carência"
        );
        assert_eq!(
            limpar_titulo("2. Rateio do pagamento"),
            "Rateio do pagamento"
        );
    }

    /// #287: o título não termina no meio da frase ("…da parcela em aberto mais").
    #[test]
    fn titulo_longo_corta_na_oracao_ou_com_reticencias() {
        // Fronteira de oração dentro do limite: o título é a oração, sem reticências.
        assert_eq!(
            titulo_da_regra(
                "O acordo de parcelamento é cancelado automaticamente, quando houver \
                 atraso superior a noventa dias da parcela em aberto mais antiga"
            ),
            "O acordo de parcelamento é cancelado automaticamente"
        );
        assert_eq!(
            titulo_da_regra(
                "A multa por atraso incide sobre o valor original da fatura e os juros \
                 de mora incidem sobre o saldo devedor atualizado diariamente"
            ),
            "A multa por atraso incide sobre o valor original da fatura"
        );
        // Sem fronteira: reticências, sem palavra de ligação solta antes delas.
        let t = titulo_da_regra(
            "O desconto concedido na renegociação considera o valor total da parcela em \
             aberto mais antiga do contrato",
        );
        assert!(t.ends_with('…'), "{t}");
        let sem = t.trim_end_matches('…');
        let ultima = sem.rsplit(' ').next().unwrap();
        assert!(!PALAVRAS_DE_LIGACAO.contains(&ultima), "{t}");
        assert!(t.chars().count() <= LIMITE_DO_TITULO, "{t}");
        // A oração curta demais não serve de título.
        let t = titulo_da_regra(
            "Na baixa, o sistema recalcula o saldo devedor do contrato considerando todos os \
             pagamentos parciais registrados",
        );
        assert!(
            t.ends_with('…') && t.starts_with("Na baixa, o sistema"),
            "{t}"
        );
        // Curto: intacto.
        assert_eq!(titulo_da_regra("Multa de 2%"), "Multa de 2%");
    }

    #[test]
    fn titulo_vem_do_negrito_inicial_sem_markdown() {
        // O caso do teste de uso completo (#214): o título era o texto cru, cortado.
        assert_eq!(
            titulo_da_regra(
                "**Origem dos parâmetros.** As multas são cadastradas no produto de \
                 crédito e copiadas para o contrato na abertura[^3]."
            ),
            "Origem dos parâmetros"
        );
        assert_eq!(
            titulo_da_regra("- __Carência:__ três dias corridos."),
            "Carência"
        );
        // Negrito no meio da frase não vira título: vale a primeira frase, limpa.
        assert_eq!(
            titulo_da_regra("A multa é de **2%** sobre o `valor_original`[^1]. Depois, juros."),
            "A multa é de 2% sobre o valor_original"
        );
        assert_eq!(
            titulo_da_regra("Ver [o contrato](https://x/y.md) e a tabela[^nota]."),
            "Ver o contrato e a tabela"
        );
        assert_eq!(titulo_da_regra("## Resposta: sim"), "Resposta: sim");
        // Negrito vazio cai na primeira frase.
        assert_eq!(titulo_da_regra("**** Texto simples."), "Texto simples");
    }

    #[test]
    fn titulo_nunca_tem_markdown_e_cabe_no_limite() {
        let casos = [
            "**Origem dos parâmetros.** As multas são cadastradas no produto de crédito",
            "**Um negrito muito longo que passa do limite de oitenta caracteres do título da regra derivada.** Resto.",
            "Texto longo com nota[^1] e **ênfase** que vai bem além dos oitenta caracteres permitidos no título",
            "Palavraenormesemespacosnenhumquepassadolimitedeoitentacaracteresdotitulodaregraderivadaaqui",
            "`codigo`",
            "",
        ];
        for texto in casos {
            let t = titulo_da_regra(texto);
            assert!(
                !t.contains("**") && !t.contains("[^") && !t.contains('`'),
                "{t}"
            );
            assert!(!t.ends_with("..."), "{t}");
            // Reticências só no fim, quando o título foi cortado.
            assert!(!t.trim_end_matches('…').contains('…'), "{t}");
            assert!(t.chars().count() <= LIMITE_DO_TITULO, "{t}");
        }
    }

    #[test]
    fn titulo_do_analista_sai_sem_markdown() {
        assert_eq!(
            limpar_titulo("  **Multa** por atraso[^2] "),
            "Multa por atraso"
        );
        assert_eq!(limpar_titulo("Multa de 2%"), "Multa de 2%");
    }

    #[test]
    fn respostas_e_exemplos_nao_viram_regra_por_padrao() {
        assert!(vira_regra_por_padrao("regras_encontradas"));
        assert!(vira_regra_por_padrao("regras"));
        assert!(!vira_regra_por_padrao("resposta"));
        assert!(!vira_regra_por_padrao("exemplo"));
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
