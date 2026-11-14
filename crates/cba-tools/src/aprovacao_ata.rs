//! Aprovação da ata de uma reunião, com o descarte do transcript (épico #93, #142;
//! plano das reuniões, seção 3, passo 6, e seção 5).
//!
//! A análise do fluxo `reuniao` não vira regras: o que ela produz é a ata. Por isso
//! ela não passa pela aprovação das análises ([`crate::revisao::aprovar_revisada`]
//! recusa o fluxo `reuniao`) e sim por [`aprovar_ata`], na mesma tela de revisão.
//!
//! # Segundo aviso
//!
//! Aprovar apaga o transcript, e as citações da ata deixam de ser conferíveis
//! ([`AVISO_DESCARTE`]). A tela mostra o aviso e pede a confirmação explícita; sem
//! ela ([`aprovar_ata`] com `confirmacao = false`), nada é lido nem gravado: a ata
//! continua em rascunho e o transcript continua guardado.
//!
//! # Ordem e falhas
//!
//! 1. confere a análise (fluxo `reuniao`, ligada a uma reunião) e, com a ata ainda em
//!    rascunho, as pendências ([`ProntidaoAta::pendencias`]: a análise concluída
//!    pelo agente e ao menos um item na ata);
//! 2. **descarta o transcript** ([`Guarda::descartar`]): apaga a guarda e marca as
//!    citações conferidas como "conferida em `hoje`", regravando a ata;
//! 3. marca a ata como **aprovada**, sob a trava da ata
//!    ([`Templates::alterar_ata`]), e a regrava;
//! 4. aprova a análise (concluída → aprovada), pela aprovação das análises
//!    ([`aprovacao::aprovar_analise`], que também sincroniza o índice da base).
//!
//! O descarte vem antes da aprovação da ata para que a ata **nunca fique aprovada
//! com o transcript guardado**: a ata aprovada some do aviso de transcript guardado
//! do painel da demanda, e um transcript órfão ficaria sem aviso até a retenção.
//! Se o descarte falha, nada mais acontece: a ata continua em rascunho e o painel
//! continua avisando. Se a aprovação da ata falha depois do descarte, o transcript
//! já não existe e a ata continua em rascunho (com as citações "conferida em"); a
//! recusa diz isso, e aprovar de novo conclui. Tudo é idempotente: repetir com a
//! ata já aprovada repete o descarte (que não muda nada se já foi feito) e a
//! aprovação da análise, e serve para completar uma aprovação que parou no meio.
//! A falha só na aprovação da análise (passo 4) vira aviso: a ata já está aprovada
//! e o transcript apagado, e a prontidão continua oferecendo a aprovação.
//!
//! # Privacidade
//!
//! O log e o diário de eventos recebem só identificadores e contagens; nenhuma
//! fala sai daqui. Na base ficam só os trechos curtos citados na ata.

use cba_core::ambiente::Ambiente;
use cba_core::analise::{Analise, EstadoAnalise, Fluxo};
use cba_core::analises;
use cba_docs::{ErroAta, EstadoAta, StatusCitacao, Templates};
use cba_kb::frontmatter::{Data, StatusAta};
use cba_kb::{Id, IdReuniao};
use cba_reunioes::{ErroGuarda, Guarda, MotivoDescarte};
use serde::Serialize;

use crate::aprovacao::{self, OpcoesAprovacao};

/// O segundo aviso, mostrado antes de confirmar a aprovação.
pub const AVISO_DESCARTE: &str = "Ao aprovar a ata, o transcript da reunião é apagado do \
     diretório de dados do app e as citações da ata deixam de ser conferíveis: elas ficam \
     marcadas como conferidas na data de hoje. Na base ficam só a ata e os trechos curtos \
     citados nela.";

/// Por que a aprovação da ata foi recusada.
#[derive(Debug, thiserror::Error)]
pub enum ErroAprovacaoAta {
    #[error(
        "confirme o aviso antes de aprovar a ata: o transcript será apagado e as \
         citações deixam de ser conferíveis; a ata continua em rascunho"
    )]
    SemConfirmacao,
    #[error(transparent)]
    Analise(#[from] analises::Erro),
    #[error("a análise {0} não é de uma reunião; aprove-a pela aprovação das análises")]
    NaoEReuniao(Id),
    #[error("a análise {0} não está ligada a uma reunião")]
    SemReuniao(Id),
    #[error(transparent)]
    Ata(#[from] ErroAta),
    #[error("a ata ainda não pode ser aprovada: {}", .0.join("; "))]
    Pendente(Vec<String>),
    #[error(
        "o transcript da reunião {reuniao} não foi apagado ({erro}); a ata continua em \
         rascunho. Tente aprovar de novo"
    )]
    Descarte {
        reuniao: IdReuniao,
        erro: ErroGuarda,
    },
    #[error(
        "o transcript da reunião {reuniao} já foi apagado, mas a ata não foi marcada como \
         aprovada ({erro}); aprove de novo para concluir"
    )]
    AprovacaoIncompleta { reuniao: IdReuniao, erro: ErroAta },
}

/// Se a ata pode ser aprovada e o que a aprovação fará.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProntidaoAta {
    /// `R-0001`.
    pub reuniao: IdReuniao,
    pub status: StatusAta,
    pub estado_analise: EstadoAnalise,
    /// A aprovação foi até o fim: ata aprovada, transcript apagado e análise
    /// aprovada. Sem isso, a aprovação continua oferecida (para completar).
    pub concluida: bool,
    pub pode_aprovar: bool,
    /// O que impede a aprovação.
    pub pendencias: Vec<String>,
    /// O que não impede, mas o analista deve saber.
    pub avisos: Vec<String>,
    /// O transcript ainda está guardado no diretório de dados do app.
    pub transcript_guardado: bool,
    /// Citações que ainda podem ser conferidas contra o transcript: viram
    /// "conferida em <data>" na aprovação.
    pub citacoes_conferiveis: usize,
    pub itens: usize,
    /// O segundo aviso ([`AVISO_DESCARTE`]).
    pub aviso_descarte: &'static str,
}

/// Resultado da aprovação da ata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AtaAprovada {
    pub reuniao: IdReuniao,
    /// A ata já estava aprovada (a chamada só completou ou conferiu).
    pub ja_aprovada: bool,
    /// O transcript foi apagado agora (falso se já tinha sido).
    pub transcript_apagado: bool,
    /// Citações que passaram a "conferida em <data>".
    pub citacoes_marcadas: usize,
    /// Data da aprovação e do descarte.
    pub data: Data,
    pub avisos: Vec<String>,
}

/// A análise do fluxo `reuniao` e a reunião dela.
fn analise_da_reuniao(amb: &Ambiente, id: &Id) -> Result<(Analise, IdReuniao), ErroAprovacaoAta> {
    let analise = amb.analises.carregar(id)?;
    if analise.fluxo != Fluxo::Reuniao {
        return Err(ErroAprovacaoAta::NaoEReuniao(id.clone()));
    }
    let reuniao = analise
        .reuniao
        .ok_or_else(|| ErroAprovacaoAta::SemReuniao(id.clone()))?;
    Ok((analise, reuniao))
}

/// O que impede aprovar a ata em rascunho.
fn pendencias(analise: &Analise, estado: &EstadoAta) -> Vec<String> {
    let mut lista = Vec::new();
    if estado.status == StatusAta::Aprovada {
        return lista;
    }
    if !matches!(
        analise.estado,
        EstadoAnalise::Concluida | EstadoAnalise::Aprovada
    ) {
        lista.push("o agente ainda não concluiu a ata (concluir_analise)".to_owned());
    }
    if estado.itens.is_empty() {
        lista.push("a ata não tem itens registrados".to_owned());
    }
    lista
}

fn citacoes_conferiveis(estado: &EstadoAta) -> usize {
    let das_propostas = estado
        .participantes
        .iter()
        .filter_map(|p| p.proposta.as_ref())
        .flat_map(|p| p.citacoes.iter());
    estado
        .itens
        .iter()
        .flat_map(|i| i.citacoes.iter())
        .chain(das_propostas)
        .filter(|c| c.status == StatusCitacao::Conferida)
        .count()
}

/// Prontidão da ata da análise `id` (fluxo `reuniao`) para a aprovação.
pub fn prontidao_ata(amb: &Ambiente, id: &Id) -> Result<ProntidaoAta, ErroAprovacaoAta> {
    let (analise, reuniao) = analise_da_reuniao(amb, id)?;
    let (estado, _) = EstadoAta::ler(&amb.kb, reuniao)?;
    let pendencias = pendencias(&analise, &estado);
    let transcript_guardado = Guarda::nova(&amb.dados).existe(amb.kb.nome(), reuniao);
    let mut avisos = Vec::new();
    let sem_papel = estado
        .participantes
        .iter()
        .filter(|p| p.papel.is_none())
        .count();
    if sem_papel > 0 && estado.status == StatusAta::Rascunho {
        avisos.push(format!(
            "{sem_papel} participante(s) sem papel confirmado; depois da aprovação, os \
             participantes não mudam mais"
        ));
    }
    let nao_conferem = estado
        .itens
        .iter()
        .flat_map(|i| i.citacoes.iter())
        .filter(|c| c.status == StatusCitacao::NaoConfere)
        .count();
    if nao_conferem > 0 {
        avisos.push(format!(
            "{nao_conferem} citação(ões) não conferem com o transcript e ficam marcadas assim"
        ));
    }
    if !transcript_guardado && estado.status == StatusAta::Rascunho {
        avisos.push("o transcript desta reunião já não está guardado".to_owned());
    }
    let concluida = estado.status == StatusAta::Aprovada
        && analise.estado == EstadoAnalise::Aprovada
        && !transcript_guardado;
    Ok(ProntidaoAta {
        reuniao,
        status: estado.status,
        estado_analise: analise.estado,
        concluida,
        pode_aprovar: pendencias.is_empty() && !concluida,
        pendencias,
        avisos,
        transcript_guardado,
        citacoes_conferiveis: citacoes_conferiveis(&estado),
        itens: estado.itens.len(),
        aviso_descarte: AVISO_DESCARTE,
    })
}

/// Aprova a ata da análise `id` (fluxo `reuniao`), descarta o transcript e aprova
/// a análise, nesta ordem (veja o [módulo](self)). Sem `confirmacao`, recusa antes
/// de ler ou gravar qualquer coisa.
pub fn aprovar_ata(
    amb: &Ambiente,
    id: &Id,
    confirmacao: bool,
    hoje: Data,
) -> Result<AtaAprovada, ErroAprovacaoAta> {
    if !confirmacao {
        return Err(ErroAprovacaoAta::SemConfirmacao);
    }
    let (analise, reuniao) = analise_da_reuniao(amb, id)?;
    let (estado, _) = EstadoAta::ler(&amb.kb, reuniao)?;
    let pendentes = pendencias(&analise, &estado);
    if !pendentes.is_empty() {
        return Err(ErroAprovacaoAta::Pendente(pendentes));
    }
    let ja_aprovada = estado.status == StatusAta::Aprovada;
    let templates = Templates::da_base(&amb.base).0;

    // 1. O descarte: o transcript sai antes de a ata ficar aprovada.
    let descarte = Guarda::nova(&amb.dados)
        .descartar(
            &amb.kb,
            &templates,
            reuniao,
            hoje,
            MotivoDescarte::AtaAprovada,
        )
        .map_err(|erro| ErroAprovacaoAta::Descarte { reuniao, erro })?;

    // 2. A ata aprovada, sob a trava (o servidor MCP também grava nela).
    let (marcadas_agora, _, _) = templates
        .alterar_ata(&amb.kb, reuniao, |e: &mut EstadoAta| {
            // Nenhuma citação conferível sobra sem o transcript.
            let marcadas = e.marcar_conferidas_em(hoje);
            e.status = StatusAta::Aprovada;
            Ok::<_, ErroAta>(marcadas)
        })
        .map_err(|erro| ErroAprovacaoAta::AprovacaoIncompleta { reuniao, erro })?;
    crate::reuniao::notificar_ata(amb, reuniao);

    // 3. A análise aprovada (idempotente).
    let mut avisos = Vec::new();
    match aprovacao::aprovar_analise(&amb.dados, &amb.projeto.id, id, &OpcoesAprovacao::default()) {
        Ok(feita) => avisos.extend(feita.avisos),
        Err(e) => {
            tracing::warn!(analise = %id, reuniao = %reuniao, "análise da ata não aprovada");
            avisos.push(format!(
                "a ata foi aprovada e o transcript apagado, mas a análise {id} não mudou \
                 para aprovada ({e}); aprove de novo para concluir"
            ));
        }
    }
    tracing::info!(
        projeto = amb.kb.nome(),
        reuniao = %reuniao,
        analise = %id,
        ja_aprovada,
        transcript_apagado = descarte.transcript_apagado,
        citacoes = descarte.citacoes_marcadas + marcadas_agora,
        "ata aprovada"
    );
    Ok(AtaAprovada {
        reuniao,
        ja_aprovada,
        transcript_apagado: descarte.transcript_apagado,
        citacoes_marcadas: descarte.citacoes_marcadas + marcadas_agora,
        data: hoje,
        avisos,
    })
}
