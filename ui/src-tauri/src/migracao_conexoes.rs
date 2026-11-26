//! Migração dos cadastros de hoje para conexões (#438), na primeira abertura com
//! a versão nova.
//!
//! - Antes de qualquer gravação, faz um backup datado, ao lado dos originais, da
//!   configuração do app (`config/app.json` e `config/conexoes.json`) e do
//!   cadastro de cada projeto (`projetos/<id>/projeto.json`).
//! - Agrupa e cria uma conexão por grupo: o rastreador e os repositórios (de
//!   código e da documentação) por tipo, endereço, comando de credencial e
//!   arquivo do jira-cli. O nome vem do host ("GitLab gitlab.example.com").
//! - A credencial não é copiada: o token global do host (`host:<host>` no
//!   chaveiro) vira a fonte `Chaveiro` daquela mesma conta; o `credential_cmd`
//!   (ou o comando do repositório, ou o `comando:host:<host>`) vira `Comando`; o
//!   token próprio do repositório (`repo:<host>/<caminho>`) fica onde está e vale
//!   como sobrescrita.
//! - Reescreve os projetos para apontar para as conexões, mantendo os campos
//!   próprios (chave, JQL, caminho, pasta, branch, subpasta).
//! - É idempotente: um cadastro que já aponta para uma conexão não muda, e uma
//!   conexão igual à que seria criada é reaproveitada.
//! - Uma falha no meio volta ao backup: o app abre com os cadastros antigos (que
//!   continuam valendo pelo caminho legado) e mostra o erro.
//!
//! O resumo vai para o log, sem segredos: quantas conexões foram criadas e quais
//! projetos foram migrados.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use cba_core::dados::DiretorioDados;
use cba_index::{Cadastro, Projeto};
use cba_repos::chaveiro::{Chaveiro, comando_global, conta_global};
use cba_repos::codigo::HOST_GITHUB;
use cba_repos::providers::{Conexao, FonteCredencial, host_de, todos};
use cba_repos::{OrigemToken, RepositorioCodigo, TrackerConfig};
use serde::Serialize;

use crate::conexoes;

/// O que a migração fez.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumoMigracao {
    /// Conexões criadas (nomes).
    pub criadas: Vec<String>,
    /// Projetos reescritos para apontar para as conexões.
    pub projetos: Vec<String>,
    /// Projetos que ficaram como estavam, com o motivo (cadastro inválido).
    pub pulados: Vec<(String, String)>,
    /// Os arquivos de backup gravados, ao lado dos originais.
    pub backups: Vec<PathBuf>,
}

/// Chave de agrupamento: tipo, endereço normalizado, comando e arquivo do
/// jira-cli.
type Grupo = (String, String, String, String);

/// O erro da última migração, para a tela mostrar.
static ERRO: Mutex<Option<String>> = Mutex::new(None);

/// O erro da migração desta abertura, se houve.
pub fn erro() -> Option<String> {
    ERRO.lock().ok().and_then(|e| e.clone())
}

/// Roda a migração na abertura do app e guarda o erro para a tela.
pub fn na_abertura(dados: &DiretorioDados, chaveiro: &dyn Chaveiro) {
    match migrar(dados, chaveiro) {
        Ok(r) if r.criadas.is_empty() && r.projetos.is_empty() => {}
        Ok(r) => tracing::info!(
            conexoes_criadas = r.criadas.len(),
            conexoes = ?r.criadas,
            projetos = ?r.projetos,
            pulados = ?r.pulados.iter().map(|(p, _)| p).collect::<Vec<_>>(),
            "cadastros migrados para conexões"
        ),
        Err(e) => {
            tracing::error!(erro = %e, "migração para conexões desfeita");
            if let Ok(mut g) = ERRO.lock() {
                *g = Some(e);
            }
        }
    }
}

/// Migra os cadastros. Sem nada a migrar, não grava nada (nem backup).
pub fn migrar(dados: &DiretorioDados, chaveiro: &dyn Chaveiro) -> Result<ResumoMigracao, String> {
    migrar_com(dados, chaveiro, |cadastro, p| {
        cadastro.salvar(p).map_err(|e| e.to_string())
    })
}

/// [`migrar`] com a gravação de cada projeto à parte (os testes simulam uma
/// falha no meio).
pub(crate) fn migrar_com(
    dados: &DiretorioDados,
    chaveiro: &dyn Chaveiro,
    mut gravar_projeto: impl FnMut(&Cadastro, &Projeto) -> Result<(), String>,
) -> Result<ResumoMigracao, String> {
    let cadastro = Cadastro::em(dados.projetos());
    let projetos = cadastro.listar().map_err(|e| e.to_string())?;
    let mut resumo = ResumoMigracao::default();
    let pendentes: Vec<Projeto> = projetos
        .into_iter()
        .filter(precisa_migrar)
        .filter(|p| match p.validar() {
            Ok(()) => true,
            Err(e) => {
                resumo.pulados.push((p.id.clone(), e.to_string()));
                false
            }
        })
        .collect();
    if pendentes.is_empty() {
        return Ok(resumo);
    }

    resumo.backups = backup(dados, &pendentes)?;
    let mut pulados = Vec::new();
    let resultado = (|| {
        let mut lista = conexoes::conexoes(dados)?;
        let antes = lista.len();
        let mut migrados = Vec::new();
        for mut p in pendentes {
            // Um cadastro que não se converte (endereço inválido) fica como
            // estava, pelo caminho legado, e os outros seguem.
            let mut tentativa = lista.clone();
            let r = apontar(&mut p.repositorios, &mut tentativa, chaveiro)
                .and_then(|()| conexoes::aplicar_conexoes(&mut p.repositorios, &tentativa));
            match r {
                Ok(()) => {
                    lista = tentativa;
                    migrados.push(p);
                }
                Err(e) => pulados.push((p.id.clone(), e)),
            }
        }
        conexoes::gravar(dados, &lista)?;
        for p in &migrados {
            gravar_projeto(&cadastro, p).map_err(|e| format!("projeto {}: {e}", p.nome))?;
        }
        Ok::<_, String>((
            lista[antes..].iter().map(|c| c.nome.clone()).collect(),
            migrados.into_iter().map(|p| p.id).collect(),
        ))
    })();
    match resultado {
        Ok((criadas, projetos)) => {
            resumo.criadas = criadas;
            resumo.projetos = projetos;
            resumo.pulados.extend(pulados);
            Ok(resumo)
        }
        Err(e) => {
            let restaurado = restaurar(&resumo.backups);
            Err(match restaurado {
                Ok(()) => format!(
                    "a migração dos cadastros para conexões falhou e foi desfeita (os cadastros \
                     antigos continuam valendo): {e}"
                ),
                Err(r) => format!(
                    "a migração dos cadastros para conexões falhou ({e}) e o backup não pôde \
                     ser restaurado ({r}): os arquivos de backup estão ao lado dos originais"
                ),
            })
        }
    }
}

/// Algum cadastro de rastreador ou repositório ainda sem conexão.
fn precisa_migrar(p: &Projeto) -> bool {
    let r = &p.repositorios;
    let sem = |c: &str| c.trim().is_empty();
    r.codigo.iter().any(|c| sem(&c.conexao))
        || r.docs
            .as_ref()
            .and_then(|d| d.repositorio_proprio())
            .is_some_and(|c| sem(&c.conexao))
        || r.tracker
            .as_ref()
            .is_some_and(|t| sem(conexao_do_rastreador(t)))
}

fn conexao_do_rastreador(t: &TrackerConfig) -> &str {
    match t {
        TrackerConfig::Github(g) => &g.conexao,
        TrackerConfig::Gitlab(g) => &g.conexao,
        TrackerConfig::Jira(j) => &j.conexao,
    }
}

/// Sufixo do backup: a data e a hora UTC, sem `:` (que o Windows não aceita).
fn sufixo() -> String {
    let iso = cba_core::tempo::instante_iso(cba_core::tempo::agora_ms());
    format!("antes-conexoes-{}", iso.replace(':', "-"))
}

/// Copia cada arquivo para `<arquivo>.<sufixo>` ao lado dele. Devolve os pares
/// gravados (o backup e, implícito, o original sem o sufixo).
fn backup(dados: &DiretorioDados, projetos: &[Projeto]) -> Result<Vec<PathBuf>, String> {
    let sufixo = sufixo();
    let mut arquivos = vec![dados.config().join("app.json"), conexoes::arquivo(dados)];
    arquivos.extend(
        projetos
            .iter()
            .map(|p| dados.projetos().join(&p.id).join("projeto.json")),
    );
    let mut feitos = Vec::new();
    for a in arquivos {
        let destino = com_sufixo(&a, &sufixo);
        if a.is_file() {
            std::fs::copy(&a, &destino)
                .map_err(|e| format!("backup de {} não gravado: {e}", a.display()))?;
        } else {
            // Marca que o original não existia: a restauração o apaga.
            std::fs::create_dir_all(destino.parent().unwrap_or(Path::new(".")))
                .and_then(|()| std::fs::write(&destino, AUSENTE))
                .map_err(|e| format!("backup de {} não gravado: {e}", a.display()))?;
        }
        feitos.push(destino);
    }
    Ok(feitos)
}

/// Conteúdo do backup de um arquivo que não existia antes da migração.
const AUSENTE: &[u8] = b"cba: arquivo ausente antes da migracao\n";

fn com_sufixo(a: &Path, sufixo: &str) -> PathBuf {
    let mut nome = a.file_name().unwrap_or_default().to_os_string();
    nome.push(".");
    nome.push(sufixo);
    a.with_file_name(nome)
}

fn original(backup: &Path) -> PathBuf {
    let nome = backup.file_name().unwrap_or_default().to_string_lossy();
    let base = nome
        .rsplit_once(".antes-conexoes-")
        .map_or(nome.as_ref(), |(b, _)| b);
    backup.with_file_name(base)
}

/// Volta cada arquivo ao backup.
fn restaurar(backups: &[PathBuf]) -> Result<(), String> {
    let mut erros = Vec::new();
    for b in backups {
        let o = original(b);
        let r = match std::fs::read(b) {
            Ok(bytes) if bytes == AUSENTE => match std::fs::remove_file(&o) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
                _ => Ok(()),
            },
            Ok(bytes) => cba_kb::arquivo::gravar(&o, &bytes),
            Err(e) => Err(e),
        };
        if let Err(e) = r {
            erros.push(format!("{}: {e}", o.display()));
        }
    }
    if erros.is_empty() {
        Ok(())
    } else {
        Err(erros.join("; "))
    }
}

/// A fonte da credencial global de um host, sem ler o valor: o comando do host
/// (`comando:host:<host>`) quando ele existe e não há token guardado; senão, a
/// conta `host:<host>` do chaveiro (mesmo vazia: vale como hoje, sem token, e
/// passa a valer quando o analista guardar um).
fn fonte_do_host(host: &str, chaveiro: &dyn Chaveiro) -> Result<FonteCredencial, String> {
    let conta = conta_global(host);
    if chaveiro.ler(&conta)?.is_none()
        && let Some(cmd) = comando_global(host, chaveiro)?
    {
        return Ok(FonteCredencial::Comando { cmd });
    }
    Ok(FonteCredencial::Chaveiro { conta })
}

/// A conexão do grupo: uma já cadastrada igual ou uma nova, acrescentada à
/// lista. Devolve o `id`.
fn conexao_do_grupo(
    grupo: &Grupo,
    lista: &mut Vec<Conexao>,
    chaveiro: &dyn Chaveiro,
) -> Result<String, String> {
    let (tipo, endereco, cmd, config_file) = grupo;
    let d = todos()
        .get(tipo)
        .ok_or_else(|| format!("tipo de conexão desconhecido: {tipo}"))?;
    let host = host_de(endereco);
    let credencial = if cmd.is_empty() {
        fonte_do_host(&host, chaveiro)?
    } else {
        FonteCredencial::Comando { cmd: cmd.clone() }
    };
    let mut opcoes = BTreeMap::new();
    if !config_file.is_empty() {
        opcoes.insert("config_file".to_string(), config_file.clone());
    }
    if let Some(c) = lista.iter().find(|c| {
        c.tipo == *tipo
            && c.endereco == *endereco
            && c.credencial == credencial
            && c.opcoes == opcoes
    }) {
        return Ok(c.id.clone());
    }
    let base = format!("{} {host}", d.rotulo());
    let mut nome = base.clone();
    let mut n = 2;
    while lista
        .iter()
        .any(|c| c.nome.trim().eq_ignore_ascii_case(&nome))
    {
        nome = format!("{base} ({n})");
        n += 1;
    }
    let mut id = cba_index::slug(&nome);
    let mut n = 2;
    while lista.iter().any(|c| c.id == id) {
        id = format!("{}-{n}", cba_index::slug(&nome));
        n += 1;
    }
    let c = Conexao {
        id: id.clone(),
        nome,
        tipo: tipo.clone(),
        endereco: endereco.clone(),
        credencial,
        opcoes,
    };
    todos()
        .validar(&c)
        .map_err(|e| format!("conexão \"{}\": {e}", c.nome))?;
    lista.push(c);
    Ok(id)
}

/// O grupo de um repositório de código: forja, host e, com `token = comando`,
/// o comando do repositório.
fn grupo_do_repositorio(r: &RepositorioCodigo) -> Result<Grupo, String> {
    let tipo = r.forja.as_str().to_string();
    let d = todos()
        .get(&tipo)
        .ok_or_else(|| format!("tipo de conexão desconhecido: {tipo}"))?;
    let host = if r.host.trim().is_empty() {
        HOST_GITHUB.to_string()
    } else {
        r.host.trim().to_string()
    };
    let endereco = d
        .normalizar_endereco(&host)
        .map_err(|e| format!("repositório {}: {e}", r.rotulo()))?;
    let cmd = if r.token == OrigemToken::Comando {
        r.comando_credencial.trim().to_string()
    } else {
        String::new()
    };
    Ok((tipo, endereco, cmd, String::new()))
}

/// Aponta cada cadastro sem conexão para a conexão do grupo dele.
fn apontar(
    r: &mut cba_repos::Repositorios,
    lista: &mut Vec<Conexao>,
    chaveiro: &dyn Chaveiro,
) -> Result<(), String> {
    let mut repo = |c: &mut RepositorioCodigo| -> Result<(), String> {
        if c.conexao.trim().is_empty() {
            c.conexao = conexao_do_grupo(&grupo_do_repositorio(c)?, lista, chaveiro)?;
        }
        Ok(())
    };
    for c in &mut r.codigo {
        repo(c)?;
    }
    if let Some(cba_repos::docs::DestinoDocs::Repositorio { repositorio, .. }) = &mut r.docs {
        repo(repositorio)?;
    }
    let grupo = match &r.tracker {
        Some(t) if conexao_do_rastreador(t).trim().is_empty() => {
            let tipo = t.kind().as_str();
            let d = todos()
                .get(tipo)
                .ok_or_else(|| format!("tipo de conexão desconhecido: {tipo}"))?;
            let (endereco, cmd, config_file) = match t {
                TrackerConfig::Github(_) => (HOST_GITHUB, "", ""),
                TrackerConfig::Gitlab(g) => (g.host.as_str(), g.credential_cmd.as_str(), ""),
                TrackerConfig::Jira(j) => (
                    j.url.as_str(),
                    j.credential_cmd.as_str(),
                    j.config_file.as_str(),
                ),
            };
            Some((
                tipo.to_string(),
                d.normalizar_endereco(endereco)
                    .map_err(|e| format!("repositório de issues: {e}"))?,
                cmd.trim().to_string(),
                config_file.trim().to_string(),
            ))
        }
        _ => None,
    };
    if let Some(g) = grupo {
        let id = conexao_do_grupo(&g, lista, chaveiro)?;
        match &mut r.tracker {
            Some(TrackerConfig::Github(t)) => t.conexao = id,
            Some(TrackerConfig::Gitlab(t)) => t.conexao = id,
            Some(TrackerConfig::Jira(t)) => t.conexao = id,
            None => {}
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "migracao_conexoes_testes.rs"]
mod tests;
