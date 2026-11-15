//! Ferramentas de registro (`afirmar`, `perguntar`, `registrar_sugestao`,
//! `propor_termo`) com o sistema fictício "Faturamento Exemplo": cada registro
//! válido entra no estado e no documento da análise; cada recusa volta com o motivo
//! e não grava nada.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use cba_core::ambiente::Ambiente;
use cba_core::analise::{Analise, Confianca, EstadoAnalise, Fluxo};
use cba_core::analises::NovaAnalise;
use cba_core::config::Config;
use cba_core::dados::DiretorioDados;
use cba_core::eventos::Mudanca;
use cba_kb::frontmatter::{FrontmatterAnalise, FrontmatterSugestao, StatusAnalise};
use cba_kb::{Documento, Id};
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
    Cenario {
        _dir: dir,
        dados,
        amb,
    }
}

impl Cenario {
    fn criar(&self, fluxo: Fluxo) -> Analise {
        self.amb
            .analises
            .criar(NovaAnalise {
                fluxo,
                titulo: "Multa por atraso".into(),
                entrada: "Como é calculada a multa por atraso de pagamento?".into(),
                analista: Some("Ana Exemplo".into()),
                base: None,
            })
            .unwrap()
    }

    fn chamar(&self, nome: &str, args: Value) -> Result<Value, ErroFerramenta> {
        match registro_padrao().chamar(nome, Ok(&self.amb), Some(args)) {
            Ok(saida) => Ok(saida.dados.expect("resposta estruturada")),
            Err(ErroChamada::Ferramenta(e)) => Err(e),
            Err(e) => panic!("{e}"),
        }
    }

    fn recusa(&self, nome: &str, args: Value) -> String {
        match self.chamar(nome, args) {
            Err(ErroFerramenta::Recusa(m)) => m,
            outro => panic!("esperava recusa, veio {outro:?}"),
        }
    }

    fn analise(&self, id: &Id) -> Analise {
        self.amb.analises.carregar(id).unwrap()
    }

    fn documento(&self, analise: &Analise) -> Documento<FrontmatterAnalise> {
        self.amb.kb.ler(&analise.documento).unwrap()
    }

    fn eventos(&self) -> Vec<Value> {
        let arquivo = self.dados.eventos().join("faturamento-exemplo.jsonl");
        std::fs::read_to_string(arquivo)
            .unwrap_or_default()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }
}

fn multa(analise: &Analise) -> Value {
    json!({
        "analise": analise.id,
        "secao": "regras_encontradas",
        "texto": "Passada a carência, a multa é de PERC_MULTA_ATRASO por cento (hoje 2%) sobre o valor original.",
        "confianca": "confirmado",
        "evidencias": [
            {
                "arquivo": "db/03_pkg_cobranca.sql",
                "linha_inicial": 71,
                "linha_final": 72,
                "trecho": "FN_PARAMETRO('PERC_MULTA_ATRASO') / 100, 2)"
            },
            {
                "arquivo": "db/02_parametros.sql",
                "linha_inicial": 6,
                "linha_final": 7,
                "trecho": "'PERC_MULTA_ATRASO', 2.0000"
            }
        ]
    })
}

#[test]
fn afirmar_grava_estado_documento_e_evento_em_menos_de_um_segundo() {
    let c = cenario();
    let a = c.criar(Fluxo::Perguntar);

    let inicio = Instant::now();
    let r = c.chamar("afirmar", multa(&a)).unwrap();
    let mut duracao = inicio.elapsed();
    // Do registro ao documento gravado e ao evento: bem abaixo de 1 s (cerca de
    // 15 ms em debug). Com a máquina ocupada (CI), vale a melhor de três chamadas.
    for _ in 0..2 {
        let a = c.criar(Fluxo::Perguntar);
        let inicio = Instant::now();
        c.chamar("afirmar", multa(&a)).unwrap();
        duracao = duracao.min(inicio.elapsed());
    }
    assert!(duracao < Duration::from_secs(1), "{duracao:?}");

    assert_eq!(r["analise"], "A-0001");
    assert_eq!(r["afirmacao"], 1);
    assert_eq!(r["confianca"], "confirmado");
    assert_eq!(r["pergunta"], Value::Null);
    assert_eq!(r["documento"], "faturamento-exemplo/analises/A-0001.md");
    assert_eq!(r["evidencias"][0]["linhas"], "71-72");
    assert_eq!(r["evidencias"][0]["hash"].as_str().unwrap().len(), 64);

    // Estado.
    let estado = c.analise(&a.id);
    assert_eq!(estado.estado, EstadoAnalise::EmAndamento);
    let afirmacao = &estado.afirmacoes[0];
    assert_eq!(afirmacao.confianca, Confianca::Confirmado);
    assert_eq!(afirmacao.evidencias.len(), 2);
    assert_eq!(
        afirmacao.evidencias[1].arquivo.as_str(),
        "db/02_parametros.sql"
    );

    // Documento, montado pelo template da pergunta ao sistema.
    let doc = c.documento(&a);
    assert_eq!(doc.frontmatter.status, StatusAnalise::Rascunho);
    assert_eq!(doc.frontmatter.analista.as_deref(), Some("Ana Exemplo"));
    let corpo = &doc.corpo;
    assert!(
        corpo.contains("## Pergunta\n\nComo é calculada a multa"),
        "{corpo}"
    );
    assert!(
        corpo.contains("- **[confirmado]** Passada a carência, a multa é de PERC_MULTA_ATRASO"),
        "{corpo}"
    );
    assert!(
        corpo.contains("[^1]: `db/03_pkg_cobranca.sql:71-72` · hash `"),
        "{corpo}"
    );
    assert!(
        corpo.contains("[^2]: `db/02_parametros.sql:6-7`"),
        "{corpo}"
    );

    // Evento para o app, depois do documento gravado.
    let eventos = c.eventos();
    let evento = eventos
        .iter()
        .find(|e| e["analise"] == "A-0001" && e["mudanca"] == "afirmacao")
        .unwrap();
    assert_eq!(evento["versao"], estado.versao);
}

#[test]
fn afirmar_recusa_e_nao_grava_nada() {
    let c = cenario();
    let a = c.criar(Fluxo::Perguntar);
    let versao = c.analise(&a.id).versao;

    // Trecho que não confere.
    let mut args = multa(&a);
    args["evidencias"][1]["trecho"] = json!("'PERC_MULTA_ATRASO', 3.0000");
    let m = c.recusa("afirmar", args);
    assert!(
        m.starts_with("1 de 2 evidências recusadas; nada foi registrado"),
        "{m}"
    );
    assert!(
        m.contains("evidências[1] (`db/02_parametros.sql`, linhas 6-7"),
        "{m}"
    );
    assert!(m.contains("não aparece nessas linhas"), "{m}");

    // Arquivo excluído.
    let mut args = multa(&a);
    args["evidencias"][0] = json!({
        "arquivo": ".env", "linha_inicial": 1, "linha_final": 1, "trecho": "Arquivo FALSO"
    });
    let m = c.recusa("afirmar", args);
    assert!(m.contains("excluído da análise"), "{m}");

    // Fora do projeto.
    let mut args = multa(&a);
    args["evidencias"][0]["arquivo"] = json!("../gabaritos/faturamento-exemplo.yaml");
    assert!(
        c.recusa("afirmar", args)
            .contains("relativo à raiz do projeto")
    );

    // Confirmado sem evidência.
    let mut args = multa(&a);
    args["evidencias"] = json!([]);
    assert!(
        c.recusa("afirmar", args)
            .contains("precisa de ao menos uma evidência")
    );

    // Seção que o documento não tem para este fluxo.
    let mut args = multa(&a);
    args["secao"] = json!("como_e_hoje");
    let m = c.recusa("afirmar", args);
    assert!(m.contains("resposta, regras_encontradas, exemplo"), "{m}");
    let mut args = multa(&a);
    args["secao"] = json!("perguntas");
    assert!(c.recusa("afirmar", args).contains("use `perguntar`"));

    // Análise inexistente e identificador malformado.
    let mut args = multa(&a);
    args["analise"] = json!("A-0099");
    assert!(
        c.recusa("afirmar", args)
            .contains("não existe neste projeto")
    );
    let mut args = multa(&a);
    args["analise"] = json!("0001");
    assert!(matches!(
        c.chamar("afirmar", args),
        Err(ErroFerramenta::Argumentos(m)) if m.contains("A-NNNN")
    ));

    // Nada foi gravado.
    let estado = c.analise(&a.id);
    assert_eq!(estado.versao, versao);
    assert!(estado.afirmacoes.is_empty());
    assert_eq!(estado.estado, EstadoAnalise::Criada);
}

#[test]
fn a_confirmar_cria_pergunta_ligada_a_afirmacao() {
    let c = cenario();
    let a = c.criar(Fluxo::Impacto);
    let r = c
        .chamar(
            "afirmar",
            json!({
                "analise": a.id,
                "secao": "riscos",
                "texto": "Relatórios externos podem agrupar faturas pela data de vencimento.",
                "confianca": "a_confirmar"
            }),
        )
        .unwrap();
    assert_eq!(r["afirmacao"], 1);
    assert_eq!(r["pergunta"], 1);
    let r = c
        .chamar(
            "afirmar",
            json!({
                "analise": a.id,
                "secao": "como_e_hoje",
                "texto": "O parcelamento existe só para pessoa física.",
                "confianca": "a_confirmar",
                "pergunta": "O parcelamento para PJ já foi pedido antes?"
            }),
        )
        .unwrap();
    assert_eq!(r["pergunta"], 2);

    let estado = c.analise(&a.id);
    assert_eq!(estado.perguntas[0].afirmacao, Some(1));
    assert!(
        estado.perguntas[0]
            .texto
            .starts_with("Confirmar com o solicitante: Relatórios")
    );
    assert_eq!(estado.perguntas[1].afirmacao, Some(2));
    let corpo = c.documento(&a).corpo;
    assert!(
        corpo.contains("## Demanda como veio\n\nComo é calculada"),
        "{corpo}"
    );
    assert!(
        corpo.contains("- **[a confirmar]** Relatórios externos"),
        "{corpo}"
    );
    assert!(
        corpo.contains("2. O parcelamento para PJ já foi pedido antes?"),
        "{corpo}"
    );
}

#[test]
fn perguntar_registra_e_recusa_afirmacao_inexistente() {
    let c = cenario();
    let a = c.criar(Fluxo::Perguntar);
    let r = c
        .chamar(
            "perguntar",
            json!({ "analise": a.id, "texto": "A multa vale também para clientes PJ?" }),
        )
        .unwrap();
    assert_eq!(r["pergunta"], 1);
    assert_eq!(r["documento"], "faturamento-exemplo/analises/A-0001.md");
    assert!(
        c.documento(&a)
            .corpo
            .contains("## Perguntas em aberto\n\n1. A multa vale também para clientes PJ?")
    );
    let m = c.recusa(
        "perguntar",
        json!({ "analise": a.id, "texto": "E esta?", "afirmacao": 7 }),
    );
    assert!(m.contains("não existe afirmação número 7"), "{m}");
    assert!(c.eventos().iter().any(|e| e["mudanca"] == "pergunta"));
}

#[test]
fn registrar_sugestao_cria_o_documento_da_sugestao() {
    let c = cenario();
    let a = c.criar(Fluxo::Perguntar);
    let r = c
        .chamar(
            "registrar_sugestao",
            json!({
                "analise": a.id,
                "arquivo": "db/02_parametros.sql",
                "titulo": "Multa de 3%",
                "trecho_atual": "VALUES ('PERC_MULTA_ATRASO', 2.0000, 'Percentual de multa por atraso sobre o valor original');",
                "trecho_proposto": "VALUES ('PERC_MULTA_ATRASO', 3.0000, 'Percentual de multa por atraso sobre o valor original');",
                "motivo": "A diretoria aprovou multa de 3%.",
                "impacto": "Faturas vencidas depois da mudança pagam multa maior.",
                "exemplo": "Fatura de R$ 1.000,00 com 10 dias de atraso: multa de R$ 20,00 passa a R$ 30,00."
            }),
        )
        .unwrap();
    assert_eq!(r["sugestao"], 1);
    assert_eq!(r["id"], "S-0001");
    assert_eq!(
        r["documento_da_sugestao"],
        "faturamento-exemplo/sugestoes/S-0001.md"
    );
    assert_eq!(r["trecho_atual"]["linhas"], "7-7");
    assert!(r.get("aviso_da_sugestao").is_none(), "{r}");

    let estado = c.analise(&a.id);
    let s = &estado.sugestoes[0];
    assert_eq!(s.id.as_ref().unwrap().to_string(), "S-0001");
    assert_eq!(s.evidencia.as_ref().unwrap().linhas.to_string(), "7-7");

    let doc: Documento<FrontmatterSugestao> = c
        .amb
        .kb
        .ler(&cba_kb::CaminhoRelativo::novo("sugestoes/S-0001.md").unwrap())
        .unwrap();
    assert_eq!(
        doc.frontmatter.analise.as_ref().unwrap().to_string(),
        "A-0001"
    );
    assert!(
        doc.corpo.contains("# S-0001 · Multa de 3%"),
        "{}",
        doc.corpo
    );
    assert!(
        doc.corpo
            .contains("```sql\nVALUES ('PERC_MULTA_ATRASO', 3.0000"),
        "{}",
        doc.corpo
    );
    assert!(
        doc.corpo.contains("[A-0001](../analises/A-0001.md)"),
        "{}",
        doc.corpo
    );
    assert!(
        doc.corpo.contains("`db/02_parametros.sql:7`"),
        "{}",
        doc.corpo
    );
    assert!(doc.corpo.contains("R$ 30,00"), "{}", doc.corpo);

    // O documento da análise cita a sugestão com link relativo.
    let corpo = c.documento(&a).corpo;
    assert!(
        corpo.contains("[S-0001 · Multa de 3%](../sugestoes/S-0001.md) (`db/02_parametros.sql`)"),
        "{corpo}"
    );
}

#[test]
fn registrar_sugestao_confere_o_trecho_atual() {
    let c = cenario();
    let a = c.criar(Fluxo::Perguntar);
    let base = json!({
        "analise": a.id,
        "arquivo": "db/03_pkg_cobranca.sql",
        "trecho_atual": "IF V_DIAS <= FN_PARAMETRO('DIAS_CARENCIA') THEN",
        "trecho_proposto": "IF V_DIAS < FN_PARAMETRO('DIAS_CARENCIA') THEN",
        "motivo": "Carência deveria terminar um dia antes.",
        "impacto": "Encargos começam no terceiro dia."
    });

    // O trecho aparece duas vezes: pede o intervalo.
    let m = c.recusa("registrar_sugestao", base.clone());
    assert!(
        m.contains("aparece em mais de um lugar (linhas 67-67, 90-90)"),
        "{m}"
    );

    // Linhas só de um lado.
    let mut args = base.clone();
    args["linha_inicial"] = json!(90);
    assert!(matches!(
        c.chamar("registrar_sugestao", args),
        Err(ErroFerramenta::Argumentos(_))
    ));

    // Trecho que não está no arquivo e arquivo excluído.
    let mut args = base.clone();
    args["trecho_atual"] = json!("IF V_DIAS > 99 THEN");
    assert!(
        c.recusa("registrar_sugestao", args)
            .contains("não aparece no arquivo")
    );
    let mut args = base.clone();
    args["arquivo"] = json!(".env");
    assert!(
        c.recusa("registrar_sugestao", args)
            .contains("excluído da análise")
    );

    // Nenhuma recusa reservou documento de sugestão.
    assert!(
        !c.amb
            .kb
            .existe(&cba_kb::CaminhoRelativo::novo("sugestoes/S-0001.md").unwrap())
    );

    // Com o intervalo, vale; sem título, ganha um padrão.
    let mut args = base.clone();
    args["linha_inicial"] = json!(90);
    args["linha_final"] = json!(92);
    let r = c.chamar("registrar_sugestao", args).unwrap();
    assert_eq!(r["id"], "S-0001");
    assert_eq!(r["trecho_atual"]["linhas"], "90-92");
    let estado = c.analise(&a.id);
    assert_eq!(
        estado.sugestoes[0].titulo,
        "Alteração em db/03_pkg_cobranca.sql"
    );

    // Trecho atual vazio: só acrescenta código.
    let mut args = base;
    args["trecho_atual"] = json!("");
    let r = c.chamar("registrar_sugestao", args).unwrap();
    assert_eq!(r["id"], "S-0002");
    assert_eq!(r["trecho_atual"], Value::Null);
}

#[test]
fn analise_aprovada_nao_aceita_registro() {
    let c = cenario();
    let a = c.criar(Fluxo::Perguntar);
    c.chamar("afirmar", multa(&a)).unwrap();
    c.amb
        .analises
        .alterar(&a.id, Mudanca::Estado, |an, _| {
            an.concluir()?;
            an.aprovar()
        })
        .unwrap();
    assert!(c.recusa("afirmar", multa(&a)).contains("já foi aprovada"));
    assert!(
        c.recusa("perguntar", json!({ "analise": a.id, "texto": "Mais?" }))
            .contains("já foi aprovada")
    );
    let m = c.recusa(
        "registrar_sugestao",
        json!({
            "analise": a.id,
            "arquivo": "db/02_parametros.sql",
            "trecho_atual": "",
            "trecho_proposto": "x",
            "motivo": "m",
            "impacto": "i"
        }),
    );
    assert!(m.contains("já foi aprovada"), "{m}");
    assert!(
        !c.amb
            .kb
            .existe(&cba_kb::CaminhoRelativo::novo("sugestoes/S-0001.md").unwrap())
    );
    assert!(
        c.recusa(
            "propor_termo",
            json!({ "analise": a.id, "termo": "Multa", "definicao": "d" })
        )
        .contains("já foi aprovada")
    );
}

#[test]
fn propor_termo_registra_com_evidencias_e_recusa_repetido() {
    let c = cenario();
    let a = c.criar(Fluxo::Perguntar);
    let args = json!({
        "analise": a.id,
        "termo": "Multa por atraso",
        "definicao": "Valor único cobrado sobre a fatura paga depois da carência.",
        "sinonimos": ["multa moratória"],
        "onde_vive_no_codigo": ["PKG_COBRANCA.FN_CALCULAR_MULTA", "parâmetro PERC_MULTA_ATRASO"],
        "area": "cobranca",
        "evidencias": [{
            "arquivo": "db/02_parametros.sql",
            "linha_inicial": 6,
            "linha_final": 7,
            "trecho": "'PERC_MULTA_ATRASO', 2.0000"
        }]
    });
    let r = c.chamar("propor_termo", args.clone()).unwrap();
    assert_eq!(r["termo"], 1);
    assert_eq!(
        r["glossario"],
        "faturamento-exemplo/glossario/multa-por-atraso.md"
    );
    assert_eq!(r["ja_no_glossario"], false);
    let estado = c.analise(&a.id);
    assert_eq!(estado.termos[0].onde_vive.len(), 2);
    assert_eq!(estado.termos[0].evidencias[0].linhas.to_string(), "6-7");
    assert!(c.eventos().iter().any(|e| e["mudanca"] == "termo"));

    let m = c.recusa("propor_termo", args.clone());
    assert!(m.contains("já foi proposto nesta análise"), "{m}");

    let mut outro = args;
    outro["termo"] = json!("Juros de mora");
    outro["evidencias"][0]["linha_inicial"] = json!(200);
    outro["evidencias"][0]["linha_final"] = json!(201);
    assert!(
        c.recusa("propor_termo", outro)
            .contains("passa do fim do arquivo")
    );
    assert_eq!(c.analise(&a.id).termos.len(), 1);
}
