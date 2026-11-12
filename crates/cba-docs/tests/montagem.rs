//! Montagem de cada tipo de documento com registros do sistema fictício
//! "Faturamento Exemplo" (`fixtures/gabaritos/faturamento-exemplo.yaml`),
//! comparada com os textos esperados em `tests/esperados/`.
//!
//! Para atualizar os textos esperados depois de mudar um template ou a montagem:
//! `CBA_ATUALIZAR_ESPERADOS=1 cargo test -p cba-docs`, e revise o diff.

use std::fs;
use std::path::{Path, PathBuf};

use cba_docs::{
    Afirmacao, Aviso, Citacao, Confianca, Conteudo, ErroMontagem, Montavel, Origem, Pergunta,
    ReferenciaSugestao, Templates, Texto, arquivo_do_template, bloco_de_codigo,
};
use cba_kb::frontmatter::{
    Abrangencia, Area, Data, Demanda, Evidencia, FrontmatterAnalise, FrontmatterFuncionalidade,
    FrontmatterLivre, FrontmatterProjeto, FrontmatterRegra, FrontmatterSugestao, FrontmatterTermo,
    Linhas, StatusAnalise, StatusRegra, StatusSugestao, VersaoCodigo,
};
use cba_kb::yaml::Value;
use cba_kb::{Base, CaminhoRelativo, Documento, Id, Serie, TipoDocumento, layout};
use serde::Deserialize;

const PROJETO: &str = "faturamento-exemplo";

// ---------------------------------------------------------------------------
// Gabarito do sistema fictício

#[derive(Deserialize)]
struct Gabarito {
    regras: Vec<RegraG>,
    inconsistencias: Vec<RegraG>,
    perguntas: Vec<PerguntaG>,
    funcionalidades: Vec<FuncionalidadeG>,
    demandas: Vec<DemandaG>,
}

#[derive(Deserialize)]
struct RegraG {
    id: String,
    titulo: String,
    #[serde(default)]
    area: Option<String>,
    enunciado: String,
    evidencias: Vec<EvidenciaG>,
}

#[derive(Deserialize)]
struct EvidenciaG {
    arquivo: String,
    linha_inicial: u32,
    linha_final: u32,
}

#[derive(Deserialize)]
struct PerguntaG {
    id: String,
    pergunta: String,
    resposta_esperada: String,
    regras: Vec<String>,
}

#[derive(Deserialize)]
struct FuncionalidadeG {
    id: String,
    nome: String,
    objetivo: String,
    regras: Vec<String>,
}

#[derive(Deserialize)]
struct DemandaG {
    id: String,
    texto: String,
    premissa: String,
    regras: Vec<String>,
    arquivos_impactados: Vec<String>,
}

fn raiz_do_repositorio() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn gabarito() -> Gabarito {
    let texto = fs::read_to_string(
        raiz_do_repositorio().join("fixtures/gabaritos/faturamento-exemplo.yaml"),
    )
    .unwrap();
    serde_norway::from_str(&texto).unwrap()
}

impl Gabarito {
    /// Regra ou inconsistência pelo id.
    fn regra(&self, id: &str) -> &RegraG {
        self.regras
            .iter()
            .chain(&self.inconsistencias)
            .find(|r| r.id == id)
            .unwrap_or_else(|| panic!("{id} não está no gabarito"))
    }

    fn pergunta(&self, id: &str) -> &PerguntaG {
        self.perguntas.iter().find(|p| p.id == id).unwrap()
    }

    fn funcionalidade(&self, id: &str) -> &FuncionalidadeG {
        self.funcionalidades.iter().find(|f| f.id == id).unwrap()
    }

    fn demanda(&self, id: &str) -> &DemandaG {
        self.demandas.iter().find(|d| d.id == id).unwrap()
    }
}

impl RegraG {
    fn citacoes(&self) -> Vec<Citacao> {
        self.evidencias
            .iter()
            .map(|e| {
                Citacao::nova(
                    c(&e.arquivo),
                    Linhas::novo(e.linha_inicial, e.linha_final).unwrap(),
                )
            })
            .collect()
    }

    /// Afirmação com o enunciado da regra e as evidências do gabarito.
    fn afirmacao(&self, secao: &str, confianca: Confianca) -> Afirmacao {
        Afirmacao {
            secao: secao.into(),
            texto: format!("{} ({}): {}", self.titulo, self.id, self.enunciado),
            confianca,
            evidencias: self.citacoes(),
        }
    }
}

// ---------------------------------------------------------------------------
// Apoio

fn c(caminho: &str) -> CaminhoRelativo {
    CaminhoRelativo::novo(caminho).unwrap()
}

fn data() -> Data {
    Data::nova(2026, 11, 11).unwrap()
}

fn analise(tipo: TipoDocumento, numero: u32, titulo: &str) -> FrontmatterAnalise {
    FrontmatterAnalise {
        tipo,
        id: Serie::Analise.id(numero),
        titulo: titulo.into(),
        projeto: PROJETO.into(),
        demanda: Demanda {
            origem: "texto".into(),
            chave: None,
        },
        status: StatusAnalise::Rascunho,
        abrangencia: None,
        base: Some(VersaoCodigo {
            branch: Some("develop".into()),
            commit: Some("1a2b3c4d".into()),
        }),
        criado: data(),
        analista: Some("Ana Exemplo".into()),
    }
}

/// Monta com o conjunto padrão, confere que não há avisos e que o documento
/// volta igual pela leitura de cba-kb, e compara com o texto esperado.
fn montar_e_conferir<F: Montavel + PartialEq + std::fmt::Debug>(
    nome: &str,
    conteudo: &Conteudo<F>,
) -> String {
    let montado = Templates::padrao().montar(conteudo).unwrap();
    assert!(montado.avisos.is_empty(), "{nome}: {:?}", montado.avisos);
    let texto = montado.documento.para_texto().unwrap();

    // Frontmatter válido: lido de volta pelo tipo e pelo leitor genérico.
    let lido: Documento<F> = Documento::analisar(&texto).unwrap();
    assert_eq!(lido.frontmatter, conteudo.frontmatter);
    let livre: Documento<FrontmatterLivre> = Documento::analisar(&texto).unwrap();
    assert_eq!(livre.frontmatter.tipo(), Some(conteudo.frontmatter.tipo()));

    conferir_caminhos_relativos(&texto);

    // Determinístico.
    let de_novo = Templates::padrao().montar(conteudo).unwrap();
    assert_eq!(de_novo.documento.para_texto().unwrap(), texto);

    conferir_esperado(nome, &texto);
    texto
}

/// Nenhum caminho absoluto nos links, nas notas e no frontmatter.
fn conferir_caminhos_relativos(texto: &str) {
    let livre: Documento<FrontmatterLivre> = Documento::analisar(texto).unwrap();
    fn valores(v: &Value, saida: &mut Vec<String>) {
        match v {
            Value::String(s) => saida.push(s.clone()),
            Value::Sequence(l) => l.iter().for_each(|x| valores(x, saida)),
            Value::Mapping(m) => m.values().for_each(|x| valores(x, saida)),
            _ => {}
        }
    }
    let mut textos = Vec::new();
    valores(&Value::Mapping(livre.frontmatter.0.clone()), &mut textos);
    for s in textos.iter().filter(|s| s.contains('/')) {
        assert!(CaminhoRelativo::novo(s).is_ok(), "caminho absoluto: {s}");
    }
    let raiz = raiz_do_repositorio().canonicalize().unwrap();
    assert!(!texto.contains(&*raiz.to_string_lossy()));
    for destino in texto.split("](").skip(1) {
        let destino = destino.split(')').next().unwrap();
        let destino = destino.trim_start_matches('<').trim_end_matches('>');
        assert!(
            !destino.starts_with('/') && !destino.contains(':') && !destino.contains('\\'),
            "link não relativo: {destino}"
        );
    }
    for nota in texto
        .lines()
        .filter(|l| l.starts_with("[^") && l.contains("]: "))
    {
        let local = nota.split('`').nth(1).unwrap();
        let arquivo = local.rsplit_once(':').unwrap().0;
        assert!(
            CaminhoRelativo::novo(arquivo).is_ok(),
            "nota não relativa: {nota}"
        );
    }
}

fn conferir_esperado(nome: &str, texto: &str) {
    let caminho = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/esperados")
        .join(format!("{nome}.md"));
    if std::env::var_os("CBA_ATUALIZAR_ESPERADOS").is_some() {
        fs::create_dir_all(caminho.parent().unwrap()).unwrap();
        fs::write(&caminho, texto).unwrap();
        return;
    }
    let esperado = fs::read_to_string(&caminho)
        .unwrap_or_else(|e| panic!("{}: {e}", caminho.display()))
        .replace("\r\n", "\n");
    assert!(
        texto == esperado,
        "{nome}: o documento montado difere de {}\n--- montado ---\n{texto}",
        caminho.display()
    );
}

// ---------------------------------------------------------------------------
// Conteúdos de exemplo, um por tipo

fn conteudo_pergunta(g: &Gabarito) -> Conteudo<FrontmatterAnalise> {
    let p = g.pergunta("P-FAT-001");
    let mut conteudo = Conteudo::novo(
        analise(TipoDocumento::Pergunta, 1, "Cálculo da multa por atraso"),
        c("analises/A-0001.md"),
    );
    conteudo.textos.push(Texto::novo("pergunta", &p.pergunta));
    conteudo
        .textos
        .push(Texto::novo("resposta", &p.resposta_esperada));
    for id in &p.regras {
        let regra = g.regra(id);
        let confianca = if id.starts_with("INC-") {
            Confianca::Inferido
        } else {
            Confianca::Confirmado
        };
        conteudo
            .afirmacoes
            .push(regra.afirmacao("regras_encontradas", confianca));
    }
    conteudo.textos.push(Texto::novo(
        "exemplo",
        "Fatura de R$ 1.000,00 de pessoa jurídica, paga com 10 dias de atraso:\n\n\
         | Item | Valor |\n|---|---|\n| Multa (2%) | R$ 20,00 |\n\
         | Juros (0,033% x 10 dias) | R$ 3,30 |\n| Total de encargos | R$ 23,30 |",
    ));
    conteudo.perguntas.push(Pergunta {
        texto: "O lote de cobrança deveria ler os parâmetros em vez de usar valores fixos?".into(),
    });
    conteudo
}

fn conteudo_analise_impacto(g: &Gabarito) -> Conteudo<FrontmatterAnalise> {
    let d = g.demanda("D-FAT-001");
    let mut fm = analise(
        TipoDocumento::AnaliseImpacto,
        2,
        "Parcelamento da fatura para pessoa jurídica",
    );
    fm.abrangencia = Some(Abrangencia::Pequena);
    let mut conteudo = Conteudo::novo(fm, c("analises/A-0002.md"));
    conteudo
        .textos
        .push(Texto::novo("demanda_como_veio", &d.texto));
    conteudo
        .textos
        .push(Texto::novo("demanda_como_entendida", &d.premissa));
    for id in &d.regras {
        let regra = g.regra(id);
        if id.starts_with("INC-") {
            conteudo
                .afirmacoes
                .push(regra.afirmacao("riscos", Confianca::Confirmado));
        } else {
            conteudo
                .afirmacoes
                .push(regra.afirmacao("como_e_hoje", Confianca::Confirmado));
        }
    }
    conteudo.afirmacoes.push(Afirmacao {
        secao: "funcionalidades".into(),
        texto: "Parcelamento de fatura (F-FAT-002): só o limite de parcelas muda, se o \
                objetivo for limitar a 3."
            .into(),
        confianca: Confianca::Inferido,
        evidencias: g.regra("RN-FAT-008").citacoes(),
    });
    conteudo.afirmacoes.push(Afirmacao {
        secao: "dados".into(),
        texto: "Nenhuma tabela muda; o limite vem do parâmetro MAX_PARCELAS.".into(),
        confianca: Confianca::Inferido,
        evidencias: Vec::new(),
    });
    conteudo.afirmacoes.push(Afirmacao {
        secao: "documentos_e_telas".into(),
        texto: format!(
            "Arquivos que mudariam: {}.",
            d.arquivos_impactados
                .iter()
                .map(|a| format!("`{a}`"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        confianca: Confianca::AConfirmar,
        evidencias: Vec::new(),
    });
    conteudo.textos.push(Texto::novo(
        "abrangencia",
        "Pequena: o parcelamento já existe; a mudança se limita a um parâmetro e à tela.",
    ));
    conteudo.perguntas.push(Pergunta {
        texto: "O limite de 3 parcelas vale também para órgão público?".into(),
    });
    conteudo.perguntas.push(Pergunta {
        texto: "A tela deve deixar de oferecer até 12 parcelas?\nHoje o backend recusa acima de 6."
            .into(),
    });
    conteudo.sugestoes.push(ReferenciaSugestao {
        id: Serie::Sugestao.id(1),
        titulo: "Limitar a tela ao parâmetro MAX_PARCELAS".into(),
        arquivo: Some(c("frontend/src/telas/parcelamento-form.ts")),
    });
    conteudo
}

fn conteudo_funcionalidade(g: &Gabarito) -> Conteudo<FrontmatterFuncionalidade> {
    let f = g.funcionalidade("F-FAT-002");
    let fm = FrontmatterFuncionalidade {
        tipo: TipoDocumento::Funcionalidade,
        titulo: f.nome.clone(),
        area: "parcelamento".into(),
        tema: "parcelamento-de-fatura".into(),
        status: StatusAnalise::EmRevisao,
        origem: Some(c("analises/A-0003.md")),
    };
    let caminho = layout::funcionalidade("parcelamento", "parcelamento-de-fatura")
        .unwrap()
        .juntar("negocial.md")
        .unwrap();
    let mut conteudo = Conteudo::novo(fm, caminho);
    conteudo.textos.push(Texto::novo("objetivo", &f.objetivo));
    conteudo.afirmacoes.push(Afirmacao {
        secao: "atores_e_telas".into(),
        texto: "Atendente, pela tela de parcelamento.".into(),
        confianca: Confianca::Confirmado,
        evidencias: vec![Citacao::nova(
            c("frontend/src/telas/parcelamento-form.ts"),
            Linhas::novo(19, 19).unwrap(),
        )],
    });
    for id in &f.regras {
        let secao = if id.starts_with("INC-") {
            "excecoes"
        } else {
            "regras"
        };
        conteudo
            .afirmacoes
            .push(g.regra(id).afirmacao(secao, Confianca::Confirmado));
    }
    conteudo.perguntas.push(Pergunta {
        texto: "Qual dos limites de parcelas (tela ou backend) é o correto?".into(),
    });
    conteudo
}

fn conteudo_caso_de_uso(g: &Gabarito) -> Conteudo<FrontmatterFuncionalidade> {
    let f = g.funcionalidade("F-FAT-003");
    let fm = FrontmatterFuncionalidade {
        tipo: TipoDocumento::CasoDeUso,
        titulo: f.nome.clone(),
        area: "cancelamento".into(),
        tema: "cancelamento-de-fatura".into(),
        status: StatusAnalise::Rascunho,
        origem: None,
    };
    let caminho = layout::funcionalidade("cancelamento", "cancelamento-de-fatura")
        .unwrap()
        .juntar("uc-cancelar-fatura.md")
        .unwrap();
    let mut conteudo = Conteudo::novo(fm, caminho);
    conteudo.textos.push(Texto::novo("objetivo", &f.objetivo));
    conteudo.textos.push(Texto::novo(
        "gatilho",
        "O atendente pede o cancelamento de uma fatura emitida.",
    ));
    conteudo.afirmacoes.push(
        g.regra("RN-FAT-012")
            .afirmacao("pre_condicoes", Confianca::Confirmado),
    );
    conteudo.textos.push(Texto::novo(
        "fluxo_principal",
        "1. O atendente informa a fatura e o motivo.\n2. O projeto confere as regras.\n\
         3. A fatura é cancelada com o responsável registrado.",
    ));
    conteudo.afirmacoes.push(
        g.regra("RN-FAT-014")
            .afirmacao("fluxos_alternativos", Confianca::Confirmado),
    );
    conteudo.afirmacoes.push(
        g.regra("RN-FAT-013")
            .afirmacao("excecoes", Confianca::Confirmado),
    );
    for id in &f.regras {
        conteudo
            .afirmacoes
            .push(g.regra(id).afirmacao("regras", Confianca::Confirmado));
    }
    conteudo
}

fn conteudo_regra(g: &Gabarito) -> Conteudo<FrontmatterRegra> {
    let r = g.regra("RN-FAT-001");
    let evidencias = r
        .evidencias
        .iter()
        .enumerate()
        .map(|(i, e)| Evidencia {
            arquivo: c(&e.arquivo),
            linhas: Linhas::novo(e.linha_inicial, e.linha_final).unwrap(),
            hash: format!("hash-ficticio-{}", i + 1),
            commit: Some("1a2b3c4d".into()),
        })
        .collect();
    let fm = FrontmatterRegra {
        tipo: TipoDocumento::Regra,
        id: Id::analisar(&r.id).unwrap(),
        titulo: r.titulo.clone(),
        area: r.area.clone().unwrap(),
        status: StatusRegra::Confirmada,
        evidencias,
        origem: Some(c("analises/A-0001.md")),
    };
    Conteudo::da_regra(fm, &r.enunciado)
}

fn conteudo_termo() -> Conteudo<FrontmatterTermo> {
    let fm = FrontmatterTermo {
        tipo: TipoDocumento::Termo,
        termo: "Carência".into(),
        sinonimos: vec!["prazo de tolerância".into(), "dias de carência".into()],
        area: Some("cobranca".into()),
        origem: Some(c("analises/A-0001.md")),
    };
    let caminho = layout::termo("Carência").unwrap();
    let mut conteudo = Conteudo::novo(fm, caminho);
    conteudo.textos.push(Texto::novo(
        "definicao",
        "Dias corridos após o vencimento em que o atraso não gera multa nem juros.",
    ));
    conteudo.afirmacoes.push(Afirmacao {
        secao: "onde_vive_no_codigo".into(),
        texto: "Parâmetro `DIAS_CARENCIA` na tabela `PARAMETRO`.".into(),
        confianca: Confianca::Confirmado,
        evidencias: vec![Citacao::nova(
            c("db/02_parametros.sql"),
            Linhas::novo(12, 13).unwrap(),
        )],
    });
    conteudo.afirmacoes.push(Afirmacao {
        secao: "onde_vive_no_codigo".into(),
        texto: "Pacote de cobrança, nos cálculos de multa e de juros.".into(),
        confianca: Confianca::Confirmado,
        evidencias: vec![
            Citacao::nova(c("db/03_pkg_cobranca.sql"), Linhas::novo(66, 69).unwrap()),
            Citacao::nova(c("db/03_pkg_cobranca.sql"), Linhas::novo(90, 92).unwrap()),
        ],
    });
    conteudo
}

fn conteudo_sugestao() -> Conteudo<FrontmatterSugestao> {
    let id = Serie::Sugestao.id(1);
    let fm = FrontmatterSugestao {
        tipo: TipoDocumento::Sugestao,
        id: id.clone(),
        titulo: "Ler a multa do parâmetro no lote de cobrança".into(),
        projeto: PROJETO.into(),
        status: StatusSugestao::Proposta,
        analise: Some(Serie::Analise.id(4)),
        arquivo: Some(c("cobol/CALCOBR.cbl")),
        criado: data(),
    };
    let atual = Citacao::nova(c("cobol/CALCOBR.cbl"), Linhas::novo(38, 40).unwrap());
    let mut conteudo = Conteudo::novo(fm, id.arquivo());
    conteudo.textos.push(Texto::novo(
        "contexto",
        "A demanda reduz a multa de 2% para 1% alterando o parâmetro PERC_MULTA_ATRASO.",
    ));
    conteudo.textos.push(Texto {
        secao: "trecho_atual".into(),
        texto: bloco_de_codigo(
            "       01  WS-PERC-MULTA           PIC 9V9999     VALUE 2.0000.",
            "cobol",
        ),
        citacoes: vec![atual],
    });
    conteudo.textos.push(Texto::novo(
        "trecho_proposto",
        bloco_de_codigo(
            "       01  WS-PERC-MULTA           PIC 9V9999.\n      * Lido de PARAMETRO no início do lote.",
            "cobol",
        ),
    ));
    conteudo.textos.push(Texto::novo(
        "motivo",
        "O lote usa 2% fixo e continuaria cobrando a multa antiga.",
    ));
    conteudo.afirmacoes.push(Afirmacao {
        secao: "impacto".into(),
        texto: "O lote passa a seguir o parâmetro, como o banco e o backend.".into(),
        confianca: Confianca::Inferido,
        evidencias: vec![Citacao::nova(
            c("db/03_pkg_cobranca.sql"),
            Linhas::novo(71, 72).unwrap(),
        )],
    });
    conteudo.textos.push(Texto::novo(
        "exemplo",
        "Fatura de R$ 1.000,00 com 10 dias de atraso: hoje o lote cobra R$ 20,00 de multa; \
         com o parâmetro em 1%, cobraria R$ 10,00.",
    ));
    conteudo
}

// ---------------------------------------------------------------------------
// Um teste de montagem por tipo

#[test]
fn monta_pergunta() {
    let texto = montar_e_conferir("pergunta", &conteudo_pergunta(&gabarito()));
    // RN-FAT-003 cita duas vezes o mesmo arquivo em intervalos diferentes; cada
    // intervalo é uma nota.
    assert!(texto.contains("[^1]: `db/03_pkg_cobranca.sql:71-72`"));
    assert!(texto.contains("**[inferido]** Lote COBOL"));
}

#[test]
fn monta_analise_de_impacto() {
    let texto = montar_e_conferir("analise-impacto", &conteudo_analise_impacto(&gabarito()));
    assert!(texto.contains("Estimativa: **pequena**"));
    assert!(
        texto.contains(
            "[S-0001 · Limitar a tela ao parâmetro MAX_PARCELAS](../sugestoes/S-0001.md)"
        )
    );
}

#[test]
fn monta_documento_negocial() {
    let texto = montar_e_conferir("funcionalidade", &conteudo_funcionalidade(&gabarito()));
    // O template padrão não mostra a origem; o campo existe para templates editados.
    assert!(texto.starts_with("---\ntipo: funcionalidade\n"));
    assert!(texto.contains("**[confirmado]** Atendente, pela tela de parcelamento.[^1]"));
}

#[test]
fn monta_documento_negocial_de_uma_analise() {
    // O fluxo "explicar funcionalidade" grava em analises/ com o frontmatter da
    // análise; os campos que ele não tem saem como "—".
    let g = gabarito();
    let f = g.funcionalidade("F-FAT-001");
    let mut conteudo = Conteudo::novo(
        analise(TipoDocumento::Funcionalidade, 5, &f.nome),
        c("analises/A-0005.md"),
    );
    conteudo.textos.push(Texto::novo("objetivo", &f.objetivo));
    for id in &f.regras {
        conteudo
            .afirmacoes
            .push(g.regra(id).afirmacao("regras", Confianca::Confirmado));
    }
    montar_e_conferir("funcionalidade-de-analise", &conteudo);
}

#[test]
fn monta_caso_de_uso() {
    montar_e_conferir("caso-de-uso", &conteudo_caso_de_uso(&gabarito()));
}

#[test]
fn monta_regra() {
    let texto = montar_e_conferir("regra", &conteudo_regra(&gabarito()));
    assert!(texto.contains(
        "[^1]: `db/03_pkg_cobranca.sql:71-72` · hash `hash-ficticio-1` · commit `1a2b3c4d`"
    ));
    assert!(texto.contains("[analises/A-0001.md](../analises/A-0001.md)"));
}

#[test]
fn monta_termo() {
    montar_e_conferir("termo", &conteudo_termo());
}

#[test]
fn monta_sugestao() {
    let texto = montar_e_conferir("sugestao", &conteudo_sugestao());
    // A nota do trecho atual fica fora do bloco de código.
    assert!(texto.contains("VALUE 2.0000.\n```\n\n[^1]"));
    assert!(texto.contains("[A-0004](../analises/A-0004.md)"));
}

#[test]
fn documento_vazio_tem_todas_as_secoes() {
    let conteudo = Conteudo::novo(
        analise(TipoDocumento::AnaliseImpacto, 9, "Sem registros ainda"),
        c("analises/A-0009.md"),
    );
    montar_e_conferir("analise-impacto-vazia", &conteudo);
}

// ---------------------------------------------------------------------------
// Templates da base, avisos e erros

fn base_com_projeto() -> (tempfile::TempDir, Base, cba_kb::Projeto) {
    let dir = tempfile::tempdir().unwrap();
    let base = Base::criar(dir.path().join("Base de Conhecimento – Análises")).unwrap();
    let projeto = base
        .criar_projeto(&Documento::novo(
            FrontmatterProjeto {
                tipo: TipoDocumento::Projeto,
                id: PROJETO.into(),
                nome: "Faturamento Exemplo".into(),
                areas: vec![Area {
                    id: "cobranca".into(),
                    nome: "Cobrança".into(),
                    sigla: Some("FAT".into()),
                }],
                criado: Some(data()),
            },
            "\n# Faturamento Exemplo\n",
        ))
        .unwrap();
    (dir, base, projeto)
}

#[test]
fn template_editado_na_base_substitui_o_padrao() {
    let (_dir, base, _) = base_com_projeto();
    let criados = Templates::copiar_padrao(&base).unwrap();
    assert_eq!(criados.len(), 7);
    assert!(criados.contains(&c("_templates/regra.md")));

    // O analista edita o template da regra (no Windows, com CRLF).
    let editado = "# Regra {{id}}\r\n\r\n> {{enunciado}}\r\n\r\nFontes:\r\n\r\n{{evidencias}}\r\n";
    base.gravar_texto(&arquivo_do_template(TipoDocumento::Regra), editado)
        .unwrap();
    // Copiar de novo não sobrescreve o que foi editado.
    assert!(Templates::copiar_padrao(&base).unwrap().is_empty());

    let (templates, avisos) = Templates::da_base(&base);
    assert!(avisos.is_empty(), "{avisos:?}");
    let t = templates.template(TipoDocumento::Regra).unwrap();
    assert_eq!(t.origem(), &Origem::Base(c("_templates/regra.md")));
    assert_eq!(
        templates.template(TipoDocumento::Termo).unwrap().origem(),
        &Origem::Base(c("_templates/termo.md"))
    );

    let montado = templates.montar(&conteudo_regra(&gabarito())).unwrap();
    assert!(montado.avisos.is_empty());
    assert_eq!(
        montado.documento.corpo,
        "\n# Regra RN-FAT-001\n\n> Passada a carência, a fatura em atraso recebe multa única de \
         PERC_MULTA_ATRASO por cento (hoje 2%) sobre o valor original, lida da tabela PARAMETRO \
         e arredondada em 2 casas.[^1][^2]\n\nFontes:\n\n\
         [^1]: `db/03_pkg_cobranca.sql:71-72` · hash `hash-ficticio-1` · commit `1a2b3c4d`\n\
         [^2]: `db/02_parametros.sql:6-7` · hash `hash-ficticio-2` · commit `1a2b3c4d`\n"
    );
}

#[test]
fn sem_template_na_base_usa_o_padrao() {
    let (_dir, base, _) = base_com_projeto();
    let (templates, avisos) = Templates::da_base(&base);
    assert!(avisos.is_empty());
    assert_eq!(templates, Templates::padrao());
}

#[test]
fn marcador_desconhecido_no_template_da_base_gera_aviso() {
    let (_dir, base, _) = base_com_projeto();
    base.gravar_texto(
        &arquivo_do_template(TipoDocumento::Termo),
        "# {{termo}}\n\n{{definicao}}\n\nResponsável: {{responsavel}}\n\n{{evidencias}}\n",
    )
    .unwrap();
    let (templates, avisos) = Templates::da_base(&base);
    assert_eq!(
        avisos,
        vec![Aviso::MarcadorDesconhecido {
            tipo: TipoDocumento::Termo,
            origem: Origem::Base(c("_templates/termo.md")),
            marcador: "responsavel".into(),
            linha: 5,
        }]
    );
    assert_eq!(
        avisos[0].to_string(),
        "_templates/termo.md, linha 5: marcador desconhecido `{{responsavel}}` no template de termo"
    );

    // O marcador fica no texto; a seção sem marcador vai para o fim, com aviso.
    let montado = templates.montar(&conteudo_termo()).unwrap();
    assert_eq!(
        montado.avisos,
        vec![Aviso::SecaoSemMarcador {
            tipo: TipoDocumento::Termo,
            secao: "onde_vive_no_codigo".into(),
        }]
    );
    let corpo = &montado.documento.corpo;
    assert!(corpo.contains("Responsável: {{responsavel}}"));
    let fim = corpo.find("## Onde vive no código").unwrap();
    assert!(
        fim > corpo.find("[^1]: ").unwrap(),
        "seção no fim do documento"
    );
    assert!(corpo[fim..].contains("**[confirmado]** Parâmetro `DIAS_CARENCIA`"));
}

#[test]
fn template_ilegivel_usa_o_padrao_com_aviso() {
    let (_dir, base, _) = base_com_projeto();
    fs::create_dir_all(base.templates()).unwrap();
    fs::write(base.templates().join("pergunta.md"), [0xff, 0xfe, 0x00]).unwrap();
    let (templates, avisos) = Templates::da_base(&base);
    assert!(matches!(
        &avisos[..],
        [Aviso::TemplateIlegivel {
            tipo: TipoDocumento::Pergunta,
            ..
        }]
    ));
    assert_eq!(
        templates
            .template(TipoDocumento::Pergunta)
            .unwrap()
            .origem(),
        &Origem::Padrao
    );
}

#[test]
fn registro_em_secao_que_o_tipo_nao_tem_vai_para_o_fim() {
    let mut conteudo = conteudo_termo();
    conteudo.afirmacoes.push(Afirmacao {
        secao: "historico".into(),
        texto: "Já foi de 5 dias.".into(),
        confianca: Confianca::AConfirmar,
        evidencias: Vec::new(),
    });
    let montado = Templates::padrao().montar(&conteudo).unwrap();
    assert_eq!(
        montado.avisos,
        vec![Aviso::SecaoDesconhecida {
            tipo: TipoDocumento::Termo,
            secao: "historico".into(),
        }]
    );
    assert!(
        montado
            .documento
            .corpo
            .ends_with("## historico\n\n- **[a confirmar]** Já foi de 5 dias.\n")
    );
}

#[test]
fn frontmatter_invalido_nao_monta() {
    let mut conteudo = conteudo_pergunta(&gabarito());
    conteudo.frontmatter.id = Serie::Sugestao.id(1);
    let erro = Templates::padrao().montar(&conteudo).unwrap_err();
    assert!(
        matches!(&erro, ErroMontagem::Frontmatter { campo, .. } if campo == "id"),
        "{erro}"
    );

    let mut conteudo = conteudo_termo();
    conteudo.frontmatter.tipo = TipoDocumento::Regra;
    assert!(matches!(
        Templates::padrao().montar(&conteudo).unwrap_err(),
        ErroMontagem::Frontmatter { .. }
    ));

    let regra = Templates::padrao()
        .template(TipoDocumento::Regra)
        .unwrap()
        .clone();
    assert!(matches!(
        regra.montar(&conteudo_termo()).unwrap_err(),
        ErroMontagem::TipoDiferente { .. }
    ));
}

#[test]
fn gravacao_continua_remonta_a_cada_registro() {
    let (_dir, _base, projeto) = base_com_projeto();
    let templates = Templates::padrao();
    let g = gabarito();
    let mut conteudo = conteudo_pergunta(&g);
    conteudo.afirmacoes.clear();

    templates.gravar(&projeto, &conteudo).unwrap();
    let antes: Documento<FrontmatterAnalise> = projeto.ler(&conteudo.caminho).unwrap();
    assert!(!antes.corpo.contains("[^1]"));

    conteudo.afirmacoes.push(
        g.regra("RN-FAT-001")
            .afirmacao("regras_encontradas", Confianca::Confirmado),
    );
    templates.gravar(&projeto, &conteudo).unwrap();
    let depois: Documento<FrontmatterAnalise> = projeto.ler(&conteudo.caminho).unwrap();
    assert_eq!(depois.frontmatter, conteudo.frontmatter);
    assert!(
        depois
            .corpo
            .contains("[^1]: `db/03_pkg_cobranca.sql:71-72`")
    );
}

#[test]
fn conteudo_ida_e_volta_em_json_ou_yaml() {
    // A UI (#36) e o modelo da análise (#21) podem trocar o conteúdo serializado.
    let conteudo = conteudo_analise_impacto(&gabarito());
    let yaml = serde_norway::to_string(&conteudo).unwrap();
    let lido: Conteudo<FrontmatterAnalise> = serde_norway::from_str(&yaml).unwrap();
    assert_eq!(lido, conteudo);
}
