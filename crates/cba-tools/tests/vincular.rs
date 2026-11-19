//! Vínculo das issues do rastreador às funcionalidades (#270), na fixture
//! "Faturamento Exemplo": a issue que cita o parcelamento é sugerida para a
//! funcionalidade de parcelamento, a sem relação fica sem funcionalidade,
//! confirmar cria a demanda com `chave` e endereço e liga à funcionalidade, e
//! repetir a importação não duplica nada.
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use cba_core::ambiente::Ambiente;
use cba_core::config::Config;
use cba_core::dados::DiretorioDados;
use cba_core::funcionalidades::MapaFuncionalidades;
use cba_core::vinculos::{TextoDaIssue, sugerir};
use cba_repos::GithubConfig;
use cba_repos::demanda::Ferramentas;
use cba_repos::listagem::{Periodo, listar_github};
use cba_tools::vincular::{IssueConfirmada, confirmar_vinculos};
use cba_tools::{ErroChamada, registro_padrao};
use serde_json::{Value, json};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/faturamento-exemplo")
}

struct Cenario {
    _dir: tempfile::TempDir,
    amb: Ambiente,
    pontos: Vec<cba_index::PontoEntrada>,
}

fn cenario() -> Cenario {
    let dir = tempfile::tempdir().unwrap();
    let raiz = dir.path();
    let dados = DiretorioDados::em(raiz.join("dados"));
    let projeto = cba_index::Projeto::novo("Faturamento Exemplo", [fixture()]).unwrap();
    let cadastro = cba_index::Cadastro::em(dados.projetos());
    cadastro.cadastrar(&projeto).unwrap();
    cadastro.inventariar(&projeto.id).unwrap();
    let pontos = cadastro.mapear(&projeto.id).unwrap().pontos_de_entrada();
    cba_kb::Base::criar(raiz.join("base")).unwrap();
    Config {
        base: Some(raiz.join("base")),
        ..Default::default()
    }
    .gravar(&dados)
    .unwrap();
    let amb = Ambiente::do_diretorio(&fixture(), &dados).unwrap();
    Cenario {
        _dir: dir,
        amb,
        pontos,
    }
}

impl Cenario {
    fn ponto(&self, sufixo: &str) -> Value {
        let p = self
            .pontos
            .iter()
            .find(|p| p.chave.ends_with(sufixo))
            .unwrap_or_else(|| panic!("ponto {sufixo}"));
        let texto = std::fs::read_to_string(fixture().join(&p.arquivo)).unwrap();
        let linha = texto
            .lines()
            .nth(p.linhas.inicio as usize - 1)
            .unwrap()
            .trim();
        json!({"ponto": p.chave, "evidencia": {"arquivo": p.arquivo,
            "linha_inicial": p.linhas.inicio, "linha_final": p.linhas.inicio, "trecho": linha}})
    }

    fn registrar(&self, nome: &str, area: &str, pontos: &[&str]) {
        let args = json!({
            "nome": nome, "area": area,
            "descricao": format!("{nome}, para o atendimento."),
            "atores": ["Atendente"],
            "pontos_de_entrada": pontos.iter().map(|p| self.ponto(p)).collect::<Vec<_>>(),
        });
        match registro_padrao().chamar("registrar_funcionalidade", Ok(&self.amb), Some(args)) {
            Ok(_) => {}
            Err(ErroChamada::Ferramenta(e)) => panic!("{e}"),
            Err(e) => panic!("{e}"),
        }
    }

    fn mapa(&self) -> MapaFuncionalidades {
        MapaFuncionalidades::carregar(&self.amb.kb).unwrap()
    }
}

fn gh_falso(dir: &Path) -> PathBuf {
    let p = dir.join("gh");
    std::fs::write(
        &p,
        r#"#!/bin/sh
echo '[{"number":41,"title":"Parcelamento calcula as parcelas errado","body":"Ao parcelar a fatura em 12x, o valor de cada parcela não bate.","url":"https://github.com/o/r/issues/41","createdAt":"2026-10-01T00:00:00Z","state":"OPEN","labels":[{"name":"bug"}]},{"number":42,"title":"Trocar a cor do botão","body":"O azul está muito claro.","url":"https://github.com/o/r/issues/42","createdAt":"2026-10-02T00:00:00Z","state":"CLOSED","labels":[]}]'
"#,
    )
    .unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    p
}

#[test]
fn sugere_confirma_e_repetir_nao_duplica() {
    let c = cenario();
    c.registrar(
        "Parcelar fatura",
        "Parcelamento",
        &[
            "ParcelamentoService.parcelar",
            "PKG_SITUACAO.PRC_EFETIVAR_PARCELAMENTO",
        ],
    );
    c.registrar(
        "Cancelar fatura",
        "Cancelamento",
        &["#FrmCancelamento", "PKG_COBRANCA.PRC_CANCELAR_FATURA"],
    );
    let ferramentas = tempfile::tempdir().unwrap();
    let f = Ferramentas {
        gh: gh_falso(ferramentas.path()),
        ..Default::default()
    };
    let lista = listar_github(
        &GithubConfig { repo: "o/r".into() },
        &Periodo::default(),
        50,
        &f,
    )
    .unwrap();
    assert_eq!(lista.issues.len(), 2);

    let mapa = c.mapa();
    let sugestoes: Vec<_> = lista
        .issues
        .iter()
        .map(|i| {
            sugerir(
                &mapa,
                &TextoDaIssue {
                    chave: i.chave.clone(),
                    titulo: i.titulo.clone(),
                    descricao: i.descricao.clone(),
                    arquivos: vec![],
                },
            )
        })
        .collect();
    assert_eq!(sugestoes[0].len(), 1, "{:?}", sugestoes[0]);
    assert_eq!(sugestoes[0][0].nome, "Parcelar fatura");
    assert!(sugestoes[1].is_empty(), "{:?}", sugestoes[1]);

    let lote = vec![
        IssueConfirmada {
            issue: lista.issues[0].clone(),
            funcionalidades: vec![sugestoes[0][0].funcionalidade.clone()],
        },
        // Sem funcionalidade: não vira demanda.
        IssueConfirmada {
            issue: lista.issues[1].clone(),
            funcionalidades: vec![],
        },
    ];
    let r = confirmar_vinculos(&c.amb, &lote).unwrap();
    assert_eq!(r.resultado.criadas.len(), 1);
    assert_eq!(r.resultado.vinculos_novos, 1);
    let demandas = c.amb.kb.demandas().unwrap();
    assert_eq!(demandas.len(), 1);
    let fm = &demandas[0].documento.frontmatter;
    assert_eq!(fm.chave.as_deref(), Some("#41"));
    assert_eq!(
        fm.origem.as_deref(),
        Some("https://github.com/o/r/issues/41")
    );
    let id = demandas[0].id;
    assert_eq!(
        c.mapa()
            .funcionalidade(&sugestoes[0][0].funcionalidade)
            .unwrap()
            .demandas,
        vec![id]
    );

    // De novo: reaproveita a demanda e não duplica o vínculo.
    let v = c.mapa().versao;
    let r = confirmar_vinculos(&c.amb, &lote).unwrap();
    assert!(r.resultado.criadas.is_empty());
    assert_eq!(r.resultado.reaproveitadas, vec![id]);
    assert_eq!(
        (r.resultado.vinculos_novos, r.resultado.vinculos_existentes),
        (0, 1)
    );
    assert_eq!(c.amb.kb.demandas().unwrap().len(), 1);
    assert_eq!(
        c.mapa()
            .funcionalidade(&sugestoes[0][0].funcionalidade)
            .unwrap()
            .demandas,
        vec![id]
    );
    assert!(c.mapa().versao >= v);
}
