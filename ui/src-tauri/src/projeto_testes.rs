//! Testes da tela do projeto (#38): regras, termos, áreas, revalidação e mapa, com o
//! sistema fictício "Faturamento Exemplo".

use super::*;
use crate::teste_base::{PKG, PROJETO, cenario};

#[test]
fn mostra_as_regras_os_termos_e_as_areas_depois_da_aprovacao() {
    let c = cenario();
    let vazio = visao_projeto(&c.dados, PROJETO).unwrap();
    assert!(vazio.regras.is_empty() && vazio.termos.is_empty());
    assert_eq!(vazio.nome, "Faturamento Exemplo");
    assert_eq!(vazio.documento, format!("{PROJETO}/projeto.md"));

    c.aprovar_analise_da_multa();
    let v = visao_projeto(&c.dados, PROJETO).unwrap();
    assert_eq!(v.regras.len(), 1);
    let r = &v.regras[0];
    assert_eq!(r.id, "RN-COB-001");
    assert_eq!(r.area, "cobranca");
    assert_eq!(r.status, StatusRegra::Confirmada);
    assert_eq!(r.caminho, format!("{PROJETO}/regras/RN-COB-001.md"));
    assert_eq!(
        r.origem.as_deref(),
        Some(format!("{PROJETO}/analises/A-0001.md").as_str())
    );
    assert_eq!(r.evidencias, 1);
    assert_eq!(v.termos.len(), 1);
    assert_eq!(v.termos[0].termo, "Carência");
    let area = v.areas.iter().find(|a| a.id == "cobranca").unwrap();
    assert_eq!((area.regras, area.termos, area.a_revalidar), (1, 1, 0));
    assert!(v.problemas.is_empty(), "{:?}", v.problemas);

    // Formato que a tela espera (ui/src/conhecimento.ts).
    let json = serde_json::to_value(&v).unwrap();
    assert_eq!(json["regras"][0]["status"], "confirmada");
    assert_eq!(json["areas"][0]["aRevalidar"], 0);
}

/// Critério de aceite da #38: a revalidação marca a regra e a lista a destaca
/// (primeiro, com o status `a-revalidar`).
#[test]
fn revalidacao_marca_a_regra_a_revalidar_e_desfaz_a_marca() {
    let c = cenario();
    c.aprovar_analise_da_multa();
    let r = revalidar(&c.dados, PROJETO).unwrap();
    assert!(r.regras.is_empty() && r.desatualizadas == 0, "{r:?}");

    let original = std::fs::read_to_string(c.codigo.join(PKG)).unwrap();
    c.mudar_o_codigo_da_multa();
    let r = revalidar(&c.dados, PROJETO).unwrap();
    assert!(r.mudou);
    assert_eq!(r.regras.len(), 1, "{r:?}");
    assert_eq!(r.regras[0].depois, StatusRegra::ARevalidar);
    assert_eq!(r.analises, ["A-0001"]);
    let v = visao_projeto(&c.dados, PROJETO).unwrap();
    assert_eq!(v.a_revalidar(), 1);
    assert_eq!(v.regras[0].status, StatusRegra::ARevalidar);
    assert_eq!(
        serde_json::to_value(&v).unwrap()["regras"][0]["status"],
        "a-revalidar"
    );
    let area = v.areas.iter().find(|a| a.id == "cobranca").unwrap();
    assert_eq!(area.a_revalidar, 1);
    let inicio = crate::inicio::inicio(&c.dados).unwrap();
    assert_eq!(inicio.projetos[0].a_revalidar, 1);

    // O código volta: a marca é desfeita.
    std::fs::write(c.codigo.join(PKG), original).unwrap();
    let r = revalidar(&c.dados, PROJETO).unwrap();
    assert_eq!(r.regras[0].depois, StatusRegra::Confirmada);
    assert_eq!(visao_projeto(&c.dados, PROJETO).unwrap().a_revalidar(), 0);
}

#[test]
fn mapa_por_pasta_depois_de_mapear() {
    let c = cenario();
    assert!(mapa_projeto(&c.dados, PROJETO, "").unwrap().is_none());
    let raiz = mapear_projeto(&c.dados, PROJETO).unwrap();
    assert_eq!(raiz.modulo.caminho, "");
    assert!(raiz.modulo.arquivos > 0);
    assert!(raiz.modulo.subpastas.iter().any(|s| s.caminho == "db"));
    assert!(
        raiz.modulo.linguagens.iter().any(|l| l.chave == "sql"),
        "{:?}",
        raiz.modulo.linguagens
    );
    let db = mapa_projeto(&c.dados, PROJETO, "db/").unwrap().unwrap();
    assert_eq!(db.modulo.caminho, "db");
    assert!(db.modulo.arquivos_diretos.iter().any(|a| a == PKG));
    assert!(
        db.modulo.simbolos.iter().any(|s| s.chave == "tabela"),
        "{:?}",
        db.modulo.simbolos
    );
    assert!(mapa_projeto(&c.dados, PROJETO, "nao-existe").is_err());
    assert!(visao_projeto(&c.dados, "inexistente").is_err());
}
