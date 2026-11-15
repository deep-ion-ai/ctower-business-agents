//! Comparação do estado de uma ata (`R-NNNN.estado.json`) com a reunião do gabarito
//! (fluxo `reuniao`, #137).
//!
//! Determinística, sem modelo. Mede:
//!
//! - **papéis:** para cada participante esperado, a proposta do agente
//!   (`propor_papel`, campo `proposta` do participante) é o papel esperado ou um dos
//!   aceitos. O nome casa como no catálogo de pessoas ([`cba_kb::normalizar_nome`]:
//!   "Silva, Maria" e "Maria Silva" são o mesmo nome);
//! - **itens por seção:** um item esperado é atendido por um item registrado do
//!   mesmo tipo (ou de um dos `tambem_em`) cujo texto tem um termo de cada grupo de
//!   palavras-chave ([`crate::texto::atende`]). Nas pendências, o responsável e o
//!   prazo registrados são conferidos à parte, pelos grupos do gabarito. Um item
//!   registrado pode atender a mais de um item esperado (o agente juntou dois);
//! - **fala esperada citada:** o item que atende cita uma das falas que o gabarito
//!   aponta (pelo falante e pelo início, comparando o texto da fala com a do
//!   transcript original, o que vale também para a reunião longa sintética);
//! - **citações:** o status gravado (conferida, conferida em, não confere) e, com o
//!   transcript à mão, a conferência de novo de cada citação
//!   ([`cba_reunioes::conferir_citacao`]), pelo falante e pelo início;
//! - **seções vazias:** as seções sem item, e quais delas têm itens no gabarito.
//!
//! Nada de fala vai para o resultado: só identificadores, contagens, os papéis e os
//! motivos das recusas (que não trazem texto de fala).

use std::collections::BTreeMap;

use cba_docs::{CitacaoFala, EstadoAta, ItemAta, StatusCitacao, TipoItem};
use cba_kb::normalizar_nome;
use cba_reunioes::{Fala, PedidoCitacao, conferir_citacao};
use serde::Serialize;

use crate::comparacao::fracao;
use crate::gabarito::{ItemEsperado, Reuniao};
use crate::texto;

/// Os transcripts usados na comparação: o original da fixture (onde ficam as falas
/// que o gabarito aponta) e o que o agente leu (o mesmo, ou a reunião longa gerada
/// a partir dele).
#[derive(Debug, Clone, Copy)]
pub struct Transcripts<'a> {
    pub original: &'a [Fala],
    pub lido: &'a [Fala],
}

/// Resultado da comparação de uma ata com a reunião do gabarito.
#[derive(Debug, Clone, Serialize)]
pub struct AvaliacaoAta {
    /// Reunião do gabarito (`REU-*`).
    pub reuniao: String,
    /// Reunião na base (`R-NNNN`).
    pub ata: String,
    /// Status da ata (`rascunho` ou `aprovada`).
    pub status: String,
    /// Análise da reunião (`A-NNNN`), quando a execução a encontrou.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub analise: Option<String>,
    /// A análise foi concluída (`concluir_analise`), quando a execução a encontrou.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub concluida: Option<bool>,
    pub objetivo: bool,
    pub resumo: bool,
    pub papeis: Vec<PapelAvaliado>,
    pub papeis_acertados: usize,
    pub papeis_esperados: usize,
    /// Participantes com proposta que o gabarito não lista.
    pub outros_participantes: Vec<String>,
    pub itens_registrados: usize,
    pub itens: Vec<ItemAvaliado>,
    pub itens_atendidos: usize,
    pub itens_esperados: usize,
    pub por_secao: Vec<SecaoAvaliada>,
    pub citacoes: Citacoes,
    /// Itens registrados sem nenhuma citação.
    pub itens_sem_citacao: Vec<String>,
    /// Seções sem nenhum item registrado.
    pub secoes_vazias: Vec<String>,
    /// Seções vazias em que o gabarito espera itens.
    pub secoes_vazias_com_itens_esperados: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PapelAvaliado {
    pub nome: String,
    pub esperado: String,
    /// Papel proposto pelo agente, se propôs.
    pub proposto: Option<String>,
    pub acertou: bool,
    /// Citações da proposta.
    pub citacoes: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ItemAvaliado {
    pub id: String,
    pub tipo: TipoItem,
    pub descricao: String,
    /// Item da ata que o atende (`D-1`).
    pub atendido_por: Option<String>,
    /// Pendência: o responsável registrado confere com o gabarito.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub responsavel_confere: Option<bool>,
    /// Pendência: o prazo registrado confere com o gabarito.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prazo_confere: Option<bool>,
    /// O item que atende cita uma das falas esperadas (só com o transcript).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cita_fala_esperada: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SecaoAvaliada {
    pub secao: TipoItem,
    pub registrados: usize,
    pub esperados: usize,
    pub atendidos: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Citacoes {
    /// Citações dos itens e das propostas de papel.
    pub total: usize,
    /// Status `conferida` gravado.
    pub conferidas: usize,
    /// Status `conferida_em` (o transcript já foi descartado).
    pub conferidas_em: usize,
    /// Status `nao_confere` gravado.
    pub nao_conferem: usize,
    /// Conferidas de novo contra o transcript, quando há transcript.
    pub reconferidas: Option<usize>,
    /// Recusadas na conferência de novo, com o motivo (sem texto de fala).
    pub recusadas: Vec<CitacaoRecusada>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CitacaoRecusada {
    /// Item (`D-1`) ou participante (`papel de <nome>`).
    pub onde: String,
    pub motivo: String,
}

impl AvaliacaoAta {
    /// Falhas graves: nenhum item, item sem citação ou citação que não confere.
    pub fn falhas(&self) -> Vec<String> {
        let mut f = Vec::new();
        if self.itens_registrados == 0 {
            f.push("nenhum item registrado na ata".to_owned());
        }
        if !self.itens_sem_citacao.is_empty() {
            f.push(format!(
                "itens sem citação: {}",
                self.itens_sem_citacao.join(", ")
            ));
        }
        if self.citacoes.nao_conferem > 0 {
            f.push(format!(
                "{} citação(ões) gravada(s) como `não confere`",
                self.citacoes.nao_conferem
            ));
        }
        if !self.citacoes.recusadas.is_empty() {
            f.push(format!(
                "{} citação(ões) não conferem com o transcript",
                self.citacoes.recusadas.len()
            ));
        }
        f
    }

    /// Fração dos papéis esperados acertados (0 a 1).
    pub fn cobertura_de_papeis(&self) -> f64 {
        fracao(self.papeis_acertados, self.papeis_esperados)
    }

    /// Fração dos itens esperados atendidos (0 a 1).
    pub fn cobertura_de_itens(&self) -> f64 {
        fracao(self.itens_atendidos, self.itens_esperados)
    }
}

/// A citação aponta para uma fala do transcript lido (falante e início).
fn fala_citada<'a>(lido: &'a [Fala], c: &CitacaoFala) -> Option<&'a Fala> {
    let falante = normalizar_nome(&c.falante);
    lido.iter().find(|f| {
        f.inicio
            .is_some_and(|i| i.segundos() == u64::from(c.inicio.segundos()))
            && f.falante
                .as_deref()
                .is_some_and(|n| normalizar_nome(n) == falante)
    })
}

/// Texto das falas do transcript original que começam nos horários dados (`M:SS`).
fn falas_esperadas<'a>(original: &'a [Fala], horarios: &[String]) -> Vec<&'a str> {
    horarios
        .iter()
        .filter_map(|h| cba_kb::frontmatter::Horario::analisar(h))
        .flat_map(|h| {
            original
                .iter()
                .filter(move |f| {
                    f.inicio
                        .is_some_and(|i| i.segundos() == u64::from(h.segundos()))
                })
                .map(|f| f.texto.as_str())
        })
        .collect()
}

/// O item cita uma das falas esperadas: a fala citada contém o texto de uma delas.
fn cita_alguma(item: &ItemAta, lido: &[Fala], esperadas: &[&str]) -> bool {
    item.citacoes.iter().any(|c| {
        fala_citada(lido, c).is_some_and(|f| esperadas.iter().any(|e| f.texto.contains(e)))
    })
}

/// Pontos de um candidato para atender ao item esperado: a seção principal, o
/// responsável, o prazo e a fala citada desempatam.
fn melhor<'a>(
    esperado: &ItemEsperado,
    candidatos: Vec<&'a ItemAta>,
    pontos: impl Fn(&ItemAta) -> usize,
) -> Option<&'a ItemAta> {
    candidatos
        .into_iter()
        .enumerate()
        .max_by_key(|(i, item)| {
            (
                usize::from(item.tipo == esperado.tipo) * 8 + pontos(item),
                usize::MAX - i,
            )
        })
        .map(|(_, item)| item)
}

/// Compara a ata com a reunião do gabarito. Com `transcripts`, confere de novo as
/// citações e se o agente citou as falas esperadas.
pub fn avaliar_ata(
    estado: &EstadoAta,
    esperada: &Reuniao,
    transcripts: Option<Transcripts<'_>>,
) -> AvaliacaoAta {
    // Papéis.
    let proposta_de = |nome: &str| {
        let chave = normalizar_nome(nome);
        estado
            .participantes
            .iter()
            .find(|p| normalizar_nome(&p.nome) == chave)
    };
    let papeis: Vec<PapelAvaliado> = esperada
        .participantes
        .iter()
        .map(|p| {
            let proposta = proposta_de(&p.nome).and_then(|x| x.proposta.as_ref());
            PapelAvaliado {
                nome: p.nome.clone(),
                esperado: p.papel.clone(),
                proposto: proposta.map(|x| x.papel.clone()),
                acertou: proposta.is_some_and(|x| p.aceita(&x.papel)),
                citacoes: proposta.map_or(0, |x| x.citacoes.len()),
            }
        })
        .collect();
    let outros_participantes = estado
        .participantes
        .iter()
        .filter(|p| p.proposta.is_some())
        .filter(|p| {
            let chave = normalizar_nome(&p.nome);
            !esperada
                .participantes
                .iter()
                .any(|e| normalizar_nome(&e.nome) == chave)
        })
        .map(|p| p.nome.clone())
        .collect();

    // Itens.
    let itens: Vec<ItemAvaliado> = esperada
        .itens
        .iter()
        .map(|e| {
            let candidatos: Vec<&ItemAta> = estado
                .itens
                .iter()
                .filter(|i| e.aceita_tipo(i.tipo) && texto::atende(&i.texto, &e.palavras_chave))
                .collect();
            let responsavel = |i: &ItemAta| {
                (!e.responsavel.is_empty())
                    .then(|| texto::atende(i.responsavel.as_deref().unwrap_or(""), &e.responsavel))
            };
            let prazo = |i: &ItemAta| {
                (!e.prazo.is_empty())
                    .then(|| texto::atende(i.prazo.as_deref().unwrap_or(""), &e.prazo))
            };
            let esperadas: Option<(Vec<&str>, &[Fala])> = transcripts
                .filter(|_| !e.falas.is_empty())
                .map(|t| (falas_esperadas(t.original, &e.falas), t.lido));
            let cita = |i: &ItemAta| {
                esperadas
                    .as_ref()
                    .map(|(es, lido)| cita_alguma(i, lido, es))
            };
            let escolhido = melhor(e, candidatos, |i| {
                usize::from(responsavel(i) == Some(true)) * 4
                    + usize::from(prazo(i) == Some(true)) * 2
                    + usize::from(cita(i) == Some(true))
            });
            ItemAvaliado {
                id: e.id.clone(),
                tipo: e.tipo,
                descricao: e.descricao.clone(),
                atendido_por: escolhido.map(|i| i.id.to_string()),
                responsavel_confere: escolhido.and_then(responsavel),
                prazo_confere: escolhido.and_then(prazo),
                cita_fala_esperada: escolhido.and_then(cita),
            }
        })
        .collect();

    // Por seção.
    let mut por_secao: BTreeMap<TipoItem, SecaoAvaliada> = TipoItem::TODOS
        .into_iter()
        .map(|t| {
            (
                t,
                SecaoAvaliada {
                    secao: t,
                    registrados: 0,
                    esperados: 0,
                    atendidos: 0,
                },
            )
        })
        .collect();
    for i in &estado.itens {
        if let Some(s) = por_secao.get_mut(&i.tipo) {
            s.registrados += 1;
        }
    }
    for i in &itens {
        if let Some(s) = por_secao.get_mut(&i.tipo) {
            s.esperados += 1;
            s.atendidos += usize::from(i.atendido_por.is_some());
        }
    }
    let por_secao: Vec<SecaoAvaliada> = TipoItem::TODOS
        .into_iter()
        .filter_map(|t| por_secao.remove(&t))
        .collect();
    let secoes_vazias: Vec<String> = por_secao
        .iter()
        .filter(|s| s.registrados == 0)
        .map(|s| s.secao.secao().to_owned())
        .collect();
    let secoes_vazias_com_itens_esperados = por_secao
        .iter()
        .filter(|s| s.registrados == 0 && s.esperados > 0)
        .map(|s| s.secao.secao().to_owned())
        .collect();

    // Citações.
    let mut citacoes = Citacoes {
        reconferidas: transcripts.map(|_| 0),
        ..Default::default()
    };
    let das_propostas = estado.participantes.iter().flat_map(|p| {
        p.proposta
            .iter()
            .flat_map(|x| x.citacoes.iter())
            .map(move |c| (format!("papel de {}", p.nome), c))
    });
    let dos_itens = estado
        .itens
        .iter()
        .flat_map(|i| i.citacoes.iter().map(move |c| (i.id.to_string(), c)));
    for (onde, c) in dos_itens.chain(das_propostas) {
        citacoes.total += 1;
        match c.status {
            StatusCitacao::Conferida => citacoes.conferidas += 1,
            StatusCitacao::ConferidaEm(_) => citacoes.conferidas_em += 1,
            StatusCitacao::NaoConfere => citacoes.nao_conferem += 1,
        }
        if let Some(t) = transcripts {
            let pedido = PedidoCitacao::por_falante(c.falante.clone(), c.inicio, c.trecho.clone());
            match conferir_citacao(t.lido, &pedido) {
                Ok(_) => {
                    if let Some(n) = citacoes.reconferidas.as_mut() {
                        *n += 1;
                    }
                }
                Err(motivo) => citacoes.recusadas.push(CitacaoRecusada {
                    onde,
                    motivo: motivo.to_string(),
                }),
            }
        }
    }

    let papeis_acertados = papeis.iter().filter(|p| p.acertou).count();
    let itens_atendidos = itens.iter().filter(|i| i.atendido_por.is_some()).count();
    AvaliacaoAta {
        reuniao: esperada.id.clone(),
        ata: estado.id.to_string(),
        status: serde_json::to_value(estado.status)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default(),
        analise: None,
        concluida: None,
        objetivo: estado
            .objetivo
            .as_deref()
            .is_some_and(|t| !t.trim().is_empty()),
        resumo: estado
            .resumo
            .as_deref()
            .is_some_and(|t| !t.trim().is_empty()),
        papeis_esperados: papeis.len(),
        papeis_acertados,
        papeis,
        outros_participantes,
        itens_registrados: estado.itens.len(),
        itens_esperados: itens.len(),
        itens_atendidos,
        itens,
        por_secao,
        citacoes,
        itens_sem_citacao: estado
            .itens
            .iter()
            .filter(|i| i.citacoes.is_empty())
            .map(|i| i.id.to_string())
            .collect(),
        secoes_vazias,
        secoes_vazias_com_itens_esperados,
    }
}
