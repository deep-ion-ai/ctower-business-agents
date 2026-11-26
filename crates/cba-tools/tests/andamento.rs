//! `mudancas_em_andamento` (#441) com o projeto fictício "Faturamento Exemplo" e
//! um `gh` falso: o PR aberto que altera um arquivo da análise aparece com
//! número, título, branch, chaves e link; sem rede ou sem repositório cadastrado
//! a resposta traz o aviso e não falha; o token não aparece.
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use cba_core::ambiente::Ambiente;
use cba_core::analise::{Analise, Fluxo};
use cba_core::analises::NovaAnalise;
use cba_core::config::Config;
use cba_core::dados::DiretorioDados;
use cba_repos::andamento::Cache;
use cba_repos::demanda::Ferramentas;
use cba_repos::{JiraConfig, OrigemToken, RepositorioCodigo, TrackerConfig};
use cba_tools::andamento::mudancas_em_andamento_com;
use cba_tools::{Registro, Saida};
use serde_json::{Value, json};

const FATURA: &str = "backend/src/main/java/exemplo/faturamento/Fatura.java";

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/faturamento-exemplo")
}

fn script(dir: &Path, nome: &str, corpo: &str) -> PathBuf {
    let p = dir.join(nome);
    std::fs::write(&p, format!("#!/bin/sh\n{corpo}\n")).unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    p
}

fn ambiente(dir: &tempfile::TempDir, com_repositorio: bool) -> Ambiente {
    let raiz = dir.path();
    let dados = DiretorioDados::em(raiz.join("dados"));
    let mut projeto = cba_index::Projeto::novo("Faturamento Exemplo", [fixture()]).unwrap();
    if com_repositorio {
        projeto.repositorios.codigo = vec![RepositorioCodigo {
            caminho: "o/faturamento".into(),
            token: OrigemToken::Comando,
            comando_credencial: "echo tok-repo-secreto".into(),
            ..Default::default()
        }];
    }
    projeto.repositorios.tracker = Some(TrackerConfig::Jira(JiraConfig {
        url: "https://jira.example.com".into(),
        project_key: "FAT".into(),
        ..Default::default()
    }));
    cba_index::Cadastro::em(dados.projetos())
        .cadastrar(&projeto)
        .unwrap();
    cba_kb::Base::criar(raiz.join("base")).unwrap();
    Config {
        base: Some(raiz.join("base")),
        ..Default::default()
    }
    .gravar(&dados)
    .unwrap();
    Ambiente::do_diretorio(&fixture(), &dados).unwrap()
}

fn criar(amb: &Ambiente) -> Analise {
    amb.analises
        .criar(NovaAnalise {
            fluxo: Fluxo::Impacto,
            titulo: "Parcelar fatura".into(),
            entrada: "Parcelar a fatura em 3 vezes".into(),
            analista: None,
            base: None,
        })
        .unwrap()
}

fn chamar(gh: PathBuf, amb: &Ambiente, args: Value) -> Saida {
    let mut r = Registro::new();
    r.registrar(mudancas_em_andamento_com(
        Ferramentas {
            gh,
            ..Default::default()
        },
        Arc::new(Cache::default()),
    ));
    r.chamar("mudancas_em_andamento", Ok(amb), Some(args))
        .unwrap()
}

#[test]
fn pr_aberto_no_arquivo_da_analise_vem_com_numero_titulo_chaves_e_link() {
    let dir = tempfile::tempdir().unwrap();
    let amb = ambiente(&dir, true);
    let a = criar(&amb);
    let gh = script(
        dir.path(),
        "gh",
        &format!(
            r#"[ "$GH_TOKEN" = "tok-repo-secreto" ] || {{ echo "401 Bad credentials" >&2; exit 1; }}
case "$2" in
  repos/o/faturamento/pulls/7/files*) echo '[{{"filename":"{FATURA}"}},{{"filename":"README.md"}}]';;
  repos/o/faturamento/pulls/8/files*) echo '[{{"filename":"db/outro.sql"}}]';;
  repos/o/faturamento/pulls\?*) echo '[{{"number":7,"title":"FAT-31 juros da fatura","head":{{"ref":"feat/FAT-31-juros","sha":"a1"}},"user":{{"login":"bruno"}},"created_at":"2026-11-20T10:00:00Z","html_url":"https://github.com/o/faturamento/pull/7"}},{{"number":8,"title":"Outro","head":{{"ref":"x","sha":"b2"}},"user":{{"login":"ana"}},"created_at":"2026-11-21T10:00:00Z","html_url":"https://github.com/o/faturamento/pull/8"}}]';;
  *) echo "rota $2" >&2; exit 2;;
esac"#
        ),
    );
    let s = chamar(
        gh,
        &amb,
        json!({ "analise": a.id.to_string(), "arquivos": [FATURA, "db/nao-alterado.sql"] }),
    );
    let d = s.dados.unwrap();
    let m = &d["mudancas"];
    assert_eq!(m.as_array().unwrap().len(), 1, "{d}");
    assert_eq!(m[0]["numero"], "#7");
    assert_eq!(m[0]["titulo"], "FAT-31 juros da fatura");
    assert_eq!(m[0]["branch"], "feat/FAT-31-juros");
    assert_eq!(m[0]["autor"], "bruno");
    assert_eq!(m[0]["url"], "https://github.com/o/faturamento/pull/7");
    assert_eq!(m[0]["chaves"], json!(["FAT-31"]));
    assert_eq!(m[0]["arquivos"], json!([FATURA]));
    assert!(
        s.texto.contains("#7 \"FAT-31 juros da fatura\"") && s.texto.contains("`riscos`"),
        "{}",
        s.texto
    );
    assert!(!format!("{}{d}", s.texto).contains("tok-repo-secreto"));
}

#[test]
fn sem_rede_avisa_e_nao_falha() {
    let dir = tempfile::tempdir().unwrap();
    let amb = ambiente(&dir, true);
    let a = criar(&amb);
    let gh = script(
        dir.path(),
        "gh",
        r#"echo "error connecting to api.github.com ($GH_TOKEN)" >&2; exit 1"#,
    );
    let s = chamar(
        gh,
        &amb,
        json!({ "analise": a.id.to_string(), "arquivos": [FATURA] }),
    );
    let d = s.dados.unwrap();
    assert!(d["mudancas"].as_array().unwrap().is_empty());
    let aviso = d["avisos"][0].as_str().unwrap();
    assert!(
        aviso.contains("error connecting") && aviso.contains("siga a análise"),
        "{aviso}"
    );
    assert!(!format!("{}{d}", s.texto).contains("tok-repo-secreto"));
}

#[test]
fn sem_repositorio_ou_sem_arquivos_avisa() {
    let dir = tempfile::tempdir().unwrap();
    let amb = ambiente(&dir, false);
    let a = criar(&amb);
    let nao_chamado = dir.path().join("nao-existe");
    let s = chamar(
        nao_chamado.clone(),
        &amb,
        json!({ "analise": a.id.to_string(), "arquivos": [FATURA] }),
    );
    assert!(
        s.texto.contains("não tem repositório de código"),
        "{}",
        s.texto
    );
    let s = chamar(nao_chamado, &amb, json!({ "analise": a.id.to_string() }));
    assert!(s.texto.contains("ainda não tem evidências"), "{}", s.texto);
}
