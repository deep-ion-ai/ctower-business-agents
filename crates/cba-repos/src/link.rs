//! Links para a forja: arquivo, linhas e commit de uma evidência.

use crate::codigo::{RepositorioCodigo, repositorio_do_arquivo};
use std::path::Path;

/// Link da evidência no commit: `arquivo` (relativo ao projeto), `inicio..=fim`
/// (1-indexadas). O repositório é o que contém o arquivo (`absoluto`, pela pasta
/// de cada repositório) ou, no cadastro antigo sem pasta, o único repositório.
/// `None` se nenhum repositório serve ou falta dado (caminho ou commit).
pub fn link_evidencia(
    repos: &[RepositorioCodigo],
    absoluto: Option<&Path>,
    commit: &str,
    arquivo: &str,
    linhas: Option<(u32, u32)>,
) -> Option<String> {
    let (repo, no_repo) = repositorio_do_arquivo(repos, absoluto, arquivo)?;
    repo.link(commit, &no_repo, linhas)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ForgeKind;

    #[test]
    fn cada_arquivo_no_seu_repositorio() {
        let tmp = tempfile::tempdir().unwrap();
        let repos = vec![
            RepositorioCodigo {
                caminho: "o/web".into(),
                pasta: Some(tmp.path().join("web")),
                ..Default::default()
            },
            RepositorioCodigo {
                forja: ForgeKind::Gitlab,
                host: "gitlab.example.com".into(),
                caminho: "g/p/legado".into(),
                pasta: Some(tmp.path().join("legado")),
                ..Default::default()
            },
        ];
        assert_eq!(
            link_evidencia(
                &repos,
                Some(&tmp.path().join("web/src/a b.ts")),
                "abc",
                "web/src/a b.ts",
                Some((3, 9))
            )
            .unwrap(),
            "https://github.com/o/web/blob/abc/src/a%20b.ts#L3-L9"
        );
        assert_eq!(
            link_evidencia(
                &repos,
                Some(&tmp.path().join("legado/db/01.sql")),
                "abc",
                "legado/db/01.sql",
                Some((1, 4))
            )
            .unwrap(),
            "https://gitlab.example.com/g/p/legado/-/blob/abc/db/01.sql#L1-4"
        );
        assert!(
            link_evidencia(&repos, Some(&tmp.path().join("x/a")), "abc", "x/a", None).is_none()
        );
        assert!(link_evidencia(&[], None, "abc", "a", None).is_none());
    }
}
