//! Os nomes que uma consulta de leitura usa (#453): as tabelas (com o apelido),
//! as colunas citadas, os apelidos que a própria consulta cria e os parâmetros
//! nomeados (`:id_fatura`).
//!
//! Serve às consultas de verificação que o agente escreve no documento da
//! análise: quem registra confere os nomes contra o modelo de dados conhecido
//! (os scripts do projeto ou o banco ligado). Além da validação só leitura
//! ([`crate::validacao`]), a leitura recusa o que põe dado real na consulta:
//!
//! - parâmetro posicional (`?`, `$1`) no lugar do nomeado;
//! - identificador comparado a um valor fixo (`ID_FATURA = 4711`,
//!   `ID_CLIENTE IN (1, 2)`): o caso entra por parâmetro;
//! - texto com cara de dado pessoal (CPF, CNPJ, e-mail).
//!
//! Valores de domínio (`STATUS = 'PARCELADA'`) e datas de corte são aceitos: são
//! a regra, não o caso.

use std::ops::ControlFlow;

use cba_repos::providers::DialetoSql;
use sqlparser::ast::{
    BinaryOperator, Expr, Ident, InTarget, ObjectName, ObjectNamePart, Query, Select, SelectItem,
    TableFactor, Value, Visit, Visitor,
};

use crate::validacao::{Recusa, validar_e_ler};

/// Uma tabela (ou visão) lida na consulta, como foi escrita.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relacao {
    /// O nome, com o esquema se veio com ele (`APP.FATURA`), sem aspas.
    pub nome: String,
    /// O apelido (`F` em `FROM FATURA F`), se houver.
    pub apelido: Option<String>,
}

/// Uma coluna citada, como foi escrita.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColunaCitada {
    /// O que vem antes do nome (`F` em `F.STATUS`, `APP.FATURA` em
    /// `APP.FATURA.STATUS`), se houver.
    pub qualificador: Option<String>,
    pub nome: String,
}

impl std::fmt::Display for ColunaCitada {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.qualificador {
            Some(q) => write!(f, "{q}.{}", self.nome),
            None => f.write_str(&self.nome),
        }
    }
}

/// A consulta validada e os nomes que ela usa, sem repetição, na ordem em que
/// aparecem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsultaLida {
    /// O SQL sem espaços nas pontas e sem o `;` final.
    pub sql: String,
    /// Tabelas e visões do banco (sem as consultas nomeadas do `WITH` nem as
    /// subconsultas do `FROM`).
    pub relacoes: Vec<Relacao>,
    /// Nomes que a própria consulta cria e que podem qualificar uma coluna: as
    /// consultas do `WITH` e os apelidos das subconsultas do `FROM`.
    pub virtuais: Vec<String>,
    pub colunas: Vec<ColunaCitada>,
    /// Apelidos de coluna criados pela consulta (`COUNT(*) AS TOTAL`, colunas
    /// declaradas no `WITH`), que podem aparecer no `ORDER BY` ou fora do `WITH`.
    pub apelidos: Vec<String>,
    /// Parâmetros nomeados, sem os dois-pontos (`id_fatura`).
    pub parametros: Vec<String>,
}

impl ConsultaLida {
    /// A tabela a que um qualificador se refere: pelo apelido ou pelo nome (com
    /// ou sem o esquema), sem diferenciar maiúsculas. `None` se o qualificador é
    /// uma consulta virtual ou não é conhecido.
    pub fn tabela_do_qualificador(&self, qualificador: &str) -> Option<&Relacao> {
        let q = qualificador.to_uppercase();
        let ultimo = |n: &str| n.rsplit('.').next().unwrap_or(n).to_uppercase();
        self.relacoes
            .iter()
            .find(|r| r.apelido.as_deref().is_some_and(|a| a.to_uppercase() == q))
            .or_else(|| {
                self.relacoes
                    .iter()
                    .find(|r| r.nome.to_uppercase() == q || ultimo(&r.nome) == ultimo(&q))
            })
    }

    /// Se o nome é uma consulta virtual (`WITH` ou subconsulta com apelido).
    pub fn e_virtual(&self, nome: &str) -> bool {
        let n = nome.to_uppercase();
        self.virtuais.iter().any(|v| v.to_uppercase() == n)
    }

    /// Se o nome é um apelido de coluna criado pela consulta.
    pub fn e_apelido(&self, nome: &str) -> bool {
        let n = nome.to_uppercase();
        self.apelidos.iter().any(|a| a.to_uppercase() == n)
    }
}

/// Valida `sql` como consulta só de leitura (no dialeto da conexão ou, sem
/// conexão, no genérico) e lê os nomes que ela usa.
pub fn ler(dialeto: Option<DialetoSql>, sql: &str) -> Result<ConsultaLida, Recusa> {
    let (texto, instrucao) = validar_e_ler(dialeto, sql)?;
    let mut leitor = Leitor::default();
    if let ControlFlow::Break(r) = instrucao.visit(&mut leitor) {
        return Err(r);
    }
    let Leitor {
        relacoes,
        virtuais,
        colunas,
        apelidos,
        parametros,
    } = leitor;
    Ok(ConsultaLida {
        sql: texto,
        relacoes,
        virtuais,
        colunas,
        apelidos,
        parametros,
    })
}

#[derive(Default)]
struct Leitor {
    relacoes: Vec<Relacao>,
    virtuais: Vec<String>,
    colunas: Vec<ColunaCitada>,
    apelidos: Vec<String>,
    parametros: Vec<String>,
}

fn sem_repetir<T: PartialEq>(lista: &mut Vec<T>, item: T) {
    if !lista.contains(&item) {
        lista.push(item);
    }
}

fn nome_do_objeto(nome: &ObjectName) -> String {
    nome.0
        .iter()
        .map(|p| match p {
            ObjectNamePart::Identifier(i) => i.value.clone(),
            outra => outra.to_string(),
        })
        .collect::<Vec<_>>()
        .join(".")
}

fn juntar(partes: &[Ident]) -> String {
    partes
        .iter()
        .map(|i| i.value.as_str())
        .collect::<Vec<_>>()
        .join(".")
}

fn recusa(texto: impl Into<String>) -> ControlFlow<Recusa> {
    ControlFlow::Break(Recusa(texto.into()))
}

/// O nome da coluna, se a expressão é uma coluna (`ID_FATURA`, `F.ID_FATURA`).
fn coluna(e: &Expr) -> Option<&str> {
    match e {
        Expr::Identifier(i) => Some(&i.value),
        Expr::CompoundIdentifier(v) => v.last().map(|i| i.value.as_str()),
        Expr::Nested(e) => coluna(e),
        _ => None,
    }
}

/// Coluna que identifica um registro: `ID`, `ID_FATURA`, `FATURA_ID`.
fn e_identificador(nome: &str) -> bool {
    let n = nome.to_uppercase();
    n == "ID" || n.starts_with("ID_") || n.ends_with("_ID")
}

/// Um valor fixo (número ou texto), e não um parâmetro nem uma expressão.
fn literal(e: &Expr) -> Option<String> {
    match e {
        Expr::Value(v) => match &v.value {
            Value::Number(n, _) => Some(n.clone()),
            Value::SingleQuotedString(s) | Value::DoubleQuotedString(s) => Some(format!("'{s}'")),
            _ => None,
        },
        Expr::Nested(e) => literal(e),
        Expr::UnaryOp { expr, .. } => literal(expr),
        _ => None,
    }
}

/// Se o texto tem cara de dado pessoal: CPF ou CNPJ (com ou sem pontuação) ou
/// e-mail.
fn parece_dado_pessoal(texto: &str) -> Option<&'static str> {
    let t = texto.trim();
    let digitos: String = t.chars().filter(char::is_ascii_digit).collect();
    let so_documento = t
        .chars()
        .all(|c| c.is_ascii_digit() || matches!(c, '.' | '-' | '/' | ' '));
    if so_documento && digitos.len() == 11 {
        return Some("CPF");
    }
    if so_documento && digitos.len() == 14 {
        return Some("CNPJ");
    }
    let mut partes = t.split('@');
    if let (Some(antes), Some(depois), None) = (partes.next(), partes.next(), partes.next())
        && !antes.is_empty()
        && !antes.contains(char::is_whitespace)
        && depois.contains('.')
        && !depois.contains(char::is_whitespace)
    {
        return Some("e-mail");
    }
    None
}

fn com_valor_fixo(coluna: &str, valor: &str) -> ControlFlow<Recusa> {
    recusa(format!(
        "`{coluna}` é comparada a um valor fixo ({valor}): o identificador do caso entra \
         por parâmetro nomeado (`:{}`), sem dado real na consulta",
        coluna.to_lowercase()
    ))
}

impl Visitor for Leitor {
    type Break = Recusa;

    fn pre_visit_query(&mut self, q: &Query) -> ControlFlow<Recusa> {
        if let Some(with) = &q.with {
            for cte in &with.cte_tables {
                sem_repetir(&mut self.virtuais, cte.alias.name.value.clone());
                for c in &cte.alias.columns {
                    sem_repetir(&mut self.apelidos, c.name.value.clone());
                }
            }
        }
        ControlFlow::Continue(())
    }

    fn pre_visit_table_factor(&mut self, t: &TableFactor) -> ControlFlow<Recusa> {
        match t {
            TableFactor::Table { name, alias, .. } => {
                let nome = nome_do_objeto(name);
                // Uma consulta do `WITH` usada no `FROM` não é tabela do banco.
                let do_with = name.0.len() == 1
                    && self.virtuais.iter().any(|v| v.eq_ignore_ascii_case(&nome));
                if do_with {
                    if let Some(a) = alias {
                        sem_repetir(&mut self.virtuais, a.name.value.clone());
                    }
                } else {
                    sem_repetir(
                        &mut self.relacoes,
                        Relacao {
                            nome,
                            apelido: alias.as_ref().map(|a| a.name.value.clone()),
                        },
                    );
                }
            }
            TableFactor::Derived { alias: Some(a), .. } => {
                sem_repetir(&mut self.virtuais, a.name.value.clone());
                for c in &a.columns {
                    sem_repetir(&mut self.apelidos, c.name.value.clone());
                }
            }
            _ => {}
        }
        ControlFlow::Continue(())
    }

    fn pre_visit_select(&mut self, s: &Select) -> ControlFlow<Recusa> {
        for item in &s.projection {
            if let SelectItem::ExprWithAlias { alias, .. } = item {
                sem_repetir(&mut self.apelidos, alias.value.clone());
            }
        }
        ControlFlow::Continue(())
    }

    fn pre_visit_expr(&mut self, e: &Expr) -> ControlFlow<Recusa> {
        match e {
            Expr::Identifier(i) => sem_repetir(
                &mut self.colunas,
                ColunaCitada {
                    qualificador: None,
                    nome: i.value.clone(),
                },
            ),
            Expr::CompoundIdentifier(partes) if partes.len() > 1 => sem_repetir(
                &mut self.colunas,
                ColunaCitada {
                    qualificador: Some(juntar(&partes[..partes.len() - 1])),
                    nome: partes[partes.len() - 1].value.clone(),
                },
            ),
            Expr::Value(v) => match &v.value {
                Value::Placeholder(p) => {
                    let nome = p.strip_prefix(':').unwrap_or_default();
                    if nome.is_empty() || !nome.chars().all(|c| c.is_alphanumeric() || c == '_') {
                        return recusa(format!(
                            "o parâmetro `{p}` não é nomeado: use dois-pontos e um nome \
                             (`:id_fatura`), que o leitor troca pelo valor do caso"
                        ));
                    }
                    sem_repetir(&mut self.parametros, nome.to_owned());
                }
                Value::SingleQuotedString(s) | Value::DoubleQuotedString(s) => {
                    if let Some(tipo) = parece_dado_pessoal(s) {
                        return recusa(format!(
                            "o valor '{s}' parece um {tipo}: a consulta não leva dado real; \
                             use um parâmetro nomeado (`:documento`)"
                        ));
                    }
                }
                _ => {}
            },
            Expr::BinaryOp {
                left,
                op: BinaryOperator::Eq | BinaryOperator::NotEq,
                right,
            } => {
                for (a, b) in [(left, right), (right, left)] {
                    if let (Some(c), Some(v)) = (coluna(a), literal(b))
                        && e_identificador(c)
                    {
                        return com_valor_fixo(c, &v);
                    }
                }
            }
            Expr::In {
                expr,
                target: InTarget::List(lista),
                ..
            } => {
                if let Some(c) = coluna(expr)
                    && e_identificador(c)
                    && let Some(v) = lista.iter().find_map(literal)
                {
                    return com_valor_fixo(c, &v);
                }
            }
            _ => {}
        }
        ControlFlow::Continue(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lida(sql: &str) -> ConsultaLida {
        match ler(None, sql) {
            Ok(c) => c,
            Err(e) => panic!("deveria aceitar {sql:?}: {e}"),
        }
    }

    fn recusada(sql: &str, trecho: &str) {
        match ler(None, sql) {
            Ok(_) => panic!("deveria recusar {sql:?}"),
            Err(Recusa(e)) => assert!(e.contains(trecho), "{sql:?}: {e}"),
        }
    }

    fn nomes(c: &ConsultaLida) -> Vec<String> {
        c.colunas.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn le_tabelas_colunas_e_parametros() {
        let c = lida(
            "SELECT F.ID_FATURA, F.STATUS, COUNT(P.NR_PARCELA) AS PARCELAS\n\
             FROM FATURA F\n\
             JOIN PARCELA P ON P.ID_FATURA = F.ID_FATURA\n\
             WHERE F.ID_FATURA = :id_fatura AND P.STATUS = 'PAGA'\n\
             GROUP BY F.ID_FATURA, F.STATUS\n\
             ORDER BY PARCELAS;",
        );
        assert!(!c.sql.ends_with(';'));
        assert_eq!(
            c.relacoes,
            vec![
                Relacao {
                    nome: "FATURA".into(),
                    apelido: Some("F".into())
                },
                Relacao {
                    nome: "PARCELA".into(),
                    apelido: Some("P".into())
                },
            ]
        );
        assert_eq!(
            nomes(&c),
            [
                "F.ID_FATURA",
                "F.STATUS",
                "P.NR_PARCELA",
                "P.ID_FATURA",
                "P.STATUS",
                "PARCELAS"
            ]
        );
        assert_eq!(c.apelidos, ["PARCELAS"]);
        assert_eq!(c.parametros, ["id_fatura"]);
        assert_eq!(c.tabela_do_qualificador("p").unwrap().nome, "PARCELA");
        assert_eq!(c.tabela_do_qualificador("FATURA").unwrap().nome, "FATURA");
        assert!(c.tabela_do_qualificador("X").is_none());
    }

    #[test]
    fn with_e_subconsulta_sao_virtuais() {
        let c = lida(
            "WITH ABERTAS (ID, ANO) AS (\n\
               SELECT ID_FATURA, EXTRACT(YEAR FROM DT_EMISSAO) FROM APP.FATURA WHERE STATUS = 'ABERTA'\n\
             )\n\
             SELECT A.ANO, COUNT(*) AS TOTAL FROM ABERTAS A\n\
             JOIN (SELECT ID_FATURA FROM PARCELA) PP ON PP.ID_FATURA = A.ID\n\
             GROUP BY A.ANO ORDER BY TOTAL DESC",
        );
        let tabelas: Vec<&str> = c.relacoes.iter().map(|r| r.nome.as_str()).collect();
        assert_eq!(tabelas, ["APP.FATURA", "PARCELA"]);
        assert!(c.e_virtual("abertas") && c.e_virtual("A") && c.e_virtual("PP"));
        assert!(c.e_apelido("ANO") && c.e_apelido("TOTAL") && c.e_apelido("ID"));
        assert_eq!(
            c.tabela_do_qualificador("FATURA").unwrap().nome,
            "APP.FATURA"
        );
    }

    #[test]
    fn aceita_sintaxe_comum_sem_dialeto() {
        lida("SELECT TRUNC(DT_EMISSAO), NVL(VL_PAGO, 0) FROM FATURA WHERE DT_EMISSAO < SYSDATE");
        lida("SELECT * FROM FATURA WHERE ID_CLIENTE = :id_cliente FETCH FIRST 10 ROWS ONLY");
        lida(
            "SELECT STATUS, COUNT(*) FROM FATURA WHERE STATUS IN ('ABERTA', 'PARCELADA') \
             AND DT_EMISSAO >= DATE '2024-01-01' GROUP BY STATUS",
        );
    }

    #[test]
    fn recusa_o_que_nao_e_leitura() {
        recusada(
            "UPDATE FATURA SET STATUS = 'PAGA'",
            "só consultas de leitura",
        );
        recusada("DELETE FROM FATURA WHERE ID_FATURA = :id", "DELETE");
        recusada("SELECT 1; DROP TABLE FATURA", "uma instrução por vez");
        recusada("SELECT * FROM FATURA FOR UPDATE", "travas");
    }

    #[test]
    fn recusa_dado_real_na_consulta() {
        recusada(
            "SELECT * FROM FATURA WHERE ID_FATURA = 4711",
            "`ID_FATURA` é comparada a um valor fixo (4711)",
        );
        recusada(
            "SELECT * FROM FATURA F WHERE 12 = F.ID_CLIENTE",
            ":id_cliente",
        );
        recusada(
            "SELECT * FROM FATURA WHERE ID_CLIENTE IN (:a, 7)",
            "valor fixo (7)",
        );
        recusada(
            "SELECT * FROM CLIENTE WHERE DOCUMENTO = '123.456.789-09'",
            "parece um CPF",
        );
        recusada(
            "SELECT * FROM CLIENTE WHERE DOCUMENTO = '12345678000199'",
            "parece um CNPJ",
        );
        recusada(
            "SELECT * FROM CONTATO WHERE EMAIL = 'fulano@exemplo.com'",
            "parece um e-mail",
        );
        recusada("SELECT * FROM FATURA WHERE ID_FATURA = ?", "não é nomeado");
    }

    #[test]
    fn valores_de_dominio_e_datas_sao_aceitos() {
        let c = lida(
            "SELECT COUNT(*) FROM FATURA WHERE STATUS = 'PARCELADA' AND VL_PAGO >= VL_ORIGINAL \
             AND DT_VENCIMENTO < DATE '2025-01-01' AND TIPO_CLIENTE <> 'PJ'",
        );
        assert!(c.parametros.is_empty());
    }
}
