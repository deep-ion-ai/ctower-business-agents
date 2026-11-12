//! Comandos Tauri da tela do projeto (spec, seções 6.5, 7 e 10): áreas, regras (com
//! as "a revalidar" destacadas), glossário e mapa estrutural, mais a revalidação das
//! evidências ao abrir o projeto e sob demanda.
//!
//! As regras e os termos são lidos dos arquivos da base (`regras/`, `glossario/`), a
//! fonte da verdade, e não do índice: a lista do projeto tem de refletir na hora a
//! marca `a-revalidar` gravada pela revalidação. Documento ilegível não esconde os
//! outros; vai para `problemas`.
//!
//! A revalidação usa o último inventário gravado (`cba_tools::revalidacao`), sem
//! inventariar de novo: arquivo com tamanho ou data diferentes do inventário é lido,
//! então um inventário velho não engana a conferência, e inventariar o projeto todo a
//! cada abertura custaria mais do que a própria revalidação.

use std::collections::BTreeMap;

use cba_core::dados::DiretorioDados;
use cba_index::{Cadastro, Modulo};
use cba_kb::frontmatter::{FrontmatterRegra, FrontmatterTermo, StatusRegra};
use cba_kb::{CaminhoRelativo, Documento, layout};
use cba_tools::revalidacao::{self, Relatorio};
use serde::Serialize;

use crate::analise::ambiente;
use crate::primeiro_uso::Contagem;

// ---------------------------------------------------------------------------
// Visão do projeto

/// Regra de negócio na lista do projeto.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegraVista {
    pub id: String,
    pub titulo: String,
    pub area: String,
    /// `confirmada`, `a-revalidar` ou `obsoleta`.
    pub status: StatusRegra,
    /// Documento da regra, relativo à raiz da base.
    pub caminho: String,
    /// Documento que originou a regra (a análise), relativo à raiz da base.
    pub origem: Option<String>,
    pub evidencias: usize,
}

/// Termo do glossário.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TermoVista {
    pub termo: String,
    pub sinonimos: Vec<String>,
    pub area: Option<String>,
    /// Relativo à raiz da base.
    pub caminho: String,
    /// Relativo à raiz da base.
    pub origem: Option<String>,
}

/// Área funcional: as declaradas no `projeto.md` e as que só aparecem nas regras ou
/// nos termos.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AreaVista {
    pub id: String,
    pub nome: String,
    pub sigla: Option<String>,
    /// Declarada no `projeto.md`.
    pub declarada: bool,
    pub regras: usize,
    pub a_revalidar: usize,
    pub termos: usize,
    /// Memória da área (`areas/<area>/memoria.md`), relativa à raiz da base.
    pub memoria: Option<String>,
}

/// Documento que não pôde ser lido.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProblemaVista {
    pub caminho: String,
    pub erro: String,
}

/// O que a tela do projeto mostra, menos o mapa.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VisaoProjeto {
    pub id: String,
    pub nome: String,
    /// Documento `projeto.md`, relativo à raiz da base.
    pub documento: String,
    /// As "a revalidar" primeiro; depois por identificador.
    pub regras: Vec<RegraVista>,
    pub termos: Vec<TermoVista>,
    pub areas: Vec<AreaVista>,
    pub problemas: Vec<ProblemaVista>,
}

impl VisaoProjeto {
    pub fn a_revalidar(&self) -> usize {
        self.regras
            .iter()
            .filter(|r| r.status == StatusRegra::ARevalidar)
            .count()
    }
}

fn na_pasta(rel: &CaminhoRelativo, pasta: &str) -> bool {
    rel.partes().next() == Some(pasta) && rel.as_str().ends_with(".md")
}

/// Áreas, regras e glossário do projeto, lidos dos arquivos da base.
pub fn visao_projeto(dados: &DiretorioDados, projeto: &str) -> Result<VisaoProjeto, String> {
    let amb = ambiente(dados, projeto)?;
    let kb = &amb.kb;
    let documentos = kb.documentos().map_err(|e| e.to_string())?;
    let mut problemas = Vec::new();
    let mut regras = Vec::new();
    let mut termos = Vec::new();
    for rel in &documentos {
        let caminho = kb.na_base(rel).as_str().to_owned();
        if na_pasta(rel, layout::REGRAS) {
            match kb.ler::<FrontmatterRegra>(rel) {
                Ok(Documento { frontmatter: f, .. }) => regras.push(RegraVista {
                    id: f.id.to_string(),
                    titulo: f.titulo,
                    area: f.area,
                    status: f.status,
                    caminho,
                    origem: f.origem.map(|o| kb.na_base(&o).as_str().to_owned()),
                    evidencias: f.evidencias.len(),
                }),
                Err(e) => problemas.push(ProblemaVista {
                    caminho,
                    erro: e.to_string(),
                }),
            }
        } else if na_pasta(rel, layout::GLOSSARIO) {
            match kb.ler::<FrontmatterTermo>(rel) {
                Ok(Documento { frontmatter: f, .. }) => termos.push(TermoVista {
                    termo: f.termo,
                    sinonimos: f.sinonimos,
                    area: f.area,
                    caminho,
                    origem: f.origem.map(|o| kb.na_base(&o).as_str().to_owned()),
                }),
                Err(e) => problemas.push(ProblemaVista {
                    caminho,
                    erro: e.to_string(),
                }),
            }
        }
    }
    regras.sort_by(|a, b| {
        let peso = |r: &RegraVista| u8::from(r.status != StatusRegra::ARevalidar);
        (peso(a), &a.id).cmp(&(peso(b), &b.id))
    });
    termos.sort_by_key(|t| t.termo.to_lowercase());

    let (nome, declaradas) = match kb.ler_projeto() {
        Ok(d) => (d.frontmatter.nome, d.frontmatter.areas),
        Err(e) => {
            problemas.push(ProblemaVista {
                caminho: kb.na_base(&layout::projeto()).as_str().to_owned(),
                erro: e.to_string(),
            });
            (amb.projeto.nome.clone(), Vec::new())
        }
    };
    let mut areas: BTreeMap<String, AreaVista> = BTreeMap::new();
    let nova = |id: &str| AreaVista {
        id: id.to_owned(),
        nome: id.to_owned(),
        sigla: None,
        declarada: false,
        regras: 0,
        a_revalidar: 0,
        termos: 0,
        memoria: None,
    };
    for a in declaradas {
        areas.insert(
            a.id.clone(),
            AreaVista {
                nome: a.nome,
                sigla: a.sigla,
                declarada: true,
                ..nova(&a.id)
            },
        );
    }
    for r in &regras {
        let a = areas.entry(r.area.clone()).or_insert_with(|| nova(&r.area));
        a.regras += 1;
        if r.status == StatusRegra::ARevalidar {
            a.a_revalidar += 1;
        }
    }
    for t in &termos {
        if let Some(area) = &t.area {
            areas
                .entry(area.clone())
                .or_insert_with(|| nova(area))
                .termos += 1;
        }
    }
    for a in areas.values_mut() {
        if let Ok(rel) = layout::memoria(&a.id)
            && kb.existe(&rel)
        {
            a.memoria = Some(kb.na_base(&rel).as_str().to_owned());
        }
    }
    let mut areas: Vec<AreaVista> = areas.into_values().collect();
    // As declaradas na ordem do `projeto.md` não importam aqui: por nome.
    areas.sort_by_key(|a| a.nome.to_lowercase());

    Ok(VisaoProjeto {
        id: amb.projeto.id.clone(),
        nome,
        documento: kb.na_base(&layout::projeto()).as_str().to_owned(),
        regras,
        termos,
        areas,
        problemas,
    })
}

// ---------------------------------------------------------------------------
// Revalidação

/// Regra cujo status mudou na revalidação.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegraRevalidada {
    pub id: String,
    pub titulo: String,
    pub antes: StatusRegra,
    pub depois: StatusRegra,
}

/// Resumo da revalidação para a tela.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumoRevalidacao {
    pub evidencias: usize,
    /// Evidências que não conferem com o código atual.
    pub desatualizadas: usize,
    pub regras: Vec<RegraRevalidada>,
    /// Análises com afirmações marcadas ou restauradas.
    pub analises: Vec<String>,
    pub problemas: Vec<ProblemaVista>,
    pub mudou: bool,
    pub duracao_ms: u64,
}

impl From<&Relatorio> for ResumoRevalidacao {
    fn from(r: &Relatorio) -> Self {
        Self {
            evidencias: r.evidencias,
            desatualizadas: r.desatualizadas.len(),
            regras: r
                .regras
                .iter()
                .map(|m| RegraRevalidada {
                    id: m.id.clone(),
                    titulo: m.titulo.clone(),
                    antes: m.antes,
                    depois: m.depois,
                })
                .collect(),
            analises: r.analises.iter().map(|a| a.id.to_string()).collect(),
            problemas: r
                .problemas
                .iter()
                .map(|p| ProblemaVista {
                    caminho: p.documento.as_str().to_owned(),
                    erro: p.erro.clone(),
                })
                .collect(),
            mudou: r.mudou(),
            duracao_ms: r.duracao_ms,
        }
    }
}

/// Confere as evidências das regras e das análises do projeto contra o código atual
/// e marca (ou desmarca) o que está "a revalidar".
pub fn revalidar(dados: &DiretorioDados, projeto: &str) -> Result<ResumoRevalidacao, String> {
    let amb = ambiente(dados, projeto)?;
    let relatorio = revalidacao::revalidar_projeto(&amb, dados).map_err(|e| e.to_string())?;
    Ok(ResumoRevalidacao::from(&relatorio))
}

// ---------------------------------------------------------------------------
// Mapa

/// Uma pasta do mapa estrutural.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuloVista {
    /// `""` é a raiz do projeto.
    pub caminho: String,
    pub arquivos: usize,
    pub falhas: usize,
    pub linguagens: Vec<Contagem>,
    pub simbolos: Vec<Contagem>,
    pub subpastas: Vec<SubpastaVista>,
    pub arquivos_diretos: Vec<String>,
    /// As de mais referências primeiro.
    pub dependencias: Vec<DependenciaVista>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubpastaVista {
    pub caminho: String,
    pub arquivos: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DependenciaVista {
    pub de: String,
    pub para: String,
    pub referencias: usize,
}

/// Mapa do projeto numa pasta.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MapaVista {
    /// Segundos desde 1970 (UTC).
    pub gerado_em: u64,
    pub modulo: ModuloVista,
}

/// Dependências mostradas por pasta.
const DEPENDENCIAS: usize = 15;

/// Nome serializado de um valor (`snake_case` do enum), para a chave da contagem.
fn chave<T: Serialize>(valor: &T) -> String {
    serde_json::to_value(valor)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

fn contagens<K: Serialize>(
    mapa: &BTreeMap<K, usize>,
    rotulo: impl Fn(&K) -> String,
) -> Vec<Contagem> {
    let mut v: Vec<Contagem> = mapa
        .iter()
        .map(|(k, n)| Contagem {
            chave: chave(k),
            rotulo: rotulo(k),
            quantidade: *n,
        })
        .collect();
    v.sort_by_key(|a| std::cmp::Reverse(a.quantidade));
    v
}

fn modulo_vista(mapa: &cba_index::Mapa, m: Modulo) -> ModuloVista {
    let mut dependencias: Vec<DependenciaVista> = m
        .dependencias
        .into_iter()
        .map(|d| DependenciaVista {
            de: d.de,
            para: d.para,
            referencias: d.referencias,
        })
        .collect();
    dependencias.sort_by_key(|a| std::cmp::Reverse(a.referencias));
    dependencias.truncate(DEPENDENCIAS);
    ModuloVista {
        linguagens: contagens(&m.linguagens, |l| l.nome().to_owned()),
        simbolos: contagens(&m.simbolos, |t| chave(t).replace('_', " ")),
        subpastas: m
            .subpastas
            .iter()
            .map(|s| SubpastaVista {
                caminho: s.clone(),
                arquivos: mapa.modulo(s).map_or(0, |x| x.arquivos),
            })
            .collect(),
        caminho: m.caminho,
        arquivos: m.arquivos,
        falhas: m.falhas,
        arquivos_diretos: m.arquivos_diretos,
        dependencias,
    }
}

/// O mapa gravado do projeto, na pasta `modulo` (`""` é a raiz). `None` se o
/// projeto ainda não foi mapeado; pasta sem arquivo mapeado é recusada.
pub fn mapa_projeto(
    dados: &DiretorioDados,
    projeto: &str,
    modulo: &str,
) -> Result<Option<MapaVista>, String> {
    let Some(mapa) = Cadastro::em(dados.projetos())
        .mapa(projeto)
        .map_err(|e| e.to_string())?
    else {
        return Ok(None);
    };
    let modulo = modulo.trim().trim_matches('/');
    let m = mapa
        .modulo(modulo)
        .ok_or_else(|| format!("a pasta `{modulo}` não tem arquivo mapeado no projeto"))?;
    Ok(Some(MapaVista {
        gerado_em: mapa.gerado_em,
        modulo: modulo_vista(&mapa, m),
    }))
}

/// Inventaria e mapeia o projeto (sem IA, sem custo) e devolve a raiz do mapa.
pub fn mapear_projeto(dados: &DiretorioDados, projeto: &str) -> Result<MapaVista, String> {
    let mapa = Cadastro::em(dados.projetos())
        .mapear(projeto)
        .map_err(|e| e.to_string())?;
    let m = mapa.modulo("").unwrap_or_default();
    Ok(MapaVista {
        gerado_em: mapa.gerado_em,
        modulo: modulo_vista(&mapa, m),
    })
}

// ---------------------------------------------------------------------------
// Comandos

fn dados() -> Result<DiretorioDados, String> {
    DiretorioDados::padrao().map_err(|e| format!("diretório de dados do app inacessível: {e}"))
}

async fn bloqueante<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| format!("falha interna: {e}"))?
}

#[tauri::command]
pub async fn visao_projeto_app(projeto: String) -> Result<VisaoProjeto, String> {
    let dados = dados()?;
    bloqueante(move || visao_projeto(&dados, &projeto)).await
}

#[tauri::command]
pub async fn revalidar_projeto_app(projeto: String) -> Result<ResumoRevalidacao, String> {
    let dados = dados()?;
    bloqueante(move || {
        let r = revalidar(&dados, &projeto)?;
        tracing::info!(
            evidencias = r.evidencias,
            desatualizadas = r.desatualizadas,
            regras = r.regras.len(),
            duracao_ms = r.duracao_ms,
            "projeto revalidado"
        );
        Ok(r)
    })
    .await
}

#[tauri::command]
pub async fn mapa_projeto_app(
    projeto: String,
    modulo: Option<String>,
) -> Result<Option<MapaVista>, String> {
    let dados = dados()?;
    bloqueante(move || mapa_projeto(&dados, &projeto, modulo.as_deref().unwrap_or_default())).await
}

#[tauri::command]
pub async fn mapear_projeto_app(projeto: String) -> Result<MapaVista, String> {
    let dados = dados()?;
    bloqueante(move || mapear_projeto(&dados, &projeto)).await
}

#[cfg(test)]
#[path = "projeto_testes.rs"]
mod tests;
