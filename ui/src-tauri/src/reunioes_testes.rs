//! Testes da nova reunião, da confirmação dos participantes e da ata (#141), com as
//! reuniões fictícias do "Faturamento Exemplo" (`fixtures/reunioes`).

use std::path::{Path, PathBuf};

use super::*;
use crate::analise::carregar_analise;
use crate::demandas::{EventoDemanda, painel_demanda};
use crate::teste_base::{PROJETO, cenario};
use cba_docs::NovoItem;
use cba_kb::frontmatter::FrontmatterAnalise;

fn fixture(nome: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/reunioes")
        .join(nome)
}

fn vtt() -> EntradaTranscript {
    EntradaTranscript::Arquivo {
        caminho: fixture("reuniao-01-cancelamento.vtt"),
    }
}

fn texto_colado() -> EntradaTranscript {
    EntradaTranscript::Texto {
        texto: std::fs::read_to_string(fixture("reuniao-01-cancelamento.txt")).unwrap(),
    }
}

/// Reunião curta em que a mesma pessoa aparece com dois nomes.
fn com_dois_nomes() -> EntradaTranscript {
    EntradaTranscript::Texto {
        texto: "WEBVTT\n\n\
                00:00:01.000 --> 00:00:05.000\n\
                <v Maria Silva>Bom dia, vamos falar do cancelamento de fatura.</v>\n\n\
                00:00:06.000 --> 00:00:09.000\n\
                <v Joao Pereira>A trava dos 30 dias fica na tela de retaguarda.</v>\n\n\
                00:00:10.000 --> 00:00:14.000\n\
                <v Maria S>Voltei, a conexão caiu. Seguimos com o cancelamento.</v>\n"
            .into(),
    }
}

fn nova_demanda() -> DemandaDaReuniao {
    DemandaDaReuniao::Nova {
        titulo: "Cancelamento de fatura antiga".into(),
        chave: Some("FAT-31".into()),
    }
}

#[test]
fn previa_do_texto_colado_e_do_vtt() {
    let p = previa_transcript(&texto_colado()).unwrap();
    assert_eq!(p.formato, Formato::TextoTeams);
    assert!(p.pode_criar);
    let nomes: Vec<&str> = p.falantes.iter().map(|f| f.nome.as_str()).collect();
    assert!(nomes.contains(&"João Pereira"), "{nomes:?}");
    assert_eq!(
        p.falantes.iter().map(|f| f.falas).sum::<usize>() + p.sem_falante,
        p.falas
    );

    let v = previa_transcript(&vtt()).unwrap();
    assert_eq!(v.formato, Formato::Vtt);
    assert_eq!(v.falantes[0].nome, "Silva, Maria");

    // A prévia não traz o texto das falas.
    let json = serde_json::to_string(&v).unwrap();
    assert!(!json.contains("auditoria interna"), "{json}");
    assert_eq!(serde_json::to_value(&v).unwrap()["formato"], "vtt");

    let vazia = previa_transcript(&EntradaTranscript::Texto { texto: "  ".into() }).unwrap();
    assert!(!vazia.pode_criar);
    assert_eq!(vazia.avisos[0].codigo, "entrada_vazia");

    assert!(
        previa_transcript(&EntradaTranscript::Arquivo {
            caminho: fixture("nao-existe.vtt")
        })
        .is_err()
    );
}

/// Critério de aceite da #141: sem aceitar o aviso, a reunião não é criada.
#[test]
fn sem_aceitar_o_aviso_a_reuniao_nao_e_criada() {
    let c = cenario();
    let erro = criar_reuniao(&c.dados, PROJETO, &nova_demanda(), &vtt(), false).unwrap_err();
    assert!(erro.contains("aviso de privacidade"), "{erro}");
    let amb = c.ambiente();
    assert!(amb.kb.reunioes().unwrap().is_empty());
    assert!(amb.kb.demandas().unwrap().is_empty());
    assert!(amb.analises.listar().unwrap().is_empty());
    assert!(Guarda::nova(&c.dados).listar().unwrap().is_empty());
}

#[test]
fn cria_a_reuniao_a_guarda_e_a_analise_ligada() {
    let c = cenario();
    let criada = criar_reuniao(&c.dados, PROJETO, &nova_demanda(), &vtt(), true).unwrap();
    assert_eq!(criada.reuniao, "R-0001");
    assert_eq!(criada.demanda, "D-0001");
    let r = IdReuniao::novo(1);

    // A demanda nova, com a chave.
    let amb = c.ambiente();
    let d = amb
        .kb
        .demanda(IdDemanda::analisar("D-0001").unwrap())
        .unwrap();
    assert_eq!(d.documento.frontmatter.chave.as_deref(), Some("FAT-31"));

    // O estado inicial da ata, com os falantes como participantes, e a ata montada.
    let (estado, arquivos) = EstadoAta::ler(&amb.kb, r).unwrap();
    assert_eq!(estado.status, StatusAta::Rascunho);
    let nomes: Vec<&str> = estado
        .participantes
        .iter()
        .map(|p| p.nome.as_str())
        .collect();
    assert_eq!(nomes[0], "Silva, Maria");
    assert!(nomes.contains(&"João Pereira"));
    assert!(estado.participantes.iter().all(|p| p.papel.is_none()));
    assert!(amb.kb.existe(&arquivos.ata));

    // O transcript fica só no diretório de dados.
    let guarda = Guarda::nova(&c.dados);
    assert!(guarda.existe(PROJETO, r));
    assert!(
        guarda
            .pasta(PROJETO, r)
            .unwrap()
            .starts_with(c.dados.raiz())
    );

    // A análise do fluxo reuniao, ligada à demanda e à reunião.
    let vista = carregar_analise(&c.dados, PROJETO, &criada.analise.analise.id).unwrap();
    assert_eq!(vista.analise.fluxo, Fluxo::Reuniao);
    assert_eq!(vista.analise.reuniao, Some(r));
    let doc = amb
        .kb
        .ler::<FrontmatterAnalise>(&vista.analise.documento)
        .unwrap();
    assert_eq!(doc.frontmatter.demanda.id, Some(d.id));
    assert_eq!(doc.frontmatter.demanda.chave.as_deref(), Some("FAT-31"));
    let prompt = &criada.analise.prompt_inicial;
    assert!(prompt.contains(&format!("analise {}", criada.analise.analise.id)));
    assert!(prompt.contains("reuniao R-0001"));
    for proibido in ['"', '%', '!', '(', ')', '\n'] {
        assert!(!prompt.contains(proibido), "{prompt}");
    }

    // Formato que a tela espera (ui/src/reunioes.ts).
    let json = serde_json::to_value(&criada).unwrap();
    assert_eq!(json["reuniao"], "R-0001");
    assert!(json["promptInicial"].is_string());
    assert_eq!(json["analise"]["fluxo"], "reuniao");

    // Uma segunda reunião na mesma demanda, pelo texto colado.
    let outra = criar_reuniao(
        &c.dados,
        PROJETO,
        &DemandaDaReuniao::Existente {
            id: "D-0001".into(),
        },
        &texto_colado(),
        true,
    )
    .unwrap();
    assert_eq!(outra.reuniao, "R-0002");
    assert_eq!(outra.demanda, "D-0001");
}

#[test]
fn recusa_demanda_encerrada_e_transcript_sem_falas() {
    let c = cenario();
    let amb = c.ambiente();
    let d = amb
        .kb
        .criar_demanda(
            &NovaDemanda {
                titulo: "Multa".into(),
                descricao: None,
                chave: None,
                origem: None,
            },
            tempo::hoje(),
        )
        .unwrap();
    amb.kb.encerrar_demanda(d.id, tempo::hoje()).unwrap();
    let existente = DemandaDaReuniao::Existente {
        id: d.id.to_string(),
    };
    let erro = criar_reuniao(&c.dados, PROJETO, &existente, &vtt(), true).unwrap_err();
    assert!(erro.contains("encerrada"), "{erro}");

    let vazio = EntradaTranscript::Texto { texto: " ".into() };
    let erro = criar_reuniao(&c.dados, PROJETO, &nova_demanda(), &vazio, true).unwrap_err();
    assert!(erro.contains("não tem falas"), "{erro}");
    assert!(amb.kb.reunioes().unwrap().is_empty());
}

#[test]
fn o_fluxo_reuniao_nao_nasce_pelos_outros_caminhos() {
    let c = cenario();
    let erro = crate::analise::criar_analise_do_fluxo(
        &c.dados,
        PROJETO,
        Fluxo::Reuniao,
        "Reunião de ontem",
        None,
    )
    .unwrap_err();
    assert!(erro.contains("nova reunião"), "{erro}");
}

#[test]
fn ata_com_participantes_sugestoes_e_itens() {
    let c = cenario();
    let amb = c.ambiente();
    // A Maria já está no catálogo, com o papel confirmado numa reunião anterior.
    let maria = amb
        .kb
        .registrar_pessoa(&NovaPessoa {
            nome: "Maria Silva".into(),
            apelidos: vec![],
            papel: None,
        })
        .unwrap();
    amb.kb
        .registrar_papel(&maria.slug, "D-0009/R-0009", "PO", tempo::hoje())
        .unwrap();
    let criada = criar_reuniao(&c.dados, PROJETO, &nova_demanda(), &vtt(), true).unwrap();
    let r = IdReuniao::novo(1);

    // O agente propõe o papel do João e registra uma decisão.
    let templates = Templates::da_base(&amb.base).0;
    templates
        .alterar_ata(&amb.kb, r, |e: &mut EstadoAta| {
            let joao = e
                .participantes
                .iter_mut()
                .find(|p| p.nome == "João Pereira")
                .unwrap();
            joao.proposta = Some(cba_docs::PropostaDePapel {
                papel: "Desenvolvedor".into(),
                justificativa: Some("Explica onde fica a trava no código.".into()),
                sugerido_pelo_catalogo: None,
                citacoes: vec![],
            });
            e.adicionar_item(NovoItem::novo(
                TipoItem::Decisao,
                "A trava dos 30 dias vale também no banco",
            ))?;
            Ok::<_, ErroAta>(())
        })
        .unwrap();

    let ata = ata_da_reuniao(&c.dados, PROJETO, &criada.reuniao).unwrap();
    assert_eq!(ata.id, "R-0001");
    assert_eq!(ata.demanda, "D-0001");
    let m = &ata.participantes[0];
    assert_eq!(m.nome, "Silva, Maria");
    assert_eq!(m.pessoa.as_deref(), Some(maria.slug.as_str()));
    let sugestao = m.catalogo.as_ref().unwrap();
    assert_eq!(sugestao.papel.as_deref(), Some("PO"));
    assert_eq!(sugestao.reuniao.as_deref(), Some("D-0009/R-0009"));
    let j = ata
        .participantes
        .iter()
        .find(|p| p.nome == "João Pereira")
        .unwrap();
    assert!(j.catalogo.is_none());
    assert_eq!(j.proposta.as_ref().unwrap().papel, "Desenvolvedor");
    assert_eq!(ata.itens.len(), 1);
    assert_eq!(ata.itens[0].id, "D-1");
    assert!(ata.papeis.iter().any(|p| p.nome == "PO"));
    assert!(ata.documento.is_some());
    assert!(ata.transcript.is_some());

    let json = serde_json::to_value(&ata).unwrap();
    assert_eq!(json["itens"][0]["tipo"], "decisao");
    assert_eq!(json["status"], "rascunho");
    assert!(json["participantes"][0]["catalogo"]["pessoa"]["slug"].is_string());
}

#[test]
fn confirmar_registra_pessoas_e_papeis_no_catalogo_e_na_ata() {
    let c = cenario();
    criar_reuniao(&c.dados, PROJETO, &nova_demanda(), &vtt(), true).unwrap();
    let ata = confirmar_participantes(
        &c.dados,
        PROJETO,
        "R-0001",
        &[
            ConfirmacaoParticipante {
                nome: "Silva, Maria".into(),
                papel: Some("po".into()),
                ..Default::default()
            },
            ConfirmacaoParticipante {
                nome: "João Pereira".into(),
                papel: Some("Arquiteto do legado".into()),
                ..Default::default()
            },
        ],
    )
    .unwrap();

    let amb = c.ambiente();
    let maria = amb.kb.pessoa("maria-silva").unwrap();
    // O papel padrão vem com a grafia do catálogo de papéis.
    assert_eq!(maria.documento.frontmatter.papel.as_deref(), Some("PO"));
    let h = &maria.documento.frontmatter.historico;
    assert_eq!(h.len(), 1);
    assert_eq!(h[0].reuniao, "D-0001/R-0001");
    let joao = amb.kb.pessoa("joao-pereira").unwrap();
    assert_eq!(
        joao.documento.frontmatter.papel.as_deref(),
        Some("Arquiteto do legado")
    );

    let m = &ata.participantes[0];
    assert_eq!(m.papel.as_deref(), Some("PO"));
    assert_eq!(m.pessoa.as_deref(), Some("maria-silva"));
    let (estado, _) = EstadoAta::ler(&amb.kb, IdReuniao::novo(1)).unwrap();
    assert_eq!(estado.participantes[0].papel.as_deref(), Some("PO"));
    // Os não confirmados ficam como estavam.
    assert!(
        estado
            .participantes
            .iter()
            .any(|p| p.nome == "Ana Souza" && p.papel.is_none())
    );

    // Confirmar de novo na mesma reunião troca o papel, sem outro histórico.
    confirmar_participantes(
        &c.dados,
        PROJETO,
        "R-0001",
        &[ConfirmacaoParticipante {
            nome: "Silva, Maria".into(),
            papel: Some("Gerente".into()),
            ..Default::default()
        }],
    )
    .unwrap();
    let maria = amb.kb.pessoa("maria-silva").unwrap();
    assert_eq!(maria.documento.frontmatter.historico.len(), 1);
    assert_eq!(
        maria.documento.frontmatter.papel.as_deref(),
        Some("Gerente")
    );
}

/// Critério de aceite da #141: juntar dois nomes na tela atualiza o catálogo.
#[test]
fn juntar_dois_nomes_atualiza_o_catalogo() {
    let c = cenario();
    let amb = c.ambiente();
    // Nas reuniões anteriores, os dois nomes viraram duas pessoas.
    for nome in ["Maria Silva", "Maria S"] {
        amb.kb
            .registrar_pessoa(&NovaPessoa {
                nome: nome.into(),
                apelidos: vec![],
                papel: None,
            })
            .unwrap();
    }
    assert_eq!(amb.kb.pessoas().unwrap().len(), 2);
    criar_reuniao(&c.dados, PROJETO, &nova_demanda(), &com_dois_nomes(), true).unwrap();

    let ata = confirmar_participantes(
        &c.dados,
        PROJETO,
        "R-0001",
        &[ConfirmacaoParticipante {
            nome: "Maria Silva".into(),
            papel: Some("PO".into()),
            mesmos: vec!["Maria S".into()],
            ..Default::default()
        }],
    )
    .unwrap();

    let pessoas = amb.kb.pessoas().unwrap();
    assert_eq!(pessoas.len(), 1, "{pessoas:?}");
    let maria = &pessoas[0];
    assert_eq!(maria.slug, "maria-silva");
    assert!(maria.casa_com("Maria S"));
    assert_eq!(maria.documento.frontmatter.papel.as_deref(), Some("PO"));
    // Na ata, os dois nomes viram um participante.
    let nomes: Vec<&str> = ata.participantes.iter().map(|p| p.nome.as_str()).collect();
    assert_eq!(nomes, ["Maria Silva", "Joao Pereira"]);
}

#[test]
fn juntar_nomes_sem_catalogo_registra_uma_pessoa_com_apelido() {
    let c = cenario();
    criar_reuniao(&c.dados, PROJETO, &nova_demanda(), &com_dois_nomes(), true).unwrap();
    confirmar_participantes(
        &c.dados,
        PROJETO,
        "R-0001",
        &[ConfirmacaoParticipante {
            nome: "Maria Silva".into(),
            papel: None,
            mesmos: vec!["Maria S".into()],
            ..Default::default()
        }],
    )
    .unwrap();
    let pessoas = c.ambiente().kb.pessoas().unwrap();
    assert_eq!(pessoas.len(), 1);
    assert_eq!(pessoas[0].apelidos(), ["Maria S"]);
    // Sem papel, nada no histórico.
    assert!(pessoas[0].documento.frontmatter.historico.is_empty());

    // Um nome em dois lugares é recusado.
    let erro = confirmar_participantes(
        &c.dados,
        PROJETO,
        "R-0001",
        &[
            ConfirmacaoParticipante {
                nome: "Joao Pereira".into(),
                mesmos: vec!["Maria Silva".into()],
                ..Default::default()
            },
            ConfirmacaoParticipante {
                nome: "Maria Silva".into(),
                ..Default::default()
            },
        ],
    )
    .unwrap_err();
    assert!(erro.contains("mais de uma vez"), "{erro}");
}

#[test]
fn ata_aprovada_nao_aceita_confirmacao() {
    let c = cenario();
    criar_reuniao(&c.dados, PROJETO, &nova_demanda(), &vtt(), true).unwrap();
    let amb = c.ambiente();
    Templates::da_base(&amb.base)
        .0
        .alterar_ata(&amb.kb, IdReuniao::novo(1), |e: &mut EstadoAta| {
            e.status = StatusAta::Aprovada;
            Ok::<_, ErroAta>(())
        })
        .unwrap();
    let erro = confirmar_participantes(
        &c.dados,
        PROJETO,
        "R-0001",
        &[ConfirmacaoParticipante {
            nome: "Ana Souza".into(),
            papel: Some("Analista".into()),
            ..Default::default()
        }],
    )
    .unwrap_err();
    assert!(erro.contains("aprovada"), "{erro}");
    assert!(amb.kb.pessoas().unwrap().is_empty());
}

/// O painel da demanda avisa do transcript guardado enquanto a ata não é aprovada.
#[test]
fn painel_da_demanda_mostra_o_transcript_guardado() {
    let c = cenario();
    criar_reuniao(&c.dados, PROJETO, &nova_demanda(), &vtt(), true).unwrap();
    let reuniao = |c: &crate::teste_base::Cenario| {
        painel_demanda(&c.dados, PROJETO, "D-0001")
            .unwrap()
            .linha_do_tempo
            .into_iter()
            .find_map(|i| match i.evento {
                EventoDemanda::Reuniao(r) => Some(r),
                _ => None,
            })
            .unwrap()
    };
    let r = reuniao(&c);
    let guardado = r.transcript_guardado.expect("transcript guardado");
    let prazo = Config::default().prazo_retencao_transcript();
    assert_eq!(
        guardado.apaga_em,
        tempo::somar_dias(tempo::hoje(), i64::from(prazo)).to_string()
    );
    assert!(!guardado.vence_em_breve);

    // Aprovada a ata, o aviso some (a #142 apaga o transcript na aprovação).
    let amb = c.ambiente();
    Templates::da_base(&amb.base)
        .0
        .alterar_ata(&amb.kb, IdReuniao::novo(1), |e: &mut EstadoAta| {
            e.status = StatusAta::Aprovada;
            Ok::<_, ErroAta>(())
        })
        .unwrap();
    assert!(reuniao(&c).transcript_guardado.is_none());
}

#[test]
fn transcript_que_vence_em_breve() {
    let c = cenario();
    criar_reuniao(&c.dados, PROJETO, &nova_demanda(), &vtt(), true).unwrap();
    let guarda = Guarda::nova(&c.dados);
    let r = IdReuniao::novo(1);
    let hoje = tempo::hoje();
    let longe = transcript_guardado(&guarda, PROJETO, r, StatusAta::Rascunho, hoje, 30).unwrap();
    assert!(!longe.vence_em_breve);
    let perto = transcript_guardado(&guarda, PROJETO, r, StatusAta::Rascunho, hoje, 5).unwrap();
    assert!(perto.vence_em_breve);
    assert!(transcript_guardado(&guarda, PROJETO, r, StatusAta::Aprovada, hoje, 5).is_none());
    assert!(
        transcript_guardado(
            &guarda,
            PROJETO,
            IdReuniao::novo(9),
            StatusAta::Rascunho,
            hoje,
            5
        )
        .is_none()
    );
}
