//! Ciclo da análise (`iniciar_analise`, `retomar_analise`, `concluir_analise`) com
//! o sistema fictício "Faturamento Exemplo".

use std::path::{Path, PathBuf};

use cba_core::ambiente::Ambiente;
use cba_core::analise::{Analise, EstadoAfirmacao, EstadoAnalise, Fluxo};
use cba_core::analises::NovaAnalise;
use cba_core::config::Config;
use cba_core::dados::DiretorioDados;
use cba_core::eventos::Mudanca;
use cba_kb::frontmatter::{
    FrontmatterAnalise, FrontmatterRegra, FrontmatterTermo, StatusAnalise, StatusRegra,
    TipoDocumento,
};
use cba_kb::{CaminhoRelativo, Documento, Id, Serie};
use cba_tools::{ErroChamada, ErroFerramenta, Saida, registro_padrao};
use serde_json::{Value, json};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/faturamento-exemplo")
}

struct Cenario {
    dir: tempfile::TempDir,
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
    }
    .gravar(&dados)
    .unwrap();
    let amb = Ambiente::do_diretorio(&fixture(), &dados).unwrap();
    Cenario { dir, amb }
}

impl Cenario {
    fn criar(&self, fluxo: Fluxo, titulo: &str, entrada: &str) -> Analise {
        self.amb
            .analises
            .criar(NovaAnalise {
                fluxo,
                titulo: titulo.into(),
                entrada: entrada.into(),
                analista: Some("Ana Exemplo".into()),
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

    fn erro(&self, nome: &str, args: Value) -> ErroFerramenta {
        self.saida(nome, args).unwrap_err()
    }

    fn analise(&self, id: &str) -> Analise {
        self.amb
            .analises
            .carregar(&Id::analisar(id).unwrap())
            .unwrap()
    }

    fn documento(&self, id: &str) -> Documento<FrontmatterAnalise> {
        self.amb.kb.ler(&self.analise(id).documento).unwrap()
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
fn iniciar_analise_criada_pelo_app_devolve_roteiro_e_monta_o_documento() {
    let c = cenario();
    let a = c.criar(
        Fluxo::Perguntar,
        "Multa por atraso",
        "Como é calculada a multa por atraso de pagamento?",
    );
    let saida = c
        .saida("iniciar_analise", json!({ "analise": "A-0001" }))
        .unwrap();
    let texto = &saida.texto;
    assert!(
        texto.contains("# Roteiro da análise A-0001: Multa por atraso"),
        "{texto}"
    );
    assert!(
        texto.contains("> Como é calculada a multa por atraso"),
        "{texto}"
    );
    assert!(
        texto.contains("`resposta` (Resposta) (obrigatória)"),
        "{texto}"
    );
    assert!(texto.contains("analise: \"A-0001\""), "{texto}");
    assert!(texto.contains("Nada relacionado na base"), "{texto}");
    assert!(!texto.contains("criada. Use"), "{texto}");
    let dados = saida.dados.unwrap();
    assert_eq!(dados["criada"], false);
    assert_eq!(dados["estado"], "em-andamento");
    assert_eq!(dados["documento"], "faturamento-exemplo/analises/A-0001.md");
    assert_eq!(dados["secoes_obrigatorias"], json!(["resposta"]));
    assert_eq!(dados["roteiro"].as_str().unwrap(), texto.as_str());

    // Estado em andamento, templates copiados e documento já no template do tipo.
    assert_eq!(c.analise("A-0001").estado, EstadoAnalise::EmAndamento);
    assert!(c.amb.base.raiz().join("_templates/pergunta.md").is_file());
    let doc = c.documento("A-0001");
    assert_eq!(doc.frontmatter.status, StatusAnalise::Rascunho);
    assert!(
        doc.corpo.contains("## Pergunta\n\nComo é calculada"),
        "{}",
        doc.corpo
    );
    assert!(doc.corpo.contains("## Resposta"), "{}", doc.corpo);
    assert_eq!(a.id.to_string(), "A-0001");
}

#[test]
fn iniciar_analise_cria_a_analise_a_partir_do_fluxo_e_da_entrada() {
    let c = cenario();
    let demanda = "Permitir parcelamento da fatura em até 3 vezes para clientes pessoa \
                   jurídica, mantendo o valor mínimo de cada parcela.\nSalvo engano, hoje \
                   não existe parcelamento.";
    let saida = c
        .saida(
            "iniciar_analise",
            json!({ "fluxo": "impacto", "entrada": demanda }),
        )
        .unwrap();
    assert!(
        saida
            .texto
            .starts_with("Análise A-0001 criada. Use analise: \"A-0001\""),
        "{}",
        saida.texto
    );
    assert!(
        saida.texto.contains("**Separe fato de suposição.**"),
        "{}",
        saida.texto
    );
    let dados = saida.dados.unwrap();
    assert_eq!(dados["analise"], "A-0001");
    assert_eq!(dados["criada"], true);
    assert_eq!(dados["fluxo"], "impacto");
    let a = c.analise("A-0001");
    assert_eq!(a.fluxo, Fluxo::Impacto);
    assert_eq!(a.entrada, demanda);
    assert!(
        a.titulo.starts_with("Permitir parcelamento"),
        "{}",
        a.titulo
    );
    assert!(a.titulo.ends_with('…'), "{}", a.titulo);
    let doc = c.documento("A-0001");
    assert_eq!(doc.frontmatter.tipo, TipoDocumento::AnaliseImpacto);
    assert!(doc.corpo.contains("## Demanda como veio"), "{}", doc.corpo);

    // Com título.
    let dados = c.chamar(
        "iniciar_analise",
        json!({ "fluxo": "explicar", "entrada": "Cancelamento de fatura", "titulo": "Cancelamento" }),
    );
    assert_eq!(dados["analise"], "A-0002");
    assert_eq!(c.analise("A-0002").titulo, "Cancelamento");
}

#[test]
fn iniciar_analise_recusa_argumentos_desconhecida_e_aprovada() {
    let c = cenario();
    for args in [
        json!({}),
        json!({ "fluxo": "perguntar" }),
        json!({ "entrada": "x" }),
        json!({ "analise": "A-0001", "fluxo": "perguntar", "entrada": "x" }),
        json!({ "analise": "A-0001", "titulo": "x" }),
        json!({ "analise": "S-0001" }),
    ] {
        match c.erro("iniciar_analise", args.clone()) {
            ErroFerramenta::Argumentos(m) => assert!(!m.is_empty()),
            outro => panic!("{args}: {outro:?}"),
        }
    }
    // Identificador desconhecido: recusa com instrução.
    match c.erro("iniciar_analise", json!({ "analise": "A-0099" })) {
        ErroFerramenta::Recusa(m) => {
            assert!(m.contains("A-0099 não existe"), "{m}");
            assert!(m.contains("liste as análises"), "{m}");
        }
        outro => panic!("{outro:?}"),
    }
    for nome in ["retomar_analise", "concluir_analise"] {
        match c.erro(nome, json!({ "analise": "A-0099" })) {
            ErroFerramenta::Recusa(m) => assert!(m.contains("A-0099 não existe"), "{m}"),
            outro => panic!("{nome}: {outro:?}"),
        }
    }

    // Aprovada: não reinicia.
    let a = c.criar(Fluxo::Perguntar, "Multa", "Como é a multa?");
    c.chamar("iniciar_analise", json!({ "analise": a.id }));
    c.chamar("afirmar", multa("A-0001", "resposta"));
    c.chamar("concluir_analise", json!({ "analise": a.id }));
    c.amb
        .analises
        .alterar(&a.id, Mudanca::Estado, |a, _| a.aprovar())
        .unwrap();
    match c.erro("iniciar_analise", json!({ "analise": a.id })) {
        ErroFerramenta::Recusa(m) => assert!(m.contains("já foi aprovada"), "{m}"),
        outro => panic!("{outro:?}"),
    }
    let r = c
        .saida("retomar_analise", json!({ "analise": a.id }))
        .unwrap();
    assert!(r.texto.contains("foi aprovada e não aceita"), "{}", r.texto);
    assert!(!r.texto.contains("# Roteiro"), "{}", r.texto);
    assert_eq!(r.dados.unwrap()["roteiro"], Value::Null);
}

#[test]
fn iniciar_analise_traz_o_contexto_da_base() {
    let c = cenario();
    let kb = &c.amb.kb;
    let regra = Serie::regra("FAT").unwrap().id(1);
    kb.gravar(
        &regra.arquivo(),
        &Documento::novo(
            FrontmatterRegra {
                tipo: TipoDocumento::Regra,
                id: regra.clone(),
                titulo: "Multa por atraso vem de parâmetro".into(),
                area: "cobranca".into(),
                status: StatusRegra::Confirmada,
                evidencias: Vec::new(),
                origem: None,
            },
            "Multa única.\n",
        ),
    )
    .unwrap();
    let outra = Serie::regra("FAT").unwrap().id(2);
    kb.gravar(
        &outra.arquivo(),
        &Documento::novo(
            FrontmatterRegra {
                tipo: TipoDocumento::Regra,
                id: outra,
                titulo: "Cancelamento exige motivo".into(),
                area: "faturamento".into(),
                status: StatusRegra::Confirmada,
                evidencias: Vec::new(),
                origem: None,
            },
            "Motivo.\n",
        ),
    )
    .unwrap();
    kb.gravar(
        &CaminhoRelativo::novo("glossario/multa.md").unwrap(),
        &Documento::novo(
            FrontmatterTermo {
                tipo: TipoDocumento::Termo,
                termo: "Multa".into(),
                sinonimos: vec!["encargo por atraso".into()],
                area: Some("cobranca".into()),
                origem: None,
            },
            "Encargo.\n",
        ),
    )
    .unwrap();
    std::fs::create_dir_all(kb.raiz().join("areas/cobranca")).unwrap();
    std::fs::write(
        kb.raiz().join("areas/cobranca/memoria.md"),
        "---\ntipo: memoria\narea: cobranca\n---\n\nNotas.\n",
    )
    .unwrap();
    let anterior = c.criar(
        Fluxo::Perguntar,
        "Juros e multa",
        "Como são os juros de atraso?",
    );
    let a = c.criar(
        Fluxo::Perguntar,
        "Multa",
        "Como é calculada a multa por atraso?",
    );

    let saida = c
        .saida("iniciar_analise", json!({ "analise": a.id }))
        .unwrap();
    let ctx = &saida.dados.as_ref().unwrap()["contexto"];
    assert_eq!(ctx["total_de_regras"], 2);
    assert_eq!(ctx["regras"].as_array().unwrap().len(), 1, "{ctx}");
    assert_eq!(ctx["regras"][0]["id"], "RN-FAT-001");
    assert_eq!(
        ctx["regras"][0]["arquivo"],
        "faturamento-exemplo/regras/RN-FAT-001.md"
    );
    assert_eq!(ctx["termos"][0]["termo"], "Multa");
    assert_eq!(ctx["memorias"][0]["area"], "cobranca");
    assert_eq!(ctx["analises"][0]["id"], anterior.id.to_string());
    assert!(
        saida
            .texto
            .contains("- RN-FAT-001 (confirmada): Multa por atraso vem de parâmetro"),
        "{}",
        saida.texto
    );
}

#[test]
fn retomar_analise_devolve_todo_o_estado_sem_alterar() {
    let c = cenario();
    let a = c.criar(
        Fluxo::Perguntar,
        "Multa",
        "Como é calculada a multa por atraso?",
    );
    c.chamar("iniciar_analise", json!({ "analise": a.id }));
    c.chamar("afirmar", multa("A-0001", "regras_encontradas"));
    let mut inferida = multa("A-0001", "resposta");
    inferida["confianca"] = json!("a_confirmar");
    inferida["texto"] = json!("O lote de cobrança pode usar outro percentual.");
    inferida["evidencias"] = json!([]);
    c.chamar("afirmar", inferida);
    c.chamar(
        "perguntar",
        json!({ "analise": "A-0001", "texto": "A multa vale para órgão público?" }),
    );
    c.amb
        .analises
        .alterar(&a.id, Mudanca::Afirmacao, |a, _| {
            a.marcar_afirmacao(
                1,
                EstadoAfirmacao::Incorreta {
                    comentario: "Faltou a carência".into(),
                },
            )
        })
        .unwrap();
    let versao = c.analise("A-0001").versao;

    let saida = c
        .saida("retomar_analise", json!({ "analise": "A-0001" }))
        .unwrap();
    assert_eq!(c.analise("A-0001").versao, versao, "retomar não grava");
    let t = &saida.texto;
    assert!(t.contains("# Estado da análise A-0001: Multa"), "{t}");
    assert!(t.contains("Estado: em-andamento"), "{t}");
    assert!(t.contains("Próximo passo: Continue de onde"), "{t}");
    assert!(
        t.contains("### Regras encontradas (`regras_encontradas`)"),
        "{t}"
    );
    assert!(
        t.contains("1. [confirmado; incorreta, segundo o analista] Passada a carência"),
        "{t}"
    );
    assert!(
        t.contains("Comentário do analista: Faltou a carência"),
        "{t}"
    );
    assert!(t.contains("`db/03_pkg_cobranca.sql:71-72`"), "{t}");
    assert!(
        t.contains("[a_confirmar; registrada] O lote de cobrança"),
        "{t}"
    );
    assert!(
        t.contains("[aberta] A multa vale para órgão público?"),
        "{t}"
    );
    assert!(t.contains("# Roteiro da análise A-0001"), "{t}");

    let d = saida.dados.unwrap();
    assert_eq!(d["entrada"], "Como é calculada a multa por atraso?");
    assert_eq!(d["afirmacoes"].as_array().unwrap().len(), 2);
    assert_eq!(d["afirmacoes"][0]["estado"], "incorreta");
    assert_eq!(d["afirmacoes"][0]["comentario"], "Faltou a carência");
    assert_eq!(d["afirmacoes"][0]["evidencias"][0]["linhas"], "71-72");
    assert_eq!(
        d["afirmacoes"][0]["evidencias"][0]["trecho"],
        "FN_PARAMETRO('PERC_MULTA_ATRASO') / 100, 2)"
    );
    assert_eq!(d["afirmacoes"][1]["estado"], "registrada");
    // a_confirmar cria pergunta; mais a registrada à parte.
    assert_eq!(d["perguntas"].as_array().unwrap().len(), 2);
    assert_eq!(d["perguntas"][1]["estado"], "aberta");
    assert_eq!(d["secoes_obrigatorias_pendentes"], json!([]));
}

#[test]
fn concluir_analise_exige_afirmacoes_e_secoes_obrigatorias() {
    let c = cenario();
    let a = c.criar(
        Fluxo::Perguntar,
        "Multa",
        "Como é calculada a multa por atraso?",
    );
    c.chamar("iniciar_analise", json!({ "analise": a.id }));
    match c.erro("concluir_analise", json!({ "analise": a.id })) {
        ErroFerramenta::Recusa(m) => {
            assert!(m.contains("ainda não tem afirmações"), "{m}");
            assert!(m.contains("resposta, regras_encontradas, exemplo"), "{m}");
        }
        outro => panic!("{outro:?}"),
    }
    c.chamar("afirmar", multa("A-0001", "regras_encontradas"));
    match c.erro("concluir_analise", json!({ "analise": a.id })) {
        ErroFerramenta::Recusa(m) => assert!(m.contains("`resposta` (Resposta)"), "{m}"),
        outro => panic!("{outro:?}"),
    }
    assert_eq!(c.analise("A-0001").estado, EstadoAnalise::EmAndamento);

    // Análise de impacto: três seções obrigatórias.
    let i = c.criar(Fluxo::Impacto, "Parcelamento", "Parcelar em 3 vezes");
    let mut args = multa(&i.id.to_string(), "como_e_hoje");
    c.chamar("afirmar", args.clone());
    match c.erro("concluir_analise", json!({ "analise": i.id })) {
        ErroFerramenta::Recusa(m) => {
            assert!(m.contains("`demanda_como_entendida`"), "{m}");
            assert!(m.contains("`abrangencia`"), "{m}");
            assert!(!m.contains("`como_e_hoje`"), "{m}");
        }
        outro => panic!("{outro:?}"),
    }
    args["secao"] = json!("demanda_como_entendida");
    c.chamar("afirmar", args.clone());
    // A abrangência começa pela estimativa, que vai para o campo do documento.
    args["secao"] = json!("abrangencia");
    match c.erro("afirmar", args.clone()) {
        ErroFerramenta::Recusa(m) => {
            assert!(m.contains("começa pela estimativa"), "{m}");
            assert!(m.contains("\"Média:\""), "{m}");
        }
        outro => panic!("{outro:?}"),
    }
    args["texto"] = json!("Pequena: só o parâmetro muda.");
    args["confianca"] = json!("inferido");
    c.chamar("afirmar", args.clone());
    c.chamar("concluir_analise", json!({ "analise": i.id }));
    let doc =
        std::fs::read_to_string(c.amb.kb.caminho(&c.analise(&i.id.to_string()).documento)).unwrap();
    assert!(doc.contains("\nabrangencia: pequena\n"), "{doc}");
    assert!(doc.contains("Estimativa: **pequena**"), "{doc}");

    // Explicar funcionalidade: objetivo, fluxo principal e regras.
    let e = c.criar(Fluxo::Explicar, "Parcelamento", "Parcelamento de fatura");
    let mut args = multa(&e.id.to_string(), "objetivo");
    c.chamar("afirmar", args.clone());
    match c.erro("concluir_analise", json!({ "analise": e.id })) {
        ErroFerramenta::Recusa(m) => {
            assert!(m.contains("`fluxo_principal` (Fluxo principal)"), "{m}");
            assert!(m.contains("`regras` (Regras de negócio)"), "{m}");
        }
        outro => panic!("{outro:?}"),
    }
    for secao in ["fluxo_principal", "regras"] {
        args["secao"] = json!(secao);
        c.chamar("afirmar", args.clone());
    }
    c.chamar("concluir_analise", json!({ "analise": e.id }));
}

#[test]
fn concluir_analise_monta_o_documento_final_e_aponta_pendencias() {
    let c = cenario();
    let a = c.criar(
        Fluxo::Perguntar,
        "Multa",
        "Como é calculada a multa por atraso?",
    );
    c.chamar("iniciar_analise", json!({ "analise": a.id }));
    c.chamar("afirmar", multa("A-0001", "resposta"));
    c.chamar("afirmar", multa("A-0001", "regras_encontradas"));
    let mut exemplo = multa("A-0001", "exemplo");
    exemplo["confianca"] = json!("inferido");
    exemplo["evidencias"] = json!([]);
    exemplo["texto"] =
        json!("Fatura de R$ 1.000,00 paga com 10 dias de atraso: multa de R$ 20,00.");
    c.chamar("afirmar", exemplo);
    c.chamar(
        "perguntar",
        json!({ "analise": "A-0001", "texto": "O lote de cobrança deveria ler os parâmetros?" }),
    );
    // Simula o código citado na afirmação 2 mudando depois do registro.
    c.amb
        .analises
        .alterar(&a.id, Mudanca::Afirmacao, |a, _| {
            a.afirmacoes[1].evidencias[0].hash = "0".repeat(64);
            Ok(())
        })
        .unwrap();

    let saida = c
        .saida("concluir_analise", json!({ "analise": "A-0001" }))
        .unwrap();
    let t = &saida.texto;
    assert!(
        t.starts_with(
            "Análise A-0001 concluída. Documento na base: \
             `faturamento-exemplo/analises/A-0001.md`"
        ),
        "{t}"
    );
    assert!(t.contains("Não copie o documento para o chat"), "{t}");
    assert!(
        t.contains("Perguntas em aberto (1):\n1. O lote de cobrança"),
        "{t}"
    );
    assert!(t.contains("afirmação 2: o código citado mudou"), "{t}");
    assert!(t.contains("afirmação 3: inferida sem evidência"), "{t}");
    assert!(!t.contains("afirmação 1:"), "{t}");
    let d = saida.dados.unwrap();
    assert_eq!(d["estado"], "concluida");
    assert_eq!(d["documento"], "faturamento-exemplo/analises/A-0001.md");
    let arquivo = PathBuf::from(d["arquivo"].as_str().unwrap());
    assert!(arquivo.is_file());
    assert!(arquivo.starts_with(c.dir.path()));
    assert_eq!(d["registros"]["afirmacoes"], 3);
    assert_eq!(d["pendencias"].as_array().unwrap().len(), 2);

    let estado = c.analise("A-0001");
    assert_eq!(estado.estado, EstadoAnalise::Concluida);
    assert_eq!(estado.afirmacoes[0].estado, EstadoAfirmacao::Registrada);
    assert_eq!(estado.afirmacoes[1].estado, EstadoAfirmacao::ARevalidar);
    let doc = c.documento("A-0001");
    assert_eq!(doc.frontmatter.status, StatusAnalise::EmRevisao);
    assert!(doc.corpo.contains("Status: em revisão"), "{}", doc.corpo);
    assert!(
        doc.corpo
            .contains("## Perguntas em aberto\n\n1. O lote de cobrança deveria ler"),
        "{}",
        doc.corpo
    );

    // Concluir de novo é aceito (rodada nova sem registros, ou repetição).
    c.chamar("concluir_analise", json!({ "analise": "A-0001" }));
    // Um registro novo reabre; iniciar não reabre.
    c.chamar("iniciar_analise", json!({ "analise": "A-0001" }));
    assert_eq!(c.analise("A-0001").estado, EstadoAnalise::Concluida);
    c.chamar(
        "perguntar",
        json!({ "analise": "A-0001", "texto": "Vale para PJ?" }),
    );
    assert_eq!(c.analise("A-0001").estado, EstadoAnalise::EmAndamento);
    let r = c
        .saida("retomar_analise", json!({ "analise": "A-0001" }))
        .unwrap();
    assert!(r.texto.contains("(mudou desde o registro)"), "{}", r.texto);
}

#[test]
fn roteiro_da_base_sobrepoe_o_padrao_em_iniciar_e_retomar() {
    let c = cenario();
    c.criar(
        Fluxo::Perguntar,
        "Multa por atraso",
        "Como é calculada a multa por atraso de pagamento?",
    );
    let base = &c.amb.base;
    let arquivo = cba_tools::roteiro::arquivo_do_roteiro(Fluxo::Perguntar);
    std::fs::create_dir_all(base.caminho(&arquivo).parent().unwrap()).unwrap();
    base.gravar_texto(
        &arquivo,
        "<!-- roteiro da casa -->\n# Roteiro da casa para {{id}}\n\n{{entrada}}\n\nSeções: {{secoes_obrigatorias}}. {{responsavel}}\n\n{{secoes}}\n\n## Contexto\n\n{{contexto}}",
    )
    .unwrap();

    let saida = c
        .saida("iniciar_analise", json!({ "analise": "A-0001" }))
        .unwrap();
    let texto = &saida.texto;
    assert!(
        texto.starts_with("# Roteiro da casa para A-0001\n\n> Como é calculada"),
        "{texto}"
    );
    assert!(
        texto.contains("Seções: `resposta`. {{responsavel}}"),
        "{texto}"
    );
    assert!(!texto.contains("roteiro da casa -->"), "{texto}");
    assert!(
        texto.contains(
            "Aviso: roteiro da base: _templates/roteiros/perguntar.md, linha 6: variável desconhecida `{{responsavel}}`"
        ),
        "{texto}"
    );
    // A cópia do padrão não sobrescreveu o roteiro editado.
    assert!(
        base.ler_texto(&arquivo)
            .unwrap()
            .starts_with("<!-- roteiro da casa -->")
    );
    let dados = saida.dados.unwrap();
    assert!(
        dados["roteiro"]
            .as_str()
            .unwrap()
            .starts_with("# Roteiro da casa para A-0001")
    );
    assert_eq!(dados["avisos"].as_array().unwrap().len(), 1);

    // retomar_analise usa o mesmo roteiro.
    let r = c
        .saida("retomar_analise", json!({ "analise": "A-0001" }))
        .unwrap();
    assert!(
        r.texto.contains("# Roteiro da casa para A-0001"),
        "{}",
        r.texto
    );
    assert!(
        r.texto.contains("variável desconhecida `{{responsavel}}`"),
        "{}",
        r.texto
    );
}

#[test]
fn prompts_prontos_da_analise_com_o_texto_da_base() {
    let c = cenario();
    c.criar(
        Fluxo::Perguntar,
        "Multa por atraso",
        "Como é calculada a multa?",
    );
    c.chamar("iniciar_analise", json!({ "analise": "A-0001" }));
    let dados = DiretorioDados::em(c.dir.path().join("dados"));
    let arquivo = cba_tools::prompts::arquivo_do_prompt("aprofundar").unwrap();
    assert!(
        c.amb.base.caminho(&arquivo).is_file(),
        "cópia editável na base"
    );
    c.amb
        .base
        .gravar_texto(&arquivo, "Aprofunde {{id}}: {{titulo}}.\nSó a resposta.\n")
        .unwrap();

    let prontos = cba_tools::prompts::da_analise(&dados, "faturamento-exemplo", "A-0001").unwrap();
    let aprofundar = prontos.iter().find(|p| p.id == "aprofundar").unwrap();
    assert_eq!(
        aprofundar.texto,
        "Aprofunde A-0001: Multa por atraso. Só a resposta."
    );
    let retomar = prontos.iter().find(|p| p.id == "retomar").unwrap();
    assert_eq!(retomar.texto, "Retome a análise A-0001 pelo ctower.");

    let erro = cba_tools::prompts::da_analise(&dados, "faturamento-exemplo", "A-0099")
        .unwrap_err()
        .to_string();
    assert!(erro.contains("A-0099"), "{erro}");
    let erro = cba_tools::prompts::da_analise(&dados, "outro", "A-0001")
        .unwrap_err()
        .to_string();
    assert!(erro.contains("`outro` não está cadastrado"), "{erro}");
    let erro = cba_tools::prompts::da_analise(&dados, "faturamento-exemplo", "S-0001")
        .unwrap_err()
        .to_string();
    assert!(erro.contains("A-NNNN"), "{erro}");
}
