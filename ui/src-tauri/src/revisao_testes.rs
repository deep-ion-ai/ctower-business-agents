//! Revisão, aprovação e exportação de ponta a ponta com o sistema fictício
//! "Faturamento Exemplo" (#37), pelas funções dos comandos.

use std::collections::BTreeMap;
use std::fs;

use cba_core::analise::{EstadoAfirmacao, EstadoAnalise, EstadoPergunta, EstadoTermo};
use cba_kb::frontmatter::Area;
use cba_kb::layout;
use cba_tools::aprovacao::{AjusteDaRegra, OpcoesAprovacao};
use cba_tools::revisao::{Decisao, Formato, OpcoesExportacao};
use serde_json::json;

use super::*;
use crate::analise;
use crate::teste_base::{Cenario, PKG, SISTEMA, cenario};

const MULTA: &str = "Passada a carência, a multa é de 2% sobre o valor original.";
const ERRADA: &str = "A multa incide também sobre os juros de mora.";
const EDITADA: &str =
    "Depois da carência, a multa por atraso é de 2% sobre o valor original da fatura.";

/// Análise da multa concluída pelo agente: uma afirmação com evidência, uma
/// inferida (que o analista vai marcar como incorreta), um termo e uma pergunta.
fn analise_concluida(c: &Cenario) -> String {
    let criada = analise::criar_analise(
        &c.dados,
        SISTEMA,
        "Como é calculada a multa por atraso de pagamento?",
        Some("Multa por atraso"),
    )
    .unwrap();
    let id = criada.analise.id;
    c.chamar("iniciar_analise", json!({"analise": id}));
    c.chamar(
        "afirmar",
        json!({
            "analise": id,
            "secao": "resposta",
            "texto": MULTA,
            "confianca": "confirmado",
            "evidencias": [{
                "arquivo": PKG,
                "linha_inicial": 71,
                "linha_final": 72,
                "trecho": "FN_PARAMETRO('PERC_MULTA_ATRASO') / 100, 2)"
            }]
        }),
    );
    c.chamar(
        "afirmar",
        json!({"analise": id, "secao": "resposta", "texto": ERRADA, "confianca": "inferido"}),
    );
    c.chamar(
        "propor_termo",
        json!({
            "analise": id,
            "termo": "Carência",
            "definicao": "Dias depois do vencimento em que a fatura não recebe multa.",
        }),
    );
    c.chamar(
        "perguntar",
        json!({"analise": id, "texto": "A carência vale para pessoa jurídica?"}),
    );
    c.chamar("concluir_analise", json!({"analise": id}));
    id
}

/// As três áreas do gabarito no `sistema.md`.
fn declarar_areas(c: &Cenario) {
    let kb = c.ambiente().kb;
    let mut doc = kb.ler_sistema().unwrap();
    let area = |id: &str, nome: &str, sigla: &str| Area {
        id: id.into(),
        nome: nome.into(),
        sigla: Some(sigla.into()),
    };
    doc.frontmatter.areas = vec![
        area("cobranca", "Cobrança", "COB"),
        area("parcelamento", "Parcelamento", "PAR"),
        area("cancelamento", "Cancelamento", "CAN"),
    ];
    kb.gravar(&layout::sistema(), &doc).unwrap();
}

fn decidir(c: &Cenario, id: &str, decisao: serde_json::Value) -> VistaAnalise {
    let decisao: Decisao = serde_json::from_value(decisao).unwrap();
    revisar(&c.dados, SISTEMA, id, decisao).unwrap()
}

fn documento(vista: &VistaAnalise) -> String {
    fs::read_to_string(&vista.documento).unwrap()
}

/// Critérios de aceite da #37: o fluxo completo de revisão e aprovação, e a
/// afirmação incorreta fica registrada e não vira regra.
#[test]
fn revisao_e_aprovacao_de_ponta_a_ponta() {
    let c = cenario();
    declarar_areas(&c);
    let id = analise_concluida(&c);

    // Antes da revisão: nada decidido, e a área é obrigatória (três áreas).
    let p = prontidao(&c.dados, SISTEMA, &id).unwrap();
    assert_eq!(p.estado, EstadoAnalise::Concluida);
    assert!(!p.pode_aprovar);
    let pendentes: Vec<_> = p.pendencias.iter().map(|p| (p.tipo, p.numero)).collect();
    assert_eq!(
        pendentes,
        [
            ("afirmacao", Some(1)),
            ("afirmacao", Some(2)),
            ("termo", Some(1))
        ]
    );
    assert!(p.area_obrigatoria);
    assert_eq!(p.areas.len(), 3);
    assert_eq!(p.avisos.len(), 1, "{:?}", p.avisos);
    let erro = aprovar(&c.dados, SISTEMA, &id, &OpcoesAprovacao::default()).unwrap_err();
    assert!(erro.contains("ainda não pode ser aprovada"), "{erro}");
    assert!(erro.contains("afirmação 1"), "{erro}");

    // Edição do texto: as evidências ficam, e o texto do agente fica guardado.
    let v = decidir(
        &c,
        &id,
        json!({"tipo": "editar-afirmacao", "numero": 1, "texto": EDITADA}),
    );
    let a1 = &v.analise.afirmacoes[0];
    assert_eq!(a1.texto, EDITADA);
    assert_eq!(a1.texto_original.as_deref(), Some(MULTA));
    assert_eq!(a1.evidencias.len(), 1);
    assert_eq!(a1.evidencias[0].arquivo.as_str(), PKG);
    let doc = documento(&v);
    assert!(doc.contains(EDITADA), "{doc}");
    assert!(!doc.contains(MULTA), "{doc}");
    assert!(doc.contains("03_pkg_cobranca.sql"), "{doc}");

    // Decisões: confirmar, aprofundar e depois marcar incorreta, termo e pergunta.
    decidir(
        &c,
        &id,
        json!({"tipo": "afirmacao", "numero": 1, "estado": "confirmada"}),
    );
    let v = decidir(
        &c,
        &id,
        json!({"tipo": "afirmacao", "numero": 2, "estado": "aprofundar"}),
    );
    assert_eq!(v.analise.afirmacoes[1].estado, EstadoAfirmacao::Aprofundar);
    let p = prontidao(&c.dados, SISTEMA, &id).unwrap();
    assert!(
        p.pendencias
            .iter()
            .any(|p| p.motivo.contains("aprofundamento"))
    );
    // Comentário vazio é recusado.
    let vazio: Decisao = serde_json::from_value(
        json!({"tipo": "afirmacao", "numero": 2, "estado": "incorreta", "comentario": " "}),
    )
    .unwrap();
    assert!(revisar(&c.dados, SISTEMA, &id, vazio).is_err());
    let v = decidir(
        &c,
        &id,
        json!({
            "tipo": "afirmacao", "numero": 2, "estado": "incorreta",
            "comentario": "Os juros não entram na base da multa."
        }),
    );
    assert!(!documento(&v).contains(ERRADA));
    decidir(
        &c,
        &id,
        json!({"tipo": "termo", "numero": 1, "estado": "aceito"}),
    );
    let v = decidir(
        &c,
        &id,
        json!({"tipo": "pergunta", "numero": 1, "estado": "respondida", "resposta": "Sim."}),
    );
    assert_eq!(
        v.analise.perguntas[0].estado,
        EstadoPergunta::Respondida {
            resposta: "Sim.".into()
        }
    );
    // A revalidação é do app, não do analista.
    let invalida: Decisao =
        serde_json::from_value(json!({"tipo": "afirmacao", "numero": 1, "estado": "a-revalidar"}))
            .unwrap();
    assert!(revisar(&c.dados, SISTEMA, &id, invalida).is_err());

    let p = prontidao(&c.dados, SISTEMA, &id).unwrap();
    assert!(p.pode_aprovar, "{:?}", p.pendencias);
    assert_eq!(p.regras.len(), 1);
    assert_eq!(p.regras[0].afirmacao, 1);
    assert_eq!(p.incorretas, [2]);
    assert_eq!(p.termos_aceitos, ["Carência"]);

    // Sem área, com três áreas declaradas, a aprovação pede a escolha.
    let erro = aprovar(&c.dados, SISTEMA, &id, &OpcoesAprovacao::default()).unwrap_err();
    assert!(erro.contains("escolha a área"), "{erro}");

    // Área por afirmação no lugar da geral, e o título trocado pelo analista.
    let opcoes = OpcoesAprovacao {
        area: None,
        afirmacoes: BTreeMap::from([(
            1,
            AjusteDaRegra {
                area: Some("Cobrança".into()),
                titulo: Some("Multa por atraso".into()),
            },
        )]),
    };
    // O formato que a tela envia: chaves numéricas como texto.
    let da_tela: OpcoesAprovacao = serde_json::from_value(json!({
        "area": null,
        "afirmacoes": {"1": {"area": "Cobrança", "titulo": "Multa por atraso"}}
    }))
    .unwrap();
    assert_eq!(da_tela, opcoes);
    let feita = aprovar(&c.dados, SISTEMA, &id, &da_tela).unwrap();
    assert!(!feita.ja_aprovada);
    assert_eq!(feita.regras.len(), 1, "a incorreta não vira regra");
    let regra = &feita.regras[0];
    assert_eq!(regra.id, "RN-COB-001");
    assert_eq!(regra.area, "cobranca");
    assert_eq!(regra.afirmacoes, [1]);
    assert_eq!(feita.termos.len(), 1);
    let json = serde_json::to_value(&feita).unwrap();
    assert_eq!(json["jaAprovada"], false);
    assert_eq!(json["regras"][0]["acao"], "criada");

    let kb = c.ambiente().kb;
    let regras: Vec<String> =
        fs::read_dir(kb.caminho(&cba_kb::CaminhoRelativo::novo("regras").unwrap()))
            .unwrap()
            .map(|e| fs::read_to_string(e.unwrap().path()).unwrap())
            .collect();
    assert_eq!(regras.len(), 1);
    assert!(regras[0].contains(EDITADA), "{}", regras[0]);
    assert!(regras[0].contains("Multa por atraso"), "{}", regras[0]);
    assert!(!regras.iter().any(|r| r.contains(ERRADA)));

    // A incorreta fica registrada no estado, com o comentário.
    let vista = analise::carregar_analise(&c.dados, SISTEMA, &id).unwrap();
    assert_eq!(vista.analise.estado, EstadoAnalise::Aprovada);
    assert_eq!(
        vista.analise.afirmacoes[1].estado,
        EstadoAfirmacao::Incorreta {
            comentario: "Os juros não entram na base da multa.".into()
        }
    );
    assert_eq!(vista.analise.termos[0].estado, EstadoTermo::Aceito);
    assert!(documento(&vista).contains("status: aprovada"));

    // Aprovada: a revisão fica fechada, e aprovar de novo só confere a base.
    let depois: Decisao =
        serde_json::from_value(json!({"tipo": "afirmacao", "numero": 2, "estado": "confirmada"}))
            .unwrap();
    let erro = revisar(&c.dados, SISTEMA, &id, depois).unwrap_err();
    assert!(erro.contains("aprovada"), "{erro}");
    let de_novo = aprovar(&c.dados, SISTEMA, &id, &OpcoesAprovacao::default()).unwrap();
    assert!(de_novo.ja_aprovada);
    assert_eq!(de_novo.regras.len(), 1);
}

#[test]
fn exporta_markdown_e_word_fora_das_pastas_do_sistema() {
    let c = cenario();
    let id = analise_concluida(&c);
    decidir(
        &c,
        &id,
        json!({"tipo": "editar-afirmacao", "numero": 1, "texto": EDITADA}),
    );
    let pasta = c.dir.path().join("exportados");
    fs::create_dir_all(&pasta).unwrap();

    let pedido = |destino: std::path::PathBuf, formato| PedidoExportacao {
        destino,
        formato,
        opcoes: OpcoesExportacao::default(),
    };
    // Sem a extensão, o app acrescenta a do formato.
    let md = exportar(
        &c.dados,
        SISTEMA,
        &id,
        &pedido(pasta.join("multa"), Formato::Markdown),
    )
    .unwrap();
    assert_eq!(md, pasta.join("multa.md"));
    let texto = fs::read_to_string(&md).unwrap();
    assert!(texto.contains(EDITADA), "{texto}");
    assert!(
        !texto.starts_with("---"),
        "sem o frontmatter interno: {texto}"
    );
    assert!(
        texto.contains("PERC_MULTA_ATRASO"),
        "apêndice com o trecho: {texto}"
    );

    let docx = exportar(
        &c.dados,
        SISTEMA,
        &id,
        &pedido(pasta.join("multa.DOCX"), Formato::Word),
    )
    .unwrap();
    assert_eq!(docx, pasta.join("multa.DOCX"));
    assert!(fs::read(&docx).unwrap().starts_with(b"PK"));

    // Sem evidências nem perguntas, quando o analista desliga.
    let enxuto = exportar(
        &c.dados,
        SISTEMA,
        &id,
        &PedidoExportacao {
            destino: pasta.join("enxuto.md"),
            formato: Formato::Markdown,
            opcoes: OpcoesExportacao {
                evidencias: false,
                perguntas: false,
                trechos: false,
            },
        },
    )
    .unwrap();
    let texto = fs::read_to_string(enxuto).unwrap();
    assert!(!texto.contains("PERC_MULTA_ATRASO"), "{texto}");
    assert!(!texto.contains("pessoa jurídica"), "{texto}");

    // Dentro do repositório analisado, numa pasta que não existe ou relativo: recusa.
    let dentro = exportar(
        &c.dados,
        SISTEMA,
        &id,
        &pedido(c.codigo.join("db").join("multa.md"), Formato::Markdown),
    )
    .unwrap_err();
    assert!(dentro.contains("pastas do sistema"), "{dentro}");
    assert!(!c.codigo.join("db").join("multa.md").exists());
    assert!(
        exportar(
            &c.dados,
            SISTEMA,
            &id,
            &pedido(pasta.join("nao-existe").join("x.md"), Formato::Markdown),
        )
        .is_err()
    );
    assert!(
        exportar(
            &c.dados,
            SISTEMA,
            &id,
            &pedido("relativo.md".into(), Formato::Markdown),
        )
        .is_err()
    );

    // O pedido no formato que a tela envia.
    let da_tela: PedidoExportacao = serde_json::from_value(json!({
        "destino": pasta.join("tela.docx"),
        "formato": "word",
        "opcoes": {"evidencias": true, "perguntas": false, "trechos": true}
    }))
    .unwrap();
    assert_eq!(da_tela.formato, Formato::Word);
    assert!(!da_tela.opcoes.perguntas);
}
