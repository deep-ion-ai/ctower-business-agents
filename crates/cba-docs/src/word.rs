//! Markdown para Word (`.docx`, Office Open XML), sem dependências nativas.
//!
//! O Markdown é lido com `pulldown-cmark` e o pacote é escrito à mão (o XML das
//! partes) e compactado com `zip`. Mapeamento:
//!
//! | Markdown                      | Word                                           |
//! |-------------------------------|------------------------------------------------|
//! | `#` a `######`                | estilos `Heading1` a `Heading6` ("heading n")  |
//! | parágrafo                     | estilo `Normal`                                |
//! | `**negrito**`, `_itálico_`, `~~riscado~~` | `w:b`, `w:i`, `w:strike`           |
//! | `` `código` ``                | estilo de caractere `CodigoEmLinha`            |
//! | lista com marcador / numerada | `numbering.xml` (cada lista numerada reinicia) |
//! | bloco de código               | estilo `Codigo`, linhas com `w:br`             |
//! | citação `>`                   | estilo `Citacao`                               |
//! | tabela                        | `w:tbl` com bordas e linha de título repetida  |
//! | `[^n]` (referência)           | nota de rodapé do Word, uma por ocorrência     |
//! | `[^n]: ...` (definição)       | parágrafo `Evidencia` com `[n]` no lugar       |
//! | link externo                  | texto seguido do endereço entre parênteses     |
//! | `---`                         | parágrafo com borda inferior                   |
//!
//! O Word numera as notas de rodapé na ordem em que aparecem; para ligar a nota
//! à lista de evidências e ao apêndice, o texto de cada nota começa com
//! `Evidência [n]:`.
//!
//! A saída é determinística: o mesmo Markdown dá sempre os mesmos bytes.

use std::collections::HashMap;
use std::io::{Cursor, Write};

use pulldown_cmark::{Alignment, Event, HeadingLevel, Parser, Tag, TagEnd};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::exportacao::opcoes_markdown;

const NS_W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const NS_R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const NS_REL: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const REL_DOC: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

/// Recuo de cada nível de lista, em twips.
const RECUO: u32 = 360;
/// Largura útil da página A4 com as margens usadas, em twips.
const LARGURA_UTIL: u32 = 8504;

/// Gera o `.docx` do Markdown.
pub(crate) fn docx(markdown: &str) -> zip::result::ZipResult<Vec<u8>> {
    let mut escritor = Escritor::novo(markdown);
    for evento in Parser::new_ext(markdown, opcoes_markdown()) {
        escritor.evento(evento);
    }
    escritor.fechar_paragrafo();
    escritor.empacotar()
}

/// Formatação de caractere em vigor.
#[derive(Default, Clone)]
struct Formato {
    negrito: u32,
    italico: u32,
    riscado: u32,
    sobrescrito: u32,
    subscrito: u32,
}

/// Monta os runs (`w:r`) de um trecho em linha.
#[derive(Default)]
struct EmLinha {
    xml: String,
    formato: Formato,
    /// Destinos dos links abertos, para escrever o endereço depois do texto.
    links: Vec<(String, String)>,
    /// Último run de texto, ainda aberto: propriedades e texto. Textos seguidos
    /// com a mesma formatação vão num run só (o leitor de Markdown parte
    /// `[confirmado]` em três textos).
    pendente: Option<(String, String)>,
}

impl EmLinha {
    /// Trata um evento em linha. Devolve `false` se o evento não é em linha.
    fn evento(&mut self, evento: &Event) -> bool {
        match evento {
            Event::Text(t) => self.texto(t, None),
            Event::Code(t) => self.texto(t, Some("CodigoEmLinha")),
            Event::InlineMath(t) | Event::DisplayMath(t) => self.texto(t, None),
            Event::Html(t) | Event::InlineHtml(t) => self.texto(t, None),
            Event::SoftBreak => self.texto(" ", None),
            Event::HardBreak => self.bruto("<w:r><w:br/></w:r>"),
            Event::TaskListMarker(feito) => self.texto(if *feito { "☒ " } else { "☐ " }, None),
            Event::Start(Tag::Strong) => self.formato.negrito += 1,
            Event::End(TagEnd::Strong) => self.formato.negrito -= 1,
            Event::Start(Tag::Emphasis) => self.formato.italico += 1,
            Event::End(TagEnd::Emphasis) => self.formato.italico -= 1,
            Event::Start(Tag::Strikethrough) => self.formato.riscado += 1,
            Event::End(TagEnd::Strikethrough) => self.formato.riscado -= 1,
            Event::Start(Tag::Superscript) => self.formato.sobrescrito += 1,
            Event::End(TagEnd::Superscript) => self.formato.sobrescrito -= 1,
            Event::Start(Tag::Subscript) => self.formato.subscrito += 1,
            Event::End(TagEnd::Subscript) => self.formato.subscrito -= 1,
            Event::Start(Tag::Link { dest_url, .. } | Tag::Image { dest_url, .. }) => {
                self.links.push((dest_url.to_string(), String::new()));
            }
            Event::End(TagEnd::Link | TagEnd::Image) => {
                if let Some((destino, texto)) = self.links.pop()
                    && !destino.is_empty()
                    && destino != texto
                {
                    self.texto(&format!(" ({destino})"), None);
                }
            }
            _ => return false,
        }
        true
    }

    fn texto(&mut self, texto: &str, estilo: Option<&str>) {
        if let Some((_, t)) = self.links.last_mut() {
            t.push_str(texto);
        }
        let mut rpr = String::new();
        if let Some(e) = estilo {
            rpr.push_str(&format!("<w:rStyle w:val=\"{e}\"/>"));
        }
        if self.formato.negrito > 0 {
            rpr.push_str("<w:b/>");
        }
        if self.formato.italico > 0 {
            rpr.push_str("<w:i/>");
        }
        if self.formato.riscado > 0 {
            rpr.push_str("<w:strike/>");
        }
        if self.formato.sobrescrito > 0 {
            rpr.push_str("<w:vertAlign w:val=\"superscript\"/>");
        } else if self.formato.subscrito > 0 {
            rpr.push_str("<w:vertAlign w:val=\"subscript\"/>");
        }
        match &mut self.pendente {
            Some((anterior, acumulado)) if *anterior == rpr => acumulado.push_str(texto),
            _ => {
                self.descarregar();
                self.pendente = Some((rpr, texto.to_owned()));
            }
        }
    }

    /// XML pronto (um run especial), depois do texto pendente.
    fn bruto(&mut self, xml: &str) {
        self.descarregar();
        self.xml.push_str(xml);
    }

    fn descarregar(&mut self) {
        if let Some((rpr, texto)) = self.pendente.take() {
            self.xml.push_str("<w:r>");
            if !rpr.is_empty() {
                self.xml.push_str(&format!("<w:rPr>{rpr}</w:rPr>"));
            }
            self.xml.push_str(&conteudo_do_run(&texto));
            self.xml.push_str("</w:r>");
        }
    }

    /// Os runs, com o texto pendente.
    fn runs(mut self) -> String {
        self.descarregar();
        self.xml
    }
}

/// `w:t`, `w:tab` e `w:br` do texto.
fn conteudo_do_run(texto: &str) -> String {
    let mut saida = String::new();
    let mut atual = String::new();
    let fechar = |atual: &mut String, saida: &mut String| {
        if !atual.is_empty() {
            saida.push_str(&format!(
                "<w:t xml:space=\"preserve\">{}</w:t>",
                escapar(atual)
            ));
            atual.clear();
        }
    };
    for c in texto.chars() {
        match c {
            '\t' => {
                fechar(&mut atual, &mut saida);
                saida.push_str("<w:tab/>");
            }
            '\n' => {
                fechar(&mut atual, &mut saida);
                saida.push_str("<w:br/>");
            }
            '\r' => {}
            c => atual.push(c),
        }
    }
    fechar(&mut atual, &mut saida);
    saida
}

/// Escapa o texto para XML e tira os caracteres que o XML 1.0 não aceita.
fn escapar(texto: &str) -> String {
    let mut saida = String::with_capacity(texto.len());
    for c in texto.chars() {
        match c {
            '&' => saida.push_str("&amp;"),
            '<' => saida.push_str("&lt;"),
            '>' => saida.push_str("&gt;"),
            '"' => saida.push_str("&quot;"),
            '\t' | '\n' | '\r' => saida.push(c),
            c if (c as u32) < 0x20 || c == '\u{fffe}' || c == '\u{ffff}' => {}
            c => saida.push(c),
        }
    }
    saida
}

struct Lista {
    num_id: u32,
    /// O item atual já tem o parágrafo com o número ou marcador.
    item_com_paragrafo: bool,
}

struct Tabela {
    alinhamentos: Vec<Alignment>,
    linha: String,
    celulas: usize,
    linhas: String,
    cabecalho: bool,
}

/// Parágrafo em montagem.
struct Paragrafo {
    ppr: String,
    em_linha: EmLinha,
}

struct Escritor {
    corpo: String,
    paragrafo: Option<Paragrafo>,
    /// Formatação herdada pelos parágrafos novos (negrito do cabeçalho da tabela).
    negrito_herdado: bool,
    listas: Vec<Lista>,
    citacao: usize,
    tabela: Option<Tabela>,
    celula: Option<usize>,
    codigo: Option<String>,
    /// Rótulo da definição de nota em escrita, até o primeiro parágrafo.
    nota_em_definicao: Option<String>,
    em_definicao: bool,
    /// Eventos em linha de cada definição de nota, pelo rótulo.
    definicoes: HashMap<String, Vec<Event<'static>>>,
    /// `w:footnote` das notas de rodapé, na ordem.
    notas: Vec<String>,
    /// Listas numeradas e com marcador: `(numId, numerada, nível, início)`.
    numeracoes: Vec<(u32, bool, usize, u64)>,
    /// Texto do primeiro título, para as propriedades do documento.
    titulo: Option<String>,
    titulo_aberto: bool,
    ultimo_bloco_tabela: bool,
}

impl Escritor {
    fn novo(markdown: &str) -> Self {
        Self {
            corpo: String::new(),
            paragrafo: None,
            negrito_herdado: false,
            listas: Vec::new(),
            citacao: 0,
            tabela: None,
            celula: None,
            codigo: None,
            nota_em_definicao: None,
            em_definicao: false,
            definicoes: definicoes(markdown),
            notas: Vec::new(),
            numeracoes: Vec::new(),
            titulo: None,
            titulo_aberto: false,
            ultimo_bloco_tabela: false,
        }
    }

    fn evento(&mut self, evento: Event) {
        if let Some(codigo) = &mut self.codigo {
            match evento {
                Event::Text(t) => codigo.push_str(&t),
                Event::End(TagEnd::CodeBlock) => {
                    let codigo = self.codigo.take().unwrap_or_default();
                    let codigo = codigo.strip_suffix('\n').unwrap_or(&codigo);
                    self.bloco(&format!(
                        "<w:p><w:pPr><w:pStyle w:val=\"Codigo\"/></w:pPr><w:r>{}</w:r></w:p>",
                        conteudo_do_run(codigo)
                    ));
                }
                _ => {}
            }
            return;
        }
        match evento {
            Event::Start(Tag::Paragraph) => {
                self.fechar_paragrafo();
                self.abrir_paragrafo();
            }
            Event::End(TagEnd::Paragraph) => self.fechar_paragrafo(),
            Event::Start(Tag::Heading { level, .. }) => {
                self.fechar_paragrafo();
                let n = nivel(level);
                self.paragrafo = Some(Paragrafo {
                    ppr: format!("<w:pStyle w:val=\"Heading{n}\"/>"),
                    em_linha: EmLinha::default(),
                });
                self.titulo_aberto = self.titulo.is_none();
            }
            Event::End(TagEnd::Heading(_)) => {
                self.titulo_aberto = false;
                self.fechar_paragrafo();
            }
            Event::Start(Tag::CodeBlock(_)) => {
                self.fechar_paragrafo();
                self.codigo = Some(String::new());
            }
            Event::Start(Tag::BlockQuote(_)) => {
                self.fechar_paragrafo();
                self.citacao += 1;
            }
            Event::End(TagEnd::BlockQuote(_)) => {
                self.fechar_paragrafo();
                self.citacao -= 1;
            }
            Event::Start(Tag::List(inicio)) => {
                self.fechar_paragrafo();
                let num_id = self.numeracoes.len() as u32 + 1;
                self.numeracoes.push((
                    num_id,
                    inicio.is_some(),
                    self.listas.len().min(8),
                    inicio.unwrap_or(1),
                ));
                self.listas.push(Lista {
                    num_id,
                    item_com_paragrafo: false,
                });
            }
            Event::End(TagEnd::List(_)) => {
                self.fechar_paragrafo();
                self.listas.pop();
            }
            Event::Start(Tag::Item) => {
                self.fechar_paragrafo();
                if let Some(l) = self.listas.last_mut() {
                    l.item_com_paragrafo = false;
                }
            }
            Event::End(TagEnd::Item) => self.fechar_paragrafo(),
            Event::Start(Tag::FootnoteDefinition(rotulo)) => {
                self.fechar_paragrafo();
                self.em_definicao = true;
                self.nota_em_definicao = Some(rotulo.to_string());
            }
            Event::End(TagEnd::FootnoteDefinition) => {
                self.fechar_paragrafo();
                self.em_definicao = false;
                self.nota_em_definicao = None;
            }
            Event::Start(Tag::Table(alinhamentos)) => {
                self.fechar_paragrafo();
                self.tabela = Some(Tabela {
                    alinhamentos,
                    linha: String::new(),
                    celulas: 0,
                    linhas: String::new(),
                    cabecalho: false,
                });
            }
            Event::Start(Tag::TableHead) => {
                if let Some(t) = &mut self.tabela {
                    t.cabecalho = true;
                }
                self.negrito_herdado = true;
            }
            Event::Start(Tag::TableRow) => {}
            Event::End(TagEnd::TableHead | TagEnd::TableRow) => {
                self.negrito_herdado = false;
                if let Some(t) = &mut self.tabela {
                    while t.celulas < t.alinhamentos.len() {
                        t.linha
                            .push_str("<w:tc><w:tcPr><w:tcW w:w=\"0\" w:type=\"auto\"/></w:tcPr><w:p/></w:tc>");
                        t.celulas += 1;
                    }
                    let trpr = if t.cabecalho {
                        "<w:trPr><w:tblHeader/></w:trPr>"
                    } else {
                        ""
                    };
                    t.linhas.push_str(&format!(
                        "<w:tr>{trpr}{}</w:tr>",
                        std::mem::take(&mut t.linha)
                    ));
                    t.celulas = 0;
                    t.cabecalho = false;
                }
            }
            Event::Start(Tag::TableCell) => {
                let indice = self.tabela.as_ref().map_or(0, |t| t.celulas);
                self.celula = Some(indice);
                self.abrir_paragrafo();
            }
            Event::End(TagEnd::TableCell) => {
                let p = self.paragrafo.take().map(montar_paragrafo);
                self.celula = None;
                if let Some(t) = &mut self.tabela {
                    t.linha.push_str(&format!(
                        "<w:tc><w:tcPr><w:tcW w:w=\"0\" w:type=\"auto\"/></w:tcPr>{}</w:tc>",
                        p.unwrap_or_else(|| "<w:p/>".into())
                    ));
                    t.celulas += 1;
                }
            }
            Event::End(TagEnd::Table) => {
                if let Some(t) = self.tabela.take() {
                    let colunas = t.alinhamentos.len().max(1) as u32;
                    let largura = LARGURA_UTIL / colunas;
                    let grade: String = (0..colunas)
                        .map(|_| format!("<w:gridCol w:w=\"{largura}\"/>"))
                        .collect();
                    let borda = |lado: &str| {
                        format!(
                            "<w:{lado} w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"808080\"/>"
                        )
                    };
                    let bordas: String = ["top", "left", "bottom", "right", "insideH", "insideV"]
                        .iter()
                        .map(|l| borda(l))
                        .collect();
                    self.bloco(&format!(
                        "<w:tbl><w:tblPr><w:tblW w:w=\"5000\" w:type=\"pct\"/><w:tblBorders>{bordas}</w:tblBorders><w:tblLook w:val=\"04A0\" w:firstRow=\"1\" w:lastRow=\"0\" w:firstColumn=\"0\" w:lastColumn=\"0\" w:noHBand=\"1\" w:noVBand=\"1\"/></w:tblPr><w:tblGrid>{grade}</w:tblGrid>{}</w:tbl>",
                        t.linhas
                    ));
                    self.ultimo_bloco_tabela = true;
                }
            }
            Event::Rule => {
                self.fechar_paragrafo();
                self.bloco("<w:p><w:pPr><w:pBdr><w:bottom w:val=\"single\" w:sz=\"6\" w:space=\"1\" w:color=\"808080\"/></w:pBdr></w:pPr></w:p>");
            }
            Event::FootnoteReference(rotulo) => {
                if self.em_definicao {
                    self.em_linha().texto(&format!("[{rotulo}]"), None);
                } else {
                    let referencia = self.nota(&rotulo);
                    self.em_linha().bruto(&referencia);
                }
            }
            Event::Start(Tag::HtmlBlock) | Event::End(TagEnd::HtmlBlock) => {}
            Event::Html(t) => {
                // HTML de bloco: uma linha por parágrafo.
                self.em_linha().texto(t.trim_end(), None);
                self.fechar_paragrafo();
            }
            outro => {
                if self.titulo_aberto
                    && let Event::Text(t) | Event::Code(t) = &outro
                {
                    self.titulo_texto(t);
                }
                self.em_linha().evento(&outro);
            }
        }
    }

    fn titulo_texto(&mut self, t: &str) {
        let titulo = self.titulo.get_or_insert_with(String::new);
        titulo.push_str(t);
    }

    /// Bloco no corpo (ou na célula da tabela, se houver).
    fn bloco(&mut self, xml: &str) {
        self.corpo.push_str(xml);
        self.ultimo_bloco_tabela = false;
    }

    /// Runs do parágrafo aberto, abrindo um se preciso (itens de lista "tight"
    /// têm texto sem parágrafo).
    fn em_linha(&mut self) -> &mut EmLinha {
        if self.paragrafo.is_none() {
            self.abrir_paragrafo();
        }
        &mut self.paragrafo.as_mut().expect("parágrafo aberto").em_linha
    }

    fn abrir_paragrafo(&mut self) {
        let mut ppr = String::new();
        let mut em_linha = EmLinha::default();
        if self.negrito_herdado {
            em_linha.formato.negrito = 1;
        }
        if let Some(indice) = self.celula {
            let alinhamento = self
                .tabela
                .as_ref()
                .and_then(|t| t.alinhamentos.get(indice).copied())
                .unwrap_or(Alignment::None);
            match alinhamento {
                Alignment::Center => ppr.push_str("<w:jc w:val=\"center\"/>"),
                Alignment::Right => ppr.push_str("<w:jc w:val=\"right\"/>"),
                _ => {}
            }
            ppr.insert_str(0, "<w:spacing w:after=\"0\"/>");
        } else if self.em_definicao {
            ppr.push_str("<w:pStyle w:val=\"Evidencia\"/>");
            if let Some(rotulo) = self.nota_em_definicao.take() {
                em_linha.texto(&format!("[{rotulo}] "), None);
            }
        } else if !self.listas.is_empty() {
            let nivel = self.listas.len() - 1;
            let lista = &mut self.listas[nivel];
            ppr.push_str("<w:pStyle w:val=\"ListParagraph\"/>");
            if lista.item_com_paragrafo {
                ppr.push_str(&format!(
                    "<w:ind w:left=\"{}\"/>",
                    RECUO * 2 * (nivel as u32 + 1)
                ));
            } else {
                lista.item_com_paragrafo = true;
                ppr.push_str(&format!(
                    "<w:numPr><w:ilvl w:val=\"{}\"/><w:numId w:val=\"{}\"/></w:numPr>",
                    nivel.min(8),
                    lista.num_id
                ));
            }
        } else if self.citacao > 0 {
            ppr.push_str("<w:pStyle w:val=\"Citacao\"/>");
        }
        self.paragrafo = Some(Paragrafo { ppr, em_linha });
    }

    fn fechar_paragrafo(&mut self) {
        if self.celula.is_some() {
            return;
        }
        if let Some(p) = self.paragrafo.take() {
            let xml = montar_paragrafo(p);
            self.bloco(&xml);
        }
    }

    /// Cria a nota de rodapé da referência e devolve o run da referência.
    fn nota(&mut self, rotulo: &str) -> String {
        let id = self.notas.len() + 1;
        let mut em_linha = EmLinha::default();
        em_linha.texto(&format!(" Evidência [{rotulo}]: "), None);
        match self.definicoes.get(rotulo) {
            Some(eventos) => {
                for e in eventos {
                    if let Event::FootnoteReference(r) = e {
                        em_linha.texto(&format!("[{r}]"), None);
                    } else {
                        em_linha.evento(e);
                    }
                }
            }
            None => em_linha.texto("sem definição.", None),
        }
        self.notas.push(format!(
            "<w:footnote w:id=\"{id}\"><w:p><w:pPr><w:pStyle w:val=\"FootnoteText\"/></w:pPr><w:r><w:rPr><w:rStyle w:val=\"FootnoteReference\"/></w:rPr><w:footnoteRef/></w:r>{}</w:p></w:footnote>",
            em_linha.runs()
        ));
        format!(
            "<w:r><w:rPr><w:rStyle w:val=\"FootnoteReference\"/></w:rPr><w:footnoteReference w:id=\"{id}\"/></w:r>"
        )
    }

    fn empacotar(mut self) -> zip::result::ZipResult<Vec<u8>> {
        if self.ultimo_bloco_tabela || self.corpo.is_empty() {
            self.corpo.push_str("<w:p/>");
        }
        let documento = format!(
            "{XML}<w:document xmlns:w=\"{NS_W}\" xmlns:r=\"{NS_R}\"><w:body>{}<w:sectPr><w:pgSz w:w=\"11906\" w:h=\"16838\"/><w:pgMar w:top=\"1417\" w:right=\"1701\" w:bottom=\"1417\" w:left=\"1701\" w:header=\"708\" w:footer=\"708\" w:gutter=\"0\"/></w:sectPr></w:body></w:document>",
            self.corpo
        );
        let notas = format!(
            "{XML}<w:footnotes xmlns:w=\"{NS_W}\" xmlns:r=\"{NS_R}\"><w:footnote w:type=\"separator\" w:id=\"-1\"><w:p><w:pPr><w:spacing w:after=\"0\" w:line=\"240\" w:lineRule=\"auto\"/></w:pPr><w:r><w:separator/></w:r></w:p></w:footnote><w:footnote w:type=\"continuationSeparator\" w:id=\"0\"><w:p><w:pPr><w:spacing w:after=\"0\" w:line=\"240\" w:lineRule=\"auto\"/></w:pPr><w:r><w:continuationSeparator/></w:r></w:p></w:footnote>{}</w:footnotes>",
            self.notas.concat()
        );
        let titulo = self.titulo.as_deref().unwrap_or("Documento");
        let propriedades = format!(
            "{XML}<cp:coreProperties xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:dcterms=\"http://purl.org/dc/terms/\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"><dc:title>{}</dc:title></cp:coreProperties>",
            escapar(titulo)
        );
        let partes: [(&str, &str); 9] = [
            ("[Content_Types].xml", TIPOS),
            ("_rels/.rels", &rels_pacote()),
            ("docProps/core.xml", &propriedades),
            ("word/document.xml", &documento),
            ("word/_rels/document.xml.rels", &rels_documento()),
            ("word/styles.xml", &estilos()),
            ("word/numbering.xml", &numeracao(&self.numeracoes)),
            ("word/footnotes.xml", &notas),
            ("word/settings.xml", &configuracoes()),
        ];
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        let opcoes = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .last_modified_time(zip::DateTime::default());
        for (nome, xml) in partes {
            zip.start_file(nome, opcoes)?;
            zip.write_all(xml.as_bytes())?;
        }
        Ok(zip.finish()?.into_inner())
    }
}

fn montar_paragrafo(p: Paragrafo) -> String {
    let ppr = if p.ppr.is_empty() {
        String::new()
    } else {
        format!("<w:pPr>{}</w:pPr>", p.ppr)
    };
    format!("<w:p>{ppr}{}</w:p>", p.em_linha.runs())
}

fn nivel(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

/// Eventos em linha de cada definição de nota (parágrafos juntados por espaço).
fn definicoes(markdown: &str) -> HashMap<String, Vec<Event<'static>>> {
    let mut mapa: HashMap<String, Vec<Event<'static>>> = HashMap::new();
    let mut atual: Option<(String, Vec<Event<'static>>, bool)> = None;
    for evento in Parser::new_ext(markdown, opcoes_markdown()) {
        match evento {
            Event::Start(Tag::FootnoteDefinition(r)) => {
                atual = Some((r.to_string(), Vec::new(), false));
            }
            Event::End(TagEnd::FootnoteDefinition) => {
                if let Some((r, eventos, _)) = atual.take() {
                    mapa.entry(r).or_insert(eventos);
                }
            }
            Event::Start(Tag::Paragraph) => {
                if let Some((_, eventos, ja)) = &mut atual {
                    if *ja {
                        eventos.push(Event::SoftBreak);
                    }
                    *ja = true;
                }
            }
            e => {
                if let Some((_, eventos, _)) = &mut atual {
                    eventos.push(e.into_static());
                }
            }
        }
    }
    mapa
}

const XML: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n";

const TIPOS: &str = concat!(
    "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n",
    "<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">",
    "<Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>",
    "<Default Extension=\"xml\" ContentType=\"application/xml\"/>",
    "<Override PartName=\"/word/document.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml\"/>",
    "<Override PartName=\"/word/styles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml\"/>",
    "<Override PartName=\"/word/numbering.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml\"/>",
    "<Override PartName=\"/word/footnotes.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml\"/>",
    "<Override PartName=\"/word/settings.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml\"/>",
    "<Override PartName=\"/docProps/core.xml\" ContentType=\"application/vnd.openxmlformats-package.core-properties+xml\"/>",
    "</Types>"
);

fn rels_pacote() -> String {
    format!(
        "{XML}<Relationships xmlns=\"{NS_REL}\"><Relationship Id=\"rId1\" Type=\"{REL_DOC}/officeDocument\" Target=\"word/document.xml\"/><Relationship Id=\"rId2\" Type=\"http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties\" Target=\"docProps/core.xml\"/></Relationships>"
    )
}

fn rels_documento() -> String {
    let rel = |id: u32, tipo: &str| {
        format!("<Relationship Id=\"rId{id}\" Type=\"{REL_DOC}/{tipo}\" Target=\"{tipo}.xml\"/>")
    };
    format!(
        "{XML}<Relationships xmlns=\"{NS_REL}\">{}{}{}{}</Relationships>",
        rel(1, "styles"),
        rel(2, "numbering"),
        rel(3, "footnotes"),
        rel(4, "settings"),
    )
}

fn configuracoes() -> String {
    format!(
        "{XML}<w:settings xmlns:w=\"{NS_W}\"><w:defaultTabStop w:val=\"708\"/><w:characterSpacingControl w:val=\"doNotCompress\"/><w:footnotePr><w:footnote w:id=\"-1\"/><w:footnote w:id=\"0\"/></w:footnotePr><w:compat><w:compatSetting w:name=\"compatibilityMode\" w:uri=\"http://schemas.microsoft.com/office/word\" w:val=\"15\"/></w:compat></w:settings>"
    )
}

fn estilos() -> String {
    let mut s = format!(
        "{XML}<w:styles xmlns:w=\"{NS_W}\"><w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii=\"Calibri\" w:hAnsi=\"Calibri\" w:eastAsia=\"Calibri\" w:cs=\"Calibri\"/><w:sz w:val=\"22\"/><w:szCs w:val=\"22\"/><w:lang w:val=\"pt-BR\"/></w:rPr></w:rPrDefault><w:pPrDefault><w:pPr><w:spacing w:after=\"120\" w:line=\"264\" w:lineRule=\"auto\"/></w:pPr></w:pPrDefault></w:docDefaults>"
    );
    s.push_str("<w:style w:type=\"paragraph\" w:default=\"1\" w:styleId=\"Normal\"><w:name w:val=\"Normal\"/><w:qFormat/></w:style>");
    let tamanhos = [36, 30, 26, 24, 22, 22];
    for (i, tamanho) in tamanhos.iter().enumerate() {
        let n = i + 1;
        s.push_str(&format!(
            "<w:style w:type=\"paragraph\" w:styleId=\"Heading{n}\"><w:name w:val=\"heading {n}\"/><w:basedOn w:val=\"Normal\"/><w:next w:val=\"Normal\"/><w:qFormat/><w:pPr><w:keepNext/><w:keepLines/><w:spacing w:before=\"{}\" w:after=\"120\"/><w:outlineLvl w:val=\"{i}\"/></w:pPr><w:rPr><w:b/><w:color w:val=\"1F3864\"/><w:sz w:val=\"{tamanho}\"/><w:szCs w:val=\"{tamanho}\"/></w:rPr></w:style>",
            if n == 1 { 240 } else { 200 }
        ));
    }
    s.push_str("<w:style w:type=\"paragraph\" w:styleId=\"ListParagraph\"><w:name w:val=\"List Paragraph\"/><w:basedOn w:val=\"Normal\"/><w:qFormat/><w:pPr><w:spacing w:after=\"60\"/><w:ind w:left=\"720\"/></w:pPr></w:style>");
    s.push_str("<w:style w:type=\"paragraph\" w:styleId=\"Codigo\"><w:name w:val=\"Código\"/><w:basedOn w:val=\"Normal\"/><w:pPr><w:keepLines/><w:pBdr><w:top w:val=\"single\" w:sz=\"4\" w:space=\"4\" w:color=\"D0D0D0\"/><w:left w:val=\"single\" w:sz=\"4\" w:space=\"4\" w:color=\"D0D0D0\"/><w:bottom w:val=\"single\" w:sz=\"4\" w:space=\"4\" w:color=\"D0D0D0\"/><w:right w:val=\"single\" w:sz=\"4\" w:space=\"4\" w:color=\"D0D0D0\"/></w:pBdr><w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"F4F4F4\"/><w:spacing w:before=\"60\" w:after=\"180\" w:line=\"240\" w:lineRule=\"auto\"/><w:ind w:left=\"113\" w:right=\"113\"/></w:pPr><w:rPr><w:rFonts w:ascii=\"Consolas\" w:hAnsi=\"Consolas\" w:cs=\"Consolas\"/><w:noProof/><w:sz w:val=\"18\"/><w:szCs w:val=\"18\"/></w:rPr></w:style>");
    s.push_str("<w:style w:type=\"character\" w:styleId=\"CodigoEmLinha\"><w:name w:val=\"Código em linha\"/><w:rPr><w:rFonts w:ascii=\"Consolas\" w:hAnsi=\"Consolas\" w:cs=\"Consolas\"/><w:noProof/><w:sz w:val=\"20\"/><w:szCs w:val=\"20\"/><w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"F4F4F4\"/></w:rPr></w:style>");
    s.push_str("<w:style w:type=\"paragraph\" w:styleId=\"Citacao\"><w:name w:val=\"Quote\"/><w:basedOn w:val=\"Normal\"/><w:qFormat/><w:pPr><w:ind w:left=\"720\" w:right=\"720\"/></w:pPr><w:rPr><w:i/><w:color w:val=\"555555\"/></w:rPr></w:style>");
    s.push_str("<w:style w:type=\"paragraph\" w:styleId=\"Evidencia\"><w:name w:val=\"Evidência\"/><w:basedOn w:val=\"Normal\"/><w:pPr><w:spacing w:after=\"60\"/><w:ind w:left=\"567\" w:hanging=\"567\"/></w:pPr><w:rPr><w:sz w:val=\"20\"/><w:szCs w:val=\"20\"/></w:rPr></w:style>");
    s.push_str("<w:style w:type=\"paragraph\" w:styleId=\"FootnoteText\"><w:name w:val=\"footnote text\"/><w:basedOn w:val=\"Normal\"/><w:pPr><w:spacing w:after=\"0\" w:line=\"240\" w:lineRule=\"auto\"/></w:pPr><w:rPr><w:sz w:val=\"18\"/><w:szCs w:val=\"18\"/></w:rPr></w:style>");
    s.push_str("<w:style w:type=\"character\" w:styleId=\"FootnoteReference\"><w:name w:val=\"footnote reference\"/><w:rPr><w:vertAlign w:val=\"superscript\"/></w:rPr></w:style>");
    s.push_str("</w:styles>");
    s
}

/// `numbering.xml`: um modelo com marcador e um numerado, e uma instância por
/// lista (as numeradas recomeçam do número do Markdown).
fn numeracao(listas: &[(u32, bool, usize, u64)]) -> String {
    let mut s = format!("{XML}<w:numbering xmlns:w=\"{NS_W}\">");
    let marcadores = ["•", "◦", "▪"];
    for (id, numerada) in [(0, false), (1, true)] {
        s.push_str(&format!(
            "<w:abstractNum w:abstractNumId=\"{id}\"><w:multiLevelType w:val=\"hybridMultilevel\"/>"
        ));
        for nivel in 0..9u32 {
            let (formato, texto) = if numerada {
                ("decimal", format!("%{}.", nivel + 1))
            } else {
                ("bullet", marcadores[nivel as usize % 3].to_owned())
            };
            s.push_str(&format!(
                "<w:lvl w:ilvl=\"{nivel}\"><w:start w:val=\"1\"/><w:numFmt w:val=\"{formato}\"/><w:lvlText w:val=\"{texto}\"/><w:lvlJc w:val=\"left\"/><w:pPr><w:ind w:left=\"{}\" w:hanging=\"{RECUO}\"/></w:pPr></w:lvl>",
                RECUO * 2 * (nivel + 1)
            ));
        }
        s.push_str("</w:abstractNum>");
    }
    for &(num_id, numerada, nivel, inicio) in listas {
        let abstrato = u32::from(numerada);
        s.push_str(&format!(
            "<w:num w:numId=\"{num_id}\"><w:abstractNumId w:val=\"{abstrato}\"/>"
        ));
        if numerada {
            s.push_str(&format!(
                "<w:lvlOverride w:ilvl=\"{nivel}\"><w:startOverride w:val=\"{inicio}\"/></w:lvlOverride>"
            ));
        }
        s.push_str("</w:num>");
    }
    s.push_str("</w:numbering>");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapa_e_tira_caracteres_de_controle() {
        assert_eq!(escapar("a<b>&\"c\u{1}"), "a&lt;b&gt;&amp;&quot;c");
    }

    #[test]
    fn tabulacao_e_quebra_no_run() {
        assert_eq!(
            conteudo_do_run("a\tb\nc"),
            "<w:t xml:space=\"preserve\">a</w:t><w:tab/><w:t xml:space=\"preserve\">b</w:t><w:br/><w:t xml:space=\"preserve\">c</w:t>"
        );
    }
}
