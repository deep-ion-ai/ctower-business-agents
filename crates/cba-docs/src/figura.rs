//! Figuras dos diagramas para o Word (#118).
//!
//! O `cba-docs` não renderiza Mermaid: quem exporta (o app, na webview, com a
//! mesma biblioteca da prévia) renderiza cada bloco ` ```mermaid ` do documento
//! ([`blocos_mermaid`]) em PNG e entrega as imagens em [`Figuras`], pelo código
//! do bloco. Na exportação para Word, o bloco com figura vira a imagem; o bloco
//! sem figura (Mermaid inválido, falha ao renderizar, exportação fora do app)
//! vira um aviso seguido do código, e o resto do documento sai normalmente.

use std::collections::HashMap;

use pulldown_cmark::{CodeBlockKind, Event, Parser, Tag, TagEnd};

use crate::diagrama::LINGUAGEM_MERMAID;
use crate::exportacao::opcoes_markdown;

/// Tamanho máximo de uma figura.
pub const TAMANHO_MAXIMO_DA_FIGURA: usize = 8 * 1024 * 1024;
/// Figuras no máximo numa exportação.
pub const MAXIMO_DE_FIGURAS: usize = 100;

const ASSINATURA_PNG: &[u8] = b"\x89PNG\r\n\x1a\n";

/// Imagem PNG de um diagrama e o tamanho em que aparece (em pixels CSS, a 96
/// pontos por polegada; a imagem pode ter mais resolução).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Figura {
    pub png: Vec<u8>,
    pub largura: u32,
    pub altura: u32,
}

/// Figura recusada.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ErroFigura {
    #[error("a figura não é um PNG")]
    NaoEPng,
    #[error("a figura passa de {} MiB", TAMANHO_MAXIMO_DA_FIGURA / 1024 / 1024)]
    Grande,
    #[error("mais de {MAXIMO_DE_FIGURAS} figuras")]
    Muitas,
}

impl Figura {
    /// Confere o PNG (assinatura e cabeçalho) e completa o tamanho que vier zerado
    /// com o do próprio PNG.
    pub fn nova(png: Vec<u8>, largura: u32, altura: u32) -> Result<Self, ErroFigura> {
        if png.len() > TAMANHO_MAXIMO_DA_FIGURA {
            return Err(ErroFigura::Grande);
        }
        let (l, a) = dimensoes_png(&png).ok_or(ErroFigura::NaoEPng)?;
        let (largura, altura) = if largura == 0 || altura == 0 {
            (l, a)
        } else {
            (largura, altura)
        };
        Ok(Self {
            png,
            largura,
            altura,
        })
    }
}

/// Largura e altura do cabeçalho `IHDR` do PNG.
fn dimensoes_png(png: &[u8]) -> Option<(u32, u32)> {
    if png.len() < 24 || !png.starts_with(ASSINATURA_PNG) || &png[12..16] != b"IHDR" {
        return None;
    }
    let numero = |i: usize| u32::from_be_bytes([png[i], png[i + 1], png[i + 2], png[i + 3]]);
    let (l, a) = (numero(16), numero(20));
    (l > 0 && a > 0).then_some((l, a))
}

/// Chave do bloco: o código sem `\r` e sem espaços e quebras no fim.
fn chave(codigo: &str) -> String {
    codigo.replace("\r\n", "\n").trim_end().to_owned()
}

/// Figuras renderizadas, pelo código Mermaid do bloco.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Figuras {
    mapa: HashMap<String, Figura>,
}

impl Figuras {
    pub fn novas() -> Self {
        Self::default()
    }

    /// Guarda a figura do bloco de código `codigo`.
    pub fn inserir(&mut self, codigo: &str, figura: Figura) -> Result<(), ErroFigura> {
        let chave = chave(codigo);
        if !self.mapa.contains_key(&chave) && self.mapa.len() >= MAXIMO_DE_FIGURAS {
            return Err(ErroFigura::Muitas);
        }
        self.mapa.insert(chave, figura);
        Ok(())
    }

    /// Figura do bloco, se houver.
    pub fn figura(&self, codigo: &str) -> Option<&Figura> {
        self.mapa.get(&chave(codigo))
    }

    pub fn is_empty(&self) -> bool {
        self.mapa.is_empty()
    }
}

/// O bloco de código é de Mermaid (` ```mermaid `).
pub(crate) fn e_mermaid(tipo: &CodeBlockKind) -> bool {
    match tipo {
        CodeBlockKind::Fenced(info) => info
            .split_whitespace()
            .next()
            .is_some_and(|l| l.eq_ignore_ascii_case(LINGUAGEM_MERMAID)),
        CodeBlockKind::Indented => false,
    }
}

/// Código dos blocos ` ```mermaid ` do Markdown, na ordem, sem repetição: o que o
/// app precisa renderizar para exportar o documento em Word.
pub fn blocos_mermaid(markdown: &str) -> Vec<String> {
    let mut blocos: Vec<String> = Vec::new();
    let mut atual: Option<String> = None;
    for evento in Parser::new_ext(markdown, opcoes_markdown()) {
        match evento {
            Event::Start(Tag::CodeBlock(tipo)) if e_mermaid(&tipo) => atual = Some(String::new()),
            Event::Text(t) => {
                if let Some(a) = &mut atual {
                    a.push_str(&t);
                }
            }
            Event::End(TagEnd::CodeBlock) => {
                if let Some(codigo) = atual.take() {
                    let codigo = chave(&codigo);
                    if !blocos.contains(&codigo) {
                        blocos.push(codigo);
                    }
                }
            }
            _ => {}
        }
    }
    blocos
}

#[cfg(test)]
mod tests {
    use super::*;

    /// PNG mínimo (só a assinatura e o IHDR), suficiente para o cabeçalho.
    pub(crate) fn png(largura: u32, altura: u32) -> Vec<u8> {
        let mut p = ASSINATURA_PNG.to_vec();
        p.extend_from_slice(&13u32.to_be_bytes());
        p.extend_from_slice(b"IHDR");
        p.extend_from_slice(&largura.to_be_bytes());
        p.extend_from_slice(&altura.to_be_bytes());
        p.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
        p
    }

    #[test]
    fn confere_o_png_e_completa_o_tamanho() {
        let f = Figura::nova(png(400, 200), 0, 0).unwrap();
        assert_eq!((f.largura, f.altura), (400, 200));
        let f = Figura::nova(png(800, 400), 400, 200).unwrap();
        assert_eq!((f.largura, f.altura), (400, 200));
        assert_eq!(
            Figura::nova(b"<svg/>".to_vec(), 1, 1),
            Err(ErroFigura::NaoEPng)
        );
        assert_eq!(Figura::nova(png(0, 10), 0, 0), Err(ErroFigura::NaoEPng));
    }

    #[test]
    fn blocos_na_ordem_sem_repeticao() {
        let md = "# T\n\n```mermaid\nflowchart LR\n  a --> b\n```\n\n```sql\nselect 1\n```\n\n~~~ Mermaid\r\nflowchart TD\r\n  x\r\n~~~\n\n```mermaid\nflowchart LR\n  a --> b\n\n```\n";
        assert_eq!(
            blocos_mermaid(md),
            ["flowchart LR\n  a --> b", "flowchart TD\n  x"]
        );
    }

    #[test]
    fn figura_pelo_codigo_normalizado() {
        let mut f = Figuras::novas();
        f.inserir(
            "flowchart LR\r\n  a\r\n\n",
            Figura::nova(png(1, 1), 0, 0).unwrap(),
        )
        .unwrap();
        assert!(f.figura("flowchart LR\n  a").is_some());
        assert!(f.figura("flowchart LR\n  b").is_none());
    }
}
