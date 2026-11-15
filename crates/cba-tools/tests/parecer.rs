//! Fluxo "Revisar requisito" (#116) com o sistema fictício "Faturamento Exemplo":
//! uma história fictícia com uma contradição plantada contra a regra RN-FAT-012
//! (só fatura sem nenhum valor pago pode ser cancelada) gera achado de correção
//! com evidência; os termos vagos da lista de referência são apontados; o
//! parecer sai no documento e o analista pode descartar um achado.

use std::path::{Path, PathBuf};

use cba_core::ambiente::Ambiente;
use cba_core::analise::{Analise, EstadoAnalise, Fluxo};
use cba_core::analises::NovaAnalise;
use cba_core::config::Config;
use cba_core::dados::DiretorioDados;
use cba_core::parecer::{EstadoAchado, Veredito};
use cba_kb::frontmatter::{FrontmatterAnalise, FrontmatterRegra, StatusRegra, TipoDocumento};
use cba_kb::{Documento, Id, Serie};
use cba_tools::revisao::{Decisao, pendencias, revisar};
use cba_tools::{ErroChamada, ErroFerramenta, registro_padrao};
use serde_json::{Value, json};

/// A mesma história do gabarito (`revisoes`, REV-FAT-001).
const HISTORIA: &str = "Cancelamento de fatura pelo atendente

Como atendente da central, quero cancelar uma fatura informando o motivo, para corrigir cobranças lançadas por engano.

Critérios de aceite:
- O atendente pode cancelar fatura aberta, inclusive a que já tem pagamento parcial.
- O motivo do cancelamento é obrigatório.
- O cancelamento deve ser rápido e a mensagem de confirmação, adequada.
- Vale para faturas avulsas e/ou parceladas, etc.";

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/faturamento-exemplo")
}

struct Cenario {
    _dir: tempfile::TempDir,
    dados: DiretorioDados,
    amb: Ambiente,
}

fn cenario() -> Cenario {
    let dir = tempfile::tempdir().unwrap();
    let raiz = dir.path();
    let dados = DiretorioDados::em(raiz.join("dados"));
    let projeto = cba_index::Projeto::novo("Faturamento Exemplo", [fixture()]).unwrap();
    cba_index::Cadastro::em(dados.projetos())
        .cadastrar(&projeto)
        .unwrap();
    cba_kb::Base::criar(raiz.join("base")).unwrap();
    Config {
        base: Some(raiz.join("base")),
        ..Default::default()
    }
    .gravar(&dados)
    .unwrap();
    let amb = Ambiente::do_diretorio(&fixture(), &dados).unwrap();
    // A regra conhecida, confirmada na base.
    for (numero, status) in [(12, StatusRegra::Confirmada), (99, StatusRegra::Obsoleta)] {
        let id = Serie::regra("FAT").unwrap().id(numero);
        amb.kb
            .gravar(
                &id.arquivo(),
                &Documento::novo(
                    FrontmatterRegra {
                        tipo: TipoDocumento::Regra,
                        id: id.clone(),
                        titulo: "Cancelamento só de fatura aberta e sem pagamento".into(),
                        area: "cancelamento".into(),
                        status,
                        evidencias: Vec::new(),
                        origem: None,
                    },
                    "Só fatura ABERTA e sem nenhum valor pago pode ser cancelada.\n",
                ),
            )
            .unwrap();
    }
    Cenario {
        _dir: dir,
        dados,
        amb,
    }
}

impl Cenario {
    fn criar(&self, fluxo: Fluxo, entrada: &str) -> Analise {
        self.amb
            .analises
            .criar(NovaAnalise {
                fluxo,
                titulo: "Revisão da história de cancelamento".into(),
                entrada: entrada.into(),
                analista: Some("Ana Exemplo".into()),
                base: None,
            })
            .unwrap()
    }

    fn chamar(&self, nome: &str, args: Value) -> Result<Value, ErroFerramenta> {
        match registro_padrao().chamar(nome, Ok(&self.amb), Some(args)) {
            Ok(saida) => Ok(saida.dados.expect("resposta estruturada")),
            Err(ErroChamada::Ferramenta(e)) => Err(e),
            Err(e) => panic!("{e}"),
        }
    }

    fn recusa(&self, nome: &str, args: Value) -> String {
        match self.chamar(nome, args) {
            Err(ErroFerramenta::Recusa(m)) => m,
            outro => panic!("esperava recusa, veio {outro:?}"),
        }
    }

    fn analise(&self, id: &Id) -> Analise {
        self.amb.analises.carregar(id).unwrap()
    }

    fn documento(&self, id: &Id) -> Documento<FrontmatterAnalise> {
        self.amb.kb.ler(&self.analise(id).documento).unwrap()
    }
}

fn evidencia_do_pagamento() -> Value {
    json!({
        "arquivo": "db/03_pkg_cobranca.sql",
        "linha_inicial": 115,
        "linha_final": 117,
        "trecho": "IF V_VL_PAGO > 0 THEN"
    })
}

fn correcao(analise: &Id) -> Value {
    json!({
        "analise": analise,
        "caracteristica": "correcao",
        "severidade": "alta",
        "trecho": "inclusive a que já tem pagamento parcial",
        "problema": "Contradiz a regra confirmada: o sistema só cancela fatura sem nenhum valor pago; com pagamento parcial, o cancelamento é recusado.",
        "reescrita": "O atendente pode cancelar fatura aberta e sem nenhum valor pago.",
        "regras": ["RN-FAT-012"],
        "evidencias": [evidencia_do_pagamento()]
    })
}

#[test]
fn contradicao_plantada_gera_achado_de_correcao_com_evidencia() {
    let c = cenario();
    let a = c.criar(Fluxo::Revisar, HISTORIA);
    assert_eq!(a.documento.to_string(), "analises/A-0001.md");

    // Sem evidência, a correção é recusada.
    let mut sem = correcao(&a.id);
    sem.as_object_mut().unwrap().remove("evidencias");
    let m = c.recusa("registrar_achado", sem);
    assert!(m.contains("evidência"), "{m}");
    // Trecho que não está no texto.
    let mut fora = correcao(&a.id);
    fora["trecho"] = json!("fatura vencida há 90 dias");
    let m = c.recusa("registrar_achado", fora);
    assert!(m.contains("não está no texto revisado"), "{m}");
    // Regra que não existe e regra obsoleta.
    let mut inexistente = correcao(&a.id);
    inexistente["regras"] = json!(["RN-FAT-050"]);
    let m = c.recusa("registrar_achado", inexistente);
    assert!(m.contains("RN-FAT-050 não existe"), "{m}");
    let mut obsoleta = correcao(&a.id);
    obsoleta["regras"] = json!(["RN-FAT-099"]);
    assert!(c.recusa("registrar_achado", obsoleta).contains("obsoleta"));
    // Evidência que não confere.
    let mut errada = correcao(&a.id);
    errada["evidencias"][0]["trecho"] = json!("IF V_VL_PAGO > 10 THEN");
    c.recusa("registrar_achado", errada);
    assert!(c.analise(&a.id).parecer.is_none(), "recusa não grava");

    let r = c.chamar("registrar_achado", correcao(&a.id)).unwrap();
    assert_eq!(r["achado"], 1);
    assert_eq!(r["caracteristica"], "correcao");
    assert_eq!(r["regras"], json!(["RN-FAT-012"]));
    assert_eq!(r["evidencias"][0]["linhas"], "115-117");
    assert_eq!(r["veredito_sugerido"], "precisa_de_retrabalho");
    assert_eq!(
        r["termos_vagos_sem_achado"],
        json!(["rápido", "adequada", "e/ou", "etc."])
    );

    let estado = c.analise(&a.id);
    assert_eq!(estado.estado, EstadoAnalise::EmAndamento);
    let achado = &estado.parecer.as_ref().unwrap().achados[0];
    assert_eq!(achado.evidencias[0].hash.len(), 64);

    let doc = c.documento(&a.id);
    assert_eq!(doc.frontmatter.tipo, TipoDocumento::ParecerRevisao);
    let corpo = &doc.corpo;
    assert!(corpo.contains("## Requisito revisado"), "{corpo}");
    assert!(
        corpo.contains("inclusive a que já tem pagamento parcial."),
        "{corpo}"
    );
    assert!(corpo.contains("### Severidade alta"), "{corpo}");
    assert!(
        corpo.contains("**Achado 1 · Correção** · “inclusive a que já tem pagamento parcial”"),
        "{corpo}"
    );
    assert!(
        corpo.contains("[RN-FAT-012](../regras/RN-FAT-012.md).[^1]"),
        "{corpo}"
    );
    assert!(
        corpo.contains("[^1]: `db/03_pkg_cobranca.sql:115-117`"),
        "{corpo}"
    );
    assert!(
        corpo.contains("> **Reescrita sugerida:** O atendente pode cancelar fatura aberta e sem nenhum valor pago."),
        "{corpo}"
    );
    // Os termos vagos aparecem no documento mesmo sem achado.
    assert!(
        corpo.contains(
            "_Termos vagos da lista de referência no texto: “rápido”, “adequada”, “e/ou”, “etc.”._"
        ),
        "{corpo}"
    );
}

#[test]
fn termos_vagos_veredito_invest_e_conclusao() {
    let c = cenario();
    let a = c.criar(Fluxo::Revisar, HISTORIA);
    let id = a.id.to_string();

    let r = c
        .chamar("verificar_termos_vagos", json!({ "analise": id }))
        .unwrap();
    let termos: Vec<&str> = r["termos_vagos"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["termo"].as_str().unwrap())
        .collect();
    assert_eq!(termos, ["rápido", "adequado", "e/ou", "etc."]);
    assert!(
        r["termos_vagos"]
            .as_array()
            .unwrap()
            .iter()
            .all(|t| t["com_achado"] == false)
    );

    // O veredito precisa existir para concluir.
    let m = c.recusa("concluir_analise", json!({ "analise": id }));
    assert!(m.contains("veredito"), "{m}");

    c.chamar("registrar_achado", correcao(&a.id)).unwrap();
    let m = c.recusa(
        "registrar_veredito",
        json!({ "analise": id, "veredito": "pronta", "justificativa": "Tudo certo." }),
    );
    assert!(m.contains("precisa_de_retrabalho"), "{m}");
    c.chamar(
        "registrar_veredito",
        json!({
            "analise": id, "veredito": "precisa_de_retrabalho",
            "justificativa": "A história contradiz a regra de cancelamento."
        }),
    )
    .unwrap();

    // Concluída, com aviso dos termos vagos sem achado.
    let r = c
        .chamar("concluir_analise", json!({ "analise": id }))
        .unwrap();
    assert_eq!(r["estado"], "concluida");
    let pendencias_do_agente = r["pendencias"].to_string();
    assert!(
        pendencias_do_agente.contains("termos vagos"),
        "{pendencias_do_agente}"
    );

    // Ambiguidade (um achado para a frase com dois termos) e verificabilidade.
    for (caracteristica, trecho) in [
        (
            "ambiguidade",
            "deve ser rápido e a mensagem de confirmação, adequada",
        ),
        ("ambiguidade", "avulsas e/ou parceladas, etc."),
    ] {
        c.chamar(
            "registrar_achado",
            json!({
                "analise": id, "caracteristica": caracteristica, "severidade": "media",
                "trecho": trecho, "problema": "Termos vagos.",
                "reescrita": "O cancelamento conclui em até 2 segundos."
            }),
        )
        .unwrap();
    }
    // Completude sem trecho, com evidência do motivo mínimo.
    c.chamar(
        "registrar_achado",
        json!({
            "analise": id, "caracteristica": "completude", "severidade": "baixa",
            "problema": "O sistema exige motivo com ao menos 10 caracteres; a história não diz.",
            "evidencias": [{
                "arquivo": "db/03_pkg_cobranca.sql", "linha_inicial": 119, "linha_final": 121,
                "trecho": "LENGTH(TRIM(P_MOTIVO)) < 10"
            }]
        }),
    )
    .unwrap();
    let criterios: Vec<Value> = [
        "independente",
        "negociavel",
        "valiosa",
        "estimavel",
        "pequena",
        "testavel",
    ]
    .iter()
    .map(|c| json!({ "criterio": c, "resultado": "atende", "justificativa": "Ok." }))
    .collect();
    let mut repetidos = criterios.clone();
    repetidos[5] = repetidos[0].clone();
    let e = c
        .chamar(
            "registrar_invest",
            json!({ "analise": id, "criterios": repetidos }),
        )
        .unwrap_err();
    assert!(e.to_string().contains("mais de uma vez"), "{e}");
    let mut criterios = criterios;
    criterios[5] = json!({
        "criterio": "testavel", "resultado": "nao_atende",
        "justificativa": "\"Rápido\" não tem medida."
    });
    let r = c
        .chamar(
            "registrar_invest",
            json!({ "analise": id, "criterios": criterios }),
        )
        .unwrap();
    assert_eq!(r["nao_atendidos"], json!(["testavel"]));
    c.chamar(
        "perguntar",
        json!({ "analise": id, "texto": "A história muda a regra de cancelamento ou o texto está errado?" }),
    )
    .unwrap();

    let r = c
        .chamar("concluir_analise", json!({ "analise": id }))
        .unwrap();
    assert_eq!(r["pendencias"], json!([]));
    assert_eq!(r["registros"]["achados"], 4);

    let corpo = c.documento(&a.id).corpo;
    assert!(
        corpo.contains("**Precisa de retrabalho.** A história contradiz"),
        "{corpo}"
    );
    assert!(
        corpo.contains("Achados: 1 de correção · 2 de ambiguidade · 1 de completude."),
        "{corpo}"
    );
    assert!(corpo.contains("### Severidade média"), "{corpo}");
    assert!(
        corpo.contains("**Achado 4 · Completude**: O sistema exige motivo"),
        "{corpo}"
    );
    assert!(
        corpo.contains("| Testável (T) | não atende | \"Rápido\" não tem medida. |"),
        "{corpo}"
    );
    assert!(
        corpo.contains("## Perguntas para o autor\n\n1. A história muda"),
        "{corpo}"
    );

    // retomar_analise traz o parecer.
    let r = registro_padrao()
        .chamar(
            "retomar_analise",
            Ok(&c.amb),
            Some(json!({ "analise": id })),
        )
        .unwrap();
    assert!(r.texto.contains("## Parecer de revisão"), "{}", r.texto);
    assert!(
        r.texto.contains("Veredito: precisa_de_retrabalho"),
        "{}",
        r.texto
    );
    assert_eq!(
        r.dados.unwrap()["parecer"]["achados"]
            .as_array()
            .unwrap()
            .len(),
        4
    );

    // O analista descarta um achado: sai do documento; o parecer pode ser aprovado.
    assert!(pendencias(&c.analise(&a.id)).is_empty());
    revisar(
        &c.amb,
        &a.id,
        Decisao::Achado {
            numero: 4,
            estado: EstadoAchado::Descartado {
                motivo: Some("O motivo mínimo já está na tela.".into()),
            },
        },
    )
    .unwrap();
    let corpo = c.documento(&a.id).corpo;
    assert!(!corpo.contains("Achado 4"), "{corpo}");
    let parecer = c.analise(&a.id).parecer.unwrap();
    assert_eq!(
        parecer.veredito.unwrap().veredito,
        Veredito::PrecisaDeRetrabalho
    );

    // Aprovado, o parecer não aceita registros novos.
    cba_tools::revisao::aprovar_revisada(&c.amb, &a.id, &Default::default()).unwrap();
    assert_eq!(c.analise(&a.id).estado, EstadoAnalise::Aprovada);
    assert!(c.documento(&a.id).corpo.contains("Status: aprovada"));
    let m = c.recusa("registrar_achado", correcao(&a.id));
    assert!(m.contains("aprovada"), "{m}");

    // O diário registra a mudança do parecer, sem conteúdo.
    let eventos =
        std::fs::read_to_string(c.dados.eventos().join("faturamento-exemplo.jsonl")).unwrap();
    assert!(eventos.contains("\"parecer\""), "{eventos}");
    assert!(!eventos.contains("pagamento parcial"), "{eventos}");
}

#[test]
fn ferramentas_do_parecer_so_valem_na_revisao() {
    let c = cenario();
    let a = c.criar(Fluxo::Perguntar, "Como é cancelada uma fatura?");
    let m = c.recusa("registrar_achado", correcao(&a.id));
    assert!(m.contains("não é uma revisão de requisito"), "{m}");
    let m = c.recusa(
        "registrar_veredito",
        json!({ "analise": a.id, "veredito": "pronta", "justificativa": "Ok." }),
    );
    assert!(m.contains("fluxo `revisar`"), "{m}");
    c.recusa("verificar_termos_vagos", json!({ "analise": a.id }));

    // A ambiguidade precisa do trecho.
    let r = c.criar(Fluxo::Revisar, HISTORIA);
    let e = c
        .chamar(
            "registrar_achado",
            json!({
                "analise": r.id, "caracteristica": "ambiguidade", "severidade": "media",
                "problema": "Vago."
            }),
        )
        .unwrap_err();
    assert!(matches!(e, ErroFerramenta::Argumentos(_)), "{e:?}");
}

#[test]
fn iniciar_analise_cria_a_revisao_com_o_roteiro() {
    let c = cenario();
    let r = registro_padrao()
        .chamar(
            "iniciar_analise",
            Ok(&c.amb),
            Some(json!({ "fluxo": "revisar", "entrada": HISTORIA })),
        )
        .unwrap();
    let dados = r.dados.unwrap();
    assert_eq!(dados["fluxo"], "revisar");
    assert!(r.texto.contains("verificar_termos_vagos"), "{}", r.texto);
    assert!(r.texto.contains("`contexto_no_codigo`"), "{}", r.texto);
    let id = Id::analisar(dados["analise"].as_str().unwrap()).unwrap();
    let doc = c.documento(&id);
    assert_eq!(doc.frontmatter.tipo, TipoDocumento::ParecerRevisao);
    assert!(
        doc.corpo.contains("_Veredito ainda não registrado._"),
        "{}",
        doc.corpo
    );
}

#[test]
fn parecer_exporta_para_markdown_e_word() {
    use cba_tools::revisao::{Formato, OpcoesExportacao, exportar_analise};
    let c = cenario();
    let a = c.criar(Fluxo::Revisar, HISTORIA);
    c.chamar("registrar_achado", correcao(&a.id)).unwrap();
    c.chamar(
        "registrar_veredito",
        json!({
            "analise": a.id, "veredito": "precisa_de_retrabalho",
            "justificativa": "Contradiz a regra de cancelamento."
        }),
    )
    .unwrap();
    let destino = tempfile::tempdir().unwrap();
    let md = exportar_analise(
        &c.amb,
        &a.id,
        &destino.path().join("parecer"),
        Formato::Markdown,
        &OpcoesExportacao::default(),
    )
    .unwrap();
    let texto = std::fs::read_to_string(&md).unwrap();
    assert!(texto.contains("## Veredito"), "{texto}");
    assert!(texto.contains("**Achado 1 · Correção**"), "{texto}");
    assert!(
        texto.contains("IF V_VL_PAGO > 0 THEN"),
        "apêndice com o trecho: {texto}"
    );
    let docx = exportar_analise(
        &c.amb,
        &a.id,
        &destino.path().join("parecer"),
        Formato::Word,
        &OpcoesExportacao::default(),
    )
    .unwrap();
    assert!(docx.extension().is_some_and(|e| e == "docx"));
    assert!(std::fs::metadata(&docx).unwrap().len() > 0);
}
