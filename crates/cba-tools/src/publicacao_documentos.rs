//! Prévia por documento da publicação (#289).
//!
//! As páginas legíveis juntam vários documentos numa página (as regras de uma
//! funcionalidade, o glossário), cada um num bloco marcado
//! ([`cba_docs::marcador`]). Para o analista, "a página `regras/cobranca.md`
//! mudou" diz pouco; a prévia compara os blocos (tipo + id, conteúdo e
//! frontmatter) do que seria publicado com o que o app publicou na branch base
//! e diz "2 regras novas, 1 alterada; 1 termo novo".
//!
//! Na publicação no formato antigo (#276, manifesto versão 1), cada `.md`
//! publicado com frontmatter é um documento: ele casa com o bloco da página nova
//! pelo caminho na base (que o marcador guarda) e conta como alterado só se o
//! frontmatter mudou; com o mesmo frontmatter, só muda de formato
//! ([`ResumoDosDocumentos::reformatados`]).

use std::collections::BTreeMap;

use cba_docs::marcador;
use cba_kb::Documento;
use cba_kb::frontmatter::FrontmatterLivre;
use serde::Serialize;

use super::TipoMudanca;

/// Uma página lida (gerada ou publicada).
#[derive(Debug, Clone, Copy)]
pub struct PaginaLida<'a> {
    /// Relativo à raiz do repositório.
    pub caminho: &'a str,
    /// Relativo à pasta da documentação.
    pub relativo: &'a str,
    pub texto: &'a str,
}

/// Um documento que muda na publicação.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MudancaDeDocumento {
    /// O `tipo` do frontmatter (`regra`, `termo`, `pergunta`...).
    pub tipo: String,
    /// O id do marcador (`RN-COB-001`, `multa`, `A-0001`).
    pub id: String,
    pub titulo: String,
    pub mudanca: TipoMudanca,
    /// Página onde o documento fica (ou ficava, se removido), relativa à raiz do
    /// repositório.
    pub pagina: String,
}

/// O que muda por documento.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ResumoDosDocumentos {
    /// Novos, alterados e removidos, nessa ordem e, dentro de cada um, por
    /// categoria (regras, termos, análises...) e id.
    pub documentos: Vec<MudancaDeDocumento>,
    /// Documentos da publicação no formato antigo que só mudam de formato.
    pub reformatados: usize,
    /// "2 regras novas, 1 alterada; 1 termo novo"; vazio sem mudança.
    pub texto: String,
}

/// Um documento publicado (ou a publicar).
struct Doc {
    titulo: String,
    pagina: String,
    conteudo: String,
    frontmatter: serde_json::Value,
    antigo: bool,
}

type Chave = (String, String);

/// Os documentos das páginas legíveis, pelos marcadores. Página com marcador
/// ilegível (editada à mão no destino) fica de fora.
fn dos_blocos(paginas: &[PaginaLida<'_>]) -> (BTreeMap<Chave, Doc>, BTreeMap<String, Chave>) {
    let mut docs = BTreeMap::new();
    let mut por_caminho_na_base = BTreeMap::new();
    for p in paginas {
        let Ok(blocos) = marcador::blocos(p.texto) else {
            continue;
        };
        for b in blocos {
            let m = b.marcador;
            let chave = (m.tipo.clone(), m.id.clone());
            por_caminho_na_base.insert(m.metadados.caminho.clone(), chave.clone());
            docs.insert(
                chave,
                Doc {
                    titulo: titulo(&m.metadados.frontmatter, &m.id),
                    pagina: p.caminho.to_owned(),
                    conteudo: b.conteudo,
                    frontmatter: m.metadados.frontmatter,
                    antigo: false,
                },
            );
        }
    }
    (docs, por_caminho_na_base)
}

/// Os documentos da publicação no formato antigo: cada `.md` com frontmatter.
/// A chave é a do bloco novo com o mesmo caminho na base; sem ele (documento que
/// saiu), o `tipo` e o `id` do frontmatter (ou o nome do arquivo).
fn dos_arquivos(
    paginas: &[PaginaLida<'_>],
    por_caminho_na_base: &BTreeMap<String, Chave>,
) -> BTreeMap<Chave, Doc> {
    let mut docs = BTreeMap::new();
    for p in paginas {
        let Ok(doc) = Documento::<FrontmatterLivre>::analisar(p.texto) else {
            continue;
        };
        let Some(tipo) = doc.frontmatter.tipo() else {
            continue;
        };
        let fm = serde_json::to_value(&doc.frontmatter.0).unwrap_or(serde_json::Value::Null);
        let chave = por_caminho_na_base
            .get(p.relativo)
            .cloned()
            .unwrap_or_else(|| {
                let id = fm
                    .get("id")
                    .and_then(|v| v.as_str())
                    .map(str::to_owned)
                    .unwrap_or_else(|| {
                        let nome = p.relativo.rsplit('/').next().unwrap_or(p.relativo);
                        nome.strip_suffix(".md").unwrap_or(nome).to_owned()
                    });
                (tipo.como_str().to_owned(), id)
            });
        docs.insert(
            chave.clone(),
            Doc {
                titulo: titulo(&fm, &chave.1),
                pagina: p.caminho.to_owned(),
                conteudo: p.texto.to_owned(),
                frontmatter: fm,
                antigo: true,
            },
        );
    }
    docs
}

/// Título legível: `titulo`, `termo` ou o id.
fn titulo(fm: &serde_json::Value, id: &str) -> String {
    ["titulo", "termo"]
        .iter()
        .find_map(|c| fm.get(c).and_then(|v| v.as_str()))
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .unwrap_or(id)
        .to_owned()
}

/// Compara os documentos das páginas `novas` (o que seria publicado) com os das
/// `anteriores` (o que o app publicou na branch base). `formato_antigo`: as
/// anteriores são os `.md` da base, da publicação da versão 1.
pub fn comparar_documentos(
    novas: &[PaginaLida<'_>],
    anteriores: &[PaginaLida<'_>],
    formato_antigo: bool,
) -> ResumoDosDocumentos {
    let (novos, por_caminho) = dos_blocos(novas);
    let antigos = if formato_antigo {
        dos_arquivos(anteriores, &por_caminho)
    } else {
        dos_blocos(anteriores).0
    };
    let mut documentos = Vec::new();
    let mut reformatados = 0;
    let mut empurrar = |(tipo, id): &Chave, d: &Doc, mudanca| {
        documentos.push(MudancaDeDocumento {
            tipo: tipo.clone(),
            id: id.clone(),
            titulo: d.titulo.clone(),
            mudanca,
            pagina: d.pagina.clone(),
        })
    };
    for (chave, n) in &novos {
        match antigos.get(chave) {
            None => empurrar(chave, n, TipoMudanca::Novo),
            Some(a) if a.antigo => {
                if a.frontmatter == n.frontmatter {
                    reformatados += 1;
                } else {
                    empurrar(chave, n, TipoMudanca::Alterado);
                }
            }
            Some(a) => {
                if a.conteudo != n.conteudo || a.frontmatter != n.frontmatter {
                    empurrar(chave, n, TipoMudanca::Alterado);
                }
            }
        }
    }
    for (chave, a) in &antigos {
        if !novos.contains_key(chave) {
            empurrar(chave, a, TipoMudanca::Removido);
        }
    }
    documentos.sort_by(|a, b| {
        (a.mudanca, Categoria::de(&a.tipo), &a.id).cmp(&(b.mudanca, Categoria::de(&b.tipo), &b.id))
    });
    let texto = texto(&documentos, reformatados);
    ResumoDosDocumentos {
        documentos,
        reformatados,
        texto,
    }
}

/// Como o documento é contado no resumo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Categoria {
    Regra,
    Termo,
    Funcionalidade,
    Analise,
    Sugestao,
    Outro,
}

impl Categoria {
    fn de(tipo: &str) -> Self {
        match tipo {
            "regra" => Self::Regra,
            "termo" => Self::Termo,
            "funcionalidade" | "caso-de-uso" => Self::Funcionalidade,
            "analise-impacto" | "pergunta" | "parecer-revisao" | "historias" | "cenarios-teste"
            | "analise-reuniao" => Self::Analise,
            "sugestao" => Self::Sugestao,
            _ => Self::Outro,
        }
    }

    /// (singular, plural, feminino)
    fn nome(self) -> (&'static str, &'static str, bool) {
        match self {
            Self::Regra => ("regra", "regras", true),
            Self::Termo => ("termo", "termos", false),
            Self::Funcionalidade => (
                "documento de funcionalidade",
                "documentos de funcionalidade",
                false,
            ),
            Self::Analise => ("análise", "análises", true),
            Self::Sugestao => ("sugestão", "sugestões", true),
            Self::Outro => ("documento", "documentos", false),
        }
    }
}

/// "nova", "alterados"... concordando com a categoria.
fn adjetivo(m: TipoMudanca, feminino: bool, plural: bool) -> String {
    let raiz = match m {
        TipoMudanca::Novo => "nov",
        TipoMudanca::Alterado => "alterad",
        TipoMudanca::Removido => "removid",
    };
    format!(
        "{raiz}{}{}",
        if feminino { "a" } else { "o" },
        if plural { "s" } else { "" }
    )
}

/// "3 regras novas, 1 alterada; 1 termo novo; 12 documentos só mudam de
/// formato".
fn texto(documentos: &[MudancaDeDocumento], reformatados: usize) -> String {
    let mut contagem: BTreeMap<Categoria, BTreeMap<TipoMudanca, usize>> = BTreeMap::new();
    for d in documentos {
        *contagem
            .entry(Categoria::de(&d.tipo))
            .or_default()
            .entry(d.mudanca)
            .or_default() += 1;
    }
    let mut partes: Vec<String> = contagem
        .into_iter()
        .map(|(cat, por_mudanca)| {
            let (singular, plural, feminino) = cat.nome();
            por_mudanca
                .into_iter()
                .enumerate()
                .map(|(i, (m, n))| {
                    let adj = adjetivo(m, feminino, n > 1);
                    if i == 0 {
                        format!("{n} {} {adj}", if n > 1 { plural } else { singular })
                    } else {
                        format!("{n} {adj}")
                    }
                })
                .collect::<Vec<_>>()
                .join(", ")
        })
        .collect();
    if reformatados > 0 {
        partes.push(if reformatados == 1 {
            "1 documento só muda de formato".to_owned()
        } else {
            format!("{reformatados} documentos só mudam de formato")
        });
    }
    partes.join("; ")
}
