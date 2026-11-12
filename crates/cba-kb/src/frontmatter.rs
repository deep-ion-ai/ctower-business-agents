//! Frontmatter tipado por tipo de documento (spec, seções 7.2 e 7.3).
//!
//! Cada tipo de documento tem uma estrutura que implementa [`Frontmatter`]. Os
//! campos seguem a ordem dos exemplos da spec, que é a ordem de gravação de um
//! documento novo. Campos opcionais ausentes são gravados como `null`, para o
//! documento mostrar todos os campos do tipo. Campos que a estrutura não conhece
//! são preservados ao gravar de novo (veja [`crate::Documento`]).
//!
//! Para ler qualquer documento sem conhecer o tipo (o índice, por exemplo), use
//! [`FrontmatterLivre`].

use std::fmt;

use serde::de::{DeserializeOwned, Error as _};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::caminho::{CaminhoRelativo, e_slug};
use crate::id::{Id, Serie};
use crate::yaml::{Mapping, Value};

/// Tipo de documento, no campo `tipo` do frontmatter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TipoDocumento {
    /// `sistema.md`.
    Sistema,
    /// Análise de impacto de uma demanda.
    AnaliseImpacto,
    /// Pergunta ao sistema.
    Pergunta,
    /// Documento negocial da funcionalidade.
    Funcionalidade,
    /// Caso de uso.
    CasoDeUso,
    /// Regra de negócio.
    Regra,
    /// Termo do glossário.
    Termo,
    /// Sugestão de alteração.
    Sugestao,
    /// Memória de uma área funcional.
    MemoriaArea,
}

impl TipoDocumento {
    pub const TODOS: [TipoDocumento; 9] = [
        Self::Sistema,
        Self::AnaliseImpacto,
        Self::Pergunta,
        Self::Funcionalidade,
        Self::CasoDeUso,
        Self::Regra,
        Self::Termo,
        Self::Sugestao,
        Self::MemoriaArea,
    ];

    /// Valor no frontmatter (`analise-impacto`).
    pub fn como_str(self) -> &'static str {
        match self {
            Self::Sistema => "sistema",
            Self::AnaliseImpacto => "analise-impacto",
            Self::Pergunta => "pergunta",
            Self::Funcionalidade => "funcionalidade",
            Self::CasoDeUso => "caso-de-uso",
            Self::Regra => "regra",
            Self::Termo => "termo",
            Self::Sugestao => "sugestao",
            Self::MemoriaArea => "memoria-area",
        }
    }

    pub fn de_str(texto: &str) -> Option<Self> {
        Self::TODOS.into_iter().find(|t| t.como_str() == texto)
    }
}

impl fmt::Display for TipoDocumento {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.como_str())
    }
}

/// Erro de validação num campo do frontmatter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErroCampo {
    /// Caminho do campo, como `status` ou `evidencias[0].linhas`.
    pub campo: String,
    pub mensagem: String,
}

impl ErroCampo {
    pub fn novo(campo: impl Into<String>, mensagem: impl Into<String>) -> Self {
        Self {
            campo: campo.into(),
            mensagem: mensagem.into(),
        }
    }
}

/// Frontmatter de um tipo de documento.
pub trait Frontmatter: Serialize + DeserializeOwned {
    /// Valores aceitos no campo `tipo`. Vazio aceita qualquer documento, mesmo sem
    /// `tipo`.
    const TIPOS: &'static [TipoDocumento];

    /// Ao gravar de novo um documento lido, mantém os campos que a estrutura não
    /// conhece. Só [`FrontmatterLivre`] desliga, porque nela o mapa é o documento
    /// inteiro e remover um campo precisa funcionar.
    const PRESERVA_DESCONHECIDOS: bool = true;

    /// Validações além dos tipos dos campos (por exemplo, a série do `id`).
    fn validar(&self) -> Result<(), ErroCampo> {
        Ok(())
    }
}

fn exigir_serie(id: &Id, esperada: &Serie, nome: &str) -> Result<(), ErroCampo> {
    if id.serie() == esperada {
        Ok(())
    } else {
        Err(ErroCampo::novo(
            "id",
            format!("`{id}` não é um identificador de {nome}"),
        ))
    }
}

fn exigir_regra(id: &Id) -> Result<(), ErroCampo> {
    if matches!(id.serie(), Serie::Regra { .. }) {
        Ok(())
    } else {
        Err(ErroCampo::novo(
            "id",
            format!("`{id}` não é um identificador de regra (RN-<SIGLA>-NNN)"),
        ))
    }
}

// ---------------------------------------------------------------------------
// Tipos de campo

/// Data no formato `AAAA-MM-DD`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Data {
    pub ano: u16,
    pub mes: u8,
    pub dia: u8,
}

impl Data {
    pub fn nova(ano: u16, mes: u8, dia: u8) -> Option<Self> {
        let bissexto =
            (ano.is_multiple_of(4) && !ano.is_multiple_of(100)) || ano.is_multiple_of(400);
        let dias = match mes {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 if bissexto => 29,
            2 => 28,
            _ => return None,
        };
        (1..=dias).contains(&dia).then_some(Self { ano, mes, dia })
    }

    pub fn analisar(texto: &str) -> Option<Self> {
        let b = texto.as_bytes();
        if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
            return None;
        }
        let numero = |s: &str| {
            s.bytes()
                .all(|c| c.is_ascii_digit())
                .then(|| s.parse::<u16>().ok())
                .flatten()
        };
        let ano = numero(&texto[0..4])?;
        let mes = u8::try_from(numero(&texto[5..7])?).ok()?;
        let dia = u8::try_from(numero(&texto[8..10])?).ok()?;
        Self::nova(ano, mes, dia)
    }
}

impl fmt::Display for Data {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.ano, self.mes, self.dia)
    }
}

impl Serialize for Data {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Data {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let texto = String::deserialize(d)?;
        Self::analisar(&texto)
            .ok_or_else(|| D::Error::custom(format!("data inválida `{texto}`; use AAAA-MM-DD")))
    }
}

/// Intervalo de linhas inclusivo, a partir de 1, gravado como `10-12`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Linhas {
    pub inicio: u32,
    pub fim: u32,
}

impl Linhas {
    pub fn novo(inicio: u32, fim: u32) -> Option<Self> {
        (inicio >= 1 && fim >= inicio).then_some(Self { inicio, fim })
    }
}

impl fmt::Display for Linhas {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}", self.inicio, self.fim)
    }
}

impl Serialize for Linhas {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Linhas {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let erro = |t: &str| D::Error::custom(format!("intervalo inválido `{t}`; use 10-12"));
        // Uma linha só pode vir como número (`linhas: 10`).
        let texto = texto_ou_numero(Value::deserialize(d)?).map_err(D::Error::custom)?;
        let (a, b) = texto.split_once('-').unwrap_or((&texto, &texto));
        let a = a.trim().parse().map_err(|_| erro(&texto))?;
        let b = b.trim().parse().map_err(|_| erro(&texto))?;
        Self::novo(a, b).ok_or_else(|| erro(&texto))
    }
}

/// Texto que pode ter sido escrito sem aspas e lido como número pelo YAML (um
/// commit `12345678`, por exemplo). Ao gravar, o texto sai entre aspas quando
/// necessário.
fn texto_ou_numero(valor: Value) -> Result<String, String> {
    match valor {
        Value::String(s) => Ok(s),
        Value::Number(n) => Ok(n.to_string()),
        Value::Bool(b) => Ok(b.to_string()),
        outro => Err(format!("esperado um texto, encontrado {outro:?}")),
    }
}

fn texto_flexivel<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    texto_ou_numero(Value::deserialize(d)?).map_err(D::Error::custom)
}

fn texto_flexivel_opcional<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    match Value::deserialize(d)? {
        Value::Null => Ok(None),
        v => texto_ou_numero(v).map(Some).map_err(D::Error::custom),
    }
}

/// Estado de uma análise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StatusAnalise {
    Rascunho,
    EmRevisao,
    Aprovada,
}

/// Abrangência estimada de uma demanda.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Abrangencia {
    Pequena,
    Media,
    Grande,
}

/// Estado de uma regra.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StatusRegra {
    Confirmada,
    ARevalidar,
    Obsoleta,
}

/// Estado de uma sugestão de alteração.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StatusSugestao {
    Proposta,
    Aceita,
    Recusada,
}

/// De onde veio a demanda de uma análise.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Demanda {
    /// `texto` para demanda colada pelo analista; no futuro, o rastreador de origem.
    pub origem: String,
    /// Chave no rastreador de origem, quando houver.
    #[serde(default, deserialize_with = "texto_flexivel_opcional")]
    pub chave: Option<String>,
}

/// Versão do código do sistema sobre a qual a análise foi feita.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersaoCodigo {
    #[serde(default, deserialize_with = "texto_flexivel_opcional")]
    pub branch: Option<String>,
    #[serde(default, deserialize_with = "texto_flexivel_opcional")]
    pub commit: Option<String>,
}

/// Evidência gravada numa regra (spec, seção 6.5): caminho relativo à raiz do
/// sistema, intervalo, hash do trecho e commit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evidencia {
    pub arquivo: CaminhoRelativo,
    pub linhas: Linhas,
    #[serde(deserialize_with = "texto_flexivel")]
    pub hash: String,
    #[serde(default, deserialize_with = "texto_flexivel_opcional")]
    pub commit: Option<String>,
}

/// Área funcional declarada no `sistema.md`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Area {
    /// Slug da área (`cobranca`), usado nas pastas `areas/` e `funcionalidades/`.
    pub id: String,
    pub nome: String,
    /// Sigla usada nos identificadores de regra (`RN-<SIGLA>-NNN`), se a área tiver.
    #[serde(default)]
    pub sigla: Option<String>,
}

// ---------------------------------------------------------------------------
// Frontmatter por tipo

/// `sistema.md`: identificação e áreas do sistema. Visão, estado e próximo passo
/// ficam no corpo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrontmatterSistema {
    pub tipo: TipoDocumento,
    /// Slug do sistema, igual ao nome da pasta (`faturamento-exemplo`).
    pub id: String,
    pub nome: String,
    #[serde(default)]
    pub areas: Vec<Area>,
    #[serde(default)]
    pub criado: Option<Data>,
}

impl Frontmatter for FrontmatterSistema {
    const TIPOS: &'static [TipoDocumento] = &[TipoDocumento::Sistema];

    fn validar(&self) -> Result<(), ErroCampo> {
        if !e_slug(&self.id) {
            return Err(ErroCampo::novo(
                "id",
                format!("`{}` não é um slug (a-z, 0-9 e -)", self.id),
            ));
        }
        for (i, area) in self.areas.iter().enumerate() {
            if !e_slug(&area.id) {
                return Err(ErroCampo::novo(
                    format!("areas[{i}].id"),
                    format!("`{}` não é um slug (a-z, 0-9 e -)", area.id),
                ));
            }
            if let Some(sigla) = &area.sigla {
                Serie::regra(sigla)
                    .map_err(|e| ErroCampo::novo(format!("areas[{i}].sigla"), e.motivo))?;
            }
        }
        Ok(())
    }
}

/// Documento de uma análise, em `analises/<id>.md`: análise de impacto, pergunta
/// ao sistema ou explicação de funcionalidade (documento negocial).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrontmatterAnalise {
    pub tipo: TipoDocumento,
    pub id: Id,
    pub titulo: String,
    /// Slug do sistema.
    pub sistema: String,
    pub demanda: Demanda,
    pub status: StatusAnalise,
    #[serde(default)]
    pub abrangencia: Option<Abrangencia>,
    #[serde(default)]
    pub base: Option<VersaoCodigo>,
    pub criado: Data,
    #[serde(default)]
    pub analista: Option<String>,
}

impl Frontmatter for FrontmatterAnalise {
    const TIPOS: &'static [TipoDocumento] = &[
        TipoDocumento::AnaliseImpacto,
        TipoDocumento::Pergunta,
        TipoDocumento::Funcionalidade,
    ];

    fn validar(&self) -> Result<(), ErroCampo> {
        exigir_serie(&self.id, &Serie::Analise, "análise (A-NNNN)")
    }
}

/// Regra de negócio, em `regras/RN-<SIGLA>-<NNN>.md`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrontmatterRegra {
    pub tipo: TipoDocumento,
    pub id: Id,
    pub titulo: String,
    /// Slug da área.
    pub area: String,
    pub status: StatusRegra,
    #[serde(default)]
    pub evidencias: Vec<Evidencia>,
    /// Documento que originou a regra, relativo à raiz do sistema
    /// (`analises/A-0193.md`).
    #[serde(default)]
    pub origem: Option<CaminhoRelativo>,
}

impl Frontmatter for FrontmatterRegra {
    const TIPOS: &'static [TipoDocumento] = &[TipoDocumento::Regra];

    fn validar(&self) -> Result<(), ErroCampo> {
        exigir_regra(&self.id)
    }
}

/// Termo do glossário, em `glossario/<termo>.md`. A definição e onde o termo vive
/// no código ficam no corpo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrontmatterTermo {
    pub tipo: TipoDocumento,
    pub termo: String,
    #[serde(default)]
    pub sinonimos: Vec<String>,
    #[serde(default)]
    pub area: Option<String>,
    #[serde(default)]
    pub origem: Option<CaminhoRelativo>,
}

impl Frontmatter for FrontmatterTermo {
    const TIPOS: &'static [TipoDocumento] = &[TipoDocumento::Termo];
}

/// Sugestão de alteração, em `sugestoes/<id>.md`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrontmatterSugestao {
    pub tipo: TipoDocumento,
    pub id: Id,
    pub titulo: String,
    pub sistema: String,
    pub status: StatusSugestao,
    /// Análise em que a sugestão foi registrada.
    #[serde(default)]
    pub analise: Option<Id>,
    /// Arquivo do sistema que a sugestão altera, relativo à raiz do sistema.
    #[serde(default)]
    pub arquivo: Option<CaminhoRelativo>,
    pub criado: Data,
}

impl Frontmatter for FrontmatterSugestao {
    const TIPOS: &'static [TipoDocumento] = &[TipoDocumento::Sugestao];

    fn validar(&self) -> Result<(), ErroCampo> {
        exigir_serie(&self.id, &Serie::Sugestao, "sugestão (S-NNNN)")?;
        match &self.analise {
            Some(a) if a.serie() != &Serie::Analise => Err(ErroCampo::novo(
                "analise",
                format!("`{a}` não é um identificador de análise (A-NNNN)"),
            )),
            _ => Ok(()),
        }
    }
}

/// Documento negocial ou caso de uso, em `funcionalidades/<area>/<tema>/`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrontmatterFuncionalidade {
    pub tipo: TipoDocumento,
    pub titulo: String,
    pub area: String,
    pub tema: String,
    pub status: StatusAnalise,
    /// Análise que originou o documento, relativa à raiz do sistema.
    #[serde(default)]
    pub origem: Option<CaminhoRelativo>,
}

impl Frontmatter for FrontmatterFuncionalidade {
    const TIPOS: &'static [TipoDocumento] =
        &[TipoDocumento::Funcionalidade, TipoDocumento::CasoDeUso];
}

/// Memória de uma área, em `areas/<area>/memoria.md`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrontmatterMemoria {
    pub tipo: TipoDocumento,
    pub area: String,
    pub titulo: String,
    #[serde(default)]
    pub atualizado: Option<Data>,
}

impl Frontmatter for FrontmatterMemoria {
    const TIPOS: &'static [TipoDocumento] = &[TipoDocumento::MemoriaArea];
}

/// Frontmatter sem tipo fixo: o mapa YAML na ordem do arquivo. Serve para ler
/// qualquer documento da base (índice, migrações, templates de organizações).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FrontmatterLivre(pub Mapping);

impl FrontmatterLivre {
    /// Tipo do documento, se o campo `tipo` existir e for conhecido.
    pub fn tipo(&self) -> Option<TipoDocumento> {
        self.0
            .get("tipo")
            .and_then(Value::as_str)
            .and_then(TipoDocumento::de_str)
    }
}

impl Frontmatter for FrontmatterLivre {
    const TIPOS: &'static [TipoDocumento] = &[];
    const PRESERVA_DESCONHECIDOS: bool = false;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tipos_ida_e_volta() {
        for t in TipoDocumento::TODOS {
            assert_eq!(TipoDocumento::de_str(t.como_str()), Some(t));
            let yaml = serde_norway::to_string(&t).unwrap();
            assert_eq!(yaml.trim(), t.como_str());
        }
    }

    #[test]
    fn datas() {
        assert_eq!(
            Data::analisar("2026-11-11").unwrap().to_string(),
            "2026-11-11"
        );
        assert!(Data::analisar("2024-02-29").is_some());
        for ruim in [
            "2026-02-29",
            "2026-13-01",
            "2026-1-1",
            "2026/11/11",
            "+026-11-11",
        ] {
            assert!(Data::analisar(ruim).is_none(), "{ruim}");
        }
    }

    #[test]
    fn linhas() {
        let l: Linhas = serde_norway::from_str("10-12").unwrap();
        assert_eq!(l, Linhas::novo(10, 12).unwrap());
        let l: Linhas = serde_norway::from_str("7").unwrap();
        assert_eq!(l.to_string(), "7-7");
        assert!(serde_norway::from_str::<Linhas>("12-10").is_err());
        assert!(serde_norway::from_str::<Linhas>("0-1").is_err());
    }
}
