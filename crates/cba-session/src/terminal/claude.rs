//! Como o `claude` é iniciado no terminal embutido.
//!
//! Só opções documentadas da linha de comando do Claude Code
//! (<https://code.claude.com/docs/en/cli-reference>, consultada em 2026-11-11):
//!
//! | Pedido | Argumentos |
//! |---|---|
//! | conversa nova | `--session-id <uuid>` (o app já sabe o identificador para retomar depois) |
//! | continuar a última conversa da pasta | `--continue` |
//! | retomar uma conversa | `--resume <id>` |
//! | prompt inicial | último argumento, posicional: a sessão interativa começa com ele |
//!
//! A opção de retomada vem sempre logo antes do prompt: uma opção que aceita vários
//! valores em [`PedidoClaude::argumentos`] (`--add-dir a b`) engoliria o prompt
//! posicional, e uma opção comum no meio encerra a lista (como no ctower-agents,
//! `crates/ct-sessions/src/launch.rs`).

use std::ffi::{OsStr, OsString};
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::pty::Comando;
use crate::clientes::{self, Ambiente, Passo, Plataforma};
use crate::preparo::{Alvo, Preparador, Situacao, Verificacao};

/// Opções que tiram a governança do analista: não passam pelo terminal embutido.
const PROIBIDOS: &[&str] = &[
    "--dangerously-skip-permissions",
    "--allow-dangerously-skip-permissions",
    "bypassPermissions",
];

/// Variáveis que o app não repassa ao `claude`: as que marcam um processo iniciado de
/// dentro de uma sessão do Claude Code (o app aberto por um terminal onde o Claude Code
/// roda). `CLAUDECODE` faria o `claude` recusar a sessão aninhada, e
/// `CLAUDE_CODE_CHILD_SESSION` desliga a gravação da conversa: sem ela, a sessão da
/// análise não pode ser retomada pelo identificador (#130). As demais são da sessão
/// de fora (identificador, canal de mensagens) e não valem para a conversa nova.
const VARIAVEIS_REMOVIDAS: &[&str] = &[
    "CLAUDECODE",
    "CLAUDE_CODE_CHILD_SESSION",
    "CLAUDE_CODE_SESSION_ID",
    "CLAUDE_CODE_ENTRYPOINT",
    "CLAUDE_CODE_EXECPATH",
    "CLAUDE_CODE_MESSAGING_SOCKET",
    "CLAUDE_CODE_MESSAGING_TOKEN",
    "CLAUDE_CODE_SESSION_ATTENDED",
    "CLAUDE_PID",
];

/// Como a conversa começa.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "modo",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Retomada {
    /// Conversa nova, com identificador gerado pelo app.
    #[default]
    Nova,
    /// A conversa mais recente na pasta do projeto (`--continue`).
    Continuar,
    /// Uma conversa conhecida (`--resume <id>`).
    Sessao { id: String },
}

/// O que o terminal embutido pede ao `claude`. Tudo é opcional: o padrão é uma
/// conversa nova, sem prompt.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PedidoClaude {
    /// Primeira mensagem da conversa, como se o analista a tivesse digitado.
    pub prompt_inicial: Option<String>,
    pub retomada: Retomada,
    /// Opções extras do `claude`, antes da retomada e do prompt.
    pub argumentos: Vec<String>,
}

/// Comando pronto para o terminal, com o identificador da conversa quando o app o
/// conhece.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lancamento {
    pub comando: Comando,
    /// `Some` em conversa nova (`--session-id`) e em retomada por identificador.
    pub id_sessao: Option<String>,
}

/// Por que o terminal não abriu. Vai para a interface como JSON (veja
/// [`ErroAbertura::relatorio`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, thiserror::Error)]
#[serde(
    tag = "motivo",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ErroAbertura {
    #[error("o projeto \"{projeto}\" não está cadastrado")]
    ProjetoNaoCadastrado { projeto: String },

    #[error(
        "o ambiente do Claude ainda não está preparado para este projeto: prepare-o pelo app antes de abrir o terminal"
    )]
    AmbienteNaoPreparado {
        /// O que falta, em linguagem do analista.
        pendencias: Vec<String>,
    },

    #[error("o Claude Code não foi encontrado neste computador")]
    ClaudeNaoEncontrado {
        /// Passo a passo oficial de instalação.
        orientacao: Vec<Passo>,
    },

    /// `claude --desktop` indisponível (plataforma ou versão do Claude Code).
    #[error("{detalhe}")]
    DesktopIndisponivel {
        detalhe: String,
        /// Passo a passo oficial, quando há o que fazer.
        orientacao: Vec<Passo>,
    },

    #[error("pedido inválido: {mensagem}")]
    PedidoInvalido { mensagem: String },

    #[error("{mensagem}")]
    Falha { mensagem: String },
}

/// [`ErroAbertura`] com a mensagem pronta, como a interface recebe:
/// `{ "motivo": "...", "mensagem": "...", ...campos do motivo }`.
#[derive(Debug, Clone, Serialize)]
pub struct RelatorioErro {
    #[serde(flatten)]
    pub erro: ErroAbertura,
    pub mensagem: String,
}

impl ErroAbertura {
    pub fn relatorio(self) -> RelatorioErro {
        RelatorioErro {
            mensagem: self.to_string(),
            erro: self,
        }
    }

    fn invalido(mensagem: impl Into<String>) -> Self {
        Self::PedidoInvalido {
            mensagem: mensagem.into(),
        }
    }
}

/// Confere o ambiente e monta o comando do `claude` para a sessão de `projeto`.
///
/// Recusa com orientação quando o ambiente não está preparado
/// ([`Preparador::verificar`]) ou o Claude Code não está instalado
/// ([`clientes::localizar_claude`]). O `claude` roda na pasta do projeto
/// ([`Alvo::diretorio`]).
pub fn lancamento(
    projeto: &cba_index::Projeto,
    pedido: &PedidoClaude,
    preparador: &Preparador,
    ambiente: &dyn Ambiente,
) -> Result<Lancamento, ErroAbertura> {
    let alvo = Alvo::do_projeto(projeto).map_err(|e| ErroAbertura::Falha {
        mensagem: e.to_string(),
    })?;
    let verificacao = preparador
        .verificar(&alvo)
        .map_err(|e| ErroAbertura::Falha {
            mensagem: e.to_string(),
        })?;
    if !verificacao.pronto {
        return Err(ErroAbertura::AmbienteNaoPreparado {
            pendencias: pendencias(&verificacao),
        });
    }
    let Some((claude, _)) = clientes::localizar_claude(ambiente) else {
        return Err(ErroAbertura::ClaudeNaoEncontrado {
            orientacao: clientes::orientacao::claude_code_instalar(ambiente.plataforma()),
        });
    };
    montar(
        &claude,
        &alvo.diretorio,
        pedido,
        ambiente.plataforma(),
        ambiente.var("PATH").as_deref(),
        || uuid::Uuid::new_v4().to_string(),
    )
}

/// O que falta no ambiente, em frases curtas.
pub fn pendencias(v: &Verificacao) -> Vec<String> {
    let mut lista = Vec::new();
    match v.servidor_mcp {
        Situacao::Ok => {}
        Situacao::Ausente => {
            lista.push("o servidor MCP do app não está registrado para esta pasta".into())
        }
        Situacao::Desatualizado => lista.push(
            "o servidor MCP do app está registrado com outro executável ou parâmetros".into(),
        ),
        Situacao::Conflito => lista
            .push("já existe um servidor MCP chamado \"cba\" que não é do app nesta pasta".into()),
    }
    if v.hooks.iter().any(|h| h.situacao != Situacao::Ok) {
        lista.push("os hooks do app estão ausentes ou desatualizados".into());
    }
    if v.hooks_desativados {
        lista.push(
            "os hooks estão desativados nas configurações do Claude (disableAllHooks)".into(),
        );
    }
    lista
}

/// Monta o comando do `claude` em `diretorio`. `path` é o `PATH` atual do app;
/// `gerar_id` cria o identificador de uma conversa nova.
pub fn montar(
    claude: &Path,
    diretorio: &Path,
    pedido: &PedidoClaude,
    plataforma: Plataforma,
    path: Option<&OsStr>,
    gerar_id: impl FnOnce() -> String,
) -> Result<Lancamento, ErroAbertura> {
    let lote = e_lancador_de_lote(claude);
    let mut args: Vec<String> = Vec::new();
    for arg in &pedido.argumentos {
        if arg.contains('\0') {
            return Err(ErroAbertura::invalido("argumento com caractere nulo"));
        }
        if PROIBIDOS.iter().any(|p| arg.contains(p)) {
            return Err(ErroAbertura::invalido(format!(
                "a opção {arg} não é permitida no terminal embutido"
            )));
        }
        args.push(arg.clone());
    }

    let id_sessao = match &pedido.retomada {
        Retomada::Nova => {
            let id = gerar_id();
            args.extend(["--session-id".into(), id.clone()]);
            Some(id)
        }
        Retomada::Continuar => {
            args.push("--continue".into());
            None
        }
        Retomada::Sessao { id } => {
            if !id_valido(id) {
                return Err(ErroAbertura::invalido(
                    "identificador de sessão inválido (só letras, números, - e _)",
                ));
            }
            args.extend(["--resume".into(), id.clone()]);
            Some(id.clone())
        }
    };

    if let Some(prompt) = pedido.prompt_inicial.as_deref().and_then(limpar_prompt) {
        args.push(prompt);
    }

    if lote {
        for arg in &args {
            seguro_para_lote(arg)?;
        }
    }

    let mut env: Vec<(OsString, OsString)> = Vec::new();
    if let Some(pasta) = claude.parent().filter(|p| !p.as_os_str().is_empty()) {
        env.push(("PATH".into(), path_com(pasta, path, plataforma)));
    }
    if plataforma != Plataforma::Windows {
        env.push(("TERM".into(), "xterm-256color".into()));
        env.push(("COLORTERM".into(), "truecolor".into()));
    }

    Ok(Lancamento {
        comando: Comando {
            programa: claude.to_path_buf(),
            args: args.into_iter().map(OsString::from).collect(),
            diretorio: diretorio.to_path_buf(),
            env,
            env_remover: VARIAVEIS_REMOVIDAS.iter().map(OsString::from).collect(),
        },
        id_sessao,
    })
}

/// `PATH` com a pasta do `claude` na frente. O app aberto pelo Finder ou pelo menu
/// Iniciar tem um `PATH` mínimo, sem `~/.local/bin`; o `claude` e o que ele executa
/// (o próprio `claude` em subprocessos, por exemplo) precisam ser achados.
pub fn path_com(pasta: &Path, path: Option<&OsStr>, plataforma: Plataforma) -> OsString {
    let mut novo = OsString::from(pasta.as_os_str());
    if let Some(path) = path.filter(|p| !p.is_empty()) {
        novo.push(plataforma.separador_path().to_string());
        novo.push(path);
    }
    novo
}

/// `claude.cmd` ou `claude.bat` (instalação pelo npm no Windows): o Windows passa a
/// linha de comando pelo `cmd.exe`, que interpreta `%`, `!`, aspas e, fora de aspas,
/// `&`, `|`, `<`, `>`, `^` e parênteses.
pub fn e_lancador_de_lote(programa: &Path) -> bool {
    programa
        .extension()
        .and_then(OsStr::to_str)
        .is_some_and(|e| e.eq_ignore_ascii_case("cmd") || e.eq_ignore_ascii_case("bat"))
}

/// Recusa argumentos que o `cmd.exe` poderia interpretar como comando. Um argumento
/// com espaço vai entre aspas (onde `&|<>^()` são texto); sem espaço, vai como está.
fn seguro_para_lote(arg: &str) -> Result<(), ErroAbertura> {
    let sempre = ['"', '%', '!', '\r', '\n'];
    let fora_de_aspas = ['&', '|', '<', '>', '^', '(', ')'];
    let entre_aspas = arg.contains([' ', '\t']);
    if arg.contains(sempre) || (!entre_aspas && arg.contains(fora_de_aspas)) {
        return Err(ErroAbertura::invalido(
            "com o Claude Code instalado pelo npm (claude.cmd), o prompt não pode ter aspas, %, !, quebras de linha nem & | < > ^ ( ) numa palavra solta. Instale pelo instalador nativo ou envie o texto depois que a sessão abrir",
        ));
    }
    Ok(())
}

/// Tira caracteres de controle (exceto quebra de linha e tabulação) e espaços nas
/// pontas. Prompt vazio vira `None`. Um prompt que começa com `-` ganha um espaço na
/// frente para não ser lido como opção.
pub fn limpar_prompt(prompt: &str) -> Option<String> {
    let limpo: String = prompt
        .replace("\r\n", "\n")
        .chars()
        .map(|c| if c == '\r' { '\n' } else { c })
        .filter(|c| !c.is_control() || matches!(c, '\n' | '\t'))
        .collect();
    let limpo = limpo.trim();
    if limpo.is_empty() {
        None
    } else if limpo.starts_with('-') {
        Some(format!(" {limpo}"))
    } else {
        Some(limpo.to_string())
    }
}

/// Identificador de sessão aceito na linha de comando: letras, dígitos, `-` e `_`.
pub fn id_valido(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn montar_unix(pedido: &PedidoClaude) -> Result<Lancamento, ErroAbertura> {
        montar(
            Path::new("/home/a/.local/bin/claude"),
            Path::new("/projetos/fat"),
            pedido,
            Plataforma::Linux,
            Some(OsStr::new("/usr/bin:/bin")),
            || "11111111-2222-4333-8444-555555555555".into(),
        )
    }

    fn args(l: &Lancamento) -> Vec<String> {
        l.comando
            .args
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    fn env(l: &Lancamento, nome: &str) -> Option<String> {
        l.comando
            .env
            .iter()
            .find(|(k, _)| k == nome)
            .map(|(_, v)| v.to_string_lossy().into_owned())
    }

    #[test]
    fn conversa_nova_sem_prompt() {
        let l = montar_unix(&PedidoClaude::default()).unwrap();
        assert_eq!(
            args(&l),
            ["--session-id", "11111111-2222-4333-8444-555555555555"]
        );
        assert_eq!(
            l.id_sessao.as_deref(),
            Some("11111111-2222-4333-8444-555555555555")
        );
        assert_eq!(l.comando.diretorio, Path::new("/projetos/fat"));
        assert_eq!(
            env(&l, "PATH").as_deref(),
            Some("/home/a/.local/bin:/usr/bin:/bin")
        );
        assert_eq!(env(&l, "TERM").as_deref(), Some("xterm-256color"));
        assert!(l.comando.env_remover.iter().any(|v| v == "CLAUDECODE"));
        assert!(
            l.comando
                .env_remover
                .iter()
                .any(|v| v == "CLAUDE_CODE_CHILD_SESSION")
        );
    }

    #[test]
    fn prompt_inicial_vem_por_ultimo_depois_da_retomada() {
        let pedido = PedidoClaude {
            prompt_inicial: Some("  Inicie a análise A-0001 pelo ctower\r\n".into()),
            retomada: Retomada::Sessao {
                id: "abc-123".into(),
            },
            argumentos: vec!["--add-dir".into(), "/docs".into()],
        };
        let l = montar_unix(&pedido).unwrap();
        assert_eq!(
            args(&l),
            [
                "--add-dir",
                "/docs",
                "--resume",
                "abc-123",
                "Inicie a análise A-0001 pelo ctower"
            ]
        );
        assert_eq!(l.id_sessao.as_deref(), Some("abc-123"));
    }

    #[test]
    fn continuar_nao_conhece_o_id() {
        let pedido = PedidoClaude {
            retomada: Retomada::Continuar,
            ..Default::default()
        };
        let l = montar_unix(&pedido).unwrap();
        assert_eq!(args(&l), ["--continue"]);
        assert_eq!(l.id_sessao, None);
    }

    #[test]
    fn recusa_id_invalido_e_opcoes_proibidas() {
        let pedido = PedidoClaude {
            retomada: Retomada::Sessao {
                id: "x; rm -rf".into(),
            },
            ..Default::default()
        };
        assert!(matches!(
            montar_unix(&pedido),
            Err(ErroAbertura::PedidoInvalido { .. })
        ));
        let pedido = PedidoClaude {
            argumentos: vec!["--dangerously-skip-permissions".into()],
            ..Default::default()
        };
        assert!(matches!(
            montar_unix(&pedido),
            Err(ErroAbertura::PedidoInvalido { .. })
        ));
        let pedido = PedidoClaude {
            argumentos: vec!["--permission-mode".into(), "bypassPermissions".into()],
            ..Default::default()
        };
        assert!(montar_unix(&pedido).is_err());
    }

    #[test]
    fn prompt_limpo() {
        assert_eq!(limpar_prompt("  \n "), None);
        assert_eq!(limpar_prompt("a\x1b[2Jb\u{7}"), Some("a[2Jb".into()));
        assert_eq!(limpar_prompt("um\rdois"), Some("um\ndois".into()));
        assert_eq!(limpar_prompt("--help"), Some(" --help".into()));
    }

    #[test]
    fn windows_sem_term_e_path_com_ponto_e_virgula() {
        let l = montar(
            // Com `/`, para o caminho ter pasta também quando o teste roda fora do Windows.
            Path::new("C:/Users/a/.local/bin/claude.exe"),
            Path::new(r"C:\projetos\fat"),
            &PedidoClaude::default(),
            Plataforma::Windows,
            Some(OsStr::new(r"C:\Windows\system32")),
            || "id".into(),
        )
        .unwrap();
        assert_eq!(env(&l, "TERM"), None);
        let path = env(&l, "PATH").unwrap();
        assert_eq!(path, r"C:/Users/a/.local/bin;C:\Windows\system32");
    }

    #[test]
    fn lancador_de_lote_recusa_o_que_o_cmd_interpretaria() {
        assert!(e_lancador_de_lote(Path::new(r"C:\npm\claude.CMD")));
        assert!(!e_lancador_de_lote(Path::new(r"C:\bin\claude.exe")));
        let montar_cmd = |prompt: &str| {
            montar(
                Path::new(r"C:\npm\claude.cmd"),
                Path::new(r"C:\s"),
                &PedidoClaude {
                    prompt_inicial: Some(prompt.into()),
                    ..Default::default()
                },
                Plataforma::Windows,
                None,
                || "id".into(),
            )
        };
        assert!(montar_cmd("Explique o cálculo (multa & juros) da fatura?").is_ok());
        assert!(montar_cmd("a&calc").is_err());
        assert!(montar_cmd("100% certo").is_err());
        assert!(montar_cmd("diga \"oi\"").is_err());
        assert!(montar_cmd("linha um\nlinha dois").is_err());
    }

    #[test]
    fn pedido_em_json() {
        let p: PedidoClaude = serde_json::from_str(
            r#"{"promptInicial":"oi","retomada":{"modo":"sessao","id":"abc"}}"#,
        )
        .unwrap();
        assert_eq!(p.prompt_inicial.as_deref(), Some("oi"));
        assert_eq!(p.retomada, Retomada::Sessao { id: "abc".into() });
        let p: PedidoClaude = serde_json::from_str("{}").unwrap();
        assert_eq!(p, PedidoClaude::default());
    }

    #[test]
    fn erro_em_json_tem_motivo_e_mensagem() {
        let e = ErroAbertura::AmbienteNaoPreparado {
            pendencias: vec!["x".into()],
        };
        let v = serde_json::to_value(e.relatorio()).unwrap();
        assert_eq!(v["motivo"], "ambienteNaoPreparado");
        assert_eq!(v["pendencias"][0], "x");
        assert!(v["mensagem"].as_str().unwrap().contains("preparado"));
    }
}
