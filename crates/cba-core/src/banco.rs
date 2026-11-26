//! Banco conectado a uma sessão (#439): o schema real, lido do servidor por uma
//! conexão só de leitura.
//!
//! O acesso (driver, só leitura imposta, tempo limite) fica na crate
//! `cba-banco`; aqui ficam só o modelo e o trait que as ferramentas usam, para
//! que elas não dependam do driver. Quem monta o [`crate::ambiente::Ambiente`]
//! decide se há um banco conectado ([`Ambiente::banco`](crate::ambiente::Ambiente::banco)).

use std::fmt;

use serde::{Deserialize, Serialize};

/// De onde vem o banco: a conexão cadastrada, sem endereço nem usuário.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrigemBanco {
    /// Identificador da conexão (`banco-homologacao`).
    pub conexao: String,
    /// Nome que o analista vê ("Banco de homologação").
    pub nome: String,
    /// Rótulo do tipo ("PostgreSQL", "MySQL/MariaDB").
    pub tipo: String,
    /// Rótulo do ambiente ("homologação"), se informado.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ambiente: Option<String>,
}

impl fmt::Display for OrigemBanco {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({}", self.nome, self.tipo)?;
        if let Some(a) = &self.ambiente {
            write!(f, ", {a}")?;
        }
        f.write_str(")")
    }
}

/// Tabelas e visões visíveis ao usuário da conexão.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaBanco {
    pub tabelas: Vec<TabelaBanco>,
}

/// Tabela ou visão do banco.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TabelaBanco {
    pub esquema: String,
    pub nome: String,
    /// `tabela` ou `visao`.
    pub tipo: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comentario: Option<String>,
    /// Na ordem da tabela.
    pub colunas: Vec<ColunaBanco>,
    /// Colunas da chave primária, na ordem da chave.
    #[serde(default)]
    pub chave_primaria: Vec<String>,
    #[serde(default)]
    pub chaves_estrangeiras: Vec<ChaveEstrangeira>,
}

impl TabelaBanco {
    /// `esquema.nome`.
    pub fn qualificado(&self) -> String {
        format!("{}.{}", self.esquema, self.nome)
    }
}

/// Coluna de uma tabela do banco.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColunaBanco {
    pub nome: String,
    /// Tipo como o servidor o descreve (`character varying(120)`, `varchar(120)`).
    pub tipo: String,
    pub nula: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comentario: Option<String>,
}

/// Chave estrangeira de uma tabela para outra.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChaveEstrangeira {
    pub nome: String,
    pub colunas: Vec<String>,
    /// `esquema.tabela` referenciada.
    pub referencia: String,
    pub colunas_referencia: Vec<String>,
}

/// Um banco conectado à sessão. A implementação (em `cba-banco`) guarda a
/// conexão e a credencial; as ferramentas só veem a origem e o resultado. Os
/// erros já vêm sem senha nem string de conexão.
pub trait BancoConectado: Send + Sync + fmt::Debug {
    fn origem(&self) -> OrigemBanco;
    /// Tabelas, colunas, tipos e chaves do banco.
    fn ler_schema(&self) -> Result<SchemaBanco, String>;
}
