//! Markdown para HTML (#166), sem a feature `html` do `pulldown-cmark`.
//!
//! O renderizador lê os eventos do Markdown já exportado (sem frontmatter, com
//! os links da base trocados pelo texto e, se for o caso, o apêndice) e escreve
//! o corpo do documento. O resultado vai no `{{conteudo}}` do template
//! ([`crate::ModeloHtml`]).
//!
//! Segurança: o arquivo abre no navegador de quem recebe, então nada do
//! documento sai como HTML cru.
//!
//! - Todo texto é escapado, inclusive o HTML que o documento tiver (`<script>`,
//!   `<b>`): ele aparece como texto, como na prévia do app.
//! - Só viram link (`href`) os endereços `http`, `https` e `mailto` e as âncoras
//!   do próprio documento. Os outros ficam como texto.
//! - Imagem do Markdown vira o texto alternativo e o endereço, sem `src`: o
//!   arquivo não carrega nada de fora.
//! - Diagrama Mermaid com figura renderizada pelo app ([`Figuras`]) sai como PNG
//!   embutido (`data:`). Sem figura, um aviso e o código, como no Word.
//!
//! Notas de rodapé: a referência `[^n]` vira `<sup>` com link para a nota
//! (`#nota-k`) e um `id` próprio (`ref-k-i`, a i-ésima citação da nota). A nota,
//! na seção de evidências, tem um link de volta para cada citação.
//!
//! Cada título de nível 2 abre uma `<section>`; a do apêndice tem a classe
//! `apendice`, para a impressão começar numa página nova.
//!
//! A saída é determinística.

use std::collections::HashMap;

use pulldown_cmark::{Alignment, CodeBlockKind, Event, HeadingLevel, Parser, Tag, TagEnd};

use crate::exportacao::opcoes_markdown;
use crate::figura::{Figura, Figuras, e_mermaid};
use crate::word::AVISO_DIAGRAMA;

/// Corpo HTML do Markdown.
pub(crate) fn corpo(markdown: &str, figuras: &Figuras) -> String {
    let mut r = Renderizador::novo(markdown, figuras);
    for evento in Parser::new_ext(markdown, opcoes_markdown()) {
        r.evento(evento);
    }
    r.fechar()
}

/// Escapa o texto para HTML (conteúdo e valor de atributo) e tira os caracteres
/// de controle.
pub(crate) fn escapar(texto: &str) -> String {
    let mut saida = String::with_capacity(texto.len());
    for c in texto.chars() {
        match c {
            '&' => saida.push_str("&amp;"),
            '<' => saida.push_str("&lt;"),
            '>' => saida.push_str("&gt;"),
            '"' => saida.push_str("&quot;"),
            '\'' => saida.push_str("&#39;"),
            '\t' | '\n' => saida.push(c),
            '\r' => {}
            c if c.is_control() => {}
            c => saida.push(c),
        }
    }
    saida
}

/// O endereço pode virar `href`: web, e-mail ou âncora do documento.
fn link_seguro(destino: &str) -> bool {
    let d = destino.trim().to_ascii_lowercase();
    d.starts_with("http://")
        || d.starts_with("https://")
        || d.starts_with("mailto:")
        || (d.starts_with('#') && d.len() > 1)
}

/// Classe da linguagem do bloco de código, só com caracteres seguros.
fn linguagem(tipo: &CodeBlockKind) -> String {
    match tipo {
        CodeBlockKind::Fenced(info) => info
            .split_whitespace()
            .next()
            .unwrap_or("")
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '+'))
            .collect::<String>()
            .to_ascii_lowercase(),
        CodeBlockKind::Indented => String::new(),
    }
}

/// Classe do selo de confiança, pelo texto em negrito.
fn classe_do_selo(texto: &str) -> Option<&'static str> {
    match texto {
        "[confirmado]" => Some("selo selo-confirmado"),
        "[inferido]" => Some("selo selo-inferido"),
        "[a confirmar]" => Some("selo selo-a-confirmar"),
        _ => None,
    }
}

fn tag_do_titulo(nivel: HeadingLevel) -> &'static str {
    match nivel {
        HeadingLevel::H1 => "h1",
        HeadingLevel::H2 => "h2",
        HeadingLevel::H3 => "h3",
        HeadingLevel::H4 => "h4",
        HeadingLevel::H5 => "h5",
        HeadingLevel::H6 => "h6",
    }
}

/// Base64 padrão (RFC 4648), para a figura embutida.
fn base64(bytes: &[u8]) -> String {
    const ALFABETO: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut saida = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for bloco in bytes.chunks(3) {
        let b = [
            bloco[0],
            *bloco.get(1).unwrap_or(&0),
            *bloco.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= bloco.len() {
                saida.push(ALFABETO[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                saida.push('=');
            }
        }
    }
    saida
}

/// Rótulos das notas de rodapé, na ordem em que aparecem (referência ou
/// definição), e quantas vezes cada uma é citada no texto (fora das notas).
fn mapa_de_notas(markdown: &str) -> (HashMap<String, usize>, HashMap<String, usize>) {
    let mut indices = HashMap::new();
    let mut citacoes: HashMap<String, usize> = HashMap::new();
    let mut em_definicao = false;
    for evento in Parser::new_ext(markdown, opcoes_markdown()) {
        match evento {
            Event::FootnoteReference(r) => {
                let proximo = indices.len() + 1;
                indices.entry(r.to_string()).or_insert(proximo);
                if !em_definicao {
                    *citacoes.entry(r.to_string()).or_default() += 1;
                }
            }
            Event::Start(Tag::FootnoteDefinition(r)) => {
                em_definicao = true;
                let proximo = indices.len() + 1;
                indices.entry(r.to_string()).or_insert(proximo);
            }
            Event::End(TagEnd::FootnoteDefinition) => em_definicao = false,
            _ => {}
        }
    }
    (indices, citacoes)
}

/// Título em escrita: o HTML de dentro e o texto, para a seção e o selo.
struct Titulo {
    nivel: HeadingLevel,
    html: String,
    texto: String,
}

struct Tabela {
    alinhamentos: Vec<Alignment>,
    cabecalho: bool,
    celula: usize,
}

struct Renderizador<'f> {
    saida: String,
    figuras: &'f Figuras,
    titulo: Option<Titulo>,
    /// Há uma `<section>` aberta.
    secao: bool,
    /// Blocos que contêm outros (citação, lista, nota, tabela): título dentro
    /// deles não abre seção.
    profundidade: usize,
    /// Bloco de código aberto: o código e o tipo.
    codigo: Option<(String, CodeBlockKind<'static>)>,
    diagramas: usize,
    tabela: Option<Tabela>,
    /// Para cada link aberto, se virou `<a>`, e o destino de imagem.
    links: Vec<Option<String>>,
    /// Posições (na saída corrente) dos `<strong>` abertos.
    negritos: Vec<usize>,
    indices: HashMap<String, usize>,
    citacoes: HashMap<String, usize>,
    /// Citações já escritas de cada nota.
    escritas: HashMap<String, usize>,
    em_definicao: Option<String>,
    /// Imagem da base aberta, embutida no fim: a figura e o texto alternativo.
    imagem: Option<(&'f Figura, String)>,
}

impl<'f> Renderizador<'f> {
    fn novo(markdown: &str, figuras: &'f Figuras) -> Self {
        let (indices, citacoes) = mapa_de_notas(markdown);
        Self {
            saida: String::new(),
            figuras,
            titulo: None,
            secao: false,
            profundidade: 0,
            codigo: None,
            diagramas: 0,
            tabela: None,
            links: Vec::new(),
            negritos: Vec::new(),
            indices,
            citacoes,
            escritas: HashMap::new(),
            em_definicao: None,
            imagem: None,
        }
    }

    /// Onde o HTML vai agora: o título em escrita ou a saída.
    fn alvo(&mut self) -> &mut String {
        match &mut self.titulo {
            Some(t) => &mut t.html,
            None => &mut self.saida,
        }
    }

    fn html(&mut self, html: &str) {
        self.alvo().push_str(html);
    }

    fn texto(&mut self, texto: &str) {
        if let Some(t) = &mut self.titulo {
            t.texto.push_str(texto);
        }
        let escapado = escapar(texto);
        self.html(&escapado);
    }

    fn indice(&self, rotulo: &str) -> usize {
        self.indices.get(rotulo).copied().unwrap_or(0)
    }

    fn evento(&mut self, evento: Event) {
        if let Some((codigo, _)) = &mut self.codigo {
            match evento {
                Event::Text(t) => codigo.push_str(&t),
                Event::End(TagEnd::CodeBlock) => self.fechar_codigo(),
                _ => {}
            }
            return;
        }
        // Dentro da imagem embutida, o texto vira o `alt`.
        if let Some((_, alt)) = &mut self.imagem {
            match evento {
                Event::Text(t) | Event::Code(t) => {
                    alt.push_str(&t);
                    return;
                }
                Event::End(TagEnd::Image) => {}
                _ => return,
            }
        }
        match evento {
            Event::Start(tag) => self.abrir(tag),
            Event::End(tag) => self.encerrar(tag),
            Event::Text(t) => self.texto(&t),
            Event::Code(t) => {
                if let Some(titulo) = &mut self.titulo {
                    titulo.texto.push_str(&t);
                }
                let html = format!("<code>{}</code>", escapar(&t));
                self.html(&html);
            }
            Event::InlineMath(t) | Event::DisplayMath(t) => self.texto(&t),
            // HTML do documento sai como texto, nunca cru.
            Event::Html(t) | Event::InlineHtml(t) => self.texto(&t),
            Event::SoftBreak => self.html("\n"),
            Event::HardBreak => self.html("<br>\n"),
            Event::Rule => self.html("<hr>\n"),
            Event::TaskListMarker(feito) => self.texto(if feito { "☒ " } else { "☐ " }),
            Event::FootnoteReference(rotulo) => self.referencia(&rotulo),
        }
    }

    fn referencia(&mut self, rotulo: &str) {
        let k = self.indice(rotulo);
        let texto = escapar(rotulo);
        if self.em_definicao.is_some() {
            // Dentro de uma nota, só o link para a outra nota.
            let html = format!("<a href=\"#nota-{k}\">[{texto}]</a>");
            self.html(&html);
            return;
        }
        let i = {
            let n = self.escritas.entry(rotulo.to_owned()).or_default();
            *n += 1;
            *n
        };
        let html = format!(
            "<sup class=\"ref-nota\"><a href=\"#nota-{k}\" id=\"ref-{k}-{i}\">[{texto}]</a></sup>"
        );
        self.html(&html);
    }

    fn abrir(&mut self, tag: Tag) {
        match tag {
            Tag::Paragraph => self.html("<p>"),
            Tag::Heading { level, .. } => {
                if level == HeadingLevel::H2 && self.profundidade == 0 {
                    self.titulo = Some(Titulo {
                        nivel: level,
                        html: String::new(),
                        texto: String::new(),
                    });
                } else {
                    self.html(&format!("<{}>", tag_do_titulo(level)));
                }
            }
            Tag::BlockQuote(_) => {
                self.profundidade += 1;
                self.html("<blockquote>\n");
            }
            Tag::CodeBlock(tipo) => self.codigo = Some((String::new(), tipo.into_static())),
            Tag::HtmlBlock => self.html("<p>"),
            Tag::List(Some(1)) => {
                self.profundidade += 1;
                self.html("<ol>\n");
            }
            Tag::List(Some(inicio)) => {
                self.profundidade += 1;
                self.html(&format!("<ol start=\"{inicio}\">\n"));
            }
            Tag::List(None) => {
                self.profundidade += 1;
                self.html("<ul>\n");
            }
            Tag::Item => self.html("<li>"),
            Tag::FootnoteDefinition(rotulo) => {
                self.profundidade += 1;
                let k = self.indice(&rotulo);
                let html = format!(
                    "<div class=\"nota\" id=\"nota-{k}\"><span class=\"nota-rotulo\">[{}]</span><div class=\"nota-corpo\">",
                    escapar(&rotulo)
                );
                self.html(&html);
                self.em_definicao = Some(rotulo.to_string());
            }
            Tag::Table(alinhamentos) => {
                self.profundidade += 1;
                self.tabela = Some(Tabela {
                    alinhamentos,
                    cabecalho: false,
                    celula: 0,
                });
                self.html("<table>\n");
            }
            Tag::TableHead => {
                if let Some(t) = &mut self.tabela {
                    t.cabecalho = true;
                    t.celula = 0;
                }
                self.html("<thead>\n<tr>");
            }
            Tag::TableRow => {
                if let Some(t) = &mut self.tabela {
                    t.celula = 0;
                }
                self.html("<tr>");
            }
            Tag::TableCell => {
                let (tag, alinhamento) = match &self.tabela {
                    Some(t) => (
                        if t.cabecalho { "th" } else { "td" },
                        t.alinhamentos.get(t.celula).copied(),
                    ),
                    None => ("td", None),
                };
                let classe = match alinhamento {
                    Some(Alignment::Center) => " class=\"alinhar-centro\"",
                    Some(Alignment::Right) => " class=\"alinhar-direita\"",
                    _ => "",
                };
                self.html(&format!("<{tag}{classe}>"));
            }
            Tag::Emphasis => self.html("<em>"),
            Tag::Strong => {
                let pos = self.alvo().len();
                self.negritos.push(pos);
                self.html("<strong>");
            }
            Tag::Strikethrough => self.html("<del>"),
            Tag::Superscript => self.html("<sup>"),
            Tag::Subscript => self.html("<sub>"),
            Tag::Link { dest_url, .. } => {
                if link_seguro(&dest_url) {
                    let html = format!("<a href=\"{}\">", escapar(dest_url.trim()));
                    self.html(&html);
                    self.links.push(None);
                } else {
                    // Fica só o texto do link.
                    self.links.push(Some(String::new()));
                }
            }
            Tag::Image { dest_url, .. } => {
                let figuras: &'f Figuras = self.figuras;
                if let Some(figura) = figuras.imagem(&dest_url) {
                    // Imagem da base entregue por quem exporta: PNG embutido,
                    // com o texto alternativo no `alt`.
                    self.imagem = Some((figura, String::new()));
                    return;
                }
                // Sem `src`: o texto alternativo e o endereço, como texto.
                self.html("<span class=\"imagem\">");
                self.links.push(Some(dest_url.to_string()));
            }
            Tag::DefinitionList
            | Tag::DefinitionListTitle
            | Tag::DefinitionListDefinition
            | Tag::MetadataBlock(_) => {}
        }
    }

    fn encerrar(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => self.html("</p>\n"),
            TagEnd::Heading(level) => match self.titulo.take() {
                Some(t) if t.nivel == level => self.secao(t),
                outro => {
                    self.titulo = outro;
                    self.html(&format!("</{}>\n", tag_do_titulo(level)));
                }
            },
            TagEnd::BlockQuote(_) => {
                self.profundidade -= 1;
                self.html("</blockquote>\n");
            }
            TagEnd::CodeBlock => {}
            TagEnd::HtmlBlock => self.html("</p>\n"),
            TagEnd::List(numerada) => {
                self.profundidade -= 1;
                self.html(if numerada { "</ol>\n" } else { "</ul>\n" });
            }
            TagEnd::Item => self.html("</li>\n"),
            TagEnd::FootnoteDefinition => {
                self.profundidade -= 1;
                let rotulo = self.em_definicao.take().unwrap_or_default();
                let k = self.indice(&rotulo);
                let n = self.citacoes.get(&rotulo).copied().unwrap_or(0);
                let mut voltas = String::new();
                for i in 1..=n {
                    let rotulo_volta = if n == 1 {
                        "Voltar à citação".to_owned()
                    } else {
                        format!("Voltar à citação {i}")
                    };
                    voltas.push_str(&format!(
                        "<a class=\"nota-voltar\" href=\"#ref-{k}-{i}\" title=\"{rotulo_volta}\" aria-label=\"{rotulo_volta}\">↩</a>"
                    ));
                }
                let html = format!("</div>{voltas}</div>\n");
                self.html(&html);
            }
            TagEnd::Table => {
                self.profundidade -= 1;
                self.tabela = None;
                self.html("</tbody>\n</table>\n");
            }
            TagEnd::TableHead => {
                if let Some(t) = &mut self.tabela {
                    t.cabecalho = false;
                }
                self.html("</tr>\n</thead>\n<tbody>\n");
            }
            TagEnd::TableRow => self.html("</tr>\n"),
            TagEnd::TableCell => {
                let tag = match &mut self.tabela {
                    Some(t) => {
                        t.celula += 1;
                        if t.cabecalho { "th" } else { "td" }
                    }
                    None => "td",
                };
                self.html(&format!("</{tag}>"));
            }
            TagEnd::Emphasis => self.html("</em>"),
            TagEnd::Strong => {
                let inicio = self.negritos.pop();
                let alvo = self.alvo();
                let abertura = "<strong>";
                if let Some(inicio) = inicio
                    && alvo[inicio..].starts_with(abertura)
                    && let Some(classe) = classe_do_selo(&alvo[inicio + abertura.len()..])
                {
                    alvo.replace_range(
                        inicio..inicio + abertura.len(),
                        &format!("<strong class=\"{classe}\">"),
                    );
                }
                self.html("</strong>");
            }
            TagEnd::Strikethrough => self.html("</del>"),
            TagEnd::Superscript => self.html("</sup>"),
            TagEnd::Subscript => self.html("</sub>"),
            TagEnd::Link => {
                if let Some(None) = self.links.pop() {
                    self.html("</a>");
                }
            }
            TagEnd::Image if self.imagem.is_some() => {
                let (figura, alt) = self.imagem.take().expect("imagem aberta");
                let html = format!(
                    "<img class=\"print\" alt=\"{}\" width=\"{}\" height=\"{}\" src=\"data:image/png;base64,{}\">",
                    escapar(alt.trim()),
                    figura.largura,
                    figura.altura,
                    base64(&figura.png)
                );
                self.html(&html);
            }
            TagEnd::Image => {
                if let Some(Some(destino)) = self.links.pop()
                    && !destino.trim().is_empty()
                {
                    self.texto(&format!(" ({})", destino.trim()));
                }
                self.html("</span>");
            }
            TagEnd::DefinitionList
            | TagEnd::DefinitionListTitle
            | TagEnd::DefinitionListDefinition
            | TagEnd::MetadataBlock(_) => {}
        }
    }

    /// Fecha a seção anterior e abre a do título de nível 2.
    fn secao(&mut self, t: Titulo) {
        if self.secao {
            self.saida.push_str("</section>\n");
        }
        let classe = if t.texto.trim_start().starts_with("Apêndice") {
            " class=\"apendice\""
        } else {
            ""
        };
        self.saida
            .push_str(&format!("<section{classe}>\n<h2>{}</h2>\n", t.html));
        self.secao = true;
    }

    fn fechar_codigo(&mut self) {
        let Some((codigo, tipo)) = self.codigo.take() else {
            return;
        };
        let codigo = codigo.strip_suffix('\n').unwrap_or(&codigo);
        if e_mermaid(&tipo) {
            self.diagramas += 1;
            let n = self.diagramas;
            let figuras: &'f Figuras = self.figuras;
            if let Some(figura) = figuras.figura(codigo) {
                let html = format!(
                    "<figure class=\"diagrama\"><img alt=\"Diagrama {n}\" width=\"{}\" height=\"{}\" src=\"data:image/png;base64,{}\"></figure>\n",
                    figura.largura,
                    figura.altura,
                    base64(&figura.png)
                );
                self.html(&html);
                return;
            }
            self.html(&format!(
                "<p class=\"aviso\"><strong>{AVISO_DIAGRAMA}</strong> O código Mermaid não pôde ser desenhado e foi mantido como texto; o restante do documento não foi afetado.</p>\n"
            ));
        }
        let linguagem = linguagem(&tipo);
        let classe = if linguagem.is_empty() {
            String::new()
        } else {
            format!(" class=\"language-{linguagem}\"")
        };
        let html = format!("<pre><code{classe}>{}</code></pre>\n", escapar(codigo));
        self.html(&html);
    }

    fn fechar(mut self) -> String {
        if let Some(t) = self.titulo.take() {
            self.secao(t);
        }
        if self.secao {
            self.saida.push_str("</section>\n");
        }
        self.saida
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn imagem_da_base_entregue_sai_embutida_com_alt() {
        let mut figuras = Figuras::novas();
        figuras
            .inserir_imagem(
                "manual/passo-01.png",
                Figura::nova(crate::figura::tests::png(40, 20), 0, 0).unwrap(),
            )
            .unwrap();
        let h = corpo(
            "![Tela \"a\" & b](manual/passo-01.png)\n\n![Sem](manual/outra.png)\n",
            &figuras,
        );
        assert!(
            h.contains("<img class=\"print\" alt=\"Tela &quot;a&quot; &amp; b\" width=\"40\" height=\"20\" src=\"data:image/png;base64,"),
            "{h}"
        );
        assert!(
            h.contains("<span class=\"imagem\">Sem (manual/outra.png)</span>"),
            "{h}"
        );
    }

    use super::*;

    fn html(md: &str) -> String {
        corpo(md, &Figuras::default())
    }

    #[test]
    fn escapa_html_do_documento() {
        let h = html(
            "Texto <b>negrito</b> e <script>alert(1)</script>.\n\n<div onclick=\"x\">bloco</div>\n",
        );
        assert!(!h.contains("<b>"), "{h}");
        assert!(!h.contains("<script"), "{h}");
        assert!(!h.contains("<div"), "{h}");
        assert!(h.contains("&lt;b&gt;negrito&lt;/b&gt;"), "{h}");
        assert!(h.contains("&lt;script&gt;alert(1)&lt;/script&gt;"), "{h}");
        assert!(
            h.contains("&lt;div onclick=&quot;x&quot;&gt;bloco&lt;/div&gt;"),
            "{h}"
        );
    }

    #[test]
    fn so_links_seguros_viram_href() {
        let h = html(
            "[a](https://x.com/?a=1&b=2) [b](javascript:alert(1)) [c](mailto:a@b.c) [d](ftp://x) ![img](https://x.com/i.png)\n",
        );
        assert!(
            h.contains("<a href=\"https://x.com/?a=1&amp;b=2\">a</a>"),
            "{h}"
        );
        assert!(h.contains("<a href=\"mailto:a@b.c\">c</a>"), "{h}");
        assert!(!h.contains("javascript"), "{h}");
        assert!(h.contains(" b "), "{h}");
        assert!(!h.contains("ftp://x\""), "{h}");
        assert!(!h.contains("src="), "{h}");
        assert!(
            h.contains("<span class=\"imagem\">img (https://x.com/i.png)</span>"),
            "{h}"
        );
    }

    #[test]
    fn notas_com_ida_e_volta() {
        let h = html(
            "Um.[^a] Dois.[^b] Três.[^a]\n\n## Evidências\n\n[^a]: `x.sql:1`\n[^b]: Ver [^a].\n",
        );
        assert!(
            h.contains("<sup class=\"ref-nota\"><a href=\"#nota-1\" id=\"ref-1-1\">[a]</a></sup>"),
            "{h}"
        );
        assert!(
            h.contains("<a href=\"#nota-2\" id=\"ref-2-1\">[b]</a>"),
            "{h}"
        );
        assert!(
            h.contains("<a href=\"#nota-1\" id=\"ref-1-2\">[a]</a>"),
            "{h}"
        );
        assert!(h.contains("<div class=\"nota\" id=\"nota-1\">"), "{h}");
        assert!(h.contains("href=\"#ref-1-1\""), "{h}");
        assert!(h.contains("href=\"#ref-1-2\""), "{h}");
        assert!(h.contains("href=\"#ref-2-1\""), "{h}");
        // A referência dentro da nota só aponta, sem id.
        assert!(h.contains("Ver <a href=\"#nota-1\">[a]</a>"), "{h}");
        assert_eq!(h.matches("id=\"ref-1-").count(), 2);
    }

    #[test]
    fn selos_secoes_tabela_e_codigo() {
        let h = html(
            "# T\n\n## A\n\n- **[confirmado]** x\n- **[a confirmar]** y\n- **outro**\n\n| a | b |\n|:-:|--:|\n| 1 | 2 |\n\n```sql\nselect 1 < 2\n```\n\n## Apêndice: trechos citados\n\nfim\n",
        );
        assert!(
            h.contains("<strong class=\"selo selo-confirmado\">[confirmado]</strong>"),
            "{h}"
        );
        assert!(
            h.contains("<strong class=\"selo selo-a-confirmar\">[a confirmar]</strong>"),
            "{h}"
        );
        assert!(h.contains("<strong>outro</strong>"), "{h}");
        assert!(h.starts_with("<h1>T</h1>\n<section>\n<h2>A</h2>\n"), "{h}");
        assert!(
            h.contains(
                "</section>\n<section class=\"apendice\">\n<h2>Apêndice: trechos citados</h2>"
            ),
            "{h}"
        );
        assert!(h.ends_with("</section>\n"), "{h}");
        assert!(
            h.contains("<th class=\"alinhar-centro\">a</th><th class=\"alinhar-direita\">b</th>"),
            "{h}"
        );
        assert!(h.contains("<td class=\"alinhar-centro\">1</td>"), "{h}");
        assert!(
            h.contains("<pre><code class=\"language-sql\">select 1 &lt; 2</code></pre>"),
            "{h}"
        );
    }

    #[test]
    fn base64_conhecido() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }
}
