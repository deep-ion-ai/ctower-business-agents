//! Cenários de teste derivados das regras, no estado da análise do fluxo
//! `cenarios` (#117).
//!
//! # Modelo
//!
//! - [`TabelaDecisao`]: tabela de decisão `TD-n` (número dentro da análise) para
//!   uma regra com duas ou mais condições combinadas: as condições (cada uma com a
//!   evidência do código, quando houver), as ações e as colunas `R1`, `R2`, …,
//!   cada uma com o valor de cada condição (sim, não ou indiferente) e as ações
//!   executadas, ou marcada impossível com a justificativa. A tabela é completa:
//!   toda combinação das condições cai numa coluna, e duas colunas não cobrem a
//!   mesma combinação ([`TabelaDecisao::conferir`]).
//! - [`Cenario`]: cenário de teste com identificador estável `CT-NNNN`
//!   (sequencial por projeto, nunca reaproveitado; a numeração é de
//!   [`crate::analises::Analises::maior_numero`]), a técnica ([`Tecnica`]), os
//!   passos (pré-condição, ação e resultado esperado, em Dado / Quando / Então,
//!   com valores concretos), o valor testado e, nas técnicas de valor-limite e de
//!   partição de equivalência, o [`Limite`] lido do código, com a evidência. Liga
//!   se às regras e afirmações de que foi derivado ([`Fundamento`]), à coluna da
//!   tabela de decisão que exercita e, se houver, à história (`HU-NNNN`).
//!
//! # Limite lido do código
//!
//! O valor do limite precisa aparecer no trecho de código citado como evidência
//! (`10` em `LENGTH(TRIM(P_MOTIVO)) < 10`): é o que garante que o cenário testa o
//! limite que o sistema aplica, e não o que alguém lembra. Quando o valor vem de
//! tabela de parâmetros ou de configuração (fora do código), o limite leva
//! `configuracao` (o parâmetro) e o cenário precisa de uma pergunta em aberto: fica
//! "a confirmar".
//!
//! # Estados
//!
//! ```text
//! Tabela e cenário:  pendente ──► aceito | descartado   (pendente desfaz)
//! ```

use cba_kb::{IdCenario, IdHistoria};
use serde::{Deserialize, Serialize};

use crate::analise::{Analise, ErroModelo, EstadoAnalise, Evidencia, Fluxo};
use crate::historias::Gherkin;
use crate::requisito::{Fundamento, RefItem, SituacaoItem, sem_repetidos};

/// Condições no máximo numa tabela de decisão (2^6 = 64 combinações).
pub const MAXIMO_DE_CONDICOES: usize = 6;

/// Técnica de projeto de teste que gerou o cenário.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tecnica {
    /// Uma coluna de uma tabela de decisão.
    TabelaDeDecisao,
    /// Um representante de uma classe de equivalência (válida ou inválida).
    ParticaoDeEquivalencia,
    /// O limite de uma regra numérica ou de data, ou o valor logo antes ou depois.
    ValorLimite,
    /// Entrada inválida que o código recusa (mensagem ou código de erro).
    Negativo,
    /// Situação de exceção que o código trata (falha, estado inesperado).
    Excecao,
}

impl Tecnica {
    pub const TODAS: [Tecnica; 5] = [
        Self::TabelaDeDecisao,
        Self::ParticaoDeEquivalencia,
        Self::ValorLimite,
        Self::Negativo,
        Self::Excecao,
    ];

    /// Nome para o analista ("valor-limite").
    pub fn rotulo(self) -> &'static str {
        match self {
            Self::TabelaDeDecisao => "tabela de decisão",
            Self::ParticaoDeEquivalencia => "partição de equivalência",
            Self::ValorLimite => "valor-limite",
            Self::Negativo => "negativo",
            Self::Excecao => "exceção",
        }
    }

    /// A técnica precisa do limite lido do código.
    pub fn exige_limite(self) -> bool {
        matches!(self, Self::ValorLimite | Self::ParticaoDeEquivalencia)
    }
}

/// Valor de uma condição numa coluna da tabela de decisão.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ValorCondicao {
    #[serde(rename = "S")]
    Sim,
    #[serde(rename = "N")]
    Nao,
    /// Tanto faz: a coluna vale para os dois valores.
    #[serde(rename = "-")]
    Indiferente,
}

impl ValorCondicao {
    /// `S`, `N` ou `—`.
    pub fn simbolo(self) -> &'static str {
        match self {
            Self::Sim => "S",
            Self::Nao => "N",
            Self::Indiferente => "—",
        }
    }

    /// Lê `S`/`sim`/`true`, `N`/`nao`/`não`/`false` e `-`/`—`/`indiferente`.
    pub fn analisar(texto: &str) -> Option<Self> {
        match texto.trim().to_lowercase().as_str() {
            "s" | "sim" | "true" | "v" => Some(Self::Sim),
            "n" | "nao" | "não" | "false" | "f" => Some(Self::Nao),
            "-" | "—" | "–" | "*" | "indiferente" | "x" => Some(Self::Indiferente),
            _ => None,
        }
    }

    fn cobre(self, valor: bool) -> bool {
        match self {
            Self::Sim => valor,
            Self::Nao => !valor,
            Self::Indiferente => true,
        }
    }

    /// Os dois valores podem coincidir (sobreposição de colunas).
    fn compativel(self, outro: Self) -> bool {
        self == outro || self == Self::Indiferente || outro == Self::Indiferente
    }
}

/// Condição de uma tabela de decisão ("fatura emitida há mais de 30 dias").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Condicao {
    pub texto: String,
    /// Trechos do código que testam a condição.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidencias: Vec<Evidencia>,
}

/// Coluna (regra da tabela) `R<numero>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Coluna {
    pub numero: u32,
    /// Valor de cada condição, na ordem das condições.
    pub valores: Vec<ValorCondicao>,
    /// Ações executadas (números a partir de 1, na ordem das ações).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub acoes: Vec<u32>,
    /// Combinação impossível, com a justificativa (sem ações).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub impossivel: Option<String>,
}

impl Coluna {
    pub fn rotulo(&self) -> String {
        format!("R{}", self.numero)
    }

    /// A coluna cobre a combinação (um valor por condição).
    pub fn cobre(&self, combinacao: &[bool]) -> bool {
        self.valores
            .iter()
            .zip(combinacao)
            .all(|(v, c)| v.cobre(*c))
    }
}

/// Dados de uma coluna nova.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NovaColuna {
    pub valores: Vec<ValorCondicao>,
    pub acoes: Vec<u32>,
    pub impossivel: Option<String>,
}

/// Tabela de decisão de uma regra com várias condições.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TabelaDecisao {
    /// `TD-<numero>`, sequencial dentro da análise.
    pub numero: u32,
    pub titulo: String,
    pub condicoes: Vec<Condicao>,
    pub acoes: Vec<String>,
    pub colunas: Vec<Coluna>,
    /// Regras e afirmações de que a tabela foi derivada (ao menos uma).
    pub fundamentos: Vec<Fundamento>,
    #[serde(default)]
    pub situacao: SituacaoItem,
    #[serde(default)]
    pub sessao: Option<String>,
    pub registrada_ms: u64,
}

impl TabelaDecisao {
    /// `TD-1`.
    pub fn rotulo(&self) -> String {
        format!("TD-{}", self.numero)
    }

    pub fn coluna(&self, numero: u32) -> Option<&Coluna> {
        self.colunas.iter().find(|c| c.numero == numero)
    }

    /// Confere a tabela: ao menos duas condições (no máximo
    /// [`MAXIMO_DE_CONDICOES`]) e uma ação; cada coluna com um valor por condição
    /// e, se possível, ao menos uma ação que existe, ou, se impossível, a
    /// justificativa e nenhuma ação; nenhuma combinação descoberta e nenhuma
    /// coberta por duas colunas.
    pub fn conferir(&self) -> Result<(), ErroModelo> {
        let erro = |m: String| Err(ErroModelo::TabelaInvalida(m));
        let n = self.condicoes.len();
        if n < 2 {
            return erro(
                "a tabela de decisão é para regra com duas ou mais condições combinadas; com \
                 uma condição só, registre os cenários por partição de equivalência"
                    .into(),
            );
        }
        if n > MAXIMO_DE_CONDICOES {
            return erro(format!(
                "no máximo {MAXIMO_DE_CONDICOES} condições por tabela; divida a regra"
            ));
        }
        if self.acoes.is_empty() {
            return erro("informe ao menos uma ação (o que o sistema faz)".into());
        }
        if self.colunas.is_empty() {
            return erro("informe as colunas (combinações das condições)".into());
        }
        for c in &self.colunas {
            let r = c.rotulo();
            if c.valores.len() != n {
                return erro(format!(
                    "a coluna {r} tem {} valor(es) e a tabela tem {n} condições: informe um \
                     valor por condição (S, N ou -)",
                    c.valores.len()
                ));
            }
            if let Some(a) = c
                .acoes
                .iter()
                .find(|a| **a == 0 || **a as usize > self.acoes.len())
            {
                return erro(format!(
                    "a coluna {r} cita a ação {a}, e a tabela tem {} ação(ões)",
                    self.acoes.len()
                ));
            }
            match &c.impossivel {
                Some(_) if !c.acoes.is_empty() => {
                    return erro(format!(
                        "a coluna {r} é impossível e não executa ações; tire as ações ou a \
                         marca de impossível"
                    ));
                }
                None if c.acoes.is_empty() => {
                    return erro(format!(
                        "a coluna {r} não executa nenhuma ação; informe as ações ou, se a \
                         combinação não pode acontecer, marque-a impossível com a \
                         justificativa"
                    ));
                }
                _ => {}
            }
        }
        for (i, a) in self.colunas.iter().enumerate() {
            for b in &self.colunas[i + 1..] {
                if a.valores
                    .iter()
                    .zip(&b.valores)
                    .all(|(x, y)| x.compativel(*y))
                {
                    return erro(format!(
                        "as colunas {} e {} cobrem a mesma combinação; cada combinação fica \
                         numa coluna só",
                        a.rotulo(),
                        b.rotulo()
                    ));
                }
            }
        }
        let faltam = self.combinacoes_descobertas();
        if !faltam.is_empty() {
            return erro(format!(
                "combinação(ões) sem coluna: {}. Acrescente as colunas ou, se a combinação \
                 não pode acontecer, marque-a impossível com a justificativa",
                faltam.join(", ")
            ));
        }
        Ok(())
    }

    /// Combinações das condições que nenhuma coluna cobre, como `S/N/S`.
    pub fn combinacoes_descobertas(&self) -> Vec<String> {
        let n = self.condicoes.len().min(MAXIMO_DE_CONDICOES);
        let mut faltam = Vec::new();
        for bits in 0..(1u32 << n) {
            // Sim primeiro, como se escreve a tabela à mão.
            let combinacao: Vec<bool> = (0..n).map(|i| bits & (1 << (n - 1 - i)) == 0).collect();
            if !self.colunas.iter().any(|c| c.cobre(&combinacao)) {
                faltam.push(
                    combinacao
                        .iter()
                        .map(|b| if *b { "S" } else { "N" })
                        .collect::<Vec<_>>()
                        .join("/"),
                );
            }
        }
        faltam
    }
}

/// Dados de uma tabela nova.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NovaTabela {
    pub titulo: String,
    pub condicoes: Vec<Condicao>,
    pub acoes: Vec<String>,
    pub colunas: Vec<NovaColuna>,
    pub fundamentos: Vec<Fundamento>,
    pub sessao: Option<String>,
}

/// Limite de uma regra numérica ou de data, lido do código.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Limite {
    /// O que é medido ("tamanho do motivo, em caracteres, sem espaços nas pontas").
    pub grandeza: String,
    /// O limite como está no código ("10").
    pub valor: String,
    /// Trechos do código que definem ou aplicam o limite (ao menos um).
    pub evidencias: Vec<Evidencia>,
    /// Parâmetro ou tabela de configuração de onde o valor vem, quando não está no
    /// código (`PARAMETRO.DIAS_CARENCIA`): o cenário fica "a confirmar".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configuracao: Option<String>,
}

/// Referência a uma coluna de tabela de decisão: `TD-1/R2`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RefColuna {
    pub tabela: u32,
    pub coluna: u32,
}

impl RefColuna {
    /// Lê `TD-1/R2`.
    pub fn analisar(texto: &str) -> Option<Self> {
        let (t, c) = texto.trim().split_once('/')?;
        let numero = |s: &str, prefixo: &str| -> Option<u32> {
            let s = s.trim();
            let resto = s
                .strip_prefix(prefixo)
                .or_else(|| s.strip_prefix(&prefixo.to_lowercase()))?;
            resto.parse().ok().filter(|n| *n > 0)
        };
        Some(Self {
            tabela: numero(t, "TD-")?,
            coluna: numero(c, "R")?,
        })
    }
}

impl std::fmt::Display for RefColuna {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TD-{}/R{}", self.tabela, self.coluna)
    }
}

/// Título e passos de um cenário, a parte que o analista edita.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextoCenario {
    pub titulo: String,
    /// Pré-condição, ação e resultado esperado.
    #[serde(flatten)]
    pub passos: Gherkin,
}

impl TextoCenario {
    fn conferir(&self) -> Result<Self, ErroModelo> {
        exigir("titulo", &self.titulo)?;
        Ok(Self {
            titulo: self.titulo.trim().to_owned(),
            passos: self.passos.conferir()?,
        })
    }
}

/// Cenário de teste.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cenario {
    pub id: IdCenario,
    pub tecnica: Tecnica,
    #[serde(flatten)]
    pub texto: TextoCenario,
    /// Título e passos registrados pelo agente, guardados na primeira edição do
    /// analista.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original: Option<TextoCenario>,
    /// Valor ou combinação testada ("motivo com 9 caracteres").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limite: Option<Limite>,
    /// Coluna da tabela de decisão que o cenário exercita.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coluna: Option<RefColuna>,
    /// História que o cenário verifica.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub historia: Option<IdHistoria>,
    /// Regras e afirmações de que o cenário foi derivado (ao menos uma).
    pub fundamentos: Vec<Fundamento>,
    /// Pergunta em aberto de que o cenário depende: com ela, fica "a confirmar".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pergunta: Option<RefItem>,
    #[serde(default)]
    pub situacao: SituacaoItem,
    #[serde(default)]
    pub sessao: Option<String>,
    pub registrado_ms: u64,
}

impl Cenario {
    pub fn a_confirmar(&self) -> bool {
        self.pergunta.is_some()
    }

    pub fn no_documento(&self) -> bool {
        self.situacao != SituacaoItem::Descartado
    }

    /// Evidências do cenário: as do limite e as dos fundamentos, sem repetir o
    /// mesmo intervalo.
    pub fn evidencias(&self) -> Vec<cba_kb::frontmatter::Evidencia> {
        let mut lista: Vec<cba_kb::frontmatter::Evidencia> = Vec::new();
        let limite = self.limite.iter().flat_map(|l| &l.evidencias).map(|e| {
            cba_kb::frontmatter::Evidencia {
                arquivo: e.arquivo.clone(),
                linhas: e.linhas,
                hash: e.hash.clone(),
                commit: e.commit.clone(),
            }
        });
        let fundamentos = self
            .fundamentos
            .iter()
            .flat_map(|f| f.evidencias().iter().cloned());
        for e in limite.chain(fundamentos) {
            if !lista
                .iter()
                .any(|x| x.arquivo == e.arquivo && x.linhas == e.linhas)
            {
                lista.push(e);
            }
        }
        lista
    }
}

/// Dados de um cenário novo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NovoCenario {
    pub tecnica: Tecnica,
    pub texto: TextoCenario,
    pub valor: Option<String>,
    pub limite: Option<Limite>,
    pub coluna: Option<RefColuna>,
    pub historia: Option<IdHistoria>,
    pub fundamentos: Vec<Fundamento>,
    pub pergunta: Option<RefItem>,
    pub sessao: Option<String>,
}

fn exigir(campo: &'static str, valor: &str) -> Result<(), ErroModelo> {
    if valor.trim().is_empty() {
        Err(ErroModelo::CampoVazio(campo))
    } else {
        Ok(())
    }
}

/// Números de um texto, como o código e o analista escrevem: `10`, `10.00`,
/// `2.0000`, `R$ 1.000,50`, `0,033`.
pub fn numeros(texto: &str) -> Vec<f64> {
    let mut saida = Vec::new();
    let mut atual = String::new();
    let mut fechar = |atual: &mut String| {
        let t = atual.trim_end_matches(['.', ',']);
        if !t.is_empty() {
            let normal = if t.contains(',') {
                t.replace('.', "").replace(',', ".")
            } else if t.matches('.').count() > 1 {
                t.replace('.', "")
            } else {
                t.to_owned()
            };
            if let Ok(n) = normal.parse::<f64>() {
                saida.push(n);
            }
        }
        atual.clear();
    };
    for ch in texto.chars() {
        if ch.is_ascii_digit() || (!atual.is_empty() && (ch == '.' || ch == ',')) {
            atual.push(ch);
        } else {
            fechar(&mut atual);
        }
    }
    fechar(&mut atual);
    saida
}

fn sem_acento_minusculo(texto: &str) -> String {
    texto
        .chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' => 'a',
            'é' | 'ê' => 'e',
            'í' => 'i',
            'ó' | 'ô' | 'õ' => 'o',
            'ú' | 'ü' => 'u',
            'ç' => 'c',
            outro => outro,
        })
        .collect()
}

impl Limite {
    fn conferir(&self) -> Result<Self, ErroModelo> {
        exigir("limite.grandeza", &self.grandeza)?;
        exigir("limite.valor", &self.valor)?;
        if self.evidencias.is_empty() {
            return Err(ErroModelo::CampoVazio("limite.evidencias"));
        }
        let configuracao = self
            .configuracao
            .as_ref()
            .map(|c| c.trim().to_owned())
            .filter(|c| !c.is_empty());
        if configuracao.is_none() && !self.valor_no_codigo() {
            return Err(ErroModelo::LimiteForaDaEvidencia {
                valor: self.valor.trim().to_owned(),
                trechos: self
                    .evidencias
                    .iter()
                    .map(|e| format!("`{}`", e.trecho))
                    .collect::<Vec<_>>()
                    .join("; "),
            });
        }
        Ok(Self {
            grandeza: self.grandeza.trim().to_owned(),
            valor: self.valor.trim().to_owned(),
            evidencias: self.evidencias.clone(),
            configuracao,
        })
    }

    /// O valor do limite aparece num trecho citado: o primeiro número do valor
    /// entre os números do trecho ou, se o valor não tem número, o texto dele.
    pub fn valor_no_codigo(&self) -> bool {
        match numeros(&self.valor).first() {
            Some(n) => self
                .evidencias
                .iter()
                .any(|e| numeros(&e.trecho).iter().any(|x| (x - n).abs() < 1e-9)),
            None => {
                let valor = sem_acento_minusculo(self.valor.trim());
                self.evidencias
                    .iter()
                    .any(|e| sem_acento_minusculo(&e.trecho).contains(&valor))
            }
        }
    }
}

impl Analise {
    fn exigir_fluxo_de_cenarios(&self) -> Result<(), ErroModelo> {
        if self.fluxo == Fluxo::Cenarios {
            Ok(())
        } else {
            Err(ErroModelo::ForaDoFluxoDeCenarios(self.id.clone()))
        }
    }

    fn exigir_nao_aprovada_cenarios(&self) -> Result<(), ErroModelo> {
        if self.estado == EstadoAnalise::Aprovada {
            Err(ErroModelo::AnaliseAprovada(self.id.clone()))
        } else {
            Ok(())
        }
    }

    /// Maior número de cenário desta análise (0 sem cenários).
    pub fn maior_cenario(&self) -> u32 {
        self.cenarios
            .iter()
            .map(|c| c.id.numero())
            .max()
            .unwrap_or(0)
    }

    pub fn cenario(&self, id: IdCenario) -> Option<&Cenario> {
        self.cenarios.iter().find(|c| c.id == id)
    }

    pub fn tabela(&self, numero: u32) -> Option<&TabelaDecisao> {
        self.tabelas.iter().find(|t| t.numero == numero)
    }

    /// Registra uma tabela de decisão e devolve o número dela (`TD-n`).
    pub fn registrar_tabela(&mut self, nova: NovaTabela, agora_ms: u64) -> Result<u32, ErroModelo> {
        self.exigir_fluxo_de_cenarios()?;
        exigir("titulo", &nova.titulo)?;
        let fundamentos = sem_repetidos(nova.fundamentos);
        if fundamentos.is_empty() {
            return Err(ErroModelo::ItemSemFundamento("tabela de decisão"));
        }
        let mut condicoes = Vec::new();
        for c in nova.condicoes {
            exigir("condicoes.texto", &c.texto)?;
            condicoes.push(Condicao {
                texto: c.texto.trim().to_owned(),
                evidencias: c.evidencias,
            });
        }
        let mut acoes = Vec::new();
        for a in nova.acoes {
            exigir("acoes", &a)?;
            acoes.push(a.trim().to_owned());
        }
        let mut colunas = Vec::new();
        for (c, numero) in nova.colunas.into_iter().zip(1..) {
            let impossivel = match c.impossivel {
                Some(j) if j.trim().is_empty() => {
                    return Err(ErroModelo::TabelaInvalida(format!(
                        "a coluna R{numero} é impossível: escreva a justificativa"
                    )));
                }
                Some(j) => Some(j.trim().to_owned()),
                None => None,
            };
            let mut acoes = c.acoes;
            acoes.sort_unstable();
            acoes.dedup();
            colunas.push(Coluna {
                numero,
                valores: c.valores,
                acoes,
                impossivel,
            });
        }
        let numero = self.tabelas.iter().map(|t| t.numero).max().unwrap_or(0) + 1;
        let tabela = TabelaDecisao {
            numero,
            titulo: nova.titulo.trim().to_owned(),
            condicoes,
            acoes,
            colunas,
            fundamentos,
            situacao: SituacaoItem::Pendente,
            sessao: nova.sessao,
            registrada_ms: agora_ms,
        };
        tabela.conferir()?;
        self.aceitar_registro()?;
        self.tabelas.push(tabela);
        Ok(numero)
    }

    /// Registra um cenário com o identificador `id` (reservado por quem chama, sem
    /// colisão no projeto).
    pub fn registrar_cenario(
        &mut self,
        novo: NovoCenario,
        id: IdCenario,
        agora_ms: u64,
    ) -> Result<IdCenario, ErroModelo> {
        self.exigir_fluxo_de_cenarios()?;
        if self.cenario(id).is_some() {
            return Err(ErroModelo::CenarioRepetido(id));
        }
        let texto = novo.texto.conferir()?;
        let fundamentos = sem_repetidos(novo.fundamentos);
        if fundamentos.is_empty() {
            return Err(ErroModelo::ItemSemFundamento("cenário de teste"));
        }
        let limite = match (novo.limite, novo.tecnica.exige_limite()) {
            (None, true) => return Err(ErroModelo::CenarioSemLimite(novo.tecnica.rotulo())),
            (None, false) => None,
            (Some(l), _) => Some(l.conferir()?),
        };
        if let Some(l) = &limite
            && let Some(c) = &l.configuracao
            && novo.pergunta.is_none()
        {
            return Err(ErroModelo::LimiteDeConfiguracaoSemPergunta(c.clone()));
        }
        match (novo.tecnica, novo.coluna) {
            (Tecnica::TabelaDeDecisao, None) => {
                return Err(ErroModelo::ColunaInvalida(
                    "informe a coluna que este cenário exercita".into(),
                ));
            }
            (_, Some(r)) => {
                let t = self.tabela(r.tabela).ok_or_else(|| {
                    ErroModelo::ColunaInvalida(format!("a tabela TD-{} não existe", r.tabela))
                })?;
                if t.situacao == SituacaoItem::Descartado {
                    return Err(ErroModelo::ColunaInvalida(format!(
                        "a tabela {} foi descartada pelo analista",
                        t.rotulo()
                    )));
                }
                let c = t.coluna(r.coluna).ok_or_else(|| {
                    ErroModelo::ColunaInvalida(format!(
                        "a tabela {} não tem a coluna R{}",
                        t.rotulo(),
                        r.coluna
                    ))
                })?;
                if c.impossivel.is_some() {
                    return Err(ErroModelo::ColunaInvalida(format!(
                        "a coluna {r} é uma combinação impossível e não vira cenário"
                    )));
                }
            }
            _ => {}
        }
        self.aceitar_registro()?;
        self.cenarios.push(Cenario {
            id,
            tecnica: novo.tecnica,
            texto,
            original: None,
            valor: novo
                .valor
                .map(|v| v.trim().to_owned())
                .filter(|v| !v.is_empty()),
            limite,
            coluna: novo.coluna,
            historia: novo.historia,
            fundamentos,
            pergunta: novo.pergunta,
            situacao: SituacaoItem::Pendente,
            sessao: novo.sessao,
            registrado_ms: agora_ms,
        });
        Ok(id)
    }

    /// Decisão do analista sobre um cenário (`Pendente` desfaz).
    pub fn decidir_cenario(
        &mut self,
        id: IdCenario,
        situacao: SituacaoItem,
    ) -> Result<(), ErroModelo> {
        self.exigir_nao_aprovada_cenarios()?;
        let c = self
            .cenarios
            .iter_mut()
            .find(|c| c.id == id)
            .ok_or(ErroModelo::CenarioDesconhecido(id))?;
        c.situacao = situacao;
        Ok(())
    }

    /// Decisão do analista sobre uma tabela de decisão (`Pendente` desfaz).
    pub fn decidir_tabela(
        &mut self,
        numero: u32,
        situacao: SituacaoItem,
    ) -> Result<(), ErroModelo> {
        self.exigir_nao_aprovada_cenarios()?;
        let t = self.tabelas.iter_mut().find(|t| t.numero == numero).ok_or(
            ErroModelo::ItemDesconhecido {
                tipo: "tabela de decisão",
                numero,
            },
        )?;
        t.situacao = situacao;
        Ok(())
    }

    /// Edição do analista no título e nos passos de um cenário; o texto do agente
    /// fica em [`Cenario::original`] na primeira edição. Voltar ao texto original
    /// apaga a marca.
    pub fn editar_cenario(&mut self, id: IdCenario, novo: TextoCenario) -> Result<(), ErroModelo> {
        self.exigir_nao_aprovada_cenarios()?;
        let novo = novo.conferir()?;
        let c = self
            .cenarios
            .iter_mut()
            .find(|c| c.id == id)
            .ok_or(ErroModelo::CenarioDesconhecido(id))?;
        if c.texto == novo {
            return Ok(());
        }
        let original = c.original.take().unwrap_or_else(|| c.texto.clone());
        if original != novo {
            c.original = Some(original);
        }
        c.texto = novo;
        Ok(())
    }

    /// Cenários que entram no documento (não descartados).
    pub fn cenarios_no_documento(&self) -> impl Iterator<Item = &Cenario> {
        self.cenarios.iter().filter(|c| c.no_documento())
    }

    /// Tabelas que entram no documento (não descartadas).
    pub fn tabelas_no_documento(&self) -> impl Iterator<Item = &TabelaDecisao> {
        self.tabelas
            .iter()
            .filter(|t| t.situacao != SituacaoItem::Descartado)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analise::EstadoEvidencia;
    use cba_kb::frontmatter::Linhas;
    use cba_kb::{CaminhoRelativo, Id, Serie};

    fn analise() -> Analise {
        Analise::nova(
            Serie::Analise.id(2),
            "faturamento-exemplo",
            Fluxo::Cenarios,
            "Cenários do cancelamento",
            "Cenários das regras RN-FAT-013 e RN-FAT-014",
            1,
        )
    }

    fn regra() -> Fundamento {
        Fundamento::Regra {
            id: Id::analisar("RN-FAT-013").unwrap(),
            titulo: "Motivo com ao menos 10 caracteres".into(),
            evidencias: vec![],
        }
    }

    fn evidencia(trecho: &str) -> Evidencia {
        Evidencia {
            arquivo: CaminhoRelativo::novo("db/03_pkg_cobranca.sql").unwrap(),
            linhas: Linhas::novo(119, 119).unwrap(),
            trecho: trecho.into(),
            hash: "abc".into(),
            commit: None,
            estado: EstadoEvidencia::Valida,
        }
    }

    fn passos() -> TextoCenario {
        TextoCenario {
            titulo: "Motivo com 9 caracteres".into(),
            passos: Gherkin {
                dado: "uma fatura ABERTA sem pagamento".into(),
                quando: "o atendente cancela com o motivo \"duplicada\" (9 caracteres)".into(),
                entao: "o cancelamento é recusado com o erro -20012".into(),
            },
        }
    }

    fn limite(trecho: &str, configuracao: Option<&str>) -> Limite {
        Limite {
            grandeza: "tamanho do motivo".into(),
            valor: "10".into(),
            evidencias: vec![evidencia(trecho)],
            configuracao: configuracao.map(Into::into),
        }
    }

    fn cenario(tecnica: Tecnica, limite: Option<Limite>) -> NovoCenario {
        NovoCenario {
            tecnica,
            texto: passos(),
            valor: Some("9 caracteres".into()),
            limite,
            coluna: None,
            historia: None,
            fundamentos: vec![regra(), regra()],
            pergunta: None,
            sessao: None,
        }
    }

    fn tabela(colunas: Vec<NovaColuna>) -> NovaTabela {
        NovaTabela {
            titulo: "Cancelamento após 30 dias".into(),
            condicoes: vec![
                Condicao {
                    texto: "emitida há mais de 30 dias".into(),
                    evidencias: vec![],
                },
                Condicao {
                    texto: "perfil SUPERVISOR".into(),
                    evidencias: vec![],
                },
            ],
            acoes: vec!["cancela".into(), "recusa e pede supervisor".into()],
            colunas,
            fundamentos: vec![regra()],
            sessao: None,
        }
    }

    fn col(valores: &str, acoes: &[u32], impossivel: Option<&str>) -> NovaColuna {
        NovaColuna {
            valores: valores
                .split('/')
                .map(|v| ValorCondicao::analisar(v).unwrap())
                .collect(),
            acoes: acoes.to_vec(),
            impossivel: impossivel.map(Into::into),
        }
    }

    #[test]
    fn numeros_do_codigo() {
        assert_eq!(numeros("LENGTH(TRIM(P_MOTIVO)) < 10"), [10.0]);
        assert_eq!(numeros("VALUE 10.00."), [10.0]);
        assert_eq!(numeros("'MAX_PARCELAS', 6.0000"), [6.0]);
        assert_eq!(numeros("R$ 1.000,50 e 0,033"), [1000.5, 0.033]);
        assert_eq!(numeros("sem número"), Vec::<f64>::new());
    }

    #[test]
    fn limite_precisa_aparecer_no_trecho() {
        let mut a = analise();
        let id = a
            .registrar_cenario(
                cenario(
                    Tecnica::ValorLimite,
                    Some(limite("LENGTH(TRIM(P_MOTIVO)) < 10", None)),
                ),
                IdCenario::novo(3),
                5,
            )
            .unwrap();
        assert_eq!(id.to_string(), "CT-0003");
        assert_eq!(a.maior_cenario(), 3);
        assert_eq!(a.cenario(id).unwrap().fundamentos.len(), 1);
        assert_eq!(a.estado, EstadoAnalise::EmAndamento);

        let mut l = limite("LENGTH(TRIM(P_MOTIVO)) < 10", None);
        l.valor = "12".into();
        assert!(matches!(
            a.registrar_cenario(
                cenario(Tecnica::ValorLimite, Some(l)),
                IdCenario::novo(4),
                6
            ),
            Err(ErroModelo::LimiteForaDaEvidencia { .. })
        ));
        assert_eq!(
            a.registrar_cenario(cenario(Tecnica::ValorLimite, None), IdCenario::novo(4), 6),
            Err(ErroModelo::CenarioSemLimite("valor-limite"))
        );
        // Limite de configuração: fora do código, exige pergunta.
        let conf = limite(
            "FN_PARAMETRO('DIAS_CARENCIA')",
            Some("PARAMETRO.DIAS_CARENCIA"),
        );
        assert!(matches!(
            a.registrar_cenario(
                cenario(Tecnica::ValorLimite, Some(conf.clone())),
                IdCenario::novo(4),
                6
            ),
            Err(ErroModelo::LimiteDeConfiguracaoSemPergunta(_))
        ));
        let mut c = cenario(Tecnica::ValorLimite, Some(conf));
        c.pergunta = Some(RefItem::analisar("A-0001/2").unwrap());
        a.registrar_cenario(c, IdCenario::novo(4), 6).unwrap();
        assert!(a.cenario(IdCenario::novo(4)).unwrap().a_confirmar());
        // Negativo não exige limite; repetido é recusado.
        a.registrar_cenario(cenario(Tecnica::Negativo, None), IdCenario::novo(5), 7)
            .unwrap();
        assert_eq!(
            a.registrar_cenario(cenario(Tecnica::Negativo, None), IdCenario::novo(5), 7),
            Err(ErroModelo::CenarioRepetido(IdCenario::novo(5)))
        );
        let mut sem = cenario(Tecnica::Negativo, None);
        sem.fundamentos.clear();
        assert_eq!(
            a.registrar_cenario(sem, IdCenario::novo(6), 7),
            Err(ErroModelo::ItemSemFundamento("cenário de teste"))
        );
    }

    #[test]
    fn limite_sem_numero_compara_o_texto() {
        let l = Limite {
            grandeza: "status da fatura".into(),
            valor: "Aberta".into(),
            evidencias: vec![evidencia("IF V_STATUS <> 'ABERTA' THEN")],
            configuracao: None,
        };
        assert!(l.valor_no_codigo());
    }

    #[test]
    fn tabela_completa_e_sem_sobreposicao() {
        let mut a = analise();
        let n = a
            .registrar_tabela(
                tabela(vec![
                    col("S/S", &[1], None),
                    col("S/N", &[2], None),
                    col("N/-", &[1], None),
                ]),
                2,
            )
            .unwrap();
        assert_eq!(n, 1);
        let t = a.tabela(1).unwrap();
        assert_eq!(t.rotulo(), "TD-1");
        assert_eq!(t.colunas[2].rotulo(), "R3");
        assert!(t.combinacoes_descobertas().is_empty());

        let erro = |colunas| match a.clone().registrar_tabela(tabela(colunas), 3) {
            Err(ErroModelo::TabelaInvalida(m)) => m,
            outro => panic!("{outro:?}"),
        };
        assert!(erro(vec![col("S/S", &[1], None)]).contains("S/N, N/S, N/N"));
        assert!(erro(vec![col("S/-", &[1], None), col("S/N", &[2], None)]).contains("R1 e R2"));
        assert!(erro(vec![col("S", &[1], None)]).contains("um valor por condição"));
        assert!(erro(vec![col("S/S", &[], None)]).contains("nenhuma ação"));
        assert!(erro(vec![col("S/S", &[3], None)]).contains("ação 3"));
        assert!(erro(vec![col("S/S", &[1], Some("x"))]).contains("impossível"));

        let mut uma = tabela(vec![col("S", &[1], None), col("N", &[2], None)]);
        uma.condicoes.pop();
        assert!(matches!(
            a.registrar_tabela(uma, 3),
            Err(ErroModelo::TabelaInvalida(m)) if m.contains("duas ou mais condições")
        ));
        // Impossível com justificativa vale.
        let n = a
            .registrar_tabela(
                tabela(vec![
                    col("S/-", &[2], None),
                    col("N/S", &[], Some("supervisor só entra depois de 30 dias")),
                    col("N/N", &[1], None),
                ]),
                3,
            )
            .unwrap();
        assert_eq!(n, 2);
    }

    #[test]
    fn cenario_de_tabela_aponta_coluna_possivel() {
        let mut a = analise();
        a.registrar_tabela(
            tabela(vec![
                col("S/S", &[1], None),
                col("S/N", &[2], None),
                col("N/S", &[], Some("só supervisor")),
                col("N/N", &[1], None),
            ]),
            2,
        )
        .unwrap();
        let mut c = cenario(Tecnica::TabelaDeDecisao, None);
        assert!(matches!(
            a.registrar_cenario(c.clone(), IdCenario::novo(1), 3),
            Err(ErroModelo::ColunaInvalida(_))
        ));
        c.coluna = RefColuna::analisar("TD-1/R3");
        assert!(matches!(
            a.registrar_cenario(c.clone(), IdCenario::novo(1), 3),
            Err(ErroModelo::ColunaInvalida(m)) if m.contains("impossível")
        ));
        c.coluna = RefColuna::analisar("TD-1/R9");
        assert!(
            a.registrar_cenario(c.clone(), IdCenario::novo(1), 3)
                .is_err()
        );
        c.coluna = RefColuna::analisar("td-1/r2");
        a.registrar_cenario(c, IdCenario::novo(1), 3).unwrap();
        assert_eq!(
            a.cenario(IdCenario::novo(1))
                .unwrap()
                .coluna
                .unwrap()
                .to_string(),
            "TD-1/R2"
        );
    }

    #[test]
    fn decisoes_e_edicao() {
        let mut a = analise();
        let id = a
            .registrar_cenario(cenario(Tecnica::Negativo, None), IdCenario::novo(1), 1)
            .unwrap();
        a.decidir_cenario(id, SituacaoItem::Descartado).unwrap();
        assert_eq!(a.cenarios_no_documento().count(), 0);
        a.decidir_cenario(id, SituacaoItem::Aceito).unwrap();
        let mut t = passos();
        t.titulo = "Motivo curto".into();
        a.editar_cenario(id, t).unwrap();
        assert_eq!(
            a.cenario(id).unwrap().original.as_ref().unwrap().titulo,
            "Motivo com 9 caracteres"
        );
        a.editar_cenario(id, passos()).unwrap();
        assert!(a.cenario(id).unwrap().original.is_none());
        assert_eq!(
            a.decidir_cenario(IdCenario::novo(9), SituacaoItem::Aceito),
            Err(ErroModelo::CenarioDesconhecido(IdCenario::novo(9)))
        );
        a.estado = EstadoAnalise::Aprovada;
        assert!(a.decidir_cenario(id, SituacaoItem::Pendente).is_err());
    }

    #[test]
    fn so_no_fluxo_de_cenarios_e_ida_e_volta_em_json() {
        let mut a = analise();
        a.fluxo = Fluxo::Historias;
        assert!(matches!(
            a.registrar_cenario(cenario(Tecnica::Negativo, None), IdCenario::novo(1), 1),
            Err(ErroModelo::ForaDoFluxoDeCenarios(_))
        ));
        let mut a = analise();
        a.registrar_tabela(
            tabela(vec![
                col("S/S", &[1], None),
                col("S/N", &[2], None),
                col("N/-", &[1], None),
            ]),
            1,
        )
        .unwrap();
        let mut c = cenario(
            Tecnica::ValorLimite,
            Some(limite("LENGTH(TRIM(P_MOTIVO)) < 10", None)),
        );
        c.historia = Some(IdHistoria::novo(2));
        a.registrar_cenario(c, IdCenario::novo(1), 2).unwrap();
        let json = serde_json::to_string(&a).unwrap();
        assert!(json.contains("\"id\":\"CT-0001\""), "{json}");
        assert!(json.contains("\"valores\":[\"S\",\"N\"]"), "{json}");
        assert!(json.contains("\"tecnica\":\"valor_limite\""), "{json}");
        let lida: Analise = serde_json::from_str(&json).unwrap();
        assert_eq!(lida, a);
    }
}
