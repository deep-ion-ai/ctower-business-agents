//! Testes da lista e do painel da demanda (#138), com o sistema fictício
//! "Faturamento Exemplo".

use super::*;
use crate::analise::carregar_analise;
use crate::teste_base::{PROJETO, cenario};
use cba_docs::{NovoItem, Templates};
use cba_kb::frontmatter::Data;
use cba_session::sessoes::RegistroSessao;

fn nova(titulo: &str, chave: Option<&str>) -> NovaDemanda {
    NovaDemanda {
        titulo: titulo.into(),
        descricao: Some("O cliente quer parcelar a fatura em até 3 vezes.".into()),
        chave: chave.map(str::to_owned),
        origem: None,
    }
}

/// Ata da demanda na data dada, com uma pendência, uma dúvida e um próximo passo.
fn ata(c: &crate::teste_base::Cenario, demanda: &str, data: Data, aprovada: bool) -> String {
    let amb = c.ambiente();
    let id = IdDemanda::analisar(demanda).unwrap();
    let (mut estado, _) = EstadoAta::criar(&amb.kb, id, data).unwrap();
    estado.definir_objetivo("Entender o parcelamento para pessoa jurídica");
    let mut p = NovoItem::novo(
        TipoItem::Pendencia,
        "Enviar a tabela de juros do parcelamento",
    );
    p.responsavel = Some("Ana (PO)".into());
    p.prazo = Some("2026-11-20".into());
    estado.adicionar_item(p).unwrap();
    estado
        .adicionar_item(NovoItem::novo(
            TipoItem::Duvida,
            "O parcelamento vale para fatura já vencida?",
        ))
        .unwrap();
    estado
        .adicionar_item(NovoItem::novo(
            TipoItem::ProximoPasso,
            "Analisar o impacto no cálculo da multa",
        ))
        .unwrap();
    estado
        .adicionar_item(NovoItem::novo(TipoItem::Decisao, "Até 3 parcelas"))
        .unwrap();
    if aprovada {
        estado.status = StatusAta::Aprovada;
    }
    Templates::da_base(&amb.base)
        .0
        .gravar_ata(&amb.kb, &estado)
        .unwrap();
    estado.id.to_string()
}

#[test]
fn lista_cria_encerra_e_reabre() {
    let c = cenario();
    assert!(listar_demandas(&c.dados, PROJETO).unwrap().is_empty());

    let d1 = criar_demanda(&c.dados, PROJETO, &nova("Parcelamento PJ", Some("FAT-12"))).unwrap();
    let d2 = criar_demanda(&c.dados, PROJETO, &nova("Multa de fatura vencida", None)).unwrap();
    assert_eq!(d1.id, "D-0001");
    assert_eq!(d1.chave.as_deref(), Some("FAT-12"));
    assert_eq!(d1.status, StatusDemanda::Aberta);
    assert!(
        d1.documento
            .starts_with(&format!("{PROJETO}/demandas/D-0001-"))
    );

    let encerrada = mudar_status_demanda(&c.dados, PROJETO, "D-0002", true).unwrap();
    assert_eq!(encerrada.status, StatusDemanda::Encerrada);
    assert!(encerrada.encerrado.is_some());

    // Abertas primeiro.
    let lista = listar_demandas(&c.dados, PROJETO).unwrap();
    let ids: Vec<_> = lista.iter().map(|d| d.id.as_str()).collect();
    assert_eq!(ids, [d1.id.as_str(), d2.id.as_str()]);

    let reaberta = mudar_status_demanda(&c.dados, PROJETO, "D-0002", false).unwrap();
    assert_eq!(reaberta.status, StatusDemanda::Aberta);
    assert_eq!(reaberta.encerrado, None);

    // Formato que a tela espera (ui/src/demandas.ts).
    let json = serde_json::to_value(&reaberta).unwrap();
    assert_eq!(json["status"], "aberta");
    assert_eq!(json["encerrado"], serde_json::Value::Null);
    assert!(json["documento"].is_string());

    assert!(mudar_status_demanda(&c.dados, PROJETO, "X-1", true).is_err());
}

/// Critérios de aceite da #138: a ata e a análise aparecem na linha do tempo, em
/// ordem; a análise criada pelo painel grava `demanda.id`.
#[test]
fn linha_do_tempo_com_ata_analise_e_sessao_em_ordem() {
    let c = cenario();
    let d = criar_demanda(&c.dados, PROJETO, &nova("Parcelamento PJ", Some("FAT-12"))).unwrap();
    // Reunião antes da análise e outra depois (data futura): a ordem é a das datas,
    // não a de criação.
    let antes = ata(&c, &d.id, Data::nova(2026, 1, 10).unwrap(), false);
    let criada = criar_analise_ligada(
        &c.dados,
        PROJETO,
        &d.id,
        Fluxo::Impacto,
        "Permitir parcelamento da fatura em até 3 vezes para clientes pessoa jurídica.",
        None,
    )
    .unwrap();
    let depois = ata(&c, &d.id, Data::nova(2999, 1, 1).unwrap(), false);
    // Análise sem demanda: fica de fora.
    crate::analise::criar_analise(&c.dados, PROJETO, "Como é calculada a multa?", None).unwrap();

    // Sessão do Claude ligada à análise (registro dos hooks).
    let id_analise = cba_kb::Id::analisar(&criada.analise.id).unwrap();
    let inicio_sessao = tempo::agora_ms() + 1;
    Sessoes::do_diretorio(&c.dados)
        .alterar("sessao-1", |_| {
            let mut r = RegistroSessao::novo("sessao-1", PROJETO, inicio_sessao);
            r.analise = Some(id_analise.clone());
            Some(r)
        })
        .unwrap();

    let p = painel_demanda(&c.dados, PROJETO, &d.id).unwrap();
    assert!(p.problemas.is_empty(), "{:?}", p.problemas);
    assert_eq!(p.demanda.titulo, "Parcelamento PJ");
    let ids: Vec<String> = p
        .linha_do_tempo
        .iter()
        .map(|i| match &i.evento {
            EventoDemanda::Reuniao(r) => r.id.clone(),
            EventoDemanda::Analise(a) => a.id.clone(),
            EventoDemanda::Sessao(s) => s.id.clone(),
        })
        .collect();
    assert_eq!(
        ids,
        [
            antes.clone(),
            criada.analise.id.clone(),
            "sessao-1".into(),
            depois
        ]
    );
    let EventoDemanda::Reuniao(r) = &p.linha_do_tempo[0].evento else {
        unreachable!()
    };
    assert_eq!(r.status, StatusAta::Rascunho);
    assert_eq!(r.itens, 4);
    assert!(
        r.documento
            .as_deref()
            .is_some_and(|d| d.ends_with("R-0001.md"))
    );
    // Espaço para o aviso da guarda do transcript (#136), ainda vazio.
    assert_eq!(r.transcript_guardado, None);

    // Formato que a tela espera.
    let json = serde_json::to_value(&p).unwrap();
    assert_eq!(json["linhaDoTempo"][0]["tipo"], "reuniao");
    assert_eq!(json["linhaDoTempo"][0]["status"], "rascunho");
    assert_eq!(
        json["linhaDoTempo"][0]["transcriptGuardado"],
        serde_json::Value::Null
    );
    assert_eq!(json["linhaDoTempo"][1]["tipo"], "analise");
    assert_eq!(json["linhaDoTempo"][2]["tipo"], "sessao");
    assert_eq!(
        json["linhaDoTempo"][2]["analise"],
        criada.analise.id.as_str()
    );

    // A análise grava `demanda.id` (e a chave da demanda) no frontmatter.
    let amb = c.ambiente();
    let vista = carregar_analise(&c.dados, PROJETO, &criada.analise.id).unwrap();
    let doc = amb
        .kb
        .ler::<FrontmatterAnalise>(&vista.analise.documento)
        .unwrap();
    assert_eq!(doc.frontmatter.demanda.id, Some(IdDemanda::novo(1)));
    assert_eq!(doc.frontmatter.demanda.chave.as_deref(), Some("FAT-12"));
}

/// Critério de aceite da #138: pendências e dúvidas da ata aprovada aparecem no
/// painel; as do rascunho, não.
#[test]
fn itens_so_das_atas_aprovadas() {
    let c = cenario();
    let d = criar_demanda(&c.dados, PROJETO, &nova("Parcelamento PJ", None)).unwrap();
    let rascunho = ata(&c, &d.id, Data::nova(2026, 11, 1).unwrap(), false);
    let aprovada = ata(&c, &d.id, Data::nova(2026, 11, 5).unwrap(), true);

    let p = painel_demanda(&c.dados, PROJETO, &d.id).unwrap();
    assert_eq!(p.pendencias.len(), 1, "{:?}", p.pendencias);
    let pend = &p.pendencias[0];
    assert_eq!(pend.id, "P-1");
    assert_eq!(pend.reuniao, aprovada);
    assert_eq!(pend.data, "2026-11-05");
    assert_eq!(pend.texto, "Enviar a tabela de juros do parcelamento");
    assert_eq!(pend.responsavel.as_deref(), Some("Ana (PO)"));
    assert_eq!(pend.prazo.as_deref(), Some("2026-11-20"));
    assert_eq!(p.duvidas.len(), 1);
    assert_eq!(p.duvidas[0].id, "DV-1");
    assert_eq!(p.duvidas[0].reuniao, aprovada);
    assert_eq!(p.proximos_passos.len(), 1);
    assert_eq!(p.proximos_passos[0].id, "PP-1");
    assert!(
        p.pendencias
            .iter()
            .chain(&p.duvidas)
            .chain(&p.proximos_passos)
            .all(|i| i.reuniao != rascunho)
    );
    // As duas atas estão na linha do tempo.
    assert_eq!(p.linha_do_tempo.len(), 2);
    let json = serde_json::to_value(&p).unwrap();
    assert_eq!(json["proximosPassos"][0]["id"], "PP-1");
}

#[test]
fn analise_pelo_inicio_com_demanda_e_demanda_encerrada() {
    let c = cenario();
    let d = criar_demanda(&c.dados, PROJETO, &nova("Parcelamento PJ", None)).unwrap();
    mudar_status_demanda(&c.dados, PROJETO, &d.id, true).unwrap();
    let erro = criar_analise_ligada(
        &c.dados,
        PROJETO,
        &d.id,
        Fluxo::Perguntar,
        "Como funciona o parcelamento?",
        None,
    )
    .unwrap_err();
    assert!(erro.contains("encerrada"), "{erro}");
    assert!(
        criar_analise_ligada(&c.dados, PROJETO, "D-0009", Fluxo::Perguntar, "x", None).is_err()
    );

    mudar_status_demanda(&c.dados, PROJETO, &d.id, false).unwrap();
    criar_analise_ligada(
        &c.dados,
        PROJETO,
        &d.id,
        Fluxo::Perguntar,
        "Como funciona o parcelamento?",
        None,
    )
    .unwrap();
    let p = painel_demanda(&c.dados, PROJETO, &d.id).unwrap();
    assert_eq!(p.linha_do_tempo.len(), 1);
}
