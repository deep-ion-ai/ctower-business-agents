//! Testes da publicação dos documentos aprovados (#276): a seleção do aprovado, o
//! cálculo das mudanças e, no Unix, a publicação num repositório bare local com
//! o gh falso, nos dois modos de destino.

use super::*;
use cba_kb::Base;
use cba_repos::escrita::{ArvoreRemota, FormatoObjeto};
use cba_repos::{DestinoDocs, RepositorioCodigo};

const PROJETO: &str = "faturamento-exemplo";

fn doc(tipo: &str, status: Option<&str>, titulo: &str) -> String {
    let status = status.map(|s| format!("status: {s}\n")).unwrap_or_default();
    format!("---\ntipo: {tipo}\n{status}---\n\n# {titulo}\n")
}

#[test]
fn so_o_aprovado_e_publicado() {
    for (tipo, status, esperado) in [
        ("analise-impacto", Some("aprovada"), true),
        ("analise-impacto", Some("rascunho"), false),
        ("analise-impacto", Some("em-revisao"), false),
        ("historias", Some("aprovada"), true),
        ("cenarios-teste", Some("aprovada"), true),
        ("funcionalidade", Some("aprovada"), true),
        ("caso-de-uso", Some("rascunho"), false),
        ("mapa-funcionalidades", Some("aprovada"), true),
        ("regra", Some("confirmada"), true),
        ("regra", Some("a-revalidar"), false),
        ("regra", Some("obsoleta"), false),
        ("termo", None, true),
        ("sugestao", Some("aceita"), true),
        ("sugestao", Some("proposta"), false),
        // Dados pessoais, trabalho em andamento ou configuração local.
        ("pessoa", None, false),
        ("ata", Some("aprovada"), false),
        ("analise-reuniao", Some("aprovada"), false),
        ("demanda", Some("encerrada"), false),
        ("decisao", Some("aprovada"), false),
        ("memoria-area", None, false),
        ("projeto", None, false),
        ("desconhecido", Some("aprovada"), false),
    ] {
        assert_eq!(
            aprovado(&doc(tipo, status, "Emissão")),
            esperado,
            "{tipo} {status:?}"
        );
    }
    assert!(!aprovado("# Sem frontmatter\n"));
    assert!(!aprovado("---\ntipo: [\n---\n"));
}

/// Projeto do Faturamento Exemplo com documentos aprovados e não aprovados.
fn base() -> (tempfile::TempDir, Projeto) {
    let tmp = tempfile::tempdir().unwrap();
    let raiz = tmp.path().join(PROJETO);
    let arquivos = [
        ("projeto.md", doc("projeto", None, "Faturamento Exemplo")),
        (
            "regras/RN-FAT-001.md",
            doc("regra", Some("confirmada"), "Cancelar nota"),
        ),
        (
            "regras/RN-FAT-002.md",
            doc("regra", Some("obsoleta"), "Regra antiga"),
        ),
        (
            "glossario/nota-fiscal.md",
            doc("termo", None, "Nota fiscal"),
        ),
        (
            "analises/A-0001.md",
            doc("analise-impacto", Some("aprovada"), "Cancelamento"),
        ),
        (
            "analises/A-0002.md",
            doc("analise-impacto", Some("rascunho"), "Em andamento"),
        ),
        ("analises/A-0001.memoria.md", "memória\n".into()),
        ("pessoas/fulana.md", doc("pessoa", None, "Fulana")),
    ];
    for (c, t) in arquivos {
        let p = raiz.join(c);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, t).unwrap();
    }
    let projeto = Base::abrir(tmp.path()).unwrap().projeto(PROJETO).unwrap();
    (tmp, projeto)
}

#[test]
fn documentos_aprovados_mantem_o_layout_e_o_conteudo() {
    let (_tmp, projeto) = base();
    let docs = documentos_aprovados(&projeto).unwrap();
    let caminhos: Vec<&str> = docs.iter().map(|d| d.caminho.as_str()).collect();
    assert_eq!(
        caminhos,
        [
            "analises/A-0001.md",
            "glossario/nota-fiscal.md",
            "regras/RN-FAT-001.md"
        ]
    );
    assert_eq!(
        docs[2].conteudo,
        doc("regra", Some("confirmada"), "Cancelar nota").into_bytes()
    );
}

fn repo() -> RepositorioCodigo {
    RepositorioCodigo {
        caminho: "exemplo/faturamento".into(),
        ..Default::default()
    }
}

fn aprovados(itens: &[(&str, &str)]) -> Vec<DocumentoAprovado> {
    itens
        .iter()
        .map(|(c, t)| DocumentoAprovado {
            caminho: (*c).into(),
            conteudo: t.as_bytes().to_vec(),
        })
        .collect()
}

/// Leitura do remoto com os arquivos (caminho no repositório, conteúdo) na base
/// e, se houver, na branch da publicação.
fn remoto(base: &[(&str, &str)], branch: Option<&[(&str, &str)]>) -> LeituraRemota {
    let arvore = |arquivos: &[(&str, &str)], commit: &str| ArvoreRemota {
        commit: commit.into(),
        arquivos: arquivos
            .iter()
            .map(|(c, t)| {
                (
                    (*c).to_string(),
                    FormatoObjeto::Sha1.id_do_blob(t.as_bytes()),
                )
            })
            .collect(),
        conteudos: arquivos
            .iter()
            .filter(|(c, _)| c.ends_with(MANIFESTO))
            .map(|(c, t)| ((*c).to_string(), t.as_bytes().to_vec()))
            .collect(),
    };
    LeituraRemota {
        base: "main".into(),
        na_base: arvore(base, "c1"),
        na_branch: branch.map(|b| arvore(b, "c2")),
        formato: FormatoObjeto::Sha1,
    }
}

fn manifesto(projeto: &str, docs: &[&str]) -> String {
    let m = Manifesto {
        gerado_por: GERADO_POR.into(),
        projeto: projeto.into(),
        documentos: docs.iter().map(|d| d.to_string()).collect(),
    };
    serde_json::to_string_pretty(&m).unwrap() + "\n"
}

fn previa_de(p: &Plano) -> Vec<(TipoMudanca, &str)> {
    p.previa
        .iter()
        .map(|m| (m.tipo, m.caminho.as_str()))
        .collect()
}

#[test]
fn novos_alterados_e_removidos_na_subpasta() {
    let r = repo();
    let destino = DestinoResolvido {
        repositorio: &r,
        subpasta: "docs/negocio",
        independente: false,
    };
    let docs = aprovados(&[
        ("regras/RN-FAT-001.md", "# Cancelar nota (v2)\n"),
        ("glossario/nota-fiscal.md", "# Nota fiscal\n"),
        ("analises/A-0001.md", "# Cancelamento\n"),
    ]);
    let m = manifesto(
        PROJETO,
        &[
            "regras/RN-FAT-001.md",
            "glossario/nota-fiscal.md",
            "regras/RN-FAT-002.md",
            "../../fora.md",
        ],
    );
    let l = remoto(
        &[
            ("docs/negocio/.cba-publicacao.json", &m),
            ("docs/negocio/regras/RN-FAT-001.md", "# Cancelar nota\n"),
            ("docs/negocio/glossario/nota-fiscal.md", "# Nota fiscal\n"),
            ("docs/negocio/regras/RN-FAT-002.md", "# Regra antiga\n"),
            // Escrito à mão: não está no manifesto, fica.
            ("docs/negocio/leia-me.md", "# Docs do Faturamento\n"),
        ],
        None,
    );
    let p = planejar(PROJETO, &docs, &destino, &l).unwrap();
    assert_eq!(
        previa_de(&p),
        [
            (TipoMudanca::Novo, "docs/negocio/analises/A-0001.md"),
            (TipoMudanca::Alterado, "docs/negocio/regras/RN-FAT-001.md"),
            (TipoMudanca::Removido, "docs/negocio/regras/RN-FAT-002.md"),
        ]
    );
    assert!(p.muda_a_base && !p.pendente_na_branch);
    let caminhos: Vec<(&str, bool)> = p
        .mudancas
        .iter()
        .map(|m| (m.caminho(), matches!(m, Mudanca::Remover { .. })))
        .collect();
    assert_eq!(
        caminhos,
        [
            ("docs/negocio/analises/A-0001.md", false),
            ("docs/negocio/glossario/nota-fiscal.md", false),
            ("docs/negocio/regras/RN-FAT-001.md", false),
            ("docs/negocio/.cba-publicacao.json", false),
            ("docs/negocio/regras/RN-FAT-002.md", true),
        ]
    );
    // O manifesto novo lista os aprovados, relativos à pasta.
    let Mudanca::Gravar { conteudo, .. } = &p.mudancas[3] else {
        panic!()
    };
    let novo: Manifesto = serde_json::from_slice(conteudo).unwrap();
    assert_eq!(novo.projeto, PROJETO);
    assert_eq!(
        novo.documentos,
        [
            "regras/RN-FAT-001.md",
            "glossario/nota-fiscal.md",
            "analises/A-0001.md"
        ]
    );
}

#[test]
fn nada_a_publicar_quando_a_base_ja_tem_os_aprovados() {
    let r = repo();
    let destino = DestinoResolvido {
        repositorio: &r,
        subpasta: "",
        independente: true,
    };
    let docs = aprovados(&[("regras/RN-FAT-001.md", "# Cancelar nota\n")]);
    let m = manifesto(PROJETO, &["regras/RN-FAT-001.md"]);
    let base = [
        (".cba-publicacao.json", m.as_str()),
        ("regras/RN-FAT-001.md", "# Cancelar nota\n"),
    ];
    let p = planejar(PROJETO, &docs, &destino, &remoto(&base, None)).unwrap();
    assert!(p.previa.is_empty());
    assert!(!p.muda_a_base && !p.pendente_na_branch);
    // A branch da publicação igual à base (PR mesclado) também não pende.
    let p = planejar(PROJETO, &docs, &destino, &remoto(&base, Some(&base))).unwrap();
    assert!(!p.muda_a_base && !p.pendente_na_branch);

    // Base sem nada aprovado e nada publicado: nenhuma mudança, nem o manifesto.
    let p = planejar(PROJETO, &[], &destino, &remoto(&[], None)).unwrap();
    assert!(p.mudancas.is_empty() && p.previa.is_empty() && !p.muda_a_base);
}

#[test]
fn branch_pendente_remove_o_que_ela_publicou() {
    let r = repo();
    let destino = DestinoResolvido {
        repositorio: &r,
        subpasta: "docs",
        independente: false,
    };
    // A branch tem um PR aberto com a A-0001, que voltou a rascunho.
    let m = manifesto(PROJETO, &["analises/A-0001.md"]);
    let branch = [
        ("docs/.cba-publicacao.json", m.as_str()),
        ("docs/analises/A-0001.md", "# Cancelamento\n"),
    ];
    let p = planejar(PROJETO, &[], &destino, &remoto(&[], Some(&branch))).unwrap();
    assert!(p.previa.is_empty() && p.pendente_na_branch);
    assert!(
        p.mudancas
            .contains(&Mudanca::remover("docs/analises/A-0001.md"))
    );
}

#[test]
fn nunca_sobrescreve_documento_escrito_a_mao() {
    let r = repo();
    let destino = DestinoResolvido {
        repositorio: &r,
        subpasta: "docs/negocio",
        independente: false,
    };
    let docs = aprovados(&[("glossario/nota-fiscal.md", "# Nota fiscal\n")]);
    let l = remoto(
        &[("docs/negocio/glossario/nota-fiscal.md", "# Escrito à mão\n")],
        None,
    );
    let e = planejar(PROJETO, &docs, &destino, &l).unwrap_err();
    assert!(
        e.contains("docs/negocio/glossario/nota-fiscal.md") && e.contains("à mão"),
        "{e}"
    );
    // Com o mesmo conteúdo, não há o que sobrescrever.
    let l = remoto(
        &[("docs/negocio/glossario/nota-fiscal.md", "# Nota fiscal\n")],
        None,
    );
    let p = planejar(PROJETO, &docs, &destino, &l).unwrap();
    assert!(p.previa.is_empty());
}

#[test]
fn manifesto_de_outro_projeto_ou_ilegivel_e_recusado() {
    let r = repo();
    let destino = DestinoResolvido {
        repositorio: &r,
        subpasta: "docs",
        independente: false,
    };
    let outro = manifesto("estoque-exemplo", &[]);
    let e = planejar(
        PROJETO,
        &[],
        &destino,
        &remoto(&[("docs/.cba-publicacao.json", &outro)], None),
    )
    .unwrap_err();
    assert!(e.contains("estoque-exemplo"), "{e}");
    let e = planejar(
        PROJETO,
        &[],
        &destino,
        &remoto(&[("docs/.cba-publicacao.json", "{")], None),
    )
    .unwrap_err();
    assert!(e.contains("ilegível"), "{e}");
}

#[test]
fn branch_da_publicacao_por_projeto() {
    assert_eq!(
        branch_da_publicacao(PROJETO),
        "docs/cba-faturamento-exemplo"
    );
    validar_branch_de_escrita(&branch_da_publicacao(PROJETO), Some("main")).unwrap();
}

#[test]
fn sem_destino_explica_o_que_fazer() {
    let (_tmp, projeto) = base();
    let repositorios = Repositorios::default();
    let e = previa(
        &Ferramentas::default(),
        &Alvo {
            projeto: &projeto,
            nome: "Faturamento Exemplo",
            repositorios: &repositorios,
        },
    )
    .unwrap_err();
    assert!(e.contains("destino da documentação"), "{e}");
    // Sem clone local do repositório da documentação.
    let repositorios = Repositorios {
        docs: Some(DestinoDocs::Repositorio {
            repositorio: RepositorioCodigo {
                caminho: "exemplo/faturamento-docs".into(),
                ..Default::default()
            },
            subpasta: String::new(),
        }),
        ..Default::default()
    };
    let e = previa(
        &Ferramentas::default(),
        &Alvo {
            projeto: &projeto,
            nome: "Faturamento Exemplo",
            repositorios: &repositorios,
        },
    )
    .unwrap_err();
    assert!(e.contains("clone"), "{e}");
}

#[cfg(unix)]
mod com_git {
    //! Repositório bare local como remoto e gh falso que guarda o PR aberto.
    use super::*;
    use cba_repos::chaveiro::{Chaveiro, ChaveiroMemoria, conta_global};
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::Arc;

    const TOKEN: &str = "ghp-segredo-da-publicacao";

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

    fn script(dir: &Path, nome: &str, corpo: &str) -> PathBuf {
        let p = dir.join(nome);
        std::fs::write(&p, format!("#!/bin/sh\n{corpo}\n")).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p
    }

    struct Cenario {
        _tmp: tempfile::TempDir,
        _base: tempfile::TempDir,
        projeto: Projeto,
        remoto: PathBuf,
        forja: PathBuf,
        repositorios: Repositorios,
        f: Ferramentas,
    }

    /// Remoto com um `leia-me.md` na pasta da documentação (escrito à mão) e
    /// clone do analista; `subpasta` vazia = repositório próprio da documentação.
    fn cenario(subpasta: &str) -> Cenario {
        let tmp = tempfile::tempdir().unwrap();
        let raiz = dunce::canonicalize(tmp.path()).unwrap();
        let semente = raiz.join("semente");
        let pasta_docs = if subpasta.is_empty() {
            semente.clone()
        } else {
            semente.join(subpasta)
        };
        std::fs::create_dir_all(&pasta_docs).unwrap();
        git(&semente, &["init", "-q"]);
        std::fs::write(pasta_docs.join("leia-me.md"), "# Docs do Faturamento\n").unwrap();
        git(&semente, &["add", "-A"]);
        git(&semente, &["commit", "-q", "-m", "base"]);
        let remoto = raiz.join("remoto.git");
        git(&raiz, &["clone", "-q", "--bare", "semente", "remoto.git"]);
        let url = format!("file://{}", remoto.display());
        git(&raiz, &["clone", "-q", &url, "clone"]);
        let forja = raiz.join("forja");
        std::fs::create_dir_all(&forja).unwrap();
        let bin = raiz.join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let git_app = script(
            &bin,
            "git",
            "GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 exec git \"$@\"",
        );
        let gh = script(
            &bin,
            "gh",
            &format!(
                r#"d='{d}'
case "$*" in *{TOKEN}*) echo "token nos argumentos" >&2; exit 9;; esac
[ "$GH_TOKEN" = "{TOKEN}" ] || {{ echo "sem token" >&2; exit 8; }}
echo "$*" >> "$d/chamadas"
case "$*" in
  *"--method POST"*) echo 1 > "$d/aberto"; echo '{{"html_url":"https://github.com/exemplo/faturamento/pull/1"}}';;
  *pulls\?*) if [ -f "$d/aberto" ]; then echo '[{{"html_url":"https://github.com/exemplo/faturamento/pull/1"}}]'; else echo '[]'; fi;;
  *) echo "rota inesperada: $*" >&2; exit 3;;
esac"#,
                d = forja.display()
            ),
        );
        let chaveiro = ChaveiroMemoria::default();
        chaveiro
            .gravar(&conta_global("github.com"), &Secret::new(TOKEN))
            .unwrap();
        let f = Ferramentas {
            git: git_app,
            gh,
            chaveiro: Arc::new(chaveiro),
            ..Default::default()
        };
        let repo = RepositorioCodigo {
            caminho: "exemplo/faturamento".into(),
            url_clone: url,
            pasta: Some(raiz.join("clone")),
            branch: "main".into(),
            ..Default::default()
        };
        let repositorios = if subpasta.is_empty() {
            Repositorios {
                docs: Some(DestinoDocs::Repositorio {
                    repositorio: repo,
                    subpasta: String::new(),
                }),
                ..Default::default()
            }
        } else {
            Repositorios {
                docs: Some(DestinoDocs::Subpasta {
                    codigo: repo.chave(),
                    subpasta: subpasta.into(),
                }),
                codigo: vec![repo],
                ..Default::default()
            }
        };
        let (b, projeto) = base();
        Cenario {
            _tmp: tmp,
            _base: b,
            projeto,
            remoto,
            forja,
            repositorios,
            f,
        }
    }

    impl Cenario {
        fn alvo(&self) -> Alvo<'_> {
            Alvo {
                projeto: &self.projeto,
                nome: "Faturamento Exemplo",
                repositorios: &self.repositorios,
            }
        }

        fn aberturas(&self) -> usize {
            std::fs::read_to_string(self.forja.join("chamadas"))
                .unwrap_or_default()
                .matches("--method POST")
                .count()
        }

        /// Mescla a branch da publicação na base do remoto (o PR aprovado).
        fn mesclar(&self) {
            let branch = branch_da_publicacao(PROJETO);
            git(&self.remoto, &["update-ref", "refs/heads/main", &branch]);
            let _ = std::fs::remove_file(self.forja.join("aberto"));
        }
    }

    fn publicar_e_conferir(subpasta: &str) {
        let c = cenario(subpasta);
        let pref = |c: &str| {
            if subpasta.is_empty() {
                c.to_string()
            } else {
                format!("{subpasta}/{c}")
            }
        };
        // Prévia: os aprovados entram como novos; o leia-me não aparece.
        let p = previa(&c.f, &c.alvo()).unwrap();
        assert_eq!(p.branch, "docs/cba-faturamento-exemplo");
        assert_eq!(p.base, "main");
        assert_eq!(p.aprovados, 3);
        assert_eq!(p.independente, subpasta.is_empty());
        assert!(!p.nada_a_publicar);
        let novos: Vec<&str> = p
            .mudancas
            .iter()
            .filter(|m| m.tipo == TipoMudanca::Novo)
            .map(|m| m.caminho.as_str())
            .collect();
        assert_eq!(
            novos,
            [
                pref("analises/A-0001.md"),
                pref("glossario/nota-fiscal.md"),
                pref("regras/RN-FAT-001.md")
            ]
        );
        assert_eq!(p.mudancas.len(), 3);
        assert_eq!(c.aberturas(), 0, "a prévia não abre PR");

        // Publica: abre o PR com os arquivos e o manifesto.
        let r = publicar(&c.f, &c.alvo()).unwrap();
        assert_eq!(r.publicado.situacao, Situacao::Aberto);
        assert_eq!(
            r.publicado.link.as_deref(),
            Some("https://github.com/exemplo/faturamento/pull/1")
        );
        let branch = branch_da_publicacao(PROJETO);
        let arvore = git(&c.remoto, &["ls-tree", "-r", "--name-only", &branch]);
        let esperado: Vec<String> = [
            ".cba-publicacao.json",
            "analises/A-0001.md",
            "glossario/nota-fiscal.md",
            "leia-me.md",
            "regras/RN-FAT-001.md",
        ]
        .iter()
        .map(|c| pref(c))
        .collect();
        assert_eq!(arvore.lines().collect::<Vec<_>>(), esperado);
        assert_eq!(
            git(
                &c.remoto,
                &[
                    "show",
                    &format!("{branch}:{}", pref("regras/RN-FAT-001.md"))
                ]
            ),
            doc("regra", Some("confirmada"), "Cancelar nota").trim_end()
        );

        // Republicar sem mudanças: nada de commit nem PR novo.
        let r = publicar(&c.f, &c.alvo()).unwrap();
        assert_eq!(r.publicado.situacao, Situacao::SemMudancas);
        assert_eq!(c.aberturas(), 1);

        // PR mesclado: a prévia diz que não há nada a publicar.
        c.mesclar();
        let p = previa(&c.f, &c.alvo()).unwrap();
        assert!(p.nada_a_publicar && p.mudancas.is_empty(), "{p:?}");
        let r = publicar(&c.f, &c.alvo()).unwrap();
        assert_eq!(r.publicado.situacao, Situacao::SemMudancas);
        assert_eq!(c.aberturas(), 1);

        // A regra deixa de ser aprovada e a análise muda: removida e alterada; o
        // leia-me escrito à mão continua.
        let raiz = c.projeto.raiz();
        std::fs::write(
            raiz.join("regras/RN-FAT-001.md"),
            doc("regra", Some("obsoleta"), "Cancelar nota"),
        )
        .unwrap();
        std::fs::write(
            raiz.join("analises/A-0001.md"),
            doc(
                "analise-impacto",
                Some("aprovada"),
                "Cancelamento (revisada)",
            ),
        )
        .unwrap();
        let p = previa(&c.f, &c.alvo()).unwrap();
        let mudancas: Vec<(TipoMudanca, String)> = p
            .mudancas
            .iter()
            .map(|m| (m.tipo, m.caminho.clone()))
            .collect();
        assert_eq!(
            mudancas,
            [
                (TipoMudanca::Alterado, pref("analises/A-0001.md")),
                (TipoMudanca::Removido, pref("regras/RN-FAT-001.md")),
            ]
        );
        let r = publicar(&c.f, &c.alvo()).unwrap();
        assert_eq!(r.publicado.situacao, Situacao::Aberto);
        let arvore = git(&c.remoto, &["ls-tree", "-r", "--name-only", &branch]);
        assert!(!arvore.contains("RN-FAT-001"), "{arvore}");
        assert!(arvore.contains(&pref("leia-me.md")), "{arvore}");
    }

    #[test]
    fn publica_numa_subpasta_do_repositorio_de_codigo() {
        publicar_e_conferir("docs/negocio");
    }

    #[test]
    fn publica_num_repositorio_proprio_da_documentacao() {
        publicar_e_conferir("");
    }
}
