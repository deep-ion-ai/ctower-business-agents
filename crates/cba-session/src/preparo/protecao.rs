//! Proteção opcional contra edição: regras `deny` nas configurações locais das pastas
//! do projeto (spec, princípios e seção 10, "proteção opcional contra edição"; #29,
//! #304).
//!
//! # O que é gravado, e onde
//!
//! | Item | Arquivo | Escopo |
//! |---|---|---|
//! | `Edit(//<pasta do projeto>/**)` em `permissions.deny`, uma por pasta | `<raiz>/.claude/settings.local.json` da pasta ([`super::arquivo_local`]) | local: só sessões nessa raiz, com o caminho absoluto da pasta |
//!
//! Instalações anteriores à #304 gravavam as regras no `~/.claude/settings.json`; as
//! funções sem `_em` ([`Preparador::negar`], [`Preparador::liberar`]) ainda tratam
//! esse arquivo, para a migração (#305).
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
//! - O escopo é o local (`.claude/settings.local.json`, pessoal e fora do
//!   versionamento; spec, seção 6.2): vale só nas sessões da raiz da pasta e não se
//!   mistura às regras do analista no escopo do usuário. Verificado no spike da #303:
//!   a regra local nega `Edit` e `Write` mesmo com `allow` em outro escopo e o modo
//!   `acceptEdits`. Como o caminho é absoluto, a regra só pega a pasta do projeto,
//!   mesmo gravada na raiz do repositório.
//! - Uma regra `Bash` sem padrão tira a ferramenta de todas as sessões do analista;
//!   regras de Bash casam o texto do comando, não a pasta. Daí o hook.
//!
//! As funções daqui só tocam as regras que recebem; quem sabe quais são do app é
//! [`crate::protecao`], que guarda as que ele mesmo acrescentou.

use std::path::{Path, PathBuf};

use serde::Serialize;

use super::json::Json;
use super::{Erro, Modo, Preparador, arquivo_local, editar_com, formato, raiz_local};
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

    /// Regra de proteção de cada pasta e o `settings.local.json` (da raiz da pasta)
    /// em que ela vai, sem repetição.
    pub fn regras_por_arquivo(&self, pastas: &[PathBuf]) -> Vec<(PathBuf, String)> {
        let mut v: Vec<(PathBuf, String)> = Vec::new();
        for p in pastas {
            let pasta = dunce::canonicalize(p).unwrap_or(p.clone());
            let (raiz, _) = raiz_local(&pasta);
            let item = (
                arquivo_local(&raiz),
                regra_de_protecao(&pasta, self.plataforma),
            );
            if !v.contains(&item) {
                v.push(item);
            }
        }
        v
    }

    /// Quais das regras estão em `permissions.deny` do `settings.json` do usuário.
    pub fn negacoes_presentes(&self, regras: &[String]) -> Result<Vec<bool>, Erro> {
        self.negacoes_presentes_em(&self.config.settings, regras)
    }

    /// Quais das regras estão em `permissions.deny` de `arquivo`.
    pub fn negacoes_presentes_em(
        &self,
        arquivo: &Path,
        regras: &[String],
    ) -> Result<Vec<bool>, Erro> {
        let lido = super::arquivo::ler(arquivo)?;
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

    /// Acrescenta as regras que faltam em `permissions.deny` do `settings.json` do
    /// usuário (instalações anteriores à #304).
    pub fn negar(&self, regras: &[String]) -> Result<ResultadoNegacoes, Erro> {
        self.negar_em(&self.config.settings.clone(), regras)
    }

    /// Acrescenta as regras que faltam em `permissions.deny` de `arquivo`,
    /// preservando o resto. Um arquivo local fica fora do versionamento como no
    /// preparo ([`super::LINHA_EXCLUDE`]).
    pub fn negar_em(&self, arquivo: &Path, regras: &[String]) -> Result<ResultadoNegacoes, Erro> {
        let local = arquivo != self.config.settings;
        let backup_local = local.then(|| self.config.backup_de(arquivo));
        let mut alteradas = Vec::new();
        let backup = editar_com(
            arquivo,
            Modo::Gravar,
            backup_local.as_deref(),
            false,
            |raiz, caminho| {
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
            },
        )?;
        if local && !alteradas.is_empty() {
            self.garantir_exclude(arquivo)?;
        }
        tracing::info!(regras = alteradas.len(), "proteção contra edição gravada");
        Ok(ResultadoNegacoes {
            arquivo: arquivo.to_path_buf(),
            alteradas,
            backups: backup.into_iter().collect(),
        })
    }

    /// Tira as regras de `permissions.deny` do `settings.json` do usuário
    /// (instalações anteriores à #304).
    pub fn liberar(&self, regras: &[String]) -> Result<ResultadoNegacoes, Erro> {
        self.liberar_em(&self.config.settings.clone(), regras)
    }

    /// Tira exatamente as regras informadas de `permissions.deny` de `arquivo`.
    /// Listas e objetos que ficaram vazios por isso saem também; um arquivo local que
    /// fica vazio é apagado.
    pub fn liberar_em(&self, arquivo: &Path, regras: &[String]) -> Result<ResultadoNegacoes, Erro> {
        let local = arquivo != self.config.settings;
        let backup_local = local.then(|| self.config.backup_de(arquivo));
        let mut alteradas = Vec::new();
        let backup = editar_com(
            arquivo,
            Modo::Gravar,
            backup_local.as_deref(),
            local,
            |raiz, caminho| {
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
            },
        )?;
        tracing::info!(regras = alteradas.len(), "proteção contra edição retirada");
        Ok(ResultadoNegacoes {
            arquivo: arquivo.to_path_buf(),
            alteradas,
            backups: backup.into_iter().collect(),
        })
    }

    /// Garante a linha do `info/exclude` para o arquivo local (`<raiz>/.claude/
    /// settings.local.json`), registrando-a como do app para o desfazer do preparo.
    fn garantir_exclude(&self, arquivo: &Path) -> Result<(), Erro> {
        let Some(raiz) = arquivo.parent().and_then(Path::parent) else {
            return Ok(());
        };
        let Some(exclude) = super::arquivo_exclude(raiz) else {
            return Ok(());
        };
        if super::tem_linha(&exclude, super::e_linha_do_arquivo_local) {
            return Ok(());
        }
        let mut registro = super::registro::Registro::ler(&self.config.registro());
        let chave = super::chave_do_caminho(raiz, self.plataforma);
        self.registrar_exclude(&mut registro, &chave, true, true)?;
        super::acrescentar_linha(&exclude, super::LINHA_EXCLUDE)
    }
}
