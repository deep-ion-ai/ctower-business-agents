//! `perguntar`: registra uma pergunta em aberto para o solicitante: uma dúvida
//! ou uma decisão para o negócio, com as opções (#452).

use cba_core::analise::{MAX_OPCOES, MIN_OPCOES, NovaDecisao, OpcaoDecisao, TipoPergunta};
use cba_core::eventos::Mudanca;
use serde::Deserialize;
use serde_json::json;

use crate::comum;
use crate::registro::{ErroFerramenta, Ferramenta, ler_argumentos};

#[derive(Debug, Deserialize)]
struct Opcao {
    texto: String,
    efeito: String,
}

#[derive(Debug, Deserialize)]
struct Args {
    analise: String,
    texto: String,
    #[serde(default)]
    afirmacao: Option<u32>,
    #[serde(default)]
    tipo: TipoPergunta,
    #[serde(default)]
    opcoes: Vec<Opcao>,
    #[serde(default)]
    recomendacao: Option<String>,
}

/// `perguntar`: perguntas que o código não responde (parâmetros em tabela, regras
/// fora do código analisável) e premissas da demanda a confirmar (`duvida`), e as
/// decisões que o negócio precisa tomar, com as opções e o efeito de cada uma
/// (`decisao`, #452).
pub fn perguntar() -> Ferramenta {
    Ferramenta::nova(
        "perguntar",
        "Registrar pergunta",
        "Registra uma pergunta em aberto para o solicitante. Com tipo `duvida` (o padrão): \
         o que o código não responde (valores em tabela, regras fora do código \
         analisável, projetos vizinhos) ou uma premissa da demanda que precisa ser \
         confirmada; entra em \"Perguntas para o solicitante\". Com tipo `decisao`: uma \
         escolha que cabe ao negócio (qual regra vale, se uma regra criada por demanda \
         anterior continua valendo, se a mudança vale para outro público), com 2 a 4 \
         `opcoes`, cada uma com o efeito de escolhê-la, e a `recomendacao`, se houver; \
         entra em \"Decisões para o negócio\", no topo do documento. Quando a correção \
         exige escolher uma regra, ou uma mudança depende de uma hipótese não \
         verificada, registre a decisão (ou a dúvida, com o que confirmaria a hipótese) \
         em vez de propor a regra em `funcionalidades`. Uma afirmação com confiança \
         a_confirmar já cria a sua pergunta; não repita.",
        json!({
            "type": "object",
            "properties": {
                "analise": comum::schema_da_analise(),
                "texto": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 4000,
                    "description": "A pergunta ou a decisão, clara para quem não lê código."
                },
                "afirmacao": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Número da afirmação a que a pergunta ou a decisão se refere, se houver (devolvido por afirmar)."
                },
                "tipo": {
                    "type": "string",
                    "enum": ["duvida", "decisao"],
                    "default": "duvida",
                    "description": "`duvida`: um fato que o código não responde. `decisao`: uma escolha do negócio entre opções."
                },
                "opcoes": {
                    "type": "array",
                    "minItems": MIN_OPCOES,
                    "maxItems": MAX_OPCOES,
                    "description": "Só na decisão: as opções, cada uma com o efeito de escolhê-la.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "texto": {
                                "type": "string",
                                "minLength": 1,
                                "maxLength": 1000,
                                "description": "A opção (\"Manter a regra da DEM-12\")."
                            },
                            "efeito": {
                                "type": "string",
                                "minLength": 1,
                                "maxLength": 2000,
                                "description": "O que acontece se o negócio escolher a opção (\"revoga a regra da DEM-12\", \"mantém a exceção para pessoa jurídica\")."
                            }
                        },
                        "required": ["texto", "efeito"],
                        "additionalProperties": false
                    }
                },
                "recomendacao": {
                    "type": "string",
                    "maxLength": 2000,
                    "description": "Só na decisão: a opção que você recomenda e por quê, se recomenda alguma."
                }
            },
            "required": ["analise", "texto"],
            "additionalProperties": false
        }),
        |ambiente, args| {
            let args: Args = ler_argumentos(args)?;
            let id = comum::id_da_analise(&args.analise)?;
            let decisao = match args.tipo {
                TipoPergunta::Duvida => {
                    if !args.opcoes.is_empty() || args.recomendacao.is_some() {
                        return Err(ErroFerramenta::Recusa(
                            "uma dúvida não tem opções nem recomendação; se o negócio precisa \
                             escolher entre opções, use tipo `decisao`"
                                .into(),
                        ));
                    }
                    None
                }
                TipoPergunta::Decisao => Some(NovaDecisao {
                    opcoes: args
                        .opcoes
                        .into_iter()
                        .map(|o| OpcaoDecisao {
                            texto: o.texto,
                            efeito: o.efeito,
                        })
                        .collect(),
                    recomendacao: args.recomendacao,
                }),
            };
            let e_decisao = decisao.is_some();
            let registrado =
                comum::alterar(ambiente, &id, Mudanca::Pergunta, |a, agora| match decisao {
                    Some(d) => a.registrar_decisao(&args.texto, d, args.afirmacao, None, agora),
                    None => a.registrar_pergunta(&args.texto, args.afirmacao, None, agora),
                })?;
            let numero = registrado.resultado;
            Ok(comum::resposta(
                ambiente,
                &registrado,
                json!({
                    "pergunta": numero,
                    "afirmacao": args.afirmacao,
                    "tipo": if e_decisao { "decisao" } else { "duvida" },
                }),
            ))
        },
    )
}
