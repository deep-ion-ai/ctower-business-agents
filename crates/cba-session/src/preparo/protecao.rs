//! Proteção opcional contra edição: regras `deny` na configuração do usuário do
//! Claude Code (spec, princípios e seção 10, "proteção opcional contra edição"; #29).
//!
//! # O que é gravado, e onde
//!
//! | Item | Arquivo | Escopo |
//! |---|---|---|
//! | `Edit(//<pasta do projeto>/**)` em `permissions.deny`, uma por pasta | `~/.claude/settings.json` | usuário: vale em toda sessão, de qualquer pasta |
//!
//! O `Bash` não tem regra por pasta: quem o nega, nas sessões do projeto protegido,
//! é o receptor de hooks (`PreToolUse`, [`crate::protecao`]).
//!
//! # Por que assim
//!
//! Fontes (documentação oficial, <https://code.claude.com/docs/en/permissions.md>
//! e <https://code.claude.com/docs/en/permission-modes.md>, consultadas em
//! 2026-11-11):
//!
//! - As regras `deny` são avaliadas antes de `ask` e `allow`, valem em todos os
//!   modos, inclusive `bypassPermissions`, e nenhum outro escopo as libera: a edição
//!   é negada mesmo que o analista aprove ou libere tudo.
//! - `Edit(path)` cobre todas as ferramentas que editam arquivos (`Edit`, `Write`,
//!   `NotebookEdit`) e também os comandos de arquivo que o Claude Code reconhece no
//!   Bash e os redirecionamentos (`> arquivo`). Uma regra `Write(path)` é aceita mas
//!   não é usada na conferência: por isso não é gravada.
//! - `//caminho` é absoluto; `/caminho` no `settings.json` do usuário seria relativo
//!   a `~/.claude`. No Windows, o caminho vai na forma POSIX (`//c/Users/...`).
//! - O escopo é o do usuário porque os de projeto (`.claude/settings*.json`) ficam
//!   dentro do repositório do projeto, onde o app não escreve (spec, seção 6.2), e o
//!   `~/.claude.json` não é local documentado de permissões. Como o caminho é
//!   absoluto, a regra só pega a pasta do projeto.
//! - Uma regra `Bash` sem padrão tira a ferramenta de todas as sessões do analista;
//!   regras de Bash casam o texto do comando, não a pasta. Daí o hook.
//!
//! As funções daqui só tocam as regras que recebem; quem sabe quais são do app é
//! [`crate::protecao`], que guarda as que ele mesmo acrescentou.

use std::path::{Path, PathBuf};

use serde::Serialize;

use super::json::Json;
use super::{Erro, Modo, Preparador, editar, formato};
use crate::clientes::Plataforma;

/// Regra `deny` que impede a edição de `pasta` e de tudo dentro dela.
pub fn regra_de_protecao(pasta: &Path, plataforma: Plataforma) -> String {
    let texto = pasta.to_string_lossy();
    let posix = match plataforma {
        Plataforma::Windows => {
            let t = texto.replace('\\', "/");
            match t.split_once(':') {
                Some((unidade, resto)) if unidade.len() == 1 => {
                    format!("/{}{resto}", unidade.to_lowercase())
                }
                _ => t,
            }
        }
        _ => texto.into_owned(),
    };
    // Caracteres especiais do padrão gitignore valem literalmente.
    let mut escapado = String::with_capacity(posix.len());
    for c in posix.trim_end_matches('/').chars() {
        if matches!(c, '*' | '?' | '[' | ']' | '\\') {
            escapado.push('\\');
        }
        escapado.push(c);
    }
    let absoluto = if escapado.starts_with('/') {
        format!("/{escapado}")
    } else {
        format!("//{escapado}")
    };
    format!("Edit({absoluto}/**)")
}

/// Resultado de [`Preparador::negar`] e [`Preparador::liberar`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultadoNegacoes {
    pub arquivo: PathBuf,
    /// Regras acrescentadas (em [`Preparador::negar`]) ou tiradas.
    pub alteradas: Vec<String>,
    pub backups: Vec<PathBuf>,
}

impl Preparador {
    /// Regras de proteção das pastas, no formato da plataforma.
    pub fn regras_de_protecao(&self, pastas: &[PathBuf]) -> Vec<String> {
        let mut regras: Vec<String> = pastas
            .iter()
            .map(|p| {
                regra_de_protecao(
                    &dunce::canonicalize(p).unwrap_or(p.clone()),
                    self.plataforma,
                )
            })
            .collect();
        regras.dedup();
        regras
    }

    /// Quais das regras estão em `permissions.deny` do `settings.json` do usuário.
    pub fn negacoes_presentes(&self, regras: &[String]) -> Result<Vec<bool>, Erro> {
        let lido = super::arquivo::ler(&self.config.settings)?;
        let deny = lido
            .raiz
            .get("permissions")
            .and_then(Json::como_objeto)
            .and_then(|p| p.get("deny"))
            .and_then(Json::como_lista);
        Ok(regras
            .iter()
            .map(|r| deny.is_some_and(|d| d.iter().any(|v| v.como_texto() == Some(r))))
            .collect())
    }

    /// Acrescenta as regras que faltam em `permissions.deny`, preservando o resto.
    pub fn negar(&self, regras: &[String]) -> Result<ResultadoNegacoes, Erro> {
        let mut alteradas = Vec::new();
        let backup = editar(&self.config.settings, Modo::Gravar, |raiz, caminho| {
            alteradas.clear();
            let permissoes = raiz
                .objeto_mut("permissions")
                .ok_or_else(|| formato(caminho, "permissions"))?;
            if permissoes.get("deny").is_none() {
                permissoes.inserir("deny", Json::Lista(Vec::new()));
            }
            let Some(Json::Lista(deny)) = permissoes.get_mut("deny") else {
                return Err(formato(caminho, "permissions.deny"));
            };
            for r in regras {
                if !deny.iter().any(|v| v.como_texto() == Some(r)) {
                    deny.push(Json::texto(r));
                    alteradas.push(r.clone());
                }
            }
            Ok(())
        })?;
        tracing::info!(regras = alteradas.len(), "proteção contra edição gravada");
        Ok(ResultadoNegacoes {
            arquivo: self.config.settings.clone(),
            alteradas,
            backups: backup.into_iter().collect(),
        })
    }

    /// Tira exatamente as regras informadas de `permissions.deny`. Listas e objetos
    /// que ficaram vazios por isso saem também.
    pub fn liberar(&self, regras: &[String]) -> Result<ResultadoNegacoes, Erro> {
        let mut alteradas = Vec::new();
        let backup = editar(&self.config.settings, Modo::Gravar, |raiz, caminho| {
            alteradas.clear();
            let Some(Json::Objeto(permissoes)) = raiz.get_mut("permissions") else {
                return match raiz.get("permissions") {
                    None => Ok(()),
                    Some(_) => Err(formato(caminho, "permissions")),
                };
            };
            let Some(Json::Lista(deny)) = permissoes.get_mut("deny") else {
                return match permissoes.get("deny") {
                    None => Ok(()),
                    Some(_) => Err(formato(caminho, "permissions.deny")),
                };
            };
            deny.retain(|v| match v.como_texto() {
                Some(t) if regras.iter().any(|r| r == t) => {
                    alteradas.push(t.to_owned());
                    false
                }
                _ => true,
            });
            if !alteradas.is_empty() && deny.is_empty() {
                permissoes.remover("deny");
            }
            if !alteradas.is_empty() && permissoes.is_empty() {
                raiz.remover("permissions");
            }
            Ok(())
        })?;
        tracing::info!(regras = alteradas.len(), "proteção contra edição retirada");
        Ok(ResultadoNegacoes {
            arquivo: self.config.settings.clone(),
            alteradas,
            backups: backup.into_iter().collect(),
        })
    }
}
