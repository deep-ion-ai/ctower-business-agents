//! Testes da publicação da documentação aprovada (#276, #289): a seleção do
//! aprovado, as páginas geradas, o cálculo das mudanças (com a migração do
//! formato antigo e a branch pendente já atualizada), a prévia por documento e,
//! no Unix, a publicação num repositório bare local com o gh falso, nos dois
//! modos de destino.

use super::*;
use cba_docs::marcador::{Marcador, Metadados};
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

fn regra(id: &str, status: &str, titulo: &str, enunciado: &str) -> String {
    format!(
        "---\ntipo: regra\nid: {id}\ntitulo: {titulo}\narea: faturamento\nstatus: {status}\n\
         origem: analises/A-0001.md\n---\n\n# {id} · {titulo}\n\n## Enunciado\n\n{enunciado}\n"
    )
}

fn analise(id: &str, status: &str, titulo: &str, criado: &str) -> String {
    format!(
        "---\ntipo: analise-impacto\nid: {id}\ntitulo: {titulo}\nprojeto: {PROJETO}\n\
         demanda:\n  id: null\n  origem: texto\n  chave: null\nstatus: {status}\n\
         criado: {criado}\n---\n\n# {id} · {titulo}\n\n## Resumo\n\n{titulo}: o que muda.\n"
    )
}

const TERMO: &str = "---\ntipo: termo\ntermo: Nota fiscal\nsinonimos: []\narea: faturamento\n---\n\n\
                     # Nota fiscal\n\n## Definição de negócio\n\nDocumento fiscal da venda.\n";

/// Projeto do Faturamento Exemplo com documentos aprovados e não aprovados.
fn base() -> (tempfile::TempDir, Projeto) {
    let tmp = tempfile::tempdir().unwrap();
    let raiz = tmp.path().join(PROJETO);
    let arquivos = [
        ("projeto.md", doc("projeto", None, "Faturamento Exemplo")),
        (
            "regras/RN-FAT-001.md",
            regra(
                "RN-FAT-001",
                "confirmada",
                "Cancelar nota no mesmo dia",
                "A nota fiscal só pode ser cancelada no dia da emissão.",
            ),
        ),
        (
            "regras/RN-FAT-002.md",
            regra("RN-FAT-002", "obsoleta", "Regra antiga", "Não vale mais."),
        ),
        ("glossario/nota-fiscal.md", TERMO.to_owned()),
        (
            "analises/A-0001.md",
            analise("A-0001", "aprovada", "Cancelamento", "2026-11-10"),
        ),
        (
            "analises/A-0002.md",
            analise("A-0002", "rascunho", "Em andamento", "2026-11-15"),
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
        regra(
            "RN-FAT-001",
            "confirmada",
            "Cancelar nota no mesmo dia",
            "A nota fiscal só pode ser cancelada no dia da emissão."
        )
        .into_bytes()
    );
    // A data do README: a da última mudança dos aprovados (a rascunho, mais
    // nova, não conta).
    assert_eq!(data_da_base(&docs).as_deref(), Some("2026-11-10"));
}

#[test]
fn data_da_base_e_a_maior_dos_frontmatters() {
    let d = |t: &str| DocumentoAprovado {
        caminho: "x.md".into(),
        conteudo: t.as_bytes().to_vec(),
    };
    assert_eq!(
        data_da_base(&[
            d("---\ntipo: termo\ncriado: 2026-11-02\n---\n"),
            d("---\ntipo: mapa-funcionalidades\natualizado: 2026-11-19\n---\n"),
            d("---\ntipo: ata\ndata: 2026-11-05T10:00:00\n---\n"),
            d("---\ntipo: termo\ncriado: ontem\n---\n"),
            d("sem frontmatter"),
        ])
        .as_deref(),
        Some("2026-11-19")
    );
    assert_eq!(data_da_base(&[d("---\ntipo: termo\n---\n")]), None);
}

#[test]
fn paginas_legiveis_da_base_aprovada() {
    let (_tmp, projeto) = base();
    let g = paginas_do_projeto(&projeto, None, true).unwrap();
    assert_eq!(g.aprovados, 3);
    assert!(g.avisos.is_empty(), "{:?}", g.avisos);
    let caminhos: Vec<&str> = g.paginas.iter().map(|p| p.caminho.as_str()).collect();
    assert!(caminhos.contains(&LEIA_ME), "{caminhos:?}");
    assert!(caminhos.contains(&"glossario.md"), "{caminhos:?}");
    assert!(caminhos.contains(&"analises/A-0001.md"), "{caminhos:?}");
    // Nenhum `.md` cru da base: as regras vão nas páginas.
    assert!(
        !caminhos.iter().any(|c| c.starts_with("regras/RN-")),
        "{caminhos:?}"
    );
    let todas: String = g
        .paginas
        .iter()
        .map(|p| String::from_utf8_lossy(&p.conteudo).into_owned())
        .collect();
    assert!(todas.contains("<!-- cba:regra id=RN-FAT-001 "), "{todas}");
    assert!(!todas.contains("RN-FAT-002"), "a obsoleta não sai");
    // A data do README é a da base, não a do dia: publicar de novo não muda nada.
    let leia_me = String::from_utf8_lossy(&g.paginas[0].conteudo).into_owned();
    assert!(
        leia_me.contains("**Atualizada em:** 2026-11-10"),
        "{leia_me}"
    );
    assert_eq!(paginas_do_projeto(&projeto, None, true).unwrap(), g);
}

#[test]
fn paginas_mascaram_os_dados_pessoais_da_demanda() {
    let (_tmp, projeto) = base();
    let caminho = projeto.raiz().join("analises/A-0001.md");
    let original = analise("A-0001", "aprovada", "Cancelamento", "2026-11-10").replace(
        "## Resumo",
        "## Demanda como veio\n\nCliente CPF 529.982.247-25, empresa 11.222.333/0001-81, \
         ana@example.com, (11) 98765-4321.\n\n## Resumo",
    );
    std::fs::write(&caminho, &original).unwrap();
    let pagina = |mascarar: bool| {
        let g = paginas_do_projeto(&projeto, None, mascarar).unwrap();
        let p = g
            .paginas
            .iter()
            .find(|p| p.caminho == "analises/A-0001.md")
            .unwrap();
        String::from_utf8(p.conteudo.clone()).unwrap()
    };
    let mascarada = pagina(true);
    for marca in [
        "[CPF mascarado]",
        "[CNPJ mascarado]",
        "[e-mail mascarado]",
        "[telefone mascarado]",
    ] {
        assert!(mascarada.contains(marca), "{marca}: {mascarada}");
    }
    assert!(!mascarada.contains("529.982.247-25"), "{mascarada}");
    // A base fica com o original; sem a preferência, a página também.
    assert_eq!(std::fs::read_to_string(&caminho).unwrap(), original);
    let como_veio = pagina(false);
    assert!(
        como_veio.contains("CPF 529.982.247-25, empresa 11.222.333/0001-81"),
        "{como_veio}"
    );
}

#[test]
fn sem_aprovados_nao_ha_paginas() {
    let (_tmp, projeto) = base();
    for c in [
        "regras/RN-FAT-001.md",
        "glossario/nota-fiscal.md",
        "analises/A-0001.md",
    ] {
        std::fs::remove_file(projeto.raiz().join(c)).unwrap();
    }
    assert_eq!(
        paginas_do_projeto(&projeto, None, true).unwrap(),
        Paginas::default()
    );
}

fn repo() -> RepositorioCodigo {
    RepositorioCodigo {
        caminho: "exemplo/faturamento".into(),
        ..Default::default()
    }
}

fn paginas(itens: &[(&str, &str)]) -> Vec<PaginaPublicada> {
    itens
        .iter()
        .map(|(c, t)| PaginaPublicada {
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

/// Manifesto da versão atual com as páginas.
fn manifesto(projeto: &str, itens: &[&str]) -> String {
    String::from_utf8(manifesto_das_paginas(
        projeto,
        &paginas(&itens.iter().map(|c| (*c, "")).collect::<Vec<_>>()),
    ))
    .unwrap()
}

/// Manifesto da versão 1 (#276): os `.md` da base em `documentos`, sem versão.
fn manifesto_v1(projeto: &str, docs: &[&str]) -> String {
    serde_json::to_string_pretty(&serde_json::json!({
        "gerado_por": GERADO_POR,
        "projeto": projeto,
        "documentos": docs,
    }))
    .unwrap()
        + "\n"
}

fn previa_de(p: &Plano) -> Vec<(TipoMudanca, &str)> {
    p.previa
        .iter()
        .map(|m| (m.tipo, m.caminho.as_str()))
        .collect()
}

fn subpasta(s: &str) -> DestinoResolvido<'_> {
    // O repositório vive o bastante: só o rótulo e o caminho são usados.
    static REPO: std::sync::OnceLock<RepositorioCodigo> = std::sync::OnceLock::new();
    DestinoResolvido {
        repositorio: REPO.get_or_init(repo),
        subpasta: s,
        independente: s.is_empty(),
    }
}

#[test]
fn paginas_novas_alteradas_e_removidas_na_subpasta() {
    let destino = subpasta("docs/negocio");
    let novas = paginas(&[
        ("README.md", "# Faturamento (v2)\n"),
        ("glossario.md", "# Glossário\n"),
        ("analises/A-0001.md", "# Cancelamento\n"),
    ]);
    let m = manifesto(
        PROJETO,
        &[
            "README.md",
            "glossario.md",
            "regras/faturamento.md",
            "../../fora.md",
        ],
    );
    let l = remoto(
        &[
            ("docs/negocio/.cba-publicacao.json", &m),
            ("docs/negocio/README.md", "# Faturamento\n"),
            ("docs/negocio/glossario.md", "# Glossário\n"),
            ("docs/negocio/regras/faturamento.md", "# Regras\n"),
            // Escrito à mão: não está no manifesto, fica.
            ("docs/negocio/leia-me.md", "# Docs do Faturamento\n"),
        ],
        None,
    );
    let p = planejar(PROJETO, &novas, &destino, &l).unwrap();
    assert_eq!(
        previa_de(&p),
        [
            (TipoMudanca::Novo, "docs/negocio/analises/A-0001.md"),
            (TipoMudanca::Alterado, "docs/negocio/README.md"),
            (TipoMudanca::Removido, "docs/negocio/regras/faturamento.md"),
        ]
    );
    assert!(p.muda_a_base && !p.pendente_na_branch && !p.formato_antigo);
    let caminhos: Vec<(&str, bool)> = p
        .mudancas
        .iter()
        .map(|m| (m.caminho(), matches!(m, Mudanca::Remover { .. })))
        .collect();
    assert_eq!(
        caminhos,
        [
            ("docs/negocio/README.md", false),
            ("docs/negocio/analises/A-0001.md", false),
            ("docs/negocio/glossario.md", false),
            ("docs/negocio/.cba-publicacao.json", false),
            ("docs/negocio/regras/faturamento.md", true),
        ]
    );
    // O manifesto novo, versão 2, lista as páginas relativas à pasta.
    let Mudanca::Gravar { conteudo, .. } = &p.mudancas[3] else {
        panic!()
    };
    let novo: serde_json::Value = serde_json::from_slice(conteudo).unwrap();
    assert_eq!(novo["versao"], 2);
    assert_eq!(novo["projeto"], PROJETO);
    assert_eq!(
        novo["paginas"],
        serde_json::json!(["README.md", "glossario.md", "analises/A-0001.md"])
    );
    assert!(novo.get("documentos").is_none(), "{novo}");
}

#[test]
fn migra_a_publicacao_no_formato_antigo() {
    let destino = subpasta("docs");
    // A #276 publicou os `.md` da base; o leia-me foi escrito à mão.
    let m = manifesto_v1(
        PROJETO,
        &[
            "regras/RN-FAT-001.md",
            "glossario/nota-fiscal.md",
            "analises/A-0001.md",
        ],
    );
    let l = remoto(
        &[
            ("docs/.cba-publicacao.json", &m),
            ("docs/regras/RN-FAT-001.md", "regra crua\n"),
            ("docs/glossario/nota-fiscal.md", "termo cru\n"),
            ("docs/analises/A-0001.md", "análise crua\n"),
            ("docs/leia-me.md", "# Escrito à mão\n"),
        ],
        None,
    );
    let novas = paginas(&[
        ("README.md", "# Faturamento\n"),
        ("analises/A-0001.md", "# A-0001 legível\n"),
        ("glossario.md", "# Glossário\n"),
        ("regras/faturamento.md", "# Regras\n"),
    ]);
    let p = planejar(PROJETO, &novas, &destino, &l).unwrap();
    assert!(p.formato_antigo && p.muda_a_base);
    assert_eq!(
        previa_de(&p),
        [
            (TipoMudanca::Novo, "docs/README.md"),
            (TipoMudanca::Novo, "docs/glossario.md"),
            (TipoMudanca::Novo, "docs/regras/faturamento.md"),
            // Mesmo caminho: a página substitui o arquivo antigo.
            (TipoMudanca::Alterado, "docs/analises/A-0001.md"),
            (TipoMudanca::Removido, "docs/glossario/nota-fiscal.md"),
            (TipoMudanca::Removido, "docs/regras/RN-FAT-001.md"),
        ]
    );
    // O leia-me escrito à mão nunca entra nas mudanças.
    assert!(
        !p.mudancas.iter().any(|m| m.caminho().contains("leia-me")),
        "{:?}",
        p.mudancas
    );
    assert_eq!(
        p.publicados_na_base.iter().collect::<Vec<_>>(),
        [
            "docs/analises/A-0001.md",
            "docs/glossario/nota-fiscal.md",
            "docs/regras/RN-FAT-001.md"
        ]
    );

    // Arquivo alheio com o nome de uma página nova continua sendo conflito.
    let l = remoto(
        &[
            ("docs/.cba-publicacao.json", &m),
            ("docs/README.md", "# Escrito à mão\n"),
        ],
        None,
    );
    let e = planejar(PROJETO, &novas, &destino, &l).unwrap_err();
    assert!(e.contains("docs/README.md") && e.contains("à mão"), "{e}");
}

#[test]
fn nada_a_publicar_quando_a_base_ja_tem_as_paginas() {
    let destino = subpasta("");
    let novas = paginas(&[("README.md", "# Faturamento\n")]);
    let m = manifesto(PROJETO, &["README.md"]);
    let base = [
        (".cba-publicacao.json", m.as_str()),
        ("README.md", "# Faturamento\n"),
    ];
    let p = planejar(PROJETO, &novas, &destino, &remoto(&base, None)).unwrap();
    assert!(p.previa.is_empty());
    assert!(!p.muda_a_base && !p.pendente_na_branch && !p.branch_atualizada);
    // A branch da publicação igual à base (PR mesclado) também não pende.
    let p = planejar(PROJETO, &novas, &destino, &remoto(&base, Some(&base))).unwrap();
    assert!(!p.muda_a_base && !p.pendente_na_branch);

    // Base sem nada aprovado e nada publicado: nenhuma mudança, nem o manifesto.
    let p = planejar(PROJETO, &[], &destino, &remoto(&[], None)).unwrap();
    assert!(p.mudancas.is_empty() && p.previa.is_empty() && !p.muda_a_base);
}

#[test]
fn branch_pendente_ja_atualizada() {
    let destino = subpasta("docs");
    let novas = paginas(&[
        ("README.md", "# Faturamento (v2)\n"),
        ("glossario.md", "# Glossário\n"),
    ]);
    let m1 = manifesto(PROJETO, &["README.md", "regras/faturamento.md"]);
    let base = [
        ("docs/.cba-publicacao.json", m1.as_str()),
        ("docs/README.md", "# Faturamento\n"),
        ("docs/regras/faturamento.md", "# Regras\n"),
        ("docs/leia-me.md", "# À mão\n"),
    ];
    // O PR aberto já tem exatamente esta versão.
    let m2 = manifesto(PROJETO, &["README.md", "glossario.md"]);
    let branch = [
        ("docs/.cba-publicacao.json", m2.as_str()),
        ("docs/README.md", "# Faturamento (v2)\n"),
        ("docs/glossario.md", "# Glossário\n"),
        ("docs/leia-me.md", "# À mão\n"),
    ];
    let p = planejar(PROJETO, &novas, &destino, &remoto(&base, Some(&branch))).unwrap();
    assert!(p.muda_a_base && p.pendente_na_branch && p.branch_atualizada);
    assert_eq!(p.previa.len(), 3, "a prévia continua contra a base");

    // O PR tem uma versão anterior: há o que enviar.
    let antiga = [
        ("docs/.cba-publicacao.json", m2.as_str()),
        ("docs/README.md", "# Faturamento (v1.5)\n"),
        ("docs/glossario.md", "# Glossário\n"),
    ];
    let p = planejar(PROJETO, &novas, &destino, &remoto(&base, Some(&antiga))).unwrap();
    assert!(p.pendente_na_branch && !p.branch_atualizada);
    // Uma página que sairia ainda está na branch: também não.
    let mut com_sobra = branch.to_vec();
    com_sobra.push(("docs/regras/faturamento.md", "# Regras\n"));
    let p = planejar(PROJETO, &novas, &destino, &remoto(&base, Some(&com_sobra))).unwrap();
    assert!(p.pendente_na_branch && !p.branch_atualizada);
}

#[test]
fn branch_pendente_remove_o_que_ela_publicou() {
    let destino = subpasta("docs");
    // A branch tem um PR aberto com uma página que não é mais gerada.
    let m = manifesto(PROJETO, &["analises/A-0001.md"]);
    let branch = [
        ("docs/.cba-publicacao.json", m.as_str()),
        ("docs/analises/A-0001.md", "# Cancelamento\n"),
    ];
    let p = planejar(PROJETO, &[], &destino, &remoto(&[], Some(&branch))).unwrap();
    assert!(p.previa.is_empty() && p.pendente_na_branch && !p.branch_atualizada);
    assert!(
        p.mudancas
            .contains(&Mudanca::remover("docs/analises/A-0001.md"))
    );
}

#[test]
fn nunca_sobrescreve_documento_escrito_a_mao() {
    let destino = subpasta("docs/negocio");
    let novas = paginas(&[("glossario.md", "# Glossário\n")]);
    let l = remoto(&[("docs/negocio/glossario.md", "# Escrito à mão\n")], None);
    let e = planejar(PROJETO, &novas, &destino, &l).unwrap_err();
    assert!(
        e.contains("docs/negocio/glossario.md") && e.contains("à mão"),
        "{e}"
    );
    // Com o mesmo conteúdo, não há o que sobrescrever.
    let l = remoto(&[("docs/negocio/glossario.md", "# Glossário\n")], None);
    let p = planejar(PROJETO, &novas, &destino, &l).unwrap();
    assert!(p.previa.is_empty());
}

#[test]
fn manifesto_de_outro_projeto_ilegivel_ou_mais_novo_e_recusado() {
    let destino = subpasta("docs");
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
    let futuro = format!(
        r#"{{"versao": 3, "gerado_por": "{GERADO_POR}", "projeto": "{PROJETO}", "paginas": []}}"#
    );
    let e = planejar(
        PROJETO,
        &[],
        &destino,
        &remoto(&[("docs/.cba-publicacao.json", &futuro)], None),
    )
    .unwrap_err();
    assert!(e.contains("versão mais nova"), "{e}");
}

/// Um bloco marcado, como o renderizador escreve.
fn bloco(tipo: &str, id: &str, caminho: &str, fm: serde_json::Value, texto: &str) -> String {
    let m = Marcador::novo(
        tipo,
        id,
        Metadados {
            caminho: caminho.into(),
            frontmatter: fm,
        },
    )
    .unwrap();
    format!(
        "{}\n<a id=\"{}\"></a>\n{texto}\n{}\n",
        m.abertura(),
        id.to_lowercase(),
        cba_docs::marcador::FECHAMENTO
    )
}

fn fm_regra(id: &str, titulo: &str) -> serde_json::Value {
    serde_json::json!({"tipo": "regra", "id": id, "titulo": titulo, "status": "confirmada"})
}

fn lidas(v: &[(&'static str, String)], pasta: &str) -> Vec<(String, String, String)> {
    v.iter()
        .map(|(r, t)| (format!("{pasta}{r}"), (*r).to_owned(), t.clone()))
        .collect()
}

#[test]
fn previa_por_documento_compara_os_blocos() {
    let r1 = bloco(
        "regra",
        "RN-FAT-001",
        "regras/RN-FAT-001.md",
        fm_regra("RN-FAT-001", "Cancelar nota no mesmo dia"),
        "Só no dia.",
    );
    let r2 = |texto: &str| {
        bloco(
            "regra",
            "RN-FAT-002",
            "regras/RN-FAT-002.md",
            fm_regra("RN-FAT-002", "Prazo do estorno"),
            texto,
        )
    };
    let r3 = bloco(
        "regra",
        "RN-FAT-003",
        "regras/RN-FAT-003.md",
        fm_regra("RN-FAT-003", "Nota de serviço"),
        "Serviço.",
    );
    let r4 = bloco(
        "regra",
        "RN-FAT-004",
        "regras/RN-FAT-004.md",
        fm_regra("RN-FAT-004", "Multa"),
        "Multa.",
    );
    let termo = bloco(
        "termo",
        "nota-fiscal",
        "glossario/nota-fiscal.md",
        serde_json::json!({"tipo": "termo", "termo": "Nota fiscal"}),
        "Documento fiscal.",
    );
    let antes = lidas(
        &[
            (
                "regras/faturamento.md",
                format!("# Regras\n\n{r1}\n{}", r2("Em 5 dias.")),
            ),
            ("regras/servicos.md", format!("# Serviços\n\n{r3}")),
        ],
        "docs/",
    );
    // A RN-FAT-001 muda de página sem mudar (vai para uma funcionalidade): não
    // conta como alterada. A RN-FAT-002 muda o texto, a RN-FAT-003 sai, a
    // RN-FAT-004 e o termo entram.
    let depois = lidas(
        &[
            (
                "funcionalidades/f-fat-001.md",
                format!("# F-FAT-001\n\n{r1}"),
            ),
            (
                "regras/faturamento.md",
                format!("# Regras\n\n{}\n{r4}", r2("Em 10 dias.")),
            ),
            ("glossario.md", format!("# Glossário\n\n{termo}")),
        ],
        "docs/",
    );
    let r = comparar_documentos(&super::lidas(&depois), &super::lidas(&antes), false);
    let lista: Vec<(TipoMudanca, &str, &str)> = r
        .documentos
        .iter()
        .map(|d| (d.mudanca, d.id.as_str(), d.pagina.as_str()))
        .collect();
    assert_eq!(
        lista,
        [
            (
                TipoMudanca::Novo,
                "RN-FAT-004",
                "docs/regras/faturamento.md"
            ),
            (TipoMudanca::Novo, "nota-fiscal", "docs/glossario.md"),
            (
                TipoMudanca::Alterado,
                "RN-FAT-002",
                "docs/regras/faturamento.md"
            ),
            (
                TipoMudanca::Removido,
                "RN-FAT-003",
                "docs/regras/servicos.md"
            ),
        ]
    );
    assert_eq!(r.documentos[0].titulo, "Multa");
    assert_eq!(r.documentos[1].titulo, "Nota fiscal");
    assert_eq!(
        r.texto,
        "1 regra nova, 1 alterada, 1 removida; 1 termo novo"
    );
    assert_eq!(r.reformatados, 0);

    // Plural e concordância.
    let depois = lidas(
        &[(
            "regras/faturamento.md",
            format!("{r1}\n{}\n{r3}\n{r4}", r2("x")),
        )],
        "",
    );
    let r = comparar_documentos(&super::lidas(&depois), &[], false);
    assert_eq!(r.texto, "4 regras novas");
    // Sem mudança, sem texto.
    let r = comparar_documentos(&super::lidas(&depois), &super::lidas(&depois), false);
    assert!(r.documentos.is_empty() && r.texto.is_empty(), "{r:?}");
}

#[test]
fn previa_por_documento_na_migracao_do_formato_antigo() {
    let fm1 = "tipo: regra\nid: RN-FAT-001\ntitulo: Cancelar nota\nstatus: confirmada";
    let fm2 = "tipo: regra\nid: RN-FAT-002\ntitulo: Estorno\nstatus: confirmada";
    // Os `.md` da base, como a #276 publicou.
    let antes = lidas(
        &[
            (
                "regras/RN-FAT-001.md",
                format!("---\n{fm1}\n---\n\n# Cancelar nota\n"),
            ),
            (
                "regras/RN-FAT-002.md",
                format!("---\n{fm2}\n---\n\n# Estorno\n"),
            ),
            (
                "regras/RN-FAT-009.md",
                "---\ntipo: regra\nid: RN-FAT-009\ntitulo: Antiga\nstatus: confirmada\n---\n"
                    .into(),
            ),
            (
                "glossario/nota-fiscal.md",
                "---\ntipo: termo\ntermo: Nota fiscal\n---\n\n# Nota fiscal\n".into(),
            ),
            ("notas.md", "sem frontmatter\n".into()),
        ],
        "docs/",
    );
    let depois = lidas(
        &[
            (
                "regras/faturamento.md",
                format!(
                    "{}\n{}",
                    // Mesmo frontmatter: só muda de formato.
                    bloco(
                        "regra",
                        "RN-FAT-001",
                        "regras/RN-FAT-001.md",
                        fm_regra("RN-FAT-001", "Cancelar nota"),
                        "Legível."
                    ),
                    // Título trocado na base depois da publicação: alterada.
                    bloco(
                        "regra",
                        "RN-FAT-002",
                        "regras/RN-FAT-002.md",
                        fm_regra("RN-FAT-002", "Estorno em 10 dias"),
                        "Legível."
                    )
                ),
            ),
            (
                "glossario.md",
                bloco(
                    "termo",
                    "nota-fiscal",
                    "glossario/nota-fiscal.md",
                    serde_json::json!({"tipo": "termo", "termo": "Nota fiscal"}),
                    "Documento fiscal.",
                ),
            ),
        ],
        "docs/",
    );
    let r = comparar_documentos(&super::lidas(&depois), &super::lidas(&antes), true);
    let lista: Vec<(TipoMudanca, &str)> = r
        .documentos
        .iter()
        .map(|d| (d.mudanca, d.id.as_str()))
        .collect();
    assert_eq!(
        lista,
        [
            (TipoMudanca::Alterado, "RN-FAT-002"),
            (TipoMudanca::Removido, "RN-FAT-009"),
        ]
    );
    assert_eq!(r.documentos[1].pagina, "docs/regras/RN-FAT-009.md");
    assert_eq!(r.reformatados, 2, "a RN-FAT-001 e o termo");
    assert_eq!(
        r.texto,
        "1 regra alterada, 1 removida; 2 documentos só mudam de formato"
    );
}

#[test]
fn corpo_do_pr_resume_por_documento_e_aponta_o_readme() {
    let previa = Previa {
        destino: "docs em GitHub exemplo/faturamento".into(),
        repositorio: "GitHub exemplo/faturamento".into(),
        independente: false,
        base: "main".into(),
        branch: branch_da_publicacao(PROJETO),
        aprovados: 3,
        paginas: 4,
        documentos: ResumoDosDocumentos {
            documentos: vec![MudancaDeDocumento {
                tipo: "regra".into(),
                id: "RN-FAT-001".into(),
                titulo: "Cancelar nota".into(),
                mudanca: TipoMudanca::Novo,
                pagina: "docs/regras/faturamento.md".into(),
            }],
            reformatados: 0,
            texto: "1 regra nova".into(),
        },
        mudancas: vec![MudancaPrevista {
            caminho: "docs/regras/faturamento.md".into(),
            tipo: TipoMudanca::Novo,
        }],
        formato_antigo: true,
        pendente_na_branch: false,
        branch_atualizada: false,
        nada_a_publicar: false,
        avisos: vec![],
    };
    let link = repo().link(&previa.branch, "docs/README.md", None);
    assert_eq!(
        link.as_deref(),
        Some(
            "https://github.com/exemplo/faturamento/blob/docs/cba-faturamento-exemplo/docs/README.md"
        )
    );
    let c = corpo_do_pr(&previa, link.as_deref());
    for trecho in [
        "4 páginas geradas de 3 documentos aprovados",
        "[README](https://github.com/exemplo/faturamento/blob/docs/cba-faturamento-exemplo/docs/README.md)",
        "## O que muda\n\n1 regra nova",
        "### Documentos novos (1)",
        "- RN-FAT-001 · Cancelar nota (`docs/regras/faturamento.md`)",
        "formato antigo",
        "- `docs/regras/faturamento.md` (nova)",
    ] {
        assert!(c.contains(trecho), "{trecho}\n---\n{c}");
    }
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
            escopo: None,
            mascarar_demanda: true,
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
            escopo: None,
            mascarar_demanda: true,
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

    /// Remoto com um `leia-me.md` na pasta da documentação (escrito à mão) e,
    /// se pedido, os arquivos de uma publicação anterior; clone do analista.
    /// `subpasta` vazia = repositório próprio da documentação.
    fn cenario_com(subpasta: &str, anteriores: &[(&str, String)]) -> Cenario {
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
        for (c, t) in anteriores {
            let p = pasta_docs.join(c);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, t).unwrap();
        }
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

    fn cenario(subpasta: &str) -> Cenario {
        cenario_com(subpasta, &[])
    }

    impl Cenario {
        fn alvo(&self) -> Alvo<'_> {
            Alvo {
                projeto: &self.projeto,
                nome: "Faturamento Exemplo",
                repositorios: &self.repositorios,
                escopo: None,
                mascarar_demanda: true,
            }
        }

        fn aberturas(&self) -> usize {
            std::fs::read_to_string(self.forja.join("chamadas"))
                .unwrap_or_default()
                .matches("--method POST")
                .count()
        }

        fn arvore(&self, referencia: &str) -> Vec<String> {
            git(&self.remoto, &["ls-tree", "-r", "--name-only", referencia])
                .lines()
                .map(str::to_owned)
                .collect()
        }

        fn ponta(&self) -> String {
            git(&self.remoto, &["rev-parse", &branch_da_publicacao(PROJETO)])
        }

        /// Mescla a branch da publicação na base do remoto (o PR aprovado).
        fn mesclar(&self) {
            let branch = branch_da_publicacao(PROJETO);
            git(&self.remoto, &["update-ref", "refs/heads/main", &branch]);
            let _ = std::fs::remove_file(self.forja.join("aberto"));
        }
    }

    fn docs_da_previa(p: &Previa) -> Vec<(TipoMudanca, String)> {
        p.documentos
            .documentos
            .iter()
            .map(|d| (d.mudanca, d.id.clone()))
            .collect()
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
        // Prévia: os documentos aprovados entram como novos, nas páginas; o
        // leia-me não aparece.
        let p = previa(&c.f, &c.alvo()).unwrap();
        assert_eq!(p.branch, "docs/cba-faturamento-exemplo");
        assert_eq!(p.base, "main");
        assert_eq!(p.aprovados, 3);
        assert_eq!(p.independente, subpasta.is_empty());
        assert!(!p.nada_a_publicar && !p.formato_antigo && !p.branch_atualizada);
        assert_eq!(
            docs_da_previa(&p),
            [
                (TipoMudanca::Novo, "RN-FAT-001".to_owned()),
                (TipoMudanca::Novo, "nota-fiscal".to_owned()),
                (TipoMudanca::Novo, "A-0001".to_owned()),
            ]
        );
        assert_eq!(
            p.documentos.texto,
            "1 regra nova; 1 termo novo; 1 análise nova"
        );
        assert!(p.mudancas.iter().all(|m| m.tipo == TipoMudanca::Novo));
        assert_eq!(p.mudancas.len(), p.paginas);
        assert!(
            p.mudancas.iter().any(|m| m.caminho == pref(LEIA_ME)),
            "{p:?}"
        );
        assert_eq!(c.aberturas(), 0, "a prévia não abre PR");

        // Publica: abre o PR com as páginas e o manifesto.
        let r = publicar(&c.f, &c.alvo()).unwrap();
        assert_eq!(r.publicado.situacao, Situacao::Aberto);
        assert_eq!(
            r.publicado.link.as_deref(),
            Some("https://github.com/exemplo/faturamento/pull/1")
        );
        let branch = branch_da_publicacao(PROJETO);
        let arvore = c.arvore(&branch);
        let mut esperado: Vec<String> = p.mudancas.iter().map(|m| m.caminho.clone()).collect();
        esperado.push(pref(MANIFESTO));
        esperado.push(pref("leia-me.md"));
        esperado.sort();
        let mut obtido = arvore.clone();
        obtido.sort();
        assert_eq!(obtido, esperado);
        assert!(
            !arvore.iter().any(|a| a.ends_with("RN-FAT-001.md")),
            "nenhum .md cru: {arvore:?}"
        );
        let leia_me = git(&c.remoto, &["show", &format!("{branch}:{}", pref(LEIA_ME))]);
        assert!(leia_me.contains("Faturamento"), "{leia_me}");
        // O corpo do PR leva o resumo e o link do README na branch.
        let chamadas = std::fs::read_to_string(c.forja.join("chamadas")).unwrap();
        assert!(chamadas.contains("1 regra nova"), "{chamadas}");
        assert!(
            chamadas.contains(&format!(
                "https://github.com/exemplo/faturamento/blob/{branch}/{}",
                pref(LEIA_ME)
            )),
            "{chamadas}"
        );

        // Com o PR aberto e nada novo na base, a prévia já diz que o PR tem esta
        // versão; publicar não cria commit nem PR.
        let ponta = c.ponta();
        let p = previa(&c.f, &c.alvo()).unwrap();
        assert!(
            p.pendente_na_branch && p.branch_atualizada && p.nada_a_publicar,
            "{p:?}"
        );
        let r = publicar(&c.f, &c.alvo()).unwrap();
        assert_eq!(r.publicado.situacao, Situacao::SemMudancas);
        assert_eq!(
            r.publicado.link.as_deref(),
            Some("https://github.com/exemplo/faturamento/pull/1")
        );
        assert_eq!(c.ponta(), ponta, "sem commit");
        assert_eq!(c.aberturas(), 1);

        // PR mesclado: a prévia diz que não há nada a publicar.
        c.mesclar();
        let p = previa(&c.f, &c.alvo()).unwrap();
        assert!(p.nada_a_publicar && p.mudancas.is_empty(), "{p:?}");
        assert!(!p.branch_atualizada && p.documentos.documentos.is_empty());
        let r = publicar(&c.f, &c.alvo()).unwrap();
        assert_eq!(r.publicado.situacao, Situacao::SemMudancas);
        assert_eq!(c.aberturas(), 1);

        // A regra deixa de ser aprovada e a análise muda de título: removida e
        // alteradas, por documento; o leia-me escrito à mão continua.
        let raiz = c.projeto.raiz();
        std::fs::write(
            raiz.join("regras/RN-FAT-001.md"),
            regra("RN-FAT-001", "obsoleta", "Cancelar nota no mesmo dia", "x"),
        )
        .unwrap();
        std::fs::write(
            raiz.join("analises/A-0001.md"),
            analise(
                "A-0001",
                "aprovada",
                "Cancelamento (revisada)",
                "2026-11-10",
            ),
        )
        .unwrap();
        let p = previa(&c.f, &c.alvo()).unwrap();
        assert_eq!(
            docs_da_previa(&p),
            [
                // O termo cita a análise de origem pelo título.
                (TipoMudanca::Alterado, "nota-fiscal".to_owned()),
                (TipoMudanca::Alterado, "A-0001".to_owned()),
                (TipoMudanca::Removido, "RN-FAT-001".to_owned()),
            ]
        );
        assert_eq!(
            p.documentos.texto,
            "1 regra removida; 1 termo alterado; 1 análise alterada"
        );
        let r = publicar(&c.f, &c.alvo()).unwrap();
        assert_eq!(r.publicado.situacao, Situacao::Aberto);
        let arvore = c.arvore(&branch).join("\n");
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

    #[test]
    fn migra_a_publicacao_antiga_sem_tocar_em_arquivos_alheios() {
        // A #276 publicou os `.md` da base como estavam.
        let (_t, projeto) = base();
        let antigos: Vec<(&str, String)> = [
            "analises/A-0001.md",
            "glossario/nota-fiscal.md",
            "regras/RN-FAT-001.md",
        ]
        .into_iter()
        .map(|c| (c, std::fs::read_to_string(projeto.raiz().join(c)).unwrap()))
        .collect();
        let mut anteriores = antigos.clone();
        anteriores.push((
            MANIFESTO,
            manifesto_v1(
                PROJETO,
                &antigos.iter().map(|(c, _)| *c).collect::<Vec<_>>(),
            ),
        ));
        let c = cenario_com("docs", &anteriores);
        let p = previa(&c.f, &c.alvo()).unwrap();
        assert!(p.formato_antigo && !p.nada_a_publicar, "{p:?}");
        // Os mesmos documentos, com o mesmo frontmatter: só mudam de formato.
        assert!(p.documentos.documentos.is_empty(), "{:?}", p.documentos);
        assert_eq!(p.documentos.reformatados, 3);
        assert_eq!(p.documentos.texto, "3 documentos só mudam de formato");
        let removidos: Vec<&str> = p
            .mudancas
            .iter()
            .filter(|m| m.tipo == TipoMudanca::Removido)
            .map(|m| m.caminho.as_str())
            .collect();
        assert_eq!(
            removidos,
            ["docs/glossario/nota-fiscal.md", "docs/regras/RN-FAT-001.md"]
        );

        let r = publicar(&c.f, &c.alvo()).unwrap();
        assert_eq!(r.publicado.situacao, Situacao::Aberto);
        let arvore = c.arvore(&branch_da_publicacao(PROJETO));
        assert!(arvore.contains(&"docs/leia-me.md".to_owned()), "{arvore:?}");
        assert!(arvore.contains(&"docs/README.md".to_owned()), "{arvore:?}");
        for saiu in ["docs/glossario/nota-fiscal.md", "docs/regras/RN-FAT-001.md"] {
            assert!(!arvore.contains(&saiu.to_owned()), "{saiu}: {arvore:?}");
        }
        let m = git(
            &c.remoto,
            &[
                "show",
                &format!("{}:docs/{MANIFESTO}", branch_da_publicacao(PROJETO)),
            ],
        );
        assert!(m.contains("\"versao\": 2"), "{m}");
    }

    #[test]
    fn sem_aprovados_nada_a_publicar() {
        let c = cenario("docs");
        for d in [
            "regras/RN-FAT-001.md",
            "glossario/nota-fiscal.md",
            "analises/A-0001.md",
        ] {
            std::fs::remove_file(c.projeto.raiz().join(d)).unwrap();
        }
        let p = previa(&c.f, &c.alvo()).unwrap();
        assert_eq!((p.aprovados, p.paginas), (0, 0));
        assert!(p.nada_a_publicar && p.mudancas.is_empty(), "{p:?}");
        let r = publicar(&c.f, &c.alvo()).unwrap();
        assert_eq!(r.publicado.situacao, Situacao::SemMudancas);
        assert_eq!(c.aberturas(), 0);
    }
}
