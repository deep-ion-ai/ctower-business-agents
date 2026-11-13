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
use crate::id::{Id, IdDemanda, IdReuniao, Serie};
use crate::yaml::{Mapping, Value};

/// Tipo de documento, no campo `tipo` do frontmatter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TipoDocumento {
    /// `projeto.md`.
    Projeto,
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
    /// Demanda, em `demandas/D-NNNN-<slug>/demanda.md`.
    Demanda,
    /// Pessoa do catálogo de partes interessadas, em `pessoas/<slug>.md`.
    Pessoa,
    /// Ata de reunião, em `demandas/D-NNNN-<slug>/reunioes/R-NNNN.md`.
    Ata,
}

impl TipoDocumento {
    pub const TODOS: [TipoDocumento; 12] = [
        Self::Projeto,
        Self::AnaliseImpacto,
        Self::Pergunta,
        Self::Funcionalidade,
        Self::CasoDeUso,
        Self::Regra,
        Self::Termo,
        Self::Sugestao,
        Self::MemoriaArea,
        Self::Demanda,
        Self::Pessoa,
        Self::Ata,
    ];

    /// Valor no frontmatter (`analise-impacto`).
    pub fn como_str(self) -> &'static str {
        match self {
            Self::Projeto => "projeto",
            Self::AnaliseImpacto => "analise-impacto",
            Self::Pergunta => "pergunta",
            Self::Funcionalidade => "funcionalidade",
            Self::CasoDeUso => "caso-de-uso",
            Self::Regra => "regra",
            Self::Termo => "termo",
            Self::Sugestao => "sugestao",
            Self::MemoriaArea => "memoria-area",
            Self::Demanda => "demanda",
            Self::Pessoa => "pessoa",
            Self::Ata => "ata",
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

/// Horário dentro de uma reunião, ou uma duração, em segundos. Gravado como
/// `HH:MM:SS` (`00:12:34`); na leitura, aceita também `MM:SS`, o formato curto
/// dos transcripts do Teams.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Horario(pub u32);

impl Horario {
    pub fn de_segundos(segundos: u32) -> Self {
        Self(segundos)
    }

    pub fn segundos(self) -> u32 {
        self.0
    }

    /// Lê `HH:MM:SS`, `H:MM:SS` ou `MM:SS`. Minutos e segundos com dois dígitos,
    /// menores que 60.
    pub fn analisar(texto: &str) -> Option<Self> {
        let partes: Vec<&str> = texto.trim().split(':').collect();
        let numero = |s: &str, max: u32| {
            (!s.is_empty() && s.len() <= 2 && s.bytes().all(|b| b.is_ascii_digit()))
                .then(|| s.parse::<u32>().ok())
                .flatten()
                .filter(|n| *n < max)
        };
        let (h, m, s) = match partes.as_slice() {
            [h, m, s] if m.len() == 2 && s.len() == 2 => {
                (numero(h, 100)?, numero(m, 60)?, numero(s, 60)?)
            }
            [m, s] if s.len() == 2 => (0, numero(m, 60)?, numero(s, 60)?),
            _ => return None,
        };
        Some(Self(h * 3600 + m * 60 + s))
    }
}

impl fmt::Display for Horario {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = self.0;
        write!(f, "{:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    }
}

impl Serialize for Horario {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Horario {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let texto = String::deserialize(d)?;
        Self::analisar(&texto)
            .ok_or_else(|| D::Error::custom(format!("horário inválido `{texto}`; use HH:MM:SS")))
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

/// Estado de uma demanda.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StatusDemanda {
    Aberta,
    Encerrada,
}

/// Estado de uma ata de reunião.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StatusAta {
    Rascunho,
    Aprovada,
}

/// De onde veio a demanda de uma análise e a qual demanda da base ela pertence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Demanda {
    /// Demanda da base (`demandas/D-NNNN-<slug>/`) a que a análise está ligada.
    /// Opcional: as análises podem existir sem demanda.
    #[serde(default)]
    pub id: Option<IdDemanda>,
    /// `texto` para demanda colada pelo analista; no futuro, o rastreador de origem.
    pub origem: String,
    /// Chave no rastreador de origem, quando houver.
    #[serde(default, deserialize_with = "texto_flexivel_opcional")]
    pub chave: Option<String>,
}

/// Versão do código do projeto sobre a qual a análise foi feita.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersaoCodigo {
    #[serde(default, deserialize_with = "texto_flexivel_opcional")]
    pub branch: Option<String>,
    #[serde(default, deserialize_with = "texto_flexivel_opcional")]
    pub commit: Option<String>,
}

/// Evidência gravada numa regra (spec, seção 6.5): caminho relativo à raiz do
/// projeto, intervalo, hash do trecho e commit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evidencia {
    pub arquivo: CaminhoRelativo,
    pub linhas: Linhas,
    #[serde(deserialize_with = "texto_flexivel")]
    pub hash: String,
    #[serde(default, deserialize_with = "texto_flexivel_opcional")]
    pub commit: Option<String>,
}

/// Área funcional declarada no `projeto.md`.
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

/// `projeto.md`: identificação e áreas do projeto. Visão, estado e próximo passo
/// ficam no corpo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrontmatterProjeto {
    pub tipo: TipoDocumento,
    /// Slug do projeto, igual ao nome da pasta (`faturamento-exemplo`).
    pub id: String,
    pub nome: String,
    #[serde(default)]
    pub areas: Vec<Area>,
    #[serde(default)]
    pub criado: Option<Data>,
}

impl Frontmatter for FrontmatterProjeto {
    const TIPOS: &'static [TipoDocumento] = &[TipoDocumento::Projeto];

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
/// ao projeto ou explicação de funcionalidade (documento negocial).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrontmatterAnalise {
    pub tipo: TipoDocumento,
    pub id: Id,
    pub titulo: String,
    /// Slug do projeto.
    pub projeto: String,
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
    /// Documento que originou a regra, relativo à raiz do projeto
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
    pub projeto: String,
    pub status: StatusSugestao,
    /// Análise em que a sugestão foi registrada.
    #[serde(default)]
    pub analise: Option<Id>,
    /// Arquivo do projeto que a sugestão altera, relativo à raiz do projeto.
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
    /// Análise que originou o documento, relativa à raiz do projeto.
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

/// Demanda, em `demandas/D-NNNN-<slug>/demanda.md`: a unidade de trabalho do
/// analista, que reúne as atas e as análises de um mesmo pedido do cliente. A
/// descrição e o próximo passo ficam no corpo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrontmatterDemanda {
    pub tipo: TipoDocumento,
    pub id: IdDemanda,
    pub titulo: String,
    /// Slug do projeto.
    pub projeto: String,
    pub status: StatusDemanda,
    /// Chave no rastreador (`PROJ-123`, `#12`), quando houver.
    #[serde(default, deserialize_with = "texto_flexivel_opcional")]
    pub chave: Option<String>,
    /// Endereço da demanda no rastreador (URL da issue), quando houver.
    #[serde(default, deserialize_with = "texto_flexivel_opcional")]
    pub origem: Option<String>,
    pub criado: Data,
    #[serde(default)]
    pub encerrado: Option<Data>,
}

impl Frontmatter for FrontmatterDemanda {
    const TIPOS: &'static [TipoDocumento] = &[TipoDocumento::Demanda];

    fn validar(&self) -> Result<(), ErroCampo> {
        if !e_slug(&self.projeto) {
            return Err(ErroCampo::novo(
                "projeto",
                format!("`{}` não é um slug (a-z, 0-9 e -)", self.projeto),
            ));
        }
        if self.titulo.trim().is_empty() {
            return Err(ErroCampo::novo("titulo", "a demanda precisa de um título"));
        }
        Ok(())
    }
}

/// Tamanho máximo, em caracteres, do nome e de cada apelido de uma pessoa.
pub const MAXIMO_DO_NOME: usize = 80;
/// Número máximo de palavras do nome e de cada apelido de uma pessoa.
pub const MAXIMO_DE_PALAVRAS_DO_NOME: usize = 8;
/// Tamanho máximo, em caracteres, de um papel.
pub const MAXIMO_DO_PAPEL: usize = 40;
/// Número máximo de palavras de um papel.
pub const MAXIMO_DE_PALAVRAS_DO_PAPEL: usize = 5;
/// Tamanho máximo, em caracteres, da referência a uma reunião.
pub const MAXIMO_DA_REUNIAO: usize = 64;

/// Confere um texto curto de uma linha: nome, apelido ou papel. Os limites de
/// tamanho e de palavras e a recusa de pontuação de frase impedem que uma fala da
/// reunião entre no catálogo de pessoas por engano.
pub(crate) fn conferir_texto_curto(
    campo: &str,
    texto: &str,
    maximo: usize,
    palavras: usize,
) -> Result<(), ErroCampo> {
    let erro = |mensagem: String| Err(ErroCampo::novo(campo, mensagem));
    if !texto.chars().any(char::is_alphanumeric) {
        return erro("precisa ter letras ou dígitos".into());
    }
    if texto.trim() != texto {
        return erro("sem espaços no início nem no fim".into());
    }
    if texto.chars().any(char::is_control) {
        return erro("precisa ter uma linha só".into());
    }
    if let Some(c) = texto
        .chars()
        .find(|c| matches!(c, '?' | '!' | ';' | ':' | '"' | '@'))
    {
        return erro(format!("não aceita `{c}`: guarde só o nome ou o papel"));
    }
    if texto.chars().count() > maximo {
        return erro(format!("tem mais de {maximo} caracteres"));
    }
    if texto.split_whitespace().count() > palavras {
        return erro(format!("tem mais de {palavras} palavras"));
    }
    Ok(())
}

/// Confere a referência a uma reunião: um identificador (`D-0001/R-0002`), não um
/// texto livre.
pub(crate) fn conferir_reuniao(campo: &str, reuniao: &str) -> Result<(), ErroCampo> {
    let valida = !reuniao.is_empty()
        && reuniao.len() <= MAXIMO_DA_REUNIAO
        && reuniao.bytes().any(|b| b.is_ascii_alphanumeric())
        && reuniao
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'/'));
    if valida {
        Ok(())
    } else {
        Err(ErroCampo::novo(
            campo,
            format!(
                "`{reuniao}` não é uma referência de reunião: use até {MAXIMO_DA_REUNIAO} \
                 caracteres A-Z, a-z, 0-9, `-`, `_`, `.` ou `/` (por exemplo, D-0001/R-0002)"
            ),
        ))
    }
}

/// Papel de uma pessoa confirmado pelo analista numa reunião.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PapelConfirmado {
    /// Referência da reunião (`D-0001/R-0002`).
    #[serde(deserialize_with = "texto_flexivel")]
    pub reuniao: String,
    pub papel: String,
    pub confirmado_em: Data,
}

/// Pessoa do catálogo de partes interessadas do projeto, em `pessoas/<slug>.md`.
/// Guarda só o nome, os apelidos e os papéis; nunca as falas das reuniões.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrontmatterPessoa {
    pub tipo: TipoDocumento,
    pub nome: String,
    /// Variações do nome que aparecem nos transcripts ("Silva, Maria", "Maria S.",
    /// "maria.silva"), sem o domínio do e-mail.
    #[serde(default)]
    pub apelidos: Vec<String>,
    /// Papel atual: o confirmado mais recente do histórico ou, sem histórico, o
    /// informado ao registrar a pessoa.
    #[serde(default)]
    pub papel: Option<String>,
    /// Papéis confirmados, um por reunião.
    #[serde(default)]
    pub historico: Vec<PapelConfirmado>,
}

impl FrontmatterPessoa {
    /// O papel confirmado mais recente: a maior data de confirmação e, no mesmo
    /// dia, o último registrado.
    pub fn ultimo_confirmado(&self) -> Option<&PapelConfirmado> {
        self.historico
            .iter()
            .enumerate()
            .max_by_key(|(i, p)| (p.confirmado_em, *i))
            .map(|(_, p)| p)
    }
}

impl Frontmatter for FrontmatterPessoa {
    const TIPOS: &'static [TipoDocumento] = &[TipoDocumento::Pessoa];

    fn validar(&self) -> Result<(), ErroCampo> {
        conferir_texto_curto(
            "nome",
            &self.nome,
            MAXIMO_DO_NOME,
            MAXIMO_DE_PALAVRAS_DO_NOME,
        )?;
        for (i, apelido) in self.apelidos.iter().enumerate() {
            conferir_texto_curto(
                &format!("apelidos[{i}]"),
                apelido,
                MAXIMO_DO_NOME,
                MAXIMO_DE_PALAVRAS_DO_NOME,
            )?;
        }
        if let Some(papel) = &self.papel {
            conferir_texto_curto("papel", papel, MAXIMO_DO_PAPEL, MAXIMO_DE_PALAVRAS_DO_PAPEL)?;
        }
        for (i, p) in self.historico.iter().enumerate() {
            conferir_reuniao(&format!("historico[{i}].reuniao"), &p.reuniao)?;
            conferir_texto_curto(
                &format!("historico[{i}].papel"),
                &p.papel,
                MAXIMO_DO_PAPEL,
                MAXIMO_DE_PALAVRAS_DO_PAPEL,
            )?;
        }
        Ok(())
    }
}

/// Ata de reunião, em `demandas/D-NNNN-<slug>/reunioes/R-NNNN.md`. O documento é
/// montado a partir do estado (`R-NNNN.estado.json`, ao lado), pelo template
/// `ata` de `cba-docs`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrontmatterAta {
    pub tipo: TipoDocumento,
    pub id: IdReuniao,
    /// Slug do projeto.
    pub projeto: String,
    /// Demanda da reunião; a ata fica na pasta dela.
    pub demanda: IdDemanda,
    /// Data da reunião.
    pub data: Data,
    #[serde(default)]
    pub duracao: Option<Horario>,
    pub status: StatusAta,
}

impl Frontmatter for FrontmatterAta {
    const TIPOS: &'static [TipoDocumento] = &[TipoDocumento::Ata];

    fn validar(&self) -> Result<(), ErroCampo> {
        if !e_slug(&self.projeto) {
            return Err(ErroCampo::novo(
                "projeto",
                format!("`{}` não é um slug (a-z, 0-9 e -)", self.projeto),
            ));
        }
        Ok(())
    }
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
    fn horarios() {
        let h: Horario = serde_norway::from_str("\"01:02:03\"").unwrap();
        assert_eq!(h.segundos(), 3723);
        assert_eq!(h.to_string(), "01:02:03");
        assert_eq!(Horario::analisar("12:34").unwrap().to_string(), "00:12:34");
        assert_eq!(Horario::analisar("1:00:00").unwrap().segundos(), 3600);
        for ruim in [
            "",
            "1:2",
            "00:60",
            "00:61:00",
            "a:00",
            "1:00:00:00",
            "123:00:00",
        ] {
            assert!(Horario::analisar(ruim).is_none(), "{ruim}");
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
