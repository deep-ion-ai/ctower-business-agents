//! Preparo do ambiente: servidor MCP e hooks na configuração do Claude Code (spec,
//! seção 6.2).
//!
//! Qualquer sessão do Claude Code (terminal embutido, terminal do analista ou aba de
//! código do Claude Desktop) aberta na pasta de um sistema cadastrado precisa enxergar
//! o servidor MCP do app e mandar os eventos para o receptor de hooks, sem que nenhum
//! arquivo seja criado no repositório do sistema.
//!
//! # O que é gravado, e onde
//!
//! | Item | Arquivo | Escopo |
//! |---|---|---|
//! | Servidor MCP `cba` (`<executável> mcp`) | `~/.claude.json`, em `projects["<chave>"].mcpServers` | local: só sessões nessa pasta |
//! | Hooks dos eventos de [`crate::hooks::EVENTOS`] (`"<executável>" hook <evento>`) | `~/.claude/settings.json`, em `hooks` | usuário: todas as sessões |
//!
//! Com `CLAUDE_CONFIG_DIR` definido, os dois arquivos ficam nessa pasta
//! (`<dir>/.claude.json` e `<dir>/settings.json`).
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
//! - <https://code.claude.com/docs/en/settings.md> e
//!   <https://code.claude.com/docs/en/claude-directory.md>: `CLAUDE_CONFIG_DIR` muda
//!   `~/.claude` e `~/.claude.json`; o Desktop lê os mesmos arquivos de configuração.
//!
//! # Por que esses escopos
//!
//! - **MCP no escopo local** (e não no de projeto, `.mcp.json`) porque o de projeto
//!   cria arquivo no repositório do sistema, e não no de usuário porque o servidor só
//!   faz sentido nas pastas cadastradas.
//! - **A chave do escopo local** não está documentada. Verificado com o Claude Code
//!   2.1.322 (`claude mcp add-json --scope local` com `CLAUDE_CONFIG_DIR` temporário):
//!   numa pasta dentro de um repositório git, a chave é a raiz do repositório; numa
//!   worktree, a raiz do repositório **principal**; fora de repositório, a própria
//!   pasta. É o que [`Alvo::da_pasta`] calcula, com [`cba_index::detectar_git`].
//! - **Hooks no escopo de usuário** porque os únicos locais por projeto ficam dentro
//!   do repositório (`.claude/settings*.json`). Os hooks disparam em qualquer sessão
//!   do analista; o receptor responde `{}` rápido e ignora as sessões fora das pastas
//!   cadastradas (pelo `cwd` do evento, #25). São compartilhados entre os sistemas:
//!   saem só quando o último sistema preparado é desfeito.
//!
//! # Garantias
//!
//! - **Idempotente:** preparar de novo não muda nada se já está tudo certo; com outro
//!   executável (reinstalação em outro lugar), troca as entradas antigas em vez de
//!   duplicar.
//! - **Preserva o resto:** só as nossas entradas são tocadas. A ordem das chaves é
//!   mantida. Um servidor `cba` que não é nosso é conflito, e nada é gravado.
//! - **Reversível:** [`Preparador::desfazer`] tira exatamente o que foi posto.
//! - **Gravação atômica com cópia de segurança** (`<arquivo>.cba-backup`), veja
//!   [`arquivo`].
//! - **Sem credenciais:** os arquivos são tratados como JSON genérico; nenhum campo
//!   além dos nossos é interpretado, registrado no log ou copiado para outro lugar.
//! - **Nada nas pastas do sistema:** só os dois arquivos acima (e as cópias de
//!   segurança ao lado deles) são escritos.
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

#[cfg(test)]
mod tests;

use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::clientes::{Ambiente, AmbienteReal, Plataforma};
use crate::hooks::EVENTOS;
use json::{Json, Objeto};

/// Nome do servidor MCP na configuração (as ferramentas aparecem como `mcp__cba__*`).
pub const NOME_SERVIDOR: &str = "cba";
/// Variável de ambiente com o identificador do sistema, passada ao servidor MCP.
pub const VAR_SISTEMA: &str = "CBA_SISTEMA";
/// Limite de tempo de cada hook, em segundos (o padrão do Claude Code é 600).
pub const LIMITE_HOOK_S: u64 = 10;
/// Nome do executável do app, sem extensão: identifica as nossas entradas.
const NOME_EXECUTAVEL: &str = "cba";

/// Erro do preparo. As mensagens nunca citam o conteúdo dos arquivos.
#[derive(Debug, thiserror::Error)]
pub enum Erro {
    #[error("não foi possível localizar a pasta do usuário para achar a configuração do Claude")]
    SemPastaDoUsuario,

    #[error("o caminho do executável precisa ser absoluto e sem aspas, $ ou crase: {0}")]
    ExecutavelInvalido(PathBuf),

    #[error("pasta do sistema inacessível: {0}")]
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

    /// Arquivos padrão dentro de uma pasta do usuário.
    pub fn da_pasta_do_usuario(home: &Path) -> Self {
        Self {
            estado: home.join(".claude.json"),
            settings: home.join(".claude").join("settings.json"),
        }
    }
}

/// Pasta de um sistema, já com a chave que o Claude Code usa no escopo local.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Alvo {
    /// Identificador do sistema (vai para o servidor MCP em [`VAR_SISTEMA`]).
    pub sistema: String,
    /// Diretório de trabalho da sessão do Claude (canônico).
    pub diretorio: PathBuf,
    /// Chave em `projects` do `.claude.json`.
    pub chave: String,
    /// Se a pasta está numa worktree do git (a chave é o repositório principal).
    pub worktree: bool,
}

impl Alvo {
    /// Alvo de um sistema cadastrado: a pasta raiz (sem prefixo) ou, na falta dela, a
    /// primeira.
    pub fn do_sistema(sistema: &cba_index::Sistema) -> Result<Self, Erro> {
        let pasta = sistema
            .pastas
            .iter()
            .find(|p| p.prefixo.is_none())
            .or_else(|| sistema.pastas.first())
            .ok_or_else(|| Erro::Pasta(PathBuf::new()))?;
        Self::da_pasta(&sistema.id, &pasta.caminho)
    }

    /// Alvo de uma pasta, na plataforma atual.
    pub fn da_pasta(sistema: &str, pasta: &Path) -> Result<Self, Erro> {
        Self::da_pasta_em(sistema, pasta, Plataforma::atual())
    }

    /// Alvo de uma pasta, com a chave no formato de `plataforma`.
    pub fn da_pasta_em(sistema: &str, pasta: &Path, plataforma: Plataforma) -> Result<Self, Erro> {
        let diretorio = dunce::canonicalize(pasta).map_err(|_| Erro::Pasta(pasta.to_path_buf()))?;
        if !diretorio.is_dir() {
            return Err(Erro::Pasta(diretorio));
        }
        let (raiz, worktree) = match cba_index::detectar_git(&diretorio) {
            Some(repo) => (repo.raiz_principal, repo.worktree),
            None => (diretorio.clone(), false),
        };
        Ok(Self {
            sistema: sistema.to_string(),
            chave: chave_do_caminho(&raiz, plataforma),
            diretorio,
            worktree,
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
    /// `hooks.<evento>` no `settings.json`.
    Hook { evento: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Alteracao {
    pub arquivo: PathBuf,
    pub item: Item,
    pub acao: Acao,
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
}

impl Resultado {
    /// Se algum item mudou (ou mudaria, no plano).
    pub fn houve_mudanca(&self) -> bool {
        self.alteracoes.iter().any(|a| a.acao != Acao::SemMudanca)
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
}

/// Resultado de [`Preparador::verificar`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Verificacao {
    pub diretorio: PathBuf,
    pub chave: String,
    pub servidor_mcp: Situacao,
    pub hooks: Vec<SituacaoHook>,
    /// `disableAllHooks: true` nas configurações do usuário: os hooks não rodam.
    pub hooks_desativados: bool,
    /// Tudo no lugar e os hooks ativos.
    pub pronto: bool,
}

/// Prepara, verifica e desfaz o ambiente de um sistema.
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
    /// Configuração no local padrão do sistema atual.
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

    /// Registra o servidor MCP no escopo local de `alvo` e os hooks no escopo do
    /// usuário.
    pub fn preparar(&self, alvo: &Alvo) -> Result<Resultado, Erro> {
        self.aplicar(alvo, Modo::Gravar)
    }

    /// Confere se o servidor MCP e os hooks estão no lugar, sem gravar nada.
    pub fn verificar(&self, alvo: &Alvo) -> Result<Verificacao, Erro> {
        let estado = arquivo::ler(&self.config.estado)?;
        let servidor_mcp = match self.projeto(&estado.raiz, alvo, &estado.caminho)? {
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
        let hooks_desativados =
            matches!(settings.raiz.get("disableAllHooks"), Some(Json::Bool(true)));
        let mut hooks = Vec::new();
        for evento in EVENTOS {
            let esperado = self.comando_hook(evento);
            let comandos = comandos_nossos(&settings.raiz, evento, &settings.caminho)?;
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
            });
        }
        let pronto = servidor_mcp == Situacao::Ok
            && !hooks_desativados
            && hooks.iter().all(|h| h.situacao == Situacao::Ok);
        Ok(Verificacao {
            diretorio: alvo.diretorio.clone(),
            chave: alvo.chave.clone(),
            servidor_mcp,
            hooks,
            hooks_desativados,
            pronto,
        })
    }

    /// Tira o servidor MCP do escopo local de `alvo`. Os hooks saem junto se nenhum
    /// outro projeto ainda tiver o nosso servidor.
    pub fn desfazer(&self, alvo: &Alvo) -> Result<Resultado, Erro> {
        let mut resultado = self.resultado(alvo, Modo::Gravar);
        let mut restam = false;
        let mut acao_mcp = Acao::SemMudanca;
        if let Some(backup) = editar(&self.config.estado, Modo::Gravar, |raiz, caminho| {
            acao_mcp = self.remover_mcp(raiz, alvo, caminho)?;
            restam = algum_projeto_com_servidor(raiz);
            Ok(())
        })? {
            resultado.backups.push(backup);
        }
        resultado.alteracoes.push(Alteracao {
            arquivo: self.config.estado.clone(),
            item: Item::ServidorMcp {
                chave: alvo.chave.clone(),
            },
            acao: acao_mcp,
        });
        if !restam {
            let arq = self.config.settings.clone();
            let mut acoes = Vec::new();
            if let Some(backup) = editar(&self.config.settings, Modo::Gravar, |raiz, caminho| {
                acoes = remover_hooks(raiz, caminho)?;
                Ok(())
            })? {
                resultado.backups.push(backup);
            }
            resultado
                .alteracoes
                .extend(acoes.into_iter().map(|(evento, acao)| Alteracao {
                    arquivo: arq.clone(),
                    item: Item::Hook { evento },
                    acao,
                }));
        }
        tracing::info!(
            sistema = alvo.sistema,
            mudou = resultado.houve_mudanca(),
            hooks_mantidos = restam,
            "preparo desfeito"
        );
        Ok(resultado)
    }

    fn resultado(&self, alvo: &Alvo, modo: Modo) -> Resultado {
        Resultado {
            diretorio: alvo.diretorio.clone(),
            chave: alvo.chave.clone(),
            worktree: alvo.worktree,
            alteracoes: Vec::new(),
            backups: Vec::new(),
            gravado: modo == Modo::Gravar,
        }
    }

    fn aplicar(&self, alvo: &Alvo, modo: Modo) -> Result<Resultado, Erro> {
        let mut resultado = self.resultado(alvo, modo);

        let mut acao_mcp = Acao::SemMudanca;
        if let Some(backup) = editar(&self.config.estado, modo, |raiz, caminho| {
            acao_mcp = self.registrar_mcp(raiz, alvo, caminho)?;
            Ok(())
        })? {
            resultado.backups.push(backup);
        }
        resultado.alteracoes.push(Alteracao {
            arquivo: self.config.estado.clone(),
            item: Item::ServidorMcp {
                chave: alvo.chave.clone(),
            },
            acao: acao_mcp,
        });

        let mut acoes = Vec::new();
        if let Some(backup) = editar(&self.config.settings, modo, |raiz, caminho| {
            acoes = self.registrar_hooks(raiz, caminho)?;
            Ok(())
        })? {
            resultado.backups.push(backup);
        }
        resultado
            .alteracoes
            .extend(acoes.into_iter().map(|(evento, acao)| Alteracao {
                arquivo: self.config.settings.clone(),
                item: Item::Hook { evento },
                acao,
            }));

        if modo == Modo::Gravar {
            tracing::info!(
                sistema = alvo.sistema,
                worktree = alvo.worktree,
                mudou = resultado.houve_mudanca(),
                "ambiente preparado"
            );
        }
        Ok(resultado)
    }

    /// Entrada do servidor MCP, no formato que `claude mcp add-json` grava.
    fn entrada_mcp(&self, alvo: &Alvo) -> Json {
        let mut env = Objeto::default();
        env.inserir(VAR_SISTEMA, Json::texto(&alvo.sistema));
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
        let caminho = self.executavel.to_string_lossy();
        let caminho = match self.plataforma {
            Plataforma::Windows => caminho.replace('\\', "/"),
            _ => caminho.into_owned(),
        };
        format!("\"{caminho}\" hook {evento}")
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

    fn registrar_mcp(&self, raiz: &mut Objeto, alvo: &Alvo, caminho: &Path) -> Result<Acao, Erro> {
        let projetos = raiz
            .objeto_mut("projects")
            .ok_or_else(|| formato(caminho, "projects"))?;
        let chave = self
            .chave_existente(projetos, alvo)
            .unwrap_or_else(|| alvo.chave.clone());
        let projeto = projetos
            .objeto_mut(&chave)
            .ok_or_else(|| formato(caminho, &format!("projects[{chave:?}]")))?;
        let servidores = projeto
            .objeto_mut("mcpServers")
            .ok_or_else(|| formato(caminho, "mcpServers"))?;
        let nova = self.entrada_mcp(alvo);
        let acao = match servidores.get(NOME_SERVIDOR) {
            None => Acao::Adicionado,
            Some(v) if v.para_value() == nova.para_value() => return Ok(Acao::SemMudanca),
            Some(v) if e_nosso_servidor(v) => Acao::Atualizado,
            Some(_) => {
                return Err(Erro::Conflito {
                    arquivo: caminho.to_path_buf(),
                    chave,
                });
            }
        };
        servidores.inserir(NOME_SERVIDOR, nova);
        Ok(acao)
    }

    fn remover_mcp(&self, raiz: &mut Objeto, alvo: &Alvo, caminho: &Path) -> Result<Acao, Erro> {
        let Some(projetos) = raiz.get_mut("projects") else {
            return Ok(Acao::SemMudanca);
        };
        let Json::Objeto(projetos) = projetos else {
            return Err(formato(caminho, "projects"));
        };
        let Some(chave) = self.chave_existente(projetos, alvo) else {
            return Ok(Acao::SemMudanca);
        };
        let Some(Json::Objeto(projeto)) = projetos.get_mut(&chave) else {
            return Err(formato(caminho, &format!("projects[{chave:?}]")));
        };
        let Some(Json::Objeto(servidores)) = projeto.get_mut("mcpServers") else {
            return Ok(Acao::SemMudanca);
        };
        match servidores.get(NOME_SERVIDOR) {
            Some(v) if e_nosso_servidor(v) => {}
            // Ausente, ou de outra pessoa: não é nosso para tirar.
            _ => return Ok(Acao::SemMudanca),
        }
        servidores.remover(NOME_SERVIDOR);
        // Projeto que só existia por nossa causa (nada além de `mcpServers` vazio).
        let sobrou_vazio = servidores.is_empty() && projeto.chaves().all(|k| k == "mcpServers");
        if sobrou_vazio {
            projetos.remover(&chave);
        }
        Ok(Acao::Removido)
    }

    fn registrar_hooks(
        &self,
        raiz: &mut Objeto,
        caminho: &Path,
    ) -> Result<Vec<(String, Acao)>, Erro> {
        let hooks = raiz
            .objeto_mut("hooks")
            .ok_or_else(|| formato(caminho, "hooks"))?;
        let mut acoes = Vec::new();
        for evento in EVENTOS {
            let comando = self.comando_hook(evento);
            let grupo = grupo_hook(&comando);
            if hooks.get(evento).is_none() {
                hooks.inserir(evento, Json::Lista(Vec::new()));
            }
            let Some(Json::Lista(grupos)) = hooks.get_mut(evento) else {
                return Err(formato(caminho, &format!("hooks.{evento}")));
            };
            let nossos = contar_nossos(grupos, caminho, evento)?;
            let ja_certo =
                nossos == 1 && grupos.iter().any(|g| g.para_value() == grupo.para_value());
            let acao = if ja_certo {
                Acao::SemMudanca
            } else {
                tirar_nossos(grupos);
                grupos.push(grupo);
                if nossos == 0 {
                    Acao::Adicionado
                } else {
                    Acao::Atualizado
                }
            };
            acoes.push((evento.to_string(), acao));
        }
        Ok(acoes)
    }
}

fn formato(caminho: &Path, campo: &str) -> Erro {
    Erro::Formato {
        arquivo: caminho.to_path_buf(),
        campo: campo.to_string(),
    }
}

/// Lê, edita e (no modo de gravação) grava se algo mudou. Repete se o arquivo mudar
/// entre a leitura e a gravação. Devolve a cópia de segurança, se houve.
fn editar<F>(caminho: &Path, modo: Modo, mut f: F) -> Result<Option<PathBuf>, Erro>
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
        match arquivo::gravar(&lido, &raiz) {
            Err(Erro::AlteradoDuranteGravacao(_)) if tentativa < 4 => tentativa += 1,
            outro => return outro,
        }
    }
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
    let Some(resto) = comando.strip_prefix('"') else {
        return false;
    };
    let Some((caminho, depois)) = resto.split_once('"') else {
        return false;
    };
    e_executavel_nosso(caminho) && depois.starts_with(" hook ")
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
    let mut tirados = 0;
    grupos.retain_mut(|g| {
        let Json::Objeto(o) = g else { return true };
        let Some(Json::Lista(hs)) = o.get_mut("hooks") else {
            return true;
        };
        let antes = hs.len();
        hs.retain(|h| !comando_de(h).is_some_and(e_nosso_hook));
        let agora = hs.len();
        tirados += antes - agora;
        !(agora == 0 && antes > 0)
    });
    tirados
}

fn remover_hooks(raiz: &mut Objeto, caminho: &Path) -> Result<Vec<(String, Acao)>, Erro> {
    let mut acoes = Vec::new();
    let Some(hooks) = raiz.get_mut("hooks") else {
        return Ok(EVENTOS
            .iter()
            .map(|e| (e.to_string(), Acao::SemMudanca))
            .collect());
    };
    let Json::Objeto(hooks) = hooks else {
        return Err(formato(caminho, "hooks"));
    };
    for evento in EVENTOS {
        let acao = match hooks.get_mut(evento) {
            Some(Json::Lista(grupos)) => {
                let tirados = tirar_nossos(grupos);
                if tirados > 0 && grupos.is_empty() {
                    hooks.remover(evento);
                }
                if tirados > 0 {
                    Acao::Removido
                } else {
                    Acao::SemMudanca
                }
            }
            None => Acao::SemMudanca,
            Some(_) => return Err(formato(caminho, &format!("hooks.{evento}"))),
        };
        acoes.push((evento.to_string(), acao));
    }
    let vazio = hooks.is_empty();
    if vazio && acoes.iter().any(|(_, a)| *a == Acao::Removido) {
        raiz.remover("hooks");
    }
    Ok(acoes)
}

/// Algum projeto do `.claude.json` ainda tem o nosso servidor?
fn algum_projeto_com_servidor(raiz: &Objeto) -> bool {
    let Some(Json::Objeto(projetos)) = raiz.get("projects") else {
        return false;
    };
    projetos.0.iter().any(|(_, p)| {
        p.como_objeto()
            .and_then(|p| p.get("mcpServers"))
            .and_then(Json::como_objeto)
            .and_then(|s| s.get(NOME_SERVIDOR))
            .is_some_and(e_nosso_servidor)
    })
}
