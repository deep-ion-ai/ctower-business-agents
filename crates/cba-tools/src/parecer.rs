//! Parecer de revisão de um requisito ou história (fluxo `revisar`, #116): as
//! ferramentas de registro e a montagem das seções do documento.
//!
//! - [`verificar_termos_vagos`]: os termos da lista de referência
//!   ([`cba_core::qualidade::TERMOS_VAGOS`]) no texto revisado e se já há achado
//!   de ambiguidade para cada um. Só de leitura.
//! - [`registrar_achado`]: um achado com a característica da ISO/IEC/IEEE 29148
//!   violada, a severidade, o trecho (conferido no texto revisado), o problema, a
//!   reescrita sugerida, as regras da base (`RN-…`, conferidas na base) e as
//!   evidências do código (conferidas como as do `afirmar`). Achado de correção
//!   exige evidência.
//! - [`registrar_invest`]: a verificação INVEST de uma história, os seis critérios.
//! - [`registrar_veredito`]: pronta, pronta com ressalvas ou precisa de retrabalho;
//!   recusa um veredito mais brando do que os achados indicam.
//!
//! As perguntas para o autor são as do `perguntar`. O documento é remontado a
//! cada registro: [`textos`] devolve o veredito, os achados por severidade e a
//! tabela INVEST como textos das seções do template `parecer-revisao`.

use std::fmt::Write as _;

use cba_core::ambiente::Ambiente;
use cba_core::analise::{Analise, ErroModelo, Fluxo};
use cba_core::eventos::Mudanca;
use cba_core::parecer::{Achado, AvaliacaoInvest, EstadoAchado, NovoAchado, Parecer, Veredito};
use cba_core::qualidade::{
    self, Caracteristica, CriterioInvest, ResultadoInvest, Severidade, contem_trecho,
};
use cba_docs::Texto;
use cba_kb::frontmatter::{Demanda, FrontmatterRegra, StatusRegra};
use cba_kb::{CaminhoRelativo, Id, Serie};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::comum;
use crate::documento::citacao;
use crate::evidencias::{self, EvidenciaCitada};
use crate::registro::{ErroFerramenta, Ferramenta, Saida, ler_argumentos};

/// Seções do parecer montadas a partir do parecer do estado (não recebem
/// afirmações).
pub const SECOES: &[&str] = &["veredito", "achados", "invest"];

/// Regras citadas no máximo por achado.
const LIMITE_DE_REGRAS: u64 = 10;

// ---------------------------------------------------------------------------
// Montagem

/// Linha da fonte do texto revisado, quando ele veio do rastreador: a chave da
/// issue, com o link.
pub fn fonte(demanda: &Demanda) -> Option<String> {
    let url = demanda.origem.trim();
    let url = (url.starts_with("https://") || url.starts_with("http://")).then_some(url);
    match (demanda.chave.as_deref(), url) {
        (Some(chave), Some(url)) => Some(format!("Fonte: [{chave}]({url})")),
        (Some(chave), None) => Some(format!("Fonte: {chave}")),
        (None, Some(url)) => Some(format!("Fonte: <{url}>")),
        (None, None) => None,
    }
}

/// Link relativo do documento da análise para o de uma regra.
fn link_da_regra(documento: &CaminhoRelativo, regra: &Id) -> String {
    let subir = documento.partes().count().saturating_sub(1);
    format!(
        "[{regra}]({}{})",
        "../".repeat(subir),
        regra.arquivo().as_str()
    )
}

/// Trecho entre aspas, numa linha só.
fn entre_aspas(trecho: &str) -> String {
    format!(
        "“{}”",
        trecho.split_whitespace().collect::<Vec<_>>().join(" ")
    )
}

/// Contagem dos achados ativos por característica ("1 de correção · 3 de
/// ambiguidade"), na ordem de [`Caracteristica::TODAS`].
pub fn contagem(parecer: &Parecer) -> Vec<(Caracteristica, usize)> {
    Caracteristica::TODAS
        .into_iter()
        .map(|c| {
            (
                c,
                parecer.ativos().filter(|a| a.caracteristica == c).count(),
            )
        })
        .filter(|(_, n)| *n > 0)
        .collect()
}

fn texto_do_achado(analise: &Analise, a: &Achado) -> String {
    let mut t = format!("**Achado {} · {}**", a.numero, a.caracteristica.rotulo());
    if !a.trecho.is_empty() {
        let _ = write!(t, " · {}", entre_aspas(&a.trecho));
    }
    let _ = write!(t, ": {}", a.problema.trim());
    if !a.regras.is_empty() {
        let regras: Vec<String> = a
            .regras
            .iter()
            .map(|r| link_da_regra(&analise.documento, r))
            .collect();
        let _ = write!(t, " Regras da base: {}.", regras.join(", "));
    }
    t
}

/// Textos das seções do parecer (`requisito`, `veredito`, `achados` e `invest`)
/// a partir do estado. A entrada (o texto revisado) entra pela seção da entrada.
pub fn textos(analise: &Analise) -> Vec<Texto> {
    let mut textos = Vec::new();
    let vagos = qualidade::termos_vagos(&analise.entrada);
    if !vagos.is_empty() {
        let mut lista: Vec<String> = Vec::new();
        for o in &vagos {
            let t = entre_aspas(&o.trecho);
            if !lista.contains(&t) {
                lista.push(t);
            }
        }
        textos.push(Texto::novo(
            "requisito",
            format!(
                "_Termos vagos da lista de referência no texto: {}._",
                lista.join(", ")
            ),
        ));
    }
    let Some(parecer) = &analise.parecer else {
        return textos;
    };
    let contagem: Vec<String> = contagem(parecer)
        .into_iter()
        .map(|(c, n)| format!("{n} de {}", c.rotulo().to_lowercase()))
        .collect();
    if let Some(v) = &parecer.veredito {
        textos.push(Texto::novo(
            "veredito",
            format!("**{}.** {}", v.veredito.rotulo(), v.justificativa.trim()),
        ));
    }
    if !contagem.is_empty() {
        textos.push(Texto::novo(
            "veredito",
            format!("Achados: {}.", contagem.join(" · ")),
        ));
    }
    for severidade in Severidade::TODAS {
        let daqui: Vec<&Achado> = parecer
            .ativos()
            .filter(|a| a.severidade == severidade)
            .collect();
        if daqui.is_empty() {
            continue;
        }
        textos.push(Texto::novo(
            "achados",
            format!("### Severidade {}", severidade.rotulo()),
        ));
        for a in daqui {
            let mut texto = Texto::novo("achados", texto_do_achado(analise, a));
            texto.citacoes = a.evidencias.iter().map(citacao).collect();
            textos.push(texto);
            if let Some(r) = &a.reescrita {
                let mut linhas = r.trim().lines();
                let mut q = format!("> **Reescrita sugerida:** {}", linhas.next().unwrap_or(""));
                for l in linhas {
                    q.push_str("\n> ");
                    q.push_str(l);
                }
                textos.push(Texto::novo("achados", q));
            }
        }
    }
    if !parecer.invest.is_empty() {
        let mut tabela = String::from("| Critério | Resultado | Justificativa |\n|---|---|---|");
        for i in &parecer.invest {
            let _ = write!(
                tabela,
                "\n| {} ({}) | {} | {} |",
                i.criterio.rotulo(),
                i.criterio.letra(),
                i.resultado.rotulo(),
                i.justificativa
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
                    .replace('|', "\\|")
            );
        }
        textos.push(Texto::novo("invest", tabela));
    }
    textos
}

/// O parecer para `retomar_analise`: o texto para o agente e os dados.
pub(crate) fn para_retomar(analise: &Analise) -> (String, Value) {
    let parecer = analise.parecer.clone().unwrap_or_default();
    let mut t = String::from("## Parecer de revisão\n\n");
    match &parecer.veredito {
        Some(v) => {
            let _ = writeln!(
                t,
                "Veredito: {} ({}).\n",
                nome_do_veredito(v.veredito),
                v.justificativa
            );
        }
        None => t.push_str("Veredito ainda não registrado.\n\n"),
    }
    if parecer.achados.is_empty() {
        t.push_str("Nenhum achado registrado.\n");
    }
    for a in &parecer.achados {
        let estado = match &a.estado {
            EstadoAchado::Registrado => String::new(),
            EstadoAchado::Descartado { motivo } => format!(
                " (descartado pelo analista{})",
                motivo
                    .as_deref()
                    .map(|m| format!(": {m}"))
                    .unwrap_or_default()
            ),
        };
        let _ = writeln!(
            t,
            "{}. [{}; {}] {}{}{estado}",
            a.numero,
            a.caracteristica.como_str(),
            a.severidade.rotulo(),
            if a.trecho.is_empty() {
                String::new()
            } else {
                format!("{} ", entre_aspas(&a.trecho))
            },
            a.problema
        );
    }
    if !parecer.invest.is_empty() {
        let _ = writeln!(
            t,
            "\nINVEST registrada ({} critérios).",
            parecer.invest.len()
        );
    }
    let vagos = vagos_pendentes(analise);
    if !vagos.is_empty() {
        let _ = writeln!(t, "\nTermos vagos sem achado: {}.", vagos.join(", "));
    }
    t.push('\n');
    (
        t,
        json!({
            "veredito": parecer.veredito.as_ref().map(|v| v.veredito),
            "veredito_sugerido": parecer.veredito_sugerido(),
            "achados": parecer.achados.iter().map(|a| json!({
                "numero": a.numero,
                "caracteristica": a.caracteristica,
                "severidade": a.severidade,
                "trecho": a.trecho,
                "estado": if a.ativo() { "registrado" } else { "descartado" },
            })).collect::<Vec<_>>(),
            "invest": !parecer.invest.is_empty(),
            "termos_vagos_sem_achado": vagos,
        }),
    )
}

// ---------------------------------------------------------------------------
// Conferências comuns

fn exigir_revisao(analise: &Analise) -> Result<(), ErroFerramenta> {
    if analise.fluxo != Fluxo::Revisar {
        return Err(comum::recusa(ErroModelo::ForaDaRevisao(analise.id.clone())));
    }
    Ok(())
}

/// Termos vagos do texto revisado e, para cada um, se algum achado de
/// ambiguidade ativo cita o trecho.
fn termos_vagos(analise: &Analise) -> Vec<(qualidade::Ocorrencia, bool)> {
    let achados: Vec<&Achado> = analise
        .parecer
        .iter()
        .flat_map(|p| p.ativos())
        .filter(|a| {
            matches!(
                a.caracteristica,
                Caracteristica::Ambiguidade | Caracteristica::Verificabilidade
            )
        })
        .collect();
    qualidade::termos_vagos(&analise.entrada)
        .into_iter()
        .map(|o| {
            let coberto = achados.iter().any(|a| {
                contem_trecho(&a.trecho, &o.trecho)
                    || a.problema.to_lowercase().contains(&o.trecho.to_lowercase())
            });
            (o, coberto)
        })
        .collect()
}

/// Termos vagos ainda sem achado, sem repetição, para a resposta ao agente.
fn vagos_pendentes(analise: &Analise) -> Vec<String> {
    let mut lista: Vec<String> = Vec::new();
    for (o, coberto) in termos_vagos(analise) {
        if !coberto && !lista.contains(&o.trecho) {
            lista.push(o.trecho);
        }
    }
    lista
}

/// Pendências do parecer para `concluir_analise`: o veredito tem de existir e
/// não pode ser mais brando que o indicado pelos achados (recusa); termos vagos
/// sem achado viram aviso.
pub(crate) fn conferir_para_concluir(analise: &Analise) -> Result<Vec<String>, ErroFerramenta> {
    let id = &analise.id;
    let parecer = analise.parecer.clone().unwrap_or_default();
    let Some(v) = &parecer.veredito else {
        return Err(ErroFerramenta::Recusa(format!(
            "o parecer da análise {id} ainda não tem veredito. Registre os achados com \
             `registrar_achado` e o veredito com `registrar_veredito` (pronta, \
             pronta_com_ressalvas ou precisa_de_retrabalho) e chame `concluir_analise` \
             de novo"
        )));
    };
    let sugerido = parecer.veredito_sugerido();
    if v.veredito < sugerido {
        return Err(ErroFerramenta::Recusa(format!(
            "o veredito registrado ({}) é mais brando do que os achados indicam ({}): \
             registre o veredito de novo com `registrar_veredito`",
            nome_do_veredito(v.veredito),
            nome_do_veredito(sugerido)
        )));
    }
    let mut pendencias = Vec::new();
    let vagos = vagos_pendentes(analise);
    if !vagos.is_empty() {
        pendencias.push(format!(
            "termos vagos da lista de referência sem achado de ambiguidade: {}",
            vagos
                .iter()
                .map(|v| entre_aspas(v))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    Ok(pendencias)
}

fn nome_do_veredito(v: Veredito) -> &'static str {
    match v {
        Veredito::Pronta => "pronta",
        Veredito::ProntaComRessalvas => "pronta_com_ressalvas",
        Veredito::PrecisaDeRetrabalho => "precisa_de_retrabalho",
    }
}

/// Regras citadas, conferidas na base: existem e não são obsoletas. Devolve os
/// identificadores e os avisos (regra a revalidar).
fn conferir_regras(
    ambiente: &Ambiente,
    citadas: &[String],
) -> Result<(Vec<Id>, Vec<String>), ErroFerramenta> {
    let mut ids = Vec::new();
    let mut avisos = Vec::new();
    for texto in citadas {
        let id = Id::analisar(texto.trim())
            .ok()
            .filter(|id| matches!(id.serie(), Serie::Regra { .. }))
            .ok_or_else(|| {
                ErroFerramenta::Argumentos(format!(
                    "`regras`: `{texto}` não é o identificador de uma regra; use a forma \
                     RN-<SIGLA>-NNN, como RN-FAT-012"
                ))
            })?;
        let regra = ambiente
            .kb
            .ler::<FrontmatterRegra>(&id.arquivo())
            .map_err(|_| {
                ErroFerramenta::Recusa(format!(
                    "a regra {id} não existe na base do projeto. Cite só regras que o \
                     contexto ou `conhecimento` mostraram; o que o código faz e a base \
                     ainda não tem vai nas evidências"
                ))
            })?;
        match regra.frontmatter.status {
            StatusRegra::Obsoleta => {
                return Err(ErroFerramenta::Recusa(format!(
                    "a regra {id} está obsoleta na base e não vale mais; confira o código \
                     atual e cite a evidência"
                )));
            }
            StatusRegra::ARevalidar => avisos.push(format!(
                "a regra {id} está a revalidar: o código mudou desde a confirmação; \
                 confira a evidência"
            )),
            StatusRegra::Confirmada => {}
        }
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    Ok((ids, avisos))
}

// ---------------------------------------------------------------------------
// verificar_termos_vagos

#[derive(Debug, Deserialize)]
struct ArgsAnalise {
    analise: String,
}

/// `verificar_termos_vagos`: termos da lista de referência no texto revisado.
pub fn verificar_termos_vagos() -> Ferramenta {
    Ferramenta::nova(
        "verificar_termos_vagos",
        "Verificar termos vagos",
        "Procura no texto revisado (a entrada da análise do fluxo `revisar`) os termos \
         vagos da lista de referência (\"rápido\", \"adequado\", \"etc.\", \
         \"normalmente\", \"e/ou\" e outros), sem diferenciar maiúsculas nem acentos. \
         Devolve cada ocorrência com o trecho como está no texto, o motivo e se já há \
         achado de ambiguidade que o cita. Registre um achado de `ambiguidade` (ou de \
         `verificabilidade`) para cada termo, com a reescrita; `concluir_analise` \
         avisa dos que ficarem sem achado.",
        json!({
            "type": "object",
            "properties": { "analise": comum::schema_da_analise() },
            "required": ["analise"],
            "additionalProperties": false
        }),
        |ambiente, args| {
            let args: ArgsAnalise = ler_argumentos(args)?;
            let id = comum::id_da_analise(&args.analise)?;
            let analise = ambiente.analises.carregar(&id)?;
            exigir_revisao(&analise)?;
            let ocorrencias: Vec<Value> = termos_vagos(&analise)
                .into_iter()
                .map(|(o, coberto)| {
                    json!({
                        "termo": o.termo,
                        "trecho": o.trecho,
                        "posicao": o.inicio,
                        "motivo": o.motivo,
                        "com_achado": coberto,
                    })
                })
                .collect();
            Ok(Saida::json(json!({
                "analise": id,
                "termos_vagos": ocorrencias,
                "lista_de_referencia": qualidade::TERMOS_VAGOS
                    .iter()
                    .map(|t| t.termo)
                    .collect::<Vec<_>>(),
            })))
        },
    )
    .somente_leitura()
}

// ---------------------------------------------------------------------------
// registrar_achado

#[derive(Debug, Deserialize)]
struct ArgsAchado {
    analise: String,
    caracteristica: Caracteristica,
    severidade: Severidade,
    #[serde(default)]
    trecho: Option<String>,
    problema: String,
    #[serde(default)]
    reescrita: Option<String>,
    #[serde(default)]
    regras: Vec<String>,
    #[serde(default)]
    evidencias: Vec<EvidenciaCitada>,
}

fn descricao_das_caracteristicas() -> String {
    Caracteristica::TODAS
        .iter()
        .map(|c| format!("{} ({})", c.como_str(), c.descricao()))
        .collect::<Vec<_>>()
        .join("; ")
}

/// `registrar_achado`: um problema do texto revisado, com a característica, o
/// trecho e a reescrita.
pub fn registrar_achado() -> Ferramenta {
    let caracteristicas = descricao_das_caracteristicas();
    Ferramenta::nova(
        "registrar_achado",
        "Registrar achado da revisão",
        "Registra um achado no parecer de revisão (fluxo `revisar`): a característica \
         da ISO/IEC/IEEE 29148 violada, a severidade, o trecho do texto revisado, o \
         problema e a reescrita sugerida. O trecho é conferido no texto revisado: \
         copie-o como está (só a completude, que aponta o que falta, pode vir sem \
         trecho). Achado de `correcao` (contradição com regra confirmada da base ou com \
         o código) exige evidência do código, conferida como no `afirmar`; cite também \
         em `regras` a regra da base (RN-…) contrariada, quando houver. Um achado por \
         problema.",
        json!({
            "type": "object",
            "properties": {
                "analise": comum::schema_da_analise(),
                "caracteristica": {
                    "type": "string",
                    "enum": Caracteristica::TODAS.map(Caracteristica::como_str),
                    "description": format!("Característica violada: {caracteristicas}.")
                },
                "severidade": {
                    "type": "string",
                    "enum": ["alta", "media", "baixa"],
                    "description": "alta: impede o desenvolvimento como está escrito (contradição, regra errada); media: gera dúvida ou retrabalho provável; baixa: melhoria de redação."
                },
                "trecho": {
                    "type": "string",
                    "maxLength": 2000,
                    "description": "Trecho do texto revisado, copiado como está (o termo vago, a frase contraditória). Obrigatório, menos na completude."
                },
                "problema": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 4000,
                    "description": "O problema, em linguagem de negócio. Na correção, diga o que a regra ou o código fazem, com valores (\"o sistema só cancela fatura sem nenhum valor pago\")."
                },
                "reescrita": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 4000,
                    "description": "Reescrita sugerida do trecho, verificável, ou o texto a acrescentar (na completude)."
                },
                "regras": {
                    "type": "array",
                    "maxItems": LIMITE_DE_REGRAS,
                    "items": { "type": "string", "minLength": 1 },
                    "description": "Regras da base (RN-<SIGLA>-NNN) com que o achado se relaciona; cada uma é conferida na base."
                },
                "evidencias": {
                    "type": "array",
                    "maxItems": crate::afirmar::LIMITE_DE_EVIDENCIAS,
                    "items": evidencias::schema_da_evidencia(),
                    "description": "Trechos do código que mostram o que o sistema faz. Obrigatória na correção; recomendada na completude."
                }
            },
            "required": ["analise", "caracteristica", "severidade", "problema"],
            "additionalProperties": false
        }),
        |ambiente, args| {
            let args: ArgsAchado = ler_argumentos(args)?;
            let id = comum::id_da_analise(&args.analise)?;
            let analise = ambiente.analises.carregar(&id)?;
            exigir_revisao(&analise)?;
            let trecho = args.trecho.unwrap_or_default();
            let trecho = trecho.trim();
            if trecho.is_empty() && args.caracteristica.exige_trecho() {
                return Err(ErroFerramenta::Argumentos(format!(
                    "`trecho`: o achado de {} precisa do trecho do texto revisado, copiado \
                     como está",
                    args.caracteristica.como_str()
                )));
            }
            if !trecho.is_empty() && !contem_trecho(&analise.entrada, trecho) {
                return Err(ErroFerramenta::Recusa(format!(
                    "o trecho {} não está no texto revisado. Copie o trecho como está no \
                     texto (pode ser parte de uma frase) e chame de novo",
                    entre_aspas(trecho)
                )));
            }
            if args.caracteristica.exige_evidencia() && args.evidencias.is_empty() {
                return Err(comum::recusa(ErroModelo::AchadoSemEvidencia(
                    args.caracteristica.rotulo(),
                )));
            }
            let (regras, avisos) = conferir_regras(ambiente, &args.regras)?;
            let evidencias = evidencias::validar_todas(&ambiente.escopo, &args.evidencias)
                .map_err(ErroFerramenta::Recusa)?;
            let novo = NovoAchado {
                caracteristica: args.caracteristica,
                severidade: args.severidade,
                trecho: trecho.to_owned(),
                problema: args.problema,
                reescrita: args.reescrita,
                regras,
                evidencias,
                sessao: None,
            };
            let registrado = comum::alterar(ambiente, &id, Mudanca::Parecer, |a, agora| {
                a.registrar_achado(novo, agora)
            })?;
            let numero = registrado.resultado;
            let parecer = registrado.analise.parecer.clone().unwrap_or_default();
            let achado = parecer
                .achados
                .iter()
                .find(|a| a.numero == numero)
                .expect("achado recém-registrado");
            Ok(comum::resposta(
                ambiente,
                &registrado,
                json!({
                    "achado": numero,
                    "caracteristica": achado.caracteristica,
                    "severidade": achado.severidade,
                    "regras": achado.regras,
                    "evidencias": achado.evidencias.iter().map(|e| json!({
                        "arquivo": e.arquivo,
                        "linhas": e.linhas,
                        "hash": e.hash,
                    })).collect::<Vec<_>>(),
                    "veredito_sugerido": parecer.veredito_sugerido(),
                    "termos_vagos_sem_achado": vagos_pendentes(&registrado.analise),
                    "avisos_das_regras": avisos,
                }),
            ))
        },
    )
}

// ---------------------------------------------------------------------------
// registrar_invest

#[derive(Debug, Deserialize)]
struct ArgsInvest {
    analise: String,
    criterios: Vec<ItemInvest>,
}

#[derive(Debug, Deserialize)]
struct ItemInvest {
    criterio: CriterioInvest,
    resultado: ResultadoInvest,
    justificativa: String,
}

fn nome_do_criterio(c: CriterioInvest) -> &'static str {
    match c {
        CriterioInvest::Independente => "independente",
        CriterioInvest::Negociavel => "negociavel",
        CriterioInvest::Valiosa => "valiosa",
        CriterioInvest::Estimavel => "estimavel",
        CriterioInvest::Pequena => "pequena",
        CriterioInvest::Testavel => "testavel",
    }
}

/// `registrar_invest`: a verificação INVEST de uma história, os seis critérios.
pub fn registrar_invest() -> Ferramenta {
    let perguntas = CriterioInvest::TODOS
        .iter()
        .map(|c| format!("{} ({})", nome_do_criterio(*c), c.pergunta()))
        .collect::<Vec<_>>()
        .join("; ");
    Ferramenta::nova(
        "registrar_invest",
        "Registrar verificação INVEST",
        "Registra a verificação INVEST do texto revisado, quando ele é uma história de \
         usuário (\"Como <perfil>, quero <ação>, para <valor>\"): os seis critérios, \
         cada um com o resultado (atende, atende_em_parte, nao_atende) e uma \
         justificativa curta. Substitui a verificação anterior. Um critério não \
         atendido costuma ter também um achado (por exemplo, uma história que junta \
         duas necessidades não é pequena e tem achado de singularidade).",
        json!({
            "type": "object",
            "properties": {
                "analise": comum::schema_da_analise(),
                "criterios": {
                    "type": "array",
                    "minItems": 6,
                    "maxItems": 6,
                    "items": {
                        "type": "object",
                        "properties": {
                            "criterio": {
                                "type": "string",
                                "enum": CriterioInvest::TODOS.map(nome_do_criterio),
                                "description": format!("Critério: {perguntas}.")
                            },
                            "resultado": {
                                "type": "string",
                                "enum": ["atende", "atende_em_parte", "nao_atende"]
                            },
                            "justificativa": {
                                "type": "string",
                                "minLength": 1,
                                "maxLength": 1000
                            }
                        },
                        "required": ["criterio", "resultado", "justificativa"],
                        "additionalProperties": false
                    },
                    "description": "Os seis critérios INVEST, um item por critério."
                }
            },
            "required": ["analise", "criterios"],
            "additionalProperties": false
        }),
        |ambiente, args| {
            let args: ArgsInvest = ler_argumentos(args)?;
            let id = comum::id_da_analise(&args.analise)?;
            let itens: Vec<AvaliacaoInvest> = args
                .criterios
                .into_iter()
                .map(|i| AvaliacaoInvest {
                    criterio: i.criterio,
                    resultado: i.resultado,
                    justificativa: i.justificativa.trim().to_owned(),
                })
                .collect();
            let registrado = comum::alterar(ambiente, &id, Mudanca::Parecer, |a, _| {
                a.registrar_invest(itens)
            })?;
            let nao_atende: Vec<&str> = registrado
                .analise
                .parecer
                .iter()
                .flat_map(|p| &p.invest)
                .filter(|i| i.resultado != ResultadoInvest::Atende)
                .map(|i| nome_do_criterio(i.criterio))
                .collect();
            Ok(comum::resposta(
                ambiente,
                &registrado,
                json!({ "invest": "registrada", "nao_atendidos": nao_atende }),
            ))
        },
    )
}

// ---------------------------------------------------------------------------
// registrar_veredito

#[derive(Debug, Deserialize)]
struct ArgsVeredito {
    analise: String,
    veredito: Veredito,
    justificativa: String,
}

/// `registrar_veredito`: o veredito da revisão, com a justificativa.
pub fn registrar_veredito() -> Ferramenta {
    Ferramenta::nova(
        "registrar_veredito",
        "Registrar veredito da revisão",
        "Registra o veredito do parecer de revisão: `pronta` (pode ir para o \
         desenvolvimento como está), `pronta_com_ressalvas` (pode ir com os ajustes dos \
         achados) ou `precisa_de_retrabalho` (volta ao autor). Registre depois dos \
         achados. O veredito não pode ser mais brando do que os achados indicam: \
         achado de severidade alta pede retrabalho; os demais, ao menos ressalvas. Vale \
         o mais recente.",
        json!({
            "type": "object",
            "properties": {
                "analise": comum::schema_da_analise(),
                "veredito": {
                    "type": "string",
                    "enum": ["pronta", "pronta_com_ressalvas", "precisa_de_retrabalho"]
                },
                "justificativa": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 2000,
                    "description": "Uma ou duas frases com o motivo principal do veredito, para o autor."
                }
            },
            "required": ["analise", "veredito", "justificativa"],
            "additionalProperties": false
        }),
        |ambiente, args| {
            let args: ArgsVeredito = ler_argumentos(args)?;
            let id = comum::id_da_analise(&args.analise)?;
            let analise = ambiente.analises.carregar(&id)?;
            exigir_revisao(&analise)?;
            let sugerido = analise
                .parecer
                .clone()
                .unwrap_or_default()
                .veredito_sugerido();
            if args.veredito < sugerido {
                return Err(ErroFerramenta::Recusa(format!(
                    "os achados registrados indicam ao menos `{}` (achado de severidade \
                     alta pede retrabalho; os demais, ressalvas). Registre esse veredito \
                     ou um mais rigoroso",
                    nome_do_veredito(sugerido)
                )));
            }
            let registrado = comum::alterar(ambiente, &id, Mudanca::Parecer, |a, agora| {
                a.registrar_veredito(args.veredito, &args.justificativa, agora)
            })?;
            Ok(comum::resposta(
                ambiente,
                &registrado,
                json!({
                    "veredito": args.veredito,
                    "veredito_sugerido": sugerido,
                    "termos_vagos_sem_achado": vagos_pendentes(&registrado.analise),
                }),
            ))
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use cba_kb::frontmatter::Linhas;

    fn analise() -> Analise {
        let mut a = Analise::nova(
            Serie::Analise.id(3),
            "faturamento-exemplo",
            Fluxo::Revisar,
            "Cancelamento",
            "Como atendente, quero cancelar a fatura de forma rápida, etc.",
            1,
        );
        a.registrar_achado(
            NovoAchado {
                caracteristica: Caracteristica::Correcao,
                severidade: Severidade::Alta,
                trecho: "cancelar a fatura".into(),
                problema: "O sistema só cancela fatura sem pagamento.".into(),
                reescrita: Some("Cancelar a fatura aberta e sem pagamento.".into()),
                regras: vec![Id::analisar("RN-FAT-012").unwrap()],
                evidencias: vec![cba_core::analise::Evidencia {
                    arquivo: CaminhoRelativo::novo("db/03_pkg_cobranca.sql").unwrap(),
                    linhas: Linhas::novo(115, 117).unwrap(),
                    trecho: "IF V_VL_PAGO > 0 THEN".into(),
                    hash: "abc".into(),
                    commit: None,
                    estado: Default::default(),
                }],
                sessao: None,
            },
            2,
        )
        .unwrap();
        a
    }

    #[test]
    fn fonte_com_chave_e_link() {
        let d = |origem: &str, chave: Option<&str>| Demanda {
            id: None,
            origem: origem.into(),
            chave: chave.map(str::to_owned),
        };
        assert_eq!(
            fonte(&d("https://exemplo.com/i/7", Some("FAT-7"))).unwrap(),
            "Fonte: [FAT-7](https://exemplo.com/i/7)"
        );
        assert_eq!(fonte(&d("texto", None)), None);
        assert_eq!(fonte(&d("texto", Some("#7"))).unwrap(), "Fonte: #7");
    }

    #[test]
    fn textos_do_parecer() {
        let mut a = analise();
        a.registrar_veredito(Veredito::PrecisaDeRetrabalho, "Contradiz a regra.", 3)
            .unwrap();
        let t = textos(&a);
        let secao = |s: &str| -> Vec<&str> {
            t.iter()
                .filter(|x| x.secao == s)
                .map(|x| x.texto.as_str())
                .collect()
        };
        assert_eq!(
            secao("requisito"),
            ["_Termos vagos da lista de referência no texto: “rápida”, “etc.”._"]
        );
        assert_eq!(
            secao("veredito"),
            [
                "**Precisa de retrabalho.** Contradiz a regra.",
                "Achados: 1 de correção."
            ]
        );
        let achados = secao("achados");
        assert_eq!(achados[0], "### Severidade alta");
        assert_eq!(
            achados[1],
            "**Achado 1 · Correção** · “cancelar a fatura”: O sistema só cancela fatura sem \
             pagamento. Regras da base: [RN-FAT-012](../regras/RN-FAT-012.md)."
        );
        assert_eq!(
            achados[2],
            "> **Reescrita sugerida:** Cancelar a fatura aberta e sem pagamento."
        );
        let com_citacao = t
            .iter()
            .find(|x| x.texto.starts_with("**Achado 1"))
            .unwrap();
        assert_eq!(com_citacao.citacoes.len(), 1);
        assert!(secao("invest").is_empty());

        // Descartado sai do parecer.
        a.decidir_achado(1, EstadoAchado::Descartado { motivo: None })
            .unwrap();
        let t = textos(&a);
        assert!(t.iter().all(|x| x.secao != "achados"));
    }

    #[test]
    fn concluir_exige_veredito_coerente_e_avisa_termos_vagos() {
        let mut a = analise();
        assert!(conferir_para_concluir(&a).is_err());
        a.registrar_veredito(Veredito::ProntaComRessalvas, "Ajustes.", 3)
            .unwrap();
        let e = conferir_para_concluir(&a).unwrap_err();
        assert!(e.to_string().contains("precisa_de_retrabalho"), "{e}");
        a.registrar_veredito(Veredito::PrecisaDeRetrabalho, "Contradiz.", 4)
            .unwrap();
        let pendencias = conferir_para_concluir(&a).unwrap();
        assert_eq!(pendencias.len(), 1);
        assert!(pendencias[0].contains("“rápida”"), "{pendencias:?}");
        a.registrar_achado(
            NovoAchado {
                caracteristica: Caracteristica::Ambiguidade,
                severidade: Severidade::Media,
                trecho: "de forma rápida, etc.".into(),
                problema: "Não diz o tempo nem fecha a lista.".into(),
                reescrita: None,
                regras: Vec::new(),
                evidencias: Vec::new(),
                sessao: None,
            },
            5,
        )
        .unwrap();
        assert!(conferir_para_concluir(&a).unwrap().is_empty());
    }
}
