//! Testes do painel da funcionalidade (#208) com o sistema fictício "Faturamento
//! Exemplo": análises abertas pelo painel, pela demanda e pelo código, demandas
//! ligadas com as etapas e as perguntas em aberto.

use cba_kb::NovaDemanda;
use cba_kb::frontmatter::Data;
use cba_tools::mapear::{RevisaoDoMapa, revisar_mapa};
use serde_json::json;

use super::*;
use crate::teste_base::{PROJETO, cenario};

const PACOTE: &str = "db/03_pkg_cobranca.sql";

fn linha(c: &crate::teste_base::Cenario, n: usize) -> String {
    std::fs::read_to_string(c.codigo.join(PACOTE))
        .unwrap()
        .lines()
        .nth(n - 1)
        .unwrap()
        .trim()
        .to_owned()
}

#[test]
fn painel_junta_analises_demandas_e_perguntas() {
    let c = cenario();
    let amb = c.ambiente();
    c.chamar(
        "registrar_funcionalidade",
        json!({
            "nome": "Cancelar fatura",
            "area": "Cancelamento",
            "descricao": "Cancela uma fatura aberta, com motivo.",
            "pontos_de_entrada": [{
                "ponto": "PKG_COBRANCA.PRC_CANCELAR_FATURA",
                "evidencia": {
                    "arquivo": PACOTE, "linha_inicial": 99, "linha_final": 99,
                    "trecho": "PROCEDURE PRC_CANCELAR_FATURA"
                }
            }]
        }),
    );
    amb.kb
        .criar_demanda(
            &NovaDemanda {
                titulo: "Permitir cancelamento parcial".into(),
                descricao: None,
                chave: None,
                origem: None,
                tipo: None,
            },
            Data::nova(2026, 11, 14).unwrap(),
        )
        .unwrap();

    // Sem explicação aprovada nem demanda, histórias são recusadas.
    let erro =
        criar_analise(&c.dados, PROJETO, "F-CAN-001", Fluxo::Historias, None, None).unwrap_err();
    assert!(erro.contains("explicação aprovada"), "{erro}");

    // Ligar a demanda no painel e abrir a análise de impacto dela.
    revisar_mapa(
        &amb.base,
        &amb.kb,
        &c.dados,
        RevisaoDoMapa::LigarDemanda {
            funcionalidade: IdFuncionalidade::analisar("F-CAN-001").unwrap(),
            demanda: IdDemanda::analisar("D-0001").unwrap(),
            ligar: true,
        },
    )
    .unwrap();
    let impacto = criar_analise(
        &c.dados,
        PROJETO,
        "F-CAN-001",
        Fluxo::Impacto,
        None,
        Some("D-0001"),
    )
    .unwrap();
    assert!(impacto.analise.titulo.contains("F-CAN-001"));
    // Uma pergunta aberta pelo painel, com uma pergunta em aberto para o negócio.
    let pergunta = criar_analise(
        &c.dados,
        PROJETO,
        "F-CAN-001",
        Fluxo::Perguntar,
        Some("Quem pode cancelar uma fatura vencida?"),
        None,
    )
    .unwrap();
    c.chamar(
        "perguntar",
        json!({ "analise": pergunta.analise.id, "texto": "Supervisor também cancela?" }),
    );
    // Uma análise de fora, que só toca o código da funcionalidade.
    let fora = crate::analise::criar_analise_com_demanda(
        &c.dados,
        PROJETO,
        Fluxo::Perguntar,
        "Como a multa é calculada?",
        None,
        None,
    )
    .unwrap();
    c.chamar(
        "afirmar",
        json!({
            "analise": fora.analise.id,
            "secao": "resposta",
            "texto": "O cancelamento fica no pacote de cobrança.",
            "confianca": "confirmado",
            "evidencias": [{
                "arquivo": PACOTE, "linha_inicial": 99, "linha_final": 99,
                "trecho": linha(&c, 99)
            }]
        }),
    );
    // E uma que não tem nada a ver.
    crate::analise::criar_analise_com_demanda(
        &c.dados,
        PROJETO,
        Fluxo::Perguntar,
        "Outra coisa",
        None,
        None,
    )
    .unwrap();

    let p = painel(&c.dados, PROJETO, "F-CAN-001").unwrap();
    let ligacoes: BTreeMap<String, Ligacao> = p
        .analises
        .iter()
        .map(|a| (a.resumo.id.clone(), a.ligacao))
        .collect();
    assert_eq!(ligacoes.len(), 3, "{ligacoes:?}");
    assert_eq!(ligacoes[&impacto.analise.id], Ligacao::Funcionalidade);
    assert_eq!(ligacoes[&pergunta.analise.id], Ligacao::Funcionalidade);
    assert_eq!(ligacoes[&fora.analise.id], Ligacao::Codigo);

    assert_eq!(p.demandas.len(), 1);
    let d = &p.demandas[0];
    assert_eq!(d.id, "D-0001");
    assert!(d.ligada_no_painel && d.aberta);
    assert_eq!(d.impacto.as_ref().unwrap().analise, impacto.analise.id);
    assert!(d.historias.is_none());

    assert_eq!(p.perguntas.len(), 1);
    assert_eq!(p.perguntas[0].texto, "Supervisor também cancela?");
    assert_eq!(p.demandas_do_projeto.len(), 1);
    assert!(!p.atividade.is_empty());
    let j = serde_json::to_value(&p).unwrap();
    assert_eq!(
        j["analises"][0]["ligacao"], "codigo",
        "a mais recente primeiro"
    );
    assert!(j["funcionalidade"]["areaNome"].is_string());

    // A pergunta guarda a funcionalidade, mas não vira documento negocial.
    let estado = amb
        .analises
        .carregar(&cba_kb::Id::analisar(&pergunta.analise.id).unwrap())
        .unwrap();
    assert_eq!(
        estado
            .funcionalidade
            .as_ref()
            .map(ToString::to_string)
            .as_deref(),
        Some("F-CAN-001")
    );
    assert_eq!(
        mapear::ligar_explicacao(&amb.base, &amb.kb, &estado).unwrap(),
        None
    );
    // E não conta como explicação: "Explicar" continua livre.
    assert!(
        crate::funcionalidades::explicar_funcionalidade(&c.dados, PROJETO, "F-CAN-001").is_ok()
    );
}
