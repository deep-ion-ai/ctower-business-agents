//! Testes da tela inicial (#38) com o sistema fictício "Faturamento Exemplo".

use super::*;
use crate::teste_base::{PROJETO, cenario};

#[test]
fn projetos_e_analises_em_andamento_e_recentes() {
    let c = cenario();
    let i = inicio(&c.dados).unwrap();
    assert_eq!(i.projetos.len(), 1);
    assert_eq!(i.projetos[0].nome, "Faturamento Exemplo");
    assert!(i.projetos[0].erro.is_none());
    assert!(i.em_andamento.is_empty() && i.recentes.is_empty());

    c.aprovar_analise_da_multa();
    let criada = criar_analise_do_fluxo(
        &c.dados,
        PROJETO,
        Fluxo::Impacto,
        "Permitir parcelamento da fatura em até 3 vezes para pessoa jurídica.",
        None,
    )
    .unwrap();
    assert_eq!(criada.analise.fluxo, Fluxo::Impacto);
    assert!(criada.prompt_inicial.contains("A-0002"));
    let explicar =
        criar_analise_do_fluxo(&c.dados, PROJETO, Fluxo::Explicar, "Cancelamento", None).unwrap();
    assert_eq!(explicar.analise.fluxo, Fluxo::Explicar);

    let i = inicio(&c.dados).unwrap();
    let s = &i.projetos[0];
    assert_eq!(
        (s.analises, s.regras, s.termos, s.a_revalidar),
        (3, 1, 1, 0)
    );
    let ids = |l: &[AnaliseInicio]| l.iter().map(|a| a.analise.id.clone()).collect::<Vec<_>>();
    assert_eq!(ids(&i.em_andamento), ["A-0003", "A-0002"]);
    assert_eq!(ids(&i.recentes), ["A-0003", "A-0002", "A-0001"]);
    assert_eq!(i.recentes[2].analise.estado, EstadoAnalise::Aprovada);

    let json = serde_json::to_value(&i).unwrap();
    assert_eq!(json["recentes"][0]["projetoNome"], "Faturamento Exemplo");
    assert_eq!(json["recentes"][0]["fluxo"], "explicar");
    assert_eq!(json["recentes"][0]["id"], "A-0003");
    assert!(json["recentes"][0]["sessao"].is_null());
}
