//! Premissas, decisões, riscos e restrições da demanda (#122) com o sistema
//! fictício "Faturamento Exemplo": a análise de impacto da demanda propõe
//! premissas e riscos, a ata propõe decisões com a pessoa do catálogo; o analista
//! aceita (com ajustes) ou recusa; o registro aceito aponta para a origem e para a
//! pessoa no índice e entra no documento exportado.

use std::path::{Path, PathBuf};

use cba_core::ambiente::Ambiente;
use cba_core::analise::{Analise, Confianca, EstadoAfirmacao, Fluxo, NovaAfirmacao};
use cba_core::analises::NovaAnalise;
use cba_core::config::Config;
use cba_core::dados::DiretorioDados;
use cba_core::eventos::Mudanca;
use cba_docs::{CitacaoFala, EstadoAta, NovoItem, Participante, StatusCitacao, TipoItem};
use cba_kb::frontmatter::{Data, Demanda, Horario};
use cba_kb::indice::{Indice, Tipo};
use cba_kb::registros::{Nivel, StatusPremissa};
use cba_kb::{Categoria, IdDemanda, NovaDemanda, NovaPessoa, TipoDocumento};
use cba_tools::registros::{
    ErroRegistros, aceitar_proposta, com_registros, propostas, recusar_proposta, secao_markdown,
};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/faturamento-exemplo")
}

fn data(dia: u8) -> Data {
    Data::nova(2026, 11, dia).unwrap()
}

struct Cenario {
    dir: tempfile::TempDir,
    amb: Ambiente,
    demanda: IdDemanda,
    analise: Analise,
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
            data(9),
        )
        .unwrap()
        .id;

    // Análise de impacto ligada à demanda, com as afirmações que o roteiro pede.
    let analise = amb
        .analises
        .criar_com_demanda(
            NovaAnalise {
                fluxo: Fluxo::Impacto,
                titulo: "Parcelamento da fatura para PJ".into(),
                entrada: "Hoje o sistema não parcela fatura nenhuma; queremos parcelar em até \
                          3 vezes para pessoa jurídica."
                    .into(),
                analista: Some("Ana Exemplo".into()),
                base: None,
            },
            Some(Demanda {
                id: Some(demanda),
                origem: "texto".into(),
                chave: None,
            }),
        )
        .unwrap();
    let afirmacoes = [
        (
            "demanda_como_entendida",
            "Premissa do solicitante: hoje o projeto não parcela fatura nenhuma. Verificação: \
             não confere. O parcelamento já existe para PJ, em até 6 vezes.",
            Confianca::Inferido,
        ),
        (
            "demanda_como_entendida",
            "Premissa do solicitante: o banco aceita boletos com vencimentos distintos na \
             mesma remessa. Verificação: confere em parte.",
            Confianca::AConfirmar,
        ),
        (
            "demanda_como_entendida",
            "Demanda como entendida: limitar o parcelamento de PJ a 3 vezes.",
            Confianca::Inferido,
        ),
        (
            "riscos",
            "Faturas antigas não têm a coluna de parcela. A migração precisa preencher o valor.",
            Confianca::Inferido,
        ),
        (
            "riscos",
            "Risco descartado pelo analista.",
            Confianca::Inferido,
        ),
        (
            "como_e_hoje",
            "O parcelamento aceita até 6 parcelas para PJ.",
            Confianca::Inferido,
        ),
    ];
    let (_, analise) = amb
        .analises
        .alterar(&analise.id, Mudanca::Afirmacao, |a, agora| {
            for (secao, texto, confianca) in afirmacoes {
                a.registrar_afirmacao(
                    NovaAfirmacao {
                        secao: secao.into(),
                        texto: texto.into(),
                        confianca,
                        evidencias: vec![],
                        sessao: None,
                    },
                    agora,
                )?;
            }
            a.marcar_afirmacao(
                5,
                EstadoAfirmacao::Incorreta {
                    comentario: "não se aplica".into(),
                },
            )
        })
        .unwrap();

    // Ata da reunião com uma decisão dita pela Maria Silva (no catálogo).
    let (mut estado, arquivos) = EstadoAta::criar(&amb.kb, demanda, data(10)).unwrap();
    estado.definir_participantes(vec![Participante {
        pessoa: Some("maria-silva".into()),
        papel: Some("PO".into()),
        ..Participante::novo("Silva, Maria")
    }]);
    estado
        .adicionar_item(
            NovoItem::novo(
                TipoItem::Decisao,
                "Máximo de 3 parcelas; acima disso, só com aprovação do financeiro.",
            )
            .com_citacao(CitacaoFala::nova(
                "Silva, Maria",
                Horario::de_segundos(754),
                "três parcelas no máximo",
                StatusCitacao::Conferida,
            )),
        )
        .unwrap();
    estado
        .adicionar_item(NovoItem::novo(TipoItem::Pendencia, "Consultar o banco."))
        .unwrap();
    amb.kb
        .gravar_texto(&arquivos.estado, &estado.para_json())
        .unwrap();

    Cenario {
        dir,
        amb,
        demanda,
        analise,
    }
}

#[test]
fn analise_de_impacto_e_ata_propoem_e_o_analista_aceita_ou_recusa() {
    let c = cenario();
    let ps = propostas(&c.amb, c.demanda).unwrap();
    let refs: Vec<(&str, Categoria)> = ps
        .iter()
        .map(|p| (p.referencia.as_str(), p.categoria))
        .collect();
    assert_eq!(
        refs,
        [
            ("A-0001/1", Categoria::Premissa),
            ("A-0001/2", Categoria::Premissa),
            ("A-0001/4", Categoria::Risco),
            ("R-0001/D-1", Categoria::Decisao),
        ]
    );
    let premissa = &ps[0].dados;
    assert_eq!(
        premissa.titulo,
        "hoje o projeto não parcela fatura nenhuma."
    );
    assert_eq!(premissa.status, Some(StatusPremissa::Invalidada));
    assert!(
        premissa
            .como_validada
            .as_deref()
            .unwrap()
            .contains("A-0001")
    );
    assert_eq!(ps[1].dados.status, Some(StatusPremissa::NaoValidada));
    assert_eq!(
        ps[2].dados.titulo,
        "Faturas antigas não têm a coluna de parcela."
    );
    let decisao = &ps[3].dados;
    assert_eq!(decisao.pessoa.as_deref(), Some("maria-silva"));
    assert_eq!(decisao.quem.as_deref(), Some("Maria Silva"));
    assert_eq!(decisao.decidido_em, Some(data(10)));

    // Aceita a premissa como veio, o risco com ajustes e a decisão; recusa a outra.
    let p = aceitar_proposta(&c.amb, c.demanda, "A-0001/1", None, data(11)).unwrap();
    assert_eq!(p.id.to_string(), "PR-0001");
    assert_eq!(p.frontmatter().referencia.as_deref(), Some("A-0001/1"));
    assert_eq!(p.frontmatter().origem.as_ref(), Some(&c.analise.documento));
    let mut risco = ps[2].dados.clone();
    risco.probabilidade = Some(Nivel::Alto);
    risco.impacto = Some(Nivel::Alto);
    risco.resposta = Some("Migrar antes de liberar".into());
    let r = aceitar_proposta(&c.amb, c.demanda, "A-0001/4", Some(risco), data(11)).unwrap();
    assert_eq!(r.frontmatter().probabilidade, Some(Nivel::Alto));
    let d = aceitar_proposta(&c.amb, c.demanda, "R-0001/D-1", None, data(11)).unwrap();
    recusar_proposta(&c.amb, c.demanda, "A-0001/2").unwrap();
    assert!(propostas(&c.amb, c.demanda).unwrap().is_empty());
    assert!(matches!(
        aceitar_proposta(&c.amb, c.demanda, "A-0001/2", None, data(11)),
        Err(ErroRegistros::PropostaInexistente(_))
    ));

    // A decisão aponta para a ata e para a pessoa, no arquivo e no índice.
    let ata = c.amb.kb.reuniao(cba_kb::IdReuniao::novo(1)).unwrap().ata;
    assert_eq!(d.frontmatter().origem.as_ref(), Some(&ata));
    assert_eq!(d.frontmatter().referencia.as_deref(), Some("R-0001/D-1"));
    assert!(d.detalhe().unwrap().contains("três parcelas no máximo"));
    let mut indice = Indice::abrir(c.dir.path().join("indice.sqlite"), &c.amb.base).unwrap();
    indice.sincronizar().unwrap();
    let ligacoes = indice.ligacoes(&c.amb.kb.na_base(&d.arquivo)).unwrap();
    let para = |campo: &str| {
        ligacoes
            .iter()
            .find(|l| l.campo == campo)
            .map(|l| l.para.as_str().to_owned())
    };
    assert_eq!(
        para("origem").as_deref(),
        Some(c.amb.kb.na_base(&ata).as_str())
    );
    assert_eq!(
        para("pessoa").as_deref(),
        Some("faturamento-exemplo/pessoas/maria-silva.md")
    );
    let da_demanda = indice
        .documentos_da_demanda("faturamento-exemplo", c.demanda)
        .unwrap();
    for (tipo, id) in [
        (TipoDocumento::Premissa, "PR-0001"),
        (TipoDocumento::Risco, "RI-0001"),
        (TipoDocumento::Decisao, "DE-0001"),
    ] {
        assert!(
            da_demanda
                .iter()
                .any(|x| x.tipo == Some(Tipo::Documento(tipo))
                    && x.identificador.as_deref() == Some(id)),
            "{id} no índice: {da_demanda:?}"
        );
    }
}

#[test]
fn exportacao_ganha_a_secao_dos_registros() {
    let c = cenario();
    assert!(secao_markdown(&c.amb.kb, c.demanda).unwrap().is_none());
    assert_eq!(com_registros(&c.amb, &c.analise, "# A\n".into()), "# A\n");

    aceitar_proposta(&c.amb, c.demanda, "A-0001/1", None, data(11)).unwrap();
    aceitar_proposta(&c.amb, c.demanda, "R-0001/D-1", None, data(11)).unwrap();
    let secao = secao_markdown(&c.amb.kb, c.demanda).unwrap().unwrap();
    assert!(
        secao.starts_with("## Premissas, decisões, riscos e restrições"),
        "{secao}"
    );
    assert!(
        secao.contains("### Premissas\n\n- **PR-0001** hoje o projeto"),
        "{secao}"
    );
    assert!(secao.contains("invalidada"), "{secao}");
    assert!(
        secao.contains("### Decisões\n\n- **DE-0001** Máximo de 3 parcelas"),
        "{secao}"
    );
    assert!(secao.contains("decidiu Maria Silva"), "{secao}");
    assert!(!secao.contains("### Riscos"), "{secao}");

    let texto = com_registros(
        &c.amb,
        &c.analise,
        "# A\n\n## Riscos\n\nx\n\n## Evidências\n\n[^1]: a\n".into(),
    );
    let registros = texto.find("## Premissas, decisões").unwrap();
    let evidencias = texto.find("## Evidências").unwrap();
    assert!(registros < evidencias, "{texto}");
}
