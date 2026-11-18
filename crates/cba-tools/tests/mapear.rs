//! Fluxo "Mapear funcionalidades" (#208) com o sistema fictício "Faturamento
//! Exemplo": funcionalidade sem ponto de entrada válido é recusada, a conclusão
//! lista os pontos sem tratamento, a aprovação monta o catálogo e uma segunda
//! rodada mantém os identificadores; a explicação aberta a partir do catálogo
//! grava o documento negocial ligado ao item.

use std::path::{Path, PathBuf};

use cba_core::ambiente::Ambiente;
use cba_core::catalogo::{Catalogo, SituacaoNoCatalogo};
use cba_core::config::Config;
use cba_core::dados::DiretorioDados;
use cba_core::funcionalidades::DecisaoFuncionalidade;
use cba_index::PontoEntrada;
use cba_kb::{Id, IdFuncionalidade, layout};
use cba_tools::aprovacao::OpcoesAprovacao;
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

    fn funcionalidade(
        &self,
        analise: &Id,
        nome: &str,
        area: &str,
        pontos: &[&str],
        id: Option<&str>,
    ) -> Value {
        let mut args = json!({
            "analise": analise,
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

    fn iniciar_mapeamento(&self) -> Id {
        let r = self.ok("iniciar_analise", json!({ "fluxo": "mapear" }));
        Id::analisar(r["analise"].as_str().unwrap()).unwrap()
    }

    fn aceitar_todas(&self, id: &Id) {
        let a = self.amb.analises.carregar(id).unwrap();
        for f in a.funcionalidades_no_documento() {
            revisao::revisar(
                &self.amb,
                id,
                Decisao::Funcionalidade {
                    funcionalidade: f.id.clone(),
                    decisao: DecisaoFuncionalidade::Aceitar,
                },
            )
            .unwrap();
        }
    }
}

fn f(id: &str) -> IdFuncionalidade {
    IdFuncionalidade::analisar(id).unwrap()
}

/// Registra o mapa da fixture como o gabarito espera, deixando o ponto
/// `PRC_SUSPENDER_COBRANCA` de fora.
fn mapear_quase_tudo(c: &Cenario, a: &Id) {
    c.funcionalidade(
        a,
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
        a,
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
        a,
        "Cancelar fatura",
        "Cancelamento",
        &["#FrmCancelamento", "PKG_COBRANCA.PRC_CANCELAR_FATURA"],
        None,
    );
    c.funcionalidade(
        a,
        "Gerar lote mensal de cobrança",
        "Cobrança",
        &["#CALCOBR"],
        None,
    );
    c.funcionalidade(
        a,
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
            "analise": a,
            "ponto": "db/03_pkg_cobranca.sql#PKG_COBRANCA.FN_PARAMETRO",
            "tratamento": "justificar",
            "motivo": "utilitario",
            "justificativa": "Lê os parâmetros para as rotinas de cobrança e de parcelamento.",
        }),
    );
}

#[test]
fn mapeia_conclui_aprova_e_mantem_os_ids() {
    let c = cenario();
    let a = c.iniciar_mapeamento();

    // Sem ponto de entrada, ou com evidência que não confere: recusada.
    let m = c.recusa(
        "registrar_funcionalidade",
        json!({
            "analise": a, "nome": "X", "area": "Cobrança", "descricao": "X.",
            "pontos_de_entrada": []
        }),
    );
    assert!(m.contains("pontos_de_entrada"), "{m}");
    let m = c.recusa(
        "registrar_funcionalidade",
        json!({
            "analise": a, "nome": "X", "area": "Cobrança", "descricao": "X.",
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
    assert!(
        c.amb
            .analises
            .carregar(&a)
            .unwrap()
            .funcionalidades
            .is_empty()
    );

    mapear_quase_tudo(&c, &a);
    let estado = c.amb.analises.carregar(&a).unwrap();
    let ids: Vec<String> = estado
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
        estado
            .funcionalidades
            .iter()
            .all(|f| f.pontos.iter().all(|p| p.chave.is_some()))
    );

    // Falta um ponto: a conclusão lista e recusa.
    let m = c.recusa("concluir_analise", json!({ "analise": a }));
    assert!(m.contains("PKG_SITUACAO.PRC_SUSPENDER_COBRANCA"), "{m}");
    assert!(
        m.contains("1 ponto(s) de entrada sem funcionalidade"),
        "{m}"
    );
    let sem = c.ok(
        "pontos_de_entrada",
        json!({ "analise": a, "so_sem_tratamento": true }),
    );
    assert_eq!(sem["sem_tratamento"], 1);
    assert_eq!(sem["total"], 15);

    c.funcionalidade(
        &a,
        "Suspender a cobrança por contestação",
        "Cobrança",
        &["PKG_SITUACAO.PRC_SUSPENDER_COBRANCA"],
        None,
    );
    let r = c.ok("concluir_analise", json!({ "analise": a }));
    assert_eq!(r["registros"]["funcionalidades"], 6);

    // Aprovar exige decisão em cada funcionalidade.
    let pendente = revisao::aprovar_revisada(&c.amb, &a, &OpcoesAprovacao::default());
    assert!(pendente.is_err());
    c.aceitar_todas(&a);
    let aprovacao = revisao::aprovar_revisada(&c.amb, &a, &OpcoesAprovacao::default()).unwrap();
    assert!(aprovacao.avisos.is_empty(), "{:?}", aprovacao.avisos);
    let fusao = &aprovacao.catalogo.as_ref().unwrap().fusao;
    assert_eq!(fusao.novas.len(), 6);

    let catalogo = Catalogo::carregar(&c.amb.kb).unwrap();
    assert_eq!(catalogo.funcionalidades.len(), 6);
    let md = std::fs::read_to_string(c.amb.kb.caminho(&layout::catalogo())).unwrap();
    assert!(md.contains("tipo: catalogo-funcionalidades"), "{md}");
    // O catálogo entra no índice da base, com frontmatter válido, e na busca.
    let mut indice =
        cba_kb::indice::Indice::abrir(c.amb.dados.arquivo_do_indice(), &c.amb.base).unwrap();
    indice.sincronizar().unwrap();
    let achados = indice
        .buscar(&cba_kb::indice::Consulta::nova("Parcelar fatura"))
        .unwrap();
    let catalogo = achados
        .resultados
        .iter()
        .find(|r| r.caminho.as_str().ends_with("funcionalidades/catalogo.md"))
        .unwrap_or_else(|| panic!("{achados:?}"));
    assert_eq!(
        catalogo.tipo,
        Some(cba_kb::indice::Tipo::Documento(
            cba_kb::TipoDocumento::CatalogoFuncionalidades
        ))
    );
    assert!(
        md.contains("| F-CAN-001 | Cancelar fatura | só mapeada · nova | 2 |"),
        "{md}"
    );
    assert!(md.contains("#### F-PAR-001 · Parcelar fatura"), "{md}");
    let documento = std::fs::read_to_string(c.amb.kb.caminho(&estado.documento)).unwrap();
    assert!(
        documento.contains("| `PKG_COBRANCA.FN_PARAMETRO` | rotina de banco"),
        "{documento}"
    );

    // Segunda rodada: sem `id`, o ponto em comum devolve o identificador do
    // catálogo; com `id`, o nome muda e o identificador fica.
    let b = c.iniciar_mapeamento();
    let r = c.funcionalidade(
        &b,
        "Cancelar uma fatura emitida",
        "Cancelamento",
        &["#FrmCancelamento"],
        None,
    );
    assert_eq!(r["funcionalidade"], "F-CAN-001");
    assert_eq!(r["do_catalogo"], true);
    let r = c.funcionalidade(
        &b,
        "Parcelamento da fatura",
        "Parcelamento",
        &["ParcelamentoService.parcelar"],
        Some("F-PAR-001"),
    );
    assert_eq!(r["funcionalidade"], "F-PAR-001");
    // Um ponto que o catálogo já atribui devolve o identificador dele.
    let r = c.funcionalidade(&b, "Lote de cobrança", "Cobrança", &["#CALCOBR"], None);
    assert_eq!(r["funcionalidade"], "F-COB-002");
    // Funcionalidade nova continua a numeração da sigla; um ponto fora da lista
    // do mapa entra só com a evidência.
    let r = c.ok(
        "registrar_funcionalidade",
        json!({
            "analise": b, "nome": "Limitar encargos por tipo de cliente", "area": "Cobrança",
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
    let retomada = c.ok("retomar_analise", json!({ "analise": b }));
    assert_eq!(
        retomada["funcionalidades"]["catalogo"]
            .as_array()
            .unwrap()
            .len(),
        6
    );
}

#[test]
fn explicacao_do_catalogo_grava_o_documento_negocial_ligado() {
    let c = cenario();
    let a = c.iniciar_mapeamento();
    mapear_quase_tudo(&c, &a);
    c.funcionalidade(
        &a,
        "Suspender a cobrança",
        "Cobrança",
        &["PKG_SITUACAO.PRC_SUSPENDER_COBRANCA"],
        None,
    );
    c.ok("concluir_analise", json!({ "analise": a }));
    c.aceitar_todas(&a);
    revisao::aprovar_revisada(&c.amb, &a, &OpcoesAprovacao::default()).unwrap();

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
    let catalogo = Catalogo::carregar(&c.amb.kb).unwrap();
    let item = catalogo.item(&f("F-CAN-001")).unwrap();
    assert_eq!(item.documento.as_ref(), Some(&negocial));
    assert_eq!(item.explicacao.as_ref(), Some(&e));
    assert_eq!(item.situacao, SituacaoNoCatalogo::Nova);
    let md = std::fs::read_to_string(c.amb.kb.caminho(&layout::catalogo())).unwrap();
    assert!(md.contains("documentada"), "{md}");
}
