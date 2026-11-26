//! `schema` com `origem: "banco"` (#439): tabelas, colunas, tipos e chaves do
//! banco conectado à sessão, lidos do catálogo do servidor, com os indícios de
//! dado pessoal pelos nomes e tipos reais.

use std::fmt::Write as _;

use cba_core::banco::{BancoConectado, ColunaBanco, SchemaBanco, TabelaBanco};
use serde_json::{Value, json};

use crate::dados_pessoais;
use crate::paginacao::{ArgsPagina, Item, Pagina, curto};
use crate::registro::ErroFerramenta;

/// Nomes parecidos sugeridos quando a tabela não é encontrada.
const SUGESTOES: usize = 10;

/// O schema do banco conectado: de todas as tabelas ou de uma.
pub(crate) fn do_banco(
    banco: &dyn BancoConectado,
    tabela: Option<&str>,
    args: ArgsPagina,
) -> Result<(String, Value), ErroFerramenta> {
    let origem = banco.origem();
    let schema = banco.ler_schema().map_err(|e| {
        ErroFerramenta::Recusa(format!(
            "não foi possível ler o schema do banco {origem}: {e}. Use `schema` sem \
             `origem` para o modelo dos scripts do projeto"
        ))
    })?;
    let (corpo, mut dados) = match tabela {
        Some(t) => da_tabela(&schema, t, args)?,
        None => de_todas(&schema, args),
    };
    let texto =
        format!("Origem: banco conectado {origem}, lido do catálogo do servidor.\n\n{corpo}");
    dados["origem"] = json!({ "tipo": "banco", "conexao": origem });
    Ok((texto, dados))
}

fn indicio(tabela: &TabelaBanco, c: &ColunaBanco) -> Option<dados_pessoais::Indicio> {
    dados_pessoais::indicio(
        &tabela.nome,
        &c.nome,
        Some(&c.tipo),
        c.comentario.as_deref(),
    )
}

fn de_todas(schema: &SchemaBanco, args: ArgsPagina) -> (String, Value) {
    let itens: Vec<Item> = schema
        .tabelas
        .iter()
        .map(|t| {
            let pessoais = t.colunas.iter().filter(|c| indicio(t, c).is_some()).count();
            let tipo = if t.tipo == "visao" {
                "visão"
            } else {
                "tabela"
            };
            let mut linha = format!("- {tipo} {}, {} colunas", t.qualificado(), t.colunas.len());
            if pessoais > 0 {
                let _ = write!(linha, " ({pessoais} com indício de dado pessoal)");
            }
            if let Some(c) = &t.comentario {
                let _ = write!(linha, " — {}", curto(c));
            }
            Item {
                linha,
                dados: json!({
                    "nome": t.qualificado(),
                    "tipo": t.tipo,
                    "colunas": t.colunas.len(),
                    "colunas_com_dado_pessoal": pessoais,
                    "chave_primaria": t.chave_primaria,
                    "comentario": t.comentario.as_deref().map(curto),
                }),
            }
        })
        .collect();
    let pagina = Pagina::da_lista(itens, args);
    let mut texto = String::new();
    if pagina.total == 0 {
        let _ = writeln!(
            texto,
            "O usuário da conexão não enxerga nenhuma tabela neste banco. Confira as \
             permissões de leitura com quem administra o banco."
        );
    } else {
        let _ = writeln!(
            texto,
            "{} tabela(s) e visão(ões). Para as colunas e chaves de uma, chame `schema` \
             com `tabela` e `origem: \"banco\"`.\n",
            pagina.total
        );
        texto.push_str(&pagina.linhas());
        let _ = writeln!(texto, "\n{}", pagina.rodape());
    }
    let dados = json!({ "objetos": pagina.dados(), "pagina": pagina.json() });
    (texto, dados)
}

/// As tabelas com esse nome: `esquema.nome` exato ou só o nome, sem
/// diferenciar maiúsculas.
fn achar<'a>(schema: &'a SchemaBanco, procura: &str) -> Vec<&'a TabelaBanco> {
    let p = procura.trim().to_lowercase();
    let qualificadas: Vec<&TabelaBanco> = schema
        .tabelas
        .iter()
        .filter(|t| t.qualificado().to_lowercase() == p)
        .collect();
    if !qualificadas.is_empty() {
        return qualificadas;
    }
    schema
        .tabelas
        .iter()
        .filter(|t| t.nome.to_lowercase() == p)
        .collect()
}

fn da_tabela(
    schema: &SchemaBanco,
    procura: &str,
    args: ArgsPagina,
) -> Result<(String, Value), ErroFerramenta> {
    let tabelas = achar(schema, procura);
    if tabelas.is_empty() {
        return Err(nao_encontrada(schema, procura));
    }
    let varias = tabelas.len() > 1;
    let mut cabecalho = String::new();
    let mut objetos = Vec::new();
    let mut itens = Vec::new();
    let mut pessoais = 0usize;
    for t in &tabelas {
        let q = t.qualificado();
        let tipo = if t.tipo == "visao" {
            "visão"
        } else {
            "tabela"
        };
        let _ = write!(cabecalho, "{tipo} {q}");
        if let Some(c) = &t.comentario {
            let _ = write!(cabecalho, " — {}", curto(c));
        }
        cabecalho.push('\n');
        if !t.chave_primaria.is_empty() {
            let _ = writeln!(
                cabecalho,
                "  Chave primária: {}",
                t.chave_primaria.join(", ")
            );
        }
        for f in &t.chaves_estrangeiras {
            let _ = writeln!(
                cabecalho,
                "  Chave estrangeira {}: ({}) → {} ({})",
                f.nome,
                f.colunas.join(", "),
                f.referencia,
                f.colunas_referencia.join(", ")
            );
        }
        let referenciada_por: Vec<Value> = schema
            .tabelas
            .iter()
            .flat_map(|o| {
                o.chaves_estrangeiras
                    .iter()
                    .filter(|f| f.referencia == q)
                    .map(move |f| json!({ "de": o.qualificado(), "chave": f.nome, "colunas": f.colunas }))
            })
            .collect();
        if !referenciada_por.is_empty() {
            let lista: Vec<&str> = referenciada_por
                .iter()
                .map(|r| r["de"].as_str().unwrap_or_default())
                .collect();
            let _ = writeln!(cabecalho, "  Referenciada por: {}", lista.join(", "));
        }
        objetos.push(json!({
            "nome": q,
            "tipo": t.tipo,
            "comentario": t.comentario.as_deref().map(curto),
            "colunas": t.colunas.len(),
            "chave_primaria": t.chave_primaria,
            "chaves_estrangeiras": t.chaves_estrangeiras,
            "referenciada_por": referenciada_por,
        }));
        for c in &t.colunas {
            let mostrado = if varias {
                format!("{q}.{}", c.nome)
            } else {
                c.nome.clone()
            };
            let mut linha = format!("- {mostrado} {}", c.tipo);
            if !c.nula {
                linha.push_str(" NOT NULL");
            }
            let na_chave = t.chave_primaria.contains(&c.nome);
            if na_chave {
                linha.push_str(" (chave primária)");
            }
            if let Some(cm) = &c.comentario {
                let _ = write!(linha, " — {}", curto(cm));
            }
            let pessoal = indicio(t, c);
            if let Some(i) = &pessoal {
                pessoais += 1;
                let _ = write!(
                    linha,
                    " [indício de dado pessoal{}: {}]",
                    if i.sensivel { " sensível" } else { "" },
                    i.categoria
                );
            }
            itens.push(Item {
                linha,
                dados: json!({
                    "tabela": q,
                    "nome": c.nome,
                    "tipo": c.tipo,
                    "nula": c.nula,
                    "chave_primaria": na_chave,
                    "comentario": c.comentario.as_deref().map(curto),
                    "dado_pessoal": pessoal,
                }),
            });
        }
    }
    if pessoais > 0 {
        let _ = writeln!(
            cabecalho,
            "  {pessoais} coluna(s) com indício de dado pessoal, pelo nome e pelo tipo reais: \
             confirme no código o que a coluna guarda e de quem antes de afirmar."
        );
    }
    let pagina = Pagina::da_lista(itens, args);
    let mut texto = cabecalho;
    if pagina.total == 0 {
        let _ = writeln!(texto, "\nNenhuma coluna visível.");
    } else {
        let _ = writeln!(texto, "\nColunas:");
        texto.push_str(&pagina.linhas());
        let _ = writeln!(texto, "\n{}", pagina.rodape());
    }
    let dados = json!({
        "tabela": procura,
        "objetos": objetos,
        "colunas": pagina.dados(),
        "pagina": pagina.json(),
    });
    Ok((texto, dados))
}

fn nao_encontrada(schema: &SchemaBanco, procura: &str) -> ErroFerramenta {
    let p = procura.to_lowercase();
    let ultimo = p.rsplit('.').next().unwrap_or(&p).to_owned();
    let parecidos: Vec<String> = schema
        .tabelas
        .iter()
        .filter(|t| {
            let n = t.nome.to_lowercase();
            n.contains(&ultimo) || ultimo.contains(&n)
        })
        .map(TabelaBanco::qualificado)
        .take(SUGESTOES)
        .collect();
    let mut m = format!("a tabela `{procura}` não foi encontrada no banco conectado");
    if parecidos.is_empty() {
        m.push_str(". Chame `schema` com `origem: \"banco\"` e sem `tabela` para a lista");
    } else {
        let _ = write!(m, ". Nomes parecidos: {}", parecidos.join(", "));
    }
    ErroFerramenta::Recusa(m)
}
