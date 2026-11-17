//! `registrar_diagrama` (#118) com o sistema fictício "Faturamento Exemplo": o
//! diagrama registrado entra no estado e no documento como bloco Mermaid gerado
//! pelo app, as recusas não gravam nada e a exportação para Word usa a figura
//! renderizada pelo app.

use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use cba_core::ambiente::Ambiente;
use cba_core::analise::{Analise, Fluxo};
use cba_core::analises::NovaAnalise;
use cba_core::config::Config;
use cba_core::dados::DiretorioDados;
use cba_core::diagramas::TipoDiagrama;
use cba_docs::{Figura, Figuras};
use cba_tools::revisao::{self, Formato, OpcoesExportacao};
use cba_tools::{ErroChamada, ErroFerramenta, registro_padrao};
use serde_json::{Value, json};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/faturamento-exemplo")
}

struct Cenario {
    dir: tempfile::TempDir,
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
    Cenario { dir, dados, amb }
}

impl Cenario {
    fn criar(&self, fluxo: Fluxo) -> Analise {
        self.amb
            .analises
            .criar(NovaAnalise {
                fluxo,
                titulo: "Situações da fatura".into(),
                entrada: "Quais são as situações de uma fatura e como ela muda de uma para outra?"
                    .into(),
                analista: Some("Ana Exemplo".into()),
                base: None,
            })
            .unwrap()
    }

    fn chamar(&self, args: Value) -> Result<Value, ErroFerramenta> {
        match registro_padrao().chamar("registrar_diagrama", Ok(&self.amb), Some(args)) {
            Ok(saida) => Ok(saida.dados.expect("resposta estruturada")),
            Err(ErroChamada::Ferramenta(e)) => Err(e),
            Err(e) => panic!("{e}"),
        }
    }

    fn recusa(&self, args: Value) -> String {
        match self.chamar(args) {
            Err(ErroFerramenta::Recusa(m) | ErroFerramenta::Argumentos(m)) => m,
            outro => panic!("esperava recusa, veio {outro:?}"),
        }
    }

    fn documento(&self, analise: &Analise) -> String {
        std::fs::read_to_string(self.amb.kb.caminho(&analise.documento)).unwrap()
    }
}

fn ciclo(analise: &Analise) -> Value {
    json!({
        "analise": analise.id,
        "secao": "resposta",
        "tipo": "estados",
        "titulo": "Ciclo de vida da fatura",
        "nos": [
            {
                "chave": "ABERTA",
                "rotulo": "Aberta",
                "papel": "inicio",
                "descricao": "Situação de toda fatura nova.",
                "evidencias": [{
                    "arquivo": "db/01_tabelas.sql",
                    "linha_inicial": 28,
                    "linha_final": 28,
                    "trecho": "DEFAULT 'ABERTA'"
                }]
            },
            {"chave": "CANCELADA", "rotulo": "Cancelada", "papel": "fim"},
            {"chave": "PAGA", "rotulo": "Paga", "papel": "fim"}
        ],
        "arestas": [
            {
                "de": "ABERTA",
                "para": "CANCELADA",
                "rotulo": "cancelar",
                "descricao": "Só fatura aberta e sem pagamento.",
                "confianca": "confirmado",
                "evidencias": [{
                    "arquivo": "db/03_pkg_cobranca.sql",
                    "linha_inicial": 111,
                    "linha_final": 124,
                    "trecho": "SET STATUS          = 'CANCELADA'"
                }]
            },
            {"de": "ABERTA", "para": "PAGA", "rotulo": "pagar", "confianca": "a_confirmar"}
        ]
    })
}

#[test]
fn registra_o_diagrama_no_estado_e_no_documento() {
    let c = cenario();
    let a = c.criar(Fluxo::Perguntar);
    let r = c.chamar(ciclo(&a)).unwrap();
    assert_eq!(r["diagrama"], "D-1");
    assert_eq!(r["nos"], 3);
    assert_eq!(r["arestas"], 2);
    let avisos = r["avisos"].as_array().unwrap();
    assert_eq!(avisos.len(), 1, "{r}");
    assert!(avisos[0].as_str().unwrap().contains("ABERTA → PAGA"));

    let estado = c.amb.analises.carregar(&a.id).unwrap();
    let d = estado.diagrama(1).unwrap();
    assert_eq!(d.tipo, TipoDiagrama::Estados);
    assert_eq!(d.nos[0].evidencias[0].arquivo.as_str(), "db/01_tabelas.sql");
    assert!(!d.arestas[0].evidencias[0].hash.is_empty());

    let doc = c.documento(&estado);
    assert!(
        doc.contains("```mermaid\nflowchart LR\n  n1([\"Aberta\"])\n"),
        "{doc}"
    );
    assert!(doc.contains("  n1 -->|\"cancelar\"| n2\n"), "{doc}");
    assert!(doc.contains("  n1 -.->|\"pagar\"| n3\n"), "{doc}");
    assert!(doc.contains("*Figura 1: Ciclo de vida da fatura*"));
    assert!(
        doc.contains(
            "| Aberta → Cancelada | cancelar | Só fatura aberta e sem pagamento.[^2] | confirmado |"
        ),
        "{doc}"
    );
    assert!(
        doc.contains("[^2]: `db/03_pkg_cobranca.sql:111-124`"),
        "{doc}"
    );

    // O evento avisa o app.
    let eventos =
        std::fs::read_to_string(c.dados.eventos().join("faturamento-exemplo.jsonl")).unwrap();
    assert!(eventos.contains("\"mudanca\":\"diagrama\""), "{eventos}");

    // Substituir mantém o número e troca o conteúdo.
    let mut novo = ciclo(&a);
    novo["substituir"] = json!(1);
    novo["titulo"] = json!("Situações da fatura");
    let r = c.chamar(novo).unwrap();
    assert_eq!(r["diagrama"], "D-1");
    let estado = c.amb.analises.carregar(&a.id).unwrap();
    assert_eq!(estado.diagramas.len(), 1);
    let doc = c.documento(&estado);
    assert!(doc.contains("*Figura 1: Situações da fatura*"));
    assert!(!doc.contains("Ciclo de vida da fatura*"));
}

#[test]
fn recusas_nao_gravam_nada() {
    let c = cenario();
    let a = c.criar(Fluxo::Perguntar);

    let mut secao = ciclo(&a);
    secao["secao"] = json!("evidencias");
    assert!(c.recusa(secao).contains("não recebe diagramas"));

    let mut ponta = ciclo(&a);
    ponta["arestas"][1]["para"] = json!("QUITADA");
    assert!(c.recusa(ponta).contains("`QUITADA` não é a chave"));

    let mut sem_evidencia = ciclo(&a);
    sem_evidencia["arestas"][1]["confianca"] = json!("confirmado");
    assert!(c.recusa(sem_evidencia).contains("não tem evidência"));

    let mut trecho = ciclo(&a);
    trecho["arestas"][0]["evidencias"][0]["trecho"] = json!("SET STATUS = 'QUITADA'");
    let m = c.recusa(trecho);
    assert!(m.starts_with("aresta 1 (`ABERTA` → `CANCELADA`)"), "{m}");

    let mut substituir = ciclo(&a);
    substituir["substituir"] = json!(3);
    assert!(c.recusa(substituir).contains("diagrama número 3"));

    let mut tipo = ciclo(&a);
    tipo["tipo"] = json!("dados");
    c.recusa(tipo);

    let estado = c.amb.analises.carregar(&a.id).unwrap();
    assert!(estado.diagramas.is_empty());
    assert!(!c.documento(&estado).contains("```mermaid"));

    let historias = c.criar(Fluxo::Historias);
    assert!(c.recusa(ciclo(&historias)).contains("não recebe diagramas"));
}

/// PNG mínimo (assinatura e IHDR).
fn png() -> Vec<u8> {
    let mut p = b"\x89PNG\r\n\x1a\n".to_vec();
    p.extend_from_slice(&13u32.to_be_bytes());
    p.extend_from_slice(b"IHDR");
    p.extend_from_slice(&640u32.to_be_bytes());
    p.extend_from_slice(&240u32.to_be_bytes());
    p.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
    p
}

fn partes_do_docx(caminho: &Path) -> Vec<(String, Vec<u8>)> {
    let bytes = std::fs::read(caminho).unwrap();
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    (0..zip.len())
        .map(|i| {
            let mut f = zip.by_index(i).unwrap();
            let mut conteudo = Vec::new();
            f.read_to_end(&mut conteudo).unwrap();
            (f.name().unwrap().into_owned(), conteudo)
        })
        .collect()
}

#[test]
fn exporta_para_word_com_a_figura_do_app() {
    let c = cenario();
    let a = c.criar(Fluxo::Perguntar);
    c.chamar(ciclo(&a)).unwrap();

    let blocos = revisao::diagramas_para_exportar(&c.amb, &a.id).unwrap();
    assert_eq!(blocos.len(), 1);
    assert!(blocos[0].starts_with("flowchart LR\n"));

    let mut figuras = Figuras::novas();
    figuras
        .inserir(&blocos[0], Figura::nova(png(), 0, 0).unwrap())
        .unwrap();
    let docx = revisao::exportar_analise(
        &c.amb,
        &a.id,
        &c.dir.path().join("saida"),
        Formato::Word,
        &OpcoesExportacao::default(),
        &figuras,
    )
    .unwrap();
    let partes = partes_do_docx(&docx);
    let figura = partes
        .iter()
        .find(|(n, _)| n == "word/media/figura1.png")
        .expect("figura no pacote");
    assert_eq!(figura.1, png());
    let documento = partes
        .iter()
        .find(|(n, _)| n == "word/document.xml")
        .map(|(_, x)| String::from_utf8(x.clone()).unwrap())
        .unwrap();
    assert!(documento.contains("<wp:inline"));
    assert!(!documento.contains("Diagrama não renderizado"));

    // Sem figura (exportação fora do app), aviso e código no lugar.
    let docx = revisao::exportar_analise(
        &c.amb,
        &a.id,
        &c.dir.path().join("sem-figura"),
        Formato::Word,
        &OpcoesExportacao::default(),
        &Figuras::default(),
    )
    .unwrap();
    let partes = partes_do_docx(&docx);
    assert!(!partes.iter().any(|(n, _)| n.starts_with("word/media/")));
    let documento = partes
        .iter()
        .find(|(n, _)| n == "word/document.xml")
        .map(|(_, x)| String::from_utf8(x.clone()).unwrap())
        .unwrap();
    assert!(documento.contains("Diagrama não renderizado."));
}

#[test]
fn ciclo_de_vida_aponta_achados_e_o_achado_vira_pergunta() {
    use cba_core::analise::{EstadoAnalise, OrigemPergunta};
    use cba_tools::revisao::Decisao;

    let c = cenario();
    let a = c.criar(Fluxo::Explicar);
    let mut args = ciclo(&a);
    args["secao"] = json!("fluxo_principal");
    args["arestas"][0]["condicao"] = json!("aberta, sem pagamento e com motivo");
    args["arestas"][0]["efeitos"] = json!("grava o log CANCELAMENTO");
    args["nos"].as_array_mut().unwrap().push(json!({
        "chave": "PROTESTADA",
        "rotulo": "Protestada",
        "evidencias": [{
            "arquivo": "db/04_pkg_situacao.sql",
            "linha_inicial": 9,
            "linha_final": 10,
            "trecho": "'PROTESTADA'"
        }]
    }));
    let r = c.chamar(args).unwrap();
    let ids: Vec<&str> = r["achados"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["inalcancavel:PROTESTADA", "sem-condicao:2"], "{r}");

    let estado = c.amb.analises.carregar(&a.id).unwrap();
    let doc = c.documento(&estado);
    assert!(
        doc.contains(
            "| Aberta → Cancelada | cancelar | aberta, sem pagamento e com motivo | grava o log \
             CANCELAMENTO |"
        ),
        "{doc}"
    );
    assert!(
        doc.contains(
            "**Achados do ciclo de vida (Ciclo de vida da fatura):**\n- O estado Protestada"
        ),
        "{doc}"
    );

    // "Virar pergunta": a pergunta leva o texto editado e a evidência do estado, e a
    // análise concluída continua concluída.
    c.amb
        .analises
        .alterar(&a.id, cba_core::eventos::Mudanca::Estado, |a, _| {
            a.concluir()
        })
        .unwrap();
    let origem = OrigemPergunta::AchadoDiagrama {
        diagrama: 1,
        achado: "inalcancavel:PROTESTADA".into(),
    };
    let decisao = Decisao::VirarPergunta {
        texto: "A fatura ainda pode ser protestada?".into(),
        origem: origem.clone(),
    };
    // A decisão chega da tela em JSON.
    assert_eq!(
        serde_json::to_value(&decisao).unwrap(),
        json!({
            "tipo": "virar-pergunta",
            "texto": "A fatura ainda pode ser protestada?",
            "origem": {"tipo": "achado-diagrama", "diagrama": 1, "achado": "inalcancavel:PROTESTADA"}
        })
    );
    let depois = revisao::revisar(&c.amb, &a.id, decisao.clone()).unwrap();
    assert_eq!(depois.estado, EstadoAnalise::Concluida);
    let p = &depois.perguntas[0];
    assert_eq!(p.origem.as_ref(), Some(&origem));
    assert_eq!(p.evidencias[0].arquivo.as_str(), "db/04_pkg_situacao.sql");
    let doc = c.documento(&depois);
    let perguntas = &doc[doc.find("A fatura ainda pode ser protestada?").expect(&doc)..];
    assert!(
        perguntas.starts_with("A fatura ainda pode ser protestada?[^"),
        "{doc}"
    );
    assert!(doc.contains("`db/04_pkg_situacao.sql:9-10`"), "{doc}");
    // A pergunta não impede a aprovação.
    let prontidao = revisao::prontidao(&c.amb, &a.id).unwrap();
    assert!(
        prontidao
            .pendencias
            .iter()
            .all(|p| !p.motivo.contains("pergunta")),
        "{:?}",
        prontidao.pendencias
    );
    // Sem duplicar.
    let erro = revisao::revisar(&c.amb, &a.id, decisao).unwrap_err();
    assert!(erro.to_string().contains("já virou a pergunta 1"), "{erro}");
}
