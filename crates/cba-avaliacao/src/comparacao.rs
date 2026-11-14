//! Comparação do estado de uma análise (`A-NNNN.estado.json`) com o gabarito.
//!
//! É a parte determinística da avaliação: recebe o estado gravado, o item do gabarito
//! (pergunta, funcionalidade ou demanda) e o escopo do projeto, e mede, sem chamar
//! nenhum modelo:
//!
//! - **evidências válidas:** cada evidência das afirmações é conferida de novo contra
//!   o código da fixture, com a mesma validação do `afirmar`
//!   ([`cba_tools::evidencias::validar`]), e o hash do intervalo precisa ser o gravado;
//! - **nenhuma afirmação `confirmado` sem evidência válida:** sem evidência ou com
//!   alguma evidência inválida é falha grave;
//! - **regras encontradas:** uma regra (`RN-*`) é encontrada quando uma evidência
//!   válida de alguma afirmação está no mesmo arquivo de uma evidência esperada da
//!   regra e os intervalos de linhas se sobrepõem. Uma inconsistência (`INC-*`) exige
//!   que **uma mesma afirmação** se sobreponha a duas evidências esperadas diferentes
//!   dela (os dois lados da divergência). Evidências com mais de
//!   [`LINHAS_PARA_CASAR`] linhas não contam, para um intervalo largo não "achar"
//!   todas as regras do arquivo;
//! - **perguntas esperadas:** atendidas por palavras-chave ([`crate::texto`]) no texto
//!   das perguntas registradas;
//! - **premissa corrigida** (impacto): uma afirmação da seção
//!   `demanda_como_entendida`, com evidência válida, cujo veredito ("Verificação:
//!   não confere | confere em parte") é o esperado e que atende às palavras-chave
//!   da premissa. Para `confere_em_parte`, `não confere` também vale;
//! - **abrangência** (impacto): a afirmação mais recente da seção `abrangencia`
//!   começa pela estimativa esperada;
//! - **arquivos citados:** quais arquivos esperados do item aparecem em evidências
//!   válidas.

use std::collections::{BTreeMap, BTreeSet};

use cba_core::analise::{Afirmacao, Analise, Confianca, EstadoAfirmacao, EstadoAnalise, Evidencia};
use cba_index::Escopo;
use cba_kb::frontmatter::Abrangencia;
use cba_tools::documento::{SECAO_DA_ABRANGENCIA, abrangencia_do_texto};
use cba_tools::evidencias::{EvidenciaCitada, validar};
use serde::Serialize;

use crate::gabarito::{Gabarito, Item, Regra, Veredito};
use crate::parecer::{ParecerAvaliado, avaliar_parecer};
use crate::texto;

/// Evidências mais largas que isso não contam para achar regras.
pub const LINHAS_PARA_CASAR: u32 = 30;
/// Seção do fluxo de impacto onde a premissa é verificada.
pub const SECAO_DA_PREMISSA: &str = "demanda_como_entendida";

/// Resultado da comparação de uma análise com um item do gabarito.
#[derive(Debug, Clone, Serialize)]
pub struct Avaliacao {
    pub item: String,
    pub fluxo: String,
    pub analise: String,
    /// Estado da análise (`concluida` quando o agente chamou `concluir_analise`).
    pub estado: String,
    pub concluida: bool,
    pub afirmacoes: usize,
    pub por_confianca: PorConfianca,
    pub perguntas_registradas: usize,
    pub sugestoes: usize,
    pub termos: usize,
    pub evidencias: usize,
    pub evidencias_validas: usize,
    pub evidencias_invalidas: Vec<EvidenciaInvalida>,
    /// Afirmações `confirmado` sem evidência ou com alguma evidência inválida.
    pub confirmado_sem_evidencia_valida: Vec<u32>,
    pub regras: Vec<RegraAvaliada>,
    pub regras_encontradas: usize,
    pub regras_esperadas: usize,
    /// Regras do gabarito fora do item que a análise também encontrou.
    pub outras_regras_encontradas: Vec<String>,
    /// Afirmações com evidência válida que não tocam nenhuma evidência do gabarito.
    pub afirmacoes_fora_do_gabarito: Vec<u32>,
    pub arquivos: Vec<ArquivoAvaliado>,
    pub perguntas_esperadas: Vec<PerguntaAvaliada>,
    pub premissa: Option<PremissaAvaliada>,
    pub abrangencia: Option<AbrangenciaAvaliada>,
    /// Parecer de revisão (fluxo `revisar`, [`crate::parecer`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parecer: Option<ParecerAvaliado>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct PorConfianca {
    pub confirmado: usize,
    pub inferido: usize,
    pub a_confirmar: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct EvidenciaInvalida {
    pub afirmacao: u32,
    pub arquivo: String,
    pub linhas: String,
    pub motivo: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RegraAvaliada {
    pub id: String,
    pub titulo: String,
    pub encontrada: bool,
    /// Afirmações que a encontraram.
    pub afirmacoes: Vec<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArquivoAvaliado {
    pub arquivo: String,
    pub citado: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct PerguntaAvaliada {
    pub tema: String,
    /// Número da primeira pergunta registrada que atende.
    pub atendida_por: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PremissaAvaliada {
    pub veredito_esperado: Veredito,
    pub corrigida: bool,
    /// Afirmação que corrige a premissa.
    pub afirmacao: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AbrangenciaAvaliada {
    pub esperada: String,
    pub registrada: Option<String>,
    pub confere: bool,
}

impl Avaliacao {
    /// Falhas graves: invalidam a execução independentemente da cobertura.
    pub fn falhas(&self) -> Vec<String> {
        let mut f = Vec::new();
        // No parecer de revisão, o conteúdo são os achados; as afirmações são opcionais.
        match &self.parecer {
            Some(p) => f.extend(p.falhas()),
            None if self.afirmacoes == 0 => f.push("nenhuma afirmação registrada".to_owned()),
            None => {}
        }
        if !self.confirmado_sem_evidencia_valida.is_empty() {
            f.push(format!(
                "afirmações `confirmado` sem evidência válida: {}",
                lista_de_numeros(&self.confirmado_sem_evidencia_valida)
            ));
        }
        f
    }

    /// Fração das regras esperadas encontradas (0 a 1).
    pub fn cobertura_de_regras(&self) -> f64 {
        fracao(self.regras_encontradas, self.regras_esperadas)
    }

    /// Fração das perguntas esperadas atendidas, se o item tiver alguma.
    pub fn cobertura_de_perguntas(&self) -> Option<f64> {
        let total = self.perguntas_esperadas.len();
        (total > 0).then(|| {
            fracao(
                self.perguntas_esperadas
                    .iter()
                    .filter(|p| p.atendida_por.is_some())
                    .count(),
                total,
            )
        })
    }

    /// Fração dos arquivos esperados citados, se o item tiver algum.
    pub fn cobertura_de_arquivos(&self) -> Option<f64> {
        let total = self.arquivos.len();
        (total > 0).then(|| fracao(self.arquivos.iter().filter(|a| a.citado).count(), total))
    }
}

pub(crate) fn fracao(parte: usize, total: usize) -> f64 {
    if total == 0 {
        1.0
    } else {
        parte as f64 / total as f64
    }
}

pub(crate) fn lista_de_numeros(numeros: &[u32]) -> String {
    numeros
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Veredito de uma afirmação no formato "Premissa do solicitante: ... Verificação:
/// <veredito>. ...". Sem o marcador "Verificação", procura o veredito no texto todo.
pub fn veredito_do_texto(texto: &str) -> Option<Veredito> {
    let n = texto::normalizar(texto);
    let trecho = match n.find("verificacao") {
        Some(i) => {
            let resto = &n[i + "verificacao".len()..];
            resto
                .trim_start_matches(|c: char| c == ':' || c == '*' || c.is_whitespace())
                .to_owned()
        }
        None => {
            return if n.contains("nao confere") {
                Some(Veredito::NaoConfere)
            } else if n.contains("confere em parte") || n.contains("confere parcialmente") {
                Some(Veredito::ConfereEmParte)
            } else {
                None
            };
        }
    };
    if trecho.starts_with("nao confere") {
        Some(Veredito::NaoConfere)
    } else if trecho.starts_with("confere em parte") || trecho.starts_with("confere parcialmente") {
        Some(Veredito::ConfereEmParte)
    } else if trecho.starts_with("confere") {
        Some(Veredito::Confere)
    } else {
        None
    }
}

fn sobrepoe(e: &Evidencia, arquivo: &str, inicio: u32, fim: u32) -> bool {
    e.arquivo.as_str() == arquivo && e.linhas.inicio <= fim && inicio <= e.linhas.fim
}

/// Índices das evidências esperadas de `regra` que `evidencias` tocam.
fn tocadas(regra: &Regra, evidencias: &[&Evidencia]) -> BTreeSet<usize> {
    regra
        .evidencias
        .iter()
        .enumerate()
        .filter(|(_, esperada)| {
            evidencias.iter().any(|e| {
                sobrepoe(
                    e,
                    &esperada.arquivo,
                    esperada.linha_inicial,
                    esperada.linha_final,
                )
            })
        })
        .map(|(i, _)| i)
        .collect()
}

/// As evidências dadas tocam uma evidência esperada da regra (evidências largas
/// demais não contam).
pub(crate) fn toca(regra: &Regra, evidencias: &[&Evidencia]) -> bool {
    let ev: Vec<&Evidencia> = evidencias
        .iter()
        .copied()
        .filter(|e| e.linhas.fim - e.linhas.inicio < LINHAS_PARA_CASAR)
        .collect();
    !tocadas(regra, &ev).is_empty()
}

/// A afirmação, pelas evidências dadas, encontra a regra.
fn encontra(regra: &Regra, evidencias: &[&Evidencia]) -> bool {
    let n = tocadas(regra, evidencias).len();
    if regra.e_inconsistencia() {
        n >= regra.evidencias.len().min(2)
    } else {
        n >= 1
    }
}

/// Compara `analise` com `item`, conferindo as evidências em `escopo` (o projeto da
/// fixture).
pub fn avaliar(analise: &Analise, item: &Item, gabarito: &Gabarito, escopo: &Escopo) -> Avaliacao {
    let mut por_confianca = PorConfianca::default();
    let mut evidencias = 0;
    let mut invalidas = Vec::new();
    let mut confirmado_sem = Vec::new();
    // Evidências válidas de cada afirmação considerada (não marcada como incorreta).
    let mut validas_por_afirmacao: BTreeMap<u32, Vec<&Evidencia>> = BTreeMap::new();

    for a in &analise.afirmacoes {
        match a.confianca {
            Confianca::Confirmado => por_confianca.confirmado += 1,
            Confianca::Inferido => por_confianca.inferido += 1,
            Confianca::AConfirmar => por_confianca.a_confirmar += 1,
        }
        let mut validas = Vec::new();
        let mut todas_validas = true;
        for e in &a.evidencias {
            evidencias += 1;
            match conferir(escopo, e) {
                Ok(()) => validas.push(e),
                Err(motivo) => {
                    todas_validas = false;
                    invalidas.push(EvidenciaInvalida {
                        afirmacao: a.numero,
                        arquivo: e.arquivo.as_str().to_owned(),
                        linhas: format!("{}-{}", e.linhas.inicio, e.linhas.fim),
                        motivo,
                    });
                }
            }
        }
        if a.confianca == Confianca::Confirmado && (a.evidencias.is_empty() || !todas_validas) {
            confirmado_sem.push(a.numero);
        }
        if !matches!(a.estado, EstadoAfirmacao::Incorreta { .. }) {
            validas_por_afirmacao.insert(a.numero, validas);
        }
    }
    let evidencias_validas = evidencias - invalidas.len();

    // Regras encontradas, por afirmação.
    let mut encontradas: BTreeMap<&str, Vec<u32>> = BTreeMap::new();
    let mut fora = Vec::new();
    for (numero, ev) in &validas_por_afirmacao {
        let ev: Vec<&Evidencia> = ev
            .iter()
            .copied()
            .filter(|e| e.linhas.fim - e.linhas.inicio < LINHAS_PARA_CASAR)
            .collect();
        if ev.is_empty() {
            continue;
        }
        let mut tocou = false;
        for regra in gabarito.todas_as_regras() {
            if !tocadas(regra, &ev).is_empty() {
                tocou = true;
            }
            if encontra(regra, &ev) {
                encontradas
                    .entry(regra.id.as_str())
                    .or_default()
                    .push(*numero);
            }
        }
        if !tocou {
            fora.push(*numero);
        }
    }
    let regras: Vec<RegraAvaliada> = item
        .regras
        .iter()
        .map(|id| {
            let afirmacoes = encontradas.get(id.as_str()).cloned().unwrap_or_default();
            RegraAvaliada {
                id: id.clone(),
                titulo: gabarito
                    .regra(id)
                    .map(|r| r.titulo.clone())
                    .unwrap_or_default(),
                encontrada: !afirmacoes.is_empty(),
                afirmacoes,
            }
        })
        .collect();
    let outras: Vec<String> = encontradas
        .keys()
        .filter(|id| !item.regras.iter().any(|r| r == *id))
        .map(|id| id.to_string())
        .collect();

    // Arquivos esperados citados por evidências válidas.
    let citados: BTreeSet<&str> = validas_por_afirmacao
        .values()
        .flatten()
        .map(|e| e.arquivo.as_str())
        .collect();
    let arquivos = item
        .arquivos
        .iter()
        .map(|a| ArquivoAvaliado {
            arquivo: a.clone(),
            citado: citados.contains(a.as_str()),
        })
        .collect();

    // Perguntas esperadas.
    let perguntas_esperadas = item
        .perguntas_esperadas
        .iter()
        .map(|esperada| PerguntaAvaliada {
            tema: esperada.tema.clone(),
            atendida_por: analise
                .perguntas
                .iter()
                .find(|p| texto::atende(&p.texto, &esperada.palavras_chave))
                .map(|p| p.numero),
        })
        .collect();

    // Premissa (impacto).
    let premissa = item.verificacao_da_premissa.as_ref().map(|v| {
        let aceita = |r: Veredito| match v.veredito {
            Veredito::ConfereEmParte => {
                matches!(r, Veredito::ConfereEmParte | Veredito::NaoConfere)
            }
            esperado => r == esperado,
        };
        let afirmacao = analise
            .afirmacoes
            .iter()
            .filter(|a| a.secao == SECAO_DA_PREMISSA)
            .filter(|a| {
                validas_por_afirmacao
                    .get(&a.numero)
                    .is_some_and(|ev| !ev.is_empty())
            })
            .filter(|a| veredito_do_texto(do_agente(a)).is_some_and(aceita))
            .find(|a| texto::atende(do_agente(a), &v.palavras_chave))
            .map(|a| a.numero);
        PremissaAvaliada {
            veredito_esperado: v.veredito,
            corrigida: afirmacao.is_some(),
            afirmacao,
        }
    });

    // Abrangência (impacto): a afirmação mais recente da seção, não incorreta.
    let abrangencia = item.abrangencia.as_ref().map(|esperada| {
        let registrada = analise
            .afirmacoes
            .iter()
            .rev()
            .filter(|a| a.secao == SECAO_DA_ABRANGENCIA)
            .filter(|a| !matches!(a.estado, EstadoAfirmacao::Incorreta { .. }))
            .find_map(|a| abrangencia_do_texto(do_agente(a)))
            .map(|a| {
                match a {
                    Abrangencia::Pequena => "pequena",
                    Abrangencia::Media => "media",
                    Abrangencia::Grande => "grande",
                }
                .to_owned()
            });
        AbrangenciaAvaliada {
            esperada: esperada.clone(),
            confere: registrada.as_deref() == Some(texto::normalizar(esperada).as_str()),
            registrada,
        }
    });

    let regras_encontradas = regras.iter().filter(|r| r.encontrada).count();
    Avaliacao {
        item: item.id.clone(),
        fluxo: nome_do_fluxo(item.fluxo).to_owned(),
        analise: analise.id.to_string(),
        estado: serde_json::to_value(analise.estado)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default(),
        concluida: matches!(
            analise.estado,
            EstadoAnalise::Concluida | EstadoAnalise::Aprovada
        ),
        afirmacoes: analise.afirmacoes.len(),
        por_confianca,
        perguntas_registradas: analise.perguntas.len(),
        sugestoes: analise.sugestoes.len(),
        termos: analise.termos.len(),
        evidencias,
        evidencias_validas,
        evidencias_invalidas: invalidas,
        confirmado_sem_evidencia_valida: confirmado_sem,
        regras_esperadas: regras.len(),
        regras_encontradas,
        regras,
        outras_regras_encontradas: outras,
        afirmacoes_fora_do_gabarito: fora,
        arquivos,
        perguntas_esperadas,
        premissa,
        abrangencia,
        parecer: item
            .revisao
            .as_ref()
            .map(|r| avaliar_parecer(analise, r, gabarito, escopo)),
    }
}

/// Texto que o agente registrou (o analista pode ter editado depois).
fn do_agente(a: &Afirmacao) -> &str {
    a.texto_original.as_deref().unwrap_or(&a.texto)
}

/// Nome do fluxo como no estado e no `iniciar_analise`.
pub fn nome_do_fluxo(fluxo: cba_core::analise::Fluxo) -> &'static str {
    use cba_core::analise::Fluxo;
    match fluxo {
        Fluxo::Perguntar => "perguntar",
        Fluxo::Explicar => "explicar",
        Fluxo::Impacto => "impacto",
        Fluxo::Reuniao => "reuniao",
        Fluxo::Revisar => "revisar",
        Fluxo::Historias => "historias",
    }
}

/// Confere a evidência gravada contra o código: a mesma validação do registro e o
/// mesmo hash.
pub(crate) fn conferir_evidencia(escopo: &Escopo, e: &Evidencia) -> Result<(), String> {
    conferir(escopo, e)
}

fn conferir(escopo: &Escopo, e: &Evidencia) -> Result<(), String> {
    let citada = EvidenciaCitada {
        arquivo: e.arquivo.as_str().to_owned(),
        linha_inicial: e.linhas.inicio,
        linha_final: e.linhas.fim,
        trecho: e.trecho.clone(),
    };
    let nova = validar(escopo, &citada).map_err(|r| r.motivo.to_string())?;
    if nova.hash != e.hash {
        return Err("o hash do intervalo não confere com o gravado".to_owned());
    }
    Ok(())
}
