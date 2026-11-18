//! Comparação do mapa de funcionalidades (#208) com o gabarito `MAP-FAT-001`: o
//! mapa completo cobre as funcionalidades e trata todos os pontos de entrada; um
//! parcial aponta o ponto sem tratamento (falha grave) e a justificativa de um
//! ponto que deveria estar numa funcionalidade.

use cba_avaliacao::comparacao::avaliar;
use cba_avaliacao::execucao::escopo;
use cba_avaliacao::gabarito::Gabarito;
use cba_avaliacao::gabarito_padrao;
use cba_core::analise::{Analise, Fluxo};
use cba_core::funcionalidades::{
    DecisaoFuncionalidade, MotivoSemFuncionalidade, NovaFuncionalidade, PontoAtribuido,
    PontoJustificado,
};
use cba_index::{Escopo, PontoEntrada};
use cba_kb::frontmatter::Linhas;
use cba_kb::{IdFuncionalidade, Serie};
use cba_tools::evidencias::ArquivoDoProjeto;

fn gabarito() -> Gabarito {
    Gabarito::carregar(&gabarito_padrao()).unwrap()
}

/// Pontos de entrada da fixture, pelo mapa estrutural.
fn pontos(g: &Gabarito) -> Vec<PontoEntrada> {
    let tmp = tempfile::tempdir().unwrap();
    let cadastro = cba_index::Cadastro::em(tmp.path());
    let projeto = cba_index::Projeto::novo("Faturamento Exemplo", [g.raiz_do_projeto()]).unwrap();
    cadastro.cadastrar(&projeto).unwrap();
    cadastro.inventariar(&projeto.id).unwrap();
    cadastro.mapear(&projeto.id).unwrap().pontos_de_entrada()
}

/// Ponto atribuído com a evidência da declaração (a primeira linha do símbolo).
fn atribuido(escopo: &Escopo, pontos: &[PontoEntrada], sufixo: &str) -> PontoAtribuido {
    let p = pontos.iter().find(|p| p.chave.ends_with(sufixo)).unwrap();
    let arquivo = ArquivoDoProjeto::abrir(escopo, &p.arquivo).unwrap();
    let linha = arquivo
        .texto
        .lines()
        .nth(p.linhas.inicio as usize - 1)
        .unwrap()
        .trim()
        .to_owned();
    let evidencia = arquivo
        .conferir(
            Linhas::novo(p.linhas.inicio, p.linhas.inicio).unwrap(),
            &linha,
        )
        .unwrap();
    PontoAtribuido {
        chave: Some(p.chave.clone()),
        tipo: Some(p.tipo),
        nome: p.nome.clone(),
        evidencia,
    }
}

fn registrar(a: &mut Analise, n: u32, nome: &str, area: &str, pontos: Vec<PontoAtribuido>) {
    let id = IdFuncionalidade::novo("FAT", n).unwrap();
    a.registrar_funcionalidade(
        NovaFuncionalidade {
            nome: nome.into(),
            area: area.into(),
            area_nome: area.into(),
            descricao: format!("{nome}."),
            atores: vec![],
            pontos,
            dados_lidos: vec![],
            dados_gravados: vec![],
            relacionadas: vec![],
            do_catalogo: false,
            sessao: None,
        },
        id.clone(),
        1,
    )
    .unwrap();
    a.decidir_funcionalidade(&id, DecisaoFuncionalidade::Aceitar)
        .unwrap();
}

fn justificar(a: &mut Analise, pontos: &[PontoEntrada], sufixo: &str) {
    let p = pontos.iter().find(|p| p.chave.ends_with(sufixo)).unwrap();
    a.justificar_ponto(PontoJustificado {
        chave: p.chave.clone(),
        tipo: p.tipo,
        nome: p.nome.clone(),
        arquivo: p.arquivo.clone(),
        motivo: MotivoSemFuncionalidade::Utilitario,
        justificativa: "ajudante".into(),
        sessao: None,
        registrado_ms: 1,
    })
    .unwrap();
}

/// O mapa da fixture como o gabarito espera; `com_suspensao` decide se o ponto
/// da suspensão entra.
fn mapa(g: &Gabarito, com_suspensao: bool) -> Analise {
    let escopo = escopo(g).unwrap();
    let pontos = pontos(g);
    let p = |s: &str| atribuido(&escopo, &pontos, s);
    let mut a = Analise::nova(
        Serie::Analise.id(1),
        "faturamento-exemplo",
        Fluxo::Mapear,
        "Mapa",
        "",
        1,
    );
    registrar(
        &mut a,
        1,
        "Calcular encargos por atraso",
        "cobranca",
        vec![
            p("EncargosService.calcular"),
            p("FN_CALCULAR_MULTA"),
            p("FN_CALCULAR_JUROS"),
            p("FN_DIAS_ATRASO"),
        ],
    );
    registrar(
        &mut a,
        2,
        "Parcelar fatura",
        "parcelamento",
        vec![
            p("ParcelamentoService.parcelar"),
            p("PRC_EFETIVAR_PARCELAMENTO"),
            p("#opcoesDeParcelas"),
            p("#enviarParcelamento"),
        ],
    );
    registrar(
        &mut a,
        3,
        "Cancelar fatura",
        "cancelamento",
        vec![p("#FrmCancelamento"), p("PRC_CANCELAR_FATURA")],
    );
    registrar(
        &mut a,
        4,
        "Lote mensal de cobrança",
        "cobranca",
        vec![p("#CALCOBR")],
    );
    registrar(
        &mut a,
        5,
        "Registrar pagamento",
        "cobranca",
        vec![p("PRC_REGISTRAR_PAGAMENTO"), p("PRC_QUITAR_PARCELA")],
    );
    if com_suspensao {
        registrar(
            &mut a,
            6,
            "Suspender a cobrança por contestação",
            "cobranca",
            vec![p("PRC_SUSPENDER_COBRANCA")],
        );
    }
    justificar(&mut a, &pontos, "FN_PARAMETRO");
    a
}

#[test]
fn mapa_completo_cobre_tudo() {
    let g = gabarito();
    let item = g.item("MAP-FAT-001").unwrap();
    assert_eq!(item.fluxo, Fluxo::Mapear);
    let a = mapa(&g, true);
    let av = avaliar(&a, &item, &g, &escopo(&g).unwrap());
    let m = av.mapeamento.as_ref().unwrap();
    assert!(av.falhas().is_empty(), "{:?}", av.falhas());
    assert_eq!(m.cobertura_de_funcionalidades(), 1.0, "{m:#?}");
    assert_eq!(m.cobertura_de_pontos(), 1.0, "{m:#?}");
    assert!(m.funcionalidades.iter().all(|f| f.area_confere), "{m:#?}");
    assert!(m.evidencias_invalidas.is_empty());
}

#[test]
fn mapa_parcial_aponta_o_que_falta() {
    let g = gabarito();
    let item = g.item("MAP-FAT-001").unwrap();
    let mut a = mapa(&g, false);
    // A rotina de cálculo da multa deveria estar numa funcionalidade, não
    // justificada: o mapa a tira da F-FAT-001 e a justifica.
    let f1 = IdFuncionalidade::novo("FAT", 1).unwrap();
    let pontos = pontos(&g);
    a.funcionalidades
        .iter_mut()
        .find(|f| f.id == f1)
        .unwrap()
        .pontos
        .retain(|p| !p.chave.as_deref().unwrap().ends_with("FN_CALCULAR_MULTA"));
    justificar(&mut a, &pontos, "FN_CALCULAR_MULTA");

    let av = avaliar(&a, &item, &g, &escopo(&g).unwrap());
    let falhas = av.falhas();
    assert!(
        falhas
            .iter()
            .any(|f| f.contains("sem tratamento") && f.contains("PRC_SUSPENDER_COBRANCA")),
        "{falhas:?}"
    );
    let m = av.mapeamento.unwrap();
    let suspensao = m
        .funcionalidades
        .iter()
        .find(|f| f.esperada == "F-FAT-006")
        .unwrap();
    assert!(suspensao.registrada.is_none());
    let multa = m
        .pontos
        .iter()
        .find(|p| p.chave.ends_with("FN_CALCULAR_MULTA"))
        .unwrap();
    assert_eq!(multa.tratamento, "justificado");
    assert!(!multa.bem_tratado);
    assert!(m.cobertura_de_pontos() < 1.0);
}
