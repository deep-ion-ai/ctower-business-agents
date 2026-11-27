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
//!    ([`cba_session::preparo`]): o estado do Claude em arquivos temporários e os
//!    hooks no `.claude/settings.local.json` da cópia (#304; a fixture não é
//!    tocada). Passa o servidor ao `claude` por `--mcp-config --strict-mcp-config`, e
//!    os hooks pelo próprio arquivo local, com `--setting-sources local` (nenhuma
//!    configuração do usuário, do projeto ou MCP dele entra) e
//!    `--no-session-persistence`. O mesmo arquivo vai também em `--settings` (o
//!    Claude Code roda uma vez só o comando repetido);
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
use cba_core::conducao::Conducao;
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

/// Copia a fixture e cria o repositório git: um commit só ou, com `historico` (a
/// pasta de um histórico sintético, #450), os commits dele.
fn preparar_projeto(
    raiz: &Path,
    projeto: &Path,
    historico: Option<&Path>,
) -> Result<Projeto, String> {
    let codigo = raiz.join("faturamento");
    pastas::copiar(projeto, &codigo).map_err(erro("cópia da fixture"))?;
    match historico {
        Some(pasta) => {
            crate::historico::Historico::ler(pasta)?.reproduzir(pasta, projeto, &codigo)?
        }
        None => {
            git(&codigo, &["init", "-q"])?;
            git(&codigo, &["add", "-A"])?;
            git(&codigo, &["commit", "-q", "-m", "inicial"])?;
        }
    }
    cadastrar_projeto(
        raiz,
        &codigo,
        "dados",
        "base",
        cba_repos::Repositorios::default(),
    )
}

/// Cadastra o projeto em `codigo` com dados do app (`raiz/<dados>`) e base
/// (`raiz/<base>`) próprios e os repositórios dados.
fn cadastrar_projeto(
    raiz: &Path,
    codigo: &Path,
    dados: &str,
    base: &str,
    repositorios: cba_repos::Repositorios,
) -> Result<Projeto, String> {
    let dados = DiretorioDados::em(raiz.join(dados));
    let mut cadastro = cba_index::Projeto::novo("Faturamento Exemplo", [codigo])
        .map_err(erro("cadastro do projeto"))?;
    cadastro.repositorios = repositorios;
    cba_index::Cadastro::em(dados.projetos())
        .cadastrar(&cadastro)
        .map_err(erro("cadastro do projeto"))?;
    let base = raiz.join(base);
    cba_kb::Base::criar(&base).map_err(erro("base de conhecimento"))?;
    Config {
        base: Some(base.clone()),
        ..Default::default()
    }
    .gravar(&dados)
    .map_err(erro("configuração do app"))?;

    let ambiente = Ambiente::do_diretorio(codigo, &dados).map_err(erro("ambiente do projeto"))?;
    Ok(Projeto {
        codigo: codigo.to_path_buf(),
        dados,
        cadastro,
        ambiente,
    })
}

/// O mesmo preparo do app, com o estado do Claude em arquivos temporários: devolve o
/// `mcp.json` e o `settings.local.json` com os hooks, na cópia.
fn preparar_claude(raiz: &Path, p: &Projeto, cba: &Path) -> Result<(PathBuf, PathBuf), String> {
    preparar_claude_em(&raiz.join("claude"), p, cba)
}

/// Como [`preparar_claude`], com os arquivos na pasta `claude`.
fn preparar_claude_em(
    claude: &Path,
    p: &Projeto,
    cba: &Path,
) -> Result<(PathBuf, PathBuf), String> {
    fs::create_dir_all(claude).map_err(erro("pasta da configuração"))?;
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
    let settings = feito
        .alteracoes
        .iter()
        .find(|a| matches!(a.item, cba_session::preparo::Item::Hook { .. }))
        .map(|a| a.arquivo.clone())
        .ok_or("o preparo não gravou os hooks")?;
    Ok((mcp, settings))
}

/// Ambiente preparado de uma execução.
struct Preparado {
    projeto: Projeto,
    /// Pedido ao `claude`.
    pedido: String,
    mcp: PathBuf,
    settings: PathBuf,
    /// Condução pelo workflow (#371): o identificador da sessão, que fica gravada
    /// para a medição dos agentes ([`crate::conducao`]) e é apagada depois.
    sessao: Option<String>,
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
    rodar_claude_como(p, o, pasta_saida, "claude", false)
}

/// Como [`rodar_claude`], gravando a saída em `<nome>.json` (ou, com
/// `transcricao`, a transcrição `stream-json` em `<nome>.jsonl`, com cada
/// mensagem e chamada de ferramenta).
fn rodar_claude_como(
    p: &Preparado,
    o: &Opcoes,
    pasta_saida: &Path,
    nome: &str,
    transcricao: bool,
) -> Result<String, String> {
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
        .args([
            "--output-format",
            if transcricao { "stream-json" } else { "json" },
        ])
        .args(if transcricao { &["--verbose"][..] } else { &[] })
        .arg("--mcp-config")
        .arg(&p.mcp)
        .arg("--strict-mcp-config")
        .arg("--settings")
        .arg(&p.settings)
        .args(["--allowedTools", &permitidas]);
    match &p.sessao {
        // O workflow só é achado com a origem `project` (spike da #352, R1): o
        // `.claude/workflows/` da cópia, instalado pelo preparo. A cópia não tem
        // `.claude/settings.json`, então nada mais entra.
        Some(id) => {
            comando
                .args(["--setting-sources", "project,local"])
                .args(["--session-id", id]);
        }
        None => {
            comando
                .args(["--setting-sources", "local"])
                .arg("--no-session-persistence");
        }
    }
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
    let extensao = if transcricao { "jsonl" } else { "json" };
    let _ = fs::write(pasta_saida.join(format!("{nome}.{extensao}")), &saida);
    if !saida_err.trim().is_empty() {
        let _ = fs::write(pasta_saida.join(format!("{nome}.stderr.txt")), &saida_err);
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
    executar_com(gabarito, item, repeticao, opcoes, pasta_saida, None)
}

/// Como [`executar`], com a condução pedida (`rodar --conducao`, #371). Com
/// `Some`, o relatório traz a medição da condução ([`crate::conducao`]); com
/// [`Conducao::Workflow`], o pedido é o do app para o workflow do fluxo.
pub fn executar_com(
    gabarito: &Gabarito,
    item: &Item,
    repeticao: u32,
    opcoes: &Opcoes,
    pasta_saida: &Path,
    conducao: Option<Conducao>,
) -> Execucao {
    let mut execucao = Execucao {
        fluxo: nome_do_fluxo(item.fluxo).to_owned(),
        item: item.id.clone(),
        repeticao,
        avaliacao: None,
        ata: None,
        transcript: None,
        mapa: None,
        publicacao: None,
        conducao: None,
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
        let historico = item
            .origem
            .as_ref()
            .map(|o| gabarito.pasta_do_historico(&o.historico));
        let p = preparar_projeto(&raiz, &projeto, historico.as_deref())?;
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
        let (pedido, sessao) = match conducao {
            Some(Conducao::Workflow) => {
                let nome = cba_tools::conducao::workflow_do_fluxo(analise.fluxo).ok_or(
                    "o fluxo não tem workflow: use --conducao simples ou só a análise de impacto",
                )?;
                (
                    cba_tools::conducao::pedido_de_inicio(&analise.id, &nome),
                    Some(crate::conducao::novo_id_de_sessao()),
                )
            }
            _ => (format!("Inicie a análise {} pelo ctower", analise.id), None),
        };
        Ok::<_, String>((
            Preparado {
                projeto: p,
                pedido,
                mcp,
                settings,
                sessao,
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
    let relogio = Instant::now();
    match rodar_claude(&preparado, opcoes, pasta_saida) {
        Ok(saida) => {
            let mut c = SaidaClaude::do_json(&saida);
            // Na condução pelo workflow, o `duration_ms` é só o do último turno (o
            // que concluiu depois do aviso de fim): vale o tempo de relógio.
            if conducao.is_some() {
                c.duracao_ms = Some(relogio.elapsed().as_millis() as u64);
            }
            execucao.claude = Some(c);
        }
        Err(e) => execucao.erro = Some(e),
    }
    // A sessão gravada (condução pelo workflow): os journals vão para a saída, os
    // agentes são medidos e a sessão é apagada.
    let sessao = preparado
        .sessao
        .as_deref()
        .and_then(crate::conducao::pasta_da_sessao);
    let pasta_da_sessao = preparado
        .sessao
        .as_deref()
        .zip(sessao.as_deref())
        .map(|(id, projeto)| projeto.join(id));
    if let Some(p) = &pasta_da_sessao {
        crate::conducao::copiar_journals(p, pasta_saida);
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
                    if let Some(c) = conducao {
                        execucao.conducao = Some(crate::conducao::medir(
                            c,
                            &analise,
                            pasta_da_sessao.as_deref(),
                        ));
                    }
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

    if let Some(p) = &sessao
        && !opcoes.manter
    {
        crate::conducao::apagar_sessao(p);
    }
    manter_se_pedido(temporaria, opcoes, pasta_saida);
    execucao
}

/// Roda o mapeamento das funcionalidades (#208) uma vez e compara o mapa do
/// projeto com o gabarito. O mapa não é uma análise: o agente começa por
/// `iniciar_mapeamento`, e o estado lido é `funcionalidades/mapa.estado.json`.
/// O estado e o documento do mapa ficam em `pasta_saida`.
pub fn executar_mapeamento(
    gabarito: &Gabarito,
    mapeamento: &crate::gabarito::Mapeamento,
    repeticao: u32,
    opcoes: &Opcoes,
    pasta_saida: &Path,
) -> Execucao {
    let mut execucao = Execucao {
        fluxo: "mapear".to_owned(),
        item: mapeamento.id.clone(),
        repeticao,
        avaliacao: None,
        ata: None,
        transcript: None,
        mapa: None,
        publicacao: None,
        conducao: None,
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
        let p = preparar_projeto(&raiz, &projeto, None)?;
        let (mcp, settings) = preparar_claude(&raiz, &p, &opcoes.cba)?;
        Ok::<_, String>(Preparado {
            projeto: p,
            pedido: cba_tools::mapear::pedido_de_mapeamento(None, None, None),
            mcp,
            settings,
            sessao: None,
        })
    })();
    let preparado = match preparado {
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
    let kb = &preparado.projeto.ambiente.kb;
    for (relativo, nome) in [
        (cba_kb::layout::estado_do_mapa(), "mapa.estado.json"),
        (cba_kb::layout::mapa_de_funcionalidades(), "mapa.md"),
    ] {
        if let Ok(bytes) = fs::read(kb.caminho(&relativo)) {
            let _ = fs::write(pasta_saida.join(nome), bytes);
        }
    }
    match cba_core::funcionalidades::MapaFuncionalidades::carregar(kb) {
        Ok(mapa) if mapa.versao > 0 => match escopo(gabarito) {
            Ok(escopo) => {
                execucao.mapa = Some(crate::mapeamento::avaliar_mapeamento(
                    &mapa,
                    &mapeamento.esperado,
                    &escopo,
                ))
            }
            Err(e) => execucao.erro = Some(e),
        },
        Ok(_) => {
            execucao
                .erro
                .get_or_insert("o agente não registrou nada no mapa".to_owned());
        }
        Err(e) => {
            execucao
                .erro
                .get_or_insert(format!("estado do mapa ilegível: {e}"));
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
        mapa: None,
        publicacao: None,
        conducao: None,
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
        let p = preparar_projeto(&raiz, &projeto, None)?;
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
                sessao: None,
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
    copiar_documento_como(
        &raiz.join("base"),
        analise,
        &pasta_saida.join("documento.md"),
    );
}

/// Copia o documento da análise, da base em `base`, para `destino`, se existir.
fn copiar_documento_como(base: &Path, analise: &Analise, destino: &Path) {
    let documento = analise
        .documento
        .as_str()
        .split('/')
        .fold(base.join(&analise.projeto), |acc, p| acc.join(p));
    let _ = fs::copy(&documento, destino);
}

/// Escopo do projeto da fixture, para conferir as evidências.
pub fn escopo(gabarito: &Gabarito) -> Result<cba_index::Escopo, String> {
    cba_index::Projeto::novo("Faturamento Exemplo", [gabarito.raiz_do_projeto()])
        .and_then(|s| s.escopo())
        .map_err(erro("escopo da fixture"))
}

/// Cria a análise de um item de pergunta, funcionalidade ou demanda, como o app.
fn criar_analise(ambiente: &Ambiente, item: &Item) -> Result<Analise, String> {
    ambiente
        .analises
        .criar(NovaAnalise {
            fluxo: item.fluxo,
            titulo: item.titulo.clone(),
            entrada: item.entrada.clone(),
            analista: None,
            base: None,
        })
        .map_err(erro("criação da análise"))
}

/// Roda o ciclo de publicação da documentação (#279) uma vez: analisa e aprova o
/// item `analise` na base A, publica no repositório de docs local (PR na forja
/// falsa, dado como mesclado), sincroniza numa base B nova e analisa o item
/// `reanalise` nela. A segunda análise é comparada com o gabarito, e o uso da
/// documentação publicada é lido da transcrição. `exe` é o executável copiado
/// como forja falsa ([`crate::publicacao::forja_falsa`]). O teto do ciclo é o
/// orçamento das duas execuções.
pub fn executar_publicacao(
    gabarito: &Gabarito,
    esperada: &crate::gabarito::Publicacao,
    exe: &Path,
    repeticao: u32,
    opcoes: &Opcoes,
    pasta_saida: &Path,
) -> Execucao {
    use crate::publicacao::{self as ciclo, CicloAvaliado, RemotoDeDocs};

    let mut execucao = Execucao {
        fluxo: "publicacao".to_owned(),
        item: esperada.id.clone(),
        repeticao,
        avaliacao: None,
        ata: None,
        transcript: None,
        mapa: None,
        publicacao: None,
        conducao: None,
        arquivos_alterados: Vec::new(),
        claude: None,
        erro: None,
    };
    let mut resultado = CicloAvaliado {
        analise: esperada.analise.clone(),
        reanalise: esperada.reanalise.clone(),
        teto_usd: opcoes.orcamento_usd * 2.0,
        ..Default::default()
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
    let git_exe = PathBuf::from("git");

    // Cada etapa pode parar o ciclo; o que já foi medido fica em `resultado`.
    let mut rodar = || -> Result<(Analise, Item), String> {
        let item_a = gabarito
            .item(&esperada.analise)
            .map_err(|e| e.to_string())?;
        let item_b = gabarito
            .item(&esperada.reanalise)
            .map_err(|e| e.to_string())?;

        // 1. Analisar, na base A.
        let a = preparar_projeto(&raiz, &projeto, None)?;
        let codigo = a.codigo.clone();
        let analise_a = criar_analise(&a.ambiente, &item_a)?;
        resultado.primeira_analise = Some(analise_a.id.to_string());
        let (mcp, settings) = preparar_claude_em(&raiz.join("claude-a"), &a, &opcoes.cba)?;
        let preparado = Preparado {
            projeto: a,
            pedido: format!("Inicie a análise {} pelo ctower", analise_a.id),
            mcp,
            settings,
            sessao: None,
        };
        let saida = rodar_claude_como(&preparado, opcoes, pasta_saida, "claude-1", true)?;
        resultado.primeira = Some(ciclo::saida_da_transcricao(&saida));
        let a = preparado.projeto;
        let estado = a
            .ambiente
            .kb
            .caminho(&cba_core::analises::Analises::arquivo_de_estado(
                &analise_a.id,
            ));
        let _ = fs::copy(&estado, pasta_saida.join("estado-1.json"));

        // 2. Aprovar, como o analista.
        let aprovacao = ciclo::aprovar_como_o_analista(&a.dados, &a.ambiente, &analise_a.id)?;
        resultado.aprovada = true;
        resultado.regras_aprovadas = aprovacao.regras.len();
        copiar_documento_como(
            &raiz.join("base"),
            &aprovacao.analise,
            &pasta_saida.join("documento-1.md"),
        );

        // 3. Publicar no repositório de docs local, com o PR na forja falsa, e
        // dar o PR como mesclado.
        let forja = ciclo::instalar_forja_falsa(exe, &raiz.join("forja"))?;
        let remoto = RemotoDeDocs::criar(&git_exe, &raiz)?;
        let clone_a = remoto.clonar(&raiz, "docs-a")?;
        let f = ciclo::ferramentas(&git_exe, &forja);
        let publicacao = ciclo::publicar(
            &a.ambiente,
            &ciclo::destino(esperada, &remoto, &clone_a),
            &f,
        )?;
        resultado.publicados = ciclo::publicados(&publicacao);
        resultado.pr = serde_json::to_value(publicacao.publicado.situacao)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned));
        resultado.link = publicacao.publicado.link.clone();
        resultado.commit = Some(remoto.mesclar(&publicacao.publicado.branch, &forja)?);
        let _ = fs::write(
            pasta_saida.join("publicados.txt"),
            remoto.arquivos()?.join("\n") + "\n",
        );

        // 4. Sincronizar numa base nova (outro analista, outro clone do
        // repositório de docs, o mesmo código).
        let clone_b = remoto.clonar(&raiz, "docs-b")?;
        let b = cadastrar_projeto(
            &raiz,
            &codigo,
            "dados-b",
            "base-b",
            ciclo::destino(esperada, &remoto, &clone_b),
        )?;
        let sincronizada = ciclo::sincronizar(&b.ambiente, &f)?;
        let i = &sincronizada.importacao;
        resultado.importados = i.novos + i.alterados + i.inalterados;

        // 5. Analisar de novo, na base B.
        let analise_b = criar_analise(&b.ambiente, &item_b)?;
        let (mcp, settings) = preparar_claude_em(&raiz.join("claude-b"), &b, &opcoes.cba)?;
        let preparado = Preparado {
            projeto: b,
            pedido: format!("Inicie a análise {} pelo ctower", analise_b.id),
            mcp,
            settings,
            sessao: None,
        };
        let saida = rodar_claude_como(&preparado, opcoes, pasta_saida, "claude-2", true)?;
        resultado.segunda = Some(ciclo::saida_da_transcricao(&saida));
        resultado.uso = Some(ciclo::uso_da_documentacao(
            &saida,
            &esperada.repositorio,
            resultado.commit.as_deref().unwrap_or(""),
        ));
        let estado = preparado.projeto.ambiente.kb.caminho(
            &cba_core::analises::Analises::arquivo_de_estado(&analise_b.id),
        );
        let bytes = fs::read(&estado)
            .map_err(|e| format!("estado da análise {} não encontrado: {e}", analise_b.id))?;
        let _ = fs::write(pasta_saida.join("estado.json"), &bytes);
        let analise: Analise = serde_json::from_slice(&bytes)
            .map_err(|e| format!("estado da análise ilegível: {e}"))?;
        copiar_documento_como(
            &raiz.join("base-b"),
            &analise,
            &pasta_saida.join("documento.md"),
        );
        Ok((analise, item_b))
    };
    match rodar() {
        Ok((analise, item)) => match escopo(gabarito) {
            Ok(escopo) => execucao.avaliacao = Some(avaliar(&analise, &item, gabarito, &escopo)),
            Err(e) => execucao.erro = Some(e),
        },
        Err(e) => execucao.erro = Some(e),
    }
    let copia = raiz.join("faturamento");
    if copia.is_dir() {
        match pastas::diferencas(&projeto, &copia) {
            Ok(d) => execucao.arquivos_alterados = d,
            Err(e) => {
                execucao
                    .erro
                    .get_or_insert(format!("comparação da cópia: {e}"));
            }
        }
    }
    execucao.claude = match (&resultado.primeira, &resultado.segunda) {
        (Some(a), Some(b)) => Some(a.somar(b)),
        (a, b) => a.clone().or_else(|| b.clone()),
    };
    execucao.publicacao = Some(resultado);
    manter_se_pedido(temporaria, opcoes, pasta_saida);
    execucao
}
