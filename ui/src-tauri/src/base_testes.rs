//! Testes da busca, da abertura de documento e da vigia da base (#38), com o
//! sistema fictício "Faturamento Exemplo".

use std::sync::Mutex;
use std::time::Duration;

use super::*;
use crate::teste_base::{PROJETO, cenario};

fn caminhos(b: &Busca) -> Vec<&str> {
    b.resultados.iter().map(|r| r.caminho.as_str()).collect()
}

/// Critério de aceite da #38: a busca por um termo do sistema fictício encontra a
/// regra e a análise de origem.
#[test]
fn busca_encontra_a_regra_e_a_analise_de_origem() {
    let c = cenario();
    let aprovacao = c.aprovar_analise_da_multa();
    let regra = format!("{PROJETO}/{}", aprovacao.regras[0].arquivo);
    let analise = format!("{PROJETO}/analises/A-0001.md");

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

    // Sem acento e só no projeto; filtro por tipo.
    let b = buscar(&c.dados, "carencia", Some(PROJETO), &["termo".into()], 0).unwrap();
    assert_eq!(b.total, 1, "{:?}", caminhos(&b));
    assert_eq!(b.resultados[0].tipo.as_deref(), Some("termo"));
    let b = buscar(&c.dados, "multa", None, &["analise".into()], 0).unwrap();
    assert_eq!(caminhos(&b), [analise.as_str()]);

    // Nada com todas as palavras: tenta com qualquer uma.
    let b = buscar(&c.dados, "multa inexistentezzz", None, &[], 0).unwrap();
    assert!(b.qualquer_palavra);
    assert!(b.total >= 2);

    // Sem texto, lista os documentos (a Base abre com a lista, não vazia).
    let lista = buscar(&c.dados, "  ", None, &[], 0).unwrap();
    assert!(lista.total >= 2, "{lista:?}");
    assert!(lista.resultados.iter().all(|r| r.trecho.is_empty()));
    let so_termos = buscar(&c.dados, "", Some(PROJETO), &["termo".into()], 0).unwrap();
    assert!(so_termos.total >= 1 && so_termos.total < lista.total);
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
    let regra = format!("{PROJETO}/{}", aprovacao.regras[0].arquivo);
    let analise = format!("{PROJETO}/analises/A-0001.md");

    let d = abrir_documento(&c.dados, &regra).unwrap();
    assert_eq!(d.tipo.as_deref(), Some("regra"));
    assert_eq!(d.projeto.as_deref(), Some(PROJETO));
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
            projeto: PROJETO.into(),
            id: "A-0001".into()
        })
    );
    let vinda = a.ligacoes.iter().find(|l| l.caminho == regra).unwrap();
    assert!(!vinda.sai);
    assert!(a.ligacoes.iter().any(|l| l.caminho.contains("glossario/")));

    // O regras-do-projeto.md não tem frontmatter.
    let r = abrir_documento(&c.dados, &format!("{PROJETO}/regras-do-projeto.md")).unwrap();
    assert_eq!(r.titulo, "Regras do projeto");
    assert!(r.campos.is_empty());
}

/// Critério de aceite da #220: editar a regra, salvar e buscar o termo novo a encontra,
/// com o frontmatter intacto.
#[test]
fn salvar_documento_mantem_o_frontmatter_e_reindexa() {
    let c = cenario();
    let aprovacao = c.aprovar_analise_da_multa();
    let regra = format!("{PROJETO}/{}", aprovacao.regras[0].arquivo);
    let antes = abrir_documento(&c.dados, &regra).unwrap();
    assert!(buscar(&c.dados, "zebrafish", None, &[], 0).unwrap().total == 0);

    let novo = format!(
        "{}\nO termo zebrafish agora faz parte da regra.[^1][^2]",
        antes.markdown
    );
    let salvo = salvar_documento(&c.dados, &regra, &novo, &antes.versao).unwrap();
    assert!(salvo.markdown.contains("zebrafish"));
    assert_ne!(salvo.versao, antes.versao);
    assert_eq!(salvo.campos, antes.campos);
    assert_eq!(salvo.titulo, antes.titulo);

    let disco = std::fs::read_to_string(c.dir.path().join("base").join(&regra)).unwrap();
    assert!(disco.starts_with("---\n"), "{disco}");
    assert!(disco.ends_with("[^2]\n"), "{disco}");

    let b = buscar(&c.dados, "zebrafish", None, &[], 0).unwrap();
    assert_eq!(caminhos(&b), [regra.as_str()]);
}

#[test]
fn salvar_documento_recusa_quando_o_arquivo_mudou_fora_do_app() {
    let c = cenario();
    let aprovacao = c.aprovar_analise_da_multa();
    let regra = format!("{PROJETO}/{}", aprovacao.regras[0].arquivo);
    let aberto = abrir_documento(&c.dados, &regra).unwrap();
    let arquivo = c.dir.path().join("base").join(&regra);
    let texto = std::fs::read_to_string(&arquivo).unwrap();
    std::fs::write(&arquivo, format!("{texto}\nmexido por fora\n")).unwrap();

    let erro = salvar_documento(&c.dados, &regra, "meu texto", &aberto.versao).unwrap_err();
    assert!(erro.starts_with(CONFLITO), "{erro}");
    assert!(
        std::fs::read_to_string(&arquivo)
            .unwrap()
            .contains("mexido por fora")
    );
    assert!(salvar_documento(&c.dados, "_templates/x.md", "x", "v").is_err());
}

#[test]
fn so_abre_documentos_markdown_dentro_da_base() {
    let c = cenario();
    for caminho in [
        "../dados/config.json".to_owned(),
        "/etc/passwd".to_owned(),
        "C:/Windows/win.ini".to_owned(),
        String::new(),
        format!("{PROJETO}/analises/A-0001.estado.json"),
        format!("{PROJETO}/nao-existe.md"),
        "_templates/pergunta.md".to_owned(),
    ] {
        assert!(abrir_documento(&c.dados, &caminho).is_err(), "{caminho}");
    }
    #[cfg(unix)]
    {
        let fora = c.dir.path().join("fora.md");
        std::fs::write(&fora, "segredo").unwrap();
        let link = c.dir.path().join("base").join(PROJETO).join("link.md");
        std::os::unix::fs::symlink(&fora, &link).unwrap();
        let erro = abrir_documento(&c.dados, &format!("{PROJETO}/link.md")).unwrap_err();
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
        .join(PROJETO)
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

/// #277: a documentação externa aparece na busca da base com o rótulo da origem e
/// abre só para leitura; salvar é recusado.
#[test]
fn documento_externo_aparece_rotulado_e_nao_se_edita() {
    use cba_kb::externos::{ArquivoExterno, Fonte, importar};

    let c = cenario();
    c.aprovar_analise_da_multa();
    let kb = Base::abrir(c.dir.path().join("base"))
        .unwrap()
        .projeto(PROJETO)
        .unwrap();
    let texto = "# Segunda via do boleto\n\nA segunda via sai com a multa calculada.\n";
    importar(
        &kb,
        &Fonte {
            repositorio: "GitHub exemplo/faturamento-docs".into(),
            branch: "main".into(),
            commit: "c0ffee1234567".into(),
        },
        &[ArquivoExterno {
            caminho: "processos/segunda-via.md".into(),
            caminho_no_repositorio: "docs/negocio/processos/segunda-via.md".into(),
            versao: "1".into(),
            tamanho: texto.len() as u64,
        }],
        &|_| None,
        &mut |_| Ok(texto.as_bytes().to_vec()),
        1,
    )
    .unwrap();
    let caminho =
        format!("{PROJETO}/externos/github-exemplo-faturamento-docs/processos/segunda-via.md");

    let b = buscar(&c.dados, "segunda via", None, &[], 0).unwrap();
    let r = b.resultados.iter().find(|r| r.caminho == caminho).unwrap();
    assert_eq!(r.tipo.as_deref(), Some("doc-externo"));
    let o = r.externo.as_ref().unwrap();
    assert_eq!(o.commit, "c0ffee1234567");
    assert_eq!(o.confianca, "externa");
    let so_externos = buscar(&c.dados, "multa", None, &["externo".into()], 0).unwrap();
    assert_eq!(caminhos(&so_externos), [caminho.as_str()]);

    let d = abrir_documento(&c.dados, &caminho).unwrap();
    assert!(d.somente_leitura);
    assert_eq!(
        d.externo.as_ref().unwrap().caminho,
        "docs/negocio/processos/segunda-via.md"
    );
    let e = salvar_documento(&c.dados, &caminho, "mexido", &d.versao).unwrap_err();
    assert!(e.contains("documentação externa"), "{e}");
    assert!(
        std::fs::read_to_string(c.dir.path().join("base").join(&caminho))
            .unwrap()
            .contains("Segunda via do boleto")
    );
}
