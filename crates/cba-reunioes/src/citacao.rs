//! Evidência do tipo **fala** (#136): confere a citação de um item da ata
//! contra a fala citada, enquanto o transcript existe.
//!
//! # O que a ferramenta `registrar_item_ata` (#139) manda
//!
//! Um [`PedidoCitacao`] por fala de origem:
//!
//! - `indice`: o índice da fala (a partir de 1), como `ler_transcript` devolve.
//!   É o jeito preferido de apontar a fala.
//! - `falante` e `inicio` (`"00:12:34"`): opcionais quando há `indice`, e então
//!   precisam bater com a fala (o falante pela chave de
//!   [`cba_kb::normalizar_nome`], o início no mesmo segundo). Sem `indice`, os
//!   dois são obrigatórios e a fala é procurada por eles.
//! - `trecho`: o trecho curto citado, copiado da fala.
//!
//! A citação aceita ([`CitacaoConferida`]) leva o falante e o horário **da
//! fala**, não os do pedido, para a ata mostrar o que está no transcript.
//!
//! # Quando o trecho confere
//!
//! O trecho tem que aparecer na fala como uma sequência inteira de palavras,
//! depois de normalizar os dois textos com [`cba_kb::normalizar_texto`]:
//!
//! - **caixa** e **espaços** (inclusive quebras de linha) não importam;
//! - **acentos** não importam (`nao` confere com `não`): o transcript do Teams
//!   erra acentos com frequência, e o agente às vezes os corrige;
//! - **pontuação** não importa (vira separador de palavras): a pontuação do
//!   transcript é gerada pelo reconhecimento de voz e não faz parte do que foi
//!   dito;
//! - **palavras inteiras**: um pedaço de palavra não confere (`cancela` não
//!   confere com `cancelamento`), e a ordem das palavras é a da fala.
//!
//! O trecho tem no máximo [`LIMITE_DO_TRECHO`] caracteres (a ata guarda só
//! trechos curtos) e precisa ter ao menos uma letra ou um número.
//!
//! Nada aqui escreve no log. As recusas ([`RecusaCitacao`]) dizem o motivo com
//! o índice, o falante e o horário da fala, nunca o texto dela nem o trecho.

use std::fmt;

use cba_docs::{CitacaoFala, StatusCitacao};
use cba_kb::frontmatter::Horario as HorarioAta;
use cba_kb::{normalizar_nome, normalizar_texto};
use serde::{Deserialize, Serialize};

use crate::transcript::Fala;

/// Tamanho máximo do trecho citado, em caracteres.
pub const LIMITE_DO_TRECHO: usize = 240;

/// Falante gravado na citação quando a fala não tem falante (cue sem `<v>`,
/// texto não reconhecido).
pub const FALANTE_DESCONHECIDO: &str = "falante não identificado";

/// Fala citada por um item da ata, como a ferramenta `registrar_item_ata` manda.
/// Veja o [módulo](self).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PedidoCitacao {
    /// Índice da fala no transcript, a partir de 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub indice: Option<usize>,
    /// Falante, como aparece no transcript (ou com outra grafia que normalize
    /// para o mesmo nome: "Maria Silva" para "Silva, Maria").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub falante: Option<String>,
    /// Início da fala (`HH:MM:SS`), com precisão de segundo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inicio: Option<HorarioAta>,
    /// Trecho curto citado.
    pub trecho: String,
}

impl PedidoCitacao {
    /// Pedido pela posição da fala.
    pub fn por_indice(indice: usize, trecho: impl Into<String>) -> Self {
        Self {
            indice: Some(indice),
            trecho: trecho.into(),
            ..Self::default()
        }
    }

    /// Pedido pelo falante e pelo horário de início da fala.
    pub fn por_falante(
        falante: impl Into<String>,
        inicio: HorarioAta,
        trecho: impl Into<String>,
    ) -> Self {
        Self {
            falante: Some(falante.into()),
            inicio: Some(inicio),
            trecho: trecho.into(),
            ..Self::default()
        }
    }
}

/// Citação que conferiu: a fala encontrada e a citação pronta para a ata, com o
/// status [`StatusCitacao::Conferida`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CitacaoConferida {
    /// Índice da fala citada.
    pub indice: usize,
    pub citacao: CitacaoFala,
}

/// Motivo da recusa de uma citação. A mensagem é para o agente corrigir o
/// pedido, sem texto de fala.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RecusaCitacao {
    #[error("o trecho citado está vazio")]
    TrechoVazio,
    #[error("o trecho citado tem {caracteres} caracteres; cite um trecho curto, de até {limite}")]
    TrechoLongo { caracteres: usize, limite: usize },
    #[error("informe o índice da fala, ou o falante e o horário de início dela")]
    PedidoIncompleto,
    #[error("a fala {indice} não existe: o transcript tem {total} falas")]
    FalaInexistente { indice: usize, total: usize },
    #[error("a fala {indice} é de {falante_da_fala}, não de {falante}")]
    FalanteDiferente {
        indice: usize,
        falante: String,
        falante_da_fala: String,
    },
    #[error("a fala {indice} começa em {inicio_da_fala}, não em {inicio}")]
    HorarioDiferente {
        indice: usize,
        inicio: HorarioAta,
        inicio_da_fala: String,
    },
    #[error("nenhuma fala de {falante} começa em {inicio}; confira com ler_transcript")]
    FalaNaoEncontrada { falante: String, inicio: HorarioAta },
    #[error(
        "o trecho não aparece na fala {indice}: copie as palavras como estão na fala \
         (caixa, espaços, acentos e pontuação não importam, mas as palavras sim)"
    )]
    NaoConfere { indice: usize },
}

/// Confere o pedido contra as falas do transcript. Veja o [módulo](self).
pub fn conferir_citacao(
    falas: &[Fala],
    pedido: &PedidoCitacao,
) -> Result<CitacaoConferida, RecusaCitacao> {
    let trecho = em_uma_linha(&pedido.trecho);
    let caracteres = trecho.chars().count();
    if caracteres > LIMITE_DO_TRECHO {
        return Err(RecusaCitacao::TrechoLongo {
            caracteres,
            limite: LIMITE_DO_TRECHO,
        });
    }
    let palavras_do_trecho = palavras(&trecho);
    if palavras_do_trecho.is_empty() {
        return Err(RecusaCitacao::TrechoVazio);
    }
    let falante_pedido = pedido.falante.as_deref().filter(|f| !f.trim().is_empty());
    let candidatas: Vec<&Fala> = match pedido.indice {
        Some(indice) => {
            let fala = indice.checked_sub(1).and_then(|i| falas.get(i)).ok_or(
                RecusaCitacao::FalaInexistente {
                    indice,
                    total: falas.len(),
                },
            )?;
            if let Some(falante) = falante_pedido
                && !mesmo_falante(fala, falante)
            {
                return Err(RecusaCitacao::FalanteDiferente {
                    indice,
                    falante: falante.to_owned(),
                    falante_da_fala: nome_do_falante(fala).to_owned(),
                });
            }
            if let Some(inicio) = pedido.inicio
                && !mesmo_inicio(fala, inicio)
            {
                return Err(RecusaCitacao::HorarioDiferente {
                    indice,
                    inicio,
                    inicio_da_fala: fala
                        .inicio
                        .map_or_else(|| "horário desconhecido".into(), |h| h.to_string()),
                });
            }
            vec![fala]
        }
        None => {
            let (Some(falante), Some(inicio)) = (falante_pedido, pedido.inicio) else {
                return Err(RecusaCitacao::PedidoIncompleto);
            };
            let achadas: Vec<&Fala> = falas
                .iter()
                .filter(|f| mesmo_falante(f, falante) && mesmo_inicio(f, inicio))
                .collect();
            if achadas.is_empty() {
                return Err(RecusaCitacao::FalaNaoEncontrada {
                    falante: falante.to_owned(),
                    inicio,
                });
            }
            achadas
        }
    };
    let fala = candidatas
        .iter()
        .find(|f| contem_palavras(&palavras(&f.texto), &palavras_do_trecho))
        .ok_or(RecusaCitacao::NaoConfere {
            indice: candidatas[0].indice,
        })?;
    Ok(CitacaoConferida {
        indice: fala.indice,
        citacao: CitacaoFala::nova(
            nome_do_falante(fala),
            inicio_na_ata(fala),
            trecho,
            StatusCitacao::Conferida,
        ),
    })
}

/// Palavras do texto, normalizadas (caixa, acentos e pontuação).
pub(crate) fn palavras(texto: &str) -> Vec<String> {
    normalizar_texto(texto)
        .split(' ')
        .filter(|p| !p.is_empty())
        .map(str::to_owned)
        .collect()
}

/// `agulha` aparece em `palheiro` como sequência contígua de palavras inteiras.
pub(crate) fn contem_palavras(palheiro: &[String], agulha: &[String]) -> bool {
    !agulha.is_empty() && palheiro.windows(agulha.len()).any(|j| j == agulha)
}

fn mesmo_falante(fala: &Fala, falante: &str) -> bool {
    fala.falante
        .as_deref()
        .is_some_and(|f| normalizar_nome(f) == normalizar_nome(falante))
}

fn mesmo_inicio(fala: &Fala, inicio: HorarioAta) -> bool {
    fala.inicio
        .is_some_and(|h| h.segundos() == u64::from(inicio.segundos()))
}

fn nome_do_falante(fala: &Fala) -> &str {
    fala.falante
        .as_deref()
        .map(str::trim)
        .filter(|f| !f.is_empty())
        .unwrap_or(FALANTE_DESCONHECIDO)
}

/// Início da fala no formato da ata (segundos). Sem horário (texto não
/// reconhecido), fica `00:00:00`.
fn inicio_na_ata(fala: &Fala) -> HorarioAta {
    let segundos = fala.inicio.map_or(0, |h| h.segundos());
    HorarioAta::de_segundos(u32::try_from(segundos).unwrap_or(u32::MAX))
}

fn em_uma_linha(texto: &str) -> String {
    texto.split_whitespace().collect::<Vec<_>>().join(" ")
}

impl fmt::Display for CitacaoConferida {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "fala {} ({}, {})",
            self.indice, self.citacao.falante, self.citacao.inicio
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcript::Horario;

    fn fala(indice: usize, falante: Option<&str>, inicio_s: Option<u64>, texto: &str) -> Fala {
        Fala {
            indice,
            falante: falante.map(str::to_owned),
            inicio: inicio_s.map(|s| Horario::de_milissegundos(s * 1000 + 120)),
            fim: None,
            texto: texto.into(),
        }
    }

    fn falas() -> Vec<Fala> {
        vec![
            fala(
                1,
                Some("Silva, Maria"),
                Some(3),
                "Bom dia, pessoal. O assunto de hoje é o cancelamento de fatura.",
            ),
            fala(
                2,
                Some("João Pereira"),
                Some(30),
                "A trava dos 30 dias só existe na tela\nde retaguarda.",
            ),
        ]
    }

    fn h(s: &str) -> HorarioAta {
        HorarioAta::analisar(s).unwrap()
    }

    #[test]
    fn trecho_na_fala_confere_com_normalizacao() {
        let falas = falas();
        for trecho in [
            "o assunto de hoje",
            "  O   ASSUNTO de HOJE ",
            "assunto de hoje e o cancelamento",
            "Bom dia pessoal",
            "tela de retaguarda",
        ] {
            let indice = if trecho.contains("retaguarda") { 2 } else { 1 };
            let ok = conferir_citacao(&falas, &PedidoCitacao::por_indice(indice, trecho))
                .unwrap_or_else(|e| panic!("{trecho}: {e}"));
            assert_eq!(ok.indice, indice);
            assert_eq!(ok.citacao.status, StatusCitacao::Conferida);
        }
        let ok = conferir_citacao(
            &falas,
            &PedidoCitacao::por_indice(1, "  o  assunto\nde hoje"),
        )
        .unwrap();
        assert_eq!(ok.citacao.trecho, "o assunto de hoje");
        assert_eq!(ok.citacao.falante, "Silva, Maria");
        assert_eq!(ok.citacao.inicio, h("00:00:03"));
    }

    #[test]
    fn trecho_fora_da_fala_e_recusado() {
        let falas = falas();
        for trecho in [
            "o assunto de amanhã",
            "cancela",                // pedaço de palavra
            "hoje o assunto",         // fora de ordem
            "tela de retaguarda",     // está em outra fala
            "dia, pessoal. Obrigada", // palavra a mais
        ] {
            assert_eq!(
                conferir_citacao(&falas, &PedidoCitacao::por_indice(1, trecho)),
                Err(RecusaCitacao::NaoConfere { indice: 1 }),
                "{trecho}"
            );
        }
    }

    #[test]
    fn por_falante_e_horario() {
        let falas = falas();
        let ok = conferir_citacao(
            &falas,
            &PedidoCitacao::por_falante("Maria Silva", h("00:00:03"), "cancelamento de fatura"),
        )
        .unwrap();
        assert_eq!(ok.indice, 1);
        assert!(matches!(
            conferir_citacao(
                &falas,
                &PedidoCitacao::por_falante("Maria Silva", h("00:00:04"), "bom dia"),
            ),
            Err(RecusaCitacao::FalaNaoEncontrada { .. })
        ));
        let incompleto = PedidoCitacao {
            falante: Some("Maria Silva".into()),
            trecho: "bom dia".into(),
            ..Default::default()
        };
        assert_eq!(
            conferir_citacao(&falas, &incompleto),
            Err(RecusaCitacao::PedidoIncompleto)
        );
    }

    #[test]
    fn indice_com_falante_ou_horario_que_nao_batem() {
        let falas = falas();
        let mut p = PedidoCitacao::por_indice(1, "bom dia");
        p.falante = Some("João Pereira".into());
        let e = conferir_citacao(&falas, &p).unwrap_err();
        assert!(matches!(
            e,
            RecusaCitacao::FalanteDiferente { indice: 1, .. }
        ));
        assert!(e.to_string().contains("Silva, Maria"), "{e}");
        p.falante = Some("maria.silva@exemplo.com".into());
        p.inicio = Some(h("00:00:30"));
        assert!(matches!(
            conferir_citacao(&falas, &p),
            Err(RecusaCitacao::HorarioDiferente { indice: 1, .. })
        ));
        p.inicio = Some(h("00:00:03"));
        assert!(conferir_citacao(&falas, &p).is_ok());
        assert_eq!(
            conferir_citacao(&falas, &PedidoCitacao::por_indice(3, "x")),
            Err(RecusaCitacao::FalaInexistente {
                indice: 3,
                total: 2
            })
        );
        assert!(matches!(
            conferir_citacao(&falas, &PedidoCitacao::por_indice(0, "x")),
            Err(RecusaCitacao::FalaInexistente { indice: 0, .. })
        ));
    }

    #[test]
    fn trecho_vazio_ou_longo() {
        let falas = falas();
        for vazio in ["", "   ", "...", " — "] {
            assert_eq!(
                conferir_citacao(&falas, &PedidoCitacao::por_indice(1, vazio)),
                Err(RecusaCitacao::TrechoVazio),
                "{vazio:?}"
            );
        }
        let longo = "palavra ".repeat(40);
        assert!(matches!(
            conferir_citacao(&falas, &PedidoCitacao::por_indice(1, longo)),
            Err(RecusaCitacao::TrechoLongo { limite: 240, .. })
        ));
    }

    #[test]
    fn recusa_nao_traz_texto_da_fala_nem_trecho() {
        let falas = falas();
        let e = conferir_citacao(&falas, &PedidoCitacao::por_indice(1, "assunto de amanhã"))
            .unwrap_err()
            .to_string();
        assert!(!e.contains("assunto") && !e.contains("amanhã"), "{e}");
    }

    #[test]
    fn fala_sem_falante_nem_horario() {
        let falas = vec![fala(1, None, None, "Texto corrido sem formato.")];
        let ok = conferir_citacao(&falas, &PedidoCitacao::por_indice(1, "texto corrido")).unwrap();
        assert_eq!(ok.citacao.falante, FALANTE_DESCONHECIDO);
        assert_eq!(ok.citacao.inicio, h("00:00:00"));
    }

    #[test]
    fn pedido_em_json() {
        let p: PedidoCitacao =
            serde_json::from_str(r#"{"indice": 4, "inicio": "00:01:11", "trecho": "Isso."}"#)
                .unwrap();
        assert_eq!(p.indice, Some(4));
        assert_eq!(p.inicio, Some(h("00:01:11")));
        assert_eq!(p.falante, None);
    }
}
