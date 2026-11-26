//! Testes dos bancos do projeto, do banco da análise e da decisão do analista
//! sobre as consultas (#440).

use std::collections::BTreeMap;

use super::*;
use crate::teste_base::{PROJETO, cenario};
use cba_core::banco::OrigemBanco;
use cba_core::consultas::SituacaoConsulta;
use cba_repos::providers::FonteCredencial;

fn conexao(nome: &str, tipo: &str, endereco: &str) -> Conexao {
    let mut opcoes = BTreeMap::new();
    if tipo == "postgres" {
        opcoes.insert("usuario".into(), "leitura".into());
        opcoes.insert("ambiente".into(), "homologação".into());
    }
    Conexao {
        id: String::new(),
        nome: nome.into(),
        tipo: tipo.into(),
        endereco: endereco.into(),
        credencial: if tipo == "postgres" {
            FonteCredencial::Nenhuma
        } else {
            FonteCredencial::Chaveiro {
                conta: String::new(),
            }
        },
        opcoes,
    }
}

#[test]
fn bancos_do_projeto_ligar_na_analise_e_decidir_consultas() {
    let c = cenario();
    let banco = crate::conexoes::salvar(
        &c.dados,
        conexao("Banco de homologação", "postgres", "db.example.com/fat"),
    )
    .unwrap();
    let jira = crate::conexoes::salvar(
        &c.dados,
        conexao("Jira", "jira", "https://jira.example.com"),
    )
    .unwrap();

    // Só conexões de banco entram no projeto.
    let e = salvar_bancos(&c.dados, PROJETO, vec![jira.id.clone()]).unwrap_err();
    assert!(e.contains("não é de banco"), "{e}");
    assert!(salvar_bancos(&c.dados, PROJETO, vec!["nao-existe".into()]).is_err());
    let lista = salvar_bancos(&c.dados, PROJETO, vec![banco.id.clone(), banco.id.clone()]).unwrap();
    assert_eq!(lista.len(), 1);
    assert_eq!(lista[0].nome, "Banco de homologação");
    assert_eq!(lista[0].rotulo_tipo, "PostgreSQL");
    assert_eq!(lista[0].ambiente.as_deref(), Some("homologação"));
    assert_eq!(bancos(&c.dados, PROJETO).unwrap(), lista);
    // A conexão passa a constar como usada pelo projeto.
    let usos = crate::conexoes::usos(&c.dados).unwrap();
    assert_eq!(usos[&banco.id][0].onde, "banco de dados");

    // Banco da análise: só um do projeto.
    let criada = crate::analise::criar_analise(&c.dados, PROJETO, "Multa zerada?", None).unwrap();
    let id = criada.analise.id.to_string();
    let e = ligar_banco(&c.dados, PROJETO, &id, Some(&jira.id)).unwrap_err();
    assert!(e.contains("não é um banco deste projeto"), "{e}");
    ligar_banco(&c.dados, PROJETO, &id, Some(&banco.id)).unwrap();
    let ambiente = crate::analise::ambiente(&c.dados, PROJETO).unwrap();
    let kb_id = cba_kb::Id::analisar(&id).unwrap();
    let a = ambiente.analises.carregar(&kb_id).unwrap();
    assert_eq!(a.banco.as_ref().unwrap().conexao, banco.id);
    assert_eq!(a.banco.as_ref().unwrap().nome, "Banco de homologação");

    // O agente pede duas consultas; o analista executa uma com edição e recusa a outra.
    let origem = OrigemBanco {
        conexao: banco.id.clone(),
        nome: banco.nome.clone(),
        tipo: "PostgreSQL".into(),
        ambiente: Some("homologação".into()),
    };
    ambiente
        .analises
        .alterar(&kb_id, Mudanca::Consulta, |a, agora| {
            a.pedir_consulta(
                "medir",
                "SELECT count(*) FROM fatura",
                origem.clone(),
                agora,
            )?;
            a.pedir_consulta("ver", "SELECT * FROM cliente", origem.clone(), agora)
        })
        .unwrap();
    decidir_consulta(
        &c.dados,
        PROJETO,
        &id,
        1,
        DecisaoConsulta::EditarEExecutar {
            sql: "SELECT count(*) FROM fatura WHERE multa = 0".into(),
        },
    )
    .unwrap();
    decidir_consulta(
        &c.dados,
        PROJETO,
        &id,
        2,
        DecisaoConsulta::Recusar {
            motivo: "traz clientes inteiros".into(),
        },
    )
    .unwrap();
    // Decidida não muda mais.
    assert!(decidir_consulta(&c.dados, PROJETO, &id, 2, DecisaoConsulta::Executar).is_err());
    let a = ambiente.analises.carregar(&kb_id).unwrap();
    assert_eq!(a.consultas[0].situacao, SituacaoConsulta::Aprovada);
    assert!(a.consultas[0].editada);
    assert_eq!(a.consultas[0].decidida_por, quem());
    assert_eq!(a.consultas[1].situacao, SituacaoConsulta::Recusada);

    // A decisão chega pelo JSON da tela.
    let d: DecisaoConsulta =
        serde_json::from_value(serde_json::json!({"decisao": "recusar", "motivo": "x"})).unwrap();
    assert_eq!(d, DecisaoConsulta::Recusar { motivo: "x".into() });

    // Desligar o banco tira a ferramenta da sessão.
    ligar_banco(&c.dados, PROJETO, &id, None).unwrap();
    assert!(ambiente.analises.carregar(&kb_id).unwrap().banco.is_none());
}
