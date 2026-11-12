//! Execução de um fluxo com o Claude Code real, isolada da configuração do usuário.
//!
//! Para cada execução:
//!
//! 1. copia a fixture para uma pasta temporária e cria nela um repositório git (como
//!    o sistema de um analista);
//! 2. cria dados do app e base de conhecimento temporários, cadastra o sistema e cria
//!    a análise, como o app faz (spec, seção 4.2);
//! 3. gera o registro do servidor MCP e os hooks com o mesmo preparo do app
//!    ([`cba_session::preparo`]), mas em arquivos temporários, e os passa ao `claude`
//!    por `--mcp-config --strict-mcp-config` e `--settings`, com
//!    `--setting-sources local` (nenhuma configuração do usuário, do projeto ou MCP
//!    dele entra) e `--no-session-persistence`;
//! 4. roda `claude -p "Inicie a análise A-0001 pelo ctower"` na cópia, com
//!    `CBA_DATA_DIR` apontando para os dados temporários;
//! 5. lê o estado da análise e compara com o gabarito, e compara a cópia com a
//!    fixture.
//!
//! A sessão usa o login do Claude Code de quem roda (assinatura ou chave), sem ler nem
//! gravar credenciais.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use cba_core::analise::Analise;
use cba_core::analises::NovaAnalise;
use cba_core::config::Config;
use cba_core::dados::DiretorioDados;
use cba_session::clientes::Plataforma;
use cba_session::preparo::{Alvo, ConfigClaude, Preparador};

use crate::comparacao::{avaliar, nome_do_fluxo};
use crate::gabarito::{Gabarito, Item};
use crate::pastas;
use crate::relatorio::{Execucao, SaidaClaude};

/// Ferramentas liberadas sempre: leitura e o servidor do app.
pub const FERRAMENTAS_DE_LEITURA: &str = "Read,Grep,Glob,mcp__cba";
/// Ferramentas que alteram arquivos, liberadas só com `--permitir-edicao`.
pub const FERRAMENTAS_DE_EDICAO: &str = "Edit,Write,MultiEdit,NotebookEdit,Bash";

/// Opções de uma execução.
#[derive(Debug, Clone)]
pub struct Opcoes {
    pub claude: PathBuf,
    /// Executável `cba` (servidor MCP e hooks).
    pub cba: PathBuf,
    pub modelo: String,
    pub orcamento_usd: f64,
    pub permitir_edicao: bool,
    pub tempo_limite: Duration,
    /// Mantém a pasta temporária (para depurar).
    pub manter: bool,
}

/// Erro antes de o `claude` rodar (ambiente da execução).
fn erro<E: std::fmt::Display>(o_que: &str) -> impl FnOnce(E) -> String + '_ {
    move |e| format!("{o_que}: {e}")
}

fn git(pasta: &Path, args: &[&str]) -> Result<(), String> {
    let saida = Command::new("git")
        .args([
            "-c",
            "user.name=cba-avaliacao",
            "-c",
            "user.email=avaliacao@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.autocrlf=false",
        ])
        .args(args)
        .current_dir(pasta)
        .output()
        .map_err(erro("git"))?;
    if !saida.status.success() {
        return Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&saida.stderr).trim()
        ));
    }
    Ok(())
}

/// Ambiente preparado de uma execução.
struct Preparado {
    codigo: PathBuf,
    dados: DiretorioDados,
    analise: String,
    arquivo_de_estado: PathBuf,
    mcp: PathBuf,
    settings: PathBuf,
}

fn preparar(raiz: &Path, sistema: &Path, item: &Item, cba: &Path) -> Result<Preparado, String> {
    let codigo = raiz.join("faturamento");
    pastas::copiar(sistema, &codigo).map_err(erro("cópia da fixture"))?;
    git(&codigo, &["init", "-q"])?;
    git(&codigo, &["add", "-A"])?;
    git(&codigo, &["commit", "-q", "-m", "inicial"])?;

    let dados = DiretorioDados::em(raiz.join("dados"));
    let cadastro = cba_index::Sistema::novo("Faturamento Exemplo", [&codigo])
        .map_err(erro("cadastro do sistema"))?;
    cba_index::Cadastro::em(dados.sistemas())
        .cadastrar(&cadastro)
        .map_err(erro("cadastro do sistema"))?;
    let base = raiz.join("base");
    cba_kb::Base::criar(&base).map_err(erro("base de conhecimento"))?;
    Config {
        base: Some(base.clone()),
    }
    .gravar(&dados)
    .map_err(erro("configuração do app"))?;

    let ambiente = cba_core::ambiente::Ambiente::do_diretorio(&codigo, &dados)
        .map_err(erro("ambiente do sistema"))?;
    let analise = ambiente
        .analises
        .criar(NovaAnalise {
            fluxo: item.fluxo,
            titulo: item.titulo.clone(),
            entrada: item.entrada.clone(),
            analista: None,
            base: None,
        })
        .map_err(erro("criação da análise"))?;
    let id = analise.id.to_string();
    let arquivo_de_estado = ambiente
        .kb
        .caminho(&cba_core::analises::Analises::arquivo_de_estado(
            &analise.id,
        ));

    // O mesmo preparo do app, em arquivos temporários.
    let claude = raiz.join("claude");
    fs::create_dir_all(&claude).map_err(erro("pasta da configuração"))?;
    let config = ConfigClaude {
        estado: claude.join("claude.json"),
        settings: claude.join("settings.json"),
    };
    let preparador = Preparador::novo(config.clone(), cba.to_path_buf(), Plataforma::atual())
        .map_err(erro("preparo do ambiente"))?;
    let alvo = Alvo::da_pasta(&cadastro.id, &codigo).map_err(erro("preparo do ambiente"))?;
    let feito = preparador
        .preparar(&alvo)
        .map_err(erro("preparo do ambiente"))?;
    let estado: serde_json::Value = serde_json::from_slice(
        &fs::read(&config.estado).map_err(erro("leitura do registro do MCP"))?,
    )
    .map_err(erro("leitura do registro do MCP"))?;
    let servidores = estado
        .pointer(&format!(
            "/projects/{}/mcpServers",
            feito.chave.replace('~', "~0").replace('/', "~1")
        ))
        .cloned()
        .ok_or("o preparo não registrou o servidor MCP")?;
    let mcp = claude.join("mcp.json");
    fs::write(
        &mcp,
        serde_json::to_vec_pretty(&serde_json::json!({ "mcpServers": servidores }))
            .expect("JSON serializável"),
    )
    .map_err(erro("gravação do mcp.json"))?;

    Ok(Preparado {
        codigo,
        dados,
        analise: id,
        arquivo_de_estado,
        mcp,
        settings: config.settings,
    })
}

/// Versão do `claude` instalado.
pub fn versao_do_claude(claude: &Path) -> Option<String> {
    let saida = Command::new(claude).arg("--version").output().ok()?;
    saida
        .status
        .success()
        .then(|| String::from_utf8_lossy(&saida.stdout).trim().to_owned())
}

/// Roda o `claude` e devolve a saída padrão, ou o erro (inclusive tempo esgotado).
fn rodar_claude(p: &Preparado, o: &Opcoes, pasta_saida: &Path) -> Result<String, String> {
    let mut permitidas = FERRAMENTAS_DE_LEITURA.to_owned();
    if o.permitir_edicao {
        permitidas = format!("{permitidas},{FERRAMENTAS_DE_EDICAO}");
    }
    let mut comando = Command::new(&o.claude);
    comando
        .arg("-p")
        .arg(format!("Inicie a análise {} pelo ctower", p.analise))
        .args(["--model", &o.modelo])
        .args(["--max-budget-usd", &format!("{}", o.orcamento_usd)])
        .args(["--output-format", "json"])
        .arg("--mcp-config")
        .arg(&p.mcp)
        .arg("--strict-mcp-config")
        .arg("--settings")
        .arg(&p.settings)
        .args(["--setting-sources", "local"])
        .arg("--no-session-persistence")
        .args(["--allowedTools", &permitidas]);
    if !o.permitir_edicao {
        comando.args(["--disallowedTools", FERRAMENTAS_DE_EDICAO]);
    }
    comando
        .current_dir(&p.codigo)
        .env("CBA_DATA_DIR", p.dados.raiz())
        // Uma sessão do Claude Code em volta não deve influir na execução.
        .env_remove("CLAUDECODE")
        .env_remove("CLAUDE_CODE_ENTRYPOINT")
        .env_remove("CBA_SISTEMA")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut filho = comando.spawn().map_err(erro("execução do claude"))?;

    let mut stdout = filho.stdout.take().expect("stdout com pipe");
    let mut stderr = filho.stderr.take().expect("stderr com pipe");
    let leitor = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = stdout.read_to_string(&mut s);
        s
    });
    let leitor_err = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = stderr.read_to_string(&mut s);
        s
    });
    let inicio = Instant::now();
    let status = loop {
        if let Some(status) = filho.try_wait().map_err(erro("execução do claude"))? {
            break Some(status);
        }
        if inicio.elapsed() > o.tempo_limite {
            let _ = filho.kill();
            let _ = filho.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(500));
    };
    let saida = leitor.join().unwrap_or_default();
    let saida_err = leitor_err.join().unwrap_or_default();
    let _ = fs::write(pasta_saida.join("claude.json"), &saida);
    if !saida_err.trim().is_empty() {
        let _ = fs::write(pasta_saida.join("claude.stderr.txt"), &saida_err);
    }
    match status {
        None => Err(format!(
            "tempo esgotado ({} s); a sessão foi encerrada",
            o.tempo_limite.as_secs()
        )),
        Some(s) if !s.success() && saida.trim().is_empty() => Err(format!(
            "o claude terminou com {s} sem saída: {}",
            saida_err.trim()
        )),
        Some(_) => Ok(saida),
    }
}

/// Roda um item uma vez e compara com o gabarito. Os registros (saída do `claude`,
/// estado e documento da análise) ficam em `pasta_saida`.
pub fn executar(
    gabarito: &Gabarito,
    item: &Item,
    repeticao: u32,
    opcoes: &Opcoes,
    pasta_saida: &Path,
) -> Execucao {
    let mut execucao = Execucao {
        fluxo: nome_do_fluxo(item.fluxo).to_owned(),
        item: item.id.clone(),
        repeticao,
        avaliacao: None,
        arquivos_alterados: Vec::new(),
        claude: None,
        erro: None,
    };
    if let Err(e) = fs::create_dir_all(pasta_saida) {
        execucao.erro = Some(format!("pasta de saída: {e}"));
        return execucao;
    }
    let temporaria = match tempfile::Builder::new().prefix("cba-avaliacao-").tempdir() {
        Ok(t) => t,
        Err(e) => {
            execucao.erro = Some(format!("pasta temporária: {e}"));
            return execucao;
        }
    };
    let raiz =
        dunce::canonicalize(temporaria.path()).unwrap_or_else(|_| temporaria.path().to_path_buf());
    let sistema = gabarito.raiz_do_sistema();

    let preparado = match preparar(&raiz, &sistema, item, &opcoes.cba) {
        Ok(p) => p,
        Err(e) => {
            execucao.erro = Some(e);
            return execucao;
        }
    };
    match rodar_claude(&preparado, opcoes, pasta_saida) {
        Ok(saida) => execucao.claude = Some(SaidaClaude::do_json(&saida)),
        Err(e) => execucao.erro = Some(e),
    }

    match pastas::diferencas(&sistema, &preparado.codigo) {
        Ok(d) => execucao.arquivos_alterados = d,
        Err(e) => execucao.erro = Some(format!("comparação da cópia: {e}")),
    }

    match fs::read(&preparado.arquivo_de_estado) {
        Ok(bytes) => {
            let _ = fs::write(pasta_saida.join("estado.json"), &bytes);
            match serde_json::from_slice::<Analise>(&bytes) {
                Ok(analise) => {
                    copiar_documento(&raiz, &analise, pasta_saida);
                    match escopo(gabarito) {
                        Ok(escopo) => {
                            execucao.avaliacao = Some(avaliar(&analise, item, gabarito, &escopo))
                        }
                        Err(e) => execucao.erro = Some(e),
                    }
                }
                Err(e) => execucao.erro = Some(format!("estado da análise ilegível: {e}")),
            }
        }
        Err(e) => {
            execucao.erro.get_or_insert(format!(
                "estado da análise {} não encontrado: {e}",
                preparado.analise
            ));
        }
    }

    if opcoes.manter {
        let mantida = temporaria.keep();
        let _ = fs::write(
            pasta_saida.join("pasta-temporaria.txt"),
            mantida.display().to_string(),
        );
    }
    execucao
}

/// Copia o documento da análise para a saída, se existir.
fn copiar_documento(raiz: &Path, analise: &Analise, pasta_saida: &Path) {
    let documento = analise
        .documento
        .as_str()
        .split('/')
        .fold(raiz.join("base").join(&analise.sistema), |acc, p| {
            acc.join(p)
        });
    let _ = fs::copy(&documento, pasta_saida.join("documento.md"));
}

/// Escopo do sistema da fixture, para conferir as evidências.
pub fn escopo(gabarito: &Gabarito) -> Result<cba_index::Escopo, String> {
    cba_index::Sistema::novo("Faturamento Exemplo", [gabarito.raiz_do_sistema()])
        .and_then(|s| s.escopo())
        .map_err(erro("escopo da fixture"))
}
