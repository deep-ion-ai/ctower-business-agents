//! Transcripts fictícios do Faturamento Exemplo em `fixtures/reunioes/`.
//!
//! A reunião 1 existe nos três formatos (`.vtt`, `.docx` e `.txt` colado) com
//! o mesmo conteúdo; a reunião 2 (`.vtt`) tem participantes em comum, um deles
//! com o nome escrito de outro jeito.
//!
//! O `.docx` é gerado a partir do `.txt` por [`docx_da_reuniao_1`]. Para
//! regerar depois de mudar o `.txt`:
//!
//! ```text
//! CBA_ATUALIZAR_ESPERADOS=1 cargo test -p cba-reunioes --test fixtures_reunioes
//! ```

use std::fs;
use std::io::{Cursor, Write};
use std::path::PathBuf;

use cba_reunioes::{Fala, Formato, analisar};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

const REUNIAO_1: &str = "reuniao-01-cancelamento";
const REUNIAO_2: &str = "reuniao-02-fechamento";

fn pasta() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/reunioes")
}

fn ler(nome: &str) -> Vec<u8> {
    let caminho = pasta().join(nome);
    fs::read(&caminho).unwrap_or_else(|e| panic!("{}: {e}", caminho.display()))
}

fn sem_avisos(nome: &str, formato: Formato) -> Vec<Fala> {
    let t = analisar(&ler(nome), Some(nome));
    assert_eq!(t.formato, formato, "{nome}");
    assert!(t.avisos.is_empty(), "{nome}: {:?}", t.avisos);
    t.falas
}

fn falante_e_texto(falas: &[Fala]) -> Vec<(Option<&str>, &str)> {
    falas
        .iter()
        .map(|f| (f.falante.as_deref(), f.texto.as_str()))
        .collect()
}

#[test]
fn os_tres_formatos_da_reuniao_1_geram_as_mesmas_falas() {
    let vtt = sem_avisos(&format!("{REUNIAO_1}.vtt"), Formato::Vtt);
    let docx = sem_avisos(&format!("{REUNIAO_1}.docx"), Formato::Docx);
    let txt = sem_avisos(&format!("{REUNIAO_1}.txt"), Formato::TextoTeams);

    assert!(vtt.len() >= 20, "a reunião 1 tem {} falas", vtt.len());
    assert_eq!(falante_e_texto(&vtt), falante_e_texto(&docx));
    assert_eq!(falante_e_texto(&vtt), falante_e_texto(&txt));

    // O início coincide no segundo; só o WebVTT traz o fim.
    for ((v, d), t) in vtt.iter().zip(&docx).zip(&txt) {
        let segundos = |f: &Fala| f.inicio.map(|h| h.segundos());
        assert_eq!(segundos(v), segundos(d), "fala {}", v.indice);
        assert_eq!(segundos(v), segundos(t), "fala {}", v.indice);
        assert!(v.fim.is_some_and(|fim| Some(fim) > v.inicio));
        assert_eq!((d.fim, t.fim), (None, None));
        assert_eq!(
            (v.indice, d.indice, t.indice),
            (v.indice, v.indice, v.indice)
        );
    }

    // Reunião curta: entre 5 e 10 minutos.
    let duracao = vtt.last().and_then(|f| f.fim).unwrap().segundos();
    assert!((300..=600).contains(&duracao), "{duracao} s");
}

#[test]
fn nomes_da_reuniao_1_preservados_como_aparecem() {
    for extensao in ["vtt", "docx", "txt"] {
        let nome = format!("{REUNIAO_1}.{extensao}");
        let t = analisar(&ler(&nome), None);
        assert_eq!(
            t.falantes(),
            vec![
                "Silva, Maria",
                "João Pereira",
                "Ana Souza",
                "carlos.lima@exemplo.com"
            ],
            "{nome}"
        );
        // Falas consecutivas do mesmo falante continuam separadas.
        assert_eq!(t.falas[0].falante, t.falas[1].falante, "{nome}");
    }
}

#[test]
fn reuniao_2_tem_participantes_em_comum_com_nome_variado() {
    let r1 = analisar(&ler(&format!("{REUNIAO_1}.vtt")), None);
    let r2 = analisar(&ler(&format!("{REUNIAO_2}.vtt")), None);
    assert_eq!(r2.formato, Formato::Vtt);
    assert!(r2.avisos.is_empty(), "{:?}", r2.avisos);
    let falantes = r2.falantes();
    assert_eq!(
        falantes,
        vec!["Maria Silva", "Beatriz Rocha", "Ana Souza", "João Pereira"]
    );
    // "Silva, Maria" na reunião 1 e "Maria Silva" na 2: o parser não junta.
    assert!(r1.falantes().contains(&"Silva, Maria"));
    assert!(!r1.falantes().contains(&"Maria Silva"));
    let em_comum: Vec<_> = falantes
        .iter()
        .filter(|n| r1.falantes().contains(n))
        .collect();
    assert_eq!(em_comum, vec![&"Ana Souza", &"João Pereira"]);
}

// ---------------------------------------------------------------------------
// Geração do .docx

/// Monta o `.docx` da reunião 1 a partir do `.txt`, no layout suposto do
/// export do Teams (ver `cba_reunioes::paragrafos_docx`): cabeçalho com
/// título, data e duração, a linha de sistema do início da transcrição e,
/// para cada fala, um parágrafo `Nome<tab>M:SS` seguido do parágrafo da fala.
///
/// Reprodutível: entradas sem compressão e com data fixa, para os bytes
/// serem os mesmos em qualquer máquina.
fn docx_da_reuniao_1(txt: &str) -> Vec<u8> {
    let mut paragrafos: Vec<Vec<String>> = vec![
        vec!["Cancelamento de fatura - alinhamento-20261110_100000-Gravação da reunião".into()],
        vec!["10 de novembro de 2026, 10:00AM".into()],
        vec!["5 min 35 s".into()],
        vec!["Silva, Maria iniciou a transcrição".into()],
    ];
    for linha in txt.lines().filter(|l| !l.trim().is_empty()) {
        // Cabeçalho da fala: nome e horário separados por dois espaços no
        // .txt; no .docx, por tabulação.
        match linha.rsplit_once("  ") {
            Some((nome, hora)) if hora.contains(':') && !hora.contains(' ') => {
                paragrafos.push(vec![nome.to_string(), hora.to_string()]);
            }
            _ => paragrafos.push(vec![linha.to_string()]),
        }
    }
    paragrafos.push(vec!["Silva, Maria parou a transcrição".into()]);

    let mut corpo = String::new();
    for partes in &paragrafos {
        corpo.push_str("<w:p>");
        for (i, parte) in partes.iter().enumerate() {
            corpo.push_str("<w:r>");
            if i > 0 {
                corpo.push_str("<w:tab/>");
            }
            corpo.push_str(&format!(
                "<w:t xml:space=\"preserve\">{}</w:t>",
                escapar(parte)
            ));
            corpo.push_str("</w:r>");
        }
        corpo.push_str("</w:p>\n");
    }
    let documento = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\">\
         <w:body>\n{corpo}</w:body></w:document>\n"
    );
    let tipos = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
        <Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
        <Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
        <Default Extension=\"xml\" ContentType=\"application/xml\"/>\
        <Override PartName=\"/word/document.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml\"/>\
        </Types>\n";
    let rels = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
        <Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
        <Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"word/document.xml\"/>\
        </Relationships>\n";

    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let opcoes = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .last_modified_time(zip::DateTime::default());
    for (nome, conteudo) in [
        ("[Content_Types].xml", tipos),
        ("_rels/.rels", rels),
        ("word/document.xml", documento.as_str()),
    ] {
        zip.start_file(nome, opcoes).unwrap();
        zip.write_all(conteudo.as_bytes()).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

fn escapar(texto: &str) -> String {
    texto
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[test]
fn docx_da_reuniao_1_esta_em_dia_com_o_txt() {
    let txt = String::from_utf8(ler(&format!("{REUNIAO_1}.txt"))).unwrap();
    let gerado = docx_da_reuniao_1(&txt);
    let caminho = pasta().join(format!("{REUNIAO_1}.docx"));
    if std::env::var_os("CBA_ATUALIZAR_ESPERADOS").is_some() {
        fs::write(&caminho, &gerado).unwrap();
        return;
    }
    let atual = fs::read(&caminho).unwrap_or_default();
    assert!(
        atual == gerado,
        "{} não corresponde ao .txt; regere com \
         CBA_ATUALIZAR_ESPERADOS=1 cargo test -p cba-reunioes --test fixtures_reunioes",
        caminho.display()
    );
}
