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
    let caminho = &lido.caminho;
    let pasta = caminho
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(pasta).map_err(|e| erro_io(pasta, e))?;

    let mut texto = Json::Objeto(raiz.clone()).para_texto();
    // Mantém o final do arquivo como estava (com ou sem quebra de linha).
    if lido.original.as_ref().is_none_or(|b| b.ends_with(b"\n")) {
        texto.push('\n');
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
            Some(_) => {
                let mut b = caminho.clone().into_os_string();
                b.push(SUFIXO_BACKUP);
                let b = PathBuf::from(b);
                fs::copy(caminho, &b).map_err(|e| erro_io(&b, e))?;
                Some(b)
            }
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
