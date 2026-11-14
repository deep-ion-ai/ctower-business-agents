//! Revisão da análise pelo analista, antes da aprovação (spec, princípios, seções
//! 4.3, 7.4 e 10).
//!
//! O analista assina o resultado: o agente registra, e o analista revisa no app.
//! Este módulo reúne o que a tela de revisão (#37) faz, sem depender da interface:
//!
//! - [`revisar`] aplica uma [`Decisao`]: confirmar, marcar como incorreta (com
//!   comentário) ou pedir aprofundamento de uma afirmação, desfazer essa decisão,
//!   editar o texto de uma afirmação, responder ou descartar uma pergunta, aceitar
//!   ou recusar uma sugestão ou um termo. Tudo vai para o estado da análise, sob a
//!   trava, e o documento é remontado a partir dele ([`crate::documento::remontar`]):
//!   o documento `.md` nunca é editado à mão, porque a próxima montagem apagaria a
//!   edição. A edição muda só o texto: seção, confiança e evidências ficam.
//! - [`prontidao`] diz se a análise pode ser aprovada e o que falta
//!   ([`Pendencia`]), e prevê as regras que a aprovação vai criar, com as áreas do
//!   `projeto.md` para a escolha.
//! - [`aprovar_revisada`] confere a prontidão e a área e chama a aprovação
//!   ([`crate::aprovacao::aprovar_analise`]).
//! - [`exportar_analise`] exporta o documento para Markdown ou Word
//!   (`cba_docs::gravar_exportacao`), com os trechos citados lidos pelo escopo do
//!   projeto (as exclusões valem também aqui), fora das pastas do projeto.
//!
//! # O que é obrigatório antes de aprovar
//!
//! A análise precisa estar concluída pelo agente, e cada afirmação precisa de uma
//! decisão do analista: confirmada (vira regra) ou incorreta (fica registrada e não
//! vira regra). Afirmação sem decisão, com aprofundamento pedido (o pedido ainda
//! não foi atendido) ou a revalidar (o código mudou) impede a aprovação, assim como
//! termo proposto sem decisão (aceito vira glossário). Perguntas em aberto e
//! sugestões sem decisão não impedem: elas vão no documento para o solicitante e
//! para o time de desenvolvimento.
//!
//! Depois da aprovação, a revisão fica fechada: as regras já foram para a base.
//!
//! A análise do fluxo `reuniao` não é aprovada aqui ([`aprovar_revisada`] recusa):
//! ela é aprovada com a ata, em [`crate::aprovacao_ata`], que apaga o transcript.

use std::path::{Path, PathBuf};

use cba_core::ambiente::Ambiente;
use cba_core::analise::{
    Analise, ErroModelo, EstadoAfirmacao, EstadoAnalise, EstadoPergunta, EstadoTermo, Fluxo,
};
use cba_core::analises;
use cba_core::eventos::Mudanca;
use cba_docs::FonteDeTrechos;
/// Formato e opções da exportação, para quem chama sem depender de `cba-docs`.
pub use cba_docs::{Formato, OpcoesExportacao};
use cba_index::Escopo;
use cba_kb::frontmatter::{Area, Linhas, StatusSugestao};
use cba_kb::{CaminhoRelativo, Id};
use serde::{Deserialize, Serialize};

use crate::aprovacao::{self, Aprovacao, ErroAprovacao, OpcoesAprovacao, titulo_da_regra};
use crate::documento;
use crate::evidencias::ArquivoDoProjeto;

/// Decisão do analista sobre um item da análise.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "tipo", rename_all = "kebab-case")]
pub enum Decisao {
    /// Confirmar, marcar como incorreta, pedir aprofundamento ou, com
    /// `registrada`, desfazer a decisão.
    Afirmacao {
        numero: u32,
        #[serde(flatten)]
        estado: EstadoAfirmacao,
    },
    /// Novo texto de uma afirmação, com as evidências preservadas.
    EditarAfirmacao { numero: u32, texto: String },
    /// Responder ou descartar uma pergunta aberta.
    Pergunta {
        numero: u32,
        #[serde(flatten)]
        estado: EstadoPergunta,
    },
    /// Aceitar ou recusar uma sugestão proposta.
    Sugestao { numero: u32, estado: StatusSugestao },
    /// Aceitar ou recusar um termo proposto.
    Termo { numero: u32, estado: EstadoTermo },
    /// Nova definição de um termo proposto ou aceito.
    EditarTermo { numero: u32, definicao: String },
    /// As respostas destas perguntas foram enviadas ao agente.
    RespostasEnviadas { numeros: Vec<u32> },
}

impl Decisao {
    fn mudanca(&self) -> Mudanca {
        match self {
            Self::Afirmacao { .. } | Self::EditarAfirmacao { .. } => Mudanca::Afirmacao,
            Self::Pergunta { .. } | Self::RespostasEnviadas { .. } => Mudanca::Pergunta,
            Self::Sugestao { .. } => Mudanca::Sugestao,
            Self::Termo { .. } | Self::EditarTermo { .. } => Mudanca::Termo,
        }
    }

    fn aplicar(self, a: &mut Analise) -> Result<(), ErroModelo> {
        if a.estado == EstadoAnalise::Aprovada {
            return Err(ErroModelo::AnaliseAprovada(a.id.clone()));
        }
        match self {
            Self::Afirmacao { numero, estado } => a.marcar_afirmacao(numero, estado),
            Self::EditarAfirmacao { numero, texto } => a.editar_afirmacao(numero, &texto),
            Self::Pergunta { numero, estado } => a.decidir_pergunta(numero, estado),
            Self::Sugestao { numero, estado } => a.decidir_sugestao(numero, estado),
            Self::Termo { numero, estado } => a.decidir_termo(numero, estado),
            Self::EditarTermo { numero, definicao } => a.editar_termo(numero, &definicao),
            Self::RespostasEnviadas { numeros } => a.marcar_respostas_enviadas(&numeros),
        }
    }
}

/// Por que a revisão, a aprovação ou a exportação foi recusada.
#[derive(Debug, thiserror::Error)]
pub enum ErroRevisao {
    #[error(transparent)]
    Analise(#[from] analises::Erro),
    #[error(
        "a revalidação é marcada pelo app quando o código muda; escolha confirmada, \
         incorreta, aprofundar ou registrada"
    )]
    DecisaoInvalida,
    #[error("a pergunta só pode ser respondida ou descartada")]
    PerguntaReaberta,
    #[error("a sugestão só pode ser aceita ou recusada")]
    SugestaoReaberta,
    #[error("o termo só pode ser aceito ou recusado")]
    TermoReaberto,
    #[error("a análise ainda não pode ser aprovada: {}", .0.join("; "))]
    Pendente(Vec<String>),
    #[error(
        "o projeto tem mais de uma área; escolha a área das regras desta análise \
         (ou a de cada afirmação confirmada)"
    )]
    AreaNaoEscolhida,
    #[error(
        "a análise {0} é de uma reunião: ela é aprovada pela aprovação da ata, que \
         apaga o transcript"
    )]
    Reuniao(Id),
    #[error(transparent)]
    Aprovacao(#[from] ErroAprovacao),
    #[error("{0}")]
    Destino(String),
    #[error(transparent)]
    Exportacao(#[from] cba_docs::ErroExportacao),
    #[error("documento da análise ilegível: {0}")]
    Documento(std::io::Error),
}

/// Aplica a decisão do analista e remonta o documento. Devolve a análise gravada.
pub fn revisar(amb: &Ambiente, id: &Id, decisao: Decisao) -> Result<Analise, ErroRevisao> {
    match &decisao {
        Decisao::Afirmacao {
            estado: EstadoAfirmacao::ARevalidar,
            ..
        } => return Err(ErroRevisao::DecisaoInvalida),
        Decisao::Pergunta {
            estado: EstadoPergunta::Aberta,
            ..
        } => return Err(ErroRevisao::PerguntaReaberta),
        Decisao::Sugestao {
            estado: StatusSugestao::Proposta,
            ..
        } => return Err(ErroRevisao::SugestaoReaberta),
        Decisao::Termo {
            estado: EstadoTermo::Proposto,
            ..
        } => return Err(ErroRevisao::TermoReaberto),
        _ => {}
    }
    let mudanca = decisao.mudanca();
    let (_, analise) = amb.analises.alterar_e(
        id,
        mudanca,
        |a, _| decisao.aplicar(a),
        |a| {
            if documento::remontar(&amb.base, &amb.kb, a).is_err() {
                // O estado é a fonte de verdade; o documento se refaz no próximo
                // registro.
                tracing::warn!("documento da análise não remontado depois da revisão");
            }
        },
    )?;
    Ok(analise)
}

// ---------------------------------------------------------------------------
// Prontidão para aprovar

/// O que impede a aprovação.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pendencia {
    /// `analise`, `afirmacao` ou `termo`.
    pub tipo: &'static str,
    pub numero: Option<u32>,
    pub motivo: String,
}

/// Regra que a aprovação vai criar a partir de uma afirmação confirmada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegraPrevista {
    pub afirmacao: u32,
    /// Título derivado do texto, que o analista pode trocar.
    pub titulo: String,
    pub texto: String,
}

/// Se a análise pode ser aprovada, o que falta e o que a aprovação fará.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Prontidao {
    pub estado: EstadoAnalise,
    pub pode_aprovar: bool,
    pub pendencias: Vec<Pendencia>,
    /// O que não impede, mas o analista deve saber.
    pub avisos: Vec<String>,
    /// Áreas do `projeto.md`, para a escolha.
    pub areas: Vec<Area>,
    /// Com mais de uma área declarada, a área precisa ser escolhida.
    pub area_obrigatoria: bool,
    pub regras: Vec<RegraPrevista>,
    /// Afirmações incorretas: ficam registradas e não viram regra.
    pub incorretas: Vec<u32>,
    pub termos_aceitos: Vec<String>,
}

/// Pendências que impedem a aprovação de `analise` (veja o módulo).
pub fn pendencias(analise: &Analise) -> Vec<Pendencia> {
    let mut lista = Vec::new();
    match analise.estado {
        EstadoAnalise::Concluida | EstadoAnalise::Aprovada => {}
        EstadoAnalise::Criada | EstadoAnalise::EmAndamento => lista.push(Pendencia {
            tipo: "analise",
            numero: None,
            motivo: "o agente ainda não concluiu a análise (concluir_analise)".into(),
        }),
    }
    if analise.estado == EstadoAnalise::Aprovada {
        return lista;
    }
    if analise.afirmacoes.is_empty() {
        lista.push(Pendencia {
            tipo: "analise",
            numero: None,
            motivo: "não há afirmações registradas".into(),
        });
    }
    for a in &analise.afirmacoes {
        let motivo = match a.estado {
            EstadoAfirmacao::Registrada => "sem decisão: confirme ou marque como incorreta",
            EstadoAfirmacao::Aprofundar { .. } => {
                "aprofundamento pedido: espere o agente ou decida assim mesmo"
            }
            EstadoAfirmacao::ARevalidar => "a revalidar: o código mudou desde o registro",
            EstadoAfirmacao::Confirmada | EstadoAfirmacao::Incorreta { .. } => continue,
        };
        lista.push(Pendencia {
            tipo: "afirmacao",
            numero: Some(a.numero),
            motivo: format!("afirmação {}: {motivo}", a.numero),
        });
    }
    for t in &analise.termos {
        if t.estado == EstadoTermo::Proposto {
            lista.push(Pendencia {
                tipo: "termo",
                numero: Some(t.numero),
                motivo: format!("termo \"{}\": aceite ou recuse", t.termo),
            });
        }
    }
    lista
}

/// Prontidão da análise para a aprovação.
pub fn prontidao(amb: &Ambiente, id: &Id) -> Result<Prontidao, ErroRevisao> {
    let analise = amb.analises.carregar(id)?;
    let areas = amb
        .kb
        .ler_projeto()
        .map(|d| d.frontmatter.areas)
        .unwrap_or_default();
    let pendencias = pendencias(&analise);
    let mut avisos = Vec::new();
    let abertas = analise
        .perguntas
        .iter()
        .filter(|p| p.estado == EstadoPergunta::Aberta)
        .count();
    if abertas > 0 {
        avisos.push(format!(
            "{abertas} pergunta(s) em aberto vão no documento para o solicitante"
        ));
    }
    let propostas = analise
        .sugestoes
        .iter()
        .filter(|s| s.estado == StatusSugestao::Proposta)
        .count();
    if propostas > 0 {
        avisos.push(format!(
            "{propostas} sugestão(ões) de alteração sem decisão vão no documento"
        ));
    }
    let regras: Vec<RegraPrevista> = analise
        .afirmacoes
        .iter()
        .filter(|a| a.estado == EstadoAfirmacao::Confirmada)
        .map(|a| RegraPrevista {
            afirmacao: a.numero,
            titulo: titulo_da_regra(&a.texto),
            texto: a.texto.clone(),
        })
        .collect();
    Ok(Prontidao {
        estado: analise.estado,
        pode_aprovar: pendencias.is_empty() && analise.estado == EstadoAnalise::Concluida,
        pendencias,
        avisos,
        area_obrigatoria: areas.len() > 1,
        areas,
        regras,
        incorretas: analise
            .afirmacoes
            .iter()
            .filter(|a| matches!(a.estado, EstadoAfirmacao::Incorreta { .. }))
            .map(|a| a.numero)
            .collect(),
        termos_aceitos: analise
            .termos
            .iter()
            .filter(|t| t.estado == EstadoTermo::Aceito)
            .map(|t| t.termo.clone())
            .collect(),
    })
}

/// Aprova a análise revisada: confere as pendências e a escolha da área e chama
/// [`aprovacao::aprovar_analise`]. Uma análise já aprovada só confere a base
/// (a aprovação é idempotente).
pub fn aprovar_revisada(
    amb: &Ambiente,
    id: &Id,
    opcoes: &OpcoesAprovacao,
) -> Result<Aprovacao, ErroRevisao> {
    if amb.analises.carregar(id)?.fluxo == Fluxo::Reuniao {
        return Err(ErroRevisao::Reuniao(id.clone()));
    }
    let p = prontidao(amb, id)?;
    if p.estado != EstadoAnalise::Aprovada {
        if !p.pendencias.is_empty() {
            return Err(ErroRevisao::Pendente(
                p.pendencias.into_iter().map(|p| p.motivo).collect(),
            ));
        }
        let tem = |a: &Option<String>| a.as_deref().is_some_and(|t| !t.trim().is_empty());
        let todas_com_area = p.regras.iter().all(|r| {
            opcoes
                .afirmacoes
                .get(&r.afirmacao)
                .is_some_and(|aj| tem(&aj.area))
        });
        if p.area_obrigatoria && !tem(&opcoes.area) && !todas_com_area {
            return Err(ErroRevisao::AreaNaoEscolhida);
        }
    }
    Ok(aprovacao::aprovar_analise(
        &amb.dados,
        &amb.projeto.id,
        id,
        opcoes,
    )?)
}

// ---------------------------------------------------------------------------
// Exportação

/// Trechos citados lidos pelo escopo do projeto: fora das pastas ou excluído
/// (segredo, binário) não entra no apêndice.
struct TrechosDoProjeto<'a>(&'a Escopo);

impl FonteDeTrechos for TrechosDoProjeto<'_> {
    fn trecho(&self, arquivo: &CaminhoRelativo, linhas: Linhas) -> Option<String> {
        let aberto = ArquivoDoProjeto::abrir(self.0, arquivo.as_str()).ok()?;
        let todas: Vec<&str> = aberto.texto.lines().collect();
        let inicio = linhas.inicio as usize;
        if inicio == 0 || inicio > todas.len() {
            return None;
        }
        let fim = (linhas.fim as usize).min(todas.len());
        Some(todas[inicio - 1..fim].join("\n"))
    }
}

/// Destino com a extensão do formato (acrescentada se faltar), absoluto, numa
/// pasta que existe e fora das pastas do projeto: o produto não grava no
/// repositório analisado.
pub fn destino_da_exportacao(
    escopo: &Escopo,
    destino: &Path,
    formato: Formato,
) -> Result<PathBuf, ErroRevisao> {
    if !destino.is_absolute() {
        return Err(ErroRevisao::Destino(format!(
            "o destino {} precisa ser um caminho completo",
            destino.display()
        )));
    }
    let extensao = formato.extensao();
    let tem_extensao = destino
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case(extensao));
    let destino = if tem_extensao {
        destino.to_path_buf()
    } else {
        let mut nome = destino.as_os_str().to_owned();
        nome.push(".");
        nome.push(extensao);
        PathBuf::from(nome)
    };
    let pasta = destino.parent().filter(|p| p.is_dir()).ok_or_else(|| {
        ErroRevisao::Destino(format!(
            "a pasta de {} não existe; escolha outra",
            destino.display()
        ))
    })?;
    let pasta = dunce::canonicalize(pasta).map_err(|e| {
        ErroRevisao::Destino(format!("pasta de destino inacessível ({e}); escolha outra"))
    })?;
    if escopo
        .pastas()
        .iter()
        .any(|p| pasta.starts_with(&p.caminho))
    {
        return Err(ErroRevisao::Destino(
            "o destino fica dentro das pastas do projeto; o app não grava no \
             repositório. Escolha outra pasta"
                .into(),
        ));
    }
    Ok(destino)
}

/// Exporta o documento da análise para `destino` e devolve o caminho gravado.
pub fn exportar_analise(
    amb: &Ambiente,
    id: &Id,
    destino: &Path,
    formato: Formato,
    opcoes: &OpcoesExportacao,
) -> Result<PathBuf, ErroRevisao> {
    let analise = amb.analises.carregar(id)?;
    let destino = destino_da_exportacao(&amb.escopo, destino, formato)?;
    let texto = std::fs::read_to_string(amb.kb.caminho(&analise.documento))
        .map_err(ErroRevisao::Documento)?;
    let fonte = TrechosDoProjeto(&amb.escopo);
    // Commit da análise: o `base.commit` do frontmatter, para a nota sem commit próprio.
    let commit = cba_kb::Documento::<cba_kb::frontmatter::FrontmatterAnalise>::analisar(&texto)
        .ok()
        .and_then(|d| d.frontmatter.base.and_then(|b| b.commit));
    let forge = amb.projeto.repositorios.forge.as_ref().map(LinksDoForge);
    let links = forge
        .as_ref()
        .map(|f| (f as &dyn cba_docs::LinksDeEvidencia, commit.as_deref()));
    cba_docs::gravar_exportacao_com_links(&destino, &texto, formato, opcoes, Some(&fonte), links)?;
    Ok(destino)
}

/// Links das evidências no forge do projeto (GitHub ou GitLab), no commit da análise.
struct LinksDoForge<'a>(&'a cba_repos::ForgeConfig);

impl cba_docs::LinksDeEvidencia for LinksDoForge<'_> {
    fn link(&self, arquivo: &str, linhas: Linhas, commit: Option<&str>) -> Option<String> {
        cba_repos::link::link_evidencia(self.0, commit?, arquivo, Some((linhas.inicio, linhas.fim)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cba_core::analise::{Confianca, Fluxo, NovaAfirmacao, NovoTermo};
    use cba_kb::Serie;

    fn analise() -> Analise {
        let mut a = Analise::nova(
            Serie::Analise.id(1),
            "faturamento-exemplo",
            Fluxo::Perguntar,
            "Multa",
            "Como é a multa?",
            1,
        );
        for texto in ["A multa é de 2%.", "A carência é de 5 dias."] {
            a.registrar_afirmacao(
                NovaAfirmacao {
                    secao: "resposta".into(),
                    texto: texto.into(),
                    confianca: Confianca::Inferido,
                    evidencias: vec![],
                    sessao: None,
                },
                2,
            )
            .unwrap();
        }
        a.registrar_termo(
            NovoTermo {
                termo: "Multa".into(),
                definicao: "Encargo por atraso.".into(),
                sinonimos: vec![],
                onde_vive: vec![],
                area: None,
                evidencias: vec![],
                sessao: None,
            },
            3,
        )
        .unwrap();
        a
    }

    #[test]
    fn pendencias_ate_tudo_decidido() {
        let mut a = analise();
        let p = pendencias(&a);
        let tipos: Vec<_> = p.iter().map(|p| (p.tipo, p.numero)).collect();
        assert_eq!(
            tipos,
            [
                ("analise", None),
                ("afirmacao", Some(1)),
                ("afirmacao", Some(2)),
                ("termo", Some(1))
            ]
        );
        a.concluir().unwrap();
        a.marcar_afirmacao(1, EstadoAfirmacao::Confirmada).unwrap();
        a.marcar_afirmacao(2, EstadoAfirmacao::Aprofundar { pedido: None })
            .unwrap();
        a.decidir_termo(1, EstadoTermo::Recusado).unwrap();
        let p = pendencias(&a);
        assert_eq!(p.len(), 1);
        assert!(p[0].motivo.contains("aprofundamento"), "{p:?}");
        a.marcar_afirmacao(
            2,
            EstadoAfirmacao::Incorreta {
                comentario: "São 3 dias.".into(),
            },
        )
        .unwrap();
        assert!(pendencias(&a).is_empty());
    }

    #[test]
    fn decisoes_no_formato_da_tela() {
        let d: Decisao = serde_json::from_value(serde_json::json!({
            "tipo": "afirmacao", "numero": 2, "estado": "incorreta", "comentario": "não"
        }))
        .unwrap();
        assert_eq!(
            d,
            Decisao::Afirmacao {
                numero: 2,
                estado: EstadoAfirmacao::Incorreta {
                    comentario: "não".into()
                }
            }
        );
        let d: Decisao = serde_json::from_value(serde_json::json!({
            "tipo": "pergunta", "numero": 1, "estado": "respondida", "resposta": "Sim."
        }))
        .unwrap();
        assert!(matches!(d, Decisao::Pergunta { numero: 1, .. }));
        let d: Decisao = serde_json::from_value(serde_json::json!({
            "tipo": "respostas-enviadas", "numeros": [1, 3]
        }))
        .unwrap();
        assert_eq!(
            d,
            Decisao::RespostasEnviadas {
                numeros: vec![1, 3]
            }
        );
        let d: Decisao = serde_json::from_value(serde_json::json!({
            "tipo": "editar-afirmacao", "numero": 1, "texto": "Novo."
        }))
        .unwrap();
        assert!(matches!(d, Decisao::EditarAfirmacao { numero: 1, .. }));
        let d: Decisao = serde_json::from_value(serde_json::json!({
            "tipo": "editar-termo", "numero": 2, "definicao": "Nova."
        }))
        .unwrap();
        assert!(matches!(d, Decisao::EditarTermo { numero: 2, .. }));
        for (tipo, estado) in [("sugestao", "aceita"), ("termo", "recusado")] {
            let d: Result<Decisao, _> = serde_json::from_value(
                serde_json::json!({ "tipo": tipo, "numero": 1, "estado": estado }),
            );
            assert!(d.is_ok(), "{tipo}");
        }
    }
}
