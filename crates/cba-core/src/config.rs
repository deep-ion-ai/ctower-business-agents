//! Configuração do app, compartilhada pelos modos do executável.
//!
//! Fica em `<dados>/config/app.json`. Por enquanto guarda só a pasta da base de
//! conhecimento, escolhida pelo analista no primeiro uso (spec, seção 4.1, passo 3).
//! Campos desconhecidos são ignorados na leitura, para uma versão mais nova do app
//! poder acrescentar campos sem quebrar a mais antiga.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::dados::DiretorioDados;

const ARQUIVO: &str = "app.json";

/// Configuração do app.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    /// Pasta da base de conhecimento (caminho absoluto, desta máquina).
    #[serde(default)]
    pub base: Option<PathBuf>,
}

impl Config {
    /// Arquivo da configuração.
    pub fn arquivo(dados: &DiretorioDados) -> PathBuf {
        dados.config().join(ARQUIVO)
    }

    /// Lê a configuração; sem arquivo, devolve a padrão.
    pub fn ler(dados: &DiretorioDados) -> io::Result<Self> {
        match fs::read(Self::arquivo(dados)) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e),
        }
    }

    /// Grava a configuração (atômica).
    pub fn gravar(&self, dados: &DiretorioDados) -> io::Result<()> {
        let json = serde_json::to_vec_pretty(self).map_err(io::Error::other)?;
        cba_kb::arquivo::gravar(&Self::arquivo(dados), &json)
    }

    /// Pasta da base, se configurada.
    pub fn base(&self) -> Option<&Path> {
        self.base.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sem_arquivo_e_padrao_e_ida_e_volta() {
        let dir = tempfile::tempdir().unwrap();
        let dados = DiretorioDados::em(dir.path());
        assert_eq!(Config::ler(&dados).unwrap(), Config::default());
        let c = Config {
            base: Some(dir.path().join("base")),
        };
        c.gravar(&dados).unwrap();
        assert_eq!(Config::ler(&dados).unwrap(), c);
    }

    #[test]
    fn campos_desconhecidos_sao_ignorados() {
        let dir = tempfile::tempdir().unwrap();
        let dados = DiretorioDados::em(dir.path());
        fs::create_dir_all(dados.config()).unwrap();
        fs::write(
            Config::arquivo(&dados),
            r#"{"base": "x", "tema": "escuro"}"#,
        )
        .unwrap();
        assert_eq!(Config::ler(&dados).unwrap().base(), Some(Path::new("x")));
    }

    #[test]
    fn arquivo_invalido_e_erro() {
        let dir = tempfile::tempdir().unwrap();
        let dados = DiretorioDados::em(dir.path());
        fs::create_dir_all(dados.config()).unwrap();
        fs::write(Config::arquivo(&dados), "{").unwrap();
        assert_eq!(
            Config::ler(&dados).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
    }
}
