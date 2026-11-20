//! Testes dos comandos da publicação (#276) com o Faturamento Exemplo: a análise
//! da multa aprovada vai para um repositório próprio da documentação (bare local)
//! com o gh falso.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use cba_index::Cadastro;
use cba_repos::chaveiro::{Chaveiro, ChaveiroMemoria, conta_global};
use cba_repos::escrita::Situacao;
use cba_repos::{DestinoDocs, RepositorioCodigo, Secret};
use cba_tools::publicacao::TipoMudanca;

use super::*;
use crate::teste_base::{PROJETO, cenario};

const TOKEN: &str = "ghp-token-da-publicacao";

fn git(dir: &Path, args: &[&str]) -> String {
    let o = Command::new("git")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .args([
            "-c",
            "user.name=Teste",
            "-c",
            "user.email=teste@example.com",
            "-c",
            "init.defaultBranch=main",
        ])
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        o.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    String::from_utf8_lossy(&o.stdout).trim_end().to_string()
}

fn script(dir: &Path, nome: &str, corpo: &str) -> PathBuf {
    let p = dir.join(nome);
    fs::write(&p, format!("#!/bin/sh\n{corpo}\n")).unwrap();
    fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
    p
}

#[test]
fn previa_e_publicacao_da_analise_aprovada() {
    let c = cenario();
    c.aprovar_analise_da_multa();

    // Repositório da documentação (bare) e o clone do analista.
    let raiz = dunce::canonicalize(c.dir.path()).unwrap();
    let semente = raiz.join("docs-semente");
    fs::create_dir_all(&semente).unwrap();
    git(&semente, &["init", "-q"]);
    fs::write(semente.join("leia-me.md"), "# Faturamento Exemplo\n").unwrap();
    git(&semente, &["add", "-A"]);
    git(&semente, &["commit", "-q", "-m", "início"]);
    git(
        &raiz,
        &["clone", "-q", "--bare", "docs-semente", "docs.git"],
    );
    let url = format!("file://{}", raiz.join("docs.git").display());
    git(&raiz, &["clone", "-q", &url, "docs-clone"]);

    let forja = raiz.join("forja");
    fs::create_dir_all(&forja).unwrap();
    let bin = raiz.join("bin");
    fs::create_dir_all(&bin).unwrap();
    let chaveiro = ChaveiroMemoria::default();
    chaveiro
        .gravar(&conta_global("github.com"), &Secret::new(TOKEN))
        .unwrap();
    let f = Ferramentas {
        git: script(
            &bin,
            "git",
            "GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 exec git \"$@\"",
        ),
        gh: script(
            &bin,
            "gh",
            &format!(
                r#"[ "$GH_TOKEN" = "{TOKEN}" ] || exit 8
echo "$*" >> '{d}/chamadas'
case "$*" in
  *"--method POST"*) echo '{{"html_url":"https://github.com/exemplo/faturamento-docs/pull/7"}}';;
  *) echo '[]';;
esac"#,
                d = forja.display()
            ),
        ),
        chaveiro: Arc::new(chaveiro),
        ..Default::default()
    };

    let cadastro = Cadastro::em(c.dados.projetos());
    let mut p = cadastro.obter(PROJETO).unwrap().unwrap();
    p.repositorios.docs = Some(DestinoDocs::Repositorio {
        repositorio: RepositorioCodigo {
            caminho: "exemplo/faturamento-docs".into(),
            url_clone: url,
            pasta: Some(raiz.join("docs-clone")),
            ..Default::default()
        },
        subpasta: "negocio".into(),
    });
    cadastro.salvar(&p).unwrap();

    let pv = previa(&c.dados, PROJETO, &f).unwrap();
    assert_eq!(pv.branch, "docs/cba-faturamento-exemplo");
    assert_eq!(pv.base, "main");
    assert!(pv.independente && !pv.nada_a_publicar);
    assert!(pv.aprovados >= 3, "{pv:?}");
    assert!(
        pv.mudancas
            .iter()
            .all(|m| m.tipo == TipoMudanca::Novo && m.caminho.starts_with("negocio/")),
        "{pv:?}"
    );
    // A análise aprovada, a regra e o termo.
    let caminhos: Vec<&str> = pv.mudancas.iter().map(|m| m.caminho.as_str()).collect();
    assert!(caminhos.iter().any(|c| c.starts_with("negocio/analises/")));
    assert!(caminhos.iter().any(|c| c.starts_with("negocio/regras/")));
    assert!(caminhos.iter().any(|c| c.starts_with("negocio/glossario/")));
    assert!(
        !forja.join("chamadas").exists(),
        "a prévia não chama a forja"
    );

    let r = publicar(&c.dados, PROJETO, &f).unwrap();
    assert_eq!(r.publicado.situacao, Situacao::Aberto);
    assert_eq!(
        r.publicado.link.as_deref(),
        Some("https://github.com/exemplo/faturamento-docs/pull/7")
    );
    let arvore = git(
        &raiz.join("docs.git"),
        &[
            "ls-tree",
            "-r",
            "--name-only",
            "docs/cba-faturamento-exemplo",
        ],
    );
    assert!(arvore.contains("negocio/.cba-publicacao.json"), "{arvore}");
    assert!(arvore.contains("leia-me.md"), "{arvore}");
}
