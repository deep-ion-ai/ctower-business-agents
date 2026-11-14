//! Comparação das histórias com critérios de aceite (fluxo `historias`, #115)
//! com o gabarito.
//!
//! Determinística, como o resto da comparação. Para cada item `historias` do
//! gabarito, mede:
//!
//! - **critérios sem origem:** todo critério precisa citar ao menos uma afirmação
//!   ou regra; critério sem origem é falha grave (a ferramenta já recusa, então
//!   indica estado adulterado ou regressão);
//! - **histórias esperadas:** uma história registrada (não descartada nem
//!   quebrada) atende quando o título e a narrativa têm um termo de cada grupo de
//!   palavras-chave;
//! - **regras citadas:** cada regra esperada é citada por algum critério, pelo
//!   identificador ou por uma afirmação cuja evidência toca uma evidência esperada
//!   da regra;
//! - **valores concretos:** critérios com ao menos um número no Dado, no Quando ou
//!   no Então;
//! - **a confirmar:** cada tema esperado tem um critério com pergunta cujo texto
//!   tem as palavras-chave;
//! - **quebra:** quando o gabarito pede, há proposta de quebra, fatia ou mais de
//!   uma história.

use cba_core::analise::Analise;
use cba_core::historias::{Criterio, Historia};
use cba_core::requisito::Fundamento;
use cba_kb::frontmatter::Evidencia;
use serde::Serialize;

use crate::gabarito::{EvidenciaEsperada, Gabarito, HistoriasEsperadas};
use crate::texto;

/// Evidências mais largas que isso não contam para achar regras (como na
/// comparação das afirmações).
const LINHAS_PARA_CASAR: u32 = 30;

/// Resultado da comparação das histórias.
#[derive(Debug, Clone, Serialize)]
pub struct HistoriasAvaliadas {
    pub historias: usize,
    pub criterios: usize,
    /// Critérios sem nenhuma afirmação ou regra de origem (`HU-0001 CA-2`).
    pub criterios_sem_origem: Vec<String>,
    /// Histórias sem critério de aceite.
    pub historias_sem_criterio: Vec<String>,
    pub criterios_com_valor: usize,
    pub esperadas: Vec<HistoriaAvaliada>,
    pub regras: Vec<RegraCitada>,
    pub a_confirmar: Vec<TemaAvaliado>,
    pub quebra_esperada: bool,
    pub quebra_proposta: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct HistoriaAvaliada {
    pub id: String,
    pub descricao: String,
    /// Primeira história registrada que atende (`HU-0001`).
    pub atendida_por: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RegraCitada {
    pub id: String,
    /// Primeiro critério que cita a regra (`HU-0001 CA-1`).
    pub citada_por: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TemaAvaliado {
    pub tema: String,
    pub marcado_em: Option<String>,
}

impl HistoriasAvaliadas {
    /// Falhas graves das histórias.
    pub fn falhas(&self) -> Vec<String> {
        let mut f = Vec::new();
        if self.historias == 0 {
            f.push("nenhuma história registrada".to_owned());
        }
        if !self.criterios_sem_origem.is_empty() {
            f.push(format!(
                "critérios sem origem: {}",
                self.criterios_sem_origem.join(", ")
            ));
        }
        f
    }

    pub fn historias_atendidas(&self) -> usize {
        self.esperadas
            .iter()
            .filter(|h| h.atendida_por.is_some())
            .count()
    }

    pub fn regras_citadas(&self) -> usize {
        self.regras
            .iter()
            .filter(|r| r.citada_por.is_some())
            .count()
    }

    pub fn temas_marcados(&self) -> usize {
        self.a_confirmar
            .iter()
            .filter(|t| t.marcado_em.is_some())
            .count()
    }
}

fn texto_da_historia(h: &Historia) -> String {
    let n = &h.narrativa;
    format!("{} {} {} {}", n.titulo, n.como, n.quero, n.para)
}

fn texto_do_criterio(c: &Criterio) -> String {
    format!("{} {} {}", c.passos.dado, c.passos.quando, c.passos.entao)
}

fn tem_numero(c: &Criterio) -> bool {
    texto_do_criterio(c).chars().any(|ch| ch.is_ascii_digit())
}

fn toca(esperadas: &[EvidenciaEsperada], e: &Evidencia) -> bool {
    e.linhas.fim - e.linhas.inicio < LINHAS_PARA_CASAR
        && esperadas.iter().any(|x| {
            x.arquivo == e.arquivo.as_str()
                && e.linhas.inicio <= x.linha_final
                && x.linha_inicial <= e.linhas.fim
        })
}

/// O critério cita a regra: pelo id ou por uma evidência de uma origem que toca
/// a regra.
fn cita(c: &Criterio, id: &str, esperadas: &[EvidenciaEsperada]) -> bool {
    c.fundamentos.iter().any(|f| match f {
        Fundamento::Regra { id: r, .. } if r.to_string() == id => true,
        outro => outro.evidencias().iter().any(|e| toca(esperadas, e)),
    })
}

/// Prepara a análise de origem das histórias, como o analista a teria deixado:
/// a análise de impacto da demanda do gabarito, com uma afirmação confirmada por
/// regra da demanda (o enunciado e as evidências esperadas, conferidas como no
/// `afirmar`) e uma pergunta em aberto por pergunta esperada, concluída e
/// aprovada; e as regras da demanda gravadas na base (`regras/RN-…md`), como a
/// aprovação de análises anteriores as teria deixado. Devolve o identificador da
/// análise de origem.
pub fn preparar_origem(
    ambiente: &cba_core::ambiente::Ambiente,
    gabarito: &Gabarito,
    esperado: &HistoriasEsperadas,
) -> Result<cba_kb::Id, String> {
    use cba_core::analise::{Confianca, EstadoAfirmacao, Fluxo, NovaAfirmacao};
    use cba_core::analises::NovaAnalise;
    use cba_core::eventos::Mudanca;
    use cba_kb::frontmatter::{FrontmatterRegra, StatusRegra, TipoDocumento};
    use cba_tools::evidencias::{EvidenciaCitada, validar};

    let demanda = gabarito
        .demandas
        .iter()
        .find(|d| d.id == esperado.demanda)
        .ok_or_else(|| format!("a demanda {} não existe no gabarito", esperado.demanda))?;
    let origem = ambiente
        .analises
        .criar(NovaAnalise {
            fluxo: Fluxo::Impacto,
            titulo: format!("Impacto · {}", demanda.id),
            entrada: demanda.texto.clone(),
            analista: None,
            base: None,
        })
        .map_err(|e| format!("análise de origem: {e}"))?;
    let mut afirmacoes = Vec::new();
    for id in demanda.regras.iter().filter(|r| r.starts_with("RN-")) {
        let regra = gabarito
            .regras
            .iter()
            .find(|r| &r.id == id)
            .ok_or_else(|| format!("a regra {id} não existe no gabarito"))?;
        let mut evidencias = Vec::new();
        for e in &regra.evidencias {
            let citada = EvidenciaCitada {
                arquivo: e.arquivo.clone(),
                linha_inicial: e.linha_inicial,
                linha_final: e.linha_final,
                trecho: e.trecho.clone(),
            };
            evidencias
                .push(validar(&ambiente.escopo, &citada).map_err(|r| format!("{id}: {r:?}"))?);
        }
        let rn = cba_kb::Id::analisar(id).map_err(|e| e.to_string())?;
        let documento = cba_kb::Documento::novo(
            FrontmatterRegra {
                tipo: TipoDocumento::Regra,
                id: rn.clone(),
                titulo: regra.titulo.clone(),
                area: "geral".into(),
                status: StatusRegra::Confirmada,
                evidencias: evidencias
                    .iter()
                    .map(|e| Evidencia {
                        arquivo: e.arquivo.clone(),
                        linhas: e.linhas,
                        hash: e.hash.clone(),
                        commit: e.commit.clone(),
                    })
                    .collect(),
                origem: Some(origem.documento.clone()),
            },
            format!(
                "\n# {id} · {}\n\n{}\n",
                regra.titulo,
                regra.enunciado.trim()
            ),
        );
        ambiente
            .kb
            .gravar(&rn.arquivo(), &documento)
            .map_err(|e| format!("regra {id}: {e}"))?;
        afirmacoes.push(NovaAfirmacao {
            secao: "como_e_hoje".into(),
            texto: regra.enunciado.trim().to_owned(),
            confianca: Confianca::Confirmado,
            evidencias,
            sessao: None,
        });
    }
    let perguntas: Vec<String> = demanda
        .perguntas_esperadas
        .iter()
        .map(|p| p.tema.clone())
        .collect();
    ambiente
        .analises
        .alterar(&origem.id, Mudanca::Estado, |a, agora| {
            for nova in afirmacoes {
                let n = a.registrar_afirmacao(nova, agora)?;
                a.marcar_afirmacao(n, EstadoAfirmacao::Confirmada)?;
            }
            for p in &perguntas {
                a.registrar_pergunta(p, None, None, agora)?;
            }
            let abrangencia = format!(
                "{}: abrangência do gabarito.",
                match demanda.abrangencia_esperada.as_str() {
                    "pequena" => "Pequena",
                    "grande" => "Grande",
                    _ => "Média",
                }
            );
            let n = a.registrar_afirmacao(
                NovaAfirmacao {
                    secao: "abrangencia".into(),
                    texto: abrangencia,
                    confianca: Confianca::Inferido,
                    evidencias: Vec::new(),
                    sessao: None,
                },
                agora,
            )?;
            a.marcar_afirmacao(n, EstadoAfirmacao::Confirmada)?;
            a.concluir()?;
            a.aprovar()
        })
        .map_err(|e| format!("análise de origem: {e}"))?;
    Ok(origem.id)
}

/// Compara as histórias da análise com o esperado.
pub fn avaliar_historias(
    analise: &Analise,
    esperado: &HistoriasEsperadas,
    gabarito: &Gabarito,
) -> HistoriasAvaliadas {
    let historias: Vec<&Historia> = analise.historias_no_documento().collect();
    let criterios: Vec<(&Historia, &Criterio)> = historias
        .iter()
        .flat_map(|h| h.criterios_validos().map(move |c| (*h, c)))
        .collect();
    let rotulo = |h: &Historia, c: &Criterio| format!("{} {}", h.id, c.rotulo());

    let esperadas = esperado
        .historias_esperadas
        .iter()
        .map(|e| HistoriaAvaliada {
            id: e.id.clone(),
            descricao: e.descricao.clone(),
            atendida_por: historias
                .iter()
                .find(|h| texto::atende(&texto_da_historia(h), &e.palavras_chave))
                .map(|h| h.id.to_string()),
        })
        .collect();
    let regras = esperado
        .regras
        .iter()
        .map(|id| {
            let evidencias = gabarito
                .regras
                .iter()
                .chain(&gabarito.inconsistencias)
                .find(|r| &r.id == id)
                .map(|r| r.evidencias.as_slice())
                .unwrap_or_default();
            RegraCitada {
                id: id.clone(),
                citada_por: criterios
                    .iter()
                    .find(|(_, c)| cita(c, id, evidencias))
                    .map(|(h, c)| rotulo(h, c)),
            }
        })
        .collect();
    let a_confirmar = esperado
        .a_confirmar
        .iter()
        .map(|t| TemaAvaliado {
            tema: t.tema.clone(),
            marcado_em: criterios
                .iter()
                .filter(|(_, c)| c.a_confirmar())
                .find(|(_, c)| texto::atende(&texto_do_criterio(c), &t.palavras_chave))
                .map(|(h, c)| rotulo(h, c)),
        })
        .collect();
    HistoriasAvaliadas {
        historias: historias.len(),
        criterios: criterios.len(),
        criterios_sem_origem: criterios
            .iter()
            .filter(|(_, c)| c.fundamentos.is_empty())
            .map(|(h, c)| rotulo(h, c))
            .collect(),
        historias_sem_criterio: historias
            .iter()
            .filter(|h| h.criterios_validos().next().is_none())
            .map(|h| h.id.to_string())
            .collect(),
        criterios_com_valor: criterios.iter().filter(|(_, c)| tem_numero(c)).count(),
        esperadas,
        regras,
        a_confirmar,
        quebra_esperada: esperado.quebra,
        quebra_proposta: historias.len() > 1
            || analise
                .historias
                .iter()
                .any(|h| h.quebra.is_some() || h.fatia_de.is_some()),
    }
}
