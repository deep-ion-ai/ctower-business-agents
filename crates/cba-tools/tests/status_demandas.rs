//! Conferência do status das demandas com o rastreador (#292), com CLIs falsas
//! (`gh`, `glab`, `jira`): a issue fechada encerra a demanda com a data, reabrir
//! volta a demanda para aberta, sem rede fica o último estado com o aviso, o
//! cache evita reler o rastreador e a demanda sem chave mantém o status local.
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use cba_kb::frontmatter::{
    Data, EstadoRastreador, FrontmatterProjeto, StatusDemanda, TipoDocumento,
};
use cba_kb::{Base, Documento, IdDemanda, NovaDemanda, Projeto};
use cba_repos::demanda::Ferramentas;
use cba_repos::{GithubConfig, GitlabConfig, JiraConfig, TrackerConfig};
use cba_tools::status_demandas::{CacheConferencia, conferir_demandas};

fn base() -> (tempfile::TempDir, Projeto) {
    let dir = tempfile::tempdir().unwrap();
    let base = Base::criar(dir.path().join("base")).unwrap();
    let projeto = base
        .criar_projeto(&Documento::novo(
            FrontmatterProjeto {
                tipo: TipoDocumento::Projeto,
                id: "faturamento".into(),
                nome: "Faturamento".into(),
                areas: vec![],
                criado: Data::nova(2026, 11, 1),
            },
            "\n# Faturamento\n",
        ))
        .unwrap();
    (dir, projeto)
}

fn criar(kb: &Projeto, titulo: &str, chave: Option<&str>, origem: Option<&str>) -> IdDemanda {
    kb.criar_demanda(
        &NovaDemanda {
            titulo: titulo.into(),
            chave: chave.map(Into::into),
            origem: origem.map(Into::into),
            ..Default::default()
        },
        Data::nova(2026, 11, 2).unwrap(),
    )
    .unwrap()
    .id
}

fn script(dir: &Path, nome: &str, corpo: &str) -> PathBuf {
    let p = dir.join(nome);
    std::fs::write(&p, format!("#!/bin/sh\n{corpo}\n")).unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    p
}

/// `gh` que responde o conteúdo de `resposta` (para o teste trocar o estado) e
/// conta as chamadas em `chamadas`.
fn gh_falso(dir: &Path) -> PathBuf {
    let r = dir.join("resposta");
    let n = dir.join("chamadas");
    script(
        dir,
        "gh",
        &format!(
            r#"echo x >> "{}"
[ -f "{}" ] || {{ echo "error connecting to api.github.com" >&2; exit 1; }}
cat "{}""#,
            n.display(),
            r.display(),
            r.display()
        ),
    )
}

fn responder(dir: &Path, json: &str) {
    std::fs::write(dir.join("resposta"), json).unwrap();
}

fn chamadas(dir: &Path) -> usize {
    std::fs::read_to_string(dir.join("chamadas"))
        .map(|s| s.lines().count())
        .unwrap_or(0)
}

#[test]
fn github_encerra_reabre_e_sem_rede_fica_o_ultimo_estado() {
    let (_dir, kb) = base();
    let fechada = criar(
        &kb,
        "Parcelamento",
        Some("#5"),
        Some("https://github.com/o/r/issues/5"),
    );
    let aberta = criar(&kb, "Botão", Some("#7"), None);
    let manual = criar(&kb, "Pedido por e-mail", None, None);
    let outro_repo = criar(
        &kb,
        "De outro repositório",
        Some("#5"),
        Some("https://github.com/o/outro/issues/5"),
    );
    let ferramentas = tempfile::tempdir().unwrap();
    let fdir = ferramentas.path();
    let f = Ferramentas {
        gh: gh_falso(fdir),
        ..Default::default()
    };
    let tracker = TrackerConfig::Github(GithubConfig {
        repo: "o/r".into(),
        ..Default::default()
    });
    let cache = CacheConferencia::default();
    responder(
        fdir,
        r#"{"data":{"repository":{"i5":{"state":"CLOSED","closedAt":"2026-11-20T12:00:00Z"},"i7":{"state":"OPEN","closedAt":null}}}}"#,
    );

    let s = conferir_demandas(&kb, Some(&tracker), None, false, &cache, &f);
    assert_eq!(s.len(), 2, "só as demandas deste rastreador: {s:?}");
    assert!(
        s.values()
            .all(|x| x.aviso.is_none() && x.conferido.is_some())
    );
    let d = kb.demanda(fechada).unwrap();
    assert!(!d.aberta());
    assert_eq!(d.status_efetivo(), StatusDemanda::Encerrada);
    assert_eq!(d.encerrado_efetivo(), Data::nova(2026, 11, 20));
    // O status local do analista não muda.
    assert_eq!(d.documento.frontmatter.status, StatusDemanda::Aberta);
    assert_eq!(d.documento.frontmatter.encerrado, None);
    assert!(kb.demanda(aberta).unwrap().aberta());
    assert!(
        kb.demanda(manual)
            .unwrap()
            .documento
            .frontmatter
            .rastreador
            .is_none()
    );
    assert!(
        kb.demanda(outro_repo)
            .unwrap()
            .documento
            .frontmatter
            .rastreador
            .is_none()
    );
    // As abertas primeiro: a encerrada pelo rastreador vai para o fim.
    let ordem: Vec<_> = kb.demandas().unwrap().iter().map(|d| d.id).collect();
    assert_eq!(ordem.last(), Some(&fechada));
    assert_eq!(chamadas(fdir), 1);

    // No cache: não relê o rastreador nem regrava a demanda.
    let texto = std::fs::read_to_string(kb.caminho(&kb.demanda(fechada).unwrap().arquivo)).unwrap();
    conferir_demandas(&kb, Some(&tracker), None, false, &cache, &f);
    assert_eq!(chamadas(fdir), 1);

    // Forçada ("Atualizar status"), mesmo estado no mesmo dia: relê, mas não regrava.
    conferir_demandas(&kb, Some(&tracker), None, true, &cache, &f);
    assert_eq!(chamadas(fdir), 2);
    let depois =
        std::fs::read_to_string(kb.caminho(&kb.demanda(fechada).unwrap().arquivo)).unwrap();
    assert_eq!(texto, depois);

    // Reaberta no GitHub: volta a demanda para aberta, sem data de encerramento.
    responder(
        fdir,
        r#"{"data":{"repository":{"i5":{"state":"OPEN","closedAt":null},"i7":{"state":"OPEN","closedAt":null}}}}"#,
    );
    conferir_demandas(&kb, Some(&tracker), Some(&[fechada]), true, &cache, &f);
    let d = kb.demanda(fechada).unwrap();
    assert!(d.aberta());
    assert_eq!(d.encerrado_efetivo(), None);

    // Sem rede: fica o último estado conhecido, com o aviso.
    std::fs::remove_file(fdir.join("resposta")).unwrap();
    let s = conferir_demandas(&kb, Some(&tracker), None, true, &cache, &f);
    let aviso = s[&fechada].aviso.as_deref().unwrap();
    assert!(
        aviso.contains("inacessível") && aviso.contains("error connecting"),
        "{aviso}"
    );
    assert!(
        s[&fechada].conferido.is_some(),
        "guarda a última conferência boa"
    );
    assert!(kb.demanda(fechada).unwrap().aberta());

    // Sem rastreador cadastrado, nada é conferido.
    assert!(conferir_demandas(&kb, None, None, true, &cache, &f).is_empty());
}

#[test]
fn cache_vencido_rele_o_rastreador() {
    let (_dir, kb) = base();
    criar(&kb, "Parcelamento", Some("#5"), None);
    let ferramentas = tempfile::tempdir().unwrap();
    let fdir = ferramentas.path();
    let f = Ferramentas {
        gh: gh_falso(fdir),
        ..Default::default()
    };
    responder(fdir, r#"{"data":{"repository":{"i5":{"state":"OPEN"}}}}"#);
    let tracker = TrackerConfig::Github(GithubConfig {
        repo: "o/r".into(),
        ..Default::default()
    });
    let cache = CacheConferencia::com_validade(Duration::ZERO);
    conferir_demandas(&kb, Some(&tracker), None, false, &cache, &f);
    conferir_demandas(&kb, Some(&tracker), None, false, &cache, &f);
    assert_eq!(chamadas(fdir), 2);
}

#[test]
fn issue_nao_encontrada_avisa_e_mantem_o_estado() {
    let (_dir, kb) = base();
    let id = criar(&kb, "Apagada", Some("#9"), None);
    let ferramentas = tempfile::tempdir().unwrap();
    let fdir = ferramentas.path();
    let f = Ferramentas {
        gh: gh_falso(fdir),
        ..Default::default()
    };
    responder(fdir, r#"{"data":{"repository":{"i9":null}}}"#);
    let tracker = TrackerConfig::Github(GithubConfig {
        repo: "o/r".into(),
        ..Default::default()
    });
    let s = conferir_demandas(
        &kb,
        Some(&tracker),
        None,
        false,
        &CacheConferencia::default(),
        &f,
    );
    assert!(
        s[&id]
            .aviso
            .as_deref()
            .unwrap()
            .contains("não foi encontrada")
    );
    assert!(
        kb.demanda(id)
            .unwrap()
            .documento
            .frontmatter
            .rastreador
            .is_none()
    );
    assert!(kb.demanda(id).unwrap().aberta());
}

#[test]
fn gitlab_fechada_encerra_com_a_data() {
    let (_dir, kb) = base();
    let id = criar(
        &kb,
        "Reajuste",
        Some("#3"),
        Some("https://gitlab.example.com/g/app/-/issues/3"),
    );
    let ferramentas = tempfile::tempdir().unwrap();
    let f = Ferramentas {
        glab: script(
            ferramentas.path(),
            "glab",
            r#"[ "$GITLAB_TOKEN" = "tok" ] || exit 1
echo '[{"iid":3,"state":"closed","closed_at":"2026-10-01T08:00:00.000Z"}]'"#,
        ),
        ..Default::default()
    };
    let tracker = TrackerConfig::Gitlab(GitlabConfig {
        host: "gitlab.example.com".into(),
        path: "g/app".into(),
        credential_cmd: "echo tok".into(),
        ..Default::default()
    });
    conferir_demandas(
        &kb,
        Some(&tracker),
        None,
        false,
        &CacheConferencia::default(),
        &f,
    );
    let d = kb.demanda(id).unwrap();
    assert!(!d.aberta());
    assert_eq!(d.encerrado_efetivo(), Data::nova(2026, 10, 1));
}

#[test]
fn jira_pela_categoria_do_status() {
    let (_dir, kb) = base();
    let feita = criar(&kb, "Nota fiscal", Some("PROJ-1"), None);
    let fazendo = criar(&kb, "Boleto", Some("proj-2"), None);
    let ferramentas = tempfile::tempdir().unwrap();
    let f = Ferramentas {
        jira: script(
            ferramentas.path(),
            "jira",
            r#"[ "$JIRA_API_TOKEN" = "tok" ] || exit 1
echo '[{"key":"PROJ-1","fields":{"status":{"name":"Homologado","statusCategory":{"key":"done"}},"resolutiondate":"2026-11-02T10:00:00.000-0300"}},{"key":"PROJ-2","fields":{"status":{"name":"Done","statusCategory":{"key":"indeterminate"}}}}]'"#,
        ),
        ..Default::default()
    };
    let tracker = TrackerConfig::Jira(JiraConfig {
        url: "https://jira.example.com".into(),
        project_key: "PROJ".into(),
        credential_cmd: "echo tok".into(),
        ..Default::default()
    });
    conferir_demandas(
        &kb,
        Some(&tracker),
        None,
        false,
        &CacheConferencia::default(),
        &f,
    );
    let d = kb.demanda(feita).unwrap();
    assert!(!d.aberta());
    assert_eq!(d.encerrado_efetivo(), Data::nova(2026, 11, 2));
    let d = kb.demanda(fazendo).unwrap();
    assert!(d.aberta());
    assert_eq!(
        d.documento
            .frontmatter
            .rastreador
            .as_ref()
            .map(|r| r.estado),
        Some(EstadoRastreador::EmAndamento)
    );
}
