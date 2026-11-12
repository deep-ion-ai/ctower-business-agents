//! Detecção dos clientes de IA instalados e logados (spec, seções 4.1, 6.3, 6.4 e 9).
//!
//! O primeiro uso e a tela inicial mostram ao analista o que falta: o Claude Code
//! (instalado? qual versão? logado?) e o Claude Desktop (instalado?). Quando falta algo,
//! [`orientacao`] traz o passo a passo oficial da Anthropic.
//!
//! # Como cada coisa é detectada
//!
//! - **Claude Code instalado:** procura `claude` nas pastas do `PATH` e depois nos locais
//!   padrão de instalação (o `PATH` de um app aberto pelo Finder ou pelo menu Iniciar
//!   costuma não ter `~/.local/bin`). Só consulta metadados dos caminhos.
//! - **Versão:** `claude --version`.
//! - **Login do Claude Code:** só pelo comando oficial `claude auth status --json`. O app
//!   lê apenas os campos `loggedIn`, `authMethod` e `apiProvider` da resposta e descarta
//!   o resto (e-mail, organização), que não é guardado nem vai para o log. Nenhum
//!   arquivo de credencial ou de configuração do Claude é aberto, e o app não consulta o
//!   chaveiro do sistema operacional. Versões antigas sem esse comando resultam em
//!   [`LoginClaudeCode::Indeterminado`], com a orientação de atualizar.
//! - **Claude Desktop instalado:** existência do aplicativo nos locais de instalação
//!   (macOS: `Claude.app` em `/Applications` ou `~/Applications`; Windows: instalação
//!   por usuário em `%LOCALAPPDATA%\AnthropicClaude` ou pacote MSIX `Claude_*` em
//!   `%LOCALAPPDATA%\Packages`). Não existe no Linux.
//! - **Login do Claude Desktop:** não é verificado. O Desktop não tem comando de status
//!   documentado, e descobrir o login exigiria ler os dados do aplicativo. A tela pede ao
//!   analista que confira no próprio Desktop.
//!
//! Tudo o que toca o projeto passa por [`Ambiente`], o que permite testar sem o Claude
//! instalado e garantir por teste que nenhum arquivo é aberto.

mod ambiente;
pub mod orientacao;

#[cfg(test)]
mod tests;

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;
use serde_json::Value;

pub use ambiente::{Ambiente, AmbienteReal, ErroExecucao, Plataforma, Saida};
pub use orientacao::Passo;

/// Limite para `claude --version`.
pub const LIMITE_VERSAO: Duration = Duration::from_secs(10);
/// Limite para `claude auth status`, que pode consultar a rede para renovar o login.
pub const LIMITE_STATUS: Duration = Duration::from_secs(20);

/// Estado dos dois clientes, como a interface mostra.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EstadoClientes {
    pub plataforma: Plataforma,
    pub claude_code: EstadoClaudeCode,
    pub claude_desktop: EstadoClaudeDesktop,
}

/// Claude Code CLI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EstadoClaudeCode {
    pub instalacao: InstalacaoClaudeCode,
    pub login: LoginClaudeCode,
    /// Passo a passo oficial para o que falta (vazio se está tudo certo).
    pub orientacao: Vec<Passo>,
}

/// Onde o executável `claude` foi achado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum OrigemCaminho {
    /// Numa pasta do `PATH`.
    Path,
    /// Num local padrão de instalação, fora do `PATH`.
    LocalPadrao,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "estado",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum InstalacaoClaudeCode {
    NaoEncontrado,
    Encontrado {
        caminho: PathBuf,
        origem: OrigemCaminho,
        /// Versão informada por `claude --version`; `None` se o comando falhou.
        versao: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "estado",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum LoginClaudeCode {
    /// O Claude Code não está instalado, então não há o que verificar.
    NaoVerificado,
    Logado {
        /// Valor de `authMethod` (ex.: `claude.ai`, chave de API), se informado.
        metodo: Option<String>,
        /// Valor de `apiProvider` (ex.: `firstParty`, Bedrock, Vertex), se informado.
        provedor: Option<String>,
    },
    NaoLogado,
    /// O comando de status não deu uma resposta reconhecível.
    Indeterminado {
        motivo: String,
    },
}

/// Claude Desktop.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EstadoClaudeDesktop {
    pub instalacao: InstalacaoClaudeDesktop,
    /// Passo a passo oficial. Mesmo instalado, lembra o analista de entrar com a
    /// própria conta, porque o login do Desktop não é verificado.
    pub orientacao: Vec<Passo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "estado",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum InstalacaoClaudeDesktop {
    /// O Claude Desktop não existe para esta plataforma (Linux).
    NaoSuportado,
    NaoEncontrado,
    Encontrado {
        caminho: PathBuf,
    },
}

/// Detecta os dois clientes no sistema de verdade. Bloqueia enquanto os comandos do
/// Claude Code rodam (até [`LIMITE_VERSAO`] + [`LIMITE_STATUS`]); chame fora da thread
/// da interface.
pub fn detectar() -> EstadoClientes {
    detectar_em(&AmbienteReal)
}

/// Detecta os dois clientes usando `ambiente`.
pub fn detectar_em(ambiente: &dyn Ambiente) -> EstadoClientes {
    let plataforma = ambiente.plataforma();
    let claude_code = detectar_claude_code(ambiente);
    let claude_desktop = detectar_claude_desktop(ambiente);
    tracing::info!(
        code_instalado = matches!(
            claude_code.instalacao,
            InstalacaoClaudeCode::Encontrado { .. }
        ),
        code_login = nome_login(&claude_code.login),
        desktop = nome_desktop(&claude_desktop.instalacao),
        "clientes de IA detectados"
    );
    EstadoClientes {
        plataforma,
        claude_code,
        claude_desktop,
    }
}

/// Detecta o Claude Code: caminho, versão e login.
pub fn detectar_claude_code(ambiente: &dyn Ambiente) -> EstadoClaudeCode {
    let plataforma = ambiente.plataforma();
    let Some((caminho, origem)) = localizar_claude(ambiente) else {
        return EstadoClaudeCode {
            instalacao: InstalacaoClaudeCode::NaoEncontrado,
            login: LoginClaudeCode::NaoVerificado,
            orientacao: orientacao::claude_code_instalar(plataforma),
        };
    };
    let versao = ambiente
        .executar(&caminho, &["--version"], LIMITE_VERSAO)
        .ok()
        .filter(|s| s.codigo == Some(0))
        .and_then(|s| versao_da_saida(&s.stdout));
    let login =
        login_do_status(ambiente.executar(&caminho, &["auth", "status", "--json"], LIMITE_STATUS));
    let orientacao = match login {
        LoginClaudeCode::Logado { .. } => Vec::new(),
        LoginClaudeCode::NaoLogado => orientacao::claude_code_login(),
        _ => orientacao::claude_code_verificar(plataforma),
    };
    EstadoClaudeCode {
        instalacao: InstalacaoClaudeCode::Encontrado {
            caminho,
            origem,
            versao,
        },
        login,
        orientacao,
    }
}

/// Detecta o Claude Desktop pela existência do aplicativo.
pub fn detectar_claude_desktop(ambiente: &dyn Ambiente) -> EstadoClaudeDesktop {
    let plataforma = ambiente.plataforma();
    let instalacao = match plataforma {
        Plataforma::Linux => InstalacaoClaudeDesktop::NaoSuportado,
        _ => match localizar_desktop(ambiente) {
            Some(caminho) => InstalacaoClaudeDesktop::Encontrado { caminho },
            None => InstalacaoClaudeDesktop::NaoEncontrado,
        },
    };
    let orientacao = match instalacao {
        InstalacaoClaudeDesktop::NaoSuportado => Vec::new(),
        InstalacaoClaudeDesktop::NaoEncontrado => orientacao::claude_desktop_instalar(),
        InstalacaoClaudeDesktop::Encontrado { .. } => orientacao::claude_desktop_login(),
    };
    EstadoClaudeDesktop {
        instalacao,
        orientacao,
    }
}

/// Procura o executável do Claude Code: primeiro no `PATH`, depois nos locais padrão.
pub fn localizar_claude(ambiente: &dyn Ambiente) -> Option<(PathBuf, OrigemCaminho)> {
    let plataforma = ambiente.plataforma();
    let nomes = nomes_executavel(ambiente);
    if let Some(path) = ambiente.var("PATH") {
        let path = path.to_string_lossy().into_owned();
        for pasta in path.split(plataforma.separador_path()) {
            // Aspas aparecem no PATH do Windows em pastas com `;` ou espaço.
            let pasta = pasta.trim().trim_matches('"');
            if pasta.is_empty() {
                continue;
            }
            for nome in &nomes {
                let candidato = Path::new(pasta).join(nome);
                if ambiente.e_arquivo(&candidato) {
                    return Some((candidato, OrigemCaminho::Path));
                }
            }
        }
    }
    locais_padrao_claude(ambiente)
        .into_iter()
        .find(|c| ambiente.e_arquivo(c))
        .map(|c| (c, OrigemCaminho::LocalPadrao))
}

/// Nomes de arquivo que valem como `claude` nesta plataforma.
fn nomes_executavel(ambiente: &dyn Ambiente) -> Vec<String> {
    match ambiente.plataforma() {
        Plataforma::Windows => {
            let pathext = ambiente
                .var("PATHEXT")
                .map(|v| v.to_string_lossy().into_owned())
                .unwrap_or_else(|| ".COM;.EXE;.BAT;.CMD".into());
            pathext
                .split(';')
                .map(|e| e.trim().to_ascii_lowercase())
                // Só o que o `Command` sabe executar direto.
                .filter(|e| matches!(e.as_str(), ".exe" | ".cmd" | ".bat" | ".com"))
                .map(|e| format!("claude{e}"))
                .collect()
        }
        _ => vec!["claude".into()],
    }
}

/// Locais padrão de instalação do Claude Code, na ordem de preferência.
pub fn locais_padrao_claude(ambiente: &dyn Ambiente) -> Vec<PathBuf> {
    let home = ambiente.home();
    let mut locais = Vec::new();
    match ambiente.plataforma() {
        Plataforma::Windows => {
            if let Some(home) = &home {
                // Instalador nativo (`irm https://claude.ai/install.ps1 | iex`).
                locais.push(home.join(".local").join("bin").join("claude.exe"));
            }
            if let Some(appdata) = ambiente.var("APPDATA") {
                // `npm install -g @anthropic-ai/claude-code`.
                locais.push(PathBuf::from(appdata).join("npm").join("claude.cmd"));
            }
        }
        plataforma => {
            if let Some(home) = &home {
                // Instalador nativo (`curl -fsSL https://claude.ai/install.sh | bash`).
                locais.push(home.join(".local").join("bin").join("claude"));
                // Instalação local antiga (`claude migrate-installer`).
                locais.push(home.join(".claude").join("local").join("claude"));
            }
            if plataforma == Plataforma::MacOs {
                // Homebrew em Apple Silicon.
                locais.push(PathBuf::from("/opt/homebrew/bin/claude"));
            }
            locais.push(PathBuf::from("/usr/local/bin/claude"));
        }
    }
    locais
}

/// Procura o Claude Desktop nos locais de instalação (macOS e Windows).
pub fn localizar_desktop(ambiente: &dyn Ambiente) -> Option<PathBuf> {
    match ambiente.plataforma() {
        Plataforma::MacOs => {
            let mut locais = vec![PathBuf::from("/Applications/Claude.app")];
            if let Some(home) = ambiente.home() {
                locais.push(home.join("Applications").join("Claude.app"));
            }
            locais.into_iter().find(|c| ambiente.e_pasta(c))
        }
        Plataforma::Windows => {
            let local = PathBuf::from(ambiente.var("LOCALAPPDATA")?);
            // Instalador por usuário (Squirrel).
            let squirrel = local.join("AnthropicClaude");
            if ambiente.e_pasta(&squirrel) {
                return Some(squirrel);
            }
            // Pacote MSIX: a pasta de dados do pacote tem o nome da família
            // (`Claude_<id do publicador>`). Só o nome é listado, nada é aberto.
            let pacotes = local.join("Packages");
            ambiente
                .nomes_na_pasta(&pacotes)
                .into_iter()
                .find(e_pacote_claude)
                .map(|n| pacotes.join(n))
        }
        Plataforma::Linux => None,
    }
}

fn e_pacote_claude(nome: &OsString) -> bool {
    nome.to_str().is_some_and(|n| {
        n.len() > "Claude_".len()
            && n.get(..7)
                .is_some_and(|p| p.eq_ignore_ascii_case("Claude_"))
    })
}

/// Extrai a versão de `claude --version` (ex.: `2.1.0 (Claude Code)` → `2.1.0`).
pub fn versao_da_saida(stdout: &str) -> Option<String> {
    let primeira = stdout.split_whitespace().next()?;
    let valida = primeira.len() <= 40
        && primeira.starts_with(|c: char| c.is_ascii_digit())
        && primeira
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+'));
    valida.then(|| primeira.to_string())
}

/// Interpreta o resultado de `claude auth status --json`.
///
/// Só os campos `loggedIn`, `authMethod` e `apiProvider` são lidos; o resto da
/// resposta (e-mail, organização, datas) é descartado aqui e não chega ao log.
pub fn login_do_status(resultado: Result<Saida, ErroExecucao>) -> LoginClaudeCode {
    let saida = match resultado {
        Ok(saida) => saida,
        Err(ErroExecucao::TempoEsgotado) => {
            return LoginClaudeCode::Indeterminado {
                motivo: "o comando `claude auth status` não respondeu a tempo".into(),
            };
        }
        Err(ErroExecucao::NaoIniciou(tipo)) => {
            return LoginClaudeCode::Indeterminado {
                motivo: format!("não foi possível executar o `claude` ({tipo})"),
            };
        }
    };
    let json = serde_json::from_str::<Value>(saida.stdout.trim()).ok();
    match json
        .as_ref()
        .and_then(|v| v.get("loggedIn"))
        .and_then(Value::as_bool)
    {
        Some(true) => {
            let campo = |nome: &str| {
                json.as_ref()
                    .and_then(|v| v.get(nome))
                    .and_then(Value::as_str)
                    .map(str::to_string)
            };
            LoginClaudeCode::Logado {
                metodo: campo("authMethod"),
                provedor: campo("apiProvider"),
            }
        }
        Some(false) => LoginClaudeCode::NaoLogado,
        None => LoginClaudeCode::Indeterminado {
            motivo: if saida.codigo == Some(0) {
                "resposta de `claude auth status` sem o campo `loggedIn`".into()
            } else {
                "esta versão do Claude Code não respondeu a `claude auth status`".into()
            },
        },
    }
}

fn nome_login(login: &LoginClaudeCode) -> &'static str {
    match login {
        LoginClaudeCode::NaoVerificado => "nao_verificado",
        LoginClaudeCode::Logado { .. } => "logado",
        LoginClaudeCode::NaoLogado => "nao_logado",
        LoginClaudeCode::Indeterminado { .. } => "indeterminado",
    }
}

fn nome_desktop(instalacao: &InstalacaoClaudeDesktop) -> &'static str {
    match instalacao {
        InstalacaoClaudeDesktop::NaoSuportado => "nao_suportado",
        InstalacaoClaudeDesktop::NaoEncontrado => "nao_encontrado",
        InstalacaoClaudeDesktop::Encontrado { .. } => "encontrado",
    }
}
