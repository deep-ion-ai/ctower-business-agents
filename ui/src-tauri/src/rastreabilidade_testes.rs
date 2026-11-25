//! Testes dos comandos da rastreabilidade (#121), com o sistema fictício
//! "Faturamento Exemplo": a matriz da demanda, a consulta reversa da tela da regra
//! e a exportação.

use super::*;
use crate::teste_base::{PROJETO, cenario};
use cba_kb::NovaDemanda;
use cba_kb::frontmatter::Data;
use cba_kb::indice::Alerta;
use serde_json::json;

const SERVICO: &str = "backend/src/main/java/exemplo/faturamento/ParcelamentoService.java";

fn linha(c: &crate::teste_base::Cenario, n: usize) -> String {
    std::fs::read_to_string(c.codigo.join(SERVICO))
        .unwrap()
        .lines()
        .nth(n - 1)
        .unwrap()
        .trim()
        .to_owned()
}

#[test]
fn matriz_consulta_reversa_e_exportacao() {
    let c = cenario();
    c.ambiente()
        .kb
        .criar_demanda(
            &NovaDemanda {
                titulo: "Parcelamento para pessoa física".into(),
                descricao: None,
                chave: None,
                origem: None,
                tipo: None,
            },
            Data::nova(2026, 11, 13).unwrap(),
        )
        .unwrap();
    c.chamar(
        "iniciar_analise",
        json!({
            "fluxo": "impacto",
            "demanda": "D-0001",
            "titulo": "Parcelamento para pessoa física",
            "entrada": "Permitir que pessoa física parcele faturas."
        }),
    );
    c.chamar(
        "afirmar",
        json!({
            "analise": "A-0001",
            "secao": "como_e_hoje",
            "texto": "Pessoa física não pode parcelar fatura.",
            "confianca": "confirmado",
            "evidencias": [{
                "arquivo": SERVICO, "linha_inicial": 30, "linha_final": 32,
                "trecho": linha(&c, 30)
            }]
        }),
    );
    c.chamar(
        "iniciar_analise",
        json!({ "fluxo": "historias", "origem": "A-0001" }),
    );
    c.chamar(
        "registrar_historia",
        json!({
            "analise": "A-0002",
            "titulo": "Parcelar fatura de pessoa física",
            "como": "atendente", "quero": "parcelar", "para": "receber",
            "criterios": [{
                "dado": "uma fatura de pessoa física", "quando": "pede 3 parcelas",
                "entao": "são geradas 3 parcelas", "afirmacoes": [1]
            }]
        }),
    );

    let m = matriz_demanda(&c.dados, PROJETO, "D-0001").unwrap();
    assert_eq!(m.demanda.titulo, "Parcelamento para pessoa física");
    assert_eq!(m.historias.len(), 1);
    assert_eq!(m.historias[0].id, "HU-0001");
    assert_eq!(m.historias[0].alertas, [Alerta::HistoriaSemCenario]);
    assert!(matriz_demanda(&c.dados, PROJETO, "D-1").is_err());

    let d = dependentes(&c.dados, PROJETO, &AlvoReverso::Arquivo(SERVICO.into())).unwrap();
    assert_eq!(d.demandas[0].id, "D-0001");
    assert_eq!(d.criterios[0].historia, "HU-0001");

    let destino = c.dir.path().join("matriz.xlsx");
    let gravado = exportar_matriz(
        &c.dados,
        PROJETO,
        "D-0001",
        FormatoMatriz::Xlsx,
        destino.to_str().unwrap(),
    )
    .unwrap();
    assert_eq!(gravado, destino);
    assert!(std::fs::read(&destino).unwrap().starts_with(b"PK"));
    let word = c.dir.path().join("matriz.docx");
    exportar_matriz(
        &c.dados,
        PROJETO,
        "D-0001",
        FormatoMatriz::Word,
        word.to_str().unwrap(),
    )
    .unwrap();
    assert!(std::fs::read(&word).unwrap().starts_with(b"PK"));
    assert!(exportar_matriz(&c.dados, PROJETO, "D-0001", FormatoMatriz::Word, " ").is_err());
}
