//! Comandos Tauri da reunião (épico #93, #141): a nova reunião com a prévia do
//! transcript e o aviso de privacidade, a confirmação dos participantes e a ata ao
//! vivo.
//!
//! # Nova reunião
//!
//! A tela lê o transcript (texto colado ou `.vtt`/`.docx` anexado) com o parser
//! determinístico de `cba_reunioes` e mostra a prévia: formato, quantidade de falas,
//! falantes e avisos. A reunião só é criada com o **aviso de privacidade aceito**
//! (o transcript vai ao provedor de IA usado no Claude Code, fica só no diretório de
//! dados do app, fora da base, e é apagado ao aprovar a ata ou no fim do prazo de
//! retenção). Ao criar: a demanda (escolhida ou nova), a reunião (`R-NNNN`) com o
//! estado inicial da ata (os falantes como participantes), a guarda do transcript e
//! a análise do fluxo `reuniao` ligada à demanda e à reunião, com o pedido que abre
//! a conversa.
//!
//! # Participantes
//!
//! Cada participante da ata aparece com o papel sugerido pelo catálogo de pessoas
//! (sem IA) e o proposto pelo agente (`propor_papel`, com as citações). Ao
//! confirmar, o catálogo muda (pessoa registrada, apelidos, papel confirmado na
//! reunião `D-NNNN/R-NNNN`, pessoas juntadas) e a ata recebe os papéis.
//!
//! # Aprovação da ata (#142)
//!
//! Na mesma tela de revisão das análises: a prontidão e a aprovação ficam em
//! [`cba_tools::aprovacao_ata`] (o segundo aviso, a confirmação explícita, o
//! descarte do transcript antes de marcar a ata como aprovada, e a repetição
//! idempotente). Depois da aprovação, cada próximo passo (`PP-n`) vira botões que
//! abrem análises já ligadas à demanda, com a ata como contexto
//! ([`analisar_proximo_passo`]).
//!
//! # Privacidade
//!
//! Nenhuma fala sai daqui: a prévia só traz os nomes dos falantes, contagens e os
//! avisos do parser; os logs e o diário de eventos só recebem identificadores.

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use cba_core::analise::Fluxo;
use cba_core::config::Config;
use cba_core::dados::DiretorioDados;
use cba_core::eventos::{DiarioEventos, Evento, Mudanca, Notificador};
use cba_core::tempo;
use cba_docs::{
    CitacaoFala, ErroAta, EstadoAta, IdItemAta, ItemAta, Participante, Templates, TipoItem,
};
use cba_kb::frontmatter::{Data, Demanda, StatusAta};
use cba_kb::{
    Casamento, CatalogoDePessoas, DemandaLida, Id, IdDemanda, IdReuniao, NovaDemanda, NovaPessoa,
    Papeis, normalizar_nome,
};
use cba_reunioes::{Formato, Guarda, MotivoDescarte, Transcript};
use serde::{Deserialize, Serialize};

use crate::analise::{AnaliseCriada, ambiente, criar_analise_com_demanda, titulo_da_entrada};
use crate::demandas::{TranscriptGuardado, criar_analise_ligada};
use cba_tools::aprovacao_ata::{self, AtaAprovada, ProntidaoAta};

/// Tamanho máximo do arquivo do transcript anexado.
pub const TAMANHO_MAXIMO_DO_ARQUIVO: u64 = 20 * 1024 * 1024;

/// Recusa da criação sem o aviso de privacidade aceito.
pub const AVISO_NAO_ACEITO: &str = "aceite o aviso de privacidade antes de criar a reunião: o \
     transcript vai ao provedor de IA usado no Claude Code e fica guardado no diretório de \
     dados do app até ser apagado";

// ---------------------------------------------------------------------------
// Entrada e prévia

/// Transcript informado na tela: o texto colado ou o arquivo anexado.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "tipo", rename_all = "camelCase")]
pub enum EntradaTranscript {
    Texto {
        texto: String,
    },
    /// Caminho do `.vtt`, `.docx` ou `.txt` escolhido no diálogo.
    Arquivo {
        caminho: PathBuf,
    },
}

impl EntradaTranscript {
    /// Lê o transcript. O parser nunca falha: o que ele não entende vira aviso. A
    /// falha aqui é só a do arquivo (inexistente, grande demais).
    pub fn ler(&self) -> Result<Transcript, String> {
        match self {
            Self::Texto { texto } => Ok(cba_reunioes::analisar_texto(texto)),
            Self::Arquivo { caminho } => {
                let erro = |e: std::io::Error| format!("não foi possível ler o arquivo: {e}");
                let meta = fs::metadata(caminho).map_err(erro)?;
                if !meta.is_file() {
                    return Err("o caminho escolhido não é um arquivo".into());
                }
                if meta.len() > TAMANHO_MAXIMO_DO_ARQUIVO {
                    return Err(format!(
                        "o arquivo tem {} MB; o limite é {} MB",
                        meta.len().div_ceil(1024 * 1024),
                        TAMANHO_MAXIMO_DO_ARQUIVO / (1024 * 1024)
                    ));
                }
                let bytes = fs::read(caminho).map_err(erro)?;
                let nome = caminho.file_name().and_then(|n| n.to_str());
                Ok(cba_reunioes::analisar(&bytes, nome))
            }
        }
    }
}

/// Falante do transcript e quantas falas ele tem.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FalanteVista {
    pub nome: String,
    pub falas: usize,
}

/// Aviso do parser: o código estável e a frase para a tela (sem trecho de fala).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvisoVista {
    pub codigo: String,
    pub mensagem: String,
}

/// Prévia do transcript, antes de criar a reunião. Sem o texto das falas.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviaTranscript {
    pub formato: Formato,
    pub falas: usize,
    /// Na ordem da primeira fala de cada um.
    pub falantes: Vec<FalanteVista>,
    /// Falas sem falante identificado.
    pub sem_falante: usize,
    pub avisos: Vec<AvisoVista>,
    /// Há ao menos uma fala: a reunião pode ser criada.
    pub pode_criar: bool,
}

/// Prévia do transcript lido.
pub fn previa(t: &Transcript) -> PreviaTranscript {
    let falantes = t
        .falantes()
        .into_iter()
        .map(|nome| FalanteVista {
            nome: nome.to_owned(),
            falas: t
                .falas
                .iter()
                .filter(|f| f.falante.as_deref() == Some(nome))
                .count(),
        })
        .collect();
    PreviaTranscript {
        formato: t.formato,
        falas: t.falas.len(),
        falantes,
        sem_falante: t.falas.iter().filter(|f| f.falante.is_none()).count(),
        avisos: t
            .avisos
            .iter()
            .map(|a| AvisoVista {
                codigo: a.codigo().to_owned(),
                mensagem: a.to_string(),
            })
            .collect(),
        pode_criar: !t.falas.is_empty(),
    }
}

pub fn previa_transcript(entrada: &EntradaTranscript) -> Result<PreviaTranscript, String> {
    Ok(previa(&entrada.ler()?))
}

// ---------------------------------------------------------------------------
// Nova reunião

/// Demanda da reunião: uma aberta do projeto ou uma nova.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "tipo", rename_all = "camelCase")]
pub enum DemandaDaReuniao {
    Existente {
        id: String,
    },
    Nova {
        titulo: String,
        /// Chave no rastreador (`PROJ-123`), opcional.
        #[serde(default)]
        chave: Option<String>,
    },
}

/// Reunião criada, com a análise do fluxo `reuniao` e o pedido que abre a conversa.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReuniaoCriada {
    /// `R-0001`.
    pub reuniao: String,
    /// `D-0001`.
    pub demanda: String,
    #[serde(flatten)]
    pub analise: AnaliseCriada,
}

/// Pedido inicial da conversa da análise de uma reunião. Sem aspas, `%`, `!`,
/// parênteses nem quebras de linha (spec, seção 6.4).
pub fn prompt_da_reuniao(analise: &Id, reuniao: IdReuniao) -> String {
    format!(
        "Inicie a análise {analise} pelo ctower: chame a ferramenta iniciar_analise do cba \
         com analise {analise} e reuniao {reuniao} e siga o roteiro devolvido."
    )
}

fn id_da_demanda(texto: &str) -> Result<IdDemanda, String> {
    IdDemanda::analisar(texto.trim()).map_err(|e| e.to_string())
}

fn id_da_reuniao(texto: &str) -> Result<IdReuniao, String> {
    IdReuniao::analisar(texto.trim()).map_err(|e| e.to_string())
}

fn sem_vazio(texto: &Option<String>) -> Option<String> {
    texto
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
}

/// Demanda escolhida (precisa estar aberta) ou criada agora.
fn demanda_da_reuniao(
    kb: &cba_kb::Projeto,
    demanda: &DemandaDaReuniao,
    hoje: Data,
) -> Result<DemandaLida, String> {
    match demanda {
        DemandaDaReuniao::Existente { id } => {
            let id = id_da_demanda(id)?;
            let lida = kb.demanda(id).map_err(|e| e.to_string())?;
            if !lida.aberta() {
                return Err(format!(
                    "a demanda {id} está encerrada; reabra-a para registrar uma reunião nela"
                ));
            }
            Ok(lida)
        }
        DemandaDaReuniao::Nova { titulo, chave } => {
            if titulo.trim().is_empty() {
                return Err("informe o título da demanda nova".into());
            }
            kb.criar_demanda(
                &NovaDemanda {
                    titulo: titulo.trim().to_owned(),
                    descricao: None,
                    chave: sem_vazio(chave),
                    origem: None,
                },
                hoje,
            )
            .map_err(|e| e.to_string())
        }
    }
}

/// Cria a reunião e a análise que monta a ata. Sem `aviso_aceito`, recusa antes de
/// ler ou gravar qualquer coisa. Uma falha no meio desfaz o que já foi gravado da
/// reunião (estado, ata e transcript); a demanda nova, se criada, fica.
pub fn criar_reuniao(
    dados: &DiretorioDados,
    projeto: &str,
    demanda: &DemandaDaReuniao,
    entrada: &EntradaTranscript,
    aviso_aceito: bool,
) -> Result<ReuniaoCriada, String> {
    if !aviso_aceito {
        return Err(AVISO_NAO_ACEITO.into());
    }
    let transcript = entrada.ler()?;
    if transcript.falas.is_empty() {
        let motivo = transcript
            .avisos
            .first()
            .map(ToString::to_string)
            .unwrap_or_default();
        return Err(format!("o transcript não tem falas. {motivo}")
            .trim()
            .to_owned());
    }
    let amb = ambiente(dados, projeto)?;
    let kb = &amb.kb;
    let hoje = tempo::hoje();
    let lida = demanda_da_reuniao(kb, demanda, hoje)?;

    let (mut estado, arquivos) = EstadoAta::criar(kb, lida.id, hoje).map_err(|e| e.to_string())?;
    let r = estado.id;
    let desfazer = |guardado: bool| {
        if guardado {
            let _ = Guarda::nova(dados).apagar(kb.nome(), r, MotivoDescarte::ReuniaoDescartada);
        }
        for arquivo in [&arquivos.estado, &arquivos.ata] {
            let _ = fs::remove_file(kb.caminho(arquivo));
        }
    };

    // Os falantes entram como participantes, ligados à pessoa do catálogo quando o
    // nome casa com uma só; o papel fica para o analista confirmar.
    let sugestoes = kb
        .sugerir_participantes(&transcript.falantes())
        .unwrap_or_default();
    estado.definir_participantes(
        sugestoes
            .into_iter()
            .map(|s| {
                let mut p = Participante::novo(s.falante);
                p.pessoa = s.casamento.pessoa().map(|p| p.slug.clone());
                p
            })
            .collect(),
    );
    let templates = Templates::da_base(&amb.base).0;
    if let Err(e) = templates.gravar_ata(kb, &estado) {
        desfazer(false);
        return Err(e.to_string());
    }
    if let Err(e) = Guarda::nova(dados).guardar(kb.nome(), r, &transcript, tempo::agora_ms()) {
        desfazer(false);
        return Err(e.to_string());
    }

    let f = &lida.documento.frontmatter;
    let entrada_da_analise = format!(
        "Reunião {r} da demanda {} ({}). Ata: `{}`.",
        lida.id,
        f.titulo,
        kb.na_base(&arquivos.ata)
    );
    let titulo = titulo_da_entrada(&format!("Reunião {r} · {}", f.titulo));
    let criada = criar_analise_com_demanda(
        dados,
        projeto,
        Fluxo::Reuniao,
        &entrada_da_analise,
        Some(&titulo),
        Some(Demanda {
            id: Some(lida.id),
            origem: f.origem.clone().unwrap_or_else(|| "texto".into()),
            chave: f.chave.clone(),
        }),
    );
    let mut criada = match criada {
        Ok(c) => c,
        Err(e) => {
            desfazer(true);
            return Err(e);
        }
    };
    // A reunião vai para o estado da análise já aqui: `retomar_analise` e o
    // `iniciar_analise` só com o A-NNNN acham a reunião.
    let id = Id::analisar(&criada.analise.id).map_err(|e| e.to_string())?;
    amb.analises
        .alterar(&id, Mudanca::Estado, |a, _| {
            a.reuniao = Some(r);
            Ok(())
        })
        .map_err(|e| e.to_string())?;
    criada.prompt_inicial = prompt_da_reuniao(&id, r);
    tracing::info!(
        projeto,
        reuniao = %r,
        demanda = %lida.id,
        analise = %id,
        falas = transcript.falas.len(),
        "reunião criada"
    );
    Ok(ReuniaoCriada {
        reuniao: r.to_string(),
        demanda: lida.id.to_string(),
        analise: criada,
    })
}

// ---------------------------------------------------------------------------
// Ata ao vivo

/// Fala citada por um item ou por uma proposta de papel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CitacaoVista {
    pub falante: String,
    /// `HH:MM:SS`.
    pub inicio: String,
    pub trecho: String,
    /// "conferida", "conferida em AAAA-MM-DD" ou "não confere".
    pub status: String,
}

impl From<&CitacaoFala> for CitacaoVista {
    fn from(c: &CitacaoFala) -> Self {
        Self {
            falante: c.falante.clone(),
            inicio: c.inicio.to_string(),
            trecho: c.trecho.clone(),
            status: c.status.rotulo(),
        }
    }
}

/// Item da ata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemVista {
    /// `D-1`, `P-2`.
    pub id: String,
    pub tipo: TipoItem,
    pub texto: String,
    pub responsavel: Option<String>,
    pub prazo: Option<String>,
    pub referencias: Vec<String>,
    pub citacoes: Vec<CitacaoVista>,
    /// Evidências de código (`arquivo:linhas`).
    pub evidencias: Vec<String>,
}

impl From<&ItemAta> for ItemVista {
    fn from(i: &ItemAta) -> Self {
        Self {
            id: i.id.to_string(),
            tipo: i.tipo,
            texto: i.texto.clone(),
            responsavel: i.responsavel.clone(),
            prazo: i.prazo.clone(),
            referencias: i.referencias.iter().map(ToString::to_string).collect(),
            citacoes: i.citacoes.iter().map(CitacaoVista::from).collect(),
            evidencias: i
                .evidencias
                .iter()
                .map(|e| format!("{}:{}", e.arquivo, e.linhas))
                .collect(),
        }
    }
}

/// Papel proposto pelo agente.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PropostaVista {
    pub papel: String,
    pub justificativa: Option<String>,
    pub citacoes: Vec<CitacaoVista>,
}

/// Pessoa do catálogo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PessoaVista {
    /// Slug (`maria-silva`).
    pub slug: String,
    pub nome: String,
}

/// Papel sugerido pelo catálogo, sem IA.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SugestaoCatalogo {
    pub pessoa: PessoaVista,
    pub papel: Option<String>,
    /// Reunião em que o papel foi confirmado (`D-0001/R-0001`); `None` quando é o
    /// papel informado no registro, sem confirmação.
    pub reuniao: Option<String>,
}

/// Participante da reunião, como a tela de participantes mostra.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParticipanteVista {
    pub nome: String,
    /// Papel confirmado pelo analista.
    pub papel: Option<String>,
    /// Pessoa do catálogo ligada ao participante (slug).
    pub pessoa: Option<String>,
    /// O que o catálogo sugere (casamento único do nome, ou a pessoa ligada).
    pub catalogo: Option<SugestaoCatalogo>,
    /// Pessoas do catálogo com o mesmo nome (casamento ambíguo): o analista escolhe.
    pub candidatos: Vec<PessoaVista>,
    pub proposta: Option<PropostaVista>,
}

/// Papel do catálogo de papéis da base.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PapelVista {
    pub nome: String,
    pub descricao: Option<String>,
}

/// A ata da reunião, relida a cada `Mudanca::Ata`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AtaVista {
    pub id: String,
    pub demanda: String,
    pub data: String,
    pub status: StatusAta,
    pub objetivo: Option<String>,
    pub resumo: Option<String>,
    pub participantes: Vec<ParticipanteVista>,
    /// Na ordem das seções da ata e, em cada uma, dos números.
    pub itens: Vec<ItemVista>,
    /// A ata (`R-NNNN.md`), relativa à raiz da base; `None` antes da montagem.
    pub documento: Option<String>,
    /// Papéis padrão da base, para a confirmação.
    pub papeis: Vec<PapelVista>,
    /// Transcript guardado sem a ata aprovada.
    pub transcript: Option<TranscriptGuardado>,
}

fn pessoa_vista(p: &cba_kb::PessoaLida) -> PessoaVista {
    PessoaVista {
        slug: p.slug.clone(),
        nome: p.nome().to_owned(),
    }
}

fn participante_vista(p: &Participante, catalogo: &CatalogoDePessoas) -> ParticipanteVista {
    let ligada = p
        .pessoa
        .as_deref()
        .and_then(|s| catalogo.pessoas().iter().find(|x| x.slug == s));
    let (pessoa, candidatos) = match ligada {
        Some(x) => (Some(x.clone()), Vec::new()),
        None => match catalogo.casar(&p.nome) {
            Casamento::Nenhuma => (None, Vec::new()),
            Casamento::Unica(x) => (Some(*x), Vec::new()),
            Casamento::Ambigua(v) => (None, v.iter().map(pessoa_vista).collect()),
        },
    };
    let catalogo = pessoa.map(|x| {
        let sugerido = x.papel_sugerido();
        SugestaoCatalogo {
            pessoa: pessoa_vista(&x),
            papel: sugerido.as_ref().map(|s| s.papel.clone()),
            reuniao: sugerido.and_then(|s| s.reuniao),
        }
    });
    ParticipanteVista {
        nome: p.nome.clone(),
        papel: p.papel.clone(),
        pessoa: p.pessoa.clone(),
        catalogo,
        candidatos,
        proposta: p.proposta.as_ref().map(|pr| PropostaVista {
            papel: pr.papel.clone(),
            justificativa: pr.justificativa.clone(),
            citacoes: pr.citacoes.iter().map(CitacaoVista::from).collect(),
        }),
    }
}

/// Prazo de retenção da configuração (o padrão, se ela não puder ser lida).
fn prazo_de_retencao(dados: &DiretorioDados) -> u32 {
    Config::ler(dados)
        .unwrap_or_default()
        .prazo_retencao_transcript()
}

/// Situação do transcript guardado de uma reunião com a ata não aprovada.
pub(crate) fn transcript_guardado(
    guarda: &Guarda,
    projeto: &str,
    reuniao: IdReuniao,
    status: StatusAta,
    hoje: Data,
    prazo_dias: u32,
) -> Option<TranscriptGuardado> {
    if status == StatusAta::Aprovada {
        return None;
    }
    match guarda.situacao(projeto, reuniao, hoje, prazo_dias) {
        Ok(s) => s.map(|s| TranscriptGuardado {
            apaga_em: s.apaga_em.to_string(),
            vence_em_breve: s.vence_em_breve || s.vencida,
        }),
        Err(e) => {
            tracing::warn!(projeto, reuniao = %reuniao, erro = %e, "guarda do transcript ilegível");
            None
        }
    }
}

fn vista_da_ata(
    dados: &DiretorioDados,
    amb: &cba_core::ambiente::Ambiente,
    estado: &EstadoAta,
) -> Result<AtaVista, String> {
    let kb = &amb.kb;
    let catalogo = kb.catalogo_de_pessoas().map_err(|e| e.to_string())?;
    let arquivos = kb.reuniao(estado.id).map_err(|e| e.to_string())?;
    let mut itens: Vec<&ItemAta> = estado.itens.iter().collect();
    itens.sort_by_key(|i| (i.tipo, i.id.numero));
    let (papeis, _) = Papeis::da_base(&amb.base);
    Ok(AtaVista {
        id: estado.id.to_string(),
        demanda: estado.demanda.to_string(),
        data: estado.data.to_string(),
        status: estado.status,
        objetivo: estado.objetivo.clone(),
        resumo: estado.resumo.clone(),
        participantes: estado
            .participantes
            .iter()
            .map(|p| participante_vista(p, &catalogo))
            .collect(),
        itens: itens.into_iter().map(ItemVista::from).collect(),
        documento: kb
            .existe(&arquivos.ata)
            .then(|| kb.na_base(&arquivos.ata).as_str().to_owned()),
        papeis: papeis
            .papeis()
            .iter()
            .map(|p| PapelVista {
                nome: p.nome.clone(),
                descricao: p.descricao.clone(),
            })
            .collect(),
        transcript: transcript_guardado(
            &Guarda::nova(dados),
            kb.nome(),
            estado.id,
            estado.status,
            tempo::hoje(),
            prazo_de_retencao(dados),
        ),
    })
}

/// A ata da reunião, com os participantes e as sugestões do catálogo.
pub fn ata_da_reuniao(
    dados: &DiretorioDados,
    projeto: &str,
    reuniao: &str,
) -> Result<AtaVista, String> {
    let amb = ambiente(dados, projeto)?;
    let id = id_da_reuniao(reuniao)?;
    let (estado, _) = EstadoAta::ler(&amb.kb, id).map_err(|e| e.to_string())?;
    vista_da_ata(dados, &amb, &estado)
}

// ---------------------------------------------------------------------------
// Confirmação dos participantes

/// Participante confirmado pelo analista.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmacaoParticipante {
    /// Nome do participante na ata (como veio do transcript).
    pub nome: String,
    /// Papel confirmado (padrão ou texto livre); vazio deixa o papel a definir.
    #[serde(default)]
    pub papel: Option<String>,
    /// Pessoa do catálogo escolhida (slug), quando o nome casa com mais de uma.
    #[serde(default)]
    pub pessoa: Option<String>,
    /// Outros nomes da reunião que são a mesma pessoa: viram apelidos dela (e, se
    /// já estavam no catálogo como outra pessoa, as duas são juntadas) e saem da
    /// lista de participantes.
    #[serde(default)]
    pub mesmos: Vec<String>,
}

/// Recusa dentro da alteração da ata.
struct Recusa(String);

impl From<ErroAta> for Recusa {
    fn from(e: ErroAta) -> Self {
        Self(e.to_string())
    }
}

fn erro_kb(e: cba_kb::Erro) -> String {
    e.to_string()
}

/// Pessoa do catálogo que fica com os nomes da confirmação: a escolhida, a que
/// casa com o nome ou com um dos outros nomes, ou uma registrada agora. As outras
/// pessoas que casam com esses nomes são juntadas a ela, e os nomes que não casam
/// com ninguém viram apelidos.
fn resolver_pessoa(kb: &cba_kb::Projeto, c: &ConfirmacaoParticipante) -> Result<String, String> {
    let nomes: Vec<&str> = std::iter::once(c.nome.as_str())
        .chain(c.mesmos.iter().map(String::as_str))
        .collect();
    let mut manter = sem_vazio(&c.pessoa);
    if let Some(s) = &manter {
        kb.pessoa(s).map_err(erro_kb)?;
    } else {
        let catalogo = kb.catalogo_de_pessoas().map_err(erro_kb)?;
        for n in &nomes {
            match catalogo.casar(n) {
                Casamento::Nenhuma => {}
                Casamento::Unica(p) => {
                    manter = Some(p.slug.clone());
                    break;
                }
                Casamento::Ambigua(v) => return Err(ambiguo(n, &v)),
            }
        }
    }
    let manter = match manter {
        Some(s) => s,
        None => {
            kb.registrar_pessoa(&NovaPessoa {
                nome: c.nome.trim().to_owned(),
                apelidos: c.mesmos.clone(),
                papel: None,
            })
            .map_err(erro_kb)?
            .slug
        }
    };
    for n in &nomes {
        // Relido a cada nome: a junção anterior muda o catálogo.
        match kb.catalogo_de_pessoas().map_err(erro_kb)?.casar(n) {
            Casamento::Nenhuma => {
                kb.acrescentar_apelido(&manter, n).map_err(erro_kb)?;
            }
            Casamento::Unica(p) if p.slug != manter => {
                kb.juntar_pessoas(&manter, &p.slug).map_err(erro_kb)?;
                tracing::info!(pessoa = manter, juntada = p.slug, "pessoas juntadas");
            }
            Casamento::Unica(_) => {}
            Casamento::Ambigua(v) if v.iter().any(|p| p.slug == manter) => {}
            Casamento::Ambigua(v) => return Err(ambiguo(n, &v)),
        }
    }
    Ok(manter)
}

fn ambiguo(nome: &str, pessoas: &[cba_kb::PessoaLida]) -> String {
    let slugs: Vec<&str> = pessoas.iter().map(|p| p.slug.as_str()).collect();
    format!(
        "`{nome}` casa com mais de uma pessoa do catálogo ({}); escolha qual é",
        slugs.join(", ")
    )
}

/// Confere a lista: nomes preenchidos, sem repetir e sem um nome em dois lugares.
fn conferir_confirmacoes(confirmacoes: &[ConfirmacaoParticipante]) -> Result<(), String> {
    let mut vistos = HashSet::new();
    for c in confirmacoes {
        for n in std::iter::once(&c.nome).chain(&c.mesmos) {
            let chave = normalizar_nome(n);
            if chave.is_empty() {
                return Err(format!("nome de participante inválido: `{}`", n.trim()));
            }
            if !vistos.insert(chave) {
                return Err(format!(
                    "`{}` aparece mais de uma vez na confirmação",
                    n.trim()
                ));
            }
        }
    }
    Ok(())
}

/// Confirma os participantes: atualiza o catálogo de pessoas (registro, apelidos,
/// junções e o papel confirmado na reunião `D-NNNN/R-NNNN`) e os participantes da
/// ata. Notifica a mudança da ata no diário de eventos.
pub fn confirmar_participantes(
    dados: &DiretorioDados,
    projeto: &str,
    reuniao: &str,
    confirmacoes: &[ConfirmacaoParticipante],
) -> Result<AtaVista, String> {
    conferir_confirmacoes(confirmacoes)?;
    let amb = ambiente(dados, projeto)?;
    let kb = &amb.kb;
    let id = id_da_reuniao(reuniao)?;
    let (atual, _) = EstadoAta::ler(kb, id).map_err(|e| e.to_string())?;
    if atual.status == StatusAta::Aprovada {
        return Err(format!(
            "a ata da reunião {id} já foi aprovada; os participantes não mudam mais"
        ));
    }
    let referencia = format!("{}/{}", atual.demanda, atual.id);
    let (papeis, _) = Papeis::da_base(&amb.base);
    let hoje = tempo::hoje();

    let mut resolvidas = Vec::with_capacity(confirmacoes.len());
    for c in confirmacoes {
        let slug = resolver_pessoa(kb, c)?;
        let papel = sem_vazio(&c.papel).map(|p| papeis.canonico(&p));
        if let Some(p) = &papel {
            kb.registrar_papel(&slug, &referencia, p, hoje)
                .map_err(erro_kb)?;
        }
        resolvidas.push((c, slug, papel));
    }

    let removidos: HashSet<String> = confirmacoes
        .iter()
        .flat_map(|c| c.mesmos.iter().map(|n| normalizar_nome(n)))
        .collect();
    let templates = Templates::da_base(&amb.base).0;
    let (_, estado, _) = templates
        .alterar_ata(kb, id, |estado: &mut EstadoAta| {
            if estado.status == StatusAta::Aprovada {
                return Err(Recusa(format!("a ata da reunião {id} já foi aprovada")));
            }
            estado
                .participantes
                .retain(|p| !removidos.contains(&normalizar_nome(&p.nome)));
            for (c, slug, papel) in &resolvidas {
                let chave = normalizar_nome(&c.nome);
                match estado
                    .participantes
                    .iter_mut()
                    .find(|p| normalizar_nome(&p.nome) == chave)
                {
                    Some(p) => {
                        p.papel = papel.clone();
                        p.pessoa = Some(slug.clone());
                    }
                    None => {
                        let mut p = Participante::novo(c.nome.trim());
                        p.papel = papel.clone();
                        p.pessoa = Some(slug.clone());
                        estado.participantes.push(p);
                    }
                }
            }
            Ok(())
        })
        .map_err(|Recusa(e)| e)?;
    notificar_ata(dados, kb.nome(), id);
    tracing::info!(
        projeto,
        reuniao = %id,
        participantes = resolvidas.len(),
        juntados = removidos.len(),
        "participantes confirmados"
    );
    vista_da_ata(dados, &amb, &estado)
}

/// Avisa as telas (a ata ao vivo) que a ata mudou. Só identificadores.
fn notificar_ata(dados: &DiretorioDados, projeto: &str, reuniao: IdReuniao) {
    let evento = Evento {
        quando_ms: tempo::agora_ms(),
        projeto: projeto.to_owned(),
        analise: reuniao.to_string(),
        mudanca: Mudanca::Ata,
        versao: 0,
        processo: std::process::id(),
    };
    if let Err(e) = DiarioEventos::em(dados.eventos()).notificar(&evento) {
        tracing::warn!(reuniao = %reuniao, erro = %e.kind(), "diário de eventos: falha ao notificar");
    }
}

// ---------------------------------------------------------------------------
// Aprovação da ata

fn id_da_analise(analise: &str) -> Result<Id, String> {
    Id::analisar(analise.trim()).map_err(|e| e.to_string())
}

/// Se a ata da análise `analise` (fluxo `reuniao`) pode ser aprovada.
pub fn prontidao_ata(
    dados: &DiretorioDados,
    projeto: &str,
    analise: &str,
) -> Result<ProntidaoAta, String> {
    let amb = ambiente(dados, projeto)?;
    aprovacao_ata::prontidao_ata(&amb, &id_da_analise(analise)?).map_err(|e| e.to_string())
}

/// Aprova a ata e apaga o transcript. Sem `confirmacao` (o segundo aviso aceito),
/// recusa sem mudar nada: a ata continua em rascunho e o transcript, guardado.
pub fn aprovar_ata(
    dados: &DiretorioDados,
    projeto: &str,
    analise: &str,
    confirmacao: bool,
) -> Result<AtaAprovada, String> {
    if !confirmacao {
        return Err(aprovacao_ata::ErroAprovacaoAta::SemConfirmacao.to_string());
    }
    let amb = ambiente(dados, projeto)?;
    aprovacao_ata::aprovar_ata(&amb, &id_da_analise(analise)?, true, tempo::hoje())
        .map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Próximos passos

/// O que fazer com um próximo passo da ata aprovada.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "tipo", rename_all = "camelCase")]
pub enum AcaoProximoPasso {
    /// Análise de impacto da necessidade (`N-3`) a que o próximo passo se refere
    /// ou, sem ela, do próprio próximo passo.
    Impacto {
        #[serde(default)]
        necessidade: Option<String>,
    },
    /// Pergunta ao sistema, escrita (ou ajustada) pelo analista.
    Perguntar { pergunta: String },
}

fn id_do_item(texto: &str) -> Result<IdItemAta, String> {
    IdItemAta::analisar(texto.trim()).ok_or_else(|| format!("item da ata inválido: `{texto}`"))
}

/// Linha de um item da ata para o contexto da análise.
fn linha_do_item(i: &ItemAta) -> String {
    let tipo = match i.tipo {
        TipoItem::Decisao => "decisão",
        TipoItem::Necessidade => "necessidade",
        TipoItem::Regra => "regra citada",
        TipoItem::Pendencia => "pendência",
        TipoItem::Duvida => "dúvida",
        TipoItem::ProximoPasso => "próximo passo",
    };
    let mut linha = format!("- {} ({tipo}): {}", i.id, i.texto.trim());
    if let Some(r) = &i.responsavel {
        linha.push_str(&format!(" Responsável: {r}."));
    }
    if let Some(p) = &i.prazo {
        linha.push_str(&format!(" Prazo: {p}."));
    }
    linha
}

/// Entrada da análise aberta por um próximo passo: o pedido e, como contexto, a
/// ata aprovada (o caminho na base), o próximo passo e os itens a que ele se
/// refere. Sem falas: só o texto dos itens, que o analista aprovou.
fn entrada_do_proximo_passo(
    pedido: &str,
    estado: &EstadoAta,
    passo: &ItemAta,
    ata: &str,
) -> String {
    let mut itens = vec![linha_do_item(passo)];
    itens.extend(
        passo
            .referencias
            .iter()
            .filter_map(|r| estado.item(*r))
            .map(linha_do_item),
    );
    format!(
        "{pedido}\n\nContexto: próximo passo {} da ata aprovada da reunião {} ({}) da \
         demanda {}. Ata: `{ata}`.\n{}",
        passo.id,
        estado.id,
        estado.data,
        estado.demanda,
        itens.join("\n")
    )
}

/// Abre a análise de um próximo passo (`PP-n`) da ata aprovada da reunião: de
/// impacto (da necessidade a que ele se refere, ou do próprio passo) ou uma
/// pergunta ao sistema. A análise nasce ligada à demanda da reunião (`demanda.id`),
/// com a ata como contexto na entrada; o `iniciar_analise` traz ainda as atas da
/// demanda no contexto. A demanda precisa estar aberta.
pub fn analisar_proximo_passo(
    dados: &DiretorioDados,
    projeto: &str,
    reuniao: &str,
    item: &str,
    acao: &AcaoProximoPasso,
) -> Result<AnaliseCriada, String> {
    let amb = ambiente(dados, projeto)?;
    let kb = &amb.kb;
    let r = id_da_reuniao(reuniao)?;
    let (estado, arquivos) = EstadoAta::ler(kb, r).map_err(|e| e.to_string())?;
    if estado.status != StatusAta::Aprovada {
        return Err(format!(
            "aprove a ata da reunião {r} antes de abrir análises dos próximos passos"
        ));
    }
    let id = id_do_item(item)?;
    let passo = estado
        .item(id)
        .filter(|i| i.tipo == TipoItem::ProximoPasso)
        .ok_or_else(|| format!("a ata da reunião {r} não tem o próximo passo {id}"))?;
    let ata = kb.na_base(&arquivos.ata).as_str().to_owned();
    let (fluxo, pedido, titulo) = match acao {
        AcaoProximoPasso::Impacto { necessidade } => {
            match necessidade
                .as_deref()
                .map(str::trim)
                .filter(|n| !n.is_empty())
            {
                Some(n) => {
                    let n = id_do_item(n)?;
                    let necessidade = estado
                        .item(n)
                        .filter(|i| i.tipo == TipoItem::Necessidade)
                        .ok_or_else(|| format!("a ata da reunião {r} não tem a necessidade {n}"))?;
                    (
                        Fluxo::Impacto,
                        format!(
                            "Analisar o impacto da necessidade {n}: {}",
                            necessidade.texto.trim()
                        ),
                        format!("Impacto da necessidade {n} ({r}) · {}", necessidade.texto),
                    )
                }
                None => (
                    Fluxo::Impacto,
                    format!("Analisar o impacto de {id}: {}", passo.texto.trim()),
                    format!("Impacto de {id} ({r}) · {}", passo.texto),
                ),
            }
        }
        AcaoProximoPasso::Perguntar { pergunta } => {
            let pergunta = pergunta.trim();
            if pergunta.is_empty() {
                return Err("escreva a pergunta ao sistema".into());
            }
            (Fluxo::Perguntar, pergunta.to_owned(), pergunta.to_owned())
        }
    };
    let entrada = entrada_do_proximo_passo(&pedido, &estado, passo, &ata);
    let criada = criar_analise_ligada(
        dados,
        projeto,
        &estado.demanda.to_string(),
        fluxo,
        &entrada,
        Some(&titulo_da_entrada(&titulo)),
    )?;
    tracing::info!(
        projeto,
        reuniao = %r,
        item = %id,
        analise = criada.analise.id,
        "análise aberta pelo próximo passo"
    );
    Ok(criada)
}

// ---------------------------------------------------------------------------
// Comandos

fn dados() -> Result<DiretorioDados, String> {
    DiretorioDados::padrao().map_err(|e| format!("diretório de dados do app inacessível: {e}"))
}

async fn bloqueante<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| format!("falha interna: {e}"))?
}

#[tauri::command]
pub async fn previa_transcript_app(entrada: EntradaTranscript) -> Result<PreviaTranscript, String> {
    bloqueante(move || previa_transcript(&entrada)).await
}

#[tauri::command]
pub async fn criar_reuniao_app(
    projeto: String,
    demanda: DemandaDaReuniao,
    entrada: EntradaTranscript,
    aviso_aceito: bool,
) -> Result<ReuniaoCriada, String> {
    let dados = dados()?;
    bloqueante(move || criar_reuniao(&dados, &projeto, &demanda, &entrada, aviso_aceito)).await
}

#[tauri::command]
pub async fn ata_reuniao_app(projeto: String, reuniao: String) -> Result<AtaVista, String> {
    let dados = dados()?;
    bloqueante(move || ata_da_reuniao(&dados, &projeto, &reuniao)).await
}

#[tauri::command]
pub async fn confirmar_participantes_app(
    projeto: String,
    reuniao: String,
    confirmacoes: Vec<ConfirmacaoParticipante>,
) -> Result<AtaVista, String> {
    let dados = dados()?;
    bloqueante(move || confirmar_participantes(&dados, &projeto, &reuniao, &confirmacoes)).await
}

#[tauri::command]
pub async fn prontidao_ata_app(projeto: String, analise: String) -> Result<ProntidaoAta, String> {
    let dados = dados()?;
    bloqueante(move || prontidao_ata(&dados, &projeto, &analise)).await
}

#[tauri::command]
pub async fn aprovar_ata_app(
    projeto: String,
    analise: String,
    confirmacao: bool,
) -> Result<AtaAprovada, String> {
    let dados = dados()?;
    bloqueante(move || aprovar_ata(&dados, &projeto, &analise, confirmacao)).await
}

#[tauri::command]
pub async fn analisar_proximo_passo_app(
    projeto: String,
    reuniao: String,
    item: String,
    acao: AcaoProximoPasso,
) -> Result<AnaliseCriada, String> {
    let dados = dados()?;
    bloqueante(move || analisar_proximo_passo(&dados, &projeto, &reuniao, &item, &acao)).await
}

#[cfg(test)]
#[path = "reunioes_testes.rs"]
mod tests;
