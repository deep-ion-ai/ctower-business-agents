//! Cobertura da base (#125): cruza o mapa estrutural do projeto (rotinas, tabelas
//! e quantas referências cada arquivo recebe), o mapa de funcionalidades (#208) e
//! o índice da base (regras, termos e documentos) para mostrar o que a base já
//! cobre e o que falta, por área e por pasta, e sugerir a próxima análise de
//! maior valor. É conta, sem IA: [`calcular`] só recebe os dados já lidos.
//!
//! - **Documentação** de uma área: regras (menos as obsoletas), termos e
//!   documentos negociais (`funcionalidade` e `caso-de-uso`) cuja área é ela, no
//!   índice. Os números são os do índice: a tela da base mostra os mesmos.
//! - **Idade**: dias desde a última modificação do arquivo no índice (o mais
//!   recente e o mais antigo da área); regra parada há mais de
//!   [`DIAS_SEM_CONFIRMACAO`] dias conta como "sem confirmação recente"; "a
//!   revalidar" vem do `status`.
//! - **Arquivos de uma área**: os citados nas evidências das regras dela e nos
//!   pontos de entrada das funcionalidades dela (a área não existe no código).
//! - **Centralidade** de um arquivo: referências que as rotinas, tabelas e
//!   classes definidas nele recebem de outros arquivos. A da área ou da pasta é
//!   a soma dos arquivos.
//! - **Lacuna**: área com código referenciado e nenhum documento, ou com
//!   funcionalidade sem documento negocial; pasta com código referenciado e
//!   nenhuma regra ou funcionalidade documentada. O valor da lacuna é
//!   `centralidade / (1 + documentação)`; o mapa de calor é esse valor relativo
//!   ao maior.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use cba_core::ambiente::Ambiente;
use cba_core::analise::Fluxo;
use cba_core::funcionalidades::MapaFuncionalidades;
use cba_index::{Mapa, TipoSimbolo};
use cba_kb::frontmatter::{Area, TipoDocumento};
use cba_kb::indice::{DocumentoIndexado, EvidenciaIndexada, Tipo};
use serde::Serialize;

/// Regra sem modificação há mais dias que isto conta como "sem confirmação recente".
pub const DIAS_SEM_CONFIRMACAO: u64 = 90;

const NS_POR_DIA: i64 = 86_400 * 1_000_000_000;

/// Tudo o que a conta precisa, já lido.
pub struct Entrada<'a> {
    pub mapa: &'a Mapa,
    pub documentos: &'a [DocumentoIndexado],
    pub evidencias: &'a [EvidenciaIndexada],
    pub funcionalidades: &'a MapaFuncionalidades,
    /// Áreas declaradas no `projeto.md`.
    pub areas: &'a [Area],
    /// Agora, em nanossegundos desde 1970 (UTC).
    pub agora_ns: i64,
}

/// Cobertura de uma área funcional.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoberturaArea {
    pub id: String,
    pub nome: String,
    pub regras: usize,
    pub a_revalidar: usize,
    /// Regras sem modificação há mais de [`DIAS_SEM_CONFIRMACAO`] dias.
    pub sem_confirmacao_recente: usize,
    pub termos: usize,
    /// Documentos negociais da área.
    pub documentos: usize,
    /// Funcionalidades abertas do mapa e quantas ainda não têm documento.
    pub funcionalidades: usize,
    pub funcionalidades_sem_documento: usize,
    pub arquivos: usize,
    pub centralidade: usize,
    /// Dias desde a modificação mais recente e a mais antiga dos documentos da área.
    pub idade_recente_dias: Option<u64>,
    pub idade_antiga_dias: Option<u64>,
    pub lacuna: bool,
    /// De 0 a 1: a lacuna da área relativa à maior (0 sem lacuna).
    pub calor: f64,
}

/// Cobertura de uma pasta do código.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoberturaPasta {
    /// Caminho relativo (`""` é a raiz do projeto).
    pub pasta: String,
    pub arquivos: usize,
    pub centralidade: usize,
    /// Regras com evidência em arquivos da pasta.
    pub regras: usize,
    /// Funcionalidades abertas com ponto de entrada na pasta e as que têm documento.
    pub funcionalidades: usize,
    pub documentadas: usize,
    pub lacuna: bool,
    pub calor: f64,
}

/// A próxima análise sugerida.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sugestao {
    /// `area` ou `pasta`.
    pub tipo: String,
    /// Id da área ou caminho da pasta.
    pub alvo: String,
    pub nome: String,
    pub fluxo: Fluxo,
    /// Funcionalidade a explicar (`F-COB-001`), quando o fluxo é `explicar`.
    pub funcionalidade: Option<String>,
    /// Texto para abrir a análise.
    pub entrada: String,
    pub motivo: String,
}

/// Totais da base, iguais aos do índice.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TotaisCobertura {
    pub regras: usize,
    pub termos: usize,
    pub documentos: usize,
    pub a_revalidar: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cobertura {
    /// Das maiores lacunas para as menores.
    pub areas: Vec<CoberturaArea>,
    pub pastas: Vec<CoberturaPasta>,
    pub sugestao: Option<Sugestao>,
    pub totais: TotaisCobertura,
}

fn dias(agora_ns: i64, ns: i64) -> u64 {
    u64::try_from((agora_ns - ns).max(0) / NS_POR_DIA).unwrap_or(0)
}

fn pasta_de(arquivo: &str) -> &str {
    arquivo.rsplit_once('/').map_or("", |(p, _)| p)
}

fn conta_como_codigo(t: TipoSimbolo) -> bool {
    !matches!(
        t,
        TipoSimbolo::Coluna
            | TipoSimbolo::Campo
            | TipoSimbolo::Propriedade
            | TipoSimbolo::Variavel
            | TipoSimbolo::Constante
            | TipoSimbolo::Construtor
    )
}

/// Referências que o arquivo recebe de outros arquivos, por arquivo.
fn centralidade_por_arquivo(mapa: &Mapa) -> HashMap<String, usize> {
    let mut saida: HashMap<String, usize> = HashMap::new();
    for achado in mapa.simbolos() {
        if !conta_como_codigo(achado.simbolo.tipo) {
            continue;
        }
        let de_fora = mapa
            .referencias_a(achado.id)
            .iter()
            .filter(|u| u.arquivo != achado.arquivo)
            .count();
        if de_fora > 0 {
            *saida.entry(achado.arquivo.to_owned()).or_default() += de_fora;
        }
    }
    saida
}

fn valor(centralidade: usize, documentacao: usize) -> f64 {
    centralidade as f64 / (1 + documentacao) as f64
}

#[derive(Default)]
struct AreaEmConta {
    nome: String,
    regras: usize,
    a_revalidar: usize,
    antigas: usize,
    termos: usize,
    documentos: usize,
    funcionalidades: usize,
    sem_documento: usize,
    arquivos: BTreeSet<String>,
    modificados: Vec<i64>,
}

#[derive(Default)]
struct PastaEmConta {
    arquivos: usize,
    centralidade: usize,
    regras: BTreeSet<String>,
    funcionalidades: usize,
    documentadas: usize,
}

/// A cobertura do projeto.
pub fn calcular(e: &Entrada<'_>) -> Cobertura {
    let por_arquivo = centralidade_por_arquivo(e.mapa);
    let central = |arquivo: &str| por_arquivo.get(arquivo).copied().unwrap_or(0);

    let mut areas: BTreeMap<String, AreaEmConta> = BTreeMap::new();
    for a in e.areas {
        areas.entry(a.id.clone()).or_default().nome = a.nome.clone();
    }
    // Documentos do índice: área, estado e idade.
    let mut area_da_regra: HashMap<&str, &str> = HashMap::new();
    let mut totais = TotaisCobertura::default();
    for d in e.documentos {
        let Some(Tipo::Documento(tipo)) = d.tipo else {
            continue;
        };
        let slug = d.area.clone().unwrap_or_default();
        match tipo {
            TipoDocumento::Regra => {
                if d.status.as_deref() == Some("obsoleta") {
                    continue;
                }
                totais.regras += 1;
                let a = areas.entry(slug.clone()).or_default();
                a.regras += 1;
                a.modificados.push(d.modificado_ns);
                if d.status.as_deref() == Some("a-revalidar") {
                    a.a_revalidar += 1;
                    totais.a_revalidar += 1;
                }
                if dias(e.agora_ns, d.modificado_ns) > DIAS_SEM_CONFIRMACAO {
                    a.antigas += 1;
                }
                area_da_regra.insert(d.caminho.as_str(), d.area.as_deref().unwrap_or_default());
            }
            TipoDocumento::Termo => {
                totais.termos += 1;
                let a = areas.entry(slug).or_default();
                a.termos += 1;
                a.modificados.push(d.modificado_ns);
            }
            TipoDocumento::Funcionalidade | TipoDocumento::CasoDeUso => {
                totais.documentos += 1;
                let a = areas.entry(slug).or_default();
                a.documentos += 1;
                a.modificados.push(d.modificado_ns);
            }
            _ => {}
        }
    }
    areas.remove("");
    // Arquivos das regras, pelas evidências.
    for ev in e.evidencias {
        if let Some(slug) = area_da_regra.get(ev.documento.as_str())
            && let Some(a) = areas.get_mut(*slug)
        {
            a.arquivos.insert(ev.arquivo.clone());
        }
    }
    // Funcionalidades do mapa: área e arquivos dos pontos de entrada.
    let mut central_da_funcionalidade: HashMap<String, usize> = HashMap::new();
    for f in e.funcionalidades.abertas() {
        let a = areas.entry(f.area.clone()).or_default();
        if a.nome.is_empty() {
            a.nome = f.area_nome.clone();
        }
        a.funcionalidades += 1;
        if f.documento_negocial().is_none() {
            a.sem_documento += 1;
        }
        let arquivos: BTreeSet<String> = f
            .pontos
            .iter()
            .map(|p| p.evidencia.arquivo.to_string())
            .collect();
        central_da_funcionalidade
            .insert(f.id.to_string(), arquivos.iter().map(|x| central(x)).sum());
        a.arquivos.extend(arquivos);
    }

    let mut linhas: Vec<(CoberturaArea, f64)> = areas
        .into_iter()
        .map(|(id, a)| {
            let centralidade: usize = a.arquivos.iter().map(|x| central(x)).sum();
            let doc = a.regras + a.termos + a.documentos;
            let lacuna = centralidade > 0 && (doc == 0 || a.sem_documento > 0);
            let v = if lacuna {
                valor(centralidade, doc)
            } else {
                0.0
            };
            let recente = a.modificados.iter().max().map(|m| dias(e.agora_ns, *m));
            let antiga = a.modificados.iter().min().map(|m| dias(e.agora_ns, *m));
            (
                CoberturaArea {
                    nome: if a.nome.is_empty() {
                        id.clone()
                    } else {
                        a.nome
                    },
                    id,
                    regras: a.regras,
                    a_revalidar: a.a_revalidar,
                    sem_confirmacao_recente: a.antigas,
                    termos: a.termos,
                    documentos: a.documentos,
                    funcionalidades: a.funcionalidades,
                    funcionalidades_sem_documento: a.sem_documento,
                    arquivos: a.arquivos.len(),
                    centralidade,
                    idade_recente_dias: recente,
                    idade_antiga_dias: antiga,
                    lacuna,
                    calor: 0.0,
                },
                v,
            )
        })
        .collect();
    let maior = linhas.iter().map(|(_, v)| *v).fold(0.0, f64::max);
    for (a, v) in &mut linhas {
        a.calor = if maior > 0.0 { *v / maior } else { 0.0 };
    }
    linhas.sort_by(|(a, va), (b, vb)| {
        vb.total_cmp(va)
            .then(b.centralidade.cmp(&a.centralidade))
            .then(a.nome.cmp(&b.nome))
    });
    let areas: Vec<CoberturaArea> = linhas.into_iter().map(|(a, _)| a).collect();

    // Pastas.
    let mut pastas: BTreeMap<String, PastaEmConta> = BTreeMap::new();
    for a in e.mapa.arquivos() {
        let p = pastas.entry(pasta_de(&a.caminho).to_owned()).or_default();
        p.arquivos += 1;
        p.centralidade += central(&a.caminho);
    }
    for ev in e.evidencias {
        if area_da_regra.contains_key(ev.documento.as_str())
            && let Some(p) = pastas.get_mut(pasta_de(&ev.arquivo))
        {
            p.regras.insert(ev.documento.to_string());
        }
    }
    for f in e.funcionalidades.abertas() {
        let pontos: BTreeSet<&str> = f
            .pontos
            .iter()
            .map(|p| pasta_de(p.evidencia.arquivo.as_str()))
            .collect();
        for pasta in pontos {
            if let Some(p) = pastas.get_mut(pasta) {
                p.funcionalidades += 1;
                if f.documento_negocial().is_some() {
                    p.documentadas += 1;
                }
            }
        }
    }
    let mut linhas: Vec<(CoberturaPasta, f64)> = pastas
        .into_iter()
        .map(|(pasta, p)| {
            let doc = p.regras.len() + p.documentadas;
            let lacuna = p.centralidade > 0 && doc == 0;
            let v = if lacuna {
                valor(p.centralidade, doc)
            } else {
                0.0
            };
            (
                CoberturaPasta {
                    pasta,
                    arquivos: p.arquivos,
                    centralidade: p.centralidade,
                    regras: p.regras.len(),
                    funcionalidades: p.funcionalidades,
                    documentadas: p.documentadas,
                    lacuna,
                    calor: 0.0,
                },
                v,
            )
        })
        .collect();
    let maior = linhas.iter().map(|(_, v)| *v).fold(0.0, f64::max);
    for (p, v) in &mut linhas {
        p.calor = if maior > 0.0 { *v / maior } else { 0.0 };
    }
    linhas.sort_by(|(a, va), (b, vb)| {
        vb.total_cmp(va)
            .then(b.centralidade.cmp(&a.centralidade))
            .then(a.pasta.cmp(&b.pasta))
    });
    let pastas: Vec<CoberturaPasta> = linhas.into_iter().map(|(p, _)| p).collect();

    let sugestao = sugerir(e, &areas, &pastas, &central_da_funcionalidade);
    Cobertura {
        areas,
        pastas,
        sugestao,
        totais,
    }
}

/// Lê o mapa estrutural, o mapa de funcionalidades e o índice do projeto e
/// calcula a cobertura. Sem IA: leitura e conta.
pub fn cobertura_do_projeto(ambiente: &Ambiente) -> Result<Cobertura, String> {
    let estrutural = crate::fontes::mapa(ambiente).map_err(|e| e.to_string())?;
    let funcionalidades = crate::mapear::mapa(ambiente).map_err(|e| e.to_string())?;
    let indice = crate::fontes::indice(ambiente).map_err(|e| e.to_string())?;
    let projeto = ambiente.kb.nome();
    let documentos = indice
        .documentos_do_projeto(projeto)
        .map_err(|e| e.to_string())?;
    let evidencias = indice
        .evidencias(Some(projeto))
        .map_err(|e| e.to_string())?;
    let areas = crate::mapear::areas_do_projeto(&ambiente.kb);
    let agora_ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_nanos()).unwrap_or(i64::MAX));
    Ok(calcular(&Entrada {
        mapa: &estrutural.mapa,
        documentos: &documentos,
        evidencias: &evidencias,
        funcionalidades: &funcionalidades,
        areas: &areas,
        agora_ns,
    }))
}

fn sugerir(
    e: &Entrada<'_>,
    areas: &[CoberturaArea],
    pastas: &[CoberturaPasta],
    central_da_funcionalidade: &HashMap<String, usize>,
) -> Option<Sugestao> {
    if let Some(a) = areas.first().filter(|a| a.lacuna) {
        let motivo = if a.regras + a.termos + a.documentos == 0 {
            format!(
                "A área recebe {} referências de outras partes do código e a base não tem nenhum documento dela.",
                a.centralidade
            )
        } else {
            format!(
                "A área recebe {} referências e tem {} de {} funcionalidades do mapa sem documento negocial.",
                a.centralidade, a.funcionalidades_sem_documento, a.funcionalidades
            )
        };
        let funcionalidade = e
            .funcionalidades
            .abertas()
            .filter(|f| f.area == a.id && f.documento_negocial().is_none())
            .max_by_key(|f| {
                (
                    central_da_funcionalidade
                        .get(&f.id.to_string())
                        .copied()
                        .unwrap_or(0),
                    std::cmp::Reverse(f.id.to_string()),
                )
            });
        return Some(match funcionalidade {
            Some(f) => Sugestao {
                tipo: "area".into(),
                alvo: a.id.clone(),
                nome: a.nome.clone(),
                fluxo: Fluxo::Explicar,
                funcionalidade: Some(f.id.to_string()),
                entrada: format!("Explicar a funcionalidade \"{}\" ({})", f.nome, f.id),
                motivo,
            },
            None => Sugestao {
                tipo: "area".into(),
                alvo: a.id.clone(),
                nome: a.nome.clone(),
                fluxo: Fluxo::Perguntar,
                funcionalidade: None,
                entrada: format!(
                    "Como funciona a área {} e quais regras de negócio ela tem?",
                    a.nome
                ),
                motivo,
            },
        });
    }
    let p = pastas.first().filter(|p| p.lacuna)?;
    let nome = if p.pasta.is_empty() {
        "(raiz)".to_owned()
    } else {
        p.pasta.clone()
    };
    Some(Sugestao {
        tipo: "pasta".into(),
        alvo: p.pasta.clone(),
        nome: nome.clone(),
        fluxo: Fluxo::Perguntar,
        funcionalidade: None,
        entrada: format!(
            "O que o código da pasta {nome} faz e quais regras de negócio ele implementa?"
        ),
        motivo: format!(
            "A pasta recebe {} referências de outros arquivos e nenhuma regra ou funcionalidade documentada a cobre.",
            p.centralidade
        ),
    })
}
