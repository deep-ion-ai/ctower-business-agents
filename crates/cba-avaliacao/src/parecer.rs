//! Comparação do parecer de revisão (fluxo `revisar`, #116) com o gabarito.
//!
//! Determinística, como o resto da comparação. Para cada revisão do gabarito
//! (`revisoes`), mede:
//!
//! - **evidências dos achados:** cada uma conferida de novo contra a fixture, como
//!   as das afirmações; achado de correção sem evidência válida é falha grave;
//! - **achados esperados:** um achado ativo atende quando é da característica
//!   esperada (ou de uma das `tambem_em`), tem um termo de cada grupo de
//!   palavras-chave no trecho, no problema ou na reescrita e, se o esperado cita
//!   regras, tem evidência válida que toca uma evidência esperada de uma delas (ou
//!   cita a regra pelo id). A contradição plantada é um achado esperado de
//!   correção;
//! - **termos vagos apontados:** cada termo esperado da lista de referência tem
//!   um achado ativo de ambiguidade ou de verificabilidade cujo trecho o contém;
//! - **veredito:** o registrado é o esperado.

use cba_core::analise::{Analise, Evidencia};
use cba_core::parecer::{Achado, Veredito};
use cba_core::qualidade::{self, Caracteristica};
use cba_index::Escopo;
use serde::Serialize;

use crate::comparacao::{EvidenciaInvalida, conferir_evidencia, toca};
use crate::gabarito::{Gabarito, RevisaoEsperada};
use crate::texto;

/// Resultado da comparação do parecer.
#[derive(Debug, Clone, Serialize)]
pub struct ParecerAvaliado {
    pub veredito_esperado: Veredito,
    pub veredito_registrado: Option<Veredito>,
    pub achados_registrados: usize,
    pub achados: Vec<AchadoAvaliado>,
    pub termos_vagos: Vec<TermoAvaliado>,
    pub evidencias: usize,
    pub evidencias_invalidas: Vec<EvidenciaInvalida>,
    /// Achados de correção sem evidência ou com alguma evidência inválida.
    pub correcao_sem_evidencia_valida: Vec<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AchadoAvaliado {
    pub id: String,
    pub descricao: String,
    /// Número do primeiro achado registrado que atende.
    pub atendido_por: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TermoAvaliado {
    pub termo: String,
    pub apontado_por: Option<u32>,
}

impl ParecerAvaliado {
    /// Falhas graves do parecer.
    pub fn falhas(&self) -> Vec<String> {
        let mut f = Vec::new();
        if self.achados_registrados == 0 && self.veredito_esperado != Veredito::Pronta {
            f.push("nenhum achado registrado".to_owned());
        }
        if !self.correcao_sem_evidencia_valida.is_empty() {
            f.push(format!(
                "achados de correção sem evidência válida: {}",
                crate::comparacao::lista_de_numeros(&self.correcao_sem_evidencia_valida)
            ));
        }
        f
    }

    pub fn veredito_confere(&self) -> bool {
        self.veredito_registrado == Some(self.veredito_esperado)
    }

    pub fn achados_atendidos(&self) -> usize {
        self.achados
            .iter()
            .filter(|a| a.atendido_por.is_some())
            .count()
    }

    pub fn termos_apontados(&self) -> usize {
        self.termos_vagos
            .iter()
            .filter(|t| t.apontado_por.is_some())
            .count()
    }
}

/// Compara o parecer da análise com o esperado.
pub fn avaliar_parecer(
    analise: &Analise,
    esperado: &RevisaoEsperada,
    gabarito: &Gabarito,
    escopo: &Escopo,
) -> ParecerAvaliado {
    let parecer = analise.parecer.clone().unwrap_or_default();
    let ativos: Vec<&Achado> = parecer.ativos().collect();
    let mut evidencias = 0;
    let mut invalidas = Vec::new();
    let mut correcao_sem = Vec::new();
    // Evidências válidas por achado ativo.
    let mut validas: Vec<(&Achado, Vec<&Evidencia>)> = Vec::new();
    for a in &parecer.achados {
        let mut daqui = Vec::new();
        let mut todas = true;
        for e in &a.evidencias {
            evidencias += 1;
            match conferir_evidencia(escopo, e) {
                Ok(()) => daqui.push(e),
                Err(motivo) => {
                    todas = false;
                    invalidas.push(EvidenciaInvalida {
                        afirmacao: a.numero,
                        arquivo: e.arquivo.as_str().to_owned(),
                        linhas: format!("{}-{}", e.linhas.inicio, e.linhas.fim),
                        motivo,
                    });
                }
            }
        }
        if a.caracteristica == Caracteristica::Correcao && (a.evidencias.is_empty() || !todas) {
            correcao_sem.push(a.numero);
        }
        if a.ativo() {
            validas.push((a, daqui));
        }
    }

    let achados = esperado
        .achados
        .iter()
        .map(|e| {
            let atendido_por = validas
                .iter()
                .find(|(a, ev)| {
                    let caracteristica = a.caracteristica == e.caracteristica
                        || e.tambem_em.contains(&a.caracteristica);
                    let texto = format!(
                        "{} {} {}",
                        a.trecho,
                        a.problema,
                        a.reescrita.as_deref().unwrap_or_default()
                    );
                    let regras = e.regras.is_empty()
                        || e.regras.iter().any(|id| {
                            a.regras.iter().any(|r| r.to_string() == *id)
                                || gabarito.regra(id).is_some_and(|r| toca(r, ev))
                        });
                    caracteristica && regras && texto::atende(&texto, &e.palavras_chave)
                })
                .map(|(a, _)| a.numero);
            AchadoAvaliado {
                id: e.id.clone(),
                descricao: e.descricao.clone(),
                atendido_por,
            }
        })
        .collect();

    let termos_vagos = esperado
        .termos_vagos
        .iter()
        .map(|termo| TermoAvaliado {
            termo: termo.clone(),
            apontado_por: ativos
                .iter()
                .filter(|a| {
                    matches!(
                        a.caracteristica,
                        Caracteristica::Ambiguidade | Caracteristica::Verificabilidade
                    )
                })
                .find(|a| {
                    qualidade::termos_vagos(&a.trecho)
                        .iter()
                        .any(|o| o.termo == termo.as_str())
                })
                .map(|a| a.numero),
        })
        .collect();

    ParecerAvaliado {
        veredito_esperado: esperado.veredito,
        veredito_registrado: parecer.veredito.as_ref().map(|v| v.veredito),
        achados_registrados: ativos.len(),
        achados,
        termos_vagos,
        evidencias,
        evidencias_invalidas: invalidas,
        correcao_sem_evidencia_valida: correcao_sem,
    }
}
