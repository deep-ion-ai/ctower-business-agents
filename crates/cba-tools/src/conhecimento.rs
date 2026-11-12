//! `conhecimento`: busca na base de conhecimento do projeto (regras, glossário,
//! análises anteriores, memórias das áreas), pelo índice FTS5 (`cba_kb::indice`).
//!
//! Primeiro com todas as palavras do termo; se nada tiver todas, com qualquer uma,
//! os documentos com mais palavras (e mais raras) antes. A resposta traz, por
//! documento, o tipo, o identificador, o título, a área, o status, um trecho com as
//! palavras encontradas e o caminho relativo à base, para o agente ler o documento
//! completo quando precisar.

use std::fmt::Write as _;

use cba_kb::TipoDocumento;
use cba_kb::indice::{Consulta, Modo, Resultado, Tipo};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::fontes;
use crate::paginacao::{ArgsPagina, Item, Pagina, com_paginacao};
use crate::registro::{ErroFerramenta, Ferramenta, Saida, ler_argumentos};

/// Tipos que o agente pode pedir, como aparecem no schema.
const TIPOS: [&str; 7] = [
    "regra",
    "termo",
    "analise",
    "memoria",
    "caso-de-uso",
    "sugestao",
    "regras-do-projeto",
];

fn tipos_do_indice(nome: &str) -> Vec<Tipo> {
    use TipoDocumento as T;
    match nome {
        "regra" => vec![Tipo::Documento(T::Regra)],
        "termo" => vec![Tipo::Documento(T::Termo)],
        "analise" => Tipo::ANALISES.to_vec(),
        "memoria" => vec![Tipo::Documento(T::MemoriaArea)],
        "caso-de-uso" => vec![Tipo::Documento(T::CasoDeUso)],
        "sugestao" => vec![Tipo::Documento(T::Sugestao)],
        "regras-do-projeto" => vec![Tipo::RegrasDoProjeto],
        _ => Vec::new(),
    }
}

#[derive(Debug, Deserialize)]
struct Args {
    termo: String,
    #[serde(default)]
    tipos: Vec<String>,
    #[serde(flatten)]
    pagina: ArgsPagina,
}

/// `conhecimento(termo)`: busca na base de conhecimento do projeto.
pub fn conhecimento() -> Ferramenta {
    Ferramenta::nova(
        "conhecimento",
        "Consultar a base de conhecimento",
        "Busca na base de conhecimento do projeto o que já foi apurado: regras de \
         negócio (com o status: confirmada, a-revalidar, obsoleta), termos do glossário \
         (onde cada conceito de negócio vive no código), análises anteriores e memórias \
         das áreas. Use ANTES de explorar o código, com os conceitos de negócio da \
         pergunta (\"carência\", \"multa atraso\"). Sem diferença de acentos nem de \
         maiúsculas; uma palavra casa também como começo de outra. Devolve título, \
         trecho e o caminho de cada documento, relativo à base; resposta paginada.",
        com_paginacao(json!({
            "type": "object",
            "properties": {
                "termo": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 200,
                    "description": "Palavras a buscar (conceito de negócio, identificador de regra, nome de tabela ou rotina)."
                },
                "tipos": {
                    "type": "array",
                    "items": { "type": "string", "enum": TIPOS },
                    "description": "Só estes tipos de documento. Sem o campo: todos."
                }
            },
            "required": ["termo"],
            "additionalProperties": false
        })),
        |ambiente, args| {
            let args: Args = ler_argumentos(args)?;
            let termo = args.termo.trim();
            if termo.is_empty() {
                return Err(ErroFerramenta::Argumentos(
                    "`termo`: informe ao menos uma palavra".into(),
                ));
            }
            let tipos: Vec<Tipo> = args.tipos.iter().flat_map(|t| tipos_do_indice(t)).collect();
            let indice = fontes::indice(ambiente)?;
            let consulta = |modo| {
                Consulta::nova(termo)
                    .modo(modo)
                    .projeto(ambiente.kb.nome())
                    .tipos(tipos.iter().copied())
                    .pagina(args.pagina.limite(), args.pagina.inicio())
            };
            let buscar = |modo| {
                indice.buscar(&consulta(modo)).map_err(|e| {
                    ErroFerramenta::Interno(format!("busca no índice da base: {e}"))
                })
            };
            let mut modo = Modo::Todas;
            let mut resultado = buscar(modo)?;
            if resultado.total == 0 && termo.split_whitespace().nth(1).is_some() {
                modo = Modo::Qualquer;
                resultado = buscar(modo)?;
            }
            let pagina = Pagina::da_janela(
                resultado.resultados.iter().map(item),
                args.pagina.inicio(),
                args.pagina.limite(),
                resultado.total,
            );
            Ok(resposta(ambiente, termo, modo, &pagina))
        },
    )
    .somente_leitura()
}

fn item(r: &Resultado) -> Item {
    let tipo = r.tipo.map(Tipo::como_str).unwrap_or("documento");
    let mut detalhes = vec![tipo.to_owned()];
    detalhes.extend(r.status.clone());
    detalhes.extend(r.area.clone());
    let id = r
        .identificador
        .as_deref()
        .filter(|i| *i != r.titulo)
        .map(|i| format!("{i} "))
        .unwrap_or_default();
    let mut linha = format!(
        "- {id}({}): {} — `{}`",
        detalhes.join(", "),
        r.titulo,
        r.caminho
    );
    if !r.trecho.is_empty() {
        let _ = write!(linha, "\n  {}", r.trecho);
    }
    Item {
        linha,
        dados: json!({
            "caminho": r.caminho,
            "tipo": r.tipo,
            "identificador": r.identificador,
            "titulo": r.titulo,
            "area": r.area,
            "status": r.status,
            "trecho": r.trecho,
        }),
    }
}

fn resposta(
    ambiente: &cba_core::ambiente::Ambiente,
    termo: &str,
    modo: Modo,
    pagina: &Pagina,
) -> Saida {
    let raiz = ambiente.base.raiz().to_string_lossy().into_owned();
    let mut texto = String::new();
    if pagina.total == 0 {
        let _ = writeln!(
            texto,
            "Nada na base de conhecimento para \"{termo}\". Tente sinônimos ou outros \
             conceitos de negócio; se a base não souber, trabalhe a partir do código \
             (`referencias`, `estrutura`, `schema` e as suas ferramentas de busca)."
        );
    } else {
        let _ = writeln!(
            texto,
            "Base de conhecimento: {} documento(s) para \"{termo}\"{}.",
            pagina.total,
            if modo == Modo::Qualquer {
                " (nenhum tinha todas as palavras; estes têm ao menos uma)"
            } else {
                ""
            }
        );
        let _ = writeln!(
            texto,
            "Caminhos relativos à base (`{raiz}`); leia o documento quando precisar do \
             conteúdo completo. Regra confirmada não precisa ser redescoberta, mas \
             confira a evidência.\n"
        );
        texto.push_str(&pagina.linhas());
        let _ = writeln!(texto, "\n{}", pagina.rodape());
    }
    let dados: Value = json!({
        "termo": termo,
        "modo": if modo == Modo::Qualquer { "qualquer" } else { "todas" },
        "base": raiz,
        "resultados": pagina.dados(),
        "pagina": pagina.json(),
    });
    Saida {
        texto,
        dados: Some(dados),
    }
}
