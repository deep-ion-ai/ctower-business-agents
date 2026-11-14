//! Registro de ferramentas: nome, descrição, schema de entrada e o que executar.
//!
//! O registro não conhece o protocolo: o servidor MCP (`cba-mcp`) o expõe por
//! `tools/list` e `tools/call`, e o laço próprio da fase 2 (spec, seção 6.6) vai
//! chamar as mesmas ferramentas. Para acrescentar uma ferramenta, crie uma função que
//! devolve uma [`Ferramenta`] e registre-a em [`crate::registro_padrao`].
//!
//! ```
//! use cba_tools::{Ferramenta, Registro, Saida, ler_argumentos};
//! use serde::Deserialize;
//! use serde_json::json;
//!
//! #[derive(Deserialize)]
//! struct Args { texto: String }
//!
//! let eco = Ferramenta::nova(
//!     "eco",
//!     "Eco",
//!     "Devolve o texto recebido.",
//!     json!({
//!         "type": "object",
//!         "properties": { "texto": { "type": "string", "minLength": 1 } },
//!         "required": ["texto"],
//!         "additionalProperties": false
//!     }),
//!     |_ambiente, args| {
//!         let args: Args = ler_argumentos(args)?;
//!         Ok(Saida::texto(args.texto))
//!     },
//! )
//! .somente_leitura();
//! let mut registro = Registro::new();
//! registro.registrar(eco);
//! assert_eq!(registro.ferramentas().len(), 1);
//! ```

use std::panic::{AssertUnwindSafe, catch_unwind};

use cba_core::ambiente::{Ambiente, ErroAmbiente};
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::schema;

/// O que uma ferramenta executa: recebe o ambiente do projeto e os argumentos já
/// conferidos contra o schema.
pub type Executar = dyn Fn(&Ambiente, Value) -> Result<Saida, ErroFerramenta> + Send + Sync;

/// Resultado de uma ferramenta.
#[derive(Debug, Clone, PartialEq)]
pub struct Saida {
    /// Texto para o agente.
    pub texto: String,
    /// Dados estruturados (o MCP os envia como `structuredContent`), quando houver.
    pub dados: Option<Value>,
}

impl Saida {
    /// Só texto (um roteiro, por exemplo).
    pub fn texto(texto: impl Into<String>) -> Self {
        Self {
            texto: texto.into(),
            dados: None,
        }
    }

    /// Dados estruturados; o texto é o mesmo JSON, formatado.
    pub fn json(dados: Value) -> Self {
        Self {
            texto: serde_json::to_string_pretty(&dados).unwrap_or_default(),
            dados: Some(dados),
        }
    }
}

/// Erro de uma ferramenta. Toda mensagem se dirige ao agente e diz como corrigir;
/// o servidor a devolve como resultado de erro da ferramenta, que o agente lê.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ErroFerramenta {
    /// Argumentos fora do schema ou inválidos para a ferramenta.
    #[error("argumentos inválidos: {0}")]
    Argumentos(String),
    /// O pedido foi entendido e recusado (evidência que não confere, análise
    /// inexistente, transição de estado inválida).
    #[error("{0}")]
    Recusa(String),
    /// O processo não está num projeto cadastrado ou a base não está acessível.
    #[error("{0}")]
    SemProjeto(String),
    /// Falha nossa, sem ação do agente que resolva.
    #[error("falha interna do Ctower Business Agents: {0}")]
    Interno(String),
}

impl ErroFerramenta {
    /// Nome curto, para o log (sem conteúdo).
    pub fn tipo(&self) -> &'static str {
        match self {
            Self::Argumentos(_) => "argumentos",
            Self::Recusa(_) => "recusa",
            Self::SemProjeto(_) => "sem_projeto",
            Self::Interno(_) => "interno",
        }
    }
}

impl From<cba_core::analises::Erro> for ErroFerramenta {
    fn from(erro: cba_core::analises::Erro) -> Self {
        use cba_core::analises::Erro;
        match erro {
            Erro::NaoEncontrada(_) | Erro::NaoEAnalise(_) | Erro::Modelo(_) | Erro::Memoria(_) => {
                Self::Recusa(erro.to_string())
            }
            Erro::EstadoIlegivel { .. } | Erro::Io { .. } | Erro::Base(_) => {
                Self::Interno(erro.to_string())
            }
        }
    }
}

/// Converte os argumentos (já conferidos pelo schema) na estrutura da ferramenta.
pub fn ler_argumentos<T: DeserializeOwned>(argumentos: Value) -> Result<T, ErroFerramenta> {
    serde_json::from_value(argumentos).map_err(|e| ErroFerramenta::Argumentos(e.to_string()))
}

/// Uma ferramenta registrada.
pub struct Ferramenta {
    nome: &'static str,
    titulo: &'static str,
    descricao: &'static str,
    schema: Value,
    somente_leitura: bool,
    executar: Box<Executar>,
}

impl std::fmt::Debug for Ferramenta {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ferramenta")
            .field("nome", &self.nome)
            .finish_non_exhaustive()
    }
}

impl Ferramenta {
    /// Ferramenta nova. `nome` usa só `a-z`, `0-9` e `_` (o que o agente chama);
    /// `schema` é o JSON Schema dos argumentos, um objeto (`"type": "object"`).
    ///
    /// # Panics
    ///
    /// Se o nome ou o schema forem inválidos (erro de programação, pego nos testes).
    pub fn nova(
        nome: &'static str,
        titulo: &'static str,
        descricao: &'static str,
        schema: Value,
        executar: impl Fn(&Ambiente, Value) -> Result<Saida, ErroFerramenta> + Send + Sync + 'static,
    ) -> Self {
        assert!(
            !nome.is_empty()
                && nome
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_'),
            "nome de ferramenta inválido: {nome:?}"
        );
        assert!(
            schema.get("type") == Some(&Value::String("object".into())),
            "o schema de entrada de {nome} precisa ser um objeto"
        );
        Self {
            nome,
            titulo,
            descricao,
            schema,
            somente_leitura: false,
            executar: Box::new(executar),
        }
    }

    /// Marca a ferramenta como só de leitura (não altera a análise nem a base).
    pub fn somente_leitura(mut self) -> Self {
        self.somente_leitura = true;
        self
    }

    pub fn nome(&self) -> &'static str {
        self.nome
    }

    pub fn titulo(&self) -> &'static str {
        self.titulo
    }

    pub fn descricao(&self) -> &'static str {
        self.descricao
    }

    pub fn schema(&self) -> &Value {
        &self.schema
    }

    pub fn e_somente_leitura(&self) -> bool {
        self.somente_leitura
    }
}

/// Falha de uma chamada pelo registro.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ErroChamada {
    /// Não há ferramenta com esse nome (erro de protocolo).
    #[error("ferramenta desconhecida: {0}")]
    Desconhecida(String),
    /// A ferramenta existe e recusou ou falhou (resultado de erro, para o agente).
    #[error(transparent)]
    Ferramenta(#[from] ErroFerramenta),
}

/// Ferramentas disponíveis, na ordem de registro.
#[derive(Debug, Default)]
pub struct Registro {
    ferramentas: Vec<Ferramenta>,
}

impl Registro {
    pub fn new() -> Self {
        Self::default()
    }

    /// Acrescenta uma ferramenta.
    ///
    /// # Panics
    ///
    /// Se já houver uma ferramenta com o mesmo nome.
    pub fn registrar(&mut self, ferramenta: Ferramenta) -> &mut Self {
        assert!(
            self.obter(ferramenta.nome).is_none(),
            "ferramenta registrada duas vezes: {}",
            ferramenta.nome
        );
        self.ferramentas.push(ferramenta);
        self
    }

    pub fn ferramentas(&self) -> &[Ferramenta] {
        &self.ferramentas
    }

    pub fn obter(&self, nome: &str) -> Option<&Ferramenta> {
        self.ferramentas.iter().find(|f| f.nome == nome)
    }

    /// Chama uma ferramenta: confere os argumentos contra o schema, exige o ambiente
    /// do projeto e executa. Sem argumentos, a ferramenta recebe `{}`. Um pânico na
    /// ferramenta vira [`ErroFerramenta::Interno`], sem derrubar o processo.
    pub fn chamar(
        &self,
        nome: &str,
        ambiente: Result<&Ambiente, &ErroAmbiente>,
        argumentos: Option<Value>,
    ) -> Result<Saida, ErroChamada> {
        let ferramenta = self
            .obter(nome)
            .ok_or_else(|| ErroChamada::Desconhecida(nome.to_owned()))?;
        let argumentos = argumentos.unwrap_or_else(|| Value::Object(Default::default()));
        schema::validar(&argumentos, &ferramenta.schema).map_err(ErroFerramenta::Argumentos)?;
        let ambiente = ambiente.map_err(|e| ErroFerramenta::SemProjeto(e.to_string()))?;
        catch_unwind(AssertUnwindSafe(|| {
            (ferramenta.executar)(ambiente, argumentos)
        }))
        .unwrap_or_else(|_| {
            Err(ErroFerramenta::Interno(format!(
                "a ferramenta {nome} falhou inesperadamente; tente de novo ou avise o analista"
            )))
        })
        .map_err(ErroChamada::from)
    }
}
