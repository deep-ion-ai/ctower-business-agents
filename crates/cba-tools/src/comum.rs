//! Peças comuns às ferramentas de registro: identificador da análise, alteração
//! com remontagem do documento e resposta padrão.

use std::cell::RefCell;

use cba_core::ambiente::Ambiente;
use cba_core::analise::{Analise, ErroModelo};
use cba_core::eventos::Mudanca;
use cba_kb::{Id, Serie};
use serde_json::{Map, Value, json};

use crate::documento;
use crate::registro::{ErroFerramenta, Saida};

/// Schema do argumento `analise`, comum a todas as ferramentas de registro.
pub(crate) fn schema_da_analise() -> Value {
    json!({
        "type": "string",
        "minLength": 1,
        "description": "Identificador da análise (A-NNNN), o mesmo do pedido do analista (\"Inicie a análise A-0001\")."
    })
}

/// Identificador de análise informado pelo agente.
pub(crate) fn id_da_analise(texto: &str) -> Result<Id, ErroFerramenta> {
    let id = Id::analisar(texto.trim()).map_err(|_| {
        ErroFerramenta::Argumentos(format!(
            "`analise`: `{texto}` não é o identificador de uma análise; use a forma \
             A-NNNN, como A-0001"
        ))
    })?;
    if id.serie() != &Serie::Analise {
        return Err(ErroFerramenta::Argumentos(format!(
            "`analise`: `{id}` não é uma análise; use a forma A-NNNN, como A-0001"
        )));
    }
    Ok(id)
}

/// Recusa do modelo como erro da ferramenta.
pub(crate) fn recusa(erro: ErroModelo) -> ErroFerramenta {
    ErroFerramenta::Recusa(erro.to_string())
}

/// Registro feito: o resultado da alteração, a análise gravada e, se o documento
/// não pôde ser remontado, o motivo.
pub(crate) struct Registrado<T> {
    pub resultado: T,
    pub analise: Analise,
    pub aviso: Option<String>,
}

/// Altera a análise sob a trava e remonta o documento antes de notificar o app.
/// Falha ao remontar não desfaz o registro (o estado é a fonte de verdade e o
/// documento é remontado no próximo registro); vira um aviso na resposta.
pub(crate) fn alterar<T>(
    ambiente: &Ambiente,
    id: &Id,
    mudanca: Mudanca,
    alterar: impl FnOnce(&mut Analise, u64) -> Result<T, ErroModelo>,
) -> Result<Registrado<T>, ErroFerramenta> {
    let aviso = RefCell::new(None);
    let (resultado, analise) = ambiente.analises.alterar_e(id, mudanca, alterar, |a| {
        if let Err(e) = documento::remontar(&ambiente.base, &ambiente.kb, a) {
            tracing::warn!("documento da análise não remontado");
            *aviso.borrow_mut() = Some(format!(
                "o registro foi gravado, mas o documento da análise não foi atualizado \
                 ({e}); ele será remontado no próximo registro"
            ));
        }
    })?;
    Ok(Registrado {
        resultado,
        analise,
        aviso: aviso.into_inner(),
    })
}

/// Resposta de um registro: os dados próprios da ferramenta, mais a análise, o
/// documento (caminho relativo à base, que o agente pode citar) e o aviso, se houver.
pub(crate) fn resposta<T>(ambiente: &Ambiente, registrado: &Registrado<T>, dados: Value) -> Saida {
    let mut saida = Map::new();
    saida.insert("analise".into(), json!(registrado.analise.id));
    if let Value::Object(campos) = dados {
        saida.extend(campos);
    }
    saida.insert(
        "documento".into(),
        json!(ambiente.kb.na_base(&registrado.analise.documento)),
    );
    if let Some(aviso) = &registrado.aviso {
        saida.insert("aviso".into(), json!(aviso));
    }
    Saida::json(Value::Object(saida))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identificador_da_analise() {
        assert_eq!(id_da_analise(" A-0007 ").unwrap().to_string(), "A-0007");
        for errado in ["7", "S-0001", "a-0001", ""] {
            let e = id_da_analise(errado).unwrap_err();
            assert!(e.to_string().contains("A-NNNN"), "{errado}: {e}");
        }
    }
}
