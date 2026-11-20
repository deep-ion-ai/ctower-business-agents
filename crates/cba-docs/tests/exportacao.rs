//! Exportação dos documentos esperados da montagem (`tests/esperados/`), com o
//! código do sistema fictício "Faturamento Exemplo" (`fixtures/`) como fonte dos
//! trechos citados.
//!
//! O Markdown exportado é comparado com `tests/esperados/exportados/`. O `.docx`
//! é conferido abrindo o zip e lendo o XML, sem depender do Word. O HTML da
//! análise de impacto e da ata também é comparado com o texto esperado
//! (`exportados/*.html`).
//!
//! Para atualizar os textos esperados: `CBA_ATUALIZAR_ESPERADOS=1 cargo test -p
//! cba-docs`, e revise o diff.

use std::collections::BTreeMap;
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use cba_docs::{
    ArquivoHtml, AvisoHtml, CodigoEmPasta, ErroFigura, Figura, Figuras, FonteDeTrechos, Formato,
    LinksDeEvidencia, ModeloHtml, OpcoesExportacao, Origem, blocos_mermaid, exportar,
    exportar_com_links, exportar_html, exportar_markdown, exportar_word, gravar_exportacao,
};
use cba_kb::Base;
use quick_xml::Reader;
use quick_xml::events::Event;

const DOCUMENTOS: [&str; 13] = [
    "ata",
    "analise-impacto",
    "analise-impacto-vazia",
    "caso-de-uso",
    "cenarios-teste",
    "funcionalidade",
    "funcionalidade-de-analise",
    "funcionalidade-com-diagrama",
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
        ..OpcoesExportacao::default()
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
        let nome = arquivo.name().unwrap().into_owned();
        if nome.starts_with("word/media/") {
            let mut bytes = Vec::new();
            arquivo.read_to_end(&mut bytes).unwrap();
            partes.insert(nome, format!("{} bytes", bytes.len()));
            continue;
        }
        let mut texto = String::new();
        arquivo.read_to_string(&mut texto).unwrap();
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
        "Dados pessoais e requisitos não funcionais",
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

    // 13 referências a 6 evidências: 13 notas do Word.
    assert_eq!(doc.matches("<w:footnoteReference ").count(), 13);
    let notas = texto(&partes["word/footnotes.xml"]);
    assert!(notas.contains(
        "Evidência [3]: backend/src/main/java/exemplo/faturamento/ParcelamentoService.java:34-35"
    ));
    assert_eq!(notas.matches("Evidência [3]:").count(), 4);

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

    // Parágrafos: título do documento e identificação (2), 14 seções, 1 + 1 de
    // demanda, 2 + 1 + 1 + 1 + 1 + 1 itens, 1 vazio (integrações), 2 de
    // abrangência, 2 perguntas, 1 sugestão, 6 evidências e 6 trechos com o seu
    // título: 2 + 14 + 2 + 7 + 1 + 2 + 2 + 1 + 6 + 12 = 49.
    assert_eq!(paragrafos(doc), 49);
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
    for formato in [Formato::Markdown, Formato::Word, Formato::Html] {
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

/// Com `CBA_EXPORTAR_EM=<pasta>`, grava a exportação de cada documento nos três
/// formatos, para abrir no Word e no LibreOffice (critério de aceite da #10).
#[test]
fn exporta_para_conferencia_manual() {
    let Some(pasta) = std::env::var_os("CBA_EXPORTAR_EM") else {
        return;
    };
    let pasta = PathBuf::from(pasta);
    for nome in DOCUMENTOS {
        for formato in [Formato::Markdown, Formato::Word, Formato::Html] {
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

// ---------------------------------------------------------------------------
// Diagramas (#118)

/// PNG mínimo (assinatura e IHDR) com o tamanho dado.
fn png(largura: u32, altura: u32) -> Vec<u8> {
    let mut p = b"\x89PNG\r\n\x1a\n".to_vec();
    p.extend_from_slice(&13u32.to_be_bytes());
    p.extend_from_slice(b"IHDR");
    p.extend_from_slice(&largura.to_be_bytes());
    p.extend_from_slice(&altura.to_be_bytes());
    p.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
    p
}

fn word_com(documento: &str, figuras: &Figuras, opcoes: &OpcoesExportacao) -> Vec<u8> {
    exportar_com_links(documento, Formato::Word, opcoes, None, None, figuras).unwrap()
}

#[test]
fn markdown_mantem_o_bloco_mermaid() {
    let d = documento("funcionalidade-com-diagrama");
    let md = exportar_markdown(&d, &OpcoesExportacao::default(), None);
    assert!(md.contains("```mermaid\nflowchart LR\n"), "{md}");
    assert!(!md.contains("Apêndice: código dos diagramas"));
    // Exportar Markdown com figuras não muda nada.
    let mut figuras = Figuras::novas();
    for b in blocos_mermaid(&d) {
        figuras
            .inserir(&b, Figura::nova(png(10, 10), 0, 0).unwrap())
            .unwrap();
    }
    let com = exportar_com_links(
        &d,
        Formato::Markdown,
        &OpcoesExportacao::default(),
        None,
        None,
        &figuras,
    )
    .unwrap();
    assert_eq!(String::from_utf8(com).unwrap(), md);
}

#[test]
fn docx_desenha_o_diagrama_como_figura() {
    let d = documento("funcionalidade-com-diagrama");
    let blocos = blocos_mermaid(&d);
    assert_eq!(blocos.len(), 1);
    assert!(blocos[0].starts_with("flowchart LR\n"));
    let mut figuras = Figuras::novas();
    // 1200 px de largura: maior que a página, reduzida para a largura útil.
    figuras
        .inserir(&blocos[0], Figura::nova(png(2400, 600), 1200, 300).unwrap())
        .unwrap();
    let partes = partes(&word_com(&d, &figuras, &OpcoesExportacao::default()));
    assert_eq!(
        partes["word/media/figura1.png"],
        format!("{} bytes", png(1, 1).len())
    );
    assert!(partes["[Content_Types].xml"].contains("Extension=\"png\""));
    assert!(partes["word/_rels/document.xml.rels"].contains(
        "Id=\"rId101\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/image\" Target=\"media/figura1.png\""
    ));
    let doc = &partes["word/document.xml"];
    assert!(doc.contains("<a:blip r:embed=\"rId101\"/>"), "{doc}");
    // Largura útil: 8504 twips = 5400040 EMU; altura na mesma proporção (1/4).
    assert!(
        doc.contains("<wp:extent cx=\"5400040\" cy=\"1350010\"/>"),
        "{doc}"
    );
    let corpo = texto(doc);
    // O código não aparece no corpo, só no apêndice; a legenda e a tabela ficam.
    assert!(corpo.contains("Figura 1: Ciclo de vida da fatura"));
    assert!(corpo.contains("Aberta → Cancelada"));
    assert!(!corpo.contains("Diagrama não renderizado"));
    let apendice = corpo
        .find("Apêndice: código dos diagramas")
        .expect("apêndice");
    assert!(corpo[apendice..].contains("flowchart LR"));
    assert_eq!(corpo.matches("flowchart LR").count(), 1);

    // Sem a opção, sem o apêndice.
    let sem = OpcoesExportacao {
        codigo_diagramas: false,
        ..OpcoesExportacao::default()
    };
    let corpo = texto(&partes_de(&word_com(&d, &figuras, &sem)));
    assert!(!corpo.contains("Apêndice: código dos diagramas"));
    assert!(!corpo.contains("flowchart LR"));
}

fn partes_de(docx: &[u8]) -> String {
    partes(docx).remove("word/document.xml").unwrap()
}

#[test]
fn docx_sem_figura_mostra_aviso_e_codigo() {
    // Exportação sem o app (ou com falha ao desenhar): o diagrama vira aviso e
    // código, e o resto do documento sai igual.
    let d = documento("funcionalidade-com-diagrama");
    let docx = exportar_word(&d, &OpcoesExportacao::default(), None).unwrap();
    let partes = partes(&docx);
    assert!(!partes.keys().any(|k| k.starts_with("word/media/")));
    let doc = &partes["word/document.xml"];
    assert!(doc.contains("<w:pStyle w:val=\"Aviso\"/>"));
    let corpo = texto(doc);
    assert!(corpo.contains("Diagrama não renderizado."), "{corpo}");
    assert!(corpo.contains("flowchart LR"));
    assert!(corpo.contains("Figura 1: Ciclo de vida da fatura"));
    assert!(!corpo.contains("Apêndice: código dos diagramas"));
}

#[test]
fn mermaid_invalido_nao_quebra_a_exportacao() {
    // Bloco editado à mão, que o app não conseguiu desenhar: sem figura para ele.
    // O outro bloco, válido, sai como figura.
    let md = "# Doc\n\nAntes.\n\n```mermaid\nflowchart LR\n  a -->\n```\n\n```mermaid\nflowchart TD\n  x --> y\n```\n\nDepois.\n";
    let mut figuras = Figuras::novas();
    figuras
        .inserir(
            "flowchart TD\n  x --> y",
            Figura::nova(png(100, 50), 0, 0).unwrap(),
        )
        .unwrap();
    let partes = partes(&word_com(md, &figuras, &OpcoesExportacao::default()));
    let corpo = texto(&partes["word/document.xml"]);
    assert!(corpo.contains("Antes."));
    assert!(corpo.contains("Depois."));
    assert_eq!(corpo.matches("Diagrama não renderizado.").count(), 1);
    assert!(corpo.contains("a -->"));
    assert!(partes.contains_key("word/media/figura1.png"));
    assert!(!partes.contains_key("word/media/figura2.png"));
    // O apêndice traz só o diagrama desenhado, com o número dele no documento.
    let apendice = &corpo[corpo.find("Apêndice: código dos diagramas").unwrap()..];
    assert!(apendice.contains("Diagrama 2"));
    assert!(!apendice.contains("Diagrama 1"));
}

#[test]
fn figura_que_nao_e_png_e_recusada() {
    assert_eq!(
        Figura::nova(
            b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>".to_vec(),
            10,
            10
        ),
        Err(ErroFigura::NaoEPng)
    );
}

#[test]
fn docx_com_figura_e_deterministico() {
    let d = documento("funcionalidade-com-diagrama");
    let mut figuras = Figuras::novas();
    figuras
        .inserir(
            &blocos_mermaid(&d)[0],
            Figura::nova(png(300, 100), 0, 0).unwrap(),
        )
        .unwrap();
    let a = word_com(&d, &figuras, &OpcoesExportacao::default());
    let b = word_com(&d, &figuras, &OpcoesExportacao::default());
    assert_eq!(a, b);
}

// ---------------------------------------------------------------------------
// HTML (#166)

const DATA: &str = "2026-11-17";

struct Forja;

impl LinksDeEvidencia for Forja {
    fn link(
        &self,
        arquivo: &str,
        linhas: cba_kb::frontmatter::Linhas,
        commit: Option<&str>,
    ) -> Option<String> {
        Some(format!(
            "https://forja.exemplo/blob/{}/{arquivo}#L{}-L{}",
            commit?, linhas.inicio, linhas.fim
        ))
    }
}

fn html(nome: &str, opcoes: &OpcoesExportacao) -> String {
    let r = exportar_html(
        &documento(nome),
        opcoes,
        Some(&codigo()),
        Some((&Forja, Some("1a2b3c4d"))),
        &Figuras::default(),
        &ModeloHtml::padrao(),
        DATA,
    );
    assert!(r.avisos.is_empty(), "{nome}: {:?}", r.avisos);
    r.html
}

/// Tags do HTML (o que vai de `<` a `>`). Como todo texto do documento sai
/// escapado, um `<` real só abre tag.
fn tags(html: &str) -> Vec<String> {
    html.split('<')
        .skip(1)
        .filter_map(|t| t.split_once('>').map(|(tag, _)| tag.to_ascii_lowercase()))
        .collect()
}

/// Confere que o HTML não tem script, recurso externo nem HTML cru.
fn autocontido(nome: &str, html: &str) {
    let permitidas = [
        "!doctype",
        "html",
        "head",
        "meta",
        "title",
        "style",
        "body",
        "header",
        "main",
        "footer",
        "section",
        "h1",
        "h2",
        "h3",
        "h4",
        "h5",
        "h6",
        "p",
        "a",
        "strong",
        "em",
        "del",
        "sup",
        "sub",
        "code",
        "pre",
        "ul",
        "ol",
        "li",
        "blockquote",
        "table",
        "thead",
        "tbody",
        "tr",
        "th",
        "td",
        "hr",
        "br",
        "span",
        "div",
        "figure",
        "img",
    ];
    for tag in tags(html) {
        let tag = tag.trim_start_matches('/');
        let nome_da_tag = tag.split_whitespace().next().unwrap_or("");
        assert!(permitidas.contains(&nome_da_tag), "{nome}: tag <{tag}>");
        let atributos = &tag[nome_da_tag.len()..];
        for atributo in atributos.split_whitespace() {
            assert!(!atributo.starts_with("on"), "{nome}: evento em <{tag}>");
        }
        if let Some((_, src)) = atributos.split_once("src=\"") {
            assert!(
                src.starts_with("data:image/png;base64,"),
                "{nome}: src externo"
            );
        }
        if let Some((_, href)) = atributos.split_once("href=\"") {
            assert!(
                ["#", "https://", "http://", "mailto:"]
                    .iter()
                    .any(|p| href.starts_with(p)),
                "{nome}: href {href}"
            );
        }
    }
    let css = html.to_ascii_lowercase();
    assert!(
        !css.contains("@import") && !css.contains("url("),
        "{nome}: recurso no CSS"
    );
}

#[test]
fn html_exportado_da_analise_e_da_ata() {
    for nome in ["analise-impacto", "ata"] {
        let h = html(nome, &OpcoesExportacao::default());
        conferir(
            &pasta_do_crate().join(format!("tests/esperados/exportados/{nome}.html")),
            &h,
        );
        autocontido(nome, &h);
        assert!(
            h.starts_with("<!doctype html>\n"),
            "{nome}: comentário do template saiu"
        );
        assert!(!h.contains("{{"), "{nome}");
    }
}

#[test]
fn html_de_cada_documento_e_autocontido() {
    for nome in DOCUMENTOS {
        let h = html(nome, &OpcoesExportacao::default());
        autocontido(nome, &h);
        // Sem frontmatter nem links para a base.
        assert!(!h.contains("\ntipo: "), "{nome}");
        assert!(!h.contains(".md\""), "{nome}");
    }
}

#[test]
fn html_da_analise_tem_cabecalho_selos_notas_e_apendice() {
    let h = html("analise-impacto", &OpcoesExportacao::default());
    assert!(h.contains("<title>A-0002 · Parcelamento da fatura para pessoa jurídica</title>"));
    assert!(h.contains("Análise de impacto · faturamento-exemplo"));
    assert!(h.contains("Status: rascunho · Exportado em 2026-11-17"));
    assert!(h.contains("<strong class=\"selo selo-confirmado\">[confirmado]</strong>"));
    assert!(h.contains("<strong class=\"selo selo-a-confirmar\">[a confirmar]</strong>"));
    // 13 citações, cada uma com id e link para a nota; cada nota volta para
    // todas as suas citações.
    assert_eq!(h.matches("<sup class=\"ref-nota\">").count(), 13);
    for k in 1..=6 {
        assert!(
            h.contains(&format!("<div class=\"nota\" id=\"nota-{k}\">")),
            "nota {k}"
        );
        let citacoes = h
            .matches(&format!("href=\"#nota-{k}\" id=\"ref-{k}-"))
            .count();
        assert!(citacoes >= 1, "nota {k} sem citação");
        for i in 1..=citacoes {
            assert!(h.contains(&format!("id=\"ref-{k}-{i}\"")));
            assert!(
                h.contains(&format!("href=\"#ref-{k}-{i}\"")),
                "volta {k}-{i}"
            );
        }
    }
    // Citação da nota com link para a forja.
    assert!(h.contains(
        "<a href=\"https://forja.exemplo/blob/1a2b3c4d/db/02_parametros.sql#L15-L16\"><code>db/02_parametros.sql:15-16</code></a>"
    ), "{h}");
    // Apêndice numa seção própria, com o código numerado.
    assert!(h.contains("<section class=\"apendice\">\n<h2>Apêndice: trechos citados</h2>"));
    assert!(h.contains("<pre><code class=\"language-sql\">15 | "));
    // Link para a sugestão vira texto.
    assert!(h.contains("S-0001 · Limitar a tela ao parâmetro MAX_PARCELAS"));
    // Estilo de impressão.
    assert!(h.contains("@media print"));
}

#[test]
fn html_respeita_as_opcoes() {
    let sem = OpcoesExportacao {
        evidencias: false,
        perguntas: false,
        ..OpcoesExportacao::default()
    };
    let h = html("analise-impacto", &sem);
    assert!(!h.contains("<sup class=\"ref-nota\""));
    assert!(!h.contains("<div class=\"nota\""));
    assert!(!h.contains("<h2>Evidências</h2>"));
    assert!(!h.contains("<h2>Perguntas"));
    assert!(!h.contains("Apêndice"));
    assert!(h.contains("<h2>Riscos</h2>"));

    let sem_trechos = OpcoesExportacao {
        trechos: false,
        ..OpcoesExportacao::default()
    };
    let h = html("analise-impacto", &sem_trechos);
    assert!(h.contains("<h2>Evidências</h2>"));
    assert!(!h.contains("Apêndice: trechos citados"));
}

#[test]
fn html_escapa_o_html_do_documento() {
    let doc = "---\ntipo: termo\nid: T-1\nprojeto: x<script>\n---\n\n# Termo <script>alert(1)</script>\n\nTexto com <b>negrito</b> e <script>alert('x')</script>.\n\n<script src=\"https://mal.exemplo/x.js\"></script>\n\n<img src=\"https://mal.exemplo/a.png\" onerror=\"alert(1)\">\n\n[clique](javascript:alert(1)) e ![logo](https://mal.exemplo/logo.png)\n";
    let r = exportar_html(
        doc,
        &OpcoesExportacao::default(),
        None,
        None,
        &Figuras::default(),
        &ModeloHtml::padrao(),
        DATA,
    );
    let h = &r.html;
    autocontido("documento com HTML", h);
    assert!(!h.contains("<b>"));
    assert!(h.contains("&lt;b&gt;negrito&lt;/b&gt;"));
    assert!(h.contains("&lt;script&gt;alert(&#39;x&#39;)&lt;/script&gt;"));
    assert!(h.contains("<title>Termo &lt;script&gt;alert(1)&lt;/script&gt;</title>"));
    assert!(h.contains("· x&lt;script&gt;"));
    assert!(!h.contains("javascript:"));
}

#[test]
fn html_com_figura_embutida_e_apendice_dos_diagramas() {
    let d = documento("funcionalidade-com-diagrama");
    let mut figuras = Figuras::novas();
    figuras
        .inserir(
            &blocos_mermaid(&d)[0],
            Figura::nova(png(300, 100), 0, 0).unwrap(),
        )
        .unwrap();
    let r = exportar_html(
        &d,
        &OpcoesExportacao::default(),
        None,
        None,
        &figuras,
        &ModeloHtml::padrao(),
        DATA,
    );
    autocontido("figura", &r.html);
    assert!(r.html.contains(
        "<figure class=\"diagrama\"><img alt=\"Diagrama 1\" width=\"300\" height=\"100\" src=\"data:image/png;base64,iVBORw0KGgo"
    ));
    assert!(r.html.contains("Apêndice: código dos diagramas"));
    // Sem figura: aviso e código.
    let sem = exportar_html(
        &d,
        &OpcoesExportacao::default(),
        None,
        None,
        &Figuras::default(),
        &ModeloHtml::padrao(),
        DATA,
    );
    assert!(
        sem.html
            .contains("<p class=\"aviso\"><strong>Diagrama não renderizado.</strong>")
    );
    assert!(
        sem.html
            .contains("<pre><code class=\"language-mermaid\">flowchart LR")
    );
    assert!(!sem.html.contains("<img"));
}

#[test]
fn html_usa_o_template_e_os_tokens_da_base() {
    let dir = tempfile::tempdir().unwrap();
    let base = Base::criar(dir.path().join("Base")).unwrap();

    // Sem cópia na base: o padrão, sem avisos.
    let (modelo, avisos) = ModeloHtml::da_base(&base);
    assert!(avisos.is_empty());
    assert_eq!(modelo, ModeloHtml::padrao());

    // Com cópia: template e tokens da base.
    base.gravar_texto(
        &ArquivoHtml::Documento.arquivo(),
        "<!-- instrução -->\n<html><head><style>{{css}}</style></head><body><h1>{{titulo}}</h1>{{inventada}}\n{{conteudo}}</body></html>\n",
    )
    .unwrap();
    base.gravar_texto(
        &ArquivoHtml::Tokens.arquivo(),
        ":root { --cor-destaque: #c00; }\n",
    )
    .unwrap();
    let (modelo, avisos) = ModeloHtml::da_base(&base);
    let origem = Origem::Base(ArquivoHtml::Documento.arquivo());
    assert_eq!(
        avisos,
        vec![AvisoHtml::VariavelDesconhecida {
            origem: origem.clone(),
            variavel: "inventada".into(),
            linha: 2,
        }]
    );
    let r = exportar_html(
        &documento("regra"),
        &OpcoesExportacao::default(),
        None,
        None,
        &Figuras::default(),
        &modelo,
        DATA,
    );
    assert!(
        r.html
            .starts_with("<html><head><style>:root { --cor-destaque: #c00; }\n")
    );
    assert!(r.html.contains("{{inventada}}"));
    assert!(!r.html.contains("instrução"));
    assert_eq!(r.avisos.len(), 1);

    // Template sem `{{conteudo}}`: o documento vai para o fim, com aviso.
    base.gravar_texto(
        &ArquivoHtml::Documento.arquivo(),
        "<body><h1>{{titulo}}</h1></body>",
    )
    .unwrap();
    let (modelo, avisos) = ModeloHtml::da_base(&base);
    assert_eq!(avisos, vec![AvisoHtml::ConteudoAusente { origem }]);
    let r = exportar_html(
        &documento("regra"),
        &OpcoesExportacao::default(),
        None,
        None,
        &Figuras::default(),
        &modelo,
        DATA,
    );
    assert!(r.html.contains("<main class=\"documento\">"));
    assert!(r.html.ends_with("</main>\n</body>"));

    // Cópia ilegível: aviso e o padrão.
    fs::write(
        base.caminho(&ArquivoHtml::Tokens.arquivo()),
        [0xff, 0xfe, 0x00],
    )
    .unwrap();
    let (modelo, avisos) = ModeloHtml::da_base(&base);
    assert!(
        avisos
            .iter()
            .any(|a| matches!(a, AvisoHtml::Ilegivel { .. })),
        "{avisos:?}"
    );
    assert_eq!(
        modelo.texto(ArquivoHtml::Tokens),
        ArquivoHtml::Tokens.padrao()
    );
}
