//! O que a detecção precisa do sistema operacional, atrás de um trait para os testes.
//!
//! O trait só permite perguntar se um caminho existe, listar os nomes de uma pasta e
//! executar um programa. Não há operação de leitura de arquivo: a detecção não abre
//! nenhum arquivo, nem de credencial nem de configuração (spec, seção 9; critério de
//! aceite 7).

use std::ffi::OsString;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// Sistema operacional em que a detecção roda.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Plataforma {
    Windows,
    MacOs,
    Linux,
}

impl Plataforma {
    /// A plataforma em que o programa foi compilado.
    pub fn atual() -> Self {
        if cfg!(windows) {
            Plataforma::Windows
        } else if cfg!(target_os = "macos") {
            Plataforma::MacOs
        } else {
            Plataforma::Linux
        }
    }

    /// Separador das pastas na variável `PATH`.
    pub fn separador_path(self) -> char {
        match self {
            Plataforma::Windows => ';',
            _ => ':',
        }
    }
}

/// Resultado de um programa executado até o fim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Saida {
    /// Código de saída; `None` se o processo terminou por sinal.
    pub codigo: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

/// Por que um programa não chegou ao fim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErroExecucao {
    /// Não foi possível iniciar o processo (tipo do erro de E/S).
    NaoIniciou(String),
    /// Passou do limite de tempo e foi encerrado.
    TempoEsgotado,
}

/// Acesso ao sistema usado pela detecção. [`AmbienteReal`] usa o sistema de verdade;
/// os testes usam uma implementação falsa que registra cada acesso.
pub trait Ambiente {
    fn plataforma(&self) -> Plataforma;
    /// Variável de ambiente.
    fn var(&self, nome: &str) -> Option<OsString>;
    /// Pasta do usuário.
    fn home(&self) -> Option<PathBuf>;
    /// O caminho existe e é um arquivo (só metadados; o arquivo não é aberto).
    fn e_arquivo(&self, caminho: &Path) -> bool;
    /// O caminho existe e é uma pasta.
    fn e_pasta(&self, caminho: &Path) -> bool;
    /// Nomes das entradas de uma pasta (vazio se não existir).
    fn nomes_na_pasta(&self, pasta: &Path) -> Vec<OsString>;
    /// Executa `programa` com `args`, sem entrada padrão, até `limite`.
    fn executar(
        &self,
        programa: &Path,
        args: &[&str],
        limite: Duration,
    ) -> Result<Saida, ErroExecucao>;
}

/// O sistema de verdade.
#[derive(Debug, Clone, Copy, Default)]
pub struct AmbienteReal;

impl Ambiente for AmbienteReal {
    fn plataforma(&self) -> Plataforma {
        Plataforma::atual()
    }

    fn var(&self, nome: &str) -> Option<OsString> {
        std::env::var_os(nome)
    }

    fn home(&self) -> Option<PathBuf> {
        dirs::home_dir()
    }

    fn e_arquivo(&self, caminho: &Path) -> bool {
        caminho.is_file()
    }

    fn e_pasta(&self, caminho: &Path) -> bool {
        caminho.is_dir()
    }

    fn nomes_na_pasta(&self, pasta: &Path) -> Vec<OsString> {
        match std::fs::read_dir(pasta) {
            Ok(entradas) => entradas
                .filter_map(|e| e.ok().map(|e| e.file_name()))
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    fn executar(
        &self,
        programa: &Path,
        args: &[&str],
        limite: Duration,
    ) -> Result<Saida, ErroExecucao> {
        let mut comando = Command::new(programa);
        comando
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // Um app aberto pelo Finder ou pelo menu Iniciar tem um PATH mínimo. A pasta do
        // próprio `claude` vai na frente, para uma instalação pelo npm achar o `node`
        // que costuma estar ao lado.
        if let Some(pasta) = programa.parent() {
            let mut pastas = vec![pasta.to_path_buf()];
            if let Some(path) = std::env::var_os("PATH") {
                pastas.extend(std::env::split_paths(&path));
            }
            if let Ok(path) = std::env::join_paths(pastas) {
                comando.env("PATH", path);
            }
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // CREATE_NO_WINDOW: o app (subsistema windows) não faz piscar um console.
            comando.creation_flags(0x0800_0000);
        }

        let mut filho = comando
            .spawn()
            .map_err(|e| ErroExecucao::NaoIniciou(format!("{:?}", e.kind())))?;
        let ler = |fluxo: Option<Box<dyn Read + Send>>| {
            thread::spawn(move || {
                let mut bytes = Vec::new();
                if let Some(mut fluxo) = fluxo {
                    let _ = fluxo.read_to_end(&mut bytes);
                }
                String::from_utf8_lossy(&bytes).into_owned()
            })
        };
        let stdout = ler(filho
            .stdout
            .take()
            .map(|s| Box::new(s) as Box<dyn Read + Send>));
        let stderr = ler(filho
            .stderr
            .take()
            .map(|s| Box::new(s) as Box<dyn Read + Send>));

        let inicio = Instant::now();
        let status = loop {
            match filho.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if inicio.elapsed() >= limite => {
                    let _ = filho.kill();
                    let _ = filho.wait();
                    return Err(ErroExecucao::TempoEsgotado);
                }
                Ok(None) => thread::sleep(Duration::from_millis(20)),
                Err(e) => return Err(ErroExecucao::NaoIniciou(format!("{:?}", e.kind()))),
            }
        };
        Ok(Saida {
            codigo: status.code(),
            stdout: stdout.join().unwrap_or_default(),
            stderr: stderr.join().unwrap_or_default(),
        })
    }
}
