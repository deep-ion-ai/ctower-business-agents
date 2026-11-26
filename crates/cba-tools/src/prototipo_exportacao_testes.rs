//! Testes da exportação do protótipo em pasta HTML (#380) com o sistema
//! fictício "Faturamento Exemplo": arquivos gerados, links relativos válidos
//! entre o índice e as telas, nada de script nem de recurso de fora, e a pasta
//! existente só substituída com confirmação.

use std::collections::BTreeSet;
use std::path::Path;

use cba_core::prototipo::{Fidelidade, MudancaTela, NovaTela, NovoElemento, Rastro, TipoElemento};
use cba_core::prototipo_html::{self, Visual};
use cba_core::requisito::Fundamento;
use cba_kb::{Id, Serie};

use super::*;

fn projeto(tmp: &tempfile::TempDir) -> cba_kb::Projeto {
    let base = cba_kb::Base::criar(tmp.path().join("base")).unwrap();
    let doc = cba_kb::Documento::novo(
        cba_kb::frontmatter::FrontmatterProjeto {
            tipo: cba_kb::TipoDocumento::Projeto,
            id: "faturamento-exemplo".into(),
            nome: "Faturamento Exemplo".into(),
            areas: Vec::new(),
            criado: None,
        },
        "# Faturamento Exemplo\n",
    );
    base.criar_projeto(&doc).unwrap()
}

fn regra() -> Rastro {
    Rastro {
        fundamentos: vec![Fundamento::Regra {
            id: Id::analisar("RN-FAT-007").unwrap(),
            titulo: "Limite de parcelas".into(),
            evidencias: Vec::new(),
        }],
        ..Rastro::default()
    }
}

fn tela(
    nome: &str,
    mudanca: MudancaTela,
    rastro: Rastro,
    elementos: Vec<NovoElemento>,
) -> NovaTela {
    NovaTela {
        nome: nome.into(),
        mudanca,
        motivo: format!("{nome} entra no parcelamento de faturas"),
        rastro,
        elementos,
        sessao: None,
    }
}

fn elemento(nome: &str, rastro: Rastro) -> NovoElemento {
    NovoElemento {
        tipo: TipoElemento::Campo,
        nome: nome.into(),
        descricao: None,
        rastro,
        sessao: None,
    }
}

/// Protótipo com três telas: TL-1 e TL-3 com HTML, TL-2 ainda sem.
fn analise(kb: &cba_kb::Projeto) -> Analise {
    let mut a = Analise::nova(
        Serie::Analise.id(3),
        "faturamento-exemplo",
        Fluxo::Prototipo,
        "Protótipo do parcelamento",
        "Protótipo de telas da análise A-0001",
        1,
    );
    a.origem = Some(Serie::Analise.id(1));
    a.registrar_tela(
        tela(
            "Parcelar fatura <PJ>",
            MudancaTela::Alterada,
            regra(),
            vec![
                elemento("Número de parcelas", regra()),
                elemento("Valor mínimo da parcela", Rastro::default()),
            ],
        ),
        2,
    )
    .unwrap();
    a.registrar_tela(
        tela(
            "Consultar parcelas",
            MudancaTela::Nova,
            Rastro::default(),
            Vec::new(),
        ),
        3,
    )
    .unwrap();
    a.registrar_tela(
        tela(
            "Confirmar parcelamento",
            MudancaTela::Nova,
            regra(),
            vec![elemento("Resumo das parcelas", regra())],
        ),
        4,
    )
    .unwrap();
    for numero in [1, 3] {
        let t = a.tela(numero).unwrap().clone();
        let marcas: String = t
            .elementos
            .iter()
            .map(|e| format!("<p data-elemento=\"{}\">{}</p>", e.rotulo(), e.nome))
            .collect();
        let corpo = format!(
            "<section data-estado=\"vazio\">{marcas}</section>\
             <section data-estado=\"preenchido\"><p>Fatura 2024-000123</p></section>\
             <section data-estado=\"erro\"><p>Parcelas acima do limite</p></section>\
             <section data-estado=\"sucesso\"><p>Parcelamento registrado</p></section>"
        );
        let conferido = prototipo_html::conferir_corpo(&corpo, &t).unwrap();
        let css = TokensProjeto::css_aprovado(kb).unwrap().unwrap();
        let html = prototipo_html::montar(
            &t,
            &conferido,
            &Visual::Tokens(css),
            Fidelidade::AltaFidelidade,
        );
        let caminho = cba_kb::layout::html_da_tela(&a.id, numero);
        cba_kb::arquivo::gravar(&kb.caminho(&caminho), html.as_bytes()).unwrap();
        a.definir_html_da_tela(numero, Some(caminho)).unwrap();
    }
    a
}

fn aprovar_tokens(kb: &cba_kb::Projeto) {
    TokensProjeto::gravar_extracao(kb, Vec::new()).unwrap();
    TokensProjeto::alterar(kb, |t, agora| {
        t.aprovar(agora);
        Ok(())
    })
    .unwrap();
}

/// Valores de `atributo="…"` no HTML.
fn valores(html: &str, atributo: &str) -> Vec<String> {
    let marca = format!("{atributo}=\"");
    html.match_indices(&marca)
        .map(|(i, _)| {
            let resto = &html[i + marca.len()..];
            resto[..resto.find('"').unwrap()].to_owned()
        })
        .collect()
}

/// Todo link de `arquivo` aponta para um arquivo da pasta e, com fragmento,
/// para um `id` que existe nele.
fn conferir_links(pasta: &Path, arquivo: &str) -> Vec<String> {
    let html = std::fs::read_to_string(pasta.join(arquivo)).unwrap();
    let mut links = Vec::new();
    for href in valores(&html, "href") {
        let (alvo, fragmento) = href.split_once('#').unwrap_or((href.as_str(), ""));
        let alvo = if alvo.is_empty() { arquivo } else { alvo };
        assert!(
            !alvo.contains('/') && !alvo.contains(':'),
            "{arquivo}: link não relativo à pasta: {href}"
        );
        let destino = pasta.join(alvo);
        assert!(destino.is_file(), "{arquivo}: link quebrado: {href}");
        if !fragmento.is_empty() {
            let ids = valores(&std::fs::read_to_string(&destino).unwrap(), "id");
            assert!(
                ids.iter().any(|i| i == fragmento),
                "{arquivo}: {href} sem o id {fragmento}"
            );
        }
        links.push(href);
    }
    links
}

#[test]
fn exporta_a_pasta_com_indice_telas_e_tokens() {
    let tmp = tempfile::tempdir().unwrap();
    let kb = projeto(&tmp);
    aprovar_tokens(&kb);
    let a = analise(&kb);
    let destino = tmp.path().join("destino");
    std::fs::create_dir_all(&destino).unwrap();

    let feita = exportar_prototipo(&kb, &a, &destino, false).unwrap();
    assert_eq!(nome_da_pasta(&a), "A-0003-prototipo-do-parcelamento");
    assert_eq!(
        feita.pasta,
        destino.join("A-0003-prototipo-do-parcelamento")
    );
    assert_eq!((feita.telas, feita.sem_prototipo), (2, 1));

    let arquivos: BTreeSet<String> = std::fs::read_dir(&feita.pasta)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        arquivos,
        ["TL-1.html", "TL-3.html", "index.html", "tokens.css"]
            .map(String::from)
            .into()
    );

    // Índice: análise, origem, telas, "a confirmar" e links para telas e estados.
    let indice = std::fs::read_to_string(feita.pasta.join(INDICE)).unwrap();
    assert!(
        indice.contains("A-0003 · Protótipo do parcelamento"),
        "{indice}"
    );
    assert!(indice.contains("Análise de origem: A-0001"), "{indice}");
    assert!(indice.contains("Parcelar fatura &lt;PJ&gt;"), "{indice}");
    assert!(indice.contains("<td>alterada</td>"), "{indice}");
    assert!(
        indice.contains("<td>Consultar parcelas</td><td>nova</td>"),
        "{indice}"
    );
    assert!(indice.contains("sem protótipo ainda"), "{indice}");
    // TL-1: um elemento sem fundamento; TL-2: o motivo sem fundamento.
    assert!(
        indice.contains("2 itens a confirmar com o negócio"),
        "{indice}"
    );
    let links = conferir_links(&feita.pasta, INDICE);
    for esperado in [
        "TL-1.html",
        "TL-1.html#estado-vazio",
        "TL-1.html#estado-preenchido",
        "TL-1.html#estado-erro",
        "TL-3.html#estado-sucesso",
        "tokens.css",
    ] {
        assert!(links.iter().any(|l| l == esperado), "{esperado}: {links:?}");
    }
    assert!(!links.iter().any(|l| l.starts_with("TL-2")), "{links:?}");

    // Telas: navegação para o índice, a anterior e a próxima (pulando a TL-2).
    let tl1 = conferir_links(&feita.pasta, "TL-1.html");
    assert!(tl1.contains(&"index.html".to_owned()), "{tl1:?}");
    assert!(tl1.contains(&"TL-3.html".to_owned()), "{tl1:?}");
    let tl3 = conferir_links(&feita.pasta, "TL-3.html");
    assert!(tl3.contains(&"index.html".to_owned()), "{tl3:?}");
    assert!(tl3.contains(&"TL-1.html".to_owned()), "{tl3:?}");
    let html_tl1 = std::fs::read_to_string(feita.pasta.join("TL-1.html")).unwrap();
    assert!(
        html_tl1.contains("Próxima: TL-3 · Confirmar parcelamento"),
        "{html_tl1}"
    );
    assert!(!html_tl1.contains("Anterior:"), "{html_tl1}");
    assert!(html_tl1.contains("Content-Security-Policy"), "{html_tl1}");
    let nav = html_tl1.find(CLASSE_NAVEGACAO).unwrap();
    assert!(html_tl1.find("<body").unwrap() < nav, "{html_tl1}");

    // Sem script e sem nada de fora, em nenhum arquivo.
    for nome in &arquivos {
        let texto = std::fs::read_to_string(feita.pasta.join(nome))
            .unwrap()
            .to_ascii_lowercase();
        for proibido in [
            "<script", "http:", "https:", "\"//", "'//", "(//", "@import",
        ] {
            assert!(!texto.contains(proibido), "{nome} contém {proibido}");
        }
    }
    let tokens = std::fs::read_to_string(feita.pasta.join(TOKENS)).unwrap();
    assert!(tokens.contains(":root {"), "{tokens}");
}

#[test]
fn pasta_existente_so_com_confirmacao() {
    let tmp = tempfile::tempdir().unwrap();
    let kb = projeto(&tmp);
    aprovar_tokens(&kb);
    let a = analise(&kb);
    let destino = tmp.path().join("destino");
    let pasta = destino.join(nome_da_pasta(&a));
    std::fs::create_dir_all(&pasta).unwrap();
    std::fs::write(pasta.join("notas.txt"), "do analista").unwrap();

    match exportar_prototipo(&kb, &a, &destino, false) {
        Err(ErroExportacaoPrototipo::PastaExiste(p)) => assert_eq!(p, pasta),
        outro => panic!("esperava PastaExiste: {outro:?}"),
    }
    assert!(!pasta.join(INDICE).exists());

    let feita = exportar_prototipo(&kb, &a, &destino, true).unwrap();
    assert!(feita.pasta.join(INDICE).is_file());
    // O que não é da exportação fica.
    assert!(pasta.join("notas.txt").is_file());
}

#[test]
fn sem_tokens_aprovados_e_sem_telas() {
    let tmp = tempfile::tempdir().unwrap();
    let kb = projeto(&tmp);
    aprovar_tokens(&kb);
    let a = analise(&kb);
    // Tokens voltam a não existir: o índice avisa e o tokens.css não vai.
    std::fs::remove_file(kb.caminho(&cba_kb::layout::estado_dos_tokens())).unwrap();
    let destino = tmp.path().join("destino");
    let feita = exportar_prototipo(&kb, &a, &destino, false).unwrap();
    assert!(!feita.pasta.join(TOKENS).exists());
    let indice = std::fs::read_to_string(feita.pasta.join(INDICE)).unwrap();
    assert!(indice.contains("wireframe neutro"), "{indice}");

    let mut vazia = a.clone();
    vazia.prototipo = None;
    let e = exportar_prototipo(&kb, &vazia, &destino, true).unwrap_err();
    assert!(e.to_string().contains("ainda não tem telas"), "{e}");
}
