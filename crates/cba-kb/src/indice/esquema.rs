//! Esquema SQLite do índice e gravação de um documento.

use rusqlite::{Transaction, params};

use super::extracao::Registro;

/// Versão do esquema, em `PRAGMA user_version`. Mudou o esquema, suba o número: o
/// índice é recriado e reconstruído a partir da base.
pub(super) const VERSAO: i64 = 1;

/// Tabelas do índice.
///
/// - `documento`: um arquivo da base. `caminho` é relativo à raiz da base
///   (`<projeto>/regras/RN-FAT-001.md`). `tipo` é o do frontmatter ou
///   `regras-do-projeto`; nulo quando o frontmatter não pôde ser lido. `erro` diz
///   por que o documento não passou na validação do tipo.
/// - `evidencia`: evidências do frontmatter das regras.
/// - `ligacao`: campos `origem` e `analise` que apontam para outro documento.
/// - `busca`: FTS5 com o título, as chaves (identificador, termo, sinônimos,
///   área) e o corpo, `rowid` igual ao `id` do documento.
pub(super) const CRIAR: &str = "
CREATE TABLE meta (
    chave TEXT PRIMARY KEY,
    valor TEXT NOT NULL
);
CREATE TABLE documento (
    id INTEGER PRIMARY KEY,
    caminho TEXT NOT NULL UNIQUE,
    projeto TEXT NOT NULL,
    tipo TEXT,
    identificador TEXT,
    titulo TEXT NOT NULL,
    area TEXT,
    status TEXT,
    frontmatter TEXT NOT NULL,
    erro TEXT,
    hash TEXT NOT NULL,
    tamanho INTEGER NOT NULL,
    modificado_ns INTEGER NOT NULL,
    verificado_ns INTEGER NOT NULL
);
CREATE INDEX documento_projeto_tipo ON documento (projeto, tipo);
CREATE INDEX documento_identificador ON documento (identificador);
CREATE TABLE evidencia (
    documento INTEGER NOT NULL REFERENCES documento (id),
    ordem INTEGER NOT NULL,
    arquivo TEXT NOT NULL,
    linha_inicial INTEGER NOT NULL,
    linha_final INTEGER NOT NULL,
    hash TEXT NOT NULL,
    commit_ TEXT,
    PRIMARY KEY (documento, ordem)
);
CREATE INDEX evidencia_arquivo ON evidencia (arquivo);
CREATE TABLE ligacao (
    documento INTEGER NOT NULL REFERENCES documento (id),
    para TEXT NOT NULL,
    campo TEXT NOT NULL,
    PRIMARY KEY (documento, para, campo)
);
CREATE INDEX ligacao_para ON ligacao (para);
CREATE VIRTUAL TABLE busca USING fts5 (
    titulo,
    chaves,
    corpo,
    tokenize = 'unicode61 remove_diacritics 2'
);
";

/// Apaga as tabelas (para recriar numa versão nova do esquema).
pub(super) const APAGAR: &str = "
DROP TABLE IF EXISTS busca;
DROP TABLE IF EXISTS ligacao;
DROP TABLE IF EXISTS evidencia;
DROP TABLE IF EXISTS documento;
DROP TABLE IF EXISTS meta;
";

/// Esvazia as tabelas de conteúdo (outra base, reconstrução).
pub(super) const LIMPAR: &str = "
DELETE FROM busca;
DELETE FROM ligacao;
DELETE FROM evidencia;
DELETE FROM documento;
";

/// Dados do arquivo para a atualização incremental.
pub(super) struct Arquivo<'a> {
    pub hash: &'a str,
    pub tamanho: i64,
    pub modificado_ns: i64,
    pub verificado_ns: i64,
}

pub(super) fn inserir(
    tx: &Transaction<'_>,
    r: &Registro,
    arquivo: &Arquivo<'_>,
) -> rusqlite::Result<()> {
    tx.execute(
        "INSERT INTO documento (caminho, projeto, tipo, identificador, titulo, area, status, \
             frontmatter, erro, hash, tamanho, modificado_ns, verificado_ns) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            r.caminho,
            r.projeto,
            r.tipo.map(|t| t.como_str()),
            r.identificador,
            r.titulo,
            r.area,
            r.status,
            r.frontmatter,
            r.erro,
            arquivo.hash,
            arquivo.tamanho,
            arquivo.modificado_ns,
            arquivo.verificado_ns,
        ],
    )?;
    let id = tx.last_insert_rowid();
    if r.buscavel {
        tx.execute(
            "INSERT INTO busca (rowid, titulo, chaves, corpo) VALUES (?1, ?2, ?3, ?4)",
            params![id, r.titulo, r.chaves, r.corpo],
        )?;
    }
    for (ordem, e) in r.evidencias.iter().enumerate() {
        tx.execute(
            "INSERT INTO evidencia (documento, ordem, arquivo, linha_inicial, linha_final, hash, \
                 commit_) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                id,
                i64::try_from(ordem).unwrap_or(i64::MAX),
                e.arquivo.as_str(),
                e.linhas.inicio,
                e.linhas.fim,
                e.hash,
                e.commit,
            ],
        )?;
    }
    for (para, campo) in &r.ligacoes {
        tx.execute(
            "INSERT OR IGNORE INTO ligacao (documento, para, campo) VALUES (?1, ?2, ?3)",
            params![id, para, campo],
        )?;
    }
    Ok(())
}

pub(super) fn remover(tx: &Transaction<'_>, id: i64) -> rusqlite::Result<()> {
    tx.execute("DELETE FROM busca WHERE rowid = ?1", [id])?;
    tx.execute("DELETE FROM ligacao WHERE documento = ?1", [id])?;
    tx.execute("DELETE FROM evidencia WHERE documento = ?1", [id])?;
    tx.execute("DELETE FROM documento WHERE id = ?1", [id])?;
    Ok(())
}
