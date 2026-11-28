//! Entregáveis da demanda (#464) com o sistema fictício "Faturamento Exemplo":
//! o preparo da avaliação (a demanda do gabarito, a análise de impacto validada
//! e, no caso de uso, a funcionalidade no mapa) e o registro pelas ferramentas
//! reais, como o agente faria, comparado com o gabarito. O caso de uso gerado a
//! partir da análise validada tem fluxo principal, ao menos um fluxo
//! alternativo e todas as etapas com regra citada; o épico gera histórias que,
//! juntas, cobrem as regras confirmadas da análise, ou o app avisa a que ficou
//! de fora.

use std::path::{Path, PathBuf};

use cba_avaliacao::entregaveis::{avaliar_entregavel, campos_registrados, preparar_entregavel};
use cba_avaliacao::gabarito::Gabarito;
use cba_avaliacao::gabarito_padrao;
use cba_core::ambiente::Ambiente;
use cba_core::config::Config;
use cba_core::dados::DiretorioDados;
use cba_tools::{ErroChamada, ErroFerramenta, registro_padrao};
use serde_json::{Value, json};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/faturamento-exemplo")
}

struct Cenario {
    _dir: tempfile::TempDir,
    dados: DiretorioDados,
    amb: Ambiente,
}

fn cenario() -> Cenario {
    let dir = tempfile::tempdir().unwrap();
    let dados = DiretorioDados::em(dir.path().join("dados"));
    let projeto = cba_index::Projeto::novo("Faturamento Exemplo", [fixture()]).unwrap();
    cba_index::Cadastro::em(dados.projetos())
        .cadastrar(&projeto)
        .unwrap();
    cba_kb::Base::criar(dir.path().join("base")).unwrap();
    Config {
        base: Some(dir.path().join("base")),
        ..Default::default()
    }
    .gravar(&dados)
    .unwrap();
    let amb = Ambiente::do_diretorio(&fixture(), &dados).unwrap();
    Cenario {
        _dir: dir,
        dados,
        amb,
    }
}

impl Cenario {
    fn chamar(&self, nome: &str, args: Value) -> Result<(String, Value), ErroFerramenta> {
        match registro_padrao().chamar(nome, Ok(&self.amb), Some(args)) {
            Ok(saida) => Ok((saida.texto, saida.dados.unwrap_or_default())),
            Err(ErroChamada::Ferramenta(e)) => Err(e),
            Err(e) => panic!("{e}"),
        }
    }
}

fn gabarito() -> Gabarito {
    Gabarito::carregar(&gabarito_padrao()).unwrap()
}

#[test]
fn caso_de_uso_da_analise_validada_tem_fluxo_alternativo_e_regra_em_cada_etapa() {
    let g = gabarito();
    let esperado = &g.entregavel("ENT-FAT-001").unwrap().esperado;
    let mut c = cenario();
    let p = preparar_entregavel(&mut c.amb, &c.dados, &g, esperado).unwrap();
    assert!(p.prompt.contains("iniciar_entregavel"), "{}", p.prompt);

    let alvo = json!({ "demanda": p.demanda.to_string(), "entregavel": p.numero });
    let (roteiro, dados) = c.chamar("iniciar_entregavel", alvo.clone()).unwrap();
    assert_eq!(dados["finalidade"], "versao_inicial");
    for trecho in [
        "## Como redigir o caso de uso",
        "`fluxo_principal` (Fluxo principal)",
        "RN-FAT-007",
        "RN-FAT-010",
        "## Regras confirmadas da funcionalidade",
    ] {
        assert!(roteiro.contains(trecho), "{trecho}\n{roteiro}");
    }

    let campos = json!({
        "titulo": "Parcelar fatura",
        "objetivo": "Dividir a fatura em aberto em parcelas mensais.",
        "atores": ["Atendente do faturamento"],
        "gatilho": "O cliente pede para parcelar uma fatura.",
        "pre_condicoes": ["A fatura está ABERTA (RN-FAT-010)."],
        "fluxo_principal": [
            "O atendente escolhe a fatura ABERTA (RN-FAT-010).",
            "O atendente informa de 2 a 6 parcelas (RN-FAT-008).",
            "O sistema confere que o cliente é pessoa jurídica ou órgão público (RN-FAT-007).",
            "O sistema confere a parcela mínima de R$ 50,00 (RN-FAT-009).",
            "O sistema gera parcelas iguais, com vencimento a cada 30 dias (RN-FAT-011)."
        ],
        "fluxos_alternativos": [
            "Na etapa 4, se o cliente é órgão público, não há valor mínimo de parcela (RN-FAT-009)."
        ],
        "excecoes": [
            "Na etapa 3, se o cliente é pessoa física, o sistema recusa o parcelamento (RN-FAT-007).",
            "Na etapa 2, quantidade fora de 2 a MAX_PARCELAS é recusada (RN-FAT-008).",
            "Na etapa 1, fatura que não está ABERTA é recusada (RN-FAT-010).",
            "Na etapa 4, parcela abaixo do mínimo é recusada (RN-FAT-009)."
        ],
        "pos_condicoes": ["As parcelas ficam gravadas, a primeira com a diferença dos centavos (RN-FAT-011)."],
        "regras": ["RN-FAT-007", "RN-FAT-008", "RN-FAT-009", "RN-FAT-010", "RN-FAT-011"]
    });
    let mut registro = alvo.clone();
    // Uma etapa sem regra é recusada pelo app.
    let mut sem_regra = campos.clone();
    sem_regra["fluxo_principal"][1] = json!("O atendente informa as parcelas.");
    registro["campos"] = sem_regra;
    let erro = c
        .chamar("registrar_entregavel", registro.clone())
        .unwrap_err();
    assert!(
        erro.to_string().contains("etapa 2 sem regra citada"),
        "{erro}"
    );

    registro["campos"] = campos;
    let (texto, d) = c.chamar("registrar_entregavel", registro).unwrap();
    assert!(texto.contains("Proposta de versão"), "{texto}");
    assert_eq!(d["regras_sem_item"], json!([]), "{texto}");

    let registrados = campos_registrados(&c.amb, &p).unwrap();
    let a = avaliar_entregavel(registrados.as_ref(), esperado);
    assert!(a.falhas().is_empty(), "{:?}", a.falhas());
    assert!(a.etapas >= esperado.minimo_de_etapas);
    assert!(a.fluxos_alternativos >= 1);
    assert!(a.etapas_sem_regra.is_empty());
    assert_eq!(a.regras_citadas(), esperado.regras.len());
    assert_eq!(
        a.esperados_atendidos(),
        a.esperados.len(),
        "{:#?}",
        a.esperados
    );
}

#[test]
fn epico_cobre_as_regras_da_analise_ou_avisa_a_que_ficou_de_fora() {
    let g = gabarito();
    let esperado = &g.entregavel("ENT-FAT-002").unwrap().esperado;
    let mut c = cenario();
    let p = preparar_entregavel(&mut c.amb, &c.dados, &g, esperado).unwrap();
    let alvo = json!({ "demanda": p.demanda.to_string(), "entregavel": p.numero });
    let (roteiro, _) = c.chamar("iniciar_entregavel", alvo.clone()).unwrap();
    assert!(roteiro.contains("## Como redigir o épico"), "{roteiro}");
    for r in &esperado.regras {
        assert!(roteiro.contains(r.as_str()), "{r}\n{roteiro}");
    }

    let historia = |titulo: &str, acao: &str, regras: &[&str], depende: &[&str]| {
        json!({
            "titulo": titulo, "papel": "atendente do faturamento", "acao": acao,
            "beneficio": "atender o cliente pessoa física",
            "criterios": ["Dado um cliente PF com fatura ABERTA de R$ 300,00, quando pede 3 parcelas, então são geradas 3 parcelas de R$ 100,00"],
            "regras": regras,
            "depende_de": depende,
        })
    };
    let epico = |historias: Vec<Value>| {
        json!({
            "titulo": "Parcelamento para pessoa física",
            "objetivo": "Permitir que pessoa física parcele em até 3 vezes, com parcela mínima de R$ 100,00.",
            "historias": historias,
            "riscos": ["MAX_PARCELAS e VALOR_MIN_PARCELA são parâmetros únicos: mudá-los afeta os outros tipos de cliente."],
        })
    };
    let mut registro = alvo.clone();
    registro["campos"] = epico(vec![historia(
        "Liberar o parcelamento para pessoa física",
        "parcelar a fatura de um cliente PF",
        &["RN-FAT-007"],
        &[],
    )]);
    let (texto, d) = c.chamar("registrar_entregavel", registro.clone()).unwrap();
    assert!(
        texto.contains("RN-FAT-008 e RN-FAT-009 sem história no épico"),
        "{texto}"
    );
    assert_eq!(d["regras_sem_item"], json!(["RN-FAT-008", "RN-FAT-009"]));

    registro["campos"] = epico(vec![
        historia(
            "Liberar o parcelamento para pessoa física",
            "parcelar a fatura de um cliente PF",
            &["RN-FAT-007"],
            &[],
        ),
        historia(
            "Limite de 3 parcelas e mínimo de R$ 100,00 para pessoa física",
            "usar limites próprios para PF",
            &["RN-FAT-008", "RN-FAT-009"],
            &["H1"],
        ),
    ]);
    let (texto, _) = c.chamar("registrar_entregavel", registro).unwrap();
    assert!(
        texto.contains("Todas as regras confirmadas têm item"),
        "{texto}"
    );

    let registrados = campos_registrados(&c.amb, &p).unwrap();
    let a = avaliar_entregavel(registrados.as_ref(), esperado);
    assert!(a.falhas().is_empty(), "{:?}", a.falhas());
    assert_eq!(a.historias, 2);
    assert_eq!(a.regras_citadas(), 3);
    assert_eq!(
        a.esperados_atendidos(),
        a.esperados.len(),
        "{:#?}",
        a.esperados
    );
}
