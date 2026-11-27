//! Dados pessoais do texto da demanda na exportação (#454): uma análise de
//! impacto cuja demanda cita CPF, CNPJ, e-mail e telefone fictícios sai, em
//! Markdown, com os quatro mascarados; com a preferência do projeto desligada,
//! sai como veio. A base local fica sempre com o texto original.

use std::path::{Path, PathBuf};

use cba_core::ambiente::Ambiente;
use cba_core::analise::{Analise, Fluxo};
use cba_core::analises::NovaAnalise;
use cba_core::config::Config;
use cba_core::dados::DiretorioDados;
use cba_tools::revisao::{Formato, OpcoesExportacao, exportar_analise};

const DEMANDA: &str = "Fatura cobrada em duplicidade

A cliente Maria Exemplo (CPF 529.982.247-25), da empresa 11.222.333/0001-81,
reclamou pelo e-mail maria.exemplo@example.com e pelo telefone (11) 98765-4321.
A multa de R$ 1.234,56 foi lançada em 2026-11-26 no protocolo 1234-5678.";

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/faturamento-exemplo")
}

fn ambiente(dir: &Path) -> Ambiente {
    let dados = DiretorioDados::em(dir.join("dados"));
    let projeto = cba_index::Projeto::novo("Faturamento Exemplo", [fixture()]).unwrap();
    assert!(projeto.mascarar_demanda, "ligada por padrão");
    cba_index::Cadastro::em(dados.projetos())
        .cadastrar(&projeto)
        .unwrap();
    cba_kb::Base::criar(dir.join("base")).unwrap();
    Config {
        base: Some(dir.join("base")),
        ..Default::default()
    }
    .gravar(&dados)
    .unwrap();
    Ambiente::do_diretorio(&fixture(), &dados).unwrap()
}

fn analise(amb: &Ambiente) -> Analise {
    let a = amb
        .analises
        .criar(NovaAnalise {
            fluxo: Fluxo::Impacto,
            titulo: "Cobrança em duplicidade".into(),
            entrada: DEMANDA.into(),
            analista: Some("Ana Exemplo".into()),
            base: None,
        })
        .unwrap();
    cba_tools::documento::remontar(&amb.base, &amb.kb, &a).unwrap();
    a
}

fn exportar(amb: &Ambiente, a: &Analise, destino: &Path) -> String {
    let md = exportar_analise(
        amb,
        &a.id,
        &destino.join("impacto"),
        Formato::Markdown,
        &OpcoesExportacao::default(),
        &cba_docs::Figuras::default(),
    )
    .unwrap();
    std::fs::read_to_string(md).unwrap()
}

#[test]
fn exportacao_mascara_cpf_cnpj_email_e_telefone_da_demanda() {
    let dir = tempfile::tempdir().unwrap();
    let amb = ambiente(dir.path());
    let a = analise(&amb);
    let destino = tempfile::tempdir().unwrap();

    let texto = exportar(&amb, &a, destino.path());
    assert!(texto.contains("## Demanda como veio"), "{texto}");
    for (original, marca) in [
        ("529.982.247-25", "[CPF mascarado]"),
        ("11.222.333/0001-81", "[CNPJ mascarado]"),
        ("maria.exemplo@example.com", "[e-mail mascarado]"),
        ("(11) 98765-4321", "[telefone mascarado]"),
    ] {
        assert!(!texto.contains(original), "{original}: {texto}");
        assert!(texto.contains(marca), "{marca}: {texto}");
    }
    // Valores, datas e códigos ficam.
    assert!(texto.contains("R$ 1.234,56"), "{texto}");
    assert!(texto.contains("2026-11-26"), "{texto}");
    assert!(texto.contains("protocolo 1234-5678"), "{texto}");
    // A base local continua com o texto original.
    let na_base = std::fs::read_to_string(amb.kb.caminho(&a.documento)).unwrap();
    assert!(na_base.contains("529.982.247-25"), "{na_base}");
    assert!(na_base.contains("maria.exemplo@example.com"), "{na_base}");
}

#[test]
fn com_a_preferencia_desligada_a_demanda_sai_como_veio() {
    let dir = tempfile::tempdir().unwrap();
    let mut amb = ambiente(dir.path());
    amb.projeto.mascarar_demanda = false;
    let a = analise(&amb);
    let destino = tempfile::tempdir().unwrap();

    let texto = exportar(&amb, &a, destino.path());
    for original in [
        "529.982.247-25",
        "11.222.333/0001-81",
        "maria.exemplo@example.com",
        "(11) 98765-4321",
    ] {
        assert!(texto.contains(original), "{original}: {texto}");
    }
    assert!(!texto.contains("mascarado]"), "{texto}");
}
