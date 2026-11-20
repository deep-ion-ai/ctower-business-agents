//! Ciclo de publicação da documentação (#279): analisar, aprovar, publicar e
//! analisar de novo com a documentação publicada.
//!
//! O cenário (`publicacoes` no gabarito, fluxo `publicacao` do `rodar`) faz, numa
//! pasta temporária e sem forja real:
//!
//! 1. **Analisar:** o item `analise` do gabarito, com o Claude, numa base A (a do
//!    analista que publica).
//! 2. **Aprovar** ([`aprovar_como_o_analista`]): confirma as afirmações
//!    `confirmado` com todas as evidências válidas, como o analista faria na
//!    revisão, e aprova a análise pela mesma aprovação do app; as confirmadas
//!    viram regras na base A.
//! 3. **Publicar** ([`publicar`]): o destino da documentação é um repositório
//!    próprio, um repositório bare local ([`RemotoDeDocs`]); a publicação é a do
//!    app (`cba_tools::publicacao`), com branch própria e PR. O PR vai para uma
//!    forja falsa ([`forja_falsa`]: o próprio executável copiado como `gh`), que
//!    devolve um link fictício. Depois, o PR é dado como mesclado
//!    ([`RemotoDeDocs::mesclar`]: a branch base do bare avança até a branch da
//!    publicação), como faria quem revisa.
//! 4. **Sincronizar:** numa base B, nova, de outro analista no mesmo código, com o
//!    mesmo destino (outro clone). A base B só conhece o que foi publicado: a
//!    sincronização (`cba_tools::docs_externos::sincronizar`) copia a
//!    documentação para `externos/`. Na base A ela não copiaria nada, porque a
//!    base não importa o que ela mesma publicou.
//! 5. **Analisar de novo:** o item `reanalise`, com o Claude, na base B. A análise
//!    é comparada com o gabarito como nos outros fluxos e, pela transcrição do
//!    `claude` (`--output-format stream-json`), confere-se o uso da documentação
//!    publicada ([`uso_da_documentacao`]): `conhecimento` chamado com `externos`,
//!    resultados `[doc. publicada]` devolvidos e a fonte citada na resposta.
//!
//! As etapas sem modelo (2 a 4 e a leitura da transcrição) são determinísticas e
//! testadas no CI (`tests/publicacao.rs`).

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use cba_core::ambiente::Ambiente;
use cba_core::analise::{Confianca, EstadoAfirmacao, EstadoAnalise, EstadoEvidencia};
use cba_core::dados::DiretorioDados;
use cba_core::eventos::Mudanca;
use cba_kb::Id;
use cba_repos::chaveiro::{Chaveiro, ChaveiroMemoria, conta_global};
use cba_repos::demanda::Ferramentas;
use cba_repos::{DestinoDocs, ForgeKind, RepositorioCodigo, Repositorios, Secret};
use cba_tools::aprovacao::{Aprovacao, OpcoesAprovacao, aprovar_analise};
use cba_tools::docs_externos::{self, Sincronizada};
use cba_tools::publicacao::{self as pub_docs, Alvo, Publicacao, TipoMudanca};
use serde::Serialize;

use crate::gabarito::Publicacao as Esperada;
use crate::relatorio::SaidaClaude;

/// Nome da forja falsa: o executável copiado com este nome responde como o `gh`.
pub const NOME_DA_FORJA_FALSA: &str = "gh";

/// Arquivo, ao lado da forja falsa, que marca o PR aberto.
const PR_ABERTO: &str = "pr-aberto";

/// Arquivo, ao lado da forja falsa, com as chamadas recebidas (uma por linha).
pub const CHAMADAS_DA_FORJA: &str = "chamadas-da-forja.txt";

/// Token fictício: a forja falsa não confere, e o git local não usa. Fica no
/// chaveiro em memória para a publicação nunca buscar o token real do analista
/// (chaveiro do sistema ou `gh auth token`).
const TOKEN_FICTICIO: &str = "token-ficticio-da-avaliacao";

/// Rótulo dos documentos da documentação publicada nas respostas de
/// `conhecimento` (o mesmo de `cba_tools::conhecimento`).
pub const ROTULO_EXTERNO: &str = "[doc. publicada]";

/// Branch base do repositório de docs.
pub const BRANCH_BASE: &str = "main";

/// O executável foi chamado como a forja falsa (`gh`)?
pub fn chamado_como_forja_falsa(argv0: &str) -> bool {
    // Os dois separadores, para valer igual em qualquer sistema.
    let nome = argv0.rsplit(['/', '\\']).next().unwrap_or(argv0);
    let len = nome.len();
    let nome = if len > 4 && nome[len - 4..].eq_ignore_ascii_case(".exe") {
        &nome[..len - 4]
    } else {
        nome
    };
    nome.eq_ignore_ascii_case(NOME_DA_FORJA_FALSA)
}

/// Responde como a API do GitHub pelo `gh api`, só no que a publicação usa:
/// procurar o PR aberto da branch (`repos/<dono>/<repo>/pulls?...`) e abrir o PR
/// (`--method POST repos/<dono>/<repo>/pulls`). O estado (PR aberto) e as
/// chamadas ficam em `pasta` (a pasta do executável). Devolve a saída e o código
/// de saída.
pub fn forja_falsa(args: &[String], pasta: &Path) -> (String, i32) {
    let _ = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(pasta.join(CHAMADAS_DA_FORJA))
        .and_then(|mut f| {
            use std::io::Write as _;
            writeln!(f, "{}", args.join(" "))
        });
    if args.first().map(String::as_str) != Some("api") {
        return (
            format!("forja falsa: comando inesperado: {}", args.join(" ")),
            3,
        );
    }
    let rota = args
        .iter()
        .skip(1)
        .find(|a| a.starts_with("repos/"))
        .cloned()
        .unwrap_or_default();
    let repo: String = rota
        .trim_start_matches("repos/")
        .split('/')
        .take(2)
        .collect::<Vec<_>>()
        .join("/");
    let link = format!("https://github.com/{repo}/pull/1");
    let aberto = pasta.join(PR_ABERTO);
    if args.iter().any(|a| a == "POST") && rota.ends_with("/pulls") {
        let _ = fs::write(&aberto, &link);
        return (serde_json::json!({ "html_url": link }).to_string(), 0);
    }
    if rota.contains("/pulls?") {
        let lista = if aberto.is_file() {
            serde_json::json!([{ "html_url": link }])
        } else {
            serde_json::json!([])
        };
        return (lista.to_string(), 0);
    }
    (
        format!("forja falsa: rota inesperada: {}", args.join(" ")),
        3,
    )
}

/// Copia o executável `exe` para `pasta` com o nome da forja falsa e devolve o
/// caminho da cópia.
pub fn instalar_forja_falsa(exe: &Path, pasta: &Path) -> Result<PathBuf, String> {
    fs::create_dir_all(pasta).map_err(|e| format!("pasta da forja falsa: {e}"))?;
    let nome = if cfg!(windows) {
        format!("{NOME_DA_FORJA_FALSA}.exe")
    } else {
        NOME_DA_FORJA_FALSA.to_owned()
    };
    let destino = pasta.join(nome);
    fs::copy(exe, &destino).map_err(|e| format!("cópia da forja falsa: {e}"))?;
    Ok(destino)
}

/// Roda o `git` sem depender da configuração do analista para o que importa
/// (autor, assinatura, branch inicial). Devolve a saída padrão, sem o fim de
/// linha.
pub fn git(git: &Path, pasta: &Path, args: &[&str]) -> Result<String, String> {
    let saida = Command::new(git)
        .args([
            "-c",
            "user.name=cba-avaliacao",
            "-c",
            "user.email=avaliacao@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.autocrlf=false",
            "-c",
            "init.defaultBranch=main",
        ])
        .args(args)
        .current_dir(pasta)
        .output()
        .map_err(|e| format!("git: {e}"))?;
    if !saida.status.success() {
        return Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&saida.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&saida.stdout).trim_end().to_owned())
}

/// URL `file://` de uma pasta local, com `/` também no Windows.
pub fn url_de_arquivo(pasta: &Path) -> String {
    let s = pasta.to_string_lossy().replace('\\', "/");
    if s.starts_with('/') {
        format!("file://{s}")
    } else {
        format!("file:///{s}")
    }
}

/// Repositório de docs local: um bare que faz o papel do remoto, com um
/// `leia-me.md` escrito à mão (que a publicação não pode apagar).
#[derive(Debug, Clone)]
pub struct RemotoDeDocs {
    pub git: PathBuf,
    pub bare: PathBuf,
    pub url: String,
}

impl RemotoDeDocs {
    /// Cria o remoto em `raiz/docs-remoto.git`.
    pub fn criar(git_exe: &Path, raiz: &Path) -> Result<Self, String> {
        let semente = raiz.join("docs-semente");
        fs::create_dir_all(&semente).map_err(|e| format!("semente do repositório: {e}"))?;
        git(git_exe, &semente, &["init", "-q"])?;
        git(
            git_exe,
            &semente,
            &["symbolic-ref", "HEAD", &format!("refs/heads/{BRANCH_BASE}")],
        )?;
        fs::write(
            semente.join("leia-me.md"),
            "# Documentação do Faturamento Exemplo\n\nPublicada pelo ctower-business-agents.\n",
        )
        .map_err(|e| format!("semente do repositório: {e}"))?;
        git(git_exe, &semente, &["add", "-A"])?;
        git(git_exe, &semente, &["commit", "-q", "-m", "inicial"])?;
        git(
            git_exe,
            raiz,
            &["clone", "-q", "--bare", "docs-semente", "docs-remoto.git"],
        )?;
        let bare = raiz.join("docs-remoto.git");
        Ok(Self {
            git: git_exe.to_path_buf(),
            url: url_de_arquivo(&bare),
            bare,
        })
    }

    /// Clona o remoto em `raiz/<nome>` (o clone de um analista).
    pub fn clonar(&self, raiz: &Path, nome: &str) -> Result<PathBuf, String> {
        git(&self.git, raiz, &["clone", "-q", &self.url, nome])?;
        Ok(raiz.join(nome))
    }

    /// O PR é mesclado por quem revisa: a branch base avança até a branch da
    /// publicação (fast-forward, como um merge sem commit de merge) e a forja
    /// falsa deixa de ter PR aberto. Devolve o commit da base.
    pub fn mesclar(&self, branch: &str, forja: &Path) -> Result<String, String> {
        let ponta = git(&self.git, &self.bare, &["rev-parse", branch])?;
        git(
            &self.git,
            &self.bare,
            &["update-ref", &format!("refs/heads/{BRANCH_BASE}"), &ponta],
        )?;
        if let Some(pasta) = forja.parent() {
            let _ = fs::remove_file(pasta.join(PR_ABERTO));
        }
        Ok(ponta)
    }

    /// Arquivos da branch base, relativos à raiz do repositório.
    pub fn arquivos(&self) -> Result<Vec<String>, String> {
        let saida = git(
            &self.git,
            &self.bare,
            &["ls-tree", "-r", "--name-only", BRANCH_BASE],
        )?;
        Ok(saida.lines().map(str::to_owned).collect())
    }
}

/// Destino da documentação (repositório próprio) apontando para o clone.
pub fn destino(esperada: &Esperada, remoto: &RemotoDeDocs, clone: &Path) -> Repositorios {
    Repositorios {
        docs: Some(DestinoDocs::Repositorio {
            repositorio: RepositorioCodigo {
                forja: ForgeKind::Github,
                host: "github.com".into(),
                caminho: esperada.repositorio.clone(),
                url_clone: remoto.url.clone(),
                pasta: Some(clone.to_path_buf()),
                branch: BRANCH_BASE.into(),
                ..Default::default()
            },
            subpasta: esperada.subpasta.clone(),
        }),
        ..Default::default()
    }
}

/// Ferramentas da publicação e da sincronização: o `git` dado, a forja falsa e
/// um chaveiro em memória com um token fictício (nunca o do analista).
pub fn ferramentas(git: &Path, forja: &Path) -> Ferramentas {
    let chaveiro = ChaveiroMemoria::default();
    let _ = chaveiro.gravar(&conta_global("github.com"), &Secret::new(TOKEN_FICTICIO));
    Ferramentas {
        git: git.to_path_buf(),
        gh: forja.to_path_buf(),
        glab: forja.to_path_buf(),
        chaveiro: Arc::new(chaveiro),
        ..Default::default()
    }
}

/// Aprova a análise como o analista faria na revisão: confirma as afirmações
/// `confirmado` com todas as evidências válidas (as outras ficam como estão e
/// não viram regra) e aprova pela aprovação do app, que grava as regras e o
/// documento aprovado na base. Recusa a análise que o agente não concluiu.
pub fn aprovar_como_o_analista(
    dados: &DiretorioDados,
    ambiente: &Ambiente,
    id: &Id,
) -> Result<Aprovacao, String> {
    let analise = ambiente
        .analises
        .carregar(id)
        .map_err(|e| format!("análise {id}: {e}"))?;
    if !matches!(
        analise.estado,
        EstadoAnalise::Concluida | EstadoAnalise::Aprovada
    ) {
        return Err(format!(
            "a análise {id} não foi concluída pelo agente ({:?}): não há o que aprovar",
            analise.estado
        ));
    }
    ambiente
        .analises
        .alterar(id, Mudanca::Afirmacao, |a, _| {
            let confirmar: Vec<u32> = a
                .afirmacoes
                .iter()
                .filter(|x| {
                    x.confianca == Confianca::Confirmado
                        && x.estado == EstadoAfirmacao::Registrada
                        && !x.evidencias.is_empty()
                        && x.evidencias
                            .iter()
                            .all(|e| e.estado == EstadoEvidencia::Valida)
                })
                .map(|x| x.numero)
                .collect();
            for n in confirmar {
                a.marcar_afirmacao(n, EstadoAfirmacao::Confirmada)?;
            }
            Ok(())
        })
        .map_err(|e| format!("revisão da análise {id}: {e}"))?;
    aprovar_analise(dados, ambiente.kb.nome(), id, &OpcoesAprovacao::default())
        .map_err(|e| format!("aprovação da análise {id}: {e}"))
}

/// Publica a base aprovada do projeto no destino, pela publicação do app.
pub fn publicar(
    ambiente: &Ambiente,
    repositorios: &Repositorios,
    f: &Ferramentas,
) -> Result<Publicacao, String> {
    pub_docs::publicar(
        f,
        &Alvo {
            projeto: &ambiente.kb,
            nome: &ambiente.projeto.nome,
            repositorios,
        },
    )
}

/// Sincroniza o repositório de docs na base do ambiente (cópia em `externos/`).
pub fn sincronizar(ambiente: &Ambiente, f: &Ferramentas) -> Result<Sincronizada, String> {
    docs_externos::sincronizar(ambiente, f).map_err(|e| format!("sincronização: {e}"))
}

/// Caminhos publicados como novos ou alterados (relativos ao repositório).
pub fn publicados(p: &Publicacao) -> Vec<String> {
    p.previa
        .mudancas
        .iter()
        .filter(|m| m.tipo != TipoMudanca::Removido)
        .map(|m| m.caminho.clone())
        .collect()
}

/// Uso da documentação publicada na segunda análise, lido da transcrição do
/// `claude` (`--output-format stream-json`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct UsoDaDocumentacao {
    /// Chamadas a `conhecimento`.
    pub consultas: usize,
    /// Chamadas a `conhecimento` com `externos` `incluir` ou `somente`.
    pub consultas_com_externos: usize,
    /// Respostas de `conhecimento` com algum documento `[doc. publicada]`.
    pub respostas_com_externos: usize,
    /// Leituras (`Read`) de arquivos da cópia em `externos/`.
    pub leituras_de_externos: usize,
    /// A resposta final do agente cita a fonte: o repositório de docs ou o commit.
    pub fonte_citada: bool,
}

impl UsoDaDocumentacao {
    /// O agente pediu a documentação publicada e recebeu ao menos um documento
    /// dela (ou leu um).
    pub fn usada(&self) -> bool {
        self.consultas_com_externos > 0
            && (self.respostas_com_externos > 0 || self.leituras_de_externos > 0)
    }
}

/// Texto do conteúdo de um `tool_result` (texto ou lista de blocos de texto).
fn texto_do_resultado(conteudo: &serde_json::Value) -> String {
    match conteudo {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(blocos) => blocos
            .iter()
            .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// Lê a transcrição (`stream-json`, uma mensagem JSON por linha) e mede o uso da
/// documentação publicada. `repositorio` é o caminho do repositório de docs
/// (`dono/repo`) e `commit`, o commit sincronizado: a fonte é citada quando a
/// resposta final traz o nome do repositório ou o commit (7 caracteres).
pub fn uso_da_documentacao(
    transcricao: &str,
    repositorio: &str,
    commit: &str,
) -> UsoDaDocumentacao {
    let mut uso = UsoDaDocumentacao::default();
    // Ferramenta de cada chamada, pelo id, para ligar a resposta à chamada.
    let mut chamadas: HashMap<String, String> = HashMap::new();
    let mut resposta_final = String::new();
    for linha in transcricao.lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(linha.trim()) else {
            continue;
        };
        let blocos = v
            .pointer("/message/content")
            .and_then(|c| c.as_array())
            .cloned()
            .unwrap_or_default();
        match v.get("type").and_then(|t| t.as_str()) {
            Some("assistant") => {
                for b in blocos
                    .iter()
                    .filter(|b| b.get("type").and_then(|t| t.as_str()) == Some("tool_use"))
                {
                    let nome = b.get("name").and_then(|n| n.as_str()).unwrap_or("");
                    if let Some(id) = b.get("id").and_then(|i| i.as_str()) {
                        chamadas.insert(id.to_owned(), nome.to_owned());
                    }
                    let entrada = b.get("input").cloned().unwrap_or_default();
                    if nome.ends_with("conhecimento") {
                        uso.consultas += 1;
                        if matches!(
                            entrada.get("externos").and_then(|e| e.as_str()),
                            Some("incluir" | "somente")
                        ) {
                            uso.consultas_com_externos += 1;
                        }
                    } else if nome == "Read" {
                        let arquivo = entrada
                            .get("file_path")
                            .and_then(|f| f.as_str())
                            .unwrap_or("")
                            .replace('\\', "/");
                        if arquivo.contains("/externos/") {
                            uso.leituras_de_externos += 1;
                        }
                    }
                }
            }
            Some("user") => {
                for b in blocos
                    .iter()
                    .filter(|b| b.get("type").and_then(|t| t.as_str()) == Some("tool_result"))
                {
                    let id = b.get("tool_use_id").and_then(|i| i.as_str()).unwrap_or("");
                    let e_conhecimento = chamadas
                        .get(id)
                        .is_some_and(|n| n.ends_with("conhecimento"));
                    if e_conhecimento
                        && texto_do_resultado(b.get("content").unwrap_or(&serde_json::Value::Null))
                            .contains(ROTULO_EXTERNO)
                    {
                        uso.respostas_com_externos += 1;
                    }
                }
            }
            Some("result") => {
                resposta_final = v
                    .get("result")
                    .and_then(|r| r.as_str())
                    .unwrap_or("")
                    .to_owned();
            }
            _ => {}
        }
    }
    let resposta = resposta_final.to_lowercase();
    let nome = repositorio
        .rsplit('/')
        .next()
        .unwrap_or(repositorio)
        .to_lowercase();
    let curto: String = commit.chars().take(7).collect::<String>().to_lowercase();
    uso.fonte_citada = (!nome.is_empty() && resposta.contains(&nome))
        || (curto.len() == 7 && resposta.contains(&curto));
    uso
}

/// A linha `result` da transcrição (`stream-json`), no formato da saída JSON do
/// `claude -p`.
pub fn saida_da_transcricao(transcricao: &str) -> SaidaClaude {
    transcricao
        .lines()
        .rev()
        .find(|l| l.contains("\"type\":\"result\"") || l.contains("\"type\": \"result\""))
        .map(SaidaClaude::do_json)
        .unwrap_or_default()
}

/// Resultado do ciclo de publicação, à parte da comparação da segunda análise
/// com o gabarito.
#[derive(Debug, Clone, Default, Serialize)]
pub struct CicloAvaliado {
    /// Item analisado e publicado, e a análise criada para ele (base A).
    pub analise: String,
    pub primeira_analise: Option<String>,
    /// Item analisado de novo (base B).
    pub reanalise: String,
    /// Saída do `claude` em cada análise.
    pub primeira: Option<SaidaClaude>,
    pub segunda: Option<SaidaClaude>,
    /// Teto do ciclo: o orçamento das duas execuções, em US$.
    pub teto_usd: f64,
    /// A primeira análise foi aprovada.
    pub aprovada: bool,
    /// Regras gravadas na base A pela aprovação.
    pub regras_aprovadas: usize,
    /// Documentos novos ou alterados na publicação (caminhos no repositório).
    pub publicados: Vec<String>,
    /// Situação do PR (`aberto`, `atualizado`, `sem_mudancas`) e o link.
    pub pr: Option<String>,
    pub link: Option<String>,
    /// Commit da branch base do repositório de docs depois do merge.
    pub commit: Option<String>,
    /// Documentos copiados para `externos/` na base B.
    pub importados: usize,
    /// Uso da documentação publicada na segunda análise.
    pub uso: Option<UsoDaDocumentacao>,
}

impl CicloAvaliado {
    /// Custo das duas execuções, em US$.
    pub fn custo_usd(&self) -> f64 {
        [&self.primeira, &self.segunda]
            .iter()
            .filter_map(|s| s.as_ref().and_then(|s| s.custo_usd))
            .sum()
    }

    /// Falhas graves do ciclo (além das da segunda análise e do erro de etapa).
    pub fn falhas(&self) -> Vec<String> {
        let mut f = Vec::new();
        if self.primeira_analise.is_some() && !self.aprovada {
            f.push("a primeira análise não foi aprovada".to_owned());
        }
        if self.aprovada && self.publicados.is_empty() {
            f.push("a publicação não levou nenhum documento ao repositório".to_owned());
        }
        if self.commit.is_some() && self.importados == 0 {
            f.push("a sincronização não trouxe a documentação publicada para a base".to_owned());
        }
        if let Some(u) = &self.uso
            && !u.usada()
        {
            f.push(
                "a segunda análise não consultou a documentação publicada (conhecimento com \
                 externos)"
                    .to_owned(),
            );
        }
        f
    }
}
