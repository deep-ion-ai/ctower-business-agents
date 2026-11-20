//! Peças comuns aos artefatos de requisito derivados de uma análise (épico #127):
//! histórias com critérios de aceite (#115) e, depois, cenários de teste (#117).
//!
//! - [`RefItem`]: referência a um item numerado de uma análise do projeto
//!   (`A-0193/4`, a afirmação ou a pergunta 4 da análise A-0193).
//! - [`Fundamento`]: o que sustenta um item de requisito: uma afirmação de uma
//!   análise ou uma regra de negócio da base (`RN-…`). Guarda uma cópia do texto
//!   (ou do título) e das evidências no momento do registro, para o documento e a
//!   exportação mostrarem a ligação sem reler a origem. Quem confere que a
//!   referência existe é a ferramenta de registro (`cba_tools::fundamentos`).
//! - [`SituacaoItem`]: a decisão do analista sobre um item (pendente, aceito,
//!   descartado).
//! - [`Revalidacao`]: a marca "a revalidar" de um item cuja evidência deixou de
//!   conferir com o código (#169), ao lado da decisão do analista; e
//!   [`ItemRequisito`], a referência a um critério, um cenário ou uma tabela.
//!
//! Este módulo é só domínio, sem disco.

use std::fmt;

use cba_kb::frontmatter::Evidencia;
use cba_kb::{Id, IdCenario, IdHistoria, Serie};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Item numerado de uma análise do projeto: a afirmação ou a pergunta `numero`
/// da análise `analise`. Escrito `A-0193/4`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RefItem {
    pub analise: Id,
    pub numero: u32,
}

/// Referência fora do formato `A-NNNN/n`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("`{0}` não é uma referência a um item de análise; use A-NNNN/n, como A-0193/4")]
pub struct RefInvalida(pub String);

impl RefItem {
    pub fn nova(analise: Id, numero: u32) -> Self {
        Self { analise, numero }
    }

    /// Lê `A-0193/4` (sem espaços; número a partir de 1).
    pub fn analisar(texto: &str) -> Result<Self, RefInvalida> {
        let erro = || RefInvalida(texto.to_owned());
        let (analise, numero) = texto.split_once('/').ok_or_else(erro)?;
        let analise = Id::analisar(analise).map_err(|_| erro())?;
        // Só dígitos, sem zero à esquerda (o número começa em 1).
        if analise.serie() != &Serie::Analise
            || numero.is_empty()
            || numero.starts_with('0')
            || !numero.bytes().all(|b| b.is_ascii_digit())
        {
            return Err(erro());
        }
        let numero: u32 = numero.parse().map_err(|_| erro())?;
        Ok(Self { analise, numero })
    }
}

impl fmt::Display for RefItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.analise, self.numero)
    }
}

impl Serialize for RefItem {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for RefItem {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let texto = String::deserialize(d)?;
        Self::analisar(&texto).map_err(serde::de::Error::custom)
    }
}

/// O que sustenta um item de requisito (um critério de aceite, um cenário de
/// teste): uma afirmação de uma análise ou uma regra de negócio da base. É o que
/// liga o requisito ao código, pelas evidências da afirmação ou da regra.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "tipo", rename_all = "kebab-case")]
pub enum Fundamento {
    /// Afirmação de uma análise do projeto.
    Afirmacao {
        #[serde(rename = "ref")]
        referencia: RefItem,
        /// Texto da afirmação no registro.
        texto: String,
        /// Evidências da afirmação no registro (sem o trecho, que fica na
        /// análise de origem).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        evidencias: Vec<Evidencia>,
    },
    /// Regra de negócio da base (`regras/RN-<SIGLA>-<NNN>.md`).
    Regra {
        id: Id,
        /// Título da regra no registro.
        titulo: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        evidencias: Vec<Evidencia>,
    },
}

impl Fundamento {
    /// Rótulo curto: `A-0193/4` ou `RN-FAT-007`.
    pub fn rotulo(&self) -> String {
        match self {
            Self::Afirmacao { referencia, .. } => referencia.to_string(),
            Self::Regra { id, .. } => id.to_string(),
        }
    }

    /// Texto da afirmação ou título da regra.
    pub fn texto(&self) -> &str {
        match self {
            Self::Afirmacao { texto, .. } => texto,
            Self::Regra { titulo, .. } => titulo,
        }
    }

    pub fn evidencias(&self) -> &[Evidencia] {
        match self {
            Self::Afirmacao { evidencias, .. } | Self::Regra { evidencias, .. } => evidencias,
        }
    }

    /// Mesma referência (a cópia do texto não conta).
    pub fn mesma_referencia(&self, outro: &Self) -> bool {
        match (self, outro) {
            (Self::Afirmacao { referencia: a, .. }, Self::Afirmacao { referencia: b, .. }) => {
                a == b
            }
            (Self::Regra { id: a, .. }, Self::Regra { id: b, .. }) => a == b,
            _ => false,
        }
    }
}

/// Tira referências repetidas, mantendo a ordem.
pub fn sem_repetidos(fundamentos: Vec<Fundamento>) -> Vec<Fundamento> {
    let mut saida: Vec<Fundamento> = Vec::with_capacity(fundamentos.len());
    for f in fundamentos {
        if !saida.iter().any(|s| s.mesma_referencia(&f)) {
            saida.push(f);
        }
    }
    saida
}

/// Decisão do analista sobre um item de requisito (um critério de aceite, um
/// cenário de teste).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SituacaoItem {
    /// Registrado pelo agente, sem decisão.
    #[default]
    Pendente,
    /// Aceito: vai para o documento aprovado.
    Aceito,
    /// Descartado: fica no estado e sai do documento.
    Descartado,
}

/// Estado de revalidação de um item de requisito (#169).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EstadoItem {
    /// As evidências conferem (ou ninguém marcou o item).
    #[default]
    Valido,
    /// Alguma evidência deixou de conferir com o código, ou o analista marcou o item
    /// para rever.
    ARevalidar,
}

impl EstadoItem {
    pub fn valido(&self) -> bool {
        *self == Self::Valido
    }
}

/// Marca "a revalidar" de um item de requisito (critério de aceite, cenário de
/// teste, tabela de decisão), ao lado da decisão do analista ([`SituacaoItem`]),
/// que a marca não muda (#169).
///
/// Funciona como a das afirmações ([`crate::analise::Afirmacao::antes_de_revalidar`]):
/// a revalidação ([`crate::analise::Analise::revalidar`]) marca o item quando uma
/// evidência dele deixa de conferir e guarda o estado anterior em
/// `antes_de_revalidar`; quando todas voltam a conferir, o item volta a esse estado.
/// A marca do analista ([`Revalidacao::marcar_pelo_analista`]) não guarda estado
/// anterior, e a revalidação não a desfaz.
///
/// As evidências de um fundamento são uma cópia sem estado próprio: o fundamento
/// com evidência que deixou de conferir fica, pelo rótulo, em
/// `fundamentos_a_revalidar`. As evidências do limite de um cenário e das
/// condições de uma tabela têm estado ([`crate::analise::EstadoEvidencia`]).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Revalidacao {
    #[serde(default, skip_serializing_if = "EstadoItem::valido")]
    pub estado: EstadoItem,
    /// Estado do item antes de a revalidação o marcar. Só existe enquanto a marca
    /// for da revalidação.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub antes_de_revalidar: Option<EstadoItem>,
    /// Fundamentos (pelo rótulo, `A-0193/4` ou `RN-FAT-007`) com alguma evidência
    /// que não confere mais com o código.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fundamentos_a_revalidar: Vec<String>,
}

impl Revalidacao {
    pub fn a_revalidar(&self) -> bool {
        self.estado == EstadoItem::ARevalidar
    }

    /// Marcado pelo analista (a revalidação não desfaz).
    pub fn do_analista(&self) -> bool {
        self.a_revalidar() && self.antes_de_revalidar.is_none()
    }

    /// Marca (ou tira a marca) por decisão do analista. A marca do analista fica até
    /// ele tirá-la. Tirar a marca da revalidação também vale: ela só volta se outra
    /// evidência deixar de conferir.
    pub fn marcar_pelo_analista(&mut self, marcar: bool) {
        self.estado = if marcar {
            EstadoItem::ARevalidar
        } else {
            EstadoItem::Valido
        };
        self.antes_de_revalidar = None;
    }

    /// Aplica o resultado da revalidação das evidências do item: `marcou`, alguma
    /// evidência deixou de conferir agora; `alguma`, alguma ainda não confere.
    /// Devolve `Some(true)` se o item passou a a revalidar e `Some(false)` se voltou
    /// ao estado anterior.
    pub(crate) fn aplicar(&mut self, marcou: bool, alguma: bool) -> Option<bool> {
        if marcou && !self.a_revalidar() {
            let antes = std::mem::replace(&mut self.estado, EstadoItem::ARevalidar);
            self.antes_de_revalidar = Some(antes);
            Some(true)
        } else if !alguma
            && self.a_revalidar()
            && let Some(antes) = self.antes_de_revalidar.take()
        {
            self.estado = antes;
            Some(false)
        } else {
            None
        }
    }
}

/// Item de requisito que a revalidação marca (#169): um critério de aceite de uma
/// história, um cenário de teste ou uma tabela de decisão.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "tipo", rename_all = "kebab-case")]
pub enum ItemRequisito {
    Criterio { historia: IdHistoria, numero: u32 },
    Cenario { cenario: IdCenario },
    Tabela { numero: u32 },
}

impl fmt::Display for ItemRequisito {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Criterio { historia, numero } => write!(f, "{historia} CA-{numero}"),
            Self::Cenario { cenario } => write!(f, "{cenario}"),
            Self::Tabela { numero } => write!(f, "TD-{numero}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn referencia_a_item() {
        let r = RefItem::analisar("A-0193/4").unwrap();
        assert_eq!(r.analise.to_string(), "A-0193");
        assert_eq!(r.numero, 4);
        assert_eq!(r.to_string(), "A-0193/4");
        for ruim in [
            "A-0193",
            "A-0193/",
            "A-0193/0",
            "A-0193/04",
            "S-0001/1",
            "A-193/1",
            "4",
            "A-0193/a",
            " A-0193/4",
        ] {
            assert!(RefItem::analisar(ruim).is_err(), "{ruim:?}");
        }
        let json = serde_json::to_string(&r).unwrap();
        assert_eq!(json, "\"A-0193/4\"");
        assert_eq!(serde_json::from_str::<RefItem>(&json).unwrap(), r);
    }

    #[test]
    fn fundamento_no_formato_do_estado() {
        let f = Fundamento::Afirmacao {
            referencia: RefItem::analisar("A-0001/2").unwrap(),
            texto: "Pessoa física não parcela.".into(),
            evidencias: vec![],
        };
        let v = serde_json::to_value(&f).unwrap();
        assert_eq!(
            v,
            serde_json::json!({ "tipo": "afirmacao", "ref": "A-0001/2", "texto": "Pessoa física não parcela." })
        );
        assert_eq!(f.rotulo(), "A-0001/2");
        let r = Fundamento::Regra {
            id: Id::analisar("RN-FAT-007").unwrap(),
            titulo: "Limite de parcelas".into(),
            evidencias: vec![],
        };
        assert_eq!(r.rotulo(), "RN-FAT-007");
        let lista = sem_repetidos(vec![f.clone(), r.clone(), f.clone()]);
        assert_eq!(lista, vec![f, r]);
    }

    #[test]
    fn marca_da_revalidacao_volta_e_a_do_analista_fica() {
        let mut r = Revalidacao::default();
        assert_eq!(serde_json::to_value(&r).unwrap(), serde_json::json!({}));
        assert_eq!(r.aplicar(true, true), Some(true));
        assert!(r.a_revalidar() && !r.do_analista());
        assert_eq!(
            serde_json::to_value(&r).unwrap(),
            serde_json::json!({ "estado": "a-revalidar", "antes_de_revalidar": "valido" })
        );
        // Já marcado: outra evidência que deixa de conferir não conta de novo.
        assert_eq!(r.aplicar(true, true), None);
        assert_eq!(r.aplicar(false, true), None);
        assert_eq!(r.aplicar(false, false), Some(false));
        assert_eq!(r, Revalidacao::default());

        r.marcar_pelo_analista(true);
        assert!(r.do_analista());
        assert_eq!(r.aplicar(false, false), None);
        assert!(r.a_revalidar());
        r.marcar_pelo_analista(false);
        assert!(!r.a_revalidar());
    }

    #[test]
    fn item_de_requisito() {
        let c = ItemRequisito::Criterio {
            historia: IdHistoria::novo(3),
            numero: 2,
        };
        assert_eq!(c.to_string(), "HU-0003 CA-2");
        assert_eq!(
            serde_json::to_value(&c).unwrap(),
            serde_json::json!({ "tipo": "criterio", "historia": "HU-0003", "numero": 2 })
        );
        assert_eq!(ItemRequisito::Tabela { numero: 1 }.to_string(), "TD-1");
    }
}
