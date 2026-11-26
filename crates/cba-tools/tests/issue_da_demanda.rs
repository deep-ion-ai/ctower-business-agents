//! `ler_issue_da_demanda` (#351) com o sistema fictício "Faturamento Exemplo" e
//! um `jira` falso: lê os comentários da issue da análise sem expor o token,
//! recusa a análise sem issue, recusa com motivo claro sem rede, e o roteiro
//! orienta a ler os comentários no início.
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use cba_core::ambiente::Ambiente;
use cba_core::analise::{Analise, Fluxo};
use cba_core::analises::NovaAnalise;
use cba_core::config::Config;
use cba_core::dados::DiretorioDados;
use cba_kb::frontmatter::Demanda;
use cba_repos::demanda::Ferramentas;
use cba_repos::{JiraConfig, TrackerConfig};
use cba_tools::issue_da_demanda::ler_issue_da_demanda_com;
use cba_tools::{ErroChamada, ErroFerramenta, Registro, Saida, registro_padrao};
use serde_json::json;

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/faturamento-exemplo")
}

fn script(dir: &Path, nome: &str, corpo: &str) -> PathBuf {
    let p = dir.join(nome);
    std::fs::write(&p, format!("#!/bin/sh\n{corpo}\n")).unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    p
}

fn ambiente(dir: &tempfile::TempDir) -> Ambiente {
    let raiz = dir.path();
    let dados = DiretorioDados::em(raiz.join("dados"));
    let mut projeto = cba_index::Projeto::novo("Faturamento Exemplo", [fixture()]).unwrap();
    projeto.repositorios.tracker = Some(TrackerConfig::Jira(JiraConfig {
        url: "https://jira.example.com".into(),
        project_key: "FAT".into(),
        credential_cmd: "echo tok-jira-secreto".into(),
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

fn criar(amb: &Ambiente, demanda: Option<Demanda>) -> Analise {
    amb.analises
        .criar_com_demanda(
            NovaAnalise {
                fluxo: Fluxo::Impacto,
                titulo: "Cancelar nota".into(),
                entrada: "Cancelar nota fiscal emitida".into(),
                analista: Some("Ana Exemplo".into()),
                base: None,
            },
            demanda,
        )
        .unwrap()
}

fn da_issue() -> Option<Demanda> {
    Some(Demanda {
        id: None,
        origem: "https://jira.example.com/browse/FAT-12".into(),
        chave: Some("FAT-12".into()),
    })
}

fn chamar(r: &Registro, amb: &Ambiente, analise: &str) -> Result<Saida, ErroFerramenta> {
    match r.chamar(
        "ler_issue_da_demanda",
        Ok(amb),
        Some(json!({ "analise": analise })),
    ) {
        Ok(s) => Ok(s),
        Err(ErroChamada::Ferramenta(e)) => Err(e),
        Err(e) => panic!("{e}"),
    }
}

fn registro(jira: PathBuf) -> Registro {
    let mut r = Registro::new();
    r.registrar(ler_issue_da_demanda_com(Ferramentas {
        jira,
        ..Default::default()
    }));
    r
}

#[test]
fn le_os_comentarios_da_issue_da_analise_sem_expor_o_token() {
    let dir = tempfile::tempdir().unwrap();
    let amb = ambiente(&dir);
    let a = criar(&amb, da_issue());
    let jira = script(
        dir.path(),
        "jira",
        r#"[ "$JIRA_API_TOKEN" = "tok-jira-secreto" ] || { echo "401 sem token" >&2; exit 1; }
[ "$3" = "FAT-12" ] || { echo "outra issue: $3" >&2; exit 2; }
cat <<'FIM'
{"key":"FAT-12","fields":{"summary":"Cancelar nota","description":"Cancelar *qualquer* nota",
"status":{"name":"Em análise"},"assignee":{"displayName":"Ana Exemplo"},"labels":["faturamento"],
"comment":{"total":1,"comments":[{"author":{"displayName":"Bruno Exemplo"},"created":"2026-11-02T10:00:00.000-0300","body":"Só notas de *serviço*, não de produto"}]}}}
FIM"#,
    );
    let s = chamar(&registro(jira), &amb, &a.id.to_string()).unwrap();
    assert!(
        s.texto.contains("Bruno Exemplo em 2026-11-02"),
        "{}",
        s.texto
    );
    assert!(s.texto.contains("Só notas de **serviço**"), "{}", s.texto);
    assert!(s.texto.contains("Situação: Em análise"), "{}", s.texto);
    let dados = s.dados.unwrap();
    assert_eq!(dados["comentarios"][0]["autor"], "Bruno Exemplo");
    assert_eq!(dados["chave"], "FAT-12");
    let tudo = format!("{}{dados}", s.texto);
    assert!(!tudo.contains("tok-jira-secreto"));
}

#[test]
fn sem_rede_recusa_com_o_motivo_e_sem_o_token() {
    let dir = tempfile::tempdir().unwrap();
    let amb = ambiente(&dir);
    let a = criar(&amb, da_issue());
    let jira = script(
        dir.path(),
        "jira",
        r#"echo "dial tcp: lookup jira.example.com: no such host $JIRA_API_TOKEN" >&2; exit 1"#,
    );
    let e = chamar(&registro(jira), &amb, &a.id.to_string()).unwrap_err();
    let ErroFerramenta::Recusa(m) = e else {
        panic!("{e:?}")
    };
    assert!(
        m.contains("no such host") && m.contains("Siga a análise"),
        "{m}"
    );
    assert!(!m.contains("tok-jira-secreto"), "{m}");
}

#[test]
fn analise_sem_issue_e_recusada_e_o_roteiro_so_orienta_com_issue() {
    let dir = tempfile::tempdir().unwrap();
    let amb = ambiente(&dir);
    let sem = criar(&amb, None);
    let e = chamar(
        &registro(dir.path().join("nao-chamado")),
        &amb,
        &sem.id.to_string(),
    )
    .unwrap_err();
    assert!(
        matches!(&e, ErroFerramenta::Recusa(m) if m.contains("não nasceu de uma issue")),
        "{e:?}"
    );

    let com = criar(&amb, da_issue());
    let roteiro = |id: &str| {
        registro_padrao()
            .chamar("retomar_analise", Ok(&amb), Some(json!({ "analise": id })))
            .unwrap()
            .texto
    };
    let r = roteiro(&com.id.to_string());
    assert!(r.contains("## Issue da demanda no rastreador"), "{r}");
    assert!(
        r.contains(&format!(
            "`ler_issue_da_demanda` com `analise: \"{}\"`",
            com.id
        )),
        "{r}"
    );
    assert!(!roteiro(&sem.id.to_string()).contains("ler_issue_da_demanda"));
}
