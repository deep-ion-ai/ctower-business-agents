//! Comparação dos cenários de teste (fluxo `cenarios`, #117) com o gabarito:
//! cenários bons (limites lidos do código, valores testados, tabela de decisão,
//! negativos e tema a confirmar) e ruins (limite divergente, cenário sem origem
//! é falha grave), e o preparo das regras na base.

use cba_avaliacao::cenarios::preparar_regras;
use cba_avaliacao::comparacao::avaliar;
use cba_avaliacao::execucao::escopo;
use cba_avaliacao::gabarito::Gabarito;
use cba_avaliacao::gabarito_padrao;
use cba_avaliacao::relatorio::{Execucao, Relatorio};
use cba_core::analise::{Analise, EstadoEvidencia, Evidencia, Fluxo};
use cba_core::cenarios::{
    Condicao, Limite, NovaColuna, NovaTabela, NovoCenario, RefColuna, Tecnica, TextoCenario,
    ValorCondicao,
};
use cba_core::historias::Gherkin;
use cba_core::requisito::{Fundamento, RefItem};
use cba_kb::frontmatter::Linhas;
use cba_kb::{CaminhoRelativo, Id, IdCenario, Serie};

const DELPHI: &str = "delphi/UFrmCancelamento.pas";
const PKG: &str = "db/03_pkg_cobranca.sql";

fn gabarito() -> Gabarito {
    Gabarito::carregar(&gabarito_padrao()).unwrap()
}

fn ev(arquivo: &str, linha: u32, trecho: &str) -> Evidencia {
    Evidencia {
        arquivo: CaminhoRelativo::novo(arquivo).unwrap(),
        linhas: Linhas::novo(linha, linha).unwrap(),
        trecho: trecho.into(),
        hash: "h".into(),
        commit: None,
        estado: EstadoEvidencia::Valida,
    }
}

fn regra(id: &str) -> Fundamento {
    Fundamento::Regra {
        id: Id::analisar(id).unwrap(),
        titulo: id.into(),
        evidencias: vec![],
    }
}

fn novo(
    tecnica: Tecnica,
    titulo: &str,
    quando: &str,
    entao: &str,
    valor: &str,
    limite: Option<Limite>,
    rn: &str,
) -> NovoCenario {
    NovoCenario {
        tecnica,
        texto: TextoCenario {
            titulo: titulo.into(),
            passos: Gherkin {
                dado: "uma fatura de R$ 500,00".into(),
                quando: quando.into(),
                entao: entao.into(),
            },
        },
        valor: Some(valor.into()),
        limite,
        coluna: None,
        historia: None,
        fundamentos: vec![regra(rn)],
        pergunta: None,
        sessao: None,
    }
}

fn analise() -> Analise {
    let mut a = Analise::nova(
        Serie::Analise.id(1),
        "faturamento-exemplo",
        Fluxo::Cenarios,
        "Cenários do cancelamento",
        "Cenários das regras RN-FAT-012, RN-FAT-013 e RN-FAT-014",
        1,
    );
    let v = |s: &str| -> Vec<ValorCondicao> {
        s.split('/')
            .map(|x| ValorCondicao::analisar(x).unwrap())
            .collect()
    };
    a.registrar_tabela(
        NovaTabela {
            titulo: "Cancelamento de fatura antiga".into(),
            condicoes: vec![
                Condicao {
                    texto: "emitida há mais de 30 dias".into(),
                    evidencias: vec![],
                },
                Condicao {
                    texto: "perfil SUPERVISOR".into(),
                    evidencias: vec![],
                },
            ],
            acoes: vec!["cancela".into(), "recusa".into()],
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
            fundamentos: vec![regra("RN-FAT-014")],
            sessao: None,
        },
        1,
    )
    .unwrap();
    let limite_30 = || Limite {
        grandeza: "dias desde a emissão".into(),
        valor: "30".into(),
        evidencias: vec![ev(DELPHI, 41, "DIAS_LIMITE_SEM_SUPERVISOR = 30;")],
        configuracao: None,
    };
    let limite_10 = || Limite {
        grandeza: "tamanho do motivo".into(),
        valor: "10".into(),
        evidencias: vec![ev(PKG, 119, "LENGTH(TRIM(P_MOTIVO)) < 10")],
        configuracao: None,
    };
    let mut lista = vec![
        novo(
            Tecnica::ValorLimite,
            "31 dias sem supervisor",
            "o atendente cancela",
            "recusa",
            "31 dias",
            Some(limite_30()),
            "RN-FAT-014",
        ),
        novo(
            Tecnica::ValorLimite,
            "30 dias sem supervisor",
            "o atendente cancela",
            "cancela",
            "30 dias",
            Some(limite_30()),
            "RN-FAT-014",
        ),
        novo(
            Tecnica::ValorLimite,
            "Motivo de 9 caracteres",
            "o atendente cancela",
            "erro -20012",
            "9 caracteres",
            Some(limite_10()),
            "RN-FAT-013",
        ),
        novo(
            Tecnica::ValorLimite,
            "Motivo de 10 caracteres",
            "o atendente cancela",
            "cancela",
            "10 caracteres",
            Some(limite_10()),
            "RN-FAT-013",
        ),
        novo(
            Tecnica::Negativo,
            "Fatura com pagamento parcial",
            "o atendente cancela uma fatura com R$ 100,00 pago",
            "erro -20011",
            "VL_PAGO 100",
            None,
            "RN-FAT-012",
        ),
        novo(
            Tecnica::Negativo,
            "Fatura que não está aberta",
            "o atendente cancela uma fatura PAGA",
            "erro -20010: somente fatura ABERTA pode ser cancelada",
            "status PAGA",
            None,
            "RN-FAT-012",
        ),
    ];
    lista[0].coluna = RefColuna::analisar("TD-1/R2");
    for (n, c) in lista.into_iter().enumerate() {
        a.registrar_cenario(c, IdCenario::novo(n as u32 + 1), 2)
            .unwrap();
    }
    a.concluir().unwrap();
    a
}

#[test]
fn cenarios_bons_atendem_o_gabarito() {
    let g = gabarito();
    let item = g.item("CEN-FAT-001").unwrap();
    assert_eq!(item.fluxo, Fluxo::Cenarios);
    let av = avaliar(&analise(), &item, &g, &escopo(&g).unwrap());
    assert!(av.falhas().is_empty(), "{:?}", av.falhas());
    let c = av.cenarios.as_ref().unwrap();
    assert_eq!((c.cenarios, c.tabelas), (6, 1));
    assert_eq!(c.limites_lidos(), 2, "{:?}", c.limites);
    assert!(
        c.limites.iter().all(|l| l.valores_cobertos() == 2),
        "{:?}",
        c.limites
    );
    assert_eq!(c.tabelas_atendidas(), 1, "{:?}", c.tabelas_esperadas);
    assert_eq!(c.negativos_atendidos(), 2, "{:?}", c.negativos);

    let relatorio = Relatorio::novo(
        None,
        vec![Execucao {
            fluxo: "cenarios".into(),
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
    assert!(md.contains("Limites lidos do código: 2 de 2"), "{md}");
    assert!(md.contains("Tabelas de decisão esperadas: 1 de 1"), "{md}");
    assert!(md.contains("Negativos esperados: 2 de 2"), "{md}");
    assert!(relatorio.sem_falhas_graves);
}

#[test]
fn limite_divergente_e_tema_a_confirmar() {
    let g = gabarito();
    let item = g.item("CEN-FAT-002").unwrap();
    let mut a = Analise::nova(
        Serie::Analise.id(1),
        "faturamento-exemplo",
        Fluxo::Cenarios,
        "Cenários das parcelas",
        "RN-FAT-008",
        1,
    );
    // Um limite lido errado (o máximo da tela, 12) e o máximo do parâmetro a
    // confirmar; o mínimo (2) não foi lido.
    let tela = "frontend/src/telas/parcelamento-form.ts";
    let mut errado = novo(
        Tecnica::ValorLimite,
        "13 parcelas",
        "o cliente pede 13 parcelas",
        "recusa",
        "13 parcelas",
        Some(Limite {
            grandeza: "quantidade de parcelas".into(),
            valor: "12".into(),
            evidencias: vec![ev(tela, 19, "const MAX_PARCELAS = 12;")],
            configuracao: None,
        }),
        "RN-FAT-008",
    );
    errado.fundamentos = vec![regra("RN-FAT-008")];
    a.registrar_cenario(errado, IdCenario::novo(1), 2).unwrap();
    let mut conf = novo(
        Tecnica::ValorLimite,
        "7 parcelas",
        "o cliente pede 7 parcelas",
        "recusa",
        "7 parcelas",
        Some(Limite {
            grandeza: "quantidade máxima de parcelas".into(),
            valor: "6".into(),
            evidencias: vec![ev(
                "backend/src/main/java/exemplo/faturamento/ParcelamentoService.java",
                34,
                "parametros.inteiro(ParametroRepository.MAX_PARCELAS)",
            )],
            configuracao: Some("PARAMETRO.MAX_PARCELAS".into()),
        }),
        "RN-FAT-008",
    );
    conf.pergunta = Some(RefItem::analisar("A-0001/1").unwrap());
    a.registrar_cenario(conf, IdCenario::novo(2), 2).unwrap();
    let av = avaliar(&a, &item, &g, &escopo(&g).unwrap());
    let c = av.cenarios.as_ref().unwrap();
    assert_eq!(c.limites_lidos(), 0);
    assert_eq!(c.limites[0].divergentes, ["CT-0001: 12"]);
    assert_eq!(c.temas_marcados(), 1);
    assert!(av.falhas().is_empty(), "{:?}", av.falhas());

    // Cenário sem origem é falha grave; nenhum cenário também.
    a.cenarios[0].fundamentos.clear();
    let av = avaliar(&a, &item, &g, &escopo(&g).unwrap());
    assert_eq!(av.falhas(), ["cenários sem origem: CT-0001"]);
    a.cenarios.clear();
    let av = avaliar(&a, &item, &g, &escopo(&g).unwrap());
    assert_eq!(av.falhas(), ["nenhum cenário de teste registrado"]);
}

#[test]
fn preparo_grava_as_regras_e_cria_a_analise() {
    let g = gabarito();
    let item = g.item("CEN-FAT-001").unwrap();
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
    preparar_regras(&amb, &g, &item.regras).unwrap();
    for rn in ["RN-FAT-012", "RN-FAT-013", "RN-FAT-014"] {
        assert!(amb.kb.existe(&Id::analisar(rn).unwrap().arquivo()), "{rn}");
    }
    let a = cba_tools::ciclo::criar_analise_de_cenarios(
        &amb,
        None,
        None,
        Some(item.entrada.clone()),
        Some(item.titulo.clone()),
    )
    .unwrap();
    assert_eq!(a.fluxo, Fluxo::Cenarios);
    assert!(a.entrada.contains("RN-FAT-014"));
}
