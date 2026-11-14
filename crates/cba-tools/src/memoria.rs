//! `memorizar`: o agente mantém a memória de trabalho da análise (#143), que
//! `retomar_analise` devolve para uma conversa nova continuar de onde a anterior
//! parou, sem depender da conversa gravada pelo Claude Code.

use cba_core::analise::{ErroModelo, EstadoAnalise};
use cba_core::analises;
use cba_core::memoria::{AVISO_DA_MEMORIA, LIMITE_DA_MEMORIA, Memoria, ModoMemoria, SecaoMemoria};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::comum;
use crate::registro::{Ferramenta, Saida, ler_argumentos};

#[derive(Debug, Deserialize)]
struct Args {
    analise: String,
    secao: SecaoMemoria,
    texto: String,
    #[serde(default)]
    modo: ModoMemoria,
}

fn schema() -> Value {
    let secoes: Vec<&str> = SecaoMemoria::TODAS.iter().map(|s| s.como_str()).collect();
    let descricao = SecaoMemoria::TODAS
        .iter()
        .map(|s| format!("`{}`: {}", s.como_str(), s.descricao()))
        .collect::<Vec<_>>()
        .join("; ");
    json!({
        "type": "object",
        "properties": {
            "analise": comum::schema_da_analise(),
            "secao": {
                "type": "string",
                "enum": secoes,
                "description": format!("Seção da memória. {descricao}.")
            },
            "texto": {
                "type": "string",
                "maxLength": 8000,
                "description": "O que guardar, curto e autossuficiente para quem não viu a conversa (caminhos, nomes de rotinas, o motivo). Com modo substituir, o texto novo da seção inteira; vazio limpa a seção."
            },
            "modo": {
                "type": "string",
                "enum": ["acrescentar", "substituir"],
                "default": "acrescentar",
                "description": "acrescentar (padrão) põe um item no fim da seção; substituir troca a seção inteira: use para `onde_parou` e para resumir uma seção que cresceu."
            }
        },
        "required": ["analise", "secao", "texto"],
        "additionalProperties": false
    })
}

/// `memorizar`: grava numa seção da memória da análise.
pub fn memorizar() -> Ferramenta {
    Ferramenta::nova(
        "memorizar",
        "Atualizar a memória da análise",
        "Grava na memória da análise o contexto que não vai para o documento e que uma \
         conversa nova precisaria para continuar: onde parou, o que já foi investigado \
         ou descartado e por quê, hipóteses em aberto, onde está o código (inclusive \
         repositórios fora do projeto), o que foi combinado com o analista e o que \
         ficou pendente. A memória fica na base, ao lado do documento, e volta em \
         `retomar_analise`. Atualize nos marcos: depois de uma descoberta relevante, \
         quando o analista der uma orientação e antes de `concluir_analise`. Não \
         repita aqui as afirmações, perguntas e termos já registrados.",
        schema(),
        |ambiente, args| {
            let args: Args = ler_argumentos(args)?;
            let id = comum::id_da_analise(&args.analise)?;
            let (_, memoria, analise) =
                ambiente.analises.alterar_memoria(&id, |analise, memoria| {
                    if analise.estado == EstadoAnalise::Aprovada {
                        return Err(ErroModelo::AnaliseAprovada(analise.id.clone()).into());
                    }
                    memoria
                        .gravar(args.secao, args.modo, &args.texto, LIMITE_DA_MEMORIA)
                        .map_err(analises::Erro::from)
                })?;
            Ok(resposta(ambiente, &analise.id, args.secao, &memoria))
        },
    )
}

fn resposta(
    ambiente: &cba_core::ambiente::Ambiente,
    id: &cba_kb::Id,
    secao: SecaoMemoria,
    memoria: &Memoria,
) -> Saida {
    let tamanho = memoria.tamanho();
    let mut dados = json!({
        "analise": id,
        "secao": secao,
        "memoria": ambiente.kb.na_base(&analises::Analises::arquivo_de_memoria(id)),
        "tamanho": tamanho,
        "limite": LIMITE_DA_MEMORIA,
    });
    #[allow(clippy::cast_precision_loss)]
    if tamanho as f64 >= LIMITE_DA_MEMORIA as f64 * AVISO_DA_MEMORIA {
        dados["aviso"] = json!(format!(
            "a memória está com {tamanho} de {LIMITE_DA_MEMORIA} caracteres; resuma as \
             seções mais longas com `modo: \"substituir\"`, mantendo só o que ainda \
             importa para continuar"
        ));
    }
    Saida::json(dados)
}
