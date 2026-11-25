//! Registro do que o preparo trocou, para o desfazer restaurar exatamente (#213).
//!
//! Os hooks e o servidor MCP são compartilhados por todas as instalações do app (a de
//! desenvolvimento, a de teste, a instalada). Quando uma instalação prepara um projeto
//! e as entradas apontam para outra, o preparo as troca; sem guardar o valor anterior,
//! o desfazer não tinha como devolvê-las.
//!
//! O registro fica ao lado do `settings.json` ([`ConfigClaude::registro`]), em
//! `<settings>.cba-anterior`, fora das pastas dos projetos, e guarda, por instalação
//! (o caminho do executável):
//!
//! - para cada raiz de pasta preparada (#304), o que ela mudou no
//!   `<raiz>/.claude/settings.local.json` ([`TrocaLocal`]): se o arquivo e a pasta
//!   `.claude` já existiam, o valor de cada `hooks.<evento>` antes e o que ela gravou,
//!   se foi ela que pôs a linha do arquivo no `info/exclude` do git e as regras de
//!   permissão dos workflows que ela pôs em `permissions.allow` (#370);
//! - para cada raiz de workflows (#370), os arquivos `.claude/workflows/cba-*.js` que
//!   ela gravou, com o conteúdo de antes se havia, se as pastas `.claude` e
//!   `.claude/workflows` já existiam e se foi ela que pôs a linha dos workflows no
//!   `info/exclude` ([`TrocaWorkflows`]);
//! - para cada evento de hook que ela mudou no `settings.json` do usuário (preparos
//!   anteriores à #304, que gravavam os hooks no escopo do usuário), o valor de
//!   `hooks.<evento>` antes (`None` se não existia) e o que ela gravou;
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

/// O que uma instalação trocou no `<raiz>/.claude/settings.local.json` de uma raiz
/// (#304).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrocaLocal {
    /// O `settings.local.json` já existia antes da primeira troca.
    pub arquivo_existia: bool,
    /// A pasta `.claude` da raiz já existia antes da primeira troca.
    pub pasta_existia: bool,
    /// A chave `hooks` existia no arquivo antes da primeira troca.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hooks_existia: Option<bool>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub hooks: BTreeMap<String, TrocaHook>,
    /// Foi esta instalação que pôs o arquivo no `info/exclude` do git.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub exclude: bool,
    /// Regras de permissão dos workflows (`Workflow(cba-<tipo>)`, `Skill(cba-<tipo>)`)
    /// que esta instalação pôs em `permissions.allow` (#370). As que já estavam lá
    /// não entram: o desfazer não as tira.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub permissoes: Vec<String>,
    /// A chave `permissions` existia no arquivo antes da primeira regra posta.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permissoes_existia: Option<bool>,
    /// A lista `permissions.allow` existia antes da primeira regra posta.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_existia: Option<bool>,
}

impl TrocaLocal {
    pub fn vazia(&self) -> bool {
        self.hooks.is_empty() && !self.exclude && self.permissoes.is_empty()
    }
}

/// Um workflow instalado por uma instalação em `<raiz>/.claude/workflows/` (#370).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TrocaArquivo {
    /// Conteúdo de antes da primeira gravação (um arquivo do app, de outra
    /// instalação ou de um preparo sem registro); `None` se o arquivo não existia.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub antes: Option<String>,
}

/// O que uma instalação gravou na pasta `.claude/workflows` de uma raiz de workflows
/// (`git rev-parse --show-toplevel` da pasta, ou a pasta fora do git; #370).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrocaWorkflows {
    /// A pasta `.claude` da raiz já existia antes da primeira gravação.
    pub pasta_existia: bool,
    /// A pasta `.claude/workflows` já existia antes da primeira gravação.
    pub pasta_workflows_existia: bool,
    /// Por nome do workflow (`cba-impacto`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub arquivos: BTreeMap<String, TrocaArquivo>,
    /// Foi esta instalação que pôs a linha dos workflows no `info/exclude` do git.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub exclude: bool,
}

impl TrocaWorkflows {
    pub fn vazia(&self) -> bool {
        self.arquivos.is_empty() && !self.exclude
    }
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
    /// Por raiz (caminho, como em [`super::Alvo::raizes`]).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub locais: BTreeMap<String, TrocaLocal>,
    /// Por raiz de workflows (caminho, como em [`super::Alvo::raizes_workflows`]).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub workflows: BTreeMap<String, TrocaWorkflows>,
}

impl Instalacao {
    pub fn vazia(&self) -> bool {
        self.hooks.is_empty()
            && self.mcp.is_empty()
            && self.confianca.is_empty()
            && self.locais.values().all(TrocaLocal::vazia)
            && self.workflows.values().all(TrocaWorkflows::vazia)
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

/// Versão gravada: a 2 acrescenta [`Instalacao::locais`] (#304); a 3, as regras de
/// permissão dos workflows em [`TrocaLocal`] e [`Instalacao::workflows`] (#370).
pub const VERSAO: u32 = 3;

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
        let mut atual = self.clone();
        atual.versao = VERSAO;
        for i in atual.instalacoes.values_mut() {
            i.locais.retain(|_, t| !t.vazia());
            i.workflows.retain(|_, t| !t.vazia());
        }
        let mut texto = serde_json::to_string_pretty(&atual).unwrap_or_default();
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

    /// Tira do registro as trocas desta instalação no arquivo local da `raiz`,
    /// emendando as das outras instalações na mesma raiz como em
    /// [`Registro::tirar_mcp`]. O segundo valor diz se outra instalação ainda tem
    /// trocas nessa raiz (e passa a responder pela linha do `info/exclude`).
    pub fn tirar_local(&mut self, exe: &str, raiz: &str) -> Option<(TrocaLocal, bool)> {
        let saiu = self.instalacoes.get_mut(exe)?.locais.remove(raiz)?;
        self.limpar(exe);
        let mut outra_usa = false;
        for outra in self.instalacoes.values_mut() {
            let Some(troca) = outra.locais.get_mut(raiz) else {
                continue;
            };
            outra_usa = true;
            for (evento, t) in &mut troca.hooks {
                if let Some(minha) = saiu.hooks.get(evento)
                    && t.antes.as_ref().is_some_and(|a| mesmo(a, &minha.depois))
                {
                    t.antes = minha.antes.clone();
                }
            }
            troca.arquivo_existia &= saiu.arquivo_existia;
            troca.pasta_existia &= saiu.pasta_existia;
            if let (Some(a), Some(b)) = (troca.hooks_existia, saiu.hooks_existia) {
                troca.hooks_existia = Some(a && b);
            }
            // A linha do exclude e as regras de permissão passam a ser da que fica.
            troca.exclude |= saiu.exclude;
            for r in &saiu.permissoes {
                if !troca.permissoes.contains(r) {
                    troca.permissoes.push(r.clone());
                }
            }
            if !saiu.permissoes.is_empty() {
                let e = |a: Option<bool>, b: Option<bool>| match (a, b) {
                    (Some(a), Some(b)) => Some(a && b),
                    (a, b) => a.or(b),
                };
                troca.permissoes_existia = e(troca.permissoes_existia, saiu.permissoes_existia);
                troca.allow_existia = e(troca.allow_existia, saiu.allow_existia);
            }
        }
        Some((saiu, outra_usa))
    }

    /// Tira do registro os workflows que a instalação `exe` gravou na raiz `raiz`.
    /// Se outra instalação também gravou nessa raiz, os arquivos e a linha do
    /// `info/exclude` passam a ser dela (ela os tira quando desfizer), e o segundo
    /// valor é `true`.
    pub fn tirar_workflows(&mut self, exe: &str, raiz: &str) -> Option<(TrocaWorkflows, bool)> {
        let saiu = self.instalacoes.get_mut(exe)?.workflows.remove(raiz)?;
        self.limpar(exe);
        let mut outra_usa = false;
        for outra in self.instalacoes.values_mut() {
            let Some(troca) = outra.workflows.get_mut(raiz) else {
                continue;
            };
            outra_usa = true;
            for (nome, t) in &saiu.arquivos {
                troca
                    .arquivos
                    .entry(nome.clone())
                    .or_insert_with(|| t.clone());
            }
            troca.pasta_existia &= saiu.pasta_existia;
            troca.pasta_workflows_existia &= saiu.pasta_workflows_existia;
            troca.exclude |= saiu.exclude;
        }
        Some((saiu, outra_usa))
    }

    /// Troca local registrada pela instalação `exe` na `raiz`.
    pub fn local(&self, exe: &str, raiz: &str) -> Option<&TrocaLocal> {
        self.instalacoes.get(exe)?.locais.get(raiz)
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
