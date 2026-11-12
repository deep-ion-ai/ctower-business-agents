//! Testes da busca, da abertura de documento e da vigia da base (#38), com o
//! sistema fictício "Faturamento Exemplo".

use std::sync::Mutex;
use std::time::Duration;

use super::*;
use crate::teste_base::{SISTEMA, cenario};

fn caminhos(b: &Busca) -> Vec<&str> {
    b.resultados.iter().map(|r| r.caminho.as_str()).collect()
}

/// Critério de aceite da #38: a busca por um termo do sistema fictício encontra a
/// regra e a análise de origem.
#[test]
fn busca_encontra_a_regra_e_a_analise_de_origem() {
    let c = cenario();
    let aprovacao = c.aprovar_analise_da_multa();
    let regra = format!("{SISTEMA}/{}", aprovacao.regras[0].arquivo);
    let analise = format!("{SISTEMA}/analises/A-0001.md");

    let b = buscar(&c.dados, "multa", None, &[], 0).unwrap();
    let achados = caminhos(&b);
    assert!(achados.contains(&regra.as_str()), "{achados:?}");
    assert!(achados.contains(&analise.as_str()), "{achados:?}");
    let r = b.resultados.iter().find(|r| r.caminho == regra).unwrap();
    assert_eq!(r.tipo.as_deref(), Some("regra"));
    assert_eq!(r.status.as_deref(), Some("confirmada"));
    assert_eq!(r.identificador.as_deref(), Some("RN-COB-001"));
    assert!(r.trecho.contains("**"), "{}", r.trecho);
    assert!(!b.qualquer_palavra);

    // Sem acento e só no sistema; filtro por tipo.
    let b = buscar(&c.dados, "carencia", Some(SISTEMA), &["termo".into()], 0).unwrap();
    assert_eq!(b.total, 1, "{:?}", caminhos(&b));
    assert_eq!(b.resultados[0].tipo.as_deref(), Some("termo"));
    let b = buscar(&c.dados, "multa", None, &["analise".into()], 0).unwrap();
    assert_eq!(caminhos(&b), [analise.as_str()]);

    // Nada com todas as palavras: tenta com qualquer uma.
    let b = buscar(&c.dados, "multa inexistentezzz", None, &[], 0).unwrap();
    assert!(b.qualquer_palavra);
    assert!(b.total >= 2);

    assert_eq!(buscar(&c.dados, "  ", None, &[], 0).unwrap().total, 0);
    assert_eq!(
        buscar(&c.dados, "multa", Some("outro"), &[], 0)
            .unwrap()
            .total,
        0
    );
}

#[test]
fn abre_a_regra_com_a_ligacao_para_a_analise_de_origem() {
    let c = cenario();
    let aprovacao = c.aprovar_analise_da_multa();
    let regra = format!("{SISTEMA}/{}", aprovacao.regras[0].arquivo);
    let analise = format!("{SISTEMA}/analises/A-0001.md");

    let d = abrir_documento(&c.dados, &regra).unwrap();
    assert_eq!(d.tipo.as_deref(), Some("regra"));
    assert_eq!(d.sistema.as_deref(), Some(SISTEMA));
    assert_eq!(d.status.as_deref(), Some("confirmada"));
    assert!(d.problema.is_none());
    assert!(!d.markdown.starts_with("---"), "{}", d.markdown);
    assert!(d.markdown.contains("multa"), "{}", d.markdown);
    assert!(
        d.campos
            .iter()
            .any(|c| c.chave == "id" && c.valor == "RN-COB-001")
    );
    let origem = d
        .ligacoes
        .iter()
        .find(|l| l.caminho == analise)
        .unwrap_or_else(|| panic!("{:?}", d.ligacoes));
    assert!(origem.sai && origem.existe);
    assert_eq!(origem.campo, "origem");
    assert_eq!(origem.titulo.as_deref(), Some("Multa por atraso"));
    assert!(d.analise.is_none());

    // A análise mostra a regra (e o termo) que saíram dela.
    let a = abrir_documento(&c.dados, &analise).unwrap();
    assert_eq!(
        a.analise,
        Some(AnaliseDoDocumento {
            sistema: SISTEMA.into(),
            id: "A-0001".into()
        })
    );
    let vinda = a.ligacoes.iter().find(|l| l.caminho == regra).unwrap();
    assert!(!vinda.sai);
    assert!(a.ligacoes.iter().any(|l| l.caminho.contains("glossario/")));

    // O regras-do-sistema.md não tem frontmatter.
    let r = abrir_documento(&c.dados, &format!("{SISTEMA}/regras-do-sistema.md")).unwrap();
    assert_eq!(r.titulo, "Regras do sistema");
    assert!(r.campos.is_empty());
}

#[test]
fn so_abre_documentos_markdown_dentro_da_base() {
    let c = cenario();
    for caminho in [
        "../dados/config.json".to_owned(),
        "/etc/passwd".to_owned(),
        "C:/Windows/win.ini".to_owned(),
        String::new(),
        format!("{SISTEMA}/analises/A-0001.estado.json"),
        format!("{SISTEMA}/nao-existe.md"),
        "_templates/pergunta.md".to_owned(),
    ] {
        assert!(abrir_documento(&c.dados, &caminho).is_err(), "{caminho}");
    }
    #[cfg(unix)]
    {
        let fora = c.dir.path().join("fora.md");
        std::fs::write(&fora, "segredo").unwrap();
        let link = c.dir.path().join("base").join(SISTEMA).join("link.md");
        std::os::unix::fs::symlink(&fora, &link).unwrap();
        let erro = abrir_documento(&c.dados, &format!("{SISTEMA}/link.md")).unwrap_err();
        assert!(erro.contains("fora da base"), "{erro}");
    }
}

#[test]
fn separa_o_frontmatter_do_corpo() {
    assert_eq!(separar("---\na: 1\n---\n# T\n"), (Some("a: 1\n"), "# T\n"));
    assert_eq!(separar("# Sem\n"), (None, "# Sem\n"));
    assert_eq!(separar("---\nsem fim\n"), (None, "---\nsem fim\n"));
}

#[test]
fn vigia_avisa_quando_a_base_muda_fora_do_app() {
    let c = cenario();
    let vigia = VigiaBase::default();
    let (tx, rx) = std::sync::mpsc::channel();
    let tx = Mutex::new(tx);
    vigia.assinar(move |s| {
        let _ = tx.lock().unwrap().send(*s);
    });
    vigia.garantir(&c.dados, Duration::from_millis(50));
    // A primeira rodada indexa a base criada pelo cadastro.
    let _ = rx.recv_timeout(Duration::from_secs(5));
    let novo = c
        .dir
        .path()
        .join("base")
        .join(SISTEMA)
        .join("glossario")
        .join("fatura.md");
    std::fs::create_dir_all(novo.parent().unwrap()).unwrap();
    std::fs::write(
        &novo,
        "---\ntipo: termo\ntermo: Fatura\n---\n\nDocumento de cobrança.\n",
    )
    .unwrap();
    let s = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("aviso da vigia");
    assert!(s.mudou());
    let b = buscar(&c.dados, "fatura cobranca", None, &["termo".into()], 0).unwrap();
    assert_eq!(b.total, 1);
    // A mesma base não inicia outra vigia.
    vigia.garantir(&c.dados, Duration::from_millis(50));
}
