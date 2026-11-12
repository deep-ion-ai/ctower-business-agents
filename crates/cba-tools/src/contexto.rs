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

use std::collections::BTreeSet;
use std::fmt::Write as _;

use cba_core::ambiente::Ambiente;
use cba_core::analise::Analise;
use cba_kb::frontmatter::FrontmatterTermo;
use cba_kb::indice::{Consulta, Indice, Modo, Resultado, Tipo};
use cba_kb::{CaminhoRelativo, Id, TipoDocumento, layout};
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
}

/// Busca de um tipo de documento no índice, só do sistema da análise.
fn buscar(
    indice: &Indice,
    ambiente: &Ambiente,
    texto: &str,
    tipos: &[Tipo],
    limite: usize,
) -> Vec<Resultado> {
    let consulta = Consulta::nova(texto)
        .modo(Modo::Qualquer)
        .sistema(ambiente.kb.nome())
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

/// Caminho de um resultado, relativo à pasta do sistema na base.
fn no_sistema(ambiente: &Ambiente, r: &Resultado) -> Option<CaminhoRelativo> {
    let prefixo = format!("{}/", ambiente.kb.nome());
    CaminhoRelativo::novo(r.caminho.as_str().strip_prefix(&prefixo)?).ok()
}

impl Contexto {
    /// Monta o contexto da análise com o que há na base do sistema.
    pub fn da_analise(ambiente: &Ambiente, analise: &Analise) -> Self {
        let kb = &ambiente.kb;
        let mut contexto = Contexto::default();
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
            let sinonimos = no_sistema(ambiente, &r)
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
        v
    }

    /// O contexto em Markdown, para o roteiro. Caminhos relativos à base.
    pub fn markdown(&self) -> String {
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
