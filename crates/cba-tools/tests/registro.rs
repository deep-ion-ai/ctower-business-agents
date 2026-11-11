//! Registro de ferramentas com o sistema fictício "Faturamento Exemplo".

use std::path::{Path, PathBuf};

use cba_core::ambiente::{Ambiente, ErroAmbiente};
use cba_core::analise::Fluxo;
use cba_core::analises::NovaAnalise;
use cba_core::config::Config;
use cba_core::dados::DiretorioDados;
use cba_tools::{ErroChamada, ErroFerramenta, Ferramenta, Registro, Saida, registro_padrao};
use serde_json::json;

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/faturamento-exemplo")
}

fn ambiente(raiz: &Path) -> Ambiente {
    let dados = DiretorioDados::em(raiz.join("dados"));
    let sistema = cba_index::Sistema::novo("Faturamento Exemplo", [fixture()]).unwrap();
    cba_index::Cadastro::em(dados.sistemas())
        .cadastrar(&sistema)
        .unwrap();
    cba_kb::Base::criar(raiz.join("base")).unwrap();
    Config {
        base: Some(raiz.join("base")),
    }
    .gravar(&dados)
    .unwrap();
    Ambiente::do_diretorio(&fixture(), &dados).unwrap()
}

#[test]
fn listar_analises_com_filtro() {
    let dir = tempfile::tempdir().unwrap();
    let amb = ambiente(dir.path());
    let a = amb
        .analises
        .criar(NovaAnalise {
            fluxo: Fluxo::Perguntar,
            titulo: "Multa por atraso".into(),
            entrada: "Como é calculada a multa?".into(),
            analista: None,
            base: None,
        })
        .unwrap();
    let r = registro_padrao();

    let saida = r.chamar("listar_analises", Ok(&amb), None).unwrap();
    let dados = saida.dados.unwrap();
    assert_eq!(dados["sistema"]["id"], "faturamento-exemplo");
    assert_eq!(dados["analises"][0]["id"], a.id.to_string());
    assert_eq!(dados["analises"][0]["fluxo"], "perguntar");
    assert_eq!(dados["analises"][0]["estado"], "criada");
    assert_eq!(
        dados["analises"][0]["documento"],
        "faturamento-exemplo/analises/A-0001.md"
    );

    let saida = r
        .chamar(
            "listar_analises",
            Ok(&amb),
            Some(json!({ "estado": "aprovada" })),
        )
        .unwrap();
    assert_eq!(saida.dados.unwrap()["analises"], json!([]));
}

#[test]
fn erros_que_o_agente_corrige() {
    let dir = tempfile::tempdir().unwrap();
    let amb = ambiente(dir.path());
    let r = registro_padrao();

    assert_eq!(
        r.chamar("nao_existe", Ok(&amb), None),
        Err(ErroChamada::Desconhecida("nao_existe".into()))
    );
    let erro = r
        .chamar("listar_analises", Ok(&amb), Some(json!({ "estado": "x" })))
        .unwrap_err();
    assert!(
        matches!(&erro, ErroChamada::Ferramenta(ErroFerramenta::Argumentos(m)) if m.contains("`estado`")),
        "{erro:?}"
    );

    // Fora de um sistema cadastrado: a mensagem do ambiente chega ao agente.
    let fora = ErroAmbiente::ForaDeSistema(dir.path().to_path_buf());
    let erro = r.chamar("listar_analises", Err(&fora), None).unwrap_err();
    let ErroChamada::Ferramenta(ErroFerramenta::SemSistema(m)) = erro else {
        panic!("{erro:?}");
    };
    assert!(m.contains("sistema cadastrado"), "{m}");
}

#[test]
fn panico_na_ferramenta_vira_erro_interno() {
    let dir = tempfile::tempdir().unwrap();
    let amb = ambiente(dir.path());
    let mut r = Registro::new();
    r.registrar(Ferramenta::nova(
        "quebra",
        "Quebra",
        "Sempre falha.",
        json!({ "type": "object" }),
        |_, _| -> Result<Saida, ErroFerramenta> { panic!("de propósito") },
    ));
    let erro = r.chamar("quebra", Ok(&amb), None).unwrap_err();
    assert!(
        matches!(erro, ErroChamada::Ferramenta(ErroFerramenta::Interno(_))),
        "{erro:?}"
    );
}

#[test]
#[should_panic(expected = "registrada duas vezes")]
fn nome_repetido_e_erro_de_programacao() {
    let mut r = registro_padrao();
    r.registrar(cba_tools::analises::listar_analises());
}
