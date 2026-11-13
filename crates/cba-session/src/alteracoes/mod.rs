//! Aviso de alteração na pasta do projeto (spec, princípios e seção 4.6; #29).
//!
//! O produto analisa e não desenvolve, mas o app não bloqueia ferramentas por
//! padrão: ao fim de cada resposta do agente (`Stop`) e da sessão (`SessionEnd`), o
//! app confere as pastas do projeto contra o retrato tirado no início da sessão e,
//! se algo mudou, avisa na análise, com o diff e a opção de desfazer.
//!
//! As pastas de código-fonte (num repositório git cujo remoto é o repositório de
//! código cadastrado no projeto) não entram no retrato: o aviso cobre só as demais
//! pastas do projeto ([`pastas_da_sessao`]; #130).
//!
//! # Retrato e conferência
//!
//! O retrato é tirado uma vez por sessão, no primeiro `SessionStart` (ou no primeiro
//! evento, se a sessão começou antes de os hooks existirem), e fica em
//! `<dados>/retratos/<session_id>/`, fora do repositório:
//!
//! - **Pastas num repositório git** (o caso normal): o commit atual (`HEAD`) e, para
//!   cada arquivo que já estava diferente do `HEAD` (modificado, novo fora do
//!   `.gitignore` ou removido), o conteúdo naquele instante, guardado por SHA-256 em
//!   `conteudo/`. O conteúdo dos demais arquivos é o do commit, que o git já guarda:
//!   o retrato custa um `git status` e a cópia dos poucos arquivos sujos, e não a
//!   leitura do projeto inteiro.
//! - **Pastas fora do git** (ou sem o `git` instalado): tamanho e data de
//!   modificação de cada arquivo. A conferência lista o que mudou, sem diff nem
//!   desfazer.
//!
//! A conferência ([`conferir`]) junta os candidatos (o que está sujo agora, o que
//! estava sujo no início e, se o `HEAD` mudou, o que difere entre os dois commits) e
//! compara o SHA-256 do arquivo atual com o do conteúdo de base: o guardado no
//! retrato ou, para os demais, o do commit do início (`git cat-file --filters`, com
//! o fim de linha da cópia de trabalho). Assim, uma alteração que já existia antes
//! da sessão não é atribuída a ela, e uma alteração da sessão num arquivo que já
//! estava sujo aparece só com a parte nova no diff.
//!
//! # Custo nos hooks
//!
//! O receptor de hooks não faz nada disso no caminho do evento: ele só dispara
//! `cba fundo retratar|conferir <sessao>` como processo separado e responde
//! ([`disparar`]). Num projeto grande, o `git status` pode passar do segundo; o
//! hook continua abaixo dos 200 ms. Para a primeira escrita não chegar antes do
//! retrato, o `PreToolUse` de uma ferramenta que escreve espera o retrato terminar
//! (até [`ESPERA_DO_RETRATO`]); depois disso, é uma consulta de arquivo.
//!
//! # O que é guardado
//!
//! Só no diretório de dados do app, na máquina do analista: o conteúdo dos arquivos
//! sujos no início (para o diff e o desfazer) e a última conferência. O diário de
//! eventos recebe só o identificador da análise ([`Mudanca::Alteracao`]). Nada é
//! escrito nas pastas do projeto, exceto quando o analista pede para desfazer.

pub mod git;

#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, UNIX_EPOCH};

use cba_core::dados::DiretorioDados;
use cba_core::eventos::{DiarioEventos, Evento, Mudanca, Notificador};
use cba_core::tempo;
use cba_kb::Id;
use cba_repos::ForgeConfig;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::sessoes::{RegistroSessao, Sessoes, id_valido};

/// Versão do formato do retrato.
pub const VERSAO_DO_FORMATO: u32 = 1;
/// Pasta dos retratos no diretório de dados.
pub const PASTA: &str = "retratos";
const ARQUIVO_RETRATO: &str = "retrato.json";
const ARQUIVO_CONFERENCIA: &str = "conferencia.json";
const CONTEUDO: &str = "conteudo";

/// Maior arquivo sujo cujo conteúdo é guardado no retrato (acima, só o hash: o
/// aviso lista o arquivo, sem diff nem desfazer).
pub const LIMITE_POR_ARQUIVO: u64 = 8 * 1024 * 1024;
/// Total guardado por sessão.
pub const LIMITE_POR_SESSAO: u64 = 256 * 1024 * 1024;
/// Arquivos lidos numa pasta fora do git.
pub const LIMITE_SEM_GIT: usize = 200_000;
/// Tamanho máximo do diff de um arquivo mostrado no aviso.
pub const LIMITE_DO_DIFF: usize = 256 * 1024;
/// Quanto o `PreToolUse` de uma ferramenta que escreve espera o retrato.
pub const ESPERA_DO_RETRATO: Duration = Duration::from_secs(5);

/// Árvore vazia do git, para comparar com um repositório que não tinha commits.
const ARVORE_VAZIA: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

/// Erro do aviso de alteração. As mensagens citam só caminhos relativos do projeto.
#[derive(Debug, thiserror::Error)]
pub enum Erro {
    #[error("a sessão não tem retrato do início")]
    SemRetrato,
    #[error("a sessão não está registrada")]
    SemSessao,
    #[error("o projeto da sessão não está cadastrado")]
    SemProjeto,
    #[error("a sessão não é desta análise")]
    OutraAnalise,
    #[error(transparent)]
    Git(#[from] git::ErroGit),
    #[error("falha de acesso a arquivo: {0:?}")]
    Io(io::ErrorKind),
    #[error("{0} não está entre as alterações da sessão; confira de novo")]
    NaoAlterado(String),
    #[error("{0} mudou desde que o aviso foi mostrado; confira de novo")]
    MudouDeNovo(String),
    #[error(
        "o conteúdo anterior de {0} não foi guardado (arquivo grande ou pasta fora do git); desfaça pelo git"
    )]
    SemConteudoAnterior(String),
}

impl From<io::Error> for Erro {
    fn from(e: io::Error) -> Self {
        Self::Io(e.kind())
    }
}

/// Conteúdo de um arquivo no início da sessão.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Base {
    /// Se o arquivo existia.
    pub existia: bool,
    /// SHA-256 do conteúdo (se existia).
    #[serde(default)]
    pub hash: Option<String>,
    /// Se o conteúdo está em `conteudo/<hash>` (para o diff e o desfazer).
    #[serde(default)]
    pub guardado: bool,
}

/// Retrato de um repositório git.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetratoGit {
    /// Raiz da cópia de trabalho (numa worktree, a da worktree).
    pub raiz: PathBuf,
    /// Commit no início (`None` num repositório sem commits).
    #[serde(default)]
    pub head: Option<String>,
    /// Pastas do projeto dentro do repositório, relativas à raiz (vazio: todo ele).
    #[serde(default)]
    pub pastas: Vec<String>,
    /// Arquivos que já diferiam do `head` no início. Os demais têm como base o
    /// conteúdo do `head`.
    #[serde(default)]
    pub base: BTreeMap<String, Base>,
}

/// Tamanho e modificação de um arquivo fora do git.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Assinatura {
    pub tamanho: u64,
    /// Nanossegundos desde 1970 (0 se o sistema de arquivos não informa).
    pub modificado_ns: u64,
}

/// Retrato de uma pasta fora do git.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetratoSemGit {
    pub pasta: PathBuf,
    pub arquivos: BTreeMap<String, Assinatura>,
    /// A pasta passou de [`LIMITE_SEM_GIT`] arquivos: o resto não foi visto.
    #[serde(default)]
    pub incompleto: bool,
}

/// Estado das pastas do projeto no início de uma sessão.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Retrato {
    pub formato: u32,
    pub sessao: String,
    pub projeto: String,
    pub momento_ms: u64,
    #[serde(default)]
    pub repositorios: Vec<RetratoGit>,
    #[serde(default)]
    pub sem_git: Vec<RetratoSemGit>,
    /// Limitações do retrato, em frases curtas (git ausente, pasta inacessível).
    #[serde(default)]
    pub avisos: Vec<String>,
}

/// O que aconteceu com um arquivo durante a sessão.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TipoAlteracao {
    Criado,
    Modificado,
    Removido,
}

/// Arquivo alterado durante a sessão.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Alterado {
    /// Raiz do repositório (ou a pasta, fora do git).
    pub pasta: PathBuf,
    /// Caminho relativo a `pasta`, com `/`.
    pub caminho: String,
    pub tipo: TipoAlteracao,
    /// SHA-256 do conteúdo atual (`None` se removido ou fora do git). O desfazer
    /// confere que o arquivo ainda está como o aviso mostrou.
    pub hash_atual: Option<String>,
    /// Se há conteúdo de base para o diff e o desfazer.
    pub desfazivel: bool,
}

/// Resultado de [`conferir`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Conferencia {
    pub sessao: String,
    pub alterados: Vec<Alterado>,
    pub avisos: Vec<String>,
}

/// Arquivo alterado, com o diff, como a tela mostra.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Detalhe {
    pub sessao: String,
    #[serde(flatten)]
    pub alterado: Alterado,
    /// Diff unificado (antes → agora), cortado em [`LIMITE_DO_DIFF`].
    pub diff: Option<String>,
    /// Conteúdo binário: sem diff de linhas.
    pub binario: bool,
}

/// Aviso de alteração de uma análise: os arquivos alterados pelas sessões ligadas
/// a ela.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvisoAlteracao {
    pub arquivos: Vec<Detalhe>,
    pub avisos: Vec<String>,
}

/// Arquivo que o analista pediu para desfazer ou manter.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pedido {
    pub sessao: String,
    pub pasta: PathBuf,
    pub caminho: String,
    /// O `hash_atual` que o aviso mostrou.
    pub hash_atual: Option<String>,
}

/// Retratos das sessões numa pasta.
#[derive(Debug, Clone)]
pub struct Retratos {
    pasta: PathBuf,
}

impl Retratos {
    pub fn em(pasta: impl Into<PathBuf>) -> Self {
        Self {
            pasta: pasta.into(),
        }
    }

    /// Retratos no diretório de dados do app (`<dados>/retratos`).
    pub fn do_diretorio(dados: &DiretorioDados) -> Self {
        Self::em(dados.raiz().join(PASTA))
    }

    fn da_sessao(&self, id: &str) -> Option<PathBuf> {
        id_valido(id).then(|| self.pasta.join(id))
    }

    /// Marca que o retrato da sessão foi pedido. Devolve `false` se já estava
    /// marcado (por este ou por outro processo).
    pub fn reservar(&self, id: &str) -> io::Result<bool> {
        let Some(pasta) = self.da_sessao(id) else {
            return Ok(false);
        };
        fs::create_dir_all(&self.pasta)?;
        match fs::create_dir(&pasta) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// Se o retrato da sessão já foi pedido.
    pub fn reservado(&self, id: &str) -> bool {
        self.da_sessao(id).is_some_and(|p| p.is_dir())
    }

    /// Se o retrato da sessão está pronto.
    pub fn pronto(&self, id: &str) -> bool {
        self.da_sessao(id)
            .is_some_and(|p| p.join(ARQUIVO_RETRATO).is_file())
    }

    /// Espera o retrato pedido ficar pronto, até `limite`. Devolve se está pronto
    /// (ou se nem foi pedido, caso em que não há o que esperar).
    pub fn aguardar(&self, id: &str, limite: Duration) -> bool {
        if !self.reservado(id) {
            return true;
        }
        let inicio = Instant::now();
        loop {
            if self.pronto(id) {
                return true;
            }
            if inicio.elapsed() >= limite {
                return false;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    pub fn obter(&self, id: &str) -> Result<Option<Retrato>, Erro> {
        let Some(pasta) = self.da_sessao(id) else {
            return Ok(None);
        };
        match fs::read(pasta.join(ARQUIVO_RETRATO)) {
            Ok(b) => Ok(serde_json::from_slice(&b).ok()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn gravar_json<T: Serialize>(&self, id: &str, nome: &str, valor: &T) -> Result<(), Erro> {
        let pasta = self.da_sessao(id).ok_or(Erro::SemSessao)?;
        let mut json = serde_json::to_vec_pretty(valor).map_err(io::Error::other)?;
        json.push(b'\n');
        cba_kb::arquivo::gravar(&pasta.join(nome), &json)?;
        Ok(())
    }

    fn gravar(&self, retrato: &Retrato) -> Result<(), Erro> {
        self.gravar_json(&retrato.sessao, ARQUIVO_RETRATO, retrato)
    }

    /// Última conferência gravada (para só notificar quando muda).
    pub fn ultima_conferencia(&self, id: &str) -> Option<Conferencia> {
        let pasta = self.da_sessao(id)?;
        let bytes = fs::read(pasta.join(ARQUIVO_CONFERENCIA)).ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    fn guardar(&self, id: &str, bytes: &[u8]) -> Result<String, Erro> {
        let hash = sha256(bytes);
        let pasta = self.da_sessao(id).ok_or(Erro::SemSessao)?.join(CONTEUDO);
        let destino = pasta.join(&hash);
        if !destino.is_file() {
            cba_kb::arquivo::gravar(&destino, bytes)?;
        }
        Ok(hash)
    }

    fn guardado(&self, id: &str, hash: &str) -> Result<Vec<u8>, Erro> {
        let pasta = self.da_sessao(id).ok_or(Erro::SemSessao)?;
        let bytes = fs::read(pasta.join(CONTEUDO).join(hash))?;
        // Um conteúdo corrompido não serve de base para desfazer nada.
        if sha256(&bytes) != hash {
            return Err(Erro::Io(io::ErrorKind::InvalidData));
        }
        Ok(bytes)
    }
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn sha256_do_arquivo(caminho: &Path) -> io::Result<String> {
    let mut arquivo = File::open(caminho)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = arquivo.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(format!("{:x}", h.finalize()))
}

/// Caminho no disco de um caminho relativo do git (`a/b.sql`), recusando `..`.
fn no_disco(raiz: &Path, relativo: &str) -> Option<PathBuf> {
    let mut p = raiz.to_path_buf();
    for parte in relativo.split('/') {
        if parte.is_empty() || parte == "." || parte == ".." {
            return None;
        }
        p.push(parte);
    }
    Some(p)
}

fn relativo_git(raiz: &Path, pasta: &Path) -> Option<String> {
    let rel = pasta.strip_prefix(raiz).ok()?;
    let partes: Vec<String> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    Some(partes.join("/"))
}

/// Arquivo regular (sem seguir links) e seu tamanho.
fn tamanho_de_arquivo(caminho: &Path) -> Option<u64> {
    fs::symlink_metadata(caminho)
        .ok()
        .filter(|m| m.is_file())
        .map(|m| m.len())
}

/// Tira o retrato das `pastas` do projeto no início da sessão `sessao` e o grava.
/// Pastas inacessíveis ou falhas do git viram avisos no retrato, que é gravado
/// assim mesmo (para a sessão não esperar por ele de novo).
pub fn retratar(
    retratos: &Retratos,
    sessao: &str,
    projeto: &str,
    pastas: &[PathBuf],
) -> Result<Retrato, Erro> {
    if retratos.da_sessao(sessao).is_none() {
        return Err(Erro::SemSessao);
    }
    retratos.reservar(sessao)?;
    let mut retrato = Retrato {
        formato: VERSAO_DO_FORMATO,
        sessao: sessao.to_owned(),
        projeto: projeto.to_owned(),
        momento_ms: tempo::agora_ms(),
        repositorios: Vec::new(),
        sem_git: Vec::new(),
        avisos: Vec::new(),
    };

    // Agrupa as pastas por repositório.
    let mut grupos: BTreeMap<PathBuf, Vec<String>> = BTreeMap::new();
    let mut fora_do_git = Vec::new();
    for pasta in pastas {
        let Ok(pasta) = dunce::canonicalize(pasta) else {
            retrato
                .avisos
                .push("uma pasta do projeto está inacessível".into());
            continue;
        };
        match cba_index::detectar_git(&pasta).and_then(|g| {
            let rel = relativo_git(&g.raiz, &pasta)?;
            Some((g.raiz, rel))
        }) {
            Some((raiz, rel)) => grupos.entry(raiz).or_default().push(rel),
            None => fora_do_git.push(pasta),
        }
    }

    let mut guardado_total = 0u64;
    for (raiz, mut rels) in grupos {
        // A raiz do repositório entre as pastas: o repositório todo.
        if rels.iter().any(String::is_empty) {
            rels.clear();
        }
        rels.sort();
        rels.dedup();
        match retratar_git(retratos, sessao, &raiz, rels, &mut guardado_total) {
            Ok(r) => retrato.repositorios.push(r),
            Err(Erro::Git(git::ErroGit::NaoEncontrado)) => {
                retrato.avisos.push(git::ErroGit::NaoEncontrado.to_string());
                fora_do_git.push(raiz);
            }
            Err(e) => retrato.avisos.push(e.to_string()),
        }
    }
    for pasta in fora_do_git {
        let (arquivos, incompleto) = assinaturas(&pasta);
        retrato.sem_git.push(RetratoSemGit {
            pasta,
            arquivos,
            incompleto,
        });
    }
    if !retrato.sem_git.is_empty() {
        retrato.avisos.push(
            "pasta fora do git: o aviso lista os arquivos alterados, sem diff nem desfazer".into(),
        );
    }
    retratos.gravar(&retrato)?;
    Ok(retrato)
}

fn retratar_git(
    retratos: &Retratos,
    sessao: &str,
    raiz: &Path,
    pastas: Vec<String>,
    guardado_total: &mut u64,
) -> Result<RetratoGit, Erro> {
    let head = git::head(raiz)?;
    let sujos = git::sujos(raiz, &pastas)?;
    let mut base = BTreeMap::new();
    for caminho in sujos {
        let Some(arquivo) = no_disco(raiz, &caminho) else {
            continue;
        };
        let entrada = match tamanho_de_arquivo(&arquivo) {
            None => Base {
                existia: false,
                hash: None,
                guardado: false,
            },
            Some(tamanho)
                if tamanho <= LIMITE_POR_ARQUIVO
                    && *guardado_total + tamanho <= LIMITE_POR_SESSAO =>
            {
                let bytes = fs::read(&arquivo)?;
                *guardado_total += bytes.len() as u64;
                Base {
                    existia: true,
                    hash: Some(retratos.guardar(sessao, &bytes)?),
                    guardado: true,
                }
            }
            Some(_) => Base {
                existia: true,
                hash: Some(sha256_do_arquivo(&arquivo)?),
                guardado: false,
            },
        };
        base.insert(caminho, entrada);
    }
    Ok(RetratoGit {
        raiz: raiz.to_path_buf(),
        head,
        pastas,
        base,
    })
}

/// Tamanho e modificação dos arquivos de uma pasta fora do git (sem seguir links e
/// sem entrar em `.git`).
fn assinaturas(pasta: &Path) -> (BTreeMap<String, Assinatura>, bool) {
    let mut arquivos = BTreeMap::new();
    let mut pilha = vec![(pasta.to_path_buf(), String::new())];
    while let Some((dir, prefixo)) = pilha.pop() {
        let Ok(entradas) = fs::read_dir(&dir) else {
            continue;
        };
        for entrada in entradas.flatten() {
            let nome = entrada.file_name().to_string_lossy().into_owned();
            let rel = if prefixo.is_empty() {
                nome.clone()
            } else {
                format!("{prefixo}/{nome}")
            };
            let Ok(meta) = entrada.metadata() else {
                continue;
            };
            if meta.is_dir() {
                if nome != ".git" {
                    pilha.push((entrada.path(), rel));
                }
            } else if meta.is_file() {
                if arquivos.len() >= LIMITE_SEM_GIT {
                    return (arquivos, true);
                }
                let modificado_ns = meta
                    .modified()
                    .ok()
                    .and_then(|m| m.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_nanos() as u64)
                    .unwrap_or(0);
                arquivos.insert(
                    rel,
                    Assinatura {
                        tamanho: meta.len(),
                        modificado_ns,
                    },
                );
            }
        }
    }
    (arquivos, false)
}

/// Conteúdo de base de um arquivo.
enum ConteudoBase {
    Ausente,
    Conteudo(Vec<u8>),
    /// Só o hash (arquivo grande no início).
    SoHash(String),
}

impl ConteudoBase {
    fn hash(&self) -> Option<String> {
        match self {
            Self::Ausente => None,
            Self::Conteudo(b) => Some(sha256(b)),
            Self::SoHash(h) => Some(h.clone()),
        }
    }
}

fn conteudo_base(
    retratos: &Retratos,
    sessao: &str,
    repo: &RetratoGit,
    caminho: &str,
) -> Result<ConteudoBase, Erro> {
    if let Some(b) = repo.base.get(caminho) {
        return Ok(match (&b.hash, b.existia, b.guardado) {
            (_, false, _) | (None, _, _) => ConteudoBase::Ausente,
            (Some(h), true, true) => ConteudoBase::Conteudo(retratos.guardado(sessao, h)?),
            (Some(h), true, false) => ConteudoBase::SoHash(h.clone()),
        });
    }
    match &repo.head {
        None => Ok(ConteudoBase::Ausente),
        Some(head) => Ok(match git::conteudo(&repo.raiz, head, caminho)? {
            Some(b) => ConteudoBase::Conteudo(b),
            None => ConteudoBase::Ausente,
        }),
    }
}

/// Arquivo comparado, com os conteúdos quando pedidos.
struct Comparado {
    alterado: Alterado,
    antes: Option<ConteudoBase>,
    depois: Option<Vec<u8>>,
}

fn comparar(
    retratos: &Retratos,
    retrato: &Retrato,
    com_conteudo: bool,
) -> Result<(Vec<Comparado>, Vec<String>), Erro> {
    let mut lista = Vec::new();
    let mut avisos = retrato.avisos.clone();
    for repo in &retrato.repositorios {
        let mut candidatos: BTreeSet<String> = git::sujos(&repo.raiz, &repo.pastas)?;
        candidatos.extend(repo.base.keys().cloned());
        let agora = git::head(&repo.raiz)?;
        if agora != repo.head
            && let Some(agora) = &agora
        {
            let de = repo.head.as_deref().unwrap_or(ARVORE_VAZIA);
            candidatos.extend(git::diferentes_entre(&repo.raiz, de, agora, &repo.pastas)?);
        }
        for caminho in candidatos {
            let Some(arquivo) = no_disco(&repo.raiz, &caminho) else {
                continue;
            };
            let antes = conteudo_base(retratos, &retrato.sessao, repo, &caminho)?;
            let depois = match tamanho_de_arquivo(&arquivo) {
                None => None,
                Some(_) => Some(fs::read(&arquivo)?),
            };
            let hash_antes = antes.hash();
            let hash_depois = depois.as_deref().map(sha256);
            if hash_antes == hash_depois {
                continue;
            }
            let tipo = match (&hash_antes, &hash_depois) {
                (None, _) => TipoAlteracao::Criado,
                (_, None) => TipoAlteracao::Removido,
                _ => TipoAlteracao::Modificado,
            };
            let desfazivel = !matches!(antes, ConteudoBase::SoHash(_));
            lista.push(Comparado {
                alterado: Alterado {
                    pasta: repo.raiz.clone(),
                    caminho,
                    tipo,
                    hash_atual: hash_depois,
                    desfazivel,
                },
                antes: com_conteudo.then_some(antes),
                depois: if com_conteudo { depois } else { None },
            });
        }
    }
    for sg in &retrato.sem_git {
        let (agora, incompleto) = assinaturas(&sg.pasta);
        if incompleto || sg.incompleto {
            avisos.push(format!(
                "pasta com mais de {LIMITE_SEM_GIT} arquivos fora do git: a conferência é parcial"
            ));
        }
        let caminhos: BTreeSet<&String> = sg.arquivos.keys().chain(agora.keys()).collect();
        for caminho in caminhos {
            let tipo = match (sg.arquivos.get(caminho), agora.get(caminho)) {
                (None, Some(_)) => TipoAlteracao::Criado,
                (Some(_), None) => TipoAlteracao::Removido,
                (Some(a), Some(b)) if a != b => TipoAlteracao::Modificado,
                _ => continue,
            };
            lista.push(Comparado {
                alterado: Alterado {
                    pasta: sg.pasta.clone(),
                    caminho: caminho.clone(),
                    tipo,
                    hash_atual: None,
                    desfazivel: false,
                },
                antes: None,
                depois: None,
            });
        }
    }
    Ok((lista, avisos))
}

/// Confere as pastas do projeto contra o retrato do início da sessão.
pub fn conferir(retratos: &Retratos, sessao: &str) -> Result<Conferencia, Erro> {
    let retrato = retratos.obter(sessao)?.ok_or(Erro::SemRetrato)?;
    let (lista, avisos) = comparar(retratos, &retrato, false)?;
    Ok(Conferencia {
        sessao: sessao.to_owned(),
        alterados: lista.into_iter().map(|c| c.alterado).collect(),
        avisos,
    })
}

/// Como [`conferir`], com o diff de cada arquivo.
pub fn detalhar(retratos: &Retratos, sessao: &str) -> Result<(Vec<Detalhe>, Vec<String>), Erro> {
    let retrato = retratos.obter(sessao)?.ok_or(Erro::SemRetrato)?;
    let (lista, avisos) = comparar(retratos, &retrato, true)?;
    let detalhes = lista
        .into_iter()
        .map(|c| {
            let antes = match &c.antes {
                Some(ConteudoBase::Conteudo(b)) => Some(Some(b.as_slice())),
                Some(ConteudoBase::Ausente) => Some(None),
                _ => None,
            };
            let (diff, binario) = match antes {
                Some(antes) if c.alterado.desfazivel => {
                    diff(&c.alterado.caminho, antes, c.depois.as_deref())
                }
                _ => (None, false),
            };
            Detalhe {
                sessao: sessao.to_owned(),
                alterado: c.alterado,
                diff,
                binario,
            }
        })
        .collect();
    Ok((detalhes, avisos))
}

fn texto(bytes: &[u8]) -> Option<&str> {
    if bytes.contains(&0) {
        return None;
    }
    std::str::from_utf8(bytes).ok()
}

/// Diff unificado de `antes` para `depois` (`None` = arquivo ausente). Devolve
/// `(None, true)` para conteúdo binário.
pub fn diff(caminho: &str, antes: Option<&[u8]>, depois: Option<&[u8]>) -> (Option<String>, bool) {
    let (Some(a), Some(b)) = (
        antes.map_or(Some(""), texto),
        depois.map_or(Some(""), texto),
    ) else {
        return (None, true);
    };
    let de = if antes.is_some() {
        format!("a/{caminho}")
    } else {
        "/dev/null".to_owned()
    };
    let para = if depois.is_some() {
        format!("b/{caminho}")
    } else {
        "/dev/null".to_owned()
    };
    let mut saida = similar::TextDiff::from_lines(a, b)
        .unified_diff()
        .context_radius(3)
        .header(&de, &para)
        .to_string();
    if saida.len() > LIMITE_DO_DIFF {
        let mut corte = LIMITE_DO_DIFF;
        while !saida.is_char_boundary(corte) {
            corte -= 1;
        }
        saida.truncate(corte);
        saida.push_str("\n… (diff cortado)\n");
    }
    (Some(saida), false)
}

/// Confere os pedidos contra a situação atual: cada um precisa estar entre as
/// alterações da sessão, com o mesmo conteúdo que o aviso mostrou.
fn validar<'a>(
    retratos: &Retratos,
    retrato: &'a Retrato,
    pedidos: &[&Pedido],
) -> Result<Vec<(&'a RetratoGit, Alterado)>, Erro> {
    let (lista, _) = comparar(retratos, retrato, false)?;
    let mut validos = Vec::new();
    for p in pedidos {
        let alterado = lista
            .iter()
            .map(|c| &c.alterado)
            .find(|a| a.pasta == p.pasta && a.caminho == p.caminho)
            .ok_or_else(|| Erro::NaoAlterado(p.caminho.clone()))?;
        if alterado.hash_atual != p.hash_atual {
            return Err(Erro::MudouDeNovo(p.caminho.clone()));
        }
        if !alterado.desfazivel {
            return Err(Erro::SemConteudoAnterior(p.caminho.clone()));
        }
        let repo = retrato
            .repositorios
            .iter()
            .find(|r| r.raiz == p.pasta)
            .ok_or_else(|| Erro::SemConteudoAnterior(p.caminho.clone()))?;
        validos.push((repo, alterado.clone()));
    }
    Ok(validos)
}

/// Volta os arquivos pedidos ao conteúdo do início da sessão (apaga os criados e
/// recria os removidos). Confere todos antes de mexer em qualquer um. Devolve os
/// caminhos desfeitos.
pub fn desfazer(
    retratos: &Retratos,
    sessao: &str,
    pedidos: &[Pedido],
) -> Result<Vec<String>, Erro> {
    let retrato = retratos.obter(sessao)?.ok_or(Erro::SemRetrato)?;
    let pedidos: Vec<&Pedido> = pedidos.iter().filter(|p| p.sessao == sessao).collect();
    let validos = validar(retratos, &retrato, &pedidos)?;
    let mut bases = Vec::new();
    for (repo, alterado) in &validos {
        let base = conteudo_base(retratos, sessao, repo, &alterado.caminho)?;
        if let ConteudoBase::SoHash(_) = base {
            return Err(Erro::SemConteudoAnterior(alterado.caminho.clone()));
        }
        bases.push(base);
    }
    let mut feitos = Vec::new();
    for ((repo, alterado), base) in validos.into_iter().zip(bases) {
        let arquivo = no_disco(&repo.raiz, &alterado.caminho)
            .ok_or_else(|| Erro::NaoAlterado(alterado.caminho.clone()))?;
        match base {
            ConteudoBase::Ausente => match fs::remove_file(&arquivo) {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            },
            ConteudoBase::Conteudo(bytes) => {
                if let Some(pai) = arquivo.parent() {
                    fs::create_dir_all(pai)?;
                }
                // Escrita direta (e não arquivo temporário e troca): mantém as
                // permissões do arquivo do analista.
                fs::write(&arquivo, bytes)?;
            }
            ConteudoBase::SoHash(_) => unreachable!("conferido acima"),
        }
        feitos.push(alterado.caminho);
    }
    tracing::info!(arquivos = feitos.len(), "alterações desfeitas");
    Ok(feitos)
}

/// O analista decidiu manter as alterações pedidas: o conteúdo atual passa a ser a
/// base da sessão, e elas saem do aviso. Devolve os caminhos mantidos.
pub fn manter(retratos: &Retratos, sessao: &str, pedidos: &[Pedido]) -> Result<Vec<String>, Erro> {
    let mut retrato = retratos.obter(sessao)?.ok_or(Erro::SemRetrato)?;
    let pedidos: Vec<&Pedido> = pedidos.iter().filter(|p| p.sessao == sessao).collect();
    let validos: Vec<(PathBuf, String)> = validar(retratos, &retrato, &pedidos)?
        .into_iter()
        .map(|(repo, a)| (repo.raiz.clone(), a.caminho))
        .collect();
    let mut feitos = Vec::new();
    for (raiz, caminho) in validos {
        let arquivo =
            no_disco(&raiz, &caminho).ok_or_else(|| Erro::NaoAlterado(caminho.clone()))?;
        let base = match tamanho_de_arquivo(&arquivo) {
            None => Base {
                existia: false,
                hash: None,
                guardado: false,
            },
            Some(t) if t <= LIMITE_POR_ARQUIVO => Base {
                existia: true,
                hash: Some(retratos.guardar(sessao, &fs::read(&arquivo)?)?),
                guardado: true,
            },
            Some(_) => Base {
                existia: true,
                hash: Some(sha256_do_arquivo(&arquivo)?),
                guardado: false,
            },
        };
        if let Some(repo) = retrato.repositorios.iter_mut().find(|r| r.raiz == raiz) {
            repo.base.insert(caminho.clone(), base);
        }
        feitos.push(caminho);
    }
    retratos.gravar(&retrato)?;
    tracing::info!(arquivos = feitos.len(), "alterações mantidas pelo analista");
    Ok(feitos)
}

// ---------------------------------------------------------------------------
// Ligação com as sessões, as análises e o diário de eventos

/// Pastas a retratar numa sessão: as do projeto cadastrado e, se a sessão foi
/// aberta numa worktree fora delas, a worktree. As pastas de código-fonte (num
/// repositório git cujo remoto é o repositório de código cadastrado no projeto)
/// ficam de fora: o aviso cobre só as demais pastas do projeto (#130).
pub fn pastas_da_sessao(
    dados: &DiretorioDados,
    registro: &RegistroSessao,
) -> Result<Vec<PathBuf>, Erro> {
    let projeto = cba_index::Cadastro::em(dados.projetos())
        .obter(&registro.projeto)
        .map_err(|_| Erro::SemProjeto)?
        .ok_or(Erro::SemProjeto)?;
    let mut pastas: Vec<PathBuf> = projeto.pastas.into_iter().map(|p| p.caminho).collect();
    if let Some(dir) = registro
        .diretorio
        .as_ref()
        .and_then(|d| dunce::canonicalize(d).ok())
        && !pastas.iter().any(|p| dir.starts_with(p))
        && let Some(g) = cba_index::detectar_git(&dir)
    {
        pastas.push(g.raiz);
    }
    Ok(sem_codigo_fonte(
        pastas,
        projeto.repositorios.forge.as_ref(),
    ))
}

/// Tira de `pastas` as que estão num repositório git com algum remoto apontando
/// para `forja` (o repositório de código cadastrado). Sem forja, devolve todas.
/// Uma pasta cujo remoto não se consegue ler (git ausente) continua na lista.
pub fn sem_codigo_fonte(pastas: Vec<PathBuf>, forja: Option<&ForgeConfig>) -> Vec<PathBuf> {
    let Some(forja) = forja else {
        return pastas;
    };
    let mut vistos: BTreeMap<PathBuf, bool> = BTreeMap::new();
    pastas
        .into_iter()
        .filter(|pasta| {
            let raiz = dunce::canonicalize(pasta)
                .ok()
                .and_then(|p| cba_index::detectar_git(&p))
                .map(|g| g.raiz);
            let Some(raiz) = raiz else {
                return true;
            };
            let codigo = *vistos.entry(raiz.clone()).or_insert_with(|| {
                git::remotos(&raiz)
                    .unwrap_or_default()
                    .iter()
                    .any(|url| aponta_para(url, forja))
            });
            !codigo
        })
        .collect()
}

/// Host (sem usuário nem porta) e caminho (sem `.git` nem barras nas pontas) de
/// uma URL de remoto do git: `https://host/a/b.git`, `ssh://git@host:22/a/b`,
/// `git@host:a/b.git`. Em minúsculas, para comparar.
fn host_e_caminho(url: &str) -> Option<(String, String)> {
    let url = url.trim();
    let (autoridade, caminho) = if let Some((_, resto)) = url.split_once("://") {
        resto.split_once('/')?
    } else {
        // Forma scp do ssh: [usuário@]host:caminho.
        let (antes, depois) = url.split_once(':')?;
        if antes.contains('/') {
            return None;
        }
        (antes, depois)
    };
    let host = autoridade.rsplit_once('@').map_or(autoridade, |(_, h)| h);
    let host = host.split_once(':').map_or(host, |(h, _)| h);
    let caminho = caminho.trim_matches('/');
    let caminho = caminho.strip_suffix(".git").unwrap_or(caminho);
    if host.is_empty() || caminho.is_empty() {
        return None;
    }
    Some((host.to_ascii_lowercase(), caminho.to_ascii_lowercase()))
}

/// Se a URL do remoto aponta para o repositório de código cadastrado.
pub fn aponta_para(url: &str, forja: &ForgeConfig) -> bool {
    let Some((host, caminho)) = host_e_caminho(url) else {
        return false;
    };
    let mesmo = |h: &str, c: &str| {
        let h = h.trim().to_ascii_lowercase();
        let c = c.trim().trim_matches('/').to_ascii_lowercase();
        !c.is_empty() && host == h && caminho == c
    };
    match forja {
        ForgeConfig::Github(g) => mesmo("github.com", &g.repo) || mesmo("ssh.github.com", &g.repo),
        ForgeConfig::Gitlab(g) => mesmo(&g.host, &g.path),
    }
}

/// Tarefa feita fora do caminho do hook.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tarefa {
    /// Retrato do início da sessão.
    Retratar,
    /// Conferência do fim de uma resposta ou da sessão.
    Conferir,
}

impl Tarefa {
    pub fn nome(self) -> &'static str {
        match self {
            Self::Retratar => "retratar",
            Self::Conferir => "conferir",
        }
    }

    pub fn do_nome(nome: &str) -> Option<Self> {
        match nome {
            "retratar" => Some(Self::Retratar),
            "conferir" => Some(Self::Conferir),
            _ => None,
        }
    }
}

/// Inicia `<exe> fundo <tarefa> <sessao>` como processo separado, sem esperar: o
/// hook responde na hora. O processo não herda a entrada nem a saída do hook (o
/// Claude Code não espera por ele) e, no Unix, fica num grupo de processos próprio
/// (um Ctrl+C no terminal da sessão não o interrompe no meio).
pub fn disparar(exe: &Path, tarefa: Tarefa, sessao: &str) -> io::Result<()> {
    use std::process::{Command, Stdio};
    if !id_valido(sessao) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "sessão inválida",
        ));
    }
    let novo = || {
        let mut c = Command::new(exe);
        c.args(["fundo", tarefa.nome(), sessao])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        c
    };
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        novo().process_group(0).spawn().map(drop)
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let base = CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW;
        // Fora do job do cliente, se ele permitir: o processo sobrevive ao fim do
        // hook. Se não permitir, roda dentro dele.
        novo()
            .creation_flags(base | CREATE_BREAKAWAY_FROM_JOB)
            .spawn()
            .or_else(|_| novo().creation_flags(base).spawn())
            .map(drop)
    }
    #[cfg(not(any(unix, windows)))]
    {
        novo().spawn().map(drop)
    }
}

/// Executa uma tarefa (modo `cba fundo`).
pub fn executar(dados: &DiretorioDados, tarefa: Tarefa, sessao: &str) -> Result<(), Erro> {
    let registro = Sessoes::do_diretorio(dados)
        .obter(sessao)?
        .ok_or(Erro::SemSessao)?;
    let retratos = Retratos::do_diretorio(dados);
    match tarefa {
        Tarefa::Retratar => {
            if retratos.pronto(sessao) {
                return Ok(());
            }
            let pastas = match pastas_da_sessao(dados, &registro) {
                Ok(p) => p,
                Err(e) => {
                    // Grava o retrato vazio, com o motivo: a sessão não espera de novo.
                    let mut vazio = retratar(&retratos, sessao, &registro.projeto, &[])?;
                    vazio.avisos.push(e.to_string());
                    retratos.gravar(&vazio)?;
                    return Err(e);
                }
            };
            retratar(&retratos, sessao, &registro.projeto, &pastas).map(drop)
        }
        Tarefa::Conferir => conferir_e_avisar(dados, &registro).map(drop),
    }
}

/// Confere a sessão e, se o resultado mudou desde a última conferência, grava-o e
/// avisa a análise ligada pelo diário de eventos.
pub fn conferir_e_avisar(
    dados: &DiretorioDados,
    registro: &RegistroSessao,
) -> Result<Conferencia, Erro> {
    let retratos = Retratos::do_diretorio(dados);
    let conferencia = conferir(&retratos, &registro.id)?;
    let anterior = retratos.ultima_conferencia(&registro.id);
    if anterior.as_ref() != Some(&conferencia) {
        retratos.gravar_json(&registro.id, ARQUIVO_CONFERENCIA, &conferencia)?;
        // Sem conferência anterior, a lista de antes é vazia.
        let mudou_lista =
            anterior.map(|a| a.alterados).unwrap_or_default() != conferencia.alterados;
        if mudou_lista && let Some(analise) = &registro.analise {
            avisar(dados, &registro.projeto, analise);
        }
    }
    tracing::info!(
        arquivos = conferencia.alterados.len(),
        "pasta do projeto conferida"
    );
    Ok(conferencia)
}

/// Linha [`Mudanca::Alteracao`] no diário de eventos do projeto (falhas só no log).
pub fn avisar(dados: &DiretorioDados, projeto: &str, analise: &Id) {
    let evento = Evento {
        quando_ms: tempo::agora_ms(),
        projeto: projeto.to_owned(),
        analise: analise.to_string(),
        mudanca: Mudanca::Alteracao,
        // O estado da análise não muda; a tela relê o aviso, não a análise.
        versao: 0,
        processo: std::process::id(),
    };
    if let Err(e) = DiarioEventos::em(dados.eventos()).notificar(&evento) {
        tracing::warn!(erro = ?e.kind(), "aviso de alteração não notificado");
    }
}

/// Aviso de alteração de uma análise: o que as sessões ligadas a ela alteraram (a
/// mais recente vence quando duas alteraram o mesmo arquivo).
pub fn da_analise(
    dados: &DiretorioDados,
    projeto: &str,
    analise: &Id,
) -> Result<AvisoAlteracao, Erro> {
    let retratos = Retratos::do_diretorio(dados);
    let mut aviso = AvisoAlteracao::default();
    let mut vistos = BTreeSet::new();
    for registro in Sessoes::do_diretorio(dados).da_analise(projeto, analise)? {
        if !retratos.pronto(&registro.id) {
            continue;
        }
        let (detalhes, avisos) = detalhar(&retratos, &registro.id)?;
        for a in avisos {
            if !aviso.avisos.contains(&a) {
                aviso.avisos.push(a);
            }
        }
        for d in detalhes {
            if vistos.insert((d.alterado.pasta.clone(), d.alterado.caminho.clone())) {
                aviso.arquivos.push(d);
            }
        }
    }
    Ok(aviso)
}

/// Desfaz ou mantém, a pedido do analista na tela da análise, arquivos alterados
/// por sessões dela. Depois, reconfere as sessões envolvidas e avisa as telas.
pub fn resolver_na_analise(
    dados: &DiretorioDados,
    projeto: &str,
    analise: &Id,
    pedidos: &[Pedido],
    desfazer_: bool,
) -> Result<Vec<String>, Erro> {
    let retratos = Retratos::do_diretorio(dados);
    let registros = Sessoes::do_diretorio(dados).da_analise(projeto, analise)?;
    let sessoes: BTreeSet<&str> = pedidos.iter().map(|p| p.sessao.as_str()).collect();
    let mut feitos = Vec::new();
    for sessao in sessoes {
        let registro = registros
            .iter()
            .find(|r| r.id == sessao)
            .ok_or(Erro::OutraAnalise)?;
        feitos.extend(if desfazer_ {
            desfazer(&retratos, sessao, pedidos)?
        } else {
            manter(&retratos, sessao, pedidos)?
        });
        if let Err(e) = conferir_e_avisar(dados, registro) {
            tracing::warn!(erro = %e, "reconferência depois de resolver falhou");
        }
    }
    avisar(dados, projeto, analise);
    Ok(feitos)
}

/// Nome curto do erro, para o log (sem caminhos).
pub fn tipo_do_erro(e: &Erro) -> &'static str {
    match e {
        Erro::SemRetrato => "sem_retrato",
        Erro::SemSessao => "sem_sessao",
        Erro::SemProjeto => "sem_projeto",
        Erro::OutraAnalise => "outra_analise",
        Erro::Git(git::ErroGit::NaoEncontrado) => "git_nao_encontrado",
        Erro::Git(git::ErroGit::Falhou(_)) => "git_falhou",
        Erro::Io(_) => "io",
        Erro::NaoAlterado(_) => "nao_alterado",
        Erro::MudouDeNovo(_) => "mudou_de_novo",
        Erro::SemConteudoAnterior(_) => "sem_conteudo_anterior",
    }
}
