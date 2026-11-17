//! Planilha do Excel (`.xlsx`, Office Open XML) sem dependências novas: o XML das
//! partes escrito à mão e compactado com `zip`, como o Word ([`crate::word`]).
//!
//! Cada [`Aba`] é uma folha com a primeira linha como cabeçalho (negrito, fixo na
//! rolagem e com filtro). As células são texto (`inlineStr`), com quebra de linha
//! automática; a largura de cada coluna segue o texto mais longo, até um limite.
//! A saída é determinística: as mesmas abas dão sempre os mesmos bytes.
//!
//! Serve para a exportação da matriz de rastreabilidade (#121).

use std::fmt::Write as _;
use std::io::{Cursor, Write};

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

const NS_MAIN: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const NS_R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const NS_REL: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const REL_DOC: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

/// Largura mínima e máxima de uma coluna, em caracteres.
const LARGURA_MINIMA: usize = 8;
const LARGURA_MAXIMA: usize = 60;
/// Tamanho máximo do nome de uma aba no Excel.
const NOME_MAXIMO: usize = 31;

/// Uma folha da planilha: a primeira linha é o cabeçalho.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aba {
    pub nome: String,
    pub linhas: Vec<Vec<String>>,
}

impl Aba {
    pub fn nova(nome: impl Into<String>, cabecalho: &[&str]) -> Self {
        Self {
            nome: nome.into(),
            linhas: vec![cabecalho.iter().map(|c| (*c).to_owned()).collect()],
        }
    }

    pub fn linha(&mut self, celulas: Vec<String>) {
        self.linhas.push(celulas);
    }
}

/// Gera o `.xlsx` com as abas, na ordem.
pub fn xlsx(abas: &[Aba]) -> zip::result::ZipResult<Vec<u8>> {
    let nomes = nomes_das_abas(abas);
    let mut partes: Vec<(String, String)> = vec![
        ("[Content_Types].xml".into(), tipos(abas.len())),
        (
            "_rels/.rels".into(),
            format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
                 <Relationships xmlns=\"{NS_REL}\">\
                 <Relationship Id=\"rId1\" Type=\"{REL_DOC}/officeDocument\" Target=\"xl/workbook.xml\"/>\
                 </Relationships>"
            ),
        ),
        ("xl/workbook.xml".into(), pasta(&nomes)),
        ("xl/_rels/workbook.xml.rels".into(), relacoes(abas.len())),
        ("xl/styles.xml".into(), estilos()),
    ];
    for (i, aba) in abas.iter().enumerate() {
        partes.push((format!("xl/worksheets/sheet{}.xml", i + 1), folha(aba)));
    }
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let opcoes = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .last_modified_time(zip::DateTime::default());
    for (nome, xml) in partes {
        zip.start_file(nome, opcoes)?;
        zip.write_all(xml.as_bytes())?;
    }
    Ok(zip.finish()?.into_inner())
}

/// Nomes válidos e únicos: sem `[]:*?/\`, até 31 caracteres.
fn nomes_das_abas(abas: &[Aba]) -> Vec<String> {
    let mut saida: Vec<String> = Vec::new();
    for (i, aba) in abas.iter().enumerate() {
        let limpo: String = aba
            .nome
            .chars()
            .map(|c| if "[]:*?/\\".contains(c) { '-' } else { c })
            .collect();
        let limpo = limpo.trim().trim_matches('\'').to_owned();
        let mut nome: String = if limpo.is_empty() {
            format!("Aba {}", i + 1)
        } else {
            limpo.chars().take(NOME_MAXIMO).collect()
        };
        let mut n = 2;
        while saida.iter().any(|s| s.eq_ignore_ascii_case(&nome)) {
            let sufixo = format!(" ({n})");
            let base: String = nome
                .chars()
                .take(NOME_MAXIMO - sufixo.chars().count())
                .collect();
            nome = format!("{base}{sufixo}");
            n += 1;
        }
        saida.push(nome);
    }
    saida
}

fn tipos(abas: usize) -> String {
    let mut s = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
         <Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
         <Default Extension=\"xml\" ContentType=\"application/xml\"/>\
         <Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/>\
         <Override PartName=\"/xl/styles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml\"/>",
    );
    for i in 1..=abas {
        let _ = write!(
            s,
            "<Override PartName=\"/xl/worksheets/sheet{i}.xml\" \
             ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/>"
        );
    }
    s.push_str("</Types>");
    s
}

fn pasta(nomes: &[String]) -> String {
    let mut s = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <workbook xmlns=\"{NS_MAIN}\" xmlns:r=\"{NS_R}\"><sheets>"
    );
    for (i, nome) in nomes.iter().enumerate() {
        let _ = write!(
            s,
            "<sheet name=\"{}\" sheetId=\"{}\" r:id=\"rId{}\"/>",
            escapar(nome),
            i + 1,
            i + 1
        );
    }
    s.push_str("</sheets></workbook>");
    s
}

fn relacoes(abas: usize) -> String {
    let mut s = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <Relationships xmlns=\"{NS_REL}\">"
    );
    for i in 1..=abas {
        let _ = write!(
            s,
            "<Relationship Id=\"rId{i}\" Type=\"{REL_DOC}/worksheet\" Target=\"worksheets/sheet{i}.xml\"/>"
        );
    }
    let _ = write!(
        s,
        "<Relationship Id=\"rId{}\" Type=\"{REL_DOC}/styles\" Target=\"styles.xml\"/>",
        abas + 1
    );
    s.push_str("</Relationships>");
    s
}

/// Estilo 0: texto com quebra, alinhado no topo. Estilo 1: o mesmo, em negrito e
/// com fundo cinza (cabeçalho).
fn estilos() -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <styleSheet xmlns=\"{NS_MAIN}\">\
         <fonts count=\"2\"><font><sz val=\"11\"/><name val=\"Calibri\"/></font>\
         <font><b/><sz val=\"11\"/><name val=\"Calibri\"/></font></fonts>\
         <fills count=\"3\"><fill><patternFill patternType=\"none\"/></fill>\
         <fill><patternFill patternType=\"gray125\"/></fill>\
         <fill><patternFill patternType=\"solid\"><fgColor rgb=\"FFE7E6E6\"/><bgColor indexed=\"64\"/></patternFill></fill></fills>\
         <borders count=\"1\"><border><left/><right/><top/><bottom/><diagonal/></border></borders>\
         <cellStyleXfs count=\"1\"><xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\"/></cellStyleXfs>\
         <cellXfs count=\"2\">\
         <xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\" xfId=\"0\" applyAlignment=\"1\"><alignment vertical=\"top\" wrapText=\"1\"/></xf>\
         <xf numFmtId=\"0\" fontId=\"1\" fillId=\"2\" borderId=\"0\" xfId=\"0\" applyFont=\"1\" applyFill=\"1\" applyAlignment=\"1\"><alignment vertical=\"top\" wrapText=\"1\"/></xf>\
         </cellXfs>\
         <cellStyles count=\"1\"><cellStyle name=\"Normal\" xfId=\"0\" builtinId=\"0\"/></cellStyles>\
         </styleSheet>"
    )
}

/// `A`, `B`, …, `Z`, `AA`, … (coluna a partir de 0).
fn coluna(mut n: usize) -> String {
    let mut letras = Vec::new();
    loop {
        letras.push(char::from(b'A' + u8::try_from(n % 26).unwrap_or(0)));
        if n < 26 {
            break;
        }
        n = n / 26 - 1;
    }
    letras.iter().rev().collect()
}

fn folha(aba: &Aba) -> String {
    let colunas = aba.linhas.iter().map(Vec::len).max().unwrap_or(0);
    let mut s = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <worksheet xmlns=\"{NS_MAIN}\" xmlns:r=\"{NS_R}\">"
    );
    if !aba.linhas.is_empty() {
        s.push_str(
            "<sheetViews><sheetView workbookViewId=\"0\">\
             <pane ySplit=\"1\" topLeftCell=\"A2\" activePane=\"bottomLeft\" state=\"frozen\"/>\
             </sheetView></sheetViews>",
        );
    }
    if colunas > 0 {
        s.push_str("<cols>");
        for c in 0..colunas {
            let largura = aba
                .linhas
                .iter()
                .filter_map(|l| l.get(c))
                .map(|t| t.lines().map(|l| l.chars().count()).max().unwrap_or(0))
                .max()
                .unwrap_or(0)
                .clamp(LARGURA_MINIMA, LARGURA_MAXIMA);
            let _ = write!(
                s,
                "<col min=\"{n}\" max=\"{n}\" width=\"{}\" customWidth=\"1\"/>",
                largura + 2,
                n = c + 1
            );
        }
        s.push_str("</cols>");
    }
    s.push_str("<sheetData>");
    for (i, linha) in aba.linhas.iter().enumerate() {
        let r = i + 1;
        let _ = write!(s, "<row r=\"{r}\">");
        for (c, texto) in linha.iter().enumerate() {
            let estilo = if i == 0 { 1 } else { 0 };
            let _ = write!(
                s,
                "<c r=\"{}{r}\" s=\"{estilo}\" t=\"inlineStr\"><is><t xml:space=\"preserve\">{}</t></is></c>",
                coluna(c),
                escapar(texto)
            );
        }
        s.push_str("</row>");
    }
    s.push_str("</sheetData>");
    if colunas > 0 && !aba.linhas.is_empty() {
        let _ = write!(
            s,
            "<autoFilter ref=\"A1:{}{}\"/>",
            coluna(colunas - 1),
            aba.linhas.len()
        );
    }
    s.push_str("</worksheet>");
    s
}

/// Escapa para XML e tira os caracteres de controle que o XML não aceita.
fn escapar(texto: &str) -> String {
    let mut s = String::with_capacity(texto.len());
    for c in texto.chars() {
        match c {
            '&' => s.push_str("&amp;"),
            '<' => s.push_str("&lt;"),
            '>' => s.push_str("&gt;"),
            '"' => s.push_str("&quot;"),
            '\n' | '\t' => s.push(c),
            '\r' => {}
            c if (c as u32) < 0x20 => {}
            c => s.push(c),
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use std::io::Read;

    use super::*;

    fn parte(bytes: &[u8], nome: &str) -> String {
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        let mut texto = String::new();
        zip.by_name(nome)
            .unwrap()
            .read_to_string(&mut texto)
            .unwrap();
        texto
    }

    #[test]
    fn colunas_em_letras() {
        assert_eq!(coluna(0), "A");
        assert_eq!(coluna(25), "Z");
        assert_eq!(coluna(26), "AA");
        assert_eq!(coluna(27), "AB");
        assert_eq!(coluna(701), "ZZ");
        assert_eq!(coluna(702), "AAA");
    }

    #[test]
    fn planilha_com_abas_cabecalho_e_texto_escapado() {
        let mut matriz = Aba::nova("Matriz", &["História", "Regras"]);
        matriz.linha(vec![
            "HU-0001 <PJ> & cia".into(),
            "RN-FAT-007\nRN-FAT-008".into(),
        ]);
        let lacunas = Aba::nova("Lacunas: [todas]", &["Lacuna"]);
        let bytes = xlsx(&[matriz.clone(), lacunas, Aba::nova("Matriz", &["x"])]).unwrap();
        assert_eq!(
            bytes,
            xlsx(&[
                matriz,
                Aba::nova("Lacunas: [todas]", &["Lacuna"]),
                Aba::nova("Matriz", &["x"])
            ])
            .unwrap()
        );

        let pasta = parte(&bytes, "xl/workbook.xml");
        assert!(pasta.contains("name=\"Matriz\""), "{pasta}");
        assert!(pasta.contains("name=\"Lacunas- -todas-\""), "{pasta}");
        assert!(pasta.contains("name=\"Matriz (2)\""), "{pasta}");
        let folha = parte(&bytes, "xl/worksheets/sheet1.xml");
        assert!(folha.contains("HU-0001 &lt;PJ&gt; &amp; cia"), "{folha}");
        assert!(folha.contains("<c r=\"B2\" s=\"0\""), "{folha}");
        assert!(folha.contains("<c r=\"A1\" s=\"1\""), "{folha}");
        assert!(folha.contains("<autoFilter ref=\"A1:B2\"/>"), "{folha}");
        assert!(folha.contains("state=\"frozen\""), "{folha}");
        let tipos = parte(&bytes, "[Content_Types].xml");
        assert!(tipos.contains("/xl/worksheets/sheet3.xml"), "{tipos}");
    }
}
