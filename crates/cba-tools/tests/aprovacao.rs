//! Aprovação da análise (#11) com o sistema fictício "Faturamento Exemplo": as
//! afirmações confirmadas viram regras e os termos aceitos viram glossário, sem
//! duplicar, e uma análise nova sobre o mesmo assunto recebe as regras no contexto.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use cba_core::ambiente::Ambiente;
use cba_core::analise::{Analise, EstadoAfirmacao, EstadoAnalise, EstadoTermo, Fluxo};
use cba_core::analises::NovaAnalise;
use cba_core::config::Config;
use cba_core::dados::DiretorioDados;
use cba_core::eventos::Mudanca;
use cba_kb::frontmatter::{
    Area, FrontmatterAnalise, FrontmatterProjeto, FrontmatterRegra, FrontmatterTermo,
    StatusAnalise, StatusRegra,
};
use cba_kb::indice::{Consulta, Indice};
use cba_kb::{CaminhoRelativo, Documento, Id, layout};
use cba_tools::aprovacao::{
    Acao, AjusteDaRegra, Aprovacao, ErroAprovacao, OpcoesAprovacao, aprovar, aprovar_analise,
};
use cba_tools::evidencias::hash_das_linhas;
use cba_tools::{ErroChamada, registro_padrao};
use serde::Deserialize;
use serde_json::{Value, json};

// ---------------------------------------------------------------------------
// Gabarito

#[derive(Deserialize)]
struct Gabarito {
    regras: Vec<RegraG>,
}

#[derive(Deserialize)]
struct RegraG {
    id: String,
    area: String,
    enunciado: String,
    evidencias: Vec<EvidenciaG>,
}

#[derive(Deserialize)]
struct EvidenciaG {
    arquivo: String,
    linha_inicial: u32,
    linha_final: u32,
    trecho: String,
}

fn raiz_do_repositorio() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixture() -> PathBuf {
    raiz_do_repositorio().join("fixtures/faturamento-exemplo")
}

fn gabarito() -> Gabarito {
    let texto = fs::read_to_string(
        raiz_do_repositorio().join("fixtures/gabaritos/faturamento-exemplo.yaml"),
    )
    .unwrap();
    serde_norway::from_str(&texto).unwrap()
}

impl Gabarito {
    fn regra(&self, id: &str) -> &RegraG {
        self.regras.iter().find(|r| r.id == id).unwrap()
    }
}

impl RegraG {
    /// Argumentos de `afirmar` com o enunciado e as evidências do gabarito.
    fn afirmar(&self, analise: &Id) -> Value {
        let evidencias: Vec<Value> = self
            .evidencias
            .iter()
            .map(|e| {
                json!({
                    "arquivo": e.arquivo,
                    "linha_inicial": e.linha_inicial,
                    "linha_final": e.linha_final,
                    "trecho": e.trecho,
                })
            })
            .collect();
        json!({
            "analise": analise,
            "secao": "resposta",
            "texto": self.enunciado,
            "confianca": "confirmado",
            "evidencias": evidencias,
        })
    }

    /// (arquivo, linhas, hash) esperados das evidências da regra.
    fn evidencias_esperadas(&self) -> Vec<(String, String, String)> {
        self.evidencias
            .iter()
            .map(|e| {
                let texto = fs::read_to_string(fixture().join(&e.arquivo)).unwrap();
                let linhas =
                    cba_kb::frontmatter::Linhas::novo(e.linha_inicial, e.linha_final).unwrap();
                (
                    e.arquivo.clone(),
                    linhas.to_string(),
                    hash_das_linhas(&texto, linhas).unwrap(),
                )
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Cenário

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
    /// Declara as áreas no `projeto.md`.
    fn declarar_areas(&self, areas: Vec<Area>) {
        let mut doc = self.amb.kb.ler_projeto().unwrap();
        doc.frontmatter.areas = areas;
        self.amb.kb.gravar(&layout::projeto(), &doc).unwrap();
    }

    fn areas_do_gabarito(&self) {
        let area = |id: &str, nome: &str| Area {
            id: id.into(),
            nome: nome.into(),
            sigla: Some("FAT".into()),
        };
        self.declarar_areas(vec![
            area("cobranca", "Cobrança"),
            area("parcelamento", "Parcelamento"),
            area("cancelamento", "Cancelamento"),
        ]);
    }

    fn chamar(&self, nome: &str, args: Value) -> Value {
        match registro_padrao().chamar(nome, Ok(&self.amb), Some(args)) {
            Ok(saida) => saida.dados.unwrap(),
            Err(ErroChamada::Ferramenta(e)) => panic!("{nome}: {e}"),
            Err(e) => panic!("{e}"),
        }
    }

    fn criar(&self, titulo: &str, entrada: &str) -> Analise {
        let a = self
            .amb
            .analises
            .criar(NovaAnalise {
                fluxo: Fluxo::Perguntar,
                titulo: titulo.into(),
                entrada: entrada.into(),
                analista: Some("Ana Exemplo".into()),
                base: None,
            })
            .unwrap();
        self.chamar("iniciar_analise", json!({ "analise": a.id }));
        a
    }

    /// Conclui a análise e aplica as decisões do analista.
    fn concluir_e_decidir(
        &self,
        id: &Id,
        afirmacoes: &[(u32, EstadoAfirmacao)],
        termos: &[(u32, EstadoTermo)],
    ) {
        self.chamar("concluir_analise", json!({ "analise": id }));
        self.amb
            .analises
            .alterar(id, Mudanca::Afirmacao, |a, _| {
                for (n, e) in afirmacoes {
                    a.marcar_afirmacao(*n, e.clone())?;
                }
                for (n, e) in termos {
                    a.decidir_termo(*n, *e)?;
                }
                Ok(())
            })
            .unwrap();
    }

    fn aprovar(&self, id: &Id, opcoes: &OpcoesAprovacao) -> Result<Aprovacao, ErroAprovacao> {
        aprovar(&self.amb.base, &self.amb.analises, id, opcoes)
    }

    fn regra(&self, arquivo: &CaminhoRelativo) -> Documento<FrontmatterRegra> {
        self.amb.kb.ler(arquivo).unwrap()
    }

    fn arquivos(&self, pasta: &str) -> Vec<String> {
        let mut nomes: Vec<String> = fs::read_dir(self.amb.kb.raiz().join(pasta))
            .unwrap()
            .flatten()
            .map(|e| e.file_name().into_string().unwrap())
            .filter(|n| !n.starts_with('.'))
            .collect();
        nomes.sort();
        nomes
    }

    /// Texto de todos os documentos de regras e glossário, para comparar.
    fn instantaneo(&self) -> BTreeMap<String, String> {
        let mut tudo = BTreeMap::new();
        for pasta in [layout::REGRAS, layout::GLOSSARIO] {
            for nome in self.arquivos(pasta) {
                let caminho = self.amb.kb.raiz().join(pasta).join(&nome);
                tudo.insert(
                    format!("{pasta}/{nome}"),
                    fs::read_to_string(caminho).unwrap(),
                );
            }
        }
        tudo
    }
}

fn evidencias_da_regra(f: &FrontmatterRegra) -> Vec<(String, String, String)> {
    f.evidencias
        .iter()
        .map(|e| {
            (
                e.arquivo.as_str().to_owned(),
                e.linhas.to_string(),
                e.hash.clone(),
            )
        })
        .collect()
}

/// Análise da multa e da carência: RN-FAT-001 e RN-FAT-003 confirmadas, uma
/// afirmação incorreta, um termo aceito e um recusado.
fn analise_da_multa(c: &Cenario) -> Analise {
    let g = gabarito();
    let a = c.criar(
        "Multa e carência na cobrança",
        "Como são calculadas a multa e a carência de uma fatura em atraso?",
    );
    c.chamar("afirmar", g.regra("RN-FAT-001").afirmar(&a.id));
    c.chamar("afirmar", g.regra("RN-FAT-003").afirmar(&a.id));
    c.chamar(
        "afirmar",
        json!({
            "analise": a.id,
            "secao": "resposta",
            "texto": "A multa é cobrada todo dia de atraso.",
            "confianca": "inferido",
        }),
    );
    c.chamar(
        "propor_termo",
        json!({
            "analise": a.id,
            "termo": "Multa por atraso",
            "definicao": "Valor único cobrado sobre a fatura paga depois da carência.",
            "sinonimos": ["multa moratória"],
            "onde_vive_no_codigo": ["PKG_COBRANCA.FN_CALCULAR_MULTA", "parâmetro PERC_MULTA_ATRASO"],
            "area": "cobranca",
            "evidencias": [{
                "arquivo": "db/03_pkg_cobranca.sql",
                "linha_inicial": 71,
                "linha_final": 72,
                "trecho": "FN_PARAMETRO('PERC_MULTA_ATRASO') / 100, 2)"
            }]
        }),
    );
    c.chamar(
        "propor_termo",
        json!({
            "analise": a.id,
            "termo": "Juros compostos",
            "definicao": "Juros sobre juros.",
        }),
    );
    c.concluir_e_decidir(
        &a.id,
        &[
            (1, EstadoAfirmacao::Confirmada),
            (2, EstadoAfirmacao::Confirmada),
            (
                3,
                EstadoAfirmacao::Incorreta {
                    comentario: "a multa é única".into(),
                },
            ),
        ],
        &[(1, EstadoTermo::Aceito), (2, EstadoTermo::Recusado)],
    );
    a
}

// ---------------------------------------------------------------------------
// Testes

#[test]
fn aprovar_cria_as_regras_e_os_termos_esperados() {
    let c = cenario();
    c.areas_do_gabarito();
    let g = gabarito();
    let a = analise_da_multa(&c);
    let opcoes = OpcoesAprovacao {
        area: Some("Cobrança".into()),
        ..Default::default()
    };
    let r = c.aprovar(&a.id, &opcoes).unwrap();

    // A análise está aprovada, e o documento também.
    assert!(!r.ja_aprovada);
    assert_eq!(r.analise.estado, EstadoAnalise::Aprovada);
    let doc: Documento<FrontmatterAnalise> = c.amb.kb.ler(&a.documento).unwrap();
    assert_eq!(doc.frontmatter.status, StatusAnalise::Aprovada);

    // Duas regras, uma por afirmação confirmada; a incorreta fica de fora.
    let ids: Vec<String> = r.regras.iter().map(|x| x.id.to_string()).collect();
    assert_eq!(ids, ["RN-FAT-001", "RN-FAT-002"]);
    assert!(r.regras.iter().all(|x| x.acao == Acao::Criada));
    assert_eq!(r.regras[0].afirmacoes, [1]);
    assert_eq!(r.regras[1].afirmacoes, [2]);
    assert_eq!(c.arquivos("regras"), ["RN-FAT-001.md", "RN-FAT-002.md"]);

    for (aprovada, esperada) in r.regras.iter().zip(["RN-FAT-001", "RN-FAT-003"]) {
        let esperada = g.regra(esperada);
        let doc = c.regra(&aprovada.arquivo);
        let f = &doc.frontmatter;
        assert_eq!(f.id, aprovada.id);
        assert_eq!(f.area, esperada.area);
        assert_eq!(f.status, StatusRegra::Confirmada);
        assert_eq!(f.origem.as_ref().unwrap().as_str(), "analises/A-0001.md");
        assert_eq!(evidencias_da_regra(f), esperada.evidencias_esperadas());
        // O documento sai do template de regra, com o enunciado e as evidências.
        assert!(
            doc.corpo.contains(&format!("# {} · ", f.id)),
            "{}",
            doc.corpo
        );
        assert!(doc.corpo.contains(&esperada.enunciado), "{}", doc.corpo);
        assert!(
            doc.corpo
                .contains("[analises/A-0001.md](../analises/A-0001.md)"),
            "{}",
            doc.corpo
        );
        for e in &esperada.evidencias {
            assert!(doc.corpo.contains(&e.arquivo), "{}", doc.corpo);
        }
    }
    let primeira = c.regra(&r.regras[0].arquivo).frontmatter;
    assert!(
        primeira.titulo.starts_with("Passada a carência"),
        "{}",
        primeira.titulo
    );
    assert!(primeira.titulo.chars().count() <= 81);

    // Só o termo aceito entra no glossário.
    assert_eq!(r.termos.len(), 1);
    assert_eq!(r.termos[0].acao, Acao::Criada);
    assert_eq!(
        r.termos[0].arquivo.as_str(),
        "glossario/multa-por-atraso.md"
    );
    assert_eq!(c.arquivos("glossario"), ["multa-por-atraso.md"]);
    let termo: Documento<FrontmatterTermo> = c.amb.kb.ler(&r.termos[0].arquivo).unwrap();
    assert_eq!(termo.frontmatter.termo, "Multa por atraso");
    assert_eq!(termo.frontmatter.sinonimos, ["multa moratória"]);
    assert_eq!(termo.frontmatter.area.as_deref(), Some("cobranca"));
    assert_eq!(
        termo.frontmatter.origem.as_ref().unwrap().as_str(),
        "analises/A-0001.md"
    );
    assert!(
        termo.corpo.contains("Valor único cobrado"),
        "{}",
        termo.corpo
    );
    assert!(
        termo.corpo.contains("- PKG_COBRANCA.FN_CALCULAR_MULTA"),
        "{}",
        termo.corpo
    );
    assert!(
        termo.corpo.contains("db/03_pkg_cobranca.sql"),
        "{}",
        termo.corpo
    );
}

#[test]
fn aprovar_de_novo_nao_duplica_nada() {
    let c = cenario();
    c.areas_do_gabarito();
    let a = analise_da_multa(&c);
    let opcoes = OpcoesAprovacao::default();
    let primeira = c.aprovar(&a.id, &opcoes).unwrap();
    let antes = c.instantaneo();
    let versao = primeira.analise.versao;

    let segunda = c.aprovar(&a.id, &opcoes).unwrap();
    assert!(segunda.ja_aprovada);
    assert_eq!(segunda.analise.versao, versao, "o estado não muda");
    assert_eq!(
        segunda.regras.iter().map(|r| &r.id).collect::<Vec<_>>(),
        primeira.regras.iter().map(|r| &r.id).collect::<Vec<_>>()
    );
    assert!(segunda.regras.iter().all(|r| r.acao == Acao::SemMudanca));
    assert!(segunda.termos.iter().all(|t| t.acao == Acao::SemMudanca));
    assert_eq!(c.instantaneo(), antes);

    // Um termo aceito depois da aprovação entra numa nova aprovação, e só ele.
    c.amb
        .analises
        .alterar(&a.id, Mudanca::Termo, |an, _| {
            an.termos[1].estado = EstadoTermo::Proposto;
            an.decidir_termo(2, EstadoTermo::Aceito)
        })
        .unwrap();
    let terceira = c.aprovar(&a.id, &opcoes).unwrap();
    let acoes: Vec<Acao> = terceira.termos.iter().map(|t| t.acao).collect();
    assert_eq!(acoes, [Acao::SemMudanca, Acao::Criada]);
    assert_eq!(c.arquivos("regras").len(), 2);
    assert_eq!(
        c.arquivos("glossario"),
        ["juros-compostos.md", "multa-por-atraso.md"]
    );
}

#[test]
fn nova_analise_com_a_mesma_evidencia_atualiza_a_regra_e_o_termo() {
    let c = cenario();
    c.areas_do_gabarito();
    let g = gabarito();
    let a = analise_da_multa(&c);
    let cobranca = OpcoesAprovacao {
        area: Some("cobranca".into()),
        ..Default::default()
    };
    c.aprovar(&a.id, &cobranca).unwrap();

    // Outra análise confirma a multa com as mesmas evidências (em outra ordem) e
    // texto novo, e propõe de novo o termo, com outro sinônimo.
    let b = c.criar("Multa", "Qual é a multa por atraso?");
    let mut args = g.regra("RN-FAT-001").afirmar(&b.id);
    args["texto"] = json!("A multa por atraso é de 2% sobre o valor original.");
    args["evidencias"].as_array_mut().unwrap().reverse();
    c.chamar("afirmar", args);
    c.chamar(
        "propor_termo",
        json!({
            "analise": b.id,
            "termo": "multa por atraso",
            "definicao": "Multa única de 2% sobre o valor original.",
            "sinonimos": ["multa moratória", "multa de mora"],
        }),
    );
    c.concluir_e_decidir(
        &b.id,
        &[(1, EstadoAfirmacao::Confirmada)],
        &[(1, EstadoTermo::Aceito)],
    );
    let r = c.aprovar(&b.id, &OpcoesAprovacao::default()).unwrap();

    assert_eq!(r.regras.len(), 1);
    assert_eq!(r.regras[0].id.to_string(), "RN-FAT-001");
    assert_eq!(r.regras[0].acao, Acao::Atualizada);
    assert_eq!(c.arquivos("regras"), ["RN-FAT-001.md", "RN-FAT-002.md"]);
    let doc = c.regra(&r.regras[0].arquivo);
    assert_eq!(
        doc.frontmatter.titulo,
        "A multa por atraso é de 2% sobre o valor original"
    );
    assert_eq!(doc.frontmatter.area, "cobranca");
    // A origem continua sendo a análise que criou a regra.
    assert_eq!(
        doc.frontmatter.origem.as_ref().unwrap().as_str(),
        "analises/A-0001.md"
    );
    assert!(
        doc.corpo.contains("A multa por atraso é de 2%"),
        "{}",
        doc.corpo
    );

    assert_eq!(r.termos.len(), 1);
    assert_eq!(r.termos[0].acao, Acao::Atualizada);
    assert_eq!(c.arquivos("glossario"), ["multa-por-atraso.md"]);
    let termo: Documento<FrontmatterTermo> = c.amb.kb.ler(&r.termos[0].arquivo).unwrap();
    assert_eq!(termo.frontmatter.termo, "Multa por atraso");
    assert_eq!(
        termo.frontmatter.sinonimos,
        ["multa moratória", "multa de mora"]
    );
    assert_eq!(termo.frontmatter.area.as_deref(), Some("cobranca"));
    assert!(termo.corpo.contains("Multa única de 2%"), "{}", termo.corpo);
}

#[test]
fn nova_analise_sobre_o_mesmo_assunto_recebe_as_regras_no_contexto() {
    let c = cenario();
    c.areas_do_gabarito();
    let a = analise_da_multa(&c);
    let r = aprovar_analise(
        &c.dados,
        "faturamento-exemplo",
        &a.id,
        &OpcoesAprovacao::default(),
    )
    .unwrap();
    assert!(r.avisos.is_empty(), "{:?}", r.avisos);
    let regra = r.regras[0].id.to_string();

    // O índice já tem a regra.
    let indice = Indice::abrir(c.dados.arquivo_do_indice(), &c.amb.base).unwrap();
    let achados = indice.buscar(&Consulta::nova("multa atraso")).unwrap();
    assert!(
        achados
            .resultados
            .iter()
            .any(|x| x.identificador.as_deref() == Some(regra.as_str())),
        "{achados:?}"
    );

    // E a análise seguinte recebe a regra no contexto de `iniciar_analise`.
    let b = c
        .amb
        .analises
        .criar(NovaAnalise {
            fluxo: Fluxo::Perguntar,
            titulo: "Multa por atraso".into(),
            entrada: "Como é calculada a multa por atraso de uma fatura?".into(),
            analista: None,
            base: None,
        })
        .unwrap();
    let dados = c.chamar("iniciar_analise", json!({ "analise": b.id }));
    let regras = dados["contexto"]["regras"].as_array().unwrap();
    assert!(
        regras.iter().any(|x| x["id"] == regra.as_str()),
        "{}",
        dados["contexto"]
    );
}

#[test]
fn area_da_aprovacao_ajustes_e_sem_area() {
    // Projeto sem áreas declaradas e sem área na aprovação: área geral, RN-GER.
    let c = cenario();
    let a = analise_da_multa(&c);
    let r = c.aprovar(&a.id, &OpcoesAprovacao::default()).unwrap();
    let ids: Vec<String> = r.regras.iter().map(|x| x.id.to_string()).collect();
    assert_eq!(ids, ["RN-GER-001", "RN-GER-002"]);
    assert!(r.regras.iter().all(|x| x.area == "geral"));

    // Uma área só no projeto.md: ela vale. Ajuste por afirmação: área e título.
    let c = cenario();
    c.declarar_areas(vec![Area {
        id: "cobranca".into(),
        nome: "Cobrança".into(),
        sigla: Some("COBR".into()),
    }]);
    let a = analise_da_multa(&c);
    let opcoes = OpcoesAprovacao {
        area: None,
        afirmacoes: BTreeMap::from([(
            2,
            AjusteDaRegra {
                area: Some("Prazos".into()),
                titulo: Some("Carência de 3 dias sem encargos".into()),
            },
        )]),
    };
    let r = c.aprovar(&a.id, &opcoes).unwrap();
    let ids: Vec<String> = r.regras.iter().map(|x| x.id.to_string()).collect();
    assert_eq!(ids, ["RN-COBR-001", "RN-PRA-001"]);
    assert_eq!(r.regras[1].area, "prazos");
    let doc = c.regra(&r.regras[1].arquivo);
    assert_eq!(doc.frontmatter.titulo, "Carência de 3 dias sem encargos");
}

#[test]
fn recusas_nao_gravam_nada() {
    let c = cenario();
    c.areas_do_gabarito();

    // Análise em andamento não é aprovada.
    let a = c.criar("Multa", "Qual é a multa?");
    let erro = c.aprovar(&a.id, &OpcoesAprovacao::default()).unwrap_err();
    assert!(matches!(erro, ErroAprovacao::NaoConcluida { .. }), "{erro}");
    assert!(erro.to_string().contains("em andamento"), "{erro}");

    // Ajuste para afirmação não confirmada (a 3 foi marcada incorreta).
    let b = analise_da_multa(&c);
    let opcoes = OpcoesAprovacao {
        afirmacoes: BTreeMap::from([(3, AjusteDaRegra::default())]),
        ..Default::default()
    };
    let erro = c.aprovar(&b.id, &opcoes).unwrap_err();
    assert!(
        matches!(erro, ErroAprovacao::AjusteSemAfirmacao(3)),
        "{erro}"
    );
    let opcoes = OpcoesAprovacao {
        area: Some("??".into()),
        ..Default::default()
    };
    assert!(matches!(
        c.aprovar(&b.id, &opcoes).unwrap_err(),
        ErroAprovacao::AreaInvalida(_)
    ));
    assert_eq!(
        c.amb.analises.carregar(&b.id).unwrap().estado,
        EstadoAnalise::Concluida
    );
    assert!(c.arquivos("regras").is_empty());
    assert!(c.arquivos("glossario").is_empty());
}

#[test]
fn campos_desconhecidos_da_regra_existente_sao_mantidos() {
    let c = cenario();
    c.areas_do_gabarito();
    let a = analise_da_multa(&c);
    let r = c.aprovar(&a.id, &OpcoesAprovacao::default()).unwrap();
    let caminho = c.amb.kb.caminho(&r.regras[0].arquivo);
    let texto = fs::read_to_string(&caminho).unwrap();
    let texto = texto.replacen("tipo: regra\n", "tipo: regra\nresponsavel: Ana\n", 1);
    fs::write(&caminho, &texto).unwrap();

    // Sem mudança, o arquivo fica como está; com o status mudado à mão, a
    // aprovação volta o status e mantém o campo novo.
    let r2 = c.aprovar(&a.id, &OpcoesAprovacao::default()).unwrap();
    assert_eq!(r2.regras[0].acao, Acao::SemMudanca);
    let mudado = texto.replacen("status: confirmada", "status: a-revalidar", 1);
    fs::write(&caminho, &mudado).unwrap();
    let r3 = c.aprovar(&a.id, &OpcoesAprovacao::default()).unwrap();
    assert_eq!(r3.regras[0].acao, Acao::Atualizada);
    let depois = fs::read_to_string(&caminho).unwrap();
    assert!(depois.contains("responsavel: Ana"), "{depois}");
    assert!(depois.contains("status: confirmada"), "{depois}");
    let projeto: Documento<FrontmatterProjeto> = c.amb.kb.ler_projeto().unwrap();
    assert_eq!(projeto.frontmatter.areas.len(), 3);
}
