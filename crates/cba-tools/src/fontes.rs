//! De onde as ferramentas de consulta leem: o índice da base de conhecimento
//! (`cba_kb::indice`) e o mapa estrutural do sistema (`cba_index::mapa`).
//!
//! - **Índice:** aberto a cada chamada no diretório de dados e sincronizado antes da
//!   busca, o que custa pouco quando nada mudou na base. O servidor MCP não mantém
//!   uma vigia, então é a sincronização que reflete as edições recentes.
//! - **Mapa:** o último mapa gravado no cadastro do sistema. Se ainda não existe
//!   (sistema cadastrado antes do primeiro mapeamento) ou se é de outro formato, a
//!   ferramenta o gera na hora, uma vez, e avisa o agente. Um mapa existente não é
//!   refeito pela ferramenta: o app o atualiza (no cadastro e ao reabrir o sistema),
//!   e a resposta traz a data em que foi gerado. O mapa lido fica em memória
//!   enquanto o arquivo não mudar, porque num sistema grande ele tem dezenas de
//!   megabytes.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use cba_core::ambiente::Ambiente;
use cba_core::tempo;
use cba_index::{Cadastro, FORMATO_MAPA, Mapa};
use cba_kb::indice::Indice;
use serde_json::{Value, json};

use crate::registro::ErroFerramenta;

/// Nome do arquivo do mapa na pasta do sistema no cadastro (o mesmo de
/// `cba_index::cadastro`).
const ARQUIVO_DO_MAPA: &str = "mapa.json";

/// Abre o índice da base e o sincroniza com os arquivos.
pub(crate) fn indice(ambiente: &Ambiente) -> Result<Indice, ErroFerramenta> {
    let falha = |e: cba_kb::indice::ErroIndice| {
        tracing::warn!("índice da base indisponível");
        ErroFerramenta::Interno(format!(
            "o índice da base de conhecimento não pôde ser aberto ({e}). Leia os \
             documentos da base diretamente ou peça ao analista para reabrir o app"
        ))
    };
    let mut indice =
        Indice::abrir(ambiente.dados.arquivo_do_indice(), &ambiente.base).map_err(falha)?;
    indice.sincronizar().map_err(falha)?;
    Ok(indice)
}

/// Mapa carregado para uma consulta.
#[derive(Debug, Clone)]
pub(crate) struct MapaDoSistema {
    pub mapa: Arc<Mapa>,
    /// O mapa não existia (ou era de outro formato) e foi gerado nesta chamada.
    pub gerado_agora: bool,
}

impl MapaDoSistema {
    /// Resumo do mapa para a resposta estruturada.
    pub fn json(&self) -> Value {
        json!({
            "gerado_em": data(self.mapa.gerado_em),
            "gerado_agora": self.gerado_agora,
            "arquivos": self.mapa.arquivos().len(),
            "falhas": self.mapa.estatisticas.falhas,
        })
    }

    /// Nota para o texto: de quando é o mapa e, se foi gerado agora, o aviso.
    pub fn nota(&self) -> String {
        if self.gerado_agora {
            format!(
                "O mapa do sistema não existia e foi gerado agora ({} arquivos). Ele é \
                 estrutural (nomes de rotinas, tabelas e referências, sem IA): confirme \
                 no código antes de afirmar.",
                self.mapa.arquivos().len()
            )
        } else {
            format!(
                "Mapa do sistema gerado em {} ({} arquivos). Ele é estrutural: confirme \
                 no código antes de afirmar.",
                data(self.mapa.gerado_em),
                self.mapa.arquivos().len()
            )
        }
    }
}

fn data(segundos: u64) -> String {
    tempo::data_utc(segundos.saturating_mul(1000)).to_string()
}

/// Mapa em memória: o arquivo, a data de modificação e o tamanho quando foi lido,
/// e o mapa.
type MapaEmMemoria = (PathBuf, (SystemTime, u64), Arc<Mapa>);

/// Data de modificação e tamanho do arquivo, para saber se mudou.
fn versao(arquivo: &std::path::Path) -> Option<(SystemTime, u64)> {
    let m = std::fs::metadata(arquivo).ok()?;
    Some((m.modified().ok()?, m.len()))
}

static EM_MEMORIA: Mutex<Option<MapaEmMemoria>> = Mutex::new(None);

/// O mapa do sistema: o gravado ou, se não houver um válido, um gerado agora.
pub(crate) fn mapa(ambiente: &Ambiente) -> Result<MapaDoSistema, ErroFerramenta> {
    let id = &ambiente.sistema.id;
    let cadastro = Cadastro::em(ambiente.dados.sistemas());
    let arquivo = cadastro.raiz().join(id).join(ARQUIVO_DO_MAPA);
    let modificado = versao(&arquivo);

    if let Some(modificado) = modificado {
        let guardado = EM_MEMORIA
            .lock()
            .ok()
            .and_then(|g| g.as_ref().cloned())
            .filter(|(a, m, _)| *a == arquivo && *m == modificado);
        if let Some((_, _, mapa)) = guardado {
            return Ok(MapaDoSistema {
                mapa,
                gerado_agora: false,
            });
        }
        match cadastro.mapa(id) {
            Ok(Some(mapa)) if mapa.formato == FORMATO_MAPA => {
                let mapa = Arc::new(mapa);
                guardar(arquivo, modificado, &mapa);
                return Ok(MapaDoSistema {
                    mapa,
                    gerado_agora: false,
                });
            }
            Ok(_) => tracing::info!("mapa de outro formato; gerando de novo"),
            Err(_) => tracing::warn!("mapa ilegível; gerando de novo"),
        }
    }

    tracing::info!("mapa do sistema ausente; gerando sob demanda");
    let mapa = cadastro.mapear(id).map_err(|e| {
        tracing::warn!("mapa do sistema não gerado");
        ErroFerramenta::Recusa(format!(
            "o mapa do sistema ainda não existe e não pôde ser gerado agora ({e}). Use as \
             suas ferramentas de leitura e busca no código e peça ao analista para refazer \
             o mapeamento do sistema no app"
        ))
    })?;
    let mapa = Arc::new(mapa);
    if let Some(modificado) = versao(&arquivo) {
        guardar(arquivo, modificado, &mapa);
    }
    Ok(MapaDoSistema {
        mapa,
        gerado_agora: true,
    })
}

fn guardar(arquivo: PathBuf, modificado: (SystemTime, u64), mapa: &Arc<Mapa>) {
    if let Ok(mut g) = EM_MEMORIA.lock() {
        *g = Some((arquivo, modificado, Arc::clone(mapa)));
    }
}
