//! Exportação dos documentos esperados da montagem (`tests/esperados/`), com o
//! código do sistema fictício "Faturamento Exemplo" (`fixtures/`) como fonte dos
//! trechos citados.
//!
//! O Markdown exportado é comparado com `tests/esperados/exportados/`. O `.docx`
//! é conferido abrindo o zip e lendo o XML, sem depender do Word.
//!
//! Para atualizar os textos esperados: `CBA_ATUALIZAR_ESPERADOS=1 cargo test -p
//! cba-docs`, e revise o diff.

use std::collections::BTreeMap;
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use cba_docs::{
    CodigoEmPasta, FonteDeTrechos, Formato, OpcoesExportacao, exportar, exportar_markdown,
    exportar_word, gravar_exportacao,
};
use quick_xml::Reader;
use quick_xml::events::Event;

const DOCUMENTOS: [&str; 11] = [
    "ata",
    "analise-impacto",
    "analise-impacto-vazia",
    "caso-de-uso",
    "funcionalidade",
    "funcionalidade-de-analise",
    "historias",
    "pergunta",
    "regra",
    "sugestao",
    "termo",
];

fn pasta_do_crate() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn documento(nome: &str) -> String {
    fs::read_to_string(pasta_do_crate().join(format!("tests/esperados/{nome}.md"))).unwrap()
}

fn codigo() -> CodigoEmPasta {
    CodigoEmPasta::nova(pasta_do_crate().join("../../fixtures/faturamento-exemplo"))
}

fn conferir(caminho: &Path, obtido: &str) {
    if std::env::var_os("CBA_ATUALIZAR_ESPERADOS").is_some() {
        fs::create_dir_all(caminho.parent().unwrap()).unwrap();
        fs::write(caminho, obtido).unwrap();
        return;
    }
    let esperado = fs::read_to_string(caminho).unwrap_or_else(|_| {
        panic!(
            "falta {}; rode com CBA_ATUALIZAR_ESPERADOS=1",
            caminho.display()
        )
    });
    assert_eq!(obtido, esperado, "{} mudou", caminho.display());
}

// ---------------------------------------------------------------------------
// Markdown

#[test]
fn markdown_exportado_de_cada_documento() {
    let codigo = codigo();
    for nome in DOCUMENTOS {
        let md = exportar_markdown(
            &documento(nome),
            &OpcoesExportacao::default(),
            Some(&codigo),
        );
        conferir(
            &pasta_do_crate().join(format!("tests/esperados/exportados/{nome}.md")),
            &md,
        );
        // Autocontido: sem frontmatter e sem links para documentos da base.
        assert!(!md.starts_with("---"), "{nome}");
        assert!(
            !md.contains(".md)") && !md.contains(".md>)"),
            "{nome}: {md}"
        );
    }
}

#[test]
fn markdown_preserva_secoes_confianca_e_evidencias() {
    let original = documento("analise-impacto");
    let md = exportar_markdown(&original, &OpcoesExportacao::default(), Some(&codigo()));
    for linha in original.lines().filter(|l| l.starts_with("## ")) {
        assert!(
            md.contains(&format!("\n{linha}\n")),
            "falta a seção {linha}"
        );
    }
    for selo in ["**[confirmado]**", "**[inferido]**", "**[a confirmar]**"] {
        assert!(md.contains(selo), "falta o selo {selo}");
    }
    assert!(md.contains(
        "[^1]: `backend/src/main/java/exemplo/faturamento/ParcelamentoService.java:30-32`"
    ));
    assert!(md.contains("[^4][^5]"));
    // Link para a sugestão vira texto.
    assert!(md.contains("- S-0001 · Limitar a tela ao parâmetro MAX_PARCELAS (`frontend/"));
    // Apêndice com o código citado, linhas numeradas.
    assert!(md.contains("## Apêndice: trechos citados"));
    assert!(md.contains("### [5] `db/02_parametros.sql:15-16`\n\n```sql\n15 | "));
}

#[test]
fn markdown_sem_codigo_nao_tem_apendice() {
    let md = exportar_markdown(&documento("regra"), &OpcoesExportacao::default(), None);
    assert!(!md.contains("Apêndice"));
    assert!(md.contains("[^1]: `db/03_pkg_cobranca.sql:71-72` · hash `hash-ficticio-1`"));
    assert!(md.contains("- **Origem:** analises/A-0001.md\n"));
}

#[test]
fn opcoes_tiram_perguntas_e_evidencias() {
    let opcoes = OpcoesExportacao {
        evidencias: false,
        perguntas: false,
        trechos: true,
    };
    let md = exportar_markdown(&documento("analise-impacto"), &opcoes, Some(&codigo()));
    assert!(!md.contains("[^"), "{md}");
    assert!(!md.contains("## Evidências"));
    assert!(!md.contains("## Perguntas"));
    assert!(!md.contains("Apêndice"));
    assert!(md.contains("## Riscos"));
    assert!(md.contains("## Sugestões de alteração"));
    assert!(md.contains("**[confirmado]**"));
}

#[test]
fn trecho_inexistente_fica_indicado() {
    struct Vazio;
    impl FonteDeTrechos for Vazio {
        fn trecho(
            &self,
            _: &cba_kb::CaminhoRelativo,
            _: cba_kb::frontmatter::Linhas,
        ) -> Option<String> {
            None
        }
    }
    let md = exportar_markdown(
        &documento("termo"),
        &OpcoesExportacao::default(),
        Some(&Vazio),
    );
    assert_eq!(
        md.matches("_Trecho indisponível no código consultado._")
            .count(),
        3
    );
}

// ---------------------------------------------------------------------------
// Word

/// Partes do `.docx`, conferindo que cada XML é bem formado.
fn partes(docx: &[u8]) -> BTreeMap<String, String> {
    let mut zip = zip::ZipArchive::new(Cursor::new(docx)).unwrap();
    let mut partes = BTreeMap::new();
    for i in 0..zip.len() {
        let mut arquivo = zip.by_index(i).unwrap();
        let mut texto = String::new();
        arquivo.read_to_string(&mut texto).unwrap();
        let nome = arquivo.name().unwrap().into_owned();
        bem_formado(&nome, &texto);
        partes.insert(nome, texto);
    }
    partes
}

fn bem_formado(nome: &str, xml: &str) {
    let mut leitor = Reader::from_str(xml);
    leitor.config_mut().check_end_names = true;
    let mut profundidade = 0i32;
    loop {
        match leitor.read_event() {
            Ok(Event::Start(_)) => profundidade += 1,
            Ok(Event::End(_)) => profundidade -= 1,
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(e) => panic!("{nome}: XML inválido: {e}"),
        }
    }
    assert_eq!(profundidade, 0, "{nome}: elementos sem fechar");
}

/// Texto visível de um trecho de XML do Word (os `w:t`), um parágrafo por
/// linha.
fn texto(xml: &str) -> String {
    let mut saida = String::new();
    let mut resto = xml;
    while let Some(i) = resto.find('<') {
        let fim = resto[i..].find('>').unwrap() + i;
        let marca = &resto[i + 1..fim];
        resto = &resto[fim + 1..];
        if marca == "/w:p" || marca == "w:p/" {
            saida.push('\n');
        } else if marca == "w:t" || marca.starts_with("w:t ") {
            let fecha = resto.find("</w:t>").unwrap();
            saida.push_str(
                &resto[..fecha]
                    .replace("&lt;", "<")
                    .replace("&gt;", ">")
                    .replace("&quot;", "\"")
                    .replace("&amp;", "&"),
            );
            resto = &resto[fecha..];
        }
    }
    saida
}

/// Parágrafos (`<w:p>` e `<w:p/>`) do corpo, fora das tabelas.
fn paragrafos(xml: &str) -> usize {
    xml.matches("<w:p>").count() + xml.matches("<w:p/>").count()
}

#[test]
fn docx_de_cada_documento_e_um_pacote_valido() {
    let codigo = codigo();
    for nome in DOCUMENTOS {
        let docx = exportar_word(
            &documento(nome),
            &OpcoesExportacao::default(),
            Some(&codigo),
        )
        .unwrap();
        let partes = partes(&docx);
        for parte in [
            "[Content_Types].xml",
            "_rels/.rels",
            "docProps/core.xml",
            "word/document.xml",
            "word/_rels/document.xml.rels",
            "word/styles.xml",
            "word/numbering.xml",
            "word/footnotes.xml",
            "word/settings.xml",
        ] {
            assert!(partes.contains_key(parte), "{nome}: falta {parte}");
        }
        // Uma nota do Word por referência `[^n]` no texto.
        let md = exportar_markdown(
            &documento(nome),
            &OpcoesExportacao::default(),
            Some(&codigo),
        );
        let referencias = md
            .lines()
            .filter(|l| !(l.starts_with("[^") && l.contains("]:")))
            .map(|l| l.matches("[^").count())
            .sum::<usize>();
        let doc = &partes["word/document.xml"];
        assert_eq!(
            doc.matches("<w:footnoteReference ").count(),
            referencias,
            "{nome}"
        );
        assert_eq!(
            partes["word/footnotes.xml"].matches("<w:footnote ").count(),
            referencias + 2,
            "{nome}: notas mais os dois separadores"
        );
        // Títulos: um por linha `#` fora dos blocos de código.
        let titulos = |nivel: usize| {
            let prefixo = format!("{} ", "#".repeat(nivel));
            let mut em_bloco = false;
            md.lines()
                .filter(|l| {
                    if l.starts_with("```") {
                        em_bloco = !em_bloco;
                    }
                    !em_bloco && l.starts_with(&prefixo)
                })
                .count()
        };
        for nivel in 1..=3 {
            assert_eq!(
                doc.matches(&format!("<w:pStyle w:val=\"Heading{nivel}\"/>"))
                    .count(),
                titulos(nivel),
                "{nome}: títulos de nível {nivel}"
            );
        }
    }
}

#[test]
fn docx_da_analise_tem_secoes_confianca_e_evidencias() {
    let docx = exportar_word(
        &documento("analise-impacto"),
        &OpcoesExportacao::default(),
        Some(&codigo()),
    )
    .unwrap();
    let partes = partes(&docx);
    let doc = &partes["word/document.xml"];
    let corpo = texto(doc);

    // Título nas propriedades do documento.
    assert!(
        partes["docProps/core.xml"]
            .contains("<dc:title>A-0002 · Parcelamento da fatura para pessoa jurídica</dc:title>")
    );

    // Seções na ordem, como títulos.
    let secoes = [
        "Demanda como veio",
        "Demanda como entendida",
        "Como é hoje",
        "Funcionalidades",
        "Dados",
        "Integrações",
        "Documentos e telas",
        "Riscos",
        "Abrangência",
        "Perguntas para o solicitante",
        "Sugestões de alteração",
        "Evidências",
        "Apêndice: trechos citados",
    ];
    let mut pos = 0;
    for s in secoes {
        let alvo = format!(
            "<w:p><w:pPr><w:pStyle w:val=\"Heading2\"/></w:pPr><w:r><w:t xml:space=\"preserve\">{s}</w:t></w:r></w:p>"
        );
        let achado = doc[pos..]
            .find(&alvo)
            .unwrap_or_else(|| panic!("falta a seção {s}"));
        pos += achado + alvo.len();
    }

    // Selo de confiança em negrito.
    for selo in ["[confirmado]", "[inferido]", "[a confirmar]"] {
        assert!(
            doc.contains(&format!(
                "<w:r><w:rPr><w:b/></w:rPr><w:t xml:space=\"preserve\">{selo}</w:t></w:r>"
            )),
            "falta o selo {selo}"
        );
    }

    // 10 referências a 6 evidências: 10 notas do Word.
    assert_eq!(doc.matches("<w:footnoteReference ").count(), 10);
    let notas = texto(&partes["word/footnotes.xml"]);
    assert!(notas.contains(
        "Evidência [3]: backend/src/main/java/exemplo/faturamento/ParcelamentoService.java:34-35"
    ));
    assert_eq!(notas.matches("Evidência [3]:").count(), 3);

    // Lista de evidências e apêndice com o código.
    assert!(corpo.contains("[6] frontend/src/telas/parcelamento-form.ts:19\n"));
    assert!(corpo.contains("[5] db/02_parametros.sql:15-16\n"));
    assert_eq!(doc.matches("<w:pStyle w:val=\"Codigo\"/>").count(), 6);

    // Perguntas numeradas, a lista de "Como é hoje" com marcador.
    let numeracao = &partes["word/numbering.xml"];
    assert!(numeracao.contains("<w:numFmt w:val=\"decimal\"/>"));
    assert!(numeracao.contains("<w:numFmt w:val=\"bullet\"/>"));
    assert!(corpo.contains("O limite de 3 parcelas vale também para órgão público?\n"));
    assert!(corpo.contains(
        "A tela deve deixar de oferecer até 12 parcelas? Hoje o backend recusa acima de 6.\n"
    ));

    // Link para a sugestão vira texto.
    assert!(corpo.contains("S-0001 · Limitar a tela ao parâmetro MAX_PARCELAS (frontend/src/telas/parcelamento-form.ts)"));

    // Parágrafos: título do documento e identificação (2), 13 seções, 1 + 1 de
    // demanda, 2 + 1 + 1 + 1 + 1 itens, 1 vazio (integrações), 2 de
    // abrangência, 2 perguntas, 1 sugestão, 6 evidências e 6 trechos com o seu
    // título: 2 + 13 + 2 + 6 + 1 + 2 + 2 + 1 + 6 + 12 = 47.
    assert_eq!(paragrafos(doc), 47);
}

#[test]
fn docx_com_tabela_citacao_e_codigo() {
    let md = "---\ntipo: termo\n---\n\n# Tabela\n\n> Citação com [link](https://exemplo.com).\n\n| Campo | Valor |\n|:--|--:|\n| a | **1** |\n| b |\n\n```\nx\ty\n```\n\n1. um\n   - sub\n2. dois\n\n3. três\n\n---\n";
    let partes = partes(&exportar_word(md, &OpcoesExportacao::default(), None).unwrap());
    let doc = &partes["word/document.xml"];
    let corpo = texto(doc);
    assert_eq!(doc.matches("<w:tr>").count(), 3);
    assert_eq!(doc.matches("<w:tc>").count(), 6, "linha curta completada");
    assert!(doc.contains("<w:tblHeader/>"));
    assert!(doc.contains("<w:jc w:val=\"right\"/>"));
    assert!(doc.contains("<w:pStyle w:val=\"Citacao\"/>"));
    assert!(corpo.contains("Citação com link (https://exemplo.com)."));
    assert!(doc.contains(
        "<w:t xml:space=\"preserve\">x</w:t><w:tab/><w:t xml:space=\"preserve\">y</w:t>"
    ));
    assert!(doc.contains("<w:ilvl w:val=\"1\"/>"));
    assert!(doc.contains("<w:pBdr><w:bottom "));
    // Depois da tabela vem outro bloco; o documento termina num parágrafo.
    assert!(doc.ends_with("</w:p><w:sectPr><w:pgSz w:w=\"11906\" w:h=\"16838\"/><w:pgMar w:top=\"1417\" w:right=\"1701\" w:bottom=\"1417\" w:left=\"1701\" w:header=\"708\" w:footer=\"708\" w:gutter=\"0\"/></w:sectPr></w:body></w:document>"));
}

#[test]
fn docx_e_deterministico() {
    let d = documento("funcionalidade");
    let a = exportar_word(&d, &OpcoesExportacao::default(), Some(&codigo())).unwrap();
    let b = exportar_word(&d, &OpcoesExportacao::default(), Some(&codigo())).unwrap();
    assert_eq!(a, b);
}

#[test]
fn grava_nos_dois_formatos() {
    let pasta = tempfile::tempdir().unwrap();
    let d = documento("sugestao");
    for formato in [Formato::Markdown, Formato::Word] {
        let destino = pasta.path().join(format!("S-0001.{}", formato.extensao()));
        gravar_exportacao(
            &destino,
            &d,
            formato,
            &OpcoesExportacao::default(),
            Some(&codigo()),
        )
        .unwrap();
        assert_eq!(
            fs::read(&destino).unwrap(),
            exportar(&d, formato, &OpcoesExportacao::default(), Some(&codigo())).unwrap()
        );
    }
    let opcoes: OpcoesExportacao = serde_norway::from_str("perguntas: false").unwrap();
    assert!(opcoes.evidencias && opcoes.trechos && !opcoes.perguntas);
}

/// Com `CBA_EXPORTAR_EM=<pasta>`, grava a exportação de cada documento nos dois
/// formatos, para abrir no Word e no LibreOffice (critério de aceite da #10).
#[test]
fn exporta_para_conferencia_manual() {
    let Some(pasta) = std::env::var_os("CBA_EXPORTAR_EM") else {
        return;
    };
    let pasta = PathBuf::from(pasta);
    for nome in DOCUMENTOS {
        for formato in [Formato::Markdown, Formato::Word] {
            gravar_exportacao(
                &pasta.join(format!("{nome}.{}", formato.extensao())),
                &documento(nome),
                formato,
                &OpcoesExportacao::default(),
                Some(&codigo()),
            )
            .unwrap();
        }
    }
}
