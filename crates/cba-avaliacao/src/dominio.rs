//! Comparação dos valores de domínio usados como critério da correção (#455).
//!
//! Quando a correção depende de um subconjunto dos valores de um enum, de uma
//! tabela de domínio ou de constantes ("cancelar quando a fatura estiver em X ou
//! Y"), a análise de impacto deve citar a lista completa e classificar cada valor
//! como entra, não entra ou a decidir; os valores a decidir viram decisões para o
//! negócio. Determinística, como o resto da comparação. Só contam as afirmações
//! que não foram marcadas como incorretas:
//!
//! - **lista completa:** uma afirmação cita todos os valores esperados. O valor
//!   casa com a palavra inteira, com as maiúsculas do código (`PAGA` não casa com
//!   "paga", nem com `PAGAMENTO`). O relatório diz também se essa afirmação tem
//!   evidência válida que toca a definição esperada do domínio;
//! - **classificação:** no formato do roteiro, "<VALOR>: entra | não entra | a
//!   decidir (<motivo>)", a classificação de um valor é o primeiro termo de
//!   classificação depois dele, até o próximo valor esperado citado. Vale a
//!   primeira afirmação que classifica o valor;
//! - **valor a decidir:** precisa também de uma pergunta do tipo decisão, não
//!   descartada, com ao menos duas opções, que cita o valor no texto ou nas
//!   opções.
//!
//! Completo quando há a lista e cada valor está classificado como o esperado (e,
//! a decidir, com a decisão).

use std::collections::BTreeMap;

use cba_core::analise::{
    Afirmacao, Analise, EstadoAfirmacao, EstadoPergunta, Evidencia, MIN_OPCOES,
};
use serde::Serialize;

use crate::caminhos::{texto_do_agente, toca};
use crate::gabarito::{Classificacao, ValoresDeDominioEsperados};
use crate::texto;

/// Termos de cada classificação, já normalizados (sem acentos, minúsculas). Vale o
/// que aparece primeiro depois do valor: "não entra" começa antes do "entra"
/// contido nele.
const TERMOS: [(Classificacao, &[&str]); 3] = [
    (
        Classificacao::ADecidir,
        &[
            "a decidir",
            "a definir",
            "para decidir",
            "decisao do negocio",
        ],
    ),
    (
        Classificacao::NaoEntra,
        &[
            "nao entra",
            "nao entram",
            "fica fora",
            "ficam fora",
            "fica de fora",
            "ficam de fora",
            "excluido",
            "excluida",
        ],
    ),
    (
        Classificacao::Entra,
        &["entra", "entram", "incluido", "incluida"],
    ),
];

/// Resultado da comparação dos valores de domínio.
#[derive(Debug, Clone, Serialize)]
pub struct ValoresAvaliados {
    pub campo: String,
    pub valores: Vec<ValorAvaliado>,
    /// Primeira afirmação que cita todos os valores esperados.
    pub lista_em: Option<u32>,
    /// A afirmação da lista tem evidência válida que toca a definição esperada.
    pub lista_com_evidencia: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ValorAvaliado {
    pub valor: String,
    pub esperada: Classificacao,
    /// Classificação dada pela análise, se alguma.
    pub classificado_como: Option<Classificacao>,
    /// Afirmação que classificou o valor.
    pub classificado_em: Option<u32>,
    /// Valor a decidir: a pergunta do tipo decisão que o cita. Sempre `None` nos
    /// outros.
    pub decisao_em: Option<u32>,
}

impl ValorAvaliado {
    /// Classificado como o esperado e, se a decidir, com a decisão.
    pub fn atendido(&self) -> bool {
        self.classificado_como == Some(self.esperada)
            && (self.esperada != Classificacao::ADecidir || self.decisao_em.is_some())
    }
}

impl ValoresAvaliados {
    /// Valores classificados como o esperado.
    pub fn classificados(&self) -> usize {
        self.valores
            .iter()
            .filter(|v| v.classificado_como == Some(v.esperada))
            .count()
    }

    /// A lista completa citada e cada valor atendido.
    pub fn completos(&self) -> bool {
        self.lista_em.is_some() && self.valores.iter().all(ValorAvaliado::atendido)
    }
}

fn parte_de_palavra(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Posições (início, fim) do valor como palavra inteira, com as maiúsculas do
/// código.
fn ocorrencias(texto: &str, valor: &str) -> Vec<(usize, usize)> {
    if valor.is_empty() {
        return Vec::new();
    }
    texto
        .match_indices(valor)
        .filter(|(i, _)| {
            let antes = texto[..*i].chars().next_back();
            let depois = texto[i + valor.len()..].chars().next();
            !antes.is_some_and(parte_de_palavra) && !depois.is_some_and(parte_de_palavra)
        })
        .map(|(i, _)| (i, i + valor.len()))
        .collect()
}

/// Primeiro termo de classificação no trecho, como palavra inteira.
fn classificacao_do_trecho(trecho: &str) -> Option<Classificacao> {
    let normalizado = texto::normalizar(trecho);
    TERMOS
        .iter()
        .filter_map(|(c, termos)| {
            termos
                .iter()
                .filter_map(|t| ocorrencias(&normalizado, t).first().map(|(i, _)| *i))
                .min()
                .map(|i| (i, *c))
        })
        .min_by_key(|(i, _)| *i)
        .map(|(_, c)| c)
}

/// Classificação de cada valor no texto: o primeiro termo depois de uma
/// ocorrência do valor, até a próxima ocorrência de qualquer valor.
fn classificacoes(texto: &str, valores: &[&str]) -> BTreeMap<usize, Classificacao> {
    let mut todas: Vec<(usize, usize, usize)> = valores
        .iter()
        .enumerate()
        .flat_map(|(k, v)| {
            ocorrencias(texto, v)
                .into_iter()
                .map(move |(i, f)| (i, f, k))
        })
        .collect();
    todas.sort_unstable();
    let mut achadas = BTreeMap::new();
    for (n, (_, fim, k)) in todas.iter().enumerate() {
        if achadas.contains_key(k) {
            continue;
        }
        let ate = todas
            .get(n + 1)
            .map_or(texto.len(), |(inicio, _, _)| *inicio)
            .max(*fim);
        if let Some(c) = classificacao_do_trecho(&texto[*fim..ate]) {
            achadas.insert(*k, c);
        }
    }
    achadas
}

/// Compara as afirmações e as decisões com os valores esperados. `validas` são as
/// evidências válidas de cada afirmação não incorreta, já conferidas contra o
/// código.
pub fn avaliar_valores(
    analise: &Analise,
    esperado: &ValoresDeDominioEsperados,
    validas: &BTreeMap<u32, Vec<&Evidencia>>,
) -> ValoresAvaliados {
    let afirmacoes: Vec<&Afirmacao> = analise
        .afirmacoes
        .iter()
        .filter(|a| !matches!(a.estado, EstadoAfirmacao::Incorreta { .. }))
        .collect();
    let nomes: Vec<&str> = esperado.valores.iter().map(|v| v.valor.as_str()).collect();
    let mut classificados: BTreeMap<usize, (Classificacao, u32)> = BTreeMap::new();
    for a in &afirmacoes {
        for (k, c) in classificacoes(texto_do_agente(a), &nomes) {
            classificados.entry(k).or_insert((c, a.numero));
        }
    }
    let decisoes: Vec<_> = analise
        .perguntas
        .iter()
        .filter(|p| p.estado != EstadoPergunta::Descartada)
        .filter(|p| p.e_decisao() && p.opcoes.len() >= MIN_OPCOES)
        .collect();
    let valores = esperado
        .valores
        .iter()
        .enumerate()
        .map(|(k, v)| {
            let (classificado_como, classificado_em) = classificados
                .get(&k)
                .map_or((None, None), |(c, n)| (Some(*c), Some(*n)));
            let decisao_em = if v.classificacao == Classificacao::ADecidir {
                decisoes
                    .iter()
                    .find(|p| {
                        let mut conteudo = p.texto.clone();
                        for o in &p.opcoes {
                            conteudo.push('\n');
                            conteudo.push_str(&o.texto);
                            conteudo.push('\n');
                            conteudo.push_str(&o.efeito);
                        }
                        texto::contem(&texto::normalizar(&conteudo), &v.valor)
                    })
                    .map(|p| p.numero)
            } else {
                None
            };
            ValorAvaliado {
                valor: v.valor.clone(),
                esperada: v.classificacao,
                classificado_como,
                classificado_em,
                decisao_em,
            }
        })
        .collect();
    let lista = afirmacoes.iter().find(|a| {
        !nomes.is_empty()
            && nomes
                .iter()
                .all(|v| !ocorrencias(texto_do_agente(a), v).is_empty())
    });
    let lista_com_evidencia = lista.is_some_and(|a| {
        validas.get(&a.numero).is_some_and(|ev| {
            ev.iter()
                .any(|e| esperado.evidencias.iter().any(|esp| toca(e, esp)))
        })
    });
    ValoresAvaliados {
        campo: esperado.campo.clone(),
        valores,
        lista_em: lista.map(|a| a.numero),
        lista_com_evidencia,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALORES: [&str; 4] = ["ABERTA", "PAGA", "SUSPENSA", "PROTESTADA"];

    #[test]
    fn classifica_cada_valor_pelo_termo_que_vem_depois() {
        let c = classificacoes(
            "Valores de FATURA.STATUS: ABERTA: entra (hoje); PAGA: não entra, já quitada; \
             SUSPENSA: a decidir (contestação); PROTESTADA: fica de fora.",
            &VALORES,
        );
        assert_eq!(c[&0], Classificacao::Entra);
        assert_eq!(c[&1], Classificacao::NaoEntra);
        assert_eq!(c[&2], Classificacao::ADecidir);
        assert_eq!(c[&3], Classificacao::NaoEntra);
    }

    #[test]
    fn valor_com_as_maiusculas_do_codigo_e_palavra_inteira() {
        // "paga" minúsculo e PAGAMENTO não são o valor PAGA.
        let c = classificacoes("A fatura paga entra no PAGAMENTO; ABERTA: entra.", &VALORES);
        assert_eq!(c.get(&1), None);
        assert_eq!(c[&0], Classificacao::Entra);
    }

    #[test]
    fn sem_termo_depois_do_valor_nao_classifica() {
        // A classificação antes do valor (agrupada) não segue o formato.
        let c = classificacoes("Entram ABERTA e PAGA.", &VALORES);
        assert!(c.is_empty(), "{c:?}");
        // "entrada" não é "entra".
        assert!(classificacao_do_trecho(": entrada de dados").is_none());
    }
}
