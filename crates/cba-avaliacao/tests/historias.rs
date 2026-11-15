//! Comparação das histórias (fluxo `historias`, #115) com o gabarito: histórias
//! boas (as esperadas, as regras citadas, o tema a confirmar e a quebra) e ruins
//! (critério sem origem é falha grave), e o preparo da análise de origem.

use cba_avaliacao::comparacao::avaliar;
use cba_avaliacao::execucao::escopo;
use cba_avaliacao::gabarito::Gabarito;
use cba_avaliacao::gabarito_padrao;
use cba_avaliacao::historias::preparar_origem;
use cba_avaliacao::relatorio::{Execucao, Relatorio};
use cba_core::analise::{Analise, EstadoAfirmacao, EstadoAnalise, Fluxo};
use cba_core::historias::{Gherkin, Narrativa, NovaHistoria, NovoCriterio};
use cba_core::requisito::{Fundamento, RefItem};
use cba_kb::frontmatter::{Evidencia, Linhas};
use cba_kb::{CaminhoRelativo, Id, IdHistoria, Serie};

fn gabarito() -> Gabarito {
    Gabarito::carregar(&gabarito_padrao()).unwrap()
}

fn ev(arquivo: &str, inicio: u32, fim: u32) -> Evidencia {
    Evidencia {
        arquivo: CaminhoRelativo::novo(arquivo).unwrap(),
        linhas: Linhas::novo(inicio, fim).unwrap(),
        hash: "h".into(),
        commit: None,
    }
}

fn criterio(dado: &str, fundamentos: Vec<Fundamento>, pergunta: Option<&str>) -> NovoCriterio {
    NovoCriterio {
        passos: Gherkin {
            dado: dado.into(),
            quando: "ela pede o parcelamento em 3 vezes".into(),
            entao: "o pedido segue a regra".into(),
        },
        fundamentos,
        pergunta: pergunta.map(|p| RefItem::analisar(p).unwrap()),
        sessao: None,
    }
}

fn historia(titulo: &str, criterios: Vec<NovoCriterio>, quebra: Option<&str>) -> NovaHistoria {
    NovaHistoria {
        narrativa: Narrativa {
            titulo: titulo.into(),
            como: "atendente do faturamento".into(),
            quero: titulo.to_lowercase(),
            para: "reduzir a inadimplência".into(),
        },
        criterios,
        invest: vec![],
        quebra: quebra.map(str::to_owned),
        fatia_de: None,
        sessao: None,
    }
}

fn analise() -> Analise {
    let mut a = Analise::nova(
        Serie::Analise.id(2),
        "faturamento-exemplo",
        Fluxo::Historias,
        "Histórias",
        "Histórias da A-0001",
        1,
    );
    a.origem = Some(Serie::Analise.id(1));
    // RN-FAT-007 pela afirmação com a evidência dela; RN-FAT-008 pelo id.
    let pf = Fundamento::Afirmacao {
        referencia: RefItem::analisar("A-0001/1").unwrap(),
        texto: "Pessoa física não pode parcelar.".into(),
        evidencias: vec![ev(
            "backend/src/main/java/exemplo/faturamento/ParcelamentoService.java",
            30,
            32,
        )],
    };
    let quantidade = Fundamento::Regra {
        id: Id::analisar("RN-FAT-008").unwrap(),
        titulo: "Quantidade de parcelas".into(),
        evidencias: vec![],
    };
    a.registrar_historia(
        historia(
            "Parcelar fatura de pessoa física",
            vec![criterio(
                "uma pessoa física com fatura de R$ 300,00",
                vec![pf.clone(), quantidade],
                None,
            )],
            Some("Separar o limite por tipo de cliente numa fatia."),
        ),
        IdHistoria::novo(1),
        2,
    )
    .unwrap();
    a.registrar_historia(
        historia(
            "Limite de 3 parcelas para pessoa física",
            vec![criterio(
                "uma parcela mínima de R$ 100,00",
                vec![pf],
                Some("A-0001/1"),
            )],
            None,
        ),
        IdHistoria::novo(2),
        3,
    )
    .unwrap();
    a.concluir().unwrap();
    a
}

#[test]
fn historias_boas_atendem_o_gabarito() {
    let g = gabarito();
    let item = g.item("HIS-FAT-001").unwrap();
    assert_eq!(item.fluxo, Fluxo::Historias);
    let av = avaliar(&analise(), &item, &g, &escopo(&g).unwrap());
    assert!(av.falhas().is_empty(), "{:?}", av.falhas());
    let h = av.historias.as_ref().unwrap();
    assert_eq!((h.historias, h.criterios, h.criterios_com_valor), (2, 2, 2));
    assert_eq!(h.historias_atendidas(), 2, "{:?}", h.esperadas);
    let regra = |id: &str| {
        h.regras
            .iter()
            .find(|r| r.id == id)
            .unwrap()
            .citada_por
            .clone()
    };
    assert_eq!(regra("RN-FAT-007").as_deref(), Some("HU-0001 CA-1"));
    assert_eq!(regra("RN-FAT-008").as_deref(), Some("HU-0001 CA-1"));
    assert_eq!(regra("RN-FAT-009"), None);
    assert_eq!(h.temas_marcados(), 1);
    assert!(h.quebra_proposta);

    let relatorio = Relatorio::novo(
        None,
        vec![Execucao {
            fluxo: "historias".into(),
            item: item.id.clone(),
            repeticao: 1,
            avaliacao: Some(av),
            ata: None,
            transcript: None,
            arquivos_alterados: Vec::new(),
            claude: None,
            erro: None,
        }],
    );
    let md = relatorio.markdown();
    assert!(md.contains("Histórias esperadas: 2 de 2"), "{md}");
    assert!(
        md.contains("Regras citadas pelos critérios: 2 de 3"),
        "{md}"
    );
    assert!(md.contains("RN-FAT-009: não citada"), "{md}");
    assert!(md.contains("Quebra em fatias: proposta"), "{md}");
    assert!(relatorio.sem_falhas_graves);
}

#[test]
fn criterio_sem_origem_e_falha_grave() {
    let g = gabarito();
    let item = g.item("HIS-FAT-001").unwrap();
    // O registro recusa; um estado adulterado (ou uma regressão) é pego aqui.
    let mut a = analise();
    a.historias[0].criterios[0].fundamentos.clear();
    let av = avaliar(&a, &item, &g, &escopo(&g).unwrap());
    let falhas = av.falhas();
    assert_eq!(falhas, ["critérios sem origem: HU-0001 CA-1"], "{falhas:?}");

    let mut vazia = analise();
    vazia.historias.clear();
    let av = avaliar(&vazia, &item, &g, &escopo(&g).unwrap());
    assert_eq!(av.falhas(), ["nenhuma história registrada"]);
}

#[test]
fn preparo_cria_a_origem_aprovada_com_as_regras_da_demanda() {
    let g = gabarito();
    let item = g.item("HIS-FAT-001").unwrap();
    let dir = tempfile::tempdir().unwrap();
    let dados = cba_core::dados::DiretorioDados::em(dir.path().join("dados"));
    let projeto = cba_index::Projeto::novo("Faturamento Exemplo", [g.raiz_do_projeto()]).unwrap();
    cba_index::Cadastro::em(dados.projetos())
        .cadastrar(&projeto)
        .unwrap();
    cba_kb::Base::criar(dir.path().join("base")).unwrap();
    cba_core::config::Config {
        base: Some(dir.path().join("base")),
        ..Default::default()
    }
    .gravar(&dados)
    .unwrap();
    let amb = cba_core::ambiente::Ambiente::do_diretorio(&g.raiz_do_projeto(), &dados).unwrap();

    let id = preparar_origem(&amb, &g, item.historias.as_ref().unwrap()).unwrap();
    let origem = amb.analises.carregar(&id).unwrap();
    assert_eq!(origem.fluxo, Fluxo::Impacto);
    assert_eq!(origem.estado, EstadoAnalise::Aprovada);
    // Três regras (RN-FAT-007, 008 e 009) e a abrangência, todas confirmadas.
    assert_eq!(origem.afirmacoes.len(), 4);
    assert!(
        origem
            .afirmacoes
            .iter()
            .all(|a| a.estado == EstadoAfirmacao::Confirmada)
    );
    assert_eq!(origem.perguntas.len(), 2);
    for rn in ["RN-FAT-007", "RN-FAT-008", "RN-FAT-009"] {
        assert!(amb.kb.existe(&Id::analisar(rn).unwrap().arquivo()), "{rn}");
    }
    let historias = cba_tools::ciclo::criar_analise_de_historias(
        &amb,
        Some(id.clone()),
        None,
        None,
        Some(item.titulo.clone()),
    )
    .unwrap();
    assert_eq!(historias.origem, Some(id));
}
