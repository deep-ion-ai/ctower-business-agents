//! Ferramentas sobre as análises do projeto.

use cba_core::analise::{EstadoAnalise, EstadoPergunta};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::registro::{Ferramenta, Saida, ler_argumentos};

#[derive(Debug, Deserialize)]
struct ArgsListar {
    #[serde(default)]
    estado: Option<EstadoAnalise>,
}

/// `listar_analises`: as análises do projeto, para o agente achar o identificador
/// quando o analista cita uma análise pelo assunto.
pub fn listar_analises() -> Ferramenta {
    Ferramenta::nova(
        "listar_analises",
        "Listar análises",
        "Lista as análises do projeto desta sessão: identificador (A-NNNN), título, \
         fluxo, estado, documento e quantos registros cada uma tem. Use para achar o \
         identificador de uma análise que o analista citou pelo assunto.",
        json!({
            "type": "object",
            "properties": {
                "estado": {
                    "type": "string",
                    "enum": ["criada", "em-andamento", "concluida", "aprovada"],
                    "description": "Só as análises neste estado."
                }
            },
            "additionalProperties": false
        }),
        |ambiente, args| {
            let args: ArgsListar = ler_argumentos(args)?;
            let analises: Vec<Value> = ambiente
                .analises
                .listar()?
                .into_iter()
                .filter(|a| args.estado.is_none_or(|e| a.estado == e))
                .map(|a| {
                    json!({
                        "id": a.id,
                        "titulo": a.titulo,
                        "fluxo": a.fluxo,
                        "estado": a.estado,
                        "documento": ambiente.kb.na_base(&a.documento),
                        "afirmacoes": a.afirmacoes.len(),
                        "perguntas_abertas": a
                            .perguntas
                            .iter()
                            .filter(|p| p.estado == EstadoPergunta::Aberta)
                            .count(),
                        "sugestoes": a.sugestoes.len(),
                    })
                })
                .collect();
            Ok(Saida::json(json!({
                "projeto": { "id": ambiente.projeto.id, "nome": ambiente.projeto.nome },
                "analises": analises,
            })))
        },
    )
    .somente_leitura()
}
