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

// ---------------------------------------------------------------------------
// Aprovação da ata e próximos passos (#142)

/// Reunião criada pela tela, com itens registrados pelo agente (necessidade,
/// pendência, dúvida e o próximo passo que aponta para a necessidade) e concluída.
/// Devolve o id da análise da reunião.
fn reuniao_concluida(c: &crate::teste_base::Cenario) -> String {
    let criada = criar_reuniao(&c.dados, PROJETO, &nova_demanda(), &vtt(), true).unwrap();
    let id = criada.analise.analise.id;
    c.chamar("iniciar_analise", serde_json::json!({ "analise": id }));
    let necessidade = serde_json::json!([{
        "indice": 8,
        "trecho": "a checagem do supervisor precisa valer também fora da tela Delphi"
    }]);
    for (tipo, texto, referencias) in [
        (
            "necessidade",
            "Trava do supervisor em todos os canais.",
            vec![],
        ),
        ("pendencia", "Levantar os canais de cancelamento.", vec![]),
        ("duvida", "O portal do cliente também cancela?", vec![]),
        (
            "proximo_passo",
            "Analisar o impacto da trava nos outros canais.",
            vec!["N-1"],
        ),
    ] {
        c.chamar(
            "registrar_item_ata",
            serde_json::json!({
                "reuniao": "R-0001", "tipo": tipo, "texto": texto,
                "referencias": referencias, "citacoes": necessidade
            }),
        );
    }
    c.chamar("concluir_analise", serde_json::json!({ "analise": id }));
    id
}

/// Critério de aceite da #142: sem confirmar o aviso, a ata não é aprovada e o
/// transcript continua.
#[test]
fn sem_confirmar_o_segundo_aviso_a_ata_continua_em_rascunho() {
    let c = cenario();
    let analise = reuniao_concluida(&c);
    let p = prontidao_ata(&c.dados, PROJETO, &analise).unwrap();
    assert!(p.pode_aprovar, "{p:?}");
    assert!(p.transcript_guardado);
    assert!(p.aviso_descarte.contains("deixam de ser conferíveis"));
    // Os participantes vieram do transcript, sem papel confirmado.
    assert!(
        p.avisos.iter().any(|a| a.contains("sem papel confirmado")),
        "{:?}",
        p.avisos
    );
    let json = serde_json::to_value(&p).unwrap();
    assert_eq!(json["podeAprovar"], true);
    assert_eq!(json["status"], "rascunho");

    let erro = aprovar_ata(&c.dados, PROJETO, &analise, false).unwrap_err();
    assert!(erro.contains("confirme o aviso"), "{erro}");
    let ata = ata_da_reuniao(&c.dados, PROJETO, "R-0001").unwrap();
    assert_eq!(ata.status, StatusAta::Rascunho);
    assert!(ata.transcript.is_some());
    assert!(Guarda::nova(&c.dados).existe(PROJETO, IdReuniao::novo(1)));
    // O painel ainda não tem os itens (a ata não foi aprovada).
    let painel = painel_demanda(&c.dados, PROJETO, "D-0001").unwrap();
    assert!(painel.pendencias.is_empty());
    assert!(painel.proximos_passos.is_empty());
}

/// A aprovação apaga o transcript e leva as pendências, as dúvidas e os próximos
/// passos ao painel da demanda.
#[test]
fn aprovar_a_ata_apaga_o_transcript_e_alimenta_o_painel_da_demanda() {
    let c = cenario();
    let analise = reuniao_concluida(&c);
    let feita = aprovar_ata(&c.dados, PROJETO, &analise, true).unwrap();
    assert!(feita.transcript_apagado);
    assert_eq!(feita.citacoes_marcadas, 4);
    assert_eq!(feita.data, tempo::hoje());

    let ata = ata_da_reuniao(&c.dados, PROJETO, "R-0001").unwrap();
    assert_eq!(ata.status, StatusAta::Aprovada);
    assert!(ata.transcript.is_none());
    let rotulo = format!("conferida em {}", tempo::hoje());
    assert!(
        ata.itens
            .iter()
            .flat_map(|i| &i.citacoes)
            .all(|c| c.status == rotulo)
    );
    assert!(!Guarda::nova(&c.dados).existe(PROJETO, IdReuniao::novo(1)));
    let vista = carregar_analise(&c.dados, PROJETO, &analise).unwrap();
    assert_eq!(
        vista.analise.estado,
        cba_core::analise::EstadoAnalise::Aprovada
    );

    let painel = painel_demanda(&c.dados, PROJETO, "D-0001").unwrap();
    assert_eq!(painel.pendencias[0].id, "P-1");
    assert_eq!(painel.duvidas[0].id, "DV-1");
    let pp = &painel.proximos_passos[0];
    assert_eq!(pp.id, "PP-1");
    assert_eq!(pp.reuniao, "R-0001");
    assert_eq!(pp.referencias, vec!["N-1".to_owned()]);
    let r = painel
        .linha_do_tempo
        .iter()
        .find_map(|i| match &i.evento {
            EventoDemanda::Reuniao(r) => Some(r),
            _ => None,
        })
        .unwrap();
    assert!(r.transcript_guardado.is_none());
    assert!(
        prontidao_ata(&c.dados, PROJETO, &analise)
            .unwrap()
            .concluida
    );
}

/// Critério de aceite da #142: um próximo passo abre a análise de impacto com
/// `demanda.id` e a ata no contexto.
#[test]
fn proximo_passo_abre_analise_de_impacto_ligada_a_demanda_com_a_ata() {
    let c = cenario();
    let analise = reuniao_concluida(&c);
    let impacto = AcaoProximoPasso::Impacto {
        necessidade: Some("N-1".into()),
    };
    // Antes da aprovação, não.
    let erro = analisar_proximo_passo(&c.dados, PROJETO, "R-0001", "PP-1", &impacto).unwrap_err();
    assert!(erro.contains("aprove a ata"), "{erro}");
    aprovar_ata(&c.dados, PROJETO, &analise, true).unwrap();

    let criada = analisar_proximo_passo(&c.dados, PROJETO, "R-0001", "PP-1", &impacto).unwrap();
    assert_eq!(criada.analise.fluxo, Fluxo::Impacto);
    assert!(
        criada
            .analise
            .titulo
            .starts_with("Impacto da necessidade N-1 (R-0001)"),
        "{}",
        criada.analise.titulo
    );
    let id = criada.analise.id.clone();
    let vista = carregar_analise(&c.dados, PROJETO, &id).unwrap();
    let entrada = &vista.analise.entrada;
    let amb = c.ambiente();
    let arquivos = amb.kb.reuniao(IdReuniao::novo(1)).unwrap();
    let caminho_da_ata = amb.kb.na_base(&arquivos.ata).as_str().to_owned();
    for esperado in [
        "Analisar o impacto da necessidade N-1: Trava do supervisor em todos os canais.",
        "próximo passo PP-1 da ata aprovada da reunião R-0001",
        "demanda D-0001",
        caminho_da_ata.as_str(),
        "- PP-1 (próximo passo): Analisar o impacto da trava nos outros canais.",
        "- N-1 (necessidade): Trava do supervisor em todos os canais.",
    ] {
        assert!(
            entrada.contains(esperado),
            "falta `{esperado}` em:\n{entrada}"
        );
    }
    // Sem falas: o trecho citado fica só na ata.
    assert!(!entrada.contains("fora da tela Delphi"), "{entrada}");

    // Ligada à demanda pelo frontmatter.
    let a = amb.analises.carregar(&Id::analisar(&id).unwrap()).unwrap();
    let doc: cba_kb::Documento<FrontmatterAnalise> = amb.kb.ler(&a.documento).unwrap();
    assert_eq!(
        doc.frontmatter.demanda.id,
        Some(IdDemanda::analisar("D-0001").unwrap())
    );
    assert_eq!(doc.frontmatter.demanda.chave.as_deref(), Some("FAT-31"));

    // O iniciar_analise traz a demanda e a ata aprovada no contexto.
    let saida = cba_tools::registro_padrao()
        .chamar(
            "iniciar_analise",
            Ok(&amb),
            Some(serde_json::json!({ "analise": id })),
        )
        .unwrap();
    let dados = saida.dados.unwrap();
    let ctx = &dados["contexto"]["demanda"];
    assert_eq!(ctx["id"], "D-0001");
    assert_eq!(ctx["atas_anteriores"][0]["id"], "R-0001");
    assert_eq!(ctx["atas_anteriores"][0]["status"], "aprovada");
    assert_eq!(ctx["atas_anteriores"][0]["ata"], caminho_da_ata.as_str());
    assert_eq!(ctx["pendencias"][0]["id"], "P-1");

    // E aparece na linha do tempo da demanda.
    let painel = painel_demanda(&c.dados, PROJETO, "D-0001").unwrap();
    assert!(painel.linha_do_tempo.iter().any(|i| matches!(
        &i.evento,
        EventoDemanda::Analise(x) if x.id == id && x.fluxo == Fluxo::Impacto
    )));
}

#[test]
fn proximo_passo_com_pergunta_e_recusas() {
    let c = cenario();
    let analise = reuniao_concluida(&c);
    aprovar_ata(&c.dados, PROJETO, &analise, true).unwrap();

    let pergunta = AcaoProximoPasso::Perguntar {
        pergunta: "Quais canais cancelam fatura sem checar o perfil?".into(),
    };
    let criada = analisar_proximo_passo(&c.dados, PROJETO, "R-0001", "PP-1", &pergunta).unwrap();
    assert_eq!(criada.analise.fluxo, Fluxo::Perguntar);
    assert_eq!(
        criada.analise.titulo,
        "Quais canais cancelam fatura sem checar o perfil?"
    );
    let vista = carregar_analise(&c.dados, PROJETO, &criada.analise.id).unwrap();
    assert!(
        vista
            .analise
            .entrada
            .contains("Contexto: próximo passo PP-1")
    );

    // Sem necessidade, a análise de impacto é do próprio passo.
    let criada = analisar_proximo_passo(
        &c.dados,
        PROJETO,
        "R-0001",
        "PP-1",
        &AcaoProximoPasso::Impacto { necessidade: None },
    )
    .unwrap();
    assert!(
        criada.analise.titulo.starts_with("Impacto de PP-1"),
        "{}",
        criada.analise.titulo
    );

    for (item, acao, esperado) in [
        ("P-1", pergunta.clone(), "não tem o próximo passo P-1"),
        ("PP-9", pergunta.clone(), "não tem o próximo passo PP-9"),
        (
            "PP-1",
            AcaoProximoPasso::Impacto {
                necessidade: Some("P-1".into()),
            },
            "não tem a necessidade P-1",
        ),
        (
            "PP-1",
            AcaoProximoPasso::Perguntar {
                pergunta: "  ".into(),
            },
            "escreva a pergunta",
        ),
        ("xx", pergunta.clone(), "item da ata inválido"),
    ] {
        let erro = analisar_proximo_passo(&c.dados, PROJETO, "R-0001", item, &acao).unwrap_err();
        assert!(erro.contains(esperado), "{item}: {erro}");
    }

    // Demanda encerrada não recebe análise nova.
    crate::demandas::mudar_status_demanda(&c.dados, PROJETO, "D-0001", true).unwrap();
    let erro = analisar_proximo_passo(&c.dados, PROJETO, "R-0001", "PP-1", &pergunta).unwrap_err();
    assert!(erro.contains("encerrada"), "{erro}");
}

#[test]
fn acao_do_proximo_passo_vem_da_tela_em_camel_case() {
    let a: AcaoProximoPasso =
        serde_json::from_value(serde_json::json!({ "tipo": "impacto", "necessidade": "N-3" }))
            .unwrap();
    assert_eq!(
        a,
        AcaoProximoPasso::Impacto {
            necessidade: Some("N-3".into())
        }
    );
    let a: AcaoProximoPasso =
        serde_json::from_value(serde_json::json!({ "tipo": "impacto" })).unwrap();
    assert_eq!(a, AcaoProximoPasso::Impacto { necessidade: None });
    let a: AcaoProximoPasso =
        serde_json::from_value(serde_json::json!({ "tipo": "perguntar", "pergunta": "Por quê?" }))
            .unwrap();
    assert_eq!(
        a,
        AcaoProximoPasso::Perguntar {
            pergunta: "Por quê?".into()
        }
    );
}
