//! Passo a passo oficial da Anthropic para instalar e entrar nos clientes de IA.
//!
//! O app só mostra o caminho: não executa instaladores, não oferece login próprio e não
//! intermedeia credenciais (spec, seção 9). Os comandos são os mesmos dos scripts de
//! instalação (`packaging/install`).

use serde::Serialize;

use super::Plataforma;

/// Instalador oficial do Claude Code para macOS e Linux.
pub const INSTALAR_CODE_SH: &str = "curl -fsSL https://claude.ai/install.sh | bash";
/// Instalador oficial do Claude Code para Windows (PowerShell).
pub const INSTALAR_CODE_PS1: &str = "irm https://claude.ai/install.ps1 | iex";
/// Documentação oficial de instalação do Claude Code.
pub const DOC_CODE: &str = "https://code.claude.com/docs/en/setup";
/// Página oficial de download do Claude Desktop.
pub const DOWNLOAD_DESKTOP: &str = "https://claude.ai/download";

/// Um passo para o analista seguir.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Passo {
    pub texto: String,
    /// Comando para copiar e rodar num terminal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comando: Option<String>,
    /// Página oficial para abrir no navegador.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
}

impl Passo {
    fn texto(texto: &str) -> Self {
        Self {
            texto: texto.into(),
            comando: None,
            link: None,
        }
    }

    fn comando(texto: &str, comando: &str) -> Self {
        Self {
            comando: Some(comando.into()),
            ..Self::texto(texto)
        }
    }

    fn link(texto: &str, link: &str) -> Self {
        Self {
            link: Some(link.into()),
            ..Self::texto(texto)
        }
    }
}

/// Claude Code não encontrado.
pub fn claude_code_instalar(plataforma: Plataforma) -> Vec<Passo> {
    let (terminal, comando) = match plataforma {
        Plataforma::Windows => ("Abra o PowerShell e rode", INSTALAR_CODE_PS1),
        _ => ("Abra um terminal e rode", INSTALAR_CODE_SH),
    };
    let mut passos = vec![
        Passo::comando(
            &format!("{terminal} o instalador oficial da Anthropic:"),
            comando,
        ),
        Passo::link(
            "Outras formas de instalar estão na documentação oficial.",
            DOC_CODE,
        ),
    ];
    passos.extend(claude_code_login());
    passos.push(Passo::texto(
        "Volte aqui e clique em \"Verificar de novo\".",
    ));
    passos
}

/// Claude Code instalado, sem login.
pub fn claude_code_login() -> Vec<Passo> {
    vec![
        Passo::comando(
            "Num terminal novo, rode o Claude Code e faça login com a sua conta pelo fluxo dele (/login):",
            "claude",
        ),
        Passo::texto(
            "O login é seu e fica no Claude Code: este app não pede, não lê e não guarda credenciais.",
        ),
    ]
}

/// Claude Code instalado, sem resposta clara do comando de status.
pub fn claude_code_verificar(plataforma: Plataforma) -> Vec<Passo> {
    let comando = match plataforma {
        Plataforma::Windows => INSTALAR_CODE_PS1,
        _ => INSTALAR_CODE_SH,
    };
    let mut passos = vec![
        Passo::comando(
            "Não deu para confirmar o login. Se o Claude Code for antigo, atualize-o:",
            "claude update",
        ),
        Passo::comando(
            "Se a atualização não funcionar, reinstale pelo instalador oficial:",
            comando,
        ),
    ];
    passos.extend(claude_code_login());
    passos
}

/// Claude Desktop não encontrado.
pub fn claude_desktop_instalar() -> Vec<Passo> {
    vec![
        Passo::link(
            "Baixe e instale o Claude Desktop pela página oficial:",
            DOWNLOAD_DESKTOP,
        ),
        Passo::texto(
            "Abra o Claude Desktop e entre com a sua conta do Claude (é preciso ter assinatura).",
        ),
        Passo::texto("Volte aqui e clique em \"Verificar de novo\"."),
    ]
}

/// Claude Desktop instalado: o login não é verificado pelo app.
pub fn claude_desktop_login() -> Vec<Passo> {
    vec![Passo::texto(
        "Confira no próprio Claude Desktop se você entrou com a sua conta: este app não verifica o login do Desktop.",
    )]
}
