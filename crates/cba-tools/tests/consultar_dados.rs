//! `consultar_dados` (#440): visibilidade por análise, aprovação, recusa, edição,
//! mascaramento, registro sem valores e a evidência de dados no documento.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use cba_core::ambiente::Ambiente;
use cba_core::analise::{Analise, Fluxo};
use cba_core::analises::NovaAnalise;
use cba_core::banco::{BancoConectado, ColunaDados, OrigemBanco, ResultadoDados, SchemaBanco};
use cba_core::config::Config;
use cba_core::consultas::{DecisaoConsulta, SituacaoConsulta};
use cba_core::dados::DiretorioDados;
use cba_core::eventos::Mudanca;
use cba_kb::Id;
use cba_tools::consultar_dados::consultar_dados_com_prazo;
use cba_tools::{ErroChamada, ErroFerramenta, Registro, Saida, registro_padrao};
use serde_json::json;

const CONEXAO: &str = "banco-hml";

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/faturamento-exemplo")
}

/// Banco de teste: devolve um resultado fixo e guarda o SQL executado.
#[derive(Debug, Default)]
struct BancoFalso {
    executados: Mutex<Vec<String>>,
}

impl BancoConectado for BancoFalso {
    fn origem(&self) -> OrigemBanco {
        OrigemBanco {
            conexao: CONEXAO.into(),
            nome: "Banco de homologação".into(),
            tipo: "PostgreSQL".into(),
            ambiente: Some("homologação".into()),
        }
    }

    fn ler_schema(&self) -> Result<SchemaBanco, String> {
        Ok(SchemaBanco::default())
    }

    fn validar(&self, sql: &str) -> Result<(), String> {
        if sql.to_uppercase().contains("DELETE") {
            Err("consulta recusada pelo app: só SELECT".into())
        } else {
            Ok(())
        }
    }

    fn consultar(&self, sql: &str) -> Result<ResultadoDados, String> {
        self.executados.lock().unwrap().push(sql.to_owned());
        let col = |nome: &str, tipo: &str| ColunaDados {
            nome: nome.into(),
            tipo: tipo.into(),
        };
        if sql.contains("count") {
            return Ok(ResultadoDados {
                colunas: vec![col("situacao", "VARCHAR"), col("total", "INT8")],
                linhas: vec![vec![json!("vencida"), json!(312)]],
                cortada: false,
                limite_linhas: 200,
                duracao_ms: 4,
            });
        }
        Ok(ResultadoDados {
            colunas: vec![
                col("id", "INT4"),
                col("nome", "VARCHAR"),
                col("cpf", "BPCHAR"),
                col("email", "VARCHAR"),
                col("motivo", "VARCHAR"),
            ],
            linhas: vec![vec![
                json!(7),
                json!("Maria da Silva"),
                json!("12345678909"),
                json!("maria@example.com"),
                json!("Ligou João Souza, CPF 987.654.321-00, e-mail joao@example.com"),
            ]],
            cortada: true,
            limite_linhas: 1,
            duracao_ms: 9,
        })
    }
}

struct Cenario {
    _dir: tempfile::TempDir,
    amb: Ambiente,
    banco: Arc<BancoFalso>,
}

fn cenario(com_banco_no_projeto: bool) -> Cenario {
    let dir = tempfile::tempdir().unwrap();
    let raiz = dir.path();
    let dados = DiretorioDados::em(raiz.join("dados"));
    let mut projeto = cba_index::Projeto::novo("Faturamento Exemplo", [fixture()]).unwrap();
    if com_banco_no_projeto {
        projeto.bancos = vec![CONEXAO.into()];
    }
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
    let mut amb = Ambiente::do_diretorio(&fixture(), &dados).unwrap();
    let banco = Arc::new(BancoFalso::default());
    amb.banco = Some(banco.clone());
    Cenario {
        _dir: dir,
        amb,
        banco,
    }
}

impl Cenario {
    fn criar(&self, ligar: bool) -> Analise {
        let a = self
            .amb
            .analises
            .criar(NovaAnalise {
                fluxo: Fluxo::Impacto,
                titulo: "Multa zerada".into(),
                entrada: "Faturas vencidas estão saindo sem multa.".into(),
                analista: Some("Ana Exemplo".into()),
                base: None,
            })
            .unwrap();
        if ligar {
            self.amb
                .analises
                .alterar(&a.id, Mudanca::Consulta, |a, agora| {
                    a.ligar_banco(CONEXAO, "Banco de homologação", Some("ana".into()), agora)
                })
                .unwrap();
        }
        a
    }

    fn carregar(&self, id: &Id) -> Analise {
        self.amb.analises.carregar(id).unwrap()
    }

    /// Chama `consultar_dados` numa thread e, quando a consulta aparece pendente,
    /// decide como o analista. Devolve a saída da ferramenta.
    fn consultar_e_decidir(
        &self,
        id: &Id,
        sql: &str,
        decisao: Option<DecisaoConsulta>,
    ) -> Result<Saida, ErroFerramenta> {
        let amb = self.amb.clone();
        let args = json!({
            "analise": id.to_string(),
            "objetivo": "medir quantas faturas estão na mesma situação",
            "sql": sql,
        });
        let prazo = if decisao.is_some() {
            Duration::from_secs(30)
        } else {
            Duration::from_millis(300)
        };
        let chamada = thread::spawn(move || {
            let mut r = Registro::new();
            r.registrar(consultar_dados_com_prazo(prazo));
            match r.chamar("consultar_dados", Ok(&amb), Some(args)) {
                Ok(s) => Ok(s),
                Err(ErroChamada::Ferramenta(e)) => Err(e),
                Err(e) => panic!("{e}"),
            }
        });
        if let Some(decisao) = decisao {
            let inicio = Instant::now();
            let numero = loop {
                let a = self.carregar(id);
                if let Some(c) = a.consultas_pendentes().last() {
                    // A tela mostra o pedido com o objetivo, o SQL e a conexão.
                    assert_eq!(c.objetivo, "medir quantas faturas estão na mesma situação");
                    assert_eq!(c.origem.ambiente.as_deref(), Some("homologação"));
                    break c.numero;
                }
                assert!(inicio.elapsed() < Duration::from_secs(10), "sem pedido");
                thread::sleep(Duration::from_millis(20));
            };
            // Enquanto o analista não decide, nada roda no banco.
            assert!(self.banco.executados.lock().unwrap().is_empty() || numero > 1);
            self.amb
                .analises
                .alterar(id, Mudanca::Consulta, |a, agora| {
                    a.decidir_consulta(numero, decisao, Some("ana".into()), agora)
                })
                .unwrap();
        }
        chamada.join().unwrap()
    }
}

#[test]
fn sem_banco_ligado_a_ferramenta_nao_aparece() {
    // Sem banco na sessão, a ferramenta não está na lista e recusa se chamada.
    let mut c = cenario(true);
    c.amb.banco = None;
    let r = registro_padrao();
    let nomes: Vec<&str> = r.visiveis(Ok(&c.amb)).iter().map(|f| f.nome()).collect();
    assert!(!nomes.contains(&"consultar_dados"));
    assert!(nomes.contains(&"schema"));
    let a = c.criar(true);
    let e = r
        .chamar(
            "consultar_dados",
            Ok(&c.amb),
            Some(json!({"analise": a.id.to_string(), "objetivo": "x", "sql": "SELECT 1"})),
        )
        .unwrap_err();
    assert!(e.to_string().contains("Usar o banco"), "{e}");

    // Com o banco na sessão, aparece.
    let c = cenario(true);
    let nomes: Vec<&str> = registro_padrao()
        .visiveis(Ok(&c.amb))
        .iter()
        .map(|f| f.nome())
        .collect();
    assert!(nomes.contains(&"consultar_dados"));
    // Mas a análise sem o banco ligado (ou o projeto sem a conexão) recusa.
    let a = c.criar(false);
    let e = c
        .consultar_e_decidir(&a.id, "SELECT count(*) FROM fatura", None)
        .unwrap_err();
    assert!(e.to_string().contains("não está ligado"), "{e}");
    let c = cenario(false);
    let a = c.criar(true);
    let e = c
        .consultar_e_decidir(&a.id, "SELECT count(*) FROM fatura", None)
        .unwrap_err();
    assert!(e.to_string().contains("não está ligado"), "{e}");
    assert!(c.carregar(&a.id).consultas.is_empty());
}

#[test]
fn aprovada_executa_mascara_e_registra_sem_valores() {
    let c = cenario(true);
    let a = c.criar(true);
    let s = c
        .consultar_e_decidir(
            &a.id,
            "SELECT * FROM cliente WHERE id = 7",
            Some(DecisaoConsulta::Executar),
        )
        .unwrap();
    let dados = s.dados.unwrap();
    assert_eq!(dados["consulta"], 1);
    // Nada pessoal chega ao agente, nem no texto nem nos dados.
    let tudo = format!("{}{}", s.texto, dados);
    for vazado in [
        "Maria",
        "12345678909",
        "maria@example.com",
        "João",
        "987.654.321-00",
        "joao@example.com",
    ] {
        assert!(!tudo.contains(vazado), "{vazado} vazou: {tudo}");
    }
    assert_eq!(dados["linhas"][0][0], 7);
    assert_eq!(dados["linhas"][0][1], "[mascarado]");
    assert_eq!(dados["linhas"][0][4], "[nome], CPF [cpf], e-mail [e-mail]");
    let mascaradas: Vec<&str> = dados["mascaradas"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["coluna"].as_str().unwrap())
        .collect();
    assert_eq!(mascaradas, ["nome", "cpf", "email", "motivo"]);
    assert!(s.texto.contains("Colunas mascaradas"), "{}", s.texto);
    assert!(s.texto.contains("cortada no limite"), "{}", s.texto);

    // O registro guarda quem aprovou, quando, o SQL e as linhas; nunca os valores.
    let analise = c.carregar(&a.id);
    let consulta = &analise.consultas[0];
    assert_eq!(consulta.situacao, SituacaoConsulta::Executada);
    assert_eq!(consulta.decidida_por.as_deref(), Some("ana"));
    assert!(consulta.decidida_ms.is_some() && consulta.executada_ms.is_some());
    assert_eq!(consulta.sql(), "SELECT * FROM cliente WHERE id = 7");
    assert_eq!(consulta.linhas, Some(1));
    assert!(consulta.cortada);
    assert_eq!(consulta.mascaradas, ["nome", "cpf", "email", "motivo"]);
    let estado = serde_json::to_string(&analise).unwrap();
    for vazado in ["Maria", "12345678909", "maria@", "[nome]", "[mascarado]"] {
        assert!(!estado.contains(vazado), "{vazado} no estado");
    }
}

#[test]
fn recusa_devolve_o_motivo_e_nao_executa() {
    let c = cenario(true);
    let a = c.criar(true);
    let e = c
        .consultar_e_decidir(
            &a.id,
            "SELECT * FROM cliente",
            Some(DecisaoConsulta::Recusar {
                motivo: "traz clientes inteiros; use COUNT por situação".into(),
            }),
        )
        .unwrap_err();
    assert!(
        e.to_string()
            .contains("traz clientes inteiros; use COUNT por situação"),
        "{e}"
    );
    assert!(c.banco.executados.lock().unwrap().is_empty());
    let analise = c.carregar(&a.id);
    assert_eq!(analise.consultas[0].situacao, SituacaoConsulta::Recusada);
    assert_eq!(analise.consultas[0].decidida_por.as_deref(), Some("ana"));
    assert_eq!(analise.consultas[0].sql_proposto, "SELECT * FROM cliente");
}

#[test]
fn editar_e_executar_roda_o_sql_do_analista() {
    let c = cenario(true);
    let a = c.criar(true);
    let s = c
        .consultar_e_decidir(
            &a.id,
            "SELECT * FROM fatura",
            Some(DecisaoConsulta::EditarEExecutar {
                sql: "SELECT situacao, count(*) AS total FROM fatura GROUP BY situacao".into(),
            }),
        )
        .unwrap();
    assert_eq!(
        *c.banco.executados.lock().unwrap(),
        ["SELECT situacao, count(*) AS total FROM fatura GROUP BY situacao"]
    );
    assert!(s.texto.contains("com edição"), "{}", s.texto);
    assert!(s.texto.contains("Nenhuma coluna mascarada"), "{}", s.texto);
    let d = s.dados.unwrap();
    assert_eq!(d["editada"], true);
    assert_eq!(d["linhas"], json!([["vencida", 312]]));
    let consulta = &c.carregar(&a.id).consultas[0];
    assert!(consulta.editada);
    assert_eq!(consulta.sql_proposto, "SELECT * FROM fatura");
}

#[test]
fn sem_resposta_expira_e_sql_invalido_nem_chega_ao_analista() {
    let c = cenario(true);
    let a = c.criar(true);
    let e = c
        .consultar_e_decidir(&a.id, "SELECT count(*) FROM fatura", None)
        .unwrap_err();
    assert!(e.to_string().contains("não foi aprovada a tempo"), "{e}");
    assert_eq!(
        c.carregar(&a.id).consultas[0].situacao,
        SituacaoConsulta::Expirada
    );
    let e = c
        .consultar_e_decidir(&a.id, "DELETE FROM fatura", None)
        .unwrap_err();
    assert!(e.to_string().contains("só SELECT"), "{e}");
    assert_eq!(c.carregar(&a.id).consultas.len(), 1);
}

#[test]
fn evidencia_de_dados_no_documento_e_na_exportacao_sem_evidencias() {
    let c = cenario(true);
    let a = c.criar(true);
    c.consultar_e_decidir(
        &a.id,
        "SELECT situacao, count(*) AS total\nFROM fatura\nGROUP BY situacao",
        Some(DecisaoConsulta::Executar),
    )
    .unwrap();
    // Consulta não executada não vira evidência.
    let e = registro_padrao()
        .chamar(
            "afirmar",
            Ok(&c.amb),
            Some(json!({
                "analise": a.id.to_string(),
                "secao": "abrangencia",
                "texto": "Média: 312 faturas vencidas na mesma situação.",
                "confianca": "confirmado",
                "dados": [{ "consulta": 9, "resumo": "x" }],
            })),
        )
        .unwrap_err();
    assert!(e.to_string().contains("consulta"), "{e}");
    // Confirmada só com a evidência de dados.
    registro_padrao()
        .chamar(
            "afirmar",
            Ok(&c.amb),
            Some(json!({
                "analise": a.id.to_string(),
                "secao": "abrangencia",
                "texto": "Média: 312 faturas vencidas na mesma situação.",
                "confianca": "confirmado",
                "dados": [{ "consulta": 1, "resumo": "312 faturas vencidas com multa zerada" }],
            })),
        )
        .unwrap();
    let analise = c.carregar(&a.id);
    assert_eq!(analise.afirmacoes[0].dados.len(), 1);
    let documento = std::fs::read_to_string(c.amb.kb.caminho(&analise.documento)).unwrap();
    assert!(
        documento.contains("(em dados, ")
            && documento.contains(": 312 faturas vencidas com multa zerada)[^dados-1]"),
        "{documento}"
    );
    let nota = documento
        .lines()
        .find(|l| l.starts_with("[^dados-1]:"))
        .unwrap_or_else(|| panic!("sem nota: {documento}"));
    for parte in [
        "Banco de homologação (PostgreSQL, homologação)",
        "UTC",
        "SELECT situacao, count(*) AS total FROM fatura GROUP BY situacao",
        "1 linha(s)",
    ] {
        assert!(nota.contains(parte), "{parte}: {nota}");
    }
    assert!(
        documento.contains("a revalidação confere o código, não reexecuta consultas"),
        "{documento}"
    );

    // Sem evidências, fica só o resumo.
    let sem = cba_docs::exportar_markdown(
        &documento,
        &cba_docs::OpcoesExportacao {
            evidencias: false,
            ..Default::default()
        },
        None,
    );
    assert!(
        sem.contains("312 faturas vencidas com multa zerada"),
        "{sem}"
    );
    assert!(!sem.contains("SELECT"), "{sem}");
    assert!(!sem.contains("[^dados-1]"), "{sem}");

    // A revalidação não mexe na evidência de dados.
    cba_tools::revalidacao::revalidar_projeto(&c.amb, &c.amb.dados).unwrap();
    assert_eq!(
        c.carregar(&a.id).afirmacoes[0].dados,
        analise.afirmacoes[0].dados
    );
}

#[test]
fn roteiro_de_impacto_orienta_agregados_ou_sql_pronto() {
    let c = cenario(true);
    let a = c.criar(true);
    let r = registro_padrao()
        .chamar(
            "iniciar_analise",
            Ok(&c.amb),
            Some(json!({ "analise": a.id.to_string() })),
        )
        .unwrap();
    assert!(r.texto.contains("## Banco de dados ligado"), "{}", r.texto);
    assert!(r.texto.contains("COUNT"), "{}", r.texto);

    let mut sem = cenario(true);
    sem.amb.banco = None;
    let a = sem.criar(false);
    let r = registro_padrao()
        .chamar(
            "iniciar_analise",
            Ok(&sem.amb),
            Some(json!({ "analise": a.id.to_string() })),
        )
        .unwrap();
    assert!(r.texto.contains("## Verificação em dados"), "{}", r.texto);
    assert!(!r.texto.contains("consultar_dados"), "{}", r.texto);
    // As consultas ficam no documento pela ferramenta própria (#453).
    assert!(
        r.texto
            .contains("registre cada uma com `registrar_verificacao`"),
        "{}",
        r.texto
    );
}
