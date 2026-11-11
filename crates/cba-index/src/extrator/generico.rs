//! Extrator genérico: reserva para as linguagens sem extrator próprio.
//!
//! Separa código, comentários e textos (strings) com uma [`Sintaxe`] por
//! linguagem e então:
//!
//! - acha definições por heurística, linha a linha (`class X`, `def x`,
//!   `function x`, `Sub X`, `procedure X`, `x() {`), com o fim pela indentação e o
//!   comentário logo acima como documentação;
//! - registra menções ([`TipoReferencia::Mencao`]) a nomes com cara de objeto de
//!   banco ou de constante (`PKG_COBRANCA.FN_CALCULAR_MULTA`, `FATURA`) no código
//!   e em qualquer nome dentro das strings, onde mora o SQL embutido. Comentários
//!   não geram menção. O mapa descarta as menções que não batem com nenhum
//!   símbolo conhecido.
//!
//! Documentação (Markdown, texto) não gera definição nem menção.

use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;

use super::{Extracao, Extrator, Fonte, Linhas, Referencia, Simbolo, TipoReferencia, TipoSimbolo};
use crate::linguagem::Linguagem;

/// Extrator genérico, para qualquer linguagem.
#[derive(Debug, Clone, Copy, Default)]
pub struct ExtratorGenerico;

impl Extrator for ExtratorGenerico {
    fn nome(&self) -> &'static str {
        "generico"
    }

    fn versao(&self) -> u32 {
        1
    }

    /// Todas: é o extrator de reserva.
    fn linguagens(&self) -> &'static [Linguagem] {
        &[]
    }

    fn extrair(&self, fonte: &Fonte<'_>) -> Result<Extracao, String> {
        let l = fonte.linguagem;
        if matches!(l, Linguagem::Markdown | Linguagem::Texto) {
            return Ok(Extracao::default());
        }
        let sintaxe = Sintaxe::de(l);
        let sep = separar(fonte.texto, &sintaxe);
        let mut saida = Extracao::default();
        if definicoes_fazem_sentido(l) {
            saida.simbolos = definicoes(&sep);
        }
        saida.referencias = mencoes(&sep, &saida.simbolos);
        Ok(saida)
    }
}

/// Como uma linguagem marca comentários e textos.
#[derive(Debug, Clone, Copy)]
pub struct Sintaxe {
    /// Início de comentário de linha (`//`, `#`, `--`, `'`).
    pub linha: &'static [&'static str],
    /// Comentários de bloco (início, fim).
    pub bloco: &'static [(&'static str, &'static str)],
    /// Delimitadores de string (o mesmo caractere abre e fecha).
    pub aspas: &'static [char],
    /// `\` escapa o caractere seguinte dentro da string.
    pub escape: bool,
}

impl Sintaxe {
    pub const C: Sintaxe = Sintaxe {
        linha: &["//"],
        bloco: &[("/*", "*/")],
        aspas: &['"', '\'', '`'],
        escape: true,
    };
    pub const SCRIPT: Sintaxe = Sintaxe {
        linha: &["#"],
        bloco: &[],
        aspas: &['"', '\''],
        escape: true,
    };
    pub const PASCAL: Sintaxe = Sintaxe {
        linha: &["//"],
        bloco: &[("{", "}"), ("(*", "*)")],
        aspas: &['\''],
        escape: false,
    };
    pub const BASIC: Sintaxe = Sintaxe {
        linha: &["'", "REM ", "rem "],
        bloco: &[],
        aspas: &['"'],
        escape: false,
    };
    pub const SQL: Sintaxe = Sintaxe {
        linha: &["--"],
        bloco: &[("/*", "*/")],
        aspas: &['\''],
        escape: false,
    };
    pub const MARCACAO: Sintaxe = Sintaxe {
        linha: &[],
        bloco: &[("<!--", "-->")],
        aspas: &['"', '\''],
        escape: false,
    };
    pub const BATCH: Sintaxe = Sintaxe {
        linha: &["REM ", "rem ", "::"],
        bloco: &[],
        aspas: &['"'],
        escape: false,
    };
    /// Sem comentários nem strings: tudo é código.
    pub const NENHUMA: Sintaxe = Sintaxe {
        linha: &[],
        bloco: &[],
        aspas: &[],
        escape: false,
    };

    pub fn de(linguagem: Linguagem) -> Sintaxe {
        use Linguagem::*;
        match linguagem {
            Java | Kotlin | CSharp | TypeScript | JavaScript | C | Cpp | Go | Css => Sintaxe::C,
            Php => Sintaxe {
                linha: &["//", "#"],
                ..Sintaxe::C
            },
            Python | Ruby | Shell | PowerShell | Yaml | Configuracao => Sintaxe::SCRIPT,
            Delphi => Sintaxe::PASCAL,
            VisualBasic6 | VisualBasicNet => Sintaxe::BASIC,
            Sql => Sintaxe::SQL,
            Html | Xml => Sintaxe::MARCACAO,
            Batch => Sintaxe::BATCH,
            Json => Sintaxe {
                aspas: &['"'],
                ..Sintaxe::NENHUMA
            },
            // COBOL tem comentário pela coluna; fica para o extrator próprio (#17).
            Cobol | Markdown | Texto | Outra => Sintaxe::NENHUMA,
        }
    }
}

/// Texto de um arquivo separado por natureza, linha a linha.
#[derive(Debug, Default)]
pub struct Separado {
    /// Cada linha só com o código: comentários e conteúdo de strings viram
    /// espaços (as aspas ficam), preservando as colunas.
    pub codigo: Vec<String>,
    /// Conteúdo das strings: (linha, texto), uma entrada por linha ocupada.
    pub textos: Vec<(u32, String)>,
    /// Comentários: (linha, texto, sozinho na linha).
    pub comentarios: Vec<(u32, String, bool)>,
}

/// Separa código, comentários e strings.
pub fn separar(texto: &str, sintaxe: &Sintaxe) -> Separado {
    #[derive(PartialEq)]
    enum Estado {
        Codigo,
        Bloco(&'static str),
        Texto(char),
    }
    let mut sep = Separado::default();
    let mut estado = Estado::Codigo;
    for (n, linha) in texto.lines().enumerate() {
        let num = n as u32 + 1;
        let mut codigo = String::with_capacity(linha.len());
        let mut texto_atual = String::new();
        let mut comentario = String::new();
        let mut sozinho = true;
        let mut resto = linha;
        while let Some(ch) = resto.chars().next() {
            match estado {
                Estado::Codigo => {
                    if sintaxe.linha.iter().any(|p| resto.starts_with(*p)) {
                        comentario.push_str(resto);
                        brancos(&mut codigo, resto);
                        resto = "";
                        continue;
                    }
                    if let Some((ini, fim)) =
                        sintaxe.bloco.iter().find(|(i, _)| resto.starts_with(i))
                    {
                        estado = Estado::Bloco(fim);
                        brancos(&mut codigo, ini);
                        comentario.push(' ');
                        resto = &resto[ini.len()..];
                        continue;
                    }
                    if sintaxe.aspas.contains(&ch) {
                        estado = Estado::Texto(ch);
                    }
                    if !ch.is_whitespace() {
                        sozinho = false;
                    }
                    codigo.push(ch);
                }
                Estado::Bloco(fim) => {
                    if resto.starts_with(fim) {
                        estado = Estado::Codigo;
                        brancos(&mut codigo, fim);
                        resto = &resto[fim.len()..];
                        continue;
                    }
                    comentario.push(ch);
                    codigo.push(' ');
                }
                Estado::Texto(q) => {
                    if sintaxe.escape && ch == '\\' {
                        let mut it = resto.chars();
                        it.next();
                        let prox = it.next();
                        texto_atual.push(ch);
                        codigo.push(' ');
                        if let Some(p) = prox {
                            texto_atual.push(p);
                            codigo.push(' ');
                            resto = &resto[ch.len_utf8() + p.len_utf8()..];
                        } else {
                            resto = &resto[ch.len_utf8()..];
                        }
                        continue;
                    }
                    if ch == q {
                        estado = Estado::Codigo;
                        codigo.push(ch);
                        sep.textos.push((num, std::mem::take(&mut texto_atual)));
                    } else {
                        texto_atual.push(ch);
                        codigo.push(' ');
                    }
                }
            }
            resto = &resto[ch.len_utf8()..];
        }
        if !texto_atual.is_empty() {
            sep.textos.push((num, texto_atual));
        }
        // Só a crase (template do JavaScript) abre string de várias linhas: aspas
        // sem fechamento (um apóstrofo no texto) não contaminam o resto.
        if matches!(estado, Estado::Texto(q) if q != '`') {
            estado = Estado::Codigo;
        }
        if !comentario.trim().is_empty() {
            sep.comentarios.push((num, comentario, sozinho));
        }
        sep.codigo.push(codigo);
    }
    sep
}

fn brancos(s: &mut String, x: &str) {
    s.extend(std::iter::repeat_n(' ', x.chars().count()));
}

fn definicoes_fazem_sentido(l: Linguagem) -> bool {
    !matches!(
        l,
        // COBOL: `PROCEDURE DIVISION` não é rotina; fica para o extrator próprio.
        Linguagem::Cobol
            | Linguagem::Json
            | Linguagem::Yaml
            | Linguagem::Xml
            | Linguagem::Html
            | Linguagem::Css
            | Linguagem::Configuracao
            | Linguagem::Markdown
            | Linguagem::Texto
    )
}

static DEFINICAO: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?ix)
        ^\s*
        (?:(?:public|private|protected|internal|static|final|abstract|sealed|partial|export
            |default|async|virtual|override|open|data|inline|friend|shared|overridable
            |overrides|declare|local|global|pub(?:\([a-z]+\))?|unsafe|const)\s+)*
        (?P<kw>class|interface|enum|struct|record|trait|module|namespace|object|def|function
            |func|fun|fn|sub|procedure|program)
        \s+
        (?:\([^)]*\)\s*)?
        (?P<nome>[a-z_][a-z0-9_$]*(?:\.[a-z_][a-z0-9_$]*)*)",
    )
    .expect("regex de definição")
});

/// `nome() {` e `function nome {` de shell.
static FUNCAO_SHELL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*(?:function\s+)?(?P<nome>[A-Za-z_][A-Za-z0-9_-]*)\s*\(\)\s*\{?\s*$")
        .expect("regex de função de shell")
});

fn tipo_da_palavra(kw: &str) -> TipoSimbolo {
    match kw.to_ascii_lowercase().as_str() {
        "class" | "object" => TipoSimbolo::Classe,
        "interface" | "trait" => TipoSimbolo::Interface,
        "enum" => TipoSimbolo::Enum,
        "struct" | "record" => TipoSimbolo::Registro,
        "module" => TipoSimbolo::Modulo,
        "namespace" => TipoSimbolo::Namespace,
        "program" => TipoSimbolo::Programa,
        "procedure" => TipoSimbolo::Procedure,
        "sub" => TipoSimbolo::Rotina,
        _ => TipoSimbolo::Funcao,
    }
}

fn indentacao(s: &str) -> usize {
    s.chars()
        .take_while(|c| c.is_whitespace())
        .map(|c| if c == '\t' { 4 } else { 1 })
        .sum()
}

/// Definições por heurística, com o fim pela indentação: o bloco vai até a
/// última linha antes da próxima linha não vazia com indentação menor ou igual,
/// incluída a linha de fechamento (`}`, `end`, `End Sub`).
fn definicoes(sep: &Separado) -> Vec<Simbolo> {
    let linhas = &sep.codigo;
    let mut achadas: Vec<(usize, usize, Simbolo)> = Vec::new(); // (linha 0-based, indentação, símbolo)
    for (n, l) in linhas.iter().enumerate() {
        let (tipo, nome) = if let Some(c) = DEFINICAO.captures(l) {
            (tipo_da_palavra(&c["kw"]), c["nome"].to_string())
        } else if let Some(c) = FUNCAO_SHELL.captures(l) {
            (TipoSimbolo::Funcao, c["nome"].to_string())
        } else {
            continue;
        };
        let ind = indentacao(l);
        let mut fim = n;
        for (k, prox) in linhas.iter().enumerate().skip(n + 1) {
            if prox.trim().is_empty() {
                continue;
            }
            if indentacao(prox) <= ind {
                if fecha_bloco(prox) {
                    fim = k;
                }
                break;
            }
            fim = k;
        }
        let nome_simples = nome.rsplit('.').next().unwrap_or(&nome).to_string();
        let mut s = Simbolo::novo(
            nome_simples,
            tipo,
            Linhas::new(n as u32 + 1, fim as u32 + 1),
        );
        s.comentario = comentario_antes(sep, n as u32 + 1);
        achadas.push((n, ind, s));
    }
    // Pai: a definição anterior mais próxima que contém esta e é menos indentada.
    let mut saida: Vec<Simbolo> = Vec::with_capacity(achadas.len());
    for k in 0..achadas.len() {
        let (n, ind, _) = achadas[k];
        let pai = (0..k).rev().find(|&j| {
            let (_, ind_j, ref s) = achadas[j];
            ind_j < ind && s.linhas.contem(n as u32 + 1)
        });
        let mut s = achadas[k].2.clone();
        if let Some(j) = pai {
            s = s.dentro_de(&saida[j].qualificado);
        }
        saida.push(s);
    }
    saida
}

fn fecha_bloco(linha: &str) -> bool {
    let t = linha.trim_start().to_ascii_lowercase();
    t.starts_with('}') || t.starts_with("end") || t.starts_with(')')
}

/// Comentários em linhas inteiras logo acima de `linha`.
fn comentario_antes(sep: &Separado, linha: u32) -> Option<String> {
    let mut partes = Vec::new();
    let mut alvo = linha.checked_sub(1)?;
    for (l, texto, sozinho) in sep.comentarios.iter().rev() {
        if *l > alvo {
            continue;
        }
        if *l < alvo || !*sozinho {
            break;
        }
        partes.push(texto.as_str());
        alvo = match alvo.checked_sub(1) {
            Some(x) => x,
            None => break,
        };
    }
    partes.reverse();
    let limpo: Vec<String> = partes
        .iter()
        .map(|p| {
            p.trim()
                .trim_start_matches(['/', '#', '*', '\'', '-', '{', '(', '!', '<'])
                .trim_end_matches(['/', '*', '}', ')', '>', '-'])
                .trim()
                .to_string()
        })
        .filter(|p| !p.is_empty() && !p.chars().all(|c| "=-*#~_".contains(c)))
        .collect();
    (!limpo.is_empty()).then(|| super::sql::resumir(&limpo.join(" ")))
}

static IDENT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[A-Za-z_][A-Za-z0-9_$#]*(?:\.[A-Za-z_][A-Za-z0-9_$#]*)*")
        .expect("regex de identificador")
});

/// Palavras de SQL comuns em strings, que nunca são nome de objeto.
const PALAVRAS_SQL: &[&str] = &[
    "AND", "BEGIN", "CALL", "CASE", "COUNT", "DELETE", "DISTINCT", "DUAL", "ELSE", "END", "EXEC",
    "EXISTS", "FROM", "GROUP", "HAVING", "INNER", "INSERT", "INTO", "JOIN", "LEFT", "LIKE", "NULL",
    "ORDER", "OUTER", "RIGHT", "SELECT", "THEN", "UNION", "UPDATE", "VALUES", "WHEN", "WHERE",
    "WITH",
];

/// Nome com cara de objeto de banco ou constante: tem `_` ou é todo maiúsculo
/// com 4 letras ou mais. Nas strings, onde costuma estar o SQL embutido (às vezes
/// em minúsculas), conta qualquer nome com 4 letras ou mais que não seja palavra
/// do SQL.
fn candidato(nome: &str, em_texto: bool) -> bool {
    if nome.len() < 3 || PALAVRAS_SQL.contains(&nome.to_ascii_uppercase().as_str()) {
        return false;
    }
    if em_texto {
        return nome.len() >= 4;
    }
    let parte_ok = |p: &str| {
        p.contains('_') && p.chars().any(|c| c.is_ascii_alphabetic())
            || (p.len() >= 4
                && p.chars().all(|c| !c.is_ascii_lowercase())
                && p.chars().any(|c| c.is_ascii_alphabetic()))
    };
    nome.split('.').any(parte_ok)
}

/// Menções a nomes no código (fora de comentários) e nas strings.
pub fn mencoes(sep: &Separado, definidos: &[Simbolo]) -> Vec<Referencia> {
    let definicoes: HashSet<(u32, &str)> = definidos
        .iter()
        .map(|s| (s.linhas.inicio, s.nome.as_str()))
        .collect();
    let mut vistos: HashSet<(u32, String)> = HashSet::new();
    let mut saida = Vec::new();
    let mut registrar = |linha: u32, nome: &str| {
        if definicoes.contains(&(linha, nome)) {
            return;
        }
        if vistos.insert((linha, nome.to_string())) {
            let origem = definidos
                .iter()
                .rev()
                .find(|s| s.linhas.contem(linha))
                .map(|s| s.qualificado.clone());
            saida.push(Referencia {
                origem,
                alvo: nome.to_string(),
                tipo: TipoReferencia::Mencao,
                linha,
            });
        }
    };
    for (n, l) in sep.codigo.iter().enumerate() {
        for m in IDENT.find_iter(l) {
            if candidato(m.as_str(), false) {
                registrar(n as u32 + 1, m.as_str());
            }
        }
    }
    for (linha, t) in &sep.textos {
        for m in IDENT.find_iter(t) {
            if candidato(m.as_str(), true) {
                registrar(*linha, m.as_str());
            }
        }
    }
    saida.sort_by_key(|a| a.linha);
    saida
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extrair(linguagem: Linguagem, texto: &str) -> Extracao {
        ExtratorGenerico
            .extrair(&Fonte {
                caminho: "x",
                linguagem,
                texto,
            })
            .unwrap()
    }

    #[test]
    fn separa_codigo_comentario_e_texto() {
        let s = separar(
            "a = \"PKG.FN\" // fim\n/* bloco\n continua */ b = 'x\\'y'\n",
            &Sintaxe::C,
        );
        assert_eq!(s.codigo[0], "a = \"      \"       ");
        assert_eq!(s.textos[0], (1, "PKG.FN".into()));
        assert_eq!(s.textos[1], (3, "x\\'y".into()));
        assert_eq!(s.comentarios[0].0, 1);
        assert!(!s.comentarios[0].2);
        assert!(s.comentarios[1].2);
        assert!(s.codigo[2].trim_start().starts_with("b = '"));
    }

    #[test]
    fn definicoes_python_e_c() {
        let e = extrair(
            Linguagem::Python,
            "# Cálculo de encargos\nclass Encargos:\n    # multa\n    def multa(self):\n        return 1\n\n    def juros(self):\n        pass\n\ndef solta():\n    pass\n",
        );
        let nomes: Vec<_> = e
            .simbolos
            .iter()
            .map(|s| (s.qualificado.as_str(), s.tipo, s.linhas))
            .collect();
        assert_eq!(
            nomes,
            [
                ("Encargos", TipoSimbolo::Classe, Linhas::new(2, 8)),
                ("Encargos.multa", TipoSimbolo::Funcao, Linhas::new(4, 5)),
                ("Encargos.juros", TipoSimbolo::Funcao, Linhas::new(7, 8)),
                ("solta", TipoSimbolo::Funcao, Linhas::new(10, 11)),
            ]
        );
        assert_eq!(
            e.simbolos[0].comentario.as_deref(),
            Some("Cálculo de encargos")
        );
        assert_eq!(e.simbolos[1].comentario.as_deref(), Some("multa"));

        let e = extrair(
            Linguagem::Go,
            "package x\n\nfunc (s *Servico) Calcular() int {\n\treturn 1\n}\n",
        );
        assert_eq!(e.simbolos[0].nome, "Calcular");
        assert_eq!(e.simbolos[0].linhas, Linhas::new(3, 5));

        let e = extrair(Linguagem::Shell, "#!/bin/sh\ncarregar() {\n  echo\n}\n");
        assert_eq!(e.simbolos[0].nome, "carregar");
        assert_eq!(e.simbolos[0].linhas, Linhas::new(2, 4));
    }

    #[test]
    fn mencoes_em_strings_e_codigo() {
        let e = extrair(
            Linguagem::Java,
            "class A {\n  // chama PKG_X.FN_COMENTARIO\n  void f() {\n    call(\"PKG_COBRANCA.FN_CALCULAR_MULTA\", MAX_PARCELAS);\n    q(\"select * from fatura where x = 1\");\n  }\n}\n",
        );
        let m: Vec<_> = e
            .referencias
            .iter()
            .map(|r| (r.origem.as_deref(), r.alvo.as_str(), r.linha))
            .collect();
        assert_eq!(
            m,
            [
                (Some("A"), "MAX_PARCELAS", 4),
                (Some("A"), "PKG_COBRANCA.FN_CALCULAR_MULTA", 4),
                (Some("A"), "fatura", 5),
            ]
        );
        assert!(
            e.referencias
                .iter()
                .all(|r| r.tipo == TipoReferencia::Mencao)
        );
        // Nada em documentação.
        assert_eq!(
            extrair(Linguagem::Markdown, "# FATURA_X\nPKG_COBRANCA"),
            Extracao::default()
        );
        // XML: só menções, sem definições.
        let e = extrair(
            Linguagem::Xml,
            "<!-- class Nada -->\n<call proc=\"PKG_COBRANCA.PRC_CANCELAR_FATURA\"/>",
        );
        assert!(e.simbolos.is_empty());
        assert_eq!(e.referencias.len(), 1);
        assert_eq!(e.referencias[0].linha, 2);
    }
}
