//! Extrator de Visual Basic 6: módulos (`.bas`), classes (`.cls`), formulários
//! (`.frm`), controles (`.ctl`, `.dsr`) e projetos (`.vbp`).
//!
//! Separa código, comentários e strings com o léxico do genérico
//! ([`Sintaxe::BASIC`]), junta as linhas continuadas com ` _` e reconhece, linha
//! a linha:
//!
//! - o módulo, com o nome de `Attribute VB_Name` (ou do arquivo): formulário vira
//!   [`TipoSimbolo::Tela`], classe e controle viram [`TipoSimbolo::Classe`] e o
//!   `.bas`, [`TipoSimbolo::Modulo`];
//! - no bloco do designer (`Begin VB.Form ... End`), cada controle como
//!   [`TipoSimbolo::Componente`], aninhado no contêiner, com a classe e o
//!   `Caption` no detalhe;
//! - `Sub`, `Function` e `Property Get/Let/Set` (do cabeçalho ao `End`),
//!   `Declare` (declaração), `Type` (com os campos), `Enum` (com os valores),
//!   constantes e variáveis do módulo;
//! - referências: `Implements` (implementação), o evento ligado pelo nome
//!   (`cmdOk_Click` é chamado pelo controle `cmdOk`), e nos procedimentos
//!   chamadas aos do próprio arquivo, uso das constantes e variáveis dele,
//!   `New Classe` (instanciação) e menção às demais rotinas chamadas como
//!   instrução ou com `(`. Strings com cara de SQL viram menções, como no
//!   genérico;
//! - no `.vbp`, o projeto ([`TipoSimbolo::Programa`]) importa os formulários,
//!   módulos e classes que lista.

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use regex::Regex;

use super::delphi::{acrescentar_mencoes, comentario_antes, nome_do_arquivo, so_sql};
use super::generico::{Separado, Sintaxe, separar};
use super::sql::resumir;
use super::{Extracao, Extrator, Fonte, Linhas, Referencia, Simbolo, TipoReferencia, TipoSimbolo};
use crate::linguagem::Linguagem;

/// Extrator de Visual Basic 6.
#[derive(Debug, Clone, Copy, Default)]
pub struct ExtratorVb6;

impl Extrator for ExtratorVb6 {
    fn nome(&self) -> &'static str {
        "vb6"
    }

    fn versao(&self) -> u32 {
        1
    }

    fn linguagens(&self) -> &'static [Linguagem] {
        &[Linguagem::VisualBasic6]
    }

    fn extrair(&self, fonte: &Fonte<'_>) -> Result<Extracao, String> {
        let ext = fonte
            .caminho
            .rsplit_once('.')
            .map(|(_, e)| e.to_ascii_lowercase())
            .unwrap_or_default();
        if ext == "vbp" {
            return Ok(projeto(fonte.caminho, fonte.texto));
        }
        Ok(modulo(fonte.caminho, &ext, fonte.texto))
    }
}

fn ultima_linha(texto: &str) -> u32 {
    texto
        .lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
        .last()
        .map_or(1, |(n, _)| n as u32 + 1)
}

/// Valor entre aspas de `Chave = "valor"`.
fn entre_aspas(linha: &str) -> Option<&str> {
    let (_, resto) = linha.split_once('"')?;
    Some(resto.split('"').next().unwrap_or(resto))
}

// ---------------------------------------------------------------------------
// Projeto (.vbp)

fn projeto(caminho: &str, texto: &str) -> Extracao {
    let mut saida = Extracao::default();
    let nome = texto
        .lines()
        .find_map(|l| {
            let (k, v) = l.split_once('=')?;
            k.trim()
                .eq_ignore_ascii_case("Name")
                .then(|| v.trim().trim_matches('"').to_string())
        })
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| nome_do_arquivo(caminho).to_string());
    let s = Simbolo::novo(
        nome.clone(),
        TipoSimbolo::Programa,
        Linhas::new(1, ultima_linha(texto)),
    );
    saida.simbolos.push(s);
    for (n, l) in texto.lines().enumerate() {
        let Some((k, v)) = l.split_once('=') else {
            continue;
        };
        let k = k.trim().to_ascii_lowercase();
        if !matches!(
            k.as_str(),
            "form" | "module" | "class" | "usercontrol" | "designer" | "propertypage"
        ) {
            continue;
        }
        // `Module=Nome; arquivo.bas` ou `Form=arquivo.frm`.
        let alvo = match v.split_once(';') {
            Some((nome, _)) => nome.trim().to_string(),
            None => nome_do_arquivo(v.trim()).to_string(),
        };
        if !alvo.is_empty() {
            saida.referencias.push(Referencia {
                origem: Some(nome.clone()),
                alvo,
                tipo: TipoReferencia::Importacao,
                linha: n as u32 + 1,
            });
        }
    }
    saida
}

// ---------------------------------------------------------------------------
// Módulos, classes e formulários

static PROCEDIMENTO: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^(?:(?:public|private|friend|global)\s+)?(?:static\s+)?(sub|function|property\s+(?:get|let|set))\s+([a-z_][a-z0-9_]*)",
    )
    .expect("regex de procedimento")
});

static DECLARE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^(?:(?:public|private|global)\s+)?declare\s+(?:ptrsafe\s+)?(sub|function)\s+([a-z_][a-z0-9_]*)",
    )
    .expect("regex de declare")
});

static BLOCO: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(?:(?:public|private)\s+)?(type|enum)\s+([a-z_][a-z0-9_]*)\s*$")
        .expect("regex de type e enum")
});

static CONSTANTE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(?:(?:public|private|global)\s+)?const\s+(.*)$").expect("regex de const")
});

static VARIAVEL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(?:public|private|dim|global)\s+(?:withevents\s+)?(.*)$")
        .expect("regex de variável")
});

static IMPLEMENTS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^implements\s+([a-z_][a-z0-9_.]*)").expect("regex de implements")
});

static QUALIFICADO: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[A-Za-z_][A-Za-z0-9_]*(?:\.[A-Za-z_][A-Za-z0-9_]*)*").expect("regex de nome")
});

static IDENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[A-Za-z_][A-Za-z0-9_]*").expect("regex de identificador"));

const RESERVADAS: &[&str] = &[
    "and",
    "as",
    "attribute",
    "begin",
    "boolean",
    "byref",
    "byte",
    "byval",
    "call",
    "case",
    "close",
    "const",
    "currency",
    "date",
    "debug",
    "declare",
    "dim",
    "do",
    "double",
    "each",
    "else",
    "elseif",
    "empty",
    "end",
    "enum",
    "erase",
    "error",
    "event",
    "exit",
    "explicit",
    "false",
    "for",
    "friend",
    "function",
    "get",
    "global",
    "gosub",
    "goto",
    "if",
    "implements",
    "in",
    "input",
    "integer",
    "is",
    "let",
    "lib",
    "like",
    "line",
    "long",
    "loop",
    "me",
    "mod",
    "new",
    "next",
    "not",
    "nothing",
    "null",
    "object",
    "on",
    "open",
    "option",
    "optional",
    "or",
    "paramarray",
    "preserve",
    "print",
    "private",
    "property",
    "public",
    "put",
    "redim",
    "rem",
    "resume",
    "return",
    "select",
    "set",
    "single",
    "static",
    "step",
    "stop",
    "string",
    "sub",
    "then",
    "to",
    "true",
    "type",
    "typeof",
    "until",
    "variant",
    "wend",
    "while",
    "with",
    "withevents",
    "xor",
];

/// Funções nativas comuns demais para virar menção.
const NATIVAS: &[&str] = &[
    "abs",
    "array",
    "asc",
    "cbool",
    "ccur",
    "cdate",
    "cdbl",
    "chr",
    "cint",
    "clng",
    "createobject",
    "cstr",
    "cvar",
    "date",
    "dateadd",
    "datediff",
    "datepart",
    "dateserial",
    "day",
    "dir",
    "doevents",
    "environ",
    "eof",
    "err",
    "fix",
    "format",
    "freefile",
    "getobject",
    "hour",
    "iif",
    "inputbox",
    "instr",
    "int",
    "isdate",
    "isempty",
    "isnull",
    "isnumeric",
    "join",
    "kill",
    "lbound",
    "lcase",
    "left",
    "len",
    "load",
    "ltrim",
    "mid",
    "minute",
    "month",
    "msgbox",
    "now",
    "nz",
    "replace",
    "right",
    "round",
    "rtrim",
    "second",
    "sgn",
    "shell",
    "space",
    "split",
    "sqr",
    "time",
    "trim",
    "typename",
    "ubound",
    "ucase",
    "unload",
    "val",
    "vartype",
    "weekday",
    "year",
];

/// Linha lógica: (primeira linha, última linha, código).
type Logica = (u32, u32, String);

/// Junta as linhas continuadas com ` _`, pulando as marcadas.
fn linhas_logicas(codigo: &[String], pular: &HashSet<u32>) -> Vec<Logica> {
    let mut saida: Vec<Logica> = Vec::new();
    let mut atual: Option<Logica> = None;
    for (n, l) in codigo.iter().enumerate() {
        let num = n as u32 + 1;
        if pular.contains(&num) {
            continue;
        }
        let t = l.trim_end();
        let (parte, continua) = match t.strip_suffix('_') {
            Some(p) if p.is_empty() || p.ends_with(char::is_whitespace) => (p, true),
            _ => (t, false),
        };
        match &mut atual {
            Some((_, fim, texto)) => {
                *fim = num;
                texto.push(' ');
                texto.push_str(parte.trim());
            }
            None => atual = Some((num, num, parte.trim().to_string())),
        }
        if !continua {
            saida.extend(atual.take());
        }
    }
    saida.extend(atual);
    saida
}

/// Divide por vírgulas fora de parênteses.
fn dividir(texto: &str) -> Vec<&str> {
    let mut partes = Vec::new();
    let mut n = 0i32;
    let mut ini = 0;
    for (i, c) in texto.char_indices() {
        match c {
            '(' => n += 1,
            ')' => n -= 1,
            ',' if n <= 0 => {
                partes.push(&texto[ini..i]);
                ini = i + 1;
            }
            _ => {}
        }
    }
    partes.push(&texto[ini..]);
    partes
}

fn primeiro_nome(texto: &str) -> Option<&str> {
    IDENT
        .find(texto.trim())
        .filter(|m| m.start() == 0)
        .map(|m| m.as_str())
}

struct Modulo {
    saida: Extracao,
    raiz: String,
    /// Controles do formulário: nome em minúsculas → índice do símbolo.
    controles: HashMap<String, usize>,
    /// Linhas dos procedimentos, com a origem.
    corpos: Vec<(u32, String, String)>,
}

fn modulo(caminho: &str, ext: &str, texto: &str) -> Extracao {
    let sep = separar(texto, &Sintaxe::BASIC);
    let crus: Vec<&str> = texto.lines().collect();
    let nome = crus
        .iter()
        .find(|l| {
            l.trim_start()
                .to_ascii_lowercase()
                .starts_with("attribute vb_name")
        })
        .and_then(|l| entre_aspas(l))
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| nome_do_arquivo(caminho))
        .to_string();
    let tipo_raiz = match ext {
        "frm" => TipoSimbolo::Tela,
        "cls" | "ctl" | "dsr" => TipoSimbolo::Classe,
        _ => TipoSimbolo::Modulo,
    };
    let classe = tipo_raiz != TipoSimbolo::Modulo;
    let mut m = Modulo {
        saida: Extracao::default(),
        raiz: nome.clone(),
        controles: HashMap::new(),
        corpos: Vec::new(),
    };
    m.saida.simbolos.push(Simbolo::novo(
        nome,
        tipo_raiz,
        Linhas::new(1, ultima_linha(texto)),
    ));
    let mut pular = m.designer(&sep);
    for (n, l) in crus.iter().enumerate() {
        let t = l.trim_start().to_ascii_lowercase();
        if t.starts_with("attribute ") || t.starts_with("version ") {
            pular.insert(n as u32 + 1);
        }
    }
    let logicas = linhas_logicas(&sep.codigo, &pular);
    m.codigo(&logicas, &sep, classe);
    m.eventos();
    m.referencias_dos_corpos();
    // Menções: o código e as strings com cara de SQL, fora do designer.
    let mut filtrado = so_sql(&sep);
    for l in &pular {
        if let Some(c) = filtrado.codigo.get_mut(*l as usize - 1) {
            c.clear();
        }
    }
    filtrado.textos.retain(|(l, _)| !pular.contains(l));
    acrescentar_mencoes(&mut m.saida, &filtrado);
    m.saida
}

impl Modulo {
    fn novo(&mut self, nome: &str, tipo: TipoSimbolo, linhas: Linhas, pai: &str) -> usize {
        let s = Simbolo::novo(nome, tipo, linhas).dentro_de(pai);
        self.saida.simbolos.push(s);
        self.saida.simbolos.len() - 1
    }

    fn referir(&mut self, origem: &str, alvo: &str, tipo: TipoReferencia, linha: u32) {
        self.saida.referencias.push(Referencia {
            origem: Some(origem.to_string()),
            alvo: alvo.to_string(),
            tipo,
            linha,
        });
    }

    /// Bloco do designer (`VERSION`, `Begin ... End`): controles. Devolve as
    /// linhas do bloco.
    fn designer(&mut self, sep: &Separado) -> HashSet<u32> {
        let mut linhas = HashSet::new();
        let comeca = sep
            .codigo
            .iter()
            .find(|l| !l.trim().is_empty())
            .is_some_and(|l| l.trim_start().to_ascii_lowercase().starts_with("version"));
        if !comeca {
            return linhas;
        }
        // Pilha: índice do símbolo (0 é o módulo) e classe.
        let mut pilha: Vec<(usize, String)> = Vec::new();
        let mut propriedades = 0u32;
        let mut aberto = false;
        for (n, l) in sep.codigo.iter().enumerate() {
            let num = n as u32 + 1;
            let t = l.trim();
            let low = t.to_ascii_lowercase();
            if aberto && pilha.is_empty() {
                break;
            }
            linhas.insert(num);
            let palavras: Vec<&str> = t.split_whitespace().collect();
            let primeira = low.split_whitespace().next().unwrap_or("");
            match primeira {
                "begin" => {
                    aberto = true;
                    let classe = palavras.get(1).copied().unwrap_or("").to_string();
                    match (pilha.is_empty(), palavras.get(2)) {
                        (false, Some(nome)) => {
                            let pai = self.saida.simbolos[pilha.last().map_or(0, |p| p.0)]
                                .qualificado
                                .clone();
                            let idx = self.novo(
                                nome,
                                TipoSimbolo::Componente,
                                Linhas::new(num, num),
                                &pai,
                            );
                            self.saida.simbolos[idx].detalhe = Some(classe.clone());
                            self.controles.entry(nome.to_lowercase()).or_insert(idx);
                            pilha.push((idx, classe));
                        }
                        _ => {
                            if !classe.is_empty() {
                                self.saida.simbolos[0].detalhe = Some(classe.clone());
                            }
                            pilha.push((0, classe));
                        }
                    }
                }
                "beginproperty" => propriedades += 1,
                "endproperty" => propriedades = propriedades.saturating_sub(1),
                "end" if palavras.len() == 1 && propriedades == 0 => {
                    if let Some((idx, _)) = pilha.pop()
                        && idx != 0
                    {
                        let s = &mut self.saida.simbolos[idx];
                        s.linhas = Linhas::new(s.linhas.inicio, num);
                    }
                }
                _ if propriedades == 0 && low.starts_with("caption") && t.contains('=') => {
                    let legenda: String = sep
                        .textos
                        .iter()
                        .filter(|(l, _)| *l == num)
                        .map(|(_, x)| x.as_str())
                        .collect();
                    if let Some((idx, classe)) = pilha.last()
                        && !legenda.trim().is_empty()
                    {
                        self.saida.simbolos[*idx].detalhe =
                            Some(resumir(&format!("{classe}: {}", legenda.trim())));
                    }
                }
                _ => {}
            }
        }
        linhas
    }

    fn codigo(&mut self, logicas: &[Logica], sep: &Separado, classe: bool) {
        let raiz = self.raiz.clone();
        // Procedimento aberto: (índice, palavra que o fecha).
        let mut proc_aberto: Option<(usize, &'static str)> = None;
        // Type ou Enum aberto: (índice, é enum).
        let mut bloco: Option<(usize, bool)> = None;
        for (ini, fim, l) in logicas {
            let t = l.trim();
            if t.is_empty() {
                continue;
            }
            let low = t.to_ascii_lowercase();
            if let Some((idx, fecha)) = proc_aberto {
                if low.starts_with("end ") && low[4..].trim_start().starts_with(fecha) {
                    let s = &mut self.saida.simbolos[idx];
                    s.linhas = Linhas::new(s.linhas.inicio, *fim);
                    proc_aberto = None;
                } else {
                    let q = self.saida.simbolos[idx].qualificado.clone();
                    self.corpos.push((*ini, t.to_string(), q));
                }
                continue;
            }
            if let Some((idx, enum_)) = bloco {
                let fecha = if enum_ { "enum" } else { "type" };
                if low.starts_with("end ") && low[4..].trim_start().starts_with(fecha) {
                    let s = &mut self.saida.simbolos[idx];
                    s.linhas = Linhas::new(s.linhas.inicio, *fim);
                    bloco = None;
                } else if let Some(nome) = primeiro_nome(t) {
                    let q = self.saida.simbolos[idx].qualificado.clone();
                    let tipo = if enum_ {
                        TipoSimbolo::Constante
                    } else {
                        TipoSimbolo::Campo
                    };
                    let i = self.novo(nome, tipo, Linhas::new(*ini, *fim), &q);
                    let resto = t[nome.len()..].trim();
                    self.saida.simbolos[i].detalhe = (!resto.is_empty()).then(|| resumir(resto));
                }
                continue;
            }
            let comentario = comentario_antes(&sep.comentarios, *ini);
            if let Some(c) = DECLARE.captures(t) {
                let tipo = if c[1].eq_ignore_ascii_case("sub") {
                    TipoSimbolo::Rotina
                } else {
                    TipoSimbolo::Funcao
                };
                let i = self.novo(&c[2], tipo, Linhas::new(*ini, *fim), &raiz);
                let s = &mut self.saida.simbolos[i];
                s.declaracao = true;
                s.detalhe = Some(resumir(&original(sep, *ini, *fim, t)));
                s.comentario = comentario;
            } else if let Some(c) = PROCEDIMENTO.captures(t) {
                let kw = c[1].to_ascii_lowercase();
                let (tipo, fecha) = if kw.starts_with("property") {
                    (TipoSimbolo::Propriedade, "property")
                } else if classe {
                    (
                        TipoSimbolo::Metodo,
                        if kw == "sub" { "sub" } else { "function" },
                    )
                } else if kw == "sub" {
                    (TipoSimbolo::Rotina, "sub")
                } else {
                    (TipoSimbolo::Funcao, "function")
                };
                let i = self.novo(&c[2], tipo, Linhas::new(*ini, *fim), &raiz);
                let s = &mut self.saida.simbolos[i];
                s.detalhe = Some(resumir(t));
                s.comentario = comentario;
                proc_aberto = Some((i, fecha));
            } else if let Some(c) = BLOCO.captures(t) {
                let enum_ = c[1].eq_ignore_ascii_case("enum");
                let tipo = if enum_ {
                    TipoSimbolo::Enum
                } else {
                    TipoSimbolo::Tipo
                };
                let i = self.novo(&c[2], tipo, Linhas::new(*ini, *fim), &raiz);
                self.saida.simbolos[i].comentario = comentario;
                bloco = Some((i, enum_));
            } else if let Some(c) = CONSTANTE.captures(t) {
                for parte in dividir(c.get(1).map_or("", |m| m.as_str())) {
                    if let Some(nome) = primeiro_nome(parte) {
                        let i =
                            self.novo(nome, TipoSimbolo::Constante, Linhas::new(*ini, *fim), &raiz);
                        let d = original(sep, *ini, *fim, parte.trim());
                        let d = d.trim()[nome.len().min(d.trim().len())..]
                            .trim()
                            .to_string();
                        let s = &mut self.saida.simbolos[i];
                        s.detalhe = (!d.is_empty()).then(|| resumir(&d));
                        s.comentario = comentario.clone();
                    }
                }
            } else if let Some(c) = IMPLEMENTS.captures(t) {
                self.referir(&raiz, &c[1], TipoReferencia::Implementacao, *ini);
            } else if low.starts_with("public event") || low.starts_with("event ") {
                continue;
            } else if let Some(c) = VARIAVEL.captures(t) {
                for parte in dividir(c.get(1).map_or("", |m| m.as_str())) {
                    if let Some(nome) = primeiro_nome(parte) {
                        let i =
                            self.novo(nome, TipoSimbolo::Variavel, Linhas::new(*ini, *fim), &raiz);
                        let resto = parte.trim()[nome.len()..].trim();
                        let s = &mut self.saida.simbolos[i];
                        s.detalhe = (!resto.is_empty()).then(|| resumir(resto));
                        s.comentario = comentario.clone();
                    }
                }
            }
        }
        if let Some(idx) = proc_aberto.map(|p| p.0).or(bloco.map(|b| b.0)) {
            let fim = logicas.last().map_or(1, |l| l.1);
            let s = &mut self.saida.simbolos[idx];
            s.linhas = Linhas::new(s.linhas.inicio, fim);
        }
    }

    /// `cmdOk_Click` é o evento `Click` do controle `cmdOk`.
    fn eventos(&mut self) {
        let mut novas = Vec::new();
        for s in &self.saida.simbolos {
            if !matches!(s.tipo, TipoSimbolo::Metodo | TipoSimbolo::Rotina) {
                continue;
            }
            let Some((controle, _)) = s.nome.rsplit_once('_') else {
                continue;
            };
            if let Some(&idx) = self.controles.get(&controle.to_lowercase()) {
                let c = &self.saida.simbolos[idx];
                novas.push(Referencia {
                    origem: Some(c.qualificado.clone()),
                    alvo: s.qualificado.clone(),
                    tipo: TipoReferencia::Chamada,
                    linha: c.linhas.inicio,
                });
            }
        }
        self.saida.referencias.extend(novas);
    }

    fn referencias_dos_corpos(&mut self) {
        let nomes = |f: &dyn Fn(TipoSimbolo) -> bool| -> HashSet<String> {
            self.saida
                .simbolos
                .iter()
                .filter(|s| f(s.tipo))
                .map(|s| s.nome.to_lowercase())
                .collect()
        };
        let rotinas = nomes(&|t| {
            matches!(
                t,
                TipoSimbolo::Rotina
                    | TipoSimbolo::Funcao
                    | TipoSimbolo::Metodo
                    | TipoSimbolo::Propriedade
            )
        });
        let globais = nomes(&|t| matches!(t, TipoSimbolo::Constante | TipoSimbolo::Variavel));
        let mut vistos: HashSet<(u32, String)> = HashSet::new();
        let corpos = std::mem::take(&mut self.corpos);
        for (linha, texto, origem) in corpos {
            let inicio = texto.len() - texto.trim_start().len();
            let mut anterior = String::new();
            for m in IDENT.find_iter(&texto) {
                let nome = m.as_str();
                let low = nome.to_lowercase();
                let antes = texto[..m.start()].trim_end();
                let depois = texto[m.end()..].trim_start();
                let membro = antes.ends_with('.');
                let prev = std::mem::replace(&mut anterior, low.clone());
                if membro || origem.to_lowercase().ends_with(&format!(".{low}")) {
                    continue;
                }
                let mut alvo = nome;
                let tipo = if prev == "new" {
                    // `New ADODB.Recordset`: o nome inteiro.
                    alvo = QUALIFICADO
                        .find(&texto[m.start()..])
                        .map_or(nome, |q| q.as_str());
                    TipoReferencia::Instanciacao
                } else if RESERVADAS.contains(&low.as_str()) {
                    continue;
                } else if rotinas.contains(&low) {
                    TipoReferencia::Chamada
                } else if globais.contains(&low) {
                    TipoReferencia::Uso
                } else {
                    let instrucao = (m.start() == inicio || prev == "call")
                        && !depois.starts_with('=')
                        && !depois.starts_with('.')
                        && !depois.starts_with(':');
                    if (instrucao || depois.starts_with('(')) && !NATIVAS.contains(&low.as_str()) {
                        TipoReferencia::Mencao
                    } else {
                        continue;
                    }
                };
                if vistos.insert((linha, low)) {
                    self.referir(&origem, alvo, tipo, linha);
                }
            }
        }
    }
}

/// Texto original (com as strings) das linhas `ini..=fim`, ou `padrao`.
fn original(sep: &Separado, ini: u32, fim: u32, padrao: &str) -> String {
    let textos: Vec<&str> = sep
        .textos
        .iter()
        .filter(|(l, _)| (ini..=fim).contains(l))
        .map(|(_, t)| t.as_str())
        .collect();
    if textos.is_empty() {
        return padrao.to_string();
    }
    // Recoloca as strings nos espaços entre aspas, na ordem.
    let mut saida = String::new();
    let mut partes = padrao.split('"');
    let mut k = 0;
    if let Some(p) = partes.next() {
        saida.push_str(p);
    }
    let mut dentro = false;
    for p in partes {
        saida.push('"');
        dentro = !dentro;
        if dentro {
            saida.push_str(textos.get(k).copied().unwrap_or(p));
            k += 1;
        } else {
            saida.push_str(p);
        }
    }
    saida
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extrair(caminho: &str, texto: &str) -> Extracao {
        ExtratorVb6
            .extrair(&Fonte {
                caminho,
                linguagem: Linguagem::VisualBasic6,
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

    const FORM: &str = "VERSION 5.00\r
Begin VB.Form frmCancelamento \r
   Caption         =   \"Cancelamento de Fatura\"\r
   Begin VB.Frame fraDados \r
      Caption         =   \"Dados\"\r
      BeginProperty Font \r
         Name            =   \"MS Sans Serif\"\r
      EndProperty\r
      Begin VB.TextBox txtFatura \r
         Text            =   \"\"\r
      End\r
   End\r
   Begin VB.CommandButton cmdCancelar \r
      Caption         =   \"Cancelar fatura\"\r
   End\r
End\r
Attribute VB_Name = \"frmCancelamento\"\r
Option Explicit\r
\r
Private Const DIAS_LIMITE As Integer = 30, PERFIL = \"SUPERVISOR\"\r
Private mUsuario As String\r
\r
' Cancela a fatura informada\r
Private Sub cmdCancelar_Click()\r
    Dim rs As ADODB.Recordset\r
    If Not ValidarCancelamento(CLng(txtFatura.Text)) Then Exit Sub\r
    Set rs = New ADODB.Recordset\r
    Call Executar(\"PKG_COBRANCA.PRC_CANCELAR_FATURA\", _\r
        mUsuario)\r
    RegistrarLog \"Fatura cancelada\"\r
    MsgBox \"Fatura cancelada.\"\r
End Sub\r
\r
Private Function ValidarCancelamento(ByVal Id As Long) As Boolean\r
    ValidarCancelamento = DiasDesdeEmissao(Id) <= DIAS_LIMITE\r
End Function\r
";

    #[test]
    fn formulario() {
        use TipoSimbolo as T;
        let e = extrair("vb/frmCancelamento.frm", FORM);
        let r = resumo(&e);
        assert_eq!(
            r,
            [
                ("frmCancelamento", T::Tela, Linhas::new(1, 36), false),
                (
                    "frmCancelamento.fraDados",
                    T::Componente,
                    Linhas::new(4, 12),
                    false
                ),
                (
                    "frmCancelamento.fraDados.txtFatura",
                    T::Componente,
                    Linhas::new(9, 11),
                    false
                ),
                (
                    "frmCancelamento.cmdCancelar",
                    T::Componente,
                    Linhas::new(13, 15),
                    false
                ),
                (
                    "frmCancelamento.DIAS_LIMITE",
                    T::Constante,
                    Linhas::new(20, 20),
                    false
                ),
                (
                    "frmCancelamento.PERFIL",
                    T::Constante,
                    Linhas::new(20, 20),
                    false
                ),
                (
                    "frmCancelamento.mUsuario",
                    T::Variavel,
                    Linhas::new(21, 21),
                    false
                ),
                (
                    "frmCancelamento.cmdCancelar_Click",
                    T::Metodo,
                    Linhas::new(24, 32),
                    false
                ),
                (
                    "frmCancelamento.ValidarCancelamento",
                    T::Metodo,
                    Linhas::new(34, 36),
                    false
                ),
            ]
        );
        assert_eq!(
            e.simbolos[0].detalhe.as_deref(),
            Some("VB.Form: Cancelamento de Fatura")
        );
        assert_eq!(
            e.simbolos[3].detalhe.as_deref(),
            Some("VB.CommandButton: Cancelar fatura")
        );
        assert_eq!(e.simbolos[5].detalhe.as_deref(), Some("= \"SUPERVISOR\""));
        assert_eq!(
            e.simbolos[7].comentario.as_deref(),
            Some("Cancela a fatura informada")
        );

        let r = refs(&e);
        let clique = Some("frmCancelamento.cmdCancelar_Click");
        let validar = Some("frmCancelamento.ValidarCancelamento");
        for esperado in [
            (
                Some("frmCancelamento.cmdCancelar"),
                "frmCancelamento.cmdCancelar_Click",
                TipoReferencia::Chamada,
                13,
            ),
            (clique, "ValidarCancelamento", TipoReferencia::Chamada, 26),
            (clique, "ADODB.Recordset", TipoReferencia::Instanciacao, 27),
            (clique, "Executar", TipoReferencia::Mencao, 28),
            (
                clique,
                "PKG_COBRANCA.PRC_CANCELAR_FATURA",
                TipoReferencia::Mencao,
                28,
            ),
            (clique, "mUsuario", TipoReferencia::Uso, 28),
            (clique, "RegistrarLog", TipoReferencia::Mencao, 30),
            (validar, "DiasDesdeEmissao", TipoReferencia::Mencao, 35),
            (validar, "DIAS_LIMITE", TipoReferencia::Uso, 35),
        ] {
            assert!(r.contains(&esperado), "{esperado:?} fora de {r:#?}");
        }
        // Legendas, mensagens, nativas e o designer não viram menção.
        for nome in [
            "Fatura",
            "MsgBox",
            "CLng",
            "VB_Name",
            "frmCancelamento",
            "Font",
            "Id",
        ] {
            assert!(!r.iter().any(|x| x.1 == nome), "{nome} em {r:#?}");
        }
    }

    #[test]
    fn modulo_classe_e_projeto() {
        use TipoSimbolo as T;
        let e = extrair(
            "vb/modFatura.bas",
            "Attribute VB_Name = \"modFatura\"\nOption Explicit\n\nPublic Declare Function GetTickCount Lib \"kernel32\" () As Long\n\nPublic Type TFatura\n    Id As Long\n    Valor As Currency\nEnd Type\n\nPublic Enum EStatus\n    stAberta = 1\n    stPaga\nEnd Enum\n\n' Calcula a multa\nPublic Function CalcularMulta(ByVal V As Currency) As Currency\n    CalcularMulta = V * 0.02\nEnd Function\n\nPublic Sub Registrar()\n    Dim f As TFatura\n    f.Valor = CalcularMulta(f.Valor)\nEnd Sub\n",
        );
        let r = resumo(&e);
        assert_eq!(
            r,
            [
                ("modFatura", T::Modulo, Linhas::new(1, 24), false),
                ("modFatura.GetTickCount", T::Funcao, Linhas::new(4, 4), true),
                ("modFatura.TFatura", T::Tipo, Linhas::new(6, 9), false),
                ("modFatura.TFatura.Id", T::Campo, Linhas::new(7, 7), false),
                (
                    "modFatura.TFatura.Valor",
                    T::Campo,
                    Linhas::new(8, 8),
                    false
                ),
                ("modFatura.EStatus", T::Enum, Linhas::new(11, 14), false),
                (
                    "modFatura.EStatus.stAberta",
                    T::Constante,
                    Linhas::new(12, 12),
                    false
                ),
                (
                    "modFatura.EStatus.stPaga",
                    T::Constante,
                    Linhas::new(13, 13),
                    false
                ),
                (
                    "modFatura.CalcularMulta",
                    T::Funcao,
                    Linhas::new(17, 19),
                    false
                ),
                ("modFatura.Registrar", T::Rotina, Linhas::new(21, 24), false),
            ]
        );
        assert_eq!(
            e.simbolos[1].detalhe.as_deref(),
            Some("Public Declare Function GetTickCount Lib \"kernel32\" () As Long")
        );
        assert_eq!(e.simbolos[8].comentario.as_deref(), Some("Calcula a multa"));
        assert!(refs(&e).contains(&(
            Some("modFatura.Registrar"),
            "CalcularMulta",
            TipoReferencia::Chamada,
            23
        )));

        let e = extrair(
            "vb/clsRegra.cls",
            "VERSION 1.0 CLASS\nBEGIN\n  MultiUse = -1  'True\nEND\nAttribute VB_Name = \"clsRegra\"\nImplements IRegra\nPrivate mTaxa As Double\nPublic Property Get Taxa() As Double\n    Taxa = mTaxa\nEnd Property\n",
        );
        assert_eq!(
            resumo(&e),
            [
                ("clsRegra", T::Classe, Linhas::new(1, 10), false),
                ("clsRegra.mTaxa", T::Variavel, Linhas::new(7, 7), false),
                ("clsRegra.Taxa", T::Propriedade, Linhas::new(8, 10), false),
            ]
        );
        assert!(refs(&e).contains(&(Some("clsRegra"), "IRegra", TipoReferencia::Implementacao, 6)));

        let e = extrair(
            "vb/Faturamento.vbp",
            "Type=Exe\nForm=frmCancelamento.frm\nModule=modFatura; modFatura.bas\nClass=clsRegra; clsRegra.cls\nName=\"Faturamento\"\n",
        );
        assert_eq!(
            resumo(&e),
            [("Faturamento", T::Programa, Linhas::new(1, 5), false)]
        );
        assert_eq!(
            refs(&e),
            [
                (
                    Some("Faturamento"),
                    "frmCancelamento",
                    TipoReferencia::Importacao,
                    2
                ),
                (
                    Some("Faturamento"),
                    "modFatura",
                    TipoReferencia::Importacao,
                    3
                ),
                (
                    Some("Faturamento"),
                    "clsRegra",
                    TipoReferencia::Importacao,
                    4
                ),
            ]
        );
    }

    #[test]
    fn windows_1252_e_texto_estranho() {
        let bytes = b"Attribute VB_Name = \"modAviso\"\r\n' Situa\xe7\xe3o da fatura\r\nPublic Sub Avisar()\r\n    MsgBox \"Opera\xe7\xe3o inv\xe1lida\"\r\nEnd Sub\r\n";
        let texto = String::from_utf8_lossy(bytes);
        let e = extrair("modAviso.bas", &texto);
        assert_eq!(e.simbolos[1].nome, "Avisar");
        assert_eq!(e.simbolos[1].linhas, Linhas::new(3, 5));
        assert!(e.simbolos[1].comentario.is_some());
        for t in [
            "",
            "VERSION",
            "Begin",
            "Sub",
            "Public Sub X(",
            "End Sub",
            "x _",
            "Type T",
        ] {
            extrair("x.frm", t);
            extrair("x.vbp", t);
        }
    }
}
