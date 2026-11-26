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

#[test]
fn historias_a_partir_de_uma_analise_aprovada() {
    let c = cenario();
    c.aprovar_analise_da_multa();
    let criar = |origem: Option<&str>| {
        crate::analise::criar_analise_de_historias(&c.dados, PROJETO, origem, None, None, None)
    };
    let criada = criar(Some("A-0001")).unwrap();
    assert_eq!(criada.analise.fluxo, Fluxo::Historias);
    assert_eq!(criada.analise.id, "A-0002");
    assert!(criada.analise.titulo.starts_with("Histórias · "));
    assert!(criada.prompt_inicial.contains("A-0002"));
    // Sem origem nem demanda, ou com origem que não existe: recusa.
    let erro = criar(None).unwrap_err();
    assert!(erro.contains("origem"), "{erro}");
    let erro = criar(Some("A-0042")).unwrap_err();
    assert!(erro.contains("A-0042"), "{erro}");
}

#[test]
fn cenarios_a_partir_de_uma_analise_ou_do_pedido() {
    let c = cenario();
    c.aprovar_analise_da_multa();
    let criar = |origem: Option<&str>, entrada: Option<&str>| {
        crate::analise::criar_analise_de_cenarios(&c.dados, PROJETO, origem, None, entrada, None)
    };
    let criada = criar(Some("A-0001"), None).unwrap();
    assert_eq!(criada.analise.fluxo, Fluxo::Cenarios);
    assert!(criada.analise.titulo.starts_with("Cenários · "));
    assert!(criada.prompt_inicial.contains(&criada.analise.id));
    // Só o pedido, com as regras, também vale; sem nada, recusa.
    let criada = criar(None, Some("Cenários do RN-FAT-001")).unwrap();
    assert_eq!(criada.analise.titulo, "Cenários do RN-FAT-001");
    let erro = criar(None, None).unwrap_err();
    assert!(erro.contains("entrada"), "{erro}");
}

#[test]
fn prototipo_a_partir_de_uma_analise_com_o_pedido() {
    use cba_core::prototipo::{Fidelidade, PedidoPrototipo};
    let c = cenario();
    c.aprovar_analise_da_multa();
    let pedido = PedidoPrototipo {
        telas: vec!["Cadastro da fatura".into(), " ".into()],
        fidelidade: Fidelidade::AltaFidelidade,
        fora: Some("Relatório de recebíveis".into()),
        ..PedidoPrototipo::default()
    };
    let criada = crate::analise::criar_analise_de_prototipo(
        &c.dados, PROJETO, "A-0001", None, pedido, None, None,
    )
    .unwrap();
    assert_eq!(criada.analise.fluxo, Fluxo::Prototipo);
    assert!(criada.analise.titulo.starts_with("Protótipo · "));
    assert!(criada.prompt_inicial.contains(&criada.analise.id));
    let vista = crate::analise::carregar_analise(&c.dados, PROJETO, &criada.analise.id).unwrap();
    let p = vista.analise.prototipo.expect("pedido gravado");
    assert_eq!(p.pedido.telas, ["Cadastro da fatura"]);
    assert_eq!(p.pedido.fidelidade, Fidelidade::AltaFidelidade);
    // Sem origem, recusa.
    let erro = crate::analise::criar_analise_de_prototipo(
        &c.dados,
        PROJETO,
        " ",
        None,
        PedidoPrototipo::default(),
        None,
        None,
    )
    .unwrap_err();
    assert!(erro.contains("origem"), "{erro}");
}

#[test]
fn pedido_do_prototipo_vem_da_tela_em_json() {
    use cba_core::prototipo::{Fidelidade, PedidoPrototipo};
    let p: PedidoPrototipo = serde_json::from_value(serde_json::json!({
        "telas": ["Cadastro da fatura"],
        "fidelidade": "alta_fidelidade",
        "fora": "Relatório de recebíveis"
    }))
    .unwrap();
    assert_eq!(p.fidelidade, Fidelidade::AltaFidelidade);
    let p: PedidoPrototipo = serde_json::from_value(serde_json::json!({})).unwrap();
    assert_eq!(p.fidelidade, Fidelidade::Wireframe);
    assert_eq!(p.modo, cba_core::prototipo::ModoPrototipo::Html);
}

#[test]
fn prototipo_hospedado_so_com_a_confirmacao_do_envio() {
    use cba_core::prototipo::{ModoPrototipo, PedidoPrototipo};
    let c = cenario();
    c.aprovar_analise_da_multa();
    // Como a tela manda: o modo e, no hospedado, a confirmação com o aviso lido.
    let sem: PedidoPrototipo = serde_json::from_value(serde_json::json!({
        "telas": [],
        "fidelidade": "wireframe",
        "modo": "design_hospedado"
    }))
    .unwrap();
    let erro = crate::analise::criar_analise_de_prototipo(
        &c.dados, PROJETO, "A-0001", None, sem, None, None,
    )
    .unwrap_err();
    assert!(erro.contains("confirmação explícita"), "{erro}");

    let com: PedidoPrototipo = serde_json::from_value(serde_json::json!({
        "telas": [],
        "fidelidade": "wireframe",
        "modo": "design_hospedado",
        "confirmacao_envio": { "aviso": "As telas e os tokens saem do ambiente." }
    }))
    .unwrap();
    let criada = crate::analise::criar_analise_de_prototipo(
        &c.dados, PROJETO, "A-0001", None, com, None, None,
    )
    .unwrap();
    let vista = crate::analise::carregar_analise(&c.dados, PROJETO, &criada.analise.id).unwrap();
    let pedido = vista.analise.prototipo.expect("pedido gravado").pedido;
    assert_eq!(pedido.modo, ModoPrototipo::DesignHospedado);
    let confirmacao = pedido.confirmacao_envio.expect("confirmação gravada");
    assert!(confirmacao.em_ms > 0);
    assert_eq!(confirmacao.canal.as_deref(), Some("app"));
    assert_eq!(confirmacao.aviso, "As telas e os tokens saem do ambiente.");
}
