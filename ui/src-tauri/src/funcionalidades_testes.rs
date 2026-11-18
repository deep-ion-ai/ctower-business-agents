//! Testes do mapa de funcionalidades (#208) com o sistema fictício "Faturamento
//! Exemplo".

use cba_core::analise::Fluxo;
use cba_core::funcionalidades::DecisaoFuncionalidade;
use cba_tools::aprovacao::OpcoesAprovacao;
use cba_tools::revisao::{self, Decisao};
use serde_json::json;

use super::*;
use crate::teste_base::{PROJETO, cenario};

#[test]
fn mapeia_aprova_lista_e_abre_a_explicacao() {
    let c = cenario();
    let criada = criar_analise_de_mapeamento(
        &c.dados,
        PROJETO,
        Some("db"),
        None,
        Some("Comece pelo banco."),
    )
    .unwrap();
    assert_eq!(criada.analise.fluxo, Fluxo::Mapear);
    assert!(criada.prompt_inicial.contains(&criada.analise.id));

    // Escopo `db`: 9 rotinas públicas, nenhuma tratada.
    let completude = completude_do_mapa(&c.dados, PROJETO, &criada.analise.id).unwrap();
    assert_eq!(completude.total, 9);
    assert_eq!(completude.sem_tratamento.len(), 9);
    assert!(
        completude
            .sem_tratamento
            .iter()
            .all(|p| p.tipo == TipoEntrada::RotinaBanco && p.arquivo.starts_with("db/"))
    );

    // O agente registra uma funcionalidade; o analista aceita e aprova.
    let amb = ambiente(&c.dados, PROJETO).unwrap();
    let id = cba_kb::Id::analisar(&criada.analise.id).unwrap();
    cba_tools::registro_padrao()
        .chamar(
            "registrar_funcionalidade",
            Ok(&amb),
            Some(json!({
                "analise": id,
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
    let completude = completude_do_mapa(&c.dados, PROJETO, &criada.analise.id).unwrap();
    assert_eq!(completude.sem_tratamento.len(), 8);

    // Para concluir sem tratar tudo, o teste justifica o resto direto no estado.
    amb.analises
        .alterar_e(
            &id,
            cba_core::eventos::Mudanca::Funcionalidade,
            |a, agora| {
                for p in &completude.sem_tratamento {
                    a.justificar_ponto(cba_core::funcionalidades::PontoJustificado {
                        chave: p.chave.clone(),
                        tipo: p.tipo,
                        nome: p.nome.clone(),
                        arquivo: p.arquivo.clone(),
                        motivo: cba_core::funcionalidades::MotivoSemFuncionalidade::Outro,
                        justificativa: "fora deste teste".into(),
                        sessao: None,
                        registrado_ms: agora,
                    })?;
                }
                a.concluir()
            },
            |_| {},
        )
        .unwrap();
    revisao::revisar(
        &amb,
        &id,
        Decisao::Funcionalidade {
            funcionalidade: IdFuncionalidade::analisar("F-CAN-001").unwrap(),
            decisao: DecisaoFuncionalidade::Aceitar,
        },
    )
    .unwrap();
    revisao::aprovar_revisada(&amb, &id, &OpcoesAprovacao::default()).unwrap();

    let catalogo = catalogo_funcionalidades(&c.dados, PROJETO).unwrap();
    assert_eq!(catalogo.total, 1);
    assert_eq!(catalogo.documentadas, 0);
    assert_eq!(catalogo.areas[0].nome, "Cancelamento");
    let item = &catalogo.areas[0].funcionalidades[0];
    assert_eq!(item.id, "F-CAN-001");
    assert_eq!(item.pontos, ["PKG_COBRANCA.PRC_CANCELAR_FATURA"]);
    assert_eq!(item.situacao, SituacaoNoCatalogo::Nova);
    assert!(
        catalogo
            .documento
            .as_deref()
            .is_some_and(|d| d.ends_with("funcionalidades/catalogo.md"))
    );
    let json = serde_json::to_value(&catalogo).unwrap();
    assert_eq!(json["areas"][0]["funcionalidades"][0]["situacao"], "nova");

    let explicacao = explicar_funcionalidade(&c.dados, PROJETO, "F-CAN-001").unwrap();
    assert_eq!(explicacao.analise.fluxo, Fluxo::Explicar);
    assert!(
        explicacao
            .analise
            .titulo
            .starts_with("F-CAN-001 · Cancelar fatura")
    );
    let erro = explicar_funcionalidade(&c.dados, PROJETO, "F-CAN-009").unwrap_err();
    assert!(erro.contains("F-CAN-009"), "{erro}");
}
