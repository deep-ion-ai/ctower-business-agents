//! Histórias de usuário com critérios de aceite, no estado da análise do fluxo
//! `historias` (#115).
//!
//! # Modelo
//!
//! - [`Historia`]: identificador estável `HU-NNNN` (sequencial por projeto, nunca
//!   reaproveitado; a numeração é de [`crate::analises::Analises::maior_numero`]),
//!   título e a narrativa "Como <papel>, quero <objetivo>, para <benefício>"
//!   ([`Narrativa`]), os alertas INVEST do agente ([`AlertaInvest`]) e, quando
//!   houver, a proposta de quebra em fatias verticais e a história de que esta é
//!   uma fatia.
//! - [`Criterio`]: critério de aceite `CA-n` (número dentro da história) em
//!   Gherkin ([`Gherkin`]: dado, quando, então), com os fundamentos (afirmações e
//!   regras que o sustentam; ao menos um) e, se depender de uma pergunta ainda em
//!   aberto, a referência a ela: o critério fica "a confirmar".
//!
//! # Estados
//!
//! ```text
//! História:  pendente ──► aceita | descartada | quebrada (substituída pelas fatias)
//!            (desfazer volta a pendente; "quebrar" fica pendente com o pedido)
//! Critério:  pendente ──► aceito | descartado
//! ```
//!
//! Aceitar a história aceita os critérios dela ainda pendentes; o analista pode
//! descartar um critério depois. Registrar uma fatia (`fatia_de`) marca a
//! história original como quebrada.

use cba_kb::IdHistoria;
use serde::{Deserialize, Serialize};

use crate::analise::{Analise, ErroModelo, EstadoAnalise, Fluxo};
use crate::qualidade::CriterioInvest;
use crate::requisito::{Fundamento, RefItem, SituacaoItem, sem_repetidos};

/// Título e narrativa da história: "Como <papel>, quero <objetivo>, para
/// <benefício>".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Narrativa {
    pub titulo: String,
    /// Papel de quem usa ("analista financeiro do cliente PJ").
    pub como: String,
    /// Objetivo, o que a pessoa quer fazer.
    pub quero: String,
    /// Benefício para o negócio.
    pub para: String,
}

impl Narrativa {
    fn conferir(&self) -> Result<Self, ErroModelo> {
        exigir("titulo", &self.titulo)?;
        exigir("como", &self.como)?;
        exigir("quero", &self.quero)?;
        exigir("para", &self.para)?;
        Ok(Self {
            titulo: self.titulo.trim().to_owned(),
            como: self.como.trim().to_owned(),
            quero: self.quero.trim().to_owned(),
            para: self.para.trim().to_owned(),
        })
    }
}

/// Passos de um critério de aceite em Gherkin, com valores concretos.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Gherkin {
    /// Contexto ("uma fatura PJ em aberto de R$ 3.000,00").
    pub dado: String,
    /// Ação ("o cliente pede 3 parcelas").
    pub quando: String,
    /// Resultado esperado ("são geradas 3 parcelas de R$ 1.000,00").
    pub entao: String,
}

impl Gherkin {
    pub(crate) fn conferir(&self) -> Result<Self, ErroModelo> {
        exigir("dado", &self.dado)?;
        exigir("quando", &self.quando)?;
        exigir("entao", &self.entao)?;
        Ok(Self {
            dado: self.dado.trim().to_owned(),
            quando: self.quando.trim().to_owned(),
            entao: self.entao.trim().to_owned(),
        })
    }
}

/// Critério INVEST que a história não atende, com o motivo. Os critérios são os
/// de [`crate::qualidade::CriterioInvest`], os mesmos da revisão de requisito.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlertaInvest {
    pub criterio: CriterioInvest,
    pub motivo: String,
}

impl AlertaInvest {
    /// Nome do critério para o analista, em minúsculas ("pequena").
    pub fn nome(&self) -> String {
        self.criterio.rotulo().to_lowercase()
    }
}

/// Posição do critério na sigla INVEST, para ordenar.
fn posicao(c: CriterioInvest) -> usize {
    CriterioInvest::TODOS
        .iter()
        .position(|x| *x == c)
        .unwrap_or(usize::MAX)
}

/// Situação de uma história.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SituacaoHistoria {
    /// Registrada pelo agente, sem decisão.
    #[default]
    Pendente,
    Aceita,
    Descartada,
    /// Substituída pelas fatias registradas com `fatia_de`.
    Quebrada,
}

/// Critério de aceite de uma história.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Criterio {
    /// `CA-<numero>`, sequencial dentro da história.
    pub numero: u32,
    #[serde(flatten)]
    pub passos: Gherkin,
    /// Passos registrados pelo agente, guardados na primeira edição do analista.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original: Option<Gherkin>,
    /// Afirmações e regras que sustentam o critério (ao menos uma).
    pub fundamentos: Vec<Fundamento>,
    /// Pergunta em aberto de que o critério depende: com ela, o critério fica "a
    /// confirmar".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pergunta: Option<RefItem>,
    #[serde(default)]
    pub situacao: SituacaoItem,
    #[serde(default)]
    pub sessao: Option<String>,
    pub registrado_ms: u64,
}

impl Criterio {
    /// Identificador dentro da história (`CA-1`).
    pub fn rotulo(&self) -> String {
        format!("CA-{}", self.numero)
    }

    /// Depende de uma pergunta em aberto.
    pub fn a_confirmar(&self) -> bool {
        self.pergunta.is_some()
    }
}

/// História de usuário.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Historia {
    pub id: IdHistoria,
    #[serde(flatten)]
    pub narrativa: Narrativa,
    /// Narrativa registrada pelo agente, guardada na primeira edição do analista.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original: Option<Narrativa>,
    #[serde(default)]
    pub criterios: Vec<Criterio>,
    /// Critérios INVEST que o agente julgou não atendidos.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub invest: Vec<AlertaInvest>,
    /// Proposta de quebra em fatias verticais, com o porquê (história grande).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quebra: Option<String>,
    /// História de que esta é uma fatia.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fatia_de: Option<IdHistoria>,
    #[serde(default)]
    pub situacao: SituacaoHistoria,
    /// O analista pediu a quebra em fatias (com o que quer, se disse; texto vazio
    /// sem pedido). Volta a `None` quando a história é quebrada ou decidida.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quebra_pedida: Option<String>,
    #[serde(default)]
    pub sessao: Option<String>,
    pub registrada_ms: u64,
}

impl Historia {
    /// Critérios que não foram descartados.
    pub fn criterios_validos(&self) -> impl Iterator<Item = &Criterio> {
        self.criterios
            .iter()
            .filter(|c| c.situacao != SituacaoItem::Descartado)
    }

    /// Alertas INVEST da história: os do agente e, se nenhum critério válido
    /// puder ser testado já (sem critério, ou todos a confirmar), o de testável.
    pub fn alertas_invest(&self) -> Vec<AlertaInvest> {
        let mut alertas = self.invest.clone();
        if !alertas
            .iter()
            .any(|a| a.criterio == CriterioInvest::Testavel)
        {
            let validos: Vec<&Criterio> = self.criterios_validos().collect();
            let motivo = if validos.is_empty() {
                Some("a história ainda não tem critério de aceite")
            } else if validos.iter().all(|c| c.a_confirmar()) {
                Some("todos os critérios dependem de pergunta em aberto")
            } else {
                None
            };
            if let Some(m) = motivo {
                alertas.push(AlertaInvest {
                    criterio: CriterioInvest::Testavel,
                    motivo: m.into(),
                });
            }
        }
        alertas.sort_by_key(|a| posicao(a.criterio));
        alertas
    }

    /// Entra no documento: nem descartada nem quebrada.
    pub fn no_documento(&self) -> bool {
        matches!(
            self.situacao,
            SituacaoHistoria::Pendente | SituacaoHistoria::Aceita
        )
    }
}

/// Dados de um critério novo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NovoCriterio {
    pub passos: Gherkin,
    pub fundamentos: Vec<Fundamento>,
    pub pergunta: Option<RefItem>,
    pub sessao: Option<String>,
}

/// Dados de uma história nova.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NovaHistoria {
    pub narrativa: Narrativa,
    pub criterios: Vec<NovoCriterio>,
    pub invest: Vec<AlertaInvest>,
    pub quebra: Option<String>,
    pub fatia_de: Option<IdHistoria>,
    pub sessao: Option<String>,
}

/// Decisão do analista sobre uma história.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "decisao", rename_all = "kebab-case")]
pub enum DecisaoHistoria {
    /// Aceita, com os critérios ainda pendentes.
    Aceitar,
    Descartar,
    /// Pede ao agente a quebra em fatias; a história fica pendente.
    Quebrar {
        #[serde(default)]
        pedido: Option<String>,
    },
    /// Volta a pendente (desfaz aceitar, descartar ou o pedido de quebra).
    Desfazer,
}

fn exigir(campo: &'static str, valor: &str) -> Result<(), ErroModelo> {
    if valor.trim().is_empty() {
        Err(ErroModelo::CampoVazio(campo))
    } else {
        Ok(())
    }
}

fn texto_opcional(texto: Option<String>) -> Option<String> {
    texto.map(|t| t.trim().to_owned()).filter(|t| !t.is_empty())
}

impl NovoCriterio {
    fn criar(self, numero: u32, agora_ms: u64) -> Result<Criterio, ErroModelo> {
        let passos = self.passos.conferir()?;
        let fundamentos = sem_repetidos(self.fundamentos);
        if fundamentos.is_empty() {
            return Err(ErroModelo::CriterioSemFundamento);
        }
        Ok(Criterio {
            numero,
            passos,
            original: None,
            fundamentos,
            pergunta: self.pergunta,
            situacao: SituacaoItem::Pendente,
            sessao: self.sessao,
            registrado_ms: agora_ms,
        })
    }
}

impl Analise {
    fn exigir_fluxo_de_historias(&self) -> Result<(), ErroModelo> {
        if self.fluxo == Fluxo::Historias {
            Ok(())
        } else {
            Err(ErroModelo::ForaDoFluxoDeHistorias(self.id.clone()))
        }
    }

    fn exigir_nao_aprovada(&self) -> Result<(), ErroModelo> {
        if self.estado == EstadoAnalise::Aprovada {
            Err(ErroModelo::AnaliseAprovada(self.id.clone()))
        } else {
            Ok(())
        }
    }

    /// Maior número de história desta análise (0 sem histórias).
    pub fn maior_historia(&self) -> u32 {
        self.historias
            .iter()
            .map(|h| h.id.numero())
            .max()
            .unwrap_or(0)
    }

    pub fn historia(&self, id: IdHistoria) -> Option<&Historia> {
        self.historias.iter().find(|h| h.id == id)
    }

    fn historia_mut(&mut self, id: IdHistoria) -> Result<&mut Historia, ErroModelo> {
        self.historias
            .iter_mut()
            .find(|h| h.id == id)
            .ok_or(ErroModelo::HistoriaDesconhecida(id))
    }

    /// Registra uma história com o identificador `id` (reservado por quem chama,
    /// sem colisão no projeto) e os critérios dela, numerados a partir de 1.
    /// Tudo ou nada: um critério recusado recusa a história. Com `fatia_de`, a
    /// história original fica quebrada.
    pub fn registrar_historia(
        &mut self,
        nova: NovaHistoria,
        id: IdHistoria,
        agora_ms: u64,
    ) -> Result<IdHistoria, ErroModelo> {
        self.exigir_fluxo_de_historias()?;
        let narrativa = nova.narrativa.conferir()?;
        if self.historia(id).is_some() {
            return Err(ErroModelo::HistoriaRepetida(id));
        }
        if let Some(original) = nova.fatia_de {
            match self.historia(original) {
                None => return Err(ErroModelo::HistoriaDesconhecida(original)),
                Some(h) if h.situacao == SituacaoHistoria::Descartada => {
                    return Err(ErroModelo::ItemDecidido {
                        tipo: "a história de que esta seria fatia",
                        numero: original.numero(),
                    });
                }
                Some(_) => {}
            }
        }
        let mut invest: Vec<AlertaInvest> = Vec::new();
        for a in nova.invest {
            exigir("invest.motivo", &a.motivo)?;
            if !invest.iter().any(|x| x.criterio == a.criterio) {
                invest.push(AlertaInvest {
                    criterio: a.criterio,
                    motivo: a.motivo.trim().to_owned(),
                });
            }
        }
        let criterios = nova
            .criterios
            .into_iter()
            .zip(1..)
            .map(|(c, n)| c.criar(n, agora_ms))
            .collect::<Result<Vec<_>, _>>()?;
        self.aceitar_registro()?;
        if let Some(original) = nova.fatia_de {
            let h = self.historia_mut(original)?;
            h.situacao = SituacaoHistoria::Quebrada;
            h.quebra_pedida = None;
        }
        self.historias.push(Historia {
            id,
            narrativa,
            original: None,
            criterios,
            invest,
            quebra: texto_opcional(nova.quebra),
            fatia_de: nova.fatia_de,
            situacao: SituacaoHistoria::Pendente,
            quebra_pedida: None,
            sessao: nova.sessao,
            registrada_ms: agora_ms,
        });
        Ok(id)
    }

    /// Acrescenta um critério a uma história e devolve o número dele. A história
    /// descartada ou quebrada não recebe critérios.
    pub fn registrar_criterio(
        &mut self,
        historia: IdHistoria,
        novo: NovoCriterio,
        agora_ms: u64,
    ) -> Result<u32, ErroModelo> {
        self.exigir_fluxo_de_historias()?;
        let h = self
            .historia(historia)
            .ok_or(ErroModelo::HistoriaDesconhecida(historia))?;
        if !h.no_documento() {
            return Err(ErroModelo::HistoriaFechada(historia));
        }
        let numero = h.criterios.iter().map(|c| c.numero).max().unwrap_or(0) + 1;
        let criterio = novo.criar(numero, agora_ms)?;
        self.aceitar_registro()?;
        self.historia_mut(historia)?.criterios.push(criterio);
        Ok(numero)
    }

    /// Decisão do analista sobre uma história.
    pub fn decidir_historia(
        &mut self,
        id: IdHistoria,
        decisao: DecisaoHistoria,
    ) -> Result<(), ErroModelo> {
        self.exigir_nao_aprovada()?;
        let h = self.historia_mut(id)?;
        if h.situacao == SituacaoHistoria::Quebrada {
            return Err(ErroModelo::HistoriaFechada(id));
        }
        match decisao {
            DecisaoHistoria::Aceitar => {
                h.situacao = SituacaoHistoria::Aceita;
                h.quebra_pedida = None;
                for c in &mut h.criterios {
                    if c.situacao == SituacaoItem::Pendente {
                        c.situacao = SituacaoItem::Aceito;
                    }
                }
            }
            DecisaoHistoria::Descartar => {
                h.situacao = SituacaoHistoria::Descartada;
                h.quebra_pedida = None;
            }
            DecisaoHistoria::Quebrar { pedido } => {
                h.situacao = SituacaoHistoria::Pendente;
                h.quebra_pedida = Some(texto_opcional(pedido).unwrap_or_default());
            }
            DecisaoHistoria::Desfazer => {
                h.situacao = SituacaoHistoria::Pendente;
                h.quebra_pedida = None;
            }
        }
        Ok(())
    }

    /// Decisão do analista sobre um critério (`Pendente` desfaz).
    pub fn decidir_criterio(
        &mut self,
        historia: IdHistoria,
        numero: u32,
        situacao: SituacaoItem,
    ) -> Result<(), ErroModelo> {
        self.exigir_nao_aprovada()?;
        let h = self.historia_mut(historia)?;
        if !h.no_documento() {
            return Err(ErroModelo::HistoriaFechada(historia));
        }
        let c = h.criterios.iter_mut().find(|c| c.numero == numero).ok_or(
            ErroModelo::ItemDesconhecido {
                tipo: "critério",
                numero,
            },
        )?;
        c.situacao = situacao;
        Ok(())
    }

    /// Edição do analista na narrativa de uma história; a do agente fica em
    /// [`Historia::original`] na primeira edição. Voltar ao texto original apaga a
    /// marca.
    pub fn editar_historia(&mut self, id: IdHistoria, nova: Narrativa) -> Result<(), ErroModelo> {
        self.exigir_nao_aprovada()?;
        let nova = nova.conferir()?;
        let h = self.historia_mut(id)?;
        if !h.no_documento() {
            return Err(ErroModelo::HistoriaFechada(id));
        }
        if h.narrativa == nova {
            return Ok(());
        }
        let original = h.original.take().unwrap_or_else(|| h.narrativa.clone());
        if original != nova {
            h.original = Some(original);
        }
        h.narrativa = nova;
        Ok(())
    }

    /// Edição do analista nos passos de um critério; fundamentos e pergunta ficam.
    pub fn editar_criterio(
        &mut self,
        historia: IdHistoria,
        numero: u32,
        passos: Gherkin,
    ) -> Result<(), ErroModelo> {
        self.exigir_nao_aprovada()?;
        let passos = passos.conferir()?;
        let h = self.historia_mut(historia)?;
        if !h.no_documento() {
            return Err(ErroModelo::HistoriaFechada(historia));
        }
        let c = h.criterios.iter_mut().find(|c| c.numero == numero).ok_or(
            ErroModelo::ItemDesconhecido {
                tipo: "critério",
                numero,
            },
        )?;
        if c.passos == passos {
            return Ok(());
        }
        let original = c.original.take().unwrap_or_else(|| c.passos.clone());
        if original != passos {
            c.original = Some(original);
        }
        c.passos = passos;
        Ok(())
    }

    /// Histórias que entram no documento (nem descartadas nem quebradas).
    pub fn historias_no_documento(&self) -> impl Iterator<Item = &Historia> {
        self.historias.iter().filter(|h| h.no_documento())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cba_kb::{Id, Serie};

    fn analise() -> Analise {
        let mut a = Analise::nova(
            Serie::Analise.id(2),
            "faturamento-exemplo",
            Fluxo::Historias,
            "Histórias do parcelamento",
            "Histórias da análise A-0001",
            1,
        );
        a.origem = Some(Serie::Analise.id(1));
        a
    }

    fn regra() -> Fundamento {
        Fundamento::Regra {
            id: Id::analisar("RN-FAT-007").unwrap(),
            titulo: "Limite de parcelas".into(),
            evidencias: vec![],
        }
    }

    fn criterio(fundamentos: Vec<Fundamento>) -> NovoCriterio {
        NovoCriterio {
            passos: Gherkin {
                dado: " uma fatura PJ de R$ 3.000,00 ".into(),
                quando: "o cliente pede 3 parcelas".into(),
                entao: "são geradas 3 parcelas de R$ 1.000,00".into(),
            },
            fundamentos,
            pergunta: None,
            sessao: None,
        }
    }

    fn nova(criterios: Vec<NovoCriterio>) -> NovaHistoria {
        NovaHistoria {
            narrativa: Narrativa {
                titulo: "Parcelar fatura PJ".into(),
                como: "analista financeiro do cliente PJ".into(),
                quero: "parcelar uma fatura em até 3 vezes".into(),
                para: "distribuir o pagamento".into(),
            },
            criterios,
            invest: vec![],
            quebra: None,
            fatia_de: None,
            sessao: None,
        }
    }

    #[test]
    fn registra_historia_com_criterios_numerados() {
        let mut a = analise();
        let id = a
            .registrar_historia(
                nova(vec![criterio(vec![regra(), regra()])]),
                IdHistoria::novo(5),
                10,
            )
            .unwrap();
        assert_eq!(id.to_string(), "HU-0005");
        assert_eq!(a.estado, EstadoAnalise::EmAndamento);
        assert_eq!(a.maior_historia(), 5);
        let h = a.historia(id).unwrap();
        assert_eq!(h.criterios[0].rotulo(), "CA-1");
        assert_eq!(h.criterios[0].passos.dado, "uma fatura PJ de R$ 3.000,00");
        // Fundamento repetido entra uma vez só.
        assert_eq!(h.criterios[0].fundamentos.len(), 1);
        let n = a
            .registrar_criterio(id, criterio(vec![regra()]), 11)
            .unwrap();
        assert_eq!(n, 2);
        assert_eq!(
            a.registrar_historia(nova(vec![]), IdHistoria::novo(5), 12),
            Err(ErroModelo::HistoriaRepetida(IdHistoria::novo(5)))
        );
    }

    #[test]
    fn criterio_sem_fundamento_e_recusado() {
        let mut a = analise();
        assert_eq!(
            a.registrar_historia(nova(vec![criterio(vec![])]), IdHistoria::novo(1), 1),
            Err(ErroModelo::CriterioSemFundamento)
        );
        assert!(a.historias.is_empty());
        a.registrar_historia(nova(vec![]), IdHistoria::novo(1), 1)
            .unwrap();
        assert_eq!(
            a.registrar_criterio(IdHistoria::novo(1), criterio(vec![]), 2),
            Err(ErroModelo::CriterioSemFundamento)
        );
    }

    #[test]
    fn so_no_fluxo_de_historias() {
        let mut a = analise();
        a.fluxo = Fluxo::Impacto;
        assert!(matches!(
            a.registrar_historia(nova(vec![]), IdHistoria::novo(1), 1),
            Err(ErroModelo::ForaDoFluxoDeHistorias(_))
        ));
    }

    #[test]
    fn invest_testavel_derivado_dos_criterios() {
        let mut a = analise();
        let mut n = nova(vec![]);
        n.invest = vec![AlertaInvest {
            criterio: CriterioInvest::Pequena,
            motivo: "tela e relatório juntos".into(),
        }];
        let id = a.registrar_historia(n, IdHistoria::novo(1), 1).unwrap();
        let letras: Vec<_> = a
            .historia(id)
            .unwrap()
            .alertas_invest()
            .iter()
            .map(|x| x.criterio.letra())
            .collect();
        assert_eq!(letras, ['S', 'T']);
        let mut c = criterio(vec![regra()]);
        c.pergunta = Some(RefItem::analisar("A-0001/2").unwrap());
        a.registrar_criterio(id, c, 2).unwrap();
        assert_eq!(a.historia(id).unwrap().alertas_invest().len(), 2);
        a.registrar_criterio(id, criterio(vec![regra()]), 3)
            .unwrap();
        assert_eq!(a.historia(id).unwrap().alertas_invest().len(), 1);
    }

    #[test]
    fn decisoes_edicao_e_quebra() {
        let mut a = analise();
        let id = a
            .registrar_historia(
                nova(vec![criterio(vec![regra()]), criterio(vec![regra()])]),
                IdHistoria::novo(1),
                1,
            )
            .unwrap();
        a.decidir_criterio(id, 2, SituacaoItem::Descartado).unwrap();
        a.decidir_historia(id, DecisaoHistoria::Aceitar).unwrap();
        let h = a.historia(id).unwrap();
        assert_eq!(h.situacao, SituacaoHistoria::Aceita);
        assert_eq!(h.criterios[0].situacao, SituacaoItem::Aceito);
        assert_eq!(h.criterios[1].situacao, SituacaoItem::Descartado);

        let mut editada = h.narrativa.clone();
        editada.titulo = "Parcelar fatura de PJ".into();
        a.editar_historia(id, editada.clone()).unwrap();
        let h = a.historia(id).unwrap();
        assert_eq!(h.narrativa.titulo, "Parcelar fatura de PJ");
        assert_eq!(h.original.as_ref().unwrap().titulo, "Parcelar fatura PJ");
        let mut volta = editada;
        volta.titulo = "Parcelar fatura PJ".into();
        a.editar_historia(id, volta).unwrap();
        assert!(a.historia(id).unwrap().original.is_none());

        a.editar_criterio(
            id,
            1,
            Gherkin {
                dado: "uma fatura PJ de R$ 300,00".into(),
                quando: "o cliente pede 3 parcelas".into(),
                entao: "são geradas 3 parcelas de R$ 100,00".into(),
            },
        )
        .unwrap();
        assert!(a.historia(id).unwrap().criterios[0].original.is_some());

        a.decidir_historia(
            id,
            DecisaoHistoria::Quebrar {
                pedido: Some(" tela e relatório ".into()),
            },
        )
        .unwrap();
        let h = a.historia(id).unwrap();
        assert_eq!(h.situacao, SituacaoHistoria::Pendente);
        assert_eq!(h.quebra_pedida.as_deref(), Some("tela e relatório"));

        let mut fatia = nova(vec![criterio(vec![regra()])]);
        fatia.fatia_de = Some(id);
        a.registrar_historia(fatia, IdHistoria::novo(2), 5).unwrap();
        let h = a.historia(id).unwrap();
        assert_eq!(h.situacao, SituacaoHistoria::Quebrada);
        assert!(h.quebra_pedida.is_none());
        assert_eq!(a.historias_no_documento().count(), 1);
        assert_eq!(
            a.decidir_historia(id, DecisaoHistoria::Aceitar),
            Err(ErroModelo::HistoriaFechada(id))
        );
        assert_eq!(
            a.registrar_criterio(id, criterio(vec![regra()]), 6),
            Err(ErroModelo::HistoriaFechada(id))
        );
    }

    #[test]
    fn estado_ida_e_volta_em_json() {
        let mut a = analise();
        let mut c = criterio(vec![regra()]);
        c.pergunta = Some(RefItem::analisar("A-0001/2").unwrap());
        a.registrar_historia(nova(vec![c]), IdHistoria::novo(1), 1)
            .unwrap();
        let json = serde_json::to_string(&a).unwrap();
        assert!(json.contains("\"id\":\"HU-0001\""), "{json}");
        assert!(json.contains("\"pergunta\":\"A-0001/2\""), "{json}");
        assert!(json.contains("\"origem\":\"A-0001\""), "{json}");
        let lida: Analise = serde_json::from_str(&json).unwrap();
        assert_eq!(lida, a);
    }
}
