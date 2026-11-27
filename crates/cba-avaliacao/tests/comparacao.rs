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
    Afirmacao, Analise, Confianca, EstadoAfirmacao, EstadoPergunta, Evidencia, EvidenciaOrigem,
    OpcaoDecisao, Pergunta, TipoPergunta,
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
        "projeto": "faturamento-exemplo",
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
        dados: Vec::new(),
        origens: Vec::new(),
        estado: EstadoAfirmacao::Registrada,
        antes_de_revalidar: None,
        contestacao: None,
        sessao: None,
        registrada_ms: 0,
    });
}

fn perguntar(a: &mut Analise, texto: &str) {
    let numero = a.perguntas.len() as u32 + 1;
    a.perguntas.push(Pergunta {
        numero,
        texto: texto.into(),
        tipo: TipoPergunta::Duvida,
        opcoes: Vec::new(),
        recomendacao: None,
        afirmacao: None,
        estado: EstadoPergunta::Aberta,
        resposta_enviada: false,
        sessao: None,
        registrada_ms: 0,
        origem: None,
        evidencias: vec![],
        destinatario: None,
        informada: None,
        verificacao: None,
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
        "Premissa do solicitante: hoje o projeto não parcela fatura nenhuma. Verificação: não confere. \
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
    let certo = "Premissa do solicitante: o projeto não parcela fatura nenhuma. \
                 Verificação: não confere. O parcelamento já existe.";
    assert!(premissa_sem_evidencia(
        certo,
        vec![ev(JAVA, 30, 32, "TipoCliente.PF")]
    ));
    assert!(!premissa_sem_evidencia(certo, vec![]));
    assert!(!premissa_sem_evidencia(
        "Premissa do solicitante: o projeto não parcela fatura nenhuma. Verificação: confere.",
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
        veredito_do_texto("Verificação: confere. O projeto não confere o CPF."),
        Some(Veredito::Confere)
    );
    assert_eq!(
        veredito_do_texto("A suposição não confere com o código."),
        Some(Veredito::NaoConfere)
    );
    assert_eq!(veredito_do_texto("Sem veredito."), None);
}

const TABELAS: &str = "db/01_tabelas.sql";
const CONTATO: &str = "db/05_cliente_contato.sql";
const LOTE: &str = "cobol/CALCOBR.cbl";

/// Uma análise de impacto da D-FAT-004 (#126) com a seção de dados pessoais e
/// requisitos não funcionais como o gabarito espera.
fn impacto_com_dados_pessoais() -> Analise {
    let mut a = analise_vazia("impacto", "Incluir o nome, o CPF e o e-mail ...");
    afirmar(
        &mut a,
        "demanda_como_entendida",
        "Premissa do solicitante: é só acrescentar os campos no arquivo. Verificação: não confere. \
         O lote lê o arquivo de faturas vencidas (FATVENC), que não traz o cliente.",
        Confianca::Confirmado,
        vec![ev(LOTE, 21, 26, "01  REG-FATURA.")],
    );
    afirmar(
        &mut a,
        "dados_pessoais_e_rnf",
        "Campo CLIENTE.NOME: nome (dado pessoal). Finalidade aparente: identificar o cliente. \
         Onde aparece: passa a sair no arquivo de cobrança.",
        Confianca::Confirmado,
        vec![ev(TABELAS, 9, 9, "NOME")],
    );
    afirmar(
        &mut a,
        "dados_pessoais_e_rnf",
        "Campo CLIENTE.DOCUMENTO: CPF quando TIPO_CLIENTE = 'PF' (dado pessoal); CNPJ para PJ.",
        Confianca::Confirmado,
        vec![
            ev(TABELAS, 10, 12, "DOCUMENTO"),
            ev("db/02_parametros.sql", 23, 23, "'000.000.000-00'"),
        ],
    );
    afirmar(
        &mut a,
        "dados_pessoais_e_rnf",
        "Campo CLIENTE_CONTATO.EMAIL: e-mail (contato), opcional.",
        Confianca::Confirmado,
        vec![ev(CONTATO, 8, 8, "EMAIL")],
    );
    afirmar(
        &mut a,
        "dados_pessoais_e_rnf",
        "Auditoria: o lote CALCOBR não grava log do que envia; só exibe os contadores.",
        Confianca::Confirmado,
        vec![ev(LOTE, 56, 58, "DISPLAY 'CALCOBR - LIDAS....: '")],
    );
    afirmar(
        &mut a,
        "dados_pessoais_e_rnf",
        "A coluna ISENTO_ENCARGOS também é dado pessoal.",
        Confianca::Inferido,
        vec![ev(TABELAS, 14, 14, "ISENTO_ENCARGOS")],
    );
    perguntar(
        &mut a,
        "Qual o volume de faturas vencidas por mês e a janela do lote mensal, agora com a leitura do cliente?",
    );
    perguntar(
        &mut a,
        "Quem tem acesso ao arquivo enviado à empresa terceirizada, e com que controle?",
    );
    a
}

#[test]
fn dados_pessoais_campos_aspectos_e_falso_positivo() {
    let av = avaliar_item(&impacto_com_dados_pessoais(), "D-FAT-004");
    assert!(av.falhas().is_empty(), "{:?}", av.falhas());
    let d = av.dados_pessoais.as_ref().unwrap();
    assert_eq!(d.afirmacoes, 5);
    let apontados: Vec<(&str, Option<u32>)> = d
        .campos
        .iter()
        .map(|c| (c.campo.as_str(), c.apontado_por))
        .collect();
    assert_eq!(
        apontados,
        [
            ("CLIENTE.NOME", Some(2)),
            ("CLIENTE.DOCUMENTO", Some(3)),
            ("CLIENTE_CONTATO.EMAIL", Some(4)),
        ]
    );
    // TIPO_CLIENTE aparece na afirmação do CPF, que aponta um campo esperado: não
    // é falso positivo. ISENTO_ENCARGOS, sozinho, é.
    assert_eq!(d.falsos_positivos.len(), 1, "{:?}", d.falsos_positivos);
    assert_eq!(d.falsos_positivos[0].campo, "CLIENTE.ISENTO_ENCARGOS");
    assert_eq!(d.falsos_positivos[0].afirmacoes, [6]);
    let aspectos: Vec<(&str, Option<u32>, Option<u32>)> = d
        .aspectos
        .iter()
        .map(|a| (a.aspecto.as_str(), a.afirmacao, a.pergunta))
        .collect();
    assert_eq!(
        aspectos,
        [
            ("auditoria", Some(5), None),
            ("desempenho", None, Some(1)),
            ("acesso", None, Some(2)),
        ]
    );
    assert_eq!(d.aspectos_atendidos(), 3);
    assert!(regra(&av, "RN-FAT-016"));
}

#[test]
fn campo_pessoal_exige_evidencia_que_toca_a_coluna() {
    let mut a = analise_vazia("impacto", "Incluir o nome ...");
    // Cita o nome, mas a evidência é de outra tabela; e o e-mail sem evidência.
    afirmar(
        &mut a,
        "dados_pessoais_e_rnf",
        "Campo CLIENTE.NOME: nome do cliente.",
        Confianca::Inferido,
        vec![ev(TABELAS, 22, 22, "ID_CLIENTE")],
    );
    afirmar(
        &mut a,
        "dados_pessoais_e_rnf",
        "Campo CLIENTE_CONTATO.EMAIL: e-mail.",
        Confianca::AConfirmar,
        vec![],
    );
    // Em outra seção, não conta.
    afirmar(
        &mut a,
        "dados",
        "Campo CLIENTE.DOCUMENTO: CPF.",
        Confianca::Confirmado,
        vec![ev(TABELAS, 10, 10, "DOCUMENTO")],
    );
    let av = avaliar_item(&a, "D-FAT-004");
    let d = av.dados_pessoais.as_ref().unwrap();
    assert_eq!(d.campos_apontados(), 0, "{:?}", d.campos);
    assert_eq!(d.aspectos_atendidos(), 0);
    // Os outros itens de impacto não avaliam a seção.
    assert!(
        avaliar_item(&impacto_bom(), "D-FAT-001")
            .dados_pessoais
            .is_none()
    );

    let execucao = Execucao {
        fluxo: "impacto".into(),
        item: "D-FAT-004".into(),
        repeticao: 1,
        avaliacao: Some(avaliar_item(&impacto_com_dados_pessoais(), "D-FAT-004")),
        ata: None,
        transcript: None,
        mapa: None,
        publicacao: None,
        conducao: None,
        arquivos_alterados: vec![],
        claude: None,
        erro: None,
    };
    let dir = tempfile::tempdir().unwrap();
    Relatorio::novo(None, vec![execucao])
        .gravar(dir.path())
        .unwrap();
    let md = std::fs::read_to_string(dir.path().join("relatorio.md")).unwrap();
    assert!(
        md.contains("campos pessoais apontados 3 de 3; aspectos tratados 3 de 3"),
        "{md}"
    );
    assert!(md.contains("  - CLIENTE.NOME (nome): afirmação 2"), "{md}");
    assert!(md.contains("  - desempenho ("), "{md}");
    assert!(
        md.contains("Possível falso positivo: CLIENTE.ISENTO_ENCARGOS (afirmações 6)"),
        "{md}"
    );
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
    assert!(g.raiz_do_projeto().join("db/01_tabelas.sql").is_file());
}

#[test]
fn relatorio_resume_por_fluxo_em_json_e_markdown() {
    let av = avaliar_item(&impacto_bom(), "D-FAT-001");
    let execucao = |r: u32, avaliacao: Option<Avaliacao>| Execucao {
        fluxo: "impacto".into(),
        item: "D-FAT-001".into(),
        repeticao: r,
        avaliacao,
        ata: None,
        transcript: None,
        mapa: None,
        publicacao: None,
        conducao: None,
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

/// D-FAT-005 (#450): a análise cita a DEM-12, que criou a RN-FAT-012 de propósito,
/// em "Como é hoje" e pergunta se a regra continua valendo.
#[test]
fn origem_da_regra_citada_em_como_e_hoje_e_a_pergunta_se_continua_valendo() {
    let sem = analise_vazia("impacto", "Corrigir o cancelamento ...");
    let av = avaliar_item(&sem, "D-FAT-005");
    let o = av.origem.as_ref().unwrap();
    assert_eq!(
        (o.chave.as_str(), o.regra.as_str()),
        ("DEM-12", "RN-FAT-012")
    );
    assert_eq!(o.citada_em, None);
    assert!(!o.com_commit);

    let mut a = analise_vazia("impacto", "Corrigir o cancelamento ...");
    afirmar(
        &mut a,
        "como_e_hoje",
        "Fatura com pagamento parcial não pode ser cancelada. A regra foi criada de \
         propósito pela demanda DEM-12, em 2024-03-12.",
        Confianca::Confirmado,
        vec![ev(
            "db/03_pkg_cobranca.sql",
            115,
            117,
            "IF V_VL_PAGO > 0 THEN",
        )],
    );
    perguntar(
        &mut a,
        "A regra da DEM-12, que impede cancelar fatura com pagamento parcial, continua valendo?",
    );
    perguntar(
        &mut a,
        "Cancelada a fatura, o valor já pago será estornado ou fica como crédito?",
    );
    let av = avaliar_item(&a, "D-FAT-005");
    assert!(av.falhas().is_empty(), "{:?}", av.falhas());
    assert!(regra(&av, "RN-FAT-012"));
    let o = av.origem.as_ref().unwrap();
    assert_eq!(o.citada_em, Some(1));
    assert!(!o.com_commit);
    assert!(
        av.perguntas_esperadas
            .iter()
            .all(|p| p.atendida_por.is_some()),
        "{:?}",
        av.perguntas_esperadas
    );

    // Com o commit em `origens`, sem a chave no texto.
    let mut b = analise_vazia("impacto", "Corrigir o cancelamento ...");
    afirmar(
        &mut b,
        "como_e_hoje",
        "Fatura com pagamento parcial não pode ser cancelada, por pedido do financeiro.",
        Confianca::Confirmado,
        vec![ev(
            "db/03_pkg_cobranca.sql",
            115,
            117,
            "IF V_VL_PAGO > 0 THEN",
        )],
    );
    b.afirmacoes[0].origens.push(EvidenciaOrigem {
        commit: "0123456789abcdef0123456789abcdef01234567".into(),
        data: "2024-03-12".into(),
        assunto: "[DEM-12] Impedir o cancelamento de fatura com pagamento parcial".into(),
        chaves: vec!["DEM-12".into()],
        autor: None,
    });
    let o = avaliar_item(&b, "D-FAT-005").origem.unwrap();
    assert_eq!(o.citada_em, Some(1));
    assert!(o.com_commit);
}

const CLIENTE_SERVICE: &str = "backend/src/main/java/exemplo/faturamento/ClienteService.java";

/// D-FAT-006 (#451): a análise lista os caminhos que gravam CANCELADA (a tela da
/// retaguarda, o portal e a edição do cliente) e aponta, em riscos, o que não
/// valida.
#[test]
fn caminhos_que_gravam_o_estado_e_o_que_nao_valida() {
    let sem = analise_vazia("impacto", "Desativar a rota antiga ...");
    let c = avaliar_item(&sem, "D-FAT-006").caminhos.unwrap();
    assert_eq!(c.caminhos.len(), 3);
    assert_eq!(c.citados(), 0);
    assert!(!c.completos());
    assert_eq!(c.lista_em, None);

    let mut a = analise_vazia("impacto", "Desativar a rota antiga ...");
    afirmar(
        &mut a,
        "como_e_hoje",
        "Caminhos que gravam a situação CANCELADA: 1. tela da retaguarda → \
         PRC_CANCELAR_FATURA: valida a situação, o pagamento parcial e o motivo; 2. portal \
         → CancelamentoService.cancelar: valida o mesmo; 3. edição do cliente, ao desligá-lo \
         → ClienteService.editar: não valida. A lista é completa.",
        Confianca::Confirmado,
        vec![ev(
            "db/03_pkg_cobranca.sql",
            123,
            127,
            "SET STATUS          = 'CANCELADA'",
        )],
    );
    // O desligamento sem dizer o que não valida não basta.
    afirmar(
        &mut a,
        "riscos",
        "O desligamento do cliente cancela as faturas abertas dele.",
        Confianca::Confirmado,
        vec![ev(
            CLIENTE_SERVICE,
            44,
            49,
            "\"DESLIGADO\".equals(cliente.situacao())",
        )],
    );
    let c = avaliar_item(&a, "D-FAT-006").caminhos.unwrap();
    assert_eq!(c.citados(), 3, "{c:?}");
    assert_eq!(c.lista_em, Some(1));
    assert!(!c.completos(), "{c:?}");
    assert_eq!(c.caminhos[2].sem_validacao_em, None);

    afirmar(
        &mut a,
        "riscos",
        "O cancelamento chamado pela edição do cliente passa a validação desligada: \
         fatura com pagamento parcial é cancelada sem a recusa da RN-FAT-012.",
        Confianca::Confirmado,
        vec![ev(
            CLIENTE_SERVICE,
            47,
            47,
            "cancelamentoService.cancelar(idFatura, MOTIVO_DESLIGAMENTO, usuario, false)",
        )],
    );
    let av = avaliar_item(&a, "D-FAT-006");
    let c = av.caminhos.as_ref().unwrap();
    assert!(c.completos(), "{c:?}");
    assert_eq!(c.caminhos[2].sem_validacao_em, Some(3));
    assert!(c.caminhos[0].valida && c.caminhos[1].valida && !c.caminhos[2].valida);
    // Em como_e_hoje, a falta de validação não conta: precisa ser item de riscos
    // ou de funcionalidades.
    let mut b = analise_vazia("impacto", "Desativar a rota antiga ...");
    afirmar(
        &mut b,
        "como_e_hoje",
        "A edição do cliente cancela sem validar o pagamento parcial.",
        Confianca::Confirmado,
        vec![ev(
            CLIENTE_SERVICE,
            47,
            47,
            "cancelamentoService.cancelar(idFatura, MOTIVO_DESLIGAMENTO, usuario, false)",
        )],
    );
    let c = avaliar_item(&b, "D-FAT-006").caminhos.unwrap();
    assert_eq!(c.caminhos[2].citado_em, Some(1));
    assert_eq!(c.caminhos[2].sem_validacao_em, None);

    // O relatório da execução mostra os caminhos.
    let rel = Relatorio::novo(
        None,
        vec![Execucao {
            fluxo: "impacto".into(),
            item: "D-FAT-006".into(),
            repeticao: 1,
            avaliacao: Some(av),
            ata: None,
            transcript: None,
            mapa: None,
            publicacao: None,
            conducao: None,
            arquivos_alterados: vec![],
            claude: None,
            erro: None,
        }],
    );
    let dir = tempfile::tempdir().unwrap();
    rel.gravar(dir.path()).unwrap();
    let md = std::fs::read_to_string(dir.path().join("relatorio.md")).unwrap();
    assert!(
        md.contains("Caminhos que gravam FATURA.STATUS = CANCELADA: 3 de 3 citados"),
        "{md}"
    );
    assert!(
        md.contains("falta de validação apontada (afirmação 3)"),
        "{md}"
    );
}

/// D-FAT-005 (#452): manter ou revogar a regra da DEM-12 é uma decisão para o
/// negócio, com as opções; registrada só como dúvida, o relatório mostra.
#[test]
fn decisao_esperada_registrada_como_decisao_com_opcoes() {
    let texto =
        "A regra da DEM-12, que impede cancelar fatura com pagamento parcial, continua valendo?";
    let mut a = analise_vazia("impacto", "Corrigir o cancelamento ...");
    perguntar(&mut a, texto);
    let av = avaliar_item(&a, "D-FAT-005");
    assert_eq!(av.decisoes_esperadas.len(), 1);
    let d = &av.decisoes_esperadas[0];
    assert_eq!(d.atendida_por, None);
    assert_eq!(d.so_como_duvida, Some(1));
    let execucao = Execucao {
        fluxo: "impacto".into(),
        item: "D-FAT-005".into(),
        repeticao: 1,
        avaliacao: Some(av),
        ata: None,
        transcript: None,
        mapa: None,
        publicacao: None,
        conducao: None,
        arquivos_alterados: vec![],
        claude: None,
        erro: None,
    };
    let dir = tempfile::tempdir().unwrap();
    Relatorio::novo(None, vec![execucao])
        .gravar(dir.path())
        .unwrap();
    let md = std::fs::read_to_string(dir.path().join("relatorio.md")).unwrap();
    assert!(md.contains("- Decisões para o negócio esperadas:"), "{md}");
    assert!(
        md.contains("só como dúvida (pergunta 1, sem opções)"),
        "{md}"
    );

    // Como decisão, com as duas opções e o efeito de cada uma.
    a.perguntas[0].tipo = TipoPergunta::Decisao;
    a.perguntas[0].opcoes = vec![
        OpcaoDecisao {
            texto: "Manter a regra".into(),
            efeito: "a fatura com pagamento parcial segue sem cancelamento".into(),
        },
        OpcaoDecisao {
            texto: "Revogar a regra".into(),
            efeito: "revoga a regra da DEM-12; decidir o destino do valor pago".into(),
        },
    ];
    let av = avaliar_item(&a, "D-FAT-005");
    assert_eq!(av.decisoes_esperadas[0].atendida_por, Some(1));
    // A decisão também atende à pergunta esperada sobre a regra.
    assert_eq!(av.perguntas_esperadas[0].atendida_por, Some(1));

    // Descartada, não conta.
    a.perguntas[0].estado = EstadoPergunta::Descartada;
    let av = avaliar_item(&a, "D-FAT-005");
    assert_eq!(av.decisoes_esperadas[0].atendida_por, None);
}

/// D-FAT-007 (#455): o critério usa parte dos valores de FATURA.STATUS. A análise
/// cita os seis valores, com a evidência do CHECK mais recente, classifica cada
/// um e registra a decisão dos que ficam a decidir.
#[test]
fn valores_de_dominio_lista_completa_classificada_e_decisoes() {
    let sem = analise_vazia("impacto", "Cancelar ABERTA e PARCELADA ...");
    let v = avaliar_item(&sem, "D-FAT-007").valores_de_dominio.unwrap();
    assert_eq!(v.campo, "FATURA.STATUS");
    assert_eq!(v.valores.len(), 6);
    assert_eq!(v.classificados(), 0);
    assert_eq!(v.lista_em, None);
    assert!(!v.completos());

    // A lista do script 01 (quatro valores) não é a completa.
    let mut a = analise_vazia("impacto", "Cancelar ABERTA e PARCELADA ...");
    afirmar(
        &mut a,
        "como_e_hoje",
        "Valores de FATURA.STATUS: ABERTA: entra; PAGA: não entra; PARCELADA: entra; \
         CANCELADA: não entra.",
        Confianca::Confirmado,
        vec![ev(
            "db/01_tabelas.sql",
            35,
            36,
            "CHECK (STATUS IN ('ABERTA', 'PAGA', 'PARCELADA', 'CANCELADA'))",
        )],
    );
    let v = avaliar_item(&a, "D-FAT-007").valores_de_dominio.unwrap();
    assert_eq!(v.lista_em, None);
    assert_eq!(v.classificados(), 4, "{v:?}");
    assert!(!v.completos());

    // A lista completa, com a evidência do CHECK do script 04; SUSPENSA a decidir,
    // mas PROTESTADA dada como fora sem decisão.
    let mut a = analise_vazia("impacto", "Cancelar ABERTA e PARCELADA ...");
    afirmar(
        &mut a,
        "como_e_hoje",
        "Valores de FATURA.STATUS: ABERTA: entra (já cancelada hoje); PAGA: não entra \
         (quitada); PARCELADA: entra (pedido da demanda); CANCELADA: não entra (já \
         cancelada); SUSPENSA: a decidir (cobrança suspensa por contestação); \
         PROTESTADA: não entra.",
        Confianca::Confirmado,
        vec![ev(
            "db/04_pkg_situacao.sql",
            9,
            10,
            "'SUSPENSA', 'PROTESTADA'",
        )],
    );
    let v = avaliar_item(&a, "D-FAT-007").valores_de_dominio.unwrap();
    assert_eq!(v.lista_em, Some(1));
    assert!(v.lista_com_evidencia);
    assert_eq!(v.classificados(), 5, "{v:?}");
    let suspensa = v.valores.iter().find(|x| x.valor == "SUSPENSA").unwrap();
    assert_eq!(suspensa.decisao_em, None);
    assert!(!suspensa.atendido());
    assert!(!v.completos());

    // Corrigida a classificação e registradas as decisões, fica completo.
    a.afirmacoes[0].texto = a.afirmacoes[0].texto.replace(
        "PROTESTADA: não entra.",
        "PROTESTADA: a decidir (em protesto).",
    );
    perguntar(
        &mut a,
        "No desligamento, cancelar também as faturas SUSPENSA (em contestação) e PROTESTADA?",
    );
    a.perguntas[0].tipo = TipoPergunta::Decisao;
    a.perguntas[0].opcoes = vec![
        OpcaoDecisao {
            texto: "Cancelar".into(),
            efeito: "a cobrança das faturas suspensas e protestadas acaba no desligamento".into(),
        },
        OpcaoDecisao {
            texto: "Manter em cobrança".into(),
            efeito: "seguem cobradas depois do desligamento".into(),
        },
    ];
    let av = avaliar_item(&a, "D-FAT-007");
    let v = av.valores_de_dominio.as_ref().unwrap();
    assert!(v.completos(), "{v:?}");
    assert!(v.valores.iter().filter(|x| x.decisao_em.is_some()).count() == 2);
    assert_eq!(av.decisoes_esperadas[0].atendida_por, Some(1));

    // Só como dúvida, sem opções, a decisão não conta.
    let mut b = a.clone();
    b.perguntas[0].tipo = TipoPergunta::Duvida;
    b.perguntas[0].opcoes.clear();
    let v = avaliar_item(&b, "D-FAT-007").valores_de_dominio.unwrap();
    assert!(!v.completos());

    // O relatório da execução mostra a lista e a classificação.
    let rel = Relatorio::novo(
        None,
        vec![Execucao {
            fluxo: "impacto".into(),
            item: "D-FAT-007".into(),
            repeticao: 1,
            avaliacao: Some(av),
            ata: None,
            transcript: None,
            mapa: None,
            publicacao: None,
            conducao: None,
            arquivos_alterados: vec![],
            claude: None,
            erro: None,
        }],
    );
    let dir = tempfile::tempdir().unwrap();
    rel.gravar(dir.path()).unwrap();
    let md = std::fs::read_to_string(dir.path().join("relatorio.md")).unwrap();
    assert!(
        md.contains(
            "Valores de FATURA.STATUS usados como critério: 6 de 6 classificados como o \
             esperado; lista completa na afirmação 1, com evidência"
        ),
        "{md}"
    );
    assert!(
        md.contains("PROTESTADA (esperado: a decidir): a decidir (afirmação 1); decisão 1"),
        "{md}"
    );
}
