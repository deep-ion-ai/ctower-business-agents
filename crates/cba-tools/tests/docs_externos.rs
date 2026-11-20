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
