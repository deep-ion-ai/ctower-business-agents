//! `afirmar`: registra uma afirmação com confiança e evidências (spec, seção 6.5).

use cba_core::analise::{Confianca, ErroModelo, NovaAfirmacao};
use cba_core::eventos::Mudanca;
use serde::Deserialize;
use serde_json::json;

use crate::comum;
use crate::documento::{SECAO_DA_ABRANGENCIA, abrangencia_do_texto, secoes_de_afirmacao};
use crate::evidencias::{self, EvidenciaCitada};
use crate::registro::{ErroFerramenta, Ferramenta, ler_argumentos};

/// Evidências no máximo por afirmação.
pub const LIMITE_DE_EVIDENCIAS: u64 = 20;

/// Seções aceitas por fluxo, como aparecem na descrição de `secao`. Um teste
/// confere que a lista bate com [`secoes_de_afirmacao`].
const DESCRICAO_DA_SECAO: &str = "Seção do documento em que a afirmação entra. \
    Pergunta ao sistema: resposta, regras_encontradas, exemplo. \
    Análise de impacto: demanda_como_entendida, como_e_hoje, funcionalidades, dados, \
    integracoes, documentos_e_telas, riscos, abrangencia. \
    Explicar funcionalidade: objetivo, atores_e_telas, fluxo_principal, \
    fluxos_alternativos, excecoes, regras, dados, integracoes.";

#[derive(Debug, Deserialize)]
struct Args {
    analise: String,
    secao: String,
    texto: String,
    confianca: Confianca,
    #[serde(default)]
    evidencias: Vec<EvidenciaCitada>,
    #[serde(default)]
    pergunta: Option<String>,
}

/// `afirmar`: o conteúdo de uma análise é feito de afirmações, cada uma com seção,
/// texto, confiança e evidências conferidas contra o código.
pub fn afirmar() -> Ferramenta {
    Ferramenta::nova(
        "afirmar",
        "Registrar afirmação",
        "Registra uma afirmação na análise, com a seção do documento, o texto, a \
         confiança e as evidências do código. Cada evidência é conferida na hora: o \
         arquivo precisa estar nas pastas do sistema e fora das exclusões, o intervalo \
         precisa existir e o trecho citado precisa aparecer nessas linhas. Evidência que \
         não confere é recusada com o motivo e a correção; corrija e chame de novo. \
         Confiança: `confirmado` (evidência direta; exige ao menos uma evidência), \
         `inferido` (dedução a partir das evidências listadas) ou `a_confirmar` (sem \
         evidência suficiente; cria também uma pergunta para o solicitante). Registre \
         uma regra ou fato por afirmação. Na seção `abrangencia` da análise de impacto, \
         o texto começa pela estimativa (\"Pequena:\", \"Média:\" ou \"Grande:\").",
        json!({
            "type": "object",
            "properties": {
                "analise": comum::schema_da_analise(),
                "secao": {
                    "type": "string",
                    "minLength": 1,
                    "description": DESCRICAO_DA_SECAO
                },
                "texto": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 10000,
                    "description": "A afirmação, em linguagem de negócio, com números quando houver (\"A multa por atraso é de 2% sobre o valor original\")."
                },
                "confianca": {
                    "type": "string",
                    "enum": ["confirmado", "inferido", "a_confirmar"],
                    "description": "confirmado: evidência direta. inferido: dedução a partir das evidências. a_confirmar: sem evidência suficiente (vira pergunta)."
                },
                "evidencias": {
                    "type": "array",
                    "maxItems": LIMITE_DE_EVIDENCIAS,
                    "items": evidencias::schema_da_evidencia(),
                    "description": "Trechos do código que sustentam a afirmação."
                },
                "pergunta": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 4000,
                    "description": "Pergunta para o solicitante ligada à afirmação. Com a_confirmar, se omitida, é criada a partir do texto."
                }
            },
            "required": ["analise", "secao", "texto", "confianca"],
            "additionalProperties": false
        }),
        |ambiente, args| {
            let args: Args = ler_argumentos(args)?;
            let id = comum::id_da_analise(&args.analise)?;
            let analise = ambiente.analises.carregar(&id)?;
            let secao = args.secao.trim();
            let secoes = secoes_de_afirmacao(analise.fluxo);
            if !secoes.contains(&secao) {
                let dica = match secao {
                    "perguntas" => " Para perguntas ao solicitante, use `perguntar`.",
                    "sugestoes" => " Para sugestões de alteração, use `registrar_sugestao`.",
                    "evidencias" => " As evidências vão no campo `evidencias`.",
                    _ => "",
                };
                return Err(ErroFerramenta::Recusa(format!(
                    "a seção `{secao}` não recebe afirmações no documento desta análise \
                     ({}). Use uma destas: {}.{dica}",
                    analise.fluxo.tipo_documento(),
                    secoes.join(", ")
                )));
            }
            if secao == SECAO_DA_ABRANGENCIA && abrangencia_do_texto(&args.texto).is_none() {
                return Err(ErroFerramenta::Recusa(
                    "a afirmação da seção `abrangencia` começa pela estimativa: \"Pequena:\", \
                     \"Média:\" ou \"Grande:\", seguida da justificativa (\"Média: o parâmetro \
                     muda no banco, mas o lote também usa o valor fixo\"). Corrija o texto e \
                     chame de novo"
                        .to_owned(),
                ));
            }
            if args.confianca == Confianca::Confirmado && args.evidencias.is_empty() {
                return Err(comum::recusa(ErroModelo::ConfirmadoSemEvidencia));
            }
            let evidencias = evidencias::validar_todas(&ambiente.escopo, &args.evidencias)
                .map_err(ErroFerramenta::Recusa)?;
            let pergunta = match (args.confianca, args.pergunta) {
                (_, Some(p)) => Some(p),
                (Confianca::AConfirmar, None) => Some(format!(
                    "Confirmar com o solicitante: {}",
                    args.texto.trim()
                )),
                _ => None,
            };
            let nova = NovaAfirmacao {
                secao: secao.to_owned(),
                texto: args.texto,
                confianca: args.confianca,
                evidencias,
                sessao: None,
            };
            let registrado = comum::alterar(ambiente, &id, Mudanca::Afirmacao, |a, agora| {
                let numero = a.registrar_afirmacao(nova, agora)?;
                let pergunta = match &pergunta {
                    Some(p) => Some(a.registrar_pergunta(p, Some(numero), None, agora)?),
                    None => None,
                };
                Ok((numero, pergunta))
            })?;
            let (numero, pergunta) = registrado.resultado;
            let afirmacao = registrado
                .analise
                .afirmacoes
                .iter()
                .find(|a| a.numero == numero)
                .expect("afirmação recém-registrada");
            let evidencias: Vec<_> = afirmacao
                .evidencias
                .iter()
                .map(|e| {
                    json!({
                        "arquivo": e.arquivo,
                        "linhas": e.linhas,
                        "hash": e.hash,
                        "commit": e.commit,
                    })
                })
                .collect();
            Ok(comum::resposta(
                ambiente,
                &registrado,
                json!({
                    "afirmacao": numero,
                    "secao": afirmacao.secao,
                    "confianca": afirmacao.confianca,
                    "evidencias": evidencias,
                    "pergunta": pergunta,
                }),
            ))
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use cba_core::analise::Fluxo;

    #[test]
    fn descricao_da_secao_lista_as_secoes_de_cada_fluxo() {
        let (_, resto) = DESCRICAO_DA_SECAO
            .split_once("Pergunta ao sistema:")
            .unwrap();
        let (perguntar, resto) = resto.split_once("Análise de impacto:").unwrap();
        let (impacto, explicar) = resto.split_once("Explicar funcionalidade:").unwrap();
        for (fluxo, texto) in [
            (Fluxo::Perguntar, perguntar),
            (Fluxo::Impacto, impacto),
            (Fluxo::Explicar, explicar),
        ] {
            let lista: Vec<&str> = texto
                .trim()
                .trim_end_matches('.')
                .split(',')
                .map(str::trim)
                .collect();
            assert_eq!(lista, secoes_de_afirmacao(fluxo), "{fluxo:?}");
        }
    }
}
