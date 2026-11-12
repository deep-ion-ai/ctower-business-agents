//! `propor_termo`: propõe uma entrada para o glossário negócio → código (spec,
//! seções 6.1 e 7.4).
//!
//! O termo fica no estado da análise como proposta; na aprovação da análise, o
//! analista decide se ele vira `glossario/<termo>.md` (#11).

use cba_core::analise::NovoTermo;
use cba_core::eventos::Mudanca;
use cba_kb::layout;
use serde::Deserialize;
use serde_json::json;

use crate::comum;
use crate::evidencias::{self, EvidenciaCitada};
use crate::registro::{ErroFerramenta, Ferramenta, ler_argumentos};

#[derive(Debug, Deserialize)]
struct Args {
    analise: String,
    termo: String,
    definicao: String,
    #[serde(default)]
    sinonimos: Vec<String>,
    #[serde(default)]
    onde_vive_no_codigo: Vec<String>,
    #[serde(default)]
    area: Option<String>,
    #[serde(default)]
    evidencias: Vec<EvidenciaCitada>,
}

/// `propor_termo`: o glossário é o que mais acelera as análises seguintes, porque
/// localizar no código os conceitos da demanda é o maior custo de uma análise.
pub fn propor_termo() -> Ferramenta {
    let lista_de_textos = |descricao: &str| {
        json!({
            "type": "array",
            "maxItems": 30,
            "items": { "type": "string", "minLength": 1, "maxLength": 300 },
            "description": descricao
        })
    };
    Ferramenta::nova(
        "propor_termo",
        "Propor termo do glossário",
        "Propõe uma entrada para o glossário do projeto: um termo de negócio, a \
         definição, os sinônimos e onde ele vive no código (entidades, tabelas, \
         colunas, telas, serviços, documentos). Use quando descobrir como um conceito \
         do negócio aparece no código. As evidências, se houver, são conferidas como \
         em afirmar. O termo fica proposto na análise e entra no glossário quando o \
         analista aprovar.",
        json!({
            "type": "object",
            "properties": {
                "analise": comum::schema_da_analise(),
                "termo": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 120,
                    "description": "O termo como o negócio fala (\"Multa por atraso\")."
                },
                "definicao": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 4000,
                    "description": "Definição de negócio, sem jargão técnico."
                },
                "sinonimos": lista_de_textos("Outros nomes do termo no negócio e no código."),
                "onde_vive_no_codigo": lista_de_textos(
                    "Onde o termo aparece no código: tabelas e colunas, rotinas, classes, telas, documentos (\"tabela FATURA, coluna VL_MULTA\")."
                ),
                "area": {
                    "type": "string",
                    "maxLength": 120,
                    "description": "Área de negócio do termo (ex.: cobranca), se conhecida."
                },
                "evidencias": {
                    "type": "array",
                    "maxItems": 10,
                    "items": evidencias::schema_da_evidencia(),
                    "description": "Trechos do código em que o termo aparece."
                }
            },
            "required": ["analise", "termo", "definicao"],
            "additionalProperties": false
        }),
        |ambiente, args| {
            let args: Args = ler_argumentos(args)?;
            let id = comum::id_da_analise(&args.analise)?;
            let glossario = layout::termo(&args.termo).map_err(|_| {
                ErroFerramenta::Argumentos(
                    "`termo`: o termo precisa ter ao menos uma letra ou dígito".into(),
                )
            })?;
            // Confere a análise antes de ler os arquivos das evidências.
            ambiente.analises.carregar(&id)?;
            let evidencias = evidencias::validar_todas(&ambiente.escopo, &args.evidencias)
                .map_err(ErroFerramenta::Recusa)?;
            let novo = NovoTermo {
                termo: args.termo,
                definicao: args.definicao,
                sinonimos: args.sinonimos,
                onde_vive: args.onde_vive_no_codigo,
                area: args.area,
                evidencias,
                sessao: None,
            };
            let registrado = comum::alterar(ambiente, &id, Mudanca::Termo, |a, agora| {
                a.registrar_termo(novo, agora)
            })?;
            let numero = registrado.resultado;
            Ok(comum::resposta(
                ambiente,
                &registrado,
                json!({
                    "termo": numero,
                    "glossario": ambiente.kb.na_base(&glossario),
                    "ja_no_glossario": ambiente.kb.existe(&glossario),
                }),
            ))
        },
    )
}
