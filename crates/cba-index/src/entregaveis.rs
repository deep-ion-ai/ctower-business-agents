//! Entregáveis do projeto (#460): a metodologia e, por tipo habilitado, o
//! template e o destino.
//!
//! Aqui fica só o que o cadastro guarda, com os tipos pelo `id` em texto. O
//! catálogo dos tipos, as predefinições de cada metodologia e a validação contra
//! o catálogo ficam em `cba_docs::entregaveis`.

use serde::{Deserialize, Serialize};

/// Metodologia do projeto. Escolher Ágil ou RUP aplica a predefinição dela;
/// mudar um tipo à mão passa para [`Metodologia::Personalizado`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Metodologia {
    Agil,
    Rup,
    /// OpenSpec. Ainda não disponível: a tela mostra a opção desabilitada e o
    /// cadastro a recusa.
    OrientadoAgentes,
    #[default]
    Personalizado,
}

/// Para onde o entregável vai depois de revisado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Destino {
    /// Exportado em arquivo (Markdown, Word ou HTML).
    #[default]
    Exportar,
    /// Publicado no repositório de documentação do projeto.
    RepositorioDocs,
    /// Comentário na issue do rastreador.
    ComentarioRastreador,
}

/// Um tipo de entregável habilitado no projeto.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TipoHabilitado {
    /// `id` do tipo no catálogo (`caso-de-uso`).
    pub tipo: String,
    /// Arquivo em `_templates/` (só o nome, `caso-de-uso-empresa.md`) no lugar
    /// do template padrão do app. Ausente: vale o padrão.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
    #[serde(default)]
    pub destino: Destino,
}

impl TipoHabilitado {
    /// Tipo com o template padrão e o destino padrão.
    pub fn novo(tipo: &str) -> Self {
        Self {
            tipo: tipo.to_owned(),
            template: None,
            destino: Destino::default(),
        }
    }
}

/// Entregáveis do projeto como o cadastro guarda.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Entregaveis {
    #[serde(default)]
    pub metodologia: Metodologia,
    /// Tipos habilitados, na ordem do catálogo.
    #[serde(default)]
    pub tipos: Vec<TipoHabilitado>,
    /// Tipo oferecido primeiro ao gerar um entregável; um dos habilitados.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub padrao: Option<String>,
}

impl Entregaveis {
    /// Se o tipo está habilitado.
    pub fn habilitado(&self, tipo: &str) -> Option<&TipoHabilitado> {
        self.tipos.iter().find(|t| t.tipo == tipo)
    }

    /// `id`s dos tipos habilitados, na ordem gravada.
    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.tipos.iter().map(|t| t.tipo.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_em_snake_case_e_sem_campos_vazios() {
        let e = Entregaveis {
            metodologia: Metodologia::Rup,
            tipos: vec![
                TipoHabilitado::novo("caso-de-uso"),
                TipoHabilitado {
                    tipo: "glossario".into(),
                    template: Some("glossario-empresa.md".into()),
                    destino: Destino::RepositorioDocs,
                },
            ],
            padrao: Some("caso-de-uso".into()),
        };
        let json = serde_json::to_value(&e).unwrap();
        assert_eq!(json["metodologia"], "rup");
        assert_eq!(
            json["tipos"][0],
            serde_json::json!({"tipo": "caso-de-uso", "destino": "exportar"})
        );
        assert_eq!(json["tipos"][1]["destino"], "repositorio_docs");
        let lido: Entregaveis = serde_json::from_value(json).unwrap();
        assert_eq!(lido, e);
        assert_eq!(lido.ids().collect::<Vec<_>>(), ["caso-de-uso", "glossario"]);
        assert!(lido.habilitado("glossario").is_some());
        assert!(lido.habilitado("manual").is_none());
    }

    #[test]
    fn campos_ausentes_valem_o_padrao() {
        let e: Entregaveis = serde_json::from_str(r#"{"tipos":[{"tipo":"manual"}]}"#).unwrap();
        assert_eq!(e.metodologia, Metodologia::Personalizado);
        assert_eq!(e.tipos, [TipoHabilitado::novo("manual")]);
        assert_eq!(e.padrao, None);
    }
}
