//! Comparação do ciclo de vida de uma entidade (#119) com o gabarito.
//!
//! A análise é do fluxo explicar; o que se avalia é o diagrama de estados
//! (`registrar_diagrama`, tipo `estados`) que o agente registrou. Entre os
//! diagramas de estados da análise, vale o que tem mais estados do gabarito.
//!
//! - **Estado** esperado: um nó cuja chave é o valor do código (sem diferença de
//!   maiúsculas nem de espaços nas pontas). O inicial e os finais esperados
//!   conferem quando o papel do nó é `inicio` ou `fim`.
//! - **Transição** esperada: uma aresta entre os mesmos estados; ela é
//!   **sustentada** quando alguma evidência dela toca a evidência esperada
//!   (mesmo arquivo e linhas que se cruzam).
//! - **Achado** esperado (`inalcancavel`, `sem_saida`): o app o aponta no diagrama
//!   registrado ([`cba_core::diagramas::Diagrama::achados`]).
//!
//! Falha grave: nenhum diagrama de estados registrado.

use cba_core::analise::{Analise, Evidencia};
use cba_core::diagramas::{Diagrama, PapelNo, TipoAchadoDiagrama, TipoDiagrama};
use serde::Serialize;

use crate::comparacao::fracao;
use crate::gabarito::{CicloVidaEsperado, EvidenciaEsperada};

/// Resultado da comparação do ciclo de vida.
#[derive(Debug, Clone, Serialize)]
pub struct CicloVidaAvaliado {
    /// Diagrama comparado (`D-n`), se houver diagrama de estados.
    pub diagrama: Option<String>,
    pub estados: Vec<ElementoAvaliado>,
    /// Estados registrados que o gabarito não tem.
    pub estados_extras: Vec<String>,
    /// O estado inicial esperado está marcado como `inicio`.
    pub inicial_confere: bool,
    /// Estados finais esperados que não estão marcados como `fim`.
    pub finais_sem_marca: Vec<String>,
    pub transicoes: Vec<TransicaoAvaliada>,
    pub achados: Vec<ElementoAvaliado>,
}

/// Estado ou achado esperado e se foi encontrado.
#[derive(Debug, Clone, Serialize)]
pub struct ElementoAvaliado {
    pub esperado: String,
    pub encontrado: bool,
}

/// Transição esperada.
#[derive(Debug, Clone, Serialize)]
pub struct TransicaoAvaliada {
    pub id: String,
    /// `ABERTA → PAGA`.
    pub transicao: String,
    pub encontrada: bool,
    /// Alguma evidência da aresta toca a evidência esperada.
    pub sustentada: bool,
}

impl CicloVidaAvaliado {
    /// Falhas graves.
    pub fn falhas(&self) -> Vec<String> {
        if self.diagrama.is_none() {
            vec!["nenhum diagrama de estados registrado".to_owned()]
        } else {
            Vec::new()
        }
    }

    /// Fração dos estados esperados encontrados.
    pub fn cobertura_de_estados(&self) -> f64 {
        contar(&self.estados)
    }

    /// Fração das transições esperadas encontradas e sustentadas pela evidência.
    pub fn cobertura_de_transicoes(&self) -> f64 {
        fracao(
            self.transicoes
                .iter()
                .filter(|t| t.encontrada && t.sustentada)
                .count(),
            self.transicoes.len(),
        )
    }

    /// Fração dos achados esperados apontados.
    pub fn cobertura_de_achados(&self) -> f64 {
        contar(&self.achados)
    }
}

fn contar(lista: &[ElementoAvaliado]) -> f64 {
    fracao(lista.iter().filter(|e| e.encontrado).count(), lista.len())
}

fn mesma_chave(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

fn toca(e: &Evidencia, esperada: &EvidenciaEsperada) -> bool {
    e.arquivo.as_str() == esperada.arquivo
        && e.linhas.inicio <= esperada.linha_final
        && esperada.linha_inicial <= e.linhas.fim
}

fn tipo_do_achado(tipo: &str) -> Option<TipoAchadoDiagrama> {
    match tipo {
        "inalcancavel" => Some(TipoAchadoDiagrama::Inalcancavel),
        "sem_saida" => Some(TipoAchadoDiagrama::SemSaida),
        "sem_condicao" => Some(TipoAchadoDiagrama::SemCondicao),
        _ => None,
    }
}

/// Compara os diagramas de estados da análise com o ciclo de vida esperado.
pub fn avaliar_ciclo_vida(analise: &Analise, esperado: &CicloVidaEsperado) -> CicloVidaAvaliado {
    let acertos = |d: &Diagrama| {
        esperado
            .estados
            .iter()
            .filter(|e| d.nos.iter().any(|n| mesma_chave(&n.chave, &e.valor)))
            .count()
    };
    // O primeiro dos que têm mais estados do gabarito.
    let diagrama = analise
        .diagramas
        .iter()
        .filter(|d| d.tipo == TipoDiagrama::Estados)
        .rev()
        .max_by_key(|d| acertos(d));
    let Some(d) = diagrama else {
        return CicloVidaAvaliado {
            diagrama: None,
            estados: esperado
                .estados
                .iter()
                .map(|e| ElementoAvaliado {
                    esperado: e.valor.clone(),
                    encontrado: false,
                })
                .collect(),
            estados_extras: Vec::new(),
            inicial_confere: false,
            finais_sem_marca: Vec::new(),
            transicoes: esperado
                .transicoes
                .iter()
                .map(|t| TransicaoAvaliada {
                    id: t.id.clone(),
                    transicao: format!("{} → {}", t.de, t.para),
                    encontrada: false,
                    sustentada: false,
                })
                .collect(),
            achados: esperado
                .achados
                .iter()
                .map(|a| ElementoAvaliado {
                    esperado: format!("{}:{}", a.tipo, a.estado),
                    encontrado: false,
                })
                .collect(),
        };
    };
    let no = |valor: &str| d.nos.iter().find(|n| mesma_chave(&n.chave, valor));
    let estados = esperado
        .estados
        .iter()
        .map(|e| ElementoAvaliado {
            esperado: e.valor.clone(),
            encontrado: no(&e.valor).is_some(),
        })
        .collect();
    let estados_extras = d
        .nos
        .iter()
        .filter(|n| {
            !esperado
                .estados
                .iter()
                .any(|e| mesma_chave(&n.chave, &e.valor))
        })
        .map(|n| n.chave.clone())
        .collect();
    let inicial_confere = esperado
        .estados
        .iter()
        .filter(|e| e.inicial)
        .all(|e| no(&e.valor).is_some_and(|n| n.papel == PapelNo::Inicio));
    let finais_sem_marca = esperado
        .estados
        .iter()
        .filter(|e| e.final_)
        .filter(|e| !no(&e.valor).is_some_and(|n| n.papel == PapelNo::Fim))
        .map(|e| e.valor.clone())
        .collect();
    let transicoes = esperado
        .transicoes
        .iter()
        .map(|t| {
            let arestas: Vec<_> = d
                .arestas
                .iter()
                .filter(|a| mesma_chave(&a.de, &t.de) && mesma_chave(&a.para, &t.para))
                .collect();
            TransicaoAvaliada {
                id: t.id.clone(),
                transicao: format!("{} → {}", t.de, t.para),
                encontrada: !arestas.is_empty(),
                sustentada: arestas
                    .iter()
                    .any(|a| a.evidencias.iter().any(|e| toca(e, &t.evidencia))),
            }
        })
        .collect();
    let apontados = d.achados();
    let achados = esperado
        .achados
        .iter()
        .map(|a| ElementoAvaliado {
            esperado: format!("{}:{}", a.tipo, a.estado),
            encontrado: tipo_do_achado(&a.tipo).is_some_and(|tipo| {
                apontados.iter().any(|x| {
                    x.tipo == tipo
                        && x.estado
                            .as_deref()
                            .is_some_and(|e| mesma_chave(e, &a.estado))
                })
            }),
        })
        .collect();
    CicloVidaAvaliado {
        diagrama: Some(d.rotulo()),
        estados,
        estados_extras,
        inicial_confere,
        finais_sem_marca,
        transicoes,
        achados,
    }
}
