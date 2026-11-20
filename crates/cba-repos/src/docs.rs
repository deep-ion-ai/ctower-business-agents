//! Destino da documentação publicada (#274): onde o app grava os `.md` aprovados.
//!
//! Dois modos:
//!
//! - **repositório próprio** ([`DestinoDocs::Repositorio`]): um repositório só da
//!   documentação, com os mesmos campos de um repositório de código (forja, host,
//!   caminho, URL de clone, pasta local do clone, branch base e origem do token);
//! - **subpasta** ([`DestinoDocs::Subpasta`]): uma pasta (`docs/negocio`) dentro de
//!   um repositório de código já cadastrado no projeto.
//!
//! [`DestinoDocs::resolver`] devolve, nos dois modos, o repositório onde se escreve,
//! a branch base, a subpasta dentro dele e a pasta local do clone
//! ([`DestinoResolvido`]). A pasta da documentação não é código-fonte: fica fora do
//! aviso de alteração e do escopo das evidências.

use crate::RepositorioCodigo;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Onde a documentação do projeto é publicada.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "modo", rename_all = "snake_case")]
pub enum DestinoDocs {
    /// Repositório só da documentação. A `branch` do repositório é a branch base
    /// (vazia = a padrão) e a `pasta`, o clone local.
    Repositorio {
        repositorio: RepositorioCodigo,
        /// Subpasta dentro do repositório (vazia = a raiz).
        #[serde(default, skip_serializing_if = "String::is_empty")]
        subpasta: String,
    },
    /// Subpasta dentro de um repositório de código do projeto.
    Subpasta {
        /// O repositório de código, pela [`RepositorioCodigo::chave`]
        /// (`host/caminho` em minúsculas).
        #[serde(default)]
        codigo: String,
        /// Relativa à raiz do repositório, com `/` (`docs/negocio`).
        #[serde(default)]
        subpasta: String,
    },
}

/// O destino com o repositório de escrita já encontrado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DestinoResolvido<'a> {
    /// Onde se escreve: o repositório próprio ou o de código.
    pub repositorio: &'a RepositorioCodigo,
    /// Subpasta normalizada (com `/`, sem barras nas pontas); vazia = a raiz.
    pub subpasta: &'a str,
    /// Repositório próprio da documentação (e não uma pasta do código).
    pub independente: bool,
}

impl DestinoResolvido<'_> {
    /// Branch base do PR/MR; `None` = a branch padrão do repositório.
    pub fn branch_base(&self) -> Option<&str> {
        Some(self.repositorio.branch.trim()).filter(|b| !b.is_empty())
    }

    /// Pasta local do clone do repositório, se houver.
    pub fn pasta_local(&self) -> Option<&Path> {
        self.repositorio.pasta.as_deref()
    }

    /// Pasta local da documentação: o clone mais a subpasta.
    pub fn pasta_docs_local(&self) -> Option<PathBuf> {
        self.pasta_local().map(|p| juntar(p, self.subpasta))
    }

    /// Caminho, relativo à raiz do repositório, de um arquivo relativo à pasta da
    /// documentação (`regras/cancelamento.md` → `docs/negocio/regras/cancelamento.md`).
    pub fn caminho_no_repositorio(&self, relativo: &str) -> String {
        let relativo = relativo.replace('\\', "/");
        let relativo = relativo.trim_matches('/');
        match (self.subpasta.is_empty(), relativo.is_empty()) {
            (true, _) => relativo.to_string(),
            (false, true) => self.subpasta.to_string(),
            (false, false) => format!("{}/{relativo}", self.subpasta),
        }
    }
}

/// `base` mais a subpasta com `/`, componente a componente.
pub fn juntar(base: &Path, subpasta: &str) -> PathBuf {
    subpasta
        .split('/')
        .filter(|p| !p.is_empty())
        .fold(base.to_path_buf(), |c, p| c.join(p))
}

impl DestinoDocs {
    /// A subpasta como gravada.
    pub fn subpasta(&self) -> &str {
        match self {
            DestinoDocs::Repositorio { subpasta, .. } | DestinoDocs::Subpasta { subpasta, .. } => {
                subpasta
            }
        }
    }

    /// O repositório próprio da documentação (só no modo repositório).
    pub fn repositorio_proprio(&self) -> Option<&RepositorioCodigo> {
        match self {
            DestinoDocs::Repositorio { repositorio, .. } => Some(repositorio),
            DestinoDocs::Subpasta { .. } => None,
        }
    }

    /// O destino com o repositório de escrita, entre os de `codigo` no modo
    /// subpasta. `None` se o repositório de código não está mais cadastrado ou a
    /// subpasta é inválida (o cadastro validado não chega aqui assim).
    pub fn resolver<'a>(&'a self, codigo: &'a [RepositorioCodigo]) -> Option<DestinoResolvido<'a>> {
        let subpasta = self.subpasta().trim().trim_matches('/');
        normalizar_subpasta(subpasta)
            .ok()
            .filter(|s| s == subpasta)?;
        let (repositorio, independente) = match self {
            DestinoDocs::Repositorio { repositorio, .. } => (repositorio, true),
            DestinoDocs::Subpasta { codigo: chave, .. } => {
                let chave = chave.trim().to_ascii_lowercase();
                (codigo.iter().find(|r| r.chave() == chave)?, false)
            }
        };
        Some(DestinoResolvido {
            repositorio,
            subpasta,
            independente,
        })
    }

    /// Nome curto para mensagens: `docs/negocio em GitHub dono/repo`.
    pub fn rotulo(&self, codigo: &[RepositorioCodigo]) -> String {
        let onde = match self {
            DestinoDocs::Repositorio { repositorio, .. } => repositorio.rotulo(),
            DestinoDocs::Subpasta { codigo: chave, .. } => codigo
                .iter()
                .find(|r| r.chave() == chave.trim().to_ascii_lowercase())
                .map_or_else(|| chave.trim().to_string(), RepositorioCodigo::rotulo),
        };
        let subpasta = self.subpasta().trim().trim_matches('/');
        if subpasta.is_empty() {
            onde
        } else {
            format!("{subpasta} em {onde}")
        }
    }

    /// A mesma forma com a subpasta normalizada (`\` vira `/`, sem barras nas
    /// pontas) e a chave do repositório de código em minúsculas. Não valida.
    pub fn normalizado(&self) -> DestinoDocs {
        let sub = |s: &str| normalizar_subpasta(s).unwrap_or_else(|_| s.to_string());
        match self {
            DestinoDocs::Repositorio {
                repositorio,
                subpasta,
            } => DestinoDocs::Repositorio {
                repositorio: repositorio.clone(),
                subpasta: sub(subpasta),
            },
            DestinoDocs::Subpasta { codigo, subpasta } => DestinoDocs::Subpasta {
                codigo: codigo.trim().to_ascii_lowercase(),
                subpasta: sub(subpasta),
            },
        }
    }

    /// Confere o destino sem rede: no modo subpasta, o repositório de código
    /// existe e a subpasta é relativa e informada; no modo repositório, os campos
    /// do repositório valem e ele não é um dos de código (aí o modo é subpasta).
    pub fn validar(&self, codigo: &[RepositorioCodigo]) -> Result<(), String> {
        let onde = "destino da documentação";
        let subpasta = normalizar_subpasta(self.subpasta()).map_err(|e| format!("{onde}: {e}"))?;
        match self {
            DestinoDocs::Repositorio { repositorio, .. } => {
                repositorio.validar_como("repositório da documentação")?;
                if let Some(c) = codigo.iter().find(|c| c.chave() == repositorio.chave()) {
                    return Err(format!(
                        "{onde}: {} já é um repositório de código do projeto; escolha \
                         \"pasta num repositório de código\"",
                        c.rotulo()
                    ));
                }
            }
            DestinoDocs::Subpasta { codigo: chave, .. } => {
                if chave.trim().is_empty() {
                    return Err(format!("{onde}: escolha o repositório de código"));
                }
                let chave = chave.trim().to_ascii_lowercase();
                if !codigo.iter().any(|r| r.chave() == chave) {
                    return Err(format!(
                        "{onde}: o repositório de código {chave} não está cadastrado no projeto"
                    ));
                }
                if subpasta.is_empty() {
                    return Err(format!(
                        "{onde}: informe a subpasta dentro do repositório (ex.: docs/negocio); \
                         a raiz é do código"
                    ));
                }
            }
        }
        Ok(())
    }
}

/// Normaliza a subpasta da documentação: `\` vira `/` e as barras das pontas
/// saem. Recusa caminho absoluto, unidade do Windows, componentes `.`, `..` ou
/// vazios e a pasta `.git`. Vazia continua vazia (a raiz).
pub fn normalizar_subpasta(subpasta: &str) -> Result<String, String> {
    let s = subpasta.trim().replace('\\', "/");
    if s.starts_with('/') || s.contains(':') {
        return Err(format!(
            "a subpasta é relativa à raiz do repositório, sem / no início nem unidade: {subpasta:?}"
        ));
    }
    let s = s.trim_end_matches('/');
    if s.is_empty() {
        return Ok(String::new());
    }
    for parte in s.split('/') {
        if parte.is_empty() || parte == "." || parte == ".." {
            return Err(format!(
                "subpasta inválida: {subpasta:?} (sem .., . nem barras repetidas)"
            ));
        }
        if parte.eq_ignore_ascii_case(".git") {
            return Err(format!(
                "subpasta inválida: {subpasta:?} (a pasta .git é do git)"
            ));
        }
    }
    Ok(s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ForgeKind, OrigemToken};
    use serde_json::json;

    /// Caminho absoluto em qualquer sistema (no Windows, `/x` não é absoluto).
    fn abs(p: &str) -> PathBuf {
        if cfg!(windows) {
            PathBuf::from(format!("C:{p}"))
        } else {
            PathBuf::from(p)
        }
    }

    fn codigo() -> Vec<RepositorioCodigo> {
        vec![
            RepositorioCodigo {
                caminho: "exemplo/faturamento".into(),
                url_clone: "https://github.com/exemplo/faturamento.git".into(),
                pasta: Some(abs("/repos/faturamento")),
                branch: "develop".into(),
                ..Default::default()
            },
            RepositorioCodigo {
                forja: ForgeKind::Gitlab,
                host: "gitlab.example.com".into(),
                caminho: "fin/faturamento-web".into(),
                ..Default::default()
            },
        ]
    }

    fn proprio() -> RepositorioCodigo {
        RepositorioCodigo {
            forja: ForgeKind::Gitlab,
            host: "gitlab.example.com".into(),
            caminho: "fin/faturamento-docs".into(),
            url_clone: "git@gitlab.example.com:fin/faturamento-docs.git".into(),
            pasta: Some(abs("/repos/faturamento-docs")),
            branch: "main".into(),
            token: OrigemToken::Proprio,
            ..Default::default()
        }
    }

    #[test]
    fn normaliza_e_recusa_subpastas() {
        assert_eq!(
            normalizar_subpasta(" docs/negocio/ ").unwrap(),
            "docs/negocio"
        );
        assert_eq!(
            normalizar_subpasta("docs\\negocio").unwrap(),
            "docs/negocio"
        );
        assert_eq!(normalizar_subpasta("").unwrap(), "");
        for ruim in [
            "/docs",
            "\\docs",
            "C:/docs",
            "c:docs",
            "../docs",
            "docs/../x",
            "docs/./x",
            "docs//x",
            ".git",
            "docs/.GIT/x",
        ] {
            assert!(normalizar_subpasta(ruim).is_err(), "{ruim:?}");
        }
    }

    #[test]
    fn subpasta_num_repositorio_de_codigo() {
        let c = codigo();
        let d = DestinoDocs::Subpasta {
            codigo: "GitHub.com/Exemplo/Faturamento".into(),
            subpasta: "docs/negocio".into(),
        };
        d.validar(&c).unwrap();
        let r = d.resolver(&c).unwrap();
        assert!(!r.independente);
        assert_eq!(r.repositorio.caminho, "exemplo/faturamento");
        assert_eq!(r.subpasta, "docs/negocio");
        assert_eq!(r.branch_base(), Some("develop"));
        assert_eq!(r.pasta_local(), Some(abs("/repos/faturamento").as_path()));
        assert_eq!(
            r.pasta_docs_local().unwrap(),
            abs("/repos/faturamento").join("docs").join("negocio")
        );
        assert_eq!(
            r.caminho_no_repositorio("regras\\cancelamento.md"),
            "docs/negocio/regras/cancelamento.md"
        );
        assert_eq!(d.rotulo(&c), "docs/negocio em GitHub exemplo/faturamento");
        // Sem pasta local no repositório de código: sem pasta da documentação.
        let web = DestinoDocs::Subpasta {
            codigo: "gitlab.example.com/fin/faturamento-web".into(),
            subpasta: "docs".into(),
        };
        let r = web.resolver(&c).unwrap();
        assert_eq!((r.pasta_local(), r.branch_base()), (None, None));
    }

    #[test]
    fn subpasta_recusa_repositorio_ausente_e_raiz() {
        let c = codigo();
        let sem_repo = DestinoDocs::Subpasta {
            codigo: "github.com/exemplo/outro".into(),
            subpasta: "docs".into(),
        };
        assert!(
            sem_repo
                .validar(&c)
                .unwrap_err()
                .contains("não está cadastrado")
        );
        assert!(sem_repo.resolver(&c).is_none());
        let vazio = DestinoDocs::Subpasta {
            codigo: "".into(),
            subpasta: "docs".into(),
        };
        assert!(vazio.validar(&c).is_err());
        let raiz = DestinoDocs::Subpasta {
            codigo: c[0].chave(),
            subpasta: " / ".into(),
        };
        assert!(raiz.validar(&c).unwrap_err().contains("subpasta"));
        let fora = DestinoDocs::Subpasta {
            codigo: c[0].chave(),
            subpasta: "../docs".into(),
        };
        assert!(fora.validar(&c).is_err());
        assert!(fora.resolver(&c).is_none());
    }

    #[test]
    fn repositorio_proprio() {
        let c = codigo();
        let d = DestinoDocs::Repositorio {
            repositorio: proprio(),
            subpasta: String::new(),
        };
        d.validar(&c).unwrap();
        let r = d.resolver(&c).unwrap();
        assert!(r.independente);
        assert_eq!(r.branch_base(), Some("main"));
        assert_eq!(
            r.pasta_docs_local().unwrap(),
            abs("/repos/faturamento-docs")
        );
        assert_eq!(r.caminho_no_repositorio("a.md"), "a.md");
        assert_eq!(d.repositorio_proprio(), Some(&proprio()));
        // Com subpasta.
        let com_sub = DestinoDocs::Repositorio {
            repositorio: proprio(),
            subpasta: "negocio".into(),
        };
        assert_eq!(
            com_sub.resolver(&c).unwrap().caminho_no_repositorio(""),
            "negocio"
        );
        // Os campos do repositório valem como os de código.
        let ruim = DestinoDocs::Repositorio {
            repositorio: RepositorioCodigo {
                caminho: "app".into(),
                ..proprio()
            },
            subpasta: String::new(),
        };
        assert!(ruim.validar(&c).unwrap_err().contains("documentação"));
        // Um repositório de código não é destino próprio: é o modo subpasta.
        let repetido = DestinoDocs::Repositorio {
            repositorio: c[1].clone(),
            subpasta: "docs".into(),
        };
        assert!(
            repetido
                .validar(&c)
                .unwrap_err()
                .contains("pasta num repositório")
        );
    }

    #[test]
    fn serializa_marcado_por_modo() {
        let d = DestinoDocs::Subpasta {
            codigo: "github.com/exemplo/faturamento".into(),
            subpasta: "docs/negocio".into(),
        };
        let v = serde_json::to_value(&d).unwrap();
        assert_eq!(
            v,
            json!({
                "modo": "subpasta",
                "codigo": "github.com/exemplo/faturamento",
                "subpasta": "docs/negocio"
            })
        );
        assert_eq!(serde_json::from_value::<DestinoDocs>(v).unwrap(), d);
        let p = DestinoDocs::Repositorio {
            repositorio: proprio(),
            subpasta: String::new(),
        };
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["modo"], "repositorio");
        assert_eq!(v["repositorio"]["caminho"], "fin/faturamento-docs");
        assert!(v.get("subpasta").is_none());
        assert_eq!(serde_json::from_value::<DestinoDocs>(v).unwrap(), p);
    }

    #[test]
    fn normalizado_ajusta_subpasta_e_chave() {
        let d = DestinoDocs::Subpasta {
            codigo: " GitHub.com/Exemplo/Faturamento ".into(),
            subpasta: "docs\\negocio\\".into(),
        };
        assert_eq!(
            d.normalizado(),
            DestinoDocs::Subpasta {
                codigo: "github.com/exemplo/faturamento".into(),
                subpasta: "docs/negocio".into(),
            }
        );
    }
}
