//! Privacidade de ponta a ponta da reunião (#142): com o log em arquivo no nível
//! mais detalhado e o diário de eventos ligados, o fluxo inteiro (iniciar a
//! análise da reunião, ler o transcript, propor papel, registrar itens e o resumo,
//! concluir, recusar a aprovação sem confirmação e aprovar a ata) termina sem
//! nenhuma fala na base, no diretório de dados do app (índice, diário de eventos,
//! logs, guarda) fora os trechos curtos citados na ata aprovada.
//!
//! Fica num arquivo próprio porque liga o log global do processo de teste.

use std::fs;
use std::path::{Path, PathBuf};

use cba_core::ambiente::Ambiente;
use cba_core::config::Config;
use cba_core::dados::DiretorioDados;
use cba_docs::EstadoAta;
use cba_kb::frontmatter::{Data, StatusAta};
use cba_kb::{Id, NovaDemanda};
use cba_reunioes::{Fala, Guarda, analisar};
use cba_tools::aprovacao_ata::aprovar_ata;
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

/// `palavras` palavras da fala `indice` (1-based), a partir da `desde`.
fn trecho(falas: &[Fala], indice: usize, desde: usize, palavras: usize) -> String {
    falas[indice - 1]
        .texto
        .split_whitespace()
        .skip(desde)
        .take(palavras)
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn depois_de_aprovar_a_ata_so_os_trechos_citados_ficam() {
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
    let hoje = Data::analisar("2026-11-14").unwrap();
    let demanda = amb
        .kb
        .criar_demanda(
            &NovaDemanda {
                titulo: "Trava do supervisor".into(),
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
    let chamar = |nome: &str, args: Value| -> Value {
        match registro.chamar(nome, Ok(&amb), Some(args)) {
            Ok(s) => s.dados.unwrap(),
            Err(ErroChamada::Ferramenta(e)) => panic!("{nome}: {e}"),
            Err(e) => panic!("{e}"),
        }
    };
    let r = ata.id.to_string();
    let d = chamar(
        "iniciar_analise",
        json!({ "fluxo": "reuniao", "reuniao": r }),
    );
    let analise = Id::analisar(d["analise"].as_str().unwrap()).unwrap();
    chamar("ler_transcript", json!({ "reuniao": r, "limite": 200 }));

    // Os trechos citados: curtos, de falas diferentes.
    let citados = [
        (3, trecho(&falas, 3, 2, 8)),
        (8, trecho(&falas, 8, 1, 7)),
        (4, trecho(&falas, 4, 0, 6)),
    ];
    let citacao = |n: usize| json!([{ "indice": citados[n].0, "trecho": citados[n].1 }]);
    chamar(
        "propor_papel",
        json!({
            "reuniao": r, "participante": falas[2].falante.clone().unwrap(),
            "papel": "desenvolvedor", "citacoes": citacao(0)
        }),
    );
    chamar(
        "registrar_item_ata",
        json!({ "reuniao": r, "tipo": "necessidade", "texto": "Checagem em todos os canais.",
                "citacoes": citacao(1) }),
    );
    chamar(
        "registrar_item_ata",
        json!({ "reuniao": r, "tipo": "pendencia", "texto": "Listar os canais.",
                "responsavel": "Ana", "citacoes": citacao(2) }),
    );
    chamar(
        "registrar_item_ata",
        json!({ "reuniao": r, "tipo": "duvida", "texto": "Vale para o portal?",
                "citacoes": citacao(1) }),
    );
    chamar(
        "registrar_item_ata",
        json!({ "reuniao": r, "tipo": "proximo_passo", "texto": "Medir o impacto.",
                "referencias": ["N-1"], "citacoes": citacao(1) }),
    );
    chamar(
        "registrar_resumo_ata",
        json!({ "reuniao": r, "objetivo": "Combinar a checagem.",
                "resumo": "Reunião curta para acertar a regra nova." }),
    );
    chamar("concluir_analise", json!({ "analise": analise }));

    // Sem confirmar, nada muda; confirmando, a ata é aprovada e o transcript some.
    assert!(aprovar_ata(&amb, &analise, false, hoje).is_err());
    assert!(Guarda::nova(&dados).existe(PROJETO, ata.id));
    let feita = aprovar_ata(&amb, &analise, true, hoje).unwrap();
    assert!(feita.transcript_apagado);
    assert_eq!(
        EstadoAta::ler(&amb.kb, ata.id).unwrap().0.status,
        StatusAta::Aprovada
    );
    assert!(!Guarda::nova(&dados).existe(PROJETO, ata.id));

    // O log registrou o fluxo, só com identificadores.
    let mut escritos = Vec::new();
    arquivos(raiz, &mut escritos);
    let log: String = escritos
        .iter()
        .filter(|c| c.starts_with(dados.logs()))
        .map(|c| fs::read_to_string(c).unwrap())
        .collect();
    for esperado in [
        "item registrado na ata",
        "transcript apagado",
        "ata aprovada",
    ] {
        assert!(
            log.contains(esperado),
            "o log não registrou `{esperado}`:\n{log}"
        );
    }
    assert!(escritos.iter().any(|c| c.starts_with(dados.eventos())));
    // A ata aprovada, com os trechos, está na base (o da proposta de papel, só no
    // estado da ata).
    let arquivos_da_ata = amb.kb.reuniao(ata.id).unwrap();
    let texto_da_ata = [&arquivos_da_ata.ata, &arquivos_da_ata.estado]
        .map(|a| amb.kb.ler_texto(a).unwrap())
        .concat()
        .to_lowercase();
    for (_, t) in &citados {
        assert!(texto_da_ata.contains(&t.to_lowercase()), "{t}");
    }

    // Nenhuma sequência de 4 palavras de fala fora dos trechos citados: na base,
    // no diretório de dados (índice, diário de eventos, logs) e em qualquer outro
    // arquivo do teste.
    // Os textos do produto copiados para a base (roteiros e templates) podem
    // coincidir com uma fala ("a análise de impacto"): não são vazamento.
    let templates = raiz.join("base").join("_templates");
    let do_produto: String = escritos
        .iter()
        .filter(|c| c.starts_with(&templates))
        .map(|c| fs::read_to_string(c).unwrap_or_default().to_lowercase())
        .collect();
    let trechos: Vec<String> = citados.iter().map(|(_, t)| t.to_lowercase()).collect();
    let proibidas: Vec<String> = sequencias(&falas)
        .into_iter()
        .filter(|s| !trechos.iter().any(|t| t.contains(s.as_str())))
        .filter(|s| !do_produto.contains(s.as_str()))
        .collect();
    assert!(proibidas.len() > 50);
    assert!(escritos.iter().any(|c| c.starts_with(raiz.join("base"))));
    assert!(escritos.iter().any(|c| c.starts_with(dados.raiz())));
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
}
