//! Ferramentas do fluxo "Analisar reunião" (épico #93, #139): `ler_transcript`,
//! `propor_papel`, `registrar_item_ata` e `sugerir_demanda`.
//!
//! - `ler_transcript` lê as falas da guarda temporária (`cba_reunioes::guarda`),
//!   paginadas e com filtro por falante e por horário. A página é cortada por
//!   quantidade e por tamanho ([`LIMITE_DE_TEXTO_DO_TRANSCRIPT`]), para a resposta
//!   caber no limite de resposta do MCP mesmo numa reunião de 2 h.
//! - `propor_papel` grava o papel que o agente propõe para um participante, com as
//!   falas conferidas, no estado da ata ([`cba_docs::PropostaDePapel`]), e devolve
//!   junto o papel que o catálogo de pessoas sugere (sem IA). O catálogo não muda:
//!   quem confirma é o analista, na tela de participantes.
//! - `registrar_item_ata` registra um item da ata com as falas de origem, cada uma
//!   conferida contra o transcript (`cba_reunioes::citacao`), e as evidências de
//!   código opcionais, conferidas como as das análises. Grava o estado e monta a
//!   ata de novo ([`cba_docs::Templates::alterar_ata`]), para a ata ao vivo.
//! - `sugerir_demanda` procura demandas abertas parecidas com um texto, pela busca
//!   no índice da base e pelo título.
//!
//! # Privacidade
//!
//! O transcript só é lido aqui; nada dele vai para o log nem para o diário de
//! eventos, que recebem só identificadores (projeto, reunião, item, índice da
//! fala) e contagens. As recusas de citação dizem o motivo sem o texto da fala
//! (`cba_reunioes::RecusaCitacao`).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write as _;

use cba_core::ambiente::Ambiente;
use cba_core::eventos::{DiarioEventos, Evento, Mudanca, Notificador};
use cba_docs::{
    CitacaoFala, EstadoAta, IdItemAta, NovoItem, Participante, PropostaDePapel, Templates, TipoItem,
};
use cba_kb::frontmatter::{
    MAXIMO_DE_PALAVRAS_DO_NOME, MAXIMO_DE_PALAVRAS_DO_PAPEL, MAXIMO_DO_NOME, MAXIMO_DO_PAPEL,
    StatusAta, conferir_texto_curto,
};
use cba_kb::indice::{Consulta, Modo, Tipo};
use cba_kb::{
    ArquivosDaReuniao, Casamento, IdDemanda, IdReuniao, Papeis, TipoDocumento, normalizar_nome,
    normalizar_texto,
};
use cba_reunioes::{
    ErroGuarda, Fala, FiltroFalas, Guarda, PedidoCitacao, RecusaCitacao, conferir_citacao,
    filtrar_falas,
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::documento;
use crate::evidencias::{self, EvidenciaCitada};
use crate::fontes;
use crate::registro::{ErroFerramenta, Ferramenta, Saida, ler_argumentos};

/// Caracteres de texto de falas, no máximo, numa página de `ler_transcript`. A
/// resposta leva as falas no texto e nos dados estruturados; com o cabeçalho,
/// fica abaixo de ~30 mil caracteres (perto de 8 mil tokens), longe do limite
/// padrão de resposta de uma ferramenta MCP no Claude Code (25 mil tokens).
pub const LIMITE_DE_TEXTO_DO_TRANSCRIPT: usize = 10_000;

/// Falas por página quando o agente não informa `limite`.
const FALAS_POR_PAGINA: usize = cba_reunioes::guarda::LIMITE_PADRAO;

/// Maior `limite` de falas por página.
const MAXIMO_DE_FALAS: usize = cba_reunioes::guarda::LIMITE_MAXIMO;

/// Citações no máximo por item ou por proposta de papel.
const LIMITE_DE_CITACOES: u64 = 10;

/// Evidências de código no máximo por item.
const LIMITE_DE_EVIDENCIAS: u64 = 20;

/// Demandas sugeridas quando o agente não informa `limite`.
const DEMANDAS_POR_PADRAO: usize = 5;

// ---------------------------------------------------------------------------
// Peças comuns

/// Schema do argumento `reuniao`.
pub(crate) fn schema_da_reuniao() -> Value {
    json!({
        "type": "string",
        "minLength": 1,
        "description": "Identificador da reunião (R-NNNN), o mesmo do pedido do analista ou da resposta de iniciar_analise."
    })
}

/// Identificador de reunião informado pelo agente.
pub(crate) fn id_da_reuniao(texto: &str) -> Result<IdReuniao, ErroFerramenta> {
    IdReuniao::analisar(texto.trim()).map_err(|_| {
        ErroFerramenta::Argumentos(format!(
            "`reuniao`: `{texto}` não é o identificador de uma reunião; use a forma \
             R-NNNN, como R-0001"
        ))
    })
}

/// Identificador de demanda informado pelo agente.
pub(crate) fn id_da_demanda(texto: &str) -> Result<IdDemanda, ErroFerramenta> {
    IdDemanda::analisar(texto.trim()).map_err(|_| {
        ErroFerramenta::Argumentos(format!(
            "`demanda`: `{texto}` não é o identificador de uma demanda; use a forma \
             D-NNNN, como D-0001"
        ))
    })
}

/// Arquivos da reunião na base, ou a recusa dizendo que ela não existe.
pub(crate) fn arquivos_da_reuniao(
    ambiente: &Ambiente,
    id: IdReuniao,
) -> Result<ArquivosDaReuniao, ErroFerramenta> {
    ambiente.kb.reuniao(id).map_err(|e| match e {
        cba_kb::Erro::ReuniaoInexistente { .. } => ErroFerramenta::Recusa(format!(
            "a reunião {id} não existe neste projeto; confira o identificador (R-NNNN) \
             informado pelo analista"
        )),
        outro => ErroFerramenta::Interno(outro.to_string()),
    })
}

fn guarda(ambiente: &Ambiente) -> Guarda {
    Guarda::nova(&ambiente.dados)
}

/// Erro da guarda como erro da ferramenta. A falta do transcript é a recusa mais
/// comum: a ata foi aprovada, a reunião foi descartada ou o prazo venceu.
fn erro_da_guarda(id: IdReuniao, erro: ErroGuarda) -> ErroFerramenta {
    match erro {
        ErroGuarda::NaoGuardado { .. } => ErroFerramenta::Recusa(format!(
            "o transcript da reunião {id} não está mais guardado: ele é apagado quando a \
             ata é aprovada, quando a reunião é descartada ou quando vence o prazo de \
             retenção (ou ainda não foi enviado pelo analista). Sem ele não há falas para \
             ler nem citações para conferir; a ata continua na base. Avise o analista"
        )),
        ErroGuarda::Recusada(r) => ErroFerramenta::Recusa(r.to_string()),
        outro => {
            tracing::warn!(reuniao = %id, "transcript ilegível");
            ErroFerramenta::Interno(outro.to_string())
        }
    }
}

/// Falas guardadas da reunião (que precisa existir na base).
fn falas(ambiente: &Ambiente, id: IdReuniao) -> Result<Vec<Fala>, ErroFerramenta> {
    guarda(ambiente)
        .falas(ambiente.kb.nome(), id)
        .map_err(|e| erro_da_guarda(id, e))
}

/// Avisa o app de que a ata mudou (a ata ao vivo relê o estado). Só
/// identificadores.
fn notificar_ata(ambiente: &Ambiente, id: IdReuniao) {
    let evento = Evento {
        quando_ms: cba_core::tempo::agora_ms(),
        projeto: ambiente.kb.nome().to_owned(),
        analise: id.to_string(),
        mudanca: Mudanca::Ata,
        versao: 0,
        processo: std::process::id(),
    };
    if let Err(e) = DiarioEventos::em(ambiente.dados.eventos()).notificar(&evento) {
        tracing::warn!(reuniao = %id, erro = %e.kind(), "diário de eventos: falha ao notificar");
    }
}

/// Recusa uma ata aprovada (não aceita registros novos).
fn exigir_rascunho(estado: &EstadoAta) -> Result<(), ErroFerramenta> {
    if estado.status == StatusAta::Aprovada {
        return Err(ErroFerramenta::Recusa(format!(
            "a ata da reunião {} já foi aprovada pelo analista e não aceita registros novos",
            estado.id
        )));
    }
    Ok(())
}

/// Schema de uma citação de fala.
fn schema_da_citacao() -> Value {
    json!({
        "type": "object",
        "properties": {
            "indice": {
                "type": "integer",
                "minimum": 1,
                "description": "Índice da fala, como ler_transcript devolve. É o jeito preferido de apontar a fala."
            },
            "falante": {
                "type": "string",
                "minLength": 1,
                "maxLength": 200,
                "description": "Falante, como aparece no transcript. Sem `indice`, obrigatório junto com `inicio`."
            },
            "inicio": {
                "type": "string",
                "minLength": 1,
                "maxLength": 12,
                "description": "Horário de início da fala (HH:MM:SS). Sem `indice`, obrigatório junto com `falante`."
            },
            "trecho": {
                "type": "string",
                "minLength": 1,
                "maxLength": 2000,
                "description": "Trecho curto copiado da fala (até 240 caracteres), com as palavras como estão nela."
            }
        },
        "required": ["trecho"],
        "additionalProperties": false
    })
}

/// Confere todas as citações contra as falas. Se alguma não confere, recusa
/// com o motivo de cada uma, sem o texto da fala.
fn conferir_todas(
    id: IdReuniao,
    falas: &[Fala],
    pedidos: &[PedidoCitacao],
) -> Result<Vec<(usize, CitacaoFala)>, ErroFerramenta> {
    let mut aceitas = Vec::new();
    let mut recusas = Vec::new();
    for (i, pedido) in pedidos.iter().enumerate() {
        match conferir_citacao(falas, pedido) {
            Ok(c) => aceitas.push((c.indice, c.citacao)),
            Err(e) => {
                tracing::debug!(
                    reuniao = %id,
                    citacao = i + 1,
                    fala = ?pedido.indice,
                    motivo = motivo_da_recusa(&e),
                    "citação recusada"
                );
                recusas.push(format!("citação {}: {e}", i + 1));
            }
        }
    }
    if !recusas.is_empty() {
        return Err(ErroFerramenta::Recusa(format!(
            "nada foi registrado: {} de {} citações não conferem com o transcript da \
             reunião {id}. {}. Confira as falas com ler_transcript, corrija e chame de novo",
            recusas.len(),
            pedidos.len(),
            recusas.join("; ")
        )));
    }
    Ok(aceitas)
}

/// Nome curto do motivo, para o log (sem conteúdo).
fn motivo_da_recusa(e: &RecusaCitacao) -> &'static str {
    match e {
        RecusaCitacao::TrechoVazio => "trecho_vazio",
        RecusaCitacao::TrechoLongo { .. } => "trecho_longo",
        RecusaCitacao::PedidoIncompleto => "pedido_incompleto",
        RecusaCitacao::FalaInexistente { .. } => "fala_inexistente",
        RecusaCitacao::FalanteDiferente { .. } => "falante_diferente",
        RecusaCitacao::HorarioDiferente { .. } => "horario_diferente",
        RecusaCitacao::FalaNaoEncontrada { .. } => "fala_nao_encontrada",
        RecusaCitacao::NaoConfere { .. } => "nao_confere",
    }
}

fn citacoes_json(citacoes: &[(usize, CitacaoFala)]) -> Vec<Value> {
    citacoes
        .iter()
        .map(|(indice, c)| {
            json!({
                "indice": indice,
                "falante": c.falante,
                "inicio": c.inicio,
                "status": "conferida",
            })
        })
        .collect()
}

/// Falantes distintos do transcript, na ordem da primeira fala, com a contagem.
fn falantes(falas: &[Fala]) -> Vec<(String, usize)> {
    let mut ordem: Vec<(String, usize)> = Vec::new();
    for f in falas {
        let Some(nome) = f
            .falante
            .as_deref()
            .map(str::trim)
            .filter(|n| !n.is_empty())
        else {
            continue;
        };
        match ordem.iter_mut().find(|(n, _)| n == nome) {
            Some((_, n)) => *n += 1,
            None => ordem.push((nome.to_owned(), 1)),
        }
    }
    ordem
}

/// Texto em uma linha, cortado em `maximo` caracteres.
pub(crate) fn cortar(texto: &str, maximo: usize) -> (String, bool) {
    let linha = texto.split_whitespace().collect::<Vec<_>>().join(" ");
    if linha.chars().count() <= maximo {
        return (linha, false);
    }
    let corte: String = linha.chars().take(maximo.saturating_sub(1)).collect();
    (format!("{}…", corte.trim_end()), true)
}

/// Horário do transcript como `HH:MM:SS` (o formato das citações).
fn horario(h: cba_reunioes::Horario) -> String {
    let s = h.segundos();
    format!("{:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
}

fn horario_do_argumento(
    campo: &str,
    texto: Option<&str>,
) -> Result<Option<cba_reunioes::Horario>, ErroFerramenta> {
    let Some(texto) = texto else {
        return Ok(None);
    };
    let h = cba_kb::frontmatter::Horario::analisar(texto).ok_or_else(|| {
        ErroFerramenta::Argumentos(format!(
            "`{campo}`: horário inválido; use HH:MM:SS (como 00:12:34) ou MM:SS"
        ))
    })?;
    Ok(Some(cba_reunioes::Horario::de_milissegundos(
        u64::from(h.segundos()) * 1000,
    )))
}

// ---------------------------------------------------------------------------
// ler_transcript

#[derive(Debug, Deserialize)]
struct ArgsLer {
    reuniao: String,
    #[serde(default)]
    falante: Option<String>,
    #[serde(default)]
    desde: Option<String>,
    #[serde(default)]
    ate: Option<String>,
    #[serde(default)]
    cursor: Option<usize>,
    #[serde(default)]
    limite: Option<usize>,
}

/// `ler_transcript`: falas da reunião, paginadas, com filtro por falante e horário.
pub fn ler_transcript() -> Ferramenta {
    Ferramenta::nova(
        "ler_transcript",
        "Ler o transcript da reunião",
        "Devolve as falas do transcript de uma reunião (R-NNNN), em ordem, com o índice \
         de cada fala, o falante, o horário de início e o texto. Paginada: a página é \
         cortada por quantidade e por tamanho; para continuar, chame de novo com o \
         `cursor` da resposta até não haver próximo. Filtre por `falante` (parte do nome \
         basta) e pelo intervalo de horário (`desde` e `ate`, HH:MM:SS) para voltar a um \
         trecho. Para citar uma fala na ata, use o índice e copie um trecho curto dela. \
         Se o transcript já foi apagado (ata aprovada, reunião descartada ou prazo \
         vencido), a chamada é recusada com o motivo.",
        json!({
            "type": "object",
            "properties": {
                "reuniao": schema_da_reuniao(),
                "falante": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 200,
                    "description": "Só as falas deste falante (sem diferença de acentos nem de maiúsculas; parte do nome basta)."
                },
                "desde": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 12,
                    "description": "Só as falas que começam neste horário ou depois (HH:MM:SS)."
                },
                "ate": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 12,
                    "description": "Só as falas que começam até este horário (HH:MM:SS)."
                },
                "cursor": {
                    "type": "integer",
                    "minimum": 0,
                    "description": "Posição da primeira fala da página entre as filtradas (começa em 0). Para a próxima página, use o `proximo_cursor` da resposta anterior."
                },
                "limite": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": MAXIMO_DE_FALAS,
                    "description": format!("Falas por página (padrão {FALAS_POR_PAGINA}). A página também é cortada por tamanho.")
                }
            },
            "required": ["reuniao"],
            "additionalProperties": false
        }),
        |ambiente, args| {
            let args: ArgsLer = ler_argumentos(args)?;
            let id = id_da_reuniao(&args.reuniao)?;
            let filtro = FiltroFalas {
                falante: args.falante.filter(|f| !f.trim().is_empty()),
                desde: horario_do_argumento("desde", args.desde.as_deref())?,
                ate: horario_do_argumento("ate", args.ate.as_deref())?,
                pular: args.cursor.unwrap_or(0),
                limite: Some(args.limite.unwrap_or(FALAS_POR_PAGINA)),
            };
            arquivos_da_reuniao(ambiente, id)?;
            let todas = falas(ambiente, id)?;
            Ok(ler(id, &todas, &filtro))
        },
    )
    .somente_leitura()
}

fn ler(id: IdReuniao, todas: &[Fala], filtro: &FiltroFalas) -> Saida {
    let total_do_transcript = todas.len();
    let lista_de_falantes = falantes(todas);
    let pagina = filtrar_falas(todas.to_vec(), filtro);
    let cursor = filtro.pular;

    // Corta por tamanho, com ao menos uma fala.
    let mut linhas = Vec::new();
    let mut dados = Vec::new();
    let mut tamanho = 0;
    for f in &pagina.falas {
        let restante = LIMITE_DE_TEXTO_DO_TRANSCRIPT.saturating_sub(tamanho);
        let (texto, cortada) = cortar(&f.texto, LIMITE_DE_TEXTO_DO_TRANSCRIPT);
        let n = texto.chars().count();
        if !linhas.is_empty() && n > restante {
            break;
        }
        tamanho += n;
        let falante = f
            .falante
            .as_deref()
            .unwrap_or(cba_reunioes::FALANTE_DESCONHECIDO);
        let inicio = f.inicio.map(horario);
        linhas.push(format!(
            "[{}] {} {}: {}",
            f.indice,
            inicio.as_deref().unwrap_or("--:--:--"),
            falante,
            texto
        ));
        let mut v = json!({
            "indice": f.indice,
            "falante": f.falante,
            "inicio": inicio,
            "fim": f.fim.map(horario),
            "texto": texto,
        });
        if cortada {
            v["cortada"] = json!(true);
        }
        dados.push(v);
    }
    let quantidade = dados.len();
    let fim = cursor + quantidade;
    let proximo = (quantidade > 0 && fim < pagina.total).then_some(fim);

    let mut t = String::new();
    let _ = write!(
        t,
        "Reunião {id}: transcript com {total_do_transcript} falas"
    );
    let mut filtros = Vec::new();
    if let Some(f) = &filtro.falante {
        filtros.push(format!("falante \"{}\"", cortar(f, 60).0));
    }
    if let Some(d) = filtro.desde {
        filtros.push(format!("desde {}", horario(d)));
    }
    if let Some(a) = filtro.ate {
        filtros.push(format!("até {}", horario(a)));
    }
    if !filtros.is_empty() {
        let _ = write!(
            t,
            "; filtro: {} ({} falas)",
            filtros.join(", "),
            pagina.total
        );
    }
    let _ = writeln!(t, ".");
    if cursor == 0 && !lista_de_falantes.is_empty() {
        let nomes: Vec<String> = lista_de_falantes
            .iter()
            .map(|(n, q)| format!("{n} ({q})"))
            .collect();
        let _ = writeln!(t, "Falantes: {}.", nomes.join(", "));
    }
    let _ = writeln!(t);
    for l in &linhas {
        let _ = writeln!(t, "{l}");
    }
    let _ = writeln!(t);
    if quantidade == 0 {
        if pagina.total > 0 {
            let _ = writeln!(
                t,
                "O cursor {cursor} passou do fim: há {} falas (cursor de 0 a {}).",
                pagina.total,
                pagina.total - 1
            );
        } else {
            let _ = writeln!(t, "Nenhuma fala com esse filtro.");
        }
    } else {
        let _ = write!(t, "Falas {} a {} de {}.", cursor + 1, fim, pagina.total);
        if let Some(p) = proximo {
            let _ = write!(t, " Para continuar, chame de novo com cursor: {p}.");
        }
        let _ = writeln!(t);
    }
    tracing::info!(
        reuniao = %id,
        falas = quantidade,
        cursor,
        total = pagina.total,
        "transcript lido"
    );
    Saida {
        texto: t,
        dados: Some(json!({
            "reuniao": id,
            "falas_no_transcript": total_do_transcript,
            "falantes": lista_de_falantes
                .iter()
                .map(|(n, q)| json!({ "nome": n, "falas": q }))
                .collect::<Vec<_>>(),
            "total": pagina.total,
            "cursor": cursor,
            "quantidade": quantidade,
            "proximo_cursor": proximo,
            "falas": dados,
        })),
    }
}

// ---------------------------------------------------------------------------
// propor_papel

#[derive(Debug, Deserialize)]
struct ArgsPapel {
    reuniao: String,
    participante: String,
    papel: String,
    #[serde(default)]
    justificativa: Option<String>,
    citacoes: Vec<PedidoCitacao>,
}

/// `propor_papel`: papel de um participante, com citações, à espera do analista.
pub fn propor_papel() -> Ferramenta {
    Ferramenta::nova(
        "propor_papel",
        "Propor o papel de um participante",
        "Propõe o papel de um participante da reunião (PO, gerente, usuário-chave, \
         analista de negócio, desenvolvedor, arquiteto, QA, cliente externo ou texto \
         curto), com as falas que mostram o papel. Cada citação é conferida contra o \
         transcript; a que não confere é recusada com o motivo. A resposta traz o papel \
         que o catálogo de pessoas do projeto sugere para o participante (pelas reuniões \
         anteriores), para você confirmar ou contestar com a fala. A proposta fica na \
         ata até o analista confirmar na tela de participantes: o catálogo não muda \
         aqui. Proponha de novo para trocar a proposta.",
        json!({
            "type": "object",
            "properties": {
                "reuniao": schema_da_reuniao(),
                "participante": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 200,
                    "description": "Nome do participante como aparece no transcript (o falante)."
                },
                "papel": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 40,
                    "description": "Papel nesta reunião, curto (até 5 palavras), de preferência um do catálogo de papéis."
                },
                "justificativa": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 300,
                    "description": "Por que este papel, em uma frase, sem copiar falas."
                },
                "citacoes": {
                    "type": "array",
                    "minItems": 1,
                    "maxItems": LIMITE_DE_CITACOES,
                    "items": schema_da_citacao(),
                    "description": "Falas que mostram o papel."
                }
            },
            "required": ["reuniao", "participante", "papel", "citacoes"],
            "additionalProperties": false
        }),
        |ambiente, args| {
            let args: ArgsPapel = ler_argumentos(args)?;
            let id = id_da_reuniao(&args.reuniao)?;
            let nome = args
                .participante
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            let papel = args.papel.split_whitespace().collect::<Vec<_>>().join(" ");
            conferir_texto_curto(
                "participante",
                &nome,
                MAXIMO_DO_NOME,
                MAXIMO_DE_PALAVRAS_DO_NOME,
            )
            .map_err(|e| ErroFerramenta::Argumentos(format!("`{}`: {}", e.campo, e.mensagem)))?;
            conferir_texto_curto(
                "papel",
                &papel,
                MAXIMO_DO_PAPEL,
                MAXIMO_DE_PALAVRAS_DO_PAPEL,
            )
            .map_err(|e| ErroFerramenta::Argumentos(format!("`{}`: {}", e.campo, e.mensagem)))?;
            let (papeis, _) = Papeis::da_base(&ambiente.base);
            let papel = papeis.canonico(&papel);
            let arquivos = arquivos_da_reuniao(ambiente, id)?;
            let todas = falas(ambiente, id)?;

            // O participante precisa ser um falante do transcript ou um participante
            // já listado na ata; vale a grafia de lá.
            let chave = normalizar_nome(&nome);
            let do_transcript = falantes(&todas)
                .into_iter()
                .map(|(n, _)| n)
                .find(|n| normalizar_nome(n) == chave);
            let (atual, _) = EstadoAta::ler(&ambiente.kb, id)?;
            exigir_rascunho(&atual)?;
            let na_ata = atual
                .participantes
                .iter()
                .find(|p| normalizar_nome(&p.nome) == chave)
                .map(|p| p.nome.clone());
            let Some(grafia) = na_ata.or(do_transcript) else {
                let nomes: Vec<String> = falantes(&todas).into_iter().map(|(n, _)| n).collect();
                return Err(ErroFerramenta::Recusa(format!(
                    "`{nome}` não é falante do transcript da reunião {id} nem participante \
                     da ata. Use o nome como aparece no transcript: {}",
                    if nomes.is_empty() {
                        "o transcript não identifica os falantes".to_owned()
                    } else {
                        nomes.join(", ")
                    }
                )));
            };

            let citacoes = conferir_todas(id, &todas, &args.citacoes)?;

            // Sugestão do catálogo, sem IA.
            let sugestao = ambiente
                .kb
                .sugerir_participantes(&[grafia.as_str()])
                .map_err(|e| ErroFerramenta::Interno(e.to_string()))?
                .remove(0);
            let pessoa = sugestao.casamento.pessoa().map(|p| p.slug.clone());
            let sugerido = sugestao.papel_sugerido.as_ref().map(|s| s.papel.clone());
            let catalogo = match &sugestao.casamento {
                Casamento::Nenhuma => json!({
                    "casamento": "nenhuma",
                    "pessoa": Value::Null,
                    "papel_sugerido": Value::Null,
                }),
                Casamento::Unica(p) => json!({
                    "casamento": "unica",
                    "pessoa": p.slug,
                    "nome": p.nome(),
                    "papel_sugerido": sugestao.papel_sugerido.as_ref().map(|s| s.papel.clone()),
                    "confirmado_em": sugestao.papel_sugerido.as_ref().and_then(|s| s.confirmado_em).map(|d| d.to_string()),
                    "reuniao": sugestao.papel_sugerido.as_ref().and_then(|s| s.reuniao.clone()),
                }),
                Casamento::Ambigua(ps) => json!({
                    "casamento": "ambigua",
                    "pessoa": Value::Null,
                    "candidatos": ps.iter().map(|p| p.slug.clone()).collect::<Vec<_>>(),
                    "papel_sugerido": Value::Null,
                }),
            };

            let proposta = PropostaDePapel {
                papel: papel.clone(),
                justificativa: args
                    .justificativa
                    .map(|j| cortar(&j, 300).0)
                    .filter(|j| !j.is_empty()),
                sugerido_pelo_catalogo: sugerido.clone(),
                citacoes: citacoes.iter().map(|(_, c)| c.clone()).collect(),
            };
            let templates = Templates::da_base(&ambiente.base).0;
            let (_, estado, avisos) =
                templates.alterar_ata(&ambiente.kb, id, |estado: &mut EstadoAta| {
                    exigir_rascunho(estado)?;
                    let chave = normalizar_nome(&grafia);
                    let p = match estado
                        .participantes
                        .iter_mut()
                        .position(|p| normalizar_nome(&p.nome) == chave)
                    {
                        Some(i) => &mut estado.participantes[i],
                        None => {
                            let mut novo = Participante::novo(grafia.clone());
                            novo.pessoa = pessoa.clone();
                            estado.participantes.push(novo);
                            estado.participantes.last_mut().expect("acabou de entrar")
                        }
                    };
                    p.proposta = Some(proposta);
                    Ok::<_, ErroFerramenta>(())
                })?;
            if !avisos.is_empty() {
                tracing::warn!(reuniao = %id, avisos = avisos.len(), "avisos na montagem da ata");
            }
            notificar_ata(ambiente, id);
            tracing::info!(
                reuniao = %id,
                citacoes = citacoes.len(),
                catalogo = sugestao_tipo(&sugestao.casamento),
                "papel proposto"
            );
            let ata = ambiente.kb.na_base(&arquivos.ata);
            let confirmado = estado
                .participantes
                .iter()
                .find(|p| normalizar_nome(&p.nome) == normalizar_nome(&grafia))
                .and_then(|p| p.papel.clone());

            let mut t = format!(
                "Papel proposto para {grafia} na reunião {id}: {papel}, com {} citação(ões) \
                 conferida(s). ",
                citacoes.len()
            );
            match (&sugestao.casamento, &sugerido) {
                (Casamento::Unica(p), Some(s)) => {
                    let _ = write!(
                        t,
                        "O catálogo de pessoas tem {grafia} como `{}`, com o papel sugerido \
                         \"{s}\"{}. ",
                        p.slug,
                        if normalizar_texto(s) == normalizar_texto(&papel) {
                            " (o mesmo)"
                        } else {
                            "; a sua proposta é diferente: deixe na justificativa o que mudou"
                        }
                    );
                }
                (Casamento::Unica(p), None) => {
                    let _ = write!(
                        t,
                        "O catálogo tem {grafia} como `{}`, ainda sem papel. ",
                        p.slug
                    );
                }
                (Casamento::Ambigua(ps), _) => {
                    let _ = write!(
                        t,
                        "O nome casa com {} pessoas do catálogo; o analista escolhe. ",
                        ps.len()
                    );
                }
                (Casamento::Nenhuma, _) => {
                    let _ = write!(t, "{grafia} ainda não está no catálogo de pessoas. ");
                }
            }
            t.push_str(
                "A proposta fica na ata até o analista confirmar na tela de participantes; o \
                 catálogo de pessoas não foi alterado.",
            );
            Ok(Saida {
                texto: t,
                dados: Some(json!({
                    "reuniao": id,
                    "participante": grafia,
                    "papel_proposto": papel,
                    "papel_confirmado": confirmado,
                    "catalogo": catalogo,
                    "citacoes": citacoes_json(&citacoes),
                    "ata": ata,
                })),
            })
        },
    )
}

fn sugestao_tipo(c: &Casamento) -> &'static str {
    match c {
        Casamento::Nenhuma => "nenhuma",
        Casamento::Unica(_) => "unica",
        Casamento::Ambigua(_) => "ambigua",
    }
}

// ---------------------------------------------------------------------------
// registrar_item_ata

#[derive(Debug, Deserialize)]
struct ArgsItem {
    reuniao: String,
    tipo: TipoItem,
    texto: String,
    #[serde(default)]
    responsavel: Option<String>,
    #[serde(default)]
    prazo: Option<String>,
    #[serde(default)]
    referencias: Vec<String>,
    citacoes: Vec<PedidoCitacao>,
    #[serde(default)]
    evidencias: Vec<EvidenciaCitada>,
}

/// `registrar_item_ata`: um item da ata com as falas de origem conferidas.
pub fn registrar_item_ata() -> Ferramenta {
    Ferramenta::nova(
        "registrar_item_ata",
        "Registrar item da ata",
        "Registra um item na ata da reunião: decisão, necessidade, regra de negócio \
         citada, pendência (a ação, com responsável e prazo como foram ditos), dúvida em \
         aberto ou próximo passo. Cada item cita ao menos uma fala de origem (o índice \
         da fala e um trecho curto copiado dela); cada citação é conferida contra o \
         transcript e a que não confere é recusada com o motivo, sem registrar nada. \
         Um item pode apontar para outros (`referencias`, como N-3 no próximo passo \
         \"Analisar o impacto da necessidade N-3\") e levar evidências do código (\"o \
         sistema já faz isso\", \"conflita com RN-FAT-007\"), conferidas como em \
         `afirmar`. Devolve o identificador do item (D-1, N-1, R-1, P-1, DV-1, PP-1). A \
         ata é montada de novo a cada registro.",
        json!({
            "type": "object",
            "properties": {
                "reuniao": schema_da_reuniao(),
                "tipo": {
                    "type": "string",
                    "enum": ["decisao", "necessidade", "regra", "pendencia", "duvida", "proximo_passo"],
                    "description": "decisao, necessidade (requisito levantado), regra (regra de negócio citada), pendencia (ação com responsável e prazo), duvida (em aberto) ou proximo_passo."
                },
                "texto": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 2000,
                    "description": "O item em linguagem de negócio, nas suas palavras (a fala vai em `citacoes`). Na pendência, a ação."
                },
                "responsavel": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 200,
                    "description": "Só na pendência: quem ficou de fazer."
                },
                "prazo": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 100,
                    "description": "Só na pendência: o prazo como foi dito (2026-11-20, até sexta)."
                },
                "referencias": {
                    "type": "array",
                    "maxItems": 10,
                    "items": { "type": "string", "minLength": 1, "maxLength": 10 },
                    "description": "Itens já registrados a que este se refere (N-3, R-1)."
                },
                "citacoes": {
                    "type": "array",
                    "minItems": 1,
                    "maxItems": LIMITE_DE_CITACOES,
                    "items": schema_da_citacao(),
                    "description": "Falas de origem do item."
                },
                "evidencias": {
                    "type": "array",
                    "maxItems": LIMITE_DE_EVIDENCIAS,
                    "items": evidencias::schema_da_evidencia(),
                    "description": "Opcional: trechos do código ligados ao item."
                }
            },
            "required": ["reuniao", "tipo", "texto", "citacoes"],
            "additionalProperties": false
        }),
        |ambiente, args| {
            let args: ArgsItem = ler_argumentos(args)?;
            let id = id_da_reuniao(&args.reuniao)?;
            if args.tipo != TipoItem::Pendencia
                && (args.responsavel.is_some() || args.prazo.is_some())
            {
                return Err(ErroFerramenta::Argumentos(
                    "`responsavel` e `prazo` só valem no tipo `pendencia`; registre a ação \
                     como pendência ou tire os dois campos"
                        .into(),
                ));
            }
            let referencias = args
                .referencias
                .iter()
                .map(|r| {
                    IdItemAta::analisar(r.trim()).ok_or_else(|| {
                        ErroFerramenta::Argumentos(format!(
                            "`referencias`: `{r}` não é o identificador de um item da ata; \
                             use D-1, N-1, R-1, P-1, DV-1 ou PP-1"
                        ))
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            let arquivos = arquivos_da_reuniao(ambiente, id)?;
            let todas = falas(ambiente, id)?;
            let citacoes = conferir_todas(id, &todas, &args.citacoes)?;
            let evidencias = evidencias::validar_todas(&ambiente.escopo, &args.evidencias)
                .map_err(ErroFerramenta::Recusa)?;
            let curto = |v: Option<String>| {
                v.map(|t| t.split_whitespace().collect::<Vec<_>>().join(" "))
                    .filter(|t| !t.is_empty())
            };
            let novo = NovoItem {
                tipo: args.tipo,
                texto: args.texto.trim().to_owned(),
                responsavel: curto(args.responsavel),
                prazo: curto(args.prazo),
                referencias,
                citacoes: citacoes.iter().map(|(_, c)| c.clone()).collect(),
                evidencias: evidencias.iter().map(documento::citacao).collect(),
            };
            let templates = Templates::da_base(&ambiente.base).0;
            let (item, estado, avisos) =
                templates.alterar_ata(&ambiente.kb, id, |estado: &mut EstadoAta| {
                    exigir_rascunho(estado)?;
                    Ok::<_, ErroFerramenta>(estado.adicionar_item(novo)?)
                })?;
            if !avisos.is_empty() {
                tracing::warn!(reuniao = %id, avisos = avisos.len(), "avisos na montagem da ata");
            }
            notificar_ata(ambiente, id);
            tracing::info!(
                reuniao = %id,
                item = %item,
                citacoes = citacoes.len(),
                evidencias = evidencias.len(),
                "item registrado na ata"
            );
            let ata = ambiente.kb.na_base(&arquivos.ata);
            let registrado = estado.item(item).expect("item recém-registrado");
            let texto = format!(
                "Item {item} ({}) registrado na ata da reunião {id}, com {} citação(ões) \
                 conferida(s){}. Ata: `{ata}`. Use {item} para apontar para este item em \
                 `referencias`.",
                registrado.tipo,
                citacoes.len(),
                if evidencias.is_empty() {
                    String::new()
                } else {
                    format!(" e {} evidência(s) de código", evidencias.len())
                }
            );
            Ok(Saida {
                texto,
                dados: Some(json!({
                    "reuniao": id,
                    "item": item,
                    "tipo": registrado.tipo,
                    "referencias": registrado.referencias,
                    "citacoes": citacoes_json(&citacoes),
                    "evidencias": evidencias.iter().map(|e| json!({
                        "arquivo": e.arquivo,
                        "linhas": e.linhas,
                        "hash": e.hash,
                    })).collect::<Vec<_>>(),
                    "ata": ata,
                    "itens_na_ata": estado.itens.len(),
                })),
            })
        },
    )
}

// ---------------------------------------------------------------------------
// sugerir_demanda

#[derive(Debug, Deserialize)]
struct ArgsSugerir {
    texto: String,
    #[serde(default)]
    limite: Option<usize>,
}

/// Palavras curtas e comuns, que não dizem nada sobre o assunto.
const PALAVRAS_VAZIAS: [&str; 24] = [
    "para", "pelo", "pela", "pelos", "pelas", "como", "mais", "menos", "sobre", "entre", "quando",
    "onde", "porque", "isso", "esse", "essa", "este", "esta", "deve", "pode", "precisa", "fazer",
    "temos", "hoje",
];

/// Palavras significativas de um texto: normalizadas, com 4 letras ou mais e
/// fora das [`PALAVRAS_VAZIAS`], sem repetir.
fn palavras_do_assunto(texto: &str) -> Vec<String> {
    let mut vistas = BTreeSet::new();
    normalizar_texto(texto)
        .split(' ')
        .filter(|p| p.chars().count() >= 4 && !PALAVRAS_VAZIAS.contains(p))
        .filter(|p| vistas.insert((*p).to_owned()))
        .map(str::to_owned)
        .collect()
}

/// Radical simples para comparar títulos: as 5 primeiras letras
/// (`cancelamento` e `cancelar` se encontram).
fn radical(palavra: &str) -> String {
    palavra.chars().take(5).collect()
}

/// `sugerir_demanda`: demandas abertas parecidas com um texto.
pub fn sugerir_demanda() -> Ferramenta {
    Ferramenta::nova(
        "sugerir_demanda",
        "Sugerir demanda parecida",
        "Procura, entre as demandas abertas do projeto, as parecidas com um texto (o \
         assunto da reunião ou o pedido do cliente), pela busca na base e pelas palavras \
         do título. Devolve o identificador (D-NNNN), o título, a chave no rastreador e o \
         motivo de cada sugestão. Use para propor ao analista ligar a análise a uma \
         demanda que já existe, em vez de criar outra; quem decide é o analista.",
        json!({
            "type": "object",
            "properties": {
                "texto": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 2000,
                    "description": "Assunto, em poucas frases (\"trava de cancelamento de fatura antiga\")."
                },
                "limite": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 10,
                    "description": format!("Sugestões no máximo (padrão {DEMANDAS_POR_PADRAO}).")
                }
            },
            "required": ["texto"],
            "additionalProperties": false
        }),
        |ambiente, args| {
            let args: ArgsSugerir = ler_argumentos(args)?;
            sugerir(
                ambiente,
                &args.texto,
                args.limite.unwrap_or(DEMANDAS_POR_PADRAO).clamp(1, 10),
            )
        },
    )
    .somente_leitura()
}

/// Uma demanda candidata, com a pontuação e os motivos.
#[derive(Default)]
struct Candidata {
    pontos: f64,
    motivos: Vec<String>,
}

fn sugerir(ambiente: &Ambiente, texto: &str, limite: usize) -> Result<Saida, ErroFerramenta> {
    let abertas: Vec<_> = ambiente
        .kb
        .demandas()
        .map_err(|e| ErroFerramenta::Interno(e.to_string()))?
        .into_iter()
        .filter(|d| d.aberta())
        .collect();
    let palavras = palavras_do_assunto(texto);
    if palavras.is_empty() {
        return Err(ErroFerramenta::Argumentos(
            "`texto`: informe o assunto com palavras de 4 letras ou mais".into(),
        ));
    }
    let mut candidatas: BTreeMap<IdDemanda, Candidata> = BTreeMap::new();
    let mut avisos = Vec::new();

    // Pelo título: palavras (pelo radical) em comum.
    let radicais: HashMap<String, &String> = palavras.iter().map(|p| (radical(p), p)).collect();
    for d in &abertas {
        let do_titulo = palavras_do_assunto(&d.documento.frontmatter.titulo);
        if do_titulo.is_empty() {
            continue;
        }
        let comuns: Vec<&str> = do_titulo
            .iter()
            .filter(|p| radicais.contains_key(&radical(p)))
            .map(String::as_str)
            .collect();
        if comuns.is_empty() {
            continue;
        }
        let parte = comuns.len() as f64 / do_titulo.len() as f64;
        if comuns.len() < 2 && parte < 0.5 {
            continue;
        }
        let c = candidatas.entry(d.id).or_default();
        c.pontos += parte * 2.0;
        c.motivos.push(format!(
            "título parecido ({} de {} palavras em comum: {})",
            comuns.len(),
            do_titulo.len(),
            comuns.join(", ")
        ));
    }

    // Pela busca na base (título, chave e descrição da demanda).
    match fontes::indice(ambiente) {
        Ok(indice) => {
            let consulta = Consulta::nova(
                palavras
                    .iter()
                    .take(30)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(" "),
            )
            .modo(Modo::Qualquer)
            .projeto(ambiente.kb.nome())
            .tipos([Tipo::Documento(TipoDocumento::Demanda)])
            .pagina(20, 0);
            match indice.buscar(&consulta) {
                Ok(pagina) => {
                    for (posicao, r) in pagina.resultados.iter().enumerate() {
                        let Some(id) = r
                            .identificador
                            .as_deref()
                            .and_then(|i| IdDemanda::analisar(i).ok())
                        else {
                            continue;
                        };
                        if !abertas.iter().any(|d| d.id == id) {
                            continue;
                        }
                        let c = candidatas.entry(id).or_default();
                        c.pontos += 1.0 / (1.0 + posicao as f64);
                        let trecho = cortar(&r.trecho, 160).0;
                        c.motivos.push(if trecho.is_empty() {
                            "busca na base".to_owned()
                        } else {
                            format!("busca na base: {trecho}")
                        });
                    }
                }
                Err(e) => avisos.push(format!("busca no índice da base falhou ({e})")),
            }
        }
        Err(e) => avisos.push(e.to_string()),
    }

    let mut ordem: Vec<(IdDemanda, Candidata)> = candidatas.into_iter().collect();
    ordem.sort_by(|a, b| b.1.pontos.total_cmp(&a.1.pontos).then(b.0.cmp(&a.0)));
    ordem.truncate(limite);
    let sugestoes: Vec<Value> = ordem
        .iter()
        .filter_map(|(id, c)| {
            let d = abertas.iter().find(|d| d.id == *id)?;
            let f = &d.documento.frontmatter;
            Some(json!({
                "id": id,
                "titulo": f.titulo,
                "chave": f.chave,
                "motivo": c.motivos.join("; "),
                "demanda": ambiente.kb.na_base(&d.arquivo),
            }))
        })
        .collect();

    let mut t = String::new();
    if abertas.is_empty() {
        let _ = writeln!(t, "O projeto não tem demandas abertas.");
    } else if sugestoes.is_empty() {
        let _ = writeln!(
            t,
            "Nenhuma das {} demandas abertas parece com o texto.",
            abertas.len()
        );
    } else {
        let _ = writeln!(
            t,
            "Demandas abertas parecidas ({} de {} abertas):",
            sugestoes.len(),
            abertas.len()
        );
        for s in &sugestoes {
            let chave = s["chave"]
                .as_str()
                .map(|c| format!(" [{c}]"))
                .unwrap_or_default();
            let _ = writeln!(
                t,
                "- {}{chave}: {} — {}",
                s["id"].as_str().unwrap_or_default(),
                s["titulo"].as_str().unwrap_or_default(),
                s["motivo"].as_str().unwrap_or_default()
            );
        }
        let _ = writeln!(t, "Proponha ao analista a ligação; quem decide é ele.");
    }
    for a in &avisos {
        let _ = writeln!(t, "Aviso: {a}");
    }
    tracing::info!(
        abertas = abertas.len(),
        sugeridas = sugestoes.len(),
        "demandas sugeridas"
    );
    let mut dados = json!({
        "abertas": abertas.len(),
        "sugestoes": sugestoes,
    });
    if !avisos.is_empty() {
        dados["avisos"] = json!(avisos);
    }
    Ok(Saida {
        texto: t,
        dados: Some(dados),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identificadores_de_reuniao_e_demanda() {
        assert_eq!(id_da_reuniao(" R-0003 ").unwrap().to_string(), "R-0003");
        assert!(
            id_da_reuniao("A-0001")
                .unwrap_err()
                .to_string()
                .contains("R-NNNN")
        );
        assert_eq!(id_da_demanda("D-0002").unwrap().to_string(), "D-0002");
        assert!(
            id_da_demanda("2")
                .unwrap_err()
                .to_string()
                .contains("D-NNNN")
        );
    }

    #[test]
    fn palavras_do_assunto_tiram_as_curtas_e_as_vazias() {
        assert_eq!(
            palavras_do_assunto("Trava de cancelamento para a fatura, fatura antiga"),
            ["trava", "cancelamento", "fatura", "antiga"]
        );
        assert_eq!(radical("cancelamento"), radical("cancelar"));
    }

    #[test]
    fn horario_do_argumento_aceita_hh_mm_ss_e_mm_ss() {
        let h = horario_do_argumento("desde", Some("01:02:03"))
            .unwrap()
            .unwrap();
        assert_eq!(h.segundos(), 3723);
        let h = horario_do_argumento("desde", Some("12:34"))
            .unwrap()
            .unwrap();
        assert_eq!(horario(h), "00:12:34");
        assert!(horario_do_argumento("ate", Some("1h")).is_err());
    }
}
