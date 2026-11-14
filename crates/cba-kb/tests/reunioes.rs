//! Reuniões das demandas (#135): `R-NNNN` sequencial por projeto, arquivos na
//! pasta `reunioes/` da demanda, reserva sem colisão e a ata no índice.

use std::collections::BTreeSet;
use std::fs;
use std::sync::Arc;
use std::thread;

use cba_kb::frontmatter::{Data, FrontmatterAta, FrontmatterProjeto, Horario, StatusAta};
use cba_kb::indice::{Indice, Tipo};
use cba_kb::{Base, Documento, Erro, IdDemanda, IdReuniao, NovaDemanda, Projeto, TipoDocumento};

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

#[test]
fn caminhos_e_numeracao_por_projeto() {
    let (_dir, _base, projeto) = criar_base();
    let d1 = demanda(&projeto, "Parcelamento PJ");
    let d2 = demanda(&projeto, "Multa por atraso");
    assert_eq!(projeto.proxima_reuniao().unwrap().to_string(), "R-0001");
    assert!(projeto.reunioes().unwrap().is_empty());

    let r1 = projeto.criar_reuniao(d1, |id| format!("{id}")).unwrap();
    let r2 = projeto.criar_reuniao(d2, |id| format!("{id}")).unwrap();
    assert_eq!(r1.id, IdReuniao::novo(1));
    assert_eq!(r2.id, IdReuniao::novo(2));
    assert_eq!(
        r2.estado.as_str(),
        "demandas/D-0002-multa-por-atraso/reunioes/R-0002.estado.json"
    );
    assert_eq!(
        r2.ata.as_str(),
        "demandas/D-0002-multa-por-atraso/reunioes/R-0002.md"
    );
    assert_eq!(projeto.ler_texto(&r2.estado).unwrap(), "R-0002");
    assert_eq!(projeto.reuniao(IdReuniao::novo(2)).unwrap(), r2);
    assert_eq!(projeto.reunioes().unwrap(), vec![r1.clone(), r2]);
    assert_eq!(projeto.reunioes_da_demanda(d1).unwrap(), vec![r1]);
    assert!(matches!(
        projeto.reuniao(IdReuniao::novo(3)),
        Err(Erro::ReuniaoInexistente { .. })
    ));
    assert!(matches!(
        projeto.criar_reuniao(IdDemanda::novo(9), |id| id.to_string()),
        Err(Erro::DemandaInexistente { .. })
    ));
    // Nenhuma trava sobrou.
    let ocultos: Vec<_> = fs::read_dir(projeto.raiz().join("demandas"))
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .filter(|n| n.starts_with('.'))
        .collect();
    assert!(ocultos.is_empty(), "{ocultos:?}");
}

#[test]
fn trava_esquecida_so_pula_o_numero() {
    let (_dir, _base, projeto) = criar_base();
    let d1 = demanda(&projeto, "Parcelamento PJ");
    fs::create_dir(projeto.raiz().join("demandas/.R-0001.reserva")).unwrap();
    let r = projeto.criar_reuniao(d1, |id| id.to_string()).unwrap();
    assert_eq!(r.id.to_string(), "R-0002");
}

#[test]
fn processos_ao_mesmo_tempo_nao_colidem_entre_demandas() {
    let (_dir, _base, projeto) = criar_base();
    let demandas = [
        demanda(&projeto, "Parcelamento PJ"),
        demanda(&projeto, "Multa por atraso"),
    ];
    let projeto = Arc::new(projeto);
    let threads: Vec<_> = (0..8)
        .map(|i| {
            let projeto = Arc::clone(&projeto);
            let d = demandas[i % 2];
            thread::spawn(move || {
                (0..5)
                    .map(|_| {
                        projeto
                            .criar_reuniao(d, |id| id.to_string())
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
    assert_eq!(unicos, (1..=40).collect(), "{numeros:?}");
    let reunioes = projeto.reunioes().unwrap();
    assert_eq!(reunioes.len(), 40);
    for r in reunioes {
        assert_eq!(projeto.ler_texto(&r.estado).unwrap(), r.id.to_string());
    }
}

#[test]
fn ata_no_indice_ligada_a_demanda() {
    let (dir, base, projeto) = criar_base();
    let d1 = demanda(&projeto, "Parcelamento PJ");
    let r = projeto.criar_reuniao(d1, |_| "{}".into()).unwrap();
    projeto
        .criar(
            &r.ata,
            &Documento::novo(
                FrontmatterAta {
                    tipo: TipoDocumento::Ata,
                    id: r.id,
                    projeto: PROJETO.into(),
                    demanda: d1,
                    data: data(12),
                    duracao: Horario::analisar("00:47:10"),
                    status: StatusAta::Rascunho,
                },
                "\n# R-0001 · Ata da reunião de 2026-11-12\n",
            ),
        )
        .unwrap();

    let mut indice = Indice::abrir(dir.path().join("indice/base.sqlite"), &base).unwrap();
    indice.sincronizar().unwrap();
    assert!(indice.problemas().unwrap().is_empty());
    let ligados = indice.documentos_da_demanda(PROJETO, d1).unwrap();
    assert_eq!(ligados.len(), 1);
    assert_eq!(ligados[0].tipo, Some(Tipo::Documento(TipoDocumento::Ata)));
    assert_eq!(ligados[0].identificador.as_deref(), Some("R-0001"));
}
