//! Registro do que o preparo trocou, para o desfazer restaurar exatamente (#213).
//!
//! Os hooks ficam no escopo do usuário e são compartilhados por todas as instalações
//! do app (a de desenvolvimento, a de teste, a instalada). Quando uma instalação
//! prepara um projeto e os hooks apontam para outra, o preparo os troca; sem guardar o
//! valor anterior, o desfazer não tinha como devolvê-los.
//!
//! O registro fica ao lado do `settings.json` ([`ConfigClaude::registro`]), em
//! `<settings>.cba-anterior`, e guarda, por instalação (o caminho do executável):
//!
//! - para cada evento de hook que ela mudou, o valor de `hooks.<evento>` antes
//!   (`None` se não existia) e o que ela gravou;
//! - para cada pasta (chave de `projects`) em que ela mudou o servidor MCP, o servidor
//!   `cba` anterior (de outra instalação, se havia), o que ela gravou e se a entrada da
//!   pasta e o `mcpServers` já existiam;
//! - para cada pasta que ela marcou como confiável (`hasTrustDialogAccepted`, #218),
//!   o valor anterior do campo (`None` se não existia).
//!
//! O primeiro "antes" fica: preparar de novo só atualiza o "depois". Só entram
//! entradas no formato do app (`"<…/cba>" hook <evento>`, `<…/cba> mcp`) e os grupos
//! de hooks do evento; nenhum outro campo dos arquivos do Claude é copiado.
//!
//! [`ConfigClaude::registro`]: super::ConfigClaude::registro

use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::Erro;
use super::json::Json;

/// Sufixo do registro, ao lado do `settings.json`.
pub const SUFIXO_REGISTRO: &str = ".cba-anterior";

/// Troca de um evento de hook.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrocaHook {
    /// `hooks.<evento>` antes da primeira troca; `None` se não existia.
    #[serde(default)]
    pub antes: Option<Json>,
    /// `hooks.<evento>` que a instalação gravou por último.
    pub depois: Json,
}

/// Troca do servidor MCP de uma pasta.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrocaMcp {
    /// A entrada da pasta em `projects` já existia.
    pub projeto_existia: bool,
    /// O `mcpServers` da pasta já existia.
    pub servidores_existia: bool,
    /// Servidor `cba` anterior (de outra instalação do app); `None` se não havia.
    #[serde(default)]
    pub antes: Option<Json>,
    /// O que a instalação gravou.
    pub depois: Json,
}

/// Confiança na pasta (`projects["<chave>"].hasTrustDialogAccepted`) que a instalação
/// marcou (#218).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrocaConfianca {
    /// Valor do campo antes; `None` se não existia.
    #[serde(default)]
    pub antes: Option<Json>,
}

/// O que uma instalação trocou.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Instalacao {
    /// A chave `hooks` existia no `settings.json` antes da primeira troca.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hooks_existia: Option<bool>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub hooks: BTreeMap<String, TrocaHook>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub mcp: BTreeMap<String, TrocaMcp>,
    /// Por chave de `projects`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub confianca: BTreeMap<String, TrocaConfianca>,
}

impl Instalacao {
    pub fn vazia(&self) -> bool {
        self.hooks.is_empty() && self.mcp.is_empty() && self.confianca.is_empty()
    }
}

/// Conteúdo do registro.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Registro {
    #[serde(default = "versao")]
    pub versao: u32,
    /// Por caminho do executável.
    #[serde(default)]
    pub instalacoes: BTreeMap<String, Instalacao>,
}

fn mesmo(a: &Json, b: &Json) -> bool {
    a.para_value() == b.para_value()
}

fn versao() -> u32 {
    1
}

impl Registro {
    /// Lê o registro. Ausente, vazio ou ilegível vira registro vazio: o desfazer cai
    /// no comportamento sem registro, que nunca apaga o que não é do app.
    pub fn ler(caminho: &Path) -> Self {
        match fs::read(caminho) {
            Ok(b) if b.iter().all(u8::is_ascii_whitespace) => Self::default(),
            Ok(b) => serde_json::from_slice(&b).unwrap_or_else(|_| {
                tracing::warn!("registro do preparo ilegível; ignorado");
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }

    /// Grava o registro (atômico); sem nada registrado, apaga o arquivo.
    pub fn gravar(&self, caminho: &Path) -> Result<(), Erro> {
        let erro = |c: &Path, e: io::Error| Erro::Arquivo {
            arquivo: c.to_path_buf(),
            tipo: e.kind(),
        };
        if self.instalacoes.values().all(Instalacao::vazia) {
            return match fs::remove_file(caminho) {
                Err(e) if e.kind() != io::ErrorKind::NotFound => Err(erro(caminho, e)),
                _ => Ok(()),
            };
        }
        let pasta = caminho
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        fs::create_dir_all(pasta).map_err(|e| erro(pasta, e))?;
        let mut texto = serde_json::to_string_pretty(self).unwrap_or_default();
        texto.push('\n');
        let temporario: PathBuf = pasta.join(format!(".cba-anterior.{}.tmp", std::process::id()));
        let resultado = (|| {
            let mut f = fs::File::create(&temporario).map_err(|e| erro(&temporario, e))?;
            f.write_all(texto.as_bytes())
                .and_then(|_| f.sync_all())
                .map_err(|e| erro(&temporario, e))?;
            drop(f);
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&temporario, fs::Permissions::from_mode(0o600))
                    .map_err(|e| erro(&temporario, e))?;
            }
            fs::rename(&temporario, caminho).map_err(|e| erro(caminho, e))
        })();
        if resultado.is_err() {
            let _ = fs::remove_file(&temporario);
        }
        resultado
    }

    /// Tira do registro a troca do servidor MCP da pasta `chave` feita pela
    /// instalação `exe`, e emenda as das outras: o "antes" de outra instalação que era
    /// o "depois" desta passa a ser o "antes" desta (a camada dela sai da pilha).
    pub fn tirar_mcp(&mut self, exe: &str, chave: &str) -> Option<TrocaMcp> {
        let instalacao = self.instalacoes.get_mut(exe)?;
        let saiu = instalacao.mcp.remove(chave)?;
        self.limpar(exe);
        for outra in self.instalacoes.values_mut() {
            if let Some(troca) = outra.mcp.get_mut(chave)
                && troca.antes.as_ref().is_some_and(|a| mesmo(a, &saiu.depois))
            {
                troca.antes = saiu.antes.clone();
                troca.projeto_existia = saiu.projeto_existia;
                troca.servidores_existia = saiu.servidores_existia;
            }
        }
        Some(saiu)
    }

    /// Tira do registro as trocas de hooks da instalação `exe` (com o `hooks_existia`
    /// dela), emendando as das outras como em [`Registro::tirar_mcp`].
    pub fn tirar_hooks(
        &mut self,
        exe: &str,
    ) -> Option<(Option<bool>, BTreeMap<String, TrocaHook>)> {
        let instalacao = self.instalacoes.get_mut(exe)?;
        let hooks = std::mem::take(&mut instalacao.hooks);
        let existia = instalacao.hooks_existia.take();
        self.limpar(exe);
        for outra in self.instalacoes.values_mut() {
            for (evento, troca) in &mut outra.hooks {
                if let Some(minha) = hooks.get(evento)
                    && troca
                        .antes
                        .as_ref()
                        .is_some_and(|a| mesmo(a, &minha.depois))
                {
                    troca.antes = minha.antes.clone();
                }
            }
            // Se uma das duas achou a chave ausente, ela não existia antes de ambas.
            if let (Some(a), Some(b)) = (outra.hooks_existia, existia) {
                outra.hooks_existia = Some(a && b);
            }
        }
        Some((existia, hooks))
    }

    /// Tira do registro a confiança na pasta `chave` marcada pela instalação `exe`.
    /// Se outra instalação ainda tem o servidor MCP nessa pasta, a confiança passa a
    /// ser dela (ela a tira quando desfizer) e a resposta diz que foi transferida.
    pub fn tirar_confianca(&mut self, exe: &str, chave: &str) -> Option<(TrocaConfianca, bool)> {
        let saiu = self.instalacoes.get_mut(exe)?.confianca.remove(chave)?;
        self.limpar(exe);
        let herdeira = self
            .instalacoes
            .iter_mut()
            .find(|(e, i)| e.as_str() != exe && i.mcp.contains_key(chave));
        let transferida = match herdeira {
            Some((_, outra)) => {
                outra
                    .confianca
                    .entry(chave.to_string())
                    .or_insert_with(|| saiu.clone());
                true
            }
            None => false,
        };
        Some((saiu, transferida))
    }

    /// Tira a instalação que ficou sem trocas.
    fn limpar(&mut self, exe: &str) {
        if self.instalacoes.get(exe).is_some_and(Instalacao::vazia) {
            self.instalacoes.remove(exe);
        }
    }
}
