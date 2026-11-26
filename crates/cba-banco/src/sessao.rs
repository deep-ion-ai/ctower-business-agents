//! Sessão só de leitura com o banco: abertura, consulta e prontidão.
//!
//! Cada operação abre uma conexão própria e a fecha no fim (sem pool): as
//! consultas de uma análise são poucas e espaçadas. A API é síncrona; o
//! trabalho assíncrono do driver roda numa thread com um runtime próprio, para
//! poder ser chamado de dentro de outro runtime (servidor MCP, janela).

use std::time::{Duration, Instant};

use cba_repos::providers::DialetoSql;
use futures_util::TryStreamExt;
use serde::Serialize;
use serde_json::Value;
use sqlx::mysql::{MySqlConnectOptions, MySqlConnection, MySqlSslMode};
use sqlx::postgres::{PgConnectOptions, PgConnection, PgSslMode};
use sqlx::{AssertSqlSafe, Column, Connection, Row, TypeInfo, ValueRef, raw_sql};

use crate::{ConfigBanco, ErroBanco, PRAZO_CONEXAO, ResultadoPasso, Tls, validacao};

/// Texto maior que isso numa célula é cortado, com a marca do corte.
const MAX_CARACTERES_CELULA: usize = 2000;

/// Folga do prazo do app sobre o tempo limite do banco: o servidor cancela
/// primeiro e devolve o erro dele; o app só corta se o servidor não responder.
const FOLGA_PRAZO: Duration = Duration::from_secs(5);

/// Uma coluna do resultado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ColunaResultado {
    pub nome: String,
    /// Tipo como o driver o nomeia (`INT8`, `VARCHAR`).
    pub tipo: String,
}

/// Resultado de uma consulta.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ResultadoConsulta {
    /// A instrução enviada, já validada.
    pub sql: String,
    pub colunas: Vec<ColunaResultado>,
    /// Valores na ordem das colunas: número, booleano, texto ou `null`.
    pub linhas: Vec<Vec<Value>>,
    /// Havia mais linhas que o limite; só as primeiras vieram.
    pub cortada: bool,
    pub limite_linhas: usize,
    pub duracao_ms: u64,
}

/// Roda `trabalho` num runtime próprio, numa thread separada.
pub(crate) fn bloquear<T: Send + 'static>(
    trabalho: impl Future<Output = T> + Send + 'static,
) -> Result<T, ErroBanco> {
    std::thread::spawn(move || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map(|rt| rt.block_on(trabalho))
    })
    .join()
    .map_err(|_| ErroBanco::Conexao("falha interna no acesso ao banco".into()))?
    .map_err(|e| ErroBanco::Conexao(format!("falha interna no acesso ao banco: {e}")))
}

/// Uma sessão aberta, já em só leitura.
pub(crate) enum Sessao {
    Pg(PgConnection),
    My { con: MySqlConnection, mariadb: bool },
}

/// Erro do driver como [`ErroBanco`], com a mensagem mascarada.
pub(crate) fn erro_driver(cfg: &ConfigBanco, e: sqlx::Error, conectando: bool) -> ErroBanco {
    let texto = match &e {
        sqlx::Error::Database(d) => match d.code() {
            Some(c) => format!("{} (código {c})", d.message()),
            None => d.message().to_owned(),
        },
        outro => outro.to_string(),
    };
    let texto = cfg.mascarar(&texto);
    // Tempo limite do servidor: 57014 (PostgreSQL), 3024 (MySQL, estado HY000),
    // 1969 (MariaDB, estado 70100).
    if let sqlx::Error::Database(d) = &e {
        let estado = d.code().map(|c| c.into_owned()).unwrap_or_default();
        let msg = d.message().to_lowercase();
        if matches!(estado.as_str(), "57014" | "70100")
            || msg.contains("maximum statement execution time")
            || msg.contains("max_statement_time")
        {
            return ErroBanco::TempoEsgotado(cfg.tempo_limite.as_secs());
        }
    }
    if conectando || !matches!(e, sqlx::Error::Database(_)) {
        ErroBanco::Conexao(texto)
    } else {
        ErroBanco::Banco(texto)
    }
}

/// Expande `corpo` para cada tipo de conexão, com `con` ligado à conexão
/// concreta: o driver não tem um executor genérico simples entre os dois.
macro_rules! em_cada {
    ($sessao:expr, $con:ident => $corpo:expr) => {
        match $sessao {
            Sessao::Pg($con) => $corpo,
            Sessao::My { con: $con, .. } => $corpo,
        }
    };
}

/// A primeira coluna da primeira linha, como texto.
macro_rules! texto_unico {
    ($con:expr, $sql:expr) => {
        raw_sql($sql)
            .fetch_one(&mut *$con)
            .await
            .and_then(|l| l.try_get_unchecked::<String, _>(0usize))
    };
}

/// Todas as linhas, cada célula como texto (`None` para `NULL`).
macro_rules! linhas_texto {
    ($con:expr, $sql:expr) => {
        raw_sql(AssertSqlSafe($sql.to_owned()))
            .fetch_all(&mut *$con)
            .await
            .map(|linhas| {
                linhas
                    .iter()
                    .map(|l| (0..l.len()).map(|i| celula_texto(l, i)).collect::<Vec<_>>())
                    .collect::<Vec<_>>()
            })
    };
}

/// Lê no máximo `limite` linhas; a seguinte, se houver, só marca o corte.
macro_rules! ler_linhas {
    ($con:expr, $sql:expr, $limite:expr) => {
        async {
            let mut colunas: Vec<ColunaResultado> = Vec::new();
            let mut linhas: Vec<Vec<Value>> = Vec::new();
            let mut cortada = false;
            let mut fluxo = raw_sql(AssertSqlSafe($sql.to_owned())).fetch(&mut *$con);
            while let Some(linha) = fluxo.try_next().await? {
                if colunas.is_empty() {
                    colunas = linha
                        .columns()
                        .iter()
                        .map(|c| ColunaResultado {
                            nome: c.name().to_owned(),
                            tipo: c.type_info().name().to_owned(),
                        })
                        .collect();
                }
                if linhas.len() == $limite {
                    cortada = true;
                    break;
                }
                let valores = (0..linha.len())
                    .map(|i| valor_json(celula_texto(&linha, i), &colunas[i].tipo))
                    .collect();
                linhas.push(valores);
            }
            Ok::<_, sqlx::Error>((colunas, linhas, cortada))
        }
        .await
    };
}

impl Sessao {
    /// Conecta e deixa a sessão só de leitura, com o tempo limite.
    pub(crate) async fn abrir(cfg: &ConfigBanco) -> Result<Self, ErroBanco> {
        let ms = cfg.tempo_limite.as_millis().to_string();
        let sessao = match cfg.dialeto {
            DialetoSql::Postgres => {
                let mut opcoes = vec![
                    ("default_transaction_read_only", "on".to_owned()),
                    ("statement_timeout", ms.clone()),
                    ("lock_timeout", ms.clone()),
                    ("idle_in_transaction_session_timeout", ms.clone()),
                ];
                if let Some(e) = &cfg.esquema {
                    opcoes.push(("search_path", e.clone()));
                }
                let mut o = PgConnectOptions::new_without_pgpass()
                    .host(&cfg.host)
                    .port(cfg.porta)
                    .username(&cfg.usuario)
                    .database(&cfg.banco)
                    .application_name("ctower-business-agents")
                    .ssl_mode(match cfg.tls {
                        Tls::Exigir => PgSslMode::Require,
                        Tls::Preferir => PgSslMode::Prefer,
                        Tls::Desligar => PgSslMode::Disable,
                    })
                    .options(opcoes);
                if let Some(s) = &cfg.senha {
                    o = o.password(s.expose());
                }
                let con = conectar(cfg, PgConnection::connect_with(&o)).await?;
                Sessao::Pg(con)
            }
            DialetoSql::Mysql => {
                let mut o = MySqlConnectOptions::new()
                    .host(&cfg.host)
                    .port(cfg.porta)
                    .username(&cfg.usuario)
                    .database(&cfg.banco)
                    .ssl_mode(match cfg.tls {
                        Tls::Exigir => MySqlSslMode::Required,
                        Tls::Preferir => MySqlSslMode::Preferred,
                        Tls::Desligar => MySqlSslMode::Disabled,
                    });
                if let Some(s) = &cfg.senha {
                    o = o.password(s.expose());
                }
                let mut con = conectar(cfg, MySqlConnection::connect_with(&o)).await?;
                let versao = texto_unico!(&mut con, "SELECT VERSION()")
                    .map_err(|e| erro_driver(cfg, e, true))?;
                let mariadb = versao.to_lowercase().contains("mariadb");
                let mut comandos = vec!["SET SESSION TRANSACTION READ ONLY".to_owned()];
                comandos.push(if mariadb {
                    // Em segundos, com fração.
                    format!(
                        "SET SESSION max_statement_time = {:.3}",
                        cfg.tempo_limite.as_secs_f64()
                    )
                } else {
                    format!("SET SESSION max_execution_time = {ms}")
                });
                comandos.push(format!(
                    "SET SESSION lock_wait_timeout = {}",
                    cfg.tempo_limite.as_secs().max(1)
                ));
                if let Some(e) = &cfg.esquema {
                    // Identificador já conferido em `ConfigBanco::da_conexao`.
                    comandos.push(format!("USE `{e}`"));
                }
                for c in comandos {
                    raw_sql(AssertSqlSafe(c))
                        .execute(&mut con)
                        .await
                        .map_err(|e| erro_driver(cfg, e, true))?;
                }
                Sessao::My { con, mariadb }
            }
        };
        Ok(sessao)
    }

    /// Versão do servidor (`PostgreSQL 16.4`, `MySQL 8.4.2`, `MariaDB 11.4.3`).
    pub(crate) async fn versao(&mut self, cfg: &ConfigBanco) -> Result<String, ErroBanco> {
        match self {
            Sessao::Pg(con) => {
                let v = texto_unico!(con, "SHOW server_version")
                    .map_err(|e| erro_driver(cfg, e, false))?;
                Ok(format!("PostgreSQL {v}"))
            }
            Sessao::My { con, mariadb } => {
                let v = texto_unico!(con, "SELECT VERSION()")
                    .map_err(|e| erro_driver(cfg, e, false))?;
                let numero = v.split('-').next().unwrap_or(&v).to_owned();
                Ok(if *mariadb {
                    format!("MariaDB {numero}")
                } else {
                    format!("MySQL {numero}")
                })
            }
        }
    }

    /// Roda `sql` (já validado) numa transação só de leitura, desfeita no fim,
    /// trazendo no máximo `cfg.limite_linhas` linhas.
    pub(crate) async fn consultar(
        &mut self,
        cfg: &ConfigBanco,
        sql: String,
    ) -> Result<ResultadoConsulta, ErroBanco> {
        let inicio = Instant::now();
        let limite = cfg.limite_linhas;
        let abrir = match self {
            Sessao::Pg(_) => "BEGIN READ ONLY",
            Sessao::My { .. } => "START TRANSACTION READ ONLY",
        };
        self.executar(cfg, abrir).await?;
        let lido = em_cada!(self, con => ler_linhas!(con, sql, limite));
        let (colunas, linhas, cortada) = lido.map_err(|e| erro_driver(cfg, e, false))?;
        self.executar(cfg, "ROLLBACK").await?;
        Ok(ResultadoConsulta {
            sql,
            colunas,
            linhas,
            cortada,
            limite_linhas: limite,
            duracao_ms: inicio.elapsed().as_millis() as u64,
        })
    }

    pub(crate) async fn executar(&mut self, cfg: &ConfigBanco, sql: &str) -> Result<(), ErroBanco> {
        let r = em_cada!(self, con => raw_sql(AssertSqlSafe(sql.to_owned()))
            .execute(&mut *con)
            .await
            .map(|_| ()));
        r.map_err(|e| erro_driver(cfg, e, false))
    }

    /// Linhas de uma consulta interna (catálogo), como texto.
    pub(crate) async fn textos(
        &mut self,
        cfg: &ConfigBanco,
        sql: &str,
    ) -> Result<Vec<Vec<Option<String>>>, ErroBanco> {
        let r = em_cada!(self, con => linhas_texto!(con, sql));
        r.map_err(|e| erro_driver(cfg, e, false))
    }

    pub(crate) fn dialeto(&self) -> DialetoSql {
        match self {
            Sessao::Pg(_) => DialetoSql::Postgres,
            Sessao::My { .. } => DialetoSql::Mysql,
        }
    }

    pub(crate) async fn fechar(self) {
        let _ = match self {
            Sessao::Pg(con) => con.close().await,
            Sessao::My { con, .. } => con.close().await,
        };
    }
}

async fn conectar<C>(
    cfg: &ConfigBanco,
    f: impl Future<Output = Result<C, sqlx::Error>>,
) -> Result<C, ErroBanco> {
    match tokio::time::timeout(PRAZO_CONEXAO, f).await {
        Ok(r) => r.map_err(|e| erro_driver(cfg, e, true)),
        Err(_) => Err(ErroBanco::Conexao(format!(
            "o servidor {}:{} não respondeu em {} s",
            cfg.host,
            cfg.porta,
            PRAZO_CONEXAO.as_secs()
        ))),
    }
}

/// A célula como texto: o protocolo simples (sem parâmetros) traz tudo em
/// texto; o que não é UTF-8 vira uma marca com o tamanho.
fn celula_texto<R: Row>(linha: &R, i: usize) -> Option<String>
where
    usize: sqlx::ColumnIndex<R>,
    for<'r> String: sqlx::Decode<'r, R::Database>,
    for<'r> Vec<u8>: sqlx::Decode<'r, R::Database>,
{
    if linha.try_get_raw(i).map(|v| v.is_null()).unwrap_or(true) {
        return None;
    }
    if let Ok(t) = linha.try_get_unchecked::<String, _>(i) {
        return Some(t);
    }
    let n = linha
        .try_get_unchecked::<Vec<u8>, _>(i)
        .map(|b| b.len())
        .unwrap_or(0);
    Some(format!("<binário: {n} bytes>"))
}

/// O texto da célula como JSON, pelo tipo da coluna: inteiros e reais viram
/// número, booleanos do PostgreSQL viram booleano, o resto fica texto (decimais
/// também, para não perder precisão).
fn valor_json(texto: Option<String>, tipo: &str) -> Value {
    let Some(t) = texto else {
        return Value::Null;
    };
    let tipo = tipo.to_uppercase();
    let base = tipo.split_whitespace().next().unwrap_or_default();
    match base {
        "INT2" | "INT4" | "INT8" | "OID" | "TINYINT" | "SMALLINT" | "MEDIUMINT" | "INT"
        | "INTEGER" | "BIGINT" | "YEAR" => {
            if let Ok(n) = t.parse::<i64>() {
                return Value::from(n);
            }
            if let Ok(n) = t.parse::<u64>() {
                return Value::from(n);
            }
        }
        "FLOAT4" | "FLOAT8" | "FLOAT" | "DOUBLE" | "REAL" => {
            if let Some(n) = t.parse::<f64>().ok().and_then(serde_json::Number::from_f64) {
                return Value::Number(n);
            }
        }
        "BOOL" | "BOOLEAN" => match t.as_str() {
            "t" | "true" | "1" => return Value::Bool(true),
            "f" | "false" | "0" => return Value::Bool(false),
            _ => {}
        },
        _ => {}
    }
    Value::String(cortar(t))
}

fn cortar(t: String) -> String {
    if t.chars().count() <= MAX_CARACTERES_CELULA {
        return t;
    }
    let mut c: String = t.chars().take(MAX_CARACTERES_CELULA).collect();
    c.push_str("… [cortado]");
    c
}

/// Valida `sql` e roda numa sessão só de leitura, com o tempo limite e o
/// limite de linhas da conexão.
pub fn consultar(cfg: &ConfigBanco, sql: &str) -> Result<ResultadoConsulta, ErroBanco> {
    let sql = validacao::validar(cfg.dialeto, sql)?;
    let cfg = cfg.clone();
    let prazo = cfg.tempo_limite + FOLGA_PRAZO;
    bloquear(async move {
        let mut s = Sessao::abrir(&cfg).await?;
        let r = match tokio::time::timeout(prazo, s.consultar(&cfg, sql)).await {
            Ok(r) => r,
            Err(_) => Err(ErroBanco::TempoEsgotado(cfg.tempo_limite.as_secs())),
        };
        s.fechar().await;
        r
    })?
}

/// Os passos de prontidão dos descritores de banco, na ordem: `conectar`,
/// `versao`, `so_leitura` e `esquemas`. Depois de uma falha os seguintes não
/// rodam (não aparecem na lista).
pub fn testar(cfg: &ConfigBanco) -> Vec<ResultadoPasso> {
    let cfg = cfg.clone();
    match bloquear(async move { testar_async(&cfg).await }) {
        Ok(passos) => passos,
        Err(e) => vec![ResultadoPasso {
            id: "conectar",
            ok: false,
            detalhe: e.to_string(),
        }],
    }
}

fn passo(id: &'static str, r: Result<String, String>) -> ResultadoPasso {
    match r {
        Ok(detalhe) => ResultadoPasso {
            id,
            ok: true,
            detalhe,
        },
        Err(detalhe) => ResultadoPasso {
            id,
            ok: false,
            detalhe,
        },
    }
}

async fn testar_async(cfg: &ConfigBanco) -> Vec<ResultadoPasso> {
    let mut passos = Vec::new();
    let mut s = match Sessao::abrir(cfg).await {
        Ok(s) => s,
        Err(e) => {
            passos.push(passo("conectar", Err(e.to_string())));
            return passos;
        }
    };
    passos.push(passo(
        "conectar",
        Ok(format!(
            "conectado a {}:{}/{} como {}",
            cfg.host, cfg.porta, cfg.banco, cfg.usuario
        )),
    ));
    let etapas = async {
        let versao = s.versao(cfg).await.map_err(|e| ("versao", e.to_string()))?;
        passos.push(passo("versao", Ok(versao)));
        let so_leitura = conferir_so_leitura(&mut s, cfg)
            .await
            .map_err(|e| ("so_leitura", e))?;
        passos.push(passo("so_leitura", Ok(so_leitura)));
        let esquemas = listar_esquemas(&mut s, cfg)
            .await
            .map_err(|e| ("esquemas", e))?;
        passos.push(passo("esquemas", Ok(esquemas)));
        Ok::<(), (&'static str, String)>(())
    };
    let r = etapas.await;
    if let Err((id, motivo)) = r {
        passos.push(passo(id, Err(motivo)));
    }
    s.fechar().await;
    passos
}

/// Tenta escrever e espera a recusa do banco.
///
/// No PostgreSQL, a escrita é a criação de uma tabela temporária, que a
/// transação só de leitura recusa. No MySQL/MariaDB a transação só de leitura
/// aceita tabelas temporárias, então a escrita tentada é a criação de uma
/// tabela comum, que a sessão em `READ ONLY` recusa; se o banco aceitar, a
/// tabela é apagada na hora e o passo falha. A validação do app também precisa
/// recusar a escrita.
async fn conferir_so_leitura(s: &mut Sessao, cfg: &ConfigBanco) -> Result<String, String> {
    if validacao::validar(s.dialeto(), "CREATE TABLE cba_teste_so_leitura (a int)").is_ok() {
        return Err("a validação do app aceitou uma escrita".into());
    }
    let (escrita, desfazer) = match s.dialeto() {
        DialetoSql::Postgres => (
            "CREATE TEMPORARY TABLE cba_teste_so_leitura (a int)",
            "DROP TABLE IF EXISTS cba_teste_so_leitura",
        ),
        DialetoSql::Mysql => (
            "CREATE TABLE cba_teste_so_leitura (a int)",
            "DROP TABLE IF EXISTS cba_teste_so_leitura",
        ),
    };
    match s.executar(cfg, escrita).await {
        Err(ErroBanco::Banco(motivo)) => Ok(format!("a escrita foi recusada pelo banco: {motivo}")),
        Err(outro) => Err(format!("não foi possível conferir: {outro}")),
        Ok(()) => {
            let _ = s.executar(cfg, desfazer).await;
            Err("o banco aceitou uma escrita nesta sessão".into())
        }
    }
}

async fn listar_esquemas(s: &mut Sessao, cfg: &ConfigBanco) -> Result<String, String> {
    let sql = match s.dialeto() {
        DialetoSql::Postgres => {
            "SELECT nspname::text FROM pg_namespace \
             WHERE has_schema_privilege(oid, 'USAGE') \
               AND nspname NOT IN ('pg_catalog', 'information_schema') \
               AND nspname NOT LIKE 'pg\\_%' \
             ORDER BY 1"
        }
        DialetoSql::Mysql => {
            "SELECT SCHEMA_NAME FROM information_schema.SCHEMATA \
             WHERE SCHEMA_NAME NOT IN ('information_schema', 'mysql', 'performance_schema', 'sys') \
             ORDER BY 1"
        }
    };
    let linhas = s.textos(cfg, sql).await.map_err(|e| e.to_string())?;
    let nomes: Vec<String> = linhas
        .into_iter()
        .filter_map(|l| l.into_iter().next().flatten())
        .collect();
    if nomes.is_empty() {
        return Err("nenhum esquema visível para este usuário".into());
    }
    Ok(format!("{} esquema(s): {}", nomes.len(), nomes.join(", ")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn valores_pelo_tipo() {
        assert_eq!(valor_json(None, "INT4"), Value::Null);
        assert_eq!(valor_json(Some("42".into()), "INT8"), json!(42));
        assert_eq!(valor_json(Some("42".into()), "BIGINT UNSIGNED"), json!(42));
        assert_eq!(valor_json(Some("1.5".into()), "FLOAT8"), json!(1.5));
        assert_eq!(valor_json(Some("t".into()), "BOOL"), json!(true));
        assert_eq!(valor_json(Some("10.10".into()), "NUMERIC"), json!("10.10"));
        assert_eq!(valor_json(Some("abc".into()), "TEXT"), json!("abc"));
        let longo = "x".repeat(MAX_CARACTERES_CELULA + 10);
        let v = valor_json(Some(longo), "TEXT");
        assert!(v.as_str().unwrap().ends_with("[cortado]"));
    }
}
