//! Comparação dos cenários de teste (fluxo `cenarios`, #117) com o gabarito.
//!
//! Determinística, como o resto da comparação. Para cada item `cenarios` do
//! gabarito, mede:
//!
//! - **cenários sem origem:** todo cenário cita ao menos uma regra ou afirmação;
//!   cenário sem origem é falha grave (a ferramenta já recusa, então indica estado
//!   adulterado ou regressão), assim como nenhum cenário registrado;
//! - **limites lidos do código:** cada limite esperado é atendido por um cenário
//!   cujo limite tem o mesmo valor, não vem de configuração e cita uma evidência
//!   que toca a evidência esperada; limite de cenário da mesma regra com outro
//!   valor é listado como divergente; e cada valor testado esperado (o limite e o
//!   vizinho) aparece num cenário da regra;
//! - **tabelas de decisão:** cada tabela esperada é atendida por uma tabela com ao
//!   menos as condições esperadas, que cita a regra (pelo id ou por evidência) e
//!   tem as palavras-chave nas condições;
//! - **negativos:** cada cenário negativo esperado é atendido por um cenário da
//!   regra com as palavras-chave;
//! - **a confirmar:** cada tema esperado tem um cenário com pergunta e as
//!   palavras-chave.

use cba_core::analise::Analise;
use cba_core::cenarios::{Cenario, TabelaDecisao, numeros};
use cba_core::requisito::Fundamento;
use cba_kb::frontmatter::Evidencia;
use serde::Serialize;

use crate::gabarito::{CenariosEsperados, EvidenciaEsperada, Gabarito};
use crate::texto;

/// Evidências mais largas que isso não contam para achar regras.
const LINHAS_PARA_CASAR: u32 = 30;

/// Resultado da comparação dos cenários.
#[derive(Debug, Clone, Serialize)]
pub struct CenariosAvaliados {
    pub cenarios: usize,
    pub tabelas: usize,
    /// Cenários sem nenhuma regra ou afirmação de origem.
    pub cenarios_sem_origem: Vec<String>,
    pub limites: Vec<LimiteAvaliado>,
    pub tabelas_esperadas: Vec<ItemAvaliado>,
    pub negativos: Vec<ItemAvaliado>,
    pub a_confirmar: Vec<ItemAvaliado>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LimiteAvaliado {
    pub id: String,
    pub regra: String,
    pub valor: f64,
    /// Primeiro cenário cujo limite é o do código (`CT-0001`).
    pub lido_por: Option<String>,
    /// Cenários da regra com limite de outro valor (`CT-0003: 31`).
    pub divergentes: Vec<String>,
    /// Cada valor testado esperado e o cenário que o testa.
    pub valores: Vec<(f64, Option<String>)>,
}

impl LimiteAvaliado {
    pub fn valores_cobertos(&self) -> usize {
        self.valores.iter().filter(|(_, c)| c.is_some()).count()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ItemAvaliado {
    pub id: String,
    pub descricao: String,
    pub atendido_por: Option<String>,
}

fn atendidos(itens: &[ItemAvaliado]) -> usize {
    itens.iter().filter(|i| i.atendido_por.is_some()).count()
}

impl CenariosAvaliados {
    /// Falhas graves dos cenários.
    pub fn falhas(&self) -> Vec<String> {
        let mut f = Vec::new();
        if self.cenarios == 0 {
            f.push("nenhum cenário de teste registrado".to_owned());
        }
        if !self.cenarios_sem_origem.is_empty() {
            f.push(format!(
                "cenários sem origem: {}",
                self.cenarios_sem_origem.join(", ")
            ));
        }
        f
    }

    pub fn limites_lidos(&self) -> usize {
        self.limites.iter().filter(|l| l.lido_por.is_some()).count()
    }

    pub fn tabelas_atendidas(&self) -> usize {
        atendidos(&self.tabelas_esperadas)
    }

    pub fn negativos_atendidos(&self) -> usize {
        atendidos(&self.negativos)
    }

    pub fn temas_marcados(&self) -> usize {
        atendidos(&self.a_confirmar)
    }
}

fn toca(esperadas: &[EvidenciaEsperada], e: &Evidencia) -> bool {
    e.linhas.fim - e.linhas.inicio < LINHAS_PARA_CASAR
        && esperadas.iter().any(|x| {
            x.arquivo == e.arquivo.as_str()
                && e.linhas.inicio <= x.linha_final
                && x.linha_inicial <= e.linhas.fim
        })
}

/// Os fundamentos citam a regra: pelo id ou por uma evidência que toca a regra.
fn cita(fundamentos: &[Fundamento], id: &str, esperadas: &[EvidenciaEsperada]) -> bool {
    fundamentos.iter().any(|f| match f {
        Fundamento::Regra { id: r, .. } if r.to_string() == id => true,
        outro => outro.evidencias().iter().any(|e| toca(esperadas, e)),
    })
}

fn texto_do_cenario(c: &Cenario) -> String {
    let p = &c.texto.passos;
    let mut t = format!(
        "{} {} {} {} {}",
        c.texto.titulo,
        p.dado,
        p.quando,
        p.entao,
        c.valor.as_deref().unwrap_or_default()
    );
    if let Some(l) = &c.limite {
        t.push_str(&format!(
            " {} {} {}",
            l.grandeza,
            l.valor,
            l.configuracao.as_deref().unwrap_or_default()
        ));
    }
    t
}

fn texto_das_condicoes(t: &TabelaDecisao) -> String {
    t.condicoes
        .iter()
        .map(|c| c.texto.as_str())
        .chain([t.titulo.as_str()])
        .collect::<Vec<_>>()
        .join(" ")
}

fn evidencias_da_regra<'a>(gabarito: &'a Gabarito, id: &str) -> &'a [EvidenciaEsperada] {
    gabarito
        .regras
        .iter()
        .find(|r| r.id == id)
        .map(|r| r.evidencias.as_slice())
        .unwrap_or_default()
}

/// A evidência registrada como a do frontmatter (sem o trecho).
fn sem_trecho(e: &cba_core::analise::Evidencia) -> Evidencia {
    Evidencia {
        arquivo: e.arquivo.clone(),
        linhas: e.linhas,
        hash: e.hash.clone(),
        commit: e.commit.clone(),
    }
}

fn igual(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

/// Grava na base as regras pedidas, como a aprovação de análises anteriores as
/// teria deixado (o preparo da avaliação do fluxo `cenarios`).
pub fn preparar_regras(
    ambiente: &cba_core::ambiente::Ambiente,
    gabarito: &Gabarito,
    regras: &[String],
) -> Result<(), String> {
    for id in regras {
        let regra = gabarito
            .regras
            .iter()
            .find(|r| &r.id == id)
            .ok_or_else(|| format!("a regra {id} não existe no gabarito"))?;
        crate::historias::gravar_regra(ambiente, regra, None)?;
    }
    Ok(())
}

/// Compara as tabelas e os cenários da análise com o esperado.
pub fn avaliar_cenarios(
    analise: &Analise,
    esperado: &CenariosEsperados,
    gabarito: &Gabarito,
) -> CenariosAvaliados {
    let cenarios: Vec<&Cenario> = analise.cenarios_no_documento().collect();
    let tabelas: Vec<&TabelaDecisao> = analise.tabelas_no_documento().collect();
    let da_regra = |regra: &str| -> Vec<&Cenario> {
        let evidencias = evidencias_da_regra(gabarito, regra);
        cenarios
            .iter()
            .copied()
            .filter(|c| cita(&c.fundamentos, regra, evidencias))
            .collect()
    };

    let limites = esperado
        .limites
        .iter()
        .map(|l| {
            let candidatos = da_regra(&l.regra);
            let alvo = std::slice::from_ref(&l.evidencia);
            let lido_por = candidatos
                .iter()
                .find(|c| {
                    c.limite.as_ref().is_some_and(|lim| {
                        lim.configuracao.is_none()
                            && numeros(&lim.valor)
                                .first()
                                .is_some_and(|v| igual(*v, l.valor))
                            && lim.evidencias.iter().any(|e| toca(alvo, &sem_trecho(e)))
                    })
                })
                .map(|c| c.id.to_string());
            let divergentes = candidatos
                .iter()
                .filter_map(|c| {
                    let lim = c.limite.as_ref()?;
                    let v = *numeros(&lim.valor).first()?;
                    (lim.configuracao.is_none() && !igual(v, l.valor))
                        .then(|| format!("{}: {}", c.id, lim.valor))
                })
                .collect();
            let valores = l
                .valores_testados
                .iter()
                .map(|v| {
                    let testa = candidatos
                        .iter()
                        .find(|c| {
                            let t = format!(
                                "{} {} {}",
                                c.valor.as_deref().unwrap_or_default(),
                                c.texto.passos.dado,
                                c.texto.passos.quando
                            );
                            numeros(&t).iter().any(|n| igual(*n, *v))
                        })
                        .map(|c| c.id.to_string());
                    (*v, testa)
                })
                .collect();
            LimiteAvaliado {
                id: l.id.clone(),
                regra: l.regra.clone(),
                valor: l.valor,
                lido_por,
                divergentes,
                valores,
            }
        })
        .collect();

    let tabelas_esperadas = esperado
        .tabelas
        .iter()
        .map(|t| {
            let evidencias = evidencias_da_regra(gabarito, &t.regra);
            ItemAvaliado {
                id: t.id.clone(),
                descricao: t.descricao.clone(),
                atendido_por: tabelas
                    .iter()
                    .find(|tab| {
                        tab.condicoes.len() >= t.condicoes
                            && (cita(&tab.fundamentos, &t.regra, evidencias)
                                || tab.condicoes.iter().any(|c| {
                                    c.evidencias
                                        .iter()
                                        .any(|e| toca(evidencias, &sem_trecho(e)))
                                }))
                            && texto::atende(&texto_das_condicoes(tab), &t.palavras_chave)
                    })
                    .map(|tab| tab.rotulo()),
            }
        })
        .collect();

    let negativos = esperado
        .negativos
        .iter()
        .map(|n| ItemAvaliado {
            id: n.id.clone(),
            descricao: n.descricao.clone(),
            atendido_por: da_regra(&n.regra)
                .iter()
                .find(|c| texto::atende(&texto_do_cenario(c), &n.palavras_chave))
                .map(|c| c.id.to_string()),
        })
        .collect();

    let a_confirmar = esperado
        .a_confirmar
        .iter()
        .map(|t| ItemAvaliado {
            id: t.tema.clone(),
            descricao: t.tema.clone(),
            atendido_por: cenarios
                .iter()
                .filter(|c| c.a_confirmar())
                .find(|c| texto::atende(&texto_do_cenario(c), &t.palavras_chave))
                .map(|c| c.id.to_string()),
        })
        .collect();

    CenariosAvaliados {
        cenarios: cenarios.len(),
        tabelas: tabelas.len(),
        cenarios_sem_origem: cenarios
            .iter()
            .filter(|c| c.fundamentos.is_empty())
            .map(|c| c.id.to_string())
            .collect(),
        limites,
        tabelas_esperadas,
        negativos,
        a_confirmar,
    }
}
