//! Mapa de funcionalidades do projeto (#208): o inventário das funcionalidades em
//! linguagem de negócio, a partir do código. É um cadastro do projeto
//! ([`MapaFuncionalidades`], em `funcionalidades/mapa.estado.json`), não uma
//! análise: as ferramentas não recebem `analise`, e cada sessão de mapeamento
//! continua o mesmo mapa.
//!
//! - `iniciar_mapeamento` (só leitura): o roteiro, o recorte (projeto, pasta ou
//!   área), as áreas do projeto e o que o mapa já tem.
//! - `pontos_de_entrada` (só leitura): os pontos de entrada do mapa estrutural no
//!   recorte ([`cba_index::Mapa::pontos_de_entrada`]), com o que já foi feito com
//!   cada um (atribuído, justificado ou sem tratamento).
//! - `registrar_funcionalidade`: uma funcionalidade com nome de negócio, área,
//!   descrição, atores, pontos de entrada (cada um com evidência conferida contra
//!   o código), dados lidos e gravados e relacionadas. Sem ponto de entrada com
//!   evidência válida, é recusada. O identificador (`F-<SIGLA>-NNN`) é o
//!   informado, o da funcionalidade que já tem um dos pontos ou o próximo da
//!   sigla da área.
//! - `tratar_ponto_de_entrada`: atribui um ponto do mapa a uma funcionalidade já
//!   registrada ou o justifica (código morto, utilitário, outro motivo).
//! - `concluir_mapeamento` (só leitura): **completude, sem IA**: cruza os pontos
//!   do recorte com os atribuídos e os justificados ([`nao_mapeados`]) e recusa
//!   enquanto sobrar algum.
//!
//! A cada alteração, `funcionalidades/mapa.md` é remontado ([`gravar_documento`])
//! e o app é avisado ([`Mudanca::Funcionalidade`], alvo [`ALVO_MAPA`]). A revisão
//! (aceitar, descartar, editar, juntar, dividir) é do analista, no app. A
//! aprovação de uma explicação ligada a uma funcionalidade grava o documento
//! negocial em `funcionalidades/<area>/<tema>/negocial.md` e o liga a ela
//! ([`ligar_explicacao`]).

use std::collections::BTreeSet;
use std::fmt::Write as _;

use cba_core::ambiente::Ambiente;
use cba_core::analise::{Analise, Fluxo};
use cba_core::analises::NovaAnalise;
use cba_core::eventos::{ALVO_MAPA, DiarioEventos, Evento, Mudanca, Notificador};
use cba_core::funcionalidades::{
    ErroAlteracaoMapa, ErroMapa, EscopoMapa, Funcionalidade, MapaFuncionalidades,
    MotivoSemFuncionalidade, NovaFuncionalidade, PontoAtribuido, PontoJustificado,
    SituacaoFuncionalidade,
};
use cba_docs::{
    Citacao, Conteudo, FUNCIONALIDADES_MAPEADAS, INDICE_CATALOGO, NAO_MAPEADOS, Templates, Texto,
};
use cba_index::{Mapa, PontoEntrada};
use cba_kb::frontmatter::{
    Area, FrontmatterFuncionalidade, FrontmatterMapa, StatusAnalise, TipoDocumento,
};
use cba_kb::{Base, CaminhoRelativo, Documento, IdFuncionalidade, layout, slug};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::aprovacao::sigla_do_slug;
use crate::documento::citacao;
use crate::evidencias::{self, ArquivoDoProjeto, EvidenciaCitada};
use crate::fontes;
use crate::paginacao::{ArgsPagina, Item, Pagina, com_paginacao};
use crate::registro::{ErroFerramenta, Ferramenta, Saida, ler_argumentos};

/// Arquivo do documento negocial dentro da pasta da funcionalidade.
pub const ARQUIVO_NEGOCIAL: &str = "negocial.md";

/// Roteiro do mapeamento, devolvido por `iniciar_mapeamento`.
const ROTEIRO: &str = include_str!("../textos/roteiros/mapear.md");

fn em_uma_linha(texto: &str) -> String {
    texto.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ---------------------------------------------------------------------------
// Áreas

/// Área resolvida: slug, nome para quem lê e sigla dos identificadores.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AreaResolvida {
    pub slug: String,
    pub nome: String,
    pub sigla: String,
}

/// Área a partir do texto informado e das áreas do `projeto.md`, como na
/// aprovação das regras: procurada pelo id ou pelo nome (sem acentos nem
/// maiúsculas); fora delas, vale o slug do texto, com o texto como nome.
pub fn resolver_area(areas: &[Area], texto: &str) -> Option<AreaResolvida> {
    let chave = slug(texto);
    if chave.is_empty() {
        return None;
    }
    Some(
        match areas
            .iter()
            .find(|a| a.id == chave || slug(&a.nome) == chave)
        {
            Some(a) => AreaResolvida {
                slug: a.id.clone(),
                nome: a.nome.clone(),
                sigla: a.sigla.clone().unwrap_or_else(|| sigla_do_slug(&a.id)),
            },
            None => AreaResolvida {
                sigla: sigla_do_slug(&chave),
                slug: chave,
                nome: texto.trim().to_owned(),
            },
        },
    )
}

pub fn areas_do_projeto(kb: &cba_kb::Projeto) -> Vec<Area> {
    kb.ler_projeto()
        .map(|d| d.frontmatter.areas)
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Estado

/// O mapa do projeto.
pub fn mapa(ambiente: &Ambiente) -> Result<MapaFuncionalidades, ErroFerramenta> {
    MapaFuncionalidades::carregar(&ambiente.kb).map_err(|e| ErroFerramenta::Interno(e.to_string()))
}

fn erro_da_alteracao(e: ErroAlteracaoMapa) -> ErroFerramenta {
    match e {
        ErroAlteracaoMapa::Recusa(r) => ErroFerramenta::Recusa(r.to_string()),
        ErroAlteracaoMapa::Arquivo(a) => ErroFerramenta::Interno(a.to_string()),
    }
}

/// Mapa alterado: o estado gravado, o resultado e, se o documento não pôde ser
/// remontado, o aviso.
pub struct Alterado<T> {
    pub mapa: MapaFuncionalidades,
    pub resultado: T,
    pub avisos: Vec<String>,
}

/// Avisa o app de que o mapa mudou (a tela relê o estado). Só identificadores.
pub fn notificar(dados: &cba_core::dados::DiretorioDados, projeto: &str, versao: u64) {
    let evento = Evento {
        quando_ms: cba_core::tempo::agora_ms(),
        projeto: projeto.to_owned(),
        analise: ALVO_MAPA.to_owned(),
        mudanca: Mudanca::Funcionalidade,
        versao,
        processo: std::process::id(),
    };
    if let Err(e) = DiarioEventos::em(dados.eventos()).notificar(&evento) {
        tracing::warn!(erro = %e.kind(), "diário de eventos: falha ao notificar o mapa");
    }
}

/// Altera o mapa sob a trava, remonta o documento e avisa o app.
pub fn alterar_no_projeto<T>(
    base: &Base,
    kb: &cba_kb::Projeto,
    dados: &cba_core::dados::DiretorioDados,
    f: impl FnOnce(&mut MapaFuncionalidades, u64) -> Result<T, ErroMapa>,
) -> Result<Alterado<T>, ErroAlteracaoMapa> {
    let (mapa, resultado) = MapaFuncionalidades::alterar(kb, f)?;
    let avisos = match gravar_documento(base, kb, &mapa) {
        Ok(a) => a,
        Err(e) => {
            tracing::warn!("documento do mapa não remontado");
            vec![format!("o documento do mapa não foi remontado ({e})")]
        }
    };
    notificar(dados, kb.nome(), mapa.versao);
    Ok(Alterado {
        mapa,
        resultado,
        avisos,
    })
}

fn alterar<T>(
    ambiente: &Ambiente,
    f: impl FnOnce(&mut MapaFuncionalidades, u64) -> Result<T, ErroMapa>,
) -> Result<Alterado<T>, ErroFerramenta> {
    alterar_no_projeto(&ambiente.base, &ambiente.kb, &ambiente.dados, f).map_err(erro_da_alteracao)
}

/// Pedido que abre uma sessão de mapeamento no Claude Code (a tela "Mapa de
/// funcionalidades" do app e a avaliação usam o mesmo).
pub fn pedido_de_mapeamento(
    pasta: Option<&str>,
    area: Option<&str>,
    orientacao: Option<&str>,
) -> String {
    let limpo = |t: Option<&str>| {
        t.map(str::trim)
            .filter(|t| !t.is_empty())
            .map(str::to_owned)
    };
    let mut argumentos = Vec::new();
    if let Some(p) = limpo(pasta) {
        argumentos.push(format!("pasta \"{p}\""));
    }
    if let Some(a) = limpo(area) {
        argumentos.push(format!("area \"{a}\""));
    }
    let mut t = String::from(
        "Mapeie as funcionalidades do projeto pelo ctower: chame a ferramenta \
         iniciar_mapeamento do cba",
    );
    if !argumentos.is_empty() {
        let _ = write!(t, " com {}", argumentos.join(" e "));
    }
    t.push_str(" e siga o roteiro devolvido.");
    if let Some(o) = limpo(orientacao) {
        let _ = write!(
            t,
            " Passe em `orientacao` o pedido do analista: {}",
            em_uma_linha(&o)
        );
    }
    t
}

// ---------------------------------------------------------------------------
// Revisão do analista

/// Decisão do analista sobre o mapa, na tela "Mapa de funcionalidades".
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, Deserialize)]
#[serde(tag = "tipo", rename_all = "kebab-case")]
pub enum RevisaoDoMapa {
    /// Aceitar, descartar ou desfazer a decisão.
    Decidir {
        funcionalidade: IdFuncionalidade,
        decisao: cba_core::funcionalidades::DecisaoFuncionalidade,
    },
    /// Novo nome, área e descrição; o identificador fica. A área é procurada nas
    /// do `projeto.md`, como no registro.
    Editar {
        funcionalidade: IdFuncionalidade,
        nome: String,
        area: String,
        descricao: String,
    },
    /// Junta `origem` em `destino`.
    Juntar {
        origem: IdFuncionalidade,
        destino: IdFuncionalidade,
    },
    /// Leva os pontos escolhidos (índices a partir de 0) para uma funcionalidade
    /// nova.
    Dividir {
        funcionalidade: IdFuncionalidade,
        pontos: Vec<usize>,
        nome: String,
        descricao: String,
    },
    /// Tira a justificativa de um ponto (volta a ficar sem tratamento).
    DesfazerJustificativa { ponto: String },
    /// Liga (ou desliga) uma demanda à funcionalidade, no painel dela.
    LigarDemanda {
        funcionalidade: IdFuncionalidade,
        demanda: cba_kb::IdDemanda,
        #[serde(default = "verdadeiro")]
        ligar: bool,
    },
}

fn verdadeiro() -> bool {
    true
}

/// Aplica a decisão do analista. Devolve o mapa gravado e, na divisão, o
/// identificador da funcionalidade nova.
pub fn revisar_mapa(
    base: &Base,
    kb: &cba_kb::Projeto,
    dados: &cba_core::dados::DiretorioDados,
    revisao: RevisaoDoMapa,
) -> Result<Alterado<Option<IdFuncionalidade>>, ErroAlteracaoMapa> {
    let areas = areas_do_projeto(kb);
    alterar_no_projeto(base, kb, dados, |m, agora| match revisao {
        RevisaoDoMapa::Decidir {
            funcionalidade,
            decisao,
        } => m.decidir(&funcionalidade, decisao, agora).map(|_| None),
        RevisaoDoMapa::Editar {
            funcionalidade,
            nome,
            area,
            descricao,
        } => {
            let resolvida = resolver_area(&areas, &area).ok_or(ErroMapa::CampoVazio("area"))?;
            m.editar(
                &funcionalidade,
                cba_core::funcionalidades::EdicaoFuncionalidade {
                    nome,
                    area: resolvida.slug,
                    area_nome: resolvida.nome,
                    descricao,
                },
                agora,
            )
            .map(|_| None)
        }
        RevisaoDoMapa::Juntar { origem, destino } => {
            m.juntar(&origem, &destino, agora).map(|_| None)
        }
        RevisaoDoMapa::Dividir {
            funcionalidade,
            pontos,
            nome,
            descricao,
        } => m
            .dividir(&funcionalidade, &pontos, &nome, &descricao, agora)
            .map(Some),
        RevisaoDoMapa::DesfazerJustificativa { ponto } => {
            m.desfazer_justificativa(&ponto);
            Ok(None)
        }
        RevisaoDoMapa::LigarDemanda {
            funcionalidade,
            demanda,
            ligar,
        } => m
            .ligar_demanda(&funcionalidade, demanda, ligar)
            .map(|_| None),
    })
}

// ---------------------------------------------------------------------------
// Recorte e completude

/// Recorte a partir da pasta e da área informadas (a área é resolvida contra o
/// `projeto.md`).
pub fn escopo(
    kb: &cba_kb::Projeto,
    pasta: Option<&str>,
    area: Option<&str>,
) -> Result<EscopoMapa, ErroFerramenta> {
    let pasta = pasta.map(crate::mapa::normalizar).filter(|p| !p.is_empty());
    let area = match area.map(str::trim).filter(|a| !a.is_empty()) {
        None => None,
        Some(a) => Some(
            resolver_area(&areas_do_projeto(kb), a)
                .ok_or_else(|| {
                    ErroFerramenta::Argumentos(format!(
                        "`area`: `{a}` não tem letras nem dígitos para formar a área"
                    ))
                })?
                .slug,
        ),
    };
    Ok(EscopoMapa { pasta, area })
}

/// Pontos de entrada do mapa estrutural no recorte. No recorte de uma área, contam
/// os pontos atribuídos às funcionalidades dela e os justificados que estão na
/// pasta (a área de um ponto só existe depois que ele foi agrupado).
pub fn no_escopo(
    mapa: &MapaFuncionalidades,
    pontos: &[PontoEntrada],
    escopo: &EscopoMapa,
) -> Vec<PontoEntrada> {
    let da_area: BTreeSet<&str> = match &escopo.area {
        None => BTreeSet::new(),
        Some(area) => mapa
            .abertas()
            .filter(|f| &f.area == area)
            .flat_map(Funcionalidade::chaves)
            .collect(),
    };
    pontos
        .iter()
        .filter(|p| escopo.contem_arquivo(&p.arquivo))
        .filter(|p| escopo.area.is_none() || da_area.contains(p.chave.as_str()))
        .cloned()
        .collect()
}

/// Pontos sem funcionalidade aberta nem justificativa.
pub fn nao_mapeados<'a>(
    mapa: &MapaFuncionalidades,
    pontos: &'a [PontoEntrada],
) -> Vec<&'a PontoEntrada> {
    let atribuidos = mapa.pontos_atribuidos();
    pontos
        .iter()
        .filter(|p| {
            !atribuidos.contains(p.chave.as_str()) && mapa.justificativa(&p.chave).is_none()
        })
        .collect()
}

/// Pontos do recorte e os não mapeados, a partir do mapa estrutural atual.
pub fn completude(
    ambiente: &Ambiente,
    escopo: &EscopoMapa,
) -> Result<(Vec<PontoEntrada>, Vec<PontoEntrada>), ErroFerramenta> {
    let estrutural = fontes::mapa(ambiente)?;
    let m = mapa(ambiente)?;
    let no = no_escopo(&m, &estrutural.mapa.pontos_de_entrada(), escopo);
    let faltam = nao_mapeados(&m, &no).into_iter().cloned().collect();
    Ok((no, faltam))
}

/// Pontos de entrada do mapa estrutural do projeto (o gravado ou, sem ele, um
/// gerado agora), para o app.
pub fn pontos_do_projeto(ambiente: &Ambiente) -> Result<Vec<PontoEntrada>, String> {
    fontes::mapa(ambiente)
        .map(|m| m.mapa.pontos_de_entrada())
        .map_err(|e| e.to_string())
}

/// Chaves dos pontos de entrada do mapa estrutural atual.
pub fn chaves_do_mapa(mapa: &Mapa) -> BTreeSet<String> {
    mapa.pontos_de_entrada()
        .into_iter()
        .map(|p| p.chave)
        .collect()
}

/// A funcionalidade perdeu todos os pontos de entrada que o mapa estrutural acha
/// (e não tem ponto de fora da lista).
pub fn sem_ponto_de_entrada(f: &Funcionalidade, chaves: &BTreeSet<String>) -> bool {
    f.pontos
        .iter()
        .all(|p| p.chave.as_ref().is_some_and(|c| !chaves.contains(c)))
}

fn linha_do_ponto(p: &PontoEntrada) -> String {
    format!(
        "`{}` ({}, `{}`:{})",
        p.nome,
        p.tipo.nome(),
        p.arquivo,
        p.linhas.inicio
    )
}

/// Ponto do mapa que o agente citou: pela chave, pelo nome (único, de preferência
/// no arquivo da evidência) ou pela evidência dentro das linhas do símbolo.
fn casar_ponto<'a>(
    pontos: &'a [PontoEntrada],
    citado: Option<&str>,
    evidencia: &cba_core::analise::Evidencia,
) -> Option<&'a PontoEntrada> {
    let arquivo = evidencia.arquivo.as_str();
    if let Some(c) = citado.map(str::trim).filter(|c| !c.is_empty()) {
        if let Some(p) = pontos.iter().find(|p| p.chave == c) {
            return Some(p);
        }
        let up = c.to_uppercase();
        let pelo_nome: Vec<&PontoEntrada> = pontos
            .iter()
            .filter(|p| {
                let n = p.nome.to_uppercase();
                n == up || n.ends_with(&format!(".{up}"))
            })
            .collect();
        let no_arquivo: Vec<&&PontoEntrada> =
            pelo_nome.iter().filter(|p| p.arquivo == arquivo).collect();
        match (no_arquivo.as_slice(), pelo_nome.as_slice()) {
            ([um], _) => return Some(um),
            ([], [um]) => return Some(um),
            _ => {}
        }
    }
    let (i, f) = (evidencia.linhas.inicio, evidencia.linhas.fim);
    pontos
        .iter()
        .filter(|p| p.arquivo == arquivo && p.linhas.inicio <= f && i <= p.linhas.fim)
        .min_by_key(|p| p.linhas.fim - p.linhas.inicio)
}

// ---------------------------------------------------------------------------
// Explicação de uma funcionalidade

/// Entrada de uma explicação aberta a partir do mapa: o nome, a descrição e os
/// pontos de entrada da funcionalidade.
pub fn entrada_da_explicacao(f: &Funcionalidade) -> String {
    let mut t = format!(
        "Explicar a funcionalidade {} · {} (área {}).\n\n{}",
        f.id,
        f.nome,
        f.area_nome,
        em_uma_linha(&f.descricao)
    );
    if !f.pontos.is_empty() {
        t.push_str("\n\nPontos de entrada:");
        for p in &f.pontos {
            let _ = write!(
                t,
                "\n- {} (`{}`, linhas {}-{})",
                p.nome, p.evidencia.arquivo, p.evidencia.linhas.inicio, p.evidencia.linhas.fim
            );
        }
    }
    t
}

/// Análises do fluxo `explicar` ligadas a cada funcionalidade do mapa (a mais
/// recente de cada uma), lidas das próprias análises ([`Analise::funcionalidade`]).
pub fn explicacoes(
    analises: &cba_core::analises::Analises,
) -> std::collections::BTreeMap<IdFuncionalidade, Analise> {
    let mut mapa = std::collections::BTreeMap::new();
    for a in analises.listar().unwrap_or_default() {
        if a.fluxo != Fluxo::Explicar {
            continue;
        }
        if let Some(f) = a.funcionalidade.clone() {
            // `listar` vem em ordem de identificador: a última vence.
            mapa.insert(f, a);
        }
    }
    mapa
}

/// Cria uma análise do fluxo `explicar` para uma funcionalidade do mapa, ligada a
/// ela ([`Analise::funcionalidade`]). Recusa se a funcionalidade já tem uma.
pub fn criar_explicacao(
    ambiente: &Ambiente,
    id: &IdFuncionalidade,
) -> Result<Analise, ErroFerramenta> {
    let m = mapa(ambiente)?;
    let f = m.funcionalidade(id).filter(|f| f.aberta()).ok_or_else(|| {
        ErroFerramenta::Recusa(format!(
            "a funcionalidade {id} não está no mapa de funcionalidades do projeto"
        ))
    })?;
    if let Some(a) = explicacoes(&ambiente.analises).get(id) {
        return Err(ErroFerramenta::Recusa(format!(
            "a funcionalidade {id} já tem a explicação {}; continue nela",
            a.id
        )));
    }
    let analise = ambiente.analises.criar_com_demanda(
        NovaAnalise {
            fluxo: Fluxo::Explicar,
            titulo: format!("{} · {}", f.id, f.nome),
            entrada: entrada_da_explicacao(f),
            analista: None,
            base: crate::ciclo::versao_do_codigo(ambiente),
        },
        None,
    )?;
    let id = id.clone();
    let registrado = crate::comum::alterar(ambiente, &analise.id, Mudanca::Estado, |a, _| {
        a.funcionalidade = Some(id);
        Ok(())
    })?;
    Ok(registrado.analise)
}

/// Aprovada uma explicação ligada a uma funcionalidade do mapa, grava o documento
/// negocial em `funcionalidades/<area>/<tema>/negocial.md` (o corpo do documento
/// da análise, com o frontmatter de funcionalidade) e liga a funcionalidade a ele.
/// Devolve o caminho gravado.
pub fn ligar_explicacao(
    base: &Base,
    kb: &cba_kb::Projeto,
    analise: &Analise,
) -> Result<Option<CaminhoRelativo>, String> {
    let Some(id) = &analise.funcionalidade else {
        return Ok(None);
    };
    // Outros fluxos abertos pelo painel também guardam a funcionalidade; só a
    // explicação vira o documento negocial dela.
    if analise.fluxo != Fluxo::Explicar {
        return Ok(None);
    }
    let m = MapaFuncionalidades::carregar(kb).map_err(|e| e.to_string())?;
    let Some(f) = m.funcionalidade(id).filter(|f| f.aberta()).cloned() else {
        return Err(format!(
            "a funcionalidade {id} não está mais no mapa; o documento negocial ficou só na \
             análise"
        ));
    };
    let pasta = layout::funcionalidade(&f.area, &f.nome).map_err(|e| e.to_string())?;
    let destino = pasta.juntar(ARQUIVO_NEGOCIAL).map_err(|e| e.to_string())?;
    let corpo = kb
        .ler::<cba_kb::frontmatter::FrontmatterAnalise>(&analise.documento)
        .map_err(|e| e.to_string())?
        .corpo;
    let doc = Documento::novo(
        FrontmatterFuncionalidade {
            tipo: TipoDocumento::Funcionalidade,
            titulo: f.nome.clone(),
            area: f.area.clone(),
            tema: slug(&f.nome),
            status: StatusAnalise::Aprovada,
            origem: Some(analise.documento.clone()),
        },
        corpo,
    );
    kb.gravar(&destino, &doc).map_err(|e| e.to_string())?;
    let (gravado, _) = MapaFuncionalidades::alterar(kb, |m, _| {
        m.ligar_documento(id, &analise.id, destino.clone());
        Ok(())
    })
    .map_err(|e| e.to_string())?;
    gravar_documento(base, kb, &gravado)?;
    Ok(Some(destino))
}

// ---------------------------------------------------------------------------
// Documento do mapa

fn rotulo_situacao(f: &Funcionalidade) -> &'static str {
    match f.situacao {
        SituacaoFuncionalidade::Pendente => "a revisar",
        SituacaoFuncionalidade::Aceita => "aceita",
        SituacaoFuncionalidade::Descartada => "descartada",
        SituacaoFuncionalidade::Juntada => "juntada",
    }
}

fn situacao_no_documento(f: &Funcionalidade) -> String {
    if f.documento.is_some() {
        format!("{} · documentada", rotulo_situacao(f))
    } else {
        rotulo_situacao(f).to_owned()
    }
}

/// Bloco de uma funcionalidade, com os pontos de entrada como notas.
fn bloco(f: &Funcionalidade, caminho: &CaminhoRelativo, demandas: &[cba_kb::DemandaLida]) -> Texto {
    let mut t = format!(
        "#### {} · {}\n\n{}\n\n**Situação:** {}",
        f.id,
        em_uma_linha(&f.nome),
        em_uma_linha(&f.descricao),
        situacao_no_documento(f)
    );
    if let Some(d) = &f.documento {
        let _ = write!(
            t,
            " · documento negocial: {}",
            cba_docs::link(d.as_str(), caminho, d)
        );
    }
    if !f.atores.is_empty() {
        let _ = write!(t, "\n\n**Atores:** {}", f.atores.join(", "));
    }
    let ligadas: Vec<String> = f
        .demandas
        .iter()
        .map(|id| demanda_com_link(id, demandas))
        .collect();
    if !ligadas.is_empty() {
        let _ = write!(t, "\n\n**Demandas ligadas:** {}", ligadas.join(", "));
    }
    t.push_str("\n\n**Pontos de entrada:**\n");
    for p in &f.pontos {
        let tipo = p
            .tipo
            .map(|t| format!(" ({})", t.nome()))
            .unwrap_or_default();
        let fora = if p.chave.is_none() {
            " · _fora da lista do mapa_"
        } else {
            ""
        };
        let _ = write!(t, "\n- `{}`{tipo}{fora}", p.nome);
    }
    if !f.dados_lidos.is_empty() {
        let _ = write!(t, "\n\n**Dados lidos:** {}", f.dados_lidos.join(", "));
    }
    if !f.dados_gravados.is_empty() {
        let _ = write!(t, "\n\n**Dados gravados:** {}", f.dados_gravados.join(", "));
    }
    if !f.relacionadas.is_empty() {
        let r: Vec<String> = f.relacionadas.iter().map(ToString::to_string).collect();
        let _ = write!(t, "\n\n**Relacionadas:** {}", r.join(", "));
    }
    let mut citacoes: Vec<Citacao> = Vec::new();
    for p in &f.pontos {
        let c = citacao(&p.evidencia);
        if !citacoes
            .iter()
            .any(|x| x.arquivo == c.arquivo && x.linhas == c.linhas)
        {
            citacoes.push(c);
        }
    }
    Texto {
        secao: FUNCIONALIDADES_MAPEADAS.into(),
        texto: t,
        citacoes,
    }
}

/// `D-0004 (#41)` com a chave ligada à issue quando a demanda tem o endereço
/// do rastreador; sem ele, só o ID (e a chave, se houver).
fn demanda_com_link(id: &cba_kb::IdDemanda, demandas: &[cba_kb::DemandaLida]) -> String {
    let Some(d) = demandas.iter().find(|d| d.id == *id) else {
        return id.to_string();
    };
    let fm = &d.documento.frontmatter;
    match (fm.chave.as_deref(), fm.origem.as_deref()) {
        (Some(c), Some(o)) if o.starts_with("http") => format!("{id} ([{c}]({o}))"),
        (Some(c), _) => format!("{id} ({c})"),
        _ => id.to_string(),
    }
}

/// Textos das seções do documento do mapa: o índice por área, as funcionalidades
/// e os pontos justificados.
pub fn textos(mapa: &MapaFuncionalidades, caminho: &CaminhoRelativo) -> Vec<Texto> {
    textos_com_demandas(mapa, caminho, &[])
}

/// Como [`textos`], com as demandas da base para mostrar a chave e o link da
/// issue das demandas ligadas a cada funcionalidade (#270).
pub fn textos_com_demandas(
    mapa: &MapaFuncionalidades,
    caminho: &CaminhoRelativo,
    demandas: &[cba_kb::DemandaLida],
) -> Vec<Texto> {
    let mut textos = Vec::new();
    let itens = mapa.ordenadas();
    if !itens.is_empty() {
        let mut indice = String::new();
        let mut area_atual: Option<&str> = None;
        for f in &itens {
            if area_atual != Some(f.area_nome.as_str()) {
                if area_atual.is_some() {
                    indice.push_str("\n\n");
                }
                let _ = write!(
                    indice,
                    "### {}\n\n| Funcionalidade | Nome | Situação | Pontos de entrada |\n|---|---|---|---|",
                    f.area_nome
                );
                area_atual = Some(f.area_nome.as_str());
            }
            let nome = match &f.documento {
                Some(d) => cba_docs::link(&f.nome, caminho, d),
                None => f.nome.clone(),
            };
            let _ = write!(
                indice,
                "\n| {} | {} | {} | {} |",
                f.id,
                nome.replace('|', "\\|"),
                situacao_no_documento(f),
                f.pontos.len()
            );
        }
        textos.push(Texto::novo(INDICE_CATALOGO, indice));
        let mut area_atual: Option<&str> = None;
        for f in itens {
            if area_atual != Some(f.area_nome.as_str()) {
                textos.push(Texto::novo(
                    FUNCIONALIDADES_MAPEADAS,
                    format!("### {}", f.area_nome),
                ));
                area_atual = Some(f.area_nome.as_str());
            }
            textos.push(bloco(f, caminho, demandas));
        }
    }
    if !mapa.pontos_justificados.is_empty() {
        let mut t = String::from(
            "| Ponto de entrada | Tipo | Arquivo | Motivo | Justificativa |\n|---|---|---|---|---|",
        );
        for j in &mapa.pontos_justificados {
            let _ = write!(
                t,
                "\n| `{}` | {} | `{}` | {} | {} |",
                j.nome,
                j.tipo.nome(),
                j.arquivo,
                j.motivo.nome(),
                em_uma_linha(&j.justificativa).replace('|', "\\|")
            );
        }
        textos.push(Texto::novo(NAO_MAPEADOS, t));
    }
    textos
}

/// Remonta `funcionalidades/mapa.md` a partir do estado. Devolve os avisos.
pub fn gravar_documento(
    base: &Base,
    kb: &cba_kb::Projeto,
    mapa: &MapaFuncionalidades,
) -> Result<Vec<String>, String> {
    let caminho = layout::mapa_de_funcionalidades();
    let projeto = kb
        .ler_projeto()
        .map(|d| (d.frontmatter.id, d.frontmatter.nome))
        .map_err(|e| e.to_string())?;
    let pendentes = mapa.pendentes();
    let frontmatter = FrontmatterMapa {
        tipo: TipoDocumento::MapaFuncionalidades,
        titulo: format!("Mapa de funcionalidades · {}", projeto.1),
        projeto: projeto.0,
        status: if pendentes == 0 {
            StatusAnalise::Aprovada
        } else {
            StatusAnalise::EmRevisao
        },
        funcionalidades: mapa.abertas().count() as u32,
        pendentes: pendentes as u32,
        atualizado: Some(cba_core::tempo::hoje()),
    };
    let mut conteudo = Conteudo::novo(frontmatter, caminho.clone());
    let demandas = kb.demandas().unwrap_or_default();
    conteudo.textos = textos_com_demandas(mapa, &caminho, &demandas);
    let (templates, avisos) = Templates::da_base(base);
    let mut saida: Vec<String> = avisos.iter().map(ToString::to_string).collect();
    saida.extend(
        templates
            .gravar(kb, &conteudo)
            .map_err(|e| e.to_string())?
            .iter()
            .map(ToString::to_string),
    );
    Ok(saida)
}

// ---------------------------------------------------------------------------
// Ferramentas

#[derive(Debug, Default, Deserialize)]
struct ArgsRecorte {
    #[serde(default)]
    pasta: Option<String>,
    #[serde(default)]
    area: Option<String>,
}

impl ArgsRecorte {
    fn escopo(&self, ambiente: &Ambiente) -> Result<EscopoMapa, ErroFerramenta> {
        escopo(&ambiente.kb, self.pasta.as_deref(), self.area.as_deref())
    }
}

fn propriedades_do_recorte() -> serde_json::Map<String, Value> {
    let mut m = serde_json::Map::new();
    m.insert(
        "pasta".into(),
        json!({
            "type": "string",
            "minLength": 1,
            "description": "Opcional: só os pontos desta pasta do projeto (\"db\", \"backend/src\")."
        }),
    );
    m.insert(
        "area".into(),
        json!({
            "type": "string",
            "minLength": 1,
            "description": "Opcional: só os pontos já agrupados em funcionalidades desta área."
        }),
    );
    m
}

fn schema_do_recorte(mut extra: serde_json::Map<String, Value>) -> Value {
    let mut props = propriedades_do_recorte();
    props.append(&mut extra);
    json!({
        "type": "object",
        "properties": props,
        "additionalProperties": false
    })
}

/// Resumo do mapa para o contexto do roteiro.
fn contexto(ambiente: &Ambiente, m: &MapaFuncionalidades, escopo: &EscopoMapa) -> String {
    let mut t = String::new();
    let areas = areas_do_projeto(&ambiente.kb);
    if areas.is_empty() {
        t.push_str(
            "### Áreas do projeto\n\nO `projeto.md` não declara áreas: proponha áreas de \
             negócio.\n\n",
        );
    } else {
        t.push_str("### Áreas do projeto\n\n");
        for a in &areas {
            let _ = writeln!(t, "- {} (`{}`)", a.nome, a.id);
        }
        t.push('\n');
    }
    match completude(ambiente, escopo) {
        Ok((no, faltam)) => {
            let _ = writeln!(
                t,
                "### Pontos de entrada no recorte\n\n{} pontos de entrada, {} ainda sem \
                 funcionalidade nem justificativa.\n",
                no.len(),
                faltam.len()
            );
        }
        Err(e) => {
            let _ = writeln!(
                t,
                "### Pontos de entrada no recorte\n\nO mapa estrutural não foi lido ({e}); \
                 peça ao analista para atualizar o mapa do projeto no app.\n"
            );
        }
    }
    let itens = m.ordenadas();
    if itens.is_empty() {
        t.push_str(
            "### Mapa atual\n\nO mapa ainda está vazio: todas as funcionalidades são novas.",
        );
    } else {
        let _ = writeln!(
            t,
            "### Mapa atual\n\n{} funcionalidades ({} a revisar pelo analista), {} pontos \
             justificados. Para atualizar uma, registre com `id`.\n",
            itens.len(),
            m.pendentes(),
            m.pontos_justificados.len()
        );
        for f in itens {
            let pontos: Vec<&str> = f.pontos.iter().map(|p| p.nome.as_str()).collect();
            let _ = writeln!(
                t,
                "- {} · {} (área {}, {}): {}",
                f.id,
                f.nome,
                f.area_nome,
                rotulo_situacao(f),
                pontos.join(", ")
            );
        }
    }
    t
}

/// `iniciar_mapeamento`: o roteiro e o contexto do mapa.
pub fn iniciar_mapeamento() -> Ferramenta {
    Ferramenta::nova(
        "iniciar_mapeamento",
        "Iniciar mapeamento de funcionalidades",
        "Começa (ou continua) o mapeamento das funcionalidades do projeto a partir do \
         código e devolve o roteiro: como agrupar os pontos de entrada em \
         funcionalidades de negócio, as áreas do projeto e o que o mapa já tem. O mapa \
         é um cadastro do projeto, não uma análise: não há A-NNNN. `pasta` ou `area` \
         (opcionais) limitam o recorte desta sessão; `orientacao` traz o pedido do \
         analista. Siga o roteiro devolvido.",
        {
            let mut extra = serde_json::Map::new();
            extra.insert(
                "orientacao".into(),
                json!({
                    "type": "string",
                    "maxLength": 4000,
                    "description": "Opcional: a orientação do analista para esta sessão."
                }),
            );
            schema_do_recorte(extra)
        },
        |ambiente, args| {
            #[derive(Deserialize)]
            struct Args {
                #[serde(flatten)]
                recorte: ArgsRecorte,
                #[serde(default)]
                orientacao: Option<String>,
            }
            let args: Args = ler_argumentos(args)?;
            let escopo = args.recorte.escopo(ambiente)?;
            let m = mapa(ambiente)?;
            let mut argumentos = Vec::new();
            if let Some(p) = &escopo.pasta {
                argumentos.push(format!("pasta: \"{p}\""));
            }
            if let Some(a) = &escopo.area {
                argumentos.push(format!("area: \"{a}\""));
            }
            let argumentos = if argumentos.is_empty() {
                String::new()
            } else {
                format!(" ({})", argumentos.join(", "))
            };
            let orientacao = match args.orientacao.as_deref().map(str::trim) {
                Some(o) if !o.is_empty() => format!("Orientação do analista:\n\n{o}"),
                _ => "O analista não deixou orientação.".to_owned(),
            };
            let texto = ROTEIRO
                .replace(
                    "{{documento}}",
                    ambiente
                        .kb
                        .na_base(&layout::mapa_de_funcionalidades())
                        .as_str(),
                )
                .replace("{{escopo}}", &escopo.descricao())
                .replace("{{orientacao}}", &orientacao)
                .replace("{{argumentos}}", &argumentos)
                .replace("{{contexto}}", &contexto(ambiente, &m, &escopo));
            Ok(Saida::texto(texto))
        },
    )
    .somente_leitura()
}

/// `pontos_de_entrada`: os pontos do mapa no recorte e o tratamento de cada um.
pub fn pontos_de_entrada() -> Ferramenta {
    Ferramenta::nova(
        "pontos_de_entrada",
        "Pontos de entrada",
        "Lista os pontos de entrada do projeto, calculados do mapa estrutural: telas, \
         endpoints, lotes, rotinas de banco públicas, serviços e integrações, cada um \
         com a chave, o tipo, o arquivo e as linhas, e o que já foi feito com ele no \
         mapa de funcionalidades (atribuído a uma funcionalidade, justificado ou sem \
         tratamento). `pasta` e `area` limitam o recorte. Use para planejar o \
         agrupamento e, antes de concluir, com `so_sem_tratamento`, para ver o que \
         falta. Resposta paginada.",
        com_paginacao({
            let mut extra = serde_json::Map::new();
            extra.insert(
                "so_sem_tratamento".into(),
                json!({
                    "type": "boolean",
                    "description": "Só os pontos sem funcionalidade nem justificativa."
                }),
            );
            schema_do_recorte(extra)
        }),
        |ambiente, args| {
            #[derive(Deserialize)]
            struct Args {
                #[serde(flatten)]
                recorte: ArgsRecorte,
                #[serde(default)]
                so_sem_tratamento: bool,
                #[serde(flatten)]
                pagina: ArgsPagina,
            }
            let args: Args = ler_argumentos(args)?;
            let escopo = args.recorte.escopo(ambiente)?;
            let carregado = fontes::mapa(ambiente)?;
            let m = mapa(ambiente)?;
            let no = no_escopo(&m, &carregado.mapa.pontos_de_entrada(), &escopo);
            let faltam: BTreeSet<String> = nao_mapeados(&m, &no)
                .into_iter()
                .map(|p| p.chave.clone())
                .collect();
            let itens: Vec<Item> = no
                .iter()
                .filter(|p| !args.so_sem_tratamento || faltam.contains(&p.chave))
                .map(|p| {
                    let dona = m.dona_do_ponto(&p.chave);
                    let (situacao, detalhe) = if let Some(f) = dona {
                        ("atribuido", format!("→ {} · {}", f.id, f.nome))
                    } else if let Some(j) = m.justificativa(&p.chave) {
                        (
                            "justificado",
                            format!("→ {}: {}", j.motivo.nome(), j.justificativa),
                        )
                    } else {
                        ("sem_tratamento", "sem tratamento".to_owned())
                    };
                    Item {
                        linha: format!(
                            "- `{}` [{}] linhas {}-{}{} {}",
                            p.chave,
                            p.tipo.nome(),
                            p.linhas.inicio,
                            p.linhas.fim,
                            p.detalhe
                                .as_deref()
                                .map(|d| format!(" ({})", em_uma_linha(d)))
                                .unwrap_or_default(),
                            detalhe
                        ),
                        dados: json!({
                            "chave": p.chave,
                            "tipo": p.tipo,
                            "nome": p.nome,
                            "arquivo": p.arquivo,
                            "linha_inicial": p.linhas.inicio,
                            "linha_final": p.linhas.fim,
                            "detalhe": p.detalhe,
                            "situacao": situacao,
                            "funcionalidade": dona.map(|f| f.id.clone()),
                        }),
                    }
                })
                .collect();
            let pagina = Pagina::da_lista(itens, args.pagina);
            let mut texto = format!(
                "Pontos de entrada em {}: {} no recorte, {} sem tratamento.\n{}\n\n",
                escopo.descricao(),
                no.len(),
                faltam.len(),
                carregado.nota()
            );
            texto.push_str(&pagina.linhas());
            let _ = writeln!(texto, "\n{}", pagina.rodape());
            Ok(Saida {
                texto,
                dados: Some(json!({
                    "total": no.len(),
                    "sem_tratamento": faltam.len(),
                    "pontos": pagina.dados(),
                    "pagina": pagina.json(),
                    "mapa": carregado.json(),
                })),
            })
        },
    )
    .somente_leitura()
}

#[derive(Debug, Deserialize)]
struct PontoCitado {
    #[serde(default)]
    ponto: Option<String>,
    evidencia: EvidenciaCitada,
}

#[derive(Debug, Deserialize)]
struct ArgsRegistrar {
    #[serde(default)]
    id: Option<String>,
    nome: String,
    area: String,
    descricao: String,
    #[serde(default)]
    atores: Vec<String>,
    pontos_de_entrada: Vec<PontoCitado>,
    #[serde(default)]
    dados_lidos: Vec<String>,
    #[serde(default)]
    dados_gravados: Vec<String>,
    #[serde(default)]
    relacionadas: Vec<String>,
}

fn id_da_funcionalidade(texto: &str, campo: &str) -> Result<IdFuncionalidade, ErroFerramenta> {
    IdFuncionalidade::analisar(texto.trim()).map_err(|_| {
        ErroFerramenta::Argumentos(format!(
            "`{campo}`: `{texto}` não é o identificador de uma funcionalidade; use \
             F-<SIGLA>-NNN, como F-FAT-001"
        ))
    })
}

fn lista_de_textos(descricao: &str) -> Value {
    json!({
        "type": "array",
        "maxItems": 30,
        "items": { "type": "string", "minLength": 1, "maxLength": 200 },
        "description": descricao
    })
}

fn schema_do_ponto() -> Value {
    json!({
        "type": "object",
        "properties": {
            "ponto": {
                "type": "string",
                "minLength": 1,
                "maxLength": 500,
                "description": "O ponto de entrada como `pontos_de_entrada` mostra: a chave (`db/03_pkg.sql#PKG.PRC_X`) ou o nome (`PKG.PRC_X`, `FrmCancelamento`). Para um ponto fora da lista (menu montado por tabela, job agendado fora do repositório), descreva-o."
            },
            "evidencia": evidencias::schema_da_evidencia()
        },
        "required": ["evidencia"],
        "additionalProperties": false
    })
}

/// Identificador para registrar: o informado (precisa existir no mapa), o da
/// funcionalidade aberta que já tem um dos pontos ou nenhum (reservar um novo).
fn escolher_id(
    m: &MapaFuncionalidades,
    informado: Option<IdFuncionalidade>,
    pontos: &[PontoAtribuido],
) -> Result<Option<IdFuncionalidade>, ErroMapa> {
    if let Some(id) = informado {
        if m.funcionalidade(&id).is_none() {
            return Err(ErroMapa::Desconhecida(id));
        }
        return Ok(Some(id));
    }
    Ok(pontos
        .iter()
        .filter_map(|p| p.chave.as_deref())
        .find_map(|c| m.dona_do_ponto(c))
        .map(|f| f.id.clone()))
}

/// `registrar_funcionalidade`: uma funcionalidade com os pontos de entrada.
pub fn registrar_funcionalidade() -> Ferramenta {
    Ferramenta::nova(
        "registrar_funcionalidade",
        "Registrar funcionalidade",
        "Registra uma funcionalidade de negócio no mapa de funcionalidades do projeto: \
         `nome` de negócio (\"Parcelar fatura\", não `PKG_FAT.PRC_PARCELA`), `area` (de \
         preferência uma área do projeto.md), `descricao` em uma ou duas frases, \
         `atores`, `pontos_de_entrada` (cada um com o `ponto`, como `pontos_de_entrada` \
         mostra, e a `evidencia` do trecho que o define), `dados_lidos`, \
         `dados_gravados` e `relacionadas` (F-<SIGLA>-NNN). Uma funcionalidade é um \
         objetivo do usuário e pode ter vários pontos de entrada; um ponto fica numa \
         funcionalidade só. Sem ponto de entrada com evidência válida, é recusada. \
         Para atualizar uma funcionalidade do mapa, informe `id` (ou cite um ponto que \
         já é dela): a nova versão substitui a anterior, com os pontos, e volta para a \
         revisão do analista. Sem `id`, um novo F-<SIGLA>-NNN é reservado.",
        json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "minLength": 1,
                    "description": "F-<SIGLA>-NNN de uma funcionalidade do mapa, para atualizá-la. Omita para uma funcionalidade nova."
                },
                "nome": { "type": "string", "minLength": 1, "maxLength": 200, "description": "Nome de negócio, com verbo quando couber (\"Cancelar fatura\")." },
                "area": { "type": "string", "minLength": 1, "maxLength": 100, "description": "Área de negócio (\"Cobrança\"). Use as áreas do projeto.md quando houver; senão, proponha uma." },
                "descricao": { "type": "string", "minLength": 1, "maxLength": 1000, "description": "O que a funcionalidade faz para o usuário, em uma ou duas frases." },
                "atores": lista_de_textos("Quem usa (\"Atendente da retaguarda\", \"Lote noturno\")."),
                "pontos_de_entrada": {
                    "type": "array",
                    "minItems": 1,
                    "maxItems": 40,
                    "items": schema_do_ponto(),
                    "description": "Pontos de entrada, cada um com a evidência do trecho que o define."
                },
                "dados_lidos": lista_de_textos("Principais tabelas, arquivos ou entidades lidos."),
                "dados_gravados": lista_de_textos("Principais tabelas, arquivos ou entidades gravados."),
                "relacionadas": {
                    "type": "array",
                    "maxItems": 20,
                    "items": { "type": "string", "minLength": 1 },
                    "description": "Funcionalidades relacionadas (F-<SIGLA>-NNN)."
                }
            },
            "required": ["nome", "area", "descricao", "pontos_de_entrada"],
            "additionalProperties": false
        }),
        |ambiente, args| {
            let args: ArgsRegistrar = ler_argumentos(args)?;
            let informado = args
                .id
                .as_deref()
                .map(|t| id_da_funcionalidade(t, "id"))
                .transpose()?;
            let relacionadas = args
                .relacionadas
                .iter()
                .map(|t| id_da_funcionalidade(t, "relacionadas"))
                .collect::<Result<Vec<_>, _>>()?;
            let area =
                resolver_area(&areas_do_projeto(&ambiente.kb), &args.area).ok_or_else(|| {
                    ErroFerramenta::Argumentos(format!(
                        "`area`: `{}` não tem letras nem dígitos para formar a área",
                        args.area
                    ))
                })?;
            if args.pontos_de_entrada.is_empty() {
                return Err(ErroFerramenta::Recusa(
                    ErroMapa::SemPontoDeEntrada.to_string(),
                ));
            }
            let mut recusas = Vec::new();
            let mut evidencias_ok = Vec::new();
            for (i, p) in args.pontos_de_entrada.iter().enumerate() {
                match evidencias::validar(&ambiente.escopo, &p.evidencia) {
                    Ok(e) => evidencias_ok.push(e),
                    Err(r) => recusas.push(format!("- pontos_de_entrada[{i}].evidencia ({r})")),
                }
            }
            if !recusas.is_empty() {
                return Err(ErroFerramenta::Recusa(format!(
                    "{} de {} pontos de entrada com evidência recusada; a funcionalidade não \
                     foi registrada. Toda funcionalidade precisa de pontos de entrada com \
                     evidência válida. Corrija e chame de novo:\n{}",
                    recusas.len(),
                    args.pontos_de_entrada.len(),
                    recusas.join("\n")
                )));
            }
            let carregado = fontes::mapa(ambiente).ok();
            let todos = carregado
                .as_ref()
                .map(|c| c.mapa.pontos_de_entrada())
                .unwrap_or_default();
            let pontos: Vec<PontoAtribuido> = args
                .pontos_de_entrada
                .iter()
                .zip(evidencias_ok)
                .map(|(citado, evidencia)| {
                    match casar_ponto(&todos, citado.ponto.as_deref(), &evidencia) {
                        Some(p) => PontoAtribuido {
                            chave: Some(p.chave.clone()),
                            tipo: Some(p.tipo),
                            nome: p.nome.clone(),
                            evidencia,
                        },
                        None => PontoAtribuido {
                            chave: None,
                            tipo: None,
                            nome: citado
                                .ponto
                                .clone()
                                .filter(|p| !p.trim().is_empty())
                                .map(|p| em_uma_linha(&p))
                                .unwrap_or_else(|| {
                                    em_uma_linha(&evidencia.trecho).chars().take(80).collect()
                                }),
                            evidencia,
                        },
                    }
                })
                .collect();
            let nova = NovaFuncionalidade {
                nome: args.nome,
                area: area.slug.clone(),
                area_nome: area.nome.clone(),
                descricao: args.descricao,
                atores: args.atores,
                pontos,
                dados_lidos: args.dados_lidos,
                dados_gravados: args.dados_gravados,
                relacionadas: relacionadas.clone(),
                sessao: None,
            };
            let sigla = area.sigla.clone();
            let alterado = alterar(ambiente, |m, agora| {
                let fid = match escolher_id(m, informado, &nova.pontos)? {
                    Some(fid) => fid,
                    None => m.proximo_id(&sigla)?,
                };
                let substituida = m.registrar(nova, fid.clone(), agora)?;
                Ok((fid, substituida))
            })?;
            let (fid, substituida) = alterado.resultado.clone();
            let m = &alterado.mapa;
            let f = m
                .funcionalidade(&fid)
                .ok_or_else(|| ErroFerramenta::Interno(format!("{fid} não foi gravada")))?;
            let desconhecidas: Vec<String> = relacionadas
                .iter()
                .filter(|r| m.funcionalidade(r).is_none())
                .map(ToString::to_string)
                .collect();
            let restantes = carregado
                .as_ref()
                .map(|c| nao_mapeados(m, &c.mapa.pontos_de_entrada()).len());
            let mut dados = json!({
                "funcionalidade": fid,
                "substituida": substituida,
                "area": { "slug": f.area, "nome": f.area_nome },
                "pontos_de_entrada": f.pontos.iter().map(|p| json!({
                    "ponto": p.nome,
                    "chave": p.chave,
                    "tipo": p.tipo,
                    "no_mapa": p.chave.is_some(),
                })).collect::<Vec<_>>(),
                "pontos_sem_tratamento_no_projeto": restantes,
                "documento": ambiente.kb.na_base(&layout::mapa_de_funcionalidades()),
            });
            if !desconhecidas.is_empty() {
                dados["aviso_relacionadas"] = json!(format!(
                    "relacionadas ainda não registradas no mapa: {}; registre-as ou corrija",
                    desconhecidas.join(", ")
                ));
            }
            if !alterado.avisos.is_empty() {
                dados["avisos"] = json!(alterado.avisos);
            }
            Ok(Saida::json(dados))
        },
    )
}

/// `propor_vinculo_de_issue`: o agente propõe a funcionalidade de uma issue (#270).
pub fn propor_vinculo_de_issue() -> Ferramenta {
    Ferramenta::nova(
        "propor_vinculo_de_issue",
        "Propor vínculo de issue",
        "Propõe a(s) funcionalidade(s) do mapa a que uma issue do rastreador pertence, com a \
         evidência (ponto de entrada ou trecho da issue que sustenta o vínculo). É só uma \
         proposta: o analista confirma na tela \"Vincular demandas\". Não chame para a issue \
         sem relação clara com o mapa.",
        json!({
            "type": "object",
            "properties": {
                "origem": { "type": "string", "enum": ["github", "jira"] },
                "chave": { "type": "string", "minLength": 1, "description": "`#41` ou `PROJ-9`, como o pedido mostra." },
                "funcionalidades": {
                    "type": "array",
                    "minItems": 1,
                    "items": { "type": "string", "minLength": 1 },
                    "description": "F-<SIGLA>-NNN já registradas e abertas no mapa."
                },
                "evidencia": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 1000,
                    "description": "Por que: o ponto de entrada (`PKG.FN_X`) ou o trecho da issue que a liga à funcionalidade."
                }
            },
            "required": ["origem", "chave", "funcionalidades", "evidencia"],
            "additionalProperties": false
        }),
        |ambiente, args| {
            #[derive(Deserialize)]
            struct Args {
                origem: String,
                chave: String,
                funcionalidades: Vec<String>,
                evidencia: String,
            }
            let args: Args = ler_argumentos(args)?;
            if args.origem != "github" && args.origem != "jira" {
                return Err(ErroFerramenta::Argumentos(
                    "`origem`: use `github` ou `jira`".into(),
                ));
            }
            let ids = args
                .funcionalidades
                .iter()
                .map(|f| id_da_funcionalidade(f, "funcionalidades"))
                .collect::<Result<Vec<_>, _>>()?;
            let proposta = cba_core::funcionalidades::PropostaDeIssue {
                origem: args.origem,
                chave: args.chave.trim().to_owned(),
                funcionalidades: ids,
                evidencia: args.evidencia,
                registrado_ms: 0,
            };
            let alterado = alterar(ambiente, |m, agora| {
                m.propor_issue(cba_core::funcionalidades::PropostaDeIssue {
                    registrado_ms: agora,
                    ..proposta.clone()
                })
            })?;
            Ok(Saida::json(json!({
                "issue": proposta.chave,
                "funcionalidades": proposta.funcionalidades.iter().map(ToString::to_string).collect::<Vec<_>>(),
                "avisos": alterado.avisos,
            })))
        },
    )
}

/// `tratar_ponto_de_entrada`: atribui a uma funcionalidade ou justifica.
pub fn tratar_ponto_de_entrada() -> Ferramenta {
    Ferramenta::nova(
        "tratar_ponto_de_entrada",
        "Tratar ponto de entrada",
        "Trata um ponto de entrada do mapa estrutural que está sem funcionalidade: \
         `atribuir` à funcionalidade `funcionalidade` (F-<SIGLA>-NNN, já registrada no \
         mapa; a evidência é a declaração do ponto no código) ou justificar por que \
         fica sem funcionalidade, com `motivo` (`codigo_morto`, `utilitario` ou \
         `outro`) e `justificativa`. Antes de concluir, todo ponto do recorte precisa \
         estar numa funcionalidade ou justificado.",
        json!({
            "type": "object",
            "properties": {
                "ponto": {
                    "type": "string",
                    "minLength": 1,
                    "description": "Chave do ponto, como `pontos_de_entrada` mostra (`db/03_pkg.sql#PKG.FN_X`)."
                },
                "tratamento": { "type": "string", "enum": ["atribuir", "justificar"] },
                "funcionalidade": { "type": "string", "minLength": 1, "description": "No `atribuir`: F-<SIGLA>-NNN." },
                "motivo": { "type": "string", "enum": ["codigo_morto", "utilitario", "outro"], "description": "No `justificar`." },
                "justificativa": { "type": "string", "minLength": 1, "maxLength": 1000, "description": "No `justificar`: por que o ponto fica sem funcionalidade (\"lê parâmetros para as rotinas de cobrança\")." }
            },
            "required": ["ponto", "tratamento"],
            "additionalProperties": false
        }),
        |ambiente, args| {
            #[derive(Deserialize)]
            struct Args {
                ponto: String,
                tratamento: String,
                #[serde(default)]
                funcionalidade: Option<String>,
                #[serde(default)]
                motivo: Option<MotivoSemFuncionalidade>,
                #[serde(default)]
                justificativa: Option<String>,
            }
            let args: Args = ler_argumentos(args)?;
            let carregado = fontes::mapa(ambiente)?;
            let todos = carregado.mapa.pontos_de_entrada();
            let chave = args.ponto.trim();
            let ponto = todos.iter().find(|p| p.chave == chave).ok_or_else(|| {
                ErroFerramenta::Recusa(format!(
                    "o ponto `{chave}` não está entre os pontos de entrada do mapa; use a chave \
                     como `pontos_de_entrada` mostra. Um ponto fora da lista entra numa \
                     funcionalidade por `registrar_funcionalidade`, com a evidência"
                ))
            })?;
            let alterado = match args.tratamento.as_str() {
                "atribuir" => {
                    let fid = id_da_funcionalidade(
                        args.funcionalidade.as_deref().ok_or_else(|| {
                            ErroFerramenta::Argumentos(
                                "no `atribuir`, informe `funcionalidade` (F-<SIGLA>-NNN)".into(),
                            )
                        })?,
                        "funcionalidade",
                    )?;
                    let evidencia = evidencia_da_declaracao(ambiente, ponto)?;
                    let atribuido = PontoAtribuido {
                        chave: Some(ponto.chave.clone()),
                        tipo: Some(ponto.tipo),
                        nome: ponto.nome.clone(),
                        evidencia,
                    };
                    alterar(ambiente, |m, agora| {
                        m.atribuir_ponto(&fid, atribuido, agora)
                    })?
                }
                "justificar" => {
                    let motivo = args.motivo.ok_or_else(|| {
                        ErroFerramenta::Argumentos(
                            "no `justificar`, informe `motivo` (codigo_morto, utilitario ou \
                             outro) e `justificativa`"
                                .into(),
                        )
                    })?;
                    let justificado = PontoJustificado {
                        chave: ponto.chave.clone(),
                        tipo: ponto.tipo,
                        nome: ponto.nome.clone(),
                        arquivo: ponto.arquivo.clone(),
                        motivo,
                        justificativa: args.justificativa.unwrap_or_default(),
                        sessao: None,
                        registrado_ms: 0,
                    };
                    alterar(ambiente, |m, agora| {
                        m.justificar_ponto(PontoJustificado {
                            registrado_ms: agora,
                            ..justificado
                        })
                    })?
                }
                outro => {
                    return Err(ErroFerramenta::Argumentos(format!(
                        "`tratamento`: `{outro}` não é atribuir nem justificar"
                    )));
                }
            };
            let restantes = nao_mapeados(&alterado.mapa, &todos).len();
            let mut dados = json!({
                "ponto": ponto.chave,
                "tratamento": args.tratamento,
                "pontos_sem_tratamento_no_projeto": restantes,
            });
            if !alterado.avisos.is_empty() {
                dados["avisos"] = json!(alterado.avisos);
            }
            Ok(Saida::json(dados))
        },
    )
}

/// Evidência da declaração de um ponto do mapa: a primeira linha não vazia do
/// símbolo.
fn evidencia_da_declaracao(
    ambiente: &Ambiente,
    ponto: &PontoEntrada,
) -> Result<cba_core::analise::Evidencia, ErroFerramenta> {
    let arquivo = ArquivoDoProjeto::abrir(&ambiente.escopo, &ponto.arquivo).map_err(|m| {
        ErroFerramenta::Recusa(format!(
            "o arquivo `{}` não pôde ser lido: {m}",
            ponto.arquivo
        ))
    })?;
    let linhas: Vec<&str> = arquivo.texto.lines().collect();
    let (inicio, fim) = (ponto.linhas.inicio as usize, ponto.linhas.fim as usize);
    for n in inicio..=fim.min(linhas.len()) {
        let texto = linhas[n - 1].trim();
        if texto.is_empty() {
            continue;
        }
        let l = cba_kb::frontmatter::Linhas::novo(n as u32, n as u32).expect("linha válida");
        return arquivo.conferir(l, texto).map_err(|m| {
            ErroFerramenta::Recusa(format!(
                "a declaração de `{}` não confere com o código ({m}); atribua com \
                 `registrar_funcionalidade`, citando a evidência",
                ponto.nome
            ))
        });
    }
    Err(ErroFerramenta::Recusa(format!(
        "a declaração de `{}` não foi achada em `{}`; atribua com `registrar_funcionalidade`, \
         citando a evidência",
        ponto.nome, ponto.arquivo
    )))
}

/// `concluir_mapeamento`: confere a completude do recorte.
pub fn concluir_mapeamento() -> Ferramenta {
    Ferramenta::nova(
        "concluir_mapeamento",
        "Concluir mapeamento",
        "Confere, sem IA, se todos os pontos de entrada do recorte (`pasta` e `area` \
         opcionais) estão numa funcionalidade ou justificados. Recusa e lista os que \
         faltam; quando não falta nenhum, devolve o resumo do mapa para você relatar ao \
         analista, que revisa as funcionalidades na tela \"Mapa de funcionalidades\" do \
         app.",
        schema_do_recorte(serde_json::Map::new()),
        |ambiente, args| {
            let args: ArgsRecorte = ler_argumentos(args)?;
            let escopo = args.escopo(ambiente)?;
            let m = mapa(ambiente)?;
            if m.abertas().next().is_none() {
                return Err(ErroFerramenta::Recusa(
                    "o mapa ainda não tem funcionalidades. Veja os pontos de entrada com \
                     `pontos_de_entrada`, agrupe-os em funcionalidades de negócio com \
                     `registrar_funcionalidade` e chame `concluir_mapeamento` de novo"
                        .into(),
                ));
            }
            let (no, faltam) = completude(ambiente, &escopo)?;
            if !faltam.is_empty() {
                const MOSTRADOS: usize = 30;
                let mut t = format!(
                    "{} ponto(s) de entrada de {} sem funcionalidade nem justificativa. \
                     Para cada um, atribua a uma funcionalidade (`tratar_ponto_de_entrada` \
                     com `atribuir`, ou `registrar_funcionalidade`), crie uma \
                     funcionalidade nova ou justifique (`tratar_ponto_de_entrada` com \
                     `justificar`: código morto, utilitário ou outro motivo), e chame \
                     `concluir_mapeamento` de novo:",
                    faltam.len(),
                    escopo.descricao()
                );
                for p in faltam.iter().take(MOSTRADOS) {
                    let _ = write!(t, "\n- {} · chave `{}`", linha_do_ponto(p), p.chave);
                }
                if faltam.len() > MOSTRADOS {
                    let _ = write!(
                        t,
                        "\n- … e mais {}; veja todos com `pontos_de_entrada` \
                         (so_sem_tratamento: true)",
                        faltam.len() - MOSTRADOS
                    );
                }
                return Err(ErroFerramenta::Recusa(t));
            }
            let mut areas: Vec<(String, usize)> = Vec::new();
            for f in m.ordenadas() {
                match areas.last_mut() {
                    Some((a, n)) if a == &f.area_nome => *n += 1,
                    _ => areas.push((f.area_nome.clone(), 1)),
                }
            }
            Ok(Saida::json(json!({
                "recorte": escopo.descricao(),
                "pontos_de_entrada": no.len(),
                "funcionalidades": m.abertas().count(),
                "a_revisar": m.pendentes(),
                "pontos_justificados": m.pontos_justificados.len(),
                "areas": areas.iter().map(|(a, n)| json!({"area": a, "funcionalidades": n})).collect::<Vec<_>>(),
                "documento": ambiente.kb.na_base(&layout::mapa_de_funcionalidades()),
                "proximo_passo": "Relate ao analista no chat, como pede o roteiro. A revisão \
                    das funcionalidades é feita por ele na tela \"Mapa de funcionalidades\" \
                    do app.",
            })))
        },
    )
    .somente_leitura()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cba_index::TipoEntrada;
    use cba_kb::frontmatter::Linhas;

    fn pe(chave: &str, tipo: TipoEntrada, i: u32, f: u32) -> PontoEntrada {
        let (arquivo, nome) = chave.split_once('#').unwrap();
        PontoEntrada {
            chave: chave.into(),
            tipo,
            nome: nome.into(),
            arquivo: arquivo.into(),
            linhas: cba_index::Linhas::new(i, f),
            detalhe: None,
        }
    }

    fn ev(arquivo: &str, i: u32, f: u32) -> cba_core::analise::Evidencia {
        cba_core::analise::Evidencia {
            arquivo: CaminhoRelativo::novo(arquivo).unwrap(),
            linhas: Linhas::novo(i, f).unwrap(),
            trecho: "x".into(),
            hash: "h".into(),
            commit: None,
            estado: Default::default(),
        }
    }

    #[test]
    fn casa_pela_chave_pelo_nome_e_pela_evidencia() {
        let pontos = vec![
            pe("db/a.sql#PKG.PRC_A", TipoEntrada::RotinaBanco, 10, 20),
            pe("db/a.sql#PKG.PRC_B", TipoEntrada::RotinaBanco, 22, 30),
            pe("db/b.sql#OUTRO.PRC_A", TipoEntrada::RotinaBanco, 1, 5),
        ];
        let e = ev("db/a.sql", 12, 12);
        assert_eq!(
            casar_ponto(&pontos, Some("db/a.sql#PKG.PRC_B"), &e)
                .unwrap()
                .nome,
            "PKG.PRC_B"
        );
        // Ambíguo pelo nome: vale o do arquivo da evidência.
        assert_eq!(
            casar_ponto(&pontos, Some("prc_a"), &e).unwrap().chave,
            "db/a.sql#PKG.PRC_A"
        );
        // Sem nome: pela evidência.
        assert_eq!(
            casar_ponto(&pontos, None, &ev("db/a.sql", 25, 26))
                .unwrap()
                .nome,
            "PKG.PRC_B"
        );
        assert!(casar_ponto(&pontos, Some("menu por tabela"), &ev("db/c.sql", 1, 1)).is_none());
    }

    #[test]
    fn pedido_com_recorte_e_orientacao() {
        assert_eq!(
            pedido_de_mapeamento(None, None, None),
            "Mapeie as funcionalidades do projeto pelo ctower: chame a ferramenta \
             iniciar_mapeamento do cba e siga o roteiro devolvido."
        );
        let p = pedido_de_mapeamento(Some("db"), Some(" "), Some("Comece\npelo banco."));
        assert!(
            p.contains("iniciar_mapeamento do cba com pasta \"db\" e siga"),
            "{p}"
        );
        assert!(p.ends_with("pedido do analista: Comece pelo banco."), "{p}");
    }

    #[test]
    fn areas_do_projeto_e_propostas() {
        let areas = vec![Area {
            id: "cobranca".into(),
            nome: "Cobrança".into(),
            sigla: Some("FAT".into()),
        }];
        let a = resolver_area(&areas, "cobrança").unwrap();
        assert_eq!(
            (a.slug.as_str(), a.nome.as_str(), a.sigla.as_str()),
            ("cobranca", "Cobrança", "FAT")
        );
        let b = resolver_area(&areas, "Parcelamento").unwrap();
        assert_eq!(
            (b.slug.as_str(), b.nome.as_str(), b.sigla.as_str()),
            ("parcelamento", "Parcelamento", "PAR")
        );
        assert!(resolver_area(&areas, "??").is_none());
    }

    #[test]
    fn recorte_e_nao_mapeados() {
        let pontos = vec![
            pe("db/a.sql#A", TipoEntrada::RotinaBanco, 1, 2),
            pe("db/a.sql#B", TipoEntrada::RotinaBanco, 3, 4),
            pe("front/t.ts#T", TipoEntrada::Tela, 1, 9),
        ];
        let mut m = MapaFuncionalidades::default();
        m.registrar(
            NovaFuncionalidade {
                nome: "A".into(),
                area: "cobranca".into(),
                area_nome: "Cobrança".into(),
                descricao: "D.".into(),
                atores: vec![],
                pontos: vec![PontoAtribuido {
                    chave: Some("db/a.sql#A".into()),
                    tipo: None,
                    nome: "A".into(),
                    evidencia: ev("db/a.sql", 1, 1),
                }],
                dados_lidos: vec![],
                dados_gravados: vec![],
                relacionadas: vec![],
                sessao: None,
            },
            IdFuncionalidade::novo("COB", 1).unwrap(),
            1,
        )
        .unwrap();
        let todo = EscopoMapa::default();
        let no = no_escopo(&m, &pontos, &todo);
        assert_eq!(no.len(), 3);
        let faltam: Vec<&str> = nao_mapeados(&m, &no)
            .iter()
            .map(|p| p.chave.as_str())
            .collect();
        assert_eq!(faltam, ["db/a.sql#B", "front/t.ts#T"]);
        let db = EscopoMapa {
            pasta: Some("db".into()),
            area: None,
        };
        assert_eq!(no_escopo(&m, &pontos, &db).len(), 2);
        let area = EscopoMapa {
            pasta: None,
            area: Some("cobranca".into()),
        };
        assert_eq!(no_escopo(&m, &pontos, &area).len(), 1);

        let chaves: BTreeSet<String> = ["db/a.sql#B".to_owned()].into();
        assert!(sem_ponto_de_entrada(&m.funcionalidades[0], &chaves));

        let caminho = layout::mapa_de_funcionalidades();
        let textos = textos(&m, &caminho);
        assert!(
            textos[0]
                .texto
                .contains("| F-COB-001 | A | a revisar | 1 |")
        );
        assert_eq!(textos[1].texto, "### Cobrança");
        assert!(textos[2].texto.starts_with("#### F-COB-001 · A"));
        assert_eq!(textos[2].citacoes.len(), 1);
    }
}
