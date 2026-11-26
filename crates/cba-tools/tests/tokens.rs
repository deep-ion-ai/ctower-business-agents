//! Tokens CSS do projeto (#384) com o sistema fictício "Faturamento Exemplo":
//! extração da fixture de código da interface, "a confirmar", revisão do
//! analista e a ferramenta `tokens_css`.

use std::path::{Path, PathBuf};

use cba_core::ambiente::Ambiente;
use cba_core::config::Config;
use cba_core::dados::DiretorioDados;
use cba_core::tokens::{EstadoTokens, SituacaoPapel, TokensProjeto};
use cba_tools::registro_padrao;
use serde_json::{Value, json};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/faturamento-exemplo")
}

fn ambiente(dir: &tempfile::TempDir) -> Ambiente {
    let raiz = dir.path();
    let dados = DiretorioDados::em(raiz.join("dados"));
    let projeto = cba_index::Projeto::novo("Faturamento Exemplo", [fixture()]).unwrap();
    cba_index::Cadastro::em(dados.projetos())
        .cadastrar(&projeto)
        .unwrap();
    cba_kb::Base::criar(raiz.join("base")).unwrap();
    Config {
        base: Some(raiz.join("base")),
        ..Default::default()
    }
    .gravar(&dados)
    .unwrap();
    Ambiente::do_diretorio(&fixture(), &dados).unwrap()
}

fn tokens_css(amb: &Ambiente) -> (String, Value) {
    let s = registro_padrao()
        .chamar("tokens_css", Ok(amb), Some(json!({})))
        .unwrap();
    (s.texto, s.dados.unwrap())
}

#[test]
fn extrai_da_fixture_marca_a_confirmar_e_so_libera_o_aprovado() {
    let dir = tempfile::tempdir().unwrap();
    let amb = ambiente(&dir);

    // Antes da extração: prévia, sem gravar.
    let (texto, dados) = tokens_css(&amb);
    assert_eq!(dados["estado"], "previa");
    assert_eq!(dados["aprovado"], false);
    assert!(texto.contains("Não use nas telas"), "{texto}");
    assert!(TokensProjeto::carregar(&amb.kb).unwrap().is_none());

    let t = TokensProjeto::extrair(&amb.kb, &amb.escopo).unwrap();
    assert_eq!(t.estado, EstadoTokens::Rascunho);
    let papel = |nome: &str| t.papeis.iter().find(|p| p.nome == nome).unwrap();

    // Do tema.css e do _parcelamento.scss da fixture.
    let primaria = papel("cor-primaria");
    assert_eq!(primaria.situacao, SituacaoPapel::Encontrado);
    assert_eq!(primaria.valor.as_deref(), Some("var(--cor-primaria)"));
    assert_eq!(
        primaria.origem.as_ref().unwrap().to_string(),
        "frontend/src/estilos/tema.css:3"
    );
    assert_eq!(papel("cor-fundo").token.as_deref(), Some("--cor-fundo"));
    assert_eq!(papel("cor-alerta").token.as_deref(), Some("--cor-alerta"));
    assert_eq!(
        papel("fonte-familia").token.as_deref(),
        Some("--fonte-base")
    );
    assert_eq!(
        papel("fonte-tamanho-titulo").token.as_deref(),
        Some("--fonte-tamanho-titulo")
    );
    assert_eq!(
        papel("espaco-medio").token.as_deref(),
        Some("--espaco-base")
    );
    assert_eq!(
        papel("espaco-grande").token.as_deref(),
        Some("--espaco-grande")
    );
    assert_eq!(papel("raio-borda").token.as_deref(), Some("--raio-base"));
    // O que o código não tem fica a confirmar, sem valor.
    for nome in ["cor-sucesso", "espaco-pequeno"] {
        let p = papel(nome);
        assert_eq!(
            (p.situacao, p.valor.as_ref()),
            (SituacaoPapel::AConfirmar, None),
            "{nome}"
        );
    }
    // Valor calculado pelo SCSS: achado, sem valor.
    let fundo_do_alerta = t
        .tokens
        .iter()
        .find(|t| t.nome == "--cor-alerta-fundo")
        .unwrap();
    assert_eq!(fundo_do_alerta.valor, None);
    // A sombra não é token de cor, tipografia, espaçamento nem raio.
    assert!(t.tokens.iter().all(|t| t.nome != "--sombra-cartao"));

    let css = std::fs::read_to_string(amb.kb.caminho(&cba_kb::layout::tokens_css())).unwrap();
    assert!(css.contains("  --cor-primaria: #1f5fa8; /* frontend/src/estilos/tema.css:3 */"));
    assert!(
        css.contains("  --espaco-grande: 24px; /* frontend/src/estilos/_parcelamento.scss:4 */")
    );
    assert!(css.contains("/* --prototipo-cor-sucesso: a confirmar"));
    assert!(css.contains("/* --cor-alerta-fundo: a confirmar"));

    // Em rascunho, a ferramenta não libera para as telas.
    let (texto, dados) = tokens_css(&amb);
    assert_eq!(dados["estado"], "rascunho");
    assert!(texto.contains("ainda não use nas telas"), "{texto}");
    assert!(
        dados["a_confirmar"]
            .as_array()
            .unwrap()
            .contains(&json!("--prototipo-cor-sucesso"))
    );
    assert_eq!(TokensProjeto::css_aprovado(&amb.kb).unwrap(), None);

    // O analista informa a cor de sucesso e aprova.
    TokensProjeto::alterar(&amb.kb, |t, agora| {
        t.informar("cor-sucesso", Some("#2e7d32"))?;
        t.aprovar(agora);
        Ok(())
    })
    .unwrap();
    let (texto, dados) = tokens_css(&amb);
    assert_eq!(dados["aprovado"], true);
    assert!(texto.contains("use este CSS nas telas"), "{texto}");
    let aprovado = TokensProjeto::css_aprovado(&amb.kb).unwrap().unwrap();
    assert_eq!(dados["css"], aprovado.as_str());
    assert!(aprovado.contains("--prototipo-cor-sucesso: #2e7d32; /* informado pelo analista */"));
}
