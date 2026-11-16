//! Comparação do ciclo de vida da fatura (#119) com o gabarito `CV-FAT-001`: um
//! diagrama de estados completo cobre estados, transições e achados; um parcial
//! aponta o que falta; sem diagrama de estados é falha grave.

use cba_avaliacao::ciclo_vida::avaliar_ciclo_vida;
use cba_avaliacao::comparacao::avaliar;
use cba_avaliacao::execucao::escopo;
use cba_avaliacao::gabarito::Gabarito;
use cba_avaliacao::gabarito_padrao;
use cba_core::analise::{Analise, Confianca, EstadoEvidencia, Evidencia, Fluxo};
use cba_core::diagramas::{Aresta, No, NovoDiagrama, PapelNo, TipoDiagrama};
use cba_kb::frontmatter::Linhas;
use cba_kb::{CaminhoRelativo, Serie};

fn gabarito() -> Gabarito {
    Gabarito::carregar(&gabarito_padrao()).unwrap()
}

fn ev(arquivo: &str, inicio: u32, fim: u32) -> Evidencia {
    Evidencia {
        arquivo: CaminhoRelativo::novo(arquivo).unwrap(),
        linhas: Linhas::novo(inicio, fim).unwrap(),
        trecho: "x".into(),
        hash: "h".into(),
        commit: None,
        estado: EstadoEvidencia::Valida,
    }
}

fn no(chave: &str, papel: PapelNo) -> No {
    No {
        chave: chave.into(),
        rotulo: chave.to_lowercase(),
        papel,
        descricao: None,
        evidencias: vec![],
        raia: None,
        entidade: None,
    }
}

fn aresta(de: &str, para: &str, evidencia: Option<Evidencia>) -> Aresta {
    Aresta {
        de: de.into(),
        para: para.into(),
        rotulo: Some("evento".into()),
        descricao: None,
        condicao: Some("status ABERTA".into()),
        efeitos: None,
        confianca: if evidencia.is_some() {
            Confianca::Confirmado
        } else {
            Confianca::Inferido
        },
        evidencias: evidencia.into_iter().collect(),
        relacao: None,
    }
}

const SITUACAO: &str = "db/04_pkg_situacao.sql";

fn analise(nos: Vec<No>, arestas: Vec<Aresta>) -> Analise {
    let mut a = Analise::nova(
        Serie::Analise.id(1),
        "faturamento-exemplo",
        Fluxo::Explicar,
        "Ciclo de vida da fatura",
        "Ciclo de vida da fatura",
        1,
    );
    a.registrar_diagrama(
        NovoDiagrama {
            tipo: TipoDiagrama::Estados,
            titulo: "Ciclo de vida da fatura".into(),
            secao: "fluxo_principal".into(),
            nos,
            arestas,
            sessao: None,
        },
        None,
        2,
    )
    .unwrap();
    a
}

fn completo() -> Analise {
    analise(
        vec![
            no("ABERTA", PapelNo::Inicio),
            no("PAGA", PapelNo::Fim),
            no("PARCELADA", PapelNo::Etapa),
            no("CANCELADA", PapelNo::Fim),
            no("SUSPENSA", PapelNo::Etapa),
            no("protestada", PapelNo::Etapa),
        ],
        vec![
            aresta("ABERTA", "PAGA", Some(ev(SITUACAO, 61, 61))),
            aresta("ABERTA", "PARCELADA", Some(ev(SITUACAO, 80, 80))),
            aresta("PARCELADA", "PAGA", Some(ev(SITUACAO, 103, 107))),
            aresta(
                "ABERTA",
                "CANCELADA",
                Some(ev("db/03_pkg_cobranca.sql", 111, 128)),
            ),
            aresta("ABERTA", "SUSPENSA", Some(ev(SITUACAO, 121, 124))),
        ],
    )
}

#[test]
fn ciclo_completo_cobre_estados_transicoes_e_achados() {
    let g = gabarito();
    let item = g.item("CV-FAT-001").unwrap();
    assert_eq!(item.fluxo, Fluxo::Explicar);
    let esperado = item.ciclo_vida.as_ref().unwrap();
    let c = avaliar_ciclo_vida(&completo(), esperado);
    assert_eq!(c.diagrama.as_deref(), Some("D-1"));
    assert_eq!(c.cobertura_de_estados(), 1.0);
    assert_eq!(c.cobertura_de_transicoes(), 1.0, "{:?}", c.transicoes);
    assert_eq!(c.cobertura_de_achados(), 1.0, "{:?}", c.achados);
    assert!(c.inicial_confere);
    assert!(c.finais_sem_marca.is_empty());
    assert!(c.estados_extras.is_empty());

    // Na avaliação completa, o diagrama basta: sem afirmações não é falha.
    let av = avaliar(&completo(), &item, &g, &escopo(&g).unwrap());
    assert!(av.falhas().is_empty(), "{:?}", av.falhas());
    assert!(av.ciclo_vida.is_some());
}

#[test]
fn ciclo_parcial_aponta_o_que_falta() {
    let g = gabarito();
    let item = g.item("CV-FAT-001").unwrap();
    let a = analise(
        vec![
            no("ABERTA", PapelNo::Etapa),
            no("PAGA", PapelNo::Etapa),
            no("CANCELADA", PapelNo::Fim),
            no("ESTORNADA", PapelNo::Fim),
        ],
        vec![
            // Evidência em outro lugar: encontrada, mas não sustentada.
            aresta("ABERTA", "PAGA", Some(ev(SITUACAO, 1, 3))),
            aresta("ABERTA", "CANCELADA", None),
        ],
    );
    let c = avaliar_ciclo_vida(&a, item.ciclo_vida.as_ref().unwrap());
    assert_eq!(c.cobertura_de_estados(), 0.5);
    assert_eq!(c.cobertura_de_transicoes(), 0.0);
    assert!(c.transicoes[0].encontrada && !c.transicoes[0].sustentada);
    assert!(!c.inicial_confere);
    assert_eq!(c.finais_sem_marca, ["PAGA"]);
    assert_eq!(c.estados_extras, ["ESTORNADA"]);
    assert_eq!(c.cobertura_de_achados(), 0.0);
}

#[test]
fn sem_diagrama_de_estados_e_falha_grave() {
    let g = gabarito();
    let item = g.item("CV-FAT-001").unwrap();
    let a = Analise::nova(
        Serie::Analise.id(1),
        "faturamento-exemplo",
        Fluxo::Explicar,
        "t",
        "e",
        1,
    );
    let av = avaliar(&a, &item, &g, &escopo(&g).unwrap());
    assert_eq!(av.falhas(), ["nenhum diagrama de estados registrado"]);
}
