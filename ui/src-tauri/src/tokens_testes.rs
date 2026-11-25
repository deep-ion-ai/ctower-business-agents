//! Testes dos tokens CSS do projeto (#384) com o sistema fictício "Faturamento
//! Exemplo".

use cba_core::tokens::SituacaoPapel;
use serde_json::json;

use super::*;
use crate::teste_base::{PROJETO, cenario};

#[test]
fn extrai_revisa_e_aprova() {
    let c = cenario();
    assert!(tokens_do_projeto(&c.dados, PROJETO).unwrap().is_none());

    let t = extrair_tokens(&c.dados, PROJETO).unwrap();
    assert_eq!(t.estado, EstadoTokens::Rascunho);
    assert!(t.arquivo.ends_with("prototipo/tokens.css"), "{}", t.arquivo);
    assert!(t.css.contains("--cor-primaria: #1f5fa8;"));
    assert!(t.a_confirmar > 0);
    let j = serde_json::to_value(&t).unwrap();
    assert_eq!(j["estado"], "rascunho");
    assert!(j["aConfirmar"].as_u64().unwrap() > 0);
    let sucesso = t.papeis.iter().find(|p| p.nome == "cor-sucesso").unwrap();
    assert_eq!(sucesso.situacao, SituacaoPapel::AConfirmar);
    assert_eq!(
        serde_json::to_value(sucesso).unwrap()["situacao"],
        json!("a-confirmar")
    );

    // Valor que quebraria o CSS é recusado.
    assert!(informar_token(&c.dados, PROJETO, "cor-sucesso", Some("x; }")).is_err());

    let t = informar_token(&c.dados, PROJETO, "cor-sucesso", Some("#2e7d32")).unwrap();
    let sucesso = t.papeis.iter().find(|p| p.nome == "cor-sucesso").unwrap();
    assert_eq!(sucesso.situacao, SituacaoPapel::Informado);

    let t = aprovar_tokens(&c.dados, PROJETO).unwrap();
    assert_eq!(t.estado, EstadoTokens::Aprovado);
    assert!(t.aprovado_ms.is_some());
    let amb = c.ambiente();
    assert_eq!(
        TokensProjeto::css_aprovado(&amb.kb).unwrap().as_deref(),
        Some(t.css.as_str())
    );

    // Extrair de novo volta ao rascunho e mantém o valor informado.
    let t = extrair_tokens(&c.dados, PROJETO).unwrap();
    assert_eq!(t.estado, EstadoTokens::Rascunho);
    assert!(t.css.contains("--prototipo-cor-sucesso: #2e7d32;"));
    assert_eq!(TokensProjeto::css_aprovado(&amb.kb).unwrap(), None);
}
