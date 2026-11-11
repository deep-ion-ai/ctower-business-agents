//! Base de conhecimento numa pasta com espaços e acentos, com o sistema fictício
//! "Faturamento Exemplo" e as regras do gabarito (`fixtures/gabaritos`).

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;

use cba_kb::frontmatter::{
    Area, Data, Demanda, Evidencia, FrontmatterAnalise, FrontmatterLivre, FrontmatterRegra,
    FrontmatterSistema, FrontmatterTermo, Linhas, StatusAnalise, StatusRegra,
};
use cba_kb::{Base, CaminhoRelativo, Documento, Erro, Serie, Sistema, TipoDocumento, layout};
use serde::Deserialize;

const SISTEMA: &str = "faturamento-exemplo";

/// Pasta da base com espaços e acentos, dentro de um temporário.
fn pasta_da_base() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let raiz = dir.path().join("Base de Conhecimento – Análises e Ações");
    (dir, raiz)
}

fn data() -> Data {
    Data::nova(2026, 11, 11).unwrap()
}

fn documento_do_sistema() -> Documento<FrontmatterSistema> {
    Documento::novo(
        FrontmatterSistema {
            tipo: TipoDocumento::Sistema,
            id: SISTEMA.into(),
            nome: "Faturamento Exemplo".into(),
            areas: vec![Area {
                id: "cobranca".into(),
                nome: "Cobrança".into(),
                sigla: Some("FAT".into()),
            }],
            criado: Some(data()),
        },
        "\n# Faturamento Exemplo\n\nSistema fictício de faturamento.\n",
    )
}

fn criar_base() -> (tempfile::TempDir, Base, Sistema) {
    let (dir, raiz) = pasta_da_base();
    let base = Base::criar(&raiz).unwrap();
    let sistema = base.criar_sistema(&documento_do_sistema()).unwrap();
    (dir, base, sistema)
}

fn analise(id: &cba_kb::Id, titulo: &str) -> Documento<FrontmatterAnalise> {
    Documento::novo(
        FrontmatterAnalise {
            tipo: TipoDocumento::Pergunta,
            id: id.clone(),
            titulo: titulo.into(),
            sistema: SISTEMA.into(),
            demanda: Demanda {
                origem: "texto".into(),
                chave: None,
            },
            status: StatusAnalise::Rascunho,
            abrangencia: None,
            base: None,
            criado: data(),
            analista: Some("Ana Exemplo".into()),
        },
        format!("\n# {titulo}\n"),
    )
}

#[test]
fn cria_a_estrutura_da_spec() {
    let (_dir, base, sistema) = criar_base();
    assert!(base.raiz().join("_templates").is_dir());
    for pasta in layout::PASTAS_DO_SISTEMA {
        assert!(sistema.raiz().join(pasta).is_dir(), "{pasta}");
    }
    assert!(sistema.raiz().join("sistema.md").is_file());
    assert_eq!(base.sistemas().unwrap(), [SISTEMA]);

    let lido = sistema.ler_sistema().unwrap();
    assert_eq!(lido.frontmatter, documento_do_sistema().frontmatter);
    assert_eq!(lido.corpo, documento_do_sistema().corpo);

    // Abrir de novo e criar de novo não apagam nada.
    let reaberta = Base::abrir(base.raiz()).unwrap();
    let mut outro = documento_do_sistema();
    outro.frontmatter.nome = "Outro nome".into();
    let mesmo = reaberta.criar_sistema(&outro).unwrap();
    assert_eq!(
        mesmo.ler_sistema().unwrap().frontmatter.nome,
        "Faturamento Exemplo"
    );
    assert_eq!(reaberta.sistema(SISTEMA).unwrap(), sistema);
}

#[test]
fn recusa_sistemas_e_bases_invalidos() {
    let (_dir, base, _sistema) = criar_base();
    assert!(matches!(
        base.sistema("../fora"),
        Err(Erro::NomeDeSistema(_))
    ));
    assert!(matches!(
        base.sistema("inexistente"),
        Err(Erro::SistemaInexistente(_))
    ));
    let mut doc = documento_do_sistema();
    doc.frontmatter.id = "Faturamento Exemplo".into();
    assert!(matches!(
        base.criar_sistema(&doc),
        Err(Erro::NomeDeSistema(_))
    ));
    assert!(matches!(
        Base::abrir(base.raiz().join("nao-existe")),
        Err(Erro::BaseInexistente(_))
    ));
}

#[test]
fn numera_analises_em_sequencia() {
    let (_dir, _base, sistema) = criar_base();
    assert_eq!(
        sistema.proximo_id(&Serie::Analise).unwrap().to_string(),
        "A-0001"
    );
    let (a1, c1) = sistema
        .criar_numerado(&Serie::Analise, |id| {
            analise(id, "Como é calculada a multa?")
        })
        .unwrap();
    let (a2, c2) = sistema
        .criar_numerado(&Serie::Analise, |id| analise(id, "Juros de mora"))
        .unwrap();
    assert_eq!(a1.to_string(), "A-0001");
    assert_eq!(c1.as_str(), "analises/A-0001.md");
    assert_eq!(a2.to_string(), "A-0002");
    assert_eq!(c2.as_str(), "analises/A-0002.md");

    // Um buraco na numeração não é reaproveitado: segue do maior.
    fs::remove_file(sistema.caminho(&c1)).unwrap();
    let (a3, _) = sistema
        .criar_numerado(&Serie::Analise, |id| analise(id, "Outra"))
        .unwrap();
    assert_eq!(a3.to_string(), "A-0003");

    let lido: Documento<FrontmatterAnalise> = sistema.ler(&c2).unwrap();
    assert_eq!(lido.frontmatter.id, a2);
    assert_eq!(lido.frontmatter.titulo, "Juros de mora");
}

#[test]
fn reserva_sem_colisao_entre_threads() {
    let (_dir, base, _sistema) = criar_base();
    let base = Arc::new(base);
    let threads: Vec<_> = (0..8)
        .map(|t| {
            let base = Arc::clone(&base);
            thread::spawn(move || {
                let sistema = base.sistema(SISTEMA).unwrap();
                (0..5)
                    .map(|i| {
                        sistema
                            .criar_numerado(&Serie::Analise, |id| {
                                analise(id, &format!("thread {t} análise {i}"))
                            })
                            .unwrap()
                            .0
                            .numero()
                    })
                    .collect::<Vec<_>>()
            })
        })
        .collect();
    let numeros: Vec<u32> = threads
        .into_iter()
        .flat_map(|t| t.join().unwrap())
        .collect();
    let unicos: BTreeSet<u32> = numeros.iter().copied().collect();
    assert_eq!(unicos.len(), numeros.len(), "colisão: {numeros:?}");
    assert_eq!(unicos, (1..=40).collect());

    let sistema = base.sistema(SISTEMA).unwrap();
    let docs = sistema.documentos().unwrap();
    // 40 análises e o sistema.md, sem temporários.
    assert_eq!(docs.len(), 41, "{docs:?}");
    assert!(docs.iter().all(|d| !d.nome().starts_with('.')));
}

#[derive(Deserialize)]
struct Gabarito {
    regras: Vec<RegraGabarito>,
}

#[derive(Deserialize)]
struct RegraGabarito {
    id: String,
    titulo: String,
    area: String,
    enunciado: String,
    evidencias: Vec<EvidenciaGabarito>,
}

#[derive(Deserialize)]
struct EvidenciaGabarito {
    arquivo: String,
    linha_inicial: u32,
    linha_final: u32,
}

fn gabarito() -> Gabarito {
    let caminho = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/gabaritos/faturamento-exemplo.yaml");
    serde_norway::from_str(&fs::read_to_string(caminho).unwrap()).unwrap()
}

#[test]
fn regras_do_gabarito_vao_e_voltam() {
    let (_dir, _base, sistema) = criar_base();
    let regras = gabarito().regras;
    assert!(!regras.is_empty());
    for regra in &regras {
        let id = cba_kb::Id::analisar(&regra.id).unwrap();
        let frontmatter = FrontmatterRegra {
            tipo: TipoDocumento::Regra,
            id: id.clone(),
            titulo: regra.titulo.clone(),
            area: regra.area.clone(),
            status: StatusRegra::Confirmada,
            evidencias: regra
                .evidencias
                .iter()
                .map(|e| Evidencia {
                    arquivo: CaminhoRelativo::novo(&e.arquivo).unwrap(),
                    linhas: Linhas::novo(e.linha_inicial, e.linha_final).unwrap(),
                    hash: "0000".into(),
                    commit: None,
                })
                .collect(),
            origem: Some(CaminhoRelativo::novo("analises/A-0001.md").unwrap()),
        };
        let documento = Documento::novo(frontmatter, format!("\n{}\n", regra.enunciado));
        sistema.criar(&id.arquivo(), &documento).unwrap();

        let lido: Documento<FrontmatterRegra> = sistema.ler(&id.arquivo()).unwrap();
        assert_eq!(lido.frontmatter, documento.frontmatter);
        assert_eq!(lido.corpo, documento.corpo);

        // Ler e gravar de novo preserva o arquivo byte a byte.
        let antes = fs::read(sistema.caminho(&id.arquivo())).unwrap();
        sistema.gravar(&id.arquivo(), &lido).unwrap();
        assert_eq!(fs::read(sistema.caminho(&id.arquivo())).unwrap(), antes);
    }
    // A numeração das regras segue a sigla.
    let fat = Serie::regra("FAT").unwrap();
    let maior = regras
        .iter()
        .filter_map(|r| fat.numero(&r.id))
        .max()
        .unwrap();
    assert_eq!(sistema.proximo_id(&fat).unwrap().numero(), maior + 1);
    assert_eq!(
        sistema
            .proximo_id(&Serie::regra("COB").unwrap())
            .unwrap()
            .to_string(),
        "RN-COB-001"
    );
}

#[test]
fn regravar_preserva_comentarios_e_ordem_de_um_arquivo_editado_a_mao() {
    let (_dir, _base, sistema) = criar_base();
    let termo = layout::termo("Multa por atraso").unwrap();
    let texto = "\
---
# Termo revisado pela área de cobrança.
termo: Multa por atraso
tipo: termo
sinonimos: [multa, penalidade]   # como os usuários falam
area: cobranca
responsavel: Área de Cobrança
---

## Definição

Valor cobrado uma única vez quando a fatura é paga depois da carência.
";
    fs::write(sistema.caminho(&termo), texto).unwrap();

    let mut doc: Documento<FrontmatterTermo> = sistema.ler(&termo).unwrap();
    sistema.gravar(&termo, &doc).unwrap();
    assert_eq!(fs::read_to_string(sistema.caminho(&termo)).unwrap(), texto);

    doc.frontmatter.sinonimos.push("encargo".into());
    sistema.gravar(&termo, &doc).unwrap();
    let livre: Documento<FrontmatterLivre> = sistema.ler(&termo).unwrap();
    let chaves: Vec<_> = livre
        .frontmatter
        .0
        .keys()
        .map(|k| k.as_str().unwrap().to_owned())
        .collect();
    assert_eq!(
        chaves,
        ["termo", "tipo", "sinonimos", "area", "responsavel"]
    );
    assert_eq!(livre.corpo, doc.corpo);
}

#[test]
fn frontmatter_invalido_aponta_arquivo_e_campo() {
    let (_dir, _base, sistema) = criar_base();
    let (id, caminho) = sistema
        .criar_numerado(&Serie::Analise, |id| analise(id, "Multa"))
        .unwrap();
    let disco = sistema.caminho(&caminho);
    let texto = fs::read_to_string(&disco).unwrap();
    fs::write(
        &disco,
        texto.replace("status: rascunho", "status: concluida"),
    )
    .unwrap();

    let erro = sistema.ler::<FrontmatterAnalise>(&caminho).unwrap_err();
    let mensagem = erro.to_string();
    assert!(
        mensagem.starts_with(&format!("{SISTEMA}/analises/{id}.md: ")),
        "{mensagem}"
    );
    assert!(mensagem.contains("campo `status`"), "{mensagem}");
    match erro {
        Erro::Frontmatter { arquivo, erro } => {
            assert_eq!(arquivo.as_str(), format!("{SISTEMA}/analises/{id}.md"));
            assert_eq!(erro.campo.as_deref(), Some("status"));
        }
        outro => panic!("erro inesperado: {outro:?}"),
    }
}

#[test]
fn caminhos_absolutos_sao_recusados() {
    let (_dir, base, _sistema) = criar_base();
    let absoluto = base.raiz().join("x.md");
    assert!(CaminhoRelativo::de_caminho(&absoluto).is_err());
    assert!(CaminhoRelativo::novo(&absoluto.to_string_lossy()).is_err());
}

#[test]
fn templates_na_raiz_da_base() {
    let (_dir, base, _sistema) = criar_base();
    let template = CaminhoRelativo::novo("_templates/analise-impacto.md").unwrap();
    base.gravar_texto(&template, "# {{titulo}}\n").unwrap();
    assert_eq!(base.ler_texto(&template).unwrap(), "# {{titulo}}\n");
    assert!(base.templates().join("analise-impacto.md").is_file());
}
