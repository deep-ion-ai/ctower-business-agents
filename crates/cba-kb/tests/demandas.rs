//! Demandas do projeto (#132): pasta `demandas/D-NNNN-<slug>/`, status, chave
//! externa e ligação das análises pelo campo `demanda.id`.

use std::collections::BTreeSet;
use std::fs;
use std::sync::Arc;
use std::thread;

use cba_kb::frontmatter::{
    Data, Demanda, FrontmatterAnalise, FrontmatterDemanda, FrontmatterProjeto, StatusAnalise,
    StatusDemanda,
};
use cba_kb::indice::{Indice, Tipo};
use cba_kb::{
    Base, CaminhoRelativo, Documento, Erro, Id, IdDemanda, NovaDemanda, Projeto, TipoDocumento,
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

fn nova(titulo: &str) -> NovaDemanda {
    NovaDemanda {
        titulo: titulo.into(),
        ..Default::default()
    }
}

fn analise(id: &str, demanda: Option<IdDemanda>) -> Documento<FrontmatterAnalise> {
    Documento::novo(
        FrontmatterAnalise {
            tipo: TipoDocumento::AnaliseImpacto,
            id: Id::analisar(id).unwrap(),
            titulo: format!("Análise {id}"),
            projeto: PROJETO.into(),
            demanda: Demanda {
                id: demanda,
                origem: "reuniao".into(),
                chave: Some("FAT-12".into()),
            },
            status: StatusAnalise::Rascunho,
            abrangencia: None,
            base: None,
            criado: data(2),
            analista: None,
            origem: None,
        },
        format!("\n# Análise {id}\n"),
    )
}

#[test]
fn cria_le_e_numera_em_sequencia() {
    let (_dir, _base, projeto) = criar_base();
    assert!(projeto.demandas().unwrap().is_empty());
    assert_eq!(projeto.proxima_demanda().unwrap().to_string(), "D-0001");

    let d1 = projeto
        .criar_demanda(
            &NovaDemanda {
                titulo: "  Cancelar nota fiscal de serviço  ".into(),
                descricao: Some("O cliente quer cancelar notas em até 24 h.".into()),
                chave: Some(" FAT-12 ".into()),
                origem: Some("https://github.com/exemplo/faturamento/issues/12".into()),
            },
            data(3),
        )
        .unwrap();
    assert_eq!(d1.id.to_string(), "D-0001");
    assert_eq!(
        d1.pasta.as_str(),
        "demandas/D-0001-cancelar-nota-fiscal-de-servico"
    );
    assert_eq!(
        d1.arquivo.as_str(),
        "demandas/D-0001-cancelar-nota-fiscal-de-servico/demanda.md"
    );
    let f = &d1.documento.frontmatter;
    assert_eq!(f.titulo, "Cancelar nota fiscal de serviço");
    assert_eq!(f.projeto, PROJETO);
    assert_eq!(f.status, StatusDemanda::Aberta);
    assert_eq!(f.chave.as_deref(), Some("FAT-12"));
    assert_eq!(f.criado, data(3));
    assert_eq!(f.encerrado, None);
    assert!(
        d1.documento
            .corpo
            .contains("O cliente quer cancelar notas em até 24 h.")
    );

    // O arquivo é um documento da base como os outros.
    let texto = fs::read_to_string(projeto.caminho(&d1.arquivo)).unwrap();
    assert!(
        texto.starts_with("---\ntipo: demanda\nid: D-0001\n"),
        "{texto}"
    );
    let lido: Documento<FrontmatterDemanda> = projeto.ler(&d1.arquivo).unwrap();
    assert_eq!(lido.frontmatter, *f);

    let d2 = projeto
        .criar_demanda(&nova("Reajuste anual"), data(4))
        .unwrap();
    assert_eq!(d2.id.to_string(), "D-0002");
    // Título sem letras nem dígitos: pasta sem slug.
    let d3 = projeto.criar_demanda(&nova("???"), data(4)).unwrap();
    assert_eq!(d3.pasta.as_str(), "demandas/D-0003");

    for d in [&d2, &d3] {
        let lida = projeto.demanda(d.id).unwrap();
        assert_eq!(lida.pasta, d.pasta);
        assert_eq!(lida.documento.frontmatter, d.documento.frontmatter);
        assert_eq!(lida.documento.corpo, d.documento.corpo);
    }
    assert!(matches!(
        projeto.demanda(IdDemanda::novo(9)),
        Err(Erro::DemandaInexistente { .. })
    ));
}

#[test]
fn titulo_vazio_e_recusado_sem_deixar_pasta() {
    let (_dir, _base, projeto) = criar_base();
    let erro = projeto.criar_demanda(&nova("   "), data(3)).unwrap_err();
    assert!(erro.to_string().contains("titulo"), "{erro}");
    let pasta = projeto.raiz().join("demandas");
    assert!(!pasta.exists() || fs::read_dir(pasta).unwrap().next().is_none());
}

#[test]
fn numero_nao_reaproveita_pasta_existente() {
    let (_dir, _base, projeto) = criar_base();
    // Pasta criada à mão, com slug, e uma trava sem documento (criação interrompida).
    fs::create_dir_all(projeto.raiz().join("demandas/D-0004-antiga")).unwrap();
    fs::create_dir_all(projeto.raiz().join("demandas/D-0005")).unwrap();
    let d = projeto.criar_demanda(&nova("Nova"), data(3)).unwrap();
    assert_eq!(d.id.to_string(), "D-0006");
    // As pastas sem `demanda.md` não aparecem na lista.
    let ids: Vec<_> = projeto
        .demandas()
        .unwrap()
        .iter()
        .map(|d| d.id.to_string())
        .collect();
    assert_eq!(ids, ["D-0006"]);
}

#[test]
fn encerra_reabre_e_lista_abertas_primeiro() {
    let (_dir, _base, projeto) = criar_base();
    for titulo in ["Primeira", "Segunda", "Terceira"] {
        projeto.criar_demanda(&nova(titulo), data(3)).unwrap();
    }
    let d2 = IdDemanda::novo(2);
    let encerrada = projeto.encerrar_demanda(d2, data(10)).unwrap();
    assert_eq!(
        encerrada.documento.frontmatter.status,
        StatusDemanda::Encerrada
    );
    assert_eq!(encerrada.documento.frontmatter.encerrado, Some(data(10)));
    // Encerrar de novo mantém a data original.
    let de_novo = projeto.encerrar_demanda(d2, data(12)).unwrap();
    assert_eq!(de_novo.documento.frontmatter.encerrado, Some(data(10)));

    let ordem: Vec<_> = projeto
        .demandas()
        .unwrap()
        .iter()
        .map(|d| (d.id.to_string(), d.aberta()))
        .collect();
    assert_eq!(
        ordem,
        [
            ("D-0003".to_owned(), true),
            ("D-0001".to_owned(), true),
            ("D-0002".to_owned(), false),
        ]
    );

    let reaberta = projeto.reabrir_demanda(d2).unwrap();
    assert_eq!(reaberta.documento.frontmatter.status, StatusDemanda::Aberta);
    assert_eq!(reaberta.documento.frontmatter.encerrado, None);
    let lida = projeto.demanda(d2).unwrap();
    assert_eq!(lida.documento.frontmatter.status, StatusDemanda::Aberta);
}

#[test]
fn preserva_campos_e_comentarios_editados_a_mao() {
    let (_dir, _base, projeto) = criar_base();
    let d = projeto.criar_demanda(&nova("Editada"), data(3)).unwrap();
    let caminho = projeto.caminho(&d.arquivo);
    let texto = fs::read_to_string(&caminho).unwrap().replacen(
        "status: aberta\n",
        "status: aberta # muda no app\nprioridade: alta\n",
        1,
    );
    fs::write(&caminho, &texto).unwrap();
    projeto.encerrar_demanda(d.id, data(5)).unwrap();
    let depois = fs::read_to_string(&caminho).unwrap();
    assert!(depois.contains("prioridade: alta"), "{depois}");
    assert!(depois.contains("status: encerrada"), "{depois}");
    assert!(depois.contains("encerrado: 2026-11-05"), "{depois}");
}

#[test]
fn id_diferente_da_pasta_fica_fora_da_lista() {
    let (_dir, _base, projeto) = criar_base();
    let d = projeto.criar_demanda(&nova("Copiada"), data(3)).unwrap();
    let copia = projeto.raiz().join("demandas/D-0007-copiada");
    fs::create_dir_all(&copia).unwrap();
    fs::copy(projeto.caminho(&d.arquivo), copia.join("demanda.md")).unwrap();
    let ids: Vec<_> = projeto.demandas().unwrap().iter().map(|d| d.id).collect();
    assert_eq!(ids, [d.id]);
    assert!(matches!(
        projeto.demanda(IdDemanda::novo(7)),
        Err(Erro::Frontmatter { .. })
    ));
}

#[test]
fn processos_ao_mesmo_tempo_nao_colidem() {
    let (_dir, _base, projeto) = criar_base();
    let projeto = Arc::new(projeto);
    let threads: Vec<_> = (0..8)
        .map(|i| {
            let projeto = Arc::clone(&projeto);
            thread::spawn(move || {
                (0..5)
                    .map(|j| {
                        projeto
                            .criar_demanda(&nova(&format!("Demanda {i} {j}")), data(3))
                            .unwrap()
                            .id
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
    let unicos: BTreeSet<_> = numeros.iter().copied().collect();
    assert_eq!(unicos.len(), 40, "{numeros:?}");
    assert_eq!(unicos, (1..=40).collect());
    // Uma pasta por número, todas com o slug.
    let pastas: Vec<String> = fs::read_dir(projeto.raiz().join("demandas"))
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(pastas.len(), 40, "{pastas:?}");
    assert!(pastas.iter().all(|p| p.contains("-demanda-")), "{pastas:?}");
    assert_eq!(projeto.demandas().unwrap().len(), 40);
}

#[test]
fn analise_sem_demanda_continua_valida() {
    let (_dir, _base, projeto) = criar_base();
    // Análise da v1: `demanda` sem o campo `id`.
    let texto = "---\ntipo: pergunta\nid: A-0001\ntitulo: Pergunta\nprojeto: faturamento-exemplo\n\
                 demanda: {origem: texto, chave: null}\nstatus: rascunho\ncriado: 2026-11-11\n\
                 ---\n\n# Pergunta\n";
    let caminho = CaminhoRelativo::novo("analises/A-0001.md").unwrap();
    fs::write(projeto.caminho(&caminho), texto).unwrap();
    let mut doc: Documento<FrontmatterAnalise> = projeto.ler(&caminho).unwrap();
    assert_eq!(doc.frontmatter.demanda.id, None);
    // Gravar de novo sem mudança não acrescenta o campo.
    projeto.gravar(&caminho, &doc).unwrap();
    assert_eq!(
        fs::read_to_string(projeto.caminho(&caminho)).unwrap(),
        texto
    );
    // Ligar a uma demanda grava o identificador.
    doc.frontmatter.demanda.id = Some(IdDemanda::novo(7));
    projeto.gravar(&caminho, &doc).unwrap();
    let lido: Documento<FrontmatterAnalise> = projeto.ler(&caminho).unwrap();
    assert_eq!(lido.frontmatter.demanda.id, Some(IdDemanda::novo(7)));
    assert!(
        fs::read_to_string(projeto.caminho(&caminho))
            .unwrap()
            .contains("id: D-0007")
    );
}

#[test]
fn indice_liga_analises_a_demanda() {
    let (dir, base, projeto) = criar_base();
    let d1 = projeto
        .criar_demanda(&nova("Cancelar nota fiscal"), data(3))
        .unwrap();
    let d2 = projeto.criar_demanda(&nova("Reajuste"), data(3)).unwrap();
    for (id, demanda) in [
        ("A-0001", Some(d1.id)),
        ("A-0002", None),
        ("A-0003", Some(d1.id)),
        ("A-0004", Some(d2.id)),
        // Demanda que não existe: ligação quebrada, sem erro.
        ("A-0005", Some(IdDemanda::novo(9))),
    ] {
        let doc = analise(id, demanda);
        projeto
            .criar(&Id::analisar(id).unwrap().arquivo(), &doc)
            .unwrap();
    }

    let mut indice = Indice::abrir(dir.path().join("indice/base.sqlite"), &base).unwrap();
    indice.sincronizar().unwrap();
    assert!(indice.problemas().unwrap().is_empty());

    let ligadas: Vec<_> = indice
        .documentos_da_demanda(PROJETO, d1.id)
        .unwrap()
        .into_iter()
        .map(|d| d.identificador.unwrap())
        .collect();
    assert_eq!(ligadas, ["A-0001", "A-0003"]);
    let d2_docs = indice.documentos_da_demanda(PROJETO, d2.id).unwrap();
    assert_eq!(d2_docs.len(), 1);
    assert_eq!(
        d2_docs[0].tipo,
        Some(Tipo::Documento(TipoDocumento::AnaliseImpacto))
    );
    assert!(
        indice
            .documentos_da_demanda("outro", d1.id)
            .unwrap()
            .is_empty()
    );

    // A ligação aponta para o demanda.md, que também está no índice.
    let arquivo = projeto.na_base(&d1.arquivo);
    let ligacoes = indice.ligacoes(&arquivo).unwrap();
    assert_eq!(ligacoes.len(), 2);
    assert!(ligacoes.iter().all(|l| l.campo == "demanda" && l.existe));
    let quebrada = indice
        .ligacoes(&CaminhoRelativo::novo(&format!("{PROJETO}/analises/A-0005.md")).unwrap())
        .unwrap();
    assert_eq!(quebrada.len(), 1);
    assert!(!quebrada[0].existe);
    assert_eq!(
        quebrada[0].para.as_str(),
        format!("{PROJETO}/demandas/D-0009/demanda.md")
    );

    // A demanda é buscável pelo título.
    let pagina = indice
        .buscar(&cba_kb::indice::Consulta::nova("cancelar nota").projeto(PROJETO))
        .unwrap();
    assert!(
        pagina
            .resultados
            .iter()
            .any(|r| r.identificador.as_deref() == Some("D-0001")
                && r.tipo == Some(Tipo::Documento(TipoDocumento::Demanda))),
        "{pagina:?}"
    );

    // O índice reconstruído do zero traz as mesmas ligações.
    indice.reconstruir().unwrap();
    assert_eq!(
        indice.documentos_da_demanda(PROJETO, d1.id).unwrap().len(),
        2
    );
}
