//! Cenário dos testes das telas do início, do sistema e da base: uma cópia do
//! sistema fictício "Faturamento Exemplo" numa pasta temporária (para a revalidação
//! poder mudar o código), cadastrada pelo primeiro uso, com uma base vazia.

use std::fs;
use std::path::{Path, PathBuf};

use cba_core::ambiente::Ambiente;
use cba_core::analise::{EstadoAfirmacao, EstadoTermo};
use cba_core::dados::DiretorioDados;
use cba_core::eventos::Mudanca;
use cba_kb::Id;
use cba_tools::aprovacao::{Aprovacao, OpcoesAprovacao, aprovar};
use serde_json::json;

use crate::{analise, primeiro_uso};

pub const SISTEMA: &str = "faturamento-exemplo";
pub const PKG: &str = "db/03_pkg_cobranca.sql";

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/faturamento-exemplo")
        .canonicalize()
        .unwrap()
}

fn copiar(de: &Path, para: &Path) {
    fs::create_dir_all(para).unwrap();
    for entrada in fs::read_dir(de).unwrap() {
        let entrada = entrada.unwrap();
        let destino = para.join(entrada.file_name());
        if entrada.file_type().unwrap().is_dir() {
            copiar(&entrada.path(), &destino);
        } else {
            fs::copy(entrada.path(), destino).unwrap();
        }
    }
}

pub struct Cenario {
    pub dir: tempfile::TempDir,
    pub dados: DiretorioDados,
    pub codigo: PathBuf,
}

pub fn cenario() -> Cenario {
    let dir = tempfile::tempdir().unwrap();
    let codigo = dir.path().join("codigo").join("faturamento-exemplo");
    copiar(&fixture(), &codigo);
    let dados = DiretorioDados::em(dir.path().join("dados"));
    let base = dir.path().join("base");
    primeiro_uso::configurar_base(
        &dados,
        &primeiro_uso::Restricoes::default(),
        base.to_str().unwrap(),
    )
    .unwrap();
    primeiro_uso::cadastrar_sistema(
        &dados,
        &primeiro_uso::PedidoSistema {
            nome: "Faturamento Exemplo".into(),
            pastas: vec![codigo.to_str().unwrap().into()],
            ..Default::default()
        },
    )
    .unwrap();
    Cenario { dir, dados, codigo }
}

impl Cenario {
    pub fn ambiente(&self) -> Ambiente {
        analise::ambiente(&self.dados, SISTEMA).unwrap()
    }

    pub fn chamar(&self, ferramenta: &str, args: serde_json::Value) {
        if let Err(e) =
            cba_tools::registro_padrao().chamar(ferramenta, Ok(&self.ambiente()), Some(args))
        {
            panic!("{ferramenta}: {e}");
        }
    }

    /// A análise da multa, do início à aprovação: uma afirmação confirmada (vira a
    /// regra) e um termo aceito (vira o glossário).
    pub fn aprovar_analise_da_multa(&self) -> Aprovacao {
        let criada = analise::criar_analise(
            &self.dados,
            SISTEMA,
            "Como é calculada a multa por atraso de pagamento?",
            Some("Multa por atraso"),
        )
        .unwrap();
        let id = criada.analise.id;
        self.chamar("iniciar_analise", json!({"analise": id}));
        self.chamar(
            "afirmar",
            json!({
                "analise": id,
                "secao": "resposta",
                "texto": "Passada a carência, a multa é de 2% sobre o valor original.",
                "confianca": "confirmado",
                "evidencias": [{
                    "arquivo": PKG,
                    "linha_inicial": 71,
                    "linha_final": 72,
                    "trecho": "FN_PARAMETRO('PERC_MULTA_ATRASO') / 100, 2)"
                }]
            }),
        );
        self.chamar(
            "propor_termo",
            json!({
                "analise": id,
                "termo": "Carência",
                "definicao": "Dias depois do vencimento em que a fatura não recebe multa.",
                "area": "cobranca",
            }),
        );
        self.chamar("concluir_analise", json!({"analise": id}));
        let amb = self.ambiente();
        let id = Id::analisar(&id).unwrap();
        amb.analises
            .alterar(&id, Mudanca::Afirmacao, |a, _| {
                a.marcar_afirmacao(1, EstadoAfirmacao::Confirmada)?;
                a.decidir_termo(1, EstadoTermo::Aceito)
            })
            .unwrap();
        aprovar(
            &amb.base,
            &amb.analises,
            &id,
            &OpcoesAprovacao {
                area: Some("cobranca".into()),
                ..Default::default()
            },
        )
        .unwrap()
    }

    /// Muda as linhas citadas pela evidência da regra da multa.
    pub fn mudar_o_codigo_da_multa(&self) {
        let arquivo = self.codigo.join(PKG);
        let texto = fs::read_to_string(&arquivo).unwrap();
        let novo = texto.replace(
            "FN_PARAMETRO('PERC_MULTA_ATRASO') / 100, 2)",
            "FN_PARAMETRO('PERC_MULTA_ATRASO_NOVA') / 100, 4)",
        );
        assert_ne!(texto, novo);
        fs::write(&arquivo, novo).unwrap();
    }
}
