//! Histórico git sintético da fixture (#450): reproduzido numa cópia, termina
//! exatamente na fixture, e o histórico das linhas da regra de origem chega ao
//! commit da demanda que a criou.

use std::path::Path;
use std::process::Command;

use cba_avaliacao::gabarito::Gabarito;
use cba_avaliacao::gabarito_padrao;
use cba_avaliacao::historico::Historico;
use cba_avaliacao::pastas;

fn gabarito() -> Gabarito {
    Gabarito::carregar(&gabarito_padrao()).unwrap()
}

/// Assuntos dos commits que tocaram as linhas, do mais recente ao mais antigo.
fn historico_das_linhas(repo: &Path, arquivo: &str, inicio: u32, fim: u32) -> Vec<String> {
    let saida = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args([
            "log",
            "--no-color",
            "--format=\u{1e}%s",
            format!("-L{inicio},{fim}:{arquivo}").as_str(),
        ])
        .output()
        .unwrap();
    assert!(
        saida.status.success(),
        "{}",
        String::from_utf8_lossy(&saida.stderr)
    );
    String::from_utf8_lossy(&saida.stdout)
        .lines()
        .filter_map(|l| l.strip_prefix('\u{1e}'))
        .map(str::to_owned)
        .collect()
}

#[test]
fn historico_termina_na_fixture_e_a_origem_da_regra_e_a_demanda() {
    let g = gabarito();
    let projeto = g.raiz_do_projeto();
    let com_origem: Vec<_> = g.demandas.iter().filter(|d| d.origem.is_some()).collect();
    assert!(!com_origem.is_empty(), "nenhuma demanda com histórico");
    for d in com_origem {
        let o = d.origem.as_ref().unwrap();
        let pasta = g.pasta_do_historico(&o.historico);
        let h = Historico::ler(&pasta).unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let codigo = tmp.path().join("faturamento");
        pastas::copiar(&projeto, &codigo).unwrap();
        h.reproduzir(&pasta, &projeto, &codigo).unwrap();
        assert_eq!(
            pastas::diferencas(&projeto, &codigo).unwrap(),
            Vec::new(),
            "{}: a cópia com o histórico difere da fixture",
            d.id
        );
        let regra = g.regra(&o.regra).unwrap();
        for e in &regra.evidencias {
            let assuntos =
                historico_das_linhas(&codigo, &e.arquivo, e.linha_inicial, e.linha_final);
            let origem = assuntos
                .last()
                .unwrap_or_else(|| panic!("{}: sem histórico", e.arquivo));
            // As linhas da checagem da situação vêm da carga inicial; as do pagamento
            // parcial, da demanda.
            if e.trecho.contains("V_VL_PAGO") {
                assert!(origem.contains(&o.chave), "{}: {assuntos:?}", d.id);
                assert!(
                    assuntos.len() > 1,
                    "{}: falta o ajuste depois da origem",
                    d.id
                );
                assert!(!assuntos[0].contains(&o.chave), "{assuntos:?}");
            }
        }
    }
}

#[test]
fn historico_que_nao_termina_na_fixture_e_recusado() {
    let g = gabarito();
    let projeto = g.raiz_do_projeto();
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join("versoes")).unwrap();
    std::fs::write(tmp.path().join("versoes/v1.sql"), "-- antiga\n").unwrap();
    let escrever = |yaml: &str| {
        std::fs::write(tmp.path().join("historico.yaml"), yaml).unwrap();
        Historico::ler(tmp.path()).unwrap()
    };
    let parado = escrever(
        "commits:\n  - data: 2023-01-01\n    mensagem: Carga\n    versoes:\n      db/03_pkg_cobranca.sql: versoes/v1.sql\n",
    );
    let e = parado.validar(tmp.path(), &projeto).unwrap_err();
    assert!(e.contains("termina na versão"), "{e}");
    let sem_arquivo = escrever(
        "commits:\n  - data: 2023-01-01\n    mensagem: Carga\n    versoes:\n      db/nao-existe.sql: versoes/v1.sql\n",
    );
    assert!(sem_arquivo.validar(tmp.path(), &projeto).is_err());
    let data_errada = escrever("commits:\n  - data: 01/01/2023\n    mensagem: Carga\n");
    assert!(data_errada.validar(tmp.path(), &projeto).is_err());
    let ok = escrever(
        "commits:\n  - data: 2023-01-01\n    mensagem: Carga\n    versoes:\n      db/03_pkg_cobranca.sql: versoes/v1.sql\n  - data: 2024-01-01\n    mensagem: \"[DEM-1] Atual\"\n    versoes:\n      db/03_pkg_cobranca.sql: atual\n",
    );
    ok.validar(tmp.path(), &projeto).unwrap();
}
