//! Issues do projeto (#322) no app, com o `gh` falso: cache, releitura em
//! segundo plano, filtros, situação na base e leitura sem rede.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use super::*;
use crate::demandas::{criar_analise_ligada, criar_demanda};
use crate::teste_base::{PROJETO, cenario};
use cba_core::analise::Fluxo;
use cba_core::eventos::Mudanca;
use cba_index::Cadastro;
use cba_kb::{Id, NovaDemanda};
use cba_repos::GithubConfig;
use cba_repos::cache_issues::FiltroEstado;

const ISSUES: &str = r#"[{"number":41,"title":"Parcelamento calcula as parcelas errado","body":"Ao parcelar em 12x o valor não bate.","url":"https://github.com/o/r/issues/41","createdAt":"2026-10-01T00:00:00Z","state":"OPEN","labels":[{"name":"bug"}]},{"number":42,"title":"Trocar a cor do botão","body":"O azul está claro.","url":"https://github.com/o/r/issues/42","createdAt":"2026-10-02T00:00:00Z","state":"CLOSED","labels":[{"name":"visual"}]},{"number":43,"title":"Emitir nota de crédito","body":"","url":"https://github.com/o/r/issues/43","createdAt":"2026-10-03T00:00:00Z","state":"OPEN","labels":[]}]"#;

/// `gh` falso: registra cada chamada em `chamadas` e, com o arquivo `offline`
/// presente, falha como sem rede.
fn gh_falso(dir: &Path) -> Ferramentas {
    let gh = dir.join("gh");
    let corpo = format!(
        "#!/bin/sh\necho \"$*\" >> '{log}'\n[ -e '{off}' ] && {{ echo 'error connecting to api.github.com' >&2; exit 1; }}\necho '{ISSUES}'\n",
        log = dir.join("chamadas").display(),
        off = dir.join("offline").display(),
    );
    fs::write(&gh, corpo).unwrap();
    fs::set_permissions(&gh, fs::Permissions::from_mode(0o755)).unwrap();
    Ferramentas {
        gh,
        ..Default::default()
    }
}

/// Quantas vezes a listagem (`gh issue list`) foi chamada.
fn listagens(dir: &Path) -> usize {
    fs::read_to_string(dir.join("chamadas"))
        .unwrap_or_default()
        .lines()
        .filter(|l| l.starts_with("issue list"))
        .count()
}

fn com_rastreador(c: &crate::teste_base::Cenario) {
    let mut p = cadastrado(&c.dados, PROJETO).unwrap();
    p.repositorios.tracker = Some(TrackerConfig::Github(GithubConfig { repo: "o/r".into() }));
    Cadastro::em(c.dados.projetos()).salvar(&p).unwrap();
}

fn filtro(estado: FiltroEstado) -> FiltroLista {
    FiltroLista {
        issues: FiltroIssues {
            estado,
            ..Default::default()
        },
        situacao: None,
    }
}

fn nada() {}

#[test]
fn sem_rastreador_recusa() {
    let c = cenario();
    let f = gh_falso(c.dir.path());
    let e = listar(
        &c.dados,
        PROJETO,
        &FiltroLista::default(),
        Leitura::Cache,
        &f,
        nada,
    )
    .unwrap_err();
    assert!(e.contains("repositório de issues"), "{e}");
}

#[test]
fn com_cache_nao_chama_o_rastreador_e_filtra() {
    let c = cenario();
    com_rastreador(&c);
    let dir = c.dir.path();
    let f = gh_falso(dir);

    // Sem cache: a primeira leitura é feita na hora.
    let l = listar(
        &c.dados,
        PROJETO,
        &FiltroLista::default(),
        Leitura::Cache,
        &f,
        nada,
    )
    .unwrap();
    assert_eq!(listagens(dir), 1);
    assert_eq!(l.total, 3);
    // Padrão: as abertas no rastreador.
    let chaves: Vec<&str> = l.issues.iter().map(|i| i.chave.as_str()).collect();
    assert_eq!(chaves, ["#41", "#43"]);
    assert_eq!(l.rotulos, ["bug", "visual"]);
    assert!(l.aviso.is_none() && !l.atualizando);
    assert!(c.dados.issues().join(format!("{PROJETO}.json")).exists());

    // Com o cache novo: nenhuma chamada ao gh.
    let l = listar(
        &c.dados,
        PROJETO,
        &filtro(FiltroEstado::Todas),
        Leitura::Cache,
        &f,
        nada,
    )
    .unwrap();
    assert_eq!(listagens(dir), 1);
    assert_eq!(l.issues.len(), 3);
    let encerradas = listar(
        &c.dados,
        PROJETO,
        &filtro(FiltroEstado::Encerradas),
        Leitura::Cache,
        &f,
        nada,
    )
    .unwrap();
    assert_eq!(encerradas.issues.len(), 1);
    assert_eq!(encerradas.issues[0].estado, "fechada");

    let busca = |b: &str, rotulo: Option<&str>| FiltroLista {
        issues: FiltroIssues {
            busca: Some(b.into()),
            estado: FiltroEstado::Todas,
            rotulo: rotulo.map(str::to_owned),
        },
        situacao: None,
    };
    let l = listar(
        &c.dados,
        PROJETO,
        &busca("#42", None),
        Leitura::Cache,
        &f,
        nada,
    )
    .unwrap();
    assert_eq!(l.issues[0].chave, "#42");
    assert_eq!(l.issues.len(), 1);
    let l = listar(
        &c.dados,
        PROJETO,
        &busca("botao", None),
        Leitura::Cache,
        &f,
        nada,
    )
    .unwrap();
    assert_eq!(l.issues.len(), 1);
    let l = listar(
        &c.dados,
        PROJETO,
        &busca("", Some("bug")),
        Leitura::Cache,
        &f,
        nada,
    )
    .unwrap();
    assert_eq!(l.issues.len(), 1);
    assert_eq!(listagens(dir), 1);

    // O filtro chega da tela em JSON, com a situação ao lado.
    let json: FiltroLista =
        serde_json::from_str(r#"{"busca":"41","estado":"todas","situacao":"sem-analise"}"#)
            .unwrap();
    assert_eq!(json.issues.busca.as_deref(), Some("41"));
    assert_eq!(json.situacao, Some(SituacaoNaBase::SemAnalise));
}

#[test]
fn cache_vencido_volta_na_hora_e_rele_em_segundo_plano() {
    let c = cenario();
    com_rastreador(&c);
    let dir = c.dir.path();
    let f = gh_falso(dir);
    let tracker = tracker_do_projeto(&c.dados, PROJETO).unwrap();
    let arquivo = arquivo_do_cache(&c.dados, PROJETO);
    cache_issues::atualizar(&arquivo, &tracker, &f).unwrap();
    assert_eq!(listagens(dir), 1);

    // Leitura antiga, com um título que o rastreador já não tem.
    let mut velho = cache_issues::ler(&arquivo, &tracker).unwrap();
    velho.lida_em_ms = 1;
    velho.listagem.issues[0].titulo = "Título antigo".into();
    cache_issues::gravar(&arquivo, &velho).unwrap();

    let (tx, rx) = mpsc::channel();
    let l = listar(
        &c.dados,
        PROJETO,
        &FiltroLista::default(),
        Leitura::Cache,
        &f,
        move || tx.send(()).unwrap(),
    )
    .unwrap();
    // Volta a leitura do cache, sem esperar o rastreador.
    assert_eq!(l.issues[0].titulo, "Título antigo");
    assert_eq!(l.lida_em, "1970-01-01T00:00:00Z");
    rx.recv_timeout(Duration::from_secs(20))
        .expect("a releitura em segundo plano terminou");
    assert_eq!(listagens(dir), 2);
    let l = listar(
        &c.dados,
        PROJETO,
        &FiltroLista::default(),
        Leitura::Cache,
        &f,
        nada,
    )
    .unwrap();
    assert_eq!(
        l.issues[0].titulo,
        "Parcelamento calcula as parcelas errado"
    );
    assert!(!l.atualizando);
    assert_eq!(listagens(dir), 2);
}

#[test]
fn sem_rede_mostra_a_ultima_leitura_com_aviso() {
    let c = cenario();
    com_rastreador(&c);
    let dir = c.dir.path();
    let f = gh_falso(dir);
    fs::write(dir.join("offline"), "").unwrap();
    // Sem cache e sem rede: nada a mostrar.
    let e = listar(
        &c.dados,
        PROJETO,
        &FiltroLista::default(),
        Leitura::Cache,
        &f,
        nada,
    )
    .unwrap_err();
    assert!(e.contains("error connecting"), "{e}");

    fs::remove_file(dir.join("offline")).unwrap();
    listar(
        &c.dados,
        PROJETO,
        &FiltroLista::default(),
        Leitura::Cache,
        &f,
        nada,
    )
    .unwrap();
    fs::write(dir.join("offline"), "").unwrap();
    // "Atualizar" sem rede: a última leitura, com o aviso.
    let l = listar(
        &c.dados,
        PROJETO,
        &FiltroLista::default(),
        Leitura::Forcada,
        &f,
        nada,
    )
    .unwrap();
    assert_eq!(l.issues.len(), 2);
    assert!(l.aviso.as_deref().unwrap().contains("error connecting"));
    // O aviso fica no cache para a próxima abertura.
    let l = listar(
        &c.dados,
        PROJETO,
        &FiltroLista::default(),
        Leitura::Cache,
        &f,
        nada,
    )
    .unwrap();
    assert!(l.aviso.is_some());
    // De volta à rede, "Atualizar" limpa o aviso.
    fs::remove_file(dir.join("offline")).unwrap();
    let l = listar(
        &c.dados,
        PROJETO,
        &FiltroLista::default(),
        Leitura::Forcada,
        &f,
        nada,
    )
    .unwrap();
    assert!(l.aviso.is_none());
}

#[test]
fn situacao_na_base_pela_analise_da_demanda() {
    let c = cenario();
    com_rastreador(&c);
    let f = gh_falso(c.dir.path());
    let d = criar_demanda(
        &c.dados,
        PROJETO,
        &NovaDemanda {
            titulo: "Parcelamento".into(),
            descricao: None,
            chave: Some("#41".into()),
            origem: Some("https://github.com/o/r/issues/41".into()),
        },
    )
    .unwrap();
    let todas = filtro(FiltroEstado::Todas);
    let l = listar(&c.dados, PROJETO, &todas, Leitura::Cache, &f, nada).unwrap();
    let i41 = l.issues.iter().find(|i| i.chave == "#41").unwrap();
    assert_eq!(i41.demanda.as_deref(), Some(d.id.as_str()));
    assert_eq!(i41.situacao, SituacaoNaBase::SemAnalise);
    assert!(i41.funcionalidades.is_empty());

    let criada = criar_analise_ligada(
        &c.dados,
        PROJETO,
        &d.id,
        Fluxo::Impacto,
        "Corrigir o cálculo das parcelas.",
        None,
    )
    .unwrap();
    let l = listar(&c.dados, PROJETO, &todas, Leitura::Cache, &f, nada).unwrap();
    let i41 = l.issues.iter().find(|i| i.chave == "#41").unwrap();
    assert_eq!(i41.situacao, SituacaoNaBase::EmAndamento);
    assert_eq!(i41.analise.as_deref(), Some(criada.analise.id.as_str()));

    let amb = ambiente(&c.dados, PROJETO).unwrap();
    let id = Id::analisar(&criada.analise.id).unwrap();
    amb.analises
        .alterar(&id, Mudanca::Estado, |a, _| {
            a.estado = EstadoAnalise::Concluida;
            Ok(())
        })
        .unwrap();
    let em_revisao = FiltroLista {
        situacao: Some(SituacaoNaBase::EmRevisao),
        ..todas.clone()
    };
    let l = listar(&c.dados, PROJETO, &em_revisao, Leitura::Cache, &f, nada).unwrap();
    assert_eq!(l.issues.len(), 1);
    assert_eq!(l.issues[0].chave, "#41");
    let sem = FiltroLista {
        situacao: Some(SituacaoNaBase::SemAnalise),
        ..todas
    };
    let l = listar(&c.dados, PROJETO, &sem, Leitura::Cache, &f, nada).unwrap();
    let chaves: Vec<&str> = l.issues.iter().map(|i| i.chave.as_str()).collect();
    assert_eq!(chaves, ["#42", "#43"]);
}
