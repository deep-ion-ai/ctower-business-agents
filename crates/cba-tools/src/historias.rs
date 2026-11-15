//! Fluxo "Histórias e critérios" (#115): `registrar_historia` e
//! `registrar_criterio`, e as seções do documento montadas a partir das histórias.
//!
//! - `registrar_historia` registra uma história ("Como <papel>, quero <objetivo>,
//!   para <benefício>") com os critérios de aceite em Gherkin, os alertas INVEST e,
//!   se houver, a proposta de quebra em fatias ou a história de que esta é uma
//!   fatia. O identificador `HU-NNNN` é o próximo do projeto, reservado sob a
//!   trava das análises ([`cba_core::analises::Analises::maior_numero`]).
//! - `registrar_criterio` acrescenta um critério a uma história já registrada.
//!
//! Todo critério cita ao menos uma afirmação ou regra, conferida por
//! [`crate::fundamentos`]; critério sem origem, ou com referência que não existe,
//! é recusado. O critério que depende de pergunta em aberto fica "a confirmar".
//!
//! [`textos`] converte as histórias nas seções `historias` e `rastreabilidade` do
//! documento (template `historias`), com as evidências das afirmações e das
//! regras citadas como notas de rodapé.

use cba_core::analise::Analise;
use cba_core::eventos::Mudanca;
use cba_core::historias::{
    AlertaInvest, Criterio, Gherkin, Historia, Narrativa, NovaHistoria, NovoCriterio,
    SituacaoHistoria,
};
use cba_core::qualidade::CriterioInvest;
use cba_core::requisito::Fundamento;
use cba_docs::{Citacao, HISTORIAS, RASTREABILIDADE, Texto};
use cba_kb::{CaminhoRelativo, IdHistoria};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::comum;
use crate::fundamentos::{self, Fundamentacao, FundamentosCitados, RefCitada};
use crate::registro::{ErroFerramenta, Ferramenta, ler_argumentos};

// ---------------------------------------------------------------------------
// Documento

/// Texto numa linha só.
fn em_uma_linha(texto: &str) -> String {
    texto.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn situacao(h: &Historia) -> &'static str {
    match (h.situacao, &h.quebra_pedida) {
        (SituacaoHistoria::Pendente, Some(_)) => "quebra pedida pelo analista",
        (SituacaoHistoria::Pendente, None) => "sem decisão",
        (SituacaoHistoria::Aceita, _) => "aceita",
        (SituacaoHistoria::Descartada, _) => "descartada",
        (SituacaoHistoria::Quebrada, _) => "quebrada em fatias",
    }
}

/// `I, N, V e E`.
fn lista_de_letras(letras: &[CriterioInvest]) -> String {
    let l: Vec<String> = letras.iter().map(|x| x.letra().to_string()).collect();
    match l.as_slice() {
        [] => String::new(),
        [um] => um.clone(),
        [inicio @ .., ultimo] => format!("{} e {ultimo}", inicio.join(", ")),
    }
}

fn linha_invest(alertas: &[AlertaInvest]) -> String {
    if alertas.is_empty() {
        return "**INVEST:** atende aos seis critérios (I, N, V, E, S, T).".into();
    }
    let atende: Vec<CriterioInvest> = CriterioInvest::TODOS
        .into_iter()
        .filter(|l| !alertas.iter().any(|a| a.criterio == *l))
        .collect();
    let mut t = if atende.is_empty() {
        "**INVEST:** não atende a nenhum critério:".to_owned()
    } else {
        format!(
            "**INVEST:** atende a {}; não atende a:",
            lista_de_letras(&atende)
        )
    };
    for a in alertas {
        t.push_str(&format!(
            "\n\n- **{}** ({}): {}",
            a.criterio.letra(),
            a.nome(),
            em_uma_linha(&a.motivo)
        ));
    }
    t
}

/// Link para o que o fundamento cita: o documento da análise ou o da regra.
fn link_do_fundamento(f: &Fundamento, caminho: &CaminhoRelativo) -> String {
    match f {
        Fundamento::Afirmacao { referencia, .. } => cba_docs::link(
            &format!("{} (afirmação {})", referencia.analise, referencia.numero),
            caminho,
            &referencia.analise.arquivo(),
        ),
        Fundamento::Regra { id, .. } => cba_docs::link(&id.to_string(), caminho, &id.arquivo()),
    }
}

fn cabecalho(h: &Historia) -> String {
    let n = &h.narrativa;
    let mut t = format!(
        "### {} · {}\n\n**Como** {}, **quero** {}, **para** {}.\n\n**Situação:** {}",
        h.id,
        em_uma_linha(&n.titulo),
        em_uma_linha(&n.como),
        em_uma_linha(&n.quero).trim_end_matches('.'),
        em_uma_linha(&n.para).trim_end_matches('.'),
        situacao(h)
    );
    if let Some(original) = h.fatia_de {
        t.push_str(&format!(" · fatia de {original}"));
    }
    t.push_str("\n\n");
    t.push_str(&linha_invest(&h.alertas_invest()));
    if let Some(q) = &h.quebra {
        t.push_str(&format!("\n\n**Quebra proposta:** {}", em_uma_linha(q)));
    }
    if let Some(p) = h.quebra_pedida.as_deref().filter(|p| !p.is_empty()) {
        t.push_str(&format!("\n\n**Pedido de quebra:** {}", em_uma_linha(p)));
    }
    t.push_str("\n\n#### Critérios de aceite");
    if h.criterios_validos().next().is_none() {
        t.push_str("\n\n_Nenhum critério de aceite registrado._");
    }
    t
}

fn texto_do_criterio(c: &Criterio, caminho: &CaminhoRelativo) -> Texto {
    let mut t = format!("**{}**", c.rotulo());
    if let Some(p) = &c.pergunta {
        t.push_str(&format!(" · _a confirmar: depende da pergunta {p}_"));
    }
    let Gherkin {
        dado,
        quando,
        entao,
    } = &c.passos;
    t.push_str(&format!(
        "\n\n- **Dado** {}\n- **Quando** {}\n- **Então** {}",
        em_uma_linha(dado),
        em_uma_linha(quando),
        em_uma_linha(entao)
    ));
    let origens: Vec<String> = c
        .fundamentos
        .iter()
        .map(|f| {
            format!(
                "- {}: {}",
                link_do_fundamento(f, caminho),
                em_uma_linha(f.texto())
            )
        })
        .collect();
    t.push_str(&format!("\n\n_Origem:_\n\n{}", origens.join("\n")));
    let mut citacoes: Vec<Citacao> = Vec::new();
    for e in c.fundamentos.iter().flat_map(Fundamento::evidencias) {
        let cit = Citacao::from(e);
        if !citacoes
            .iter()
            .any(|x| x.arquivo == cit.arquivo && x.linhas == cit.linhas)
        {
            citacoes.push(cit);
        }
    }
    Texto {
        secao: HISTORIAS.into(),
        texto: t,
        citacoes,
    }
}

fn tabela(analise: &Analise) -> Option<String> {
    let mut linhas = Vec::new();
    for h in analise.historias_no_documento() {
        for c in h.criterios_validos() {
            let origem: Vec<String> = c.fundamentos.iter().map(Fundamento::rotulo).collect();
            let obs = c
                .pergunta
                .as_ref()
                .map(|p| format!("a confirmar ({p})"))
                .unwrap_or_default();
            linhas.push(format!(
                "| {} | {} | {} | {obs} |",
                h.id,
                c.rotulo(),
                origem.join(", ")
            ));
        }
    }
    (!linhas.is_empty()).then(|| {
        format!(
            "| História | Critério | Afirmações e regras | Observação |\n|---|---|---|---|\n{}",
            linhas.join("\n")
        )
    })
}

/// Textos das seções `historias` e `rastreabilidade` do documento: as histórias
/// que não foram descartadas nem quebradas, com os critérios não descartados.
/// `caminho` é o documento da análise, para os links relativos.
pub fn textos(analise: &Analise, caminho: &CaminhoRelativo) -> Vec<Texto> {
    let mut textos = Vec::new();
    for h in analise.historias_no_documento() {
        textos.push(Texto::novo(HISTORIAS, cabecalho(h)));
        for c in h.criterios_validos() {
            textos.push(texto_do_criterio(c, caminho));
        }
    }
    if let Some(t) = tabela(analise) {
        textos.push(Texto::novo(RASTREABILIDADE, t));
    }
    textos
}

// ---------------------------------------------------------------------------
// Ferramentas

#[derive(Debug, Deserialize)]
struct CriterioCitado {
    dado: String,
    quando: String,
    entao: String,
    #[serde(flatten)]
    fundamentos: FundamentosCitados,
    #[serde(default)]
    pergunta: Option<RefCitada>,
}

#[derive(Debug, Deserialize)]
struct AlertaCitado {
    criterio: CriterioInvest,
    motivo: String,
}

#[derive(Debug, Deserialize)]
struct ArgsHistoria {
    analise: String,
    titulo: String,
    como: String,
    quero: String,
    para: String,
    #[serde(default)]
    criterios: Vec<CriterioCitado>,
    #[serde(default)]
    invest: Vec<AlertaCitado>,
    #[serde(default)]
    quebra: Option<String>,
    #[serde(default)]
    fatia_de: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ArgsCriterio {
    analise: String,
    historia: String,
    #[serde(flatten)]
    criterio: CriterioCitado,
}

fn texto(max: u64, descricao: &str) -> Value {
    json!({ "type": "string", "minLength": 1, "maxLength": max, "description": descricao })
}

/// Propriedades de um critério, comuns às duas ferramentas.
fn propriedades_do_criterio() -> serde_json::Map<String, Value> {
    let v = json!({
        "dado": texto(2000, "Contexto, com valores concretos (\"uma fatura PJ em aberto de R$ 3.000,00\")."),
        "quando": texto(2000, "Ação (\"o cliente pede 3 parcelas\")."),
        "entao": texto(2000, "Resultado esperado, com valores (\"são geradas 3 parcelas de R$ 1.000,00 com vencimento a cada 30 dias\")."),
        "afirmacoes": fundamentos::schema_das_afirmacoes(),
        "regras": fundamentos::schema_das_regras(),
        "pergunta": fundamentos::schema_da_pergunta(),
    });
    match v {
        Value::Object(m) => m,
        _ => unreachable!(),
    }
}

fn schema_do_criterio() -> Value {
    json!({
        "type": "object",
        "properties": propriedades_do_criterio(),
        "required": ["dado", "quando", "entao"],
        "additionalProperties": false
    })
}

fn schema_da_historia_id() -> Value {
    json!({
        "type": "string",
        "minLength": 1,
        "description": "Identificador da história (HU-NNNN), devolvido por registrar_historia."
    })
}

fn id_da_historia(texto: &str, campo: &str) -> Result<IdHistoria, ErroFerramenta> {
    IdHistoria::analisar(texto.trim()).map_err(|_| {
        ErroFerramenta::Argumentos(format!(
            "`{campo}`: `{texto}` não é o identificador de uma história; use HU-NNNN, como \
             HU-0001"
        ))
    })
}

fn novo_criterio(
    f: &mut Fundamentacao<'_>,
    c: &CriterioCitado,
) -> Result<NovoCriterio, ErroFerramenta> {
    Ok(NovoCriterio {
        passos: Gherkin {
            dado: c.dado.clone(),
            quando: c.quando.clone(),
            entao: c.entao.clone(),
        },
        fundamentos: f.fundamentos(&c.fundamentos)?,
        pergunta: c.pergunta.as_ref().map(|p| f.pergunta(p)).transpose()?,
        sessao: None,
    })
}

/// Avisos que não impedem o registro: critério sem nenhum número (o roteiro pede
/// valores concretos).
fn avisos_do_criterio(historia: IdHistoria, c: &Criterio) -> Option<String> {
    let p = &c.passos;
    let tem_numero = [&p.dado, &p.quando, &p.entao]
        .iter()
        .any(|t| t.chars().any(|ch| ch.is_ascii_digit()));
    (!tem_numero).then(|| {
        format!(
            "{historia} {}: sem nenhum valor concreto; prefira um exemplo com números (valores, \
             datas, quantidades)",
            c.rotulo()
        )
    })
}

fn exigir_fluxo(analise: &Analise) -> Result<(), ErroFerramenta> {
    if analise.fluxo == cba_core::analise::Fluxo::Historias {
        Ok(())
    } else {
        Err(comum::recusa(
            cba_core::analise::ErroModelo::ForaDoFluxoDeHistorias(analise.id.clone()),
        ))
    }
}

/// `registrar_historia`: uma história com os critérios de aceite.
pub fn registrar_historia() -> Ferramenta {
    let mut propriedades = serde_json::Map::new();
    propriedades.insert("analise".into(), comum::schema_da_analise());
    propriedades.insert(
        "titulo".into(),
        texto(
            200,
            "Título curto da história (\"Parcelar fatura de PJ em até 3 vezes\").",
        ),
    );
    propriedades.insert(
        "como".into(),
        texto(
            300,
            "Papel de quem usa, sem o \"Como\" (\"analista financeiro do cliente PJ\").",
        ),
    );
    propriedades.insert(
        "quero".into(),
        texto(
            1000,
            "Objetivo, sem o \"quero\" (\"parcelar uma fatura em aberto em 2 ou 3 vezes\").",
        ),
    );
    propriedades.insert(
        "para".into(),
        texto(1000, "Benefício para o negócio, sem o \"para\" (\"distribuir o pagamento no fluxo de caixa\")."),
    );
    propriedades.insert(
        "criterios".into(),
        json!({
            "type": "array",
            "maxItems": 15,
            "items": schema_do_criterio(),
            "description": "Critérios de aceite em Dado / Quando / Então, cada um com ao menos uma afirmação ou regra de origem."
        }),
    );
    propriedades.insert(
        "invest".into(),
        json!({
            "type": "array",
            "maxItems": 6,
            "items": {
                "type": "object",
                "properties": {
                    "criterio": {
                        "type": "string",
                        "enum": ["independente", "negociavel", "valiosa", "estimavel", "pequena", "testavel"]
                    },
                    "motivo": texto(1000, "Por que a história não atende ao critério.")
                },
                "required": ["criterio", "motivo"],
                "additionalProperties": false
            },
            "description": "Critérios INVEST que a história NÃO atende, cada um com o motivo. Omita os que ela atende."
        }),
    );
    propriedades.insert(
        "quebra".into(),
        texto(
            2000,
            "Se a história é grande: a proposta de quebra em fatias verticais e por quê.",
        ),
    );
    propriedades.insert(
        "fatia_de".into(),
        json!({
            "type": "string",
            "minLength": 1,
            "description": "Se esta história é uma fatia de outra desta análise (HU-NNNN): a original fica marcada como quebrada."
        }),
    );
    Ferramenta::nova(
        "registrar_historia",
        "Registrar história",
        "Registra uma história de usuário na análise de histórias: título, \"Como \
         <papel>, quero <objetivo>, para <benefício>\" e os critérios de aceite em Dado \
         / Quando / Então, com valores concretos. Cada critério precisa citar ao menos \
         uma origem: `afirmacoes` (o número da afirmação na análise de origem, ou \
         A-NNNN/n) ou `regras` (RN-<SIGLA>-NNN); critério sem origem, ou com origem \
         que não existe, é recusado. Critério que depende de pergunta em aberto leva \
         `pergunta` e fica \"a confirmar\". Informe em `invest` os critérios INVEST que a \
         história não atende e, se ela for grande, a proposta de quebra em `quebra`; \
         uma fatia de outra história leva `fatia_de`. Devolve o identificador estável \
         da história (HU-NNNN).",
        json!({
            "type": "object",
            "properties": propriedades,
            "required": ["analise", "titulo", "como", "quero", "para"],
            "additionalProperties": false
        }),
        |ambiente, args| {
            let args: ArgsHistoria = ler_argumentos(args)?;
            let id = comum::id_da_analise(&args.analise)?;
            let atual = ambiente.analises.carregar(&id)?;
            exigir_fluxo(&atual)?;
            let fatia_de = args
                .fatia_de
                .as_deref()
                .map(|t| id_da_historia(t, "fatia_de"))
                .transpose()?;
            let mut f = Fundamentacao::nova(ambiente, &atual);
            let criterios = args
                .criterios
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    novo_criterio(&mut f, c).map_err(|e| match e {
                        ErroFerramenta::Recusa(m) => {
                            ErroFerramenta::Recusa(format!("critério {}: {m}", i + 1))
                        }
                        ErroFerramenta::Argumentos(m) => {
                            ErroFerramenta::Argumentos(format!("criterios[{i}]: {m}"))
                        }
                        outro => outro,
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            let nova = NovaHistoria {
                narrativa: Narrativa {
                    titulo: args.titulo,
                    como: args.como,
                    quero: args.quero,
                    para: args.para,
                },
                criterios,
                invest: args
                    .invest
                    .into_iter()
                    .map(|a| AlertaInvest {
                        criterio: a.criterio,
                        motivo: a.motivo,
                    })
                    .collect(),
                quebra: args.quebra,
                fatia_de,
                sessao: None,
            };
            let registrado = comum::alterar(ambiente, &id, Mudanca::Historia, |a, agora| {
                let maior = ambiente
                    .analises
                    .maior_numero(&a.id, Analise::maior_historia)
                    .max(a.maior_historia());
                a.registrar_historia(nova, IdHistoria::novo(maior + 1), agora)
            })?;
            let hu = registrado.resultado;
            let historia = registrado
                .analise
                .historia(hu)
                .ok_or_else(|| ErroFerramenta::Interno(format!("{hu} não foi gravada")))?;
            let avisos: Vec<String> = historia
                .criterios
                .iter()
                .filter_map(|c| avisos_do_criterio(hu, c))
                .collect();
            Ok(comum::resposta(
                ambiente,
                &registrado,
                json!({
                    "historia": hu,
                    "criterios": historia.criterios.iter().map(Criterio::rotulo).collect::<Vec<_>>(),
                    "a_confirmar": historia.criterios.iter().filter(|c| c.a_confirmar()).map(Criterio::rotulo).collect::<Vec<_>>(),
                    "invest": historia.alertas_invest().iter().map(|a| json!({
                        "criterio": a.criterio, "motivo": a.motivo
                    })).collect::<Vec<_>>(),
                    "avisos_dos_criterios": avisos,
                }),
            ))
        },
    )
}

/// `registrar_criterio`: um critério de aceite numa história já registrada.
pub fn registrar_criterio() -> Ferramenta {
    let mut propriedades = propriedades_do_criterio();
    propriedades.insert("analise".into(), comum::schema_da_analise());
    propriedades.insert("historia".into(), schema_da_historia_id());
    Ferramenta::nova(
        "registrar_criterio",
        "Registrar critério de aceite",
        "Acrescenta um critério de aceite (Dado / Quando / Então, com valores \
         concretos) a uma história já registrada com registrar_historia. O critério \
         precisa citar ao menos uma origem: `afirmacoes` (número na análise de origem \
         ou A-NNNN/n) ou `regras` (RN-<SIGLA>-NNN); sem origem, ou com origem que não \
         existe, é recusado. Com `pergunta` (em aberto), fica \"a confirmar\". Devolve o \
         identificador do critério na história (CA-n).",
        json!({
            "type": "object",
            "properties": propriedades,
            "required": ["analise", "historia", "dado", "quando", "entao"],
            "additionalProperties": false
        }),
        |ambiente, args| {
            let args: ArgsCriterio = ler_argumentos(args)?;
            let id = comum::id_da_analise(&args.analise)?;
            let hu = id_da_historia(&args.historia, "historia")?;
            let atual = ambiente.analises.carregar(&id)?;
            exigir_fluxo(&atual)?;
            if atual.historia(hu).is_none() {
                return Err(comum::recusa(
                    cba_core::analise::ErroModelo::HistoriaDesconhecida(hu),
                ));
            }
            let mut f = Fundamentacao::nova(ambiente, &atual);
            let novo = novo_criterio(&mut f, &args.criterio)?;
            let registrado = comum::alterar(ambiente, &id, Mudanca::Historia, |a, agora| {
                a.registrar_criterio(hu, novo, agora)
            })?;
            let numero = registrado.resultado;
            let criterio = registrado
                .analise
                .historia(hu)
                .and_then(|h| h.criterios.iter().find(|c| c.numero == numero));
            Ok(comum::resposta(
                ambiente,
                &registrado,
                json!({
                    "historia": hu,
                    "criterio": format!("CA-{numero}"),
                    "a_confirmar": criterio.is_some_and(Criterio::a_confirmar),
                    "aviso_do_criterio": criterio.and_then(|c| avisos_do_criterio(hu, c)),
                }),
            ))
        },
    )
}

/// Resumo das histórias para `retomar_analise`: texto e dados.
pub fn para_retomar(analise: &Analise) -> (String, Value) {
    let mut t = String::from("## Histórias\n\n");
    if analise.historias.is_empty() {
        t.push_str("Nenhuma história registrada.\n\n");
    }
    let mut dados = Vec::new();
    for h in &analise.historias {
        let n = &h.narrativa;
        t.push_str(&format!(
            "### {} · {} [{}]\n\nComo {}, quero {}, para {}.\n",
            h.id,
            n.titulo,
            situacao(h),
            n.como,
            n.quero,
            n.para
        ));
        if let Some(o) = h.fatia_de {
            t.push_str(&format!("Fatia de {o}.\n"));
        }
        if let Some(p) = &h.quebra_pedida {
            if p.is_empty() {
                t.push_str("O analista pediu a quebra em fatias.\n");
            } else {
                t.push_str(&format!("O analista pediu a quebra em fatias: {p}\n"));
            }
        }
        for a in h.alertas_invest() {
            t.push_str(&format!(
                "INVEST, não atende a {} ({}): {}\n",
                a.criterio.letra(),
                a.nome(),
                a.motivo
            ));
        }
        for c in &h.criterios {
            let origem: Vec<String> = c.fundamentos.iter().map(Fundamento::rotulo).collect();
            let decisao = match c.situacao {
                cba_core::requisito::SituacaoItem::Pendente => "sem decisão",
                cba_core::requisito::SituacaoItem::Aceito => "aceito",
                cba_core::requisito::SituacaoItem::Descartado => "descartado pelo analista",
            };
            t.push_str(&format!(
                "- {} [{decisao}{}] Dado {}; quando {}; então {}. Origem: {}\n",
                c.rotulo(),
                c.pergunta
                    .as_ref()
                    .map(|p| format!("; a confirmar: pergunta {p}"))
                    .unwrap_or_default(),
                c.passos.dado,
                c.passos.quando,
                c.passos.entao,
                origem.join(", ")
            ));
        }
        t.push('\n');
        dados.push(json!({
            "id": h.id,
            "titulo": n.titulo,
            "situacao": h.situacao,
            "quebra_pedida": h.quebra_pedida,
            "fatia_de": h.fatia_de,
            "criterios": h.criterios.iter().map(|c| json!({
                "criterio": c.rotulo(),
                "situacao": c.situacao,
                "a_confirmar": c.a_confirmar(),
                "fundamentos": c.fundamentos.iter().map(Fundamento::rotulo).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
        }));
    }
    (t, Value::Array(dados))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cba_core::analise::Fluxo;
    use cba_core::requisito::RefItem;
    use cba_kb::frontmatter::{Evidencia, Linhas};
    use cba_kb::{Id, Serie};

    fn analise() -> Analise {
        let mut a = Analise::nova(
            Serie::Analise.id(2),
            "faturamento-exemplo",
            Fluxo::Historias,
            "Histórias do parcelamento",
            "Histórias da análise A-0001",
            1,
        );
        a.origem = Some(Serie::Analise.id(1));
        let evidencia = Evidencia {
            arquivo: CaminhoRelativo::novo("db/02_parametros.sql").unwrap(),
            linhas: Linhas::novo(7, 7).unwrap(),
            hash: "abc".into(),
            commit: None,
        };
        let criterio = |pergunta| NovoCriterio {
            passos: Gherkin {
                dado: "uma fatura PJ de R$ 3.000,00".into(),
                quando: "o cliente pede 3 parcelas".into(),
                entao: "são geradas 3 parcelas de R$ 1.000,00".into(),
            },
            fundamentos: vec![
                Fundamento::Afirmacao {
                    referencia: RefItem::analisar("A-0001/4").unwrap(),
                    texto: "O limite vem de MAX_PARCELAS.".into(),
                    evidencias: vec![evidencia.clone()],
                },
                Fundamento::Regra {
                    id: Id::analisar("RN-FAT-007").unwrap(),
                    titulo: "Limite de parcelas".into(),
                    evidencias: vec![evidencia.clone()],
                },
            ],
            pergunta,
            sessao: None,
        };
        a.registrar_historia(
            NovaHistoria {
                narrativa: Narrativa {
                    titulo: "Parcelar fatura PJ".into(),
                    como: "analista financeiro".into(),
                    quero: "parcelar em até 3 vezes".into(),
                    para: "distribuir o pagamento.".into(),
                },
                criterios: vec![
                    criterio(None),
                    criterio(Some(RefItem::analisar("A-0001/2").unwrap())),
                ],
                invest: vec![AlertaInvest {
                    criterio: CriterioInvest::Pequena,
                    motivo: "tela e relatório juntos".into(),
                }],
                quebra: Some("Separar a tela do relatório.".into()),
                fatia_de: None,
                sessao: None,
            },
            IdHistoria::novo(1),
            2,
        )
        .unwrap();
        a
    }

    #[test]
    fn historias_no_documento() {
        let a = analise();
        let textos = textos(&a, &a.documento);
        assert_eq!(textos.len(), 4);
        let cab = &textos[0].texto;
        assert!(cab.starts_with("### HU-0001 · Parcelar fatura PJ"), "{cab}");
        assert!(
            cab.contains("**Como** analista financeiro, **quero** parcelar em até 3 vezes, **para** distribuir o pagamento."),
            "{cab}"
        );
        assert!(
            cab.contains("atende a I, N, V, E e T; não atende a:"),
            "{cab}"
        );
        assert!(
            cab.contains("- **S** (pequena): tela e relatório juntos"),
            "{cab}"
        );
        assert!(cab.contains("**Quebra proposta:** Separar"), "{cab}");
        let ca1 = &textos[1];
        assert!(
            ca1.texto.contains("- **Dado** uma fatura PJ"),
            "{}",
            ca1.texto
        );
        assert!(
            ca1.texto
                .contains("[A-0001 (afirmação 4)](A-0001.md): O limite vem de MAX_PARCELAS."),
            "{}",
            ca1.texto
        );
        assert!(
            ca1.texto.contains("[RN-FAT-007](../regras/RN-FAT-007.md)"),
            "{}",
            ca1.texto
        );
        // A mesma evidência das duas origens vira uma nota só.
        assert_eq!(ca1.citacoes.len(), 1);
        assert!(
            textos[2]
                .texto
                .contains("_a confirmar: depende da pergunta A-0001/2_")
        );
        assert_eq!(textos[3].secao, RASTREABILIDADE);
        assert!(
            textos[3]
                .texto
                .contains("| HU-0001 | CA-2 | A-0001/4, RN-FAT-007 | a confirmar (A-0001/2) |")
        );
    }

    #[test]
    fn retomar_lista_historias_e_criterios() {
        let (texto, dados) = para_retomar(&analise());
        assert!(texto.contains("### HU-0001 · Parcelar fatura PJ [sem decisão]"));
        assert!(texto.contains("- CA-2 [sem decisão; a confirmar: pergunta A-0001/2]"));
        assert_eq!(dados[0]["criterios"][0]["fundamentos"][1], "RN-FAT-007");
    }
}
