//! Catálogo de pessoas por projeto (#133): `pessoas/<slug>.md`, apelidos,
//! casamento de nomes normalizado, histórico de papéis e junção de pessoas.

use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::thread;

use cba_kb::frontmatter::{Data, FrontmatterPessoa, FrontmatterProjeto};
use cba_kb::indice::{Indice, Tipo};
use cba_kb::{
    Base, CaminhoRelativo, Casamento, Documento, Erro, NovaPessoa, OrigemPapeis, PADRAO_PAPEIS,
    Papeis, Projeto, TipoDocumento,
};

const PROJETO: &str = "faturamento-exemplo";

fn data(dia: u8) -> Data {
    Data::nova(2026, 11, dia).unwrap()
}

fn criar_base() -> (tempfile::TempDir, Base, Projeto) {
    let dir = tempfile::tempdir().unwrap();
    let base = Base::criar(dir.path().join("Base – Análises")).unwrap();
    let projeto = base
        .criar_projeto(&Documento::novo(
            FrontmatterProjeto {
                tipo: TipoDocumento::Projeto,
                id: PROJETO.into(),
                nome: "Faturamento Exemplo".into(),
                areas: vec![],
                criado: Some(data(1)),
            },
            "\n# Faturamento Exemplo\n",
        ))
        .unwrap();
    (dir, base, projeto)
}

fn pessoa(nome: &str) -> NovaPessoa {
    NovaPessoa {
        nome: nome.into(),
        ..Default::default()
    }
}

fn slug_casado(casamento: &Casamento) -> Option<&str> {
    casamento.pessoa().map(|p| p.slug.as_str())
}

#[test]
fn registra_pessoa_em_pessoas_slug_md() {
    let (_dir, _base, projeto) = criar_base();
    let maria = projeto
        .registrar_pessoa(&NovaPessoa {
            nome: "  Maria   Silva ".into(),
            apelidos: vec!["Maria S.".into(), "maria.silva@empresa".into()],
            papel: Some("PO".into()),
        })
        .unwrap();
    assert_eq!(maria.slug, "maria-silva");
    assert_eq!(maria.arquivo.as_str(), "pessoas/maria-silva.md");
    // A grafia que já casa com o nome (o e-mail, sem o domínio) não vira apelido.
    assert_eq!(maria.apelidos(), ["Maria S."]);
    assert_eq!(maria.nome(), "Maria Silva");

    let texto = fs::read_to_string(projeto.caminho(&maria.arquivo)).unwrap();
    assert!(
        texto.starts_with("---\ntipo: pessoa\nnome: Maria Silva\n"),
        "{texto}"
    );
    assert!(!texto.contains("empresa"), "{texto}");

    // O e-mail de outra pessoa como apelido fica sem o domínio.
    let joao = projeto
        .registrar_pessoa(&NovaPessoa {
            nome: "João Souza".into(),
            apelidos: vec!["jsouza@empresa.com.br".into()],
            ..Default::default()
        })
        .unwrap();
    assert_eq!(joao.apelidos(), ["jsouza"]);

    let lida = projeto.pessoa("maria-silva").unwrap();
    assert_eq!(lida.documento.frontmatter, maria.documento.frontmatter);
    assert_eq!(lida.arquivo, maria.arquivo);
    let slugs: Vec<_> = projeto
        .pessoas()
        .unwrap()
        .into_iter()
        .map(|p| p.slug)
        .collect();
    assert_eq!(slugs, [joao.slug, maria.slug]);
}

#[test]
fn variacoes_do_teams_casam_com_a_mesma_pessoa() {
    let (_dir, _base, projeto) = criar_base();
    projeto.registrar_pessoa(&pessoa("Maria Silva")).unwrap();
    projeto.registrar_pessoa(&pessoa("João Souza")).unwrap();

    let falantes = [
        "Silva, Maria",
        "maria.silva@empresa",
        "Maria Silva",
        "MARIA SÍLVA (Convidado)",
        "Souza, João",
        "Pedro Alves",
    ];
    let sugeridos = projeto.sugerir_participantes(&falantes).unwrap();
    let casados: Vec<_> = sugeridos
        .iter()
        .map(|p| (p.falante.as_str(), slug_casado(&p.casamento)))
        .collect();
    assert_eq!(
        casados,
        [
            ("Silva, Maria", Some("maria-silva")),
            ("maria.silva@empresa", Some("maria-silva")),
            ("Maria Silva", Some("maria-silva")),
            ("MARIA SÍLVA (Convidado)", Some("maria-silva")),
            ("Souza, João", Some("joao-souza")),
            ("Pedro Alves", None),
        ]
    );
    assert!(matches!(sugeridos[5].casamento, Casamento::Nenhuma));
}

#[test]
fn apelido_casa_variacao_que_a_normalizacao_nao_alcanca() {
    let (_dir, _base, projeto) = criar_base();
    projeto.registrar_pessoa(&pessoa("Maria Silva")).unwrap();
    let catalogo = projeto.catalogo_de_pessoas().unwrap();
    assert!(matches!(catalogo.casar("Maria S."), Casamento::Nenhuma));

    let maria = projeto
        .acrescentar_apelido("maria-silva", "Maria S.")
        .unwrap();
    assert_eq!(maria.apelidos(), ["Maria S."]);
    // Repetir, em outra grafia, não muda nada.
    let de_novo = projeto
        .acrescentar_apelido("maria-silva", "maria s")
        .unwrap();
    assert_eq!(de_novo.apelidos(), ["Maria S."]);

    let catalogo = projeto.catalogo_de_pessoas().unwrap();
    assert_eq!(
        slug_casado(&catalogo.casar("Maria S.")),
        Some("maria-silva")
    );
}

#[test]
fn papel_sugerido_e_o_ultimo_confirmado() {
    let (_dir, _base, projeto) = criar_base();
    projeto.registrar_pessoa(&pessoa("Maria Silva")).unwrap();

    // Sem histórico nem papel: sem sugestão.
    let antes = projeto.sugerir_participantes(&["Maria Silva"]).unwrap();
    assert_eq!(antes[0].papel_sugerido, None);

    // Primeira reunião: confirmada como PO.
    projeto
        .registrar_papel("maria-silva", "D-0001/R-0001", "PO", data(10))
        .unwrap();
    let segunda = projeto.sugerir_participantes(&["Silva, Maria"]).unwrap();
    let sugerido = segunda[0].papel_sugerido.clone().unwrap();
    assert_eq!(sugerido.papel, "PO");
    assert_eq!(sugerido.reuniao.as_deref(), Some("D-0001/R-0001"));
    assert_eq!(sugerido.confirmado_em, Some(data(10)));

    // Segunda reunião: conduziu como gerente/patrocinador.
    let maria = projeto
        .registrar_papel(
            "maria-silva",
            "D-0001/R-0002",
            "gerente/patrocinador",
            data(12),
        )
        .unwrap();
    assert_eq!(
        maria.documento.frontmatter.papel.as_deref(),
        Some("gerente/patrocinador")
    );
    let terceira = projeto
        .sugerir_participantes(&["maria.silva@empresa"])
        .unwrap();
    assert_eq!(
        terceira[0].papel_sugerido.as_ref().unwrap().papel,
        "gerente/patrocinador"
    );

    // Confirmar tarde uma reunião antiga não muda o papel sugerido.
    projeto
        .registrar_papel("maria-silva", "D-0002/R-0001", "usuário-chave", data(11))
        .unwrap();
    let lida = projeto.pessoa("maria-silva").unwrap();
    assert_eq!(lida.papel_sugerido().unwrap().papel, "gerente/patrocinador");
    assert_eq!(lida.documento.frontmatter.historico.len(), 3);

    // Corrigir a mesma reunião troca o papel dela, sem duplicar.
    projeto
        .registrar_papel("maria-silva", "D-0001/R-0002", "PO", data(12))
        .unwrap();
    let lida = projeto.pessoa("maria-silva").unwrap();
    assert_eq!(lida.documento.frontmatter.historico.len(), 3);
    assert_eq!(lida.papel_sugerido().unwrap().papel, "PO");
}

#[test]
fn papel_do_registro_vale_ate_a_primeira_confirmacao() {
    let (_dir, _base, projeto) = criar_base();
    projeto
        .registrar_pessoa(&NovaPessoa {
            nome: "João Souza".into(),
            papel: Some("arquiteto".into()),
            ..Default::default()
        })
        .unwrap();
    let s = projeto.sugerir_participantes(&["Souza, João"]).unwrap();
    let sugerido = s[0].papel_sugerido.clone().unwrap();
    assert_eq!(sugerido.papel, "arquiteto");
    assert_eq!(sugerido.reuniao, None);
}

#[test]
fn juntar_une_apelidos_e_historicos_e_remove_a_duplicada() {
    let (_dir, _base, projeto) = criar_base();
    projeto
        .registrar_pessoa(&NovaPessoa {
            nome: "Maria Silva".into(),
            apelidos: vec!["Maria S.".into()],
            ..Default::default()
        })
        .unwrap();
    // O Teams mostrou outro nome, que o analista registrou como outra pessoa.
    let duplicada = projeto
        .registrar_pessoa(&NovaPessoa {
            nome: "Maria Aparecida Silva".into(),
            apelidos: vec!["M. Aparecida".into()],
            ..Default::default()
        })
        .unwrap();
    projeto
        .registrar_papel("maria-silva", "D-0001/R-0001", "PO", data(10))
        .unwrap();
    projeto
        .registrar_papel(&duplicada.slug, "D-0001/R-0002", "QA", data(12))
        .unwrap();
    // Mesma reunião nas duas: vale a confirmação mais recente.
    projeto
        .registrar_papel("maria-silva", "D-0001/R-0003", "PO", data(13))
        .unwrap();
    projeto
        .registrar_papel(&duplicada.slug, "D-0001/R-0003", "usuário-chave", data(14))
        .unwrap();

    let junta = projeto
        .juntar_pessoas("maria-silva", &duplicada.slug)
        .unwrap();
    let f = &junta.documento.frontmatter;
    assert_eq!(f.nome, "Maria Silva");
    assert_eq!(
        f.apelidos,
        ["Maria S.", "Maria Aparecida Silva", "M. Aparecida"]
    );
    let historico: Vec<_> = f
        .historico
        .iter()
        .map(|p| (p.reuniao.as_str(), p.papel.as_str(), p.confirmado_em))
        .collect();
    assert_eq!(
        historico,
        [
            ("D-0001/R-0001", "PO", data(10)),
            ("D-0001/R-0002", "QA", data(12)),
            ("D-0001/R-0003", "usuário-chave", data(14)),
        ]
    );
    assert_eq!(f.papel.as_deref(), Some("usuário-chave"));

    // A duplicada sumiu e os nomes dela casam com a pessoa que ficou.
    assert!(!projeto.caminho(&duplicada.arquivo).exists());
    assert!(matches!(
        projeto.pessoa(&duplicada.slug),
        Err(Erro::PessoaInexistente { .. })
    ));
    assert_eq!(projeto.pessoas().unwrap().len(), 1);
    let catalogo = projeto.catalogo_de_pessoas().unwrap();
    for nome in ["Silva, Maria Aparecida", "M. Aparecida", "Maria S."] {
        assert_eq!(
            slug_casado(&catalogo.casar(nome)),
            Some("maria-silva"),
            "{nome}"
        );
    }
    assert!(matches!(
        projeto.juntar_pessoas("maria-silva", "maria-silva"),
        Err(Erro::JuntarMesmaPessoa(_))
    ));
}

#[test]
fn recusa_duplicata_e_apelido_de_outra_pessoa() {
    let (_dir, _base, projeto) = criar_base();
    projeto.registrar_pessoa(&pessoa("Maria Silva")).unwrap();
    projeto.registrar_pessoa(&pessoa("João Souza")).unwrap();

    match projeto.registrar_pessoa(&pessoa("Silva, Maria")) {
        Err(Erro::PessoaJaCadastrada { pessoa, .. }) => assert_eq!(pessoa, "maria-silva"),
        outro => panic!("{outro:?}"),
    }
    // Pelo apelido também.
    let nova = NovaPessoa {
        nome: "M. Silva".into(),
        apelidos: vec!["maria.silva@empresa".into()],
        ..Default::default()
    };
    assert!(matches!(
        projeto.registrar_pessoa(&nova),
        Err(Erro::PessoaJaCadastrada { .. })
    ));
    match projeto.acrescentar_apelido("joao-souza", "Silva, Maria") {
        Err(Erro::ApelidoDeOutraPessoa { outra, .. }) => assert_eq!(outra, "maria-silva"),
        outro => panic!("{outro:?}"),
    }
    assert!(matches!(
        projeto.pessoa("pedro"),
        Err(Erro::PessoaInexistente { .. })
    ));
    assert!(matches!(
        projeto.pessoa("../projeto"),
        Err(Erro::PessoaInexistente { .. })
    ));
}

#[test]
fn homonimos_ficam_ambiguos() {
    let (_dir, _base, projeto) = criar_base();
    projeto.registrar_pessoa(&pessoa("Maria Silva")).unwrap();
    // O analista distingue as duas pelo nome, e o Teams mostra só "Maria Silva"
    // para a segunda, que entra como apelido sem passar pela checagem do registro.
    let outra = projeto
        .registrar_pessoa(&pessoa("Maria Silva Financeiro"))
        .unwrap();
    let caminho = projeto.caminho(&outra.arquivo);
    let texto = fs::read_to_string(&caminho)
        .unwrap()
        .replace("apelidos: []", "apelidos:\n- Maria Silva");
    fs::write(&caminho, texto).unwrap();

    let catalogo = projeto.catalogo_de_pessoas().unwrap();
    match catalogo.casar("Maria Silva") {
        Casamento::Ambigua(v) => assert_eq!(v.len(), 2),
        outro => panic!("{outro:?}"),
    }
    let s = catalogo.sugerir(&["Maria Silva"]);
    assert_eq!(s[0].papel_sugerido, None);
}

#[test]
fn registro_concorrente_da_mesma_pessoa_cria_um_arquivo() {
    let (_dir, _base, projeto) = criar_base();
    let projeto = Arc::new(projeto);
    let fios: Vec<_> = [
        "Maria Silva",
        "Silva, Maria",
        "maria.silva@empresa",
        "MARIA SILVA",
    ]
    .into_iter()
    .map(|nome| {
        let projeto = Arc::clone(&projeto);
        thread::spawn(move || projeto.registrar_pessoa(&pessoa(nome)))
    })
    .collect();
    let resultados: Vec<_> = fios.into_iter().map(|f| f.join().unwrap()).collect();
    assert_eq!(resultados.iter().filter(|r| r.is_ok()).count(), 1);
    assert!(
        resultados
            .iter()
            .filter_map(|r| r.as_ref().err())
            .all(|e| matches!(e, Erro::PessoaJaCadastrada { .. })),
        "{resultados:?}"
    );
    assert_eq!(projeto.pessoas().unwrap().len(), 1);
}

#[test]
fn slug_repetido_de_nomes_diferentes_ganha_sufixo() {
    let (_dir, _base, projeto) = criar_base();
    projeto.registrar_pessoa(&pessoa("Ana Lima")).unwrap();
    // Outra pessoa cujo arquivo calhou de ter o slug `ana-lima` antes: simulamos
    // trocando o nome no arquivo.
    let caminho = projeto.caminho(&CaminhoRelativo::novo("pessoas/ana-lima.md").unwrap());
    let texto = fs::read_to_string(&caminho)
        .unwrap()
        .replace("nome: Ana Lima", "nome: Ana Paula");
    fs::write(&caminho, texto).unwrap();
    let ana = projeto.registrar_pessoa(&pessoa("Ana Lima")).unwrap();
    assert_eq!(ana.slug, "ana-lima-2");
}

/// Falas fictícias de uma reunião do "Faturamento Exemplo".
const FALAS: [(&str, &str); 3] = [
    (
        "Silva, Maria",
        "Precisamos cancelar a nota fiscal depois de emitida, até o fim do mês.",
    ),
    (
        "joao.souza@empresa",
        "O cancelamento hoje só é feito no mesmo dia, pela regra do faturamento.",
    ),
    (
        "Maria Silva",
        "Então vamos abrir a demanda e levar para a próxima reunião?",
    ),
];

fn textos_da_pasta(pasta: &Path) -> Vec<(String, String)> {
    let mut saida = Vec::new();
    for entrada in fs::read_dir(pasta).unwrap() {
        let caminho = entrada.unwrap().path();
        if caminho.is_dir() {
            saida.extend(textos_da_pasta(&caminho));
        } else {
            saida.push((
                caminho.display().to_string(),
                fs::read_to_string(&caminho).unwrap(),
            ));
        }
    }
    saida
}

#[test]
fn nenhuma_fala_entra_em_pessoas() {
    let (_dir, _base, projeto) = criar_base();

    // O fluxo inteiro de uma reunião: falantes, registro, apelido, papel e junção.
    let falantes: Vec<&str> = FALAS.iter().map(|(f, _)| *f).collect();
    for s in projeto.sugerir_participantes(&falantes).unwrap() {
        if matches!(s.casamento, Casamento::Nenhuma)
            && !projeto
                .catalogo_de_pessoas()
                .unwrap()
                .pessoas()
                .iter()
                .any(|p| p.casa_com(&s.falante))
        {
            projeto.registrar_pessoa(&pessoa(&s.falante)).unwrap();
        }
    }
    projeto
        .acrescentar_apelido("maria-silva", "Maria S.")
        .unwrap();
    projeto.registrar_pessoa(&pessoa("Maria S. Silva")).unwrap();
    projeto
        .juntar_pessoas("maria-silva", "maria-s-silva")
        .unwrap();
    for p in projeto.pessoas().unwrap() {
        projeto
            .registrar_papel(&p.slug, "D-0001/R-0001", "PO", data(10))
            .unwrap();
    }

    // A API recusa texto de fala em todo campo que aceita texto.
    let (_, fala) = FALAS[0];
    let (_, pergunta) = FALAS[2];
    for texto in [fala, pergunta] {
        assert!(matches!(
            projeto.registrar_pessoa(&pessoa(texto)),
            Err(Erro::Frontmatter { .. })
        ));
        assert!(matches!(
            projeto.registrar_pessoa(&NovaPessoa {
                nome: "Pedro Alves".into(),
                apelidos: vec![texto.into()],
                ..Default::default()
            }),
            Err(Erro::Frontmatter { .. })
        ));
        assert!(matches!(
            projeto.registrar_pessoa(&NovaPessoa {
                nome: "Pedro Alves".into(),
                papel: Some(texto.into()),
                ..Default::default()
            }),
            Err(Erro::Frontmatter { .. })
        ));
        assert!(matches!(
            projeto.acrescentar_apelido("maria-silva", texto),
            Err(Erro::Frontmatter { .. })
        ));
        assert!(matches!(
            projeto.registrar_papel("maria-silva", "D-0001/R-0002", texto, data(11)),
            Err(Erro::Frontmatter { .. })
        ));
        assert!(matches!(
            projeto.registrar_papel("maria-silva", texto, "PO", data(11)),
            Err(Erro::Frontmatter { .. })
        ));
    }

    // Varre os arquivos gravados: nenhuma fala, nem pedaço dela, nem domínio de
    // e-mail.
    let pasta = projeto.raiz().join("pessoas");
    let arquivos = textos_da_pasta(&pasta);
    assert_eq!(arquivos.len(), 2, "{arquivos:?}");
    for (arquivo, texto) in &arquivos {
        assert!(!texto.contains("@empresa"), "{arquivo}: {texto}");
        for (_, fala) in FALAS {
            for trecho in fala.split([',', '.', '?']).map(str::trim) {
                if trecho.split_whitespace().count() >= 3 {
                    assert!(!texto.contains(trecho), "{arquivo} tem `{trecho}`: {texto}");
                }
            }
        }
    }
}

#[test]
fn documento_editado_a_mao_com_fala_nao_e_lido() {
    let (_dir, _base, projeto) = criar_base();
    let maria = projeto.registrar_pessoa(&pessoa("Maria Silva")).unwrap();
    let caminho = projeto.caminho(&maria.arquivo);
    let texto = fs::read_to_string(&caminho).unwrap().replace(
        "historico: []",
        "historico:\n- reuniao: D-0001/R-0001\n  papel: \"Precisamos cancelar a nota fiscal depois de emitida, até o fim do mês.\"\n  confirmado_em: 2026-11-10",
    );
    fs::write(&caminho, texto).unwrap();
    assert!(matches!(
        projeto.pessoa("maria-silva"),
        Err(Erro::Frontmatter { .. })
    ));
}

#[test]
fn papeis_padrao_editaveis_em_templates() {
    let (_dir, base, _projeto) = criar_base();
    let (padrao, aviso) = Papeis::da_base(&base);
    assert!(aviso.is_none());
    assert_eq!(padrao.origem(), &OrigemPapeis::Padrao);
    assert_eq!(padrao.papeis().len(), 8);

    let criado = Papeis::copiar_padrao(&base).unwrap().unwrap();
    assert_eq!(criado.as_str(), "_templates/papeis.yaml");
    let caminho = base.caminho(&criado);
    assert_eq!(fs::read_to_string(&caminho).unwrap(), PADRAO_PAPEIS);

    // O analista edita: acrescenta um papel. Copiar de novo não sobrescreve.
    let editado = format!("{PADRAO_PAPEIS}  - nome: dono do processo\n");
    fs::write(&caminho, &editado).unwrap();
    assert_eq!(Papeis::copiar_padrao(&base).unwrap(), None);
    assert_eq!(fs::read_to_string(&caminho).unwrap(), editado);
    let (da_base, aviso) = Papeis::da_base(&base);
    assert!(aviso.is_none());
    assert_eq!(da_base.origem(), &OrigemPapeis::Base(criado.clone()));
    assert_eq!(da_base.papeis().len(), 9);
    assert_eq!(da_base.canonico("Dono do Processo"), "dono do processo");
    // Texto livre continua aceito.
    assert_eq!(da_base.canonico("consultor"), "consultor");

    // Arquivo inválido: vale o padrão, com aviso.
    fs::write(&caminho, "papeis: [").unwrap();
    let (volta, aviso) = Papeis::da_base(&base);
    assert_eq!(volta, Papeis::padrao());
    assert!(
        aviso
            .unwrap()
            .to_string()
            .starts_with("_templates/papeis.yaml: catálogo de papéis inválido")
    );
}

#[test]
fn indice_valida_e_busca_pessoas() {
    let (dir, base, projeto) = criar_base();
    projeto.registrar_pessoa(&pessoa("Maria Silva")).unwrap();
    let mut indice = Indice::abrir(dir.path().join("indice/base.sqlite"), &base).unwrap();
    indice.sincronizar().unwrap();
    assert!(indice.problemas().unwrap().is_empty());
    let doc: Documento<FrontmatterPessoa> = projeto
        .ler(&CaminhoRelativo::novo("pessoas/maria-silva.md").unwrap())
        .unwrap();
    assert_eq!(doc.frontmatter.tipo, TipoDocumento::Pessoa);
    let pagina = indice
        .buscar(&cba_kb::indice::Consulta::nova("Maria Silva").projeto(PROJETO))
        .unwrap();
    assert!(
        pagina
            .resultados
            .iter()
            .any(|r| r.tipo == Some(Tipo::Documento(TipoDocumento::Pessoa))),
        "{pagina:?}"
    );
}
