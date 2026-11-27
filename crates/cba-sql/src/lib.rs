//! Leitura de SQL sem driver de banco.
//!
//! - [`validacao`]: a instrução é uma só, e só de leitura (o acesso ao banco,
//!   `cba-banco`, valida cada consulta antes de enviá-la).
//! - [`consulta`]: os nomes que uma consulta usa (tabelas, colunas, apelidos e
//!   parâmetros nomeados), para conferir as consultas de verificação que o
//!   agente escreve no documento da análise contra o modelo de dados (#453).
//!
//! Fica fora de `cba-banco` para as ferramentas usarem sem depender do driver.

pub mod consulta;
pub mod validacao;

pub use consulta::{ColunaCitada, ConsultaLida, Relacao, ler};
pub use validacao::{Recusa, validar};
