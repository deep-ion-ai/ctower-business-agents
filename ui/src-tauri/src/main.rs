//! Executável `cba`: o app desktop e, conforme os argumentos, o servidor MCP e o
//! receptor de hooks dos clientes de IA (spec, seção 5).
//!
//! | Argumentos | Modo |
//! |---|---|
//! | (nenhum) | abre a janela do app (Tauri) |
//! | `mcp` | servidor MCP em stdio ([`cba_mcp`]) |
//! | `hook <evento>` | lê o JSON do hook na entrada padrão e responde ([`cba_session::hooks`]) |
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

mod cli;
#[cfg(windows)]
mod console;
mod gui;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match cli::Modo::dos_argumentos(&args) {
        cli::Modo::Janela => gui::rodar(),
        modo => {
            #[cfg(windows)]
            console::anexar_se_preciso();
            std::process::exit(cli::rodar(modo));
        }
    }
}
