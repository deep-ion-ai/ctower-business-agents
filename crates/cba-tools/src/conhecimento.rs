//! `conhecimento`: busca na base de conhecimento do projeto (regras, glossário,
//! análises anteriores, memórias das áreas), pelo índice FTS5 (`cba_kb::indice`).
//!
//! Primeiro com todas as palavras do termo; se nada tiver todas, com qualquer uma,
//! os documentos com mais palavras (e mais raras) antes. A resposta traz, por
//! documento, o tipo, o identificador, o título, a área, o status, um trecho com as
//! palavras encontradas e o caminho relativo à base, para o agente ler o documento
//! completo quando precisar.
//!
//! Documentação publicada (#278): a cópia do repositório de docs, em
//! `externos/` na base (#277), fica fora da busca padrão e entra sob demanda,
//! com `externos: "incluir"` (junto da base própria, com peso menor no ranking,
//! [`cba_kb::indice::PESO_DOS_EXTERNOS`]) ou `"somente"`. Cada documento externo
//! vem rotulado com o repositório, o commit e a confiança, e a resposta lembra que
//! ele é pista, não prova. Sem pedir, a resposta diz quantos documentos externos
//! casam com o termo, para o agente decidir se os consulta.

use std::fmt::Write as _;

use cba_kb::TipoDocumento;
use cba_kb::indice::{Consulta, Externos, Modo, OrigemIndexada, Resultado, Tipo};
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

/// Rótulo dos documentos da documentação publicada, nas respostas ao agente.
pub(crate) const ROTULO_EXTERNO: &str = "[doc. publicada]";

/// Valores de `externos`, como aparecem no schema.
const EXTERNOS: [&str; 3] = ["excluir", "incluir", "somente"];

fn externos_do_argumento(nome: Option<&str>) -> Externos {
    match nome {
        Some("incluir") => Externos::Incluir,
        Some("somente") => Externos::Somente,
        _ => Externos::Excluir,
    }
}

/// Origem de um documento externo numa linha: repositório, commit curto e
/// confiança (`GitHub exemplo/faturamento-docs @ c0ffee1, confiança publicada`).
/// O trecho recortado de uma página publicada (#290) leva também o caminho com
/// a âncora (`..., trecho docs/funcionalidades/cobranca/f-cob-001.md#rn-cob-001`).
pub(crate) fn rotulo_da_origem(o: &OrigemIndexada) -> String {
    let commit: String = o.commit.chars().take(7).collect();
    let trecho = if o.ancora.is_some() {
        format!(", trecho {}", o.caminho_com_ancora())
    } else {
        String::new()
    };
    format!(
        "{} @ {commit}, confiança {}{trecho}",
        o.repositorio,
        o.confianca.como_str()
    )
}

/// A origem de um documento externo nos dados da resposta.
pub(crate) fn origem_json(o: &OrigemIndexada) -> Value {
    json!({
        "repositorio": o.repositorio,
        "caminho": o.caminho,
        "commit": o.commit,
        "branch": o.branch,
        "confianca": o.confianca.como_str(),
        "tipo_original": o.tipo_original,
        "id_original": o.id_original,
        "ancora": o.ancora,
        "url": o.url,
    })
}

#[derive(Debug, Deserialize)]
struct Args {
    termo: String,
    #[serde(default)]
    tipos: Vec<String>,
    #[serde(default)]
    externos: Option<String>,
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
         trecho e o caminho de cada documento, relativo à base; resposta paginada. \
         A documentação publicada no repositório de docs (cópia só leitura em \
         `externos/`) fica de fora, a não ser que você peça com `externos`; a \
         resposta diz quantos documentos dela casam com o termo. Documento externo \
         vem rotulado [doc. publicada], com repositório, commit e confiança: é pista \
         para achar o código, não prova; confirme no código antes de afirmar e diga \
         ao analista quando usou essa fonte.",
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
                    "description": "Só estes tipos de documento da base própria. Sem o campo: todos. Não filtra a documentação publicada, que vem inteira quando pedida em `externos`."
                },
                "externos": {
                    "type": "string",
                    "enum": EXTERNOS,
                    "description": "Documentação publicada (repositório de docs, cópia em `externos/`): excluir (padrão, só a base própria), incluir (junto da base, abaixo dela em relevância parecida) ou somente."
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
            let externos = externos_do_argumento(args.externos.as_deref());
            let mut tipos: Vec<Tipo> =
                args.tipos.iter().flat_map(|t| tipos_do_indice(t)).collect();
            if !tipos.is_empty() && externos != Externos::Excluir {
                tipos.push(Tipo::Documento(TipoDocumento::DocExterno));
            }
            let indice = fontes::indice(ambiente)?;
            let consulta = |modo, externos, limite, inicio| {
                Consulta::nova(termo)
                    .modo(modo)
                    .projeto(ambiente.kb.nome())
                    .tipos(tipos.iter().copied())
                    .externos(externos)
                    .pagina(limite, inicio)
            };
            let buscar = |c: Consulta| {
                indice.buscar(&c).map_err(|e| {
                    ErroFerramenta::Interno(format!("busca no índice da base: {e}"))
                })
            };
            let (limite, inicio) = (args.pagina.limite(), args.pagina.inicio());
            let mut modo = Modo::Todas;
            let mut resultado = buscar(consulta(modo, externos, limite, inicio))?;
            if resultado.total == 0 && termo.split_whitespace().nth(1).is_some() {
                modo = Modo::Qualquer;
                resultado = buscar(consulta(modo, externos, limite, inicio))?;
            }
            // Sem pedir a fonte externa, só a contagem dela, para o agente decidir.
            let externos_disponiveis = if externos == Externos::Excluir {
                buscar(consulta(modo, Externos::Somente, 1, 0))?.total
            } else {
                0
            };
            let pagina = Pagina::da_janela(
                resultado.resultados.iter().map(item),
                inicio,
                limite,
                resultado.total,
            );
            Ok(resposta(
                ambiente,
                termo,
                modo,
                externos,
                externos_disponiveis,
                &pagina,
            ))
        },
    )
    .somente_leitura()
}

fn item(r: &Resultado) -> Item {
    if let Some(o) = &r.externo {
        return item_externo(r, o);
    }
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
            "fonte": "base",
        }),
    }
}

/// Documento da documentação publicada: rotulado e com a origem, para o agente
/// não confundir com a base própria.
fn item_externo(r: &Resultado, o: &OrigemIndexada) -> Item {
    let id = o
        .id_original
        .as_deref()
        .filter(|i| *i != r.titulo)
        .map(|i| format!("{i} "))
        .unwrap_or_default();
    let tipo = o.tipo_original.as_deref().unwrap_or("documento");
    let mut linha = format!(
        "- {ROTULO_EXTERNO} {id}({tipo}; {}): {} — `{}`",
        rotulo_da_origem(o),
        r.titulo,
        r.caminho
    );
    if let Some(url) = &o.url {
        let _ = write!(linha, "\n  {url}");
    }
    if !r.trecho.is_empty() {
        let _ = write!(linha, "\n  {}", r.trecho);
    }
    Item {
        linha,
        dados: json!({
            "caminho": r.caminho,
            "tipo": r.tipo,
            "identificador": o.id_original,
            "titulo": r.titulo,
            "area": r.area,
            "status": Value::Null,
            "trecho": r.trecho,
            "fonte": "externa",
            "origem": origem_json(o),
        }),
    }
}

/// Lembrete sobre a documentação publicada, quando ela aparece na resposta.
const PISTA_NAO_PROVA: &str = "Itens [doc. publicada] vêm da documentação publicada no \
     repositório de docs (cópia só leitura em `externos/`): são pista, não prova. Use-os \
     para achar o código, confirme no código antes de afirmar (evidência é sempre o \
     código, nunca o documento) e diga ao analista que usou essa fonte, com o \
     repositório e o commit.";

fn resposta(
    ambiente: &cba_core::ambiente::Ambiente,
    termo: &str,
    modo: Modo,
    externos: Externos,
    externos_disponiveis: usize,
    pagina: &Pagina,
) -> Saida {
    let raiz = ambiente.base.raiz().to_string_lossy().into_owned();
    let mut texto = String::new();
    let (onde, onde_nada, nome) = match externos {
        Externos::Excluir => ("Base de conhecimento", "na base de conhecimento", "excluir"),
        Externos::Incluir => (
            "Base de conhecimento e documentação publicada",
            "na base de conhecimento nem na documentação publicada",
            "incluir",
        ),
        Externos::Somente => (
            "Documentação publicada",
            "na documentação publicada",
            "somente",
        ),
    };
    if pagina.total == 0 {
        let _ = writeln!(
            texto,
            "Nada {onde_nada} para \"{termo}\". Tente sinônimos ou outros \
             conceitos de negócio; se a base não souber, trabalhe a partir do código \
             (`referencias`, `estrutura`, `schema` e as suas ferramentas de busca)."
        );
    } else {
        let _ = writeln!(
            texto,
            "{onde}: {} documento(s) para \"{termo}\"{}.",
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
        if pagina.itens.iter().any(|i| i.dados["fonte"] == "externa") {
            let _ = writeln!(texto, "{PISTA_NAO_PROVA}\n");
        }
        texto.push_str(&pagina.linhas());
        let _ = writeln!(texto, "\n{}", pagina.rodape());
    }
    if externos_disponiveis > 0 {
        let _ = writeln!(
            texto,
            "A documentação publicada (repositório de docs) tem {externos_disponiveis} \
             documento(s) para \"{termo}\", fora desta resposta. Para vê-los, chame de \
             novo com externos: \"incluir\" (junto da base) ou \"somente\"; são pista, \
             não prova."
        );
    }
    let mut dados: Value = json!({
        "termo": termo,
        "modo": if modo == Modo::Qualquer { "qualquer" } else { "todas" },
        "externos": nome,
        "base": raiz,
        "resultados": pagina.dados(),
        "pagina": pagina.json(),
    });
    if externos == Externos::Excluir {
        dados["externos_disponiveis"] = json!(externos_disponiveis);
    }
    Saida {
        texto,
        dados: Some(dados),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cba_kb::frontmatter::Confianca;

    fn origem(ancora: Option<&str>) -> OrigemIndexada {
        OrigemIndexada {
            repositorio: "GitHub exemplo/faturamento-docs".into(),
            caminho: "docs/funcionalidades/cobranca/f-cob-001.md".into(),
            commit: "c0ffee1234567890".into(),
            branch: Some("main".into()),
            confianca: Confianca::Publicada,
            tipo_original: Some("regra".into()),
            id_original: Some("RN-COB-001".into()),
            ancora: ancora.map(str::to_owned),
            url: ancora.map(|a| {
                format!(
                    "https://github.com/exemplo/faturamento-docs/blob/c0ffee1234567890/docs/\
                     funcionalidades/cobranca/f-cob-001.md#{a}"
                )
            }),
        }
    }

    #[test]
    fn rotulo_aponta_para_a_ancora_do_trecho() {
        assert_eq!(
            rotulo_da_origem(&origem(None)),
            "GitHub exemplo/faturamento-docs @ c0ffee1, confiança publicada"
        );
        assert_eq!(
            rotulo_da_origem(&origem(Some("rn-cob-001"))),
            "GitHub exemplo/faturamento-docs @ c0ffee1, confiança publicada, trecho \
             docs/funcionalidades/cobranca/f-cob-001.md#rn-cob-001"
        );
        let r = Resultado {
            caminho: cba_kb::CaminhoRelativo::novo("p/externos/x/regra-RN-COB-001.md").unwrap(),
            projeto: "p".into(),
            tipo: None,
            identificador: None,
            titulo: "Multa".into(),
            area: None,
            status: None,
            trecho: String::new(),
            relevancia: 0.0,
            externo: Some(origem(Some("rn-cob-001"))),
        };
        let item = item_externo(&r, r.externo.as_ref().unwrap());
        assert!(
            item.linha.contains("f-cob-001.md#rn-cob-001): Multa"),
            "{}",
            item.linha
        );
        assert!(
            item.linha.contains(
                "regra-RN-COB-001.md`\n  https://github.com/exemplo/faturamento-docs/blob/\
                 c0ffee1234567890/docs/funcionalidades/cobranca/f-cob-001.md#rn-cob-001"
            ),
            "{}",
            item.linha
        );
        assert_eq!(item.dados["origem"]["ancora"], "rn-cob-001");
    }
}
