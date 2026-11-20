//! Importação da documentação externa, com o sistema fictício Faturamento Exemplo.

use std::collections::HashMap;
use std::time::Duration;

use super::*;
use crate::Base;
use crate::frontmatter::{Area, FrontmatterProjeto};

const REPO: &str = "GitHub exemplo/faturamento-docs";

struct Cenario {
    _tmp: tempfile::TempDir,
    projeto: Projeto,
    /// Conteúdo de cada arquivo do "commit", por caminho relativo à pasta de docs.
    commit: HashMap<String, Vec<u8>>,
    leituras: usize,
}

impl Cenario {
    fn novo() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let base = Base::criar(tmp.path().join("base")).unwrap();
        let projeto = base
            .criar_projeto(&Documento::novo(
                FrontmatterProjeto {
                    tipo: TipoDocumento::Projeto,
                    id: "faturamento-exemplo".into(),
                    nome: "Faturamento Exemplo".into(),
                    areas: vec![Area {
                        id: "cobranca".into(),
                        nome: "Cobrança".into(),
                        sigla: Some("FAT".into()),
                    }],
                    criado: None,
                },
                "\n# Faturamento Exemplo\n",
            ))
            .unwrap();
        Self {
            _tmp: tmp,
            projeto,
            commit: HashMap::new(),
            leituras: 0,
        }
    }

    fn arquivo(&mut self, caminho: &str, texto: &str) {
        self.commit
            .insert(caminho.into(), texto.as_bytes().to_vec());
    }

    fn importar(&mut self, commit: &str) -> Importacao {
        let mut arquivos: Vec<ArquivoExterno> = self
            .commit
            .iter()
            .map(|(c, b)| ArquivoExterno {
                caminho: c.clone(),
                caminho_no_repositorio: format!("docs/negocio/{c}"),
                versao: hash(b),
                tamanho: b.len() as u64,
            })
            .collect();
        arquivos.sort_by(|a, b| a.caminho.cmp(&b.caminho));
        let commit_atual = self.commit.clone();
        let mut leituras = 0;
        let r = importar(
            &self.projeto,
            &Fonte {
                repositorio: REPO.into(),
                branch: "main".into(),
                commit: commit.into(),
            },
            &arquivos,
            &|c: &str| {
                c.ends_with("/.env.md")
                    .then(|| "possível segredo (padrão `.env`)".to_string())
            },
            &mut |a: &ArquivoExterno| {
                leituras += 1;
                Ok(commit_atual[&a.caminho].clone())
            },
            1_000,
        )
        .unwrap();
        self.leituras = leituras;
        r
    }

    fn copia(&self, caminho: &str) -> Documento<FrontmatterDocExterno> {
        self.projeto
            .ler(&CaminhoRelativo::novo(caminho).unwrap())
            .unwrap()
    }
}

const PASTA: &str = "externos/github-exemplo-faturamento-docs";

#[test]
fn importa_com_e_sem_frontmatter_com_origem_e_commit() {
    let mut c = Cenario::novo();
    c.arquivo(
        "visao-geral.md",
        "# Visão geral do faturamento\n\nA fatura vence no dia 10.\n",
    );
    c.arquivo(
        "regras/multa.md",
        "---\ntipo: regra\nid: RN-FAT-001\ntitulo: Multa por atraso\narea: cobranca\nstatus: confirmada\n---\n\nMulta de 2% ao mês.\n",
    );
    c.arquivo("sem-titulo.md", "texto solto\n");
    let r = c.importar("abc123");
    assert_eq!((r.novos, r.alterados, r.removidos), (3, 0, 0));

    let doc = c.copia(&format!("{PASTA}/visao-geral.md"));
    let f = &doc.frontmatter;
    assert_eq!(f.tipo, TipoDocumento::DocExterno);
    assert_eq!(f.titulo, "Visão geral do faturamento");
    assert_eq!(f.origem.repositorio, REPO);
    assert_eq!(f.origem.caminho, "docs/negocio/visao-geral.md");
    assert_eq!(f.origem.commit, "abc123");
    assert_eq!(f.origem.branch.as_deref(), Some("main"));
    assert_eq!(f.confianca, Confianca::Externa);
    assert!(f.frontmatter_original.is_none());
    assert!(doc.corpo.contains("vence no dia 10"));

    let regra = c.copia(&format!("{PASTA}/regras/multa.md"));
    assert_eq!(regra.frontmatter.confianca, Confianca::Publicada);
    assert_eq!(regra.frontmatter.tipo_original.as_deref(), Some("regra"));
    assert_eq!(regra.frontmatter.id_original.as_deref(), Some("RN-FAT-001"));
    assert_eq!(regra.frontmatter.titulo, "Multa por atraso");
    assert!(regra.frontmatter.frontmatter_original.is_some());
    assert_eq!(regra.corpo.trim(), "Multa de 2% ao mês.");

    assert_eq!(
        c.copia(&format!("{PASTA}/sem-titulo.md"))
            .frontmatter
            .titulo,
        "sem-titulo"
    );

    let estado = EstadoExternos::ler(&c.projeto).unwrap();
    assert_eq!(estado.importados(), 3);
    assert_eq!(estado.fonte.unwrap().commit, "abc123");
    assert_eq!(estado.sincronizado_ms, Some(1_000));
}

#[test]
fn so_le_o_que_mudou_e_remove_o_que_saiu() {
    let mut c = Cenario::novo();
    c.arquivo("a.md", "# A\n");
    c.arquivo("pasta/b.md", "# B\n");
    c.importar("c1");
    assert_eq!(c.leituras, 2);

    // Mesmo conteúdo em outro commit: nada é lido nem regravado; a cópia
    // continua com o commit em que foi importada.
    let r = c.importar("c2");
    assert_eq!((r.novos, r.alterados, r.inalterados), (0, 0, 2));
    assert_eq!(c.leituras, 0);
    assert_eq!(
        c.copia(&format!("{PASTA}/a.md")).frontmatter.origem.commit,
        "c1"
    );

    // Mudou um, saiu outro.
    c.arquivo("a.md", "# A nova\n");
    c.commit.remove("pasta/b.md");
    let r = c.importar("c3");
    assert_eq!((r.alterados, r.removidos), (1, 1));
    assert_eq!(
        c.copia(&format!("{PASTA}/a.md")).frontmatter.origem.commit,
        "c3"
    );
    assert!(!c.projeto.raiz().join(PASTA).join("pasta").exists());

    // Cópia mexida à mão volta a ser a do repositório.
    let disco = c.projeto.raiz().join(PASTA).join("a.md");
    fs::write(&disco, "editado\n").unwrap();
    let r = c.importar("c3");
    assert_eq!(r.alterados, 1);
    assert!(fs::read_to_string(&disco).unwrap().contains("# A nova"));

    // Arquivo estranho na pasta gerida sai.
    fs::write(c.projeto.raiz().join(PASTA).join("solto.md"), "x").unwrap();
    assert_eq!(c.importar("c3").removidos, 1);
}

#[test]
fn nao_duplica_o_que_a_base_ja_tem() {
    let mut c = Cenario::novo();
    c.projeto
        .gravar_texto(
            &CaminhoRelativo::novo("regras/RN-FAT-001.md").unwrap(),
            "---\ntipo: regra\nid: RN-FAT-001\ntitulo: Multa por atraso\n---\n\nMulta.\n",
        )
        .unwrap();
    c.arquivo(
        "regras/RN-FAT-001.md",
        "---\ntipo: regra\nid: RN-FAT-001\ntitulo: Multa por atraso\n---\n\nMulta.\n",
    );
    c.arquivo(
        "regras/RN-FAT-002.md",
        "---\ntipo: regra\nid: RN-FAT-002\ntitulo: Juros\n---\n\nJuros de outra base.\n",
    );
    let r = c.importar("c1");
    assert_eq!(r.ja_na_base, ["docs/negocio/regras/RN-FAT-001.md"]);
    assert_eq!(r.novos, 1);
    assert!(
        !c.projeto
            .raiz()
            .join(PASTA)
            .join("regras/RN-FAT-001.md")
            .exists()
    );

    // Sem a regra na base, a publicada volta como externa (sem ler de novo).
    fs::remove_file(c.projeto.raiz().join("regras/RN-FAT-001.md")).unwrap();
    let r = c.importar("c2");
    assert!(r.ja_na_base.is_empty());
    assert_eq!(r.novos, 1);
    let doc = c.copia(&format!("{PASTA}/regras/RN-FAT-001.md"));
    assert_eq!(doc.frontmatter.confianca, Confianca::Publicada);
}

#[test]
fn pula_e_mascara_o_que_e_sensivel() {
    let mut c = Cenario::novo();
    c.arquivo("config/.env.md", "SENHA=x\n");
    c.arquivo(
        "acesso.md",
        "# Acesso\n\nsenha: Fatura2026!\nContato: ana@example.com\n",
    );
    c.arquivo(
        "chave.md",
        "-----BEGIN RSA PRIVATE KEY-----\nabc\n-----END RSA PRIVATE KEY-----\n",
    );
    c.arquivo("CON.md", "# Nome reservado\n");
    c.arquivo(".github/modelo.md", "# Oculto\n");
    c.commit.insert("binario.md".into(), vec![0xff, 0xfe, 0x00]);
    let r = c.importar("c1");
    assert_eq!(r.novos, 1);
    let mut pulados: Vec<&str> = r.pulados.iter().map(|p| p.caminho.as_str()).collect();
    pulados.sort();
    assert_eq!(
        pulados,
        [
            "docs/negocio/.github/modelo.md",
            "docs/negocio/CON.md",
            "docs/negocio/binario.md",
            "docs/negocio/chave.md",
            "docs/negocio/config/.env.md",
        ]
    );
    assert_eq!(
        r.mascarados,
        [Mascarado {
            caminho: "docs/negocio/acesso.md".into(),
            trechos: 2
        }]
    );
    let doc = c.copia(&format!("{PASTA}/acesso.md"));
    assert!(!doc.corpo.contains("Fatura2026"));
    assert!(!doc.corpo.contains("ana@example.com"));
    assert_eq!(doc.frontmatter.trechos_mascarados, 2);

    // Na próxima, o pulado pelo conteúdo não é lido de novo, e continua pulado.
    let r = c.importar("c2");
    assert_eq!(r.pulados.len(), 5);
    assert_eq!(c.leituras, 0);
    let estado = EstadoExternos::ler(&c.projeto).unwrap();
    assert_eq!(estado.pulados.len(), 5);
    assert_eq!(estado.mascarados.len(), 1);
}

#[test]
fn falha_e_desatualizacao() {
    let mut c = Cenario::novo();
    let limite = Duration::from_secs(3600);
    assert!(EstadoExternos::ler(&c.projeto).is_none());
    registrar_falha(&c.projeto, "sem rede", 500).unwrap();
    let e = EstadoExternos::ler(&c.projeto).unwrap();
    assert!(
        e.desatualizado(600, limite)
            .unwrap()
            .contains("ainda não foi sincronizado")
    );

    c.arquivo("a.md", "# A\n");
    c.importar("abcdef1234567");
    let e = EstadoExternos::ler(&c.projeto).unwrap();
    assert_eq!(e.desatualizado(1_000 + 60_000, limite), None);
    assert!(
        e.desatualizado(1_000 + 3 * 3_600_000, limite)
            .unwrap()
            .contains("há 3 horas")
    );
    registrar_falha(&c.projeto, "sem rede", 2_000).unwrap();
    let e = EstadoExternos::ler(&c.projeto).unwrap();
    let aviso = e.desatualizado(2_000, limite).unwrap();
    assert!(aviso.contains("falhou (sem rede)"), "{aviso}");
    assert!(aviso.contains("abcdef1234"), "{aviso}");
    // A falha não mexe nas cópias.
    assert_eq!(e.importados(), 1);
    assert!(c.projeto.raiz().join(PASTA).join("a.md").exists());

    assert!(remover_tudo(&c.projeto).unwrap());
    assert!(!c.projeto.raiz().join("externos").exists());
}

#[test]
fn trava_uma_sincronizacao_por_vez() {
    let c = Cenario::novo();
    let t = travar(&c.projeto).unwrap().unwrap();
    assert!(travar(&c.projeto).unwrap().is_none());
    drop(t);
    assert!(travar(&c.projeto).unwrap().is_some());
}
