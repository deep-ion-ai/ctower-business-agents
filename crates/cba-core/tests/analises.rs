//! Ambiente e persistência das análises com o sistema fictício "Faturamento
//! Exemplo", inclusive com vários processos gravando ao mesmo tempo.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use cba_core::ambiente::{Ambiente, ErroAmbiente};
use cba_core::analise::{
    Cliente, Confianca, EstadoAnalise, EstadoEvidencia, EstadoSessao, Evidencia, Fluxo,
    NovaAfirmacao,
};
use cba_core::analises::{Analises, Erro, NovaAnalise};
use cba_core::config::Config;
use cba_core::dados::DiretorioDados;
use cba_core::eventos::{DiarioEventos, Mudanca, SemNotificacao};
use cba_index::{Cadastro, Sistema};
use cba_kb::frontmatter::{FrontmatterAnalise, Linhas, StatusAnalise};
use cba_kb::{CaminhoRelativo, Id};

/// Variável que faz o teste `processo_filho` trabalhar (só quando executado por
/// `varios_processos_nao_perdem_registros`).
const VAR_FILHO: &str = "CBA_TESTE_FILHO";

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/faturamento-exemplo")
}

/// Dados do app com o sistema fictício cadastrado e a base configurada.
fn preparar(raiz: &Path) -> DiretorioDados {
    let dados = DiretorioDados::em(raiz.join("dados"));
    let sistema = Sistema::novo("Faturamento Exemplo", [fixture()]).unwrap();
    Cadastro::em(dados.sistemas()).cadastrar(&sistema).unwrap();
    Config {
        base: Some(raiz.join("base")),
    }
    .gravar(&dados)
    .unwrap();
    cba_kb::Base::criar(raiz.join("base")).unwrap();
    dados
}

fn nova(titulo: &str) -> NovaAnalise {
    NovaAnalise {
        fluxo: Fluxo::Perguntar,
        titulo: titulo.into(),
        entrada: "Como é calculada a multa por atraso de pagamento?".into(),
        analista: Some("Ana Exemplo".into()),
        base: None,
    }
}

fn afirmacao(texto: &str) -> NovaAfirmacao {
    NovaAfirmacao {
        secao: "regras".into(),
        texto: texto.into(),
        confianca: Confianca::Inferido,
        evidencias: Vec::new(),
        sessao: None,
    }
}

#[test]
fn ambiente_pela_pasta_do_sistema_e_subpasta() {
    let dir = tempfile::tempdir().unwrap();
    let dados = preparar(dir.path());
    for pasta in [fixture(), fixture().join("db")] {
        let amb = Ambiente::do_diretorio(&pasta, &dados).unwrap();
        assert_eq!(amb.sistema.id, "faturamento-exemplo");
        assert_eq!(amb.kb.nome(), "faturamento-exemplo");
        // O sistema foi criado na base, com o sistema.md.
        let doc = amb.kb.ler_sistema().unwrap();
        assert_eq!(doc.frontmatter.nome, "Faturamento Exemplo");
        // O escopo é o das ferramentas: recusa o que está excluído.
        assert!(amb.escopo.resolver("db/03_pkg_cobranca.sql").is_ok());
        assert!(amb.escopo.resolver("../fora.txt").is_err());
    }
}

#[test]
fn fora_de_sistema_e_sem_base_dao_erro_claro() {
    let dir = tempfile::tempdir().unwrap();
    let dados = preparar(dir.path());
    let fora = tempfile::tempdir().unwrap();
    let erro = Ambiente::do_diretorio(fora.path(), &dados).unwrap_err();
    assert!(matches!(erro, ErroAmbiente::ForaDeSistema(_)), "{erro:?}");
    let texto = erro.to_string();
    assert!(
        texto.contains("não está na pasta de um sistema cadastrado"),
        "{texto}"
    );
    assert!(texto.contains("cadastrar o sistema"), "{texto}");

    // Sem nenhum cadastro, o mesmo erro.
    let vazio = DiretorioDados::em(dir.path().join("outros-dados"));
    assert!(matches!(
        Ambiente::do_diretorio(&fixture(), &vazio).unwrap_err(),
        ErroAmbiente::ForaDeSistema(_)
    ));

    // Sistema cadastrado, mas sem base escolhida.
    Config::default().gravar(&dados).unwrap();
    let erro = Ambiente::do_diretorio(&fixture(), &dados).unwrap_err();
    assert!(matches!(erro, ErroAmbiente::BaseNaoConfigurada), "{erro:?}");
    assert!(erro.to_string().contains("primeiro uso"));
}

#[test]
fn criar_alterar_carregar_e_listar() {
    let dir = tempfile::tempdir().unwrap();
    let dados = preparar(dir.path());
    let amb = Ambiente::do_diretorio(&fixture(), &dados).unwrap();
    let analises = &amb.analises;

    let a = analises.criar(nova("Multa por atraso")).unwrap();
    assert_eq!(a.id.to_string(), "A-0001");
    assert_eq!(a.estado, EstadoAnalise::Criada);
    let b = analises.criar(nova("Juros")).unwrap();
    assert_eq!(b.id.to_string(), "A-0002");

    // O documento existe na base, com o frontmatter da análise.
    let doc: cba_kb::Documento<FrontmatterAnalise> = amb.kb.ler(&a.documento).unwrap();
    assert_eq!(doc.frontmatter.status, StatusAnalise::Rascunho);
    assert_eq!(doc.frontmatter.titulo, "Multa por atraso");

    let evidencia = Evidencia {
        arquivo: CaminhoRelativo::novo("db/03_pkg_cobranca.sql").unwrap(),
        linhas: Linhas::novo(1, 1).unwrap(),
        trecho: "--".into(),
        hash: "h".into(),
        commit: None,
        estado: EstadoEvidencia::Valida,
    };
    let (n, depois) = analises
        .alterar(&a.id, Mudanca::Afirmacao, |an, agora| {
            an.iniciar_sessao("sessao-1", Cliente::ClaudeCode, agora)?;
            an.registrar_afirmacao(
                NovaAfirmacao {
                    confianca: Confianca::Confirmado,
                    evidencias: vec![evidencia],
                    sessao: Some("sessao-1".into()),
                    ..afirmacao("A multa é de 2% sobre o valor em atraso.")
                },
                agora,
            )
        })
        .unwrap();
    assert_eq!(n, 1);
    assert_eq!(depois.versao, 2);
    assert_eq!(analises.carregar(&a.id).unwrap(), depois);

    // Recusa do modelo não grava nada.
    let erro = analises
        .alterar(&a.id, Mudanca::Afirmacao, |an, agora| {
            an.registrar_afirmacao(
                NovaAfirmacao {
                    confianca: Confianca::Confirmado,
                    ..afirmacao("sem evidência")
                },
                agora,
            )
        })
        .unwrap_err();
    assert!(matches!(erro, Erro::Modelo(_)), "{erro:?}");
    assert_eq!(analises.carregar(&a.id).unwrap().versao, 2);

    // Concluir muda o status do documento.
    analises
        .alterar(&a.id, Mudanca::Estado, |an, _| an.concluir())
        .unwrap();
    let doc: cba_kb::Documento<FrontmatterAnalise> = amb.kb.ler(&a.documento).unwrap();
    assert_eq!(doc.frontmatter.status, StatusAnalise::EmRevisao);
    analises
        .alterar(&a.id, Mudanca::Sessao, |an, agora| {
            an.atualizar_sessao("sessao-1", EstadoSessao::Encerrada, agora)
        })
        .unwrap();

    let lista = analises.listar().unwrap();
    let ids: Vec<String> = lista.iter().map(|a| a.id.to_string()).collect();
    assert_eq!(ids, ["A-0001", "A-0002"]);

    // Cada mudança foi notificada ao app, sem conteúdo.
    let eventos = DiarioEventos::em(dados.eventos())
        .ler("faturamento-exemplo")
        .unwrap();
    let mudancas: Vec<Mudanca> = eventos.iter().map(|e| e.mudanca).collect();
    assert_eq!(
        mudancas,
        [
            Mudanca::Criada,
            Mudanca::Criada,
            Mudanca::Afirmacao,
            Mudanca::Estado,
            Mudanca::Sessao
        ]
    );
    let diario =
        std::fs::read_to_string(DiarioEventos::em(dados.eventos()).arquivo("faturamento-exemplo"))
            .unwrap();
    assert!(!diario.contains("multa"), "{diario}");
}

#[test]
fn identificador_desconhecido_ou_de_outra_serie() {
    let dir = tempfile::tempdir().unwrap();
    let dados = preparar(dir.path());
    let amb = Ambiente::do_diretorio(&fixture(), &dados).unwrap();
    let erro = amb
        .analises
        .carregar(&Id::analisar("A-0042").unwrap())
        .unwrap_err();
    assert!(matches!(erro, Erro::NaoEncontrada(_)));
    assert!(erro.to_string().contains("A-0042"));
    let erro = amb
        .analises
        .carregar(&Id::analisar("S-0001").unwrap())
        .unwrap_err();
    assert!(matches!(erro, Erro::NaoEAnalise(_)));
}

#[test]
fn varias_threads_nao_perdem_registros() {
    let dir = tempfile::tempdir().unwrap();
    let dados = preparar(dir.path());
    let amb = Ambiente::do_diretorio(&fixture(), &dados).unwrap();
    let id = amb.analises.criar(nova("Concorrência")).unwrap().id;
    std::thread::scope(|s| {
        for t in 0..4 {
            // Cada thread com a própria instância, como processos distintos.
            let analises = Analises::new(amb.kb.clone(), Arc::new(SemNotificacao));
            let id = id.clone();
            s.spawn(move || {
                for i in 0..10 {
                    analises
                        .alterar(&id, Mudanca::Afirmacao, |a, agora| {
                            a.registrar_afirmacao(afirmacao(&format!("t{t} i{i}")), agora)
                        })
                        .unwrap();
                }
            });
        }
    });
    let a = amb.analises.carregar(&id).unwrap();
    assert_eq!(a.afirmacoes.len(), 40);
    assert_eq!(a.versao, 41);
    let mut numeros: Vec<u32> = a.afirmacoes.iter().map(|x| x.numero).collect();
    numeros.sort();
    assert_eq!(numeros, (1..=40).collect::<Vec<_>>());
}

/// Dois (ou mais) processos — como duas sessões com `cba mcp` — criando análises e
/// registrando na mesma análise ao mesmo tempo: nada se perde nem se corrompe.
#[test]
fn varios_processos_nao_perdem_registros() {
    const PROCESSOS: usize = 3;
    const REGISTROS: usize = 15;
    let dir = tempfile::tempdir().unwrap();
    let dados = preparar(dir.path());
    let amb = Ambiente::do_diretorio(&fixture(), &dados).unwrap();
    let id = amb.analises.criar(nova("Compartilhada")).unwrap().id;

    let exe = std::env::current_exe().unwrap();
    let filhos: Vec<_> = (0..PROCESSOS)
        .map(|_| {
            Command::new(&exe)
                .args(["processo_filho", "--exact", "--nocapture"])
                .env(VAR_FILHO, dados.raiz())
                .env("CBA_TESTE_ANALISE", id.to_string())
                .env("CBA_TESTE_REGISTROS", REGISTROS.to_string())
                .spawn()
                .unwrap()
        })
        .collect();
    for mut f in filhos {
        assert!(f.wait().unwrap().success());
    }

    let a = amb.analises.carregar(&id).unwrap();
    assert_eq!(a.afirmacoes.len(), PROCESSOS * REGISTROS);
    // Cada processo criou também uma análise própria: identificadores distintos.
    let ids: Vec<String> = amb
        .analises
        .listar()
        .unwrap()
        .iter()
        .map(|a| a.id.to_string())
        .collect();
    assert_eq!(ids.len(), PROCESSOS + 1, "{ids:?}");
    let eventos = DiarioEventos::em(dados.eventos())
        .ler("faturamento-exemplo")
        .unwrap();
    assert_eq!(eventos.len(), 1 + PROCESSOS * (1 + REGISTROS));
}

/// Trabalho de um processo filho de `varios_processos_nao_perdem_registros`. Sem a
/// variável, não faz nada.
#[test]
fn processo_filho() {
    let Some(dados) = std::env::var_os(VAR_FILHO) else {
        return;
    };
    let dados = DiretorioDados::em(dados);
    let id = Id::analisar(&std::env::var("CBA_TESTE_ANALISE").unwrap()).unwrap();
    let registros: usize = std::env::var("CBA_TESTE_REGISTROS")
        .unwrap()
        .parse()
        .unwrap();
    let amb = Ambiente::do_diretorio(&fixture(), &dados).unwrap();
    amb.analises.criar(nova("Do filho")).unwrap();
    for i in 0..registros {
        amb.analises
            .alterar(&id, Mudanca::Afirmacao, |a, agora| {
                a.registrar_afirmacao(
                    afirmacao(&format!("processo {} registro {i}", std::process::id())),
                    agora,
                )
            })
            .unwrap();
    }
}
