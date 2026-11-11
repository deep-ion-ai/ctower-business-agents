//! Modelo da análise: análise, sessão, afirmação, evidência, pergunta, sugestão e
//! termo proposto, cada um com o seu estado (spec, seções 4, 6.5 e 11).
//!
//! Este módulo é só domínio: tipos, invariantes e transições de estado, sem disco.
//! A persistência (um arquivo de estado por análise, na base) e a notificação ao app
//! estão em [`crate::analises`].
//!
//! # Estados
//!
//! ```text
//! Análise:   criada ──► em-andamento ◄──► concluida ──► aprovada
//!                 (registro ou sessão)  (concluir / registro novo)
//! Sessão:    ativa ◄──► aguardando-analista ──► concluida ──► encerrada
//!            (qualquer estado volta a ativa, menos encerrada)
//! Afirmação: registrada ──► confirmada | incorreta | aprofundar ; qualquer ──► a-revalidar
//! Evidência: valida ──► a-revalidar (o hash do trecho não confere mais)
//! Pergunta:  aberta ──► respondida | descartada
//! Sugestão:  proposta ──► aceita | recusada
//! Termo:     proposto ──► aceito | recusado
//! ```
//!
//! Itens de uma análise (afirmações, perguntas, sugestões, termos) têm número sequencial a
//! partir de 1, único dentro da análise e nunca reaproveitado.

use cba_kb::frontmatter::{Linhas, StatusAnalise, StatusSugestao, TipoDocumento};
use cba_kb::{CaminhoRelativo, Id};
use serde::{Deserialize, Serialize};

/// Versão do formato do arquivo de estado. Sobe quando um campo muda de sentido;
/// campos novos com valor padrão não exigem mudança.
pub const VERSAO_DO_FORMATO: u32 = 1;

/// Fluxo da análise (spec, seções 4.3 a 4.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Fluxo {
    /// Pergunta ao sistema.
    Perguntar,
    /// Explicação de funcionalidade (documento negocial).
    Explicar,
    /// Análise de impacto de uma demanda.
    Impacto,
}

impl Fluxo {
    /// Tipo do documento que o fluxo produz.
    pub fn tipo_documento(self) -> TipoDocumento {
        match self {
            Self::Perguntar => TipoDocumento::Pergunta,
            Self::Explicar => TipoDocumento::Funcionalidade,
            Self::Impacto => TipoDocumento::AnaliseImpacto,
        }
    }
}

/// Estado da análise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EstadoAnalise {
    /// Criada pelo app, ainda sem sessão nem registro.
    Criada,
    /// Um agente está trabalhando (houve sessão ou registro).
    EmAndamento,
    /// A rodada foi concluída (`concluir_analise`); aguarda a revisão do analista.
    Concluida,
    /// Aprovada pelo analista. Não aceita registros novos.
    Aprovada,
}

impl EstadoAnalise {
    /// `status` do frontmatter do documento.
    pub fn status_documento(self) -> StatusAnalise {
        match self {
            Self::Criada | Self::EmAndamento => StatusAnalise::Rascunho,
            Self::Concluida => StatusAnalise::EmRevisao,
            Self::Aprovada => StatusAnalise::Aprovada,
        }
    }
}

/// Cliente de IA de uma sessão.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Cliente {
    ClaudeCode,
    ClaudeDesktop,
    Desconhecido,
}

/// Estado de uma sessão do cliente de IA, vindo dos hooks (#25).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EstadoSessao {
    Ativa,
    /// O cliente espera uma ação do analista (`Notification`).
    AguardandoAnalista,
    /// O agente terminou a resposta (`Stop`).
    Concluida,
    /// A sessão foi fechada (`SessionEnd`). Estado final.
    Encerrada,
}

/// Sessão de um cliente de IA ligada à análise.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sessao {
    /// Identificador da sessão no cliente (o `session_id` dos hooks), para a retomada.
    pub id: String,
    pub cliente: Cliente,
    pub estado: EstadoSessao,
    pub inicio_ms: u64,
    #[serde(default)]
    pub fim_ms: Option<u64>,
}

/// Nível de confiança de uma afirmação (spec, seção 6.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confianca {
    /// Evidência direta.
    Confirmado,
    /// Dedução a partir das evidências listadas.
    Inferido,
    /// Sem evidência suficiente; vira pergunta.
    AConfirmar,
}

/// Estado de uma evidência.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EstadoEvidencia {
    /// Conferida no registro.
    #[default]
    Valida,
    /// O hash do trecho não confere mais com o arquivo (spec, seção 6.5).
    ARevalidar,
}

/// Trecho do código que sustenta uma afirmação: caminho relativo à raiz do sistema,
/// intervalo de linhas, trecho citado, hash do trecho e commit (spec, seção 6.5).
///
/// O modelo só guarda; quem confere o trecho e calcula o hash é a validação (#20).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evidencia {
    pub arquivo: CaminhoRelativo,
    pub linhas: Linhas,
    pub trecho: String,
    pub hash: String,
    #[serde(default)]
    pub commit: Option<String>,
    #[serde(default)]
    pub estado: EstadoEvidencia,
}

/// Estado de uma afirmação, decidido pelo analista no app.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "estado", rename_all = "kebab-case")]
pub enum EstadoAfirmacao {
    /// Registrada pelo agente, ainda sem revisão.
    Registrada,
    /// Confirmada pelo analista (vira regra quando a análise é aprovada).
    Confirmada,
    /// Marcada como incorreta, com o comentário do analista.
    Incorreta { comentario: String },
    /// O analista pediu aprofundamento.
    Aprofundar,
    /// Alguma evidência mudou e a afirmação precisa ser revista.
    ARevalidar,
}

/// Afirmação registrada pelo agente (`afirmar`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Afirmacao {
    pub numero: u32,
    /// Seção do documento onde a afirmação entra.
    pub secao: String,
    pub texto: String,
    pub confianca: Confianca,
    #[serde(default)]
    pub evidencias: Vec<Evidencia>,
    #[serde(flatten)]
    pub estado: EstadoAfirmacao,
    /// Sessão que registrou, quando conhecida.
    #[serde(default)]
    pub sessao: Option<String>,
    pub registrada_ms: u64,
}

/// Dados de uma afirmação nova.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NovaAfirmacao {
    pub secao: String,
    pub texto: String,
    pub confianca: Confianca,
    pub evidencias: Vec<Evidencia>,
    pub sessao: Option<String>,
}

/// Estado de uma pergunta para o solicitante.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "estado", rename_all = "kebab-case")]
pub enum EstadoPergunta {
    Aberta,
    Respondida { resposta: String },
    Descartada,
}

/// Pergunta em aberto (`perguntar`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pergunta {
    pub numero: u32,
    pub texto: String,
    /// Afirmação que originou a pergunta (confiança `a_confirmar`), se houver.
    #[serde(default)]
    pub afirmacao: Option<u32>,
    #[serde(flatten)]
    pub estado: EstadoPergunta,
    #[serde(default)]
    pub sessao: Option<String>,
    pub registrada_ms: u64,
}

/// Sugestão de alteração (`registrar_sugestao`, spec, seção 4.6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sugestao {
    pub numero: u32,
    /// Documento da sugestão na base (`S-NNNN`, em `sugestoes/`), quando criado.
    #[serde(default)]
    pub id: Option<Id>,
    #[serde(default)]
    pub titulo: String,
    pub arquivo: CaminhoRelativo,
    pub trecho_atual: String,
    pub trecho_proposto: String,
    pub motivo: String,
    pub impacto: String,
    /// Exemplo com números do antes e depois (spec, seção 4.6).
    #[serde(default)]
    pub exemplo: Option<String>,
    /// Onde está o trecho atual, conferido no registro (como uma evidência).
    #[serde(default)]
    pub evidencia: Option<Evidencia>,
    pub estado: StatusSugestao,
    #[serde(default)]
    pub sessao: Option<String>,
    pub registrada_ms: u64,
}

/// Dados de uma sugestão nova.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NovaSugestao {
    /// Documento da sugestão, se já reservado.
    pub id: Option<Id>,
    pub titulo: String,
    pub arquivo: CaminhoRelativo,
    pub trecho_atual: String,
    pub trecho_proposto: String,
    pub motivo: String,
    pub impacto: String,
    pub exemplo: Option<String>,
    pub evidencia: Option<Evidencia>,
    pub sessao: Option<String>,
}

/// Estado de um termo proposto para o glossário.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EstadoTermo {
    /// Proposto pelo agente; vira entrada do glossário se aceito na aprovação.
    Proposto,
    Aceito,
    Recusado,
}

/// Termo de negócio proposto para o glossário (`propor_termo`, spec, seções 6.1 e
/// 7.4). Fica na análise até a aprovação, quando o analista decide se entra no
/// glossário.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TermoProposto {
    pub numero: u32,
    pub termo: String,
    pub definicao: String,
    #[serde(default)]
    pub sinonimos: Vec<String>,
    /// Onde o termo vive no código: entidades, tabelas, telas, serviços, documentos.
    #[serde(default)]
    pub onde_vive: Vec<String>,
    #[serde(default)]
    pub area: Option<String>,
    #[serde(default)]
    pub evidencias: Vec<Evidencia>,
    pub estado: EstadoTermo,
    #[serde(default)]
    pub sessao: Option<String>,
    pub registrada_ms: u64,
}

/// Dados de um termo novo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NovoTermo {
    pub termo: String,
    pub definicao: String,
    pub sinonimos: Vec<String>,
    pub onde_vive: Vec<String>,
    pub area: Option<String>,
    pub evidencias: Vec<Evidencia>,
    pub sessao: Option<String>,
}

/// Estado completo de uma análise.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Analise {
    pub formato: u32,
    pub id: Id,
    /// Slug do sistema.
    pub sistema: String,
    pub fluxo: Fluxo,
    pub titulo: String,
    /// Entrada do analista: a pergunta, a funcionalidade ou o texto da demanda.
    pub entrada: String,
    pub estado: EstadoAnalise,
    /// Documento da análise, relativo à raiz do sistema na base.
    pub documento: CaminhoRelativo,
    /// Sobe a cada gravação. O app usa para saber se precisa reler.
    pub versao: u64,
    pub criada_ms: u64,
    pub atualizada_ms: u64,
    #[serde(default)]
    pub sessoes: Vec<Sessao>,
    #[serde(default)]
    pub afirmacoes: Vec<Afirmacao>,
    #[serde(default)]
    pub perguntas: Vec<Pergunta>,
    #[serde(default)]
    pub sugestoes: Vec<Sugestao>,
    #[serde(default)]
    pub termos: Vec<TermoProposto>,
}

/// Operação recusada pelo modelo. As mensagens se dirigem a quem chamou (o agente
/// ou o analista) e dizem como corrigir.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ErroModelo {
    #[error("o campo `{0}` não pode ficar vazio")]
    CampoVazio(&'static str),
    #[error(
        "uma afirmação com confiança `confirmado` precisa de ao menos uma evidência; \
         cite o trecho do código ou use `inferido` ou `a_confirmar`"
    )]
    ConfirmadoSemEvidencia,
    #[error("a análise {0} já foi aprovada e não aceita registros novos")]
    AnaliseAprovada(Id),
    #[error("a análise {id} está em `{estado:?}` e não pode passar para `{destino:?}`")]
    TransicaoDaAnalise {
        id: Id,
        estado: EstadoAnalise,
        destino: EstadoAnalise,
    },
    #[error("a sessão `{0}` não está ligada a esta análise")]
    SessaoDesconhecida(String),
    #[error("a sessão `{id}` está em `{estado:?}` e não pode passar para `{destino:?}`")]
    TransicaoDaSessao {
        id: String,
        estado: EstadoSessao,
        destino: EstadoSessao,
    },
    #[error("não existe {tipo} número {numero} nesta análise")]
    ItemDesconhecido { tipo: &'static str, numero: u32 },
    #[error("{tipo} número {numero} já foi decidida e não muda mais de estado")]
    ItemDecidido { tipo: &'static str, numero: u32 },
    #[error(
        "o termo `{termo}` já foi proposto nesta análise (número {numero}); \
         não registre de novo"
    )]
    TermoRepetido { termo: String, numero: u32 },
}

fn exigir(campo: &'static str, valor: &str) -> Result<(), ErroModelo> {
    if valor.trim().is_empty() {
        Err(ErroModelo::CampoVazio(campo))
    } else {
        Ok(())
    }
}

fn proximo<T>(itens: &[T], numero: impl Fn(&T) -> u32) -> u32 {
    itens.iter().map(numero).max().unwrap_or(0) + 1
}

impl Analise {
    /// Análise nova, em [`EstadoAnalise::Criada`], com o documento em
    /// `analises/<id>.md`.
    pub fn nova(
        id: Id,
        sistema: impl Into<String>,
        fluxo: Fluxo,
        titulo: impl Into<String>,
        entrada: impl Into<String>,
        agora_ms: u64,
    ) -> Self {
        Self {
            formato: VERSAO_DO_FORMATO,
            documento: id.arquivo(),
            id,
            sistema: sistema.into(),
            fluxo,
            titulo: titulo.into(),
            entrada: entrada.into(),
            estado: EstadoAnalise::Criada,
            versao: 1,
            criada_ms: agora_ms,
            atualizada_ms: agora_ms,
            sessoes: Vec::new(),
            afirmacoes: Vec::new(),
            perguntas: Vec::new(),
            sugestoes: Vec::new(),
            termos: Vec::new(),
        }
    }

    /// Registros movem a análise para em andamento; aprovada não aceita mais nada.
    fn aceitar_registro(&mut self) -> Result<(), ErroModelo> {
        match self.estado {
            EstadoAnalise::Aprovada => Err(ErroModelo::AnaliseAprovada(self.id.clone())),
            _ => {
                self.estado = EstadoAnalise::EmAndamento;
                Ok(())
            }
        }
    }

    fn transicao(&mut self, permitido: bool, destino: EstadoAnalise) -> Result<(), ErroModelo> {
        if !permitido {
            return Err(ErroModelo::TransicaoDaAnalise {
                id: self.id.clone(),
                estado: self.estado,
                destino,
            });
        }
        self.estado = destino;
        Ok(())
    }

    /// Fecha a rodada: em andamento → concluída.
    pub fn concluir(&mut self) -> Result<(), ErroModelo> {
        let ok = matches!(
            self.estado,
            EstadoAnalise::EmAndamento | EstadoAnalise::Concluida
        );
        self.transicao(ok, EstadoAnalise::Concluida)
    }

    /// Aprovação do analista: concluída → aprovada.
    pub fn aprovar(&mut self) -> Result<(), ErroModelo> {
        let ok = self.estado == EstadoAnalise::Concluida;
        self.transicao(ok, EstadoAnalise::Aprovada)
    }

    /// Liga uma sessão à análise, ou reativa uma já ligada (retomada). Uma sessão
    /// encerrada que volta (`claude --resume`) também é reativada.
    pub fn iniciar_sessao(
        &mut self,
        id: &str,
        cliente: Cliente,
        agora_ms: u64,
    ) -> Result<(), ErroModelo> {
        exigir("sessao", id)?;
        self.aceitar_registro()?;
        match self.sessoes.iter_mut().find(|s| s.id == id) {
            Some(s) => {
                s.estado = EstadoSessao::Ativa;
                s.fim_ms = None;
                if cliente != Cliente::Desconhecido {
                    s.cliente = cliente;
                }
            }
            None => self.sessoes.push(Sessao {
                id: id.to_owned(),
                cliente,
                estado: EstadoSessao::Ativa,
                inicio_ms: agora_ms,
                fim_ms: None,
            }),
        }
        Ok(())
    }

    /// Muda o estado de uma sessão ligada. Encerrada é final (só
    /// [`Analise::iniciar_sessao`] a reabre).
    pub fn atualizar_sessao(
        &mut self,
        id: &str,
        destino: EstadoSessao,
        agora_ms: u64,
    ) -> Result<(), ErroModelo> {
        let sessao = self
            .sessoes
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or_else(|| ErroModelo::SessaoDesconhecida(id.to_owned()))?;
        if sessao.estado == EstadoSessao::Encerrada && destino != EstadoSessao::Encerrada {
            return Err(ErroModelo::TransicaoDaSessao {
                id: id.to_owned(),
                estado: sessao.estado,
                destino,
            });
        }
        sessao.estado = destino;
        if destino == EstadoSessao::Encerrada {
            sessao.fim_ms.get_or_insert(agora_ms);
        }
        Ok(())
    }

    /// Registra uma afirmação e devolve o número dela.
    pub fn registrar_afirmacao(
        &mut self,
        nova: NovaAfirmacao,
        agora_ms: u64,
    ) -> Result<u32, ErroModelo> {
        exigir("secao", &nova.secao)?;
        exigir("texto", &nova.texto)?;
        if nova.confianca == Confianca::Confirmado && nova.evidencias.is_empty() {
            return Err(ErroModelo::ConfirmadoSemEvidencia);
        }
        self.aceitar_registro()?;
        let numero = proximo(&self.afirmacoes, |a| a.numero);
        self.afirmacoes.push(Afirmacao {
            numero,
            secao: nova.secao.trim().to_owned(),
            texto: nova.texto.trim().to_owned(),
            confianca: nova.confianca,
            evidencias: nova.evidencias,
            estado: EstadoAfirmacao::Registrada,
            sessao: nova.sessao,
            registrada_ms: agora_ms,
        });
        Ok(numero)
    }

    /// Decisão do analista sobre uma afirmação (ou marcação de revalidação).
    pub fn marcar_afirmacao(
        &mut self,
        numero: u32,
        estado: EstadoAfirmacao,
    ) -> Result<(), ErroModelo> {
        if let EstadoAfirmacao::Incorreta { comentario } = &estado {
            exigir("comentario", comentario)?;
        }
        let a = self
            .afirmacoes
            .iter_mut()
            .find(|a| a.numero == numero)
            .ok_or(ErroModelo::ItemDesconhecido {
                tipo: "afirmação",
                numero,
            })?;
        a.estado = estado;
        Ok(())
    }

    /// Registra uma pergunta para o solicitante e devolve o número dela.
    pub fn registrar_pergunta(
        &mut self,
        texto: &str,
        afirmacao: Option<u32>,
        sessao: Option<String>,
        agora_ms: u64,
    ) -> Result<u32, ErroModelo> {
        exigir("texto", texto)?;
        if let Some(n) = afirmacao
            && !self.afirmacoes.iter().any(|a| a.numero == n)
        {
            return Err(ErroModelo::ItemDesconhecido {
                tipo: "afirmação",
                numero: n,
            });
        }
        self.aceitar_registro()?;
        let numero = proximo(&self.perguntas, |p| p.numero);
        self.perguntas.push(Pergunta {
            numero,
            texto: texto.trim().to_owned(),
            afirmacao,
            estado: EstadoPergunta::Aberta,
            sessao,
            registrada_ms: agora_ms,
        });
        Ok(numero)
    }

    /// Responde ou descarta uma pergunta aberta.
    pub fn decidir_pergunta(
        &mut self,
        numero: u32,
        estado: EstadoPergunta,
    ) -> Result<(), ErroModelo> {
        if let EstadoPergunta::Respondida { resposta } = &estado {
            exigir("resposta", resposta)?;
        }
        let p = self
            .perguntas
            .iter_mut()
            .find(|p| p.numero == numero)
            .ok_or(ErroModelo::ItemDesconhecido {
                tipo: "pergunta",
                numero,
            })?;
        if p.estado != EstadoPergunta::Aberta {
            return Err(ErroModelo::ItemDecidido {
                tipo: "a pergunta",
                numero,
            });
        }
        p.estado = estado;
        Ok(())
    }

    /// Registra uma sugestão de alteração e devolve o número dela.
    pub fn registrar_sugestao(
        &mut self,
        nova: NovaSugestao,
        agora_ms: u64,
    ) -> Result<u32, ErroModelo> {
        exigir("titulo", &nova.titulo)?;
        exigir("trecho_proposto", &nova.trecho_proposto)?;
        exigir("motivo", &nova.motivo)?;
        exigir("impacto", &nova.impacto)?;
        self.aceitar_registro()?;
        let numero = proximo(&self.sugestoes, |s| s.numero);
        self.sugestoes.push(Sugestao {
            numero,
            id: nova.id,
            titulo: nova.titulo.trim().to_owned(),
            arquivo: nova.arquivo,
            trecho_atual: nova.trecho_atual,
            trecho_proposto: nova.trecho_proposto,
            motivo: nova.motivo.trim().to_owned(),
            impacto: nova.impacto.trim().to_owned(),
            exemplo: nova
                .exemplo
                .map(|e| e.trim().to_owned())
                .filter(|e| !e.is_empty()),
            evidencia: nova.evidencia,
            estado: StatusSugestao::Proposta,
            sessao: nova.sessao,
            registrada_ms: agora_ms,
        });
        Ok(numero)
    }

    /// Registra um termo proposto para o glossário e devolve o número dele. O
    /// mesmo termo (pelo nome normalizado) não é proposto duas vezes na análise.
    pub fn registrar_termo(&mut self, novo: NovoTermo, agora_ms: u64) -> Result<u32, ErroModelo> {
        exigir("termo", &novo.termo)?;
        exigir("definicao", &novo.definicao)?;
        let chave = cba_kb::slug(&novo.termo);
        if let Some(t) = self.termos.iter().find(|t| cba_kb::slug(&t.termo) == chave) {
            return Err(ErroModelo::TermoRepetido {
                termo: t.termo.clone(),
                numero: t.numero,
            });
        }
        self.aceitar_registro()?;
        let limpar = |lista: Vec<String>| -> Vec<String> {
            let mut saida: Vec<String> = Vec::new();
            for item in lista.iter().map(|s| s.trim()).filter(|s| !s.is_empty()) {
                if !saida.iter().any(|s| s == item) {
                    saida.push(item.to_owned());
                }
            }
            saida
        };
        let numero = proximo(&self.termos, |t| t.numero);
        self.termos.push(TermoProposto {
            numero,
            termo: novo.termo.trim().to_owned(),
            definicao: novo.definicao.trim().to_owned(),
            sinonimos: limpar(novo.sinonimos),
            onde_vive: limpar(novo.onde_vive),
            area: novo
                .area
                .map(|a| a.trim().to_owned())
                .filter(|a| !a.is_empty()),
            evidencias: novo.evidencias,
            estado: EstadoTermo::Proposto,
            sessao: novo.sessao,
            registrada_ms: agora_ms,
        });
        Ok(numero)
    }

    /// Aceita ou recusa um termo proposto.
    pub fn decidir_termo(&mut self, numero: u32, estado: EstadoTermo) -> Result<(), ErroModelo> {
        let t = self.termos.iter_mut().find(|t| t.numero == numero).ok_or(
            ErroModelo::ItemDesconhecido {
                tipo: "termo",
                numero,
            },
        )?;
        if t.estado != EstadoTermo::Proposto {
            return Err(ErroModelo::ItemDecidido {
                tipo: "o termo",
                numero,
            });
        }
        t.estado = estado;
        Ok(())
    }

    /// Aceita ou recusa uma sugestão proposta.
    pub fn decidir_sugestao(
        &mut self,
        numero: u32,
        estado: StatusSugestao,
    ) -> Result<(), ErroModelo> {
        let s = self
            .sugestoes
            .iter_mut()
            .find(|s| s.numero == numero)
            .ok_or(ErroModelo::ItemDesconhecido {
                tipo: "sugestão",
                numero,
            })?;
        if s.estado != StatusSugestao::Proposta {
            return Err(ErroModelo::ItemDecidido {
                tipo: "a sugestão",
                numero,
            });
        }
        s.estado = estado;
        Ok(())
    }

    /// Marca como a revalidar as evidências que não conferem mais com o código, e
    /// as afirmações que dependem delas. `confere` diz se a evidência ainda vale; o
    /// hash é do intervalo citado, não do arquivo inteiro (veja
    /// `cba_tools::evidencias::hash_das_linhas`). Devolve quantas afirmações mudaram.
    pub fn marcar_a_revalidar(&mut self, confere: impl Fn(&Evidencia) -> bool) -> usize {
        let marcar = |e: &mut Evidencia| {
            let mudou = e.estado == EstadoEvidencia::Valida && !confere(e);
            if mudou {
                e.estado = EstadoEvidencia::ARevalidar;
            }
            mudou
        };
        let mut mudaram = 0;
        for a in &mut self.afirmacoes {
            let mut mudou = false;
            for e in &mut a.evidencias {
                mudou |= marcar(e);
            }
            if mudou && a.estado != EstadoAfirmacao::ARevalidar {
                a.estado = EstadoAfirmacao::ARevalidar;
                mudaram += 1;
            }
        }
        let outras = self
            .termos
            .iter_mut()
            .flat_map(|t| t.evidencias.iter_mut())
            .chain(
                self.sugestoes
                    .iter_mut()
                    .filter_map(|s| s.evidencia.as_mut()),
            );
        for e in outras {
            marcar(e);
        }
        mudaram
    }

    /// Sessão ativa mais recente, se houver.
    pub fn sessao_ativa(&self) -> Option<&Sessao> {
        self.sessoes
            .iter()
            .filter(|s| s.estado != EstadoSessao::Encerrada)
            .max_by_key(|s| s.inicio_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cba_kb::Serie;

    fn analise() -> Analise {
        Analise::nova(
            Serie::Analise.id(7),
            "faturamento-exemplo",
            Fluxo::Perguntar,
            "Multa por atraso",
            "Como é calculada a multa por atraso?",
            1_000,
        )
    }

    fn evidencia() -> Evidencia {
        Evidencia {
            arquivo: CaminhoRelativo::novo("db/03_pkg_cobranca.sql").unwrap(),
            linhas: Linhas::novo(10, 12).unwrap(),
            trecho: "v_multa := v_valor * 0.02;".into(),
            hash: "abc".into(),
            commit: None,
            estado: EstadoEvidencia::Valida,
        }
    }

    fn afirmacao(confianca: Confianca, evidencias: Vec<Evidencia>) -> NovaAfirmacao {
        NovaAfirmacao {
            secao: "regras".into(),
            texto: "A multa é de 2%.".into(),
            confianca,
            evidencias,
            sessao: None,
        }
    }

    #[test]
    fn nova_comeca_criada_com_documento_na_pasta_de_analises() {
        let a = analise();
        assert_eq!(a.estado, EstadoAnalise::Criada);
        assert_eq!(a.documento.as_str(), "analises/A-0007.md");
        assert_eq!(a.fluxo.tipo_documento(), TipoDocumento::Pergunta);
        assert_eq!(a.estado.status_documento(), StatusAnalise::Rascunho);
    }

    #[test]
    fn registro_numera_e_poe_em_andamento() {
        let mut a = analise();
        let n1 = a
            .registrar_afirmacao(afirmacao(Confianca::Confirmado, vec![evidencia()]), 2)
            .unwrap();
        let n2 = a
            .registrar_afirmacao(afirmacao(Confianca::Inferido, vec![]), 3)
            .unwrap();
        assert_eq!((n1, n2), (1, 2));
        assert_eq!(a.estado, EstadoAnalise::EmAndamento);
        let p = a
            .registrar_pergunta("Vale para PJ?", Some(2), None, 4)
            .unwrap();
        assert_eq!(p, 1);
        assert_eq!(
            a.registrar_pergunta("x", Some(9), None, 4),
            Err(ErroModelo::ItemDesconhecido {
                tipo: "afirmação",
                numero: 9
            })
        );
    }

    #[test]
    fn confirmado_exige_evidencia_e_campos_nao_vazios() {
        let mut a = analise();
        assert_eq!(
            a.registrar_afirmacao(afirmacao(Confianca::Confirmado, vec![]), 2),
            Err(ErroModelo::ConfirmadoSemEvidencia)
        );
        let mut vazia = afirmacao(Confianca::Inferido, vec![]);
        vazia.secao = "  ".into();
        assert_eq!(
            a.registrar_afirmacao(vazia, 2),
            Err(ErroModelo::CampoVazio("secao"))
        );
        // Recusa não muda o estado.
        assert_eq!(a.estado, EstadoAnalise::Criada);
        assert!(a.afirmacoes.is_empty());
    }

    #[test]
    fn ciclo_da_analise() {
        let mut a = analise();
        assert!(matches!(
            a.concluir(),
            Err(ErroModelo::TransicaoDaAnalise { .. })
        ));
        a.iniciar_sessao("s1", Cliente::ClaudeCode, 5).unwrap();
        assert_eq!(a.estado, EstadoAnalise::EmAndamento);
        a.concluir().unwrap();
        assert_eq!(a.estado.status_documento(), StatusAnalise::EmRevisao);
        // Registro depois de concluir reabre a rodada.
        a.registrar_pergunta("Outra?", None, None, 6).unwrap();
        assert_eq!(a.estado, EstadoAnalise::EmAndamento);
        assert!(a.aprovar().is_err());
        a.concluir().unwrap();
        a.aprovar().unwrap();
        assert_eq!(
            a.registrar_pergunta("Mais?", None, None, 7),
            Err(ErroModelo::AnaliseAprovada(a.id.clone()))
        );
        assert!(a.iniciar_sessao("s2", Cliente::ClaudeCode, 8).is_err());
    }

    #[test]
    fn sessoes() {
        let mut a = analise();
        a.iniciar_sessao("s1", Cliente::ClaudeCode, 5).unwrap();
        a.atualizar_sessao("s1", EstadoSessao::AguardandoAnalista, 6)
            .unwrap();
        a.atualizar_sessao("s1", EstadoSessao::Ativa, 7).unwrap();
        a.atualizar_sessao("s1", EstadoSessao::Encerrada, 8)
            .unwrap();
        assert_eq!(a.sessoes[0].fim_ms, Some(8));
        assert!(a.sessao_ativa().is_none());
        assert!(matches!(
            a.atualizar_sessao("s1", EstadoSessao::Ativa, 9),
            Err(ErroModelo::TransicaoDaSessao { .. })
        ));
        assert_eq!(
            a.atualizar_sessao("s9", EstadoSessao::Ativa, 9),
            Err(ErroModelo::SessaoDesconhecida("s9".into()))
        );
        // Retomada reativa a mesma sessão, sem duplicar.
        a.iniciar_sessao("s1", Cliente::Desconhecido, 10).unwrap();
        assert_eq!(a.sessoes.len(), 1);
        assert_eq!(a.sessoes[0].estado, EstadoSessao::Ativa);
        assert_eq!(a.sessoes[0].cliente, Cliente::ClaudeCode);
        assert_eq!(a.sessao_ativa().unwrap().id, "s1");
    }

    #[test]
    fn decisoes_do_analista() {
        let mut a = analise();
        a.registrar_afirmacao(afirmacao(Confianca::Inferido, vec![]), 2)
            .unwrap();
        assert_eq!(
            a.marcar_afirmacao(
                1,
                EstadoAfirmacao::Incorreta {
                    comentario: " ".into()
                }
            ),
            Err(ErroModelo::CampoVazio("comentario"))
        );
        a.marcar_afirmacao(1, EstadoAfirmacao::Confirmada).unwrap();
        assert!(a.marcar_afirmacao(2, EstadoAfirmacao::Confirmada).is_err());

        a.registrar_pergunta("Vale para PJ?", None, None, 3)
            .unwrap();
        a.decidir_pergunta(
            1,
            EstadoPergunta::Respondida {
                resposta: "Sim".into(),
            },
        )
        .unwrap();
        assert!(matches!(
            a.decidir_pergunta(1, EstadoPergunta::Descartada),
            Err(ErroModelo::ItemDecidido { .. })
        ));

        let nova = NovaSugestao {
            id: None,
            titulo: "Multa de 3%".into(),
            exemplo: None,
            evidencia: None,
            arquivo: CaminhoRelativo::novo("db/03_pkg_cobranca.sql").unwrap(),
            trecho_atual: "0.02".into(),
            trecho_proposto: "0.03".into(),
            motivo: "Nova regra".into(),
            impacto: "Multas maiores".into(),
            sessao: Some("s1".into()),
        };
        assert_eq!(a.registrar_sugestao(nova, 4).unwrap(), 1);
        a.decidir_sugestao(1, StatusSugestao::Aceita).unwrap();
        assert!(a.decidir_sugestao(1, StatusSugestao::Recusada).is_err());
    }

    #[test]
    fn termos_propostos() {
        let mut a = analise();
        let novo = |termo: &str| NovoTermo {
            termo: termo.into(),
            definicao: "Valor cobrado por atraso.".into(),
            sinonimos: vec![" encargo ".into(), "encargo".into(), "".into()],
            onde_vive: vec!["PKG_COBRANCA.FN_CALCULAR_MULTA".into()],
            area: Some(" ".into()),
            evidencias: vec![],
            sessao: None,
        };
        assert_eq!(a.registrar_termo(novo("Multa"), 2).unwrap(), 1);
        assert_eq!(a.termos[0].sinonimos, vec!["encargo".to_owned()]);
        assert_eq!(a.termos[0].area, None);
        assert_eq!(a.estado, EstadoAnalise::EmAndamento);
        assert_eq!(
            a.registrar_termo(novo(" multa "), 3),
            Err(ErroModelo::TermoRepetido {
                termo: "Multa".into(),
                numero: 1
            })
        );
        let mut vazio = novo("Juros");
        vazio.definicao = " ".into();
        assert_eq!(
            a.registrar_termo(vazio, 3),
            Err(ErroModelo::CampoVazio("definicao"))
        );
        a.decidir_termo(1, EstadoTermo::Aceito).unwrap();
        assert!(matches!(
            a.decidir_termo(1, EstadoTermo::Recusado),
            Err(ErroModelo::ItemDecidido { .. })
        ));
    }

    #[test]
    fn hash_diferente_marca_a_revalidar() {
        let mut a = analise();
        a.registrar_afirmacao(afirmacao(Confianca::Confirmado, vec![evidencia()]), 2)
            .unwrap();
        assert_eq!(a.marcar_a_revalidar(|e| e.hash == "abc"), 0);
        assert_eq!(a.marcar_a_revalidar(|e| e.hash == "def"), 1);
        // Já marcada: não conta de novo.
        assert_eq!(a.marcar_a_revalidar(|e| e.hash == "def"), 0);
        assert_eq!(a.afirmacoes[0].estado, EstadoAfirmacao::ARevalidar);
        assert_eq!(
            a.afirmacoes[0].evidencias[0].estado,
            EstadoEvidencia::ARevalidar
        );
    }

    #[test]
    fn json_ida_e_volta_com_estados_legiveis() {
        let mut a = analise();
        a.registrar_afirmacao(afirmacao(Confianca::AConfirmar, vec![evidencia()]), 2)
            .unwrap();
        a.marcar_afirmacao(
            1,
            EstadoAfirmacao::Incorreta {
                comentario: "É 3%".into(),
            },
        )
        .unwrap();
        let json = serde_json::to_value(&a).unwrap();
        assert_eq!(json["id"], "A-0007");
        assert_eq!(json["estado"], "em-andamento");
        assert_eq!(json["afirmacoes"][0]["confianca"], "a_confirmar");
        assert_eq!(json["afirmacoes"][0]["estado"], "incorreta");
        assert_eq!(json["afirmacoes"][0]["comentario"], "É 3%");
        assert_eq!(json["afirmacoes"][0]["evidencias"][0]["linhas"], "10-12");
        let volta: Analise = serde_json::from_value(json).unwrap();
        assert_eq!(volta, a);
    }
}
