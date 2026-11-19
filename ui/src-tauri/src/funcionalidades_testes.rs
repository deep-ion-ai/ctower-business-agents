//! Testes do mapa de funcionalidades (#208) com o sistema fictício "Faturamento
//! Exemplo".

use cba_core::analise::Fluxo;
use cba_core::funcionalidades::DecisaoFuncionalidade;
use serde_json::json;

use super::*;
use crate::teste_base::{PROJETO, cenario};

#[test]
fn mapa_revisao_explicacao_e_exportacao() {
    let c = cenario();
    // Sem nada registrado: todos os pontos do mapa estrutural sem tratamento.
    let vazio = mapa_funcionalidades(&c.dados, PROJETO).unwrap();
    assert_eq!(vazio.total, 0);
    assert!(vazio.documento.is_none());
    let completude = vazio.completude.as_ref().unwrap();
    assert_eq!(completude.total, 15);
    assert_eq!(completude.sem_tratamento.len(), 15);

    // O agente registra uma funcionalidade (sem análise).
    let amb = ambiente(&c.dados, PROJETO).unwrap();
    cba_tools::registro_padrao()
        .chamar(
            "registrar_funcionalidade",
            Ok(&amb),
            Some(json!({
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
            })),
        )
        .unwrap();
    assert!(amb.analises.listar().unwrap().is_empty());

    let mapa = mapa_funcionalidades(&c.dados, PROJETO).unwrap();
    assert_eq!((mapa.total, mapa.pendentes), (1, 1));
    assert_eq!(mapa.completude.as_ref().unwrap().sem_tratamento.len(), 14);
    assert!(
        mapa.documento
            .as_deref()
            .is_some_and(|d| d.ends_with("funcionalidades/mapa.md"))
    );
    let f = &mapa.areas[0].funcionalidades[0];
    assert_eq!(mapa.areas[0].nome, "Cancelamento");
    assert_eq!(f.id, "F-CAN-001");
    assert_eq!(f.pontos[0].nome, "PKG_COBRANCA.PRC_CANCELAR_FATURA");
    assert!(!f.sem_ponto_de_entrada);
    let j = serde_json::to_value(&mapa).unwrap();
    assert_eq!(j["areas"][0]["funcionalidades"][0]["situacao"], "pendente");
    assert_eq!(
        j["areas"][0]["funcionalidades"][0]["areaNome"],
        "Cancelamento"
    );

    // A revisão vem da tela como JSON.
    let revisao: RevisaoDoMapa = serde_json::from_value(json!({
        "tipo": "decidir", "funcionalidade": "F-CAN-001", "decisao": "aceitar"
    }))
    .unwrap();
    let mapa = revisar_mapa(&c.dados, PROJETO, revisao).unwrap();
    assert_eq!(mapa.pendentes, 0);
    let revisao: RevisaoDoMapa = serde_json::from_value(json!({
        "tipo": "editar", "funcionalidade": "F-CAN-001", "nome": "Cancelar fatura emitida",
        "area": "Cancelamento", "descricao": "Cancela."
    }))
    .unwrap();
    let mapa = revisar_mapa(&c.dados, PROJETO, revisao).unwrap();
    assert!(mapa.areas[0].funcionalidades[0].editada);
    let erro = revisar_mapa(
        &c.dados,
        PROJETO,
        RevisaoDoMapa::Decidir {
            funcionalidade: IdFuncionalidade::analisar("F-CAN-009").unwrap(),
            decisao: DecisaoFuncionalidade::Aceitar,
        },
    )
    .unwrap_err();
    assert!(erro.contains("F-CAN-009"), "{erro}");

    // Exporta o documento, fora das pastas do projeto.
    let destino = c.dir.path().join("saida").join("mapa");
    std::fs::create_dir_all(destino.parent().unwrap()).unwrap();
    let gravado = exportar_mapa(
        &c.dados,
        PROJETO,
        destino.to_str().unwrap(),
        cba_tools::revisao::Formato::Markdown,
    )
    .unwrap();
    let md = std::fs::read_to_string(&gravado).unwrap();
    assert!(md.contains("F-CAN-001"), "{md}");
    assert!(!md.starts_with("---"), "a exportação tira o frontmatter");

    let explicacao = explicar_funcionalidade(&c.dados, PROJETO, "F-CAN-001").unwrap();
    assert_eq!(explicacao.analise.fluxo, Fluxo::Explicar);
    assert!(
        explicacao
            .analise
            .titulo
            .starts_with("F-CAN-001 · Cancelar fatura emitida")
    );
    // Ligada à funcionalidade: a tela mostra a análise, e uma segunda é recusada.
    let mapa = mapa_funcionalidades(&c.dados, PROJETO).unwrap();
    let ligada = mapa.areas[0].funcionalidades[0]
        .explicacao
        .as_ref()
        .unwrap();
    assert_eq!(ligada.id, explicacao.analise.id);
    let erro = explicar_funcionalidade(&c.dados, PROJETO, "F-CAN-001").unwrap_err();
    assert!(erro.contains(&explicacao.analise.id), "{erro}");
    let erro = explicar_funcionalidade(&c.dados, PROJETO, "F-CAN-009").unwrap_err();
    assert!(erro.contains("F-CAN-009"), "{erro}");

    let pedido = pedido_de_mapeamento(Some("db".into()), None, None);
    assert!(pedido.contains("iniciar_mapeamento"), "{pedido}");
}
