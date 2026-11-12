//! `schema(tabela?)`: tabelas, visões, sequências, índices e sinônimos extraídos dos
//! scripts de banco, com as colunas e os relacionamentos de uma tabela, a partir do
//! mapa estrutural (`cba_index::Mapa::schema`, `filhos` e as referências).
//!
//! O módulo se chama `banco` porque `schema` já é a validação dos argumentos.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use cba_index::{Achado, Mapa, TipoReferencia};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::fontes;
use crate::paginacao::{ArgsPagina, Item, Pagina, com_paginacao, curto, nome};
use crate::registro::{ErroFerramenta, Ferramenta, Saida, ler_argumentos};

/// Nomes parecidos sugeridos quando a tabela não é encontrada.
const SUGESTOES: usize = 10;

#[derive(Debug, Deserialize)]
struct Args {
    #[serde(default)]
    tabela: Option<String>,
    #[serde(flatten)]
    pagina: ArgsPagina,
}

/// `schema(tabela?)`: o modelo de dados do projeto ou de uma tabela.
pub fn schema() -> Ferramenta {
    Ferramenta::nova(
        "schema",
        "Schema do banco",
        "Consulta o modelo de dados extraído dos scripts de banco do projeto. Sem \
         `tabela`: lista tabelas, visões, sequências, índices e sinônimos, com arquivo, \
         linhas e quantidade de colunas. Com `tabela`: as colunas (tipo e comentário), \
         as chaves estrangeiras de e para a tabela e quantas vezes ela é lida e gravada \
         no código. Resposta paginada.",
        com_paginacao(json!({
            "type": "object",
            "properties": {
                "tabela": {
                    "type": "string",
                    "minLength": 1,
                    "description": "Nome da tabela ou visão, com ou sem esquema (\"FATURA\", \"APP.FATURA\"). Sem o campo: todas."
                }
            },
            "additionalProperties": false
        })),
        |ambiente, args| {
            let args: Args = ler_argumentos(args)?;
            let carregado = fontes::mapa(ambiente)?;
            let mapa = &carregado.mapa;
            let (mut texto, mut dados) = match args.tabela.as_deref().map(str::trim) {
                Some(tabela) if !tabela.is_empty() => da_tabela(mapa, tabela, args.pagina)?,
                _ => de_todas(mapa, args.pagina),
            };
            let _ = writeln!(texto, "\n{}", carregado.nota());
            dados["mapa"] = carregado.json();
            Ok(Saida {
                texto,
                dados: Some(dados),
            })
        },
    )
    .somente_leitura()
}

fn local(a: &Achado<'_>) -> String {
    let l = a.simbolo.linhas;
    if l.inicio == l.fim {
        format!("{}:{}", a.arquivo, l.inicio)
    } else {
        format!("{}:{}–{}", a.arquivo, l.inicio, l.fim)
    }
}

fn de_todas(mapa: &Mapa, args: ArgsPagina) -> (String, Value) {
    let objetos = mapa.schema();
    let mut por_tipo: BTreeMap<String, usize> = BTreeMap::new();
    let itens: Vec<Item> = objetos
        .iter()
        .map(|a| {
            *por_tipo.entry(nome(&a.simbolo.tipo)).or_default() += 1;
            let colunas = mapa.filhos(a.id).len();
            let mut linha = format!(
                "- {} {} ({})",
                nome(&a.simbolo.tipo),
                a.simbolo.qualificado,
                local(a)
            );
            if colunas > 0 {
                let _ = write!(linha, ", {colunas} colunas");
            }
            if let Some(c) = &a.simbolo.comentario {
                let _ = write!(linha, " — {}", curto(c));
            }
            Item {
                linha,
                dados: json!({
                    "nome": a.simbolo.qualificado,
                    "tipo": a.simbolo.tipo,
                    "arquivo": a.arquivo,
                    "linha_inicial": a.simbolo.linhas.inicio,
                    "linha_final": a.simbolo.linhas.fim,
                    "colunas": colunas,
                    "comentario": a.simbolo.comentario.as_deref().map(curto),
                }),
            }
        })
        .collect();
    let pagina = Pagina::da_lista(itens, args);
    let mut texto = String::new();
    if pagina.total == 0 {
        let _ = writeln!(
            texto,
            "Nenhuma tabela extraída dos scripts do projeto. O modelo de dados pode estar \
             fora do código (banco sem scripts versionados): procure o uso das tabelas \
             no código com `referencias` ou registre a dúvida com `perguntar`."
        );
    } else {
        let _ = writeln!(
            texto,
            "Modelo de dados: {}. Para as colunas e relacionamentos de uma tabela, chame \
             `schema` com `tabela`.\n",
            por_tipo
                .iter()
                .map(|(t, n)| format!("{n} {t}"))
                .collect::<Vec<_>>()
                .join(", ")
        );
        texto.push_str(&pagina.linhas());
        let _ = writeln!(texto, "\n{}", pagina.rodape());
    }
    let dados = json!({
        "objetos": pagina.dados(),
        "por_tipo": por_tipo,
        "pagina": pagina.json(),
    });
    (texto, dados)
}

fn da_tabela(
    mapa: &Mapa,
    tabela: &str,
    args: ArgsPagina,
) -> Result<(String, Value), ErroFerramenta> {
    let achados = mapa.buscar(tabela);
    let objetos: Vec<Achado<'_>> = achados
        .iter()
        .copied()
        .filter(|a| a.simbolo.tipo.eh_schema())
        .collect();
    if objetos.is_empty() {
        return Err(nao_encontrada(mapa, tabela, &achados));
    }

    let mut cabecalho = String::new();
    let mut dados_objetos = Vec::new();
    let mut itens = Vec::new();
    let varios = objetos.len() > 1;
    for o in &objetos {
        let s = o.simbolo;
        let _ = write!(
            cabecalho,
            "{} {} ({})",
            nome(&s.tipo),
            s.qualificado,
            local(o)
        );
        if let Some(c) = &s.comentario {
            let _ = write!(cabecalho, " — {}", curto(c));
        }
        cabecalho.push('\n');

        let chaves: Vec<Value> = mapa
            .referencias_de(o.id)
            .iter()
            .filter(|u| u.referencia.tipo == TipoReferencia::ChaveEstrangeira)
            .map(|u| json!({ "para": u.referencia.alvo, "arquivo": u.arquivo, "linha": u.referencia.linha }))
            .collect();
        let mut referenciada_por = Vec::new();
        let mut usos: BTreeMap<String, usize> = BTreeMap::new();
        for u in mapa.referencias_a(o.id) {
            if u.referencia.tipo == TipoReferencia::ChaveEstrangeira {
                let de = u
                    .origem
                    .map(|a| a.simbolo.qualificado.clone())
                    .unwrap_or_else(|| u.arquivo.to_owned());
                referenciada_por
                    .push(json!({ "de": de, "arquivo": u.arquivo, "linha": u.referencia.linha }));
            } else {
                *usos.entry(nome(&u.referencia.tipo)).or_default() += 1;
            }
        }
        if !chaves.is_empty() {
            let lista: Vec<String> = chaves
                .iter()
                .map(|c| {
                    format!(
                        "{} (linha {})",
                        c["para"].as_str().unwrap_or_default(),
                        c["linha"]
                    )
                })
                .collect();
            let _ = writeln!(
                cabecalho,
                "  Chaves estrangeiras para: {}",
                lista.join(", ")
            );
        }
        if !referenciada_por.is_empty() {
            let lista: Vec<&str> = referenciada_por
                .iter()
                .map(|c| c["de"].as_str().unwrap_or_default())
                .collect();
            let _ = writeln!(cabecalho, "  Referenciada por: {}", lista.join(", "));
        }
        if !usos.is_empty() {
            let _ = writeln!(
                cabecalho,
                "  Usos no código: {}; veja onde com `referencias` (\"{}\").",
                usos.iter()
                    .map(|(t, n)| format!("{n} {t}"))
                    .collect::<Vec<_>>()
                    .join(", "),
                s.qualificado
            );
        }

        let colunas = mapa.filhos(o.id);
        dados_objetos.push(json!({
            "nome": s.qualificado,
            "tipo": s.tipo,
            "arquivo": o.arquivo,
            "linha_inicial": s.linhas.inicio,
            "linha_final": s.linhas.fim,
            "detalhe": s.detalhe.as_deref().map(curto),
            "comentario": s.comentario.as_deref().map(curto),
            "colunas": colunas.len(),
            "chaves_estrangeiras": chaves,
            "referenciada_por": referenciada_por,
            "usos": usos,
        }));
        for c in colunas {
            let cs = c.simbolo;
            let mostrado = if varios { &cs.qualificado } else { &cs.nome };
            let mut linha = format!("- {mostrado}");
            if let Some(d) = &cs.detalhe {
                let _ = write!(linha, " {}", curto(d));
            }
            let _ = write!(linha, " (linha {})", cs.linhas.inicio);
            if let Some(cm) = &cs.comentario {
                let _ = write!(linha, " — {}", curto(cm));
            }
            itens.push(Item {
                linha,
                dados: json!({
                    "tabela": s.qualificado,
                    "nome": cs.nome,
                    "tipo": cs.tipo,
                    "detalhe": cs.detalhe.as_deref().map(curto),
                    "linha": cs.linhas.inicio,
                    "comentario": cs.comentario.as_deref().map(curto),
                }),
            });
        }
    }

    let pagina = Pagina::da_lista(itens, args);
    let mut texto = cabecalho;
    if pagina.total == 0 {
        let _ = writeln!(texto, "\nNenhuma coluna extraída.");
    } else {
        let _ = writeln!(texto, "\nColunas:");
        texto.push_str(&pagina.linhas());
        let _ = writeln!(texto, "\n{}", pagina.rodape());
    }
    let dados = json!({
        "tabela": tabela,
        "objetos": dados_objetos,
        "colunas": pagina.dados(),
        "pagina": pagina.json(),
    });
    Ok((texto, dados))
}

fn nao_encontrada(mapa: &Mapa, tabela: &str, achados: &[Achado<'_>]) -> ErroFerramenta {
    if let Some(outro) = achados.first() {
        return ErroFerramenta::Recusa(format!(
            "`{tabela}` não é tabela nem visão: é {} {} em `{}`. Use `estrutura` ou \
             `referencias` para ele",
            nome(&outro.simbolo.tipo),
            outro.simbolo.qualificado,
            local(outro)
        ));
    }
    let procura = tabela.to_uppercase();
    let ultimo = procura.rsplit('.').next().unwrap_or(&procura).to_owned();
    let parecidos: Vec<String> = mapa
        .schema()
        .iter()
        .map(|a| a.simbolo.qualificado.clone())
        .filter(|q| {
            let q = q.to_uppercase();
            q.contains(&ultimo) || ultimo.contains(q.rsplit('.').next().unwrap_or(&q))
        })
        .take(SUGESTOES)
        .collect();
    let mut mensagem = format!("a tabela `{tabela}` não foi encontrada nos scripts de banco");
    if parecidos.is_empty() {
        mensagem.push_str(
            ". Chame `schema` sem `tabela` para a lista; se a tabela não tem script no \
             projeto, procure o uso dela com `referencias`",
        );
    } else {
        let _ = write!(mensagem, ". Nomes parecidos: {}", parecidos.join(", "));
    }
    ErroFerramenta::Recusa(mensagem)
}
