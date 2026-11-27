//! Ciclo da análise: `iniciar_analise`, `retomar_analise` e `concluir_analise`
//! (spec, seções 4.2, 4.7, 6.1 e 7.3).
//!
//! - `iniciar_analise` recebe a análise que o app criou (`analise`, o caminho da
//!   spec, que liga a sessão à análise pelo hook `PreToolUse`) ou cria uma nova a
//!   partir de `fluxo` e `entrada`, quando o analista pediu direto no chat. Copia
//!   os templates padrão para a base, se faltarem, põe a análise em andamento,
//!   monta o documento em rascunho e devolve o roteiro do fluxo com o contexto da
//!   base. Aceita `demanda` (D-NNNN, opcional), gravada em `demanda.id` no
//!   frontmatter do documento, e, no fluxo `reuniao`, a reunião (R-NNNN), gravada
//!   no estado; o contexto devolvido traz então a demanda, as atas anteriores, as
//!   pendências e as pessoas do projeto ([`crate::contexto::contexto_da_demanda`]).
//! - `retomar_analise` só lê: devolve a entrada, as afirmações com as evidências,
//!   as perguntas, as sugestões e os termos, a memória de trabalho do agente
//!   (`memorizar`, #143), o roteiro e o próximo passo, para uma sessão nova
//!   continuar de onde a outra parou sem depender da conversa gravada.
//! - `concluir_analise` confere as pendências (nenhuma afirmação, seção obrigatória
//!   vazia, evidência que não confere mais com o código; no fluxo `reuniao`, ata
//!   sem itens), muda o estado para
//!   concluída, remonta o documento e devolve o caminho que o agente cita no chat.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Write as _;

use cba_core::ambiente::Ambiente;
use cba_core::analise::{
    Analise, Confianca, EstadoAfirmacao, EstadoAnalise, EstadoEvidencia, EstadoPergunta, Fluxo,
};
use cba_core::analises::NovaAnalise;
use cba_core::conducao::Conducao;
use cba_core::eventos::Mudanca;
use cba_core::memoria::Memoria;
use cba_core::prototipo::{Fidelidade, ModoPrototipo, PedidoPrototipo};
use cba_docs::{EstadoAta, Templates};
use cba_index::detectar_git;
use cba_kb::frontmatter::{Demanda, FrontmatterAnalise, VersaoCodigo};
use cba_kb::{DemandaLida, Documento, Id, IdDemanda, IdReuniao};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::comum;
use crate::contexto::Contexto;
use crate::documento::secoes_de_afirmacao;
use crate::evidencias::{ArquivoDoProjeto, hash_das_linhas};
use crate::parecer;
use crate::prompts::Prompts;
use crate::registro::{ErroFerramenta, Ferramenta, Saida, ler_argumentos};
use crate::reuniao;
use crate::roteiro::{Roteiros, nome_do_fluxo, secoes_obrigatorias, titulo_da_secao};

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

/// Branch e commit da primeira pasta do projeto, quando ela está num repositório.
pub(crate) fn versao_do_codigo(ambiente: &Ambiente) -> Option<VersaoCodigo> {
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
        "achados": analise.parecer.as_ref().map_or(0, |p| p.ativos().count()),
        "perguntas_abertas": perguntas_abertas(analise).count(),
        "sugestoes": analise.sugestoes.len(),
        "termos": analise.termos.len(),
        "historias": analise.historias_no_documento().count(),
        "tabelas": analise.tabelas_no_documento().count(),
        "cenarios": analise.cenarios_no_documento().count(),
        "telas": analise.telas().len(),
    })
}

/// Orientação da memória para quando o roteiro (uma cópia editável da base mais
/// antiga que a `memorizar`) não fala dela.
fn lembrete_da_memoria(roteiro: &str, id: &Id) -> Option<String> {
    (!roteiro.contains("`memorizar`")).then(|| {
        format!(
            "\n## Memória da análise\n\nMantenha a memória da análise com `memorizar` \
             (analise: \"{id}\"): o que foi investigado e descartado, as hipóteses em \
             aberto, onde está o código, o que o analista orientou, o que ficou pendente \
             e onde você parou. Atualize depois de uma descoberta relevante, quando o \
             analista der uma orientação e antes de concluir: é o que uma conversa nova \
             recebe em `retomar_analise` para continuar.\n"
        )
    })
}

/// Pedido padrão de uma análise de histórias a partir da análise de origem.
fn pedido_de_historias(origem: &Analise) -> String {
    format!(
        "Histórias de usuário com critérios de aceite a partir da análise {} ({}).",
        origem.id, origem.titulo
    )
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
    #[serde(default)]
    demanda: Option<String>,
    #[serde(default)]
    reuniao: Option<String>,
    #[serde(default)]
    origem: Option<String>,
    #[serde(default)]
    funcionalidade: Option<String>,
    #[serde(default)]
    telas: Vec<String>,
    #[serde(default)]
    fidelidade: Option<Fidelidade>,
    #[serde(default)]
    fora: Option<String>,
    #[serde(default)]
    modo: Option<ModoPrototipo>,
    #[serde(default)]
    conducao: Option<Conducao>,
    #[serde(default)]
    motivo: Option<String>,
}

/// Condução pedida em `iniciar_analise` (#371).
#[derive(Debug, Clone, Default)]
struct PedidoDeConducao {
    conducao: Option<Conducao>,
    motivo: Option<String>,
}

/// Ligações pedidas em `iniciar_analise`: a demanda (gravada em `demanda.id` no
/// frontmatter do documento) e, no fluxo `reuniao`, a reunião (no estado).
#[derive(Debug, Clone, Copy, Default)]
struct Ligacao {
    demanda: Option<IdDemanda>,
    reuniao: Option<IdReuniao>,
}

/// `demanda` do frontmatter a partir da demanda da base: o identificador, a
/// origem (padrão `texto`) e a chave no rastreador.
fn demanda_do_frontmatter(lida: &DemandaLida) -> Demanda {
    let f = &lida.documento.frontmatter;
    Demanda {
        id: Some(lida.id),
        origem: f.origem.clone().unwrap_or_else(|| "texto".into()),
        chave: f.chave.clone(),
    }
}

/// Demanda da base, ou a recusa dizendo que não existe.
fn ler_demanda(ambiente: &Ambiente, id: IdDemanda) -> Result<DemandaLida, ErroFerramenta> {
    ambiente.kb.demanda(id).map_err(|e| match e {
        cba_kb::Erro::DemandaInexistente { .. } => ErroFerramenta::Recusa(format!(
            "a demanda {id} não existe neste projeto; confira o identificador (D-NNNN) ou \
             use `sugerir_demanda` para achar uma demanda aberta parecida"
        )),
        outro => ErroFerramenta::Interno(outro.to_string()),
    })
}

/// Demanda aberta, para ligar uma análise a ela.
fn demanda_aberta(ambiente: &Ambiente, id: IdDemanda) -> Result<DemandaLida, ErroFerramenta> {
    let lida = ler_demanda(ambiente, id)?;
    if !lida.aberta() {
        return Err(ErroFerramenta::Recusa(format!(
            "a demanda {id} está encerrada e não recebe análises novas; peça ao analista \
             para reabri-la ou escolha outra"
        )));
    }
    Ok(lida)
}

/// Cria uma análise do fluxo `historias` (#115) a partir da análise de origem
/// (`origem`, A-NNNN, que não pode ser outra de histórias) ou de uma demanda
/// aberta (`demanda`). Sem `demanda`, a análise herda a da origem. Sem `entrada` e
/// sem `titulo`, eles vêm da origem (ou da demanda). Usada por `iniciar_analise` e
/// pela tela de nova análise do app.
pub fn criar_analise_de_historias(
    ambiente: &Ambiente,
    origem: Option<Id>,
    demanda: Option<IdDemanda>,
    entrada: Option<String>,
    titulo: Option<String>,
) -> Result<Analise, ErroFerramenta> {
    let lida = origem
        .as_ref()
        .map(|o| ambiente.analises.carregar(o))
        .transpose()?;
    if let Some(o) = &lida
        && matches!(o.fluxo, Fluxo::Historias | Fluxo::Cenarios)
    {
        return Err(ErroFerramenta::Recusa(format!(
            "a análise {} já é de histórias ou de cenários; a origem é a análise do código de onde vêm as \
             afirmações (impacto, explicação ou pergunta)",
            o.id
        )));
    }
    // A demanda pedida ou, sem ela, a da análise de origem.
    let demanda = match (demanda, &lida) {
        (Some(d), _) => Some(demanda_aberta(ambiente, d)?),
        (None, Some(o)) => ambiente
            .kb
            .ler::<FrontmatterAnalise>(&o.documento)
            .ok()
            .and_then(|d| d.frontmatter.demanda.id)
            .and_then(|d| ler_demanda(ambiente, d).ok()),
        (None, None) => {
            return Err(ErroFerramenta::Argumentos(
                "no fluxo `historias`, informe `origem` (a análise A-NNNN de onde vêm as \
                 afirmações) ou `demanda` (D-NNNN)"
                    .into(),
            ));
        }
    };
    let entrada =
        entrada
            .filter(|e| !e.trim().is_empty())
            .unwrap_or_else(|| match (&lida, &demanda) {
                (Some(o), _) => pedido_de_historias(o),
                (None, Some(d)) => format!(
                    "Histórias de usuário com critérios de aceite da demanda {} ({}).",
                    d.id, d.documento.frontmatter.titulo
                ),
                (None, None) => "Histórias de usuário com critérios de aceite.".into(),
            });
    let titulo = match titulo {
        Some(t) if !t.trim().is_empty() => t,
        _ => titulo_da_entrada(&match (&lida, &demanda) {
            (Some(o), _) => format!("Histórias · {}", o.titulo),
            (None, Some(d)) => format!("Histórias · {}", d.documento.frontmatter.titulo),
            (None, None) => entrada.clone(),
        }),
    };
    Ok(ambiente.analises.criar_derivada(
        NovaAnalise {
            fluxo: Fluxo::Historias,
            titulo,
            entrada,
            analista: None,
            base: versao_do_codigo(ambiente),
        },
        demanda.as_ref().map(demanda_do_frontmatter),
        origem,
    )?)
}

/// Cria uma análise do fluxo `cenarios` (#117) a partir da análise de origem
/// (`origem`, A-NNNN: uma análise do código ou uma análise de histórias, cujas
/// histórias os cenários podem citar), de uma demanda aberta (`demanda`) ou só do
/// pedido (`entrada`, com as regras a cobrir). Sem `demanda`, a análise herda a da
/// origem. Usada por `iniciar_analise` e pela tela de nova análise do app.
pub fn criar_analise_de_cenarios(
    ambiente: &Ambiente,
    origem: Option<Id>,
    demanda: Option<IdDemanda>,
    entrada: Option<String>,
    titulo: Option<String>,
) -> Result<Analise, ErroFerramenta> {
    let lida = origem
        .as_ref()
        .map(|o| ambiente.analises.carregar(o))
        .transpose()?;
    if let Some(o) = &lida
        && o.fluxo == Fluxo::Cenarios
    {
        return Err(ErroFerramenta::Recusa(format!(
            "a análise {} já é de cenários; a origem é a análise de onde vêm as regras e as \
             afirmações (impacto, explicação, pergunta) ou a de histórias",
            o.id
        )));
    }
    let entrada = entrada.filter(|e| !e.trim().is_empty());
    let demanda = match (demanda, &lida) {
        (Some(d), _) => Some(demanda_aberta(ambiente, d)?),
        (None, Some(o)) => ambiente
            .kb
            .ler::<FrontmatterAnalise>(&o.documento)
            .ok()
            .and_then(|d| d.frontmatter.demanda.id)
            .and_then(|d| ler_demanda(ambiente, d).ok()),
        (None, None) if entrada.is_none() => {
            return Err(ErroFerramenta::Argumentos(
                "no fluxo `cenarios`, informe `origem` (a análise A-NNNN de onde vêm as regras \
                 ou as histórias), `demanda` (D-NNNN) ou `entrada` com as regras a cobrir \
                 (\"Cenários das regras RN-FAT-013 e RN-FAT-014\")"
                    .into(),
            ));
        }
        (None, None) => None,
    };
    let entrada = entrada.unwrap_or_else(|| match (&lida, &demanda) {
        (Some(o), _) if o.fluxo == Fluxo::Historias => format!(
            "Cenários de teste das histórias da análise {} ({}).",
            o.id, o.titulo
        ),
        (Some(o), _) => format!(
            "Cenários de teste das regras da análise {} ({}).",
            o.id, o.titulo
        ),
        (None, Some(d)) => format!(
            "Cenários de teste das regras da demanda {} ({}).",
            d.id, d.documento.frontmatter.titulo
        ),
        (None, None) => "Cenários de teste.".into(),
    });
    let titulo = match titulo {
        Some(t) if !t.trim().is_empty() => t,
        _ => titulo_da_entrada(&match (&lida, &demanda) {
            (Some(o), _) => format!("Cenários · {}", o.titulo),
            (None, Some(d)) => format!("Cenários · {}", d.documento.frontmatter.titulo),
            (None, None) => entrada.clone(),
        }),
    };
    Ok(ambiente.analises.criar_derivada(
        NovaAnalise {
            fluxo: Fluxo::Cenarios,
            titulo,
            entrada,
            analista: None,
            base: versao_do_codigo(ambiente),
        },
        demanda.as_ref().map(demanda_do_frontmatter),
        origem,
    )?)
}

/// Cria uma análise do fluxo `prototipo` (#381) a partir da análise de origem
/// (`origem`, A-NNNN: uma análise do código ou uma análise de histórias, cujos
/// critérios de aceite os elementos das telas podem citar). A origem é
/// obrigatória e não pode ser outro protótipo nem uma análise de cenários; de
/// preferência aprovada (sem a aprovação, `iniciar_analise` avisa). Sem `demanda`,
/// a análise herda a da origem. O `pedido` do analista (telas, fidelidade e o que
/// deixar de fora) vai para o estado ([`cba_core::prototipo::Prototipo::pedido`])
/// e, em frases, para a entrada. Usada por `iniciar_analise` e pela tela de nova
/// análise do app.
///
/// O modo `design_hospedado` (#383) exige a confirmação do envio no pedido, dada
/// pelo analista no app; sem ela, nada é criado. A hora da confirmação é a da
/// criação.
pub fn criar_analise_de_prototipo(
    ambiente: &Ambiente,
    origem: Id,
    demanda: Option<IdDemanda>,
    pedido: PedidoPrototipo,
    entrada: Option<String>,
    titulo: Option<String>,
) -> Result<Analise, ErroFerramenta> {
    let o = ambiente.analises.carregar(&origem)?;
    if matches!(o.fluxo, Fluxo::Prototipo | Fluxo::Cenarios) {
        return Err(ErroFerramenta::Recusa(format!(
            "a análise {} é de {}; a origem do protótipo é a análise de onde vêm as regras \
             e as afirmações (impacto, explicação, pergunta) ou a de histórias",
            o.id,
            nome_do_fluxo(o.fluxo)
        )));
    }
    let demanda = match demanda {
        Some(d) => Some(demanda_aberta(ambiente, d)?),
        None => ambiente
            .kb
            .ler::<FrontmatterAnalise>(&o.documento)
            .ok()
            .and_then(|d| d.frontmatter.demanda.id)
            .and_then(|d| ler_demanda(ambiente, d).ok()),
    };
    let mut pedido = pedido.conferido();
    pedido.exigir_confirmacao().map_err(comum::recusa)?;
    if let Some(c) = pedido.confirmacao_envio.as_mut() {
        c.em_ms = cba_core::tempo::agora_ms();
    }
    let base = entrada.filter(|e| !e.trim().is_empty()).unwrap_or_else(|| {
        format!(
            "Protótipo de telas a partir da análise {} ({}).",
            o.id, o.titulo
        )
    });
    let entrada = format!("{}\n\n{}", base.trim_end(), pedido.descrever());
    let titulo = match titulo {
        Some(t) if !t.trim().is_empty() => t,
        _ => titulo_da_entrada(&format!("Protótipo · {}", o.titulo)),
    };
    let criada = ambiente.analises.criar_derivada(
        NovaAnalise {
            fluxo: Fluxo::Prototipo,
            titulo,
            entrada,
            analista: None,
            base: versao_do_codigo(ambiente),
        },
        demanda.as_ref().map(demanda_do_frontmatter),
        Some(origem),
    )?;
    let (_, analise) = ambiente
        .analises
        .alterar(&criada.id, Mudanca::Tela, |a, _| a.pedir_prototipo(pedido))?;
    Ok(analise)
}

/// `iniciar_analise`: o roteiro do fluxo, a entrada e o contexto da base.
pub fn iniciar_analise() -> Ferramenta {
    Ferramenta::nova(
        "iniciar_analise",
        "Iniciar análise",
        "Inicia o trabalho numa análise e devolve o roteiro do fluxo, a entrada do \
         analista e o contexto da base (regras, termos e análises relacionados e, se a \
         análise for de uma demanda, as atas anteriores, as pendências e as pessoas do \
         projeto). Chame antes de tudo. Quando o analista pedir \"Inicie a análise \
         A-NNNN\", passe só `analise`. Quando ele fizer o pedido direto no chat, sem \
         identificador, passe `fluxo` e `entrada` (e, se quiser, `titulo`) para criar a \
         análise; use o identificador devolvido em todas as chamadas seguintes. Para \
         analisar uma reunião, passe `fluxo: \"reuniao\"` e `reuniao` (R-NNNN). Para \
         gerar histórias com critérios de aceite, passe `fluxo: \"historias\"` e \
         `origem` (a análise A-NNNN de onde vêm as afirmações) ou `demanda`. Para \
         derivar cenários de teste das regras, passe `fluxo: \"cenarios\"` e `origem` \
         (uma análise do código ou de histórias), `demanda` ou `entrada` com as regras. \
         Para prototipar as telas a partir de uma análise validada, passe `fluxo: \
         \"prototipo\"` e `origem` (de preferência aprovada) e, se o analista disse, \
         `telas`, `fidelidade`, `fora` e `modo` (`html`, o padrão, ou `design_local`; o \
         `design_hospedado` envia dados para fora e só é criado pelo app, com a \
         confirmação do analista). \
         Para explicar uma funcionalidade do mapa de funcionalidades, passe `fluxo: \
         \"explicar\"` e `funcionalidade` (F-<SIGLA>-NNN). `demanda` (D-NNNN, opcional) \
         liga a análise a uma demanda do projeto. Passe `conducao: \"workflow\"` quando \
         o analista pedir a análise conduzida pelo workflow (ele também pede a \
         ferramenta Workflow); o roteiro devolvido é então o curto, da sessão que \
         espera o workflow e conclui. Se o workflow não puder rodar, chame de novo com \
         `conducao: \"simples\"` e `motivo`. Siga o roteiro devolvido.",
        json!({
            "type": "object",
            "properties": {
                "analise": comum::schema_da_analise(),
                "fluxo": {
                    "type": "string",
                    "enum": ["perguntar", "explicar", "impacto", "reuniao", "revisar", "historias", "cenarios", "manual", "prototipo"],
                    "description": "Só para criar: perguntar (pergunta ao sistema), explicar (explicar funcionalidade, documento negocial), impacto (análise de impacto de uma demanda), reuniao (montar a ata de uma reunião a partir do transcript; exige `reuniao`), revisar (revisar a qualidade de um requisito ou história contra a base e o código; a entrada é o texto revisado) historias (histórias com critérios de aceite; exige `origem` ou `demanda`) ou cenarios (cenários de teste derivados das regras: tabela de decisão, partição de equivalência e valores-limite; exige `origem`, `demanda` ou `entrada`) ou manual (manual do usuário de uma funcionalidade do mapa, com o passo a passo; exige `funcionalidade`) ou prototipo (protótipo de telas a partir de uma análise validada: telas afetadas e elementos ligados às regras e aos critérios; exige `origem`)."
                },
                "entrada": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 20000,
                    "description": "Só para criar: a pergunta, a funcionalidade, o texto da demanda ou o requisito ou história a revisar, como o analista escreveu. Nos fluxos reuniao, historias e cenarios é opcional (em cenarios sem origem nem demanda, diga as regras a cobrir)."
                },
                "origem": {
                    "type": "string",
                    "minLength": 1,
                    "description": "Só para criar, nos fluxos historias, cenarios e prototipo: a análise de origem (A-NNNN), normalmente a análise de impacto (ou, em cenarios e prototipo, a de histórias), de preferência aprovada. A análise nova herda a demanda dela."
                },
                "telas": {
                    "type": "array",
                    "maxItems": 30,
                    "items": { "type": "string", "minLength": 1, "maxLength": 200 },
                    "description": "Só para criar, no fluxo prototipo: as telas que o analista quer ver. Sem elas, o agente lista as telas afetadas."
                },
                "fidelidade": {
                    "type": "string",
                    "enum": ["wireframe", "alta_fidelidade"],
                    "description": "Só para criar, no fluxo prototipo: wireframe (padrão; estrutura e conteúdo) ou alta_fidelidade (com a identidade visual do projeto)."
                },
                "fora": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 2000,
                    "description": "Só para criar, no fluxo prototipo: o que deixar de fora do protótipo."
                },
                "modo": {
                    "type": "string",
                    "enum": ["html", "design_local", "design_hospedado"],
                    "description": "Só para criar, no fluxo prototipo: html (padrão; o HTML local do app), design_local (o método da ferramenta de design do Claude rodando aqui; nada sai do ambiente) ou design_hospedado (a ferramenta de design hospedada do Claude: telas, elementos e tokens saem do ambiente). O design_hospedado exige a confirmação explícita do analista, que só o app grava: por aqui ele é recusado; peça ao analista para criar o protótipo pelo app."
                },
                "titulo": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 200,
                    "description": "Só para criar: título curto da análise. Sem ele, vale a primeira linha da entrada."
                },
                "demanda": {
                    "type": "string",
                    "minLength": 1,
                    "description": "Opcional: a demanda do projeto (D-NNNN) a que a análise pertence. Grava a ligação na análise."
                },
                "reuniao": reuniao::schema_da_reuniao(),
                "funcionalidade": {
                    "type": "string",
                    "minLength": 1,
                    "description": "Só para criar, nos fluxos explicar e manual: a funcionalidade do mapa (F-<SIGLA>-NNN) a explicar ou cujo manual do usuário será escrito. A entrada vem do mapa, e o documento aprovado fica ligado a ela."
                },
                "conducao": {
                    "type": "string",
                    "enum": ["simples", "workflow"],
                    "description": "simples (padrão): você segue o roteiro completo. workflow: só quando o pedido do analista manda chamar a ferramenta Workflow; a investigação fica com o workflow do tipo, e você conclui."
                },
                "motivo": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 500,
                    "description": "Com conducao simples numa análise pedida com workflow: por que o workflow não rodou (ferramenta ausente ou recusada, erro ao carregar o script), em uma frase. Vai para o aviso que o analista vê."
                }
            },
            "additionalProperties": false
        }),
        |ambiente, args| {
            let args: ArgsIniciar = ler_argumentos(args)?;
            let conducao = PedidoDeConducao {
                conducao: args.conducao,
                motivo: args.motivo.clone(),
            };
            let mut ligacao = Ligacao {
                demanda: args
                    .demanda
                    .as_deref()
                    .map(reuniao::id_da_demanda)
                    .transpose()?,
                reuniao: args
                    .reuniao
                    .as_deref()
                    .map(reuniao::id_da_reuniao)
                    .transpose()?,
            };
            if args.funcionalidade.is_some()
                && !matches!(args.fluxo, Some(Fluxo::Explicar | Fluxo::Manual))
            {
                return Err(ErroFerramenta::Argumentos(
                    "`funcionalidade` só vale para criar uma análise dos fluxos `explicar` ou \
                     `manual`"
                        .into(),
                ));
            }
            if args.origem.is_some()
                && !matches!(
                    args.fluxo,
                    Some(Fluxo::Historias | Fluxo::Cenarios | Fluxo::Prototipo)
                )
            {
                return Err(ErroFerramenta::Argumentos(
                    "`origem` só vale para criar uma análise dos fluxos `historias`, \
                     `cenarios` ou `prototipo`"
                        .into(),
                ));
            }
            if (!args.telas.is_empty()
                || args.fidelidade.is_some()
                || args.fora.is_some()
                || args.modo.is_some())
                && args.fluxo != Some(Fluxo::Prototipo)
            {
                return Err(ErroFerramenta::Argumentos(
                    "`telas`, `fidelidade`, `fora` e `modo` só valem para criar uma análise do \
                     fluxo `prototipo`"
                        .into(),
                ));
            }
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
                (None, Some(Fluxo::Reuniao), entrada) => {
                    let Some(r) = ligacao.reuniao else {
                        return Err(ErroFerramenta::Argumentos(
                            "no fluxo `reuniao`, informe `reuniao`: o identificador da reunião \
                             (R-NNNN) que o analista criou no app"
                                .into(),
                        ));
                    };
                    let arquivos = reuniao::arquivos_da_reuniao(ambiente, r)?;
                    if let Some(d) = ligacao.demanda
                        && d != arquivos.demanda
                    {
                        return Err(ErroFerramenta::Recusa(format!(
                            "a reunião {r} é da demanda {}, não de {d}; tire `demanda` ou \
                             corrija-a",
                            arquivos.demanda
                        )));
                    }
                    let lida = ler_demanda(ambiente, arquivos.demanda)?;
                    let titulo_da_demanda = lida.documento.frontmatter.titulo.clone();
                    let entrada = entrada.filter(|e| !e.trim().is_empty()).unwrap_or_else(|| {
                        format!(
                            "Reunião {r} da demanda {} ({titulo_da_demanda}). Ata: `{}`.",
                            arquivos.demanda,
                            ambiente.kb.na_base(&arquivos.ata)
                        )
                    });
                    let titulo = match args.titulo {
                        Some(t) if !t.trim().is_empty() => t,
                        _ => titulo_da_entrada(&format!("Reunião {r} · {titulo_da_demanda}")),
                    };
                    let analise = ambiente.analises.criar_com_demanda(
                        NovaAnalise {
                            fluxo: Fluxo::Reuniao,
                            titulo,
                            entrada,
                            analista: None,
                            base: versao_do_codigo(ambiente),
                        },
                        Some(demanda_do_frontmatter(&lida)),
                    )?;
                    ligacao.demanda = None;
                    (analise.id, true)
                }
                (None, Some(Fluxo::Explicar), _) if args.funcionalidade.is_some() => {
                    let fid = cba_kb::IdFuncionalidade::analisar(
                        args.funcionalidade.as_deref().unwrap_or_default().trim(),
                    )
                    .map_err(|_| {
                        ErroFerramenta::Argumentos(
                            "`funcionalidade`: use F-<SIGLA>-NNN, como F-FAT-001".into(),
                        )
                    })?;
                    let analise = crate::mapear::criar_explicacao(ambiente, &fid)?;
                    (analise.id, true)
                }
                (None, Some(Fluxo::Manual), _) => {
                    let fid = cba_kb::IdFuncionalidade::analisar(
                        args.funcionalidade.as_deref().unwrap_or_default().trim(),
                    )
                    .map_err(|_| {
                        ErroFerramenta::Argumentos(
                            "no fluxo `manual`, informe `funcionalidade`: a funcionalidade \
                             do mapa (F-<SIGLA>-NNN), como F-FAT-001"
                                .into(),
                        )
                    })?;
                    let analise = crate::manual::criar_manual(ambiente, &fid)?;
                    (analise.id, true)
                }
                (None, Some(Fluxo::Historias), entrada) => {
                    if ligacao.reuniao.is_some() {
                        return Err(ErroFerramenta::Argumentos(
                            "`reuniao` só vale no fluxo `reuniao`".into(),
                        ));
                    }
                    let origem = args
                        .origem
                        .as_deref()
                        .map(comum::id_da_analise)
                        .transpose()?;
                    let analise = criar_analise_de_historias(
                        ambiente,
                        origem,
                        ligacao.demanda,
                        entrada,
                        args.titulo,
                    )?;
                    ligacao.demanda = None;
                    (analise.id, true)
                }
                (None, Some(Fluxo::Cenarios), entrada) => {
                    if ligacao.reuniao.is_some() {
                        return Err(ErroFerramenta::Argumentos(
                            "`reuniao` só vale no fluxo `reuniao`".into(),
                        ));
                    }
                    let origem = args
                        .origem
                        .as_deref()
                        .map(comum::id_da_analise)
                        .transpose()?;
                    let analise = criar_analise_de_cenarios(
                        ambiente,
                        origem,
                        ligacao.demanda,
                        entrada,
                        args.titulo,
                    )?;
                    ligacao.demanda = None;
                    (analise.id, true)
                }
                (None, Some(Fluxo::Prototipo), entrada) => {
                    if ligacao.reuniao.is_some() {
                        return Err(ErroFerramenta::Argumentos(
                            "`reuniao` só vale no fluxo `reuniao`".into(),
                        ));
                    }
                    let Some(origem) = args.origem.as_deref() else {
                        return Err(ErroFerramenta::Argumentos(
                            "no fluxo `prototipo`, informe `origem`: a análise A-NNNN, de \
                             preferência aprovada, de onde vêm as regras, as afirmações e os \
                             critérios das telas"
                                .into(),
                        ));
                    };
                    let origem = comum::id_da_analise(origem)?;
                    let pedido = PedidoPrototipo {
                        telas: args.telas,
                        fidelidade: args.fidelidade.unwrap_or_default(),
                        fora: args.fora,
                        // A confirmação do envio é do analista, no app: por aqui, o
                        // modo hospedado é recusado.
                        modo: args.modo.unwrap_or_default(),
                        confirmacao_envio: None,
                    };
                    let analise = criar_analise_de_prototipo(
                        ambiente,
                        origem,
                        ligacao.demanda,
                        pedido,
                        entrada,
                        args.titulo,
                    )?;
                    ligacao.demanda = None;
                    (analise.id, true)
                }
                (None, Some(fluxo), Some(entrada)) => {
                    if ligacao.reuniao.is_some() {
                        return Err(ErroFerramenta::Argumentos(
                            "`reuniao` só vale no fluxo `reuniao`".into(),
                        ));
                    }
                    let demanda = ligacao
                        .demanda
                        .map(|d| demanda_aberta(ambiente, d))
                        .transpose()?;
                    let titulo = match args.titulo {
                        Some(t) if !t.trim().is_empty() => t,
                        _ => titulo_da_entrada(&entrada),
                    };
                    let analise = ambiente.analises.criar_com_demanda(
                        NovaAnalise {
                            fluxo,
                            titulo,
                            entrada,
                            analista: None,
                            base: versao_do_codigo(ambiente),
                        },
                        demanda.as_ref().map(demanda_do_frontmatter),
                    )?;
                    ligacao.demanda = None;
                    (analise.id, true)
                }
                (None, ..) => {
                    return Err(ErroFerramenta::Argumentos(
                        "informe `analise` (o A-NNNN do pedido do analista) ou, para criar \
                         uma análise nova, `fluxo` e `entrada` (no fluxo reuniao, `fluxo` e \
                         `reuniao`)"
                            .into(),
                    ));
                }
            };
            iniciar(ambiente, &id, criada, ligacao, conducao)
        },
    )
}

/// Confere as ligações pedidas para uma análise e devolve a demanda a gravar no
/// frontmatter (só quando a análise ainda não está ligada a ela) e a reunião a
/// gravar no estado (só quando ainda não está gravada).
fn conferir_ligacao(
    ambiente: &Ambiente,
    atual: &Analise,
    ligacao: Ligacao,
) -> Result<(Option<Demanda>, Option<IdReuniao>), ErroFerramenta> {
    let id = &atual.id;
    let mut demanda = ligacao.demanda;
    let mut reuniao = None;
    if let Some(r) = ligacao.reuniao.or(atual.reuniao) {
        if atual.fluxo != Fluxo::Reuniao {
            return Err(ErroFerramenta::Argumentos(format!(
                "`reuniao` só vale no fluxo `reuniao`, e a análise {id} é de {}",
                nome_do_fluxo(atual.fluxo)
            )));
        }
        if let Some(outra) = atual.reuniao
            && outra != r
        {
            return Err(ErroFerramenta::Recusa(format!(
                "a análise {id} é da reunião {outra}, não de {r}; para analisar outra \
                 reunião, crie uma análise nova"
            )));
        }
        let arquivos = reuniao::arquivos_da_reuniao(ambiente, r)?;
        if let Some(d) = demanda
            && d != arquivos.demanda
        {
            return Err(ErroFerramenta::Recusa(format!(
                "a reunião {r} é da demanda {}, não de {d}",
                arquivos.demanda
            )));
        }
        demanda = Some(arquivos.demanda);
        if atual.reuniao.is_none() {
            reuniao = Some(r);
        }
    }
    let Some(d) = demanda else {
        return Ok((None, reuniao));
    };
    let doc: Documento<FrontmatterAnalise> = ambiente
        .kb
        .ler(&atual.documento)
        .map_err(|e| ErroFerramenta::Interno(e.to_string()))?;
    match doc.frontmatter.demanda.id {
        Some(ja) if ja == d => Ok((None, reuniao)),
        Some(ja) => Err(ErroFerramenta::Recusa(format!(
            "a análise {id} já está ligada à demanda {ja}, não a {d}; para mudar a \
             ligação, peça ao analista"
        ))),
        None => {
            let lida = demanda_aberta(ambiente, d)?;
            Ok((Some(demanda_do_frontmatter(&lida)), reuniao))
        }
    }
}

/// Grava a demanda no frontmatter do documento da análise, antes de o documento
/// ser remontado (a montagem mantém o frontmatter).
fn gravar_demanda(
    kb: &cba_kb::Projeto,
    analise: &Analise,
    demanda: Demanda,
) -> Result<(), cba_kb::Erro> {
    let mut doc: Documento<FrontmatterAnalise> = kb.ler(&analise.documento)?;
    doc.frontmatter.demanda = demanda;
    kb.gravar(&analise.documento, &doc)
}

/// Aplica a condução pedida (#371). Com `workflow` num fluxo que tem workflow, a
/// análise passa a ser conduzida por ele. Com `simples` (ou sem `conducao`) numa
/// análise conduzida pelo workflow, a sessão voltou à condução simples: grava o
/// aviso com o motivo (spec, seção 3.8). Devolve o aviso para o agente quando o
/// fluxo não tem workflow.
fn aplicar_conducao(a: &mut Analise, pedido: &PedidoDeConducao) -> Option<String> {
    match pedido.conducao.unwrap_or_default() {
        Conducao::Workflow => {
            if crate::conducao::tipo_do_fluxo(a.fluxo).is_none() {
                return Some(format!(
                    "a {} não tem workflow: siga o roteiro completo abaixo, sem chamar a \
                     ferramenta Workflow",
                    nome_do_fluxo(a.fluxo)
                ));
            }
            a.conducao = Conducao::Workflow;
            a.aviso_conducao = None;
            None
        }
        Conducao::Simples => {
            if a.conducao == Conducao::Workflow {
                a.conducao = Conducao::Simples;
                let motivo = pedido
                    .motivo
                    .as_deref()
                    .map(str::trim)
                    .filter(|m| !m.is_empty())
                    .unwrap_or("a sessão seguiu pelo roteiro completo");
                a.aviso_conducao = Some(motivo.to_owned());
            }
            None
        }
    }
}

fn iniciar(
    ambiente: &Ambiente,
    id: &Id,
    criada: bool,
    ligacao: Ligacao,
    conducao: PedidoDeConducao,
) -> Result<Saida, ErroFerramenta> {
    let atual = ambiente.analises.carregar(id)?;
    if atual.estado == EstadoAnalise::Aprovada {
        return Err(ErroFerramenta::Recusa(format!(
            "a análise {id} já foi aprovada pelo analista e não aceita registros novos. \
             Para continuar o assunto, peça ao analista para criar uma análise nova"
        )));
    }
    let (demanda, reuniao) = conferir_ligacao(ambiente, &atual, ligacao)?;
    if atual.fluxo == Fluxo::Reuniao && atual.reuniao.is_none() && reuniao.is_none() {
        return Err(ErroFerramenta::Argumentos(format!(
            "a análise {id} é de reunião e ainda não está ligada a uma; passe também \
             `reuniao` (R-NNNN)"
        )));
    }
    let mut avisos = Vec::new();
    if let Some(o) = &atual.origem
        && let Ok(origem) = ambiente.analises.carregar(o)
        && origem.estado != EstadoAnalise::Aprovada
    {
        avisos.push(format!(
            "a análise de origem {o} ainda não foi aprovada pelo analista: prefira as \
             afirmações que ele já confirmou e trate as demais como a confirmar"
        ));
    }
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
    // Roteiros revisados e prompts prontos: a cópia editável na base, como os
    // templates. Falhar aqui não impede a análise: vale o padrão embutido.
    // O catálogo de papéis das pessoas (`_templates/papeis.yaml`) vai junto.
    let copiados = Roteiros::copiar_padrao(&ambiente.base).and_then(|mut r| {
        Prompts::copiar_padrao(&ambiente.base).and_then(|p| {
            r.extend(p);
            r.extend(cba_kb::Papeis::copiar_padrao(&ambiente.base)?);
            Ok(r)
        })
    });
    match copiados {
        Ok(criados) if !criados.is_empty() => {
            tracing::info!(
                arquivos = criados.len(),
                "roteiros, prompts e papéis padrão copiados para a base"
            );
        }
        Ok(_) => {}
        Err(e) => avisos.push(format!(
            "os roteiros, prompts e papéis padrão não foram copiados para a base ({e}); vale o \
             padrão embutido"
        )),
    }
    // Documentação publicada (#277): sincroniza em segundo plano e avisa se a
    // cópia está desatualizada. Nunca impede a análise.
    avisos.extend(crate::docs_externos::ao_iniciar_analise(ambiente));
    let falha_da_demanda = RefCell::new(None);
    let registrado = comum::alterar_com(
        ambiente,
        id,
        Mudanca::Estado,
        |a, _| {
            a.iniciar()?;
            if reuniao.is_some() {
                a.reuniao = reuniao;
            }
            Ok(aplicar_conducao(a, &conducao))
        },
        |a| {
            if let Some(d) = demanda
                && let Err(e) = gravar_demanda(&ambiente.kb, a, d)
            {
                *falha_da_demanda.borrow_mut() = Some(e.to_string());
            }
        },
    )?;
    if let Some(e) = falha_da_demanda.into_inner() {
        return Err(ErroFerramenta::Interno(format!(
            "a análise {id} foi iniciada, mas a ligação com a demanda não foi gravada \
             ({e}); chame `iniciar_analise` de novo"
        )));
    }
    avisos.extend(registrado.aviso.clone());
    avisos.extend(registrado.resultado.clone());
    let analise = &registrado.analise;
    let contexto = Contexto::da_analise(ambiente, analise);
    let documento = documento(ambiente, analise);
    let (roteiros, a) = Roteiros::da_base(&ambiente.base);
    let (completo, b) = roteiros.montar(analise, &documento, &contexto.markdown());
    avisos.extend(a.iter().chain(&b).map(aviso_do_roteiro));
    let workflow = (analise.conducao == Conducao::Workflow)
        .then(|| crate::conducao::workflow_do_fluxo(analise.fluxo))
        .flatten();
    let mut roteiro = match &workflow {
        Some(nome) => crate::conducao::roteiro_curto(analise, nome, &documento, &completo),
        None => completo,
    };
    if let Some(chave) = crate::issue_da_demanda::chave_no_rastreador(ambiente, analise) {
        roteiro.push_str(&crate::roteiro::orientacao_da_issue(&analise.id, &chave));
    }
    roteiro.push_str(&crate::roteiro::orientacao_dos_dados(
        analise,
        banco_da_sessao(ambiente, analise).as_ref(),
    ));

    let mut texto = String::new();
    if criada {
        let _ = writeln!(
            texto,
            "Análise {id} criada. Use analise: \"{id}\" em todas as chamadas seguintes e \
             informe o identificador ao analista.\n"
        );
    }
    let registros = afirmacoes_validas(analise).count()
        + analise.perguntas.len()
        + analise.historias.len()
        + analise.tabelas.len()
        + analise.cenarios.len();
    if registros > 0 {
        let _ = writeln!(
            texto,
            "Esta análise já tem registros ({} afirmações, {} perguntas abertas, {} \
             histórias, {} tabelas de decisão, {} cenários). Chame `retomar_analise` para vê-los antes de continuar e não \
             registre de novo o que já está lá.\n",
            afirmacoes_validas(analise).count(),
            perguntas_abertas(analise).count(),
            analise.historias_no_documento().count(),
            analise.tabelas_no_documento().count(),
            analise.cenarios_no_documento().count()
        );
    }
    texto.push_str(&roteiro);
    if workflow.is_none()
        && let Some(lembrete) = lembrete_da_memoria(&roteiro, id)
    {
        texto.push_str(&lembrete);
    }
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
        "reuniao": analise.reuniao,
        "origem": analise.origem,
        "contexto": contexto.json(),
        "roteiro": roteiro,
        "conducao": analise.conducao,
        "workflow": workflow,
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
         registrados até aqui, o que o analista já marcou, a memória da análise (o \
         contexto gravado com `memorizar`: onde parou, o que foi investigado e \
         descartado, onde está o código, o que foi combinado), o roteiro do fluxo e o \
         próximo passo. Use numa sessão nova para continuar de onde a outra parou, sem \
         registrar de novo o que já existe.",
        schema_so_da_analise(),
        |ambiente, args| {
            let args: ArgsAnalise = ler_argumentos(args)?;
            let id = comum::id_da_analise(&args.analise)?;
            let analise = ambiente.analises.carregar(&id)?;
            let memoria = ambiente.analises.memoria(&id)?;
            Ok(retomar(ambiente, &analise, &memoria))
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
        EstadoAfirmacao::Aprofundar { .. } => "o analista pediu para aprofundar",
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
            "Continue de onde a sessão anterior parou, a partir da memória da análise \
             (abaixo): complete as seções que faltam, trate o que o analista marcou e não \
             registre de novo o que já está aqui. Ao terminar, atualize a memória com \
             `memorizar` (`onde_parou`) e chame `concluir_analise` (analise: \"{id}\")."
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

/// A memória da análise no texto da retomada.
fn escrever_memoria(t: &mut String, analise: &Analise, memoria: &Memoria) {
    let _ = writeln!(t, "## Memória da análise");
    let _ = writeln!(t);
    if memoria.vazia() {
        if analise.estado != EstadoAnalise::Aprovada {
            let _ = writeln!(
                t,
                "Nenhuma memória gravada. Grave com `memorizar` (analise: \"{}\") o \
                 contexto que uma conversa nova precisaria: onde parou, o que foi \
                 investigado ou descartado, onde está o código e o que foi combinado com \
                 o analista.",
                analise.id
            );
            let _ = writeln!(t);
        }
        return;
    }
    let _ = writeln!(
        t,
        "Contexto que as conversas anteriores gravaram com `memorizar`. Parta daqui e \
         mantenha-a atualizada."
    );
    let _ = writeln!(t);
    if !memoria.preambulo().is_empty() {
        let _ = writeln!(t, "{}", memoria.preambulo());
        let _ = writeln!(t);
    }
    for (secao, texto) in memoria.secoes() {
        let _ = writeln!(t, "### {} (`{}`)", secao.titulo(), secao.como_str());
        let _ = writeln!(t);
        let _ = writeln!(t, "{texto}");
        let _ = writeln!(t);
    }
}

fn retomar(ambiente: &Ambiente, analise: &Analise, memoria: &Memoria) -> Saida {
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
    // No fluxo `reuniao`, o conteúdo está na ata: vai antes das afirmações.
    let ata = analise
        .reuniao
        .filter(|_| fluxo == Fluxo::Reuniao)
        .and_then(|r| reuniao::ata_para_retomar(ambiente, r));
    if let Some((texto, _)) = &ata {
        t.push_str(texto);
    }
    // No fluxo `revisar`, o conteúdo está no parecer.
    let parecer = (fluxo == Fluxo::Revisar).then(|| parecer::para_retomar(analise));
    if let Some((texto, _)) = &parecer {
        t.push_str(texto);
    }
    // No fluxo `historias`, a origem e as histórias vêm antes.
    let historias = (fluxo == Fluxo::Historias).then(|| crate::historias::para_retomar(analise));
    let cenarios = (fluxo == Fluxo::Cenarios).then(|| crate::cenarios::para_retomar(analise));
    let prototipo = (fluxo == Fluxo::Prototipo).then(|| crate::prototipo::para_retomar(analise));
    if matches!(fluxo, Fluxo::Historias | Fluxo::Cenarios | Fluxo::Prototipo) {
        match &analise.origem {
            Some(o) => {
                let _ = writeln!(
                    t,
                    "Análise de origem: {o} (leia com `retomar_analise`, analise: \"{o}\"; \
                     cite as afirmações pelo número dela).\n"
                );
            }
            None => {
                let _ = writeln!(
                    t,
                    "Sem análise de origem: cite as afirmações como A-NNNN/n e as regras \
                     como RN-<SIGLA>-NNN.\n"
                );
            }
        }
    }
    if let Some((texto, _)) = &historias {
        t.push_str(texto);
    }
    if let Some((texto, _)) = &cenarios {
        t.push_str(texto);
    }
    if let Some((texto, _)) = &prototipo {
        t.push_str(texto);
    }
    if fluxo == Fluxo::Manual {
        t.push_str(&crate::manual::para_retomar(analise));
    }
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
            let estado = match (&a.estado, &a.contestacao) {
                (EstadoAfirmacao::ARevalidar, Some(_)) => {
                    "a revalidar: contestada pela verificação independente"
                }
                (e, _) => rotulo_do_estado(e),
            };
            let _ = writeln!(
                t,
                "{}. [{}; {estado}] {}",
                a.numero,
                rotulo_da_confianca(a.confianca),
                a.texto
            );
            if let Some(c) = &a.contestacao {
                let _ = writeln!(t, "   Contestação ({}): {}", c.origem.rotulo(), c.motivo);
            }
            match &a.estado {
                EstadoAfirmacao::Incorreta { comentario } => {
                    let _ = writeln!(t, "   Comentário do analista: {comentario}");
                }
                EstadoAfirmacao::Aprofundar {
                    pedido: Some(pedido),
                } => {
                    let _ = writeln!(t, "   O que o analista quer aprofundar: {pedido}");
                }
                _ => {}
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
        let tipo = if p.e_decisao() { "decisão, " } else { "" };
        let _ = writeln!(t, "{}. [{tipo}{estado}] {}", p.numero, p.texto);
        for (i, o) in p.opcoes.iter().enumerate() {
            let _ = writeln!(
                t,
                "   - Opção {}: {} — efeito: {}",
                i + 1,
                o.texto,
                o.efeito
            );
        }
        if let Some(r) = &p.recomendacao {
            let _ = writeln!(t, "   - Recomendação: {r}");
        }
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
    escrever_memoria(&mut t, analise, memoria);
    let mut avisos = Vec::new();
    let roteiro = (analise.estado != EstadoAnalise::Aprovada).then(|| {
        let contexto = Contexto::da_analise(ambiente, analise);
        let (roteiros, a) = Roteiros::da_base(&ambiente.base);
        let (roteiro, b) = roteiros.montar(analise, &documento, &contexto.markdown());
        avisos.extend(a.iter().chain(&b).map(aviso_do_roteiro));
        // Conduzida pelo workflow: a sessão que retoma espera o workflow e conclui
        // (#371).
        let mut roteiro = match crate::conducao::workflow_do_fluxo(analise.fluxo)
            .filter(|_| analise.conducao == Conducao::Workflow)
        {
            Some(nome) => crate::conducao::roteiro_curto(analise, &nome, &documento, &roteiro),
            None => roteiro,
        };
        if let Some(chave) = crate::issue_da_demanda::chave_no_rastreador(ambiente, analise) {
            roteiro.push_str(&crate::roteiro::orientacao_da_issue(&analise.id, &chave));
        }
        roteiro.push_str(&crate::roteiro::orientacao_dos_dados(
            analise,
            banco_da_sessao(ambiente, analise).as_ref(),
        ));
        roteiro
    });
    if let Some(r) = &roteiro {
        t.push_str(r);
        if analise.conducao == Conducao::Simples
            && let Some(lembrete) = lembrete_da_memoria(r, &analise.id)
        {
            t.push_str(&lembrete);
        }
    }
    for aviso in &avisos {
        let _ = write!(t, "\nAviso: {aviso}\n");
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
                "contestacao": a.contestacao.as_ref().map(|c| &c.motivo),
                "evidencias": a.evidencias.iter().map(|e| json!({
                    "arquivo": e.arquivo,
                    "linhas": e.linhas,
                    "trecho": e.trecho,
                    "estado": e.estado,
                })).collect::<Vec<_>>(),
            });
            // `estado` vem como objeto ({"estado": "incorreta", "comentario": ...}).
            if let Some(estado) = v["estado"].get("estado").cloned() {
                for campo in ["comentario", "pedido"] {
                    if let Some(c) = v["estado"].get(campo).cloned() {
                        v[campo] = c;
                    }
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
            let mut v = json!({
                "numero": p.numero,
                "texto": p.texto,
                "afirmacao": p.afirmacao,
                "estado": estado,
                "resposta": resposta,
            });
            if p.e_decisao() {
                v["tipo"] = json!("decisao");
                v["opcoes"] = json!(p.opcoes);
                v["recomendacao"] = json!(p.recomendacao);
            }
            v
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
            "reuniao": analise.reuniao,
            "ata": ata.map(|(_, dados)| dados),
            "parecer": parecer.map(|(_, dados)| dados),
            "origem": analise.origem,
            "historias": historias.map(|(_, dados)| dados),
            "cenarios": cenarios.map(|(_, dados)| dados),
            "prototipo": prototipo.map(|(_, dados)| dados),
            "afirmacoes": afirmacoes,
            "perguntas": perguntas,
            "sugestoes": sugestoes,
            "termos": termos,
            "memoria": {
                "arquivo": ambiente.kb.na_base(&cba_core::analises::Analises::arquivo_de_memoria(&analise.id)),
                "vazia": memoria.vazia(),
                "preambulo": memoria.preambulo(),
                "secoes": memoria
                    .secoes()
                    .map(|(s, texto)| json!({ "secao": s, "titulo": s.titulo(), "texto": texto }))
                    .collect::<Vec<_>>(),
            },
            "secoes_obrigatorias_pendentes": secoes_obrigatorias(fluxo)
                .iter()
                .filter(|s| !afirmacoes_validas(analise).any(|a| a.secao == **s))
                .collect::<Vec<_>>(),
            "proximo_passo": proximo_passo(analise),
            "roteiro": roteiro,
            "avisos": avisos,
        }),
    )
}

/// Aviso do roteiro da base, para o agente repassar ao analista.
fn aviso_do_roteiro(aviso: &crate::editavel::Aviso) -> String {
    format!("roteiro da base: {aviso}")
}

// ---------------------------------------------------------------------------
// concluir_analise

/// Diagramas que o documento negocial (fluxo `explicar`, #120) ainda não tem: o
/// fluxograma do processo e o modelo de dados conceitual. Viram pendências da
/// conclusão, sem impedi-la (nem toda funcionalidade grava dados).
fn diagramas_faltando(analise: &Analise) -> Vec<String> {
    use cba_core::diagramas::TipoDiagrama;
    if analise.fluxo != Fluxo::Explicar {
        return Vec::new();
    }
    let tem = |tipo: TipoDiagrama| analise.diagramas.iter().any(|d| d.tipo == tipo);
    let mut faltam = Vec::new();
    if !tem(TipoDiagrama::Fluxo) {
        faltam.push(format!(
            "o documento não tem o fluxo do processo: registre-o com `registrar_diagrama` \
             (tipo `fluxo`, secao `{}`), com as raias por ator",
            cba_docs::FLUXO_PROCESSO
        ));
    }
    if !tem(TipoDiagrama::Dados) {
        faltam.push(format!(
            "o documento não tem o modelo de dados conceitual: registre-o com \
             `registrar_diagrama` (tipo `dados`, secao `{}`), se a funcionalidade lê ou \
             grava dados",
            cba_docs::MODELO_DADOS
        ));
    }
    faltam
}

/// Seção do que muda na análise de impacto, onde uma hipótese não vira escopo.
const SECAO_DO_ESCOPO: &str = "funcionalidades";

/// Palavras que indicam hipótese numa afirmação (#452), comparadas sem acento e
/// sem diferenciar maiúsculas, como palavras inteiras (o hífen não separa:
/// "aplica-se" não conta como "se").
const PALAVRAS_DE_HIPOTESE: [&str; 16] = [
    "se",
    "pode",
    "podem",
    "poderia",
    "poderiam",
    "possivelmente",
    "talvez",
    "eventualmente",
    "opcional",
    "opcionalmente",
    "preventiva",
    "preventivo",
    "preventivas",
    "preventivos",
    "preventivamente",
    "hipotese",
];

/// Expressões que indicam uma regra nova, sem acento.
const EXPRESSOES_DE_REGRA_NOVA: [&str; 6] = [
    "regra nova",
    "nova regra",
    "nova validacao",
    "passa a",
    "passar a",
    "passara a",
];

/// Se o texto indica hipótese ou traz uma regra nova (veja as listas acima).
pub(crate) fn indica_hipotese(texto: &str) -> bool {
    let palavras: Vec<String> = texto
        .split(|c: char| !(c.is_alphanumeric() || c == '-'))
        .filter(|p| !p.is_empty())
        .map(|p| {
            p.split('-')
                .map(cba_kb::normalizar_texto)
                .collect::<Vec<_>>()
                .join("-")
        })
        .collect();
    if palavras
        .iter()
        .any(|p| PALAVRAS_DE_HIPOTESE.contains(&p.as_str()))
    {
        return true;
    }
    let frase = format!(" {} ", palavras.join(" "));
    EXPRESSOES_DE_REGRA_NOVA
        .iter()
        .any(|e| frase.contains(&format!(" {e} ")))
}

/// Afirmações `inferido` de `funcionalidades` que indicam hipótese ou regra nova
/// e não têm pergunta nem decisão vinculada (#452): sem as evidências, o leitor
/// não distingue a hipótese de um fato, e ela entra no escopo. Viram pendência.
fn hipoteses_no_escopo(analise: &Analise) -> Vec<String> {
    if analise.fluxo != Fluxo::Impacto {
        return Vec::new();
    }
    let vinculada = |numero: u32| {
        analise
            .perguntas
            .iter()
            .any(|p| p.afirmacao == Some(numero) && p.estado != EstadoPergunta::Descartada)
    };
    afirmacoes_validas(analise)
        .filter(|a| a.secao == SECAO_DO_ESCOPO && a.confianca == Confianca::Inferido)
        .filter(|a| indica_hipotese(&a.texto) && !vinculada(a.numero))
        .map(|a| {
            format!(
                "afirmação {}: mudança em `{SECAO_DO_ESCOPO}` que depende de hipótese não \
                 verificada ou traz uma regra nova, sem pergunta nem decisão vinculada. \
                 Verifique no código; se não der, registre com `perguntar` (afirmacao: {}) a \
                 dúvida, com o que confirmaria a hipótese, ou a decisão (tipo `decisao`, com \
                 as opções), e marque a mudança como dependente dela",
                a.numero, a.numero
            )
        })
        .collect()
}

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

fn chave(e: cba_core::analise::TrechoCitado<'_>) -> Chave {
    (
        e.arquivo.to_string(),
        e.linhas.inicio,
        e.linhas.fim,
        e.hash.to_owned(),
    )
}

/// Confere, fora da trava, se cada evidência válida das afirmações ainda bate com o
/// código (o hash do intervalo). Arquivos são lidos uma vez.
fn conferir_evidencias(ambiente: &Ambiente, analise: &Analise) -> HashMap<Chave, bool> {
    let mut textos: HashMap<String, Option<String>> = HashMap::new();
    let mut resultado = HashMap::new();
    let dos_achados = analise
        .parecer
        .iter()
        .flat_map(|p| p.ativos())
        .flat_map(|a| &a.evidencias);
    for e in afirmacoes_validas(analise)
        .flat_map(|a| &a.evidencias)
        .chain(dos_achados)
        .filter(|e| e.estado == EstadoEvidencia::Valida)
    {
        let arquivo = e.arquivo.to_string();
        let texto = textos.entry(arquivo.clone()).or_insert_with(|| {
            ArquivoDoProjeto::abrir(&ambiente.escopo, &arquivo)
                .ok()
                .map(|a| a.texto)
        });
        let confere = texto
            .as_deref()
            .and_then(|t| hash_das_linhas(t, e.linhas))
            .is_some_and(|h| h == e.hash);
        resultado.insert(chave(e.into()), confere);
    }
    resultado
}

/// No fluxo `reuniao`, o conteúdo está na ata: ela precisa ter ao menos um item.
fn conferir_ata(ambiente: &Ambiente, analise: &Analise) -> Result<(), ErroFerramenta> {
    let id = &analise.id;
    let Some(r) = analise.reuniao else {
        return Err(ErroFerramenta::Recusa(format!(
            "a análise {id} é de reunião e não está ligada a uma; chame `iniciar_analise` \
             com `reuniao` (R-NNNN)"
        )));
    };
    let (ata, _) = EstadoAta::ler(&ambiente.kb, r)?;
    if ata.itens.is_empty() {
        return Err(ErroFerramenta::Recusa(format!(
            "a ata da reunião {r} ainda não tem itens. Registre as decisões, necessidades, \
             regras, pendências, dúvidas e próximos passos com `registrar_item_ata` e chame \
             `concluir_analise` de novo"
        )));
    }
    Ok(())
}

/// No fluxo `historias`, o conteúdo são as histórias: ao menos uma, e cada uma com
/// ao menos um critério de aceite.
fn conferir_historias(analise: &Analise) -> Result<(), ErroFerramenta> {
    let id = &analise.id;
    if analise.historias_no_documento().next().is_none() {
        return Err(ErroFerramenta::Recusa(format!(
            "a análise {id} ainda não tem histórias. Registre cada história com \
             `registrar_historia` (com os critérios de aceite) e chame `concluir_analise` de \
             novo"
        )));
    }
    let sem: Vec<String> = analise
        .historias_no_documento()
        .filter(|h| h.criterios_validos().next().is_none())
        .map(|h| h.id.to_string())
        .collect();
    if !sem.is_empty() {
        return Err(ErroFerramenta::Recusa(format!(
            "história(s) sem critério de aceite: {}. Registre ao menos um critério em cada \
             uma com `registrar_criterio` e chame `concluir_analise` de novo",
            sem.join(", ")
        )));
    }
    Ok(())
}

/// No fluxo `cenarios`, o conteúdo são os cenários: ao menos um, e cada coluna
/// possível das tabelas de decisão com um cenário.
fn conferir_cenarios(analise: &Analise) -> Result<(), ErroFerramenta> {
    let id = &analise.id;
    if analise.cenarios_no_documento().next().is_none() {
        return Err(ErroFerramenta::Recusa(format!(
            "a análise {id} ainda não tem cenários de teste. Registre as tabelas de decisão \
             com `registrar_tabela_decisao` e os cenários com `registrar_cenario`, e chame \
             `concluir_analise` de novo"
        )));
    }
    let faltam = crate::cenarios::colunas_sem_cenario(analise);
    if !faltam.is_empty() {
        return Err(ErroFerramenta::Recusa(format!(
            "coluna(s) de tabela de decisão sem cenário: {}. Registre um cenário para cada \
             uma com `registrar_cenario` (tecnica `tabela_de_decisao`, `tabela`: \"TD-n/Rn\") \
             e chame `concluir_analise` de novo",
            faltam.join(", ")
        )));
    }
    Ok(())
}

fn concluir(ambiente: &Ambiente, id: &Id) -> Result<Saida, ErroFerramenta> {
    let atual = ambiente.analises.carregar(id)?;
    let fluxo = atual.fluxo;
    if atual.estado == EstadoAnalise::Aprovada {
        return Err(ErroFerramenta::Recusa(format!(
            "a análise {id} já foi aprovada pelo analista; não há o que concluir"
        )));
    }
    let mut pendencias = Vec::new();
    if fluxo == Fluxo::Reuniao {
        conferir_ata(ambiente, &atual)?;
    } else if fluxo == Fluxo::Revisar {
        // O conteúdo da revisão está no parecer: o veredito é obrigatório.
        pendencias.extend(parecer::conferir_para_concluir(&atual)?);
    } else if fluxo == Fluxo::Historias {
        conferir_historias(&atual)?;
    } else if fluxo == Fluxo::Cenarios {
        conferir_cenarios(&atual)?;
    } else if fluxo == Fluxo::Manual {
        crate::manual::conferir_para_concluir(&atual)?;
    } else if fluxo == Fluxo::Prototipo {
        crate::prototipo::conferir_para_concluir(&ambiente.kb, &atual)?;
    } else if afirmacoes_validas(&atual).next().is_none() {
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

    for h in analise.historias_no_documento() {
        if let Some(p) = &h.quebra_pedida {
            pendencias.push(if p.is_empty() {
                format!("{}: o analista pediu a quebra em fatias", h.id)
            } else {
                format!("{}: o analista pediu a quebra em fatias: {p}", h.id)
            });
        }
        for alerta in h.alertas_invest() {
            pendencias.push(format!(
                "{}: não atende ao INVEST ({}): {}",
                h.id,
                alerta.nome(),
                alerta.motivo
            ));
        }
    }
    for c in analise.cenarios_no_documento() {
        if let Some(p) = &c.pergunta {
            pendencias.push(format!(
                "{}: a confirmar, depende da pergunta {p}{}",
                c.id,
                c.limite
                    .as_ref()
                    .and_then(|l| l.configuracao.as_ref())
                    .map(|conf| format!(" (limite em `{conf}`)"))
                    .unwrap_or_default()
            ));
        }
    }
    for a in afirmacoes_validas(analise) {
        if let Some(c) = &a.contestacao
            && a.estado == EstadoAfirmacao::ARevalidar
        {
            pendencias.push(format!(
                "afirmação {}: contestada pela {}: {}; o analista decide na revisão",
                a.numero,
                c.origem.rotulo(),
                c.motivo
            ));
        } else if a.estado == EstadoAfirmacao::ARevalidar {
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
        if let EstadoAfirmacao::Aprofundar { pedido } = &a.estado {
            pendencias.push(match pedido {
                Some(p) => format!(
                    "afirmação {}: o analista pediu para aprofundar: {p}",
                    a.numero
                ),
                None => format!("afirmação {}: o analista pediu para aprofundar", a.numero),
            });
        }
    }
    for achado in analise
        .parecer
        .iter()
        .flat_map(|p| p.achados.iter())
        .filter(|a| a.a_revalidar())
    {
        if let Some(c) = &achado.contestacao {
            pendencias.push(format!(
                "achado {}: contestado pela {}: {}; o analista decide na revisão",
                achado.numero,
                c.origem.rotulo(),
                c.motivo
            ));
        }
    }
    pendencias.extend(diagramas_faltando(analise));
    pendencias.extend(hipoteses_no_escopo(analise));
    let abertas: Vec<&str> = perguntas_abertas(analise)
        .filter(|p| !p.e_decisao())
        .map(|p| p.texto.as_str())
        .collect();
    let decisoes: Vec<&cba_core::analise::Pergunta> = perguntas_abertas(analise)
        .filter(|p| p.e_decisao())
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
                "primeiro as decisões para o negócio, com as opções; depois a demanda como entendida, a abrangência, os principais riscos e as perguntas para o solicitante",
            Fluxo::Reuniao =>
                "o objetivo da reunião, as decisões, as pendências com responsável e prazo e as dúvidas em aberto",
            Fluxo::Revisar =>
                "o veredito, as contradições com a base ou o código, os principais achados e as perguntas para o autor",
            Fluxo::Historias =>
                "as histórias (HU-NNNN e título) com o número de critérios, os critérios a confirmar, os alertas INVEST e as quebras propostas",
            Fluxo::Cenarios =>
                "as tabelas de decisão, o número de cenários por técnica, os limites lidos do código e os cenários a confirmar",
            Fluxo::Manual =>
                "o objetivo, os passos com a tela sugerida para o print de cada um e as perguntas em aberto",
            Fluxo::Prototipo =>
                "as telas (TL-n, nome, nova ou alterada) com o número de elementos, os elementos e as telas a confirmar",
        }
    );
    if !decisoes.is_empty() {
        let _ = writeln!(t, "\nDecisões para o negócio ({}):", decisoes.len());
        for (i, d) in decisoes.iter().enumerate() {
            let _ = writeln!(t, "{}. {}", i + 1, d.texto);
            for (j, o) in d.opcoes.iter().enumerate() {
                let _ = writeln!(
                    t,
                    "   - Opção {}: {} — efeito: {}",
                    j + 1,
                    o.texto,
                    o.efeito
                );
            }
            if let Some(r) = &d.recomendacao {
                let _ = writeln!(t, "   - Recomendação: {r}");
            }
        }
    }
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
            "decisoes_abertas": decisoes.iter().map(|d| json!({
                "numero": d.numero,
                "texto": d.texto,
                "opcoes": d.opcoes,
                "recomendacao": d.recomendacao,
            })).collect::<Vec<Value>>(),
            "pendencias": pendencias,
        }),
    ))
}

/// O banco da sessão, quando é o ligado nesta análise (#440).
fn banco_da_sessao(
    ambiente: &Ambiente,
    analise: &cba_core::analise::Analise,
) -> Option<cba_core::banco::OrigemBanco> {
    let origem = ambiente.banco.as_ref()?.origem();
    (crate::consultar_dados::conexao_ligada(ambiente, analise).as_deref()
        == Some(origem.conexao.as_str()))
    .then_some(origem)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hipotese_e_regra_nova_no_texto() {
        for texto in [
            "Validar o status antes de gravar, que pode estar inconsistente.",
            "Opcional, preventiva: bloquear a gravação.",
            "Se houver faturas antigas, migrar o status.",
            "Possivelmente o lote também precisa mudar.",
            "A tela PASSA A exigir o motivo do cancelamento.",
            "Criar uma nova regra para o estorno.",
            "Uma HIPÓTESE: o lote regrava o status.",
        ] {
            assert!(indica_hipotese(texto), "{texto}");
        }
        for texto in [
            "O limite de parcelas muda de 6 para 3 no parâmetro MAX_PARCELAS.",
            "Aplica-se a todas as faturas em aberto.",
            "Remover a trava de cancelamento em db/03_pkg_cobranca.sql.",
            "Passagem do status para cancelada.",
        ] {
            assert!(!indica_hipotese(texto), "{texto}");
        }
    }

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
