//! Fluxo "Cenários de teste" (#117): `registrar_tabela_decisao` e
//! `registrar_cenario`, as seções do documento e a exportação em CSV.
//!
//! - `registrar_tabela_decisao` registra a tabela de decisão de uma regra com duas
//!   ou mais condições: condições (com evidência), ações e colunas, com as
//!   combinações impossíveis justificadas. A tabela incompleta (combinação sem
//!   coluna) ou ambígua (duas colunas para a mesma combinação) é recusada
//!   ([`cba_core::cenarios::TabelaDecisao::conferir`]).
//! - `registrar_cenario` registra um cenário de teste com identificador `CT-NNNN`
//!   (o próximo do projeto, reservado sob a trava das análises), a técnica, os
//!   passos com valores concretos, o limite lido do código (valor-limite e
//!   partição de equivalência), a coluna da tabela que exercita e a história
//!   ligada. As evidências do limite e das condições são conferidas contra o código
//!   ([`crate::evidencias`]); o valor do limite precisa aparecer no trecho citado
//!   ou, se vier de configuração, o cenário fica "a confirmar" com a pergunta.
//!
//! Todo item cita ao menos uma regra ou afirmação, conferida por
//! [`crate::fundamentos`]. [`textos`] monta as seções do documento (template
//! `cenarios-teste`) e [`csv`] a exportação com uma linha por cenário.

use cba_core::analise::{Analise, Evidencia, Fluxo};
use cba_core::cenarios::{
    Cenario, Condicao, Limite, NovaColuna, NovaTabela, NovoCenario, RefColuna, TabelaDecisao,
    Tecnica, TextoCenario, ValorCondicao,
};
use cba_core::eventos::Mudanca;
use cba_core::historias::Gherkin;
use cba_core::requisito::{Fundamento, SituacaoItem};
use cba_docs::{CENARIOS, Citacao, RASTREABILIDADE, TABELAS_DECISAO, Texto, VALORES_LIMITE};
use cba_kb::{CaminhoRelativo, IdCenario, IdHistoria};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::comum;
use crate::evidencias::{self, EvidenciaCitada};
use crate::fundamentos::{self, Fundamentacao, FundamentosCitados, RefCitada};
use crate::registro::{ErroFerramenta, Ferramenta, ler_argumentos};

// ---------------------------------------------------------------------------
// Documento

fn em_uma_linha(texto: &str) -> String {
    texto.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Texto que vai numa célula de tabela Markdown.
fn celula(texto: &str) -> String {
    em_uma_linha(texto).replace('|', "\\|")
}

fn citacao(e: &Evidencia) -> Citacao {
    Citacao {
        arquivo: e.arquivo.clone(),
        linhas: e.linhas,
        hash: Some(e.hash.clone()),
        commit: e.commit.clone(),
    }
}

fn situacao(s: SituacaoItem) -> &'static str {
    match s {
        SituacaoItem::Pendente => "sem decisão",
        SituacaoItem::Aceito => "aceito",
        SituacaoItem::Descartado => "descartado",
    }
}

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

fn origens(fundamentos: &[Fundamento], caminho: &CaminhoRelativo) -> String {
    fundamentos
        .iter()
        .map(|f| {
            format!(
                "- {}: {}",
                link_do_fundamento(f, caminho),
                em_uma_linha(f.texto())
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Cenários que exercitam a coluna.
fn cenarios_da_coluna(analise: &Analise, tabela: u32, coluna: u32) -> Vec<String> {
    analise
        .cenarios_no_documento()
        .filter(|c| c.coluna == Some(RefColuna { tabela, coluna }))
        .map(|c| c.id.to_string())
        .collect()
}

fn textos_da_tabela(analise: &Analise, t: &TabelaDecisao, caminho: &CaminhoRelativo) -> Vec<Texto> {
    let mut cab = format!(
        "### {} · {}\n\n**Situação:** {}\n\n_Origem:_\n\n{}",
        t.rotulo(),
        em_uma_linha(&t.titulo),
        situacao(t.situacao),
        origens(&t.fundamentos, caminho)
    );
    let mut tabela = String::from("|  |");
    for c in &t.colunas {
        tabela.push_str(&format!(" {} |", c.rotulo()));
    }
    tabela.push_str("\n|---|");
    tabela.push_str(&"---|".repeat(t.colunas.len()));
    for (i, cond) in t.condicoes.iter().enumerate() {
        tabela.push_str(&format!("\n| **C{}** {} |", i + 1, celula(&cond.texto)));
        for c in &t.colunas {
            tabela.push_str(&format!(" {} |", c.valores[i].simbolo()));
        }
    }
    for (i, acao) in t.acoes.iter().enumerate() {
        let n = i as u32 + 1;
        tabela.push_str(&format!("\n| **A{n}** {} |", celula(acao)));
        for c in &t.colunas {
            let marca = if c.impossivel.is_some() {
                "·"
            } else if c.acoes.contains(&n) {
                "X"
            } else {
                ""
            };
            tabela.push_str(&format!(" {marca} |"));
        }
    }
    tabela.push_str("\n| **Cenário** |");
    for c in &t.colunas {
        let marca = if c.impossivel.is_some() {
            "impossível".to_owned()
        } else {
            let lista = cenarios_da_coluna(analise, t.numero, c.numero);
            if lista.is_empty() {
                "—".to_owned()
            } else {
                lista.join(", ")
            }
        };
        tabela.push_str(&format!(" {marca} |"));
    }
    cab.push_str("\n\n");
    cab.push_str(&tabela);
    let impossiveis: Vec<String> = t
        .colunas
        .iter()
        .filter_map(|c| {
            c.impossivel
                .as_ref()
                .map(|j| format!("- **{}**: {}", c.rotulo(), em_uma_linha(j)))
        })
        .collect();
    if !impossiveis.is_empty() {
        cab.push_str(&format!(
            "\n\n**Combinações impossíveis:**\n\n{}",
            impossiveis.join("\n")
        ));
    }
    let mut textos = vec![Texto::novo(TABELAS_DECISAO, cab)];
    // As evidências de cada condição, fora da tabela (a nota de rodapé no fim de
    // uma linha da tabela a quebraria).
    let com_evidencia: Vec<(usize, &Condicao)> = t
        .condicoes
        .iter()
        .enumerate()
        .filter(|(_, c)| !c.evidencias.is_empty())
        .collect();
    if !com_evidencia.is_empty() {
        textos.push(Texto::novo(
            TABELAS_DECISAO,
            "**Onde o código testa cada condição:**",
        ));
        for (i, c) in com_evidencia {
            textos.push(Texto {
                secao: TABELAS_DECISAO.into(),
                texto: format!("- **C{}** {}", i + 1, em_uma_linha(&c.texto)),
                citacoes: c.evidencias.iter().map(citacao).collect(),
            });
        }
    }
    textos
}

fn limite_em_texto(l: &Limite) -> String {
    match &l.configuracao {
        Some(c) => format!(
            "**{}** ({}; valor de configuração em `{}`, a confirmar)",
            em_uma_linha(&l.valor),
            em_uma_linha(&l.grandeza),
            em_uma_linha(c)
        ),
        None => format!(
            "**{}** ({}, lido do código)",
            em_uma_linha(&l.valor),
            em_uma_linha(&l.grandeza)
        ),
    }
}

fn tabela_de_limites(analise: &Analise) -> Option<String> {
    let mut linhas = Vec::new();
    for c in analise.cenarios_no_documento() {
        let Some(l) = &c.limite else { continue };
        let evidencias: Vec<String> = l
            .evidencias
            .iter()
            .map(|e| format!("`{}`", citacao(e).local()))
            .collect();
        let valor = if l.configuracao.is_some() {
            format!("**{}** · a confirmar", celula(&l.valor))
        } else {
            format!("**{}**", celula(&l.valor))
        };
        linhas.push(format!(
            "| {} | {} | {} | {valor} | {} | {} | {} |",
            c.id,
            c.tecnica.rotulo(),
            celula(&l.grandeza),
            c.valor.as_deref().map(celula).unwrap_or_else(|| "—".into()),
            celula(&c.texto.passos.entao),
            evidencias.join(", ")
        ));
    }
    (!linhas.is_empty()).then(|| {
        format!(
            "| Cenário | Técnica | Grandeza | Limite no código | Valor testado | Resultado esperado | Evidência do limite |\n|---|---|---|---|---|---|---|\n{}",
            linhas.join("\n")
        )
    })
}

fn texto_do_cenario(c: &Cenario, caminho: &CaminhoRelativo) -> Texto {
    let mut t = format!(
        "### {} · {}\n\n**Técnica:** {} · **Situação:** {}",
        c.id,
        em_uma_linha(&c.texto.titulo),
        c.tecnica.rotulo(),
        situacao(c.situacao)
    );
    if let Some(r) = c.coluna {
        t.push_str(&format!(" · **Coluna:** {r}"));
    }
    if let Some(h) = c.historia {
        t.push_str(&format!(" · **História:** {h}"));
    }
    if let Some(p) = &c.pergunta {
        t.push_str(&format!("\n\n_A confirmar: depende da pergunta {p}._"));
    }
    if let Some(l) = &c.limite {
        t.push_str(&format!("\n\n**Limite:** {}", limite_em_texto(l)));
    }
    if let Some(v) = &c.valor {
        t.push_str(&format!("\n\n**Valor testado:** {}", em_uma_linha(v)));
    }
    let Gherkin {
        dado,
        quando,
        entao,
    } = &c.texto.passos;
    t.push_str(&format!(
        "\n\n- **Dado** {}\n- **Quando** {}\n- **Então** {}",
        em_uma_linha(dado),
        em_uma_linha(quando),
        em_uma_linha(entao)
    ));
    t.push_str(&format!(
        "\n\n_Origem:_\n\n{}",
        origens(&c.fundamentos, caminho)
    ));
    Texto {
        secao: CENARIOS.into(),
        texto: t,
        citacoes: c.evidencias().iter().map(Citacao::from).collect(),
    }
}

fn rastreabilidade(analise: &Analise) -> Option<String> {
    let mut linhas = Vec::new();
    for t in analise.tabelas_no_documento() {
        let origem: Vec<String> = t.fundamentos.iter().map(Fundamento::rotulo).collect();
        linhas.push(format!(
            "| {} | tabela de decisão | {} | — | — | |",
            t.rotulo(),
            origem.join(", ")
        ));
    }
    for c in analise.cenarios_no_documento() {
        let origem: Vec<String> = c.fundamentos.iter().map(Fundamento::rotulo).collect();
        let evidencias: Vec<String> = c
            .evidencias()
            .iter()
            .map(|e| format!("`{}`", Citacao::from(e).local()))
            .collect();
        let obs = c
            .pergunta
            .as_ref()
            .map(|p| format!("a confirmar ({p})"))
            .unwrap_or_default();
        linhas.push(format!(
            "| {} | {} | {} | {} | {} | {obs} |",
            c.id,
            c.tecnica.rotulo(),
            origem.join(", "),
            if evidencias.is_empty() {
                "—".to_owned()
            } else {
                evidencias.join(", ")
            },
            c.historia
                .map(|h| h.to_string())
                .unwrap_or_else(|| "—".into()),
        ));
    }
    (!linhas.is_empty()).then(|| {
        format!(
            "| Item | Técnica | Regras e afirmações | Evidências | História | Observação |\n|---|---|---|---|---|---|\n{}",
            linhas.join("\n")
        )
    })
}

/// Textos das seções do documento de cenários: tabelas de decisão, valores-limite,
/// cenários e rastreabilidade, sem os itens descartados. `caminho` é o documento
/// da análise, para os links relativos.
pub fn textos(analise: &Analise, caminho: &CaminhoRelativo) -> Vec<Texto> {
    let mut textos = Vec::new();
    for t in analise.tabelas_no_documento() {
        textos.extend(textos_da_tabela(analise, t, caminho));
    }
    if let Some(t) = tabela_de_limites(analise) {
        textos.push(Texto::novo(VALORES_LIMITE, t));
    }
    for c in analise.cenarios_no_documento() {
        textos.push(texto_do_cenario(c, caminho));
    }
    if let Some(t) = rastreabilidade(analise) {
        textos.push(Texto::novo(RASTREABILIDADE, t));
    }
    textos
}

// ---------------------------------------------------------------------------
// CSV

/// Colunas do CSV, na ordem.
pub const COLUNAS_DO_CSV: [&str; 15] = [
    "id",
    "titulo",
    "tecnica",
    "situacao",
    "pre_condicao",
    "passos",
    "resultado_esperado",
    "valor_testado",
    "limite",
    "grandeza",
    "coluna_tabela",
    "regras_e_afirmacoes",
    "historia",
    "evidencias",
    "a_confirmar",
];

fn campo_csv(texto: &str) -> String {
    let texto = em_uma_linha(texto);
    if texto.contains([',', '"', ';', '\n', '\r']) {
        format!("\"{}\"", texto.replace('"', "\"\""))
    } else {
        texto
    }
}

/// Cenários em CSV (RFC 4180, separador vírgula, UTF-8 com BOM para o Excel
/// reconhecer os acentos): cabeçalho [`COLUNAS_DO_CSV`] e uma linha por cenário
/// que não foi descartado, na ordem do identificador.
pub fn csv(analise: &Analise) -> String {
    let mut linhas = vec![COLUNAS_DO_CSV.join(",")];
    let mut cenarios: Vec<&Cenario> = analise.cenarios_no_documento().collect();
    cenarios.sort_by_key(|c| c.id);
    for c in cenarios {
        let origem: Vec<String> = c.fundamentos.iter().map(Fundamento::rotulo).collect();
        let evidencias: Vec<String> = c
            .evidencias()
            .iter()
            .map(|e| Citacao::from(e).local())
            .collect();
        let limite = c.limite.as_ref();
        let campos = [
            c.id.to_string(),
            c.texto.titulo.clone(),
            c.tecnica.rotulo().to_owned(),
            situacao(c.situacao).to_owned(),
            c.texto.passos.dado.clone(),
            c.texto.passos.quando.clone(),
            c.texto.passos.entao.clone(),
            c.valor.clone().unwrap_or_default(),
            limite.map(|l| l.valor.clone()).unwrap_or_default(),
            limite.map(|l| l.grandeza.clone()).unwrap_or_default(),
            c.coluna.map(|r| r.to_string()).unwrap_or_default(),
            origem.join(" "),
            c.historia.map(|h| h.to_string()).unwrap_or_default(),
            evidencias.join(" "),
            c.pergunta
                .as_ref()
                .map(|p| format!("pergunta {p}"))
                .unwrap_or_default(),
        ];
        linhas.push(
            campos
                .iter()
                .map(|t| campo_csv(t))
                .collect::<Vec<_>>()
                .join(","),
        );
    }
    format!("\u{feff}{}\r\n", linhas.join("\r\n"))
}

// ---------------------------------------------------------------------------
// Ferramentas

#[derive(Debug, Deserialize)]
struct CondicaoCitada {
    texto: String,
    #[serde(default)]
    evidencias: Vec<EvidenciaCitada>,
}

#[derive(Debug, Deserialize)]
struct ColunaCitada {
    valores: Vec<String>,
    #[serde(default)]
    acoes: Vec<u32>,
    #[serde(default)]
    impossivel: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ArgsTabela {
    analise: String,
    titulo: String,
    condicoes: Vec<CondicaoCitada>,
    acoes: Vec<String>,
    colunas: Vec<ColunaCitada>,
    #[serde(flatten)]
    fundamentos: FundamentosCitados,
}

#[derive(Debug, Deserialize)]
struct LimiteCitado {
    grandeza: String,
    valor: String,
    evidencias: Vec<EvidenciaCitada>,
    #[serde(default)]
    configuracao: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ArgsCenario {
    analise: String,
    titulo: String,
    tecnica: Tecnica,
    dado: String,
    quando: String,
    entao: String,
    #[serde(default)]
    valor: Option<String>,
    #[serde(default)]
    limite: Option<LimiteCitado>,
    #[serde(default)]
    tabela: Option<String>,
    #[serde(default)]
    historia: Option<String>,
    #[serde(flatten)]
    fundamentos: FundamentosCitados,
    #[serde(default)]
    pergunta: Option<RefCitada>,
}

fn texto(max: u64, descricao: &str) -> Value {
    json!({ "type": "string", "minLength": 1, "maxLength": max, "description": descricao })
}

fn exigir_fluxo(analise: &Analise) -> Result<(), ErroFerramenta> {
    if analise.fluxo == Fluxo::Cenarios {
        Ok(())
    } else {
        Err(comum::recusa(
            cba_core::analise::ErroModelo::ForaDoFluxoDeCenarios(analise.id.clone()),
        ))
    }
}

/// Fundamentos citados, conferidos; recusa o item sem nenhum.
fn fundamentos_do_item(
    f: &mut Fundamentacao<'_>,
    citados: &FundamentosCitados,
    tipo: &'static str,
) -> Result<Vec<Fundamento>, ErroFerramenta> {
    if citados.afirmacoes.is_empty() && citados.regras.is_empty() {
        return Err(comum::recusa(
            cba_core::analise::ErroModelo::ItemSemFundamento(tipo),
        ));
    }
    f.fundamentos(citados)
}

fn evidencias_conferidas(
    ambiente: &cba_core::ambiente::Ambiente,
    citadas: &[EvidenciaCitada],
    onde: &str,
) -> Result<Vec<Evidencia>, ErroFerramenta> {
    evidencias::validar_todas(&ambiente.escopo, citadas)
        .map_err(|m| ErroFerramenta::Recusa(format!("{onde}: {m}")))
}

/// A história citada existe numa análise do projeto (a de origem primeiro) e não
/// foi descartada nem quebrada.
fn conferir_historia(
    ambiente: &cba_core::ambiente::Ambiente,
    analise: &Analise,
    texto: &str,
) -> Result<IdHistoria, ErroFerramenta> {
    let id = IdHistoria::analisar(texto.trim()).map_err(|_| {
        ErroFerramenta::Argumentos(format!(
            "`historia`: `{texto}` não é o identificador de uma história; use HU-NNNN, como \
             HU-0001"
        ))
    })?;
    let na_origem = analise
        .origem
        .as_ref()
        .and_then(|o| ambiente.analises.carregar(o).ok())
        .and_then(|o| o.historia(id).cloned());
    let historia = match na_origem {
        Some(h) => Some(h),
        None => ambiente
            .analises
            .listar()?
            .into_iter()
            .find_map(|a| a.historia(id).cloned()),
    };
    match historia {
        None => Err(ErroFerramenta::Recusa(format!(
            "a história {id} não existe neste projeto; confira o identificador na análise de \
             histórias (`retomar_analise`) ou omita `historia`"
        ))),
        Some(h) if !h.no_documento() => Err(ErroFerramenta::Recusa(format!(
            "a história {id} foi descartada ou quebrada em fatias; ligue o cenário a uma \
             história vigente ou omita `historia`"
        ))),
        Some(_) => Ok(id),
    }
}

fn schema_das_evidencias(descricao: &str) -> Value {
    json!({
        "type": "array",
        "maxItems": 10,
        "items": evidencias::schema_da_evidencia(),
        "description": descricao
    })
}

/// `registrar_tabela_decisao`: tabela de decisão de uma regra com várias
/// condições.
pub fn registrar_tabela_decisao() -> Ferramenta {
    Ferramenta::nova(
        "registrar_tabela_decisao",
        "Registrar tabela de decisão",
        "Registra a tabela de decisão de uma regra com duas ou mais condições \
         combinadas: as condições (com a evidência do trecho que testa cada uma), as \
         ações e as colunas, cada uma com o valor de cada condição (S, N ou - para \
         indiferente) e os números das ações executadas. Combinação que não pode \
         acontecer vira coluna com `impossivel` e a justificativa, sem ações. A tabela \
         precisa cobrir todas as combinações, sem duas colunas para a mesma; tabela \
         incompleta ou ambígua é recusada com as combinações a corrigir. Cite a regra \
         (`regras`) ou a afirmação (`afirmacoes`) de origem. Devolve o número da tabela \
         (TD-n); registre depois um cenário por coluna possível com \
         `registrar_cenario` (tecnica `tabela_de_decisao`, `tabela`: \"TD-n/Rn\").",
        json!({
            "type": "object",
            "properties": {
                "analise": comum::schema_da_analise(),
                "titulo": texto(200, "Regra ou decisão que a tabela descreve (\"Cancelamento de fatura emitida há mais de 30 dias\")."),
                "condicoes": {
                    "type": "array",
                    "minItems": 2,
                    "maxItems": cba_core::cenarios::MAXIMO_DE_CONDICOES,
                    "items": {
                        "type": "object",
                        "properties": {
                            "texto": texto(500, "Condição verdadeira ou falsa (\"fatura emitida há mais de 30 dias\")."),
                            "evidencias": schema_das_evidencias("Trechos do código que testam a condição.")
                        },
                        "required": ["texto"],
                        "additionalProperties": false
                    },
                    "description": "Condições combinadas pela regra (de 2 a 6)."
                },
                "acoes": {
                    "type": "array",
                    "minItems": 1,
                    "maxItems": 10,
                    "items": { "type": "string", "minLength": 1, "maxLength": 500 },
                    "description": "Ações ou resultados possíveis, com a mensagem ou o código de erro quando houver (\"recusa com 'Solicite a um supervisor'\")."
                },
                "colunas": {
                    "type": "array",
                    "minItems": 1,
                    "maxItems": 64,
                    "items": {
                        "type": "object",
                        "properties": {
                            "valores": {
                                "type": "array",
                                "items": { "type": "string", "enum": ["S", "N", "-"] },
                                "description": "Um valor por condição, na ordem: S (sim), N (não) ou - (indiferente)."
                            },
                            "acoes": {
                                "type": "array",
                                "items": { "type": "integer", "minimum": 1 },
                                "description": "Números das ações executadas (a partir de 1)."
                            },
                            "impossivel": texto(1000, "Se a combinação não pode acontecer: a justificativa. Sem ações.")
                        },
                        "required": ["valores"],
                        "additionalProperties": false
                    },
                    "description": "Colunas da tabela (R1, R2...)."
                },
                "afirmacoes": fundamentos::schema_das_afirmacoes(),
                "regras": fundamentos::schema_das_regras()
            },
            "required": ["analise", "titulo", "condicoes", "acoes", "colunas"],
            "additionalProperties": false
        }),
        |ambiente, args| {
            let args: ArgsTabela = ler_argumentos(args)?;
            let id = comum::id_da_analise(&args.analise)?;
            let atual = ambiente.analises.carregar(&id)?;
            exigir_fluxo(&atual)?;
            let mut f = Fundamentacao::nova(ambiente, &atual);
            let fundamentos = fundamentos_do_item(&mut f, &args.fundamentos, "tabela de decisão")?;
            let mut condicoes = Vec::new();
            for (i, c) in args.condicoes.iter().enumerate() {
                condicoes.push(Condicao {
                    texto: c.texto.clone(),
                    evidencias: evidencias_conferidas(
                        ambiente,
                        &c.evidencias,
                        &format!("condição {}", i + 1),
                    )?,
                });
            }
            let mut colunas = Vec::new();
            for (i, c) in args.colunas.iter().enumerate() {
                let valores = c
                    .valores
                    .iter()
                    .map(|v| {
                        ValorCondicao::analisar(v).ok_or_else(|| {
                            ErroFerramenta::Argumentos(format!(
                                "colunas[{i}]: `{v}` não é um valor de condição; use S, N ou -"
                            ))
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                colunas.push(NovaColuna {
                    valores,
                    acoes: c.acoes.clone(),
                    impossivel: c.impossivel.clone(),
                });
            }
            let nova = NovaTabela {
                titulo: args.titulo,
                condicoes,
                acoes: args.acoes,
                colunas,
                fundamentos,
                sessao: None,
            };
            let registrado = comum::alterar(ambiente, &id, Mudanca::Cenario, |a, agora| {
                a.registrar_tabela(nova, agora)
            })?;
            let numero = registrado.resultado;
            let tabela = registrado
                .analise
                .tabela(numero)
                .ok_or_else(|| ErroFerramenta::Interno(format!("TD-{numero} não foi gravada")))?;
            let possiveis: Vec<String> = tabela
                .colunas
                .iter()
                .filter(|c| c.impossivel.is_none())
                .map(|c| format!("TD-{numero}/{}", c.rotulo()))
                .collect();
            let sem_evidencia: Vec<String> = tabela
                .condicoes
                .iter()
                .enumerate()
                .filter(|(_, c)| c.evidencias.is_empty())
                .map(|(i, _)| format!("C{}", i + 1))
                .collect();
            Ok(comum::resposta(
                ambiente,
                &registrado,
                json!({
                    "tabela": tabela.rotulo(),
                    "colunas_para_cenarios": possiveis,
                    "impossiveis": tabela.colunas.iter().filter(|c| c.impossivel.is_some()).map(|c| c.rotulo()).collect::<Vec<_>>(),
                    "aviso": (!sem_evidencia.is_empty()).then(|| format!(
                        "condição(ões) sem evidência do código: {}; cite o trecho que testa cada uma, se houver",
                        sem_evidencia.join(", ")
                    )),
                }),
            ))
        },
    )
}

/// Avisos que não impedem o registro.
fn avisos_do_cenario(c: &Cenario) -> Vec<String> {
    let mut avisos = Vec::new();
    let p = &c.texto.passos;
    let tem_numero = [&p.dado, &p.quando, &p.entao]
        .iter()
        .chain(c.valor.as_ref().iter())
        .any(|t| t.chars().any(|ch| ch.is_ascii_digit()));
    if !tem_numero && c.tecnica != Tecnica::TabelaDeDecisao {
        avisos.push(format!(
            "{}: sem nenhum valor concreto; prefira valores (quantidades, datas, valores em \
             reais) nos passos",
            c.id
        ));
    }
    if let Some(l) = &c.limite
        && l.configuracao.is_none()
        && l.evidencias.iter().any(|e| {
            let t = e.trecho.to_lowercase();
            t.contains("parametr") || t.contains("insert into")
        })
    {
        avisos.push(format!(
            "{}: o limite parece vir de tabela de parâmetros ou de carga de dados; se o valor \
             em produção pode ser outro, informe `configuracao` e a `pergunta` (o cenário \
             fica a confirmar)",
            c.id
        ));
    }
    avisos
}

/// `registrar_cenario`: um cenário de teste com identificador estável.
pub fn registrar_cenario() -> Ferramenta {
    Ferramenta::nova(
        "registrar_cenario",
        "Registrar cenário de teste",
        "Registra um cenário de teste derivado de uma regra: título, técnica, Dado \
         (pré-condição) / Quando (ação) / Então (resultado esperado, com a mensagem ou \
         o código de erro quando houver) com valores concretos e o valor testado. Nas \
         técnicas `valor_limite` e `particao_de_equivalencia`, informe o `limite`: a \
         grandeza, o valor como está no código e a evidência do trecho que o define; o \
         valor precisa aparecer no trecho citado. Se o limite vem de tabela de \
         parâmetros ou de configuração, informe `configuracao` e a `pergunta` (em \
         aberto): o cenário fica \"a confirmar\". Na técnica `tabela_de_decisao`, \
         informe a coluna (`tabela`: \"TD-1/R2\"). Cite a regra (`regras`) ou a \
         afirmação (`afirmacoes`) de origem e, se houver, a história (`historia`, \
         HU-NNNN). Devolve o identificador estável do cenário (CT-NNNN).",
        json!({
            "type": "object",
            "properties": {
                "analise": comum::schema_da_analise(),
                "titulo": texto(200, "Título curto (\"Motivo com 9 caracteres é recusado\")."),
                "tecnica": {
                    "type": "string",
                    "enum": ["tabela_de_decisao", "particao_de_equivalencia", "valor_limite", "negativo", "excecao"],
                    "description": "tabela_de_decisao (uma coluna de uma tabela registrada), particao_de_equivalencia (representante de uma classe), valor_limite (no limite, logo antes ou logo depois), negativo (entrada que o código recusa) ou excecao (situação de exceção que o código trata)."
                },
                "dado": texto(2000, "Pré-condição, com valores (\"uma fatura ABERTA, sem pagamento, emitida há 31 dias\")."),
                "quando": texto(2000, "Ação, com a entrada testada (\"um atendente sem perfil SUPERVISOR pede o cancelamento\")."),
                "entao": texto(2000, "Resultado esperado, com a mensagem ou o código de erro (\"o cancelamento é recusado: 'Solicite a um supervisor'\")."),
                "valor": texto(300, "Valor ou combinação testada (\"31 dias\", \"motivo com 9 caracteres\")."),
                "limite": {
                    "type": "object",
                    "properties": {
                        "grandeza": texto(300, "O que é medido (\"dias desde a emissão da fatura\")."),
                        "valor": texto(100, "O limite como está no código (\"30\")."),
                        "evidencias": schema_das_evidencias("Trechos do código que definem ou aplicam o limite; o valor precisa aparecer num deles."),
                        "configuracao": texto(300, "Se o valor vem de tabela de parâmetros ou de configuração, fora do código: o parâmetro (\"PARAMETRO.DIAS_CARENCIA\"). Exige `pergunta`.")
                    },
                    "required": ["grandeza", "valor", "evidencias"],
                    "additionalProperties": false
                },
                "tabela": {
                    "type": "string",
                    "minLength": 1,
                    "description": "Coluna da tabela de decisão que o cenário exercita: TD-n/Rn (\"TD-1/R2\")."
                },
                "historia": {
                    "type": "string",
                    "minLength": 1,
                    "description": "História que o cenário verifica (HU-NNNN), quando houver."
                },
                "afirmacoes": fundamentos::schema_das_afirmacoes(),
                "regras": fundamentos::schema_das_regras(),
                "pergunta": fundamentos::schema_da_pergunta()
            },
            "required": ["analise", "titulo", "tecnica", "dado", "quando", "entao"],
            "additionalProperties": false
        }),
        |ambiente, args| {
            let args: ArgsCenario = ler_argumentos(args)?;
            let id = comum::id_da_analise(&args.analise)?;
            let atual = ambiente.analises.carregar(&id)?;
            exigir_fluxo(&atual)?;
            let coluna = args
                .tabela
                .as_deref()
                .map(|t| {
                    RefColuna::analisar(t).ok_or_else(|| {
                        ErroFerramenta::Argumentos(format!(
                            "`tabela`: `{t}` não é uma coluna de tabela de decisão; use \
                             TD-n/Rn, como TD-1/R2"
                        ))
                    })
                })
                .transpose()?;
            let historia = args
                .historia
                .as_deref()
                .map(|h| conferir_historia(ambiente, &atual, h))
                .transpose()?;
            let mut f = Fundamentacao::nova(ambiente, &atual);
            let fundamentos = fundamentos_do_item(&mut f, &args.fundamentos, "cenário de teste")?;
            let pergunta = args.pergunta.as_ref().map(|p| f.pergunta(p)).transpose()?;
            let limite = args
                .limite
                .as_ref()
                .map(|l| {
                    Ok::<_, ErroFerramenta>(Limite {
                        grandeza: l.grandeza.clone(),
                        valor: l.valor.clone(),
                        evidencias: evidencias_conferidas(ambiente, &l.evidencias, "limite")?,
                        configuracao: l.configuracao.clone(),
                    })
                })
                .transpose()?;
            let novo = NovoCenario {
                tecnica: args.tecnica,
                texto: TextoCenario {
                    titulo: args.titulo,
                    passos: Gherkin {
                        dado: args.dado,
                        quando: args.quando,
                        entao: args.entao,
                    },
                },
                valor: args.valor,
                limite,
                coluna,
                historia,
                fundamentos,
                pergunta,
                sessao: None,
            };
            let registrado = comum::alterar(ambiente, &id, Mudanca::Cenario, |a, agora| {
                let maior = ambiente
                    .analises
                    .maior_numero(&a.id, Analise::maior_cenario)
                    .max(a.maior_cenario());
                a.registrar_cenario(novo, IdCenario::novo(maior + 1), agora)
            })?;
            let ct = registrado.resultado;
            let cenario = registrado
                .analise
                .cenario(ct)
                .ok_or_else(|| ErroFerramenta::Interno(format!("{ct} não foi gravado")))?;
            Ok(comum::resposta(
                ambiente,
                &registrado,
                json!({
                    "cenario": ct,
                    "tecnica": cenario.tecnica,
                    "a_confirmar": cenario.a_confirmar(),
                    "limite_no_codigo": cenario.limite.as_ref().map(|l| l.configuracao.is_none()),
                    "avisos": avisos_do_cenario(cenario),
                }),
            ))
        },
    )
}

/// Colunas possíveis das tabelas vigentes sem nenhum cenário.
pub fn colunas_sem_cenario(analise: &Analise) -> Vec<String> {
    let mut faltam = Vec::new();
    for t in analise.tabelas_no_documento() {
        for c in t.colunas.iter().filter(|c| c.impossivel.is_none()) {
            if cenarios_da_coluna(analise, t.numero, c.numero).is_empty() {
                faltam.push(format!("TD-{}/{}", t.numero, c.rotulo()));
            }
        }
    }
    faltam
}

/// Resumo das tabelas e dos cenários para `retomar_analise`: texto e dados.
pub fn para_retomar(analise: &Analise) -> (String, Value) {
    let mut t = String::from("## Tabelas de decisão\n\n");
    if analise.tabelas.is_empty() {
        t.push_str("Nenhuma tabela registrada.\n\n");
    }
    for tab in &analise.tabelas {
        t.push_str(&format!(
            "### {} · {} [{}]\n",
            tab.rotulo(),
            tab.titulo,
            situacao(tab.situacao)
        ));
        for (i, c) in tab.condicoes.iter().enumerate() {
            t.push_str(&format!("- C{}: {}\n", i + 1, c.texto));
        }
        for (i, a) in tab.acoes.iter().enumerate() {
            t.push_str(&format!("- A{}: {a}\n", i + 1));
        }
        for c in &tab.colunas {
            let valores: Vec<&str> = c.valores.iter().map(|v| v.simbolo()).collect();
            match &c.impossivel {
                Some(j) => t.push_str(&format!(
                    "- {} [{}]: impossível ({j})\n",
                    c.rotulo(),
                    valores.join("/")
                )),
                None => t.push_str(&format!(
                    "- {} [{}]: ações {:?}\n",
                    c.rotulo(),
                    valores.join("/"),
                    c.acoes
                )),
            }
        }
        t.push('\n');
    }
    t.push_str("## Cenários de teste\n\n");
    if analise.cenarios.is_empty() {
        t.push_str("Nenhum cenário registrado.\n\n");
    }
    for c in &analise.cenarios {
        let origem: Vec<String> = c.fundamentos.iter().map(Fundamento::rotulo).collect();
        t.push_str(&format!(
            "- {} · {} [{}; {}{}{}] Dado {}; quando {}; então {}. Origem: {}\n",
            c.id,
            c.texto.titulo,
            c.tecnica.rotulo(),
            situacao(c.situacao),
            c.coluna
                .map(|r| format!("; coluna {r}"))
                .unwrap_or_default(),
            c.pergunta
                .as_ref()
                .map(|p| format!("; a confirmar: pergunta {p}"))
                .unwrap_or_default(),
            c.texto.passos.dado,
            c.texto.passos.quando,
            c.texto.passos.entao,
            origem.join(", ")
        ));
    }
    let faltam = colunas_sem_cenario(analise);
    if !faltam.is_empty() {
        t.push_str(&format!("\nColunas sem cenário: {}.\n", faltam.join(", ")));
    }
    t.push('\n');
    let dados = json!({
        "tabelas": analise.tabelas.iter().map(|tab| json!({
            "tabela": tab.rotulo(),
            "titulo": tab.titulo,
            "situacao": tab.situacao,
            "colunas": tab.colunas.len(),
        })).collect::<Vec<_>>(),
        "cenarios": analise.cenarios.iter().map(|c| json!({
            "id": c.id,
            "titulo": c.texto.titulo,
            "tecnica": c.tecnica,
            "situacao": c.situacao,
            "a_confirmar": c.a_confirmar(),
            "coluna": c.coluna.map(|r| r.to_string()),
            "fundamentos": c.fundamentos.iter().map(Fundamento::rotulo).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "colunas_sem_cenario": faltam,
    });
    (t, dados)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cba_core::analise::EstadoEvidencia;
    use cba_core::cenarios::NovaColuna;
    use cba_kb::frontmatter::Linhas;
    use cba_kb::{Id, Serie};

    fn evidencia(arquivo: &str, l: u32, trecho: &str) -> Evidencia {
        Evidencia {
            arquivo: CaminhoRelativo::novo(arquivo).unwrap(),
            linhas: Linhas::novo(l, l).unwrap(),
            trecho: trecho.into(),
            hash: "abc".into(),
            commit: None,
            estado: EstadoEvidencia::Valida,
        }
    }

    fn regra() -> Fundamento {
        Fundamento::Regra {
            id: Id::analisar("RN-FAT-014").unwrap(),
            titulo: "Cancelamento após 30 dias exige supervisor".into(),
            evidencias: vec![],
        }
    }

    fn analise() -> Analise {
        let mut a = Analise::nova(
            Serie::Analise.id(2),
            "faturamento-exemplo",
            Fluxo::Cenarios,
            "Cenários do cancelamento",
            "Cenários das regras do cancelamento",
            1,
        );
        let v = |s: &str| {
            s.split('/')
                .map(|x| ValorCondicao::analisar(x).unwrap())
                .collect()
        };
        a.registrar_tabela(
            NovaTabela {
                titulo: "Cancelamento após 30 dias".into(),
                condicoes: vec![
                    Condicao {
                        texto: "emitida há mais de 30 dias".into(),
                        evidencias: vec![evidencia(
                            "delphi/UFrmCancelamento.pas",
                            70,
                            "DIAS_LIMITE_SEM_SUPERVISOR",
                        )],
                    },
                    Condicao {
                        texto: "perfil SUPERVISOR".into(),
                        evidencias: vec![],
                    },
                ],
                acoes: vec!["cancela".into(), "recusa | pede supervisor".into()],
                colunas: vec![
                    NovaColuna {
                        valores: v("S/S"),
                        acoes: vec![1],
                        impossivel: None,
                    },
                    NovaColuna {
                        valores: v("S/N"),
                        acoes: vec![2],
                        impossivel: None,
                    },
                    NovaColuna {
                        valores: v("N/-"),
                        acoes: vec![1],
                        impossivel: None,
                    },
                ],
                fundamentos: vec![regra()],
                sessao: None,
            },
            1,
        )
        .unwrap();
        let passos = |titulo: &str| TextoCenario {
            titulo: titulo.into(),
            passos: Gherkin {
                dado: "uma fatura ABERTA emitida há 31 dias".into(),
                quando: "um atendente pede o cancelamento, motivo \"erro, de digitação\"".into(),
                entao: "é recusado".into(),
            },
        };
        a.registrar_cenario(
            NovoCenario {
                tecnica: Tecnica::ValorLimite,
                texto: passos("31 dias sem supervisor"),
                valor: Some("31 dias".into()),
                limite: Some(Limite {
                    grandeza: "dias desde a emissão".into(),
                    valor: "30".into(),
                    evidencias: vec![evidencia(
                        "delphi/UFrmCancelamento.pas",
                        41,
                        "DIAS_LIMITE_SEM_SUPERVISOR = 30;",
                    )],
                    configuracao: None,
                }),
                coluna: RefColuna::analisar("TD-1/R2"),
                historia: Some(IdHistoria::novo(4)),
                fundamentos: vec![regra()],
                pergunta: None,
                sessao: None,
            },
            IdCenario::novo(7),
            2,
        )
        .unwrap();
        a.registrar_cenario(
            NovoCenario {
                tecnica: Tecnica::Negativo,
                texto: passos("Descartado"),
                valor: None,
                limite: None,
                coluna: None,
                historia: None,
                fundamentos: vec![regra()],
                pergunta: Some(cba_core::requisito::RefItem::analisar("A-0001/2").unwrap()),
                sessao: None,
            },
            IdCenario::novo(8),
            3,
        )
        .unwrap();
        a
    }

    #[test]
    fn documento_tem_tabela_limites_cenarios_e_rastreabilidade() {
        let mut a = analise();
        let textos = textos(&a, &a.documento);
        let tabela = &textos[0].texto;
        assert!(
            tabela.starts_with("### TD-1 · Cancelamento após 30 dias"),
            "{tabela}"
        );
        assert!(tabela.contains("|  | R1 | R2 | R3 |"), "{tabela}");
        assert!(
            tabela.contains("| **C1** emitida há mais de 30 dias | S | S | N |"),
            "{tabela}"
        );
        assert!(
            tabela.contains("| **C2** perfil SUPERVISOR | S | N | — |"),
            "{tabela}"
        );
        assert!(
            tabela.contains("| **A2** recusa \\| pede supervisor |  | X |  |"),
            "{tabela}"
        );
        assert!(
            tabela.contains("| **Cenário** | — | CT-0007 | — |"),
            "{tabela}"
        );
        assert_eq!(textos[2].citacoes.len(), 1);
        let limites = textos.iter().find(|t| t.secao == VALORES_LIMITE).unwrap();
        assert!(
            limites
                .texto
                .contains("| CT-0007 | valor-limite | dias desde a emissão | **30** | 31 dias |"),
            "{}",
            limites.texto
        );
        let ct = textos.iter().find(|t| t.secao == CENARIOS).unwrap();
        assert!(
            ct.texto
                .contains("**Coluna:** TD-1/R2 · **História:** HU-0004")
        );
        assert!(
            ct.texto
                .contains("**Limite:** **30** (dias desde a emissão, lido do código)")
        );
        assert!(ct.texto.contains("[RN-FAT-014](../regras/RN-FAT-014.md)"));
        let rast = textos.iter().find(|t| t.secao == RASTREABILIDADE).unwrap();
        assert!(
            rast.texto
                .contains("| CT-0008 | negativo | RN-FAT-014 | — | — | a confirmar (A-0001/2) |"),
            "{}",
            rast.texto
        );

        a.decidir_cenario(IdCenario::novo(8), SituacaoItem::Descartado)
            .unwrap();
        assert_eq!(
            textos_count(&a, CENARIOS),
            1,
            "o descartado sai do documento"
        );
        assert_eq!(colunas_sem_cenario(&a), ["TD-1/R1", "TD-1/R3"]);
    }

    fn textos_count(a: &Analise, secao: &str) -> usize {
        textos(a, &a.documento)
            .iter()
            .filter(|t| t.secao == secao)
            .count()
    }

    #[test]
    fn csv_uma_linha_por_cenario() {
        let a = analise();
        let csv = csv(&a);
        assert!(csv.starts_with('\u{feff}'));
        let linhas: Vec<&str> = csv
            .trim_start_matches('\u{feff}')
            .trim_end()
            .split("\r\n")
            .collect();
        assert_eq!(linhas.len(), 3);
        assert_eq!(linhas[0], COLUNAS_DO_CSV.join(","));
        assert!(
            linhas[1].starts_with("CT-0007,31 dias sem supervisor,valor-limite,sem decisão,"),
            "{}",
            linhas[1]
        );
        assert!(
            linhas[1].contains(
                ",\"um atendente pede o cancelamento, motivo \"\"erro, de digitação\"\"\","
            ),
            "{}",
            linhas[1]
        );
        assert!(linhas[1].contains(",30,dias desde a emissão,TD-1/R2,RN-FAT-014,HU-0004,delphi/UFrmCancelamento.pas:41,"), "{}", linhas[1]);
        assert!(linhas[2].ends_with(",pergunta A-0001/2"), "{}", linhas[2]);
    }

    #[test]
    fn retomar_lista_tabelas_e_cenarios() {
        let (texto, dados) = para_retomar(&analise());
        assert!(texto.contains("### TD-1 · Cancelamento após 30 dias [sem decisão]"));
        assert!(texto.contains("- R3 [N/—]: ações [1]"));
        assert!(texto.contains(
            "- CT-0007 · 31 dias sem supervisor [valor-limite; sem decisão; coluna TD-1/R2]"
        ));
        assert!(texto.contains("Colunas sem cenário: TD-1/R1, TD-1/R3."));
        assert_eq!(dados["cenarios"][1]["a_confirmar"], true);
    }
}
