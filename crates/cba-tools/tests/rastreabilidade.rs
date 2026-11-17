//! Matriz de rastreabilidade e consulta reversa (#121) com o sistema fictício
//! "Faturamento Exemplo".
//!
//! A demanda D-0001 (parcelamento para pessoa física) passa pelo caminho completo
//! com as ferramentas de verdade: a análise de impacto A-0001 registra três
//! afirmações com evidência no `ParcelamentoService.java`; a análise de histórias
//! A-0002, derivada dela, registra HU-0001 (aceita, ligada à afirmação 1 e à regra
//! RN-FAT-007) e HU-0002 (ligada à afirmação 2, sem cenário); a análise de
//! cenários A-0003 registra CT-0001 para HU-0001. A afirmação 3 toca o trecho da
//! RN-FAT-009, que não entra em história nenhuma. Depois, o código da afirmação 1
//! "muda" (a revalidação marca a evidência), e a HU-0001, aprovada, passa a ter
//! evidência a revalidar.
//!
//! Confere: a matriz completa, as três lacunas, a consulta reversa por regra e
//! por arquivo, a ferramenta MCP `rastreabilidade`, a exportação (.xlsx e Word) e
//! que apagar e reconstruir o índice recupera a mesma matriz.

use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use cba_core::ambiente::Ambiente;
use cba_core::config::Config;
use cba_core::dados::DiretorioDados;
use cba_core::eventos::Mudanca;
use cba_core::historias::DecisaoHistoria;
use cba_kb::frontmatter::{Data, Evidencia, FrontmatterRegra, Linhas, StatusRegra, TipoDocumento};
use cba_kb::indice::{Alerta, AlvoReverso, Indice, Matriz};
use cba_kb::{CaminhoRelativo, Documento, Id, IdDemanda, IdHistoria, NovaDemanda};
use cba_tools::rastreabilidade::{self, FormatoMatriz};
use cba_tools::{ErroChamada, ErroFerramenta, registro_padrao};
use serde_json::{Value, json};

const SERVICO: &str = "backend/src/main/java/exemplo/faturamento/ParcelamentoService.java";

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
    _dir: tempfile::TempDir,
    amb: Ambiente,
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

    fn regra(&self, id: &str, titulo: &str, inicio: u32, fim: u32) {
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
                        area: "parcelamento".into(),
                        status: StatusRegra::Confirmada,
                        evidencias: vec![Evidencia {
                            arquivo: CaminhoRelativo::novo(SERVICO).unwrap(),
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

    fn afirmar(&self, texto: &str, inicio: u32, fim: u32) {
        self.chamar(
            "afirmar",
            json!({
                "analise": "A-0001",
                "secao": "como_e_hoje",
                "texto": texto,
                "confianca": "confirmado",
                "evidencias": [{
                    "arquivo": SERVICO,
                    "linha_inicial": inicio,
                    "linha_final": fim,
                    "trecho": linha(SERVICO, inicio as usize).trim()
                }]
            }),
        )
        .unwrap();
    }

    fn indice(&self) -> Indice {
        let mut indice = Indice::abrir(self.amb.dados.arquivo_do_indice(), &self.amb.base).unwrap();
        indice.sincronizar().unwrap();
        indice
    }

    fn matriz(&self) -> Matriz {
        self.indice()
            .matriz(self.amb.kb.nome(), IdDemanda::novo(1))
            .unwrap()
    }
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
    let c = Caso { _dir: dir, amb };

    // Regras confirmadas, com as evidências do gabarito.
    c.regra(
        "RN-FAT-007",
        "Parcelamento só para pessoa jurídica e órgão público",
        30,
        32,
    );
    c.regra(
        "RN-FAT-008",
        "Quantidade de parcelas entre 2 e MAX_PARCELAS",
        34,
        35,
    );
    c.regra(
        "RN-FAT-009",
        "Valor mínimo da parcela, exceto órgão público",
        47,
        53,
    );

    // A demanda e a análise de impacto ligada a ela.
    c.amb
        .kb
        .criar_demanda(
            &NovaDemanda {
                titulo: "Parcelamento para pessoa física".into(),
                descricao: Some(
                    "Permitir que pessoa física parcele faturas em até 3 vezes.".into(),
                ),
                chave: Some("FAT-470".into()),
                origem: None,
            },
            Data::nova(2026, 11, 13).unwrap(),
        )
        .unwrap();
    let r = c
        .chamar(
            "iniciar_analise",
            json!({
                "fluxo": "impacto",
                "demanda": "D-0001",
                "titulo": "Parcelamento para pessoa física",
                "entrada": "Permitir que pessoa física parcele faturas em até 3 vezes."
            }),
        )
        .unwrap();
    assert_eq!(r["analise"], "A-0001");
    c.afirmar(
        "Pessoa física não pode parcelar fatura (RN-FAT-007): o backend recusa o pedido.",
        30,
        32,
    );
    c.afirmar(
        "A quantidade de parcelas fica entre 2 e MAX_PARCELAS.",
        34,
        35,
    );
    c.afirmar(
        "O valor de cada parcela não pode ficar abaixo do mínimo, exceto para órgão público.",
        47,
        53,
    );

    // Histórias derivadas do impacto (herdam a demanda).
    let r = c
        .chamar(
            "iniciar_analise",
            json!({ "fluxo": "historias", "origem": "A-0001" }),
        )
        .unwrap();
    assert_eq!(r["analise"], "A-0002");
    for (titulo, afirmacao, regras) in [
        ("Parcelar fatura de pessoa física", 1, json!(["RN-FAT-007"])),
        ("Limitar a 3 parcelas para pessoa física", 2, json!([])),
    ] {
        c.chamar(
            "registrar_historia",
            json!({
                "analise": "A-0002",
                "titulo": titulo,
                "como": "atendente do faturamento",
                "quero": "parcelar a fatura de um cliente pessoa física",
                "para": "reduzir a inadimplência",
                "criterios": [{
                    "dado": "uma pessoa física com uma fatura em aberto de R$ 300,00",
                    "quando": "ela pede o parcelamento em 3 vezes",
                    "entao": "são geradas 3 parcelas de R$ 100,00",
                    "afirmacoes": [afirmacao],
                    "regras": regras
                }]
            }),
        )
        .unwrap();
    }
    c.amb
        .analises
        .alterar(
            &Id::analisar("A-0002").unwrap(),
            Mudanca::Historia,
            |a, _| a.decidir_historia(IdHistoria::novo(1), DecisaoHistoria::Aceitar),
        )
        .unwrap();

    // Cenário de teste da HU-0001.
    let r = c
        .chamar(
            "iniciar_analise",
            json!({ "fluxo": "cenarios", "origem": "A-0002" }),
        )
        .unwrap();
    assert_eq!(r["analise"], "A-0003");
    let r = c
        .chamar(
            "registrar_cenario",
            json!({
                "analise": "A-0003",
                "titulo": "Pessoa física pede parcelamento",
                "tecnica": "negativo",
                "dado": "uma fatura em aberto de um cliente pessoa física",
                "quando": "o atendente pede o parcelamento",
                "entao": "o pedido é recusado",
                "historia": "HU-0001",
                "regras": ["RN-FAT-007"]
            }),
        )
        .unwrap();
    assert_eq!(r["cenario"], "CT-0001");
    c
}

#[test]
fn matriz_completa_lacunas_e_reconstrucao() {
    let c = caso();
    let m = c.matriz();
    assert_eq!(m.demanda.id, "D-0001");
    assert_eq!(m.demanda.titulo, "Parcelamento para pessoa física");
    assert_eq!(m.demanda.status.as_deref(), Some("aberta"));
    let analises: Vec<&str> = m.analises.iter().map(|a| a.id.as_str()).collect();
    assert_eq!(analises, ["A-0001", "A-0002", "A-0003"]);
    assert_eq!(m.analises[1].fluxo.as_deref(), Some("historias"));
    assert_eq!(m.analises[1].demanda.as_deref(), Some("D-0001"));

    // Por história: análise → regras → evidências → cenários.
    let ids: Vec<&str> = m.historias.iter().map(|h| h.id.as_str()).collect();
    assert_eq!(ids, ["HU-0001", "HU-0002"]);
    let hu1 = &m.historias[0];
    assert!(hu1.aprovada);
    assert_eq!(hu1.analises, ["A-0001", "A-0002"]);
    assert_eq!(hu1.afirmacoes, ["A-0001/1"]);
    let regras: Vec<&str> = hu1.regras.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(regras, ["RN-FAT-007"]);
    assert_eq!(hu1.criterios.len(), 1);
    assert_eq!(hu1.criterios[0].criterio, "CA-1");
    assert_eq!(
        hu1.criterios[0].texto,
        "são geradas 3 parcelas de R$ 100,00"
    );
    assert_eq!(hu1.evidencias.len(), 1, "{:?}", hu1.evidencias);
    assert_eq!(hu1.evidencias[0].arquivo, SERVICO);
    assert_eq!(
        (
            hu1.evidencias[0].linha_inicial,
            hu1.evidencias[0].linha_final
        ),
        (30, 32)
    );
    assert!(!hu1.evidencias[0].a_revalidar);
    let cenarios: Vec<&str> = hu1.cenarios.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(cenarios, ["CT-0001"]);
    assert!(hu1.alertas.is_empty(), "{:?}", hu1.alertas);

    let hu2 = &m.historias[1];
    assert!(!hu2.aprovada);
    let regras: Vec<&str> = hu2.regras.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(regras, ["RN-FAT-008"], "pela evidência da afirmação 2");
    assert_eq!(hu2.alertas, [Alerta::HistoriaSemCenario]);

    // Por regra: as três afetadas pelo impacto; a RN-FAT-009 sem história.
    let regras: Vec<(&str, &[String])> = m
        .regras
        .iter()
        .map(|r| (r.regra.id.as_str(), r.historias.as_slice()))
        .collect();
    assert_eq!(
        regras,
        [
            ("RN-FAT-007", &["HU-0001".to_owned()][..]),
            ("RN-FAT-008", &["HU-0002".to_owned()][..]),
            ("RN-FAT-009", &[][..]),
        ]
    );
    assert_eq!(m.regras[0].analises, ["A-0001", "A-0002", "A-0003"]);
    assert_eq!(m.regras[0].cenarios[0].id, "CT-0001");
    assert_eq!(m.regras[2].alertas, [Alerta::RegraSemHistoria]);
    assert_eq!(m.regras[2].analises, ["A-0001"]);

    let lacunas: Vec<(Alerta, &str)> = m
        .lacunas
        .iter()
        .map(|l| (l.alerta, l.alvo.as_str()))
        .collect();
    assert_eq!(
        lacunas,
        [
            (Alerta::RegraSemHistoria, "RN-FAT-009"),
            (Alerta::HistoriaSemCenario, "HU-0002"),
        ]
    );

    // O código da afirmação 1 muda: a revalidação marca a evidência, e a HU-0001,
    // aprovada, passa a ter evidência a revalidar.
    c.amb
        .analises
        .alterar(
            &Id::analisar("A-0001").unwrap(),
            Mudanca::Afirmacao,
            |a, _| Ok(a.marcar_a_revalidar(|e| e.linhas.inicio != 30)),
        )
        .unwrap();
    let m = c.matriz();
    let hu1 = &m.historias[0];
    assert!(hu1.evidencias[0].a_revalidar);
    assert!(hu1.criterios[0].a_revalidar);
    assert_eq!(hu1.alertas, [Alerta::EvidenciaARevalidar]);
    let lacunas: Vec<(Alerta, &str)> = m
        .lacunas
        .iter()
        .map(|l| (l.alerta, l.alvo.as_str()))
        .collect();
    assert_eq!(
        lacunas,
        [
            (Alerta::EvidenciaARevalidar, "HU-0001"),
            (Alerta::RegraSemHistoria, "RN-FAT-009"),
            (Alerta::HistoriaSemCenario, "HU-0002"),
        ]
    );
    let descricao = &m.lacunas[0].descricao;
    assert!(descricao.contains("A-0001/1"), "{descricao}");
    assert!(
        descricao.contains(&format!("{SERVICO}:30-32")),
        "{descricao}"
    );
    // A HU-0002 não está aprovada: a marca não vira lacuna nela.
    assert!(
        !m.historias[1]
            .alertas
            .contains(&Alerta::EvidenciaARevalidar)
    );

    // Apagar o índice e reconstruir recupera a mesma matriz.
    let arquivo = c.amb.dados.arquivo_do_indice();
    std::fs::remove_dir_all(arquivo.parent().unwrap()).unwrap();
    let mut indice = Indice::abrir(&arquivo, &c.amb.base).unwrap();
    let vazia = indice.matriz(c.amb.kb.nome(), IdDemanda::novo(1)).unwrap();
    assert!(vazia.historias.is_empty() && vazia.analises.is_empty());
    let resumo = indice.sincronizar().unwrap();
    assert!(resumo.novos > 0, "{resumo:?}");
    assert_eq!(
        indice.matriz(c.amb.kb.nome(), IdDemanda::novo(1)).unwrap(),
        m
    );
    indice.reconstruir().unwrap();
    assert_eq!(
        indice.matriz(c.amb.kb.nome(), IdDemanda::novo(1)).unwrap(),
        m
    );
    // Sem mudança, nada é regravado.
    assert!(!indice.sincronizar().unwrap().mudou());
}

#[test]
fn consulta_reversa_por_regra_e_por_arquivo() {
    let c = caso();
    let indice = c.indice();
    let projeto = c.amb.kb.nome();

    let d = indice
        .dependentes(projeto, &AlvoReverso::Regra("RN-FAT-007".into()))
        .unwrap();
    assert_eq!(
        d.regra.as_ref().unwrap().titulo,
        "Parcelamento só para pessoa jurídica e órgão público"
    );
    assert_eq!(d.arquivos, [SERVICO]);
    let demandas: Vec<&str> = d.demandas.iter().map(|x| x.id.as_str()).collect();
    assert_eq!(demandas, ["D-0001"]);
    let analises: Vec<&str> = d.analises.iter().map(|x| x.id.as_str()).collect();
    assert_eq!(analises, ["A-0001", "A-0002", "A-0003"]);
    let criterios: Vec<String> = d
        .criterios
        .iter()
        .map(|x| format!("{} {}", x.historia, x.criterio))
        .collect();
    assert_eq!(criterios, ["HU-0001 CA-1"]);
    let cenarios: Vec<&str> = d.cenarios.iter().map(|x| x.id.as_str()).collect();
    assert_eq!(cenarios, ["CT-0001"]);

    // Regra que nada cita.
    let d = indice
        .dependentes(projeto, &AlvoReverso::Regra("RN-FAT-099".into()))
        .unwrap();
    assert!(d.vazio());

    let d = indice
        .dependentes(projeto, &AlvoReverso::Arquivo(SERVICO.into()))
        .unwrap();
    let regras: Vec<&str> = d.regras.iter().map(|x| x.id.as_str()).collect();
    assert_eq!(regras, ["RN-FAT-007", "RN-FAT-008", "RN-FAT-009"]);
    let criterios: Vec<String> = d
        .criterios
        .iter()
        .map(|x| format!("{} {}", x.historia, x.criterio))
        .collect();
    assert_eq!(criterios, ["HU-0001 CA-1", "HU-0002 CA-1"]);
    assert_eq!(d.cenarios.len(), 1);
    assert_eq!(d.demandas.len(), 1);

    let d = indice
        .dependentes(
            projeto,
            &AlvoReverso::Arquivo("db/02_parametros.sql".into()),
        )
        .unwrap();
    assert!(d.vazio());
}

#[test]
fn ferramenta_mcp_e_exportacao() {
    let c = caso();

    let texto = c.texto("rastreabilidade", json!({ "regra": "RN-FAT-007" }));
    assert!(texto.contains("D-0001"), "{texto}");
    assert!(texto.contains("HU-0001 CA-1"), "{texto}");
    assert!(
        texto.contains("já foi tratada na demanda D-0001"),
        "{texto}"
    );
    let dados = c
        .chamar("rastreabilidade", json!({ "arquivo": SERVICO }))
        .unwrap();
    assert_eq!(dados["regras"].as_array().unwrap().len(), 3);
    assert_eq!(dados["demandas"][0]["id"], "D-0001");

    let texto = c.texto("rastreabilidade", json!({ "demanda": "D-0001" }));
    assert!(texto.contains("HU-0002"), "{texto}");
    assert!(texto.contains("regra afetada sem história"), "{texto}");
    let dados = c
        .chamar("rastreabilidade", json!({ "demanda": "D-0001" }))
        .unwrap();
    assert_eq!(dados["historias"].as_array().unwrap().len(), 2);
    assert_eq!(dados["lacunas"][0]["alerta"], "regra-sem-historia");

    for args in [
        json!({}),
        json!({ "demanda": "D-0001", "regra": "RN-FAT-007" }),
        json!({ "regra": "A-0001" }),
        json!({ "demanda": "0001" }),
        json!({ "arquivo": "../fora.sql" }),
    ] {
        match c.chamar("rastreabilidade", args.clone()) {
            Err(ErroFerramenta::Argumentos(_)) => {}
            outro => panic!("{args}: esperava recusa, veio {outro:?}"),
        }
    }
    assert!(
        registro_padrao()
            .obter("rastreabilidade")
            .unwrap()
            .e_somente_leitura()
    );

    // Exportação: planilha com as três abas e Word com as tabelas.
    let m = c.matriz();
    let xlsx = rastreabilidade::exportar(&m, FormatoMatriz::Xlsx).unwrap();
    let parte = |bytes: &[u8], nome: &str| {
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        let mut texto = String::new();
        zip.by_name(nome)
            .unwrap()
            .read_to_string(&mut texto)
            .unwrap();
        texto
    };
    let pasta = parte(&xlsx, "xl/workbook.xml");
    for aba in ["Por história", "Por regra", "Lacunas"] {
        assert!(pasta.contains(aba), "{pasta}");
    }
    let historias = parte(&xlsx, "xl/worksheets/sheet1.xml");
    assert!(historias.contains("HU-0001"), "{historias}");
    assert!(historias.contains("CT-0001"), "{historias}");
    assert!(
        historias.contains(&format!("{SERVICO}:30-32")),
        "{historias}"
    );
    let lacunas = parte(&xlsx, "xl/worksheets/sheet3.xml");
    assert!(lacunas.contains("RN-FAT-009"), "{lacunas}");

    let docx = rastreabilidade::exportar(&m, FormatoMatriz::Word).unwrap();
    let documento = parte(&docx, "word/document.xml");
    assert!(
        documento.contains("Matriz de rastreabilidade da demanda D-0001"),
        "{documento}"
    );
    assert!(documento.contains("RN-FAT-009"));
    assert!(documento.contains("<w:tbl>"));
    let md = rastreabilidade::markdown(&m);
    assert!(
        md.contains("| HU-0002 Limitar a 3 parcelas para pessoa física |"),
        "{md}"
    );
}
