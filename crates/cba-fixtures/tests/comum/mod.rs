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

/// Texto de um arquivo da fixture. O `.docx` (transcripts de reunião) é lido
/// pelo texto dos parágrafos, um por linha; os demais arquivos precisam ser
/// texto UTF-8.
#[allow(dead_code)] // cada arquivo de teste usa uma parte de `comum`
pub fn texto_do_arquivo(arquivo: &Path) -> String {
    if arquivo.extension().is_some_and(|e| e == "docx") {
        let bytes = fs::read(arquivo)
            .unwrap_or_else(|e| panic!("não foi possível ler {}: {e}", arquivo.display()));
        return cba_reunioes::paragrafos_docx(&bytes)
            .unwrap_or_else(|e| panic!("{} não é um .docx legível: {e}", arquivo.display()))
            .join("\n");
    }
    fs::read_to_string(arquivo)
        .unwrap_or_else(|e| panic!("{} não é texto UTF-8: {e}", arquivo.display()))
}
