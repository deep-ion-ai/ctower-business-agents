//! Contexto da base para o roteiro de `iniciar_analise`: regras, termos do
//! glossário, memórias de área e análises anteriores relacionados à entrada
//! (spec, seções 6.1 e 8).
//!
//! A relação vem do índice da base (`cba_kb::indice`, o mesmo da ferramenta
//! `conhecimento`): busca com qualquer palavra do título e da entrada da análise,
//! os documentos com mais palavras (e mais raras) antes. As memórias são as das
//! áreas das regras e termos encontrados e as que a busca achar. Regras obsoletas
//! ficam de fora. A lista é curta de propósito: o agente lê o documento completo
//! na base, ou pede mais com `conhecimento`, quando precisar. Sem o índice, o
//! contexto vem vazio, com o motivo.
//!
//! Quando a análise é de uma demanda (`demanda.id` no frontmatter), o contexto
//! traz também a demanda ([`contexto_da_demanda`]): as atas anteriores (resumo e
//! decisões), as pendências delas, as pessoas do projeto com o papel e, no fluxo
//! `reuniao`, os falantes da reunião com o papel que o catálogo sugere.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use cba_core::ambiente::Ambiente;
use cba_core::analise::Analise;
use cba_kb::frontmatter::{FrontmatterAnalise, FrontmatterTermo, StatusAta};
use cba_kb::indice::{Consulta, Indice, Modo, Resultado, Tipo};
use cba_kb::{CaminhoRelativo, Id, IdDemanda, IdReuniao, TipoDocumento, layout};
use serde_json::{Value, json};

use crate::fontes;

/// Itens no máximo por tipo.
const LIMITE: usize = 10;

/// Contexto da base relacionado a uma análise.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Contexto {
    pub regras: Vec<Value>,
    pub termos: Vec<Value>,
    pub memorias: Vec<Value>,
    pub analises: Vec<Value>,
    /// Totais na base, para o agente saber o tamanho do que não veio.
    pub total_de_regras: usize,
    pub total_de_termos: usize,
    /// Por que o contexto não pôde ser montado (índice indisponível).
    pub aviso: Option<String>,
    /// A demanda da análise, com as atas anteriores, as pendências e as pessoas.
    pub demanda: Option<Value>,
}

/// Busca de um tipo de documento no índice, só do projeto da análise.
fn buscar(
    indice: &Indice,
    ambiente: &Ambiente,
    texto: &str,
    tipos: &[Tipo],
    limite: usize,
) -> Vec<Resultado> {
    let consulta = Consulta::nova(texto)
        .modo(Modo::Qualquer)
        .projeto(ambiente.kb.nome())
        .tipos(tipos.iter().copied())
        .pagina(limite, 0);
    indice
        .buscar(&consulta)
        .map(|p| p.resultados)
        .unwrap_or_else(|_| {
            tracing::warn!("busca no índice da base falhou");
            Vec::new()
        })
}

/// Caminho de um resultado, relativo à pasta do projeto na base.
fn no_projeto(ambiente: &Ambiente, r: &Resultado) -> Option<CaminhoRelativo> {
    let prefixo = format!("{}/", ambiente.kb.nome());
    CaminhoRelativo::novo(r.caminho.as_str().strip_prefix(&prefixo)?).ok()
}

impl Contexto {
    /// Monta o contexto da análise com o que há na base do projeto.
    pub fn da_analise(ambiente: &Ambiente, analise: &Analise) -> Self {
        let kb = &ambiente.kb;
        let mut contexto = Contexto {
            demanda: kb
                .ler::<FrontmatterAnalise>(&analise.documento)
                .ok()
                .and_then(|d| d.frontmatter.demanda.id)
                .map(|d| contexto_da_demanda(ambiente, d, analise.reuniao)),
            ..Contexto::default()
        };
        let documentos = kb.documentos().unwrap_or_default();
        let na_pasta = |pasta: &str| {
            documentos
                .iter()
                .filter(|d| d.partes().next() == Some(pasta))
                .count()
        };
        contexto.total_de_regras = na_pasta(layout::REGRAS);
        contexto.total_de_termos = na_pasta(layout::GLOSSARIO);

        let indice = match fontes::indice(ambiente) {
            Ok(i) => i,
            Err(e) => {
                contexto.aviso = Some(e.to_string());
                return contexto;
            }
        };
        let texto = format!("{} {}", analise.titulo, analise.entrada);
        let mut areas = BTreeSet::new();

        let regras = buscar(
            &indice,
            ambiente,
            &texto,
            &[Tipo::Documento(TipoDocumento::Regra)],
            LIMITE * 2,
        );
        for r in regras
            .iter()
            .filter(|r| r.status.as_deref() != Some("obsoleta"))
            .take(LIMITE)
        {
            areas.extend(r.area.clone());
            contexto.regras.push(json!({
                "id": r.identificador,
                "titulo": r.titulo,
                "area": r.area,
                "status": r.status,
                "arquivo": r.caminho,
            }));
        }

        for r in buscar(
            &indice,
            ambiente,
            &texto,
            &[Tipo::Documento(TipoDocumento::Termo)],
            LIMITE,
        ) {
            areas.extend(r.area.clone());
            let sinonimos = no_projeto(ambiente, &r)
                .and_then(|c| kb.ler::<FrontmatterTermo>(&c).ok())
                .map(|d| d.frontmatter.sinonimos)
                .unwrap_or_default();
            contexto.termos.push(json!({
                "termo": r.identificador.as_deref().unwrap_or(&r.titulo),
                "sinonimos": sinonimos,
                "arquivo": r.caminho,
            }));
        }

        // Memória das áreas: das regras e termos relacionados e as que a busca achar.
        let mut memorias = BTreeSet::new();
        for area in &areas {
            if let Ok(c) = layout::memoria(area)
                && kb.existe(&c)
            {
                memorias.insert((area.clone(), kb.na_base(&c)));
            }
        }
        for r in buscar(
            &indice,
            ambiente,
            &texto,
            &[Tipo::Documento(TipoDocumento::MemoriaArea)],
            LIMITE,
        ) {
            let area = r.area.clone().or_else(|| {
                let partes: Vec<&str> = r.caminho.partes().collect();
                partes.get(2).map(|a| (*a).to_owned())
            });
            if let Some(area) = area {
                memorias.insert((area, r.caminho.clone()));
            }
        }
        contexto.memorias = memorias
            .into_iter()
            .take(LIMITE)
            .map(|(area, arquivo)| json!({ "area": area, "arquivo": arquivo }))
            .collect();

        for r in buscar(&indice, ambiente, &texto, &Tipo::ANALISES, LIMITE + 1) {
            let Some(id) = r
                .identificador
                .as_deref()
                .and_then(|i| Id::analisar(i).ok())
            else {
                continue;
            };
            if id == analise.id || contexto.analises.len() >= LIMITE {
                continue;
            }
            let v = match ambiente.analises.carregar(&id) {
                Ok(a) => json!({
                    "id": a.id,
                    "titulo": a.titulo,
                    "fluxo": a.fluxo,
                    "estado": a.estado,
                    "documento": kb.na_base(&a.documento),
                }),
                // Documento sem estado (importado, ou o estado ficou ilegível): o que o
                // índice sabe.
                Err(_) => json!({
                    "id": id,
                    "titulo": r.titulo,
                    "fluxo": Value::Null,
                    "estado": r.status,
                    "documento": r.caminho,
                }),
            };
            contexto.analises.push(v);
        }
        contexto
    }

    pub fn json(&self) -> Value {
        let mut v = json!({
            "regras": self.regras,
            "termos": self.termos,
            "memorias": self.memorias,
            "analises": self.analises,
            "total_de_regras": self.total_de_regras,
            "total_de_termos": self.total_de_termos,
        });
        if let Some(aviso) = &self.aviso {
            v["aviso"] = json!(aviso);
        }
        if let Some(d) = &self.demanda {
            v["demanda"] = d.clone();
        }
        v
    }

    /// O contexto em Markdown, para o roteiro. Caminhos relativos à base.
    pub fn markdown(&self) -> String {
        let mut t = self
            .demanda
            .as_ref()
            .map(markdown_da_demanda)
            .unwrap_or_default();
        t.push_str(&self.markdown_da_base());
        t
    }

    fn markdown_da_base(&self) -> String {
        let mut t = String::new();
        let texto = |v: &Value, campo: &str| v[campo].as_str().unwrap_or_default().to_owned();
        if self.regras.is_empty()
            && self.termos.is_empty()
            && self.memorias.is_empty()
            && self.analises.is_empty()
        {
            if let Some(aviso) = &self.aviso {
                let _ = writeln!(t, "Contexto da base indisponível: {aviso}.");
            }
            let _ = writeln!(
                t,
                "Nada relacionado na base ({} regras e {} termos no total). Consulte \
                 `conhecimento` com outros termos e trabalhe a partir do código, \
                 começando por `referencias` e `estrutura`.",
                self.total_de_regras, self.total_de_termos
            );
            return t;
        }
        if !self.regras.is_empty() {
            let _ = writeln!(t, "Regras relacionadas ({} na base):", self.total_de_regras);
            for r in &self.regras {
                let _ = writeln!(
                    t,
                    "- {} ({}): {} — `{}`",
                    texto(r, "id"),
                    texto(r, "status"),
                    texto(r, "titulo"),
                    texto(r, "arquivo")
                );
            }
        }
        if !self.termos.is_empty() {
            let _ = writeln!(t, "Termos do glossário ({} na base):", self.total_de_termos);
            for r in &self.termos {
                let _ = writeln!(t, "- {} — `{}`", texto(r, "termo"), texto(r, "arquivo"));
            }
        }
        if !self.memorias.is_empty() {
            let _ = writeln!(t, "Memória das áreas:");
            for r in &self.memorias {
                let _ = writeln!(t, "- {} — `{}`", texto(r, "area"), texto(r, "arquivo"));
            }
        }
        if !self.analises.is_empty() {
            let _ = writeln!(t, "Análises anteriores sobre o assunto:");
            for r in &self.analises {
                let _ = writeln!(
                    t,
                    "- {} ({}): {} — `{}`",
                    texto(r, "id"),
                    texto(r, "estado"),
                    texto(r, "titulo"),
                    texto(r, "documento")
                );
            }
        }
        t
    }
}

/// Atas anteriores no máximo no contexto da demanda (as mais recentes).
const LIMITE_DE_ATAS: usize = 10;
/// Decisões no máximo por ata.
const LIMITE_DE_DECISOES: usize = 10;
/// Pendências no máximo, somando as atas.
const LIMITE_DE_PENDENCIAS: usize = 30;
/// Pessoas do catálogo no máximo.
const LIMITE_DE_PESSOAS: usize = 50;
/// Caracteres no máximo do resumo de uma ata.
const LIMITE_DO_RESUMO: usize = 400;
/// Caracteres no máximo do texto de um item.
const LIMITE_DO_ITEM: usize = 200;

fn status_da_ata(s: StatusAta) -> &'static str {
    match s {
        StatusAta::Rascunho => "rascunho",
        StatusAta::Aprovada => "aprovada",
    }
}

/// Contexto da demanda de uma análise: a demanda, as atas das outras reuniões
/// dela (resumo e decisões; as mais recentes), as pendências dessas atas, as
/// pessoas do projeto (nome e papel) e, se a análise é de uma reunião, os
/// falantes do transcript com o papel sugerido pelo catálogo. Uma ata ilegível
/// fica de fora, com um aviso. Nada aqui vem do transcript além dos nomes dos
/// falantes.
pub fn contexto_da_demanda(
    ambiente: &Ambiente,
    demanda: IdDemanda,
    reuniao: Option<IdReuniao>,
) -> Value {
    let kb = &ambiente.kb;
    let mut avisos: Vec<String> = Vec::new();
    let lida = match kb.demanda(demanda) {
        Ok(l) => l,
        Err(e) => {
            return json!({ "id": demanda, "aviso": format!("demanda ilegível ({e})") });
        }
    };
    let f = &lida.documento.frontmatter;

    let mut atas = Vec::new();
    let mut pendencias = Vec::new();
    let reunioes = kb.reunioes_da_demanda(demanda).unwrap_or_else(|e| {
        avisos.push(format!("reuniões da demanda ilegíveis ({e})"));
        Vec::new()
    });
    let anteriores: Vec<_> = reunioes
        .into_iter()
        .filter(|r| Some(r.id) != reuniao)
        .collect();
    let total_de_atas = anteriores.len();
    for r in anteriores.into_iter().rev().take(LIMITE_DE_ATAS) {
        let estado = match cba_docs::EstadoAta::ler(kb, r.id) {
            Ok((e, _)) => e,
            Err(e) => {
                avisos.push(format!("ata {} ilegível ({e})", r.id));
                continue;
            }
        };
        let decisoes: Vec<Value> = estado
            .itens_do_tipo(cba_docs::TipoItem::Decisao)
            .take(LIMITE_DE_DECISOES)
            .map(|i| json!({ "id": i.id, "texto": crate::reuniao::cortar(&i.texto, LIMITE_DO_ITEM).0 }))
            .collect();
        for p in estado.itens_do_tipo(cba_docs::TipoItem::Pendencia) {
            if pendencias.len() >= LIMITE_DE_PENDENCIAS {
                break;
            }
            pendencias.push(json!({
                "reuniao": r.id,
                "ata_status": status_da_ata(estado.status),
                "id": p.id,
                "texto": crate::reuniao::cortar(&p.texto, LIMITE_DO_ITEM).0,
                "responsavel": p.responsavel,
                "prazo": p.prazo,
            }));
        }
        atas.push(json!({
            "id": r.id,
            "data": estado.data.to_string(),
            "status": status_da_ata(estado.status),
            "resumo": estado.resumo.as_deref().map(|t| crate::reuniao::cortar(t, LIMITE_DO_RESUMO).0),
            "decisoes": decisoes,
            "ata": kb.na_base(&r.ata),
        }));
    }

    let catalogo = kb.catalogo_de_pessoas().unwrap_or_else(|e| {
        avisos.push(format!("catálogo de pessoas ilegível ({e})"));
        Default::default()
    });
    let pessoas: Vec<Value> = catalogo
        .pessoas()
        .iter()
        .take(LIMITE_DE_PESSOAS)
        .map(|p| {
            json!({
                "pessoa": p.slug,
                "nome": p.nome(),
                "papel": p.papel_sugerido().map(|s| s.papel),
            })
        })
        .collect();

    let mut v = json!({
        "id": demanda,
        "titulo": f.titulo,
        "chave": f.chave,
        "status": f.status,
        "demanda": kb.na_base(&lida.arquivo),
        "atas_anteriores": atas,
        "total_de_atas_anteriores": total_de_atas,
        "pendencias": pendencias,
        "pessoas": pessoas,
        "total_de_pessoas": catalogo.pessoas().len(),
    });
    if let Some(r) = reuniao {
        v["reuniao"] = json!(r);
        match cba_reunioes::Guarda::nova(&ambiente.dados).falas(kb.nome(), r) {
            Ok(falas) => {
                let mut nomes: Vec<String> = Vec::new();
                for n in falas.iter().filter_map(|f| f.falante.as_deref()) {
                    if !nomes.iter().any(|m| m == n) {
                        nomes.push(n.to_owned());
                    }
                }
                let participantes: Vec<Value> = catalogo
                    .sugerir(&nomes)
                    .into_iter()
                    .map(|p| {
                        json!({
                            "falante": p.falante,
                            "pessoa": p.casamento.pessoa().map(|x| x.slug.clone()),
                            "papel_sugerido": p.papel_sugerido.map(|s| s.papel),
                        })
                    })
                    .collect();
                v["participantes"] = json!(participantes);
            }
            Err(_) => avisos.push(format!(
                "o transcript da reunião {r} não está guardado; sem ele, não há falas para ler"
            )),
        }
    }
    if !avisos.is_empty() {
        v["avisos"] = json!(avisos);
    }
    v
}

/// O contexto da demanda em Markdown, para o roteiro.
fn markdown_da_demanda(d: &Value) -> String {
    let texto = |v: &Value, campo: &str| v[campo].as_str().unwrap_or_default().to_owned();
    let mut t = String::new();
    let chave = d["chave"]
        .as_str()
        .map(|c| format!(" [{c}]"))
        .unwrap_or_default();
    let _ = writeln!(
        t,
        "Demanda {}{chave}: {} ({}) — `{}`",
        texto(d, "id"),
        texto(d, "titulo"),
        texto(d, "status"),
        texto(d, "demanda")
    );
    if let Some(participantes) = d["participantes"].as_array().filter(|p| !p.is_empty()) {
        let _ = writeln!(
            t,
            "Participantes da reunião {} (papel sugerido pelo catálogo; confirme ou conteste com `propor_papel`):",
            texto(d, "reuniao")
        );
        for p in participantes {
            let papel = p["papel_sugerido"].as_str().unwrap_or("sem sugestão");
            let _ = writeln!(t, "- {}: {papel}", texto(p, "falante"));
        }
    }
    let atas = d["atas_anteriores"].as_array().cloned().unwrap_or_default();
    if atas.is_empty() {
        let _ = writeln!(t, "Nenhuma ata anterior nesta demanda.");
    } else {
        let _ = writeln!(t, "Atas anteriores da demanda:");
        for a in &atas {
            let _ = writeln!(
                t,
                "- {} ({}, {}) — `{}`",
                texto(a, "id"),
                texto(a, "data"),
                texto(a, "status"),
                texto(a, "ata")
            );
            if let Some(r) = a["resumo"].as_str() {
                let _ = writeln!(t, "  Resumo: {r}");
            }
            for dec in a["decisoes"].as_array().into_iter().flatten() {
                let _ = writeln!(t, "  - {}: {}", texto(dec, "id"), texto(dec, "texto"));
            }
        }
    }
    let pendencias = d["pendencias"].as_array().cloned().unwrap_or_default();
    if !pendencias.is_empty() {
        let _ = writeln!(t, "Pendências das atas anteriores:");
        for p in &pendencias {
            let resp = p["responsavel"].as_str().unwrap_or("a definir");
            let prazo = p["prazo"].as_str().unwrap_or("a definir");
            let _ = writeln!(
                t,
                "- {}/{}: {} · responsável: {resp} · prazo: {prazo}",
                texto(p, "reuniao"),
                texto(p, "id"),
                texto(p, "texto")
            );
        }
    }
    let pessoas = d["pessoas"].as_array().cloned().unwrap_or_default();
    if !pessoas.is_empty() {
        let lista: Vec<String> = pessoas
            .iter()
            .map(|p| match p["papel"].as_str() {
                Some(papel) => format!("{} ({papel})", texto(p, "nome")),
                None => texto(p, "nome"),
            })
            .collect();
        let _ = writeln!(
            t,
            "Pessoas do projeto ({} no catálogo): {}.",
            d["total_de_pessoas"].as_u64().unwrap_or(0),
            lista.join(", ")
        );
    }
    for a in d["avisos"].as_array().into_iter().flatten() {
        let _ = writeln!(t, "Aviso: {}", a.as_str().unwrap_or_default());
    }
    let _ = writeln!(t);
    t
}
