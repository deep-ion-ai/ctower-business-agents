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
use cba_tools::mapear::{
    Casamento, PontosCasaveis, RevisaoDoMapa, localizar_ponto, normalizar_rota, revisar_mapa,
};
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

fn endpoint(nome: &str, metodo: &str, rota: &str, linha: u32) -> PontoEntrada {
    let arquivo = "backend/src/main/java/exemplo/faturamento/FaturaController.java";
    PontoEntrada {
        chave: PontoEntrada::chave_de(arquivo, nome),
        tipo: cba_index::TipoEntrada::Endpoint,
        nome: nome.into(),
        arquivo: arquivo.into(),
        linhas: cba_index::Linhas::new(linha, linha + 3),
        detalhe: Some(format!("endpoint {metodo} {rota}")),
    }
}

#[test]
fn casa_endpoint_com_outro_nome_de_parametro_e_sugere_parecidos() {
    let pontos = vec![
        endpoint(
            "exemplo.FaturaController.buscar./api/faturas/{}",
            "GET",
            "/api/faturas/{id}",
            10,
        ),
        endpoint(
            "exemplo.FaturaController.remover./api/faturas/{}",
            "DELETE",
            "/api/faturas/{id}",
            20,
        ),
        endpoint(
            "exemplo.FaturaController.parcelar./api/faturas/{}/parcelamento",
            "POST",
            "/api/faturas/{id}/parcelamento",
            30,
        ),
        PontoEntrada {
            chave: "db/03_pkg_cobranca.sql#PKG_COBRANCA.PRC_CANCELAR_FATURA".into(),
            tipo: cba_index::TipoEntrada::RotinaBanco,
            nome: "PKG_COBRANCA.PRC_CANCELAR_FATURA".into(),
            arquivo: "db/03_pkg_cobranca.sql".into(),
            linhas: cba_index::Linhas::new(99, 120),
            detalhe: None,
        },
    ];
    let casaveis = PontosCasaveis::novos(&pontos);
    let nome = |citado: &str| match casaveis.casar(citado) {
        Casamento::Um(p) => p.nome.clone(),
        outro => panic!("{citado}: {outro:?}"),
    };

    assert_eq!(
        normalizar_rota("/API/Faturas/{idFatura:[0-9]{1,9}}/"),
        "/api/faturas/{}"
    );
    assert_eq!(
        normalizar_rota("/api/faturas/:idFatura/parcelamento"),
        "/api/faturas/{}/parcelamento"
    );

    // A rota com o método, outro nome de parâmetro, `:x`, `{x:regex}`, barra
    // final e maiúsculas.
    assert!(nome("GET /api/faturas/{idFatura}").contains("buscar"));
    assert!(nome("delete /API/faturas/{id:[0-9]+}/").contains("remover"));
    assert!(nome("/api/faturas/:idFatura/parcelamento").contains("parcelar"));
    // A chave e o nome com outro nome de parâmetro.
    assert!(
        nome(
            "backend/src/main/java/exemplo/faturamento/FaturaController.java#exemplo.FaturaController.buscar./api/faturas/{idFatura}"
        )
        .contains("buscar")
    );
    assert!(
        nome("FaturaController.parcelar./api/faturas/{numero}/parcelamento").contains("parcelar")
    );
    assert!(nome("pkg_cobranca.prc_cancelar_fatura").contains("PRC_CANCELAR_FATURA"));

    // Sem o método, a rota é de dois endpoints: recusa com os candidatos.
    let ambiguo = match localizar_ponto(&casaveis, "/api/faturas/{idFatura}") {
        Err(ErroFerramenta::Recusa(m)) => m,
        outro => panic!("{outro:?}"),
    };
    assert!(ambiguo.contains("casa com 2 pontos"), "{ambiguo}");
    assert!(ambiguo.contains("buscar./api/faturas/{}"), "{ambiguo}");
    assert!(ambiguo.contains("remover./api/faturas/{}"), "{ambiguo}");
    // O método errado não casa.
    assert!(matches!(
        casaveis.casar("PUT /api/faturas/{id}"),
        Casamento::Nenhum(_)
    ));

    // Não achado: as chaves parecidas, as do arquivo citado primeiro.
    let m = match localizar_ponto(
        &casaveis,
        "db/03_pkg_cobranca.sql#PKG_COBRANCA.PRC_ESTORNAR_FATURA",
    ) {
        Err(ErroFerramenta::Recusa(m)) => m,
        outro => panic!("{outro:?}"),
    };
    assert!(m.contains("Chaves parecidas"), "{m}");
    let primeira = m.lines().find(|l| l.starts_with("- `")).unwrap();
    assert!(primeira.contains("PRC_CANCELAR_FATURA"), "{m}");
    let Casamento::Nenhum(parecidos) =
        casaveis.casar("exemplo.FaturaController.cancelar./api/faturas/{id}/cancelamento")
    else {
        panic!("esperava nenhum");
    };
    // Os endpoints do controlador vêm antes da rotina que só tem "cancelar".
    assert_eq!(parecidos.len(), 4);
    assert!(
        parecidos[..3]
            .iter()
            .all(|p| p.nome.contains("FaturaController"))
    );
    assert!(parecidos[3].nome.contains("PRC_CANCELAR_FATURA"));
}

#[test]
fn trata_pontos_em_lote_numa_gravacao_so() {
    let c = cenario();
    c.funcionalidade(
        "Cancelar fatura",
        "Cancelamento",
        &["#FrmCancelamento"],
        None,
    );
    let versao = c.mapa().versao;
    let arquivo = "db/03_pkg_cobranca.sql";
    let no_arquivo = c.pontos.iter().filter(|p| p.arquivo == arquivo).count();
    assert_eq!(no_arquivo, 5);

    let r = c.ok(
        "tratar_ponto_de_entrada",
        json!({
            "pontos": [
                // Pelo nome, em minúsculas: aplicado.
                {
                    "ponto": "pkg_cobranca.prc_cancelar_fatura",
                    "tratamento": "atribuir",
                    "funcionalidade": "F-CAN-001"
                },
                // Não existe: recusado com sugestões.
                {
                    "ponto": "db/04_pkg_situacao.sql#PKG_SITUACAO.PRC_SUSPENDER",
                    "tratamento": "justificar",
                    "motivo": "outro",
                    "justificativa": "Não é usado."
                },
                // Funcionalidade que não existe: recusado ao aplicar.
                {
                    "ponto": "PKG_SITUACAO.PRC_SUSPENDER_COBRANCA",
                    "tratamento": "atribuir",
                    "funcionalidade": "F-COB-009"
                },
                // O mesmo ponto de novo: recusado.
                {
                    "ponto": "db/03_pkg_cobranca.sql#PKG_COBRANCA.PRC_CANCELAR_FATURA",
                    "tratamento": "justificar",
                    "motivo": "outro",
                    "justificativa": "Repetido."
                },
                // Justificado: aplicado.
                {
                    "ponto": "PKG_SITUACAO.PRC_QUITAR_PARCELA",
                    "tratamento": "justificar",
                    "motivo": "codigo_morto",
                    "justificativa": "Ninguém chama."
                }
            ],
            "justificar_padrao": [
                {
                    "arquivo": arquivo,
                    "motivo": "utilitario",
                    "justificativa": "Rotinas de apoio do cálculo de encargos."
                },
                {
                    "prefixo": "db/99_nada.sql",
                    "motivo": "utilitario",
                    "justificativa": "Nada."
                }
            ]
        }),
    );
    let itens = r["pontos"].as_array().unwrap();
    let resultados: Vec<&str> = itens
        .iter()
        .map(|i| i["resultado"].as_str().unwrap())
        .collect();
    assert_eq!(
        resultados,
        ["aplicado", "recusado", "recusado", "recusado", "aplicado"],
        "{r:#}"
    );
    assert_eq!(
        itens[0]["chave"],
        "db/03_pkg_cobranca.sql#PKG_COBRANCA.PRC_CANCELAR_FATURA"
    );
    assert!(
        itens[1]["recusa"]
            .as_str()
            .unwrap()
            .contains("PKG_SITUACAO.PRC_SUSPENDER_COBRANCA"),
        "{r:#}"
    );
    assert!(
        itens[2]["recusa"]
            .as_str()
            .unwrap()
            .contains("não existe no mapa"),
        "{r:#}"
    );
    assert!(
        itens[3]["recusa"].as_str().unwrap().contains("pontos[0]"),
        "{r:#}"
    );
    let padroes = r["justificar_padrao"].as_array().unwrap();
    assert_eq!(padroes[0]["pontos"], 5);
    assert_eq!(padroes[0]["aplicados"], 4);
    assert_eq!(padroes[0]["ja_tratados"], 1);
    assert!(
        padroes[1]["recusa"]
            .as_str()
            .unwrap()
            .contains("nenhum ponto")
    );
    assert_eq!(r["aplicados"], 6);
    assert_eq!(r["recusados"], 4);

    // Uma gravação só, com tudo o que foi aplicado.
    let mapa = c.mapa();
    assert_eq!(mapa.versao, versao + 1);
    assert_eq!(
        mapa.dona_do_ponto("db/03_pkg_cobranca.sql#PKG_COBRANCA.PRC_CANCELAR_FATURA")
            .map(|f| f.id.to_string())
            .as_deref(),
        Some("F-CAN-001")
    );
    assert_eq!(mapa.pontos_justificados.len(), 5);
    let md = std::fs::read_to_string(c.amb.kb.caminho(&layout::mapa_de_funcionalidades())).unwrap();
    assert!(md.contains("`PKG_COBRANCA.FN_PARAMETRO`"), "{md}");
    assert!(md.contains("`PKG_SITUACAO.PRC_QUITAR_PARCELA`"), "{md}");
    assert_eq!(
        r["pontos_sem_tratamento_no_projeto"],
        c.pontos.len() - 2 - 5
    );

    // Nada aplicável: não grava.
    let r = c.ok(
        "tratar_ponto_de_entrada",
        json!({
            "pontos": [{
                "ponto": "PKG_COBRANCA.PRC_CANCELAR_FATURA",
                "tratamento": "justificar",
                "motivo": "outro",
                "justificativa": "Já está numa funcionalidade."
            }]
        }),
    );
    assert_eq!(r["aplicados"], 0);
    assert!(
        r["pontos"][0]["recusa"]
            .as_str()
            .unwrap()
            .contains("já está na funcionalidade F-CAN-001"),
        "{r:#}"
    );
    assert_eq!(c.mapa().versao, versao + 1);

    // Um item só continua como antes; a recusa sugere chaves parecidas.
    let m = c.recusa(
        "tratar_ponto_de_entrada",
        json!({
            "ponto": "db/04_pkg_situacao.sql#PKG_SITUACAO.PRC_SUSPENDER",
            "tratamento": "justificar",
            "motivo": "outro",
            "justificativa": "X."
        }),
    );
    assert!(m.contains("Chaves parecidas"), "{m}");
    assert!(m.contains("PRC_SUSPENDER_COBRANCA"), "{m}");
    let r = c.ok(
        "tratar_ponto_de_entrada",
        json!({
            "ponto": "pkg_situacao.prc_suspender_cobranca",
            "tratamento": "justificar",
            "motivo": "outro",
            "justificativa": "Suspensão desativada."
        }),
    );
    assert_eq!(
        r["ponto"],
        "db/04_pkg_situacao.sql#PKG_SITUACAO.PRC_SUSPENDER_COBRANCA"
    );
    let m = c.recusa(
        "tratar_ponto_de_entrada",
        json!({
            "ponto": "PKG_SITUACAO.PRC_LOG",
            "tratamento": "justificar",
            "pontos": [{ "ponto": "PKG_SITUACAO.PRC_LOG", "tratamento": "justificar" }]
        }),
    );
    assert!(m.contains("não os dois"), "{m}");
}

// ---------------------------------------------------------------------------
// Vínculo com as funcionalidades (#287)

/// Afirma com a evidência do ponto `sufixo`.
fn afirmar_com_ponto(c: &Cenario, analise: &Id, secao: &str, texto: &str, sufixo: &str) {
    c.ok(
        "afirmar",
        json!({
            "analise": analise, "secao": secao, "texto": texto, "confianca": "confirmado",
            "evidencias": [c.ponto(sufixo)["evidencia"].clone()],
        }),
    );
}

/// Conclui a análise, confirma as afirmações e aceita os termos.
fn concluir_e_aceitar(c: &Cenario, analise: &Id) {
    c.ok("concluir_analise", json!({ "analise": analise }));
    let a = c.amb.analises.carregar(analise).unwrap();
    for af in &a.afirmacoes {
        revisao::revisar(
            &c.amb,
            analise,
            Decisao::Afirmacao {
                numero: af.numero,
                estado: cba_core::analise::EstadoAfirmacao::Confirmada,
            },
        )
        .unwrap();
    }
    for t in &a.termos {
        revisao::revisar(
            &c.amb,
            analise,
            Decisao::Termo {
                numero: t.numero,
                estado: cba_core::analise::EstadoTermo::Aceito,
            },
        )
        .unwrap();
    }
}

fn frontmatter<F: cba_kb::Frontmatter>(c: &Cenario, caminho: &cba_kb::CaminhoRelativo) -> F {
    c.amb.kb.ler::<F>(caminho).unwrap().frontmatter
}

fn ids_propostos(p: &revisao::Prontidao) -> Vec<String> {
    p.funcionalidades.iter().map(|f| f.id.to_string()).collect()
}

/// Aprova uma explicação de F-CAN-001 com uma regra e um termo.
fn explicacao_aprovada(c: &Cenario) -> (Id, cba_tools::aprovacao::Aprovacao) {
    let r = c.ok(
        "iniciar_analise",
        json!({ "fluxo": "explicar", "funcionalidade": "F-CAN-001" }),
    );
    let e = Id::analisar(r["analise"].as_str().unwrap()).unwrap();
    for (secao, texto) in [
        ("objetivo", "Cancelar uma fatura emitida, com motivo."),
        (
            "fluxo_principal",
            "1. O atendente informa a fatura e o motivo.",
        ),
    ] {
        c.ok(
            "afirmar",
            json!({ "analise": e, "secao": secao, "texto": texto, "confianca": "inferido" }),
        );
    }
    afirmar_com_ponto(
        c,
        &e,
        "regras",
        "Só fatura aberta pode ser cancelada pelo atendente.",
        "PKG_COBRANCA.PRC_CANCELAR_FATURA",
    );
    c.ok(
        "propor_termo",
        json!({
            "analise": e, "termo": "Cancelamento", "definicao": "Fim da cobrança de uma fatura.",
            "evidencias": [c.ponto("PKG_COBRANCA.PRC_CANCELAR_FATURA")["evidencia"].clone()],
        }),
    );
    concluir_e_aceitar(c, &e);
    let prontidao = revisao::prontidao(&c.amb, &e).unwrap();
    assert_eq!(ids_propostos(&prontidao), ["F-CAN-001"]);
    assert_eq!(
        prontidao.origem_das_funcionalidades,
        cba_tools::vinculo::OrigemDoVinculo::Sessao
    );
    assert_eq!(prontidao.area_sugerida.as_deref(), Some("cancelamento"));
    // Só a afirmação das regras (a 3ª) vira regra.
    let opcoes = OpcoesAprovacao {
        regras: Some([3].into()),
        ..Default::default()
    };
    let aprovacao = revisao::aprovar_revisada(&c.amb, &e, &opcoes).unwrap();
    (e, aprovacao)
}

#[test]
fn explicacao_aprovada_liga_analise_regras_termos_e_negocial_a_funcionalidade() {
    use cba_kb::frontmatter::{
        FrontmatterAnalise, FrontmatterFuncionalidade, FrontmatterRegra, FrontmatterTermo,
    };
    let c = cenario();
    mapear_quase_tudo(&c);
    c.aceitar_todas();
    let (e, aprovacao) = explicacao_aprovada(&c);
    assert_eq!(aprovacao.funcionalidades, [f("F-CAN-001")]);

    let analise = c.amb.analises.carregar(&e).unwrap();
    let fa: FrontmatterAnalise = frontmatter(&c, &analise.documento);
    assert_eq!(fa.funcionalidades, [f("F-CAN-001")]);
    assert_eq!(aprovacao.regras.len(), 1);
    // A área da regra é a da funcionalidade, não `geral`.
    assert_eq!(aprovacao.regras[0].area, "cancelamento");
    let fr: FrontmatterRegra = frontmatter(&c, &aprovacao.regras[0].arquivo);
    assert_eq!(fr.funcionalidades, [f("F-CAN-001")]);
    assert_eq!(fr.id.to_string(), "RN-CAN-001");
    let ft: FrontmatterTermo = frontmatter(&c, &aprovacao.termos[0].arquivo);
    assert_eq!(ft.funcionalidades, [f("F-CAN-001")]);
    assert_eq!(ft.area.as_deref(), Some("cancelamento"));
    let negocial = aprovacao.documento_negocial.expect("documento negocial");
    let fn_: FrontmatterFuncionalidade = frontmatter(&c, &negocial);
    assert_eq!(fn_.funcionalidade, Some(f("F-CAN-001")));

    // Aprovar de novo (só confere a base) não muda nada.
    let opcoes = OpcoesAprovacao {
        regras: Some([3].into()),
        ..Default::default()
    };
    let de_novo = revisao::aprovar_revisada(&c.amb, &e, &opcoes).unwrap();
    assert!(de_novo.ja_aprovada);
    assert_eq!(de_novo.funcionalidades, [f("F-CAN-001")]);
    assert!(
        de_novo
            .regras
            .iter()
            .all(|r| r.acao == cba_tools::aprovacao::Acao::SemMudanca)
    );
}

#[test]
fn pergunta_sugere_funcionalidades_pelas_evidencias_e_o_analista_troca() {
    use cba_kb::frontmatter::{FrontmatterAnalise, FrontmatterRegra};
    let c = cenario();
    mapear_quase_tudo(&c);
    let r = c.ok(
        "iniciar_analise",
        json!({ "fluxo": "perguntar", "entrada": "Como a multa é calculada?" }),
    );
    let p = Id::analisar(r["analise"].as_str().unwrap()).unwrap();
    c.ok(
        "afirmar",
        json!({
            "analise": p, "secao": "resposta", "texto": "Pelo pacote de cobrança.",
            "confianca": "inferido",
        }),
    );
    // Encargos (F-COB-001): o serviço e o pacote; cancelamento (F-CAN-001): o pacote.
    afirmar_com_ponto(
        &c,
        &p,
        "regras_encontradas",
        "O cálculo dos encargos começa pelo serviço de encargos.",
        "EncargosService.calcular",
    );
    afirmar_com_ponto(
        &c,
        &p,
        "regras_encontradas",
        "A multa é calculada no pacote de cobrança.",
        "PKG_COBRANCA.FN_CALCULAR_MULTA",
    );
    concluir_e_aceitar(&c, &p);

    // Sem nenhuma aceita, não há sugestão: só as aceitas contam.
    let prontidao = revisao::prontidao(&c.amb, &p).unwrap();
    assert!(prontidao.funcionalidades.is_empty());
    assert_eq!(
        prontidao.origem_das_funcionalidades,
        cba_tools::vinculo::OrigemDoVinculo::Nenhuma
    );
    assert!(!prontidao.funcionalidades_do_mapa.is_empty());

    c.aceitar_todas();
    let prontidao = revisao::prontidao(&c.amb, &p).unwrap();
    assert_eq!(ids_propostos(&prontidao), ["F-COB-001", "F-CAN-001"]);
    assert_eq!(
        prontidao.origem_das_funcionalidades,
        cba_tools::vinculo::OrigemDoVinculo::Evidencias
    );
    assert_eq!(prontidao.area_sugerida.as_deref(), Some("cobranca"));

    // O analista troca pela de cancelamento: a análise e as regras levam a escolha.
    let aprovacao = revisao::aprovar_revisada(
        &c.amb,
        &p,
        &OpcoesAprovacao {
            funcionalidades: Some(vec![f("F-CAN-001"), f("F-ZZZ-999")]),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(aprovacao.funcionalidades, [f("F-CAN-001")]);
    assert!(
        aprovacao.avisos.iter().any(|a| a.contains("F-ZZZ-999")),
        "{:?}",
        aprovacao.avisos
    );
    let analise = c.amb.analises.carregar(&p).unwrap();
    let fa: FrontmatterAnalise = frontmatter(&c, &analise.documento);
    assert_eq!(fa.funcionalidades, [f("F-CAN-001")]);
    assert!(!aprovacao.regras.is_empty());
    for regra in &aprovacao.regras {
        assert_eq!(regra.area, "cancelamento");
        let fr: FrontmatterRegra = frontmatter(&c, &regra.arquivo);
        assert_eq!(fr.funcionalidades, [f("F-CAN-001")]);
    }
    // Depois de aprovada, a proposta é a gravada no documento.
    let prontidao = revisao::prontidao(&c.amb, &p).unwrap();
    assert_eq!(ids_propostos(&prontidao), ["F-CAN-001"]);
    assert_eq!(
        prontidao.origem_das_funcionalidades,
        cba_tools::vinculo::OrigemDoVinculo::Documento
    );

    // Tirar todas: a análise e as regras dela ficam sem o campo.
    let sem = revisao::aprovar_revisada(
        &c.amb,
        &p,
        &OpcoesAprovacao {
            funcionalidades: Some(Vec::new()),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(sem.funcionalidades.is_empty());
    let texto = std::fs::read_to_string(c.amb.kb.caminho(&analise.documento)).unwrap();
    assert!(!texto.contains("funcionalidades:"), "{texto}");
    for regra in &sem.regras {
        let texto = std::fs::read_to_string(c.amb.kb.caminho(&regra.arquivo)).unwrap();
        assert!(!texto.contains("funcionalidades:"), "{texto}");
    }
}

#[test]
fn religa_documentos_aprovados_antes_do_vinculo() {
    use cba_kb::frontmatter::{
        FrontmatterAnalise, FrontmatterFuncionalidade, FrontmatterRegra, FrontmatterTermo,
    };
    let c = cenario();
    mapear_quase_tudo(&c);
    c.aceitar_todas();
    let (e, aprovacao) = explicacao_aprovada(&c);
    let analise = c.amb.analises.carregar(&e).unwrap().documento;
    let regra = aprovacao.regras[0].arquivo.clone();
    let termo = aprovacao.termos[0].arquivo.clone();
    let negocial = aprovacao.documento_negocial.clone().unwrap();

    // Como numa base anterior ao #287: nenhum documento tem o vínculo.
    let kb = &c.amb.kb;
    let mut d = kb.ler::<FrontmatterAnalise>(&analise).unwrap();
    d.frontmatter.funcionalidades.clear();
    d.tirar_campo("funcionalidades");
    kb.gravar(&analise, &d).unwrap();
    let mut d = kb.ler::<FrontmatterRegra>(&regra).unwrap();
    d.frontmatter.funcionalidades.clear();
    d.tirar_campo("funcionalidades");
    kb.gravar(&regra, &d).unwrap();
    let mut d = kb.ler::<FrontmatterTermo>(&termo).unwrap();
    d.frontmatter.funcionalidades.clear();
    d.tirar_campo("funcionalidades");
    kb.gravar(&termo, &d).unwrap();
    let mut d = kb.ler::<FrontmatterFuncionalidade>(&negocial).unwrap();
    d.frontmatter.funcionalidade = None;
    d.tirar_campo("funcionalidade");
    kb.gravar(&negocial, &d).unwrap();
    let texto = std::fs::read_to_string(kb.caminho(&regra)).unwrap();
    assert!(!texto.contains("funcionalidades:"), "{texto}");

    let feita = cba_tools::vinculo::religar_projeto(&c.amb).unwrap();
    assert_eq!(feita.analises, std::slice::from_ref(&analise));
    assert_eq!(feita.regras, std::slice::from_ref(&regra));
    assert_eq!(feita.termos, std::slice::from_ref(&termo));
    assert_eq!(feita.negociais, std::slice::from_ref(&negocial));
    assert_eq!(feita.regras_sem_vinculo, 0);
    assert!(feita.avisos.is_empty(), "{:?}", feita.avisos);
    let fr: FrontmatterRegra = frontmatter(&c, &regra);
    assert_eq!(fr.funcionalidades, [f("F-CAN-001")]);
    let fn_: FrontmatterFuncionalidade = frontmatter(&c, &negocial);
    assert_eq!(fn_.funcionalidade, Some(f("F-CAN-001")));

    // Idempotente: nada a religar na segunda vez.
    let de_novo = cba_tools::vinculo::religar_projeto(&c.amb).unwrap();
    assert_eq!(de_novo.total(), 0);
}
