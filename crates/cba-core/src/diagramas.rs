//! Diagramas registrados numa análise (#118).
//!
//! O agente não escreve Mermaid: registra os elementos do diagrama (nós e arestas,
//! ou estados e transições), cada um com as evidências do código, e o app gera o
//! Mermaid na montagem do documento (`cba_docs::mermaid`). Assim o diagrama é
//! padronizado por construção e cada elemento fica ligado ao trecho que o sustenta.
//!
//! # Modelo
//!
//! - [`Diagrama`] `D-n` (número dentro da análise): o tipo ([`TipoDiagrama`]), o
//!   título (a legenda da figura), a seção do documento em que entra, os nós e as
//!   arestas.
//! - [`No`]: chave dada pelo agente (única no diagrama, usada pelas arestas), o
//!   rótulo em linguagem de negócio, o papel ([`PapelNo`]: etapa, início, fim ou
//!   decisão), a descrição e as evidências.
//! - [`Aresta`]: de uma chave para outra, com o rótulo (o evento da transição ou a
//!   condição do caminho), a descrição, a confiança e as evidências. Aresta
//!   `inferido` ou `a_confirmar` aparece tracejada.
//!
//! Registrar de novo com `substituir` troca o diagrama inteiro, mantendo o número:
//! é como o agente corrige um diagrama.

use serde::{Deserialize, Serialize};

use crate::analise::{Analise, Confianca, ErroModelo, Evidencia};

/// Nós no máximo num diagrama: mais que isso não se lê numa página.
pub const MAXIMO_DE_NOS: usize = 40;
/// Arestas no máximo num diagrama.
pub const MAXIMO_DE_ARESTAS: usize = 80;
/// Tamanho máximo da chave de um nó.
pub const TAMANHO_DA_CHAVE: usize = 40;

/// O que o diagrama mostra.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TipoDiagrama {
    /// Ciclo de vida de uma entidade: estados e transições.
    Estados,
    /// Fluxo de um processo: passos, decisões e caminhos.
    Fluxo,
}

/// Papel de um nó no diagrama, que define a forma dele na figura.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PapelNo {
    /// Estado ou passo comum.
    #[default]
    Etapa,
    /// Estado inicial ou início do processo.
    Inicio,
    /// Estado final ou fim do processo.
    Fim,
    /// Ponto de decisão (os caminhos saem com a condição no rótulo).
    Decisao,
}

/// Nó do diagrama (estado ou passo).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct No {
    /// Chave dada pelo agente, única no diagrama (`E`, `emitida`).
    pub chave: String,
    /// Nome em linguagem de negócio ("Emitida").
    pub rotulo: String,
    #[serde(default)]
    pub papel: PapelNo,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub descricao: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidencias: Vec<Evidencia>,
}

/// Aresta do diagrama (transição ou caminho).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Aresta {
    /// Chave do nó de origem.
    pub de: String,
    /// Chave do nó de destino.
    pub para: String,
    /// Evento da transição ou condição do caminho ("pagar", "sim").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotulo: Option<String>,
    /// Condição e efeito, em linguagem de negócio.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub descricao: Option<String>,
    pub confianca: Confianca,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidencias: Vec<Evidencia>,
}

/// Diagrama `D-<numero>` registrado na análise.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagrama {
    pub numero: u32,
    pub tipo: TipoDiagrama,
    pub titulo: String,
    /// Marcador da seção do documento em que o diagrama entra.
    pub secao: String,
    pub nos: Vec<No>,
    #[serde(default)]
    pub arestas: Vec<Aresta>,
    #[serde(default)]
    pub sessao: Option<String>,
    pub registrada_ms: u64,
}

impl Diagrama {
    /// `D-1`.
    pub fn rotulo(&self) -> String {
        format!("D-{}", self.numero)
    }

    /// Nó pela chave.
    pub fn no(&self, chave: &str) -> Option<&No> {
        self.nos.iter().find(|n| n.chave == chave)
    }
}

/// Dados de um diagrama novo, como o agente registra.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NovoDiagrama {
    pub tipo: TipoDiagrama,
    pub titulo: String,
    pub secao: String,
    pub nos: Vec<No>,
    pub arestas: Vec<Aresta>,
    pub sessao: Option<String>,
}

fn invalido(motivo: impl Into<String>) -> ErroModelo {
    ErroModelo::DiagramaInvalido(motivo.into())
}

fn aparar(texto: Option<String>) -> Option<String> {
    texto.map(|t| t.trim().to_owned()).filter(|t| !t.is_empty())
}

impl NovoDiagrama {
    /// Confere e normaliza (textos sem espaços nas pontas, opcionais vazios
    /// viram `None`).
    fn conferido(mut self) -> Result<Self, ErroModelo> {
        self.titulo = self.titulo.trim().to_owned();
        self.secao = self.secao.trim().to_owned();
        if self.titulo.is_empty() {
            return Err(ErroModelo::CampoVazio("titulo"));
        }
        if self.secao.is_empty() {
            return Err(ErroModelo::CampoVazio("secao"));
        }
        if self.nos.is_empty() {
            return Err(invalido("o diagrama precisa de ao menos um nó"));
        }
        if self.nos.len() > MAXIMO_DE_NOS {
            return Err(invalido(format!(
                "{} nós passam do máximo de {MAXIMO_DE_NOS}; divida em diagramas menores",
                self.nos.len()
            )));
        }
        if self.arestas.len() > MAXIMO_DE_ARESTAS {
            return Err(invalido(format!(
                "{} arestas passam do máximo de {MAXIMO_DE_ARESTAS}; divida em diagramas \
                 menores",
                self.arestas.len()
            )));
        }
        let mut chaves: Vec<String> = Vec::new();
        for no in &mut self.nos {
            no.chave = no.chave.trim().to_owned();
            no.rotulo = no.rotulo.trim().to_owned();
            no.descricao = aparar(no.descricao.take());
            if no.chave.is_empty() {
                return Err(invalido("todo nó precisa de `chave`"));
            }
            if no.chave.chars().count() > TAMANHO_DA_CHAVE {
                return Err(invalido(format!(
                    "a chave `{}` passa de {TAMANHO_DA_CHAVE} caracteres",
                    no.chave
                )));
            }
            if no.rotulo.is_empty() {
                return Err(invalido(format!("o nó `{}` precisa de `rotulo`", no.chave)));
            }
            if chaves.contains(&no.chave) {
                return Err(invalido(format!(
                    "a chave `{}` aparece em mais de um nó; cada nó tem chave própria",
                    no.chave
                )));
            }
            chaves.push(no.chave.clone());
        }
        for (i, aresta) in self.arestas.iter_mut().enumerate() {
            aresta.de = aresta.de.trim().to_owned();
            aresta.para = aresta.para.trim().to_owned();
            aresta.rotulo = aparar(aresta.rotulo.take());
            aresta.descricao = aparar(aresta.descricao.take());
            for ponta in [&aresta.de, &aresta.para] {
                if !chaves.contains(ponta) {
                    return Err(invalido(format!(
                        "a aresta {} liga `{}` a `{}`, mas `{ponta}` não é a chave de nenhum \
                         nó; chaves registradas: {}",
                        i + 1,
                        aresta.de,
                        aresta.para,
                        chaves.join(", ")
                    )));
                }
            }
            if aresta.confianca == Confianca::Confirmado && aresta.evidencias.is_empty() {
                return Err(invalido(format!(
                    "a aresta {} (`{}` → `{}`) é `confirmado`, mas não tem evidência; cite o \
                     trecho do código ou use `inferido` ou `a_confirmar`",
                    i + 1,
                    aresta.de,
                    aresta.para
                )));
            }
        }
        Ok(self)
    }
}

impl Analise {
    /// Diagrama pelo número.
    pub fn diagrama(&self, numero: u32) -> Option<&Diagrama> {
        self.diagramas.iter().find(|d| d.numero == numero)
    }

    /// Registra um diagrama novo ou, com `substituir`, troca o diagrama daquele
    /// número (que mantém o número e a posição). Devolve o número.
    pub fn registrar_diagrama(
        &mut self,
        novo: NovoDiagrama,
        substituir: Option<u32>,
        agora_ms: u64,
    ) -> Result<u32, ErroModelo> {
        let novo = novo.conferido()?;
        let posicao = match substituir {
            Some(n) => Some(self.diagramas.iter().position(|d| d.numero == n).ok_or(
                ErroModelo::ItemDesconhecido {
                    tipo: "diagrama",
                    numero: n,
                },
            )?),
            None => None,
        };
        self.aceitar_registro()?;
        let numero = match posicao {
            Some(p) => self.diagramas[p].numero,
            None => self.diagramas.iter().map(|d| d.numero).max().unwrap_or(0) + 1,
        };
        let diagrama = Diagrama {
            numero,
            tipo: novo.tipo,
            titulo: novo.titulo,
            secao: novo.secao,
            nos: novo.nos,
            arestas: novo.arestas,
            sessao: novo.sessao,
            registrada_ms: agora_ms,
        };
        match posicao {
            Some(p) => self.diagramas[p] = diagrama,
            None => self.diagramas.push(diagrama),
        }
        Ok(numero)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analise::{EstadoAnalise, Fluxo};
    use cba_kb::Id;

    fn analise() -> Analise {
        Analise::nova(
            Id::analisar("A-0001").unwrap(),
            "faturamento",
            Fluxo::Explicar,
            "Ciclo de vida da fatura",
            "fatura",
            1,
        )
    }

    fn no(chave: &str, rotulo: &str) -> No {
        No {
            chave: chave.into(),
            rotulo: rotulo.into(),
            papel: PapelNo::Etapa,
            descricao: None,
            evidencias: vec![],
        }
    }

    fn aresta(de: &str, para: &str, confianca: Confianca) -> Aresta {
        Aresta {
            de: de.into(),
            para: para.into(),
            rotulo: Some(" emitir ".into()),
            descricao: Some("  ".into()),
            confianca,
            evidencias: vec![],
        }
    }

    fn novo(nos: Vec<No>, arestas: Vec<Aresta>) -> NovoDiagrama {
        NovoDiagrama {
            tipo: TipoDiagrama::Estados,
            titulo: " Ciclo de vida da fatura ".into(),
            secao: "fluxo_principal".into(),
            nos,
            arestas,
            sessao: None,
        }
    }

    #[test]
    fn registra_normaliza_e_substitui() {
        let mut a = analise();
        let n = a
            .registrar_diagrama(
                novo(
                    vec![no(" R ", "Rascunho"), no("E", "Emitida")],
                    vec![aresta("R", "E", Confianca::Inferido)],
                ),
                None,
                5,
            )
            .unwrap();
        assert_eq!(n, 1);
        assert_eq!(a.estado, EstadoAnalise::EmAndamento);
        let d = a.diagrama(1).unwrap();
        assert_eq!(d.rotulo(), "D-1");
        assert_eq!(d.titulo, "Ciclo de vida da fatura");
        assert_eq!(d.nos[0].chave, "R");
        assert_eq!(d.arestas[0].rotulo.as_deref(), Some("emitir"));
        assert_eq!(d.arestas[0].descricao, None);
        assert!(d.no("E").is_some());

        let n2 = a
            .registrar_diagrama(novo(vec![no("X", "X")], vec![]), None, 6)
            .unwrap();
        assert_eq!(n2, 2);
        // Substituir mantém o número e a posição.
        let n = a
            .registrar_diagrama(novo(vec![no("A", "Aberta")], vec![]), Some(1), 7)
            .unwrap();
        assert_eq!(n, 1);
        assert_eq!(a.diagramas[0].nos[0].rotulo, "Aberta");
        assert_eq!(a.diagramas[0].registrada_ms, 7);
        assert_eq!(
            a.registrar_diagrama(novo(vec![no("A", "A")], vec![]), Some(9), 8),
            Err(ErroModelo::ItemDesconhecido {
                tipo: "diagrama",
                numero: 9
            })
        );
    }

    #[test]
    fn recusa_diagrama_inconsistente() {
        let mut a = analise();
        let casos = [
            (novo(vec![], vec![]), "ao menos um nó"),
            (
                novo(vec![no("A", "a"), no("A", "b")], vec![]),
                "mais de um nó",
            ),
            (novo(vec![no("A", " ")], vec![]), "`rotulo`"),
            (novo(vec![no("", "a")], vec![]), "`chave`"),
            (
                novo(
                    vec![no("A", "a")],
                    vec![aresta("A", "B", Confianca::Inferido)],
                ),
                "`B` não é a chave",
            ),
            (
                novo(
                    vec![no("A", "a"), no("B", "b")],
                    vec![aresta("A", "B", Confianca::Confirmado)],
                ),
                "não tem evidência",
            ),
            (
                novo(
                    (0..=MAXIMO_DE_NOS)
                        .map(|i| no(&i.to_string(), "n"))
                        .collect(),
                    vec![],
                ),
                "máximo",
            ),
        ];
        for (diagrama, trecho) in casos {
            let erro = a.registrar_diagrama(diagrama, None, 1).unwrap_err();
            assert!(erro.to_string().contains(trecho), "{erro}");
        }
        let mut sem_titulo = novo(vec![no("A", "a")], vec![]);
        sem_titulo.titulo = " ".into();
        assert_eq!(
            a.registrar_diagrama(sem_titulo, None, 1),
            Err(ErroModelo::CampoVazio("titulo"))
        );
        assert!(a.diagramas.is_empty());
        assert_eq!(a.estado, EstadoAnalise::Criada);
    }

    #[test]
    fn serializa_em_snake_case_sem_campos_vazios() {
        let d = Diagrama {
            numero: 1,
            tipo: TipoDiagrama::Fluxo,
            titulo: "t".into(),
            secao: "s".into(),
            nos: vec![No {
                papel: PapelNo::Decisao,
                ..no("A", "a")
            }],
            arestas: vec![],
            sessao: None,
            registrada_ms: 1,
        };
        let json = serde_json::to_value(&d).unwrap();
        assert_eq!(json["tipo"], "fluxo");
        assert_eq!(json["nos"][0]["papel"], "decisao");
        assert!(json["nos"][0].get("evidencias").is_none());
        let lido: Diagrama = serde_json::from_value(json).unwrap();
        assert_eq!(lido, d);
    }
}
