//! Mapa de funcionalidades do projeto (#208).
//!
//! O mapa é um cadastro do projeto, não uma análise: o agente parte dos pontos
//! de entrada do mapa estrutural ([`cba_index::Mapa::pontos_de_entrada`]) e os
//! agrupa em funcionalidades com nome de negócio; o analista revisa cada uma no
//! app (aceitar, descartar, editar, juntar, dividir). Rodar o mapeamento de novo
//! continua o mesmo mapa, então os identificadores ([`IdFuncionalidade`],
//! `F-<SIGLA>-NNN`) ficam estáveis e nunca são reaproveitados.
//!
//! - [`Funcionalidade`]: nome, área, descrição, atores, pontos de entrada (cada um
//!   com evidência), dados lidos e gravados, relacionadas, a decisão do analista e
//!   o documento negocial, quando a funcionalidade foi explicada. Funcionalidade
//!   sem ponto de entrada é recusada.
//! - [`PontoJustificado`]: ponto de entrada do mapa que fica sem funcionalidade,
//!   com o motivo (código morto, utilitário ou outro).
//! - [`MapaFuncionalidades`]: o estado, em `funcionalidades/mapa.estado.json`,
//!   alterado sob trava ([`MapaFuncionalidades::alterar`]) pelo app e pelo
//!   servidor MCP. Na primeira leitura sem estado, importa o catálogo e as
//!   análises de mapeamento da primeira versão do #208.
//!
//! A completude (pontos do mapa sem funcionalidade nem justificativa) depende do
//! mapa estrutural e é calculada pelas ferramentas (`cba_tools::mapear`).

use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io;

use cba_index::TipoEntrada;
use cba_kb::{CaminhoRelativo, Id, IdDemanda, IdFuncionalidade, layout};
use serde::{Deserialize, Serialize};

use crate::analise::Evidencia;

/// Versão do formato do estado do mapa.
pub const VERSAO_DO_MAPA: u32 = 1;

/// Ponto de entrada atribuído a uma funcionalidade.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PontoAtribuido {
    /// Chave do ponto no mapa (`<arquivo>#<nome qualificado>`), quando é um dos
    /// pontos que o app acha. Sem ela, o ponto está fora da lista (menu montado
    /// por tabela, job agendado fora do repositório) e só a evidência o sustenta.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chave: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tipo: Option<TipoEntrada>,
    /// Nome do ponto (`PKG_COBRANCA.PRC_CANCELAR_FATURA`, `FrmCancelamento`).
    pub nome: String,
    pub evidencia: Evidencia,
}

impl PontoAtribuido {
    /// O mesmo ponto: a mesma chave ou, sem chave, a mesma evidência.
    pub fn mesmo(&self, outro: &Self) -> bool {
        match (&self.chave, &outro.chave) {
            (Some(a), Some(b)) => a == b,
            _ => {
                self.evidencia.arquivo == outro.evidencia.arquivo
                    && self.evidencia.linhas == outro.evidencia.linhas
            }
        }
    }
}

/// Decisão do analista sobre uma funcionalidade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SituacaoFuncionalidade {
    /// Registrada pelo agente, sem decisão.
    #[default]
    Pendente,
    Aceita,
    Descartada,
    /// Juntada a outra ([`Funcionalidade::juntada_em`]).
    Juntada,
}

/// Funcionalidade de negócio do mapa.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Funcionalidade {
    pub id: IdFuncionalidade,
    /// Nome de negócio ("Parcelar fatura").
    pub nome: String,
    /// Slug da área (`cobranca`).
    pub area: String,
    /// Nome da área para quem lê ("Cobrança").
    pub area_nome: String,
    /// Uma ou duas frases.
    pub descricao: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub atores: Vec<String>,
    pub pontos: Vec<PontoAtribuido>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dados_lidos: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dados_gravados: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relacionadas: Vec<IdFuncionalidade>,
    #[serde(default)]
    pub situacao: SituacaoFuncionalidade,
    /// Funcionalidade em que esta foi juntada.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub juntada_em: Option<IdFuncionalidade>,
    /// Nome, área ou descrição editados pelo analista.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub editada: bool,
    /// Documento negocial (`funcionalidades/<area>/<tema>/negocial.md`), quando a
    /// funcionalidade foi explicada e a explicação aprovada.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub documento: Option<CaminhoRelativo>,
    /// Análise do fluxo `explicar` ligada à funcionalidade (a mais recente).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explicacao: Option<Id>,
    /// Demandas ligadas pelo analista no painel da funcionalidade (as das
    /// análises ligadas a ela entram sem estar aqui).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub demandas: Vec<IdDemanda>,
    #[serde(default)]
    pub sessao: Option<String>,
    /// Primeiro registro.
    pub registrada_ms: u64,
    /// Última mudança (registro de novo, decisão, edição).
    #[serde(default)]
    pub atualizada_ms: u64,
}

impl Funcionalidade {
    /// Entra no documento do mapa (não foi descartada nem juntada).
    pub fn aberta(&self) -> bool {
        matches!(
            self.situacao,
            SituacaoFuncionalidade::Pendente | SituacaoFuncionalidade::Aceita
        )
    }

    /// Chaves dos pontos do mapa atribuídos.
    pub fn chaves(&self) -> impl Iterator<Item = &str> {
        self.pontos.iter().filter_map(|p| p.chave.as_deref())
    }
}

/// Dados de uma funcionalidade nova (ou da nova versão de uma já registrada).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NovaFuncionalidade {
    pub nome: String,
    pub area: String,
    pub area_nome: String,
    pub descricao: String,
    pub atores: Vec<String>,
    pub pontos: Vec<PontoAtribuido>,
    pub dados_lidos: Vec<String>,
    pub dados_gravados: Vec<String>,
    pub relacionadas: Vec<IdFuncionalidade>,
    pub sessao: Option<String>,
}

/// Por que um ponto de entrada fica sem funcionalidade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MotivoSemFuncionalidade {
    /// Ninguém chama e não é chamado de fora.
    CodigoMorto,
    /// Ajudante usado por várias funcionalidades (leitura de parâmetro, log).
    Utilitario,
    Outro,
}

impl MotivoSemFuncionalidade {
    pub fn nome(self) -> &'static str {
        match self {
            Self::CodigoMorto => "código morto",
            Self::Utilitario => "utilitário",
            Self::Outro => "outro motivo",
        }
    }
}

/// Ponto de entrada do mapa que fica sem funcionalidade, com a justificativa.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PontoJustificado {
    pub chave: String,
    pub tipo: TipoEntrada,
    pub nome: String,
    pub arquivo: String,
    pub motivo: MotivoSemFuncionalidade,
    pub justificativa: String,
    #[serde(default)]
    pub sessao: Option<String>,
    pub registrado_ms: u64,
}

/// Por que uma issue do rastreador fica sem funcionalidade (#270).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MotivoIssue {
    ForaDoEscopo,
    Infraestrutura,
    Outro,
}

impl MotivoIssue {
    pub fn nome(self) -> &'static str {
        match self {
            Self::ForaDoEscopo => "fora do escopo",
            Self::Infraestrutura => "infraestrutura",
            Self::Outro => "outro motivo",
        }
    }
}

/// Issue do rastreador que fica sem funcionalidade, com a justificativa.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueJustificada {
    /// `github` ou `jira`.
    pub origem: String,
    /// `#41` ou `PROJ-9`.
    pub chave: String,
    #[serde(default)]
    pub url: String,
    pub titulo: String,
    pub motivo: MotivoIssue,
    pub justificativa: String,
    pub registrado_ms: u64,
}

/// Vínculo de uma issue que o agente propôs, com a evidência. É só proposta: a
/// confirmação é do analista, na tela "Vincular demandas" (#270).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PropostaDeIssue {
    /// `github` ou `jira`.
    pub origem: String,
    pub chave: String,
    pub funcionalidades: Vec<IdFuncionalidade>,
    /// Ponto de entrada ou trecho da issue que sustenta o vínculo.
    pub evidencia: String,
    pub registrado_ms: u64,
}

/// Recorte de um mapeamento: o projeto todo, uma pasta ou uma área.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EscopoMapa {
    /// Pasta relativa à raiz do projeto (`db`, `backend/src`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pasta: Option<String>,
    /// Slug da área.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub area: Option<String>,
}

impl EscopoMapa {
    /// Escopo do projeto inteiro.
    pub fn e_projeto(&self) -> bool {
        self.pasta.is_none() && self.area.is_none()
    }

    /// Descrição para quem lê.
    pub fn descricao(&self) -> String {
        match (&self.pasta, &self.area) {
            (None, None) => "o projeto inteiro".into(),
            (Some(p), None) => format!("a pasta `{p}`"),
            (None, Some(a)) => format!("a área `{a}`"),
            (Some(p), Some(a)) => format!("a pasta `{p}`, na área `{a}`"),
        }
    }

    /// O arquivo está na pasta do escopo (qualquer arquivo, sem pasta).
    pub fn contem_arquivo(&self, arquivo: &str) -> bool {
        match self.pasta.as_deref().map(|p| p.trim().trim_matches('/')) {
            None | Some("") => true,
            Some(p) => arquivo == p || arquivo.strip_prefix(p).is_some_and(|r| r.starts_with('/')),
        }
    }
}

/// Decisão do analista sobre uma funcionalidade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DecisaoFuncionalidade {
    Aceitar,
    Descartar,
    /// Volta a pendente.
    Desfazer,
}

/// Edição do analista no nome, na área e na descrição.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EdicaoFuncionalidade {
    pub nome: String,
    pub area: String,
    pub area_nome: String,
    pub descricao: String,
}

fn exigir(campo: &'static str, valor: &str) -> Result<String, ErroMapa> {
    let v = valor.trim();
    if v.is_empty() {
        Err(ErroMapa::CampoVazio(campo))
    } else {
        Ok(v.to_owned())
    }
}

/// Textos aparados, sem vazios e sem repetidos (sem diferenciar maiúsculas).
fn lista(textos: Vec<String>) -> Vec<String> {
    let mut saida: Vec<String> = Vec::new();
    for t in textos {
        let t = t.trim();
        if !t.is_empty() && !saida.iter().any(|x| x.eq_ignore_ascii_case(t)) {
            saida.push(t.to_owned());
        }
    }
    saida
}

fn somar(destino: &mut Vec<String>, origem: &[String]) {
    let juntos = lista(destino.iter().chain(origem).cloned().collect());
    *destino = juntos;
}

fn somar_pontos(destino: &mut Vec<PontoAtribuido>, origem: Vec<PontoAtribuido>) {
    for p in origem {
        if !destino.iter().any(|x| x.mesmo(&p)) {
            destino.push(p);
        }
    }
}

/// Operação recusada no mapa. As mensagens se dirigem a quem chamou (o agente ou
/// o analista).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ErroMapa {
    #[error("o campo `{0}` não pode ficar vazio")]
    CampoVazio(&'static str),
    #[error(
        "toda funcionalidade precisa de ao menos um ponto de entrada (tela, endpoint, \
         lote, rotina de banco, serviço ou integração) com evidência do código; cite em \
         `pontos_de_entrada` o ponto e o trecho que o define"
    )]
    SemPontoDeEntrada,
    #[error(
        "a funcionalidade {0} não existe no mapa; confira o identificador (F-<SIGLA>-NNN) \
         ou, para uma funcionalidade nova, não informe identificador"
    )]
    Desconhecida(IdFuncionalidade),
    #[error(
        "a funcionalidade {0} foi descartada ou juntada a outra e não muda mais; registre \
         uma funcionalidade nova, se preciso"
    )]
    Fechada(IdFuncionalidade),
    #[error(
        "o ponto de entrada `{chave}` já está na funcionalidade {funcionalidade}; um ponto \
         fica numa funcionalidade só (cite-a em `relacionadas`, se for o caso)"
    )]
    PontoJaAtribuido {
        chave: String,
        funcionalidade: IdFuncionalidade,
    },
    #[error("divisão recusada: {0}")]
    DivisaoInvalida(String),
    #[error("junção recusada: {0}")]
    JuncaoInvalida(String),
}

/// Falha ao ler ou gravar o estado do mapa.
#[derive(Debug, thiserror::Error)]
pub enum ErroArquivoMapa {
    #[error("não foi possível ler ou gravar `{arquivo}`: {erro}")]
    Io {
        arquivo: CaminhoRelativo,
        erro: io::Error,
    },
    #[error("`{arquivo}` não é um estado de mapa de funcionalidades válido: {erro}")]
    Formato {
        arquivo: CaminhoRelativo,
        erro: serde_json::Error,
    },
}

/// Falha de uma alteração: recusada pelo modelo ou erro de disco.
#[derive(Debug, thiserror::Error)]
pub enum ErroAlteracaoMapa {
    #[error(transparent)]
    Recusa(#[from] ErroMapa),
    #[error(transparent)]
    Arquivo(#[from] ErroArquivoMapa),
}

/// Estado do mapa de funcionalidades do projeto.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapaFuncionalidades {
    pub formato: u32,
    /// Todas as funcionalidades, inclusive as descartadas e as juntadas (que
    /// guardam o identificador).
    #[serde(default)]
    pub funcionalidades: Vec<Funcionalidade>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pontos_justificados: Vec<PontoJustificado>,
    /// Issues do rastreador sem funcionalidade, justificadas pelo analista (#270).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub issues_justificadas: Vec<IssueJustificada>,
    /// Vínculos de issues propostos pelo agente, à espera do analista (#270).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub propostas_de_issue: Vec<PropostaDeIssue>,
    /// Identificadores que saíram do catálogo da primeira versão (não são
    /// reaproveitados).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub retirados: Vec<IdFuncionalidade>,
    /// Sobe a cada alteração gravada.
    #[serde(default)]
    pub versao: u64,
    #[serde(default)]
    pub atualizado_ms: u64,
}

impl Default for MapaFuncionalidades {
    fn default() -> Self {
        Self {
            formato: VERSAO_DO_MAPA,
            funcionalidades: Vec::new(),
            pontos_justificados: Vec::new(),
            issues_justificadas: Vec::new(),
            propostas_de_issue: Vec::new(),
            retirados: Vec::new(),
            versao: 0,
            atualizado_ms: 0,
        }
    }
}

impl MapaFuncionalidades {
    pub fn funcionalidade(&self, id: &IdFuncionalidade) -> Option<&Funcionalidade> {
        self.funcionalidades.iter().find(|f| &f.id == id)
    }

    fn funcionalidade_mut(
        &mut self,
        id: &IdFuncionalidade,
    ) -> Result<&mut Funcionalidade, ErroMapa> {
        self.funcionalidades
            .iter_mut()
            .find(|f| &f.id == id)
            .ok_or_else(|| ErroMapa::Desconhecida(id.clone()))
    }

    fn aberta_mut(&mut self, id: &IdFuncionalidade) -> Result<&mut Funcionalidade, ErroMapa> {
        let f = self.funcionalidade_mut(id)?;
        if !f.aberta() {
            return Err(ErroMapa::Fechada(id.clone()));
        }
        Ok(f)
    }

    /// Maior número usado com a sigla (funcionalidades de qualquer situação e
    /// retirados); 0 sem nenhum.
    pub fn maior_numero(&self, sigla: &str) -> u32 {
        self.funcionalidades
            .iter()
            .map(|f| &f.id)
            .chain(&self.retirados)
            .filter(|id| id.sigla() == sigla)
            .map(IdFuncionalidade::numero)
            .max()
            .unwrap_or(0)
    }

    /// Próximo identificador livre da sigla.
    pub fn proximo_id(&self, sigla: &str) -> Result<IdFuncionalidade, ErroMapa> {
        IdFuncionalidade::novo(sigla, self.maior_numero(sigla) + 1)
            .map_err(|_| ErroMapa::CampoVazio("area"))
    }

    /// Funcionalidades que entram no documento (nem descartadas nem juntadas).
    pub fn abertas(&self) -> impl Iterator<Item = &Funcionalidade> {
        self.funcionalidades.iter().filter(|f| f.aberta())
    }

    /// Abertas em ordem de área e identificador.
    pub fn ordenadas(&self) -> Vec<&Funcionalidade> {
        let mut v: Vec<&Funcionalidade> = self.abertas().collect();
        v.sort_by(|a, b| (&a.area_nome, &a.id).cmp(&(&b.area_nome, &b.id)));
        v
    }

    /// Abertas ainda sem decisão do analista.
    pub fn pendentes(&self) -> usize {
        self.abertas()
            .filter(|f| f.situacao == SituacaoFuncionalidade::Pendente)
            .count()
    }

    /// Chaves dos pontos do mapa atribuídos a funcionalidades abertas.
    pub fn pontos_atribuidos(&self) -> BTreeSet<&str> {
        self.abertas().flat_map(Funcionalidade::chaves).collect()
    }

    /// Funcionalidade aberta que tem o ponto do mapa.
    pub fn dona_do_ponto(&self, chave: &str) -> Option<&Funcionalidade> {
        self.abertas().find(|f| f.chaves().any(|c| c == chave))
    }

    pub fn justificativa(&self, chave: &str) -> Option<&PontoJustificado> {
        self.pontos_justificados.iter().find(|j| j.chave == chave)
    }

    /// Registra a funcionalidade com o identificador `id` (reservado por quem
    /// chama, com [`Self::proximo_id`], ou o de uma funcionalidade do mapa). Se o
    /// identificador já existe, a funcionalidade é substituída pela nova versão
    /// (os pontos também) e volta a pendente; o documento negocial fica. Os pontos
    /// atribuídos deixam de estar justificados. Devolve se substituiu.
    pub fn registrar(
        &mut self,
        nova: NovaFuncionalidade,
        id: IdFuncionalidade,
        agora_ms: u64,
    ) -> Result<bool, ErroMapa> {
        let nome = exigir("nome", &nova.nome)?;
        let area = exigir("area", &nova.area)?;
        let area_nome = exigir("area", &nova.area_nome)?;
        let descricao = exigir("descricao", &nova.descricao)?;
        if nova.pontos.is_empty() {
            return Err(ErroMapa::SemPontoDeEntrada);
        }
        let mut pontos = Vec::new();
        somar_pontos(&mut pontos, nova.pontos);
        if let Some(f) = self.funcionalidade(&id)
            && !f.aberta()
        {
            return Err(ErroMapa::Fechada(id));
        }
        for p in pontos.iter().filter_map(|p| p.chave.as_deref()) {
            if let Some(dona) = self.dona_do_ponto(p)
                && dona.id != id
            {
                return Err(ErroMapa::PontoJaAtribuido {
                    chave: p.to_owned(),
                    funcionalidade: dona.id.clone(),
                });
            }
        }
        let mut relacionadas: Vec<IdFuncionalidade> = Vec::new();
        for r in nova.relacionadas {
            if r != id && !relacionadas.contains(&r) {
                relacionadas.push(r);
            }
        }
        let funcionalidade = Funcionalidade {
            id: id.clone(),
            nome,
            area,
            area_nome,
            descricao,
            atores: lista(nova.atores),
            pontos,
            dados_lidos: lista(nova.dados_lidos),
            dados_gravados: lista(nova.dados_gravados),
            relacionadas,
            situacao: SituacaoFuncionalidade::Pendente,
            juntada_em: None,
            editada: false,
            documento: None,
            explicacao: None,
            demandas: Vec::new(),
            sessao: nova.sessao,
            registrada_ms: agora_ms,
            atualizada_ms: agora_ms,
        };
        let atribuidas: BTreeSet<String> = funcionalidade.chaves().map(str::to_owned).collect();
        self.pontos_justificados
            .retain(|j| !atribuidas.contains(&j.chave));
        match self.funcionalidades.iter_mut().find(|f| f.id == id) {
            Some(f) => {
                let anterior = std::mem::replace(f, funcionalidade);
                f.documento = anterior.documento;
                f.explicacao = anterior.explicacao;
                f.demandas = anterior.demandas;
                f.registrada_ms = anterior.registrada_ms;
                Ok(true)
            }
            None => {
                self.funcionalidades.push(funcionalidade);
                Ok(false)
            }
        }
    }

    /// Acrescenta um ponto de entrada a uma funcionalidade aberta (a
    /// justificativa do ponto, se houver, sai). A funcionalidade volta a pendente.
    pub fn atribuir_ponto(
        &mut self,
        id: &IdFuncionalidade,
        ponto: PontoAtribuido,
        agora_ms: u64,
    ) -> Result<(), ErroMapa> {
        if let Some(chave) = ponto.chave.as_deref()
            && let Some(dona) = self.dona_do_ponto(chave)
            && &dona.id != id
        {
            return Err(ErroMapa::PontoJaAtribuido {
                chave: chave.to_owned(),
                funcionalidade: dona.id.clone(),
            });
        }
        self.aberta_mut(id)?;
        if let Some(chave) = ponto.chave.clone() {
            self.pontos_justificados.retain(|j| j.chave != chave);
        }
        let f = self.aberta_mut(id)?;
        somar_pontos(&mut f.pontos, vec![ponto]);
        f.situacao = SituacaoFuncionalidade::Pendente;
        f.atualizada_ms = agora_ms;
        Ok(())
    }

    /// Justifica um ponto do mapa sem funcionalidade (substitui a justificativa
    /// anterior do mesmo ponto). Ponto já atribuído é recusado.
    pub fn justificar_ponto(&mut self, ponto: PontoJustificado) -> Result<(), ErroMapa> {
        exigir("justificativa", &ponto.justificativa)?;
        if let Some(dona) = self.dona_do_ponto(&ponto.chave) {
            return Err(ErroMapa::PontoJaAtribuido {
                chave: ponto.chave.clone(),
                funcionalidade: dona.id.clone(),
            });
        }
        let ponto = PontoJustificado {
            justificativa: ponto.justificativa.trim().to_owned(),
            ..ponto
        };
        match self
            .pontos_justificados
            .iter_mut()
            .find(|j| j.chave == ponto.chave)
        {
            Some(j) => *j = ponto,
            None => self.pontos_justificados.push(ponto),
        }
        Ok(())
    }

    /// Registra (ou substitui) a proposta do agente para uma issue. As
    /// funcionalidades precisam existir e estar abertas.
    pub fn propor_issue(&mut self, proposta: PropostaDeIssue) -> Result<(), ErroMapa> {
        let evidencia = exigir("evidencia", &proposta.evidencia)?;
        if proposta.funcionalidades.is_empty() {
            return Err(ErroMapa::CampoVazio("funcionalidades"));
        }
        for id in &proposta.funcionalidades {
            match self.funcionalidade(id) {
                None => return Err(ErroMapa::Desconhecida(id.clone())),
                Some(f) if !f.aberta() => return Err(ErroMapa::Fechada(id.clone())),
                Some(_) => {}
            }
        }
        let proposta = PropostaDeIssue {
            evidencia,
            ..proposta
        };
        match self
            .propostas_de_issue
            .iter_mut()
            .find(|p| p.chave == proposta.chave && p.origem == proposta.origem)
        {
            Some(p) => *p = proposta,
            None => self.propostas_de_issue.push(proposta),
        }
        Ok(())
    }

    /// Tira a proposta de uma issue (confirmada ou recusada). Devolve se havia.
    pub fn limpar_proposta_da_issue(&mut self, origem: &str, chave: &str) -> bool {
        let antes = self.propostas_de_issue.len();
        self.propostas_de_issue
            .retain(|p| !(p.chave == chave && p.origem == origem));
        antes != self.propostas_de_issue.len()
    }

    /// Justifica uma issue sem funcionalidade (substitui a anterior da mesma
    /// `chave` e origem).
    pub fn justificar_issue(&mut self, issue: IssueJustificada) -> Result<(), ErroMapa> {
        exigir("justificativa", &issue.justificativa)?;
        let issue = IssueJustificada {
            justificativa: issue.justificativa.trim().to_owned(),
            ..issue
        };
        match self
            .issues_justificadas
            .iter_mut()
            .find(|j| j.chave == issue.chave && j.origem == issue.origem)
        {
            Some(j) => *j = issue,
            None => self.issues_justificadas.push(issue),
        }
        Ok(())
    }

    /// Tira a justificativa de uma issue. Devolve se havia.
    pub fn desfazer_justificativa_da_issue(&mut self, origem: &str, chave: &str) -> bool {
        let antes = self.issues_justificadas.len();
        self.issues_justificadas
            .retain(|j| !(j.chave == chave && j.origem == origem));
        antes != self.issues_justificadas.len()
    }

    /// Desfaz a justificativa de um ponto (o analista discorda): o ponto volta a
    /// ficar sem tratamento. Devolve se havia justificativa.
    pub fn desfazer_justificativa(&mut self, chave: &str) -> bool {
        let antes = self.pontos_justificados.len();
        self.pontos_justificados.retain(|j| j.chave != chave);
        antes != self.pontos_justificados.len()
    }

    /// Decisão do analista.
    pub fn decidir(
        &mut self,
        id: &IdFuncionalidade,
        decisao: DecisaoFuncionalidade,
        agora_ms: u64,
    ) -> Result<(), ErroMapa> {
        let f = self.funcionalidade_mut(id)?;
        if f.situacao == SituacaoFuncionalidade::Juntada {
            return Err(ErroMapa::Fechada(id.clone()));
        }
        f.situacao = match decisao {
            DecisaoFuncionalidade::Aceitar => SituacaoFuncionalidade::Aceita,
            DecisaoFuncionalidade::Descartar => SituacaoFuncionalidade::Descartada,
            DecisaoFuncionalidade::Desfazer => SituacaoFuncionalidade::Pendente,
        };
        f.atualizada_ms = agora_ms;
        if decisao == DecisaoFuncionalidade::Descartar {
            let id = id.clone();
            for g in &mut self.funcionalidades {
                g.relacionadas.retain(|r| r != &id);
            }
        }
        Ok(())
    }

    /// Edição do analista no nome, na área e na descrição. O identificador fica.
    pub fn editar(
        &mut self,
        id: &IdFuncionalidade,
        edicao: EdicaoFuncionalidade,
        agora_ms: u64,
    ) -> Result<(), ErroMapa> {
        let nome = exigir("nome", &edicao.nome)?;
        let area = exigir("area", &edicao.area)?;
        let area_nome = exigir("area", &edicao.area_nome)?;
        let descricao = exigir("descricao", &edicao.descricao)?;
        let f = self.aberta_mut(id)?;
        if f.nome == nome && f.area == area && f.area_nome == area_nome && f.descricao == descricao
        {
            return Ok(());
        }
        f.nome = nome;
        f.area = area;
        f.area_nome = area_nome;
        f.descricao = descricao;
        f.editada = true;
        f.atualizada_ms = agora_ms;
        Ok(())
    }

    /// Junta `origem` em `destino`: o destino recebe os pontos, os atores, os
    /// dados e as relacionadas da origem, que fica juntada (fora do documento).
    pub fn juntar(
        &mut self,
        origem: &IdFuncionalidade,
        destino: &IdFuncionalidade,
        agora_ms: u64,
    ) -> Result<(), ErroMapa> {
        if origem == destino {
            return Err(ErroMapa::JuncaoInvalida(
                "escolha duas funcionalidades diferentes".into(),
            ));
        }
        self.aberta_mut(destino)?;
        let o = self.aberta_mut(origem)?.clone();
        let d = self.aberta_mut(destino)?;
        somar_pontos(&mut d.pontos, o.pontos);
        somar(&mut d.atores, &o.atores);
        somar(&mut d.dados_lidos, &o.dados_lidos);
        somar(&mut d.dados_gravados, &o.dados_gravados);
        for r in o.relacionadas {
            if &r != destino && !d.relacionadas.contains(&r) {
                d.relacionadas.push(r);
            }
        }
        d.relacionadas.retain(|r| r != origem);
        d.situacao = SituacaoFuncionalidade::Pendente;
        d.atualizada_ms = agora_ms;
        if d.documento.is_none() {
            d.documento = o.documento;
            d.explicacao = o.explicacao;
        }
        for dem in o.demandas {
            if !d.demandas.contains(&dem) {
                d.demandas.push(dem);
            }
        }
        let o = self.funcionalidade_mut(origem)?;
        o.situacao = SituacaoFuncionalidade::Juntada;
        o.juntada_em = Some(destino.clone());
        o.atualizada_ms = agora_ms;
        // Quem citava a origem passa a citar o destino.
        for f in &mut self.funcionalidades {
            if f.relacionadas.contains(origem) {
                f.relacionadas.retain(|r| r != origem);
                if &f.id != destino && !f.relacionadas.contains(destino) {
                    f.relacionadas.push(destino.clone());
                }
            }
        }
        Ok(())
    }

    /// Divide uma funcionalidade: os pontos de índice `pontos` (a partir de 0)
    /// passam para uma funcionalidade nova, com o próximo identificador da sigla
    /// da original e o nome e a descrição dados. As duas ficam pendentes e citam
    /// uma à outra como relacionadas. Devolve o identificador novo.
    pub fn dividir(
        &mut self,
        id: &IdFuncionalidade,
        pontos: &[usize],
        nome: &str,
        descricao: &str,
        agora_ms: u64,
    ) -> Result<IdFuncionalidade, ErroMapa> {
        let nome = exigir("nome", nome)?;
        let descricao = exigir("descricao", descricao)?;
        let novo_id = self.proximo_id(id.sigla())?;
        let original = self.aberta_mut(id)?;
        let escolhidos: BTreeSet<usize> = pontos.iter().copied().collect();
        if escolhidos.is_empty() || escolhidos.iter().any(|i| *i >= original.pontos.len()) {
            return Err(ErroMapa::DivisaoInvalida(
                "escolha ao menos um ponto de entrada da funcionalidade para levar".into(),
            ));
        }
        if escolhidos.len() == original.pontos.len() {
            return Err(ErroMapa::DivisaoInvalida(
                "deixe ao menos um ponto de entrada na funcionalidade original".into(),
            ));
        }
        let mut ficam = Vec::new();
        let mut vao = Vec::new();
        for (i, p) in original.pontos.drain(..).enumerate() {
            if escolhidos.contains(&i) {
                vao.push(p);
            } else {
                ficam.push(p);
            }
        }
        original.pontos = ficam;
        original.situacao = SituacaoFuncionalidade::Pendente;
        original.atualizada_ms = agora_ms;
        if !original.relacionadas.contains(&novo_id) {
            original.relacionadas.push(novo_id.clone());
        }
        let nova = Funcionalidade {
            id: novo_id.clone(),
            nome,
            area: original.area.clone(),
            area_nome: original.area_nome.clone(),
            descricao,
            atores: original.atores.clone(),
            pontos: vao,
            dados_lidos: Vec::new(),
            dados_gravados: Vec::new(),
            relacionadas: vec![id.clone()],
            situacao: SituacaoFuncionalidade::Pendente,
            juntada_em: None,
            editada: true,
            documento: None,
            explicacao: None,
            demandas: Vec::new(),
            sessao: None,
            registrada_ms: agora_ms,
            atualizada_ms: agora_ms,
        };
        self.funcionalidades.push(nova);
        Ok(novo_id)
    }

    /// Liga (ou desliga, com `ligar` falso) uma demanda à funcionalidade.
    pub fn ligar_demanda(
        &mut self,
        id: &IdFuncionalidade,
        demanda: IdDemanda,
        ligar: bool,
    ) -> Result<(), ErroMapa> {
        let f = self.aberta_mut(id)?;
        f.demandas.retain(|d| *d != demanda);
        if ligar {
            f.demandas.push(demanda);
        }
        Ok(())
    }

    /// Liga o documento negocial aprovado de uma explicação à funcionalidade.
    /// Devolve se a funcionalidade existe e está aberta.
    pub fn ligar_documento(
        &mut self,
        id: &IdFuncionalidade,
        explicacao: &Id,
        documento: CaminhoRelativo,
    ) -> bool {
        match self.aberta_mut(id) {
            Ok(f) => {
                f.explicacao = Some(explicacao.clone());
                f.documento = Some(documento);
                true
            }
            Err(_) => false,
        }
    }

    // -----------------------------------------------------------------------
    // Disco

    /// Lê o estado do mapa do projeto. Sem estado, importa o da primeira versão
    /// (catálogo e análises de mapeamento não aprovadas) ou devolve o mapa vazio.
    pub fn carregar(kb: &cba_kb::Projeto) -> Result<Self, ErroArquivoMapa> {
        let relativo = layout::estado_do_mapa();
        match fs::read(kb.caminho(&relativo)) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|erro| ErroArquivoMapa::Formato {
                arquivo: kb.na_base(&relativo),
                erro,
            }),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(legado::importar(kb)),
            Err(erro) => Err(ErroArquivoMapa::Io {
                arquivo: kb.na_base(&relativo),
                erro,
            }),
        }
    }

    /// Altera o mapa sob a trava do projeto: lê, aplica `f` e, se ela não
    /// recusou, grava com a versão seguinte. Devolve o mapa gravado e o
    /// resultado de `f`.
    pub fn alterar<T>(
        kb: &cba_kb::Projeto,
        f: impl FnOnce(&mut Self, u64) -> Result<T, ErroMapa>,
    ) -> Result<(Self, T), ErroAlteracaoMapa> {
        let _trava = travar(kb)?;
        let mut mapa = Self::carregar(kb)?;
        let agora = crate::tempo::agora_ms();
        let resultado = f(&mut mapa, agora)?;
        mapa.versao += 1;
        mapa.atualizado_ms = agora;
        mapa.gravar(kb)?;
        Ok((mapa, resultado))
    }

    /// Grava o estado (gravação atômica). Use [`Self::alterar`], que trava.
    fn gravar(&self, kb: &cba_kb::Projeto) -> Result<(), ErroArquivoMapa> {
        let relativo = layout::estado_do_mapa();
        let destino = kb.caminho(&relativo);
        let io_err = |erro| ErroArquivoMapa::Io {
            arquivo: kb.na_base(&relativo),
            erro,
        };
        if let Some(pasta) = destino.parent() {
            fs::create_dir_all(pasta).map_err(io_err)?;
        }
        let mut json = serde_json::to_vec_pretty(self).expect("o mapa serializa");
        json.push(b'\n');
        cba_kb::arquivo::gravar(&destino, &json).map_err(io_err)
    }
}

fn travar(kb: &cba_kb::Projeto) -> Result<fs::File, ErroArquivoMapa> {
    let relativo = layout::trava_do_mapa();
    let caminho = kb.caminho(&relativo);
    let io_err = |erro| ErroArquivoMapa::Io {
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

/// Importação do estado da primeira versão do #208, quando o mapa era uma
/// análise do fluxo `mapear` aprovada para dentro de um catálogo.
mod legado {
    use super::*;

    #[derive(Deserialize)]
    struct ItemCatalogo {
        id: IdFuncionalidade,
        nome: String,
        area: String,
        area_nome: String,
        descricao: String,
        #[serde(default)]
        atores: Vec<String>,
        pontos: Vec<PontoAtribuido>,
        #[serde(default)]
        dados_lidos: Vec<String>,
        #[serde(default)]
        dados_gravados: Vec<String>,
        #[serde(default)]
        relacionadas: Vec<IdFuncionalidade>,
        #[serde(default)]
        documento: Option<CaminhoRelativo>,
        #[serde(default)]
        explicacao: Option<Id>,
        #[serde(default)]
        atualizada_ms: u64,
    }

    #[derive(Deserialize)]
    struct Catalogo {
        #[serde(default)]
        funcionalidades: Vec<ItemCatalogo>,
        #[serde(default)]
        retirados: Vec<IdFuncionalidade>,
    }

    #[derive(Deserialize)]
    struct AnaliseDeMapeamento {
        fluxo: String,
        #[serde(default)]
        estado: String,
        #[serde(default)]
        funcionalidades: Vec<Funcionalidade>,
        #[serde(default)]
        pontos_justificados: Vec<PontoJustificado>,
    }

    fn ler<T: for<'de> Deserialize<'de>>(caminho: &std::path::Path) -> Option<T> {
        let bytes = fs::read(caminho).ok()?;
        match serde_json::from_slice(&bytes) {
            Ok(v) => Some(v),
            Err(_) => {
                tracing::warn!("estado legado do mapa ilegível; ignorado");
                None
            }
        }
    }

    /// O mapa montado do catálogo (funcionalidades aceitas) e, por cima, das
    /// análises de mapeamento ainda não aprovadas, na ordem dos identificadores.
    pub(super) fn importar(kb: &cba_kb::Projeto) -> MapaFuncionalidades {
        let mut mapa = MapaFuncionalidades::default();
        if let Some(cat) = ler::<Catalogo>(&kb.caminho(&layout::estado_do_catalogo_legado())) {
            mapa.retirados = cat.retirados;
            for i in cat.funcionalidades {
                mapa.funcionalidades.push(Funcionalidade {
                    id: i.id,
                    nome: i.nome,
                    area: i.area,
                    area_nome: i.area_nome,
                    descricao: i.descricao,
                    atores: i.atores,
                    pontos: i.pontos,
                    dados_lidos: i.dados_lidos,
                    dados_gravados: i.dados_gravados,
                    relacionadas: i.relacionadas,
                    situacao: SituacaoFuncionalidade::Aceita,
                    juntada_em: None,
                    editada: false,
                    documento: i.documento,
                    explicacao: i.explicacao,
                    demandas: Vec::new(),
                    sessao: None,
                    registrada_ms: i.atualizada_ms,
                    atualizada_ms: i.atualizada_ms,
                });
            }
        }
        let pasta = kb.caminho(&CaminhoRelativo::novo(layout::ANALISES).expect("nome fixo"));
        let mut estados: Vec<(u32, std::path::PathBuf)> = fs::read_dir(&pasta)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let nome = e.file_name().to_str()?.to_owned();
                let id = Id::analisar(nome.strip_suffix(".estado.json")?).ok()?;
                Some((id.numero(), e.path()))
            })
            .collect();
        estados.sort();
        for (_, caminho) in estados {
            let Some(a) = ler::<AnaliseDeMapeamento>(&caminho) else {
                continue;
            };
            if a.fluxo != "mapear" || a.estado == "aprovada" {
                continue;
            }
            for f in a.funcionalidades {
                match mapa.funcionalidades.iter_mut().find(|x| x.id == f.id) {
                    Some(x) => {
                        let documento = x.documento.take();
                        let explicacao = x.explicacao.take();
                        *x = f;
                        x.documento = x.documento.take().or(documento);
                        x.explicacao = x.explicacao.take().or(explicacao);
                    }
                    None => mapa.funcionalidades.push(f),
                }
            }
            for j in a.pontos_justificados {
                if mapa.dona_do_ponto(&j.chave).is_none() {
                    mapa.pontos_justificados.retain(|x| x.chave != j.chave);
                    mapa.pontos_justificados.push(j);
                }
            }
        }
        for f in &mut mapa.funcionalidades {
            if f.atualizada_ms == 0 {
                f.atualizada_ms = f.registrada_ms;
            }
        }
        mapa
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analise::EstadoEvidencia;
    use cba_kb::frontmatter::Linhas;

    fn f(n: u32) -> IdFuncionalidade {
        IdFuncionalidade::novo("FAT", n).unwrap()
    }

    fn ponto(chave: &str, linha: u32) -> PontoAtribuido {
        PontoAtribuido {
            chave: Some(chave.into()),
            tipo: Some(TipoEntrada::RotinaBanco),
            nome: chave.rsplit('#').next().unwrap().into(),
            evidencia: Evidencia {
                arquivo: CaminhoRelativo::novo("db/x.sql").unwrap(),
                linhas: Linhas::novo(linha, linha).unwrap(),
                trecho: "X".into(),
                hash: "h".into(),
                commit: None,
                estado: EstadoEvidencia::Valida,
            },
        }
    }

    fn nova(nome: &str, pontos: Vec<PontoAtribuido>) -> NovaFuncionalidade {
        NovaFuncionalidade {
            nome: nome.into(),
            area: "cobranca".into(),
            area_nome: "Cobrança".into(),
            descricao: "Faz algo.".into(),
            atores: vec!["Atendente".into(), " atendente ".into()],
            pontos,
            dados_lidos: vec!["FATURA".into()],
            dados_gravados: vec![],
            relacionadas: vec![],
            sessao: None,
        }
    }

    fn just(chave: &str) -> PontoJustificado {
        PontoJustificado {
            chave: chave.into(),
            tipo: TipoEntrada::RotinaBanco,
            nome: "FN_PARAMETRO".into(),
            arquivo: "db/x.sql".into(),
            motivo: MotivoSemFuncionalidade::Utilitario,
            justificativa: "lê parâmetro".into(),
            sessao: None,
            registrado_ms: 1,
        }
    }

    #[test]
    fn registra_substitui_e_recusa_sem_ponto() {
        let mut m = MapaFuncionalidades::default();
        assert_eq!(
            m.registrar(nova("Cancelar", vec![]), f(1), 2),
            Err(ErroMapa::SemPontoDeEntrada)
        );
        assert_eq!(m.proximo_id("FAT").unwrap(), f(1));
        assert!(
            !m.registrar(nova("Cancelar", vec![ponto("db/x.sql#A", 1)]), f(1), 2)
                .unwrap()
        );
        assert_eq!(m.funcionalidades[0].atores, vec!["Atendente"]);
        m.decidir(&f(1), DecisaoFuncionalidade::Aceitar, 3).unwrap();
        m.ligar_documento(
            &f(1),
            &cba_kb::Serie::Analise.id(9),
            CaminhoRelativo::novo("funcionalidades/cobranca/cancelar/negocial.md").unwrap(),
        );
        // De novo, com o mesmo id: substitui, volta a pendente e mantém o documento.
        assert!(
            m.registrar(
                nova(
                    "Cancelar fatura",
                    vec![ponto("db/x.sql#A", 1), ponto("db/x.sql#B", 2)]
                ),
                f(1),
                4
            )
            .unwrap()
        );
        let um = m.funcionalidade(&f(1)).unwrap();
        assert_eq!(um.situacao, SituacaoFuncionalidade::Pendente);
        assert!(um.documento.is_some());
        assert_eq!(um.registrada_ms, 2);
        assert_eq!(m.pontos_atribuidos().len(), 2);
        assert_eq!(m.pendentes(), 1);
        assert!(matches!(
            m.registrar(nova("Outra", vec![ponto("db/x.sql#B", 2)]), f(2), 5),
            Err(ErroMapa::PontoJaAtribuido { .. })
        ));
        // Descartada, o número não volta.
        m.decidir(&f(1), DecisaoFuncionalidade::Descartar, 6)
            .unwrap();
        assert_eq!(m.proximo_id("FAT").unwrap(), f(2));
        assert!(matches!(
            m.registrar(nova("C", vec![ponto("db/x.sql#C", 3)]), f(1), 7),
            Err(ErroMapa::Fechada(_))
        ));
    }

    #[test]
    fn justificativa_sai_quando_o_ponto_e_atribuido() {
        let mut m = MapaFuncionalidades::default();
        m.justificar_ponto(just("db/x.sql#P")).unwrap();
        m.justificar_ponto(just("db/x.sql#P")).unwrap();
        assert_eq!(m.pontos_justificados.len(), 1);
        m.registrar(nova("Cobrar", vec![ponto("db/x.sql#A", 1)]), f(1), 2)
            .unwrap();
        m.atribuir_ponto(&f(1), ponto("db/x.sql#P", 5), 3).unwrap();
        assert!(m.pontos_justificados.is_empty());
        assert!(matches!(
            m.justificar_ponto(just("db/x.sql#P")),
            Err(ErroMapa::PontoJaAtribuido { .. })
        ));
        m.justificar_ponto(just("db/x.sql#Q")).unwrap();
        assert!(m.desfazer_justificativa("db/x.sql#Q"));
        assert!(!m.desfazer_justificativa("db/x.sql#Q"));
    }

    #[test]
    fn juntar_e_dividir() {
        let mut m = MapaFuncionalidades::default();
        m.registrar(nova("A", vec![ponto("db/x.sql#A", 1)]), f(1), 2)
            .unwrap();
        m.registrar(
            nova("B", vec![ponto("db/x.sql#B", 2), ponto("db/x.sql#C", 3)]),
            f(2),
            2,
        )
        .unwrap();
        let mut c = nova("C", vec![ponto("db/x.sql#D", 4)]);
        c.relacionadas = vec![f(2)];
        m.registrar(c, f(3), 2).unwrap();

        m.juntar(&f(2), &f(1), 3).unwrap();
        assert_eq!(m.funcionalidade(&f(1)).unwrap().pontos.len(), 3);
        assert_eq!(m.funcionalidade(&f(2)).unwrap().juntada_em, Some(f(1)));
        assert_eq!(m.funcionalidade(&f(3)).unwrap().relacionadas, vec![f(1)]);
        assert_eq!(m.abertas().count(), 2);
        assert!(matches!(
            m.juntar(&f(2), &f(1), 4),
            Err(ErroMapa::Fechada(_))
        ));

        assert!(matches!(
            m.dividir(&f(1), &[0, 1, 2], "N", "D", 5),
            Err(ErroMapa::DivisaoInvalida(_))
        ));
        let novo = m.dividir(&f(1), &[2], "Nova", "Desc.", 5).unwrap();
        assert_eq!(novo, f(4));
        assert_eq!(m.funcionalidade(&f(1)).unwrap().pontos.len(), 2);
        let n = m.funcionalidade(&f(4)).unwrap();
        assert_eq!(n.pontos[0].chave.as_deref(), Some("db/x.sql#C"));
        assert_eq!(n.relacionadas, vec![f(1)]);
        assert!(
            m.funcionalidade(&f(1))
                .unwrap()
                .relacionadas
                .contains(&f(4))
        );

        let d = IdDemanda::analisar("D-0004").unwrap();
        m.ligar_demanda(&f(1), d, true).unwrap();
        m.ligar_demanda(&f(1), d, true).unwrap();
        assert_eq!(m.funcionalidade(&f(1)).unwrap().demandas, vec![d]);
        m.ligar_demanda(&f(1), d, false).unwrap();
        assert!(m.funcionalidade(&f(1)).unwrap().demandas.is_empty());

        m.editar(
            &f(1),
            EdicaoFuncionalidade {
                nome: "Cobrar fatura".into(),
                area: "cobranca".into(),
                area_nome: "Cobrança".into(),
                descricao: "Cobra.".into(),
            },
            6,
        )
        .unwrap();
        assert!(m.funcionalidade(&f(1)).unwrap().editada);
    }

    #[test]
    fn escopo() {
        let e = EscopoMapa {
            pasta: Some("db/".into()),
            area: None,
        };
        assert!(e.contem_arquivo("db/x.sql"));
        assert!(!e.contem_arquivo("dbx/y.sql"));
        assert!(EscopoMapa::default().contem_arquivo("qualquer"));
        assert_eq!(EscopoMapa::default().descricao(), "o projeto inteiro");
    }

    fn projeto(tmp: &tempfile::TempDir) -> cba_kb::Projeto {
        let base = cba_kb::Base::criar(tmp.path().join("base")).unwrap();
        let doc = cba_kb::Documento::novo(
            cba_kb::frontmatter::FrontmatterProjeto {
                tipo: cba_kb::TipoDocumento::Projeto,
                id: "p".into(),
                nome: "Projeto P".into(),
                areas: Vec::new(),
                criado: None,
            },
            "# P\n",
        );
        base.criar_projeto(&doc).unwrap()
    }

    #[test]
    fn altera_sob_trava_e_nao_grava_a_recusa() {
        let tmp = tempfile::tempdir().unwrap();
        let kb = projeto(&tmp);
        assert_eq!(
            MapaFuncionalidades::carregar(&kb).unwrap(),
            MapaFuncionalidades::default()
        );
        let (m, id) = MapaFuncionalidades::alterar(&kb, |m, agora| {
            let id = m.proximo_id("FAT")?;
            m.registrar(
                nova("Cobrar", vec![ponto("db/x.sql#A", 1)]),
                id.clone(),
                agora,
            )?;
            Ok(id)
        })
        .unwrap();
        assert_eq!(id, f(1));
        assert_eq!(m.versao, 1);
        assert_eq!(MapaFuncionalidades::carregar(&kb).unwrap(), m);
        let recusa = MapaFuncionalidades::alterar(&kb, |m, agora| {
            m.registrar(nova("X", vec![]), f(2), agora)
        });
        assert!(matches!(
            recusa,
            Err(ErroAlteracaoMapa::Recusa(ErroMapa::SemPontoDeEntrada))
        ));
        assert_eq!(MapaFuncionalidades::carregar(&kb).unwrap().versao, 1);
    }

    #[test]
    fn importa_o_catalogo_e_as_analises_da_primeira_versao() {
        let tmp = tempfile::tempdir().unwrap();
        let kb = projeto(&tmp);
        let ev = serde_json::json!({
            "arquivo": "db/x.sql", "linhas": "1-1", "trecho": "X", "hash": "h"
        });
        let pt = |c: &str| serde_json::json!({"chave": c, "nome": c, "evidencia": ev});
        let catalogo = serde_json::json!({
            "formato": 1,
            "funcionalidades": [{
                "id": "F-FAT-001", "nome": "Cobrar", "area": "cobranca",
                "area_nome": "Cobrança", "descricao": "D.", "pontos": [pt("db/x.sql#A")],
                "situacao": "mantida", "origem": "A-0001", "atualizada_por": "A-0001",
                "atualizada_ms": 5,
                "documento": "funcionalidades/cobranca/cobrar/negocial.md"
            }],
            "retirados": ["F-FAT-007"]
        });
        let pasta = kb.caminho(&CaminhoRelativo::novo("funcionalidades").unwrap());
        fs::create_dir_all(&pasta).unwrap();
        fs::write(pasta.join("catalogo.estado.json"), catalogo.to_string()).unwrap();
        let analises = kb.caminho(&CaminhoRelativo::novo("analises").unwrap());
        fs::create_dir_all(&analises).unwrap();
        let analise = serde_json::json!({
            "fluxo": "mapear", "estado": "em-andamento",
            "funcionalidades": [{
                "id": "F-FAT-001", "nome": "Cobrar fatura", "area": "cobranca",
                "area_nome": "Cobrança", "descricao": "D2.", "pontos": [pt("db/x.sql#A")],
                "situacao": "pendente", "do_catalogo": true, "registrada_ms": 9
            }, {
                "id": "F-FAT-008", "nome": "Parcelar", "area": "cobranca",
                "area_nome": "Cobrança", "descricao": "P.", "pontos": [pt("db/x.sql#B")],
                "registrada_ms": 9
            }],
            "pontos_justificados": [{
                "chave": "db/x.sql#P", "tipo": "rotina_banco", "nome": "P",
                "arquivo": "db/x.sql", "motivo": "utilitario", "justificativa": "j",
                "registrado_ms": 9
            }]
        });
        fs::write(analises.join("A-0002.estado.json"), analise.to_string()).unwrap();
        let aprovada = serde_json::json!({"fluxo": "mapear", "estado": "aprovada",
        "funcionalidades": [{
            "id": "F-FAT-009", "nome": "Velha", "area": "cobranca",
            "area_nome": "Cobrança", "descricao": "V.", "pontos": [pt("db/x.sql#C")],
            "registrada_ms": 1
        }]});
        fs::write(analises.join("A-0001.estado.json"), aprovada.to_string()).unwrap();

        let m = MapaFuncionalidades::carregar(&kb).unwrap();
        assert_eq!(m.funcionalidades.len(), 2, "{m:#?}");
        let um = m.funcionalidade(&f(1)).unwrap();
        assert_eq!(um.nome, "Cobrar fatura");
        assert_eq!(um.situacao, SituacaoFuncionalidade::Pendente);
        assert!(um.documento.is_some());
        assert_eq!(m.pontos_justificados.len(), 1);
        assert_eq!(m.maior_numero("FAT"), 8);
        assert_eq!(m.retirados, vec![f(7)]);
    }
}
