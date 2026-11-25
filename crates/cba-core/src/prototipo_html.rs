//! Protótipo HTML de uma tela (#385).
//!
//! O agente escreve só o corpo da tela; o app confere ([`conferir_corpo`]) e
//! monta o documento completo ([`montar`]), gravado em
//! `analises/<id>/prototipo/TL-<n>.html` ([`cba_kb::layout::html_da_tela`]) e
//! apontado por [`crate::prototipo::Tela::html`].
//!
//! # Estrutura do corpo
//!
//! - Uma `<section data-estado="…">` para cada estado principal
//!   ([`EstadoTela`]): `vazio`, `preenchido`, `erro` (erro de validação) e
//!   `sucesso`, cada um uma vez. O app dá a cada seção o `id` `estado-<estado>`:
//!   abrir o arquivo com `#estado-erro` mostra só aquele estado (CSS `:target`,
//!   sem script); sem fragmento, os quatro aparecem um abaixo do outro.
//! - O elemento rastreado leva `data-elemento="E-n"` (o número na tela). O app
//!   destaca os elementos "a confirmar" e lista no cabeçalho.
//! - Tela alterada: o que muda em relação à tela atual leva a classe `mudou`.
//!
//! # Sem script e sem rede
//!
//! O visualizador abre o HTML isolado, sem rede (#382). O corpo é recusado com
//! `<script>` (ou outra tag que executa ou carrega algo: `iframe`, `object`,
//! `embed`, `link`, `meta`, `base`...), atributo `on…`, URL com esquema
//! (`http:`, `https:`, `javascript:`...) ou que começa com `//`, `..` em `src` e
//! `href`, e `@import` ou `url()` externa no CSS. O documento montado ainda leva
//! uma Content-Security-Policy que não deixa carregar nada de fora.
//!
//! # Visual
//!
//! [`Visual`]: com os tokens aprovados pelo analista (#384), o CSS deles vai
//! inline num `<style>`, em qualquer fidelidade; sem eles, o CSS base usa
//! `var(--prototipo-*, <cinza neutro>)` e a tela sai em wireframe neutro. Nunca
//! se inventa valor de identidade visual. A [`Fidelidade`] muda só o
//! acabamento (`data-fidelidade` no `<body>`): no wireframe, sem sombras nem
//! imagens (blocos cinza no lugar), com a mesma identidade.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use crate::prototipo::{Fidelidade, MudancaTela, Tela};

/// Tamanho máximo do corpo escrito pelo agente.
pub const TAMANHO_MAXIMO_DO_CORPO: usize = 256 * 1024;

/// Classe que marca, numa tela alterada, o que muda em relação à tela atual.
pub const CLASSE_MUDOU: &str = "mudou";

/// Estado principal de uma tela: cada protótipo HTML tem os quatro.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EstadoTela {
    Vazio,
    Preenchido,
    /// Erro de validação.
    Erro,
    Sucesso,
}

impl EstadoTela {
    pub const TODOS: [EstadoTela; 4] = [Self::Vazio, Self::Preenchido, Self::Erro, Self::Sucesso];

    /// Valor de `data-estado` (`erro`).
    pub fn valor(self) -> &'static str {
        match self {
            Self::Vazio => "vazio",
            Self::Preenchido => "preenchido",
            Self::Erro => "erro",
            Self::Sucesso => "sucesso",
        }
    }

    /// Nome para o analista ("erro de validação").
    pub fn rotulo(self) -> &'static str {
        match self {
            Self::Vazio => "vazio",
            Self::Preenchido => "preenchido",
            Self::Erro => "erro de validação",
            Self::Sucesso => "sucesso",
        }
    }

    /// `id` da seção do estado (`estado-erro`), o fragmento que mostra só ele.
    pub fn id(self) -> String {
        format!("estado-{}", self.valor())
    }

    fn de_valor(valor: &str) -> Option<Self> {
        Self::TODOS.into_iter().find(|e| e.valor() == valor)
    }
}

/// Corpo conferido, pronto para [`montar`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorpoConferido {
    /// O corpo com o `id` de cada seção de estado.
    corpo: String,
    /// Elementos da tela marcados com `data-elemento`.
    pub elementos_marcados: BTreeSet<u32>,
    /// Algum elemento tem a classe [`CLASSE_MUDOU`].
    pub tem_mudanca_marcada: bool,
}

impl CorpoConferido {
    pub fn corpo(&self) -> &str {
        &self.corpo
    }
}

/// Visual do protótipo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Visual {
    /// Tokens aprovados pelo analista: o CSS de [`crate::tokens::TokensProjeto::css`].
    Tokens(String),
    /// Sem tokens aprovados: wireframe neutro (cinzas).
    SemTokens,
}

impl Visual {
    /// O visual da tela pelo CSS aprovado (se houver), em qualquer fidelidade.
    pub fn escolher(css_aprovado: Option<String>) -> Self {
        match css_aprovado {
            Some(css) => Self::Tokens(css),
            None => Self::SemTokens,
        }
    }

    /// Valor de `data-visual` no `<body>`.
    pub fn valor(&self) -> &'static str {
        match self {
            Self::Tokens(_) => "tokens",
            Self::SemTokens => "sem-tokens",
        }
    }
}

// ---------------------------------------------------------------------------
// Leitura das tags

/// Tag de abertura (ou de fechamento) achada no HTML.
struct Tag {
    nome: String,
    fechamento: bool,
    /// Atributos com o nome em minúsculas e o valor como escrito.
    atributos: Vec<(String, Option<String>)>,
    /// Posição logo depois do nome da tag (onde o app insere o `id`).
    depois_do_nome: usize,
}

impl Tag {
    fn atributo(&self, nome: &str) -> Option<&str> {
        self.atributos
            .iter()
            .find(|(n, _)| n == nome)
            .map(|(_, v)| v.as_deref().unwrap_or(""))
    }
}

/// Partes do HTML: tags e o conteúdo de `<style>`.
enum Parte {
    Tag(Tag),
    Estilo(String),
}

fn achar(texto: &[u8], desde: usize, padrao: &[u8]) -> Option<usize> {
    texto
        .get(desde..)?
        .windows(padrao.len())
        .position(|w| w.eq_ignore_ascii_case(padrao))
        .map(|p| p + desde)
}

/// Lê as tags do HTML (sem parser completo: o bastante para conferir o que
/// executa ou vai à rede). Os delimitadores são ASCII, então os índices caem
/// sempre em fronteira de caractere.
fn ler_partes(html: &str, problemas: &mut Vec<String>) -> Vec<Parte> {
    let b = html.as_bytes();
    let mut partes = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] != b'<' {
            i += 1;
            continue;
        }
        if b[i..].starts_with(b"<!--") {
            match achar(b, i + 4, b"-->") {
                Some(fim) => i = fim + 3,
                None => {
                    problemas.push("comentário `<!--` sem o `-->` de fechamento".into());
                    break;
                }
            }
            continue;
        }
        let mut j = i + 1;
        let fechamento = b.get(j) == Some(&b'/');
        if fechamento {
            j += 1;
        }
        match b.get(j) {
            Some(c) if c.is_ascii_alphabetic() => {}
            Some(b'!' | b'?') => {
                problemas.push(
                    "declaração `<!…>` ou `<?…>` no corpo: escreva só o corpo da tela".into(),
                );
                i = achar(b, j, b">").map_or(b.len(), |f| f + 1);
                continue;
            }
            _ => {
                i += 1;
                continue;
            }
        }
        let inicio_nome = j;
        while j < b.len() && (b[j].is_ascii_alphanumeric() || matches!(b[j], b'-' | b':' | b'_')) {
            j += 1;
        }
        let nome = html[inicio_nome..j].to_ascii_lowercase();
        let depois_do_nome = j;
        let mut atributos = Vec::new();
        let mut fechou = false;
        while j < b.len() {
            match b[j] {
                b'>' => {
                    fechou = true;
                    j += 1;
                    break;
                }
                c if c.is_ascii_whitespace() || c == b'/' => j += 1,
                _ => {
                    let inicio = j;
                    while j < b.len()
                        && !b[j].is_ascii_whitespace()
                        && !matches!(b[j], b'=' | b'>' | b'/')
                    {
                        j += 1;
                    }
                    let nome_attr = html[inicio..j].to_ascii_lowercase();
                    let mut k = j;
                    while k < b.len() && b[k].is_ascii_whitespace() {
                        k += 1;
                    }
                    let mut valor = None;
                    if b.get(k) == Some(&b'=') {
                        k += 1;
                        while k < b.len() && b[k].is_ascii_whitespace() {
                            k += 1;
                        }
                        match b.get(k) {
                            Some(&q @ (b'"' | b'\'')) => match achar(b, k + 1, &[q]) {
                                Some(f) => {
                                    valor = Some(html[k + 1..f].to_owned());
                                    k = f + 1;
                                }
                                None => {
                                    problemas.push(format!(
                                        "atributo `{nome_attr}` de `<{nome}>` com aspas sem fechamento"
                                    ));
                                    k = b.len();
                                }
                            },
                            _ => {
                                let ini = k;
                                while k < b.len() && !b[k].is_ascii_whitespace() && b[k] != b'>' {
                                    k += 1;
                                }
                                valor = Some(html[ini..k].to_owned());
                            }
                        }
                        j = k;
                    }
                    if !nome_attr.is_empty() {
                        atributos.push((nome_attr, valor));
                    }
                }
            }
        }
        if !fechou {
            problemas.push(format!("tag `<{nome}` sem o `>` de fechamento"));
            break;
        }
        let estilo = !fechamento && nome == "style";
        partes.push(Parte::Tag(Tag {
            nome,
            fechamento,
            atributos,
            depois_do_nome,
        }));
        i = j;
        if estilo {
            let fim = achar(b, i, b"</style").unwrap_or(b.len());
            partes.push(Parte::Estilo(html[i..fim].to_owned()));
            i = fim;
        }
    }
    partes
}

// ---------------------------------------------------------------------------
// Conferência

/// Tags que executam, carregam algo ou mexem no documento montado pelo app.
const TAGS_PROIBIDAS: &[&str] = &[
    "script", "iframe", "frame", "frameset", "object", "embed", "applet", "base", "meta", "link",
    "html", "head", "body", "noscript", "portal",
];

/// Atributos cujo valor é URL.
const ATRIBUTOS_DE_URL: &[&str] = &[
    "href",
    "src",
    "action",
    "formaction",
    "poster",
    "data",
    "background",
    "xlink:href",
    "cite",
    "longdesc",
    "ping",
    "manifest",
    "lowsrc",
    "dynsrc",
    "srcset",
    "imagesrcset",
];

/// Por que a URL não pode ficar no protótipo, ou `None` se pode: só âncoras e
/// caminhos relativos dentro da pasta do protótipo (outra tela, `TL-2.html`) e
/// imagens embutidas em `data:image/` (não SVG).
fn url_proibida(url: &str) -> Option<&'static str> {
    // O navegador ignora espaços e controles dentro do esquema (`java\tscript:`).
    let limpa: String = url
        .chars()
        .filter(|c| !c.is_whitespace() && !c.is_control())
        .collect::<String>()
        .to_ascii_lowercase();
    if limpa.starts_with("//") || limpa.starts_with("\\\\") || limpa.starts_with("/\\") {
        return Some("aponta para outro servidor");
    }
    let antes = limpa.split(['/', '?', '#']).next().unwrap_or("");
    if antes.contains(':') {
        let imagem_embutida =
            limpa.starts_with("data:image/") && !limpa.starts_with("data:image/svg");
        return (!imagem_embutida).then_some("tem esquema (http:, https:, javascript:…)");
    }
    if limpa.starts_with('/') {
        return Some("é caminho absoluto");
    }
    if limpa.split(['/', '\\']).any(|p| p == "..") {
        return Some("sai da pasta do protótipo (`..`)");
    }
    None
}

/// Problemas de um trecho de CSS (`<style>` ou atributo `style`).
fn conferir_css(css: &str, onde: &str, problemas: &mut Vec<String>) {
    let minusculo = css.to_ascii_lowercase();
    if minusculo.contains("@import") {
        problemas.push(format!(
            "{onde}: `@import` não é permitido (o protótipo abre sem rede)"
        ));
    }
    if minusculo.contains("expression(") || minusculo.contains("javascript:") {
        problemas.push(format!("{onde}: CSS que executa código não é permitido"));
    }
    let mut desde = 0;
    while let Some(p) = minusculo[desde..].find("url(") {
        let inicio = desde + p + 4;
        let fim = minusculo[inicio..]
            .find(')')
            .map_or(minusculo.len(), |f| inicio + f);
        let url = css[inicio..fim].trim().trim_matches(['"', '\'']);
        if let Some(motivo) = url_proibida(url) {
            problemas.push(format!("{onde}: `url({url})` {motivo}"));
        }
        desde = fim;
    }
    for esquema in ["http:", "https:"] {
        if minusculo.contains(esquema) {
            problemas.push(format!(
                "{onde}: endereço `{esquema}` não é permitido (o protótipo abre sem rede)"
            ));
        }
    }
}

/// Número do elemento em `data-elemento` (`E-3` ou `3`).
fn numero_do_elemento(valor: &str) -> Option<u32> {
    let v = valor.trim();
    v.strip_prefix("E-")
        .unwrap_or(v)
        .parse::<u32>()
        .ok()
        .filter(|n| *n > 0)
}

/// Confere o corpo escrito pelo agente para a tela e devolve o corpo pronto
/// para [`montar`] ou a lista de problemas (todos de uma vez, para o agente
/// corrigir numa chamada só).
pub fn conferir_corpo(corpo: &str, tela: &Tela) -> Result<CorpoConferido, Vec<String>> {
    let mut problemas = Vec::new();
    if corpo.trim().is_empty() {
        return Err(vec!["o corpo da tela está vazio".into()]);
    }
    if corpo.len() > TAMANHO_MAXIMO_DO_CORPO {
        return Err(vec![format!(
            "o corpo passa de {} KiB; deixe só a tela, sem imagens grandes embutidas",
            TAMANHO_MAXIMO_DO_CORPO / 1024
        )]);
    }
    let partes = ler_partes(corpo, &mut problemas);
    let mut secoes: Vec<(EstadoTela, usize)> = Vec::new();
    let mut elementos_marcados = BTreeSet::new();
    let mut tem_mudanca_marcada = false;
    for parte in &partes {
        let tag = match parte {
            Parte::Estilo(css) => {
                conferir_css(css, "`<style>`", &mut problemas);
                continue;
            }
            Parte::Tag(t) => t,
        };
        if TAGS_PROIBIDAS.contains(&tag.nome.as_str()) {
            if !tag.fechamento {
                problemas.push(format!(
                    "`<{}>` não é permitido: escreva só o corpo da tela, sem script nem nada que \
                     carregue de fora",
                    tag.nome
                ));
            }
            continue;
        }
        if tag.fechamento {
            continue;
        }
        for (nome, valor) in &tag.atributos {
            let valor = valor.as_deref().unwrap_or("");
            if nome.starts_with("on") {
                problemas.push(format!(
                    "atributo `{nome}` em `<{}>`: o protótipo não tem script",
                    tag.nome
                ));
            } else if nome == "style" {
                conferir_css(
                    valor,
                    &format!("`style` de `<{}>`", tag.nome),
                    &mut problemas,
                );
            } else if ATRIBUTOS_DE_URL.contains(&nome.as_str()) {
                let urls: Vec<&str> = if nome.ends_with("srcset") {
                    valor
                        .split(',')
                        .filter_map(|c| c.split_whitespace().next())
                        .collect()
                } else {
                    vec![valor]
                };
                for url in urls {
                    if let Some(motivo) = url_proibida(url) {
                        problemas.push(format!(
                            "`{nome}=\"{url}\"` em `<{}>` {motivo}; o protótipo abre sem rede \
                             (use `#…` ou outra tela, como `TL-2.html`)",
                            tag.nome
                        ));
                    }
                }
            } else if nome == "data-elemento" {
                match numero_do_elemento(valor) {
                    Some(n) if tela.elemento(n).is_some() => {
                        elementos_marcados.insert(n);
                    }
                    _ => problemas.push(format!(
                        "`data-elemento=\"{valor}\"`: a tela {} não tem esse elemento; use E-n \
                         dos elementos registrados ({})",
                        tela.rotulo(),
                        rotulos_dos_elementos(tela)
                    )),
                }
            } else if nome == "class" && valor.split_whitespace().any(|c| c == CLASSE_MUDOU) {
                tem_mudanca_marcada = true;
            }
        }
        if let Some(valor) = tag.atributo("data-estado") {
            if tag.nome != "section" {
                problemas.push(format!(
                    "`data-estado` fica numa `<section>`, não em `<{}>`",
                    tag.nome
                ));
                continue;
            }
            match EstadoTela::de_valor(valor.trim()) {
                Some(e) if secoes.iter().any(|(x, _)| *x == e) => problemas.push(format!(
                    "o estado `{}` aparece mais de uma vez; deixe uma seção por estado",
                    e.valor()
                )),
                Some(e) => {
                    if tag.atributo("id").is_some() {
                        problemas.push(format!(
                            "a seção do estado `{}` tem `id`: tire, o app põe `id=\"{}\"`",
                            e.valor(),
                            e.id()
                        ));
                    }
                    secoes.push((e, tag.depois_do_nome));
                }
                None => problemas.push(format!(
                    "`data-estado=\"{valor}\"` não é um estado: use {}",
                    nomes_dos_estados()
                )),
            }
        }
    }
    let faltam: Vec<&str> = EstadoTela::TODOS
        .into_iter()
        .filter(|e| !secoes.iter().any(|(x, _)| x == e))
        .map(EstadoTela::valor)
        .collect();
    if !faltam.is_empty() {
        problemas.push(format!(
            "falta a seção do(s) estado(s) {}: cada tela tem `<section data-estado=\"…\">` para \
             {}",
            faltam.join(", "),
            nomes_dos_estados()
        ));
    }
    if !problemas.is_empty() {
        return Err(problemas);
    }
    let mut corpo = corpo.to_owned();
    secoes.sort_by_key(|(_, pos)| std::cmp::Reverse(*pos));
    for (estado, pos) in secoes {
        corpo.insert_str(pos, &format!(" id=\"{}\"", estado.id()));
    }
    Ok(CorpoConferido {
        corpo,
        elementos_marcados,
        tem_mudanca_marcada,
    })
}

fn nomes_dos_estados() -> String {
    EstadoTela::TODOS
        .iter()
        .map(|e| format!("`{}`", e.valor()))
        .collect::<Vec<_>>()
        .join(", ")
}

fn rotulos_dos_elementos(tela: &Tela) -> String {
    if tela.elementos.is_empty() {
        return "nenhum".into();
    }
    tela.elementos
        .iter()
        .map(|e| e.rotulo())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Elementos marcados com `data-elemento` num HTML já gravado (a conferência
/// para concluir usa para achar elemento registrado depois do HTML).
pub fn elementos_marcados(html: &str) -> BTreeSet<u32> {
    let mut ignorados = Vec::new();
    ler_partes(html, &mut ignorados)
        .iter()
        .filter_map(|p| match p {
            Parte::Tag(t) if !t.fechamento => t.atributo("data-elemento"),
            _ => None,
        })
        .filter_map(numero_do_elemento)
        .collect()
}

/// Estados presentes num HTML já gravado (`<section data-estado>`).
pub fn estados_presentes(html: &str) -> BTreeSet<EstadoTela> {
    let mut ignorados = Vec::new();
    ler_partes(html, &mut ignorados)
        .iter()
        .filter_map(|p| match p {
            Parte::Tag(t) if !t.fechamento && t.nome == "section" => t.atributo("data-estado"),
            _ => None,
        })
        .filter_map(|v| EstadoTela::de_valor(v.trim()))
        .collect()
}

// ---------------------------------------------------------------------------
// Montagem

/// Texto seguro dentro do HTML.
pub fn escapar(texto: &str) -> String {
    let mut s = String::with_capacity(texto.len());
    for c in texto.chars() {
        match c {
            '&' => s.push_str("&amp;"),
            '<' => s.push_str("&lt;"),
            '>' => s.push_str("&gt;"),
            '"' => s.push_str("&quot;"),
            '\'' => s.push_str("&#39;"),
            _ => s.push(c),
        }
    }
    s
}

/// O CSS dos tokens para o `<style>`: sem `url()` nem `@import` (o protótipo
/// abre sem rede) e sem nada que feche a tag `<style>`.
fn css_dos_tokens(css: &str) -> String {
    let mut saida = String::new();
    for linha in css.lines() {
        let m = linha.to_ascii_lowercase();
        if m.contains("url(") || m.contains("@import") || m.contains('<') {
            saida.push_str(
                "  /* linha omitida: recurso externo ou `<` não entram no protótipo */\n",
            );
        } else {
            saida.push_str(linha);
            saida.push('\n');
        }
    }
    saida
}

/// Política do documento: nada vem de fora; só estilo inline e imagens embutidas.
pub const POLITICA_DE_CONTEUDO: &str = "default-src 'none'; style-src 'unsafe-inline'; \
     img-src data:; font-src data:; form-action 'none'; base-uri 'none'";

/// CSS base, neutro de wireframe: cada papel cai num cinza quando não há tokens
/// aprovados ou o papel está a confirmar.
pub const CSS_BASE: &str = r#"*, *::before, *::after { box-sizing: border-box; }
body {
  margin: 0;
  font-family: var(--prototipo-fonte-familia, system-ui, -apple-system, "Segoe UI", sans-serif);
  font-size: var(--prototipo-fonte-tamanho-base, 14px);
  line-height: 1.45;
  color: var(--prototipo-cor-texto, #2b2b2b);
  background: var(--prototipo-cor-fundo, #ffffff);
}
.prototipo-cabecalho {
  font: 13px/1.45 system-ui, -apple-system, "Segoe UI", sans-serif;
  color: #333333;
  background: #f2f2f2;
  border-bottom: 1px dashed #9a9a9a;
  padding: 12px 24px;
}
.prototipo-cabecalho h1 { font-size: 16px; margin: 2px 0 6px; }
.prototipo-cabecalho p { margin: 2px 0; }
.prototipo-cabecalho a { color: #333333; }
.prototipo-rotulo { text-transform: uppercase; letter-spacing: .04em; font-size: 11px; color: #666666; }
.prototipo-aviso { color: #5a5a5a; font-style: italic; }
.prototipo-tela {
  display: grid;
  gap: var(--prototipo-espaco-grande, 24px);
  padding: var(--prototipo-espaco-grande, 24px);
}
.prototipo-tela:has(section[data-estado]:target) section[data-estado]:not(:target) { display: none; }
section[data-estado] {
  border: 1px solid var(--prototipo-cor-borda, #c4c4c4);
  border-radius: var(--prototipo-raio-borda, 4px);
  padding: var(--prototipo-espaco-medio, 16px);
}
section[data-estado]::before {
  display: block;
  margin-bottom: var(--prototipo-espaco-medio, 16px);
  font: 600 11px/1.2 system-ui, -apple-system, "Segoe UI", sans-serif;
  letter-spacing: .04em;
  text-transform: uppercase;
  color: #6b6b6b;
}
section[data-estado="vazio"]::before { content: "Estado: vazio"; }
section[data-estado="preenchido"]::before { content: "Estado: preenchido"; }
section[data-estado="erro"]::before { content: "Estado: erro de validação"; }
section[data-estado="sucesso"]::before { content: "Estado: sucesso"; }
h1, h2, h3 { font-size: var(--prototipo-fonte-tamanho-titulo, 20px); margin: 0 0 var(--prototipo-espaco-medio, 16px); }
h3 { font-size: var(--prototipo-fonte-tamanho-base, 14px); }
p { margin: 0 0 var(--prototipo-espaco-pequeno, 8px); }
form, fieldset { display: grid; gap: var(--prototipo-espaco-medio, 16px); }
fieldset { border: 1px solid var(--prototipo-cor-borda, #c4c4c4); border-radius: var(--prototipo-raio-borda, 4px); padding: var(--prototipo-espaco-medio, 16px); }
label { display: block; margin-bottom: var(--prototipo-espaco-pequeno, 8px); font-weight: 600; }
input, select, textarea {
  font: inherit;
  color: inherit;
  min-width: 14em;
  padding: 6px 8px;
  background: #ffffff;
  border: 1px solid var(--prototipo-cor-borda, #9a9a9a);
  border-radius: var(--prototipo-raio-borda, 4px);
}
input::placeholder, textarea::placeholder { color: #9a9a9a; }
button, .botao {
  font: inherit;
  cursor: default;
  padding: 6px 16px;
  color: #ffffff;
  background: var(--prototipo-cor-primaria, #5c5c5c);
  border: 1px solid var(--prototipo-cor-primaria, #5c5c5c);
  border-radius: var(--prototipo-raio-borda, 4px);
}
button.secundario, .botao.secundario { color: var(--prototipo-cor-primaria, #5c5c5c); background: transparent; }
a { color: var(--prototipo-cor-primaria, #4a4a4a); }
table { border-collapse: collapse; width: 100%; }
th, td { text-align: left; padding: 6px 8px; border: 1px solid var(--prototipo-cor-borda, #c4c4c4); }
.acoes { display: flex; gap: var(--prototipo-espaco-pequeno, 8px); flex-wrap: wrap; }
.mensagem, .erro, .alerta, .sucesso {
  padding: var(--prototipo-espaco-pequeno, 8px) var(--prototipo-espaco-medio, 16px);
  border-left: 4px solid var(--prototipo-cor-borda, #9a9a9a);
  border-radius: var(--prototipo-raio-borda, 4px);
  background: #f5f5f5;
}
.erro { border-left-color: var(--prototipo-cor-erro, #3d3d3d); color: var(--prototipo-cor-erro, inherit); border-left-style: double; border-left-width: 6px; }
.alerta { border-left-color: var(--prototipo-cor-alerta, #7a7a7a); color: var(--prototipo-cor-alerta, inherit); border-left-style: dashed; }
.sucesso { border-left-color: var(--prototipo-cor-sucesso, #5c5c5c); color: var(--prototipo-cor-sucesso, inherit); }
.campo-erro, [aria-invalid="true"] { border: 2px solid var(--prototipo-cor-erro, #3d3d3d); }
.mudou { box-shadow: 0 0 0 3px rgba(255, 196, 0, .65); border-radius: 2px; }
.a-confirmar { outline: 2px dashed #8a8a8a; outline-offset: 2px; }
body[data-fidelidade="wireframe"] .prototipo-tela *:not(.mudou) { box-shadow: none; text-shadow: none; }
body[data-fidelidade="wireframe"] .prototipo-tela * { background-image: none; transition: none; animation: none; }
body[data-fidelidade="wireframe"] .prototipo-tela img,
body[data-fidelidade="wireframe"] .prototipo-tela svg,
body[data-fidelidade="wireframe"] .prototipo-tela picture,
body[data-fidelidade="wireframe"] .prototipo-tela video {
  display: inline-block;
  min-width: 48px;
  min-height: 32px;
  object-position: -9999px -9999px;
  background: #e3e3e3;
  border: 1px dashed #9a9a9a;
}
"#;

/// CSS da tela: o destaque dos elementos a confirmar.
fn css_da_tela(tela: &Tela) -> String {
    let a_confirmar: Vec<String> = tela
        .elementos_a_confirmar()
        .map(|e| format!("[data-elemento=\"{}\"]", e.rotulo()))
        .collect();
    if a_confirmar.is_empty() {
        return String::new();
    }
    let seletores = a_confirmar.join(",\n");
    format!(
        "{seletores} {{ outline: 2px dashed #8a8a8a; outline-offset: 2px; }}\n\
         {} {{ content: \" (a confirmar)\"; font: italic 11px system-ui, sans-serif; color: #6b6b6b; }}\n",
        a_confirmar
            .iter()
            .map(|s| format!("{s}:not(input):not(select):not(textarea):not(img)::after"))
            .collect::<Vec<_>>()
            .join(",\n")
    )
}

/// Valor de `data-fidelidade` no `<body>`.
fn valor_da_fidelidade(fidelidade: Fidelidade) -> &'static str {
    match fidelidade {
        Fidelidade::Wireframe => "wireframe",
        Fidelidade::AltaFidelidade => "alta-fidelidade",
    }
}

/// O documento completo da tela: cabeçalho com a tela, o motivo, os estados e
/// o que está a confirmar; os tokens (ou o aviso do wireframe neutro), o CSS
/// base e o corpo conferido. A `fidelidade` muda só o acabamento.
pub fn montar(
    tela: &Tela,
    corpo: &CorpoConferido,
    visual: &Visual,
    fidelidade: Fidelidade,
) -> String {
    let mut h = String::new();
    let titulo = format!("{} · {}", tela.rotulo(), tela.nome);
    let _ = write!(
        h,
        "<!doctype html>\n<html lang=\"pt-BR\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <meta http-equiv=\"Content-Security-Policy\" content=\"{POLITICA_DE_CONTEUDO}\">\n\
         <meta name=\"generator\" content=\"Ctower Business Agents\">\n\
         <title>{}</title>\n<style id=\"prototipo-tokens\">\n",
        escapar(&titulo)
    );
    match visual {
        Visual::Tokens(css) => h.push_str(&css_dos_tokens(css)),
        Visual::SemTokens => {
            h.push_str("/* Sem tokens CSS aprovados pelo analista: wireframe neutro. */\n")
        }
    }
    h.push_str("</style>\n<style id=\"prototipo-base\">\n");
    h.push_str(CSS_BASE);
    h.push_str("</style>\n");
    let css_tela = css_da_tela(tela);
    if !css_tela.is_empty() {
        h.push_str("<style id=\"prototipo-tela\">\n");
        h.push_str(&css_tela);
        h.push_str("</style>\n");
    }
    let _ = write!(
        h,
        "</head>\n<body data-tela=\"{}\" data-mudanca=\"{}\" data-visual=\"{}\" \
         data-fidelidade=\"{}\">\n\
         <header class=\"prototipo-cabecalho\">\n\
         <p class=\"prototipo-rotulo\">Protótipo · {} · tela {} · {}</p>\n<h1>{}</h1>\n",
        tela.rotulo(),
        tela.mudanca.rotulo(),
        visual.valor(),
        valor_da_fidelidade(fidelidade),
        tela.rotulo(),
        tela.mudanca.rotulo(),
        fidelidade.rotulo(),
        escapar(&tela.nome)
    );
    let mut motivo = format!("<strong>Motivo:</strong> {}", escapar(&tela.motivo));
    let rotulos = tela.rastro.rotulos();
    if !rotulos.is_empty() {
        let _ = write!(motivo, " · Origem: {}", escapar(&rotulos.join(", ")));
    }
    if tela.a_confirmar() {
        motivo.push_str(" · <em>a confirmar</em>");
    }
    let _ = writeln!(h, "<p>{motivo}</p>");
    if tela.mudanca == MudancaTela::Alterada {
        let _ = writeln!(
            h,
            "<p>Tela alterada: o que muda em relação à tela atual está destacado.</p>"
        );
    }
    let a_confirmar: Vec<String> = tela
        .elementos_a_confirmar()
        .map(|e| format!("{} {}", e.rotulo(), e.nome))
        .collect();
    if !a_confirmar.is_empty() {
        let _ = writeln!(
            h,
            "<p><strong>A confirmar:</strong> {}</p>",
            escapar(&a_confirmar.join("; "))
        );
    }
    match visual {
        Visual::Tokens(_) => {}
        Visual::SemTokens => {
            let _ = writeln!(
                h,
                "<p class=\"prototipo-aviso\">Wireframe neutro: os tokens CSS do projeto \
                 ainda não foram aprovados pelo analista.</p>"
            );
        }
    }
    let estados: Vec<String> = EstadoTela::TODOS
        .iter()
        .map(|e| format!("<a href=\"#{}\">{}</a>", e.id(), e.rotulo()))
        .collect();
    let _ = write!(
        h,
        "<nav class=\"prototipo-estados\">Estados: {} · <a href=\"#\">todos</a></nav>\n\
         </header>\n<main class=\"prototipo-tela\">\n",
        estados.join(" · ")
    );
    h.push_str(corpo.corpo());
    if !corpo.corpo().ends_with('\n') {
        h.push('\n');
    }
    h.push_str("</main>\n</body>\n</html>\n");
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prototipo::{ElementoTela, Rastro, TipoElemento};
    use crate::requisito::Fundamento;
    use cba_kb::Id;

    fn regra() -> Rastro {
        Rastro {
            fundamentos: vec![Fundamento::Regra {
                id: Id::analisar("RN-FAT-008").unwrap(),
                titulo: "Parcelas entre 2 e 3".into(),
                evidencias: Vec::new(),
            }],
            ..Rastro::default()
        }
    }

    fn elemento(numero: u32, tipo: TipoElemento, nome: &str, rastro: Rastro) -> ElementoTela {
        ElementoTela {
            numero,
            tipo,
            nome: nome.into(),
            descricao: None,
            rastro,
            sessao: None,
            registrado_ms: 1,
        }
    }

    fn tela() -> Tela {
        Tela {
            numero: 1,
            nome: "Parcelar fatura <PF>".into(),
            mudanca: MudancaTela::Alterada,
            motivo: "O parcelamento passa a aceitar pessoa física".into(),
            rastro: regra(),
            elementos: vec![
                elemento(1, TipoElemento::Campo, "Número de parcelas", regra()),
                elemento(
                    2,
                    TipoElemento::Mensagem,
                    "Parcela abaixo do mínimo",
                    Rastro::default(),
                ),
            ],
            html: None,
            sessao: None,
            registrada_ms: 1,
        }
    }

    const CORPO: &str = r##"<section data-estado="vazio">
  <label for="p">Número de parcelas</label>
  <input id="p" data-elemento="E-1" class="mudou" placeholder="2 a 3">
</section>
<section data-estado='preenchido'><input data-elemento="E-1" value="3"></section>
<!-- <script>comentário não conta</script> -->
<section data-estado=erro><p class="erro" data-elemento="E-2">A parcela mínima é de R$ 100,00.</p></section>
<section data-estado="sucesso"><p class="sucesso">Parcelamento registrado. <a href="TL-2.html">Ver parcelas</a> <a href="#estado-vazio">voltar</a></p></section>
"##;

    #[test]
    fn corpo_com_os_quatro_estados() {
        let c = conferir_corpo(CORPO, &tela()).unwrap();
        assert_eq!(c.elementos_marcados, BTreeSet::from([1, 2]));
        assert!(c.tem_mudanca_marcada);
        assert!(
            c.corpo()
                .contains("<section id=\"estado-vazio\" data-estado=\"vazio\">")
        );
        assert!(
            c.corpo()
                .contains("<section id=\"estado-erro\" data-estado=erro>")
        );
        assert_eq!(elementos_marcados(c.corpo()), BTreeSet::from([1, 2]));
        assert_eq!(
            estados_presentes(c.corpo()),
            BTreeSet::from(EstadoTela::TODOS)
        );
    }

    #[test]
    fn corpo_recusado_com_todos_os_problemas() {
        let ruim = r#"<section data-estado="vazio" onclick="x()">
<script>alert(1)</script>
<img src="https://exemplo.invalid/logo.png">
<img src=" java	script:alert(1)">
<a href="//exemplo.invalid">x</a>
<a href="../../regras/RN-FAT-008.md">y</a>
<div style="background: url('https://exemplo.invalid/f.png')">z</div>
<style>@import "x.css"; .a { background: url(data:image/png;base64,AAAA) }</style>
<p data-elemento="E-9">?</p>
<iframe src="TL-2.html"></iframe>
</section>
<section data-estado="vazio"></section>
<section data-estado="carregando"></section>"#;
        let p = conferir_corpo(ruim, &tela()).unwrap_err().join("\n");
        for esperado in [
            "`onclick`",
            "`<script>`",
            "`src=\"https://exemplo.invalid/logo.png\"`",
            "javascript",
            "`href=\"//exemplo.invalid\"` em `<a>` aponta para outro servidor",
            "`..`",
            "`url(https://exemplo.invalid/f.png)`",
            "`@import`",
            "`data-elemento=\"E-9\"`",
            "`<iframe>`",
            "`vazio` aparece mais de uma vez",
            "`data-estado=\"carregando\"`",
            "falta a seção do(s) estado(s) preenchido, erro, sucesso",
        ] {
            assert!(p.contains(esperado), "{esperado}:\n{p}");
        }
        // Imagem embutida é aceita; SVG embutido não.
        assert!(!p.contains("data:image/png"), "{p}");
        let svg = CORPO.replace(
            "voltar</a>",
            "voltar</a><img src=\"data:image/svg+xml;base64,AAAA\">",
        );
        assert!(conferir_corpo(&svg, &tela()).is_err());
        let com_id = CORPO.replace(
            "<section data-estado=\"sucesso\">",
            "<section id=\"x\" data-estado=\"sucesso\">",
        );
        let p = conferir_corpo(&com_id, &tela()).unwrap_err().join("\n");
        assert!(p.contains("o app põe `id=\"estado-sucesso\"`"), "{p}");
        assert!(conferir_corpo("  ", &tela()).is_err());
        assert!(conferir_corpo("<!doctype html><html><body></body></html>", &tela()).is_err());
    }

    #[test]
    fn documento_com_tokens_e_neutro() {
        let t = tela();
        let c = conferir_corpo(CORPO, &t).unwrap();
        let css = ":root {\n  --prototipo-cor-primaria: #0b5394; /* src/tema.css:3 */\n  \
                   /* --prototipo-cor-erro: a confirmar (erro) */\n  \
                   --fonte: url(https://exemplo.invalid/f.woff);\n}\n";
        let html = montar(
            &t,
            &c,
            &Visual::Tokens(css.into()),
            Fidelidade::AltaFidelidade,
        );
        assert!(html.starts_with("<!doctype html>"));
        assert!(html.contains("<title>TL-1 · Parcelar fatura &lt;PF&gt;</title>"));
        assert!(html.contains("--prototipo-cor-primaria: #0b5394;"));
        assert!(!html.contains("exemplo.invalid"), "{html}");
        assert!(html.contains("var(--prototipo-cor-erro, #3d3d3d)"));
        assert!(html.contains("Content-Security-Policy"));
        assert!(html.contains("data-visual=\"tokens\""));
        assert!(html.contains("<strong>A confirmar:</strong> E-2 Parcela abaixo do mínimo"));
        assert!(html.contains("[data-elemento=\"E-2\"] { outline"));
        assert!(!html.contains("[data-elemento=\"E-1\"] {"));
        assert!(html.contains("<a href=\"#estado-erro\">erro de validação</a>"));
        assert!(html.contains("Origem: RN-FAT-008"));
        assert!(html.ends_with("</main>\n</body>\n</html>\n"));

        assert!(html.contains("data-fidelidade=\"alta-fidelidade\""));
        let neutro = montar(&t, &c, &Visual::escolher(None), Fidelidade::AltaFidelidade);
        assert!(neutro.contains("data-visual=\"sem-tokens\""));
        assert!(neutro.contains("os tokens CSS do projeto ainda não foram aprovados"));
        assert!(!neutro.contains("--prototipo-cor-primaria:"));
        // Wireframe com tokens aprovados: a mesma identidade, outro acabamento.
        let wire = montar(
            &t,
            &c,
            &Visual::escolher(Some(css.into())),
            Fidelidade::Wireframe,
        );
        assert!(wire.contains("data-visual=\"tokens\""));
        assert!(wire.contains("data-fidelidade=\"wireframe\""));
        assert!(wire.contains("--prototipo-cor-primaria: #0b5394;"));
        assert!(wire.contains("Protótipo · TL-1 · tela alterada · wireframe"));
    }

    #[test]
    fn urls() {
        for ok in [
            "#estado-erro",
            "TL-2.html",
            "TL-2.html#estado-vazio",
            "",
            "data:image/png;base64,AA",
        ] {
            assert_eq!(url_proibida(ok), None, "{ok}");
        }
        for ruim in [
            "http://x",
            "HTTPS://x",
            "//x",
            "\\\\x",
            "javascript:x",
            "mailto:a@b",
            "/etc/passwd",
            "a/../../b",
            "data:text/html,x",
        ] {
            assert!(url_proibida(ruim).is_some(), "{ruim}");
        }
    }
}
