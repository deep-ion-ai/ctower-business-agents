//! Guarda temporária do transcript (#136): guardar, ler, conferir citação,
//! descartar e retenção, com a fixture da reunião 1 do "Faturamento Exemplo".

mod comum;

use std::fs;
use std::sync::Arc;

use cba_core::dados::DiretorioDados;
use cba_core::eventos::{DiarioEventos, Mudanca, SemNotificacao};
use cba_docs::{EstadoAta, NovoItem, StatusCitacao, Templates, TipoItem};
use cba_kb::IdReuniao;
use cba_kb::frontmatter::Horario as HorarioAta;
use cba_reunioes::guarda::{ARQUIVO_FALAS, ARQUIVO_METADADOS};
use cba_reunioes::{
    ErroGuarda, FiltroFalas, Formato, Guarda, Horario, MotivoDescarte, PedidoCitacao,
    RecusaCitacao, ReuniaoGuardada, aplicar_retencao,
};
use comum::{PROJETO, base_com_reuniao, data, ms, reuniao_1};

fn guarda(dados: &DiretorioDados) -> Guarda {
    Guarda::com_notificador(dados, Arc::new(SemNotificacao))
}

#[test]
fn guarda_no_diretorio_de_dados_e_le_de_volta() {
    let dir = tempfile::tempdir().unwrap();
    let dados = DiretorioDados::em(dir.path().join("dados"));
    let g = guarda(&dados);
    let r = IdReuniao::novo(1);
    let t = reuniao_1();
    assert!(!g.existe(PROJETO, r));

    let m = g.guardar(PROJETO, r, &t, ms("2026-11-12")).unwrap();
    assert_eq!(m.falas, t.falas.len());
    assert_eq!(m.formato, Formato::Vtt);
    assert_eq!(m.criada_em(), data("2026-11-12"));
    assert!(g.existe(PROJETO, r));

    let pasta = dados.reunioes().join(PROJETO).join("R-0001");
    assert_eq!(g.pasta(PROJETO, r).unwrap(), pasta);
    assert!(pasta.join(ARQUIVO_FALAS).is_file());
    assert!(pasta.join(ARQUIVO_METADADOS).is_file());
    // Sem temporários sobrando ao lado.
    let nomes: Vec<String> = fs::read_dir(pasta.parent().unwrap())
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(nomes, ["R-0001"]);
    // Os metadados não trazem fala.
    let meta = fs::read_to_string(pasta.join(ARQUIVO_METADADOS)).unwrap();
    assert!(!meta.contains(&t.falas[0].texto), "{meta}");

    assert_eq!(g.metadados(PROJETO, r).unwrap(), m);
    assert_eq!(g.falas(PROJETO, r).unwrap(), t.falas);

    // Guardar de novo é recusado.
    assert!(matches!(
        g.guardar(PROJETO, r, &t, ms("2026-11-13")),
        Err(ErroGuarda::JaGuardado { .. })
    ));
    assert!(matches!(
        g.falas(PROJETO, IdReuniao::novo(2)),
        Err(ErroGuarda::NaoGuardado { .. })
    ));
    assert_eq!(
        g.listar().unwrap(),
        [ReuniaoGuardada {
            projeto: PROJETO.into(),
            reuniao: r
        }]
    );
}

#[test]
fn ler_falas_com_filtro_e_paginas() {
    let dir = tempfile::tempdir().unwrap();
    let dados = DiretorioDados::em(dir.path());
    let g = guarda(&dados);
    let r = IdReuniao::novo(1);
    let t = reuniao_1();
    g.guardar(PROJETO, r, &t, ms("2026-11-12")).unwrap();

    // "Maria Silva" casa com "Silva, Maria".
    let maria = g
        .ler_falas(
            PROJETO,
            r,
            &FiltroFalas {
                falante: Some("Maria Silva".into()),
                ..Default::default()
            },
        )
        .unwrap();
    assert!(maria.total > 0);
    assert!(
        maria
            .falas
            .iter()
            .all(|f| f.falante.as_deref() == Some("Silva, Maria"))
    );

    let primeiro_minuto = g
        .ler_falas(
            PROJETO,
            r,
            &FiltroFalas {
                ate: Some(Horario::de_milissegundos(60_000)),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        primeiro_minuto
            .falas
            .iter()
            .map(|f| f.indice)
            .collect::<Vec<_>>(),
        [1, 2, 3, 4, 5]
    );

    // Páginas de 4 cobrem todas as falas, sem repetir.
    let mut indices = Vec::new();
    let mut filtro = FiltroFalas {
        limite: Some(4),
        ..Default::default()
    };
    loop {
        let p = g.ler_falas(PROJETO, r, &filtro).unwrap();
        assert_eq!(p.total, t.falas.len());
        assert!(p.falas.len() <= 4);
        indices.extend(p.falas.iter().map(|f| f.indice));
        match p.proxima {
            Some(n) => filtro.pular = n,
            None => break,
        }
    }
    assert_eq!(indices, (1..=t.falas.len()).collect::<Vec<_>>());
}

#[test]
fn citacao_com_trecho_da_fala_e_aceita_e_sem_e_recusada() {
    let dir = tempfile::tempdir().unwrap();
    let dados = DiretorioDados::em(dir.path());
    let g = guarda(&dados);
    let r = IdReuniao::novo(1);
    g.guardar(PROJETO, r, &reuniao_1(), ms("2026-11-12"))
        .unwrap();

    // Fala 3 (João Pereira, 0:30): "Bom dia. Isso acontece porque a trava dos
    // 30 dias só existe na tela de retaguarda, a tela Delphi."
    let ok = g
        .conferir_citacao(
            PROJETO,
            r,
            &PedidoCitacao::por_indice(3, "a trava dos 30 dias só existe na tela de retaguarda"),
        )
        .unwrap();
    assert_eq!(ok.indice, 3);
    assert_eq!(ok.citacao.falante, "João Pereira");
    assert_eq!(ok.citacao.inicio, HorarioAta::analisar("00:00:30").unwrap());
    assert_eq!(ok.citacao.status, StatusCitacao::Conferida);

    let por_falante = g
        .conferir_citacao(
            PROJETO,
            r,
            &PedidoCitacao::por_falante(
                "joão pereira",
                HorarioAta::analisar("00:00:30").unwrap(),
                "TRAVA DOS 30 DIAS SO EXISTE",
            ),
        )
        .unwrap();
    assert_eq!(por_falante.indice, 3);

    let recusa = g
        .conferir_citacao(
            PROJETO,
            r,
            &PedidoCitacao::por_indice(3, "a trava dos 60 dias só existe na tela de retaguarda"),
        )
        .unwrap_err();
    assert!(
        matches!(
            recusa,
            ErroGuarda::Recusada(RecusaCitacao::NaoConfere { indice: 3 })
        ),
        "{recusa}"
    );
    assert!(recusa.to_string().contains("não aparece na fala 3"));
}

#[test]
fn descarte_apaga_a_pasta_e_marca_as_citacoes_da_ata() {
    let dir = tempfile::tempdir().unwrap();
    let dados = DiretorioDados::em(dir.path().join("dados"));
    let diario = DiarioEventos::em(dados.eventos());
    let g = Guarda::com_notificador(&dados, Arc::new(diario.clone()));
    let (base, projeto, r) = base_com_reuniao(dir.path());
    let templates = Templates::padrao();
    g.guardar(PROJETO, r, &reuniao_1(), ms("2026-11-12"))
        .unwrap();

    // Um item da ata com a citação conferida contra o transcript.
    let citacao = g
        .conferir_citacao(
            PROJETO,
            r,
            &PedidoCitacao::por_indice(3, "a trava dos 30 dias só existe na tela de retaguarda"),
        )
        .unwrap()
        .citacao;
    let (mut estado, _) = EstadoAta::ler(&projeto, r).unwrap();
    estado
        .adicionar_item(
            NovoItem::novo(
                TipoItem::Regra,
                "Fatura emitida há mais de 30 dias só pode ser cancelada por supervisor.",
            )
            .com_citacao(citacao),
        )
        .unwrap();
    templates.gravar_ata(&projeto, &estado).unwrap();

    let hoje = data("2026-11-14");
    let d = g
        .descartar(&projeto, &templates, r, hoje, MotivoDescarte::AtaAprovada)
        .unwrap();
    assert!(d.transcript_apagado && d.ata_encontrada);
    assert_eq!(d.citacoes_marcadas, 1);
    assert!(!g.pasta(PROJETO, r).unwrap().exists());
    assert!(!g.existe(PROJETO, r));
    assert!(!dados.reunioes().join(PROJETO).exists());

    let (lido, arquivos) = EstadoAta::ler(&projeto, r).unwrap();
    let c = &lido.itens[0].citacoes[0];
    assert_eq!(c.status, StatusCitacao::ConferidaEm(hoje));
    assert_eq!(
        c.trecho,
        "a trava dos 30 dias só existe na tela de retaguarda"
    );
    let ata = fs::read_to_string(projeto.caminho(&arquivos.ata)).unwrap();
    assert!(ata.contains("conferida em 2026-11-14"), "{ata}");

    // Repetir não muda nada (nem a data das citações).
    let de_novo = g
        .descartar(
            &projeto,
            &templates,
            r,
            data("2026-11-20"),
            MotivoDescarte::AtaAprovada,
        )
        .unwrap();
    assert!(!de_novo.transcript_apagado && de_novo.ata_encontrada);
    assert_eq!(de_novo.citacoes_marcadas, 0);
    assert_eq!(EstadoAta::ler(&projeto, r).unwrap().0, lido);

    // Reunião sem ata: só apaga a guarda.
    let sem_ata = IdReuniao::novo(99);
    g.guardar(PROJETO, sem_ata, &reuniao_1(), ms("2026-11-12"))
        .unwrap();
    let d = g
        .descartar(
            &projeto,
            &templates,
            sem_ata,
            hoje,
            MotivoDescarte::ReuniaoDescartada,
        )
        .unwrap();
    assert!(d.transcript_apagado && !d.ata_encontrada);

    // O diário recebeu guardar e apagar, só com identificadores.
    let eventos = diario.ler(PROJETO).unwrap();
    assert!(eventos.iter().all(|e| e.mudanca == Mudanca::Transcript));
    assert_eq!(
        eventos
            .iter()
            .map(|e| e.analise.as_str())
            .collect::<Vec<_>>(),
        ["R-0001", "R-0001", "R-0099", "R-0099"]
    );
    drop(base);
}

#[test]
fn retencao_apaga_vencidos_e_sinaliza_os_que_vencem_em_7_dias() {
    let dir = tempfile::tempdir().unwrap();
    let dados = DiretorioDados::em(dir.path());
    let g = guarda(&dados);
    let t = reuniao_1();
    let hoje = data("2026-12-14");
    // Guardada há 31, 30, 29, 23, 22 e 1 dia(s).
    let casos = [
        (1, "2026-11-13"),
        (2, "2026-11-14"),
        (3, "2026-11-15"),
        (4, "2026-11-21"),
        (5, "2026-11-22"),
        (6, "2026-12-13"),
    ];
    for (n, quando) in casos {
        g.guardar(PROJETO, IdReuniao::novo(n), &t, ms(quando))
            .unwrap();
    }
    g.guardar("outro-projeto", IdReuniao::novo(1), &t, ms("2026-10-01"))
        .unwrap();

    let relatorio = aplicar_retencao(&dados, hoje, 30);
    let apagados: Vec<String> = relatorio
        .apagados
        .iter()
        .map(|g| format!("{}/{}", g.projeto, g.reuniao))
        .collect();
    assert_eq!(
        apagados,
        [
            "faturamento-exemplo/R-0001",
            "faturamento-exemplo/R-0002",
            "outro-projeto/R-0001"
        ]
    );
    let a_vencer: Vec<(String, i64)> = relatorio
        .a_vencer
        .iter()
        .map(|s| (s.reuniao.to_string(), s.dias_restantes))
        .collect();
    assert_eq!(
        a_vencer,
        [("R-0003".to_string(), 1), ("R-0004".to_string(), 7)]
    );
    assert!(relatorio.falhas.is_empty());
    for (n, _) in casos {
        assert_eq!(g.existe(PROJETO, IdReuniao::novo(n)), n >= 3, "R-{n}");
    }
    assert!(!dados.reunioes().join("outro-projeto").exists());
    let s = g
        .situacao(PROJETO, IdReuniao::novo(3), hoje, 30)
        .unwrap()
        .unwrap();
    assert_eq!(s.apaga_em, data("2026-12-15"));
    assert!(s.vence_em_breve);
    assert!(
        g.situacao(PROJETO, IdReuniao::novo(1), hoje, 30)
            .unwrap()
            .is_none()
    );

    // Prazo configurável: com 10 dias, as de 29, 23 e 22 dias atrás também
    // vencem; a de ontem vence em 9 dias, ainda sem aviso.
    let relatorio = aplicar_retencao(&dados, hoje, 10);
    assert_eq!(relatorio.apagados.len(), 3);
    assert!(g.existe(PROJETO, IdReuniao::novo(6)));
    assert!(relatorio.a_vencer.is_empty());
    let s = g
        .situacao(PROJETO, IdReuniao::novo(6), data("2026-12-16"), 10)
        .unwrap()
        .unwrap();
    assert_eq!((s.dias_restantes, s.vence_em_breve), (7, true));
    // Aplicar de novo não acha mais nada para apagar.
    assert!(aplicar_retencao(&dados, hoje, 10).apagados.is_empty());
}

#[test]
fn retencao_apaga_guarda_com_metadados_ilegiveis_pela_data_da_pasta() {
    let dir = tempfile::tempdir().unwrap();
    let dados = DiretorioDados::em(dir.path());
    let g = guarda(&dados);
    let r = IdReuniao::novo(1);
    g.guardar(PROJETO, r, &reuniao_1(), ms("2026-11-12"))
        .unwrap();
    let meta = g.pasta(PROJETO, r).unwrap().join(ARQUIVO_METADADOS);
    fs::write(&meta, "{ estragado").unwrap();
    let erro = g.metadados(PROJETO, r).unwrap_err();
    assert!(matches!(erro, ErroGuarda::Ilegivel { .. }), "{erro}");
    // A pasta foi modificada agora: daqui a 30 dias, vence.
    let hoje = cba_core::tempo::hoje();
    assert!(aplicar_retencao(&dados, hoje, 30).apagados.is_empty());
    let depois = cba_core::tempo::somar_dias(hoje, 30);
    assert_eq!(aplicar_retencao(&dados, depois, 30).apagados.len(), 1);
    assert!(!g.existe(PROJETO, r));
}

#[test]
fn apagar_e_idempotente() {
    let dir = tempfile::tempdir().unwrap();
    let dados = DiretorioDados::em(dir.path());
    let g = guarda(&dados);
    let r = IdReuniao::novo(1);
    assert!(
        !g.apagar(PROJETO, r, MotivoDescarte::ReuniaoDescartada)
            .unwrap()
    );
    g.guardar(PROJETO, r, &reuniao_1(), ms("2026-11-12"))
        .unwrap();
    assert!(
        g.apagar(PROJETO, r, MotivoDescarte::ReuniaoDescartada)
            .unwrap()
    );
    assert!(
        !g.apagar(PROJETO, r, MotivoDescarte::ReuniaoDescartada)
            .unwrap()
    );
    // Depois de apagar, pode guardar de novo.
    g.guardar(PROJETO, r, &reuniao_1(), ms("2026-11-13"))
        .unwrap();
    assert!(g.existe(PROJETO, r));
}

#[test]
fn retencao_no_inicio_usa_o_prazo_da_config_e_marca_a_ata() {
    let dir = tempfile::tempdir().unwrap();
    let dados = DiretorioDados::em(dir.path().join("dados"));
    let g = guarda(&dados);
    let (base, projeto, r) = base_com_reuniao(dir.path());
    cba_core::config::Config {
        base: Some(base.raiz().to_path_buf()),
        retencao_transcript_dias: Some(10),
    }
    .gravar(&dados)
    .unwrap();
    let hoje = cba_core::tempo::hoje();
    let ha_dias = |n: i64| {
        cba_core::tempo::dias_desde_1970(cba_core::tempo::somar_dias(hoje, -n)) as u64 * 86_400_000
    };
    g.guardar(PROJETO, r, &reuniao_1(), ha_dias(10)).unwrap();
    let r2 = IdReuniao::novo(2);
    g.guardar(PROJETO, r2, &reuniao_1(), ha_dias(5)).unwrap();
    let citacao = g
        .conferir_citacao(
            PROJETO,
            r,
            &PedidoCitacao::por_indice(1, "Bom dia, pessoal"),
        )
        .unwrap()
        .citacao;
    let (mut estado, _) = EstadoAta::ler(&projeto, r).unwrap();
    estado
        .adicionar_item(NovoItem::novo(TipoItem::Decisao, "Abrir a demanda.").com_citacao(citacao))
        .unwrap();
    Templates::padrao().gravar_ata(&projeto, &estado).unwrap();

    let relatorio = cba_reunioes::aplicar_retencao_no_inicio(&dados);
    assert_eq!(relatorio.apagados.len(), 1);
    assert_eq!(relatorio.apagados[0].reuniao, r);
    assert_eq!(
        relatorio
            .a_vencer
            .iter()
            .map(|s| (s.reuniao, s.dias_restantes))
            .collect::<Vec<_>>(),
        [(r2, 5)]
    );
    let (lido, _) = EstadoAta::ler(&projeto, r).unwrap();
    assert_eq!(
        lido.itens[0].citacoes[0].status,
        StatusCitacao::ConferidaEm(hoje)
    );
}
