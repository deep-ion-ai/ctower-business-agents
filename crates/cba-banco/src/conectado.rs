//! [`BancoConectado`] de uma conexão cadastrada, para o ambiente das
//! ferramentas: o schema é lido uma vez por processo e guardado.

use std::sync::Mutex;

use cba_core::banco::{BancoConectado, OrigemBanco, SchemaBanco};

use crate::{ConfigBanco, ler_schema};

/// O banco de uma conexão, com o schema guardado depois da primeira leitura.
pub struct BancoDaConexao {
    config: ConfigBanco,
    schema: Mutex<Option<SchemaBanco>>,
}

impl BancoDaConexao {
    pub fn new(config: ConfigBanco) -> Self {
        Self {
            config,
            schema: Mutex::new(None),
        }
    }

    pub fn config(&self) -> &ConfigBanco {
        &self.config
    }
}

impl std::fmt::Debug for BancoDaConexao {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BancoDaConexao")
            .field("origem", &self.config.origem)
            .finish_non_exhaustive()
    }
}

impl BancoConectado for BancoDaConexao {
    fn origem(&self) -> OrigemBanco {
        self.config.origem.clone()
    }

    fn ler_schema(&self) -> Result<SchemaBanco, String> {
        let mut guardado = self.schema.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(s) = guardado.as_ref() {
            return Ok(s.clone());
        }
        let s = ler_schema(&self.config).map_err(|e| e.to_string())?;
        *guardado = Some(s.clone());
        Ok(s)
    }
}
