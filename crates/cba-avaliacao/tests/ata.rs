//! Comparação de atas de exemplo com a reunião do gabarito (fluxo `reuniao`, #137),
//! sem chamar o Claude: uma ata boa, que atende ao gabarito, e uma ruim.

use cba_avaliacao::ata::{AvaliacaoAta, Transcripts, avaliar_ata};
use cba_avaliacao::execucao::falas_da_reuniao;
use cba_avaliacao::gabarito::{Gabarito, Reuniao};
use cba_avaliacao::gabarito_padrao;
use cba_avaliacao::longa::{self, OpcoesLonga, TamanhoTranscript};
use cba_avaliacao::relatorio::{Execucao, Relatorio};
use cba_docs::{
    CitacaoFala, EstadoAta, NovoItem, Participante, PropostaDePapel, StatusCitacao, TipoItem,
};
use cba_kb::frontmatter::{Data, Horario};
use cba_kb::{IdDemanda, IdReuniao};
use cba_reunioes::Fala;

fn gabarito() -> Gabarito {
    Gabarito::carregar(&gabarito_padrao()).unwrap()
}

fn falas(g: &Gabarito, r: &Reuniao) -> Vec<Fala> {
    falas_da_reuniao(g, r).unwrap().falas
}

fn ata_vazia() -> EstadoAta {
    EstadoAta::nova(
        IdReuniao::novo(1),
        "faturamento-exemplo",
        IdDemanda::novo(1),
        Data::analisar("2026-11-12").unwrap(),
    )
}

/// Citação da fala que começa em `inicio` (M:SS, no transcript original),
/// deslocada de `bloco` blocos na reunião longa, com as primeiras palavras dela
/// (ou as primeiras palavras do texto original, na fala repetida).
fn citar(lido: &[Fala], original: &[Fala], inicio: &str) -> CitacaoFala {
    let segundos = u64::from(Horario::analisar(inicio).unwrap().segundos());
    let base = original
        .iter()
        .find(|f| f.inicio.unwrap().segundos() == segundos)
        .unwrap_or_else(|| panic!("nenhuma fala em {inicio}"));
    let fala = lido
        .iter()
        .rev()
        .find(|f| f.falante == base.falante && f.texto.ends_with(&base.texto))
        .unwrap();
    let trecho: String = base
        .texto
        .split_whitespace()
        .take(5)
        .collect::<Vec<_>>()
        .join(" ");
    CitacaoFala::nova(
        fala.falante.clone().unwrap(),
        Horario::de_segundos(fala.inicio.unwrap().segundos() as u32),
        trecho,
        StatusCitacao::Conferida,
    )
}

fn proposta(nome: &str, papel: &str, citacao: CitacaoFala) -> Participante {
    let mut p = Participante::novo(nome);
    p.proposta = Some(PropostaDePapel {
        papel: papel.into(),
        justificativa: Some("Conduz e decide.".into()),
        sugerido_pelo_catalogo: None,
        citacoes: vec![citacao],
    });
    p
}

/// Ata que atende ao gabarito: os papéis esperados, um item por item esperado (com
/// o texto da descrição do gabarito), a fala esperada citada e, nas pendências, o
/// responsável e o prazo. Em `lido` (a reunião longa), as citações apontam para a
/// última repetição de cada fala.
fn ata_boa(r: &Reuniao, original: &[Fala], lido: &[Fala]) -> EstadoAta {
    let mut ata = ata_vazia();
    ata.definir_objetivo("Alinhar a trava de cancelamento de fatura antiga.");
    ata.definir_resumo("A trava vai para o banco; o relatório mensal fica para depois.");
    ata.definir_participantes(
        r.participantes
            .iter()
            .map(|p| {
                let primeira = original
                    .iter()
                    .find(|f| f.falante.as_deref() == Some(p.nome.as_str()))
                    .unwrap();
                let inicio = primeira.inicio.unwrap().to_string();
                // A grafia "Maria Silva" casa com "Silva, Maria" do transcript.
                let nome = if p.nome == "Silva, Maria" {
                    "Maria Silva"
                } else {
                    &p.nome
                };
                proposta(nome, &p.papel, citar(lido, original, &inicio))
            })
            .collect(),
    );
    for e in &r.itens {
        let mut novo = NovoItem::novo(e.tipo, e.descricao.clone());
        novo.citacoes = vec![citar(lido, original, &e.falas[0])];
        if e.tipo == TipoItem::Pendencia {
            let (responsavel, prazo) = match e.id.as_str() {
                "REU-FAT-001-P1" => ("Carlos Lima", "sexta-feira, dia 20"),
                "REU-FAT-001-P2" => ("Ana Souza", "quarta, dia 18"),
                "REU-FAT-001-P3" => ("Maria Silva", "até o dia 25"),
                outro => panic!("pendência sem dados de teste: {outro}"),
            };
            novo.responsavel = Some(responsavel.into());
            novo.prazo = Some(prazo.into());
        }
        ata.adicionar_item(novo).unwrap();
    }
    ata
}

fn avaliar_original(ata: &EstadoAta) -> AvaliacaoAta {
    let g = gabarito();
    let r = g.reuniao("REU-FAT-001").unwrap();
    let f = falas(&g, r);
    avaliar_ata(
        ata,
        r,
        Some(Transcripts {
            original: &f,
            lido: &f,
        }),
    )
}

#[test]
fn ata_boa_atende_a_tudo() {
    let g = gabarito();
    let r = g.reuniao("REU-FAT-001").unwrap();
    let f = falas(&g, r);
    let a = avaliar_original(&ata_boa(r, &f, &f));

    assert_eq!((a.papeis_acertados, a.papeis_esperados), (4, 4));
    assert!(a.outros_participantes.is_empty());
    assert_eq!(a.itens_esperados, 16);
    assert_eq!(a.itens_atendidos, 16, "{:#?}", a.itens);
    for i in &a.itens {
        assert_eq!(i.cita_fala_esperada, Some(true), "{}", i.id);
        if i.tipo == TipoItem::Pendencia {
            assert_eq!(i.responsavel_confere, Some(true), "{}", i.id);
            assert_eq!(i.prazo_confere, Some(true), "{}", i.id);
        } else {
            assert_eq!(i.responsavel_confere, None);
        }
    }
    assert_eq!(a.citacoes.total, 16 + 4);
    assert_eq!(a.citacoes.conferidas, 20);
    assert_eq!(a.citacoes.reconferidas, Some(20));
    assert!(a.citacoes.recusadas.is_empty());
    assert!(a.secoes_vazias.is_empty());
    assert!(a.objetivo && a.resumo);
    assert!(a.falhas().is_empty(), "{:?}", a.falhas());
    assert_eq!(a.cobertura_de_itens(), 1.0);
    let secoes: Vec<(TipoItem, usize, usize)> = a
        .por_secao
        .iter()
        .map(|s| (s.secao, s.atendidos, s.esperados))
        .collect();
    assert_eq!(
        secoes,
        [
            (TipoItem::Decisao, 3, 3),
            (TipoItem::Necessidade, 3, 3),
            (TipoItem::Regra, 2, 2),
            (TipoItem::Pendencia, 3, 3),
            (TipoItem::Duvida, 3, 3),
            (TipoItem::ProximoPasso, 2, 2),
        ]
    );
}

#[test]
fn ata_ruim_mostra_o_que_falta() {
    let g = gabarito();
    let r = g.reuniao("REU-FAT-001").unwrap();
    let f = falas(&g, r);
    let mut ata = ata_vazia();
    ata.definir_participantes(vec![
        proposta("Silva, Maria", "usuário-chave", citar(&f, &f, "0:03")),
        proposta("João Pereira", "Usuario-Chave", citar(&f, &f, "0:30")),
        // Carlos sem proposta; Ana só listada.
        Participante::novo("Ana Souza"),
    ]);
    // A decisão certa, mas registrada como necessidade: não atende à decisão.
    let mut d = NovoItem::novo(
        TipoItem::Necessidade,
        "Levar a trava do supervisor para a procedure do banco.",
    );
    d.citacoes = vec![citar(&f, &f, "2:56")];
    ata.adicionar_item(d).unwrap();
    // A pendência do Carlos com o responsável errado e sem prazo.
    let mut p = NovoItem::novo(
        TipoItem::Pendencia,
        "Levantar os pontos que cancelam fatura.",
    );
    p.responsavel = Some("Ana Souza".into());
    p.citacoes = vec![citar(&f, &f, "4:06")];
    ata.adicionar_item(p).unwrap();
    // Uma dúvida sem citação.
    ata.adicionar_item(NovoItem::novo(
        TipoItem::Duvida,
        "Os 30 dias contam da emissão ou do vencimento?",
    ))
    .unwrap();
    // Uma regra com o trecho de outra fala (não confere ao conferir de novo) e outra
    // gravada como `não confere`.
    let mut errada = citar(&f, &f, "1:11");
    errada.trecho = "Minha sugestão é colocar".into();
    let mut nao_confere = citar(&f, &f, "1:27");
    nao_confere.status = StatusCitacao::NaoConfere;
    let mut regra = NovoItem::novo(
        TipoItem::Regra,
        "Fatura com pagamento parcial não pode ser cancelada.",
    );
    regra.citacoes = vec![errada, nao_confere];
    ata.adicionar_item(regra).unwrap();

    let a = avaliar_original(&ata);
    assert_eq!((a.papeis_acertados, a.papeis_esperados), (1, 4));
    let maria = a.papeis.iter().find(|p| p.nome == "Silva, Maria").unwrap();
    assert_eq!(maria.proposto.as_deref(), Some("usuário-chave"));
    assert!(!maria.acertou);
    let carlos = a
        .papeis
        .iter()
        .find(|p| p.nome == "carlos.lima@exemplo.com")
        .unwrap();
    assert_eq!(carlos.proposto, None);

    let item = |id: &str| a.itens.iter().find(|i| i.id == id).unwrap();
    assert_eq!(item("REU-FAT-001-D1").atendido_por, None);
    // Como necessidade, o texto atende à N1 (a trava fora da tela Delphi).
    assert_eq!(item("REU-FAT-001-N1").atendido_por.as_deref(), Some("N-1"));
    let p1 = item("REU-FAT-001-P1");
    assert_eq!(p1.atendido_por.as_deref(), Some("P-1"));
    assert_eq!(p1.responsavel_confere, Some(false));
    assert_eq!(p1.prazo_confere, Some(false));
    assert_eq!(p1.cita_fala_esperada, Some(true));
    let dv1 = item("REU-FAT-001-DV1");
    assert_eq!(dv1.atendido_por.as_deref(), Some("DV-1"));
    assert_eq!(dv1.cita_fala_esperada, Some(false));
    let r1 = item("REU-FAT-001-R1");
    assert_eq!(r1.atendido_por.as_deref(), Some("R-1"));
    assert_eq!(r1.cita_fala_esperada, Some(true));
    assert_eq!(a.itens_atendidos, 4);

    assert_eq!(a.itens_sem_citacao, ["DV-1"]);
    assert_eq!(a.citacoes.total, 2 + 1 + 1 + 2);
    assert_eq!(a.citacoes.nao_conferem, 1);
    assert_eq!(a.citacoes.reconferidas, Some(5));
    assert_eq!(a.citacoes.recusadas.len(), 1);
    assert_eq!(a.citacoes.recusadas[0].onde, "R-1");
    assert!(
        a.citacoes.recusadas[0].motivo.contains("não aparece"),
        "{}",
        a.citacoes.recusadas[0].motivo
    );
    assert_eq!(a.secoes_vazias, ["decisoes", "proximos_passos"]);
    assert_eq!(
        a.secoes_vazias_com_itens_esperados,
        ["decisoes", "proximos_passos"]
    );
    assert!(!a.objetivo && !a.resumo);
    let falhas = a.falhas();
    assert_eq!(falhas.len(), 3, "{falhas:?}");

    // Sem nenhum item, a ata é falha grave.
    let vazia = avaliar_original(&ata_vazia());
    assert!(vazia.falhas()[0].contains("nenhum item"));
    assert_eq!(vazia.secoes_vazias.len(), 6);
}

#[test]
fn sem_transcript_mede_o_resto() {
    let g = gabarito();
    let r = g.reuniao("REU-FAT-001").unwrap();
    let f = falas(&g, r);
    let a = avaliar_ata(&ata_boa(r, &f, &f), r, None);
    assert_eq!(a.itens_atendidos, 16);
    assert_eq!(a.citacoes.reconferidas, None);
    assert!(a.itens.iter().all(|i| i.cita_fala_esperada.is_none()));
}

#[test]
fn reuniao_longa_tem_o_tamanho_pedido_e_a_ata_boa_confere() {
    let g = gabarito();
    let r = g.reuniao("REU-FAT-001").unwrap();
    let original = falas(&g, r);
    let lida = longa::gerar(&original, OpcoesLonga::default());
    let t = TamanhoTranscript::de(&lida);
    assert!((7_200..7_800).contains(&t.duracao_s), "{t:?}");
    assert!((25_000..27_000).contains(&t.palavras), "{t:?}");
    assert_eq!(t.falas % original.len(), 0);
    // Cada fala original continua inteira na repetição.
    assert!(
        lida[lida.len() - 1]
            .texto
            .ends_with(&original[original.len() - 1].texto)
    );

    // A ata boa, citando a última repetição de cada fala.
    let a = avaliar_ata(
        &ata_boa(r, &original, &lida),
        r,
        Some(Transcripts {
            original: &original,
            lido: &lida,
        }),
    );
    assert_eq!(a.itens_atendidos, 16);
    assert!(a.itens.iter().all(|i| i.cita_fala_esperada == Some(true)));
    assert!(
        a.citacoes.recusadas.is_empty(),
        "{:?}",
        a.citacoes.recusadas
    );
    // Contra a reunião original, as citações deslocadas não conferem.
    let contra_original = avaliar_original(&ata_boa(r, &original, &lida));
    assert_eq!(contra_original.citacoes.recusadas.len(), 20);
}

#[test]
fn segunda_reuniao_acha_os_participantes_pela_outra_grafia() {
    let g = gabarito();
    let r = g.reuniao("REU-FAT-002").unwrap();
    let f = falas(&g, r);
    let mut ata = ata_vazia();
    ata.definir_participantes(vec![
        proposta("Silva, Maria", "PO", citar(&f, &f, "0:02")),
        proposta("Beatriz Rocha", "qa", citar(&f, &f, "0:25")),
    ]);
    let a = avaliar_ata(
        &ata,
        r,
        Some(Transcripts {
            original: &f,
            lido: &f,
        }),
    );
    // "PO" é aceito para a Maria; QA casa sem maiúsculas.
    assert_eq!(a.papeis_acertados, 2);
    assert!(a.outros_participantes.is_empty());
}

#[test]
fn relatorio_da_reuniao_sem_texto_de_fala() {
    let g = gabarito();
    let r = g.reuniao("REU-FAT-001").unwrap();
    let f = falas(&g, r);
    let a = avaliar_original(&ata_boa(r, &f, &f));
    let e = Execucao {
        fluxo: "reuniao".into(),
        item: r.id.clone(),
        repeticao: 1,
        avaliacao: None,
        ata: Some(a),
        transcript: Some(TamanhoTranscript::de(&f)),
        arquivos_alterados: vec![],
        claude: None,
        erro: None,
    };
    let sem_ata = Execucao {
        ata: None,
        repeticao: 2,
        ..e.clone()
    };
    let rel = Relatorio::novo(None, vec![e, sem_ata]);
    assert!(!rel.sem_falhas_graves);
    let resumo = rel.resumo[0].ata.as_ref().unwrap();
    assert_eq!(resumo.itens_media, 0.5);
    assert_eq!(resumo.itens_min, 0.0);
    assert_eq!(resumo.papeis_esperados, 4);
    assert_eq!(rel.execucoes[1].falhas(), ["sem estado da ata"]);
    let md = rel.markdown();
    assert!(md.contains("## Reuniões"), "{md}");
    assert!(!md.contains("## Cobertura por fluxo"));
    assert!(md.contains("| reuniao | REU-FAT-001 | 2 | 50% de 4 | 50% de 16 (0%–100%)"));
    let json = serde_json::to_string(&rel).unwrap();
    // Nenhuma fala do transcript nem trecho citado vai para o relatório.
    for fala in &f {
        assert!(!md.contains(&fala.texto), "{}", fala.texto);
        assert!(!json.contains(&fala.texto), "{}", fala.texto);
    }
    let trecho = citar(&f, &f, "4:06").trecho;
    assert!(!md.contains(&trecho) && !json.contains(&trecho));
}
