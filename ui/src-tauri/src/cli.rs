//! Modos de linha de comando: `mcp`, `hook <evento>`, `fundo <tarefa> <sessao>`,
//! `--version` e `--help`.
//!
//! Nada aqui toca o Tauri. Erros de uso saem com código 1, nunca 2: para o Claude Code,
//! o código 2 de um hook bloqueia a ação (ex.: a ferramenta no `PreToolUse`).

use std::io::{self, Write};

use cba_core::ambiente::{Ambiente, ErroAmbiente};
use cba_core::dados::DiretorioDados;

const USO: &str = "\
Uso:
  cba                 abre o app
  cba mcp             servidor MCP em stdio (iniciado pelo cliente de IA)
  cba hook <evento>   receptor de hooks: JSON do evento na entrada padrão
  cba fundo <tarefa> <sessao>
                      retrato (retratar) ou conferência (conferir) da pasta do
                      sistema para o aviso de alteração; iniciado pelo hook
  cba --version       versão
  cba --help          esta ajuda";

/// Modo do executável, decidido pelos argumentos (sem o nome do programa).
#[derive(Debug, PartialEq, Eq)]
pub enum Modo {
    Janela,
    Mcp,
    Hook(String),
    /// Tarefa do aviso de alteração, fora do caminho do hook.
    Fundo(cba_session::alteracoes::Tarefa, String),
    Versao,
    Ajuda,
    Uso(String),
}

impl Modo {
    pub fn dos_argumentos(args: &[String]) -> Self {
        match args {
            [] => Modo::Janela,
            // Versões antigas do macOS passam o número do processo ao abrir pelo Finder.
            [a, ..] if a.starts_with("-psn_") => Modo::Janela,
            [a] if a == "mcp" => Modo::Mcp,
            [a, evento] if a == "hook" && !evento.is_empty() => Modo::Hook(evento.clone()),
            [a, ..] if a == "hook" => Modo::Uso("uso: cba hook <evento>".into()),
            [a, tarefa, sessao] if a == "fundo" => {
                match cba_session::alteracoes::Tarefa::do_nome(tarefa) {
                    Some(t) if cba_session::sessoes::id_valido(sessao) => {
                        Modo::Fundo(t, sessao.clone())
                    }
                    _ => Modo::Uso("uso: cba fundo retratar|conferir <sessao>".into()),
                }
            }
            [a, ..] if a == "fundo" => {
                Modo::Uso("uso: cba fundo retratar|conferir <sessao>".into())
            }
            [a] if a == "--version" || a == "-V" => Modo::Versao,
            [a] if a == "--help" || a == "-h" => Modo::Ajuda,
            _ => Modo::Uso(format!("argumentos não reconhecidos: {}", args.join(" "))),
        }
    }
}

/// Executa um modo de linha de comando e devolve o código de saída.
pub fn rodar(modo: Modo) -> i32 {
    match modo {
        Modo::Janela => unreachable!("o modo janela é do gui.rs"),
        Modo::Versao => {
            println!("cba {}", cba_core::VERSAO);
            0
        }
        Modo::Ajuda => {
            println!("{} {}\n\n{USO}", cba_core::PRODUTO, cba_core::VERSAO);
            0
        }
        Modo::Uso(erro) => {
            eprintln!("{erro}\n\n{USO}");
            1
        }
        Modo::Mcp => mcp(),
        Modo::Hook(evento) => hook(&evento),
        Modo::Fundo(tarefa, sessao) => fundo(tarefa, &sessao),
    }
}

fn iniciar_log(modo: &str) {
    // O log é auxiliar: sem ele, o modo segue (a saída padrão é do protocolo).
    if let Ok(dados) = DiretorioDados::padrao() {
        let _ = cba_core::log::iniciar(&dados.logs(), modo);
    }
}

fn mcp() -> i32 {
    iniciar_log("mcp");
    tracing::info!(versao = cba_core::VERSAO, "servidor MCP iniciado");
    // O sistema é o da pasta em que o cliente iniciou o processo. Se não houver, o
    // servidor sobe assim mesmo e cada chamada de ferramenta explica o que fazer.
    let ambiente = ambiente_do_processo();
    match &ambiente {
        Ok(_) => tracing::info!("sistema identificado"),
        Err(e) => tracing::warn!(erro = e.tipo(), "sistema não identificado"),
    }
    let mut servidor = cba_mcp::Servidor::new(
        cba_mcp::InfoServidor {
            nome: "cba".into(),
            titulo: cba_core::PRODUTO.into(),
            versao: cba_core::VERSAO.into(),
        },
        cba_tools::registro_padrao(),
        ambiente,
    );
    match cba_mcp::servir(io::stdin().lock(), io::stdout().lock(), &mut servidor) {
        Ok(()) => {
            tracing::info!("servidor MCP encerrado: fim da entrada");
            0
        }
        Err(e) => {
            tracing::error!(erro = %e.kind(), "servidor MCP encerrado com erro de E/S");
            1
        }
    }
}

/// Ambiente do sistema do processo: o de `CBA_SISTEMA` (gravado pelo preparo do
/// ambiente no registro do servidor), ou o que contém o diretório de trabalho,
/// inclusive numa worktree do git.
fn ambiente_do_processo() -> Result<Ambiente, ErroAmbiente> {
    let dados = DiretorioDados::padrao().map_err(ErroAmbiente::Config)?;
    let diretorio = std::env::current_dir().map_err(ErroAmbiente::Config)?;
    let sistema = std::env::var(cba_core::ambiente::VAR_SISTEMA).ok();
    Ambiente::do_processo(&diretorio, sistema.as_deref(), &dados)
}

fn hook(evento: &str) -> i32 {
    iniciar_log("hook");
    let resposta = cba_session::hooks::tratar_stdin(evento);
    let mut saida = io::stdout().lock();
    // Se o cliente já fechou a saída, não há a quem avisar; o código de saída vale.
    let _ = serde_json::to_writer(&mut saida, &resposta.saida)
        .map_err(io::Error::from)
        .and_then(|()| saida.write_all(b"\n"))
        .and_then(|()| saida.flush());
    resposta.codigo
}

/// Retrato ou conferência do aviso de alteração. Ninguém lê a saída: o resultado vai
/// para o diretório de dados e o desfecho, sem caminhos nem identificadores, para o
/// log.
fn fundo(tarefa: cba_session::alteracoes::Tarefa, sessao: &str) -> i32 {
    // Sem o diretório de dados (apagado depois do hook), não há o que fazer, e
    // nada deve ser recriado.
    let Some(dados) = DiretorioDados::padrao().ok().filter(|d| d.raiz().is_dir()) else {
        return 1;
    };
    iniciar_log("fundo");
    let inicio = std::time::Instant::now();
    let resultado = cba_session::alteracoes::executar(&dados, tarefa, sessao);
    let duracao_ms = inicio.elapsed().as_millis() as u64;
    match resultado {
        Ok(()) => {
            tracing::info!(tarefa = tarefa.nome(), duracao_ms, "aviso de alteração");
            0
        }
        Err(e) => {
            tracing::warn!(
                tarefa = tarefa.nome(),
                duracao_ms,
                erro = cba_session::alteracoes::tipo_do_erro(&e),
                "aviso de alteração falhou"
            );
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn modo(args: &[&str]) -> Modo {
        let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
        Modo::dos_argumentos(&args)
    }

    #[test]
    fn argumentos() {
        assert_eq!(modo(&[]), Modo::Janela);
        assert_eq!(modo(&["-psn_0_12345"]), Modo::Janela);
        assert_eq!(modo(&["mcp"]), Modo::Mcp);
        assert_eq!(
            modo(&["hook", "SessionStart"]),
            Modo::Hook("SessionStart".into())
        );
        assert_eq!(modo(&["--version"]), Modo::Versao);
        assert_eq!(modo(&["-V"]), Modo::Versao);
        assert_eq!(modo(&["--help"]), Modo::Ajuda);
        assert!(matches!(modo(&["hook"]), Modo::Uso(_)));
        assert!(matches!(modo(&["hook", "A", "B"]), Modo::Uso(_)));
        assert!(matches!(modo(&["mcp", "x"]), Modo::Uso(_)));
        assert!(matches!(modo(&["outro"]), Modo::Uso(_)));
        assert_eq!(
            modo(&["fundo", "retratar", "abc-123"]),
            Modo::Fundo(cba_session::alteracoes::Tarefa::Retratar, "abc-123".into())
        );
        assert!(matches!(modo(&["fundo", "outra", "abc"]), Modo::Uso(_)));
        assert!(matches!(modo(&["fundo", "conferir", "../x"]), Modo::Uso(_)));
        assert!(matches!(modo(&["fundo", "conferir"]), Modo::Uso(_)));
    }
}
