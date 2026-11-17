//! Pauta de perguntas pendentes (#123) com o sistema fictício "Faturamento
//! Exemplo": duas análises da demanda perguntam a mesma coisa e uma ata aprovada
//! tem uma dúvida; a pauta junta tudo por destinatário; a resposta fecha a
//! pergunta nas duas análises, vira regra informada (sem evidência de código) ou
//! valida uma premissa; a pauta é exportada em Markdown e Word.

use std::path::{Path, PathBuf};

use cba_core::ambiente::Ambiente;
use cba_core::analise::{Confianca, EstadoPergunta, Fluxo, NovaAfirmacao};
use cba_core::analises::NovaAnalise;
use cba_core::config::Config;
use cba_core::dados::DiretorioDados;
use cba_core::eventos::Mudanca;
use cba_docs::{EstadoAta, Formato, NovoItem, TipoItem};
use cba_kb::frontmatter::{Data, Demanda, FrontmatterRegra, StatusAta};
use cba_kb::informada::Destinatario;
use cba_kb::registros::StatusPremissa;
use cba_kb::{
    Categoria, DadosRegistro, Documento, Id, IdDemanda, IdReuniao, NovaDemanda, NovaPessoa,
    NovoRegistro,
};
use cba_tools::pauta::{
    DestinoDaResposta, ErroPauta, RespostaRecebida, SEM_DESTINATARIO, definir_destinatario, email,
    exportar, markdown, pauta, registrar_resposta,
};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/faturamento-exemplo")
}

fn data(dia: u8) -> Data {
    Data::nova(2026, 11, dia).unwrap()
}

struct Cenario {
    _dir: tempfile::TempDir,
    amb: Ambiente,
    demanda: IdDemanda,
    a1: Id,
    a2: Id,
    reuniao: IdReuniao,
}

fn analise(amb: &Ambiente, demanda: IdDemanda, titulo: &str, perguntas: &[&str]) -> Id {
    let id = amb
        .analises
        .criar_com_demanda(
            NovaAnalise {
                fluxo: Fluxo::Impacto,
                titulo: titulo.into(),
                entrada: "Parcelar a fatura de pessoa jurídica em até 3 vezes.".into(),
                analista: Some("Ana Exemplo".into()),
                base: None,
            },
            Some(Demanda {
                id: Some(demanda),
                origem: "texto".into(),
                chave: None,
            }),
        )
        .unwrap()
        .id;
    amb.analises
        .alterar(&id, Mudanca::Pergunta, |a, agora| {
            let n = a.registrar_afirmacao(
                NovaAfirmacao {
                    secao: "como_e_hoje".into(),
                    texto: "O parcelamento aceita até 6 parcelas para PJ.".into(),
                    confianca: Confianca::AConfirmar,
                    evidencias: vec![],
                    sessao: None,
                },
                agora,
            )?;
            for (i, p) in perguntas.iter().enumerate() {
                a.registrar_pergunta(p, (i == 0).then_some(n), None, agora)?;
            }
            Ok(())
        })
        .unwrap();
    id
}

fn cenario() -> Cenario {
    let dir = tempfile::tempdir().unwrap();
    let raiz = dir.path();
    let dados = DiretorioDados::em(raiz.join("dados"));
    let projeto = cba_index::Projeto::novo("Faturamento Exemplo", [fixture()]).unwrap();
    cba_index::Cadastro::em(dados.projetos())
        .cadastrar(&projeto)
        .unwrap();
    cba_kb::Base::criar(raiz.join("base")).unwrap();
    Config {
        base: Some(raiz.join("base")),
        ..Default::default()
    }
    .gravar(&dados)
    .unwrap();
    let amb = Ambiente::do_diretorio(&fixture(), &dados).unwrap();
    amb.kb
        .registrar_pessoa(&NovaPessoa {
            nome: "Maria Silva".into(),
            papel: Some("PO".into()),
            ..Default::default()
        })
        .unwrap();
    let demanda = amb
        .kb
        .criar_demanda(
            &NovaDemanda {
                titulo: "Parcelamento da fatura para PJ".into(),
                ..Default::default()
            },
            data(2),
        )
        .unwrap()
        .id;
    let a1 = analise(
        &amb,
        demanda,
        "Impacto do parcelamento",
        &[
            "O limite de parcelas vale para PJ?",
            "Há juros no parcelamento?",
        ],
    );
    let a2 = analise(
        &amb,
        demanda,
        "Impacto na remessa do banco",
        &["o limite de parcelas vale para PJ"],
    );
    // Ata aprovada com uma dúvida cujo responsável é a Maria (no catálogo).
    let (mut estado, arquivos) = EstadoAta::criar(&amb.kb, demanda, data(3)).unwrap();
    let mut duvida = NovoItem::novo(TipoItem::Duvida, "O boleto aceita vencimentos distintos?");
    duvida.responsavel = Some("Silva, Maria".into());
    estado.adicionar_item(duvida).unwrap();
    estado.status = StatusAta::Aprovada;
    amb.kb
        .gravar_texto(&arquivos.estado, &estado.para_json())
        .unwrap();
    Cenario {
        _dir: dir,
        amb,
        demanda,
        a1,
        a2,
        reuniao: arquivos.id,
    }
}

fn maria() -> Destinatario {
    Destinatario::Pessoa {
        pessoa: "maria-silva".into(),
    }
}

fn resposta(perguntas: &[&str], destino: DestinoDaResposta) -> RespostaRecebida {
    RespostaRecebida {
        texto: "Sim. O limite de 3 parcelas vale só para pessoa jurídica.".into(),
        por: "Maria Silva".into(),
        pessoa: Some("maria-silva".into()),
        em: Some(data(17)),
        fonte: "reunião de 17/11".into(),
        perguntas: perguntas.iter().map(|p| (*p).to_owned()).collect(),
        destino,
    }
}

#[test]
fn perguntas_de_duas_analises_juntas_por_destinatario() {
    let c = cenario();
    let a1 = format!("{}/1", c.a1);
    let a2 = format!("{}/1", c.a2);
    definir_destinatario(&c.amb, &[a1.clone(), a2.clone()], Some(maria())).unwrap();

    let p = pauta(&c.amb, Some(c.demanda), data(20)).unwrap();
    assert_eq!(
        p.titulo,
        format!("{}: Parcelamento da fatura para PJ", c.demanda)
    );
    assert_eq!(p.total, 4);
    let rotulos: Vec<&str> = p.grupos.iter().map(|g| g.rotulo.as_str()).collect();
    assert_eq!(rotulos, ["Maria Silva (PO)", SEM_DESTINATARIO]);

    let maria = &p.grupos[0];
    // As perguntas iguais das duas análises viram um item, com as duas origens;
    // a dúvida da ata vai para a Maria pelo responsável.
    assert_eq!(maria.itens.len(), 2);
    let igual = maria
        .itens
        .iter()
        .find(|i| i.perguntas.len() == 2)
        .expect("perguntas iguais juntas");
    let refs: Vec<&str> = igual
        .perguntas
        .iter()
        .map(|q| q.referencia.as_str())
        .collect();
    assert!(
        refs.contains(&a1.as_str()) && refs.contains(&a2.as_str()),
        "{refs:?}"
    );
    assert_eq!(
        igual.perguntas[0].contexto.as_deref(),
        Some("O parcelamento aceita até 6 parcelas para PJ.")
    );
    let da_ata = maria
        .itens
        .iter()
        .find(|i| i.perguntas[0].referencia == format!("{}/DV-1", c.reuniao))
        .expect("dúvida da ata");
    assert_eq!(da_ata.idade_dias, 17);
    assert!(da_ata.antiga);

    let sem = &p.grupos[1];
    assert_eq!(sem.destinatario, None);
    assert_eq!(sem.itens[0].perguntas[0].referencia, format!("{}/2", c.a1));

    // Exportação e e-mail.
    let md = markdown(&p);
    assert!(md.starts_with("# Pauta de perguntas: D-0001"), "{md}");
    assert!(md.contains("## Maria Silva (PO)"));
    assert!(md.contains("**O que o sistema faz hoje:** O parcelamento aceita até 6"));
    assert!(md.contains("- **Resposta:**"));
    let texto = email(&p, maria);
    assert!(texto.starts_with("Olá, Maria Silva,"), "{texto}");
    assert!(!texto.contains("A-0001"));
    let pasta = tempfile::tempdir().unwrap();
    for formato in [Formato::Markdown, Formato::Word] {
        let destino = pasta.path().join(format!("pauta.{}", formato.extensao()));
        exportar(&p, &destino, formato).unwrap();
        assert!(std::fs::metadata(&destino).unwrap().len() > 0);
    }
}

#[test]
fn resposta_fecha_a_pergunta_nas_duas_analises_e_vira_regra_informada() {
    let c = cenario();
    let a1 = format!("{}/1", c.a1);
    let a2 = format!("{}/1", c.a2);
    let r = registrar_resposta(
        &c.amb,
        &resposta(
            &[&a1, &a2],
            DestinoDaResposta::Regra {
                titulo: Some("Limite de 3 parcelas só para PJ".into()),
                area: None,
            },
        ),
        data(20),
    )
    .unwrap();
    assert_eq!(r.fechadas, [a1.clone(), a2.clone()]);

    for id in [&c.a1, &c.a2] {
        let a = c.amb.analises.carregar(id).unwrap();
        let p = &a.perguntas[0];
        assert_eq!(
            p.estado,
            EstadoPergunta::Respondida {
                resposta: "Sim. O limite de 3 parcelas vale só para pessoa jurídica.".into()
            }
        );
        let info = p.informada.as_ref().unwrap();
        assert_eq!(info.por, "Maria Silva");
        assert_eq!(info.em, data(17));
        assert_eq!(info.fonte, "reunião de 17/11");
    }
    // Fora da pauta; a outra pergunta continua.
    let p = pauta(&c.amb, Some(c.demanda), data(20)).unwrap();
    assert_eq!(p.total, 2);

    // A regra informada: RN-…, sem evidência, marcada como tal.
    let regra = r.regra.unwrap();
    assert_eq!(regra.id, "RN-GER-001");
    let id = Id::analisar(&regra.id).unwrap();
    let doc: Documento<FrontmatterRegra> = c.amb.kb.ler(&id.arquivo()).unwrap();
    let f = &doc.frontmatter;
    assert!(f.evidencias.is_empty());
    assert_eq!(f.titulo, "Limite de 3 parcelas só para PJ");
    let informada = f.informada.as_ref().expect("origem informada");
    assert_eq!(informada.informacao.por, "Maria Silva");
    assert_eq!(informada.perguntas, [a1.clone(), a2.clone()]);
    assert!(
        doc.corpo.contains(
            "informada por Maria Silva em 2026-11-17 (reunião de 17/11) (sem evidência no código)"
        ),
        "{}",
        doc.corpo
    );
    assert!(doc.corpo.contains("> **Regra informada por Maria Silva"));
    assert!(
        doc.corpo
            .contains(&format!("- {a1}: O limite de parcelas vale para PJ?"))
    );

    // Responder de novo: a pergunta já está fechada; nada muda.
    let erro = registrar_resposta(
        &c.amb,
        &resposta(
            &[&format!("{}/2", c.a1), &a2],
            DestinoDaResposta::SoRegistrar,
        ),
        data(20),
    )
    .unwrap_err();
    assert!(
        matches!(erro, ErroPauta::NaoPendente(ref r) if *r == a2),
        "{erro}"
    );
    let a = c.amb.analises.carregar(&c.a1).unwrap();
    assert_eq!(a.perguntas[1].estado, EstadoPergunta::Aberta);
}

#[test]
fn resposta_valida_premissa_e_fecha_a_duvida_da_ata() {
    let c = cenario();
    let premissa = c
        .amb
        .kb
        .criar_registro(
            c.demanda,
            &NovoRegistro {
                categoria: Categoria::Premissa,
                dados: DadosRegistro {
                    titulo: "O banco aceita vencimentos distintos na mesma remessa.".into(),
                    ..Default::default()
                },
                origem: None,
                referencia: None,
                evento: "criada à mão".into(),
            },
            data(4),
        )
        .unwrap();
    let duvida = format!("{}/DV-1", c.reuniao);
    let r = registrar_resposta(
        &c.amb,
        &resposta(
            &[&duvida],
            DestinoDaResposta::Premissa {
                premissa: premissa.id.to_string(),
                status: StatusPremissa::Validada,
            },
        ),
        data(20),
    )
    .unwrap();
    assert_eq!(r.premissa.as_deref(), Some("PR-0001"));
    let p = pauta(&c.amb, Some(c.demanda), data(20)).unwrap();
    assert_eq!(p.premissas.len(), 1);
    assert_eq!(p.premissas[0].status, StatusPremissa::Validada);
    let lida = c.amb.kb.registro(premissa.id).unwrap();
    let f = lida.frontmatter();
    assert_eq!(f.status, Some(StatusPremissa::Validada));
    let v = f.validacao.as_ref().unwrap();
    assert!(
        v.como.starts_with("informado por Maria Silva"),
        "{}",
        v.como
    );
    assert_eq!(v.pergunta.as_deref(), Some(duvida.as_str()));

    let (estado, arquivos) = EstadoAta::ler(&c.amb.kb, c.reuniao).unwrap();
    let item = &estado.itens[0];
    assert_eq!(
        item.resposta.as_ref().unwrap().informacao.por,
        "Maria Silva"
    );
    let ata = c.amb.kb.ler_texto(&arquivos.ata).unwrap();
    assert!(
        ata.contains("**Resposta** (informada por Maria Silva"),
        "{ata}"
    );

    // Premissa que não é premissa é recusada antes de fechar qualquer coisa.
    let erro = registrar_resposta(
        &c.amb,
        &resposta(
            &[&format!("{}/2", c.a1)],
            DestinoDaResposta::Premissa {
                premissa: "DE-0001".into(),
                status: StatusPremissa::Validada,
            },
        ),
        data(20),
    )
    .unwrap_err();
    assert!(matches!(erro, ErroPauta::NaoEPremissa(_)), "{erro}");
    let a = c.amb.analises.carregar(&c.a1).unwrap();
    assert_eq!(a.perguntas[1].estado, EstadoPergunta::Aberta);
}

#[test]
fn pauta_do_projeto_e_referencias_invalidas() {
    let c = cenario();
    let p = pauta(&c.amb, None, data(20)).unwrap();
    assert_eq!(p.total, 4);
    assert!(p.pessoas.iter().any(|x| x.slug == "maria-silva"));
    assert!(p.papeis.iter().any(|x| x == "PO"));
    assert!(matches!(
        registrar_resposta(
            &c.amb,
            &resposta(&["X-1"], DestinoDaResposta::SoRegistrar),
            data(20)
        ),
        Err(ErroPauta::Referencia(_))
    ));
    assert!(matches!(
        registrar_resposta(
            &c.amb,
            &resposta(&[], DestinoDaResposta::SoRegistrar),
            data(20)
        ),
        Err(ErroPauta::SemPerguntas)
    ));
}
