//! Testes do "O que mudou no código" (#124) com o sistema fictício "Faturamento
//! Exemplo" num repositório git temporário.

use std::path::Path;
use std::process::Command;

use cba_kb::frontmatter::{FrontmatterRegra, StatusRegra};
use cba_tools::mudancas::{AjusteDaProposta, Classe, SituacaoProposta};
use serde_json::json;

use super::*;
use crate::teste_base::{PKG, PROJETO, cenario};

fn git(dir: &Path, args: &[&str]) {
    let ok = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args([
            "-c",
            "user.name=Ana Exemplo",
            "-c",
            "user.email=ana@exemplo.invalid",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .status()
        .unwrap()
        .success();
    assert!(ok, "git {args:?}");
}

#[test]
fn relatorio_nota_e_confirmacao_da_proposta() {
    let c = cenario();
    git(&c.codigo, &["init", "-q", "-b", "main"]);
    git(&c.codigo, &["add", "-A"]);
    git(&c.codigo, &["commit", "-q", "-m", "Versão inicial"]);
    let aprovada = c.aprovar_analise_da_multa();
    let regra = aprovada.regras[0].clone();
    c.mudar_o_codigo_da_multa();

    // Desde a última análise aprovada até a cópia de trabalho.
    let r = relatorio_do_projeto(&c.dados, PROJETO, None, None).unwrap();
    assert_eq!(r.regras.len(), 1);
    assert_eq!(r.regras[0].id, regra.id.to_string());
    assert_eq!(r.regras[0].classe, Classe::Alterado);
    assert!(
        r.regras[0].trechos[0]
            .diff
            .contains("PERC_MULTA_ATRASO_NOVA")
    );
    let nota = nota_do_projeto(&c.dados, PROJETO, None, None).unwrap();
    assert!(nota.contains("## Regras com mudança no código"), "{nota}");
    let destino = c.dir.path().join("nota.md");
    let gravado = exportar_nota(&c.dados, PROJETO, None, None, destino.to_str().unwrap()).unwrap();
    assert_eq!(std::fs::read_to_string(gravado).unwrap(), nota);
    assert!(pedido_de_explicacao_app(None, None).contains("relatorio_de_mudancas"));

    // O agente propõe; o analista confirma no app.
    c.chamar(
        "propor_atualizacao_de_regra",
        json!({
            "regra": regra.id.to_string(),
            "comportamento_mudou": true,
            "explicacao": "A multa passou a usar o parâmetro novo, com quatro casas.",
            "enunciado": "Passada a carência, a multa usa o parâmetro PERC_MULTA_ATRASO_NOVA.",
            "evidencias": [{
                "arquivo": PKG, "linha_inicial": 71, "linha_final": 72,
                "trecho": "FN_PARAMETRO('PERC_MULTA_ATRASO_NOVA') / 100, 4)"
            }]
        }),
    );
    let propostas = propostas_do_projeto(&c.dados, PROJETO).unwrap();
    assert_eq!(propostas.len(), 1);
    assert_eq!(propostas[0].situacao, SituacaoProposta::Pendente);
    let atualizada = confirmar(&c.dados, PROJETO, 1, AjusteDaProposta::default()).unwrap();
    assert_eq!(atualizada.id, regra.id.to_string());
    assert_eq!(atualizada.documento, regra.arquivo);

    let doc = c
        .ambiente()
        .kb
        .ler::<FrontmatterRegra>(&regra.arquivo)
        .unwrap();
    assert_eq!(doc.frontmatter.id, regra.id);
    assert_eq!(doc.frontmatter.status, StatusRegra::Confirmada);
    assert_eq!(doc.frontmatter.historico.len(), 1);
    assert!(
        doc.corpo.contains("PERC_MULTA_ATRASO_NOVA"),
        "{}",
        doc.corpo
    );
    assert!(descartar(&c.dados, PROJETO, 1).is_err(), "já confirmada");
}
