//! Ciclo da análise: `iniciar_analise`, `retomar_analise` e `concluir_analise`
//! (spec, seções 4.2, 4.7, 6.1 e 7.3).
//!
//! - `iniciar_analise` recebe a análise que o app criou (`analise`, o caminho da
//!   spec, que liga a sessão à análise pelo hook `PreToolUse`) ou cria uma nova a
//!   partir de `fluxo` e `entrada`, quando o analista pediu direto no chat. Copia
//!   os templates padrão para a base, se faltarem, põe a análise em andamento,
//!   monta o documento em rascunho e devolve o roteiro do fluxo com o contexto da
//!   base.
//! - `retomar_analise` só lê: devolve a entrada, as afirmações com as evidências,
//!   as perguntas, as sugestões e os termos, com o roteiro e o próximo passo, para
//!   uma sessão nova continuar de onde a outra parou.
//! - `concluir_analise` confere as pendências (nenhuma afirmação, seção obrigatória
//!   vazia, evidência que não confere mais com o código), muda o estado para
//!   concluída, remonta o documento e devolve o caminho que o agente cita no chat.

use std::collections::HashMap;
use std::fmt::Write as _;

use cba_core::ambiente::Ambiente;
use cba_core::analise::{
    Analise, Confianca, EstadoAfirmacao, EstadoAnalise, EstadoEvidencia, EstadoPergunta, Fluxo,
};
use cba_core::analises::NovaAnalise;
use cba_core::eventos::Mudanca;
use cba_docs::Templates;
use cba_index::detectar_git;
use cba_kb::Id;
use cba_kb::frontmatter::VersaoCodigo;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::comum;
use crate::contexto::Contexto;
use crate::documento::secoes_de_afirmacao;
use crate::evidencias::{ArquivoDoSistema, hash_das_linhas};
use crate::registro::{ErroFerramenta, Ferramenta, Saida, ler_argumentos};
use crate::roteiro::{self, nome_do_fluxo, secoes_obrigatorias, titulo_da_secao};

/// Tamanho máximo do título derivado da entrada, em caracteres.
const TITULO_MAXIMO: usize = 80;

/// Título a partir da entrada: a primeira linha, cortada numa palavra.
fn titulo_da_entrada(entrada: &str) -> String {
    let linha = entrada.trim().lines().next().unwrap_or_default().trim();
    if linha.chars().count() <= TITULO_MAXIMO {
        return linha.to_owned();
    }
    let corte: String = linha.chars().take(TITULO_MAXIMO).collect();
    let corte = match corte.rfind(' ') {
        Some(i) if i > TITULO_MAXIMO / 2 => &corte[..i],
        _ => &corte,
    };
    format!("{}…", corte.trim_end())
}

/// Branch e commit da primeira pasta do sistema, quando ela está num repositório.
fn versao_do_codigo(ambiente: &Ambiente) -> Option<VersaoCodigo> {
    let git = detectar_git(&ambiente.escopo.pastas().first()?.caminho)?;
    Some(VersaoCodigo {
        branch: git.branch,
        commit: git.commit,
    })
}

/// Documento da análise, relativo à base.
fn documento(ambiente: &Ambiente, analise: &Analise) -> String {
    ambiente.kb.na_base(&analise.documento).to_string()
}

fn afirmacoes_validas(analise: &Analise) -> impl Iterator<Item = &cba_core::analise::Afirmacao> {
    analise
        .afirmacoes
        .iter()
        .filter(|a| !matches!(a.estado, EstadoAfirmacao::Incorreta { .. }))
}

fn perguntas_abertas(analise: &Analise) -> impl Iterator<Item = &cba_core::analise::Pergunta> {
    analise
        .perguntas
        .iter()
        .filter(|p| p.estado == EstadoPergunta::Aberta)
}

/// Contagem dos registros, para o agente saber o que já existe.
fn contagem(analise: &Analise) -> Value {
    json!({
        "afirmacoes": afirmacoes_validas(analise).count(),
        "perguntas_abertas": perguntas_abertas(analise).count(),
        "sugestoes": analise.sugestoes.len(),
        "termos": analise.termos.len(),
    })
}

/// Saída com o texto para o agente e os dados estruturados (que repetem o
/// roteiro, para clientes que só leem um dos dois).
fn saida(texto: String, dados: Value) -> Saida {
    Saida {
        texto,
        dados: Some(dados),
    }
}

// ---------------------------------------------------------------------------
// iniciar_analise

#[derive(Debug, Deserialize)]
struct ArgsIniciar {
    #[serde(default)]
    analise: Option<String>,
    #[serde(default)]
    fluxo: Option<Fluxo>,
    #[serde(default)]
    entrada: Option<String>,
    #[serde(default)]
    titulo: Option<String>,
}

/// `iniciar_analise`: o roteiro do fluxo, a entrada e o contexto da base.
pub fn iniciar_analise() -> Ferramenta {
    Ferramenta::nova(
        "iniciar_analise",
        "Iniciar análise",
        "Inicia o trabalho numa análise e devolve o roteiro do fluxo, a entrada do \
         analista e o contexto da base (regras, termos e análises relacionados). Chame \
         antes de tudo. Quando o analista pedir \"Inicie a análise A-NNNN\", passe só \
         `analise`. Quando ele fizer o pedido direto no chat, sem identificador, passe \
         `fluxo` e `entrada` (e, se quiser, `titulo`) para criar a análise; use o \
         identificador devolvido em todas as chamadas seguintes. Siga o roteiro \
         devolvido.",
        json!({
            "type": "object",
            "properties": {
                "analise": comum::schema_da_analise(),
                "fluxo": {
                    "type": "string",
                    "enum": ["perguntar", "explicar", "impacto"],
                    "description": "Só para criar: perguntar (pergunta ao sistema), explicar (explicar funcionalidade, documento negocial) ou impacto (análise de impacto de uma demanda)."
                },
                "entrada": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 20000,
                    "description": "Só para criar: a pergunta, a funcionalidade ou o texto da demanda, como o analista escreveu."
                },
                "titulo": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 200,
                    "description": "Só para criar: título curto da análise. Sem ele, vale a primeira linha da entrada."
                }
            },
            "additionalProperties": false
        }),
        |ambiente, args| {
            let args: ArgsIniciar = ler_argumentos(args)?;
            let (id, criada) = match (args.analise, args.fluxo, args.entrada) {
                (Some(analise), None, None) if args.titulo.is_none() => {
                    (comum::id_da_analise(&analise)?, false)
                }
                (Some(_), ..) => {
                    return Err(ErroFerramenta::Argumentos(
                        "passe `analise` para iniciar uma análise que já existe, ou \
                         `fluxo` e `entrada` para criar uma nova, não os dois"
                            .into(),
                    ));
                }
                (None, Some(fluxo), Some(entrada)) => {
                    let titulo = match args.titulo {
                        Some(t) if !t.trim().is_empty() => t,
                        _ => titulo_da_entrada(&entrada),
                    };
                    let analise = ambiente.analises.criar(NovaAnalise {
                        fluxo,
                        titulo,
                        entrada,
                        analista: None,
                        base: versao_do_codigo(ambiente),
                    })?;
                    (analise.id, true)
                }
                (None, ..) => {
                    return Err(ErroFerramenta::Argumentos(
                        "informe `analise` (o A-NNNN do pedido do analista) ou, para criar \
                         uma análise nova, `fluxo` e `entrada`"
                            .into(),
                    ));
                }
            };
            iniciar(ambiente, &id, criada)
        },
    )
}

fn iniciar(ambiente: &Ambiente, id: &Id, criada: bool) -> Result<Saida, ErroFerramenta> {
    let atual = ambiente.analises.carregar(id)?;
    if atual.estado == EstadoAnalise::Aprovada {
        return Err(ErroFerramenta::Recusa(format!(
            "a análise {id} já foi aprovada pelo analista e não aceita registros novos. \
             Para continuar o assunto, peça ao analista para criar uma análise nova"
        )));
    }
    let mut avisos = Vec::new();
    match Templates::copiar_padrao(&ambiente.base) {
        Ok(criados) if !criados.is_empty() => {
            tracing::info!(
                templates = criados.len(),
                "templates padrão copiados para a base"
            );
        }
        Ok(_) => {}
        Err(e) => avisos.push(format!(
            "os templates padrão não foram copiados para a base ({e}); o documento usa o \
             padrão embutido"
        )),
    }
    let registrado = comum::alterar(ambiente, id, Mudanca::Estado, |a, _| a.iniciar())?;
    avisos.extend(registrado.aviso.clone());
    let analise = &registrado.analise;
    let contexto = Contexto::da_analise(ambiente, analise);
    let documento = documento(ambiente, analise);
    let roteiro = roteiro::roteiro(analise, &documento, &contexto.markdown());

    let mut texto = String::new();
    if criada {
        let _ = writeln!(
            texto,
            "Análise {id} criada. Use analise: \"{id}\" em todas as chamadas seguintes e \
             informe o identificador ao analista.\n"
        );
    }
    let registros = afirmacoes_validas(analise).count() + analise.perguntas.len();
    if registros > 0 {
        let _ = writeln!(
            texto,
            "Esta análise já tem registros ({} afirmações, {} perguntas abertas). Chame \
             `retomar_analise` para vê-los antes de continuar e não registre de novo o que \
             já está lá.\n",
            afirmacoes_validas(analise).count(),
            perguntas_abertas(analise).count()
        );
    }
    texto.push_str(&roteiro);
    for aviso in &avisos {
        let _ = write!(texto, "\nAviso: {aviso}\n");
    }

    let mut dados = json!({
        "analise": analise.id,
        "criada": criada,
        "titulo": analise.titulo,
        "fluxo": analise.fluxo,
        "estado": analise.estado,
        "entrada": analise.entrada,
        "documento": documento,
        "secoes": secoes_de_afirmacao(analise.fluxo),
        "secoes_obrigatorias": secoes_obrigatorias(analise.fluxo),
        "registros": contagem(analise),
        "contexto": contexto.json(),
        "roteiro": roteiro,
    });
    if !avisos.is_empty() {
        dados["avisos"] = json!(avisos);
    }
    Ok(saida(texto, dados))
}

// ---------------------------------------------------------------------------
// retomar_analise

#[derive(Debug, Deserialize)]
struct ArgsAnalise {
    analise: String,
}

fn schema_so_da_analise() -> Value {
    json!({
        "type": "object",
        "properties": { "analise": comum::schema_da_analise() },
        "required": ["analise"],
        "additionalProperties": false
    })
}

/// `retomar_analise`: o estado de uma análise para continuar numa sessão nova.
pub fn retomar_analise() -> Ferramenta {
    Ferramenta::nova(
        "retomar_analise",
        "Retomar análise",
        "Devolve o estado de uma análise já iniciada (\"Retome a análise A-NNNN\"): \
         entrada, afirmações com as evidências, perguntas, sugestões e termos \
         registrados até aqui, o que o analista já marcou, o roteiro do fluxo e o \
         próximo passo. Use numa sessão nova para continuar de onde a outra parou, sem \
         registrar de novo o que já existe.",
        schema_so_da_analise(),
        |ambiente, args| {
            let args: ArgsAnalise = ler_argumentos(args)?;
            let id = comum::id_da_analise(&args.analise)?;
            let analise = ambiente.analises.carregar(&id)?;
            Ok(retomar(ambiente, &analise))
        },
    )
    .somente_leitura()
}

fn rotulo_da_confianca(c: Confianca) -> &'static str {
    match c {
        Confianca::Confirmado => "confirmado",
        Confianca::Inferido => "inferido",
        Confianca::AConfirmar => "a_confirmar",
    }
}

fn rotulo_do_estado(e: &EstadoAfirmacao) -> &'static str {
    match e {
        EstadoAfirmacao::Registrada => "registrada",
        EstadoAfirmacao::Confirmada => "confirmada pelo analista",
        EstadoAfirmacao::Incorreta { .. } => "incorreta, segundo o analista",
        EstadoAfirmacao::Aprofundar => "o analista pediu para aprofundar",
        EstadoAfirmacao::ARevalidar => "a revalidar: o código citado mudou",
    }
}

fn proximo_passo(analise: &Analise) -> String {
    let id = &analise.id;
    match analise.estado {
        EstadoAnalise::Criada => format!(
            "A análise ainda não foi iniciada: siga o roteiro abaixo, registrando com \
             analise: \"{id}\"."
        ),
        EstadoAnalise::EmAndamento => format!(
            "Continue de onde a sessão anterior parou: complete as seções que faltam, \
             trate o que o analista marcou e não registre de novo o que já está aqui. Ao \
             terminar, chame `concluir_analise` (analise: \"{id}\")."
        ),
        EstadoAnalise::Concluida => format!(
            "A rodada foi concluída e o documento está em revisão pelo analista. Se ele \
             pedir mais (aprofundar, corrigir, responder perguntas), registre: a análise \
             volta a em andamento. Depois, chame `concluir_analise` (analise: \"{id}\") de \
             novo."
        ),
        EstadoAnalise::Aprovada => "A análise foi aprovada e não aceita registros novos. Para \
             continuar o assunto, peça ao analista para criar uma análise nova."
            .to_owned(),
    }
}

fn retomar(ambiente: &Ambiente, analise: &Analise) -> Saida {
    let fluxo = analise.fluxo;
    let documento = documento(ambiente, analise);
    let mut t = String::new();
    let _ = writeln!(t, "# Estado da análise {}: {}", analise.id, analise.titulo);
    let _ = writeln!(t);
    let _ = writeln!(
        t,
        "Fluxo: {}. Estado: {}. Documento: `{documento}`.",
        nome_do_fluxo(fluxo),
        serde_json::to_value(analise.estado)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default()
    );
    let _ = writeln!(t);
    let _ = writeln!(t, "Próximo passo: {}", proximo_passo(analise));
    let _ = writeln!(t);
    let _ = writeln!(t, "## Afirmações");
    let _ = writeln!(t);
    if analise.afirmacoes.is_empty() {
        let _ = writeln!(t, "Nenhuma afirmação registrada.");
    }
    for secao in secoes_de_afirmacao(fluxo) {
        let daqui: Vec<_> = analise
            .afirmacoes
            .iter()
            .filter(|a| a.secao == secao)
            .collect();
        if daqui.is_empty() {
            continue;
        }
        let _ = writeln!(t, "### {} (`{secao}`)", titulo_da_secao(fluxo, secao));
        let _ = writeln!(t);
        for a in daqui {
            let _ = writeln!(
                t,
                "{}. [{}; {}] {}",
                a.numero,
                rotulo_da_confianca(a.confianca),
                rotulo_do_estado(&a.estado),
                a.texto
            );
            if let EstadoAfirmacao::Incorreta { comentario } = &a.estado {
                let _ = writeln!(t, "   Comentário do analista: {comentario}");
            }
            for e in &a.evidencias {
                let marca = if e.estado == EstadoEvidencia::ARevalidar {
                    " (mudou desde o registro)"
                } else {
                    ""
                };
                let _ = writeln!(
                    t,
                    "   - `{}:{}-{}`{marca}: {}",
                    e.arquivo,
                    e.linhas.inicio,
                    e.linhas.fim,
                    e.trecho.lines().next().unwrap_or_default().trim()
                );
            }
        }
        let _ = writeln!(t);
    }
    let pendentes: Vec<_> = secoes_obrigatorias(fluxo)
        .iter()
        .filter(|s| !afirmacoes_validas(analise).any(|a| a.secao == **s))
        .map(|s| format!("`{s}`"))
        .collect();
    if !pendentes.is_empty() && analise.estado != EstadoAnalise::Aprovada {
        let _ = writeln!(
            t,
            "Seções obrigatórias ainda sem afirmação: {}.\n",
            pendentes.join(", ")
        );
    }
    let _ = writeln!(t, "## Perguntas");
    let _ = writeln!(t);
    if analise.perguntas.is_empty() {
        let _ = writeln!(t, "Nenhuma pergunta registrada.");
    }
    for p in &analise.perguntas {
        let estado = match &p.estado {
            EstadoPergunta::Aberta => "aberta".to_owned(),
            EstadoPergunta::Respondida { resposta } => format!("respondida: {resposta}"),
            EstadoPergunta::Descartada => "descartada".to_owned(),
        };
        let _ = writeln!(t, "{}. [{estado}] {}", p.numero, p.texto);
    }
    let _ = writeln!(t);
    if !analise.sugestoes.is_empty() {
        let _ = writeln!(t, "## Sugestões de alteração");
        let _ = writeln!(t);
        for s in &analise.sugestoes {
            let id = s.id.as_ref().map(|i| format!("{i} ")).unwrap_or_default();
            let _ = writeln!(t, "{}. {id}{} (`{}`)", s.numero, s.titulo, s.arquivo);
        }
        let _ = writeln!(t);
    }
    if !analise.termos.is_empty() {
        let _ = writeln!(t, "## Termos propostos");
        let _ = writeln!(t);
        for termo in &analise.termos {
            let _ = writeln!(t, "{}. {}: {}", termo.numero, termo.termo, termo.definicao);
        }
        let _ = writeln!(t);
    }
    let roteiro = (analise.estado != EstadoAnalise::Aprovada).then(|| {
        let contexto = Contexto::da_analise(ambiente, analise);
        roteiro::roteiro(analise, &documento, &contexto.markdown())
    });
    if let Some(r) = &roteiro {
        t.push_str(r);
    }

    let afirmacoes: Vec<Value> = analise
        .afirmacoes
        .iter()
        .map(|a| {
            let mut v = json!({
                "numero": a.numero,
                "secao": a.secao,
                "texto": a.texto,
                "confianca": a.confianca,
                "estado": a.estado,
                "evidencias": a.evidencias.iter().map(|e| json!({
                    "arquivo": e.arquivo,
                    "linhas": e.linhas,
                    "trecho": e.trecho,
                    "estado": e.estado,
                })).collect::<Vec<_>>(),
            });
            // `estado` vem como objeto ({"estado": "incorreta", "comentario": ...}).
            if let Some(estado) = v["estado"].get("estado").cloned() {
                if let Some(c) = v["estado"].get("comentario").cloned() {
                    v["comentario"] = c;
                }
                v["estado"] = estado;
            }
            v
        })
        .collect();
    let perguntas: Vec<Value> = analise
        .perguntas
        .iter()
        .map(|p| {
            let (estado, resposta) = match &p.estado {
                EstadoPergunta::Aberta => ("aberta", None),
                EstadoPergunta::Respondida { resposta } => ("respondida", Some(resposta)),
                EstadoPergunta::Descartada => ("descartada", None),
            };
            json!({
                "numero": p.numero,
                "texto": p.texto,
                "afirmacao": p.afirmacao,
                "estado": estado,
                "resposta": resposta,
            })
        })
        .collect();
    let sugestoes: Vec<Value> = analise
        .sugestoes
        .iter()
        .map(|s| {
            json!({
                "numero": s.numero,
                "id": s.id,
                "titulo": s.titulo,
                "arquivo": s.arquivo,
                "estado": s.estado,
            })
        })
        .collect();
    let termos: Vec<Value> = analise
        .termos
        .iter()
        .map(|t| json!({ "numero": t.numero, "termo": t.termo, "estado": t.estado }))
        .collect();
    saida(
        t,
        json!({
            "analise": analise.id,
            "titulo": analise.titulo,
            "fluxo": fluxo,
            "estado": analise.estado,
            "entrada": analise.entrada,
            "documento": documento,
            "sessoes": analise.sessoes.len(),
            "afirmacoes": afirmacoes,
            "perguntas": perguntas,
            "sugestoes": sugestoes,
            "termos": termos,
            "secoes_obrigatorias_pendentes": secoes_obrigatorias(fluxo)
                .iter()
                .filter(|s| !afirmacoes_validas(analise).any(|a| a.secao == **s))
                .collect::<Vec<_>>(),
            "proximo_passo": proximo_passo(analise),
            "roteiro": roteiro,
        }),
    )
}

// ---------------------------------------------------------------------------
// concluir_analise

/// `concluir_analise`: fecha a rodada e devolve o caminho do documento.
pub fn concluir_analise() -> Ferramenta {
    Ferramenta::nova(
        "concluir_analise",
        "Concluir análise",
        "Fecha a rodada da análise: confere as pendências, monta o documento final na \
         base e o deixa em revisão pelo analista. Devolve o caminho do documento, que \
         você cita no chat junto de um resumo curto (o documento completo não vai para o \
         chat), e as perguntas em aberto. Recusa, com o que falta, se não houver \
         afirmações ou se uma seção obrigatória do fluxo estiver vazia. Pode ser chamada \
         de novo depois de registros novos.",
        schema_so_da_analise(),
        |ambiente, args| {
            let args: ArgsAnalise = ler_argumentos(args)?;
            let id = comum::id_da_analise(&args.analise)?;
            concluir(ambiente, &id)
        },
    )
}

/// Chave de uma evidência para a conferência: arquivo, intervalo e hash gravado.
type Chave = (String, u32, u32, String);

fn chave(e: &cba_core::analise::Evidencia) -> Chave {
    (
        e.arquivo.to_string(),
        e.linhas.inicio,
        e.linhas.fim,
        e.hash.clone(),
    )
}

/// Confere, fora da trava, se cada evidência válida das afirmações ainda bate com o
/// código (o hash do intervalo). Arquivos são lidos uma vez.
fn conferir_evidencias(ambiente: &Ambiente, analise: &Analise) -> HashMap<Chave, bool> {
    let mut textos: HashMap<String, Option<String>> = HashMap::new();
    let mut resultado = HashMap::new();
    for e in afirmacoes_validas(analise)
        .flat_map(|a| &a.evidencias)
        .filter(|e| e.estado == EstadoEvidencia::Valida)
    {
        let arquivo = e.arquivo.to_string();
        let texto = textos.entry(arquivo.clone()).or_insert_with(|| {
            ArquivoDoSistema::abrir(&ambiente.escopo, &arquivo)
                .ok()
                .map(|a| a.texto)
        });
        let confere = texto
            .as_deref()
            .and_then(|t| hash_das_linhas(t, e.linhas))
            .is_some_and(|h| h == e.hash);
        resultado.insert(chave(e), confere);
    }
    resultado
}

fn concluir(ambiente: &Ambiente, id: &Id) -> Result<Saida, ErroFerramenta> {
    let atual = ambiente.analises.carregar(id)?;
    let fluxo = atual.fluxo;
    if atual.estado == EstadoAnalise::Aprovada {
        return Err(ErroFerramenta::Recusa(format!(
            "a análise {id} já foi aprovada pelo analista; não há o que concluir"
        )));
    }
    if afirmacoes_validas(&atual).next().is_none() {
        return Err(ErroFerramenta::Recusa(format!(
            "a análise {id} ainda não tem afirmações. Registre o que apurou com `afirmar` \
             (seções: {}) e chame `concluir_analise` de novo",
            secoes_de_afirmacao(fluxo).join(", ")
        )));
    }
    let faltam: Vec<String> = secoes_obrigatorias(fluxo)
        .iter()
        .filter(|s| !afirmacoes_validas(&atual).any(|a| a.secao == **s))
        .map(|s| format!("`{s}` ({})", titulo_da_secao(fluxo, s)))
        .collect();
    if !faltam.is_empty() {
        return Err(ErroFerramenta::Recusa(format!(
            "o documento de {} precisa de ao menos uma afirmação em: {}. Registre com \
             `afirmar` (use a confiança a_confirmar se o código não for suficiente) e \
             chame `concluir_analise` de novo",
            nome_do_fluxo(fluxo),
            faltam.join(", ")
        )));
    }

    let conferidas = conferir_evidencias(ambiente, &atual);
    let registrado = comum::alterar(ambiente, id, Mudanca::Estado, |a, _| {
        a.marcar_a_revalidar(|e| conferidas.get(&chave(e)).copied().unwrap_or(true));
        a.concluir()
    })?;
    if let Some(aviso) = &registrado.aviso {
        return Err(ErroFerramenta::Interno(format!(
            "a análise {id} foi marcada como concluída, mas o documento final não foi \
             gravado: {aviso}. Chame `concluir_analise` de novo; se persistir, avise o \
             analista"
        )));
    }
    let analise = &registrado.analise;
    let documento = documento(ambiente, analise);
    let arquivo = ambiente.kb.caminho(&analise.documento);

    let mut pendencias = Vec::new();
    for a in afirmacoes_validas(analise) {
        if a.estado == EstadoAfirmacao::ARevalidar {
            pendencias.push(format!(
                "afirmação {}: o código citado mudou desde o registro; registre de novo com \
                 o trecho atual, se a afirmação ainda vale",
                a.numero
            ));
        } else if a.confianca == Confianca::Inferido && a.evidencias.is_empty() {
            pendencias.push(format!(
                "afirmação {}: inferida sem evidência citada",
                a.numero
            ));
        }
        if a.estado == EstadoAfirmacao::Aprofundar {
            pendencias.push(format!(
                "afirmação {}: o analista pediu para aprofundar",
                a.numero
            ));
        }
    }
    let abertas: Vec<&str> = perguntas_abertas(analise)
        .map(|p| p.texto.as_str())
        .collect();

    let mut t = String::new();
    let _ = writeln!(
        t,
        "Análise {id} concluída. Documento na base: `{documento}` (em revisão pelo \
         analista).\nArquivo: {}\n",
        arquivo.display()
    );
    let _ = writeln!(
        t,
        "No chat, responda ao analista com um resumo curto ({}) e cite o caminho do \
         documento. Não copie o documento para o chat. O analista revisa e aprova as \
         afirmações no app.",
        match fluxo {
            Fluxo::Perguntar =>
                "a resposta, as regras numeradas, um exemplo com números e as perguntas em aberto",
            Fluxo::Explicar => "objetivo, regras principais e as perguntas em aberto",
            Fluxo::Impacto =>
                "a demanda como entendida, a abrangência, os principais riscos e as perguntas para o solicitante",
        }
    );
    if !abertas.is_empty() {
        let _ = writeln!(t, "\nPerguntas em aberto ({}):", abertas.len());
        for (i, p) in abertas.iter().enumerate() {
            let _ = writeln!(t, "{}. {p}", i + 1);
        }
    }
    if !pendencias.is_empty() {
        let _ = writeln!(t, "\nPendências (avise o analista):");
        for p in &pendencias {
            let _ = writeln!(t, "- {p}");
        }
    }
    Ok(saida(
        t,
        json!({
            "analise": analise.id,
            "estado": analise.estado,
            "documento": documento,
            "arquivo": arquivo,
            "registros": contagem(analise),
            "perguntas_abertas": abertas,
            "pendencias": pendencias,
        }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titulo_vem_da_primeira_linha_e_corta_em_palavra() {
        assert_eq!(
            titulo_da_entrada("  Como é calculada a multa?\nDetalhes"),
            "Como é calculada a multa?"
        );
        let longa = "Permitir parcelamento da fatura em até três vezes para clientes pessoa \
                     jurídica com contrato ativo";
        let t = titulo_da_entrada(longa);
        assert!(t.ends_with('…'), "{t}");
        assert!(t.chars().count() <= TITULO_MAXIMO + 1, "{t}");
        assert!(longa.starts_with(t.trim_end_matches('…')), "{t}");
    }
}
