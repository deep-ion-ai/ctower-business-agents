//! Detecção de repositório git: branch e commit para registrar nas evidências.
//!
//! Lê os arquivos do `.git` diretamente (`HEAD`, refs soltas, `packed-refs`,
//! `commondir`), sem executar o `git` nem depender de libgit2: o analista pode não
//! ter o git instalado, e só precisamos do commit atual.
//!
//! Também identifica worktrees: o Claude Code usa o repositório principal como
//! chave do escopo local de uma worktree (spec, seção 6.2), e o preparo do
//! ambiente (#24) precisa desse caminho.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Estado do repositório git que contém uma pasta.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepositorioGit {
    /// Raiz da cópia de trabalho (a pasta que contém o `.git`).
    pub raiz: PathBuf,
    /// Raiz do repositório principal. Igual a `raiz`, exceto numa worktree
    /// (`git worktree add`), em que aponta para a cópia de trabalho principal.
    pub raiz_principal: PathBuf,
    /// Se a raiz é uma worktree adicional.
    pub worktree: bool,
    /// Branch atual; `None` com HEAD destacado.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// Commit atual (hash completo); `None` num repositório sem commits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
}

/// Procura o repositório git que contém `caminho` (ele mesmo ou uma pasta acima).
///
/// Devolve `None` se a pasta não está num repositório ou se o `.git` não pôde ser
/// lido.
pub fn detectar_git(caminho: &Path) -> Option<RepositorioGit> {
    detectar_com_comum(caminho).map(|(r, _)| r)
}

/// Diretório comum do git do repositório que contém `caminho` (onde ficam o `config`
/// e o `info/exclude`, compartilhados pelas worktrees); `None` fora de repositório.
pub fn diretorio_comum_git(caminho: &Path) -> Option<PathBuf> {
    detectar_com_comum(caminho).map(|(_, c)| c)
}

/// Como [`detectar_git`], com o diretório comum do git (onde fica o `config`).
fn detectar_com_comum(caminho: &Path) -> Option<(RepositorioGit, PathBuf)> {
    let inicio = dunce::canonicalize(caminho).ok()?;
    let mut atual = Some(inicio.as_path());
    while let Some(dir) = atual {
        let ponto_git = dir.join(".git");
        if ponto_git.is_dir() {
            return ler(dir, &ponto_git, &ponto_git, false).map(|r| (r, ponto_git));
        }
        if ponto_git.is_file() {
            // Worktree ou submódulo: `.git` é um arquivo `gitdir: <caminho>`.
            let conteudo = fs::read_to_string(&ponto_git).ok()?;
            let alvo = conteudo.trim().strip_prefix("gitdir:")?.trim();
            let git_dir = dunce::canonicalize(dir.join(alvo)).ok()?;
            return match fs::read_to_string(git_dir.join("commondir")) {
                Ok(c) => {
                    let comum = dunce::canonicalize(git_dir.join(c.trim())).ok()?;
                    ler(dir, &git_dir, &comum, true).map(|r| (r, comum))
                }
                // Submódulo: repositório próprio, sem diretório comum.
                Err(_) => ler(dir, &git_dir, &git_dir, false).map(|r| (r, git_dir)),
            };
        }
        atual = dir.parent();
    }
    None
}

fn ler(raiz: &Path, git_dir: &Path, comum: &Path, worktree: bool) -> Option<RepositorioGit> {
    let head = fs::read_to_string(git_dir.join("HEAD")).ok()?;
    let head = head.trim();
    let (branch, commit) = match head.strip_prefix("ref:") {
        Some(r) => {
            let r = r.trim();
            let branch = r.strip_prefix("refs/heads/").unwrap_or(r).to_string();
            (Some(branch), resolver_ref(git_dir, comum, r))
        }
        None if eh_hash(head) => (None, Some(head.to_string())),
        None => return None,
    };
    let raiz_principal = if worktree && comum.file_name().is_some_and(|n| n == ".git") {
        comum.parent().unwrap_or(raiz).to_path_buf()
    } else {
        raiz.to_path_buf()
    };
    Some(RepositorioGit {
        raiz: raiz.to_path_buf(),
        raiz_principal,
        worktree,
        branch,
        commit,
    })
}

/// URLs dos remotos (`remote.<nome>.url`) do repositório git que contém
/// `caminho`, na ordem do `config`, lidas do arquivo, sem executar o git. Vazio
/// fora de um repositório ou sem remotos.
pub fn remotos_git(caminho: &Path) -> Vec<String> {
    let Some((_, comum)) = detectar_com_comum(caminho) else {
        return Vec::new();
    };
    let Ok(config) = fs::read_to_string(comum.join("config")) else {
        return Vec::new();
    };
    let mut urls = Vec::new();
    let mut em_remoto = false;
    for linha in config.lines() {
        let l = linha.trim();
        if l.starts_with('[') {
            em_remoto = l
                .trim_start_matches('[')
                .trim_start()
                .to_ascii_lowercase()
                .starts_with("remote ");
            continue;
        }
        if !em_remoto || l.starts_with('#') || l.starts_with(';') {
            continue;
        }
        if let Some((chave, valor)) = l.split_once('=')
            && chave.trim().eq_ignore_ascii_case("url")
        {
            let url = valor.trim().trim_matches('"').trim();
            if !url.is_empty() {
                urls.push(url.to_string());
            }
        }
    }
    urls
}

/// Repositórios git em `pasta`: ela mesma, se é a raiz de um; senão, as
/// subpastas imediatas (um nível) que são raiz de um repositório, em ordem de
/// nome. Pastas ocultas ficam de fora.
pub fn repositorios_em(pasta: &Path) -> Vec<RepositorioGit> {
    let Ok(pasta) = dunce::canonicalize(pasta) else {
        return Vec::new();
    };
    let eh_raiz = |p: &Path| p.join(".git").exists();
    if eh_raiz(&pasta) {
        return detectar_git(&pasta).into_iter().collect();
    }
    let Ok(itens) = fs::read_dir(&pasta) else {
        return Vec::new();
    };
    let mut filhas: Vec<PathBuf> = itens
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
        .map(|e| e.path())
        .filter(|p| eh_raiz(p))
        .collect();
    filhas.sort();
    filhas.iter().filter_map(|p| detectar_git(p)).collect()
}

fn resolver_ref(git_dir: &Path, comum: &Path, nome: &str) -> Option<String> {
    for base in [git_dir, comum] {
        if let Ok(c) = fs::read_to_string(base.join(nome)) {
            let c = c.trim();
            if eh_hash(c) {
                return Some(c.to_string());
            }
        }
    }
    let packed = fs::read_to_string(comum.join("packed-refs")).ok()?;
    packed.lines().find_map(|l| {
        let (hash, r) = l.split_once(' ')?;
        (r.trim() == nome && eh_hash(hash)).then(|| hash.to_string())
    })
}

/// SHA-1 (40) ou SHA-256 (64) em hexadecimal.
fn eh_hash(s: &str) -> bool {
    matches!(s.len(), 40 | 64) && s.bytes().all(|b| b.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    const C1: &str = "1111111111111111111111111111111111111111";
    const C2: &str = "2222222222222222222222222222222222222222";

    fn escrever(caminho: &Path, conteudo: &str) {
        fs::create_dir_all(caminho.parent().unwrap()).unwrap();
        fs::write(caminho, conteudo).unwrap();
    }

    #[test]
    fn fora_de_repositorio() {
        let tmp = tempfile::tempdir().unwrap();
        // O tempdir pode estar dentro de um repositório só em máquinas estranhas;
        // garantimos que a resposta, se houver, não é a própria pasta.
        if let Some(r) = detectar_git(tmp.path()) {
            assert_ne!(r.raiz, dunce::canonicalize(tmp.path()).unwrap());
        }
    }

    #[test]
    fn branch_com_ref_solta_e_subpasta() {
        let tmp = tempfile::tempdir().unwrap();
        let raiz = tmp.path();
        escrever(&raiz.join(".git/HEAD"), "ref: refs/heads/main\n");
        escrever(&raiz.join(".git/refs/heads/main"), &format!("{C1}\n"));
        fs::create_dir_all(raiz.join("src/db")).unwrap();
        let r = detectar_git(&raiz.join("src/db")).unwrap();
        assert_eq!(r.raiz, dunce::canonicalize(raiz).unwrap());
        assert_eq!(r.raiz_principal, r.raiz);
        assert!(!r.worktree);
        assert_eq!(r.branch.as_deref(), Some("main"));
        assert_eq!(r.commit.as_deref(), Some(C1));
    }

    #[test]
    fn branch_em_packed_refs_e_head_destacado() {
        let tmp = tempfile::tempdir().unwrap();
        let raiz = tmp.path();
        escrever(&raiz.join(".git/HEAD"), "ref: refs/heads/feat/x\n");
        escrever(
            &raiz.join(".git/packed-refs"),
            &format!("# pack-refs with: peeled\n{C2} refs/heads/feat/x\n"),
        );
        let r = detectar_git(raiz).unwrap();
        assert_eq!(r.branch.as_deref(), Some("feat/x"));
        assert_eq!(r.commit.as_deref(), Some(C2));

        escrever(&raiz.join(".git/HEAD"), &format!("{C1}\n"));
        let r = detectar_git(raiz).unwrap();
        assert_eq!(r.branch, None);
        assert_eq!(r.commit.as_deref(), Some(C1));
    }

    #[test]
    fn repositorio_sem_commits() {
        let tmp = tempfile::tempdir().unwrap();
        escrever(&tmp.path().join(".git/HEAD"), "ref: refs/heads/main\n");
        let r = detectar_git(tmp.path()).unwrap();
        assert_eq!(r.branch.as_deref(), Some("main"));
        assert_eq!(r.commit, None);
    }

    #[test]
    fn remotos_lidos_do_config() {
        let tmp = tempfile::tempdir().unwrap();
        let raiz = tmp.path();
        escrever(&raiz.join(".git/HEAD"), "ref: refs/heads/main\n");
        escrever(
            &raiz.join(".git/config"),
            "[core]\n\tbare = false\n[remote \"origin\"]\n\turl = git@github.com:o/r.git\n\
             \tfetch = +refs/heads/*:refs/remotes/origin/*\n[branch \"main\"]\n\tremote = origin\n\
             [remote \"gl\"]\n\t# comentário\n\tURL = \"https://gitlab.example.com/g/app.git\"\n",
        );
        fs::create_dir_all(raiz.join("src")).unwrap();
        assert_eq!(
            remotos_git(&raiz.join("src")),
            [
                "git@github.com:o/r.git",
                "https://gitlab.example.com/g/app.git"
            ]
        );
        let sem = tempfile::tempdir().unwrap();
        escrever(&sem.path().join(".git/HEAD"), "ref: refs/heads/main\n");
        assert!(remotos_git(sem.path()).is_empty());
    }

    #[test]
    fn repositorios_na_pasta_um_nivel() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        for n in ["b-web", "a-legado", ".oculto"] {
            escrever(&base.join(n).join(".git/HEAD"), "ref: refs/heads/main\n");
        }
        escrever(&base.join("solta/x/.git/HEAD"), "ref: refs/heads/main\n");
        let achados: Vec<_> = repositorios_em(base)
            .into_iter()
            .map(|r| r.raiz.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(achados, ["a-legado", "b-web"]);
        // A própria pasta é um repositório: só ela.
        let um = repositorios_em(&base.join("b-web"));
        assert_eq!(um.len(), 1);
        assert_eq!(um[0].raiz, dunce::canonicalize(base.join("b-web")).unwrap());
        assert!(repositorios_em(&base.join("nao-existe")).is_empty());
    }

    #[test]
    fn worktree_aponta_para_o_repositorio_principal() {
        let tmp = tempfile::tempdir().unwrap();
        let principal = tmp.path().join("principal");
        let wt = tmp.path().join("wt");
        escrever(&principal.join(".git/HEAD"), "ref: refs/heads/main\n");
        escrever(&principal.join(".git/refs/heads/main"), C1);
        escrever(&principal.join(".git/refs/heads/outra"), C2);
        let gd = principal.join(".git/worktrees/wt");
        escrever(&gd.join("HEAD"), "ref: refs/heads/outra\n");
        escrever(&gd.join("commondir"), "../..\n");
        escrever(&wt.join(".git"), &format!("gitdir: {}\n", gd.display()));
        let r = detectar_git(&wt).unwrap();
        assert!(r.worktree);
        assert_eq!(r.raiz, dunce::canonicalize(&wt).unwrap());
        assert_eq!(r.raiz_principal, dunce::canonicalize(&principal).unwrap());
        assert_eq!(r.branch.as_deref(), Some("outra"));
        assert_eq!(r.commit.as_deref(), Some(C2));
        // O config (e os remotos) é o do repositório principal.
        escrever(
            &principal.join(".git/config"),
            "[remote \"origin\"]\n\turl = https://github.com/o/r.git\n",
        );
        assert_eq!(remotos_git(&wt), ["https://github.com/o/r.git"]);
    }
}
