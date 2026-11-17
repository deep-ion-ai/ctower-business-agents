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
//!
//! # Fluxo do processo e modelo de dados (#120)
//!
//! - No tipo `fluxo`, cada passo pode ter a raia ([`No::raia`]): o ator que o
//!   executa (a tela do atendente, o banco, o lote). A figura agrupa os passos de
//!   cada raia.
//! - O tipo `dados` é o modelo de dados conceitual: cada nó é uma entidade de
//!   negócio ([`Entidade`]: a tabela de origem e os atributos de negócio, sem as
//!   colunas técnicas), e cada aresta é uma relação ([`Relacao`]: a cardinalidade
//!   das duas pontas e a origem, chave estrangeira ou join no código). Relação
//!   inferida de join não é `confirmado`: o código mostra o uso, não a regra.

use serde::{Deserialize, Serialize};

use crate::analise::{Analise, Confianca, ErroModelo, Evidencia};

/// Nós no máximo num diagrama: mais que isso não se lê numa página.
pub const MAXIMO_DE_NOS: usize = 40;
/// Arestas no máximo num diagrama.
pub const MAXIMO_DE_ARESTAS: usize = 80;
/// Tamanho máximo da chave de um nó.
pub const TAMANHO_DA_CHAVE: usize = 40;
/// Atributos no máximo numa entidade do modelo de dados: o modelo conceitual
/// mostra os de negócio, não a tabela inteira.
pub const MAXIMO_DE_ATRIBUTOS: usize = 30;

/// O que o diagrama mostra.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TipoDiagrama {
    /// Ciclo de vida de uma entidade: estados e transições.
    Estados,
    /// Fluxo de um processo: passos, decisões e caminhos, em raias por ator.
    Fluxo,
    /// Modelo de dados conceitual: entidades de negócio e relações (#120).
    Dados,
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
    /// Raia do passo (tipo `fluxo`): o ator que o executa ("Atendente",
    /// "Banco de dados").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raia: Option<String>,
    /// Entidade de negócio (tipo `dados`, e só nele).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entidade: Option<Entidade>,
}

/// Entidade de negócio do modelo de dados conceitual: o nó do tipo `dados`, cujo
/// rótulo é o nome de negócio (o do glossário, quando há).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entidade {
    /// Tabela de origem (`FATURA`).
    pub tabela: String,
    /// Atributos de negócio, sem as colunas técnicas (chaves, auditoria).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub atributos: Vec<Atributo>,
    /// Termo do glossário da base ou proposto na análise que vive nesta tabela,
    /// achado pelo app no registro.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub termo: Option<String>,
}

/// Atributo de negócio de uma entidade.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Atributo {
    /// Nome de negócio ("Data de emissão").
    pub nome: String,
    /// Coluna de origem (`DT_EMISSAO`).
    pub coluna: String,
}

/// Quantos registros de uma ponta se ligam a um registro da outra.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Multiplicidade {
    ZeroOuUm,
    Um,
    ZeroOuMuitos,
    UmOuMuitos,
}

impl Multiplicidade {
    /// Rótulo curto ("0..1", "1", "0..N", "1..N").
    pub fn rotulo(self) -> &'static str {
        match self {
            Self::ZeroOuUm => "0..1",
            Self::Um => "1",
            Self::ZeroOuMuitos => "0..N",
            Self::UmOuMuitos => "1..N",
        }
    }
}

/// De onde vem a relação entre duas entidades.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrigemRelacao {
    /// Chave estrangeira declarada nos scripts do banco.
    ChaveEstrangeira,
    /// Inferida de um join (ou de uma gravação) no código, sem chave estrangeira.
    Join,
}

/// Relação entre duas entidades (aresta do tipo `dados`): `de` é a multiplicidade
/// da entidade de origem para cada registro da de destino, e `para` a da de
/// destino para cada registro da de origem. Cliente → Fatura: `de` = um (cada
/// fatura tem um cliente), `para` = zero ou muitos (cada cliente tem zero ou mais
/// faturas).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Relacao {
    pub de: Multiplicidade,
    pub para: Multiplicidade,
    pub origem: OrigemRelacao,
}

impl Relacao {
    /// `1:0..N`.
    pub fn cardinalidade(&self) -> String {
        format!("{}:{}", self.de.rotulo(), self.para.rotulo())
    }
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
    /// Guarda da transição: o que precisa ser verdade para o estado mudar (#119).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condicao: Option<String>,
    /// Efeitos colaterais da transição: o que mais a rotina grava ou dispara (#119).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub efeitos: Option<String>,
    pub confianca: Confianca,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidencias: Vec<Evidencia>,
    /// Relação entre entidades (tipo `dados`, e só nele).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relacao: Option<Relacao>,
}

impl Aresta {
    /// Relação inferida de join no código, sem chave estrangeira.
    pub fn inferida_de_join(&self) -> bool {
        self.relacao
            .is_some_and(|r| r.origem == OrigemRelacao::Join)
    }
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

    /// Aresta pelas chaves das pontas (a primeira, se houver mais de uma).
    pub fn aresta(&self, de: &str, para: &str) -> Option<&Aresta> {
        self.arestas.iter().find(|a| a.de == de && a.para == para)
    }

    /// Achados do ciclo de vida (#119), só no diagrama de estados, calculados das
    /// transições registradas: estado inalcançável (não é o inicial e nenhuma
    /// transição chega a ele), estado sem saída (não é final, alguma transição
    /// chega e nenhuma sai) e transição sem condição. Transição de um estado para
    /// ele mesmo não conta como chegada nem saída. Na ordem dos nós e depois das
    /// arestas.
    pub fn achados(&self) -> Vec<AchadoDiagrama> {
        if self.tipo != TipoDiagrama::Estados {
            return Vec::new();
        }
        let mut achados = Vec::new();
        for no in &self.nos {
            let chega = self
                .arestas
                .iter()
                .any(|a| a.para == no.chave && a.de != no.chave);
            let sai = self
                .arestas
                .iter()
                .any(|a| a.de == no.chave && a.para != no.chave);
            let tipo = if no.papel != PapelNo::Inicio && !chega {
                TipoAchadoDiagrama::Inalcancavel
            } else if no.papel != PapelNo::Fim && chega && !sai {
                TipoAchadoDiagrama::SemSaida
            } else {
                continue;
            };
            achados.push(AchadoDiagrama::do_estado(tipo, no));
        }
        for (i, aresta) in self.arestas.iter().enumerate() {
            if aresta.condicao.is_none() {
                let rotulo = |c: &str| self.no(c).map_or(c.to_owned(), |n| n.rotulo.clone());
                achados.push(AchadoDiagrama {
                    id: format!("sem-condicao:{}", i + 1),
                    tipo: TipoAchadoDiagrama::SemCondicao,
                    estado: None,
                    aresta: Some(i as u32 + 1),
                    texto: format!(
                        "A transição de {} para {} não tem condição registrada: o código \
                         muda o estado sem conferir nada, ou a condição não foi encontrada.",
                        rotulo(&aresta.de),
                        rotulo(&aresta.para)
                    ),
                    evidencias: aresta.evidencias.clone(),
                });
            }
        }
        achados
    }

    /// Achado pelo id (`inalcancavel:PROTESTADA`, `sem-condicao:2`).
    pub fn achado(&self, id: &str) -> Option<AchadoDiagrama> {
        self.achados().into_iter().find(|a| a.id == id)
    }
}

/// Tipo de achado do ciclo de vida.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TipoAchadoDiagrama {
    /// Nenhuma transição chega ao estado (e ele não é o inicial).
    Inalcancavel,
    /// O estado não é final e nenhuma transição sai dele.
    SemSaida,
    /// A transição não tem condição (guarda) registrada.
    SemCondicao,
}

impl TipoAchadoDiagrama {
    /// Prefixo do id do achado.
    pub fn prefixo(self) -> &'static str {
        match self {
            Self::Inalcancavel => "inalcancavel",
            Self::SemSaida => "sem-saida",
            Self::SemCondicao => "sem-condicao",
        }
    }
}

/// Achado do ciclo de vida para o analista, calculado de um diagrama de estados
/// ([`Diagrama::achados`]). Não fica gravado: muda junto com o diagrama.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AchadoDiagrama {
    /// Estável enquanto o elemento existir: `inalcancavel:<chave>`,
    /// `sem-saida:<chave>` ou `sem-condicao:<n>` (n-ésima aresta, a partir de 1).
    pub id: String,
    pub tipo: TipoAchadoDiagrama,
    /// Chave do estado (inalcançável e sem saída).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estado: Option<String>,
    /// Número da aresta, a partir de 1 (sem condição).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aresta: Option<u32>,
    /// O achado em linguagem de negócio, pronto para virar pergunta.
    pub texto: String,
    /// Evidências do elemento (o estado ou a transição).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidencias: Vec<Evidencia>,
}

impl AchadoDiagrama {
    fn do_estado(tipo: TipoAchadoDiagrama, no: &No) -> Self {
        let texto = match tipo {
            TipoAchadoDiagrama::Inalcancavel => format!(
                "O estado {} (`{}`) é inalcançável: nenhuma rotina registrada leva a \
                 entidade até ele. Ele ainda é usado ou ficou para trás?",
                no.rotulo, no.chave
            ),
            _ => format!(
                "O estado {} (`{}`) não tem saída: nenhuma rotina registrada tira a \
                 entidade dele, e ele não é final. Como a entidade sai desse estado?",
                no.rotulo, no.chave
            ),
        };
        Self {
            id: format!("{}:{}", tipo.prefixo(), no.chave),
            tipo,
            estado: Some(no.chave.clone()),
            aresta: None,
            texto,
            evidencias: no.evidencias.clone(),
        }
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
            no.raia = aparar(no.raia.take());
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
            conferir_no(self.tipo, no)?;
            chaves.push(no.chave.clone());
        }
        if self.tipo == TipoDiagrama::Dados {
            let mut tabelas: Vec<String> = Vec::new();
            for no in &self.nos {
                let tabela = no.entidade.as_ref().map(|e| e.tabela.to_uppercase());
                if let Some(t) = tabela {
                    if tabelas.contains(&t) {
                        return Err(invalido(format!(
                            "a tabela `{t}` aparece em mais de uma entidade; cada entidade \
                             tem a sua tabela de origem"
                        )));
                    }
                    tabelas.push(t);
                }
            }
        }
        for (i, aresta) in self.arestas.iter_mut().enumerate() {
            aresta.de = aresta.de.trim().to_owned();
            aresta.para = aresta.para.trim().to_owned();
            aresta.rotulo = aparar(aresta.rotulo.take());
            aresta.descricao = aparar(aresta.descricao.take());
            aresta.condicao = aparar(aresta.condicao.take());
            aresta.efeitos = aparar(aresta.efeitos.take());
            conferir_relacao(self.tipo, i, aresta)?;
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

/// Confere o nó conforme o tipo do diagrama: raia só no fluxo; entidade (com
/// tabela e atributos completos) em todo nó do modelo de dados, e só nele.
fn conferir_no(tipo: TipoDiagrama, no: &mut No) -> Result<(), ErroModelo> {
    if no.raia.is_some() && tipo != TipoDiagrama::Fluxo {
        return Err(invalido(format!(
            "o nó `{}` tem `raia`, que só vale no tipo `fluxo` (o ator que executa o passo)",
            no.chave
        )));
    }
    match (tipo, no.entidade.as_mut()) {
        (TipoDiagrama::Dados, None) => Err(invalido(format!(
            "no tipo `dados`, cada nó é uma entidade: informe a `tabela` do nó `{}`",
            no.chave
        ))),
        (TipoDiagrama::Dados, Some(e)) => {
            if no.papel != PapelNo::Etapa {
                return Err(invalido(format!(
                    "no tipo `dados`, os nós são entidades: omita o `papel` do nó `{}`",
                    no.chave
                )));
            }
            e.tabela = e.tabela.trim().to_owned();
            e.termo = aparar(e.termo.take());
            if e.tabela.is_empty() {
                return Err(invalido(format!(
                    "a entidade `{}` precisa de `tabela`",
                    no.chave
                )));
            }
            if e.atributos.len() > MAXIMO_DE_ATRIBUTOS {
                return Err(invalido(format!(
                    "a entidade `{}` tem {} atributos, mais que o máximo de \
                     {MAXIMO_DE_ATRIBUTOS}; deixe só os de negócio",
                    no.chave,
                    e.atributos.len()
                )));
            }
            for atributo in &mut e.atributos {
                atributo.nome = atributo.nome.trim().to_owned();
                atributo.coluna = atributo.coluna.trim().to_owned();
                if atributo.nome.is_empty() || atributo.coluna.is_empty() {
                    return Err(invalido(format!(
                        "cada atributo da entidade `{}` precisa de `nome` e `coluna`",
                        no.chave
                    )));
                }
            }
            Ok(())
        }
        (_, Some(_)) => Err(invalido(format!(
            "o nó `{}` tem `tabela` ou `atributos`, que só valem no tipo `dados`",
            no.chave
        ))),
        (_, None) => Ok(()),
    }
}

/// Confere a aresta conforme o tipo: relação (cardinalidade e origem) em toda
/// aresta do modelo de dados, e só nele; relação de join não é `confirmado`.
fn conferir_relacao(tipo: TipoDiagrama, i: usize, aresta: &Aresta) -> Result<(), ErroModelo> {
    let onde = format!("a aresta {} (`{}` → `{}`)", i + 1, aresta.de, aresta.para);
    match (tipo, aresta.relacao) {
        (TipoDiagrama::Dados, None) => Err(invalido(format!(
            "no tipo `dados`, cada aresta é uma relação: informe a cardinalidade e a \
             origem (`chave_estrangeira` ou `join`) d{onde}"
        ))),
        (TipoDiagrama::Dados, Some(r))
            if r.origem == OrigemRelacao::Join && aresta.confianca == Confianca::Confirmado =>
        {
            Err(invalido(format!(
                "{onde} vem de um join no código, sem chave estrangeira: é uma relação \
                 inferida; use `inferido` (ou `a_confirmar`)"
            )))
        }
        (TipoDiagrama::Dados, Some(_)) | (_, None) => Ok(()),
        (_, Some(_)) => Err(invalido(format!(
            "{onde} tem cardinalidade ou origem, que só valem no tipo `dados`"
        ))),
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
            raia: None,
            entidade: None,
        }
    }

    fn aresta(de: &str, para: &str, confianca: Confianca) -> Aresta {
        Aresta {
            de: de.into(),
            para: para.into(),
            rotulo: Some(" emitir ".into()),
            descricao: Some("  ".into()),
            condicao: None,
            efeitos: None,
            confianca,
            evidencias: vec![],
            relacao: None,
        }
    }

    fn entidade(chave: &str, tabela: &str) -> No {
        No {
            entidade: Some(Entidade {
                tabela: format!(" {tabela} "),
                atributos: vec![Atributo {
                    nome: " Nome ".into(),
                    coluna: "NOME".into(),
                }],
                termo: None,
            }),
            ..no(chave, chave)
        }
    }

    fn relacao(de: &str, para: &str, origem: OrigemRelacao, confianca: Confianca) -> Aresta {
        Aresta {
            relacao: Some(Relacao {
                de: Multiplicidade::Um,
                para: Multiplicidade::ZeroOuMuitos,
                origem,
            }),
            ..aresta(de, para, confianca)
        }
    }

    #[test]
    fn raias_no_fluxo_e_entidades_no_modelo_de_dados() {
        let mut a = analise();
        let mut fluxo = novo(
            vec![
                No {
                    raia: Some(" Atendente ".into()),
                    ..no("A", "Informar a fatura")
                },
                no("B", "Gravar"),
            ],
            vec![aresta("A", "B", Confianca::Inferido)],
        );
        fluxo.tipo = TipoDiagrama::Fluxo;
        a.registrar_diagrama(fluxo, None, 1).unwrap();
        assert_eq!(a.diagramas[0].nos[0].raia.as_deref(), Some("Atendente"));
        assert_eq!(a.diagramas[0].nos[1].raia, None);

        let mut dados = novo(
            vec![entidade("C", "CLIENTE"), entidade("F", "FATURA")],
            vec![relacao("C", "F", OrigemRelacao::Join, Confianca::Inferido)],
        );
        dados.tipo = TipoDiagrama::Dados;
        let n = a.registrar_diagrama(dados, None, 2).unwrap();
        let d = a.diagrama(n).unwrap();
        let e = d.nos[0].entidade.as_ref().unwrap();
        assert_eq!(e.tabela, "CLIENTE");
        assert_eq!(e.atributos[0].nome, "Nome");
        assert!(d.aresta("C", "F").unwrap().inferida_de_join());
        assert_eq!(d.arestas[0].relacao.unwrap().cardinalidade(), "1:0..N");
    }

    #[test]
    fn relacao_vira_pergunta_uma_vez_so() {
        use crate::analise::{EstadoPergunta, OrigemPergunta};
        let mut a = analise();
        let evidencia = Evidencia {
            arquivo: cba_kb::CaminhoRelativo::novo("db/03_pkg_cobranca.sql").unwrap(),
            linhas: cba_kb::frontmatter::Linhas::novo(130, 131).unwrap(),
            trecho: "INSERT INTO LOG_FATURA".into(),
            hash: "h".into(),
            commit: None,
            estado: crate::analise::EstadoEvidencia::Valida,
        };
        let mut dados = novo(
            vec![entidade("F", "FATURA"), entidade("L", "LOG_FATURA")],
            vec![Aresta {
                evidencias: vec![evidencia.clone()],
                ..relacao("F", "L", OrigemRelacao::Join, Confianca::Inferido)
            }],
        );
        dados.tipo = TipoDiagrama::Dados;
        a.registrar_diagrama(dados, None, 1).unwrap();
        a.concluir().unwrap();
        let origem = OrigemPergunta::Aresta {
            diagrama: 1,
            de: "F".into(),
            para: "L".into(),
        };
        let n = a
            .virar_pergunta(" O histórico é da fatura? ", origem.clone(), 9)
            .unwrap();
        let p = a.pergunta_de(&origem).unwrap();
        assert_eq!(p.numero, n);
        assert_eq!(p.texto, "O histórico é da fatura?");
        assert_eq!(p.estado, EstadoPergunta::Aberta);
        assert_eq!(p.evidencias, vec![evidencia]);
        // Não reabre a análise concluída.
        assert_eq!(a.estado, EstadoAnalise::Concluida);
        let erro = a.virar_pergunta("de novo", origem, 10).unwrap_err();
        assert!(erro.to_string().contains("já virou a pergunta 1"), "{erro}");
        let outra = OrigemPergunta::Aresta {
            diagrama: 1,
            de: "L".into(),
            para: "F".into(),
        };
        assert!(a.virar_pergunta("x", outra, 10).is_err());
        assert_eq!(a.perguntas.len(), 1);
        let json = serde_json::to_value(&a.perguntas[0]).unwrap();
        assert_eq!(json["origem"]["tipo"], "aresta");
        assert_eq!(json["origem"]["diagrama"], 1);
    }

    #[test]
    fn recusa_elementos_fora_do_tipo() {
        let mut a = analise();
        let dados = |nos, arestas| NovoDiagrama {
            tipo: TipoDiagrama::Dados,
            ..novo(nos, arestas)
        };
        let casos = [
            (
                dados(vec![no("C", "Cliente")], vec![]),
                "informe a `tabela`",
            ),
            (
                dados(
                    vec![entidade("C", "CLIENTE"), entidade("D", "cliente")],
                    vec![],
                ),
                "mais de uma entidade",
            ),
            (
                dados(
                    vec![No {
                        papel: PapelNo::Inicio,
                        ..entidade("C", "CLIENTE")
                    }],
                    vec![],
                ),
                "omita o `papel`",
            ),
            (
                dados(
                    vec![entidade("C", "CLIENTE"), entidade("F", "FATURA")],
                    vec![aresta("C", "F", Confianca::Inferido)],
                ),
                "cardinalidade e a origem",
            ),
            (
                dados(
                    vec![entidade("C", "CLIENTE"), entidade("F", "FATURA")],
                    vec![Aresta {
                        evidencias: vec![],
                        ..relacao("C", "F", OrigemRelacao::Join, Confianca::Confirmado)
                    }],
                ),
                "relação inferida",
            ),
            (
                novo(vec![entidade("C", "CLIENTE")], vec![]),
                "só valem no tipo `dados`",
            ),
            (
                novo(
                    vec![No {
                        raia: Some("Atendente".into()),
                        ..no("A", "a")
                    }],
                    vec![],
                ),
                "só vale no tipo `fluxo`",
            ),
            (
                novo(
                    vec![no("A", "a"), no("B", "b")],
                    vec![relacao("A", "B", OrigemRelacao::Join, Confianca::Inferido)],
                ),
                "só valem no tipo `dados`",
            ),
        ];
        for (diagrama, trecho) in casos {
            let erro = a.registrar_diagrama(diagrama, None, 1).unwrap_err();
            assert!(erro.to_string().contains(trecho), "{trecho}: {erro}");
        }
        assert!(a.diagramas.is_empty());
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

    #[test]
    fn achados_do_ciclo_de_vida() {
        let papel = |chave: &str, rotulo: &str, papel: PapelNo| No {
            papel,
            ..no(chave, rotulo)
        };
        let transicao = |de: &str, para: &str, condicao: Option<&str>| Aresta {
            condicao: condicao.map(Into::into),
            ..aresta(de, para, Confianca::Inferido)
        };
        let d = Diagrama {
            numero: 1,
            tipo: TipoDiagrama::Estados,
            titulo: "Ciclo de vida da fatura".into(),
            secao: "s".into(),
            nos: vec![
                papel("ABERTA", "Aberta", PapelNo::Inicio),
                papel("PAGA", "Paga", PapelNo::Fim),
                no("SUSPENSA", "Suspensa"),
                no("PROTESTADA", "Protestada"),
            ],
            arestas: vec![
                transicao("ABERTA", "PAGA", Some("pagamento total")),
                transicao("ABERTA", "SUSPENSA", None),
                // Laço não conta como saída.
                transicao("SUSPENSA", "SUSPENSA", Some("x")),
            ],
            sessao: None,
            registrada_ms: 1,
        };
        let achados = d.achados();
        let ids: Vec<&str> = achados.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "sem-saida:SUSPENSA",
                "inalcancavel:PROTESTADA",
                "sem-condicao:2"
            ]
        );
        assert!(
            achados[1].texto.contains("Protestada"),
            "{}",
            achados[1].texto
        );
        assert_eq!(achados[2].aresta, Some(2));
        assert!(achados[2].texto.contains("de Aberta para Suspensa"));
        assert_eq!(
            d.achado("inalcancavel:PROTESTADA")
                .unwrap()
                .estado
                .as_deref(),
            Some("PROTESTADA")
        );
        assert!(d.achado("inalcancavel:ABERTA").is_none());
        // Só o diagrama de estados tem achados.
        let fluxo = Diagrama {
            tipo: TipoDiagrama::Fluxo,
            ..d
        };
        assert!(fluxo.achados().is_empty());
    }
}
