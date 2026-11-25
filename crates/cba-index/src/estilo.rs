//! Tokens de estilo do código da interface do projeto analisado (#384): cores,
//! tipografia, espaçamento e raio, para o protótipo de telas seguir a
//! identidade visual do próprio projeto, não a do Control Tower.
//!
//! A extração é determinística (sem IA) e lê só o que está escrito no código:
//!
//! - CSS, SCSS, Sass e LESS: propriedades customizadas (`--nome: valor;`),
//!   variáveis do SCSS e do Sass (`$nome`) e do LESS (`@nome`), e as
//!   declarações de cor, fonte e raio das regras `:root`, `html` e `body`
//!   (projeto sem variável nenhuma ainda tem a fonte e as cores da página).
//! - `tailwind.config.*`: as seções de `theme` e `theme.extend` (`colors`,
//!   `fontFamily`, `fontSize`, `fontWeight`, `lineHeight`, `spacing`,
//!   `borderRadius`), com valores literais.
//! - Tema em JavaScript ou TypeScript (arquivo com `tema`, `theme` ou `tokens`
//!   no nome): os pares chave e valor literais do objeto.
//!
//! Valor que o pré-processador calcula (`lighten($cor, 10%)`, interpolação) ou
//! número sem unidade numa medida não é resolvido nem completado: o token sai
//! sem valor, com a nota, para o analista confirmar. O nome de cada token vira
//! uma propriedade customizada do CSS (`$cor-alerta` → `--cor-alerta`,
//! `theme.colors.primary` → `--cor-primary`).
//!
//! Variável que não é de cor, tipografia, espaçamento ou raio (sombra,
//! `z-index`, duração de animação) fica de fora.

use std::fs;
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::inventario::relativo;
use crate::projeto::Escopo;

/// Tamanho máximo de um arquivo lido na extração. Folha de estilo maior que
/// isso é gerada (build), não escrita à mão.
pub const LIMITE_ARQUIVO_ESTILO: u64 = 512 * 1024;

/// Categoria de um token de estilo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CategoriaToken {
    Cor,
    Tipografia,
    Espacamento,
    Raio,
}

impl CategoriaToken {
    pub const TODAS: [Self; 4] = [Self::Cor, Self::Tipografia, Self::Espacamento, Self::Raio];

    /// Nome para quem lê.
    pub fn nome(self) -> &'static str {
        match self {
            Self::Cor => "cores",
            Self::Tipografia => "tipografia",
            Self::Espacamento => "espaçamento",
            Self::Raio => "raio",
        }
    }
}

/// Onde o token está no código do projeto.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrigemToken {
    /// Caminho relativo à raiz do projeto, com `/`.
    pub arquivo: String,
    /// Linha (a partir de 1).
    pub linha: u32,
}

impl std::fmt::Display for OrigemToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.arquivo, self.linha)
    }
}

/// Token de estilo achado no código.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenEstilo {
    /// Nome como propriedade customizada do CSS (`--cor-primaria`).
    pub nome: String,
    /// Nome como está no código (`$cor-alerta`, `theme.colors.primary`).
    pub original: String,
    pub categoria: CategoriaToken,
    /// Valor pronto para o CSS. Sem valor: o código não tem um valor literal
    /// (calculado pelo pré-processador, número sem unidade); fica a confirmar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valor: Option<String>,
    pub origem: OrigemToken,
    /// Por que o valor ficou a confirmar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nota: Option<String>,
}

/// Tipo de arquivo que a extração lê.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TipoArquivo {
    /// CSS, SCSS ou LESS (declarações com `;`).
    Folha,
    /// Sass com a sintaxe indentada (sem `;` nem chaves).
    SassIndentado,
    Tailwind,
    TemaScript,
}

fn tipo_do_arquivo(arquivo: &str) -> Option<TipoArquivo> {
    let nome = arquivo.rsplit('/').next().unwrap_or(arquivo).to_lowercase();
    let (base, ext) = nome.rsplit_once('.')?;
    let script = matches!(
        ext,
        "js" | "cjs" | "mjs" | "ts" | "cts" | "mts" | "jsx" | "tsx"
    );
    match ext {
        "css" | "scss" | "less" => Some(TipoArquivo::Folha),
        "sass" => Some(TipoArquivo::SassIndentado),
        _ if script && base.starts_with("tailwind.config") => Some(TipoArquivo::Tailwind),
        _ if script
            && palavras(arquivo.rsplit('/').next().unwrap_or(arquivo))
                .iter()
                .any(|p| {
                    matches!(p.as_str(), "tema" | "temas" | "theme" | "themes" | "tokens")
                }) =>
        {
            Some(TipoArquivo::TemaScript)
        }
        _ => None,
    }
}

/// Tokens de estilo do código do projeto: percorre as pastas do escopo (fora das
/// exclusões, sem seguir links), na ordem das pastas e dos nomes. Arquivo
/// ilegível, que não é UTF-8 ou maior que [`LIMITE_ARQUIVO_ESTILO`] fica de fora.
pub fn extrair_estilo(escopo: &Escopo) -> Vec<TokenEstilo> {
    let exclusoes = escopo.exclusoes();
    let mut tokens = Vec::new();
    for pasta in escopo.pastas() {
        let raiz = pasta.caminho.as_path();
        let prefixo = pasta.prefixo.as_deref();
        let rel = |p: &Path| relativo(raiz, prefixo, p);
        let it = WalkDir::new(raiz)
            .follow_links(false)
            .sort_by_file_name()
            .min_depth(1)
            .into_iter()
            .filter_entry(|e| {
                let Some(r) = rel(e.path()) else {
                    return false;
                };
                !e.path_is_symlink() && exclusoes.avaliar(&r, e.file_type().is_dir()).is_none()
            });
        for entrada in it.flatten() {
            if !entrada.file_type().is_file() {
                continue;
            }
            let Some(arquivo) = rel(entrada.path()) else {
                continue;
            };
            if tipo_do_arquivo(&arquivo).is_none()
                || entrada
                    .metadata()
                    .map_or(true, |m| m.len() > LIMITE_ARQUIVO_ESTILO)
            {
                continue;
            }
            let Ok(texto) = fs::read_to_string(entrada.path()) else {
                continue;
            };
            tokens.extend(extrair_do_texto(&arquivo, &texto));
        }
    }
    tokens
}

/// Tokens de estilo de um arquivo, pelo nome (`arquivo`, relativo à raiz do
/// projeto) e pelo conteúdo. Arquivo que não é de estilo nem de tema devolve
/// lista vazia.
pub fn extrair_do_texto(arquivo: &str, texto: &str) -> Vec<TokenEstilo> {
    match tipo_do_arquivo(arquivo) {
        Some(TipoArquivo::Folha) => folha(arquivo, texto),
        Some(TipoArquivo::SassIndentado) => sass_indentado(arquivo, texto),
        Some(TipoArquivo::Tailwind) => objeto(arquivo, texto, true),
        Some(TipoArquivo::TemaScript) => objeto(arquivo, texto, false),
        None => Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// Nomes e categorias

/// Palavras de um nome: separa por qualquer caractere que não é letra nem
/// dígito e pelas emendas do camelCase, em minúsculas.
fn palavras(nome: &str) -> Vec<String> {
    let mut saida = Vec::new();
    let mut atual = String::new();
    let mut anterior_minuscula = false;
    for c in nome.chars() {
        if !c.is_alphanumeric() {
            if !atual.is_empty() {
                saida.push(std::mem::take(&mut atual));
            }
            anterior_minuscula = false;
            continue;
        }
        if c.is_uppercase() && anterior_minuscula && !atual.is_empty() {
            saida.push(std::mem::take(&mut atual));
        }
        anterior_minuscula = c.is_lowercase() || c.is_ascii_digit();
        atual.extend(c.to_lowercase());
    }
    if !atual.is_empty() {
        saida.push(atual);
    }
    saida
}

/// Nome de propriedade customizada a partir de partes (`["colors", "primaryDark"]`
/// → `--colors-primary-dark`).
fn propriedade<'a>(partes: impl IntoIterator<Item = &'a str>) -> String {
    let mut nome = String::from("-");
    for parte in partes {
        for p in palavras(parte) {
            nome.push('-');
            nome.push_str(&p);
        }
    }
    nome
}

const RAIO: &[&str] = &[
    "raio",
    "raios",
    "radius",
    "radii",
    "rounded",
    "arredondamento",
];
const TIPOGRAFIA: &[&str] = &[
    "fonte",
    "fontes",
    "font",
    "fonts",
    "tipografia",
    "typography",
    "family",
    "familia",
    "leading",
    "tracking",
    "weight",
    "peso",
    "letter",
];
const ESPACO: &[&str] = &[
    "espaco",
    "espacos",
    "espacamento",
    "spacing",
    "space",
    "spaces",
    "gap",
    "gutter",
    "margem",
    "margin",
    "padding",
    "recuo",
];
const COR: &[&str] = &[
    "cor",
    "cores",
    "color",
    "colors",
    "colour",
    "bg",
    "background",
    "fundo",
    "paleta",
    "palette",
];

/// Se o valor é uma cor literal (hexadecimal ou função de cor).
fn e_cor(valor: &str) -> bool {
    static HEX: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^#(?:[0-9a-fA-F]{3,4}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})$").unwrap()
    });
    let v = valor.trim().to_lowercase();
    HEX.is_match(&v)
        || [
            "rgb(", "rgba(", "hsl(", "hsla(", "hwb(", "lab(", "lch(", "oklab(", "oklch(",
        ]
        .iter()
        .any(|f| v.starts_with(f))
}

/// Categoria pelo nome e pelo valor; `None` para o que não é token de interesse.
fn categoria(nome: &str, valor: Option<&str>) -> Option<CategoriaToken> {
    let partes = palavras(nome);
    let tem = |lista: &[&str]| partes.iter().any(|p| lista.contains(&p.as_str()));
    let junto = format!("-{}-", partes.join("-"));
    if valor.is_some_and(e_cor) {
        return Some(CategoriaToken::Cor);
    }
    if tem(RAIO) {
        return Some(CategoriaToken::Raio);
    }
    if tem(TIPOGRAFIA) || junto.contains("-line-height-") {
        return Some(CategoriaToken::Tipografia);
    }
    if tem(ESPACO) {
        return Some(CategoriaToken::Espacamento);
    }
    if tem(COR) {
        return Some(CategoriaToken::Cor);
    }
    None
}

// ---------------------------------------------------------------------------
// Valores

/// Funções que o navegador resolve; outra função no valor é do pré-processador.
const FUNCOES_DO_CSS: &[&str] = &[
    "rgb",
    "rgba",
    "hsl",
    "hsla",
    "hwb",
    "lab",
    "lch",
    "oklab",
    "oklch",
    "color",
    "color-mix",
    "var",
    "calc",
    "min",
    "max",
    "clamp",
    "url",
    "local",
    "format",
];

/// Resultado da limpeza de um valor.
enum Valor {
    /// Pronto para o CSS.
    Pronto(String),
    /// Achado, mas sem valor literal: a nota diz por quê.
    AConfirmar(String),
    /// Não é token (mapa do SCSS, valor vazio).
    Ignorar,
}

/// Limpa o valor bruto de uma declaração. `pre` é o prefixo das variáveis do
/// pré-processador do arquivo (`$` ou `@`), trocadas por `var(--nome)`.
fn limpar(bruto: &str, pre: Option<char>) -> Valor {
    static FLAG: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\s*!(?:default|global|important)\b").unwrap());
    static FUNCAO: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"([A-Za-z][\w-]*)\(").unwrap());
    static VARIAVEL_SCSS: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\$([A-Za-z_][\w-]*)").unwrap());
    static VARIAVEL_LESS: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"@([A-Za-z_][\w-]*)").unwrap());
    let sem_flag = FLAG.replace_all(bruto, "");
    let v = sem_flag.split_whitespace().collect::<Vec<_>>().join(" ");
    if v.is_empty() || v.starts_with('(') {
        return Valor::Ignorar;
    }
    if v.contains("#{") || v.contains("@{") {
        return Valor::AConfirmar(format!(
            "valor com interpolação do pré-processador (`{v}`), não resolvido"
        ));
    }
    if let Some(f) = FUNCAO
        .captures_iter(&v)
        .map(|c| c[1].to_lowercase())
        .find(|f| !FUNCOES_DO_CSS.contains(&f.as_str()))
    {
        return Valor::AConfirmar(format!(
            "valor calculado pelo pré-processador (`{v}`, função `{f}`), não resolvido"
        ));
    }
    let v = match pre {
        Some('$') => VARIAVEL_SCSS.replace_all(&v, "var(--$1)").into_owned(),
        Some('@') => VARIAVEL_LESS.replace_all(&v, "var(--$1)").into_owned(),
        _ => v,
    };
    Valor::Pronto(v)
}

fn token(
    arquivo: &str,
    linha: u32,
    nome: String,
    original: String,
    categoria: CategoriaToken,
    valor: Valor,
) -> Option<TokenEstilo> {
    let (valor, nota) = match valor {
        Valor::Pronto(v) => (Some(v), None),
        Valor::AConfirmar(n) => (None, Some(n)),
        Valor::Ignorar => return None,
    };
    Some(TokenEstilo {
        nome,
        original,
        categoria,
        valor,
        origem: OrigemToken {
            arquivo: arquivo.to_owned(),
            linha,
        },
        nota,
    })
}

// ---------------------------------------------------------------------------
// Folhas de estilo

/// Troca os comentários por espaços, mantendo as quebras de linha (e as
/// posições). `linha`: também os comentários de linha (`//`), do SCSS, do LESS
/// e dos scripts. Strings ficam como estão.
fn sem_comentarios(texto: &str, linha: bool) -> String {
    let mut saida = String::with_capacity(texto.len());
    let mut it = texto.chars().peekable();
    let mut aspas: Option<char> = None;
    while let Some(c) = it.next() {
        if let Some(q) = aspas {
            saida.push(c);
            if c == '\\' {
                if let Some(n) = it.next() {
                    saida.push(n);
                }
            } else if c == q || c == '\n' {
                aspas = None;
            }
            continue;
        }
        match (c, it.peek()) {
            ('"' | '\'' | '`', _) => {
                aspas = Some(c);
                saida.push(c);
            }
            ('/', Some('*')) => {
                it.next();
                saida.push_str("  ");
                let mut anterior = ' ';
                for n in it.by_ref() {
                    saida.push(if n == '\n' { '\n' } else { ' ' });
                    if anterior == '*' && n == '/' {
                        break;
                    }
                    anterior = n;
                }
            }
            ('/', Some('/')) if linha => {
                for n in it.by_ref() {
                    if n == '\n' {
                        saida.push('\n');
                        break;
                    }
                    saida.push(' ');
                }
            }
            _ => saida.push(c),
        }
    }
    saida
}

/// Número da linha (a partir de 1) de uma posição do texto.
fn linha_de(texto: &str, posicao: usize) -> u32 {
    let n = texto[..posicao].bytes().filter(|b| *b == b'\n').count() + 1;
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// Propriedades das regras `:root`, `html` e `body` que viram token.
const DECLARACOES_DA_PAGINA: &[(&str, CategoriaToken)] = &[
    ("color", CategoriaToken::Cor),
    ("background-color", CategoriaToken::Cor),
    ("background", CategoriaToken::Cor),
    ("font-family", CategoriaToken::Tipografia),
    ("font-size", CategoriaToken::Tipografia),
    ("font-weight", CategoriaToken::Tipografia),
    ("line-height", CategoriaToken::Tipografia),
    ("border-radius", CategoriaToken::Raio),
];

fn folha(arquivo: &str, texto: &str) -> Vec<TokenEstilo> {
    static VARIAVEL: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(--[A-Za-z0-9_-]+|\$[A-Za-z_][A-Za-z0-9_-]*|@[A-Za-z_][A-Za-z0-9_-]*)\s*:\s*([^;{}]*)[;}]")
            .unwrap()
    });
    static PAGINA: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?:^|[\s};,])(:root|html|body)\s*\{([^{}]*)\}").unwrap());
    static DECLARACAO: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"([a-z-]+)\s*:\s*([^;]+)").unwrap());

    let minusculo = arquivo.to_lowercase();
    let pre = if minusculo.ends_with(".less") {
        Some('@')
    } else if minusculo.ends_with(".scss") {
        Some('$')
    } else {
        None
    };
    let limpo = sem_comentarios(texto, pre.is_some());
    let mut tokens = Vec::new();
    for c in VARIAVEL.captures_iter(&limpo) {
        let original = c.get(1).expect("grupo").as_str();
        // `@` só é variável no LESS (no CSS e no SCSS é regra: `@media`).
        if original.starts_with('@') && pre != Some('@') {
            continue;
        }
        if original.starts_with('$') && pre != Some('$') {
            continue;
        }
        let bruto = c.get(2).expect("grupo").as_str();
        let valor = limpar(bruto, pre);
        let pronto = match &valor {
            Valor::Pronto(v) => Some(v.as_str()),
            _ => None,
        };
        let Some(cat) = categoria(original, pronto) else {
            continue;
        };
        let nome = format!("--{}", original.trim_start_matches(['-', '$', '@']));
        let linha = linha_de(&limpo, c.get(1).expect("grupo").start());
        tokens.extend(token(arquivo, linha, nome, original.to_owned(), cat, valor));
    }
    for c in PAGINA.captures_iter(&limpo) {
        let seletor = c.get(1).expect("grupo").as_str();
        let corpo = c.get(2).expect("grupo");
        for d in DECLARACAO.captures_iter(corpo.as_str()) {
            let propriedade = d.get(1).expect("grupo").as_str();
            let Some((_, cat)) = DECLARACOES_DA_PAGINA
                .iter()
                .find(|(p, _)| *p == propriedade)
            else {
                continue;
            };
            let bruto = d.get(2).expect("grupo").as_str().trim();
            // Referência a uma variável já é o token dela.
            if bruto.starts_with("var(") || bruto.starts_with('$') || bruto.starts_with('@') {
                continue;
            }
            let valor = limpar(bruto, pre);
            if propriedade == "background" && !matches!(&valor, Valor::Pronto(v) if e_cor(v)) {
                continue;
            }
            let nome = format!(
                "--{}-{propriedade}",
                if seletor == ":root" { "root" } else { seletor }
            );
            let original = format!("{seletor} {{ {propriedade} }}");
            let linha = linha_de(&limpo, corpo.start() + d.get(1).expect("grupo").start());
            tokens.extend(token(arquivo, linha, nome, original, *cat, valor));
        }
    }
    tokens.sort_by_key(|t| t.origem.linha);
    tokens
}

fn sass_indentado(arquivo: &str, texto: &str) -> Vec<TokenEstilo> {
    static VARIAVEL: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^\s*(\$[A-Za-z_][A-Za-z0-9_-]*)\s*:\s*(.+)$").unwrap());
    let limpo = sem_comentarios(texto, true);
    let mut tokens = Vec::new();
    for (i, l) in limpo.lines().enumerate() {
        let Some(c) = VARIAVEL.captures(l) else {
            continue;
        };
        let original = &c[1];
        let valor = limpar(&c[2], Some('$'));
        let pronto = match &valor {
            Valor::Pronto(v) => Some(v.as_str()),
            _ => None,
        };
        let Some(cat) = categoria(original, pronto) else {
            continue;
        };
        let linha = u32::try_from(i + 1).unwrap_or(u32::MAX);
        let nome = format!("--{}", &original[1..]);
        tokens.extend(token(arquivo, linha, nome, original.to_owned(), cat, valor));
    }
    tokens
}

// ---------------------------------------------------------------------------
// Objetos de JavaScript e TypeScript (Tailwind e temas)

#[derive(Debug, Clone, PartialEq)]
enum Lexema {
    Nome(String),
    Texto(String),
    Numero(String),
    Simbolo(char),
}

/// Lexemas de um script, com a linha. Comentários ficam de fora; template
/// string com interpolação vira símbolo (não é valor literal).
fn lexemas(texto: &str) -> Vec<(Lexema, u32)> {
    let limpo = sem_comentarios(texto, true);
    let mut saida = Vec::new();
    let mut linha = 1u32;
    let mut it = limpo.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '\n' => linha += 1,
            c if c.is_whitespace() => {}
            '"' | '\'' | '`' => {
                let inicio = linha;
                let mut s = String::new();
                let mut interpolado = false;
                while let Some(n) = it.next() {
                    match n {
                        '\\' => {
                            if let Some(e) = it.next() {
                                s.push(e);
                            }
                        }
                        n if n == c => break,
                        '\n' => {
                            linha += 1;
                            s.push(n);
                        }
                        '$' if c == '`' && it.peek() == Some(&'{') => {
                            interpolado = true;
                            s.push(n);
                        }
                        n => s.push(n),
                    }
                }
                saida.push((
                    if interpolado {
                        Lexema::Simbolo('`')
                    } else {
                        Lexema::Texto(s)
                    },
                    inicio,
                ));
            }
            c if c.is_ascii_digit() => {
                let mut s = String::from(c);
                while let Some(&n) = it.peek() {
                    if n.is_ascii_alphanumeric() || n == '.' || n == '%' {
                        s.push(n);
                        it.next();
                    } else {
                        break;
                    }
                }
                saida.push((Lexema::Numero(s), linha));
            }
            c if c.is_alphabetic() || c == '_' || c == '$' => {
                let mut s = String::from(c);
                while let Some(&n) = it.peek() {
                    if n.is_alphanumeric() || n == '_' || n == '$' {
                        s.push(n);
                        it.next();
                    } else {
                        break;
                    }
                }
                saida.push((Lexema::Nome(s), linha));
            }
            c => saida.push((Lexema::Simbolo(c), linha)),
        }
    }
    saida
}

/// Seções do `theme` do Tailwind: categoria e prefixo do nome do token.
const SECOES_TAILWIND: &[(&str, CategoriaToken, &str)] = &[
    ("colors", CategoriaToken::Cor, "cor"),
    ("backgroundColor", CategoriaToken::Cor, "cor-fundo"),
    ("textColor", CategoriaToken::Cor, "cor-texto"),
    ("borderColor", CategoriaToken::Cor, "cor-borda"),
    ("fontFamily", CategoriaToken::Tipografia, "fonte-familia"),
    ("fontSize", CategoriaToken::Tipografia, "fonte-tamanho"),
    ("fontWeight", CategoriaToken::Tipografia, "fonte-peso"),
    (
        "lineHeight",
        CategoriaToken::Tipografia,
        "fonte-altura-linha",
    ),
    ("spacing", CategoriaToken::Espacamento, "espaco"),
    ("borderRadius", CategoriaToken::Raio, "raio"),
];

/// Valor de uma folha do objeto: texto ou número, ou a lista deles (a família da
/// fonte une todos; nas outras seções vale o primeiro, como em
/// `fontSize: ['14px', { lineHeight: '20px' }]`).
fn valor_da_lista(itens: &[String], familia: bool) -> Option<String> {
    if familia {
        let partes: Vec<String> = itens
            .iter()
            .map(|i| {
                if i.contains(' ') {
                    format!("\"{i}\"")
                } else {
                    i.clone()
                }
            })
            .collect();
        (!partes.is_empty()).then(|| partes.join(", "))
    } else {
        itens.first().cloned()
    }
}

/// Folhas (chave com valor literal) de um objeto de script: caminho das chaves,
/// valor, se é número e a linha.
fn folhas(texto: &str) -> Vec<(Vec<String>, String, bool, u32)> {
    let lx = lexemas(texto);
    let mut pilha: Vec<Option<String>> = Vec::new();
    let mut saida = Vec::new();
    let caminho = |pilha: &[Option<String>], chave: &str| -> Vec<String> {
        pilha
            .iter()
            .flatten()
            .cloned()
            .chain(std::iter::once(chave.to_owned()))
            .collect()
    };
    let mut i = 0;
    while i < lx.len() {
        let (atual, linha) = &lx[i];
        let chave = match atual {
            Lexema::Nome(s) | Lexema::Texto(s) | Lexema::Numero(s) => Some(s.as_str()),
            Lexema::Simbolo(_) => None,
        };
        let dois_pontos = matches!(lx.get(i + 1), Some((Lexema::Simbolo(':'), _)));
        match (chave, dois_pontos) {
            (Some(chave), true) => {
                let fim_de_valor = |j: usize| {
                    matches!(
                        lx.get(j),
                        None | Some((Lexema::Simbolo(',' | '}' | ';'), _))
                    )
                };
                match lx.get(i + 2) {
                    Some((Lexema::Simbolo('{'), _)) => {
                        pilha.push(Some(chave.to_owned()));
                        i += 3;
                    }
                    Some((Lexema::Texto(v), _)) if fim_de_valor(i + 3) => {
                        saida.push((caminho(&pilha, chave), v.clone(), false, *linha));
                        i += 3;
                    }
                    Some((Lexema::Numero(v), _)) if fim_de_valor(i + 3) => {
                        saida.push((caminho(&pilha, chave), v.clone(), true, *linha));
                        i += 3;
                    }
                    Some((Lexema::Simbolo('['), _)) => {
                        // Lista: os textos e números do primeiro nível.
                        let mut itens = Vec::new();
                        let mut nivel = 0i32;
                        let mut j = i + 3;
                        let mut literal = true;
                        while let Some((l, _)) = lx.get(j) {
                            match l {
                                Lexema::Simbolo('[' | '{' | '(') => nivel += 1,
                                Lexema::Simbolo(']') if nivel == 0 => break,
                                Lexema::Simbolo(']' | '}' | ')') => nivel -= 1,
                                Lexema::Texto(s) | Lexema::Numero(s) if nivel == 0 => {
                                    itens.push(s.clone());
                                }
                                Lexema::Nome(_) if nivel == 0 => literal = false,
                                _ => {}
                            }
                            j += 1;
                        }
                        let caminho = caminho(&pilha, chave);
                        let familia = caminho
                            .iter()
                            .any(|c| palavras(c).iter().any(|p| p == "family" || p == "familia"));
                        if literal && let Some(v) = valor_da_lista(&itens, familia) {
                            saida.push((caminho, v, false, *linha));
                        }
                        i = j + 1;
                    }
                    _ => i += 2,
                }
            }
            _ => {
                match atual {
                    Lexema::Simbolo('{') => pilha.push(None),
                    Lexema::Simbolo('}') => {
                        pilha.pop();
                    }
                    _ => {}
                }
                i += 1;
            }
        }
    }
    saida
}

/// Se o número sem unidade é uma medida incompleta (espaço, raio, tamanho de
/// fonte): o código não diz a unidade.
fn medida_sem_unidade(categoria: CategoriaToken, nome: &str, valor: &str, numero: bool) -> bool {
    if !numero || !valor.chars().all(|c| c.is_ascii_digit() || c == '.') || valor == "0" {
        return false;
    }
    match categoria {
        CategoriaToken::Espacamento | CategoriaToken::Raio => true,
        CategoriaToken::Tipografia => palavras(nome)
            .iter()
            .any(|p| matches!(p.as_str(), "size" | "tamanho")),
        CategoriaToken::Cor => false,
    }
}

fn objeto(arquivo: &str, texto: &str, tailwind: bool) -> Vec<TokenEstilo> {
    let mut tokens = Vec::new();
    for (caminho, valor, numero, linha) in folhas(texto) {
        let original = caminho.join(".");
        let (nome, cat) = if tailwind {
            // theme.<secao>... ou theme.extend.<secao>...
            let Some(t) = caminho.iter().position(|c| c == "theme") else {
                continue;
            };
            let mut resto = &caminho[t + 1..];
            if resto.first().is_some_and(|c| c == "extend") {
                resto = &resto[1..];
            }
            let Some(((_, cat, prefixo), resto)) = resto.split_first().and_then(|(s, r)| {
                SECOES_TAILWIND
                    .iter()
                    .find(|(nome, _, _)| nome == s)
                    .map(|sec| (sec, r))
            }) else {
                continue;
            };
            let partes = std::iter::once(*prefixo)
                .chain(resto.iter().map(String::as_str).filter(|p| *p != "DEFAULT"));
            (propriedade(partes), *cat)
        } else {
            let nome = propriedade(caminho.iter().map(String::as_str));
            let Some(cat) = categoria(&nome, Some(&valor)) else {
                continue;
            };
            (nome, cat)
        };
        if nome == "-" {
            continue;
        }
        let v = if medida_sem_unidade(cat, &nome, &valor, numero) {
            Valor::AConfirmar(format!(
                "número sem unidade no código (`{valor}`): confirme a unidade"
            ))
        } else {
            limpar(&valor, None)
        };
        tokens.extend(token(arquivo, linha, nome, original, cat, v));
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resumo(tokens: &[TokenEstilo]) -> Vec<(String, CategoriaToken, Option<String>, u32)> {
        tokens
            .iter()
            .map(|t| (t.nome.clone(), t.categoria, t.valor.clone(), t.origem.linha))
            .collect()
    }

    #[test]
    fn palavras_e_propriedades() {
        assert_eq!(palavras("primaryDark-500"), ["primary", "dark", "500"]);
        assert_eq!(palavras("$cor_alerta"), ["cor", "alerta"]);
        assert_eq!(
            propriedade(["colors", "primaryDark"]),
            "--colors-primary-dark"
        );
    }

    #[test]
    fn tipos_de_arquivo() {
        assert_eq!(tipo_do_arquivo("a/b.SCSS"), Some(TipoArquivo::Folha));
        assert_eq!(
            tipo_do_arquivo("tailwind.config.ts"),
            Some(TipoArquivo::Tailwind)
        );
        assert_eq!(
            tipo_do_arquivo("src/temaPortal.ts"),
            Some(TipoArquivo::TemaScript)
        );
        assert_eq!(
            tipo_do_arquivo("src/design-tokens.js"),
            Some(TipoArquivo::TemaScript)
        );
        assert_eq!(tipo_do_arquivo("src/telas/parcelamento-form.ts"), None);
        assert_eq!(tipo_do_arquivo("x.sass"), Some(TipoArquivo::SassIndentado));
        assert_eq!(tipo_do_arquivo("README"), None);
    }

    #[test]
    fn css_com_propriedades_e_regra_da_pagina() {
        let css = "/* tema --cor-falsa: #000; */\n\
                   :root {\n  --cor-primaria: #1F5FA8;\n  --sombra: 0 1px rgba(0,0,0,.2);\n  \
                   --z-topo: 10;\n  --espaco-base: 8px\n}\n\
                   body { font-family: Arial, sans-serif; color: var(--cor-texto); }\n\
                   @media (min-width: 600px) { .a { --raio-base: 2px; } }\n";
        let t = extrair_do_texto("estilos/tema.css", css);
        assert_eq!(
            resumo(&t),
            [
                (
                    "--cor-primaria".into(),
                    CategoriaToken::Cor,
                    Some("#1F5FA8".into()),
                    3
                ),
                (
                    "--espaco-base".into(),
                    CategoriaToken::Espacamento,
                    Some("8px".into()),
                    6
                ),
                (
                    "--body-font-family".into(),
                    CategoriaToken::Tipografia,
                    Some("Arial, sans-serif".into()),
                    8
                ),
                (
                    "--raio-base".into(),
                    CategoriaToken::Raio,
                    Some("2px".into()),
                    9
                ),
            ]
        );
        assert_eq!(t[0].origem.to_string(), "estilos/tema.css:3");
        assert_eq!(t[2].original, "body { font-family }");
    }

    #[test]
    fn scss_e_less_com_valor_calculado_a_confirmar() {
        let scss = "// $cor-comentada: #fff;\n$cor-alerta: #b26a00;\n\
                    $cor-alerta-fundo: lighten($cor-alerta, 50%);\n\
                    $espaco-grande: $espaco-base * 3 !default;\n\
                    $cores: (primaria: #fff);\n\
                    @media screen { a { color: red; } }\n";
        let t = extrair_do_texto("x.scss", scss);
        assert_eq!(t.len(), 3, "{t:?}");
        assert_eq!(t[0].valor.as_deref(), Some("#b26a00"));
        assert_eq!(t[1].nome, "--cor-alerta-fundo");
        assert_eq!(t[1].valor, None);
        assert!(t[1].nota.as_deref().unwrap().contains("lighten"));
        assert_eq!(t[2].valor.as_deref(), Some("var(--espaco-base) * 3"));

        let less = "@cor-primaria: #123456;\n@import 'x';\n@raio: @raio-base;\n";
        let t = extrair_do_texto("x.less", less);
        assert_eq!(
            resumo(&t),
            [
                (
                    "--cor-primaria".into(),
                    CategoriaToken::Cor,
                    Some("#123456".into()),
                    1
                ),
                (
                    "--raio".into(),
                    CategoriaToken::Raio,
                    Some("var(--raio-base)".into()),
                    3
                ),
            ]
        );
    }

    #[test]
    fn tailwind_e_tema_em_script() {
        let tw = "module.exports = {\n  content: ['./src/**/*.tsx'],\n  theme: {\n    extend: {\n\
                  \x20     colors: { primary: { DEFAULT: '#1f5fa8', dark: '#123a66' } },\n\
                  \x20     fontFamily: { sans: ['Open Sans', 'Arial', 'sans-serif'] },\n\
                  \x20     fontSize: { base: ['14px', { lineHeight: '20px' }] },\n\
                  \x20     spacing: { 18: '4.5rem' },\n\
                  \x20     borderRadius: { md: '6px' },\n\
                  \x20     zIndex: { topo: '10' },\n    },\n  },\n};\n";
        let t = extrair_do_texto("tailwind.config.js", tw);
        assert_eq!(
            resumo(&t),
            [
                (
                    "--cor-primary".into(),
                    CategoriaToken::Cor,
                    Some("#1f5fa8".into()),
                    5
                ),
                (
                    "--cor-primary-dark".into(),
                    CategoriaToken::Cor,
                    Some("#123a66".into()),
                    5
                ),
                (
                    "--fonte-familia-sans".into(),
                    CategoriaToken::Tipografia,
                    Some("\"Open Sans\", Arial, sans-serif".into()),
                    6
                ),
                (
                    "--fonte-tamanho-base".into(),
                    CategoriaToken::Tipografia,
                    Some("14px".into()),
                    7
                ),
                (
                    "--espaco-18".into(),
                    CategoriaToken::Espacamento,
                    Some("4.5rem".into()),
                    8
                ),
                (
                    "--raio-md".into(),
                    CategoriaToken::Raio,
                    Some("6px".into()),
                    9
                ),
            ]
        );
        assert_eq!(t[0].original, "theme.extend.colors.primary.DEFAULT");

        let tema = "export const tema = {\n  cores: { primaria: '#0a0', fundoClaro: \"#fafafa\" },\n\
                    espacos: { pequeno: 4, medio: '8px' },\n  sombra: '0 1px #000',\n  \
                    rotulo: `${x}px`,\n};\n";
        let t = extrair_do_texto("src/tema.ts", tema);
        assert_eq!(
            resumo(&t),
            [
                (
                    "--cores-primaria".into(),
                    CategoriaToken::Cor,
                    Some("#0a0".into()),
                    2
                ),
                (
                    "--cores-fundo-claro".into(),
                    CategoriaToken::Cor,
                    Some("#fafafa".into()),
                    2
                ),
                (
                    "--espacos-pequeno".into(),
                    CategoriaToken::Espacamento,
                    None,
                    3
                ),
                (
                    "--espacos-medio".into(),
                    CategoriaToken::Espacamento,
                    Some("8px".into()),
                    3
                ),
            ]
        );
        assert!(t[2].nota.as_deref().unwrap().contains("sem unidade"));
    }

    #[test]
    fn extrai_da_fixture_pelo_escopo() {
        let raiz = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/faturamento-exemplo");
        let projeto = crate::Projeto::novo("Faturamento Exemplo", [raiz]).unwrap();
        let tokens = extrair_estilo(&projeto.escopo().unwrap());
        let nomes: Vec<&str> = tokens.iter().map(|t| t.nome.as_str()).collect();
        assert!(nomes.contains(&"--cor-primaria"), "{nomes:?}");
        assert!(nomes.contains(&"--cor-alerta-fundo"));
        assert!(!nomes.contains(&"--sombra-cartao"));
        assert!(
            tokens
                .iter()
                .all(|t| t.origem.arquivo.starts_with("frontend/src/estilos/"))
        );
        let primaria = tokens.iter().find(|t| t.nome == "--cor-primaria").unwrap();
        assert_eq!(primaria.origem.arquivo, "frontend/src/estilos/tema.css");
        assert_eq!(primaria.origem.linha, 3);
    }
}
