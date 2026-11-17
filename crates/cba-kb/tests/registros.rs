//! Premissas, decisões, riscos e restrições da demanda (#122): arquivos na pasta
//! da demanda, numeração por projeto, histórico, validação de premissa, recusas e
//! o índice com as ligações para a demanda, a origem e a pessoa.

use cba_kb::frontmatter::{Data, FrontmatterProjeto};
use cba_kb::indice::{Indice, Tipo};
use cba_kb::registros::{Nivel, StatusPremissa, TipoRestricao};
use cba_kb::{
    Base, Categoria, DadosRegistro, Documento, Erro, IdDemanda, NovaDemanda, NovaPessoa,
    NovoRegistro, Projeto, TipoDocumento, ValidacaoDePremissa,
};

const PROJETO: &str = "faturamento-exemplo";

fn data(dia: u8) -> Data {
    Data::nova(2026, 11, dia).unwrap()
}

fn criar_base() -> (tempfile::TempDir, Base, Projeto) {
    let dir = tempfile::tempdir().unwrap();
    let base = Base::criar(dir.path().join("base")).unwrap();
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

fn demanda(projeto: &Projeto, titulo: &str) -> IdDemanda {
    projeto
        .criar_demanda(
            &NovaDemanda {
                titulo: titulo.into(),
                ..Default::default()
            },
            data(2),
        )
        .unwrap()
        .id
}

fn manual(categoria: Categoria, dados: DadosRegistro) -> NovoRegistro {
    NovoRegistro {
        categoria,
        dados,
        origem: None,
        referencia: None,
        evento: "criado à mão".into(),
    }
}

#[test]
fn numeracao_por_projeto_e_arquivos_na_pasta_da_demanda() {
    let (_dir, _base, projeto) = criar_base();
    let d1 = demanda(&projeto, "Parcelamento PJ");
    let d2 = demanda(&projeto, "Multa por atraso");
    let premissa = |titulo: &str| {
        manual(
            Categoria::Premissa,
            DadosRegistro {
                titulo: titulo.into(),
                ..Default::default()
            },
        )
    };
    let p1 = projeto
        .criar_registro(
            d1,
            &premissa("Clientes PJ já têm limite de crédito."),
            data(3),
        )
        .unwrap();
    let p2 = projeto
        .criar_registro(d2, &premissa("A multa é de 2%."), data(3))
        .unwrap();
    let r1 = projeto
        .criar_registro(
            d1,
            &manual(
                Categoria::Risco,
                DadosRegistro {
                    titulo: "Integração bancária rejeita remessa com parcelas.".into(),
                    probabilidade: Some(Nivel::Alto),
                    impacto: Some(Nivel::Alto),
                    resposta: Some("Testar com o banco antes".into()),
                    ..Default::default()
                },
            ),
            data(3),
        )
        .unwrap();
    assert_eq!(p1.id.to_string(), "PR-0001");
    assert_eq!(p2.id.to_string(), "PR-0002");
    assert_eq!(r1.id.to_string(), "RI-0001");
    assert_eq!(
        p1.arquivo.as_str(),
        "demandas/D-0001-parcelamento-pj/registros/PR-0001.md"
    );
    // A premissa sem status nasce não validada.
    assert_eq!(p1.frontmatter().status, Some(StatusPremissa::NaoValidada));

    let texto = projeto.ler_texto(&r1.arquivo).unwrap();
    assert!(texto.contains("tipo: risco"), "{texto}");
    assert!(texto.contains("probabilidade: alto"), "{texto}");
    assert!(texto.contains("# RI-0001: Integração bancária"), "{texto}");
    assert!(!texto.contains("status:"), "{texto}");

    let da_d1: Vec<String> = projeto
        .registros_da_demanda(d1)
        .unwrap()
        .iter()
        .map(|r| r.id.to_string())
        .collect();
    assert_eq!(da_d1, ["PR-0001", "RI-0001"]);
    assert_eq!(projeto.registro(p2.id).unwrap().frontmatter().demanda, d2);
}

#[test]
fn recusa_campos_obrigatorios_e_pessoa_inexistente() {
    let (_dir, _base, projeto) = criar_base();
    let d1 = demanda(&projeto, "Parcelamento PJ");
    let erro = projeto
        .criar_registro(
            d1,
            &manual(
                Categoria::Restricao,
                DadosRegistro {
                    titulo: "Entrar antes do fechamento fiscal.".into(),
                    ..Default::default()
                },
            ),
            data(3),
        )
        .unwrap_err();
    assert!(erro.to_string().contains("restricao"), "{erro}");
    let erro = projeto
        .criar_registro(
            d1,
            &manual(
                Categoria::Decisao,
                DadosRegistro {
                    titulo: "Máximo de 3 parcelas.".into(),
                    pessoa: Some("ninguem".into()),
                    ..Default::default()
                },
            ),
            data(3),
        )
        .unwrap_err();
    assert!(matches!(erro, Erro::PessoaInexistente { .. }), "{erro}");
    // Nenhum número foi gasto.
    assert_eq!(
        projeto
            .proximo_registro(Categoria::Decisao)
            .unwrap()
            .to_string(),
        "DE-0001"
    );
}

#[test]
fn validar_premissa_grava_status_como_e_historico() {
    let (_dir, _base, projeto) = criar_base();
    let d1 = demanda(&projeto, "Parcelamento PJ");
    let p = projeto
        .criar_registro(
            d1,
            &manual(
                Categoria::Premissa,
                DadosRegistro {
                    titulo: "Parcelamento só para faturas acima de R$ 300,00.".into(),
                    ..Default::default()
                },
            ),
            data(3),
        )
        .unwrap();

    // Validar sem dizer como é recusado.
    assert!(
        projeto
            .validar_premissa(
                p.id,
                &ValidacaoDePremissa {
                    status: StatusPremissa::Validada,
                    como: None,
                    pergunta: None,
                },
                data(4),
            )
            .is_err()
    );

    let validada = projeto
        .validar_premissa(
            p.id,
            &ValidacaoDePremissa {
                status: StatusPremissa::Validada,
                como: Some("Confirmado pela PO na reunião R-0002".into()),
                pergunta: Some("A-0001/2".into()),
            },
            data(4),
        )
        .unwrap();
    let f = validada.frontmatter();
    assert_eq!(f.status, Some(StatusPremissa::Validada));
    let v = f.validacao.as_ref().unwrap();
    assert_eq!(v.como, "Confirmado pela PO na reunião R-0002");
    assert_eq!(v.pergunta.as_deref(), Some("A-0001/2"));
    assert_eq!(v.em, data(4));
    assert_eq!(f.historico.len(), 2);
    assert!(f.historico[1].evento.starts_with("validada"));

    // Uma decisão não é premissa.
    let de = projeto
        .criar_registro(
            d1,
            &manual(
                Categoria::Decisao,
                DadosRegistro {
                    titulo: "Máximo de 3 parcelas.".into(),
                    ..Default::default()
                },
            ),
            data(3),
        )
        .unwrap();
    assert!(
        projeto
            .validar_premissa(
                de.id,
                &ValidacaoDePremissa {
                    status: StatusPremissa::Validada,
                    como: Some("x".into()),
                    pergunta: None,
                },
                data(4),
            )
            .is_err()
    );

    // Voltar a não validada apaga a validação.
    let volta = projeto
        .validar_premissa(
            p.id,
            &ValidacaoDePremissa {
                status: StatusPremissa::NaoValidada,
                como: None,
                pergunta: None,
            },
            data(5),
        )
        .unwrap();
    assert!(volta.frontmatter().validacao.is_none());
    assert_eq!(volta.frontmatter().historico.len(), 3);
}

#[test]
fn editar_registro_mantem_id_e_acrescenta_historico() {
    let (_dir, _base, projeto) = criar_base();
    let d1 = demanda(&projeto, "Parcelamento PJ");
    let re = projeto
        .criar_registro(
            d1,
            &manual(
                Categoria::Restricao,
                DadosRegistro {
                    titulo: "Sem alteração no layout CNAB.".into(),
                    restricao: Some(TipoRestricao::Tecnica),
                    ..Default::default()
                },
            ),
            data(3),
        )
        .unwrap();
    let editado = projeto
        .editar_registro(
            re.id,
            &DadosRegistro {
                titulo: "Sem alteração no layout CNAB da remessa atual.".into(),
                detalhe: Some("O banco não aceita outro layout.".into()),
                restricao: Some(TipoRestricao::Tecnica),
                ..Default::default()
            },
            data(5),
        )
        .unwrap();
    assert_eq!(editado.id, re.id);
    assert_eq!(
        editado.detalhe().as_deref(),
        Some("O banco não aceita outro layout.")
    );
    assert_eq!(editado.frontmatter().historico.len(), 2);
    // Editar sem mudar nada não acrescenta evento.
    let igual = projeto
        .editar_registro(
            re.id,
            &DadosRegistro {
                titulo: "Sem alteração no layout CNAB da remessa atual.".into(),
                detalhe: Some("O banco não aceita outro layout.".into()),
                restricao: Some(TipoRestricao::Tecnica),
                ..Default::default()
            },
            data(6),
        )
        .unwrap();
    assert_eq!(igual.frontmatter().historico.len(), 2);
}

#[test]
fn recusas_ficam_na_pasta_dos_registros() {
    let (_dir, _base, projeto) = criar_base();
    let d1 = demanda(&projeto, "Parcelamento PJ");
    assert!(projeto.propostas_recusadas(d1).unwrap().is_empty());
    projeto.recusar_proposta(d1, "A-0001/3").unwrap();
    projeto.recusar_proposta(d1, "A-0001/3").unwrap();
    projeto.recusar_proposta(d1, "R-0001/D-2").unwrap();
    assert_eq!(
        projeto.propostas_recusadas(d1).unwrap(),
        ["A-0001/3", "R-0001/D-2"]
    );
}

#[test]
fn indice_liga_registro_a_demanda_ata_e_pessoa() {
    let (dir, base, projeto) = criar_base();
    let d1 = demanda(&projeto, "Parcelamento PJ");
    projeto
        .registrar_pessoa(&NovaPessoa {
            nome: "Maria Silva".into(),
            papel: Some("PO".into()),
            ..Default::default()
        })
        .unwrap();
    let ata = projeto
        .demanda(d1)
        .unwrap()
        .pasta
        .juntar("reunioes/R-0001.md")
        .unwrap();
    let de = projeto
        .criar_registro(
            d1,
            &NovoRegistro {
                categoria: Categoria::Decisao,
                dados: DadosRegistro {
                    titulo: "Máximo de 3 parcelas.".into(),
                    pessoa: Some("maria-silva".into()),
                    decidido_em: Some(data(10)),
                    alternativas: vec!["6 parcelas".into(), "sem limite".into()],
                    ..Default::default()
                },
                origem: Some(ata.clone()),
                referencia: Some("R-0001/D-1".into()),
                evento: "aceita a proposta do agente (R-0001/D-1)".into(),
            },
            data(10),
        )
        .unwrap();
    assert_eq!(de.frontmatter().quem.as_deref(), Some("Maria Silva"));

    let caminho_indice = dir.path().join("indice/base.sqlite");
    let mut indice = Indice::abrir(&caminho_indice, &base).unwrap();
    indice.sincronizar().unwrap();
    assert!(indice.problemas().unwrap().is_empty());
    let ligados = indice.documentos_da_demanda(PROJETO, d1).unwrap();
    let decisao = ligados
        .iter()
        .find(|d| d.tipo == Some(Tipo::Documento(TipoDocumento::Decisao)))
        .expect("a decisão entra no índice ligada à demanda");
    assert_eq!(decisao.identificador.as_deref(), Some("DE-0001"));
    assert_eq!(decisao.titulo, "Máximo de 3 parcelas.");

    let ligacoes = indice.ligacoes(&projeto.na_base(&de.arquivo)).unwrap();
    let campo = |c: &str| {
        ligacoes
            .iter()
            .find(|l| l.campo == c)
            .unwrap_or_else(|| panic!("sem ligação {c}: {ligacoes:?}"))
            .para
            .as_str()
            .to_owned()
    };
    assert_eq!(campo("origem"), projeto.na_base(&ata).as_str());
    assert_eq!(
        campo("pessoa"),
        "faturamento-exemplo/pessoas/maria-silva.md"
    );
    assert!(campo("demanda").ends_with("/demanda.md"));

    // Apagar o índice e reconstruir recupera os registros.
    drop(indice);
    std::fs::remove_file(&caminho_indice).unwrap();
    let mut indice = Indice::abrir(&caminho_indice, &base).unwrap();
    indice.sincronizar().unwrap();
    assert!(
        indice
            .documentos_da_demanda(PROJETO, d1)
            .unwrap()
            .iter()
            .any(|d| d.identificador.as_deref() == Some("DE-0001"))
    );
}
