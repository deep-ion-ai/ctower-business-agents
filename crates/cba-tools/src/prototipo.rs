//! Fluxo "Protótipo de telas" (#381): `registrar_tela` e
//! `registrar_elemento_tela`, e as seções do documento montadas a partir das
//! telas.
//!
//! - `registrar_tela` registra uma tela afetada pela análise de origem: o nome,
//!   se é nova ou alterada, o motivo ligado à origem e, se o agente já souber, os
//!   elementos (campos, validações, mensagens e ações). O número `TL-n` é o
//!   próximo da análise.
//! - `registrar_elemento_tela` acrescenta um elemento a uma tela já registrada.
//! - `gravar_html_tela` grava o protótipo HTML de uma tela (#385): o agente
//!   escreve o corpo, com uma seção por estado (vazio, preenchido, erro de
//!   validação e sucesso) e os elementos marcados com `data-elemento`; o app
//!   confere ([`cba_core::prototipo_html::conferir_corpo`]), monta o documento
//!   com os tokens CSS aprovados ou o wireframe neutro e aponta
//!   [`cba_core::prototipo::Tela::html`] para o arquivo.
//!
//! Tela e elemento citam o que os sustenta: `afirmacoes` e `regras`, conferidas
//! por [`crate::fundamentos`], e `criterios` de aceite (`HU-NNNN/CA-n`), conferidos
//! nas análises de histórias do projeto ([`Criterios`]). Referência que não existe
//! é recusada; sem nenhuma referência, o item é aceito e fica "a confirmar".
//!
//! [`textos`] converte as telas nas seções `telas` e `rastreabilidade` do
//! documento (template `prototipo`), com as evidências das afirmações e das
//! regras citadas como notas de rodapé, e o link para o protótipo HTML de cada
//! tela.

use cba_core::ambiente::Ambiente;
use cba_core::analise::EstadoAnalise;
use cba_core::analise::{Analise, ErroModelo, Fluxo};
use cba_core::eventos::Mudanca;
use cba_core::historias::SituacaoHistoria;
use cba_core::prototipo::{
    CriterioCitado, ElementoTela, MAXIMO_DE_ELEMENTOS, MudancaTela, NovaTela, NovoElemento, Rastro,
    Tela, TipoElemento,
};
use cba_core::prototipo_html::{self, EstadoTela, TAMANHO_MAXIMO_DO_CORPO, Visual};
use cba_core::requisito::{Fundamento, SituacaoItem};
use cba_core::tokens::{EstadoTokens, TokensProjeto};
use cba_docs::{Citacao, RASTREABILIDADE, TELAS, Texto};
use cba_kb::{CaminhoRelativo, IdHistoria};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::comum;
use crate::fundamentos::{self, Fundamentacao, RefCitada};
use crate::registro::{ErroFerramenta, Ferramenta, ler_argumentos};

// ---------------------------------------------------------------------------
// Documento

/// Texto numa linha só.
fn em_uma_linha(texto: &str) -> String {
    texto.split_whitespace().collect::<Vec<_>>().join(" ")
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

/// Origem de uma tela ou de um elemento, para o documento: uma linha por
/// referência ou o marcador "a confirmar".
fn linhas_da_origem(r: &Rastro, caminho: &CaminhoRelativo) -> Vec<String> {
    let mut linhas: Vec<String> = r
        .fundamentos
        .iter()
        .map(|f| {
            format!(
                "{}: {}",
                link_do_fundamento(f, caminho),
                em_uma_linha(f.texto())
            )
        })
        .collect();
    linhas.extend(r.criterios.iter().map(|c| {
        format!(
            "{}: {}",
            cba_docs::link(&c.rotulo(), caminho, &c.analise.arquivo()),
            em_uma_linha(&c.texto)
        )
    }));
    linhas
}

/// Marca "a confirmar" com o porquê.
fn marca_a_confirmar(r: &Rastro) -> Option<String> {
    match (&r.pergunta, r.sem_fundamento()) {
        (Some(p), _) => Some(format!("_a confirmar: depende da pergunta {p}_")),
        (None, true) => Some("_a confirmar: sem fundamento na análise de origem_".into()),
        (None, false) => None,
    }
}

fn citacoes(tela: &Tela) -> Vec<Citacao> {
    let mut lista: Vec<Citacao> = Vec::new();
    let fundamentos = tela
        .rastro
        .fundamentos
        .iter()
        .chain(tela.elementos.iter().flat_map(|e| &e.rastro.fundamentos));
    for e in fundamentos.flat_map(Fundamento::evidencias) {
        let c = Citacao::from(e);
        if !lista
            .iter()
            .any(|x| x.arquivo == c.arquivo && x.linhas == c.linhas)
        {
            lista.push(c);
        }
    }
    lista
}

fn texto_do_elemento(e: &ElementoTela, caminho: &CaminhoRelativo) -> String {
    let mut t = format!(
        "- **{}** · {} · {}",
        e.rotulo(),
        e.tipo.rotulo(),
        em_uma_linha(&e.nome)
    );
    if let Some(d) = &e.descricao {
        t.push_str(&format!(": {}", em_uma_linha(d)));
    }
    if let Some(marca) = marca_a_confirmar(&e.rastro) {
        t.push_str(&format!(" · {marca}"));
    }
    for linha in linhas_da_origem(&e.rastro, caminho) {
        t.push_str(&format!("\n  - {linha}"));
    }
    t
}

fn texto_da_tela(tela: &Tela, caminho: &CaminhoRelativo) -> Texto {
    let mut t = format!(
        "### {} · {} ({})\n\n**Motivo:** {}",
        tela.rotulo(),
        em_uma_linha(&tela.nome),
        tela.mudanca.rotulo(),
        em_uma_linha(&tela.motivo)
    );
    if let Some(marca) = marca_a_confirmar(&tela.rastro) {
        t.push_str(&format!(" · {marca}"));
    }
    let origem = linhas_da_origem(&tela.rastro, caminho);
    if !origem.is_empty() {
        let itens: Vec<String> = origem.iter().map(|l| format!("- {l}")).collect();
        t.push_str(&format!("\n\n_Origem:_\n\n{}", itens.join("\n")));
    }
    if let Some(html) = &tela.html {
        t.push_str(&format!(
            "\n\n**Protótipo:** {}",
            cba_docs::link(&format!("{}.html", tela.rotulo()), caminho, html)
        ));
    }
    t.push_str("\n\n#### Elementos\n\n");
    if tela.elementos.is_empty() {
        t.push_str("_Nenhum elemento registrado._");
    } else {
        let itens: Vec<String> = tela
            .elementos
            .iter()
            .map(|e| texto_do_elemento(e, caminho))
            .collect();
        t.push_str(&itens.join("\n"));
    }
    Texto {
        secao: TELAS.into(),
        texto: t,
        citacoes: citacoes(tela),
    }
}

/// Observação da linha da rastreabilidade.
fn observacao(r: &Rastro) -> String {
    match (&r.pergunta, r.sem_fundamento()) {
        (Some(p), _) => format!("a confirmar ({p})"),
        (None, true) => "a confirmar (sem fundamento)".into(),
        (None, false) => String::new(),
    }
}

fn tabela(analise: &Analise) -> Option<String> {
    let mut linhas = Vec::new();
    for tela in analise.telas() {
        linhas.push(format!(
            "| {} | motivo ({}) | {} | {} |",
            tela.rotulo(),
            tela.mudanca.rotulo(),
            tela.rastro.rotulos().join(", "),
            observacao(&tela.rastro)
        ));
        for e in &tela.elementos {
            linhas.push(format!(
                "| {} | {} ({}) | {} | {} |",
                tela.rotulo(),
                e.rotulo(),
                e.tipo.rotulo(),
                e.rastro.rotulos().join(", "),
                observacao(&e.rastro)
            ));
        }
    }
    (!linhas.is_empty()).then(|| {
        format!(
            "| Tela | Elemento | Afirmações, regras e critérios | Observação |\n\
             |---|---|---|---|\n{}",
            linhas.join("\n")
        )
    })
}

/// Textos das seções `telas` e `rastreabilidade` do documento. `caminho` é o
/// documento da análise, para os links relativos.
pub fn textos(analise: &Analise, caminho: &CaminhoRelativo) -> Vec<Texto> {
    let mut textos: Vec<Texto> = analise
        .telas()
        .iter()
        .map(|t| texto_da_tela(t, caminho))
        .collect();
    if let Some(t) = tabela(analise) {
        textos.push(Texto::novo(RASTREABILIDADE, t));
    }
    textos
}

/// No fluxo `prototipo`, o conteúdo são as telas: ao menos uma, cada uma com
/// ao menos um elemento e com o protótipo HTML gravado, com os quatro estados e
/// todos os elementos marcados (um elemento registrado depois do HTML pede o HTML
/// de novo). Usada por `concluir_analise`.
pub fn conferir_para_concluir(
    kb: &cba_kb::Projeto,
    analise: &Analise,
) -> Result<(), ErroFerramenta> {
    let id = &analise.id;
    if analise.telas().is_empty() {
        return Err(ErroFerramenta::Recusa(format!(
            "a análise {id} ainda não tem telas. Registre cada tela afetada com \
             `registrar_tela` (com os elementos) e chame `concluir_analise` de novo"
        )));
    }
    let sem: Vec<String> = analise
        .telas()
        .iter()
        .filter(|t| t.elementos.is_empty())
        .map(Tela::rotulo)
        .collect();
    if !sem.is_empty() {
        return Err(ErroFerramenta::Recusa(format!(
            "tela(s) sem elemento: {}. Registre os campos, as validações, as mensagens e as \
             ações de cada uma com `registrar_elemento_tela` e chame `concluir_analise` de novo",
            sem.join(", ")
        )));
    }
    let mut problemas = Vec::new();
    for tela in analise.telas() {
        let Some(html) = &tela.html else {
            problemas.push(format!("{}: sem protótipo HTML", tela.rotulo()));
            continue;
        };
        let Ok(texto) = std::fs::read_to_string(kb.caminho(html)) else {
            problemas.push(format!(
                "{}: o arquivo {} não foi encontrado",
                tela.rotulo(),
                kb.na_base(html)
            ));
            continue;
        };
        let estados = prototipo_html::estados_presentes(&texto);
        let faltam: Vec<&str> = EstadoTela::TODOS
            .into_iter()
            .filter(|e| !estados.contains(e))
            .map(EstadoTela::valor)
            .collect();
        if !faltam.is_empty() {
            problemas.push(format!(
                "{}: falta(m) o(s) estado(s) {}",
                tela.rotulo(),
                faltam.join(", ")
            ));
        }
        let marcados = prototipo_html::elementos_marcados(&texto);
        let sem_marca: Vec<String> = tela
            .elementos
            .iter()
            .filter(|e| !marcados.contains(&e.numero))
            .map(ElementoTela::rotulo)
            .collect();
        if !sem_marca.is_empty() {
            problemas.push(format!(
                "{}: elemento(s) fora do HTML: {}",
                tela.rotulo(),
                sem_marca.join(", ")
            ));
        }
    }
    if !problemas.is_empty() {
        return Err(ErroFerramenta::Recusa(format!(
            "o protótipo HTML das telas não está completo: {}. Grave o HTML de cada tela com \
             `gravar_html_tela` (os quatro estados, cada elemento marcado com \
             `data-elemento=\"E-n\"`) e chame `concluir_analise` de novo",
            problemas.join("; ")
        )));
    }
    Ok(())
}

/// Resumo do protótipo para `retomar_analise`: texto e dados.
pub fn para_retomar(analise: &Analise) -> (String, Value) {
    let mut t = String::from("## Protótipo\n\n");
    if let Some(p) = &analise.prototipo {
        t.push_str(&format!("Pedido do analista: {}\n\n", p.pedido.descrever()));
    }
    if analise.telas().is_empty() {
        t.push_str("Nenhuma tela registrada.\n\n");
    }
    let mut dados = Vec::new();
    for tela in analise.telas() {
        t.push_str(&format!(
            "### {} · {} [{}]\n\nMotivo: {}. Origem: {}{}\n",
            tela.rotulo(),
            tela.nome,
            tela.mudanca.rotulo(),
            tela.motivo.trim_end_matches('.'),
            rotulos_ou_nada(&tela.rastro),
            a_confirmar_curto(&tela.rastro)
        ));
        match &tela.html {
            Some(html) => t.push_str(&format!("Protótipo HTML: `{html}`.\n")),
            None => t.push_str("Protótipo HTML: ainda não gravado (`gravar_html_tela`).\n"),
        }
        for e in &tela.elementos {
            t.push_str(&format!(
                "- {} [{}] {}{}. Origem: {}{}\n",
                e.rotulo(),
                e.tipo.rotulo(),
                e.nome,
                e.descricao
                    .as_deref()
                    .map(|d| format!(": {d}"))
                    .unwrap_or_default(),
                rotulos_ou_nada(&e.rastro),
                a_confirmar_curto(&e.rastro)
            ));
        }
        t.push('\n');
        dados.push(json!({
            "tela": tela.rotulo(),
            "nome": tela.nome,
            "mudanca": tela.mudanca,
            "a_confirmar": tela.a_confirmar(),
            "fundamentos": tela.rastro.rotulos(),
            "html": tela.html,
            "elementos": tela.elementos.iter().map(|e| json!({
                "elemento": e.rotulo(),
                "tipo": e.tipo,
                "nome": e.nome,
                "a_confirmar": e.a_confirmar(),
                "fundamentos": e.rastro.rotulos(),
            })).collect::<Vec<_>>(),
        }));
    }
    (t, Value::Array(dados))
}

fn rotulos_ou_nada(r: &Rastro) -> String {
    let rotulos = r.rotulos();
    if rotulos.is_empty() {
        "nenhuma".into()
    } else {
        rotulos.join(", ")
    }
}

fn a_confirmar_curto(r: &Rastro) -> String {
    match &r.pergunta {
        Some(p) => format!("; a confirmar: pergunta {p}"),
        None if r.sem_fundamento() => "; a confirmar".into(),
        None => String::new(),
    }
}

// ---------------------------------------------------------------------------
// Critérios de aceite citados

/// `HU-0001/CA-2` como o agente cita.
fn ler_criterio(texto: &str) -> Result<(IdHistoria, u32), ErroFerramenta> {
    let erro = || {
        ErroFerramenta::Argumentos(format!(
            "`criterios`: `{texto}` não é um critério de aceite; use HU-NNNN/CA-n, como \
             HU-0001/CA-2"
        ))
    };
    let (historia, criterio) = texto.trim().split_once('/').ok_or_else(erro)?;
    let historia = IdHistoria::analisar(historia.trim()).map_err(|_| erro())?;
    let numero = criterio
        .trim()
        .strip_prefix("CA-")
        .and_then(|n| n.parse::<u32>().ok())
        .filter(|n| *n > 0)
        .ok_or_else(erro)?;
    Ok((historia, numero))
}

/// Confere os critérios de aceite citados nas análises de histórias do projeto,
/// lidas uma vez.
struct Criterios<'a> {
    ambiente: &'a Ambiente,
    historias: Option<Vec<Analise>>,
}

impl<'a> Criterios<'a> {
    fn novos(ambiente: &'a Ambiente) -> Self {
        Self {
            ambiente,
            historias: None,
        }
    }

    fn analises(&mut self) -> Result<&[Analise], ErroFerramenta> {
        if self.historias.is_none() {
            let todas = self
                .ambiente
                .analises
                .listar()
                .map_err(|e| ErroFerramenta::Interno(e.to_string()))?;
            self.historias = Some(
                todas
                    .into_iter()
                    .filter(|a| a.fluxo == Fluxo::Historias)
                    .collect(),
            );
        }
        Ok(self.historias.as_deref().unwrap_or_default())
    }

    fn criterio(&mut self, texto: &str) -> Result<CriterioCitado, ErroFerramenta> {
        let (hu, numero) = ler_criterio(texto)?;
        let analises = self.analises()?;
        let (analise, h) = analises
            .iter()
            .find_map(|a| a.historia(hu).map(|h| (a, h)))
            .ok_or_else(|| {
                ErroFerramenta::Recusa(format!(
                    "a história {hu} não existe em nenhuma análise de histórias do projeto; \
                     confira o identificador com `retomar_analise` na análise de histórias"
                ))
            })?;
        if matches!(
            h.situacao,
            SituacaoHistoria::Descartada | SituacaoHistoria::Quebrada
        ) {
            return Err(ErroFerramenta::Recusa(format!(
                "a história {hu} foi descartada ou quebrada em fatias e não sustenta o \
                 protótipo; cite a história que a substituiu"
            )));
        }
        let c = h
            .criterios
            .iter()
            .find(|c| c.numero == numero)
            .ok_or_else(|| {
                ErroFerramenta::Recusa(format!(
                    "o critério CA-{numero} não existe na história {hu}: ela tem {} \
                     critério(s)",
                    h.criterios.len()
                ))
            })?;
        if c.situacao == SituacaoItem::Descartado {
            return Err(ErroFerramenta::Recusa(format!(
                "o critério {hu}/CA-{numero} foi descartado pelo analista e não sustenta o \
                 protótipo"
            )));
        }
        let p = &c.passos;
        Ok(CriterioCitado {
            analise: analise.id.clone(),
            historia: hu,
            criterio: numero,
            texto: format!(
                "Dado {}, quando {}, então {}",
                em_uma_linha(&p.dado),
                em_uma_linha(&p.quando),
                em_uma_linha(p.entao.trim_end_matches('.'))
            ),
        })
    }
}

// ---------------------------------------------------------------------------
// Ferramentas

/// O que o agente citou para sustentar uma tela ou um elemento.
#[derive(Debug, Default, Deserialize)]
struct RastroCitado {
    #[serde(default)]
    afirmacoes: Vec<RefCitada>,
    #[serde(default)]
    regras: Vec<String>,
    #[serde(default)]
    criterios: Vec<String>,
    #[serde(default)]
    pergunta: Option<RefCitada>,
}

#[derive(Debug, Deserialize)]
struct ElementoArgs {
    tipo: TipoElemento,
    nome: String,
    #[serde(default)]
    descricao: Option<String>,
    #[serde(flatten)]
    rastro: RastroCitado,
}

#[derive(Debug, Deserialize)]
struct ArgsTela {
    analise: String,
    nome: String,
    mudanca: MudancaTela,
    motivo: String,
    #[serde(flatten)]
    rastro: RastroCitado,
    #[serde(default)]
    elementos: Vec<ElementoArgs>,
}

#[derive(Debug, Deserialize)]
struct ArgsElemento {
    analise: String,
    tela: RefCitada,
    #[serde(flatten)]
    elemento: ElementoArgs,
}

fn texto(max: u64, descricao: &str) -> Value {
    json!({ "type": "string", "minLength": 1, "maxLength": max, "description": descricao })
}

fn schema_dos_criterios() -> Value {
    json!({
        "type": "array",
        "maxItems": 20,
        "items": { "type": "string", "minLength": 1, "maxLength": 40 },
        "description": "Critérios de aceite que sustentam o item, de uma análise de histórias do projeto: HU-NNNN/CA-n (\"HU-0001/CA-2\")."
    })
}

/// Propriedades de referência, comuns à tela e ao elemento.
fn propriedades_do_rastro(m: &mut serde_json::Map<String, Value>) {
    m.insert("afirmacoes".into(), fundamentos::schema_das_afirmacoes());
    m.insert("regras".into(), fundamentos::schema_das_regras());
    m.insert("criterios".into(), schema_dos_criterios());
    m.insert("pergunta".into(), fundamentos::schema_da_pergunta());
}

fn propriedades_do_elemento() -> serde_json::Map<String, Value> {
    let mut m = serde_json::Map::new();
    m.insert(
        "tipo".into(),
        json!({
            "type": "string",
            "enum": ["campo", "validacao", "mensagem", "acao"],
            "description": "campo (entrada ou exibição), validacao (regra aplicada a campos), mensagem (erro, alerta, confirmação) ou acao (botão, link, comando)."
        }),
    );
    m.insert(
        "nome".into(),
        texto(
            200,
            "Nome do elemento como aparece na tela (\"Número de parcelas\", \"Simular\").",
        ),
    );
    m.insert(
        "descricao".into(),
        texto(
            2000,
            "Detalhe: formato e valores do campo, condição da validação ou texto exato da mensagem, com os valores lidos do código.",
        ),
    );
    propriedades_do_rastro(&mut m);
    m
}

fn schema_do_elemento() -> Value {
    json!({
        "type": "object",
        "properties": propriedades_do_elemento(),
        "required": ["tipo", "nome"],
        "additionalProperties": false
    })
}

fn exigir_fluxo(analise: &Analise) -> Result<(), ErroFerramenta> {
    if analise.fluxo == Fluxo::Prototipo {
        Ok(())
    } else {
        Err(comum::recusa(ErroModelo::ForaDoFluxoDePrototipo(
            analise.id.clone(),
        )))
    }
}

/// Confere as referências citadas: afirmações, regras, critérios e a pergunta.
/// Nenhuma referência é aceito (o item fica "a confirmar").
fn conferir_rastro(
    f: &mut Fundamentacao<'_>,
    criterios: &mut Criterios<'_>,
    citado: &RastroCitado,
) -> Result<Rastro, ErroFerramenta> {
    let mut fundamentos = Vec::new();
    for a in &citado.afirmacoes {
        fundamentos.push(f.afirmacao(a)?);
    }
    for r in &citado.regras {
        fundamentos.push(f.regra(r)?);
    }
    let criterios = citado
        .criterios
        .iter()
        .map(|c| criterios.criterio(c))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Rastro {
        fundamentos,
        criterios,
        pergunta: citado
            .pergunta
            .as_ref()
            .map(|p| f.pergunta(p))
            .transpose()?,
    })
}

fn novo_elemento(
    f: &mut Fundamentacao<'_>,
    criterios: &mut Criterios<'_>,
    e: &ElementoArgs,
) -> Result<NovoElemento, ErroFerramenta> {
    Ok(NovoElemento {
        tipo: e.tipo,
        nome: e.nome.clone(),
        descricao: e.descricao.clone(),
        rastro: conferir_rastro(f, criterios, &e.rastro)?,
        sessao: None,
    })
}

/// Número da tela citada: `TL-n` ou o número.
fn numero_da_tela(citada: &RefCitada) -> Result<u32, ErroFerramenta> {
    let erro = |t: &str| {
        ErroFerramenta::Argumentos(format!(
            "`tela`: `{t}` não é uma tela; use TL-n, como TL-1, o identificador devolvido por \
             registrar_tela"
        ))
    };
    match citada {
        RefCitada::Numero(n) if *n > 0 => Ok(*n),
        RefCitada::Numero(n) => Err(erro(&n.to_string())),
        RefCitada::Texto(t) => {
            let limpo = t.trim();
            limpo
                .strip_prefix("TL-")
                .unwrap_or(limpo)
                .parse::<u32>()
                .ok()
                .filter(|n| *n > 0)
                .ok_or_else(|| erro(t))
        }
    }
}

fn rotulos_a_confirmar(tela: &Tela) -> Vec<String> {
    let mut lista: Vec<String> = Vec::new();
    if tela.a_confirmar() {
        lista.push(format!("{} (motivo)", tela.rotulo()));
    }
    lista.extend(
        tela.elementos_a_confirmar()
            .map(|e| format!("{}/{}", tela.rotulo(), e.rotulo())),
    );
    lista
}

/// `registrar_tela`: uma tela afetada, com os elementos.
pub fn registrar_tela() -> Ferramenta {
    let mut propriedades = serde_json::Map::new();
    propriedades.insert("analise".into(), comum::schema_da_analise());
    propriedades.insert(
        "nome".into(),
        texto(
            200,
            "Nome da tela como o usuário a conhece (\"Parcelar fatura\").",
        ),
    );
    propriedades.insert(
        "mudanca".into(),
        json!({
            "type": "string",
            "enum": ["nova", "alterada"],
            "description": "nova (a tela não existe hoje) ou alterada (a tela existe e muda com a demanda)."
        }),
    );
    propriedades.insert(
        "motivo".into(),
        texto(
            2000,
            "Por que a tela entra no protótipo, a partir da análise de origem.",
        ),
    );
    propriedades_do_rastro(&mut propriedades);
    propriedades.insert(
        "elementos".into(),
        json!({
            "type": "array",
            "maxItems": MAXIMO_DE_ELEMENTOS,
            "items": schema_do_elemento(),
            "description": "Campos, validações, mensagens e ações da tela, cada um com a afirmação, a regra ou o critério de aceite de onde vem. Sem nenhuma referência, o elemento fica \"a confirmar\"."
        }),
    );
    Ferramenta::nova(
        "registrar_tela",
        "Registrar tela do protótipo",
        "Registra uma tela afetada na análise de protótipo: o nome, se é `nova` ou \
         `alterada`, o motivo ligado à análise de origem e os elementos (campos, \
         validações, mensagens e ações). A tela e cada elemento citam o que os sustenta: \
         `afirmacoes` (o número da afirmação na análise de origem, ou A-NNNN/n), \
         `regras` (RN-<SIGLA>-NNN) e `criterios` (HU-NNNN/CA-n, de uma análise de \
         histórias). Referência que não existe é recusada; item sem nenhuma referência, \
         ou com `pergunta` em aberto, fica \"a confirmar\". Devolve o identificador da tela \
         (TL-n); acrescente elementos depois com `registrar_elemento_tela`.",
        json!({
            "type": "object",
            "properties": propriedades,
            "required": ["analise", "nome", "mudanca", "motivo"],
            "additionalProperties": false
        }),
        |ambiente, args| {
            let args: ArgsTela = ler_argumentos(args)?;
            let id = comum::id_da_analise(&args.analise)?;
            let atual = ambiente.analises.carregar(&id)?;
            exigir_fluxo(&atual)?;
            let mut f = Fundamentacao::nova(ambiente, &atual);
            let mut c = Criterios::novos(ambiente);
            let rastro = conferir_rastro(&mut f, &mut c, &args.rastro)?;
            let elementos = args
                .elementos
                .iter()
                .enumerate()
                .map(|(i, e)| {
                    novo_elemento(&mut f, &mut c, e).map_err(|e| match e {
                        ErroFerramenta::Recusa(m) => {
                            ErroFerramenta::Recusa(format!("elemento {}: {m}", i + 1))
                        }
                        ErroFerramenta::Argumentos(m) => {
                            ErroFerramenta::Argumentos(format!("elementos[{i}]: {m}"))
                        }
                        outro => outro,
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            let nova = NovaTela {
                nome: args.nome,
                mudanca: args.mudanca,
                motivo: args.motivo,
                rastro,
                elementos,
                sessao: None,
            };
            let registrado = comum::alterar(ambiente, &id, Mudanca::Tela, |a, agora| {
                a.registrar_tela(nova, agora)
            })?;
            let numero = registrado.resultado;
            let tela = registrado
                .analise
                .tela(numero)
                .ok_or_else(|| ErroFerramenta::Interno(format!("TL-{numero} não foi gravada")))?;
            Ok(comum::resposta(
                ambiente,
                &registrado,
                json!({
                    "tela": tela.rotulo(),
                    "elementos": tela.elementos.iter().map(ElementoTela::rotulo).collect::<Vec<_>>(),
                    "a_confirmar": rotulos_a_confirmar(tela),
                }),
            ))
        },
    )
}

/// `registrar_elemento_tela`: um elemento numa tela já registrada.
pub fn registrar_elemento_tela() -> Ferramenta {
    let mut propriedades = propriedades_do_elemento();
    propriedades.insert("analise".into(), comum::schema_da_analise());
    propriedades.insert(
        "tela".into(),
        json!({
            "type": ["integer", "string"],
            "minimum": 1,
            "minLength": 1,
            "description": "Tela (TL-n), devolvida por registrar_tela."
        }),
    );
    Ferramenta::nova(
        "registrar_elemento_tela",
        "Registrar elemento de tela",
        "Acrescenta um elemento (campo, validação, mensagem ou ação) a uma tela já \
         registrada com registrar_tela. O elemento cita o que o sustenta: `afirmacoes` \
         (número na análise de origem ou A-NNNN/n), `regras` (RN-<SIGLA>-NNN) e \
         `criterios` (HU-NNNN/CA-n); referência que não existe é recusada. Sem nenhuma \
         referência, ou com `pergunta` em aberto, fica \"a confirmar\". Devolve o \
         identificador do elemento na tela (E-n).",
        json!({
            "type": "object",
            "properties": propriedades,
            "required": ["analise", "tela", "tipo", "nome"],
            "additionalProperties": false
        }),
        |ambiente, args| {
            let args: ArgsElemento = ler_argumentos(args)?;
            let id = comum::id_da_analise(&args.analise)?;
            let tela = numero_da_tela(&args.tela)?;
            let atual = ambiente.analises.carregar(&id)?;
            exigir_fluxo(&atual)?;
            if atual.tela(tela).is_none() {
                return Err(comum::recusa(ErroModelo::ItemDesconhecido {
                    tipo: "tela",
                    numero: tela,
                }));
            }
            let mut f = Fundamentacao::nova(ambiente, &atual);
            let mut c = Criterios::novos(ambiente);
            let novo = novo_elemento(&mut f, &mut c, &args.elemento)?;
            let registrado = comum::alterar(ambiente, &id, Mudanca::Tela, |a, agora| {
                a.registrar_elemento(tela, novo, agora)
            })?;
            let numero = registrado.resultado;
            let elemento = registrado
                .analise
                .tela(tela)
                .and_then(|t| t.elemento(numero));
            Ok(comum::resposta(
                ambiente,
                &registrado,
                json!({
                    "tela": format!("TL-{tela}"),
                    "elemento": format!("E-{numero}"),
                    "a_confirmar": elemento.is_some_and(ElementoTela::a_confirmar),
                }),
            ))
        },
    )
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArgsHtml {
    analise: String,
    tela: RefCitada,
    corpo: String,
}

/// Visual das telas: os tokens aprovados, em qualquer fidelidade, e, para o
/// agente, os avisos (sem tokens aprovados, papéis a confirmar).
fn visual(kb: &cba_kb::Projeto, avisos: &mut Vec<String>) -> Result<Visual, ErroFerramenta> {
    let tokens = TokensProjeto::carregar(kb).map_err(|e| ErroFerramenta::Interno(e.to_string()))?;
    let aprovados = tokens.filter(|t| t.estado == EstadoTokens::Aprovado);
    match &aprovados {
        None => avisos.push(
            "os tokens CSS do projeto não estão aprovados: a tela saiu em wireframe neutro \
             (cinzas). Peça ao analista para extrair, revisar e aprovar os tokens no app e \
             grave o HTML de novo para ter a identidade visual; não invente cores nem fontes"
                .into(),
        ),
        Some(t) => {
            let a_confirmar: Vec<String> =
                t.papeis_a_confirmar().map(|p| p.propriedade()).collect();
            if !a_confirmar.is_empty() {
                avisos.push(format!(
                    "papéis dos tokens a confirmar, que saem no cinza neutro: {}",
                    a_confirmar.join(", ")
                ));
            }
        }
    }
    Ok(Visual::escolher(aprovados.as_ref().map(TokensProjeto::css)))
}

/// `gravar_html_tela`: o protótipo HTML de uma tela, com os quatro estados.
pub fn gravar_html_tela() -> Ferramenta {
    Ferramenta::nova(
        "gravar_html_tela",
        "Gravar protótipo HTML da tela",
        "Grava o protótipo HTML de uma tela já registrada com registrar_tela. Escreva só o \
         `corpo` (sem <html>, <head> nem <body>): uma `<section data-estado=\"…\">` para cada \
         estado principal (`vazio`, `preenchido`, `erro` de validação e `sucesso`), com os \
         campos, as validações, as mensagens e as ações da tela; cada elemento registrado \
         leva `data-elemento=\"E-n\"`. Os textos das mensagens e as validações vêm das regras \
         e dos critérios citados nos elementos. Em tela alterada, marque com a classe `mudou` \
         o que muda em relação à tela atual, lida no código. Use HTML semântico e as classes \
         do CSS base (`erro`, `alerta`, `sucesso`, `mensagem`, `campo-erro`, `acoes`, \
         `secundario`, `a-confirmar`); estilo próprio só com `var(--prototipo-*)`, nunca \
         com cor ou fonte inventada. Recusado: <script>, atributos on…, <iframe>, <link>, \
         <meta> e URLs externas (http:, https:, //) em src, href ou url(): o protótipo abre \
         sem rede. Links entre telas: `TL-n.html`. O app monta o documento com os tokens CSS \
         aprovados (em qualquer fidelidade; o wireframe muda só o acabamento) ou, sem \
         eles, o wireframe neutro, grava em \
         analises/<id>/prototipo/TL-n.html e liga à tela. Gravar de novo substitui.",
        json!({
            "type": "object",
            "properties": {
                "analise": comum::schema_da_analise(),
                "tela": {
                    "type": ["integer", "string"],
                    "minimum": 1,
                    "minLength": 1,
                    "description": "Tela (TL-n), devolvida por registrar_tela."
                },
                "corpo": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": TAMANHO_MAXIMO_DO_CORPO,
                    "description": "Corpo da tela em HTML: as quatro <section data-estado> (vazio, preenchido, erro, sucesso), com os elementos marcados com data-elemento=\"E-n\"."
                }
            },
            "required": ["analise", "tela", "corpo"],
            "additionalProperties": false
        }),
        |ambiente, args| {
            let args: ArgsHtml = ler_argumentos(args)?;
            let id = comum::id_da_analise(&args.analise)?;
            let numero = numero_da_tela(&args.tela)?;
            let atual = ambiente.analises.carregar(&id)?;
            exigir_fluxo(&atual)?;
            if atual.estado == EstadoAnalise::Aprovada {
                return Err(comum::recusa(ErroModelo::AnaliseAprovada(id)));
            }
            let tela = atual.tela(numero).ok_or_else(|| {
                comum::recusa(ErroModelo::ItemDesconhecido {
                    tipo: "tela",
                    numero,
                })
            })?;
            if tela.elementos.is_empty() {
                return Err(ErroFerramenta::Recusa(format!(
                    "a tela {} ainda não tem elementos. Registre os campos, as validações, as \
                     mensagens e as ações com `registrar_elemento_tela` antes do HTML",
                    tela.rotulo()
                )));
            }
            let conferido =
                prototipo_html::conferir_corpo(&args.corpo, tela).map_err(|problemas| {
                    ErroFerramenta::Recusa(format!(
                        "HTML da {} recusado; corrija e grave de novo:\n- {}",
                        tela.rotulo(),
                        problemas.join("\n- ")
                    ))
                })?;
            let mut avisos = Vec::new();
            let visual = visual(&ambiente.kb, &mut avisos)?;
            let fidelidade = atual
                .prototipo
                .as_ref()
                .map(|p| p.pedido.fidelidade)
                .unwrap_or_default();
            let sem_marca: Vec<String> = tela
                .elementos
                .iter()
                .filter(|e| !conferido.elementos_marcados.contains(&e.numero))
                .map(ElementoTela::rotulo)
                .collect();
            if !sem_marca.is_empty() {
                avisos.push(format!(
                    "elemento(s) sem `data-elemento` no HTML: {}. Ponha cada um na tela (a \
                     validação, na mensagem de erro dela); `concluir_analise` exige todos",
                    sem_marca.join(", ")
                ));
            }
            if tela.mudanca == MudancaTela::Alterada && !conferido.tem_mudanca_marcada {
                avisos.push(
                    "tela alterada sem nada com a classe `mudou`: destaque o que muda em \
                     relação à tela atual"
                        .into(),
                );
            }
            let html = prototipo_html::montar(tela, &conferido, &visual, fidelidade);
            let caminho = cba_kb::layout::html_da_tela(&id, numero);
            cba_kb::arquivo::gravar(&ambiente.kb.caminho(&caminho), html.as_bytes()).map_err(
                |e| {
                    ErroFerramenta::Interno(format!(
                        "o HTML não foi gravado em {}: {e}",
                        ambiente.kb.na_base(&caminho)
                    ))
                },
            )?;
            let registrado = comum::alterar(ambiente, &id, Mudanca::Tela, |a, _| {
                a.definir_html_da_tela(numero, Some(caminho.clone()))
            })?;
            Ok(comum::resposta(
                ambiente,
                &registrado,
                json!({
                    "tela": format!("TL-{numero}"),
                    "html": caminho,
                    "arquivo": ambiente.kb.na_base(&caminho),
                    "visual": visual.valor(),
                    "estados": EstadoTela::TODOS.map(EstadoTela::valor),
                    "elementos_marcados": conferido
                        .elementos_marcados
                        .iter()
                        .map(|n| format!("E-{n}"))
                        .collect::<Vec<_>>(),
                    "avisos": avisos,
                }),
            ))
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use cba_core::prototipo::Rastro;
    use cba_core::requisito::RefItem;
    use cba_kb::frontmatter::{Evidencia, Linhas};
    use cba_kb::{Id, Serie};

    fn analise() -> Analise {
        let mut a = Analise::nova(
            Serie::Analise.id(3),
            "faturamento-exemplo",
            Fluxo::Prototipo,
            "Protótipo do parcelamento",
            "Protótipo de telas da análise A-0001",
            1,
        );
        a.origem = Some(Serie::Analise.id(1));
        let evidencia = Evidencia {
            arquivo: CaminhoRelativo::novo("db/02_parametros.sql").unwrap(),
            linhas: Linhas::novo(7, 7).unwrap(),
            hash: "abc".into(),
            commit: None,
        };
        let regra = Fundamento::Regra {
            id: Id::analisar("RN-FAT-007").unwrap(),
            titulo: "Limite de parcelas".into(),
            evidencias: vec![evidencia],
        };
        a.registrar_tela(
            NovaTela {
                nome: "Parcelar fatura".into(),
                mudanca: MudancaTela::Alterada,
                motivo: "O parcelamento passa a valer para PJ.".into(),
                rastro: Rastro {
                    fundamentos: vec![regra.clone()],
                    ..Rastro::default()
                },
                elementos: vec![
                    NovoElemento {
                        tipo: TipoElemento::Campo,
                        nome: "Número de parcelas".into(),
                        descricao: Some("de 2 a 12".into()),
                        rastro: Rastro {
                            fundamentos: vec![regra],
                            criterios: vec![CriterioCitado {
                                analise: Serie::Analise.id(2),
                                historia: IdHistoria::novo(1),
                                criterio: 2,
                                texto: "Dado uma fatura PJ, quando pede 13 parcelas, então recusa"
                                    .into(),
                            }],
                            pergunta: None,
                        },
                        sessao: None,
                    },
                    NovoElemento {
                        tipo: TipoElemento::Mensagem,
                        nome: "Parcelamento indisponível".into(),
                        descricao: None,
                        rastro: Rastro::default(),
                        sessao: None,
                    },
                    NovoElemento {
                        tipo: TipoElemento::Validacao,
                        nome: "Valor mínimo da parcela".into(),
                        descricao: None,
                        rastro: Rastro {
                            pergunta: Some(RefItem::analisar("A-0001/2").unwrap()),
                            ..Rastro::default()
                        },
                        sessao: None,
                    },
                ],
                sessao: None,
            },
            2,
        )
        .unwrap();
        a
    }

    #[test]
    fn telas_no_documento() {
        let a = analise();
        let textos = textos(&a, &a.documento);
        assert_eq!(textos.len(), 2);
        let t = &textos[0];
        assert_eq!(t.secao, TELAS);
        assert!(
            t.texto.starts_with("### TL-1 · Parcelar fatura (alterada)"),
            "{}",
            t.texto
        );
        assert!(
            t.texto
                .contains("- [RN-FAT-007](../regras/RN-FAT-007.md): Limite de parcelas"),
            "{}",
            t.texto
        );
        assert!(
            t.texto
                .contains("- **E-1** · campo · Número de parcelas: de 2 a 12"),
            "{}",
            t.texto
        );
        assert!(
            t.texto
                .contains("  - [HU-0001/CA-2](A-0002.md): Dado uma fatura PJ"),
            "{}",
            t.texto
        );
        assert!(
            t.texto.contains(
                "- **E-2** · mensagem · Parcelamento indisponível · _a confirmar: sem fundamento na análise de origem_"
            ),
            "{}",
            t.texto
        );
        assert!(
            t.texto
                .contains("_a confirmar: depende da pergunta A-0001/2_"),
            "{}",
            t.texto
        );
        // A mesma evidência da tela e do elemento vira uma nota só.
        assert_eq!(t.citacoes.len(), 1);
        let r = &textos[1];
        assert_eq!(r.secao, RASTREABILIDADE);
        assert!(
            r.texto
                .contains("| TL-1 | motivo (alterada) | RN-FAT-007 |  |")
        );
        assert!(
            r.texto
                .contains("| TL-1 | E-1 (campo) | RN-FAT-007, HU-0001/CA-2 |  |"),
            "{}",
            r.texto
        );
        assert!(
            r.texto
                .contains("| TL-1 | E-2 (mensagem) |  | a confirmar (sem fundamento) |")
        );
        assert!(
            r.texto
                .contains("| TL-1 | E-3 (validação) |  | a confirmar (A-0001/2) |")
        );
    }

    #[test]
    fn retomar_lista_telas_e_elementos() {
        let (texto, dados) = para_retomar(&analise());
        assert!(
            texto.contains("### TL-1 · Parcelar fatura [alterada]"),
            "{texto}"
        );
        assert!(
            texto.contains(
                "- E-2 [mensagem] Parcelamento indisponível. Origem: nenhuma; a confirmar"
            ),
            "{texto}"
        );
        assert_eq!(dados[0]["elementos"][0]["fundamentos"][1], "HU-0001/CA-2");
        assert_eq!(dados[0]["elementos"][1]["a_confirmar"], true);
    }

    #[test]
    fn criterio_citado_e_tela_citada() {
        assert_eq!(
            ler_criterio(" HU-0001/CA-2 ").unwrap(),
            (IdHistoria::novo(1), 2)
        );
        for ruim in ["HU-0001", "HU-0001/2", "HU-1/CA-2", "HU-0001/CA-0"] {
            assert!(ler_criterio(ruim).is_err(), "{ruim}");
        }
        assert_eq!(numero_da_tela(&RefCitada::Texto("TL-3".into())).unwrap(), 3);
        assert_eq!(numero_da_tela(&RefCitada::Numero(2)).unwrap(), 2);
        assert!(numero_da_tela(&RefCitada::Texto("T-1".into())).is_err());
    }

    fn projeto(tmp: &tempfile::TempDir) -> cba_kb::Projeto {
        let base = cba_kb::Base::criar(tmp.path().join("base")).unwrap();
        let doc = cba_kb::Documento::novo(
            cba_kb::frontmatter::FrontmatterProjeto {
                tipo: cba_kb::TipoDocumento::Projeto,
                id: "faturamento-exemplo".into(),
                nome: "Faturamento Exemplo".into(),
                areas: Vec::new(),
                criado: None,
            },
            "# Faturamento Exemplo\n",
        );
        base.criar_projeto(&doc).unwrap()
    }

    /// Grava o HTML da TL-1 com os elementos dados e aponta a tela para ele.
    fn gravar_html(kb: &cba_kb::Projeto, a: &mut Analise, elementos: &[u32]) {
        let marcas: String = elementos
            .iter()
            .map(|n| format!("<p data-elemento=\"E-{n}\">x</p>"))
            .collect();
        let corpo = format!(
            "<section data-estado=\"vazio\">{marcas}</section>\
             <section data-estado=\"preenchido\"></section>\
             <section data-estado=\"erro\"></section>\
             <section data-estado=\"sucesso\"></section>"
        );
        let tela = a.tela(1).unwrap().clone();
        let conferido = prototipo_html::conferir_corpo(&corpo, &tela).unwrap();
        let html = prototipo_html::montar(
            &tela,
            &conferido,
            &Visual::SemTokens,
            cba_core::prototipo::Fidelidade::Wireframe,
        );
        let caminho = cba_kb::layout::html_da_tela(&a.id, 1);
        cba_kb::arquivo::gravar(&kb.caminho(&caminho), html.as_bytes()).unwrap();
        a.definir_html_da_tela(1, Some(caminho)).unwrap();
    }

    #[test]
    fn concluir_exige_tela_com_elemento_e_html() {
        let tmp = tempfile::tempdir().unwrap();
        let kb = projeto(&tmp);
        let mut vazia = analise();
        vazia.prototipo = None;
        assert!(conferir_para_concluir(&kb, &vazia).is_err());
        let mut a = analise();
        let e = conferir_para_concluir(&kb, &a).unwrap_err().to_string();
        assert!(e.contains("TL-1: sem protótipo HTML"), "{e}");
        gravar_html(&kb, &mut a, &[1, 3]);
        let e = conferir_para_concluir(&kb, &a).unwrap_err().to_string();
        assert!(e.contains("TL-1: elemento(s) fora do HTML: E-2"), "{e}");
        gravar_html(&kb, &mut a, &[1, 2, 3]);
        assert!(conferir_para_concluir(&kb, &a).is_ok());
        // O documento aponta para o HTML.
        let textos = textos(&a, &a.documento);
        assert!(
            textos[0]
                .texto
                .contains("**Protótipo:** [TL-1.html](A-0003/prototipo/TL-1.html)"),
            "{}",
            textos[0].texto
        );
        std::fs::remove_file(kb.caminho(a.tela(1).unwrap().html.as_ref().unwrap())).unwrap();
        let e = conferir_para_concluir(&kb, &a).unwrap_err().to_string();
        assert!(e.contains("não foi encontrado"), "{e}");
        gravar_html(&kb, &mut a, &[1, 2, 3]);
        a.registrar_tela(
            NovaTela {
                nome: "Consultar parcelas".into(),
                mudanca: MudancaTela::Nova,
                motivo: "Ver as parcelas geradas".into(),
                rastro: Rastro::default(),
                elementos: Vec::new(),
                sessao: None,
            },
            3,
        )
        .unwrap();
        let e = conferir_para_concluir(&kb, &a).unwrap_err().to_string();
        assert!(e.contains("TL-2"), "{e}");
    }
}
