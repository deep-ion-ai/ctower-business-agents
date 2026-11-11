//! Linguagem de um arquivo, pela extensão e pelo conteúdo.
//!
//! A extensão decide na maioria dos casos. O conteúdo (só o início do arquivo)
//! desempata extensões ambíguas (`.inc`, `.cls`, `.frm`) e identifica scripts sem
//! extensão pela linha `#!`. Os extratores (#15 a #17) escolhem o analisador por
//! esta classificação.

use serde::{Deserialize, Serialize};

/// Linguagem ou tipo de conteúdo de um arquivo de texto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Linguagem {
    Java,
    Kotlin,
    CSharp,
    VisualBasicNet,
    TypeScript,
    JavaScript,
    Python,
    /// SQL e PL/SQL (DDL, pacotes, procedures).
    Sql,
    /// COBOL, inclusive copybooks e Pro*COBOL.
    Cobol,
    /// Delphi e Object Pascal, inclusive formulários `.dfm` em texto.
    Delphi,
    /// Visual Basic 6: módulos, classes, formulários e projetos.
    VisualBasic6,
    C,
    Cpp,
    Go,
    Php,
    Ruby,
    Shell,
    PowerShell,
    Batch,
    Html,
    Css,
    Xml,
    Json,
    Yaml,
    /// Configuração em texto (`.properties`, `.ini`, `.conf`, `.toml`).
    Configuracao,
    Markdown,
    Texto,
    /// Texto sem linguagem reconhecida.
    Outra,
}

impl Linguagem {
    /// Nome para a interface.
    pub fn nome(self) -> &'static str {
        match self {
            Linguagem::Java => "Java",
            Linguagem::Kotlin => "Kotlin",
            Linguagem::CSharp => "C#",
            Linguagem::VisualBasicNet => "VB.NET",
            Linguagem::TypeScript => "TypeScript",
            Linguagem::JavaScript => "JavaScript",
            Linguagem::Python => "Python",
            Linguagem::Sql => "SQL",
            Linguagem::Cobol => "COBOL",
            Linguagem::Delphi => "Delphi",
            Linguagem::VisualBasic6 => "Visual Basic 6",
            Linguagem::C => "C",
            Linguagem::Cpp => "C++",
            Linguagem::Go => "Go",
            Linguagem::Php => "PHP",
            Linguagem::Ruby => "Ruby",
            Linguagem::Shell => "Shell",
            Linguagem::PowerShell => "PowerShell",
            Linguagem::Batch => "Batch",
            Linguagem::Html => "HTML",
            Linguagem::Css => "CSS",
            Linguagem::Xml => "XML",
            Linguagem::Json => "JSON",
            Linguagem::Yaml => "YAML",
            Linguagem::Configuracao => "Configuração",
            Linguagem::Markdown => "Markdown",
            Linguagem::Texto => "Texto",
            Linguagem::Outra => "Outra",
        }
    }

    /// Detecta a linguagem pelo nome do arquivo e pelo início do conteúdo.
    pub fn detectar(nome: &str, inicio: &[u8]) -> Self {
        let nome = nome.to_ascii_lowercase();
        let ext = match nome.rsplit_once('.') {
            Some((base, ext)) if !base.is_empty() => ext,
            _ => "",
        };
        let texto = String::from_utf8_lossy(&inicio[..inicio.len().min(4096)]);
        use Linguagem::*;
        match ext {
            "java" => Java,
            "kt" | "kts" => Kotlin,
            "cs" => CSharp,
            "vb" => VisualBasicNet,
            "ts" | "tsx" | "mts" | "cts" => TypeScript,
            "js" | "jsx" | "mjs" | "cjs" => JavaScript,
            "py" => Python,
            "sql" | "pls" | "pks" | "pkb" | "pck" | "plb" | "prc" | "fnc" | "trg" | "ddl" => Sql,
            "cbl" | "cob" | "cpy" | "pco" | "ccp" => Cobol,
            "pas" | "dpr" | "dpk" | "dfm" | "lpr" | "pp" => Delphi,
            "bas" | "vbp" | "ctl" => VisualBasic6,
            "frm" if texto.trim_start().starts_with("VERSION ") => VisualBasic6,
            "cls" if texto.trim_start().starts_with("VERSION ") => VisualBasic6,
            "inc" => inc(&texto),
            "c" => C,
            "h" => {
                if texto.contains("class ") || texto.contains("namespace ") {
                    Cpp
                } else {
                    C
                }
            }
            "cpp" | "cc" | "cxx" | "hpp" | "hh" | "hxx" => Cpp,
            "go" => Go,
            "php" => Php,
            "rb" => Ruby,
            "sh" | "bash" | "ksh" | "zsh" => Shell,
            "ps1" | "psm1" | "psd1" => PowerShell,
            "bat" | "cmd" => Batch,
            "html" | "htm" | "xhtml" | "jsp" | "aspx" | "cshtml" => Html,
            "css" | "scss" | "sass" | "less" => Css,
            "xml" | "xsd" | "xsl" | "xslt" | "wsdl" | "pom" | "csproj" | "vbproj" | "config" => Xml,
            "json" => Json,
            "yaml" | "yml" => Yaml,
            "properties" | "ini" | "conf" | "cfg" | "toml" => Configuracao,
            "md" | "markdown" => Markdown,
            "txt" | "log" | "csv" => Texto,
            "" => shebang(&texto).unwrap_or(Outra),
            _ => Outra,
        }
    }
}

/// `#!/bin/sh`, `#!/usr/bin/env python3` etc.
fn shebang(texto: &str) -> Option<Linguagem> {
    let linha = texto.lines().next()?.strip_prefix("#!")?;
    let mut partes = linha.split_whitespace();
    let mut prog = partes.next()?.rsplit('/').next()?;
    if prog == "env" {
        prog = partes.find(|p| !p.starts_with('-'))?;
    }
    let prog = prog.trim_end_matches(|c: char| c.is_ascii_digit() || c == '.');
    Some(match prog {
        "sh" | "bash" | "ksh" | "zsh" | "dash" => Linguagem::Shell,
        "python" => Linguagem::Python,
        "node" => Linguagem::JavaScript,
        "ruby" => Linguagem::Ruby,
        "php" => Linguagem::Php,
        "pwsh" => Linguagem::PowerShell,
        _ => return None,
    })
}

/// `.inc` é include de Pascal/Delphi, de PHP ou de outras linguagens.
fn inc(texto: &str) -> Linguagem {
    let t = texto.to_ascii_lowercase();
    if t.contains("<?php") {
        Linguagem::Php
    } else if ["procedure ", "function ", "begin", "{$"]
        .iter()
        .any(|p| t.contains(p))
    {
        Linguagem::Delphi
    } else {
        Linguagem::Outra
    }
}

#[cfg(test)]
mod tests {
    use super::Linguagem::*;
    use super::*;

    #[test]
    fn pela_extensao() {
        assert_eq!(Linguagem::detectar("A.java", b""), Java);
        assert_eq!(Linguagem::detectar("03_pkg.SQL", b""), Sql);
        assert_eq!(Linguagem::detectar("CALCOBR.cbl", b""), Cobol);
        assert_eq!(Linguagem::detectar("UFrm.dfm", b""), Delphi);
        assert_eq!(Linguagem::detectar("tela.ts", b""), TypeScript);
        assert_eq!(Linguagem::detectar("x.desconhecida", b""), Outra);
        // Arquivo oculto sem extensão: o nome não é extensão.
        assert_eq!(Linguagem::detectar(".profile", b""), Outra);
    }

    #[test]
    fn pelo_conteudo() {
        assert_eq!(Linguagem::detectar("rodar", b"#!/bin/bash\necho"), Shell);
        assert_eq!(
            Linguagem::detectar("carga", b"#!/usr/bin/env python3\n"),
            Python
        );
        assert_eq!(
            Linguagem::detectar("Cliente.cls", b"VERSION 1.0 CLASS\nBEGIN"),
            VisualBasic6
        );
        assert_eq!(
            Linguagem::detectar("Cliente.cls", b"\\documentclass"),
            Outra
        );
        assert_eq!(
            Linguagem::detectar("FrmA.frm", b"VERSION 5.00\nBegin VB.Form"),
            VisualBasic6
        );
        assert_eq!(
            Linguagem::detectar("defs.inc", b"{$I-}\nconst X = 1;"),
            Delphi
        );
        assert_eq!(Linguagem::detectar("topo.inc", b"<?php echo 1;"), Php);
    }
}
