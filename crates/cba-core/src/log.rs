//! Log em arquivo dos processos do app.
//!
//! Cada modo do executável (`app`, `mcp`, `hook`) escreve no próprio arquivo, com
//! rotação diária, em `<dados>/logs/<modo>.AAAA-MM-DD.log`, guardando os últimos
//! [`DIAS_GUARDADOS`] arquivos. Nada vai para a saída padrão: no modo `mcp` ela é o
//! canal do protocolo e no modo `hook` é a resposta ao cliente de IA.
//!
//! Regra de privacidade (spec, seção 9): o log registra eventos (qual, quando, quanto
//! tempo), nunca conteúdo do sistema analisado, como o JSON de um hook, um prompt ou
//! os argumentos de uma ferramenta.

use std::io;
use std::path::Path;

use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::EnvFilter;

/// Quantos arquivos de log (dias) cada modo guarda.
pub const DIAS_GUARDADOS: usize = 14;

/// Variável de ambiente com o filtro de nível, na sintaxe do `RUST_LOG`. Padrão: `info`.
pub const VAR_FILTRO: &str = "CBA_LOG";

/// Liga o log do processo em `<dir>/<modo>.AAAA-MM-DD.log`.
///
/// Chame uma vez, no início do processo. Falha se o diretório não puder ser criado
/// ou se já houver um subscriber global.
pub fn iniciar(dir: &Path, modo: &str) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let arquivo = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix(modo)
        .filename_suffix("log")
        .max_log_files(DIAS_GUARDADOS)
        .build(dir)
        .map_err(io::Error::other)?;
    let filtro = EnvFilter::try_from_env(VAR_FILTRO).unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filtro)
        .with_writer(arquivo)
        .with_ansi(false)
        .with_target(false)
        .try_init()
        .map_err(io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grava_no_arquivo_do_modo() {
        let dir = tempfile::tempdir().unwrap();
        iniciar(dir.path(), "teste").unwrap();
        tracing::info!(evento = "SessionStart", "hook recebido");
        let arquivos: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(arquivos.len(), 1, "{arquivos:?}");
        assert!(arquivos[0].starts_with("teste.") && arquivos[0].ends_with(".log"));
        let texto = std::fs::read_to_string(dir.path().join(&arquivos[0])).unwrap();
        assert!(texto.contains("hook recebido") && texto.contains("SessionStart"));
    }
}
