//! Leitura do gabarito (`fixtures/gabaritos/<projeto>.yaml`).
//!
//! O formato é descrito no próprio arquivo e conferido pela crate `cba-fixtures`.
//! Aqui a leitura é tolerante (campos desconhecidos são ignorados): quem garante a
//! consistência do gabarito é aquele teste.

use std::fs;
use std::path::{Path, PathBuf};

use cba_core::analise::Fluxo;
use serde::{Deserialize, Serialize};

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
    /// Pasta do gabarito, para resolver `raiz`. Preenchida por [`Gabarito::carregar`].
    #[serde(skip)]
    pub pasta: PathBuf,
}

/// Regra de negócio ou inconsistência entre camadas, com as evidências esperadas.
#[derive(Debug, Clone, Deserialize)]
pub struct Regra {
    pub id: String,
    pub titulo: String,
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
#[derive(Debug, Clone, Deserialize)]
pub struct EvidenciaEsperada {
    pub arquivo: String,
    pub linha_inicial: u32,
    pub linha_final: u32,
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
            });
        }
        Err(Erro(format!(
            "o item `{id}` não existe no gabarito (use o id de uma pergunta, funcionalidade ou demanda)"
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
