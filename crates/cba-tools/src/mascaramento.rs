//! Mascaramento do resultado de uma consulta ao banco (#440), sempre, antes de
//! ele chegar ao agente. Não há exceção nem liberação.
//!
//! Duas camadas:
//!
//! - **por coluna:** a coluna com indício de dado pessoal pelo nome e pelo tipo
//!   reais ([`crate::dados_pessoais::indicio`]) sai inteira mascarada. Como o
//!   resultado não diz de que tabela veio cada coluna, as tabelas citadas no SQL
//!   (`FROM`/`JOIN`) fazem o papel da tabela: `nome` numa consulta a `cliente` é
//!   dado pessoal; numa consulta a `produto`, não;
//! - **por conteúdo,** em qualquer coluna de texto: CPF, CNPJ, e-mail e telefone
//!   viram `[cpf]`, `[cnpj]`, `[e-mail]` e `[telefone]`, e os nomes de pessoa ao
//!   lado de um documento (no mesmo texto ou na mesma linha) viram `[nome]`.
//!
//! O valor nulo continua nulo: ele não revela nada e ajuda a contar.
//! A resposta diz quais colunas foram mascaradas e por quê.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use cba_core::banco::{ColunaDados, ResultadoDados};
use regex::Regex;
use serde::Serialize;
use serde_json::Value;

use crate::dados_pessoais;

/// Valor de uma coluna mascarada por inteiro.
pub const MASCARA: &str = "[mascarado]";

/// Uma coluna mascarada e por quê.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ColunaMascarada {
    pub coluna: String,
    /// "coluna: documento de identificação" ou "conteúdo: CPF, e-mail".
    pub motivo: String,
}

/// O resultado já mascarado.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ResultadoMascarado {
    pub colunas: Vec<ColunaDados>,
    pub linhas: Vec<Vec<Value>>,
    pub mascaradas: Vec<ColunaMascarada>,
}

struct Padroes {
    cnpj: Regex,
    cpf: Regex,
    email: Regex,
    telefone: Regex,
    nome: Regex,
    nome_inteiro: Regex,
}

fn padroes() -> &'static Padroes {
    static P: OnceLock<Padroes> = OnceLock::new();
    P.get_or_init(|| {
        let nome = r"\p{Lu}\p{Ll}+(?:\s+(?:(?:da|de|do|das|dos|e)\s+)?\p{Lu}\p{Ll}+)+";
        Padroes {
            cnpj: Regex::new(r"\b\d{2}\.?\d{3}\.?\d{3}/?\d{4}-?\d{2}\b").expect("cnpj"),
            cpf: Regex::new(r"\b\d{3}\.?\d{3}\.?\d{3}-?\d{2}\b").expect("cpf"),
            email: Regex::new(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}").expect("email"),
            telefone: Regex::new(
                r"(?:\+55\s?)?(?:\(\d{2}\)\s?|\b\d{2}[\s-])9?\d{4}[\s-]?\d{4}\b|\b9\d{4}-\d{4}\b",
            )
            .expect("telefone"),
            nome: Regex::new(&format!(r"\b{nome}\b")).expect("nome"),
            nome_inteiro: Regex::new(&format!(r"^\s*{nome}\s*$")).expect("nome inteiro"),
        }
    })
}

/// O texto com os documentos, e-mails e telefones mascarados, e o que foi
/// achado. Os nomes de pessoa só saem quando há um documento ou um e-mail no
/// mesmo texto (ou `documento_na_linha`).
fn mascarar_texto(texto: &str, documento_na_linha: bool) -> (String, Vec<&'static str>) {
    let p = padroes();
    let mut achados = Vec::new();
    let mut saida = texto.to_owned();
    // CNPJ antes do CPF: os 14 dígitos contêm 11.
    for (re, rotulo, troca) in [
        (&p.cnpj, "CNPJ", "[cnpj]"),
        (&p.cpf, "CPF", "[cpf]"),
        (&p.email, "e-mail", "[e-mail]"),
        (&p.telefone, "telefone", "[telefone]"),
    ] {
        if re.is_match(&saida) {
            saida = re.replace_all(&saida, troca).into_owned();
            achados.push(rotulo);
        }
    }
    let tem_documento = achados.iter().any(|a| *a != "telefone") || documento_na_linha;
    if tem_documento && p.nome.is_match(&saida) {
        saida = p.nome.replace_all(&saida, "[nome]").into_owned();
        achados.push("nome junto de documento");
    }
    (saida, achados)
}

/// As tabelas citadas depois de `FROM` e `JOIN`, juntas, para a heurística de
/// coluna pessoal saber se a consulta é a uma tabela de pessoas.
pub fn tabelas_do_sql(sql: &str) -> String {
    let mut tabelas = Vec::new();
    let mut anterior_e_from = false;
    for token in sql.split(|c: char| c.is_whitespace() || c == ',' || c == '(' || c == ')') {
        if token.is_empty() {
            continue;
        }
        let maiusculo = token.to_uppercase();
        if anterior_e_from {
            let nome = token
                .trim_matches(|c| c == '"' || c == '`' || c == '[' || c == ']')
                .rsplit('.')
                .next()
                .unwrap_or_default()
                .trim_matches(|c| c == '"' || c == '`');
            if !nome.is_empty() && maiusculo != "SELECT" {
                tabelas.push(nome.to_owned());
            }
        }
        anterior_e_from = matches!(maiusculo.as_str(), "FROM" | "JOIN");
    }
    tabelas.join("_")
}

/// Mascara o resultado de uma consulta: as colunas com indício de dado pessoal
/// por inteiro, e o conteúdo de qualquer coluna de texto pelos padrões.
pub fn mascarar(resultado: &ResultadoDados, sql: &str) -> ResultadoMascarado {
    let tabela = tabelas_do_sql(sql);
    let por_coluna: Vec<Option<dados_pessoais::Indicio>> = resultado
        .colunas
        .iter()
        .map(|c| dados_pessoais::indicio(&tabela, &c.nome, Some(&c.tipo), None))
        .collect();
    let mut motivos: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    for (i, indicio) in por_coluna.iter().enumerate() {
        if let Some(ind) = indicio {
            motivos
                .entry(i)
                .or_default()
                .push(format!("coluna: {}", ind.categoria));
        }
    }
    let p = padroes();
    let linhas = resultado
        .linhas
        .iter()
        .map(|linha| {
            // Há documento nesta linha (coluna de documento ou conteúdo)?
            let documento_na_linha = linha.iter().enumerate().any(|(i, v)| {
                let pessoal_documento = por_coluna
                    .get(i)
                    .and_then(Option::as_ref)
                    .is_some_and(|ind| ind.categoria.contains("documento"));
                let texto = v.as_str().unwrap_or_default();
                (pessoal_documento && !v.is_null())
                    || p.cpf.is_match(texto)
                    || p.cnpj.is_match(texto)
                    || p.email.is_match(texto)
            });
            linha
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    if v.is_null() {
                        return Value::Null;
                    }
                    if por_coluna.get(i).is_some_and(Option::is_some) {
                        return Value::String(MASCARA.into());
                    }
                    let texto = match v {
                        Value::String(s) => s.clone(),
                        // Documento guardado como número (CPF em BIGINT).
                        Value::Number(n) => {
                            let t = n.to_string();
                            if t.len() == 11 || t.len() == 14 {
                                t
                            } else {
                                return v.clone();
                            }
                        }
                        outro => return outro.clone(),
                    };
                    let (mut saida, mut achados) = mascarar_texto(&texto, documento_na_linha);
                    if documento_na_linha && p.nome_inteiro.is_match(&saida) {
                        saida = "[nome]".into();
                        if !achados.contains(&"nome junto de documento") {
                            achados.push("nome junto de documento");
                        }
                    }
                    if achados.is_empty() {
                        return v.clone();
                    }
                    let m = motivos.entry(i).or_default();
                    for a in achados {
                        let a = format!("conteúdo: {a}");
                        if !m.contains(&a) {
                            m.push(a);
                        }
                    }
                    Value::String(saida)
                })
                .collect()
        })
        .collect();
    let mascaradas = motivos
        .into_iter()
        .map(|(i, m)| ColunaMascarada {
            coluna: resultado.colunas[i].nome.clone(),
            motivo: m.join("; "),
        })
        .collect();
    ResultadoMascarado {
        colunas: resultado.colunas.clone(),
        linhas,
        mascaradas,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn col(nome: &str, tipo: &str) -> ColunaDados {
        ColunaDados {
            nome: nome.into(),
            tipo: tipo.into(),
        }
    }

    #[test]
    fn padroes_de_conteudo() {
        let (t, a) = mascarar_texto(
            "Cliente Maria da Silva, CPF 123.456.789-09, maria@example.com, (11) 98765-4321",
            false,
        );
        assert_eq!(t, "[nome], CPF [cpf], [e-mail], [telefone]");
        assert_eq!(a, ["CPF", "e-mail", "telefone", "nome junto de documento"]);
        let (t, _) = mascarar_texto("Empresa 12.345.678/0001-95", false);
        assert_eq!(t, "Empresa [cnpj]");
        let (t, _) = mascarar_texto("12345678909", false);
        assert_eq!(t, "[cpf]");
        // Sem documento, nome próprio fica (pode ser um produto ou uma cidade).
        let (t, a) = mascarar_texto("Fatura de São Paulo vencida", false);
        assert_eq!(t, "Fatura de São Paulo vencida");
        assert!(a.is_empty());
        // Valores e datas não são telefone.
        let (t, _) = mascarar_texto("R$ 1234.56 em 2026-11-26", false);
        assert_eq!(t, "R$ 1234.56 em 2026-11-26");
    }

    #[test]
    fn tabelas_citadas_no_sql() {
        assert_eq!(
            tabelas_do_sql("select c.nome from app.cliente c join \"fatura\" f on f.id=c.id"),
            "cliente_fatura"
        );
        assert_eq!(tabelas_do_sql("SELECT count(*) FROM produto"), "produto");
    }

    #[test]
    fn mascara_colunas_e_conteudo_e_diz_quais() {
        let r = ResultadoDados {
            colunas: vec![
                col("id", "INT4"),
                col("nome", "VARCHAR"),
                col("cpf", "VARCHAR"),
                col("email", "VARCHAR"),
                col("motivo", "VARCHAR(80)"),
                col("situacao", "VARCHAR"),
                col("responsavel", "VARCHAR"),
            ],
            linhas: vec![
                vec![
                    json!(1),
                    json!("Maria da Silva"),
                    json!("12345678909"),
                    json!("maria@example.com"),
                    json!("Ligou João Souza, CPF 987.654.321-00, pediu 2ª via"),
                    json!("vencida"),
                    json!("Carlos Pereira"),
                ],
                vec![
                    json!(2),
                    Value::Null,
                    Value::Null,
                    Value::Null,
                    json!("sem contato"),
                    json!("paga"),
                    Value::Null,
                ],
            ],
            cortada: false,
            limite_linhas: 200,
            duracao_ms: 3,
        };
        let m = mascarar(&r, "SELECT * FROM cliente");
        let l0 = &m.linhas[0];
        assert_eq!(l0[0], json!(1));
        assert_eq!(l0[1], json!(MASCARA));
        assert_eq!(l0[2], json!(MASCARA));
        assert_eq!(l0[3], json!(MASCARA));
        assert_eq!(l0[4], json!("[nome], CPF [cpf], pediu 2ª via"));
        assert_eq!(l0[5], json!("vencida"));
        // Nome sozinho numa coluna, na linha com documento.
        assert_eq!(l0[6], json!("[nome]"));
        // Nulos ficam nulos; texto sem padrão fica.
        assert_eq!(m.linhas[1][1], Value::Null);
        assert_eq!(m.linhas[1][4], json!("sem contato"));
        let nomes: Vec<&str> = m.mascaradas.iter().map(|c| c.coluna.as_str()).collect();
        assert_eq!(nomes, ["nome", "cpf", "email", "motivo", "responsavel"]);
        assert!(m.mascaradas[3].motivo.contains("CPF"), "{:?}", m.mascaradas);
        let tudo = serde_json::to_string(&m.linhas).unwrap();
        for vazado in [
            "Maria",
            "12345678909",
            "maria@",
            "João",
            "987.654",
            "Carlos",
        ] {
            assert!(!tudo.contains(vazado), "{vazado} em {tudo}");
        }
    }

    #[test]
    fn agregado_sem_dado_pessoal_passa_inteiro() {
        let r = ResultadoDados {
            colunas: vec![col("situacao", "VARCHAR"), col("total", "INT8")],
            linhas: vec![vec![json!("vencida"), json!(312)]],
            cortada: false,
            limite_linhas: 200,
            duracao_ms: 1,
        };
        let m = mascarar(
            &r,
            "SELECT situacao, count(*) AS total FROM fatura GROUP BY situacao",
        );
        assert_eq!(m.linhas, r.linhas);
        assert!(m.mascaradas.is_empty());
    }
}
