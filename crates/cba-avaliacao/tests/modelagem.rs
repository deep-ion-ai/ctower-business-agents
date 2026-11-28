//! Comparação do fluxo do processo e do modelo de dados conceitual do documento
//! negocial (fluxo `explicar`, #120) com o gabarito da F-FAT-003 (cancelamento de
//! fatura), sem chamar o Claude.

use cba_avaliacao::comparacao::{Avaliacao, avaliar};
use cba_avaliacao::execucao::escopo;
use cba_avaliacao::gabarito::Gabarito;
use cba_avaliacao::gabarito_padrao;
use cba_avaliacao::relatorio::{Execucao, Relatorio};
use cba_core::analise::{Analise, Confianca, Evidencia};
use cba_core::diagramas::{
    Aresta, Atributo, Entidade, Multiplicidade, No, NovoDiagrama, OrigemRelacao, PapelNo, Relacao,
    TipoDiagrama,
};
use cba_tools::evidencias::{EvidenciaCitada, validar};

const TELA: &str = "delphi/UFrmCancelamento.pas";
const PKG: &str = "db/03_pkg_cobranca.sql";

fn gabarito() -> Gabarito {
    Gabarito::carregar(&gabarito_padrao()).unwrap()
}

fn ev(arquivo: &str, inicio: u32, fim: u32, trecho: &str) -> Evidencia {
    validar(
        &escopo(&gabarito()).unwrap(),
        &EvidenciaCitada {
            arquivo: arquivo.into(),
            linha_inicial: inicio,
            linha_final: fim,
            trecho: trecho.into(),
        },
    )
    .unwrap_or_else(|e| panic!("evidência de teste inválida: {e}"))
}

fn analise() -> Analise {
    serde_json::from_value(serde_json::json!({
        "formato": 1,
        "id": "A-0001",
        "projeto": "faturamento-exemplo",
        "fluxo": "explicar",
        "titulo": "Cancelamento de fatura",
        "entrada": "Cancelamento de fatura",
        "estado": "em-andamento",
        "documento": "analises/A-0001.md",
        "versao": 1,
        "criada_ms": 0,
        "atualizada_ms": 0
    }))
    .unwrap()
}

fn no(chave: &str, rotulo: &str, papel: PapelNo, raia: &str, evidencias: Vec<Evidencia>) -> No {
    No {
        chave: chave.into(),
        rotulo: rotulo.into(),
        papel,
        descricao: None,
        evidencias,
        raia: Some(raia.into()),
        entidade: None,
    }
}

fn aresta(de: &str, para: &str, rotulo: Option<&str>) -> Aresta {
    Aresta {
        de: de.into(),
        para: para.into(),
        rotulo: rotulo.map(Into::into),
        descricao: None,
        condicao: None,
        efeitos: None,
        confianca: Confianca::Inferido,
        evidencias: vec![],
        relacao: None,
    }
}

fn processo() -> NovoDiagrama {
    let tela = "Atendente (tela de cancelamento)";
    let banco = "Banco de dados";
    NovoDiagrama {
        tipo: TipoDiagrama::Fluxo,
        titulo: "Cancelamento de fatura".into(),
        secao: "fluxo_processo".into(),
        nos: vec![
            no(
                "i",
                "Informar a fatura e o motivo",
                PapelNo::Inicio,
                tela,
                vec![],
            ),
            no(
                "m",
                "Motivo válido?",
                PapelNo::Decisao,
                tela,
                vec![ev(TELA, 63, 63, "TAM_MINIMO_MOTIVO")],
            ),
            no(
                "p",
                "Mais de 30 dias sem supervisor?",
                PapelNo::Decisao,
                tela,
                vec![],
            ),
            no("r", "Recusado", PapelNo::Fim, tela, vec![]),
            no(
                "s",
                "Fatura aberta e sem pagamento?",
                PapelNo::Decisao,
                banco,
                vec![ev(PKG, 111, 117, "V_VL_PAGO > 0")],
            ),
            no(
                "c",
                "Chamar o cancelamento no banco",
                PapelNo::Etapa,
                tela,
                vec![ev(TELA, 94, 94, "PRC_CANCELAR_FATURA")],
            ),
            no(
                "g",
                "Gravar a fatura cancelada e o histórico",
                PapelNo::Etapa,
                banco,
                vec![],
            ),
            no("f", "Fatura cancelada", PapelNo::Fim, banco, vec![]),
        ],
        arestas: vec![
            aresta("i", "m", None),
            aresta("m", "r", Some("não")),
            aresta("m", "p", Some("sim")),
            aresta("p", "r", Some("sim")),
            aresta("p", "c", Some("não")),
            aresta("c", "s", None),
            aresta("s", "r", Some("não")),
            aresta("s", "g", Some("sim")),
            aresta("g", "f", None),
        ],
        sessao: None,
    }
}

fn entidade(chave: &str, rotulo: &str, tabela: &str, termo: Option<&str>) -> No {
    No {
        chave: chave.into(),
        rotulo: rotulo.into(),
        papel: PapelNo::Etapa,
        descricao: None,
        evidencias: vec![],
        raia: None,
        entidade: Some(Entidade {
            tabela: tabela.into(),
            atributos: vec![Atributo {
                nome: "Situação".into(),
                coluna: "STATUS".into(),
            }],
            termo: termo.map(Into::into),
        }),
    }
}

fn relacao(de: &str, para: &str, origem: OrigemRelacao, evidencias: Vec<Evidencia>) -> Aresta {
    Aresta {
        confianca: if origem == OrigemRelacao::Join {
            Confianca::Inferido
        } else {
            Confianca::Confirmado
        },
        evidencias,
        relacao: Some(Relacao {
            de: Multiplicidade::Um,
            para: Multiplicidade::ZeroOuMuitos,
            origem,
        }),
        ..aresta(de, para, Some("registra"))
    }
}

fn modelo(origem_do_historico: OrigemRelacao) -> NovoDiagrama {
    NovoDiagrama {
        tipo: TipoDiagrama::Dados,
        titulo: "Modelo de dados do cancelamento".into(),
        secao: "modelo_dados".into(),
        nos: vec![
            entidade("c", "Cliente", "CLIENTE", Some("Cliente")),
            entidade("f", "Fatura", "FATURA", Some("Fatura")),
            entidade("h", "Histórico da fatura", "LOG_FATURA", None),
        ],
        arestas: vec![
            relacao(
                "c",
                "f",
                OrigemRelacao::ChaveEstrangeira,
                vec![ev("db/01_tabelas.sql", 33, 34, "FOREIGN KEY (ID_CLIENTE)")],
            ),
            relacao(
                "f",
                "h",
                origem_do_historico,
                vec![ev(PKG, 130, 131, "INSERT INTO LOG_FATURA")],
            ),
        ],
        sessao: None,
    }
}

fn avaliar_f003(a: &Analise) -> Avaliacao {
    let g = gabarito();
    let item = g.item("F-FAT-003").unwrap();
    assert!(item.modelagem.is_some());
    avaliar(a, &item, &g, &escopo(&g).unwrap())
}

#[test]
fn fluxograma_e_modelo_como_o_gabarito() {
    let mut a = analise();
    a.registrar_diagrama(processo(), None, 1).unwrap();
    a.registrar_diagrama(modelo(OrigemRelacao::Join), None, 2)
        .unwrap();
    let av = avaliar_f003(&a);
    let m = av.modelagem.as_ref().unwrap();

    let p = m.processo.as_ref().unwrap();
    assert_eq!(p.diagrama.as_deref(), Some("D-1"));
    assert!(p.coerente(), "{p:?}");
    assert!(
        p.raias.iter().all(|r| r.atendido_por.is_some()),
        "{:?}",
        p.raias
    );
    assert!(
        p.principal.iter().all(|i| i.atendido_por.is_some()),
        "{:?}",
        p.principal
    );
    assert!(
        p.decisoes.iter().all(|i| i.atendido_por.is_some()),
        "{:?}",
        p.decisoes
    );
    // A decisão do banco atende às duas decisões (aberta e pagamento).
    let d3 = p.decisoes.iter().find(|d| d.id == "F-FAT-003-D3").unwrap();
    assert_eq!(d3.atendido_por.as_deref(), Some("s"));

    let d = m.dados.as_ref().unwrap();
    assert_eq!(d.diagrama.as_deref(), Some("D-2"));
    assert!(d.entidades.iter().all(|e| e.nome_confere));
    assert_eq!(d.entidades[0].termo.as_deref(), Some("Fatura"));
    assert!(
        d.relacoes
            .iter()
            .all(|r| r.encontrada() && !r.origem_diverge())
    );
    assert!(m.falhas().is_empty(), "{:?}", m.falhas());

    // O relatório mostra os diagramas.
    let r = Relatorio::novo(
        None,
        vec![Execucao {
            fluxo: "explicar".into(),
            item: "F-FAT-003".into(),
            repeticao: 1,
            avaliacao: Some(av),
            ata: None,
            transcript: None,
            mapa: None,
            entregavel: None,
            publicacao: None,
            conducao: None,
            arquivos_alterados: vec![],
            claude: None,
            erro: None,
        }],
    );
    let md = r.markdown();
    assert!(md.contains("- Fluxo do processo: D-1 (coerente)"), "{md}");
    assert!(
        md.contains("F-FAT-003-M1 (FATURA – LOG_FATURA, esperada join): registrada como join"),
        "{md}"
    );
}

#[test]
fn relacao_inferida_como_chave_estrangeira_e_falha_grave() {
    let mut a = analise();
    a.registrar_diagrama(modelo(OrigemRelacao::ChaveEstrangeira), None, 1)
        .unwrap();
    let av = avaliar_f003(&a);
    let m = av.modelagem.as_ref().unwrap();
    let p = m.processo.as_ref().unwrap();
    assert!(p.diagrama.is_none() && !p.coerente());
    let falhas = av.falhas();
    assert!(
        falhas
            .iter()
            .any(|f| f.contains("F-FAT-003-M1") && f.contains("esperada join")),
        "{falhas:?}"
    );
}

#[test]
fn fluxograma_incoerente_aponta_decisao_sem_caminho_e_passo_solto() {
    let mut a = analise();
    let mut p = processo();
    p.arestas.retain(|x| !(x.de == "m" && x.para == "r"));
    p.arestas.retain(|x| x.para != "g");
    a.registrar_diagrama(p, None, 1).unwrap();
    let av = avaliar_f003(&a);
    let p = av.modelagem.unwrap().processo.unwrap();
    assert!(!p.coerente());
    assert_eq!(p.decisoes_sem_caminhos, vec!["m", "s"]);
    assert_eq!(p.inalcancaveis, vec!["g", "f"]);
}
