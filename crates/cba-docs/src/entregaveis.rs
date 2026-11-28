//! Catálogo dos tipos de entregável (#460) e as predefinições de cada
//! metodologia.
//!
//! A análise investiga; o entregável é o artefato da metodologia do projeto,
//! derivado de uma análise validada ou do mapa de funcionalidades. Cada tipo do
//! [`CATALOGO`] declara:
//!
//! - o grupo ([`Grupo`]: Ágil, RUP ou Comuns);
//! - o ciclo de vida ([`CicloDeVida`]): o documento **vivo** pertence a uma
//!   funcionalidade do mapa, tem histórico de revisões e nunca é recriado; o
//!   **por demanda** nasce de uma demanda;
//! - a geração ([`Geracao`]): montado pelo app, sem IA, ou redigido pelo agente,
//!   preso ao esquema;
//! - o esquema dos campos ([`Campo`]), que o app valida
//!   ([`TipoEntregavel::validar_campos`]);
//! - o template padrão (`_templates/<template>.md`, embutido em `cba-docs`);
//! - de onde lê o conteúdo ([`Fonte`]).
//!
//! A configuração do projeto ([`cba_index::Entregaveis`]) guarda a metodologia e
//! os tipos habilitados. [`predefinicao`] dá os tipos de Ágil e RUP,
//! [`efetiva`] a configuração que vale para um cadastro (o legado quando o
//! projeto ainda não tem uma) e [`normalizar`] passa para Personalizado o que foi
//! mudado à mão.

use cba_core::analise::Fluxo;
use cba_index::{Entregaveis, Metodologia, TipoHabilitado};
use cba_kb::{Id, Serie};
use serde::Serialize;
use serde_json::Value;

/// Grupo do catálogo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Grupo {
    Agil,
    Rup,
    Comuns,
}

/// Ciclo de vida do entregável.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CicloDeVida {
    /// Pertence a uma funcionalidade do mapa: a v1.0 vem da engenharia reversa e
    /// cada demanda propõe a versão seguinte. Nunca é recriado.
    Vivo,
    /// Um por demanda.
    PorDemanda,
}

/// Quem monta o entregável.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Geracao {
    /// O app monta, de forma determinística, sem IA.
    App,
    /// O agente redige por um roteiro e registra pela ferramenta, preso ao
    /// esquema e à validação de rastreabilidade.
    Agente,
}

/// De onde o entregável lê o conteúdo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Fonte {
    /// A análise validada de origem.
    Analise,
    /// As regras de negócio da base.
    Regras,
    /// Os termos do glossário.
    Termos,
    /// O mapa de funcionalidades.
    Mapa,
    /// Os cenários de teste já registrados.
    Cenarios,
}

/// Tipo do valor de um campo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TipoCampo {
    /// Uma linha de texto.
    Texto,
    /// Texto de vários parágrafos.
    TextoLongo,
    /// Lista de textos.
    Lista,
    /// Lista de identificadores de regra (`RN-FAT-001`): a rastreabilidade.
    Regras,
    /// Etapas de um fluxo (o principal, os alternativos, as exceções do caso de
    /// uso): uma lista de textos em que cada etapa cita no próprio texto ao
    /// menos uma regra (`RN-FAT-001`). O app numera as etapas.
    Etapas,
    /// Histórias do épico, na ordem sugerida: cada uma é um objeto com os
    /// campos da história de usuário ([`CAMPOS_DA_HISTORIA_DO_EPICO`]).
    Historias,
}

/// Campo do esquema de um tipo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Campo {
    pub nome: &'static str,
    pub rotulo: &'static str,
    pub tipo: TipoCampo,
    /// Sem ele (ou vazio), o entregável não passa na validação.
    pub obrigatorio: bool,
}

/// Um tipo do catálogo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TipoEntregavel {
    /// Identificador estável (`caso-de-uso`), guardado no cadastro do projeto.
    pub id: &'static str,
    pub nome: &'static str,
    /// O que o tipo é, numa frase, para a tela de configuração.
    pub descricao: &'static str,
    pub grupo: Grupo,
    pub ciclo: CicloDeVida,
    pub geracao: Geracao,
    pub campos: &'static [Campo],
    /// Template padrão: `_templates/<template>.md` na base, ou o embutido.
    pub template: &'static str,
    pub fontes: &'static [Fonte],
    /// O app já gera este tipo. Os demais estão no catálogo e podem ser
    /// habilitados, mas a geração chega nas próximas versões.
    pub disponivel: bool,
}

/// Um campo com problema na validação do esquema.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, thiserror::Error)]
#[serde(rename_all = "camelCase")]
#[error("{campo}: {motivo}")]
pub struct ErroCampo {
    pub campo: String,
    pub motivo: String,
}

impl TipoEntregavel {
    /// Documento vivo de uma funcionalidade do mapa: uma coluna da visão
    /// "Documentação" do mapa (#462). O glossário é vivo, mas pertence à área.
    pub fn da_funcionalidade(&self) -> bool {
        self.ciclo == CicloDeVida::Vivo && self.id != GLOSSARIO
    }

    pub fn campo(&self, nome: &str) -> Option<&'static Campo> {
        self.campos.iter().find(|c| c.nome == nome)
    }

    /// Confere os campos registrados contra o esquema do tipo: um objeto, sem
    /// campo desconhecido, cada valor do tipo declarado e os obrigatórios
    /// preenchidos. Devolve todos os problemas, não só o primeiro.
    pub fn validar_campos(&self, valor: &Value) -> Result<(), Vec<ErroCampo>> {
        let mut erros = Vec::new();
        validar_objeto(self.campos, self.nome, valor, "", &mut erros);
        if erros.is_empty() { Ok(()) } else { Err(erros) }
    }

    /// As regras que os campos citam, na ordem e sem repetição: as dos campos
    /// [`TipoCampo::Regras`], as citadas no texto das [`TipoCampo::Etapas`] e
    /// as das histórias do épico. É a rastreabilidade do item.
    pub fn regras_citadas(&self, campos: &Value) -> Vec<String> {
        let mut ids = Vec::new();
        coletar_regras(self.campos, campos, &mut ids);
        ids
    }
}

/// Confere um objeto contra os `campos` (os do tipo ou os de uma história do
/// épico), acumulando os problemas em `erros` com o nome do campo prefixado.
fn validar_objeto(
    campos: &[Campo],
    nome: &str,
    valor: &Value,
    prefixo: &str,
    erros: &mut Vec<ErroCampo>,
) {
    let erro = |campo: &str, motivo: String| ErroCampo {
        campo: format!("{prefixo}{campo}"),
        motivo,
    };
    let Some(objeto) = valor.as_object() else {
        erros.push(erro("", "os campos vêm num objeto".into()));
        return;
    };
    for chave in objeto.keys() {
        if !campos.iter().any(|c| c.nome == chave) {
            erros.push(erro(chave, format!("campo desconhecido em {nome}")));
        }
    }
    for c in campos {
        match objeto.get(c.nome) {
            None | Some(Value::Null) => {
                if c.obrigatorio {
                    erros.push(erro(c.nome, "obrigatório".into()));
                }
            }
            Some(v) if c.tipo == TipoCampo::Historias => {
                validar_historias(c, v, &format!("{prefixo}{}", c.nome), erros);
            }
            Some(v) => {
                if let Err(motivo) = valor_do_tipo(c, v) {
                    erros.push(erro(c.nome, motivo));
                }
            }
        }
    }
}

/// Rótulo da história do épico pela posição (`H1`, `H2`).
fn rotulo_da_historia(i: usize) -> String {
    format!("H{}", i + 1)
}

/// As histórias do épico: uma lista de objetos, cada um no esquema da história
/// de usuário; `depende_de` só aponta para histórias anteriores (a ordem da
/// lista é a ordem sugerida).
fn validar_historias(c: &Campo, v: &Value, campo: &str, erros: &mut Vec<ErroCampo>) {
    let Some(itens) = v.as_array() else {
        erros.push(ErroCampo {
            campo: campo.to_owned(),
            motivo: "esperada uma lista de histórias".into(),
        });
        return;
    };
    if c.obrigatorio && itens.is_empty() {
        erros.push(ErroCampo {
            campo: campo.to_owned(),
            motivo: "precisa de pelo menos uma história".into(),
        });
    }
    for (i, h) in itens.iter().enumerate() {
        let rotulo = rotulo_da_historia(i);
        validar_objeto(
            CAMPOS_DA_HISTORIA_DO_EPICO,
            "história do épico",
            h,
            &format!("{campo}[{rotulo}]."),
            erros,
        );
        for d in h
            .get("depende_de")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            let anterior = d
                .trim()
                .strip_prefix('H')
                .and_then(|n| n.parse::<usize>().ok())
                .is_some_and(|n| n >= 1 && n <= i);
            if !anterior {
                erros.push(ErroCampo {
                    campo: format!("{campo}[{rotulo}].depende_de"),
                    motivo: format!(
                        "`{d}` não é uma história anterior (H1 a H{i}): a lista segue a ordem \
                         sugerida, então a dependência vem antes"
                    ),
                });
            }
        }
    }
}

/// Os identificadores de regra (`RN-FAT-001`) citados num texto, na ordem e
/// sem repetição.
pub fn regras_no_texto(texto: &str) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    for p in texto.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-')) {
        let p = p.trim_end_matches('-');
        if p.starts_with("RN-")
            && matches!(Id::analisar(p), Ok(id) if matches!(id.serie(), Serie::Regra { .. }))
            && !ids.iter().any(|i| i == p)
        {
            ids.push(p.to_owned());
        }
    }
    ids
}

fn coletar_regras(campos: &[Campo], valor: &Value, ids: &mut Vec<String>) {
    let mut juntar = |id: String| {
        if !ids.contains(&id) {
            ids.push(id);
        }
    };
    for c in campos {
        let itens = valor
            .get(c.nome)
            .and_then(Value::as_array)
            .into_iter()
            .flatten();
        match c.tipo {
            TipoCampo::Regras => {
                for id in itens.filter_map(Value::as_str) {
                    juntar(id.trim().to_owned());
                }
            }
            TipoCampo::Etapas => {
                for id in itens.filter_map(Value::as_str).flat_map(regras_no_texto) {
                    juntar(id);
                }
            }
            TipoCampo::Historias => {
                let mut das_historias = Vec::new();
                for h in itens {
                    coletar_regras(CAMPOS_DA_HISTORIA_DO_EPICO, h, &mut das_historias);
                }
                for id in das_historias {
                    juntar(id);
                }
            }
            TipoCampo::Texto | TipoCampo::TextoLongo | TipoCampo::Lista => {}
        }
    }
}

/// Campos que viram o título do item, não uma seção.
const CAMPOS_DO_TITULO: [&str; 2] = ["titulo", "funcionalidade"];

impl TipoEntregavel {
    /// O item em Markdown, montado pelo app a partir dos campos já validados
    /// ([`TipoEntregavel::validar_campos`]), sem IA: o título (o campo `titulo`,
    /// a funcionalidade ou o nome do tipo) num cabeçalho de nível `nivel`, cada
    /// campo preenchido na ordem do esquema (a linha como "**Rótulo:** valor", o
    /// texto longo e a lista numa seção) e, no fim, as regras de origem com o
    /// título de cada uma (`regras`: id e título, na ordem citada).
    pub fn markdown_do_item(
        &self,
        campos: &Value,
        regras: &[(String, String)],
        nivel: usize,
    ) -> String {
        let nivel = nivel.clamp(1, 5);
        let h = "#".repeat(nivel);
        let sub = "#".repeat(nivel + 1);
        let texto = |nome: &str| {
            campos
                .get(nome)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|t| !t.is_empty())
        };
        let titulo = CAMPOS_DO_TITULO
            .iter()
            .find_map(|n| texto(n))
            .unwrap_or(self.nome);
        let mut partes = vec![format!("{h} {titulo}")];
        for c in self.campos {
            if CAMPOS_DO_TITULO.contains(&c.nome) || c.tipo == TipoCampo::Regras {
                continue;
            }
            match c.tipo {
                TipoCampo::Texto => {
                    if let Some(v) = texto(c.nome) {
                        partes.push(format!("**{}:** {v}", c.rotulo));
                    }
                }
                TipoCampo::TextoLongo => {
                    if let Some(v) = texto(c.nome) {
                        partes.push(format!("{sub} {}\n\n{v}", c.rotulo));
                    }
                }
                TipoCampo::Lista | TipoCampo::Regras => {
                    let itens: Vec<String> = textos_da_lista(campos, c.nome)
                        .into_iter()
                        .map(|i| format!("- {i}"))
                        .collect();
                    if !itens.is_empty() {
                        partes.push(format!("{sub} {}\n\n{}", c.rotulo, itens.join("\n")));
                    }
                }
                TipoCampo::Etapas => {
                    let itens = textos_da_lista(campos, c.nome);
                    if !itens.is_empty() {
                        partes.push(format!(
                            "{sub} {}\n\n{}",
                            c.rotulo,
                            etapas_em_markdown(c.nome, &itens)
                        ));
                    }
                }
                TipoCampo::Historias => {
                    let itens: Vec<&Value> = campos
                        .get(c.nome)
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .collect();
                    if !itens.is_empty() {
                        let historias: Vec<String> = itens
                            .iter()
                            .enumerate()
                            .map(|(i, h)| historia_em_markdown(i, h, &"#".repeat(nivel + 2)))
                            .collect();
                        partes.push(format!("{sub} {}\n\n{}", c.rotulo, historias.join("\n\n")));
                    }
                }
            }
        }
        if !regras.is_empty() {
            let linhas: Vec<String> = regras
                .iter()
                .map(|(id, titulo)| {
                    if titulo.trim().is_empty() {
                        format!("- {id}")
                    } else {
                        format!("- {id}: {}", titulo.trim())
                    }
                })
                .collect();
            partes.push(format!("{sub} {}\n\n{}", REGRAS.rotulo, linhas.join("\n")));
        }
        partes.join("\n\n")
    }

    /// Palavra do item no aviso de cobertura ("RN-FAT-005 sem história").
    pub fn sem_item(&self) -> &'static str {
        match self.id {
            HISTORIA => "sem história",
            EPICO => "sem história no épico",
            CASO_DE_USO => "fora do caso de uso",
            CASOS_DE_TESTE => "sem caso de teste",
            PROTOTIPO => "fora do protótipo",
            MANUAL => "fora do manual",
            _ => "sem item no entregável",
        }
    }
}

/// Os textos de um campo de lista, cada um numa linha só.
fn textos_da_lista(campos: &Value, nome: &str) -> Vec<String> {
    campos
        .get(nome)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(em_uma_linha)
        .filter(|i| !i.is_empty())
        .collect()
}

fn em_uma_linha(texto: &str) -> String {
    texto.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Prefixo da numeração das etapas: o fluxo principal em `1.`, os fluxos
/// alternativos em `FA1`, as exceções em `FE1`.
fn prefixo_das_etapas(campo: &str) -> Option<&'static str> {
    match campo {
        "fluxos_alternativos" => Some("FA"),
        "excecoes" => Some("FE"),
        _ => None,
    }
}

/// Tira a numeração que o agente tenha posto ("3. ", "FA1 - ", "FE2:"): o app
/// numera.
fn sem_numeracao(etapa: &str, prefixo: Option<&str>) -> String {
    let mut resto = etapa.trim_start();
    if let Some(p) = prefixo
        && let Some(r) = resto.strip_prefix(p)
    {
        resto = r;
    }
    let digitos = resto.len() - resto.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    if digitos == 0 {
        return etapa.trim().to_owned();
    }
    let depois = &resto[digitos..];
    let pontuado = depois
        .trim_start()
        .starts_with(['.', ')', ':', '-', '—', '–']);
    if !pontuado && prefixo.is_none() {
        // "3 parcelas..." não é numeração.
        return etapa.trim().to_owned();
    }
    depois
        .trim_start_matches(['.', ')', ':', '-', '—', '–', ' '])
        .trim()
        .to_owned()
}

fn etapas_em_markdown(campo: &str, etapas: &[String]) -> String {
    let prefixo = prefixo_das_etapas(campo);
    etapas
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let texto = sem_numeracao(e, prefixo);
            match prefixo {
                Some(p) => format!("- **{p}{}.** {texto}", i + 1),
                None => format!("{}. {texto}", i + 1),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Uma história do épico: o título com o rótulo (`H1`), a narrativa, as
/// dependências, os critérios e as regras.
fn historia_em_markdown(i: usize, h: &Value, titulo: &str) -> String {
    let texto = |nome: &str| {
        h.get(nome)
            .and_then(Value::as_str)
            .map(em_uma_linha)
            .unwrap_or_default()
    };
    let mut partes = vec![
        format!("{titulo} {}. {}", rotulo_da_historia(i), texto("titulo")),
        format!(
            "**Como** {}, **quero** {}, **para** {}.",
            texto("papel"),
            texto("acao").trim_end_matches('.'),
            texto("beneficio").trim_end_matches('.')
        ),
    ];
    let depende = textos_da_lista(h, "depende_de");
    if !depende.is_empty() {
        partes.push(format!("**Depende de:** {}", depende.join(", ")));
    }
    let criterios: Vec<String> = textos_da_lista(h, "criterios")
        .into_iter()
        .map(|c| format!("- {c}"))
        .collect();
    if !criterios.is_empty() {
        partes.push(format!(
            "**Critérios de aceite:**\n\n{}",
            criterios.join("\n")
        ));
    }
    let regras = textos_da_lista(h, "regras");
    if !regras.is_empty() {
        partes.push(format!("**Regras:** {}", regras.join(", ")));
    }
    partes.join("\n\n")
}

fn valor_do_tipo(c: &Campo, v: &Value) -> Result<(), String> {
    match c.tipo {
        TipoCampo::Texto | TipoCampo::TextoLongo => {
            let texto = v.as_str().ok_or("esperado um texto")?;
            if c.tipo == TipoCampo::Texto && texto.contains('\n') {
                return Err("esperada uma linha só".into());
            }
            if c.obrigatorio && texto.trim().is_empty() {
                return Err("obrigatório".into());
            }
        }
        TipoCampo::Lista | TipoCampo::Regras => {
            let itens = v.as_array().ok_or("esperada uma lista")?;
            if c.obrigatorio && itens.is_empty() {
                return Err("precisa de pelo menos um item".into());
            }
            for item in itens {
                let texto = item.as_str().ok_or("cada item é um texto")?;
                if texto.trim().is_empty() {
                    return Err("item vazio".into());
                }
                if c.tipo == TipoCampo::Regras
                    && !matches!(Id::analisar(texto), Ok(id) if matches!(id.serie(), Serie::Regra { .. }))
                {
                    return Err(format!(
                        "`{texto}` não é um identificador de regra (RN-<SIGLA>-NNN)"
                    ));
                }
            }
        }
        TipoCampo::Etapas => {
            let itens = v.as_array().ok_or("esperada uma lista de etapas")?;
            if c.obrigatorio && itens.is_empty() {
                return Err("precisa de pelo menos uma etapa".into());
            }
            let mut sem_regra = Vec::new();
            for (i, item) in itens.iter().enumerate() {
                let texto = item.as_str().ok_or("cada etapa é um texto")?;
                if texto.trim().is_empty() {
                    return Err("etapa vazia".into());
                }
                if regras_no_texto(texto).is_empty() {
                    sem_regra.push((i + 1).to_string());
                }
            }
            if !sem_regra.is_empty() {
                return Err(format!(
                    "etapa {} sem regra citada: cada etapa cita no texto a regra que a \
                     sustenta (RN-<SIGLA>-NNN)",
                    sem_regra.join(", ")
                ));
            }
        }
        // Conferido por `validar_historias`.
        TipoCampo::Historias => {}
    }
    Ok(())
}

const fn campo(nome: &'static str, rotulo: &'static str, tipo: TipoCampo) -> Campo {
    Campo {
        nome,
        rotulo,
        tipo,
        obrigatorio: true,
    }
}

const fn opcional(nome: &'static str, rotulo: &'static str, tipo: TipoCampo) -> Campo {
    Campo {
        nome,
        rotulo,
        tipo,
        obrigatorio: false,
    }
}

use Fonte as F;
use TipoCampo as C;

/// Regras de origem: todo entregável cita de onde vem cada item.
const REGRAS: Campo = campo("regras", "Regras de origem", C::Regras);
const FUNCIONALIDADE: Campo = campo("funcionalidade", "Funcionalidade do mapa", C::Texto);
const TITULO: Campo = campo("titulo", "Título", C::Texto);
const PAPEL: Campo = campo("papel", "Como (papel)", C::Texto);
const ACAO: Campo = campo("acao", "Quero (ação)", C::Texto);
const BENEFICIO: Campo = campo("beneficio", "Para (benefício)", C::Texto);
const CRITERIOS: Campo = campo("criterios", "Critérios de aceite", C::Lista);

/// Os campos de cada história do épico: os da história de usuário do catálogo
/// ([`HISTORIA`]), mais as histórias anteriores de que ela depende (`H1`).
pub const CAMPOS_DA_HISTORIA_DO_EPICO: &[Campo] = &[
    TITULO,
    PAPEL,
    ACAO,
    BENEFICIO,
    CRITERIOS,
    REGRAS,
    opcional("depende_de", "Depende de", C::Lista),
];

/// Ids dos tipos que as predefinições e o legado usam.
pub const HISTORIA: &str = "historia-usuario";
pub const CRITERIOS_GHERKIN: &str = "criterios-gherkin";
pub const CENARIOS_FEATURE: &str = "cenarios-feature";
pub const EPICO: &str = "epico";
pub const RESUMO_REFINAMENTO: &str = "resumo-refinamento";
pub const DEFINITION_OF_READY: &str = "definition-of-ready";
pub const CASO_DE_USO: &str = "caso-de-uso";
pub const CATALOGO_REGRAS: &str = "catalogo-regras";
pub const ESPECIFICACAO_SUPLEMENTAR: &str = "especificacao-suplementar";
pub const CASOS_DE_TESTE: &str = "casos-de-teste";
pub const DOCUMENTO_VISAO: &str = "documento-visao";
pub const PROTOTIPO: &str = "prototipo";
pub const GLOSSARIO: &str = "glossario";
pub const MANUAL: &str = "manual";
/// O mesmo id que o mapa usa no vínculo do documento negocial.
pub const DOCUMENTO_NEGOCIAL: &str = cba_core::funcionalidades::TIPO_DOCUMENTO_NEGOCIAL;

/// Os tipos embutidos, na ordem da tela: Ágil, RUP e Comuns.
pub const CATALOGO: [TipoEntregavel; 15] = [
    // Ágil
    TipoEntregavel {
        id: HISTORIA,
        nome: "História de usuário",
        descricao: "Como, quero, para, com os critérios de aceite e as regras de origem.",
        grupo: Grupo::Agil,
        ciclo: CicloDeVida::PorDemanda,
        geracao: Geracao::Agente,
        campos: &[TITULO, PAPEL, ACAO, BENEFICIO, CRITERIOS, REGRAS],
        template: "historias",
        fontes: &[F::Analise, F::Regras, F::Termos],
        disponivel: true,
    },
    TipoEntregavel {
        id: CRITERIOS_GHERKIN,
        nome: "Critérios em Gherkin",
        descricao: "Dado, quando, então de uma história, montados dos cenários existentes.",
        grupo: Grupo::Agil,
        ciclo: CicloDeVida::PorDemanda,
        geracao: Geracao::App,
        campos: &[
            campo("historia", "História", C::Texto),
            campo("cenarios", "Cenários (Dado, Quando, Então)", C::Lista),
            REGRAS,
        ],
        template: "criterios-gherkin",
        fontes: &[F::Cenarios, F::Regras],
        disponivel: false,
    },
    TipoEntregavel {
        id: CENARIOS_FEATURE,
        nome: "Cenários .feature",
        descricao: "Arquivo .feature da funcionalidade, atualizado a cada demanda.",
        grupo: Grupo::Agil,
        ciclo: CicloDeVida::Vivo,
        geracao: Geracao::App,
        campos: &[
            FUNCIONALIDADE,
            campo("cenarios", "Cenários (Dado, Quando, Então)", C::Lista),
            REGRAS,
        ],
        template: "cenarios-feature",
        fontes: &[F::Cenarios, F::Regras, F::Mapa],
        disponivel: false,
    },
    TipoEntregavel {
        id: EPICO,
        nome: "Épico com quebra em histórias",
        descricao: "O objetivo da demanda e as histórias em que ela se divide, em ordem, com as dependências e os riscos.",
        grupo: Grupo::Agil,
        ciclo: CicloDeVida::PorDemanda,
        geracao: Geracao::Agente,
        // As regras do épico são as das histórias.
        campos: &[
            TITULO,
            campo("objetivo", "Objetivo", C::TextoLongo),
            campo("historias", "Histórias (em ordem sugerida)", C::Historias),
            opcional("dependencias", "Dependências", C::Lista),
            opcional("riscos", "Riscos", C::Lista),
            opcional("fora_do_escopo", "Fora do escopo", C::Lista),
        ],
        template: "epico",
        fontes: &[F::Analise, F::Regras, F::Mapa],
        disponivel: true,
    },
    TipoEntregavel {
        id: RESUMO_REFINAMENTO,
        nome: "Resumo para refinamento",
        descricao: "O contexto, as dúvidas e os riscos para levar à reunião de refinamento.",
        grupo: Grupo::Agil,
        ciclo: CicloDeVida::PorDemanda,
        geracao: Geracao::Agente,
        campos: &[
            TITULO,
            campo("contexto", "Contexto", C::TextoLongo),
            opcional("duvidas", "Dúvidas em aberto", C::Lista),
            opcional("riscos", "Riscos", C::Lista),
            REGRAS,
        ],
        template: "resumo-refinamento",
        fontes: &[F::Analise, F::Regras],
        disponivel: false,
    },
    TipoEntregavel {
        id: DEFINITION_OF_READY,
        nome: "Definition of Ready",
        descricao: "A lista do que a história precisa ter antes de entrar na sprint.",
        grupo: Grupo::Agil,
        ciclo: CicloDeVida::PorDemanda,
        geracao: Geracao::App,
        campos: &[
            campo("historia", "História", C::Texto),
            campo("itens", "Itens atendidos", C::Lista),
            opcional("pendencias", "Pendências", C::Lista),
        ],
        template: "definition-of-ready",
        fontes: &[F::Analise, F::Regras, F::Cenarios],
        disponivel: false,
    },
    // RUP
    TipoEntregavel {
        id: CASO_DE_USO,
        nome: "Especificação de caso de uso",
        descricao: "Atores, pré-condições, fluxo principal, alternativos, exceções e pós-condições da funcionalidade, cada etapa com a regra citada.",
        grupo: Grupo::Rup,
        ciclo: CicloDeVida::Vivo,
        geracao: Geracao::Agente,
        // Na ordem e com os títulos do template `caso-de-uso.md`.
        campos: &[
            TITULO,
            campo("objetivo", "Objetivo", C::TextoLongo),
            campo("atores", "Atores", C::Lista),
            opcional("gatilho", "Gatilho", C::Texto),
            opcional("pre_condicoes", "Pré-condições", C::Lista),
            campo("fluxo_principal", "Fluxo principal", C::Etapas),
            opcional("fluxos_alternativos", "Fluxos alternativos", C::Etapas),
            opcional("excecoes", "Exceções", C::Etapas),
            opcional("pos_condicoes", "Pós-condições", C::Lista),
            REGRAS,
        ],
        template: "caso-de-uso",
        fontes: &[F::Mapa, F::Analise, F::Regras, F::Termos],
        disponivel: true,
    },
    TipoEntregavel {
        id: CATALOGO_REGRAS,
        nome: "Catálogo de regras de negócio",
        descricao: "As regras da funcionalidade, com o enunciado e as evidências.",
        grupo: Grupo::Rup,
        ciclo: CicloDeVida::Vivo,
        geracao: Geracao::App,
        campos: &[FUNCIONALIDADE, REGRAS],
        template: "catalogo-regras",
        fontes: &[F::Regras, F::Mapa],
        disponivel: false,
    },
    TipoEntregavel {
        id: ESPECIFICACAO_SUPLEMENTAR,
        nome: "Especificação suplementar",
        descricao: "Requisitos não funcionais da funcionalidade: dados pessoais, auditoria, desempenho e acesso.",
        grupo: Grupo::Rup,
        ciclo: CicloDeVida::Vivo,
        geracao: Geracao::App,
        campos: &[
            FUNCIONALIDADE,
            campo("requisitos", "Requisitos não funcionais", C::Lista),
            REGRAS,
        ],
        template: "especificacao-suplementar",
        fontes: &[F::Analise, F::Regras, F::Mapa],
        disponivel: false,
    },
    TipoEntregavel {
        id: CASOS_DE_TESTE,
        nome: "Casos de teste",
        descricao: "Tabelas de decisão, valores-limite e cenários derivados das regras.",
        grupo: Grupo::Rup,
        ciclo: CicloDeVida::PorDemanda,
        geracao: Geracao::Agente,
        campos: &[
            campo("cenarios", "Cenários", C::Lista),
            opcional("tabelas_decisao", "Tabelas de decisão", C::Lista),
            opcional("valores_limite", "Valores-limite", C::Lista),
            REGRAS,
        ],
        template: "cenarios-teste",
        fontes: &[F::Analise, F::Regras, F::Cenarios],
        disponivel: true,
    },
    TipoEntregavel {
        id: DOCUMENTO_VISAO,
        nome: "Documento de visão",
        descricao: "O problema, as partes interessadas e as necessidades atendidas pela demanda.",
        grupo: Grupo::Rup,
        ciclo: CicloDeVida::PorDemanda,
        geracao: Geracao::Agente,
        campos: &[
            TITULO,
            campo("problema", "Problema", C::TextoLongo),
            campo("partes_interessadas", "Partes interessadas", C::Lista),
            campo("necessidades", "Necessidades", C::Lista),
            opcional("fora_do_escopo", "Fora do escopo", C::Lista),
            REGRAS,
        ],
        template: "documento-visao",
        fontes: &[F::Analise, F::Mapa, F::Regras],
        disponivel: false,
    },
    // Comuns
    TipoEntregavel {
        id: PROTOTIPO,
        nome: "Protótipo",
        descricao: "As telas afetadas, com os elementos ligados às regras e aos critérios.",
        grupo: Grupo::Comuns,
        ciclo: CicloDeVida::PorDemanda,
        geracao: Geracao::Agente,
        campos: &[campo("telas", "Telas", C::Lista), REGRAS],
        template: "prototipo",
        fontes: &[F::Analise, F::Regras],
        disponivel: true,
    },
    TipoEntregavel {
        id: GLOSSARIO,
        nome: "Glossário",
        descricao: "Os termos de negócio da funcionalidade e onde cada um vive no código.",
        grupo: Grupo::Comuns,
        ciclo: CicloDeVida::Vivo,
        geracao: Geracao::App,
        campos: &[FUNCIONALIDADE, campo("termos", "Termos", C::Lista)],
        template: "glossario",
        fontes: &[F::Termos, F::Mapa],
        disponivel: false,
    },
    TipoEntregavel {
        id: MANUAL,
        nome: "Manual do usuário",
        descricao: "O passo a passo da funcionalidade, com os prints de tela.",
        grupo: Grupo::Comuns,
        ciclo: CicloDeVida::Vivo,
        geracao: Geracao::Agente,
        campos: &[
            FUNCIONALIDADE,
            campo("objetivo", "Objetivo", C::TextoLongo),
            campo("passos", "Passo a passo", C::Lista),
            opcional("mensagens", "Mensagens e erros comuns", C::Lista),
            REGRAS,
        ],
        template: "manual",
        fontes: &[F::Mapa, F::Analise, F::Regras],
        disponivel: true,
    },
    TipoEntregavel {
        id: DOCUMENTO_NEGOCIAL,
        nome: "Documento negocial",
        descricao: "A explicação da funcionalidade em linguagem de negócio (o \"Explicar\").",
        grupo: Grupo::Comuns,
        ciclo: CicloDeVida::Vivo,
        geracao: Geracao::Agente,
        campos: &[
            FUNCIONALIDADE,
            campo("objetivo", "Objetivo", C::TextoLongo),
            campo("fluxo_principal", "Fluxo principal", C::Lista),
            opcional("excecoes", "Exceções", C::Lista),
            REGRAS,
        ],
        template: "funcionalidade",
        fontes: &[F::Mapa, F::Analise, F::Regras, F::Termos],
        disponivel: true,
    },
];

/// Tipo do catálogo pelo `id`.
pub fn tipo(id: &str) -> Option<&'static TipoEntregavel> {
    CATALOGO.iter().find(|t| t.id == id)
}

/// Os tipos habilitados no projeto que são documentos vivos de uma
/// funcionalidade, na ordem do catálogo: as colunas da visão "Documentação" do
/// mapa (#462).
pub fn vivos_da_funcionalidade(e: &Entregaveis) -> Vec<&'static TipoEntregavel> {
    CATALOGO
        .iter()
        .filter(|t| t.da_funcionalidade() && e.habilitado(t.id).is_some())
        .collect()
}

/// Tipos e tipo padrão de uma predefinição.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Predefinicao {
    pub metodologia: Metodologia,
    pub tipos: &'static [&'static str],
    pub padrao: &'static str,
}

/// As predefinições de Ágil e RUP. Personalizado não tem: fica com o que o
/// analista escolheu.
pub const PREDEFINICOES: [Predefinicao; 2] = [
    Predefinicao {
        metodologia: Metodologia::Agil,
        tipos: &[
            EPICO,
            HISTORIA,
            CRITERIOS_GHERKIN,
            CENARIOS_FEATURE,
            PROTOTIPO,
        ],
        padrao: EPICO,
    },
    Predefinicao {
        metodologia: Metodologia::Rup,
        tipos: &[
            CASO_DE_USO,
            CATALOGO_REGRAS,
            ESPECIFICACAO_SUPLEMENTAR,
            PROTOTIPO,
            GLOSSARIO,
        ],
        padrao: CASO_DE_USO,
    },
];

/// Os tipos que o app já gerava antes do catálogo, habilitados num projeto que
/// ainda não tem a configuração: histórias, cenários de teste, manual,
/// protótipo e documento negocial.
pub const LEGADO: Predefinicao = Predefinicao {
    metodologia: Metodologia::Personalizado,
    tipos: &[
        HISTORIA,
        CASOS_DE_TESTE,
        MANUAL,
        PROTOTIPO,
        DOCUMENTO_NEGOCIAL,
    ],
    padrao: HISTORIA,
};

impl Predefinicao {
    /// A configuração da predefinição, com o template e o destino padrão.
    pub fn entregaveis(&self) -> Entregaveis {
        Entregaveis {
            metodologia: self.metodologia,
            tipos: ordenados(self.tipos.iter().map(|t| TipoHabilitado::novo(t)).collect()),
            padrao: Some(self.padrao.to_owned()),
        }
    }

    /// Se a configuração tem exatamente os tipos e o padrão desta predefinição
    /// (template e destino não contam).
    pub fn confere(&self, e: &Entregaveis) -> bool {
        e.tipos.len() == self.tipos.len()
            && self.tipos.iter().all(|t| e.habilitado(t).is_some())
            && e.padrao.as_deref() == Some(self.padrao)
    }
}

/// Predefinição da metodologia, se ela tiver uma (Ágil e RUP).
pub fn predefinicao(metodologia: Metodologia) -> Option<&'static Predefinicao> {
    PREDEFINICOES.iter().find(|p| p.metodologia == metodologia)
}

/// A configuração que vale para o cadastro: a gravada ou, num projeto que
/// ainda não tem uma, o [`LEGADO`].
pub fn efetiva(gravada: Option<&Entregaveis>) -> Entregaveis {
    gravada.cloned().unwrap_or_else(|| LEGADO.entregaveis())
}

/// Escolhe a metodologia: aplica a predefinição dela, mantendo o template e o
/// destino dos tipos que continuam habilitados. Personalizado mantém os tipos
/// como estão.
pub fn aplicar(atual: &Entregaveis, metodologia: Metodologia) -> Entregaveis {
    let Some(p) = predefinicao(metodologia) else {
        return Entregaveis {
            metodologia,
            ..atual.clone()
        };
    };
    let mut nova = p.entregaveis();
    for t in &mut nova.tipos {
        if let Some(antes) = atual.habilitado(&t.tipo) {
            *t = antes.clone();
        }
    }
    nova
}

/// Põe os tipos na ordem do catálogo e passa a metodologia para Personalizado
/// quando os tipos ou o padrão não batem com a predefinição dela (um item
/// mudado à mão).
pub fn normalizar(mut e: Entregaveis) -> Entregaveis {
    e.tipos = ordenados(e.tipos);
    if let Some(p) = predefinicao(e.metodologia)
        && !p.confere(&e)
    {
        e.metodologia = Metodologia::Personalizado;
    }
    e
}

/// Se o fluxo de análise é oferecido no projeto (#465). Histórias e Cenários
/// geram tipos do catálogo e só aparecem quando habilitados:
///
/// - **Histórias** gera a [`HISTORIA`];
/// - **Cenários** gera os [`CASOS_DE_TESTE`] e registra os cenários de onde os
///   tipos com a fonte [`Fonte::Cenarios`] (critérios em Gherkin, cenários
///   `.feature`, Definition of Ready) são montados: basta um deles habilitado.
///
/// Os demais fluxos são análises (perguntar, explicar, impacto, reunião,
/// revisar) ou geram tipos que continuam como hoje (manual e protótipo) e são
/// sempre oferecidos. As análises já gravadas abrem, exportam e publicam com
/// qualquer configuração: a trava é só para criar.
pub fn fluxo_oferecido(e: &Entregaveis, fluxo: Fluxo) -> bool {
    match fluxo {
        Fluxo::Historias => e.habilitado(HISTORIA).is_some(),
        Fluxo::Cenarios => e.ids().any(|id| {
            id == CASOS_DE_TESTE || tipo(id).is_some_and(|t| t.fontes.contains(&Fonte::Cenarios))
        }),
        _ => true,
    }
}

/// Os fluxos que o projeto não oferece, para a tela esconder.
pub fn fluxos_ocultos(e: &Entregaveis) -> Vec<Fluxo> {
    [Fluxo::Historias, Fluxo::Cenarios]
        .into_iter()
        .filter(|f| !fluxo_oferecido(e, *f))
        .collect()
}

/// A recusa ao criar uma análise de um fluxo que o projeto não oferece.
pub fn conferir_fluxo(e: &Entregaveis, fluxo: Fluxo) -> Result<(), String> {
    if fluxo_oferecido(e, fluxo) {
        return Ok(());
    }
    let (nome, tipos) = match fluxo {
        Fluxo::Historias => ("Histórias", "História de usuário"),
        _ => (
            "Cenários",
            "Casos de teste, Critérios em Gherkin, Cenários .feature ou Definition of Ready",
        ),
    };
    Err(format!(
        "o projeto não oferece o fluxo {nome}: habilite {tipos} em Configurações › Entregáveis \
         (as análises já gravadas continuam abrindo)"
    ))
}

fn ordenados(mut tipos: Vec<TipoHabilitado>) -> Vec<TipoHabilitado> {
    let posicao = |t: &TipoHabilitado| CATALOGO.iter().position(|c| c.id == t.tipo);
    tipos.sort_by_key(|t| posicao(t).unwrap_or(usize::MAX));
    tipos
}

/// Configuração recusada ao salvar.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ErroEntregaveis {
    #[error("a metodologia “Orientado a agentes” ainda não está disponível")]
    MetodologiaIndisponivel,
    #[error("tipo de entregável desconhecido: {0}")]
    TipoDesconhecido(String),
    #[error("o tipo {0} aparece mais de uma vez")]
    TipoRepetido(String),
    #[error("habilite pelo menos um tipo de entregável")]
    SemTipos,
    #[error("escolha o tipo padrão entre os habilitados")]
    SemPadrao,
    #[error("o tipo padrão ({0}) não está habilitado")]
    PadraoDesabilitado(String),
    #[error(
        "template inválido para {tipo}: `{template}` (use o nome de um arquivo .md de _templates/, sem pastas)"
    )]
    TemplateInvalido { tipo: String, template: String },
}

/// Confere a configuração contra o catálogo: tipos conhecidos e sem
/// repetição, pelo menos um, o padrão entre eles e o template, quando há, só o
/// nome de um `.md` (a existência do arquivo é conferida por quem conhece a
/// base).
pub fn validar(e: &Entregaveis) -> Result<(), ErroEntregaveis> {
    if e.metodologia == Metodologia::OrientadoAgentes {
        return Err(ErroEntregaveis::MetodologiaIndisponivel);
    }
    if e.tipos.is_empty() {
        return Err(ErroEntregaveis::SemTipos);
    }
    for (i, t) in e.tipos.iter().enumerate() {
        if tipo(&t.tipo).is_none() {
            return Err(ErroEntregaveis::TipoDesconhecido(t.tipo.clone()));
        }
        if e.tipos[..i].iter().any(|a| a.tipo == t.tipo) {
            return Err(ErroEntregaveis::TipoRepetido(t.tipo.clone()));
        }
        if let Some(template) = &t.template
            && !nome_de_template(template)
        {
            return Err(ErroEntregaveis::TemplateInvalido {
                tipo: t.tipo.clone(),
                template: template.clone(),
            });
        }
    }
    match &e.padrao {
        None => Err(ErroEntregaveis::SemPadrao),
        Some(p) if e.habilitado(p).is_none() => Err(ErroEntregaveis::PadraoDesabilitado(p.clone())),
        Some(_) => Ok(()),
    }
}

/// Nome de arquivo `.md` simples, sem pasta nem caminho relativo.
fn nome_de_template(nome: &str) -> bool {
    nome.len() > ".md".len()
        && nome.len() <= 120
        && nome.ends_with(".md")
        && !nome.starts_with('.')
        && nome
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use cba_index::Destino;
    use cba_kb::TipoDocumento;
    use serde_json::json;

    use super::*;
    use crate::modelo;

    #[test]
    fn catalogo_tem_os_tipos_iniciais_por_grupo() {
        let do_grupo = |g| -> Vec<&str> {
            CATALOGO
                .iter()
                .filter(|t| t.grupo == g)
                .map(|t| t.id)
                .collect()
        };
        assert_eq!(
            do_grupo(Grupo::Agil),
            [
                HISTORIA,
                CRITERIOS_GHERKIN,
                CENARIOS_FEATURE,
                EPICO,
                RESUMO_REFINAMENTO,
                DEFINITION_OF_READY
            ]
        );
        assert_eq!(
            do_grupo(Grupo::Rup),
            [
                CASO_DE_USO,
                CATALOGO_REGRAS,
                ESPECIFICACAO_SUPLEMENTAR,
                CASOS_DE_TESTE,
                DOCUMENTO_VISAO
            ]
        );
        assert_eq!(
            do_grupo(Grupo::Comuns),
            [PROTOTIPO, GLOSSARIO, MANUAL, DOCUMENTO_NEGOCIAL]
        );
        let ids: HashSet<_> = CATALOGO.iter().map(|t| t.id).collect();
        assert_eq!(ids.len(), CATALOGO.len(), "ids únicos");
        let templates: HashSet<_> = CATALOGO.iter().map(|t| t.template).collect();
        assert_eq!(templates.len(), CATALOGO.len(), "um template por tipo");
    }

    #[test]
    fn ciclo_de_vida_e_geracao_por_tipo() {
        use CicloDeVida::{PorDemanda, Vivo};
        use Geracao::{Agente, App};
        let esperado = [
            (HISTORIA, PorDemanda, Agente),
            (CRITERIOS_GHERKIN, PorDemanda, App),
            (CENARIOS_FEATURE, Vivo, App),
            (EPICO, PorDemanda, Agente),
            (RESUMO_REFINAMENTO, PorDemanda, Agente),
            (DEFINITION_OF_READY, PorDemanda, App),
            (CASO_DE_USO, Vivo, Agente),
            (CATALOGO_REGRAS, Vivo, App),
            (ESPECIFICACAO_SUPLEMENTAR, Vivo, App),
            (CASOS_DE_TESTE, PorDemanda, Agente),
            (DOCUMENTO_VISAO, PorDemanda, Agente),
            (PROTOTIPO, PorDemanda, Agente),
            (GLOSSARIO, Vivo, App),
            (MANUAL, Vivo, Agente),
            (DOCUMENTO_NEGOCIAL, Vivo, Agente),
        ];
        assert_eq!(esperado.len(), CATALOGO.len());
        for (id, ciclo, geracao) in esperado {
            let t = tipo(id).unwrap();
            assert_eq!((t.ciclo, t.geracao), (ciclo, geracao), "{id}");
        }
    }

    #[test]
    fn esquema_de_cada_tipo() {
        for t in &CATALOGO {
            assert!(!t.fontes.is_empty(), "{}: sem fonte", t.id);
            assert!(
                t.campos.iter().any(|c| c.obrigatorio),
                "{}: sem campo obrigatório",
                t.id
            );
            let nomes: HashSet<_> = t.campos.iter().map(|c| c.nome).collect();
            assert_eq!(nomes.len(), t.campos.len(), "{}: campos repetidos", t.id);
            // O documento vivo pertence a uma funcionalidade do mapa.
            if t.ciclo == CicloDeVida::Vivo && t.id != CASO_DE_USO {
                assert!(
                    t.campo("funcionalidade").is_some(),
                    "{}: vivo sem funcionalidade",
                    t.id
                );
                assert!(t.fontes.contains(&Fonte::Mapa), "{}: vivo sem o mapa", t.id);
            }
            // O que o agente redige cita a regra de origem de cada item; no
            // épico, de cada história.
            if t.geracao == Geracao::Agente {
                let campos = match t.campos.iter().find(|c| c.tipo == TipoCampo::Historias) {
                    Some(h) => {
                        assert!(h.obrigatorio, "{}", t.id);
                        assert!(
                            t.campo("regras").is_none(),
                            "{}: regras nas histórias",
                            t.id
                        );
                        CAMPOS_DA_HISTORIA_DO_EPICO
                    }
                    None => t.campos,
                };
                let regras = campos
                    .iter()
                    .find(|c| c.nome == "regras")
                    .unwrap_or_else(|| panic!("{}: sem regras", t.id));
                assert_eq!(
                    (regras.tipo, regras.obrigatorio),
                    (TipoCampo::Regras, true),
                    "{}",
                    t.id
                );
            }
        }
    }

    #[test]
    fn vivos_da_funcionalidade_sao_as_colunas_do_mapa() {
        let ids = |e: &Entregaveis| -> Vec<&str> {
            vivos_da_funcionalidade(e).iter().map(|t| t.id).collect()
        };
        // RUP: o glossário é da área e o protótipo é por demanda.
        let rup = predefinicao(Metodologia::Rup).unwrap().entregaveis();
        assert_eq!(
            ids(&rup),
            [CASO_DE_USO, CATALOGO_REGRAS, ESPECIFICACAO_SUPLEMENTAR]
        );
        // Com o manual habilitado, ele entra na ordem do catálogo.
        let mut com_manual = rup.clone();
        com_manual.tipos.push(TipoHabilitado::novo(MANUAL));
        assert_eq!(
            ids(&com_manual),
            [
                CASO_DE_USO,
                CATALOGO_REGRAS,
                ESPECIFICACAO_SUPLEMENTAR,
                MANUAL
            ]
        );
        let agil = predefinicao(Metodologia::Agil).unwrap().entregaveis();
        assert_eq!(ids(&agil), [CENARIOS_FEATURE]);
        assert_eq!(ids(&LEGADO.entregaveis()), [MANUAL, DOCUMENTO_NEGOCIAL]);
        assert!(!tipo(GLOSSARIO).unwrap().da_funcionalidade());
    }

    #[test]
    fn os_tipos_disponiveis_tem_o_template_embutido() {
        // O épico é montado pelo esquema (#463), sem template de documento.
        for t in CATALOGO.iter().filter(|t| t.disponivel && t.id != EPICO) {
            let doc = TipoDocumento::de_str(t.template).unwrap_or_else(|| {
                panic!("{}: template {} sem tipo de documento", t.id, t.template)
            });
            assert!(
                modelo::modelo(doc).is_some(),
                "{}: sem template embutido",
                t.id
            );
        }
        assert!(tipo(CASO_DE_USO).unwrap().disponivel);
        assert!(tipo(EPICO).unwrap().disponivel);
    }

    #[test]
    fn caso_de_uso_segue_as_secoes_do_template() {
        let m = modelo::modelo(TipoDocumento::CasoDeUso).unwrap();
        let uc = tipo(CASO_DE_USO).unwrap();
        for c in uc
            .campos
            .iter()
            .filter(|c| !matches!(c.nome, "titulo" | "regras"))
        {
            let s = m
                .secoes
                .iter()
                .find(|s| s.marcador == c.nome)
                .unwrap_or_else(|| panic!("{}: sem seção no template", c.nome));
            assert_eq!(s.titulo, c.rotulo, "{}", c.nome);
        }
        let ordem = |nome: &str| m.secoes.iter().position(|s| s.marcador == nome).unwrap();
        let campos: Vec<usize> = uc
            .campos
            .iter()
            .filter(|c| !matches!(c.nome, "titulo"))
            .map(|c| ordem(c.nome))
            .collect();
        assert!(campos.is_sorted(), "{campos:?}");
    }

    #[test]
    fn etapas_do_caso_de_uso_citam_a_regra() {
        let uc = tipo(CASO_DE_USO).unwrap();
        let mut campos = json!({
            "titulo": "Parcelar fatura",
            "objetivo": "Dividir a fatura em parcelas.",
            "atores": ["Atendente"],
            "fluxo_principal": [
                "O atendente abre uma fatura ABERTA (RN-FAT-010).",
                "O atendente informa a quantidade de parcelas.",
                "O sistema gera as parcelas (RN-FAT-011)."
            ],
            "fluxos_alternativos": ["Órgão público não tem valor mínimo de parcela (RN-FAT-009)."],
            "regras": ["RN-FAT-010"],
        });
        let erros = uc.validar_campos(&campos).unwrap_err();
        assert_eq!(erros.len(), 1);
        assert_eq!(erros[0].campo, "fluxo_principal");
        assert!(
            erros[0].motivo.starts_with("etapa 2 sem regra citada"),
            "{}",
            erros[0]
        );
        campos["fluxo_principal"][1] = json!("O atendente informa de 2 a 6 parcelas (RN-FAT-008).");
        assert_eq!(uc.validar_campos(&campos), Ok(()));
        assert_eq!(
            uc.regras_citadas(&campos),
            ["RN-FAT-010", "RN-FAT-008", "RN-FAT-011", "RN-FAT-009"]
        );
        let texto = uc.markdown_do_item(&campos, &[], 1);
        assert!(
            texto.contains(
                "## Fluxo principal\n\n1. O atendente abre uma fatura ABERTA (RN-FAT-010).\n\
                 2. O atendente informa de 2 a 6 parcelas (RN-FAT-008).\n3."
            ),
            "{texto}"
        );
        assert!(
            texto.contains(
                "## Fluxos alternativos\n\n- **FA1.** Órgão público não tem valor mínimo"
            ),
            "{texto}"
        );
    }

    #[test]
    fn numeracao_do_agente_sai_da_etapa() {
        assert_eq!(
            sem_numeracao("3. Gera as parcelas", None),
            "Gera as parcelas"
        );
        assert_eq!(
            sem_numeracao("3 parcelas no máximo", None),
            "3 parcelas no máximo"
        );
        assert_eq!(
            sem_numeracao("FA2 — Órgão público", Some("FA")),
            "Órgão público"
        );
        assert_eq!(sem_numeracao("FE1: Recusa", Some("FE")), "Recusa");
        assert_eq!(sem_numeracao("Recusa PF", Some("FE")), "Recusa PF");
    }

    #[test]
    fn regras_no_texto_acha_so_identificadores_de_regra() {
        assert_eq!(
            regras_no_texto("Passo (RN-FAT-001), RN-FAT-012; F-FAT-001, RN-sem e RN-FAT-001."),
            ["RN-FAT-001", "RN-FAT-012"]
        );
    }

    #[test]
    fn epico_tem_historias_no_esquema_da_historia_de_usuario() {
        let ep = tipo(EPICO).unwrap();
        let historia = |titulo: &str, regras: &[&str], depende: &[&str]| {
            json!({
                "titulo": titulo,
                "papel": "atendente",
                "acao": "parcelar a fatura de pessoa física",
                "beneficio": "atender o cliente PF",
                "criterios": ["Dado um cliente PF, quando pede 3 parcelas, então o parcelamento é aceito"],
                "regras": regras,
                "depende_de": depende,
            })
        };
        let mut campos = json!({
            "titulo": "Parcelamento para pessoa física",
            "objetivo": "Permitir que PF parcele em até 3 vezes.",
            "historias": [
                historia("Liberar PF", &["RN-FAT-007"], &[]),
                historia("Limites por tipo", &["RN-FAT-008", "RN-FAT-009"], &["H1"]),
            ],
            "riscos": ["O parâmetro único muda os outros tipos."],
        });
        assert_eq!(ep.validar_campos(&campos), Ok(()));
        assert_eq!(
            ep.regras_citadas(&campos),
            ["RN-FAT-007", "RN-FAT-008", "RN-FAT-009"]
        );
        let texto = ep.markdown_do_item(&campos, &[], 2);
        assert!(
            texto.starts_with("## Parcelamento para pessoa física"),
            "{texto}"
        );
        assert!(
            texto.contains("### Histórias (em ordem sugerida)\n\n#### H1. Liberar PF"),
            "{texto}"
        );
        assert!(texto.contains("#### H2. Limites por tipo"), "{texto}");
        assert!(texto.contains("**Depende de:** H1"), "{texto}");
        assert!(
            texto.contains("**Regras:** RN-FAT-008, RN-FAT-009"),
            "{texto}"
        );
        assert!(
            texto.contains("### Riscos\n\n- O parâmetro único"),
            "{texto}"
        );

        // Dependência para a frente, história sem regra e campo estranho.
        campos["historias"][0]["depende_de"] = json!(["H2"]);
        campos["historias"][1]["regras"] = json!([]);
        campos["historias"][1]["extra"] = json!(1);
        let erros = ep.validar_campos(&campos).unwrap_err();
        let nomes: Vec<&str> = erros.iter().map(|e| e.campo.as_str()).collect();
        assert_eq!(
            nomes,
            [
                "historias[H1].depende_de",
                "historias[H2].extra",
                "historias[H2].regras"
            ]
        );
        assert!(
            ep.validar_campos(&json!({"titulo": "x", "objetivo": "y", "historias": []}))
                .is_err()
        );
        assert_eq!(ep.sem_item(), "sem história no épico");
    }

    #[test]
    fn validar_campos_aceita_o_esquema_e_aponta_cada_problema() {
        let h = tipo(HISTORIA).unwrap();
        let ok = json!({
            "titulo": "Cancelar fatura em atraso",
            "papel": "atendente",
            "acao": "cancelar a fatura",
            "beneficio": "corrigir a cobrança",
            "criterios": ["A multa é recalculada"],
            "regras": ["RN-FAT-001"],
        });
        assert_eq!(h.validar_campos(&ok), Ok(()));

        let ruim = json!({
            "titulo": "Duas\nlinhas",
            "papel": "",
            "criterios": [],
            "regras": ["A-0001", "RN-FAT-1"],
            "extra": 1,
        });
        let erros = h.validar_campos(&ruim).unwrap_err();
        let campos: Vec<_> = erros.iter().map(|e| e.campo.as_str()).collect();
        assert_eq!(
            campos,
            [
                "extra",
                "titulo",
                "papel",
                "acao",
                "beneficio",
                "criterios",
                "regras"
            ]
        );
        assert!(erros[6].motivo.contains("A-0001"), "{}", erros[6]);

        // Opcionais podem faltar ou vir nulos; o tipo do valor ainda é conferido.
        let uc = tipo(CASO_DE_USO).unwrap();
        let base = json!({
            "titulo": "Cancelar fatura",
            "objetivo": "Cancelar.\n\nCom multa.",
            "atores": ["Atendente"],
            "fluxo_principal": ["Abre a fatura (RN-FAT-001)", "Cancela (RN-FAT-001)"],
            "excecoes": null,
            "regras": ["RN-FAT-001"],
        });
        assert_eq!(uc.validar_campos(&base), Ok(()));
        let mut com_gatilho = base.clone();
        com_gatilho["gatilho"] = json!(["não é texto"]);
        assert_eq!(
            uc.validar_campos(&com_gatilho).unwrap_err()[0].campo,
            "gatilho"
        );
        assert!(uc.validar_campos(&json!([])).is_err());
    }

    #[test]
    fn markdown_do_item_segue_o_esquema_sem_ia() {
        let h = tipo(HISTORIA).unwrap();
        let campos = json!({
            "titulo": "Cancelar fatura em atraso",
            "papel": "atendente",
            "acao": "cancelar a fatura",
            "beneficio": "corrigir a cobrança",
            "criterios": ["A multa é\nrecalculada", "O cliente é avisado"],
            "regras": ["RN-FAT-001"],
        });
        let regras = [("RN-FAT-001".to_owned(), "Multa de 2% ao mês".to_owned())];
        assert_eq!(
            h.markdown_do_item(&campos, &regras, 2),
            "## Cancelar fatura em atraso\n\n\
             **Como (papel):** atendente\n\n\
             **Quero (ação):** cancelar a fatura\n\n\
             **Para (benefício):** corrigir a cobrança\n\n\
             ### Critérios de aceite\n\n- A multa é recalculada\n- O cliente é avisado\n\n\
             ### Regras de origem\n\n- RN-FAT-001: Multa de 2% ao mês"
        );
        // Sem título, vale a funcionalidade ou o nome do tipo; opcionais vazios
        // ficam de fora.
        let uc = tipo(CASO_DE_USO).unwrap();
        let texto = uc.markdown_do_item(
            &json!({
                "objetivo": "Cancelar.",
                "atores": ["Atendente"],
                "excecoes": [],
                "fluxo_principal": ["Abre (RN-FAT-001)", "Cancela (RN-FAT-001)"],
                "regras": []
            }),
            &[],
            1,
        );
        assert!(texto.starts_with("# Especificação de caso de uso\n\n## Objetivo\n\nCancelar."));
        assert!(!texto.contains("Exceções"));
        assert!(!texto.contains("Regras de origem"));
        assert_eq!(h.sem_item(), "sem história");
        assert_eq!(
            tipo(GLOSSARIO).unwrap().sem_item(),
            "sem item no entregável"
        );
    }

    #[test]
    fn rup_habilita_caso_de_uso_como_padrao_regras_suplementar_prototipo_e_glossario() {
        let rup = aplicar(&efetiva(None), Metodologia::Rup);
        assert_eq!(rup.metodologia, Metodologia::Rup);
        assert_eq!(
            rup.ids().collect::<Vec<_>>(),
            [
                CASO_DE_USO,
                CATALOGO_REGRAS,
                ESPECIFICACAO_SUPLEMENTAR,
                PROTOTIPO,
                GLOSSARIO
            ]
        );
        assert_eq!(rup.padrao.as_deref(), Some(CASO_DE_USO));
        assert_eq!(validar(&rup), Ok(()));
        assert_eq!(normalizar(rup.clone()), rup);
    }

    #[test]
    fn agil_habilita_o_epico_como_padrao() {
        let agil = aplicar(&efetiva(None), Metodologia::Agil);
        assert_eq!(
            agil.ids().collect::<Vec<_>>(),
            [
                HISTORIA,
                CRITERIOS_GHERKIN,
                CENARIOS_FEATURE,
                EPICO,
                PROTOTIPO
            ]
        );
        assert_eq!(agil.padrao.as_deref(), Some(EPICO));
        assert_eq!(validar(&agil), Ok(()));
    }

    #[test]
    fn aplicar_mantem_template_e_destino_dos_que_continuam() {
        let mut atual = efetiva(None);
        let p = atual
            .tipos
            .iter_mut()
            .find(|t| t.tipo == PROTOTIPO)
            .unwrap();
        p.template = Some("prototipo-empresa.md".into());
        p.destino = Destino::ComentarioRastreador;
        let rup = aplicar(&atual, Metodologia::Rup);
        let p = rup.habilitado(PROTOTIPO).unwrap();
        assert_eq!(p.template.as_deref(), Some("prototipo-empresa.md"));
        assert_eq!(p.destino, Destino::ComentarioRastreador);
        assert_eq!(
            rup.habilitado(GLOSSARIO).unwrap(),
            &TipoHabilitado::novo(GLOSSARIO)
        );
        // Template e destino não tiram da metodologia.
        assert_eq!(normalizar(rup).metodologia, Metodologia::Rup);

        // Personalizado fica com os tipos que estavam.
        let pers = aplicar(&atual, Metodologia::Personalizado);
        assert_eq!(pers.tipos, atual.tipos);
        assert_eq!(pers.metodologia, Metodologia::Personalizado);
    }

    #[test]
    fn mudar_um_item_a_mao_passa_para_personalizado() {
        let rup = aplicar(&efetiva(None), Metodologia::Rup);
        let mut sem_glossario = rup.clone();
        sem_glossario.tipos.retain(|t| t.tipo != GLOSSARIO);
        assert_eq!(
            normalizar(sem_glossario).metodologia,
            Metodologia::Personalizado
        );

        let mut com_manual = rup.clone();
        com_manual.tipos.insert(0, TipoHabilitado::novo(MANUAL));
        let n = normalizar(com_manual);
        assert_eq!(n.metodologia, Metodologia::Personalizado);
        assert_eq!(n.tipos.last().unwrap().tipo, MANUAL, "na ordem do catálogo");

        let mut outro_padrao = rup;
        outro_padrao.padrao = Some(PROTOTIPO.into());
        assert_eq!(
            normalizar(outro_padrao).metodologia,
            Metodologia::Personalizado
        );
    }

    #[test]
    fn projeto_antigo_abre_com_os_tipos_que_ja_usava() {
        let e = efetiva(None);
        assert_eq!(e.metodologia, Metodologia::Personalizado);
        assert_eq!(
            e.ids().collect::<Vec<_>>(),
            [
                HISTORIA,
                CASOS_DE_TESTE,
                PROTOTIPO,
                MANUAL,
                DOCUMENTO_NEGOCIAL
            ]
        );
        assert_eq!(e.padrao.as_deref(), Some(HISTORIA));
        assert!(
            e.ids().all(|id| tipo(id).unwrap().disponivel),
            "só tipos que o app já gera"
        );
        assert_eq!(validar(&e), Ok(()));
        let gravada = aplicar(&e, Metodologia::Agil);
        assert_eq!(efetiva(Some(&gravada)), gravada);
    }

    #[test]
    fn historias_e_cenarios_so_quando_habilitados() {
        let rup = predefinicao(Metodologia::Rup).unwrap().entregaveis();
        assert!(!fluxo_oferecido(&rup, Fluxo::Historias));
        assert!(!fluxo_oferecido(&rup, Fluxo::Cenarios));
        assert_eq!(fluxos_ocultos(&rup), [Fluxo::Historias, Fluxo::Cenarios]);
        let erro = conferir_fluxo(&rup, Fluxo::Historias).unwrap_err();
        assert!(erro.contains("História de usuário"), "{erro}");
        // Os outros fluxos não dependem do catálogo.
        for f in [
            Fluxo::Perguntar,
            Fluxo::Explicar,
            Fluxo::Impacto,
            Fluxo::Manual,
            Fluxo::Prototipo,
        ] {
            assert!(fluxo_oferecido(&rup, f), "{f:?}");
        }

        // RUP com casos de teste: cenários, sem histórias.
        let mut com_casos = rup.clone();
        com_casos.tipos.push(TipoHabilitado::novo(CASOS_DE_TESTE));
        assert_eq!(fluxos_ocultos(&com_casos), [Fluxo::Historias]);

        // Ágil: a história e os tipos montados dos cenários.
        let agil = predefinicao(Metodologia::Agil).unwrap().entregaveis();
        assert!(fluxos_ocultos(&agil).is_empty());
        let mut so_feature = agil.clone();
        so_feature.tipos.retain(|t| t.tipo == CENARIOS_FEATURE);
        assert_eq!(fluxos_ocultos(&so_feature), [Fluxo::Historias]);

        // O projeto antigo (legado) continua com os dois.
        assert!(fluxos_ocultos(&efetiva(None)).is_empty());
        assert_eq!(conferir_fluxo(&efetiva(None), Fluxo::Cenarios), Ok(()));
    }

    #[test]
    fn validar_recusa_o_que_nao_cabe() {
        let ok = efetiva(None);
        let com = |f: &dyn Fn(&mut Entregaveis)| {
            let mut e = ok.clone();
            f(&mut e);
            validar(&e)
        };
        assert_eq!(
            com(&|e| e.metodologia = Metodologia::OrientadoAgentes),
            Err(ErroEntregaveis::MetodologiaIndisponivel)
        );
        assert_eq!(com(&|e| e.tipos.clear()), Err(ErroEntregaveis::SemTipos));
        assert_eq!(
            com(&|e| e.tipos.push(TipoHabilitado::novo("openspec"))),
            Err(ErroEntregaveis::TipoDesconhecido("openspec".into()))
        );
        assert_eq!(
            com(&|e| e.tipos.push(TipoHabilitado::novo(MANUAL))),
            Err(ErroEntregaveis::TipoRepetido(MANUAL.into()))
        );
        assert_eq!(com(&|e| e.padrao = None), Err(ErroEntregaveis::SemPadrao));
        assert_eq!(
            com(&|e| e.padrao = Some(GLOSSARIO.into())),
            Err(ErroEntregaveis::PadraoDesabilitado(GLOSSARIO.into()))
        );
        for ruim in ["../segredo.md", "pasta/x.md", "x.txt", ".md", ".oculto.md"] {
            assert!(
                matches!(
                    com(&|e| e.tipos[0].template = Some(ruim.into())),
                    Err(ErroEntregaveis::TemplateInvalido { .. })
                ),
                "{ruim}"
            );
        }
        assert_eq!(
            com(&|e| e.tipos[0].template = Some("historias_empresa-v2.md".into())),
            Ok(())
        );
    }
}
