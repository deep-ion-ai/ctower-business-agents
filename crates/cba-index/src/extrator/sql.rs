//! Extrator de SQL e PL/SQL (Oracle, com tolerância a outros dialetos).
//!
//! Lê scripts de DDL e código PL/SQL com um analisador léxico próprio e uma
//! descida recursiva rasa, sem gramática completa: o objetivo é o mapa, não validar
//! o SQL. Reconhece:
//!
//! - `CREATE TABLE` (colunas, chaves estrangeiras), `VIEW`, `MATERIALIZED VIEW`,
//!   `SEQUENCE`, `INDEX`, `SYNONYM`, `TYPE`;
//! - `PACKAGE` e `PACKAGE BODY` (com as funções e procedures de dentro),
//!   `PROCEDURE`, `FUNCTION`, `TRIGGER`, `TYPE BODY` e blocos anônimos;
//! - `ALTER TABLE ... REFERENCES` e `COMMENT ON TABLE|COLUMN`;
//! - referências: leitura (`FROM`, `JOIN`, `USING` do `MERGE`), escrita (`INSERT`,
//!   `UPDATE`, `DELETE`, `MERGE`, `TRUNCATE`), chamadas (`nome(...)`, `nome;`,
//!   `CALL`, `EXEC`), `%TYPE`/`%ROWTYPE`, `seq.NEXTVAL` e disparo de trigger.
//!
//! Variáveis e parâmetros locais e as funções nativas mais comuns não viram
//! referência. Comandos do SQL*Plus (`REM`, `PROMPT`, `SPOOL`, `@arquivo`) são
//! ignorados.

use std::collections::{BTreeMap, HashSet};

use super::{Extracao, Extrator, Fonte, Linhas, Referencia, Simbolo, TipoReferencia, TipoSimbolo};
use crate::linguagem::Linguagem;

/// Extrator de SQL e PL/SQL.
#[derive(Debug, Clone, Copy, Default)]
pub struct ExtratorSql;

impl Extrator for ExtratorSql {
    fn nome(&self) -> &'static str {
        "sql"
    }

    fn versao(&self) -> u32 {
        1
    }

    fn linguagens(&self) -> &'static [Linguagem] {
        &[Linguagem::Sql]
    }

    fn extrair(&self, fonte: &Fonte<'_>) -> Result<Extracao, String> {
        let lex = lexico(fonte.texto);
        let mut p = Analisador {
            t: &lex.tokens,
            i: 0,
            comentarios: &lex.comentarios,
            saida: Extracao::default(),
            pendentes: Vec::new(),
        };
        p.arquivo();
        p.aplicar_comentarios();
        Ok(p.saida)
    }
}

// ---------------------------------------------------------------------------
// Léxico

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tipo {
    Ident,
    /// Identificador entre aspas duplas (`"Fatura"`): não é palavra-chave.
    IdentAspas,
    Texto,
    Numero,
    Sim(char),
}

#[derive(Debug, Clone)]
struct Token {
    tipo: Tipo,
    /// Texto original (identificador sem as aspas, literal com as aspas).
    texto: String,
    /// Identificador sem aspas em maiúsculas; vazio nos demais.
    chave: String,
    linha: u32,
    /// Primeiro token da linha.
    primeiro: bool,
}

struct Lexico {
    tokens: Vec<Token>,
    /// Comentários que ocupam a linha sozinhos, por linha.
    comentarios: BTreeMap<u32, String>,
}

fn lexico(texto: &str) -> Lexico {
    let c: Vec<char> = texto.chars().collect();
    let mut tokens: Vec<Token> = Vec::new();
    let mut comentarios = BTreeMap::new();
    let mut linha = 1u32;
    let mut inicio_linha = true; // ainda não houve token nesta linha
    let mut i = 0;
    let n = c.len();
    let push = |tokens: &mut Vec<Token>, tipo, texto: String, linha, inicio: &mut bool| {
        let chave = if tipo == Tipo::Ident {
            texto.to_uppercase()
        } else {
            String::new()
        };
        tokens.push(Token {
            tipo,
            texto,
            chave,
            linha,
            primeiro: *inicio,
        });
        *inicio = false;
    };
    while i < n {
        let ch = c[i];
        if ch == '\n' {
            linha += 1;
            inicio_linha = true;
            i += 1;
            continue;
        }
        if ch.is_whitespace() {
            i += 1;
            continue;
        }
        // Comandos do SQL*Plus no começo da linha.
        if inicio_linha && (ch == '@' || palavra_sqlplus(&c[i..])) {
            while i < n && c[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if ch == '-' && c.get(i + 1) == Some(&'-') {
            let ini = i + 2;
            while i < n && c[i] != '\n' {
                i += 1;
            }
            if inicio_linha {
                let t: String = c[ini..i].iter().collect();
                comentarios.insert(linha, t);
            }
            continue;
        }
        if ch == '/' && c.get(i + 1) == Some(&'*') {
            let sozinho = inicio_linha;
            let l0 = linha;
            let ini = i + 2;
            i += 2;
            while i < n && !(c[i] == '*' && c.get(i + 1) == Some(&'/')) {
                if c[i] == '\n' {
                    linha += 1;
                }
                i += 1;
            }
            let t: String = c[ini..i.min(n)].iter().collect();
            i = (i + 2).min(n);
            if sozinho {
                // Um comentário de bloco conta para cada linha que ocupa.
                for (k, parte) in t.split('\n').enumerate() {
                    comentarios.insert(l0 + k as u32, parte.to_string());
                }
            }
            continue;
        }
        // q'[...]' do Oracle.
        if (ch == 'q' || ch == 'Q') && c.get(i + 1) == Some(&'\'') && i + 2 < n {
            let abre = c[i + 2];
            let fecha = match abre {
                '[' => ']',
                '(' => ')',
                '{' => '}',
                '<' => '>',
                x => x,
            };
            let l0 = linha;
            let ini = i;
            i += 3;
            while i < n && !(c[i] == fecha && c.get(i + 1) == Some(&'\'')) {
                if c[i] == '\n' {
                    linha += 1;
                }
                i += 1;
            }
            i = (i + 2).min(n);
            let t: String = c[ini..i].iter().collect();
            push(&mut tokens, Tipo::Texto, t, l0, &mut inicio_linha);
            continue;
        }
        if ch == '\'' {
            let l0 = linha;
            let ini = i;
            i += 1;
            while i < n {
                if c[i] == '\'' {
                    if c.get(i + 1) == Some(&'\'') {
                        i += 2;
                        continue;
                    }
                    break;
                }
                if c[i] == '\n' {
                    linha += 1;
                }
                i += 1;
            }
            i = (i + 1).min(n);
            let t: String = c[ini..i].iter().collect();
            push(&mut tokens, Tipo::Texto, t, l0, &mut inicio_linha);
            continue;
        }
        if ch == '"' {
            let ini = i + 1;
            i += 1;
            while i < n && c[i] != '"' && c[i] != '\n' {
                i += 1;
            }
            let t: String = c[ini..i].iter().collect();
            i = (i + 1).min(n);
            push(&mut tokens, Tipo::IdentAspas, t, linha, &mut inicio_linha);
            continue;
        }
        if ch.is_alphabetic() || ch == '_' {
            let ini = i;
            while i < n && (c[i].is_alphanumeric() || matches!(c[i], '_' | '$' | '#')) {
                i += 1;
            }
            let t: String = c[ini..i].iter().collect();
            push(&mut tokens, Tipo::Ident, t, linha, &mut inicio_linha);
            continue;
        }
        if ch.is_ascii_digit() {
            let ini = i;
            while i < n
                && (c[i].is_ascii_digit()
                    || (c[i] == '.' && c.get(i + 1).is_some_and(|d| d.is_ascii_digit())))
            {
                i += 1;
            }
            let t: String = c[ini..i].iter().collect();
            push(&mut tokens, Tipo::Numero, t, linha, &mut inicio_linha);
            continue;
        }
        push(
            &mut tokens,
            Tipo::Sim(ch),
            ch.to_string(),
            linha,
            &mut inicio_linha,
        );
        i += 1;
    }
    Lexico {
        tokens,
        comentarios,
    }
}

fn palavra_sqlplus(c: &[char]) -> bool {
    let palavra: String = c
        .iter()
        .take_while(|x| x.is_ascii_alphabetic())
        .collect::<String>()
        .to_ascii_uppercase();
    let depois = c.get(palavra.chars().count());
    let fim_palavra = depois.is_none_or(|x| x.is_whitespace());
    fim_palavra && matches!(palavra.as_str(), "REM" | "REMARK" | "PROMPT" | "SPOOL")
}

// ---------------------------------------------------------------------------
// Análise

/// Palavras reservadas e funções nativas: nunca viram referência de chamada.
const NATIVAS: &[&str] = &[
    "ABS",
    "ADD_MONTHS",
    "AND",
    "ANY",
    "AS",
    "ASCII",
    "AVG",
    "BETWEEN",
    "BINARY_INTEGER",
    "BLOB",
    "BOOLEAN",
    "BY",
    "CASE",
    "CAST",
    "CEIL",
    "CHAR",
    "CHECK",
    "CHR",
    "CLOB",
    "COALESCE",
    "COMMIT",
    "CONCAT",
    "CONSTRAINT",
    "CONTINUE",
    "COUNT",
    "CURRENT_DATE",
    "CURRENT_TIMESTAMP",
    "DATE",
    "DECIMAL",
    "DECODE",
    "DEFAULT",
    "DELETE",
    "DENSE_RANK",
    "DISTINCT",
    "ELSE",
    "ELSIF",
    "END",
    "EXCEPTION",
    "EXISTS",
    "EXIT",
    "EXTRACT",
    "FALSE",
    "FETCH",
    "FIRST_VALUE",
    "FLOAT",
    "FLOOR",
    "FOR",
    "FORALL",
    "FROM",
    "GOTO",
    "GREATEST",
    "GROUP",
    "HAVING",
    "IF",
    "IN",
    "INITCAP",
    "INSERT",
    "INSTR",
    "INT",
    "INTEGER",
    "INTERVAL",
    "INTO",
    "IS",
    "KEY",
    "LAG",
    "LAST_DAY",
    "LAST_VALUE",
    "LEAD",
    "LEAST",
    "LENGTH",
    "LIKE",
    "LISTAGG",
    "LOOP",
    "LOWER",
    "LPAD",
    "LTRIM",
    "MAX",
    "MERGE",
    "MIN",
    "MOD",
    "MONTHS_BETWEEN",
    "NCHAR",
    "NEXT_DAY",
    "NOT",
    "NULL",
    "NULLIF",
    "NUMBER",
    "NUMERIC",
    "NVARCHAR2",
    "NVL",
    "NVL2",
    "OF",
    "ON",
    "OPEN",
    "OR",
    "ORDER",
    "OVER",
    "PARTITION",
    "PLS_INTEGER",
    "POWER",
    "PRAGMA",
    "PRIMARY",
    "RAISE",
    "RAISE_APPLICATION_ERROR",
    "RANK",
    "RAW",
    "REAL",
    "REFERENCES",
    "REGEXP_INSTR",
    "REGEXP_LIKE",
    "REGEXP_REPLACE",
    "REGEXP_SUBSTR",
    "REPLACE",
    "RETURN",
    "RETURNING",
    "ROLLBACK",
    "ROUND",
    "ROWID",
    "ROWNUM",
    "ROW_NUMBER",
    "RPAD",
    "RTRIM",
    "SAVEPOINT",
    "SELECT",
    "SET",
    "SIGN",
    "SMALLINT",
    "SQLCODE",
    "SQLERRM",
    "SQRT",
    "SUBSTR",
    "SUM",
    "SYSDATE",
    "SYSTIMESTAMP",
    "TABLE",
    "THEN",
    "TIMESTAMP",
    "TO_CHAR",
    "TO_DATE",
    "TO_NUMBER",
    "TO_TIMESTAMP",
    "TRANSLATE",
    "TRIM",
    "TRUE",
    "TRUNC",
    "UNIQUE",
    "UPDATE",
    "UPPER",
    "USING",
    "VALUES",
    "VARCHAR",
    "VARCHAR2",
    "WHEN",
    "WHERE",
    "WHILE",
    "WITH",
];

/// Pacotes do Oracle que não interessam ao mapa.
const PREFIXOS_NATIVOS: &[&str] = &["DBMS_", "UTL_", "APEX_", "OWA_", "HTP.", "HTF.", "SYS."];

/// Palavras que encerram a lista de tabelas de um `FROM`.
const FIM_DE_FROM: &[&str] = &[
    "WHERE",
    "GROUP",
    "ORDER",
    "HAVING",
    "CONNECT",
    "START",
    "UNION",
    "INTERSECT",
    "MINUS",
    "EXCEPT",
    "JOIN",
    "INNER",
    "LEFT",
    "RIGHT",
    "FULL",
    "CROSS",
    "NATURAL",
    "ON",
    "USING",
    "FOR",
    "INTO",
    "WHEN",
    "THEN",
    "SET",
    "VALUES",
    "RETURNING",
    "FETCH",
    "OFFSET",
    "LIMIT",
    "PIVOT",
    "UNPIVOT",
    "MODEL",
    "WINDOW",
    "SELECT",
    "AS",
    "OUTER",
    "APPLY",
    "WITH",
    "PARTITION",
    "SAMPLE",
];

fn nativa(nome: &str) -> bool {
    let up = nome.to_uppercase();
    NATIVAS.binary_search(&up.as_str()).is_ok()
        || PREFIXOS_NATIVOS.iter().any(|p| up.starts_with(p))
}

/// Contexto da unidade PL/SQL em análise: de onde partem as referências e quais
/// nomes são locais (parâmetros, variáveis, cursores).
#[derive(Clone, Default)]
struct Contexto {
    origem: Option<String>,
    locais: HashSet<String>,
}

impl Contexto {
    fn de(origem: Option<String>) -> Self {
        Self {
            origem,
            locais: HashSet::new(),
        }
    }
}

struct Analisador<'a> {
    t: &'a [Token],
    i: usize,
    comentarios: &'a BTreeMap<u32, String>,
    saida: Extracao,
    /// `COMMENT ON` a aplicar no fim: (nome qualificado em maiúsculas, texto).
    pendentes: Vec<(String, String)>,
}

impl Analisador<'_> {
    fn fim(&self) -> bool {
        self.i >= self.t.len()
    }

    fn kw(&self, i: usize) -> &str {
        match self.t.get(i) {
            Some(t) if t.tipo == Tipo::Ident => &t.chave,
            _ => "",
        }
    }

    fn sim(&self, i: usize, c: char) -> bool {
        matches!(self.t.get(i), Some(t) if t.tipo == Tipo::Sim(c))
    }

    fn eh_ident(&self, i: usize) -> bool {
        matches!(
            self.t.get(i),
            Some(t) if matches!(t.tipo, Tipo::Ident | Tipo::IdentAspas)
        )
    }

    fn linha(&self, i: usize) -> u32 {
        self.t
            .get(i)
            .or(self.t.last())
            .map(|t| t.linha)
            .unwrap_or(1)
    }

    /// Fim de instrução: `;` ou `/` sozinho na linha.
    fn terminador(&self, i: usize) -> bool {
        if self.sim(i, ';') {
            return true;
        }
        self.sim(i, '/')
            && self.t[i].primeiro
            && self.t.get(i + 1).is_none_or(|p| p.linha != self.t[i].linha)
    }

    /// `CREATE` no começo de linha: início de outra instrução, mesmo sem o `;`
    /// da anterior.
    fn novo_create(&self, i: usize) -> bool {
        self.kw(i) == "CREATE" && self.t[i].primeiro
    }

    /// Nome possivelmente qualificado (`a.b.c`) a partir de `i`: (nome como
    /// escrito, próximo índice, linha).
    fn nome(&self, i: usize) -> Option<(String, usize, u32)> {
        if !self.eh_ident(i) {
            return None;
        }
        let linha = self.t[i].linha;
        let mut s = self.t[i].texto.clone();
        let mut j = i + 1;
        while self.sim(j, '.') && self.eh_ident(j + 1) {
            s.push('.');
            s.push_str(&self.t[j + 1].texto);
            j += 2;
        }
        // Database link: tabela@link.
        if self.sim(j, '@') && self.eh_ident(j + 1) {
            j += 2;
        }
        Some((s, j, linha))
    }

    /// Avança até depois do terminador da instrução, colhendo referências.
    fn ate_o_fim(&mut self, ctx: &mut Contexto) -> u32 {
        let mut em_merge = false;
        while !self.fim() {
            if self.terminador(self.i) {
                let l = self.linha(self.i);
                self.i += 1;
                return l;
            }
            if self.novo_create(self.i) {
                return self.linha(self.i.saturating_sub(1));
            }
            if self.kw(self.i) == "MERGE" {
                em_merge = true;
            }
            self.referencia(ctx, em_merge);
        }
        self.linha(self.t.len().saturating_sub(1))
    }

    fn refer(&mut self, ctx: &Contexto, alvo: String, tipo: TipoReferencia, linha: u32) {
        if ctx.locais.contains(&alvo.to_uppercase()) {
            return;
        }
        self.saida.referencias.push(Referencia {
            origem: ctx.origem.clone(),
            alvo,
            tipo,
            linha,
        });
    }

    /// Examina o token atual como possível referência e avança ao menos um token.
    fn referencia(&mut self, ctx: &mut Contexto, em_merge: bool) {
        let i = self.i;
        let k = self.kw(i).to_string();
        let anterior = if i > 0 {
            self.kw(i - 1).to_string()
        } else {
            String::new()
        };
        match k.as_str() {
            "FROM" if !self.clausula_de_funcao(i) => {
                let tipo = if anterior == "DELETE" {
                    TipoReferencia::Escrita
                } else {
                    TipoReferencia::Leitura
                };
                self.i = self.lista_de_tabelas(ctx, i + 1, tipo);
                return;
            }
            "JOIN" => {
                self.i = self.tabela(ctx, i + 1, TipoReferencia::Leitura);
                return;
            }
            "USING" if em_merge => {
                self.i = self.tabela(ctx, i + 1, TipoReferencia::Leitura);
                return;
            }
            "INTO" if matches!(anterior.as_str(), "INSERT" | "MERGE") => {
                self.i = self.tabela(ctx, i + 1, TipoReferencia::Escrita);
                return;
            }
            "UPDATE" if !matches!(anterior.as_str(), "FOR" | "THEN" | "KEY") => {
                self.i = self.tabela(ctx, i + 1, TipoReferencia::Escrita);
                return;
            }
            "TABLE" if anterior == "TRUNCATE" => {
                self.i = self.tabela(ctx, i + 1, TipoReferencia::Escrita);
                return;
            }
            "CALL" | "EXEC" | "EXECUTE" if self.kw(i + 1) != "IMMEDIATE" => {
                if let Some((nome, j, l)) = self.nome(i + 1)
                    && !nativa(&nome)
                {
                    self.refer(ctx, nome, TipoReferencia::Chamada, l);
                    self.i = j;
                    return;
                }
            }
            _ => {}
        }
        if self.t[i].tipo == Tipo::Ident || self.t[i].tipo == Tipo::IdentAspas {
            // Parte de um nome qualificado já visto (`x.y`): pula.
            if i > 0 && self.sim(i - 1, '.') {
                self.i += 1;
                return;
            }
            let Some((nome, j, l)) = self.nome(i) else {
                self.i += 1;
                return;
            };
            let up = nome.to_uppercase();
            if up.ends_with(".NEXTVAL") || up.ends_with(".CURRVAL") {
                let base = nome.rsplit_once('.').map_or(nome.as_str(), |(b, _)| b);
                self.refer(ctx, base.to_string(), TipoReferencia::Uso, l);
            } else if self.sim(j, '%') && matches!(self.kw(j + 1), "TYPE" | "ROWTYPE") {
                let primeiro = up.split('.').next().unwrap_or("");
                if !ctx.locais.contains(primeiro) {
                    self.refer(ctx, nome, TipoReferencia::Tipo, l);
                }
                self.i = j + 2;
                return;
            } else if self.sim(j, '(') && !nativa(&nome) && !nativa(nome_final(&nome)) {
                let primeiro = up.split('.').next().unwrap_or("");
                if !ctx.locais.contains(primeiro) {
                    self.refer(ctx, nome, TipoReferencia::Chamada, l);
                }
            } else if self.sim(j, ';')
                && !nativa(&nome)
                && (i == 0
                    || self.sim(i - 1, ';')
                    || matches!(anterior.as_str(), "BEGIN" | "THEN" | "ELSE" | "LOOP"))
            {
                // Chamada de procedure sem argumentos: `PRC_X;`.
                self.refer(ctx, nome, TipoReferencia::Chamada, l);
            }
            self.i = j;
            return;
        }
        self.i += 1;
    }

    /// `EXTRACT(YEAR FROM x)` e `TRIM(LEADING ' ' FROM x)`: o `FROM` não é de tabela.
    fn clausula_de_funcao(&self, i: usize) -> bool {
        let mut prof = 0i32;
        let mut j = i;
        while j > 0 && i - j < 40 {
            j -= 1;
            if self.sim(j, ')') {
                prof += 1;
            } else if self.sim(j, '(') {
                if prof == 0 {
                    return j > 0 && matches!(self.kw(j - 1), "EXTRACT" | "TRIM" | "SUBSTRING");
                }
                prof -= 1;
            } else if self.sim(j, ';') {
                return false;
            }
        }
        false
    }

    /// Uma tabela (com apelido opcional) a partir de `i`. Subconsulta entre
    /// parênteses não é tabela: devolve `i` para a varredura entrar nela.
    fn tabela(&mut self, ctx: &Contexto, i: usize, tipo: TipoReferencia) -> usize {
        match self.nome(i) {
            Some((nome, j, l)) if !FIM_DE_FROM.contains(&self.kw(i)) && self.kw(i) != "SET" => {
                self.refer(ctx, nome, tipo, l);
                // Apelido.
                let mut j = j;
                if self.kw(j) == "AS" {
                    j += 1;
                }
                if self.eh_ident(j) && !FIM_DE_FROM.contains(&self.kw(j)) && !nativa(self.kw(j)) {
                    j += 1;
                }
                j
            }
            _ => i,
        }
    }

    fn lista_de_tabelas(&mut self, ctx: &Contexto, i: usize, tipo: TipoReferencia) -> usize {
        let mut j = self.tabela(ctx, i, tipo);
        while j != i && self.sim(j, ',') {
            let k = self.tabela(ctx, j + 1, tipo);
            if k == j + 1 {
                return k;
            }
            j = k;
        }
        j
    }

    // -- Nível do arquivo ---------------------------------------------------

    fn arquivo(&mut self) {
        while !self.fim() {
            match self.kw(self.i) {
                "CREATE" => self.create(),
                "ALTER" => self.alter(),
                "COMMENT" if self.kw(self.i + 1) == "ON" => self.comment(),
                "DECLARE" | "BEGIN" => {
                    let mut ctx = Contexto::default();
                    self.declaracoes_e_bloco(&mut ctx);
                }
                _ if self.terminador(self.i) => self.i += 1,
                _ => {
                    let mut ctx = Contexto::default();
                    self.ate_o_fim(&mut ctx);
                }
            }
        }
    }

    fn create(&mut self) {
        let inicio = self.i;
        let linha = self.linha(inicio);
        self.i += 1;
        let mut corpo = false;
        let mut materializada = false;
        loop {
            match self.kw(self.i) {
                "OR" | "REPLACE" | "EDITIONABLE" | "NONEDITIONABLE" | "EDITIONING" | "FORCE"
                | "NO" | "GLOBAL" | "TEMPORARY" | "PUBLIC" | "UNIQUE" | "BITMAP" | "PRIVATE"
                | "IF" | "NOT" | "EXISTS" => self.i += 1,
                "MATERIALIZED" => {
                    materializada = true;
                    self.i += 1;
                }
                _ => break,
            }
        }
        let obj = self.kw(self.i).to_string();
        self.i += 1;
        if matches!(obj.as_str(), "PACKAGE" | "TYPE") && self.kw(self.i) == "BODY" {
            corpo = true;
            self.i += 1;
        }
        // `IF NOT EXISTS` depois do tipo (PostgreSQL, SQLite).
        while matches!(self.kw(self.i), "IF" | "NOT" | "EXISTS") {
            self.i += 1;
        }
        let Some((escrito, j, _)) = self.nome(self.i) else {
            let mut ctx = Contexto::default();
            self.ate_o_fim(&mut ctx);
            return;
        };
        let nome = nome_final(&escrito).to_string();
        self.i = j;
        let comentario = self.comentario_antes(linha);
        match obj.as_str() {
            "TABLE" => self.tabela_ddl(nome, linha, comentario),
            "VIEW" => {
                let mut ctx = Contexto::de(Some(nome.clone()));
                let fim = self.ate_o_fim(&mut ctx);
                let mut s = Simbolo::novo(nome, TipoSimbolo::Visao, Linhas::new(linha, fim));
                s.comentario = comentario;
                if materializada {
                    s.detalhe = Some("materializada".into());
                }
                self.saida.simbolos.push(s);
            }
            "SEQUENCE" | "INDEX" | "SYNONYM" => {
                let tipo = match obj.as_str() {
                    "SEQUENCE" => TipoSimbolo::Sequencia,
                    "INDEX" => TipoSimbolo::Indice,
                    _ => TipoSimbolo::Sinonimo,
                };
                let pos = self.saida.simbolos.len();
                let ctx = Contexto::de(Some(nome.clone()));
                let ini = self.i;
                // Índice: `ON tabela`; sinônimo: `FOR alvo`.
                let mut detalhe = None;
                if matches!(self.kw(ini), "ON" | "FOR")
                    && let Some((alvo, _, l)) = self.nome(ini + 1)
                {
                    detalhe = Some(format!("{} {}", self.kw(ini), alvo));
                    self.refer(&ctx, alvo, TipoReferencia::Uso, l);
                }
                while !self.fim() && !self.terminador(self.i) && !self.novo_create(self.i) {
                    self.i += 1;
                }
                let fim = self.linha(self.i.min(self.t.len().saturating_sub(1)));
                if self.terminador(self.i) {
                    self.i += 1;
                }
                let mut s = Simbolo::novo(nome, tipo, Linhas::new(linha, fim));
                s.comentario = comentario;
                s.detalhe = detalhe;
                self.saida.simbolos.insert(pos, s);
            }
            "PACKAGE" => self.pacote(nome, linha, corpo, comentario, TipoSimbolo::Pacote),
            "TYPE" if corpo => self.pacote(nome, linha, true, comentario, TipoSimbolo::Tipo),
            "TYPE" => {
                let mut ctx = Contexto::de(Some(nome.clone()));
                let fim = self.ate_o_fim(&mut ctx);
                let mut s = Simbolo::novo(nome, TipoSimbolo::Tipo, Linhas::new(linha, fim));
                s.comentario = comentario;
                self.saida.simbolos.push(s);
            }
            "PROCEDURE" | "FUNCTION" => {
                self.i = inicio;
                // Reposiciona no FUNCTION/PROCEDURE para a análise comum.
                while self.kw(self.i) != obj {
                    self.i += 1;
                }
                self.subprograma(None, linha, comentario);
            }
            "TRIGGER" => self.trigger(nome, linha, comentario),
            _ => {
                let mut ctx = Contexto::default();
                self.ate_o_fim(&mut ctx);
            }
        }
    }

    fn tabela_ddl(&mut self, nome: String, linha: u32, comentario: Option<String>) {
        let ctx = Contexto::de(Some(nome.clone()));
        let pos = self.saida.simbolos.len();
        let mut colunas = Vec::new();
        if self.sim(self.i, '(') {
            self.i += 1;
            loop {
                // Um elemento: até `,` ou `)` no nível 1.
                let ini = self.i;
                let mut prof = 0;
                while !self.fim() {
                    if self.sim(self.i, '(') {
                        prof += 1;
                    } else if self.sim(self.i, ')') {
                        if prof == 0 {
                            break;
                        }
                        prof -= 1;
                    } else if (prof == 0 && self.sim(self.i, ','))
                        || self.terminador(self.i)
                        || self.novo_create(self.i)
                    {
                        break;
                    }
                    self.i += 1;
                }
                let fim_el = self.i;
                if fim_el > ini {
                    self.elemento_da_tabela(&ctx, &nome, ini, fim_el, &mut colunas);
                }
                if self.sim(self.i, ',') {
                    self.i += 1;
                    continue;
                }
                if self.sim(self.i, ')') {
                    self.i += 1;
                }
                break;
            }
        }
        // Resto da instrução (`AS SELECT`, armazenamento, partição).
        let mut ctx = ctx;
        let fim = self.ate_o_fim(&mut ctx);
        let mut s = Simbolo::novo(nome, TipoSimbolo::Tabela, Linhas::new(linha, fim));
        s.comentario = comentario;
        self.saida.simbolos.insert(pos, s);
        self.saida.simbolos.extend(colunas);
    }

    fn elemento_da_tabela(
        &mut self,
        ctx: &Contexto,
        tabela: &str,
        ini: usize,
        fim: usize,
        colunas: &mut Vec<Simbolo>,
    ) {
        // Chave estrangeira, de restrição ou na própria coluna.
        for k in ini..fim {
            if self.kw(k) == "REFERENCES"
                && let Some((alvo, _, l)) = self.nome(k + 1)
            {
                self.refer(ctx, alvo, TipoReferencia::ChaveEstrangeira, l);
            }
        }
        let primeira = self.kw(ini);
        if matches!(
            primeira,
            "CONSTRAINT" | "PRIMARY" | "FOREIGN" | "UNIQUE" | "CHECK" | "INDEX" | "KEY" | "PERIOD"
        ) || !self.eh_ident(ini)
        {
            return;
        }
        let nome = self.t[ini].texto.clone();
        let detalhe = juntar(&self.t[ini + 1..fim]);
        let linhas = Linhas::new(self.t[ini].linha, self.t[fim - 1].linha);
        let mut c = Simbolo::novo(nome, TipoSimbolo::Coluna, linhas).dentro_de(tabela);
        if !detalhe.is_empty() {
            c.detalhe = Some(detalhe);
        }
        c.comentario = self.comentario_antes(linhas.inicio);
        colunas.push(c);
    }

    fn alter(&mut self) {
        let mut ctx = Contexto::default();
        if self.kw(self.i + 1) == "TABLE"
            && let Some((nome, j, _)) = self.nome(self.i + 2)
        {
            ctx.origem = Some(nome_final(&nome).to_string());
            self.i = j;
            while !self.fim() && !self.terminador(self.i) && !self.novo_create(self.i) {
                if self.kw(self.i) == "REFERENCES"
                    && let Some((alvo, j, l)) = self.nome(self.i + 1)
                {
                    self.refer(&ctx, alvo, TipoReferencia::ChaveEstrangeira, l);
                    self.i = j;
                    continue;
                }
                self.i += 1;
            }
            if self.terminador(self.i) {
                self.i += 1;
            }
            return;
        }
        self.ate_o_fim(&mut ctx);
    }

    /// `COMMENT ON TABLE x IS '...'` e `COMMENT ON COLUMN x.c IS '...'`.
    fn comment(&mut self) {
        let alvo = self.nome(self.i + 3);
        if matches!(self.kw(self.i + 2), "TABLE" | "COLUMN" | "VIEW")
            && let Some((nome, j, _)) = alvo
            && self.kw(j) == "IS"
            && let Some(t) = self.t.get(j + 1)
            && t.tipo == Tipo::Texto
        {
            let partes: Vec<&str> = nome.split('.').collect();
            // Sem o esquema: TABELA ou TABELA.COLUNA.
            let corte = if self.kw(self.i + 2) == "COLUMN" {
                2
            } else {
                1
            };
            let chave = partes[partes.len().saturating_sub(corte)..]
                .join(".")
                .to_uppercase();
            self.pendentes.push((chave, literal(&t.texto)));
        }
        while !self.fim() && !self.terminador(self.i) && !self.novo_create(self.i) {
            self.i += 1;
        }
        if self.terminador(self.i) {
            self.i += 1;
        }
    }

    fn aplicar_comentarios(&mut self) {
        for (chave, texto) in std::mem::take(&mut self.pendentes) {
            if let Some(s) = self
                .saida
                .simbolos
                .iter_mut()
                .find(|s| s.qualificado.to_uppercase() == chave)
            {
                s.comentario = Some(resumir(&texto));
            }
        }
    }

    /// Comentário em linhas inteiras logo acima de `linha`.
    fn comentario_antes(&self, linha: u32) -> Option<String> {
        let mut partes = Vec::new();
        let mut l = linha.checked_sub(1)?;
        while let Some(c) = self.comentarios.get(&l) {
            partes.push(c.as_str());
            l = match l.checked_sub(1) {
                Some(x) if x > 0 => x,
                _ => break,
            };
        }
        partes.reverse();
        let texto = partes
            .iter()
            .map(|p| p.trim().trim_start_matches('*').trim())
            .filter(|p| !p.is_empty() && !p.chars().all(|c| matches!(c, '=' | '-' | '*' | '#')))
            .collect::<Vec<_>>()
            .join(" ");
        (!texto.is_empty()).then(|| resumir(&texto))
    }

    // -- PL/SQL -------------------------------------------------------------

    /// Pacote (especificação ou corpo) ou corpo de tipo, já depois do nome.
    fn pacote(
        &mut self,
        nome: String,
        linha: u32,
        corpo: bool,
        comentario: Option<String>,
        tipo: TipoSimbolo,
    ) {
        let pos = self.saida.simbolos.len();
        let mut ctx = Contexto::de(Some(nome.clone()));
        // Cabeçalho até IS/AS.
        while !self.fim() && !matches!(self.kw(self.i), "IS" | "AS") && !self.terminador(self.i) {
            self.i += 1;
        }
        if matches!(self.kw(self.i), "IS" | "AS") {
            self.i += 1;
        }
        let mut fim = linha;
        while !self.fim() {
            if self.novo_create(self.i) {
                fim = self.linha(self.i.saturating_sub(1));
                break;
            }
            match self.kw(self.i) {
                "FUNCTION" | "PROCEDURE" => {
                    let l = self.linha(self.i);
                    let com = self.comentario_antes(l);
                    let especificacao = !corpo;
                    let antes = self.saida.simbolos.len();
                    self.subprograma(Some(&nome), l, com);
                    if especificacao {
                        for s in &mut self.saida.simbolos[antes..] {
                            s.declaracao = true;
                        }
                    }
                }
                "MEMBER" | "STATIC" | "CONSTRUCTOR" | "MAP" | "ORDER" | "OVERRIDING" | "FINAL"
                | "INSTANTIABLE" | "NOT" => self.i += 1,
                "BEGIN" => {
                    // Inicialização do pacote, que termina no END do pacote.
                    fim = self.bloco(&mut ctx);
                    break;
                }
                "END" => {
                    self.i += 1;
                    if self.eh_ident(self.i) {
                        self.i += 1;
                    }
                    fim = self.linha(self.i);
                    if self.terminador(self.i) {
                        self.i += 1;
                    }
                    break;
                }
                _ => {
                    // Declaração do pacote (variável, cursor, tipo, exceção).
                    if self.eh_ident(self.i) {
                        ctx.locais.insert(self.kw(self.i).to_string());
                    }
                    fim = self.ate_o_fim(&mut ctx);
                }
            }
        }
        let mut s = Simbolo::novo(nome, tipo, Linhas::new(linha, fim));
        s.comentario = comentario;
        s.declaracao = !corpo;
        s.detalhe = Some(if corpo { "corpo" } else { "especificação" }.into());
        self.saida.simbolos.insert(pos, s);
    }

    /// FUNCTION ou PROCEDURE em `self.i`, solta (`pai` vazio) ou num pacote. A
    /// linha inicial vem de quem chama (a do `CREATE`, na rotina solta).
    fn subprograma(&mut self, pai: Option<&str>, linha: u32, comentario: Option<String>) {
        let tipo = if self.kw(self.i) == "FUNCTION" {
            TipoSimbolo::Funcao
        } else {
            TipoSimbolo::Procedure
        };
        self.i += 1;
        let Some((escrito, j, _)) = self.nome(self.i) else {
            return;
        };
        let nome = nome_final(&escrito).to_string();
        self.i = j;
        let mut s = Simbolo::novo(nome, tipo, Linhas::new(linha, linha));
        if let Some(p) = pai {
            s = s.dentro_de(p);
        }
        s.comentario = comentario;
        let pos = self.saida.simbolos.len();
        let mut ctx = Contexto::de(Some(s.qualificado.clone()));
        // Cabeçalho: parâmetros e retorno, até IS/AS ou `;`.
        let ini = self.i;
        let mut prof = 0;
        while !self.fim() {
            if self.sim(self.i, '(') {
                prof += 1;
                if self.eh_ident(self.i + 1) {
                    ctx.locais.insert(self.kw(self.i + 1).to_string());
                }
            } else if self.sim(self.i, ')') {
                prof -= 1;
            } else if prof == 1 && self.sim(self.i, ',') && self.eh_ident(self.i + 1) {
                ctx.locais.insert(self.kw(self.i + 1).to_string());
            } else if (prof == 0
                && (matches!(self.kw(self.i), "IS" | "AS") || self.terminador(self.i)))
                || self.novo_create(self.i)
            {
                break;
            }
            self.i += 1;
        }
        let cabecalho = juntar(&self.t[ini..self.i.min(self.t.len())]);
        if !cabecalho.is_empty() {
            s.detalhe = Some(cabecalho);
        }
        // Parâmetros com %TYPE.
        let mut k = ini;
        while k < self.i {
            if self.sim(k, '%')
                && matches!(self.kw(k + 1), "TYPE" | "ROWTYPE")
                && let Some(n) = self.nome_antes(k)
            {
                let l = self.linha(k);
                self.refer(&ctx, n, TipoReferencia::Tipo, l);
            }
            k += 1;
        }
        let fim = if self.terminador(self.i) {
            // Só a declaração (especificação ou declaração antecipada).
            s.declaracao = true;
            let l = self.linha(self.i);
            self.i += 1;
            l
        } else if matches!(self.kw(self.i), "IS" | "AS") {
            self.i += 1;
            if matches!(self.kw(self.i), "LANGUAGE" | "EXTERNAL") {
                self.ate_o_fim(&mut ctx)
            } else {
                self.declaracoes_e_bloco(&mut ctx)
            }
        } else {
            self.linha(self.i.saturating_sub(1))
        };
        s.linhas = Linhas::new(linha, fim);
        self.saida.simbolos.insert(pos, s);
    }

    /// Nome qualificado que termina logo antes de `k` (para `x.y%TYPE`).
    fn nome_antes(&self, k: usize) -> Option<String> {
        let mut j = k.checked_sub(1)?;
        if !self.eh_ident(j) {
            return None;
        }
        while j >= 2 && self.sim(j - 1, '.') && self.eh_ident(j - 2) {
            j -= 2;
        }
        self.nome(j).map(|(n, _, _)| n)
    }

    /// Seção de declarações (opcional, com `DECLARE` ou depois de IS/AS) e o bloco
    /// `BEGIN ... END`. Devolve a linha final.
    fn declaracoes_e_bloco(&mut self, ctx: &mut Contexto) -> u32 {
        if self.kw(self.i) == "DECLARE" {
            self.i += 1;
        }
        while !self.fim() {
            if self.novo_create(self.i) {
                return self.linha(self.i.saturating_sub(1));
            }
            match self.kw(self.i) {
                "BEGIN" => return self.bloco(ctx),
                "FUNCTION" | "PROCEDURE" => {
                    let l = self.linha(self.i);
                    let com = self.comentario_antes(l);
                    let pai = ctx.origem.clone();
                    self.subprograma(pai.as_deref(), l, com);
                }
                // Rotina sem BEGIN (corpo vazio, erro de sintaxe): termina no END.
                "END" => {
                    self.i += 1;
                    if self.eh_ident(self.i) {
                        self.i += 1;
                    }
                    let l = self.linha(self.i);
                    if self.terminador(self.i) {
                        self.i += 1;
                    }
                    return l;
                }
                _ => {
                    if self.eh_ident(self.i) {
                        let k = self.kw(self.i);
                        let local = if matches!(k, "CURSOR" | "TYPE" | "SUBTYPE") {
                            self.kw(self.i + 1)
                        } else {
                            k
                        };
                        ctx.locais.insert(local.to_string());
                    }
                    self.ate_o_fim(ctx);
                }
            }
        }
        self.linha(self.t.len().saturating_sub(1))
    }

    /// Bloco a partir do BEGIN até o END que o fecha (e o `;`). Devolve a linha
    /// final.
    fn bloco(&mut self, ctx: &mut Contexto) -> u32 {
        let mut prof = 0i32;
        let mut em_merge = false;
        while !self.fim() {
            if self.novo_create(self.i) {
                return self.linha(self.i.saturating_sub(1));
            }
            match self.kw(self.i) {
                "BEGIN" | "CASE" => {
                    prof += 1;
                    self.i += 1;
                    continue;
                }
                "END" => {
                    match self.kw(self.i + 1) {
                        "IF" | "LOOP" | "WHILE" | "FOR" => {
                            self.i += 2;
                            continue;
                        }
                        "CASE" => {
                            prof -= 1;
                            self.i += 2;
                        }
                        _ => {
                            prof -= 1;
                            self.i += 1;
                        }
                    }
                    if prof <= 0 {
                        // Nome opcional depois do END.
                        if self.eh_ident(self.i) && !self.terminador(self.i) {
                            self.i += 1;
                        }
                        let l = self.linha(self.i.saturating_sub(1));
                        if self.terminador(self.i) {
                            let l = self.linha(self.i);
                            self.i += 1;
                            return l;
                        }
                        return l;
                    }
                    continue;
                }
                "MERGE" => em_merge = true,
                _ => {}
            }
            if self.sim(self.i, ';') {
                em_merge = false;
                self.i += 1;
                continue;
            }
            self.referencia(ctx, em_merge);
        }
        self.linha(self.t.len().saturating_sub(1))
    }

    fn trigger(&mut self, nome: String, linha: u32, comentario: Option<String>) {
        let pos = self.saida.simbolos.len();
        let mut ctx = Contexto::de(Some(nome.clone()));
        ctx.locais
            .extend(["NEW", "OLD", "PARENT"].map(String::from));
        let ini = self.i;
        let mut evento_fim = self.i;
        while !self.fim()
            && !matches!(self.kw(self.i), "DECLARE" | "BEGIN" | "CALL" | "COMPOUND")
            && !self.novo_create(self.i)
        {
            if self.kw(self.i) == "ON"
                && let Some((tabela, j, l)) = self.nome(self.i + 1)
            {
                self.refer(&ctx, tabela, TipoReferencia::Disparo, l);
                evento_fim = j;
                self.i = j;
                continue;
            }
            self.i += 1;
        }
        let detalhe = juntar(&self.t[ini..evento_fim.max(ini)]);
        let fim = if self.kw(self.i) == "CALL" {
            self.ate_o_fim(&mut ctx)
        } else {
            self.declaracoes_e_bloco(&mut ctx)
        };
        let mut s = Simbolo::novo(nome, TipoSimbolo::Trigger, Linhas::new(linha, fim));
        s.comentario = comentario;
        if !detalhe.is_empty() {
            s.detalhe = Some(detalhe);
        }
        self.saida.simbolos.insert(pos, s);
    }
}

/// Último segmento de um nome qualificado (sem o esquema).
fn nome_final(nome: &str) -> &str {
    nome.rsplit('.').next().unwrap_or(nome)
}

/// Junta tokens num texto compacto: `NUMBER(12,2) DEFAULT 0 NOT NULL`.
fn juntar(tokens: &[Token]) -> String {
    let mut s = String::new();
    let mut anterior: Option<&Token> = None;
    for t in tokens {
        let cola = matches!(
            (anterior.map(|a| a.tipo), t.tipo),
            (None, _)
                | (_, Tipo::Sim(')' | ',' | '.' | '%'))
                | (Some(Tipo::Sim('(' | '.' | '%')), _)
                | (Some(Tipo::Ident), Tipo::Sim('('))
        );
        if !cola {
            s.push(' ');
        }
        s.push_str(&t.texto);
        anterior = Some(t);
        if s.len() > 300 {
            s.push('…');
            break;
        }
    }
    s
}

/// Conteúdo de um literal `'...'`.
fn literal(t: &str) -> String {
    t.strip_prefix('\'')
        .and_then(|x| x.strip_suffix('\''))
        .unwrap_or(t)
        .replace("''", "'")
}

/// Texto numa linha só, cortado em 300 caracteres.
pub(crate) fn resumir(texto: &str) -> String {
    let mut s = texto.split_whitespace().collect::<Vec<_>>().join(" ");
    if s.chars().count() > 300 {
        s = s.chars().take(299).collect();
        s.push('…');
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extrair(texto: &str) -> Extracao {
        ExtratorSql
            .extrair(&Fonte {
                caminho: "x.sql",
                linguagem: Linguagem::Sql,
                texto,
            })
            .unwrap()
    }

    fn simbolo<'a>(e: &'a Extracao, q: &str) -> &'a Simbolo {
        e.simbolos
            .iter()
            .find(|s| s.qualificado == q)
            .unwrap_or_else(|| panic!("{q} não encontrado em {:#?}", e.simbolos))
    }

    fn refs(e: &Extracao) -> Vec<(Option<&str>, &str, TipoReferencia, u32)> {
        e.referencias
            .iter()
            .map(|r| (r.origem.as_deref(), r.alvo.as_str(), r.tipo, r.linha))
            .collect()
    }

    #[test]
    fn nativas_ordenadas() {
        let mut v = NATIVAS.to_vec();
        v.sort_unstable();
        assert_eq!(v, NATIVAS);
    }

    #[test]
    fn tabela_colunas_e_chaves() {
        let e = extrair(
            "-- Clientes\ncreate table app.cliente (\n  id number(10) not null,\n  -- PF ou PJ\n  tipo varchar2(3) default 'PF',\n  constraint pk primary key (id)\n);\n\
             CREATE TABLE \"Fatura\" (ID NUMBER, ID_CLIENTE NUMBER REFERENCES CLIENTE(ID));\n\
             ALTER TABLE FATURA ADD CONSTRAINT FK FOREIGN KEY (X) REFERENCES OUTRA (Y);\n\
             COMMENT ON COLUMN app.cliente.id IS 'Chave do cliente';\n",
        );
        let c = simbolo(&e, "cliente");
        assert_eq!(c.tipo, TipoSimbolo::Tabela);
        assert_eq!(c.linhas, Linhas::new(2, 7));
        assert_eq!(c.comentario.as_deref(), Some("Clientes"));
        let id = simbolo(&e, "cliente.id");
        assert_eq!(id.detalhe.as_deref(), Some("number(10) not null"));
        assert_eq!(id.comentario.as_deref(), Some("Chave do cliente"));
        let tipo = simbolo(&e, "cliente.tipo");
        assert_eq!(tipo.linhas, Linhas::new(5, 5));
        assert_eq!(tipo.comentario.as_deref(), Some("PF ou PJ"));
        assert_eq!(tipo.detalhe.as_deref(), Some("varchar2(3) default 'PF'"));
        assert!(!e.simbolos.iter().any(|s| s.nome == "constraint"));
        assert_eq!(simbolo(&e, "Fatura.ID_CLIENTE").linhas.inicio, 8);
        assert_eq!(
            refs(&e),
            [
                (
                    Some("Fatura"),
                    "CLIENTE",
                    TipoReferencia::ChaveEstrangeira,
                    8
                ),
                (Some("FATURA"), "OUTRA", TipoReferencia::ChaveEstrangeira, 9),
            ]
        );
    }

    #[test]
    fn pacote_rotinas_e_referencias() {
        let e = extrair(
            "CREATE OR REPLACE PACKAGE BODY PKG AS
  G_X NUMBER;
  FUNCTION F (P_A IN T.COL%TYPE) RETURN NUMBER IS
    V_R T%ROWTYPE;
    V_LISTA LISTA_T;
    CURSOR C_X IS SELECT * FROM TAB_CURSOR;
  BEGIN
    SELECT * INTO V_R FROM T A, U B WHERE A.ID = P_A FOR UPDATE;
    V_LISTA(1) := OUTRO_PKG.G(P_A, EXTRACT(YEAR FROM SYSDATE));
    CASE WHEN P_A > 1 THEN NULL; END CASE;
    IF P_A IS NULL THEN PRC_SEM_ARGS; END IF;
    FOR R IN C_X LOOP NULL; END LOOP;
    INSERT INTO LOG (ID) VALUES (SQ_LOG.NEXTVAL);
    UPDATE T SET X = (SELECT MAX(Y) FROM V JOIN W ON 1 = 1) WHERE 1 = 1;
    DELETE FROM D WHERE 1 = 1;
    MERGE INTO M USING S ON (1 = 1) WHEN MATCHED THEN UPDATE SET A = 1;
    DBMS_OUTPUT.PUT_LINE('x');
    RETURN G(1);
  END F;
  PROCEDURE P IS BEGIN NULL; END;
END PKG;
/
",
        );
        let pkg = simbolo(&e, "PKG");
        assert_eq!(pkg.tipo, TipoSimbolo::Pacote);
        assert_eq!(pkg.linhas, Linhas::new(1, 21));
        assert!(!pkg.declaracao);
        let f = simbolo(&e, "PKG.F");
        assert_eq!(f.tipo, TipoSimbolo::Funcao);
        assert_eq!(f.linhas, Linhas::new(3, 19));
        assert_eq!(
            f.detalhe.as_deref(),
            Some("(P_A IN T.COL%TYPE) RETURN NUMBER")
        );
        assert_eq!(simbolo(&e, "PKG.P").linhas, Linhas::new(20, 20));
        let r: Vec<_> = refs(&e)
            .into_iter()
            .map(|(o, a, t, l)| {
                assert_eq!(o, Some("PKG.F"), "{a}");
                (a, t, l)
            })
            .collect();
        use TipoReferencia::*;
        assert_eq!(
            r,
            [
                ("T.COL", Tipo, 3),
                ("T", Tipo, 4),
                ("TAB_CURSOR", Leitura, 6),
                ("T", Leitura, 8),
                ("U", Leitura, 8),
                ("OUTRO_PKG.G", Chamada, 9),
                ("PRC_SEM_ARGS", Chamada, 11),
                ("LOG", Escrita, 13),
                ("SQ_LOG", Uso, 13),
                ("T", Escrita, 14),
                ("V", Leitura, 14),
                ("W", Leitura, 14),
                ("D", Escrita, 15),
                ("M", Escrita, 16),
                ("S", Leitura, 16),
                ("G", Chamada, 18),
            ]
        );
    }

    #[test]
    fn especificacao_rotina_solta_trigger_visao() {
        let e = extrair(
            "CREATE PACKAGE PKG IS
  PROCEDURE P(A NUMBER);
END;
/
CREATE OR REPLACE PROCEDURE APP.PRC_X(P NUMBER) AS
  V NUMBER;
BEGIN
  PKG.P(P);
  BEGIN
    NULL;
  EXCEPTION WHEN OTHERS THEN NULL;
  END;
END PRC_X;
/
CREATE OR REPLACE TRIGGER TRG_F
  BEFORE UPDATE OF STATUS ON FATURA
  FOR EACH ROW
DECLARE
  V NUMBER;
BEGIN
  :NEW.X := FN_Y(:OLD.X);
END;
/
CREATE VIEW VW AS SELECT * FROM FATURA WHERE 1 = 1;
CREATE SEQUENCE SQ START WITH 1;
CREATE INDEX IX ON FATURA (STATUS);
BEGIN PRC_X(1); END;
/
",
        );
        assert!(simbolo(&e, "PKG").declaracao);
        let p = simbolo(&e, "PKG.P");
        assert!(p.declaracao);
        assert_eq!(p.linhas, Linhas::new(2, 2));
        let prc = simbolo(&e, "PRC_X");
        assert_eq!(prc.tipo, TipoSimbolo::Procedure);
        assert_eq!(prc.linhas, Linhas::new(5, 13));
        let trg = simbolo(&e, "TRG_F");
        assert_eq!(trg.linhas, Linhas::new(15, 22));
        assert_eq!(
            trg.detalhe.as_deref(),
            Some("BEFORE UPDATE OF STATUS ON FATURA")
        );
        assert_eq!(simbolo(&e, "VW").linhas, Linhas::new(24, 24));
        assert_eq!(simbolo(&e, "SQ").tipo, TipoSimbolo::Sequencia);
        assert_eq!(simbolo(&e, "IX").detalhe.as_deref(), Some("ON FATURA"));
        use TipoReferencia::*;
        assert_eq!(
            refs(&e),
            [
                (Some("PRC_X"), "PKG.P", Chamada, 8),
                (Some("TRG_F"), "FATURA", Disparo, 16),
                (Some("TRG_F"), "FN_Y", Chamada, 21),
                (Some("VW"), "FATURA", Leitura, 24),
                (Some("IX"), "FATURA", Uso, 26),
                (None, "PRC_X", Chamada, 27),
            ]
        );
    }

    #[test]
    fn tolera_sql_quebrado_e_sqlplus() {
        let e = extrair(
            "REM cabecalho\nPROMPT criando\n@outro.sql\nSET DEFINE OFF\nCREATE TABLE A (X NUMBER\nCREATE TABLE B (Y NUMBER);\nSELECT q'[it's]' FROM C;\nCREATE PROCEDURE P IS BEGIN",
        );
        assert!(e.simbolos.iter().any(|s| s.qualificado == "A"));
        assert_eq!(simbolo(&e, "B").linhas, Linhas::new(6, 6));
        assert!(
            e.referencias
                .iter()
                .any(|r| r.alvo == "C" && r.tipo == TipoReferencia::Leitura && r.linha == 7)
        );
        assert_eq!(simbolo(&e, "P").linhas.inicio, 8);
        assert!(extrair("").simbolos.is_empty());
        assert!(extrair("/* sem fim").simbolos.is_empty());
        assert!(extrair("'sem fim").simbolos.is_empty());
    }
}
