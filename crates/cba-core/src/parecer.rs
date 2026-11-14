//! Parecer de revisão de um requisito ou história (fluxo `revisar`, #116).
//!
//! O texto revisado é a entrada da análise. O agente registra os achados (a
//! característica da ISO/IEC/IEEE 29148 violada, a severidade, o trecho do texto, o
//! problema e uma reescrita sugerida), a verificação INVEST, quando o texto é uma
//! história, e o veredito. O parecer fica no estado da análise
//! ([`crate::analise::Analise::parecer`]) e o documento é montado a partir dele.
//!
//! ```text
//! Achado:   registrado ◄──► descartado (pelo analista, com o motivo)
//! Veredito: o mais recente vale
//! ```
//!
//! O vocabulário (características, severidades e critérios INVEST) é de
//! [`crate::qualidade`].

use cba_kb::Id;
use serde::{Deserialize, Serialize};

use crate::analise::Evidencia;
use crate::qualidade::{Caracteristica, CriterioInvest, ResultadoInvest, Severidade};

/// Veredito da revisão.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Veredito {
    /// Pode ir para o desenvolvimento como está.
    Pronta,
    /// Pode ir, com os ajustes dos achados.
    ProntaComRessalvas,
    /// Precisa voltar ao autor antes do desenvolvimento.
    PrecisaDeRetrabalho,
}

impl Veredito {
    pub fn rotulo(self) -> &'static str {
        match self {
            Self::Pronta => "Pronta",
            Self::ProntaComRessalvas => "Pronta com ressalvas",
            Self::PrecisaDeRetrabalho => "Precisa de retrabalho",
        }
    }

    /// Veredito que os achados ativos indicam: algum de severidade alta pede
    /// retrabalho; os demais, ressalvas; nenhum, pronta.
    pub fn sugerido<'a>(achados: impl IntoIterator<Item = &'a Achado>) -> Self {
        achados
            .into_iter()
            .filter(|a| a.ativo())
            .map(|a| match a.severidade {
                Severidade::Alta => Self::PrecisaDeRetrabalho,
                Severidade::Media | Severidade::Baixa => Self::ProntaComRessalvas,
            })
            .max()
            .unwrap_or(Self::Pronta)
    }
}

/// Estado de um achado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "estado", rename_all = "kebab-case")]
pub enum EstadoAchado {
    /// Registrado pelo agente; vai para o parecer.
    Registrado,
    /// Descartado pelo analista: fica no estado e sai do parecer.
    Descartado {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        motivo: Option<String>,
    },
}

/// Achado da revisão (`registrar_achado`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Achado {
    pub numero: u32,
    pub caracteristica: Caracteristica,
    pub severidade: Severidade,
    /// Trecho do texto revisado, como está nele. Vazio só na completude, quando o
    /// achado aponta o que falta.
    #[serde(default)]
    pub trecho: String,
    /// O problema, em linguagem de negócio.
    pub problema: String,
    /// Reescrita sugerida do trecho, ou o que acrescentar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reescrita: Option<String>,
    /// Regras da base (`RN-…`) com que o texto se relaciona, conferidas no registro.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub regras: Vec<Id>,
    /// Trechos do código que sustentam o achado.
    #[serde(default)]
    pub evidencias: Vec<Evidencia>,
    #[serde(flatten)]
    pub estado: EstadoAchado,
    #[serde(default)]
    pub sessao: Option<String>,
    pub registrada_ms: u64,
}

impl Achado {
    /// Achado que vai para o parecer (não descartado).
    pub fn ativo(&self) -> bool {
        self.estado == EstadoAchado::Registrado
    }
}

/// Dados de um achado novo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NovoAchado {
    pub caracteristica: Caracteristica,
    pub severidade: Severidade,
    pub trecho: String,
    pub problema: String,
    pub reescrita: Option<String>,
    pub regras: Vec<Id>,
    pub evidencias: Vec<Evidencia>,
    pub sessao: Option<String>,
}

/// Avaliação de um critério INVEST.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AvaliacaoInvest {
    pub criterio: CriterioInvest,
    pub resultado: ResultadoInvest,
    pub justificativa: String,
}

/// Veredito registrado, com a justificativa.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VereditoRegistrado {
    pub veredito: Veredito,
    pub justificativa: String,
    pub registrada_ms: u64,
}

/// Parecer de revisão: o que o agente registrou sobre o texto revisado.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Parecer {
    #[serde(default)]
    pub achados: Vec<Achado>,
    /// Verificação INVEST, um item por critério (só para histórias).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub invest: Vec<AvaliacaoInvest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub veredito: Option<VereditoRegistrado>,
}

impl Parecer {
    /// Achados que vão para o parecer, na ordem de registro.
    pub fn ativos(&self) -> impl Iterator<Item = &Achado> {
        self.achados.iter().filter(|a| a.ativo())
    }

    /// Veredito indicado pelos achados ativos ([`Veredito::sugerido`]).
    pub fn veredito_sugerido(&self) -> Veredito {
        Veredito::sugerido(&self.achados)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn achado(numero: u32, severidade: Severidade, estado: EstadoAchado) -> Achado {
        Achado {
            numero,
            caracteristica: Caracteristica::Ambiguidade,
            severidade,
            trecho: "rápido".into(),
            problema: "Não diz o tempo.".into(),
            reescrita: None,
            regras: Vec::new(),
            evidencias: Vec::new(),
            estado,
            sessao: None,
            registrada_ms: 1,
        }
    }

    #[test]
    fn veredito_sugerido_pela_maior_severidade_ativa() {
        let mut p = Parecer::default();
        assert_eq!(p.veredito_sugerido(), Veredito::Pronta);
        p.achados
            .push(achado(1, Severidade::Baixa, EstadoAchado::Registrado));
        assert_eq!(p.veredito_sugerido(), Veredito::ProntaComRessalvas);
        p.achados.push(achado(
            2,
            Severidade::Alta,
            EstadoAchado::Descartado { motivo: None },
        ));
        assert_eq!(p.veredito_sugerido(), Veredito::ProntaComRessalvas);
        p.achados
            .push(achado(3, Severidade::Alta, EstadoAchado::Registrado));
        assert_eq!(p.veredito_sugerido(), Veredito::PrecisaDeRetrabalho);
        assert_eq!(p.ativos().count(), 2);
    }

    #[test]
    fn estado_do_achado_no_formato_do_arquivo() {
        let a = achado(
            1,
            Severidade::Media,
            EstadoAchado::Descartado {
                motivo: Some("O termo é do glossário.".into()),
            },
        );
        let v = serde_json::to_value(&a).unwrap();
        assert_eq!(v["estado"], "descartado");
        assert_eq!(v["motivo"], "O termo é do glossário.");
        assert_eq!(v["caracteristica"], "ambiguidade");
        assert_eq!(v["severidade"], "media");
        let lido: Achado = serde_json::from_value(v).unwrap();
        assert_eq!(lido, a);
        assert_eq!(
            serde_json::to_value(Veredito::PrecisaDeRetrabalho).unwrap(),
            "precisa_de_retrabalho"
        );
    }
}
