//! JSON que preserva a ordem das chaves.
//!
//! O `serde_json::Value` do workspace ordena as chaves (o recurso `preserve_order` não
//! está ligado, e ligá-lo mudaria a ordem para todas as crates). Para editar a
//! configuração do analista sem embaralhar o arquivo, os objetos aqui são listas de
//! pares na ordem em que foram lidos.

use std::fmt;

use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde::ser::{Serialize, SerializeMap, SerializeSeq, Serializer};

/// Valor JSON com objetos em ordem de leitura.
#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Numero(serde_json::Number),
    Texto(String),
    Lista(Vec<Json>),
    Objeto(Objeto),
}

/// Objeto JSON: pares na ordem do arquivo.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Objeto(pub Vec<(String, Json)>);

impl Objeto {
    pub fn get(&self, chave: &str) -> Option<&Json> {
        self.0
            .iter()
            .rev()
            .find(|(k, _)| k == chave)
            .map(|(_, v)| v)
    }

    pub fn get_mut(&mut self, chave: &str) -> Option<&mut Json> {
        self.0
            .iter_mut()
            .rev()
            .find(|(k, _)| k == chave)
            .map(|(_, v)| v)
    }

    /// Troca o valor de `chave` (mantendo a posição) ou acrescenta no fim.
    pub fn inserir(&mut self, chave: &str, valor: Json) {
        match self.get_mut(chave) {
            Some(atual) => *atual = valor,
            None => self.0.push((chave.to_string(), valor)),
        }
    }

    /// Remove `chave` e devolve o valor removido.
    pub fn remover(&mut self, chave: &str) -> Option<Json> {
        let mut removido = None;
        self.0.retain(|(k, v)| {
            if k == chave {
                removido = Some(v.clone());
                false
            } else {
                true
            }
        });
        removido
    }

    pub fn chaves(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(|(k, _)| k.as_str())
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Objeto em `chave`, criado vazio se não existir. `None` se existir com outro tipo.
    pub fn objeto_mut(&mut self, chave: &str) -> Option<&mut Objeto> {
        if self.get(chave).is_none() {
            self.0
                .push((chave.to_string(), Json::Objeto(Objeto::default())));
        }
        match self.get_mut(chave) {
            Some(Json::Objeto(o)) => Some(o),
            _ => None,
        }
    }
}

impl Json {
    pub fn texto(s: impl Into<String>) -> Self {
        Json::Texto(s.into())
    }

    pub fn como_texto(&self) -> Option<&str> {
        match self {
            Json::Texto(s) => Some(s),
            _ => None,
        }
    }

    pub fn como_objeto(&self) -> Option<&Objeto> {
        match self {
            Json::Objeto(o) => Some(o),
            _ => None,
        }
    }

    pub fn como_lista(&self) -> Option<&Vec<Json>> {
        match self {
            Json::Lista(l) => Some(l),
            _ => None,
        }
    }

    /// Converte para `serde_json::Value` (para comparar conteúdo sem olhar a ordem).
    pub fn para_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }

    /// Texto com indentação de dois espaços, como o Claude Code grava.
    pub fn para_texto(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }
}

impl Serialize for Json {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Json::Null => s.serialize_unit(),
            Json::Bool(b) => s.serialize_bool(*b),
            Json::Numero(n) => n.serialize(s),
            Json::Texto(t) => s.serialize_str(t),
            Json::Lista(l) => {
                let mut seq = s.serialize_seq(Some(l.len()))?;
                for v in l {
                    seq.serialize_element(v)?;
                }
                seq.end()
            }
            Json::Objeto(o) => {
                let mut mapa = s.serialize_map(Some(o.0.len()))?;
                for (k, v) in &o.0 {
                    mapa.serialize_entry(k, v)?;
                }
                mapa.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for Json {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(Visitante)
    }
}

struct Visitante;

impl<'de> Visitor<'de> for Visitante {
    type Value = Json;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("um valor JSON")
    }

    fn visit_unit<E: de::Error>(self) -> Result<Json, E> {
        Ok(Json::Null)
    }

    fn visit_none<E: de::Error>(self) -> Result<Json, E> {
        Ok(Json::Null)
    }

    fn visit_bool<E: de::Error>(self, v: bool) -> Result<Json, E> {
        Ok(Json::Bool(v))
    }

    fn visit_i64<E: de::Error>(self, v: i64) -> Result<Json, E> {
        Ok(Json::Numero(v.into()))
    }

    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Json, E> {
        Ok(Json::Numero(v.into()))
    }

    fn visit_f64<E: de::Error>(self, v: f64) -> Result<Json, E> {
        serde_json::Number::from_f64(v)
            .map(Json::Numero)
            .ok_or_else(|| E::custom("número inválido"))
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<Json, E> {
        Ok(Json::Texto(v.to_string()))
    }

    fn visit_string<E: de::Error>(self, v: String) -> Result<Json, E> {
        Ok(Json::Texto(v))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Json, A::Error> {
        let mut lista = Vec::new();
        while let Some(v) = seq.next_element()? {
            lista.push(v);
        }
        Ok(Json::Lista(lista))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut mapa: A) -> Result<Json, A::Error> {
        let mut pares = Vec::new();
        while let Some((k, v)) = mapa.next_entry::<String, Json>()? {
            pares.push((k, v));
        }
        Ok(Json::Objeto(Objeto(pares)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ida_e_volta_preserva_a_ordem() {
        let texto = r#"{
  "zeta": 1,
  "alfa": {
    "y": [
      true,
      null,
      2.5,
      -3
    ],
    "b": "x"
  },
  "meio": {}
}"#;
        let json: Json = serde_json::from_str(texto).unwrap();
        assert_eq!(json.para_texto(), texto);
    }

    #[test]
    fn inserir_mantem_posicao_e_remover_tira() {
        let mut o = Objeto::default();
        o.inserir("a", Json::Null);
        o.inserir("b", Json::Bool(true));
        o.inserir("a", Json::texto("novo"));
        assert_eq!(o.chaves().collect::<Vec<_>>(), ["a", "b"]);
        assert_eq!(o.get("a"), Some(&Json::texto("novo")));
        assert_eq!(o.remover("a"), Some(Json::texto("novo")));
        assert_eq!(o.chaves().collect::<Vec<_>>(), ["b"]);
        assert!(o.objeto_mut("b").is_none());
        assert!(o.objeto_mut("c").is_some());
    }
}
