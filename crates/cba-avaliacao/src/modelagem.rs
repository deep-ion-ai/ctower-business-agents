//! Comparação dos diagramas do documento negocial (fluxo `explicar`, #120) com o
//! gabarito: o fluxo do processo e o modelo de dados conceitual.
//!
//! Determinística, como o resto da comparação:
//!
//! - **processo** (o primeiro diagrama do tipo `fluxo`): cada raia esperada é
//!   atendida quando a raia de algum passo tem as palavras-chave; cada passo do
//!   fluxo principal, quando algum nó tem as palavras-chave no rótulo ou na
//!   descrição ou cita uma evidência que toca uma das esperadas; cada decisão, do
//!   mesmo jeito, mas só por nó de decisão. A coerência do fluxograma: um início,
//!   ao menos um fim, toda decisão com dois caminhos ou mais e todo nó alcançável
//!   a partir do início;
//! - **modelo de dados** (o primeiro diagrama do tipo `dados`): cada entidade
//!   esperada é achada pela tabela, com o nome de negócio (palavras-chave) e a
//!   ligação com o glossário; cada relação esperada é achada pelas tabelas das
//!   pontas, em qualquer sentido, e a origem registrada precisa ser a esperada
//!   (chave estrangeira ou join). Relação esperada como join registrada como
//!   chave estrangeira, ou o contrário, é falha grave: o documento não distingue a
//!   relação inferida.

use cba_core::analise::{Analise, Evidencia};
use cba_core::diagramas::{Diagrama, OrigemRelacao, PapelNo, TipoDiagrama};
use serde::Serialize;

use crate::gabarito::{EvidenciaEsperada, ItemDoProcesso, ModelagemEsperada};
use crate::texto;

/// Evidências mais largas que isso não contam para casar com as esperadas.
const LINHAS_PARA_CASAR: u32 = 30;

/// Resultado da comparação dos diagramas.
#[derive(Debug, Clone, Serialize)]
pub struct ModelagemAvaliada {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub processo: Option<ProcessoAvaliado>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dados: Option<DadosAvaliados>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ItemAvaliado {
    pub id: String,
    pub descricao: String,
    /// Chave do nó (ou raia) que atende.
    pub atendido_por: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProcessoAvaliado {
    /// Diagrama avaliado (`D-1`); `None` se a análise não tem fluxo.
    pub diagrama: Option<String>,
    pub raias: Vec<ItemAvaliado>,
    pub principal: Vec<ItemAvaliado>,
    pub decisoes: Vec<ItemAvaliado>,
    pub tem_inicio: bool,
    pub fins: usize,
    /// Decisões com menos de dois caminhos de saída.
    pub decisoes_sem_caminhos: Vec<String>,
    /// Nós que não se alcançam a partir do início.
    pub inalcancaveis: Vec<String>,
}

impl ProcessoAvaliado {
    /// O fluxograma é coerente: início, fim, decisões com caminhos e todos os nós
    /// alcançáveis.
    pub fn coerente(&self) -> bool {
        self.diagrama.is_some()
            && self.tem_inicio
            && self.fins > 0
            && self.decisoes_sem_caminhos.is_empty()
            && self.inalcancaveis.is_empty()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct EntidadeAvaliada {
    pub id: String,
    pub tabela: String,
    /// Rótulo da entidade registrada para a tabela.
    pub registrada: Option<String>,
    /// O rótulo tem as palavras-chave do nome de negócio.
    pub nome_confere: bool,
    /// Termo do glossário ligado pelo app.
    pub termo: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RelacaoAvaliada {
    pub id: String,
    pub de: String,
    pub para: String,
    pub origem_esperada: String,
    pub opcional: bool,
    /// Origem registrada (`chave_estrangeira` ou `join`), se a relação existe.
    pub origem_registrada: Option<String>,
}

impl RelacaoAvaliada {
    pub fn encontrada(&self) -> bool {
        self.origem_registrada.is_some()
    }

    /// Registrada com a origem errada: o documento não distingue a relação por
    /// chave estrangeira da inferida.
    pub fn origem_diverge(&self) -> bool {
        self.origem_registrada
            .as_deref()
            .is_some_and(|o| o != self.origem_esperada)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DadosAvaliados {
    pub diagrama: Option<String>,
    pub entidades: Vec<EntidadeAvaliada>,
    pub relacoes: Vec<RelacaoAvaliada>,
}

impl ModelagemAvaliada {
    /// Falhas graves dos diagramas.
    pub fn falhas(&self) -> Vec<String> {
        let mut f = Vec::new();
        if let Some(d) = &self.dados {
            for r in d.relacoes.iter().filter(|r| r.origem_diverge()) {
                f.push(format!(
                    "relação {} ({} – {}) registrada como {}, esperada {}",
                    r.id,
                    r.de,
                    r.para,
                    r.origem_registrada.as_deref().unwrap_or_default(),
                    r.origem_esperada
                ));
            }
        }
        f
    }
}

fn toca(esperadas: &[EvidenciaEsperada], evidencias: &[Evidencia]) -> bool {
    evidencias.iter().any(|e| {
        e.linhas.fim - e.linhas.inicio < LINHAS_PARA_CASAR
            && esperadas.iter().any(|x| {
                x.arquivo == e.arquivo.as_str()
                    && e.linhas.inicio <= x.linha_final
                    && x.linha_inicial <= e.linhas.fim
            })
    })
}

fn primeiro(analise: &Analise, tipo: TipoDiagrama) -> Option<&Diagrama> {
    analise.diagramas.iter().find(|d| d.tipo == tipo)
}

fn avaliar_itens(
    d: Option<&Diagrama>,
    itens: &[ItemDoProcesso],
    so_decisoes: bool,
) -> Vec<ItemAvaliado> {
    itens
        .iter()
        .map(|item| {
            let atendido_por = d.and_then(|d| {
                d.nos
                    .iter()
                    .filter(|n| !so_decisoes || n.papel == PapelNo::Decisao)
                    .find(|n| {
                        let texto = format!(
                            "{} {}",
                            n.rotulo,
                            n.descricao.as_deref().unwrap_or_default()
                        );
                        texto::atende(&texto, &item.palavras_chave)
                            || toca(&item.evidencias, &n.evidencias)
                    })
                    .map(|n| n.chave.clone())
            });
            ItemAvaliado {
                id: item.id.clone(),
                descricao: item.descricao.clone(),
                atendido_por,
            }
        })
        .collect()
}

fn avaliar_processo(
    analise: &Analise,
    esperado: &crate::gabarito::ProcessoEsperado,
) -> ProcessoAvaliado {
    let d = primeiro(analise, TipoDiagrama::Fluxo);
    let raias = esperado
        .raias
        .iter()
        .map(|item| ItemAvaliado {
            id: item.id.clone(),
            descricao: item.descricao.clone(),
            atendido_por: d.and_then(|d| {
                d.nos
                    .iter()
                    .filter_map(|n| n.raia.as_deref())
                    .find(|r| texto::atende(r, &item.palavras_chave))
                    .map(str::to_owned)
            }),
        })
        .collect();
    let mut decisoes_sem_caminhos = Vec::new();
    let mut inalcancaveis = Vec::new();
    let (mut tem_inicio, mut fins) = (false, 0);
    if let Some(d) = d {
        tem_inicio = d.nos.iter().any(|n| n.papel == PapelNo::Inicio);
        fins = d.nos.iter().filter(|n| n.papel == PapelNo::Fim).count();
        for n in d.nos.iter().filter(|n| n.papel == PapelNo::Decisao) {
            if d.arestas.iter().filter(|a| a.de == n.chave).count() < 2 {
                decisoes_sem_caminhos.push(n.chave.clone());
            }
        }
        let mut alcancados: Vec<&str> = d
            .nos
            .iter()
            .filter(|n| n.papel == PapelNo::Inicio)
            .map(|n| n.chave.as_str())
            .collect();
        let mut i = 0;
        while i < alcancados.len() {
            let atual = alcancados[i];
            for a in d.arestas.iter().filter(|a| a.de == atual) {
                if !alcancados.contains(&a.para.as_str()) {
                    alcancados.push(&a.para);
                }
            }
            i += 1;
        }
        if tem_inicio {
            inalcancaveis = d
                .nos
                .iter()
                .filter(|n| !alcancados.contains(&n.chave.as_str()))
                .map(|n| n.chave.clone())
                .collect();
        }
    }
    ProcessoAvaliado {
        diagrama: d.map(Diagrama::rotulo),
        raias,
        principal: avaliar_itens(d, &esperado.principal, false),
        decisoes: avaliar_itens(d, &esperado.decisoes, true),
        tem_inicio,
        fins,
        decisoes_sem_caminhos,
        inalcancaveis,
    }
}

fn mesma_tabela(a: &str, b: &str) -> bool {
    let curto = |t: &str| {
        t.trim()
            .rsplit('.')
            .next()
            .unwrap_or_default()
            .to_uppercase()
    };
    curto(a) == curto(b)
}

fn avaliar_dados(
    analise: &Analise,
    esperado: &crate::gabarito::ModeloDadosEsperado,
) -> DadosAvaliados {
    let d = primeiro(analise, TipoDiagrama::Dados);
    let no_da_tabela = |tabela: &str| {
        d.and_then(|d| {
            d.nos.iter().find(|n| {
                n.entidade
                    .as_ref()
                    .is_some_and(|e| mesma_tabela(&e.tabela, tabela))
            })
        })
    };
    let entidades = esperado
        .entidades
        .iter()
        .map(|e| {
            let no = no_da_tabela(&e.tabela);
            EntidadeAvaliada {
                id: e.id.clone(),
                tabela: e.tabela.clone(),
                registrada: no.map(|n| n.rotulo.clone()),
                nome_confere: no.is_some_and(|n| texto::atende(&n.rotulo, &e.palavras_chave)),
                termo: no.and_then(|n| n.entidade.as_ref()?.termo.clone()),
            }
        })
        .collect();
    let relacoes = esperado
        .relacoes
        .iter()
        .map(|r| {
            let origem = match (no_da_tabela(&r.de), no_da_tabela(&r.para), d) {
                (Some(a), Some(b), Some(d)) => d
                    .arestas
                    .iter()
                    .find(|x| {
                        (x.de == a.chave && x.para == b.chave)
                            || (x.de == b.chave && x.para == a.chave)
                    })
                    .and_then(|x| x.relacao)
                    .map(|rel| match rel.origem {
                        OrigemRelacao::ChaveEstrangeira => "chave_estrangeira".to_owned(),
                        OrigemRelacao::Join => "join".to_owned(),
                    }),
                _ => None,
            };
            RelacaoAvaliada {
                id: r.id.clone(),
                de: r.de.clone(),
                para: r.para.clone(),
                origem_esperada: r.origem.clone(),
                opcional: r.opcional,
                origem_registrada: origem,
            }
        })
        .collect();
    DadosAvaliados {
        diagrama: d.map(Diagrama::rotulo),
        entidades,
        relacoes,
    }
}

/// Compara os diagramas da análise com os esperados.
pub fn avaliar_modelagem(analise: &Analise, esperado: &ModelagemEsperada) -> ModelagemAvaliada {
    ModelagemAvaliada {
        processo: esperado
            .processo
            .as_ref()
            .map(|p| avaliar_processo(analise, p)),
        dados: esperado
            .modelo_dados
            .as_ref()
            .map(|m| avaliar_dados(analise, m)),
    }
}
