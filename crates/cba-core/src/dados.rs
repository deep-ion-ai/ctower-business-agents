//! Diretório de dados do app: índice, configuração, cadastro dos projetos, eventos,
//! sessões e logs.
//!
//! Fica no diretório de dados locais do usuário, numa pasta com o identificador do
//! app, separado da pasta da base de conhecimento (os Markdown, numa pasta escolhida
//! pelo analista). Apagar este diretório não perde conhecimento: o índice é
//! reconstruível a partir da base (spec, seção 11).
//!
//! | Projeto | Raiz |
//! |---|---|
//! | Windows | `%LOCALAPPDATA%\ai.deep-ion.ctower-business-agents` |
//! | macOS | `~/Library/Application Support/ai.deep-ion.ctower-business-agents` |
//! | Linux | `$XDG_DATA_HOME/ai.deep-ion.ctower-business-agents` (padrão `~/.local/share`) |
//!
//! No Windows é a mesma pasta que o desinstalador do Tauri apaga quando o usuário
//! marca "apagar os dados do app". A variável de ambiente `CBA_DATA_DIR` troca a
//! raiz (testes e diagnóstico).

use std::io;
use std::path::{Path, PathBuf};

use crate::IDENTIFICADOR;

/// Variável de ambiente que troca a raiz do diretório de dados.
pub const VAR_DIRETORIO: &str = "CBA_DATA_DIR";

/// Caminhos do diretório de dados do app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiretorioDados {
    raiz: PathBuf,
}

impl DiretorioDados {
    /// Diretório com a raiz informada.
    pub fn em(raiz: impl Into<PathBuf>) -> Self {
        Self { raiz: raiz.into() }
    }

    /// Diretório padrão do sistema operacional, ou o de `CBA_DATA_DIR` se a variável existir.
    pub fn padrao() -> io::Result<Self> {
        if let Some(raiz) = std::env::var_os(VAR_DIRETORIO).filter(|v| !v.is_empty()) {
            return Ok(Self::em(raiz));
        }
        let base = dirs::data_local_dir().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "diretório de dados do usuário não encontrado",
            )
        })?;
        Ok(Self::em(base.join(IDENTIFICADOR)))
    }

    pub fn raiz(&self) -> &Path {
        &self.raiz
    }

    /// Pasta do índice SQLite (reconstruível a partir da base de conhecimento).
    pub fn indice(&self) -> PathBuf {
        self.raiz.join("indice")
    }

    /// Arquivo do índice da base de conhecimento ([`cba_kb::indice::Indice`]),
    /// dentro de [`DiretorioDados::indice`]. Os arquivos `-wal` e `-shm` do SQLite
    /// ficam ao lado.
    pub fn arquivo_do_indice(&self) -> PathBuf {
        self.indice().join(cba_kb::indice::ARQUIVO)
    }

    /// Configuração do app.
    pub fn config(&self) -> PathBuf {
        self.raiz.join("config")
    }

    /// Logs dos processos do app ([`crate::log`]).
    pub fn logs(&self) -> PathBuf {
        self.raiz.join("logs")
    }

    /// Cadastro dos projetos (`cba_index::Cadastro`).
    pub fn projetos(&self) -> PathBuf {
        self.raiz.join("projetos")
    }

    /// Diário de mudanças das análises, que o app acompanha ([`crate::eventos`]).
    pub fn eventos(&self) -> PathBuf {
        self.raiz.join("eventos")
    }

    /// Estado das sessões dos clientes de IA, gravado pelo receptor de hooks
    /// (`cba_session::sessoes`).
    pub fn sessoes(&self) -> PathBuf {
        self.raiz.join("sessoes")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subpastas_ficam_dentro_da_raiz() {
        let raiz = Path::new("dados");
        let d = DiretorioDados::em(raiz);
        assert_eq!(d.raiz(), raiz);
        assert_eq!(d.indice(), raiz.join("indice"));
        assert_eq!(
            d.arquivo_do_indice(),
            raiz.join("indice").join("base.sqlite")
        );
        assert_eq!(d.config(), raiz.join("config"));
        assert_eq!(d.logs(), raiz.join("logs"));
        assert_eq!(d.projetos(), raiz.join("projetos"));
        assert_eq!(d.eventos(), raiz.join("eventos"));
        assert_eq!(d.sessoes(), raiz.join("sessoes"));
    }

    #[test]
    fn padrao_usa_o_identificador_do_app() {
        // Só vale sem a variável de ambiente no processo de teste.
        if std::env::var_os(VAR_DIRETORIO).is_some() {
            return;
        }
        let d = DiretorioDados::padrao().expect("diretório de dados");
        assert!(d.raiz().ends_with(IDENTIFICADOR), "{:?}", d.raiz());
    }
}
