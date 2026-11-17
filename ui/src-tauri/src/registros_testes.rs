//! Testes da aba de premissas, decisões, riscos e restrições (#122), com o
//! sistema fictício "Faturamento Exemplo".

use super::*;
use crate::demandas::criar_analise_ligada;
use crate::teste_base::{PROJETO, cenario};
use cba_core::analise::{Confianca, Fluxo, NovaAfirmacao};
use cba_core::eventos::Mudanca;
use cba_kb::{Id, NovaDemanda};

#[test]
fn aba_mostra_propostas_do_impacto_e_o_analista_aceita_cria_e_valida() {
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
    let criada = criar_analise_ligada(
        &c.dados,
        PROJETO,
        &d.id,
        Fluxo::Impacto,
        "Hoje não parcelamos fatura; queremos até 3 vezes para PJ.",
        None,
    )
    .unwrap();
    let amb = c.ambiente();
    amb.analises
        .alterar(
            &Id::analisar(&criada.analise.id).unwrap(),
            Mudanca::Afirmacao,
            |a, agora| {
                for (secao, texto) in [
                    (
                        "demanda_como_entendida",
                        "Premissa do solicitante: hoje não parcelamos fatura. Verificação: \
                         não confere.",
                    ),
                    ("riscos", "Faturas antigas não têm a coluna de parcela."),
                ] {
                    a.registrar_afirmacao(
                        NovaAfirmacao {
                            secao: secao.into(),
                            texto: texto.into(),
                            confianca: Confianca::Inferido,
                            evidencias: vec![],
                            sessao: None,
                        },
                        agora,
                    )?;
                }
                Ok(())
            },
        )
        .unwrap();

    let aba = registros_da_demanda(&c.dados, PROJETO, &d.id).unwrap();
    assert!(aba.registros.is_empty());
    assert_eq!(aba.propostas.len(), 2);
    assert!(aba.pasta.ends_with("/registros"), "{}", aba.pasta);
    let json = serde_json::to_value(&aba.propostas[0]).unwrap();
    assert_eq!(json["categoria"], "premissa");
    assert_eq!(json["fonte"]["tipo"], "analise");
    assert_eq!(json["dados"]["status"], "invalidada");

    let referencia = aba.propostas[1].referencia.clone();
    let risco = aceitar_proposta(&c.dados, PROJETO, &d.id, &referencia, None).unwrap();
    assert_eq!(risco.id, "RI-0001");
    assert!(risco.origem.unwrap().ends_with(".md"));
    recusar_proposta(&c.dados, PROJETO, &d.id, &aba.propostas[0].referencia).unwrap();

    let premissa = criar_registro(
        &c.dados,
        PROJETO,
        &d.id,
        Categoria::Premissa,
        DadosRegistro {
            titulo: "Clientes PJ já têm limite de crédito cadastrado.".into(),
            quem: Some("Ana Exemplo".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(premissa.status, Some(StatusPremissa::NaoValidada));
    let validada = validar_premissa(
        &c.dados,
        PROJETO,
        &premissa.id,
        ValidacaoDePremissa {
            status: StatusPremissa::Validada,
            como: Some("Confirmado pelo financeiro".into()),
            pergunta: None,
        },
    )
    .unwrap();
    assert_eq!(validada.status, Some(StatusPremissa::Validada));
    assert_eq!(validada.historico.len(), 2);

    let aba = registros_da_demanda(&c.dados, PROJETO, &d.id).unwrap();
    assert!(aba.propostas.is_empty());
    let ids: Vec<&str> = aba.registros.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, ["PR-0001", "RI-0001"]);
    assert!(editar_registro(&c.dados, PROJETO, "XX-0001", DadosRegistro::default()).is_err());
}
