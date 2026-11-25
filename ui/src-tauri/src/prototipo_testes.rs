//! Testes do visualizador do protótipo (#382) com o sistema fictício
//! "Faturamento Exemplo": o HTML da tela é lido pelo número, `None` antes de
//! gravado, e o caminho que sai da pasta do protótipo da análise é recusado.

use cba_core::eventos::Mudanca;
use cba_core::prototipo::{MudancaTela, NovaTela, PedidoPrototipo, Rastro};
use cba_kb::{CaminhoRelativo, layout};

use super::*;
use crate::teste_base::{PROJETO, cenario};

#[test]
fn le_o_html_da_tela_e_recusa_caminho_fora_da_pasta() {
    let c = cenario();
    c.aprovar_analise_da_multa();
    let criada = crate::analise::criar_analise_de_prototipo(
        &c.dados,
        PROJETO,
        "A-0001",
        None,
        PedidoPrototipo::default(),
        None,
        None,
    )
    .unwrap();
    let id = cba_kb::Id::analisar(&criada.analise.id).unwrap();
    let amb = c.ambiente();
    for nome in ["Cálculo da multa", "Consulta de faturas em atraso"] {
        amb.analises
            .alterar(&id, Mudanca::Tela, |a, agora| {
                a.registrar_tela(
                    NovaTela {
                        nome: nome.into(),
                        mudanca: MudancaTela::Alterada,
                        motivo: "A multa por atraso aparece na fatura".into(),
                        rastro: Rastro::default(),
                        elementos: Vec::new(),
                        sessao: None,
                    },
                    agora,
                )
            })
            .unwrap();
    }

    // Sem HTML gravado: nada a mostrar.
    assert_eq!(
        html_da_tela(&c.dados, PROJETO, &id.to_string(), 1),
        Ok(None)
    );

    // HTML gravado na pasta do protótipo: lido pelo número da tela.
    let caminho = layout::html_da_tela(&id, 1);
    let arquivo = amb.kb.caminho(&caminho);
    std::fs::create_dir_all(arquivo.parent().unwrap()).unwrap();
    std::fs::write(
        &arquivo,
        "<!doctype html><title>TL-1 · Cálculo da multa</title>",
    )
    .unwrap();
    amb.analises
        .alterar(&id, Mudanca::Tela, |a, _| {
            a.definir_html_da_tela(1, Some(caminho))
        })
        .unwrap();
    let html = html_da_tela(&c.dados, PROJETO, &id.to_string(), 1)
        .unwrap()
        .unwrap();
    assert!(html.contains("TL-1 · Cálculo da multa"), "{html}");

    // Tela que não existe.
    let erro = html_da_tela(&c.dados, PROJETO, &id.to_string(), 9).unwrap_err();
    assert!(erro.contains("TL-9"), "{erro}");

    // Caminho fora da pasta do protótipo da análise: recusado, mesmo existindo.
    let fora = CaminhoRelativo::novo("analises/A-0001/prototipo/TL-1.html").unwrap();
    let arquivo_fora = amb.kb.caminho(&fora);
    std::fs::create_dir_all(arquivo_fora.parent().unwrap()).unwrap();
    std::fs::write(&arquivo_fora, "<p>outra análise</p>").unwrap();
    amb.analises
        .alterar(&id, Mudanca::Tela, |a, _| {
            a.definir_html_da_tela(2, Some(fora))
        })
        .unwrap();
    let erro = html_da_tela(&c.dados, PROJETO, &id.to_string(), 2).unwrap_err();
    assert!(erro.contains("fora da pasta do protótipo"), "{erro}");

    // Arquivo que não é HTML, ainda que na pasta.
    let css = CaminhoRelativo::novo(&format!("analises/{id}/prototipo/TL-2.css")).unwrap();
    std::fs::write(amb.kb.caminho(&css), "body {}").unwrap();
    amb.analises
        .alterar(&id, Mudanca::Tela, |a, _| {
            a.definir_html_da_tela(2, Some(css))
        })
        .unwrap();
    let erro = html_da_tela(&c.dados, PROJETO, &id.to_string(), 2).unwrap_err();
    assert!(erro.contains("fora da pasta do protótipo"), "{erro}");

    // Link simbólico na pasta que aponta para fora dela.
    #[cfg(unix)]
    {
        let link = layout::html_da_tela(&id, 2);
        std::os::unix::fs::symlink(&arquivo_fora, amb.kb.caminho(&link)).unwrap();
        amb.analises
            .alterar(&id, Mudanca::Tela, |a, _| {
                a.definir_html_da_tela(2, Some(link))
            })
            .unwrap();
        let erro = html_da_tela(&c.dados, PROJETO, &id.to_string(), 2).unwrap_err();
        assert!(erro.contains("fora da pasta do protótipo"), "{erro}");
    }

    // Análise que não é de protótipo.
    let erro = html_da_tela(&c.dados, PROJETO, "A-0001", 1).unwrap_err();
    assert!(erro.contains("prototipo"), "{erro}");
}

#[test]
fn exporta_a_pasta_e_pede_confirmacao_se_ja_existe() {
    let c = cenario();
    c.aprovar_analise_da_multa();
    let criada = crate::analise::criar_analise_de_prototipo(
        &c.dados,
        PROJETO,
        "A-0001",
        None,
        PedidoPrototipo::default(),
        None,
        None,
    )
    .unwrap();
    let id = cba_kb::Id::analisar(&criada.analise.id).unwrap();
    let amb = c.ambiente();
    for nome in ["Cálculo da multa", "Consulta de faturas em atraso"] {
        amb.analises
            .alterar(&id, Mudanca::Tela, |a, agora| {
                a.registrar_tela(
                    NovaTela {
                        nome: nome.into(),
                        mudanca: MudancaTela::Alterada,
                        motivo: "A multa por atraso aparece na fatura".into(),
                        rastro: Rastro::default(),
                        elementos: Vec::new(),
                        sessao: None,
                    },
                    agora,
                )
            })
            .unwrap();
    }
    let caminho = layout::html_da_tela(&id, 1);
    let arquivo = amb.kb.caminho(&caminho);
    std::fs::create_dir_all(arquivo.parent().unwrap()).unwrap();
    std::fs::write(
        &arquivo,
        "<!doctype html><html><head><title>TL-1</title></head><body><p>Multa</p></body></html>",
    )
    .unwrap();
    amb.analises
        .alterar(&id, Mudanca::Tela, |a, _| {
            a.definir_html_da_tela(1, Some(caminho))
        })
        .unwrap();

    let destino = c.dados.raiz().join("exportados");
    let destino = destino.to_str().unwrap();
    let feita = exportar(&c.dados, PROJETO, &id.to_string(), destino, false).unwrap();
    let ExportacaoPrototipo::Exportada {
        pasta,
        telas,
        sem_prototipo,
    } = feita.clone()
    else {
        panic!("esperava exportada: {feita:?}");
    };
    assert_eq!((telas, sem_prototipo), (1, 1));
    let pasta = std::path::PathBuf::from(pasta);
    assert!(pasta.join("index.html").is_file());
    let tl1 = std::fs::read_to_string(pasta.join("TL-1.html")).unwrap();
    assert!(tl1.contains("<a href=\"index.html\">"), "{tl1}");

    // A pasta já existe: nada é gravado sem confirmar.
    let de_novo = exportar(&c.dados, PROJETO, &id.to_string(), destino, false).unwrap();
    assert_eq!(
        de_novo,
        ExportacaoPrototipo::PastaExiste {
            pasta: pasta.display().to_string()
        }
    );
    let confirmada = exportar(&c.dados, PROJETO, &id.to_string(), destino, true).unwrap();
    assert!(
        matches!(confirmada, ExportacaoPrototipo::Exportada { .. }),
        "{confirmada:?}"
    );

    let erro = exportar(&c.dados, PROJETO, &id.to_string(), " ", false).unwrap_err();
    assert!(erro.contains("pasta de destino"), "{erro}");
}
