//! Referências citadas pelos itens de requisito (critérios de aceite, #115; e,
//! depois, cenários de teste, #117), conferidas antes do registro.
//!
//! O agente cita o que sustenta um item de duas formas:
//!
//! - `afirmacoes`: o número da afirmação na análise de origem (`4`) ou a
//!   referência completa a uma afirmação de qualquer análise do projeto
//!   (`"A-0193/4"`);
//! - `regras`: o identificador de uma regra da base (`"RN-FAT-007"`).
//!
//! E, se o item depender de uma pergunta ainda em aberto, `pergunta`, no mesmo
//! formato das afirmações (o número na análise de origem ou `"A-NNNN/n"`).
//!
//! [`Fundamentacao`] confere cada referência (a análise existe, a afirmação existe
//! e não foi marcada como incorreta, a regra existe na base e não está obsoleta, a
//! pergunta está em aberto) e devolve os [`Fundamento`]s com a cópia do texto e das
//! evidências, para o documento. Referência que não confere é recusada com o
//! motivo, e o item sem nenhuma referência também ([`Fundamentacao::fundamentos`]).

use std::collections::HashMap;

use cba_core::ambiente::Ambiente;
use cba_core::analise::{Analise, EstadoAfirmacao, EstadoPergunta};
use cba_core::requisito::{Fundamento, RefItem};
use cba_kb::frontmatter::{Evidencia, FrontmatterRegra, StatusRegra};
use cba_kb::{Documento, Id, Serie};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::registro::ErroFerramenta;

/// Referência a um item de análise como o agente cita: o número na análise de
/// origem ou `A-NNNN/n`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum RefCitada {
    Numero(u32),
    Texto(String),
}

/// O que o agente citou para sustentar um item.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct FundamentosCitados {
    #[serde(default)]
    pub afirmacoes: Vec<RefCitada>,
    #[serde(default)]
    pub regras: Vec<String>,
}

/// Schema de `afirmacoes`.
pub fn schema_das_afirmacoes() -> Value {
    json!({
        "type": "array",
        "maxItems": 20,
        "items": { "type": ["integer", "string"], "minimum": 1, "minLength": 1, "maxLength": 40 },
        "description": "Afirmações que sustentam o item: o número da afirmação na análise de origem (4) ou A-NNNN/n para outra análise do projeto (\"A-0193/4\")."
    })
}

/// Schema de `regras`.
pub fn schema_das_regras() -> Value {
    json!({
        "type": "array",
        "maxItems": 20,
        "items": { "type": "string", "minLength": 1, "maxLength": 40 },
        "description": "Regras de negócio da base que sustentam o item (RN-<SIGLA>-NNN, como RN-FAT-007)."
    })
}

/// Schema de `pergunta`.
pub fn schema_da_pergunta() -> Value {
    json!({
        "type": ["integer", "string"],
        "minimum": 1,
        "minLength": 1,
        "maxLength": 40,
        "description": "Pergunta ainda em aberto de que o item depende: o número na análise de origem (2) ou A-NNNN/n. Com ela, o item fica \"a confirmar\"."
    })
}

fn recusa(texto: String) -> ErroFerramenta {
    ErroFerramenta::Recusa(texto)
}

/// Evidência da afirmação como fica na cópia do fundamento (sem o trecho).
fn evidencia(e: &cba_core::analise::Evidencia) -> Evidencia {
    Evidencia {
        arquivo: e.arquivo.clone(),
        linhas: e.linhas,
        hash: e.hash.clone(),
        commit: e.commit.clone(),
    }
}

/// Confere as referências de uma análise de requisito (`analise`), com as
/// análises citadas lidas uma vez.
pub struct Fundamentacao<'a> {
    ambiente: &'a Ambiente,
    analise: &'a Analise,
    lidas: HashMap<Id, Analise>,
}

impl<'a> Fundamentacao<'a> {
    pub fn nova(ambiente: &'a Ambiente, analise: &'a Analise) -> Self {
        Self {
            ambiente,
            analise,
            lidas: HashMap::new(),
        }
    }

    /// Referência completa: o número vale na análise de origem.
    pub fn referencia(&self, citada: &RefCitada, campo: &str) -> Result<RefItem, ErroFerramenta> {
        match citada {
            RefCitada::Numero(n) => {
                let origem = self.analise.origem.clone().ok_or_else(|| {
                    ErroFerramenta::Argumentos(format!(
                        "`{campo}`: a análise {} não tem análise de origem; cite o item como \
                         A-NNNN/n (\"A-0193/4\")",
                        self.analise.id
                    ))
                })?;
                Ok(RefItem::nova(origem, (*n).max(1)))
            }
            RefCitada::Texto(t) => {
                let t = t.trim();
                if let Ok(n) = t.parse::<u32>() {
                    return self.referencia(&RefCitada::Numero(n), campo);
                }
                RefItem::analisar(t)
                    .map_err(|e| ErroFerramenta::Argumentos(format!("`{campo}`: {e}")))
            }
        }
    }

    fn analise_citada(&mut self, id: &Id) -> Result<&Analise, ErroFerramenta> {
        if id == &self.analise.id {
            return Ok(self.analise);
        }
        if !self.lidas.contains_key(id) {
            let lida = self.ambiente.analises.carregar(id)?;
            self.lidas.insert(id.clone(), lida);
        }
        Ok(&self.lidas[id])
    }

    /// A afirmação citada, conferida.
    pub fn afirmacao(&mut self, citada: &RefCitada) -> Result<Fundamento, ErroFerramenta> {
        let r = self.referencia(citada, "afirmacoes")?;
        let analise = self.analise_citada(&r.analise)?;
        let a = analise
            .afirmacoes
            .iter()
            .find(|a| a.numero == r.numero)
            .ok_or_else(|| {
                recusa(format!(
                    "a afirmação {r} não existe: a análise {} tem {} afirmação(ões). Confira o \
                     número com `retomar_analise` (analise: \"{}\")",
                    r.analise,
                    analise.afirmacoes.len(),
                    r.analise
                ))
            })?;
        if let EstadoAfirmacao::Incorreta { comentario } = &a.estado {
            return Err(recusa(format!(
                "a afirmação {r} foi marcada como incorreta pelo analista ({comentario}) e não \
                 sustenta um requisito; cite outra afirmação ou uma regra"
            )));
        }
        Ok(Fundamento::Afirmacao {
            referencia: r,
            texto: a.texto.clone(),
            evidencias: a.evidencias.iter().map(evidencia).collect(),
        })
    }

    /// A regra citada, conferida na base.
    pub fn regra(&mut self, texto: &str) -> Result<Fundamento, ErroFerramenta> {
        let texto = texto.trim();
        let id = Id::analisar(texto)
            .ok()
            .filter(|id| matches!(id.serie(), Serie::Regra { .. }))
            .ok_or_else(|| {
                ErroFerramenta::Argumentos(format!(
                    "`regras`: `{texto}` não é o identificador de uma regra; use \
                     RN-<SIGLA>-NNN, como RN-FAT-007"
                ))
            })?;
        let arquivo = id.arquivo();
        if !self.ambiente.kb.existe(&arquivo) {
            return Err(recusa(format!(
                "a regra {id} não existe na base deste projeto. Use `conhecimento` (tipo \
                 regra) para achar a regra, ou cite a afirmação de origem em `afirmacoes`"
            )));
        }
        let doc: Documento<FrontmatterRegra> = self
            .ambiente
            .kb
            .ler(&arquivo)
            .map_err(|e| ErroFerramenta::Interno(e.to_string()))?;
        if doc.frontmatter.status == StatusRegra::Obsoleta {
            return Err(recusa(format!(
                "a regra {id} está obsoleta e não sustenta um requisito; cite a regra que a \
                 substituiu ou a afirmação de origem"
            )));
        }
        Ok(Fundamento::Regra {
            id,
            titulo: doc.frontmatter.titulo,
            evidencias: doc.frontmatter.evidencias,
        })
    }

    /// Todas as referências citadas, conferidas, na ordem (afirmações, depois
    /// regras). Recusa o item sem nenhuma.
    pub fn fundamentos(
        &mut self,
        citados: &FundamentosCitados,
    ) -> Result<Vec<Fundamento>, ErroFerramenta> {
        if citados.afirmacoes.is_empty() && citados.regras.is_empty() {
            return Err(recusa(
                cba_core::analise::ErroModelo::CriterioSemFundamento.to_string(),
            ));
        }
        let mut lista = Vec::new();
        for a in &citados.afirmacoes {
            lista.push(self.afirmacao(a)?);
        }
        for r in &citados.regras {
            lista.push(self.regra(r)?);
        }
        Ok(cba_core::requisito::sem_repetidos(lista))
    }

    /// A pergunta citada, que precisa estar em aberto.
    pub fn pergunta(&mut self, citada: &RefCitada) -> Result<RefItem, ErroFerramenta> {
        let r = self.referencia(citada, "pergunta")?;
        let propria = self.analise.id.clone();
        let analise = self.analise_citada(&r.analise)?;
        let p = analise
            .perguntas
            .iter()
            .find(|p| p.numero == r.numero)
            .ok_or_else(|| {
                recusa(format!(
                    "a pergunta {r} não existe. Confira o número com `retomar_analise` \
                     (analise: \"{}\"), ou registre a pergunta com `perguntar` nesta análise \
                     e cite-a como {}/<número>",
                    r.analise, propria
                ))
            })?;
        match &p.estado {
            EstadoPergunta::Aberta => Ok(r),
            EstadoPergunta::Respondida { resposta } => Err(recusa(format!(
                "a pergunta {r} já foi respondida (\"{resposta}\"); use a resposta no item, \
                 que não fica mais a confirmar"
            ))),
            EstadoPergunta::Descartada => Err(recusa(format!(
                "a pergunta {r} foi descartada pelo analista; o item não depende mais dela"
            ))),
        }
    }
}
