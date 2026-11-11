//! Extrator de COBOL, inclusive copybooks e SQL embutido (Pro*COBOL, DB2).
//!
//! Lê o formato fixo (área de sequência nas colunas 1 a 6, indicador na 7, código
//! da 8 à 72) e o formato livre (`>>SOURCE FORMAT FREE` ou código desde a coluna
//! 1), com um léxico próprio. Reconhece:
//!
//! - o programa (`PROGRAM-ID`, do `IDENTIFICATION DIVISION` ao `END PROGRAM` ou
//!   ao fim do arquivo) e, num arquivo sem `PROGRAM-ID`, o copybook, com o nome
//!   do arquivo;
//! - as divisões, as seções e os parágrafos da `PROCEDURE DIVISION`;
//! - os itens de dados: `FD`/`SD`, níveis `01` e `77` (registro ou variável),
//!   `02` a `49` (campo, dentro do grupo), `66` e `88` (condição);
//! - referências: `PERFORM` (com `THRU`) e `GO TO` (chamada de parágrafo ou
//!   seção), `CALL 'PROGRAMA'` (chamada), `COPY` e `EXEC SQL INCLUDE` (inclusão),
//!   uso dos itens de dados do próprio arquivo na `PROCEDURE DIVISION` (uso) e
//!   menção aos nomes com hífen que não são do arquivo (vêm de copybooks);
//! - o SQL de `EXEC SQL ... END-EXEC`, passado ao extrator de SQL (leitura,
//!   escrita, chamada) e às menções do genérico.
//!
//! Comentários (indicador `*` ou `/`, `*>`) logo antes do programa, da seção ou do
//! parágrafo viram a documentação dele. Bytes de Windows-1252 chegam como
//! `U+FFFD`, um caractere por byte, e não deslocam as colunas.

use std::collections::{BTreeMap, HashSet};

use super::generico::{Separado, mencoes};
use super::sql::{ExtratorSql, resumir};
use super::{Extracao, Extrator, Fonte, Linhas, Referencia, Simbolo, TipoReferencia, TipoSimbolo};
use crate::linguagem::Linguagem;

/// Extrator de COBOL.
#[derive(Debug, Clone, Copy, Default)]
pub struct ExtratorCobol;

impl Extrator for ExtratorCobol {
    fn nome(&self) -> &'static str {
        "cobol"
    }

    fn versao(&self) -> u32 {
        1
    }

    fn linguagens(&self) -> &'static [Linguagem] {
        &[Linguagem::Cobol]
    }

    fn extrair(&self, fonte: &Fonte<'_>) -> Result<Extracao, String> {
        let lex = lexico(fonte.texto);
        let mut a = Analisador {
            t: &lex.tokens,
            comentarios: &lex.comentarios,
            saida: Extracao::default(),
            pilha: Vec::new(),
            divisao: Divisao::Nenhuma,
            dados: HashSet::new(),
            vistos: HashSet::new(),
        };
        a.arquivo(fonte.caminho);
        Ok(a.saida)
    }
}

// ---------------------------------------------------------------------------
// Léxico

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tipo {
    Palavra,
    /// Literal, sem as aspas.
    Texto,
    /// Ponto final de sentença (seguido de espaço ou do fim da linha).
    Ponto,
    Sim(char),
}

#[derive(Debug, Clone)]
struct Token {
    tipo: Tipo,
    texto: String,
    /// Palavra em maiúsculas; vazio nos demais.
    chave: String,
    linha: u32,
    /// Começa na área A (colunas 8 a 11 no formato fixo; sempre no livre).
    area_a: bool,
    /// Primeiro token da linha.
    primeiro: bool,
    /// Havia espaço antes.
    espaco: bool,
}

struct Lexico {
    tokens: Vec<Token>,
    /// Comentários de linha inteira, por linha.
    comentarios: BTreeMap<u32, String>,
}

/// Formato livre: diretiva explícita ou código que começa antes da coluna 8.
fn formato_livre(texto: &str) -> bool {
    for linha in texto.lines().take(200) {
        let t = linha.trim_start();
        let up = t.to_ascii_uppercase();
        if up.starts_with(">>SOURCE") || up.contains("$SET SOURCEFORMAT\"FREE\"") {
            return up.contains("FREE");
        }
        let recuo = linha.chars().take_while(|c| c.is_whitespace()).count();
        if t.is_empty() || recuo >= 6 {
            continue;
        }
        if t.starts_with("*>") {
            return true;
        }
        let primeira: String = up
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '-')
            .collect();
        let resto = &up[primeira.len()..];
        let nivel = (1..=2).contains(&primeira.len())
            && primeira.chars().all(|c| c.is_ascii_digit())
            && resto.starts_with([' ', '\t']);
        if nivel
            || matches!(
                primeira.as_str(),
                "IDENTIFICATION" | "ID" | "PROGRAM-ID" | "COPY" | "EXEC" | "PROCEDURE" | "DATA"
            )
        {
            return true;
        }
    }
    false
}

fn lexico(texto: &str) -> Lexico {
    let livre = formato_livre(texto);
    let mut tokens = Vec::new();
    let mut comentarios = BTreeMap::new();
    for (n, linha) in texto.lines().enumerate() {
        let num = n as u32 + 1;
        let c: Vec<char> = linha.chars().collect();
        let (inicio, fim) = if livre {
            (0, c.len())
        } else {
            if c.len() < 7 {
                continue;
            }
            match c[6] {
                '*' | '/' => {
                    let fim = c.len().min(72);
                    comentarios.insert(num, c[7.min(fim)..fim].iter().collect());
                    continue;
                }
                _ => (7, c.len().min(72)),
            }
        };
        let area: String = c[inicio..fim].iter().collect();
        let t = area.trim_start();
        if let Some(resto) = t.strip_prefix("*>") {
            comentarios.insert(num, resto.to_string());
            continue;
        }
        if t.starts_with(">>") || t.starts_with('$') {
            continue;
        }
        lexico_linha(&c[inicio..fim], num, inicio + 1, livre, &mut tokens);
    }
    Lexico {
        tokens,
        comentarios,
    }
}

fn caractere_de_palavra(c: char) -> bool {
    c.is_alphanumeric() || c == '-' || c == '_' || c == '\u{FFFD}'
}

fn lexico_linha(c: &[char], linha: u32, coluna0: usize, livre: bool, tokens: &mut Vec<Token>) {
    let mut i = 0;
    let mut primeiro = true;
    let mut espaco = true;
    let mut push = |tipo, texto: String, coluna: usize, primeiro: &mut bool, espaco: &mut bool| {
        let chave = if tipo == Tipo::Palavra {
            texto.to_uppercase()
        } else {
            String::new()
        };
        tokens.push(Token {
            tipo,
            texto,
            chave,
            linha,
            area_a: livre || coluna0 + coluna <= 11,
            primeiro: *primeiro,
            espaco: *espaco,
        });
        *primeiro = false;
        *espaco = false;
    };
    while i < c.len() {
        let ch = c[i];
        if ch.is_whitespace() || ch == ',' || ch == ';' {
            espaco = true;
            i += 1;
            continue;
        }
        if ch == '*' && c.get(i + 1) == Some(&'>') {
            break;
        }
        if ch == '\'' || ch == '"' {
            let ini = i;
            let mut s = String::new();
            i += 1;
            while i < c.len() {
                if c[i] == ch {
                    if c.get(i + 1) == Some(&ch) {
                        s.push(ch);
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                s.push(c[i]);
                i += 1;
            }
            push(Tipo::Texto, s, ini, &mut primeiro, &mut espaco);
            continue;
        }
        if ch == '.' && c.get(i + 1).is_none_or(|p| p.is_whitespace()) {
            push(Tipo::Ponto, ".".into(), i, &mut primeiro, &mut espaco);
            i += 1;
            continue;
        }
        if caractere_de_palavra(ch) {
            let ini = i;
            while i < c.len()
                && (caractere_de_palavra(c[i])
                    || (c[i] == '.' && c.get(i + 1).is_some_and(|p| p.is_alphanumeric())))
            {
                i += 1;
            }
            let s: String = c[ini..i].iter().collect();
            push(Tipo::Palavra, s, ini, &mut primeiro, &mut espaco);
            continue;
        }
        push(Tipo::Sim(ch), ch.to_string(), i, &mut primeiro, &mut espaco);
        i += 1;
    }
}

// ---------------------------------------------------------------------------
// Análise

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Divisao {
    Nenhuma,
    Identificacao,
    Ambiente,
    Dados,
    Procedimentos,
}

/// Profundidade na árvore de símbolos: quem abre num nível fecha os abertos no
/// mesmo nível ou abaixo.
const NIVEL_PROGRAMA: u32 = 0;
const NIVEL_DIVISAO: u32 = 1;
const NIVEL_SECAO: u32 = 2;
const NIVEL_PARAGRAFO: u32 = 3;
const NIVEL_ARQUIVO: u32 = 3;
const NIVEL_DADO: u32 = 4;

/// Verbos e palavras que, sozinhos numa sentença, não são parágrafo.
const VERBOS: &[&str] = &[
    "ACCEPT",
    "ADD",
    "ALTER",
    "CALL",
    "CANCEL",
    "CLOSE",
    "COMPUTE",
    "CONTINUE",
    "DECLARATIVES",
    "DELETE",
    "DISPLAY",
    "DIVIDE",
    "EJECT",
    "ELSE",
    "END",
    "EVALUATE",
    "EXIT",
    "GO",
    "GOBACK",
    "IF",
    "INITIALIZE",
    "INSPECT",
    "MERGE",
    "MOVE",
    "MULTIPLY",
    "OPEN",
    "PERFORM",
    "READ",
    "RELEASE",
    "RETURN",
    "REWRITE",
    "SEARCH",
    "SET",
    "SKIP1",
    "SKIP2",
    "SKIP3",
    "SORT",
    "START",
    "STOP",
    "STRING",
    "SUBTRACT",
    "UNSTRING",
    "WRITE",
];

/// Palavras reservadas com hífen, que não são nome de dado.
const RESERVADAS_COM_HIFEN: &[&str] = &[
    "CURRENT-DATE",
    "DATE-COMPILED",
    "DATE-WRITTEN",
    "DAY-OF-WEEK",
    "DECIMAL-POINT",
    "FILE-CONTROL",
    "I-O",
    "I-O-CONTROL",
    "INPUT-OUTPUT",
    "LINE-COUNTER",
    "LOCAL-STORAGE",
    "LOWER-CASE",
    "NO-ECHO",
    "OBJECT-COMPUTER",
    "PAGE-COUNTER",
    "PROGRAM-ID",
    "RETURN-CODE",
    "SORT-RETURN",
    "SOURCE-COMPUTER",
    "SPECIAL-NAMES",
    "UPPER-CASE",
    "WHEN-COMPILED",
    "WORKING-STORAGE",
];

struct Analisador<'a> {
    t: &'a [Token],
    comentarios: &'a BTreeMap<u32, String>,
    saida: Extracao,
    /// Símbolos abertos: (nível, índice em `saida.simbolos`).
    pilha: Vec<(u32, usize)>,
    divisao: Divisao,
    /// Nomes (em maiúsculas) dos itens de dados definidos no arquivo.
    dados: HashSet<String>,
    /// Referências já registradas: (linha, alvo em maiúsculas, tipo).
    vistos: HashSet<(u32, String, TipoReferencia)>,
}

impl Analisador<'_> {
    fn chave(&self, i: usize) -> &str {
        self.t.get(i).map_or("", |t| t.chave.as_str())
    }

    fn tipo(&self, i: usize) -> Option<Tipo> {
        self.t.get(i).map(|t| t.tipo)
    }

    /// Próximo ponto final a partir de `i` (ou o fim).
    fn ponto(&self, i: usize) -> usize {
        (i..self.t.len())
            .find(|&k| self.t[k].tipo == Tipo::Ponto)
            .unwrap_or(self.t.len())
    }

    /// Linha do último token antes de `i`.
    fn linha_antes(&self, i: usize) -> u32 {
        i.checked_sub(1)
            .and_then(|k| self.t.get(k))
            .map_or(1, |t| t.linha)
    }

    fn origem(&self) -> Option<String> {
        self.pilha
            .last()
            .map(|&(_, i)| self.saida.simbolos[i].qualificado.clone())
    }

    fn fechar(&mut self, nivel: u32, fim: u32) {
        while let Some(&(n, i)) = self.pilha.last() {
            if n < nivel {
                break;
            }
            let s = &mut self.saida.simbolos[i];
            s.linhas = Linhas::new(s.linhas.inicio, fim);
            self.pilha.pop();
        }
    }

    /// Abre um símbolo no `nivel`, fechando os que ele encerra.
    fn abrir(&mut self, nivel: u32, mut s: Simbolo, i: usize) {
        let fim = self.linha_antes(i);
        self.fechar(nivel, fim);
        if let Some(pai) = self.origem() {
            s = s.dentro_de(&pai);
        }
        if s.comentario.is_none() {
            s.comentario = comentario_antes(self.comentarios, s.linhas.inicio);
        }
        self.pilha.push((nivel, self.saida.simbolos.len()));
        self.saida.simbolos.push(s);
    }

    fn referir(&mut self, alvo: &str, tipo: TipoReferencia, linha: u32) {
        let alvo = alvo.trim();
        if alvo.is_empty() || !self.vistos.insert((linha, alvo.to_uppercase(), tipo)) {
            return;
        }
        let origem = self.origem();
        self.saida.referencias.push(Referencia {
            origem,
            alvo: alvo.to_string(),
            tipo,
            linha,
        });
    }

    fn arquivo(&mut self, caminho: &str) {
        let Some(primeiro) = self.t.first() else {
            return;
        };
        let tem_programa = self.t.iter().any(|t| t.chave == "PROGRAM-ID");
        if !tem_programa {
            // Copybook: um símbolo com o nome do arquivo envolve o conteúdo.
            let nome = caminho.rsplit('/').next().unwrap_or(caminho);
            let nome = nome.rsplit_once('.').map_or(nome, |(b, _)| b);
            let linha = primeiro.linha;
            self.abrir(
                NIVEL_PROGRAMA,
                Simbolo::novo(nome, TipoSimbolo::Copybook, Linhas::new(linha, linha)),
                0,
            );
            self.divisao = if eh_nivel(primeiro) || primeiro.chave == "FD" {
                Divisao::Dados
            } else {
                Divisao::Procedimentos
            };
        }
        let mut inicio_ident: Option<u32> = None;
        let mut i = 0;
        let mut sentenca = true;
        while i < self.t.len() {
            let antes = i;
            i = self
                .passo(i, sentenca, &mut inicio_ident)
                .clamp(antes + 1, self.t.len());
            sentenca = self.t[i - 1].tipo == Tipo::Ponto;
        }
        let fim = self.linha_antes(self.t.len());
        self.fechar(NIVEL_PROGRAMA, fim);
        self.saida.referencias.sort_by_key(|r| r.linha);
    }

    /// Trata o token `i` e devolve o índice do próximo.
    fn passo(&mut self, i: usize, sentenca: bool, inicio_ident: &mut Option<u32>) -> usize {
        let tk = &self.t[i];
        let linha = tk.linha;
        let k = tk.chave.clone();
        if tk.tipo != Tipo::Palavra {
            return i + 1;
        }
        // Em qualquer lugar.
        if (k == "EXEC" || k == "EXECUTE") && self.tipo(i + 1) == Some(Tipo::Palavra) {
            return self.exec(i);
        }
        if k == "COPY" && matches!(self.tipo(i + 1), Some(Tipo::Palavra | Tipo::Texto)) {
            let nome = &self.t[i + 1].texto;
            let nome = if self.t[i + 1].tipo == Tipo::Texto {
                let base = nome.rsplit(['/', '\\']).next().unwrap_or(nome);
                base.rsplit_once('.').map_or(base, |(b, _)| b).to_string()
            } else {
                nome.clone()
            };
            self.referir(&nome, TipoReferencia::Inclusao, linha);
            return self.ponto(i);
        }
        if sentenca && let Some(prox) = self.cabecalho(i, inicio_ident) {
            return prox;
        }
        if self.divisao == Divisao::Procedimentos {
            return self.instrucao(i);
        }
        i + 1
    }

    /// Divisões, `PROGRAM-ID`, `END PROGRAM`, seções, itens de dados e parágrafos.
    fn cabecalho(&mut self, i: usize, inicio_ident: &mut Option<u32>) -> Option<usize> {
        let tk = &self.t[i];
        let linha = tk.linha;
        let k = tk.chave.as_str();
        let seguinte = self.chave(i + 1);
        if seguinte == "DIVISION" {
            let ponto = self.ponto(i);
            match k {
                "IDENTIFICATION" | "ID" => {
                    let fim = self.linha_antes(i);
                    self.fechar(NIVEL_PROGRAMA, fim);
                    *inicio_ident = Some(linha);
                    self.divisao = Divisao::Identificacao;
                }
                "ENVIRONMENT" | "DATA" | "PROCEDURE" => {
                    self.divisao = match k {
                        "ENVIRONMENT" => Divisao::Ambiente,
                        "DATA" => Divisao::Dados,
                        _ => Divisao::Procedimentos,
                    };
                    let mut s = Simbolo::novo(
                        format!("{k} DIVISION"),
                        TipoSimbolo::Secao,
                        Linhas::new(linha, linha),
                    );
                    s.detalhe = Some("divisão".into());
                    let using = self.juntar(i + 2, ponto);
                    if !using.is_empty() {
                        s.detalhe = Some(format!("divisão; {using}"));
                    }
                    self.abrir(NIVEL_DIVISAO, s, i);
                }
                _ => return None,
            }
            return Some(ponto);
        }
        if k == "PROGRAM-ID" {
            let mut j = i + 1;
            if self.tipo(j) == Some(Tipo::Ponto) {
                j += 1;
            }
            let nome = self.t.get(j)?.texto.clone();
            let ini = inicio_ident.take().unwrap_or(linha);
            let primeiro = self.t.iter().position(|t| t.linha >= ini).unwrap_or(i);
            self.fechar(NIVEL_PROGRAMA, self.linha_antes(primeiro));
            self.abrir(
                NIVEL_PROGRAMA,
                Simbolo::novo(nome, TipoSimbolo::Programa, Linhas::new(ini, ini)),
                primeiro,
            );
            let mut s = Simbolo::novo(
                "IDENTIFICATION DIVISION",
                TipoSimbolo::Secao,
                Linhas::new(ini, ini),
            );
            s.detalhe = Some("divisão".into());
            self.abrir(NIVEL_DIVISAO, s, primeiro);
            return Some(self.ponto(j));
        }
        if k == "END" && seguinte == "PROGRAM" {
            let ponto = self.ponto(i);
            let fim = self.t.get(ponto).map_or(linha, |t| t.linha);
            let antes = self.linha_antes(i);
            self.fechar(NIVEL_DIVISAO, antes);
            self.fechar(NIVEL_PROGRAMA, fim);
            self.divisao = Divisao::Nenhuma;
            return Some(ponto + 1);
        }
        if seguinte == "SECTION" {
            let ponto = self.ponto(i);
            let nome = if self.divisao == Divisao::Procedimentos {
                tk.texto.clone()
            } else {
                format!("{k} SECTION")
            };
            self.abrir(
                NIVEL_SECAO,
                Simbolo::novo(nome, TipoSimbolo::Secao, Linhas::new(linha, linha)),
                i,
            );
            return Some(ponto);
        }
        if self.divisao == Divisao::Dados
            && (eh_nivel(tk) || matches!(k, "FD" | "SD" | "RD" | "CD"))
        {
            return Some(self.item_de_dados(i));
        }
        if self.divisao == Divisao::Procedimentos
            && tk.primeiro
            && tk.area_a
            && self.tipo(i + 1) == Some(Tipo::Ponto)
            && !VERBOS.contains(&k)
        {
            self.abrir(
                NIVEL_PARAGRAFO,
                Simbolo::novo(
                    tk.texto.clone(),
                    TipoSimbolo::Paragrafo,
                    Linhas::new(linha, linha),
                ),
                i,
            );
            return Some(i + 1);
        }
        None
    }

    /// Item de dados a partir do número de nível (ou `FD`), até o ponto.
    fn item_de_dados(&mut self, i: usize) -> usize {
        let tk = &self.t[i];
        let linha = tk.linha;
        let ponto = self.ponto(i);
        let k = tk.chave.clone();
        let nivel: u32 = k.parse().unwrap_or(0);
        let nivel_arvore = match nivel {
            0 => NIVEL_ARQUIVO,
            1 | 77 => NIVEL_DADO,
            66 | 88 => NIVEL_DADO + 50,
            n => NIVEL_DADO + n,
        };
        let nome_tk = self.t.get(i + 1).filter(|t| {
            t.tipo == Tipo::Palavra
                && i + 1 < ponto
                && !matches!(
                    t.chave.as_str(),
                    "FILLER" | "PIC" | "PICTURE" | "REDEFINES" | "OCCURS" | "VALUE" | "VALUES"
                )
        });
        let Some(nome_tk) = nome_tk else {
            // FILLER: não vira símbolo, mas encerra os irmãos.
            let fim = self.linha_antes(i);
            self.fechar(nivel_arvore, fim);
            return ponto;
        };
        let nome = nome_tk.texto.clone();
        let resto = self.juntar(i + 2, ponto);
        let up = format!(" {} ", resto.to_uppercase());
        let elementar = up.contains(" PIC ") || up.contains(" PICTURE ");
        let (tipo, detalhe) = match nivel {
            0 => (TipoSimbolo::Outro, format!("{k} {resto}")),
            1 if elementar => (TipoSimbolo::Variavel, resto),
            1 => (TipoSimbolo::Registro, resto),
            77 => (TipoSimbolo::Variavel, resto),
            66 | 88 => (TipoSimbolo::Constante, resto),
            _ => (TipoSimbolo::Campo, resto),
        };
        let mut s = Simbolo::novo(nome.clone(), tipo, Linhas::new(linha, linha));
        let detalhe = resumir(&detalhe);
        if !detalhe.is_empty() {
            s.detalhe = Some(detalhe);
        }
        self.abrir(nivel_arvore, s, i);
        self.dados.insert(nome.to_uppercase());
        ponto
    }

    /// Texto dos tokens `[de, ate)`, com os espaços do original.
    fn juntar(&self, de: usize, ate: usize) -> String {
        let mut s = String::new();
        for t in self.t.get(de..ate.min(self.t.len())).unwrap_or(&[]) {
            if t.espaco && !s.is_empty() {
                s.push(' ');
            }
            if t.tipo == Tipo::Texto {
                s.push('\'');
                s.push_str(&t.texto);
                s.push('\'');
            } else {
                s.push_str(&t.texto);
            }
        }
        s
    }

    /// Referências de uma instrução da `PROCEDURE DIVISION`.
    fn instrucao(&mut self, i: usize) -> usize {
        let tk = &self.t[i];
        let linha = tk.linha;
        let k = tk.chave.as_str();
        match k {
            "PERFORM" => {
                let alvo = i + 1;
                if self.tipo(alvo) != Some(Tipo::Palavra)
                    || matches!(
                        self.chave(alvo),
                        "UNTIL" | "VARYING" | "WITH" | "TEST" | "FOREVER"
                    )
                    || self.chave(alvo + 1) == "TIMES"
                {
                    return i + 1;
                }
                let t = self.t[alvo].clone();
                self.referir(&t.texto, TipoReferencia::Chamada, t.linha);
                if matches!(self.chave(alvo + 1), "THRU" | "THROUGH")
                    && self.tipo(alvo + 2) == Some(Tipo::Palavra)
                {
                    let t = self.t[alvo + 2].clone();
                    self.referir(&t.texto, TipoReferencia::Chamada, t.linha);
                    return alvo + 3;
                }
                alvo + 1
            }
            "GO" => {
                let mut j = i + 1;
                if self.chave(j) == "TO" {
                    j += 1;
                }
                while self.tipo(j) == Some(Tipo::Palavra)
                    && !matches!(self.chave(j), "DEPENDING" | "ON")
                    && !VERBOS.contains(&self.chave(j))
                {
                    let t = self.t[j].clone();
                    self.referir(&t.texto, TipoReferencia::Chamada, t.linha);
                    j += 1;
                }
                j
            }
            "CALL" if self.tipo(i + 1) == Some(Tipo::Texto) => {
                let t = self.t[i + 1].clone();
                self.referir(&t.texto, TipoReferencia::Chamada, t.linha);
                i + 2
            }
            _ => {
                if self.dados.contains(k) {
                    let nome = tk.texto.clone();
                    self.referir(&nome, TipoReferencia::Uso, linha);
                } else if k.contains('-')
                    && !k.starts_with("END-")
                    && !k.starts_with('-')
                    && k.chars().any(|c| c.is_ascii_alphabetic())
                    && !RESERVADAS_COM_HIFEN.contains(&k)
                {
                    let nome = tk.texto.clone();
                    self.referir(&nome, TipoReferencia::Mencao, linha);
                }
                i + 1
            }
        }
    }

    /// `EXEC SQL ... END-EXEC` (e outros `EXEC`, ignorados). Devolve o índice depois
    /// do `END-EXEC`.
    fn exec(&mut self, i: usize) -> usize {
        let fim = (i..self.t.len())
            .find(|&k| self.t[k].chave == "END-EXEC")
            .unwrap_or(self.t.len());
        if self.chave(i + 1) != "SQL" {
            return fim + 1;
        }
        let corpo = &self.t[i + 2..fim];
        if corpo.first().map(|t| t.chave.as_str()) == Some("INCLUDE") {
            if let Some(t) = corpo.get(1) {
                let (nome, linha) = (t.texto.clone(), t.linha);
                self.referir(&nome, TipoReferencia::Inclusao, linha);
            }
            return fim + 1;
        }
        let Some(primeira) = corpo.first().map(|t| t.linha) else {
            return fim + 1;
        };
        // Reconstrói o SQL com as mesmas linhas (relativas ao EXEC); variáveis de
        // host (`:WS-ID`) viram `:WS_ID`.
        let mut sql = String::new();
        let mut linha_atual = primeira;
        let mut sep = Separado::default();
        let mut host = false;
        for t in corpo {
            while linha_atual < t.linha {
                sql.push('\n');
                linha_atual += 1;
            }
            sql.push(' ');
            match t.tipo {
                Tipo::Texto => {
                    sql.push('\'');
                    sql.push_str(&t.texto.replace('\'', "''"));
                    sql.push('\'');
                    sep.textos.push((t.linha, t.texto.clone()));
                }
                Tipo::Palavra if host => {
                    sql.push_str(&t.texto.replace('-', "_"));
                }
                Tipo::Palavra => {
                    sql.push_str(&t.texto);
                    sep.textos.push((t.linha, t.texto.clone()));
                }
                _ => sql.push_str(&t.texto),
            }
            host = t.tipo == Tipo::Sim(':');
        }
        let deslocamento = primeira - 1;
        let mut tipadas = Vec::new();
        if let Ok(e) = ExtratorSql.extrair(&Fonte {
            caminho: "",
            linguagem: Linguagem::Sql,
            texto: &sql,
        }) {
            for r in e.referencias {
                if r.tipo == TipoReferencia::Mencao {
                    continue;
                }
                tipadas.push((r.alvo, r.tipo, r.linha + deslocamento));
            }
        }
        for (alvo, tipo, linha) in tipadas {
            self.referir(&alvo, tipo, linha);
        }
        // Menções: nomes do SQL que o extrator de SQL não classificou.
        let ja: HashSet<(u32, String)> = self
            .saida
            .referencias
            .iter()
            .map(|r| (r.linha, r.alvo.to_uppercase()))
            .collect();
        for r in mencoes(&sep, &[]) {
            if !ja.contains(&(r.linha, r.alvo.to_uppercase())) {
                self.referir(&r.alvo, TipoReferencia::Mencao, r.linha);
            }
        }
        fim + 1
    }
}

fn eh_nivel(t: &Token) -> bool {
    t.tipo == Tipo::Palavra
        && (1..=2).contains(&t.texto.len())
        && t.texto.chars().all(|c| c.is_ascii_digit())
}

/// Comentários de linha inteira logo acima de `linha`, sem as molduras.
fn comentario_antes(comentarios: &BTreeMap<u32, String>, linha: u32) -> Option<String> {
    let mut partes = Vec::new();
    let mut alvo = linha.checked_sub(1)?;
    while let Some(c) = comentarios.get(&alvo) {
        partes.push(c.as_str());
        alvo = alvo.checked_sub(1)?;
    }
    partes.reverse();
    let limpo: Vec<&str> = partes
        .iter()
        .map(|p| p.trim().trim_matches(['*', '>', '/']).trim())
        .filter(|p| !p.is_empty() && !p.chars().all(|c| "=-*#~_".contains(c)))
        .collect();
    (!limpo.is_empty()).then(|| resumir(&limpo.join(" ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extrair(caminho: &str, texto: &str) -> Extracao {
        ExtratorCobol
            .extrair(&Fonte {
                caminho,
                linguagem: Linguagem::Cobol,
                texto,
            })
            .unwrap()
    }

    fn simbolo<'a>(e: &'a Extracao, q: &str) -> &'a Simbolo {
        e.simbolos
            .iter()
            .find(|s| s.qualificado == q)
            .unwrap_or_else(|| panic!("{q} não encontrado em {:#?}", e.simbolos))
    }

    fn refs(e: &Extracao) -> Vec<(Option<&str>, &str, TipoReferencia, u32)> {
        e.referencias
            .iter()
            .map(|r| (r.origem.as_deref(), r.alvo.as_str(), r.tipo, r.linha))
            .collect()
    }

    /// Linha em formato fixo: sequência, indicador e código.
    fn fixo(linhas: &[&str]) -> String {
        linhas
            .iter()
            .enumerate()
            .map(|(n, l)| {
                if let Some(c) = l.strip_prefix('*') {
                    format!("{:06}*{c}\n", (n + 1) * 10)
                } else {
                    format!("{:06} {l}\n", (n + 1) * 10)
                }
            })
            .collect()
    }

    const FIXTURE: &str =
        include_str!("../../../../fixtures/faturamento-exemplo/cobol/CALCOBR.cbl");

    #[test]
    fn programa_da_fixture() {
        let e = extrair("cobol/CALCOBR.cbl", FIXTURE);
        let resumo: Vec<_> = e
            .simbolos
            .iter()
            .filter(|s| !matches!(s.tipo, TipoSimbolo::Campo | TipoSimbolo::Variavel))
            .map(|s| (s.nome.as_str(), s.tipo, s.linhas))
            .collect();
        assert_eq!(
            resumo,
            [
                ("CALCOBR", TipoSimbolo::Programa, Linhas::new(7, 92)),
                (
                    "IDENTIFICATION DIVISION",
                    TipoSimbolo::Secao,
                    Linhas::new(7, 8)
                ),
                (
                    "ENVIRONMENT DIVISION",
                    TipoSimbolo::Secao,
                    Linhas::new(10, 16)
                ),
                (
                    "INPUT-OUTPUT SECTION",
                    TipoSimbolo::Secao,
                    Linhas::new(11, 16)
                ),
                ("DATA DIVISION", TipoSimbolo::Secao, Linhas::new(18, 47)),
                ("FILE SECTION", TipoSimbolo::Secao, Linhas::new(19, 34)),
                ("ARQ-FATURAS", TipoSimbolo::Outro, Linhas::new(20, 26)),
                ("REG-FATURA", TipoSimbolo::Registro, Linhas::new(21, 26)),
                ("ARQ-COBRANCA", TipoSimbolo::Outro, Linhas::new(28, 34)),
                ("REG-COBRANCA", TipoSimbolo::Registro, Linhas::new(29, 34)),
                (
                    "WORKING-STORAGE SECTION",
                    TipoSimbolo::Secao,
                    Linhas::new(36, 47)
                ),
                (
                    "PROCEDURE DIVISION",
                    TipoSimbolo::Secao,
                    Linhas::new(49, 92)
                ),
                (
                    "0000-PRINCIPAL",
                    TipoSimbolo::Paragrafo,
                    Linhas::new(50, 59)
                ),
                (
                    "1000-LER-FATURA",
                    TipoSimbolo::Paragrafo,
                    Linhas::new(61, 65)
                ),
                (
                    "2000-PROCESSAR-FATURA",
                    TipoSimbolo::Paragrafo,
                    Linhas::new(67, 83)
                ),
                (
                    "3000-GRAVAR-COBRANCA",
                    TipoSimbolo::Paragrafo,
                    Linhas::new(85, 92)
                ),
            ]
        );
        let programa = simbolo(&e, "CALCOBR");
        assert!(
            programa
                .comentario
                .as_deref()
                .unwrap()
                .contains("LOTE MENSAL DE COBRANCA"),
            "{:?}",
            programa.comentario
        );
        let paragrafo = simbolo(&e, "CALCOBR.PROCEDURE DIVISION.2000-PROCESSAR-FATURA");
        assert_eq!(paragrafo.pai.as_deref(), Some("CALCOBR.PROCEDURE DIVISION"));

        // Itens de dados com o PIC no detalhe.
        let minimo = e
            .simbolos
            .iter()
            .find(|s| s.nome == "WS-VALOR-MIN-COBRANCA")
            .unwrap();
        assert_eq!(minimo.tipo, TipoSimbolo::Variavel);
        assert_eq!(minimo.linhas, Linhas::new(41, 41));
        assert_eq!(minimo.detalhe.as_deref(), Some("PIC 9(05)V99 VALUE 10.00"));
        assert_eq!(
            minimo.pai.as_deref(),
            Some("CALCOBR.DATA DIVISION.WORKING-STORAGE SECTION")
        );
        let campo = e.simbolos.iter().find(|s| s.nome == "FAT-ISENTO").unwrap();
        assert_eq!(campo.tipo, TipoSimbolo::Campo);
        assert_eq!(campo.linhas, Linhas::new(24, 24));
        assert!(
            campo
                .pai
                .as_deref()
                .unwrap()
                .ends_with(".ARQ-FATURAS.REG-FATURA")
        );

        let r = refs(&e);
        let principal = Some("CALCOBR.PROCEDURE DIVISION.0000-PRINCIPAL");
        let processar = Some("CALCOBR.PROCEDURE DIVISION.2000-PROCESSAR-FATURA");
        assert!(r.contains(&(principal, "1000-LER-FATURA", TipoReferencia::Chamada, 53)));
        assert!(r.contains(&(
            principal,
            "2000-PROCESSAR-FATURA",
            TipoReferencia::Chamada,
            54
        )));
        assert!(r.contains(&(principal, "WS-FIM-ARQUIVO", TipoReferencia::Uso, 54)));
        assert!(r.contains(&(
            processar,
            "3000-GRAVAR-COBRANCA",
            TipoReferencia::Chamada,
            81
        )));
        assert!(r.contains(&(processar, "WS-VALOR-MIN-COBRANCA", TipoReferencia::Uso, 78)));
        assert!(r.contains(&(processar, "FAT-ISENTO", TipoReferencia::Uso, 69)));
        // Nada de menção: todos os nomes são do próprio arquivo.
        assert!(r.iter().all(|x| x.2 != TipoReferencia::Mencao), "{r:?}");
    }

    #[test]
    fn copy_call_sql_e_secoes() {
        let texto = fixo(&[
            "*  PROGRAMA DE TESTE",
            "IDENTIFICATION DIVISION.",
            "PROGRAM-ID. TESTE1.",
            "DATA DIVISION.",
            "WORKING-STORAGE SECTION.",
            "    COPY CPYFAT.",
            "    EXEC SQL INCLUDE SQLCA END-EXEC.",
            "01  WS-ID        PIC 9(12).",
            "01  WS-STATUS    PIC X(10).",
            "    88 WS-ABERTA VALUE 'ABERTA'.",
            "PROCEDURE DIVISION.",
            "MAIN-LOGIC SECTION.",
            "* LE A FATURA",
            "1000-INICIO.",
            "    PERFORM 2000-LER THRU 2000-LER-FIM",
            "    PERFORM 3 TIMES",
            "        DISPLAY 'X'",
            "    END-PERFORM",
            "    CALL 'CALCOBR' USING WS-ID FAT-VALOR",
            "    GO TO 9999-FIM.",
            "2000-LER.",
            "    EXEC SQL",
            "        SELECT STATUS INTO :WS-STATUS",
            "          FROM FATURA",
            "         WHERE ID_FATURA = :WS-ID",
            "    END-EXEC.",
            "    EXEC SQL",
            "        UPDATE FATURA SET STATUS = 'PAGA'",
            "    END-EXEC.",
            "    EXEC SQL CALL PKG_COBRANCA.PRC_CANCELAR_FATURA(:WS-ID)",
            "    END-EXEC.                                                      IGNORADO",
            "2000-LER-FIM.",
            "    EXIT.",
            "9999-FIM.",
            "    STOP RUN.",
            "END PROGRAM TESTE1.",
        ]);
        let e = extrair("x/TESTE1.cbl", &texto);
        let nomes: Vec<_> = e
            .simbolos
            .iter()
            .map(|s| (s.qualificado.as_str(), s.linhas))
            .collect();
        assert!(
            nomes.contains(&("TESTE1", Linhas::new(2, 36))),
            "{nomes:#?}"
        );
        assert!(
            nomes.contains(&("TESTE1.PROCEDURE DIVISION.MAIN-LOGIC", Linhas::new(12, 35))),
            "{nomes:#?}"
        );
        assert!(nomes.contains(&(
            "TESTE1.PROCEDURE DIVISION.MAIN-LOGIC.1000-INICIO",
            Linhas::new(14, 20)
        )));
        assert!(nomes.contains(&(
            "TESTE1.PROCEDURE DIVISION.MAIN-LOGIC.2000-LER-FIM",
            Linhas::new(32, 33)
        )));
        assert!(nomes.contains(&(
            "TESTE1.DATA DIVISION.WORKING-STORAGE SECTION.WS-STATUS.WS-ABERTA",
            Linhas::new(10, 10)
        )));
        assert_eq!(
            simbolo(&e, "TESTE1").comentario.as_deref(),
            Some("PROGRAMA DE TESTE")
        );
        assert_eq!(
            simbolo(&e, "TESTE1.PROCEDURE DIVISION.MAIN-LOGIC.1000-INICIO")
                .comentario
                .as_deref(),
            Some("LE A FATURA")
        );

        let r = refs(&e);
        let ws = Some("TESTE1.DATA DIVISION.WORKING-STORAGE SECTION");
        let inicio = Some("TESTE1.PROCEDURE DIVISION.MAIN-LOGIC.1000-INICIO");
        let ler = Some("TESTE1.PROCEDURE DIVISION.MAIN-LOGIC.2000-LER");
        for esperado in [
            (ws, "CPYFAT", TipoReferencia::Inclusao, 6),
            (ws, "SQLCA", TipoReferencia::Inclusao, 7),
            (inicio, "2000-LER", TipoReferencia::Chamada, 15),
            (inicio, "2000-LER-FIM", TipoReferencia::Chamada, 15),
            (inicio, "CALCOBR", TipoReferencia::Chamada, 19),
            (inicio, "WS-ID", TipoReferencia::Uso, 19),
            (inicio, "FAT-VALOR", TipoReferencia::Mencao, 19),
            (inicio, "9999-FIM", TipoReferencia::Chamada, 20),
            (ler, "FATURA", TipoReferencia::Leitura, 24),
            (ler, "FATURA", TipoReferencia::Escrita, 28),
            (
                ler,
                "PKG_COBRANCA.PRC_CANCELAR_FATURA",
                TipoReferencia::Chamada,
                30,
            ),
        ] {
            assert!(r.contains(&esperado), "{esperado:?} fora de {r:#?}");
        }
        // PERFORM em linha não é chamada.
        assert!(!r.iter().any(|x| x.1 == "3"));
        // Variável de host não é menção.
        assert!(!r.iter().any(|x| x.1.contains("WS_")), "{r:#?}");
    }

    #[test]
    fn copybook_e_formato_livre() {
        let e = extrair(
            "copy/CPYFAT.cpy",
            "      * REGISTRO DE FATURA\n       01  REG-FAT.\n           05 FAT-VALOR  PIC 9(10)V99.\n           05 FILLER     PIC X(10).\n           05 FAT-STATUS PIC X(10).\n",
        );
        let nomes: Vec<_> = e
            .simbolos
            .iter()
            .map(|s| (s.qualificado.as_str(), s.tipo, s.linhas))
            .collect();
        assert_eq!(
            nomes,
            [
                ("CPYFAT", TipoSimbolo::Copybook, Linhas::new(2, 5)),
                ("CPYFAT.REG-FAT", TipoSimbolo::Registro, Linhas::new(2, 5)),
                (
                    "CPYFAT.REG-FAT.FAT-VALOR",
                    TipoSimbolo::Campo,
                    Linhas::new(3, 3)
                ),
                (
                    "CPYFAT.REG-FAT.FAT-STATUS",
                    TipoSimbolo::Campo,
                    Linhas::new(5, 5)
                ),
            ]
        );
        assert_eq!(
            e.simbolos[0].comentario.as_deref(),
            Some("REGISTRO DE FATURA")
        );

        let e = extrair(
            "livre.cob",
            "*> programa livre\nIDENTIFICATION DIVISION.\nPROGRAM-ID. LIVRE.\nPROCEDURE DIVISION.\nINICIO.\n    PERFORM FIM. *> comentário\nFIM.\n    GOBACK.\n",
        );
        let nomes: Vec<_> = e
            .simbolos
            .iter()
            .map(|s| (s.qualificado.as_str(), s.linhas))
            .collect();
        assert_eq!(
            nomes,
            [
                ("LIVRE", Linhas::new(2, 8)),
                ("LIVRE.IDENTIFICATION DIVISION", Linhas::new(2, 3)),
                ("LIVRE.PROCEDURE DIVISION", Linhas::new(4, 8)),
                ("LIVRE.PROCEDURE DIVISION.INICIO", Linhas::new(5, 6)),
                ("LIVRE.PROCEDURE DIVISION.FIM", Linhas::new(7, 8)),
            ]
        );
        assert_eq!(e.simbolos[0].comentario.as_deref(), Some("programa livre"));
        assert_eq!(
            refs(&e),
            [(
                Some("LIVRE.PROCEDURE DIVISION.INICIO"),
                "FIM",
                TipoReferencia::Chamada,
                6
            )]
        );
    }

    #[test]
    fn windows_1252_sem_deslocar_colunas() {
        // "AÇÃO" e "é" em Windows-1252, lidos como o mapa lê (UTF-8 com perdas).
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"      * A\xc7\xc3O DE COBRAN\xc7A\n");
        bytes.extend_from_slice(b"       IDENTIFICATION DIVISION.\n");
        bytes.extend_from_slice(b"       PROGRAM-ID. ACENTO.\n");
        bytes.extend_from_slice(b"       PROCEDURE DIVISION.\n");
        bytes.extend_from_slice(b"       P1.\n");
        bytes.extend_from_slice(b"           DISPLAY 'op\xe7\xe3o \xe9 v\xe1lida'\n");
        bytes.extend_from_slice(b"           PERFORM P2.\n");
        bytes.extend_from_slice(b"       P2.\n");
        bytes.extend_from_slice(b"           STOP RUN.\n");
        let texto = String::from_utf8_lossy(&bytes);
        let e = extrair("ACENTO.cbl", &texto);
        let nomes: Vec<_> = e
            .simbolos
            .iter()
            .map(|s| (s.nome.as_str(), s.linhas))
            .collect();
        assert_eq!(
            nomes,
            [
                ("ACENTO", Linhas::new(2, 9)),
                ("IDENTIFICATION DIVISION", Linhas::new(2, 3)),
                ("PROCEDURE DIVISION", Linhas::new(4, 9)),
                ("P1", Linhas::new(5, 7)),
                ("P2", Linhas::new(8, 9)),
            ]
        );
        assert!(e.simbolos[0].comentario.is_some());
        assert_eq!(refs(&e)[0].1, "P2");
    }

    #[test]
    fn texto_vazio_ou_estranho_nao_quebra() {
        assert_eq!(extrair("x.cbl", ""), Extracao::default());
        let e = extrair("x.cbl", "      *\n\n  .\n 'sem fim\n");
        assert!(e.referencias.is_empty());
        extrair(
            "x.cbl",
            "       PROCEDURE DIVISION.\n       EXEC SQL SELECT",
        );
        extrair("x.cbl", "       PROGRAM-ID.");
    }
}
