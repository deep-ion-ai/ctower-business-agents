//! Comparação do mapa de funcionalidades do projeto (#208) com o gabarito.
//!
//! - **Funcionalidade** esperada: uma funcionalidade registrada (não descartada
//!   nem juntada) com as palavras-chave no nome ou na descrição e ao menos um
//!   ponto de entrada esperado dela. A área confere quando o slug é o esperado.
//! - **Ponto de entrada** esperado: **tratado** quando está numa funcionalidade
//!   ou justificado; **bem tratado** quando a funcionalidade em que está casou
//!   com uma das esperadas para ele ou, se justificado, quando o gabarito o
//!   aceita como utilitário (`justificavel`).
//! - **Evidências** dos pontos de entrada conferidas contra o código.
//!
//! Falhas graves: nenhuma funcionalidade registrada; ponto de entrada do
//! gabarito sem tratamento; evidência de ponto de entrada que não confere.

use std::collections::BTreeMap;

use cba_core::funcionalidades::{Funcionalidade, MapaFuncionalidades};
use cba_index::Escopo;
use serde::Serialize;

use crate::comparacao::{conferir_evidencia, fracao};
use crate::gabarito::MapeamentoEsperado;
use crate::texto;

/// Resultado da comparação do mapa.
#[derive(Debug, Clone, Serialize)]
pub struct MapeamentoAvaliado {
    pub funcionalidades_registradas: usize,
    pub funcionalidades: Vec<FuncionalidadeAvaliada>,
    pub pontos: Vec<PontoAvaliado>,
    /// Pontos de entrada com evidência que não confere: `F-…: arquivo:linhas (motivo)`.
    pub evidencias_invalidas: Vec<String>,
}

/// Funcionalidade esperada e a registrada que casou.
#[derive(Debug, Clone, Serialize)]
pub struct FuncionalidadeAvaliada {
    pub esperada: String,
    pub nome: String,
    pub registrada: Option<String>,
    pub area_confere: bool,
}

/// Ponto de entrada esperado.
#[derive(Debug, Clone, Serialize)]
pub struct PontoAvaliado {
    pub chave: String,
    /// `atribuido`, `justificado` ou `sem_tratamento`.
    pub tratamento: String,
    /// Funcionalidade registrada em que o ponto está.
    pub funcionalidade: Option<String>,
    pub bem_tratado: bool,
}

impl MapeamentoAvaliado {
    pub fn falhas(&self) -> Vec<String> {
        let mut f = Vec::new();
        if self.funcionalidades_registradas == 0 {
            f.push("nenhuma funcionalidade registrada".to_owned());
        }
        let sem: Vec<&str> = self
            .pontos
            .iter()
            .filter(|p| p.tratamento == "sem_tratamento")
            .map(|p| p.chave.as_str())
            .collect();
        if !sem.is_empty() {
            f.push(format!(
                "pontos de entrada sem tratamento: {}",
                sem.join(", ")
            ));
        }
        if !self.evidencias_invalidas.is_empty() {
            f.push(format!(
                "pontos de entrada com evidência inválida: {}",
                self.evidencias_invalidas.join("; ")
            ));
        }
        f
    }

    /// Fração das funcionalidades esperadas encontradas.
    pub fn cobertura_de_funcionalidades(&self) -> f64 {
        fracao(
            self.funcionalidades
                .iter()
                .filter(|f| f.registrada.is_some())
                .count(),
            self.funcionalidades.len(),
        )
    }

    /// Fração dos pontos de entrada esperados bem tratados.
    pub fn cobertura_de_pontos(&self) -> f64 {
        fracao(
            self.pontos.iter().filter(|p| p.bem_tratado).count(),
            self.pontos.len(),
        )
    }
}

fn texto_da(f: &Funcionalidade) -> String {
    format!("{} {}", f.nome, f.descricao)
}

/// Compara as funcionalidades do mapa com o mapeamento esperado.
pub fn avaliar_mapeamento(
    mapa: &MapaFuncionalidades,
    esperado: &MapeamentoEsperado,
    escopo: &Escopo,
) -> MapeamentoAvaliado {
    let abertas: Vec<&Funcionalidade> = mapa.abertas().collect();
    // Pontos esperados de cada funcionalidade do gabarito.
    let mut pontos_de: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for p in &esperado.pontos_de_entrada {
        for f in &p.funcionalidades {
            pontos_de
                .entry(f.as_str())
                .or_default()
                .push(p.chave.as_str());
        }
    }
    let mut casadas: BTreeMap<&str, String> = BTreeMap::new();
    let funcionalidades = esperado
        .funcionalidades
        .iter()
        .map(|e| {
            let pontos = pontos_de.get(e.id.as_str()).cloned().unwrap_or_default();
            let registrada = abertas.iter().find(|f| {
                texto::atende(&texto_da(f), &e.palavras_chave)
                    && f.chaves().any(|c| pontos.contains(&c))
            });
            if let Some(r) = registrada {
                casadas.insert(e.id.as_str(), r.id.to_string());
            }
            FuncionalidadeAvaliada {
                esperada: e.id.clone(),
                nome: e.nome.clone().unwrap_or_else(|| e.id.clone()),
                registrada: registrada.map(|r| r.id.to_string()),
                area_confere: registrada.is_some_and(|r| r.area == e.area),
            }
        })
        .collect();
    let pontos = esperado
        .pontos_de_entrada
        .iter()
        .map(|p| {
            let dona = mapa.dona_do_ponto(&p.chave);
            let justificado = mapa.justificativa(&p.chave).is_some();
            let (tratamento, bem) = match dona {
                Some(f) => (
                    "atribuido",
                    p.funcionalidades
                        .iter()
                        .any(|e| casadas.get(e.as_str()) == Some(&f.id.to_string())),
                ),
                None if justificado => ("justificado", p.justificavel),
                None => ("sem_tratamento", false),
            };
            PontoAvaliado {
                chave: p.chave.clone(),
                tratamento: tratamento.to_owned(),
                funcionalidade: dona.map(|f| f.id.to_string()),
                bem_tratado: bem,
            }
        })
        .collect();
    let mut evidencias_invalidas = Vec::new();
    for f in &abertas {
        for p in &f.pontos {
            if let Err(motivo) = conferir_evidencia(escopo, &p.evidencia) {
                evidencias_invalidas.push(format!(
                    "{}: {}:{} ({motivo})",
                    f.id, p.evidencia.arquivo, p.evidencia.linhas
                ));
            }
        }
    }
    MapeamentoAvaliado {
        funcionalidades_registradas: abertas.len(),
        funcionalidades,
        pontos,
        evidencias_invalidas,
    }
}
