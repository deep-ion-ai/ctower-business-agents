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
    p.repositorios.tracker = Some(TrackerConfig::Github(GithubConfig {
        repo: "o/r".into(),
        ..Default::default()
    }));
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
            tipo: None,
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

#[test]
fn analisar_impacto_cria_a_demanda_e_a_analise_e_reaproveita_a_demanda() {
    let c = cenario();
    com_rastreador(&c);
    let f = gh_falso(c.dir.path());
    listar(
        &c.dados,
        PROJETO,
        &FiltroLista::default(),
        Leitura::Cache,
        &f,
        nada,
    )
    .unwrap();

    let primeira = analisar_impacto(&c.dados, PROJETO, OrigemIssue::Github, "#41").unwrap();
    assert!(primeira.demanda_criada);
    let amb = ambiente(&c.dados, PROJETO).unwrap();
    let demandas = amb.kb.demandas().unwrap();
    assert_eq!(demandas.len(), 1);
    let f = &demandas[0].documento.frontmatter;
    assert_eq!(f.chave.as_deref(), Some("#41"));
    assert_eq!(
        f.origem.as_deref(),
        Some("https://github.com/o/r/issues/41")
    );

    // A análise é de impacto, com o texto da issue, ligada à demanda.
    let analises = amb.analises.listar().unwrap();
    assert_eq!(analises.len(), 1);
    assert_eq!(analises[0].fluxo, Fluxo::Impacto);
    assert!(
        analises[0]
            .entrada
            .contains("Ao parcelar em 12x o valor não bate.")
    );
    let doc = amb
        .kb
        .ler::<FrontmatterAnalise>(&analises[0].documento)
        .unwrap();
    assert_eq!(doc.frontmatter.demanda.id, Some(demandas[0].id));
    assert_eq!(primeira.demanda, demandas[0].id.to_string());

    // A lista passa a mostrar a análise e a demanda da issue.
    let l = listar(
        &c.dados,
        PROJETO,
        &FiltroLista::default(),
        Leitura::Cache,
        &gh_falso(c.dir.path()),
        nada,
    )
    .unwrap();
    let i = l.issues.iter().find(|i| i.chave == "#41").unwrap();
    assert_eq!(i.situacao, SituacaoNaBase::EmAndamento);
    assert_eq!(i.demanda.as_deref(), Some(primeira.demanda.as_str()));

    // Analisar de novo reaproveita a demanda: uma só, com duas análises.
    let segunda = analisar_impacto(&c.dados, PROJETO, OrigemIssue::Github, "#41").unwrap();
    assert!(!segunda.demanda_criada);
    assert_eq!(segunda.demanda, primeira.demanda);
    assert_eq!(amb.kb.demandas().unwrap().len(), 1);
    assert_eq!(amb.analises.listar().unwrap().len(), 2);
}

#[test]
fn analisar_impacto_recusa_issue_encerrada_ou_fora_da_lista_sem_criar_nada() {
    let c = cenario();
    com_rastreador(&c);
    let f = gh_falso(c.dir.path());

    // Sem leitura da lista, pede para atualizar.
    let e = analisar_impacto(&c.dados, PROJETO, OrigemIssue::Github, "#41").unwrap_err();
    assert!(e.contains("Atualizar"), "{e}");

    listar(
        &c.dados,
        PROJETO,
        &FiltroLista::default(),
        Leitura::Cache,
        &f,
        nada,
    )
    .unwrap();
    let e = analisar_impacto(&c.dados, PROJETO, OrigemIssue::Github, "#42").unwrap_err();
    assert!(e.contains("encerrada"), "{e}");
    let e = analisar_impacto(&c.dados, PROJETO, OrigemIssue::Github, "#99").unwrap_err();
    assert!(e.contains("não está na lista"), "{e}");
    let amb = ambiente(&c.dados, PROJETO).unwrap();
    assert!(amb.kb.demandas().unwrap().is_empty());
    assert!(amb.analises.listar().unwrap().is_empty());
}

/// A análise criada direto da issue grava o link em `demanda.origem` e a chave, sem
/// demanda da base: a issue a acha mesmo assim, com todas as ligadas (#350).
#[test]
fn analise_criada_da_issue_liga_pela_chave_e_pelo_link() {
    let c = cenario();
    com_rastreador(&c);
    let f = gh_falso(c.dir.path());
    let ligada = |entrada: &str| {
        crate::analise::criar_analise_com_demanda(
            &c.dados,
            PROJETO,
            Fluxo::Impacto,
            entrada,
            None,
            Some(cba_kb::frontmatter::Demanda {
                id: None,
                origem: "https://github.com/o/r/issues/43".into(),
                chave: Some("#43".into()),
            }),
        )
        .unwrap()
    };
    let primeira = ligada("Emitir nota de crédito.");
    let segunda = ligada("Emitir nota de crédito, de novo.");
    // A colada à mão, com a mesma chave por acaso, não liga.
    crate::analise::criar_analise_com_demanda(
        &c.dados,
        PROJETO,
        Fluxo::Impacto,
        "Outra coisa.",
        None,
        Some(cba_kb::frontmatter::Demanda {
            id: None,
            origem: "texto".into(),
            chave: Some("#43".into()),
        }),
    )
    .unwrap();
    let l = listar(
        &c.dados,
        PROJETO,
        &filtro(FiltroEstado::Todas),
        Leitura::Cache,
        &f,
        nada,
    )
    .unwrap();
    let i43 = l.issues.iter().find(|i| i.chave == "#43").unwrap();
    assert_eq!(i43.situacao, SituacaoNaBase::EmAndamento);
    let mut esperadas = vec![primeira.analise.id.clone(), segunda.analise.id.clone()];
    esperadas.sort();
    let mut achadas = i43.analises.clone();
    achadas.sort();
    assert_eq!(achadas, esperadas);
    assert!(i43.analise.as_ref().is_some_and(|a| esperadas.contains(a)));
}
