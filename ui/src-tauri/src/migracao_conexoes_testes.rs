//! Testes da migração dos cadastros para conexões (#438), com cada formato de
//! cadastro de hoje (dados fictícios do sistema "Faturamento Exemplo").

use std::path::Path;

use super::*;
use cba_repos::chaveiro::{ChaveiroMemoria, conta_comando_global, conta_propria};
use cba_repos::docs::DestinoDocs;
use cba_repos::providers::Resolvedor;
use cba_repos::{ForgeKind, GithubConfig, JiraConfig, Repositorios, Secret};
use serde_json::json;

struct Cenario {
    _dir: tempfile::TempDir,
    dados: DiretorioDados,
    pasta: std::path::PathBuf,
}

fn cenario() -> Cenario {
    let dir = tempfile::tempdir().unwrap();
    let pasta = dir.path().join("codigo");
    std::fs::create_dir_all(&pasta).unwrap();
    let dados = DiretorioDados::em(dir.path().join("dados"));
    Cenario {
        _dir: dir,
        dados,
        pasta,
    }
}

fn cadastrar(c: &Cenario, nome: &str, repositorios: Repositorios) -> String {
    let pasta = c.pasta.join(cba_index::slug(nome));
    std::fs::create_dir_all(&pasta).unwrap();
    let mut p = Projeto::novo(nome, [&pasta]).unwrap();
    p.repositorios = repositorios;
    Cadastro::em(c.dados.projetos()).salvar(&p).unwrap();
    p.id
}

fn arquivo_projeto(c: &Cenario, id: &str) -> std::path::PathBuf {
    c.dados.projetos().join(id).join("projeto.json")
}

fn obter(c: &Cenario, id: &str) -> Projeto {
    Cadastro::em(c.dados.projetos()).obter(id).unwrap().unwrap()
}

fn gitlab_repo(caminho: &str) -> RepositorioCodigo {
    RepositorioCodigo {
        forja: ForgeKind::Gitlab,
        host: "gitlab.example.com".into(),
        caminho: caminho.into(),
        url_clone: format!("https://gitlab.example.com/{caminho}.git"),
        ..Default::default()
    }
}

fn jira(chave: &str, cmd: &str) -> TrackerConfig {
    TrackerConfig::Jira(JiraConfig {
        url: "https://jira.example.com".into(),
        project_key: chave.into(),
        credential_cmd: cmd.into(),
        config_file: "/home/analista/.config/.jira/.config.yml".into(),
        ..Default::default()
    })
}

/// Todos os arquivos de dados (menos os backups), para comparar duas rodadas.
fn retrato(dados: &DiretorioDados) -> Vec<(String, String)> {
    let mut v = Vec::new();
    for raiz in [dados.config(), dados.projetos()] {
        let mut pilha = vec![raiz];
        while let Some(d) = pilha.pop() {
            let Ok(entradas) = std::fs::read_dir(&d) else {
                continue;
            };
            for e in entradas.flatten() {
                let p = e.path();
                if p.is_dir() {
                    pilha.push(p);
                } else if !p.to_string_lossy().contains(".antes-conexoes-") {
                    v.push((
                        p.display().to_string(),
                        std::fs::read_to_string(&p).unwrap_or_default(),
                    ));
                }
            }
        }
    }
    v.sort();
    v
}

fn backups(dir: &Path) -> Vec<String> {
    std::fs::read_dir(dir)
        .map(|r| {
            r.flatten()
                .map(|e| e.file_name().to_string_lossy().to_string())
                .filter(|n| n.contains(".antes-conexoes-"))
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn forge_antigo_de_um_item_so_vira_conexao_com_o_comando() {
    let c = cenario();
    let id = cadastrar(&c, "Faturamento Exemplo", Repositorios::default());
    // O cadastro de antes da #178: `forge` com o GitLab e o comando de credencial.
    let arq = arquivo_projeto(&c, &id);
    let mut v: serde_json::Value = serde_json::from_slice(&std::fs::read(&arq).unwrap()).unwrap();
    v["repositorios"] = json!({
        "forge": {
            "kind": "gitlab",
            "host": "gitlab.example.com",
            "path": "fat/app",
            "clone_url": "git@gitlab.example.com:fat/app.git",
            "credential_cmd": "pass show gitlab | head -1"
        }
    });
    std::fs::write(&arq, serde_json::to_vec(&v).unwrap()).unwrap();

    let chaveiro = ChaveiroMemoria::default();
    let r = migrar(&c.dados, &chaveiro).unwrap();
    assert_eq!(r.criadas, ["GitLab gitlab.example.com"]);
    assert_eq!(r.projetos, std::slice::from_ref(&id));
    let lista = conexoes::conexoes(&c.dados).unwrap();
    assert_eq!(
        lista[0].credencial,
        FonteCredencial::Comando {
            cmd: "pass show gitlab | head -1".into()
        }
    );
    let repo = &obter(&c, &id).repositorios.codigo[0];
    assert_eq!(repo.conexao, lista[0].id);
    assert_eq!(repo.caminho, "fat/app");
    assert_eq!(repo.url_clone, "git@gitlab.example.com:fat/app.git");
    assert!(repo.comando_credencial.is_empty());
    // O backup é o arquivo antigo, com `forge`, ao lado do original.
    let b = backups(arq.parent().unwrap());
    assert_eq!(b.len(), 1, "{b:?}");
    let antigo = std::fs::read_to_string(arq.parent().unwrap().join(&b[0])).unwrap();
    assert!(antigo.contains("\"forge\""));
}

#[test]
fn lista_codigo_agrupa_por_forja_e_host_e_reaproveita_o_token_do_host() {
    let c = cenario();
    let chaveiro = ChaveiroMemoria::default();
    chaveiro
        .gravar(
            &conta_global("gitlab.example.com"),
            &Secret::new("glpat-ficticio-host"),
        )
        .unwrap();
    let a = cadastrar(
        &c,
        "Faturamento Exemplo",
        Repositorios {
            codigo: vec![
                gitlab_repo("fat/app"),
                gitlab_repo("fat/banco"),
                RepositorioCodigo {
                    caminho: "exemplo/site".into(),
                    ..Default::default()
                },
            ],
            tracker: Some(TrackerConfig::Github(GithubConfig {
                repo: "exemplo/site".into(),
                ..Default::default()
            })),
            docs: None,
        },
    );
    let b = cadastrar(
        &c,
        "Cobrança Exemplo",
        Repositorios {
            codigo: vec![gitlab_repo("cob/app")],
            ..Default::default()
        },
    );
    let r = migrar(&c.dados, &chaveiro).unwrap();
    assert_eq!(
        r.criadas,
        ["GitLab gitlab.example.com", "GitHub github.com"],
        "{r:?}"
    );
    let lista = conexoes::conexoes(&c.dados).unwrap();
    let gl = lista.iter().find(|x| x.tipo == "gitlab").unwrap();
    // Sem copiar o valor: a mesma conta do chaveiro.
    assert_eq!(
        gl.credencial,
        FonteCredencial::Chaveiro {
            conta: "host:gitlab.example.com".into()
        }
    );
    let json = std::fs::read_to_string(conexoes::arquivo(&c.dados)).unwrap();
    assert!(!json.contains("glpat-ficticio-host"));
    let pa = obter(&c, &a).repositorios;
    let pb = obter(&c, &b).repositorios;
    assert!(pa.codigo[..2].iter().all(|x| x.conexao == gl.id));
    assert_eq!(pb.codigo[0].conexao, gl.id);
    // O GitHub do código e das issues dividem a conexão do github.com.
    let gh = lista.iter().find(|x| x.tipo == "github").unwrap();
    assert_eq!(pa.codigo[2].conexao, gh.id);
    let Some(TrackerConfig::Github(t)) = &pa.tracker else {
        panic!()
    };
    assert_eq!(t.conexao, gh.id);
    assert_eq!(t.repo, "exemplo/site");

    // Depois de migrar, a credencial resolvida é a mesma de antes.
    let token = Resolvedor::global()
        .resolver(gl, &chaveiro, None)
        .unwrap()
        .unwrap();
    assert_eq!(token.expose(), "glpat-ficticio-host");
}

#[test]
fn jira_com_credential_cmd_vira_comando_e_guarda_so_o_que_e_do_projeto() {
    let c = cenario();
    let a = cadastrar(
        &c,
        "Faturamento Exemplo",
        Repositorios {
            tracker: Some(jira("FAT", "pass show jira | head -1")),
            ..Default::default()
        },
    );
    let b = cadastrar(
        &c,
        "Cobrança Exemplo",
        Repositorios {
            tracker: Some(jira("COB", "pass show jira | head -1")),
            ..Default::default()
        },
    );
    let r = migrar(&c.dados, &ChaveiroMemoria::default()).unwrap();
    assert_eq!(r.criadas, ["Jira jira.example.com"]);
    let lista = conexoes::conexoes(&c.dados).unwrap();
    assert_eq!(lista.len(), 1);
    assert_eq!(
        lista[0].credencial,
        FonteCredencial::Comando {
            cmd: "pass show jira | head -1".into()
        }
    );
    assert_eq!(
        lista[0].opcoes.get("config_file").map(String::as_str),
        Some("/home/analista/.config/.jira/.config.yml")
    );
    for (id, chave) in [(a, "FAT"), (b, "COB")] {
        let Some(TrackerConfig::Jira(j)) = obter(&c, &id).repositorios.tracker else {
            panic!()
        };
        assert_eq!(j.conexao, lista[0].id);
        assert_eq!(j.project_key, chave);
        assert!(j.credential_cmd.is_empty());
        assert!(j.active_sprint_only);
    }
}

#[test]
fn gitlab_com_comando_do_host_vira_comando() {
    let c = cenario();
    let chaveiro = ChaveiroMemoria::default();
    chaveiro
        .gravar(
            &conta_comando_global("gitlab.example.com"),
            &Secret::new("pass show gitlab-host | head -1"),
        )
        .unwrap();
    cadastrar(
        &c,
        "Faturamento Exemplo",
        Repositorios {
            codigo: vec![gitlab_repo("fat/app")],
            ..Default::default()
        },
    );
    migrar(&c.dados, &chaveiro).unwrap();
    let lista = conexoes::conexoes(&c.dados).unwrap();
    assert_eq!(
        lista[0].credencial,
        FonteCredencial::Comando {
            cmd: "pass show gitlab-host | head -1".into()
        }
    );
}

#[test]
fn token_proprio_do_repositorio_fica_como_sobrescrita() {
    let c = cenario();
    let chaveiro = ChaveiroMemoria::default();
    let proprio = RepositorioCodigo {
        token: OrigemToken::Proprio,
        ..gitlab_repo("fat/segredo")
    };
    chaveiro
        .gravar(&conta_propria(&proprio), &Secret::new("glpat-so-deste"))
        .unwrap();
    chaveiro
        .gravar(
            &conta_global("gitlab.example.com"),
            &Secret::new("glpat-do-host"),
        )
        .unwrap();
    let id = cadastrar(
        &c,
        "Faturamento Exemplo",
        Repositorios {
            codigo: vec![proprio.clone(), gitlab_repo("fat/app")],
            ..Default::default()
        },
    );
    migrar(&c.dados, &chaveiro).unwrap();
    let lista = conexoes::conexoes(&c.dados).unwrap();
    assert_eq!(lista.len(), 1);
    let repos = obter(&c, &id).repositorios.codigo;
    assert_eq!(repos[0].token, OrigemToken::Proprio);
    // A conta `repo:` continua a mesma e vale no lugar da conexão.
    assert_eq!(conta_propria(&repos[0]), conta_propria(&proprio));
    let t = Resolvedor::global()
        .resolver_com_sobrescrita(&lista[0], Some(&conta_propria(&repos[0])), &chaveiro, None)
        .unwrap()
        .unwrap();
    assert_eq!(t.expose(), "glpat-so-deste");
    let t = Resolvedor::global()
        .resolver_com_sobrescrita(&lista[0], Some(&conta_propria(&repos[1])), &chaveiro, None)
        .unwrap()
        .unwrap();
    assert_eq!(t.expose(), "glpat-do-host");
}

#[test]
fn destino_de_docs_em_repositorio_proprio_aponta_para_a_conexao() {
    let c = cenario();
    let id = cadastrar(
        &c,
        "Faturamento Exemplo",
        Repositorios {
            codigo: vec![gitlab_repo("fat/app")],
            tracker: None,
            docs: Some(DestinoDocs::Repositorio {
                repositorio: RepositorioCodigo {
                    token: OrigemToken::Comando,
                    comando_credencial: "pass show docs | head -1".into(),
                    branch: "main".into(),
                    ..gitlab_repo("fat/docs")
                },
                subpasta: "negocio".into(),
            }),
        },
    );
    let r = migrar(&c.dados, &ChaveiroMemoria::default()).unwrap();
    // O comando próprio do repositório de docs separa o grupo.
    assert_eq!(
        r.criadas,
        ["GitLab gitlab.example.com", "GitLab gitlab.example.com (2)"]
    );
    let lista = conexoes::conexoes(&c.dados).unwrap();
    let p = obter(&c, &id).repositorios;
    let Some(DestinoDocs::Repositorio {
        repositorio,
        subpasta,
    }) = &p.docs
    else {
        panic!()
    };
    let docs = lista.iter().find(|x| x.id == repositorio.conexao).unwrap();
    assert_eq!(
        docs.credencial,
        FonteCredencial::Comando {
            cmd: "pass show docs | head -1".into()
        }
    );
    assert_eq!(subpasta, "negocio");
    assert_eq!(repositorio.branch, "main");
    assert_ne!(p.codigo[0].conexao, repositorio.conexao);
}

#[test]
fn rodar_duas_vezes_da_o_mesmo_resultado() {
    let c = cenario();
    cadastrar(
        &c,
        "Faturamento Exemplo",
        Repositorios {
            codigo: vec![gitlab_repo("fat/app")],
            tracker: Some(jira("FAT", "")),
            docs: None,
        },
    );
    let chaveiro = ChaveiroMemoria::default();
    let r1 = migrar(&c.dados, &chaveiro).unwrap();
    assert_eq!(r1.criadas.len(), 2);
    let depois = retrato(&c.dados);
    let r2 = migrar(&c.dados, &chaveiro).unwrap();
    assert_eq!(r2, ResumoMigracao::default());
    assert_eq!(retrato(&c.dados), depois);
    // A segunda rodada não faz backup de novo.
    assert_eq!(backups(&c.dados.config()).len(), 2);
}

#[test]
fn falha_no_meio_restaura_o_backup() {
    let c = cenario();
    let a = cadastrar(
        &c,
        "Cobrança Exemplo",
        Repositorios {
            codigo: vec![gitlab_repo("cob/app")],
            ..Default::default()
        },
    );
    let b = cadastrar(
        &c,
        "Faturamento Exemplo",
        Repositorios {
            tracker: Some(jira("FAT", "pass show jira | head -1")),
            ..Default::default()
        },
    );
    let antes = retrato(&c.dados);
    let mut gravados = 0;
    let e = migrar_com(&c.dados, &ChaveiroMemoria::default(), |cad, p| {
        gravados += 1;
        if gravados == 2 {
            return Err("disco cheio (simulado)".into());
        }
        cad.salvar(p).map_err(|e| e.to_string())
    })
    .unwrap_err();
    assert!(e.contains("foi desfeita"), "{e}");
    assert!(e.contains("disco cheio"), "{e}");
    // Tudo como antes: o primeiro projeto gravado voltou e as conexões sumiram.
    assert_eq!(retrato(&c.dados), antes);
    assert!(!conexoes::arquivo(&c.dados).exists());
    assert!(obter(&c, &a).repositorios.codigo[0].conexao.is_empty());
    let Some(TrackerConfig::Jira(j)) = obter(&c, &b).repositorios.tracker else {
        panic!()
    };
    assert_eq!(j.credential_cmd, "pass show jira | head -1");
}

#[test]
fn sem_nada_a_migrar_nao_grava_nem_faz_backup() {
    let c = cenario();
    cadastrar(&c, "Faturamento Exemplo", Repositorios::default());
    let r = migrar(&c.dados, &ChaveiroMemoria::default()).unwrap();
    assert_eq!(r, ResumoMigracao::default());
    assert!(backups(&c.dados.config()).is_empty());
    assert!(!conexoes::arquivo(&c.dados).exists());
}
