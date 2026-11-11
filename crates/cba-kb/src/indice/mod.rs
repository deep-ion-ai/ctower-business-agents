//! Índice SQLite da base de conhecimento, com busca FTS5 (spec, seções 7.5 e 11).
//!
//! O índice é só um espelho: a base (os Markdown) é a fonte da verdade. Ele fica no
//! diretório de dados do app, não na base, e apagá-lo não perde nada: o próximo
//! [`Indice::sincronizar`] o reconstrói a partir dos arquivos.
//!
//! - **O que entra:** os documentos `.md` de cada sistema da base
//!   ([`crate::Base::sistemas`] e [`crate::Sistema::documentos`]). O
//!   `regras-do-sistema.md`, que não tem frontmatter, entra como o tipo à parte
//!   [`Tipo::RegrasDoSistema`], sem os comentários HTML. Documento ilegível
//!   (frontmatter inválido, arquivo que não é UTF-8) não interrompe nada: fica em
//!   [`Indice::problemas`] e, se o frontmatter ao menos for YAML, continua na busca.
//! - **Atualização incremental:** a cada sincronização, o tamanho e a data de
//!   modificação de cada arquivo são comparados com os guardados; só os que mudaram
//!   são lidos, e só os de hash (SHA-256) diferente são indexados de novo. Arquivos
//!   que sumiram saem do índice. Sem mudança, nada é gravado.
//! - **Edições fora do app:** a [`Vigia`] sincroniza em intervalos curtos numa
//!   thread própria. Quem não mantém uma vigia (o servidor MCP, por exemplo) chama
//!   [`Indice::sincronizar`] antes de buscar, o que custa pouco quando nada mudou.
//! - **Processos ao mesmo tempo:** o app, o servidor MCP e o receptor de hooks
//!   abrem o mesmo arquivo. O banco usa o modo WAL (leitores não esperam o escritor)
//!   e as gravações são feitas numa transação `IMMEDIATE`, com espera se outro
//!   processo estiver gravando.
//! - **Busca:** FTS5 com o tokenizador `unicode61 remove_diacritics 2`, que tira os
//!   acentos do texto e da consulta (`carencia` encontra `carência`). Veja
//!   [`Consulta`].
//! - **Esquema:** versão em `PRAGMA user_version`. Numa versão diferente, as tabelas
//!   são recriadas e tudo é indexado de novo, sem migração: o índice é descartável.
//!   O mesmo vale para outra pasta de base e para um arquivo corrompido.
//!
//! Entidades da spec (seção 11) cobertas aqui: `documento` (com tipo, identificador,
//! título, área, status, frontmatter em JSON e hash), `regra`, `termo` e `analise`
//! como linhas de `documento` do tipo correspondente, as evidências das regras e as
//! ligações entre documentos (`origem`, `analise`).

mod busca;
mod esquema;
mod extracao;
mod vigia;

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, ErrorCode, OptionalExtension, TransactionBehavior, params};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::base::Base;
use crate::caminho::CaminhoRelativo;
use crate::frontmatter::TipoDocumento;

pub use busca::{Consulta, Modo, Pagina, Resultado};
pub use vigia::Vigia;

/// Nome do arquivo do índice, dentro da pasta de índice do diretório de dados
/// (`cba_core::dados::DiretorioDados::arquivo_do_indice`).
pub const ARQUIVO: &str = "base.sqlite";

/// Quanto esperar por outro processo que esteja gravando no índice.
const ESPERA_POR_TRAVA: Duration = Duration::from_secs(10);

/// Arquivo modificado há menos que isto (em relação à última leitura) é lido de
/// novo mesmo com tamanho e data iguais: uma edição no mesmo instante da leitura
/// anterior pode não ter mudado a data, conforme a precisão do sistema de arquivos.
const JANELA_DE_INCERTEZA_NS: i64 = 2_000_000_000;

/// Erro do índice.
#[derive(Debug, thiserror::Error)]
pub enum ErroIndice {
    /// Falha do SQLite.
    #[error("índice: {0}")]
    Banco(#[from] rusqlite::Error),
    /// Falha ao criar a pasta do índice ou ao apagar um índice corrompido.
    #[error("índice ({}): {erro}", caminho.display())]
    Io {
        caminho: PathBuf,
        #[source]
        erro: io::Error,
    },
    /// Falha ao listar a base.
    #[error(transparent)]
    Base(#[from] crate::Erro),
}

pub type ResultadoIndice<T> = Result<T, ErroIndice>;

/// Tipo de um documento no índice: um [`TipoDocumento`] do frontmatter ou o
/// `regras-do-sistema.md`, que não tem frontmatter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tipo {
    Documento(TipoDocumento),
    /// `<sistema>/regras-do-sistema.md`.
    RegrasDoSistema,
}

impl Tipo {
    /// Todos os tipos.
    pub fn todos() -> impl Iterator<Item = Tipo> {
        TipoDocumento::TODOS
            .into_iter()
            .map(Tipo::Documento)
            .chain([Tipo::RegrasDoSistema])
    }

    /// Tipos das análises (`analises/`): análise de impacto, pergunta e
    /// funcionalidade.
    pub const ANALISES: [Tipo; 3] = [
        Tipo::Documento(TipoDocumento::AnaliseImpacto),
        Tipo::Documento(TipoDocumento::Pergunta),
        Tipo::Documento(TipoDocumento::Funcionalidade),
    ];

    /// Valor guardado no índice: o do campo `tipo` ou `regras-do-sistema`.
    pub fn como_str(self) -> &'static str {
        match self {
            Tipo::Documento(t) => t.como_str(),
            Tipo::RegrasDoSistema => "regras-do-sistema",
        }
    }

    pub fn de_str(texto: &str) -> Option<Self> {
        Self::todos().find(|t| t.como_str() == texto)
    }
}

impl Serialize for Tipo {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.como_str())
    }
}

/// O que uma sincronização fez.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Sincronizacao {
    /// Documentos que entraram no índice.
    pub novos: usize,
    /// Documentos indexados de novo porque o conteúdo mudou.
    pub alterados: usize,
    /// Documentos que saíram porque o arquivo sumiu.
    pub removidos: usize,
    /// Documentos sem mudança de conteúdo.
    pub inalterados: usize,
}

impl Sincronizacao {
    /// Se o índice mudou.
    pub fn mudou(&self) -> bool {
        self.novos + self.alterados + self.removidos > 0
    }
}

/// Documento com problema de leitura (frontmatter inválido, arquivo que não é
/// UTF-8).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Problema {
    /// Caminho relativo à base.
    pub caminho: CaminhoRelativo,
    pub erro: String,
}

/// Ligação entre dois documentos, pelos campos `origem` (documento que originou
/// uma regra, um termo ou uma funcionalidade) e `analise` (análise de uma
/// sugestão) do frontmatter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Ligacao {
    /// Documento que cita (relativo à base).
    pub de: CaminhoRelativo,
    /// Documento citado (relativo à base).
    pub para: CaminhoRelativo,
    /// Campo do frontmatter: `origem` ou `analise`.
    pub campo: String,
    /// Se o documento citado está no índice.
    pub existe: bool,
}

/// Evidência de uma regra, como está no frontmatter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EvidenciaIndexada {
    /// Documento da regra (relativo à base).
    pub documento: CaminhoRelativo,
    pub sistema: String,
    /// Identificador da regra.
    pub regra: Option<String>,
    /// Arquivo do sistema analisado, relativo à raiz do sistema.
    pub arquivo: String,
    pub linha_inicial: u32,
    pub linha_final: u32,
    pub hash: String,
    pub commit: Option<String>,
}

/// Índice aberto: uma conexão com o arquivo SQLite.
#[derive(Debug)]
pub struct Indice {
    conexao: Connection,
    base: Base,
    arquivo: PathBuf,
}

/// Arquivo da base encontrado na listagem.
struct Encontrado {
    caminho: String,
    sistema: String,
    relativo: CaminhoRelativo,
    disco: PathBuf,
    tamanho: i64,
    modificado_ns: i64,
}

/// Estado guardado de um documento.
struct Guardado {
    id: i64,
    hash: String,
    tamanho: i64,
    modificado_ns: i64,
    verificado_ns: i64,
}

impl Guardado {
    fn igual(&self, e: &Encontrado) -> bool {
        self.tamanho == e.tamanho
            && self.modificado_ns == e.modificado_ns
            && self.verificado_ns - self.modificado_ns > JANELA_DE_INCERTEZA_NS
    }
}

impl Indice {
    /// Abre (ou cria) o índice da base no arquivo indicado. Cria a pasta se
    /// faltar. Esquema de outra versão, outra base ou arquivo corrompido fazem o
    /// índice começar vazio; chame [`Indice::sincronizar`] para preenchê-lo.
    pub fn abrir(arquivo: impl Into<PathBuf>, base: &Base) -> ResultadoIndice<Self> {
        let arquivo = arquivo.into();
        if let Some(pasta) = arquivo.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(pasta).map_err(|erro| ErroIndice::Io {
                caminho: pasta.to_path_buf(),
                erro,
            })?;
        }
        match Self::conectar_com_espera(&arquivo, base) {
            Err(ErroIndice::Banco(e)) if corrompido(&e) => {
                tracing::warn!(erro = %e, "índice ilegível; apagando para reconstruir");
                apagar_arquivos(&arquivo)?;
                Self::conectar_com_espera(&arquivo, base)
            }
            outro => outro,
        }
    }

    /// Na criação do arquivo, a passagem para o modo WAL e o esquema disputam o
    /// banco com os outros processos que abrem ao mesmo tempo, e o SQLite pode
    /// recusar na hora, sem esperar pela trava. Tenta de novo até o limite.
    fn conectar_com_espera(arquivo: &Path, base: &Base) -> ResultadoIndice<Self> {
        let limite = Instant::now() + ESPERA_POR_TRAVA;
        loop {
            match Self::conectar(arquivo, base) {
                Err(ErroIndice::Banco(e)) if disputado(&e) && Instant::now() < limite => {
                    thread::sleep(Duration::from_millis(20));
                }
                outro => return outro,
            }
        }
    }

    fn conectar(arquivo: &Path, base: &Base) -> ResultadoIndice<Self> {
        let conexao = Connection::open(arquivo)?;
        conexao.busy_timeout(ESPERA_POR_TRAVA)?;
        // O modo WAL fica gravado no arquivo: só muda na primeira vez.
        let mut modo: String = conexao.query_row("PRAGMA journal_mode", [], |l| l.get(0))?;
        if !modo.eq_ignore_ascii_case("wal") {
            modo = conexao.query_row("PRAGMA journal_mode = WAL", [], |l| l.get(0))?;
        }
        if !modo.eq_ignore_ascii_case("wal") {
            tracing::warn!(modo, "o índice não aceitou o modo WAL");
        }
        conexao.pragma_update(None, "synchronous", "NORMAL")?;
        let mut indice = Self {
            conexao,
            base: base.clone(),
            arquivo: arquivo.to_path_buf(),
        };
        indice.preparar()?;
        Ok(indice)
    }

    /// Cria ou recria o esquema e confere a base.
    fn preparar(&mut self) -> ResultadoIndice<()> {
        let raiz = self.base.raiz().to_string_lossy().into_owned();
        let tx = self
            .conexao
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let versao: i64 = tx.query_row("PRAGMA user_version", [], |l| l.get(0))?;
        if versao != esquema::VERSAO {
            tx.execute_batch(esquema::APAGAR)?;
            tx.execute_batch(esquema::CRIAR)?;
            tx.pragma_update(None, "user_version", esquema::VERSAO)?;
        }
        let guardada: Option<String> = tx
            .query_row("SELECT valor FROM meta WHERE chave = 'base'", [], |l| {
                l.get(0)
            })
            .optional()?;
        if guardada.as_deref() != Some(raiz.as_str()) {
            tx.execute_batch(esquema::LIMPAR)?;
            tx.execute(
                "INSERT OR REPLACE INTO meta (chave, valor) VALUES ('base', ?1)",
                [&raiz],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Arquivo do índice.
    pub fn arquivo(&self) -> &Path {
        &self.arquivo
    }

    /// Base indexada.
    pub fn base(&self) -> &Base {
        &self.base
    }

    /// Atualiza o índice com o que mudou na base desde a última sincronização.
    pub fn sincronizar(&mut self) -> ResultadoIndice<Sincronizacao> {
        self.sincronizar_com(false)
    }

    /// Apaga tudo e indexa a base inteira de novo, numa transação só (quem lê ao
    /// mesmo tempo vê o índice anterior até o fim).
    pub fn reconstruir(&mut self) -> ResultadoIndice<Sincronizacao> {
        self.sincronizar_com(true)
    }

    fn sincronizar_com(&mut self, do_zero: bool) -> ResultadoIndice<Sincronizacao> {
        let encontrados = self.listar_base()?;
        if !do_zero {
            // Primeiro sem trava: se nada mudou, não grava nada.
            let guardados = guardados(&self.conexao)?;
            let nada_mudou = encontrados.len() == guardados.len()
                && encontrados
                    .iter()
                    .all(|e| guardados.get(&e.caminho).is_some_and(|g| g.igual(e)));
            if nada_mudou {
                return Ok(Sincronizacao {
                    inalterados: encontrados.len(),
                    ..Default::default()
                });
            }
        }

        let tx = self
            .conexao
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if do_zero {
            tx.execute_batch(esquema::LIMPAR)?;
        }
        // De novo dentro da transação: outro processo pode ter acabado de gravar.
        let mut guardados = guardados(&tx)?;
        let mut resumo = Sincronizacao::default();
        let agora = agora_ns();
        for e in &encontrados {
            let anterior = guardados.remove(&e.caminho);
            if let Some(g) = &anterior
                && g.igual(e)
            {
                resumo.inalterados += 1;
                continue;
            }
            let bytes = match fs::read(&e.disco) {
                Ok(b) => b,
                // Sumiu entre a listagem e a leitura: sai na próxima sincronização.
                Err(erro) if erro.kind() == io::ErrorKind::NotFound => {
                    if let Some(g) = anterior {
                        guardados.insert(e.caminho.clone(), g);
                    }
                    continue;
                }
                Err(erro) => {
                    return Err(ErroIndice::Io {
                        caminho: e.disco.clone(),
                        erro,
                    });
                }
            };
            let hash = hash(&bytes);
            match anterior {
                Some(g) if g.hash == hash => {
                    tx.execute(
                        "UPDATE documento SET tamanho = ?1, modificado_ns = ?2, verificado_ns = ?3 \
                         WHERE id = ?4",
                        params![e.tamanho, e.modificado_ns, agora, g.id],
                    )?;
                    resumo.inalterados += 1;
                }
                anterior => {
                    if let Some(g) = &anterior {
                        esquema::remover(&tx, g.id)?;
                        resumo.alterados += 1;
                    } else {
                        resumo.novos += 1;
                    }
                    let sistema = self.base.sistema(&e.sistema)?;
                    let registro = extracao::extrair(&sistema, &e.relativo, bytes);
                    esquema::inserir(
                        &tx,
                        &registro,
                        &esquema::Arquivo {
                            hash: &hash,
                            tamanho: e.tamanho,
                            modificado_ns: e.modificado_ns,
                            verificado_ns: agora,
                        },
                    )?;
                }
            }
        }
        for g in guardados.values() {
            esquema::remover(&tx, g.id)?;
            resumo.removidos += 1;
        }
        tx.commit()?;
        if resumo.mudou() {
            tracing::debug!(?resumo, "índice da base sincronizado");
        }
        Ok(resumo)
    }

    /// Documentos com problema de leitura, em ordem de caminho.
    pub fn problemas(&self) -> ResultadoIndice<Vec<Problema>> {
        let mut consulta = self.conexao.prepare(
            "SELECT caminho, erro FROM documento WHERE erro IS NOT NULL ORDER BY caminho",
        )?;
        let linhas =
            consulta.query_map([], |l| Ok((l.get::<_, String>(0)?, l.get::<_, String>(1)?)))?;
        let mut saida = Vec::new();
        for linha in linhas {
            let (caminho, erro) = linha?;
            if let Ok(caminho) = CaminhoRelativo::novo(&caminho) {
                saida.push(Problema { caminho, erro });
            }
        }
        Ok(saida)
    }

    /// Ligações que saem do documento ou chegam a ele (caminho relativo à base),
    /// em ordem.
    pub fn ligacoes(&self, caminho: &CaminhoRelativo) -> ResultadoIndice<Vec<Ligacao>> {
        let mut consulta = self.conexao.prepare(
            "SELECT d.caminho, l.para, l.campo, \
                    EXISTS (SELECT 1 FROM documento x WHERE x.caminho = l.para) \
             FROM ligacao l JOIN documento d ON d.id = l.documento \
             WHERE d.caminho = ?1 OR l.para = ?1 \
             ORDER BY d.caminho, l.para, l.campo",
        )?;
        let linhas = consulta.query_map([caminho.as_str()], |l| {
            Ok((
                l.get::<_, String>(0)?,
                l.get::<_, String>(1)?,
                l.get::<_, String>(2)?,
                l.get::<_, bool>(3)?,
            ))
        })?;
        let mut saida = Vec::new();
        for linha in linhas {
            let (de, para, campo, existe) = linha?;
            if let (Ok(de), Ok(para)) = (CaminhoRelativo::novo(&de), CaminhoRelativo::novo(&para)) {
                saida.push(Ligacao {
                    de,
                    para,
                    campo,
                    existe,
                });
            }
        }
        Ok(saida)
    }

    /// Evidências das regras, de um sistema ou de todos, em ordem de documento.
    pub fn evidencias(&self, sistema: Option<&str>) -> ResultadoIndice<Vec<EvidenciaIndexada>> {
        let mut consulta = self.conexao.prepare(
            "SELECT d.caminho, d.sistema, d.identificador, e.arquivo, e.linha_inicial, \
                    e.linha_final, e.hash, e.commit_ \
             FROM evidencia e JOIN documento d ON d.id = e.documento \
             WHERE ?1 IS NULL OR d.sistema = ?1 \
             ORDER BY d.caminho, e.ordem",
        )?;
        let linhas = consulta.query_map([sistema], |l| {
            Ok((
                l.get::<_, String>(0)?,
                l.get::<_, String>(1)?,
                l.get::<_, Option<String>>(2)?,
                l.get::<_, String>(3)?,
                l.get::<_, u32>(4)?,
                l.get::<_, u32>(5)?,
                l.get::<_, String>(6)?,
                l.get::<_, Option<String>>(7)?,
            ))
        })?;
        let mut saida = Vec::new();
        for linha in linhas {
            let (caminho, sistema, regra, arquivo, linha_inicial, linha_final, hash, commit) =
                linha?;
            if let Ok(documento) = CaminhoRelativo::novo(&caminho) {
                saida.push(EvidenciaIndexada {
                    documento,
                    sistema,
                    regra,
                    arquivo,
                    linha_inicial,
                    linha_final,
                    hash,
                    commit,
                });
            }
        }
        Ok(saida)
    }

    /// Arquivos `.md` de todos os sistemas da base, com tamanho e data.
    fn listar_base(&self) -> ResultadoIndice<Vec<Encontrado>> {
        let mut saida = Vec::new();
        for nome in self.base.sistemas()? {
            let sistema = match self.base.sistema(&nome) {
                Ok(s) => s,
                // Removido entre a listagem e agora.
                Err(crate::Erro::SistemaInexistente(_)) => continue,
                Err(e) => return Err(e.into()),
            };
            let documentos = match sistema.documentos() {
                Ok(d) => d,
                Err(crate::Erro::Io { erro, .. }) if erro.kind() == io::ErrorKind::NotFound => {
                    continue;
                }
                Err(e) => return Err(e.into()),
            };
            for relativo in documentos {
                let disco = sistema.caminho(&relativo);
                let Ok(meta) = fs::metadata(&disco) else {
                    continue;
                };
                saida.push(Encontrado {
                    caminho: sistema.na_base(&relativo).as_str().to_owned(),
                    sistema: nome.clone(),
                    relativo,
                    disco,
                    tamanho: i64::try_from(meta.len()).unwrap_or(i64::MAX),
                    modificado_ns: meta.modified().map(ns_desde_epoca).unwrap_or(0),
                });
            }
        }
        Ok(saida)
    }
}

fn guardados(conexao: &Connection) -> ResultadoIndice<HashMap<String, Guardado>> {
    let mut consulta = conexao.prepare(
        "SELECT caminho, id, hash, tamanho, modificado_ns, verificado_ns FROM documento",
    )?;
    let linhas = consulta.query_map([], |l| {
        Ok((
            l.get::<_, String>(0)?,
            Guardado {
                id: l.get(1)?,
                hash: l.get(2)?,
                tamanho: l.get(3)?,
                modificado_ns: l.get(4)?,
                verificado_ns: l.get(5)?,
            },
        ))
    })?;
    let mut saida = HashMap::new();
    for linha in linhas {
        let (caminho, g) = linha?;
        saida.insert(caminho, g);
    }
    Ok(saida)
}

fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn ns_desde_epoca(instante: SystemTime) -> i64 {
    match instante.duration_since(UNIX_EPOCH) {
        Ok(d) => i64::try_from(d.as_nanos()).unwrap_or(i64::MAX),
        Err(e) => -i64::try_from(e.duration().as_nanos()).unwrap_or(i64::MAX),
    }
}

fn agora_ns() -> i64 {
    ns_desde_epoca(SystemTime::now())
}

fn disputado(erro: &rusqlite::Error) -> bool {
    matches!(
        erro.sqlite_error_code(),
        Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked | ErrorCode::SystemIoFailure)
    )
}

fn corrompido(erro: &rusqlite::Error) -> bool {
    matches!(
        erro.sqlite_error_code(),
        Some(ErrorCode::NotADatabase | ErrorCode::DatabaseCorrupt)
    )
}

/// Apaga o arquivo do índice e os arquivos auxiliares do modo WAL.
fn apagar_arquivos(arquivo: &Path) -> ResultadoIndice<()> {
    for sufixo in ["", "-wal", "-shm"] {
        let mut nome = arquivo.as_os_str().to_owned();
        nome.push(sufixo);
        let caminho = PathBuf::from(nome);
        match fs::remove_file(&caminho) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(erro) => return Err(ErroIndice::Io { caminho, erro }),
        }
    }
    Ok(())
}
