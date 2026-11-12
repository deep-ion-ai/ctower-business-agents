//! Erros do cadastro e do inventário.

use std::io;
use std::path::PathBuf;

/// Erro do cadastro, do escopo ou do inventário de um projeto.
#[derive(Debug, thiserror::Error)]
pub enum Erro {
    #[error("o nome do projeto está vazio")]
    NomeVazio,

    #[error("identificador de projeto inválido: {0:?} (use letras minúsculas, dígitos e hífens)")]
    IdInvalido(String),

    #[error("o projeto precisa de ao menos uma pasta")]
    SemPastas,

    #[error("pasta não encontrada: {0}")]
    PastaNaoEncontrada(PathBuf),

    #[error("não é uma pasta: {0}")]
    NaoEhPasta(PathBuf),

    #[error("as pastas {0} e {1} se sobrepõem")]
    PastasSobrepostas(PathBuf, PathBuf),

    #[error("prefixo de pasta inválido ou repetido: {0:?}")]
    PrefixoInvalido(String),

    #[error("só uma pasta do projeto pode ficar sem prefixo")]
    VariasPastasSemPrefixo,

    #[error("o prefixo {prefixo:?} coincide com um item da pasta raiz {raiz}")]
    PrefixoConflitante { prefixo: String, raiz: PathBuf },

    #[error("padrão de exclusão inválido {padrao:?}: {detalhe}")]
    PadraoInvalido { padrao: String, detalhe: String },

    #[error("projeto já cadastrado: {0}")]
    JaCadastrado(String),

    #[error("projeto não cadastrado: {0}")]
    NaoCadastrado(String),

    #[error("erro ao acessar {caminho}: {fonte}")]
    Io {
        caminho: PathBuf,
        #[source]
        fonte: io::Error,
    },

    #[error("arquivo inválido {caminho}: {fonte}")]
    Json {
        caminho: PathBuf,
        #[source]
        fonte: serde_json::Error,
    },
}

impl Erro {
    pub(crate) fn io(caminho: impl Into<PathBuf>, fonte: io::Error) -> Self {
        Self::Io {
            caminho: caminho.into(),
            fonte,
        }
    }
}
