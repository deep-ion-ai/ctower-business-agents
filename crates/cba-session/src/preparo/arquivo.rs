//! Leitura e gravação atômica dos arquivos de configuração do Claude.
//!
//! - O arquivo é lido inteiro, mas o conteúdo só é interpretado como JSON genérico: as
//!   chaves que não são nossas (inclusive dados de conta) passam intactas, sem serem
//!   examinadas, copiadas para outro lugar ou registradas no log.
//! - Antes de substituir, a versão anterior é copiada para `<arquivo>.cba-backup`, na
//!   mesma pasta e com as mesmas permissões: a cópia não leva o conteúdo para nenhum
//!   lugar novo.
//! - A gravação é atômica (arquivo temporário na mesma pasta + rename) e confere, logo
//!   antes do rename, que o arquivo não mudou desde a leitura. O Claude Code regrava o
//!   `.claude.json` com frequência; se ele mudou no meio, quem chama repete a edição.
//! - Se o caminho for um link simbólico (configuração gerida por dotfiles), a gravação
//!   vai para o destino do link, que continua sendo link.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use super::Erro;
use super::json::{Json, Objeto};

/// Sufixo da cópia de segurança, ao lado do arquivo original.
pub const SUFIXO_BACKUP: &str = ".cba-backup";

/// Arquivo lido: bytes originais (para conferir antes de gravar) e o JSON.
#[derive(Debug)]
pub struct Lido {
    /// Caminho real (destino do link simbólico, se for o caso).
    pub caminho: PathBuf,
    /// `None` se o arquivo não existia.
    pub original: Option<Vec<u8>>,
    pub raiz: Objeto,
}

fn erro_io(caminho: &Path, e: io::Error) -> Erro {
    Erro::Arquivo {
        arquivo: caminho.to_path_buf(),
        tipo: e.kind(),
    }
}

/// Destino real do caminho, seguindo links simbólicos.
fn real(caminho: &Path) -> PathBuf {
    match fs::symlink_metadata(caminho) {
        Ok(m) if m.file_type().is_symlink() => {
            dunce::canonicalize(caminho).unwrap_or_else(|_| caminho.to_path_buf())
        }
        _ => caminho.to_path_buf(),
    }
}

/// Lê um arquivo de configuração. Arquivo ausente ou vazio vira objeto vazio; JSON
/// inválido ou raiz que não é objeto é erro (e o arquivo nunca é sobrescrito).
pub fn ler(caminho: &Path) -> Result<Lido, Erro> {
    let caminho = real(caminho);
    let original = match fs::read(&caminho) {
        Ok(b) => Some(b),
        Err(e) if e.kind() == io::ErrorKind::NotFound => None,
        Err(e) => return Err(erro_io(&caminho, e)),
    };
    let raiz = match &original {
        None => Objeto::default(),
        Some(b) if b.iter().all(u8::is_ascii_whitespace) => Objeto::default(),
        Some(b) => {
            let texto = b.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(b);
            match serde_json::from_slice::<Json>(texto) {
                Ok(Json::Objeto(o)) => o,
                Ok(_) => {
                    return Err(Erro::Formato {
                        arquivo: caminho.clone(),
                        campo: "(raiz)".into(),
                    });
                }
                // Só a linha: a mensagem do serde poderia citar trechos do arquivo.
                Err(e) => {
                    return Err(Erro::JsonInvalido {
                        arquivo: caminho.clone(),
                        linha: e.line(),
                    });
                }
            }
        }
    };
    Ok(Lido {
        caminho,
        original,
        raiz,
    })
}

/// Grava `raiz` no lugar de `lido`, com cópia de segurança e conferência de
/// concorrência. Devolve o caminho da cópia de segurança, se havia arquivo antes.
pub fn gravar(lido: &Lido, raiz: &Objeto) -> Result<Option<PathBuf>, Erro> {
    gravar_com_backup_em(lido, raiz, None)
}

/// Cópia de segurança padrão: `<arquivo>.cba-backup`, ao lado do arquivo.
fn backup_ao_lado(caminho: &Path) -> PathBuf {
    let mut b = caminho.to_path_buf().into_os_string();
    b.push(SUFIXO_BACKUP);
    PathBuf::from(b)
}

/// Como [`gravar`], com a cópia de segurança em `backup` (em vez de ao lado do
/// arquivo). Para os arquivos dentro das pastas dos projetos (#304): a cópia não
/// pode ficar no repositório.
pub fn gravar_com_backup_em(
    lido: &Lido,
    raiz: &Objeto,
    backup: Option<&Path>,
) -> Result<Option<PathBuf>, Erro> {
    let caminho = &lido.caminho;
    let pasta = caminho
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(pasta).map_err(|e| erro_io(pasta, e))?;

    let original = lido.original.as_deref();
    let mut texto = Json::Objeto(raiz.clone()).para_texto_com(&recuo(original));
    // Mantém o final do arquivo como estava (com ou sem quebra de linha).
    if original.is_none_or(|b| b.ends_with(b"\n")) {
        texto.push('\n');
    }
    // E as quebras de linha do Windows, se era assim.
    if original.is_some_and(|b| b.windows(2).any(|w| w == b"\r\n")) {
        texto = texto.replace('\n', "\r\n");
    }

    let nome = caminho
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "config".into());
    let temporario = pasta.join(format!(".{nome}.cba-{}.tmp", std::process::id()));
    let resultado = (|| {
        let mut f = fs::File::create(&temporario).map_err(|e| erro_io(&temporario, e))?;
        f.write_all(texto.as_bytes())
            .and_then(|_| f.sync_all())
            .map_err(|e| erro_io(&temporario, e))?;
        drop(f);
        // Mesmas permissões do original (o `.claude.json` costuma ser 0600).
        match fs::metadata(caminho) {
            Ok(m) => fs::set_permissions(&temporario, m.permissions())
                .map_err(|e| erro_io(&temporario, e))?,
            Err(_) => restringir(&temporario)?,
        }

        // Conferência de concorrência: o arquivo ainda é o que foi lido?
        let atual = match fs::read(caminho) {
            Ok(b) => Some(b),
            Err(e) if e.kind() == io::ErrorKind::NotFound => None,
            Err(e) => return Err(erro_io(caminho, e)),
        };
        if atual != lido.original {
            return Err(Erro::AlteradoDuranteGravacao(caminho.clone()));
        }

        let backup = match &lido.original {
            Some(_) => Some(copiar_backup(caminho, backup)?),
            None => None,
        };
        renomear(&temporario, caminho)?;
        Ok(backup)
    })();
    if resultado.is_err() {
        let _ = fs::remove_file(&temporario);
    }
    resultado
}

/// Copia `caminho` para a cópia de segurança (`backup`, ou ao lado do arquivo).
fn copiar_backup(caminho: &Path, backup: Option<&Path>) -> Result<PathBuf, Erro> {
    let b = backup
        .map(Path::to_path_buf)
        .unwrap_or_else(|| backup_ao_lado(caminho));
    if let Some(pasta) = b.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(pasta).map_err(|e| erro_io(pasta, e))?;
    }
    fs::copy(caminho, &b).map_err(|e| erro_io(&b, e))?;
    Ok(b)
}

/// Apaga o arquivo lido, com cópia de segurança em `backup` (ou ao lado) e a mesma
/// conferência de concorrência de [`gravar`]. Arquivo que já não existe não é erro.
pub fn apagar(lido: &Lido, backup: Option<&Path>) -> Result<Option<PathBuf>, Erro> {
    let caminho = &lido.caminho;
    let atual = match fs::read(caminho) {
        Ok(b) => Some(b),
        Err(e) if e.kind() == io::ErrorKind::NotFound => None,
        Err(e) => return Err(erro_io(caminho, e)),
    };
    if atual != lido.original {
        return Err(Erro::AlteradoDuranteGravacao(caminho.clone()));
    }
    if atual.is_none() {
        return Ok(None);
    }
    let b = copiar_backup(caminho, backup)?;
    fs::remove_file(caminho).map_err(|e| erro_io(caminho, e))?;
    Ok(Some(b))
}

/// Indentação do arquivo: o espaço em branco no começo da primeira linha recuada
/// (dois espaços, como o Claude Code grava, se não houver nenhuma).
fn recuo(original: Option<&[u8]>) -> Vec<u8> {
    let padrao = b"  ".to_vec();
    let Some(b) = original else { return padrao };
    b.split(|c| *c == b'\n')
        .skip(1)
        .map(|linha| {
            linha
                .iter()
                .take_while(|c| **c == b' ' || **c == b'\t')
                .copied()
                .collect::<Vec<u8>>()
        })
        .find(|r| !r.is_empty())
        .unwrap_or(padrao)
}

/// Arquivo novo: só o dono lê e grava (Unix). O `.claude.json` guarda dados da conta.
fn restringir(caminho: &Path) -> Result<(), Erro> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(caminho, fs::Permissions::from_mode(0o600))
            .map_err(|e| erro_io(caminho, e))?;
    }
    let _ = caminho;
    Ok(())
}

/// `rename` com novas tentativas: no Windows, um arquivo aberto por outro processo
/// (o próprio Claude, um antivírus) recusa a troca por alguns instantes.
fn renomear(de: &Path, para: &Path) -> Result<(), Erro> {
    let mut tentativa = 0;
    loop {
        match fs::rename(de, para) {
            Ok(()) => return Ok(()),
            Err(e) if e.kind() == io::ErrorKind::PermissionDenied && tentativa < 10 => {
                tentativa += 1;
                thread::sleep(Duration::from_millis(50 * tentativa));
            }
            Err(e) => return Err(erro_io(para, e)),
        }
    }
}
