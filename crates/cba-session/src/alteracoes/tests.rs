//! Testes do aviso de alteração com um repositório git temporário com o projeto
//! fictício (`fixtures/faturamento-exemplo`).

use std::process::Command;

use super::*;

const SESSAO: &str = "3f2b8c1e-9a7d-4e2f-b1c3-0d4e5f6a7b8c";

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/faturamento-exemplo")
        .canonicalize()
        .unwrap()
}

fn copiar(de: &Path, para: &Path) {
    fs::create_dir_all(para).unwrap();
    for e in fs::read_dir(de).unwrap() {
        let e = e.unwrap();
        let destino = para.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copiar(&e.path(), &destino);
        } else {
            fs::copy(e.path(), destino).unwrap();
        }
    }
}

fn git(raiz: &Path, args: &[&str]) {
    let ok = Command::new("git")
        .arg("-C")
        .arg(raiz)
        .args(args)
        .env("GIT_AUTHOR_NAME", "Teste")
        .env("GIT_AUTHOR_EMAIL", "teste@example.com")
        .env("GIT_COMMITTER_NAME", "Teste")
        .env("GIT_COMMITTER_EMAIL", "teste@example.com")
        .output()
        .unwrap();
    assert!(ok.status.success(), "git {args:?}: {ok:?}");
}

struct Cenario {
    _dir: tempfile::TempDir,
    codigo: PathBuf,
    retratos: Retratos,
}

impl Cenario {
    /// O sistema fictício num repositório git com um commit.
    fn com_git() -> Self {
        let c = Self::sem_git();
        git(&c.codigo, &["init", "-q"]);
        git(&c.codigo, &["config", "core.autocrlf", "false"]);
        git(&c.codigo, &["add", "-A"]);
        git(&c.codigo, &["commit", "-q", "-m", "inicial"]);
        c
    }

    fn sem_git() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let raiz = dunce::canonicalize(dir.path()).unwrap();
        let codigo = raiz.join("faturamento");
        copiar(&fixture(), &codigo);
        Self {
            retratos: Retratos::em(raiz.join("dados").join(PASTA)),
            _dir: dir,
            codigo,
        }
    }

    fn arquivo(&self, rel: &str) -> PathBuf {
        no_disco(&self.codigo, rel).unwrap()
    }

    fn ler(&self, rel: &str) -> String {
        fs::read_to_string(self.arquivo(rel)).unwrap()
    }

    fn escrever(&self, rel: &str, texto: &str) {
        fs::write(self.arquivo(rel), texto).unwrap();
    }

    fn acrescentar(&self, rel: &str, texto: &str) {
        let atual = self.ler(rel);
        self.escrever(rel, &format!("{atual}{texto}"));
    }

    fn retratar(&self) -> Retrato {
        retratar(
            &self.retratos,
            SESSAO,
            "faturamento-exemplo",
            std::slice::from_ref(&self.codigo),
        )
        .unwrap()
    }

    fn conferir(&self) -> Vec<(String, TipoAlteracao)> {
        conferir(&self.retratos, SESSAO)
            .unwrap()
            .alterados
            .into_iter()
            .map(|a| (a.caminho, a.tipo))
            .collect()
    }

    fn detalhe(&self, rel: &str) -> Detalhe {
        detalhar(&self.retratos, SESSAO)
            .unwrap()
            .0
            .into_iter()
            .find(|d| d.alterado.caminho == rel)
            .unwrap_or_else(|| panic!("{rel} não está no aviso"))
    }

    fn pedidos(&self) -> Vec<Pedido> {
        conferir(&self.retratos, SESSAO)
            .unwrap()
            .alterados
            .into_iter()
            .map(|a| Pedido {
                sessao: SESSAO.into(),
                pasta: a.pasta,
                caminho: a.caminho,
                hash_atual: a.hash_atual,
            })
            .collect()
    }
}

#[test]
fn alteracao_da_sessao_aparece_com_o_diff_e_a_anterior_nao() {
    let c = Cenario::com_git();
    // Antes da sessão: o analista já tinha mexido em dois arquivos.
    c.acrescentar("db/02_parametros.sql", "-- ajuste local do analista\n");
    c.escrever("anotacoes.txt", "rascunho do analista\n");
    let antes_parametros = c.ler("db/02_parametros.sql");

    let retrato = c.retratar();
    assert_eq!(retrato.repositorios.len(), 1);
    assert_eq!(retrato.repositorios[0].raiz, c.codigo);
    assert!(retrato.repositorios[0].head.is_some());
    assert_eq!(
        retrato.repositorios[0].base.keys().collect::<Vec<_>>(),
        ["anotacoes.txt", "db/02_parametros.sql"]
    );
    // Logo depois do retrato, nada é atribuído à sessão.
    assert!(c.conferir().is_empty());

    // Durante a sessão.
    let pkg = c.ler("db/03_pkg_cobranca.sql");
    let primeira = pkg.lines().next().unwrap().to_owned();
    c.escrever(
        "db/03_pkg_cobranca.sql",
        &pkg.replacen(&primeira, "-- alterado pela sessão", 1),
    );
    c.acrescentar("db/02_parametros.sql", "-- linha da sessão\n");
    c.escrever("db/04_novo.sql", "select 1 from dual;\n");
    fs::remove_file(c.arquivo("cobol/CALCOBR.cbl")).unwrap();

    assert_eq!(
        c.conferir(),
        [
            ("cobol/CALCOBR.cbl".to_owned(), TipoAlteracao::Removido),
            ("db/02_parametros.sql".to_owned(), TipoAlteracao::Modificado),
            (
                "db/03_pkg_cobranca.sql".to_owned(),
                TipoAlteracao::Modificado
            ),
            ("db/04_novo.sql".to_owned(), TipoAlteracao::Criado),
        ]
    );

    // O diff do arquivo que já estava sujo mostra só a linha da sessão.
    let d = c.detalhe("db/02_parametros.sql");
    let diff = d.diff.unwrap();
    assert!(diff.contains("+-- linha da sessão"), "{diff}");
    assert!(!diff.contains("+-- ajuste local do analista"), "{diff}");
    assert!(diff.starts_with("--- a/db/02_parametros.sql\n+++ b/db/02_parametros.sql\n"));

    let d = c.detalhe("db/03_pkg_cobranca.sql");
    let diff = d.diff.unwrap();
    assert!(diff.contains(&format!("-{primeira}\n")), "{diff}");
    assert!(diff.contains("+-- alterado pela sessão\n"), "{diff}");
    assert!(d.alterado.desfazivel);
    assert!(!d.binario);

    let d = c.detalhe("db/04_novo.sql");
    assert!(
        d.diff
            .unwrap()
            .starts_with("--- /dev/null\n+++ b/db/04_novo.sql\n")
    );
    let d = c.detalhe("cobol/CALCOBR.cbl");
    assert_eq!(d.alterado.hash_atual, None);
    assert!(d.diff.unwrap().contains("+++ /dev/null\n"));

    // Desfazer volta tudo ao início da sessão, inclusive o ajuste do analista.
    let feitos = desfazer(&c.retratos, SESSAO, &c.pedidos()).unwrap();
    assert_eq!(feitos.len(), 4);
    assert!(c.conferir().is_empty());
    assert_eq!(c.ler("db/02_parametros.sql"), antes_parametros);
    assert_eq!(c.ler("db/03_pkg_cobranca.sql"), pkg);
    assert!(!c.arquivo("db/04_novo.sql").exists());
    assert_eq!(
        fs::read(c.arquivo("cobol/CALCOBR.cbl")).unwrap(),
        fs::read(fixture().join("cobol/CALCOBR.cbl")).unwrap()
    );
    assert_eq!(c.ler("anotacoes.txt"), "rascunho do analista\n");
}

#[test]
fn desfazer_recusa_arquivo_que_mudou_de_novo_ou_que_nao_e_da_sessao() {
    let c = Cenario::com_git();
    c.retratar();
    c.acrescentar("db/01_tabelas.sql", "-- sessão\n");
    let pedidos = c.pedidos();
    c.acrescentar("db/01_tabelas.sql", "-- de novo\n");
    let antes = c.ler("db/01_tabelas.sql");
    assert!(matches!(
        desfazer(&c.retratos, SESSAO, &pedidos),
        Err(Erro::MudouDeNovo(_))
    ));
    assert_eq!(c.ler("db/01_tabelas.sql"), antes);

    let fora = Pedido {
        sessao: SESSAO.into(),
        pasta: c.codigo.clone(),
        caminho: "db/02_parametros.sql".into(),
        hash_atual: None,
    };
    assert!(matches!(
        desfazer(&c.retratos, SESSAO, &[fora]),
        Err(Erro::NaoAlterado(_))
    ));
}

#[test]
fn commit_feito_na_sessao_tambem_aparece() {
    let c = Cenario::com_git();
    c.retratar();
    c.acrescentar("db/01_tabelas.sql", "-- sessão\n");
    git(&c.codigo, &["commit", "-q", "-am", "da sessão"]);
    assert_eq!(
        c.conferir(),
        [("db/01_tabelas.sql".to_owned(), TipoAlteracao::Modificado)]
    );
    desfazer(&c.retratos, SESSAO, &c.pedidos()).unwrap();
    assert!(c.conferir().is_empty());
}

#[test]
fn manter_tira_do_aviso_ate_mudar_de_novo() {
    let c = Cenario::com_git();
    c.retratar();
    c.acrescentar("db/01_tabelas.sql", "-- sessão\n");
    c.escrever("db/novo.sql", "select 2;\n");
    manter(&c.retratos, SESSAO, &c.pedidos()).unwrap();
    assert!(c.conferir().is_empty());
    c.acrescentar("db/novo.sql", "select 3;\n");
    let d = c.detalhe("db/novo.sql");
    assert_eq!(d.alterado.tipo, TipoAlteracao::Modificado);
    assert!(d.diff.unwrap().contains(" select 2;\n+select 3;\n"));
}

#[test]
fn so_as_pastas_do_projeto_dentro_do_repositorio() {
    let c = Cenario::com_git();
    retratar(
        &c.retratos,
        SESSAO,
        "faturamento-exemplo",
        &[c.codigo.join("db")],
    )
    .unwrap();
    c.acrescentar("db/01_tabelas.sql", "-- sessão\n");
    c.acrescentar("cobol/CALCOBR.cbl", "      * sessão\n");
    assert_eq!(
        c.conferir(),
        [("db/01_tabelas.sql".to_owned(), TipoAlteracao::Modificado)]
    );
}

#[test]
fn pasta_fora_do_git_lista_sem_diff() {
    let c = Cenario::sem_git();
    let r = c.retratar();
    assert!(r.repositorios.is_empty());
    assert_eq!(r.sem_git.len(), 1);
    assert!(!r.avisos.is_empty());
    // Data de modificação diferente mesmo em sistemas de arquivos de baixa resolução.
    std::thread::sleep(Duration::from_millis(20));
    c.acrescentar("db/01_tabelas.sql", "-- sessão\n");
    c.escrever("db/novo.sql", "x\n");
    let (detalhes, avisos) = detalhar(&c.retratos, SESSAO).unwrap();
    assert!(!avisos.is_empty());
    let nomes: Vec<_> = detalhes
        .iter()
        .map(|d| d.alterado.caminho.as_str())
        .collect();
    assert_eq!(nomes, ["db/01_tabelas.sql", "db/novo.sql"]);
    assert!(
        detalhes
            .iter()
            .all(|d| d.diff.is_none() && !d.alterado.desfazivel)
    );
    let pedido = Pedido {
        sessao: SESSAO.into(),
        pasta: c.codigo.clone(),
        caminho: "db/novo.sql".into(),
        hash_atual: None,
    };
    assert!(matches!(
        desfazer(&c.retratos, SESSAO, &[pedido]),
        Err(Erro::SemConteudoAnterior(_))
    ));
}

#[test]
fn diff_de_binario_e_cortado() {
    assert_eq!(diff("a.bin", Some(b"a\0b"), Some(b"c")), (None, true));
    let grande = "linha\n".repeat(LIMITE_DO_DIFF);
    let (d, binario) = diff("a.txt", None, Some(grande.as_bytes()));
    assert!(!binario);
    let d = d.unwrap();
    assert!(d.len() < LIMITE_DO_DIFF + 100);
    assert!(d.ends_with("(diff cortado)\n"));
}

#[test]
fn reserva_unica_e_espera() {
    let dir = tempfile::tempdir().unwrap();
    let r = Retratos::em(dir.path());
    assert!(
        r.aguardar(SESSAO, Duration::ZERO),
        "sem pedido, nada a esperar"
    );
    assert!(r.reservar(SESSAO).unwrap());
    assert!(!r.reservar(SESSAO).unwrap());
    assert!(!r.reservar("../fora").unwrap());
    assert!(!r.aguardar(SESSAO, Duration::from_millis(30)));
    assert!(matches!(conferir(&r, SESSAO), Err(Erro::SemRetrato)));
}

#[test]
fn tarefas_pelo_nome() {
    for t in [Tarefa::Retratar, Tarefa::Conferir] {
        assert_eq!(Tarefa::do_nome(t.nome()), Some(t));
    }
    assert_eq!(Tarefa::do_nome("outra"), None);
}
