//! Reuniões (épico #93): o que o app faz com o transcript do Teams antes de
//! qualquer IA.
//!
//! - [`transcript`]: parser determinístico que transforma o transcript
//!   (WebVTT, `.docx` exportado ou texto copiado da tela) em falas, com o
//!   formato detectado e os avisos (#134).
//!
//! A crate não depende do Tauri nem da base de conhecimento: recebe bytes ou
//! texto e devolve dados. Quem usa: a guarda temporária do transcript (#136),
//! a ferramenta `ler_transcript` do MCP (#139) e a tela de entrada da reunião
//! (#141).
//!
//! **Privacidade:** as falas têm dados pessoais. Nada aqui grava em disco nem
//! escreve no log, e os [`Aviso`]s nunca carregam trechos das falas, para
//! poderem ir ao diário de eventos e ao log sem vazar o conteúdo.

pub mod transcript;

pub use transcript::{
    Aviso, ErroDocx, Fala, Formato, Horario, Transcript, analisar, analisar_texto,
    detectar_formato, paragrafos_docx,
};
