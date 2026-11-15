//! Testes do cadastro de repositórios, da prontidão e da análise a partir de uma
//! issue (#86), com o jira-cli, o glab e o gh falsos.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use super::*;
use crate::analise::carregar_analise;
use crate::teste_base::{PROJETO, cenario};
use cba_repos::{ForgeConfig, GithubConfig, GitlabConfig, JiraConfig, TrackerConfig};

fn script(dir: &Path, nome: &str, corpo: &str) -> PathBuf {
    let p = dir.join(nome);
    fs::write(&p, format!("#!/bin/sh\n{corpo}\n")).unwrap();
    fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
    p
}

fn jira() -> JiraConfig {
    JiraConfig {
        url: "https://jira.example.com".into(),
        project_key: "PROJ".into(),
        credential_cmd: "echo tok-secreto".into(),
        ..Default::default()
    }
}

fn gitlab() -> GitlabConfig {
    GitlabConfig {
        host: "gitlab.example.com".into(),
        path: "grupo/projeto/app".into(),
        clone_url: "https://gitlab.example.com/grupo/projeto/app.git".into(),
        credential_cmd: "echo tok-gitlab".into(),
    }
}

#[test]
fn cadastra_codigo_no_github_e_issues_no_jira_e_codigo_no_gitlab_e_issues_no_jira() {
    let c = cenario();
    for forge in [
        ForgeConfig::Github(GithubConfig {
            repo: "deep-ion-ai/app".into(),
        }),
        ForgeConfig::Gitlab(gitlab()),
    ] {
        // Cada rodada parte de um projeto sem repositórios: o tipo não muda depois.
        let mut p = cadastrado(&c.dados, PROJETO).unwrap();
        p.repositorios = Repositorios::default();
        Cadastro::em(c.dados.projetos()).salvar(&p).unwrap();
        let r = Repositorios {
            forge: Some(forge),
            tracker: Some(TrackerConfig::Jira(jira())),
        };
        assert_eq!(
            salvar_repositorios(&c.dados, PROJETO, r.clone()).unwrap(),
            r
        );
        assert_eq!(cadastrado(&c.dados, PROJETO).unwrap().repositorios, r);
    }
}

#[test]
fn recusa_cadastro_invalido_e_troca_de_tipo() {
    let c = cenario();
    let ruim = Repositorios {
        forge: None,
        tracker: Some(TrackerConfig::Jira(JiraConfig {
            project_key: "proj".into(),
            ..jira()
        })),
    };
    assert!(
        salvar_repositorios(&c.dados, PROJETO, ruim)
            .unwrap_err()
            .contains("chave")
    );
    let r = Repositorios {
        forge: Some(ForgeConfig::Github(GithubConfig { repo: "o/r".into() })),
        tracker: Some(TrackerConfig::Jira(jira())),
    };
    salvar_repositorios(&c.dados, PROJETO, r.clone()).unwrap();
    let troca = Repositorios {
        forge: Some(ForgeConfig::Gitlab(gitlab())),
        ..r
    };
    assert!(
        salvar_repositorios(&c.dados, PROJETO, troca)
            .unwrap_err()
            .contains("não muda")
    );
}

#[test]
fn nenhum_token_vai_para_o_cadastro_em_disco() {
    let c = cenario();
    let r = Repositorios {
        forge: Some(ForgeConfig::Gitlab(gitlab())),
        tracker: Some(TrackerConfig::Jira(jira())),
    };
    salvar_repositorios(&c.dados, PROJETO, r).unwrap();
    let ferramentas = Ferramentas {
        jira: script(c.dir.path(), "jira", "echo ok"),
        glab: script(c.dir.path(), "glab", "echo ok"),
        ..Default::default()
    };
    let _ = prontidao(&c.dados, PROJETO, &ferramentas).unwrap();
    // Depois de usar as credenciais, o valor delas não está em arquivo algum dos dados.
    fn varre(dir: &Path, achados: &mut Vec<PathBuf>) {
        for e in fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                varre(&p, achados);
            } else if let Ok(t) = fs::read_to_string(&p)
                && (t.contains("tok-secreto") && !t.contains("echo tok-secreto")
                    || t.contains("tok-gitlab") && !t.contains("echo tok-gitlab"))
            {
                achados.push(p);
            }
        }
    }
    let mut achados = vec![];
    varre(c.dir.path(), &mut achados);
    assert!(achados.is_empty(), "{achados:?}");
    // O comando, sim, está no cadastro.
    let cad = fs::read_to_string(c.dados.projetos().join(PROJETO).join("projeto.json")).unwrap();
    assert!(cad.contains("echo tok-secreto"), "{cad}");
}

#[test]
fn analise_a_partir_de_uma_issue_traz_titulo_descricao_e_link() {
    let c = cenario();
    let r = Repositorios {
        forge: Some(ForgeConfig::Github(GithubConfig { repo: "o/r".into() })),
        tracker: Some(TrackerConfig::Jira(jira())),
    };
    salvar_repositorios(&c.dados, PROJETO, r).unwrap();
    let f = Ferramentas {
        jira: script(
            c.dir.path(),
            "jira",
            r#"[ "$JIRA_API_TOKEN" = "tok-secreto" ] || exit 1
echo '{"fields":{"summary":"Multa por atraso","description":"Revisar o cálculo da multa."}}'"#,
        ),
        ..Default::default()
    };
    let criada =
        criar_analise_de_issue(&c.dados, PROJETO, Fluxo::Perguntar, "PROJ-123", &f).unwrap();
    assert_eq!(criada.analise.titulo, "Multa por atraso");
    let vista = carregar_analise(&c.dados, PROJETO, &criada.analise.id).unwrap();
    assert!(vista.analise.entrada.contains("Multa por atraso"));
    assert!(
        vista
            .analise
            .entrada
            .contains("Revisar o cálculo da multa.")
    );
    let doc = fs::read_to_string(&vista.documento).unwrap();
    assert!(
        doc.contains("origem: https://jira.example.com/browse/PROJ-123"),
        "{doc}"
    );
    assert!(doc.contains("PROJ-123"), "{doc}");
    // Sem rastreador cadastrado, recusa com orientação.
    let mut p = cadastrado(&c.dados, PROJETO).unwrap();
    p.repositorios = Repositorios::default();
    Cadastro::em(c.dados.projetos()).salvar(&p).unwrap();
    assert!(
        criar_analise_de_issue(&c.dados, PROJETO, Fluxo::Perguntar, "PROJ-1", &f)
            .unwrap_err()
            .contains("repositório de issues")
    );
}

#[test]
fn revisao_de_requisito_pela_chave_no_github_no_gitlab_e_no_jira() {
    let c = cenario();
    let historia = "Como atendente, quero cancelar fatura com pagamento parcial.";
    let f = Ferramentas {
        gh: script(
            c.dir.path(),
            "gh",
            &format!(
                r#"echo '{{"title":"Cancelar fatura","body":"{historia}","url":"https://github.com/o/r/issues/7"}}'"#
            ),
        ),
        glab: script(
            c.dir.path(),
            "glab",
            &format!(
                r#"[ "$GITLAB_TOKEN" = "tok-gitlab" ] || exit 1
echo '{{"title":"Cancelar fatura","description":"{historia}","web_url":"https://gitlab.example.com/grupo/projeto/app/-/issues/8"}}'"#
            ),
        ),
        jira: script(
            c.dir.path(),
            "jira",
            &format!(
                r#"[ "$JIRA_API_TOKEN" = "tok-secreto" ] || exit 1
echo '{{"fields":{{"summary":"Cancelar fatura","description":"{historia}"}}}}'"#
            ),
        ),
    };
    for (tracker, referencia, fonte) in [
        (
            TrackerConfig::Github(GithubConfig { repo: "o/r".into() }),
            "7",
            "Fonte: [#7](https://github.com/o/r/issues/7)",
        ),
        (
            TrackerConfig::Gitlab(gitlab()),
            "#8",
            "Fonte: [#8](https://gitlab.example.com/grupo/projeto/app/-/issues/8)",
        ),
        (
            TrackerConfig::Jira(jira()),
            "proj-482",
            "Fonte: [PROJ-482](https://jira.example.com/browse/PROJ-482)",
        ),
    ] {
        let mut p = cadastrado(&c.dados, PROJETO).unwrap();
        p.repositorios = Repositorios::default();
        Cadastro::em(c.dados.projetos()).salvar(&p).unwrap();
        let r = Repositorios {
            forge: None,
            tracker: Some(tracker),
        };
        salvar_repositorios(&c.dados, PROJETO, r).unwrap();
        let criada =
            criar_analise_de_issue(&c.dados, PROJETO, Fluxo::Revisar, referencia, &f).unwrap();
        assert_eq!(criada.analise.fluxo, Fluxo::Revisar);
        assert_eq!(criada.analise.titulo, "Cancelar fatura");
        let vista = carregar_analise(&c.dados, PROJETO, &criada.analise.id).unwrap();
        assert!(vista.analise.entrada.contains(historia));
        let doc = fs::read_to_string(&vista.documento).unwrap();
        assert!(doc.contains("tipo: parecer-revisao"), "{doc}");
        // A chave e o link vão para o frontmatter; o parecer mostra a fonte.
        let fm: cba_kb::Documento<cba_kb::frontmatter::FrontmatterAnalise> =
            cba_kb::Documento::analisar(&doc).unwrap();
        assert_eq!(
            cba_tools::parecer::fonte(&fm.frontmatter.demanda).as_deref(),
            Some(fonte)
        );
    }
}

#[test]
fn exportacao_liga_as_evidencias_ao_forge_no_commit() {
    use crate::revisao::{PedidoExportacao, exportar};
    use cba_tools::revisao::{Formato, OpcoesExportacao};

    let c = cenario();
    let git = |args: &[&str]| {
        let s = std::process::Command::new("git")
            .args([
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .current_dir(&c.codigo)
            .status()
            .unwrap();
        assert!(s.success(), "git {args:?}");
    };
    git(&["init", "-q"]);
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "base"]);
    let commit = String::from_utf8(
        std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&c.codigo)
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    let commit = commit.trim().to_string();
    salvar_repositorios(
        &c.dados,
        PROJETO,
        Repositorios {
            forge: Some(ForgeConfig::Gitlab(gitlab())),
            tracker: None,
        },
    )
    .unwrap();
    c.aprovar_analise_da_multa();
    let destino = c.dir.path().join("multa");
    let md = exportar(
        &c.dados,
        PROJETO,
        "A-0001",
        &PedidoExportacao {
            destino,
            formato: Formato::Markdown,
            opcoes: OpcoesExportacao::default(),
        },
    )
    .unwrap();
    let texto = fs::read_to_string(md).unwrap();
    let esperado = format!(
        "https://gitlab.example.com/grupo/projeto/app/-/blob/{commit}/db/03_pkg_cobranca.sql#L71-72"
    );
    assert!(texto.contains(&esperado), "{texto}");
}
