//! [`BancoConectado`] de uma conexão cadastrada, para o ambiente das
//! ferramentas: o schema é lido uma vez por processo e guardado; as consultas
//! (#440) rodam pela sessão só de leitura de [`crate::consultar`].
//!
//! [`FonteDoCatalogo`] abre o banco de uma conexão do catálogo do processo pelo
//! `id`, com a senha do resolvedor de credenciais, e guarda o banco aberto
//! enquanto a conexão não mudar.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use cba_core::banco::{
    BancoConectado, ColunaDados, FonteBancos, OrigemBanco, ResultadoDados, SchemaBanco,
};
use cba_repos::chaveiro::Chaveiro;
use cba_repos::providers::{Capacidade, Catalogo, Conexao, todos};

use crate::{ConfigBanco, consultar, ler_schema};

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

    fn validar(&self, sql: &str) -> Result<(), String> {
        crate::validar(self.config.dialeto, sql)
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    fn consultar(&self, sql: &str) -> Result<ResultadoDados, String> {
        let r = consultar(&self.config, sql).map_err(|e| e.to_string())?;
        Ok(ResultadoDados {
            colunas: r
                .colunas
                .into_iter()
                .map(|c| ColunaDados {
                    nome: c.nome,
                    tipo: c.tipo,
                })
                .collect(),
            linhas: r.linhas,
            cortada: r.cortada,
            limite_linhas: r.limite_linhas,
            duracao_ms: r.duracao_ms,
        })
    }
}

/// Abre os bancos das conexões do catálogo do processo
/// ([`Catalogo::global`]), com a senha do resolvedor.
pub struct FonteDoCatalogo {
    chaveiro: Arc<dyn Chaveiro>,
    abertos: Mutex<HashMap<String, (Conexao, Arc<BancoDaConexao>)>>,
}

impl FonteDoCatalogo {
    pub fn new(chaveiro: Arc<dyn Chaveiro>) -> Self {
        Self {
            chaveiro,
            abertos: Mutex::new(HashMap::new()),
        }
    }
}

impl std::fmt::Debug for FonteDoCatalogo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FonteDoCatalogo").finish_non_exhaustive()
    }
}

impl FonteBancos for FonteDoCatalogo {
    fn abrir(&self, id: &str) -> Result<Arc<dyn BancoConectado>, String> {
        let conexao = Catalogo::global().obter(id).ok_or_else(|| {
            format!(
                "a conexão \"{}\" não está cadastrada: confira em Configurações › Conexões",
                id.trim()
            )
        })?;
        let e_banco = todos()
            .get(&conexao.tipo)
            .is_some_and(|d| d.tem(Capacidade::Banco));
        if !e_banco {
            return Err(format!("a conexão \"{}\" não é de banco", conexao.nome));
        }
        let mut abertos = self.abertos.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((c, b)) = abertos.get(id)
            && *c == conexao
        {
            return Ok(b.clone());
        }
        let config =
            ConfigBanco::resolver(&conexao, self.chaveiro.as_ref()).map_err(|e| e.to_string())?;
        let banco = Arc::new(BancoDaConexao::new(config));
        abertos.insert(id.to_owned(), (conexao, banco.clone()));
        Ok(banco)
    }
}
