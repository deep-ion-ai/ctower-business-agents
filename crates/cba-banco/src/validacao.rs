//! Validação da instrução antes de enviá-la ao banco: só uma, e só leitura.
//!
//! Usa o parser SQL do dialeto (não expressões regulares). Aceita uma única
//! instrução `SELECT` (ou `WITH … SELECT`, `VALUES`, `TABLE`, com `UNION` e
//! subconsultas) e recusa:
//!
//! - mais de uma instrução, inclusive escondida num comentário;
//! - escrita e DDL (`INSERT`, `UPDATE`, `DELETE`, `MERGE`, `CREATE`…), `CALL`,
//!   `SET`, `LOCK`, inclusive dentro de um `WITH` (`WITH x AS (DELETE …)`);
//! - `SELECT … INTO` (cria tabela no PostgreSQL, grava arquivo no MySQL);
//! - travas (`FOR UPDATE`, `FOR SHARE`, `LOCK IN SHARE MODE`);
//! - funções com efeito fora da consulta (ler arquivo do servidor, travas
//!   consultivas, sequências, encerrar sessões, consulta dinâmica);
//! - comentários executáveis do MySQL (`/*! … */`) e comentários com `;`.
//!
//! É uma camada a mais: a sessão também é só de leitura no próprio banco
//! (`crate::sessao`), e o usuário da conexão deve ter só leitura.

use std::ops::ControlFlow;

use cba_repos::providers::DialetoSql;
use sqlparser::ast::{
    Expr, ObjectName, ObjectNamePart, Query, Select, SetExpr, Statement, TableFactor, Visit,
    Visitor,
};
use sqlparser::dialect::{Dialect, MySqlDialect, PostgreSqlDialect};
use sqlparser::parser::Parser;
use sqlparser::tokenizer::{Token, Tokenizer, Whitespace};

/// Por que a instrução foi recusada, para o agente corrigir.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct Recusa(pub String);

fn recusa(texto: impl Into<String>) -> Recusa {
    Recusa(texto.into())
}

/// Funções recusadas em qualquer posição, pelo último nome (sem o esquema), em
/// minúsculas. Têm efeito fora da consulta ou executam SQL dinâmico.
const FUNCOES_RECUSADAS: &[&str] = &[
    // PostgreSQL: arquivos do servidor e objetos grandes.
    "pg_read_file",
    "pg_read_binary_file",
    "pg_ls_dir",
    "pg_stat_file",
    "pg_ls_logdir",
    "pg_ls_waldir",
    "pg_ls_tmpdir",
    "lo_import",
    "lo_export",
    "lo_from_bytea",
    "lo_put",
    "lo_unlink",
    // PostgreSQL: outras sessões, configuração e servidor.
    "pg_terminate_backend",
    "pg_cancel_backend",
    "pg_reload_conf",
    "pg_rotate_logfile",
    "set_config",
    "pg_notify",
    "pg_create_restore_point",
    "pg_switch_wal",
    "pg_promote",
    "pg_logical_emit_message",
    "pg_sleep",
    "pg_sleep_for",
    "pg_sleep_until",
    // PostgreSQL: sequências.
    "nextval",
    "setval",
    // PostgreSQL: SQL dinâmico e outros servidores.
    "query_to_xml",
    "query_to_xml_and_xmlschema",
    "query_to_xmlschema",
    "cursor_to_xml",
    "dblink",
    "dblink_exec",
    "dblink_connect",
    "dblink_connect_u",
    "dblink_send_query",
    "dblink_open",
    // MySQL/MariaDB.
    "load_file",
    "get_lock",
    "release_lock",
    "release_all_locks",
    "sleep",
    "benchmark",
    "sys_exec",
    "sys_eval",
];

/// Prefixos recusados (travas consultivas do PostgreSQL).
const PREFIXOS_RECUSADOS: &[&str] = &["pg_advisory_", "pg_try_advisory_"];

fn dialeto(d: DialetoSql) -> Box<dyn Dialect> {
    match d {
        DialetoSql::Postgres => Box::new(PostgreSqlDialect {}),
        DialetoSql::Mysql => Box::new(MySqlDialect {}),
    }
}

/// Confere `sql` no dialeto `d`. Devolve a instrução sem espaços nas pontas e
/// sem o `;` final, pronta para enviar.
pub fn validar(d: DialetoSql, sql: &str) -> Result<String, Recusa> {
    let dialeto = dialeto(d);
    let texto = sql.trim();
    if texto.is_empty() {
        return Err(recusa("a consulta está vazia"));
    }
    conferir_comentarios(dialeto.as_ref(), texto)?;
    let instrucoes = Parser::parse_sql(dialeto.as_ref(), texto)
        .map_err(|e| recusa(format!("a consulta não é SQL válido: {e}")))?;
    let instrucao = match instrucoes.as_slice() {
        [] => return Err(recusa("a consulta está vazia")),
        [uma] => uma,
        _ => {
            return Err(recusa(format!(
                "envie uma instrução por vez; vieram {}",
                instrucoes.len()
            )));
        }
    };
    let Statement::Query(_) = instrucao else {
        return Err(recusa(format!(
            "só consultas de leitura (SELECT ou WITH … SELECT) são aceitas; recebida: {}",
            tipo_da_instrucao(instrucao)
        )));
    };
    let mut fiscal = Fiscal { instrucoes: 0 };
    if let ControlFlow::Break(r) = instrucao.visit(&mut fiscal) {
        return Err(r);
    }
    Ok(texto.trim_end_matches(';').trim_end().to_owned())
}

/// Recusa comentários executáveis do MySQL e comentários com `;`, que poderiam
/// esconder outra instrução de quem lê a consulta.
fn conferir_comentarios(dialeto: &dyn Dialect, sql: &str) -> Result<(), Recusa> {
    // `/*!` e `/*M!` (MariaDB) são executados pelo servidor MySQL. Procura no
    // texto todo, além dos tokens: o tokenizador pode tratá-los como código.
    let tokens = Tokenizer::new(dialeto, sql)
        .tokenize()
        .map_err(|e| recusa(format!("a consulta não é SQL válido: {e}")))?;
    let mut pontos_e_virgulas = 0usize;
    for t in &tokens {
        match t {
            Token::Whitespace(Whitespace::MultiLineComment(c)) => {
                let c = c.trim_start();
                if c.starts_with('!') || c.starts_with("M!") {
                    return Err(recusa(
                        "comentários executáveis (/*! … */) não são aceitos; tire o comentário",
                    ));
                }
                if c.contains(';') {
                    return Err(recusa(
                        "comentários com `;` não são aceitos; tire o comentário",
                    ));
                }
            }
            Token::Whitespace(Whitespace::SingleLineComment { comment, .. })
                if comment.contains(';') =>
            {
                return Err(recusa(
                    "comentários com `;` não são aceitos; tire o comentário",
                ));
            }
            Token::SemiColon => pontos_e_virgulas += 1,
            _ => {}
        }
    }
    // Um `;` só no fim; mais do que isso é outra instrução (o parser confere de
    // novo, mas uma instrução vazia entre dois `;` passaria).
    let fim_com_ponto_e_virgula = tokens
        .iter()
        .rev()
        .find(|t| !matches!(t, Token::Whitespace(_)))
        .is_some_and(|t| *t == Token::SemiColon);
    if pontos_e_virgulas > usize::from(fim_com_ponto_e_virgula) {
        return Err(recusa("envie uma instrução por vez, sem `;` no meio"));
    }
    if sql.contains("/*!") || sql.contains("/*M!") {
        return Err(recusa(
            "comentários executáveis (/*! … */) não são aceitos; tire o comentário",
        ));
    }
    Ok(())
}

/// Percorre a árvore da consulta recusando o que não é leitura.
struct Fiscal {
    instrucoes: usize,
}

impl Visitor for Fiscal {
    type Break = Recusa;

    fn pre_visit_statement(&mut self, s: &Statement) -> ControlFlow<Recusa> {
        self.instrucoes += 1;
        // A primeira é a consulta; qualquer outra está dentro dela (um `DELETE`
        // num `WITH`, um `INSERT` numa subconsulta).
        if self.instrucoes > 1 {
            return ControlFlow::Break(recusa(format!(
                "a consulta contém uma instrução que não é leitura: {}",
                tipo_da_instrucao(s)
            )));
        }
        ControlFlow::Continue(())
    }

    fn pre_visit_query(&mut self, q: &Query) -> ControlFlow<Recusa> {
        if !q.locks.is_empty() {
            return ControlFlow::Break(recusa(
                "travas (FOR UPDATE, FOR SHARE) não são aceitas numa consulta de leitura",
            ));
        }
        if let SetExpr::Insert(_) | SetExpr::Update(_) | SetExpr::Delete(_) | SetExpr::Merge(_) =
            q.body.as_ref()
        {
            return ControlFlow::Break(recusa(
                "a consulta contém uma escrita (INSERT, UPDATE, DELETE ou MERGE)",
            ));
        }
        ControlFlow::Continue(())
    }

    fn pre_visit_select(&mut self, s: &Select) -> ControlFlow<Recusa> {
        if s.into.is_some() {
            return ControlFlow::Break(recusa(
                "SELECT … INTO não é aceito: ele cria uma tabela ou grava um arquivo",
            ));
        }
        ControlFlow::Continue(())
    }

    fn pre_visit_expr(&mut self, e: &Expr) -> ControlFlow<Recusa> {
        if let Expr::Function(f) = e {
            return conferir_funcao(&f.name);
        }
        ControlFlow::Continue(())
    }

    fn pre_visit_table_factor(&mut self, t: &TableFactor) -> ControlFlow<Recusa> {
        match t {
            TableFactor::Table {
                name,
                args: Some(_),
                ..
            }
            | TableFactor::Function { name, .. } => conferir_funcao(name),
            _ => ControlFlow::Continue(()),
        }
    }
}

fn conferir_funcao(nome: &ObjectName) -> ControlFlow<Recusa> {
    let Some(ObjectNamePart::Identifier(ultimo)) = nome.0.last() else {
        return ControlFlow::Continue(());
    };
    let f = ultimo.value.to_lowercase();
    if FUNCOES_RECUSADAS.contains(&f.as_str())
        || PREFIXOS_RECUSADOS.iter().any(|p| f.starts_with(p))
    {
        return ControlFlow::Break(recusa(format!(
            "a função `{f}` não é aceita: ela tem efeito fora da consulta ou executa outra \
             instrução"
        )));
    }
    ControlFlow::Continue(())
}

/// Nome curto da instrução, para a mensagem ("INSERT", "CREATE TABLE").
fn tipo_da_instrucao(s: &Statement) -> String {
    let texto = s.to_string();
    let palavras: Vec<&str> = texto.split_whitespace().take(2).collect();
    match palavras.as_slice() {
        [a, b]
            if matches!(
                a.to_uppercase().as_str(),
                "CREATE" | "DROP" | "ALTER" | "TRUNCATE"
            ) =>
        {
            format!("{} {}", a.to_uppercase(), b.to_uppercase())
        }
        [a, ..] => a.to_uppercase(),
        [] => "instrução vazia".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PG: DialetoSql = DialetoSql::Postgres;
    const MY: DialetoSql = DialetoSql::Mysql;

    fn aceita(d: DialetoSql, sql: &str) {
        if let Err(e) = validar(d, sql) {
            panic!("{d:?} deveria aceitar {sql:?}: {e}");
        }
    }

    fn recusa_com(d: DialetoSql, sql: &str, trecho: &str) {
        match validar(d, sql) {
            Ok(_) => panic!("{d:?} deveria recusar {sql:?}"),
            Err(Recusa(e)) => assert!(e.contains(trecho), "{sql:?}: {e}"),
        }
    }

    #[test]
    fn aceita_consultas_de_leitura() {
        for d in [PG, MY] {
            aceita(d, "SELECT 1");
            aceita(d, "select count(*) from fatura where situacao = 'ABERTA';");
            aceita(
                d,
                "WITH abertas AS (SELECT id FROM fatura WHERE situacao = 'A') \
                 SELECT count(*) FROM abertas",
            );
            aceita(
                d,
                "SELECT c.uf, count(*) FROM cliente c JOIN fatura f ON f.cliente_id = c.id \
                 GROUP BY c.uf HAVING count(*) > 10 ORDER BY 2 DESC LIMIT 20",
            );
            aceita(d, "SELECT a FROM t UNION ALL SELECT b FROM u");
            aceita(
                d,
                "SELECT * FROM t WHERE id IN (SELECT id FROM u) -- comentário",
            );
            aceita(
                d,
                "/* contagem por situação */ SELECT situacao, count(*) FROM t GROUP BY 1",
            );
        }
        aceita(PG, "SELECT * FROM generate_series(1, 3)");
        aceita(
            PG,
            "SELECT now() - interval '1 day', 'texto; com ponto e vírgula'",
        );
        aceita(MY, "SELECT `nome` FROM `cliente` WHERE `id` = 1");
    }

    #[test]
    fn devolve_sem_ponto_e_virgula_final() {
        assert_eq!(validar(PG, "  SELECT 1 ;  ").unwrap(), "SELECT 1");
    }

    #[test]
    fn recusa_escrita_e_ddl() {
        for d in [PG, MY] {
            recusa_com(d, "INSERT INTO t VALUES (1)", "INSERT");
            recusa_com(d, "UPDATE t SET a = 1", "UPDATE");
            recusa_com(d, "DELETE FROM t", "DELETE");
            recusa_com(d, "DROP TABLE t", "DROP TABLE");
            recusa_com(d, "CREATE TABLE x (a int)", "CREATE TABLE");
            recusa_com(d, "ALTER TABLE t ADD COLUMN b int", "ALTER TABLE");
            recusa_com(d, "TRUNCATE TABLE t", "TRUNCATE");
            recusa_com(d, "CALL limpar()", "CALL");
            recusa_com(d, "GRANT SELECT ON t TO u", "GRANT");
            recusa_com(d, "", "vazia");
            recusa_com(d, "   ;  ", "");
        }
        recusa_com(PG, "SET default_transaction_read_only = off", "SET");
        recusa_com(MY, "SET SESSION TRANSACTION READ WRITE", "");
        recusa_com(PG, "COPY t TO '/tmp/x'", "COPY");
        recusa_com(PG, "LOCK TABLE t", "");
        recusa_com(MY, "LOCK TABLES t READ", "");
    }

    #[test]
    fn recusa_multiplas_instrucoes_e_injecao() {
        for d in [PG, MY] {
            recusa_com(d, "SELECT 1; DELETE FROM t", "uma instrução por vez");
            recusa_com(d, "SELECT 1; SELECT 2", "uma instrução por vez");
            recusa_com(d, "SELECT 1;; ", "uma instrução por vez");
            recusa_com(
                d,
                "SELECT * FROM t WHERE nome = '' ; DROP TABLE t; -- '",
                "",
            );
            recusa_com(d, "SELECT 1 /* ; DROP TABLE t */", "`;`");
            recusa_com(d, "SELECT 1 -- ; DROP TABLE t", "`;`");
        }
        recusa_com(MY, "SELECT 1 /*!50000 ; DROP TABLE t */", "");
        recusa_com(
            MY,
            "SELECT 1 /*! UNION SELECT senha FROM usuario */",
            "executáveis",
        );
        recusa_com(MY, "SELECT 1 /*M!100000 , 2 */", "executáveis");
    }

    #[test]
    fn recusa_escrita_escondida_na_consulta() {
        recusa_com(
            PG,
            "WITH apagadas AS (DELETE FROM t RETURNING *) SELECT count(*) FROM apagadas",
            "",
        );
        recusa_com(
            PG,
            "WITH x AS (INSERT INTO t VALUES (1) RETURNING id) SELECT * FROM x",
            "",
        );
        recusa_com(
            PG,
            "WITH x AS (UPDATE t SET a = 1 RETURNING id) SELECT * FROM x",
            "",
        );
    }

    #[test]
    fn recusa_into_e_travas() {
        recusa_com(PG, "SELECT * INTO copia FROM t", "INTO");
        recusa_com(MY, "SELECT a INTO @v FROM t", "INTO");
        recusa_com(MY, "SELECT * FROM t INTO OUTFILE '/tmp/t.csv'", "");
        for d in [PG, MY] {
            recusa_com(d, "SELECT * FROM t FOR UPDATE", "travas");
            recusa_com(d, "SELECT * FROM t WHERE id = 1 FOR SHARE", "travas");
        }
        recusa_com(MY, "SELECT * FROM t LOCK IN SHARE MODE", "");
    }

    #[test]
    fn recusa_funcoes_com_efeito() {
        recusa_com(PG, "SELECT pg_read_file('/etc/passwd')", "pg_read_file");
        recusa_com(
            PG,
            "SELECT pg_catalog.pg_read_file('/etc/passwd')",
            "pg_read_file",
        );
        recusa_com(PG, "SELECT nextval('seq_fatura')", "nextval");
        recusa_com(PG, "SELECT pg_advisory_lock(1)", "pg_advisory_lock");
        recusa_com(
            PG,
            "SELECT pg_terminate_backend(123)",
            "pg_terminate_backend",
        );
        recusa_com(PG, "SELECT set_config('x', 'y', false)", "set_config");
        recusa_com(
            PG,
            "SELECT query_to_xml('DELETE FROM t', true, false, '')",
            "query_to_xml",
        );
        recusa_com(
            PG,
            "SELECT * FROM dblink('host=x', 'DELETE FROM t') AS r(a int)",
            "dblink",
        );
        recusa_com(
            PG,
            "SELECT a FROM t WHERE b = (SELECT pg_sleep(100))",
            "pg_sleep",
        );
        recusa_com(MY, "SELECT LOAD_FILE('/etc/passwd')", "load_file");
        recusa_com(MY, "SELECT GET_LOCK('x', 10)", "get_lock");
        recusa_com(MY, "SELECT SLEEP(100)", "sleep");
        recusa_com(MY, "SELECT BENCHMARK(1000000, MD5('a'))", "benchmark");
    }
}
