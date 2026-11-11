//! Validação dos argumentos de uma ferramenta contra o schema de entrada.
//!
//! Implementa o subconjunto do JSON Schema que as ferramentas usam: `type` (inclusive
//! lista de tipos), `enum`, `properties`, `required`, `additionalProperties: false`,
//! `items`, `minLength`, `maxLength`, `minimum`, `maximum`, `minItems` e `maxItems`.
//! Outras palavras-chave são ignoradas. As mensagens citam o caminho do campo
//! (`evidencias[0].linha_inicial`), para o agente corrigir a chamada.

use serde_json::{Map, Value};

/// Confere `valor` contra `schema`. Devolve a primeira divergência.
pub fn validar(valor: &Value, schema: &Value) -> Result<(), String> {
    validar_em(valor, schema, "")
}

fn nome(caminho: &str) -> String {
    if caminho.is_empty() {
        "os argumentos".to_owned()
    } else {
        format!("`{caminho}`")
    }
}

fn tipo_de(valor: &Value) -> &'static str {
    match valor {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(n) if n.is_i64() || n.is_u64() => "integer",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn aceita(tipo: &str, valor: &Value) -> bool {
    let real = tipo_de(valor);
    real == tipo || (tipo == "number" && real == "integer")
}

fn traduzir(tipo: &str) -> &str {
    match tipo {
        "string" => "texto",
        "integer" => "inteiro",
        "number" => "número",
        "boolean" => "booleano (true ou false)",
        "array" => "lista",
        "object" => "objeto",
        "null" => "null",
        outro => outro,
    }
}

fn validar_em(valor: &Value, schema: &Value, caminho: &str) -> Result<(), String> {
    let Some(schema) = schema.as_object() else {
        return Ok(());
    };
    if let Some(tipos) = schema.get("type") {
        let lista: Vec<&str> = match tipos {
            Value::String(t) => vec![t.as_str()],
            Value::Array(ts) => ts.iter().filter_map(Value::as_str).collect(),
            _ => Vec::new(),
        };
        if !lista.is_empty() && !lista.iter().any(|t| aceita(t, valor)) {
            let esperado: Vec<&str> = lista.iter().map(|t| traduzir(t)).collect();
            return Err(format!(
                "{}: esperado {}, recebido {}",
                nome(caminho),
                esperado.join(" ou "),
                traduzir(tipo_de(valor))
            ));
        }
    }
    if let Some(Value::Array(opcoes)) = schema.get("enum")
        && !opcoes.contains(valor)
    {
        let opcoes: Vec<String> = opcoes.iter().map(Value::to_string).collect();
        return Err(format!(
            "{}: valor {valor} não aceito; use um destes: {}",
            nome(caminho),
            opcoes.join(", ")
        ));
    }
    match valor {
        Value::Object(campos) => validar_objeto(campos, schema, caminho),
        Value::Array(itens) => validar_lista(itens, schema, caminho),
        Value::String(texto) => {
            let n = texto.chars().count() as u64;
            if let Some(min) = schema.get("minLength").and_then(Value::as_u64)
                && n < min
            {
                return Err(if min == 1 {
                    format!("{}: não pode ficar vazio", nome(caminho))
                } else {
                    format!("{}: mínimo de {min} caracteres", nome(caminho))
                });
            }
            if let Some(max) = schema.get("maxLength").and_then(Value::as_u64)
                && n > max
            {
                return Err(format!("{}: máximo de {max} caracteres", nome(caminho)));
            }
            Ok(())
        }
        Value::Number(numero) => {
            let n = numero.as_f64().unwrap_or(0.0);
            if let Some(min) = schema.get("minimum").and_then(Value::as_f64)
                && n < min
            {
                return Err(format!("{}: mínimo {min}", nome(caminho)));
            }
            if let Some(max) = schema.get("maximum").and_then(Value::as_f64)
                && n > max
            {
                return Err(format!("{}: máximo {max}", nome(caminho)));
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn validar_objeto(
    campos: &Map<String, Value>,
    schema: &Map<String, Value>,
    caminho: &str,
) -> Result<(), String> {
    let propriedades = schema.get("properties").and_then(Value::as_object);
    if let Some(Value::Array(obrigatorios)) = schema.get("required") {
        for campo in obrigatorios.iter().filter_map(Value::as_str) {
            if !campos.contains_key(campo) {
                return Err(format!(
                    "falta o campo obrigatório `{}`",
                    juntar(caminho, campo)
                ));
            }
        }
    }
    let fechado = schema.get("additionalProperties") == Some(&Value::Bool(false));
    for (campo, valor) in campos {
        match propriedades.and_then(|p| p.get(campo)) {
            Some(sub) => validar_em(valor, sub, &juntar(caminho, campo))?,
            None if fechado => {
                let aceitos: Vec<&str> = propriedades
                    .map(|p| p.keys().map(String::as_str).collect())
                    .unwrap_or_default();
                return Err(format!(
                    "campo desconhecido `{}`; campos aceitos: {}",
                    juntar(caminho, campo),
                    if aceitos.is_empty() {
                        "nenhum".to_owned()
                    } else {
                        aceitos.join(", ")
                    }
                ));
            }
            None => {}
        }
    }
    Ok(())
}

fn validar_lista(
    itens: &[Value],
    schema: &Map<String, Value>,
    caminho: &str,
) -> Result<(), String> {
    let n = itens.len() as u64;
    if let Some(min) = schema.get("minItems").and_then(Value::as_u64)
        && n < min
    {
        return Err(format!("{}: mínimo de {min} item(ns)", nome(caminho)));
    }
    if let Some(max) = schema.get("maxItems").and_then(Value::as_u64)
        && n > max
    {
        return Err(format!("{}: máximo de {max} item(ns)", nome(caminho)));
    }
    if let Some(sub) = schema.get("items") {
        for (i, item) in itens.iter().enumerate() {
            validar_em(item, sub, &format!("{caminho}[{i}]"))?;
        }
    }
    Ok(())
}

fn juntar(caminho: &str, campo: &str) -> String {
    if caminho.is_empty() {
        campo.to_owned()
    } else {
        format!("{caminho}.{campo}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn schema() -> Value {
        json!({
            "type": "object",
            "properties": {
                "secao": { "type": "string", "minLength": 1 },
                "confianca": { "type": "string", "enum": ["confirmado", "inferido"] },
                "limite": { "type": "integer", "minimum": 1, "maximum": 50 },
                "evidencias": {
                    "type": "array",
                    "maxItems": 2,
                    "items": {
                        "type": "object",
                        "properties": {
                            "arquivo": { "type": "string" },
                            "linha_inicial": { "type": "integer" }
                        },
                        "required": ["arquivo", "linha_inicial"],
                        "additionalProperties": false
                    }
                }
            },
            "required": ["secao"],
            "additionalProperties": false
        })
    }

    #[test]
    fn aceita_argumentos_validos() {
        let ok = json!({
            "secao": "regras",
            "confianca": "inferido",
            "limite": 10,
            "evidencias": [{ "arquivo": "db/a.sql", "linha_inicial": 3 }]
        });
        assert_eq!(validar(&ok, &schema()), Ok(()));
        assert_eq!(validar(&json!({ "secao": "x" }), &schema()), Ok(()));
    }

    #[test]
    fn mensagens_citam_o_campo() {
        let casos = [
            (json!({}), "falta o campo obrigatório `secao`"),
            (json!({ "secao": "" }), "`secao`: não pode ficar vazio"),
            (
                json!({ "secao": 1 }),
                "`secao`: esperado texto, recebido inteiro",
            ),
            (
                json!({ "secao": "x", "outro": 1 }),
                "campo desconhecido `outro`",
            ),
            (
                json!({ "secao": "x", "confianca": "talvez" }),
                "use um destes",
            ),
            (json!({ "secao": "x", "limite": 0 }), "`limite`: mínimo 1"),
            (
                json!({ "secao": "x", "limite": 1.5 }),
                "esperado inteiro, recebido número",
            ),
            (
                json!({ "secao": "x", "evidencias": [{ "arquivo": "a" }] }),
                "falta o campo obrigatório `evidencias[0].linha_inicial`",
            ),
            (
                json!({ "secao": "x", "evidencias": [{ "arquivo": "a", "linha_inicial": "3" }] }),
                "`evidencias[0].linha_inicial`: esperado inteiro, recebido texto",
            ),
            (
                json!({ "secao": "x", "evidencias": [{}, {}, {}] }),
                "`evidencias`: máximo de 2",
            ),
            (json!([]), "os argumentos: esperado objeto, recebido lista"),
        ];
        for (valor, esperado) in casos {
            let erro = validar(&valor, &schema()).unwrap_err();
            assert!(erro.contains(esperado), "{valor}: {erro}");
        }
    }

    #[test]
    fn numero_aceita_inteiro_e_lista_de_tipos() {
        let s = json!({ "type": ["number", "null"] });
        assert!(validar(&json!(1), &s).is_ok());
        assert!(validar(&json!(1.5), &s).is_ok());
        assert!(validar(&Value::Null, &s).is_ok());
        assert!(validar(&json!("1"), &s).is_err());
    }
}
