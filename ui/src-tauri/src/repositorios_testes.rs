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
            codigo: vec![forge.into()],
            tracker: Some(TrackerConfig::Jira(jira())),
            docs: None,
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
        codigo: vec![],
        tracker: Some(TrackerConfig::Jira(JiraConfig {
            project_key: "proj".into(),
            ..jira()
        })),
        docs: None,
    };
    assert!(
        salvar_repositorios(&c.dados, PROJETO, ruim)
            .unwrap_err()
            .contains("chave")
    );
    let r = Repositorios {
        codigo: vec![ForgeConfig::Github(GithubConfig { repo: "o/r".into() }).into()],
        tracker: Some(TrackerConfig::Jira(jira())),
        docs: None,
    };
    salvar_repositorios(&c.dados, PROJETO, r.clone()).unwrap();
    let troca = Repositorios {
        codigo: vec![ForgeConfig::Gitlab(gitlab()).into()],
        ..r.clone()
    };
    assert!(
        salvar_repositorios(&c.dados, PROJETO, troca)
            .unwrap_err()
            .contains("não pode ser removido nem trocado")
    );
    // Acrescentar um segundo repositório pode.
    let mais = Repositorios {
        codigo: vec![r.codigo[0].clone(), ForgeConfig::Gitlab(gitlab()).into()],
        ..r
    };
    assert_eq!(
        salvar_repositorios(&c.dados, PROJETO, mais)
            .unwrap()
            .codigo
            .len(),
        2
    );
}

#[test]
fn cadastra_edita_e_remove_o_destino_da_documentacao() {
    let c = cenario();
    let p = cadastrado(&c.dados, PROJETO).unwrap();
    let raiz = p.pastas[0].caminho.clone();
    let codigo = cba_repos::RepositorioCodigo {
        caminho: "exemplo/faturamento".into(),
        pasta: Some(raiz.clone()),
        ..Default::default()
    };
    let sem = Repositorios {
        codigo: vec![codigo.clone()],
        tracker: None,
        docs: None,
    };
    salvar_repositorios(&c.dados, PROJETO, sem.clone()).unwrap();
    // Subpasta do código, digitada com \ e barra no fim: grava normalizada.
    let na_pasta = Repositorios {
        docs: Some(cba_repos::DestinoDocs::Subpasta {
            codigo: "GitHub.com/Exemplo/Faturamento".into(),
            subpasta: "docs\\negocio\\".into(),
        }),
        ..sem.clone()
    };
    let gravado = salvar_repositorios(&c.dados, PROJETO, na_pasta).unwrap();
    assert_eq!(
        gravado.docs,
        Some(cba_repos::DestinoDocs::Subpasta {
            codigo: "github.com/exemplo/faturamento".into(),
            subpasta: "docs/negocio".into(),
        })
    );
    let projeto = cadastrado(&c.dados, PROJETO).unwrap();
    assert_eq!(projeto.repositorios, gravado);
    assert_eq!(
        projeto.pastas_de_documentacao(),
        vec![raiz.join("docs").join("negocio")]
    );
    // Subpasta com `..`: recusa e nada muda.
    let fora = Repositorios {
        docs: Some(cba_repos::DestinoDocs::Subpasta {
            codigo: codigo.chave(),
            subpasta: "../fora".into(),
        }),
        ..sem.clone()
    };
    assert!(salvar_repositorios(&c.dados, PROJETO, fora).is_err());
    assert_eq!(cadastrado(&c.dados, PROJETO).unwrap().repositorios, gravado);
    // Troca para um repositório próprio.
    let proprio = Repositorios {
        docs: Some(cba_repos::DestinoDocs::Repositorio {
            repositorio: cba_repos::RepositorioCodigo {
                caminho: "exemplo/faturamento-docs".into(),
                branch: "main".into(),
                ..Default::default()
            },
            subpasta: String::new(),
        }),
        ..sem.clone()
    };
    let gravado = salvar_repositorios(&c.dados, PROJETO, proprio).unwrap();
    assert_eq!(gravado.destino_docs().unwrap().branch_base(), Some("main"));
    // E remove.
    assert_eq!(
        salvar_repositorios(&c.dados, PROJETO, sem.clone()).unwrap(),
        sem
    );
    assert_eq!(
        cadastrado(&c.dados, PROJETO).unwrap().repositorios.docs,
        None
    );
}

#[test]
fn nenhum_token_vai_para_o_cadastro_em_disco() {
    let c = cenario();
    let r = Repositorios {
        codigo: vec![ForgeConfig::Gitlab(gitlab()).into()],
        tracker: Some(TrackerConfig::Jira(jira())),
        docs: None,
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
        codigo: vec![ForgeConfig::Github(GithubConfig { repo: "o/r".into() }).into()],
        tracker: Some(TrackerConfig::Jira(jira())),
        docs: None,
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
    let criada = criar_analise_de_issue(
        &c.dados,
        PROJETO,
        Fluxo::Perguntar,
        "PROJ-123",
        None,
        None,
        &f,
    )
    .unwrap();
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
        criar_analise_de_issue(
            &c.dados,
            PROJETO,
            Fluxo::Perguntar,
            "PROJ-1",
            None,
            None,
            &f
        )
        .unwrap_err()
        .contains("repositório de issues")
    );
}

#[test]
fn pergunta_ligada_a_issue_usa_a_pergunta_como_entrada() {
    let c = cenario();
    let r = Repositorios {
        codigo: vec![ForgeConfig::Github(GithubConfig { repo: "o/r".into() }).into()],
        tracker: Some(TrackerConfig::Jira(jira())),
        docs: None,
    };
    salvar_repositorios(&c.dados, PROJETO, r).unwrap();
    let f = Ferramentas {
        jira: script(
            c.dir.path(),
            "jira",
            r#"echo '{"fields":{"summary":"Multa por atraso","description":"Revisar o cálculo da multa."}}'"#,
        ),
        ..Default::default()
    };
    let criada = criar_analise_de_issue(
        &c.dados,
        PROJETO,
        Fluxo::Perguntar,
        "PROJ-123",
        Some("Como é calculada a multa?"),
        None,
        &f,
    )
    .unwrap();
    let vista = carregar_analise(&c.dados, PROJETO, &criada.analise.id).unwrap();
    assert_eq!(vista.analise.entrada.trim(), "Como é calculada a multa?");
    // O título sai da pergunta, não da issue; a issue só liga o resultado.
    assert_ne!(criada.analise.titulo, "Multa por atraso");
    let doc = fs::read_to_string(&vista.documento).unwrap();
    assert!(
        doc.contains("origem: https://jira.example.com/browse/PROJ-123"),
        "{doc}"
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
        ..Default::default()
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
            codigo: vec![],
            tracker: Some(tracker),
            docs: None,
        };
        salvar_repositorios(&c.dados, PROJETO, r).unwrap();
        let criada = criar_analise_de_issue(
            &c.dados,
            PROJETO,
            Fluxo::Revisar,
            referencia,
            None,
            None,
            &f,
        )
        .unwrap();
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
fn exportacao_liga_cada_evidencia_ao_seu_repositorio() {
    use crate::revisao::{PedidoExportacao, exportar};
    use cba_tools::revisao::{Formato, OpcoesExportacao};

    // Dois repositórios: o GitLab na raiz do projeto e o GitHub em `db/` (um
    // repositório dentro do outro). A evidência da multa fica em `db/`.
    let c = cenario();
    for args in [
        &["init", "-q"][..],
        &["add", "-A"],
        &["commit", "-q", "-m", "base"],
    ] {
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
    }
    let gl = cba_repos::RepositorioCodigo {
        pasta: Some(c.codigo.clone()),
        ..ForgeConfig::Gitlab(gitlab()).into()
    };
    let gh = cba_repos::RepositorioCodigo {
        pasta: Some(c.codigo.join("db")),
        ..ForgeConfig::Github(GithubConfig {
            repo: "deep-ion-ai/banco".into(),
        })
        .into()
    };
    salvar_repositorios(
        &c.dados,
        PROJETO,
        Repositorios {
            codigo: vec![gl, gh],
            tracker: None,
            docs: None,
        },
    )
    .unwrap();
    c.aprovar_analise_da_multa();
    let md = exportar(
        &c.dados,
        PROJETO,
        "A-0001",
        &PedidoExportacao {
            destino: c.dir.path().join("multa"),
            formato: Formato::Markdown,
            opcoes: OpcoesExportacao::default(),
            figuras: Vec::new(),
        },
    )
    .unwrap();
    let texto = fs::read_to_string(md).unwrap();
    assert!(
        texto.contains("](<https://github.com/deep-ion-ai/banco/blob/"),
        "{texto}"
    );
    assert!(texto.contains("/03_pkg_cobranca.sql#L71-L72>)"), "{texto}");
    assert!(!texto.contains("gitlab.example.com"), "{texto}");
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
            codigo: vec![ForgeConfig::Gitlab(gitlab()).into()],
            tracker: None,
            docs: None,
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
            figuras: Vec::new(),
        },
    )
    .unwrap();
    let texto = fs::read_to_string(md).unwrap();
    let esperado = format!(
        "https://gitlab.example.com/grupo/projeto/app/-/blob/{commit}/db/03_pkg_cobranca.sql#L71-72"
    );
    assert!(texto.contains(&esperado), "{texto}");
}

#[test]
fn detecta_repositorios_na_pasta_e_deduz_a_forja() {
    let dir = tempfile::tempdir().unwrap();
    let base = dunce::canonicalize(dir.path()).unwrap();
    for (nome, url) in [
        ("web", "git@github.com:deep-ion-ai/web.git"),
        ("legado", "https://gitlab.example.com/grupo/legado.git"),
    ] {
        let p = base.join(nome);
        fs::create_dir_all(&p).unwrap();
        let ok = |args: &[&str]| {
            assert!(
                std::process::Command::new("git")
                    .args(args)
                    .current_dir(&p)
                    .status()
                    .unwrap()
                    .success()
            )
        };
        ok(&["init", "-q", "-b", "main"]);
        ok(&["remote", "add", "origin", url]);
    }
    fs::create_dir_all(base.join("solta")).unwrap();
    let achados = detectar_repositorios(&base).unwrap();
    let resumo: Vec<_> = achados
        .iter()
        .map(|r| {
            (
                r.caminho
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
                r.forja,
                r.host.clone().unwrap_or_default(),
                r.caminho_repo.clone().unwrap_or_default(),
                r.branch.clone().unwrap_or_default(),
            )
        })
        .collect();
    assert_eq!(
        resumo,
        [
            (
                "legado".to_string(),
                Some(cba_repos::ForgeKind::Gitlab),
                "gitlab.example.com".to_string(),
                "grupo/legado".to_string(),
                "main".to_string()
            ),
            (
                "web".to_string(),
                Some(cba_repos::ForgeKind::Github),
                "github.com".to_string(),
                "deep-ion-ai/web".to_string(),
                "main".to_string()
            ),
        ]
    );
    // A própria pasta é um repositório: só ela.
    assert_eq!(detectar_repositorios(&base.join("web")).unwrap().len(), 1);
    assert!(detectar_repositorios(Path::new("relativa")).is_err());
}
