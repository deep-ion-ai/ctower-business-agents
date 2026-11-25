//! Testes do resumo do projeto (#318) com o sistema fictício "Faturamento Exemplo".

use std::time::Instant;

use super::*;
use crate::analise::{criar_analise, criar_analise_do_fluxo};
use crate::demandas::{criar_analise_ligada, criar_demanda, mudar_status_demanda};
use crate::inicio::inicio;
use crate::teste_base::{Cenario, PKG, PROJETO, cenario};
use cba_core::analise::Cliente;
use cba_core::eventos::Mudanca;
use cba_kb::NovaDemanda;
use cba_session::sessoes::RegistroSessao;
use serde_json::json;

fn nova_demanda(titulo: &str) -> NovaDemanda {
    NovaDemanda {
        titulo: titulo.into(),
        descricao: None,
        chave: None,
        origem: None,
        tipo: None,
    }
}

/// Afirmação com evidência real do código da fixture.
fn afirmar(c: &Cenario, id: &str, secao: &str, texto: &str) {
    c.chamar(
        "afirmar",
        json!({
            "analise": id,
            "secao": secao,
            "texto": texto,
            "confianca": "confirmado",
            "evidencias": [{
                "arquivo": PKG,
                "linha_inicial": 71,
                "linha_final": 72,
                "trecho": "FN_PARAMETRO('PERC_MULTA_ATRASO') / 100, 2)"
            }]
        }),
    );
}

fn id(a: &str) -> Id {
    Id::analisar(a).unwrap()
}

fn sessao(c: &Cenario, sessao: &str, analise: &str, estado: EstadoSessao, quando: u64) {
    Sessoes::do_diretorio(&c.dados)
        .alterar(sessao, |_| {
            let mut r = RegistroSessao::novo(sessao, PROJETO, quando);
            r.analise = Some(id(analise));
            r.estado = estado;
            r.cliente = Cliente::ClaudeCode;
            Some(r)
        })
        .unwrap();
}

#[test]
fn projeto_sem_nada_so_pede_a_primeira_analise() {
    let c = cenario();
    let r = resumo_projeto(&c.dados, PROJETO).unwrap();
    assert_eq!(r.projeto, PROJETO);
    assert_eq!(r.nome, "Faturamento Exemplo");
    assert_eq!(r.analises.total, 0);
    assert_eq!(r.analises.por_fluxo.len(), 8);
    assert!(r.analises.por_fluxo.iter().all(|f| f.total == 0));
    assert!(r.aguardando_revisao.is_empty() && r.sessoes.por_analise.is_empty());
    assert_eq!(r.perguntas_em_aberto, PerguntasEmAberto::default());
    assert_eq!(r.demandas, ResumoDemandas::default());
    assert_eq!(r.base, ResumoBase::default());
    let tipos: Vec<_> = r.atividades.iter().map(|a| a.tipo).collect();
    assert_eq!(tipos, [TipoAtividade::PrimeiraAnalise]);
    assert_eq!(r.pedem_acao, 1);
}

#[test]
fn contagens_e_atividades_na_ordem_do_proximo_passo() {
    let c = cenario();
    // A-0001: aprovada, com uma regra e um termo na base.
    c.aprovar_analise_da_multa();

    // A-0002: concluída, duas afirmações (uma decidida) e uma pergunta aberta.
    let a2 = criar_analise_do_fluxo(&c.dados, PROJETO, Fluxo::Impacto, "Parcelamento", None)
        .unwrap()
        .analise
        .id;
    c.chamar("iniciar_analise", json!({"analise": a2}));
    afirmar(
        &c,
        &a2,
        "demanda_como_entendida",
        "O cliente quer parcelar a fatura.",
    );
    afirmar(
        &c,
        &a2,
        "como_e_hoje",
        "A multa é de 2% sobre o valor original.",
    );
    afirmar(
        &c,
        &a2,
        "abrangencia",
        "Pequena: só o cálculo da multa muda.",
    );
    let amb = c.ambiente();
    amb.analises
        .alterar(&id(&a2), Mudanca::Pergunta, |a, agora| {
            a.registrar_pergunta("Vale para fatura parcelada?", None, None, agora)
                .map(|_| ())
        })
        .unwrap();
    c.chamar("concluir_analise", json!({"analise": a2}));
    amb.analises
        .alterar(&id(&a2), Mudanca::Afirmacao, |a, _| {
            a.marcar_afirmacao(1, EstadoAfirmacao::Confirmada)
        })
        .unwrap();

    // A-0003: criada, parada; A-0004: criada, com o agente esperando o analista.
    let a3 = criar_analise_do_fluxo(&c.dados, PROJETO, Fluxo::Explicar, "Cancelamento", None)
        .unwrap()
        .analise
        .id;
    let a4 = criar_analise(&c.dados, PROJETO, "Como a fatura é gerada?", None)
        .unwrap()
        .analise
        .id;
    sessao(&c, "s-a3-antiga", &a3, EstadoSessao::Encerrada, 1_000);
    sessao(&c, "s-a4", &a4, EstadoSessao::AguardandoAnalista, 2_000);

    // Demandas: D-0001 com análise ligada, D-0002 sem análise, D-0003 encerrada.
    let d1 = criar_demanda(&c.dados, PROJETO, &nova_demanda("Parcelamento PJ")).unwrap();
    let d2 = criar_demanda(&c.dados, PROJETO, &nova_demanda("Multa de fatura vencida")).unwrap();
    let d3 = criar_demanda(&c.dados, PROJETO, &nova_demanda("Segunda via")).unwrap();
    criar_analise_ligada(&c.dados, PROJETO, &d1.id, Fluxo::Impacto, "Impacto", None).unwrap();
    mudar_status_demanda(&c.dados, PROJETO, &d3.id, true).unwrap();

    let r = resumo_projeto(&c.dados, PROJETO).unwrap();

    assert_eq!(r.analises.total, 5);
    assert_eq!(
        r.analises.por_estado,
        PorEstado {
            criada: 3,
            em_andamento: 0,
            concluida: 1,
            aprovada: 1
        }
    );
    let fluxo = |f: Fluxo| {
        r.analises
            .por_fluxo
            .iter()
            .find(|c| c.fluxo == f)
            .unwrap()
            .total
    };
    assert_eq!(fluxo(Fluxo::Perguntar), 2);
    assert_eq!(fluxo(Fluxo::Impacto), 2);
    assert_eq!(fluxo(Fluxo::Explicar), 1);

    assert_eq!(r.aguardando_revisao.len(), 1);
    let rev = &r.aguardando_revisao[0];
    assert_eq!(rev.id, a2);
    assert_eq!(
        (
            rev.afirmacoes_decididas,
            rev.afirmacoes_total,
            rev.pendencias
        ),
        (1, 3, 2)
    );

    assert_eq!(r.perguntas_em_aberto.total, 1);
    assert_eq!(r.perguntas_em_aberto.por_analise[0].analise, a2);

    // D-0002 está aberta e sem análise; D-0001 tem a ligada; D-0003 está encerrada.
    assert_eq!(
        r.demandas,
        ResumoDemandas {
            abertas: 2,
            encerradas: 1,
            sem_analise: 1
        }
    );

    // As contagens da base são as da tela de início.
    let s = &inicio(&c.dados).unwrap().projetos[0];
    assert_eq!(
        (r.base.regras, r.base.termos, r.base.a_revalidar),
        (s.regras, s.termos, s.a_revalidar)
    );
    assert_eq!((r.base.regras, r.base.termos), (1, 1));

    // Sessão mais recente por análise (A-0003 tem só a encerrada).
    assert_eq!(r.sessoes.total, 2);
    assert_eq!(r.sessoes.aguardando_analista, 1);
    assert_eq!(r.sessoes.encerradas, 1);
    let da = |a: &str| {
        r.sessoes
            .por_analise
            .iter()
            .find(|s| s.analise == a)
            .unwrap()
    };
    assert_eq!(da(&a4).sessao.id, "s-a4");
    assert_eq!(da(&a3).sessao.estado, EstadoSessao::Encerrada);

    // Atividades: responder, revisar, demanda sem análise, retomar (sem sessão viva).
    let lista: Vec<_> = r
        .atividades
        .iter()
        .map(|a| (a.tipo, a.analise.clone(), a.demanda.clone()))
        .collect();
    assert_eq!(
        lista,
        [
            (TipoAtividade::Responder, Some(a4.clone()), None),
            (TipoAtividade::Revisar, Some(a2.clone()), None),
            (TipoAtividade::DemandaSemAnalise, None, Some(d2.id.clone())),
            // A mais recente primeiro: A-0005 (a ligada à D-0001) é posterior à A-0003.
            (TipoAtividade::Retomar, Some("A-0005".to_owned()), None),
            (TipoAtividade::Retomar, Some(a3.clone()), None),
        ]
    );
    assert!(r.atividades.iter().enumerate().all(|(i, a)| a.ordem == i));
    assert_eq!(r.pedem_acao, 5);
    assert_eq!(r.atividades[1].decididas, Some(1));
    assert_eq!(r.atividades[1].total, Some(3));

    // O JSON que a tela lê.
    let json = serde_json::to_value(&r).unwrap();
    assert_eq!(json["analises"]["porEstado"]["emAndamento"], 0);
    assert_eq!(json["aguardandoRevisao"][0]["afirmacoesDecididas"], 1);
    assert_eq!(json["atividades"][2]["tipo"], "demanda-sem-analise");
    assert!(json["sessoes"]["porAnalise"][0]["estado"].is_string());
    assert_eq!(json["pedemAcao"], 5);
}

#[test]
fn mudancas_aparecem_na_chamada_seguinte() {
    let c = cenario();
    let a1 = criar_analise(&c.dados, PROJETO, "Como a multa é calculada?", None)
        .unwrap()
        .analise
        .id;
    let d = criar_demanda(&c.dados, PROJETO, &nova_demanda("Parcelamento PJ")).unwrap();
    let r = resumo_projeto(&c.dados, PROJETO).unwrap();
    assert_eq!(r.analises.por_estado.criada, 1);
    assert_eq!(r.demandas.sem_analise, 1);
    // Uma segunda chamada, sem mudança, devolve as mesmas contagens.
    let r2 = resumo_projeto(&c.dados, PROJETO).unwrap();
    assert_eq!((&r2.analises, &r2.demandas), (&r.analises, &r.demandas));

    c.chamar("iniciar_analise", json!({"analise": a1}));
    afirmar(
        &c,
        &a1,
        "resposta",
        "A multa é de 2% sobre o valor original.",
    );
    c.chamar("concluir_analise", json!({"analise": a1}));
    let r = resumo_projeto(&c.dados, PROJETO).unwrap();
    assert_eq!(r.analises.por_estado.concluida, 1);
    assert_eq!(r.aguardando_revisao[0].afirmacoes_total, 1);

    // A análise ligada à demanda tira a demanda de "sem análise"; encerrar a tira de "abertas".
    criar_analise_ligada(&c.dados, PROJETO, &d.id, Fluxo::Impacto, "Impacto", None).unwrap();
    let r = resumo_projeto(&c.dados, PROJETO).unwrap();
    assert_eq!((r.demandas.abertas, r.demandas.sem_analise), (1, 0));
    mudar_status_demanda(&c.dados, PROJETO, &d.id, true).unwrap();
    let r = resumo_projeto(&c.dados, PROJETO).unwrap();
    assert_eq!((r.demandas.abertas, r.demandas.encerradas), (0, 1));

    // A sessão viva tira a análise criada de "retomar".
    let a3 = r
        .atividades
        .iter()
        .find(|a| a.tipo == TipoAtividade::Retomar)
        .and_then(|a| a.analise.clone())
        .unwrap();
    sessao(&c, "s1", &a3, EstadoSessao::Ativa, 5_000);
    let r = resumo_projeto(&c.dados, PROJETO).unwrap();
    assert!(
        r.atividades
            .iter()
            .all(|a| a.tipo != TipoAtividade::Retomar)
    );
    assert_eq!(r.sessoes.ativas, 1);
}

#[test]
fn regras_a_revalidar_viram_atividade() {
    let c = cenario();
    c.aprovar_analise_da_multa();
    assert!(
        resumo_projeto(&c.dados, PROJETO)
            .unwrap()
            .atividades
            .is_empty()
    );
    c.mudar_o_codigo_da_multa();
    crate::projeto::revalidar(&c.dados, PROJETO).unwrap();
    let r = resumo_projeto(&c.dados, PROJETO).unwrap();
    assert_eq!(r.base.a_revalidar, 1);
    assert_eq!(r.atividades.len(), 1);
    assert_eq!(r.atividades[0].tipo, TipoAtividade::Revalidar);
    assert_eq!(r.atividades[0].regras, Some(1));
}

#[test]
fn projeto_desconhecido_recusa() {
    let c = cenario();
    let erro = resumo_projeto(&c.dados, "nao-existe").unwrap_err();
    assert!(erro.contains("nao-existe"), "{erro}");
}

/// Meta da issue: com 100 análises, a chamada repetida responde em menos de 200 ms
/// (o cache só confere os carimbos dos arquivos).
#[test]
fn cem_analises_respondem_dentro_da_meta() {
    let c = cenario();
    for i in 0..100 {
        criar_analise(&c.dados, PROJETO, &format!("Pergunta número {i}"), None).unwrap();
    }
    let inicio = Instant::now();
    let r = resumo_projeto(&c.dados, PROJETO).unwrap();
    let frio = inicio.elapsed();
    assert_eq!(r.analises.total, 100);

    let inicio = Instant::now();
    let r = resumo_projeto(&c.dados, PROJETO).unwrap();
    let quente = inicio.elapsed();
    assert_eq!(r.analises.total, 100);
    println!("100 análises: primeira chamada {frio:?}, seguinte {quente:?}");
    assert!(
        quente.as_millis() < 200,
        "chamada com cache levou {quente:?}"
    );
}

#[test]
fn sessoes_vivas_incluem_as_sem_analise_e_a_lista_traz_todas() {
    let c = cenario();
    let a1 = criar_analise(&c.dados, PROJETO, "Como a multa é calculada?", None)
        .unwrap()
        .analise
        .id;
    sessao(&c, "s1", &a1, EstadoSessao::AguardandoAnalista, 2_000);
    sessao(&c, "s0", &a1, EstadoSessao::Encerrada, 1_000);
    // Sessão aberta na pasta do projeto, ainda sem análise ligada.
    Sessoes::do_diretorio(&c.dados)
        .alterar("s2", |_| Some(RegistroSessao::novo("s2", PROJETO, 3_000)))
        .unwrap();
    // Sessão de outro projeto não aparece.
    Sessoes::do_diretorio(&c.dados)
        .alterar("s3", |_| Some(RegistroSessao::novo("s3", "outro", 4_000)))
        .unwrap();

    let r = resumo_projeto(&c.dados, PROJETO).unwrap();
    let vivas: Vec<_> = r
        .sessoes
        .vivas
        .iter()
        .map(|s| (s.id.as_str(), s.estado, s.analise.as_deref()))
        .collect();
    assert_eq!(
        vivas,
        [
            ("s2", EstadoSessao::Ativa, None),
            ("s1", EstadoSessao::AguardandoAnalista, Some(a1.as_str())),
        ]
    );
    assert_eq!(
        r.sessoes.vivas[1].titulo.as_deref(),
        Some("Como a multa é calculada?")
    );
    assert_eq!(r.sessoes.vivas[1].cliente, Cliente::ClaudeCode);

    let todas = sessoes_do_projeto(&c.dados, PROJETO).unwrap();
    let ids: Vec<_> = todas.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["s2", "s1", "s0"]);
    assert_eq!(todas[2].estado, EstadoSessao::Encerrada);
    assert_eq!(
        todas[1].titulo.as_deref(),
        Some("Como a multa é calculada?")
    );
    assert_eq!(todas[0].titulo, None);
}

#[test]
fn atividade_recente_vem_do_historico_e_sobrevive_ao_diario_esvaziado() {
    let c = cenario();
    let r = resumo_projeto(&c.dados, PROJETO).unwrap();
    assert!(r.atividade_recente.is_empty());

    let a1 = criar_analise(&c.dados, PROJETO, "Como a multa é calculada?", None)
        .unwrap()
        .analise
        .id;
    c.chamar("iniciar_analise", json!({"analise": a1}));
    afirmar(
        &c,
        &a1,
        "resposta",
        "A multa é de 2% sobre o valor original.",
    );
    afirmar(&c, &a1, "resposta", "Os juros são de 1% ao mês.");
    c.chamar("concluir_analise", json!({"analise": a1}));

    // A vigia do app esvazia o diário; reiniciar o app não traz nada dele de volta.
    let diario = cba_core::eventos::DiarioEventos::em(c.dados.eventos());
    std::fs::write(diario.arquivo(PROJETO), "").unwrap();

    let r = resumo_projeto(&c.dados, PROJETO).unwrap();
    let linhas: Vec<_> = r
        .atividade_recente
        .iter()
        .map(|a| (a.alvo.as_str(), a.mudanca, a.estado, a.quantas))
        .collect();
    assert_eq!(linhas[0].0, a1);
    assert_eq!(
        (linhas[0].1, linhas[0].2),
        (Mudanca::Estado, Some(EstadoAnalise::Concluida))
    );
    // As duas afirmações seguidas viram uma linha só.
    assert!(
        linhas.contains(&(
            a1.as_str(),
            Mudanca::Afirmacao,
            Some(EstadoAnalise::EmAndamento),
            2
        )),
        "{linhas:?}"
    );
    assert_eq!(linhas.last().unwrap().1, Mudanca::Criada);
    assert_eq!(
        r.atividade_recente[0].titulo.as_deref(),
        Some("Como a multa é calculada?")
    );
    assert!(
        r.atividade_recente
            .iter()
            .all(|a| a.mudanca != Mudanca::Sessao)
    );
}

#[test]
fn atividade_recente_agrupa_e_limita_a_vinte() {
    let evento = |n: u64, alvo: &str, mudanca| Evento {
        quando_ms: n,
        projeto: PROJETO.into(),
        analise: alvo.into(),
        mudanca,
        versao: n,
        processo: 1,
        estado: None,
    };
    let mut eventos = Vec::new();
    for i in 0..30 {
        let alvo = if i % 2 == 0 { "A-0001" } else { "A-0002" };
        eventos.push(evento(i * 10, alvo, Mudanca::Pergunta));
        eventos.push(evento(i * 10 + 1, alvo, Mudanca::Pergunta));
        eventos.push(evento(i * 10 + 2, alvo, Mudanca::Sessao));
    }
    let titulos = HashMap::from([("A-0001", "Multa")]);
    let r = atividades_recentes(&eventos, &titulos);
    assert_eq!(r.len(), ATIVIDADES_RECENTES);
    assert_eq!(
        (r[0].alvo.as_str(), r[0].quando_ms, r[0].quantas),
        ("A-0002", 291, 2)
    );
    assert_eq!(r[1].titulo.as_deref(), Some("Multa"));
    assert_eq!(r[0].titulo, None);
}
