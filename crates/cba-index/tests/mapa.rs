//! Mapa estrutural do sistema fictício "Faturamento Exemplo", conferido contra a
//! própria fixture e o gabarito (`fixtures/gabaritos/faturamento-exemplo.yaml`).

use std::fs;
use std::path::{Path, PathBuf};

use cba_index::{
    Cadastro, Extracao, Extrator, Fonte, Linguagem, Linhas, Mapa, Registro, Sistema,
    TipoReferencia, TipoSimbolo, extrator::sql::ExtratorSql, inventariar, mapear,
};
use serde_norway::Value;

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

fn copiar(origem: &Path, destino: &Path) {
    fs::create_dir_all(destino).unwrap();
    for e in fs::read_dir(origem).unwrap() {
        let e = e.unwrap();
        let alvo = destino.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copiar(&e.path(), &alvo);
        } else {
            fs::copy(e.path(), alvo).unwrap();
        }
    }
}

fn mapa_da_fixture() -> Mapa {
    let raiz = fixtures().join("faturamento-exemplo");
    let sistema = Sistema::novo("Faturamento Exemplo", [&raiz]).unwrap();
    let inv = inventariar(&sistema, None).unwrap();
    mapear(&inv, &sistema.escopo().unwrap(), None, &Registro::padrao())
}

/// Critério de aceite: a procedure de multa e a tabela de faturas aparecem com
/// arquivo e linhas corretos.
#[test]
fn multa_e_faturas_com_arquivo_e_linhas() {
    let mapa = mapa_da_fixture();
    assert_eq!(mapa.falhas().count(), 0);

    let multa = mapa.buscar("PKG_COBRANCA.FN_CALCULAR_MULTA");
    let resumo: Vec<_> = multa
        .iter()
        .map(|a| (a.arquivo, a.simbolo.linhas, a.simbolo.declaracao))
        .collect();
    assert_eq!(
        resumo,
        [
            ("db/03_pkg_cobranca.sql", Linhas::new(13, 14), true),
            ("db/03_pkg_cobranca.sql", Linhas::new(50, 73), false),
        ]
    );
    assert_eq!(multa[1].simbolo.tipo, TipoSimbolo::Funcao);
    assert_eq!(multa[1].simbolo.pai.as_deref(), Some("PKG_COBRANCA"));
    // O nome simples também acha.
    assert_eq!(mapa.buscar("fn_calcular_multa").len(), 2);

    let fatura = mapa.buscar("FATURA");
    let tabela = fatura
        .iter()
        .find(|a| a.simbolo.tipo == TipoSimbolo::Tabela)
        .unwrap();
    assert_eq!(tabela.arquivo, "db/01_tabelas.sql");
    assert_eq!(tabela.simbolo.linhas, Linhas::new(20, 37));
    let colunas: Vec<_> = mapa
        .filhos(tabela.id)
        .iter()
        .map(|c| c.simbolo.nome.clone())
        .collect();
    assert_eq!(colunas.len(), 10);
    assert_eq!(colunas[0], "ID_FATURA");
    let status = mapa.buscar("FATURA.STATUS");
    assert_eq!(status.len(), 1);
    assert_eq!(status[0].simbolo.linhas, Linhas::new(28, 28));
    assert_eq!(
        status[0].simbolo.comentario.as_deref(),
        Some("ABERTA, PAGA, PARCELADA, CANCELADA")
    );
    // A chave estrangeira de FATURA para CLIENTE.
    assert!(mapa.referencias_de(tabela.id).iter().any(|u| {
        u.referencia.tipo == TipoReferencia::ChaveEstrangeira
            && u.referencia.alvo == "CLIENTE"
            && u.referencia.linha == 34
    }));

    // Schema: tabelas e sequência do DDL.
    let schema: Vec<_> = mapa
        .schema()
        .iter()
        .map(|a| a.simbolo.qualificado.clone())
        .collect();
    assert_eq!(
        schema,
        [
            "CLIENTE",
            "FATURA",
            "LOG_FATURA",
            "PARAMETRO",
            "PARCELA",
            "SQ_LOG_FATURA"
        ]
    );
}

/// As evidências de banco do gabarito caem dentro de um símbolo do mapa.
#[test]
fn evidencias_do_gabarito_caem_em_simbolos() {
    let mapa = mapa_da_fixture();
    let texto = fs::read_to_string(fixtures().join("gabaritos/faturamento-exemplo.yaml")).unwrap();
    let g: Value = serde_norway::from_str(&texto).unwrap();
    let mut conferidas = 0;
    for regra in g["regras"].as_sequence().unwrap() {
        for ev in regra["evidencias"].as_sequence().unwrap() {
            let arquivo = ev["arquivo"].as_str().unwrap();
            // O script de carga não define símbolos (só INSERTs).
            if !arquivo.starts_with("db/") || arquivo == "db/02_parametros.sql" {
                continue;
            }
            let ini = ev["linha_inicial"].as_u64().unwrap() as u32;
            let fim = ev["linha_final"].as_u64().unwrap() as u32;
            let simbolos = &mapa.arquivo(arquivo).unwrap().extracao.simbolos;
            assert!(
                simbolos
                    .iter()
                    .any(|s| !s.declaracao && s.linhas.contem(ini) && s.linhas.contem(fim)),
                "{arquivo}:{ini}-{fim} ({}) fora dos símbolos",
                regra["id"].as_str().unwrap()
            );
            conferidas += 1;
        }
    }
    assert!(conferidas >= 10, "{conferidas}");
}

/// Critério de aceite: as referências de quem chama a procedure aparecem no mapa.
#[test]
fn quem_chama_as_rotinas_do_pacote() {
    let mapa = mapa_da_fixture();
    let corpo = |q: &str| {
        mapa.buscar(q)
            .into_iter()
            .find(|a| !a.simbolo.declaracao)
            .unwrap()
    };
    let chamadas = |q: &str| -> Vec<(String, u32, TipoReferencia, Option<String>)> {
        mapa.referencias_a(corpo(q).id)
            .into_iter()
            .map(|u| {
                (
                    u.arquivo.to_string(),
                    u.referencia.linha,
                    u.referencia.tipo,
                    u.origem.map(|o| o.simbolo.qualificado.clone()),
                )
            })
            .collect()
    };

    // O backend chama a função de multa pelo nome, numa string.
    assert_eq!(
        chamadas("PKG_COBRANCA.FN_CALCULAR_MULTA"),
        [(
            "backend/src/main/java/exemplo/faturamento/EncargosService.java".into(),
            28,
            TipoReferencia::Mencao,
            Some("EncargosService".into())
        )]
    );
    // A tela de cancelamento chama a procedure de cancelamento.
    assert!(
        chamadas("PKG_COBRANCA.PRC_CANCELAR_FATURA")
            .iter()
            .any(|(a, l, _, _)| a == "delphi/UFrmCancelamento.pas" && *l == 94)
    );
    // Dentro do pacote: a multa chama FN_PARAMETRO e FN_DIAS_ATRASO, e as chamadas
    // vão para o corpo, não para a especificação.
    let multa = "PKG_COBRANCA.FN_CALCULAR_MULTA".to_string();
    let parametro = chamadas("PKG_COBRANCA.FN_PARAMETRO");
    assert!(parametro.contains(&(
        "db/03_pkg_cobranca.sql".into(),
        67,
        TipoReferencia::Chamada,
        Some(multa.clone())
    )));
    assert!(parametro.contains(&(
        "db/03_pkg_cobranca.sql".into(),
        72,
        TipoReferencia::Chamada,
        Some(multa.clone())
    )));
    assert!(chamadas("PKG_COBRANCA.FN_DIAS_ATRASO").contains(&(
        "db/03_pkg_cobranca.sql".into(),
        64,
        TipoReferencia::Chamada,
        Some(multa)
    )));

    // O que a procedure de cancelamento usa.
    let usa: Vec<_> = mapa
        .referencias_de(corpo("PKG_COBRANCA.PRC_CANCELAR_FATURA").id)
        .into_iter()
        .map(|u| {
            (
                u.referencia.alvo.clone(),
                u.referencia.tipo,
                u.referencia.linha,
            )
        })
        .collect();
    assert_eq!(
        usa,
        [
            ("FATURA.STATUS".to_string(), TipoReferencia::Tipo, 102),
            ("FATURA.VL_PAGO".to_string(), TipoReferencia::Tipo, 103),
            ("FATURA".to_string(), TipoReferencia::Leitura, 107),
            ("FATURA".to_string(), TipoReferencia::Escrita, 123),
            ("LOG_FATURA".to_string(), TipoReferencia::Escrita, 130),
            ("SQ_LOG_FATURA".to_string(), TipoReferencia::Uso, 131),
        ]
    );

    // Onde a tabela de parâmetros é usada (ferramenta `referencias`).
    let usos: Vec<_> = mapa
        .usos("parametro")
        .into_iter()
        .filter(|u| u.referencia.tipo != TipoReferencia::Mencao)
        .map(|u| (u.arquivo, u.referencia.linha))
        .collect();
    assert_eq!(
        usos,
        [
            ("db/02_parametros.sql", 6),
            ("db/02_parametros.sql", 9),
            ("db/02_parametros.sql", 12),
            ("db/02_parametros.sql", 15),
            ("db/02_parametros.sql", 18),
            ("db/03_pkg_cobranca.sql", 33),
        ]
    );
}

#[test]
fn modulos_por_pasta() {
    let mapa = mapa_da_fixture();
    let raiz = mapa.modulo("").unwrap();
    assert_eq!(raiz.arquivos, mapa.arquivos().len());
    assert_eq!(
        raiz.subpastas,
        ["backend", "cobol", "db", "delphi", "frontend"]
    );
    assert!(raiz.arquivos_diretos.is_empty());
    let dep = |de: &str, para: &str| {
        raiz.dependencias
            .iter()
            .find(|d| d.de == de && d.para == para)
            .map(|d| d.referencias)
    };
    assert!(dep("backend", "db").is_some_and(|n| n >= 2));
    assert!(dep("delphi", "db").is_some());
    assert_eq!(dep("db", "backend"), None);

    let db = mapa.modulo("db/").unwrap();
    assert_eq!(db.arquivos, 3);
    assert_eq!(db.linguagens[&Linguagem::Sql], 3);
    assert_eq!(db.simbolos[&TipoSimbolo::Tabela], 5);
    assert_eq!(db.simbolos[&TipoSimbolo::Pacote], 2);
    assert_eq!(db.arquivos_diretos.len(), 3);
    assert!(db.subpastas.is_empty());
    assert!(mapa.modulo("nao-existe").is_none());

    let backend = mapa.modulo("backend").unwrap();
    assert_eq!(backend.subpastas, ["backend/src"]);
}

struct Falha;
impl Extrator for Falha {
    fn nome(&self) -> &'static str {
        "falha"
    }
    fn versao(&self) -> u32 {
        1
    }
    fn linguagens(&self) -> &'static [Linguagem] {
        &[Linguagem::Java]
    }
    fn extrair(&self, fonte: &Fonte<'_>) -> Result<Extracao, String> {
        if fonte.caminho.ends_with("Fatura.java") {
            Err("sintaxe não reconhecida".into())
        } else {
            Ok(Extracao::default())
        }
    }
}

struct Panico;
impl Extrator for Panico {
    fn nome(&self) -> &'static str {
        "panico"
    }
    fn versao(&self) -> u32 {
        1
    }
    fn linguagens(&self) -> &'static [Linguagem] {
        &[Linguagem::Delphi]
    }
    fn extrair(&self, _: &Fonte<'_>) -> Result<Extracao, String> {
        panic!("extrator quebrado")
    }
}

/// Critério de aceite: extrator que falha num arquivo registra o erro e segue.
#[test]
fn falha_do_extrator_fica_no_arquivo_e_o_resto_segue() {
    let raiz = fixtures().join("faturamento-exemplo");
    let sistema = Sistema::novo("Faturamento Exemplo", [&raiz]).unwrap();
    let inv = inventariar(&sistema, None).unwrap();
    let mut registro = Registro::padrao();
    registro.registrar(Falha).registrar(Panico);
    let mapa = mapear(&inv, &sistema.escopo().unwrap(), None, &registro);

    let falhas: Vec<_> = mapa
        .falhas()
        .map(|a| (a.caminho.as_str(), a.falha.clone().unwrap()))
        .collect();
    assert_eq!(falhas.len(), 3, "{falhas:?}");
    assert_eq!(
        falhas[0],
        (
            "backend/src/main/java/exemplo/faturamento/Fatura.java",
            "sintaxe não reconhecida".to_string()
        )
    );
    assert!(falhas[1].0.starts_with("delphi/"));
    assert!(falhas[1].1.contains("extrator quebrado"), "{}", falhas[1].1);
    assert_eq!(mapa.estatisticas.falhas, 3);
    assert_eq!(mapa.modulo("delphi").unwrap().falhas, 2);
    // Os demais arquivos foram mapeados.
    assert_eq!(
        mapa.buscar("PKG_COBRANCA.FN_CALCULAR_MULTA").len(),
        2,
        "o SQL continua mapeado"
    );
    assert_eq!(mapa.arquivos().len(), inv.arquivos.len());
}

/// Extrator de SQL numa versão nova, para conferir a invalidação do cache.
struct SqlV2;
impl Extrator for SqlV2 {
    fn nome(&self) -> &'static str {
        "sql"
    }
    fn versao(&self) -> u32 {
        2
    }
    fn linguagens(&self) -> &'static [Linguagem] {
        &[Linguagem::Sql]
    }
    fn extrair(&self, fonte: &Fonte<'_>) -> Result<Extracao, String> {
        ExtratorSql.extrair(fonte)
    }
}

#[test]
fn mapa_gravado_e_incremental() {
    let tmp = tempfile::tempdir().unwrap();
    let raiz = tmp.path().join("faturamento");
    copiar(&fixtures().join("faturamento-exemplo"), &raiz);
    let cadastro = Cadastro::em(tmp.path().join("dados/sistemas"));
    let sistema = Sistema::novo("Faturamento Exemplo", [&raiz]).unwrap();
    cadastro.cadastrar(&sistema).unwrap();
    assert!(cadastro.mapa(&sistema.id).unwrap().is_none());

    let primeiro = cadastro.mapear(&sistema.id).unwrap();
    let n = primeiro.arquivos().len();
    assert_eq!(primeiro.estatisticas.processados, n);
    assert_eq!(primeiro.estatisticas.reaproveitados, 0);
    assert!(
        tmp.path()
            .join("dados/sistemas/faturamento-exemplo/mapa.json")
            .is_file()
    );
    // O gravado volta igual e com as referências resolvidas.
    let lido = cadastro.mapa(&sistema.id).unwrap().unwrap();
    assert_eq!(lido, primeiro);
    let multa = lido
        .buscar("FN_CALCULAR_MULTA")
        .into_iter()
        .find(|a| !a.simbolo.declaracao)
        .unwrap();
    assert_eq!(lido.referencias_a(multa.id).len(), 1);

    // Sem mudanças, nada é extraído de novo.
    let segundo = cadastro.mapear(&sistema.id).unwrap();
    assert_eq!(segundo.estatisticas.processados, 0);
    assert_eq!(segundo.estatisticas.reaproveitados, n);
    assert_eq!(segundo.arquivos(), primeiro.arquivos());

    // Um arquivo alterado é extraído de novo; as ligações acompanham.
    let alvo = raiz.join("db/03_pkg_cobranca.sql");
    let texto = fs::read_to_string(&alvo).unwrap();
    fs::write(&alvo, format!("-- linha nova\n{texto}")).unwrap();
    let terceiro = cadastro.mapear(&sistema.id).unwrap();
    assert_eq!(terceiro.estatisticas.processados, 1);
    let multa = terceiro
        .buscar("FN_CALCULAR_MULTA")
        .into_iter()
        .find(|a| !a.simbolo.declaracao)
        .unwrap();
    assert_eq!(multa.simbolo.linhas, Linhas::new(51, 74));
    assert_eq!(terceiro.referencias_a(multa.id).len(), 1);

    // Versão nova de um extrator reprocessa só os arquivos dele.
    let mut registro = Registro::padrao();
    registro.registrar(SqlV2);
    let quarto = cadastro.mapear_com(&sistema.id, &registro).unwrap();
    assert_eq!(quarto.estatisticas.processados, 3);
    assert_eq!(quarto.estatisticas.reaproveitados, n - 3);

    // Mapa ilegível só faz perder o cache.
    fs::write(
        tmp.path()
            .join("dados/sistemas/faturamento-exemplo/mapa.json"),
        b"{",
    )
    .unwrap();
    assert!(cadastro.mapa(&sistema.id).is_err());
    let quinto = cadastro.mapear(&sistema.id).unwrap();
    assert_eq!(quinto.estatisticas.processados, n);

    assert!(cadastro.remover(&sistema.id).unwrap());
    assert!(cadastro.mapa(&sistema.id).unwrap().is_none());
}
