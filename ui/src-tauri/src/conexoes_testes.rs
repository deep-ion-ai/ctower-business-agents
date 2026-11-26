//! Testes das Conexões nas Configurações (#437): cadastro, uso por dois
//! projetos, remoção recusada quando em uso e o "Testar" pelos passos do
//! descritor.

use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::*;
use crate::repositorios::{cadastrado, salvar_repositorios};
use crate::teste_base::{PROJETO, cenario};
use cba_repos::JiraConfig;
use cba_repos::chaveiro::ChaveiroMemoria;

const OUTRO: &str = "cobranca-exemplo";

fn script(dir: &Path, nome: &str, corpo: &str) -> PathBuf {
    let p = dir.join(nome);
    std::fs::write(&p, format!("#!/bin/sh\n{corpo}\n")).unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    p
}

fn nova(nome: &str, tipo: &str, endereco: &str) -> Conexao {
    Conexao {
        id: String::new(),
        nome: nome.into(),
        tipo: tipo.into(),
        endereco: endereco.into(),
        credencial: FonteCredencial::Chaveiro {
            conta: String::new(),
        },
        opcoes: BTreeMap::new(),
    }
}

/// Um segundo projeto, com as mesmas pastas do primeiro (só o cadastro).
fn outro_projeto(dados: &DiretorioDados) {
    let mut p = cadastrado(dados, PROJETO).unwrap();
    p.id = OUTRO.into();
    p.nome = "Cobrança Exemplo".into();
    Cadastro::em(dados.projetos()).salvar(&p).unwrap();
}

fn repositorios(jira: &str, gitlab: &str, caminho: &str, chave: &str) -> Repositorios {
    Repositorios {
        codigo: vec![RepositorioCodigo {
            forja: ForgeKind::Github,
            host: "outro.example.com".into(),
            caminho: caminho.into(),
            url_clone: format!("https://gitlab.example.com/{caminho}.git"),
            token: OrigemToken::Comando,
            comando_credencial: "echo antigo".into(),
            conexao: gitlab.into(),
            ..Default::default()
        }],
        tracker: Some(TrackerConfig::Jira(JiraConfig {
            url: "https://antigo.example.com".into(),
            project_key: chave.into(),
            credential_cmd: "echo antigo".into(),
            conexao: jira.into(),
            ..Default::default()
        })),
        docs: None,
    }
}

#[test]
fn cria_conexoes_e_dois_projetos_usam_as_mesmas() {
    let c = cenario();
    outro_projeto(&c.dados);
    let mut jira = nova("Jira da empresa", "jira", "https://jira.example.com/");
    jira.opcoes
        .insert("config_file".into(), "/tmp/jira.yml".into());
    jira.opcoes.insert("vazia".into(), "  ".into());
    let jira = salvar(&c.dados, jira).unwrap();
    assert_eq!(jira.id, "jira-da-empresa");
    assert_eq!(jira.endereco, "https://jira.example.com");
    assert_eq!(
        jira.credencial,
        FonteCredencial::Chaveiro {
            conta: "conexao:jira-da-empresa".into()
        }
    );
    let gitlab = salvar(
        &c.dados,
        nova("GitLab interno", "gitlab", "https://GitLab.Example.com/"),
    )
    .unwrap();
    assert_eq!(gitlab.endereco, "gitlab.example.com");
    // Nome repetido gera outro id.
    let mut repetido = nova("GitLab interno 2", "gitlab", "gitlab.example.com");
    repetido.nome = "GitLab Interno".into();
    assert!(
        salvar(&c.dados, repetido)
            .unwrap_err()
            .contains("já existe uma conexão chamada")
    );

    for (projeto, caminho, chave) in [(PROJETO, "g/fat", "FAT"), (OUTRO, "g/cob", "COB")] {
        let r = salvar_repositorios(
            &c.dados,
            projeto,
            repositorios(&jira.id, &gitlab.id, caminho, chave),
        )
        .unwrap();
        let repo = &r.codigo[0];
        assert_eq!(repo.forja, ForgeKind::Gitlab);
        assert_eq!(repo.host, "gitlab.example.com");
        assert_eq!(repo.token, OrigemToken::Global);
        assert!(repo.comando_credencial.is_empty());
        let Some(TrackerConfig::Jira(j)) = &r.tracker else {
            panic!("sem Jira");
        };
        assert_eq!(j.url, "https://jira.example.com");
        assert_eq!(j.config_file, "/tmp/jira.yml");
        assert!(j.credential_cmd.is_empty());
        assert_eq!(j.project_key, chave);
    }

    let u = usos(&c.dados).unwrap();
    let nomes = |id: &str| -> Vec<(String, String)> {
        u[id]
            .iter()
            .map(|x| (x.projeto.clone(), x.onde.clone()))
            .collect()
    };
    assert_eq!(
        nomes(&gitlab.id),
        [
            (OUTRO.to_string(), "código g/cob".to_string()),
            (PROJETO.to_string(), "código g/fat".to_string())
        ]
    );
    assert_eq!(nomes(&jira.id).len(), 2);
}

#[test]
fn conexao_em_uso_nao_e_removida_nem_muda_de_endereco() {
    let c = cenario();
    outro_projeto(&c.dados);
    let chaveiro = ChaveiroMemoria::default();
    let jira = salvar(&c.dados, nova("Jira", "jira", "https://jira.example.com")).unwrap();
    let gitlab = salvar(&c.dados, nova("GitLab", "gitlab", "gitlab.example.com")).unwrap();
    salvar_repositorios(
        &c.dados,
        PROJETO,
        repositorios(&jira.id, &gitlab.id, "g/a", "FAT"),
    )
    .unwrap();
    salvar_repositorios(
        &c.dados,
        OUTRO,
        repositorios(&jira.id, &gitlab.id, "g/b", "COB"),
    )
    .unwrap();

    let e = remover(&c.dados, &gitlab.id, &chaveiro).unwrap_err();
    assert!(e.contains("não pode ser removida"), "{e}");
    assert!(e.contains("Cobrança Exemplo"), "{e}");
    assert!(e.contains("Faturamento Exemplo"), "{e}");
    assert!(
        conexoes(&c.dados)
            .unwrap()
            .iter()
            .any(|x| x.id == gitlab.id)
    );

    let mut outro_host = gitlab.clone();
    outro_host.endereco = "gitlab.outro.example.com".into();
    let e = salvar(&c.dados, outro_host).unwrap_err();
    assert!(e.contains("não mudam"), "{e}");
    // Renomear pode.
    let mut renomeada = gitlab.clone();
    renomeada.nome = "GitLab da empresa".into();
    assert_eq!(salvar(&c.dados, renomeada).unwrap().id, gitlab.id);

    // Sem uso, sai, e o token guardado pelo app sai junto.
    let livre = salvar(&c.dados, nova("GitHub", "github", "github.com")).unwrap();
    guardar_credencial(&c.dados, &livre.id, "ghp_ficticio_123456", &chaveiro).unwrap();
    remover(&c.dados, &livre.id, &chaveiro).unwrap();
    assert!(
        chaveiro
            .ler(&conta_da_conexao(&livre.id))
            .unwrap()
            .is_none()
    );
}

#[test]
fn projeto_com_conexao_que_nao_existe_ou_nao_serve_e_recusado() {
    let c = cenario();
    let e = salvar_repositorios(
        &c.dados,
        PROJETO,
        repositorios("nao-existe", "", "g/a", "FAT"),
    )
    .unwrap_err();
    assert!(e.contains("não está cadastrada"), "{e}");
    let jira = salvar(&c.dados, nova("Jira", "jira", "https://jira.example.com")).unwrap();
    // Um Jira não é forja.
    let e = salvar_repositorios(
        &c.dados,
        PROJETO,
        repositorios(&jira.id, &jira.id, "g/a", "FAT"),
    )
    .unwrap_err();
    assert!(e.contains("não serve"), "{e}");
    // Um GitLab não é o rastreador de um cadastro Jira.
    let gitlab = salvar(&c.dados, nova("GitLab", "gitlab", "gitlab.example.com")).unwrap();
    let e = salvar_repositorios(
        &c.dados,
        PROJETO,
        repositorios(&gitlab.id, "", "g/a", "FAT"),
    )
    .unwrap_err();
    assert!(e.contains("outro tipo"), "{e}");
}

#[test]
fn lista_mostra_credencial_guardada_sem_o_valor_e_o_ultimo_teste() {
    let c = cenario();
    let chaveiro = Arc::new(ChaveiroMemoria::default());
    let gitlab = salvar(&c.dados, nova("GitLab", "gitlab", "gitlab.example.com")).unwrap();
    let v = listar(&c.dados, chaveiro.as_ref()).unwrap();
    assert_eq!(v[0].credencial_guardada, Some(false));
    assert_eq!(v[0].rotulo_tipo, "GitLab");
    guardar_credencial(
        &c.dados,
        &gitlab.id,
        "glpat-ficticio-conexao",
        chaveiro.as_ref(),
    )
    .unwrap();

    let glab = script(
        c.dir.path(),
        "glab",
        r#"case "$1 $2" in
"version ") echo "glab 1.48.0";;
"api version") [ "$GITLAB_TOKEN" = glpat-ficticio-conexao ] && echo '{"version":"17.2.0"}' || { echo "401 Unauthorized" >&2; exit 1; };;
"api user") echo '{"username":"ana"}';;
*) exit 2;;
esac"#,
    );
    let f = Ferramentas {
        glab,
        chaveiro: chaveiro.clone(),
        ..Default::default()
    };
    let i = testar(&c.dados, &gitlab.id, &f).unwrap();
    assert!(i.pronta(), "{i:?}");
    let v = listar(&c.dados, chaveiro.as_ref()).unwrap();
    assert_eq!(v[0].credencial_guardada, Some(true));
    let t = v[0].ultimo_teste.as_ref().unwrap();
    assert!(t.pronta);
    assert_eq!(t.resumo, "GitLab 17.2.0, conectado como ana");
    let json = serde_json::to_string(&v).unwrap();
    assert!(!json.contains("glpat-ficticio-conexao"), "{json}");

    // Token trocado e recusado: o teste mostra o passo com a orientação.
    guardar_credencial(
        &c.dados,
        &gitlab.id,
        "glpat-outro-recusado",
        chaveiro.as_ref(),
    )
    .unwrap();
    let i = testar(&c.dados, &gitlab.id, &f).unwrap();
    let falhou = i
        .passos
        .iter()
        .find(|p| p.situacao == cba_repos::prontidao::Situacao::Falhou)
        .unwrap();
    assert_eq!(falhou.id, "versao_servidor");
    assert!(falhou.orientacao.is_some());
    assert!(!format!("{i:?}").contains("glpat-outro-recusado"));
    let v = listar(&c.dados, chaveiro.as_ref()).unwrap();
    assert!(!v[0].ultimo_teste.as_ref().unwrap().pronta);
}

#[test]
fn credencial_so_vai_para_conexao_com_fonte_no_chaveiro() {
    let c = cenario();
    let chaveiro = ChaveiroMemoria::default();
    let mut cmd = nova("Jira", "jira", "https://jira.example.com");
    cmd.credencial = FonteCredencial::Comando {
        cmd: "pass show jira".into(),
    };
    let cmd = salvar(&c.dados, cmd).unwrap();
    assert!(guardar_credencial(&c.dados, &cmd.id, "x", &chaveiro).is_err());
}
