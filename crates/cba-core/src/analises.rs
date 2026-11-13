//! Persistência das análises de um projeto, com notificação ao app.
//!
//! Cada análise tem dois arquivos na pasta `analises/` do projeto na base:
//!
//! - `A-0001.md`: o documento, com o frontmatter da análise (spec, seção 7.2). O
//!   corpo é montado a partir do template do tipo (#9); aqui só se cria o documento
//!   e se mantém o `status` do frontmatter de acordo com o estado.
//! - `A-0001.estado.json`: o estado completo ([`Analise`]): sessões, afirmações,
//!   evidências, perguntas e sugestões. É a fonte de verdade dos registros, gravada a
//!   cada mudança (gravação contínua, spec seção 7.1). Fica na base, e não no
//!   diretório de dados, para a base continuar sendo o que basta guardar: o índice
//!   SQLite (#8) é reconstruído a partir dela.
//!
//! # Concorrência
//!
//! O app, o servidor MCP (um processo por sessão) e o receptor de hooks gravam na
//! mesma análise. Toda alteração é ler, mudar e gravar sob uma trava exclusiva de
//! arquivo (`analises/.trava`), e a gravação é atômica (temporário e renomeação).
//! Assim, duas sessões simultâneas não perdem registros uma da outra e quem lê sem
//! trava nunca vê um arquivo pela metade. O identificador novo é reservado sem
//! colisão por [`cba_kb::Projeto::criar_numerado`].

use std::fs::{self, File, OpenOptions};
use std::io;
use std::sync::Arc;

use cba_kb::frontmatter::{Demanda, FrontmatterAnalise, VersaoCodigo};
use cba_kb::{CaminhoRelativo, Documento, Id, Serie, layout};

use crate::analise::{Analise, ErroModelo, Fluxo};
use crate::eventos::{Evento, Mudanca, Notificador};
use crate::tempo;

/// Arquivo de trava das alterações, na pasta `analises/`.
const TRAVA: &str = ".trava";
/// Sufixo do arquivo de estado (`A-0001.estado.json`).
const SUFIXO_ESTADO: &str = ".estado.json";

/// Erro ao ler ou gravar uma análise.
#[derive(Debug, thiserror::Error)]
pub enum Erro {
    #[error(
        "a análise {0} não existe neste projeto; confira o identificador (A-NNNN) \
         informado pelo analista ou liste as análises do projeto"
    )]
    NaoEncontrada(Id),
    #[error("`{0}` não é o identificador de uma análise; use a forma A-NNNN, como A-0001")]
    NaoEAnalise(Id),
    #[error(transparent)]
    Modelo(#[from] ErroModelo),
    #[error("{arquivo}: estado da análise ilegível: {mensagem}")]
    EstadoIlegivel {
        arquivo: CaminhoRelativo,
        mensagem: String,
    },
    #[error("{arquivo}: {erro}")]
    Io {
        arquivo: CaminhoRelativo,
        #[source]
        erro: io::Error,
    },
    #[error(transparent)]
    Base(#[from] cba_kb::Erro),
}

/// Dados de uma análise nova (spec, seção 4.2, passo 2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NovaAnalise {
    pub fluxo: Fluxo,
    pub titulo: String,
    pub entrada: String,
    pub analista: Option<String>,
    /// Branch e commit do código analisado, quando conhecidos.
    pub base: Option<VersaoCodigo>,
}

/// Análises de um projeto na base.
#[derive(Debug, Clone)]
pub struct Analises {
    kb: cba_kb::Projeto,
    notificador: Arc<dyn Notificador>,
}

impl Analises {
    pub fn new(kb: cba_kb::Projeto, notificador: Arc<dyn Notificador>) -> Self {
        Self { kb, notificador }
    }

    /// Projeto na base.
    pub fn projeto(&self) -> &cba_kb::Projeto {
        &self.kb
    }

    /// Arquivo de estado de uma análise, relativo à raiz do projeto.
    pub fn arquivo_de_estado(id: &Id) -> CaminhoRelativo {
        CaminhoRelativo::novo(&format!("{}/{id}{SUFIXO_ESTADO}", layout::ANALISES))
            .expect("pasta e identificador formam um caminho válido")
    }

    /// Cria a análise: reserva o próximo `A-NNNN`, grava o documento em rascunho e o
    /// estado inicial.
    pub fn criar(&self, nova: NovaAnalise) -> Result<Analise, Erro> {
        self.criar_com_demanda(nova, None)
    }

    /// Como [`Analises::criar`], com a origem da demanda no frontmatter
    /// (`demanda.origem` e `demanda.chave`). Sem ela, a demanda é o texto colado.
    pub fn criar_com_demanda(
        &self,
        nova: NovaAnalise,
        demanda: Option<Demanda>,
    ) -> Result<Analise, Erro> {
        if nova.titulo.trim().is_empty() {
            return Err(ErroModelo::CampoVazio("titulo").into());
        }
        if nova.entrada.trim().is_empty() {
            return Err(ErroModelo::CampoVazio("entrada").into());
        }
        let agora = tempo::agora_ms();
        let titulo = nova.titulo.trim().to_owned();
        let (id, _) = self.kb.criar_numerado(&Serie::Analise, |id| {
            Documento::novo(
                FrontmatterAnalise {
                    tipo: nova.fluxo.tipo_documento(),
                    id: id.clone(),
                    titulo: titulo.clone(),
                    projeto: self.kb.nome().to_owned(),
                    demanda: demanda.clone().unwrap_or_else(|| Demanda {
                        id: None,
                        origem: "texto".into(),
                        chave: None,
                    }),
                    status: cba_kb::frontmatter::StatusAnalise::Rascunho,
                    abrangencia: None,
                    base: nova.base.clone(),
                    criado: tempo::data_utc(agora),
                    analista: nova.analista.clone(),
                },
                format!("# {titulo}\n"),
            )
        })?;
        let analise = Analise::nova(
            id,
            self.kb.nome(),
            nova.fluxo,
            titulo,
            nova.entrada.trim(),
            agora,
        );
        {
            let _trava = self.travar()?;
            self.gravar_estado(&analise, true)?;
        }
        self.notificar(&analise, Mudanca::Criada);
        Ok(analise)
    }

    /// Lê o estado atual de uma análise.
    pub fn carregar(&self, id: &Id) -> Result<Analise, Erro> {
        if id.serie() != &Serie::Analise {
            return Err(Erro::NaoEAnalise(id.clone()));
        }
        let relativo = Self::arquivo_de_estado(id);
        let bytes = match fs::read(self.kb.caminho(&relativo)) {
            Ok(b) => b,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return Err(Erro::NaoEncontrada(id.clone()));
            }
            Err(erro) => {
                return Err(Erro::Io {
                    arquivo: self.kb.na_base(&relativo),
                    erro,
                });
            }
        };
        let analise: Analise =
            serde_json::from_slice(&bytes).map_err(|e| Erro::EstadoIlegivel {
                arquivo: self.kb.na_base(&relativo),
                mensagem: e.to_string(),
            })?;
        if &analise.id != id {
            return Err(Erro::EstadoIlegivel {
                arquivo: self.kb.na_base(&relativo),
                mensagem: format!("o arquivo é da análise {}", analise.id),
            });
        }
        Ok(analise)
    }

    /// Altera uma análise sob a trava: lê o estado atual, aplica `alterar` (que
    /// recebe o instante atual em ms), grava e notifica o app. Se `alterar` recusar,
    /// nada é gravado. Devolve o resultado de `alterar` e a análise gravada.
    pub fn alterar<T>(
        &self,
        id: &Id,
        mudanca: Mudanca,
        alterar: impl FnOnce(&mut Analise, u64) -> Result<T, ErroModelo>,
    ) -> Result<(T, Analise), Erro> {
        self.alterar_e(id, mudanca, alterar, |_| ())
    }

    /// Como [`Analises::alterar`], e ainda executa `depois` com a análise gravada,
    /// sob a mesma trava e antes da notificação. Serve para remontar o documento da
    /// análise: duas sessões gravando juntas não deixam o documento mais velho que o
    /// estado, e o app, ao ser notificado, já encontra o documento novo.
    pub fn alterar_e<T>(
        &self,
        id: &Id,
        mudanca: Mudanca,
        alterar: impl FnOnce(&mut Analise, u64) -> Result<T, ErroModelo>,
        depois: impl FnOnce(&Analise),
    ) -> Result<(T, Analise), Erro> {
        let analise = {
            let _trava = self.travar()?;
            let mut analise = self.carregar(id)?;
            let estado_anterior = analise.estado;
            let agora = tempo::agora_ms().max(analise.atualizada_ms);
            let resultado = alterar(&mut analise, agora)?;
            analise.versao += 1;
            analise.atualizada_ms = agora;
            self.gravar_estado(&analise, false)?;
            if analise.estado.status_documento() != estado_anterior.status_documento() {
                self.atualizar_status_do_documento(&analise)?;
            }
            depois(&analise);
            (resultado, analise)
        };
        self.notificar(&analise.1, mudanca);
        Ok(analise)
    }

    /// Todas as análises do projeto, em ordem de identificador. Estados ilegíveis
    /// são ignorados (e registrados no log, sem conteúdo).
    pub fn listar(&self) -> Result<Vec<Analise>, Erro> {
        let pasta = CaminhoRelativo::novo(layout::ANALISES).expect("nome fixo válido");
        let entradas = match fs::read_dir(self.kb.caminho(&pasta)) {
            Ok(e) => e,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(erro) => {
                return Err(Erro::Io {
                    arquivo: self.kb.na_base(&pasta),
                    erro,
                });
            }
        };
        let mut analises = Vec::new();
        for entrada in entradas.flatten() {
            let nome = entrada.file_name();
            let Some(id) = nome
                .to_str()
                .and_then(|n| n.strip_suffix(SUFIXO_ESTADO))
                .and_then(|n| Id::analisar(n).ok())
            else {
                continue;
            };
            match self.carregar(&id) {
                Ok(a) => analises.push(a),
                Err(e) => tracing::warn!(analise = %id, erro = tipo_do_erro(&e), "estado ignorado"),
            }
        }
        analises.sort_by_key(|a| a.id.numero());
        Ok(analises)
    }

    fn travar(&self) -> Result<File, Erro> {
        let relativo = CaminhoRelativo::novo(&format!("{}/{TRAVA}", layout::ANALISES))
            .expect("nome fixo válido");
        let caminho = self.kb.caminho(&relativo);
        let io_erro = |erro| Erro::Io {
            arquivo: self.kb.na_base(&relativo),
            erro,
        };
        if let Some(pasta) = caminho.parent() {
            fs::create_dir_all(pasta).map_err(io_erro)?;
        }
        let arquivo = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&caminho)
            .map_err(io_erro)?;
        // Liberada quando o arquivo é fechado (fim do escopo de quem travou).
        arquivo.lock().map_err(io_erro)?;
        Ok(arquivo)
    }

    fn gravar_estado(&self, analise: &Analise, so_criar: bool) -> Result<(), Erro> {
        let relativo = Self::arquivo_de_estado(&analise.id);
        let mut json = serde_json::to_vec_pretty(analise).map_err(|e| Erro::EstadoIlegivel {
            arquivo: self.kb.na_base(&relativo),
            mensagem: e.to_string(),
        })?;
        json.push(b'\n');
        let destino = self.kb.caminho(&relativo);
        let resultado = if so_criar {
            cba_kb::arquivo::criar(&destino, &json)
        } else {
            cba_kb::arquivo::gravar(&destino, &json)
        };
        resultado.map_err(|erro| Erro::Io {
            arquivo: self.kb.na_base(&relativo),
            erro,
        })
    }

    fn atualizar_status_do_documento(&self, analise: &Analise) -> Result<(), Erro> {
        let mut doc: Documento<FrontmatterAnalise> = self.kb.ler(&analise.documento)?;
        doc.frontmatter.status = analise.estado.status_documento();
        self.kb.gravar(&analise.documento, &doc)?;
        Ok(())
    }

    fn notificar(&self, analise: &Analise, mudanca: Mudanca) {
        let evento = Evento {
            quando_ms: tempo::agora_ms(),
            projeto: analise.projeto.clone(),
            analise: analise.id.to_string(),
            mudanca,
            versao: analise.versao,
            processo: std::process::id(),
        };
        if let Err(e) = self.notificador.notificar(&evento) {
            tracing::warn!(erro = %e.kind(), "falha ao notificar o app");
        }
    }
}

/// Nome curto do tipo de erro, para o log (sem caminhos nem conteúdo).
pub fn tipo_do_erro(erro: &Erro) -> &'static str {
    match erro {
        Erro::NaoEncontrada(_) => "nao_encontrada",
        Erro::NaoEAnalise(_) => "nao_e_analise",
        Erro::Modelo(_) => "modelo",
        Erro::EstadoIlegivel { .. } => "estado_ilegivel",
        Erro::Io { .. } => "io",
        Erro::Base(_) => "base",
    }
}
