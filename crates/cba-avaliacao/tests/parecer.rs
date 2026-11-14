//! Comparação do parecer de revisão (fluxo `revisar`, #116) com o gabarito: um
//! parecer bom (a contradição plantada com evidência, os termos vagos apontados e
//! o veredito esperado) e um ruim (correção sem evidência, termos esquecidos).

use cba_avaliacao::comparacao::avaliar;
use cba_avaliacao::execucao::escopo;
use cba_avaliacao::gabarito::Gabarito;
use cba_avaliacao::gabarito_padrao;
use cba_avaliacao::relatorio::{Execucao, Relatorio};
use cba_core::analise::{Analise, Evidencia};
use cba_core::parecer::{NovoAchado, Veredito};
use cba_core::qualidade::{Caracteristica, Severidade};
use cba_kb::Id;
use cba_tools::evidencias::{EvidenciaCitada, validar};

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

fn revisao(entrada: &str) -> Analise {
    let mut a: Analise = serde_json::from_value(serde_json::json!({
        "formato": 1,
        "id": "A-0001",
        "projeto": "faturamento-exemplo",
        "fluxo": "revisar",
        "titulo": "Revisão",
        "entrada": entrada,
        "estado": "em-andamento",
        "documento": "analises/A-0001.md",
        "versao": 1,
        "criada_ms": 0,
        "atualizada_ms": 0
    }))
    .unwrap();
    a.concluir().unwrap();
    a
}

fn achado(
    a: &mut Analise,
    caracteristica: Caracteristica,
    severidade: Severidade,
    trecho: &str,
    problema: &str,
    evidencias: Vec<Evidencia>,
) {
    a.registrar_achado(
        NovoAchado {
            caracteristica,
            severidade,
            trecho: trecho.into(),
            problema: problema.into(),
            reescrita: None,
            regras: Vec::new(),
            evidencias,
            sessao: None,
        },
        0,
    )
    .unwrap();
}

#[test]
fn parecer_bom_atende_o_gabarito() {
    let g = gabarito();
    let item = g.item("REV-FAT-001").unwrap();
    let mut a = revisao(&item.entrada);
    achado(
        &mut a,
        Caracteristica::Correcao,
        Severidade::Alta,
        "inclusive a que já tem pagamento parcial",
        "Contradiz a regra: o sistema recusa o cancelamento de fatura com valor pago.",
        vec![ev(
            "db/03_pkg_cobranca.sql",
            115,
            117,
            "IF V_VL_PAGO > 0 THEN",
        )],
    );
    achado(
        &mut a,
        Caracteristica::Ambiguidade,
        Severidade::Media,
        "deve ser rápido e a mensagem de confirmação, adequada",
        "Termos sem medida.",
        Vec::new(),
    );
    achado(
        &mut a,
        Caracteristica::Ambiguidade,
        Severidade::Media,
        "avulsas e/ou parceladas, etc.",
        "Lista aberta.",
        Vec::new(),
    );
    achado(
        &mut a,
        Caracteristica::Completude,
        Severidade::Baixa,
        "",
        "O sistema exige motivo com ao menos 10 caracteres.",
        vec![ev(
            "db/03_pkg_cobranca.sql",
            119,
            121,
            "LENGTH(TRIM(P_MOTIVO)) < 10",
        )],
    );
    a.registrar_veredito(Veredito::PrecisaDeRetrabalho, "Contradiz a regra.", 0)
        .unwrap();
    a.concluir().unwrap();

    let av = avaliar(&a, &item, &g, &escopo(&g).unwrap());
    assert!(av.falhas().is_empty(), "{:?}", av.falhas());
    let p = av.parecer.as_ref().unwrap();
    assert!(p.veredito_confere());
    let por_id = |id: &str| p.achados.iter().find(|x| x.id == id).unwrap().atendido_por;
    assert_eq!(por_id("REV-FAT-001-C1"), Some(1));
    assert_eq!(por_id("REV-FAT-001-L1"), Some(4));
    assert_eq!(por_id("REV-FAT-001-L2"), None);
    assert_eq!(p.termos_apontados(), 4, "{:?}", p.termos_vagos);
    assert!(p.correcao_sem_evidencia_valida.is_empty());

    let relatorio = Relatorio::novo(
        None,
        vec![Execucao {
            fluxo: "revisar".into(),
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
    assert!(md.contains("Achados esperados: 2 de 3"), "{md}");
    assert!(md.contains("REV-FAT-001-L2: não encontrado"), "{md}");
    assert!(md.contains("Termos vagos apontados: 4 de 4"), "{md}");
    assert!(
        md.contains(
            "Veredito: esperado precisa de retrabalho, registrado precisa de retrabalho (confere)"
        ),
        "{md}"
    );
    assert!(relatorio.sem_falhas_graves);
}

#[test]
fn correcao_sem_evidencia_valida_e_falha_grave() {
    let g = gabarito();
    let item = g.item("REV-FAT-001").unwrap();
    let mut a = revisao(&item.entrada);
    // A evidência gravada não confere mais (hash adulterado).
    let mut adulterada = ev("db/03_pkg_cobranca.sql", 115, 117, "IF V_VL_PAGO > 0 THEN");
    adulterada.hash = "0".repeat(64);
    achado(
        &mut a,
        Caracteristica::Correcao,
        Severidade::Alta,
        "inclusive a que já tem pagamento parcial",
        "O sistema não cancela fatura com pagamento parcial.",
        vec![adulterada],
    );
    // A regra citada pelo id conta mesmo sem evidência que a toque.
    a.parecer.as_mut().unwrap().achados[0].regras = vec![Id::analisar("RN-FAT-012").unwrap()];
    a.registrar_veredito(Veredito::ProntaComRessalvas, "Ajustes.", 0)
        .unwrap();

    let av = avaliar(&a, &item, &g, &escopo(&g).unwrap());
    let falhas = av.falhas();
    assert_eq!(falhas.len(), 1, "{falhas:?}");
    assert!(
        falhas[0].contains("correção sem evidência válida: 1"),
        "{falhas:?}"
    );
    let p = av.parecer.unwrap();
    assert!(!p.veredito_confere());
    assert_eq!(p.evidencias_invalidas.len(), 1);
    assert_eq!(p.achados[0].atendido_por, Some(1));
    assert_eq!(p.termos_apontados(), 0);

    // Sem nenhum achado, também é falha grave.
    let vazia = revisao(&item.entrada);
    let av = avaliar(&vazia, &item, &g, &escopo(&g).unwrap());
    assert_eq!(av.falhas(), ["nenhum achado registrado"]);
}
