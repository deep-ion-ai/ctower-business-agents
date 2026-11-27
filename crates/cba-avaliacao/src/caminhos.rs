//! Comparação dos caminhos que gravam um estado com o gabarito (#451).
//!
//! Numa demanda sobre um estado errado (status, situação, flag), a análise de
//! impacto deve fechar o conjunto de quem grava o estado, não só a rota que a tela
//! usa. Determinística, como o resto da comparação. Só contam as afirmações que não
//! foram marcadas como incorretas:
//!
//! - **caminho citado:** uma afirmação (de qualquer seção) tem as palavras-chave do
//!   caminho ou uma evidência válida, de até
//!   [`crate::dados_pessoais::LINHAS_PARA_CASAR`] linhas, que toca uma das
//!   evidências esperadas dele;
//! - **o que não valida apontado:** o caminho sem validação aparece numa afirmação
//!   de `riscos` ou de `funcionalidades` que o cita (como acima) e tem as
//!   palavras-chave de `sem_validacao` (o que não é validado);
//! - **lista:** uma afirmação cita todos os caminhos esperados. É informativo: a
//!   lista pode estar espalhada em afirmações de grupo.

use std::collections::BTreeMap;

use cba_core::analise::{Afirmacao, Analise, EstadoAfirmacao, Evidencia};
use serde::Serialize;

use crate::dados_pessoais::LINHAS_PARA_CASAR;
use crate::gabarito::{CaminhoEsperado, CaminhosEsperados, EvidenciaEsperada};
use crate::texto;

/// Seções em que o caminho sem validação deve virar item (#451).
pub const SECOES_DO_CAMINHO_SEM_VALIDACAO: [&str; 2] = ["riscos", "funcionalidades"];

/// Resultado da comparação dos caminhos.
#[derive(Debug, Clone, Serialize)]
pub struct CaminhosAvaliados {
    pub estado: String,
    pub caminhos: Vec<CaminhoAvaliado>,
    /// Primeira afirmação que cita todos os caminhos esperados.
    pub lista_em: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CaminhoAvaliado {
    pub nome: String,
    pub valida: bool,
    /// Primeira afirmação que cita o caminho.
    pub citado_em: Option<u32>,
    /// Caminho sem validação: a afirmação de `riscos` ou `funcionalidades` que diz
    /// o que ele não valida. Sempre `None` no caminho que valida.
    pub sem_validacao_em: Option<u32>,
}

impl CaminhoAvaliado {
    /// Citado e, se não valida, apontado como tal.
    pub fn atendido(&self) -> bool {
        self.citado_em.is_some() && (self.valida || self.sem_validacao_em.is_some())
    }
}

impl CaminhosAvaliados {
    /// Caminhos citados.
    pub fn citados(&self) -> usize {
        self.caminhos
            .iter()
            .filter(|c| c.citado_em.is_some())
            .count()
    }

    /// Todos os caminhos citados e os sem validação apontados.
    pub fn completos(&self) -> bool {
        self.caminhos.iter().all(CaminhoAvaliado::atendido)
    }
}

pub(crate) fn toca(e: &Evidencia, esperada: &EvidenciaEsperada) -> bool {
    e.arquivo.as_str() == esperada.arquivo
        && e.linhas.fim - e.linhas.inicio < LINHAS_PARA_CASAR
        && e.linhas.inicio <= esperada.linha_final
        && esperada.linha_inicial <= e.linhas.fim
}

pub(crate) fn texto_do_agente(a: &Afirmacao) -> &str {
    a.texto_original.as_deref().unwrap_or(&a.texto)
}

/// Compara as afirmações com os caminhos esperados. `validas` são as evidências
/// válidas de cada afirmação não incorreta, já conferidas contra o código.
pub fn avaliar_caminhos(
    analise: &Analise,
    esperado: &CaminhosEsperados,
    validas: &BTreeMap<u32, Vec<&Evidencia>>,
) -> CaminhosAvaliados {
    let afirmacoes: Vec<(&Afirmacao, &[&Evidencia])> = analise
        .afirmacoes
        .iter()
        .filter(|a| !matches!(a.estado, EstadoAfirmacao::Incorreta { .. }))
        .map(|a| {
            let ev = validas.get(&a.numero).map_or(&[][..], Vec::as_slice);
            (a, ev)
        })
        .collect();
    let cita = |c: &CaminhoEsperado, a: &Afirmacao, ev: &[&Evidencia]| {
        texto::atende(texto_do_agente(a), &c.palavras_chave)
            || ev
                .iter()
                .any(|e| c.evidencias.iter().any(|esp| toca(e, esp)))
    };
    let caminhos = esperado
        .caminhos
        .iter()
        .map(|c| CaminhoAvaliado {
            nome: c.nome.clone(),
            valida: c.valida,
            citado_em: afirmacoes
                .iter()
                .find(|(a, ev)| cita(c, a, ev))
                .map(|(a, _)| a.numero),
            sem_validacao_em: if c.valida {
                None
            } else {
                afirmacoes
                    .iter()
                    .filter(|(a, _)| SECOES_DO_CAMINHO_SEM_VALIDACAO.contains(&a.secao.as_str()))
                    .find(|(a, ev)| {
                        cita(c, a, ev) && texto::atende(texto_do_agente(a), &c.sem_validacao)
                    })
                    .map(|(a, _)| a.numero)
            },
        })
        .collect();
    let lista_em = afirmacoes
        .iter()
        .find(|(a, _)| {
            !esperado.caminhos.is_empty()
                && esperado
                    .caminhos
                    .iter()
                    .all(|c| texto::atende(texto_do_agente(a), &c.palavras_chave))
        })
        .map(|(a, _)| a.numero);
    CaminhosAvaliados {
        estado: esperado.estado.clone(),
        caminhos,
        lista_em,
    }
}
