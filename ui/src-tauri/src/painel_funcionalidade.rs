//! Painel da funcionalidade (#208): a página de uma funcionalidade do mapa como
//! ponto de partida do analista. Junta, sem IA, o que se liga a ela:
//!
//! - **Análises**: as abertas pelo painel (`Analise::funcionalidade`; no fluxo
//!   `explicar`, a que a explica), as das demandas ligadas e as que citam nas
//!   evidências um arquivo de um ponto de entrada dela ("pelo código").
//! - **Demandas**: as ligadas pelo analista (`Funcionalidade::demandas`) e as das
//!   análises abertas pelo painel, cada uma com as etapas (impacto, histórias,
//!   cenários) e as perguntas em aberto.
//! - **Regras**: as da base com evidência num arquivo dos pontos de entrada ou
//!   nascidas da explicação dela.
//! - **Perguntas em aberto** das análises ligadas, **pendências** e a
//!   **atividade recente**.
//!
//! E cria uma análise de qualquer fluxo a partir da funcionalidade, com a entrada
//! preenchida e a funcionalidade guardada na análise.

use std::collections::{BTreeMap, BTreeSet};

use cba_core::analise::{Analise, EstadoAnalise, EstadoPergunta, Fluxo};
use cba_core::dados::DiretorioDados;
use cba_core::eventos::Mudanca;
use cba_core::funcionalidades::{Funcionalidade, MapaFuncionalidades, SituacaoFuncionalidade};
use cba_kb::frontmatter::{FrontmatterAnalise, FrontmatterRegra, StatusRegra};
use cba_kb::informada::Destinatario;
use cba_kb::{CaminhoRelativo, IdDemanda, IdFuncionalidade, layout};
use cba_tools::mapear;
use serde::Serialize;

use crate::analise::{AnaliseCriada, AnaliseResumo, ambiente};
use crate::funcionalidades::{FuncionalidadeVista, vista_da};

/// Como a análise se liga à funcionalidade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Ligacao {
    /// A explicação da funcionalidade.
    Explica,
    /// Aberta pelo painel da funcionalidade.
    Funcionalidade,
    /// Análise de uma demanda ligada.
    Demanda,
    /// Cita nas evidências um arquivo de um ponto de entrada.
    Codigo,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnaliseNoPainel {
    #[serde(flatten)]
    pub resumo: AnaliseResumo,
    pub ligacao: Ligacao,
    pub demanda: Option<String>,
    pub perguntas_abertas: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PerguntaNoPainel {
    pub analise: String,
    pub numero: u32,
    pub texto: String,
    /// Pessoa (slug) ou papel; sem ele, sem destinatário.
    pub destinatario: Option<String>,
    pub demanda: Option<String>,
}

/// Etapa de uma demanda: a análise mais recente do fluxo, se houver.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EtapaVista {
    pub analise: String,
    pub estado: EstadoAnalise,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DemandaNoPainel {
    pub id: String,
    pub titulo: String,
    /// Pelo status efetivo: o conferido no rastreador, quando há (#292).
    pub aberta: bool,
    /// Estado conferido no rastreador e quando, se a demanda veio de uma issue.
    pub rastreador: Option<crate::demandas::RastreadorVista>,
    /// Rastreador inacessível ou issue não encontrada na última conferência.
    pub aviso_rastreador: Option<String>,
    pub criado: String,
    /// Chave no rastreador (`#41`, `PROJ-9`), quando a demanda veio de uma issue.
    pub chave: Option<String>,
    /// Endereço da issue no rastreador.
    pub origem: Option<String>,
    /// Ligada pelo analista no painel (pode ser desligada).
    pub ligada_no_painel: bool,
    pub impacto: Option<EtapaVista>,
    pub historias: Option<EtapaVista>,
    pub cenarios: Option<EtapaVista>,
    pub perguntas: Vec<PerguntaNoPainel>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegraNoPainel {
    pub id: String,
    pub titulo: String,
    /// O enunciado completo (a seção "Enunciado" do documento, sem as notas);
    /// sem ela, o título.
    pub enunciado: String,
    pub status: StatusRegra,
    /// Documento da regra, relativo à raiz da base.
    pub caminho: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Relacionada {
    pub id: String,
    pub nome: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Atividade {
    pub quando_ms: u64,
    pub texto: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DemandaResumo {
    pub id: String,
    pub titulo: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PainelVista {
    pub funcionalidade: FuncionalidadeVista,
    pub analises: Vec<AnaliseNoPainel>,
    pub demandas: Vec<DemandaNoPainel>,
    pub regras: Vec<RegraNoPainel>,
    pub perguntas: Vec<PerguntaNoPainel>,
    /// Análises concluídas pelo agente, à espera da revisão do analista.
    pub a_revisar: Vec<String>,
    pub relacionadas: Vec<Relacionada>,
    pub atividade: Vec<Atividade>,
    /// Demandas abertas do projeto (para ligar uma ou analisar o impacto).
    pub demandas_do_projeto: Vec<DemandaResumo>,
    /// Manual do usuário da funcionalidade (#272): nenhum, em andamento ou
    /// aprovado (com os motivos para revisar, se a funcionalidade mudou).
    pub manual: cba_tools::manual::SituacaoManual,
}

fn nome_do_fluxo(f: Fluxo) -> &'static str {
    match f {
        Fluxo::Perguntar => "pergunta",
        Fluxo::Explicar => "explicação",
        Fluxo::Impacto => "análise de impacto",
        Fluxo::Reuniao => "análise de reunião",
        Fluxo::Revisar => "revisão",
        Fluxo::Historias => "histórias",
        Fluxo::Cenarios => "cenários de teste",
        Fluxo::Manual => "manual do usuário",
        Fluxo::Prototipo => "protótipo de telas",
    }
}

fn nome_do_estado(e: EstadoAnalise) -> &'static str {
    match e {
        EstadoAnalise::Criada => "criada",
        EstadoAnalise::EmAndamento => "em andamento",
        EstadoAnalise::Concluida => "concluída",
        EstadoAnalise::Aprovada => "aprovada",
    }
}

fn destinatario(d: &Destinatario) -> String {
    match d {
        Destinatario::Pessoa { pessoa } => pessoa.clone(),
        Destinatario::Papel { papel } => papel.clone(),
    }
}

/// Texto da seção "Enunciado" do documento da regra, sem as chamadas e as
/// definições das notas de rodapé.
fn enunciado(corpo: &str) -> Option<String> {
    let mut dentro = false;
    let mut linhas: Vec<&str> = Vec::new();
    for l in corpo.lines() {
        let t = l.trim_start();
        if let Some(titulo) = t.strip_prefix("## ") {
            if dentro {
                break;
            }
            dentro = titulo.trim().eq_ignore_ascii_case("enunciado");
            continue;
        }
        if dentro && !t.starts_with("[^") {
            linhas.push(l);
        }
    }
    let mut texto = linhas.join("\n");
    // Chamadas de nota (`[^1]`, `[^ev-2]`).
    while let Some(i) = texto.find("[^") {
        match texto[i..].find(']') {
            Some(j) => texto.replace_range(i..i + j + 1, ""),
            None => break,
        }
    }
    let texto = texto.trim().to_owned();
    (!texto.is_empty()).then_some(texto)
}

/// A análise cita nas evidências (afirmações e perguntas) um dos arquivos.
fn toca(a: &Analise, arquivos: &BTreeSet<&str>) -> bool {
    a.afirmacoes
        .iter()
        .flat_map(|af| af.evidencias.iter())
        .chain(a.perguntas.iter().flat_map(|p| p.evidencias.iter()))
        .any(|e| arquivos.contains(e.arquivo.as_str()))
}

fn perguntas_abertas(a: &Analise, demanda: Option<IdDemanda>) -> Vec<PerguntaNoPainel> {
    a.perguntas
        .iter()
        .filter(|p| matches!(p.estado, EstadoPergunta::Aberta))
        .map(|p| PerguntaNoPainel {
            analise: a.id.to_string(),
            numero: p.numero,
            texto: p.texto.clone(),
            destinatario: p.destinatario.as_ref().map(destinatario),
            demanda: demanda.map(|d| d.to_string()),
        })
        .collect()
}

/// A etapa mais recente do fluxo entre as análises.
fn etapa(
    analises: &[(&Analise, Option<IdDemanda>)],
    d: IdDemanda,
    fluxo: Fluxo,
) -> Option<EtapaVista> {
    analises
        .iter()
        .filter(|(a, dem)| a.fluxo == fluxo && *dem == Some(d))
        .max_by_key(|(a, _)| a.id.numero())
        .map(|(a, _)| EtapaVista {
            analise: a.id.to_string(),
            estado: a.estado,
        })
}

pub fn painel(
    dados: &DiretorioDados,
    projeto: &str,
    funcionalidade: &str,
) -> Result<PainelVista, String> {
    let amb = ambiente(dados, projeto)?;
    let fid = IdFuncionalidade::analisar(funcionalidade.trim())
        .map_err(|_| format!("\"{funcionalidade}\" não é uma funcionalidade (F-<SIGLA>-NNN)"))?;
    let mapa = MapaFuncionalidades::carregar(&amb.kb).map_err(|e| e.to_string())?;
    let f: &Funcionalidade = mapa
        .funcionalidade(&fid)
        .ok_or_else(|| format!("a funcionalidade {fid} não está no mapa"))?;
    let arquivos: BTreeSet<&str> = f
        .pontos
        .iter()
        .map(|p| p.evidencia.arquivo.as_str())
        .collect();

    // Todas as análises, com a demanda do frontmatter.
    let todas: Vec<Analise> = amb.analises.listar().map_err(|e| e.to_string())?;
    let com_demanda: Vec<(&Analise, Option<IdDemanda>)> = todas
        .iter()
        .map(|a| {
            let d = amb
                .kb
                .ler::<FrontmatterAnalise>(&a.documento)
                .ok()
                .and_then(|doc| doc.frontmatter.demanda.id);
            (a, d)
        })
        .collect();

    // Demandas: as ligadas no painel e as das análises abertas por ela.
    let mut ids_demandas: Vec<IdDemanda> = f.demandas.clone();
    for (a, d) in &com_demanda {
        if a.funcionalidade.as_ref() == Some(&fid)
            && let Some(d) = d
            && !ids_demandas.contains(d)
        {
            ids_demandas.push(*d);
        }
    }

    let mut analises: Vec<AnaliseNoPainel> = Vec::new();
    let mut perguntas: Vec<PerguntaNoPainel> = Vec::new();
    let mut ligadas: Vec<&Analise> = Vec::new();
    for (a, d) in &com_demanda {
        let ligacao = if a.funcionalidade.as_ref() == Some(&fid) {
            if a.fluxo == Fluxo::Explicar {
                Ligacao::Explica
            } else {
                Ligacao::Funcionalidade
            }
        } else if d.is_some_and(|d| ids_demandas.contains(&d)) {
            Ligacao::Demanda
        } else if toca(a, &arquivos) {
            Ligacao::Codigo
        } else {
            continue;
        };
        let abertas = perguntas_abertas(a, *d);
        analises.push(AnaliseNoPainel {
            resumo: AnaliseResumo::from(*a),
            ligacao,
            demanda: d.map(|d| d.to_string()),
            perguntas_abertas: abertas.len(),
        });
        perguntas.extend(abertas);
        ligadas.push(a);
    }
    analises.sort_by_key(|a| std::cmp::Reverse(a.resumo.atualizada_ms));

    let lidas = amb.kb.demandas().unwrap_or_default();
    let demandas: Vec<DemandaNoPainel> = ids_demandas
        .iter()
        .map(|d| {
            let lida = lidas.iter().find(|l| l.id == *d);
            let vista = lida.map(|l| crate::demandas::vista(&amb.kb, l));
            DemandaNoPainel {
                rastreador: vista.as_ref().and_then(|v| v.rastreador.clone()),
                aviso_rastreador: vista.and_then(|v| v.aviso_rastreador),
                id: d.to_string(),
                titulo: lida
                    .map(|l| l.documento.frontmatter.titulo.clone())
                    .unwrap_or_else(|| "(demanda não encontrada na base)".into()),
                aberta: lida.is_some_and(|l| l.aberta()),
                criado: lida
                    .map(|l| l.documento.frontmatter.criado.to_string())
                    .unwrap_or_default(),
                chave: lida.and_then(|l| l.documento.frontmatter.chave.clone()),
                origem: lida.and_then(|l| l.documento.frontmatter.origem.clone()),
                ligada_no_painel: f.demandas.contains(d),
                impacto: etapa(&com_demanda, *d, Fluxo::Impacto),
                historias: etapa(&com_demanda, *d, Fluxo::Historias),
                cenarios: etapa(&com_demanda, *d, Fluxo::Cenarios),
                perguntas: perguntas
                    .iter()
                    .filter(|p| p.demanda.as_deref() == Some(&d.to_string()))
                    .cloned()
                    .collect(),
            }
        })
        .collect();

    // Regras: evidência num arquivo dos pontos ou nascidas da explicação.
    let explicacao_doc: Option<CaminhoRelativo> = todas
        .iter()
        .filter(|a| a.fluxo == Fluxo::Explicar && a.funcionalidade.as_ref() == Some(&fid))
        .map(|a| a.documento.clone())
        .next_back();
    let mut regras: Vec<RegraNoPainel> = Vec::new();
    for rel in amb.kb.documentos().unwrap_or_default() {
        if rel.partes().next() != Some(layout::REGRAS) || !rel.as_str().ends_with(".md") {
            continue;
        }
        let Ok(doc) = amb.kb.ler::<FrontmatterRegra>(&rel) else {
            continue;
        };
        let enunciado = enunciado(&doc.corpo);
        let r = doc.frontmatter;
        let pelo_codigo = r
            .evidencias
            .iter()
            .any(|e| arquivos.contains(e.arquivo.as_str()));
        let da_explicacao = explicacao_doc.is_some() && r.origem == explicacao_doc;
        if pelo_codigo || da_explicacao {
            regras.push(RegraNoPainel {
                id: r.id.to_string(),
                enunciado: enunciado.unwrap_or_else(|| r.titulo.clone()),
                titulo: r.titulo,
                status: r.status,
                caminho: amb.kb.na_base(&rel).to_string(),
            });
        }
    }
    regras.sort_by(|a, b| {
        let peso = |r: &RegraNoPainel| u8::from(r.status != StatusRegra::ARevalidar);
        (peso(a), &a.id).cmp(&(peso(b), &b.id))
    });

    let a_revisar = ligadas
        .iter()
        .filter(|a| a.estado == EstadoAnalise::Concluida)
        .map(|a| a.id.to_string())
        .collect();

    let relacionadas = f
        .relacionadas
        .iter()
        .map(|r| Relacionada {
            id: r.to_string(),
            nome: mapa
                .funcionalidade(r)
                .map(|x| x.nome.clone())
                .unwrap_or_default(),
        })
        .collect();

    let mut atividade: Vec<Atividade> = vec![Atividade {
        quando_ms: f.registrada_ms,
        texto: "Mapeada".into(),
    }];
    if f.atualizada_ms > f.registrada_ms {
        atividade.push(Atividade {
            quando_ms: f.atualizada_ms,
            texto: match f.situacao {
                SituacaoFuncionalidade::Aceita => "Mapeamento aceito".into(),
                SituacaoFuncionalidade::Pendente => "Mapeamento alterado, a revisar".into(),
                SituacaoFuncionalidade::Descartada => "Descartada".into(),
                SituacaoFuncionalidade::Juntada => "Juntada a outra".into(),
            },
        });
    }
    for a in &ligadas {
        atividade.push(Atividade {
            quando_ms: a.criada_ms,
            texto: format!("{} criada ({})", a.id, nome_do_fluxo(a.fluxo)),
        });
        if matches!(a.estado, EstadoAnalise::Concluida | EstadoAnalise::Aprovada)
            && a.atualizada_ms > a.criada_ms
        {
            atividade.push(Atividade {
                quando_ms: a.atualizada_ms,
                texto: format!("{} {}", a.id, nome_do_estado(a.estado)),
            });
        }
    }
    atividade.sort_by_key(|a| std::cmp::Reverse(a.quando_ms));
    atividade.truncate(8);

    let demandas_do_projeto = lidas
        .iter()
        .filter(|l| l.aberta())
        .map(|l| DemandaResumo {
            id: l.id.to_string(),
            titulo: l.documento.frontmatter.titulo.clone(),
        })
        .collect();

    let explicacoes: BTreeMap<IdFuncionalidade, Analise> = mapear::explicacoes(&amb.analises);
    Ok(PainelVista {
        funcionalidade: vista_da(f, &amb.kb, None, &explicacoes),
        analises,
        demandas,
        regras,
        perguntas,
        a_revisar,
        relacionadas,
        atividade,
        demandas_do_projeto,
        manual: cba_tools::manual::situacao(&amb, f),
    })
}

/// Entrada de uma análise aberta pelo painel: o pedido do analista e, abaixo, a
/// funcionalidade (nome, descrição e pontos de entrada).
fn entrada(texto: Option<&str>, f: &Funcionalidade, cabeca: &str) -> String {
    let contexto = mapear::entrada_da_explicacao(f).replacen(
        &format!("Explicar a funcionalidade {}", f.id),
        &format!("Funcionalidade {}", f.id),
        1,
    );
    match texto.map(str::trim).filter(|t| !t.is_empty()) {
        Some(t) => format!("{t}\n\n{contexto}"),
        None => format!("{cabeca}\n\n{contexto}"),
    }
}

/// Cria uma análise do fluxo a partir da funcionalidade e guarda nela a
/// funcionalidade. `explicar` é o de sempre (uma por funcionalidade); `impacto`
/// exige a demanda; `historias` e `cenarios` partem da explicação aprovada ou da
/// demanda.
pub fn criar_analise(
    dados: &DiretorioDados,
    projeto: &str,
    funcionalidade: &str,
    fluxo: Fluxo,
    texto: Option<&str>,
    demanda: Option<&str>,
) -> Result<AnaliseCriada, String> {
    if fluxo == Fluxo::Explicar {
        return crate::funcionalidades::explicar_funcionalidade(dados, projeto, funcionalidade);
    }
    if fluxo == Fluxo::Manual {
        return crate::manual::gerar_manual(dados, projeto, funcionalidade);
    }
    let amb = ambiente(dados, projeto)?;
    let fid = IdFuncionalidade::analisar(funcionalidade.trim())
        .map_err(|_| format!("\"{funcionalidade}\" não é uma funcionalidade (F-<SIGLA>-NNN)"))?;
    let mapa = MapaFuncionalidades::carregar(&amb.kb).map_err(|e| e.to_string())?;
    let f = mapa
        .funcionalidade(&fid)
        .filter(|f| f.aberta())
        .ok_or_else(|| format!("a funcionalidade {fid} não está no mapa"))?
        .clone();
    let demanda = demanda.map(str::trim).filter(|d| !d.is_empty());
    let explicacao_aprovada = mapear::explicacoes(&amb.analises)
        .get(&fid)
        .filter(|a| a.estado == EstadoAnalise::Aprovada)
        .map(|a| a.id.to_string());
    let criada = match fluxo {
        Fluxo::Perguntar => {
            let pergunta = texto
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .ok_or("escreva a pergunta")?;
            let e = entrada(Some(pergunta), &f, "");
            match demanda {
                Some(d) => crate::demandas::criar_analise_ligada(
                    dados,
                    projeto,
                    d,
                    Fluxo::Perguntar,
                    &e,
                    Some(pergunta),
                )?,
                None => crate::analise::criar_analise_com_demanda(
                    dados,
                    projeto,
                    Fluxo::Perguntar,
                    &e,
                    Some(pergunta),
                    None,
                )?,
            }
        }
        Fluxo::Impacto => {
            let d = demanda.ok_or("escolha a demanda cujo impacto será analisado")?;
            let e = entrada(
                texto,
                &f,
                &format!("Impacto da demanda {d} na funcionalidade {}.", f.id),
            );
            crate::demandas::criar_analise_ligada(
                dados,
                projeto,
                d,
                Fluxo::Impacto,
                &e,
                Some(&format!("{d} · impacto em {} · {}", f.id, f.nome)),
            )?
        }
        Fluxo::Historias | Fluxo::Cenarios => {
            if explicacao_aprovada.is_none() && demanda.is_none() {
                return Err(format!(
                    "{} parte da explicação aprovada da funcionalidade ou de uma demanda: \
                     aprove a explicação de {fid} ou escolha a demanda",
                    nome_do_fluxo(fluxo)
                ));
            }
            let e = entrada(
                texto,
                &f,
                &format!("{} da funcionalidade {}.", nome_do_fluxo(fluxo), f.id),
            );
            let titulo = format!("{} · {} · {}", f.id, f.nome, nome_do_fluxo(fluxo));
            let origem = if demanda.is_some() {
                None
            } else {
                explicacao_aprovada.as_deref()
            };
            if fluxo == Fluxo::Historias {
                crate::analise::criar_analise_de_historias(
                    dados,
                    projeto,
                    origem,
                    demanda,
                    Some(&e),
                    Some(&titulo),
                )?
            } else {
                crate::analise::criar_analise_de_cenarios(
                    dados,
                    projeto,
                    origem,
                    demanda,
                    Some(&e),
                    Some(&titulo),
                )?
            }
        }
        outro => {
            return Err(format!(
                "o painel da funcionalidade não abre {}",
                nome_do_fluxo(outro)
            ));
        }
    };
    let id = cba_kb::Id::analisar(&criada.analise.id).map_err(|e| e.to_string())?;
    amb.analises
        .alterar(&id, Mudanca::Estado, |a, _| {
            a.funcionalidade = Some(fid);
            Ok(())
        })
        .map_err(|e| e.to_string())?;
    Ok(criada)
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
pub async fn painel_funcionalidade_app(
    projeto: String,
    funcionalidade: String,
) -> Result<PainelVista, String> {
    let dados = dados()?;
    bloqueante(move || {
        // Todas: o painel lista as ligadas e as abertas do projeto (#292).
        crate::demandas::conferir_status(
            &dados,
            &projeto,
            None,
            false,
            &cba_repos::demanda::Ferramentas::default(),
        )?;
        painel(&dados, &projeto, &funcionalidade)
    })
    .await
}

#[tauri::command]
pub async fn criar_analise_da_funcionalidade_app(
    projeto: String,
    funcionalidade: String,
    fluxo: Fluxo,
    texto: Option<String>,
    demanda: Option<String>,
) -> Result<AnaliseCriada, String> {
    let dados = dados()?;
    bloqueante(move || {
        criar_analise(
            &dados,
            &projeto,
            &funcionalidade,
            fluxo,
            texto.as_deref(),
            demanda.as_deref(),
        )
    })
    .await
}

#[cfg(test)]
mod enunciado_testes {
    #[test]
    fn le_a_secao_sem_as_notas() {
        let corpo = "# RN-X · 1\n\n- **Status:** confirmada\n\n## Enunciado\n\nSó fatura aberta pode ser cancelada.[^1]\n\nO motivo é obrigatório.[^ev-2]\n\n[^1]: `db/a.sql`, linhas 1-2\n\n## Evidências\n\nx\n";
        assert_eq!(
            super::enunciado(corpo).as_deref(),
            Some("Só fatura aberta pode ser cancelada.\n\nO motivo é obrigatório.")
        );
        assert_eq!(super::enunciado("# Sem seção\n"), None);
    }
}

#[cfg(test)]
#[path = "painel_funcionalidade_testes.rs"]
mod tests;
