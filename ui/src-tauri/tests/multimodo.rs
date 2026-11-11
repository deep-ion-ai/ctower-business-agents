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

#[test]
fn mcp_initialize_e_tools_list() {
    let dados = tempfile::tempdir().unwrap();
    let entrada = [
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": {"name": "teste", "version": "0"}
        }}),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
        json!({"jsonrpc": "2.0", "id": 3, "method": "inexistente"}),
    ]
    .iter()
    .map(|m| format!("{m}\n"))
    .collect::<String>();

    let saida = rodar(&["mcp"], entrada.as_bytes(), dados.path());
    assert_eq!(saida.status.code(), Some(0), "{saida:?}");
    let respostas: Vec<Value> = texto(&saida.stdout)
        .lines()
        .map(|l| serde_json::from_str(l).expect("cada linha é um JSON"))
        .collect();
    assert_eq!(respostas.len(), 3, "{respostas:?}");

    assert_eq!(respostas[0]["id"], 1);
    assert_eq!(respostas[0]["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(respostas[0]["result"]["serverInfo"]["name"], "cba");
    assert_eq!(
        respostas[0]["result"]["serverInfo"]["version"],
        versao_do_workspace().as_str()
    );
    assert_eq!(respostas[1]["id"], 2);
    assert_eq!(respostas[1]["result"]["tools"], json!([]));
    assert_eq!(respostas[2]["error"]["code"], -32601);
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
