//! Testes da visão "Documentação" do mapa (#462) com o sistema fictício
//! "Faturamento Exemplo": a tabela segue os entregáveis do projeto, o lote dos
//! tipos do app sai na hora como rascunho e a revisão final libera a
//! publicação.

use cba_docs::entregaveis;
use cba_index::{Metodologia, TipoHabilitado};
use serde_json::json;

use super::*;
use crate::teste_base::{PROJETO, cenario};

fn registrar_funcionalidade(c: &crate::teste_base::Cenario) -> String {
    let amb = ambiente(&c.dados, PROJETO).unwrap();
    cba_tools::registro_padrao()
        .chamar(
            "registrar_funcionalidade",
            Ok(&amb),
            Some(json!({
                "nome": "Cancelar fatura",
                "area": "Cancelamento",
                "descricao": "Cancela uma fatura aberta, com motivo.",
                "atores": ["Atendente"],
                "pontos_de_entrada": [{
                    "ponto": "PKG_COBRANCA.PRC_CANCELAR_FATURA",
                    "evidencia": {
                        "arquivo": "db/03_pkg_cobranca.sql",
                        "linha_inicial": 99, "linha_final": 99,
                        "trecho": "PROCEDURE PRC_CANCELAR_FATURA"
                    }
                }]
            })),
        )
        .unwrap();
    "F-CAN-001".to_owned()
}

#[test]
fn tabela_lote_e_revisao_final() {
    let c = cenario();
    let f = registrar_funcionalidade(&c);

    // Sem configuração: os tipos que o app já gerava (manual e negocial).
    let t = tabela(&c.dados, PROJETO).unwrap();
    let colunas: Vec<&str> = t.colunas.iter().map(|c| c.tipo).collect();
    assert_eq!(
        colunas,
        [entregaveis::MANUAL, entregaveis::DOCUMENTO_NEGOCIAL]
    );

    // RUP com o manual.
    let mut rup = entregaveis::predefinicao(Metodologia::Rup)
        .unwrap()
        .entregaveis();
    rup.tipos.push(TipoHabilitado::novo(entregaveis::MANUAL));
    crate::configuracoes::salvar_entregaveis(&c.dados, PROJETO, rup).unwrap();
    let t = tabela(&c.dados, PROJETO).unwrap();
    let j = serde_json::to_value(&t).unwrap();
    assert_eq!(j["colunas"][0]["tipo"], "caso-de-uso");
    assert_eq!(j["colunas"][0]["geracao"], "agente");
    assert_eq!(j["colunas"][1]["geracao"], "app");
    assert_eq!(j["linhas"][0]["funcionalidade"], f);
    assert_eq!(j["linhas"][0]["areaNome"], "Cancelamento");
    assert_eq!(
        j["linhas"][0]["documentos"][0]["estado"]["estado"],
        "sem_documento"
    );
    assert_eq!(
        j["linhas"][0]["documentos"][0]["acao"],
        "Gerar versão inicial"
    );
    assert_eq!(j["cobertura"][0]["semDocumento"], 1);
    // O caso de uso exige regras ligadas; a funcionalidade ainda não tem.
    assert_eq!(j["colunas"][0]["exigeRegras"], true);
    assert_eq!(j["colunas"][1]["exigeRegras"], false);
    assert_eq!(j["linhas"][0]["regras"], 0);

    // Catálogo de regras: montado pelo app, fica como rascunho.
    let lote = gerar_em_lote(
        &c.dados,
        PROJETO,
        entregaveis::CATALOGO_REGRAS,
        std::slice::from_ref(&f),
    )
    .unwrap();
    let j = serde_json::to_value(&lote).unwrap();
    assert_eq!(j["itens"][0]["resultado"], "gerada");
    let documento = j["itens"][0]["caminho"].as_str().unwrap().to_owned();

    // Caso de uso: cada etapa cita uma regra; sem regras ligadas, a
    // funcionalidade fica fora do lote e a tabela avisa antes.
    let lote = gerar_em_lote(
        &c.dados,
        PROJETO,
        entregaveis::CASO_DE_USO,
        std::slice::from_ref(&f),
    )
    .unwrap();
    let j = serde_json::to_value(&lote).unwrap();
    assert_eq!(j["itens"][0]["resultado"], "pulada");
    let motivo = j["itens"][0]["motivo"].as_str().unwrap();
    assert!(motivo.contains("sem regras de negócio"), "{motivo}");

    let p = proposta(&c.dados, PROJETO, &documento).unwrap();
    assert_eq!(p.funcionalidade.as_deref(), Some(f.as_str()));
    assert_eq!(p.publicada, None);
    assert_eq!(p.proposta.versao.to_string(), "1.0");
    assert!(p.proposta.revisada.is_none());
    let t = tabela(&c.dados, PROJETO).unwrap();
    let j = serde_json::to_value(&t).unwrap();
    assert_eq!(
        j["linhas"][0]["documentos"][1]["estado"]["estado"],
        "proposta_pendente"
    );
    assert_eq!(j["linhas"][0]["documentos"][1]["acao"], "Revisar proposta");

    // Sem a revisão final, não publica.
    let erro = publicar(&c.dados, PROJETO, &documento).unwrap_err();
    assert!(erro.contains("revisão final"), "{erro}");
    let revisada = marcar_revisada(&c.dados, PROJETO, &documento).unwrap();
    assert!(revisada.revisada.is_some());
    let publicada = publicar(&c.dados, PROJETO, &documento).unwrap();
    assert_eq!(publicada.revisao.versao.to_string(), "1.0");
    let t = tabela(&c.dados, PROJETO).unwrap();
    assert_eq!(t.cobertura[1].em_dia, 1);
    // Em dia e sem mudança no mapa: não há o que atualizar.
    let erro = propor_atualizacao(&c.dados, PROJETO, entregaveis::CATALOGO_REGRAS, &f).unwrap_err();
    assert!(erro.contains("igual à versão publicada"), "{erro}");

    // Descartar uma 1.0 em rascunho volta a "sem documento".
    let lote = gerar_em_lote(
        &c.dados,
        PROJETO,
        entregaveis::ESPECIFICACAO_SUPLEMENTAR,
        std::slice::from_ref(&f),
    )
    .unwrap();
    let j = serde_json::to_value(&lote).unwrap();
    let suplementar = j["itens"][0]["caminho"].as_str().unwrap().to_owned();
    descartar(&c.dados, PROJETO, &suplementar).unwrap();
    assert_eq!(
        tabela(&c.dados, PROJETO).unwrap().cobertura[2].sem_documento,
        1
    );
    assert!(proposta(&c.dados, PROJETO, &suplementar).is_err());
}
