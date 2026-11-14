//! Erros da base de conhecimento.

use std::io;
use std::path::PathBuf;

use crate::caminho::{CaminhoInvalido, CaminhoRelativo};
use crate::documento::ErroFrontmatter;
use crate::id::{IdDemanda, IdInvalido, IdReuniao};

pub type Resultado<T> = Result<T, Erro>;

/// Erro de uma operação na base. Os caminhos nas mensagens são relativos à raiz da
/// base, para não expor a pasta do analista em logs.
#[derive(Debug, thiserror::Error)]
pub enum Erro {
    /// Frontmatter inválido, com o arquivo e, quando for o caso, o campo.
    #[error("{arquivo}: {erro}")]
    Frontmatter {
        arquivo: CaminhoRelativo,
        #[source]
        erro: ErroFrontmatter,
    },
    /// Falha de leitura ou gravação.
    #[error("{arquivo}: {erro}")]
    Io {
        arquivo: CaminhoRelativo,
        #[source]
        erro: io::Error,
    },
    /// Arquivo de texto que não é UTF-8.
    #[error("{0}: o arquivo não está em UTF-8")]
    NaoUtf8(CaminhoRelativo),
    /// O documento já existe e a operação só cria.
    #[error("{0}: o documento já existe")]
    JaExiste(CaminhoRelativo),
    /// A pasta da base não existe ou não é uma pasta.
    #[error("{}: a pasta da base não existe", .0.display())]
    BaseInexistente(PathBuf),
    /// Falha ao criar ou abrir a pasta da base.
    #[error("{}: {erro}", caminho.display())]
    PastaDaBase {
        caminho: PathBuf,
        #[source]
        erro: io::Error,
    },
    /// O projeto não tem pasta na base.
    #[error("projeto `{0}` não encontrado na base")]
    ProjetoInexistente(String),
    /// Nome de projeto que não é slug.
    #[error("nome de projeto inválido `{0}`: use só a-z, 0-9 e -")]
    NomeDeProjeto(String),
    /// O `id` do `projeto.md` difere do nome da pasta.
    #[error("o id do projeto (`{id}`) precisa ser igual ao nome da pasta (`{pasta}`)")]
    IdDoProjeto { id: String, pasta: String },
    /// A demanda não tem pasta no projeto.
    #[error("demanda {id} não encontrada no projeto `{projeto}`")]
    DemandaInexistente { projeto: String, id: IdDemanda },
    /// A pessoa não está no catálogo do projeto.
    #[error("pessoa `{pessoa}` não encontrada no catálogo do projeto `{projeto}`")]
    PessoaInexistente { projeto: String, pessoa: String },
    /// O nome ou um apelido da pessoa nova casa com uma pessoa do catálogo.
    #[error("`{nome}` já está no catálogo do projeto `{projeto}`, como `{pessoa}`")]
    PessoaJaCadastrada {
        projeto: String,
        /// Slug da pessoa que já está no catálogo.
        pessoa: String,
        nome: String,
    },
    /// O apelido casa com outra pessoa do catálogo: são a mesma pessoa (junte as
    /// duas) ou o apelido é ambíguo.
    #[error(
        "o apelido `{apelido}` já é de `{outra}` no projeto `{projeto}`; se for a mesma \
         pessoa, junte as duas"
    )]
    ApelidoDeOutraPessoa {
        projeto: String,
        apelido: String,
        /// Slug da pessoa que já tem o apelido.
        outra: String,
    },
    /// Pedido para juntar uma pessoa com ela mesma.
    #[error("não dá para juntar `{0}` com ela mesma")]
    JuntarMesmaPessoa(String),
    /// A reunião não tem estado em nenhuma demanda do projeto.
    #[error("reunião {id} não encontrada no projeto `{projeto}`")]
    ReuniaoInexistente { projeto: String, id: IdReuniao },
    #[error(transparent)]
    Caminho(#[from] CaminhoInvalido),
    #[error(transparent)]
    Id(#[from] IdInvalido),
}
