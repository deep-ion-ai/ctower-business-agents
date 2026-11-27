//! `historico_do_trecho` (#450): os commits que criaram e mudaram um trecho do
//! código, com as chaves de issue das mensagens, para a análise de impacto achar
//! a demanda que pediu a regra de propósito.
//!
//! Lê o histórico com o `git` instalado (o mesmo que o analista usa para clonar o
//! projeto), offline, em dois modos:
//!
//! - **trecho:** `arquivo` com `linha_inicial` e `linha_final` (`git log -L`): os
//!   commits que tocaram essas linhas, até a criação delas;
//! - **texto:** um identificador ou um texto curto (`git log -S`), no `arquivo`
//!   informado ou, sem ele, em cada pasta do projeto: os commits que puseram ou
//!   tiraram o texto.
//!
//! O commit mais antigo é a origem. A saída é limitada ([`LIMITE_PADRAO`]): a
//! origem e os mais recentes, com aviso quando corta. De cada commit saem o hash,
//! a data, a mensagem e as chaves de issue tiradas dela pelos padrões do projeto
//! ([`cba_index::Projeto::padroes_de_chave_compilados`]); nunca uma chave que a
//! mensagem não traz. O autor do commit não sai daqui: a análise não fala de
//! pessoas, e o autor só fica no estado quando o agente cita o commit como origem
//! (`afirmar`, campo `origens`, [`resolver_commit`]). Com rastreador cadastrado,
//! o título das issues citadas vem do rastreador ([`cba_repos::demanda::ler_issue`],
//! #351), para mostrar a intenção da demanda e não só a chave.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use cba_core::ambiente::Ambiente;
use cba_core::analise::EvidenciaOrigem;
use cba_repos::demanda::{Ferramentas, LimitesIssue, ler_issue};
use cba_repos::estado::chave_normalizada;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::registro::{ErroFerramenta, Ferramenta, Saida, ler_argumentos};

/// Commits mostrados quando o agente não informa o limite.
pub const LIMITE_PADRAO: usize = 10;
/// Maior limite aceito.
pub const LIMITE_MAXIMO: usize = 50;
/// Títulos de issue lidos no rastreador numa chamada, no máximo.
const MAXIMO_DE_TITULOS: usize = 5;
/// Caracteres do corpo da mensagem de cada commit; o resto é cortado.
const TAMANHO_DA_MENSAGEM: usize = 1500;
/// Tamanho do texto procurado no modo texto.
const TEXTO_MINIMO: usize = 3;
const TEXTO_MAXIMO: usize = 200;

const SEPARADOR_DE_COMMIT: char = '\u{1e}';
const SEPARADOR_DE_CAMPO: char = '\u{1f}';
/// Hash, instante (para ordenar), data curta, assunto e corpo; o `%x1f` final
/// separa o diff que o `git log -L` imprime depois.
const FORMATO: &str = "--format=%x1e%H%x1f%ct%x1f%ad%x1f%s%x1f%b%x1f";

#[derive(Debug, Deserialize)]
struct Args {
    #[serde(default)]
    arquivo: Option<String>,
    #[serde(default)]
    linha_inicial: Option<u32>,
    #[serde(default)]
    linha_final: Option<u32>,
    #[serde(default)]
    texto: Option<String>,
    #[serde(default)]
    limite: Option<usize>,
    #[serde(default)]
    titulos: Option<bool>,
}

fn schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "arquivo": {
                "type": "string",
                "minLength": 1,
                "description": "Arquivo relativo à raiz do projeto, com / (ex.: db/03_pkg_cobranca.sql). Com linha_inicial e linha_final, o histórico dessas linhas; com texto, só os commits deste arquivo."
            },
            "linha_inicial": {
                "type": "integer",
                "minimum": 1,
                "description": "Primeira linha do trecho (a mesma da evidência)."
            },
            "linha_final": {
                "type": "integer",
                "minimum": 1,
                "description": "Última linha do trecho (inclusiva)."
            },
            "texto": {
                "type": "string",
                "minLength": TEXTO_MINIMO,
                "maxLength": TEXTO_MAXIMO,
                "description": "Identificador ou texto curto do código (nome de constante, de rotina, um valor fixo), numa linha: os commits que o puseram ou o tiraram. Use no lugar das linhas, quando o trecho mudou de lugar ou para achar quem criou o identificador."
            },
            "limite": {
                "type": "integer",
                "minimum": 2,
                "maximum": LIMITE_MAXIMO,
                "description": "Commits mostrados no máximo (padrão 10): o mais antigo, que é a origem, e os mais recentes."
            },
            "titulos": {
                "type": "boolean",
                "description": "Com rastreador cadastrado, lê o título das issues citadas (padrão true)."
            }
        },
        "additionalProperties": false
    })
}

/// Um commit do histórico, sem o autor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommitDoTrecho {
    pub hash: String,
    /// `AAAA-MM-DD` (data de autoria).
    pub data: String,
    /// A primeira linha da mensagem.
    pub assunto: String,
    /// O resto da mensagem, cortado em [`TAMANHO_DA_MENSAGEM`] caracteres.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub corpo: String,
    /// Chaves de issue achadas na mensagem pelos padrões do projeto.
    pub chaves: Vec<String>,
    /// É o commit mais antigo do histórico (a origem do trecho).
    pub origem: bool,
    #[serde(skip)]
    instante: i64,
}

impl CommitDoTrecho {
    fn curto(&self) -> &str {
        self.hash.get(..10).unwrap_or(&self.hash)
    }
}

/// O que foi pedido.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pedido {
    Trecho {
        arquivo: String,
        linha_inicial: u32,
        linha_final: u32,
    },
    Texto {
        texto: String,
        arquivo: Option<String>,
    },
}

impl Pedido {
    fn descricao(&self) -> String {
        match self {
            Pedido::Trecho {
                arquivo,
                linha_inicial,
                linha_final,
            } => format!("`{arquivo}`, linhas {linha_inicial} a {linha_final}"),
            Pedido::Texto {
                texto,
                arquivo: Some(a),
            } => format!("\"{texto}\" em `{a}`"),
            Pedido::Texto {
                texto,
                arquivo: None,
            } => format!("\"{texto}\" no projeto"),
        }
    }
}

/// O histórico, limitado.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Historico {
    /// Commits mostrados, do mais recente ao mais antigo (a origem por último).
    pub commits: Vec<CommitDoTrecho>,
    /// Commits achados, antes do corte.
    pub total: usize,
    /// Commits que ficaram de fora (entre a origem e os mais recentes).
    pub cortados: usize,
    /// Título de cada chave lida no rastreador.
    pub titulos: BTreeMap<String, String>,
    pub avisos: Vec<String>,
}

// ---------------------------------------------------------------------------
// Git

/// Resultado de um `git`.
struct SaidaGit {
    ok: bool,
    stdout: String,
    stderr: String,
}

/// Executa `git -C <dir> <args>` sem mexer na pasta do analista: sem travas
/// opcionais, saída em inglês, sem assinatura no log e sem janela de console no
/// Windows.
fn git(dir: &Path, args: &[&str]) -> Result<SaidaGit, String> {
    let mut c = Command::new("git");
    c.arg("-C")
        .arg(dir)
        .args(["-c", "log.showSignature=false", "-c", "core.quotepath=off"])
        .args(args)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000);
    }
    let saida = c.output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            "o git não foi encontrado; instale-o para ler o histórico do código".to_owned()
        } else {
            format!("o git não pôde ser executado ({:?})", e.kind())
        }
    })?;
    Ok(SaidaGit {
        ok: saida.status.success(),
        stdout: String::from_utf8_lossy(&saida.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&saida.stderr).into_owned(),
    })
}

/// A primeira linha do erro do git, sem o prefixo `fatal:`.
fn motivo(s: &SaidaGit) -> String {
    let linha = s
        .stderr
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("o git falhou");
    linha
        .trim_start_matches("fatal:")
        .trim_start_matches("error:")
        .trim()
        .to_owned()
}

/// A pasta está num repositório git?
fn em_repositorio(dir: &Path) -> Result<bool, String> {
    Ok(git(dir, &["rev-parse", "--is-inside-work-tree"])?.ok)
}

/// O clone é raso (`--depth`): o commit mais antigo pode não ser a origem.
fn raso(dir: &Path) -> bool {
    git(dir, &["rev-parse", "--is-shallow-repository"])
        .map(|s| s.ok && s.stdout.trim() == "true")
        .unwrap_or(false)
}

/// Corta um texto em `max` caracteres, com reticências.
fn cortar(texto: &str, max: usize) -> String {
    if texto.chars().count() <= max {
        return texto.to_owned();
    }
    let mut t: String = texto.chars().take(max).collect();
    t.push('…');
    t
}

/// Lê a saída de `git log` no [`FORMATO`]. O que vem depois do último campo (o
/// diff do `-L`) é ignorado.
fn ler_commits(saida: &str, padroes: &[regex::Regex]) -> Vec<CommitDoTrecho> {
    saida
        .split(SEPARADOR_DE_COMMIT)
        .filter_map(|bloco| {
            let mut campos = bloco.splitn(6, SEPARADOR_DE_CAMPO);
            let hash = campos.next()?.trim();
            if hash.len() < 7 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
                return None;
            }
            let instante = campos.next()?.trim().parse().unwrap_or(0);
            let data = campos.next()?.trim().to_owned();
            let assunto = campos.next()?.trim().to_owned();
            let corpo = campos.next()?.trim().to_owned();
            // Sem o sexto campo (o separador final), o bloco veio truncado.
            campos.next()?;
            let chaves =
                cba_index::Projeto::chaves_de_issue(padroes, &format!("{assunto}\n{corpo}"));
            Some(CommitDoTrecho {
                hash: hash.to_owned(),
                data,
                assunto,
                corpo: cortar(&corpo, TAMANHO_DA_MENSAGEM),
                chaves,
                origem: false,
                instante,
            })
        })
        .collect()
}

/// A pasta e o nome do arquivo, conferido pelo escopo do projeto (exclusões
/// valem).
fn localizar(ambiente: &Ambiente, arquivo: &str) -> Result<(PathBuf, String), ErroFerramenta> {
    let absoluto = ambiente
        .escopo
        .resolver(arquivo)
        .map_err(|e| ErroFerramenta::Recusa(format!("`{arquivo}`: {e}")))?;
    let pasta = absoluto
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| ErroFerramenta::Recusa(format!("`{arquivo}`: não é um arquivo")))?;
    let nome = absoluto
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .ok_or_else(|| ErroFerramenta::Recusa(format!("`{arquivo}`: não é um arquivo")))?;
    Ok((pasta, nome))
}

fn sem_repositorio(arquivo: &str) -> ErroFerramenta {
    ErroFerramenta::Recusa(format!(
        "`{arquivo}` não está num repositório git: não há histórico para ler. Siga a \
         análise sem a origem do trecho e diga isso ao analista"
    ))
}

fn falha_do_git(e: String) -> ErroFerramenta {
    ErroFerramenta::Recusa(format!("{e}. Siga a análise sem a origem do trecho"))
}

/// Os commits do arquivo (pasta e nome) que o pedido alcança, com os avisos do
/// arquivo.
fn commits_do_arquivo(
    pasta: &Path,
    nome: &str,
    arquivo: &str,
    log: &[String],
    padroes: &[regex::Regex],
    avisos: &mut Vec<String>,
) -> Result<Vec<CommitDoTrecho>, ErroFerramenta> {
    if !em_repositorio(pasta).map_err(falha_do_git)? {
        return Err(sem_repositorio(arquivo));
    }
    let versionado =
        git(pasta, &["ls-files", "--error-unmatch", "--", nome]).map_err(falha_do_git)?;
    if !versionado.ok {
        return Err(ErroFerramenta::Recusa(format!(
            "`{arquivo}` ainda não tem commit no repositório: não há histórico para ler"
        )));
    }
    let alterado = git(pasta, &["diff", "--quiet", "HEAD", "--", nome]).map_err(falha_do_git)?;
    if !alterado.ok {
        avisos.push(format!(
            "`{arquivo}` tem alterações ainda sem commit: o histórico usa as linhas do último \
             commit, que podem não ser as da cópia de trabalho"
        ));
    }
    if raso(pasta) {
        avisos.push(
            "o clone do repositório é raso (sem o histórico completo): o commit mais antigo \
             pode não ser a origem do trecho"
                .into(),
        );
    }
    let mut args: Vec<&str> = vec!["log", "--no-color", "--date=short", FORMATO];
    args.extend(log.iter().map(String::as_str));
    let saida = git(pasta, &args).map_err(falha_do_git)?;
    if !saida.ok {
        return Err(ErroFerramenta::Recusa(format!(
            "o histórico de `{arquivo}` não pôde ser lido: {}. Confira as linhas e chame de novo",
            motivo(&saida)
        )));
    }
    Ok(ler_commits(&saida.stdout, padroes))
}

/// Lê o histórico do pedido, sem limite.
fn ler_tudo(
    ambiente: &Ambiente,
    pedido: &Pedido,
    padroes: &[regex::Regex],
    avisos: &mut Vec<String>,
) -> Result<Vec<CommitDoTrecho>, ErroFerramenta> {
    match pedido {
        Pedido::Trecho {
            arquivo,
            linha_inicial,
            linha_final,
        } => {
            let (pasta, nome) = localizar(ambiente, arquivo)?;
            let intervalo = format!("-L{linha_inicial},{linha_final}:{nome}");
            commits_do_arquivo(&pasta, &nome, arquivo, &[intervalo], padroes, avisos)
        }
        Pedido::Texto {
            texto,
            arquivo: Some(arquivo),
        } => {
            let (pasta, nome) = localizar(ambiente, arquivo)?;
            let log = vec![format!("-S{texto}"), "--".to_owned(), nome.clone()];
            commits_do_arquivo(&pasta, &nome, arquivo, &log, padroes, avisos)
        }
        Pedido::Texto {
            texto,
            arquivo: None,
        } => {
            let mut todos: Vec<CommitDoTrecho> = Vec::new();
            let mut lidas = 0;
            for p in &ambiente.projeto.pastas {
                if !em_repositorio(&p.caminho).map_err(falha_do_git)? {
                    continue;
                }
                lidas += 1;
                if raso(&p.caminho) {
                    avisos.push(format!(
                        "o clone de {} é raso (sem o histórico completo): o commit mais antigo \
                         pode não ser a origem",
                        p.prefixo.as_deref().unwrap_or("raiz do projeto")
                    ));
                }
                let busca = format!("-S{texto}");
                let saida = git(
                    &p.caminho,
                    &[
                        "log",
                        "--no-color",
                        "--date=short",
                        FORMATO,
                        &busca,
                        "--",
                        ".",
                    ],
                )
                .map_err(falha_do_git)?;
                if !saida.ok {
                    avisos.push(format!(
                        "o histórico de {} não pôde ser lido: {}",
                        p.prefixo.as_deref().unwrap_or("raiz do projeto"),
                        motivo(&saida)
                    ));
                    continue;
                }
                for c in ler_commits(&saida.stdout, padroes) {
                    if !todos.iter().any(|t| t.hash == c.hash) {
                        todos.push(c);
                    }
                }
            }
            if lidas == 0 {
                return Err(ErroFerramenta::Recusa(
                    "nenhuma pasta do projeto está num repositório git: não há histórico para \
                     ler. Siga a análise sem a origem do trecho e diga isso ao analista"
                        .into(),
                ));
            }
            // Do mais recente ao mais antigo, como o git log de uma pasta só.
            todos.sort_by_key(|c| std::cmp::Reverse(c.instante));
            Ok(todos)
        }
    }
}

/// Fica com a origem (o mais antigo) e os `limite - 1` mais recentes.
fn limitar(mut todos: Vec<CommitDoTrecho>, limite: usize) -> (Vec<CommitDoTrecho>, usize) {
    if let Some(ultimo) = todos.last_mut() {
        ultimo.origem = true;
    }
    let limite = limite.max(2);
    if todos.len() <= limite {
        return (todos, 0);
    }
    let cortados = todos.len() - limite;
    let origem = todos.pop().expect("lista com mais de dois commits");
    todos.truncate(limite - 1);
    todos.push(origem);
    (todos, cortados)
}

/// Lê no rastreador o título das chaves citadas, a origem primeiro.
fn ler_titulos(ambiente: &Ambiente, h: &mut Historico, f: &Ferramentas) {
    let Some(tracker) = ambiente.projeto.repositorios.tracker.as_ref() else {
        return;
    };
    let mut ordem: Vec<&CommitDoTrecho> = h.commits.iter().filter(|c| c.origem).collect();
    ordem.extend(h.commits.iter().filter(|c| !c.origem));
    let mut pedidas: Vec<(String, String)> = Vec::new();
    for chave in ordem.iter().flat_map(|c| &c.chaves) {
        let Some(normalizada) = chave_normalizada(tracker, chave) else {
            continue;
        };
        if pedidas.iter().any(|(c, _)| c == chave) {
            continue;
        }
        if pedidas.len() == MAXIMO_DE_TITULOS {
            h.avisos.push(format!(
                "só os títulos das {MAXIMO_DE_TITULOS} primeiras issues citadas foram lidos no \
                 rastreador"
            ));
            break;
        }
        pedidas.push((chave.clone(), normalizada));
    }
    let limites = LimitesIssue {
        comentarios: 1,
        caracteres_por_comentario: 1,
        caracteres_dos_comentarios: 1,
        caracteres_da_descricao: 1,
    };
    for (chave, normalizada) in pedidas {
        match ler_issue(tracker, &normalizada, &limites, f) {
            Ok(issue) if !issue.titulo.trim().is_empty() => {
                h.titulos.insert(chave, issue.titulo.trim().to_owned());
            }
            Ok(_) => {}
            Err(e) => {
                h.avisos.push(format!(
                    "os títulos das issues não foram lidos no rastreador ({e}); cite a chave e \
                     pergunte ao analista qual era a intenção da demanda"
                ));
                break;
            }
        }
    }
}

/// O histórico do pedido, limitado, com os títulos do rastreador quando `titulos`.
pub fn historico(
    ambiente: &Ambiente,
    pedido: &Pedido,
    limite: usize,
    titulos: bool,
    f: &Ferramentas,
) -> Result<Historico, ErroFerramenta> {
    let padroes = ambiente
        .projeto
        .padroes_de_chave_compilados()
        .map_err(|e| ErroFerramenta::Recusa(format!("cadastro do projeto: {e}")))?;
    let mut avisos = Vec::new();
    let todos = ler_tudo(ambiente, pedido, &padroes, &mut avisos)?;
    let total = todos.len();
    let (commits, cortados) = limitar(todos, limite.clamp(2, LIMITE_MAXIMO));
    if cortados > 0 {
        avisos.insert(
            0,
            format!(
                "o histórico tem {total} commits: ficaram de fora os {cortados} entre a origem e \
                 os {} mais recentes. Para ver mais, chame de novo com um `limite` maior (até \
                 {LIMITE_MAXIMO})",
                commits.len() - 1
            ),
        );
    }
    let mut h = Historico {
        commits,
        total,
        cortados,
        titulos: BTreeMap::new(),
        avisos,
    };
    if titulos {
        ler_titulos(ambiente, &mut h, f);
    }
    Ok(h)
}

fn chaves_com_titulo(c: &CommitDoTrecho, titulos: &BTreeMap<String, String>) -> String {
    c.chaves
        .iter()
        .map(|k| match titulos.get(k) {
            Some(t) => format!("{k} (\"{t}\")"),
            None => k.clone(),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn linha_do_commit(t: &mut String, c: &CommitDoTrecho, titulos: &BTreeMap<String, String>) {
    let _ = writeln!(t, "- `{}` em {}: {}", c.curto(), c.data, c.assunto);
    if c.chaves.is_empty() {
        let _ = writeln!(t, "  - Chaves de issue: nenhuma na mensagem");
    } else {
        let _ = writeln!(t, "  - Chaves de issue: {}", chaves_com_titulo(c, titulos));
    }
    if !c.corpo.is_empty() {
        let corpo = c.corpo.lines().collect::<Vec<_>>().join(" ");
        let _ = writeln!(t, "  - Mensagem: {corpo}");
    }
}

/// O histórico em Markdown, para o agente ler.
fn markdown(pedido: &Pedido, h: &Historico) -> String {
    let mut t = String::new();
    let _ = writeln!(t, "# Histórico de {}\n", pedido.descricao());
    if h.commits.is_empty() {
        let _ = writeln!(
            t,
            "Nenhum commit alcança o pedido. Sem histórico, não há origem a citar: siga a \
             análise."
        );
    } else {
        let _ = writeln!(
            t,
            "{} commit(s) tocaram o trecho; o mais antigo é a origem.\n",
            h.total
        );
        let origem = h.commits.iter().find(|c| c.origem);
        if let Some(o) = origem {
            let _ = writeln!(t, "## Origem\n");
            linha_do_commit(&mut t, o, &h.titulos);
            let _ = writeln!(t);
        }
        let recentes: Vec<&CommitDoTrecho> = h.commits.iter().filter(|c| !c.origem).collect();
        if !recentes.is_empty() {
            let _ = writeln!(t, "## Mais recentes\n");
            for c in recentes {
                linha_do_commit(&mut t, c, &h.titulos);
            }
            let _ = writeln!(t);
        }
        let _ = writeln!(
            t,
            "Se a origem, ou um commit que mudou a regra, cita uma demanda que pediu esse \
             comportamento de propósito, registre isso em `como_e_hoje` (com a chave e a \
             intenção da demanda) e pergunte com `perguntar` se a regra da demanda continua \
             valendo, citando a chave. Para guardar o commit como evidência complementar, \
             passe o hash em `origens` do `afirmar`; a evidência da regra continua sendo o \
             trecho de código. Não cite nomes de pessoas. Commit sem chave é só ajuste: não \
             invente a demanda."
        );
    }
    for a in &h.avisos {
        let _ = writeln!(t, "\nAviso: {a}");
    }
    t.trim_end().to_string()
}

fn pedido_dos_argumentos(args: &Args) -> Result<Pedido, ErroFerramenta> {
    let arquivo = args
        .arquivo
        .as_deref()
        .map(|a| a.trim().trim_start_matches("./").replace('\\', "/"))
        .filter(|a| !a.is_empty());
    let texto = args
        .texto
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty());
    match (texto, args.linha_inicial, args.linha_final) {
        (Some(_), Some(_), _) | (Some(_), _, Some(_)) => Err(ErroFerramenta::Argumentos(
            "informe `texto` ou as linhas (`linha_inicial` e `linha_final`), não os dois".into(),
        )),
        (Some(t), None, None) => {
            if t.contains(['\n', '\r']) {
                return Err(ErroFerramenta::Argumentos(
                    "`texto` é uma linha só: um identificador ou um trecho curto".into(),
                ));
            }
            if t.chars().count() < TEXTO_MINIMO {
                return Err(ErroFerramenta::Argumentos(format!(
                    "`texto` precisa ter ao menos {TEXTO_MINIMO} caracteres"
                )));
            }
            Ok(Pedido::Texto {
                texto: t.to_owned(),
                arquivo,
            })
        }
        (None, Some(ini), Some(fim)) => {
            let arquivo = arquivo
                .ok_or_else(|| ErroFerramenta::Argumentos("as linhas pedem o `arquivo`".into()))?;
            if ini == 0 || fim < ini {
                return Err(ErroFerramenta::Argumentos(
                    "`linha_final` precisa ser maior ou igual a `linha_inicial`, e as duas a \
                     partir de 1"
                        .into(),
                ));
            }
            Ok(Pedido::Trecho {
                arquivo,
                linha_inicial: ini,
                linha_final: fim,
            })
        }
        (None, _, _) => Err(ErroFerramenta::Argumentos(
            "informe `arquivo` com `linha_inicial` e `linha_final` (o histórico do trecho) ou \
             `texto` (os commits que puseram ou tiraram um identificador)"
                .into(),
        )),
    }
}

/// `historico_do_trecho` com as CLIs e o chaveiro padrão.
pub fn historico_do_trecho() -> Ferramenta {
    historico_do_trecho_com(Ferramentas::default())
}

/// `historico_do_trecho` com as CLIs e o chaveiro informados (os testes usam CLIs
/// falsas para o rastreador).
pub fn historico_do_trecho_com(f: Ferramentas) -> Ferramenta {
    Ferramenta::nova(
        "historico_do_trecho",
        "Histórico do trecho no git",
        "Lê no histórico do git os commits que criaram e mudaram um trecho do código: \
         com `arquivo`, `linha_inicial` e `linha_final`, os commits dessas linhas; com \
         `texto` (um identificador ou trecho curto), os commits que o puseram ou o \
         tiraram, no `arquivo` ou no projeto todo. De cada commit: hash, data, mensagem e \
         as chaves de issue tiradas da mensagem (com o título da issue, quando há \
         rastreador cadastrado). O mais antigo é a origem; a saída traz a origem e os mais \
         recentes, com aviso quando corta. Use na análise de impacto, para cada regra que \
         a demanda muda: se uma demanda anterior criou a regra de propósito, a mudança \
         pode revogá-la sem o negócio saber.",
        schema(),
        move |ambiente, args| {
            let args: Args = ler_argumentos(args)?;
            let pedido = pedido_dos_argumentos(&args)?;
            let limite = args.limite.unwrap_or(LIMITE_PADRAO);
            let h = historico(ambiente, &pedido, limite, args.titulos.unwrap_or(true), &f)?;
            tracing::info!(
                commits = h.total,
                mostrados = h.commits.len(),
                titulos = h.titulos.len(),
                "histórico do trecho lido"
            );
            let (modo, alvo) = match &pedido {
                Pedido::Trecho {
                    arquivo,
                    linha_inicial,
                    linha_final,
                } => (
                    "trecho",
                    json!({ "arquivo": arquivo, "linha_inicial": linha_inicial, "linha_final": linha_final }),
                ),
                Pedido::Texto { texto, arquivo } => {
                    ("texto", json!({ "texto": texto, "arquivo": arquivo }))
                }
            };
            let dados = json!({
                "modo": modo,
                "pedido": alvo,
                "total": h.total,
                "cortados": h.cortados,
                "commits": h.commits,
                "titulos": h.titulos,
                "avisos": h.avisos,
            });
            Ok(Saida {
                texto: markdown(&pedido, &h),
                dados: Some(dados),
            })
        },
    )
    .somente_leitura()
}

// ---------------------------------------------------------------------------
// Commit citado como origem (`afirmar`, campo `origens`)

/// Hash aceitável: de 7 a 40 dígitos hexadecimais (não vira opção do git).
fn hash_valido(h: &str) -> bool {
    (7..=40).contains(&h.len()) && h.bytes().all(|b| b.is_ascii_hexdigit())
}

/// O commit citado como origem de uma regra, achado nos repositórios das pastas
/// do projeto: hash completo, data, assunto, chaves de issue e o autor (que fica
/// só no estado da análise, como detalhe técnico da evidência).
pub fn resolver_commit(ambiente: &Ambiente, hash: &str) -> Result<EvidenciaOrigem, String> {
    let hash = hash.trim();
    if !hash_valido(hash) {
        return Err(format!(
            "`{hash}` não é um hash de commit (de 7 a 40 dígitos hexadecimais, como devolve \
             `historico_do_trecho`)"
        ));
    }
    let padroes = ambiente
        .projeto
        .padroes_de_chave_compilados()
        .map_err(|e| format!("cadastro do projeto: {e}"))?;
    let alvo = format!("{hash}^{{commit}}");
    let mut algum_repositorio = false;
    for p in &ambiente.projeto.pastas {
        if !em_repositorio(&p.caminho)? {
            continue;
        }
        algum_repositorio = true;
        let s = git(
            &p.caminho,
            &[
                "show",
                "-s",
                "--no-color",
                "--date=short",
                "--format=%H%x1f%ad%x1f%an%x1f%s%x1f%b",
                &alvo,
                "--",
            ],
        )?;
        if !s.ok {
            continue;
        }
        let mut campos = s.stdout.splitn(5, SEPARADOR_DE_CAMPO);
        let (Some(completo), Some(data), Some(autor), Some(assunto)) =
            (campos.next(), campos.next(), campos.next(), campos.next())
        else {
            continue;
        };
        let corpo = campos.next().unwrap_or("").trim();
        let assunto = assunto.trim();
        let autor = autor.trim();
        return Ok(EvidenciaOrigem {
            commit: completo.trim().to_owned(),
            data: data.trim().to_owned(),
            assunto: assunto.to_owned(),
            chaves: cba_index::Projeto::chaves_de_issue(&padroes, &format!("{assunto}\n{corpo}")),
            autor: (!autor.is_empty()).then(|| autor.to_owned()),
        });
    }
    if algum_repositorio {
        Err(format!(
            "o commit `{hash}` não foi encontrado nos repositórios do projeto; use o hash que \
             `historico_do_trecho` devolveu"
        ))
    } else {
        Err("nenhuma pasta do projeto está num repositório git: não há commit para citar".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn commit(hash: &str, instante: i64) -> CommitDoTrecho {
        CommitDoTrecho {
            hash: hash.into(),
            data: "2024-01-01".into(),
            assunto: "x".into(),
            corpo: String::new(),
            chaves: Vec::new(),
            origem: false,
            instante,
        }
    }

    #[test]
    fn le_o_formato_e_ignora_o_diff_do_l() {
        let padroes = [regex::Regex::new(r"\b([A-Z][A-Z0-9]+-\d+)\b").unwrap()];
        let saida = "\u{1e}aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\u{1f}1700000000\u{1f}2023-11-14\u{1f}Ajusta mensagem\u{1f}\u{1f}\n\ndiff --git a/x b/x\n+DEM-99 no diff nao conta\n\u{1e}bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\u{1f}1600000000\u{1f}2020-09-13\u{1f}[DEM-12] Manter a fatura em aberto\u{1f}Pedido do financeiro.\nVer DEM-13.\u{1f}\n";
        let c = ler_commits(saida, &padroes);
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].assunto, "Ajusta mensagem");
        assert!(c[0].chaves.is_empty());
        assert_eq!(c[1].data, "2020-09-13");
        assert_eq!(c[1].chaves, ["DEM-12", "DEM-13"]);
        assert_eq!(c[1].corpo, "Pedido do financeiro.\nVer DEM-13.");
    }

    #[test]
    fn limite_fica_com_a_origem_e_os_mais_recentes() {
        let todos: Vec<_> = (0..7)
            .map(|i| commit(&format!("{i:07}"), 100 - i))
            .collect();
        let (mostrados, cortados) = limitar(todos.clone(), 3);
        assert_eq!(cortados, 4);
        let hashes: Vec<_> = mostrados.iter().map(|c| c.hash.as_str()).collect();
        assert_eq!(hashes, ["0000000", "0000001", "0000006"]);
        assert!(mostrados[2].origem && !mostrados[0].origem);
        let (todos_mostrados, sem_corte) = limitar(todos, 10);
        assert_eq!((todos_mostrados.len(), sem_corte), (7, 0));
        assert!(todos_mostrados[6].origem);
    }

    #[test]
    fn argumentos_pedem_um_modo_so() {
        let args = |v: Value| -> Args { serde_json::from_value(v).unwrap() };
        assert!(matches!(
            pedido_dos_argumentos(&args(
                json!({"arquivo": "a.sql", "linha_inicial": 3, "linha_final": 5})
            )),
            Ok(Pedido::Trecho {
                linha_inicial: 3,
                linha_final: 5,
                ..
            })
        ));
        assert!(matches!(
            pedido_dos_argumentos(&args(json!({"texto": "MAX_PARCELAS"}))),
            Ok(Pedido::Texto { arquivo: None, .. })
        ));
        for errado in [
            json!({"texto": "MAX", "linha_inicial": 1, "linha_final": 2, "arquivo": "a"}),
            json!({"linha_inicial": 1, "linha_final": 2}),
            json!({"arquivo": "a.sql", "linha_inicial": 5, "linha_final": 3}),
            json!({"arquivo": "a.sql"}),
            json!({"texto": "a\nb"}),
        ] {
            assert!(
                matches!(
                    pedido_dos_argumentos(&args(errado.clone())),
                    Err(ErroFerramenta::Argumentos(_))
                ),
                "{errado}"
            );
        }
    }

    #[test]
    fn hash_nao_vira_opcao_do_git() {
        assert!(hash_valido("abc1234"));
        assert!(!hash_valido("--all"));
        assert!(!hash_valido("abc12"));
        assert!(!hash_valido("HEAD~1"));
    }
}
