//! Gravação atômica de arquivos.
//!
//! O conteúdo vai para um arquivo temporário na mesma pasta do destino (nome
//! começando com `.cba-`), é sincronizado no disco e só então renomeado para o
//! destino. Uma queda no meio deixa o documento anterior intacto, nunca um arquivo
//! pela metade. A renomeação é atômica no mesmo sistema de arquivos, no Windows,
//! no macOS e no Linux.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use tempfile::{NamedTempFile, PersistError};

/// Prefixo dos temporários. Quem lista a base ignora arquivos que começam com `.`.
pub const PREFIXO_TEMPORARIO: &str = ".cba-";

/// No Windows, renomear sobre um arquivo aberto por outro processo (antivírus,
/// indexador, editor) falha com acesso negado por alguns instantes.
const TENTATIVAS: u32 = 10;
const ESPERA: Duration = Duration::from_millis(50);

fn temporario(destino: &Path, conteudo: &[u8]) -> io::Result<NamedTempFile> {
    let pasta = pasta_de(destino);
    fs::create_dir_all(&pasta)?;
    let mut tmp = tempfile::Builder::new()
        .prefix(PREFIXO_TEMPORARIO)
        .suffix(".tmp")
        .tempfile_in(&pasta)?;
    tmp.write_all(conteudo)?;
    tmp.as_file().sync_all()?;
    Ok(tmp)
}

fn pasta_de(destino: &Path) -> PathBuf {
    match destino.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => PathBuf::from("."),
    }
}

/// Garante a renomeação no disco (Unix). No Windows não há equivalente e a
/// renomeação já é registrada pelo sistema de arquivos.
fn sincronizar_pasta(destino: &Path) {
    #[cfg(unix)]
    if let Ok(pasta) = fs::File::open(pasta_de(destino)) {
        let _ = pasta.sync_all();
    }
    #[cfg(not(unix))]
    let _ = destino;
}

fn com_tentativas(
    mut tmp: NamedTempFile,
    mut persistir: impl FnMut(NamedTempFile) -> Result<(), PersistError>,
) -> io::Result<()> {
    let mut tentativa = 1;
    loop {
        match persistir(tmp) {
            Ok(()) => return Ok(()),
            Err(e)
                if e.error.kind() == io::ErrorKind::PermissionDenied && tentativa < TENTATIVAS =>
            {
                tentativa += 1;
                tmp = e.file;
                thread::sleep(ESPERA);
            }
            // O temporário é apagado quando `e.file` sai de escopo.
            Err(e) => return Err(e.error),
        }
    }
}

/// Grava `conteudo` em `destino`, substituindo o arquivo se existir. Cria as
/// pastas que faltarem.
pub fn gravar(destino: &Path, conteudo: &[u8]) -> io::Result<()> {
    let tmp = temporario(destino, conteudo)?;
    com_tentativas(tmp, |t| t.persist(destino).map(drop))?;
    sincronizar_pasta(destino);
    Ok(())
}

/// Grava `conteudo` em `destino` só se ele ainda não existir. Se existir, falha
/// com [`io::ErrorKind::AlreadyExists`] e não altera nada. A verificação e a
/// criação são uma operação só, então dois processos nunca criam o mesmo arquivo.
pub fn criar(destino: &Path, conteudo: &[u8]) -> io::Result<()> {
    let tmp = temporario(destino, conteudo)?;
    com_tentativas(tmp, |t| t.persist_noclobber(destino).map(drop))?;
    sincronizar_pasta(destino);
    Ok(())
}

/// A pasta de trava de uma reserva (`create_dir`) já está tomada. No Windows,
/// criar uma pasta que outro processo está apagando falha com acesso negado em
/// vez de "já existe"; para a reserva, as duas querem dizer "tente o próximo
/// número" (o laço de reserva tem tentativas limitadas).
pub(crate) fn trava_ocupada(erro: &std::io::Error) -> bool {
    erro.kind() == std::io::ErrorKind::AlreadyExists
        || (cfg!(windows) && erro.kind() == std::io::ErrorKind::PermissionDenied)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporarios(pasta: &Path) -> Vec<String> {
        fs::read_dir(pasta)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with(PREFIXO_TEMPORARIO))
            .collect()
    }

    #[test]
    fn grava_substitui_e_nao_deixa_temporario() {
        let dir = tempfile::tempdir().unwrap();
        let destino = dir.path().join("pasta nova").join("cobrança.md");
        gravar(&destino, "primeira".as_bytes()).unwrap();
        gravar(&destino, "segunda".as_bytes()).unwrap();
        assert_eq!(fs::read_to_string(&destino).unwrap(), "segunda");
        assert!(temporarios(destino.parent().unwrap()).is_empty());
    }

    #[test]
    fn criar_nao_sobrescreve() {
        let dir = tempfile::tempdir().unwrap();
        let destino = dir.path().join("A-0001.md");
        criar(&destino, b"um").unwrap();
        let erro = criar(&destino, b"dois").unwrap_err();
        assert_eq!(erro.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read_to_string(&destino).unwrap(), "um");
        assert!(temporarios(dir.path()).is_empty());
    }
}
