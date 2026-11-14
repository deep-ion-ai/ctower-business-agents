//! Ata de reunião (#135): estado com itens de cada seção, montagem pelo template
//! `ata`, gravação na pasta da demanda e exportação para Markdown e Word, com
//! dados fictícios do "Faturamento Exemplo".
//!
//! Para atualizar o texto esperado (`tests/esperados/ata.md`):
//! `CBA_ATUALIZAR_ESPERADOS=1 cargo test -p cba-docs`, e revise o diff.

use std::fs;
use std::io::{Cursor, Read};
use std::path::Path;

use cba_docs::{
    Citacao, CitacaoFala, ErroAta, EstadoAta, NovoItem, OpcoesExportacao, Participante,
    StatusCitacao, Templates, TipoItem, arquivo_do_template, exportar_markdown, exportar_word,
};
use cba_kb::frontmatter::{
    Data, FrontmatterAta, FrontmatterLivre, FrontmatterProjeto, Horario, Linhas, StatusAta,
};
use cba_kb::{
    Base, CaminhoRelativo, Documento, IdDemanda, IdReuniao, NovaDemanda, Projeto, TipoDocumento,
};

const PROJETO: &str = "faturamento-exemplo";

fn data(dia: u8) -> Data {
    Data::nova(2026, 11, dia).unwrap()
}

fn h(texto: &str) -> Horario {
    Horario::analisar(texto).unwrap()
}

fn fala(falante: &str, inicio: &str, trecho: &str) -> CitacaoFala {
    CitacaoFala::nova(falante, h(inicio), trecho, StatusCitacao::Conferida)
}

/// Registra no estado um item de cada seção, com as falas de origem.
fn registrar_itens(e: &mut EstadoAta) {
    e.definir_duracao(Some(h("00:47:10")));
    e.definir_participantes(vec![
        Participante {
            nome: "Maria Exemplo".into(),
            papel: Some("PO".into()),
            pessoa: Some("maria-exemplo".into()),
            proposta: None,
        },
        Participante {
            nome: "João Fictício".into(),
            papel: Some("usuário-chave".into()),
            pessoa: None,
            proposta: None,
        },
        Participante {
            nome: "Ana Exemplo".into(),
            papel: None,
            pessoa: None,
            proposta: None,
        },
    ]);
    e.definir_objetivo("Entender o pedido de parcelamento da fatura para pessoa jurídica.");
    e.definir_resumo(
        "O cliente quer parcelar faturas de pessoa jurídica em até 12 vezes.\n\n\
         Hoje o sistema limita o parcelamento a 6 parcelas.",
    );

    let d1 = e
        .adicionar_item(
            NovoItem::novo(
                TipoItem::Decisao,
                "O parcelamento vale só para pessoa jurídica.",
            )
            .com_citacao(fala(
                "Maria Exemplo",
                "00:12:34",
                "fica só para PJ por enquanto",
            )),
        )
        .unwrap();
    assert_eq!(d1.to_string(), "D-1");

    let mut n1 = NovoItem::novo(
        TipoItem::Necessidade,
        "Parcelar a fatura de pessoa jurídica em até 12 vezes.",
    )
    .com_citacao(fala(
        "João Fictício",
        "00:05:02",
        "a gente precisa de 12 parcelas",
    ))
    .com_citacao(fala(
        "Maria Exemplo",
        "00:06:40",
        "12 é o que o comercial prometeu",
    ));
    n1.evidencias.push(Citacao::nova(
        CaminhoRelativo::novo("db/02_parametros.sql").unwrap(),
        Linhas::novo(15, 16).unwrap(),
    ));
    let n1 = e.adicionar_item(n1).unwrap();

    let mut r1 = NovoItem::novo(
        TipoItem::Regra,
        "Cada parcela tem valor mínimo de R$ 50,00.",
    )
    .com_citacao(fala(
        "João Fictício",
        "00:21:15",
        "parcela abaixo de 50 reais não pode",
    ));
    r1.citacoes[0].status = StatusCitacao::NaoConfere;
    e.adicionar_item(r1).unwrap();

    let mut p1 = NovoItem::novo(
        TipoItem::Pendencia,
        "Enviar a lista de clientes PJ que pediram parcelamento.",
    )
    .com_citacao(fala(
        "Maria Exemplo",
        "00:40:03",
        "te mando a lista até sexta",
    ));
    p1.responsavel = Some("Maria Exemplo".into());
    p1.prazo = Some("2026-11-20".into());
    e.adicionar_item(p1).unwrap();

    e.adicionar_item(
        NovoItem::novo(
            TipoItem::Duvida,
            "O limite de 12 parcelas vale também para órgão público?",
        )
        .com_citacao(fala("Ana Exemplo", "00:33:48", "e prefeitura, entra?")),
    )
    .unwrap();

    let mut pp1 = NovoItem::novo(
        TipoItem::ProximoPasso,
        "Analisar o impacto de aumentar o limite de parcelas.",
    );
    pp1.referencias = vec![n1, d1];
    e.adicionar_item(pp1).unwrap();
}

fn estado_exemplo() -> EstadoAta {
    let mut e = EstadoAta::nova(IdReuniao::novo(1), PROJETO, IdDemanda::novo(1), data(12));
    registrar_itens(&mut e);
    e
}

fn caminho_da_ata() -> CaminhoRelativo {
    CaminhoRelativo::novo("demandas/D-0001-parcelamento-pj/reunioes/R-0001.md").unwrap()
}

fn conferir_esperado(nome: &str, texto: &str) {
    let caminho = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/esperados")
        .join(format!("{nome}.md"));
    if std::env::var_os("CBA_ATUALIZAR_ESPERADOS").is_some() {
        fs::write(&caminho, texto).unwrap();
        return;
    }
    let esperado = fs::read_to_string(&caminho)
        .unwrap_or_else(|e| panic!("{}: {e}", caminho.display()))
        .replace("\r\n", "\n");
    assert!(
        texto == esperado,
        "{nome}: a ata montada difere de {}\n--- montada ---\n{texto}",
        caminho.display()
    );
}

#[test]
fn ata_com_itens_de_cada_secao_monta_pelo_template() {
    let estado = estado_exemplo();
    let montado = Templates::padrao()
        .montar_ata(&estado, caminho_da_ata())
        .unwrap();
    assert!(montado.avisos.is_empty(), "{:?}", montado.avisos);
    let texto = montado.documento.para_texto().unwrap();

    // Frontmatter válido, lido de volta pelo tipo e pelo leitor genérico.
    let lido: Documento<FrontmatterAta> = Documento::analisar(&texto).unwrap();
    assert_eq!(lido.frontmatter, estado.frontmatter());
    let livre: Documento<FrontmatterLivre> = Documento::analisar(&texto).unwrap();
    assert_eq!(livre.frontmatter.tipo(), Some(TipoDocumento::Ata));

    // Determinística.
    let de_novo = Templates::padrao()
        .montar_ata(&estado, caminho_da_ata())
        .unwrap();
    assert_eq!(de_novo.documento.para_texto().unwrap(), texto);

    // Cada item mostra a citação com falante e horário.
    for item in &estado.itens {
        let linha = texto
            .lines()
            .find(|l| l.starts_with(&format!("- **{}** ", item.id)))
            .unwrap_or_else(|| panic!("falta o item {}", item.id));
        for c in &item.citacoes {
            assert!(
                linha.contains(&format!("{}, {}", c.falante, c.inicio)),
                "{linha}"
            );
        }
    }
    // E as referências trazem o trecho e a conferência.
    assert!(texto.contains(
        "- **R-1** · João Fictício, 00:21:15: “parcela abaixo de 50 reais não pode” _(não confere)_"
    ));
    // Link relativo para a demanda e para a pessoa do catálogo.
    assert!(texto.contains("- **Demanda:** [D-0001](../demanda.md)"));
    assert!(texto.contains("- [Maria Exemplo](../../../pessoas/maria-exemplo.md) · PO"));

    conferir_esperado("ata", &texto);
}

#[test]
fn ata_sem_itens_tem_todas_as_secoes() {
    let estado = EstadoAta::nova(IdReuniao::novo(2), PROJETO, IdDemanda::novo(1), data(13));
    let montado = Templates::padrao()
        .montar_ata(&estado, caminho_da_ata())
        .unwrap();
    assert!(montado.avisos.is_empty());
    let corpo = montado.documento.corpo;
    for secao in [
        "Identificação",
        "Participantes e papéis",
        "Objetivo",
        "Resumo",
        "Decisões",
        "Necessidades e requisitos levantados",
        "Regras de negócio citadas",
        "Pendências",
        "Dúvidas em aberto",
        "Próximos passos",
        "Referências às falas",
        "Evidências",
    ] {
        assert!(corpo.contains(&format!("\n## {secao}\n")), "falta {secao}");
    }
    assert!(corpo.contains("- **Duração:** não informada"));
    assert!(corpo.contains("_Nenhum participante registrado._"));
    assert!(corpo.contains("_Nenhuma fala citada._"));
}

#[test]
fn conferida_em_depois_do_descarte_do_transcript() {
    let mut estado = estado_exemplo();
    // 5 falas conferidas e 1 que não confere.
    assert_eq!(estado.marcar_conferidas_em(data(20)), 5);
    assert_eq!(estado.marcar_conferidas_em(data(21)), 0);
    let corpo = Templates::padrao()
        .montar_ata(&estado, caminho_da_ata())
        .unwrap()
        .documento
        .corpo;
    assert_eq!(corpo.matches("_(conferida em 2026-11-20)_").count(), 5);
    assert_eq!(corpo.matches("_(não confere)_").count(), 1);
}

fn base_com_demanda() -> (tempfile::TempDir, Base, Projeto, IdDemanda) {
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
    let demanda = projeto
        .criar_demanda(
            &NovaDemanda {
                titulo: "Parcelamento PJ".into(),
                ..Default::default()
            },
            data(10),
        )
        .unwrap()
        .id;
    (dir, base, projeto, demanda)
}

/// Partes do `.docx` como texto.
fn parte_do_docx(docx: &[u8], nome: &str) -> String {
    let mut zip = zip::ZipArchive::new(Cursor::new(docx)).unwrap();
    let mut texto = String::new();
    zip.by_name(nome)
        .unwrap()
        .read_to_string(&mut texto)
        .unwrap();
    texto
}

#[test]
fn grava_na_pasta_da_demanda_e_exporta_para_markdown_e_word() {
    let (_dir, base, projeto, demanda) = base_com_demanda();
    let (mut estado, arquivos) = EstadoAta::criar(&projeto, demanda, data(12)).unwrap();
    assert_eq!(estado.id.to_string(), "R-0001");
    assert_eq!(
        arquivos.estado.as_str(),
        "demandas/D-0001-parcelamento-pj/reunioes/R-0001.estado.json"
    );
    assert_eq!(
        arquivos.ata.as_str(),
        "demandas/D-0001-parcelamento-pj/reunioes/R-0001.md"
    );
    // O estado inicial já está gravado; a ata, só depois da primeira montagem.
    assert!(projeto.existe(&arquivos.estado));
    assert!(!projeto.existe(&arquivos.ata));

    registrar_itens(&mut estado);
    let (templates, avisos) = Templates::da_base(&base);
    assert!(avisos.is_empty());
    let avisos = templates.gravar_ata(&projeto, &estado).unwrap();
    assert!(avisos.is_empty(), "{avisos:?}");

    // O estado volta igual, e a ata gravada é a montada.
    let (lido, _) = EstadoAta::ler(&projeto, estado.id).unwrap();
    assert_eq!(lido, estado);
    let texto = projeto.ler_texto(&arquivos.ata).unwrap();
    assert_eq!(
        texto,
        templates
            .montar_ata(&estado, arquivos.ata.clone())
            .unwrap()
            .documento
            .para_texto()
            .unwrap()
    );
    let json = projeto.ler_texto(&arquivos.estado).unwrap();
    assert!(json.contains("\"id\": \"D-1\""));
    assert!(json.contains("\"status\": \"conferida\""));

    // Markdown: sem frontmatter e sem links para a base.
    let md = exportar_markdown(&texto, &OpcoesExportacao::default(), None);
    assert!(!md.starts_with("---"));
    assert!(!md.contains(".md)"), "{md}");
    assert!(md.contains("# R-0001 · Ata da reunião de 2026-11-12"));
    assert!(md.contains("- **Demanda:** D-0001\n"));
    assert!(md.contains("— _Maria Exemplo, 00:12:34_"));

    // Word: título, seções e a fala com falante e horário.
    let docx = exportar_word(&texto, &OpcoesExportacao::default(), None).unwrap();
    let doc = parte_do_docx(&docx, "word/document.xml");
    assert!(
        parte_do_docx(&docx, "docProps/core.xml")
            .contains("<dc:title>R-0001 · Ata da reunião de 2026-11-12</dc:title>")
    );
    for secao in ["Decisões", "Pendências", "Referências às falas"] {
        assert!(doc.contains(&format!(">{secao}</w:t>")), "falta {secao}");
    }
    assert!(doc.contains("Maria Exemplo, 00:12:34"));
    assert!(doc.contains("<w:footnoteReference "));
}

#[test]
fn editar_o_template_na_base_muda_a_montagem() {
    let (_dir, base, projeto, demanda) = base_com_demanda();
    let criados = Templates::copiar_padrao(&base).unwrap();
    let arquivo = arquivo_do_template(TipoDocumento::Ata);
    assert_eq!(arquivo.as_str(), "_templates/ata.md");
    assert!(criados.contains(&arquivo));

    base.gravar_texto(
        &arquivo,
        "# Reunião {{id}} ({{data}})\r\n\r\n## O que ficou decidido\r\n\r\n{{decisoes}}\r\n\r\n\
         ## Tarefas\r\n\r\n{{pendencias}}\r\n\r\n## Falas\r\n\r\n{{referencias_falas}}\r\n",
    )
    .unwrap();
    let (templates, avisos) = Templates::da_base(&base);
    assert!(avisos.is_empty(), "{avisos:?}");

    let (mut estado, arquivos) = EstadoAta::criar(&projeto, demanda, data(12)).unwrap();
    estado
        .adicionar_item(
            NovoItem::novo(TipoItem::Decisao, "Só PJ.").com_citacao(fala(
                "Maria Exemplo",
                "00:12:34",
                "só PJ",
            )),
        )
        .unwrap();
    let avisos = templates.gravar_ata(&projeto, &estado).unwrap();
    assert!(avisos.is_empty(), "{avisos:?}");
    let ata: Documento<FrontmatterAta> = projeto.ler(&arquivos.ata).unwrap();
    assert_eq!(ata.frontmatter.status, StatusAta::Rascunho);
    assert_eq!(
        ata.corpo,
        "\n# Reunião R-0001 (2026-11-12)\n\n## O que ficou decidido\n\n\
         - **D-1** Só PJ. — _Maria Exemplo, 00:12:34_\n\n## Tarefas\n\n_Nada registrado._\n\n\
         ## Falas\n\n- **D-1** · Maria Exemplo, 00:12:34: “só PJ” _(conferida)_\n"
    );

    // Sem o marcador de uma seção com conteúdo, ela vai para o fim, com aviso.
    base.gravar_texto(&arquivo, "# {{id}}\n\n{{decisoes}}\n")
        .unwrap();
    let (templates, _) = Templates::da_base(&base);
    let avisos = templates.gravar_ata(&projeto, &estado).unwrap();
    assert_eq!(avisos.len(), 1, "{avisos:?}");
    let ata = projeto.ler_texto(&arquivos.ata).unwrap();
    assert!(ata.contains("\n## Referências às falas\n\n- **D-1** · Maria Exemplo"));
}

#[test]
fn reunioes_numeradas_por_projeto_entre_demandas() {
    let (_dir, _base, projeto, d1) = base_com_demanda();
    let d2 = projeto
        .criar_demanda(
            &NovaDemanda {
                titulo: "Multa por atraso".into(),
                ..Default::default()
            },
            data(11),
        )
        .unwrap()
        .id;
    let (a, _) = EstadoAta::criar(&projeto, d1, data(12)).unwrap();
    let (b, arquivos_b) = EstadoAta::criar(&projeto, d2, data(12)).unwrap();
    let (c, _) = EstadoAta::criar(&projeto, d1, data(13)).unwrap();
    assert_eq!(
        [a.id, b.id, c.id].map(|i| i.to_string()),
        ["R-0001", "R-0002", "R-0003"]
    );
    assert_eq!(
        arquivos_b.ata.as_str(),
        "demandas/D-0002-multa-por-atraso/reunioes/R-0002.md"
    );
    let da_primeira: Vec<String> = projeto
        .reunioes_da_demanda(d1)
        .unwrap()
        .iter()
        .map(|r| r.id.to_string())
        .collect();
    assert_eq!(da_primeira, ["R-0001", "R-0003"]);

    // O estado não pode trocar de demanda nem apontar para item inexistente.
    let mut outro = b.clone();
    outro.demanda = d1;
    assert!(matches!(
        outro.gravar(&projeto),
        Err(ErroAta::DemandaDiferente { .. })
    ));
    assert!(matches!(
        EstadoAta::ler(&projeto, IdReuniao::novo(9)),
        Err(ErroAta::Base(cba_kb::Erro::ReuniaoInexistente { .. }))
    ));
}
