//! `afirmar`: registra uma afirmação com confiança e evidências (spec, seção 6.5).

use cba_core::analise::{Confianca, ErroModelo, EvidenciaOrigem, NovaAfirmacao};
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
    integracoes, documentos_e_telas, dados_pessoais_e_rnf, riscos, abrangencia. \
    Explicar funcionalidade: objetivo, atores_e_telas, fluxo_principal, \
    fluxos_alternativos, excecoes, fluxo_processo, regras, dados, modelo_dados, \
    integracoes. \
    Análise de reunião: ligacao_com_o_codigo. \
    Manual do usuário: objetivo, quem_usa, pre_requisitos, resultados_esperados, \
    mensagens_e_erros, perguntas_frequentes.";

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
    #[serde(default)]
    dados: Vec<DadoCitado>,
    #[serde(default)]
    origens: Vec<OrigemCitada>,
}

/// Commit citado como origem da regra (#450), pelo hash que
/// `historico_do_trecho` devolveu.
#[derive(Debug, Deserialize)]
struct OrigemCitada {
    commit: String,
}

/// Resultado de uma consulta citado como evidência de dados (#440).
#[derive(Debug, Deserialize)]
struct DadoCitado {
    consulta: u32,
    resumo: String,
}

/// `afirmar`: o conteúdo de uma análise é feito de afirmações, cada uma com seção,
/// texto, confiança e evidências conferidas contra o código.
pub fn afirmar() -> Ferramenta {
    Ferramenta::nova(
        "afirmar",
        "Registrar afirmação",
        "Registra uma afirmação na análise, com a seção do documento, o texto, a \
         confiança e as evidências do código. Cada evidência é conferida na hora: o \
         arquivo precisa estar nas pastas do projeto e fora das exclusões, o intervalo \
         precisa existir e o trecho citado precisa aparecer nessas linhas. Evidência que \
         não confere é recusada com o motivo e a correção; corrija e chame de novo. \
         Evidência é sempre código: a documentação publicada (resultado [doc. publicada] \
         de `conhecimento`, a cópia em `externos/` ou a pasta de docs) é pista para \
         achar o código, não prova, e é recusada como evidência. \
         Confiança: `confirmado` (evidência direta; exige ao menos uma evidência), \
         `inferido` (dedução a partir das evidências listadas) ou `a_confirmar` (sem \
         evidência suficiente; cria também uma pergunta para o solicitante). Registre \
         uma regra ou fato por afirmação. Na seção `abrangencia` da análise de impacto, \
         o texto começa pela estimativa (\"Pequena:\", \"Média:\" ou \"Grande:\"). \
         O resultado de uma consulta ao banco (`consultar_dados`) entra em `dados`, como \
         evidência de dados, separada da evidência de código. O commit que criou ou \
         mudou a regra (de `historico_do_trecho`) entra em `origens`, como evidência \
         complementar: a evidência da regra continua sendo o código.",
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
                    "description": "A afirmação, em linguagem de negócio, com números quando houver (\"A multa por atraso é de 2% sobre o valor original\"). Aceita Markdown simples: quando enumerar itens (perfis, grupos, passos), use uma lista com um item por linha em vez de uma enumeração longa entre parênteses; use **negrito** só para o que o leitor precisa notar. Não cole código, JSON nem trechos minificados no texto: o código fica nas evidências."
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
                    "description": "Trechos do código que sustentam a afirmação. Documentação (a publicada ou a cópia em `externos/`) não serve: cite o código que ela descreve."
                },
                "pergunta": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 4000,
                    "description": "Pergunta para o solicitante ligada à afirmação. Com a_confirmar, se omitida, é criada a partir do texto."
                },
                "dados": {
                    "type": "array",
                    "maxItems": 10,
                    "description": "Resultados de consultas executadas por `consultar_dados` que sustentam a afirmação: viram evidências de dados no documento, com a conexão, o ambiente, a data e hora e o SQL. Valem para a data da consulta.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "consulta": { "type": "integer", "minimum": 1, "description": "Número da consulta devolvido por consultar_dados." },
                            "resumo": { "type": "string", "minLength": 1, "maxLength": 500, "description": "O que o resultado mostra, sem dado pessoal (\"312 faturas vencidas com multa zerada\")." }
                        },
                        "required": ["consulta", "resumo"],
                        "additionalProperties": false
                    }
                },
                "origens": {
                    "type": "array",
                    "maxItems": 5,
                    "description": "Commits que criaram ou mudaram a regra, pelo hash devolvido por `historico_do_trecho` (#450). O app confere o commit no repositório e guarda a data, a mensagem e as chaves de issue; no documento entram o hash curto, a data e a mensagem. Complementam as evidências de código, não as substituem.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "commit": { "type": "string", "minLength": 7, "maxLength": 40, "description": "Hash do commit (de 7 a 40 dígitos hexadecimais)." }
                        },
                        "required": ["commit"],
                        "additionalProperties": false
                    }
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
            if analise.fluxo == cba_core::analise::Fluxo::Historias {
                return Err(ErroFerramenta::Recusa(format!(
                    "a análise {id} é de histórias e não recebe afirmações: registre as \
                     histórias com `registrar_historia` e os critérios com \
                     `registrar_criterio`, citando as afirmações da análise de origem"
                )));
            }
            if analise.fluxo == cba_core::analise::Fluxo::Cenarios {
                return Err(ErroFerramenta::Recusa(format!(
                    "a análise {id} é de cenários de teste e não recebe afirmações: registre \
                     as tabelas de decisão com `registrar_tabela_decisao` e os cenários com \
                     `registrar_cenario`, citando as regras e as afirmações de origem"
                )));
            }
            if analise.fluxo == cba_core::analise::Fluxo::Prototipo {
                return Err(ErroFerramenta::Recusa(format!(
                    "a análise {id} é de protótipo de telas e não recebe afirmações: registre \
                     as telas com `registrar_tela` e os elementos com \
                     `registrar_elemento_tela`, citando as regras, as afirmações e os \
                     critérios de origem"
                )));
            }
            if !secoes.contains(&secao) {
                let dica = match secao {
                    "perguntas" => " Para perguntas ao solicitante, use `perguntar`.",
                    "decisoes" => {
                        " Para uma decisão do negócio, use `perguntar` com tipo `decisao` e \
                         as opções."
                    }
                    "sugestoes" => " Para sugestões de alteração, use `registrar_sugestao`.",
                    cba_docs::VERIFICACOES_EM_DADOS => {
                        " Para uma consulta de verificação em dados, use \
                         `registrar_verificacao`."
                    }
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
            if args.confianca == Confianca::Confirmado
                && args.evidencias.is_empty()
                && args.dados.is_empty()
            {
                return Err(comum::recusa(ErroModelo::ConfirmadoSemEvidencia));
            }
            let evidencias = evidencias::validar_todas(&ambiente.escopo, &args.evidencias)
                .map_err(ErroFerramenta::Recusa)?;
            let mut origens: Vec<EvidenciaOrigem> = Vec::new();
            for o in &args.origens {
                let origem = crate::historico::resolver_commit(ambiente, &o.commit)
                    .map_err(|e| ErroFerramenta::Recusa(format!("origens: {e}")))?;
                if !origens.iter().any(|x| x.commit == origem.commit) {
                    origens.push(origem);
                }
            }
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
                let dados = args
                    .dados
                    .iter()
                    .map(|d| a.evidencia_de_dados(d.consulta, &d.resumo))
                    .collect::<Result<Vec<_>, _>>()?;
                let numero = a.registrar_afirmacao_com_evidencias(nova, dados, origens, agora)?;
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
                    "dados": afirmacao.dados.iter().map(|d| d.consulta).collect::<Vec<_>>(),
                    "origens": afirmacao
                        .origens
                        .iter()
                        .map(|o| json!({ "commit": o.commit, "data": o.data, "assunto": o.assunto, "chaves": o.chaves }))
                        .collect::<Vec<_>>(),
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
        let (impacto, resto) = resto.split_once("Explicar funcionalidade:").unwrap();
        let (explicar, resto) = resto.split_once("Análise de reunião:").unwrap();
        let (reuniao, manual) = resto.split_once("Manual do usuário:").unwrap();
        for (fluxo, texto) in [
            (Fluxo::Perguntar, perguntar),
            (Fluxo::Impacto, impacto),
            (Fluxo::Explicar, explicar),
            (Fluxo::Reuniao, reuniao),
            (Fluxo::Manual, manual),
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
