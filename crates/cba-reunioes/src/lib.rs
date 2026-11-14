//! Reuniões (épico #93): o que o app faz com o transcript do Teams antes de
//! qualquer IA.
//!
//! - [`transcript`]: parser determinístico que transforma o transcript
//!   (WebVTT, `.docx` exportado ou texto copiado da tela) em falas, com o
//!   formato detectado e os avisos (#134).
//! - [`guarda`]: guarda temporária das falas no diretório de dados do app
//!   (`<dados>/reunioes/<projeto>/<R-NNNN>/`), leitura filtrada e paginada,
//!   descarte e retenção (#136).
//! - [`citacao`]: evidência do tipo fala: confere o trecho citado por um item
//!   da ata contra a fala de origem (#136).
//!
//! A crate não depende do Tauri. Quem usa: a ferramenta `ler_transcript` e
//! `registrar_item_ata` do MCP (#139), a tela de entrada da reunião (#141) e a
//! aprovação da ata.
//!
//! **Privacidade:** as falas têm dados pessoais. Só a [`guarda`] grava as
//! falas em disco, e só no diretório de dados do app. O log e o diário de
//! eventos recebem só identificadores (projeto, reunião, índice da fala) e
//! contagens; os [`Aviso`]s e as mensagens de erro nunca carregam trechos das
//! falas.

pub mod citacao;
pub mod guarda;
pub mod transcript;

pub use citacao::{
    CitacaoConferida, FALANTE_DESCONHECIDO, LIMITE_DO_TRECHO, PedidoCitacao, RecusaCitacao,
    conferir_citacao,
};
pub use guarda::{
    DIAS_DE_AVISO, Descarte, ErroGuarda, FiltroFalas, Guarda, Metadados, MotivoDescarte,
    PaginaFalas, RelatorioRetencao, ReuniaoGuardada, Situacao, aplicar_retencao,
    aplicar_retencao_no_inicio, filtrar_falas, marcar_citacoes_descartadas,
};

pub use transcript::{
    Aviso, ErroDocx, Fala, Formato, Horario, Transcript, analisar, analisar_texto,
    detectar_formato, paragrafos_docx,
};
