//! Premissas, decisões, riscos e restrições de uma demanda (#122): um documento
//! por registro, na pasta da demanda.
//!
//! ```text
//! demandas/D-NNNN-<slug>/registros/
//!   PR-0001.md            # premissa
//!   DE-0001.md            # decisão
//!   RI-0001.md            # risco
//!   RE-0001.md            # restrição
//!   recusadas.json        # propostas do agente recusadas pelo analista
//! ```
//!
//! ```yaml
//! tipo: decisao
//! id: DE-0001
//! titulo: Máximo de 3 parcelas; acima disso, só com aprovação do financeiro.
//! projeto: faturamento-exemplo
//! demanda: D-0001
//! criado: 2026-11-10
//! quem: Maria Silva
//! pessoa: maria-silva
//! origem: demandas/D-0001-parcelamento-pj/reunioes/R-0001.md
//! referencia: R-0001/D-1
//! decidido_em: 2026-11-10
//! alternativas: [6 parcelas, sem limite]
//! historico:
//!   - em: 2026-11-10
//!     evento: aceita a proposta do agente (R-0001/D-1)
//! ```
//!
//! - **Identificador estável:** `PR-NNNN`, `DE-NNNN`, `RI-NNNN` e `RE-NNNN`, cada
//!   série sequencial por projeto (não por demanda), como as reuniões, e nunca
//!   reaproveitada. A reserva sem colisão usa uma trava em `demandas/`, como em
//!   [`Projeto::criar_reuniao`].
//! - **Histórico:** cada mudança acrescenta um evento em `historico`, com a data.
//!   A base versionada em git guarda o resto.
//! - **Quem:** `quem` é o nome como o analista quer ver (quem assumiu a premissa,
//!   quem decidiu, o responsável pelo risco, quem informou a restrição); `pessoa`
//!   é o slug do catálogo de pessoas, quando a pessoa está nele. O índice liga o
//!   registro à pessoa (`pessoas/<slug>.md`, campo `pessoa`).
//! - **Origem:** `origem` é o documento de onde o registro veio (a ata ou a
//!   análise), relativo à raiz do projeto, e `referencia` o item dentro dele
//!   (`R-0001/D-1`, o item `D-1` da ata; `A-0001/3`, a afirmação 3 da análise).
//!   Registro criado à mão não tem origem. O índice liga o registro à origem
//!   (campo `origem`) e à demanda (campo `demanda`).
//! - **Validar premissa:** [`Projeto::validar_premissa`] muda o status e grava
//!   como foi validada (e, quando houver, a pergunta que a validou). É a operação
//!   que a pauta de perguntas (#123) usa ao receber a resposta.
//!
//! As propostas do agente (das análises de impacto e das atas) não ficam aqui:
//! quem as deriva é `cba-tools`. Aceitar uma cria o registro com a `referencia`;
//! recusar grava a referência em `recusadas.json` ([`Projeto::recusar_proposta`]).

use std::fmt;
use std::fs;
use std::io;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::arquivo;
use crate::base::Projeto;
use crate::caminho::{CaminhoRelativo, e_slug};
use crate::demandas::pasta_das_demandas;
use crate::documento::Documento;
use crate::erro::{Erro, Resultado};
use crate::frontmatter::{Data, ErroCampo, Frontmatter, TipoDocumento};
use crate::id::{IdDemanda, IdInvalido};
use crate::layout;

/// Pasta dos registros, dentro da pasta da demanda.
pub const PASTA: &str = "registros";
/// Propostas recusadas, na pasta dos registros (fora do índice: não é `.md`).
pub const ARQUIVO_RECUSADAS: &str = "recusadas.json";
/// Tamanho máximo, em caracteres, do enunciado (`titulo`) de um registro.
pub const MAXIMO_DO_TITULO: usize = 300;
/// Quantos números seguidos [`Projeto::criar_registro`] tenta antes de desistir.
const TENTATIVAS_DE_RESERVA: u32 = 1000;
/// Sufixo da pasta oculta que trava um número durante a criação.
const SUFIXO_DA_TRAVA: &str = ".reserva";

// ---------------------------------------------------------------------------
// Categoria e identificador

/// Categoria de um registro da demanda.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Categoria {
    Premissa,
    Decisao,
    Risco,
    Restricao,
}

impl Categoria {
    pub const TODAS: [Categoria; 4] = [Self::Premissa, Self::Decisao, Self::Risco, Self::Restricao];

    /// Prefixo do identificador (`PR` em `PR-0001`).
    pub fn prefixo(self) -> &'static str {
        match self {
            Self::Premissa => "PR",
            Self::Decisao => "DE",
            Self::Risco => "RI",
            Self::Restricao => "RE",
        }
    }

    pub fn tipo(self) -> TipoDocumento {
        match self {
            Self::Premissa => TipoDocumento::Premissa,
            Self::Decisao => TipoDocumento::Decisao,
            Self::Risco => TipoDocumento::Risco,
            Self::Restricao => TipoDocumento::Restricao,
        }
    }

    pub fn do_tipo(tipo: TipoDocumento) -> Option<Self> {
        Self::TODAS.into_iter().find(|c| c.tipo() == tipo)
    }

    /// Nome para mensagens (`premissa`, `decisão`).
    pub fn nome(self) -> &'static str {
        match self {
            Self::Premissa => "premissa",
            Self::Decisao => "decisão",
            Self::Risco => "risco",
            Self::Restricao => "restrição",
        }
    }
}

/// Identificador estável de um registro: prefixo da categoria e número com 4
/// dígitos (`PR-0001`, `DE-0012`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IdRegistro {
    pub categoria: Categoria,
    pub numero: u32,
}

impl IdRegistro {
    pub fn novo(categoria: Categoria, numero: u32) -> Self {
        Self {
            categoria,
            numero: numero.max(1),
        }
    }

    /// Lê `PR-0001`, só na forma canônica.
    pub fn analisar(texto: &str) -> Result<Self, IdInvalido> {
        let erro = |motivo| IdInvalido {
            valor: texto.to_owned(),
            motivo,
        };
        let (prefixo, numero) = texto
            .split_once('-')
            .ok_or_else(|| erro("esperado PR-NNNN, DE-NNNN, RI-NNNN ou RE-NNNN"))?;
        let categoria = Categoria::TODAS
            .into_iter()
            .find(|c| c.prefixo() == prefixo)
            .ok_or_else(|| erro("esperado PR-NNNN, DE-NNNN, RI-NNNN ou RE-NNNN"))?;
        if numero.is_empty() || !numero.bytes().all(|b| b.is_ascii_digit()) {
            return Err(erro("o número tem só dígitos"));
        }
        let n: u32 = numero.parse().map_err(|_| erro("número grande demais"))?;
        if n == 0 {
            return Err(erro("a numeração começa em 1"));
        }
        let id = Self::novo(categoria, n);
        if id.to_string() != texto {
            return Err(erro("fora da forma canônica (dígitos a mais ou a menos)"));
        }
        Ok(id)
    }

    /// Nome do arquivo (`PR-0001.md`).
    pub fn arquivo(self) -> String {
        format!("{self}.{}", layout::EXTENSAO)
    }
}

impl fmt::Display for IdRegistro {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{:04}", self.categoria.prefixo(), self.numero)
    }
}

impl std::str::FromStr for IdRegistro {
    type Err = IdInvalido;

    fn from_str(texto: &str) -> Result<Self, Self::Err> {
        Self::analisar(texto)
    }
}

impl Serialize for IdRegistro {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for IdRegistro {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let texto = String::deserialize(d)?;
        Self::analisar(&texto).map_err(serde::de::Error::custom)
    }
}

// ---------------------------------------------------------------------------
// Campos

/// Status de uma premissa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StatusPremissa {
    #[default]
    NaoValidada,
    Validada,
    Invalidada,
}

/// Probabilidade ou impacto de um risco.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Nivel {
    Baixo,
    Medio,
    Alto,
}

/// Categoria de uma restrição.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TipoRestricao {
    Prazo,
    Legal,
    Tecnica,
}

/// Como uma premissa foi validada (ou invalidada).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Validacao {
    /// Como foi validada ("confirmado pelo PO na reunião R-0002", "verificado no
    /// código pela análise A-0001").
    pub como: String,
    pub em: Data,
    /// Pergunta que validou a premissa (`A-0001/2`, a pergunta 2 da análise), se
    /// houver.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pergunta: Option<String>,
}

/// Evento do histórico de um registro.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventoRegistro {
    pub em: Data,
    pub evento: String,
}

/// Frontmatter de um registro (premissa, decisão, risco ou restrição). Os campos
/// de uma categoria só são gravados nela.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrontmatterRegistro {
    pub tipo: TipoDocumento,
    pub id: IdRegistro,
    /// O enunciado: a premissa, o que foi decidido, o risco, a restrição.
    pub titulo: String,
    /// Slug do projeto.
    pub projeto: String,
    pub demanda: IdDemanda,
    pub criado: Data,
    /// Quem assumiu, decidiu, responde pelo risco ou informou a restrição.
    #[serde(default)]
    pub quem: Option<String>,
    /// Slug da pessoa no catálogo (`pessoas/<slug>.md`).
    #[serde(default)]
    pub pessoa: Option<String>,
    /// Documento de origem (a ata ou a análise), relativo à raiz do projeto.
    #[serde(default)]
    pub origem: Option<CaminhoRelativo>,
    /// Item de origem: `R-0001/D-1` (item da ata) ou `A-0001/3` (afirmação).
    #[serde(default)]
    pub referencia: Option<String>,
    /// Premissa: status da validação.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<StatusPremissa>,
    /// Premissa: como foi validada.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validacao: Option<Validacao>,
    /// Decisão: quando foi tomada.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decidido_em: Option<Data>,
    /// Decisão: alternativas consideradas.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alternativas: Vec<String>,
    /// Risco.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub probabilidade: Option<Nivel>,
    /// Risco.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub impacto: Option<Nivel>,
    /// Risco: resposta planejada (evitar, mitigar, aceitar…).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resposta: Option<String>,
    /// Restrição: prazo, legal ou técnica.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restricao: Option<TipoRestricao>,
    #[serde(default)]
    pub historico: Vec<EventoRegistro>,
}

impl FrontmatterRegistro {
    pub fn categoria(&self) -> Categoria {
        self.id.categoria
    }
}

fn texto_de_uma_linha(campo: &str, texto: &str, maximo: usize) -> Result<(), ErroCampo> {
    if texto.trim().is_empty() {
        return Err(ErroCampo::novo(campo, "não pode ficar vazio"));
    }
    if texto.chars().any(|c| c == '\n' || c == '\r') {
        return Err(ErroCampo::novo(campo, "precisa ter uma linha só"));
    }
    if texto.chars().count() > maximo {
        return Err(ErroCampo::novo(
            campo,
            format!("tem mais de {maximo} caracteres"),
        ));
    }
    Ok(())
}

impl Frontmatter for FrontmatterRegistro {
    const TIPOS: &'static [TipoDocumento] = &[
        TipoDocumento::Premissa,
        TipoDocumento::Decisao,
        TipoDocumento::Risco,
        TipoDocumento::Restricao,
    ];

    fn validar(&self) -> Result<(), ErroCampo> {
        let categoria = self.categoria();
        if categoria.tipo() != self.tipo {
            return Err(ErroCampo::novo(
                "id",
                format!("`{}` não é um identificador de {}", self.id, self.tipo),
            ));
        }
        if !e_slug(&self.projeto) {
            return Err(ErroCampo::novo(
                "projeto",
                format!("`{}` não é um slug (a-z, 0-9 e -)", self.projeto),
            ));
        }
        texto_de_uma_linha("titulo", &self.titulo, MAXIMO_DO_TITULO)?;
        if let Some(quem) = &self.quem {
            texto_de_uma_linha("quem", quem, 120)?;
        }
        if let Some(p) = &self.pessoa
            && !e_slug(p)
        {
            return Err(ErroCampo::novo(
                "pessoa",
                format!("`{p}` não é o slug de uma pessoa do catálogo"),
            ));
        }
        if let Some(r) = &self.referencia {
            texto_de_uma_linha("referencia", r, 64)?;
        }
        for (i, a) in self.alternativas.iter().enumerate() {
            texto_de_uma_linha(&format!("alternativas[{i}]"), a, MAXIMO_DO_TITULO)?;
        }
        let exigir = |presente: bool, campo: &str| {
            if presente {
                Ok(())
            } else {
                Err(ErroCampo::novo(
                    campo,
                    format!("obrigatório numa {}", categoria.nome()),
                ))
            }
        };
        let proibir = |presente: bool, campo: &str| {
            if presente {
                Err(ErroCampo::novo(
                    campo,
                    format!("não se aplica a {}", categoria.nome()),
                ))
            } else {
                Ok(())
            }
        };
        match categoria {
            Categoria::Premissa => exigir(self.status.is_some(), "status")?,
            Categoria::Risco => {
                exigir(self.probabilidade.is_some(), "probabilidade")?;
                exigir(self.impacto.is_some(), "impacto")?;
            }
            Categoria::Restricao => exigir(self.restricao.is_some(), "restricao")?,
            Categoria::Decisao => {}
        }
        if categoria != Categoria::Premissa {
            proibir(self.status.is_some(), "status")?;
            proibir(self.validacao.is_some(), "validacao")?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Dados para criar e editar

/// Campos que o analista edita num registro. Os de outra categoria são ignorados.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DadosRegistro {
    /// O enunciado.
    pub titulo: String,
    /// Detalhe, no corpo do documento.
    pub detalhe: Option<String>,
    pub quem: Option<String>,
    /// Slug da pessoa no catálogo. Sem `quem`, o nome vem do catálogo.
    pub pessoa: Option<String>,
    /// Premissa.
    pub status: Option<StatusPremissa>,
    /// Premissa: como foi validada (com status validada ou invalidada).
    pub como_validada: Option<String>,
    /// Decisão.
    pub decidido_em: Option<Data>,
    pub alternativas: Vec<String>,
    /// Risco.
    pub probabilidade: Option<Nivel>,
    pub impacto: Option<Nivel>,
    pub resposta: Option<String>,
    /// Restrição.
    pub restricao: Option<TipoRestricao>,
}

/// Registro a criar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NovoRegistro {
    pub categoria: Categoria,
    pub dados: DadosRegistro,
    /// Documento de origem (ata ou análise), relativo à raiz do projeto.
    pub origem: Option<CaminhoRelativo>,
    /// Item de origem (`R-0001/D-1`, `A-0001/3`).
    pub referencia: Option<String>,
    /// Primeiro evento do histórico ("criada à mão", "aceita a proposta…").
    pub evento: String,
}

/// Mudança no status de uma premissa: o resultado e como foi validada. É o que a
/// pauta de perguntas (#123) grava ao receber uma resposta.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidacaoDePremissa {
    pub status: StatusPremissa,
    /// Como foi validada. Obrigatório com `validada` ou `invalidada`.
    pub como: Option<String>,
    /// Pergunta que validou (`A-0001/2`), se houver.
    #[serde(default)]
    pub pergunta: Option<String>,
}

/// Registro lido da base.
#[derive(Debug, Clone, PartialEq)]
pub struct RegistroLido {
    pub id: IdRegistro,
    /// `demandas/D-NNNN-<slug>/registros/PR-0001.md`, relativo à raiz do projeto.
    pub arquivo: CaminhoRelativo,
    pub documento: Documento<FrontmatterRegistro>,
}

impl RegistroLido {
    pub fn frontmatter(&self) -> &FrontmatterRegistro {
        &self.documento.frontmatter
    }

    /// Detalhe do registro: o corpo sem o título.
    pub fn detalhe(&self) -> Option<String> {
        detalhe_do_corpo(&self.documento.corpo)
    }
}

fn detalhe_do_corpo(corpo: &str) -> Option<String> {
    let mut linhas = corpo.trim_start_matches('\n').lines().peekable();
    if linhas.peek().is_some_and(|l| l.starts_with('#')) {
        linhas.next();
    }
    let resto = linhas.collect::<Vec<_>>().join("\n");
    let resto = resto.trim();
    (!resto.is_empty()).then(|| resto.to_owned())
}

fn corpo(id: IdRegistro, titulo: &str, detalhe: Option<&str>) -> String {
    let mut texto = format!("\n# {id}: {titulo}\n");
    if let Some(d) = detalhe.map(str::trim).filter(|d| !d.is_empty()) {
        texto.push('\n');
        texto.push_str(d);
        texto.push('\n');
    }
    texto
}

fn limpo(texto: &Option<String>) -> Option<String> {
    texto
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
}

/// Identificador do registro pelo nome do arquivo (`PR-0001.md`).
fn id_do_arquivo(nome: &str) -> Option<IdRegistro> {
    let raiz = nome.strip_suffix(&format!(".{}", layout::EXTENSAO))?;
    IdRegistro::analisar(raiz).ok()
}

/// Remove a trava ao sair, em qualquer caminho.
struct Trava(std::path::PathBuf);

impl Drop for Trava {
    fn drop(&mut self) {
        let _ = fs::remove_dir(&self.0);
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Recusadas {
    #[serde(default)]
    referencias: Vec<String>,
}

impl Projeto {
    fn erro_no_registro(&self, arquivo: &CaminhoRelativo, erro: ErroCampo) -> Erro {
        Erro::Frontmatter {
            arquivo: self.na_base(arquivo),
            erro: crate::ErroFrontmatter {
                campo: Some(erro.campo),
                linha: None,
                mensagem: erro.mensagem,
            },
        }
    }

    /// Pasta dos registros de uma demanda (`demandas/D-NNNN-<slug>/registros`).
    pub fn pasta_dos_registros(&self, demanda: IdDemanda) -> Resultado<CaminhoRelativo> {
        Ok(self.demanda(demanda)?.pasta.juntar(PASTA)?)
    }

    /// Arquivos de registro (identificador e caminho) de uma pasta de demanda
    /// (relativa a `demandas/`), em ordem de identificador.
    fn arquivos_de_registro(&self, nome: &str) -> Resultado<Vec<(IdRegistro, CaminhoRelativo)>> {
        let pasta = pasta_das_demandas().juntar(&format!("{nome}/{PASTA}"))?;
        let entradas = match fs::read_dir(self.caminho(&pasta)) {
            Ok(e) => e,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(erro) => return Err(self.erro_io(&pasta, erro)),
        };
        let mut saida = Vec::new();
        for entrada in entradas.flatten() {
            if !entrada.file_type().is_ok_and(|t| t.is_file()) {
                continue;
            }
            let Ok(nome) = entrada.file_name().into_string() else {
                continue;
            };
            if let Some(id) = id_do_arquivo(&nome) {
                saida.push((id, pasta.juntar(&nome)?));
            }
        }
        saida.sort_by_key(|(id, _)| *id);
        Ok(saida)
    }

    /// Identificadores de registro usados em qualquer demanda do projeto.
    fn ids_de_registro(&self) -> Resultado<Vec<(IdRegistro, CaminhoRelativo)>> {
        let mut saida = Vec::new();
        for (_, nome) in self.pastas_de_demanda()? {
            saida.extend(self.arquivos_de_registro(&nome)?);
        }
        saida.sort_by_key(|(id, _)| *id);
        Ok(saida)
    }

    /// Próximo identificador livre da categoria no projeto, sem reservar.
    pub fn proximo_registro(&self, categoria: Categoria) -> Resultado<IdRegistro> {
        let maior = self
            .ids_de_registro()?
            .iter()
            .filter(|(id, _)| id.categoria == categoria)
            .map(|(id, _)| id.numero)
            .max()
            .unwrap_or(0);
        Ok(IdRegistro::novo(categoria, maior + 1))
    }

    /// Registros da demanda, em ordem de identificador (premissas, decisões,
    /// riscos e restrições). Um registro ilegível fica de fora, com um aviso no
    /// log (o índice o lista em `problemas`).
    pub fn registros_da_demanda(&self, demanda: IdDemanda) -> Resultado<Vec<RegistroLido>> {
        let pasta = self.demanda(demanda)?.pasta;
        let mut saida = Vec::new();
        for (id, arquivo) in self.arquivos_de_registro(pasta.nome())? {
            match self.ler::<FrontmatterRegistro>(&arquivo) {
                Ok(documento) if documento.frontmatter.id == id => saida.push(RegistroLido {
                    id,
                    arquivo,
                    documento,
                }),
                Ok(_) => tracing::warn!(registro = %id, "o id do registro difere do arquivo"),
                Err(erro) => tracing::warn!(registro = %id, %erro, "registro ilegível"),
            }
        }
        Ok(saida)
    }

    /// Lê um registro pelo identificador, em qualquer demanda do projeto.
    pub fn registro(&self, id: IdRegistro) -> Resultado<RegistroLido> {
        let (_, arquivo) = self
            .ids_de_registro()?
            .into_iter()
            .find(|(outro, _)| *outro == id)
            .ok_or_else(|| Erro::RegistroInexistente {
                projeto: self.nome().to_owned(),
                id: id.to_string(),
            })?;
        let documento: Documento<FrontmatterRegistro> = self.ler(&arquivo)?;
        Ok(RegistroLido {
            id,
            arquivo,
            documento,
        })
    }

    /// Preenche `quem` pelo catálogo quando só a pessoa foi informada, e confere
    /// que a pessoa existe.
    fn quem_e_pessoa(&self, dados: &DadosRegistro) -> Resultado<(Option<String>, Option<String>)> {
        let pessoa = limpo(&dados.pessoa);
        let mut quem = limpo(&dados.quem);
        if let Some(slug) = &pessoa {
            let lida = self.pessoa(slug)?;
            if quem.is_none() {
                quem = Some(lida.nome().to_owned());
            }
        }
        Ok((quem, pessoa))
    }

    /// Aplica os campos editáveis da categoria ao frontmatter.
    fn aplicar(
        &self,
        f: &mut FrontmatterRegistro,
        dados: &DadosRegistro,
        hoje: Data,
    ) -> Resultado<()> {
        let (quem, pessoa) = self.quem_e_pessoa(dados)?;
        f.titulo = dados.titulo.trim().to_owned();
        f.quem = quem;
        f.pessoa = pessoa;
        match f.categoria() {
            Categoria::Premissa => {
                let status = dados.status.unwrap_or_default();
                f.status = Some(status);
                f.validacao = match (status, limpo(&dados.como_validada)) {
                    (StatusPremissa::NaoValidada, _) => None,
                    (_, Some(como)) => Some(Validacao {
                        como,
                        em: hoje,
                        pergunta: f.validacao.as_ref().and_then(|v| v.pergunta.clone()),
                    }),
                    (_, None) => f.validacao.clone(),
                };
            }
            Categoria::Decisao => {
                f.decidido_em = dados.decidido_em;
                f.alternativas = dados
                    .alternativas
                    .iter()
                    .map(|a| a.trim().to_owned())
                    .filter(|a| !a.is_empty())
                    .collect();
            }
            Categoria::Risco => {
                f.probabilidade = Some(dados.probabilidade.unwrap_or(Nivel::Medio));
                f.impacto = Some(dados.impacto.unwrap_or(Nivel::Medio));
                f.resposta = limpo(&dados.resposta);
            }
            Categoria::Restricao => {
                f.restricao = dados.restricao;
            }
        }
        Ok(())
    }

    /// Cria um registro na demanda com o próximo número livre da categoria no
    /// projeto, em `registros/<ID>.md`.
    pub fn criar_registro(
        &self,
        demanda: IdDemanda,
        novo: &NovoRegistro,
        hoje: Data,
    ) -> Resultado<RegistroLido> {
        let pasta = self.pasta_dos_registros(demanda)?;
        let categoria = novo.categoria;
        let mut frontmatter = FrontmatterRegistro {
            tipo: categoria.tipo(),
            id: IdRegistro::novo(categoria, 1),
            titulo: String::new(),
            projeto: self.nome().to_owned(),
            demanda,
            criado: hoje,
            quem: None,
            pessoa: None,
            origem: novo.origem.clone(),
            referencia: limpo(&novo.referencia),
            status: None,
            validacao: None,
            decidido_em: None,
            alternativas: Vec::new(),
            probabilidade: None,
            impacto: None,
            resposta: None,
            restricao: None,
            historico: vec![EventoRegistro {
                em: hoje,
                evento: novo.evento.trim().to_owned(),
            }],
        };
        self.aplicar(&mut frontmatter, &novo.dados, hoje)?;
        // Valida antes de reservar, para não gastar um número.
        frontmatter
            .validar()
            .map_err(|e| self.erro_no_registro(&pasta, e))?;

        let demandas = pasta_das_demandas();
        let primeiro = self.proximo_registro(categoria)?.numero;
        for numero in primeiro..primeiro.saturating_add(TENTATIVAS_DE_RESERVA) {
            let id = IdRegistro::novo(categoria, numero);
            let trava = demandas.juntar(&format!(".{id}{SUFIXO_DA_TRAVA}"))?;
            match fs::create_dir(self.caminho(&trava)) {
                Ok(()) => {}
                Err(e) if arquivo::trava_ocupada(&e) => continue,
                Err(erro) => return Err(self.erro_io(&trava, erro)),
            }
            let _trava = Trava(self.caminho(&trava));
            if self
                .ids_de_registro()?
                .iter()
                .any(|(outro, _)| *outro == id)
            {
                continue;
            }
            frontmatter.id = id;
            let documento = Documento::novo(
                frontmatter.clone(),
                corpo(id, &frontmatter.titulo, novo.dados.detalhe.as_deref()),
            );
            let arquivo = pasta.juntar(&id.arquivo())?;
            match self.criar(&arquivo, &documento) {
                Ok(()) => {
                    return Ok(RegistroLido {
                        id,
                        arquivo,
                        documento,
                    });
                }
                Err(Erro::JaExiste(_)) => continue,
                Err(e) => return Err(e),
            }
        }
        Err(Erro::JaExiste(self.na_base(&pasta)))
    }

    /// Altera um registro e acrescenta `evento` ao histórico. Sem mudança, nada é
    /// gravado.
    fn alterar_registro(
        &self,
        id: IdRegistro,
        hoje: Data,
        evento: &str,
        mudar: impl FnOnce(&mut Documento<FrontmatterRegistro>) -> Resultado<()>,
    ) -> Resultado<RegistroLido> {
        let mut lido = self.registro(id)?;
        let antes = lido.documento.clone();
        mudar(&mut lido.documento)?;
        if lido.documento.frontmatter != antes.frontmatter || lido.documento.corpo != antes.corpo {
            lido.documento.frontmatter.historico.push(EventoRegistro {
                em: hoje,
                evento: evento.to_owned(),
            });
            self.gravar(&lido.arquivo, &lido.documento)?;
        }
        Ok(lido)
    }

    /// Edita os campos de um registro (enunciado, detalhe, pessoa e os campos da
    /// categoria).
    pub fn editar_registro(
        &self,
        id: IdRegistro,
        dados: &DadosRegistro,
        hoje: Data,
    ) -> Resultado<RegistroLido> {
        let pasta = self.registro(id)?.arquivo;
        self.alterar_registro(id, hoje, "editado", |doc| {
            self.aplicar(&mut doc.frontmatter, dados, hoje)?;
            doc.frontmatter
                .validar()
                .map_err(|e| self.erro_no_registro(&pasta, e))?;
            doc.corpo = corpo(id, &doc.frontmatter.titulo, dados.detalhe.as_deref());
            Ok(())
        })
    }

    /// Valida (ou invalida) uma premissa: grava o status, como foi validada e a
    /// pergunta que a validou, e acrescenta o evento ao histórico. Voltar para
    /// `nao-validada` apaga a validação. Recusa registro que não é premissa e
    /// validação sem o "como".
    ///
    /// É a operação que a pauta de perguntas (#123) usa ao receber a resposta.
    pub fn validar_premissa(
        &self,
        id: IdRegistro,
        validacao: &ValidacaoDePremissa,
        hoje: Data,
    ) -> Resultado<RegistroLido> {
        let arquivo = self.registro(id)?.arquivo;
        if id.categoria != Categoria::Premissa {
            return Err(self.erro_no_registro(
                &arquivo,
                ErroCampo::novo("id", format!("{id} não é uma premissa")),
            ));
        }
        let como = limpo(&validacao.como);
        let pergunta = limpo(&validacao.pergunta);
        let evento = match validacao.status {
            StatusPremissa::NaoValidada => "voltou a não validada",
            StatusPremissa::Validada => "validada",
            StatusPremissa::Invalidada => "invalidada",
        }
        .to_owned();
        let evento = match (&como, &pergunta) {
            (Some(c), Some(p)) => format!("{evento}: {c} (pergunta {p})"),
            (Some(c), None) => format!("{evento}: {c}"),
            _ => evento,
        };
        self.alterar_registro(id, hoje, &evento, |doc| {
            let f = &mut doc.frontmatter;
            f.status = Some(validacao.status);
            f.validacao = match validacao.status {
                StatusPremissa::NaoValidada => None,
                _ => {
                    let como = como.clone().ok_or_else(|| {
                        self.erro_no_registro(
                            &arquivo,
                            ErroCampo::novo(
                                "validacao.como",
                                "diga como a premissa foi validada ou invalidada",
                            ),
                        )
                    })?;
                    Some(Validacao {
                        como,
                        em: hoje,
                        pergunta: pergunta.clone(),
                    })
                }
            };
            f.validar().map_err(|e| self.erro_no_registro(&arquivo, e))
        })
    }

    fn arquivo_das_recusadas(&self, demanda: IdDemanda) -> Resultado<CaminhoRelativo> {
        Ok(self
            .pasta_dos_registros(demanda)?
            .juntar(ARQUIVO_RECUSADAS)?)
    }

    /// Referências das propostas recusadas na demanda (`A-0001/3`, `R-0001/D-1`).
    pub fn propostas_recusadas(&self, demanda: IdDemanda) -> Resultado<Vec<String>> {
        let arquivo = self.arquivo_das_recusadas(demanda)?;
        let bytes = match fs::read(self.caminho(&arquivo)) {
            Ok(b) => b,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(erro) => return Err(self.erro_io(&arquivo, erro)),
        };
        match serde_json::from_slice::<Recusadas>(&bytes) {
            Ok(r) => Ok(r.referencias),
            Err(erro) => {
                tracing::warn!(%erro, "propostas recusadas ilegíveis; ignoradas");
                Ok(Vec::new())
            }
        }
    }

    /// Guarda a recusa de uma proposta do agente, para ela não voltar a aparecer.
    pub fn recusar_proposta(&self, demanda: IdDemanda, referencia: &str) -> Resultado<()> {
        let referencia = referencia.trim();
        let arquivo = self.arquivo_das_recusadas(demanda)?;
        let mut referencias = self.propostas_recusadas(demanda)?;
        if referencia.is_empty() || referencias.iter().any(|r| r == referencia) {
            return Ok(());
        }
        referencias.push(referencia.to_owned());
        let mut texto = serde_json::to_string_pretty(&Recusadas { referencias })
            .expect("lista de textos sempre vira JSON");
        texto.push('\n');
        arquivo::gravar(&self.caminho(&arquivo), texto.as_bytes())
            .map_err(|e| self.erro_io(&arquivo, e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identificadores() {
        let id = IdRegistro::analisar("PR-0001").unwrap();
        assert_eq!(id.categoria, Categoria::Premissa);
        assert_eq!(id.numero, 1);
        assert_eq!(
            IdRegistro::novo(Categoria::Risco, 12).to_string(),
            "RI-0012"
        );
        assert_eq!(
            IdRegistro::novo(Categoria::Restricao, 3).arquivo(),
            "RE-0003.md"
        );
        for ruim in ["PR-1", "PR-00001", "XX-0001", "PR-0000", "PR0001", "DE-"] {
            assert!(IdRegistro::analisar(ruim).is_err(), "{ruim}");
        }
        assert_eq!(
            id_do_arquivo("DE-0002.md"),
            Some(IdRegistro::novo(Categoria::Decisao, 2))
        );
        assert_eq!(id_do_arquivo("recusadas.json"), None);
    }

    #[test]
    fn detalhe_sem_o_titulo() {
        assert_eq!(
            detalhe_do_corpo("\n# PR-0001: x\n\nmais texto\n").as_deref(),
            Some("mais texto")
        );
        assert_eq!(detalhe_do_corpo("\n# PR-0001: x\n"), None);
    }
}
