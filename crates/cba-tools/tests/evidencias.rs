//! Validação de evidências com o sistema fictício "Faturamento Exemplo" e o
//! gabarito dele: as evidências do gabarito valem, e as inválidas são recusadas
//! com o motivo.

use std::path::{Path, PathBuf};

use cba_index::{Escopo, Motivo as MotivoDaExclusao, Recusa, Sistema};
use cba_kb::frontmatter::Linhas;
use cba_tools::evidencias::{EvidenciaCitada, Motivo, validar, validar_todas};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/faturamento-exemplo")
}

fn gabarito() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/gabaritos/faturamento-exemplo.yaml")
}

fn escopo() -> Escopo {
    Sistema::novo("Faturamento Exemplo", [fixture()])
        .unwrap()
        .escopo()
        .unwrap()
}

fn citada(arquivo: &str, i: u32, f: u32, trecho: &str) -> EvidenciaCitada {
    EvidenciaCitada {
        arquivo: arquivo.into(),
        linha_inicial: i,
        linha_final: f,
        trecho: trecho.into(),
    }
}

fn motivo(e: &EvidenciaCitada) -> Motivo {
    validar(&escopo(), e).unwrap_err().motivo
}

/// Toda evidência do gabarito (`arquivo`, `linha_inicial`, `linha_final`,
/// `trecho`), em qualquer seção.
fn evidencias_do_gabarito() -> Vec<EvidenciaCitada> {
    fn coletar(valor: &serde_norway::Value, saida: &mut Vec<EvidenciaCitada>) {
        match valor {
            serde_norway::Value::Mapping(m) => {
                if m.contains_key("linha_inicial") && m.contains_key("trecho") {
                    saida.push(serde_norway::from_value(valor.clone()).unwrap());
                    return;
                }
                for v in m.values() {
                    coletar(v, saida);
                }
            }
            serde_norway::Value::Sequence(s) => s.iter().for_each(|v| coletar(v, saida)),
            _ => {}
        }
    }
    let texto = std::fs::read_to_string(gabarito()).unwrap();
    let valor: serde_norway::Value = serde_norway::from_str(&texto).unwrap();
    let mut saida = Vec::new();
    coletar(&valor, &mut saida);
    saida
}

#[test]
fn evidencias_do_gabarito_sao_aceitas() {
    let escopo = escopo();
    let citadas = evidencias_do_gabarito();
    assert!(citadas.len() >= 40, "{}", citadas.len());
    for c in &citadas {
        let e = validar(&escopo, c).unwrap_or_else(|r| panic!("{r}"));
        assert_eq!(e.arquivo.as_str(), c.arquivo);
        assert_eq!(
            e.linhas,
            Linhas::novo(c.linha_inicial, c.linha_final).unwrap()
        );
        assert_eq!(e.hash.len(), 64);
        assert!(e.hash.bytes().all(|b| b.is_ascii_hexdigit()));
        assert_eq!(e.trecho, c.trecho.trim());
    }
    assert!(validar_todas(&escopo, &citadas).is_ok());
}

#[test]
fn trecho_copiado_com_outra_indentacao_e_linhas_em_branco_confere() {
    // Linhas 66 a 69 de db/03_pkg_cobranca.sql, reindentadas e com espaços repetidos.
    let trecho = "IF V_DIAS   <= FN_PARAMETRO('DIAS_CARENCIA') THEN\n\n  RETURN 0;\nEND IF;";
    let e = validar(&escopo(), &citada("db/03_pkg_cobranca.sql", 66, 69, trecho));
    assert!(e.is_ok(), "{e:?}");
    // Separador do Windows também vale; o caminho guardado usa /.
    let e = validar(
        &escopo(),
        &citada("db\\02_parametros.sql", 6, 7, "'PERC_MULTA_ATRASO', 2.0000"),
    )
    .unwrap();
    assert_eq!(e.arquivo.as_str(), "db/02_parametros.sql");
}

#[test]
fn trecho_que_nao_confere_e_recusado_com_o_conteudo_atual() {
    let c = citada("db/02_parametros.sql", 6, 7, "'PERC_MULTA_ATRASO', 3.0000");
    let r = validar(&escopo(), &c).unwrap_err();
    assert!(matches!(&r.motivo, Motivo::NaoConfere { encontrado, .. } if encontrado.is_empty()));
    let texto = r.to_string();
    assert!(
        texto.starts_with("`db/02_parametros.sql`, linhas 6-7:"),
        "{texto}"
    );
    assert!(texto.contains("não aparece nessas linhas"), "{texto}");
    // Mostra o conteúdo atual, para o agente corrigir.
    assert!(
        texto.contains("VALUES ('PERC_MULTA_ATRASO', 2.0000"),
        "{texto}"
    );
}

#[test]
fn trecho_em_outras_linhas_e_recusado_com_as_linhas_certas() {
    let c = citada("db/02_parametros.sql", 9, 10, "'PERC_MULTA_ATRASO', 2.0000");
    let texto = validar(&escopo(), &c).unwrap_err().to_string();
    assert!(texto.contains("aparece nas linhas 7-7"), "{texto}");
    assert!(texto.contains("linha_inicial 7 e linha_final 7"), "{texto}");

    // Copiado com os números de linha da leitura do arquivo.
    let c = citada(
        "db/02_parametros.sql",
        6,
        7,
        "     6→INSERT INTO PARAMETRO (CHAVE, VALOR, DESCRICAO)\n     7→VALUES ('PERC_MULTA_ATRASO', 2.0000",
    );
    let texto = validar(&escopo(), &c).unwrap_err().to_string();
    assert!(texto.contains("números de linha"), "{texto}");
}

#[test]
fn intervalo_invalido_ou_fora_do_arquivo_e_recusado() {
    let arquivo = "db/02_parametros.sql";
    assert!(matches!(
        motivo(&citada(arquivo, 9000, 9001, "x")),
        Motivo::ForaDoArquivo { total } if total > 10
    ));
    let texto = validar(&escopo(), &citada(arquivo, 9000, 9001, "x"))
        .unwrap_err()
        .to_string();
    assert!(texto.contains("passa do fim do arquivo"), "{texto}");
    assert_eq!(
        motivo(&citada(arquivo, 7, 6, "x")),
        Motivo::IntervaloInvalido
    );
    assert_eq!(
        motivo(&citada(arquivo, 0, 1, "x")),
        Motivo::IntervaloInvalido
    );
    assert_eq!(
        motivo(&citada(arquivo, 1, 1000, "x")),
        Motivo::IntervaloGrande
    );
    assert_eq!(motivo(&citada(arquivo, 1, 1, "  \n ")), Motivo::TrechoVazio);
}

#[test]
fn arquivo_excluido_fora_do_sistema_ou_inexistente_e_recusado() {
    // Segredo, do gabarito (exclusoes).
    let m = motivo(&citada(".env", 1, 1, "Arquivo FALSO"));
    assert!(
        matches!(&m, Motivo::Caminho(Recusa::Excluido(e)) if e.motivo == MotivoDaExclusao::Segredo),
        "{m:?}"
    );
    assert!(m.to_string().contains("excluído da análise"), "{m}");

    // O gabarito fica fora da pasta do sistema.
    for caminho in [
        "../gabaritos/faturamento-exemplo.yaml",
        "/etc/passwd",
        "C:/Windows/win.ini",
    ] {
        let m = motivo(&citada(caminho, 1, 1, "x"));
        assert!(
            matches!(m, Motivo::Caminho(Recusa::CaminhoInvalido(_))),
            "{caminho}: {m:?}"
        );
        assert!(m.to_string().contains("relativo à raiz do sistema"));
    }
    let m = motivo(&citada("db/99_inexistente.sql", 1, 1, "x"));
    assert_eq!(m, Motivo::Caminho(Recusa::NaoEncontrado));
    let m = motivo(&citada("db", 1, 1, "x"));
    assert_eq!(m, Motivo::Caminho(Recusa::NaoEhArquivo));
}

#[test]
fn todas_as_recusas_vem_numa_mensagem_so() {
    let citadas = [
        citada("db/02_parametros.sql", 6, 7, "'PERC_MULTA_ATRASO', 2.0000"),
        citada(".env", 1, 1, "x"),
        citada("db/02_parametros.sql", 9000, 9000, "x"),
    ];
    let texto = validar_todas(&escopo(), &citadas).unwrap_err();
    assert!(texto.starts_with("2 de 3 evidências recusadas"), "{texto}");
    assert!(texto.contains("evidências[1] (`.env`"), "{texto}");
    assert!(
        texto.contains("evidências[2] (`db/02_parametros.sql`"),
        "{texto}"
    );
    assert!(!texto.contains("evidências[0]"), "{texto}");
}

#[cfg(unix)]
#[test]
fn link_simbolico_que_escapa_e_recusado() {
    use std::os::unix::fs::symlink;
    let dir = tempfile::tempdir().unwrap();
    let sistema = dir.path().join("sistema");
    let fora = dir.path().join("fora");
    std::fs::create_dir_all(sistema.join("db")).unwrap();
    std::fs::create_dir_all(&fora).unwrap();
    std::fs::write(sistema.join("db/regra.sql"), "SELECT 1 FROM DUAL;\n").unwrap();
    std::fs::write(fora.join("segredo.sql"), "SELECT SENHA FROM USUARIO;\n").unwrap();
    // Pasta que leva para fora do sistema e arquivo que é link.
    symlink(&fora, sistema.join("db/escapa")).unwrap();
    symlink(fora.join("segredo.sql"), sistema.join("db/atalho.sql")).unwrap();
    let escopo = Sistema::novo("Com links", [&sistema])
        .unwrap()
        .escopo()
        .unwrap();

    assert!(validar(&escopo, &citada("db/regra.sql", 1, 1, "SELECT 1")).is_ok());
    let r = validar(
        &escopo,
        &citada("db/escapa/segredo.sql", 1, 1, "SELECT SENHA"),
    )
    .unwrap_err();
    assert_eq!(r.motivo, Motivo::Caminho(Recusa::ForaDoSistema), "{r}");
    assert!(r.to_string().contains("fora das pastas do sistema"));
    let r = validar(&escopo, &citada("db/atalho.sql", 1, 1, "SELECT SENHA")).unwrap_err();
    assert!(
        matches!(&r.motivo, Motivo::Caminho(Recusa::Excluido(e)) if e.motivo == MotivoDaExclusao::LinkSimbolico),
        "{r}"
    );
}

#[test]
fn arquivo_binario_e_recusado_e_windows_1252_e_lido() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("dados.txt"), b"abc\0def\n").unwrap();
    // "Cobrança" em Windows-1252, como em fontes Delphi antigos.
    std::fs::write(
        dir.path().join("UCobranca.pas"),
        b"// Cobran\xE7a\nx := 1;\n",
    )
    .unwrap();
    let escopo = Sistema::novo("Legado", [dir.path()])
        .unwrap()
        .escopo()
        .unwrap();
    let r = validar(&escopo, &citada("dados.txt", 1, 1, "abc")).unwrap_err();
    assert_eq!(r.motivo, Motivo::Binario);
    let e = validar(&escopo, &citada("UCobranca.pas", 1, 1, "// Cobrança")).unwrap();
    assert_eq!(e.trecho, "// Cobrança");
}
