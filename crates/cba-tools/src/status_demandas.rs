//! Conferência do status das demandas com o rastreador (#292).
//!
//! A demanda que veio de uma issue (com `chave` do rastreador do projeto, e com
//! `origem` desse rastreador, quando houver) tem o estado lido em lote por
//! [`cba_repos::estado::conferir_estados`] e gravado no bloco `rastreador` do
//! `demanda.md` ([`cba_kb::Projeto::gravar_estado_no_rastreador`]), sem mexer
//! no `status` local do analista. A demanda sem `chave` mantém o status local.
//!
//! Um [`CacheConferencia`] curto evita reler o rastreador a cada tela: a
//! demanda conferida há menos de [`VALIDADE_PADRAO`] não é relida, a não ser
//! que a conferência seja forçada ("Atualizar status"). A falha (sem rede, sem
//! token) também fica no cache, com o aviso, para não repetir a espera a cada
//! tela. Nada é escrito no rastreador.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use cba_core::tempo;
use cba_kb::frontmatter::{Data, EstadoNoRastreador, EstadoRastreador};
use cba_kb::{DemandaLida, IdDemanda, Projeto};
use cba_repos::TrackerConfig;
use cba_repos::demanda::Ferramentas;
use cba_repos::estado::{
    EstadoIssue, EstadoLido, chave_normalizada, conferir_estados, origem_do_rastreador,
};

/// Por quanto tempo uma conferência vale.
pub const VALIDADE_PADRAO: Duration = Duration::from_secs(5 * 60);

/// Situação da conferência de uma demanda.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Situacao {
    /// Última conferência que deu certo nesta execução do app (ISO 8601, UTC);
    /// sem ela, vale o `conferido` gravado na demanda.
    pub conferido: Option<String>,
    /// Por que a última tentativa não trouxe o estado: o rastreador inacessível
    /// ou a issue não encontrada. A demanda fica com o último estado conhecido.
    pub aviso: Option<String>,
}

#[derive(Debug, Clone)]
struct Registro {
    quando: Instant,
    situacao: Situacao,
}

/// Cache das conferências, por projeto (a pasta dele na base, para duas bases
/// com o mesmo projeto não se misturarem) e demanda.
#[derive(Debug)]
pub struct CacheConferencia {
    validade: Duration,
    registros: Mutex<HashMap<(String, IdDemanda), Registro>>,
}

impl Default for CacheConferencia {
    fn default() -> Self {
        Self::com_validade(VALIDADE_PADRAO)
    }
}

impl CacheConferencia {
    pub fn com_validade(validade: Duration) -> Self {
        CacheConferencia {
            validade,
            registros: Mutex::new(HashMap::new()),
        }
    }

    /// Situação da última conferência da demanda nesta execução, se houve.
    pub fn situacao(&self, kb: &Projeto, id: IdDemanda) -> Option<Situacao> {
        self.registros()
            .get(&(chave_do_projeto(kb), id))
            .map(|r| r.situacao.clone())
    }

    fn registros(&self) -> std::sync::MutexGuard<'_, HashMap<(String, IdDemanda), Registro>> {
        self.registros.lock().unwrap_or_else(|e| e.into_inner())
    }
}

fn chave_do_projeto(kb: &Projeto) -> String {
    kb.raiz().to_string_lossy().into_owned()
}

/// A demanda é conferida com este rastreador? Devolve a chave normalizada.
pub fn chave_conferivel(tracker: &TrackerConfig, d: &DemandaLida) -> Option<String> {
    let f = &d.documento.frontmatter;
    let chave = chave_normalizada(tracker, f.chave.as_deref()?)?;
    origem_do_rastreador(tracker, f.origem.as_deref()).then_some(chave)
}

/// Estado a gravar na demanda, conferido no instante `agora_ms`.
pub fn estado_no_rastreador(lido: &EstadoLido, agora_ms: u64) -> EstadoNoRastreador {
    EstadoNoRastreador {
        estado: match lido.estado {
            EstadoIssue::Aberta => EstadoRastreador::Aberta,
            EstadoIssue::EmAndamento => EstadoRastreador::EmAndamento,
            EstadoIssue::Encerrada => EstadoRastreador::Encerrada,
        },
        conferido: tempo::instante_iso(agora_ms),
        encerrado: lido.encerrada_em.as_deref().and_then(Data::analisar),
    }
}

/// Confere as demandas (todas as do projeto, ou só `ids`) que vieram de issues
/// do `tracker`. Com `forcar`, ignora o cache. Devolve a situação de cada
/// demanda conferível; as gravações já estão na base quando volta.
pub fn conferir_demandas(
    kb: &Projeto,
    tracker: Option<&TrackerConfig>,
    ids: Option<&[IdDemanda]>,
    forcar: bool,
    cache: &CacheConferencia,
    f: &Ferramentas,
) -> HashMap<IdDemanda, Situacao> {
    let Some(tracker) = tracker else {
        return HashMap::new();
    };
    let demandas = match ids {
        Some([id]) => kb.demanda(*id).map(|d| vec![d]),
        _ => kb.demandas(),
    };
    let demandas = match demandas {
        Ok(d) => d,
        Err(erro) => {
            tracing::warn!(%erro, "demandas ilegíveis para conferir o status");
            return HashMap::new();
        }
    };
    let conferiveis: Vec<(IdDemanda, String)> = demandas
        .iter()
        .filter(|d| ids.is_none_or(|ids| ids.contains(&d.id)))
        .filter_map(|d| chave_conferivel(tracker, d).map(|c| (d.id, c)))
        .collect();
    let projeto = chave_do_projeto(kb);
    let vencidas: Vec<&(IdDemanda, String)> = {
        let registros = cache.registros();
        conferiveis
            .iter()
            .filter(|(id, _)| {
                forcar
                    || registros
                        .get(&(projeto.clone(), *id))
                        .is_none_or(|r| r.quando.elapsed() >= cache.validade)
            })
            .collect()
    };
    if !vencidas.is_empty() {
        let chaves: Vec<String> = vencidas.iter().map(|(_, c)| c.clone()).collect();
        let resposta = conferir_estados(tracker, &chaves, f);
        let agora_ms = tempo::agora_ms();
        let agora = Instant::now();
        let mut registros = cache.registros();
        for (id, chave) in vencidas {
            let anterior = registros
                .get(&(projeto.clone(), *id))
                .and_then(|r| r.situacao.conferido.clone());
            let situacao = match &resposta {
                Ok(estados) => match estados.get(chave) {
                    Some(lido) => {
                        match kb
                            .gravar_estado_no_rastreador(*id, estado_no_rastreador(lido, agora_ms))
                        {
                            Ok(_) => Situacao {
                                conferido: Some(tempo::instante_iso(agora_ms)),
                                aviso: None,
                            },
                            Err(erro) => Situacao {
                                conferido: anterior,
                                aviso: Some(format!(
                                    "o estado do rastreador não pôde ser gravado: {erro}"
                                )),
                            },
                        }
                    }
                    None => Situacao {
                        conferido: anterior,
                        aviso: Some(format!(
                            "a issue {chave} não foi encontrada no rastreador; \
                             fica o último estado conhecido"
                        )),
                    },
                },
                Err(erro) => Situacao {
                    conferido: anterior,
                    aviso: Some(format!(
                        "rastreador inacessível ({erro}); fica o último estado conhecido"
                    )),
                },
            };
            registros.insert(
                (projeto.clone(), *id),
                Registro {
                    quando: agora,
                    situacao,
                },
            );
        }
    }
    let registros = cache.registros();
    conferiveis
        .iter()
        .map(|(id, _)| {
            let s = registros
                .get(&(projeto.clone(), *id))
                .map(|r| r.situacao.clone())
                .unwrap_or_default();
            (*id, s)
        })
        .collect()
}
