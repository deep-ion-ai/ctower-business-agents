//! Privacidade das ferramentas do fluxo "Analisar reunião" (#139): com o log em
//! arquivo no nível mais detalhado e o diário de eventos ligados, um fluxo
//! completo (iniciar a análise da reunião, ler o transcript, propor papel,
//! registrar itens, ter uma citação recusada, sugerir demanda) não deixa texto de
//! fala no log nem no diário, e o catálogo de pessoas não recebe falas.
//!
//! Fica num arquivo próprio porque liga o log global do processo de teste.

use std::fs;
use std::path::{Path, PathBuf};

use cba_core::ambiente::Ambiente;
use cba_core::config::Config;
use cba_core::dados::DiretorioDados;
use cba_docs::EstadoAta;
use cba_kb::NovaDemanda;
use cba_kb::frontmatter::Data;
use cba_reunioes::{Fala, Guarda, analisar};
use cba_tools::{ErroChamada, registro_padrao};
use serde_json::{Value, json};

const PROJETO: &str = "faturamento-exemplo";

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/faturamento-exemplo")
}

fn falas_da_reuniao() -> Vec<Fala> {
    let nome = "reuniao-01-cancelamento.vtt";
    let caminho = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/reunioes")
        .join(nome);
    analisar(&fs::read(caminho).unwrap(), Some(nome)).falas
}

/// Sequências de 4 palavras seguidas de cada fala, em minúsculas.
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

fn arquivos(pasta: &Path, saida: &mut Vec<PathBuf>) {
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
fn ferramentas_da_reuniao_nao_deixam_fala_no_log_nem_no_diario() {
    let dir = tempfile::tempdir().unwrap();
    let raiz = dir.path();
    let dados = DiretorioDados::em(raiz.join("dados"));
    // SAFETY: o teste é o único deste binário e ainda não há outras threads.
    unsafe { std::env::set_var(cba_core::log::VAR_FILTRO, "trace") };
    cba_core::log::iniciar(&dados.logs(), "teste").unwrap();

    let projeto = cba_index::Projeto::novo("Faturamento Exemplo", [fixture()]).unwrap();
    cba_index::Cadastro::em(dados.projetos())
        .cadastrar(&projeto)
        .unwrap();
    cba_kb::Base::criar(raiz.join("base")).unwrap();
    Config {
        base: Some(raiz.join("base")),
        ..Default::default()
    }
    .gravar(&dados)
    .unwrap();
    let amb = Ambiente::do_diretorio(&fixture(), &dados).unwrap();
    let hoje = Data::analisar("2026-11-12").unwrap();
    let demanda = amb
        .kb
        .criar_demanda(
            &NovaDemanda {
                titulo: "Trava de cancelamento de fatura".into(),
                ..Default::default()
            },
            hoje,
        )
        .unwrap()
        .id;
    let (ata, _) = EstadoAta::criar(&amb.kb, demanda, hoje).unwrap();
    let falas = falas_da_reuniao();
    Guarda::nova(&dados)
        .guardar(
            PROJETO,
            ata.id,
            &cba_reunioes::Transcript {
                formato: cba_reunioes::Formato::Vtt,
                falas: falas.clone(),
                avisos: vec![],
            },
            1,
        )
        .unwrap();

    let registro = registro_padrao();
    let chamar = |nome: &str, args: Value| -> Result<Value, String> {
        match registro.chamar(nome, Ok(&amb), Some(args)) {
            Ok(s) => Ok(s.dados.unwrap()),
            Err(ErroChamada::Ferramenta(e)) => Err(e.to_string()),
            Err(e) => panic!("{e}"),
        }
    };
    let r = ata.id.to_string();
    chamar(
        "iniciar_analise",
        json!({ "fluxo": "reuniao", "reuniao": r }),
    )
    .unwrap();
    chamar("ler_transcript", json!({ "reuniao": r, "limite": 200 })).unwrap();
    chamar(
        "ler_transcript",
        json!({ "reuniao": r, "falante": "Maria", "desde": "00:00:10" }),
    )
    .unwrap();
    // Trecho de verdade da fala 3, que confere, e o mesmo trecho na fala 4, que não.
    let trecho: String = falas[2]
        .texto
        .split_whitespace()
        .skip(2)
        .take(8)
        .collect::<Vec<_>>()
        .join(" ");
    chamar(
        "propor_papel",
        json!({
            "reuniao": r, "participante": falas[2].falante.clone().unwrap(),
            "papel": "desenvolvedor",
            "citacoes": [{ "indice": 3, "trecho": trecho }]
        }),
    )
    .unwrap();
    chamar(
        "registrar_item_ata",
        json!({
            "reuniao": r, "tipo": "regra", "texto": "Regra citada.",
            "citacoes": [{ "indice": 3, "trecho": trecho }]
        }),
    )
    .unwrap();
    let recusa = chamar(
        "registrar_item_ata",
        json!({
            "reuniao": r, "tipo": "regra", "texto": "Regra citada.",
            "citacoes": [{ "indice": 4, "trecho": trecho }]
        }),
    )
    .unwrap_err();
    assert!(recusa.contains("não aparece na fala 4"), "{recusa}");
    chamar(
        "sugerir_demanda",
        json!({ "texto": "cancelamento de fatura" }),
    )
    .unwrap();

    // O que foi escrito no log e no diário.
    let mut escritos = Vec::new();
    arquivos(&dados.logs(), &mut escritos);
    arquivos(&dados.eventos(), &mut escritos);
    assert!(escritos.iter().any(|c| c.starts_with(dados.logs())));
    assert!(escritos.iter().any(|c| c.starts_with(dados.eventos())));
    let log: String = escritos
        .iter()
        .filter(|c| c.starts_with(dados.logs()))
        .map(|c| fs::read_to_string(c).unwrap())
        .collect();
    for esperado in [
        "transcript lido",
        "papel proposto",
        "item registrado na ata",
        "citação recusada",
        "demandas sugeridas",
    ] {
        assert!(
            log.contains(esperado),
            "o log não registrou `{esperado}`:\n{log}"
        );
    }

    let proibidas = sequencias(&falas);
    assert!(proibidas.len() > 50);
    // A recusa também não ecoa a fala.
    for s in &proibidas {
        assert!(!recusa.to_lowercase().contains(s.as_str()), "{recusa}");
    }
    for caminho in &escritos {
        let texto = String::from_utf8_lossy(&fs::read(caminho).unwrap()).to_lowercase();
        for s in &proibidas {
            assert!(
                !texto.contains(s.as_str()),
                "{} contém texto de fala: `{s}`",
                caminho.display()
            );
        }
    }

    // O catálogo de pessoas não recebeu nada (a proposta fica na ata).
    let mut pessoas = Vec::new();
    arquivos(
        &amb.kb
            .caminho(&cba_kb::CaminhoRelativo::novo("pessoas").unwrap()),
        &mut pessoas,
    );
    assert!(pessoas.is_empty(), "{pessoas:?}");
}
