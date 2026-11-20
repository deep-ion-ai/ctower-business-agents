//! Mapa de funcionalidades (#208) com o sistema fictício "Faturamento Exemplo":
//! funcionalidade sem ponto de entrada válido é recusada, a conclusão lista os
//! pontos sem tratamento, o documento `funcionalidades/mapa.md` entra no índice,
//! uma segunda sessão mantém os identificadores e a explicação aberta a partir
//! do mapa grava o documento negocial ligado à funcionalidade. Nada disso cria
//! análise.

use std::path::{Path, PathBuf};

use cba_core::ambiente::Ambiente;
use cba_core::config::Config;
use cba_core::dados::DiretorioDados;
use cba_core::funcionalidades::{
    DecisaoFuncionalidade, MapaFuncionalidades, SituacaoFuncionalidade,
};
use cba_index::PontoEntrada;
use cba_kb::{Id, IdFuncionalidade, layout};
use cba_tools::aprovacao::OpcoesAprovacao;
use cba_tools::mapear::{RevisaoDoMapa, revisar_mapa};
use cba_tools::revisao::{self, Decisao};
use cba_tools::{ErroChamada, ErroFerramenta, registro_padrao};
use serde_json::{Value, json};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/faturamento-exemplo")
}

struct Cenario {
    _dir: tempfile::TempDir,
    amb: Ambiente,
    pontos: Vec<PontoEntrada>,
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
    fn chamar(&self, ferramenta: &str, args: Value) -> Result<Value, ErroFerramenta> {
        match registro_padrao().chamar(ferramenta, Ok(&self.amb), Some(args)) {
            Ok(saida) => Ok(saida.dados.expect("resposta estruturada")),
            Err(ErroChamada::Ferramenta(e)) => Err(e),
            Err(e) => panic!("{e}"),
        }
    }

    fn ok(&self, ferramenta: &str, args: Value) -> Value {
        self.chamar(ferramenta, args)
            .unwrap_or_else(|e| panic!("{ferramenta}: {e}"))
    }

    fn recusa(&self, ferramenta: &str, args: Value) -> String {
        match self.chamar(ferramenta, args) {
            Err(ErroFerramenta::Recusa(m) | ErroFerramenta::Argumentos(m)) => m,
            outro => panic!("esperava recusa, veio {outro:?}"),
        }
    }

    /// Ponto citado com a evidência da declaração (a primeira linha do símbolo).
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
        json!({
            "ponto": p.chave,
            "evidencia": {
                "arquivo": p.arquivo,
                "linha_inicial": p.linhas.inicio,
                "linha_final": p.linhas.inicio,
                "trecho": linha,
            }
        })
    }

    fn funcionalidade(&self, nome: &str, area: &str, pontos: &[&str], id: Option<&str>) -> Value {
        let mut args = json!({
            "nome": nome,
            "area": area,
            "descricao": format!("{nome}, para o atendimento."),
            "atores": ["Atendente"],
            "pontos_de_entrada": pontos.iter().map(|p| self.ponto(p)).collect::<Vec<_>>(),
            "dados_lidos": ["FATURA"],
        });
        if let Some(id) = id {
            args["id"] = json!(id);
        }
        self.ok("registrar_funcionalidade", args)
    }

    fn mapa(&self) -> MapaFuncionalidades {
        MapaFuncionalidades::carregar(&self.amb.kb).unwrap()
    }

    fn revisar(&self, r: RevisaoDoMapa) -> Option<IdFuncionalidade> {
        revisar_mapa(&self.amb.base, &self.amb.kb, &self.amb.dados, r)
            .unwrap()
            .resultado
    }

    fn aceitar_todas(&self) {
        for f in self.mapa().abertas() {
            self.revisar(RevisaoDoMapa::Decidir {
                funcionalidade: f.id.clone(),
                decisao: DecisaoFuncionalidade::Aceitar,
            });
        }
    }
}

fn f(id: &str) -> IdFuncionalidade {
    IdFuncionalidade::analisar(id).unwrap()
}

/// Registra o mapa da fixture como o gabarito espera, deixando o ponto
/// `PRC_SUSPENDER_COBRANCA` de fora.
fn mapear_quase_tudo(c: &Cenario) {
    c.funcionalidade(
        "Calcular encargos por atraso",
        "Cobrança",
        &[
            "EncargosService.calcular",
            "PKG_COBRANCA.FN_CALCULAR_MULTA",
            "PKG_COBRANCA.FN_CALCULAR_JUROS",
            "PKG_COBRANCA.FN_DIAS_ATRASO",
        ],
        None,
    );
    c.funcionalidade(
        "Parcelar fatura",
        "Parcelamento",
        &[
            "ParcelamentoService.parcelar",
            "PKG_SITUACAO.PRC_EFETIVAR_PARCELAMENTO",
            "#opcoesDeParcelas",
            "#enviarParcelamento",
        ],
        None,
    );
    c.funcionalidade(
        "Cancelar fatura",
        "Cancelamento",
        &["#FrmCancelamento", "PKG_COBRANCA.PRC_CANCELAR_FATURA"],
        None,
    );
    c.funcionalidade(
        "Gerar lote mensal de cobrança",
        "Cobrança",
        &["#CALCOBR"],
        None,
    );
    c.funcionalidade(
        "Registrar pagamento",
        "Cobrança",
        &[
            "PKG_SITUACAO.PRC_REGISTRAR_PAGAMENTO",
            "PKG_SITUACAO.PRC_QUITAR_PARCELA",
        ],
        None,
    );
    c.ok(
        "tratar_ponto_de_entrada",
        json!({
            "ponto": "db/03_pkg_cobranca.sql#PKG_COBRANCA.FN_PARAMETRO",
            "tratamento": "justificar",
            "motivo": "utilitario",
            "justificativa": "Lê os parâmetros para as rotinas de cobrança e de parcelamento.",
        }),
    );
}

#[test]
fn mapeia_conclui_revisa_e_mantem_os_ids() {
    let c = cenario();
    let roteiro = registro_padrao()
        .chamar(
            "iniciar_mapeamento",
            Ok(&c.amb),
            Some(json!({ "pasta": "db" })),
        )
        .unwrap()
        .texto;
    assert!(roteiro.contains("Mapear a pasta `db`."), "{roteiro}");
    assert!(
        roteiro.contains("`pontos_de_entrada` (pasta: \"db\")"),
        "{roteiro}"
    );
    assert!(
        roteiro.contains("9 pontos de entrada, 9 ainda sem"),
        "{roteiro}"
    );
    assert!(!roteiro.contains("{{"), "{roteiro}");

    // Sem ponto de entrada, ou com evidência que não confere: recusada.
    let m = c.recusa(
        "registrar_funcionalidade",
        json!({
            "nome": "X", "area": "Cobrança", "descricao": "X.",
            "pontos_de_entrada": []
        }),
    );
    assert!(m.contains("pontos_de_entrada"), "{m}");
    let m = c.recusa(
        "registrar_funcionalidade",
        json!({
            "nome": "X", "area": "Cobrança", "descricao": "X.",
            "pontos_de_entrada": [{
                "ponto": "PKG_COBRANCA.PRC_CANCELAR_FATURA",
                "evidencia": {
                    "arquivo": "db/03_pkg_cobranca.sql",
                    "linha_inicial": 99, "linha_final": 99,
                    "trecho": "trecho que não existe"
                }
            }]
        }),
    );
    assert!(m.contains("evidência recusada"), "{m}");
    let m = c.recusa(
        "registrar_funcionalidade",
        json!({
            "id": "F-COB-009", "nome": "X", "area": "Cobrança", "descricao": "X.",
            "pontos_de_entrada": [c.ponto("PKG_COBRANCA.PRC_CANCELAR_FATURA")]
        }),
    );
    assert!(m.contains("não existe no mapa"), "{m}");
    assert!(c.mapa().funcionalidades.is_empty());
    assert_eq!(c.mapa().versao, 0, "recusa não grava");

    mapear_quase_tudo(&c);
    let mapa = c.mapa();
    let ids: Vec<String> = mapa
        .funcionalidades
        .iter()
        .map(|f| f.id.to_string())
        .collect();
    assert_eq!(
        ids,
        [
            "F-COB-001",
            "F-PAR-001",
            "F-CAN-001",
            "F-COB-002",
            "F-COB-003"
        ]
    );
    assert!(
        mapa.funcionalidades
            .iter()
            .all(|f| f.pontos.iter().all(|p| p.chave.is_some()))
    );
    assert!(
        c.amb.analises.listar().unwrap().is_empty(),
        "o mapa não cria análise"
    );

    // Falta um ponto: a conclusão lista e recusa.
    let m = c.recusa("concluir_mapeamento", json!({}));
    assert!(m.contains("PKG_SITUACAO.PRC_SUSPENDER_COBRANCA"), "{m}");
    assert!(m.contains("1 ponto(s) de entrada"), "{m}");
    let sem = c.ok("pontos_de_entrada", json!({ "so_sem_tratamento": true }));
    assert_eq!(sem["sem_tratamento"], 1);
    assert_eq!(sem["total"], 15);
    // No recorte da área Parcelamento, nada falta.
    let r = c.ok("concluir_mapeamento", json!({ "area": "Parcelamento" }));
    assert_eq!(r["pontos_de_entrada"], 4);

    c.funcionalidade(
        "Suspender a cobrança por contestação",
        "Cobrança",
        &["PKG_SITUACAO.PRC_SUSPENDER_COBRANCA"],
        None,
    );
    let r = c.ok("concluir_mapeamento", json!({}));
    assert_eq!(r["funcionalidades"], 6);
    assert_eq!(r["a_revisar"], 6);

    // O documento do mapa é remontado a cada registro, entra no índice e na busca.
    let caminho = layout::mapa_de_funcionalidades();
    let md = std::fs::read_to_string(c.amb.kb.caminho(&caminho)).unwrap();
    assert!(md.contains("tipo: mapa-funcionalidades"), "{md}");
    assert!(
        md.contains("| F-CAN-001 | Cancelar fatura | a revisar | 2 |"),
        "{md}"
    );
    assert!(
        md.contains("| `PKG_COBRANCA.FN_PARAMETRO` | rotina de banco"),
        "{md}"
    );
    c.aceitar_todas();
    let md = std::fs::read_to_string(c.amb.kb.caminho(&caminho)).unwrap();
    assert!(
        md.contains("| F-CAN-001 | Cancelar fatura | aceita | 2 |"),
        "{md}"
    );
    assert!(md.contains("status: aprovada"), "{md}");
    let mut indice =
        cba_kb::indice::Indice::abrir(c.amb.dados.arquivo_do_indice(), &c.amb.base).unwrap();
    indice.sincronizar().unwrap();
    let achados = indice
        .buscar(&cba_kb::indice::Consulta::nova("Parcelar fatura"))
        .unwrap();
    let doc = achados
        .resultados
        .iter()
        .find(|r| r.caminho.as_str().ends_with("funcionalidades/mapa.md"))
        .unwrap_or_else(|| panic!("{achados:?}"));
    assert_eq!(
        doc.tipo,
        Some(cba_kb::indice::Tipo::Documento(
            cba_kb::TipoDocumento::MapaFuncionalidades
        ))
    );

    // Segunda sessão: sem `id`, o ponto em comum devolve o identificador; com
    // `id`, o nome muda e o identificador fica. A funcionalidade volta a revisar.
    let r = c.funcionalidade(
        "Cancelar uma fatura emitida",
        "Cancelamento",
        &["#FrmCancelamento", "PKG_COBRANCA.PRC_CANCELAR_FATURA"],
        None,
    );
    assert_eq!(r["funcionalidade"], "F-CAN-001");
    assert_eq!(r["substituida"], true);
    assert_eq!(
        c.mapa().funcionalidade(&f("F-CAN-001")).unwrap().situacao,
        SituacaoFuncionalidade::Pendente
    );
    let r = c.funcionalidade(
        "Parcelamento da fatura",
        "Parcelamento",
        &[
            "ParcelamentoService.parcelar",
            "PKG_SITUACAO.PRC_EFETIVAR_PARCELAMENTO",
            "#opcoesDeParcelas",
            "#enviarParcelamento",
        ],
        Some("F-PAR-001"),
    );
    assert_eq!(r["funcionalidade"], "F-PAR-001");
    // Funcionalidade nova continua a numeração da sigla; um ponto fora da lista
    // do mapa entra só com a evidência.
    let r = c.ok(
        "registrar_funcionalidade",
        json!({
            "nome": "Limitar encargos por tipo de cliente", "area": "Cobrança",
            "descricao": "Aplica o teto de encargos.",
            "pontos_de_entrada": [{
                "ponto": "regra de teto chamada pelo cálculo",
                "evidencia": {
                    "arquivo": "backend/src/main/java/exemplo/faturamento/TetoEncargosRegra.java",
                    "linha_inicial": 10, "linha_final": 10,
                    "trecho": "public final class TetoEncargosRegra"
                }
            }]
        }),
    );
    assert_eq!(r["funcionalidade"], "F-COB-005");
    assert_eq!(r["pontos_de_entrada"][0]["no_mapa"], false);

    // Revisão: dividir, juntar, descartar; o número descartado não volta.
    let novo = c
        .revisar(RevisaoDoMapa::Dividir {
            funcionalidade: f("F-PAR-001"),
            pontos: vec![2, 3],
            nome: "Simular parcelas".into(),
            descricao: "Mostra as opções.".into(),
        })
        .unwrap();
    assert_eq!(novo, f("F-PAR-002"));
    c.revisar(RevisaoDoMapa::Juntar {
        origem: f("F-PAR-002"),
        destino: f("F-PAR-001"),
    });
    c.revisar(RevisaoDoMapa::Decidir {
        funcionalidade: f("F-COB-005"),
        decisao: DecisaoFuncionalidade::Descartar,
    });
    let r = c.ok(
        "registrar_funcionalidade",
        json!({
            "nome": "Outra regra", "area": "Cobrança", "descricao": "D.",
            "pontos_de_entrada": [{
                "ponto": "outra regra",
                "evidencia": {
                    "arquivo": "backend/src/main/java/exemplo/faturamento/TetoEncargosRegra.java",
                    "linha_inicial": 10, "linha_final": 10,
                    "trecho": "public final class TetoEncargosRegra"
                }
            }]
        }),
    );
    assert_eq!(r["funcionalidade"], "F-COB-006");
}

#[test]
fn explicacao_do_mapa_grava_o_documento_negocial_ligado() {
    let c = cenario();
    mapear_quase_tudo(&c);
    c.aceitar_todas();

    let r = c.ok(
        "iniciar_analise",
        json!({ "fluxo": "explicar", "funcionalidade": "F-CAN-001" }),
    );
    let e = Id::analisar(r["analise"].as_str().unwrap()).unwrap();
    let estado = c.amb.analises.carregar(&e).unwrap();
    assert_eq!(estado.funcionalidade, Some(f("F-CAN-001")));
    assert!(
        estado.entrada.contains("Cancelar fatura"),
        "{}",
        estado.entrada
    );
    assert!(
        estado.entrada.contains("FrmCancelamento"),
        "{}",
        estado.entrada
    );

    for (secao, texto) in [
        ("objetivo", "Cancelar uma fatura emitida, com motivo."),
        (
            "fluxo_principal",
            "1. O atendente informa a fatura e o motivo.",
        ),
        ("regras", "Só fatura aberta pode ser cancelada."),
    ] {
        c.ok(
            "afirmar",
            json!({ "analise": e, "secao": secao, "texto": texto, "confianca": "inferido" }),
        );
    }
    c.ok("concluir_analise", json!({ "analise": e }));
    let a2 = c.amb.analises.carregar(&e).unwrap();
    for af in &a2.afirmacoes {
        revisao::revisar(
            &c.amb,
            &e,
            Decisao::Afirmacao {
                numero: af.numero,
                estado: cba_core::analise::EstadoAfirmacao::Confirmada,
            },
        )
        .unwrap();
    }
    let aprovacao = revisao::aprovar_revisada(
        &c.amb,
        &e,
        &OpcoesAprovacao {
            regras: Some(Default::default()),
            ..Default::default()
        },
    )
    .unwrap();
    let negocial = aprovacao.documento_negocial.expect("documento negocial");
    assert_eq!(
        negocial.as_str(),
        "funcionalidades/cancelamento/cancelar-fatura/negocial.md"
    );
    let texto = std::fs::read_to_string(c.amb.kb.caminho(&negocial)).unwrap();
    assert!(texto.contains("tipo: funcionalidade"), "{texto}");
    assert!(texto.contains("Só fatura aberta"), "{texto}");
    let mapa = c.mapa();
    let item = mapa.funcionalidade(&f("F-CAN-001")).unwrap();
    assert_eq!(item.documento.as_ref(), Some(&negocial));
    assert_eq!(item.explicacao.as_ref(), Some(&e));
    assert_eq!(item.situacao, SituacaoFuncionalidade::Aceita);
    let md = std::fs::read_to_string(c.amb.kb.caminho(&layout::mapa_de_funcionalidades())).unwrap();
    assert!(md.contains("aceita · documentada"), "{md}");
}

#[test]
fn agente_propoe_o_vinculo_de_issue_com_evidencia() {
    let c = cenario();
    mapear_quase_tudo(&c);
    let id = c.mapa().abertas().next().unwrap().id.to_string();
    c.ok(
        "propor_vinculo_de_issue",
        json!({
            "origem": "github",
            "chave": "#41",
            "funcionalidades": [id],
            "evidencia": "cita PKG_COBRANCA.FN_CALCULAR_MULTA",
        }),
    );
    let propostas = c.mapa().propostas_de_issue;
    assert_eq!(propostas.len(), 1);
    assert_eq!(propostas[0].chave, "#41");
    // Repetir substitui; funcionalidade inexistente e evidência vazia recusam.
    c.ok(
        "propor_vinculo_de_issue",
        json!({"origem": "github", "chave": "#41", "funcionalidades": [id], "evidencia": "outra"}),
    );
    assert_eq!(c.mapa().propostas_de_issue.len(), 1);
    assert_eq!(c.mapa().propostas_de_issue[0].evidencia, "outra");
    for args in [
        json!({"origem": "github", "chave": "#41", "funcionalidades": ["F-ZZZ-999"], "evidencia": "x"}),
        json!({"origem": "github", "chave": "#41", "funcionalidades": [id], "evidencia": "  "}),
        json!({"origem": "gitlab", "chave": "#41", "funcionalidades": [id], "evidencia": "x"}),
    ] {
        assert!(c.chamar("propor_vinculo_de_issue", args).is_err());
    }
}
