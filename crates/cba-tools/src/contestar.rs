//! `contestar_afirmacao` e `contestar_achado`: a verificação independente de um
//! workflow contesta o que não se sustenta (spec
//! `docs/specs/2026-11-25-workflows-nas-analises.md`, seção 3.1; #371).
//!
//! As duas são as únicas mudanças no MCP para os workflows. Elas não apagam nada:
//! marcam o item como a revalidar, com a origem "verificação independente", o motivo
//! e, se houver, a evidência do contrário ([`cba_core::conducao::Contestacao`]). O
//! item aparece como pendência na resposta do `concluir_analise` e na revisão do app,
//! e quem decide é o analista. Contestar de novo o mesmo item substitui a contestação
//! anterior: a retomada do workflow não duplica nada.

use cba_core::conducao::{Contestacao, OrigemContestacao};
use cba_core::eventos::Mudanca;
use cba_core::tempo;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::comum;
use crate::evidencias::{self, EvidenciaCitada};
use crate::registro::{ErroFerramenta, Ferramenta, ler_argumentos};

/// Evidências no máximo por contestação.
const LIMITE_DE_EVIDENCIAS: u64 = 5;

#[derive(Debug, Deserialize)]
struct Args {
    analise: String,
    numero: u32,
    motivo: String,
    #[serde(default)]
    evidencias: Vec<EvidenciaCitada>,
}

fn schema(item: &str) -> Value {
    json!({
        "type": "object",
        "properties": {
            "analise": comum::schema_da_analise(),
            "numero": {
                "type": "integer",
                "minimum": 1,
                "description": format!("Número {item} contestado, como `retomar_analise` mostra.")
            },
            "motivo": {
                "type": "string",
                "minLength": 1,
                "maxLength": 4000,
                "description": "Por que o item não se sustenta, em linguagem de negócio e com valores (\"o trecho citado é o cálculo de juros, não o de multa; a multa é de 2% em db/03_pkg_cobranca.sql\")."
            },
            "evidencias": {
                "type": "array",
                "maxItems": LIMITE_DE_EVIDENCIAS,
                "items": evidencias::schema_da_evidencia(),
                "description": "Opcional: trechos do código que mostram o contrário, conferidos como no `afirmar`."
            }
        },
        "required": ["analise", "numero", "motivo"],
        "additionalProperties": false
    })
}

/// Lê os argumentos e confere as evidências: a contestação pronta para gravar.
fn contestacao(
    ambiente: &cba_core::ambiente::Ambiente,
    args: &Args,
) -> Result<Contestacao, ErroFerramenta> {
    let evidencias = evidencias::validar_todas(&ambiente.escopo, &args.evidencias)
        .map_err(ErroFerramenta::Recusa)?;
    Ok(Contestacao {
        origem: OrigemContestacao::VerificacaoIndependente,
        motivo: args.motivo.clone(),
        evidencias,
        sessao: None,
        registrada_ms: tempo::agora_ms(),
    })
}

/// `contestar_afirmacao`.
pub fn contestar_afirmacao() -> Ferramenta {
    Ferramenta::nova(
        "contestar_afirmacao",
        "Contestar afirmação",
        "Verificação independente (fase Verificar do workflow): contesta uma afirmação \
         que não se sustenta depois de reler a evidência dela. Não apaga nada: marca a \
         afirmação como a revalidar, com a origem \"verificação independente\", o motivo \
         e, se houver, o trecho do código que mostra o contrário. O analista decide na \
         revisão. Use só quando a evidência não sustenta o texto (trecho de outra regra, \
         valor diferente, condição que falta, camada que contradiz); afirmação que se \
         sustenta não precisa de chamada. Contestar de novo a mesma afirmação substitui a \
         contestação anterior.",
        schema("da afirmação"),
        |ambiente, args| {
            let args: Args = ler_argumentos(args)?;
            let id = comum::id_da_analise(&args.analise)?;
            let contestacao = contestacao(ambiente, &args)?;
            let registrado = comum::alterar(ambiente, &id, Mudanca::Afirmacao, |a, _| {
                a.contestar_afirmacao(args.numero, contestacao)
            })?;
            Ok(comum::resposta(
                ambiente,
                &registrado,
                json!({
                    "afirmacao": args.numero,
                    "a_revalidar": registrado.resultado,
                    "nota": if registrado.resultado {
                        "afirmação marcada como a revalidar; o analista decide na revisão"
                    } else {
                        "o analista já decidiu esta afirmação: a contestação fica ao lado, \
                         sem mudar a decisão dele"
                    },
                }),
            ))
        },
    )
}

/// `contestar_achado`.
pub fn contestar_achado() -> Ferramenta {
    Ferramenta::nova(
        "contestar_achado",
        "Contestar achado",
        "Verificação independente (fase Verificar do workflow de revisão): contesta um \
         achado do parecer que não se sustenta depois de reler o texto revisado, a base e \
         o código. Não apaga nada: o achado fica no parecer, a revalidar, com a origem \
         \"verificação independente\", o motivo e, se houver, o trecho do código que \
         mostra o contrário. O analista decide se descarta. Contestar de novo o mesmo \
         achado substitui a contestação anterior.",
        schema("do achado"),
        |ambiente, args| {
            let args: Args = ler_argumentos(args)?;
            let id = comum::id_da_analise(&args.analise)?;
            let contestacao = contestacao(ambiente, &args)?;
            let registrado = comum::alterar(ambiente, &id, Mudanca::Parecer, |a, _| {
                a.contestar_achado(args.numero, contestacao)
            })?;
            Ok(comum::resposta(
                ambiente,
                &registrado,
                json!({
                    "achado": args.numero,
                    "nota": "achado a revalidar; o analista decide na revisão",
                }),
            ))
        },
    )
}
