//! Cadastro e inventário do sistema fictício "Faturamento Exemplo", conferidos
//! contra o gabarito (`fixtures/gabaritos/faturamento-exemplo.yaml`).

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use cba_index::{Cadastro, Erro, Linguagem, Motivo, Recusa, Sistema, inventariar};
use serde_norway::Value;

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

fn gabarito() -> Value {
    let texto = fs::read_to_string(fixtures().join("gabaritos/faturamento-exemplo.yaml")).unwrap();
    serde_norway::from_str(&texto).unwrap()
}

/// Exclusões do gabarito: (arquivo, motivo).
fn exclusoes_do_gabarito() -> Vec<(String, String)> {
    gabarito()["exclusoes"]
        .as_sequence()
        .unwrap()
        .iter()
        .map(|e| {
            (
                e["arquivo"].as_str().unwrap().to_string(),
                e["motivo"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

/// Todo arquivo citado no gabarito (evidências e arquivos impactados).
fn arquivos_citados() -> BTreeSet<String> {
    fn coletar(v: &Value, chave: Option<&str>, saida: &mut BTreeSet<String>) {
        match v {
            Value::Mapping(m) => {
                for (k, v) in m {
                    coletar(v, k.as_str(), saida);
                }
            }
            Value::Sequence(s) => s.iter().for_each(|x| coletar(x, chave, saida)),
            Value::String(s) if matches!(chave, Some("arquivo" | "arquivos_impactados")) => {
                saida.insert(s.clone());
            }
            _ => {}
        }
    }
    let mut saida = BTreeSet::new();
    let g = gabarito();
    for secao in [
        "regras",
        "inconsistencias",
        "perguntas",
        "funcionalidades",
        "demandas",
    ] {
        coletar(&g[secao], None, &mut saida);
    }
    saida
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

fn escrever(caminho: &Path, conteudo: &[u8]) {
    fs::create_dir_all(caminho.parent().unwrap()).unwrap();
    fs::write(caminho, conteudo).unwrap();
}

/// Cópia da fixture com o que um repositório real costuma ter a mais: pastas de
/// dependências, saída de build, um binário sem extensão conhecida, outro segredo
/// e um `.git`.
fn fixture_com_ruido() -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let raiz = tmp.path().join("faturamento exemplo");
    copiar(&fixtures().join("faturamento-exemplo"), &raiz);
    escrever(
        &raiz.join("frontend/node_modules/lib/index.js"),
        b"module.exports = 1;",
    );
    escrever(&raiz.join("frontend/node_modules/lib/package.json"), b"{}");
    escrever(&raiz.join("backend/vendor/x.php"), b"<?php");
    escrever(
        &raiz.join("backend/target/classes/Fatura.class"),
        b"\xCA\xFE\xBA\xBE",
    );
    escrever(&raiz.join("delphi/UFrmCancelamento.dcu"), b"\x00\x01");
    escrever(&raiz.join("db/dump.dat"), b"DADOS\x00\x00\x01\x02");
    escrever(&raiz.join("backend/.env.local"), b"SENHA=ficticia");
    escrever(&raiz.join(".git/HEAD"), b"ref: refs/heads/main\n");
    escrever(
        &raiz.join(".git/refs/heads/main"),
        b"0123456789abcdef0123456789abcdef01234567\n",
    );
    (tmp, raiz)
}

#[test]
fn inventario_da_fixture_ignora_segredos_e_dependencias() {
    let (_tmp, raiz) = fixture_com_ruido();
    let sistema = Sistema::novo("Faturamento Exemplo", [&raiz]).unwrap();
    assert_eq!(sistema.id, gabarito()["sistema"].as_str().unwrap());

    let inv = inventariar(&sistema, None).unwrap();
    let analisados: BTreeSet<_> = inv.arquivos.iter().map(|a| a.caminho.as_str()).collect();

    // Exclusões do gabarito.
    for (arquivo, motivo) in exclusoes_do_gabarito() {
        let i = inv
            .ignorado(&arquivo)
            .unwrap_or_else(|| panic!("{arquivo} não foi ignorado"));
        assert_eq!(serde_json::to_value(i.motivo).unwrap(), motivo.as_str());
        assert!(!analisados.contains(arquivo.as_str()));
    }
    // Ruído acrescentado.
    let motivo = |c: &str| inv.ignorado(c).map(|i| i.motivo);
    assert_eq!(motivo("frontend/node_modules"), Some(Motivo::Dependencia));
    assert!(inv.ignorado("frontend/node_modules").unwrap().pasta);
    assert_eq!(
        motivo("frontend/node_modules/lib/index.js"),
        Some(Motivo::Dependencia)
    );
    assert_eq!(motivo("backend/vendor"), Some(Motivo::Dependencia));
    assert_eq!(motivo("backend/target"), Some(Motivo::SaidaBuild));
    assert_eq!(
        motivo("delphi/UFrmCancelamento.dcu"),
        Some(Motivo::SaidaBuild)
    );
    assert_eq!(motivo("db/dump.dat"), Some(Motivo::Binario));
    assert_eq!(motivo("backend/.env.local"), Some(Motivo::Segredo));
    assert_eq!(motivo(".git"), Some(Motivo::ControleVersao));
    // O conteúdo das pastas ignoradas não é listado, nem lido.
    assert!(
        !inv.ignorados
            .iter()
            .any(|i| i.caminho.contains("node_modules/"))
    );

    // Tudo o que o gabarito cita entra na análise.
    for a in arquivos_citados() {
        assert!(analisados.contains(a.as_str()), "{a} deveria ser analisado");
    }
    // E só os arquivos da fixture original (sem o .env) entram.
    let mut esperados = BTreeSet::new();
    let mut pendentes = vec![fixtures().join("faturamento-exemplo")];
    let base = pendentes[0].clone();
    while let Some(d) = pendentes.pop() {
        for e in fs::read_dir(d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                pendentes.push(p);
            } else {
                let rel = p.strip_prefix(&base).unwrap();
                let rel: Vec<_> = rel
                    .components()
                    .map(|c| c.as_os_str().to_str().unwrap())
                    .collect();
                esperados.insert(rel.join("/"));
            }
        }
    }
    esperados.remove(".env");
    assert_eq!(analisados, esperados.iter().map(String::as_str).collect());

    // Linguagem e hash.
    let lang = |c: &str| inv.arquivo(c).unwrap().linguagem;
    assert_eq!(lang("db/03_pkg_cobranca.sql"), Linguagem::Sql);
    assert_eq!(lang("cobol/CALCOBR.cbl"), Linguagem::Cobol);
    assert_eq!(lang("delphi/UFrmCancelamento.dfm"), Linguagem::Delphi);
    assert_eq!(
        lang("frontend/src/telas/parcelamento-form.ts"),
        Linguagem::TypeScript
    );
    assert_eq!(
        lang("backend/src/main/java/exemplo/faturamento/Fatura.java"),
        Linguagem::Java
    );
    for a in &inv.arquivos {
        assert_eq!(a.hash.len(), 64);
        assert!(
            a.hash
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        );
    }
    let vazio = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    assert!(
        inv.arquivos
            .iter()
            .all(|a| a.tamanho == 0 || a.hash != vazio)
    );

    // Git.
    let git = inv.pastas[0].git.as_ref().unwrap();
    assert_eq!(git.branch.as_deref(), Some("main"));
    assert_eq!(
        git.commit.as_deref(),
        Some("0123456789abcdef0123456789abcdef01234567")
    );

    // Resumo.
    let r = inv.resumo();
    assert_eq!(r.analisados, esperados.len());
    assert_eq!(r.por_linguagem[&Linguagem::Java], 6);
    assert_eq!(r.por_linguagem[&Linguagem::Sql], 3);
    assert_eq!(r.ignorados_por_motivo[&Motivo::Segredo], 2);
    assert_eq!(r.ignorados, inv.ignorados.len());
    assert_eq!(r.por_linguagem.values().sum::<usize>(), r.analisados);
}

#[test]
fn escopo_aceita_evidencias_do_gabarito_e_recusa_exclusoes() {
    let raiz = fixtures().join("faturamento-exemplo");
    let sistema = Sistema::novo("Faturamento Exemplo", [&raiz]).unwrap();
    let escopo = sistema.escopo().unwrap();
    for a in arquivos_citados() {
        let p = escopo.resolver(&a).unwrap_or_else(|e| panic!("{a}: {e}"));
        assert!(p.is_file());
        assert_eq!(escopo.relativo(&p).as_deref(), Some(a.as_str()));
    }
    for (arquivo, _) in exclusoes_do_gabarito() {
        assert!(matches!(
            escopo.resolver(&arquivo),
            Err(Recusa::Excluido(_))
        ));
    }
}

#[test]
fn reinventariar_sem_mudancas_nao_reprocessa() {
    let (_tmp, raiz) = fixture_com_ruido();
    let sistema = Sistema::novo("Faturamento Exemplo", [&raiz]).unwrap();
    let primeiro = inventariar(&sistema, None).unwrap();
    assert_eq!(primeiro.estatisticas.reaproveitados, 0);
    // Arquivos analisados mais o binário detectado pelo conteúdo.
    assert_eq!(
        primeiro.estatisticas.processados,
        primeiro.arquivos.len() + 1
    );

    let segundo = inventariar(&sistema, Some(&primeiro)).unwrap();
    assert_eq!(segundo.estatisticas.processados, 0);
    assert_eq!(
        segundo.estatisticas.reaproveitados,
        primeiro.arquivos.len() + 1
    );
    assert_eq!(segundo.arquivos, primeiro.arquivos);
    assert_eq!(segundo.ignorados, primeiro.ignorados);

    // Uma alteração reprocessa só o arquivo alterado.
    let alvo = raiz.join("db/02_parametros.sql");
    let mut conteudo = fs::read(&alvo).unwrap();
    conteudo.extend_from_slice(b"\n-- alterado\n");
    fs::write(&alvo, conteudo).unwrap();
    escrever(&raiz.join("db/04_novo.sql"), b"select 1 from dual;");
    let terceiro = inventariar(&sistema, Some(&segundo)).unwrap();
    assert_eq!(terceiro.estatisticas.processados, 2);
    assert_ne!(
        terceiro.arquivo("db/02_parametros.sql").unwrap().hash,
        segundo.arquivo("db/02_parametros.sql").unwrap().hash
    );
    assert!(terceiro.arquivo("db/04_novo.sql").is_some());
}

#[test]
fn cadastro_grava_le_e_remove() {
    let (tmp, raiz) = fixture_com_ruido();
    let cadastro = Cadastro::em(tmp.path().join("dados/sistemas"));
    assert!(cadastro.listar().unwrap().is_empty());

    let mut sistema = Sistema::novo("Faturamento Exemplo", [&raiz]).unwrap();
    sistema.exclusoes.push("cobol/".into());
    sistema.inclusoes.push("backend/vendor".into());
    cadastro.cadastrar(&sistema).unwrap();
    assert!(matches!(
        cadastro.cadastrar(&sistema),
        Err(Erro::JaCadastrado(_))
    ));
    assert_eq!(
        cadastro.obter(&sistema.id).unwrap().as_ref(),
        Some(&sistema)
    );
    assert_eq!(cadastro.listar().unwrap(), vec![sistema.clone()]);

    let inv = cadastro.inventariar(&sistema.id).unwrap();
    assert_eq!(inv.ignorado("cobol").unwrap().motivo, Motivo::Usuario);
    assert!(inv.arquivo("backend/vendor/x.php").is_some());
    assert_eq!(
        cadastro.inventario(&sistema.id).unwrap().as_ref(),
        Some(&inv)
    );
    let de_novo = cadastro.inventariar(&sistema.id).unwrap();
    assert_eq!(de_novo.estatisticas.processados, 0);

    // Identificadores fora do formato não viram caminho.
    assert!(matches!(cadastro.obter("../x"), Err(Erro::IdInvalido(_))));
    assert!(matches!(
        cadastro.inventariar("outro"),
        Err(Erro::NaoCadastrado(_))
    ));

    assert!(cadastro.remover(&sistema.id).unwrap());
    assert!(!cadastro.remover(&sistema.id).unwrap());
    assert!(cadastro.listar().unwrap().is_empty());
    // As pastas do sistema ficam intactas.
    assert!(raiz.join("db/01_tabelas.sql").is_file());
}

#[test]
fn varias_pastas_usam_prefixo() {
    let tmp = tempfile::tempdir().unwrap();
    let codigo = tmp.path().join("codigo");
    let scripts = tmp.path().join("Scripts Banco");
    escrever(&codigo.join("src/A.java"), b"class A {}");
    escrever(&scripts.join("01.sql"), b"create table t (id number);");
    escrever(&scripts.join("node_modules/x.js"), b"");
    let sistema = Sistema::novo("Dois", [&codigo, &scripts]).unwrap();
    let inv = inventariar(&sistema, None).unwrap();
    let caminhos: Vec<_> = inv.arquivos.iter().map(|a| a.caminho.as_str()).collect();
    assert_eq!(caminhos, ["scripts-banco/01.sql", "src/A.java"]);
    assert_eq!(
        inv.ignorado("scripts-banco/node_modules").unwrap().motivo,
        Motivo::Dependencia
    );
    let escopo = sistema.escopo().unwrap();
    assert!(escopo.resolver("scripts-banco/01.sql").is_ok());
    assert_eq!(escopo.resolver("01.sql"), Err(Recusa::NaoEncontrado));
}

/// Critério de aceite: 100 mil arquivos em menos de 1 minuto. Lento demais para
/// todo `cargo test` (criar os arquivos domina); rode com
/// `cargo test -p cba-index --release -- --ignored cem_mil`.
#[test]
#[ignore]
fn cem_mil_arquivos_em_menos_de_um_minuto() {
    let tmp = tempfile::tempdir().unwrap();
    let raiz = tmp.path().join("grande");
    for d in 0..1_000 {
        let dir = raiz
            .join(format!("modulo{:03}/src", d / 10))
            .join(format!("p{d}"));
        fs::create_dir_all(&dir).unwrap();
        for f in 0..100 {
            let conteudo = format!("-- arquivo {d}/{f}\nselect {f} from dual;\n").repeat(20);
            fs::write(dir.join(format!("a{f}.sql")), conteudo).unwrap();
        }
    }
    let sistema = Sistema::novo("Grande", [&raiz]).unwrap();
    let t = Instant::now();
    let inv = inventariar(&sistema, None).unwrap();
    let primeiro = t.elapsed();
    assert_eq!(inv.arquivos.len(), 100_000);
    let t = Instant::now();
    let inv2 = inventariar(&sistema, Some(&inv)).unwrap();
    let segundo = t.elapsed();
    assert_eq!(inv2.estatisticas.processados, 0);
    eprintln!("100 mil arquivos: {primeiro:?} (primeiro), {segundo:?} (sem mudanças)");
    assert!(primeiro < Duration::from_secs(60), "{primeiro:?}");
}
