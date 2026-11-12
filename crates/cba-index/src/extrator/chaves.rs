//! Léxico e análise estrutural comuns às linguagens de chaves do MVP: Java, C# e
//! TypeScript (que também serve a JavaScript).
//!
//! Sem gramática completa: um analisador léxico próprio (comentários, strings,
//! text blocks do Java, verbatim, interpoladas e raw strings do C#, template
//! literals e regex do TypeScript) e uma descida rasa que delimita os blocos pelas
//! chaves. Nos corpos de tipo, cada membro é lido pelo cabeçalho (o que vem antes
//! de `{`, `;`, `=` ou `=>`); nos corpos de método, só se procuram referências.
//!
//! O que é próprio de cada framework (anotações de entidade e de endpoint,
//! clientes HTTP) fica no módulo da linguagem, num [`Dialeto`].

use std::collections::{HashMap, HashSet};

use crate::extrator::generico::{Separado, mencoes};
use crate::extrator::sql::resumir;
use crate::extrator::{Extracao, Fonte, Linhas, Referencia, Simbolo, TipoReferencia, TipoSimbolo};

// ---------------------------------------------------------------------------
// Dialeto
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Ling {
    Java,
    CSharp,
    TypeScript,
}

/// Anotação do Java, atributo do C# ou decorator do TypeScript.
#[derive(Debug, Clone, Default)]
pub(crate) struct Anotacao {
    /// Último segmento do nome, sem o sufixo `Attribute` do C#.
    pub nome: String,
    pub args: Vec<Arg>,
    /// Pares `chave = "texto"` ou `chave: 'texto'` em qualquer ponto dos
    /// argumentos (inclusive dentro de objetos).
    pub pares: Vec<(String, String)>,
}

/// Argumento de uma anotação.
#[derive(Debug, Clone, Default)]
pub(crate) struct Arg {
    /// Nome do argumento nomeado (`name = "X"`), se houver.
    pub nome: Option<String>,
    /// Strings do argumento (várias num array: `{"/a", "/b"}`).
    pub textos: Vec<String>,
    /// Tokens juntos, sem espaços (`RequestMethod.POST`).
    pub bruto: String,
}

impl Anotacao {
    /// Strings do argumento sem nome ou de um dos nomes dados.
    pub fn textos(&self, nomes: &[&str]) -> Vec<String> {
        self.args
            .iter()
            .filter(|a| match &a.nome {
                None => true,
                Some(n) => nomes.iter().any(|x| x.eq_ignore_ascii_case(n)),
            })
            .flat_map(|a| a.textos.iter().cloned())
            .collect()
    }

    /// Valor do par `chave: 'texto'` (sem diferenciar maiúsculas).
    pub fn par(&self, chave: &str) -> Option<&str> {
        self.pares
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(chave))
            .map(|(_, v)| v.as_str())
    }
}

/// O que o framework diz de uma classe.
#[derive(Debug, Clone, Default)]
pub(crate) struct InfoClasse {
    /// Tipo no lugar de `Classe` (tela, por exemplo).
    pub tipo: Option<TipoSimbolo>,
    pub detalhe: Option<String>,
    /// Tabela mapeada pela entidade.
    pub tabela: Option<String>,
    /// Prefixos de rota de um controlador (`Some` mesmo sem prefixo).
    pub rotas: Option<Vec<String>>,
    /// Nome da classe (para `[controller]` do ASP.NET).
    pub nome: String,
}

/// Uma chamada de cliente HTTP ou o registro de uma rota no código.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Http {
    /// Chamada a um endpoint (cliente); a URL é o primeiro argumento.
    Chamada,
    /// Definição de rota no código (`app.MapGet`, `router.post`), com o verbo.
    Rota(String),
}

/// Endpoints de um método: (verbo, caminho completo).
pub(crate) type Rotas = Vec<(String, String)>;

/// O que é próprio da linguagem e dos frameworks dela.
pub(crate) struct Dialeto {
    pub ling: Ling,
    /// Anotações da classe, nome e bases.
    pub classe: fn(&[Anotacao], &str, &[String]) -> InfoClasse,
    /// Endpoints de um método: (verbo, caminho completo).
    pub rotas: fn(&[Anotacao], &str, &InfoClasse) -> Rotas,
    /// Coluna mapeada por um campo de entidade.
    pub coluna: fn(&[Anotacao]) -> Option<String>,
    /// Receptor, tipo do receptor e método chamado.
    pub http: fn(Option<&str>, Option<&str>, &str) -> Option<Http>,
    /// Pacotes ou módulos de bibliotecas: o que se importa deles não vira
    /// referência (só a importação).
    pub externo: fn(&str) -> bool,
}

/// Extrai símbolos e referências de um arquivo de linguagem de chaves.
pub(crate) fn analisar(fonte: &Fonte<'_>, d: &Dialeto) -> Extracao {
    let lex = lexico(fonte.texto, d.ling);
    let caminho = fonte.caminho.to_ascii_lowercase();
    let total = fonte.texto.lines().count().max(1) as u32;
    let mut p = P {
        d,
        t: &lex.tokens,
        com: &lex.comentarios,
        tsx: caminho.ends_with(".tsx") || caminho.ends_with(".jsx"),
        total,
        i: 0,
        simbolos: Vec::new(),
        endpoints: HashSet::new(),
        refs: Vec::new(),
        consts: HashMap::new(),
        externos: HashSet::new(),
        sem_mencao: vec![false; lex.tokens.len()],
    };
    let mut ctx = Ctx::arquivo();
    p.bloco(&mut ctx);
    p.concluir()
}

// ---------------------------------------------------------------------------
// Léxico
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum K {
    Ident,
    /// Número, caractere (`'x'` do Java e do C#) ou regex do TypeScript.
    Num,
    /// String, text block, template literal. O conteúdo, sem as aspas.
    Texto,
    Op,
}

#[derive(Debug, Clone)]
pub(crate) struct Tok {
    pub k: K,
    pub s: String,
    pub linha: u32,
    /// Linha onde o token termina (strings de várias linhas).
    pub fim: u32,
    /// String interpolada: `${...}` (TypeScript) ou `{...}` (C#) ficam no texto.
    pub interp: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct Comentario {
    pub ini: u32,
    pub fim: u32,
    pub texto: String,
    /// Sem código antes dele na linha em que começa.
    pub sozinho: bool,
}

#[derive(Debug, Default)]
pub(crate) struct Lexico {
    pub tokens: Vec<Tok>,
    pub comentarios: Vec<Comentario>,
}

const OPS: &[&str] = &[
    "===", "!==", "...", "=>", "->", "::", "?.", "??", "==", "!=", "<=", ">=", "&&", "||", "++",
    "--", "+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=",
];

/// Palavras depois das quais `/` abre uma regex (TypeScript).
const ANTES_DE_REGEX: &[&str] = &[
    "return",
    "typeof",
    "case",
    "do",
    "else",
    "in",
    "of",
    "new",
    "delete",
    "void",
    "throw",
    "yield",
    "await",
    "instanceof",
];

pub(crate) fn lexico(texto: &str, ling: Ling) -> Lexico {
    let c: Vec<char> = texto.chars().collect();
    let n = c.len();
    let at = |k: usize| -> char { if k < n { c[k] } else { '\0' } };
    let mut lx = Lexico::default();
    let mut i = 0;
    let mut linha = 1u32;
    // Linha em que terminou o último token.
    let mut ultima = 0u32;
    let push = |lx: &mut Lexico, k: K, s: String, ini: u32, fim: u32, interp: bool| {
        lx.tokens.push(Tok {
            k,
            s,
            linha: ini,
            fim,
            interp,
        });
        fim
    };
    while i < n {
        let ch = c[i];
        if ch == '\n' {
            linha += 1;
            i += 1;
            continue;
        }
        if ch.is_whitespace() {
            i += 1;
            continue;
        }
        if ch == '/' && at(i + 1) == '/' {
            let ini = i;
            while i < n && c[i] != '\n' {
                i += 1;
            }
            lx.comentarios.push(Comentario {
                ini: linha,
                fim: linha,
                texto: c[ini..i].iter().collect(),
                sozinho: ultima < linha,
            });
            continue;
        }
        if ch == '/' && at(i + 1) == '*' {
            let l0 = linha;
            let ini = i;
            i += 2;
            while i < n && !(c[i] == '*' && at(i + 1) == '/') {
                if c[i] == '\n' {
                    linha += 1;
                }
                i += 1;
            }
            i = (i + 2).min(n);
            lx.comentarios.push(Comentario {
                ini: l0,
                fim: linha,
                texto: c[ini..i].iter().collect(),
                sozinho: ultima < l0,
            });
            continue;
        }
        // Diretivas do pré-processador do C# (#region, #if).
        if ling == Ling::CSharp && ch == '#' && ultima < linha {
            while i < n && c[i] != '\n' {
                i += 1;
            }
            continue;
        }
        let l0 = linha;

        // Strings.
        if ling == Ling::Java && ch == '"' && at(i + 1) == '"' && at(i + 2) == '"' {
            // Text block.
            let mut s = String::new();
            let mut k = i + 3;
            while k < n && !(c[k] == '"' && at(k + 1) == '"' && at(k + 2) == '"') {
                if c[k] == '\\' && k + 1 < n {
                    s.push(c[k]);
                    k += 1;
                }
                if c[k] == '\n' {
                    linha += 1;
                }
                s.push(c[k]);
                k += 1;
            }
            i = (k + 3).min(n);
            ultima = push(&mut lx, K::Texto, s, l0, linha, false);
            continue;
        }
        if ling == Ling::CSharp && matches!(ch, '"' | '$' | '@') {
            let mut j = i;
            let mut dolares = 0;
            let mut verbatim = false;
            while at(j) == '$' {
                dolares += 1;
                j += 1;
            }
            if at(j) == '@' {
                verbatim = true;
                j += 1;
                while at(j) == '$' {
                    dolares += 1;
                    j += 1;
                }
            }
            if at(j) == '"' {
                let mut aspas = 0;
                while at(j + aspas) == '"' {
                    aspas += 1;
                }
                let interp = dolares > 0;
                let mut s = String::new();
                let mut k;
                if aspas >= 3 {
                    // Raw string: termina com a mesma quantidade de aspas.
                    k = j + aspas;
                    while k < n {
                        if (0..aspas).all(|o| at(k + o) == '"') {
                            k += aspas;
                            break;
                        }
                        if c[k] == '\n' {
                            linha += 1;
                        }
                        s.push(c[k]);
                        k += 1;
                    }
                    let s = s.trim_matches('\n').to_string();
                    i = k.min(n);
                    ultima = push(&mut lx, K::Texto, s, l0, linha, interp);
                    continue;
                }
                k = j + 1;
                while k < n {
                    let x = c[k];
                    if x == '"' {
                        if verbatim && at(k + 1) == '"' {
                            s.push('"');
                            k += 2;
                            continue;
                        }
                        k += 1;
                        break;
                    }
                    if x == '\n' {
                        if !verbatim {
                            break;
                        }
                        linha += 1;
                    }
                    if !verbatim && x == '\\' && k + 1 < n && c[k + 1] != '\n' {
                        s.push(x);
                        s.push(c[k + 1]);
                        k += 2;
                        continue;
                    }
                    if interp && x == '{' {
                        if at(k + 1) == '{' {
                            s.push_str("{{");
                            k += 2;
                            continue;
                        }
                        k = interpolacao(&c, k, &mut s, &mut linha);
                        continue;
                    }
                    s.push(x);
                    k += 1;
                }
                i = k.min(n);
                ultima = push(&mut lx, K::Texto, s, l0, linha, interp);
                continue;
            }
            if ch == '@' && (at(i + 1).is_alphabetic() || at(i + 1) == '_') {
                // Identificador verbatim (@class).
                i += 1;
                continue;
            }
        }
        if ch == '"' || (ch == '\'' && ling == Ling::TypeScript) {
            let (s, k) = texto_simples(&c, i, ch, &mut linha);
            i = k;
            ultima = push(&mut lx, K::Texto, s, l0, linha, false);
            continue;
        }
        if ch == '\'' {
            // Caractere do Java e do C#.
            let (s, k) = texto_simples(&c, i, ch, &mut linha);
            i = k;
            ultima = push(&mut lx, K::Num, s, l0, linha, false);
            continue;
        }
        if ch == '`' && ling == Ling::TypeScript {
            let mut s = String::new();
            let mut k = i + 1;
            while k < n && c[k] != '`' {
                if c[k] == '\\' && k + 1 < n {
                    s.push(c[k]);
                    s.push(c[k + 1]);
                    if c[k + 1] == '\n' {
                        linha += 1;
                    }
                    k += 2;
                    continue;
                }
                if c[k] == '$' && at(k + 1) == '{' {
                    s.push('$');
                    k = interpolacao(&c, k + 1, &mut s, &mut linha);
                    continue;
                }
                if c[k] == '\n' {
                    linha += 1;
                }
                s.push(c[k]);
                k += 1;
            }
            i = (k + 1).min(n);
            ultima = push(&mut lx, K::Texto, s, l0, linha, true);
            continue;
        }
        if ch == '/' && ling == Ling::TypeScript && regex_permitida(lx.tokens.last()) {
            let mut k = i + 1;
            let mut classe = false;
            let mut ok = false;
            while k < n && c[k] != '\n' {
                match c[k] {
                    '\\' => k += 1,
                    '[' => classe = true,
                    ']' => classe = false,
                    '/' if !classe => {
                        ok = true;
                        break;
                    }
                    _ => {}
                }
                k += 1;
            }
            if ok {
                k += 1;
                while k < n && c[k].is_ascii_alphabetic() {
                    k += 1;
                }
                let s: String = c[i..k].iter().collect();
                i = k;
                ultima = push(&mut lx, K::Num, s, l0, linha, false);
                continue;
            }
        }

        // Identificadores e números.
        if ch.is_alphabetic() || ch == '_' || (ch == '$' && ling != Ling::CSharp) {
            let ini = i;
            while i < n && (c[i].is_alphanumeric() || c[i] == '_' || c[i] == '$') {
                i += 1;
            }
            let s: String = c[ini..i].iter().collect();
            ultima = push(&mut lx, K::Ident, s, l0, linha, false);
            continue;
        }
        if ch.is_ascii_digit() || (ch == '.' && at(i + 1).is_ascii_digit()) {
            let ini = i;
            while i < n
                && (c[i].is_alphanumeric()
                    || c[i] == '_'
                    || (c[i] == '.' && at(i + 1).is_ascii_digit()))
            {
                i += 1;
            }
            let s: String = c[ini..i].iter().collect();
            ultima = push(&mut lx, K::Num, s, l0, linha, false);
            continue;
        }

        // Operadores.
        let op = OPS
            .iter()
            .find(|o| o.chars().enumerate().all(|(d, oc)| at(i + d) == oc))
            .map(|o| o.to_string())
            .unwrap_or_else(|| ch.to_string());
        i += op.chars().count();
        ultima = push(&mut lx, K::Op, op, l0, linha, false);
    }
    lx
}

/// String de uma linha com escapes `\`. Devolve o conteúdo e a posição seguinte.
/// Sem fechamento na linha, termina nela (um apóstrofo solto não contamina o
/// resto do arquivo).
fn texto_simples(c: &[char], i: usize, aspa: char, linha: &mut u32) -> (String, usize) {
    let mut s = String::new();
    let mut k = i + 1;
    while k < c.len() {
        let x = c[k];
        if x == aspa {
            return (s, k + 1);
        }
        if x == '\n' {
            return (s, k);
        }
        if x == '\\' && k + 1 < c.len() {
            if c[k + 1] == '\n' {
                *linha += 1;
            }
            s.push(x);
            s.push(c[k + 1]);
            k += 2;
            continue;
        }
        s.push(x);
        k += 1;
    }
    (s, k)
}

/// Copia `{...}` de uma string interpolada a partir da chave em `k`, com as
/// strings de dentro. Devolve a posição depois da chave que fecha.
fn interpolacao(c: &[char], k: usize, s: &mut String, linha: &mut u32) -> usize {
    let mut prof = 0;
    let mut k = k;
    while k < c.len() {
        let x = c[k];
        match x {
            '{' => prof += 1,
            '}' => {
                prof -= 1;
                if prof == 0 {
                    s.push('}');
                    return k + 1;
                }
            }
            '"' | '\'' | '`' => {
                s.push(x);
                k += 1;
                while k < c.len() && c[k] != x && c[k] != '\n' {
                    if c[k] == '\\' {
                        s.push(c[k]);
                        k += 1;
                        if k >= c.len() {
                            break;
                        }
                    }
                    s.push(c[k]);
                    k += 1;
                }
                if k < c.len() && c[k] == x {
                    s.push(x);
                    k += 1;
                }
                continue;
            }
            '\n' => *linha += 1,
            _ => {}
        }
        s.push(x);
        k += 1;
    }
    k
}

fn regex_permitida(anterior: Option<&Tok>) -> bool {
    match anterior {
        None => true,
        Some(t) => match t.k {
            K::Op => !matches!(t.s.as_str(), ")" | "]" | "}" | "<" | "++" | "--"),
            K::Ident => ANTES_DE_REGEX.contains(&t.s.as_str()),
            _ => false,
        },
    }
}

// ---------------------------------------------------------------------------
// Análise estrutural
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Corpo {
    Arquivo,
    Namespace,
    Classe,
    Interface,
    Enum,
}

#[derive(Debug, Clone)]
struct Ctx {
    corpo: Corpo,
    /// Nome qualificado do contêiner.
    qual: Option<String>,
    /// Nome simples da classe (construtores).
    classe: Option<String>,
    info: InfoClasse,
    /// Variáveis e campos com o tipo declarado.
    vars: HashMap<String, String>,
}

impl Ctx {
    fn arquivo() -> Self {
        Ctx {
            corpo: Corpo::Arquivo,
            qual: None,
            classe: None,
            info: InfoClasse::default(),
            vars: HashMap::new(),
        }
    }
}

/// Término do cabeçalho de um membro.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fim {
    Chave,
    PontoVirgula,
    Igual,
    Seta,
    /// Fim de instrução sem `;` (TypeScript) ou `}` do bloco de fora.
    Implicito,
}

const MODIFICADORES: &[&str] = &[
    "public",
    "private",
    "protected",
    "internal",
    "static",
    "final",
    "abstract",
    "sealed",
    "partial",
    "export",
    "default",
    "async",
    "virtual",
    "override",
    "readonly",
    "declare",
    "const",
    "let",
    "var",
    "function",
    "transient",
    "volatile",
    "synchronized",
    "native",
    "strictfp",
    "extern",
    "unsafe",
    "required",
    "event",
    "global",
    "accessor",
    "get",
    "set",
    "non",
];

/// Palavras que nunca são nome de rotina chamada nem tipo.
const PALAVRAS: &[&str] = &[
    "if",
    "for",
    "while",
    "switch",
    "catch",
    "return",
    "new",
    "typeof",
    "sizeof",
    "nameof",
    "using",
    "lock",
    "foreach",
    "function",
    "super",
    "this",
    "base",
    "synchronized",
    "do",
    "else",
    "throw",
    "await",
    "yield",
    "instanceof",
    "in",
    "of",
    "case",
    "default",
    "when",
    "fixed",
    "checked",
    "unchecked",
    "var",
    "let",
    "const",
    "void",
    "delete",
    "import",
    "export",
    "class",
    "interface",
    "enum",
    "struct",
    "record",
    "async",
    "static",
    "public",
    "private",
    "protected",
    "readonly",
    "assert",
    "try",
    "finally",
    "goto",
    "stackalloc",
    "is",
    "as",
    "out",
    "ref",
    "params",
    "operator",
    "and",
    "or",
    "not",
    "with",
    "where",
    "select",
    "from",
    "throws",
    "extends",
    "implements",
    "final",
    "int",
    "long",
    "short",
    "byte",
    "char",
    "boolean",
    "bool",
    "double",
    "float",
    "decimal",
    "string",
    "object",
    "dynamic",
    "uint",
    "ulong",
    "ushort",
    "sbyte",
    "nint",
    "nuint",
    "number",
    "any",
    "unknown",
    "never",
    "undefined",
    "null",
    "true",
    "false",
];

/// Tipos e objetos da plataforma (JDK, BCL, JavaScript): não viram referência.
const PLATAFORMA: &[&str] = &[
    // Java.
    "String",
    "Object",
    "Integer",
    "Long",
    "Short",
    "Byte",
    "Double",
    "Float",
    "Boolean",
    "Character",
    "Void",
    "Number",
    "Math",
    "System",
    "BigDecimal",
    "BigInteger",
    "List",
    "ArrayList",
    "LinkedList",
    "Map",
    "HashMap",
    "LinkedHashMap",
    "TreeMap",
    "Set",
    "HashSet",
    "LinkedHashSet",
    "TreeSet",
    "Collection",
    "Collections",
    "Arrays",
    "Objects",
    "Iterable",
    "Iterator",
    "Optional",
    "Stream",
    "Collectors",
    "LocalDate",
    "LocalDateTime",
    "LocalTime",
    "Instant",
    "Duration",
    "Date",
    "Exception",
    "RuntimeException",
    "IllegalArgumentException",
    "IllegalStateException",
    "NullPointerException",
    "UnsupportedOperationException",
    "Throwable",
    "Error",
    "StringBuilder",
    "Thread",
    "Class",
    "Override",
    // C#.
    "Task",
    "ValueTask",
    "IEnumerable",
    "IList",
    "ICollection",
    "IReadOnlyList",
    "IReadOnlyCollection",
    "Dictionary",
    "IDictionary",
    "DateTime",
    "DateTimeOffset",
    "DateOnly",
    "TimeSpan",
    "Guid",
    "Decimal",
    "Int32",
    "Int64",
    "Func",
    "Action",
    "CancellationToken",
    "Console",
    "Convert",
    "Enumerable",
    "ArgumentException",
    "ArgumentNullException",
    "InvalidOperationException",
    "NotImplementedException",
    "Nullable",
    "Array",
    "Type",
    // TypeScript e JavaScript.
    "Promise",
    "Record",
    "Partial",
    "Required",
    "Readonly",
    "Pick",
    "Omit",
    "ReadonlyArray",
    "JSON",
    "Function",
    "RegExp",
    "Symbol",
    "Uint8Array",
    "ArrayBuffer",
    "Intl",
    "Reflect",
    "Proxy",
    "WeakMap",
    "WeakSet",
];

fn plataforma(nome: &str) -> bool {
    let primeiro = nome.split('.').next().unwrap_or(nome);
    PLATAFORMA.contains(&primeiro)
}

fn maiuscula(s: &str) -> bool {
    s.chars().next().is_some_and(|c| c.is_uppercase())
}

/// Nome de constante: só maiúsculas, dígitos e `_`.
fn eh_constante(s: &str) -> bool {
    s.len() >= 2
        && s.chars().any(|c| c.is_ascii_alphabetic())
        && !s.chars().any(|c| c.is_lowercase())
}

struct P<'a> {
    d: &'a Dialeto,
    t: &'a [Tok],
    com: &'a [Comentario],
    tsx: bool,
    total: u32,
    i: usize,
    simbolos: Vec<Simbolo>,
    /// Índices dos símbolos de endpoint (não contam como origem de menção).
    endpoints: HashSet<usize>,
    refs: Vec<Referencia>,
    /// Constantes de string do arquivo (URLs base do TypeScript).
    consts: HashMap<String, String>,
    /// Nomes importados de bibliotecas.
    externos: HashSet<String>,
    /// Tokens cujas strings não geram menção (importações, anotações).
    sem_mencao: Vec<bool>,
}

impl<'a> P<'a> {
    // -- Acesso aos tokens ------------------------------------------------

    fn n(&self) -> usize {
        self.t.len()
    }

    /// O token `k` é a palavra ou o operador `x` (nunca uma string).
    fn e(&self, k: usize, x: &str) -> bool {
        self.t
            .get(k)
            .is_some_and(|t| matches!(t.k, K::Ident | K::Op) && t.s == x)
    }

    fn ident(&self, k: usize) -> bool {
        self.t.get(k).is_some_and(|t| t.k == K::Ident)
    }

    fn s(&self, k: usize) -> &str {
        self.t.get(k).map(|t| t.s.as_str()).unwrap_or("")
    }

    fn linha(&self, k: usize) -> u32 {
        self.t
            .get(k)
            .or(self.t.last())
            .map(|t| t.linha)
            .unwrap_or(1)
    }

    fn linha_fim(&self, k: usize) -> u32 {
        self.t.get(k).or(self.t.last()).map(|t| t.fim).unwrap_or(1)
    }

    fn ts(&self) -> bool {
        self.d.ling == Ling::TypeScript
    }

    /// Índice do fechamento do `(`, `[` ou `{` em `k` (ou `n`).
    fn fecho(&self, k: usize) -> usize {
        let mut prof = 0i32;
        let mut j = k;
        while j < self.n() {
            let t = &self.t[j];
            if t.k == K::Op {
                match t.s.as_str() {
                    "(" | "[" | "{" => prof += 1,
                    ")" | "]" | "}" => {
                        prof -= 1;
                        if prof <= 0 {
                            return j;
                        }
                    }
                    _ => {}
                }
            }
            j += 1;
        }
        self.n()
    }

    /// Depois de `<` em `k`, a posição seguinte ao `>` que fecha, se o trecho
    /// tiver cara de argumentos de tipo.
    fn pular_generico(&self, k: usize) -> Option<usize> {
        if !self.e(k, "<") {
            return None;
        }
        let mut prof = 0;
        let mut j = k;
        while j < self.n() && j < k + 64 {
            let t = &self.t[j];
            match (t.k, t.s.as_str()) {
                (K::Op, "<") => prof += 1,
                (K::Op, ">") => {
                    prof -= 1;
                    if prof == 0 {
                        return Some(j + 1);
                    }
                }
                (K::Ident, _) => {}
                (K::Op, "." | "," | "[" | "]" | "?" | "|" | "&" | "*") => {}
                (K::Texto, _) if self.ts() => {}
                _ => return None,
            }
            j += 1;
        }
        None
    }

    /// Cadeia `a.b.C` a partir de `k`: (nome, posição seguinte).
    fn cadeia(&self, k: usize) -> Option<(String, usize)> {
        if !self.ident(k) {
            return None;
        }
        let mut nome = self.s(k).to_string();
        let mut j = k + 1;
        while (self.e(j, ".") || self.e(j, "::")) && self.ident(j + 1) {
            nome.push('.');
            nome.push_str(self.s(j + 1));
            j += 2;
        }
        Some((nome, j))
    }

    /// Fim de instrução a partir de `k`: posição depois do `;`, ou do último
    /// token quando a instrução termina sem `;` (TypeScript), ou do `}` que fecha
    /// o bloco de fora (sem consumi-lo).
    fn fim_instrucao(&self, k: usize) -> usize {
        let mut prof = 0i32;
        let mut j = k;
        while j < self.n() {
            let t = &self.t[j];
            if t.k == K::Op {
                match t.s.as_str() {
                    "(" | "[" | "{" => prof += 1,
                    ")" | "]" | "}" => {
                        if prof == 0 {
                            return j;
                        }
                        prof -= 1;
                        if prof == 0 && t.s == "}" && self.quebra(j) {
                            return j + 1;
                        }
                    }
                    ";" if prof == 0 => return j + 1,
                    _ => {}
                }
            }
            if prof == 0 && self.ts() && self.asi(j) {
                return j + 1;
            }
            j += 1;
        }
        self.n()
    }

    /// Depois de um `}` que fecha um bloco: o próximo token está noutra linha e
    /// não continua a expressão.
    fn quebra(&self, j: usize) -> bool {
        let Some(b) = self.t.get(j + 1) else {
            return true;
        };
        b.linha > self.t[j].fim && !continua(b)
    }

    /// Inserção automática de `;` do TypeScript entre `j` e `j + 1`.
    fn asi(&self, j: usize) -> bool {
        let (Some(a), Some(b)) = (self.t.get(j), self.t.get(j + 1)) else {
            return false;
        };
        if b.linha <= a.fim {
            return false;
        }
        let a_fecha = match a.k {
            K::Op => matches!(a.s.as_str(), ")" | "]" | "}" | "++" | "--" | "!"),
            _ => true,
        };
        a_fecha && !continua(b)
    }

    // -- Blocos e membros -------------------------------------------------

    fn bloco(&mut self, ctx: &mut Ctx) {
        loop {
            if self.i >= self.n() {
                return;
            }
            if self.e(self.i, "}") {
                if ctx.corpo == Corpo::Arquivo {
                    self.i += 1;
                    continue;
                }
                return;
            }
            if self.e(self.i, ";") || self.e(self.i, ",") {
                self.i += 1;
                continue;
            }
            let antes = self.i;
            self.membro(ctx);
            if self.i <= antes {
                self.i = antes + 1;
            }
        }
    }

    fn membro(&mut self, ctx: &mut Ctx) {
        let ini = self.i;
        let anots = self.anotacoes();
        let k0 = self.i;
        let mods = self.modificadores();
        let k = self.i;
        let l = self.d.ling;
        let s = self.s(k).to_string();

        if l == Ling::Java && s == "package" && self.ident(k + 1) {
            self.pacote(ini, ctx);
            return;
        }
        if (l != Ling::CSharp && s == "import" && !self.e(k + 1, "(") && !self.e(k + 1, "."))
            || (l == Ling::CSharp && s == "using" && self.eh_using(k, ctx))
        {
            self.importacao(k);
            return;
        }
        if self.e(k, "@") && self.e(k + 1, "interface") {
            self.i = k + 1;
            self.tipo(ini, &anots, ctx);
            return;
        }
        let tipo_kw = match s.as_str() {
            "class" | "interface" | "enum" => self.ident(k + 1),
            "struct" => l == Ling::CSharp && self.ident(k + 1),
            "record" => {
                l != Ling::TypeScript
                    && (self.ident(k + 1) && !self.e(k + 1, "struct") && !self.e(k + 1, "class")
                        || (self.e(k + 1, "struct") || self.e(k + 1, "class")) && self.ident(k + 2))
                    && !self.e(k + 2, ";")
            }
            "type" => l == Ling::TypeScript && self.ident(k + 1) && !self.e(k + 1, "of"),
            _ => false,
        };
        if tipo_kw {
            self.tipo(ini, &anots, ctx);
            return;
        }
        if (s == "namespace" && l != Ling::Java && (self.ident(k + 1)))
            || (s == "module"
                && l == Ling::TypeScript
                && (self.ident(k + 1) || self.t.get(k + 1).is_some_and(|t| t.k == K::Texto)))
        {
            self.namespace(ini, ctx);
            return;
        }
        // Bloco de inicialização (Java: `static { }` e `{ }` no corpo da classe).
        if self.e(k, "{") && matches!(ctx.corpo, Corpo::Classe | Corpo::Enum) {
            let f = self.fecho(k);
            let origem = ctx.qual.clone();
            let mut vars = ctx.vars.clone();
            self.refs(k + 1, f, origem.as_deref(), &mut vars);
            self.i = (f + 1).min(self.n());
            return;
        }
        self.generico(ini, k0, &anots, &mods, ctx);
    }

    /// `using` de importação (e não o `using` de instrução do C#).
    fn eh_using(&self, k: usize, ctx: &Ctx) -> bool {
        if !matches!(ctx.corpo, Corpo::Arquivo | Corpo::Namespace) || self.e(k + 1, "(") {
            return false;
        }
        if self.e(k + 1, "static") || self.e(k + 1, "global") {
            return true;
        }
        // `using var x = ...` e `using Tipo x = ...` são instruções.
        !(self.e(k + 1, "var") || self.ident(k + 1) && self.ident(k + 2))
    }

    /// Anotações (`@X(...)`), atributos (`[X(...)]`) e decorators.
    fn anotacoes(&mut self) -> Vec<Anotacao> {
        let mut v = Vec::new();
        loop {
            let i = self.i;
            if self.d.ling != Ling::CSharp
                && self.e(i, "@")
                && self.ident(i + 1)
                && !self.e(i + 1, "interface")
            {
                let Some((nome, mut j)) = self.cadeia(i + 1) else {
                    break;
                };
                let mut a = Anotacao {
                    nome: nome.rsplit('.').next().unwrap_or(&nome).to_string(),
                    ..Anotacao::default()
                };
                if self.e(j, "(") {
                    let f = self.fecho(j);
                    self.args(j, f, &mut a);
                    j = f + 1;
                }
                self.calar(i, j);
                v.push(a);
                self.i = j.min(self.n());
                continue;
            }
            if self.d.ling == Ling::CSharp && self.e(i, "[") {
                let f = self.fecho(i);
                let mut j = i + 1;
                while j < f {
                    // Alvo: `[assembly: X]`, `[return: X]`.
                    if self.ident(j) && self.e(j + 1, ":") {
                        j += 2;
                    }
                    let Some((nome, mut x)) = self.cadeia(j) else {
                        j += 1;
                        continue;
                    };
                    let nome = nome.rsplit('.').next().unwrap_or(&nome);
                    let mut a = Anotacao {
                        nome: nome.strip_suffix("Attribute").unwrap_or(nome).to_string(),
                        ..Anotacao::default()
                    };
                    if self.e(x, "(") {
                        let g = self.fecho(x);
                        self.args(x, g, &mut a);
                        x = g + 1;
                    }
                    v.push(a);
                    j = x;
                    while j < f && !self.e(j, ",") {
                        j += 1;
                    }
                    j += 1;
                }
                self.calar(i, f + 1);
                self.i = (f + 1).min(self.n());
                continue;
            }
            break;
        }
        v
    }

    fn calar(&mut self, a: usize, b: usize) {
        let b = b.min(self.sem_mencao.len());
        for x in a.min(b)..b {
            self.sem_mencao[x] = true;
        }
    }

    /// Argumentos de anotação entre `(` em `a` e `)` em `f`.
    fn args(&self, a: usize, f: usize, anot: &mut Anotacao) {
        let mut j = a + 1;
        while j < f {
            let mut arg = Arg::default();
            if self.ident(j) && (self.e(j + 1, "=") || self.e(j + 1, ":")) {
                arg.nome = Some(self.s(j).to_string());
            }
            let mut prof = 0;
            while j < f {
                let t = &self.t[j];
                if t.k == K::Op {
                    match t.s.as_str() {
                        "(" | "[" | "{" => prof += 1,
                        ")" | "]" | "}" => prof -= 1,
                        "," if prof == 0 => break,
                        _ => {}
                    }
                }
                if t.k == K::Texto {
                    arg.textos.push(t.s.clone());
                    if j >= 2 && self.ident(j - 2) && (self.e(j - 1, "=") || self.e(j - 1, ":")) {
                        anot.pares.push((self.s(j - 2).to_string(), t.s.clone()));
                    }
                }
                arg.bruto.push_str(&t.s);
                j += 1;
            }
            anot.args.push(arg);
            j += 1;
        }
    }

    fn modificadores(&mut self) -> Vec<String> {
        let mut v = Vec::new();
        loop {
            let k = self.i;
            let s = self.s(k);
            if !self.ident(k) || !MODIFICADORES.contains(&s) {
                break;
            }
            // `non-sealed` do Java.
            if s == "non" {
                if self.e(k + 1, "-") && self.e(k + 2, "sealed") {
                    v.push("non-sealed".into());
                    self.i += 3;
                    continue;
                }
                break;
            }
            // `new` só é modificador no C#; no resto, o modificador precisa vir
            // antes de outro nome (senão é o próprio nome: `get()`).
            let seguinte_ok = self.ident(k + 1)
                || self.e(k + 1, "<")
                || self.e(k + 1, "*")
                || self.e(k + 1, "[") && self.d.ling != Ling::CSharp
                || self.e(k + 1, "{") && s == "static"
                || self.e(k + 1, "#")
                || self.e(k + 1, "@") && self.d.ling != Ling::CSharp;
            if !seguinte_ok {
                break;
            }
            v.push(s.to_string());
            self.i += 1;
        }
        // `new` como modificador (C#: `public new void F()`).
        if self.d.ling == Ling::CSharp && self.e(self.i, "new") && self.ident(self.i + 1) {
            self.i += 1;
            v.push("new".into());
            v.extend(self.modificadores());
        }
        v
    }

    // -- Declarações ------------------------------------------------------

    fn simbolo(
        &mut self,
        nome: &str,
        tipo: TipoSimbolo,
        ini: usize,
        fim: usize,
        ctx: &Ctx,
    ) -> usize {
        let l_ini = self.linha(ini);
        let mut s = Simbolo::novo(nome, tipo, Linhas::new(l_ini, self.linha_fim(fim)));
        if let Some(p) = &ctx.qual {
            s = s.dentro_de(p);
        }
        s.comentario = self.doc(l_ini);
        self.simbolos.push(s);
        self.simbolos.len() - 1
    }

    fn fechar(&mut self, idx: usize, fim: usize) {
        let l = self.linha_fim(fim.min(self.n().saturating_sub(1)));
        let s = &mut self.simbolos[idx];
        s.linhas = Linhas::new(s.linhas.inicio, l);
    }

    fn referencia(&mut self, origem: Option<&str>, alvo: &str, tipo: TipoReferencia, linha: u32) {
        if alvo.is_empty() {
            return;
        }
        if tipo != TipoReferencia::Importacao {
            let primeiro = alvo.split('.').next().unwrap_or(alvo);
            if plataforma(alvo) || self.externos.contains(primeiro) {
                return;
            }
        }
        self.refs.push(Referencia {
            origem: origem.map(str::to_string),
            alvo: alvo.to_string(),
            tipo,
            linha,
        });
    }

    fn pacote(&mut self, ini: usize, ctx: &mut Ctx) {
        let Some((nome, j)) = self.cadeia(ini + 1) else {
            self.i = ini + 1;
            return;
        };
        let simples = nome.rsplit('.').next().unwrap_or(&nome).to_string();
        let mut s = Simbolo::novo(
            simples,
            TipoSimbolo::Namespace,
            Linhas::new(self.linha(ini), self.total),
        );
        s.qualificado = nome.clone();
        s.comentario = self.doc(self.linha(ini));
        self.simbolos.push(s);
        self.calar(ini, j);
        ctx.qual = Some(nome);
        self.i = if self.e(j, ";") { j + 1 } else { j };
    }

    fn importacao(&mut self, k: usize) {
        let fim = self.fim_instrucao(k);
        self.calar(k, fim);
        self.i = fim.max(k + 1);
        let linha = self.linha(k);
        match self.d.ling {
            Ling::Java | Ling::CSharp => {
                let mut j = k + 1;
                while self.e(j, "static") || self.e(j, "global") {
                    j += 1;
                }
                // `using Alias = X.Y;`
                if self.d.ling == Ling::CSharp && self.ident(j) && self.e(j + 1, "=") {
                    j += 2;
                }
                if self.e(j, "global") && self.e(j + 1, "::") {
                    j += 2;
                }
                if let Some((nome, _)) = self.cadeia(j) {
                    if self.d.ling == Ling::Java && (self.d.externo)(&nome) {
                        let simples = nome.rsplit('.').next().unwrap_or(&nome);
                        self.externos.insert(simples.to_string());
                    }
                    self.referencia(None, &nome, TipoReferencia::Importacao, linha);
                }
            }
            Ling::TypeScript => {
                // Módulo: a última string da instrução.
                let modulo = (k..fim)
                    .rev()
                    .find(|&x| self.t.get(x).is_some_and(|t| t.k == K::Texto))
                    .map(|x| self.s(x).to_string())
                    .unwrap_or_default();
                let externo = (self.d.externo)(&modulo);
                let mut nomes = Vec::new();
                let mut j = k + 1;
                let mut chaves = false;
                while j < fim {
                    let s = self.s(j);
                    if self.e(j, "{") {
                        chaves = true;
                    } else if self.e(j, "}") {
                        chaves = false;
                    } else if self.e(j, "from") {
                        break;
                    } else if self.ident(j)
                        && !matches!(s, "type" | "typeof" | "as" | "default")
                        && !self.e(j.wrapping_sub(1), "as")
                        && (chaves || !self.e(j.wrapping_sub(1), "*"))
                    {
                        // Em `A as B` vale o nome original; em `* as N`, o módulo.
                        if !(self.e(j.wrapping_sub(1), "as") || self.e(j.wrapping_sub(2), "*")) {
                            nomes.push((s.to_string(), self.e(j + 1, "as")));
                        }
                    }
                    j += 1;
                }
                if externo {
                    for (n, _) in &nomes {
                        self.externos.insert(n.clone());
                    }
                    // Também os apelidos (`A as B`, `* as N`).
                    for x in k..fim {
                        if self.e(x, "as") && self.ident(x + 1) {
                            self.externos.insert(self.s(x + 1).to_string());
                        }
                    }
                }
                if nomes.is_empty() || externo {
                    if !modulo.is_empty() {
                        self.referencia(None, &modulo, TipoReferencia::Importacao, linha);
                    }
                } else {
                    for (n, _) in nomes {
                        self.referencia(None, &n, TipoReferencia::Importacao, linha);
                    }
                }
            }
        }
    }

    fn namespace(&mut self, ini: usize, ctx: &mut Ctx) {
        let mut k = ini;
        while !self.e(k, "namespace") && !self.e(k, "module") {
            k += 1;
        }
        let (nome, mut j, tipo) = if let Some((nome, j)) = self.cadeia(k + 1) {
            (nome, j, TipoSimbolo::Namespace)
        } else {
            (self.s(k + 1).to_string(), k + 2, TipoSimbolo::Modulo)
        };
        let simples = nome.rsplit('.').next().unwrap_or(&nome).to_string();
        let qual = match &ctx.qual {
            Some(p) => format!("{p}.{nome}"),
            None => nome.clone(),
        };
        let mut s = Simbolo::novo(simples, tipo, Linhas::new(self.linha(ini), self.total));
        s.qualificado = qual.clone();
        s.pai = ctx.qual.clone();
        s.comentario = self.doc(self.linha(ini));
        self.simbolos.push(s);
        let idx = self.simbolos.len() - 1;
        while j < self.n() && !self.e(j, "{") && !self.e(j, ";") {
            j += 1;
        }
        let mut sub = Ctx {
            corpo: Corpo::Namespace,
            qual: Some(qual),
            ..Ctx::arquivo()
        };
        if self.e(j, "{") {
            self.i = j + 1;
            self.bloco(&mut sub);
            self.fechar(idx, self.i);
            self.i += 1;
        } else {
            // `namespace X;` vale até o fim do arquivo.
            self.i = j + 1;
            self.bloco(&mut sub);
        }
    }

    /// Classe, interface, enum, record, struct ou alias de tipo.
    fn tipo(&mut self, ini: usize, anots: &[Anotacao], ctx: &mut Ctx) {
        let k = self.i;
        let kw = self.s(k).to_string();
        let mut nk = k + 1;
        if kw == "record" && (self.e(nk, "struct") || self.e(nk, "class")) {
            nk += 1;
        }
        let nome = self.s(nk).to_string();
        let mut tipo = match kw.as_str() {
            "interface" => TipoSimbolo::Interface,
            "enum" => TipoSimbolo::Enum,
            "record" | "struct" => TipoSimbolo::Registro,
            "type" => TipoSimbolo::Tipo,
            _ => TipoSimbolo::Classe,
        };
        // Cabeçalho até `{`, `;` ou `=` (alias do TypeScript).
        let mut j = nk + 1;
        let mut comps: Option<(usize, usize)> = None;
        while j < self.n() {
            if self.e(j, "{") || self.e(j, ";") || (kw == "type" && self.e(j, "=")) {
                break;
            }
            if self.e(j, "(") {
                let f = self.fecho(j);
                if comps.is_none() {
                    comps = Some((j, f));
                }
                j = f + 1;
                continue;
            }
            if self.e(j, "}") {
                break;
            }
            j += 1;
        }
        let bases = self.bases(nk + 1, j, &kw);
        let nomes_bases: Vec<String> = bases.iter().map(|b| b.0.clone()).collect();
        let mut info = (self.d.classe)(anots, &nome, &nomes_bases);
        info.nome = nome.clone();
        if let Some(t) = info.tipo {
            tipo = t;
        }
        let idx = self.simbolo(&nome, tipo, ini, j, ctx);
        let qual = self.simbolos[idx].qualificado.clone();
        self.simbolos[idx].detalhe = info.detalhe.clone().or_else(|| {
            let nomes: Vec<_> = anots.iter().map(|a| format!("@{}", a.nome)).collect();
            (!nomes.is_empty()).then(|| nomes.join(" "))
        });
        for (b, tr, linha) in &bases {
            self.referencia(Some(&qual), b, *tr, *linha);
        }
        if let Some(tab) = &info.tabela {
            self.referencia(Some(&qual), tab, TipoReferencia::Uso, self.linha(nk));
        }

        if kw == "type" {
            let fim = self.fim_instrucao(j + 1);
            let ult = fim.saturating_sub(1).max(j);
            self.tipos(j + 1, fim, Some(&qual));
            self.fechar(idx, ult);
            self.i = fim;
            return;
        }

        let mut sub = Ctx {
            corpo: match tipo {
                TipoSimbolo::Interface => Corpo::Interface,
                TipoSimbolo::Enum => Corpo::Enum,
                _ => Corpo::Classe,
            },
            qual: Some(qual.clone()),
            classe: Some(nome.clone()),
            info,
            vars: HashMap::new(),
        };
        // Componentes do record (e construtor primário do C#).
        if let Some((a, f)) = comps {
            let comp_tipo = if self.d.ling == Ling::CSharp && kw == "record" {
                TipoSimbolo::Propriedade
            } else {
                TipoSimbolo::Campo
            };
            for p in self.parametros(a, f) {
                if kw == "record" {
                    let ci = self.simbolo(&p.nome, comp_tipo, p.ini, p.ini, &sub);
                    self.simbolos[ci].detalhe = Some(p.tipo_texto.clone());
                    let cq = self.simbolos[ci].qualificado.clone();
                    self.tipos(p.tipo_ini, p.tipo_fim, Some(&cq));
                } else {
                    self.tipos(p.tipo_ini, p.tipo_fim, Some(&qual));
                }
                if let Some(t) = p.tipo {
                    sub.vars.insert(p.nome.clone(), t);
                }
            }
        }
        if self.e(j, "{") {
            self.i = j + 1;
            if sub.corpo == Corpo::Enum {
                self.constantes(&sub);
            }
            self.bloco(&mut sub);
            self.fechar(idx, self.i);
            self.i += 1;
        } else {
            self.fechar(idx, j);
            self.i = j + 1;
        }
    }

    /// Bases de um tipo: (nome, herança ou implementação, linha).
    fn bases(&self, a: usize, b: usize, kw: &str) -> Vec<(String, TipoReferencia, u32)> {
        let mut v = Vec::new();
        let mut modo: Option<TipoReferencia> = None;
        let mut primeiro = true;
        let mut j = a;
        while j < b {
            let s = self.s(j);
            if self.ident(j) && s == "extends" {
                modo = Some(TipoReferencia::Heranca);
            } else if self.ident(j) && s == "implements" {
                modo = Some(TipoReferencia::Implementacao);
            } else if self.e(j, ":") && self.d.ling == Ling::CSharp {
                modo = Some(TipoReferencia::Heranca);
            } else if self.ident(j) && matches!(s, "where" | "permits") {
                modo = None;
            } else if self.e(j, "(") {
                j = self.fecho(j) + 1;
                continue;
            } else if let (Some(m), Some((nome, x))) = (modo, self.cadeia(j)) {
                let tr = if self.d.ling == Ling::CSharp && kw != "interface" {
                    // No C#, a primeira base de uma classe pode ser classe; as
                    // interfaces costumam começar com `I` maiúsculo.
                    let simples = nome.rsplit('.').next().unwrap_or(&nome);
                    let interface = simples.len() > 1
                        && simples.starts_with('I')
                        && simples[1..].starts_with(|c: char| c.is_uppercase());
                    if primeiro && !interface && kw == "class" {
                        TipoReferencia::Heranca
                    } else {
                        TipoReferencia::Implementacao
                    }
                } else {
                    m
                };
                primeiro = false;
                v.push((nome, tr, self.linha(j)));
                j = x;
                if let Some(g) = self.pular_generico(j) {
                    j = g;
                }
                continue;
            }
            j += 1;
        }
        v
    }

    /// Constantes no começo do corpo de um enum.
    fn constantes(&mut self, ctx: &Ctx) {
        loop {
            let ini = self.i;
            if ini >= self.n() || self.e(ini, "}") {
                return;
            }
            if self.e(ini, ";") {
                self.i += 1;
                return;
            }
            if self.e(ini, ",") {
                self.i += 1;
                continue;
            }
            let _ = self.anotacoes();
            let k = self.i;
            if !self.ident(k) {
                self.i = k + 1;
                continue;
            }
            // Um membro comum depois das constantes sem `;` (C#, TypeScript não
            // têm membros no enum; no Java o `;` é obrigatório).
            if self.ident(k + 1) || self.e(k + 1, "<") {
                return;
            }
            let nome = self.s(k).to_string();
            let idx = self.simbolo(&nome, TipoSimbolo::Constante, ini, k, ctx);
            let qual = self.simbolos[idx].qualificado.clone();
            let mut j = k + 1;
            let mut fim = k;
            while j < self.n() && !self.e(j, ",") && !self.e(j, ";") && !self.e(j, "}") {
                if self.e(j, "(") {
                    let f = self.fecho(j);
                    let mut vars = ctx.vars.clone();
                    self.refs(j + 1, f, Some(&qual), &mut vars);
                    j = f;
                } else if self.e(j, "{") {
                    // Corpo próprio da constante (Java): uma classe anônima.
                    let mut sub = Ctx {
                        corpo: Corpo::Classe,
                        qual: Some(qual.clone()),
                        classe: None,
                        info: InfoClasse::default(),
                        vars: ctx.vars.clone(),
                    };
                    self.i = j + 1;
                    self.bloco(&mut sub);
                    j = self.i.min(self.n());
                }
                fim = j;
                j += 1;
            }
            self.fechar(idx, fim);
            self.i = j;
        }
    }

    /// Membro ou instrução que não começa com palavra de tipo.
    fn generico(
        &mut self,
        ini: usize,
        k0: usize,
        anots: &[Anotacao],
        mods: &[String],
        ctx: &mut Ctx,
    ) {
        let k = self.i;
        let ts = self.ts();
        // Cabeçalho.
        let mut j = k;
        let mut ang = 0i32;
        let mut grupo: Option<(usize, usize)> = None;
        let fim = loop {
            if j >= self.n() {
                break Fim::Implicito;
            }
            let t = &self.t[j];
            if t.k == K::Op {
                match t.s.as_str() {
                    "(" | "[" => {
                        let f = self.fecho(j);
                        if t.s == "(" && grupo.is_none() && ang == 0 {
                            grupo = Some((j, f));
                        }
                        j = f + 1;
                        if ts && ang == 0 && self.asi_membro(j.saturating_sub(1)) {
                            break Fim::Implicito;
                        }
                        continue;
                    }
                    "<" if j > k && (self.ident(j - 1) || self.e(j - 1, ".")) => ang += 1,
                    "<" if j == k => ang += 1,
                    ">" if ang > 0 => ang -= 1,
                    "{" if ang == 0 => break Fim::Chave,
                    ";" if ang == 0 => break Fim::PontoVirgula,
                    "=" if ang == 0 => break Fim::Igual,
                    "=>" if ang == 0 => break Fim::Seta,
                    "}" | ")" | "]" => break Fim::Implicito,
                    _ => {}
                }
            }
            if ts && ang == 0 && self.asi_membro(j) {
                j += 1;
                break Fim::Implicito;
            }
            j += 1;
        };
        let tem = |x: &str| mods.iter().any(|m| m == x);
        let em_tipo = matches!(ctx.corpo, Corpo::Classe | Corpo::Interface | Corpo::Enum);

        // Rotina: nome antes do primeiro `(`.
        if let Some((pi, pf)) = grupo
            && fim != Fim::Igual
        {
            let mut nk = pi.wrapping_sub(1);
            if self.e(nk, ">") {
                // `foo<T>(`: volta até o `<`.
                let mut prof = 0;
                while nk > k {
                    if self.e(nk, ">") {
                        prof += 1;
                    } else if self.e(nk, "<") {
                        prof -= 1;
                        if prof == 0 {
                            break;
                        }
                    }
                    nk -= 1;
                }
                nk = nk.wrapping_sub(1);
            }
            let nome_ok = nk >= k
                && nk < self.n()
                && self.ident(nk)
                && !PALAVRAS.contains(&self.s(nk))
                && !(nk > 0
                    && (self.e(nk - 1, ".") || self.e(nk - 1, "?.") || self.e(nk - 1, "new")));
            let declara = match ctx.corpo {
                Corpo::Classe | Corpo::Interface | Corpo::Enum => true,
                Corpo::Arquivo | Corpo::Namespace => ts && tem("function"),
            } || (self.s(nk) == "constructor" && ts)
                || self.e(k, "delegate");
            if nome_ok && declara {
                self.rotina(ini, k, nk, (pi, pf), j, fim, anots, mods, ctx);
                return;
            }
        }

        // Campo, propriedade, constante, variável.
        if (em_tipo || ts && (tem("const") || tem("let") || tem("var")))
            && let Some((nk, tipo_ini, tipo_fim)) = self.nome_de_campo(k, j, fim)
        {
            self.campo(ini, nk, (tipo_ini, tipo_fim), j, fim, anots, mods, ctx);
            return;
        }

        // Instrução solta (TypeScript, top-level statements do C#).
        let fim_inst = match fim {
            Fim::Chave if em_tipo => self.fecho(j) + 1,
            _ => self.fim_instrucao(k0),
        };
        let origem = ctx.qual.clone().filter(|_| ctx.corpo != Corpo::Namespace);
        let mut vars = ctx.vars.clone();
        self.refs(k0, fim_inst, origem.as_deref(), &mut vars);
        self.i = fim_inst.max(k0 + 1);
    }

    /// Fim de membro sem `;` no TypeScript: `nome: string` e na linha seguinte
    /// outro membro.
    fn asi_membro(&self, j: usize) -> bool {
        let (Some(a), Some(b)) = (self.t.get(j), self.t.get(j + 1)) else {
            return false;
        };
        if b.linha <= a.fim {
            return false;
        }
        let a_fecha = match a.k {
            K::Op => matches!(a.s.as_str(), ")" | "]" | ">" | "!" | "?"),
            _ => true,
        };
        let b_abre = b.k == K::Ident && !matches!(b.s.as_str(), "as" | "satisfies" | "extends")
            || b.k == K::Op && matches!(b.s.as_str(), "@" | "#");
        a_fecha && b_abre
    }

    /// Nome de campo entre `k` e o fim do cabeçalho em `j`: (posição do nome,
    /// intervalo do tipo).
    fn nome_de_campo(&self, k: usize, j: usize, fim: Fim) -> Option<(usize, usize, usize)> {
        if self.ts() {
            let mut nk = k;
            if self.e(nk, "#") {
                nk += 1;
            }
            if !self.ident(nk) {
                return None;
            }
            let mut x = nk + 1;
            while self.e(x, "?") || self.e(x, "!") {
                x += 1;
            }
            if self.e(x, ":") {
                return Some((nk, x + 1, j));
            }
            return Some((nk, j, j));
        }
        // Java e C#: o nome é o último identificador antes do fim (ou da
        // primeira vírgula: `int a, b;`).
        let mut lim = j;
        let mut prof = 0;
        for x in k..j {
            match self.s(x) {
                "(" | "[" | "<" if self.t[x].k == K::Op => prof += 1,
                ")" | "]" | ">" if self.t[x].k == K::Op => prof -= 1,
                "," if prof == 0 && self.t[x].k == K::Op => {
                    lim = x;
                    break;
                }
                _ => {}
            }
        }
        let nk = lim.checked_sub(1)?;
        if nk < k || !self.ident(nk) || PALAVRAS.contains(&self.s(nk)) && self.s(nk) != "this" {
            return None;
        }
        // Precisa de tipo antes do nome (exceto o indexador `this[...]`).
        if nk == k && !matches!(fim, Fim::Chave | Fim::Seta) {
            return None;
        }
        Some((nk, k, nk))
    }

    #[allow(clippy::too_many_arguments)]
    fn campo(
        &mut self,
        ini: usize,
        nk: usize,
        (tipo_ini, tipo_fim): (usize, usize),
        j: usize,
        fim: Fim,
        anots: &[Anotacao],
        mods: &[String],
        ctx: &mut Ctx,
    ) {
        let nome = self.s(nk).to_string();
        let tem = |x: &str| mods.iter().any(|m| m == x);
        let l = self.d.ling;
        // Inicializador.
        let (ini_init, fim_init) = match fim {
            Fim::Igual | Fim::Seta => (j + 1, self.fim_instrucao(j + 1)),
            Fim::Chave => {
                let f = self.fecho(j);
                // Propriedade com inicializador: `{ get; set; } = valor;`
                if self.e(f + 1, "=") {
                    (j + 1, self.fim_instrucao(f + 2))
                } else {
                    (j + 1, f + 1)
                }
            }
            Fim::PontoVirgula => (j, j + 1),
            Fim::Implicito => (j, j),
        };
        let funcao = l == Ling::TypeScript && fim == Fim::Igual && self.eh_funcao(ini_init);
        let em_tipo = matches!(ctx.corpo, Corpo::Classe | Corpo::Interface | Corpo::Enum);
        let tipo = if funcao {
            if em_tipo {
                TipoSimbolo::Metodo
            } else if self.tsx && maiuscula(&nome) {
                TipoSimbolo::Tela
            } else {
                TipoSimbolo::Funcao
            }
        } else if l == Ling::CSharp && matches!(fim, Fim::Chave | Fim::Seta)
            || l == Ling::TypeScript && (ctx.corpo == Corpo::Interface || tem("get") || tem("set"))
        {
            TipoSimbolo::Propriedade
        } else if (l == Ling::Java
            && (tem("static") && tem("final") || ctx.corpo == Corpo::Interface))
            || (l == Ling::CSharp && (tem("const") || tem("static") && tem("readonly")))
            || (l == Ling::TypeScript && (tem("const") || tem("readonly")) && eh_constante(&nome))
        {
            TipoSimbolo::Constante
        } else if em_tipo {
            TipoSimbolo::Campo
        } else {
            TipoSimbolo::Variavel
        };
        let ult = fim_init.saturating_sub(1).max(nk);
        let idx = self.simbolo(&nome, tipo, ini, ult, ctx);
        let qual = self.simbolos[idx].qualificado.clone();
        if tipo_fim > tipo_ini {
            self.simbolos[idx].detalhe = Some(self.juntar(tipo_ini, tipo_fim));
        }
        self.tipos(tipo_ini, tipo_fim, Some(&qual));
        // Tipo da variável, para as chamadas pelo campo.
        let tipo_var = (tipo_fim > tipo_ini && !self.e(tipo_ini, "var"))
            .then(|| self.tipo_de_var(tipo_ini))
            .flatten()
            .or_else(|| self.tipo_do_inicializador(ini_init, fim_init));
        if let Some(t) = tipo_var {
            ctx.vars.insert(nome.clone(), t);
        }
        // Constante de string (URL base).
        if fim == Fim::Igual
            && fim_init > ini_init
            && self.t.get(ini_init).is_some_and(|t| t.k == K::Texto)
            && (fim_init == ini_init + 1 || self.e(ini_init + 1, ";") || self.asi(ini_init))
        {
            self.consts
                .insert(nome.clone(), self.s(ini_init).to_string());
        }
        // Coluna de entidade.
        if let Some(tab) = ctx.info.tabela.clone()
            && let Some(col) = (self.d.coluna)(anots)
        {
            let alvo = format!("{tab}.{col}");
            self.referencia(Some(&qual), &alvo, TipoReferencia::Uso, self.linha(nk));
        }
        if fim_init > ini_init {
            let mut vars = ctx.vars.clone();
            self.refs(ini_init, fim_init, Some(&qual), &mut vars);
        }
        self.fechar(idx, ult);
        self.i = fim_init.max(nk + 1);
    }

    /// O inicializador em `k` é uma função (`() => ...`, `function`).
    fn eh_funcao(&self, k: usize) -> bool {
        let mut k = k;
        if self.e(k, "async") {
            k += 1;
        }
        if self.e(k, "function") {
            return true;
        }
        if self.ident(k) && self.e(k + 1, "=>") {
            return true;
        }
        if self.e(k, "<")
            && let Some(g) = self.pular_generico(k)
        {
            k = g;
        }
        if self.e(k, "(") {
            let f = self.fecho(k);
            let mut x = f + 1;
            // Tipo de retorno: `(): Promise<void> =>`.
            while x < self.n() && x < f + 32 {
                if self.e(x, "=>") {
                    return true;
                }
                if self.e(x, "{") || self.e(x, ";") || self.e(x, "=") {
                    return false;
                }
                x += 1;
            }
        }
        false
    }

    /// Tipo declarado em `k`, para as chamadas pela variável: nada para tipos
    /// primitivos e vetores.
    fn tipo_de_var(&self, k: usize) -> Option<String> {
        let (tipo, mut x) = self.cadeia(k)?;
        if PALAVRAS.contains(&tipo.as_str()) {
            return None;
        }
        if let Some(g) = self.pular_generico(x) {
            x = g;
        }
        while self.e(x, "?") {
            x += 1;
        }
        (!self.e(x, "[")).then_some(tipo)
    }

    /// Tipo de `new X(...)` ou `inject(X)` no começo do inicializador.
    fn tipo_do_inicializador(&self, a: usize, b: usize) -> Option<String> {
        if a >= b {
            return None;
        }
        if self.e(a, "new") {
            return self.cadeia(a + 1).map(|c| c.0);
        }
        if self.e(a, "inject") && self.e(a + 1, "(") {
            return self.cadeia(a + 2).map(|c| c.0);
        }
        None
    }

    #[allow(clippy::too_many_arguments)]
    fn rotina(
        &mut self,
        ini: usize,
        k: usize,
        nk: usize,
        (pi, pf): (usize, usize),
        j: usize,
        fim: Fim,
        anots: &[Anotacao],
        mods: &[String],
        ctx: &mut Ctx,
    ) {
        let nome = self.s(nk).to_string();
        let tem = |x: &str| mods.iter().any(|m| m == x);
        let construtor =
            ctx.classe.as_deref() == Some(nome.as_str()) || (self.ts() && nome == "constructor");
        let tipo = if construtor {
            TipoSimbolo::Construtor
        } else if tem("delegate") || self.e(k, "delegate") {
            TipoSimbolo::Tipo
        } else if self.ts() && (tem("get") || tem("set")) {
            TipoSimbolo::Propriedade
        } else if matches!(ctx.corpo, Corpo::Arquivo | Corpo::Namespace) {
            if self.tsx && maiuscula(&nome) {
                TipoSimbolo::Tela
            } else {
                TipoSimbolo::Funcao
            }
        } else {
            TipoSimbolo::Metodo
        };
        // Corpo.
        let (corpo, fim_corpo) = match fim {
            Fim::Chave => {
                let f = self.fecho(j);
                (Some((j + 1, f)), f)
            }
            Fim::Seta => {
                let f = self.fim_instrucao(j + 1);
                (Some((j + 1, f)), f.saturating_sub(1))
            }
            Fim::PontoVirgula => (None, j),
            Fim::Igual | Fim::Implicito => (None, j.saturating_sub(1).max(pf)),
        };
        let idx = self.simbolo(&nome, tipo, ini, fim_corpo, ctx);
        let qual = self.simbolos[idx].qualificado.clone();
        self.simbolos[idx].declaracao = corpo.is_none()
            && !tem("extern")
            && !tem("partial")
            && !tem("native")
            && tipo != TipoSimbolo::Tipo;
        // Assinatura: do primeiro token depois dos modificadores até o fim do
        // cabeçalho.
        self.simbolos[idx].detalhe = Some(self.juntar(k, j.min(self.n())));
        // Tipos de retorno e dos parâmetros.
        let mut vars = ctx.vars.clone();
        if self.ts() {
            if self.e(pf + 1, ":") {
                self.tipos(pf + 2, j, Some(&qual));
            }
        } else {
            let mut a = k;
            if let Some(g) = self.pular_generico(a) {
                a = g;
            }
            self.tipos(a, nk, Some(&qual));
            // `throws X` não é tipo de retorno; só o que vem antes do nome conta.
        }
        for p in self.parametros(pi, pf) {
            self.tipos(p.tipo_ini, p.tipo_fim, Some(&qual));
            if let Some(t) = &p.tipo {
                vars.insert(p.nome.clone(), t.clone());
                if p.propriedade {
                    ctx.vars.insert(p.nome.clone(), t.clone());
                }
            }
        }
        if let Some((a, b)) = corpo {
            self.refs(a, b, Some(&qual), &mut vars);
        }
        self.fechar(idx, fim_corpo);
        // Endpoints.
        if ctx.info.rotas.is_some() || !anots.is_empty() {
            let linhas = self.simbolos[idx].linhas;
            for (verbo, caminho) in (self.d.rotas)(anots, &nome, &ctx.info) {
                if let Some(chave) = rota(&caminho) {
                    let mut e = Simbolo::novo(chave, TipoSimbolo::Outro, linhas).dentro_de(&qual);
                    e.detalhe = Some(
                        format!("endpoint {verbo} {}", barra(&caminho))
                            .split_whitespace()
                            .collect::<Vec<_>>()
                            .join(" "),
                    );
                    self.simbolos.push(e);
                    self.endpoints.insert(self.simbolos.len() - 1);
                }
            }
        }
        self.i = match fim {
            Fim::Chave => fim_corpo + 1,
            Fim::Seta => self.fim_instrucao(j + 1),
            Fim::PontoVirgula => j + 1,
            Fim::Igual | Fim::Implicito => j.max(pf + 1),
        };
    }

    /// Parâmetros entre `(` em `a` e `)` em `f`.
    fn parametros(&self, a: usize, f: usize) -> Vec<Param> {
        let mut v = Vec::new();
        let mut j = a + 1;
        while j < f {
            // Um parâmetro: até a vírgula de profundidade zero.
            let ini = j;
            let mut prof = 0;
            let mut fimp = j;
            while fimp < f {
                let t = &self.t[fimp];
                if t.k == K::Op {
                    match t.s.as_str() {
                        "(" | "[" | "{" | "<" => prof += 1,
                        ")" | "]" | "}" | ">" => prof -= 1,
                        "," if prof <= 0 => break,
                        _ => {}
                    }
                }
                fimp += 1;
            }
            if let Some(p) = self.parametro(ini, fimp) {
                v.push(p);
            }
            j = fimp + 1;
        }
        v
    }

    fn parametro(&self, a: usize, b: usize) -> Option<Param> {
        let mut x = a;
        let mut propriedade = false;
        // Anotações e atributos.
        loop {
            if self.e(x, "@") && self.ident(x + 1) {
                x = self.cadeia(x + 1).map(|c| c.1).unwrap_or(x + 2);
                if self.e(x, "(") {
                    x = self.fecho(x) + 1;
                }
                continue;
            }
            if self.d.ling == Ling::CSharp && self.e(x, "[") {
                x = self.fecho(x) + 1;
                continue;
            }
            if self.ident(x)
                && matches!(
                    self.s(x),
                    "final"
                        | "this"
                        | "ref"
                        | "out"
                        | "in"
                        | "params"
                        | "readonly"
                        | "public"
                        | "private"
                        | "protected"
                        | "override"
                        | "scoped"
                )
                && (self.ident(x + 1) || self.e(x + 1, "@") || self.e(x + 1, "["))
            {
                if matches!(self.s(x), "public" | "private" | "protected" | "readonly") {
                    propriedade = true;
                }
                x += 1;
                continue;
            }
            break;
        }
        if x >= b {
            return None;
        }
        if self.ts() {
            if !self.ident(x) {
                return None;
            }
            let nome = self.s(x).to_string();
            let mut y = x + 1;
            while self.e(y, "?") {
                y += 1;
            }
            let (ti, tf) = if self.e(y, ":") {
                let mut z = y + 1;
                while z < b && !self.e(z, "=") {
                    z += 1;
                }
                (y + 1, z)
            } else {
                (y, y)
            };
            let tipo = (tf > ti).then(|| self.tipo_de_var(ti)).flatten();
            return Some(Param {
                nome,
                tipo,
                tipo_ini: ti,
                tipo_fim: tf,
                tipo_texto: self.juntar(ti, tf),
                ini: x,
                propriedade,
            });
        }
        // Java e C#: o nome é o último identificador antes de `=` (valor padrão).
        let mut z = x;
        while z < b && !self.e(z, "=") {
            z += 1;
        }
        let nk = z.checked_sub(1)?;
        if nk <= x || !self.ident(nk) {
            return None;
        }
        Some(Param {
            nome: self.s(nk).to_string(),
            tipo: self.tipo_de_var(x),
            tipo_ini: x,
            tipo_fim: nk,
            tipo_texto: self.juntar(x, nk),
            ini: x,
            propriedade,
        })
    }

    /// Referências de tipo nos tokens `[a, b)`.
    fn tipos(&mut self, a: usize, b: usize, origem: Option<&str>) {
        let mut j = a;
        while j < b.min(self.n()) {
            if self.e(j, "@") {
                // Anotação de tipo (`@NonNull`).
                j += 2;
                continue;
            }
            if let Some((nome, x)) = self.cadeia(j) {
                let ultimo = nome.rsplit('.').next().unwrap_or(&nome);
                let ok = maiuscula(&nome) || maiuscula(ultimo) && !nome.starts_with("this.");
                if ok
                    && nome.len() > 1
                    && !PALAVRAS.contains(&nome.as_str())
                    && !(j > 0 && self.e(j - 1, "."))
                {
                    self.referencia(origem, &nome, TipoReferencia::Tipo, self.linha(j));
                }
                j = x;
                continue;
            }
            j += 1;
        }
    }

    // -- Referências nos corpos ------------------------------------------

    fn refs(
        &mut self,
        a: usize,
        b: usize,
        origem: Option<&str>,
        vars: &mut HashMap<String, String>,
    ) {
        let b = b.min(self.n());
        let mut k = a;
        while k < b {
            if !self.ident(k) {
                k += 1;
                continue;
            }
            let s = self.s(k).to_string();
            let linha = self.linha(k);
            if s == "new" {
                if let Some((tipo, mut x)) = self.cadeia(k + 1) {
                    if let Some(g) = self.pular_generico(x) {
                        x = g;
                    }
                    if (self.e(x, "(") || self.e(x, "{") || self.e(x, "["))
                        && maiuscula(tipo.rsplit('.').next().unwrap_or(&tipo))
                    {
                        self.referencia(origem, &tipo, TipoReferencia::Instanciacao, linha);
                    }
                    k = x;
                    continue;
                }
                k += 1;
                continue;
            }
            if let Some((nome, tipo)) = self.declaracao_local(k) {
                vars.insert(nome, tipo);
            }
            // Chamada.
            let mut abre = k + 1;
            if let Some(g) = self.pular_generico(k + 1)
                && self.e(g, "(")
            {
                abre = g;
            }
            let apos_ponto = k > 0 && (self.e(k - 1, ".") || self.e(k - 1, "?."));
            if self.e(abre, "(") && (apos_ponto || !PALAVRAS.contains(&s.as_str())) {
                self.chamada(k, abre, origem, vars);
            }
            // Constante de outra classe: `TipoCliente.PUB`.
            if maiuscula(&s)
                && !(k > 0 && (self.e(k - 1, ".") || self.e(k - 1, "?.")))
                && self.e(k + 1, ".")
                && self.ident(k + 2)
                && eh_constante(self.s(k + 2))
                && !self.e(k + 3, "(")
            {
                let alvo = format!("{s}.{}", self.s(k + 2));
                self.referencia(origem, &alvo, TipoReferencia::Uso, linha);
            }
            // Referência de método do Java: `Classe::metodo`.
            if maiuscula(&s) && self.e(k + 1, "::") && self.ident(k + 2) && self.s(k + 2) != "new" {
                let alvo = format!("{s}.{}", self.s(k + 2));
                self.referencia(origem, &alvo, TipoReferencia::Chamada, linha);
            }
            k += 1;
        }
    }

    /// Declaração de variável local em `k`: (nome, tipo).
    fn declaracao_local(&self, k: usize) -> Option<(String, String)> {
        if self.ts() {
            if !matches!(self.s(k), "const" | "let" | "var") || !self.ident(k + 1) {
                return None;
            }
            let nome = self.s(k + 1).to_string();
            if self.e(k + 2, ":") {
                return self.tipo_de_var(k + 3).map(|t| (nome, t));
            }
            if self.e(k + 2, "=") {
                return self
                    .tipo_do_inicializador(k + 3, self.n())
                    .map(|t| (nome, t));
            }
            return None;
        }
        if k > 0
            && (self.e(k - 1, ".")
                || self.ident(k - 1)
                    && !matches!(
                        self.s(k - 1),
                        "final" | "out" | "ref" | "using" | "await" | "scoped" | "readonly"
                    ))
        {
            return None;
        }
        if PALAVRAS.contains(&self.s(k)) && self.s(k) != "var" {
            return None;
        }
        let (tipo, mut x) = self.cadeia(k)?;
        if let Some(g) = self.pular_generico(x) {
            x = g;
        }
        while self.e(x, "?") {
            x += 1;
        }
        if self.e(x, "[") {
            // Vetor: o tipo do elemento não serve para as chamadas.
            return None;
        }
        if !self.ident(x) || PALAVRAS.contains(&self.s(x)) {
            return None;
        }
        let seguinte_ok = matches!(self.s(x + 1), "=" | ";" | ":" | "," | ")" | "in")
            && self.t.get(x + 1).is_some_and(|t| t.k != K::Texto);
        if !seguinte_ok {
            return None;
        }
        let nome = self.s(x).to_string();
        if tipo == "var" {
            if self.e(x + 1, "=") {
                return self
                    .tipo_do_inicializador(x + 2, self.n())
                    .map(|t| (nome, t));
            }
            return None;
        }
        Some((nome, tipo))
    }

    fn chamada(
        &mut self,
        k: usize,
        abre: usize,
        origem: Option<&str>,
        vars: &HashMap<String, String>,
    ) {
        let nome = self.s(k).to_string();
        let linha = self.linha(k);
        let ponto = k >= 1 && (self.e(k - 1, ".") || self.e(k - 1, "?."));
        // Receptor imediato e a cadeia de nomes antes dele.
        let (receptor, cadeia) = if ponto && k >= 2 && self.ident(k - 2) {
            let r = self.s(k - 2).to_string();
            let mut ini = k - 2;
            while ini >= 2 && self.e(ini - 1, ".") && self.ident(ini - 2) {
                ini -= 2;
            }
            let cadeia: Vec<&str> = (ini..k - 1).step_by(2).map(|x| self.s(x)).collect();
            (Some(r), cadeia.join("."))
        } else {
            (None, String::new())
        };
        let tipo_rec = receptor.as_deref().and_then(|r| vars.get(r)).cloned();
        if let Some(h) = (self.d.http)(receptor.as_deref(), tipo_rec.as_deref(), &nome)
            && (receptor.is_some() || !ponto)
        {
            let f = self.fecho(abre);
            // A URL não é menção.
            self.calar(abre, f);
            match h {
                Http::Chamada => {
                    if let Some(url) = self.url(abre + 1, f)
                        && let Some(chave) = rota(&url)
                    {
                        self.refs.push(Referencia {
                            origem: origem.map(str::to_string),
                            alvo: chave,
                            tipo: TipoReferencia::Chamada,
                            linha,
                        });
                    }
                }
                Http::Rota(verbo) => {
                    if let Some(url) = self.url(abre + 1, f)
                        && let Some(chave) = rota(&url)
                    {
                        let mut e = Simbolo::novo(
                            chave,
                            TipoSimbolo::Outro,
                            Linhas::new(linha, self.linha_fim(f)),
                        );
                        if let Some(o) = origem {
                            e = e.dentro_de(o);
                        }
                        e.detalhe = Some(format!("endpoint {verbo} {}", barra(&url)));
                        self.simbolos.push(e);
                        self.endpoints.insert(self.simbolos.len() - 1);
                    }
                }
            }
            return;
        }
        if ponto && receptor.is_none() {
            return;
        }
        let alvo = match receptor.as_deref() {
            None => nome.clone(),
            Some("this" | "base" | "super") if cadeia == receptor.clone().unwrap_or_default() => {
                nome.clone()
            }
            Some(_) => {
                if let Some(t) = &tipo_rec {
                    format!("{t}.{nome}")
                } else if maiuscula(&cadeia)
                    || cadeia.split('.').any(maiuscula) && !cadeia.starts_with("this.")
                {
                    // Chamada estática: `TetoEncargosRegra.limitar`.
                    let partes: Vec<&str> = cadeia.split('.').collect();
                    let desde = partes.iter().position(|p| maiuscula(p)).unwrap_or(0);
                    format!("{}.{nome}", partes[desde..].join("."))
                } else {
                    return;
                }
            }
        };
        self.referencia(origem, &alvo, TipoReferencia::Chamada, linha);
    }

    /// URL do primeiro argumento de uma chamada, entre `a` e `f`: strings,
    /// concatenação e template; o que não é texto vira `{}`. Num objeto de
    /// configuração, o campo `url`.
    fn url(&self, a: usize, f: usize) -> Option<String> {
        if self.e(a, "{") {
            let g = self.fecho(a);
            for x in a..g {
                if (self.e(x, "url") || self.e(x, "uri")) && self.e(x + 1, ":") {
                    let fim = (x + 2..g)
                        .find(|&y| self.e(y, ",") || self.e(y, "}"))
                        .unwrap_or(g);
                    return self.url(x + 2, fim);
                }
            }
            return None;
        }
        let mut partes: Vec<String> = Vec::new();
        let mut operando: Vec<usize> = Vec::new();
        let mut prof = 0;
        let mut tem_texto = false;
        let mut x = a;
        let fechar = |p: &Self, op: &mut Vec<usize>, partes: &mut Vec<String>, tem: &mut bool| {
            if op.is_empty() {
                return;
            }
            let t = &p.t[op[0]];
            let parte = if op.len() == 1 && t.k == K::Texto {
                *tem = true;
                if t.interp {
                    p.substituir(&t.s)
                } else {
                    t.s.clone()
                }
            } else if op.len() == 1 && t.k == K::Ident && p.consts.contains_key(&t.s) {
                *tem = true;
                p.consts[&t.s].clone()
            } else {
                "{}".to_string()
            };
            partes.push(parte);
            op.clear();
        };
        while x < f {
            let t = &self.t[x];
            if t.k == K::Op {
                match t.s.as_str() {
                    "(" | "[" | "{" => prof += 1,
                    ")" | "]" | "}" => prof -= 1,
                    "," if prof == 0 => break,
                    "+" if prof == 0 => {
                        fechar(self, &mut operando, &mut partes, &mut tem_texto);
                        x += 1;
                        continue;
                    }
                    _ => {}
                }
            }
            operando.push(x);
            x += 1;
        }
        fechar(self, &mut operando, &mut partes, &mut tem_texto);
        tem_texto.then(|| partes.concat())
    }

    /// Troca `${CONST}` (TypeScript) ou `{CONST}` (C#) pelo valor da constante;
    /// as demais interpolações viram `{}`.
    fn substituir(&self, s: &str) -> String {
        let mut out = String::new();
        let c: Vec<char> = s.chars().collect();
        let mut i = 0;
        while i < c.len() {
            let dolar = c[i] == '$' && c.get(i + 1) == Some(&'{');
            if dolar || c[i] == '{' && c.get(i + 1) != Some(&'{') {
                let ini = if dolar { i + 2 } else { i + 1 };
                let mut prof = 1;
                let mut j = ini;
                while j < c.len() {
                    match c[j] {
                        '{' => prof += 1,
                        '}' => {
                            prof -= 1;
                            if prof == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                    j += 1;
                }
                let expr: String = c[ini..j.min(c.len())].iter().collect();
                let expr = expr.trim();
                match self.consts.get(expr) {
                    Some(v) => out.push_str(v),
                    None => out.push_str("{}"),
                }
                i = j + 1;
                continue;
            }
            out.push(c[i]);
            i += 1;
        }
        out
    }

    // -- Saída -------------------------------------------------------------

    /// Comentário de documentação que termina na linha antes de `linha`.
    fn doc(&self, linha: u32) -> Option<String> {
        let ate = self.com.partition_point(|c| c.ini < linha);
        let mut alvo = linha.checked_sub(1)?;
        let mut partes = Vec::new();
        for c in self.com[..ate].iter().rev() {
            if c.fim != alvo || !c.sozinho {
                break;
            }
            partes.push(c.texto.as_str());
            alvo = match c.ini.checked_sub(1) {
                Some(x) => x,
                None => break,
            };
        }
        partes.reverse();
        let texto = limpar_comentario(&partes.join("\n"));
        (!texto.is_empty()).then(|| resumir(&texto))
    }

    /// Tokens `[a, b)` como texto, com espaço só entre palavras.
    fn juntar(&self, a: usize, b: usize) -> String {
        let mut s = String::new();
        let mut ant: Option<&Tok> = None;
        for t in &self.t[a.min(self.n())..b.min(self.n())] {
            let palavra = |t: &Tok| matches!(t.k, K::Ident | K::Num | K::Texto);
            if let Some(p) = ant
                && (palavra(p) && palavra(t)
                    || p.s == ","
                    || p.s == ":" && self.ts()
                    || p.k == K::Op && matches!(p.s.as_str(), ")" | ">") && palavra(t))
            {
                s.push(' ');
            }
            match t.k {
                K::Texto => {
                    s.push('"');
                    s.push_str(&t.s);
                    s.push('"');
                }
                _ => s.push_str(&t.s),
            }
            ant = Some(t);
        }
        resumir(&s)
    }

    fn concluir(self) -> Extracao {
        // Menções nas strings (SQL embutido), como no extrator genérico.
        let mut sep = Separado::default();
        for (x, t) in self.t.iter().enumerate() {
            if t.k != K::Texto || self.sem_mencao[x] {
                continue;
            }
            let texto = if t.interp {
                sem_interpolacao(&t.s)
            } else {
                t.s.clone()
            };
            for (d, l) in texto.split('\n').enumerate() {
                if !l.trim().is_empty() {
                    sep.textos.push((t.linha + d as u32, l.to_string()));
                }
            }
        }
        let definidos: Vec<Simbolo> = self
            .simbolos
            .iter()
            .enumerate()
            .filter(|(i, _)| !self.endpoints.contains(i))
            .map(|(_, s)| s.clone())
            .collect();
        let mut refs = self.refs;
        refs.extend(mencoes(&sep, &definidos));
        let mut vistos = HashSet::new();
        refs.retain(|r| vistos.insert((r.origem.clone(), r.alvo.clone(), r.tipo, r.linha)));
        refs.sort_by_key(|r| r.linha);
        Extracao {
            simbolos: self.simbolos,
            referencias: refs,
        }
    }
}

/// Parâmetro de rotina ou componente de record.
#[derive(Debug)]
struct Param {
    nome: String,
    /// Tipo base, sem argumentos de tipo (`List` em `List<Parcela>`).
    tipo: Option<String>,
    tipo_ini: usize,
    tipo_fim: usize,
    tipo_texto: String,
    ini: usize,
    /// Parâmetro do construtor que vira propriedade (TypeScript).
    propriedade: bool,
}

/// O token continua a expressão da linha anterior.
fn continua(b: &Tok) -> bool {
    match b.k {
        K::Op => matches!(
            b.s.as_str(),
            "." | "?."
                | "("
                | "["
                | ")"
                | "]"
                | "}"
                | "?"
                | ":"
                | "+"
                | "-"
                | "*"
                | "/"
                | "%"
                | "="
                | "=="
                | "==="
                | "!="
                | "!=="
                | "&&"
                | "||"
                | "??"
                | "|"
                | "&"
                | ","
                | "=>"
                | "<"
                | ">"
                | "<="
                | ">="
                | "^"
        ),
        K::Ident => matches!(
            b.s.as_str(),
            "as" | "satisfies" | "instanceof" | "in" | "extends" | "implements"
        ),
        _ => false,
    }
}

fn sem_interpolacao(s: &str) -> String {
    let mut out = String::new();
    let mut prof = 0;
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if prof == 0 && c == '$' && it.peek() == Some(&'{') {
            continue;
        }
        match c {
            '{' if prof == 0 && it.peek() == Some(&'{') => {
                it.next();
                out.push('{');
            }
            '{' => {
                prof += 1;
                out.push(' ');
            }
            '}' if prof > 0 => {
                prof -= 1;
                out.push(' ');
            }
            _ if prof > 0 => {}
            _ => out.push(c),
        }
    }
    out
}

/// Texto de um comentário de documentação, sem delimitadores, tags XML/HTML nem
/// as linhas de tag do Javadoc (`@param`).
fn limpar_comentario(texto: &str) -> String {
    let mut partes = Vec::new();
    for l in texto.lines() {
        let mut l = l.trim();
        for p in ["/**", "/*", "///", "//"] {
            if let Some(r) = l.strip_prefix(p) {
                l = r;
                break;
            }
        }
        let l = l.strip_suffix("*/").unwrap_or(l);
        let l = l.trim().trim_start_matches('*').trim();
        if l.starts_with('@') {
            continue;
        }
        let mut s = String::new();
        let mut tag = false;
        let c: Vec<char> = l.chars().collect();
        for (i, &ch) in c.iter().enumerate() {
            if ch == '<'
                && c.get(i + 1)
                    .is_some_and(|n| n.is_ascii_alphabetic() || *n == '/')
                && c[i..].contains(&'>')
            {
                tag = true;
                s.push(' ');
                continue;
            }
            if tag {
                if ch == '>' {
                    tag = false;
                }
                continue;
            }
            s.push(ch);
        }
        let s = s.trim().to_string();
        if !s.is_empty() && !s.chars().all(|c| "=-*#~_/".contains(c)) {
            partes.push(s);
        }
    }
    partes
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Caminho com `/` no começo.
fn barra(caminho: &str) -> String {
    if caminho.starts_with('/') {
        caminho.to_string()
    } else {
        format!("/{caminho}")
    }
}

/// Junta o prefixo de rota da classe com o caminho do método.
pub(crate) fn juntar_rota(prefixo: &str, caminho: &str) -> String {
    let p = prefixo.trim_matches('/');
    let c = caminho.trim_matches('/');
    match (p.is_empty(), c.is_empty()) {
        (true, _) => format!("/{c}"),
        (false, true) => format!("/{p}"),
        (false, false) => format!("/{p}/{c}"),
    }
}

/// Chave de rota: o caminho sem esquema, host, query nem barra final, em
/// minúsculas, com cada parâmetro (`{id}`, `${id}`, `:id`, `*`, `42`) como `{}` e `.`
/// trocado por `_` (o mapa separa nomes qualificados por `.`). Um parâmetro no
/// começo (a URL base, `${api}/faturas`) cai. `None` se não parecer caminho.
pub(crate) fn rota(caminho: &str) -> Option<String> {
    let mut s = caminho.trim();
    if s.chars().any(char::is_whitespace) {
        return None;
    }
    if let Some(p) = s.find("://") {
        let resto = &s[p + 3..];
        s = resto.find('/').map(|q| &resto[q..]).unwrap_or("/");
    }
    let s = s.split(['?', '#']).next().unwrap_or("");
    let mut segs: Vec<String> = s
        .split('/')
        .filter(|x| !x.is_empty() && *x != "~")
        .map(|x| {
            if x.contains('{')
                || x.starts_with(':')
                || x.starts_with('*')
                || x.chars().all(|c| c.is_ascii_digit())
            {
                "{}".to_string()
            } else {
                x.to_lowercase().replace('.', "_")
            }
        })
        .collect();
    if segs.len() > 1 && segs[0] == "{}" {
        segs.remove(0);
    }
    if segs.iter().all(|x| x == "{}") && !segs.is_empty() {
        return None;
    }
    Some(format!("/{}", segs.join("/")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotas() {
        assert_eq!(
            rota("/api/faturas/{id}/parcelamento").unwrap(),
            "/api/faturas/{}/parcelamento"
        );
        assert_eq!(rota("api/Faturas/{id:int}").unwrap(), "/api/faturas/{}");
        assert_eq!(rota("/api/faturas/${}/x/").unwrap(), "/api/faturas/{}/x");
        assert_eq!(rota("https://host:8080/api/x?y=1").unwrap(), "/api/x");
        assert_eq!(rota("{}/faturas/:id").unwrap(), "/faturas/{}");
        assert_eq!(rota("/v1.0/a").unwrap(), "/v1_0/a");
        assert_eq!(rota("/api/itens/42").unwrap(), "/api/itens/{}");
        assert_eq!(rota("").unwrap(), "/");
        assert!(rota("Erro ao parcelar").is_none());
        assert!(rota("{}").is_none());
        assert_eq!(juntar_rota("/api/faturas/", "/{id}"), "/api/faturas/{id}");
        assert_eq!(juntar_rota("", "x"), "/x");
        assert_eq!(juntar_rota("api", ""), "/api");
    }

    #[test]
    fn lexico_de_strings() {
        let lx = lexico(
            "a = \"\"\"\n  select *\n  from fatura\n  \"\"\"; // c\nb",
            Ling::Java,
        );
        assert_eq!(lx.tokens[2].k, K::Texto);
        assert!(lx.tokens[2].s.contains("from fatura"));
        assert_eq!((lx.tokens[2].linha, lx.tokens[2].fim), (1, 4));
        assert_eq!(lx.tokens[4].linha, 5);
        assert!(!lx.comentarios[0].sozinho);

        let lx = lexico(
            "var a = @\"c:\\x \"\"q\"\"\n fim\"; var b = $\"{x} \\\" {{y}}\"; var c = \"\"\"\n raw \"\n \"\"\";",
            Ling::CSharp,
        );
        let textos: Vec<_> = lx
            .tokens
            .iter()
            .filter(|t| t.k == K::Texto)
            .map(|t| (t.s.as_str(), t.interp))
            .collect();
        assert_eq!(
            textos,
            [
                ("c:\\x \"q\"\n fim", false),
                ("{x} \\\" {{y}}", true),
                (" raw \"\n ", false)
            ]
        );

        let lx = lexico(
            "const u = `/api/${a.b}/x`; const r = /a\\/b[/]/g.test(x); y = a / b / c;\n'it''s'",
            Ling::TypeScript,
        );
        let k: Vec<_> = lx.tokens.iter().map(|t| (t.k, t.s.as_str())).collect();
        assert!(k.contains(&(K::Texto, "/api/${a.b}/x")));
        assert!(k.contains(&(K::Num, "/a\\/b[/]/g")));
        assert_eq!(k.iter().filter(|x| x.1 == "/").count(), 2);
        assert!(k.contains(&(K::Texto, "it")));
    }

    #[test]
    fn interpolacao_some_das_mencoes() {
        assert_eq!(sem_interpolacao("a ${x.y} b"), "a    b");
        assert_eq!(sem_interpolacao("a {x} {{b}}"), "a    {b}}");
    }

    #[test]
    fn comentario_limpo() {
        assert_eq!(
            limpar_comentario("/**\n * Calcula <b>multa</b>.\n *\n * @param x valor\n */"),
            "Calcula multa ."
        );
        assert_eq!(
            limpar_comentario("/// <summary>\n/// Fatura em aberto.\n/// </summary>"),
            "Fatura em aberto."
        );
    }
}
