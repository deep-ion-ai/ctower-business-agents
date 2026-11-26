//! Testes de integração com PostgreSQL, MySQL e MariaDB em contêiner (#439).
//!
//! Precisam do Docker, por isso ficam de fora do `cargo test` comum:
//!
//! ```sh
//! cargo test -p cba-banco --test conteiner -- --ignored --test-threads=1
//! ```
//!
//! A conexão usa o superusuário do contêiner de propósito: a só leitura tem de
//! vir do app (validação e sessão), não das permissões do usuário.

use std::collections::BTreeMap;
use std::process::Command;
use std::time::{Duration, Instant};

use cba_banco::{ConfigBanco, ErroBanco, consultar, ler_schema, testar};
use cba_repos::Secret;
use cba_repos::providers::{Conexao, FonteCredencial};
use serde_json::json;

const SENHA: &str = "senha-do-conteiner-439";

/// Um contêiner que é removido no fim do teste.
struct Conteiner {
    id: String,
    porta: u16,
}

impl Conteiner {
    fn iniciar(imagem: &str, porta_interna: u16, env: &[(&str, &str)], args: &[&str]) -> Self {
        let mut cmd = Command::new("docker");
        cmd.args(["run", "-d", "--rm", "-p"])
            .arg(format!("127.0.0.1::{porta_interna}"));
        for (k, v) in env {
            cmd.arg("-e").arg(format!("{k}={v}"));
        }
        cmd.arg(imagem).args(args);
        let saida = cmd.output().expect("docker instalado");
        assert!(
            saida.status.success(),
            "docker run: {}",
            String::from_utf8_lossy(&saida.stderr)
        );
        let id = String::from_utf8_lossy(&saida.stdout).trim().to_owned();
        let porta = Command::new("docker")
            .args(["port", &id, &porta_interna.to_string()])
            .output()
            .expect("docker port");
        let porta = String::from_utf8_lossy(&porta.stdout)
            .lines()
            .next()
            .and_then(|l| l.rsplit(':').next())
            .and_then(|p| p.trim().parse().ok())
            .expect("porta publicada");
        Self { id, porta }
    }

    /// Roda um comando dentro do contêiner, uma vez.
    fn executar(&self, args: &[&str]) {
        let saida = Command::new("docker")
            .arg("exec")
            .arg(&self.id)
            .args(args)
            .output()
            .expect("docker exec");
        assert!(
            saida.status.success(),
            "docker exec {args:?}: {}",
            String::from_utf8_lossy(&saida.stderr)
        );
    }

    /// Repete o comando até dar certo (o servidor ainda pode estar subindo).
    fn esperar(&self, args: &[&str]) {
        let inicio = Instant::now();
        loop {
            let saida = Command::new("docker")
                .arg("exec")
                .arg(&self.id)
                .args(args)
                .output()
                .expect("docker exec");
            if saida.status.success() {
                return;
            }
            assert!(
                inicio.elapsed() < Duration::from_secs(120),
                "docker exec {args:?}: {}",
                String::from_utf8_lossy(&saida.stderr)
            );
            std::thread::sleep(Duration::from_secs(1));
        }
    }
}

impl Drop for Conteiner {
    fn drop(&mut self) {
        let _ = Command::new("docker").args(["rm", "-f", &self.id]).output();
    }
}

fn config(
    tipo: &str,
    porta: u16,
    banco: &str,
    usuario: &str,
    opcoes: &[(&str, &str)],
) -> ConfigBanco {
    let mut o = BTreeMap::new();
    o.insert("usuario".to_owned(), usuario.to_owned());
    o.insert("tls".to_owned(), "desligar".to_owned());
    o.insert("ambiente".to_owned(), "homologação".to_owned());
    for (k, v) in opcoes {
        o.insert((*k).to_owned(), (*v).to_owned());
    }
    let conexao = Conexao {
        id: format!("{tipo}-teste"),
        nome: format!("Banco de teste {tipo}"),
        tipo: tipo.to_owned(),
        endereco: format!("127.0.0.1:{porta}/{banco}"),
        credencial: FonteCredencial::Nenhuma,
        opcoes: o,
    };
    ConfigBanco::da_conexao(&conexao, Some(Secret::new(SENHA))).unwrap()
}

/// Espera o servidor aceitar conexões pelo próprio app.
fn esperar(cfg: &ConfigBanco) {
    let inicio = Instant::now();
    while consultar(cfg, "SELECT 1").is_err() {
        assert!(
            inicio.elapsed() < Duration::from_secs(120),
            "servidor não subiu"
        );
        std::thread::sleep(Duration::from_secs(1));
    }
}

/// O que vale para os três servidores.
fn conferir(cfg: &ConfigBanco, rotulo_versao: &str, consulta_lenta: &str, sql_que_escreve: &str) {
    // Prontidão: os quatro passos, em ordem.
    let passos = testar(cfg);
    let ids: Vec<&str> = passos.iter().map(|p| p.id).collect();
    assert_eq!(
        ids,
        ["conectar", "versao", "so_leitura", "esquemas"],
        "{passos:#?}"
    );
    assert!(passos.iter().all(|p| p.ok), "{passos:#?}");
    assert!(passos[1].detalhe.starts_with(rotulo_versao), "{passos:#?}");
    assert!(
        passos[2].detalhe.contains("recusada pelo banco"),
        "{passos:#?}"
    );
    assert!(passos[3].detalhe.contains("app"), "{passos:#?}");

    // Consulta e limite de linhas (o cfg tem limite 10).
    let r = consultar(cfg, "SELECT count(*) AS total FROM app.cliente").unwrap();
    assert_eq!(r.linhas, [[json!(25)]]);
    assert!(!r.cortada);
    let r = consultar(cfg, "SELECT id, nome FROM app.cliente ORDER BY id").unwrap();
    assert_eq!(r.linhas.len(), 10);
    assert!(r.cortada);
    assert_eq!(r.colunas[0].nome, "id");
    assert_eq!(r.linhas[0], [json!(1), json!("Cliente 1")]);

    // A escrita é recusada pelo app...
    let e = consultar(cfg, "DELETE FROM app.cliente").unwrap_err();
    assert!(matches!(e, ErroBanco::Recusada(_)), "{e}");
    let e = consultar(cfg, "SELECT 1; DELETE FROM app.cliente").unwrap_err();
    assert!(matches!(e, ErroBanco::Recusada(_)), "{e}");
    // ...e, quando passa pela validação (função que grava), pelo banco.
    let e = consultar(cfg, sql_que_escreve).unwrap_err();
    assert!(matches!(e, ErroBanco::Banco(_)), "{e}");
    assert!(e.to_string().to_lowercase().contains("read"), "{e}");
    let r = consultar(cfg, "SELECT count(*) FROM app.registro").unwrap();
    assert_eq!(r.linhas, [[json!(0)]], "a escrita não pode ter ficado");

    // Tempo limite (o cfg tem 1 s).
    let inicio = Instant::now();
    let e = consultar(cfg, consulta_lenta).unwrap_err();
    assert!(matches!(e, ErroBanco::TempoEsgotado(1)), "{e}");
    assert!(inicio.elapsed() < Duration::from_secs(10));

    // Schema real.
    let s = ler_schema(cfg).unwrap();
    let cliente = s
        .tabelas
        .iter()
        .find(|t| t.qualificado() == "app.cliente")
        .expect("app.cliente");
    let nomes: Vec<&str> = cliente.colunas.iter().map(|c| c.nome.as_str()).collect();
    assert_eq!(nomes, ["id", "nome", "cpf", "email"]);
    assert_eq!(cliente.chave_primaria, ["id"]);
    assert!(
        cliente.colunas[2].tipo.contains("char"),
        "{:?}",
        cliente.colunas[2]
    );
    let fatura = s.tabelas.iter().find(|t| t.nome == "fatura").unwrap();
    assert_eq!(fatura.chaves_estrangeiras.len(), 1);
    assert_eq!(fatura.chaves_estrangeiras[0].referencia, "app.cliente");
    assert_eq!(fatura.chaves_estrangeiras[0].colunas, ["cliente_id"]);
    assert_eq!(fatura.chaves_estrangeiras[0].colunas_referencia, ["id"]);
}

/// A senha errada falha sem aparecer na mensagem.
fn senha_fora_das_mensagens(tipo: &str, porta: u16, banco: &str, usuario: &str) {
    let mut o = BTreeMap::new();
    o.insert("usuario".to_owned(), usuario.to_owned());
    o.insert("tls".to_owned(), "desligar".to_owned());
    let conexao = Conexao {
        id: "errada".into(),
        nome: "Senha errada".into(),
        tipo: tipo.into(),
        endereco: format!("127.0.0.1:{porta}/{banco}"),
        credencial: FonteCredencial::Nenhuma,
        opcoes: o,
    };
    let errada = "senha-errada-que-nao-pode-vazar";
    let cfg = ConfigBanco::da_conexao(&conexao, Some(Secret::new(errada))).unwrap();
    let e = consultar(&cfg, "SELECT 1").unwrap_err();
    assert!(matches!(e, ErroBanco::Conexao(_)), "{e}");
    let passos = testar(&cfg);
    let tudo = format!("{e} {e:?} {cfg:?} {passos:?}");
    assert!(!tudo.contains(errada), "{tudo}");
    assert!(!tudo.contains(SENHA), "{tudo}");
    assert!(!passos[0].ok);
}

#[test]
#[ignore = "precisa do Docker"]
fn postgres() {
    let c = Conteiner::iniciar(
        "postgres:16-alpine",
        5432,
        &[("POSTGRES_PASSWORD", SENHA), ("POSTGRES_DB", "loja")],
        &[],
    );
    let cfg = config(
        "postgres",
        c.porta,
        "loja",
        "postgres",
        &[("tempo_limite_s", "1"), ("limite_linhas", "10")],
    );
    esperar(&cfg);
    c.esperar(&["pg_isready", "-U", "postgres"]);
    c.executar(&[
        "psql", "-v", "ON_ERROR_STOP=1", "-U", "postgres", "-d", "loja", "-c",
        "CREATE SCHEMA app;
         CREATE TABLE app.cliente (id int PRIMARY KEY, nome text, cpf char(11), email text);
         CREATE TABLE app.fatura (id int PRIMARY KEY, cliente_id int REFERENCES app.cliente (id), valor numeric(10, 2));
         CREATE TABLE app.registro (a int);
         INSERT INTO app.cliente SELECT g, 'Cliente ' || g, lpad(g::text, 11, '0'), 'c' || g || '@example.com' FROM generate_series(1, 25) g;
         CREATE FUNCTION app.grava() RETURNS int LANGUAGE sql AS $$ INSERT INTO app.registro VALUES (1) RETURNING 1 $$;",
    ]);
    conferir(
        &cfg,
        "PostgreSQL 16",
        "SELECT count(*) FROM generate_series(1, 100000) a, generate_series(1, 100000) b",
        "SELECT app.grava()",
    );
    senha_fora_das_mensagens("postgres", c.porta, "loja", "postgres");
}

fn mysql_ou_mariadb(imagem: &str, cliente: &str, rotulo: &str) {
    let c = Conteiner::iniciar(
        imagem,
        3306,
        &[
            ("MYSQL_ROOT_PASSWORD", SENHA),
            ("MARIADB_ROOT_PASSWORD", SENHA),
            ("MYSQL_DATABASE", "app"),
            ("MARIADB_DATABASE", "app"),
        ],
        &[],
    );
    let cfg = config(
        "mysql",
        c.porta,
        "app",
        "root",
        &[("tempo_limite_s", "1"), ("limite_linhas", "10")],
    );
    esperar(&cfg);
    let senha = format!("-p{SENHA}");
    c.esperar(&[cliente, "-uroot", &senha, "app", "-e", "SELECT 1"]);
    c.executar(&[
        cliente, "-uroot", &senha, "app", "-e",
        "SET GLOBAL log_bin_trust_function_creators = 1;
         CREATE TABLE cliente (id int PRIMARY KEY, nome text, cpf char(11), email text);
         CREATE TABLE fatura (id int PRIMARY KEY, cliente_id int, valor decimal(10, 2),
                              CONSTRAINT fk_fatura_cliente FOREIGN KEY (cliente_id) REFERENCES cliente (id));
         CREATE TABLE registro (a int);
         INSERT INTO cliente
           WITH RECURSIVE g(n) AS (SELECT 1 UNION ALL SELECT n + 1 FROM g WHERE n < 25)
           SELECT n, CONCAT('Cliente ', n), LPAD(n, 11, '0'), CONCAT('c', n, '@example.com') FROM g;
         DELIMITER //
         CREATE FUNCTION grava() RETURNS int DETERMINISTIC MODIFIES SQL DATA
           BEGIN INSERT INTO registro VALUES (1); RETURN 1; END//",
    ]);
    conferir(
        &cfg,
        rotulo,
        "SELECT count(*) FROM information_schema.COLUMNS a, information_schema.COLUMNS b, \
         information_schema.COLUMNS c",
        "SELECT app.grava()",
    );
    senha_fora_das_mensagens("mysql", c.porta, "app", "root");
}

#[test]
#[ignore = "precisa do Docker"]
fn mysql() {
    mysql_ou_mariadb("mysql:8.4", "mysql", "MySQL 8.4");
}

#[test]
#[ignore = "precisa do Docker"]
fn mariadb() {
    mysql_ou_mariadb("mariadb:11.4", "mariadb", "MariaDB 11.4");
}
