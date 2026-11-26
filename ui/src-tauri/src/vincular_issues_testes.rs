//! Vincular issues (#270) no app: leitura com o `gh` falso, sugestão, confirmação
//! e repetição sem duplicar.

use std::fs;
use std::os::unix::fs::PermissionsExt;

use super::*;
use crate::teste_base::{PROJETO, cenario};
use cba_core::funcionalidades::{Funcionalidade, SituacaoFuncionalidade};
use cba_index::Cadastro;
use cba_repos::{GithubConfig, TrackerConfig};

const ISSUES: &str = r#"[{"number":41,"title":"Parcelamento calcula as parcelas errado","body":"Ao parcelar a fatura em 12x o valor de cada parcela não bate.","url":"https://github.com/o/r/issues/41","createdAt":"2026-10-01T00:00:00Z","state":"OPEN","labels":[{"name":"bug"}]},{"number":42,"title":"Trocar a cor do botão","body":"O azul está muito claro.","url":"https://github.com/o/r/issues/42","createdAt":"2026-10-02T00:00:00Z","state":"CLOSED","labels":[]}]"#;

fn gh_falso(dir: &std::path::Path) -> Ferramentas {
    let gh = dir.join("gh");
    fs::write(&gh, format!("#!/bin/sh\necho '{ISSUES}'\n")).unwrap();
    fs::set_permissions(&gh, fs::Permissions::from_mode(0o755)).unwrap();
    Ferramentas {
        gh,
        ..Default::default()
    }
}

fn funcionalidade(n: u32, nome: &str, descricao: &str) -> Funcionalidade {
    Funcionalidade {
        id: IdFuncionalidade::novo("FAT", n).unwrap(),
        nome: nome.into(),
        area: "cobranca".into(),
        area_nome: "Cobrança".into(),
        descricao: descricao.into(),
        atores: vec![],
        pontos: vec![],
        dados_lidos: vec![],
        dados_gravados: vec![],
        relacionadas: vec![],
        situacao: SituacaoFuncionalidade::Aceita,
        juntada_em: None,
        editada: false,
        documento: None,
        explicacao: None,
        demandas: vec![],
        sessao: None,
        registrada_ms: 1,
        atualizada_ms: 1,
    }
}

#[test]
fn le_sugere_confirma_e_repetir_nao_duplica() {
    let c = cenario();
    let f = gh_falso(c.dir.path());
    // Sem rastreador cadastrado, a leitura recusa.
    assert!(
        issues_para_vincular(&c.dados, PROJETO, None, None, &f)
            .unwrap_err()
            .contains("repositório de issues")
    );
    let mut p = cadastrado(&c.dados, PROJETO).unwrap();
    p.repositorios.tracker = Some(TrackerConfig::Github(GithubConfig {
        repo: "o/r".into(),
        ..Default::default()
    }));
    Cadastro::em(c.dados.projetos()).salvar(&p).unwrap();

    let amb = ambiente(&c.dados, PROJETO).unwrap();
    MapaFuncionalidades::alterar(&amb.kb, |m, _| {
        m.funcionalidades.push(funcionalidade(
            1,
            "Parcelar fatura",
            "Divide o valor da fatura em parcelas.",
        ));
        m.funcionalidades.push(funcionalidade(
            2,
            "Cancelar nota",
            "Cancela a nota emitida.",
        ));
        Ok(())
    })
    .unwrap();

    let v = issues_para_vincular(&c.dados, PROJETO, Some(12), None, &f).unwrap();
    assert_eq!(v.issues.len(), 2);
    assert_eq!(v.funcionalidades.len(), 2);
    assert_eq!(v.issues[0].sugestoes[0].funcionalidade, "F-FAT-001");
    assert!(v.issues[1].sugestoes.is_empty());
    assert!(v.issues[0].ligada_a.is_empty());

    let itens = || {
        vec![ItemConfirmado {
            issue: IssueEntrada {
                origem: OrigemIssue::Github,
                chave: "#41".into(),
                titulo: "Parcelamento calcula as parcelas errado".into(),
                descricao: "d".into(),
                url: "https://github.com/o/r/issues/41".into(),
                criada_em: String::new(),
                estado: "aberta".into(),
                rotulos: vec![],
            },
            funcionalidades: vec!["F-FAT-001".into()],
        }]
    };
    let t = std::time::Instant::now();
    let r = confirmar_lote(&c.dados, PROJETO, itens()).unwrap();
    let _ = t;
    assert_eq!((r.criadas.len(), r.vinculos_novos), (1, 1));
    let t = std::time::Instant::now();
    let r2 = confirmar_lote(&c.dados, PROJETO, itens()).unwrap();
    let _ = t;
    assert!(r2.criadas.is_empty());
    assert_eq!((r2.vinculos_novos, r2.vinculos_existentes), (0, 1));
    assert_eq!(amb.kb.demandas().unwrap().len(), 1);

    // A releitura mostra a issue já ligada.
    let v = issues_para_vincular(&c.dados, PROJETO, None, None, &f).unwrap();
    assert_eq!(v.issues[0].ligada_a, vec!["F-FAT-001"]);
    assert_eq!(v.issues[0].demanda.as_deref(), Some(r.criadas[0].as_str()));

    // Justificar a issue sem funcionalidade (e desfazer).
    let entrada = IssueEntrada {
        origem: OrigemIssue::Github,
        chave: "#42".into(),
        titulo: "Trocar a cor do botão".into(),
        descricao: String::new(),
        url: "https://github.com/o/r/issues/42".into(),
        criada_em: String::new(),
        estado: "fechada".into(),
        rotulos: vec![],
    };
    let motivo = |texto: &str| {
        Some(JustificativaVista {
            motivo: MotivoIssue::ForaDoEscopo,
            texto: texto.into(),
        })
    };
    assert!(justificar(&c.dados, PROJETO, entrada.clone(), motivo("  ")).is_err());
    justificar(&c.dados, PROJETO, entrada.clone(), motivo("Só visual")).unwrap();
    let v = issues_para_vincular(&c.dados, PROJETO, None, None, &f).unwrap();
    assert_eq!(v.issues[1].justificativa, motivo("Só visual"));
    assert!(v.issues[0].justificativa.is_none());
    justificar(&c.dados, PROJETO, entrada.clone(), None).unwrap();
    let v = issues_para_vincular(&c.dados, PROJETO, None, None, &f).unwrap();
    assert!(v.issues[1].justificativa.is_none());

    // Proposta do agente: aparece como sugestão com a evidência e some ao confirmar.
    MapaFuncionalidades::alterar(&amb.kb, |m, agora| {
        m.propor_issue(cba_core::funcionalidades::PropostaDeIssue {
            origem: "github".into(),
            chave: "#42".into(),
            funcionalidades: vec![IdFuncionalidade::analisar("F-FAT-002").unwrap()],
            evidencia: "o botão de cancelar nota".into(),
            registrado_ms: agora,
        })
    })
    .unwrap();
    let v = issues_para_vincular(&c.dados, PROJETO, None, None, &f).unwrap();
    let s = &v.issues[1].sugestoes[0];
    assert!(s.ia);
    assert_eq!(s.funcionalidade, "F-FAT-002");
    assert!(s.evidencias[0].starts_with("IA: o botão"));
    let mut ia = itens();
    ia[0].issue.chave = "#42".into();
    ia[0].funcionalidades = vec!["F-FAT-002".into()];
    confirmar_lote(&c.dados, PROJETO, ia).unwrap();
    assert!(amb.kb.demandas().unwrap().len() >= 2);
    let mapa = MapaFuncionalidades::carregar(&amb.kb).unwrap();
    assert!(mapa.propostas_de_issue.is_empty());
    // Pedido ao agente: lista as issues e cita a ferramenta.
    let pedido = pedido_de_vinculo_ia(&c.dados, PROJETO, vec![entrada.clone()]).unwrap();
    assert!(pedido.contains("propor_vinculo_de_issue") && pedido.contains("`#42`"));
    // As funcionalidades abertas vão no próprio pedido.
    assert!(pedido.contains("F-FAT-001 · Parcelar fatura"), "{pedido}");
    assert!(pedido.contains("F-FAT-002 · Cancelar nota"));
    assert!(pedido_de_vinculo_ia(&c.dados, PROJETO, vec![]).is_err());

    // Funcionalidade inválida recusa o lote.
    let mut ruim = itens();
    ruim[0].funcionalidades = vec!["x".into()];
    assert!(confirmar_lote(&c.dados, PROJETO, ruim).is_err());
}
