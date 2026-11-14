//! Privacidade (#136): um fluxo completo (guardar, ler, conferir citação,
//! recusar citação, descartar e retenção) com o diário de eventos e o log em
//! arquivo ligados. Depois, varre tudo o que ficou no diretório de dados e não
//! pode encontrar nenhum texto de fala dos transcripts da fixture.
//!
//! Fica num arquivo próprio porque liga o log global do processo de teste.

mod comum;

use std::fs;
use std::path::Path;
use std::sync::Arc;

use cba_core::dados::DiretorioDados;
use cba_core::eventos::{DiarioEventos, Mudanca};
use cba_docs::{EstadoAta, NovoItem, StatusCitacao, Templates, TipoItem};
use cba_kb::IdReuniao;
use cba_reunioes::{
    ErroGuarda, Fala, FiltroFalas, Guarda, MotivoDescarte, PedidoCitacao, Transcript, analisar,
    aplicar_retencao,
};
use comum::{PROJETO, base_com_reuniao, data, fixture, ms, reuniao_1};

/// Todas as falas das fixtures, em todos os formatos.
fn falas_das_fixtures() -> Vec<Fala> {
    let mut falas = Vec::new();
    for nome in [
        "reuniao-01-cancelamento.vtt",
        "reuniao-01-cancelamento.docx",
        "reuniao-01-cancelamento.txt",
        "reuniao-02-fechamento.vtt",
    ] {
        let t: Transcript = analisar(&fs::read(fixture(nome)).unwrap(), Some(nome));
        assert!(!t.falas.is_empty(), "{nome}");
        falas.extend(t.falas);
    }
    falas
}

/// Sequências de 4 palavras seguidas de cada fala (o texto inteiro, se for
/// mais curto), em minúsculas: o que não pode aparecer no que foi escrito.
fn sequencias(falas: &[Fala]) -> Vec<String> {
    let mut saida = Vec::new();
    for f in falas {
        let palavras: Vec<String> = f
            .texto
            .split_whitespace()
            .map(|p| p.to_lowercase())
            .collect();
        if palavras.len() < 4 {
            saida.push(palavras.join(" "));
        } else {
            saida.extend(palavras.windows(4).map(|j| j.join(" ")));
        }
    }
    saida.sort();
    saida.dedup();
    saida.retain(|s| s.chars().count() >= 8);
    saida
}

fn arquivos(pasta: &Path, saida: &mut Vec<std::path::PathBuf>) {
    let Ok(entradas) = fs::read_dir(pasta) else {
        return;
    };
    for e in entradas {
        let caminho = e.unwrap().path();
        if caminho.is_dir() {
            arquivos(&caminho, saida);
        } else {
            saida.push(caminho);
        }
    }
}

#[test]
fn eventos_e_logs_do_fluxo_completo_nao_tem_texto_de_fala() {
    let dir = tempfile::tempdir().unwrap();
    let dados = DiretorioDados::em(dir.path().join("dados"));
    // Log em arquivo no nível mais detalhado, como o app faz no início.
    // SAFETY: o teste é o único deste binário e ainda não há outras threads.
    unsafe { std::env::set_var(cba_core::log::VAR_FILTRO, "trace") };
    cba_core::log::iniciar(&dados.logs(), "teste").unwrap();

    let diario = DiarioEventos::em(dados.eventos());
    let g = Guarda::com_notificador(&dados, Arc::new(diario.clone()));
    let (_base, projeto, r) = base_com_reuniao(dir.path());
    let templates = Templates::padrao();

    // Guardar as duas reuniões (a segunda vence na retenção).
    let t1 = reuniao_1();
    g.guardar(PROJETO, r, &t1, ms("2026-11-12")).unwrap();
    let t2 = analisar(
        &fs::read(fixture("reuniao-02-fechamento.vtt")).unwrap(),
        Some("reuniao-02-fechamento.vtt"),
    );
    let r2 = IdReuniao::novo(2);
    g.guardar(PROJETO, r2, &t2, ms("2026-10-01")).unwrap();
    assert!(matches!(
        g.guardar(PROJETO, r, &t1, ms("2026-11-12")),
        Err(ErroGuarda::JaGuardado { .. })
    ));

    // Ler, com e sem filtro.
    let pagina = g
        .ler_falas(
            PROJETO,
            r,
            &FiltroFalas {
                falante: Some("Maria Silva".into()),
                limite: Some(3),
                ..Default::default()
            },
        )
        .unwrap();
    assert!(!pagina.falas.is_empty());
    let todas = g.ler_falas(PROJETO, r, &FiltroFalas::default()).unwrap();
    assert_eq!(todas.total, t1.falas.len());

    // Conferir uma citação que confere e uma que não.
    let fala = &t1.falas[2];
    let trecho: String = fala
        .texto
        .split_whitespace()
        .skip(2)
        .take(6)
        .collect::<Vec<_>>()
        .join(" ");
    let citacao = g
        .conferir_citacao(PROJETO, r, &PedidoCitacao::por_indice(3, trecho.clone()))
        .unwrap()
        .citacao;
    assert!(
        g.conferir_citacao(PROJETO, r, &PedidoCitacao::por_indice(4, trecho.clone()))
            .is_err()
    );
    let (mut estado, _) = EstadoAta::ler(&projeto, r).unwrap();
    estado
        .adicionar_item(
            NovoItem::novo(TipoItem::Regra, "Regra de cancelamento.").com_citacao(citacao),
        )
        .unwrap();
    templates.gravar_ata(&projeto, &estado).unwrap();

    // Descartar ao aprovar a ata, e a retenção para a outra reunião.
    let d = g
        .descartar(
            &projeto,
            &templates,
            r,
            data("2026-11-14"),
            MotivoDescarte::AtaAprovada,
        )
        .unwrap();
    assert_eq!(d.citacoes_marcadas, 1);
    let relatorio = aplicar_retencao(&dados, data("2026-11-14"), 30);
    assert_eq!(relatorio.apagados.len(), 1);
    assert!(!g.existe(PROJETO, r2));
    let (lido, _) = EstadoAta::ler(&projeto, r).unwrap();
    assert_eq!(
        lido.itens[0].citacoes[0].status,
        StatusCitacao::ConferidaEm(data("2026-11-14"))
    );

    // O diário tem os eventos do fluxo, só com identificadores.
    let eventos = diario.ler(PROJETO).unwrap();
    assert_eq!(eventos.len(), 4);
    assert!(eventos.iter().all(|e| e.mudanca == Mudanca::Transcript));

    // Varre tudo o que ficou no diretório de dados: diário, logs e o que mais
    // houver (a guarda já foi apagada).
    let mut escritos = Vec::new();
    arquivos(dados.raiz(), &mut escritos);
    let logs: Vec<_> = escritos
        .iter()
        .filter(|c| c.starts_with(dados.logs()))
        .collect();
    assert_eq!(logs.len(), 1, "{escritos:?}");
    assert!(escritos.iter().any(|c| c.starts_with(dados.eventos())));
    assert!(
        !escritos.iter().any(|c| c.starts_with(dados.reunioes())),
        "{escritos:?}"
    );
    let log = fs::read_to_string(logs[0]).unwrap();
    for esperado in [
        "transcript guardado",
        "citação conferida",
        "citação recusada",
        "transcript apagado",
        "retenção",
    ] {
        assert!(
            log.contains(esperado),
            "o log não registrou `{esperado}`:\n{log}"
        );
    }

    let proibidas = sequencias(&falas_das_fixtures());
    assert!(proibidas.len() > 50);
    assert!(
        proibidas
            .iter()
            .any(|s| trecho.to_lowercase().contains(s.as_str()))
    );
    for caminho in &escritos {
        let texto = String::from_utf8_lossy(&fs::read(caminho).unwrap()).to_lowercase();
        for s in &proibidas {
            assert!(
                !texto.contains(s.as_str()),
                "{} contém texto de fala: `{s}`",
                caminho.display()
            );
        }
        assert!(
            !texto.contains(&trecho.to_lowercase()),
            "{}",
            caminho.display()
        );
    }
}
