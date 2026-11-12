//! `perguntar`: registra uma pergunta em aberto para o solicitante.

use cba_core::eventos::Mudanca;
use serde::Deserialize;
use serde_json::json;

use crate::comum;
use crate::registro::{Ferramenta, ler_argumentos};

#[derive(Debug, Deserialize)]
struct Args {
    analise: String,
    texto: String,
    #[serde(default)]
    afirmacao: Option<u32>,
}

/// `perguntar`: perguntas que o código não responde (parâmetros em tabela, regras
/// fora do código analisável, decisões do negócio) e premissas da demanda a
/// confirmar.
pub fn perguntar() -> Ferramenta {
    Ferramenta::nova(
        "perguntar",
        "Registrar pergunta",
        "Registra uma pergunta em aberto para o solicitante: o que o código não \
         responde (valores em tabela, regras fora do código analisável, sistemas \
         vizinhos) ou uma premissa da demanda que precisa ser confirmada. A pergunta \
         entra na seção de perguntas do documento da análise. Uma afirmação com \
         confiança a_confirmar já cria a sua pergunta; não repita.",
        json!({
            "type": "object",
            "properties": {
                "analise": comum::schema_da_analise(),
                "texto": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 4000,
                    "description": "A pergunta, clara para quem não lê código."
                },
                "afirmacao": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Número da afirmação a que a pergunta se refere, se houver (devolvido por afirmar)."
                }
            },
            "required": ["analise", "texto"],
            "additionalProperties": false
        }),
        |ambiente, args| {
            let args: Args = ler_argumentos(args)?;
            let id = comum::id_da_analise(&args.analise)?;
            let registrado = comum::alterar(ambiente, &id, Mudanca::Pergunta, |a, agora| {
                a.registrar_pergunta(&args.texto, args.afirmacao, None, agora)
            })?;
            let numero = registrado.resultado;
            Ok(comum::resposta(
                ambiente,
                &registrado,
                json!({ "pergunta": numero, "afirmacao": args.afirmacao }),
            ))
        },
    )
}
