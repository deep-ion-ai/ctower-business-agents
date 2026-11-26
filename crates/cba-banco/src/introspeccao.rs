//! Introspecção do schema real: tabelas, visões, colunas, tipos e chaves que o
//! usuário da conexão enxerga, pelo catálogo do servidor.

use std::collections::BTreeMap;

use cba_core::banco::{ChaveEstrangeira, ColunaBanco, SchemaBanco, TabelaBanco};
use cba_repos::providers::DialetoSql;

use crate::sessao::{Sessao, bloquear};
use crate::{ConfigBanco, ErroBanco};

/// Separa as colunas de uma chave numa célula só (não aparece em nomes).
const SEP: char = '\u{1f}';

const COLUNAS_PG: &str = "\
SELECT n.nspname::text, c.relname::text,
       CASE WHEN c.relkind IN ('v', 'm') THEN 'visao' ELSE 'tabela' END,
       a.attname::text, format_type(a.atttypid, a.atttypmod),
       (NOT a.attnotnull)::text,
       col_description(c.oid, a.attnum), obj_description(c.oid, 'pg_class')
FROM pg_class c
JOIN pg_namespace n ON n.oid = c.relnamespace
JOIN pg_attribute a ON a.attrelid = c.oid AND a.attnum > 0 AND NOT a.attisdropped
WHERE c.relkind IN ('r', 'p', 'v', 'm', 'f')
  AND n.nspname NOT IN ('pg_catalog', 'information_schema')
  AND n.nspname NOT LIKE 'pg\\_%'
  AND has_schema_privilege(n.oid, 'USAGE')
  AND has_any_column_privilege(c.oid, 'SELECT')
ORDER BY 1, 2, a.attnum";

const CHAVES_PG: &str = "\
SELECT n.nspname::text, c.relname::text, k.conname::text, k.contype::text,
       (SELECT string_agg(a.attname::text, chr(31) ORDER BY x.ord)
          FROM unnest(k.conkey) WITH ORDINALITY x(num, ord)
          JOIN pg_attribute a ON a.attrelid = k.conrelid AND a.attnum = x.num),
       rn.nspname::text || '.' || rc.relname::text,
       (SELECT string_agg(a.attname::text, chr(31) ORDER BY x.ord)
          FROM unnest(k.confkey) WITH ORDINALITY x(num, ord)
          JOIN pg_attribute a ON a.attrelid = k.confrelid AND a.attnum = x.num)
FROM pg_constraint k
JOIN pg_class c ON c.oid = k.conrelid
JOIN pg_namespace n ON n.oid = c.relnamespace
LEFT JOIN pg_class rc ON rc.oid = k.confrelid
LEFT JOIN pg_namespace rn ON rn.oid = rc.relnamespace
WHERE k.contype IN ('p', 'f')
  AND n.nspname NOT IN ('pg_catalog', 'information_schema')
  AND n.nspname NOT LIKE 'pg\\_%'
ORDER BY 1, 2, 3";

const COLUNAS_MY: &str = "\
SELECT c.TABLE_SCHEMA, c.TABLE_NAME,
       CASE WHEN t.TABLE_TYPE = 'VIEW' THEN 'visao' ELSE 'tabela' END,
       c.COLUMN_NAME, c.COLUMN_TYPE,
       CASE WHEN c.IS_NULLABLE = 'YES' THEN 'true' ELSE 'false' END,
       c.COLUMN_COMMENT, t.TABLE_COMMENT
FROM information_schema.COLUMNS c
JOIN information_schema.TABLES t
  ON t.TABLE_SCHEMA = c.TABLE_SCHEMA AND t.TABLE_NAME = c.TABLE_NAME
WHERE c.TABLE_SCHEMA NOT IN ('information_schema', 'mysql', 'performance_schema', 'sys')
ORDER BY c.TABLE_SCHEMA, c.TABLE_NAME, c.ORDINAL_POSITION";

const CHAVES_MY: &str = "\
SELECT k.TABLE_SCHEMA, k.TABLE_NAME, k.CONSTRAINT_NAME,
       CASE WHEN tc.CONSTRAINT_TYPE = 'PRIMARY KEY' THEN 'p' ELSE 'f' END,
       k.COLUMN_NAME,
       CASE WHEN k.REFERENCED_TABLE_NAME IS NULL THEN NULL
            ELSE CONCAT(k.REFERENCED_TABLE_SCHEMA, '.', k.REFERENCED_TABLE_NAME) END,
       k.REFERENCED_COLUMN_NAME
FROM information_schema.KEY_COLUMN_USAGE k
JOIN information_schema.TABLE_CONSTRAINTS tc
  ON tc.CONSTRAINT_SCHEMA = k.CONSTRAINT_SCHEMA
 AND tc.TABLE_NAME = k.TABLE_NAME
 AND tc.CONSTRAINT_NAME = k.CONSTRAINT_NAME
WHERE tc.CONSTRAINT_TYPE IN ('PRIMARY KEY', 'FOREIGN KEY')
  AND k.TABLE_SCHEMA NOT IN ('information_schema', 'mysql', 'performance_schema', 'sys')
ORDER BY k.TABLE_SCHEMA, k.TABLE_NAME, k.CONSTRAINT_NAME, k.ORDINAL_POSITION";

/// Tabelas, visões, colunas e chaves do banco.
pub fn ler_schema(cfg: &ConfigBanco) -> Result<SchemaBanco, ErroBanco> {
    let cfg = cfg.clone();
    bloquear(async move {
        let mut s = Sessao::abrir(&cfg).await?;
        let (colunas, chaves) = match s.dialeto() {
            DialetoSql::Postgres => (COLUNAS_PG, CHAVES_PG),
            DialetoSql::Mysql => (COLUNAS_MY, CHAVES_MY),
        };
        let r = async {
            let c = s.textos(&cfg, colunas).await?;
            let k = s.textos(&cfg, chaves).await?;
            Ok::<_, ErroBanco>((c, k))
        }
        .await;
        s.fechar().await;
        let (c, k) = r?;
        Ok(montar(c, k))
    })?
}

type Linhas = Vec<Vec<Option<String>>>;

fn celula(l: &[Option<String>], i: usize) -> String {
    l.get(i).cloned().flatten().unwrap_or_default()
}

fn opcional(l: &[Option<String>], i: usize) -> Option<String> {
    l.get(i)
        .cloned()
        .flatten()
        .map(|t| t.trim().to_owned())
        .filter(|t| !t.is_empty())
}

/// Junta as linhas do catálogo em tabelas. As chaves do MySQL vêm uma linha
/// por coluna; as do PostgreSQL, uma por chave com as colunas separadas por
/// [`SEP`]: as duas formas se juntam pelo nome da chave.
fn montar(colunas: Linhas, chaves: Linhas) -> SchemaBanco {
    let mut tabelas: BTreeMap<(String, String), TabelaBanco> = BTreeMap::new();
    for l in &colunas {
        let chave = (celula(l, 0), celula(l, 1));
        let t = tabelas.entry(chave.clone()).or_insert_with(|| TabelaBanco {
            esquema: chave.0.clone(),
            nome: chave.1.clone(),
            tipo: celula(l, 2),
            comentario: opcional(l, 7),
            colunas: Vec::new(),
            chave_primaria: Vec::new(),
            chaves_estrangeiras: Vec::new(),
        });
        t.colunas.push(ColunaBanco {
            nome: celula(l, 3),
            tipo: celula(l, 4),
            nula: celula(l, 5) == "true",
            comentario: opcional(l, 6),
        });
    }
    for l in &chaves {
        let Some(t) = tabelas.get_mut(&(celula(l, 0), celula(l, 1))) else {
            continue;
        };
        let nome = celula(l, 2);
        let cols: Vec<String> = celula(l, 4).split(SEP).map(str::to_owned).collect();
        if celula(l, 3) == "p" {
            t.chave_primaria.extend(cols);
            continue;
        }
        let refs: Vec<String> = celula(l, 6).split(SEP).map(str::to_owned).collect();
        match t.chaves_estrangeiras.iter_mut().find(|f| f.nome == nome) {
            Some(f) => {
                f.colunas.extend(cols);
                f.colunas_referencia.extend(refs);
            }
            None => t.chaves_estrangeiras.push(ChaveEstrangeira {
                nome,
                colunas: cols,
                referencia: celula(l, 5),
                colunas_referencia: refs,
            }),
        }
    }
    SchemaBanco {
        tabelas: tabelas.into_values().collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn l(v: &[Option<&str>]) -> Vec<Option<String>> {
        v.iter().map(|c| c.map(str::to_owned)).collect()
    }

    #[test]
    fn junta_colunas_e_chaves_dos_dois_formatos() {
        let colunas = vec![
            l(&[
                Some("app"),
                Some("cliente"),
                Some("tabela"),
                Some("id"),
                Some("integer"),
                Some("false"),
                None,
                Some("Clientes"),
            ]),
            l(&[
                Some("app"),
                Some("cliente"),
                Some("tabela"),
                Some("cpf"),
                Some("char(11)"),
                Some("true"),
                Some(""),
                None,
            ]),
            l(&[
                Some("app"),
                Some("fatura"),
                Some("tabela"),
                Some("id"),
                Some("integer"),
                Some("false"),
                None,
                None,
            ]),
            l(&[
                Some("app"),
                Some("fatura"),
                Some("tabela"),
                Some("cliente_id"),
                Some("integer"),
                Some("false"),
                None,
                None,
            ]),
            l(&[
                Some("app"),
                Some("fatura"),
                Some("tabela"),
                Some("loja_id"),
                Some("integer"),
                Some("false"),
                None,
                None,
            ]),
        ];
        let chaves = vec![
            l(&[
                Some("app"),
                Some("cliente"),
                Some("cliente_pkey"),
                Some("p"),
                Some("id"),
                None,
                None,
            ]),
            // PostgreSQL: uma linha com as colunas juntas.
            l(&[
                Some("app"),
                Some("fatura"),
                Some("fk_fat"),
                Some("f"),
                Some("cliente_id\u{1f}loja_id"),
                Some("app.cliente"),
                Some("id\u{1f}loja"),
            ]),
            // MySQL: uma linha por coluna.
            l(&[
                Some("app"),
                Some("fatura"),
                Some("fk_x"),
                Some("f"),
                Some("cliente_id"),
                Some("app.cliente"),
                Some("id"),
            ]),
            l(&[
                Some("app"),
                Some("fatura"),
                Some("fk_x"),
                Some("f"),
                Some("loja_id"),
                Some("app.cliente"),
                Some("loja"),
            ]),
            l(&[
                Some("app"),
                Some("sumiu"),
                Some("pk"),
                Some("p"),
                Some("id"),
                None,
                None,
            ]),
        ];
        let s = montar(colunas, chaves);
        assert_eq!(s.tabelas.len(), 2);
        let cliente = &s.tabelas[0];
        assert_eq!(cliente.qualificado(), "app.cliente");
        assert_eq!(cliente.comentario.as_deref(), Some("Clientes"));
        assert_eq!(cliente.chave_primaria, ["id"]);
        assert!(cliente.colunas[1].nula && !cliente.colunas[0].nula);
        assert_eq!(cliente.colunas[1].comentario, None);
        let fatura = &s.tabelas[1];
        assert_eq!(fatura.chaves_estrangeiras.len(), 2);
        for f in &fatura.chaves_estrangeiras {
            assert_eq!(f.colunas, ["cliente_id", "loja_id"]);
            assert_eq!(f.colunas_referencia, ["id", "loja"]);
            assert_eq!(f.referencia, "app.cliente");
        }
    }
}
