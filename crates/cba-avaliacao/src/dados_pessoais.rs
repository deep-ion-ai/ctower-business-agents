//! Comparação da seção "Dados pessoais e requisitos não funcionais" da análise de
//! impacto com o gabarito (#126).
//!
//! Determinística, como o resto da comparação. Só contam as afirmações da seção
//! `dados_pessoais_e_rnf` que não foram marcadas como incorretas:
//!
//! - **campos pessoais:** um campo esperado é apontado quando uma afirmação da
//!   seção tem as palavras-chave do campo e uma evidência válida, de até
//!   [`LINHAS_PARA_CASAR`] linhas, que toca uma das evidências esperadas dele (a
//!   definição da coluna ou o conteúdo gravado);
//! - **falsos positivos:** uma coluna que não guarda dado pessoal é apontada por
//!   engano quando uma afirmação da seção cita a coluna e não aponta nenhum dos
//!   campos esperados. É um indício para quem revisa o relatório, não falha grave:
//!   o texto pode citar a coluna para dizer que ela não é pessoal;
//! - **aspectos não funcionais** (auditoria, desempenho, acesso): atendidos por
//!   uma afirmação da seção ou por uma pergunta ao solicitante com as
//!   palavras-chave. Aspecto sem evidência no código deve virar pergunta.

use std::collections::BTreeMap;

use cba_core::analise::{Analise, EstadoAfirmacao, Evidencia};
use cba_docs::DADOS_PESSOAIS_E_RNF;
use serde::Serialize;

use crate::gabarito::{DadosPessoaisEsperados, EvidenciaEsperada};
use crate::texto;

/// Evidências mais largas que isso não contam para casar com as esperadas.
pub const LINHAS_PARA_CASAR: u32 = 30;

/// Resultado da comparação da seção.
#[derive(Debug, Clone, Serialize)]
pub struct DadosPessoaisAvaliados {
    /// Afirmações registradas na seção.
    pub afirmacoes: usize,
    pub campos: Vec<CampoAvaliado>,
    pub falsos_positivos: Vec<FalsoPositivo>,
    pub aspectos: Vec<AspectoAvaliado>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CampoAvaliado {
    pub campo: String,
    pub categoria: String,
    /// Afirmação que aponta o campo com evidência.
    pub apontado_por: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FalsoPositivo {
    pub campo: String,
    pub afirmacoes: Vec<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AspectoAvaliado {
    pub aspecto: String,
    pub tema: String,
    /// Afirmação da seção que trata o aspecto.
    pub afirmacao: Option<u32>,
    /// Pergunta ao solicitante que trata o aspecto.
    pub pergunta: Option<u32>,
}

impl AspectoAvaliado {
    pub fn atendido(&self) -> bool {
        self.afirmacao.is_some() || self.pergunta.is_some()
    }
}

impl DadosPessoaisAvaliados {
    /// Campos pessoais apontados com evidência.
    pub fn campos_apontados(&self) -> usize {
        self.campos
            .iter()
            .filter(|c| c.apontado_por.is_some())
            .count()
    }

    /// Aspectos não funcionais tratados.
    pub fn aspectos_atendidos(&self) -> usize {
        self.aspectos.iter().filter(|a| a.atendido()).count()
    }
}

fn toca(e: &Evidencia, esperada: &EvidenciaEsperada) -> bool {
    e.arquivo.as_str() == esperada.arquivo
        && e.linhas.fim - e.linhas.inicio < LINHAS_PARA_CASAR
        && e.linhas.inicio <= esperada.linha_final
        && esperada.linha_inicial <= e.linhas.fim
}

/// Compara a seção com o esperado. `validas` são as evidências válidas de cada
/// afirmação não incorreta, já conferidas contra o código.
pub fn avaliar_dados_pessoais(
    analise: &Analise,
    esperado: &DadosPessoaisEsperados,
    validas: &BTreeMap<u32, Vec<&Evidencia>>,
) -> DadosPessoaisAvaliados {
    let da_secao: Vec<(u32, &str, &[&Evidencia])> = analise
        .afirmacoes
        .iter()
        .filter(|a| a.secao == DADOS_PESSOAIS_E_RNF)
        .filter(|a| !matches!(a.estado, EstadoAfirmacao::Incorreta { .. }))
        .map(|a| {
            let texto = a.texto_original.as_deref().unwrap_or(&a.texto);
            let ev = validas.get(&a.numero).map_or(&[][..], Vec::as_slice);
            (a.numero, texto, ev)
        })
        .collect();

    let aponta = |campo: &crate::gabarito::CampoPessoal, texto: &str, ev: &[&Evidencia]| {
        texto::atende(texto, &campo.palavras_chave)
            && ev
                .iter()
                .any(|e| campo.evidencias.iter().any(|esp| toca(e, esp)))
    };
    let campos: Vec<CampoAvaliado> = esperado
        .campos
        .iter()
        .map(|campo| CampoAvaliado {
            campo: campo.campo.clone(),
            categoria: campo.categoria.clone(),
            apontado_por: da_secao
                .iter()
                .find(|(_, texto, ev)| aponta(campo, texto, ev))
                .map(|(n, ..)| *n),
        })
        .collect();

    let falsos_positivos = esperado
        .nao_pessoais
        .iter()
        .filter_map(|np| {
            let afirmacoes: Vec<u32> = da_secao
                .iter()
                .filter(|(_, texto, ev)| {
                    texto::atende(texto, &np.palavras_chave)
                        && !esperado.campos.iter().any(|c| aponta(c, texto, ev))
                })
                .map(|(n, ..)| *n)
                .collect();
            (!afirmacoes.is_empty()).then(|| FalsoPositivo {
                campo: np.campo.clone(),
                afirmacoes,
            })
        })
        .collect();

    let aspectos = esperado
        .aspectos
        .iter()
        .map(|a| AspectoAvaliado {
            aspecto: a.aspecto.clone(),
            tema: a.tema.clone(),
            afirmacao: da_secao
                .iter()
                .find(|(_, texto, _)| texto::atende(texto, &a.palavras_chave))
                .map(|(n, ..)| *n),
            pergunta: analise
                .perguntas
                .iter()
                .find(|p| texto::atende(&p.texto, &a.palavras_chave))
                .map(|p| p.numero),
        })
        .collect();

    DadosPessoaisAvaliados {
        afirmacoes: da_secao.len(),
        campos,
        falsos_positivos,
        aspectos,
    }
}
