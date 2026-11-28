//! Documentos vivos (#461): o entregável que pertence a uma funcionalidade do
//! mapa (ou a uma área, no caso do glossário e da visão) e evolui a cada demanda.
//!
//! Um caso de uso **não é recriado** quando uma regra ou um fluxo muda: ganha uma
//! versão nova, e o histórico de revisões registra a demanda que mudou cada
//! versão. O modelo segue o das regras (`VersaoDaRegra`, em
//! `cba_tools::mudancas`): cada [`Revisao`] guarda o texto da versão, então a
//! 1.3 continua acessível depois da publicação da 1.4.
//!
//! - [`DocumentoVivo`]: o tipo (do catálogo de entregáveis), o dono
//!   ([`Dono`]), o caminho na base, a proposta pendente e o histórico. A versão
//!   publicada é a da última revisão.
//! - [`Proposta`]: o rascunho da versão seguinte, sobre a publicada. Quem propõe
//!   manda o texto completo; o app compara com a versão publicada, bloco a bloco
//!   do Markdown, e marca cada trecho ([`Marca`]: novo, alterado com o texto
//!   anterior, removido). Editar ou gerar de novo tira a revisão final;
//!   publicar exige a revisão final ([`DocumentosVivos::publicar`]).
//! - [`EstadoDocumento`]: em dia, proposta pendente, desatualizado (os arquivos
//!   citados mudaram depois da versão publicada, conta sem IA feita por
//!   `cba_tools::documentos_vivos`) ou sem documento. Um documento desatualizado
//!   não se reescreve sozinho: a ação é "Propor atualização".
//! - [`DocumentosVivos`]: o estado, em `funcionalidades/documentos.estado.json`,
//!   alterado sob trava ([`DocumentosVivos::alterar`]).
//!
//! O arquivo publicado traz a tabela "Histórico de revisões"
//! ([`DocumentoVivo::markdown_publicado`]), e a exportação do arquivo a leva junto.

use std::fmt;
use std::fs::{self, OpenOptions};
use std::io;

use cba_kb::frontmatter::Data;
use cba_kb::{CaminhoRelativo, Id, IdDemanda, IdFuncionalidade, layout};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use similar::{Algorithm, DiffOp};

/// Versão do formato do estado dos documentos vivos.
pub const VERSAO_DOS_DOCUMENTOS: u32 = 1;

/// Título da seção que o app acrescenta ao documento publicado.
pub const TITULO_DO_HISTORICO: &str = "Histórico de revisões";

// ---------------------------------------------------------------------------
// Versão

/// Versão de um documento vivo (`1.3`). A primeira é a 1.0; cada publicação
/// sobe a menor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Versao {
    pub maior: u32,
    pub menor: u32,
}

impl Versao {
    pub const INICIAL: Self = Self { maior: 1, menor: 0 };

    pub const fn nova(maior: u32, menor: u32) -> Self {
        Self { maior, menor }
    }

    /// A versão que a próxima publicação grava.
    pub const fn seguinte(self) -> Self {
        Self {
            maior: self.maior,
            menor: self.menor + 1,
        }
    }

    /// Lê `1.3` (aceita `v1.3`).
    pub fn analisar(texto: &str) -> Option<Self> {
        let t = texto.trim();
        let t = t.strip_prefix(['v', 'V']).unwrap_or(t);
        let (maior, menor) = t.split_once('.')?;
        let numero = |s: &str| {
            (!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
                .then(|| s.parse::<u32>().ok())
                .flatten()
        };
        Some(Self::nova(numero(maior)?, numero(menor)?))
    }
}

impl fmt::Display for Versao {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.maior, self.menor)
    }
}

impl Serialize for Versao {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Versao {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let texto = String::deserialize(d)?;
        Self::analisar(&texto)
            .ok_or_else(|| serde::de::Error::custom(format!("versão inválida `{texto}`; use 1.3")))
    }
}

// ---------------------------------------------------------------------------
// Modelo

/// A quem o documento pertence.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Dono {
    /// Funcionalidade do mapa (caso de uso, regras, manual).
    Funcionalidade(IdFuncionalidade),
    /// Área funcional, pelo slug (glossário, visão).
    Area(String),
}

impl fmt::Display for Dono {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Funcionalidade(id) => write!(f, "a funcionalidade {id}"),
            Self::Area(a) => write!(f, "a área `{a}`"),
        }
    }
}

/// Arquivo do projeto citado nas evidências de uma versão, com o hash do
/// conteúdo quando a versão foi publicada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArquivoCitado {
    /// Caminho relativo à raiz do projeto.
    pub arquivo: String,
    /// SHA-256 do arquivo inteiro, normalizado como o hash das evidências.
    pub hash: String,
}

/// Uma versão publicada, no histórico de revisões.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Revisao {
    pub versao: Versao,
    /// Dia da publicação.
    pub data: Data,
    /// Demanda que mudou a versão.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub demanda: Option<IdDemanda>,
    /// Análise de origem.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analise: Option<Id>,
    /// Quem fez a revisão final.
    pub revisor: String,
    /// O que mudou, para a tabela do histórico.
    pub mudancas: String,
    /// O texto da versão, em Markdown, sem a tabela do histórico.
    pub texto: String,
    /// Arquivos citados nas evidências, com o hash da publicação.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub arquivos: Vec<ArquivoCitado>,
    pub publicada_ms: u64,
}

/// Como um trecho da proposta se compara com a versão publicada.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Marca {
    /// Igual ao publicado.
    Mantido,
    Novo,
    /// Mudou: o texto anterior fica em [`Trecho::anterior`].
    Alterado,
    /// Estava no publicado e saiu: o texto é o que saiu.
    Removido,
}

/// Trecho (bloco do Markdown) da proposta.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trecho {
    pub marca: Marca,
    pub texto: String,
    /// Texto publicado, quando o trecho foi alterado.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anterior: Option<String>,
}

/// Revisão final de uma proposta, obrigatória antes de publicar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevisaoFinal {
    pub por: String,
    pub em_ms: u64,
}

/// Rascunho da versão seguinte de um documento vivo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Proposta {
    /// A versão que a publicação grava.
    pub versao: Versao,
    /// Versão publicada sobre a qual a proposta foi feita (nenhuma, na 1.0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<Versao>,
    /// Texto completo proposto, em Markdown.
    pub texto: String,
    /// O texto em trechos, marcados contra a versão publicada.
    pub trechos: Vec<Trecho>,
    /// O que muda, para o histórico.
    pub mudancas: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub demanda: Option<IdDemanda>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analise: Option<Id>,
    /// Arquivos do projeto citados nas evidências, para conferir depois da
    /// publicação se o documento ficou desatualizado.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub arquivos: Vec<String>,
    /// Sem ela, a proposta é rascunho e não pode ser publicada.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revisada: Option<RevisaoFinal>,
    pub criada_ms: u64,
    #[serde(default)]
    pub atualizada_ms: u64,
}

impl Proposta {
    /// Trechos que mudam algo (sem os mantidos).
    pub fn mudancas_marcadas(&self) -> impl Iterator<Item = &Trecho> {
        self.trechos.iter().filter(|t| t.marca != Marca::Mantido)
    }
}

/// Documento vivo da base.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentoVivo {
    /// Id do tipo no catálogo de entregáveis (`caso-de-uso`).
    pub tipo: String,
    pub dono: Dono,
    /// Caminho na base, relativo à pasta do projeto.
    pub caminho: CaminhoRelativo,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proposta: Option<Proposta>,
    /// Versões publicadas, da mais antiga para a mais recente.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub historico: Vec<Revisao>,
    pub criado_ms: u64,
}

/// O que propor para a versão seguinte.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NovaProposta {
    pub texto: String,
    pub mudancas: String,
    pub demanda: Option<IdDemanda>,
    pub analise: Option<Id>,
    pub arquivos: Vec<String>,
}

/// Situação de um documento para o mapa e a demanda.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "estado", rename_all = "snake_case")]
pub enum EstadoDocumento {
    /// Publicado e sem mudança nos arquivos citados.
    EmDia {
        versao: Versao,
    },
    /// Há um rascunho da versão seguinte.
    PropostaPendente {
        versao: Versao,
        /// Já passou pela revisão final e pode ser publicada.
        revisada: bool,
    },
    /// Os arquivos citados mudaram depois da versão publicada.
    Desatualizado {
        versao: Versao,
        arquivos: Vec<String>,
    },
    SemDocumento,
}

impl EstadoDocumento {
    pub fn rotulo(&self) -> &'static str {
        match self {
            Self::EmDia { .. } => "em dia",
            Self::PropostaPendente { .. } => "proposta pendente",
            Self::Desatualizado { .. } => "desatualizado",
            Self::SemDocumento => "sem documento",
        }
    }

    /// A ação que o app oferece. O documento desatualizado não se reescreve
    /// sozinho: a ação é propor a atualização.
    pub fn acao(&self) -> Option<&'static str> {
        match self {
            Self::EmDia { .. } => None,
            Self::PropostaPendente { .. } => Some("Revisar proposta"),
            Self::Desatualizado { .. } => Some("Propor atualização"),
            Self::SemDocumento => Some("Gerar versão inicial"),
        }
    }
}

/// Operação recusada num documento vivo. As mensagens se dirigem a quem chamou.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ErroDocumento {
    #[error("o campo `{0}` não pode ficar vazio")]
    CampoVazio(&'static str),
    #[error("não há documento vivo em `{0}`")]
    Desconhecido(CaminhoRelativo),
    #[error("o documento `{0}` não tem proposta pendente")]
    SemProposta(CaminhoRelativo),
    #[error(
        "a proposta {versao} de `{caminho}` ainda é rascunho: a revisão final é obrigatória \
         antes de publicar (marque como revisada)"
    )]
    NaoRevisada {
        caminho: CaminhoRelativo,
        versao: Versao,
    },
    #[error(
        "a proposta é igual à versão {0} publicada; não há o que publicar (descarte a \
         proposta ou mude o texto)"
    )]
    SemMudanca(Versao),
    #[error(
        "{dono} já tem o documento `{tipo}` em `{existente}`; um documento vivo nunca é \
         recriado: proponha a versão seguinte dele"
    )]
    JaExiste {
        tipo: String,
        dono: Dono,
        existente: CaminhoRelativo,
    },
    #[error(
        "a proposta {versao} de `{caminho}` está pendente e veio {origem}; publique ou \
         descarte essa proposta antes de propor outra"
    )]
    PropostaDeOutraOrigem {
        caminho: CaminhoRelativo,
        versao: Versao,
        origem: String,
    },
    #[error("`{caminho}` já é o documento `{tipo}` de {dono}")]
    CaminhoEmUso {
        caminho: CaminhoRelativo,
        tipo: String,
        dono: Dono,
    },
}

/// Falha ao ler ou gravar o estado dos documentos vivos.
#[derive(Debug, thiserror::Error)]
pub enum ErroArquivoDocumentos {
    #[error("não foi possível ler ou gravar `{arquivo}`: {erro}")]
    Io {
        arquivo: CaminhoRelativo,
        erro: io::Error,
    },
    #[error("`{arquivo}` não é um estado de documentos vivos válido: {erro}")]
    Formato {
        arquivo: CaminhoRelativo,
        erro: serde_json::Error,
    },
}

/// Falha de uma alteração: recusada pelo modelo ou erro de disco.
#[derive(Debug, thiserror::Error)]
pub enum ErroAlteracaoDocumentos {
    #[error(transparent)]
    Recusa(#[from] ErroDocumento),
    #[error(transparent)]
    Arquivo(#[from] ErroArquivoDocumentos),
}

fn exigir(campo: &'static str, valor: &str) -> Result<String, ErroDocumento> {
    let v = valor.trim();
    if v.is_empty() {
        Err(ErroDocumento::CampoVazio(campo))
    } else {
        Ok(v.to_owned())
    }
}

impl DocumentoVivo {
    /// Documento novo, ainda sem versão nem proposta.
    pub fn novo(tipo: &str, dono: Dono, caminho: CaminhoRelativo, agora_ms: u64) -> Self {
        Self {
            tipo: tipo.to_owned(),
            dono,
            caminho,
            proposta: None,
            historico: Vec::new(),
            criado_ms: agora_ms,
        }
    }

    /// Documento que já estava na base antes do histórico: o texto atual vira a
    /// versão 1.0, com `mudancas` na tabela.
    pub fn importado(
        tipo: &str,
        dono: Dono,
        caminho: CaminhoRelativo,
        texto: &str,
        mudancas: &str,
        data: Data,
        agora_ms: u64,
    ) -> Self {
        let mut d = Self::novo(tipo, dono, caminho, agora_ms);
        d.historico.push(Revisao {
            versao: Versao::INICIAL,
            data,
            demanda: None,
            analise: None,
            revisor: String::new(),
            mudancas: mudancas.trim().to_owned(),
            texto: sem_historico(texto),
            arquivos: Vec::new(),
            publicada_ms: agora_ms,
        });
        d
    }

    /// A versão publicada mais recente.
    pub fn publicada(&self) -> Option<&Revisao> {
        self.historico.last()
    }

    pub fn versao_publicada(&self) -> Option<Versao> {
        self.publicada().map(|r| r.versao)
    }

    /// Uma versão do histórico (as anteriores continuam acessíveis).
    pub fn versao(&self, versao: Versao) -> Option<&Revisao> {
        self.historico.iter().find(|r| r.versao == versao)
    }

    /// Registra (ou refaz) a proposta da versão seguinte sobre a publicada. Uma
    /// proposta pendente da mesma origem (a mesma demanda, ou o mapa e as
    /// explicações, sem demanda) é substituída e volta a rascunho ("Gerar de
    /// novo"); a de outra origem é recusada, para uma demanda não apagar o
    /// trabalho de outra.
    pub fn propor(
        &mut self,
        nova: NovaProposta,
        agora_ms: u64,
    ) -> Result<&Proposta, ErroDocumento> {
        if let Some(p) = &self.proposta
            && p.demanda != nova.demanda
        {
            return Err(ErroDocumento::PropostaDeOutraOrigem {
                caminho: self.caminho.clone(),
                versao: p.versao,
                origem: p.demanda.as_ref().map_or_else(
                    || "do mapa ou de uma explicação".to_owned(),
                    |d| format!("da demanda {d}"),
                ),
            });
        }
        let texto = sem_historico(&exigir("texto", &nova.texto)?);
        let mudancas = exigir("mudancas", &nova.mudancas)?;
        let base = self.versao_publicada();
        if let Some(publicada) = self.publicada()
            && mesmos_blocos(&publicada.texto, &texto)
        {
            return Err(ErroDocumento::SemMudanca(publicada.versao));
        }
        let trechos = marcar(self.publicada().map_or("", |r| &r.texto), &texto);
        let mut arquivos: Vec<String> = Vec::new();
        for a in nova.arquivos {
            let a = a.trim().replace('\\', "/");
            if !a.is_empty() && !arquivos.contains(&a) {
                arquivos.push(a);
            }
        }
        let criada_ms = self.proposta.as_ref().map_or(agora_ms, |p| p.criada_ms);
        Ok(self.proposta.insert(Proposta {
            versao: base.map_or(Versao::INICIAL, Versao::seguinte),
            base,
            texto,
            trechos,
            mudancas,
            demanda: nova.demanda,
            analise: nova.analise,
            arquivos,
            revisada: None,
            criada_ms,
            atualizada_ms: agora_ms,
        }))
    }

    /// Edição do analista no texto (e, se vier, no resumo do que mudou): os
    /// trechos são marcados de novo e a proposta volta a rascunho.
    pub fn editar_proposta(
        &mut self,
        texto: &str,
        mudancas: Option<&str>,
        agora_ms: u64,
    ) -> Result<&Proposta, ErroDocumento> {
        let Some(p) = self.proposta.take() else {
            return Err(ErroDocumento::SemProposta(self.caminho.clone()));
        };
        let nova = NovaProposta {
            texto: texto.to_owned(),
            mudancas: mudancas.map_or(p.mudancas.clone(), str::to_owned),
            demanda: p.demanda,
            analise: p.analise.clone(),
            arquivos: p.arquivos.clone(),
        };
        self.proposta = Some(p);
        self.propor(nova, agora_ms)
    }

    /// Revisão final da proposta, por quem revisou.
    pub fn marcar_revisada(
        &mut self,
        por: &str,
        agora_ms: u64,
    ) -> Result<&Proposta, ErroDocumento> {
        let por = exigir("revisor", por)?;
        let caminho = self.caminho.clone();
        let p = self
            .proposta
            .as_mut()
            .ok_or(ErroDocumento::SemProposta(caminho))?;
        p.revisada = Some(RevisaoFinal {
            por,
            em_ms: agora_ms,
        });
        Ok(p)
    }

    /// Publica a proposta revisada: grava a revisão no histórico, com a versão
    /// seguinte, e tira a proposta. `arquivos` são os citados, com o hash de
    /// agora.
    pub fn publicar(
        &mut self,
        data: Data,
        arquivos: Vec<ArquivoCitado>,
        agora_ms: u64,
    ) -> Result<&Revisao, ErroDocumento> {
        let Some(p) = &self.proposta else {
            return Err(ErroDocumento::SemProposta(self.caminho.clone()));
        };
        let Some(revisada) = &p.revisada else {
            return Err(ErroDocumento::NaoRevisada {
                caminho: self.caminho.clone(),
                versao: p.versao,
            });
        };
        // A versão publicada pode ter mudado depois da proposta (outra
        // publicação): a revisão sobe a partir da última.
        let versao = self
            .versao_publicada()
            .map_or(Versao::INICIAL, Versao::seguinte);
        let revisao = Revisao {
            versao,
            data,
            demanda: p.demanda,
            analise: p.analise.clone(),
            revisor: revisada.por.clone(),
            mudancas: p.mudancas.clone(),
            texto: p.texto.clone(),
            arquivos,
            publicada_ms: agora_ms,
        };
        self.proposta = None;
        self.historico.push(revisao);
        Ok(self.historico.last().expect("acabou de entrar"))
    }

    /// O estado do documento. `mudou` diz se um arquivo citado na versão
    /// publicada mudou (ou sumiu) desde a publicação.
    pub fn estado(&self, mut mudou: impl FnMut(&ArquivoCitado) -> bool) -> EstadoDocumento {
        if let Some(p) = &self.proposta {
            return EstadoDocumento::PropostaPendente {
                versao: p.versao,
                revisada: p.revisada.is_some(),
            };
        }
        let Some(r) = self.publicada() else {
            return EstadoDocumento::SemDocumento;
        };
        let arquivos: Vec<String> = r
            .arquivos
            .iter()
            .filter(|a| mudou(a))
            .map(|a| a.arquivo.clone())
            .collect();
        if arquivos.is_empty() {
            EstadoDocumento::EmDia { versao: r.versao }
        } else {
            EstadoDocumento::Desatualizado {
                versao: r.versao,
                arquivos,
            }
        }
    }

    /// O corpo do arquivo publicado: o texto da versão e a tabela "Histórico de
    /// revisões". Sem versão publicada, `None`.
    pub fn markdown_publicado(&self) -> Option<String> {
        let r = self.publicada()?;
        Some(format!(
            "{}\n\n{}",
            r.texto.trim_end(),
            tabela_de_revisoes(&self.historico)
        ))
    }
}

/// A seção "Histórico de revisões": uma linha por versão, da mais antiga para a
/// mais recente.
pub fn tabela_de_revisoes(historico: &[Revisao]) -> String {
    let celula = |t: &str| {
        let t = t.split_whitespace().collect::<Vec<_>>().join(" ");
        if t.is_empty() {
            "—".to_owned()
        } else {
            t.replace('|', "\\|")
        }
    };
    let mut s = format!(
        "## {TITULO_DO_HISTORICO}\n\n| Versão | Data | Demanda | Análise | Revisado por | O que mudou |\n|---|---|---|---|---|---|\n"
    );
    for r in historico {
        s.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} |\n",
            r.versao,
            r.data,
            celula(&r.demanda.map(|d| d.to_string()).unwrap_or_default()),
            celula(
                &r.analise
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_default()
            ),
            celula(&r.revisor),
            celula(&r.mudancas),
        ));
    }
    s
}

/// O texto sem a seção "Histórico de revisões" (que o app monta), para quem
/// propõe a partir do arquivo publicado.
pub fn sem_historico(texto: &str) -> String {
    let mut saida = Vec::new();
    let mut fora = false;
    let mut cerca = false;
    for l in texto.lines() {
        if l.trim_start().starts_with("```") {
            cerca = !cerca;
        }
        if !cerca && let Some(titulo) = l.strip_prefix("## ") {
            fora = titulo.trim().eq_ignore_ascii_case(TITULO_DO_HISTORICO);
            if fora {
                continue;
            }
        } else if !cerca && fora && l.starts_with("# ") {
            fora = false;
        }
        if !fora {
            saida.push(l);
        }
    }
    saida.join("\n").trim().to_owned()
}

// ---------------------------------------------------------------------------
// Trechos

/// Blocos do Markdown: parágrafos separados por linha em branco, cada título e
/// cada item de lista à parte, e o bloco de código cercado inteiro.
pub fn blocos(texto: &str) -> Vec<String> {
    let mut blocos: Vec<String> = Vec::new();
    let mut atual: Vec<&str> = Vec::new();
    let mut cerca = false;
    let fechar = |atual: &mut Vec<&str>, blocos: &mut Vec<String>| {
        if !atual.is_empty() {
            blocos.push(atual.join("\n").trim_end().to_owned());
            atual.clear();
        }
    };
    for l in texto.lines() {
        let l = l.trim_end();
        if cerca {
            atual.push(l);
            if l.trim_start().starts_with("```") {
                cerca = false;
                fechar(&mut atual, &mut blocos);
            }
            continue;
        }
        if l.trim().is_empty() {
            fechar(&mut atual, &mut blocos);
            continue;
        }
        if l.trim_start().starts_with("```") {
            fechar(&mut atual, &mut blocos);
            atual.push(l);
            cerca = true;
            continue;
        }
        if l.starts_with('#') {
            fechar(&mut atual, &mut blocos);
            blocos.push(l.to_owned());
            continue;
        }
        if item_de_lista(l) {
            fechar(&mut atual, &mut blocos);
        }
        atual.push(l);
    }
    fechar(&mut atual, &mut blocos);
    blocos
}

/// Linha que abre um item de lista no primeiro nível (`- `, `* `, `+ `, `1. `).
fn item_de_lista(l: &str) -> bool {
    if l.starts_with("- ") || l.starts_with("* ") || l.starts_with("+ ") {
        return true;
    }
    let digitos = l.bytes().take_while(u8::is_ascii_digit).count();
    digitos > 0 && (l[digitos..].starts_with(". ") || l[digitos..].starts_with(") "))
}

/// O bloco para comparação: espaços repetidos e nas pontas não contam.
fn chave(bloco: &str) -> String {
    bloco.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn mesmos_blocos(a: &str, b: &str) -> bool {
    let chaves = |t: &str| blocos(t).iter().map(|b| chave(b)).collect::<Vec<_>>();
    chaves(a) == chaves(b)
}

/// Compara `novo` com `publicado`, bloco a bloco, e marca cada trecho. Os
/// removidos entram no lugar em que estavam.
pub fn marcar(publicado: &str, novo: &str) -> Vec<Trecho> {
    let antigos = blocos(publicado);
    let novos = blocos(novo);
    let ka: Vec<String> = antigos.iter().map(|b| chave(b)).collect();
    let kn: Vec<String> = novos.iter().map(|b| chave(b)).collect();
    let trecho = |marca, texto: &str, anterior: Option<&str>| Trecho {
        marca,
        texto: texto.to_owned(),
        anterior: anterior.map(str::to_owned),
    };
    let mut saida = Vec::new();
    for op in similar::capture_diff_slices(Algorithm::Patience, &ka, &kn) {
        match op {
            DiffOp::Equal { new_index, len, .. } => {
                for b in &novos[new_index..new_index + len] {
                    saida.push(trecho(Marca::Mantido, b, None));
                }
            }
            DiffOp::Insert {
                new_index, new_len, ..
            } => {
                for b in &novos[new_index..new_index + new_len] {
                    saida.push(trecho(Marca::Novo, b, None));
                }
            }
            DiffOp::Delete {
                old_index, old_len, ..
            } => {
                for b in &antigos[old_index..old_index + old_len] {
                    saida.push(trecho(Marca::Removido, b, None));
                }
            }
            DiffOp::Replace {
                old_index,
                old_len,
                new_index,
                new_len,
            } => {
                let pares = old_len.min(new_len);
                for i in 0..pares {
                    saida.push(trecho(
                        Marca::Alterado,
                        &novos[new_index + i],
                        Some(&antigos[old_index + i]),
                    ));
                }
                for b in &novos[new_index + pares..new_index + new_len] {
                    saida.push(trecho(Marca::Novo, b, None));
                }
                for b in &antigos[old_index + pares..old_index + old_len] {
                    saida.push(trecho(Marca::Removido, b, None));
                }
            }
        }
    }
    saida
}

// ---------------------------------------------------------------------------
// Estado em disco

/// Estado dos documentos vivos do projeto.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentosVivos {
    pub formato: u32,
    #[serde(default)]
    pub documentos: Vec<DocumentoVivo>,
    /// Sobe a cada alteração gravada.
    #[serde(default)]
    pub versao: u64,
    #[serde(default)]
    pub atualizado_ms: u64,
}

impl Default for DocumentosVivos {
    fn default() -> Self {
        Self {
            formato: VERSAO_DOS_DOCUMENTOS,
            documentos: Vec::new(),
            versao: 0,
            atualizado_ms: 0,
        }
    }
}

impl DocumentosVivos {
    /// O documento no caminho.
    pub fn documento(&self, caminho: &CaminhoRelativo) -> Option<&DocumentoVivo> {
        self.documentos.iter().find(|d| &d.caminho == caminho)
    }

    /// O documento do tipo do dono.
    pub fn do_dono(&self, tipo: &str, dono: &Dono) -> Option<&DocumentoVivo> {
        self.documentos
            .iter()
            .find(|d| d.tipo == tipo && &d.dono == dono)
    }

    /// Os documentos de uma funcionalidade.
    pub fn da_funcionalidade<'a>(
        &'a self,
        id: &'a IdFuncionalidade,
    ) -> impl Iterator<Item = &'a DocumentoVivo> {
        self.documentos
            .iter()
            .filter(move |d| matches!(&d.dono, Dono::Funcionalidade(f) if f == id))
    }

    /// Junção de funcionalidades no mapa: os documentos de `origem` passam a ser
    /// de `destino` nos tipos que o destino ainda não tem (como os vínculos do
    /// mapa). Devolve quantos mudaram de dono.
    pub fn transferir(&mut self, origem: &IdFuncionalidade, destino: &IdFuncionalidade) -> usize {
        let de = Dono::Funcionalidade(origem.clone());
        let para = Dono::Funcionalidade(destino.clone());
        let mut n = 0;
        for i in 0..self.documentos.len() {
            if self.documentos[i].dono != de {
                continue;
            }
            let tipo = self.documentos[i].tipo.clone();
            if self.do_dono(&tipo, &para).is_none() {
                self.documentos[i].dono = para.clone();
                n += 1;
            }
        }
        n
    }

    fn documento_mut(
        &mut self,
        caminho: &CaminhoRelativo,
    ) -> Result<&mut DocumentoVivo, ErroDocumento> {
        self.documentos
            .iter_mut()
            .find(|d| &d.caminho == caminho)
            .ok_or_else(|| ErroDocumento::Desconhecido(caminho.clone()))
    }

    /// O documento do tipo do dono em `caminho`, criado se ainda não existe. Um
    /// dono tem um documento por tipo, e um caminho é de um documento só.
    pub fn garantir(
        &mut self,
        tipo: &str,
        dono: &Dono,
        caminho: &CaminhoRelativo,
        agora_ms: u64,
    ) -> Result<&mut DocumentoVivo, ErroDocumento> {
        if let Some(d) = self.do_dono(tipo, dono)
            && &d.caminho != caminho
        {
            return Err(ErroDocumento::JaExiste {
                tipo: tipo.to_owned(),
                dono: dono.clone(),
                existente: d.caminho.clone(),
            });
        }
        if let Some(d) = self.documento(caminho)
            && (d.tipo != tipo || &d.dono != dono)
        {
            return Err(ErroDocumento::CaminhoEmUso {
                caminho: caminho.clone(),
                tipo: d.tipo.clone(),
                dono: d.dono.clone(),
            });
        }
        if self.documento(caminho).is_none() {
            self.documentos.push(DocumentoVivo::novo(
                tipo,
                dono.clone(),
                caminho.clone(),
                agora_ms,
            ));
        }
        self.documento_mut(caminho)
    }

    /// Acrescenta um documento que já estava na base (ver
    /// [`DocumentoVivo::importado`]). Se o mesmo documento (tipo, dono e caminho)
    /// já está no estado, não faz nada e devolve `false`.
    pub fn importar(&mut self, documento: DocumentoVivo) -> Result<bool, ErroDocumento> {
        let ja = self.documento(&documento.caminho).is_some()
            || self.do_dono(&documento.tipo, &documento.dono).is_some();
        if ja {
            self.garantir(&documento.tipo, &documento.dono, &documento.caminho, 0)?;
            return Ok(false);
        }
        self.documentos.push(documento);
        Ok(true)
    }

    pub fn propor(
        &mut self,
        caminho: &CaminhoRelativo,
        nova: NovaProposta,
        agora_ms: u64,
    ) -> Result<Proposta, ErroDocumento> {
        self.documento_mut(caminho)?.propor(nova, agora_ms).cloned()
    }

    pub fn editar_proposta(
        &mut self,
        caminho: &CaminhoRelativo,
        texto: &str,
        mudancas: Option<&str>,
        agora_ms: u64,
    ) -> Result<Proposta, ErroDocumento> {
        self.documento_mut(caminho)?
            .editar_proposta(texto, mudancas, agora_ms)
            .cloned()
    }

    pub fn marcar_revisada(
        &mut self,
        caminho: &CaminhoRelativo,
        por: &str,
        agora_ms: u64,
    ) -> Result<Proposta, ErroDocumento> {
        self.documento_mut(caminho)?
            .marcar_revisada(por, agora_ms)
            .cloned()
    }

    /// Publica a proposta revisada do documento.
    pub fn publicar(
        &mut self,
        caminho: &CaminhoRelativo,
        data: Data,
        arquivos: Vec<ArquivoCitado>,
        agora_ms: u64,
    ) -> Result<Revisao, ErroDocumento> {
        self.documento_mut(caminho)?
            .publicar(data, arquivos, agora_ms)
            .cloned()
    }

    /// Descarta a proposta. Um documento que nunca foi publicado sai do estado
    /// (volta a "sem documento").
    pub fn descartar_proposta(&mut self, caminho: &CaminhoRelativo) -> Result<(), ErroDocumento> {
        let d = self.documento_mut(caminho)?;
        if d.proposta.take().is_none() {
            return Err(ErroDocumento::SemProposta(caminho.clone()));
        }
        if d.historico.is_empty() {
            self.documentos.retain(|d| &d.caminho != caminho);
        }
        Ok(())
    }

    /// Lê o estado do projeto (vazio, se ainda não existe).
    pub fn carregar(kb: &cba_kb::Projeto) -> Result<Self, ErroArquivoDocumentos> {
        let relativo = layout::estado_dos_documentos_vivos();
        match fs::read(kb.caminho(&relativo)) {
            Ok(bytes) => {
                serde_json::from_slice(&bytes).map_err(|erro| ErroArquivoDocumentos::Formato {
                    arquivo: kb.na_base(&relativo),
                    erro,
                })
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(erro) => Err(ErroArquivoDocumentos::Io {
                arquivo: kb.na_base(&relativo),
                erro,
            }),
        }
    }

    /// Altera o estado sob a trava do projeto: lê, aplica `f` e, se ela não
    /// recusou, grava. Devolve o estado gravado e o resultado de `f`.
    pub fn alterar<T, E>(
        kb: &cba_kb::Projeto,
        f: impl FnOnce(&mut Self, u64) -> Result<T, E>,
    ) -> Result<(Self, T), E>
    where
        E: From<ErroArquivoDocumentos>,
    {
        let _trava = travar(kb)?;
        let mut estado = Self::carregar(kb)?;
        let agora = crate::tempo::agora_ms();
        let resultado = f(&mut estado, agora)?;
        estado.formato = VERSAO_DOS_DOCUMENTOS;
        estado.versao += 1;
        estado.atualizado_ms = agora;
        estado.gravar(kb)?;
        Ok((estado, resultado))
    }

    fn gravar(&self, kb: &cba_kb::Projeto) -> Result<(), ErroArquivoDocumentos> {
        let relativo = layout::estado_dos_documentos_vivos();
        let destino = kb.caminho(&relativo);
        let io_err = |erro| ErroArquivoDocumentos::Io {
            arquivo: kb.na_base(&relativo),
            erro,
        };
        if let Some(pasta) = destino.parent() {
            fs::create_dir_all(pasta).map_err(io_err)?;
        }
        let mut json = serde_json::to_vec_pretty(self).expect("o estado serializa");
        json.push(b'\n');
        cba_kb::arquivo::gravar(&destino, &json).map_err(io_err)
    }
}

fn travar(kb: &cba_kb::Projeto) -> Result<fs::File, ErroArquivoDocumentos> {
    let relativo = layout::trava_dos_documentos_vivos();
    let caminho = kb.caminho(&relativo);
    let io_err = |erro| ErroArquivoDocumentos::Io {
        arquivo: kb.na_base(&relativo),
        erro,
    };
    if let Some(pasta) = caminho.parent() {
        fs::create_dir_all(pasta).map_err(io_err)?;
    }
    let arquivo = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&caminho)
        .map_err(io_err)?;
    // Liberada quando o arquivo é fechado.
    arquivo.lock().map_err(io_err)?;
    Ok(arquivo)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> Data {
        Data::nova(2026, 11, 28).unwrap()
    }

    fn caminho() -> CaminhoRelativo {
        CaminhoRelativo::novo("funcionalidades/cobranca/cancelar-fatura/caso-de-uso.md").unwrap()
    }

    fn dono() -> Dono {
        Dono::Funcionalidade(IdFuncionalidade::novo("FAT", 1).unwrap())
    }

    fn demanda(n: u32) -> IdDemanda {
        IdDemanda::novo(n)
    }

    const V13: &str = "# Cancelar fatura\n\n## Fluxo principal\n\n1. O atendente abre a fatura.\n2. O sistema confere se está em aberto.\n3. O sistema cancela.\n\n## Regras\n\nSó cancela fatura em aberto.";

    /// Caso de uso publicado até a 1.3.
    fn caso_de_uso_v13() -> DocumentoVivo {
        let mut d = DocumentoVivo::novo(CASO, dono(), caminho(), 1);
        for (i, texto) in ["v1.0", "v1.1", "v1.2", V13].into_iter().enumerate() {
            d.propor(
                NovaProposta {
                    texto: texto.into(),
                    mudancas: format!("mudança {i}"),
                    demanda: Some(demanda(i as u32 + 1)),
                    ..Default::default()
                },
                10,
            )
            .unwrap();
            d.marcar_revisada("Ana", 11).unwrap();
            d.publicar(data(), vec![], 12).unwrap();
        }
        d
    }

    const CASO: &str = "caso-de-uso";

    #[test]
    fn transferir_passa_os_documentos_que_o_destino_nao_tem() {
        let outra = IdFuncionalidade::novo("FAT", 2).unwrap();
        let negocial =
            CaminhoRelativo::novo("funcionalidades/cobranca/cancelar-fatura/negocial.md").unwrap();
        let mut docs = DocumentosVivos::default();
        docs.documentos.push(caso_de_uso_v13());
        docs.documentos.push(DocumentoVivo::novo(
            "documento-negocial",
            dono(),
            negocial,
            1,
        ));
        let do_destino =
            CaminhoRelativo::novo("funcionalidades/cobranca/outra/negocial.md").unwrap();
        let destino = Dono::Funcionalidade(outra.clone());
        docs.documentos.push(DocumentoVivo::novo(
            "documento-negocial",
            destino.clone(),
            do_destino.clone(),
            1,
        ));
        let origem = IdFuncionalidade::novo("FAT", 1).unwrap();
        assert_eq!(docs.transferir(&origem, &outra), 1);
        let caso = docs.do_dono(CASO, &destino).unwrap();
        assert_eq!(caso.versao_publicada(), Some(Versao::nova(1, 3)));
        assert_eq!(
            docs.do_dono("documento-negocial", &destino)
                .unwrap()
                .caminho,
            do_destino
        );
        assert!(docs.do_dono("documento-negocial", &dono()).is_some());
    }

    #[test]
    fn proposta_de_outra_origem_nao_e_substituida() {
        let mut d = caso_de_uso_v13();
        let nova = |texto: &str, demanda: Option<IdDemanda>| NovaProposta {
            texto: format!("{V13}\n\n{texto}"),
            mudancas: "muda".into(),
            demanda,
            ..Default::default()
        };
        d.propor(nova("da D-0010", Some(demanda(10))), 20).unwrap();
        d.marcar_revisada("Ana", 21).unwrap();
        // Outra demanda, ou o mapa e as explicações, não apagam a proposta.
        for outra in [Some(demanda(11)), None] {
            let erro = d.propor(nova("de outra", outra), 22).unwrap_err();
            assert!(
                matches!(erro, ErroDocumento::PropostaDeOutraOrigem { .. }),
                "{erro}"
            );
            assert!(erro.to_string().contains("da demanda D-0010"), "{erro}");
        }
        let p = d.proposta.as_ref().unwrap();
        assert!(p.texto.ends_with("da D-0010") && p.revisada.is_some());
        // A mesma demanda gera de novo, e a proposta volta a rascunho.
        let p = d.propor(nova("de novo", Some(demanda(10))), 23).unwrap();
        assert!(p.texto.ends_with("de novo") && p.revisada.is_none());
    }

    #[test]
    fn versao_le_escreve_e_sobe() {
        let v = Versao::analisar("1.3").unwrap();
        assert_eq!(v.to_string(), "1.3");
        assert_eq!(v.seguinte(), Versao::nova(1, 4));
        assert_eq!(Versao::analisar("v2.10"), Some(Versao::nova(2, 10)));
        assert!(Versao::analisar("1").is_none());
        assert!(Versao::analisar("1.x").is_none());
        assert!(Versao::nova(1, 10) > Versao::nova(1, 9));
        let json = serde_json::to_string(&v).unwrap();
        assert_eq!(json, "\"1.3\"");
        assert_eq!(serde_json::from_str::<Versao>(&json).unwrap(), v);
    }

    #[test]
    fn blocos_separam_paragrafos_titulos_itens_e_codigo() {
        let b =
            blocos("# T\nTexto\nsegue\n\n- a\n- b\n  continua\n\n```\nx\n\ny\n```\n1. um\n2) dois");
        assert_eq!(
            b,
            vec![
                "# T",
                "Texto\nsegue",
                "- a",
                "- b\n  continua",
                "```\nx\n\ny\n```",
                "1. um",
                "2) dois"
            ]
        );
    }

    #[test]
    fn marca_novo_alterado_removido_e_mantido() {
        let publicado = "# T\n\nA\n\nB\n\nC";
        let novo = "# T\n\nA\n\nB mudou\n\nD";
        let t = marcar(publicado, novo);
        let marcas: Vec<(Marca, &str, Option<&str>)> = t
            .iter()
            .map(|t| (t.marca, t.texto.as_str(), t.anterior.as_deref()))
            .collect();
        assert_eq!(
            marcas,
            vec![
                (Marca::Mantido, "# T", None),
                (Marca::Mantido, "A", None),
                (Marca::Alterado, "B mudou", Some("B")),
                (Marca::Alterado, "D", Some("C")),
            ]
        );
        let t = marcar("A\n\nB\n\nC", "A\n\nC\n\nE");
        let marcas: Vec<_> = t.iter().map(|t| (t.marca, t.texto.as_str())).collect();
        assert_eq!(
            marcas,
            vec![
                (Marca::Mantido, "A"),
                (Marca::Removido, "B"),
                (Marca::Mantido, "C"),
                (Marca::Novo, "E"),
            ]
        );
        // Espaços não contam como mudança.
        assert!(
            marcar("A  b", "A b")
                .iter()
                .all(|t| t.marca == Marca::Mantido)
        );
    }

    #[test]
    fn proposta_da_1_4_marca_os_trechos_e_publicar_grava_o_historico() {
        let mut d = caso_de_uso_v13();
        assert_eq!(d.versao_publicada(), Some(Versao::nova(1, 3)));
        let novo = V13.replace(
            "2. O sistema confere se está em aberto.",
            "2. O sistema confere se está em aberto e sem pagamento parcial.",
        ) + "\n\n## Exceções\n\nFatura com pagamento parcial vai para estorno.";
        let p = d
            .propor(
                NovaProposta {
                    texto: novo.clone(),
                    mudancas: "Fatura com pagamento parcial não cancela".into(),
                    demanda: Some(demanda(42)),
                    analise: Some(cba_kb::Serie::Analise.id(7)),
                    arquivos: vec!["db\\cobranca.sql".into(), "db/cobranca.sql".into()],
                },
                20,
            )
            .unwrap()
            .clone();
        assert_eq!(p.versao, Versao::nova(1, 4));
        assert_eq!(p.base, Some(Versao::nova(1, 3)));
        assert_eq!(p.arquivos, vec!["db/cobranca.sql"]);
        let mudancas: Vec<_> = p.mudancas_marcadas().collect();
        assert_eq!(mudancas.len(), 3, "{mudancas:#?}");
        assert_eq!(mudancas[0].marca, Marca::Alterado);
        assert_eq!(
            mudancas[0].anterior.as_deref(),
            Some("2. O sistema confere se está em aberto.")
        );
        assert_eq!(mudancas[1].marca, Marca::Novo);
        assert_eq!(mudancas[1].texto, "## Exceções");
        assert_eq!(
            d.estado(|_| false),
            EstadoDocumento::PropostaPendente {
                versao: Versao::nova(1, 4),
                revisada: false
            }
        );

        // Publicar sem revisão final é recusado.
        assert_eq!(
            d.publicar(data(), vec![], 21),
            Err(ErroDocumento::NaoRevisada {
                caminho: caminho(),
                versao: Versao::nova(1, 4)
            })
        );
        d.marcar_revisada("Bruno", 22).unwrap();
        let r = d.publicar(data(), vec![], 23).unwrap().clone();
        assert_eq!(r.versao, Versao::nova(1, 4));
        assert_eq!(r.demanda, Some(demanda(42)));
        assert_eq!(r.analise, Some(cba_kb::Serie::Analise.id(7)));
        assert_eq!(r.revisor, "Bruno");
        assert_eq!(r.texto, novo);
        assert!(d.proposta.is_none());
        assert_eq!(d.historico.len(), 5);
        // A 1.3 continua acessível.
        assert_eq!(d.versao(Versao::nova(1, 3)).unwrap().texto, V13);
        assert_eq!(
            d.estado(|_| false),
            EstadoDocumento::EmDia { versao: r.versao }
        );

        let md = d.markdown_publicado().unwrap();
        assert!(md.starts_with(&novo));
        assert!(md.contains("## Histórico de revisões"));
        assert!(md.contains(
            "| 1.4 | 2026-11-28 | D-0042 | A-0007 | Bruno | Fatura com pagamento parcial não cancela |"
        ));
        assert!(md.contains("| 1.3 | 2026-11-28 | D-0004 | — | Ana | mudança 3 |"));
        // Propor de novo a partir do arquivo publicado: a tabela não entra no texto.
        assert_eq!(sem_historico(&md), novo);
    }

    #[test]
    fn edicao_e_gerar_de_novo_voltam_a_rascunho() {
        let mut d = caso_de_uso_v13();
        let nova = |texto: &str| NovaProposta {
            texto: texto.into(),
            mudancas: "m".into(),
            ..Default::default()
        };
        d.propor(nova(&format!("{V13}\n\nNovo.")), 30).unwrap();
        d.marcar_revisada("Ana", 31).unwrap();
        let p = d
            .editar_proposta(&format!("{V13}\n\nNovo, editado."), None, 32)
            .unwrap();
        assert!(p.revisada.is_none());
        assert_eq!(p.criada_ms, 30);
        assert_eq!(p.mudancas, "m");
        assert_eq!(p.trechos.last().unwrap().marca, Marca::Novo);
        d.marcar_revisada("Ana", 33).unwrap();
        let p = d.propor(nova(&format!("{V13}\n\nOutro.")), 34).unwrap();
        assert!(p.revisada.is_none());
        assert!(matches!(
            d.publicar(data(), vec![], 35),
            Err(ErroDocumento::NaoRevisada { .. })
        ));
        // Igual ao publicado: não há o que propor.
        assert_eq!(
            d.propor(nova(V13), 36).unwrap_err(),
            ErroDocumento::SemMudanca(Versao::nova(1, 3))
        );
        assert_eq!(
            d.propor(nova("  "), 36).unwrap_err(),
            ErroDocumento::CampoVazio("texto")
        );
    }

    #[test]
    fn estado_do_documento() {
        let mut d = DocumentoVivo::novo(CASO, dono(), caminho(), 1);
        assert_eq!(d.estado(|_| true), EstadoDocumento::SemDocumento);
        assert_eq!(
            EstadoDocumento::SemDocumento.acao(),
            Some("Gerar versão inicial")
        );
        d.propor(
            NovaProposta {
                texto: "Texto".into(),
                mudancas: "Versão inicial, gerada por engenharia reversa".into(),
                ..Default::default()
            },
            2,
        )
        .unwrap();
        assert_eq!(d.proposta.as_ref().unwrap().versao, Versao::INICIAL);
        assert!(
            d.proposta
                .as_ref()
                .unwrap()
                .trechos
                .iter()
                .all(|t| t.marca == Marca::Novo)
        );
        d.marcar_revisada("Ana", 3).unwrap();
        let citados = vec![
            ArquivoCitado {
                arquivo: "db/a.sql".into(),
                hash: "1".into(),
            },
            ArquivoCitado {
                arquivo: "db/b.sql".into(),
                hash: "2".into(),
            },
        ];
        d.publicar(data(), citados, 4).unwrap();
        assert_eq!(
            d.estado(|_| false),
            EstadoDocumento::EmDia {
                versao: Versao::INICIAL
            }
        );
        let est = d.estado(|a| a.arquivo == "db/b.sql");
        assert_eq!(
            est,
            EstadoDocumento::Desatualizado {
                versao: Versao::INICIAL,
                arquivos: vec!["db/b.sql".into()]
            }
        );
        assert_eq!(est.acao(), Some("Propor atualização"));
        assert_eq!(est.rotulo(), "desatualizado");
        let json = serde_json::to_value(&est).unwrap();
        assert_eq!(json["estado"], "desatualizado");
        assert_eq!(json["versao"], "1.0");
    }

    #[test]
    fn registro_um_documento_por_tipo_e_dono_e_descarte() {
        let mut docs = DocumentosVivos::default();
        let outro = CaminhoRelativo::novo("funcionalidades/cobranca/outro.md").unwrap();
        docs.garantir(CASO, &dono(), &caminho(), 1).unwrap();
        assert!(matches!(
            docs.garantir(CASO, &dono(), &outro, 1),
            Err(ErroDocumento::JaExiste { .. })
        ));
        assert!(matches!(
            docs.garantir("manual", &dono(), &caminho(), 1),
            Err(ErroDocumento::CaminhoEmUso { .. })
        ));
        docs.propor(
            &caminho(),
            NovaProposta {
                texto: "T".into(),
                mudancas: "m".into(),
                ..Default::default()
            },
            2,
        )
        .unwrap();
        // Descartar a proposta de um documento nunca publicado tira o documento.
        docs.descartar_proposta(&caminho()).unwrap();
        assert!(docs.documento(&caminho()).is_none());
        assert_eq!(
            docs.descartar_proposta(&caminho()),
            Err(ErroDocumento::Desconhecido(caminho()))
        );

        let importado = DocumentoVivo::importado(
            CASO,
            dono(),
            caminho(),
            "Antes\n\n## Histórico de revisões\n\n| x |",
            "Anterior",
            data(),
            3,
        );
        assert!(docs.importar(importado.clone()).unwrap());
        assert!(!docs.importar(importado).unwrap());
        let d = docs.documento(&caminho()).unwrap();
        assert_eq!(d.publicada().unwrap().texto, "Antes");
        assert_eq!(d.versao_publicada(), Some(Versao::INICIAL));
        let p = docs
            .propor(
                &caminho(),
                NovaProposta {
                    texto: "Depois".into(),
                    mudancas: "m".into(),
                    ..Default::default()
                },
                4,
            )
            .unwrap();
        assert_eq!(p.versao, Versao::nova(1, 1));
        docs.descartar_proposta(&caminho()).unwrap();
        assert_eq!(docs.documento(&caminho()).unwrap().historico.len(), 1);
        let id = IdFuncionalidade::novo("FAT", 1).unwrap();
        assert_eq!(docs.da_funcionalidade(&id).count(), 1);
    }

    #[test]
    fn grava_e_le_o_estado_sob_trava() {
        let tmp = tempfile::tempdir().unwrap();
        let base = cba_kb::Base::criar(tmp.path().join("base")).unwrap();
        let kb = base
            .criar_projeto(&cba_kb::Documento::novo(
                cba_kb::frontmatter::FrontmatterProjeto {
                    tipo: cba_kb::TipoDocumento::Projeto,
                    id: "p".into(),
                    nome: "Projeto P".into(),
                    areas: Vec::new(),
                    criado: None,
                },
                "# P\n",
            ))
            .unwrap();
        assert_eq!(
            DocumentosVivos::carregar(&kb).unwrap(),
            DocumentosVivos::default()
        );
        let recusa = DocumentosVivos::alterar(&kb, |d, _| {
            d.garantir(CASO, &dono(), &caminho(), 1)?;
            d.publicar(&caminho(), data(), vec![], 1)
                .map_err(ErroAlteracaoDocumentos::from)
        });
        assert!(matches!(
            recusa,
            Err(ErroAlteracaoDocumentos::Recusa(ErroDocumento::SemProposta(
                _
            )))
        ));
        assert!(!kb.caminho(&layout::estado_dos_documentos_vivos()).exists());
        let (gravado, _) = DocumentosVivos::alterar(&kb, |d, agora| {
            d.garantir(CASO, &dono(), &caminho(), agora)?;
            d.propor(
                &caminho(),
                NovaProposta {
                    texto: "T".into(),
                    mudancas: "m".into(),
                    ..Default::default()
                },
                agora,
            )
            .map_err(ErroAlteracaoDocumentos::from)
        })
        .unwrap();
        assert_eq!(gravado.versao, 1);
        assert_eq!(DocumentosVivos::carregar(&kb).unwrap(), gravado);
    }
}
