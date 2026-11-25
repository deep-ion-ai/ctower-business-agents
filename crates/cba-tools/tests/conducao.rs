//! Condução pelo workflow (#371) com o sistema fictício "Faturamento Exemplo": o
//! roteiro curto do `iniciar_analise`, o caminho de volta à condução simples com o
//! aviso, a contestação da verificação independente e a pendência no
//! `concluir_analise`.

use std::path::{Path, PathBuf};

use cba_core::ambiente::Ambiente;
use cba_core::analise::{Analise, EstadoAfirmacao, Fluxo};
use cba_core::analises::NovaAnalise;
use cba_core::conducao::Conducao;
use cba_core::config::Config;
use cba_core::dados::DiretorioDados;
use cba_kb::Id;
use cba_tools::{ErroChamada, ErroFerramenta, Saida, registro_padrao};
use serde_json::{Value, json};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/faturamento-exemplo")
}

struct Cenario {
    _dir: tempfile::TempDir,
    amb: Ambiente,
}

fn cenario() -> Cenario {
    let dir = tempfile::tempdir().unwrap();
    let raiz = dir.path();
    let dados = DiretorioDados::em(raiz.join("dados"));
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
    Cenario { _dir: dir, amb }
}

impl Cenario {
    fn criar(&self, fluxo: Fluxo, titulo: &str, entrada: &str) -> Analise {
        self.amb
            .analises
            .criar(NovaAnalise {
                fluxo,
                titulo: titulo.into(),
                entrada: entrada.into(),
                analista: None,
                base: None,
            })
            .unwrap()
    }

    fn saida(&self, nome: &str, args: Value) -> Result<Saida, ErroFerramenta> {
        match registro_padrao().chamar(nome, Ok(&self.amb), Some(args)) {
            Ok(saida) => Ok(saida),
            Err(ErroChamada::Ferramenta(e)) => Err(e),
            Err(e) => panic!("{e}"),
        }
    }

    fn chamar(&self, nome: &str, args: Value) -> Value {
        self.saida(nome, args)
            .unwrap_or_else(|e| panic!("{nome}: {e}"))
            .dados
            .unwrap()
    }

    fn analise(&self, id: &str) -> Analise {
        self.amb
            .analises
            .carregar(&Id::analisar(id).unwrap())
            .unwrap()
    }
}

fn multa(analise: &str, secao: &str) -> Value {
    json!({
        "analise": analise,
        "secao": secao,
        "texto": "Passada a carência, a multa é de 2% sobre o valor original.",
        "confianca": "confirmado",
        "evidencias": [{
            "arquivo": "db/03_pkg_cobranca.sql",
            "linha_inicial": 71,
            "linha_final": 72,
            "trecho": "FN_PARAMETRO('PERC_MULTA_ATRASO') / 100, 2)"
        }]
    })
}

#[test]
fn iniciar_com_workflow_devolve_o_roteiro_curto_tambem_na_retomada() {
    let c = cenario();
    let i = c.criar(Fluxo::Impacto, "Parcelamento", "Parcelar em 3 vezes");
    let id = i.id.to_string();
    let saida = c
        .saida(
            "iniciar_analise",
            json!({ "analise": id, "conducao": "workflow" }),
        )
        .unwrap();
    let t = &saida.texto;
    assert!(t.contains("conduzida pelo workflow cba-impacto"), "{t}");
    assert!(t.contains("conducao \"simples\""), "{t}");
    assert!(t.contains("## Resposta no chat"), "{t}");
    assert!(t.contains("a demanda como entendida"), "{t}");
    assert!(!t.contains("**Consulte a base primeiro.**"), "{t}");
    let dados = saida.dados.unwrap();
    assert_eq!(dados["conducao"], "workflow");
    assert_eq!(dados["workflow"], "cba-impacto");
    assert_eq!(c.analise(&id).conducao, Conducao::Workflow);
    let retomada = c
        .saida("retomar_analise", json!({ "analise": id }))
        .unwrap();
    assert!(
        retomada.texto.contains("conduzida pelo workflow"),
        "{}",
        retomada.texto
    );
}

#[test]
fn contestacao_fica_a_revalidar_e_vira_pendencia_sem_apagar() {
    let c = cenario();
    let i = c.criar(Fluxo::Impacto, "Parcelamento", "Parcelar em 3 vezes");
    let id = i.id.to_string();
    c.chamar(
        "iniciar_analise",
        json!({ "analise": id, "conducao": "workflow" }),
    );
    let mut args = multa(&id, "demanda_como_entendida");
    c.chamar("afirmar", args.clone());
    args["secao"] = json!("como_e_hoje");
    c.chamar("afirmar", args.clone());
    args["secao"] = json!("abrangencia");
    args["texto"] = json!("Pequena: só o parâmetro muda.");
    args["confianca"] = json!("inferido");
    c.chamar("afirmar", args);
    let r = c.chamar(
        "contestar_afirmacao",
        json!({ "analise": id, "numero": 1, "motivo": "O trecho é do cálculo de juros." }),
    );
    assert_eq!(r["a_revalidar"], true);
    // De novo, como numa retomada: substitui, sem duplicar.
    c.chamar(
        "contestar_afirmacao",
        json!({ "analise": id, "numero": 1, "motivo": "O trecho é do cálculo de juros." }),
    );
    let a = c.analise(&id);
    assert_eq!(a.afirmacoes[0].estado, EstadoAfirmacao::ARevalidar);
    assert_eq!(a.afirmacoes.len(), 3);
    let retomada = c
        .saida("retomar_analise", json!({ "analise": id }))
        .unwrap();
    assert!(
        retomada
            .texto
            .contains("contestada pela verificação independente"),
        "{}",
        retomada.texto
    );
    let fim = c.chamar("concluir_analise", json!({ "analise": id }));
    let pendencias = fim["pendencias"].to_string();
    assert!(
        pendencias.contains("afirmação 1: contestada pela verificação independente"),
        "{pendencias}"
    );
    match c
        .saida(
            "contestar_afirmacao",
            json!({ "analise": id, "numero": 9, "motivo": "x" }),
        )
        .unwrap_err()
    {
        ErroFerramenta::Recusa(m) => assert!(m.contains("número 9"), "{m}"),
        outro => panic!("{outro:?}"),
    }
    // Achado só no fluxo de revisão.
    assert!(
        c.saida(
            "contestar_achado",
            json!({ "analise": id, "numero": 1, "motivo": "x" })
        )
        .is_err()
    );
}

#[test]
fn sem_a_ferramenta_workflow_volta_a_conducao_simples_com_aviso() {
    let c = cenario();
    let i = c.criar(Fluxo::Impacto, "Parcelamento", "Parcelar em 3 vezes");
    let id = i.id.to_string();
    c.chamar(
        "iniciar_analise",
        json!({ "analise": id, "conducao": "workflow" }),
    );
    let saida = c
        .saida(
            "iniciar_analise",
            json!({
                "analise": id,
                "conducao": "simples",
                "motivo": "a ferramenta Workflow não existe nesta sessão"
            }),
        )
        .unwrap();
    assert!(
        saida.texto.contains("**Consulte a base primeiro.**"),
        "{}",
        saida.texto
    );
    let a = c.analise(&id);
    assert_eq!(a.conducao, Conducao::Simples);
    assert_eq!(
        a.aviso_conducao.as_deref(),
        Some("a ferramenta Workflow não existe nesta sessão")
    );

    // Um fluxo sem workflow segue o roteiro completo, com aviso.
    let p = c.criar(Fluxo::Perguntar, "Multa", "Como é a multa?");
    let saida = c
        .saida(
            "iniciar_analise",
            json!({ "analise": p.id, "conducao": "workflow" }),
        )
        .unwrap();
    assert!(saida.texto.contains("não tem workflow"), "{}", saida.texto);
    assert_eq!(c.analise(&p.id.to_string()).conducao, Conducao::Simples);
}

/// A mesma história do gabarito (`revisoes`, REV-FAT-001).
const HISTORIA: &str = "Cancelamento de fatura pelo atendente

Como atendente da central, quero cancelar uma fatura informando o motivo, para corrigir cobranças lançadas por engano.

Critérios de aceite:
- O atendente pode cancelar fatura aberta, inclusive a que já tem pagamento parcial.
- O motivo do cancelamento é obrigatório.
- O cancelamento deve ser rápido e a mensagem de confirmação, adequada.
- Vale para faturas avulsas e/ou parceladas, etc.";

/// A revisão de requisito pelo workflow `cba-revisar` (#373), de ponta a ponta, com
/// as chamadas que os agentes fazem: o roteiro curto da revisão, o achado da fase
/// Avaliar, a contestação da fase Verificar, o INVEST e o veredito da Consolidar, e a
/// pendência no `concluir_analise`, sem apagar o achado.
#[test]
fn revisao_pelo_workflow_contesta_achado_e_conclui_com_pendencia() {
    let c = cenario();
    let r = c.criar(Fluxo::Revisar, "Cancelamento", HISTORIA);
    let id = r.id.to_string();
    let saida = c
        .saida(
            "iniciar_analise",
            json!({ "analise": id, "conducao": "workflow" }),
        )
        .unwrap();
    let t = &saida.texto;
    assert!(t.contains("conduzida pelo workflow cba-revisar"), "{t}");
    assert!(t.contains("`contestar_achado`"), "{t}");
    assert!(t.contains("`registrar_veredito`"), "{t}");
    assert!(t.contains("Se faltar o veredito"), "{t}");
    assert!(!t.contains("seção obrigatória"), "{t}");
    assert!(
        t.contains("quantos achados a verificação independente"),
        "{t}"
    );
    // A resposta no chat é a do roteiro de revisão.
    assert!(t.contains("o veredito, em uma frase"), "{t}");
    assert_eq!(saida.dados.unwrap()["workflow"], "cba-revisar");
    assert_eq!(c.analise(&id).conducao, Conducao::Workflow);

    // Avaliar: dois achados.
    let correcao = c.chamar(
        "registrar_achado",
        json!({
            "analise": id,
            "caracteristica": "correcao",
            "severidade": "alta",
            "trecho": "inclusive a que já tem pagamento parcial",
            "problema": "O sistema só cancela fatura sem nenhum valor pago.",
            "reescrita": "O atendente pode cancelar fatura aberta e sem nenhum valor pago.",
            "evidencias": [{
                "arquivo": "db/03_pkg_cobranca.sql",
                "linha_inicial": 115,
                "linha_final": 117,
                "trecho": "IF V_VL_PAGO > 0 THEN"
            }]
        }),
    );
    assert_eq!(correcao["achado"], 1);
    c.chamar(
        "registrar_achado",
        json!({
            "analise": id,
            "caracteristica": "ambiguidade",
            "severidade": "media",
            "trecho": "rápido",
            "problema": "Rápido não é verificável.",
            "reescrita": "O cancelamento termina em até 2 segundos."
        }),
    );
    // Verificar: o cético contesta o segundo, também numa retomada (substitui).
    for _ in 0..2 {
        c.chamar(
            "contestar_achado",
            json!({ "analise": id, "numero": 2, "motivo": "O termo está em outro critério." }),
        );
    }
    let a = c.analise(&id);
    let achados = &a.parecer.as_ref().unwrap().achados;
    assert_eq!(achados.len(), 2);
    assert!(!achados[0].a_revalidar());
    assert!(achados[1].a_revalidar());

    // Consolidar: INVEST e veredito.
    let criterios: Vec<Value> = [
        "independente",
        "negociavel",
        "valiosa",
        "estimavel",
        "pequena",
        "testavel",
    ]
    .iter()
    .map(|c| json!({ "criterio": c, "resultado": "atende", "justificativa": "ok" }))
    .collect();
    c.chamar(
        "registrar_invest",
        json!({ "analise": id, "criterios": criterios }),
    );
    c.chamar(
        "registrar_veredito",
        json!({
            "analise": id,
            "veredito": "precisa_de_retrabalho",
            "justificativa": "Contradiz a regra do pagamento parcial."
        }),
    );

    let fim = c.chamar("concluir_analise", json!({ "analise": id }));
    let pendencias = fim["pendencias"].to_string();
    assert!(
        pendencias.contains("achado 2: contestado pela verificação independente"),
        "{pendencias}"
    );
    assert_eq!(
        c.analise(&id).parecer.as_ref().unwrap().achados.len(),
        2,
        "nada é apagado"
    );
}
