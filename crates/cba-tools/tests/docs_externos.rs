//! Sincronização do repositório de docs (#277) com um repositório bare local e o
//! sistema fictício "Faturamento Exemplo". Usa o git de verdade (só Unix, como os
//! testes de escrita do `cba-repos`).
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use cba_core::ambiente::Ambiente;
use cba_core::config::Config;
use cba_core::dados::DiretorioDados;
use cba_kb::externos::EstadoExternos;
use cba_kb::frontmatter::{Confianca, TipoDocumento};
use cba_kb::indice::{Consulta, Externos, Indice, Tipo};
use cba_repos::chaveiro::ChaveiroMemoria;
use cba_repos::demanda::Ferramentas;
use cba_repos::{DestinoDocs, ForgeKind, RepositorioCodigo};
use cba_tools::docs_externos::{self, ErroSincronizacao};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/faturamento-exemplo")
}

/// git real sem a configuração global do analista.
fn git(dir: &Path, args: &[&str]) -> String {
    let o = Command::new("git")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .args([
            "-c",
            "user.name=Teste",
            "-c",
            "user.email=teste@example.com",
            "-c",
            "init.defaultBranch=main",
        ])
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        o.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    String::from_utf8_lossy(&o.stdout).trim_end().to_string()
}

struct Cenario {
    _dir: tempfile::TempDir,
    amb: Ambiente,
    /// Clone de outro analista, que publica no remoto.
    autor: PathBuf,
    f: Ferramentas,
}

fn cenario() -> Cenario {
    let dir = tempfile::tempdir().unwrap();
    let raiz = dunce::canonicalize(dir.path()).unwrap();
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
    let mut amb = Ambiente::do_diretorio(&fixture(), &dados).unwrap();

    let autor = raiz.join("autor");
    std::fs::create_dir_all(autor.join("docs/negocio/processos")).unwrap();
    git(&autor, &["init", "-q"]);
    std::fs::write(autor.join("leia-me.md"), "# Fora da subpasta\n").unwrap();
    std::fs::write(
        autor.join("docs/negocio/processos/segunda-via.md"),
        "# Segunda via do boleto\n\nO cliente pede a segunda via no balcão.\n",
    )
    .unwrap();
    git(&autor, &["add", "-A"]);
    git(&autor, &["commit", "-q", "-m", "docs"]);
    git(&raiz, &["clone", "-q", "--bare", "autor", "remoto.git"]);
    let url = format!("file://{}", raiz.join("remoto.git").display());
    git(&autor, &["remote", "add", "origin", &url]);
    git(&raiz, &["clone", "-q", &url, "clone"]);

    amb.projeto.repositorios.docs = Some(DestinoDocs::Repositorio {
        repositorio: RepositorioCodigo {
            forja: ForgeKind::Github,
            host: "github.com".into(),
            caminho: "exemplo/faturamento-docs".into(),
            url_clone: url,
            pasta: Some(raiz.join("clone")),
            branch: "main".into(),
            ..Default::default()
        },
        subpasta: "docs/negocio".into(),
    });
    let f = Ferramentas {
        chaveiro: Arc::new(ChaveiroMemoria::default()),
        ..Default::default()
    };
    Cenario {
        _dir: dir,
        amb,
        autor,
        f,
    }
}

impl Cenario {
    fn publicar(&self, arquivo: &str, texto: Option<&str>) -> String {
        let caminho = self.autor.join("docs/negocio").join(arquivo);
        match texto {
            Some(t) => {
                std::fs::create_dir_all(caminho.parent().unwrap()).unwrap();
                std::fs::write(&caminho, t).unwrap();
            }
            None => std::fs::remove_file(&caminho).unwrap(),
        }
        git(&self.autor, &["add", "-A"]);
        git(&self.autor, &["commit", "-q", "-m", "docs"]);
        git(&self.autor, &["push", "-q", "origin", "main"]);
        git(&self.autor, &["rev-parse", "HEAD"])
    }

    fn externos(&self) -> Vec<cba_kb::indice::Resultado> {
        let mut indice = Indice::abrir(self.amb.dados.arquivo_do_indice(), &self.amb.base).unwrap();
        indice.sincronizar().unwrap();
        indice
            .listar(&Consulta::nova("").externos(Externos::Somente))
            .unwrap()
            .resultados
    }
}

#[test]
fn sincroniza_importa_e_remove_com_origem_e_commit() {
    let c = cenario();
    let primeiro = git(&c.autor, &["rev-parse", "HEAD"]);
    let s = docs_externos::sincronizar(&c.amb, &c.f).unwrap();
    assert_eq!(s.fonte.repositorio, "GitHub exemplo/faturamento-docs");
    assert_eq!(s.fonte.branch, "main");
    assert_eq!(s.fonte.commit, primeiro);
    assert_eq!(s.importacao.novos, 1);
    assert!(s.aviso_do_indice.is_none());

    let externos = c.externos();
    assert_eq!(externos.len(), 1);
    let r = &externos[0];
    assert_eq!(
        r.caminho.as_str(),
        "faturamento-exemplo/externos/github-exemplo-faturamento-docs/processos/segunda-via.md"
    );
    assert_eq!(r.tipo, Some(Tipo::Documento(TipoDocumento::DocExterno)));
    let o = r.externo.as_ref().unwrap();
    assert_eq!(o.caminho, "docs/negocio/processos/segunda-via.md");
    assert_eq!(o.commit, primeiro);
    assert_eq!(o.confianca, Confianca::Externa);

    // Publicação nova no remoto, com uma regra de outra base e um segredo; o
    // arquivo antigo some.
    c.publicar(
        "regras/RN-FAT-951.md",
        Some(
            "---\ntipo: regra\nid: RN-FAT-951\ntitulo: Multa de contratos antigos\n---\n\n\
             Multa diferenciada. senha: Fatura2026!\n",
        ),
    );
    let segundo = c.publicar("processos/segunda-via.md", None);
    let s = docs_externos::sincronizar(&c.amb, &c.f).unwrap();
    assert!(s.avancou);
    assert_eq!(s.fonte.commit, segundo);
    assert_eq!((s.importacao.novos, s.importacao.removidos), (1, 1));
    assert_eq!(s.importacao.mascarados.len(), 1);
    let externos = c.externos();
    assert_eq!(externos.len(), 1);
    let o = externos[0].externo.as_ref().unwrap();
    assert_eq!(o.confianca, Confianca::Publicada);
    assert_eq!(o.id_original.as_deref(), Some("RN-FAT-951"));
    assert_eq!(o.commit, segundo);
    // Não virou regra da base.
    assert!(!c.amb.kb.raiz().join("regras/RN-FAT-951.md").exists());

    let situacao = docs_externos::situacao(&c.amb);
    assert_eq!(situacao.importados, 1);
    assert!(situacao.aviso.is_none(), "{:?}", situacao.aviso);
}

#[test]
fn falha_de_rede_fica_registrada_e_nao_apaga_as_copias() {
    let mut c = cenario();
    docs_externos::sincronizar(&c.amb, &c.f).unwrap();
    // O remoto sumiu (sem rede, repositório movido).
    if let Some(DestinoDocs::Repositorio { repositorio, .. }) = &c.amb.projeto.repositorios.docs {
        let remoto = repositorio
            .url_clone
            .trim_start_matches("file://")
            .to_string();
        std::fs::remove_dir_all(remoto).unwrap();
    }
    let e = docs_externos::sincronizar(&c.amb, &c.f).unwrap_err();
    assert!(matches!(e, ErroSincronizacao::Falhou(_)), "{e:?}");
    let estado = EstadoExternos::ler(&c.amb.kb).unwrap();
    assert!(estado.erro.is_some());
    assert_eq!(estado.importados(), 1);
    assert_eq!(c.externos().len(), 1);
    let situacao = docs_externos::situacao(&c.amb);
    assert!(
        situacao.aviso.as_deref().unwrap().contains("falhou"),
        "{:?}",
        situacao.aviso
    );
    // A análise não espera nem falha: só recebe o aviso.
    let aviso = docs_externos::ao_iniciar_analise_com(&c.amb, c.f.clone()).unwrap();
    assert!(aviso.contains("desatualizada"), "{aviso}");

    // Sem destino, nada a fazer.
    c.amb.projeto.repositorios.docs = None;
    assert_eq!(
        docs_externos::sincronizar(&c.amb, &c.f).unwrap_err(),
        ErroSincronizacao::SemDestino
    );
    assert!(docs_externos::ao_iniciar_analise_com(&c.amb, c.f.clone()).is_none());
}

// ---- Páginas da documentação legível (#288), recortadas na importação (#290) ----

/// Base de exemplo da documentação legível (a mesma dos testes do renderizador).
fn base_da_documentacao() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../cba-docs/tests/documentacao/base")
}

/// As páginas que o renderizador da #288 gera da base de exemplo.
fn paginas_geradas() -> Vec<(String, String)> {
    fn visitar(pasta: &Path, raiz: &Path, saida: &mut Vec<cba_docs::DocumentoDaBase>) {
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
                saida.push(cba_docs::DocumentoDaBase {
                    caminho,
                    texto: std::fs::read_to_string(&p).unwrap(),
                });
            }
        }
    }
    let raiz = base_da_documentacao();
    let mut documentos = Vec::new();
    visitar(&raiz, &raiz, &mut documentos);
    let mapa: cba_core::funcionalidades::MapaFuncionalidades = serde_json::from_slice(
        &std::fs::read(raiz.join("funcionalidades/mapa.estado.json")).unwrap(),
    )
    .unwrap();
    let contexto = cba_docs::ContextoDaDocumentacao {
        projeto: "Faturamento Exemplo".into(),
        areas: Default::default(),
        codigo: None,
        data: None,
    };
    cba_docs::gerar_documentacao(&documentos, Some(&mapa), None, &contexto)
        .paginas
        .into_iter()
        .map(|p| (p.caminho, p.conteudo))
        .collect()
}

const PAGINA_COB: &str = "funcionalidades/cobranca/f-cob-001-cobrar-multa-por-atraso.md";
const BLOCOS_COB: &str = "externos/github-exemplo-faturamento-docs/funcionalidades/cobranca/\
                          f-cob-001-cobrar-multa-por-atraso";

impl Cenario {
    /// Grava as páginas na subpasta de docs do autor e publica num commit só.
    fn publicar_paginas(&self, paginas: &[(String, String)]) -> String {
        for (caminho, texto) in paginas {
            let destino = self.autor.join("docs/negocio").join(caminho);
            std::fs::create_dir_all(destino.parent().unwrap()).unwrap();
            std::fs::write(destino, texto).unwrap();
        }
        git(&self.autor, &["add", "-A"]);
        git(&self.autor, &["commit", "-q", "-m", "docs"]);
        git(&self.autor, &["push", "-q", "origin", "main"]);
        git(&self.autor, &["rev-parse", "HEAD"])
    }

    fn todos_os_externos(&self) -> Vec<cba_kb::indice::Resultado> {
        let mut indice = Indice::abrir(self.amb.dados.arquivo_do_indice(), &self.amb.base).unwrap();
        indice.sincronizar().unwrap();
        indice
            .listar(
                &Consulta::nova("")
                    .externos(Externos::Somente)
                    .pagina(500, 0),
            )
            .unwrap()
            .resultados
    }
}

type Chave = (String, Option<String>, Option<String>);

/// `(caminho#âncora, tipo, id)` de cada externo esperado das páginas: um por
/// bloco, ou a página inteira quando ela não tem marcador.
fn esperados(paginas: &[(String, String)]) -> Vec<Chave> {
    let mut saida = Vec::new();
    for (caminho, texto) in paginas {
        let blocos = cba_kb::marcador::blocos(texto).unwrap();
        if blocos.is_empty() {
            saida.push((format!("docs/negocio/{caminho}"), None, None));
        }
        for b in blocos {
            let f = &b.marcador.metadados.frontmatter;
            let id = f["id"]
                .as_str()
                .or_else(|| f["termo"].as_str())
                .unwrap_or(&b.marcador.id);
            saida.push((
                format!("docs/negocio/{caminho}#{}", b.ancora().unwrap()),
                Some(b.marcador.tipo.clone()),
                Some(id.to_string()),
            ));
        }
    }
    saida.sort();
    saida
}

fn obtidos(externos: &[cba_kb::indice::Resultado]) -> Vec<Chave> {
    let mut saida: Vec<_> = externos
        .iter()
        .map(|r| {
            let o = r.externo.as_ref().unwrap();
            (
                o.caminho_com_ancora(),
                o.tipo_original.clone(),
                o.id_original.clone(),
            )
        })
        .filter(|(c, _, _)| c != "docs/negocio/processos/segunda-via.md")
        .collect();
    saida.sort();
    saida
}

/// A página sem o bloco do documento `id`.
fn sem_bloco(pagina: &str, id: &str) -> String {
    let abertura = format!("<!-- cba:regra id={id} ");
    let mut dentro = false;
    let mut saida = String::new();
    for linha in pagina.lines() {
        if linha.starts_with(&abertura) {
            dentro = true;
            continue;
        }
        if dentro {
            dentro = linha.trim() != cba_kb::marcador::FECHAMENTO;
            continue;
        }
        saida.push_str(linha);
        saida.push('\n');
    }
    saida
}

#[test]
fn paginas_geradas_viram_um_externo_por_documento_e_voltam_iguais() {
    let c = cenario();
    let paginas = paginas_geradas();
    assert!(paginas.iter().any(|(c, _)| c == PAGINA_COB));
    let commit = c.publicar_paginas(&paginas);
    let s = docs_externos::sincronizar(&c.amb, &c.f).unwrap();
    assert!(s.importacao.avisos.is_empty(), "{:?}", s.importacao.avisos);

    let externos = c.todos_os_externos();
    assert_eq!(obtidos(&externos), esperados(&paginas));

    // A página da funcionalidade com N regras vira N regras externas, com âncora
    // e o link do trecho na forja.
    let regras: Vec<_> = externos
        .iter()
        .filter_map(|r| r.externo.as_ref())
        .filter(|o| o.caminho.ends_with(PAGINA_COB) && o.tipo_original.as_deref() == Some("regra"))
        .collect();
    assert_eq!(regras.len(), 3);
    for o in &regras {
        let ancora = o.ancora.as_deref().unwrap();
        assert_eq!(ancora, o.id_original.as_deref().unwrap().to_lowercase());
        assert_eq!(
            o.url.as_deref().unwrap(),
            format!(
                "https://github.com/exemplo/faturamento-docs/blob/{commit}/docs/negocio/\
                 {PAGINA_COB}#{ancora}"
            )
        );
        assert_eq!(o.confianca, Confianca::Publicada);
    }

    // Ida e volta: o frontmatter da cópia é o do documento na base de origem.
    let copia: cba_kb::Documento<cba_kb::frontmatter::FrontmatterDocExterno> = c
        .amb
        .kb
        .ler(&cba_kb::CaminhoRelativo::novo(&format!("{BLOCOS_COB}/regra-RN-COB-001.md")).unwrap())
        .unwrap();
    let original: cba_kb::Documento<cba_kb::frontmatter::FrontmatterLivre> =
        cba_kb::Documento::analisar(
            &std::fs::read_to_string(base_da_documentacao().join("regras/RN-COB-001.md")).unwrap(),
        )
        .unwrap();
    assert_eq!(
        copia.frontmatter.frontmatter_original.as_ref(),
        Some(&original.frontmatter.0)
    );
    assert!(!copia.corpo.contains("<!--"));
    assert!(!copia.corpo.contains("<a id"));
}

#[test]
fn regra_removida_da_pagina_sai_alterada_atualiza_e_a_da_base_nao_entra() {
    let c = cenario();
    // A base deste projeto já tem a RN-COB-002 (foi ela quem publicou).
    let regra =
        std::fs::read_to_string(base_da_documentacao().join("regras/RN-COB-002.md")).unwrap();
    c.amb
        .kb
        .gravar_texto(
            &cba_kb::CaminhoRelativo::novo("regras/RN-COB-002.md").unwrap(),
            &regra,
        )
        .unwrap();

    let paginas = paginas_geradas();
    c.publicar_paginas(&paginas);
    let s = docs_externos::sincronizar(&c.amb, &c.f).unwrap();
    assert!(
        s.importacao
            .ja_na_base
            .contains(&format!("docs/negocio/{PAGINA_COB}#rn-cob-002")),
        "{:?}",
        s.importacao.ja_na_base
    );
    let raiz = c.amb.kb.raiz().join(BLOCOS_COB);
    assert!(!raiz.join("regra-RN-COB-002.md").exists());
    assert!(raiz.join("regra-RN-COB-001.md").exists());
    let antes = c.todos_os_externos().len();

    // Sai a RN-COB-003 da página e muda o texto da RN-COB-001.
    let texto = paginas
        .iter()
        .find(|(c, _)| c == PAGINA_COB)
        .unwrap()
        .1
        .clone();
    assert!(texto.contains("<!-- cba:regra id=RN-COB-003 "));
    let texto = sem_bloco(&texto, "RN-COB-003").replacen("2%", "3%", 1);
    let commit = c.publicar_paginas(&[(PAGINA_COB.to_string(), texto)]);
    let s = docs_externos::sincronizar(&c.amb, &c.f).unwrap();
    assert_eq!(
        (
            s.importacao.removidos,
            s.importacao.alterados,
            s.importacao.novos
        ),
        (1, 1, 0),
        "{:?}",
        s.importacao
    );
    assert!(!raiz.join("regra-RN-COB-003.md").exists());
    let alterada: cba_kb::Documento<cba_kb::frontmatter::FrontmatterDocExterno> = c
        .amb
        .kb
        .ler(&cba_kb::CaminhoRelativo::novo(&format!("{BLOCOS_COB}/regra-RN-COB-001.md")).unwrap())
        .unwrap();
    assert!(alterada.frontmatter.titulo.contains("3%"));
    assert_eq!(alterada.frontmatter.origem.commit, commit);
    let externos = c.todos_os_externos();
    assert_eq!(externos.len(), antes - 1);
    assert!(
        !externos
            .iter()
            .any(|r| r.externo.as_ref().unwrap().id_original.as_deref() == Some("RN-COB-003"))
    );
}
