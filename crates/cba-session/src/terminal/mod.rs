//! Terminal embutido com o Claude Code (spec, seção 6.4).
//!
//! O app roda o `claude` do analista, sem modificação e em modo interativo, num PTY na
//! pasta do projeto, e a interface mostra o terminal (xterm.js). Este módulo não
//! depende do Tauri: o app só liga os eventos ao canal da janela.
//!
//! - [`pty`]: um processo num PTY ([`Terminal`]): entrada, saída em texto,
//!   redimensionamento, encerramento e código de saída. ConPTY no Windows, `openpty` no
//!   macOS e no Linux, pela crate `portable-pty` (do wezterm).
//! - [`claude`]: o comando do `claude` ([`lancamento`]): confere se o ambiente está
//!   preparado e se o Claude Code está instalado, e monta os argumentos documentados
//!   (conversa nova com `--session-id`, `--continue`, `--resume`, prompt inicial).
//! - [`desktop`]: `claude --desktop --resume <id>` num PTY sem tela, para abrir no
//!   Claude Desktop uma sessão fechada no terminal ([`lancamento_desktop`]).
//! - [`sessoes`]: os terminais abertos ([`Sessoes`]), o envio de prompts prontos à
//!   sessão viva e a última sessão de cada projeto ([`UltimasSessoes`]), para oferecer
//!   a retomada quando o app é reaberto.
//! - [`transcricao`]: se a conversa de uma sessão foi gravada ([`conversa_gravada`]),
//!   para só oferecer a retomada pelo identificador quando ela pode dar certo.
//!
//! Botão "Continuar no Claude Desktop": é o prompt `/desktop` enviado por
//! [`Sessoes::enviar_prompt`] (comando do próprio Claude Code). Pela documentação, o
//! Claude Code salva a sessão, abre-a no Desktop e encerra o CLI (veja
//! [`crate::passagem`]).

pub mod claude;
pub mod desktop;
pub mod pty;
pub mod sessoes;
pub mod transcricao;

pub use claude::{
    ErroAbertura, Lancamento, PedidoClaude, RelatorioErro, Retomada, lancamento, pendencias,
};
pub use desktop::{ResultadoDesktop, executar_no_pty, lancamento_desktop, montar_desktop};
pub use pty::{Comando, DecodificadorUtf8, ErroTerminal, Evento, Fim, Tamanho, Terminal};
pub use sessoes::{PAUSA_ENVIO, Sessoes, UltimaSessao, UltimasSessoes};
pub use transcricao::conversa_gravada;

/// Prompt que move a sessão para a aba de código do Claude Desktop.
pub const PROMPT_DESKTOP: &str = "/desktop";
