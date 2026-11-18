//! Template e tokens CSS da exportação em HTML (#166).
//!
//! Seguem o padrão dos outros textos editáveis da base:
//!
//! - o padrão é embutido no app (`crates/cba-docs/exportacao/`);
//! - a cópia editável fica em `<base>/_templates/exportacao/` e é usada no lugar
//!   do padrão ([`ModeloHtml::da_base`]); arquivo ilegível gera aviso e vale o
//!   padrão;
//! - o template ([`ArquivoHtml::Documento`]) é HTML com variáveis `{{nome}}`
//!   ([`VARIAVEIS_HTML`]). Um comentário HTML no início é instrução para quem
//!   edita e não sai no arquivo. Variável desconhecida gera aviso e fica como
//!   está. `{{conteudo}}` é essencial: se faltar, o documento vai para o fim do
//!   arquivo, com aviso;
//! - os tokens ([`ArquivoHtml::Tokens`]) são um bloco `:root` com custom
//!   properties. A folha padrão do documento usa só `var(--…)`, então a
//!   identidade visual muda trocando só os tokens. O [`ArquivoHtml::Estilo`],
//!   opcional, vem depois da folha padrão, para quem quer ir além.
//!
//! Os valores das variáveis saem escapados; só `{{css}}` e `{{conteudo}}` são
//! HTML (e CSS) montados pelo app.

use std::fmt;

use cba_kb::{Base, CaminhoRelativo, TipoDocumento, layout};

use crate::html::escapar;
use crate::template::Origem;

/// Pasta dos arquivos da exportação em HTML, dentro de `_templates/`.
pub const PASTA_EXPORTACAO: &str = "exportacao";

const PADRAO_DOCUMENTO: &str = include_str!("../exportacao/documento.html");
const PADRAO_TOKENS: &str = include_str!("../exportacao/tokens.css");
const PADRAO_ESTILO: &str = include_str!("../exportacao/estilo.css");
const EXEMPLO: &str = include_str!("../exportacao/exemplo.md");
/// Folha de estilo padrão do documento: usa só os tokens.
pub const FOLHA_PADRAO: &str = include_str!("../exportacao/folha.css");

/// Variáveis do template HTML, com a descrição para o editor.
pub const VARIAVEIS_HTML: [(&str, &str); 8] = [
    (
        "titulo",
        "Título do documento (o primeiro título do texto).",
    ),
    ("id", "Identificador do documento (A-0001, R-0001)."),
    ("projeto", "Projeto do documento."),
    (
        "tipo",
        "Tipo do documento (Análise de impacto, Ata de reunião...).",
    ),
    ("status", "Status do documento (rascunho, aprovada...)."),
    ("data_exportacao", "Data da exportação (AAAA-MM-DD)."),
    (
        "css",
        "Estilo do documento: os tokens, a folha padrão e o estilo adicional. Vai dentro de <style>.",
    ),
    (
        "conteudo",
        "O documento em si. Essencial: se faltar, vai para o fim do arquivo.",
    ),
];

/// Variável sem a qual o arquivo perderia o documento.
const ESSENCIAL: &str = "conteudo";

/// Um dos arquivos editáveis da exportação em HTML.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArquivoHtml {
    /// `documento.html`: o template.
    Documento,
    /// `tokens.css`: as custom properties.
    Tokens,
    /// `estilo.css`: regras adicionais, opcionais.
    Estilo,
}

impl ArquivoHtml {
    pub const TODOS: [ArquivoHtml; 3] = [Self::Documento, Self::Tokens, Self::Estilo];

    /// Nome do arquivo (`documento.html`).
    pub fn nome(self) -> &'static str {
        match self {
            Self::Documento => "documento.html",
            Self::Tokens => "tokens.css",
            Self::Estilo => "estilo.css",
        }
    }

    /// Identificador curto (`documento`, `tokens`, `estilo`).
    pub fn id(self) -> &'static str {
        match self {
            Self::Documento => "documento",
            Self::Tokens => "tokens",
            Self::Estilo => "estilo",
        }
    }

    pub fn de_id(id: &str) -> Option<Self> {
        Self::TODOS.into_iter().find(|a| a.id() == id)
    }

    /// Nome para o analista.
    pub fn rotulo(self) -> &'static str {
        match self {
            Self::Documento => "Template HTML da exportação",
            Self::Tokens => "Tokens CSS da exportação",
            Self::Estilo => "Estilo adicional da exportação",
        }
    }

    /// `_templates/exportacao/<nome>`, relativo à raiz da base.
    pub fn arquivo(self) -> CaminhoRelativo {
        CaminhoRelativo::novo(&format!(
            "{}/{PASTA_EXPORTACAO}/{}",
            layout::TEMPLATES,
            self.nome()
        ))
        .expect("pasta e nome fixos formam um caminho válido")
    }

    /// Texto padrão embutido no app.
    pub fn padrao(self) -> &'static str {
        match self {
            Self::Documento => PADRAO_DOCUMENTO,
            Self::Tokens => PADRAO_TOKENS,
            Self::Estilo => PADRAO_ESTILO,
        }
    }
}

/// Algo que o analista deve saber sobre o template ou os tokens, sem impedir a
/// exportação.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AvisoHtml {
    /// O template tem uma variável que não existe; ficou no arquivo como está.
    VariavelDesconhecida {
        origem: Origem,
        variavel: String,
        /// Linha no template (a partir de 1).
        linha: usize,
    },
    /// O template não usa `{{conteudo}}`; o documento foi para o fim.
    ConteudoAusente { origem: Origem },
    /// O arquivo da base não pôde ser lido; o padrão foi usado.
    Ilegivel {
        arquivo: CaminhoRelativo,
        erro: String,
    },
}

impl fmt::Display for AvisoHtml {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::VariavelDesconhecida {
                origem,
                variavel,
                linha,
            } => write!(
                f,
                "{origem}, linha {linha}: variável desconhecida `{{{{{variavel}}}}}`"
            ),
            Self::ConteudoAusente { origem } => write!(
                f,
                "{origem} não usa a variável `{{{{{ESSENCIAL}}}}}`; o documento foi para o fim do arquivo"
            ),
            Self::Ilegivel { arquivo, erro } => {
                write!(f, "{arquivo}: arquivo ilegível ({erro}); usado o padrão")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TextoHtml {
    texto: String,
    origem: Origem,
}

/// Template HTML, tokens e estilo adicional da exportação.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModeloHtml {
    documento: TextoHtml,
    tokens: TextoHtml,
    estilo: TextoHtml,
}

impl Default for ModeloHtml {
    fn default() -> Self {
        Self::padrao()
    }
}

impl ModeloHtml {
    /// Os textos padrão embutidos no app.
    pub fn padrao() -> Self {
        let padrao = |a: ArquivoHtml| TextoHtml {
            texto: a.padrao().to_owned(),
            origem: Origem::Padrao,
        };
        Self {
            documento: padrao(ArquivoHtml::Documento),
            tokens: padrao(ArquivoHtml::Tokens),
            estilo: padrao(ArquivoHtml::Estilo),
        }
    }

    /// Os textos da base, onde existem e são legíveis; senão, o padrão. Devolve
    /// os avisos de arquivo ilegível e os do template (variável desconhecida,
    /// `{{conteudo}}` ausente).
    pub fn da_base(base: &Base) -> (Self, Vec<AvisoHtml>) {
        let mut modelo = Self::padrao();
        let mut avisos = Vec::new();
        for arquivo in ArquivoHtml::TODOS {
            let relativo = arquivo.arquivo();
            if !base.caminho(&relativo).is_file() {
                continue;
            }
            match base.ler_texto(&relativo) {
                Ok(texto) => modelo = modelo.com(arquivo, texto, Origem::Base(relativo)),
                Err(erro) => avisos.push(AvisoHtml::Ilegivel {
                    arquivo: relativo,
                    erro: erro.to_string(),
                }),
            }
        }
        avisos.extend(validar_template_html(
            &modelo.documento.texto,
            modelo.documento.origem.clone(),
        ));
        (modelo, avisos)
    }

    /// O mesmo modelo com outro texto para `arquivo` (a prévia do editor).
    pub fn com(mut self, arquivo: ArquivoHtml, texto: String, origem: Origem) -> Self {
        let novo = TextoHtml { texto, origem };
        match arquivo {
            ArquivoHtml::Documento => self.documento = novo,
            ArquivoHtml::Tokens => self.tokens = novo,
            ArquivoHtml::Estilo => self.estilo = novo,
        }
        self
    }

    /// O texto em uso de `arquivo`.
    pub fn texto(&self, arquivo: ArquivoHtml) -> &str {
        match arquivo {
            ArquivoHtml::Documento => &self.documento.texto,
            ArquivoHtml::Tokens => &self.tokens.texto,
            ArquivoHtml::Estilo => &self.estilo.texto,
        }
    }

    /// CSS do documento: tokens, folha padrão e estilo adicional. Um `</` vira
    /// `<\/`, para o CSS não fechar o `<style>`.
    pub fn css(&self) -> String {
        let css = format!(
            "{}\n{}\n{}",
            self.tokens.texto.trim_end(),
            FOLHA_PADRAO.trim_end(),
            self.estilo.texto.trim_end()
        );
        css.replace("</", "<\\/")
    }

    /// Arquivo final: o template com as variáveis preenchidas pelos metadados e
    /// o corpo HTML.
    pub(crate) fn montar(&self, meta: &Metadados, conteudo: &str) -> (String, Vec<AvisoHtml>) {
        let css = self.css();
        preencher(&self.documento, |nome| match nome {
            "titulo" => Some(escapar(&meta.titulo)),
            "id" => Some(escapar(&meta.id)),
            "projeto" => Some(escapar(&meta.projeto)),
            "tipo" => Some(escapar(&meta.tipo)),
            "status" => Some(escapar(&meta.status)),
            "data_exportacao" => Some(escapar(&meta.data_exportacao)),
            "css" => Some(css.clone()),
            "conteudo" => Some(conteudo.to_owned()),
            _ => None,
        })
    }
}

/// Trechos fixos do documento de exemplo, para o apêndice da prévia.
struct TrechosDeExemplo;

impl crate::FonteDeTrechos for TrechosDeExemplo {
    fn trecho(
        &self,
        arquivo: &CaminhoRelativo,
        linhas: cba_kb::frontmatter::Linhas,
    ) -> Option<String> {
        let todas = match arquivo.as_str() {
            "db/parametros.sql" => {
                "-- Parâmetros do faturamento\nCREATE TABLE parametro (nome VARCHAR(40), valor NUMBER);\nINSERT INTO parametro VALUES ('MAX_PARCELAS', 6);\nINSERT INTO parametro VALUES ('VALOR_MINIMO', 100);"
            }
            _ => return None,
        };
        let todas: Vec<&str> = todas.lines().collect();
        let inicio = linhas.inicio as usize;
        let fim = (linhas.fim as usize).min(todas.len());
        (inicio >= 1 && inicio <= fim).then(|| todas[inicio - 1..fim].join("\n"))
    }
}

/// Prévia do editor: um documento de exemplo exportado com `modelo`, com os
/// avisos do template.
pub fn previa_html(modelo: &ModeloHtml) -> crate::HtmlExportado {
    crate::exportar_html(
        EXEMPLO,
        &crate::OpcoesExportacao::default(),
        Some(&TrechosDeExemplo),
        None,
        &crate::Figuras::default(),
        modelo,
        &hoje(),
    )
}

/// Avisos do template HTML, sem montar nada: variável desconhecida e
/// `{{conteudo}}` ausente. Para o editor e para a exportação.
pub fn validar_template_html(texto: &str, origem: Origem) -> Vec<AvisoHtml> {
    let t = TextoHtml {
        texto: texto.to_owned(),
        origem,
    };
    preencher(&t, |n| {
        VARIAVEIS_HTML
            .iter()
            .any(|(v, _)| *v == n)
            .then(String::new)
    })
    .1
}

/// Substitui as variáveis conhecidas. `{{conteudo}}` ausente vai antes do
/// `</body>` (ou no fim), com aviso. Os valores não são reexaminados.
fn preencher(t: &TextoHtml, valor: impl Fn(&str) -> Option<String>) -> (String, Vec<AvisoHtml>) {
    let texto = t.texto.replace("\r\n", "\n");
    let (deslocamento, corpo) = sem_comentario_inicial(&texto);
    let mut saida = String::with_capacity(corpo.len());
    let mut avisos = Vec::new();
    let mut usou_essencial = false;
    let mut resto = corpo;
    let mut linha = 1 + deslocamento;
    while let Some(inicio) = resto.find("{{") {
        let antes = &resto[..inicio];
        saida.push_str(antes);
        linha += antes.matches('\n').count();
        let depois = &resto[inicio + 2..];
        let nome = depois
            .find("}}")
            .map(|fim| (fim, depois[..fim].trim()))
            .filter(|(_, n)| e_nome_de_variavel(n));
        let Some((fim, nome)) = nome else {
            saida.push_str("{{");
            resto = depois;
            continue;
        };
        let original = &resto[inicio..inicio + 2 + fim + 2];
        match valor(nome) {
            Some(v) => {
                saida.push_str(&v);
                usou_essencial |= nome == ESSENCIAL;
            }
            None => {
                avisos.push(AvisoHtml::VariavelDesconhecida {
                    origem: t.origem.clone(),
                    variavel: nome.to_owned(),
                    linha,
                });
                saida.push_str(original);
            }
        }
        linha += original.matches('\n').count();
        resto = &resto[inicio + original.len()..];
    }
    saida.push_str(resto);
    if !usou_essencial && let Some(v) = valor(ESSENCIAL) {
        avisos.push(AvisoHtml::ConteudoAusente {
            origem: t.origem.clone(),
        });
        let bloco = format!("\n<main class=\"documento\">\n{v}</main>\n");
        match saida.to_ascii_lowercase().rfind("</body>") {
            Some(pos) => saida.insert_str(pos, &bloco),
            None => saida.push_str(&bloco),
        }
    }
    (saida, avisos)
}

fn e_nome_de_variavel(nome: &str) -> bool {
    !nome.is_empty()
        && nome
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

/// Remove o comentário HTML do início, se houver. Devolve quantas linhas foram
/// removidas (para as linhas dos avisos) e o resto.
fn sem_comentario_inicial(texto: &str) -> (usize, &str) {
    let sem_espacos = texto.trim_start();
    if !sem_espacos.starts_with("<!--") {
        return (0, texto);
    }
    let Some(fim) = sem_espacos.find("-->") else {
        return (0, texto);
    };
    let resto = sem_espacos[fim + 3..].trim_start_matches(['\n', ' ', '\t']);
    let removido = &texto[..texto.len() - resto.len()];
    (removido.matches('\n').count(), resto)
}

/// Nome do tipo de documento para o leitor (`Análise de impacto`).
pub fn rotulo_do_tipo(tipo: TipoDocumento) -> &'static str {
    use TipoDocumento as T;
    match tipo {
        T::AnaliseImpacto => "Análise de impacto",
        T::Pergunta => "Pergunta ao sistema",
        T::Funcionalidade => "Documento negocial da funcionalidade",
        T::CasoDeUso => "Caso de uso",
        T::Regra => "Regra de negócio",
        T::Termo => "Termo do glossário",
        T::Sugestao => "Sugestão de alteração",
        T::Projeto => "Projeto",
        T::MemoriaArea => "Memória da área",
        T::Demanda => "Demanda",
        T::Pessoa => "Pessoa",
        T::Ata => "Ata de reunião",
        T::AnaliseReuniao => "Análise de reunião",
        T::ParecerRevisao => "Parecer de revisão",
        T::Historias => "Histórias e critérios de aceite",
        T::CenariosTeste => "Cenários de teste",
        T::Premissa => "Premissa",
        T::Decisao => "Decisão",
        T::Risco => "Risco",
        T::Restricao => "Restrição",
    }
}

/// Dados do documento para as variáveis do template.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Metadados {
    pub titulo: String,
    pub id: String,
    pub projeto: String,
    pub tipo: String,
    pub status: String,
    pub data_exportacao: String,
}

impl Metadados {
    /// Lê os campos de primeiro nível do frontmatter (`id`, `projeto`, `tipo`,
    /// `status`, `titulo`). O título é o primeiro `#` do corpo exportado; sem
    /// ele, o `titulo` do frontmatter ou o id.
    pub fn do_documento(documento: &str, corpo_markdown: &str, data_exportacao: &str) -> Self {
        let campo = |nome: &str| campo_do_frontmatter(documento, nome).unwrap_or_default();
        let tipo = campo("tipo");
        let tipo = TipoDocumento::de_str(&tipo)
            .map(|t| rotulo_do_tipo(t).to_owned())
            .unwrap_or(tipo);
        let id = campo("id");
        let titulo = primeiro_titulo(corpo_markdown)
            .or_else(|| campo_do_frontmatter(documento, "titulo"))
            .unwrap_or_else(|| id.clone());
        Self {
            titulo,
            id,
            projeto: campo("projeto"),
            tipo,
            status: campo("status"),
            data_exportacao: data_exportacao.to_owned(),
        }
    }
}

/// Valor escalar de um campo de primeiro nível do frontmatter, sem aspas.
fn campo_do_frontmatter(documento: &str, nome: &str) -> Option<String> {
    let texto = documento.strip_prefix('\u{feff}').unwrap_or(documento);
    let mut linhas = texto.lines();
    if linhas.next()?.trim_end() != "---" {
        return None;
    }
    for linha in linhas {
        if linha.trim_end() == "---" {
            break;
        }
        let Some((chave, valor)) = linha.split_once(':') else {
            continue;
        };
        if chave != nome {
            continue;
        }
        let valor = valor.trim();
        let valor = valor
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .or_else(|| valor.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
            .unwrap_or(valor);
        if valor.is_empty() || valor == "null" || valor == "~" {
            return None;
        }
        return Some(valor.to_owned());
    }
    None
}

/// Texto do primeiro título de nível 1.
fn primeiro_titulo(markdown: &str) -> Option<String> {
    use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};
    let mut texto: Option<String> = None;
    for evento in Parser::new_ext(markdown, crate::exportacao::opcoes_markdown()) {
        match evento {
            Event::Start(Tag::Heading {
                level: HeadingLevel::H1,
                ..
            }) => texto = Some(String::new()),
            Event::Text(t) | Event::Code(t) | Event::InlineHtml(t) => {
                if let Some(s) = &mut texto {
                    s.push_str(&t);
                }
            }
            Event::End(TagEnd::Heading(HeadingLevel::H1)) => {
                return texto.filter(|t| !t.trim().is_empty());
            }
            _ => {}
        }
    }
    None
}

/// Data UTC de hoje (AAAA-MM-DD), para quem exporta sem informar a data.
pub(crate) fn hoje() -> String {
    let segundos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let dias = (segundos / 86_400) as i64;
    // Algoritmo "civil from days" (Howard Hinnant).
    let z = dias + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let dia = doy - (153 * mp + 2) / 5 + 1;
    let mes = if mp < 10 { mp + 3 } else { mp - 9 };
    let ano = yoe + era * 400 + i64::from(mes <= 2);
    format!("{ano:04}-{mes:02}-{dia:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta() -> Metadados {
        Metadados {
            titulo: "A <b>1</b>".into(),
            id: "A-0001".into(),
            projeto: "p".into(),
            tipo: "Análise de impacto".into(),
            status: "rascunho".into(),
            data_exportacao: "2026-11-17".into(),
        }
    }

    fn modelo(documento: &str) -> ModeloHtml {
        ModeloHtml::padrao().com(ArquivoHtml::Documento, documento.into(), Origem::Padrao)
    }

    #[test]
    fn preenche_escapa_e_tira_o_comentario() {
        let (s, avisos) = modelo("<!--\ninstrução\n-->\n<title>{{ titulo }}</title>{{inventada}}\n<main>{{conteudo}}</main>")
            .montar(&meta(), "<p>x</p>");
        assert_eq!(
            s,
            "<title>A &lt;b&gt;1&lt;/b&gt;</title>{{inventada}}\n<main><p>x</p></main>"
        );
        assert_eq!(
            avisos,
            vec![AvisoHtml::VariavelDesconhecida {
                origem: Origem::Padrao,
                variavel: "inventada".into(),
                linha: 4,
            }]
        );
    }

    #[test]
    fn conteudo_ausente_vai_antes_do_fim_do_body() {
        let (s, avisos) =
            modelo("<html><body><h1>{{titulo}}</h1></BODY></html>").montar(&meta(), "<p>x</p>");
        assert_eq!(
            s,
            "<html><body><h1>A &lt;b&gt;1&lt;/b&gt;</h1>\n<main class=\"documento\">\n<p>x</p></main>\n</BODY></html>"
        );
        assert!(matches!(avisos[..], [AvisoHtml::ConteudoAusente { .. }]));
        assert!(avisos[0].to_string().contains("`{{conteudo}}`"));
    }

    #[test]
    fn css_nao_fecha_o_style() {
        let m = ModeloHtml::padrao().com(
            ArquivoHtml::Estilo,
            "</style><script>x</script>".into(),
            Origem::Padrao,
        );
        let css = m.css();
        assert!(!css.contains("</"), "{css}");
        assert!(css.starts_with(PADRAO_TOKENS.trim_end()));
    }

    #[test]
    fn padrao_valido_e_folha_so_com_tokens() {
        assert!(validar_template_html(PADRAO_DOCUMENTO, Origem::Padrao).is_empty());
        // A folha padrão não fixa cores nem fontes: só usa os tokens.
        let sem_comentarios: String = FOLHA_PADRAO
            .split("/*")
            .map(|p| p.split_once("*/").map_or(p, |(_, r)| r))
            .collect();
        assert!(!sem_comentarios.contains('#'), "cor fixa na folha padrão");
        for propriedade in ["color:", "font-family:", "background:"] {
            for linha in sem_comentarios.lines().filter(|l| l.contains(propriedade)) {
                for decl in linha.split(';').filter(|d| d.contains(propriedade)) {
                    let valor = decl.split_once(propriedade).unwrap().1.trim();
                    assert!(
                        valor.starts_with("var(--")
                            || ["none", "inherit"].contains(&valor.trim_end_matches(" }").trim()),
                        "{decl}"
                    );
                }
            }
        }
        // Cada token usado está definido.
        let mut resto = sem_comentarios.as_str();
        while let Some(i) = resto.find("var(--") {
            let nome = &resto[i + 4..];
            let fim = nome.find(')').unwrap();
            assert!(
                PADRAO_TOKENS.contains(&format!("{}:", &nome[..fim])),
                "{}",
                &nome[..fim]
            );
            resto = &nome[fim..];
        }
    }

    #[test]
    fn metadados_do_frontmatter() {
        let doc = "---\ntipo: ata\nid: R-0001\nprojeto: \"faturamento\"\nstatus: rascunho\ndemanda:\n  id: null\n---\n";
        let m = Metadados::do_documento(doc, "# R-0001 · Ata\n", "2026-11-17");
        assert_eq!(m.titulo, "R-0001 · Ata");
        assert_eq!(m.tipo, "Ata de reunião");
        assert_eq!(m.projeto, "faturamento");
        assert_eq!(m.status, "rascunho");
        assert_eq!(m.id, "R-0001");
        let sem = Metadados::do_documento("sem frontmatter", "texto", "d");
        assert_eq!(sem.titulo, "");
        assert_eq!(sem.tipo, "");
    }

    #[test]
    fn previa_com_o_exemplo() {
        let p = previa_html(&ModeloHtml::padrao());
        assert!(p.avisos.is_empty());
        assert!(
            p.html
                .contains("<title>A-0000 · Documento de exemplo</title>")
        );
        assert!(p.html.contains("selo-a-confirmar"));
        assert!(p.html.contains("Apêndice: trechos citados"));
        assert!(p.html.contains("MAX_PARCELAS"));
        assert!(p.html.contains("Trecho indisponível"));
        let m = ModeloHtml::padrao().com(
            ArquivoHtml::Tokens,
            ":root { --cor-destaque: #c00; }".into(),
            Origem::Padrao,
        );
        assert!(previa_html(&m).html.contains("--cor-destaque: #c00;"));
    }

    #[test]
    fn data_de_hoje_no_formato() {
        let d = hoje();
        assert_eq!(d.len(), 10);
        assert!(d.starts_with("20"));
        assert_eq!(&d[4..5], "-");
    }
}
