//! Testes da pauta de perguntas pendentes (#123), com o sistema fictício
//! "Faturamento Exemplo": duas análises da demanda com a mesma pergunta, o JSON
//! que a tela recebe, a resposta que fecha as duas e a exportação.

use super::*;
use crate::demandas::criar_analise_ligada;
use crate::teste_base::{PROJETO, cenario};
use cba_core::analise::{EstadoPergunta, Fluxo};
use cba_core::eventos::Mudanca;
use cba_kb::{Id, NovaDemanda, NovaPessoa};

#[test]
fn pauta_da_demanda_resposta_e_exportacao() {
    let c = cenario();
    let d = crate::demandas::criar_demanda(
        &c.dados,
        PROJETO,
        &NovaDemanda {
            titulo: "Parcelamento PJ".into(),
            ..Default::default()
        },
    )
    .unwrap();
    let amb = c.ambiente();
    amb.kb
        .registrar_pessoa(&NovaPessoa {
            nome: "Maria Silva".into(),
            papel: Some("PO".into()),
            ..Default::default()
        })
        .unwrap();
    let mut ids = Vec::new();
    for entrada in [
        "Parcelar a fatura de PJ em até 3 vezes.",
        "Impacto do parcelamento na remessa do banco.",
    ] {
        let criada =
            criar_analise_ligada(&c.dados, PROJETO, &d.id, Fluxo::Impacto, entrada, None).unwrap();
        let id = Id::analisar(&criada.analise.id).unwrap();
        amb.analises
            .alterar(&id, Mudanca::Pergunta, |a, agora| {
                a.registrar_pergunta("O limite de parcelas vale para PJ?", None, None, agora)
            })
            .unwrap();
        ids.push(format!("{id}/1"));
    }

    definir_destinatario(
        &c.dados,
        PROJETO,
        &ids,
        Some(Destinatario::Pessoa {
            pessoa: "maria-silva".into(),
        }),
    )
    .unwrap();
    let vista = pauta_de_perguntas(&c.dados, PROJETO, Some(&d.id)).unwrap();
    let json = serde_json::to_value(&vista).unwrap();
    assert_eq!(json["total"], 2);
    assert_eq!(json["grupos"][0]["rotulo"], "Maria Silva (PO)");
    assert_eq!(json["grupos"][0]["destinatario"]["tipo"], "pessoa");
    let item = &json["grupos"][0]["itens"][0];
    assert_eq!(item["perguntas"].as_array().unwrap().len(), 2);
    assert_eq!(item["perguntas"][0]["fonte"]["tipo"], "analise");
    assert!(
        json["emails"][0]
            .as_str()
            .unwrap()
            .starts_with("Olá, Maria Silva,")
    );

    // A resposta como a tela envia (camelCase, destino com `tipo`).
    let resposta: RespostaRecebida = serde_json::from_value(serde_json::json!({
        "texto": "Sim, só para PJ.",
        "por": "Maria Silva",
        "pessoa": "maria-silva",
        "em": "2026-11-17",
        "fonte": "e-mail",
        "perguntas": ids,
        "destino": { "tipo": "regra", "titulo": null, "area": null }
    }))
    .unwrap();
    let r = registrar_resposta(&c.dados, PROJETO, &resposta).unwrap();
    assert_eq!(r.fechadas.len(), 2);
    assert!(r.regra.as_ref().unwrap().id.starts_with("RN-"));
    for referencia in &ids {
        let id = Id::analisar(referencia.split('/').next().unwrap()).unwrap();
        let a = amb.analises.carregar(&id).unwrap();
        assert!(matches!(
            a.perguntas[0].estado,
            EstadoPergunta::Respondida { .. }
        ));
    }
    let vazia = pauta_de_perguntas(&c.dados, PROJETO, None).unwrap();
    assert_eq!(vazia.pauta.total, 0);

    let destino = c.dir.path().join("pauta.docx");
    let gravado = exportar_pauta(
        &c.dados,
        PROJETO,
        Some(&d.id),
        Formato::Word,
        destino.to_str().unwrap(),
    )
    .unwrap();
    assert!(gravado.exists());
    assert!(exportar_pauta(&c.dados, PROJETO, None, Formato::Markdown, " ").is_err());
    assert!(pauta_de_perguntas(&c.dados, PROJETO, Some("X-1")).is_err());
}
