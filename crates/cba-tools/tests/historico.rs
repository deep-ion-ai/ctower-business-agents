//! `historico_do_trecho` (#450) num repositório git temporário, no estilo do
//! sistema fictício "Faturamento Exemplo": a origem de um ramo de regra pela
//! chave da demanda na mensagem do commit, pelo intervalo de linhas e pelo
//! identificador; sem chave nas mensagens, nenhuma chave inventada; o corte com
//! aviso; o título da issue no rastreador; e o commit citado como origem no
//! `afirmar`, com o autor só no estado.
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use cba_core::ambiente::Ambiente;
use cba_core::analise::Fluxo;
use cba_core::analises::NovaAnalise;
use cba_core::config::Config;
use cba_core::dados::DiretorioDados;
use cba_repos::demanda::Ferramentas;
use cba_repos::{JiraConfig, TrackerConfig};
use cba_tools::historico::historico_do_trecho_com;
use cba_tools::{ErroChamada, ErroFerramenta, Registro, Saida, registro_padrao};
use serde_json::{Value, json};

const AUTOR: &str = "Pessoa Exemplo";
const DEMANDA: &str = "[DEM-12] Manter a fatura em aberto quando o pagamento for estornado";

fn git(dir: &Path, args: &[&str], data: Option<&str>) -> String {
    let mut c = Command::new("git");
    c.arg("-C")
        .arg(dir)
        .args([
            "-c",
            format!("user.name={AUTOR}").as_str(),
            "-c",
            "user.email=pessoa@exemplo.com",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "init.defaultBranch=main",
        ])
        .args(args)
        .env("LC_ALL", "C");
    if let Some(d) = data {
        let instante = format!("{d}T12:00:00+00:00");
        c.env("GIT_AUTHOR_DATE", &instante)
            .env("GIT_COMMITTER_DATE", &instante);
    }
    let s = c.output().unwrap();
    assert!(
        s.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&s.stderr)
    );
    String::from_utf8_lossy(&s.stdout).trim().to_owned()
}

/// Grava os arquivos e faz um commit com a data e a mensagem (assunto e corpo).
fn commit(
    repo: &Path,
    data: &str,
    assunto: &str,
    corpo: &str,
    arquivos: &[(&str, &str)],
) -> String {
    for (caminho, conteudo) in arquivos {
        let p = repo.join(caminho);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, conteudo).unwrap();
    }
    git(repo, &["add", "-A"], None);
    let mut args = vec!["commit", "-q", "-m", assunto];
    if !corpo.is_empty() {
        args.extend(["-m", corpo]);
    }
    git(repo, &args, Some(data));
    git(repo, &["rev-parse", "HEAD"], None)
}

const ESTORNO_V1: &str = "\
CREATE OR REPLACE PROCEDURE PRC_ESTORNAR (P_ID IN NUMBER) IS
BEGIN
    UPDATE PAGAMENTO SET SITUACAO = 'ESTORNADO' WHERE ID = P_ID;
END;
";

const ESTORNO_V2: &str = "\
CREATE OR REPLACE PROCEDURE PRC_ESTORNAR (P_ID IN NUMBER) IS
BEGIN
    UPDATE PAGAMENTO SET SITUACAO = 'ESTORNADO' WHERE ID = P_ID;
    -- Pagamento estornado: a fatura volta a ficar em aberto
    IF V_ESTORNO_TOTAL THEN
        UPDATE FATURA SET STATUS = 'ABERTA' WHERE ID_PAGAMENTO = P_ID;
    END IF;
END;
";

const ESTORNO_V3: &str = "\
CREATE OR REPLACE PROCEDURE PRC_ESTORNAR (P_ID IN NUMBER) IS
BEGIN
    UPDATE PAGAMENTO SET SITUACAO = 'ESTORNADO' WHERE ID = P_ID;
    -- Estorno mantem a fatura em aberto
    IF V_ESTORNO_TOTAL THEN
        UPDATE FATURA SET STATUS = 'ABERTA' WHERE ID_PAGAMENTO = P_ID;
    END IF;
END;
";

struct Caso {
    _dir: tempfile::TempDir,
    amb: Ambiente,
    raiz: PathBuf,
}

/// O projeto cadastrado sobre `repo`, com a base e os dados fora dele.
fn caso(dir: tempfile::TempDir, tracker: Option<TrackerConfig>) -> Caso {
    let raiz = dir.path().to_path_buf();
    let repo = raiz.join("repo");
    let dados = DiretorioDados::em(raiz.join("dados"));
    let mut projeto = cba_index::Projeto::novo("Faturamento Teste", [&repo]).unwrap();
    projeto.repositorios.tracker = tracker;
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
    let amb = Ambiente::do_diretorio(&repo, &dados).unwrap();
    Caso {
        _dir: dir,
        amb,
        raiz,
    }
}

/// O repositório com o ramo de regra criado pela DEM-12 e ajustado depois.
fn repositorio_do_estorno() -> (tempfile::TempDir, [String; 3]) {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-q"], None);
    let c1 = commit(
        &repo,
        "2023-01-10",
        "Carga inicial do pacote de pagamento",
        "",
        &[("db/pkg_pagamento.sql", ESTORNO_V1)],
    );
    let c2 = commit(
        &repo,
        "2024-03-12",
        DEMANDA,
        "Pedido do financeiro: o estorno devolve a fatura para cobranca.",
        &[("db/pkg_pagamento.sql", ESTORNO_V2)],
    );
    let c3 = commit(
        &repo,
        "2025-02-01",
        "Ajusta comentario do estorno",
        "",
        &[("db/pkg_pagamento.sql", ESTORNO_V3)],
    );
    (dir, [c1, c2, c3])
}

fn chamar(r: &Registro, amb: &Ambiente, nome: &str, args: Value) -> Result<Saida, ErroFerramenta> {
    match r.chamar(nome, Ok(amb), Some(args)) {
        Ok(s) => Ok(s),
        Err(ErroChamada::Ferramenta(e)) => Err(e),
        Err(e) => panic!("{e}"),
    }
}

fn historico(amb: &Ambiente, args: Value) -> Saida {
    chamar(&registro_padrao(), amb, "historico_do_trecho", args).unwrap()
}

#[test]
fn intervalo_de_linhas_chega_a_demanda_de_origem() {
    let (dir, [_, c2, c3]) = repositorio_do_estorno();
    let c = caso(dir, None);
    let s = historico(
        &c.amb,
        json!({ "arquivo": "db/pkg_pagamento.sql", "linha_inicial": 4, "linha_final": 7 }),
    );
    let d = s.dados.unwrap();
    assert_eq!(d["modo"], "trecho");
    assert_eq!(d["total"], 2, "{d}");
    let commits = d["commits"].as_array().unwrap();
    assert_eq!(commits[0]["hash"], c3.as_str());
    assert_eq!(commits[0]["chaves"], json!([]));
    let origem = &commits[1];
    assert_eq!(origem["hash"], c2.as_str());
    assert_eq!(origem["origem"], true);
    assert_eq!(origem["data"], "2024-03-12");
    assert_eq!(origem["assunto"], DEMANDA);
    assert_eq!(origem["chaves"], json!(["DEM-12"]));
    assert!(s.texto.contains("## Origem"), "{}", s.texto);
    assert!(s.texto.contains(DEMANDA), "{}", s.texto);
    assert!(s.texto.contains("Chaves de issue: DEM-12"), "{}", s.texto);
    assert!(s.texto.contains("continua"), "{}", s.texto);
    // O autor não sai da ferramenta.
    assert!(!format!("{}{d}", s.texto).contains(AUTOR));
}

#[test]
fn identificador_acha_o_commit_que_o_criou() {
    let (dir, [_, c2, _]) = repositorio_do_estorno();
    let c = caso(dir, None);
    for args in [
        json!({ "texto": "V_ESTORNO_TOTAL", "arquivo": "db/pkg_pagamento.sql" }),
        json!({ "texto": "V_ESTORNO_TOTAL" }),
    ] {
        let d = historico(&c.amb, args.clone()).dados.unwrap();
        assert_eq!(d["modo"], "texto", "{args}");
        assert_eq!(d["total"], 1, "{args}: {d}");
        assert_eq!(d["commits"][0]["hash"], c2.as_str(), "{args}");
        assert_eq!(d["commits"][0]["chaves"], json!(["DEM-12"]), "{args}");
    }
}

#[test]
fn sem_chave_nas_mensagens_devolve_os_commits_sem_inventar_chave() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-q"], None);
    commit(
        &repo,
        "2023-01-10",
        "Carga inicial",
        "",
        &[("db/a.sql", ESTORNO_V1)],
    );
    commit(
        &repo,
        "2023-05-02",
        "Trata o estorno total",
        "Leitura em UTF-8; troca &#39; por aspas.",
        &[("db/a.sql", ESTORNO_V2)],
    );
    commit(
        &repo,
        "2024-07-30",
        "Ajusta comentario",
        "",
        &[("db/a.sql", ESTORNO_V3)],
    );
    let c = caso(dir, None);
    let s = historico(
        &c.amb,
        json!({ "arquivo": "db/a.sql", "linha_inicial": 1, "linha_final": 8 }),
    );
    let d = s.dados.unwrap();
    let commits = d["commits"].as_array().unwrap();
    assert_eq!(commits.len(), 3, "{d}");
    // Nem a sigla com forma de chave ("UTF-8") nem a entidade HTML ("&#39;").
    for commit in commits {
        assert_eq!(commit["chaves"], json!([]), "{commit}");
    }
    assert!(s.texto.contains("nenhuma na mensagem"), "{}", s.texto);
    assert!(!s.texto.contains("Chaves de issue: UTF"), "{}", s.texto);
}

#[test]
fn saida_respeita_o_limite_e_avisa_do_corte() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-q"], None);
    let mut hashes = Vec::new();
    for i in 1..=6 {
        hashes.push(commit(
            &repo,
            &format!("2024-0{i}-01"),
            &format!("Ajusta o valor para {i}"),
            "",
            &[("db/parametros.sql", format!("VALOR := {i};\n").as_str())],
        ));
    }
    let c = caso(dir, None);
    let s = historico(
        &c.amb,
        json!({ "arquivo": "db/parametros.sql", "linha_inicial": 1, "linha_final": 1, "limite": 3 }),
    );
    let d = s.dados.unwrap();
    assert_eq!(d["total"], 6);
    assert_eq!(d["cortados"], 3);
    let mostrados: Vec<&str> = d["commits"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["hash"].as_str().unwrap())
        .collect();
    assert_eq!(
        mostrados,
        [&hashes[5], &hashes[4], &hashes[0]].map(String::as_str)
    );
    assert_eq!(d["commits"][2]["origem"], true);
    assert!(s.texto.contains("ficaram de fora os 3"), "{}", s.texto);
    assert!(s.texto.contains("`limite` maior"), "{}", s.texto);
    // Sem corte, sem aviso.
    let s = historico(
        &c.amb,
        json!({ "arquivo": "db/parametros.sql", "linha_inicial": 1, "linha_final": 1 }),
    );
    assert_eq!(s.dados.unwrap()["cortados"], 0);
    assert!(!s.texto.contains("ficaram de fora"), "{}", s.texto);
}

#[test]
fn pedidos_invalidos_e_arquivo_fora_do_git_sao_recusados() {
    let (dir, _) = repositorio_do_estorno();
    let c = caso(dir, None);
    let r = registro_padrao();
    let e = chamar(
        &r,
        &c.amb,
        "historico_do_trecho",
        json!({ "arquivo": "db/pkg_pagamento.sql", "linha_inicial": 50, "linha_final": 60 }),
    )
    .unwrap_err();
    assert!(
        matches!(&e, ErroFerramenta::Recusa(m) if m.contains("não pôde ser lido")),
        "{e:?}"
    );
    let e = chamar(
        &r,
        &c.amb,
        "historico_do_trecho",
        json!({ "arquivo": "db/nao-existe.sql", "linha_inicial": 1, "linha_final": 2 }),
    )
    .unwrap_err();
    assert!(matches!(e, ErroFerramenta::Recusa(_)), "{e:?}");
    let e = chamar(&r, &c.amb, "historico_do_trecho", json!({})).unwrap_err();
    assert!(matches!(e, ErroFerramenta::Argumentos(_)), "{e:?}");

    // Projeto fora do git.
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("repo/db")).unwrap();
    std::fs::write(dir.path().join("repo/db/a.sql"), ESTORNO_V1).unwrap();
    let c = caso(dir, None);
    let e = chamar(
        &r,
        &c.amb,
        "historico_do_trecho",
        json!({ "arquivo": "db/a.sql", "linha_inicial": 1, "linha_final": 2 }),
    )
    .unwrap_err();
    assert!(
        matches!(&e, ErroFerramenta::Recusa(m) if m.contains("não está num repositório git")),
        "{e:?}"
    );
}

fn script(dir: &Path, nome: &str, corpo: &str) -> PathBuf {
    let p = dir.join(nome);
    std::fs::write(&p, format!("#!/bin/sh\n{corpo}\n")).unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    p
}

#[test]
fn com_rastreador_mostra_o_titulo_da_issue_de_origem() {
    let (dir, _) = repositorio_do_estorno();
    let c = caso(
        dir,
        Some(TrackerConfig::Jira(JiraConfig {
            url: "https://jira.example.com".into(),
            project_key: "DEM".into(),
            credential_cmd: "echo tok-jira-ficticio".into(),
            ..Default::default()
        })),
    );
    let jira = script(
        &c.raiz,
        "jira",
        r#"[ "$3" = "DEM-12" ] || { echo "outra issue: $3" >&2; exit 2; }
cat <<'FIM'
{"key":"DEM-12","fields":{"summary":"Fatura em aberto no estorno do pagamento","description":"x",
"status":{"name":"Concluída"},"labels":[],"comment":{"total":0,"comments":[]}}}
FIM"#,
    );
    let mut r = Registro::new();
    r.registrar(historico_do_trecho_com(Ferramentas {
        jira,
        ..Default::default()
    }));
    let s = chamar(
        &r,
        &c.amb,
        "historico_do_trecho",
        json!({ "arquivo": "db/pkg_pagamento.sql", "linha_inicial": 4, "linha_final": 7 }),
    )
    .unwrap();
    assert!(
        s.texto
            .contains("DEM-12 (\"Fatura em aberto no estorno do pagamento\")"),
        "{}",
        s.texto
    );
    let d = s.dados.unwrap();
    assert_eq!(
        d["titulos"]["DEM-12"],
        "Fatura em aberto no estorno do pagamento"
    );
    assert!(!format!("{}{d}", s.texto).contains("tok-jira-ficticio"));

    // Rastreador fora do ar: os commits vêm, com o aviso.
    let jira = script(&c.raiz, "jira-fora", "echo 'no such host' >&2; exit 1");
    let mut r = Registro::new();
    r.registrar(historico_do_trecho_com(Ferramentas {
        jira,
        ..Default::default()
    }));
    let s = chamar(
        &r,
        &c.amb,
        "historico_do_trecho",
        json!({ "arquivo": "db/pkg_pagamento.sql", "linha_inicial": 4, "linha_final": 7 }),
    )
    .unwrap();
    assert!(
        s.texto.contains("títulos das issues não foram lidos"),
        "{}",
        s.texto
    );
    assert_eq!(s.dados.unwrap()["total"], 2);
}

#[test]
fn afirmar_guarda_o_commit_de_origem_e_o_documento_nao_cita_o_autor() {
    let (dir, [_, c2, _]) = repositorio_do_estorno();
    let c = caso(dir, None);
    let a = c
        .amb
        .analises
        .criar(NovaAnalise {
            fluxo: Fluxo::Impacto,
            titulo: "Baixar a fatura no estorno".into(),
            entrada: "Quando o pagamento for estornado, a fatura deve ficar paga.".into(),
            analista: Some("Ana Exemplo".into()),
            base: None,
        })
        .unwrap();
    let r = registro_padrao();
    let curto = &c2[..10];
    let s = chamar(
        &r,
        &c.amb,
        "afirmar",
        json!({
            "analise": a.id.to_string(),
            "secao": "como_e_hoje",
            "texto": "Pagamento estornado devolve a fatura para ABERTA, regra criada de propósito pela DEM-12.",
            "confianca": "confirmado",
            "evidencias": [{
                "arquivo": "db/pkg_pagamento.sql",
                "linha_inicial": 5,
                "linha_final": 7,
                "trecho": "IF V_ESTORNO_TOTAL THEN"
            }],
            "origens": [{ "commit": curto }]
        }),
    )
    .unwrap();
    assert_eq!(s.dados.unwrap()["origens"][0]["chaves"], json!(["DEM-12"]));
    let analise = c.amb.analises.carregar(&a.id).unwrap();
    let origem = &analise.afirmacoes[0].origens[0];
    assert_eq!(origem.commit, c2);
    assert_eq!(origem.data, "2024-03-12");
    assert_eq!(origem.assunto, DEMANDA);
    assert_eq!(origem.autor.as_deref(), Some(AUTOR));
    let documento = std::fs::read_to_string(c.amb.kb.caminho(&analise.documento)).unwrap();
    assert!(
        documento.contains(&format!(
            "(origem: commit {curto}, DEM-12)[^origem-{curto}]"
        )),
        "{documento}"
    );
    let nota = documento
        .lines()
        .find(|l| l.starts_with(&format!("[^origem-{curto}]:")))
        .unwrap_or_else(|| panic!("sem nota: {documento}"));
    assert!(
        nota.contains(&c2) && nota.contains("2024-03-12") && nota.contains(DEMANDA),
        "{nota}"
    );
    assert!(!documento.contains(AUTOR), "{documento}");

    // Commit que não existe: recusa.
    let e = chamar(
        &r,
        &c.amb,
        "afirmar",
        json!({
            "analise": a.id.to_string(),
            "secao": "como_e_hoje",
            "texto": "Outra regra.",
            "confianca": "inferido",
            "origens": [{ "commit": "0123456789abcdef" }]
        }),
    )
    .unwrap_err();
    assert!(
        matches!(&e, ErroFerramenta::Recusa(m) if m.contains("não foi encontrado")),
        "{e:?}"
    );
    // A origem sozinha não sustenta uma afirmação confirmada.
    let e = chamar(
        &r,
        &c.amb,
        "afirmar",
        json!({
            "analise": a.id.to_string(),
            "secao": "como_e_hoje",
            "texto": "Outra regra.",
            "confianca": "confirmado",
            "origens": [{ "commit": curto }]
        }),
    )
    .unwrap_err();
    assert!(matches!(e, ErroFerramenta::Recusa(_)), "{e:?}");
}
