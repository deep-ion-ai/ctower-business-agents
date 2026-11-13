//! Catálogo de papéis das pessoas nas reuniões (épico #93), em
//! `<base>/_templates/papeis.yaml`.
//!
//! Segue o padrão dos templates editáveis da base (`cba_docs::Templates` e os
//! roteiros de `cba-tools`):
//!
//! - o catálogo padrão é embutido no app ([`PADRAO_PAPEIS`]);
//! - [`Papeis::copiar_padrao`] grava a cópia editável em `_templates/` sem
//!   sobrescrever o que o analista editou;
//! - [`Papeis::da_base`] usa o arquivo da base quando existe; ilegível ou inválido,
//!   vale o padrão, com um [`AvisoPapeis`].
//!
//! O catálogo é uma sugestão: o papel de uma pessoa aceita texto livre
//! ([`Papeis::canonico`] só troca a grafia pela do catálogo quando casa).

use std::fmt;
use std::io;

use serde::{Deserialize, Serialize};

use crate::arquivo;
use crate::base::Base;
use crate::caminho::CaminhoRelativo;
use crate::erro::{Erro, Resultado};
use crate::frontmatter::{MAXIMO_DE_PALAVRAS_DO_PAPEL, MAXIMO_DO_PAPEL, conferir_texto_curto};
use crate::layout;
use crate::pessoas::normalizar_texto;

/// Conteúdo padrão de `_templates/papeis.yaml`: PO, gerente/patrocinador, analista
/// de negócio, usuário-chave, desenvolvedor, arquiteto, QA e cliente externo.
pub const PADRAO_PAPEIS: &str = include_str!("../modelos/papeis.yaml");

/// Um papel do catálogo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Papel {
    /// Nome do papel, gravado no catálogo de pessoas (`PO`, `analista de negócio`).
    pub nome: String,
    /// Explicação para a tela de participantes.
    #[serde(default)]
    pub descricao: Option<String>,
}

/// De onde veio o catálogo de papéis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrigemPapeis {
    /// Catálogo padrão embutido no app.
    Padrao,
    /// Arquivo da base, relativo à raiz da base (`_templates/papeis.yaml`).
    Base(CaminhoRelativo),
}

/// O `_templates/papeis.yaml` da base não pôde ser usado; vale o padrão.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvisoPapeis {
    pub arquivo: CaminhoRelativo,
    pub erro: String,
}

impl fmt::Display for AvisoPapeis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: catálogo de papéis inválido ({}); usado o padrão",
            self.arquivo, self.erro
        )
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Arquivo {
    #[serde(default)]
    papeis: Vec<Papel>,
}

/// Catálogo de papéis: o padrão embutido ou o da base.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Papeis {
    papeis: Vec<Papel>,
    origem: OrigemPapeis,
}

impl Papeis {
    /// Catálogo padrão embutido no app.
    pub fn padrao() -> Self {
        Self::analisar(PADRAO_PAPEIS, OrigemPapeis::Padrao).expect("catálogo padrão válido")
    }

    /// Lê um catálogo no formato de `papeis.yaml`. Recusa papel sem nome, nome
    /// longo demais ou repetido (pela grafia normalizada).
    pub fn analisar(texto: &str, origem: OrigemPapeis) -> Result<Self, String> {
        let arquivo: Arquivo = serde_norway::from_str(texto).map_err(|e| e.to_string())?;
        let mut papeis: Vec<Papel> = Vec::with_capacity(arquivo.papeis.len());
        for (i, mut p) in arquivo.papeis.into_iter().enumerate() {
            p.nome = p.nome.split_whitespace().collect::<Vec<_>>().join(" ");
            conferir_texto_curto(
                &format!("papeis[{i}].nome"),
                &p.nome,
                MAXIMO_DO_PAPEL,
                MAXIMO_DE_PALAVRAS_DO_PAPEL,
            )
            .map_err(|e| format!("{}: {}", e.campo, e.mensagem))?;
            let chave = normalizar_texto(&p.nome);
            if papeis.iter().any(|o| normalizar_texto(&o.nome) == chave) {
                return Err(format!("papeis[{i}].nome: `{}` repetido", p.nome));
            }
            p.descricao = p
                .descricao
                .map(|d| d.trim().to_owned())
                .filter(|d| !d.is_empty());
            papeis.push(p);
        }
        Ok(Self { papeis, origem })
    }

    /// Catálogo da base (`_templates/papeis.yaml`) quando existe e é válido; senão,
    /// o padrão, com o aviso do problema.
    pub fn da_base(base: &Base) -> (Self, Option<AvisoPapeis>) {
        let relativo = layout::papeis();
        if !base.caminho(&relativo).is_file() {
            return (Self::padrao(), None);
        }
        let lido = base
            .ler_texto(&relativo)
            .map_err(|e| e.to_string())
            .and_then(|texto| Self::analisar(&texto, OrigemPapeis::Base(relativo.clone())));
        match lido {
            Ok(papeis) => (papeis, None),
            Err(erro) => (
                Self::padrao(),
                Some(AvisoPapeis {
                    arquivo: relativo,
                    erro,
                }),
            ),
        }
    }

    /// Grava em `_templates/papeis.yaml` a cópia editável do catálogo padrão, se o
    /// arquivo ainda não existir. Nunca sobrescreve o que o analista editou.
    /// Devolve o arquivo criado, relativo à raiz da base.
    pub fn copiar_padrao(base: &Base) -> Resultado<Option<CaminhoRelativo>> {
        let relativo = layout::papeis();
        match arquivo::criar(&base.caminho(&relativo), PADRAO_PAPEIS.as_bytes()) {
            Ok(()) => Ok(Some(relativo)),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Ok(None),
            Err(erro) => Err(Erro::Io {
                arquivo: relativo,
                erro,
            }),
        }
    }

    /// Papéis do catálogo, na ordem do arquivo.
    pub fn papeis(&self) -> &[Papel] {
        &self.papeis
    }

    pub fn origem(&self) -> &OrigemPapeis {
        &self.origem
    }

    /// Papel do catálogo com a mesma grafia normalizada (sem acento nem caixa):
    /// `"Analista de Negocio"` casa com `analista de negócio`.
    pub fn buscar(&self, texto: &str) -> Option<&Papel> {
        let chave = normalizar_texto(texto);
        self.papeis
            .iter()
            .find(|p| normalizar_texto(&p.nome) == chave)
    }

    /// Grafia do catálogo quando o texto casa com um papel; senão, o texto livre
    /// com os espaços normalizados.
    pub fn canonico(&self, texto: &str) -> String {
        match self.buscar(texto) {
            Some(p) => p.nome.clone(),
            None => texto.split_whitespace().collect::<Vec<_>>().join(" "),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn padrao_tem_os_papeis_do_plano() {
        let nomes: Vec<_> = Papeis::padrao()
            .papeis()
            .iter()
            .map(|p| p.nome.clone())
            .collect();
        assert_eq!(
            nomes,
            [
                "PO",
                "gerente/patrocinador",
                "analista de negócio",
                "usuário-chave",
                "desenvolvedor",
                "arquiteto",
                "QA",
                "cliente externo"
            ]
        );
    }

    #[test]
    fn canonico_casa_sem_acento_e_aceita_texto_livre() {
        let p = Papeis::padrao();
        assert_eq!(p.canonico("Analista de Negocio"), "analista de negócio");
        assert_eq!(p.canonico("po"), "PO");
        assert_eq!(p.canonico("Gerente / Patrocinador"), "gerente/patrocinador");
        assert_eq!(p.canonico("  dono   do processo "), "dono do processo");
        assert!(p.buscar("dono do processo").is_none());
    }

    #[test]
    fn recusa_catalogo_invalido() {
        for ruim in [
            "papeis:\n  - nome: PO\n  - nome: po\n",
            "papeis:\n  - nome: \"\"\n",
            "papeis:\n  - nome: um papel com palavras demais para caber\n",
            "outra: coisa\n",
            "papeis: [",
        ] {
            assert!(
                Papeis::analisar(ruim, OrigemPapeis::Padrao).is_err(),
                "{ruim}"
            );
        }
        let vazio = Papeis::analisar("papeis: []\n", OrigemPapeis::Padrao).unwrap();
        assert!(vazio.papeis().is_empty());
    }
}
