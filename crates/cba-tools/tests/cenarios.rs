//! Fluxo "Cenários de teste" (#117) com o sistema fictício "Faturamento Exemplo":
//! `iniciar_analise` cria a análise de cenários (a partir das regras pedidas ou de
//! uma análise de origem); `registrar_tabela_decisao` grava a tabela de decisão do
//! cancelamento após 30 dias (RN-FAT-014), recusando a incompleta;
//! `registrar_cenario` grava os valores-limite com o limite lido do código (30 dias
//! no Delphi, 10 caracteres no PL/SQL), recusa o limite que não está no trecho
//! citado e deixa "a confirmar" o limite de parâmetro (MAX_PARCELAS); os CT-NNNN
//! são sequenciais no projeto; o documento segue o template, a revisão e a
//! aprovação passam pelas decisões do analista e o CSV tem uma linha por cenário.

use std::path::{Path, PathBuf};

use cba_core::ambiente::Ambiente;
use cba_core::analise::{Analise, EstadoAnalise, Fluxo};
use cba_core::analises::NovaAnalise;
use cba_core::cenarios::{
    Condicao, Limite, NovaColuna, NovaTabela, NovoCenario, RefColuna, Tecnica, TextoCenario,
    ValorCondicao,
};
use cba_core::config::Config;
use cba_core::dados::DiretorioDados;
use cba_core::historias::Gherkin;
use cba_core::requisito::{Fundamento, RefItem, SituacaoItem};
use cba_docs::Templates;
use cba_kb::frontmatter::{
    Data, Demanda, Evidencia, FrontmatterAnalise, FrontmatterLivre, FrontmatterRegra, Linhas,
    StatusAnalise, StatusRegra, TipoDocumento, VersaoCodigo,
};
use cba_kb::{CaminhoRelativo, Documento, Id, IdCenario, Serie};
use cba_tools::aprovacao::OpcoesAprovacao;
use cba_tools::revisao::{self, Decisao};
use cba_tools::{ErroChamada, ErroFerramenta, registro_padrao};
use serde_json::{Value, json};

const DELPHI: &str = "delphi/UFrmCancelamento.pas";
const PKG: &str = "db/03_pkg_cobranca.sql";
const PARCELAMENTO: &str = "backend/src/main/java/exemplo/faturamento/ParcelamentoService.java";

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/faturamento-exemplo")
}

/// Linha `n` (a partir de 1) de um arquivo da fixture.
fn linha(arquivo: &str, n: usize) -> String {
    std::fs::read_to_string(fixture().join(arquivo))
        .unwrap()
        .lines()
        .nth(n - 1)
        .unwrap()
        .to_owned()
}

struct Caso {
    dir: tempfile::TempDir,
    amb: Ambiente,
}

fn caso() -> Caso {
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
    let c = Caso { dir, amb };
    for (id, titulo, arquivo, i, f) in [
        (
            "RN-FAT-008",
            "Quantidade de parcelas entre 2 e MAX_PARCELAS",
            PARCELAMENTO,
            34,
            35,
        ),
        (
            "RN-FAT-013",
            "Motivo do cancelamento com ao menos 10 caracteres",
            PKG,
            119,
            121,
        ),
        (
            "RN-FAT-014",
            "Cancelamento após 30 dias da emissão exige supervisor",
            DELPHI,
            69,
            75,
        ),
    ] {
        c.regra(id, titulo, arquivo, i, f);
    }
    c
}

impl Caso {
    fn chamar(&self, nome: &str, args: Value) -> Result<Value, ErroFerramenta> {
        match registro_padrao().chamar(nome, Ok(&self.amb), Some(args)) {
            Ok(saida) => Ok(saida.dados.expect("resposta estruturada")),
            Err(ErroChamada::Ferramenta(e)) => Err(e),
            Err(e) => panic!("{e}"),
        }
    }

    fn texto(&self, nome: &str, args: Value) -> String {
        registro_padrao()
            .chamar(nome, Ok(&self.amb), Some(args))
            .unwrap()
            .texto
    }

    fn recusa(&self, nome: &str, args: Value) -> String {
        match self.chamar(nome, args) {
            Err(ErroFerramenta::Recusa(m) | ErroFerramenta::Argumentos(m)) => m,
            outro => panic!("esperava recusa, veio {outro:?}"),
        }
    }

    fn analise(&self, id: &str) -> Analise {
        self.amb
            .analises
            .carregar(&Id::analisar(id).unwrap())
            .unwrap()
    }

    /// A regra na base, como a aprovação de outra análise a deixaria.
    fn regra(&self, id: &str, titulo: &str, arquivo: &str, inicio: u32, fim: u32) {
        let regra = Id::analisar(id).unwrap();
        self.amb
            .kb
            .gravar(
                &regra.arquivo(),
                &Documento::novo(
                    FrontmatterRegra {
                        tipo: TipoDocumento::Regra,
                        id: regra.clone(),
                        titulo: titulo.into(),
                        area: "cancelamento".into(),
                        status: StatusRegra::Confirmada,
                        evidencias: vec![Evidencia {
                            arquivo: CaminhoRelativo::novo(arquivo).unwrap(),
                            linhas: Linhas::novo(inicio, fim).unwrap(),
                            hash: "h".into(),
                            commit: None,
                        }],
                        origem: None,
                    },
                    format!("\n# {id}\n"),
                ),
            )
            .unwrap();
    }
}

fn ev(arquivo: &str, i: u32, f: u32, trecho: &str) -> Value {
    json!({ "arquivo": arquivo, "linha_inicial": i, "linha_final": f, "trecho": trecho })
}

fn tabela_do_supervisor(colunas: Value) -> Value {
    json!({
        "analise": "A-0001",
        "titulo": "Cancelamento de fatura emitida há mais de 30 dias",
        "condicoes": [
            {
                "texto": "fatura emitida há mais de 30 dias",
                "evidencias": [ev(DELPHI, 70, 70, "(DiasDesdeEmissao(AIdFatura) > DIAS_LIMITE_SEM_SUPERVISOR)")]
            },
            {
                "texto": "usuário com perfil SUPERVISOR",
                "evidencias": [ev(DELPHI, 71, 71, "(FPerfil <> PERFIL_SUPERVISOR)")]
            }
        ],
        "acoes": [
            "segue para o cancelamento",
            "recusa: 'Fatura emitida ha mais de 30 dias. Solicite a um supervisor.'"
        ],
        "colunas": colunas,
        "regras": ["RN-FAT-014"]
    })
}

fn cenario(titulo: &str, tecnica: &str, extra: Value) -> Value {
    let mut v = json!({
        "analise": "A-0001",
        "titulo": titulo,
        "tecnica": tecnica,
        "dado": "uma fatura ABERTA, sem pagamento, emitida há 31 dias",
        "quando": "um atendente sem perfil SUPERVISOR pede o cancelamento",
        "entao": "o cancelamento é recusado: 'Fatura emitida ha mais de 30 dias. Solicite a um supervisor.'",
        "regras": ["RN-FAT-014"]
    });
    for (k, val) in extra.as_object().unwrap() {
        v[k] = val.clone();
    }
    v
}

#[test]
fn cenarios_das_regras_do_cancelamento() {
    let c = caso();

    // Os limites que os cenários vão citar estão no código da fixture.
    assert_eq!(linha(DELPHI, 41).trim(), "DIAS_LIMITE_SEM_SUPERVISOR = 30;");
    assert!(linha(DELPHI, 70).contains("> DIAS_LIMITE_SEM_SUPERVISOR"));
    assert!(linha(PKG, 119).contains("LENGTH(TRIM(P_MOTIVO)) < 10"));

    // Sem origem nem demanda, o pedido diz as regras.
    let m = c.recusa("iniciar_analise", json!({ "fluxo": "cenarios" }));
    assert!(m.contains("`entrada`"), "{m}");
    let r = c
        .chamar(
            "iniciar_analise",
            json!({
                "fluxo": "cenarios",
                "entrada": "Cenários das regras do cancelamento: RN-FAT-013 e RN-FAT-014."
            }),
        )
        .unwrap();
    assert_eq!(r["analise"], "A-0001");
    assert_eq!(r["fluxo"], "cenarios");
    assert!(
        r["roteiro"].as_str().unwrap().contains("tabela de decisão"),
        "{}",
        r["roteiro"]
    );
    let a = c.analise("A-0001");
    let doc: Documento<FrontmatterAnalise> = c.amb.kb.ler(&a.documento).unwrap();
    assert_eq!(doc.frontmatter.tipo, TipoDocumento::CenariosTeste);

    // Tabela incompleta: recusada com as combinações que faltam.
    let m = c.recusa(
        "registrar_tabela_decisao",
        tabela_do_supervisor(json!([
            { "valores": ["S", "N"], "acoes": [2] },
            { "valores": ["N", "-"], "acoes": [1] }
        ])),
    );
    assert!(m.contains("S/S"), "{m}");
    // Evidência que não confere com o código: recusada.
    let mut errada = tabela_do_supervisor(json!([{ "valores": ["-", "-"], "acoes": [1] }]));
    errada["condicoes"][0]["evidencias"][0]["linha_inicial"] = json!(41);
    errada["condicoes"][0]["evidencias"][0]["linha_final"] = json!(41);
    let m = c.recusa("registrar_tabela_decisao", errada);
    assert!(m.contains("condição 1"), "{m}");
    // Sem regra nem afirmação: recusada.
    let mut sem = tabela_do_supervisor(json!([{ "valores": ["-", "-"], "acoes": [1] }]));
    sem.as_object_mut().unwrap().remove("regras");
    let m = c.recusa("registrar_tabela_decisao", sem);
    assert!(m.contains("ao menos uma origem"), "{m}");

    let r = c
        .chamar(
            "registrar_tabela_decisao",
            tabela_do_supervisor(json!([
                { "valores": ["S", "S"], "acoes": [1] },
                { "valores": ["S", "N"], "acoes": [2] },
                { "valores": ["N", "-"], "acoes": [1] }
            ])),
        )
        .unwrap();
    assert_eq!(r["tabela"], "TD-1");
    assert_eq!(
        r["colunas_para_cenarios"],
        json!(["TD-1/R1", "TD-1/R2", "TD-1/R3"])
    );

    // Valor-limite: o limite (30) aparece no trecho citado.
    let limite_30 = json!({
        "grandeza": "dias desde a emissão da fatura",
        "valor": "30",
        "evidencias": [ev(DELPHI, 41, 41, "DIAS_LIMITE_SEM_SUPERVISOR = 30;")]
    });
    let r = c
        .chamar(
            "registrar_cenario",
            cenario(
                "31 dias sem supervisor é recusado",
                "valor_limite",
                json!({ "valor": "31 dias", "limite": limite_30, "tabela": "TD-1/R2" }),
            ),
        )
        .unwrap();
    assert_eq!(r["cenario"], "CT-0001");
    assert_eq!(r["limite_no_codigo"], true);
    assert_eq!(r["a_confirmar"], false);
    let r = c
        .chamar(
            "registrar_cenario",
            cenario(
                "30 dias sem supervisor cancela",
                "valor_limite",
                json!({
                    "valor": "30 dias",
                    "dado": "uma fatura ABERTA, sem pagamento, emitida há 30 dias",
                    "entao": "a fatura é cancelada",
                    "limite": limite_30,
                    "tabela": "TD-1/R3"
                }),
            ),
        )
        .unwrap();
    assert_eq!(r["cenario"], "CT-0002");
    // Limite que não está no trecho citado: recusado.
    let mut errado = limite_30.clone();
    errado["valor"] = json!("31");
    let m = c.recusa(
        "registrar_cenario",
        cenario("x", "valor_limite", json!({ "limite": errado })),
    );
    assert!(m.contains("não aparece no trecho"), "{m}");
    let m = c.recusa("registrar_cenario", cenario("x", "valor_limite", json!({})));
    assert!(m.contains("precisa do `limite`"), "{m}");
    let m = c.recusa(
        "registrar_cenario",
        cenario("x", "tabela_de_decisao", json!({ "tabela": "TD-1/R9" })),
    );
    assert!(m.contains("R9"), "{m}");
    let m = c.recusa(
        "registrar_cenario",
        cenario("x", "negativo", json!({ "historia": "HU-0009" })),
    );
    assert!(m.contains("HU-0009 não existe"), "{m}");
    let mut sem = cenario("x", "negativo", json!({}));
    sem.as_object_mut().unwrap().remove("regras");
    let m = c.recusa("registrar_cenario", sem);
    assert!(m.contains("ao menos uma origem"), "{m}");

    // Motivo: 10 caracteres no PL/SQL.
    let limite_10 = json!({
        "grandeza": "tamanho do motivo, sem espaços nas pontas",
        "valor": "10 caracteres",
        "evidencias": [ev(PKG, 119, 119, "LENGTH(TRIM(P_MOTIVO)) < 10")]
    });
    let r = c
        .chamar(
            "registrar_cenario",
            json!({
                "analise": "A-0001",
                "titulo": "Motivo com 9 caracteres é recusado",
                "tecnica": "valor_limite",
                "dado": "uma fatura ABERTA, sem pagamento",
                "quando": "o atendente cancela com o motivo \"duplicada\" (9 caracteres)",
                "entao": "o banco recusa com o erro -20012: 'Motivo do cancelamento deve ter ao menos 10 caracteres'",
                "valor": "9 caracteres",
                "limite": limite_10,
                "regras": ["RN-FAT-013"]
            }),
        )
        .unwrap();
    assert_eq!(r["cenario"], "CT-0003");

    // Limite de parâmetro (MAX_PARCELAS): fora do código, fica a confirmar.
    let limite_parametro = json!({
        "grandeza": "quantidade de parcelas",
        "valor": "6",
        "evidencias": [ev(PARCELAMENTO, 34, 34, "parametros.inteiro(ParametroRepository.MAX_PARCELAS)")]
    });
    let parcelas = |limite: Value, pergunta: Option<&str>| {
        let mut v = json!({
            "analise": "A-0001",
            "titulo": "7 parcelas são recusadas",
            "tecnica": "valor_limite",
            "dado": "uma fatura ABERTA de pessoa jurídica",
            "quando": "o cliente pede 7 parcelas",
            "entao": "o pedido é recusado: 'Quantidade de parcelas deve estar entre 2 e 6'",
            "valor": "7 parcelas",
            "limite": limite,
            "regras": ["RN-FAT-008"]
        });
        if let Some(p) = pergunta {
            v["pergunta"] = json!(p);
        }
        v
    };
    let m = c.recusa(
        "registrar_cenario",
        parcelas(limite_parametro.clone(), None),
    );
    assert!(m.contains("configuracao"), "{m}");
    let mut conf = limite_parametro.clone();
    conf["configuracao"] = json!("PARAMETRO.MAX_PARCELAS");
    let m = c.recusa("registrar_cenario", parcelas(conf.clone(), None));
    assert!(m.contains("a confirmar"), "{m}");
    c.chamar(
        "perguntar",
        json!({ "analise": "A-0001", "texto": "Qual é o valor de MAX_PARCELAS em produção?" }),
    )
    .unwrap();
    let r = c
        .chamar("registrar_cenario", parcelas(conf, Some("A-0001/1")))
        .unwrap();
    assert_eq!(r["cenario"], "CT-0004");
    assert_eq!(r["a_confirmar"], true);
    assert_eq!(r["limite_no_codigo"], false);

    // A análise não recebe afirmações.
    let m = c.recusa(
        "afirmar",
        json!({ "analise": "A-0001", "secao": "cenarios", "texto": "x", "confianca": "inferido" }),
    );
    assert!(m.contains("registrar_cenario"), "{m}");

    // Falta cenário para TD-1/R1: não conclui.
    let m = c.recusa("concluir_analise", json!({ "analise": "A-0001" }));
    assert!(m.contains("TD-1/R1"), "{m}");
    let r = c
        .chamar(
            "registrar_cenario",
            cenario(
                "Supervisor cancela fatura de 31 dias",
                "tabela_de_decisao",
                json!({
                    "quando": "um usuário com perfil SUPERVISOR pede o cancelamento",
                    "entao": "a fatura é cancelada",
                    "tabela": "TD-1/R1"
                }),
            ),
        )
        .unwrap();
    assert_eq!(r["cenario"], "CT-0005");

    // retomar_analise mostra a tabela e os cenários.
    let t = c.texto("retomar_analise", json!({ "analise": "A-0001" }));
    assert!(
        t.contains("### TD-1 · Cancelamento de fatura emitida há mais de 30 dias"),
        "{t}"
    );
    assert!(
        t.contains("- CT-0004 · 7 parcelas são recusadas [valor-limite; sem decisão; a confirmar: pergunta A-0001/1]"),
        "{t}"
    );

    let r = c
        .chamar("concluir_analise", json!({ "analise": "A-0001" }))
        .unwrap();
    assert_eq!(r["estado"], "concluida");
    assert_eq!(r["registros"]["cenarios"], 5);
    assert_eq!(r["registros"]["tabelas"], 1);

    // Documento: segue o template, com a tabela, os limites e a rastreabilidade.
    let texto = std::fs::read_to_string(c.amb.kb.caminho(&a.documento)).unwrap();
    let livre: Documento<FrontmatterLivre> = Documento::analisar(&texto).unwrap();
    assert_eq!(livre.frontmatter.tipo(), Some(TipoDocumento::CenariosTeste));
    let titulos: Vec<&str> = texto.lines().filter(|l| l.starts_with("## ")).collect();
    assert_eq!(
        titulos,
        [
            "## Pedido",
            "## Tabelas de decisão",
            "## Valores-limite e partições",
            "## Cenários de teste",
            "## Rastreabilidade",
            "## Perguntas em aberto",
            "## Evidências"
        ]
    );
    assert!(
        texto.contains("| **Cenário** | CT-0005 | CT-0001 | CT-0002 |"),
        "{texto}"
    );
    assert!(
        texto.contains(
            "| CT-0001 | valor-limite | dias desde a emissão da fatura | **30** | 31 dias |"
        ),
        "{texto}"
    );
    assert!(texto.contains("**6** · a confirmar"), "{texto}");
    assert!(
        texto.contains("`delphi/UFrmCancelamento.pas:41`"),
        "{texto}"
    );
    assert!(
        texto.contains("[RN-FAT-014](../regras/RN-FAT-014.md)"),
        "{texto}"
    );

    // Os CT-NNNN são do projeto: uma segunda análise continua a numeração.
    c.chamar(
        "iniciar_analise",
        json!({ "fluxo": "cenarios", "entrada": "Mais cenários do RN-FAT-013." }),
    )
    .unwrap();
    let mut outro = cenario(
        "Motivo vazio",
        "negativo",
        json!({ "regras": ["RN-FAT-013"] }),
    );
    outro["analise"] = json!("A-0002");
    let r = c.chamar("registrar_cenario", outro).unwrap();
    assert_eq!(r["cenario"], "CT-0006");

    // Revisão: decisões, edição e aprovação.
    let id = Id::analisar("A-0001").unwrap();
    let p = revisao::prontidao(&c.amb, &id).unwrap();
    assert!(!p.pode_aprovar);
    assert!(
        p.pendencias
            .iter()
            .any(|x| x.motivo.contains("CT-0003: sem decisão")),
        "{p:?}"
    );
    let decidir = |d: Value| {
        revisao::revisar(&c.amb, &id, serde_json::from_value::<Decisao>(d).unwrap()).unwrap()
    };
    decidir(json!({ "tipo": "tabela", "numero": 1, "situacao": "aceito" }));
    for n in 1..=5 {
        decidir(
            json!({ "tipo": "cenario", "cenario": format!("CT-000{n}"), "situacao": "aceito" }),
        );
    }
    let a = decidir(json!({
        "tipo": "editar-cenario", "cenario": "CT-0003",
        "titulo": "Motivo com 9 caracteres",
        "dado": "uma fatura ABERTA, sem pagamento",
        "quando": "o atendente cancela com o motivo \"duplicada\" (9 caracteres)",
        "entao": "o banco recusa com o erro -20012"
    }));
    assert!(a.cenario(IdCenario::novo(3)).unwrap().original.is_some());
    // Descartar o único cenário de uma coluna deixa pendência.
    decidir(json!({ "tipo": "cenario", "cenario": "CT-0005", "situacao": "descartado" }));
    let p = revisao::prontidao(&c.amb, &id).unwrap();
    assert!(
        p.pendencias.iter().any(|x| x.motivo.contains("TD-1/R1")),
        "{p:?}"
    );
    decidir(json!({ "tipo": "cenario", "cenario": "CT-0005", "situacao": "aceito" }));
    let p = revisao::prontidao(&c.amb, &id).unwrap();
    assert!(p.pode_aprovar, "{:?}", p.pendencias);
    assert!(
        p.avisos
            .iter()
            .any(|a| a.contains("cenário(s) de teste a confirmar")),
        "{p:?}"
    );
    let aprovada = revisao::aprovar_revisada(&c.amb, &id, &OpcoesAprovacao::default()).unwrap();
    assert_eq!(aprovada.analise.estado, EstadoAnalise::Aprovada);
    assert!(aprovada.regras.is_empty());

    // Exportação: CSV com uma linha por cenário; Markdown com a tabela.
    let csv = revisao::exportar_analise(
        &c.amb,
        &id,
        &c.dir.path().join("cenarios"),
        revisao::Formato::Csv,
        &revisao::OpcoesExportacao::default(),
    )
    .unwrap();
    assert_eq!(csv.extension().unwrap(), "csv");
    let csv = std::fs::read_to_string(csv).unwrap();
    let linhas: Vec<&str> = csv.trim_end().lines().collect();
    assert_eq!(linhas.len(), 6, "{csv}");
    assert!(
        linhas[0]
            .trim_start_matches('\u{feff}')
            .starts_with("id,titulo,tecnica")
    );
    assert!(
        linhas[1].starts_with("CT-0001,31 dias sem supervisor é recusado,valor-limite,aceito,"),
        "{}",
        linhas[1]
    );
    assert!(
        linhas[1].contains(",30,dias desde a emissão da fatura,TD-1/R2,RN-FAT-014,"),
        "{}",
        linhas[1]
    );
    assert!(linhas[4].ends_with(",pergunta A-0001/1"), "{}", linhas[4]);
    let md = revisao::exportar_analise(
        &c.amb,
        &id,
        &c.dir.path().join("cenarios"),
        revisao::Formato::Markdown,
        &revisao::OpcoesExportacao::default(),
    )
    .unwrap();
    let md = std::fs::read_to_string(md).unwrap();
    assert!(
        md.contains("| **C1** fatura emitida há mais de 30 dias | S | S | N |"),
        "{md}"
    );
    let m = revisao::exportar_analise(
        &c.amb,
        &Id::analisar("A-0002").unwrap(),
        &c.dir.path().join("x"),
        revisao::Formato::Csv,
        &revisao::OpcoesExportacao::default(),
    );
    assert!(m.is_ok(), "{m:?}");
}

#[test]
fn cenarios_a_partir_de_historias_ligam_a_historia() {
    let c = caso();
    let origem = c
        .amb
        .analises
        .criar(NovaAnalise {
            fluxo: Fluxo::Historias,
            titulo: "Histórias do cancelamento".into(),
            entrada: "Histórias do cancelamento.".into(),
            analista: None,
            base: None,
        })
        .unwrap();
    assert_eq!(origem.id.to_string(), "A-0001");
    c.chamar(
        "registrar_historia",
        json!({
            "analise": "A-0001",
            "titulo": "Cancelar fatura antiga com supervisor",
            "como": "supervisor do faturamento",
            "quero": "cancelar fatura emitida há mais de 30 dias",
            "para": "corrigir cobranças antigas",
            "criterios": [{
                "dado": "uma fatura emitida há 31 dias",
                "quando": "o supervisor cancela",
                "entao": "a fatura é cancelada",
                "regras": ["RN-FAT-014"]
            }]
        }),
    )
    .unwrap();
    let r = c
        .chamar(
            "iniciar_analise",
            json!({ "fluxo": "cenarios", "origem": "A-0001" }),
        )
        .unwrap();
    assert_eq!(r["analise"], "A-0002");
    let a = c.analise("A-0002");
    assert_eq!(a.origem, Some(Serie::Analise.id(1)));
    assert!(
        a.entrada.contains("histórias da análise A-0001"),
        "{}",
        a.entrada
    );
    let mut v = cenario(
        "Supervisor cancela",
        "negativo",
        json!({ "historia": "HU-0001" }),
    );
    v["analise"] = json!("A-0002");
    let r = c.chamar("registrar_cenario", v).unwrap();
    assert_eq!(r["cenario"], "CT-0001");
    assert_eq!(
        c.analise("A-0002").cenarios[0]
            .historia
            .unwrap()
            .to_string(),
        "HU-0001"
    );
    // A origem não pode ser outra análise de cenários.
    let m = c.recusa(
        "iniciar_analise",
        json!({ "fluxo": "cenarios", "origem": "A-0002" }),
    );
    assert!(m.contains("já é de cenários"), "{m}");
    // CSV só para cenários.
    let m = revisao::exportar_analise(
        &c.amb,
        &Id::analisar("A-0001").unwrap(),
        &c.dir.path().join("x"),
        revisao::Formato::Csv,
        &revisao::OpcoesExportacao::default(),
    )
    .unwrap_err();
    assert!(m.to_string().contains("CSV"), "{m}");
}

// ---------------------------------------------------------------------------
// Documento montado, comparado com o esperado de `cba-docs` (que também confere a
// exportação para Markdown e Word dele).

fn evidencia(arquivo: &str, inicio: u32, fim: u32, trecho: &str) -> cba_core::analise::Evidencia {
    cba_core::analise::Evidencia {
        arquivo: CaminhoRelativo::novo(arquivo).unwrap(),
        linhas: Linhas::novo(inicio, fim).unwrap(),
        trecho: trecho.into(),
        hash: "5f2a9c1e".into(),
        commit: None,
        estado: cba_core::analise::EstadoEvidencia::Valida,
    }
}

fn regra(id: &str, titulo: &str, arquivo: &str, inicio: u32, fim: u32) -> Fundamento {
    Fundamento::Regra {
        id: Id::analisar(id).unwrap(),
        titulo: titulo.into(),
        evidencias: vec![Evidencia {
            arquivo: CaminhoRelativo::novo(arquivo).unwrap(),
            linhas: Linhas::novo(inicio, fim).unwrap(),
            hash: "7c3d0b2a".into(),
            commit: None,
        }],
    }
}

fn analise_de_exemplo() -> Analise {
    let mut a = Analise::nova(
        Serie::Analise.id(212),
        "faturamento-exemplo",
        Fluxo::Cenarios,
        "Cenários de teste do cancelamento de fatura",
        "Cenários de teste das regras da análise A-0193 (Cancelamento de fatura).",
        1,
    );
    a.origem = Some(Serie::Analise.id(193));
    let rn014 = regra(
        "RN-FAT-014",
        "Cancelamento após 30 dias da emissão exige supervisor",
        DELPHI,
        69,
        75,
    );
    let rn013 = regra(
        "RN-FAT-013",
        "Motivo do cancelamento com ao menos 10 caracteres",
        PKG,
        119,
        121,
    );
    let v = |s: &str| -> Vec<ValorCondicao> {
        s.split('/')
            .map(|x| ValorCondicao::analisar(x).unwrap())
            .collect()
    };
    a.registrar_tabela(
        NovaTabela {
            titulo: "Cancelamento de fatura emitida há mais de 30 dias".into(),
            condicoes: vec![
                Condicao {
                    texto: "fatura emitida há mais de 30 dias".into(),
                    evidencias: vec![evidencia(
                        DELPHI,
                        70,
                        70,
                        "(DiasDesdeEmissao(AIdFatura) > DIAS_LIMITE_SEM_SUPERVISOR)",
                    )],
                },
                Condicao {
                    texto: "usuário com perfil SUPERVISOR".into(),
                    evidencias: vec![evidencia(DELPHI, 71, 71, "(FPerfil <> PERFIL_SUPERVISOR)")],
                },
            ],
            acoes: vec![
                "segue para o cancelamento".into(),
                "recusa: \"Solicite a um supervisor\"".into(),
            ],
            colunas: vec![
                NovaColuna {
                    valores: v("S/S"),
                    acoes: vec![1],
                    impossivel: None,
                },
                NovaColuna {
                    valores: v("S/N"),
                    acoes: vec![2],
                    impossivel: None,
                },
                NovaColuna {
                    valores: v("N/-"),
                    acoes: vec![1],
                    impossivel: None,
                },
            ],
            fundamentos: vec![rn014.clone()],
            sessao: None,
        },
        2,
    )
    .unwrap();
    let texto = |titulo: &str, dado: &str, quando: &str, entao: &str| TextoCenario {
        titulo: titulo.into(),
        passos: Gherkin {
            dado: dado.into(),
            quando: quando.into(),
            entao: entao.into(),
        },
    };
    let limite_30 = Limite {
        grandeza: "dias desde a emissão da fatura".into(),
        valor: "30".into(),
        evidencias: vec![evidencia(
            DELPHI,
            41,
            41,
            "DIAS_LIMITE_SEM_SUPERVISOR = 30;",
        )],
        configuracao: None,
    };
    let novos = [
        NovoCenario {
            tecnica: Tecnica::TabelaDeDecisao,
            texto: texto(
                "Supervisor cancela fatura de 31 dias",
                "uma fatura ABERTA, sem pagamento, emitida há 31 dias",
                "um usuário com perfil SUPERVISOR pede o cancelamento",
                "a fatura é cancelada",
            ),
            valor: Some("31 dias, perfil SUPERVISOR".into()),
            limite: None,
            coluna: RefColuna::analisar("TD-1/R1"),
            historia: None,
            fundamentos: vec![rn014.clone()],
            pergunta: None,
            sessao: None,
        },
        NovoCenario {
            tecnica: Tecnica::ValorLimite,
            texto: texto(
                "31 dias sem supervisor é recusado",
                "uma fatura ABERTA, sem pagamento, emitida há 31 dias",
                "um atendente sem perfil SUPERVISOR pede o cancelamento",
                "o cancelamento é recusado: \"Solicite a um supervisor\"",
            ),
            valor: Some("31 dias".into()),
            limite: Some(limite_30.clone()),
            coluna: RefColuna::analisar("TD-1/R2"),
            historia: None,
            fundamentos: vec![rn014.clone()],
            pergunta: None,
            sessao: None,
        },
        NovoCenario {
            tecnica: Tecnica::ValorLimite,
            texto: texto(
                "30 dias sem supervisor cancela",
                "uma fatura ABERTA, sem pagamento, emitida há 30 dias",
                "um atendente sem perfil SUPERVISOR pede o cancelamento",
                "a fatura é cancelada",
            ),
            valor: Some("30 dias".into()),
            limite: Some(limite_30),
            coluna: RefColuna::analisar("TD-1/R3"),
            historia: None,
            fundamentos: vec![rn014],
            pergunta: None,
            sessao: None,
        },
        NovoCenario {
            tecnica: Tecnica::ValorLimite,
            texto: texto(
                "Motivo com 9 caracteres é recusado",
                "uma fatura ABERTA, sem pagamento",
                "o atendente cancela com o motivo \"duplicada\" (9 caracteres)",
                "o banco recusa com o erro -20012",
            ),
            valor: Some("9 caracteres".into()),
            limite: Some(Limite {
                grandeza: "tamanho do motivo, sem espaços nas pontas".into(),
                valor: "10".into(),
                evidencias: vec![evidencia(PKG, 119, 119, "LENGTH(TRIM(P_MOTIVO)) < 10")],
                configuracao: None,
            }),
            coluna: None,
            historia: None,
            fundamentos: vec![rn013],
            pergunta: Some(RefItem::analisar("A-0193/1").unwrap()),
            sessao: None,
        },
    ];
    for (n, novo) in novos.into_iter().enumerate() {
        a.registrar_cenario(novo, IdCenario::novo(n as u32 + 21), 3)
            .unwrap();
    }
    a.decidir_tabela(1, SituacaoItem::Aceito).unwrap();
    a.decidir_cenario(IdCenario::novo(21), SituacaoItem::Aceito)
        .unwrap();
    a
}

fn frontmatter() -> FrontmatterAnalise {
    FrontmatterAnalise {
        tipo: TipoDocumento::CenariosTeste,
        id: Serie::Analise.id(212),
        titulo: String::new(),
        projeto: "faturamento-exemplo".into(),
        demanda: Demanda {
            id: None,
            origem: "texto".into(),
            chave: None,
        },
        status: StatusAnalise::Rascunho,
        abrangencia: None,
        base: Some(VersaoCodigo {
            branch: Some("main".into()),
            commit: Some("1a2b3c4d".into()),
        }),
        criado: Data::nova(2026, 11, 14).unwrap(),
        analista: Some("Ana Exemplo".into()),
        origem: None,
    }
}

#[test]
fn documento_dos_cenarios_segue_o_template() {
    let a = analise_de_exemplo();
    let conteudo = cba_tools::documento::conteudo(&a, frontmatter());
    assert_eq!(conteudo.frontmatter.origem, Some(Serie::Analise.id(193)));
    let montado = Templates::padrao().montar(&conteudo).unwrap();
    assert!(montado.avisos.is_empty(), "{:?}", montado.avisos);
    let texto = montado.documento.para_texto().unwrap();
    let lido: Documento<FrontmatterAnalise> = Documento::analisar(&texto).unwrap();
    assert_eq!(lido.frontmatter, conteudo.frontmatter);

    let caminho =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../cba-docs/tests/esperados/cenarios-teste.md");
    if std::env::var_os("CBA_ATUALIZAR_ESPERADOS").is_some() {
        std::fs::write(&caminho, &texto).unwrap();
        return;
    }
    let esperado = std::fs::read_to_string(&caminho)
        .unwrap_or_else(|e| panic!("{}: {e}", caminho.display()))
        .replace("\r\n", "\n");
    assert!(
        texto == esperado,
        "o documento montado difere de {}\n--- montado ---\n{texto}",
        caminho.display()
    );
}

#[test]
fn analise_nova_do_fluxo_cenarios_pelo_core() {
    // A criação direta (sem iniciar_analise) também grava o tipo certo.
    let c = caso();
    let a = c
        .amb
        .analises
        .criar(NovaAnalise {
            fluxo: Fluxo::Cenarios,
            titulo: "Cenários".into(),
            entrada: "RN-FAT-013".into(),
            analista: None,
            base: None,
        })
        .unwrap();
    let doc: Documento<FrontmatterAnalise> = c.amb.kb.ler(&a.documento).unwrap();
    assert_eq!(doc.frontmatter.tipo, TipoDocumento::CenariosTeste);
}
