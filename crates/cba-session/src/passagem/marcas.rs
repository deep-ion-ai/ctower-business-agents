//! O cliente de cada sessão, informado pelo app (veja o módulo [`super`]).

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use cba_core::analise::Cliente;
use serde::{Deserialize, Serialize};

use crate::sessoes::RegistroSessao;

/// Quantas marcas de cada tipo o arquivo guarda; as mais antigas saem.
pub(super) const MAX_MARCAS: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
struct Marca {
    cliente: Cliente,
    em_ms: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Conteudo {
    /// Por `session_id`.
    #[serde(default)]
    sessoes: BTreeMap<String, Marca>,
    /// Por `<sistema>/<análise>`: a próxima sessão ligada à análise.
    #[serde(default)]
    analises: BTreeMap<String, Marca>,
}

/// Arquivo JSON com o cliente pelo qual o app abriu cada sessão ou análise
/// (`<dados>/terminal/clientes.json`). Guarda só identificadores, clientes e datas.
#[derive(Debug, Clone)]
pub struct ClientesDasSessoes {
    arquivo: PathBuf,
}

impl ClientesDasSessoes {
    pub fn em(arquivo: impl Into<PathBuf>) -> Self {
        Self {
            arquivo: arquivo.into(),
        }
    }

    /// No diretório de dados do app.
    pub fn no_diretorio(dados: &Path) -> Self {
        Self::em(dados.join("terminal").join("clientes.json"))
    }

    fn ler(&self) -> Conteudo {
        std::fs::read(&self.arquivo)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    fn gravar(&self, mut c: Conteudo) -> io::Result<()> {
        for mapa in [&mut c.sessoes, &mut c.analises] {
            while mapa.len() > MAX_MARCAS {
                let antiga = mapa
                    .iter()
                    .min_by_key(|(_, m)| m.em_ms)
                    .map(|(k, _)| k.clone());
                match antiga {
                    Some(k) => mapa.remove(&k),
                    None => break,
                };
            }
        }
        let json = serde_json::to_vec_pretty(&c).map_err(io::Error::other)?;
        cba_kb::arquivo::gravar(&self.arquivo, &json)
    }

    /// A sessão `id` foi aberta (ou passou) para `cliente` em `em_ms`.
    pub fn marcar_sessao(&self, id: &str, cliente: Cliente, em_ms: u64) -> io::Result<()> {
        let mut c = self.ler();
        c.sessoes.insert(id.to_owned(), Marca { cliente, em_ms });
        self.gravar(c)
    }

    /// Uma conversa sobre `analise` foi aberta em `cliente` sem o app saber o
    /// identificador (links do Desktop e do terminal do analista).
    pub fn marcar_analise(
        &self,
        sistema: &str,
        analise: &str,
        cliente: Cliente,
        em_ms: u64,
    ) -> io::Result<()> {
        let mut c = self.ler();
        c.analises
            .insert(format!("{sistema}/{analise}"), Marca { cliente, em_ms });
        self.gravar(c)
    }

    /// O cliente da sessão: o do registro, se os hooks o souberem; senão a marca do
    /// identificador; senão a marca da análise feita antes de a sessão começar.
    pub fn cliente_de(&self, registro: &RegistroSessao) -> Cliente {
        if registro.cliente != Cliente::Desconhecido {
            return registro.cliente;
        }
        let c = self.ler();
        if let Some(m) = c.sessoes.get(&registro.id) {
            return m.cliente;
        }
        registro
            .analise
            .as_ref()
            .and_then(|a| c.analises.get(&format!("{}/{a}", registro.sistema)))
            // Tolerância de um segundo: os relógios são do mesmo computador, mas o
            // registro pode nascer no mesmo instante do clique.
            .filter(|m| m.em_ms <= registro.inicio_ms.saturating_add(1_000))
            .map_or(Cliente::Desconhecido, |m| m.cliente)
    }
}
