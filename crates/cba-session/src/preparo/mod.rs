//! Preparo do ambiente: servidor MCP e hooks na configuração do Claude Code (spec,
//! seção 6.2).
//!
//! Qualquer sessão do Claude Code (terminal embutido, terminal do analista ou aba de
//! código do Claude Desktop) aberta na pasta de um projeto cadastrado precisa enxergar
//! o servidor MCP do app e mandar os eventos para o receptor de hooks, sem afetar as
//! sessões dos outros projetos do analista (#302).
//!
//! # O que é gravado, e onde
//!
//! | Item | Arquivo | Escopo |
//! |---|---|---|
//! | Servidor MCP `cba` (`<executável> mcp`) | `~/.claude.json`, em `projects["<chave>"].mcpServers` | local: só sessões nessa pasta |
//! | Confiança na pasta (`hasTrustDialogAccepted: true`, #218) | `~/.claude.json`, em `projects["<chave>"]` | local: só sessões nessa pasta |
//! | Hooks dos eventos de [`crate::hooks::EVENTOS`] (`"<executável>" hook <evento>`) | `<raiz>/.claude/settings.local.json`, em `hooks`, para cada raiz de [`Alvo::raizes`] | local: só sessões nessa raiz |
//! | Linha [`LINHA_EXCLUDE`], se a raiz é de um repositório git | `info/exclude` do diretório comum do git | não versionado |
//!
//! | Opcional: proteção contra edição, `Edit(//<pasta>/**)` em `permissions.deny` ([`protecao`]) | `<raiz>/.claude/settings.local.json` da pasta | local, com caminho absoluto |
//!
//! A `<raiz>` de uma pasta é o caminho da chave do escopo local: a raiz do repositório
//! principal, ou a própria pasta fora do git. Com `CLAUDE_CONFIG_DIR` definido, o
//! estado e o `settings.json` do usuário ficam nessa pasta (`<dir>/.claude.json` e
//! `<dir>/settings.json`). O preparo não grava mais nada no `settings.json` do
//! usuário; os hooks que versões anteriores gravaram lá saem no desfazer
//! ([`Preparador::desfazer`]) e na migração (#305).
//!
//! Fontes (documentação oficial do Claude Code, consultada em 2026-11-11):
//! - <https://code.claude.com/docs/en/mcp.md>: o escopo local fica em `~/.claude.json`
//!   sob o caminho do projeto (`projects.<caminho>.mcpServers`); entrada sem `type` é
//!   stdio.
//! - <https://code.claude.com/docs/en/hooks.md>: formato `hooks.<evento>[].hooks[]`
//!   com `type: "command"`, `command` e `timeout` (segundos); sem `matcher`, o grupo
//!   vale para tudo; `UserPromptSubmit` e `Stop` não aceitam `matcher`. Locais
//!   documentados para hooks: `~/.claude/settings.json`, `.claude/settings.json` e
//!   `.claude/settings.local.json` do projeto, política gerenciada, plugins. O
//!   `~/.claude.json` não é local de hooks.
//! - A confiança na pasta não está documentada. Lido no código do Claude Code 2.1.325:
//!   o diálogo "Quick safety check" não aparece se `projects["<chave>"]` tem
//!   `hasTrustDialogAccepted: true`, com a mesma chave do escopo local do MCP (a raiz
//!   do repositório principal, ou a pasta fora do git). É o campo que o próprio Claude
//!   grava quando o analista escolhe "Yes, I trust this folder".
//! - <https://code.claude.com/docs/en/settings.md> e
//!   <https://code.claude.com/docs/en/claude-directory.md>: `CLAUDE_CONFIG_DIR` muda
//!   `~/.claude` e `~/.claude.json`; o Desktop lê os mesmos arquivos de configuração.
//! - Spike da #303 (Claude Code 2.1.330, `docs/specs/2026-11-23-spike-settings-local.md`):
//!   a sessão lê o `.claude/settings.local.json` da pasta em que começou e o da raiz
//!   do repositório principal (também numa worktree), sem subir pelas pastas
//!   intermediárias; fora do git, só o da pasta. Os hooks desse arquivo só disparam
//!   com a pasta confiável (o `hasTrustDialogAccepted` acima basta). O mesmo comando
//!   em dois escopos roda uma vez só. O Claude Code não tira do git um arquivo criado
//!   por fora: daí o `info/exclude`.
//!
//! # Por que esses escopos
//!
//! - **MCP no escopo local** (e não no de projeto, `.mcp.json`) porque o de projeto
//!   é versionado, e não no de usuário porque o servidor só faz sentido nas pastas
//!   cadastradas.
//! - **A chave do escopo local** não está documentada. Verificado com o Claude Code
//!   2.1.322 (`claude mcp add-json --scope local` com `CLAUDE_CONFIG_DIR` temporário):
//!   numa pasta dentro de um repositório git, a chave é a raiz do repositório; numa
//!   worktree, a raiz do repositório **principal**; fora de repositório, a própria
//!   pasta. É o que [`Alvo::da_pasta`] calcula, com [`cba_index::detectar_git`].
//! - **Hooks no `.claude/settings.local.json` da raiz** (#302, #304): é o único
//!   escopo por pasta para hooks fora do versionamento. No escopo do usuário (o de
//!   antes) eles disparavam em toda sessão do analista, em qualquer pasta, e davam
//!   erro em todas se o executável sumisse. Na raiz (e não na pasta cadastrada)
//!   porque é o arquivo que as sessões nas subpastas e nas worktrees também leem.
//!   Pastas que compartilham a raiz compartilham o arquivo: o desfazer não tira os
//!   hooks de uma raiz que é a pasta de outro projeto preparado por esta instalação.
//!
//! # Garantias
//!
//! - **Idempotente:** preparar de novo não muda nada se já está tudo certo; com outro
//!   executável (reinstalação em outro lugar), troca as entradas antigas em vez de
//!   duplicar.
//! - **Preserva o resto:** só as nossas entradas são tocadas. A ordem das chaves é
//!   mantida. Um servidor `cba` que não é nosso é conflito, e nada é gravado.
//! - **Reversível:** [`Preparador::desfazer`] devolve o que havia antes, inclusive os
//!   hooks e o servidor de outra instalação do app que o preparo trocou (#213). O
//!   valor de antes fica num registro ao lado do `settings.json` ([`registro`]), fora
//!   da pasta do projeto. O arquivo local e a pasta `.claude` saem se só existiam por
//!   causa do app, e a linha do `info/exclude` sai se foi o app que a pôs. O que não
//!   dá para devolver (mudou depois do preparo, ainda em uso por outra pasta, dados
//!   que o próprio Claude gravou) fica como está e é dito em [`Resultado::mantidos`].
//! - **Avisa da troca:** se um hook ou o servidor aponta para outra instalação do
//!   app, o plano diz qual ([`Alteracao::outra_instalacao`]).
//! - **Gravação atômica com cópia de segurança** (`<arquivo>.cba-backup` ao lado dos
//!   arquivos do usuário; a do arquivo local em `<pasta do settings.json>/cba-backups/`,
//!   [`ConfigClaude::backup_de`]), veja [`arquivo`].
//! - **Sem credenciais:** os arquivos são tratados como JSON genérico; nenhum campo
//!   além dos nossos é interpretado, registrado no log ou copiado para outro lugar.
//! - **Na pasta do projeto, só o arquivo local:** além do `.claude/settings.local.json`
//!   (e do `info/exclude`, dentro do `.git`), nada é escrito nas pastas do projeto. A
//!   indentação e as quebras de linha de cada arquivo são mantidas.
//!
//! O consentimento do analista fica com a interface: [`Preparador::planejar`] mostra
//! o que seria alterado sem gravar nada.
//!
//! # Uso
//!
//! ```no_run
//! use cba_session::preparo::{Alvo, Preparador};
//!
//! # fn main() -> Result<(), cba_session::preparo::Erro> {
//! let preparador = Preparador::padrao(std::env::current_exe().unwrap())?;
//! let alvo = Alvo::da_pasta("faturamento-exemplo", "/codigo/faturamento".as_ref())?;
//! let plano = preparador.planejar(&alvo)?; // para a tela de consentimento
//! let feito = preparador.preparar(&alvo)?; // `feito.diretorio`: onde rodar o `claude`
//! assert!(preparador.verificar(&alvo)?.pronto);
//! preparador.desfazer(&alvo)?; // no descadastro
//! # let _ = (plano, feito);
//! # Ok(())
//! # }
//! ```

pub mod arquivo;
mod json;
pub mod protecao;
pub mod registro;

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::clientes::{Ambiente, AmbienteReal, Plataforma};
use crate::hooks::EVENTOS;
use json::{Json, Objeto};
use registro::{Registro, TrocaConfianca, TrocaHook, TrocaLocal, TrocaMcp};

/// Nome do servidor MCP na configuração (as ferramentas aparecem como `mcp__cba__*`).
pub const NOME_SERVIDOR: &str = "cba";
/// Variável de ambiente com o identificador do projeto, passada ao servidor MCP.
pub const VAR_PROJETO: &str = "CBA_PROJETO";
/// Limite de tempo de cada hook, em segundos (o padrão do Claude Code é 600).
pub const LIMITE_HOOK_S: u64 = 10;
/// Campo da entrada da pasta em `projects` com a confiança do Claude Code na pasta.
pub const CAMPO_CONFIANCA: &str = "hasTrustDialogAccepted";
/// Nome do executável do app, sem extensão: identifica as nossas entradas.
const NOME_EXECUTAVEL: &str = "cba";
/// Pasta das configurações do Claude Code dentro de uma pasta de projeto.
pub const PASTA_LOCAL: &str = ".claude";
/// Configurações locais (pessoais, fora do versionamento) de uma pasta (#302).
pub const ARQUIVO_LOCAL: &str = "settings.local.json";
/// Linha do `info/exclude` do git que tira o arquivo local do versionamento.
pub const LINHA_EXCLUDE: &str = "/.claude/settings.local.json";
/// Pasta das cópias de segurança dos arquivos locais, ao lado do `settings.json`.
pub const PASTA_BACKUPS: &str = "cba-backups";

/// `<raiz>/.claude/settings.local.json`.
pub fn arquivo_local(raiz: &Path) -> PathBuf {
    raiz.join(PASTA_LOCAL).join(ARQUIVO_LOCAL)
}

/// Raiz do escopo local de uma pasta (já canônica): a raiz do repositório principal,
/// ou a própria pasta fora do git. É onde o Claude Code procura o
/// `.claude/settings.local.json` além da pasta da sessão (spike da #303).
fn raiz_local(diretorio: &Path) -> (PathBuf, bool) {
    match cba_index::detectar_git(diretorio) {
        Some(repo) => (repo.raiz_principal, repo.worktree),
        None => (diretorio.to_path_buf(), false),
    }
}

/// Erro do preparo. As mensagens nunca citam o conteúdo dos arquivos.
#[derive(Debug, thiserror::Error)]
pub enum Erro {
    #[error("não foi possível localizar a pasta do usuário para achar a configuração do Claude")]
    SemPastaDoUsuario,

    #[error("o caminho do executável precisa ser absoluto e sem aspas, $ ou crase: {0}")]
    ExecutavelInvalido(PathBuf),

    #[error("pasta do projeto inacessível: {0}")]
    Pasta(PathBuf),

    #[error("falha de acesso a {arquivo}: {tipo:?}")]
    Arquivo {
        arquivo: PathBuf,
        tipo: io::ErrorKind,
    },

    #[error("{arquivo} não é um JSON válido (linha {linha}); nada foi alterado")]
    JsonInvalido { arquivo: PathBuf, linha: usize },

    #[error("{arquivo}: o campo {campo} tem um formato inesperado; nada foi alterado")]
    Formato { arquivo: PathBuf, campo: String },

    #[error(
        "já existe um servidor MCP \"{NOME_SERVIDOR}\" que não é do app em {arquivo} (projeto {chave}); nada foi alterado"
    )]
    Conflito { arquivo: PathBuf, chave: String },

    #[error("{0} mudou durante a gravação várias vezes seguidas; tente de novo")]
    AlteradoDuranteGravacao(PathBuf),
}

/// Onde está a configuração do Claude Code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigClaude {
    /// `~/.claude.json`: estado do Claude Code, com o escopo local do MCP.
    pub estado: PathBuf,
    /// `~/.claude/settings.json`: configurações do usuário, com os hooks.
    pub settings: PathBuf,
}

impl ConfigClaude {
    /// Local padrão: `CLAUDE_CONFIG_DIR` se definido, senão a pasta do usuário.
    pub fn localizar(ambiente: &dyn Ambiente) -> Option<Self> {
        if let Some(dir) = ambiente.var("CLAUDE_CONFIG_DIR").filter(|d| !d.is_empty()) {
            let dir = PathBuf::from(dir);
            return Some(Self {
                estado: dir.join(".claude.json"),
                settings: dir.join("settings.json"),
            });
        }
        ambiente.home().map(|h| Self::da_pasta_do_usuario(&h))
    }

    /// Registro do que o preparo trocou, ao lado do `settings.json` ([`registro`]).
    pub fn registro(&self) -> PathBuf {
        let mut r = self.settings.clone().into_os_string();
        r.push(registro::SUFIXO_REGISTRO);
        PathBuf::from(r)
    }

    /// Cópia de segurança de um arquivo de fora da configuração (o
    /// `settings.local.json` de uma pasta): em `<pasta do settings.json>/cba-backups/`,
    /// com o caminho do arquivo no nome, nunca na pasta do projeto.
    pub fn backup_de(&self, arquivo: &Path) -> PathBuf {
        let nome: String = arquivo
            .to_string_lossy()
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '.' || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let pasta = self
            .settings
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        pasta
            .join(PASTA_BACKUPS)
            .join(format!("{nome}{}", arquivo::SUFIXO_BACKUP))
    }

    /// Arquivos padrão dentro de uma pasta do usuário.
    pub fn da_pasta_do_usuario(home: &Path) -> Self {
        Self {
            estado: home.join(".claude.json"),
            settings: home.join(".claude").join("settings.json"),
        }
    }
}

/// Pasta de um projeto, já com a chave que o Claude Code usa no escopo local.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Alvo {
    /// Identificador do projeto (vai para o servidor MCP em [`VAR_PROJETO`]).
    pub projeto: String,
    /// Diretório de trabalho da sessão do Claude (canônico).
    pub diretorio: PathBuf,
    /// Chave em `projects` do `.claude.json`.
    pub chave: String,
    /// Se a pasta está numa worktree do git (a chave é o repositório principal).
    pub worktree: bool,
    /// Raízes do escopo local das pastas do projeto, sem repetição: onde fica o
    /// `.claude/settings.local.json` com os hooks e a proteção (#304). A primeira é a
    /// da `chave`.
    pub raizes: Vec<PathBuf>,
}

impl Alvo {
    /// Alvo de um projeto cadastrado: a pasta raiz (sem prefixo) ou, na falta dela, a
    /// primeira, com as raízes de todas as pastas (as inacessíveis ficam de fora).
    pub fn do_projeto(projeto: &cba_index::Projeto) -> Result<Self, Erro> {
        let pasta = projeto
            .pastas
            .iter()
            .find(|p| p.prefixo.is_none())
            .or_else(|| projeto.pastas.first())
            .ok_or_else(|| Erro::Pasta(PathBuf::new()))?;
        let mut alvo = Self::da_pasta(&projeto.id, &pasta.caminho)?;
        for outra in &projeto.pastas {
            let Ok(dir) = dunce::canonicalize(&outra.caminho) else {
                continue;
            };
            if !dir.is_dir() {
                continue;
            }
            let (raiz, _) = raiz_local(&dir);
            if !alvo.raizes.contains(&raiz) {
                alvo.raizes.push(raiz);
            }
        }
        Ok(alvo)
    }

    /// Alvo de uma pasta, na plataforma atual.
    pub fn da_pasta(projeto: &str, pasta: &Path) -> Result<Self, Erro> {
        Self::da_pasta_em(projeto, pasta, Plataforma::atual())
    }

    /// Alvo de uma pasta, com a chave no formato de `plataforma`.
    pub fn da_pasta_em(projeto: &str, pasta: &Path, plataforma: Plataforma) -> Result<Self, Erro> {
        let diretorio = dunce::canonicalize(pasta).map_err(|_| Erro::Pasta(pasta.to_path_buf()))?;
        if !diretorio.is_dir() {
            return Err(Erro::Pasta(diretorio));
        }
        let (raiz, worktree) = raiz_local(&diretorio);
        Ok(Self {
            projeto: projeto.to_string(),
            chave: chave_do_caminho(&raiz, plataforma),
            diretorio,
            worktree,
            raizes: vec![raiz],
        })
    }
}

/// Chave de `projects` para um caminho. No Windows, com `/` como separador.
///
/// O formato no Windows não foi conferido numa máquina real (fica para os testes de
/// fumaça, #42); por isso a busca de uma chave existente ignora separador e caixa
/// ([`mesma_chave`]).
pub fn chave_do_caminho(caminho: &Path, plataforma: Plataforma) -> String {
    let texto = caminho.to_string_lossy().into_owned();
    match plataforma {
        Plataforma::Windows => texto.replace('\\', "/"),
        _ => texto,
    }
}

fn mesma_chave(a: &str, b: &str, plataforma: Plataforma) -> bool {
    match plataforma {
        Plataforma::Windows => {
            let n = |s: &str| s.replace('\\', "/").trim_end_matches('/').to_lowercase();
            n(a) == n(b)
        }
        _ => a.trim_end_matches('/') == b.trim_end_matches('/') || a == b,
    }
}

/// O que aconteceu com um item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Acao {
    Adicionado,
    /// Já existia uma entrada nossa diferente (outro executável), que foi trocada.
    Atualizado,
    Removido,
    /// O valor de antes do preparo voltou, com a entrada de outra instalação do app.
    Restaurado,
    SemMudanca,
}

/// Item da configuração.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "tipo",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Item {
    /// `projects["<chave>"].mcpServers.cba` no `.claude.json`.
    ServidorMcp { chave: String },
    /// `projects["<chave>"].hasTrustDialogAccepted` no `.claude.json`: a pasta é
    /// confiável e o Claude não pergunta na primeira conversa (#218).
    Confianca { chave: String },
    /// `hooks.<evento>` no `settings.local.json` de uma raiz (ou, em preparos
    /// anteriores à #304, no `settings.json` do usuário).
    Hook { evento: String },
    /// Linha `/.claude/settings.local.json` no `info/exclude` do git da raiz: o
    /// arquivo local fica fora do versionamento.
    ExclusaoGit { raiz: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Alteracao {
    pub arquivo: PathBuf,
    pub item: Item,
    pub acao: Acao,
    /// Executável de outra instalação do app: em [`Acao::Atualizado`], para onde a
    /// entrada apontava antes (o plano avisa da troca); em [`Acao::Restaurado`], para
    /// onde ela voltou a apontar.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outra_instalacao: Option<String>,
}

/// O que [`Preparador::desfazer`] deixou como estava, e por quê. Sem nenhum, a
/// configuração do Claude voltou a ser a de antes do preparo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "motivo",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Mantido {
    /// Os hooks ficam porque outras pastas preparadas ainda usam (chaves de
    /// `projects`).
    HooksEmUso {
        arquivo: PathBuf,
        pastas: Vec<String>,
    },
    /// Estes hooks mudaram depois do preparo (outra instalação do app ou à mão): o app
    /// tirou só as entradas dele e não voltou ao valor de antes.
    HooksMudaramDepois {
        arquivo: PathBuf,
        eventos: Vec<String>,
    },
    /// O servidor MCP da pasta mudou depois do preparo e ficou como está.
    ServidorMudouDepois { arquivo: PathBuf, chave: String },
    /// A entrada da pasta não existia antes do preparo e fica, porque guarda dados que
    /// o próprio Claude Code gravou ao abrir conversas nela (como o histórico de uso).
    EntradaDoClaude { arquivo: PathBuf, chave: String },
}

/// Resultado de [`Preparador::planejar`], [`Preparador::preparar`] e
/// [`Preparador::desfazer`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Resultado {
    /// Onde rodar o `claude` para a sessão enxergar o servidor e os hooks.
    pub diretorio: PathBuf,
    pub chave: String,
    pub worktree: bool,
    pub alteracoes: Vec<Alteracao>,
    /// Cópias de segurança feitas antes de gravar.
    pub backups: Vec<PathBuf>,
    /// `false` em [`Preparador::planejar`]: nada foi gravado.
    pub gravado: bool,
    /// No desfazer: o que ficou como estava, e por quê.
    pub mantidos: Vec<Mantido>,
}

impl Resultado {
    /// Se algum item mudou (ou mudaria, no plano).
    pub fn houve_mudanca(&self) -> bool {
        self.alteracoes.iter().any(|a| a.acao != Acao::SemMudanca)
    }

    /// No desfazer: a configuração do Claude voltou a ser a de antes do preparo.
    pub fn voltou_ao_que_era(&self) -> bool {
        self.mantidos.is_empty()
    }

    /// Executáveis de outras instalações do app que o preparo tira do lugar (para o
    /// aviso do plano), sem repetição. O mesmo executável aparece com `\` no servidor
    /// MCP e com `/` nos hooks, no Windows: conta uma vez só.
    pub fn outras_instalacoes(&self) -> Vec<&str> {
        let mut v: Vec<&str> = Vec::new();
        let normal = |c: &str| c.replace('\\', "/");
        for a in &self.alteracoes {
            if a.acao == Acao::Atualizado
                && let Some(o) = a.outra_instalacao.as_deref()
                && !v.iter().any(|x| normal(x) == normal(o))
            {
                v.push(o);
            }
        }
        v
    }
}

/// Situação de um item na configuração.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Situacao {
    Ok,
    Ausente,
    /// Entrada nossa com outro executável ou outros parâmetros.
    Desatualizado,
    /// Entrada com o nosso nome que não é do app.
    Conflito,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SituacaoHook {
    pub evento: String,
    pub situacao: Situacao,
    /// O `settings.local.json` conferido.
    pub arquivo: PathBuf,
}

/// Resultado de [`Preparador::verificar`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Verificacao {
    pub diretorio: PathBuf,
    pub chave: String,
    pub servidor_mcp: Situacao,
    /// O Claude Code confia na pasta: a primeira conversa abre sem o diálogo de
    /// confiança (#218).
    pub pasta_confiavel: bool,
    /// Por raiz e evento.
    pub hooks: Vec<SituacaoHook>,
    /// `disableAllHooks: true` nas configurações do usuário (sem `false` no arquivo
    /// local) ou no arquivo local de alguma raiz: os hooks não rodam.
    pub hooks_desativados: bool,
    /// Tudo no lugar (com a pasta confiável) e os hooks ativos.
    pub pronto: bool,
}

/// Prepara, verifica e desfaz o ambiente de um projeto.
#[derive(Debug, Clone)]
pub struct Preparador {
    pub config: ConfigClaude,
    /// Caminho absoluto do executável `cba` instalado.
    pub executavel: PathBuf,
    pub plataforma: Plataforma,
}

#[derive(Clone, Copy, PartialEq)]
enum Modo {
    Simular,
    Gravar,
}

impl Preparador {
    /// Configuração no local padrão do usuário atual.
    pub fn padrao(executavel: PathBuf) -> Result<Self, Erro> {
        Self::do_ambiente(&AmbienteReal, executavel)
    }

    /// Configuração no local que `ambiente` indica.
    pub fn do_ambiente(ambiente: &dyn Ambiente, executavel: PathBuf) -> Result<Self, Erro> {
        let config = ConfigClaude::localizar(ambiente).ok_or(Erro::SemPastaDoUsuario)?;
        Self::novo(config, executavel, ambiente.plataforma())
    }

    pub fn novo(
        config: ConfigClaude,
        executavel: PathBuf,
        plataforma: Plataforma,
    ) -> Result<Self, Erro> {
        let texto = executavel.to_string_lossy();
        let proibidos: &[char] = match plataforma {
            Plataforma::Windows => &['"', '$', '`', '%'],
            _ => &['"', '$', '`', '\\'],
        };
        if !executavel.is_absolute() || texto.contains(proibidos) {
            return Err(Erro::ExecutavelInvalido(executavel));
        }
        Ok(Self {
            config,
            executavel,
            plataforma,
        })
    }

    /// O que [`Preparador::preparar`] faria, sem gravar nada (tela de consentimento).
    pub fn planejar(&self, alvo: &Alvo) -> Result<Resultado, Erro> {
        self.aplicar(alvo, Modo::Simular)
    }

    /// Registra o servidor MCP e a confiança no escopo local de `alvo` e os hooks no
    /// `.claude/settings.local.json` de cada raiz.
    pub fn preparar(&self, alvo: &Alvo) -> Result<Resultado, Erro> {
        self.aplicar(alvo, Modo::Gravar)
    }

    /// Confere se o servidor MCP e os hooks estão no lugar, sem gravar nada.
    pub fn verificar(&self, alvo: &Alvo) -> Result<Verificacao, Erro> {
        let estado = arquivo::ler(&self.config.estado)?;
        let projeto = self.projeto(&estado.raiz, alvo, &estado.caminho)?;
        let pasta_confiavel =
            projeto.is_some_and(|p| matches!(p.get(CAMPO_CONFIANCA), Some(Json::Bool(true))));
        let servidor_mcp = match projeto {
            None => Situacao::Ausente,
            Some(projeto) => match servidores(projeto, &estado.caminho)?
                .and_then(|s| s.get(NOME_SERVIDOR))
            {
                None => Situacao::Ausente,
                Some(v) if v.para_value() == self.entrada_mcp(alvo).para_value() => Situacao::Ok,
                Some(v) if e_nosso_servidor(v) => Situacao::Desatualizado,
                Some(_) => Situacao::Conflito,
            },
        };

        let settings = arquivo::ler(&self.config.settings)?;
        let desativados_no_usuario =
            matches!(settings.raiz.get("disableAllHooks"), Some(Json::Bool(true)));
        let mut hooks_desativados = false;
        let mut hooks = Vec::new();
        for raiz in &alvo.raizes {
            let local = arquivo::ler(&arquivo_local(raiz))?;
            hooks_desativados |= match local.raiz.get("disableAllHooks") {
                Some(Json::Bool(b)) => *b,
                _ => desativados_no_usuario,
            };
            for evento in EVENTOS {
                let esperado = self.comando_hook(evento);
                let comandos = comandos_nossos(&local.raiz, evento, &local.caminho)?;
                let situacao = if comandos.iter().any(|c| *c == esperado) && comandos.len() == 1 {
                    Situacao::Ok
                } else if comandos.is_empty() {
                    Situacao::Ausente
                } else {
                    Situacao::Desatualizado
                };
                hooks.push(SituacaoHook {
                    evento: evento.to_string(),
                    situacao,
                    arquivo: arquivo_local(raiz),
                });
            }
        }
        let pronto = servidor_mcp == Situacao::Ok
            && pasta_confiavel
            && !hooks_desativados
            && !hooks.is_empty()
            && hooks.iter().all(|h| h.situacao == Situacao::Ok);
        Ok(Verificacao {
            diretorio: alvo.diretorio.clone(),
            chave: alvo.chave.clone(),
            servidor_mcp,
            pasta_confiavel,
            hooks,
            hooks_desativados,
            pronto,
        })
    }

    /// Tira o servidor MCP do escopo local de `alvo` e devolve o que o preparo
    /// trocou ([`registro`]): o servidor de antes (de outra instalação do app, se
    /// havia), os hooks de antes no `settings.local.json` de cada raiz (o arquivo e a
    /// pasta `.claude` saem se só existiam por causa do app) e a linha do
    /// `info/exclude`. O que não dá para devolver fica, e é dito em
    /// [`Resultado::mantidos`].
    ///
    /// Os hooks que preparos anteriores à #304 gravaram no `settings.json` do usuário
    /// saem quando nenhuma outra pasta preparada por esta instalação usa mais; sem
    /// registro, só se nenhuma pasta tiver mais o servidor de nenhuma instalação.
    pub fn desfazer(&self, alvo: &Alvo) -> Result<Resultado, Erro> {
        let mut resultado = self.resultado(alvo, Modo::Gravar);
        let mut registro = Registro::ler(&self.config.registro());
        let mut acao_mcp = (Acao::SemMudanca, None);
        let mut acao_confianca = Acao::SemMudanca;
        let mut mantidos = Vec::new();
        let mut pastas_minhas = Vec::new();
        let mut pastas_todas = Vec::new();
        if let Some(backup) = self.editar_registrando(
            &self.config.estado,
            Modo::Gravar,
            &mut registro,
            None,
            false,
            |raiz, caminho, reg| {
                mantidos.clear();
                // Antes do servidor: a entrada da pasta pode ficar vazia e sair.
                acao_confianca = self.remover_confianca(raiz, alvo, caminho, reg)?;
                acao_mcp = self.remover_mcp(raiz, alvo, caminho, reg, &mut mantidos)?;
                pastas_minhas = pastas_com_servidor(raiz, Some(&self.executavel_texto()));
                pastas_todas = pastas_com_servidor(raiz, None);
                Ok(())
            },
        )? {
            resultado.backups.push(backup);
        }
        resultado.alteracoes.push(Alteracao {
            arquivo: self.config.estado.clone(),
            item: Item::ServidorMcp {
                chave: alvo.chave.clone(),
            },
            acao: acao_mcp.0,
            outra_instalacao: acao_mcp.1,
        });
        resultado.alteracoes.push(Alteracao {
            arquivo: self.config.estado.clone(),
            item: Item::Confianca {
                chave: alvo.chave.clone(),
            },
            acao: acao_confianca,
            outra_instalacao: None,
        });
        resultado.mantidos.append(&mut mantidos);

        for raiz in &alvo.raizes {
            self.desfazer_local(
                raiz,
                &pastas_minhas,
                &pastas_todas,
                &mut registro,
                &mut resultado,
            )?;
        }
        self.desfazer_global(&pastas_minhas, &pastas_todas, &mut registro, &mut resultado)?;
        tracing::info!(
            projeto = alvo.projeto,
            mudou = resultado.houve_mudanca(),
            mantidos = resultado.mantidos.len(),
            "preparo desfeito"
        );
        Ok(resultado)
    }

    /// Devolve o `settings.local.json` de `raiz` ao que era antes do preparo, a menos
    /// que a raiz seja a pasta de outra pasta preparada por esta instalação.
    fn desfazer_local(
        &self,
        raiz: &Path,
        pastas_minhas: &[String],
        pastas_todas: &[String],
        registro: &mut Registro,
        resultado: &mut Resultado,
    ) -> Result<(), Erro> {
        let exe = self.executavel_texto();
        let arq = arquivo_local(raiz);
        let chave = chave_do_caminho(raiz, self.plataforma);
        let usa = |pastas: &[String]| {
            pastas
                .iter()
                .any(|k| mesma_chave(k, &chave, self.plataforma))
        };
        if usa(pastas_minhas) {
            let lido = arquivo::ler(&arq)?;
            let mut algum = false;
            for evento in EVENTOS {
                algum |= !comandos_nossos(&lido.raiz, evento, &lido.caminho)?.is_empty();
            }
            if algum {
                resultado.mantidos.push(Mantido::HooksEmUso {
                    arquivo: arq,
                    pastas: vec![chave],
                });
            }
            return Ok(());
        }
        let antes = registro.local(&exe, &chave).cloned();
        let apagar_vazio = antes.as_ref().is_some_and(|t| !t.arquivo_existia);
        let pasta_existia = antes.as_ref().is_none_or(|t| t.pasta_existia);
        let nenhuma_usa = !usa(pastas_todas);
        let mut acoes = Vec::new();
        let mut mudaram = Vec::new();
        let mut em_uso = false;
        let mut exclude_meu = false;
        let backup = self.config.backup_de(&arq);
        if let Some(b) = self.editar_registrando(
            &arq,
            Modo::Gravar,
            registro,
            Some(&backup),
            apagar_vazio,
            |obj, caminho, reg| {
                let (troca, outra_usa) = match reg.tirar_local(&exe, &chave) {
                    Some((t, o)) => (Some(t), o),
                    None => (None, false),
                };
                let (existia, trocas) = troca
                    .as_ref()
                    .map(|t| (t.hooks_existia, t.hooks.clone()))
                    .unwrap_or((None, BTreeMap::new()));
                (acoes, mudaram, em_uso) =
                    self.restaurar_hooks(obj, caminho, existia, trocas, nenhuma_usa)?;
                exclude_meu = troca.is_some_and(|t| t.exclude) && !outra_usa;
                Ok(())
            },
        )? {
            resultado.backups.push(b);
        }
        resultado
            .alteracoes
            .extend(acoes.into_iter().map(|(evento, acao, outra)| Alteracao {
                arquivo: arq.clone(),
                item: Item::Hook { evento },
                acao,
                outra_instalacao: outra,
            }));
        if !mudaram.is_empty() {
            resultado.mantidos.push(Mantido::HooksMudaramDepois {
                arquivo: arq.clone(),
                eventos: mudaram,
            });
        }
        if em_uso {
            resultado.mantidos.push(Mantido::HooksEmUso {
                arquivo: arq.clone(),
                pastas: pastas_todas.to_vec(),
            });
        }
        if !arq.exists() && !pasta_existia {
            // Só sai vazia: nunca apaga o que o analista pôs lá.
            let _ = std::fs::remove_dir(raiz.join(PASTA_LOCAL));
        }
        // A linha sai só se foi o app que a pôs e o arquivo saiu: um arquivo do
        // analista que ficou continua fora do versionamento.
        if exclude_meu
            && !arq.exists()
            && let Some(exclude) = arquivo_exclude(raiz)
            && remover_linha_exclude(&exclude)?
        {
            resultado.alteracoes.push(Alteracao {
                arquivo: exclude,
                item: Item::ExclusaoGit { raiz: chave },
                acao: Acao::Removido,
                outra_instalacao: None,
            });
        }
        Ok(())
    }

    /// Tira do `settings.json` do usuário os hooks que preparos anteriores à #304
    /// gravaram lá, quando nenhuma outra pasta preparada por esta instalação usa
    /// mais. Só as mudanças entram no resultado.
    fn desfazer_global(
        &self,
        pastas_minhas: &[String],
        pastas_todas: &[String],
        registro: &mut Registro,
        resultado: &mut Resultado,
    ) -> Result<(), Erro> {
        let arq = self.config.settings.clone();
        if !pastas_minhas.is_empty() {
            // Outra pasta preparada por esta instalação ainda usa os hooks.
            let settings = arquivo::ler(&self.config.settings)?;
            let mut algum = false;
            for evento in EVENTOS {
                algum |= !comandos_nossos(&settings.raiz, evento, &settings.caminho)?.is_empty();
            }
            if algum {
                resultado.mantidos.push(Mantido::HooksEmUso {
                    arquivo: arq,
                    pastas: pastas_minhas.to_vec(),
                });
            }
            return Ok(());
        }
        let mut acoes = Vec::new();
        let mut mudaram = Vec::new();
        let mut em_uso = false;
        let exe = self.executavel_texto();
        if let Some(backup) = self.editar_registrando(
            &self.config.settings,
            Modo::Gravar,
            registro,
            None,
            false,
            |raiz, caminho, reg| {
                let (existia, trocas) = reg.tirar_hooks(&exe).unwrap_or((None, BTreeMap::new()));
                (acoes, mudaram, em_uso) =
                    self.restaurar_hooks(raiz, caminho, existia, trocas, pastas_todas.is_empty())?;
                Ok(())
            },
        )? {
            resultado.backups.push(backup);
        }
        resultado.alteracoes.extend(
            acoes
                .into_iter()
                .filter(|(_, acao, _)| *acao != Acao::SemMudanca)
                .map(|(evento, acao, outra)| Alteracao {
                    arquivo: arq.clone(),
                    item: Item::Hook { evento },
                    acao,
                    outra_instalacao: outra,
                }),
        );
        if !mudaram.is_empty() {
            resultado.mantidos.push(Mantido::HooksMudaramDepois {
                arquivo: arq.clone(),
                eventos: mudaram,
            });
        }
        if em_uso {
            resultado.mantidos.push(Mantido::HooksEmUso {
                arquivo: arq,
                pastas: pastas_todas.to_vec(),
            });
        }
        Ok(())
    }

    fn resultado(&self, alvo: &Alvo, modo: Modo) -> Resultado {
        Resultado {
            diretorio: alvo.diretorio.clone(),
            chave: alvo.chave.clone(),
            worktree: alvo.worktree,
            alteracoes: Vec::new(),
            backups: Vec::new(),
            gravado: modo == Modo::Gravar,
            mantidos: Vec::new(),
        }
    }

    fn aplicar(&self, alvo: &Alvo, modo: Modo) -> Result<Resultado, Erro> {
        let mut resultado = self.resultado(alvo, modo);
        let mut registro = Registro::ler(&self.config.registro());

        let mut acao_mcp = (Acao::SemMudanca, None);
        let mut acao_confianca = Acao::SemMudanca;
        if let Some(backup) = self.editar_registrando(
            &self.config.estado,
            modo,
            &mut registro,
            None,
            false,
            |raiz, caminho, reg| {
                acao_mcp = self.registrar_mcp(raiz, alvo, caminho, reg)?;
                acao_confianca = self.registrar_confianca(raiz, alvo, caminho, reg)?;
                Ok(())
            },
        )? {
            resultado.backups.push(backup);
        }
        resultado.alteracoes.push(Alteracao {
            arquivo: self.config.estado.clone(),
            item: Item::ServidorMcp {
                chave: alvo.chave.clone(),
            },
            acao: acao_mcp.0,
            outra_instalacao: acao_mcp.1,
        });
        resultado.alteracoes.push(Alteracao {
            arquivo: self.config.estado.clone(),
            item: Item::Confianca {
                chave: alvo.chave.clone(),
            },
            acao: acao_confianca,
            outra_instalacao: None,
        });

        for raiz in &alvo.raizes {
            self.aplicar_local(raiz, modo, &mut registro, &mut resultado)?;
        }

        if modo == Modo::Gravar {
            tracing::info!(
                projeto = alvo.projeto,
                worktree = alvo.worktree,
                mudou = resultado.houve_mudanca(),
                outra_instalacao = !resultado.outras_instalacoes().is_empty(),
                "ambiente preparado"
            );
        }
        Ok(resultado)
    }

    /// Hooks no `settings.local.json` de `raiz` e, se a raiz é de um repositório git,
    /// a linha do arquivo no `info/exclude`.
    fn aplicar_local(
        &self,
        raiz: &Path,
        modo: Modo,
        registro: &mut Registro,
        resultado: &mut Resultado,
    ) -> Result<(), Erro> {
        let exe = self.executavel_texto();
        let arq = arquivo_local(raiz);
        let chave = chave_do_caminho(raiz, self.plataforma);
        let pasta_existia = raiz.join(PASTA_LOCAL).is_dir();
        let arquivo_existia = arq.exists();
        let mut acoes = Vec::new();
        let backup = self.config.backup_de(&arq);
        if let Some(b) = self.editar_registrando(
            &arq,
            modo,
            registro,
            Some(&backup),
            false,
            |obj, caminho, reg| {
                let instalacao = reg.instalacoes.entry(exe.clone()).or_default();
                let mut troca = instalacao.locais.remove(&chave).unwrap_or(TrocaLocal {
                    arquivo_existia,
                    pasta_existia,
                    ..TrocaLocal::default()
                });
                acoes =
                    self.registrar_hooks(obj, caminho, &mut troca.hooks, &mut troca.hooks_existia)?;
                if !troca.vazia() {
                    instalacao.locais.insert(chave.clone(), troca);
                }
                if instalacao.vazia() {
                    reg.instalacoes.remove(&exe);
                }
                Ok(())
            },
        )? {
            resultado.backups.push(b);
        }
        resultado
            .alteracoes
            .extend(acoes.into_iter().map(|(evento, acao, outra)| Alteracao {
                arquivo: arq.clone(),
                item: Item::Hook { evento },
                acao,
                outra_instalacao: outra,
            }));

        if let Some(exclude) = arquivo_exclude(raiz)
            && !tem_linha_exclude(&exclude)
        {
            if modo == Modo::Gravar {
                self.registrar_exclude(registro, &chave, arquivo_existia, pasta_existia)?;
                acrescentar_linha_exclude(&exclude)?;
            }
            resultado.alteracoes.push(Alteracao {
                arquivo: exclude,
                item: Item::ExclusaoGit { raiz: chave },
                acao: Acao::Adicionado,
                outra_instalacao: None,
            });
        }
        Ok(())
    }

    /// Anota no registro (e grava) que esta instalação pôs a linha do `info/exclude`
    /// da raiz `chave`.
    fn registrar_exclude(
        &self,
        registro: &mut Registro,
        chave: &str,
        arquivo_existia: bool,
        pasta_existia: bool,
    ) -> Result<(), Erro> {
        registro
            .instalacoes
            .entry(self.executavel_texto())
            .or_default()
            .locais
            .entry(chave.to_string())
            .or_insert(TrocaLocal {
                arquivo_existia,
                pasta_existia,
                ..TrocaLocal::default()
            })
            .exclude = true;
        registro.gravar(&self.config.registro())
    }

    /// Como [`editar_com`], levando junto o [`Registro`]: `f` o atualiza a cada tentativa,
    /// a partir do que estava em `registro`. No modo de gravação, o registro novo é
    /// gravado **antes** do arquivo do Claude (nunca há troca sem o valor de antes
    /// guardado) e volta ao que era se a gravação do arquivo falhar.
    fn editar_registrando<F>(
        &self,
        arquivo: &Path,
        modo: Modo,
        registro: &mut Registro,
        backup: Option<&Path>,
        apagar_vazio: bool,
        mut f: F,
    ) -> Result<Option<PathBuf>, Erro>
    where
        F: FnMut(&mut Objeto, &Path, &mut Registro) -> Result<(), Erro>,
    {
        let base = registro.clone();
        let caminho = self.config.registro();
        let mut novo = base.clone();
        let feito = editar_com(arquivo, modo, backup, apagar_vazio, |raiz, c| {
            novo = base.clone();
            f(raiz, c, &mut novo)?;
            if modo == Modo::Gravar && novo != base {
                novo.gravar(&caminho)?;
            }
            Ok(())
        });
        match feito {
            Ok(b) => {
                *registro = novo;
                Ok(b)
            }
            Err(e) => {
                if modo == Modo::Gravar && novo != base && base.gravar(&caminho).is_err() {
                    tracing::warn!("registro do preparo não voltou ao que era");
                }
                Err(e)
            }
        }
    }

    /// Caminho do executável como vai no servidor MCP (identifica a instalação no
    /// [`Registro`]).
    fn executavel_texto(&self) -> String {
        self.executavel.to_string_lossy().into_owned()
    }

    /// Caminho do executável como vai no comando dos hooks.
    fn executavel_no_hook(&self) -> String {
        let caminho = self.executavel.to_string_lossy();
        match self.plataforma {
            Plataforma::Windows => caminho.replace('\\', "/"),
            _ => caminho.into_owned(),
        }
    }

    /// Entrada do servidor MCP, no formato que `claude mcp add-json` grava.
    fn entrada_mcp(&self, alvo: &Alvo) -> Json {
        let mut env = Objeto::default();
        env.inserir(VAR_PROJETO, Json::texto(&alvo.projeto));
        let mut o = Objeto::default();
        o.inserir("type", Json::texto("stdio"));
        o.inserir("command", Json::texto(self.executavel.to_string_lossy()));
        o.inserir("args", Json::Lista(vec![Json::texto("mcp")]));
        o.inserir("env", Json::Objeto(env));
        Json::Objeto(o)
    }

    /// Comando do hook. Executado por um shell, por isso o caminho vai entre aspas e,
    /// no Windows, com `/` (aceito tanto pelo Git Bash quanto pelo cmd).
    fn comando_hook(&self, evento: &str) -> String {
        format!("\"{}\" hook {evento}", self.executavel_no_hook())
    }

    /// Chave existente equivalente à do alvo (mesmo caminho com outra grafia).
    fn chave_existente(&self, projetos: &Objeto, alvo: &Alvo) -> Option<String> {
        projetos
            .chaves()
            .find(|k| mesma_chave(k, &alvo.chave, self.plataforma))
            .map(str::to_string)
    }

    fn projeto<'a>(
        &self,
        raiz: &'a Objeto,
        alvo: &Alvo,
        caminho: &Path,
    ) -> Result<Option<&'a Objeto>, Erro> {
        let Some(projetos) = raiz.get("projects") else {
            return Ok(None);
        };
        let projetos = projetos
            .como_objeto()
            .ok_or_else(|| formato(caminho, "projects"))?;
        match self.chave_existente(projetos, alvo) {
            None => Ok(None),
            Some(k) => projetos
                .get(&k)
                .and_then(Json::como_objeto)
                .map(Some)
                .ok_or_else(|| formato(caminho, &format!("projects[{k:?}]"))),
        }
    }

    /// Registra o servidor MCP de `alvo`. Devolve a ação e, se a entrada era de outra
    /// instalação do app, o executável dela. Guarda no registro o que havia antes.
    fn registrar_mcp(
        &self,
        raiz: &mut Objeto,
        alvo: &Alvo,
        caminho: &Path,
        reg: &mut Registro,
    ) -> Result<(Acao, Option<String>), Erro> {
        let exe = self.executavel_texto();
        let projetos = raiz
            .objeto_mut("projects")
            .ok_or_else(|| formato(caminho, "projects"))?;
        let chave = self
            .chave_existente(projetos, alvo)
            .unwrap_or_else(|| alvo.chave.clone());
        let projeto_existia = projetos.get(&chave).is_some();
        let projeto = projetos
            .objeto_mut(&chave)
            .ok_or_else(|| formato(caminho, &format!("projects[{chave:?}]")))?;
        let servidores_existia = projeto.get("mcpServers").is_some();
        let servidores = projeto
            .objeto_mut("mcpServers")
            .ok_or_else(|| formato(caminho, "mcpServers"))?;
        let nova = self.entrada_mcp(alvo);
        let antes = servidores.get(NOME_SERVIDOR).cloned();
        let (acao, outra) = match &antes {
            None => (Acao::Adicionado, None),
            Some(v) if v.para_value() == nova.para_value() => return Ok((Acao::SemMudanca, None)),
            Some(v) if e_nosso_servidor(v) => (
                Acao::Atualizado,
                comando_do_servidor(v)
                    .filter(|c| *c != exe)
                    .map(str::to_string),
            ),
            Some(_) => {
                return Err(Erro::Conflito {
                    arquivo: caminho.to_path_buf(),
                    chave,
                });
            }
        };
        servidores.inserir(NOME_SERVIDOR, nova.clone());
        let instalacao = reg.instalacoes.entry(exe.clone()).or_default();
        match instalacao.mcp.get_mut(&chave) {
            Some(troca) => troca.depois = nova,
            None => {
                // Uma entrada desta mesma instalação (preparo antigo) não é "antes".
                let antes = antes.filter(|v| comando_do_servidor(v) != Some(exe.as_str()));
                instalacao.mcp.insert(
                    chave,
                    TrocaMcp {
                        projeto_existia,
                        servidores_existia,
                        antes,
                        depois: nova,
                    },
                );
            }
        }
        Ok((acao, outra))
    }

    /// Marca a pasta de `alvo` como confiável no Claude Code (o mesmo campo que ele
    /// grava quando o analista aceita o diálogo), guardando no registro o valor de
    /// antes. Se já é confiável, nada muda e nada é registrado: a confiança não foi o
    /// app que deu, e o desfazer não a tira.
    fn registrar_confianca(
        &self,
        raiz: &mut Objeto,
        alvo: &Alvo,
        caminho: &Path,
        reg: &mut Registro,
    ) -> Result<Acao, Erro> {
        let projetos = raiz
            .objeto_mut("projects")
            .ok_or_else(|| formato(caminho, "projects"))?;
        let chave = self
            .chave_existente(projetos, alvo)
            .unwrap_or_else(|| alvo.chave.clone());
        let projeto = projetos
            .objeto_mut(&chave)
            .ok_or_else(|| formato(caminho, &format!("projects[{chave:?}]")))?;
        let antes = projeto.get(CAMPO_CONFIANCA).cloned();
        if matches!(antes, Some(Json::Bool(true))) {
            return Ok(Acao::SemMudanca);
        }
        projeto.inserir(CAMPO_CONFIANCA, Json::Bool(true));
        reg.instalacoes
            .entry(self.executavel_texto())
            .or_default()
            .confianca
            .entry(chave)
            .or_insert(TrocaConfianca { antes });
        Ok(Acao::Adicionado)
    }

    /// Tira a confiança que esta instalação deu à pasta de `alvo`, devolvendo o valor
    /// de antes. Só com registro, e só se o campo ainda está `true`; se outra
    /// instalação ainda tem o servidor na pasta, a confiança fica e passa a ser dela.
    fn remover_confianca(
        &self,
        raiz: &mut Objeto,
        alvo: &Alvo,
        caminho: &Path,
        reg: &mut Registro,
    ) -> Result<Acao, Erro> {
        let exe = self.executavel_texto();
        let projetos = match raiz.get_mut("projects") {
            None => None,
            Some(Json::Objeto(p)) => Some(p),
            Some(_) => return Err(formato(caminho, "projects")),
        };
        let chave = projetos
            .as_ref()
            .and_then(|p| self.chave_existente(p, alvo))
            .unwrap_or_else(|| alvo.chave.clone());
        let Some((troca, transferida)) = reg.tirar_confianca(&exe, &chave) else {
            return Ok(Acao::SemMudanca);
        };
        if transferida {
            return Ok(Acao::SemMudanca);
        }
        let Some(Json::Objeto(projeto)) = projetos.and_then(|p| p.get_mut(&chave)) else {
            return Ok(Acao::SemMudanca);
        };
        if !matches!(projeto.get(CAMPO_CONFIANCA), Some(Json::Bool(true))) {
            return Ok(Acao::SemMudanca);
        }
        match troca.antes {
            Some(antes) => projeto.inserir(CAMPO_CONFIANCA, antes),
            None => {
                projeto.remover(CAMPO_CONFIANCA);
            }
        }
        Ok(Acao::Removido)
    }

    /// Tira o servidor MCP de `alvo`, devolvendo o de antes se o registro o tiver.
    fn remover_mcp(
        &self,
        raiz: &mut Objeto,
        alvo: &Alvo,
        caminho: &Path,
        reg: &mut Registro,
        mantidos: &mut Vec<Mantido>,
    ) -> Result<(Acao, Option<String>), Erro> {
        let exe = self.executavel_texto();
        let Some(projetos) = raiz.get_mut("projects") else {
            reg.tirar_mcp(&exe, &alvo.chave);
            return Ok((Acao::SemMudanca, None));
        };
        let Json::Objeto(projetos) = projetos else {
            return Err(formato(caminho, "projects"));
        };
        let Some(chave) = self.chave_existente(projetos, alvo) else {
            reg.tirar_mcp(&exe, &alvo.chave);
            return Ok((Acao::SemMudanca, None));
        };
        let troca = reg.tirar_mcp(&exe, &chave);
        let Some(Json::Objeto(projeto)) = projetos.get_mut(&chave) else {
            return Err(formato(caminho, &format!("projects[{chave:?}]")));
        };
        let Some(troca) = troca else {
            return Ok((remover_mcp_sem_registro(projetos, &chave), None));
        };

        let mut resposta = (Acao::SemMudanca, None);
        let mut servidor_ficou = false;
        match projeto.get_mut("mcpServers") {
            Some(Json::Objeto(servidores)) => {
                match servidores.get(NOME_SERVIDOR) {
                    Some(v)
                        if mesmo(v, &troca.depois)
                            || comando_do_servidor(v) == Some(exe.as_str()) =>
                    {
                        resposta = match &troca.antes {
                            Some(antes) => {
                                servidores.inserir(NOME_SERVIDOR, antes.clone());
                                (
                                    Acao::Restaurado,
                                    comando_do_servidor(antes).map(str::to_string),
                                )
                            }
                            None => {
                                servidores.remover(NOME_SERVIDOR);
                                (Acao::Removido, None)
                            }
                        };
                    }
                    // Já é o de antes.
                    Some(v) if troca.antes.as_ref().is_some_and(|a| mesmo(a, v)) => {}
                    Some(_) => {
                        servidor_ficou = true;
                        mantidos.push(Mantido::ServidorMudouDepois {
                            arquivo: caminho.to_path_buf(),
                            chave: chave.clone(),
                        });
                    }
                    None => {}
                }
                if !troca.servidores_existia && servidores.is_empty() {
                    projeto.remover("mcpServers");
                }
            }
            None => {}
            Some(_) => return Err(formato(caminho, "mcpServers")),
        }
        if !troca.projeto_existia {
            if projeto.is_empty() {
                projetos.remover(&chave);
            } else if !servidor_ficou {
                mantidos.push(Mantido::EntradaDoClaude {
                    arquivo: caminho.to_path_buf(),
                    chave,
                });
            }
        }
        Ok(resposta)
    }

    /// Põe os hooks do app em `raiz` (o `settings.local.json` de uma raiz), guardando
    /// em `trocas` e `hooks_existia_reg` o que havia antes.
    fn registrar_hooks(
        &self,
        raiz: &mut Objeto,
        caminho: &Path,
        trocas: &mut BTreeMap<String, TrocaHook>,
        hooks_existia_reg: &mut Option<bool>,
    ) -> Result<Vec<(String, Acao, Option<String>)>, Erro> {
        let meu = self.executavel_no_hook();
        let hooks_existia = raiz.get("hooks").is_some();
        let hooks = raiz
            .objeto_mut("hooks")
            .ok_or_else(|| formato(caminho, "hooks"))?;
        let mut acoes = Vec::new();
        for evento in EVENTOS {
            let comando = self.comando_hook(evento);
            let grupo = grupo_hook(&comando);
            let antes = hooks.get(evento).cloned();
            if antes.is_none() {
                hooks.inserir(evento, Json::Lista(Vec::new()));
            }
            let Some(Json::Lista(grupos)) = hooks.get_mut(evento) else {
                return Err(formato(caminho, &format!("hooks.{evento}")));
            };
            let nossos = contar_nossos(grupos, caminho, evento)?;
            let ja_certo =
                nossos == 1 && grupos.iter().any(|g| g.para_value() == grupo.para_value());
            if ja_certo {
                acoes.push((evento.to_string(), Acao::SemMudanca, None));
                continue;
            }
            let outra = executaveis_dos_hooks(grupos)
                .into_iter()
                .find(|e| *e != meu)
                .map(str::to_string);
            tirar_nossos(grupos);
            grupos.push(grupo);
            let depois = Json::Lista(grupos.clone());
            let acao = if nossos == 0 {
                Acao::Adicionado
            } else {
                Acao::Atualizado
            };
            acoes.push((evento.to_string(), acao, outra));

            hooks_existia_reg.get_or_insert(hooks_existia);
            match trocas.get_mut(*evento) {
                Some(troca) => troca.depois = depois,
                None => {
                    // Os handlers desta mesma instalação (preparo antigo) não são "antes".
                    let antes = antes.map(|mut v| {
                        let tirados = match &mut v {
                            Json::Lista(g) => tirar_do_executavel(g, &meu),
                            _ => 0,
                        };
                        (v, tirados)
                    });
                    let antes = match antes {
                        Some((Json::Lista(g), tirados)) if g.is_empty() && tirados > 0 => None,
                        outro => outro.map(|(v, _)| v),
                    };
                    trocas.insert(evento.to_string(), TrocaHook { antes, depois });
                }
            }
        }
        Ok(acoes)
    }

    /// Devolve os hooks de antes do preparo desta instalação (`trocas`, já tiradas do
    /// registro). Devolve as ações por evento, os eventos que mudaram depois do
    /// preparo e se os hooks ficaram porque outras pastas ainda usam (só sem
    /// registro).
    #[allow(clippy::type_complexity)]
    fn restaurar_hooks(
        &self,
        raiz: &mut Objeto,
        caminho: &Path,
        hooks_existia: Option<bool>,
        trocas: BTreeMap<String, TrocaHook>,
        nenhuma_pasta_usa: bool,
    ) -> Result<(Vec<(String, Acao, Option<String>)>, Vec<String>, bool), Erro> {
        let meu = self.executavel_no_hook();
        let mut acoes = Vec::new();
        let mut mudaram = Vec::new();
        let mut em_uso = false;
        let Some(hooks) = raiz.get_mut("hooks") else {
            let acoes = EVENTOS
                .iter()
                .map(|e| (e.to_string(), Acao::SemMudanca, None))
                .collect();
            return Ok((acoes, mudaram, em_uso));
        };
        let Json::Objeto(hooks) = hooks else {
            return Err(formato(caminho, "hooks"));
        };
        for evento in EVENTOS {
            let Some(atual) = hooks.get(evento) else {
                if trocas.contains_key(*evento) {
                    mudaram.push(evento.to_string());
                }
                acoes.push((evento.to_string(), Acao::SemMudanca, None));
                continue;
            };
            if atual.como_lista().is_none() {
                return Err(formato(caminho, &format!("hooks.{evento}")));
            }
            let troca = trocas.get(*evento);
            let (acao, outra) = match troca {
                Some(troca) if mesmo(atual, &troca.depois) => match &troca.antes {
                    None => {
                        hooks.remover(evento);
                        (Acao::Removido, None)
                    }
                    Some(antes) => {
                        hooks.inserir(evento, antes.clone());
                        let outra = antes
                            .como_lista()
                            .and_then(|g| executaveis_dos_hooks(g).into_iter().next())
                            .map(str::to_string);
                        let acao = if outra.is_some() {
                            Acao::Restaurado
                        } else {
                            Acao::Removido
                        };
                        (acao, outra)
                    }
                },
                // Já voltou ao que era (outra instalação o devolveu).
                Some(troca) if troca.antes.as_ref().is_some_and(|a| mesmo(a, atual)) => {
                    (Acao::SemMudanca, None)
                }
                // Mudou depois do preparo: tira só o desta instalação.
                Some(_) => {
                    mudaram.push(evento.to_string());
                    (tirar_do_evento(hooks, evento, Some(&meu)), None)
                }
                // Sem registro (preparo antigo): tira tudo do app se nenhuma pasta usa.
                None if nenhuma_pasta_usa => (tirar_do_evento(hooks, evento, None), None),
                None => {
                    let grupos = atual.como_lista().map(Vec::as_slice).unwrap_or_default();
                    em_uso |= contar_nossos(grupos, caminho, evento)? > 0;
                    (Acao::SemMudanca, None)
                }
            };
            acoes.push((evento.to_string(), acao, outra));
        }
        let houve = acoes.iter().any(|(_, a, _)| *a != Acao::SemMudanca);
        if hooks.is_empty() && houve && hooks_existia != Some(true) {
            raiz.remover("hooks");
        }
        Ok((acoes, mudaram, em_uso))
    }
}

/// Mesmo conteúdo, sem olhar a ordem das chaves.
fn mesmo(a: &Json, b: &Json) -> bool {
    a.para_value() == b.para_value()
}

fn formato(caminho: &Path, campo: &str) -> Erro {
    Erro::Formato {
        arquivo: caminho.to_path_buf(),
        campo: campo.to_string(),
    }
}

/// Lê, edita e (no modo de gravação) grava se algo mudou. Repete se o arquivo mudar
/// entre a leitura e a gravação. Devolve a cópia de segurança, se houve.
/// Com `backup`, a cópia de segurança vai para lá (e não ao lado do arquivo); com
/// `apagar_vazio`, o arquivo é apagado (em vez de gravado como `{}`) se ficar vazio.
fn editar_com<F>(
    caminho: &Path,
    modo: Modo,
    backup: Option<&Path>,
    apagar_vazio: bool,
    mut f: F,
) -> Result<Option<PathBuf>, Erro>
where
    F: FnMut(&mut Objeto, &Path) -> Result<(), Erro>,
{
    let mut tentativa = 0;
    loop {
        let lido = arquivo::ler(caminho)?;
        let mut raiz = lido.raiz.clone();
        f(&mut raiz, &lido.caminho)?;
        if modo == Modo::Simular || raiz == lido.raiz {
            return Ok(None);
        }
        let feito = if apagar_vazio && raiz.is_empty() {
            arquivo::apagar(&lido, backup)
        } else {
            arquivo::gravar_com_backup_em(&lido, &raiz, backup)
        };
        match feito {
            Err(Erro::AlteradoDuranteGravacao(_)) if tentativa < 4 => tentativa += 1,
            outro => return outro,
        }
    }
}

/// `info/exclude` do git da raiz, se ela é a raiz de um repositório.
fn arquivo_exclude(raiz: &Path) -> Option<PathBuf> {
    let repo = cba_index::detectar_git(raiz)?;
    if repo.raiz != raiz {
        return None;
    }
    Some(
        cba_index::diretorio_comum_git(raiz)?
            .join("info")
            .join("exclude"),
    )
}

/// Linhas do `info/exclude` que já tiram o arquivo local do versionamento.
fn e_linha_do_arquivo_local(linha: &str) -> bool {
    matches!(
        linha.trim(),
        LINHA_EXCLUDE | ".claude/settings.local.json" | "**/.claude/settings.local.json"
    )
}

fn tem_linha_exclude(exclude: &Path) -> bool {
    std::fs::read_to_string(exclude).is_ok_and(|t| t.lines().any(e_linha_do_arquivo_local))
}

fn erro_io(arquivo: &Path, e: io::Error) -> Erro {
    Erro::Arquivo {
        arquivo: arquivo.to_path_buf(),
        tipo: e.kind(),
    }
}

/// Acrescenta [`LINHA_EXCLUDE`] ao `info/exclude`, criando-o se preciso.
fn acrescentar_linha_exclude(exclude: &Path) -> Result<(), Erro> {
    let mut texto = match std::fs::read_to_string(exclude) {
        Ok(t) => t,
        Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(erro_io(exclude, e)),
    };
    if !texto.is_empty() && !texto.ends_with('\n') {
        texto.push('\n');
    }
    texto.push_str(LINHA_EXCLUDE);
    texto.push('\n');
    if let Some(pasta) = exclude.parent() {
        std::fs::create_dir_all(pasta).map_err(|e| erro_io(pasta, e))?;
    }
    std::fs::write(exclude, texto).map_err(|e| erro_io(exclude, e))
}

/// Tira [`LINHA_EXCLUDE`] do `info/exclude`. Devolve se tirou.
fn remover_linha_exclude(exclude: &Path) -> Result<bool, Erro> {
    let Ok(texto) = std::fs::read_to_string(exclude) else {
        return Ok(false);
    };
    let linhas: Vec<&str> = texto.lines().collect();
    let ficam: Vec<&str> = linhas
        .iter()
        .copied()
        .filter(|l| l.trim() != LINHA_EXCLUDE)
        .collect();
    if ficam.len() == linhas.len() {
        return Ok(false);
    }
    if ficam.iter().all(|l| l.trim().is_empty()) {
        // Só tinha a nossa linha: o arquivo (e a pasta `info`, se vazia) saem. O git
        // cria o `info/exclude` com comentários, então isso só acontece quando foi o
        // app que o criou.
        std::fs::remove_file(exclude).map_err(|e| erro_io(exclude, e))?;
        if let Some(info) = exclude.parent() {
            let _ = std::fs::remove_dir(info);
        }
        return Ok(true);
    }
    let mut novo = ficam.join("\n");
    novo.push('\n');
    std::fs::write(exclude, novo).map_err(|e| erro_io(exclude, e))?;
    Ok(true)
}

fn servidores<'a>(projeto: &'a Objeto, caminho: &Path) -> Result<Option<&'a Objeto>, Erro> {
    match projeto.get("mcpServers") {
        None => Ok(None),
        Some(Json::Objeto(o)) => Ok(Some(o)),
        Some(_) => Err(formato(caminho, "mcpServers")),
    }
}

/// O arquivo em `caminho` (separador `/` ou `\`) é o executável do app?
fn e_executavel_nosso(caminho: &str) -> bool {
    let nome = caminho.rsplit(['/', '\\']).next().unwrap_or("");
    let nome = nome.to_lowercase();
    nome == NOME_EXECUTAVEL || nome == format!("{NOME_EXECUTAVEL}.exe")
}

/// Entrada de servidor MCP criada pelo app (qualquer instalação): `<…/cba> mcp`.
fn e_nosso_servidor(v: &Json) -> bool {
    let Some(o) = v.como_objeto() else {
        return false;
    };
    let comando = o.get("command").and_then(Json::como_texto);
    let args = o.get("args").and_then(Json::como_lista);
    comando.is_some_and(e_executavel_nosso)
        && args.is_some_and(|a| a.len() == 1 && a[0].como_texto() == Some("mcp"))
}

/// Comando de hook criado pelo app (qualquer instalação): `"<…/cba>" hook <evento>`.
fn e_nosso_hook(comando: &str) -> bool {
    executavel_do_hook(comando).is_some()
}

/// Executável de um comando de hook do app (o caminho entre aspas).
fn executavel_do_hook(comando: &str) -> Option<&str> {
    let resto = comando.strip_prefix('"')?;
    let (caminho, depois) = resto.split_once('"')?;
    (e_executavel_nosso(caminho) && depois.starts_with(" hook ")).then_some(caminho)
}

/// Executável de um servidor MCP (`command`).
fn comando_do_servidor(v: &Json) -> Option<&str> {
    v.como_objeto()?.get("command")?.como_texto()
}

/// Executáveis dos hooks do app em `grupos`, na ordem, sem repetição.
fn executaveis_dos_hooks(grupos: &[Json]) -> Vec<&str> {
    let mut v: Vec<&str> = Vec::new();
    for e in grupos
        .iter()
        .filter_map(handlers)
        .flatten()
        .filter_map(comando_de)
        .filter_map(executavel_do_hook)
    {
        if !v.contains(&e) {
            v.push(e);
        }
    }
    v
}

fn grupo_hook(comando: &str) -> Json {
    let mut h = Objeto::default();
    h.inserir("type", Json::texto("command"));
    h.inserir("command", Json::texto(comando));
    h.inserir("timeout", Json::Numero(LIMITE_HOOK_S.into()));
    let mut g = Objeto::default();
    g.inserir("hooks", Json::Lista(vec![Json::Objeto(h)]));
    Json::Objeto(g)
}

fn comando_de(handler: &Json) -> Option<&str> {
    handler.como_objeto()?.get("command")?.como_texto()
}

fn handlers(grupo: &Json) -> Option<&Vec<Json>> {
    grupo.como_objeto()?.get("hooks")?.como_lista()
}

fn contar_nossos(grupos: &[Json], caminho: &Path, evento: &str) -> Result<usize, Erro> {
    let mut n = 0;
    for g in grupos {
        if g.como_objeto().is_none() {
            return Err(formato(caminho, &format!("hooks.{evento}")));
        }
        n += handlers(g)
            .map(|hs| {
                hs.iter()
                    .filter(|h| comando_de(h).is_some_and(e_nosso_hook))
                    .count()
            })
            .unwrap_or(0);
    }
    Ok(n)
}

/// Comandos nossos registrados para `evento`.
fn comandos_nossos<'a>(
    raiz: &'a Objeto,
    evento: &str,
    caminho: &Path,
) -> Result<Vec<&'a str>, Erro> {
    let Some(hooks) = raiz.get("hooks") else {
        return Ok(Vec::new());
    };
    let hooks = hooks
        .como_objeto()
        .ok_or_else(|| formato(caminho, "hooks"))?;
    let Some(grupos) = hooks.get(evento) else {
        return Ok(Vec::new());
    };
    let grupos = grupos
        .como_lista()
        .ok_or_else(|| formato(caminho, &format!("hooks.{evento}")))?;
    Ok(grupos
        .iter()
        .filter_map(handlers)
        .flatten()
        .filter_map(comando_de)
        .filter(|c| e_nosso_hook(c))
        .collect())
}

/// Tira os nossos handlers; grupos que ficaram vazios por isso saem também.
/// Devolve quantos foram tirados.
fn tirar_nossos(grupos: &mut Vec<Json>) -> usize {
    tirar_handlers(grupos, e_nosso_hook)
}

/// Como [`tirar_nossos`], só com os handlers do executável `exe`.
fn tirar_do_executavel(grupos: &mut Vec<Json>, exe: &str) -> usize {
    tirar_handlers(grupos, |c| executavel_do_hook(c) == Some(exe))
}

fn tirar_handlers(grupos: &mut Vec<Json>, e_para_tirar: impl Fn(&str) -> bool) -> usize {
    let mut tirados = 0;
    grupos.retain_mut(|g| {
        let Json::Objeto(o) = g else { return true };
        let Some(Json::Lista(hs)) = o.get_mut("hooks") else {
            return true;
        };
        let antes = hs.len();
        hs.retain(|h| !comando_de(h).is_some_and(&e_para_tirar));
        let agora = hs.len();
        tirados += antes - agora;
        !(agora == 0 && antes > 0)
    });
    tirados
}

/// Tira os handlers do app de `hooks.<evento>` (só os de `executavel`, se dado); o
/// evento sai se ficar vazio por isso.
fn tirar_do_evento(hooks: &mut Objeto, evento: &str, executavel: Option<&str>) -> Acao {
    let Some(Json::Lista(grupos)) = hooks.get_mut(evento) else {
        return Acao::SemMudanca;
    };
    let tirados = match executavel {
        Some(e) => tirar_do_executavel(grupos, e),
        None => tirar_nossos(grupos),
    };
    if tirados == 0 {
        return Acao::SemMudanca;
    }
    if grupos.is_empty() {
        hooks.remover(evento);
    }
    Acao::Removido
}

/// Tira o servidor `cba` de qualquer instalação da pasta `chave`, sem registro do
/// que havia antes (preparo de uma versão anterior do app).
fn remover_mcp_sem_registro(projetos: &mut Objeto, chave: &str) -> Acao {
    let Some(Json::Objeto(projeto)) = projetos.get_mut(chave) else {
        return Acao::SemMudanca;
    };
    let Some(Json::Objeto(servidores)) = projeto.get_mut("mcpServers") else {
        return Acao::SemMudanca;
    };
    match servidores.get(NOME_SERVIDOR) {
        Some(v) if e_nosso_servidor(v) => {}
        // Ausente, ou de outra pessoa: não é nosso para tirar.
        _ => return Acao::SemMudanca,
    }
    servidores.remover(NOME_SERVIDOR);
    // Projeto que só existia por nossa causa (nada além de `mcpServers` vazio).
    let sobrou_vazio = servidores.is_empty() && projeto.chaves().all(|k| k == "mcpServers");
    if sobrou_vazio {
        projetos.remover(chave);
    }
    Acao::Removido
}

/// Pastas (chaves de `projects`) com o nosso servidor; só as de `executavel`, se
/// dado.
fn pastas_com_servidor(raiz: &Objeto, executavel: Option<&str>) -> Vec<String> {
    let Some(Json::Objeto(projetos)) = raiz.get("projects") else {
        return Vec::new();
    };
    projetos
        .0
        .iter()
        .filter(|(_, p)| {
            p.como_objeto()
                .and_then(|p| p.get("mcpServers"))
                .and_then(Json::como_objeto)
                .and_then(|s| s.get(NOME_SERVIDOR))
                .is_some_and(|s| {
                    e_nosso_servidor(s)
                        && executavel.is_none_or(|e| comando_do_servidor(s) == Some(e))
                })
        })
        .map(|(k, _)| k.clone())
        .collect()
}
