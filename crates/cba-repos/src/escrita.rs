//! Escrita no repositório (#275): grava arquivos numa branch própria, faz o push e
//! abre (ou reutiliza) o PR no GitHub (`gh`) ou o MR no GitLab (`glab`).
//!
//! Regras:
//!
//! - **nunca escreve nas branches protegidas**: `develop`, `main`, `master` e a
//!   própria branch base são recusadas como branch de escrita, antes de qualquer
//!   rede;
//! - **não mexe no que o analista tem no clone**: o commit é montado num índice
//!   temporário a partir da branch remota (`git read-tree`, `git add` numa árvore
//!   temporária, `git write-tree`, `git commit-tree`), sem checkout, sem trocar a
//!   branch atual e sem criar nem mover branches locais. O push manda o commit
//!   direto para `refs/heads/<branch>` do remoto, sem `--force`;
//! - **o token só no ambiente do processo filho**: o `git` usa o mesmo credential
//!   helper do clone ([`crate::clone`], `CBA_GIT_TOKEN`), e as CLIs recebem
//!   `GH_TOKEN`/`GITLAB_TOKEN`. Nada na URL, nos argumentos nem no `.git/config`;
//!   as mensagens passam por [`cite`];
//! - **republicar não perde trabalho**: com PR/MR aberto, o commit novo vai em cima
//!   da ponta da branch remota (inclusive commits de revisores); sem PR/MR, uma
//!   branch já mesclada recomeça da base e uma não mesclada é preservada;
//! - **sem mudanças efetivas, nada acontece**: sem commit, sem push e sem PR/MR
//!   novo ([`Situacao::SemMudancas`]).
//!
//! [`atualizar_clone`] (fetch e avanço da branch base, só fast-forward) é pública
//! para a sincronização do repositório de docs (#277). [`ler_remoto`] lê, sem
//! tocar no clone, o que já está numa pasta da branch base e da branch da
//! publicação, para a prévia da publicação dos documentos (#276).

use crate::clone::{comando_git, explicar};
use crate::demanda::{Ferramentas, PRAZO_CLI};
use crate::docs::{juntar, normalizar_subpasta};
use crate::process::{self, RunError, cite};
use crate::{ForgeKind, RepositorioCodigo, Secret};
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// Prazo das operações de rede do git (fetch, push).
pub const PRAZO_REDE: Duration = Duration::from_secs(300);

/// Prazo das operações locais do git.
const PRAZO_LOCAL: Duration = Duration::from_secs(120);

/// Branches que nunca recebem escrita, além da própria branch base.
pub const BRANCHES_PROTEGIDAS: &[&str] = &["develop", "main", "master"];

/// Autor dos commits quando o git do analista não tem `user.name`/`user.email`.
const AUTOR_PADRAO: (&str, &str) = ("ctower-business-agents", "ctower-business-agents@localhost");

/// Uma mudança num arquivo, com o caminho relativo à raiz do repositório (`/`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mudanca {
    /// Cria ou substitui o arquivo.
    Gravar { caminho: String, conteudo: Vec<u8> },
    /// Remove o arquivo (sem ele na branch, não faz nada).
    Remover { caminho: String },
}

impl Mudanca {
    pub fn gravar(caminho: impl Into<String>, conteudo: impl Into<Vec<u8>>) -> Self {
        Mudanca::Gravar {
            caminho: caminho.into(),
            conteudo: conteudo.into(),
        }
    }

    pub fn remover(caminho: impl Into<String>) -> Self {
        Mudanca::Remover {
            caminho: caminho.into(),
        }
    }

    pub fn caminho(&self) -> &str {
        match self {
            Mudanca::Gravar { caminho, .. } | Mudanca::Remover { caminho } => caminho,
        }
    }
}

/// O que publicar e onde.
#[derive(Debug, Clone, Copy)]
pub struct Pedido<'a> {
    /// Repositório de escrita (o de código, no modo subpasta, ou o próprio).
    pub repositorio: &'a RepositorioCodigo,
    /// Clone local do repositório.
    pub pasta: &'a Path,
    /// Branch base do PR/MR; `None` = a padrão do remoto.
    pub base: Option<&'a str>,
    /// Branch da publicação (`docs/faturamento-exemplo`). Nunca uma protegida.
    pub branch: &'a str,
    pub mudancas: &'a [Mudanca],
    /// Mensagem do commit.
    pub mensagem: &'a str,
    /// Título e corpo do PR/MR.
    pub titulo: &'a str,
    pub corpo: &'a str,
}

/// Como a publicação terminou.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Situacao {
    /// PR/MR aberto agora (com commit novo ou com a branch que já estava no
    /// remoto sem PR/MR).
    Aberto,
    /// Commit novo na branch de um PR/MR que já estava aberto.
    Atualizado,
    /// Nada mudou: o conteúdo já está na branch do PR/MR aberto ou na base. Sem
    /// commit, sem push e sem PR/MR novo.
    SemMudancas,
}

/// Resultado de [`publicar`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Publicado {
    pub situacao: Situacao,
    /// Branch da publicação e branch base (já resolvida).
    pub branch: String,
    pub base: String,
    /// Commit criado nesta publicação; `None` sem commit novo.
    pub commit: Option<String>,
    /// Link do PR/MR (aberto agora ou reutilizado); `None` só em
    /// [`Situacao::SemMudancas`] sem PR/MR aberto.
    pub link: Option<String>,
}

/// Resultado de [`atualizar_clone`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Atualizacao {
    /// Remoto do clone que aponta para o repositório (`origin`).
    pub remoto: String,
    /// Branch base resolvida (a do cadastro ou a padrão do remoto).
    pub base: String,
    /// Ponta da branch base no remoto, depois do fetch.
    pub commit: String,
    /// A branch base local andou (fast-forward).
    pub avancou: bool,
    /// O clone está com a branch base no checkout: a pasta reflete a base (mais
    /// os commits locais ainda não enviados, se houver).
    pub pasta_na_base: bool,
}

/// Por que a escrita não aconteceu. As mensagens são em pt-BR e nunca trazem o
/// token.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ErroEscrita {
    /// Dados do pedido ou do clone inválidos (caminho, nome de branch, pasta).
    #[error("{0}")]
    Invalido(String),
    #[error(
        "a branch {0} é protegida: a publicação usa uma branch própria e abre um PR/MR; \
         escolha outro nome de branch"
    )]
    BranchProtegida(String),
    #[error(
        "o clone em {pasta} tem alterações não comitadas em {}: comite, descarte ou mova-as \
         antes de continuar",
        lista(arquivos)
    )]
    CloneSujo {
        pasta: String,
        arquivos: Vec<String>,
    },
    #[error("conflito: {0}")]
    Conflito(String),
    #[error("sem permissão de escrita: {0}")]
    SemPermissao(String),
    #[error("{0} não encontrado: instale-o e deixe-o no PATH (confira a prontidão do projeto)")]
    SemFerramenta(&'static str),
    #[error("{0}")]
    Falhou(String),
}

/// Até cinco itens e "mais N".
fn lista(itens: &[String]) -> String {
    const MAX: usize = 5;
    let mut s = itens
        .iter()
        .take(MAX)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    if itens.len() > MAX {
        s.push_str(&format!(" e mais {}", itens.len() - MAX));
    }
    s
}

/// Confere o nome da branch de escrita: um nome de branch válido e nunca
/// `develop`, `main`, `master` nem a `base` (sem diferenciar maiúsculas).
pub fn validar_branch_de_escrita(branch: &str, base: Option<&str>) -> Result<(), ErroEscrita> {
    let b = branch.trim();
    if b.is_empty() {
        return Err(ErroEscrita::Invalido(
            "informe a branch da publicação".into(),
        ));
    }
    let invalido = b.starts_with(['-', '/', '.'])
        || b.ends_with(['/', '.'])
        || b.ends_with(".lock")
        || b.starts_with("refs/")
        || b == "HEAD"
        || b.contains("..")
        || b.contains("//")
        || b.contains("@{")
        || b.contains("/.")
        || b.chars()
            .any(|c| c.is_control() || c.is_whitespace() || "~^:?*[\\".contains(c));
    if invalido {
        return Err(ErroEscrita::Invalido(format!(
            "nome de branch inválido: {b:?}"
        )));
    }
    let protegida = BRANCHES_PROTEGIDAS
        .iter()
        .copied()
        .chain(base.map(str::trim).filter(|s| !s.is_empty()))
        .any(|p| p.eq_ignore_ascii_case(b));
    if protegida {
        return Err(ErroEscrita::BranchProtegida(b.to_string()));
    }
    Ok(())
}

/// Caminhos das mudanças normalizados (`/`, relativos, sem `..` nem `.git`) e sem
/// repetição.
fn validar_mudancas(mudancas: &[Mudanca]) -> Result<Vec<String>, ErroEscrita> {
    let mut vistos = HashSet::new();
    mudancas
        .iter()
        .map(|m| {
            let c = normalizar_subpasta(m.caminho())
                .map_err(|e| ErroEscrita::Invalido(e.replace("subpasta", "caminho")))?;
            if c.is_empty() {
                return Err(ErroEscrita::Invalido(
                    "caminho vazio na lista de arquivos".into(),
                ));
            }
            if !vistos.insert(c.clone()) {
                return Err(ErroEscrita::Invalido(format!(
                    "o arquivo {c} aparece mais de uma vez na publicação"
                )));
            }
            Ok(c)
        })
        .collect()
}

/// O git de um clone, com o token só nas operações de rede.
struct Git<'a> {
    bin: &'a Path,
    /// Raiz do clone (`git rev-parse --show-toplevel`).
    raiz: PathBuf,
    forja: ForgeKind,
    token: Option<&'a Secret>,
}

impl<'a> Git<'a> {
    /// Abre o clone em `pasta` e acha o remoto que aponta para `repo`.
    fn abrir(
        bin: &'a Path,
        repo: &RepositorioCodigo,
        pasta: &Path,
        token: Option<&'a Secret>,
    ) -> Result<(Git<'a>, String), ErroEscrita> {
        if !pasta.is_dir() {
            return Err(ErroEscrita::Invalido(format!(
                "a pasta do clone de {} não existe: {} (clone o repositório antes)",
                repo.rotulo(),
                pasta.display()
            )));
        }
        let mut g = Git {
            bin,
            raiz: pasta.to_path_buf(),
            forja: repo.forja,
            token,
        };
        let raiz = g
            .tentar(&["rev-parse", "--show-toplevel"])?
            .filter(|r| !r.is_empty())
            .ok_or_else(|| {
                ErroEscrita::Invalido(format!("{} não é um clone do git", pasta.display()))
            })?;
        g.raiz = PathBuf::from(raiz);
        let url = repo.url_efetiva();
        let remotos = g
            .tentar(&["config", "--get-regexp", r"^remote\..*\.url$"])?
            .unwrap_or_default();
        let remoto = remotos.lines().find_map(|l| {
            let (chave, valor) = l.split_once(' ')?;
            let nome = chave.strip_prefix("remote.")?.strip_suffix(".url")?;
            (repo.aponta_para(valor) || valor.trim() == url).then(|| nome.to_string())
        });
        match remoto {
            Some(r) if !r.starts_with('-') => Ok((g, r)),
            _ => Err(ErroEscrita::Invalido(format!(
                "o clone em {} não tem um remoto que aponte para {}: confira a pasta do \
                 repositório no cadastro",
                g.raiz.display(),
                repo.rotulo()
            ))),
        }
    }

    /// `git` na raiz do clone; com `rede`, com o token (se houver) no ambiente.
    fn comando(&self, rede: bool) -> Command {
        let mut c = comando_git(self.bin, self.forja, if rede { self.token } else { None });
        c.current_dir(&self.raiz);
        for v in [
            "GIT_DIR",
            "GIT_WORK_TREE",
            "GIT_INDEX_FILE",
            "GIT_LITERAL_PATHSPECS",
            "GIT_GLOB_PATHSPECS",
            "GIT_NOGLOB_PATHSPECS",
            "GIT_ICASE_PATHSPECS",
        ] {
            c.env_remove(v);
        }
        c
    }

    fn executar(&self, c: Command, prazo: Duration) -> Result<process::Output, ErroEscrita> {
        process::run(c, prazo).map_err(|e| match e {
            RunError::NotFound => ErroEscrita::SemFerramenta("git"),
            RunError::Timeout(d) => ErroEscrita::Falhou(format!(
                "o git não terminou em {} s: confira a rede e o servidor",
                d.as_secs()
            )),
            e => ErroEscrita::Falhou(cite(&e.to_string(), self.token)),
        })
    }

    fn erro_local(&self, args: &[&str], o: &process::Output) -> ErroEscrita {
        let detalhe = cite(&String::from_utf8_lossy(&o.stderr), self.token);
        ErroEscrita::Falhou(format!(
            "git {} falhou: {}",
            args.first().copied().unwrap_or(""),
            if detalhe.is_empty() {
                process::describe_status(&o.status)
            } else {
                detalhe
            }
        ))
    }

    /// Roda um comando local que precisa dar certo; devolve o `stdout` sem o fim
    /// de linha.
    fn local(&self, args: &[&str]) -> Result<String, ErroEscrita> {
        let mut c = self.comando(false);
        c.args(args);
        self.saida(c, args)
    }

    fn saida(&self, c: Command, args: &[&str]) -> Result<String, ErroEscrita> {
        let o = self.executar(c, PRAZO_LOCAL)?;
        if o.status.success() {
            Ok(String::from_utf8_lossy(&o.stdout).trim_end().to_string())
        } else {
            Err(self.erro_local(args, &o))
        }
    }

    /// Roda um comando local que pode falhar (`None`).
    fn tentar(&self, args: &[&str]) -> Result<Option<String>, ErroEscrita> {
        let mut c = self.comando(false);
        c.args(args);
        let o = self.executar(c, PRAZO_LOCAL)?;
        Ok(o.status
            .success()
            .then(|| String::from_utf8_lossy(&o.stdout).trim_end().to_string()))
    }

    /// Commit de uma referência, se ela existir.
    fn commit_de(&self, referencia: &str) -> Result<Option<String>, ErroEscrita> {
        self.tentar(&[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{referencia}^{{commit}}"),
        ])
    }

    fn arvore_de(&self, commit: &str) -> Result<String, ErroEscrita> {
        self.local(&["rev-parse", &format!("{commit}^{{tree}}")])
    }

    /// `a` é ancestral de `b` (ou igual).
    fn ancestral(&self, a: &str, b: &str) -> Result<bool, ErroEscrita> {
        let mut c = self.comando(false);
        c.args(["merge-base", "--is-ancestor", a, b]);
        let o = self.executar(c, PRAZO_LOCAL)?;
        match o.status.code() {
            Some(0) => Ok(true),
            Some(1) => Ok(false),
            _ => Err(self.erro_local(&["merge-base"], &o)),
        }
    }

    /// Busca todas as branches do remoto (e apaga as que sumiram dele).
    fn buscar(&self, remoto: &str) -> Result<(), ErroEscrita> {
        let refspec = format!("+refs/heads/*:refs/remotes/{remoto}/*");
        let mut c = self.comando(true);
        c.args(["fetch", "--prune", "--no-tags", "--quiet", remoto, &refspec]);
        let o = self.executar(c, PRAZO_REDE)?;
        if o.status.success() {
            Ok(())
        } else {
            Err(ErroEscrita::Falhou(format!(
                "não foi possível atualizar o clone: {}",
                explicar(&String::from_utf8_lossy(&o.stderr), self.token)
            )))
        }
    }

    /// A branch base: a informada ou a padrão do remoto.
    fn resolver_base(&self, remoto: &str, base: Option<&str>) -> Result<String, ErroEscrita> {
        if let Some(b) = base.map(str::trim).filter(|b| !b.is_empty()) {
            return Ok(b.to_string());
        }
        let prefixo = format!("{remoto}/");
        if let Some(s) = self.tentar(&[
            "symbolic-ref",
            "--quiet",
            "--short",
            &format!("refs/remotes/{remoto}/HEAD"),
        ])? && let Some(b) = s.strip_prefix(&prefixo)
            && !b.is_empty()
        {
            return Ok(b.to_string());
        }
        let mut c = self.comando(true);
        c.args(["ls-remote", "--symref", remoto, "HEAD"]);
        let o = self.executar(c, PRAZO_REDE)?;
        let saida = String::from_utf8_lossy(&o.stdout);
        let padrao = o.status.success().then(|| {
            saida.lines().find_map(|l| {
                let (r, _) = l
                    .strip_prefix("ref:")?
                    .trim()
                    .split_once(char::is_whitespace)?;
                r.strip_prefix("refs/heads/").map(str::to_string)
            })
        });
        padrao.flatten().ok_or_else(|| {
            ErroEscrita::Invalido(
                "não foi possível descobrir a branch padrão do repositório: informe a branch \
                 base no cadastro"
                    .into(),
            )
        })
    }

    /// Ponta da branch base no remoto (depois do fetch).
    fn ponta_da_base(&self, remoto: &str, base: &str) -> Result<String, ErroEscrita> {
        self.commit_de(&format!("refs/remotes/{remoto}/{base}"))?
            .ok_or_else(|| {
                ErroEscrita::Invalido(format!(
                    "a branch base {base} não existe no remoto {remoto}: confira a branch no \
                     cadastro"
                ))
            })
    }

    /// Arquivos com alteração não comitada (ou não rastreados) no clone; com
    /// `rastreados`, só os rastreados.
    fn alterados(&self, so_rastreados: bool) -> Result<Vec<String>, ErroEscrita> {
        let args = [
            "status",
            "--porcelain=v1",
            "-z",
            if so_rastreados {
                "--untracked-files=no"
            } else {
                "--untracked-files=all"
            },
        ];
        let mut c = self.comando(false);
        c.args(args);
        let o = self.executar(c, PRAZO_LOCAL)?;
        if !o.status.success() {
            return Err(self.erro_local(&args, &o));
        }
        let saida = String::from_utf8_lossy(&o.stdout);
        let mut partes = saida.split('\0');
        let mut arquivos = Vec::new();
        while let Some(p) = partes.next() {
            if p.len() < 4 {
                continue;
            }
            let (estado, caminho) = p.split_at(3);
            // Renomeado/copiado: o caminho de origem vem a seguir.
            if estado.starts_with(['R', 'C']) {
                partes.next();
            }
            arquivos.push(caminho.to_string());
        }
        Ok(arquivos)
    }

    fn erro_sujo(&self, arquivos: Vec<String>) -> ErroEscrita {
        ErroEscrita::CloneSujo {
            pasta: self.raiz.display().to_string(),
            arquivos,
        }
    }

    /// Monta, num índice temporário, a árvore de `pai` com as `mudancas`.
    fn montar_arvore(
        &self,
        pai: &str,
        mudancas: &[Mudanca],
        caminhos: &[String],
    ) -> Result<String, ErroEscrita> {
        let git_dir = self.local(&["rev-parse", "--absolute-git-dir"])?;
        let tmp = Temporario::novo()?;
        let comando = || {
            let mut c = self.comando(false);
            c.current_dir(tmp.arvore())
                .env("GIT_DIR", &git_dir)
                .env("GIT_WORK_TREE", tmp.arvore())
                .env("GIT_INDEX_FILE", tmp.indice())
                .env("GIT_LITERAL_PATHSPECS", "1");
            c
        };
        let rodar = |args: &[&str]| {
            let mut c = comando();
            c.args(args);
            self.saida(c, args)
        };
        rodar(&["read-tree", pai])?;
        let mut gravar = Vec::new();
        let mut remover = Vec::new();
        for (m, caminho) in mudancas.iter().zip(caminhos) {
            match m {
                Mudanca::Gravar { conteudo, .. } => {
                    let destino = juntar(&tmp.arvore(), caminho);
                    if let Some(p) = destino.parent() {
                        std::fs::create_dir_all(p).map_err(|e| erro_tmp(&e))?;
                    }
                    std::fs::write(&destino, conteudo).map_err(|e| erro_tmp(&e))?;
                    gravar.push(caminho.as_str());
                }
                Mudanca::Remover { .. } => remover.push(caminho.as_str()),
            }
        }
        // A lista vai num arquivo (separada por NUL): sem limite de linha de
        // comando com muitos arquivos.
        if !remover.is_empty() {
            let lista = tmp.lista("remover", &remover)?;
            rodar(&[
                "rm",
                "--cached",
                "-f",
                "-q",
                "--ignore-unmatch",
                &format!("--pathspec-from-file={}", lista.display()),
                "--pathspec-file-nul",
            ])?;
        }
        if !gravar.is_empty() {
            let lista = tmp.lista("gravar", &gravar)?;
            rodar(&[
                "add",
                "-f",
                &format!("--pathspec-from-file={}", lista.display()),
                "--pathspec-file-nul",
            ])?;
        }
        rodar(&["write-tree"])
    }

    /// Cria o commit `arvore` sobre `pai`, com o autor do git do analista ou, sem
    /// ele, o [`AUTOR_PADRAO`].
    fn comitar(&self, arvore: &str, pai: &str, mensagem: &str) -> Result<String, ErroEscrita> {
        let args = ["commit-tree", arvore, "-p", pai, "-m", mensagem];
        let mut c = self.comando(false);
        let com_autor = self.tentar(&["var", "GIT_AUTHOR_IDENT"])?.is_some()
            && self.tentar(&["var", "GIT_COMMITTER_IDENT"])?.is_some();
        if !com_autor {
            let (nome, email) = AUTOR_PADRAO;
            c.env("GIT_AUTHOR_NAME", nome)
                .env("GIT_AUTHOR_EMAIL", email)
                .env("GIT_COMMITTER_NAME", nome)
                .env("GIT_COMMITTER_EMAIL", email);
        }
        c.args(args);
        self.saida(c, &args)
    }

    /// Envia `commit` para a `branch` do remoto, sem `--force` (só avança).
    fn enviar(&self, remoto: &str, commit: &str, branch: &str) -> Result<(), ErroEscrita> {
        let mut c = self.comando(true);
        // `--no-verify`: o commit é só de documentação; os hooks de pre-push do
        // código (testes, lint) não se aplicam e podem pedir o terminal.
        c.args([
            "push",
            "--porcelain",
            "--no-verify",
            remoto,
            &format!("{commit}:refs/heads/{branch}"),
        ]);
        let o = self.executar(c, PRAZO_REDE)?;
        if o.status.success() {
            return Ok(());
        }
        let texto = format!(
            "{}\n{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        );
        Err(erro_do_push(&texto, branch, self.token))
    }
}

/// Classifica a falha do push.
fn erro_do_push(texto: &str, branch: &str, token: Option<&Secret>) -> ErroEscrita {
    let t = texto.to_ascii_lowercase();
    let detalhe = cite(texto, token);
    if t.contains("non-fast-forward") || t.contains("fetch first") || t.contains("stale info") {
        return ErroEscrita::Conflito(format!(
            "a branch {branch} mudou no remoto durante a publicação: publique de novo ({detalhe})"
        ));
    }
    if t.contains("protected branch") || t.contains("gh006") {
        return ErroEscrita::SemPermissao(format!(
            "a forja protege a branch {branch}: escolha outro nome de branch ({detalhe})"
        ));
    }
    if (t.contains("permission to") && t.contains("denied"))
        || t.contains("not allowed to push")
        || t.contains("write access to repository not granted")
        || t.contains("403")
        || t.contains("forbidden")
    {
        return ErroEscrita::SemPermissao(format!(
            "o token (ou a sua credencial) não pode enviar para o repositório: ele precisa de \
             permissão de escrita ({detalhe})"
        ));
    }
    ErroEscrita::Falhou(format!("o push falhou: {}", explicar(texto, token)))
}

fn erro_tmp(e: &std::io::Error) -> ErroEscrita {
    ErroEscrita::Falhou(format!(
        "não foi possível preparar os arquivos da publicação: {e}"
    ))
}

/// Pasta temporária da publicação (índice, árvore e listas), apagada no fim.
struct Temporario(PathBuf);

impl Temporario {
    fn novo() -> Result<Self, ErroEscrita> {
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!(
            "cba-publicacao-{}-{nanos}-{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(dir.join("arvore")).map_err(|e| erro_tmp(&e))?;
        Ok(Temporario(dir))
    }

    fn arvore(&self) -> PathBuf {
        self.0.join("arvore")
    }

    fn indice(&self) -> PathBuf {
        self.0.join("index")
    }

    fn lista(&self, nome: &str, caminhos: &[&str]) -> Result<PathBuf, ErroEscrita> {
        let p = self.0.join(format!("{nome}.lst"));
        let mut dados = Vec::new();
        for c in caminhos {
            dados.extend_from_slice(c.as_bytes());
            dados.push(0);
        }
        std::fs::write(&p, dados).map_err(|e| erro_tmp(&e))?;
        Ok(p)
    }
}

impl Drop for Temporario {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// PR/MR aberto.
struct Pr {
    url: String,
}

/// A API da forja pelas CLIs, com o token só no ambiente.
struct Forja<'a> {
    f: &'a Ferramentas,
    repo: &'a RepositorioCodigo,
    token: Option<&'a Secret>,
}

/// Codifica um valor para a query string (tudo menos `A-Za-z0-9-._~`).
fn codificar(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

impl Forja<'_> {
    fn nome(&self) -> &'static str {
        match self.repo.forja {
            ForgeKind::Github => "gh",
            ForgeKind::Gitlab => "glab",
        }
    }

    /// Caminho do repositório (`dono/repo`), conferido para ir numa rota da API.
    fn caminho(&self) -> Result<&str, ErroEscrita> {
        let c = self.repo.caminho.trim().trim_matches('/');
        let c = c.strip_suffix(".git").unwrap_or(c);
        let ok = !c.is_empty()
            && c.contains('/')
            && c.chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '/'))
            && !c.split('/').any(|p| p.is_empty() || p == "." || p == "..");
        if ok {
            Ok(c)
        } else {
            Err(ErroEscrita::Invalido(format!(
                "caminho do repositório inválido para a API da forja: {c:?}"
            )))
        }
    }

    /// Rota da API do projeto: `repos/dono/repo` ou `projects/grupo%2Fapp`.
    fn rota(&self) -> Result<String, ErroEscrita> {
        let c = self.caminho()?;
        Ok(match self.repo.forja {
            ForgeKind::Github => format!("repos/{c}"),
            ForgeKind::Gitlab => format!("projects/{}", c.replace('/', "%2F")),
        })
    }

    fn api(&self, args: &[&str]) -> Result<Value, ErroEscrita> {
        let host = self.repo.host.trim().to_ascii_lowercase();
        let ferramenta = self.nome();
        let mut c = match self.repo.forja {
            ForgeKind::Github => {
                let mut c = Command::new(&self.f.gh);
                c.env("GH_HOST", &host).env("GH_PROMPT_DISABLED", "1");
                if let Some(t) = self.token {
                    c.env("GH_TOKEN", t.expose());
                }
                c
            }
            ForgeKind::Gitlab => {
                let mut c = Command::new(&self.f.glab);
                c.env("GITLAB_HOST", &host).env("NO_PROMPT", "1");
                if let Some(t) = self.token {
                    c.env("GITLAB_TOKEN", t.expose());
                }
                c
            }
        };
        c.env("NO_COLOR", "1").arg("api").args(args);
        let o = process::run(c, PRAZO_CLI).map_err(|e| match e {
            RunError::NotFound => ErroEscrita::SemFerramenta(ferramenta),
            RunError::Timeout(_) => ErroEscrita::Falhou(format!(
                "a API do {} não respondeu a tempo: confira o host e a rede",
                self.repo.forja.label()
            )),
            e => ErroEscrita::Falhou(cite(&e.to_string(), self.token)),
        })?;
        if !o.status.success() {
            let detalhe = cite(
                &format!(
                    "{} {}",
                    String::from_utf8_lossy(&o.stderr),
                    String::from_utf8_lossy(&o.stdout)
                ),
                self.token,
            );
            return Err(self.erro_api(&detalhe));
        }
        serde_json::from_slice(&o.stdout)
            .map_err(|e| ErroEscrita::Falhou(format!("resposta de {ferramenta} ilegível: {e}")))
    }

    fn erro_api(&self, detalhe: &str) -> ErroEscrita {
        let t = detalhe.to_ascii_lowercase();
        let forja = self.repo.forja.label();
        if t.contains("403")
            || t.contains("forbidden")
            || t.contains("resource not accessible")
            || t.contains("must have push access")
        {
            ErroEscrita::SemPermissao(format!(
                "a API do {forja} negou o acesso: o token precisa de permissão de escrita no \
                 repositório e de abrir PR/MR ({detalhe})"
            ))
        } else if t.contains("401") || t.contains("bad credentials") || t.contains("unauthorized") {
            ErroEscrita::Falhou(format!(
                "a API do {forja} recusou o token (inválido, expirado ou revogado) ({detalhe})"
            ))
        } else if t.contains("404") || t.contains("not found") {
            ErroEscrita::Falhou(format!(
                "a API do {forja} não encontrou {} (o repositório não existe ou o token não tem \
                 acesso a ele) ({detalhe})",
                self.repo.caminho.trim()
            ))
        } else {
            ErroEscrita::Falhou(format!("{} falhou: {detalhe}", self.nome()))
        }
    }

    fn pr_do_json(&self, v: &Value) -> Option<Pr> {
        let campo = match self.repo.forja {
            ForgeKind::Github => "html_url",
            ForgeKind::Gitlab => "web_url",
        };
        v[campo]
            .as_str()
            .filter(|u| !u.is_empty())
            .map(|u| Pr { url: u.to_string() })
    }

    /// O PR/MR aberto da `branch`, se houver.
    fn procurar(&self, branch: &str) -> Result<Option<Pr>, ErroEscrita> {
        let rota = self.rota()?;
        let consulta = match self.repo.forja {
            ForgeKind::Github => {
                let dono = self.caminho()?.split('/').next().unwrap_or("");
                format!(
                    "{rota}/pulls?state=open&head={}",
                    codificar(&format!("{dono}:{branch}"))
                )
            }
            ForgeKind::Gitlab => format!(
                "{rota}/merge_requests?state=opened&source_branch={}",
                codificar(branch)
            ),
        };
        let v = self.api(&[&consulta])?;
        Ok(v.as_array()
            .and_then(|a| a.iter().find_map(|p| self.pr_do_json(p))))
    }

    /// Abre o PR/MR; se outro abriu um no meio-tempo, devolve esse.
    fn abrir(
        &self,
        branch: &str,
        base: &str,
        titulo: &str,
        corpo: &str,
    ) -> Result<Pr, ErroEscrita> {
        let rota = self.rota()?;
        let campos = match self.repo.forja {
            ForgeKind::Github => [
                ("head", branch),
                ("base", base),
                ("title", titulo),
                ("body", corpo),
            ],
            ForgeKind::Gitlab => [
                ("source_branch", branch),
                ("target_branch", base),
                ("title", titulo),
                ("description", corpo),
            ],
        };
        let destino = match self.repo.forja {
            ForgeKind::Github => format!("{rota}/pulls"),
            ForgeKind::Gitlab => format!("{rota}/merge_requests"),
        };
        let campos: Vec<String> = campos.iter().map(|(k, v)| format!("{k}={v}")).collect();
        let mut args = vec!["--method", "POST", destino.as_str()];
        for c in &campos {
            args.extend(["-f", c.as_str()]);
        }
        match self.api(&args) {
            Ok(v) => self.pr_do_json(&v).ok_or_else(|| {
                ErroEscrita::Falhou(format!(
                    "{} abriu o PR/MR mas não devolveu o link",
                    self.nome()
                ))
            }),
            Err(e) => {
                // Corrida: o PR/MR já existe (GitHub 422, GitLab 409).
                let ja_existe = matches!(&e, ErroEscrita::Falhou(m)
                    if m.to_ascii_lowercase().contains("already exists"));
                match self.procurar(branch)? {
                    Some(pr) if ja_existe => Ok(pr),
                    _ => Err(e),
                }
            }
        }
    }
}

/// Atualiza o clone: busca o remoto (`git fetch`, com o token só no ambiente) e
/// avança a branch base local só por fast-forward. Com a base no checkout, a
/// pasta é atualizada (`git merge --ff-only`) e alterações não comitadas em
/// arquivos rastreados são recusadas ([`ErroEscrita::CloneSujo`]); fora do
/// checkout, só a referência anda. Base local com commits que o remoto não tem e
/// vice-versa é [`ErroEscrita::Conflito`]; base só à frente do remoto fica como
/// está. Sem branch base local, nada é criado.
pub fn atualizar_clone(
    git: &Path,
    repo: &RepositorioCodigo,
    pasta: &Path,
    base: Option<&str>,
    token: Option<&Secret>,
) -> Result<Atualizacao, ErroEscrita> {
    let (g, remoto) = Git::abrir(git, repo, pasta, token)?;
    g.buscar(&remoto)?;
    let base = g.resolver_base(&remoto, base)?;
    let remoto_sha = g.ponta_da_base(&remoto, &base)?;
    let ref_local = format!("refs/heads/{base}");
    let local = g.commit_de(&ref_local)?;
    let no_checkout = g
        .tentar(&["symbolic-ref", "--quiet", "HEAD"])?
        .is_some_and(|h| h == ref_local);
    let mut atualizacao = Atualizacao {
        remoto: remoto.clone(),
        base: base.clone(),
        commit: remoto_sha.clone(),
        avancou: false,
        pasta_na_base: no_checkout,
    };
    let Some(local) = local else {
        return Ok(atualizacao);
    };
    if no_checkout {
        let sujos = g.alterados(true)?;
        if !sujos.is_empty() {
            return Err(g.erro_sujo(sujos));
        }
    }
    if g.ancestral(&remoto_sha, &local)? {
        // Em dia ou à frente do remoto.
        return Ok(atualizacao);
    }
    if !g.ancestral(&local, &remoto_sha)? {
        return Err(ErroEscrita::Conflito(format!(
            "a branch {base} do clone e a do remoto {remoto} divergiram (as duas têm commits que \
             a outra não tem): resolva no clone (pull ou rebase) antes de continuar"
        )));
    }
    if no_checkout {
        let args = ["merge", "--ff-only", "--quiet", remoto_sha.as_str()];
        let mut c = g.comando(false);
        c.args(args);
        let o = g.executar(c, PRAZO_LOCAL)?;
        if !o.status.success() {
            let stderr = String::from_utf8_lossy(&o.stderr);
            // "untracked working tree files would be overwritten": a lista vem em
            // linhas com tabulação.
            if stderr.contains("would be overwritten") {
                let arquivos = stderr
                    .lines()
                    .filter(|l| l.starts_with('\t'))
                    .map(|l| l.trim().to_string())
                    .collect();
                return Err(g.erro_sujo(arquivos));
            }
            return Err(g.erro_local(&args, &o));
        }
    } else {
        g.local(&["update-ref", &ref_local, &remoto_sha, &local])?;
    }
    atualizacao.avancou = true;
    Ok(atualizacao)
}

/// Formato dos ids de objeto do repositório (`git rev-parse --show-object-format`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FormatoObjeto {
    #[default]
    Sha1,
    Sha256,
}

impl FormatoObjeto {
    /// Id do blob do git com este conteúdo (o mesmo de `git hash-object
    /// --no-filters`), para comparar com o que está publicado sem ler o arquivo.
    pub fn id_do_blob(self, conteudo: &[u8]) -> String {
        use sha1::Digest as _;
        let cabecalho = format!("blob {}\0", conteudo.len());
        let bytes: Vec<u8> = match self {
            FormatoObjeto::Sha1 => {
                let mut h = sha1::Sha1::new();
                h.update(cabecalho.as_bytes());
                h.update(conteudo);
                h.finalize().to_vec()
            }
            FormatoObjeto::Sha256 => {
                let mut h = sha2::Sha256::new();
                h.update(cabecalho.as_bytes());
                h.update(conteudo);
                h.finalize().to_vec()
            }
        };
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }
}

/// Os arquivos de uma pasta na ponta de uma branch do remoto.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ArvoreRemota {
    /// Ponta da branch no remoto, depois do fetch.
    pub commit: String,
    /// Caminho relativo à raiz do repositório (`/`) → id do blob.
    pub arquivos: BTreeMap<String, String>,
    /// Conteúdo dos arquivos pedidos em `ler` que existem na branch.
    pub conteudos: BTreeMap<String, Vec<u8>>,
}

/// Resultado de [`ler_remoto`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LeituraRemota {
    /// Branch base resolvida (a informada ou a padrão do remoto).
    pub base: String,
    pub na_base: ArvoreRemota,
    /// A branch da publicação, se ela existe no remoto.
    pub na_branch: Option<ArvoreRemota>,
    pub formato: FormatoObjeto,
}

/// Onde ler: o clone do repositório, a branch base e a da publicação.
#[derive(Debug, Clone, Copy)]
pub struct Leitura<'a> {
    pub repositorio: &'a RepositorioCodigo,
    /// Clone local do repositório.
    pub pasta: &'a Path,
    /// Branch base; `None` = a padrão do remoto.
    pub base: Option<&'a str>,
    /// Branch da publicação (lida se existir no remoto).
    pub branch: &'a str,
    /// Pasta dentro do repositório (`/`; vazia = tudo).
    pub pasta_no_repositorio: &'a str,
    /// Arquivos (relativos à raiz do repositório) cujo conteúdo é devolvido.
    pub ler: &'a [&'a str],
}

/// Lê, sem tocar no checkout, o que está numa pasta da branch base e da branch da
/// publicação no remoto: busca o remoto (`git fetch`, com o token só no
/// ambiente), lista os arquivos com o id do blob e traz o conteúdo dos caminhos
/// pedidos. Serve à prévia da publicação (#276), que compara o que vai ser
/// publicado com o que já está lá.
pub fn ler_remoto(
    git: &Path,
    token: Option<&Secret>,
    l: &Leitura<'_>,
) -> Result<LeituraRemota, ErroEscrita> {
    let prefixo = normalizar_subpasta(l.pasta_no_repositorio)
        .map_err(|e| ErroEscrita::Invalido(e.replace("subpasta", "pasta")))?;
    let (g, remoto) = Git::abrir(git, l.repositorio, l.pasta, token)?;
    g.buscar(&remoto)?;
    let base = g.resolver_base(&remoto, l.base)?;
    let base_sha = g.ponta_da_base(&remoto, &base)?;
    let formato = match g.tentar(&["rev-parse", "--show-object-format"])?.as_deref() {
        Some("sha256") => FormatoObjeto::Sha256,
        _ => FormatoObjeto::Sha1,
    };
    let na_base = g.arvore_remota(&base_sha, &prefixo, l.ler)?;
    let branch = l.branch.trim();
    let na_branch = match g.commit_de(&format!("refs/remotes/{remoto}/{branch}"))? {
        Some(c) if !branch.is_empty() => Some(g.arvore_remota(&c, &prefixo, l.ler)?),
        _ => None,
    };
    Ok(LeituraRemota {
        base,
        na_base,
        na_branch,
        formato,
    })
}

impl Git<'_> {
    /// Arquivos de `prefixo` no `commit`, com o conteúdo dos de `ler`.
    fn arvore_remota(
        &self,
        commit: &str,
        prefixo: &str,
        ler: &[&str],
    ) -> Result<ArvoreRemota, ErroEscrita> {
        let mut args = vec!["ls-tree", "-r", "-z", "--full-tree", commit];
        if !prefixo.is_empty() {
            args.extend(["--", prefixo]);
        }
        let mut c = self.comando(false);
        c.env("GIT_LITERAL_PATHSPECS", "1").args(&args);
        let saida = self.saida_bruta(c, &args)?;
        let mut arquivos = BTreeMap::new();
        for linha in saida.split(|b| *b == 0).filter(|l| !l.is_empty()) {
            let linha = String::from_utf8_lossy(linha);
            let Some((meta, caminho)) = linha.split_once('\t') else {
                continue;
            };
            let mut partes = meta.split(' ');
            let (_, tipo, id) = (partes.next(), partes.next(), partes.next());
            if let (Some("blob"), Some(id)) = (tipo, id) {
                arquivos.insert(caminho.to_string(), id.to_string());
            }
        }
        let mut conteudos = BTreeMap::new();
        for caminho in ler {
            if let Some(id) = arquivos.get(*caminho) {
                let args = ["cat-file", "blob", id.as_str()];
                let mut c = self.comando(false);
                c.args(args);
                conteudos.insert(caminho.to_string(), self.saida_bruta(c, &args)?);
            }
        }
        Ok(ArvoreRemota {
            commit: commit.to_string(),
            arquivos,
            conteudos,
        })
    }

    /// Como [`Git::saida`], com o `stdout` inteiro, em bytes.
    fn saida_bruta(&self, c: Command, args: &[&str]) -> Result<Vec<u8>, ErroEscrita> {
        let o = self.executar(c, PRAZO_LOCAL)?;
        if o.status.success() {
            Ok(o.stdout)
        } else {
            Err(self.erro_local(args, &o))
        }
    }
}

/// Publica as mudanças numa branch própria e abre (ou reutiliza) o PR/MR.
///
/// 1. confere o pedido e a branch (nunca uma protegida) e recusa se algum dos
///    arquivos publicados tem alteração não comitada no clone;
/// 2. busca o remoto e resolve a branch base;
/// 3. procura o PR/MR aberto da branch (falha cedo sem a CLI ou sem permissão);
/// 4. monta o commit sobre a ponta da branch remota (PR/MR aberto ou branch não
///    mesclada) ou sobre a base, sem tocar no checkout do analista;
/// 5. sem mudança efetiva, para ([`Situacao::SemMudancas`]); senão faz o push
///    (sem `--force`) e abre o PR/MR se ainda não houver.
///
/// `token` é o do repositório ([`crate::chaveiro::token_do_repositorio`]); sem
/// ele, valem a credencial do git e a autenticação das CLIs do analista.
pub fn publicar(
    f: &Ferramentas,
    token: Option<&Secret>,
    p: &Pedido<'_>,
) -> Result<Publicado, ErroEscrita> {
    validar_branch_de_escrita(p.branch, p.base)?;
    let branch = p.branch.trim();
    if p.mensagem.trim().is_empty() {
        return Err(ErroEscrita::Invalido("informe a mensagem do commit".into()));
    }
    if p.titulo.trim().is_empty() {
        return Err(ErroEscrita::Invalido("informe o título do PR/MR".into()));
    }
    let caminhos = validar_mudancas(p.mudancas)?;
    let (g, remoto) = Git::abrir(&f.git, p.repositorio, p.pasta, token)?;

    // O analista mexeu à mão num arquivo que vai ser publicado: publicar por cima
    // perderia a edição (ou travaria o próximo pull).
    let publicados: HashSet<&str> = caminhos.iter().map(String::as_str).collect();
    let sujos: Vec<String> = g
        .alterados(false)?
        .into_iter()
        .filter(|a| publicados.contains(a.as_str()))
        .collect();
    if !sujos.is_empty() {
        return Err(g.erro_sujo(sujos));
    }

    g.buscar(&remoto)?;
    let base = g.resolver_base(&remoto, p.base)?;
    validar_branch_de_escrita(branch, Some(&base))?;
    let base_sha = g.ponta_da_base(&remoto, &base)?;

    let forja = Forja {
        f,
        repo: p.repositorio,
        token,
    };
    let pr = forja.procurar(branch)?;
    let ponta = g.commit_de(&format!("refs/remotes/{remoto}/{branch}"))?;
    let pai = match &ponta {
        None => base_sha.clone(),
        // Sem PR/MR e já mesclada na base: recomeça da base (o push continua
        // sendo um avanço da branch).
        Some(t) if pr.is_none() && g.ancestral(t, &base_sha)? => base_sha.clone(),
        Some(t) => t.clone(),
    };
    let arvore = g.montar_arvore(&pai, p.mudancas, &caminhos)?;

    let resultado = |situacao, commit, link| Publicado {
        situacao,
        branch: branch.to_string(),
        base: base.clone(),
        commit,
        link,
    };

    if arvore == g.arvore_de(&pai)? {
        // Nada a comitar. Com PR/MR aberto, ou sem nada além da base, não há o que
        // fazer; se a branch remota tem conteúdo fora da base e nenhum PR/MR
        // (publicação anterior interrompida), abre o PR/MR.
        if let Some(pr) = pr {
            return Ok(resultado(Situacao::SemMudancas, None, Some(pr.url)));
        }
        if arvore == g.arvore_de(&base_sha)? {
            return Ok(resultado(Situacao::SemMudancas, None, None));
        }
        let novo = forja.abrir(branch, &base, p.titulo.trim(), p.corpo)?;
        return Ok(resultado(Situacao::Aberto, None, Some(novo.url)));
    }

    let commit = g.comitar(&arvore, &pai, p.mensagem)?;
    g.enviar(&remoto, &commit, branch)?;
    Ok(match pr {
        Some(pr) => resultado(Situacao::Atualizado, Some(commit), Some(pr.url)),
        None => {
            let novo = forja.abrir(branch, &base, p.titulo.trim(), p.corpo)?;
            resultado(Situacao::Aberto, Some(commit), Some(novo.url))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branch_de_escrita_recusa_protegidas_e_nomes_invalidos() {
        validar_branch_de_escrita("docs/faturamento-exemplo", Some("main")).unwrap();
        for p in ["main", "Develop", " master ", "release"] {
            assert!(
                matches!(
                    validar_branch_de_escrita(p, Some("release")),
                    Err(ErroEscrita::BranchProtegida(_))
                ),
                "{p}"
            );
        }
        for ruim in [
            "",
            "-x",
            "a..b",
            "a b",
            "a~1",
            "x.lock",
            "refs/heads/x",
            "HEAD",
            "a//b",
            "a/.b",
            "x/",
            "a:b",
            "a@{1}",
        ] {
            assert!(
                matches!(
                    validar_branch_de_escrita(ruim, None),
                    Err(ErroEscrita::Invalido(_))
                ),
                "{ruim:?}"
            );
        }
    }

    #[test]
    fn caminhos_das_mudancas() {
        let ok = validar_mudancas(&[
            Mudanca::gravar("docs\\negocio\\a.md", "x"),
            Mudanca::remover("docs/negocio/b.md"),
        ])
        .unwrap();
        assert_eq!(ok, ["docs/negocio/a.md", "docs/negocio/b.md"]);
        for ruim in ["/a.md", "../a.md", ".git/config", "", "c:/a.md"] {
            assert!(
                validar_mudancas(&[Mudanca::gravar(ruim, "x")]).is_err(),
                "{ruim:?}"
            );
        }
        let e = validar_mudancas(&[Mudanca::gravar("a.md", "x"), Mudanca::remover("a.md")])
            .unwrap_err();
        assert!(e.to_string().contains("mais de uma vez"), "{e}");
    }

    #[test]
    fn query_codificada() {
        assert_eq!(codificar("exemplo:docs/cba"), "exemplo%3Adocs%2Fcba");
        assert_eq!(codificar("a-b_c.d~"), "a-b_c.d~");
    }

    #[test]
    fn falhas_do_push_classificadas_sem_token() {
        let t = Secret::new("ghp-segredo");
        let e = erro_do_push(
            "remote: Permission to exemplo/docs.git denied to x.\nfatal: unable to access \
             'https://ghp-segredo@github.com/': The requested URL returned error: 403",
            "docs/cba",
            Some(&t),
        );
        assert!(matches!(&e, ErroEscrita::SemPermissao(_)), "{e:?}");
        assert!(!e.to_string().contains("ghp-segredo"), "{e}");
        assert!(matches!(
            erro_do_push(
                " ! [rejected] x -> docs/cba (fetch first)",
                "docs/cba",
                None
            ),
            ErroEscrita::Conflito(_)
        ));
        assert!(matches!(
            erro_do_push(
                "remote: GitLab: You are not allowed to push code to this project.",
                "docs/cba",
                None
            ),
            ErroEscrita::SemPermissao(_)
        ));
        assert!(matches!(
            erro_do_push("fatal: algo estranho", "docs/cba", None),
            ErroEscrita::Falhou(_)
        ));
    }

    #[test]
    fn lista_longa_resumida() {
        let itens: Vec<String> = (1..=7).map(|i| format!("{i}.md")).collect();
        assert_eq!(lista(&itens), "1.md, 2.md, 3.md, 4.md, 5.md e mais 2");
    }

    #[cfg(unix)]
    mod com_git {
        //! Repositório git local (bare como remoto) e forja simulada (gh/glab
        //! falsos que guardam os PR/MR numa pasta).
        use super::super::*;
        use crate::demanda::falsas::script;
        use std::process::Command;

        const TOKEN: &str = "ghp-segredo-nao-pode-vazar";
        const BRANCH: &str = "docs/faturamento-exemplo";

        /// git real sem a configuração global do analista (assinatura, autor).
        fn git(dir: &Path, args: &[&str]) -> String {
            let o = Command::new("git")
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .args([
                    "-c",
                    "user.name=Teste",
                    "-c",
                    "user.email=teste@example.com",
                    "-c",
                    "init.defaultBranch=main",
                ])
                .args(args)
                .current_dir(dir)
                .output()
                .unwrap();
            assert!(
                o.status.success(),
                "git {args:?}: {}",
                String::from_utf8_lossy(&o.stderr)
            );
            String::from_utf8_lossy(&o.stdout).trim_end().to_string()
        }

        struct Cenario {
            _tmp: tempfile::TempDir,
            raiz: PathBuf,
            /// Remoto (bare).
            remoto: PathBuf,
            /// Clone do analista.
            clone: PathBuf,
            /// Estado da forja falsa.
            forja: PathBuf,
            repo: RepositorioCodigo,
            f: Ferramentas,
        }

        impl Cenario {
            fn novo(forja: ForgeKind) -> Cenario {
                let tmp = tempfile::tempdir().unwrap();
                let raiz = dunce::canonicalize(tmp.path()).unwrap();
                let semente = raiz.join("semente");
                std::fs::create_dir_all(semente.join("docs/negocio")).unwrap();
                git(&semente, &["init", "-q"]);
                std::fs::write(semente.join("leia-me.md"), "# Faturamento Exemplo\n").unwrap();
                std::fs::write(
                    semente.join("docs/negocio/existente.md"),
                    "# Emissão de nota\n",
                )
                .unwrap();
                git(&semente, &["add", "-A"]);
                git(&semente, &["commit", "-q", "-m", "base"]);
                git(&semente, &["branch", "develop"]);
                let remoto = raiz.join("remoto.git");
                git(&raiz, &["clone", "-q", "--bare", "semente", "remoto.git"]);
                let url = format!("file://{}", remoto.display());
                git(&raiz, &["clone", "-q", &url, "clone"]);
                let clone = raiz.join("clone");
                let estado = raiz.join("forja");
                std::fs::create_dir_all(&estado).unwrap();
                std::fs::write(estado.join("bare"), remoto.display().to_string()).unwrap();
                let bin = raiz.join("bin");
                std::fs::create_dir_all(&bin).unwrap();
                // O git do app, sem a configuração global: o autor padrão entra.
                let git_app = script(
                    &bin,
                    "git",
                    "GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 exec git \"$@\"",
                );
                let (host, caminho) = match forja {
                    ForgeKind::Github => ("github.com", "exemplo/faturamento-docs"),
                    ForgeKind::Gitlab => ("gitlab.example.com", "fin/faturamento-docs"),
                };
                let f = Ferramentas {
                    git: git_app,
                    gh: script(&bin, "gh", &forja_falsa(&estado, "gh")),
                    glab: script(&bin, "glab", &forja_falsa(&estado, "glab")),
                    ..Default::default()
                };
                Cenario {
                    _tmp: tmp,
                    repo: RepositorioCodigo {
                        forja,
                        host: host.into(),
                        caminho: caminho.into(),
                        url_clone: url,
                        pasta: Some(clone.clone()),
                        branch: "main".into(),
                        ..Default::default()
                    },
                    raiz,
                    remoto,
                    clone,
                    forja: estado,
                    f,
                }
            }

            fn pedido<'a>(&'a self, branch: &'a str, mudancas: &'a [Mudanca]) -> Pedido<'a> {
                Pedido {
                    repositorio: &self.repo,
                    pasta: &self.clone,
                    base: Some("main"),
                    branch,
                    mudancas,
                    mensagem: "docs: publica a documentação do Faturamento Exemplo",
                    titulo: "Documentação do Faturamento Exemplo",
                    corpo: "Publicada pelo app.\n\nRegras de emissão e cancelamento.",
                }
            }

            fn publicar(
                &self,
                branch: &str,
                mudancas: &[Mudanca],
            ) -> Result<Publicado, ErroEscrita> {
                publicar(
                    &self.f,
                    Some(&Secret::new(TOKEN)),
                    &self.pedido(branch, mudancas),
                )
            }

            /// `git` no remoto.
            fn no_remoto(&self, args: &[&str]) -> String {
                git(&self.remoto, args)
            }

            fn chamadas(&self) -> String {
                std::fs::read_to_string(self.forja.join("chamadas")).unwrap_or_default()
            }

            fn aberturas(&self) -> usize {
                self.chamadas().matches("--method POST").count()
            }
        }

        /// gh/glab falsos: conferem o token no ambiente (e fora dos argumentos),
        /// guardam as chamadas e o PR/MR aberto. `sem-permissao` faz a API negar;
        /// `mexer` faz um revisor comitar na branch entre o fetch e o push.
        fn forja_falsa(estado: &Path, cli: &str) -> String {
            let (var_token, var_host, host, url, campo, num, rota_lista) = if cli == "gh" {
                (
                    "GH_TOKEN",
                    "GH_HOST",
                    "github.com",
                    "https://github.com/exemplo/faturamento-docs/pull",
                    "html_url",
                    "number",
                    "*pulls\\?*",
                )
            } else {
                (
                    "GITLAB_TOKEN",
                    "GITLAB_HOST",
                    "gitlab.example.com",
                    "https://gitlab.example.com/fin/faturamento-docs/-/merge_requests",
                    "web_url",
                    "iid",
                    "*merge_requests\\?*",
                )
            };
            format!(
                r#"d='{d}'
case "$*" in *{TOKEN}*) echo "token nos argumentos" >&2; exit 9;; esac
[ "${var_token}" = "{TOKEN}" ] || {{ echo "sem token" >&2; exit 8; }}
[ "${var_host}" = "{host}" ] || {{ echo "host errado" >&2; exit 7; }}
echo "$*" >> "$d/chamadas"
if [ -f "$d/sem-permissao" ]; then echo "HTTP 403: Resource not accessible by personal access token (${var_token})" >&2; exit 1; fi
if [ -f "$d/mexer" ]; then
  rm "$d/mexer"
  export GIT_DIR="$(cat "$d/bare")" GIT_CONFIG_GLOBAL=/dev/null GIT_AUTHOR_NAME=Revisor GIT_AUTHOR_EMAIL=r@example.com GIT_COMMITTER_NAME=Revisor GIT_COMMITTER_EMAIL=r@example.com
  ref=refs/heads/{BRANCH}
  c=$(git commit-tree "$(git rev-parse "$ref^{{tree}}")" -p "$ref" -m "revisão")
  git update-ref "$ref" "$c"
  unset GIT_DIR
fi
case "$*" in
  *"--method POST"*)
    n=$(( $(cat "$d/n" 2>/dev/null || echo 0) + 1 )); echo "$n" > "$d/n"; echo "$n" > "$d/aberto"
    echo "{{\"{campo}\":\"{url}/$n\",\"{num}\":$n}}";;
  {rota_lista})
    if [ -f "$d/aberto" ]; then n=$(cat "$d/aberto"); echo "[{{\"{campo}\":\"{url}/$n\",\"{num}\":$n}}]"; else echo '[]'; fi;;
  *) echo "rota inesperada: $*" >&2; exit 3;;
esac"#,
                d = estado.display()
            )
        }

        #[test]
        fn primeira_publicacao_abre_o_pr_sem_mexer_no_clone() {
            let c = Cenario::novo(ForgeKind::Github);
            // O analista tem trabalho seu no clone, fora da publicação.
            std::fs::write(c.clone.join("leia-me.md"), "editado à mão\n").unwrap();
            let base = c.no_remoto(&["rev-parse", "main"]);
            let r = c
                .publicar(
                    BRANCH,
                    &[
                        Mudanca::gravar("docs/negocio/emissao.md", "# Emissão\n"),
                        Mudanca::gravar("docs/negocio/regras/cancelamento.md", "# Cancelar\n"),
                        Mudanca::remover("docs/negocio/existente.md"),
                    ],
                )
                .unwrap();
            assert_eq!(r.situacao, Situacao::Aberto);
            assert_eq!(
                r.link.as_deref(),
                Some("https://github.com/exemplo/faturamento-docs/pull/1")
            );
            assert_eq!((r.branch.as_str(), r.base.as_str()), (BRANCH, "main"));
            let commit = r.commit.unwrap();
            // A branch nova no remoto, a partir da base, com os arquivos.
            assert_eq!(c.no_remoto(&["rev-parse", BRANCH]), commit);
            assert_eq!(c.no_remoto(&["rev-parse", &format!("{commit}^")]), base);
            assert_eq!(
                c.no_remoto(&[
                    "show",
                    &format!("{BRANCH}:docs/negocio/regras/cancelamento.md")
                ]),
                "# Cancelar"
            );
            assert_eq!(
                c.no_remoto(&["ls-tree", "-r", "--name-only", BRANCH]),
                "docs/negocio/emissao.md\ndocs/negocio/regras/cancelamento.md\nleia-me.md"
            );
            // Só o assunto: o autor depende do git global da máquina (o runner do
            // macOS tem `user.name`).
            assert_eq!(
                c.no_remoto(&["log", "-1", "--format=%s", BRANCH]),
                "docs: publica a documentação do Faturamento Exemplo"
            );
            // As protegidas não mudaram.
            assert_eq!(c.no_remoto(&["rev-parse", "main"]), base);
            // O clone ficou como estava: mesma branch, edição do analista, sem os
            // arquivos publicados no checkout, sem branch local e sem o token no
            // config.
            assert_eq!(git(&c.clone, &["symbolic-ref", "--short", "HEAD"]), "main");
            assert_eq!(
                std::fs::read_to_string(c.clone.join("leia-me.md")).unwrap(),
                "editado à mão\n"
            );
            assert!(!c.clone.join("docs/negocio/emissao.md").exists());
            assert!(c.clone.join("docs/negocio/existente.md").exists());
            assert_eq!(git(&c.clone, &["branch", "--list", BRANCH]), "");
            let config = std::fs::read_to_string(c.clone.join(".git/config")).unwrap();
            assert!(
                !config.contains(TOKEN) && !config.contains("CBA_GIT"),
                "{config}"
            );
            // O PR foi procurado e aberto da branch para a base.
            let chamadas = c.chamadas();
            assert!(
                chamadas.contains(
                    "repos/exemplo/faturamento-docs/pulls?state=open&head=exemplo%3Adocs%2Ffaturamento-exemplo"
                ),
                "{chamadas}"
            );
            assert!(chamadas.contains(&format!("head={BRANCH}")), "{chamadas}");
            assert!(chamadas.contains("base=main"), "{chamadas}");
            assert!(
                chamadas.contains("title=Documentação do Faturamento Exemplo"),
                "{chamadas}"
            );
        }

        #[test]
        fn republicar_atualiza_a_branch_e_o_pr_sem_perder_trabalho() {
            let c = Cenario::novo(ForgeKind::Github);
            let r1 = c
                .publicar(
                    BRANCH,
                    &[
                        Mudanca::gravar("docs/negocio/emissao.md", "# Emissão v1\n"),
                        Mudanca::gravar("docs/negocio/glosas.md", "# Glosas\n"),
                    ],
                )
                .unwrap();
            // Um revisor comita na branch do PR.
            git(
                &c.remoto,
                &[
                    "update-ref",
                    &format!("refs/heads/{BRANCH}"),
                    &revisao(&c, BRANCH),
                ],
            );
            let revisor = c.no_remoto(&["rev-parse", BRANCH]);
            let mudancas = [
                Mudanca::gravar("docs/negocio/emissao.md", "# Emissão v2\n"),
                Mudanca::remover("docs/negocio/glosas.md"),
            ];
            let r2 = c.publicar(BRANCH, &mudancas).unwrap();
            assert_eq!(r2.situacao, Situacao::Atualizado);
            assert_eq!(r2.link, r1.link);
            assert_eq!(c.aberturas(), 1, "abriu outro PR");
            let commit = r2.commit.clone().unwrap();
            // Em cima do commit do revisor, que está em cima do primeiro.
            assert_eq!(c.no_remoto(&["rev-parse", &format!("{commit}^")]), revisor);
            assert_eq!(
                c.no_remoto(&["rev-parse", &format!("{revisor}^")]),
                r1.commit.unwrap()
            );
            assert_eq!(
                c.no_remoto(&["show", &format!("{BRANCH}:docs/negocio/emissao.md")]),
                "# Emissão v2"
            );
            assert!(
                !c.no_remoto(&["ls-tree", "-r", "--name-only", BRANCH])
                    .contains("glosas")
            );
            // De novo, igual: sem commit, sem push e sem PR novo, com o link.
            let r3 = c.publicar(BRANCH, &mudancas).unwrap();
            assert_eq!(r3.situacao, Situacao::SemMudancas);
            assert_eq!((r3.commit, r3.link), (None, r1.link));
            assert_eq!(c.no_remoto(&["rev-parse", BRANCH]), commit);
            assert_eq!(c.aberturas(), 1);
        }

        /// Um commit vazio de "Revisor" sobre `referencia`, no remoto.
        fn revisao(c: &Cenario, referencia: &str) -> String {
            let o = Command::new("git")
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_AUTHOR_NAME", "Revisor")
                .env("GIT_AUTHOR_EMAIL", "r@example.com")
                .env("GIT_COMMITTER_NAME", "Revisor")
                .env("GIT_COMMITTER_EMAIL", "r@example.com")
                .args([
                    "commit-tree",
                    &format!("{referencia}^{{tree}}"),
                    "-p",
                    referencia,
                    "-m",
                    "revisão",
                ])
                .current_dir(&c.remoto)
                .output()
                .unwrap();
            assert!(o.status.success());
            String::from_utf8_lossy(&o.stdout).trim().to_string()
        }

        #[test]
        fn branch_mesclada_e_sem_pr_recomeca_da_base() {
            let c = Cenario::novo(ForgeKind::Github);
            c.publicar(BRANCH, &[Mudanca::gravar("docs/negocio/a.md", "a\n")])
                .unwrap();
            // O PR foi mesclado (fast-forward) e fechado.
            c.no_remoto(&["update-ref", "refs/heads/main", BRANCH]);
            std::fs::remove_file(c.forja.join("aberto")).unwrap();
            let r = c
                .publicar(BRANCH, &[Mudanca::gravar("docs/negocio/b.md", "b\n")])
                .unwrap();
            assert_eq!(r.situacao, Situacao::Aberto);
            assert_eq!(
                r.link.as_deref(),
                Some("https://github.com/exemplo/faturamento-docs/pull/2")
            );
            let commit = r.commit.unwrap();
            assert_eq!(
                c.no_remoto(&["rev-parse", &format!("{commit}^")]),
                c.no_remoto(&["rev-parse", "main"])
            );
        }

        #[test]
        fn branch_sem_pr_fora_da_base_ganha_o_pr_sem_commit_novo() {
            // Publicação anterior interrompida depois do push: a próxima, igual,
            // só abre o PR.
            let c = Cenario::novo(ForgeKind::Github);
            let m = [Mudanca::gravar("docs/negocio/a.md", "a\n")];
            let r1 = c.publicar(BRANCH, &m).unwrap();
            std::fs::remove_file(c.forja.join("aberto")).unwrap();
            let r2 = c.publicar(BRANCH, &m).unwrap();
            assert_eq!((r2.situacao, r2.commit), (Situacao::Aberto, None));
            assert_eq!(c.no_remoto(&["rev-parse", BRANCH]), r1.commit.unwrap());
            assert_eq!(c.aberturas(), 2);
        }

        #[test]
        fn sem_mudancas_nao_comita_nem_abre_pr() {
            let c = Cenario::novo(ForgeKind::Github);
            let r = c
                .publicar(
                    BRANCH,
                    &[
                        Mudanca::gravar("docs/negocio/existente.md", "# Emissão de nota\n"),
                        Mudanca::remover("docs/negocio/nao-existe.md"),
                    ],
                )
                .unwrap();
            assert_eq!(r.situacao, Situacao::SemMudancas);
            assert_eq!((r.commit, r.link), (None, None));
            assert_eq!(c.aberturas(), 0);
            assert_eq!(c.no_remoto(&["branch", "--list", BRANCH]), "");
        }

        #[test]
        fn branch_protegida_recusada_sem_tocar_no_remoto() {
            let c = Cenario::novo(ForgeKind::Github);
            let antes = c.no_remoto(&["for-each-ref"]);
            for b in ["main", "develop", "Master"] {
                let e = c
                    .publicar(b, &[Mudanca::gravar("docs/negocio/a.md", "a\n")])
                    .unwrap_err();
                assert!(matches!(e, ErroEscrita::BranchProtegida(_)), "{b}: {e}");
            }
            // Sem branch base no pedido, a padrão do remoto também é protegida.
            git(&c.remoto, &["branch", "publicacao"]);
            git(&c.clone, &["fetch", "-q"]);
            git(&c.clone, &["remote", "set-head", "origin", "publicacao"]);
            let m = [Mudanca::gravar("docs/negocio/a.md", "a\n")];
            let e = publicar(
                &c.f,
                None,
                &Pedido {
                    base: None,
                    ..c.pedido("publicacao", &m)
                },
            )
            .unwrap_err();
            assert_eq!(e, ErroEscrita::BranchProtegida("publicacao".into()));
            git(&c.remoto, &["branch", "-D", "publicacao"]);
            assert_eq!(c.no_remoto(&["for-each-ref"]), antes);
            assert_eq!(c.aberturas(), 0);
        }

        #[test]
        fn clone_sujo_nos_arquivos_publicados_e_recusado() {
            let c = Cenario::novo(ForgeKind::Github);
            std::fs::write(c.clone.join("docs/negocio/existente.md"), "mexido\n").unwrap();
            std::fs::write(c.clone.join("docs/negocio/nova.md"), "à mão\n").unwrap();
            let e = c
                .publicar(
                    BRANCH,
                    &[
                        Mudanca::gravar("docs/negocio/existente.md", "x\n"),
                        Mudanca::gravar("docs/negocio/nova.md", "y\n"),
                        Mudanca::gravar("docs/negocio/outra.md", "z\n"),
                    ],
                )
                .unwrap_err();
            match &e {
                ErroEscrita::CloneSujo { arquivos, .. } => assert_eq!(
                    arquivos,
                    &["docs/negocio/existente.md", "docs/negocio/nova.md"]
                ),
                e => panic!("{e:?}"),
            }
            assert!(e.to_string().contains("não comitadas"), "{e}");
            assert_eq!(c.chamadas(), "");
            // Sujeira fora da publicação não impede.
            c.publicar(BRANCH, &[Mudanca::gravar("docs/negocio/outra.md", "z\n")])
                .unwrap();
        }

        #[test]
        fn sem_permissao_na_api_e_no_push() {
            // A API nega antes do push: nada vai para o remoto, e o token não vaza.
            let c = Cenario::novo(ForgeKind::Github);
            std::fs::write(c.forja.join("sem-permissao"), "").unwrap();
            let e = c
                .publicar(BRANCH, &[Mudanca::gravar("docs/negocio/a.md", "a\n")])
                .unwrap_err();
            assert!(matches!(e, ErroEscrita::SemPermissao(_)), "{e:?}");
            let m = e.to_string();
            assert!(!m.contains(TOKEN) && m.contains("403"), "{m}");
            assert_eq!(c.no_remoto(&["branch", "--list", BRANCH]), "");

            // O remoto recusa o push (GitLab).
            let c = Cenario::novo(ForgeKind::Gitlab);
            script(
                &c.remoto.join("hooks"),
                "pre-receive",
                "echo 'GitLab: You are not allowed to push code to this project.' >&2; exit 1",
            );
            let e = c
                .publicar(BRANCH, &[Mudanca::gravar("docs/negocio/a.md", "a\n")])
                .unwrap_err();
            assert!(matches!(e, ErroEscrita::SemPermissao(_)), "{e:?}");
            assert!(e.to_string().contains("escrita"), "{e}");
            assert_eq!(c.aberturas(), 0);
        }

        #[test]
        fn gitlab_abre_o_mr_com_o_glab() {
            let c = Cenario::novo(ForgeKind::Gitlab);
            let r = c
                .publicar(BRANCH, &[Mudanca::gravar("docs/negocio/a.md", "a\n")])
                .unwrap();
            assert_eq!(r.situacao, Situacao::Aberto);
            assert_eq!(
                r.link.as_deref(),
                Some("https://gitlab.example.com/fin/faturamento-docs/-/merge_requests/1")
            );
            let chamadas = c.chamadas();
            assert!(
                chamadas.contains(
                    "projects/fin%2Ffaturamento-docs/merge_requests?state=opened&source_branch=docs%2Ffaturamento-exemplo"
                ),
                "{chamadas}"
            );
            assert!(
                chamadas.contains(&format!("source_branch={BRANCH}")),
                "{chamadas}"
            );
            assert!(chamadas.contains("target_branch=main"), "{chamadas}");
            assert!(
                chamadas.contains("description=Publicada pelo app."),
                "{chamadas}"
            );
        }

        #[test]
        fn cli_ausente_falha_antes_do_push() {
            let mut c = Cenario::novo(ForgeKind::Github);
            c.f.gh = c.raiz.join("nao-existe").join("gh");
            let e = c
                .publicar(BRANCH, &[Mudanca::gravar("docs/negocio/a.md", "a\n")])
                .unwrap_err();
            assert_eq!(e, ErroEscrita::SemFerramenta("gh"));
            assert!(e.to_string().contains("gh não encontrado"), "{e}");
            assert_eq!(c.no_remoto(&["branch", "--list", BRANCH]), "");
        }

        #[test]
        fn branch_que_anda_no_meio_da_publicacao_e_conflito() {
            let c = Cenario::novo(ForgeKind::Github);
            c.publicar(BRANCH, &[Mudanca::gravar("docs/negocio/a.md", "a\n")])
                .unwrap();
            // Um revisor comita na branch depois do fetch e antes do push (na
            // consulta do PR, na forja falsa).
            std::fs::write(c.forja.join("mexer"), "").unwrap();
            let e = c
                .publicar(BRANCH, &[Mudanca::gravar("docs/negocio/b.md", "b\n")])
                .unwrap_err();
            assert!(matches!(e, ErroEscrita::Conflito(_)), "{e:?}");
            assert!(e.to_string().contains("publique de novo"), "{e}");
            // O commit do revisor continua lá; publicar de novo vai em cima dele.
            assert_eq!(
                c.no_remoto(&["log", "-1", "--format=%an", BRANCH]),
                "Revisor"
            );
            let r = c
                .publicar(BRANCH, &[Mudanca::gravar("docs/negocio/b.md", "b\n")])
                .unwrap();
            assert_eq!(r.situacao, Situacao::Atualizado);
            assert_eq!(
                c.no_remoto(&["log", "-1", "--format=%an", &format!("{BRANCH}^")]),
                "Revisor"
            );
        }

        #[test]
        fn pasta_que_nao_e_clone_do_repositorio() {
            let c = Cenario::novo(ForgeKind::Github);
            let outro = RepositorioCodigo {
                caminho: "exemplo/outro".into(),
                url_clone: "https://github.com/exemplo/outro.git".into(),
                ..c.repo.clone()
            };
            let e = publicar(
                &c.f,
                None,
                &Pedido {
                    repositorio: &outro,
                    ..c.pedido(BRANCH, &[])
                },
            )
            .unwrap_err();
            assert!(e.to_string().contains("não tem um remoto"), "{e}");
        }

        /// Um commit novo na `main` do remoto, por outro clone.
        fn avancar_remoto(c: &Cenario, arquivo: &str) -> String {
            let outro = c.raiz.join(format!("outro-{arquivo}"));
            git(
                &c.raiz,
                &[
                    "clone",
                    "-q",
                    &format!("file://{}", c.remoto.display()),
                    outro.to_str().unwrap(),
                ],
            );
            std::fs::write(outro.join(arquivo), "novo\n").unwrap();
            git(&outro, &["add", "-A"]);
            git(&outro, &["commit", "-q", "-m", "avança"]);
            git(&outro, &["push", "-q", "origin", "main"]);
            git(&outro, &["rev-parse", "HEAD"])
        }

        #[test]
        fn atualizar_clone_avanca_a_base_so_por_fast_forward() {
            let c = Cenario::novo(ForgeKind::Github);
            let novo = avancar_remoto(&c, "novo.md");
            let token = Secret::new(TOKEN);
            let a = atualizar_clone(&c.f.git, &c.repo, &c.clone, None, Some(&token)).unwrap();
            assert_eq!(
                a,
                Atualizacao {
                    remoto: "origin".into(),
                    base: "main".into(),
                    commit: novo.clone(),
                    avancou: true,
                    pasta_na_base: true,
                }
            );
            assert!(c.clone.join("novo.md").exists());
            // De novo: em dia.
            let a = atualizar_clone(&c.f.git, &c.repo, &c.clone, Some("main"), None).unwrap();
            assert!(!a.avancou);

            // Arquivo rastreado alterado com a base no checkout: clone sujo.
            let novo2 = avancar_remoto(&c, "novo2.md");
            std::fs::write(c.clone.join("leia-me.md"), "mexido\n").unwrap();
            let e = atualizar_clone(&c.f.git, &c.repo, &c.clone, None, None).unwrap_err();
            match e {
                ErroEscrita::CloneSujo { arquivos, .. } => assert_eq!(arquivos, ["leia-me.md"]),
                e => panic!("{e:?}"),
            }
            git(&c.clone, &["checkout", "-q", "--", "leia-me.md"]);

            // Fora do checkout: só a referência anda; a pasta fica na outra branch.
            git(&c.clone, &["checkout", "-q", "-b", "trabalho"]);
            let a = atualizar_clone(&c.f.git, &c.repo, &c.clone, None, None).unwrap();
            assert!(a.avancou && !a.pasta_na_base);
            assert_eq!(git(&c.clone, &["rev-parse", "main"]), novo2);
            assert!(!c.clone.join("novo2.md").exists());

            // Divergiu: conflito, sem mexer em nada.
            git(&c.clone, &["checkout", "-q", "main"]);
            std::fs::write(c.clone.join("local.md"), "local\n").unwrap();
            git(&c.clone, &["add", "-A"]);
            git(&c.clone, &["commit", "-q", "-m", "local"]);
            let local = git(&c.clone, &["rev-parse", "HEAD"]);
            avancar_remoto(&c, "novo3.md");
            let e = atualizar_clone(&c.f.git, &c.repo, &c.clone, None, None).unwrap_err();
            assert!(matches!(e, ErroEscrita::Conflito(_)), "{e:?}");
            assert_eq!(git(&c.clone, &["rev-parse", "HEAD"]), local);
        }

        #[test]
        fn ler_remoto_traz_a_pasta_da_base_e_da_branch_sem_mexer_no_clone() {
            let c = Cenario::novo(ForgeKind::Github);
            c.publicar(
                BRANCH,
                &[Mudanca::gravar("docs/negocio/novo.md", "# Novo\n")],
            )
            .unwrap();
            let head = git(&c.clone, &["rev-parse", "HEAD"]);
            let ler = ["docs/negocio/existente.md", "docs/negocio/novo.md"];
            let l = ler_remoto(
                &c.f.git,
                None,
                &Leitura {
                    repositorio: &c.repo,
                    pasta: &c.clone,
                    base: None,
                    branch: BRANCH,
                    pasta_no_repositorio: "docs/negocio",
                    ler: &ler,
                },
            )
            .unwrap();
            assert_eq!(l.base, "main");
            assert_eq!(l.na_base.commit, c.no_remoto(&["rev-parse", "main"]));
            assert_eq!(
                l.na_base.arquivos.keys().collect::<Vec<_>>(),
                ["docs/negocio/existente.md"]
            );
            assert_eq!(
                l.na_base.conteudos["docs/negocio/existente.md"],
                b"# Emiss\xc3\xa3o de nota\n"
            );
            // O id do blob calculado bate com o do git.
            assert_eq!(
                l.na_base.arquivos["docs/negocio/existente.md"],
                l.formato.id_do_blob("# Emissão de nota\n".as_bytes())
            );
            let b = l.na_branch.unwrap();
            assert_eq!(b.commit, c.no_remoto(&["rev-parse", BRANCH]));
            assert_eq!(b.arquivos.len(), 2);
            assert_eq!(b.conteudos["docs/negocio/novo.md"], b"# Novo\n");
            // Sem a branch no remoto, só a base.
            let l = ler_remoto(
                &c.f.git,
                None,
                &Leitura {
                    repositorio: &c.repo,
                    pasta: &c.clone,
                    base: Some("main"),
                    branch: "docs/outra",
                    pasta_no_repositorio: "",
                    ler: &[],
                },
            )
            .unwrap();
            assert!(l.na_branch.is_none());
            assert_eq!(l.na_base.arquivos.len(), 2);
            assert!(l.na_base.conteudos.is_empty());
            assert_eq!(git(&c.clone, &["rev-parse", "HEAD"]), head);
        }
    }
}
