//! Utilidades compartilhadas pelos testes das fixtures.

use std::fs;
use std::path::{Path, PathBuf};

/// Pasta `fixtures/` na raiz do repositório.
pub fn pasta_fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
}

/// Todos os arquivos sob `pasta`, recursivamente, em ordem estável.
pub fn arquivos_recursivos(pasta: &Path) -> Vec<PathBuf> {
    let mut pendentes = vec![pasta.to_path_buf()];
    let mut arquivos = Vec::new();
    while let Some(atual) = pendentes.pop() {
        let entradas = fs::read_dir(&atual)
            .unwrap_or_else(|e| panic!("não foi possível listar {}: {e}", atual.display()));
        for entrada in entradas {
            let caminho = entrada.expect("entrada de diretório").path();
            if caminho.is_dir() {
                pendentes.push(caminho);
            } else {
                arquivos.push(caminho);
            }
        }
    }
    arquivos.sort();
    arquivos
}
