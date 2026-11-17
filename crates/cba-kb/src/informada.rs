//! Respostas informadas por pessoas (#123): a pauta de perguntas pendentes junta
//! as perguntas para o solicitante das análises e as dúvidas das atas, e a
//! resposta registrada no app volta para cada pergunta de origem.
//!
//! Os tipos ficam aqui porque são gravados em lugares diferentes: no estado das
//! análises (`cba_core::analise::Pergunta`), no estado das atas
//! (`cba_docs::ItemAta`) e no frontmatter da regra criada a partir da resposta
//! ([`crate::frontmatter::FrontmatterRegra::informada`]).
//!
//! ## Regra informada
//!
//! A regra que nasce de uma resposta não tem evidência no código: vale o que a
//! pessoa informou. É uma exceção ao princípio "sem evidência, não é fato", e por
//! isso fica **visível**: o frontmatter guarda quem informou, quando, a fonte e
//! as perguntas de origem ([`RegraInformada`]), e o enunciado começa com o aviso
//! de que a regra foi informada e não verificada no código.

use serde::{Deserialize, Serialize};

use crate::frontmatter::Data;

/// A quem a pergunta se dirige: uma pessoa do catálogo ou um papel (quando não se
/// sabe ainda quem, na prática, responde).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "tipo", rename_all = "kebab-case")]
pub enum Destinatario {
    /// Pessoa do catálogo, pelo slug (`pessoas/<slug>.md`).
    Pessoa { pessoa: String },
    /// Papel (PO, usuário-chave, gerente…), como no catálogo de papéis.
    Papel { papel: String },
}

/// Quem informou a resposta, quando e de onde ela veio.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Informacao {
    /// Nome de quem respondeu, como o analista quer ver.
    pub por: String,
    /// Slug da pessoa no catálogo, quando é uma pessoa cadastrada.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pessoa: Option<String>,
    /// Data da resposta.
    pub em: Data,
    /// De onde veio a resposta ("reunião de 17/11", "e-mail", "conversa").
    pub fonte: String,
}

impl Informacao {
    /// "Maria Silva em 2026-11-17 (e-mail)".
    pub fn descricao(&self) -> String {
        let fonte = self.fonte.trim();
        if fonte.is_empty() {
            format!("{} em {}", self.por.trim(), self.em)
        } else {
            format!("{} em {} ({fonte})", self.por.trim(), self.em)
        }
    }
}

/// Resposta registrada para uma pergunta: o texto e quem informou.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RespostaInformada {
    pub texto: String,
    #[serde(flatten)]
    pub informacao: Informacao,
}

/// Origem de uma regra criada a partir da resposta a uma pergunta: quem
/// informou e quais perguntas a resposta fechou. A regra não tem evidência de
/// código (veja o módulo).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegraInformada {
    #[serde(flatten)]
    pub informacao: Informacao,
    /// Perguntas de origem (`A-0001/2`, `R-0001/DV-1`).
    #[serde(default)]
    pub perguntas: Vec<String>,
}

impl RegraInformada {
    /// Aviso que abre o enunciado da regra, visível no documento e na exportação.
    pub fn aviso(&self) -> String {
        format!(
            "> **Regra informada por {}**, sem evidência no código. Vale o que a pessoa \
             informou até ser verificada no código. Perguntas de origem: {}.",
            self.informacao.descricao(),
            if self.perguntas.is_empty() {
                "nenhuma".to_owned()
            } else {
                self.perguntas.join(", ")
            }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info() -> Informacao {
        Informacao {
            por: "Maria Silva".into(),
            pessoa: Some("maria-silva".into()),
            em: Data::nova(2026, 11, 17).unwrap(),
            fonte: "e-mail".into(),
        }
    }

    #[test]
    fn destinatario_em_json() {
        let d = Destinatario::Pessoa {
            pessoa: "maria-silva".into(),
        };
        let json = serde_json::to_string(&d).unwrap();
        assert_eq!(json, r#"{"tipo":"pessoa","pessoa":"maria-silva"}"#);
        let p: Destinatario = serde_json::from_str(r#"{"tipo":"papel","papel":"PO"}"#).unwrap();
        assert_eq!(p, Destinatario::Papel { papel: "PO".into() });
    }

    #[test]
    fn aviso_da_regra_informada_diz_quem_e_que_nao_tem_evidencia() {
        let r = RegraInformada {
            informacao: info(),
            perguntas: vec!["A-0001/2".into(), "A-0002/1".into()],
        };
        let aviso = r.aviso();
        assert!(aviso.contains("informada por Maria Silva em 2026-11-17 (e-mail)"));
        assert!(aviso.contains("sem evidência no código"));
        assert!(aviso.contains("A-0001/2, A-0002/1"));
    }

    #[test]
    fn resposta_achatada_em_json() {
        let r = RespostaInformada {
            texto: "Sim, vale para PJ.".into(),
            informacao: info(),
        };
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["por"], "Maria Silva");
        assert_eq!(v["em"], "2026-11-17");
        let de_volta: RespostaInformada = serde_json::from_value(v).unwrap();
        assert_eq!(de_volta, r);
    }
}
