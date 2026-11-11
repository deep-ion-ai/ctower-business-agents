//! Ambiente de trabalho de um processo: qual sistema, qual base, quais análises.
//!
//! O cliente de IA inicia `cba mcp` (e `cba hook`) na pasta do sistema analisado
//! (spec, seções 4.2 e 6.2). O sistema é identificado:
//!
//! 1. pela variável [`VAR_SISTEMA`] (`CBA_SISTEMA`), que o preparo do ambiente
//!    grava no registro do servidor MCP, quando presente;
//! 2. senão, pelo diretório de trabalho: é o sistema cadastrado que tem uma pasta
//!    contendo esse diretório. Numa worktree do git de uma pasta do sistema, vale o
//!    caminho correspondente no repositório principal.
//!
//! Numa worktree, o escopo de leitura (e a validação de evidências) usa os arquivos
//! da worktree, que são os que o agente lê, no lugar dos do repositório principal.
//! A base vem da configuração do app ([`crate::config`]).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use cba_index::{Cadastro, Escopo, RepositorioGit, detectar_git};
use cba_kb::frontmatter::{FrontmatterSistema, TipoDocumento};
use cba_kb::{Base, Documento};

use crate::analises::Analises;
use crate::config::Config;
use crate::dados::DiretorioDados;
use crate::eventos::{DiarioEventos, Notificador};
use crate::tempo;

/// Variável de ambiente com o identificador do sistema, gravada pelo preparo do
/// ambiente no registro do servidor MCP (a mesma de `cba_session::preparo`).
pub const VAR_SISTEMA: &str = "CBA_SISTEMA";

/// Por que o ambiente não pôde ser montado. As mensagens se dirigem ao agente de
/// IA, que as repassa ao analista, e dizem o que fazer.
#[derive(Debug, thiserror::Error)]
pub enum ErroAmbiente {
    #[error(
        "esta sessão não está na pasta de um sistema cadastrado no Ctower Business \
         Agents (diretório de trabalho: {}). Peça ao analista para cadastrar o sistema \
         no app ou para abrir a sessão pela tela da análise, que já inicia o cliente na \
         pasta certa",
        .0.display()
    )]
    ForaDeSistema(PathBuf),
    #[error(
        "o sistema `{0}`, indicado na configuração desta sessão (CBA_SISTEMA), não \
         está cadastrado no Ctower Business Agents. Peça ao analista para conferir o \
         cadastro do sistema no app, que refaz o preparo do ambiente"
    )]
    SistemaDesconhecido(String),
    #[error(
        "a pasta da base de conhecimento ainda não foi escolhida. Peça ao analista para \
         abrir o app e concluir o primeiro uso"
    )]
    BaseNaoConfigurada,
    #[error("não foi possível ler a configuração do app: {0}")]
    Config(std::io::Error),
    #[error("não foi possível ler o cadastro dos sistemas: {0}")]
    Cadastro(cba_index::Erro),
    #[error("as pastas do sistema cadastrado não estão acessíveis: {0}")]
    Escopo(cba_index::Erro),
    #[error("a base de conhecimento não está acessível: {0}")]
    Base(cba_kb::Erro),
}

impl ErroAmbiente {
    /// Nome curto, para o log (sem caminhos).
    pub fn tipo(&self) -> &'static str {
        match self {
            Self::ForaDeSistema(_) => "fora_de_sistema",
            Self::SistemaDesconhecido(_) => "sistema_desconhecido",
            Self::BaseNaoConfigurada => "base_nao_configurada",
            Self::Config(_) => "config",
            Self::Cadastro(_) => "cadastro",
            Self::Escopo(_) => "escopo",
            Self::Base(_) => "base",
        }
    }
}

/// Sistema, base e análises de um processo.
#[derive(Debug, Clone)]
pub struct Ambiente {
    /// Cadastro do sistema (nome, pastas, exclusões).
    pub sistema: cba_index::Sistema,
    /// Caminhos que as ferramentas podem ler (pastas do sistema menos exclusões).
    pub escopo: Escopo,
    pub base: Base,
    /// O sistema dentro da base.
    pub kb: cba_kb::Sistema,
    pub analises: Analises,
}

impl Ambiente {
    /// Monta o ambiente do sistema que contém `diretorio`, com os dados do app em
    /// `dados` e as notificações no diário de eventos.
    pub fn do_diretorio(diretorio: &Path, dados: &DiretorioDados) -> Result<Self, ErroAmbiente> {
        Self::do_processo(diretorio, None, dados)
    }

    /// Ambiente de um processo (`cba mcp`): o sistema `sistema` (o valor de
    /// [`VAR_SISTEMA`]), se informado, ou o que contém `diretorio`.
    pub fn do_processo(
        diretorio: &Path,
        sistema: Option<&str>,
        dados: &DiretorioDados,
    ) -> Result<Self, ErroAmbiente> {
        let notificador = Arc::new(DiarioEventos::em(dados.eventos()));
        Self::do_processo_com(diretorio, sistema, dados, notificador)
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
        sistema: Option<&str>,
        dados: &DiretorioDados,
        notificador: Arc<dyn Notificador>,
    ) -> Result<Self, ErroAmbiente> {
        let cadastro = Cadastro::em(dados.sistemas());
        let sistema = match sistema.map(str::trim).filter(|s| !s.is_empty()) {
            Some(id) => cadastro
                .obter(id)
                .map_err(ErroAmbiente::Cadastro)?
                .ok_or_else(|| ErroAmbiente::SistemaDesconhecido(id.to_owned()))?,
            None => identificar_com_worktree(diretorio, &cadastro)?,
        };
        let escopo = escopo_na_worktree(&sistema, detectar_git(diretorio).as_ref())?;
        let config = Config::ler(dados).map_err(ErroAmbiente::Config)?;
        let raiz = config.base().ok_or(ErroAmbiente::BaseNaoConfigurada)?;
        let base = Base::abrir(raiz).map_err(ErroAmbiente::Base)?;
        let kb = garantir_na_base(&base, &sistema).map_err(ErroAmbiente::Base)?;
        Ok(Self {
            analises: Analises::new(kb.clone(), notificador),
            sistema,
            escopo,
            base,
            kb,
        })
    }
}

/// Sistema que contém `diretorio`, ou, se o diretório estiver numa worktree do git
/// fora das pastas cadastradas, o que contém o caminho correspondente no
/// repositório principal.
fn identificar_com_worktree(
    diretorio: &Path,
    cadastro: &Cadastro,
) -> Result<cba_index::Sistema, ErroAmbiente> {
    match identificar(diretorio, cadastro) {
        Ok((sistema, _)) => Ok(sistema),
        Err(ErroAmbiente::ForaDeSistema(dir)) => {
            let no_principal = detectar_git(diretorio)
                .filter(|g| g.worktree)
                .and_then(|g| {
                    let atual = dunce::canonicalize(diretorio).ok()?;
                    let resto = atual.strip_prefix(&g.raiz).ok()?;
                    Some(g.raiz_principal.join(resto))
                });
            match no_principal {
                Some(caminho) => match identificar(&caminho, cadastro) {
                    Ok((sistema, _)) => Ok(sistema),
                    Err(ErroAmbiente::ForaDeSistema(_)) => Err(ErroAmbiente::ForaDeSistema(dir)),
                    Err(e) => Err(e),
                },
                None => Err(ErroAmbiente::ForaDeSistema(dir)),
            }
        }
        Err(e) => Err(e),
    }
}

/// Escopo do sistema. Se o processo roda numa worktree do git, cada pasta do
/// sistema que fica no repositório principal dessa worktree é trocada pela pasta
/// correspondente na worktree (quando existe), para as ferramentas lerem e as
/// evidências citarem o código que o agente vê.
fn escopo_na_worktree(
    sistema: &cba_index::Sistema,
    git: Option<&RepositorioGit>,
) -> Result<Escopo, ErroAmbiente> {
    let mut copia = sistema.clone();
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

/// Sistema cadastrado que contém `diretorio`. Se mais de um contiver (pastas de
/// sistemas diferentes aninhadas), vence a pasta mais específica.
pub fn identificar(
    diretorio: &Path,
    cadastro: &Cadastro,
) -> Result<(cba_index::Sistema, Escopo), ErroAmbiente> {
    let dir = dunce::canonicalize(diretorio).unwrap_or_else(|_| diretorio.to_path_buf());
    let sistemas = cadastro.listar().map_err(ErroAmbiente::Cadastro)?;
    let mut melhor: Option<(usize, cba_index::Sistema, Escopo)> = None;
    let mut inacessivel = None;
    for sistema in sistemas {
        let contem = |p: &Path| dir.starts_with(p);
        let escopo = match sistema.escopo() {
            Ok(e) => e,
            // Sistema cujas pastas sumiram ou mudaram de lugar: só é erro se for o
            // do diretório e nenhum outro servir.
            Err(e) => {
                if sistema.pastas.iter().any(|p| contem(&p.caminho)) {
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
            melhor = Some((profundidade, sistema, escopo));
        }
    }
    match (melhor, inacessivel) {
        (Some((_, sistema, escopo)), _) => Ok((sistema, escopo)),
        (None, Some(e)) => Err(ErroAmbiente::Escopo(e)),
        (None, None) => Err(ErroAmbiente::ForaDeSistema(diretorio.to_path_buf())),
    }
}

/// O sistema na base; cria a pasta, o `sistema.md` e o `regras-do-sistema.md` se
/// ainda não existirem.
fn garantir_na_base(
    base: &Base,
    sistema: &cba_index::Sistema,
) -> Result<cba_kb::Sistema, cba_kb::Erro> {
    match base.sistema(&sistema.id) {
        Ok(s) => {
            // Sistemas criados antes da #31 ganham o arquivo de regras do sistema;
            // uma falha aqui não impede o servidor de subir (o log não leva o
            // erro, que cita o sistema e o caminho).
            if s.garantir_regras_do_sistema().is_err() {
                tracing::warn!("regras do sistema não criadas na base");
            }
            Ok(s)
        }
        Err(cba_kb::Erro::SistemaInexistente(_)) => base.criar_sistema(&Documento::novo(
            FrontmatterSistema {
                tipo: TipoDocumento::Sistema,
                id: sistema.id.clone(),
                nome: sistema.nome.clone(),
                areas: Vec::new(),
                criado: Some(tempo::hoje()),
            },
            format!("# {}\n", sistema.nome),
        )),
        Err(e) => Err(e),
    }
}
