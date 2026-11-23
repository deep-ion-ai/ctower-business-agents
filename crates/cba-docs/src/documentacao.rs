//! Documentação legível para a forja (#288).
//!
//! A base aprovada vira poucas páginas para ler no GitHub ou no GitLab, sem as
//! buscas do app: uma entrada com índice, uma página por funcionalidade do mapa,
//! as regras sem funcionalidade por área, o glossário e as análises, com links
//! cruzados e evidências como permalinks do código.
//!
//! ```text
//! README.md                                   entrada e índice por área
//! funcionalidades/<area>/<id>-<slug>.md       uma página por funcionalidade aceita
//! funcionalidades/<area>/<tema>-<doc>.md      documento negocial sem funcionalidade aceita
//! regras/<area>.md                            regras sem funcionalidade aceita
//! glossario.md                                termos em ordem alfabética
//! analises/README.md                          análises por tipo e data
//! analises/<id>.md                            uma análise, legível
//! ```
//!
//! [`gerar_documentacao`] é pura: recebe os documentos aprovados já lidos, o
//! mapa e o resolvedor de permalinks e devolve as páginas, sempre os mesmos
//! bytes para a mesma entrada (a data, se houver, só no README).
//!
//! - **Página canônica**: cada documento aparece inteiro numa página só, entre
//!   os marcadores de [`crate::marcador`]; nas outras, só como link. A regra vai
//!   para a página da primeira funcionalidade aceita dela (sem nenhuma, para
//!   `regras/<area>.md`); o termo, para o glossário; a análise, para a dela; a
//!   sugestão aceita, para a página da análise de origem; o documento negocial,
//!   para a página da funcionalidade que ele explica.
//! - **Vínculo com a funcionalidade**: o campo `funcionalidades` do frontmatter
//!   (#287). Sem ele (bases anteriores), vale o da análise de origem, e a análise
//!   sem o campo vale pela funcionalidade do mapa que a tem como explicação.
//!   Funcionalidade juntada vale pela de destino.
//! - **Âncoras** explícitas (`<a id="rn-cob-002"></a>`) derivadas do id, não do
//!   título.
//! - **Texto**: o preâmbulo do template (título e linha de metadados) é refeito
//!   aqui; os links para a base viram links para as páginas; todo id conhecido
//!   citado no texto (`RN-COB-002`, `A-0001`, `F-COB-001`, `S-0001`) vira link;
//!   as notas de evidência viram permalinks, sem o hash. Regras, termos e
//!   sugestões dividem página: as notas saem do texto e as evidências vão numa
//!   linha depois dele.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::ops::Range;

use cba_core::funcionalidades::{Funcionalidade, MapaFuncionalidades, SituacaoFuncionalidade};
use cba_kb::frontmatter::{
    FrontmatterAnalise, FrontmatterFuncionalidade, FrontmatterLivre, FrontmatterRegra,
    FrontmatterSugestao, FrontmatterTermo, Linhas, VersaoCodigo,
};
use cba_kb::{CaminhoRelativo, Documento, Frontmatter, TipoDocumento};
use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};
use serde::de::DeserializeOwned;

use crate::exportacao::{
    LinksDeEvidencia, aplicar, citacao, e_link_da_base, linkar_notas, opcoes_markdown,
};
use crate::marcador::{FECHAMENTO, Marcador, Metadados};
use crate::montagem::normalizar;

/// Documento aprovado da base, já lido.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentoDaBase {
    /// Caminho relativo à raiz do projeto (`regras/RN-COB-002.md`).
    pub caminho: String,
    /// Texto do arquivo, com o frontmatter.
    pub texto: String,
}

/// O que o README mostra além dos documentos.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContextoDaDocumentacao {
    /// Nome do projeto para quem lê ("OpenCBS Cloud").
    pub projeto: String,
    /// Nome das áreas declaradas no projeto (slug → nome). Sem ele, vale o nome
    /// da área no mapa e, sem esse, o slug.
    pub areas: BTreeMap<String, String>,
    /// Versão do código descrita. Sem ela, a base da análise aprovada mais
    /// recente.
    pub codigo: Option<VersaoCodigo>,
    /// Data da última mudança da base (AAAA-MM-DD), só no README ("Atualizada
    /// em"). Uma data que muda a cada publicação mudaria o README sempre; a
    /// publicação (#289) passa a da última mudança registrada nos documentos
    /// aprovados.
    pub data: Option<String>,
}

/// Página gerada, com o caminho relativo à pasta da documentação.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pagina {
    pub caminho: String,
    pub conteudo: String,
}

/// Resultado de [`gerar_documentacao`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Documentacao {
    /// Páginas em ordem de caminho.
    pub paginas: Vec<Pagina>,
    /// Documentos que ficaram de fora (frontmatter ilegível, id repetido).
    pub avisos: Vec<String>,
}

/// Página de entrada.
pub const README: &str = "README.md";
/// Glossário.
pub const GLOSSARIO: &str = "glossario.md";
/// Índice das análises.
pub const INDICE_ANALISES: &str = "analises/README.md";

/// Gera as páginas legíveis da documentação. `documentos` são os aprovados
/// (quem chama filtra); `links` resolve as evidências em permalinks (sem ele, ou
/// quando ele não resolve, a evidência fica como texto).
pub fn gerar_documentacao(
    documentos: &[DocumentoDaBase],
    mapa: Option<&MapaFuncionalidades>,
    links: Option<&dyn LinksDeEvidencia>,
    contexto: &ContextoDaDocumentacao,
) -> Documentacao {
    let mut g = Gerador::novo(documentos, mapa, links, contexto);
    let mut paginas = BTreeMap::new();
    g.gerar(&mut paginas);
    let mut avisos = std::mem::take(&mut g.avisos);
    avisos.sort();
    Documentacao {
        paginas: paginas
            .into_iter()
            .map(|(caminho, conteudo)| Pagina {
                caminho,
                conteudo: normalizar(&conteudo),
            })
            .collect(),
        avisos,
    }
}

// ---------------------------------------------------------------------------
// Documentos lidos

struct Doc<F> {
    caminho: String,
    tipo: String,
    id: String,
    frontmatter: serde_json::Value,
    corpo: String,
    fm: F,
}

impl<F> Doc<F> {
    fn marcador(&self) -> String {
        Marcador::novo(
            self.tipo.clone(),
            self.id.clone(),
            Metadados {
                caminho: self.caminho.clone(),
                frontmatter: self.frontmatter.clone(),
            },
        )
        .map(|m| m.abertura())
        .unwrap_or_default()
    }
}

type Regra = Doc<FrontmatterRegra>;
type Termo = Doc<FrontmatterTermo>;
type Analise = Doc<FrontmatterAnalise>;
type Negocial = Doc<FrontmatterFuncionalidade>;
type Sugestao = Doc<FrontmatterSugestao>;

fn ler<F: Frontmatter + DeserializeOwned>(
    d: &DocumentoDaBase,
    livre: &Documento<FrontmatterLivre>,
    tipo: TipoDocumento,
    id: impl FnOnce(&F) -> String,
) -> Result<Doc<F>, String> {
    let doc = Documento::<F>::analisar(&d.texto).map_err(|e| format!("{}: {e}", d.caminho))?;
    let id = id(&doc.frontmatter);
    Ok(Doc {
        caminho: d.caminho.clone(),
        tipo: tipo.como_str().to_owned(),
        frontmatter: serde_json::to_value(&livre.frontmatter.0).unwrap_or(serde_json::Value::Null),
        corpo: livre.corpo.clone(),
        id,
        fm: doc.frontmatter,
    })
}

/// Nome do arquivo sem a extensão.
fn nome_do_arquivo(caminho: &str) -> String {
    let nome = caminho.rsplit('/').next().unwrap_or(caminho);
    nome.strip_suffix(".md").unwrap_or(nome).to_owned()
}

/// Slug válido para pasta e âncora.
fn slug_de(texto: &str) -> String {
    if cba_kb::e_slug(texto) {
        texto.to_owned()
    } else {
        let s = cba_kb::slug(texto);
        if s.is_empty() { "sem-nome".into() } else { s }
    }
}

// ---------------------------------------------------------------------------
// Alvos de link

/// Onde um documento está publicado.
#[derive(Debug, Clone)]
struct Alvo {
    pagina: String,
    ancora: String,
    /// O documento abre a página: o link de fora vai para a página, sem âncora.
    topo: bool,
    rotulo: String,
}

/// Uma evidência para a lista depois do texto.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Ev {
    arquivo: String,
    linhas: (u32, u32),
    commit: Option<String>,
}

struct Gerador<'a> {
    contexto: &'a ContextoDaDocumentacao,
    links: Option<&'a dyn LinksDeEvidencia>,
    mapa: Option<&'a MapaFuncionalidades>,
    regras: Vec<Regra>,
    termos: Vec<Termo>,
    analises: Vec<Analise>,
    negociais: Vec<Negocial>,
    sugestoes: Vec<Sugestao>,
    /// Funcionalidade aceita → página.
    paginas_f: BTreeMap<String, String>,
    /// Chave (id do documento) → onde está.
    alvos: BTreeMap<String, Alvo>,
    /// Caminho na base → chave.
    por_caminho: BTreeMap<String, String>,
    /// Funcionalidades (resolvidas, abertas) de cada documento, pela chave.
    funcs: BTreeMap<String, Vec<String>>,
    avisos: Vec<String>,
}

fn chave_termo(slug: &str) -> String {
    format!("termo:{slug}")
}

fn chave_doc(id: &str) -> String {
    format!("doc:{id}")
}

impl<'a> Gerador<'a> {
    fn novo(
        documentos: &[DocumentoDaBase],
        mapa: Option<&'a MapaFuncionalidades>,
        links: Option<&'a dyn LinksDeEvidencia>,
        contexto: &'a ContextoDaDocumentacao,
    ) -> Self {
        let mut g = Gerador {
            contexto,
            links,
            mapa,
            regras: Vec::new(),
            termos: Vec::new(),
            analises: Vec::new(),
            negociais: Vec::new(),
            sugestoes: Vec::new(),
            paginas_f: BTreeMap::new(),
            alvos: BTreeMap::new(),
            por_caminho: BTreeMap::new(),
            funcs: BTreeMap::new(),
            avisos: Vec::new(),
        };
        let mut ordenados: Vec<&DocumentoDaBase> = documentos.iter().collect();
        ordenados.sort_by(|a, b| a.caminho.cmp(&b.caminho));
        for d in ordenados {
            if let Err(e) = g.ler(d) {
                g.avisos.push(e);
            }
        }
        g.regras.sort_by(|a, b| a.id.cmp(&b.id));
        g.analises.sort_by(|a, b| a.id.cmp(&b.id));
        g.sugestoes.sort_by(|a, b| a.id.cmp(&b.id));
        g.termos.sort_by(|a, b| {
            (cba_kb::slug(&a.fm.termo), &a.id).cmp(&(cba_kb::slug(&b.fm.termo), &b.id))
        });
        g.negociais.sort_by(|a, b| a.id.cmp(&b.id));
        g.dedup();
        g.posicionar();
        g
    }

    fn ler(&mut self, d: &DocumentoDaBase) -> Result<(), String> {
        let livre = Documento::<FrontmatterLivre>::analisar(&d.texto)
            .map_err(|e| format!("{}: {e}", d.caminho))?;
        let Some(tipo) = livre.frontmatter.tipo() else {
            return Err(format!("{}: tipo de documento desconhecido", d.caminho));
        };
        let em_analises = d.caminho.starts_with("analises/");
        use TipoDocumento as T;
        match tipo {
            T::Regra => self
                .regras
                .push(ler(d, &livre, tipo, |f: &FrontmatterRegra| {
                    f.id.to_string()
                })?),
            T::Termo => {
                let slug = nome_do_arquivo(&d.caminho);
                self.termos
                    .push(ler(d, &livre, tipo, |_: &FrontmatterTermo| slug_de(&slug))?)
            }
            T::Sugestao => self
                .sugestoes
                .push(ler(d, &livre, tipo, |f: &FrontmatterSugestao| {
                    f.id.to_string()
                })?),
            T::AnaliseImpacto
            | T::Pergunta
            | T::ParecerRevisao
            | T::Historias
            | T::CenariosTeste
            | T::AnaliseReuniao => {
                self.analises
                    .push(ler(d, &livre, tipo, |f: &FrontmatterAnalise| {
                        f.id.to_string()
                    })?)
            }
            T::Funcionalidade if em_analises => {
                self.analises
                    .push(ler(d, &livre, tipo, |f: &FrontmatterAnalise| {
                        f.id.to_string()
                    })?)
            }
            T::Funcionalidade | T::CasoDeUso => {
                let id = d
                    .caminho
                    .strip_prefix("funcionalidades/")
                    .unwrap_or(&d.caminho);
                let id = id.strip_suffix(".md").unwrap_or(id).to_owned();
                self.negociais
                    .push(ler(d, &livre, tipo, |_: &FrontmatterFuncionalidade| id)?)
            }
            // O README e as páginas das funcionalidades substituem o mapa.
            T::MapaFuncionalidades => {}
            outro => {
                return Err(format!(
                    "{}: documento do tipo {outro} não entra na documentação",
                    d.caminho
                ));
            }
        }
        Ok(())
    }

    fn dedup(&mut self) {
        fn tirar<F>(docs: &mut Vec<Doc<F>>, avisos: &mut Vec<String>) {
            let mut vistos = BTreeSet::new();
            docs.retain(|d| {
                let novo = vistos.insert(d.id.clone());
                if !novo {
                    avisos.push(format!(
                        "{}: o id {} se repete; vale o primeiro",
                        d.caminho, d.id
                    ));
                }
                novo
            });
        }
        tirar(&mut self.regras, &mut self.avisos);
        tirar(&mut self.termos, &mut self.avisos);
        tirar(&mut self.analises, &mut self.avisos);
        tirar(&mut self.negociais, &mut self.avisos);
        tirar(&mut self.sugestoes, &mut self.avisos);
    }

    // -- Funcionalidades ----------------------------------------------------

    fn funcionalidade(&self, id: &str) -> Option<&'a Funcionalidade> {
        let mapa = self.mapa?;
        let mut atual = mapa
            .funcionalidades
            .iter()
            .find(|f| f.id.to_string() == id)?;
        for _ in 0..8 {
            match (&atual.situacao, &atual.juntada_em) {
                (SituacaoFuncionalidade::Juntada, Some(destino)) => {
                    atual = mapa.funcionalidade(destino)?;
                }
                _ => break,
            }
        }
        atual.aberta().then_some(atual)
    }

    /// Ids abertos (juntadas resolvidas), sem repetir, na ordem.
    fn resolver_funcs<'s>(&self, ids: impl IntoIterator<Item = &'s str>) -> Vec<String> {
        let mut saida: Vec<String> = Vec::new();
        for id in ids {
            if let Some(f) = self.funcionalidade(id) {
                let id = f.id.to_string();
                if !saida.contains(&id) {
                    saida.push(id);
                }
            }
        }
        saida
    }

    fn area_nome(&self, slug: &str) -> String {
        if let Some(n) = self.contexto.areas.get(slug) {
            return n.clone();
        }
        if let Some(f) = self
            .mapa
            .and_then(|m| m.funcionalidades.iter().find(|f| f.area == slug))
            .filter(|f| !f.area_nome.trim().is_empty())
        {
            return f.area_nome.clone();
        }
        let texto = slug.replace('-', " ");
        let mut c = texto.chars();
        match c.next() {
            Some(p) => p.to_uppercase().chain(c).collect(),
            None => texto,
        }
    }

    fn pagina_da_area(area: &str) -> String {
        format!("regras/{}.md", slug_de(area))
    }

    fn posicionar(&mut self) {
        // Páginas das funcionalidades aceitas.
        if let Some(mapa) = self.mapa {
            for f in &mapa.funcionalidades {
                if f.situacao == SituacaoFuncionalidade::Aceita {
                    let id = f.id.to_string();
                    let pagina = format!(
                        "funcionalidades/{}/{}-{}.md",
                        slug_de(&f.area),
                        id.to_lowercase(),
                        slug_de(&f.nome)
                    );
                    self.alvos.insert(
                        id.clone(),
                        Alvo {
                            pagina: pagina.clone(),
                            ancora: id.to_lowercase(),
                            topo: true,
                            rotulo: format!("{id} · {}", f.nome),
                        },
                    );
                    self.paginas_f.insert(id, pagina);
                }
            }
        }

        // Funcionalidades de cada documento.
        let mut por_analise: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for a in &self.analises {
            let mut ids: Vec<String> = a.fm.funcionalidades.iter().map(|f| f.to_string()).collect();
            if let Some(m) = self.mapa {
                ids.extend(
                    m.funcionalidades
                        .iter()
                        .filter(|f| f.explicacao.as_ref().is_some_and(|e| e.to_string() == a.id))
                        .map(|f| f.id.to_string()),
                );
            }
            let r = self.resolver_funcs(ids.iter().map(String::as_str));
            por_analise.insert(a.caminho.clone(), r.clone());
            self.funcs.insert(a.id.clone(), r);
        }
        let herdar = |proprias: &[cba_kb::IdFuncionalidade],
                      origem: Option<&CaminhoRelativo>,
                      g: &Gerador<'_>|
         -> Vec<String> {
            if proprias.is_empty() {
                origem
                    .and_then(|o| por_analise.get(o.as_str()))
                    .cloned()
                    .unwrap_or_default()
            } else {
                let ids: Vec<String> = proprias.iter().map(|f| f.to_string()).collect();
                g.resolver_funcs(ids.iter().map(String::as_str))
            }
        };
        let regras: Vec<(String, Vec<String>)> = self
            .regras
            .iter()
            .map(|r| {
                (
                    r.id.clone(),
                    herdar(&r.fm.funcionalidades, r.fm.origem.as_ref(), self),
                )
            })
            .collect();
        let termos: Vec<(String, Vec<String>)> = self
            .termos
            .iter()
            .map(|t| {
                (
                    chave_termo(&t.id),
                    herdar(&t.fm.funcionalidades, t.fm.origem.as_ref(), self),
                )
            })
            .collect();
        let negociais: Vec<(String, Vec<String>)> = self
            .negociais
            .iter()
            .map(|n| {
                let mut ids: Vec<String> =
                    n.fm.funcionalidade.iter().map(|f| f.to_string()).collect();
                if let Some(m) = self.mapa {
                    ids.extend(
                        m.funcionalidades
                            .iter()
                            .filter(|f| {
                                f.documento
                                    .as_ref()
                                    .is_some_and(|d| d.as_str() == n.caminho)
                            })
                            .map(|f| f.id.to_string()),
                    );
                }
                (
                    chave_doc(&n.id),
                    self.resolver_funcs(ids.iter().map(String::as_str)),
                )
            })
            .collect();
        self.funcs.extend(regras);
        self.funcs.extend(termos);
        self.funcs.extend(negociais);

        // Alvos dos documentos.
        let mut alvos = Vec::new();
        for r in &self.regras {
            let pagina = self
                .canonica(&r.id)
                .unwrap_or_else(|| Self::pagina_da_area(&r.fm.area));
            alvos.push((
                r.id.clone(),
                r.caminho.clone(),
                Alvo {
                    pagina,
                    ancora: r.id.to_lowercase(),
                    topo: false,
                    rotulo: format!("{} · {}", r.id, r.fm.titulo),
                },
            ));
        }
        for t in &self.termos {
            alvos.push((
                chave_termo(&t.id),
                t.caminho.clone(),
                Alvo {
                    pagina: GLOSSARIO.into(),
                    ancora: t.id.clone(),
                    topo: false,
                    rotulo: t.fm.termo.clone(),
                },
            ));
        }
        for a in &self.analises {
            alvos.push((
                a.id.clone(),
                a.caminho.clone(),
                Alvo {
                    pagina: format!("analises/{}.md", a.id),
                    ancora: a.id.to_lowercase(),
                    topo: true,
                    rotulo: format!("{} · {}", a.id, a.fm.titulo),
                },
            ));
        }
        for n in &self.negociais {
            let chave = chave_doc(&n.id);
            let pagina = self.canonica(&chave).unwrap_or_else(|| {
                let resto =
                    n.id.strip_prefix(&format!("{}/", n.fm.area))
                        .unwrap_or(&n.id);
                format!(
                    "funcionalidades/{}/{}.md",
                    slug_de(&n.fm.area),
                    slug_de(&resto.replace('/', "-"))
                )
            });
            let topo = !self.paginas_f.values().any(|p| *p == pagina);
            alvos.push((
                chave,
                n.caminho.clone(),
                Alvo {
                    pagina,
                    ancora: slug_de(&n.id.replace('/', "-")),
                    topo,
                    rotulo: n.fm.titulo.clone(),
                },
            ));
        }
        let analises: BTreeSet<String> = self.analises.iter().map(|a| a.id.clone()).collect();
        for s in &self.sugestoes {
            let pagina = match s.fm.analise.as_ref().map(|a| a.to_string()) {
                Some(a) if analises.contains(&a) => format!("analises/{a}.md"),
                _ => INDICE_ANALISES.into(),
            };
            alvos.push((
                s.id.clone(),
                s.caminho.clone(),
                Alvo {
                    pagina,
                    ancora: s.id.to_lowercase(),
                    topo: false,
                    rotulo: format!("{} · {}", s.id, s.fm.titulo),
                },
            ));
        }
        for (chave, caminho, alvo) in alvos {
            self.por_caminho.insert(caminho, chave.clone());
            self.alvos.insert(chave, alvo);
        }
    }

    /// Página da primeira funcionalidade aceita do documento.
    fn canonica(&self, chave: &str) -> Option<String> {
        self.funcs
            .get(chave)?
            .iter()
            .find_map(|f| self.paginas_f.get(f).cloned())
    }

    fn funcs_de(&self, chave: &str) -> &[String] {
        self.funcs.get(chave).map_or(&[], Vec::as_slice)
    }

    // -- Links --------------------------------------------------------------

    /// Destino do link de `de` para o documento `chave`.
    fn href(&self, de: &str, chave: &str) -> Option<String> {
        let alvo = self.alvos.get(chave)?;
        Some(if alvo.pagina == de {
            format!("#{}", alvo.ancora)
        } else if alvo.topo {
            relativo(de, &alvo.pagina)
        } else {
            format!("{}#{}", relativo(de, &alvo.pagina), alvo.ancora)
        })
    }

    /// Link Markdown com o rótulo do alvo; sem alvo, o texto puro.
    fn link(&self, de: &str, chave: &str, texto: Option<&str>) -> String {
        let rotulo = self
            .alvos
            .get(chave)
            .map(|a| a.rotulo.clone())
            .unwrap_or_else(|| chave.to_owned());
        let texto = texto.unwrap_or(&rotulo);
        match self.href(de, chave) {
            Some(h) => md_link(texto, &h),
            None => escapar(texto),
        }
    }

    /// Link para a funcionalidade: a página se aceita; senão o nome, com a
    /// situação.
    fn link_funcionalidade(&self, de: &str, id: &str) -> String {
        if self.paginas_f.contains_key(id) {
            return self.link(de, id, None);
        }
        match self.funcionalidade(id) {
            Some(f) => format!("{id} · {} (ainda em revisão no mapa)", escapar(&f.nome)),
            None => id.to_owned(),
        }
    }

    // -- Evidências ---------------------------------------------------------

    fn evidencia_md(&self, e: &Ev, commit_padrao: Option<&str>) -> String {
        let linhas = if e.linhas.0 == e.linhas.1 {
            e.linhas.0.to_string()
        } else {
            format!("{}-{}", e.linhas.0, e.linhas.1)
        };
        let commit = e.commit.as_deref().or(commit_padrao);
        let url = self.links.and_then(|l| {
            l.link(
                &e.arquivo,
                Linhas::novo(e.linhas.0, e.linhas.1)?,
                commit.filter(|c| !c.trim().is_empty()),
            )
        });
        match url {
            Some(url) => {
                let nome = e.arquivo.rsplit('/').next().unwrap_or(&e.arquivo);
                format!(
                    "[`{nome}:{linhas}`](<{url}> \"{}\")",
                    e.arquivo.replace('"', "'")
                )
            }
            None => format!("`{}:{linhas}`", e.arquivo),
        }
    }

    fn linha_evidencias(&self, evs: &[Ev], commit_padrao: Option<&str>) -> String {
        let mut vistos = BTreeSet::new();
        let itens: Vec<String> = evs
            .iter()
            .filter(|e| vistos.insert((e.arquivo.clone(), e.linhas)))
            .map(|e| self.evidencia_md(e, commit_padrao))
            .collect();
        if itens.is_empty() {
            String::new()
        } else {
            format!("**Evidências no código:** {}\n", itens.join(" · "))
        }
    }

    fn commit_da_analise(&self, caminho: Option<&str>) -> Option<String> {
        let a = self
            .analises
            .iter()
            .find(|a| Some(a.caminho.as_str()) == caminho)?;
        a.fm.base.as_ref()?.commit.clone()
    }

    fn versao(&self) -> Option<VersaoCodigo> {
        if let Some(v) = &self.contexto.codigo {
            return Some(v.clone());
        }
        self.analises
            .iter()
            .filter(|a| a.fm.base.as_ref().is_some_and(|b| b.commit.is_some()))
            .max_by(|a, b| (a.fm.criado.to_string(), &a.id).cmp(&(b.fm.criado.to_string(), &b.id)))
            .and_then(|a| a.fm.base.clone())
    }

    // -- Texto --------------------------------------------------------------

    /// O corpo do documento pronto para a página `pagina`.
    fn texto(&self, corpo: &str, o: &OpcoesTexto<'_>) -> String {
        let s = sem_preambulo(corpo);
        let eventos: Vec<(Event, Range<usize>)> = Parser::new_ext(s, opcoes_markdown())
            .into_offset_iter()
            .collect();
        let mut edicoes: Vec<(Range<usize>, String)> = Vec::new();
        let mut titulo: Option<(Range<usize>, String)> = None;
        let mut link: Option<(usize, usize, Option<usize>, String)> = None;
        let mut codigo = 0usize;
        let mut nota_def = 0usize;
        let mut em_link = 0usize;
        for (evento, r) in &eventos {
            match evento {
                Event::Start(Tag::Heading { .. }) => titulo = Some((r.clone(), String::new())),
                Event::End(TagEnd::Heading(_)) => {
                    if let Some((r, t)) = titulo.take() {
                        let t = t.trim().to_owned();
                        let fonte = &s[r.clone()];
                        let conteudo = fonte.trim_end();
                        let resto = &fonte[conteudo.len()..];
                        if t == "Evidências" || o.desembrulhar.contains(&t.as_str()) {
                            edicoes.push((r, String::new()));
                        } else if o.rotulos {
                            edicoes.push((r, format!("**{}**{resto}", t)));
                        } else if o.rebaixar > 0 && conteudo.starts_with('#') {
                            let n = conteudo.bytes().take_while(|b| *b == b'#').count();
                            let novo = (n + o.rebaixar).min(6);
                            edicoes.push((
                                r,
                                format!("{}{}{resto}", "#".repeat(novo), &conteudo[n..]),
                            ));
                        }
                    }
                }
                Event::Start(Tag::Link { dest_url, .. }) => {
                    em_link += 1;
                    if e_link_da_base(dest_url) {
                        link = Some((r.start, r.start + 1, None, dest_url.to_string()));
                    }
                }
                Event::Start(Tag::Image { .. }) => em_link += 1,
                Event::End(TagEnd::Link | TagEnd::Image) => {
                    em_link = em_link.saturating_sub(1);
                    if let Some((inicio, texto, fim, destino)) = link.take() {
                        let interno = fim.map_or("", |fim| &s[texto..fim]);
                        let novo = match self
                            .destino_da_base(o.origem, &destino)
                            .and_then(|chave| self.href(o.pagina, &chave))
                        {
                            Some(h) => md_link_cru(interno, &h),
                            None => interno.to_owned(),
                        };
                        edicoes.push((inicio..r.end, novo));
                    }
                }
                Event::Start(Tag::CodeBlock(_)) => codigo += 1,
                Event::End(TagEnd::CodeBlock) => codigo = codigo.saturating_sub(1),
                Event::FootnoteReference(rotulo) => match &o.notas {
                    Notas::Tirar => edicoes.push((r.clone(), String::new())),
                    Notas::Prefixar(p) => edicoes.push((r.clone(), format!("[^{p}-{rotulo}]"))),
                    Notas::Manter => {}
                },
                Event::Start(Tag::FootnoteDefinition(rotulo)) => {
                    nota_def += 1;
                    match &o.notas {
                        Notas::Tirar => edicoes.push((r.clone(), String::new())),
                        Notas::Prefixar(p) => {
                            let antigo = format!("[^{rotulo}]");
                            if s[r.start..].starts_with(&antigo) {
                                edicoes.push((
                                    r.start..r.start + antigo.len(),
                                    format!("[^{p}-{rotulo}]"),
                                ));
                            }
                        }
                        Notas::Manter => {}
                    }
                }
                Event::End(TagEnd::FootnoteDefinition) => nota_def = nota_def.saturating_sub(1),
                Event::Text(t) => {
                    if let Some((_, tt)) = &mut titulo {
                        tt.push_str(t);
                    } else if em_link == 0 && codigo == 0 && nota_def == 0 {
                        for (sub, id) in ids_no_texto(&s[r.clone()]) {
                            if id != o.proprio && self.alvos.contains_key(id) {
                                let a = r.start + sub.start;
                                let b = r.start + sub.end;
                                if let Some(h) = self.href(o.pagina, id) {
                                    edicoes.push((a..b, format!("[{id}]({h})")));
                                }
                            }
                        }
                    }
                }
                Event::Code(t) => {
                    if let Some((_, tt)) = &mut titulo {
                        tt.push_str(t);
                    }
                }
                _ => {}
            }
            if let Some((_, _, fim, _)) = &mut link
                && !matches!(evento, Event::Start(Tag::Link { .. } | Tag::Image { .. }))
            {
                *fim = Some(fim.unwrap_or(0).max(r.end));
            }
        }
        let texto = normalizar(aplicar(s, edicoes).trim_start_matches(['\n', '\r']));
        if matches!(o.notas, Notas::Tirar) {
            return texto;
        }
        let texto = match self.links {
            Some(l) => linkar_notas(&texto, l, o.commit_padrao),
            None => texto,
        };
        texto
            .split_inclusive('\n')
            .map(limpar_nota)
            .collect::<String>()
    }

    /// Chave do documento da base para onde aponta o link `destino`, escrito no
    /// documento `origem`.
    fn destino_da_base(&self, origem: &str, destino: &str) -> Option<String> {
        let destino = destino.split('#').next().unwrap_or(destino);
        let mut partes: Vec<&str> = origem.split('/').collect();
        partes.pop();
        for p in destino.split('/') {
            match p {
                "" | "." => {}
                ".." => {
                    partes.pop()?;
                }
                p => partes.push(p),
            }
        }
        self.por_caminho.get(&partes.join("/")).cloned()
    }

    // -- Geração ------------------------------------------------------------

    fn gerar(&mut self, paginas: &mut BTreeMap<String, String>) {
        let mut por_pagina_regras: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (i, r) in self.regras.iter().enumerate() {
            let pagina = self.alvos[&r.id].pagina.clone();
            por_pagina_regras.entry(pagina).or_default().push(i);
        }
        // Funcionalidades.
        for (id, pagina) in &self.paginas_f {
            let f = self.funcionalidade(id).expect("aceita está no mapa");
            paginas.insert(pagina.clone(), self.pagina_funcionalidade(f, pagina));
        }
        // Documentos negociais sem funcionalidade aceita.
        for n in &self.negociais {
            let alvo = &self.alvos[&chave_doc(&n.id)];
            if alvo.topo {
                paginas.insert(alvo.pagina.clone(), self.pagina_negocial(n, &alvo.pagina));
            }
        }
        // Regras sem funcionalidade.
        for (pagina, idx) in &por_pagina_regras {
            if pagina.starts_with("regras/") {
                paginas.insert(pagina.clone(), self.pagina_area(pagina, idx));
            }
        }
        if !self.termos.is_empty() {
            paginas.insert(GLOSSARIO.into(), self.glossario());
        }
        for a in &self.analises {
            let pagina = self.alvos[&a.id].pagina.clone();
            let conteudo = self.pagina_analise(a, &pagina);
            paginas.insert(pagina, conteudo);
        }
        if !self.analises.is_empty() || !self.sugestoes.is_empty() {
            paginas.insert(INDICE_ANALISES.into(), self.indice_analises());
        }
        paginas.insert(README.into(), self.readme(&por_pagina_regras));
    }

    fn inicio(&self, de: &str) -> String {
        md_link("Início", &relativo(de, README))
    }

    /// Bloco de uma regra, com o título no nível `nivel`.
    fn bloco_regra(&self, r: &Regra, pagina: &str, nivel: usize) -> String {
        let mut s = String::new();
        let _ = writeln!(s, "{}", r.marcador());
        let _ = writeln!(s, "<a id=\"{}\"></a>", r.id.to_lowercase());
        let _ = writeln!(
            s,
            "{} {} · {}\n",
            "#".repeat(nivel),
            r.id,
            escapar(&r.fm.titulo)
        );
        let commit_padrao = self.commit_da_analise(r.fm.origem.as_ref().map(|o| o.as_str()));
        let texto = self.texto(
            &r.corpo,
            &OpcoesTexto {
                pagina,
                origem: &r.caminho,
                proprio: &r.id,
                rebaixar: 0,
                rotulos: true,
                desembrulhar: &["Enunciado"],
                notas: Notas::Tirar,
                commit_padrao: None,
            },
        );
        let _ = writeln!(s, "{}", texto.trim_end());
        let mut evs: Vec<Ev> =
            r.fm.evidencias
                .iter()
                .map(|e| Ev {
                    arquivo: e.arquivo.as_str().to_owned(),
                    linhas: (e.linhas.inicio, e.linhas.fim),
                    commit: e.commit.clone(),
                })
                .collect();
        if evs.is_empty() {
            evs = evidencias_das_notas(&r.corpo);
        }
        let mut meta = Vec::new();
        let ev = self.linha_evidencias(&evs, commit_padrao.as_deref());
        if !ev.is_empty() {
            meta.push(ev.trim_end().to_owned());
        }
        let funcs = self.funcs_de(&r.id);
        if !funcs.is_empty() {
            let outras: Vec<String> = funcs
                .iter()
                .filter(|f| self.paginas_f.get(*f).is_none_or(|p| p != pagina))
                .map(|f| self.link_funcionalidade(pagina, f))
                .collect();
            if !outras.is_empty() {
                let rotulo = if pagina.starts_with("funcionalidades/") {
                    "Também em"
                } else {
                    "Funcionalidades"
                };
                meta.push(format!("**{rotulo}:** {}", outras.join(" · ")));
            }
        }
        if let Some(o) = &r.fm.origem
            && let Some(chave) = self.por_caminho.get(o.as_str())
        {
            meta.push(format!("**Origem:** {}", self.link(pagina, chave, None)));
        }
        if !meta.is_empty() {
            let _ = writeln!(s);
            for m in meta {
                let _ = writeln!(s, "* {m}");
            }
        }
        let _ = writeln!(s, "{FECHAMENTO}\n");
        s
    }

    fn indice_de_regras(&self, pagina: &str, idx: &[usize]) -> String {
        let mut s = String::new();
        for i in idx {
            let r = &self.regras[*i];
            let _ = writeln!(s, "- {}", self.link(pagina, &r.id, None));
        }
        s
    }

    fn pagina_funcionalidade(&self, f: &Funcionalidade, pagina: &str) -> String {
        let id = f.id.to_string();
        let mut s = String::new();
        let _ = writeln!(
            s,
            "{} › {}\n",
            self.inicio(pagina),
            escapar(&self.area_nome(&f.area))
        );
        let _ = writeln!(s, "<a id=\"{}\"></a>", id.to_lowercase());
        let _ = writeln!(s, "# {id} · {}\n", escapar(&f.nome));
        if !f.descricao.trim().is_empty() {
            let _ = writeln!(s, "{}\n", f.descricao.trim());
        }
        let mut meta = vec![format!("**Área:** {}", escapar(&self.area_nome(&f.area)))];
        if !f.atores.is_empty() {
            meta.push(format!("**Atores:** {}", escapar(&f.atores.join(", "))));
        }
        if !f.dados_lidos.is_empty() {
            meta.push(format!(
                "**Dados lidos:** {}",
                escapar(&f.dados_lidos.join(", "))
            ));
        }
        if !f.dados_gravados.is_empty() {
            meta.push(format!(
                "**Dados gravados:** {}",
                escapar(&f.dados_gravados.join(", "))
            ));
        }
        if !f.relacionadas.is_empty() {
            let ids: Vec<String> = f.relacionadas.iter().map(|r| r.to_string()).collect();
            let rel: Vec<String> = self
                .resolver_funcs(ids.iter().map(String::as_str))
                .iter()
                .filter(|r| **r != id)
                .map(|r| self.link_funcionalidade(pagina, r))
                .collect();
            if !rel.is_empty() {
                meta.push(format!("**Relacionadas:** {}", rel.join(" · ")));
            }
        }
        for m in meta {
            let _ = writeln!(s, "- {m}");
        }
        let _ = writeln!(s);

        // Documentos negociais.
        let negociais: Vec<&Negocial> = self
            .negociais
            .iter()
            .filter(|n| self.alvos[&chave_doc(&n.id)].pagina == pagina)
            .collect();
        for n in &negociais {
            let titulo = if negociais.len() == 1 && n.tipo == "funcionalidade" {
                "Como funciona".to_owned()
            } else {
                escapar(&n.fm.titulo)
            };
            let _ = writeln!(s, "## {titulo}\n");
            s.push_str(&self.bloco_negocial(n, pagina, 1));
        }

        // Regras.
        let chave_f = id.clone();
        let proprias: Vec<usize> = (0..self.regras.len())
            .filter(|i| self.alvos[&self.regras[*i].id].pagina == pagina)
            .collect();
        let de_outras: Vec<usize> = (0..self.regras.len())
            .filter(|i| {
                let r = &self.regras[*i];
                self.alvos[&r.id].pagina != pagina && self.funcs_de(&r.id).contains(&chave_f)
            })
            .collect();
        if !proprias.is_empty() || !de_outras.is_empty() {
            let _ = writeln!(s, "## Regras de negócio\n");
            if proprias.len() > 3 {
                s.push_str(&self.indice_de_regras(pagina, &proprias));
                let _ = writeln!(s);
            }
            for i in &proprias {
                s.push_str(&self.bloco_regra(&self.regras[*i], pagina, 3));
            }
            if !de_outras.is_empty() {
                let _ = writeln!(s, "Regras descritas em outra funcionalidade:\n");
                s.push_str(&self.indice_de_regras(pagina, &de_outras));
                let _ = writeln!(s);
            }
        }

        // Termos.
        let termos = self.termos_da_funcionalidade(&id, &proprias, &negociais);
        if !termos.is_empty() {
            let _ = writeln!(s, "## Termos usados\n");
            for t in termos {
                let def = primeira_frase(&self.texto_puro(&t.corpo));
                let _ = write!(s, "- {}", self.link(pagina, &chave_termo(&t.id), None));
                if !def.is_empty() {
                    let _ = write!(s, ": {}", escapar(&def));
                }
                let _ = writeln!(s);
            }
            let _ = writeln!(s);
        }

        // Análises.
        let analises: Vec<&Analise> = self
            .analises
            .iter()
            .filter(|a| self.funcs_de(&a.id).contains(&id))
            .collect();
        if !analises.is_empty() {
            let _ = writeln!(s, "## Análises\n");
            for a in analises {
                let _ = writeln!(
                    s,
                    "- {} ({}, {})",
                    self.link(pagina, &a.id, None),
                    rotulo_tipo(&a.tipo).0,
                    a.fm.criado
                );
            }
            let _ = writeln!(s);
        }

        // Pontos de entrada.
        if !f.pontos.is_empty() {
            let _ = writeln!(s, "## Pontos de entrada no código\n");
            let mut pontos: Vec<_> = f.pontos.iter().collect();
            pontos.sort_by(|a, b| {
                (a.tipo.map(|t| t.nome()), &a.nome).cmp(&(b.tipo.map(|t| t.nome()), &b.nome))
            });
            for p in pontos {
                let ev = Ev {
                    arquivo: p.evidencia.arquivo.as_str().to_owned(),
                    linhas: (p.evidencia.linhas.inicio, p.evidencia.linhas.fim),
                    commit: p.evidencia.commit.clone(),
                };
                let tipo = p.tipo.map(|t| t.nome()).unwrap_or("ponto");
                let _ = writeln!(
                    s,
                    "- {} `{}`: {}",
                    primeira_maiuscula(tipo),
                    p.nome.replace('`', "'"),
                    self.evidencia_md(&ev, self.versao().and_then(|v| v.commit).as_deref())
                );
            }
            let _ = writeln!(s);
        }
        s
    }

    fn termos_da_funcionalidade(
        &self,
        id: &str,
        regras: &[usize],
        negociais: &[&Negocial],
    ) -> Vec<&Termo> {
        let f = self.funcionalidade(id);
        let mut texto = String::new();
        if let Some(f) = f {
            texto.push_str(&f.descricao);
            texto.push(' ');
        }
        for i in regras {
            texto.push_str(&self.regras[*i].corpo);
            texto.push(' ');
        }
        for n in negociais {
            texto.push_str(&n.corpo);
            texto.push(' ');
        }
        let busca = para_busca(&texto);
        self.termos
            .iter()
            .filter(|t| {
                self.funcs_de(&chave_termo(&t.id)).iter().any(|f| f == id)
                    || menciona(&busca, &t.fm.termo, &t.fm.sinonimos)
            })
            .collect()
    }

    fn bloco_negocial(&self, n: &Negocial, pagina: &str, rebaixar: usize) -> String {
        let mut s = String::new();
        let ancora = &self.alvos[&chave_doc(&n.id)].ancora;
        let _ = writeln!(s, "{}", n.marcador());
        let _ = writeln!(s, "<a id=\"{ancora}\"></a>\n");
        let commit = self.commit_da_analise(n.fm.origem.as_ref().map(|o| o.as_str()));
        let texto = self.texto(
            &n.corpo,
            &OpcoesTexto {
                pagina,
                origem: &n.caminho,
                proprio: "",
                rebaixar,
                rotulos: false,
                desembrulhar: &[],
                notas: Notas::Prefixar(ancora.clone()),
                commit_padrao: commit.as_deref(),
            },
        );
        let _ = writeln!(s, "{}", texto.trim_end());
        if let Some(o) = &n.fm.origem
            && let Some(chave) = self.por_caminho.get(o.as_str())
        {
            let _ = writeln!(
                s,
                "\n*Explicação aprovada na análise {}.*",
                self.link(pagina, chave, Some(chave))
            );
        }
        let _ = writeln!(s, "{FECHAMENTO}\n");
        s
    }

    fn pagina_negocial(&self, n: &Negocial, pagina: &str) -> String {
        let mut s = String::new();
        let _ = writeln!(
            s,
            "{} › {}\n",
            self.inicio(pagina),
            escapar(&self.area_nome(&n.fm.area))
        );
        let _ = writeln!(s, "# {}\n", escapar(&n.fm.titulo));
        let funcs = self.funcs_de(&chave_doc(&n.id));
        if !funcs.is_empty() {
            let l: Vec<String> = funcs
                .iter()
                .map(|f| self.link_funcionalidade(pagina, f))
                .collect();
            let _ = writeln!(s, "- **Funcionalidade:** {}\n", l.join(" · "));
        }
        s.push_str(&self.bloco_negocial(n, pagina, 0));
        s
    }

    fn pagina_area(&self, pagina: &str, idx: &[usize]) -> String {
        let area = &self.regras[idx[0]].fm.area;
        let nome = self.area_nome(area);
        let mut s = String::new();
        let _ = writeln!(s, "{} › Regras sem funcionalidade\n", self.inicio(pagina));
        let _ = writeln!(s, "# Regras de negócio · {}\n", escapar(&nome));
        let _ = writeln!(
            s,
            "{} da área {} que ainda não {} a uma funcionalidade aceita do mapa.\n",
            contagem(idx.len(), "regra confirmada", "regras confirmadas"),
            escapar(&nome),
            if idx.len() == 1 {
                "está ligada"
            } else {
                "estão ligadas"
            }
        );
        let _ = writeln!(s, "## Índice\n");
        s.push_str(&self.indice_de_regras(pagina, idx));
        let _ = writeln!(s, "\n## Regras\n");
        for i in idx {
            s.push_str(&self.bloco_regra(&self.regras[*i], pagina, 3));
        }
        s
    }

    fn glossario(&self) -> String {
        let pagina = GLOSSARIO;
        let mut s = String::new();
        let _ = writeln!(s, "{} › Glossário\n", self.inicio(pagina));
        let _ = writeln!(s, "# Glossário\n");
        let _ = writeln!(
            s,
            "{} do negócio, em ordem alfabética, com as regras e funcionalidades que os usam.\n",
            contagem(self.termos.len(), "termo", "termos")
        );
        let letra = |t: &Termo| {
            cba_kb::slug(&t.fm.termo)
                .chars()
                .next()
                .map_or('#', |c| c.to_ascii_uppercase())
        };
        let letras: BTreeSet<char> = self.termos.iter().map(letra).collect();
        if letras.len() > 1 {
            let l: Vec<String> = letras
                .iter()
                .map(|c| format!("[{c}](#letra-{})", c.to_ascii_lowercase()))
                .collect();
            let _ = writeln!(s, "{}\n", l.join(" · "));
        }
        let busca_regras: Vec<String> = self.regras.iter().map(|r| para_busca(&r.corpo)).collect();
        let mut atual = None;
        for t in &self.termos {
            let c = letra(t);
            if atual != Some(c) {
                atual = Some(c);
                let _ = writeln!(s, "<a id=\"letra-{}\"></a>", c.to_ascii_lowercase());
                let _ = writeln!(s, "## {c}\n");
            }
            let _ = writeln!(s, "{}", t.marcador());
            let _ = writeln!(s, "<a id=\"{}\"></a>", t.id);
            let _ = writeln!(s, "### {}\n", escapar(&t.fm.termo));
            let mut meta = Vec::new();
            if !t.fm.sinonimos.is_empty() {
                meta.push(format!(
                    "*Também chamado de:* {}",
                    escapar(&t.fm.sinonimos.join(", "))
                ));
            }
            if let Some(a) = &t.fm.area {
                meta.push(format!("*Área:* {}", escapar(&self.area_nome(a))));
            }
            if !meta.is_empty() {
                let _ = writeln!(s, "{}\n", meta.join(" · "));
            }
            let texto = self.texto(
                &t.corpo,
                &OpcoesTexto {
                    pagina,
                    origem: &t.caminho,
                    proprio: "",
                    rebaixar: 0,
                    rotulos: true,
                    desembrulhar: &["Definição de negócio"],
                    notas: Notas::Tirar,
                    commit_padrao: None,
                },
            );
            let _ = writeln!(s, "{}\n", texto.trim_end());
            let commit = self.commit_da_analise(t.fm.origem.as_ref().map(|o| o.as_str()));
            let mut linhas = Vec::new();
            let ev = self.linha_evidencias(&evidencias_das_notas(&t.corpo), commit.as_deref());
            if !ev.is_empty() {
                linhas.push(ev.trim_end().to_owned());
            }
            // Onde é usado.
            let mut usado: Vec<String> = Vec::new();
            let chave = chave_termo(&t.id);
            let mut funcs: Vec<String> = self.funcs_de(&chave).to_vec();
            for fid in self.paginas_f.keys() {
                if !funcs.contains(fid)
                    && self.funcionalidade(fid).is_some_and(|f| {
                        menciona(&para_busca(&f.descricao), &t.fm.termo, &t.fm.sinonimos)
                    })
                {
                    funcs.push(fid.clone());
                }
            }
            for f in &funcs {
                usado.push(self.link_funcionalidade(pagina, f));
            }
            for (i, r) in self.regras.iter().enumerate() {
                if menciona(&busca_regras[i], &t.fm.termo, &t.fm.sinonimos) {
                    usado.push(self.link(pagina, &r.id, Some(&r.id)));
                }
            }
            if !usado.is_empty() {
                linhas.push(format!("**Usado em:** {}", usado.join(" · ")));
            }
            if let Some(o) = &t.fm.origem
                && let Some(c) = self.por_caminho.get(o.as_str())
            {
                linhas.push(format!("**Origem:** {}", self.link(pagina, c, None)));
            }
            if !linhas.is_empty() {
                let _ = writeln!(s);
            }
            for l in linhas {
                let _ = writeln!(s, "* {l}");
            }
            let _ = writeln!(s, "{FECHAMENTO}\n");
        }
        s
    }

    fn pagina_analise(&self, a: &Analise, pagina: &str) -> String {
        let mut s = String::new();
        let _ = writeln!(
            s,
            "{} › {}\n",
            self.inicio(pagina),
            md_link("Análises", &relativo(pagina, INDICE_ANALISES))
        );
        let _ = writeln!(s, "{}", a.marcador());
        let _ = writeln!(s, "<a id=\"{}\"></a>", a.id.to_lowercase());
        let _ = writeln!(s, "# {} · {}\n", a.id, escapar(&a.fm.titulo));
        let mut cab = vec![
            rotulo_tipo(&a.tipo).0.to_owned(),
            format!("Aprovada · criada em {}", a.fm.criado),
        ];
        if let Some(v) = &a.fm.base
            && let Some(c) = &v.commit
        {
            cab.push(format!("Código: {}", versao_md(v.branch.as_deref(), c)));
        }
        let _ = writeln!(s, "{}\n", cab.join(" · "));
        let funcs = self.funcs_de(&a.id);
        let mut meta = Vec::new();
        if !funcs.is_empty() {
            let l: Vec<String> = funcs
                .iter()
                .map(|f| self.link_funcionalidade(pagina, f))
                .collect();
            meta.push(format!("**Funcionalidades:** {}", l.join(" · ")));
        }
        let regras: Vec<String> = self
            .regras
            .iter()
            .filter(|r| {
                r.fm.origem
                    .as_ref()
                    .is_some_and(|o| o.as_str() == a.caminho)
            })
            .map(|r| self.link(pagina, &r.id, Some(&r.id)))
            .collect();
        if !regras.is_empty() {
            meta.push(format!("**Regras confirmadas:** {}", regras.join(" · ")));
        }
        let termos: Vec<String> = self
            .termos
            .iter()
            .filter(|t| {
                t.fm.origem
                    .as_ref()
                    .is_some_and(|o| o.as_str() == a.caminho)
            })
            .map(|t| self.link(pagina, &chave_termo(&t.id), None))
            .collect();
        if !termos.is_empty() {
            meta.push(format!("**Termos do glossário:** {}", termos.join(" · ")));
        }
        if let Some(o) = &a.fm.origem
            && self.alvos.contains_key(&o.to_string())
        {
            meta.push(format!(
                "**Derivada de:** {}",
                self.link(pagina, &o.to_string(), None)
            ));
        }
        for m in &meta {
            let _ = writeln!(s, "- {m}");
        }
        if !meta.is_empty() {
            let _ = writeln!(s);
        }
        let texto = self.texto(
            &a.corpo,
            &OpcoesTexto {
                pagina,
                origem: &a.caminho,
                proprio: &a.id,
                rebaixar: 0,
                rotulos: false,
                desembrulhar: &[],
                notas: Notas::Manter,
                commit_padrao: a.fm.base.as_ref().and_then(|b| b.commit.as_deref()),
            },
        );
        let _ = writeln!(s, "{}", texto.trim_end());
        let _ = writeln!(s, "{FECHAMENTO}\n");
        let sugestoes: Vec<&Sugestao> = self
            .sugestoes
            .iter()
            .filter(|x| self.alvos[&x.id].pagina == pagina)
            .collect();
        if !sugestoes.is_empty() {
            let _ = writeln!(s, "## Sugestões de alteração aceitas\n");
            for x in sugestoes {
                s.push_str(&self.bloco_sugestao(x, pagina));
            }
        }
        s
    }

    fn bloco_sugestao(&self, x: &Sugestao, pagina: &str) -> String {
        let mut s = String::new();
        let _ = writeln!(s, "{}", x.marcador());
        let _ = writeln!(s, "<a id=\"{}\"></a>", x.id.to_lowercase());
        let _ = writeln!(s, "### {} · {}\n", x.id, escapar(&x.fm.titulo));
        let texto = self.texto(
            &x.corpo,
            &OpcoesTexto {
                pagina,
                origem: &x.caminho,
                proprio: &x.id,
                rebaixar: 0,
                rotulos: true,
                desembrulhar: &[],
                notas: Notas::Tirar,
                commit_padrao: None,
            },
        );
        let _ = writeln!(s, "{}", texto.trim_end());
        let analise = x.fm.analise.as_ref().map(|a| a.to_string());
        let commit = analise.as_deref().and_then(|a| {
            self.analises
                .iter()
                .find(|x| x.id == a)
                .and_then(|x| x.fm.base.as_ref()?.commit.clone())
        });
        let ev = self.linha_evidencias(&evidencias_das_notas(&x.corpo), commit.as_deref());
        if !ev.is_empty() {
            let _ = writeln!(s, "\n* {}", ev.trim_end());
        }
        let _ = writeln!(s, "{FECHAMENTO}\n");
        s
    }

    fn indice_analises(&self) -> String {
        let pagina = INDICE_ANALISES;
        let mut s = String::new();
        let _ = writeln!(s, "{} › Análises\n", self.inicio(pagina));
        let _ = writeln!(s, "# Análises\n");
        let _ = writeln!(
            s,
            "{} pelos analistas, por tipo, das mais recentes para as mais antigas.\n",
            contagem(
                self.analises.len(),
                "análise aprovada",
                "análises aprovadas"
            )
        );
        let mut grupos: BTreeMap<(u8, &str), Vec<&Analise>> = BTreeMap::new();
        for a in &self.analises {
            let (_, grupo, ordem) = rotulo_tipo(&a.tipo);
            grupos.entry((ordem, grupo)).or_default().push(a);
        }
        for ((_, grupo), mut lista) in grupos {
            lista.sort_by(|a, b| {
                (b.fm.criado.to_string(), &b.id).cmp(&(a.fm.criado.to_string(), &a.id))
            });
            let _ = writeln!(s, "## {grupo}\n");
            for a in lista {
                let _ = write!(s, "- {} ({})", self.link(pagina, &a.id, None), a.fm.criado);
                let resumo = self.resumo(a);
                if !resumo.is_empty() {
                    let _ = write!(s, ": {}", escapar(&resumo));
                }
                let funcs = self.funcs_de(&a.id);
                if !funcs.is_empty() {
                    let l: Vec<String> = funcs
                        .iter()
                        .map(|f| {
                            if self.paginas_f.contains_key(f) {
                                self.link(pagina, f, Some(f))
                            } else {
                                f.clone()
                            }
                        })
                        .collect();
                    let _ = write!(s, " · {}", l.join(", "));
                }
                let _ = writeln!(s);
            }
            let _ = writeln!(s);
        }
        let soltas: Vec<&Sugestao> = self
            .sugestoes
            .iter()
            .filter(|x| self.alvos[&x.id].pagina == pagina)
            .collect();
        if !soltas.is_empty() {
            let _ = writeln!(s, "## Sugestões de alteração aceitas\n");
            for x in soltas {
                s.push_str(&self.bloco_sugestao(x, pagina));
            }
        }
        s
    }

    /// Uma linha sobre a análise: o primeiro parágrafo que não repete o título.
    fn resumo(&self, a: &Analise) -> String {
        let titulo = para_busca(&a.fm.titulo);
        paragrafos(sem_preambulo(&a.corpo))
            .into_iter()
            .map(|p| sem_selo(&p))
            .find(|p| !p.is_empty() && para_busca(p) != titulo)
            .map(|p| primeira_frase(&p))
            .unwrap_or_default()
    }

    /// Texto sem marcação do corpo (para definição curta).
    fn texto_puro(&self, corpo: &str) -> String {
        paragrafos(sem_preambulo(corpo))
            .into_iter()
            .map(|p| sem_selo(&p))
            .find(|p| !p.is_empty())
            .unwrap_or_default()
    }

    fn readme(&self, por_pagina_regras: &BTreeMap<String, Vec<usize>>) -> String {
        let pagina = README;
        let mut s = String::new();
        let _ = writeln!(
            s,
            "<!-- Gerado pelo ctower-business-agents a partir da base aprovada. Não edite à mão: \
             edite na base, pelo app, e publique de novo. -->"
        );
        let nome = if self.contexto.projeto.trim().is_empty() {
            "Documentação de negócio".to_owned()
        } else {
            format!("{} · documentação de negócio", self.contexto.projeto.trim())
        };
        let _ = writeln!(s, "# {}\n", escapar(&nome));
        let _ = writeln!(
            s,
            "O que o sistema faz, em linguagem de negócio: funcionalidades, regras de negócio, \
             termos e análises aprovados pelos analistas, com links para o trecho do código que \
             sustenta cada afirmação.\n"
        );
        let _ = writeln!(
            s,
            "> **Gerada pelo app.** Para corrigir ou completar, edite na base do \
             ctower-business-agents e publique de novo: o que for mudado direto aqui volta ao \
             que está na base na próxima publicação.\n"
        );
        let mut info = Vec::new();
        if let Some(v) = self.versao()
            && let Some(c) = &v.commit
        {
            info.push(format!(
                "**Código descrito:** {}",
                versao_md(v.branch.as_deref(), c)
            ));
        }
        if let Some(d) = &self.contexto.data {
            info.push(format!("**Atualizada em:** {d}"));
        }
        if !info.is_empty() {
            let _ = writeln!(s, "{}\n", info.join(" · "));
        }
        let sem_func: usize = por_pagina_regras
            .iter()
            .filter(|(p, _)| p.starts_with("regras/"))
            .map(|(_, v)| v.len())
            .sum();
        let _ = writeln!(
            s,
            "{} · {} · {} · {}\n",
            contagem(self.paginas_f.len(), "funcionalidade", "funcionalidades"),
            contagem(self.regras.len(), "regra de negócio", "regras de negócio"),
            contagem(self.termos.len(), "termo", "termos"),
            contagem(self.analises.len(), "análise", "análises"),
        );

        let _ = writeln!(s, "## Por onde começar\n");
        if !self.paginas_f.is_empty() {
            let _ = writeln!(
                s,
                "- [Funcionalidades por área](#funcionalidades-por-area): o que o sistema faz, com as regras de cada funcionalidade."
            );
        }
        if sem_func > 0 {
            let _ = writeln!(
                s,
                "- [Regras sem funcionalidade](#regras-sem-funcionalidade): {} ainda não {} a uma funcionalidade do mapa.",
                contagem(sem_func, "regra", "regras"),
                if sem_func == 1 { "ligada" } else { "ligadas" }
            );
        }
        if !self.termos.is_empty() {
            let _ = writeln!(
                s,
                "- {}: os termos do negócio e onde aparecem.",
                md_link("Glossário", GLOSSARIO)
            );
        }
        if !self.analises.is_empty() {
            let _ = writeln!(
                s,
                "- {}: as perguntas e análises que deram origem a esta documentação.",
                md_link("Análises", INDICE_ANALISES)
            );
        }
        let _ = writeln!(s);

        // Índice por área.
        let mut areas: BTreeMap<String, (Vec<&String>, Vec<&Negocial>)> = BTreeMap::new();
        for id in self.paginas_f.keys() {
            if let Some(f) = self.funcionalidade(id) {
                areas.entry(f.area.clone()).or_default().0.push(id);
            }
        }
        for n in &self.negociais {
            if self.alvos[&chave_doc(&n.id)].topo {
                areas.entry(n.fm.area.clone()).or_default().1.push(n);
            }
        }
        if !areas.is_empty() {
            let _ = writeln!(s, "<a id=\"funcionalidades-por-area\"></a>");
            let _ = writeln!(s, "## Funcionalidades por área\n");
            let mut ordem: Vec<(String, String)> = areas
                .keys()
                .map(|a| (self.area_nome(a), a.clone()))
                .collect();
            ordem.sort();
            for (nome, area) in ordem {
                let (funcs, docs) = &areas[&area];
                let _ = writeln!(s, "### {}\n", escapar(&nome));
                if !funcs.is_empty() {
                    let _ = writeln!(s, "| Funcionalidade | O que faz | Regras |");
                    let _ = writeln!(s, "| --- | --- | ---: |");
                    for id in funcs {
                        let f = self.funcionalidade(id).expect("aceita");
                        let n = self
                            .regras
                            .iter()
                            .filter(|r| self.funcs_de(&r.id).contains(id))
                            .count();
                        let _ = writeln!(
                            s,
                            "| {} | {} | {n} |",
                            celula(&self.link(pagina, id, None)),
                            celula(&escapar(&cortar(&f.descricao, 200)))
                        );
                    }
                    let _ = writeln!(s);
                }
                for n in docs {
                    let _ = writeln!(
                        s,
                        "- {} (documento sem funcionalidade aceita no mapa)",
                        self.link(pagina, &chave_doc(&n.id), None)
                    );
                }
                if !docs.is_empty() {
                    let _ = writeln!(s);
                }
                let regras_area = Self::pagina_da_area(&area);
                if let Some(v) = por_pagina_regras.get(&regras_area) {
                    let _ = writeln!(
                        s,
                        "Mais {} desta área sem funcionalidade: {}.\n",
                        contagem(v.len(), "regra", "regras"),
                        md_link(&format!("Regras · {nome}"), &regras_area)
                    );
                }
            }
        }
        if sem_func > 0 {
            let _ = writeln!(s, "<a id=\"regras-sem-funcionalidade\"></a>");
            let _ = writeln!(s, "## Regras sem funcionalidade\n");
            let mut lista: Vec<(String, &String, usize)> = por_pagina_regras
                .iter()
                .filter(|(p, _)| p.starts_with("regras/"))
                .map(|(p, v)| (self.area_nome(&self.regras[v[0]].fm.area), p, v.len()))
                .collect();
            lista.sort();
            for (nome, p, n) in lista {
                let _ = writeln!(
                    s,
                    "- {}: {}",
                    md_link(&nome, p),
                    contagem(n, "regra", "regras")
                );
            }
            let _ = writeln!(s);
        }
        if let Some(m) = self.mapa {
            let pendentes = m.pendentes();
            if pendentes > 0 {
                let _ = writeln!(
                    s,
                    "O mapa tem mais {} ainda sem a revisão do analista; {} aqui quando for{} aceita{}.\n",
                    contagem(pendentes, "funcionalidade", "funcionalidades"),
                    if pendentes == 1 {
                        "ela entra"
                    } else {
                        "elas entram"
                    },
                    if pendentes == 1 { "" } else { "em" },
                    if pendentes == 1 { "" } else { "s" },
                );
            }
        }
        s
    }
}

// ---------------------------------------------------------------------------
// Opções e utilitários de texto

enum Notas {
    /// Tira as notas do texto (a página tem outros documentos com notas).
    Tirar,
    /// Mantém, com o prefixo no rótulo para não colidir com outro documento.
    Prefixar(String),
    Manter,
}

struct OpcoesTexto<'a> {
    pagina: &'a str,
    origem: &'a str,
    proprio: &'a str,
    rebaixar: usize,
    /// Subtítulos viram rótulos em negrito (documento dentro de uma lista).
    rotulos: bool,
    /// Subtítulos tirados, mantendo o conteúdo.
    desembrulhar: &'a [&'a str],
    notas: Notas,
    commit_padrao: Option<&'a str>,
}

/// Corpo sem o preâmbulo do template: do primeiro subtítulo (`##`) em diante;
/// sem subtítulo, sem o título (`#`).
fn sem_preambulo(corpo: &str) -> &str {
    let mut fim_h1 = None;
    for (e, r) in Parser::new_ext(corpo, opcoes_markdown()).into_offset_iter() {
        if let Event::Start(Tag::Heading { level, .. }) = e {
            if level == HeadingLevel::H2 {
                return &corpo[r.start..];
            }
            if level == HeadingLevel::H1 && fim_h1.is_none() {
                fim_h1 = Some(r.end);
            }
        }
    }
    fim_h1.map_or(corpo, |f| &corpo[f..])
}

/// Ids de documento no texto: sequências de letras, dígitos e `-` que
/// parecem um id (`RN-COB-002`, `A-0001`, `F-COB-001`, `S-0001`).
fn ids_no_texto(texto: &str) -> Vec<(Range<usize>, &str)> {
    let mut saida = Vec::new();
    let bytes = texto.as_bytes();
    let parte = |b: u8| b.is_ascii_alphanumeric() || b == b'-';
    let mut i = 0;
    while i < bytes.len() {
        if parte(bytes[i]) && (i == 0 || !parte(bytes[i - 1]) && bytes[i - 1].is_ascii()) {
            let mut j = i;
            while j < bytes.len() && parte(bytes[j]) {
                j += 1;
            }
            let mut fim = j;
            while fim > i && bytes[fim - 1] == b'-' {
                fim -= 1;
            }
            let token = &texto[i..fim];
            let fora_de_palavra = j >= bytes.len() || bytes[j].is_ascii();
            if fora_de_palavra && parece_id(token) {
                saida.push((i..fim, token));
            }
            i = j;
        } else {
            i += 1;
        }
    }
    saida
}

fn parece_id(t: &str) -> bool {
    let partes: Vec<&str> = t.split('-').collect();
    let num = |s: &str| s.len() >= 3 && s.bytes().all(|b| b.is_ascii_digit());
    let sigla = |s: &str| {
        !s.is_empty()
            && s.bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
    };
    match partes.as_slice() {
        ["A" | "S", n] => num(n),
        ["RN" | "F", s, n] => sigla(s) && num(n),
        _ => false,
    }
}

/// Evidências das notas de rodapé (`[^1]: \`arq:1-2\` · hash … · commit \`c\``).
fn evidencias_das_notas(corpo: &str) -> Vec<Ev> {
    let mut saida = Vec::new();
    for linha in corpo.lines() {
        let Some(resto) = linha.strip_prefix("[^") else {
            continue;
        };
        let Some((_, depois)) = resto.split_once("]: `") else {
            continue;
        };
        let Some((codigo, cauda)) = depois.split_once('`') else {
            continue;
        };
        let Some(c) = citacao(codigo) else { continue };
        let commit = cauda
            .split_once("commit `")
            .and_then(|(_, r)| r.split_once('`'))
            .map(|(c, _)| c.to_owned())
            .filter(|c| !c.trim().is_empty());
        saida.push(Ev {
            arquivo: c.arquivo.as_str().to_owned(),
            linhas: (c.linhas.inicio, c.linhas.fim),
            commit,
        });
    }
    saida
}

/// Nota de evidência sem o hash; com permalink, sem o commit (está no link).
fn limpar_nota(linha: &str) -> String {
    if !linha.starts_with("[^") {
        return linha.to_owned();
    }
    let mut l = tirar_campo(linha, " · hash `");
    if l.contains("](<") {
        l = tirar_campo(&l, " · commit `");
    }
    l
}

fn tirar_campo(linha: &str, inicio: &str) -> String {
    let Some(i) = linha.find(inicio) else {
        return linha.to_owned();
    };
    let depois = &linha[i + inicio.len()..];
    match depois.find('`') {
        Some(j) => format!("{}{}", &linha[..i], &depois[j + 1..]),
        None => linha.to_owned(),
    }
}

/// Textos dos parágrafos e itens de lista, sem marcação. Fica de fora o que é
/// todo em itálico (o "_Nenhuma sugestão registrada._" dos templates).
fn paragrafos(texto: &str) -> Vec<String> {
    let mut saida = Vec::new();
    let mut atual = String::new();
    let mut fora_do_italico = false;
    let mut italico = 0usize;
    let mut titulo = false;
    let mut nota = false;
    let mut tabela = false;
    let fechar = |atual: &mut String, fora: &mut bool, saida: &mut Vec<String>| {
        let t = atual.split_whitespace().collect::<Vec<_>>().join(" ");
        if !t.is_empty() && *fora {
            saida.push(t);
        }
        atual.clear();
        *fora = false;
    };
    for e in Parser::new_ext(texto, opcoes_markdown()) {
        match e {
            Event::Start(Tag::Heading { .. }) => titulo = true,
            Event::End(TagEnd::Heading(_)) => titulo = false,
            Event::Start(Tag::FootnoteDefinition(_)) => nota = true,
            Event::End(TagEnd::FootnoteDefinition) => nota = false,
            Event::Start(Tag::Table(_)) => tabela = true,
            Event::End(TagEnd::Table) => tabela = false,
            Event::Start(Tag::Emphasis) => italico += 1,
            Event::End(TagEnd::Emphasis) => italico = italico.saturating_sub(1),
            Event::Start(Tag::Item) | Event::End(TagEnd::Paragraph | TagEnd::Item) => {
                fechar(&mut atual, &mut fora_do_italico, &mut saida)
            }
            Event::Text(t) | Event::Code(t) if !titulo && !nota && !tabela => {
                if italico == 0 && !t.trim().is_empty() {
                    fora_do_italico = true;
                }
                atual.push_str(&t);
            }
            Event::SoftBreak | Event::HardBreak => atual.push(' '),
            _ => {}
        }
    }
    saida
}

/// Sem o selo de confiança (`[confirmado]`) do começo.
fn sem_selo(p: &str) -> String {
    let p = p.trim();
    let p = match p.strip_prefix('[').and_then(|r| r.split_once(']')) {
        Some((selo, resto)) if !selo.contains(' ') => resto.trim(),
        _ => p,
    };
    p.to_owned()
}

fn primeira_frase(p: &str) -> String {
    let fim = p
        .char_indices()
        .find(|(i, c)| *c == '.' && p[i + 1..].starts_with(' '))
        .map_or(p.len(), |(i, _)| i + 1);
    cortar(&p[..fim], 200)
}

/// Corta em `max` caracteres, na última palavra inteira, com "…".
fn cortar(texto: &str, max: usize) -> String {
    let texto = texto.split_whitespace().collect::<Vec<_>>().join(" ");
    if texto.chars().count() <= max {
        return texto;
    }
    // Um caractere a mais: se ele é espaço, a última palavra cabe inteira.
    let mut corte: String = texto.chars().take(max + 1).collect();
    if let Some(i) = corte.rfind(' ') {
        corte.truncate(i);
    }
    let corte = corte.trim_end_matches([',', ';', ':', ' ', '.']);
    format!("{corte}…")
}

/// Texto para comparar menções: minúsculas, sem acento, só letras e dígitos
/// separados por um espaço, com espaço nas pontas.
fn para_busca(texto: &str) -> String {
    let mut s = String::from(" ");
    for palavra in cba_kb::slug(texto).split('-').filter(|p| !p.is_empty()) {
        s.push_str(palavra);
        s.push(' ');
    }
    s
}

/// O texto (já em [`para_busca`]) cita o termo ou um sinônimo.
fn menciona(busca: &str, termo: &str, sinonimos: &[String]) -> bool {
    std::iter::once(termo)
        .chain(sinonimos.iter().map(String::as_str))
        .map(para_busca)
        .filter(|t| t.trim().len() >= 4)
        .any(|t| busca.contains(&t))
}

fn rotulo_tipo(tipo: &str) -> (&'static str, &'static str, u8) {
    match tipo {
        "funcionalidade" => (
            "Explicação de funcionalidade",
            "Explicações de funcionalidade",
            1,
        ),
        "pergunta" => ("Pergunta ao sistema", "Perguntas ao sistema", 2),
        "analise-impacto" => ("Análise de impacto", "Análises de impacto", 3),
        "historias" => ("Histórias de usuário", "Histórias de usuário", 4),
        "cenarios-teste" => ("Cenários de teste", "Cenários de teste", 5),
        "parecer-revisao" => ("Parecer de revisão", "Pareceres de revisão", 6),
        "analise-reuniao" => ("Análise de reunião", "Análises de reunião", 7),
        _ => ("Análise", "Outras análises", 9),
    }
}

fn versao_md(branch: Option<&str>, commit: &str) -> String {
    let curto: String = commit.chars().take(12).collect();
    match branch.filter(|b| !b.trim().is_empty()) {
        Some(b) => format!("`{b}` @ `{curto}`"),
        None => format!("`{curto}`"),
    }
}

fn contagem(n: usize, um: &str, varios: &str) -> String {
    if n == 1 {
        format!("1 {um}")
    } else {
        format!("{n} {varios}")
    }
}

fn primeira_maiuscula(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(p) => p.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

/// Escapa o que o Markdown leria como marcação num texto curto (título, nome).
fn escapar(texto: &str) -> String {
    let mut s = String::with_capacity(texto.len());
    for c in texto.chars() {
        if matches!(c, '[' | ']' | '*' | '_' | '`' | '<' | '>' | '|' | '\\') {
            s.push('\\');
        }
        s.push(c);
    }
    s
}

fn celula(texto: &str) -> String {
    texto.replace(['\n', '\r'], " ")
}

/// `[texto](destino)` com o texto já em Markdown.
fn md_link_cru(texto: &str, destino: &str) -> String {
    if destino.contains([' ', '(', ')', '<', '>']) {
        format!("[{texto}](<{destino}>)")
    } else {
        format!("[{texto}]({destino})")
    }
}

/// `[texto](destino)` com o texto escapado.
fn md_link(texto: &str, destino: &str) -> String {
    md_link_cru(&escapar(texto), destino)
}

/// Caminho de `para` visto da pasta da página `de` (as duas relativas à pasta
/// da documentação).
fn relativo(de: &str, para: &str) -> String {
    match (CaminhoRelativo::novo(de), CaminhoRelativo::novo(para)) {
        (Ok(a), Ok(b)) => crate::caminho_relativo(&a, &b),
        _ => para.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_do_texto() {
        let t =
            "Ver RN-COB-002, A-0001/2 e F-COB-001. Não: XRN-COB-002, RN-cob-002, A-01, ÁA-0001.";
        let ids: Vec<&str> = ids_no_texto(t).into_iter().map(|(_, i)| i).collect();
        assert_eq!(ids, ["RN-COB-002", "A-0001", "F-COB-001"]);
    }

    #[test]
    fn preambulo_sai() {
        assert_eq!(sem_preambulo("# T\n\nmeta\n\n## A\n\nx\n"), "## A\n\nx\n");
        assert_eq!(sem_preambulo("# T\n\nx\n"), "\nx\n");
        assert_eq!(sem_preambulo("x\n"), "x\n");
    }

    #[test]
    fn nota_sem_hash() {
        assert_eq!(
            limpar_nota("[^1]: [`a.sql:1-2`](<https://x>) · hash `abc` · commit `c1`\n"),
            "[^1]: [`a.sql:1-2`](<https://x>)\n"
        );
        assert_eq!(
            limpar_nota("[^1]: `a.sql:1-2` · hash `abc` · commit `c1`\n"),
            "[^1]: `a.sql:1-2` · commit `c1`\n"
        );
    }

    #[test]
    fn mencoes_e_cortes() {
        let b = para_busca("A multa por atraso é cobrada.");
        assert!(menciona(&b, "Penalidade", &["Multa por atraso".into()]));
        assert!(!menciona(&b, "Juros", &[]));
        assert_eq!(cortar("um dois três", 7), "um dois…");
        assert_eq!(primeira_frase("Uma. Duas."), "Uma.");
        assert_eq!(sem_selo("[confirmado] Texto"), "Texto");
    }
}
