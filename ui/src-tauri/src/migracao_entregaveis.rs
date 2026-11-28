//! Migração para o catálogo de entregáveis (#465), na abertura do app.
//!
//! - **Documento negocial**: o documento gravado pelo "Explicar" antes do
//!   histórico de revisões vira a v1.0 do documento vivo "Documento negocial" da
//!   funcionalidade ([`cba_tools::documentos_vivos::importar_negociais`]). O
//!   arquivo na base não muda: abre igual. Só o estado dos documentos vivos
//!   (`funcionalidades/documentos.estado.json`) ganha a versão.
//! - **Histórias e cenários** continuam como análises: as já gravadas abrem,
//!   exportam e publicam como antes, com qualquer configuração. Os fluxos só
//!   deixam de ser oferecidos para criar quando o projeto não habilita os tipos
//!   deles (`cba_docs::entregaveis::fluxo_oferecido`); o projeto sem
//!   configuração continua com o legado, que habilita os dois. Por isso não há
//!   o que regravar.
//! - **Manual e protótipo** já constam no catálogo e são gerados como antes.
//!
//! Antes de gravar, copia o estado de cada projeto a migrar para
//! `<dados>/backups/antes-entregaveis-<data e hora>/<projeto>/` (nos dados do
//! app, para não sujar a base, que é publicada). Uma falha no meio volta tudo ao
//! backup e o erro vai para o log. É idempotente: sem documento a importar, não
//! grava nada, nem o backup.

use std::path::{Path, PathBuf};

use cba_core::config::Config;
use cba_core::dados::DiretorioDados;
use cba_kb::{Base, layout};
use cba_tools::documentos_vivos::{NegocialAImportar, importar_negociais, negociais_sem_historico};
use serde::Serialize;

/// O que a migração fez.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumoMigracao {
    /// Por projeto, os documentos negociais importados como v1.0.
    pub importados: Vec<(String, Vec<NegocialAImportar>)>,
    /// Projetos que ficaram como estavam, com o motivo (mapa ou estado ilegível).
    pub pulados: Vec<(String, String)>,
    /// A pasta do backup, quando houve o que migrar.
    pub backup: Option<PathBuf>,
}

/// Roda a migração na abertura do app; o resultado vai para o log.
pub fn na_abertura(dados: &DiretorioDados) {
    match migrar(dados) {
        Ok(r) if r.importados.is_empty() && r.pulados.is_empty() => {}
        Ok(r) => tracing::info!(
            projetos = ?r.importados.iter().map(|(p, d)| (p, d.len())).collect::<Vec<_>>(),
            pulados = ?r.pulados,
            backup = ?r.backup,
            "documentos negociais importados como documentos vivos"
        ),
        Err(e) => tracing::error!(erro = %e, "migração dos entregáveis desfeita"),
    }
}

/// Migra os projetos da base. Sem base ou sem nada a migrar, não grava nada.
pub fn migrar(dados: &DiretorioDados) -> Result<ResumoMigracao, String> {
    migrar_com(dados, importar_negociais)
}

/// [`migrar`] com a importação de cada projeto à parte (os testes simulam uma
/// falha no meio).
pub(crate) fn migrar_com(
    dados: &DiretorioDados,
    mut importar: impl FnMut(&cba_kb::Projeto) -> Result<Vec<NegocialAImportar>, String>,
) -> Result<ResumoMigracao, String> {
    let mut resumo = ResumoMigracao::default();
    let config = Config::ler(dados).map_err(|e| e.to_string())?;
    let Some(raiz) = config.base() else {
        return Ok(resumo);
    };
    let base = Base::abrir(raiz).map_err(|e| e.to_string())?;
    let mut pendentes = Vec::new();
    for nome in base.projetos().map_err(|e| e.to_string())? {
        let kb = base.projeto(&nome).map_err(|e| e.to_string())?;
        match negociais_sem_historico(&kb) {
            Ok(p) if p.is_empty() => {}
            Ok(_) => pendentes.push((nome, kb)),
            Err(e) => resumo.pulados.push((nome, e)),
        }
    }
    if pendentes.is_empty() {
        return Ok(resumo);
    }

    let pasta = dados.raiz().join("backups").join(sufixo());
    let backups = backup(&pasta, &pendentes)?;
    resumo.backup = Some(pasta);
    let resultado = (|| {
        let mut feitos = Vec::new();
        for (nome, kb) in &pendentes {
            let importados = importar(kb).map_err(|e| format!("projeto {nome}: {e}"))?;
            feitos.push((nome.clone(), importados));
        }
        Ok::<_, String>(feitos)
    })();
    match resultado {
        Ok(feitos) => {
            resumo.importados = feitos;
            Ok(resumo)
        }
        Err(e) => Err(match restaurar(&backups) {
            Ok(()) => format!(
                "a migração dos documentos negociais para o catálogo de entregáveis falhou e \
                 foi desfeita: {e}"
            ),
            Err(r) => format!(
                "a migração dos documentos negociais falhou ({e}) e o backup não pôde ser \
                 restaurado ({r}): o backup está em {}",
                resumo.backup.as_deref().unwrap_or(Path::new("")).display()
            ),
        }),
    }
}

/// Nome da pasta do backup: a data e a hora UTC, sem `:` (que o Windows não
/// aceita).
fn sufixo() -> String {
    let iso = cba_core::tempo::instante_iso(cba_core::tempo::agora_ms());
    format!("antes-entregaveis-{}", iso.replace(':', "-"))
}

/// Conteúdo do backup de um arquivo que não existia antes da migração.
const AUSENTE: &[u8] = b"cba: arquivo ausente antes da migracao\n";

/// Um arquivo copiado: o original e o backup.
struct Copia {
    original: PathBuf,
    backup: PathBuf,
}

/// Copia o estado dos documentos vivos de cada projeto para
/// `<pasta>/<projeto>/`.
fn backup(pasta: &Path, projetos: &[(String, cba_kb::Projeto)]) -> Result<Vec<Copia>, String> {
    let relativo = layout::estado_dos_documentos_vivos();
    let mut feitas = Vec::new();
    for (nome, kb) in projetos {
        let original = kb.caminho(&relativo);
        let destino = pasta.join(nome).join(
            original
                .file_name()
                .unwrap_or_else(|| std::ffi::OsStr::new("documentos.estado.json")),
        );
        let erro = |e: std::io::Error| format!("backup de {} não gravado: {e}", original.display());
        std::fs::create_dir_all(destino.parent().unwrap_or(pasta)).map_err(erro)?;
        if original.is_file() {
            std::fs::copy(&original, &destino).map_err(erro)?;
        } else {
            // Marca que o original não existia: a restauração o apaga.
            std::fs::write(&destino, AUSENTE).map_err(erro)?;
        }
        feitas.push(Copia {
            original,
            backup: destino,
        });
    }
    Ok(feitas)
}

/// Volta cada arquivo ao backup.
fn restaurar(copias: &[Copia]) -> Result<(), String> {
    let mut erros = Vec::new();
    for c in copias {
        let r = match std::fs::read(&c.backup) {
            Ok(bytes) if bytes == AUSENTE => match std::fs::remove_file(&c.original) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
                _ => Ok(()),
            },
            Ok(bytes) => cba_kb::arquivo::gravar(&c.original, &bytes),
            Err(e) => Err(e),
        };
        if let Err(e) = r {
            erros.push(format!("{}: {e}", c.original.display()));
        }
    }
    if erros.is_empty() {
        Ok(())
    } else {
        Err(erros.join("; "))
    }
}

#[cfg(test)]
#[path = "migracao_entregaveis_testes.rs"]
mod tests;
