//! Verificações em dados (#453) com o sistema fictício "Faturamento Exemplo":
//! numa demanda de status (faturas com pagamento parcial na situação
//! CANCELADA, a D-FAT-006 do gabarito), a consulta que reproduz a condição da
//! regra no caso relatado e a que conta os registros afetados são aceitas contra
//! os scripts de `db/` e entram no documento; a pergunta aponta para a consulta;
//! o que não é só leitura, o que foge do modelo de dados e o dado real são
//! recusados ao registrar; a exportação sem evidências mantém as verificações.

use std::path::{Path, PathBuf};

use cba_core::ambiente::Ambiente;
use cba_core::analise::{Analise, Fluxo};
use cba_core::analises::NovaAnalise;
use cba_core::config::Config;
use cba_core::dados::DiretorioDados;
use cba_kb::frontmatter::FrontmatterAnalise;
use cba_kb::{Documento, Id};
use cba_tools::revisao::{Formato, OpcoesExportacao, exportar_analise};
use cba_tools::{ErroChamada, ErroFerramenta, registro_padrao};
use serde_json::{Value, json};

const DEMANDA: &str = "Há faturas com pagamento parcial na situação CANCELADA, e o valor \
pago ficou sem destino. O cancelamento pela tela e pelo portal recusa fatura com \
pagamento parcial, então essas faturas devem ter sido canceladas pela rota antiga de \
cancelamento, que não valida. Desativar a rota antiga.";

/// A condição da regra (RN-FAT-012: fatura com pagamento parcial não pode ser
/// cancelada) no caso relatado.
const CONDICAO: &str = "SELECT F.ID_FATURA, F.STATUS, F.VL_ORIGINAL, F.VL_PAGO,
       F.DT_CANCELAMENTO, F.MOTIVO_CANCEL
  FROM FATURA F
 WHERE F.ID_FATURA = :id_fatura
   AND F.STATUS = 'CANCELADA'
   AND F.VL_PAGO > 0";

/// Os registros na mesma situação, por ano do cancelamento.
const CONTAGEM: &str = "SELECT EXTRACT(YEAR FROM F.DT_CANCELAMENTO) AS ANO,
       COUNT(*) AS FATURAS,
       SUM(F.VL_PAGO) AS VALOR_PAGO
  FROM FATURA F
 WHERE F.STATUS = 'CANCELADA'
   AND F.VL_PAGO > 0
 GROUP BY EXTRACT(YEAR FROM F.DT_CANCELAMENTO)
 ORDER BY ANO;";

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
    fn criar(&self, fluxo: Fluxo) -> Analise {
        self.amb
            .analises
            .criar(NovaAnalise {
                fluxo,
                titulo: "Faturas com pagamento parcial canceladas".into(),
                entrada: DEMANDA.into(),
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
}

fn condicao(analise: &Id) -> Value {
    json!({
        "analise": analise.to_string(),
        "objetivo": "Confirmar o diagnóstico no caso relatado",
        "sql": CONDICAO,
        "parametros": [
            { "nome": "id_fatura", "descricao": "o número da fatura do caso relatado" }
        ],
        "resultados": [
            {
                "resultado": "Uma linha",
                "significado": "confirma o diagnóstico: a fatura foi cancelada com pagamento parcial, o que a regra RN-FAT-012 recusa"
            },
            {
                "resultado": "Nenhuma linha",
                "significado": "a fatura não está nessa situação; a causa é outra"
            }
        ]
    })
}

fn contagem(analise: &Id) -> Value {
    json!({
        "analise": analise.to_string(),
        "objetivo": "Contar as faturas na mesma situação, por ano do cancelamento",
        "sql": CONTAGEM,
        "resultados": [
            {
                "resultado": "Zero",
                "significado": "o caso relatado é isolado"
            },
            {
                "resultado": "Mais de zero, em anos recentes",
                "significado": "o caso não é só legado: um caminho ainda cancela sem validar, e a abrangência cresce"
            }
        ]
    })
}

#[test]
fn demanda_de_status_traz_a_condicao_e_a_contagem_validas_contra_os_scripts() {
    let c = cenario();
    let a = c.criar(Fluxo::Impacto);

    let r = c.chamar("registrar_verificacao", condicao(&a.id)).unwrap();
    assert_eq!(r["verificacao"], 1);
    assert_eq!(r["rotulo"], "V1");
    assert_eq!(r["tabelas"], json!(["FATURA"]));
    assert_eq!(r["parametros"], json!(["id_fatura"]));
    let r = c.chamar("registrar_verificacao", contagem(&a.id)).unwrap();
    assert_eq!(r["verificacao"], 2);

    let estado = c.analise(&a.id);
    assert_eq!(estado.verificacoes.len(), 2);
    // O `;` final sai; o resto fica como o agente escreveu, para o leitor.
    assert_eq!(estado.verificacoes[1].sql, CONTAGEM.trim_end_matches(';'));
    assert_eq!(estado.verificacoes[0].parametros[0].nome, "id_fatura");

    let corpo = c.documento(&estado).corpo;
    let secao = corpo
        .split("## Verificações em dados\n")
        .nth(1)
        .and_then(|s| s.split("\n## ").next())
        .unwrap_or_else(|| panic!("sem a seção: {corpo}"));
    assert!(secao.contains("Rode numa sessão só de leitura"), "{secao}");
    assert!(
        secao.contains("### V1. Confirmar o diagnóstico no caso relatado"),
        "{secao}"
    );
    assert!(
        secao.contains(&format!("```sql\n{CONDICAO}\n```")),
        "{secao}"
    );
    assert!(
        secao.contains("- `:id_fatura`: o número da fatura do caso relatado"),
        "{secao}"
    );
    assert!(
        secao.contains("- **Uma linha:** confirma o diagnóstico"),
        "{secao}"
    );
    assert!(
        secao.contains("### V2. Contar as faturas na mesma situação, por ano do cancelamento"),
        "{secao}"
    );
    assert!(secao.contains("COUNT(*) AS FATURAS"), "{secao}");
    // A seção fica antes das perguntas para o solicitante.
    let verificacoes = corpo.find("## Verificações em dados").unwrap();
    let perguntas = corpo.find("## Perguntas para o solicitante").unwrap();
    assert!(verificacoes < perguntas, "{corpo}");

    // A verificação não é seção de afirmação.
    let erro = c.recusa(
        "afirmar",
        json!({
            "analise": a.id.to_string(),
            "secao": "verificacoes_em_dados",
            "texto": "x",
            "confianca": "inferido"
        }),
    );
    assert!(erro.contains("verificacoes_em_dados"), "{erro}");
}

#[test]
fn pergunta_aponta_para_a_consulta() {
    let c = cenario();
    let a = c.criar(Fluxo::Impacto);
    c.chamar("registrar_verificacao", condicao(&a.id)).unwrap();

    let erro = c.recusa(
        "perguntar",
        json!({
            "analise": a.id.to_string(),
            "texto": "A fatura do caso relatado foi cancelada com pagamento parcial?",
            "verificacao": 7
        }),
    );
    assert!(erro.contains("verificação número 7"), "{erro}");
    assert!(c.analise(&a.id).perguntas.is_empty());

    let r = c
        .chamar(
            "perguntar",
            json!({
                "analise": a.id.to_string(),
                "texto": "A fatura do caso relatado foi cancelada com pagamento parcial?",
                "verificacao": 1
            }),
        )
        .unwrap();
    assert_eq!(r["verificacao"], 1);
    let estado = c.analise(&a.id);
    assert_eq!(estado.perguntas[0].verificacao, Some(1));
    let corpo = c.documento(&estado).corpo;
    assert!(
        corpo.contains(
            "1. A fatura do caso relatado foi cancelada com pagamento parcial? (Para \
             responder com dados: consulta V1 em \"Verificações em dados\".)"
        ),
        "{corpo}"
    );
}

#[test]
fn consulta_que_nao_e_so_leitura_e_recusada_ao_registrar() {
    let c = cenario();
    let a = c.criar(Fluxo::Impacto);
    let com_sql = |sql: &str| {
        let mut args = contagem(&a.id);
        args["sql"] = json!(sql);
        args
    };
    for (sql, motivo) in [
        (
            "UPDATE FATURA SET STATUS = 'ABERTA' WHERE STATUS = 'CANCELADA'",
            "só consultas de leitura",
        ),
        ("DELETE FROM LOG_FATURA", "só consultas de leitura"),
        (
            "SELECT COUNT(*) FROM FATURA; DROP TABLE FATURA",
            "uma instrução por vez",
        ),
        (
            "WITH x AS (DELETE FROM FATURA RETURNING ID_FATURA) SELECT COUNT(*) FROM x",
            "",
        ),
        ("SELECT * FROM FATURA FOR UPDATE", "travas"),
    ] {
        let erro = c.recusa("registrar_verificacao", com_sql(sql));
        assert!(erro.starts_with("consulta recusada: "), "{sql}: {erro}");
        assert!(erro.contains(motivo), "{sql}: {erro}");
    }
    assert!(c.analise(&a.id).verificacoes.is_empty());
}

#[test]
fn nomes_fora_do_modelo_e_dado_real_sao_recusados() {
    let c = cenario();
    let a = c.criar(Fluxo::Impacto);
    let com_sql = |sql: &str| {
        let mut args = contagem(&a.id);
        args["sql"] = json!(sql);
        args
    };
    let erro = c.recusa(
        "registrar_verificacao",
        com_sql("SELECT COUNT(*) FROM FATURAS WHERE STATUS = 'CANCELADA'"),
    );
    assert!(erro.contains("FATURAS"), "{erro}");
    assert!(erro.contains("`schema`"), "{erro}");

    let erro = c.recusa(
        "registrar_verificacao",
        com_sql("SELECT COUNT(*) FROM FATURA F WHERE F.VALOR_PAGO > 0"),
    );
    assert!(erro.contains("`F.VALOR_PAGO`"), "{erro}");
    assert!(erro.contains("FATURA não tem a coluna"), "{erro}");

    let erro = c.recusa(
        "registrar_verificacao",
        com_sql("SELECT COUNT(*) FROM FATURA WHERE SITUACAO_X = 'CANCELADA'"),
    );
    assert!(erro.contains("`SITUACAO_X`"), "{erro}");

    // Colunas de duas tabelas, com e sem apelido, e o apelido da coluna.
    let mut juncao = com_sql(
        "SELECT P.STATUS, COUNT(*) AS QTD FROM FATURA F \
         JOIN PARCELA P ON P.ID_FATURA = F.ID_FATURA \
         WHERE F.STATUS = 'CANCELADA' AND NR_PARCELA > 0 GROUP BY P.STATUS ORDER BY QTD",
    );
    juncao["objetivo"] = json!("Separar pela situação das parcelas");
    let r = c.chamar("registrar_verificacao", juncao).unwrap();
    assert_eq!(r["tabelas"], json!(["FATURA", "PARCELA"]));

    let erro = c.recusa(
        "registrar_verificacao",
        com_sql("SELECT STATUS FROM FATURA WHERE ID_FATURA = 4711"),
    );
    assert!(erro.contains("valor fixo (4711)"), "{erro}");

    let erro = c.recusa(
        "registrar_verificacao",
        com_sql("SELECT ID_CLIENTE FROM CLIENTE WHERE DOCUMENTO = '529.982.247-25'"),
    );
    assert!(erro.contains("parece um CPF"), "{erro}");

    // Parâmetro do SQL sem declaração, e declaração sem uso.
    let erro = c.recusa(
        "registrar_verificacao",
        com_sql("SELECT STATUS FROM FATURA WHERE ID_FATURA = :id_fatura"),
    );
    assert!(erro.contains(":id_fatura"), "{erro}");
    let mut sobra = contagem(&a.id);
    sobra["parametros"] = json!([{ "nome": ":ano", "descricao": "o ano" }]);
    let erro = c.recusa("registrar_verificacao", sobra);
    assert!(erro.contains("o SQL não usa :ano"), "{erro}");

    assert_eq!(c.analise(&a.id).verificacoes.len(), 1);
}

#[test]
fn so_na_analise_de_impacto() {
    let c = cenario();
    let a = c.criar(Fluxo::Perguntar);
    let erro = c.recusa("registrar_verificacao", contagem(&a.id));
    assert!(erro.contains("não é uma análise de impacto"), "{erro}");
}

#[test]
fn exportacao_sem_evidencias_mantem_as_verificacoes() {
    let c = cenario();
    let a = c.criar(Fluxo::Impacto);
    c.chamar("registrar_verificacao", condicao(&a.id)).unwrap();
    c.chamar("registrar_verificacao", contagem(&a.id)).unwrap();
    let destino = tempfile::tempdir().unwrap();
    for (nome, opcoes) in [
        (
            "sem-evidencias",
            OpcoesExportacao {
                evidencias: false,
                ..Default::default()
            },
        ),
        (
            "sem-notas-tecnicas",
            OpcoesExportacao {
                evidencias: false,
                notas_tecnicas: false,
                ..Default::default()
            },
        ),
    ] {
        let md = exportar_analise(
            &c.amb,
            &a.id,
            &destino.path().join(nome),
            Formato::Markdown,
            &opcoes,
            &cba_docs::Figuras::default(),
        )
        .unwrap();
        let texto = std::fs::read_to_string(md).unwrap();
        assert!(!texto.contains("## Evidências"), "{nome}: {texto}");
        assert!(
            texto.contains("## Verificações em dados"),
            "{nome}: {texto}"
        );
        assert!(
            texto.contains("Rode numa sessão só de leitura"),
            "{nome}: {texto}"
        );
        assert!(texto.contains(CONDICAO), "{nome}: {texto}");
        assert!(texto.contains("### V2. "), "{nome}: {texto}");
    }
}
