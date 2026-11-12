//! Comparação de estados de exemplo com o gabarito, sem chamar o Claude.
//!
//! Os estados são montados aqui, com evidências validadas contra a fixture (o mesmo
//! hash que o `afirmar` grava), e em `tests/estados/` há estados gravados por
//! execuções reais da avaliação.

use std::path::{Path, PathBuf};

use cba_avaliacao::comparacao::{Avaliacao, avaliar, veredito_do_texto};
use cba_avaliacao::execucao::escopo;
use cba_avaliacao::gabarito::{Gabarito, Veredito};
use cba_avaliacao::gabarito_padrao;
use cba_avaliacao::relatorio::{Execucao, Relatorio};
use cba_core::analise::{
    Afirmacao, Analise, Confianca, EstadoAfirmacao, EstadoPergunta, Evidencia, Pergunta,
};
use cba_tools::evidencias::{EvidenciaCitada, validar};

const JAVA: &str = "backend/src/main/java/exemplo/faturamento/ParcelamentoService.java";
const TELA: &str = "frontend/src/telas/parcelamento-form.ts";

fn gabarito() -> Gabarito {
    Gabarito::carregar(&gabarito_padrao()).unwrap()
}

/// Evidência válida, como o `afirmar` grava.
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

fn analise_vazia(fluxo: &str, entrada: &str) -> Analise {
    serde_json::from_value(serde_json::json!({
        "formato": 1,
        "id": "A-0001",
        "sistema": "faturamento-exemplo",
        "fluxo": fluxo,
        "titulo": "Teste",
        "entrada": entrada,
        "estado": "concluida",
        "documento": "analises/A-0001-teste.md",
        "versao": 1,
        "criada_ms": 0,
        "atualizada_ms": 0
    }))
    .unwrap()
}

fn afirmar(a: &mut Analise, secao: &str, texto: &str, confianca: Confianca, evs: Vec<Evidencia>) {
    let numero = a.afirmacoes.len() as u32 + 1;
    a.afirmacoes.push(Afirmacao {
        numero,
        secao: secao.into(),
        texto: texto.into(),
        texto_original: None,
        confianca,
        evidencias: evs,
        estado: EstadoAfirmacao::Registrada,
        antes_de_revalidar: None,
        sessao: None,
        registrada_ms: 0,
    });
}

fn perguntar(a: &mut Analise, texto: &str) {
    let numero = a.perguntas.len() as u32 + 1;
    a.perguntas.push(Pergunta {
        numero,
        texto: texto.into(),
        afirmacao: None,
        estado: EstadoPergunta::Aberta,
        sessao: None,
        registrada_ms: 0,
    });
}

fn avaliar_item(a: &Analise, item: &str) -> Avaliacao {
    let g = gabarito();
    let item = g.item(item).unwrap();
    avaliar(a, &item, &g, &escopo(&g).unwrap())
}

fn regra(av: &Avaliacao, id: &str) -> bool {
    av.regras.iter().find(|r| r.id == id).unwrap().encontrada
}

/// Uma análise de impacto da D-FAT-001 como o gabarito espera.
fn impacto_bom() -> Analise {
    let mut a = analise_vazia("impacto", "Permitir parcelamento ...");
    afirmar(
        &mut a,
        "demanda_como_entendida",
        "Premissa do solicitante: hoje o sistema não parcela fatura nenhuma. Verificação: não confere. \
         O parcelamento já existe para pessoa jurídica e órgão público, em até 6 parcelas.",
        Confianca::Confirmado,
        vec![ev(JAVA, 30, 32, "TipoCliente.PF")],
    );
    afirmar(
        &mut a,
        "como_e_hoje",
        "A quantidade de parcelas vai de 2 até MAX_PARCELAS (hoje 6).",
        Confianca::Confirmado,
        vec![ev(
            JAVA,
            34,
            35,
            "parametros.inteiro(ParametroRepository.MAX_PARCELAS)",
        )],
    );
    afirmar(
        &mut a,
        "riscos",
        "A tela oferece até 12 parcelas, e o backend recusa acima de 6.",
        Confianca::Confirmado,
        vec![
            ev(TELA, 19, 19, "const MAX_PARCELAS = 12;"),
            ev(JAVA, 34, 35, "quantidade > maxParcelas"),
        ],
    );
    afirmar(
        &mut a,
        "abrangencia",
        "Pequena: um parâmetro e a tela.",
        Confianca::Inferido,
        vec![ev(JAVA, 34, 35, "MAX_PARCELAS")],
    );
    perguntar(
        &mut a,
        "O limite de 3 parcelas vale também para órgão público, que hoje usa o mesmo parâmetro?",
    );
    perguntar(
        &mut a,
        "A tela, que oferece até 12 parcelas, deve ser alinhada ao novo limite?",
    );
    a
}

#[test]
fn analise_de_impacto_como_o_gabarito_cobre_tudo_e_sem_falhas() {
    let av = avaliar_item(&impacto_bom(), "D-FAT-001");
    assert!(av.falhas().is_empty(), "{:?}", av.falhas());
    assert_eq!(av.regras_encontradas, 3, "{:?}", av.regras);
    assert_eq!(av.cobertura_de_regras(), 1.0);
    assert_eq!(av.evidencias, av.evidencias_validas);
    assert!(av.concluida);
    let p = av.premissa.as_ref().unwrap();
    assert!(p.corrigida);
    assert_eq!(p.afirmacao, Some(1));
    let ab = av.abrangencia.as_ref().unwrap();
    assert!(ab.confere);
    assert_eq!(ab.registrada.as_deref(), Some("pequena"));
    assert_eq!(av.cobertura_de_perguntas(), Some(1.0));
    assert_eq!(av.perguntas_esperadas[0].atendida_por, Some(1));
    // Os três arquivos impactados: Java e tela citados; o de parâmetros, não.
    assert_eq!(
        av.cobertura_de_arquivos().map(|c| (c * 3.0).round()),
        Some(2.0)
    );
    assert!(av.afirmacoes_fora_do_gabarito.is_empty());
}

#[test]
fn confirmado_sem_evidencia_ou_com_evidencia_que_nao_confere_e_falha_grave() {
    let mut a = impacto_bom();
    afirmar(
        &mut a,
        "como_e_hoje",
        "Afirmação sem evidência.",
        Confianca::Confirmado,
        vec![],
    );
    let mut adulterada = ev(JAVA, 30, 32, "TipoCliente.PF");
    adulterada.trecho = "trecho que não está no arquivo".into();
    afirmar(
        &mut a,
        "como_e_hoje",
        "Afirmação com evidência adulterada.",
        Confianca::Confirmado,
        vec![ev(JAVA, 34, 35, "MAX_PARCELAS"), adulterada],
    );
    let mut hash_errado = ev(JAVA, 14, 14, "MIN_PARCELAS = 2");
    hash_errado.hash = "0".repeat(64);
    afirmar(
        &mut a,
        "como_e_hoje",
        "Inferência com hash que não confere (não é falha grave, mas é evidência inválida).",
        Confianca::Inferido,
        vec![hash_errado],
    );
    let av = avaliar_item(&a, "D-FAT-001");
    assert_eq!(av.confirmado_sem_evidencia_valida, vec![5, 6]);
    assert_eq!(av.evidencias_invalidas.len(), 2);
    assert!(av.evidencias_invalidas[1].motivo.contains("hash"));
    assert_eq!(av.falhas().len(), 1);
    assert!(av.falhas()[0].contains("5, 6"));
}

#[test]
fn analise_vazia_e_falha_e_nao_encontra_nada() {
    let av = avaliar_item(&analise_vazia("perguntar", "?"), "P-FAT-001");
    assert_eq!(av.regras_encontradas, 0);
    assert_eq!(av.regras_esperadas, 7);
    assert!(av.falhas()[0].contains("nenhuma afirmação"));
    assert_eq!(av.cobertura_de_perguntas(), Some(0.0));
    assert!(av.premissa.is_none() && av.abrangencia.is_none());
}

#[test]
fn inconsistencia_exige_os_dois_lados_na_mesma_afirmacao() {
    let mut a = analise_vazia("explicar", "Parcelamento de fatura");
    afirmar(
        &mut a,
        "regras",
        "A tela oferece até 12 parcelas.",
        Confianca::Confirmado,
        vec![ev(TELA, 19, 19, "const MAX_PARCELAS = 12;")],
    );
    afirmar(
        &mut a,
        "regras",
        "O backend aceita até MAX_PARCELAS.",
        Confianca::Confirmado,
        vec![ev(JAVA, 34, 35, "quantidade > maxParcelas")],
    );
    let av = avaliar_item(&a, "F-FAT-002");
    assert!(regra(&av, "RN-FAT-008"));
    assert!(!regra(&av, "INC-FAT-001"));

    afirmar(
        &mut a,
        "excecoes",
        "Divergência: a tela oferece 12, o backend aceita 6.",
        Confianca::Confirmado,
        vec![
            ev(TELA, 19, 19, "const MAX_PARCELAS = 12;"),
            ev(JAVA, 34, 35, "quantidade > maxParcelas"),
        ],
    );
    let av = avaliar_item(&a, "F-FAT-002");
    assert!(regra(&av, "INC-FAT-001"));
    let inc = av.regras.iter().find(|r| r.id == "INC-FAT-001").unwrap();
    assert_eq!(inc.afirmacoes, vec![3]);
}

#[test]
fn evidencia_larga_nao_encontra_regra_e_incorreta_nao_conta() {
    let mut a = analise_vazia("perguntar", "multa");
    // 41 linhas da package de cobrança: cobre multa, juros, carência e isenção.
    afirmar(
        &mut a,
        "regras",
        "A package calcula os encargos.",
        Confianca::Confirmado,
        vec![ev(
            "db/03_pkg_cobranca.sql",
            60,
            100,
            "FN_PARAMETRO('PERC_MULTA_ATRASO')",
        )],
    );
    let av = avaliar_item(&a, "P-FAT-001");
    assert_eq!(av.regras_encontradas, 0);
    assert!(av.falhas().is_empty());
    // Evidência larga não conta para o gabarito, nem para "fora do gabarito".
    assert_eq!(av.afirmacoes_fora_do_gabarito, Vec::<u32>::new());

    afirmar(
        &mut a,
        "regras",
        "Multa de PERC_MULTA_ATRASO.",
        Confianca::Confirmado,
        vec![ev(
            "db/03_pkg_cobranca.sql",
            71,
            72,
            "FN_PARAMETRO('PERC_MULTA_ATRASO') / 100, 2)",
        )],
    );
    assert!(regra(&avaliar_item(&a, "P-FAT-001"), "RN-FAT-001"));
    a.afirmacoes[1].estado = EstadoAfirmacao::Incorreta {
        comentario: "não".into(),
    };
    assert!(!regra(&avaliar_item(&a, "P-FAT-001"), "RN-FAT-001"));
}

#[test]
fn regra_de_outro_item_e_afirmacao_fora_do_gabarito_sao_informadas() {
    let mut a = analise_vazia("perguntar", "Pessoa física pode parcelar?");
    afirmar(
        &mut a,
        "regras",
        "Só fatura aberta pode ser parcelada.",
        Confianca::Confirmado,
        vec![ev(JAVA, 26, 28, "Somente fatura ABERTA pode ser parcelada")],
    );
    afirmar(
        &mut a,
        "regras",
        "O serviço recebe a data do pedido.",
        Confianca::Inferido,
        vec![ev(JAVA, 25, 25, "LocalDate dataPedido")],
    );
    let av = avaliar_item(&a, "P-FAT-002");
    assert_eq!(av.regras_encontradas, 0);
    assert_eq!(av.outras_regras_encontradas, vec!["RN-FAT-010".to_owned()]);
    assert_eq!(av.afirmacoes_fora_do_gabarito, vec![2]);
    // Sem perguntas esperadas no item: não há cobertura de perguntas.
    assert_eq!(av.cobertura_de_perguntas(), None);
}

#[test]
fn premissa_exige_veredito_palavras_e_evidencia() {
    let premissa_sem_evidencia = |texto: &str, evs: Vec<Evidencia>| {
        let mut a = analise_vazia("impacto", "...");
        afirmar(
            &mut a,
            "demanda_como_entendida",
            texto,
            Confianca::Inferido,
            evs,
        );
        avaliar_item(&a, "D-FAT-001").premissa.unwrap().corrigida
    };
    let certo = "Premissa do solicitante: o sistema não parcela fatura nenhuma. \
                 Verificação: não confere. O parcelamento já existe.";
    assert!(premissa_sem_evidencia(
        certo,
        vec![ev(JAVA, 30, 32, "TipoCliente.PF")]
    ));
    assert!(!premissa_sem_evidencia(certo, vec![]));
    assert!(!premissa_sem_evidencia(
        "Premissa do solicitante: o sistema não parcela fatura nenhuma. Verificação: confere.",
        vec![ev(JAVA, 30, 32, "TipoCliente.PF")]
    ));
    // Veredito certo, mas sobre outra premissa.
    assert!(!premissa_sem_evidencia(
        "Premissa do solicitante: a tela é bonita. Verificação: não confere.",
        vec![ev(JAVA, 30, 32, "TipoCliente.PF")]
    ));

    // D-FAT-002 espera "confere em parte"; "não confere" também vale.
    let mut a = analise_vazia("impacto", "...");
    afirmar(
        &mut a,
        "demanda_como_entendida",
        "Premissa do solicitante: basta alterar o parâmetro. Verificação: confere em parte. \
         O lote COBOL usa 2% fixo.",
        Confianca::Confirmado,
        vec![ev("cobol/CALCOBR.cbl", 38, 40, "WS-PERC-MULTA")],
    );
    let av = avaliar_item(&a, "D-FAT-002");
    assert!(av.premissa.as_ref().unwrap().corrigida);
    assert_eq!(
        av.premissa.as_ref().unwrap().veredito_esperado,
        Veredito::ConfereEmParte
    );
    // Sem afirmação de abrangência: registrada nenhuma, não confere.
    assert!(!av.abrangencia.as_ref().unwrap().confere);
}

#[test]
fn veredito_pelo_marcador_ou_pelo_texto() {
    assert_eq!(
        veredito_do_texto("Premissa do solicitante: x. Verificação: não confere. y"),
        Some(Veredito::NaoConfere)
    );
    assert_eq!(
        veredito_do_texto("Premissa: x. **Verificação:** Confere em parte."),
        Some(Veredito::ConfereEmParte)
    );
    assert_eq!(
        veredito_do_texto("Verificação: confere. O sistema não confere o CPF."),
        Some(Veredito::Confere)
    );
    assert_eq!(
        veredito_do_texto("A suposição não confere com o código."),
        Some(Veredito::NaoConfere)
    );
    assert_eq!(veredito_do_texto("Sem veredito."), None);
}

#[test]
fn itens_padrao_existem_com_o_fluxo_certo_e_perguntas_esperadas() {
    let g = gabarito();
    for (id, fluxo) in [
        ("P-FAT-001", "perguntar"),
        ("F-FAT-002", "explicar"),
        ("D-FAT-001", "impacto"),
    ] {
        let item = g.item(id).unwrap();
        assert_eq!(cba_avaliacao::comparacao::nome_do_fluxo(item.fluxo), fluxo);
        assert!(!item.perguntas_esperadas.is_empty(), "{id}");
        assert!(!item.regras.is_empty());
        for r in &item.regras {
            assert!(g.regra(r).is_some(), "{r}");
        }
    }
    assert!(
        g.item("D-FAT-001")
            .unwrap()
            .verificacao_da_premissa
            .is_some()
    );
    assert!(g.item("X-0").is_err());
    assert!(g.raiz_do_sistema().join("db/01_tabelas.sql").is_file());
}

#[test]
fn relatorio_resume_por_fluxo_em_json_e_markdown() {
    let av = avaliar_item(&impacto_bom(), "D-FAT-001");
    let execucao = |r: u32, avaliacao: Option<Avaliacao>| Execucao {
        fluxo: "impacto".into(),
        item: "D-FAT-001".into(),
        repeticao: r,
        avaliacao,
        arquivos_alterados: vec![],
        claude: None,
        erro: None,
    };
    let rel = Relatorio::novo(None, vec![execucao(1, Some(av)), execucao(2, None)]);
    assert!(!rel.sem_falhas_graves);
    let r = &rel.resumo[0];
    assert_eq!(r.execucoes, 2);
    assert_eq!(r.avaliadas, 1);
    assert_eq!(r.regras_media, 0.5);
    assert_eq!(r.regras_min, 0.0);
    assert_eq!(r.regras_max, 1.0);
    assert_eq!(r.premissa_corrigida, Some(1));
    assert_eq!(r.abrangencia_confere, Some(1));

    let dir = tempfile::tempdir().unwrap();
    rel.gravar(dir.path()).unwrap();
    let json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.path().join("relatorio.json")).unwrap()).unwrap();
    assert_eq!(json["resumo"][0]["item"], "D-FAT-001");
    assert_eq!(json["execucoes"][0]["avaliacao"]["regras_encontradas"], 3);
    let md = std::fs::read_to_string(dir.path().join("relatorio.md")).unwrap();
    assert!(md.contains("## Cobertura por fluxo"));
    assert!(md.contains("| impacto | D-FAT-001 | 2 | 50% de 3 (0%–100%)"));
    assert!(md.contains("Premissa corrigida: sim (afirmação 1)"));
}

/// Estados gravados por execuções reais (`tests/estados/<item>.estado.json`).
fn estados_gravados() -> Vec<(String, PathBuf)> {
    let pasta = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/estados");
    let mut v: Vec<(String, PathBuf)> = std::fs::read_dir(&pasta)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.to_string_lossy().ends_with(".estado.json"))
        .map(|p| {
            let nome = p.file_name().unwrap().to_string_lossy().into_owned();
            (nome.trim_end_matches(".estado.json").to_owned(), p)
        })
        .collect();
    v.sort();
    v
}

#[test]
fn estados_de_execucoes_reais_continuam_sem_falhas() {
    let estados = estados_gravados();
    assert!(!estados.is_empty(), "nenhum estado em tests/estados");
    for (item, caminho) in estados {
        let analise: Analise = serde_json::from_slice(&std::fs::read(&caminho).unwrap()).unwrap();
        let av = avaliar_item(&analise, &item);
        assert!(av.falhas().is_empty(), "{item}: {:?}", av.falhas());
        assert!(
            av.evidencias_invalidas.is_empty(),
            "{item}: {:?}",
            av.evidencias_invalidas
        );
        assert!(av.regras_encontradas > 0, "{item}");
    }
}
