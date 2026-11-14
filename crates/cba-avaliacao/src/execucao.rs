//! Execução de um fluxo com o Claude Code real, isolada da configuração do usuário.
//!
//! Para cada execução:
//!
//! 1. copia a fixture para uma pasta temporária e cria nela um repositório git (como
//!    o projeto de um analista);
//! 2. cria dados do app e base de conhecimento temporários e cadastra o projeto. Nos
//!    fluxos perguntar, explicar e impacto, cria a análise, como o app faz (spec,
//!    seção 4.2). No fluxo `reuniao`, cria a demanda e a reunião na base e guarda o
//!    transcript no diretório de dados temporário ([`cba_reunioes::Guarda`]), como o
//!    app faz ao receber o transcript;
//! 3. gera o registro do servidor MCP e os hooks com o mesmo preparo do app
//!    ([`cba_session::preparo`]), mas em arquivos temporários, e os passa ao `claude`
//!    por `--mcp-config --strict-mcp-config` e `--settings`, com
//!    `--setting-sources local` (nenhuma configuração do usuário, do projeto ou MCP
//!    dele entra) e `--no-session-persistence`;
//! 4. roda `claude -p "Inicie a análise A-0001 pelo ctower"` (no fluxo `reuniao`,
//!    "Analise a reunião R-0001 da demanda D-0001 pelo ctower", e o agente cria a
//!    análise com `iniciar_analise`) na cópia, com `CBA_DATA_DIR` apontando para os
//!    dados temporários;
//! 5. lê o estado da análise (ou da ata) e compara com o gabarito, e compara a cópia
//!    com a fixture.
//!
//! A sessão usa o login do Claude Code de quem roda (assinatura ou chave), sem ler nem
//! gravar credenciais. O transcript fica só na pasta temporária, apagada no fim
//! (a não ser com `--manter`); para a saída vão o estado e a ata, que só têm os
//! trechos curtos das citações.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use cba_core::ambiente::Ambiente;
use cba_core::analise::{Analise, EstadoAnalise, Fluxo};
use cba_core::analises::NovaAnalise;
use cba_core::config::Config;
use cba_core::dados::DiretorioDados;
use cba_docs::EstadoAta;
use cba_kb::{IdReuniao, NovaDemanda};
use cba_reunioes::{Guarda, Transcript};
use cba_session::clientes::Plataforma;
use cba_session::preparo::{Alvo, ConfigClaude, Preparador};

use crate::ata::{Transcripts, avaliar_ata};
use crate::comparacao::{avaliar, nome_do_fluxo};
use crate::gabarito::{Gabarito, Item, Reuniao};
use crate::longa::{self, OpcoesLonga, TamanhoTranscript};
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

/// Projeto copiado e cadastrado, com dados e base temporários.
struct Projeto {
    codigo: PathBuf,
    dados: DiretorioDados,
    cadastro: cba_index::Projeto,
    ambiente: Ambiente,
}

fn preparar_projeto(raiz: &Path, projeto: &Path) -> Result<Projeto, String> {
    let codigo = raiz.join("faturamento");
    pastas::copiar(projeto, &codigo).map_err(erro("cópia da fixture"))?;
    git(&codigo, &["init", "-q"])?;
    git(&codigo, &["add", "-A"])?;
    git(&codigo, &["commit", "-q", "-m", "inicial"])?;

    let dados = DiretorioDados::em(raiz.join("dados"));
    let cadastro = cba_index::Projeto::novo("Faturamento Exemplo", [&codigo])
        .map_err(erro("cadastro do projeto"))?;
    cba_index::Cadastro::em(dados.projetos())
        .cadastrar(&cadastro)
        .map_err(erro("cadastro do projeto"))?;
    let base = raiz.join("base");
    cba_kb::Base::criar(&base).map_err(erro("base de conhecimento"))?;
    Config {
        base: Some(base.clone()),
        ..Default::default()
    }
    .gravar(&dados)
    .map_err(erro("configuração do app"))?;

    let ambiente = Ambiente::do_diretorio(&codigo, &dados).map_err(erro("ambiente do projeto"))?;
    Ok(Projeto {
        codigo,
        dados,
        cadastro,
        ambiente,
    })
}

/// O mesmo preparo do app, em arquivos temporários: devolve o `mcp.json` e o
/// `settings.json`.
fn preparar_claude(raiz: &Path, p: &Projeto, cba: &Path) -> Result<(PathBuf, PathBuf), String> {
    let claude = raiz.join("claude");
    fs::create_dir_all(&claude).map_err(erro("pasta da configuração"))?;
    let config = ConfigClaude {
        estado: claude.join("claude.json"),
        settings: claude.join("settings.json"),
    };
    let preparador = Preparador::novo(config.clone(), cba.to_path_buf(), Plataforma::atual())
        .map_err(erro("preparo do ambiente"))?;
    let alvo = Alvo::da_pasta(&p.cadastro.id, &p.codigo).map_err(erro("preparo do ambiente"))?;
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
    Ok((mcp, config.settings))
}

/// Ambiente preparado de uma execução.
struct Preparado {
    projeto: Projeto,
    /// Pedido ao `claude`.
    pedido: String,
    mcp: PathBuf,
    settings: PathBuf,
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
        .arg(&p.pedido)
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
        .current_dir(&p.projeto.codigo)
        .env("CBA_DATA_DIR", p.projeto.dados.raiz())
        // Uma sessão do Claude Code em volta não deve influir na execução.
        .env_remove("CLAUDECODE")
        .env_remove("CLAUDE_CODE_ENTRYPOINT")
        .env_remove("CBA_PROJETO")
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

/// Pasta temporária da execução e a raiz canônica dela.
fn temporaria() -> Result<(tempfile::TempDir, PathBuf), String> {
    let t = tempfile::Builder::new()
        .prefix("cba-avaliacao-")
        .tempdir()
        .map_err(erro("pasta temporária"))?;
    let raiz = dunce::canonicalize(t.path()).unwrap_or_else(|_| t.path().to_path_buf());
    Ok((t, raiz))
}

/// Guarda a pasta temporária, se pedido, e anota onde ela ficou.
fn manter_se_pedido(temporaria: tempfile::TempDir, opcoes: &Opcoes, pasta_saida: &Path) {
    if opcoes.manter {
        let mantida = temporaria.keep();
        let _ = fs::write(
            pasta_saida.join("pasta-temporaria.txt"),
            mantida.display().to_string(),
        );
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
        ata: None,
        transcript: None,
        arquivos_alterados: Vec::new(),
        claude: None,
        erro: None,
    };
    if let Err(e) = fs::create_dir_all(pasta_saida) {
        execucao.erro = Some(format!("pasta de saída: {e}"));
        return execucao;
    }
    let (temporaria, raiz) = match temporaria() {
        Ok(t) => t,
        Err(e) => {
            execucao.erro = Some(e);
            return execucao;
        }
    };
    let projeto = gabarito.raiz_do_projeto();

    let preparado = (|| {
        let p = preparar_projeto(&raiz, &projeto)?;
        let analise = match &item.historias {
            // Cenários: as regras pedidas gravadas na base e a análise de cenários
            // criada a partir do pedido, como pela nova análise do app.
            None if item.cenarios.is_some() => {
                let esperado = item.cenarios.as_ref().expect("conferido acima");
                crate::cenarios::preparar_regras(&p.ambiente, gabarito, &esperado.regras)?;
                cba_tools::ciclo::criar_analise_de_cenarios(
                    &p.ambiente,
                    None,
                    None,
                    Some(item.entrada.clone()),
                    Some(item.titulo.clone()),
                )
                .map_err(erro("criação da análise de cenários"))?
            }
            // Histórias: a análise de origem preparada e a de histórias ligada a ela,
            // como o app cria pela nova análise.
            Some(esperado) => {
                let origem = crate::historias::preparar_origem(&p.ambiente, gabarito, esperado)?;
                cba_tools::ciclo::criar_analise_de_historias(
                    &p.ambiente,
                    Some(origem),
                    None,
                    Some(item.entrada.clone()).filter(|e| !e.trim().is_empty()),
                    Some(item.titulo.clone()),
                )
                .map_err(erro("criação da análise de histórias"))?
            }
            None => p
                .ambiente
                .analises
                .criar(NovaAnalise {
                    fluxo: item.fluxo,
                    titulo: item.titulo.clone(),
                    entrada: item.entrada.clone(),
                    analista: None,
                    base: None,
                })
                .map_err(erro("criação da análise"))?,
        };
        let (mcp, settings) = preparar_claude(&raiz, &p, &opcoes.cba)?;
        Ok::<_, String>((
            Preparado {
                projeto: p,
                pedido: format!("Inicie a análise {} pelo ctower", analise.id),
                mcp,
                settings,
            },
            analise.id,
        ))
    })();
    let (preparado, id) = match preparado {
        Ok(p) => p,
        Err(e) => {
            execucao.erro = Some(e);
            return execucao;
        }
    };
    let arquivo_de_estado = preparado
        .projeto
        .ambiente
        .kb
        .caminho(&cba_core::analises::Analises::arquivo_de_estado(&id));
    match rodar_claude(&preparado, opcoes, pasta_saida) {
        Ok(saida) => execucao.claude = Some(SaidaClaude::do_json(&saida)),
        Err(e) => execucao.erro = Some(e),
    }

    match pastas::diferencas(&projeto, &preparado.projeto.codigo) {
        Ok(d) => execucao.arquivos_alterados = d,
        Err(e) => execucao.erro = Some(format!("comparação da cópia: {e}")),
    }

    match fs::read(&arquivo_de_estado) {
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
            execucao
                .erro
                .get_or_insert(format!("estado da análise {id} não encontrado: {e}"));
        }
    }

    manter_se_pedido(temporaria, opcoes, pasta_saida);
    execucao
}

/// Falas do transcript da reunião do gabarito, lidas pelo mesmo parser do app.
pub fn falas_da_reuniao(gabarito: &Gabarito, reuniao: &Reuniao) -> Result<Transcript, String> {
    let caminho = gabarito.transcript(reuniao);
    let bytes = fs::read(&caminho).map_err(|e| format!("{}: {e}", caminho.display()))?;
    let nome = caminho
        .file_name()
        .map(|n| n.to_string_lossy().into_owned());
    let transcript = cba_reunioes::analisar(&bytes, nome.as_deref());
    if transcript.falas.is_empty() {
        return Err(format!("{}: nenhuma fala no transcript", caminho.display()));
    }
    Ok(transcript)
}

/// Roda o fluxo `reuniao` uma vez numa reunião do gabarito e compara a ata. Com
/// `longa`, o agente analisa a reunião longa sintética gerada a partir dela
/// ([`crate::longa`]). Os registros (saída do `claude`, estado e texto da ata, que
/// só têm os trechos curtos das citações) ficam em `pasta_saida`; o transcript fica
/// só na pasta temporária.
pub fn executar_reuniao(
    gabarito: &Gabarito,
    reuniao: &Reuniao,
    longa: Option<OpcoesLonga>,
    repeticao: u32,
    opcoes: &Opcoes,
    pasta_saida: &Path,
) -> Execucao {
    let mut execucao = Execucao {
        fluxo: if longa.is_some() {
            "reuniao-longa"
        } else {
            "reuniao"
        }
        .to_owned(),
        item: reuniao.id.clone(),
        repeticao,
        avaliacao: None,
        ata: None,
        transcript: None,
        arquivos_alterados: Vec::new(),
        claude: None,
        erro: None,
    };
    if let Err(e) = fs::create_dir_all(pasta_saida) {
        execucao.erro = Some(format!("pasta de saída: {e}"));
        return execucao;
    }
    let original = match falas_da_reuniao(gabarito, reuniao) {
        Ok(t) => t,
        Err(e) => {
            execucao.erro = Some(e);
            return execucao;
        }
    };
    let transcript = match longa {
        Some(o) => Transcript {
            formato: original.formato,
            falas: longa::gerar(&original.falas, o),
            avisos: Vec::new(),
        },
        None => original.clone(),
    };
    execucao.transcript = Some(TamanhoTranscript::de(&transcript.falas));
    let (temporaria, raiz) = match temporaria() {
        Ok(t) => t,
        Err(e) => {
            execucao.erro = Some(e);
            return execucao;
        }
    };
    let projeto = gabarito.raiz_do_projeto();

    let preparado = (|| {
        let p = preparar_projeto(&raiz, &projeto)?;
        let agora = cba_core::tempo::agora_ms();
        let hoje = cba_core::tempo::data_utc(agora);
        let demanda = p
            .ambiente
            .kb
            .criar_demanda(
                &NovaDemanda {
                    titulo: reuniao.demanda.clone(),
                    ..Default::default()
                },
                hoje,
            )
            .map_err(erro("criação da demanda"))?
            .id;
        let (ata, _) =
            EstadoAta::criar(&p.ambiente.kb, demanda, hoje).map_err(erro("criação da reunião"))?;
        Guarda::nova(&p.dados)
            .guardar(p.ambiente.kb.nome(), ata.id, &transcript, agora)
            .map_err(erro("guarda do transcript"))?;
        let (mcp, settings) = preparar_claude(&raiz, &p, &opcoes.cba)?;
        Ok::<_, String>((
            Preparado {
                projeto: p,
                pedido: format!(
                    "Analise a reunião {} da demanda {demanda} pelo ctower: chame \
                     iniciar_analise com fluxo reuniao, reuniao {} e demanda {demanda}, e \
                     siga o roteiro devolvido.",
                    ata.id, ata.id
                ),
                mcp,
                settings,
            },
            ata.id,
        ))
    })();
    let (preparado, id) = match preparado {
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

    match pastas::diferencas(&projeto, &preparado.projeto.codigo) {
        Ok(d) => execucao.arquivos_alterados = d,
        Err(e) => execucao.erro = Some(format!("comparação da cópia: {e}")),
    }

    let amb = &preparado.projeto.ambiente;
    match EstadoAta::ler(&amb.kb, id) {
        Ok((estado, arquivos)) => {
            let _ = fs::write(pasta_saida.join("ata.estado.json"), estado.para_json());
            let _ = fs::copy(amb.kb.caminho(&arquivos.ata), pasta_saida.join("ata.md"));
            let mut a = avaliar_ata(
                &estado,
                reuniao,
                Some(Transcripts {
                    original: &original.falas,
                    lido: &transcript.falas,
                }),
            );
            if let Some(analise) = analise_da_reuniao(amb, id) {
                a.analise = Some(analise.id.to_string());
                a.concluida = Some(matches!(
                    analise.estado,
                    EstadoAnalise::Concluida | EstadoAnalise::Aprovada
                ));
            }
            execucao.ata = Some(a);
        }
        Err(e) => {
            execucao
                .erro
                .get_or_insert(format!("estado da ata {id} ilegível ou ausente: {e}"));
        }
    }

    manter_se_pedido(temporaria, opcoes, pasta_saida);
    execucao
}

/// Análise do fluxo `reuniao` ligada à reunião, se o agente a criou.
fn analise_da_reuniao(ambiente: &Ambiente, id: IdReuniao) -> Option<Analise> {
    ambiente
        .analises
        .listar()
        .ok()?
        .into_iter()
        .filter(|a| a.fluxo == Fluxo::Reuniao)
        .find(|a| a.reuniao == Some(id))
}

/// Copia o documento da análise para a saída, se existir.
fn copiar_documento(raiz: &Path, analise: &Analise, pasta_saida: &Path) {
    let documento = analise
        .documento
        .as_str()
        .split('/')
        .fold(raiz.join("base").join(&analise.projeto), |acc, p| {
            acc.join(p)
        });
    let _ = fs::copy(&documento, pasta_saida.join("documento.md"));
}

/// Escopo do projeto da fixture, para conferir as evidências.
pub fn escopo(gabarito: &Gabarito) -> Result<cba_index::Escopo, String> {
    cba_index::Projeto::novo("Faturamento Exemplo", [gabarito.raiz_do_projeto()])
        .and_then(|s| s.escopo())
        .map_err(erro("escopo da fixture"))
}
