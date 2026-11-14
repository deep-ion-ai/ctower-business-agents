//! Comandos Tauri das demandas do projeto (épico #93, #138): a lista (abertas
//! primeiro), criar, encerrar e reabrir, e o painel da demanda.
//!
//! O painel junta o que está espalhado: a linha do tempo, em ordem cronológica, com
//! as atas das reuniões (rascunho ou aprovada), as análises ligadas pelo campo
//! `demanda.id` do frontmatter e as sessões do Claude ligadas a essas análises; e,
//! das atas **aprovadas**, as pendências, as dúvidas em aberto e os próximos passos,
//! cada um com o id do item e a reunião de origem. O rascunho ainda pode mudar, por
//! isso os itens dele não entram nessas seções.
//!
//! As análises são achadas pelo frontmatter dos documentos, e não pelo índice da
//! base, para o painel refletir na hora uma análise recém-criada.

use cba_core::analise::{Analise, Cliente, EstadoAnalise, EstadoSessao, Fluxo};
use cba_core::dados::DiretorioDados;
use cba_core::tempo;
use cba_docs::{EstadoAta, ItemAta, TipoItem};
use cba_kb::frontmatter::{Demanda, FrontmatterAnalise, StatusAta, StatusDemanda};
use cba_kb::{DemandaLida, IdDemanda, NovaDemanda};
use cba_session::passagem::ClientesDasSessoes;
use cba_session::sessoes::Sessoes;
use serde::Serialize;

use crate::analise::{AnaliseCriada, ambiente, criar_analise_com_demanda};

// ---------------------------------------------------------------------------
// Lista

/// Demanda na lista do projeto e no cabeçalho do painel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DemandaVista {
    /// `D-0001`.
    pub id: String,
    pub titulo: String,
    pub status: StatusDemanda,
    /// Chave no rastreador (`PROJ-123`).
    pub chave: Option<String>,
    /// Endereço da demanda no rastreador.
    pub origem: Option<String>,
    /// `AAAA-MM-DD`.
    pub criado: String,
    pub encerrado: Option<String>,
    /// `demanda.md`, relativo à raiz da base (para abrir na base).
    pub documento: String,
}

fn vista(kb: &cba_kb::Projeto, d: &DemandaLida) -> DemandaVista {
    let f = &d.documento.frontmatter;
    DemandaVista {
        id: d.id.to_string(),
        titulo: f.titulo.clone(),
        status: f.status,
        chave: f.chave.clone(),
        origem: f.origem.clone(),
        criado: f.criado.to_string(),
        encerrado: f.encerrado.map(|e| e.to_string()),
        documento: kb.na_base(&d.arquivo).as_str().to_owned(),
    }
}

fn id_da_demanda(demanda: &str) -> Result<IdDemanda, String> {
    IdDemanda::analisar(demanda.trim()).map_err(|e| e.to_string())
}

/// Demandas do projeto: as abertas primeiro e, em cada grupo, as mais novas primeiro.
pub fn listar_demandas(dados: &DiretorioDados, projeto: &str) -> Result<Vec<DemandaVista>, String> {
    let amb = ambiente(dados, projeto)?;
    let demandas = amb.kb.demandas().map_err(|e| e.to_string())?;
    Ok(demandas.iter().map(|d| vista(&amb.kb, d)).collect())
}

/// Cria uma demanda aberta com o próximo número livre.
pub fn criar_demanda(
    dados: &DiretorioDados,
    projeto: &str,
    nova: &NovaDemanda,
) -> Result<DemandaVista, String> {
    let amb = ambiente(dados, projeto)?;
    let d = amb
        .kb
        .criar_demanda(nova, tempo::hoje())
        .map_err(|e| e.to_string())?;
    Ok(vista(&amb.kb, &d))
}

/// Encerra (`encerrar = true`) ou reabre a demanda.
pub fn mudar_status_demanda(
    dados: &DiretorioDados,
    projeto: &str,
    demanda: &str,
    encerrar: bool,
) -> Result<DemandaVista, String> {
    let amb = ambiente(dados, projeto)?;
    let id = id_da_demanda(demanda)?;
    let d = if encerrar {
        amb.kb.encerrar_demanda(id, tempo::hoje())
    } else {
        amb.kb.reabrir_demanda(id)
    }
    .map_err(|e| e.to_string())?;
    Ok(vista(&amb.kb, &d))
}

// ---------------------------------------------------------------------------
// Painel

/// Aviso de transcript guardado sem a ata aprovada: até quando ele fica no
/// diretório de dados do app.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptGuardado {
    /// Data em que o transcript é apagado (`AAAA-MM-DD`).
    pub apaga_em: String,
    /// Faltam 7 dias ou menos.
    pub vence_em_breve: bool,
}

/// Uma reunião (ata) da demanda.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReuniaoVista {
    /// `R-0001`.
    pub id: String,
    pub data: String,
    pub status: StatusAta,
    pub objetivo: Option<String>,
    pub participantes: usize,
    pub itens: usize,
    /// A ata (`R-NNNN.md`), relativa à raiz da base; `None` se ainda não foi montada.
    pub documento: Option<String>,
    /// Transcript guardado sem a ata aprovada. Sempre `None` por enquanto: a guarda
    /// do transcript (#136, em `crates/cba-reunioes`) vai preenchê-lo.
    pub transcript_guardado: Option<TranscriptGuardado>,
}

/// Análise ligada à demanda.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnaliseDaDemanda {
    pub id: String,
    pub titulo: String,
    pub fluxo: Fluxo,
    pub estado: EstadoAnalise,
    pub criada_ms: u64,
}

/// Sessão do Claude ligada a uma análise da demanda.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessaoDaDemanda {
    pub id: String,
    /// Análise a que a sessão está ligada.
    pub analise: String,
    pub estado: EstadoSessao,
    pub cliente: Cliente,
    pub inicio_ms: u64,
}

/// Evento da linha do tempo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "tipo", rename_all = "camelCase")]
pub enum EventoDemanda {
    Reuniao(ReuniaoVista),
    Analise(AnaliseDaDemanda),
    Sessao(SessaoDaDemanda),
}

/// Item da linha do tempo, com a data e, quando há, o instante (a ata só tem a data
/// da reunião).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemLinhaDoTempo {
    /// `AAAA-MM-DD`.
    pub data: String,
    /// Milissegundos desde 1970 (UTC).
    pub instante_ms: Option<u64>,
    #[serde(flatten)]
    pub evento: EventoDemanda,
}

/// Pendência, dúvida ou próximo passo de uma ata aprovada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemDaAta {
    /// `P-1`, `DV-2`, `PP-1`.
    pub id: String,
    /// Reunião de origem (`R-0001`).
    pub reuniao: String,
    /// Data da reunião.
    pub data: String,
    /// Na pendência, a ação.
    pub texto: String,
    pub responsavel: Option<String>,
    pub prazo: Option<String>,
}

/// Documento da demanda que não pôde ser lido.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProblemaDemanda {
    pub caminho: String,
    pub erro: String,
}

/// O que o painel da demanda mostra.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PainelDemanda {
    pub demanda: DemandaVista,
    /// Em ordem cronológica (a mais antiga primeiro).
    pub linha_do_tempo: Vec<ItemLinhaDoTempo>,
    /// Das atas aprovadas, na ordem das reuniões e dos itens.
    pub pendencias: Vec<ItemDaAta>,
    pub duvidas: Vec<ItemDaAta>,
    pub proximos_passos: Vec<ItemDaAta>,
    pub problemas: Vec<ProblemaDemanda>,
}

fn item_da_ata(estado: &EstadoAta, item: &ItemAta) -> ItemDaAta {
    ItemDaAta {
        id: item.id.to_string(),
        reuniao: estado.id.to_string(),
        data: estado.data.to_string(),
        texto: item.texto.clone(),
        responsavel: item.responsavel.clone(),
        prazo: item.prazo.clone(),
    }
}

/// Análises do projeto ligadas à demanda pelo `demanda.id` do frontmatter.
fn analises_da_demanda(
    amb: &cba_core::ambiente::Ambiente,
    id: IdDemanda,
    problemas: &mut Vec<ProblemaDemanda>,
) -> Result<Vec<Analise>, String> {
    let mut saida = Vec::new();
    for a in amb.analises.listar().map_err(|e| e.to_string())? {
        match amb.kb.ler::<FrontmatterAnalise>(&a.documento) {
            Ok(doc) if doc.frontmatter.demanda.id == Some(id) => saida.push(a),
            Ok(_) => {}
            // Sem o frontmatter não dá para saber a demanda: o painel avisa.
            Err(e) => problemas.push(ProblemaDemanda {
                caminho: amb.kb.na_base(&a.documento).as_str().to_owned(),
                erro: e.to_string(),
            }),
        }
    }
    Ok(saida)
}

/// Monta o painel da demanda.
pub fn painel_demanda(
    dados: &DiretorioDados,
    projeto: &str,
    demanda: &str,
) -> Result<PainelDemanda, String> {
    let amb = ambiente(dados, projeto)?;
    let kb = &amb.kb;
    let id = id_da_demanda(demanda)?;
    let lida = kb.demanda(id).map_err(|e| e.to_string())?;
    let mut problemas = Vec::new();
    let mut linha = Vec::new();
    let mut pendencias = Vec::new();
    let mut duvidas = Vec::new();
    let mut proximos_passos = Vec::new();

    for arquivos in kb.reunioes_da_demanda(id).map_err(|e| e.to_string())? {
        let estado = match EstadoAta::ler(kb, arquivos.id) {
            Ok((estado, _)) => estado,
            Err(e) => {
                problemas.push(ProblemaDemanda {
                    caminho: kb.na_base(&arquivos.estado).as_str().to_owned(),
                    erro: e.to_string(),
                });
                continue;
            }
        };
        if estado.status == StatusAta::Aprovada {
            for item in &estado.itens {
                let destino = match item.tipo {
                    TipoItem::Pendencia => &mut pendencias,
                    TipoItem::Duvida => &mut duvidas,
                    TipoItem::ProximoPasso => &mut proximos_passos,
                    _ => continue,
                };
                destino.push(item_da_ata(&estado, item));
            }
        }
        linha.push(ItemLinhaDoTempo {
            data: estado.data.to_string(),
            instante_ms: None,
            evento: EventoDemanda::Reuniao(ReuniaoVista {
                id: estado.id.to_string(),
                data: estado.data.to_string(),
                status: estado.status,
                objetivo: estado.objetivo.clone(),
                participantes: estado.participantes.len(),
                itens: estado.itens.len(),
                documento: kb
                    .existe(&arquivos.ata)
                    .then(|| kb.na_base(&arquivos.ata).as_str().to_owned()),
                // A guarda do transcript (#136, desenvolvida em paralelo em
                // `crates/cba-reunioes`) vai preencher este aviso.
                transcript_guardado: None,
            }),
        });
    }

    let analises = analises_da_demanda(&amb, id, &mut problemas)?;
    let registros = Sessoes::do_diretorio(dados)
        .do_projeto(projeto)
        .unwrap_or_default();
    let clientes = ClientesDasSessoes::no_diretorio(dados.raiz());
    for a in &analises {
        linha.push(ItemLinhaDoTempo {
            data: tempo::data_utc(a.criada_ms).to_string(),
            instante_ms: Some(a.criada_ms),
            evento: EventoDemanda::Analise(AnaliseDaDemanda {
                id: a.id.to_string(),
                titulo: a.titulo.clone(),
                fluxo: a.fluxo,
                estado: a.estado,
                criada_ms: a.criada_ms,
            }),
        });
        // As sessões do registro dos hooks e, sem registro, as gravadas na análise.
        let mut vistas: Vec<SessaoDaDemanda> = registros
            .iter()
            .filter(|r| r.analise.as_ref() == Some(&a.id))
            .map(|r| SessaoDaDemanda {
                id: r.id.clone(),
                analise: a.id.to_string(),
                estado: r.estado,
                cliente: clientes.cliente_de(r),
                inicio_ms: r.inicio_ms,
            })
            .collect();
        for s in &a.sessoes {
            if !vistas.iter().any(|v| v.id == s.id) {
                vistas.push(SessaoDaDemanda {
                    id: s.id.clone(),
                    analise: a.id.to_string(),
                    estado: s.estado,
                    cliente: s.cliente,
                    inicio_ms: s.inicio_ms,
                });
            }
        }
        linha.extend(vistas.into_iter().map(|s| ItemLinhaDoTempo {
            data: tempo::data_utc(s.inicio_ms).to_string(),
            instante_ms: Some(s.inicio_ms),
            evento: EventoDemanda::Sessao(s),
        }));
    }

    // Pela data; no mesmo dia, a reunião (sem instante) antes, depois pelo instante.
    // A ordenação é estável: empates ficam na ordem em que foram juntados.
    linha.sort_by(|a, b| {
        (&a.data, a.instante_ms.unwrap_or(0)).cmp(&(&b.data, b.instante_ms.unwrap_or(0)))
    });

    Ok(PainelDemanda {
        demanda: vista(kb, &lida),
        linha_do_tempo: linha,
        pendencias,
        duvidas,
        proximos_passos,
        problemas,
    })
}

// ---------------------------------------------------------------------------
// Análise ligada à demanda

/// Cria a análise de um fluxo ligada à demanda (`demanda.id` no frontmatter, com a
/// chave e a origem da demanda). A demanda precisa estar aberta.
pub fn criar_analise_ligada(
    dados: &DiretorioDados,
    projeto: &str,
    demanda: &str,
    fluxo: Fluxo,
    entrada: &str,
    titulo: Option<&str>,
) -> Result<AnaliseCriada, String> {
    let id = id_da_demanda(demanda)?;
    let lida = ambiente(dados, projeto)?
        .kb
        .demanda(id)
        .map_err(|e| e.to_string())?;
    if !lida.aberta() {
        return Err(format!(
            "a demanda {id} está encerrada; reabra-a para ligar uma análise nova"
        ));
    }
    let f = &lida.documento.frontmatter;
    criar_analise_com_demanda(
        dados,
        projeto,
        fluxo,
        entrada,
        titulo,
        Some(Demanda {
            id: Some(id),
            origem: f.origem.clone().unwrap_or_else(|| "texto".into()),
            chave: f.chave.clone(),
        }),
    )
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
pub async fn listar_demandas_app(projeto: String) -> Result<Vec<DemandaVista>, String> {
    let dados = dados()?;
    bloqueante(move || listar_demandas(&dados, &projeto)).await
}

#[tauri::command]
pub async fn criar_demanda_app(
    projeto: String,
    titulo: String,
    descricao: Option<String>,
    chave: Option<String>,
) -> Result<DemandaVista, String> {
    let dados = dados()?;
    bloqueante(move || {
        criar_demanda(
            &dados,
            &projeto,
            &NovaDemanda {
                titulo,
                descricao,
                chave,
                origem: None,
            },
        )
    })
    .await
}

#[tauri::command]
pub async fn encerrar_demanda_app(
    projeto: String,
    demanda: String,
) -> Result<DemandaVista, String> {
    let dados = dados()?;
    bloqueante(move || mudar_status_demanda(&dados, &projeto, &demanda, true)).await
}

#[tauri::command]
pub async fn reabrir_demanda_app(projeto: String, demanda: String) -> Result<DemandaVista, String> {
    let dados = dados()?;
    bloqueante(move || mudar_status_demanda(&dados, &projeto, &demanda, false)).await
}

#[tauri::command]
pub async fn painel_demanda_app(projeto: String, demanda: String) -> Result<PainelDemanda, String> {
    let dados = dados()?;
    bloqueante(move || painel_demanda(&dados, &projeto, &demanda)).await
}

#[cfg(test)]
#[path = "demandas_testes.rs"]
mod tests;
