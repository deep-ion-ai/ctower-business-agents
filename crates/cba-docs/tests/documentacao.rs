//! Documentação legível (#288): páginas geradas da base de exemplo em
//! `tests/documentacao/base/`, comparadas com as esperadas em
//! `tests/esperados/documentacao/`.
//!
//! Para atualizar as páginas esperadas depois de mudar o renderizador:
//! `CBA_ATUALIZAR_ESPERADOS=1 cargo test -p cba-docs --test documentacao`, e
//! revise o diff.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use cba_core::funcionalidades::MapaFuncionalidades;
use cba_docs::marcador::{Marcador, blocos};
use cba_docs::{
    ContextoDaDocumentacao, Documentacao, DocumentoDaBase, LinksDeEvidencia, gerar_documentacao,
};
use cba_kb::frontmatter::{Linhas, VersaoCodigo};
use cba_repos::{ForgeKind, RepositorioCodigo};
use pulldown_cmark::{Event, Options, Parser, Tag};

fn raiz() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/documentacao/base")
}

fn documentos() -> Vec<DocumentoDaBase> {
    fn visitar(pasta: &Path, raiz: &Path, saida: &mut Vec<DocumentoDaBase>) {
        let mut entradas: Vec<_> = std::fs::read_dir(pasta)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        entradas.sort();
        for p in entradas {
            if p.is_dir() {
                visitar(&p, raiz, saida);
            } else if p.extension().is_some_and(|e| e == "md") {
                let caminho = p
                    .strip_prefix(raiz)
                    .unwrap()
                    .components()
                    .map(|c| c.as_os_str().to_str().unwrap())
                    .collect::<Vec<_>>()
                    .join("/");
                saida.push(DocumentoDaBase {
                    caminho,
                    texto: std::fs::read_to_string(&p).unwrap(),
                });
            }
        }
    }
    let mut saida = Vec::new();
    visitar(&raiz(), &raiz(), &mut saida);
    saida
}

fn mapa() -> MapaFuncionalidades {
    let texto = std::fs::read(raiz().join("funcionalidades/mapa.estado.json")).unwrap();
    serde_json::from_slice(&texto).unwrap()
}

fn contexto() -> ContextoDaDocumentacao {
    ContextoDaDocumentacao {
        projeto: "Faturamento Exemplo".into(),
        areas: BTreeMap::from([("faturamento".into(), "Faturamento".into())]),
        codigo: Some(VersaoCodigo {
            branch: Some("main".into()),
            commit: Some("c0ffee1234567890".into()),
        }),
        data: Some("2026-11-20".into()),
    }
}

/// Permalinks pelo cadastro real do repositório (`RepositorioCodigo::link`).
struct Forja(RepositorioCodigo);

impl LinksDeEvidencia for Forja {
    fn link(&self, arquivo: &str, linhas: Linhas, commit: Option<&str>) -> Option<String> {
        self.0
            .link(commit?, arquivo, Some((linhas.inicio, linhas.fim)))
    }
}

fn github() -> Forja {
    Forja(RepositorioCodigo {
        caminho: "exemplo/faturamento".into(),
        ..Default::default()
    })
}

fn gitlab() -> Forja {
    Forja(RepositorioCodigo {
        forja: ForgeKind::Gitlab,
        host: "gitlab.example.com".into(),
        caminho: "grupo/faturamento".into(),
        ..Default::default()
    })
}

fn gerar(links: Option<&dyn LinksDeEvidencia>) -> Documentacao {
    gerar_documentacao(&documentos(), Some(&mapa()), links, &contexto())
}

fn paginas(d: &Documentacao) -> BTreeMap<&str, &str> {
    d.paginas
        .iter()
        .map(|p| (p.caminho.as_str(), p.conteudo.as_str()))
        .collect()
}

#[test]
fn paginas_esperadas() {
    let d = gerar(Some(&github()));
    let esperados = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/esperados/documentacao");
    if std::env::var_os("CBA_ATUALIZAR_ESPERADOS").is_some() {
        let _ = std::fs::remove_dir_all(&esperados);
        for p in &d.paginas {
            let destino = esperados.join(&p.caminho);
            std::fs::create_dir_all(destino.parent().unwrap()).unwrap();
            std::fs::write(destino, &p.conteudo).unwrap();
        }
    }
    let caminhos: Vec<&str> = d.paginas.iter().map(|p| p.caminho.as_str()).collect();
    assert_eq!(
        caminhos,
        [
            "README.md",
            "analises/A-0001.md",
            "analises/A-0002.md",
            "analises/README.md",
            "funcionalidades/cancelamento/f-can-001-cancelar-fatura.md",
            "funcionalidades/cobranca/f-cob-001-cobrar-multa-por-atraso.md",
            "glossario.md",
            "regras/cobranca.md",
            "regras/faturamento.md",
        ]
    );
    for p in &d.paginas {
        let esperado = std::fs::read_to_string(esperados.join(&p.caminho))
            .unwrap_or_else(|_| panic!("falta o esperado de {}", p.caminho));
        assert_eq!(p.conteudo, esperado, "página {}", p.caminho);
    }
    // O documento ilegível vira aviso; o mapa não é página.
    assert_eq!(d.avisos.len(), 1, "{:?}", d.avisos);
    assert!(
        d.avisos[0].starts_with("analises/A-0009.md"),
        "{:?}",
        d.avisos
    );
}

#[test]
fn mesma_base_mesmos_bytes() {
    let a = gerar(Some(&github()));
    let mut docs = documentos();
    docs.reverse();
    let b = gerar_documentacao(&docs, Some(&mapa()), Some(&github()), &contexto());
    assert_eq!(a, b);
    // A data só aparece no README.
    for p in &a.paginas {
        assert_eq!(
            p.conteudo.contains("2026-11-20"),
            p.caminho == "README.md",
            "{}",
            p.caminho
        );
    }
}

/// Destino de um link relativo resolvido a partir da página.
fn resolver(de: &str, destino: &str) -> String {
    let mut partes: Vec<&str> = de.split('/').collect();
    partes.pop();
    for p in destino.split('/') {
        match p {
            "" | "." => {}
            ".." => {
                partes.pop().expect("link sai da pasta da documentação");
            }
            p => partes.push(p),
        }
    }
    partes.join("/")
}

#[test]
fn todo_link_relativo_resolve() {
    for d in [gerar(Some(&github())), gerar(None)] {
        let paginas = paginas(&d);
        let mut conferidos = 0;
        for (caminho, conteudo) in &paginas {
            for evento in Parser::new_ext(conteudo, Options::all()) {
                let Event::Start(Tag::Link { dest_url, .. }) = evento else {
                    continue;
                };
                if dest_url.contains("://") {
                    assert!(dest_url.starts_with("https://"), "{dest_url}");
                    continue;
                }
                let (arquivo, ancora) = match dest_url.split_once('#') {
                    Some((a, b)) => (a, Some(b)),
                    None => (dest_url.as_ref(), None),
                };
                let alvo = if arquivo.is_empty() {
                    caminho.to_string()
                } else {
                    resolver(caminho, arquivo)
                };
                let texto = paginas
                    .get(alvo.as_str())
                    .unwrap_or_else(|| panic!("{caminho}: link para {dest_url} não existe"));
                if let Some(a) = ancora {
                    assert!(
                        texto.contains(&format!("<a id=\"{a}\"></a>")),
                        "{caminho}: âncora {dest_url} não existe"
                    );
                }
                conferidos += 1;
            }
        }
        assert!(conferidos > 40, "{conferidos}");
    }
}

#[test]
fn ids_citados_viram_links() {
    let d = gerar(Some(&github()));
    let p = paginas(&d);
    // Na análise: link da base reescrito, id solto e o termo.
    let a = p["analises/A-0001.md"];
    assert!(a.contains("[RN-COB-001](../funcionalidades/cobranca/f-cob-001-cobrar-multa-por-atraso.md#rn-cob-001)"), "{a}");
    assert!(a.contains("e [RN-COB-002](../funcionalidades/cobranca/f-cob-001-cobrar-multa-por-atraso.md#rn-cob-002))"), "{a}");
    assert!(a.contains("[termo Multa](../glossario.md#multa)"), "{a}");
    assert!(a.contains("[explicação](A-0002.md)"), "{a}");
    // Documento fora da base publicada: só o texto.
    assert!(a.contains(" e o rascunho."), "{a}");
    // Código não vira link.
    assert!(a.contains("(`RN-COB-001` em código"), "{a}");
    assert!(a.contains("[S-0001](#s-0001)"), "{a}");
    // Regra sem funcionalidade aponta para a página da funcionalidade.
    let fat = p["regras/faturamento.md"];
    assert!(fat.contains("multa ([RN-COB-001](../funcionalidades/cobranca/f-cob-001-cobrar-multa-por-atraso.md#rn-cob-001))"), "{fat}");
}

#[test]
fn cada_documento_num_bloco_so() {
    let d = gerar(Some(&github()));
    let mut vistos: BTreeMap<(String, String), String> = BTreeMap::new();
    for p in &d.paginas {
        for b in blocos(&p.conteudo).unwrap() {
            let chave = (b.marcador.tipo.clone(), b.marcador.id.clone());
            assert!(b.ancora().is_some(), "{chave:?} sem âncora");
            if let Some(outra) = vistos.insert(chave.clone(), p.caminho.clone()) {
                panic!("{chave:?} em {outra} e em {}", p.caminho);
            }
        }
    }
    let ids: BTreeSet<String> = vistos.keys().map(|(t, i)| format!("{t}:{i}")).collect();
    assert_eq!(
        ids,
        BTreeSet::from(
            [
                "funcionalidade:A-0002",
                "funcionalidade:cancelamento/cancelar-fatura/negocial",
                "pergunta:A-0001",
                "regra:RN-COB-001",
                "regra:RN-COB-002",
                "regra:RN-COB-003",
                "regra:RN-COB-004",
                "regra:RN-FAT-001",
                "sugestao:S-0001",
                "termo:fatura",
                "termo:multa",
            ]
            .map(String::from)
        )
    );
    // Onde cada um ficou.
    let onde = |t: &str, i: &str| vistos[&(t.to_owned(), i.to_owned())].clone();
    let cob = "funcionalidades/cobranca/f-cob-001-cobrar-multa-por-atraso.md";
    assert_eq!(onde("regra", "RN-COB-001"), cob);
    // Juntada vale pela de destino.
    assert_eq!(onde("regra", "RN-COB-002"), cob);
    // Sem o campo: herda da análise de origem.
    assert_eq!(onde("regra", "RN-COB-003"), cob);
    // Funcionalidade pendente: página da área.
    assert_eq!(onde("regra", "RN-COB-004"), "regras/cobranca.md");
    assert_eq!(onde("regra", "RN-FAT-001"), "regras/faturamento.md");
    assert_eq!(
        onde("funcionalidade", "cancelamento/cancelar-fatura/negocial"),
        "funcionalidades/cancelamento/f-can-001-cancelar-fatura.md"
    );
    assert_eq!(onde("sugestao", "S-0001"), "analises/A-0001.md");
}

#[test]
fn marcador_reconstroi_o_frontmatter() {
    let d = gerar(None);
    let originais: BTreeMap<String, String> = documentos()
        .into_iter()
        .map(|d| (d.caminho, d.texto))
        .collect();
    let mut n = 0;
    for p in &d.paginas {
        for b in blocos(&p.conteudo).unwrap() {
            let original = &originais[&b.marcador.metadados.caminho];
            let yaml = original
                .strip_prefix("---\n")
                .and_then(|r| r.split_once("\n---\n"))
                .unwrap()
                .0;
            let esperado: serde_json::Value = serde_norway::from_str(yaml).unwrap();
            assert_eq!(b.marcador.metadados.frontmatter, esperado);
            // A linha volta igual.
            let linha = b.marcador.abertura();
            assert_eq!(Marcador::analisar(&linha).unwrap(), b.marcador);
            assert!(p.conteudo.contains(&linha));
            n += 1;
        }
    }
    assert_eq!(n, 11);
}

#[test]
fn evidencias_viram_permalinks_no_github_e_no_gitlab() {
    let gh = gerar(Some(&github()));
    let pagina = |d: &Documentacao| {
        d.paginas
            .iter()
            .find(|p| p.caminho == "regras/faturamento.md")
            .unwrap()
            .conteudo
            .clone()
    };
    assert!(pagina(&gh).contains(
        "[`emitir.sh:1-3`](<https://github.com/exemplo/faturamento/blob/c0ffee1234567890/jobs/emitir.sh#L1-L3> \"jobs/emitir.sh\")"
    ));
    let gl = gerar(Some(&gitlab()));
    assert!(pagina(&gl).contains(
        "(<https://gitlab.example.com/grupo/faturamento/-/blob/c0ffee1234567890/jobs/emitir.sh#L1-3> \"jobs/emitir.sh\")"
    ));
    // Nota de rodapé da análise: permalink sem hash; sem commit próprio, o da
    // análise.
    let a = gl
        .paginas
        .iter()
        .find(|p| p.caminho == "analises/A-0001.md")
        .unwrap();
    assert!(a.conteudo.contains(
        "[^2]: [`db/pkg_cobranca.sql:20-22`](<https://gitlab.example.com/grupo/faturamento/-/blob/c0ffee1234567890/db/pkg_cobranca.sql#L20-22>)\n"
    ), "{}", a.conteudo);
    // Sem forja, o texto de antes (sem hash).
    let sem = gerar(None);
    let p = sem
        .paginas
        .iter()
        .find(|p| p.caminho == "regras/faturamento.md")
        .unwrap();
    assert!(
        p.conteudo
            .contains("**Evidências no código:** `jobs/emitir.sh:1-3`")
    );
    for d in [&gh, &gl, &sem] {
        for p in &d.paginas {
            assert!(!p.conteudo.contains("hash `"), "{}", p.caminho);
            assert!(!p.conteudo.contains("/Users/"), "{}", p.caminho);
        }
    }
}

#[test]
fn sem_mapa_tudo_vai_para_as_areas() {
    let d = gerar_documentacao(
        &documentos(),
        None,
        None,
        &ContextoDaDocumentacao::default(),
    );
    let caminhos: Vec<&str> = d.paginas.iter().map(|p| p.caminho.as_str()).collect();
    assert_eq!(
        caminhos,
        [
            "README.md",
            "analises/A-0001.md",
            "analises/A-0002.md",
            "analises/README.md",
            "funcionalidades/cancelamento/cancelar-fatura-negocial.md",
            "glossario.md",
            "regras/cobranca.md",
            "regras/faturamento.md",
        ]
    );
    let readme = &d.paginas[0].conteudo;
    assert!(readme.starts_with("<!-- Gerado pelo ctower-business-agents"));
    // Sem versão no contexto: a da análise aprovada mais recente.
    assert!(
        readme.contains("**Código descrito:** `main` @ `c0ffee123456`"),
        "{readme}"
    );
}
