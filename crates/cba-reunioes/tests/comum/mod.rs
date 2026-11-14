//! Preparação comum aos testes da guarda: a fixture da reunião 1 e uma base
//! com o projeto "Faturamento Exemplo", uma demanda e a reunião.

#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

use cba_docs::EstadoAta;
use cba_kb::frontmatter::{Data, FrontmatterProjeto};
use cba_kb::{Base, Documento, IdReuniao, NovaDemanda, Projeto, TipoDocumento};
use cba_reunioes::{Transcript, analisar};

pub const PROJETO: &str = "faturamento-exemplo";

pub fn data(texto: &str) -> Data {
    Data::analisar(texto).unwrap()
}

/// Milissegundos (UTC) do início do dia.
pub fn ms(texto: &str) -> u64 {
    cba_core::tempo::dias_desde_1970(data(texto)) as u64 * 86_400_000
}

pub fn fixture(nome: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/reunioes")
        .join(nome)
}

/// Transcript da reunião 1 (`.vtt`).
pub fn reuniao_1() -> Transcript {
    let nome = "reuniao-01-cancelamento.vtt";
    analisar(&fs::read(fixture(nome)).unwrap(), Some(nome))
}

/// Base com o projeto, uma demanda e uma reunião (estado da ata gravado).
pub fn base_com_reuniao(raiz: &Path) -> (Base, Projeto, IdReuniao) {
    let base = Base::criar(raiz.join("base")).unwrap();
    let projeto = base
        .criar_projeto(&Documento::novo(
            FrontmatterProjeto {
                tipo: TipoDocumento::Projeto,
                id: PROJETO.into(),
                nome: "Faturamento Exemplo".into(),
                areas: vec![],
                criado: Some(data("2026-11-01")),
            },
            "\n# Faturamento Exemplo\n",
        ))
        .unwrap();
    let demanda = projeto
        .criar_demanda(
            &NovaDemanda {
                titulo: "Trava de cancelamento de fatura".into(),
                ..Default::default()
            },
            data("2026-11-10"),
        )
        .unwrap()
        .id;
    let (estado, _) = EstadoAta::criar(&projeto, demanda, data("2026-11-12")).unwrap();
    (base, projeto, estado.id)
}
