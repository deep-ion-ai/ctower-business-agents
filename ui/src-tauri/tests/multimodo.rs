//! Testes do executável `cba` nos modos de linha de comando, rodando o binário de
//! verdade com a entrada e a saída em pipes, como o cliente de IA faz.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

const CBA: &str = env!("CARGO_BIN_EXE_cba");

/// Meta do hook (issue #3): terminar em menos de 200 ms.
const LIMITE_HOOK: Duration = Duration::from_millis(200);

fn raiz_do_repositorio() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn ler_json(caminho: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(caminho).unwrap()).unwrap()
}

fn versao_do_workspace() -> String {
    let texto = std::fs::read_to_string(raiz_do_repositorio().join("Cargo.toml")).unwrap();
    let cargo: toml::Table = texto.parse().unwrap();
    cargo["workspace"]["package"]["version"]
        .as_str()
        .unwrap()
        .to_string()
}

/// Roda o `cba` com o diretório de dados em `dados` e `entrada` na entrada padrão.
fn rodar(args: &[&str], entrada: &[u8], dados: &Path) -> Output {
    let mut filho = Command::new(CBA)
        .args(args)
        .env("CBA_DATA_DIR", dados)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("executar o cba");
    filho.stdin.take().unwrap().write_all(entrada).unwrap();
    filho.wait_with_output().unwrap()
}

fn texto(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn version_igual_a_do_workspace() {
    let dados = tempfile::tempdir().unwrap();
    let saida = rodar(&["--version"], b"", dados.path());
    assert!(saida.status.success(), "{saida:?}");
    let versao = versao_do_workspace();
    assert_eq!(texto(&saida.stdout).trim(), format!("cba {versao}"));

    // A UI mostra a versão do tauri.conf.json; precisa ser a mesma.
    let conf = ler_json(&raiz_do_repositorio().join("ui/src-tauri/tauri.conf.json"));
    assert_eq!(conf["version"], versao.as_str());
}

#[test]
fn identificador_do_app_e_o_do_diretorio_de_dados() {
    let conf = ler_json(&raiz_do_repositorio().join("ui/src-tauri/tauri.conf.json"));
    assert_eq!(conf["identifier"], cba_core::IDENTIFICADOR);
    assert_eq!(conf["productName"], cba_core::PRODUTO);
    assert_eq!(conf["mainBinaryName"], "cba");
}

#[test]
fn hook_session_start_responde_rapido_e_nao_loga_o_conteudo() {
    let dados = tempfile::tempdir().unwrap();
    let entrada = json!({
        "session_id": "sessao-secreta-123",
        "transcript_path": "/tmp/transcricao.jsonl",
        "cwd": "/tmp/sistema-ficticio",
        "hook_event_name": "SessionStart",
        "source": "startup"
    })
    .to_string();

    // A primeira execução aquece o cache de disco do sistema; as seguintes são medidas.
    let saida = rodar(&["hook", "SessionStart"], entrada.as_bytes(), dados.path());
    assert_eq!(saida.status.code(), Some(0), "{saida:?}");
    assert_eq!(texto(&saida.stdout).trim(), "{}");

    let mut tempos = Vec::new();
    for _ in 0..5 {
        let inicio = Instant::now();
        let saida = rodar(&["hook", "SessionStart"], entrada.as_bytes(), dados.path());
        tempos.push(inicio.elapsed());
        assert_eq!(saida.status.code(), Some(0), "{saida:?}");
        assert_eq!(texto(&saida.stdout).trim(), "{}");
    }
    tempos.sort();
    let mediana = tempos[tempos.len() / 2];
    assert!(
        mediana < LIMITE_HOOK,
        "hook levou {mediana:?} (mediana; tempos: {tempos:?})"
    );

    // O log registra o evento, não o JSON recebido.
    let logs: String = std::fs::read_dir(dados.path().join("logs"))
        .expect("pasta de logs")
        .map(|e| std::fs::read_to_string(e.unwrap().path()).unwrap())
        .collect();
    assert!(logs.contains("SessionStart"), "{logs}");
    assert!(!logs.contains("sessao-secreta-123"), "{logs}");
    assert!(!logs.contains("sistema-ficticio"), "{logs}");
}

#[test]
fn hook_com_entrada_invalida_nao_bloqueia() {
    let dados = tempfile::tempdir().unwrap();
    let saida = rodar(&["hook", "PreToolUse"], b"isto nao e json", dados.path());
    assert_eq!(saida.status.code(), Some(0), "{saida:?}");
    assert_eq!(texto(&saida.stdout).trim(), "{}");
}

fn fixture() -> PathBuf {
    raiz_do_repositorio().join("fixtures/faturamento-exemplo")
}

/// Diretório de dados com o sistema fictício cadastrado e a base em `raiz/base`
/// (sem o sistema dentro: o servidor cria na primeira vez).
fn preparar_dados(raiz: &Path) -> cba_core::dados::DiretorioDados {
    let dados = cba_core::dados::DiretorioDados::em(raiz.join("dados"));
    let sistema = cba_index::Sistema::novo("Faturamento Exemplo", [fixture()]).unwrap();
    cba_index::Cadastro::em(dados.sistemas())
        .cadastrar(&sistema)
        .unwrap();
    cba_kb::Base::criar(raiz.join("base")).unwrap();
    cba_core::config::Config {
        base: Some(raiz.join("base")),
    }
    .gravar(&dados)
    .unwrap();
    dados
}

/// Inicia o `cba mcp` em `pasta`, como o cliente de IA faz.
fn iniciar_mcp(pasta: &Path, dados: &Path) -> std::process::Child {
    iniciar_mcp_do_sistema(pasta, dados, None)
}

/// Inicia o `cba mcp` em `pasta`, com `CBA_SISTEMA` quando informado (como o
/// preparo do ambiente registra o servidor).
fn iniciar_mcp_do_sistema(
    pasta: &Path,
    dados: &Path,
    sistema: Option<&str>,
) -> std::process::Child {
    let mut comando = Command::new(CBA);
    comando.env_remove("CBA_SISTEMA");
    if let Some(s) = sistema {
        comando.env("CBA_SISTEMA", s);
    }
    comando
        .arg("mcp")
        .current_dir(pasta)
        .env("CBA_DATA_DIR", dados)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("executar o cba mcp")
}

/// Envia as mensagens (uma por linha), fecha a entrada e devolve as respostas.
fn conversar(mut filho: std::process::Child, mensagens: &[Value]) -> Vec<Value> {
    let entrada: String = mensagens.iter().map(|m| format!("{m}\n")).collect();
    filho
        .stdin
        .take()
        .unwrap()
        .write_all(entrada.as_bytes())
        .unwrap();
    let saida = filho.wait_with_output().unwrap();
    assert_eq!(saida.status.code(), Some(0), "{saida:?}");
    texto(&saida.stdout)
        .lines()
        .map(|l| serde_json::from_str(l).expect("cada linha é um JSON"))
        .collect()
}

fn inicio_da_conversa() -> Vec<Value> {
    vec![
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": {"name": "teste", "version": "0"}
        }}),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
    ]
}

fn logs(dados: &Path) -> String {
    std::fs::read_dir(dados.join("logs"))
        .expect("pasta de logs")
        .map(|e| std::fs::read_to_string(e.unwrap().path()).unwrap())
        .collect()
}

/// De ponta a ponta, como o Claude Code: o processo começa na pasta do sistema,
/// inicializa, lista as ferramentas e chama uma.
#[test]
fn mcp_de_ponta_a_ponta_na_pasta_do_sistema() {
    let dir = tempfile::tempdir().unwrap();
    let dados = preparar_dados(dir.path());
    // Uma análise criada pelo app antes da sessão.
    let ambiente = cba_core::ambiente::Ambiente::do_diretorio(&fixture(), &dados).unwrap();
    let analise = ambiente
        .analises
        .criar(cba_core::analises::NovaAnalise {
            fluxo: cba_core::analise::Fluxo::Perguntar,
            titulo: "Multa por atraso secreta".into(),
            entrada: "Como é calculada a multa por atraso de pagamento?".into(),
            analista: None,
            base: None,
        })
        .unwrap();

    let mut mensagens = inicio_da_conversa();
    mensagens.push(
        json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {
            "name": "listar_analises", "arguments": {}
        }}),
    );
    mensagens.push(
        json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {
            "name": "listar_analises", "arguments": {"estado": "inexistente"}
        }}),
    );
    mensagens.push(json!({"jsonrpc": "2.0", "id": 5, "method": "inexistente"}));
    let respostas = conversar(iniciar_mcp(&fixture(), dados.raiz()), &mensagens);
    assert_eq!(respostas.len(), 5, "{respostas:?}");

    assert_eq!(respostas[0]["id"], 1);
    assert_eq!(respostas[0]["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(respostas[0]["result"]["serverInfo"]["name"], "cba");
    assert_eq!(
        respostas[0]["result"]["serverInfo"]["version"],
        versao_do_workspace().as_str()
    );

    let ferramentas: Vec<&str> = respostas[1]["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["name"].as_str().unwrap())
        .collect();
    assert!(ferramentas.contains(&"listar_analises"), "{ferramentas:?}");

    let resultado = &respostas[2]["result"];
    assert_eq!(resultado["isError"], false, "{resultado}");
    assert_eq!(
        resultado["structuredContent"]["sistema"]["id"],
        "faturamento-exemplo"
    );
    assert_eq!(
        resultado["structuredContent"]["analises"][0]["id"],
        analise.id.to_string()
    );

    assert_eq!(respostas[3]["result"]["isError"], true);
    assert_eq!(respostas[4]["error"]["code"], -32601);

    // O log registra as ferramentas chamadas, não o conteúdo nem os caminhos.
    let logs = logs(dados.raiz());
    assert!(logs.contains("listar_analises"), "{logs}");
    assert!(!logs.contains("secreta"), "{logs}");
    assert!(!logs.contains("faturamento"), "{logs}");
}

/// Processo iniciado fora de uma pasta de sistema cadastrado: conecta e lista as
/// ferramentas, e a chamada responde com um erro que diz o que fazer.
#[test]
fn mcp_fora_de_sistema_responde_erro_claro() {
    let dir = tempfile::tempdir().unwrap();
    let dados = preparar_dados(dir.path());
    let fora = tempfile::tempdir().unwrap();
    let mut mensagens = inicio_da_conversa();
    mensagens.push(
        json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {
            "name": "listar_analises"
        }}),
    );
    let respostas = conversar(iniciar_mcp(fora.path(), dados.raiz()), &mensagens);
    assert_eq!(respostas.len(), 3, "{respostas:?}");
    assert!(respostas[0]["result"]["serverInfo"].is_object());
    assert!(
        !respostas[1]["result"]["tools"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let resultado = &respostas[2]["result"];
    assert_eq!(resultado["isError"], true, "{resultado}");
    let mensagem = resultado["content"][0]["text"].as_str().unwrap();
    assert!(
        mensagem.contains("não está na pasta de um sistema cadastrado"),
        "{mensagem}"
    );
}

/// Sessão fora da pasta do sistema, com o `CBA_SISTEMA` que o preparo grava (como
/// numa worktree ou no Desktop): o sistema vem da variável, a evidência válida é
/// registrada e a que não confere volta como erro da ferramenta, com o motivo.
#[test]
fn mcp_com_cba_sistema_registra_afirmacao_e_recusa_evidencia() {
    let dir = tempfile::tempdir().unwrap();
    let dados = preparar_dados(dir.path());
    let ambiente = cba_core::ambiente::Ambiente::do_diretorio(&fixture(), &dados).unwrap();
    let analise = ambiente
        .analises
        .criar(cba_core::analises::NovaAnalise {
            fluxo: cba_core::analise::Fluxo::Perguntar,
            titulo: "Multa por atraso".into(),
            entrada: "Como é calculada a multa por atraso de pagamento?".into(),
            analista: None,
            base: None,
        })
        .unwrap();
    let afirmar = |id: i64, trecho: &str| {
        json!({"jsonrpc": "2.0", "id": id, "method": "tools/call", "params": {
            "name": "afirmar", "arguments": {
                "analise": analise.id,
                "secao": "regras_encontradas",
                "texto": "A multa por atraso é de 2% sobre o valor original.",
                "confianca": "confirmado",
                "evidencias": [{
                    "arquivo": "db/02_parametros.sql",
                    "linha_inicial": 6,
                    "linha_final": 7,
                    "trecho": trecho
                }]
            }
        }})
    };
    let mut mensagens = inicio_da_conversa();
    mensagens.push(afirmar(3, "'PERC_MULTA_ATRASO', 2.0000"));
    mensagens.push(afirmar(4, "'PERC_MULTA_ATRASO', 9.9999"));
    let fora = tempfile::tempdir().unwrap();
    let respostas = conversar(
        iniciar_mcp_do_sistema(fora.path(), dados.raiz(), Some("faturamento-exemplo")),
        &mensagens,
    );
    assert_eq!(respostas.len(), 4, "{respostas:?}");
    let ferramentas: Vec<&str> = respostas[1]["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["name"].as_str().unwrap())
        .collect();
    for nome in ["afirmar", "perguntar", "registrar_sugestao", "propor_termo"] {
        assert!(ferramentas.contains(&nome), "{ferramentas:?}");
    }

    let ok = &respostas[2]["result"];
    assert_eq!(ok["isError"], false, "{ok}");
    assert_eq!(ok["structuredContent"]["afirmacao"], 1);
    let recusa = &respostas[3]["result"];
    assert_eq!(recusa["isError"], true, "{recusa}");
    let texto = recusa["content"][0]["text"].as_str().unwrap();
    assert!(texto.contains("não aparece nessas linhas"), "{texto}");

    let estado = ambiente.analises.carregar(&analise.id).unwrap();
    assert_eq!(estado.afirmacoes.len(), 1);
    let doc = std::fs::read_to_string(ambiente.kb.caminho(&estado.documento)).unwrap();
    assert!(doc.contains("**[confirmado]** A multa por atraso"), "{doc}");

    // O log não guarda o conteúdo citado.
    let logs = logs(dados.raiz());
    assert!(logs.contains("afirmar"), "{logs}");
    assert!(!logs.contains("PERC_MULTA_ATRASO"), "{logs}");
}

/// Duas sessões ao mesmo tempo, com a base ainda sem o sistema: as duas criam o
/// sistema na base sem se atrapalhar e respondem normalmente.
#[test]
fn dois_processos_mcp_simultaneos() {
    let dir = tempfile::tempdir().unwrap();
    let dados = preparar_dados(dir.path());
    let mut mensagens = inicio_da_conversa();
    mensagens.push(
        json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {
            "name": "listar_analises"
        }}),
    );
    let a = iniciar_mcp(&fixture(), dados.raiz());
    let b = iniciar_mcp(&fixture().join("db"), dados.raiz());
    let (ra, rb) = std::thread::scope(|s| {
        let ta = s.spawn(|| conversar(a, &mensagens));
        let tb = s.spawn(|| conversar(b, &mensagens));
        (ta.join().unwrap(), tb.join().unwrap())
    });
    for r in [ra, rb] {
        assert_eq!(r[2]["result"]["isError"], false, "{r:?}");
        assert_eq!(
            r[2]["result"]["structuredContent"]["sistema"]["id"],
            "faturamento-exemplo"
        );
    }
    let base = cba_kb::Base::abrir(dir.path().join("base")).unwrap();
    let sistema = base.sistema("faturamento-exemplo").unwrap();
    assert_eq!(
        sistema.ler_sistema().unwrap().frontmatter.nome,
        "Faturamento Exemplo"
    );
}

#[test]
fn argumentos_invalidos_saem_com_codigo_1() {
    let dados = tempfile::tempdir().unwrap();
    for args in [&["hook"][..], &["desconhecido"][..]] {
        let saida = rodar(args, b"", dados.path());
        assert_eq!(saida.status.code(), Some(1), "{args:?}: {saida:?}");
        assert!(saida.stdout.is_empty());
    }
}
