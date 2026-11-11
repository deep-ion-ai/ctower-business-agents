//! Extrator de Delphi e Object Pascal: units, programas e formulários `.dfm`.
//!
//! Separa código, comentários e strings com o léxico do genérico
//! ([`Sintaxe::PASCAL`]) e lê o código com uma descida recursiva rasa, sem
//! gramática completa. Reconhece:
//!
//! - `unit`, `program`, `library` e `package` (o arquivo todo);
//! - na seção `type`: classes, interfaces, records e objects (com campos,
//!   métodos, propriedades, constantes e tipos aninhados), enums e demais tipos;
//! - constantes, `resourcestring` e variáveis globais;
//! - procedures e funções, soltas ou métodos (`TClasse.Metodo`), com as rotinas
//!   aninhadas. A declaração (na classe, na seção `interface`, `forward`,
//!   `external`) fica marcada como declaração; a implementação vai do cabeçalho ao
//!   `end` do corpo;
//! - referências: `uses`, `requires` e `contains` (importação), ancestral
//!   (herança) e interfaces (implementação), `{$I arquivo}` (inclusão) e, nos
//!   corpos, chamadas às rotinas do próprio arquivo, uso das constantes e
//!   variáveis dele, `TClasse.Create` (instanciação) e menção às demais rotinas
//!   chamadas com `(` ou `;`. Strings e nomes com cara de objeto de banco viram
//!   menções, como no genérico: é ali que mora o SQL embutido.
//!
//! No `.dfm` em texto, o formulário vira [`TipoSimbolo::Tela`] e cada `object`
//! um [`TipoSimbolo::Componente`] (com a classe e o `Caption` no detalhe); cada
//! evento ligado (`OnClick = btnOkClick`) é uma chamada do componente ao método
//! `TFormulario.btnOkClick`. `.dfm` binário não gera nada.

use std::collections::HashSet;

use super::generico::{Separado, Sintaxe, mencoes, separar};
use super::sql::resumir;
use super::{Extracao, Extrator, Fonte, Linhas, Referencia, Simbolo, TipoReferencia, TipoSimbolo};
use crate::linguagem::Linguagem;

/// Extrator de Delphi (`.pas`, `.dpr`, `.dpk`, `.dfm`).
#[derive(Debug, Clone, Copy, Default)]
pub struct ExtratorDelphi;

impl Extrator for ExtratorDelphi {
    fn nome(&self) -> &'static str {
        "delphi"
    }

    fn versao(&self) -> u32 {
        1
    }

    fn linguagens(&self) -> &'static [Linguagem] {
        &[Linguagem::Delphi]
    }

    fn extrair(&self, fonte: &Fonte<'_>) -> Result<Extracao, String> {
        if eh_dfm(fonte.caminho, fonte.texto) {
            return Ok(dfm(fonte.texto));
        }
        Ok(pascal(fonte.caminho, fonte.texto))
    }
}

fn eh_dfm(caminho: &str, texto: &str) -> bool {
    if caminho.to_ascii_lowercase().ends_with(".dfm") {
        return true;
    }
    let primeira = texto
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    matches!(primeira.as_str(), "object" | "inherited" | "inline")
}

/// Nome do arquivo sem pasta nem extensão.
pub(super) fn nome_do_arquivo(caminho: &str) -> &str {
    let nome = caminho.rsplit(['/', '\\']).next().unwrap_or(caminho);
    nome.rsplit_once('.').map_or(nome, |(b, _)| b)
}

/// Comentários em linhas inteiras logo acima de `linha` (como separados por
/// [`separar`]), sem os delimitadores. Serve aos extratores de Delphi e VB6.
pub(super) fn comentario_antes(comentarios: &[(u32, String, bool)], linha: u32) -> Option<String> {
    let mut partes = Vec::new();
    let mut alvo = linha.checked_sub(1)?;
    for (l, texto, sozinho) in comentarios.iter().rev() {
        if *l > alvo {
            continue;
        }
        if *l < alvo || !*sozinho || texto.trim_start().starts_with('$') {
            break;
        }
        partes.push(texto.as_str());
        alvo = match alvo.checked_sub(1) {
            Some(x) => x,
            None => break,
        };
    }
    partes.reverse();
    let limpo: Vec<&str> = partes
        .iter()
        .map(|p| {
            let p = p.trim();
            let p = p
                .strip_prefix("//")
                .or_else(|| p.strip_prefix('\''))
                .unwrap_or(p);
            let p = if p.len() >= 4 && p[..4].eq_ignore_ascii_case("rem ") {
                &p[4..]
            } else {
                p
            };
            p.trim_matches(['/', '*', '{', '}', '(', ')', '\'']).trim()
        })
        .filter(|p| !p.is_empty() && !p.chars().all(|c| "=-*#~_".contains(c)))
        .collect();
    (!limpo.is_empty()).then(|| resumir(&limpo.join(" ")))
}

/// Acrescenta as menções do genérico que não repetem uma referência já
/// registrada na mesma linha e com o mesmo alvo.
pub(super) fn acrescentar_mencoes(saida: &mut Extracao, sep: &Separado) {
    let ja: HashSet<(u32, String)> = saida
        .referencias
        .iter()
        .map(|r| (r.linha, r.alvo.to_uppercase()))
        .collect();
    for r in mencoes(sep, &saida.simbolos) {
        if !ja.contains(&(r.linha, r.alvo.to_uppercase())) {
            saida.referencias.push(r);
        }
    }
    saida.referencias.sort_by_key(|r| r.linha);
}

/// Texto com cara de SQL ou de nome de objeto de banco: tem uma palavra do SQL
/// (`SELECT`, `FROM`, `EXEC`...) ou é um nome só, com `_` ou em maiúsculas
/// (`PKG_COBRANCA.PRC_CANCELAR_FATURA`, `FATURA`). Mensagens e legendas ficam de
/// fora, para não virar menção a classes e tabelas de nome parecido.
pub(super) fn parece_sql(texto: &str) -> bool {
    const PALAVRAS: &[&str] = &[
        "SELECT", "INSERT", "UPDATE", "DELETE", "MERGE", "FROM", "WHERE", "JOIN", "INTO", "VALUES",
        "EXEC", "EXECUTE", "CALL", "BEGIN", "TRUNCATE",
    ];
    let t = texto.trim();
    let palavras: Vec<String> = t
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|p| !p.is_empty())
        .map(|p| p.to_uppercase())
        .collect();
    if palavras.iter().any(|p| PALAVRAS.contains(&p.as_str())) {
        return true;
    }
    let nome = !t.is_empty()
        && t.split('.').all(|p| {
            p.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
                && p.chars()
                    .all(|c| c.is_ascii_alphanumeric() || "_$#".contains(c))
        });
    nome && (t.contains('_') || !t.chars().any(|c| c.is_ascii_lowercase()))
}

/// Separado só com o código e as strings com cara de SQL.
pub(super) fn so_sql(sep: &Separado) -> Separado {
    Separado {
        codigo: sep.codigo.clone(),
        textos: sep
            .textos
            .iter()
            .filter(|(_, t)| parece_sql(t))
            .cloned()
            .collect(),
        comentarios: Vec::new(),
    }
}

/// Nome qualificado do símbolo mais interno que contém a linha.
pub(super) fn dono(simbolos: &[Simbolo], linha: u32) -> Option<String> {
    simbolos
        .iter()
        .rev()
        .find(|s| !s.declaracao && s.linhas.contem(linha))
        .map(|s| s.qualificado.clone())
}

// ---------------------------------------------------------------------------
// Formulário (.dfm)

fn dfm(texto: &str) -> Extracao {
    let mut saida = Extracao::default();
    if texto.contains('\0') {
        return saida;
    }
    let inicio = texto.trim_start();
    if !eh_dfm("", inicio) {
        // Binário (`TPF0`) ou desconhecido.
        return saida;
    }
    let sep = separar(texto, &Sintaxe::PASCAL);
    // (índice do símbolo, classe que recebe os eventos)
    let mut pilha: Vec<(usize, String)> = Vec::new();
    let mut itens = 0u32;
    let mut ultima = 1;
    // Linhas com valores de propriedades de SQL (`SQL.Strings`, `CommandText`).
    let mut linhas_sql: HashSet<u32> = HashSet::new();
    let mut lista_sql: Option<bool> = None;
    for (n, l) in sep.codigo.iter().enumerate() {
        let num = n as u32 + 1;
        let t = l.trim();
        if t.is_empty() {
            continue;
        }
        ultima = num;
        if let Some(sql) = lista_sql {
            if sql {
                linhas_sql.insert(num);
            }
            if t.ends_with(')') {
                lista_sql = None;
            }
            continue;
        }
        let low = t.to_ascii_lowercase();
        let palavra = low.split_whitespace().next().unwrap_or("");
        if matches!(palavra, "object" | "inherited" | "inline") && t.len() > palavra.len() {
            let resto = t[palavra.len()..].trim();
            let (nome, classe) = match resto.split_once(':') {
                Some((a, b)) => (
                    a.trim(),
                    b.split_whitespace().next().unwrap_or("").to_string(),
                ),
                None => {
                    let c = resto.split_whitespace().next().unwrap_or("");
                    (c, c.to_string())
                }
            };
            let tipo = if pilha.is_empty() {
                TipoSimbolo::Tela
            } else {
                TipoSimbolo::Componente
            };
            let mut s = Simbolo::novo(nome, tipo, Linhas::new(num, num));
            if let Some((pai, _)) = pilha.last() {
                s = s.dentro_de(&saida.simbolos[*pai].qualificado.clone());
            }
            s.detalhe = (!classe.is_empty()).then(|| classe.clone());
            let classe_eventos = match pilha.last() {
                Some((_, c)) if palavra != "inline" => c.clone(),
                _ => {
                    if !classe.is_empty() {
                        saida.referencias.push(Referencia {
                            origem: Some(s.qualificado.clone()),
                            alvo: classe.clone(),
                            tipo: TipoReferencia::Tipo,
                            linha: num,
                        });
                    }
                    classe.clone()
                }
            };
            pilha.push((saida.simbolos.len(), classe_eventos));
            saida.simbolos.push(s);
            continue;
        }
        if low == "item" {
            itens += 1;
            continue;
        }
        if itens > 0 && low.starts_with("end") {
            itens -= 1;
            continue;
        }
        if low == "end" {
            if let Some((i, _)) = pilha.pop() {
                let s = &mut saida.simbolos[i];
                s.linhas = Linhas::new(s.linhas.inicio, num);
            }
            continue;
        }
        let Some((prop, valor)) = t.split_once('=') else {
            continue;
        };
        let (prop, valor) = (prop.trim(), valor.trim());
        let sql = propriedade_de_sql(prop);
        if sql {
            linhas_sql.insert(num);
        }
        if valor.starts_with('(') && !valor.ends_with(')') {
            lista_sql = Some(sql);
        }
        let Some((topo, classe)) = pilha.last() else {
            continue;
        };
        let evento = prop.len() > 2
            && prop.starts_with("On")
            && prop[2..].starts_with(|c: char| c.is_ascii_uppercase());
        if evento && !valor.is_empty() && valor.chars().all(|c| c.is_alphanumeric() || c == '_') {
            saida.referencias.push(Referencia {
                origem: Some(saida.simbolos[*topo].qualificado.clone()),
                alvo: if classe.is_empty() {
                    valor.to_string()
                } else {
                    format!("{classe}.{valor}")
                },
                tipo: TipoReferencia::Chamada,
                linha: num,
            });
        } else if prop.eq_ignore_ascii_case("Caption") {
            let legenda: String = sep
                .textos
                .iter()
                .filter(|(l, _)| *l == num)
                .map(|(_, x)| x.as_str())
                .collect();
            let s = &mut saida.simbolos[*topo];
            if !legenda.trim().is_empty() {
                let classe = s.detalhe.clone().unwrap_or_default();
                s.detalhe = Some(resumir(&format!("{classe}: {}", legenda.trim())));
            }
        }
    }
    for (i, _) in pilha {
        let s = &mut saida.simbolos[i];
        s.linhas = Linhas::new(s.linhas.inicio, ultima);
    }
    // SQL e nomes de procedure nas strings (SQL.Strings, ProcedureName).
    let so_textos = Separado {
        textos: sep
            .textos
            .into_iter()
            .filter(|(l, _)| linhas_sql.contains(l))
            .collect(),
        ..Separado::default()
    };
    acrescentar_mencoes(&mut saida, &so_textos);
    saida
}

fn propriedade_de_sql(prop: &str) -> bool {
    let p = prop.to_ascii_lowercase();
    p.contains("sql")
        || [
            "commandtext",
            "procedurename",
            "storedprocname",
            "procname",
            "tablename",
        ]
        .iter()
        .any(|x| p.ends_with(x))
}

// ---------------------------------------------------------------------------
// Léxico do Pascal

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tipo {
    Ident,
    Numero,
    Texto,
    Sim,
}

#[derive(Debug, Clone)]
struct Token {
    tipo: Tipo,
    /// Texto original (a string com aspas e conteúdo).
    texto: String,
    /// Em minúsculas.
    chave: String,
    linha: u32,
    /// Identificador escapado com `&` (nunca é palavra reservada).
    escapado: bool,
}

fn eh_inicio_ident(c: char) -> bool {
    c.is_alphabetic() || c == '_' || c == '\u{FFFD}'
}

fn eh_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '\u{FFFD}'
}

/// Tokens do código (sem comentários) separado; as strings voltam do original.
fn lexico(sep: &Separado, texto: &str) -> Vec<Token> {
    let originais: Vec<Vec<char>> = texto.lines().map(|l| l.chars().collect()).collect();
    let mut tokens = Vec::new();
    for (n, linha) in sep.codigo.iter().enumerate() {
        let num = n as u32 + 1;
        let c: Vec<char> = linha.chars().collect();
        let original = originais.get(n);
        let mut i = 0;
        let mut push = |tipo, texto: String, escapado| {
            tokens.push(Token {
                tipo,
                chave: texto.to_lowercase(),
                texto,
                linha: num,
                escapado,
            });
        };
        while i < c.len() {
            let ch = c[i];
            if ch.is_whitespace() {
                i += 1;
                continue;
            }
            let escapado = ch == '&' && c.get(i + 1).is_some_and(|p| eh_inicio_ident(*p));
            if eh_inicio_ident(ch) || escapado {
                let ini = if escapado { i + 1 } else { i };
                i = ini;
                while i < c.len() && eh_ident(c[i]) {
                    i += 1;
                }
                push(Tipo::Ident, c[ini..i].iter().collect(), escapado);
                continue;
            }
            if ch.is_ascii_digit() || ch == '$' || ch == '#' {
                let ini = i;
                i += 1;
                while i < c.len()
                    && (c[i].is_ascii_alphanumeric()
                        || (c[i] == '.' && c.get(i + 1).is_some_and(|p| p.is_ascii_digit())))
                {
                    i += 1;
                }
                let tipo = if ch == '#' { Tipo::Texto } else { Tipo::Numero };
                push(tipo, c[ini..i].iter().collect(), false);
                continue;
            }
            if ch == '\'' {
                let ini = i;
                i += 1;
                while i < c.len() && c[i] != '\'' {
                    i += 1;
                }
                i = (i + 1).min(c.len());
                let fonte = original
                    .filter(|o| o.len() >= i)
                    .map_or(&c[ini..i], |o| &o[ini..i]);
                push(Tipo::Texto, fonte.iter().collect(), false);
                continue;
            }
            let dois: String = c[i..(i + 2).min(c.len())].iter().collect();
            if matches!(dois.as_str(), ":=" | ".." | "<=" | ">=" | "<>") {
                push(Tipo::Sim, dois, false);
                i += 2;
                continue;
            }
            push(Tipo::Sim, ch.to_string(), false);
            i += 1;
        }
    }
    tokens
}

// ---------------------------------------------------------------------------
// Análise do Pascal

/// Palavras que encerram uma lista de declarações.
const PARADA: &[&str] = &[
    "asm",
    "automated",
    "begin",
    "class",
    "const",
    "constructor",
    "destructor",
    "end",
    "exports",
    "finalization",
    "function",
    "implementation",
    "initialization",
    "interface",
    "label",
    "operator",
    "private",
    "procedure",
    "property",
    "protected",
    "public",
    "published",
    "resourcestring",
    "strict",
    "threadvar",
    "type",
    "uses",
    "var",
];

const DIRETIVAS: &[&str] = &[
    "abstract",
    "assembler",
    "cdecl",
    "delayed",
    "deprecated",
    "dispid",
    "dynamic",
    "experimental",
    "export",
    "external",
    "far",
    "final",
    "forward",
    "index",
    "inline",
    "library",
    "local",
    "message",
    "name",
    "near",
    "overload",
    "override",
    "pascal",
    "platform",
    "register",
    "reintroduce",
    "safecall",
    "static",
    "stdcall",
    "unsafe",
    "varargs",
    "virtual",
    "winapi",
];

const RESERVADAS: &[&str] = &[
    "and",
    "array",
    "as",
    "asm",
    "begin",
    "break",
    "case",
    "class",
    "const",
    "constructor",
    "continue",
    "destructor",
    "div",
    "do",
    "downto",
    "else",
    "end",
    "except",
    "exit",
    "exports",
    "false",
    "file",
    "finalization",
    "finally",
    "for",
    "function",
    "goto",
    "if",
    "implementation",
    "in",
    "inherited",
    "initialization",
    "inline",
    "interface",
    "is",
    "label",
    "library",
    "mod",
    "nil",
    "not",
    "object",
    "of",
    "on",
    "or",
    "out",
    "packed",
    "procedure",
    "program",
    "property",
    "raise",
    "record",
    "repeat",
    "resourcestring",
    "result",
    "self",
    "set",
    "shl",
    "shr",
    "string",
    "then",
    "threadvar",
    "to",
    "true",
    "try",
    "type",
    "unit",
    "until",
    "uses",
    "var",
    "while",
    "with",
    "xor",
];

/// Rotinas da RTL e da VCL comuns demais para virar menção.
const NATIVAS: &[&str] = &[
    "abort",
    "abs",
    "append",
    "application",
    "assert",
    "assigned",
    "beep",
    "cancel",
    "chr",
    "clear",
    "close",
    "copy",
    "create",
    "date",
    "datetostr",
    "dec",
    "delete",
    "destroy",
    "edit",
    "exclude",
    "execsql",
    "first",
    "floattostr",
    "format",
    "free",
    "freeandnil",
    "halt",
    "high",
    "inc",
    "include",
    "insert",
    "inttostr",
    "last",
    "length",
    "low",
    "lowercase",
    "messagedlg",
    "next",
    "now",
    "open",
    "ord",
    "pos",
    "post",
    "pred",
    "prior",
    "quotedstr",
    "readln",
    "refresh",
    "round",
    "sender",
    "setlength",
    "showmessage",
    "sizeof",
    "sleep",
    "str",
    "strtodate",
    "strtofloat",
    "strtoint",
    "strtointdef",
    "succ",
    "time",
    "trim",
    "trunc",
    "uppercase",
    "val",
    "write",
    "writeln",
];

fn pascal(caminho: &str, texto: &str) -> Extracao {
    let sep = separar(texto, &Sintaxe::PASCAL);
    let t = lexico(&sep, texto);
    let mut p = Analisador {
        t: &t,
        i: 0,
        comentarios: &sep.comentarios,
        saida: Extracao::default(),
        corpos: Vec::new(),
        raiz: String::new(),
        interface: false,
    };
    p.arquivo(caminho);
    p.referencias_dos_corpos();
    let mut saida = p.saida;
    // `{$I arquivo}` e `{$INCLUDE arquivo}`.
    for (linha, c, _) in &sep.comentarios {
        let c = c.trim();
        let up = c.to_ascii_uppercase();
        let resto = up
            .strip_prefix("$INCLUDE ")
            .or_else(|| up.strip_prefix("$I "))
            .map(|r| &c[c.len() - r.len()..]);
        if let Some(arquivo) = resto {
            let arquivo = arquivo.trim().trim_matches('\'');
            if !arquivo.is_empty() && !arquivo.starts_with('+') && !arquivo.starts_with('-') {
                let origem = dono(&saida.simbolos, *linha);
                saida.referencias.push(Referencia {
                    origem,
                    alvo: nome_do_arquivo(arquivo).to_string(),
                    tipo: TipoReferencia::Inclusao,
                    linha: *linha,
                });
            }
        }
    }
    acrescentar_mencoes(&mut saida, &so_sql(&sep));
    saida
}

struct Analisador<'a> {
    t: &'a [Token],
    i: usize,
    comentarios: &'a [(u32, String, bool)],
    saida: Extracao,
    /// Corpos das rotinas: (primeiro token, `end`, origem).
    corpos: Vec<(usize, usize, String)>,
    raiz: String,
    /// Na seção `interface` da unit: rotinas são só declaração.
    interface: bool,
}

impl Analisador<'_> {
    fn chave(&self, k: usize) -> &str {
        self.t.get(self.i + k).map_or("", |t| t.chave.as_str())
    }

    fn linha(&self) -> u32 {
        self.t.get(self.i).or(self.t.last()).map_or(1, |t| t.linha)
    }

    fn eh_ident(&self, k: usize) -> bool {
        self.t
            .get(self.i + k)
            .is_some_and(|t| t.tipo == Tipo::Ident)
    }

    /// Palavra de parada no token atual (identificador escapado nunca é).
    fn parada(&self) -> bool {
        self.t
            .get(self.i)
            .is_some_and(|t| !t.escapado && PARADA.contains(&t.chave.as_str()))
    }

    fn novo(&mut self, nome: &str, tipo: TipoSimbolo, ini: u32, fim: u32, pai: &str) -> usize {
        let mut s = Simbolo::novo(nome, tipo, Linhas::new(ini, fim));
        if !pai.is_empty() {
            s = s.dentro_de(pai);
        }
        s.comentario = comentario_antes(self.comentarios, ini);
        self.saida.simbolos.push(s);
        self.saida.simbolos.len() - 1
    }

    fn fim_em(&mut self, idx: usize, fim: u32) {
        let s = &mut self.saida.simbolos[idx];
        s.linhas = Linhas::new(s.linhas.inicio, fim);
    }

    fn referir(&mut self, origem: &str, alvo: &str, tipo: TipoReferencia, linha: u32) {
        self.saida.referencias.push(Referencia {
            origem: (!origem.is_empty()).then(|| origem.to_string()),
            alvo: alvo.to_string(),
            tipo,
            linha,
        });
    }

    /// Texto dos tokens `[de, ate)` numa linha só.
    fn juntar(&self, de: usize, ate: usize) -> String {
        let mut s = String::new();
        let mut antes = "";
        for tk in self.t.get(de..ate.min(self.t.len())).unwrap_or(&[]) {
            let x = tk.texto.as_str();
            let cola = s.is_empty()
                || matches!(x, ")" | "," | ";" | ":" | "." | "]" | "[")
                || matches!(antes, "(" | "." | "[")
                || (x == "(" && antes.ends_with(|c: char| c.is_alphanumeric() || c == '_'));
            if !cola {
                s.push(' ');
            }
            s.push_str(x);
            antes = x;
        }
        resumir(&s)
    }

    /// Pula até o `;` que encerra a declaração (fora de parênteses, colchetes e
    /// `record ... end`), consumindo-o. Devolve o índice do `;` e a linha do token
    /// anterior a ele.
    fn pular_declaracao(&mut self) -> (usize, u32) {
        let mut parenteses = 0i32;
        let mut blocos = 0i32;
        let inicio = self.i;
        let mut linha = self.linha();
        while self.i < self.t.len() {
            let k = self.chave(0);
            match k {
                "(" | "[" => parenteses += 1,
                ")" | "]" => parenteses -= 1,
                "record" => blocos += 1,
                "object" if self.i == 0 || self.t[self.i - 1].chave != "of" => blocos += 1,
                "end" if blocos > 0 => blocos -= 1,
                ";" if parenteses <= 0 && blocos <= 0 => {
                    let fim = self.i;
                    self.i += 1;
                    return (fim, linha);
                }
                _ if self.parada() && parenteses <= 0 && blocos <= 0 && self.i > inicio => {
                    return (self.i, linha);
                }
                _ => {}
            }
            linha = self.t[self.i].linha;
            self.i += 1;
        }
        (self.t.len(), linha)
    }

    /// Pula de `(`, `[` ou `<` até o fechamento correspondente, consumindo-o.
    fn pular_par(&mut self, abre: &str, fecha: &str) {
        let mut n = 0;
        while self.i < self.t.len() {
            let k = self.chave(0);
            if k == abre {
                n += 1;
            } else if k == fecha {
                n -= 1;
                if n <= 0 {
                    self.i += 1;
                    return;
                }
            }
            self.i += 1;
        }
    }

    /// Índice do `end` que fecha o bloco que começa em `ini` (`begin`, `asm`,
    /// `try`, `case`).
    fn fim_de_bloco(&self, ini: usize) -> usize {
        let mut n = 0;
        for k in ini..self.t.len() {
            match self.t[k].chave.as_str() {
                "begin" | "try" | "case" | "asm" => n += 1,
                "end" => {
                    n -= 1;
                    if n <= 0 {
                        return k;
                    }
                }
                _ => {}
            }
        }
        self.t.len().saturating_sub(1)
    }

    fn arquivo(&mut self, caminho: &str) {
        let Some(primeiro) = self.t.first() else {
            return;
        };
        let ini = primeiro.linha;
        let fim = self.t.last().map_or(ini, |t| t.linha);
        let (nome, tipo) = match self.chave(0) {
            k @ ("unit" | "program" | "library" | "package") => {
                let tipo = if matches!(k, "unit" | "package") {
                    TipoSimbolo::Modulo
                } else {
                    TipoSimbolo::Programa
                };
                self.i += 1;
                let mut nome = nome_do_arquivo(caminho).to_string();
                while self.eh_ident(0) {
                    nome = self.t[self.i].texto.clone();
                    self.i += 1;
                    if self.chave(0) != "." {
                        break;
                    }
                    self.i += 1;
                }
                self.pular_declaracao();
                (nome, tipo)
            }
            _ => (nome_do_arquivo(caminho).to_string(), TipoSimbolo::Modulo),
        };
        let raiz = self.novo(&nome, tipo, ini, fim, "");
        self.raiz = self.saida.simbolos[raiz].qualificado.clone();
        let raiz_q = self.raiz.clone();
        while self.i < self.t.len() {
            match self.chave(0) {
                "interface" => {
                    self.interface = true;
                    self.i += 1;
                }
                "implementation" => {
                    self.interface = false;
                    self.i += 1;
                }
                "uses" | "requires" | "contains" => self.uses(),
                "type" => {
                    self.i += 1;
                    self.tipos(&raiz_q, true);
                }
                "const" | "resourcestring" => {
                    self.i += 1;
                    self.constantes(&raiz_q, true);
                }
                "var" | "threadvar" => {
                    self.i += 1;
                    self.variaveis(&raiz_q, true);
                }
                "procedure" | "function" | "constructor" | "destructor" | "operator" => {
                    self.rotina(&raiz_q, false);
                }
                "initialization" | "finalization" | "begin" => {
                    // Até `finalization` ou o `end.` final.
                    let ini = self.i + 1;
                    let mut n = 0;
                    let mut k = ini;
                    while k < self.t.len() {
                        match self.t[k].chave.as_str() {
                            "begin" | "try" | "case" | "asm" => n += 1,
                            "end" if n == 0 => break,
                            "end" => n -= 1,
                            "finalization" if n == 0 => break,
                            _ => {}
                        }
                        k += 1;
                    }
                    self.corpos.push((ini, k, raiz_q.clone()));
                    self.i = k;
                    if self.chave(0) == "end" {
                        self.i += 1;
                    }
                }
                "label" | "exports" => {
                    self.pular_declaracao();
                }
                _ => self.i += 1,
            }
        }
    }

    fn uses(&mut self) {
        self.i += 1;
        let raiz = self.raiz.clone();
        while self.i < self.t.len() && self.chave(0) != ";" {
            if self.eh_ident(0) && !self.parada() {
                let linha = self.t[self.i].linha;
                let mut nome = self.t[self.i].texto.clone();
                self.i += 1;
                while self.chave(0) == "." && self.eh_ident(1) {
                    nome = self.t[self.i + 1].texto.clone();
                    self.i += 2;
                }
                self.referir(&raiz, &nome, TipoReferencia::Importacao, linha);
                // `in 'caminho.pas'`
                if self.chave(0) == "in" {
                    self.i += 2;
                }
            } else if self.parada() {
                return;
            } else {
                self.i += 1;
            }
        }
        self.i += 1;
    }

    fn tipos(&mut self, pai: &str, emitir: bool) {
        while self.i < self.t.len() {
            if self.eh_ident(0) && matches!(self.chave(1), "=" | "<") && !self.parada() {
                self.tipo(pai, emitir);
            } else if self.parada() {
                return;
            } else {
                self.i += 1;
            }
        }
    }

    fn tipo(&mut self, pai: &str, emitir: bool) {
        let nome = self.t[self.i].texto.clone();
        let ini = self.t[self.i].linha;
        self.i += 1;
        if self.chave(0) == "<" {
            self.pular_par("<", ">");
        }
        if self.chave(0) != "=" {
            return;
        }
        self.i += 1;
        if self.chave(0) == "type" {
            self.i += 1;
        }
        if self.chave(0) == "packed" {
            self.i += 1;
        }
        let k = self.chave(0).to_string();
        match k.as_str() {
            "class" | "interface" | "dispinterface" if self.chave(1) == ";" => {
                // Declaração antecipada.
                self.i += 2;
            }
            "class" if self.chave(1) == "of" => self.tipo_simples(&nome, ini, pai, emitir),
            "class" | "object" | "record" | "interface" | "dispinterface" => {
                self.i += 1;
                let tipo = match k.as_str() {
                    "record" => TipoSimbolo::Registro,
                    "interface" | "dispinterface" => TipoSimbolo::Interface,
                    _ => TipoSimbolo::Classe,
                };
                self.corpo_de_tipo(&nome, ini, tipo, pai, emitir);
            }
            "(" => {
                self.pular_par("(", ")");
                let (_, fim) = self.pular_declaracao();
                if emitir {
                    self.novo(&nome, TipoSimbolo::Enum, ini, fim, pai);
                }
            }
            _ => self.tipo_simples(&nome, ini, pai, emitir),
        }
    }

    fn tipo_simples(&mut self, nome: &str, ini: u32, pai: &str, emitir: bool) {
        let de = self.i;
        let (ate, fim) = self.pular_declaracao();
        if emitir {
            let idx = self.novo(nome, TipoSimbolo::Tipo, ini, fim, pai);
            let detalhe = self.juntar(de, ate);
            self.saida.simbolos[idx].detalhe = (!detalhe.is_empty()).then_some(detalhe);
        }
    }

    /// Classe, interface ou record depois da palavra que o abre.
    fn corpo_de_tipo(&mut self, nome: &str, ini: u32, tipo: TipoSimbolo, pai: &str, emitir: bool) {
        let idx = emitir.then(|| self.novo(nome, tipo, ini, ini, pai));
        let q = if pai.is_empty() {
            nome.to_string()
        } else {
            format!("{pai}.{nome}")
        };
        while matches!(self.chave(0), "abstract" | "sealed") {
            self.i += 1;
        }
        if self.chave(0) == "helper" {
            self.i += 1;
            if self.chave(0) == "for" {
                self.i += 2;
            }
        }
        if self.chave(0) == "(" {
            self.i += 1;
            let mut primeiro = true;
            while self.i < self.t.len() && !matches!(self.chave(0), ")" | ";") {
                if self.eh_ident(0) {
                    let linha = self.t[self.i].linha;
                    let mut ancestral = self.t[self.i].texto.clone();
                    self.i += 1;
                    while self.chave(0) == "." && self.eh_ident(1) {
                        ancestral = format!("{ancestral}.{}", self.t[self.i + 1].texto);
                        self.i += 2;
                    }
                    if self.chave(0) == "<" {
                        self.pular_par("<", ">");
                    }
                    let rel = if primeiro || tipo == TipoSimbolo::Interface {
                        TipoReferencia::Heranca
                    } else {
                        TipoReferencia::Implementacao
                    };
                    if emitir {
                        self.referir(&q, &ancestral, rel, linha);
                    }
                    primeiro = false;
                } else {
                    self.i += 1;
                }
            }
            if self.chave(0) == ")" {
                self.i += 1;
            }
        }
        if self.chave(0) == "[" {
            self.pular_par("[", "]");
        }
        let fim = if self.chave(0) == ";" {
            let l = self.t[self.i].linha;
            self.i += 1;
            l
        } else {
            let l = self.membros(&q, emitir);
            // `end;` ou `end platform;`
            self.pular_declaracao();
            l
        };
        if let Some(idx) = idx {
            self.fim_em(idx, fim);
        }
    }

    /// Membros até o `end` do tipo (consumido). Devolve a linha do `end`.
    fn membros(&mut self, q: &str, emitir: bool) -> u32 {
        while self.i < self.t.len() {
            match self.chave(0) {
                "end" => {
                    let l = self.t[self.i].linha;
                    self.i += 1;
                    return l;
                }
                "private" | "protected" | "public" | "published" | "automated" | "strict"
                | "class" | "var" | "threadvar" => self.i += 1,
                "procedure" | "function" | "constructor" | "destructor" | "operator" => {
                    self.rotina_declarada(q, emitir);
                }
                "property" => {
                    let ini = self.t[self.i].linha;
                    self.i += 1;
                    let nome = self.t.get(self.i).map(|t| t.texto.clone());
                    let de = self.i;
                    let (ate, fim) = self.pular_declaracao();
                    if self.chave(0) == "default" && self.chave(1) == ";" {
                        self.i += 2;
                    }
                    if let (true, Some(nome)) = (emitir, nome) {
                        let idx = self.novo(&nome, TipoSimbolo::Propriedade, ini, fim, q);
                        let d = self.juntar(de + 1, ate);
                        self.saida.simbolos[idx].detalhe = (!d.is_empty()).then_some(d);
                    }
                }
                "const" => {
                    self.i += 1;
                    self.constantes(q, emitir);
                }
                "type" => {
                    self.i += 1;
                    self.tipos(q, emitir);
                }
                "case" => {
                    // Parte variante do record: até o `end` do record.
                    let mut n = 0;
                    while self.i < self.t.len() {
                        match self.chave(0) {
                            "(" => n += 1,
                            ")" => n -= 1,
                            "end" if n <= 0 => break,
                            _ => {}
                        }
                        self.i += 1;
                    }
                }
                "[" => self.pular_par("[", "]"),
                _ if self.eh_ident(0) && matches!(self.chave(1), "," | ":") => {
                    self.variaveis_de(q, emitir, TipoSimbolo::Campo);
                }
                _ => self.i += 1,
            }
        }
        self.linha()
    }

    fn constantes(&mut self, pai: &str, emitir: bool) {
        while self.i < self.t.len() {
            if self.eh_ident(0) && matches!(self.chave(1), "=" | ":") && !self.parada() {
                let nome = self.t[self.i].texto.clone();
                let ini = self.t[self.i].linha;
                self.i += 1;
                let de = self.i;
                let (ate, fim) = self.pular_declaracao();
                if emitir {
                    let idx = self.novo(&nome, TipoSimbolo::Constante, ini, fim, pai);
                    let d = self.juntar(de, ate);
                    self.saida.simbolos[idx].detalhe = (!d.is_empty()).then_some(d);
                }
            } else if self.parada() {
                return;
            } else {
                self.i += 1;
            }
        }
    }

    fn variaveis(&mut self, pai: &str, emitir: bool) {
        while self.i < self.t.len() {
            if self.eh_ident(0) && matches!(self.chave(1), "," | ":") && !self.parada() {
                self.variaveis_de(pai, emitir, TipoSimbolo::Variavel);
            } else if self.parada() {
                return;
            } else {
                self.i += 1;
            }
        }
    }

    /// `A, B: Tipo;` como variáveis ou campos.
    fn variaveis_de(&mut self, pai: &str, emitir: bool, tipo: TipoSimbolo) {
        let mut nomes = Vec::new();
        while self.eh_ident(0) {
            nomes.push((self.t[self.i].texto.clone(), self.t[self.i].linha));
            self.i += 1;
            if self.chave(0) != "," {
                break;
            }
            self.i += 1;
        }
        let de = self.i + 1;
        let (ate, fim) = self.pular_declaracao();
        if emitir {
            let d = self.juntar(de, ate);
            for (nome, ini) in nomes {
                let idx = self.novo(&nome, tipo, ini, fim, pai);
                self.saida.simbolos[idx].detalhe = (!d.is_empty()).then(|| d.clone());
            }
        }
    }

    /// Cabeçalho de rotina: nome (partes), assinatura e diretivas. Devolve
    /// (palavra, partes do nome, linha inicial, linha final, assinatura, externa).
    fn cabecalho(&mut self) -> Option<(String, Vec<String>, u32, u32, String, bool)> {
        let kw = self.chave(0).to_string();
        let ini = self.t[self.i].linha;
        let de = self.i;
        self.i += 1;
        let mut partes = Vec::new();
        while self.eh_ident(0) {
            partes.push(self.t[self.i].texto.clone());
            self.i += 1;
            if self.chave(0) == "<" {
                self.pular_par("<", ">");
            }
            if self.chave(0) != "." {
                break;
            }
            self.i += 1;
        }
        if partes.is_empty() {
            return None;
        }
        if self.chave(0) == "(" {
            self.pular_par("(", ")");
        }
        let (ate, mut fim) = self.pular_declaracao();
        let assinatura = self.juntar(de, ate);
        let mut externa = false;
        while DIRETIVAS.contains(&self.chave(0)) && !matches!(self.chave(1), ":" | ",") {
            if matches!(self.chave(0), "forward" | "external") {
                externa = true;
            }
            fim = self.pular_declaracao().1;
        }
        Some((kw, partes, ini, fim, assinatura, externa))
    }

    fn tipo_de_rotina(kw: &str, metodo: bool) -> TipoSimbolo {
        match kw {
            "constructor" => TipoSimbolo::Construtor,
            _ if metodo => TipoSimbolo::Metodo,
            "function" | "operator" => TipoSimbolo::Funcao,
            _ => TipoSimbolo::Procedure,
        }
    }

    /// Método declarado na classe.
    fn rotina_declarada(&mut self, q: &str, emitir: bool) {
        let Some((kw, partes, ini, fim, assinatura, _)) = self.cabecalho() else {
            return;
        };
        if emitir {
            let nome = partes.last().cloned().unwrap_or_default();
            let idx = self.novo(&nome, Self::tipo_de_rotina(&kw, true), ini, fim, q);
            let s = &mut self.saida.simbolos[idx];
            s.declaracao = true;
            s.detalhe = Some(assinatura);
        }
    }

    /// Rotina solta ou implementação de método, com o corpo.
    fn rotina(&mut self, pai: &str, aninhada: bool) {
        let Some((kw, partes, ini, fim, assinatura, externa)) = self.cabecalho() else {
            return;
        };
        let nome = partes.last().cloned().unwrap_or_default();
        let metodo = partes.len() > 1;
        let pai_q = if metodo {
            format!("{}.{}", self.raiz, partes[..partes.len() - 1].join("."))
        } else {
            pai.to_string()
        };
        let tipo = Self::tipo_de_rotina(&kw, metodo);
        let idx = self.novo(&nome, tipo, ini, fim, &pai_q);
        self.saida.simbolos[idx].detalhe = Some(assinatura);
        if (self.interface && !aninhada) || externa {
            self.saida.simbolos[idx].declaracao = true;
            return;
        }
        let q = self.saida.simbolos[idx].qualificado.clone();
        while self.i < self.t.len() {
            match self.chave(0) {
                "var" | "threadvar" => {
                    self.i += 1;
                    self.variaveis(&q, false);
                }
                "const" | "resourcestring" => {
                    self.i += 1;
                    self.constantes(&q, false);
                }
                "type" => {
                    self.i += 1;
                    self.tipos(&q, false);
                }
                "label" => {
                    self.pular_declaracao();
                }
                "procedure" | "function" => self.rotina(&q, true),
                "begin" | "asm" => {
                    let fim = self.fim_de_bloco(self.i);
                    self.corpos.push((self.i + 1, fim, q.clone()));
                    let linha = self.t[fim].linha;
                    self.i = fim + 1;
                    if self.chave(0) == ";" {
                        self.i += 1;
                    }
                    self.fim_em(idx, linha);
                    return;
                }
                _ => return,
            }
        }
    }

    /// Chamadas, usos e instanciações dentro dos corpos.
    fn referencias_dos_corpos(&mut self) {
        let rotinas: HashSet<String> = self
            .saida
            .simbolos
            .iter()
            .filter(|s| {
                matches!(
                    s.tipo,
                    TipoSimbolo::Procedure
                        | TipoSimbolo::Funcao
                        | TipoSimbolo::Metodo
                        | TipoSimbolo::Construtor
                )
            })
            .map(|s| s.nome.to_lowercase())
            .collect();
        let globais: HashSet<String> = self
            .saida
            .simbolos
            .iter()
            .filter(|s| matches!(s.tipo, TipoSimbolo::Constante | TipoSimbolo::Variavel))
            .map(|s| s.nome.to_lowercase())
            .collect();
        let mut vistos: HashSet<(u32, String)> = HashSet::new();
        let corpos = std::mem::take(&mut self.corpos);
        for (de, ate, origem) in corpos {
            for k in de..ate.min(self.t.len()) {
                let tk = &self.t[k];
                if tk.tipo != Tipo::Ident
                    || (!tk.escapado && RESERVADAS.contains(&tk.chave.as_str()))
                {
                    continue;
                }
                let antes = k.checked_sub(1).map_or("", |a| self.t[a].chave.as_str());
                if antes == "." || antes == "inherited" {
                    continue;
                }
                let depois = self.t.get(k + 1).map_or("", |t| t.chave.as_str());
                let depois2 = self.t.get(k + 2).map_or("", |t| t.chave.as_str());
                let cria_form = antes == "(" && k >= 2 && self.t[k - 2].chave == "createform";
                let tipo = if (depois == "." && depois2 == "create") || cria_form {
                    TipoReferencia::Instanciacao
                } else if rotinas.contains(&tk.chave) {
                    TipoReferencia::Chamada
                } else if globais.contains(&tk.chave) {
                    TipoReferencia::Uso
                } else if matches!(depois, "(" | ";") && !NATIVAS.contains(&tk.chave.as_str()) {
                    TipoReferencia::Mencao
                } else {
                    continue;
                };
                if vistos.insert((tk.linha, tk.chave.clone())) {
                    let (alvo, linha) = (tk.texto.clone(), tk.linha);
                    self.referir(&origem, &alvo, tipo, linha);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extrair(caminho: &str, texto: &str) -> Extracao {
        ExtratorDelphi
            .extrair(&Fonte {
                caminho,
                linguagem: Linguagem::Delphi,
                texto,
            })
            .unwrap()
    }

    fn resumo(e: &Extracao) -> Vec<(&str, TipoSimbolo, Linhas, bool)> {
        e.simbolos
            .iter()
            .map(|s| (s.qualificado.as_str(), s.tipo, s.linhas, s.declaracao))
            .collect()
    }

    fn refs(e: &Extracao) -> Vec<(Option<&str>, &str, TipoReferencia, u32)> {
        e.referencias
            .iter()
            .map(|r| (r.origem.as_deref(), r.alvo.as_str(), r.tipo, r.linha))
            .collect()
    }

    const PAS: &str =
        include_str!("../../../../fixtures/faturamento-exemplo/delphi/UFrmCancelamento.pas");
    const DFM: &str =
        include_str!("../../../../fixtures/faturamento-exemplo/delphi/UFrmCancelamento.dfm");

    #[test]
    fn unit_da_fixture() {
        use TipoSimbolo as T;
        let e = extrair("delphi/UFrmCancelamento.pas", PAS);
        let r = resumo(&e);
        let classe = "UFrmCancelamento.TFrmCancelamento";
        for esperado in [
            ("UFrmCancelamento", T::Modulo, Linhas::new(1, 111), false),
            (classe, T::Classe, Linhas::new(12, 30), false),
            (
                "UFrmCancelamento.TFrmCancelamento.btnCancelar",
                T::Campo,
                Linhas::new(17, 17),
                false,
            ),
            (
                "UFrmCancelamento.TFrmCancelamento.btnCancelarClick",
                T::Metodo,
                Linhas::new(21, 21),
                true,
            ),
            (
                "UFrmCancelamento.TFrmCancelamento.FUsuario",
                T::Campo,
                Linhas::new(24, 24),
                false,
            ),
            (
                "UFrmCancelamento.FrmCancelamento",
                T::Variavel,
                Linhas::new(33, 33),
                false,
            ),
            (
                "UFrmCancelamento.DIAS_LIMITE_SEM_SUPERVISOR",
                T::Constante,
                Linhas::new(41, 41),
                false,
            ),
            (
                "UFrmCancelamento.TFrmCancelamento.Configurar",
                T::Metodo,
                Linhas::new(44, 48),
                false,
            ),
            (
                "UFrmCancelamento.TFrmCancelamento.ValidarCancelamento",
                T::Metodo,
                Linhas::new(59, 78),
                false,
            ),
            (
                "UFrmCancelamento.TFrmCancelamento.btnCancelarClick",
                T::Metodo,
                Linhas::new(80, 104),
                false,
            ),
            (
                "UFrmCancelamento.TFrmCancelamento.btnFecharClick",
                T::Metodo,
                Linhas::new(106, 109),
                false,
            ),
        ] {
            assert!(r.contains(&esperado), "{esperado:?} fora de {r:#?}");
        }
        // Variáveis locais não viram símbolo.
        assert!(!r.iter().any(|s| s.0.ends_with("IdFatura")));
        let m = e
            .simbolos
            .iter()
            .find(|s| s.nome == "DiasDesdeEmissao" && !s.declaracao)
            .unwrap();
        assert_eq!(
            m.detalhe.as_deref(),
            Some("function TFrmCancelamento.DiasDesdeEmissao(AIdFatura: Integer): Integer")
        );
        assert_eq!(
            e.simbolos[0].comentario, None,
            "o comentário depois do cabeçalho não é da unit"
        );
        let c = e
            .simbolos
            .iter()
            .find(|s| s.nome == "PERFIL_SUPERVISOR")
            .unwrap();
        assert_eq!(c.detalhe.as_deref(), Some("= 'SUPERVISOR'"));

        let r = refs(&e);
        let unit = Some("UFrmCancelamento");
        let validar = Some("UFrmCancelamento.TFrmCancelamento.ValidarCancelamento");
        let cancelar = Some("UFrmCancelamento.TFrmCancelamento.btnCancelarClick");
        for esperado in [
            (unit, "ADODB", TipoReferencia::Importacao, 9),
            (Some(classe), "TForm", TipoReferencia::Heranca, 12),
            (validar, "TAM_MINIMO_MOTIVO", TipoReferencia::Uso, 63),
            (validar, "DiasDesdeEmissao", TipoReferencia::Chamada, 70),
            (validar, "PERFIL_SUPERVISOR", TipoReferencia::Uso, 71),
            (cancelar, "ValidarCancelamento", TipoReferencia::Chamada, 91),
            (
                cancelar,
                "PKG_COBRANCA.PRC_CANCELAR_FATURA",
                TipoReferencia::Mencao,
                94,
            ),
            (
                Some("UFrmCancelamento.TFrmCancelamento.DiasDesdeEmissao"),
                "FATURA",
                TipoReferencia::Mencao,
                53,
            ),
        ] {
            assert!(r.contains(&esperado), "{esperado:?} fora de {r:#?}");
        }
        // Sem menção duplicada do que já é uso, nem das nativas.
        assert!(!r.contains(&(validar, "TAM_MINIMO_MOTIVO", TipoReferencia::Mencao, 63)));
        assert!(!r.iter().any(|x| x.1 == "ShowMessage" || x.1 == "Exit"));
    }

    #[test]
    fn formulario_da_fixture() {
        let e = extrair("delphi/UFrmCancelamento.dfm", DFM);
        let r = resumo(&e);
        assert_eq!(
            r[0],
            (
                "FrmCancelamento",
                TipoSimbolo::Tela,
                Linhas::new(1, 68),
                false
            )
        );
        assert!(r.contains(&(
            "FrmCancelamento.btnCancelar",
            TipoSimbolo::Componente,
            Linhas::new(40, 48),
            false
        )));
        assert_eq!(r.len(), 9);
        assert_eq!(
            e.simbolos[0].detalhe.as_deref(),
            Some("TFrmCancelamento: Cancelamento de Fatura")
        );
        let botao = e.simbolos.iter().find(|s| s.nome == "btnCancelar").unwrap();
        assert_eq!(botao.detalhe.as_deref(), Some("TButton: Cancelar fatura"));
        assert_eq!(
            refs(&e),
            [
                (
                    Some("FrmCancelamento"),
                    "TFrmCancelamento",
                    TipoReferencia::Tipo,
                    1
                ),
                (
                    Some("FrmCancelamento.btnCancelar"),
                    "TFrmCancelamento.btnCancelarClick",
                    TipoReferencia::Chamada,
                    47
                ),
                (
                    Some("FrmCancelamento.btnFechar"),
                    "TFrmCancelamento.btnFecharClick",
                    TipoReferencia::Chamada,
                    56
                ),
            ]
        );
    }

    #[test]
    fn dfm_com_colecao_sql_e_frame() {
        let e = extrair(
            "f.dfm",
            "inherited FrmLista: TFrmLista\n  object Grade: TDBGrid\n    Columns = <\n      item\n        FieldName = 'ID'\n      end\n      item\n        FieldName = 'STATUS'\n      end>\n    OnDblClick = GradeDblClick\n  end\n  object qry: TADOQuery\n    SQL.Strings = (\n      'SELECT * FROM FATURA'\n      'WHERE STATUS = :S')\n    Picture.Data = {\n      0A54}\n  end\n  inline FrameFiltro: TFrameFiltro\n    object btnFiltrar: TButton\n      OnClick = btnFiltrarClick\n    end\n  end\nend\n",
        );
        let r = resumo(&e);
        assert_eq!(
            r.iter().map(|s| (s.0, s.2)).collect::<Vec<_>>(),
            [
                ("FrmLista", Linhas::new(1, 24)),
                ("FrmLista.Grade", Linhas::new(2, 11)),
                ("FrmLista.qry", Linhas::new(12, 18)),
                ("FrmLista.FrameFiltro", Linhas::new(19, 23)),
                ("FrmLista.FrameFiltro.btnFiltrar", Linhas::new(20, 22)),
            ]
        );
        let r = refs(&e);
        for esperado in [
            (
                Some("FrmLista.Grade"),
                "TFrmLista.GradeDblClick",
                TipoReferencia::Chamada,
                10,
            ),
            (Some("FrmLista.qry"), "FATURA", TipoReferencia::Mencao, 14),
            (
                Some("FrmLista.FrameFiltro"),
                "TFrameFiltro",
                TipoReferencia::Tipo,
                19,
            ),
            (
                Some("FrmLista.FrameFiltro.btnFiltrar"),
                "TFrameFiltro.btnFiltrarClick",
                TipoReferencia::Chamada,
                21,
            ),
        ] {
            assert!(r.contains(&esperado), "{esperado:?} fora de {r:#?}");
        }
        // Binário: nada.
        assert_eq!(
            extrair("b.dfm", "TPF0\u{FFFD}\u{FFFD}"),
            Extracao::default()
        );
    }

    #[test]
    fn tipos_rotinas_e_inclusao() {
        use TipoSimbolo as T;
        let e = extrair(
            "dm/UCalculo.pas",
            "unit UCalculo;\n\ninterface\n\nuses SysUtils, UBase in 'UBase.pas';\n\ntype\n  TStatus = (stAberta, stPaga);\n  TValor = Currency;\n  IRegra = interface\n    ['{00000000-0000-0000-0000-000000000000}']\n    function Aplicar(V: TValor): TValor;\n  end;\n  TRegistro = packed record\n    Id: Integer;\n    case Tipo: Byte of\n      0: (A: Integer);\n  end;\n  TCalculo = class(TBase, IRegra)\n  private\n    FTaxa: Double;\n  public\n    constructor Create;\n    function Aplicar(V: TValor): TValor; virtual;\n    property Taxa: Double read FTaxa write FTaxa;\n  end;\n\n{ Calcula a multa da fatura }\nfunction CalcularMulta(V: Currency): Currency; stdcall;\n\nimplementation\n\n{$I constantes.inc}\n\nfunction Externa: Integer; external 'x.dll';\n\nconstructor TCalculo.Create;\nbegin\n  inherited Create;\nend;\n\nfunction TCalculo.Aplicar(V: TValor): TValor;\n  function Interna: Double;\n  begin\n    Result := FTaxa;\n  end;\nbegin\n  Result := V * Interna;\n  Registrar(V);\nend;\n\n// Calcula a multa da fatura\nfunction CalcularMulta(V: Currency): Currency;\nvar\n  C: TCalculo;\nbegin\n  C := TCalculo.Create;\n  try\n    case Trunc(V) of\n      0: Result := 0;\n    else\n      Result := C.Aplicar(V);\n    end;\n  finally\n    C.Free;\n  end;\nend;\n\nend.\n",
        );
        let r = resumo(&e);
        for esperado in [
            ("UCalculo.TStatus", T::Enum, Linhas::new(8, 8), false),
            ("UCalculo.TValor", T::Tipo, Linhas::new(9, 9), false),
            ("UCalculo.IRegra", T::Interface, Linhas::new(10, 13), false),
            (
                "UCalculo.IRegra.Aplicar",
                T::Metodo,
                Linhas::new(12, 12),
                true,
            ),
            (
                "UCalculo.TRegistro",
                T::Registro,
                Linhas::new(14, 18),
                false,
            ),
            (
                "UCalculo.TRegistro.Id",
                T::Campo,
                Linhas::new(15, 15),
                false,
            ),
            ("UCalculo.TCalculo", T::Classe, Linhas::new(19, 26), false),
            (
                "UCalculo.TCalculo.Create",
                T::Construtor,
                Linhas::new(23, 23),
                true,
            ),
            (
                "UCalculo.TCalculo.Taxa",
                T::Propriedade,
                Linhas::new(25, 25),
                false,
            ),
            (
                "UCalculo.CalcularMulta",
                T::Funcao,
                Linhas::new(29, 29),
                true,
            ),
            ("UCalculo.Externa", T::Funcao, Linhas::new(35, 35), true),
            (
                "UCalculo.TCalculo.Create",
                T::Construtor,
                Linhas::new(37, 40),
                false,
            ),
            (
                "UCalculo.TCalculo.Aplicar",
                T::Metodo,
                Linhas::new(42, 50),
                false,
            ),
            (
                "UCalculo.TCalculo.Aplicar.Interna",
                T::Funcao,
                Linhas::new(43, 46),
                false,
            ),
            (
                "UCalculo.CalcularMulta",
                T::Funcao,
                Linhas::new(53, 67),
                false,
            ),
        ] {
            assert!(r.contains(&esperado), "{esperado:?} fora de {r:#?}");
        }
        assert!(
            !r.iter().any(|s| s.0.ends_with(".C") || s.0.ends_with(".A")),
            "{r:#?}"
        );
        let multa: Vec<_> = e
            .simbolos
            .iter()
            .filter(|s| s.nome == "CalcularMulta")
            .map(|s| s.comentario.as_deref())
            .collect();
        assert_eq!(
            multa,
            [
                Some("Calcula a multa da fatura"),
                Some("Calcula a multa da fatura")
            ]
        );

        let r = refs(&e);
        let aplicar = Some("UCalculo.TCalculo.Aplicar");
        let multa = Some("UCalculo.CalcularMulta");
        for esperado in [
            (Some("UCalculo"), "UBase", TipoReferencia::Importacao, 5),
            (
                Some("UCalculo.TCalculo"),
                "TBase",
                TipoReferencia::Heranca,
                19,
            ),
            (
                Some("UCalculo.TCalculo"),
                "IRegra",
                TipoReferencia::Implementacao,
                19,
            ),
            (Some("UCalculo"), "constantes", TipoReferencia::Inclusao, 33),
            (aplicar, "Interna", TipoReferencia::Chamada, 48),
            (aplicar, "Registrar", TipoReferencia::Mencao, 49),
            (multa, "TCalculo", TipoReferencia::Instanciacao, 57),
        ] {
            assert!(r.contains(&esperado), "{esperado:?} fora de {r:#?}");
        }
        // `inherited Create` e `C.Aplicar` não são chamadas soltas.
        assert!(!r.iter().any(|x| x.1 == "Create"), "{r:#?}");
    }

    #[test]
    fn windows_1252_e_texto_estranho() {
        let bytes = b"unit UAcento;\n\n{ Cobran\xe7a e situa\xe7\xe3o }\ninterface\nconst\n  MSG = 'Opera\xe7\xe3o inv\xe1lida';\nimplementation\nprocedure Avisar;\nbegin\n  ShowMessage(MSG);\nend;\nend.\n";
        let texto = String::from_utf8_lossy(bytes);
        let e = extrair("UAcento.pas", &texto);
        let r = resumo(&e);
        assert!(r.contains(&(
            "UAcento.MSG",
            TipoSimbolo::Constante,
            Linhas::new(6, 6),
            false
        )));
        assert!(r.contains(&(
            "UAcento.Avisar",
            TipoSimbolo::Procedure,
            Linhas::new(8, 11),
            false
        )));
        assert!(refs(&e).contains(&(Some("UAcento.Avisar"), "MSG", TipoReferencia::Uso, 10)));
        // Nada disso quebra.
        for t in [
            "",
            "unit",
            "begin end",
            "type T = class(",
            "procedure",
            "object",
            "x = (",
            "end.",
        ] {
            extrair("x.pas", t);
        }
    }
}
