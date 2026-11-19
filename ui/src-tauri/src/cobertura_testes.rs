//! Testes da cobertura da base (#125) com o sistema fictício "Faturamento Exemplo".

use cba_core::analise::Fluxo;
use cba_kb::frontmatter::TipoDocumento;
use cba_kb::indice::{Consulta, Indice, Tipo};
use serde_json::json;

use super::*;
use crate::teste_base::{PROJETO, cenario};

#[test]
fn area_sem_documentacao_e_lacuna_e_e_sugerida() {
    let c = cenario();
    // Sem nada na base: nenhuma área, e a sugestão cai na pasta mais referenciada.
    let vazia = cobertura_do_projeto(&c.dados, PROJETO).unwrap();
    assert!(vazia.areas.is_empty());
    assert_eq!(vazia.totais.regras, 0);
    let pasta = vazia.sugestao.expect("sugere uma pasta");
    assert_eq!(pasta.tipo, "pasta");

    // A multa vira regra e termo na área "cobranca"; a "Cancelamento" só tem uma
    // funcionalidade no mapa, sem nenhum documento.
    c.aprovar_analise_da_multa();
    c.chamar(
        "registrar_funcionalidade",
        json!({
            "nome": "Cancelar fatura",
            "area": "Cancelamento",
            "descricao": "Cancela uma fatura aberta, com motivo.",
            "pontos_de_entrada": [{
                "ponto": "PKG_COBRANCA.PRC_CANCELAR_FATURA",
                "evidencia": {
                    "arquivo": "db/03_pkg_cobranca.sql",
                    "linha_inicial": 99, "linha_final": 99,
                    "trecho": "PROCEDURE PRC_CANCELAR_FATURA"
                }
            }]
        }),
    );

    let cobertura = cobertura_do_projeto(&c.dados, PROJETO).unwrap();
    let cancelamento = cobertura
        .areas
        .iter()
        .find(|a| a.id == "cancelamento")
        .expect("a área do mapa aparece");
    assert_eq!(
        (
            cancelamento.regras,
            cancelamento.termos,
            cancelamento.documentos
        ),
        (0, 0, 0)
    );
    assert_eq!(cancelamento.funcionalidades_sem_documento, 1);
    assert!(cancelamento.lacuna, "{cancelamento:?}");
    assert!(cancelamento.centralidade > 0);
    assert!((cancelamento.calor - 1.0).abs() < f64::EPSILON);
    // A lacuna maior vem primeiro, e é a sugerida, com o fluxo "explicar".
    assert_eq!(cobertura.areas[0].id, "cancelamento");
    let s = cobertura
        .sugestao
        .as_ref()
        .expect("sugere a próxima análise");
    assert_eq!((s.tipo.as_str(), s.alvo.as_str()), ("area", "cancelamento"));
    assert_eq!(s.fluxo, Fluxo::Explicar);
    assert_eq!(s.funcionalidade.as_deref(), Some("F-CAN-001"));

    // Os números conferem com o índice.
    let amb = c.ambiente();
    let mut indice = Indice::abrir(c.dados.arquivo_do_indice(), &amb.base).unwrap();
    indice.sincronizar().unwrap();
    let no_indice = |t: TipoDocumento| {
        indice
            .listar(
                &Consulta::nova("")
                    .projeto(amb.kb.nome())
                    .tipos([Tipo::Documento(t)])
                    .pagina(1000, 0),
            )
            .unwrap()
            .total
    };
    let cobranca = cobertura.areas.iter().find(|a| a.id == "cobranca").unwrap();
    assert_eq!(cobranca.regras, no_indice(TipoDocumento::Regra));
    assert_eq!(cobranca.termos, no_indice(TipoDocumento::Termo));
    assert_eq!(cobertura.totais.regras, no_indice(TipoDocumento::Regra));
    assert_eq!(cobertura.totais.termos, no_indice(TipoDocumento::Termo));
    assert_eq!(cobranca.regras, 1);
    assert!(!cobranca.lacuna || cobranca.calor < 1.0);
    assert_eq!(cobranca.idade_recente_dias, Some(0));

    let j = serde_json::to_value(&cobertura).unwrap();
    assert_eq!(j["sugestao"]["fluxo"], "explicar");
    assert!(j["areas"][0]["semConfirmacaoRecente"].is_number());
}
