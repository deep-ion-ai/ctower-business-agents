//! Chamadas ao `git` de linha de comando para o retrato e a conferência.
//!
//! O `git` é instalado junto com o app (`packaging/install`) e é o que o analista
//! usa para clonar o projeto. Cada chamada:
//!
//! - roda com `GIT_OPTIONAL_LOCKS=0`, para o `git status` não regravar o índice
//!   do repositório do analista (nada é escrito na pasta do projeto);
//! - usa caminhos literais (`GIT_LITERAL_PATHSPECS=1`), sem curingas;
//! - no Windows, não abre janela de console (`CREATE_NO_WINDOW`).

use std::collections::BTreeSet;
use std::path::Path;
use std::process::{Command, Stdio};

/// Falha ao executar o `git`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ErroGit {
    #[error("o git não foi encontrado; instale-o para o app conferir as alterações")]
    NaoEncontrado,
    #[error("o git falhou ({0})")]
    Falhou(String),
}

fn comando(raiz: &Path) -> Command {
    let mut c = Command::new("git");
    c.arg("-C")
        .arg(raiz)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_LITERAL_PATHSPECS", "1")
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000);
    }
    c
}

/// Executa `git -C <raiz> <args>` e devolve a saída padrão (`None` se o git saiu
/// com erro, o que algumas consultas usam como "não existe").
fn executar(raiz: &Path, args: &[&str]) -> Result<Option<Vec<u8>>, ErroGit> {
    let saida = comando(raiz).args(args).output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            ErroGit::NaoEncontrado
        } else {
            ErroGit::Falhou(format!("{:?}", e.kind()))
        }
    })?;
    Ok(saida.status.success().then_some(saida.stdout))
}

fn obrigatorio(raiz: &Path, args: &[&str]) -> Result<Vec<u8>, ErroGit> {
    executar(raiz, args)?.ok_or_else(|| ErroGit::Falhou(args.first().unwrap_or(&"").to_string()))
}

fn com_pastas<'a>(mut args: Vec<&'a str>, pastas: &'a [String]) -> Vec<&'a str> {
    if !pastas.is_empty() {
        args.push("--");
        args.extend(pastas.iter().map(String::as_str));
    }
    args
}

/// Commit atual (`None` num repositório sem commits).
pub fn head(raiz: &Path) -> Result<Option<String>, ErroGit> {
    Ok(executar(raiz, &["rev-parse", "--verify", "-q", "HEAD"])?
        .map(|s| String::from_utf8_lossy(&s).trim().to_owned())
        .filter(|s| !s.is_empty()))
}

/// Caminhos (relativos à raiz, com `/`) com alguma diferença em relação ao HEAD:
/// modificados, novos fora do `.gitignore`, removidos, no índice ou não. Restrito a
/// `pastas` (relativas à raiz; vazio = o repositório todo).
pub fn sujos(raiz: &Path, pastas: &[String]) -> Result<BTreeSet<String>, ErroGit> {
    let args = com_pastas(
        vec![
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
            "--no-renames",
            "--ignore-submodules=all",
        ],
        pastas,
    );
    let saida = obrigatorio(raiz, &args)?;
    Ok(saida
        .split(|b| *b == 0)
        .filter(|e| e.len() > 3)
        .map(|e| String::from_utf8_lossy(&e[3..]).into_owned())
        .collect())
}

/// Caminhos que diferem entre dois commits, restritos a `pastas`.
pub fn diferentes_entre(
    raiz: &Path,
    de: &str,
    para: &str,
    pastas: &[String],
) -> Result<BTreeSet<String>, ErroGit> {
    let args = com_pastas(
        vec!["diff", "--name-only", "-z", "--no-renames", de, para],
        pastas,
    );
    let saida = obrigatorio(raiz, &args)?;
    Ok(saida
        .split(|b| *b == 0)
        .filter(|e| !e.is_empty())
        .map(|e| String::from_utf8_lossy(e).into_owned())
        .collect())
}

/// Conteúdo de `caminho` no commit, como ele fica na cópia de trabalho (com os
/// filtros do repositório, como o fim de linha do Windows). `None` se o arquivo não
/// existe no commit.
pub fn conteudo(raiz: &Path, commit: &str, caminho: &str) -> Result<Option<Vec<u8>>, ErroGit> {
    let objeto = format!("{commit}:{caminho}");
    executar(raiz, &["cat-file", "--filters", &objeto])
}
