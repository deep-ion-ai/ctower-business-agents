//! Fluxo "Histórias e critérios" (#115) com o sistema fictício "Faturamento
//! Exemplo": a partir de uma análise de impacto, `iniciar_analise` cria a análise
//! de histórias ligada à origem; `registrar_historia` e `registrar_criterio`
//! gravam histórias com identificador estável (HU-NNNN, sequencial no projeto) e
//! critérios ligados às afirmações e às regras; critério sem origem, ou com origem
//! que não existe, é recusado; o documento segue o template, entra no índice e na
//! busca, e a revisão e a aprovação passam pelas decisões do analista.

use std::path::{Path, PathBuf};

use cba_core::ambiente::Ambiente;
use cba_core::analise::{Analise, EstadoAfirmacao, EstadoAnalise, Fluxo};
use cba_core::analises::NovaAnalise;
use cba_core::config::Config;
use cba_core::dados::DiretorioDados;
use cba_core::historias::{
    DecisaoHistoria, Gherkin, Narrativa, NovaHistoria, NovoCriterio, SituacaoHistoria,
};
use cba_core::requisito::{Fundamento, RefItem, SituacaoItem};
use cba_docs::{Templates, Texto};
use cba_kb::frontmatter::{
    Data, Demanda, Evidencia, FrontmatterAnalise, FrontmatterLivre, FrontmatterRegra, Linhas,
    StatusAnalise, StatusRegra, TipoDocumento, VersaoCodigo,
};
use cba_kb::{CaminhoRelativo, Documento, Id, IdDemanda, IdHistoria, Serie};
use cba_tools::aprovacao::OpcoesAprovacao;
use cba_tools::revisao::{self, Decisao};
use cba_tools::{ErroChamada, ErroFerramenta, registro_padrao};
use serde_json::{Value, json};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/faturamento-exemplo")
}

struct Cenario {
    _dir: tempfile::TempDir,
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
    Cenario { _dir: dir, amb }
}

impl Cenario {
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

    /// Análise de impacto A-0001 (D-FAT-003, parcelamento para pessoa física) com
    /// três afirmações (a 3 marcada como incorreta) e uma pergunta em aberto, e a
    /// regra RN-FAT-008 na base.
    fn origem(&self) -> Analise {
        let a = self
            .amb
            .analises
            .criar(NovaAnalise {
                fluxo: Fluxo::Impacto,
                titulo: "Parcelamento para pessoa física".into(),
                entrada: "Permitir que pessoa física parcele faturas em até 3 vezes, com \
                          parcela mínima de R$ 100,00."
                    .into(),
                analista: Some("Ana Exemplo".into()),
                base: None,
            })
            .unwrap();
        let afirmar = |texto: &str, arquivo: &str, inicio: u32, fim: u32, trecho: &str| {
            self.chamar(
                "afirmar",
                json!({
                    "analise": a.id,
                    "secao": "como_e_hoje",
                    "texto": texto,
                    "confianca": "confirmado",
                    "evidencias": [{
                        "arquivo": arquivo,
                        "linha_inicial": inicio,
                        "linha_final": fim,
                        "trecho": trecho
                    }]
                }),
            )
            .unwrap();
        };
        afirmar(
            "Pessoa física não pode parcelar fatura: o backend recusa o pedido.",
            "backend/src/main/java/exemplo/faturamento/ParcelamentoService.java",
            30,
            32,
            "if (fatura.tipoCliente() == TipoCliente.PF) {",
        );
        afirmar(
            "O valor mínimo da parcela vem do parâmetro VALOR_MIN_PARCELA (hoje R$ 50,00).",
            "db/02_parametros.sql",
            18,
            19,
            "'VALOR_MIN_PARCELA', 50.0000",
        );
        afirmar(
            "O parcelamento aceita até 12 parcelas.",
            "db/02_parametros.sql",
            15,
            16,
            "'MAX_PARCELAS', 6.0000",
        );
        self.chamar(
            "perguntar",
            json!({
                "analise": a.id,
                "texto": "A parcela mínima de R$ 100,00 vale só para pessoa física?"
            }),
        )
        .unwrap();
        self.amb
            .analises
            .alterar(&a.id, cba_core::eventos::Mudanca::Afirmacao, |an, _| {
                an.marcar_afirmacao(1, EstadoAfirmacao::Confirmada)?;
                an.marcar_afirmacao(
                    3,
                    EstadoAfirmacao::Incorreta {
                        comentario: "o limite é 6".into(),
                    },
                )
            })
            .unwrap();
        // A regra RN-FAT-008, como a aprovação de outra análise a deixaria.
        let regra = Id::analisar("RN-FAT-008").unwrap();
        self.amb
            .kb
            .gravar(
                &regra.arquivo(),
                &Documento::novo(
                    FrontmatterRegra {
                        tipo: TipoDocumento::Regra,
                        id: regra.clone(),
                        titulo: "Quantidade de parcelas entre 2 e MAX_PARCELAS".into(),
                        area: "parcelamento".into(),
                        status: StatusRegra::Confirmada,
                        evidencias: vec![Evidencia {
                            arquivo: CaminhoRelativo::novo("db/02_parametros.sql").unwrap(),
                            linhas: Linhas::novo(15, 16).unwrap(),
                            hash: "h".into(),
                            commit: None,
                        }],
                        origem: None,
                    },
                    "\n# RN-FAT-008\n",
                ),
            )
            .unwrap();
        self.analise(&a.id.to_string())
    }
}

fn criterio(afirmacoes: Value, regras: Value) -> Value {
    json!({
        "dado": "uma pessoa física com uma fatura em aberto de R$ 300,00",
        "quando": "ela pede o parcelamento em 3 vezes",
        "entao": "são geradas 3 parcelas de R$ 100,00",
        "afirmacoes": afirmacoes,
        "regras": regras
    })
}

fn historia(analise: &str, criterios: Value) -> Value {
    json!({
        "analise": analise,
        "titulo": "Parcelar fatura de pessoa física em até 3 vezes",
        "como": "atendente do faturamento",
        "quero": "parcelar a fatura em aberto de um cliente pessoa física",
        "para": "reduzir a inadimplência",
        "criterios": criterios
    })
}

#[test]
fn historias_a_partir_de_uma_analise_de_impacto() {
    let c = cenario();
    let origem = c.origem();

    // iniciar_analise cria a análise de histórias ligada à origem.
    let r = c
        .chamar(
            "iniciar_analise",
            json!({ "fluxo": "historias", "origem": origem.id }),
        )
        .unwrap();
    assert_eq!(r["analise"], "A-0002");
    assert_eq!(r["fluxo"], "historias");
    assert_eq!(r["origem"], "A-0001");
    assert_eq!(r["contexto"]["origem"]["afirmacoes"], 3);
    assert!(
        r["roteiro"]
            .as_str()
            .unwrap()
            .contains("### Análise de origem"),
        "{}",
        r["roteiro"]
    );
    // A origem ainda não foi aprovada: aviso.
    assert!(r["avisos"][0].as_str().unwrap().contains("A-0001"));
    let a = c.analise("A-0002");
    assert_eq!(a.origem, Some(origem.id.clone()));
    assert_eq!(a.fluxo, Fluxo::Historias);
    let doc: Documento<FrontmatterAnalise> = c.amb.kb.ler(&a.documento).unwrap();
    assert_eq!(doc.frontmatter.tipo, TipoDocumento::Historias);
    assert_eq!(doc.frontmatter.origem, Some(origem.id.clone()));

    // Histórias com critérios ligados à afirmação e à regra.
    let r = c
        .chamar(
            "registrar_historia",
            historia(
                "A-0002",
                json!([
                    criterio(json!([1]), json!(["RN-FAT-008"])),
                    {
                        "dado": "uma pessoa física com uma fatura em aberto de R$ 250,00",
                        "quando": "ela pede 3 parcelas",
                        "entao": "o pedido é recusado: a parcela ficaria abaixo de R$ 100,00",
                        "afirmacoes": ["A-0001/2"],
                        "pergunta": 1
                    }
                ]),
            ),
        )
        .unwrap();
    assert_eq!(r["historia"], "HU-0001");
    assert_eq!(r["criterios"], json!(["CA-1", "CA-2"]));
    assert_eq!(r["a_confirmar"], json!(["CA-2"]));
    assert_eq!(r["documento"], "faturamento-exemplo/analises/A-0002.md");
    let r = c
        .chamar(
            "registrar_criterio",
            json!({
                "analise": "A-0002",
                "historia": "HU-0001",
                "dado": "uma pessoa jurídica com uma fatura em aberto",
                "quando": "ela pede parcelamento",
                "entao": "o parcelamento segue a regra atual",
                "regras": ["RN-FAT-008"]
            }),
        )
        .unwrap();
    assert_eq!(r["criterio"], "CA-3");
    // Sem número no critério: aviso, sem recusa.
    assert!(r["aviso_do_criterio"].as_str().unwrap().contains("CA-3"));

    let a = c.analise("A-0002");
    let h = a.historia(IdHistoria::novo(1)).unwrap();
    assert_eq!(h.criterios.len(), 3);
    match &h.criterios[0].fundamentos[0] {
        Fundamento::Afirmacao {
            referencia,
            texto,
            evidencias,
        } => {
            assert_eq!(referencia.to_string(), "A-0001/1");
            assert!(texto.starts_with("Pessoa física não pode parcelar"));
            assert_eq!(evidencias.len(), 1);
        }
        outro => panic!("{outro:?}"),
    }
    assert_eq!(
        h.criterios[1].pergunta,
        Some(RefItem::analisar("A-0001/1").unwrap())
    );

    // Recusas: nada é gravado.
    let sem_origem = c.recusa(
        "registrar_historia",
        historia("A-0002", json!([criterio(json!([]), json!([]))])),
    );
    assert!(sem_origem.contains("ao menos uma origem"), "{sem_origem}");
    let m = c.recusa(
        "registrar_criterio",
        json!({
            "analise": "A-0002", "historia": "HU-0001",
            "dado": "x 1", "quando": "y", "entao": "z"
        }),
    );
    assert!(m.contains("ao menos uma origem"), "{m}");
    let m = c.recusa(
        "registrar_historia",
        historia("A-0002", json!([criterio(json!([99]), json!([]))])),
    );
    assert!(m.contains("A-0001/99 não existe"), "{m}");
    let m = c.recusa(
        "registrar_historia",
        historia(
            "A-0002",
            json!([criterio(json!([]), json!(["RN-FAT-999"]))]),
        ),
    );
    assert!(m.contains("RN-FAT-999 não existe"), "{m}");
    let m = c.recusa(
        "registrar_historia",
        historia("A-0002", json!([criterio(json!([3]), json!([]))])),
    );
    assert!(m.contains("incorreta"), "{m}");
    let m = c.recusa(
        "registrar_historia",
        historia("A-0002", json!([criterio(json!(["A-0009/1"]), json!([]))])),
    );
    assert!(m.contains("A-0009"), "{m}");
    let m = c.recusa(
        "registrar_historia",
        historia("A-0001", json!([criterio(json!([1]), json!([]))])),
    );
    assert!(m.contains("não é do fluxo `historias`"), "{m}");
    let m = c.recusa(
        "afirmar",
        json!({ "analise": "A-0002", "secao": "historias", "texto": "x", "confianca": "inferido" }),
    );
    assert!(m.contains("registrar_historia"), "{m}");
    assert_eq!(c.analise("A-0002").historias.len(), 1);

    // O identificador é sequencial no projeto, não na análise.
    c.chamar(
        "iniciar_analise",
        json!({ "fluxo": "historias", "origem": "A-0001", "titulo": "Outras histórias" }),
    )
    .unwrap();
    // Sem histórias, não conclui.
    let m = c.recusa("concluir_analise", json!({ "analise": "A-0003" }));
    assert!(m.contains("ainda não tem histórias"), "{m}");
    let r = c
        .chamar(
            "registrar_historia",
            historia("A-0003", json!([criterio(json!([1]), json!([]))])),
        )
        .unwrap();
    assert_eq!(r["historia"], "HU-0002");

    // retomar_analise mostra as histórias e a origem.
    let t = c.texto("retomar_analise", json!({ "analise": "A-0002" }));
    assert!(t.contains("Análise de origem: A-0001"), "{t}");
    assert!(
        t.contains("### HU-0001 · Parcelar fatura de pessoa física"),
        "{t}"
    );
    assert!(
        t.contains("CA-2 [sem decisão; a confirmar: pergunta A-0001/1]"),
        "{t}"
    );

    // Documento: segue o template, frontmatter válido, histórias, critérios e
    // ligações.
    let texto = std::fs::read_to_string(c.amb.kb.caminho(&a.documento)).unwrap();
    let livre: Documento<FrontmatterLivre> = Documento::analisar(&texto).unwrap();
    assert_eq!(livre.frontmatter.tipo(), Some(TipoDocumento::Historias));
    let titulos: Vec<&str> = texto.lines().filter(|l| l.starts_with("## ")).collect();
    assert_eq!(
        titulos,
        [
            "## Pedido",
            "## Histórias",
            "## Rastreabilidade",
            "## Perguntas em aberto",
            "## Evidências"
        ]
    );
    assert!(texto.contains("### HU-0001 · Parcelar fatura de pessoa física em até 3 vezes"));
    assert!(texto.contains("- **Dado** uma pessoa física com uma fatura em aberto de R$ 300,00"));
    assert!(
        texto.contains("[A-0001 (afirmação 1)](A-0001.md)"),
        "{texto}"
    );
    assert!(
        texto.contains("[RN-FAT-008](../regras/RN-FAT-008.md)"),
        "{texto}"
    );
    assert!(texto.contains("| HU-0001 | CA-2 | A-0001/2 | a confirmar (A-0001/1) |"));
    assert!(texto.contains("Origem: [A-0001](A-0001.md)"), "{texto}");

    // concluir_analise confere as histórias.
    let r = c
        .chamar("concluir_analise", json!({ "analise": "A-0002" }))
        .unwrap();
    assert_eq!(r["estado"], "concluida");
    assert_eq!(r["registros"]["historias"], 1);

    // Índice e busca.
    let r = c
        .chamar(
            "conhecimento",
            json!({ "termo": "parcelar pessoa física", "tipos": ["analise"] }),
        )
        .unwrap();
    assert!(
        r.to_string()
            .contains("faturamento-exemplo/analises/A-0002.md"),
        "{r}"
    );

    // Revisão: decisões, edição e aprovação.
    let id = Id::analisar("A-0002").unwrap();
    let p = revisao::prontidao(&c.amb, &id).unwrap();
    assert!(!p.pode_aprovar);
    assert!(
        p.pendencias
            .iter()
            .any(|x| x.motivo.contains("HU-0001: sem decisão")),
        "{p:?}"
    );
    let decidir = |d: Value| {
        revisao::revisar(&c.amb, &id, serde_json::from_value::<Decisao>(d).unwrap()).unwrap()
    };
    decidir(
        json!({ "tipo": "criterio", "historia": "HU-0001", "numero": 3, "situacao": "descartado" }),
    );
    decidir(json!({
        "tipo": "editar-criterio", "historia": "HU-0001", "numero": 1,
        "dado": "uma pessoa física com uma fatura em aberto de R$ 330,00",
        "quando": "ela pede o parcelamento em 3 vezes",
        "entao": "são geradas 3 parcelas de R$ 110,00"
    }));
    decidir(json!({
        "tipo": "editar-historia", "historia": "HU-0001",
        "titulo": "Parcelar fatura de pessoa física",
        "como": "atendente do faturamento",
        "quero": "parcelar a fatura em aberto de um cliente pessoa física",
        "para": "reduzir a inadimplência"
    }));
    let a = decidir(json!({ "tipo": "historia", "historia": "HU-0001", "decisao": "aceitar" }));
    let h = a.historia(IdHistoria::novo(1)).unwrap();
    assert_eq!(h.situacao, SituacaoHistoria::Aceita);
    assert_eq!(h.criterios[0].situacao, SituacaoItem::Aceito);
    assert_eq!(h.criterios[2].situacao, SituacaoItem::Descartado);
    assert!(h.criterios[0].original.is_some());
    let p = revisao::prontidao(&c.amb, &id).unwrap();
    assert!(p.pode_aprovar, "{:?}", p.pendencias);
    assert!(p.avisos.iter().any(|a| a.contains("a confirmar")), "{p:?}");
    let aprovada = revisao::aprovar_revisada(&c.amb, &id, &OpcoesAprovacao::default()).unwrap();
    assert_eq!(aprovada.analise.estado, EstadoAnalise::Aprovada);
    assert!(aprovada.regras.is_empty());
    let texto = std::fs::read_to_string(c.amb.kb.caminho(&a.documento)).unwrap();
    assert!(texto.contains("status: aprovada"), "{texto}");
    assert!(texto.contains("R$ 330,00"));
    assert!(
        !texto.contains("a regra atual"),
        "critério descartado fica fora"
    );

    // A exportação preserva os critérios e as ligações.
    let saida = c._dir.path().join("historias");
    let arquivo = revisao::exportar_analise(
        &c.amb,
        &id,
        &saida,
        revisao::Formato::Markdown,
        &revisao::OpcoesExportacao::default(),
    )
    .unwrap();
    let exportado = std::fs::read_to_string(arquivo).unwrap();
    assert!(exportado.contains("**Dado** uma pessoa física com uma fatura em aberto de R$ 330,00"));
    assert!(exportado.contains("RN-FAT-008"));
    assert!(exportado.contains("A-0001 (afirmação 1)"));
}

#[test]
fn historias_a_partir_de_uma_demanda_ou_sem_origem_recusam_numero_solto() {
    let c = cenario();
    let m = c.recusa("iniciar_analise", json!({ "fluxo": "historias" }));
    assert!(m.contains("`origem`"), "{m}");
    let m = c.recusa(
        "iniciar_analise",
        json!({ "fluxo": "impacto", "entrada": "x", "origem": "A-0001" }),
    );
    assert!(m.contains("só vale"), "{m}");
    let m = c.recusa(
        "iniciar_analise",
        json!({ "fluxo": "historias", "origem": "A-0042" }),
    );
    assert!(m.contains("A-0042"), "{m}");
}

// ---------------------------------------------------------------------------
// Documento montado, comparado com o esperado de `cba-docs` (que também confere a
// exportação para Markdown e Word dele).

fn evidencia(arquivo: &str, inicio: u32, fim: u32) -> Evidencia {
    Evidencia {
        arquivo: CaminhoRelativo::novo(arquivo).unwrap(),
        linhas: Linhas::novo(inicio, fim).unwrap(),
        hash: "9f2c1e7a".into(),
        commit: None,
    }
}

fn analise_de_exemplo() -> Analise {
    let mut a = Analise::nova(
        Serie::Analise.id(204),
        "faturamento-exemplo",
        Fluxo::Historias,
        "Histórias do parcelamento para pessoa física",
        "Histórias de usuário com critérios de aceite a partir da análise A-0193 \
         (Parcelamento para pessoa física).",
        1,
    );
    a.origem = Some(Serie::Analise.id(193));
    let afirmacao = |n: u32, texto: &str, e: Evidencia| Fundamento::Afirmacao {
        referencia: RefItem::analisar(&format!("A-0193/{n}")).unwrap(),
        texto: texto.into(),
        evidencias: vec![e],
    };
    let regra = |id: &str, titulo: &str, e: Evidencia| Fundamento::Regra {
        id: Id::analisar(id).unwrap(),
        titulo: titulo.into(),
        evidencias: vec![e],
    };
    let pf = afirmacao(
        4,
        "Pessoa física não pode parcelar fatura: o backend recusa o pedido.",
        evidencia(
            "backend/src/main/java/exemplo/faturamento/ParcelamentoService.java",
            30,
            32,
        ),
    );
    let minimo = regra(
        "RN-FAT-009",
        "Valor mínimo da parcela, exceto órgão público",
        evidencia("db/02_parametros.sql", 18, 19),
    );
    let quantidade = regra(
        "RN-FAT-008",
        "Quantidade de parcelas entre 2 e MAX_PARCELAS",
        evidencia("db/02_parametros.sql", 15, 16),
    );
    let ca =
        |dado: &str, quando: &str, entao: &str, f: Vec<Fundamento>, p: Option<&str>| NovoCriterio {
            passos: Gherkin {
                dado: dado.into(),
                quando: quando.into(),
                entao: entao.into(),
            },
            fundamentos: f,
            pergunta: p.map(|p| RefItem::analisar(p).unwrap()),
            sessao: None,
        };
    a.registrar_historia(
        NovaHistoria {
            narrativa: Narrativa {
                titulo: "Parcelar fatura de pessoa física em até 3 vezes".into(),
                como: "atendente do faturamento".into(),
                quero: "parcelar a fatura em aberto de um cliente pessoa física em 2 ou 3 vezes"
                    .into(),
                para: "reduzir a inadimplência das faturas de pessoa física".into(),
            },
            criterios: vec![
                ca(
                    "uma pessoa física com uma fatura em aberto de R$ 300,00",
                    "ela pede o parcelamento em 3 vezes",
                    "são geradas 3 parcelas de R$ 100,00, com vencimento a cada 30 dias",
                    vec![pf.clone(), quantidade.clone()],
                    None,
                ),
                ca(
                    "uma pessoa física com uma fatura em aberto de R$ 250,00",
                    "ela pede 3 parcelas",
                    "o pedido é recusado: a parcela ficaria abaixo de R$ 100,00",
                    vec![minimo.clone()],
                    Some("A-0193/2"),
                ),
            ],
            invest: vec![],
            quebra: None,
            fatia_de: None,
            sessao: None,
        },
        IdHistoria::novo(1),
        2,
    )
    .unwrap();
    a.registrar_historia(
        NovaHistoria {
            narrativa: Narrativa {
                titulo: "Mostrar o parcelamento de pessoa física no relatório de recebíveis".into(),
                como: "gestor financeiro".into(),
                quero: "ver as parcelas de pessoa física no relatório de recebíveis".into(),
                para: "prever o caixa do mês".into(),
            },
            criterios: vec![ca(
                "uma fatura de pessoa física de R$ 300,00 parcelada em 3 vezes",
                "o relatório de recebíveis de março é emitido",
                "aparecem 3 recebíveis de R$ 100,00, um em cada mês",
                vec![pf],
                None,
            )],
            invest: vec![cba_core::historias::AlertaInvest {
                criterio: cba_core::qualidade::CriterioInvest::Pequena,
                motivo: "junta a mudança do relatório com a da tela; não cabe numa entrega".into(),
            }],
            quebra: Some(
                "Separar em duas fatias: o relatório com as parcelas e o filtro por tipo de \
                 cliente, que depende da primeira."
                    .into(),
            ),
            fatia_de: None,
            sessao: None,
        },
        IdHistoria::novo(2),
        3,
    )
    .unwrap();
    a.registrar_pergunta(
        "O relatório de recebíveis deve separar as parcelas por tipo de cliente?",
        None,
        None,
        4,
    )
    .unwrap();
    a.decidir_historia(IdHistoria::novo(1), DecisaoHistoria::Aceitar)
        .unwrap();
    a
}

fn frontmatter() -> FrontmatterAnalise {
    FrontmatterAnalise {
        tipo: TipoDocumento::Historias,
        id: Serie::Analise.id(204),
        titulo: String::new(),
        projeto: "faturamento-exemplo".into(),
        demanda: Demanda {
            id: Some(IdDemanda::novo(3)),
            origem: "texto".into(),
            chave: None,
        },
        status: StatusAnalise::Rascunho,
        abrangencia: None,
        base: Some(VersaoCodigo {
            branch: Some("main".into()),
            commit: Some("1a2b3c4d".into()),
        }),
        criado: Data::nova(2026, 11, 13).unwrap(),
        analista: Some("Ana Exemplo".into()),
        origem: None,
    }
}

#[test]
fn documento_das_historias_segue_o_template() {
    let a = analise_de_exemplo();
    let conteudo = cba_tools::documento::conteudo(&a, frontmatter());
    assert_eq!(conteudo.frontmatter.origem, Some(Serie::Analise.id(193)));
    assert!(
        conteudo
            .textos
            .iter()
            .any(|t: &Texto| t.secao == "pedido" && t.texto.contains("A-0193"))
    );
    let montado = Templates::padrao().montar(&conteudo).unwrap();
    assert!(montado.avisos.is_empty(), "{:?}", montado.avisos);
    let texto = montado.documento.para_texto().unwrap();
    let lido: Documento<FrontmatterAnalise> = Documento::analisar(&texto).unwrap();
    assert_eq!(lido.frontmatter, conteudo.frontmatter);

    let caminho =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../cba-docs/tests/esperados/historias.md");
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
