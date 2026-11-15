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
        "cwd": "/tmp/projeto-ficticio",
        "hook_event_name": "SessionStart",
        "source": "startup"
    })
    .to_string();

    // A primeira execução aquece o cache de disco do projeto; as seguintes são medidas.
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
    assert!(!logs.contains("projeto-ficticio"), "{logs}");
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
/// (sem o projeto dentro: o servidor cria na primeira vez).
fn preparar_dados(raiz: &Path) -> cba_core::dados::DiretorioDados {
    let dados = cba_core::dados::DiretorioDados::em(raiz.join("dados"));
    let projeto = cba_index::Projeto::novo("Faturamento Exemplo", [fixture()]).unwrap();
    cba_index::Cadastro::em(dados.projetos())
        .cadastrar(&projeto)
        .unwrap();
    cba_kb::Base::criar(raiz.join("base")).unwrap();
    cba_core::config::Config {
        base: Some(raiz.join("base")),
        ..Default::default()
    }
    .gravar(&dados)
    .unwrap();
    dados
}

/// Inicia o `cba mcp` em `pasta`, como o cliente de IA faz.
fn iniciar_mcp(pasta: &Path, dados: &Path) -> std::process::Child {
    iniciar_mcp_do_projeto(pasta, dados, None)
}

/// Inicia o `cba mcp` em `pasta`, com `CBA_PROJETO` quando informado (como o
/// preparo do ambiente registra o servidor).
fn iniciar_mcp_do_projeto(
    pasta: &Path,
    dados: &Path,
    projeto: Option<&str>,
) -> std::process::Child {
    let mut comando = Command::new(CBA);
    comando.env_remove("CBA_PROJETO");
    if let Some(s) = projeto {
        comando.env("CBA_PROJETO", s);
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

/// De ponta a ponta, como o Claude Code: o processo começa na pasta do projeto,
/// inicializa, lista as ferramentas e chama uma.
#[test]
fn mcp_de_ponta_a_ponta_na_pasta_do_projeto() {
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
        resultado["structuredContent"]["projeto"]["id"],
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

/// Processo iniciado fora de uma pasta de projeto cadastrado: conecta e lista as
/// ferramentas, e a chamada responde com um erro que diz o que fazer.
#[test]
fn mcp_fora_de_projeto_responde_erro_claro() {
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
        mensagem.contains("não está na pasta de um projeto cadastrado"),
        "{mensagem}"
    );
}

/// Sessão fora da pasta do projeto, com o `CBA_PROJETO` que o preparo grava (como
/// numa worktree ou no Desktop): o projeto vem da variável, a evidência válida é
/// registrada e a que não confere volta como erro da ferramenta, com o motivo.
#[test]
fn mcp_com_cba_projeto_registra_afirmacao_e_recusa_evidencia() {
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
        iniciar_mcp_do_projeto(fora.path(), dados.raiz(), Some("faturamento-exemplo")),
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

/// Duas sessões ao mesmo tempo, com a base ainda sem o projeto: as duas criam o
/// projeto na base sem se atrapalhar e respondem normalmente.
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
            r[2]["result"]["structuredContent"]["projeto"]["id"],
            "faturamento-exemplo"
        );
    }
    let base = cba_kb::Base::abrir(dir.path().join("base")).unwrap();
    let projeto = base.projeto("faturamento-exemplo").unwrap();
    assert_eq!(
        projeto.ler_projeto().unwrap().frontmatter.nome,
        "Faturamento Exemplo"
    );
}

fn gabarito() -> Value {
    let caminho = raiz_do_repositorio().join("fixtures/gabaritos/faturamento-exemplo.yaml");
    let gabarito: serde_norway::Value =
        serde_norway::from_str(&std::fs::read_to_string(caminho).unwrap()).unwrap();
    serde_json::to_value(gabarito).unwrap()
}

/// Item de uma lista do gabarito pelo `id`.
fn do_gabarito(lista: &str, id: &str) -> Value {
    gabarito()[lista]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == id)
        .cloned()
        .unwrap_or_else(|| panic!("{id} em {lista} no gabarito"))
}

/// Regra (ou inconsistência) do gabarito do sistema fictício, como JSON
/// (enunciado e evidências no formato dos argumentos de `afirmar`).
fn regra_do_gabarito(id: &str) -> Value {
    let lista = if id.starts_with("INC-") {
        "inconsistencias"
    } else {
        "regras"
    };
    do_gabarito(lista, id)
}

fn chamada(id: i64, nome: &str, argumentos: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "method": "tools/call", "params": {
        "name": nome, "arguments": argumentos
    }})
}

/// O critério do M1 pelo executável: uma pergunta ao sistema fictício, de
/// `iniciar_analise` a `concluir_analise`, gera o documento padronizado na base,
/// com a evidência do gabarito validada; uma sessão nova retoma a análise com
/// todo o estado.
#[test]
fn mcp_pergunta_ao_projeto_do_inicio_ao_documento_na_base() {
    let dir = tempfile::tempdir().unwrap();
    let dados = preparar_dados(dir.path());
    let regra = regra_do_gabarito("RN-FAT-001");
    let evidencias = regra["evidencias"].clone();
    assert_eq!(evidencias.as_array().unwrap().len(), 2);

    let mut mensagens = inicio_da_conversa();
    mensagens.push(chamada(
        3,
        "iniciar_analise",
        json!({
            "fluxo": "perguntar",
            "entrada": "Como é calculada a multa por atraso de pagamento?",
            "titulo": "Multa por atraso"
        }),
    ));
    mensagens.push(chamada(
        4,
        "afirmar",
        json!({
            "analise": "A-0001",
            "secao": "resposta",
            "texto": "A multa por atraso é única, de 2% sobre o valor original da fatura, cobrada depois da carência.",
            "confianca": "confirmado",
            "evidencias": evidencias
        }),
    ));
    mensagens.push(chamada(
        5,
        "afirmar",
        json!({
            "analise": "A-0001",
            "secao": "regras_encontradas",
            "texto": format!("{}: {}", regra["titulo"].as_str().unwrap(), regra["enunciado"].as_str().unwrap()),
            "confianca": "confirmado",
            "evidencias": [evidencias[0]]
        }),
    ));
    mensagens.push(chamada(
        6,
        "afirmar",
        json!({
            "analise": "A-0001",
            "secao": "exemplo",
            "texto": "Fatura de R$ 1.000,00 paga depois da carência: multa de 2% = R$ 20,00.",
            "confianca": "inferido",
            "evidencias": [evidencias[0]]
        }),
    ));
    mensagens.push(chamada(
        7,
        "perguntar",
        json!({
            "analise": "A-0001",
            "texto": "O lote de cobrança também deveria ler o percentual da tabela de parâmetros?"
        }),
    ));
    mensagens.push(chamada(8, "concluir_analise", json!({"analise": "A-0001"})));
    mensagens.push(chamada(9, "concluir_analise", json!({"analise": "A-0099"})));
    let respostas = conversar(iniciar_mcp(&fixture(), dados.raiz()), &mensagens);
    assert_eq!(respostas.len(), 9, "{respostas:?}");

    let ferramentas: Vec<&str> = respostas[1]["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["name"].as_str().unwrap())
        .collect();
    for nome in ["iniciar_analise", "retomar_analise", "concluir_analise"] {
        assert!(ferramentas.contains(&nome), "{ferramentas:?}");
    }
    for r in &respostas[2..8] {
        assert_eq!(r["result"]["isError"], false, "{r}");
    }

    // iniciar_analise: o roteiro vem no texto que o agente lê.
    let iniciar = &respostas[2]["result"];
    assert_eq!(iniciar["structuredContent"]["analise"], "A-0001");
    let roteiro = iniciar["content"][0]["text"].as_str().unwrap();
    assert!(roteiro.contains("Análise A-0001 criada"), "{roteiro}");
    assert!(
        roteiro.contains("# Roteiro da análise A-0001: Multa por atraso"),
        "{roteiro}"
    );
    assert!(roteiro.contains("concluir_analise"), "{roteiro}");
    // O roteiro de perguntar ao sistema, com as variáveis preenchidas.
    assert!(
        roteiro.contains(
            "Fluxo: pergunta ao sistema. Documento na base: `faturamento-exemplo/analises/A-0001.md`"
        ),
        "{roteiro}"
    );
    assert!(
        roteiro.contains(
            "## Pergunta do analista\n\n> Como é calculada a multa por atraso de pagamento?"
        ),
        "{roteiro}"
    );
    for secao in [
        "`resposta` (Resposta) (obrigatória)",
        "`regras_encontradas`",
        "`exemplo`",
    ] {
        assert!(roteiro.contains(secao), "{secao}: {roteiro}");
    }
    assert!(roteiro.contains("## Contexto da base"), "{roteiro}");
    assert!(!roteiro.contains("{{"), "{roteiro}");

    // concluir_analise: o caminho que o agente cita no chat.
    let concluir = &respostas[7]["result"];
    let documento = "faturamento-exemplo/analises/A-0001.md";
    assert_eq!(concluir["structuredContent"]["documento"], documento);
    assert_eq!(concluir["structuredContent"]["estado"], "concluida");
    let texto = concluir["content"][0]["text"].as_str().unwrap();
    assert!(texto.contains(documento), "{texto}");
    assert!(texto.contains("Perguntas em aberto (1)"), "{texto}");

    // Identificador desconhecido: erro da ferramenta, com instrução.
    let desconhecida = &respostas[8]["result"];
    assert_eq!(desconhecida["isError"], true);
    let texto = desconhecida["content"][0]["text"].as_str().unwrap();
    assert!(texto.contains("A-0099 não existe"), "{texto}");

    // O documento padronizado, na base.
    let markdown = std::fs::read_to_string(dir.path().join("base").join(documento)).unwrap();
    let (frontmatter, corpo) = markdown
        .strip_prefix("---\n")
        .and_then(|m| m.split_once("\n---\n"))
        .expect("frontmatter YAML");
    let frontmatter: serde_norway::Value = serde_norway::from_str(frontmatter).unwrap();
    assert_eq!(frontmatter["tipo"].as_str(), Some("pergunta"));
    assert_eq!(frontmatter["id"].as_str(), Some("A-0001"));
    assert_eq!(frontmatter["titulo"].as_str(), Some("Multa por atraso"));
    assert_eq!(frontmatter["projeto"].as_str(), Some("faturamento-exemplo"));
    assert_eq!(frontmatter["status"].as_str(), Some("em-revisao"));
    let secoes: Vec<&str> = corpo.lines().filter(|l| l.starts_with("## ")).collect();
    assert_eq!(
        secoes,
        [
            "## Pergunta",
            "## Resposta",
            "## Regras encontradas",
            "## Exemplo",
            "## Perguntas em aberto",
            "## Sugestões de alteração",
            "## Evidências"
        ],
        "{corpo}"
    );
    assert!(corpo.contains("# A-0001 · Multa por atraso"), "{corpo}");
    assert!(
        corpo.contains("## Pergunta\n\nComo é calculada a multa por atraso de pagamento?"),
        "{corpo}"
    );
    assert!(
        corpo.contains("- **[confirmado]** A multa por atraso é única, de 2% sobre o valor original da fatura, cobrada depois da carência.[^1][^2]"),
        "{corpo}"
    );
    // O mesmo trecho citado de novo usa a mesma nota.
    assert!(
        corpo.contains("- **[confirmado]** Multa por atraso vem de parâmetro: Passada a carência"),
        "{corpo}"
    );
    assert!(corpo.contains("arredondada em 2 casas.[^1]\n"), "{corpo}");
    assert!(
        corpo.contains("1. O lote de cobrança também deveria ler o percentual"),
        "{corpo}"
    );
    assert!(
        corpo.contains("[^1]: `db/03_pkg_cobranca.sql:71-72` · hash `"),
        "{corpo}"
    );
    assert!(
        corpo.contains("[^2]: `db/02_parametros.sql:6-7`"),
        "{corpo}"
    );
    assert_eq!(corpo.matches("[^1]:").count(), 1, "{corpo}");
    // Cada seção que o fluxo preenche tem conteúdo.
    for secao in [
        "Pergunta",
        "Resposta",
        "Regras encontradas",
        "Exemplo",
        "Perguntas em aberto",
        "Evidências",
    ] {
        let (_, depois) = corpo.split_once(&format!("## {secao}\n\n")).unwrap();
        assert!(
            !depois.starts_with("_Nada registrado._"),
            "{secao}: {corpo}"
        );
        assert!(!depois.starts_with("_Nenhuma"), "{secao}: {corpo}");
    }
    assert!(
        corpo.contains("- **[inferido]** Fatura de R$ 1.000,00 paga depois da carência"),
        "{corpo}"
    );
    // Caminhos relativos: nada da máquina no documento.
    assert!(
        !markdown.contains(&*dir.path().to_string_lossy()),
        "{markdown}"
    );
    assert!(
        dir.path().join("base/_templates/pergunta.md").is_file(),
        "templates padrão copiados para a base"
    );
    // Roteiro revisado e prompts prontos: a cópia editável, na base.
    assert!(
        dir.path()
            .join("base/_templates/roteiros/perguntar.md")
            .is_file()
    );
    for fluxo in ["explicar", "impacto"] {
        assert!(
            dir.path()
                .join(format!("base/_templates/roteiros/{fluxo}.md"))
                .is_file(),
            "{fluxo}"
        );
    }
    assert!(
        dir.path()
            .join("base/_templates/prompts/aprofundar.md")
            .is_file()
    );

    // Os prompts prontos da análise, como o comando `prompts_prontos` da janela.
    let prontos = cba_tools::prompts::da_analise(&dados, "faturamento-exemplo", "A-0001").unwrap();
    assert_eq!(prontos.len(), 6);
    for p in &prontos {
        assert!(p.texto.contains("A-0001"), "{p:?}");
        assert!(!p.texto.contains("{{"), "{p:?}");
    }

    // Sessão nova (outro processo): retomar_analise devolve todo o estado.
    let mut mensagens = inicio_da_conversa();
    mensagens.push(chamada(3, "retomar_analise", json!({"analise": "A-0001"})));
    let respostas = conversar(iniciar_mcp(&fixture().join("db"), dados.raiz()), &mensagens);
    let retomar = &respostas[2]["result"];
    assert_eq!(retomar["isError"], false, "{retomar}");
    let estado = &retomar["structuredContent"];
    assert_eq!(estado["estado"], "concluida");
    assert_eq!(
        estado["entrada"],
        "Como é calculada a multa por atraso de pagamento?"
    );
    assert_eq!(estado["afirmacoes"].as_array().unwrap().len(), 3);
    assert_eq!(
        estado["afirmacoes"][0]["evidencias"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(estado["afirmacoes"][0]["evidencias"][0]["linhas"], "71-72");
    assert_eq!(estado["perguntas"][0]["estado"], "aberta");
    let texto = retomar["content"][0]["text"].as_str().unwrap();
    assert!(
        texto.contains("Próximo passo: A rodada foi concluída"),
        "{texto}"
    );
    assert!(texto.contains("`db/02_parametros.sql:6-7`"), "{texto}");

    // O log registra as ferramentas, não o conteúdo.
    let logs = logs(dados.raiz());
    assert!(logs.contains("concluir_analise"), "{logs}");
    assert!(!logs.contains("PERC_MULTA_ATRASO"), "{logs}");
    assert!(!logs.contains("Multa por atraso"), "{logs}");
}

/// Uma afirmação para `afirmar`: seção, texto, confiança e evidências.
type Registro<'a> = (&'a str, String, &'a str, Value);

/// Roda um fluxo pelo executável, como o agente faz: `iniciar_analise` com o
/// fluxo e a entrada, uma afirmação por registro, uma pergunta, uma sugestão e
/// `concluir_analise`. Devolve o texto do roteiro, a resposta de
/// `concluir_analise`, o frontmatter e o corpo do documento.
fn fluxo_de_ponta_a_ponta(
    raiz: &Path,
    dados: &Path,
    fluxo: &str,
    titulo: &str,
    entrada: &str,
    registros: &[Registro<'_>],
) -> (String, Value, serde_norway::Value, String) {
    let mut mensagens = inicio_da_conversa();
    mensagens.push(chamada(
        3,
        "iniciar_analise",
        json!({ "fluxo": fluxo, "entrada": entrada, "titulo": titulo }),
    ));
    let mut id = 4;
    for (secao, texto, confianca, evidencias) in registros {
        mensagens.push(chamada(
            id,
            "afirmar",
            json!({
                "analise": "A-0001",
                "secao": secao,
                "texto": texto,
                "confianca": confianca,
                "evidencias": evidencias
            }),
        ));
        id += 1;
    }
    mensagens.push(chamada(
        id,
        "perguntar",
        json!({
            "analise": "A-0001",
            "texto": "O limite de parcelas deve valer também para órgão público?"
        }),
    ));
    mensagens.push(chamada(
        id + 1,
        "registrar_sugestao",
        json!({
            "analise": "A-0001",
            "arquivo": "frontend/src/telas/parcelamento-form.ts",
            "titulo": "Tela com o mesmo limite de parcelas do backend",
            "trecho_atual": "const MAX_PARCELAS = 12;",
            "trecho_proposto": "const MAX_PARCELAS = 6;",
            "motivo": "A tela oferece até 12 parcelas, mas o backend recusa acima de 6.",
            "impacto": "Pedidos de 7 a 12 parcelas deixam de ser oferecidos e recusados depois.",
            "exemplo": "Pedido de 8 parcelas: hoje passa pela tela e é recusado; depois, a tela não oferece."
        }),
    ));
    mensagens.push(chamada(
        id + 2,
        "concluir_analise",
        json!({"analise": "A-0001"}),
    ));
    let respostas = conversar(iniciar_mcp(&fixture(), dados), &mensagens);
    assert_eq!(respostas.len(), registros.len() + 6, "{respostas:?}");
    for r in &respostas[2..] {
        assert_eq!(r["result"]["isError"], false, "{r}");
    }
    let roteiro = respostas[2]["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(!roteiro.contains("{{"), "{roteiro}");
    assert!(!roteiro.contains("<!--"), "{roteiro}");
    let concluir = respostas.last().unwrap()["result"].clone();
    assert_eq!(concluir["structuredContent"]["estado"], "concluida");
    assert_eq!(
        concluir["structuredContent"]["pendencias"],
        json!([]),
        "{concluir}"
    );
    let documento = concluir["structuredContent"]["documento"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(documento, "faturamento-exemplo/analises/A-0001.md");
    let markdown = std::fs::read_to_string(raiz.join("base").join(&documento)).unwrap();
    let (frontmatter, corpo) = markdown
        .strip_prefix("---\n")
        .and_then(|m| m.split_once("\n---\n"))
        .expect("frontmatter YAML");
    assert!(!markdown.contains(&*raiz.to_string_lossy()), "{markdown}");
    (
        roteiro,
        concluir,
        serde_norway::from_str(frontmatter).unwrap(),
        corpo.to_owned(),
    )
}

/// Confere que o documento tem exatamente as seções do template, na ordem, e que
/// nenhuma ficou vazia.
fn todas_as_secoes_preenchidas(corpo: &str, secoes: &[&str]) {
    let titulos: Vec<&str> = corpo.lines().filter(|l| l.starts_with("## ")).collect();
    let esperados: Vec<String> = secoes.iter().map(|s| format!("## {s}")).collect();
    assert_eq!(titulos, esperados, "{corpo}");
    for secao in secoes {
        let (_, depois) = corpo.split_once(&format!("## {secao}\n\n")).unwrap();
        let conteudo = depois.split("\n## ").next().unwrap();
        assert!(!conteudo.contains("_Nada registrado._"), "{secao}: {corpo}");
        assert!(!conteudo.contains("_Nenhuma"), "{secao}: {corpo}");
        assert!(!conteudo.contains("a definir"), "{secao}: {corpo}");
    }
}

/// O critério do M2 pelo executável (#32): a análise de impacto de uma demanda do
/// gabarito, com registro em todas as seções, gera o documento com todas as
/// seções do template, a premissa do solicitante corrigida em "Demanda como
/// entendida" e a abrangência estimada no campo do documento.
#[test]
fn mcp_analise_de_impacto_do_inicio_ao_documento_na_base() {
    let dir = tempfile::tempdir().unwrap();
    let dados = preparar_dados(dir.path());
    let demanda = do_gabarito("demandas", "D-FAT-001");
    let entrada = demanda["texto"].as_str().unwrap();
    let pf = regra_do_gabarito("RN-FAT-007")["evidencias"].clone();
    let parcelas = regra_do_gabarito("RN-FAT-008")["evidencias"].clone();
    let tela = regra_do_gabarito("INC-FAT-001")["evidencias"].clone();
    let registros: Vec<Registro<'_>> = vec![
        (
            "demanda_como_entendida",
            "Premissa do solicitante: hoje o projeto não parcela fatura nenhuma. Verificação: não confere. O parcelamento já existe para pessoa jurídica e órgão público, em até 6 parcelas (parâmetro MAX_PARCELAS); só pessoa física é recusada.".into(),
            "confirmado",
            json!([pf[0], parcelas[0], parcelas[2]]),
        ),
        (
            "demanda_como_entendida",
            "Demanda como entendida: o parcelamento para pessoa jurídica já está atendido; o que resta é decidir se o limite deve cair de 6 para 3.".into(),
            "inferido",
            json!([parcelas[0]]),
        ),
        (
            "como_e_hoje",
            "Pessoa jurídica e órgão público parcelam de 2 a 6 vezes; pessoa física não parcela.".into(),
            "confirmado",
            json!([pf[0], parcelas[1]]),
        ),
        (
            "funcionalidades",
            "O limite de parcelas do serviço de parcelamento vem do parâmetro MAX_PARCELAS.".into(),
            "confirmado",
            json!([parcelas[0]]),
        ),
        (
            "dados",
            "O parâmetro MAX_PARCELAS, hoje 6, é único para todos os tipos de cliente.".into(),
            "confirmado",
            json!([parcelas[2]]),
        ),
        (
            "integracoes",
            "Nenhuma integração lê o limite de parcelas; o parâmetro é lido só pelo backend.".into(),
            "inferido",
            json!([parcelas[0]]),
        ),
        (
            "documentos_e_telas",
            "A tela de parcelamento tem o limite fixo de 12 parcelas, diferente do backend.".into(),
            "confirmado",
            tela.clone(),
        ),
        (
            "riscos",
            "Reduzir MAX_PARCELAS para 3 afeta também órgão público, porque o parâmetro é único.".into(),
            "inferido",
            json!([parcelas[0], parcelas[2]]),
        ),
        (
            "abrangencia",
            "Pequena: a demanda já está atendida; limitar a 3 muda só o parâmetro e, para alinhar, a constante da tela.".into(),
            "inferido",
            json!([parcelas[2], tela[0]]),
        ),
    ];
    let (roteiro, concluir, frontmatter, corpo) = fluxo_de_ponta_a_ponta(
        dir.path(),
        dados.raiz(),
        "impacto",
        "Parcelamento em 3 vezes para PJ",
        entrada,
        &registros,
    );

    // O roteiro de impacto, com as variáveis preenchidas.
    assert!(
        roteiro.contains("Fluxo: análise de impacto de uma demanda. Documento na base:"),
        "{roteiro}"
    );
    assert!(
        roteiro.contains("**Separe fato de suposição.**"),
        "{roteiro}"
    );
    assert!(roteiro.contains("Premissa do solicitante:"), "{roteiro}");
    for secao in [
        "`demanda_como_entendida` (Demanda como entendida) (obrigatória)",
        "`como_e_hoje` (Como é hoje) (obrigatória)",
        "`abrangencia` (Abrangência) (obrigatória)",
        "`riscos` (Riscos)",
    ] {
        assert!(roteiro.contains(secao), "{secao}: {roteiro}");
    }
    let texto = concluir["content"][0]["text"].as_str().unwrap();
    assert!(
        texto.contains("a demanda como entendida, a abrangência"),
        "{texto}"
    );

    assert_eq!(frontmatter["tipo"].as_str(), Some("analise-impacto"));
    assert_eq!(frontmatter["abrangencia"].as_str(), Some("pequena"));
    todas_as_secoes_preenchidas(
        &corpo,
        &[
            "Demanda como veio",
            "Demanda como entendida",
            "Como é hoje",
            "Funcionalidades",
            "Dados",
            "Integrações",
            "Documentos e telas",
            "Riscos",
            "Abrangência",
            "Perguntas para o solicitante",
            "Sugestões de alteração",
            "Evidências",
        ],
    );
    assert!(
        corpo.contains("## Demanda como veio\n\nPermitir parcelamento da fatura em até 3 vezes"),
        "{corpo}"
    );
    // A premissa falsa da demanda, corrigida no lugar dela.
    let (_, entendida) = corpo.split_once("## Demanda como entendida\n\n").unwrap();
    let entendida = entendida.split("\n## ").next().unwrap();
    assert!(
        entendida.contains("- **[confirmado]** Premissa do solicitante: hoje o projeto não parcela fatura nenhuma. Verificação: não confere."),
        "{corpo}"
    );
    assert!(
        entendida.contains("Demanda como entendida: o parcelamento"),
        "{corpo}"
    );
    assert!(corpo.contains("Estimativa: **pequena**"), "{corpo}");
    assert!(
        corpo.contains("- **[inferido]** Pequena: a demanda já está atendida"),
        "{corpo}"
    );
    assert!(
        corpo.contains("1. O limite de parcelas deve valer também"),
        "{corpo}"
    );
    assert!(
        corpo.contains("Tela com o mesmo limite de parcelas do backend"),
        "{corpo}"
    );
    assert!(
        corpo.contains("`frontend/src/telas/parcelamento-form.ts:19`"),
        "{corpo}"
    );
}

/// O critério do M2 pelo executável (#32): a explicação de uma funcionalidade do
/// gabarito, com registro em todas as seções, gera o documento negocial com todas
/// as seções do template.
#[test]
fn mcp_explicar_funcionalidade_do_inicio_ao_documento_na_base() {
    let dir = tempfile::tempdir().unwrap();
    let dados = preparar_dados(dir.path());
    let funcionalidade = do_gabarito("funcionalidades", "F-FAT-002");
    let nome = funcionalidade["nome"].as_str().unwrap();
    let pf = regra_do_gabarito("RN-FAT-007")["evidencias"].clone();
    let parcelas = regra_do_gabarito("RN-FAT-008")["evidencias"].clone();
    let minimo = regra_do_gabarito("RN-FAT-009")["evidencias"].clone();
    let aberta = regra_do_gabarito("RN-FAT-010")["evidencias"].clone();
    let composicao = regra_do_gabarito("RN-FAT-011")["evidencias"].clone();
    let regra = |id: &str| {
        let r = regra_do_gabarito(id);
        format!(
            "{}: {}",
            r["titulo"].as_str().unwrap(),
            r["enunciado"].as_str().unwrap()
        )
    };
    let registros: Vec<Registro<'_>> = vec![
        (
            "objetivo",
            format!("{nome}: dividir uma fatura em aberto em parcelas."),
            "inferido",
            json!([composicao[0]]),
        ),
        (
            "atores_e_telas",
            "O atendente pede o parcelamento pela tela de parcelamento; o backend valida e gera as parcelas.".into(),
            "confirmado",
            json!([pf[1], pf[0]]),
        ),
        (
            "fluxo_principal",
            "1. O valor parcelado é o valor original mais os encargos na data do pedido, dividido em parcelas iguais.".into(),
            "confirmado",
            json!([composicao[0]]),
        ),
        (
            "fluxos_alternativos",
            "Órgão público parcela sem valor mínimo de parcela.".into(),
            "confirmado",
            json!([minimo[0]]),
        ),
        (
            "excecoes",
            "Fatura que não está aberta é recusada.".into(),
            "confirmado",
            aberta.clone(),
        ),
        ("regras", regra("RN-FAT-007"), "confirmado", pf.clone()),
        ("regras", regra("RN-FAT-008"), "confirmado", parcelas.clone()),
        (
            "dados",
            "Lê os parâmetros MAX_PARCELAS e VALOR_MIN_PARCELA da tabela de parâmetros.".into(),
            "confirmado",
            json!([parcelas[2], minimo[1]]),
        ),
        (
            "integracoes",
            "O cálculo dos encargos é pedido ao serviço de encargos.".into(),
            "confirmado",
            json!([composicao[0]]),
        ),
    ];
    let (roteiro, concluir, frontmatter, corpo) = fluxo_de_ponta_a_ponta(
        dir.path(),
        dados.raiz(),
        "explicar",
        nome,
        "Como funciona o parcelamento de fatura?",
        &registros,
    );
    assert!(
        roteiro.contains("Fluxo: explicação de funcionalidade (documento negocial)."),
        "{roteiro}"
    );
    assert!(
        roteiro.contains("**Siga o caminho do começo ao fim.**"),
        "{roteiro}"
    );
    for secao in [
        "`objetivo` (Objetivo) (obrigatória)",
        "`fluxo_principal` (Fluxo principal) (obrigatória)",
        "`regras` (Regras de negócio) (obrigatória)",
        "`excecoes` (Exceções)",
    ] {
        assert!(roteiro.contains(secao), "{secao}: {roteiro}");
    }
    let texto = concluir["content"][0]["text"].as_str().unwrap();
    assert!(texto.contains("objetivo, regras principais"), "{texto}");
    assert_eq!(frontmatter["tipo"].as_str(), Some("funcionalidade"));
    todas_as_secoes_preenchidas(
        &corpo,
        &[
            "Objetivo",
            "Atores e telas",
            "Fluxo principal",
            "Fluxos alternativos",
            "Exceções",
            "Regras de negócio",
            "Dados lidos e gravados",
            "Integrações",
            "Perguntas em aberto",
            "Sugestões de alteração",
            "Evidências",
        ],
    );
    assert!(corpo.contains("# Parcelamento de fatura"), "{corpo}");
    assert!(
        corpo.contains("- **[confirmado]** Parcelamento só para pessoa jurídica e órgão público:"),
        "{corpo}"
    );
    assert!(
        corpo.contains("Tela com o mesmo limite de parcelas do backend"),
        "{corpo}"
    );
}

/// As ferramentas de consulta pelo executável (#22): sem mapa gravado, a primeira
/// chamada gera o mapa; `referencias` acha quem chama a função de multa, com
/// paginação; `conhecimento` acha o termo gravado na base depois de o servidor
/// subir.
#[test]
fn mcp_ferramentas_de_consulta_de_ponta_a_ponta() {
    let dir = tempfile::tempdir().unwrap();
    let dados = preparar_dados(dir.path());
    let ambiente = cba_core::ambiente::Ambiente::do_diretorio(&fixture(), &dados).unwrap();
    let mapa = dados.projetos().join("faturamento-exemplo/mapa.json");
    assert!(!mapa.exists());

    let mut mensagens = inicio_da_conversa();
    mensagens.push(chamada(
        3,
        "referencias",
        json!({"identificador": "PKG_COBRANCA.FN_CALCULAR_MULTA"}),
    ));
    mensagens.push(chamada(4, "conhecimento", json!({"termo": "carencia"})));
    mensagens.push(chamada(5, "schema", json!({"limite": 1})));
    let respostas = conversar(iniciar_mcp(&fixture(), dados.raiz()), &mensagens);
    assert_eq!(respostas.len(), 5, "{respostas:?}");

    let ferramentas: Vec<&str> = respostas[1]["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["name"].as_str().unwrap())
        .collect();
    for nome in ["conhecimento", "mapa", "estrutura", "schema", "referencias"] {
        assert!(ferramentas.contains(&nome), "{nome}: {ferramentas:?}");
        let f = respostas[1]["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["name"] == nome)
            .unwrap();
        assert_eq!(f["annotations"]["readOnlyHint"], true, "{f}");
    }

    let referencias = &respostas[2]["result"];
    assert_eq!(referencias["isError"], false, "{referencias}");
    let dados_ref = &referencias["structuredContent"];
    assert_eq!(dados_ref["mapa"]["gerado_agora"], true);
    assert_eq!(
        dados_ref["referencias"][0]["arquivo"],
        "backend/src/main/java/exemplo/faturamento/EncargosService.java"
    );
    assert_eq!(dados_ref["referencias"][0]["linha"], 28);
    assert!(
        referencias["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("EncargosService.java:28"),
        "{referencias}"
    );
    assert!(mapa.is_file(), "o mapa gerado fica gravado no cadastro");

    assert_eq!(
        respostas[3]["result"]["structuredContent"]["pagina"]["total"],
        0
    );

    let schema = &respostas[4]["result"]["structuredContent"];
    assert_eq!(schema["pagina"]["quantidade"], 1, "{schema}");
    assert_eq!(schema["pagina"]["proximo_cursor"], 1, "{schema}");

    // Um termo gravado na base depois: um processo novo o encontra, e o mapa
    // gravado é reaproveitado.
    ambiente
        .kb
        .criar(
            &cba_kb::layout::termo("Carência").unwrap(),
            &cba_kb::Documento::novo(
                cba_kb::frontmatter::FrontmatterTermo {
                    tipo: cba_kb::TipoDocumento::Termo,
                    termo: "Carência".into(),
                    sinonimos: vec![],
                    area: Some("cobranca".into()),
                    origem: None,
                },
                "\nDias de atraso sem encargos (DIAS_CARENCIA).\n",
            ),
        )
        .unwrap();
    let mut mensagens = inicio_da_conversa();
    mensagens.push(chamada(3, "conhecimento", json!({"termo": "carencia"})));
    mensagens.push(chamada(
        4,
        "estrutura",
        json!({"arquivo": "db/03_pkg_cobranca.sql"}),
    ));
    let respostas = conversar(iniciar_mcp(&fixture(), dados.raiz()), &mensagens);
    let conhecimento = &respostas[2]["result"]["structuredContent"];
    assert_eq!(
        conhecimento["resultados"][0]["caminho"], "faturamento-exemplo/glossario/carencia.md",
        "{conhecimento}"
    );
    let estrutura = &respostas[3]["result"]["structuredContent"];
    assert_eq!(estrutura["mapa"]["gerado_agora"], false, "{estrutura}");
    assert_eq!(estrutura["arquivo"], "db/03_pkg_cobranca.sql");

    // O log registra as ferramentas, não o conteúdo.
    let logs = logs(dados.raiz());
    assert!(logs.contains("referencias"), "{logs}");
    assert!(!logs.contains("FN_CALCULAR_MULTA"), "{logs}");
    assert!(!logs.contains("carencia"), "{logs}");
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

/// Roda `cba hook <evento>` como o Claude Code, sem herdar o `CLAUDE_PROJECT_DIR`
/// de quem roda os testes.
fn rodar_hook(evento: &str, entrada: &Value, dados: &Path) -> Output {
    let mut filho = Command::new(CBA)
        .args(["hook", evento])
        .env("CBA_DATA_DIR", dados)
        .env_remove("CLAUDE_PROJECT_DIR")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("executar o cba hook");
    filho
        .stdin
        .take()
        .unwrap()
        .write_all(entrada.to_string().as_bytes())
        .unwrap();
    let saida = filho.wait_with_output().unwrap();
    assert_eq!(saida.status.code(), Some(0), "{evento}: {saida:?}");
    saida
}

/// A saída padrão do hook: um objeto JSON só, como o Claude Code espera.
fn saida_do_hook(saida: &Output) -> Value {
    serde_json::from_str(texto(&saida.stdout).trim()).expect("saída do hook em JSON")
}

/// Confere que o hook respondeu `{}` (o cliente segue sem contexto extra).
fn sem_contexto(saida: &Output, evento: &str) {
    assert_eq!(texto(&saida.stdout).trim(), "{}", "{evento}");
}

/// Entrada de exemplo de cada evento, no formato da documentação do Claude Code
/// (code.claude.com/docs/en/hooks.md), na pasta `cwd`.
fn entrada_do_hook(evento: &str, sessao: &str, cwd: &Path, analise: &str) -> Value {
    let mut e = json!({
        "session_id": sessao,
        "transcript_path": "/tmp/projetos/transcricao-secreta.jsonl",
        "cwd": cwd,
        "permission_mode": "default",
        "hook_event_name": evento,
    });
    let extra = match evento {
        "SessionStart" => json!({"source": "startup", "model": "claude-opus-5"}),
        "UserPromptSubmit" => json!({"prompt": "Inicie a análise secreta pelo ctower"}),
        "PreToolUse" => json!({
            "tool_name": "mcp__cba__iniciar_analise",
            "tool_input": {"analise": analise},
            "tool_use_id": "toolu_01ABC"
        }),
        "Notification" => json!({
            "message": "Claude needs your permission",
            "title": "Permission needed",
            "notification_type": "permission_prompt"
        }),
        "Stop" => json!({"stop_hook_active": false, "last_assistant_message": "resposta secreta"}),
        "SessionEnd" => json!({"reason": "other"}),
        _ => unreachable!(),
    };
    for (k, v) in extra.as_object().unwrap() {
        e[k] = v.clone();
    }
    e
}

fn estado_da_sessao(dados: &Path, sessao: &str) -> Value {
    ler_json(&dados.join("sessoes").join(format!("{sessao}.json")))
}

/// Uma sessão inteira na pasta do projeto: cada evento muda o estado da sessão e da
/// análise ligada. O executável responde com código 0: no `SessionStart`, com os
/// limites em `additionalContext` (#31); nos demais, `{}`.
#[test]
fn hook_acompanha_a_sessao_na_pasta_do_projeto() {
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
    let id = analise.id.to_string();
    let sessao = "0b7e2c4a-1d3f-4a5b-9c8d-7e6f5a4b3c2d";
    let cwd = fixture().join("db");

    let esperado = [
        ("SessionStart", "ativa", None),
        ("UserPromptSubmit", "ativa", None),
        ("PreToolUse", "ativa", Some("ativa")),
        (
            "Notification",
            "aguardando-analista",
            Some("aguardando-analista"),
        ),
        ("Stop", "concluida", Some("concluida")),
        ("SessionEnd", "encerrada", Some("encerrada")),
    ];
    for (evento, na_sessao, na_analise) in esperado {
        let saida = rodar_hook(
            evento,
            &entrada_do_hook(evento, sessao, &cwd, &id),
            dados.raiz(),
        );
        if evento == "SessionStart" {
            let json = saida_do_hook(&saida);
            assert_eq!(json["hookSpecificOutput"]["hookEventName"], "SessionStart");
            let contexto = json["hookSpecificOutput"]["additionalContext"]
                .as_str()
                .unwrap();
            assert!(contexto.contains("registrar_sugestao"), "{contexto}");
        } else {
            sem_contexto(&saida, evento);
        }
        let estado = estado_da_sessao(dados.raiz(), sessao);
        assert_eq!(estado["estado"], na_sessao, "{evento}: {estado}");
        assert_eq!(estado["projeto"], "faturamento-exemplo");
        let a = ambiente.analises.carregar(&analise.id).unwrap();
        let s = a.sessoes.iter().find(|s| s.id == sessao);
        assert_eq!(
            s.map(|s| serde_json::to_value(s.estado).unwrap()),
            na_analise.map(Value::from),
            "{evento}"
        );
    }
    let estado = estado_da_sessao(dados.raiz(), sessao);
    assert_eq!(estado["analise"], id.as_str());
    assert_eq!(estado["motivo_fim"], "other");

    // O log não traz o identificador da sessão, caminhos nem conteúdo.
    let logs = logs(dados.raiz());
    assert!(logs.contains("registrado"), "{logs}");
    for proibido in [sessao, "secreta", "faturamento", "A-0001"] {
        assert!(!logs.contains(proibido), "{proibido}: {logs}");
    }
}

/// Com o projeto cadastrado, uma sessão em outra pasta não deixa rastro e o hook
/// segue rápido, assim como o evento mais frequente de uma sessão do projeto.
#[test]
fn hook_fora_de_projeto_nao_grava_e_ambos_os_caminhos_sao_rapidos() {
    let dir = tempfile::tempdir().unwrap();
    let dados = preparar_dados(dir.path());
    let fora = tempfile::tempdir().unwrap();
    let sessao = "9a8b7c6d-5e4f-4a3b-8c2d-1e0f9a8b7c6d";

    for evento in cba_session::hooks::EVENTOS {
        let saida = rodar_hook(
            evento,
            &entrada_do_hook(evento, sessao, fora.path(), "A-0001"),
            dados.raiz(),
        );
        sem_contexto(&saida, evento);
    }
    assert!(!dados.sessoes().exists());

    let mediana = |entrada: &Value| {
        let evento = entrada["hook_event_name"].as_str().unwrap().to_owned();
        let mut tempos: Vec<Duration> = (0..5)
            .map(|_| {
                let inicio = Instant::now();
                rodar_hook(&evento, entrada, dados.raiz());
                inicio.elapsed()
            })
            .collect();
        tempos.sort();
        tempos[tempos.len() / 2]
    };
    let fora_do_projeto = mediana(&entrada_do_hook(
        "PreToolUse",
        sessao,
        fora.path(),
        "A-0001",
    ));
    assert!(
        fora_do_projeto < LIMITE_HOOK,
        "fora do projeto: {fora_do_projeto:?}"
    );

    let mut no_projeto = entrada_do_hook("PreToolUse", sessao, &fixture(), "A-0001");
    no_projeto["tool_name"] = "Read".into();
    no_projeto["tool_input"] = json!({"file_path": "db/01_tabelas.sql"});
    let no_projeto = mediana(&no_projeto);
    assert!(no_projeto < LIMITE_HOOK, "no projeto: {no_projeto:?}");

    // O SessionStart no projeto lê a base e as regras do projeto (#31).
    let mut inicio = entrada_do_hook("SessionStart", sessao, &fixture(), "A-0001");
    inicio["source"] = "resume".into();
    let inicio = mediana(&inicio);
    assert!(inicio < LIMITE_HOOK, "SessionStart no projeto: {inicio:?}");
    assert!(dados.sessoes().join(format!("{sessao}.json")).is_file());
}

fn copiar_pasta(de: &Path, para: &Path) {
    std::fs::create_dir_all(para).unwrap();
    for e in std::fs::read_dir(de).unwrap() {
        let e = e.unwrap();
        let destino = para.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copiar_pasta(&e.path(), &destino);
        } else {
            std::fs::copy(e.path(), destino).unwrap();
        }
    }
}

fn git_em(pasta: &Path, args: &[&str]) {
    let saida = Command::new("git")
        .arg("-C")
        .arg(pasta)
        .args(args)
        .env("GIT_AUTHOR_NAME", "Teste")
        .env("GIT_AUTHOR_EMAIL", "teste@example.com")
        .env("GIT_COMMITTER_NAME", "Teste")
        .env("GIT_COMMITTER_EMAIL", "teste@example.com")
        .output()
        .unwrap();
    assert!(saida.status.success(), "git {args:?}: {saida:?}");
}

fn esperar(mut condicao: impl FnMut() -> bool, o_que: &str) {
    let inicio = Instant::now();
    while !condicao() {
        assert!(
            inicio.elapsed() < Duration::from_secs(30),
            "esperando {o_que}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Aviso de alteração de ponta a ponta (#29), com o executável de verdade: o
/// `SessionStart` responde rápido e dispara `cba fundo retratar`; o `Stop` dispara
/// `cba fundo conferir`, que avisa a análise pelo diário de eventos. A alteração
/// que já existia antes da sessão não entra no aviso.
#[test]
fn hook_dispara_o_retrato_e_a_conferencia_em_processo_separado() {
    let dir = tempfile::tempdir().unwrap();
    let raiz = dunce::canonicalize(dir.path()).unwrap();
    let codigo = raiz.join("faturamento");
    copiar_pasta(&fixture(), &codigo);
    git_em(&codigo, &["init", "-q"]);
    git_em(&codigo, &["config", "core.autocrlf", "false"]);
    git_em(&codigo, &["add", "-A"]);
    git_em(&codigo, &["commit", "-q", "-m", "inicial"]);
    std::fs::write(codigo.join("db/rascunho.sql"), "-- do analista\n").unwrap();

    let dados = cba_core::dados::DiretorioDados::em(raiz.join("dados"));
    let projeto = cba_index::Projeto::novo("Faturamento Exemplo", [&codigo]).unwrap();
    cba_index::Cadastro::em(dados.projetos())
        .cadastrar(&projeto)
        .unwrap();
    cba_kb::Base::criar(raiz.join("base")).unwrap();
    cba_core::config::Config {
        base: Some(raiz.join("base")),
        ..Default::default()
    }
    .gravar(&dados)
    .unwrap();
    let ambiente = cba_core::ambiente::Ambiente::do_diretorio(&codigo, &dados).unwrap();
    let analise = ambiente
        .analises
        .criar(cba_core::analises::NovaAnalise {
            fluxo: cba_core::analise::Fluxo::Perguntar,
            titulo: "Multa por atraso".into(),
            entrada: "Como é calculada a multa?".into(),
            analista: None,
            base: None,
        })
        .unwrap();
    let id = analise.id.to_string();
    let sessao = "5d4c3b2a-1f0e-4d9c-8b7a-6f5e4d3c2b1a";
    let retrato = dados
        .raiz()
        .join("retratos")
        .join(sessao)
        .join("retrato.json");

    let inicio = Instant::now();
    rodar_hook(
        "SessionStart",
        &entrada_do_hook("SessionStart", sessao, &codigo, &id),
        dados.raiz(),
    );
    assert!(inicio.elapsed() < LIMITE_HOOK * 5, "{:?}", inicio.elapsed());
    esperar(|| retrato.is_file(), "o retrato");
    rodar_hook(
        "PreToolUse",
        &entrada_do_hook("PreToolUse", sessao, &codigo, &id),
        dados.raiz(),
    );

    // A sessão altera um arquivo.
    let pkg = codigo.join("db/03_pkg_cobranca.sql");
    let mut texto_pkg = std::fs::read_to_string(&pkg).unwrap();
    texto_pkg.push_str("-- alterado na sessão\n");
    std::fs::write(&pkg, texto_pkg).unwrap();
    rodar_hook(
        "Stop",
        &entrada_do_hook("Stop", sessao, &codigo, &id),
        dados.raiz(),
    );
    let diario = cba_core::eventos::DiarioEventos::em(dados.eventos());
    esperar(
        || {
            diario
                .ler("faturamento-exemplo")
                .unwrap()
                .iter()
                .any(|e| e.mudanca == cba_core::eventos::Mudanca::Alteracao && e.analise == id)
        },
        "o aviso no diário",
    );
    let aviso =
        cba_session::alteracoes::da_analise(&dados, "faturamento-exemplo", &analise.id).unwrap();
    let nomes: Vec<_> = aviso
        .arquivos
        .iter()
        .map(|d| d.alterado.caminho.as_str())
        .collect();
    assert_eq!(nomes, ["db/03_pkg_cobranca.sql"]);
    assert!(
        aviso.arquivos[0]
            .diff
            .as_deref()
            .unwrap()
            .contains("+-- alterado na sessão\n")
    );

    // O log não traz o identificador da sessão nem caminhos.
    let logs = logs(dados.raiz());
    assert!(logs.contains("aviso de alteração"), "{logs}");
    for proibido in [sessao, "faturamento", "pkg_cobranca"] {
        assert!(!logs.contains(proibido), "{proibido}: {logs}");
    }
}
