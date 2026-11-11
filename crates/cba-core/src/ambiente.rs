//! Ambiente de trabalho de um processo: qual sistema, qual base, quais análises.
//!
//! O cliente de IA inicia `cba mcp` (e `cba hook`) na pasta do sistema analisado
//! (spec, seções 4.2 e 6.2). O sistema é identificado pelo diretório de trabalho:
//! é o sistema cadastrado que tem uma pasta contendo esse diretório. A base vem da
//! configuração do app ([`crate::config`]).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use cba_index::{Cadastro, Escopo};
use cba_kb::frontmatter::{FrontmatterSistema, TipoDocumento};
use cba_kb::{Base, Documento};

use crate::analises::Analises;
use crate::config::Config;
use crate::dados::DiretorioDados;
use crate::eventos::{DiarioEventos, Notificador};
use crate::tempo;

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
        let notificador = Arc::new(DiarioEventos::em(dados.eventos()));
        Self::do_diretorio_com(diretorio, dados, notificador)
    }

    /// Como [`Ambiente::do_diretorio`], com outro destino para as notificações.
    pub fn do_diretorio_com(
        diretorio: &Path,
        dados: &DiretorioDados,
        notificador: Arc<dyn Notificador>,
    ) -> Result<Self, ErroAmbiente> {
        let (sistema, escopo) = identificar(diretorio, &Cadastro::em(dados.sistemas()))?;
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

/// O sistema na base; cria a pasta e o `sistema.md` se ainda não existirem.
fn garantir_na_base(
    base: &Base,
    sistema: &cba_index::Sistema,
) -> Result<cba_kb::Sistema, cba_kb::Erro> {
    match base.sistema(&sistema.id) {
        Ok(s) => Ok(s),
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
