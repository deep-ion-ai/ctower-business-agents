//! Ambiente de trabalho de um processo: qual projeto, qual base, quais análises.
//!
//! O cliente de IA inicia `cba mcp` (e `cba hook`) na pasta do sistema analisado
//! (spec, seções 4.2 e 6.2). O projeto é identificado:
//!
//! 1. pela variável [`VAR_PROJETO`] (`CBA_PROJETO`), que o preparo do ambiente
//!    grava no registro do servidor MCP, quando presente;
//! 2. senão, pelo diretório de trabalho: é o projeto cadastrado que tem uma pasta
//!    contendo esse diretório. Numa worktree do git de uma pasta do projeto, vale o
//!    caminho correspondente no repositório principal.
//!
//! Numa worktree, o escopo de leitura (e a validação de evidências) usa os arquivos
//! da worktree, que são os que o agente lê, no lugar dos do repositório principal.
//! A base vem da configuração do app ([`crate::config`]).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use cba_index::{Cadastro, Escopo, RepositorioGit, detectar_git};
use cba_kb::frontmatter::{FrontmatterProjeto, TipoDocumento};
use cba_kb::{Base, Documento};

use crate::analises::Analises;
use crate::config::Config;
use crate::dados::DiretorioDados;
use crate::eventos::{DiarioEventos, Notificador};
use crate::tempo;

/// Variável de ambiente com o identificador do projeto, gravada pelo preparo do
/// ambiente no registro do servidor MCP (a mesma de `cba_session::preparo`).
pub const VAR_PROJETO: &str = "CBA_PROJETO";

/// Por que o ambiente não pôde ser montado. As mensagens se dirigem ao agente de
/// IA, que as repassa ao analista, e dizem o que fazer.
#[derive(Debug, thiserror::Error)]
pub enum ErroAmbiente {
    #[error(
        "esta sessão não está na pasta de um projeto cadastrado no Ctower Business \
         Agents (diretório de trabalho: {}). Peça ao analista para cadastrar o projeto \
         no app ou para abrir a sessão pela tela da análise, que já inicia o cliente na \
         pasta certa",
        .0.display()
    )]
    ForaDeProjeto(PathBuf),
    #[error(
        "o projeto `{0}`, indicado na configuração desta sessão (CBA_PROJETO), não \
         está cadastrado no Ctower Business Agents. Peça ao analista para conferir o \
         cadastro do projeto no app, que refaz o preparo do ambiente"
    )]
    ProjetoDesconhecido(String),
    #[error(
        "a pasta da base de conhecimento ainda não foi escolhida. Peça ao analista para \
         abrir o app e concluir o primeiro uso"
    )]
    BaseNaoConfigurada,
    #[error("não foi possível ler a configuração do app: {0}")]
    Config(std::io::Error),
    #[error("não foi possível ler o cadastro dos projetos: {0}")]
    Cadastro(cba_index::Erro),
    #[error("as pastas do projeto cadastrado não estão acessíveis: {0}")]
    Escopo(cba_index::Erro),
    #[error("a base de conhecimento não está acessível: {0}")]
    Base(cba_kb::Erro),
}

impl ErroAmbiente {
    /// Nome curto, para o log (sem caminhos).
    pub fn tipo(&self) -> &'static str {
        match self {
            Self::ForaDeProjeto(_) => "fora_de_projeto",
            Self::ProjetoDesconhecido(_) => "projeto_desconhecido",
            Self::BaseNaoConfigurada => "base_nao_configurada",
            Self::Config(_) => "config",
            Self::Cadastro(_) => "cadastro",
            Self::Escopo(_) => "escopo",
            Self::Base(_) => "base",
        }
    }
}

/// Projeto, base e análises de um processo.
#[derive(Debug, Clone)]
pub struct Ambiente {
    /// Cadastro do projeto (nome, pastas, exclusões).
    pub projeto: cba_index::Projeto,
    /// Caminhos que as ferramentas podem ler (pastas do projeto menos exclusões).
    pub escopo: Escopo,
    pub base: Base,
    /// O projeto dentro da base.
    pub kb: cba_kb::Projeto,
    pub analises: Analises,
    /// Diretório de dados do app (índice da base, cadastro e mapa do projeto).
    pub dados: DiretorioDados,
}

impl Ambiente {
    /// Monta o ambiente do projeto que contém `diretorio`, com os dados do app em
    /// `dados` e as notificações no diário de eventos.
    pub fn do_diretorio(diretorio: &Path, dados: &DiretorioDados) -> Result<Self, ErroAmbiente> {
        Self::do_processo(diretorio, None, dados)
    }

    /// Ambiente de um processo (`cba mcp`): o projeto `projeto` (o valor de
    /// [`VAR_PROJETO`]), se informado, ou o que contém `diretorio`.
    pub fn do_processo(
        diretorio: &Path,
        projeto: Option<&str>,
        dados: &DiretorioDados,
    ) -> Result<Self, ErroAmbiente> {
        let notificador = Arc::new(DiarioEventos::em(dados.eventos()));
        Self::do_processo_com(diretorio, projeto, dados, notificador)
    }

    /// Como [`Ambiente::do_diretorio`], com outro destino para as notificações.
    pub fn do_diretorio_com(
        diretorio: &Path,
        dados: &DiretorioDados,
        notificador: Arc<dyn Notificador>,
    ) -> Result<Self, ErroAmbiente> {
        Self::do_processo_com(diretorio, None, dados, notificador)
    }

    /// Como [`Ambiente::do_processo`], com outro destino para as notificações.
    pub fn do_processo_com(
        diretorio: &Path,
        projeto: Option<&str>,
        dados: &DiretorioDados,
        notificador: Arc<dyn Notificador>,
    ) -> Result<Self, ErroAmbiente> {
        let cadastro = Cadastro::em(dados.projetos());
        let projeto = match projeto.map(str::trim).filter(|s| !s.is_empty()) {
            Some(id) => cadastro
                .obter(id)
                .map_err(ErroAmbiente::Cadastro)?
                .ok_or_else(|| ErroAmbiente::ProjetoDesconhecido(id.to_owned()))?,
            None => identificar_com_worktree(diretorio, &cadastro)?,
        };
        let escopo = escopo_na_worktree(&projeto, detectar_git(diretorio).as_ref())?;
        let config = Config::ler(dados).map_err(ErroAmbiente::Config)?;
        let raiz = config.base().ok_or(ErroAmbiente::BaseNaoConfigurada)?;
        let base = Base::abrir(raiz).map_err(ErroAmbiente::Base)?;
        let kb = garantir_na_base(&base, &projeto).map_err(ErroAmbiente::Base)?;
        Ok(Self {
            analises: Analises::new(kb.clone(), notificador),
            projeto,
            escopo,
            base,
            kb,
            dados: dados.clone(),
        })
    }
}

/// Projeto que contém `diretorio`, ou, se o diretório estiver numa worktree do git
/// fora das pastas cadastradas, o que contém o caminho correspondente no
/// repositório principal.
fn identificar_com_worktree(
    diretorio: &Path,
    cadastro: &Cadastro,
) -> Result<cba_index::Projeto, ErroAmbiente> {
    match identificar(diretorio, cadastro) {
        Ok((projeto, _)) => Ok(projeto),
        Err(ErroAmbiente::ForaDeProjeto(dir)) => {
            let no_principal = detectar_git(diretorio)
                .filter(|g| g.worktree)
                .and_then(|g| {
                    let atual = dunce::canonicalize(diretorio).ok()?;
                    let resto = atual.strip_prefix(&g.raiz).ok()?;
                    Some(g.raiz_principal.join(resto))
                });
            match no_principal {
                Some(caminho) => match identificar(&caminho, cadastro) {
                    Ok((projeto, _)) => Ok(projeto),
                    Err(ErroAmbiente::ForaDeProjeto(_)) => Err(ErroAmbiente::ForaDeProjeto(dir)),
                    Err(e) => Err(e),
                },
                None => Err(ErroAmbiente::ForaDeProjeto(dir)),
            }
        }
        Err(e) => Err(e),
    }
}

/// Escopo do projeto. Se o processo roda numa worktree do git, cada pasta do
/// projeto que fica no repositório principal dessa worktree é trocada pela pasta
/// correspondente na worktree (quando existe), para as ferramentas lerem e as
/// evidências citarem o código que o agente vê.
fn escopo_na_worktree(
    projeto: &cba_index::Projeto,
    git: Option<&RepositorioGit>,
) -> Result<Escopo, ErroAmbiente> {
    let mut copia = projeto.clone();
    if let Some(git) = git.filter(|g| g.worktree) {
        for pasta in &mut copia.pastas {
            let Ok(resto) = pasta.caminho.strip_prefix(&git.raiz_principal) else {
                continue;
            };
            // Uma pasta cadastrada que já está dentro da worktree fica como está.
            if pasta.caminho.starts_with(&git.raiz) {
                continue;
            }
            let na_worktree = git.raiz.join(resto);
            if na_worktree.is_dir() {
                pasta.caminho = na_worktree;
            }
        }
    }
    copia.escopo().map_err(ErroAmbiente::Escopo)
}

/// Projeto cadastrado que contém `diretorio`. Se mais de um contiver (pastas de
/// projetos diferentes aninhadas), vence a pasta mais específica.
pub fn identificar(
    diretorio: &Path,
    cadastro: &Cadastro,
) -> Result<(cba_index::Projeto, Escopo), ErroAmbiente> {
    let dir = dunce::canonicalize(diretorio).unwrap_or_else(|_| diretorio.to_path_buf());
    let projetos = cadastro.listar().map_err(ErroAmbiente::Cadastro)?;
    let mut melhor: Option<(usize, cba_index::Projeto, Escopo)> = None;
    let mut inacessivel = None;
    for projeto in projetos {
        let contem = |p: &Path| dir.starts_with(p);
        let escopo = match projeto.escopo() {
            Ok(e) => e,
            // Projeto cujas pastas sumiram ou mudaram de lugar: só é erro se for o
            // do diretório e nenhum outro servir.
            Err(e) => {
                if projeto.pastas.iter().any(|p| contem(&p.caminho)) {
                    inacessivel = Some(e);
                }
                continue;
            }
        };
        let profundidade = escopo
            .pastas()
            .iter()
            .filter(|p| contem(&p.caminho))
            .map(|p| p.caminho.components().count())
            .max();
        if let Some(profundidade) = profundidade
            && melhor.as_ref().is_none_or(|(d, ..)| profundidade > *d)
        {
            melhor = Some((profundidade, projeto, escopo));
        }
    }
    match (melhor, inacessivel) {
        (Some((_, projeto, escopo)), _) => Ok((projeto, escopo)),
        (None, Some(e)) => Err(ErroAmbiente::Escopo(e)),
        (None, None) => Err(ErroAmbiente::ForaDeProjeto(diretorio.to_path_buf())),
    }
}

/// O projeto na base; cria a pasta, o `projeto.md` e o `regras-do-projeto.md` se
/// ainda não existirem.
fn garantir_na_base(
    base: &Base,
    projeto: &cba_index::Projeto,
) -> Result<cba_kb::Projeto, cba_kb::Erro> {
    match base.projeto(&projeto.id) {
        Ok(s) => {
            // Projetos criados antes da #31 ganham o arquivo de regras do projeto;
            // uma falha aqui não impede o servidor de subir (o log não leva o
            // erro, que cita o projeto e o caminho).
            if s.garantir_regras_do_projeto().is_err() {
                tracing::warn!("regras do projeto não criadas na base");
            }
            Ok(s)
        }
        Err(cba_kb::Erro::ProjetoInexistente(_)) => base.criar_projeto(&Documento::novo(
            FrontmatterProjeto {
                tipo: TipoDocumento::Projeto,
                id: projeto.id.clone(),
                nome: projeto.nome.clone(),
                areas: Vec::new(),
                criado: Some(tempo::hoje()),
            },
            format!("# {}\n", projeto.nome),
        )),
        Err(e) => Err(e),
    }
}
