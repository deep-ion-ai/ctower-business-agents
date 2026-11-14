//! Leitura do gabarito (`fixtures/gabaritos/<projeto>.yaml`).
//!
//! O formato é descrito no próprio arquivo e conferido pela crate `cba-fixtures`.
//! Aqui a leitura é tolerante (campos desconhecidos são ignorados): quem garante a
//! consistência do gabarito é aquele teste.

use std::fs;
use std::path::{Path, PathBuf};

use cba_core::analise::Fluxo;
use cba_docs::TipoItem;
use serde::{Deserialize, Serialize};

use crate::texto;

/// Gabarito de um sistema fictício.
#[derive(Debug, Clone, Deserialize)]
pub struct Gabarito {
    pub projeto: String,
    /// Pasta do projeto, relativa ao gabarito (resolvida em [`Gabarito::raiz_do_projeto`]).
    pub raiz: String,
    pub regras: Vec<Regra>,
    #[serde(default)]
    pub inconsistencias: Vec<Regra>,
    #[serde(default)]
    pub perguntas: Vec<Pergunta>,
    #[serde(default)]
    pub funcionalidades: Vec<Funcionalidade>,
    #[serde(default)]
    pub demandas: Vec<Demanda>,
    /// Reuniões fictícias e a ata esperada de cada uma (fluxo `reuniao`).
    #[serde(default)]
    pub reunioes: Vec<Reuniao>,
    /// Requisitos e histórias com problemas plantados (fluxo `revisar`, #116).
    #[serde(default)]
    pub revisoes: Vec<Revisao>,
    /// Histórias com critérios de aceite esperadas a partir de uma demanda (fluxo
    /// `historias`, #115).
    #[serde(default)]
    pub historias: Vec<Historias>,
    /// Cenários de teste esperados a partir de regras (fluxo `cenarios`, #117).
    #[serde(default)]
    pub cenarios: Vec<Cenarios>,
    /// Pasta do gabarito, para resolver `raiz`. Preenchida por [`Gabarito::carregar`].
    #[serde(skip)]
    pub pasta: PathBuf,
}

/// Regra de negócio ou inconsistência entre camadas, com as evidências esperadas.
#[derive(Debug, Clone, Deserialize)]
pub struct Regra {
    pub id: String,
    pub titulo: String,
    /// Enunciado da regra (para preparar a análise de origem das histórias).
    #[serde(default)]
    pub enunciado: String,
    pub evidencias: Vec<EvidenciaEsperada>,
}

impl Regra {
    /// Inconsistências têm id `INC-*` e só contam como encontradas quando uma
    /// afirmação cita os dois lados (veja [`crate::comparacao`]).
    pub fn e_inconsistencia(&self) -> bool {
        self.id.starts_with("INC-")
    }
}

/// Evidência esperada: arquivo relativo à raiz do projeto e intervalo inclusivo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenciaEsperada {
    pub arquivo: String,
    pub linha_inicial: u32,
    pub linha_final: u32,
    /// Texto curto que aparece no intervalo (para citar a evidência no preparo).
    #[serde(default)]
    pub trecho: String,
}

/// Pergunta que o agente deve registrar para o solicitante.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerguntaEsperada {
    pub tema: String,
    /// Grupos de termos: cada grupo precisa de ao menos um termo no texto
    /// ([`crate::texto::atende`]).
    pub palavras_chave: Vec<Vec<String>>,
}

/// Veredito esperado para a premissa do solicitante.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Veredito {
    Confere,
    NaoConfere,
    ConfereEmParte,
}

/// Como o agente deve julgar a suposição do solicitante.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificacaoDaPremissa {
    pub veredito: Veredito,
    pub palavras_chave: Vec<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Pergunta {
    pub id: String,
    pub pergunta: String,
    pub regras: Vec<String>,
    #[serde(default)]
    pub perguntas_esperadas: Vec<PerguntaEsperada>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Funcionalidade {
    pub id: String,
    pub nome: String,
    pub regras: Vec<String>,
    #[serde(default)]
    pub arquivos: Vec<String>,
    #[serde(default)]
    pub perguntas_esperadas: Vec<PerguntaEsperada>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Demanda {
    pub id: String,
    pub texto: String,
    pub abrangencia_esperada: String,
    #[serde(default)]
    pub verificacao_da_premissa: Option<VerificacaoDaPremissa>,
    pub regras: Vec<String>,
    #[serde(default)]
    pub arquivos_impactados: Vec<String>,
    #[serde(default)]
    pub perguntas_esperadas: Vec<PerguntaEsperada>,
}

/// Requisito ou história fictícios e o parecer esperado (fluxo `revisar`).
#[derive(Debug, Clone, Deserialize)]
pub struct Revisao {
    pub id: String,
    pub titulo: String,
    pub texto: String,
    #[serde(flatten)]
    pub esperado: RevisaoEsperada,
    #[serde(default)]
    pub perguntas_esperadas: Vec<PerguntaEsperada>,
}

/// O que o parecer da revisão deve ter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevisaoEsperada {
    pub veredito: cba_core::parecer::Veredito,
    /// Termos da lista de referência (`cba_core::qualidade::TERMOS_VAGOS`) que
    /// devem ter achado de ambiguidade ou verificabilidade.
    #[serde(default)]
    pub termos_vagos: Vec<String>,
    pub achados: Vec<AchadoEsperado>,
}

/// Achado esperado no parecer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AchadoEsperado {
    pub id: String,
    pub caracteristica: cba_core::qualidade::Caracteristica,
    /// Outras características aceitas.
    #[serde(default)]
    pub tambem_em: Vec<cba_core::qualidade::Caracteristica>,
    pub descricao: String,
    /// Regras do gabarito que o achado toca (pela evidência ou pelo id).
    #[serde(default)]
    pub regras: Vec<String>,
    pub palavras_chave: Vec<Vec<String>>,
}

/// Histórias esperadas a partir da análise de impacto de uma demanda (fluxo
/// `historias`, #115).
#[derive(Debug, Clone, Deserialize)]
pub struct Historias {
    pub id: String,
    pub titulo: String,
    /// O que o analista pede nas histórias (opcional).
    #[serde(default)]
    pub pedido: Option<String>,
    #[serde(flatten)]
    pub esperado: HistoriasEsperadas,
}

/// O que as histórias devem ter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoriasEsperadas {
    /// Demanda do gabarito (`D-FAT-003`) cuja análise de impacto é a origem: o
    /// preparo cria a análise de origem com uma afirmação confirmada por regra
    /// da demanda (o enunciado e as evidências do gabarito) e uma pergunta em
    /// aberto por pergunta esperada, e grava as regras na base.
    pub demanda: String,
    /// Regras que os critérios devem citar (pela regra ou pela afirmação com a
    /// evidência da regra).
    pub regras: Vec<String>,
    /// Se a abrangência pede a quebra em fatias (uma proposta de quebra, uma
    /// fatia ou mais de uma história valem).
    #[serde(default)]
    pub quebra: bool,
    pub historias_esperadas: Vec<HistoriaEsperada>,
    /// Temas que devem ficar "a confirmar" (critério com pergunta), com
    /// palavras-chave conferidas no texto do critério.
    #[serde(default)]
    pub a_confirmar: Vec<PerguntaEsperada>,
}

/// História esperada, pelas palavras-chave do título e da narrativa.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoriaEsperada {
    pub id: String,
    pub descricao: String,
    pub palavras_chave: Vec<Vec<String>>,
}

/// Cenários de teste esperados a partir das regras pedidas (fluxo `cenarios`,
/// #117).
#[derive(Debug, Clone, Deserialize)]
pub struct Cenarios {
    pub id: String,
    pub titulo: String,
    /// O pedido do analista, com as regras a cobrir.
    pub pedido: String,
    #[serde(flatten)]
    pub esperado: CenariosEsperados,
}

/// O que os cenários devem ter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CenariosEsperados {
    /// Regras a cobrir: o preparo as grava na base, com as evidências do gabarito.
    pub regras: Vec<String>,
    #[serde(default)]
    pub limites: Vec<LimiteEsperado>,
    #[serde(default)]
    pub tabelas: Vec<TabelaEsperada>,
    #[serde(default)]
    pub negativos: Vec<NegativoEsperado>,
    /// Temas que devem ficar "a confirmar" (cenário com pergunta), com
    /// palavras-chave no título, nos passos, no valor e no limite.
    #[serde(default)]
    pub a_confirmar: Vec<PerguntaEsperada>,
}

/// Limite que o agente deve ler do código.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimiteEsperado {
    pub id: String,
    pub regra: String,
    pub grandeza: String,
    /// O limite como está no código.
    pub valor: f64,
    /// O limite e o vizinho do outro lado da comparação.
    pub valores_testados: Vec<f64>,
    pub evidencia: EvidenciaEsperada,
}

/// Tabela de decisão esperada.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabelaEsperada {
    pub id: String,
    pub regra: String,
    pub descricao: String,
    /// Número mínimo de condições.
    pub condicoes: usize,
    /// Palavras-chave no texto das condições.
    pub palavras_chave: Vec<Vec<String>>,
}

/// Cenário negativo esperado.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NegativoEsperado {
    pub id: String,
    pub regra: String,
    pub descricao: String,
    pub palavras_chave: Vec<Vec<String>>,
}

/// Reunião fictícia: o transcript e a ata esperada.
#[derive(Debug, Clone, Deserialize)]
pub struct Reuniao {
    pub id: String,
    pub titulo: String,
    /// Transcript, relativo ao gabarito (resolvido em [`Gabarito::transcript`]).
    pub transcript: String,
    /// Título da demanda a que a reunião pertence.
    pub demanda: String,
    pub participantes: Vec<ParticipanteEsperado>,
    pub itens: Vec<ItemEsperado>,
}

/// Papel esperado de um participante.
#[derive(Debug, Clone, Deserialize)]
pub struct ParticipanteEsperado {
    /// Nome como aparece no transcript.
    pub nome: String,
    /// Papel do catálogo padrão.
    pub papel: String,
    /// Outros papéis que também valem.
    #[serde(default)]
    pub tambem_aceitos: Vec<String>,
}

impl ParticipanteEsperado {
    /// O papel proposto é o esperado ou um dos aceitos (sem acentos nem maiúsculas).
    pub fn aceita(&self, papel: &str) -> bool {
        let proposto = texto::normalizar(papel.trim());
        std::iter::once(&self.papel)
            .chain(&self.tambem_aceitos)
            .any(|p| texto::normalizar(p.trim()) == proposto)
    }
}

/// Item esperado da ata.
#[derive(Debug, Clone, Deserialize)]
pub struct ItemEsperado {
    pub id: String,
    pub tipo: TipoItem,
    /// Outras seções em que o item também vale (uma decisão registrada como regra).
    #[serde(default)]
    pub tambem_em: Vec<TipoItem>,
    pub descricao: String,
    /// Grupos de termos ([`crate::texto::atende`]) conferidos no texto do item.
    pub palavras_chave: Vec<Vec<String>>,
    /// Pendência: grupos de termos conferidos no responsável registrado.
    #[serde(default)]
    pub responsavel: Vec<Vec<String>>,
    /// Pendência: grupos de termos conferidos no prazo registrado.
    #[serde(default)]
    pub prazo: Vec<Vec<String>>,
    /// Início (`M:SS`) das falas que sustentam o item, no transcript da reunião.
    #[serde(default)]
    pub falas: Vec<String>,
}

impl ItemEsperado {
    /// O item vale na seção do `tipo`.
    pub fn aceita_tipo(&self, tipo: TipoItem) -> bool {
        self.tipo == tipo || self.tambem_em.contains(&tipo)
    }
}

/// Um exemplo de um dos três fluxos, já no formato que a avaliação usa.
#[derive(Debug, Clone, Serialize)]
pub struct Item {
    pub id: String,
    pub fluxo: Fluxo,
    /// Título da análise criada para o exemplo.
    pub titulo: String,
    /// Entrada do analista: a pergunta, o nome da funcionalidade ou o texto da demanda.
    pub entrada: String,
    /// Regras e inconsistências esperadas (ids do gabarito).
    pub regras: Vec<String>,
    /// Arquivos que a análise deve citar (vazio no fluxo perguntar).
    pub arquivos: Vec<String>,
    pub perguntas_esperadas: Vec<PerguntaEsperada>,
    pub verificacao_da_premissa: Option<VerificacaoDaPremissa>,
    /// Abrangência esperada (`pequena`, `media` ou `grande`), só no fluxo impacto.
    pub abrangencia: Option<String>,
    /// Parecer esperado, só no fluxo revisar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revisao: Option<RevisaoEsperada>,
    /// Histórias esperadas e a demanda de origem, só no fluxo historias.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub historias: Option<HistoriasEsperadas>,
    /// Cenários esperados, só no fluxo cenarios.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cenarios: Option<CenariosEsperados>,
}

/// Erro ao ler o gabarito ou ao achar um item.
#[derive(Debug)]
pub struct Erro(pub String);

impl std::fmt::Display for Erro {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Erro {}

impl Gabarito {
    /// Lê o gabarito de `caminho`.
    pub fn carregar(caminho: &Path) -> Result<Self, Erro> {
        let texto = fs::read_to_string(caminho)
            .map_err(|e| Erro(format!("não foi possível ler {}: {e}", caminho.display())))?;
        let mut g =
            Self::do_texto(&texto).map_err(|e| Erro(format!("{}: {}", caminho.display(), e.0)))?;
        g.pasta = caminho
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        Ok(g)
    }

    /// Lê o gabarito de um texto YAML (sem a pasta, que fica `.`).
    pub fn do_texto(texto: &str) -> Result<Self, Erro> {
        let mut g: Self =
            serde_norway::from_str(texto).map_err(|e| Erro(format!("YAML inválido: {e}")))?;
        g.pasta = PathBuf::from(".");
        Ok(g)
    }

    /// Pasta do sistema analisado.
    pub fn raiz_do_projeto(&self) -> PathBuf {
        self.raiz
            .split('/')
            .fold(self.pasta.clone(), |acc, parte| acc.join(parte))
    }

    /// Reunião pelo id (`REU-*`).
    pub fn reuniao(&self, id: &str) -> Result<&Reuniao, Erro> {
        self.reunioes.iter().find(|r| r.id == id).ok_or_else(|| {
            Erro(format!(
                "a reunião `{id}` não existe no gabarito (use o id de uma reunião, como REU-FAT-001)"
            ))
        })
    }

    /// Caminho do transcript da reunião.
    pub fn transcript(&self, reuniao: &Reuniao) -> PathBuf {
        reuniao
            .transcript
            .split('/')
            .fold(self.pasta.clone(), |acc, parte| acc.join(parte))
    }

    /// Regra ou inconsistência pelo id.
    pub fn regra(&self, id: &str) -> Option<&Regra> {
        self.regras
            .iter()
            .chain(&self.inconsistencias)
            .find(|r| r.id == id)
    }

    /// Todas as regras e inconsistências.
    pub fn todas_as_regras(&self) -> impl Iterator<Item = &Regra> {
        self.regras.iter().chain(&self.inconsistencias)
    }

    /// Item de um fluxo pelo id (`P-*`, `F-*` ou `D-*`).
    pub fn item(&self, id: &str) -> Result<Item, Erro> {
        if let Some(p) = self.perguntas.iter().find(|p| p.id == id) {
            return Ok(Item {
                id: p.id.clone(),
                fluxo: Fluxo::Perguntar,
                titulo: p.pergunta.clone(),
                entrada: p.pergunta.clone(),
                regras: p.regras.clone(),
                arquivos: Vec::new(),
                perguntas_esperadas: p.perguntas_esperadas.clone(),
                verificacao_da_premissa: None,
                abrangencia: None,
                revisao: None,
                historias: None,
                cenarios: None,
            });
        }
        if let Some(f) = self.funcionalidades.iter().find(|f| f.id == id) {
            return Ok(Item {
                id: f.id.clone(),
                fluxo: Fluxo::Explicar,
                titulo: f.nome.clone(),
                entrada: f.nome.clone(),
                regras: f.regras.clone(),
                arquivos: f.arquivos.clone(),
                perguntas_esperadas: f.perguntas_esperadas.clone(),
                verificacao_da_premissa: None,
                abrangencia: None,
                revisao: None,
                historias: None,
                cenarios: None,
            });
        }
        if let Some(d) = self.demandas.iter().find(|d| d.id == id) {
            return Ok(Item {
                id: d.id.clone(),
                fluxo: Fluxo::Impacto,
                titulo: titulo_da_demanda(&d.texto),
                entrada: d.texto.clone(),
                regras: d.regras.clone(),
                arquivos: d.arquivos_impactados.clone(),
                perguntas_esperadas: d.perguntas_esperadas.clone(),
                verificacao_da_premissa: d.verificacao_da_premissa.clone(),
                abrangencia: Some(d.abrangencia_esperada.clone()),
                revisao: None,
                historias: None,
                cenarios: None,
            });
        }
        if let Some(r) = self.revisoes.iter().find(|r| r.id == id) {
            return Ok(Item {
                id: r.id.clone(),
                fluxo: Fluxo::Revisar,
                titulo: r.titulo.clone(),
                entrada: r.texto.clone(),
                // As regras da revisão ficam nos achados esperados.
                regras: Vec::new(),
                arquivos: Vec::new(),
                perguntas_esperadas: r.perguntas_esperadas.clone(),
                verificacao_da_premissa: None,
                abrangencia: None,
                revisao: Some(r.esperado.clone()),
                historias: None,
                cenarios: None,
            });
        }
        if let Some(h) = self.historias.iter().find(|h| h.id == id) {
            let demanda = self.demandas.iter().find(|d| d.id == h.esperado.demanda);
            return Ok(Item {
                id: h.id.clone(),
                fluxo: Fluxo::Historias,
                titulo: h.titulo.clone(),
                entrada: h.pedido.clone().unwrap_or_default(),
                regras: h.esperado.regras.clone(),
                arquivos: Vec::new(),
                perguntas_esperadas: Vec::new(),
                verificacao_da_premissa: None,
                abrangencia: demanda.map(|d| d.abrangencia_esperada.clone()),
                revisao: None,
                historias: Some(h.esperado.clone()),
                cenarios: None,
            });
        }
        if let Some(c) = self.cenarios.iter().find(|c| c.id == id) {
            return Ok(Item {
                id: c.id.clone(),
                fluxo: Fluxo::Cenarios,
                titulo: c.titulo.clone(),
                entrada: c.pedido.clone(),
                regras: c.esperado.regras.clone(),
                arquivos: Vec::new(),
                perguntas_esperadas: Vec::new(),
                verificacao_da_premissa: None,
                abrangencia: None,
                revisao: None,
                historias: None,
                cenarios: Some(c.esperado.clone()),
            });
        }
        Err(Erro(format!(
            "o item `{id}` não existe no gabarito (use o id de uma pergunta, funcionalidade, \
             demanda, revisão, histórias ou cenários)"
        )))
    }
}

/// Título curto da análise de uma demanda: a primeira frase, até 80 caracteres.
fn titulo_da_demanda(texto: &str) -> String {
    let frase = texto.split('.').next().unwrap_or(texto).trim();
    let mut titulo: String = frase.chars().take(80).collect();
    if frase.chars().count() > 80 {
        titulo.push('…');
    }
    titulo
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titulo_da_demanda_e_a_primeira_frase() {
        assert_eq!(
            titulo_da_demanda("Reduzir a multa de 2% para 1%. Salvo engano, basta."),
            "Reduzir a multa de 2% para 1%"
        );
        let longo = "a".repeat(100);
        assert_eq!(titulo_da_demanda(&longo).chars().count(), 81);
    }
}
