//! Texto dos parágrafos de um `.docx` (zip com `word/document.xml`).

use std::fmt;
use std::io::{Cursor, Read};

use quick_xml::Reader;
use quick_xml::events::Event;

use super::texto::decodificar_entidades;

/// Tamanho máximo de `word/document.xml` lido (proteção contra zip-bomba).
const LIMITE_DOCUMENTO: u64 = 64 * 1024 * 1024;

/// Erro ao ler o texto de um `.docx`. A mensagem é técnica e não traz
/// conteúdo do documento.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErroDocx(String);

impl fmt::Display for ErroDocx {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ErroDocx {}

/// Texto de cada parágrafo (`w:p`) do corpo de um `.docx`, na ordem do
/// documento, inclusive os vazios e os de dentro de tabelas.
///
/// Dentro do parágrafo, `w:t` vira o texto, `w:tab` vira `\t` e `w:br` e
/// `w:cr` viram `\n`. Cabeçalho, rodapé, comentários e notas (outras partes
/// do pacote) não entram, e o texto apagado com revisão (`w:delText`) também
/// não.
///
/// Layout esperado do transcript exportado pelo Teams (não há especificação
/// publicada; suposição a partir do que o Teams mostra na tela e do relato
/// de quem processa o export): um parágrafo de cabeçalho por fala, com o nome
/// e o horário relativo separados por tabulação ou espaços, seguido de um ou
/// mais parágrafos com a fala; antes da primeira fala vêm o título, a data e
/// a duração da reunião. O export antigo usava blocos `0:0:0.0 --> 0:0:3.25`
/// / nome / fala, um por parágrafo ou separados por quebra de linha.
pub fn paragrafos_docx(bytes: &[u8]) -> Result<Vec<String>, ErroDocx> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|e| ErroDocx(format!("o arquivo não é um zip válido ({e})")))?;
    let mut parte = zip
        .by_name("word/document.xml")
        .map_err(|_| ErroDocx("o pacote não tem word/document.xml".to_string()))?;
    let mut xml = Vec::new();
    (&mut parte)
        .take(LIMITE_DOCUMENTO + 1)
        .read_to_end(&mut xml)
        .map_err(|e| ErroDocx(format!("falha ao descompactar word/document.xml ({e})")))?;
    if xml.len() as u64 > LIMITE_DOCUMENTO {
        return Err(ErroDocx(format!(
            "word/document.xml passa de {} MiB",
            LIMITE_DOCUMENTO / 1024 / 1024
        )));
    }
    let xml = String::from_utf8(xml)
        .map_err(|_| ErroDocx("word/document.xml não está em UTF-8".to_string()))?;
    paragrafos_do_xml(&xml)
}

fn paragrafos_do_xml(xml: &str) -> Result<Vec<String>, ErroDocx> {
    let mut leitor = Reader::from_str(xml);
    let mut paragrafos = Vec::new();
    let mut atual: Option<String> = None;
    let mut em_texto = false;
    loop {
        let evento = leitor
            .read_event()
            .map_err(|e| ErroDocx(format!("XML inválido em word/document.xml ({e})")))?;
        match evento {
            Event::Start(e) => match e.local_name().as_ref() {
                "p" => atual = Some(String::new()),
                "t" => em_texto = true,
                _ => {}
            },
            Event::Empty(e) => {
                let simbolo = match e.local_name().as_ref() {
                    "tab" => Some('\t'),
                    "br" | "cr" => Some('\n'),
                    "noBreakHyphen" => Some('-'),
                    "p" => {
                        paragrafos.push(String::new());
                        None
                    }
                    _ => None,
                };
                if let (Some(c), Some(p)) = (simbolo, atual.as_mut()) {
                    p.push(c);
                }
            }
            Event::End(e) => match e.local_name().as_ref() {
                "p" => {
                    if let Some(p) = atual.take() {
                        paragrafos.push(p);
                    }
                }
                "t" => em_texto = false,
                _ => {}
            },
            Event::Text(t) if em_texto => {
                if let Some(p) = atual.as_mut() {
                    p.push_str(&t.xml10_content());
                }
            }
            Event::CData(t) if em_texto => {
                if let Some(p) = atual.as_mut() {
                    p.push_str(&t);
                }
            }
            Event::GeneralRef(r) if em_texto => {
                if let Some(p) = atual.as_mut() {
                    let entidade = format!("&{};", &*r);
                    p.push_str(&decodificar_entidades(&entidade));
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(paragrafos)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn paragrafos_com_tabulacao_quebra_e_entidades() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
<w:p><w:r><w:t>Silva, Maria</w:t></w:r><w:r><w:tab/><w:t>0:03</w:t></w:r></w:p>
<w:p><w:r><w:t xml:space="preserve">P&amp;D &#233; </w:t></w:r><w:r><w:t>linha</w:t><w:br/><w:t>outra</w:t></w:r></w:p>
<w:p/>
<w:tbl><w:tr><w:tc><w:p><w:r><w:t>célula</w:t></w:r></w:p></w:tc></w:tr></w:tbl>
<w:p><w:r><w:delText>apagado</w:delText><w:t>fica</w:t></w:r></w:p>
</w:body></w:document>"#;
        assert_eq!(
            paragrafos_do_xml(xml).unwrap(),
            vec![
                "Silva, Maria\t0:03",
                "P&D é linha\noutra",
                "",
                "célula",
                "fica"
            ]
        );
    }

    #[test]
    fn erros_sem_panico() {
        assert!(paragrafos_docx(b"nada").is_err());
        assert!(paragrafos_do_xml("<w:p><w:t>a</w:p>").is_err());
    }
}
