//! Executável `cba`: o app desktop e, conforme os argumentos, o servidor MCP e o
//! receptor de hooks dos clientes de IA (spec, seção 5).
//!
//! | Argumentos | Modo |
//! |---|---|
//! | (nenhum) | abre a janela do app (Tauri) |
//! | `mcp` | servidor MCP em stdio ([`cba_mcp`]) |
//! | `hook <evento>` | lê o JSON do hook na entrada padrão e responde ([`cba_session::hooks`]) |
//! | `fundo <tarefa> <sessao>` | retrato ou conferência do aviso de alteração, disparado pelo hook ([`cba_session::alteracoes`]) |
//! | `--version` | imprime `cba <versão>` |
//!
//! Os modos de linha de comando não inicializam o Tauri nem a webview: o cliente de IA
//! espera por eles (meta do hook: menos de 200 ms).
//!
//! # Windows: subsistema e console
//!
//! O executável usa sempre o subsistema `windows` (inclusive em debug, para os testes
//! exercitarem o mesmo caminho da release): abrir o app não cria janela de console, e o
//! Claude Code, que inicia `cba mcp` e `cba hook` com a entrada e a saída redirecionadas
//! para pipes, também não faz piscar uma janela. Um processo do subsistema `windows`
//! herda os pipes normalmente; o que ele não tem é console. Por isso, nos modos de linha
//! de comando, se a saída padrão não existir (executado direto num terminal, sem
//! redirecionamento), o processo se anexa ao console do processo pai
//! (`AttachConsole(ATTACH_PARENT_PROCESS)`), e `cba --version` aparece no terminal. O
//! terminal não espera um programa do subsistema `windows` terminar, então o prompt pode
//! voltar antes da saída; scripts que precisam da saída a capturam por pipe
//! (`cba --version | Out-String` no PowerShell), o que funciona sem o console.

#![windows_subsystem = "windows"]

mod acessos;
mod alteracoes;
mod ambiente_teste;
mod analise;
mod base;
mod cli;
mod clones;
mod cobertura;
mod conducao;
mod conexoes;
mod configuracoes;
#[cfg(windows)]
mod console;
mod consultas;
mod conversa;
mod demandas;
mod docs_externos;
mod documentacao_do_mapa;
mod entregaveis_demanda;
mod funcionalidades;
mod gui;
mod inicio;
mod issues_projeto;
mod links_projeto;
mod manual;
mod migracao_conexoes;
mod mudancas;
mod painel_funcionalidade;
mod pauta;
mod primeiro_uso;
mod projeto;
mod prompts;
mod prototipo;
mod publicacao;
mod rastreabilidade;
mod registros;
mod repositorios;
mod resumo;
mod reunioes;
mod revisao;
mod terminal;
#[cfg(test)]
mod teste_base;
mod tokens;
mod vigia;
mod vincular_issues;
mod workflows;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let modo = cli::Modo::dos_argumentos(&args);
    // As conexões das Configurações (#437) valem para a janela e para as
    // ferramentas do agente (servidor MCP): o catálogo as relê quando mudam.
    if matches!(modo, cli::Modo::Janela | cli::Modo::Mcp)
        && let Ok(dados) = cba_core::dados::DiretorioDados::padrao()
        && let Err(e) = conexoes::carregar(&dados)
    {
        eprintln!("conexões não carregadas: {e}");
    }
    match modo {
        cli::Modo::Janela => gui::rodar(),
        modo => {
            #[cfg(windows)]
            console::anexar_se_preciso();
            std::process::exit(cli::rodar(modo));
        }
    }
}
