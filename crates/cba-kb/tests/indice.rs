//! Índice SQLite da base (#8), com o sistema fictício "Faturamento Exemplo" e as
//! regras do gabarito (`fixtures/gabaritos`).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use cba_kb::frontmatter::{
    Area, Data, Demanda, Evidencia, FrontmatterAnalise, FrontmatterRegra, FrontmatterSistema,
    FrontmatterSugestao, FrontmatterTermo, Linhas, StatusAnalise, StatusRegra, StatusSugestao,
};
use cba_kb::indice::{Consulta, Indice, Modo, Sincronizacao, Tipo, Vigia};
use cba_kb::{Base, CaminhoRelativo, Documento, Id, Sistema, TipoDocumento, layout};
use serde::Deserialize;

const SISTEMA: &str = "faturamento-exemplo";

#[derive(Deserialize)]
struct Gabarito {
    regras: Vec<RegraGabarito>,
}

#[derive(Deserialize)]
struct RegraGabarito {
    id: String,
    titulo: String,
    area: String,
    enunciado: String,
    evidencias: Vec<EvidenciaGabarito>,
}

#[derive(Deserialize)]
struct EvidenciaGabarito {
    arquivo: String,
    linha_inicial: u32,
    linha_final: u32,
}

fn gabarito() -> Gabarito {
    let caminho = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/gabaritos/faturamento-exemplo.yaml");
    serde_norway::from_str(&fs::read_to_string(caminho).unwrap()).unwrap()
}

fn data() -> Data {
    Data::nova(2026, 11, 11).unwrap()
}

fn caminho(texto: &str) -> CaminhoRelativo {
    CaminhoRelativo::novo(texto).unwrap()
}

fn criar_sistema(base: &Base, id: &str, nome: &str) -> Sistema {
    base.criar_sistema(&Documento::novo(
        FrontmatterSistema {
            tipo: TipoDocumento::Sistema,
            id: id.into(),
            nome: nome.into(),
            areas: vec![Area {
                id: "cobranca".into(),
                nome: "Cobrança".into(),
                sigla: Some("FAT".into()),
            }],
            criado: Some(data()),
        },
        format!("\n# {nome}\n\nSistema fictício.\n"),
    ))
    .unwrap()
}

fn regra(
    id: &str,
    titulo: &str,
    area: &str,
    corpo: &str,
    evidencias: Vec<Evidencia>,
) -> Documento<FrontmatterRegra> {
    Documento::novo(
        FrontmatterRegra {
            tipo: TipoDocumento::Regra,
            id: Id::analisar(id).unwrap(),
            titulo: titulo.into(),
            area: area.into(),
            status: StatusRegra::Confirmada,
            evidencias,
            origem: Some(caminho("analises/A-0001.md")),
        },
        format!("\n{corpo}\n"),
    )
}

fn termo(nome: &str, sinonimos: &[&str], corpo: &str) -> Documento<FrontmatterTermo> {
    Documento::novo(
        FrontmatterTermo {
            tipo: TipoDocumento::Termo,
            termo: nome.into(),
            sinonimos: sinonimos.iter().map(|s| (*s).into()).collect(),
            area: Some("cobranca".into()),
            origem: Some(caminho("analises/A-0001.md")),
        },
        format!("\n{corpo}\n"),
    )
}

/// Base numa pasta com espaços e acentos: o Faturamento Exemplo com as regras do
/// gabarito, dois termos, uma análise, uma sugestão, as regras do sistema e dois
/// documentos com problema; e um segundo sistema, para os filtros.
fn montar_base() -> (tempfile::TempDir, Base, Sistema, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let base = Base::criar(dir.path().join("Base de Conhecimento – Análises")).unwrap();
    let sistema = criar_sistema(&base, SISTEMA, "Faturamento Exemplo");

    for r in gabarito().regras {
        let evidencias = r
            .evidencias
            .iter()
            .map(|e| Evidencia {
                arquivo: caminho(&e.arquivo),
                linhas: Linhas::novo(e.linha_inicial, e.linha_final).unwrap(),
                hash: "0000".into(),
                commit: Some("1a2b3c4d".into()),
            })
            .collect();
        let doc = regra(&r.id, &r.titulo, &r.area, &r.enunciado, evidencias);
        sistema
            .criar(&Id::analisar(&r.id).unwrap().arquivo(), &doc)
            .unwrap();
    }
    sistema
        .criar(
            &layout::termo("Carência").unwrap(),
            &termo(
                "Carência",
                &["período de tolerância"],
                "Dias após o vencimento em que a fatura em atraso não gera encargos.",
            ),
        )
        .unwrap();
    sistema
        .criar(
            &layout::termo("Fatura").unwrap(),
            &termo(
                "Fatura",
                &["conta"],
                "Documento de cobrança emitido ao cliente.",
            ),
        )
        .unwrap();
    sistema
        .criar(
            &caminho("analises/A-0001.md"),
            &Documento::novo(
                FrontmatterAnalise {
                    tipo: TipoDocumento::Pergunta,
                    id: Id::analisar("A-0001").unwrap(),
                    titulo: "Como são calculados multa e juros de atraso?".into(),
                    sistema: SISTEMA.into(),
                    demanda: Demanda {
                        origem: "texto".into(),
                        chave: None,
                    },
                    status: StatusAnalise::Aprovada,
                    abrangencia: None,
                    base: None,
                    criado: data(),
                    analista: Some("Ana Exemplo".into()),
                },
                "\n# Pergunta\n\nComo a cobrança calcula os encargos de uma fatura paga \
                 com atraso?\n\n## Resposta\n\nPassada a carência, há multa única e \
                 juros diários.\n",
            ),
        )
        .unwrap();
    sistema
        .criar(
            &caminho("sugestoes/S-0001.md"),
            &Documento::novo(
                FrontmatterSugestao {
                    tipo: TipoDocumento::Sugestao,
                    id: Id::analisar("S-0001").unwrap(),
                    titulo: "Ler os parâmetros de cobrança no lote COBOL".into(),
                    sistema: SISTEMA.into(),
                    status: StatusSugestao::Proposta,
                    analise: Some(Id::analisar("A-0001").unwrap()),
                    arquivo: Some(caminho("cobol/CALCOBR.cbl")),
                    criado: data(),
                },
                "\nO lote usa valores fixos.\n",
            ),
        )
        .unwrap();
    fs::write(
        sistema.caminho(&layout::regras_do_sistema()),
        "# Regras do sistema\n\n<!-- Instruções: palavrasecreta. -->\n\n\
         - Chame o cliente de contratante nos documentos.\n",
    )
    .unwrap();
    // Status inválido: fica nos problemas, mas continua na busca pelo título.
    fs::write(
        sistema.raiz().join("regras").join("RN-FAT-900.md"),
        "---\ntipo: regra\nid: RN-FAT-900\ntitulo: Arredondamento bancário\narea: cobranca\n\
         status: talvez\n---\n\nTexto.\n",
    )
    .unwrap();
    // Sem frontmatter: fica só nos problemas.
    fs::write(
        sistema.raiz().join("areas").join("rascunho.md"),
        "Anotações soltas sobre arredondamento.\n",
    )
    .unwrap();

    let outro = criar_sistema(&base, "outro-sistema", "Outro Sistema");
    outro
        .criar(
            &caminho("regras/RN-FAT-001.md"),
            &regra(
                "RN-FAT-001",
                "Carência de 5 dias no outro sistema",
                "cobranca",
                "Atraso de até 5 dias não gera encargos.",
                Vec::new(),
            ),
        )
        .unwrap();

    let arquivo = dir.path().join("dados").join("indice").join("base.sqlite");
    (dir, base, sistema, arquivo)
}

fn abrir(arquivo: &Path, base: &Base) -> Indice {
    let mut indice = Indice::abrir(arquivo, base).unwrap();
    indice.sincronizar().unwrap();
    indice
}

/// Caminhos dos resultados.
fn achados(indice: &Indice, consulta: &Consulta) -> Vec<String> {
    indice
        .buscar(consulta)
        .unwrap()
        .resultados
        .into_iter()
        .map(|r| r.caminho.to_string())
        .collect()
}

fn consultas() -> Vec<Consulta> {
    vec![
        Consulta::nova("carencia"),
        Consulta::nova("multa juros").sistema(SISTEMA),
        Consulta::nova("parcelamento").tipos([Tipo::Documento(TipoDocumento::Regra)]),
        Consulta::nova("Como é calculada a multa por atraso?").modo(Modo::Qualquer),
        Consulta::nova("RN-FAT-003"),
        Consulta::nova("tolerancia"),
        Consulta::nova("contratante"),
    ]
}

/// Resultado completo (caminho, título, trecho e relevância) das consultas.
fn fotografia(indice: &Indice) -> Vec<String> {
    consultas()
        .iter()
        .flat_map(|c| indice.buscar(c).unwrap().resultados)
        .map(|r| {
            format!(
                "{} | {} | {} | {:.6}",
                r.caminho, r.titulo, r.trecho, r.relevancia
            )
        })
        .collect()
}

#[test]
fn carencia_encontra_carencia() {
    let (_dir, base, _sistema, arquivo) = montar_base();
    let indice = abrir(&arquivo, &base);

    let regras = Consulta::nova("carencia")
        .sistema(SISTEMA)
        .tipos([Tipo::Documento(TipoDocumento::Regra)]);
    let pagina = indice.buscar(&regras).unwrap();
    let primeiro = &pagina.resultados[0];
    assert_eq!(primeiro.titulo, "Carência de 3 dias sem encargos");
    assert_eq!(primeiro.identificador.as_deref(), Some("RN-FAT-003"));
    assert_eq!(primeiro.tipo, Some(Tipo::Documento(TipoDocumento::Regra)));
    assert_eq!(primeiro.status.as_deref(), Some("confirmada"));
    assert_eq!(primeiro.area.as_deref(), Some("cobranca"));
    // O trecho marca a palavra encontrada, como está no texto.
    assert!(
        primeiro.trecho.to_lowercase().contains("**carência**")
            || primeiro.trecho.to_lowercase().contains("carencia**"),
        "{}",
        primeiro.trecho
    );

    // Com acento, em maiúsculas ou só o começo da palavra: o mesmo resultado.
    for texto in ["carência", "CARÊNCIA", "Carenc"] {
        assert_eq!(
            achados(
                &indice,
                &Consulta {
                    texto: texto.into(),
                    ..regras.clone()
                }
            ),
            achados(&indice, &regras),
            "{texto}"
        );
    }

    // Termos e análises também, e sinônimos contam.
    let todos = achados(&indice, &Consulta::nova("carencia").sistema(SISTEMA));
    assert!(
        todos.contains(&format!("{SISTEMA}/glossario/carencia.md")),
        "{todos:?}"
    );
    assert!(
        todos.contains(&format!("{SISTEMA}/analises/A-0001.md")),
        "{todos:?}"
    );
    assert_eq!(
        achados(&indice, &Consulta::nova("tolerancia")),
        [format!("{SISTEMA}/glossario/carencia.md")]
    );
    // Cedilha também: "cobranca" encontra "cobrança".
    assert!(!achados(&indice, &Consulta::nova("cobranca encargos")).is_empty());
}

#[test]
fn apagar_o_indice_e_reabrir_reconstroi_o_mesmo_resultado() {
    let (_dir, base, _sistema, arquivo) = montar_base();
    let indice = abrir(&arquivo, &base);
    let antes = fotografia(&indice);
    assert!(antes.len() > 10, "{antes:?}");
    drop(indice);

    fs::remove_dir_all(arquivo.parent().unwrap()).unwrap();
    let mut indice = Indice::abrir(&arquivo, &base).unwrap();
    assert!(fotografia(&indice).is_empty());
    let resumo = indice.sincronizar().unwrap();
    assert_eq!(resumo.removidos + resumo.alterados, 0);
    assert_eq!(fotografia(&indice), antes);

    // Reconstruir sem apagar dá o mesmo.
    let resumo = indice.reconstruir().unwrap();
    assert_eq!(resumo.inalterados, 0);
    assert_eq!(fotografia(&indice), antes);
}

#[test]
fn atualizacao_incremental_por_hash() {
    let (_dir, base, sistema, arquivo) = montar_base();
    let mut indice = Indice::abrir(&arquivo, &base).unwrap();
    let primeira = indice.sincronizar().unwrap();
    assert!(primeira.novos > 15, "{primeira:?}");
    assert_eq!(
        primeira.alterados + primeira.removidos + primeira.inalterados,
        0
    );

    let segunda = indice.sincronizar().unwrap();
    assert_eq!(
        segunda,
        Sincronizacao {
            inalterados: primeira.novos,
            ..Default::default()
        }
    );

    // Edição externa, novo arquivo e arquivo apagado.
    let rn001 = sistema.caminho(&caminho("regras/RN-FAT-001.md"));
    let texto = fs::read_to_string(&rn001).unwrap();
    fs::write(
        &rn001,
        format!("{texto}\nValor de referência: palavranova.\n"),
    )
    .unwrap();
    fs::write(
        sistema.raiz().join("glossario").join("multa.md"),
        "---\ntipo: termo\ntermo: Multa\n---\n\nPenalidade por atraso.\n",
    )
    .unwrap();
    fs::remove_file(sistema.raiz().join("glossario").join("fatura.md")).unwrap();
    // Gravado de novo com o mesmo conteúdo: não conta como alterado.
    let a1 = sistema.caminho(&caminho("analises/A-0001.md"));
    fs::write(&a1, fs::read(&a1).unwrap()).unwrap();

    let terceira = indice.sincronizar().unwrap();
    assert_eq!(
        (terceira.novos, terceira.alterados, terceira.removidos),
        (1, 1, 1)
    );
    assert_eq!(
        achados(&indice, &Consulta::nova("palavranova")),
        [format!("{SISTEMA}/regras/RN-FAT-001.md")]
    );
    assert_eq!(
        achados(&indice, &Consulta::nova("penalidade")),
        [format!("{SISTEMA}/glossario/multa.md")]
    );
    let termos = achados(
        &indice,
        &Consulta::nova("fatura").tipos([Tipo::Documento(TipoDocumento::Termo)]),
    );
    assert!(
        !termos.contains(&format!("{SISTEMA}/glossario/fatura.md")),
        "{termos:?}"
    );
}

#[test]
fn regras_do_sistema_e_documentos_com_problema() {
    let (_dir, base, _sistema, arquivo) = montar_base();
    let indice = abrir(&arquivo, &base);

    let pagina = indice.buscar(&Consulta::nova("contratante")).unwrap();
    assert_eq!(pagina.total, 1);
    let r = &pagina.resultados[0];
    assert_eq!(r.tipo, Some(Tipo::RegrasDoSistema));
    assert_eq!(r.titulo, "Regras do sistema");
    assert_eq!(
        r.caminho.as_str(),
        format!("{SISTEMA}/regras-do-sistema.md")
    );
    // Os comentários são instrução, não conteúdo; o padrão do outro sistema só tem
    // comentários.
    assert!(achados(&indice, &Consulta::nova("palavrasecreta")).is_empty());
    assert!(
        achados(
            &indice,
            &Consulta::nova("Exemplos").tipos([Tipo::RegrasDoSistema])
        )
        .is_empty()
    );

    let problemas = indice.problemas().unwrap();
    let caminhos: Vec<&str> = problemas.iter().map(|p| p.caminho.as_str()).collect();
    assert_eq!(
        caminhos,
        [
            format!("{SISTEMA}/areas/rascunho.md"),
            format!("{SISTEMA}/regras/RN-FAT-900.md"),
        ]
    );
    assert!(
        problemas[0].erro.contains("sem frontmatter"),
        "{}",
        problemas[0].erro
    );
    assert!(
        problemas[1].erro.contains("status"),
        "{}",
        problemas[1].erro
    );
    // A regra com status inválido continua na busca; o arquivo sem frontmatter não.
    assert_eq!(
        achados(&indice, &Consulta::nova("arredondamento")),
        [format!("{SISTEMA}/regras/RN-FAT-900.md")]
    );
}

#[test]
fn filtros_e_paginacao() {
    let (_dir, base, _sistema, arquivo) = montar_base();
    let indice = abrir(&arquivo, &base);

    let todos = achados(&indice, &Consulta::nova("carencia").pagina(100, 0));
    assert!(todos.iter().any(|c| c.starts_with("outro-sistema/")));
    let so_um = achados(
        &indice,
        &Consulta::nova("carencia").sistema("outro-sistema"),
    );
    assert_eq!(so_um, ["outro-sistema/regras/RN-FAT-001.md"]);

    let analises = achados(&indice, &Consulta::nova("multa").tipos(Tipo::ANALISES));
    assert_eq!(analises, [format!("{SISTEMA}/analises/A-0001.md")]);

    let consulta = Consulta::nova("multa juros carencia parcelas").modo(Modo::Qualquer);
    let total = indice.buscar(&consulta).unwrap().total;
    assert!(total > 4, "{total}");
    let mut paginado = Vec::new();
    for pagina in 0.. {
        let p = indice
            .buscar(&consulta.clone().pagina(2, pagina * 2))
            .unwrap();
        assert_eq!(p.total, total);
        if p.resultados.is_empty() {
            break;
        }
        assert!(p.resultados.len() <= 2);
        paginado.extend(p.resultados.into_iter().map(|r| r.caminho.to_string()));
    }
    assert_eq!(paginado, achados(&indice, &consulta.clone().pagina(100, 0)));

    // Todas as palavras é mais restrito que qualquer uma.
    let todas = indice
        .buscar(&Consulta::nova("multa juros carencia parcelas"))
        .unwrap()
        .total;
    assert!(todas < total);

    // Texto vazio ou só pontuação não encontra nada, sem erro.
    assert_eq!(indice.buscar(&Consulta::nova(" ?! ")).unwrap().total, 0);
    // A sintaxe do FTS5 é só texto: nada de erro de consulta.
    indice.buscar(&Consulta::nova("\"* OR (")).unwrap();
    indice.buscar(&Consulta::nova("NEAR(multa juros)")).unwrap();
}

#[test]
fn ligacoes_e_evidencias() {
    let (_dir, base, _sistema, arquivo) = montar_base();
    let indice = abrir(&arquivo, &base);
    let a1 = caminho(&format!("{SISTEMA}/analises/A-0001.md"));

    let ligacoes = indice.ligacoes(&a1).unwrap();
    let regras = gabarito().regras;
    // Cada regra do gabarito, os dois termos e a sugestão apontam para a análise.
    assert_eq!(ligacoes.len(), regras.len() + 3);
    assert!(ligacoes.iter().all(|l| l.para == a1 && l.existe));
    assert!(ligacoes.iter().any(|l| {
        l.de.as_str() == format!("{SISTEMA}/sugestoes/S-0001.md") && l.campo == "analise"
    }));
    assert!(ligacoes.iter().any(|l| {
        l.de.as_str() == format!("{SISTEMA}/regras/RN-FAT-003.md") && l.campo == "origem"
    }));

    // A regra do outro sistema aponta para uma análise que não existe.
    let outra = indice
        .ligacoes(&caminho("outro-sistema/regras/RN-FAT-001.md"))
        .unwrap();
    assert_eq!(outra.len(), 1);
    assert!(!outra[0].existe);

    let evidencias = indice.evidencias(Some(SISTEMA)).unwrap();
    let esperadas: usize = regras.iter().map(|r| r.evidencias.len()).sum();
    assert_eq!(evidencias.len(), esperadas);
    let primeira = &evidencias[0];
    assert_eq!(primeira.regra.as_deref(), Some("RN-FAT-001"));
    assert_eq!(primeira.arquivo, regras[0].evidencias[0].arquivo);
    assert_eq!(
        primeira.linha_inicial,
        regras[0].evidencias[0].linha_inicial
    );
    assert_eq!(primeira.commit.as_deref(), Some("1a2b3c4d"));
    assert!(indice.evidencias(Some("outro-sistema")).unwrap().is_empty());
}

#[test]
fn vigia_reflete_edicao_externa_em_poucos_segundos() {
    let (_dir, base, sistema, arquivo) = montar_base();
    let mudancas = Arc::new(Mutex::new(Vec::new()));
    let anotadas = Arc::clone(&mudancas);
    let vigia = Vigia::iniciar(&arquivo, &base, Duration::from_millis(100), move |s| {
        anotadas.lock().unwrap().push(*s);
    })
    .unwrap();

    let leitor = Indice::abrir(&arquivo, &base).unwrap();
    let esperar = |texto: &str, caminho: String| {
        let inicio = Instant::now();
        loop {
            if achados(&leitor, &Consulta::nova(texto)).contains(&caminho) {
                return inicio.elapsed();
            }
            assert!(
                inicio.elapsed() < Duration::from_secs(10),
                "vigia não viu `{texto}`"
            );
            thread::sleep(Duration::from_millis(50));
        }
    };
    esperar("carencia", format!("{SISTEMA}/regras/RN-FAT-003.md"));

    // Edição como a de um editor externo: grava num temporário e renomeia.
    let destino = sistema.caminho(&caminho("glossario/carencia.md"));
    let temporario = sistema.raiz().join("glossario").join(".carencia.md.tmp");
    let texto = fs::read_to_string(&destino).unwrap();
    fs::write(
        &temporario,
        texto.replace("encargos", "encargos nem bloqueio"),
    )
    .unwrap();
    fs::rename(&temporario, &destino).unwrap();
    let demora = esperar("bloqueio", format!("{SISTEMA}/glossario/carencia.md"));
    assert!(demora < Duration::from_secs(5), "{demora:?}");

    vigia.parar();
    let mudancas = mudancas.lock().unwrap();
    assert!(mudancas.len() >= 2, "{mudancas:?}");
    assert!(mudancas[0].novos > 0);
}

#[test]
fn processos_ao_mesmo_tempo() {
    let (_dir, base, _sistema, arquivo) = montar_base();
    let threads: Vec<_> = (0..4)
        .map(|_| {
            let base = base.clone();
            let arquivo = arquivo.clone();
            thread::spawn(move || {
                let mut indice = Indice::abrir(&arquivo, &base).unwrap();
                for _ in 0..3 {
                    indice.sincronizar().unwrap();
                    indice.buscar(&Consulta::nova("multa")).unwrap();
                }
            })
        })
        .collect();
    for t in threads {
        t.join().unwrap();
    }
    let indice = abrir(&arquivo, &base);
    let mut sozinho = Indice::abrir(arquivo.with_file_name("sozinho.sqlite"), &base).unwrap();
    sozinho.sincronizar().unwrap();
    assert_eq!(fotografia(&indice), fotografia(&sozinho));
}

#[test]
fn indice_corrompido_ou_de_outra_base_recomeca() {
    let (dir, base, _sistema, arquivo) = montar_base();
    let antes = fotografia(&abrir(&arquivo, &base));

    // Arquivo que não é SQLite.
    fs::write(&arquivo, vec![b'x'; 8192]).unwrap();
    let indice = abrir(&arquivo, &base);
    assert_eq!(fotografia(&indice), antes);
    drop(indice);

    // Outra base no mesmo arquivo: o conteúdo da anterior sai.
    let outra = Base::criar(dir.path().join("outra")).unwrap();
    criar_sistema(&outra, "vazio", "Vazio");
    let indice = abrir(&arquivo, &outra);
    assert!(fotografia(&indice).is_empty());
    assert!(achados(&indice, &Consulta::nova("vazio")).len() == 1);
}
