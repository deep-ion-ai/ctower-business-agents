//! Reunião longa sintética, para medir o custo do fluxo `reuniao` numa reunião de
//! cerca de 2 h (questão 4 da #93).
//!
//! A reunião é a da fixture repetida em blocos, um depois do outro, com os horários
//! deslocados. A partir do segundo bloco, cada fala ganha antes uma ou mais frases de
//! enchimento (retomadas, problemas de áudio), sem conteúdo de negócio, até o total
//! de palavras chegar perto do alvo. O conteúdo de negócio é o mesmo da reunião
//! original, então a ata esperada é a mesma do gabarito; o texto de cada fala
//! original continua inteiro na fala repetida, e a comparação acha a fala esperada
//! pelo texto ([`crate::ata`]).
//!
//! A geração é determinística: as mesmas opções geram sempre o mesmo transcript.

use cba_reunioes::{Fala, Horario};
use serde::Serialize;

/// Frases de enchimento, sem decisão, prazo, número nem pergunta de negócio.
const ENCHIMENTO: [&str; 12] = [
    "Retomando o assunto para quem entrou agora na chamada.",
    "Deixa eu repetir, porque o áudio cortou para algumas pessoas.",
    "Só para ficar registrado de novo na gravação desta parte.",
    "Voltando um pouco ao que a gente conversou no começo.",
    "Peço desculpas, a conexão aqui caiu e eu perdi o fio.",
    "Vou falar mais devagar desta vez para todo mundo acompanhar.",
    "Alguém mais está ouvindo um eco ou é só aqui comigo?",
    "Acho que vale a pena passar por esse ponto mais uma vez.",
    "Enquanto o pessoal volta do intervalo, eu vou adiantando aqui.",
    "Desculpa, eu estava no mudo e falei sozinho por um tempo.",
    "A tela compartilhada travou, mas dá para seguir sem ela.",
    "Pelo que eu anotei no meu caderno, foi mais ou menos assim.",
];

/// Pausa entre um bloco e o próximo.
const PAUSA_MS: u64 = 10_000;

/// Tamanho da reunião longa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct OpcoesLonga {
    /// Duração aproximada, em minutos.
    pub minutos: u32,
    /// Palavras aproximadas nas falas.
    pub palavras: usize,
}

impl Default for OpcoesLonga {
    fn default() -> Self {
        Self {
            minutos: 120,
            palavras: 25_000,
        }
    }
}

/// Tamanho de um transcript, para o relatório (sem texto de fala).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct TamanhoTranscript {
    pub falas: usize,
    pub palavras: usize,
    /// Do começo ao fim da última fala, em segundos.
    pub duracao_s: u64,
}

impl TamanhoTranscript {
    pub fn de(falas: &[Fala]) -> Self {
        Self {
            falas: falas.len(),
            palavras: palavras(falas),
            duracao_s: fim_ms(falas) / 1000,
        }
    }
}

fn palavras(falas: &[Fala]) -> usize {
    falas
        .iter()
        .map(|f| f.texto.split_whitespace().count())
        .sum()
}

/// Fim da última fala (ou o início dela, se o formato não traz o fim).
fn fim_ms(falas: &[Fala]) -> u64 {
    falas
        .iter()
        .filter_map(|f| f.fim.or(f.inicio))
        .map(Horario::milissegundos)
        .max()
        .unwrap_or(0)
}

/// Gera a reunião longa a partir das falas da reunião base.
pub fn gerar(base: &[Fala], opcoes: OpcoesLonga) -> Vec<Fala> {
    if base.is_empty() {
        return Vec::new();
    }
    let bloco_ms = fim_ms(base) + PAUSA_MS;
    let alvo_ms = u64::from(opcoes.minutos) * 60_000;
    let blocos = alvo_ms.div_ceil(bloco_ms).max(1) as usize;
    let por_bloco = palavras(base);
    // Palavras de enchimento por fala repetida, para chegar ao alvo.
    let faltam = opcoes.palavras.saturating_sub(por_bloco * blocos);
    let repetidas = (blocos - 1) * base.len();
    let extra = if repetidas == 0 {
        0
    } else {
        faltam.div_ceil(repetidas)
    };

    let mut restante = faltam;
    let mut saida = Vec::with_capacity(blocos * base.len());
    for bloco in 0..blocos {
        let deslocamento = bloco as u64 * bloco_ms;
        for (j, fala) in base.iter().enumerate() {
            let mut texto = String::new();
            if bloco > 0 {
                let mut n = 0;
                let mut k = bloco * 7 + j;
                while n < extra && restante > 0 {
                    let frase = ENCHIMENTO[k % ENCHIMENTO.len()];
                    texto.push_str(frase);
                    texto.push(' ');
                    let palavras = frase.split_whitespace().count();
                    n += palavras;
                    restante = restante.saturating_sub(palavras);
                    k += 1;
                }
            }
            texto.push_str(&fala.texto);
            let mover = |h: Horario| Horario::de_milissegundos(h.milissegundos() + deslocamento);
            saida.push(Fala {
                indice: saida.len() + 1,
                falante: fala.falante.clone(),
                inicio: fala.inicio.map(mover),
                fim: fala.fim.map(mover),
                texto,
            });
        }
    }
    saida
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fala(indice: usize, falante: &str, inicio_s: u64, texto: &str) -> Fala {
        Fala {
            indice,
            falante: Some(falante.into()),
            inicio: Some(Horario::de_milissegundos(inicio_s * 1000)),
            fim: Some(Horario::de_milissegundos(inicio_s * 1000 + 9_000)),
            texto: texto.into(),
        }
    }

    #[test]
    fn repete_em_blocos_ate_o_tamanho_pedido() {
        let base = vec![
            fala(1, "Ana", 3, "Uma fala com seis palavras aqui."),
            fala(2, "João", 20, "Outra fala curta."),
        ];
        let longa = gerar(
            &base,
            OpcoesLonga {
                minutos: 2,
                palavras: 300,
            },
        );
        // Bloco de 29 s + 10 s de pausa: 4 blocos para 2 min.
        assert_eq!(longa.len(), 8);
        assert_eq!(
            longa.iter().map(|f| f.indice).collect::<Vec<_>>(),
            (1..=8).collect::<Vec<_>>()
        );
        assert_eq!(longa[0], base[0]);
        assert_eq!(longa[2].inicio.unwrap().segundos(), 42);
        assert!(longa[2].texto.ends_with(&base[0].texto));
        let tamanho = TamanhoTranscript::de(&longa);
        assert!(tamanho.palavras >= 300, "{tamanho:?}");
        assert!(tamanho.duracao_s >= 120, "{tamanho:?}");
        // Determinística.
        assert_eq!(
            longa,
            gerar(
                &base,
                OpcoesLonga {
                    minutos: 2,
                    palavras: 300
                }
            )
        );
        assert!(gerar(&[], OpcoesLonga::default()).is_empty());
    }
}
