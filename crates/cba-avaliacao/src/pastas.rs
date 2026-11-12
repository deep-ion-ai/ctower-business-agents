//! Cópia da fixture e conferência de que a sessão não alterou nenhum arquivo.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;

/// Pastas ignoradas na comparação (o repositório git criado na cópia).
const IGNORADAS: &[&str] = &[".git"];

/// Diferença entre a pasta original e a cópia em que o agente trabalhou.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "tipo", content = "arquivo", rename_all = "snake_case")]
pub enum Diferenca {
    Criado(String),
    Removido(String),
    Alterado(String),
}

/// Arquivos sob `raiz`, relativos e com `/`, menos as pastas ignoradas.
fn arquivos(raiz: &Path) -> io::Result<BTreeMap<String, PathBuf>> {
    let mut saida = BTreeMap::new();
    let mut pendentes = vec![raiz.to_path_buf()];
    while let Some(atual) = pendentes.pop() {
        for entrada in fs::read_dir(&atual)? {
            let entrada = entrada?;
            let caminho = entrada.path();
            let tipo = entrada.file_type()?;
            if tipo.is_dir() {
                if atual == raiz && IGNORADAS.iter().any(|i| entrada.file_name() == *i) {
                    continue;
                }
                pendentes.push(caminho);
            } else {
                let relativo = caminho
                    .strip_prefix(raiz)
                    .expect("dentro da raiz")
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join("/");
                saida.insert(relativo, caminho);
            }
        }
    }
    Ok(saida)
}

/// Compara as duas pastas byte a byte.
pub fn diferencas(original: &Path, copia: &Path) -> io::Result<Vec<Diferenca>> {
    let a = arquivos(original)?;
    let b = arquivos(copia)?;
    let mut d = Vec::new();
    for (rel, caminho) in &a {
        match b.get(rel) {
            None => d.push(Diferenca::Removido(rel.clone())),
            Some(outro) => {
                if fs::read(caminho)? != fs::read(outro)? {
                    d.push(Diferenca::Alterado(rel.clone()));
                }
            }
        }
    }
    for rel in b.keys() {
        if !a.contains_key(rel) {
            d.push(Diferenca::Criado(rel.clone()));
        }
    }
    Ok(d)
}

/// Copia `de` para `para` (criada), arquivo por arquivo.
pub fn copiar(de: &Path, para: &Path) -> io::Result<()> {
    for (rel, origem) in arquivos(de)? {
        let destino = rel
            .split('/')
            .fold(para.to_path_buf(), |acc, p| acc.join(p));
        if let Some(pai) = destino.parent() {
            fs::create_dir_all(pai)?;
        }
        fs::copy(origem, destino)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copia_igual_nao_tem_diferencas_e_cada_mudanca_aparece() {
        let dir = tempfile::tempdir().unwrap();
        let original = dir.path().join("original");
        fs::create_dir_all(original.join("db")).unwrap();
        fs::write(original.join("db/a.sql"), "select 1;\n").unwrap();
        fs::write(original.join("b.txt"), "b\n").unwrap();
        let copia = dir.path().join("copia");
        copiar(&original, &copia).unwrap();
        fs::create_dir_all(copia.join(".git")).unwrap();
        fs::write(copia.join(".git/HEAD"), "ref").unwrap();
        assert_eq!(diferencas(&original, &copia).unwrap(), vec![]);

        fs::write(copia.join("db/a.sql"), "select 2;\n").unwrap();
        fs::remove_file(copia.join("b.txt")).unwrap();
        fs::write(copia.join("novo.txt"), "x").unwrap();
        assert_eq!(
            diferencas(&original, &copia).unwrap(),
            vec![
                Diferenca::Removido("b.txt".into()),
                Diferenca::Alterado("db/a.sql".into()),
                Diferenca::Criado("novo.txt".into()),
            ]
        );
    }
}
