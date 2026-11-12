//! Ferramentas de consulta (`conhecimento`, `mapa`, `estrutura`, `schema` e
//! `referencias`) com o sistema fictício "Faturamento Exemplo" e o gabarito.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use cba_core::ambiente::Ambiente;
use cba_core::config::Config;
use cba_core::dados::DiretorioDados;
use cba_kb::frontmatter::{
    Evidencia, FrontmatterRegra, FrontmatterTermo, Linhas, StatusRegra, TipoDocumento,
};
use cba_kb::{CaminhoRelativo, Documento, Id, layout};
use cba_tools::{ErroChamada, ErroFerramenta, Saida, registro_padrao};
use serde_json::{Value, json};

fn raiz_do_repositorio() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixture() -> PathBuf {
    raiz_do_repositorio().join("fixtures/faturamento-exemplo")
}

struct Cenario {
    _dir: tempfile::TempDir,
    amb: Ambiente,
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
    }
    .gravar(&dados)
    .unwrap();
    let amb = Ambiente::do_diretorio(&fixture(), &dados).unwrap();
    Cenario { _dir: dir, amb }
}

impl Cenario {
    fn saida(&self, nome: &str, args: Value) -> Result<Saida, ErroFerramenta> {
        match registro_padrao().chamar(nome, Ok(&self.amb), Some(args)) {
            Ok(saida) => Ok(saida),
            Err(ErroChamada::Ferramenta(e)) => Err(e),
            Err(e) => panic!("{e}"),
        }
    }

    fn chamar(&self, nome: &str, args: Value) -> (String, Value) {
        let s = self
            .saida(nome, args)
            .unwrap_or_else(|e| panic!("{nome}: {e}"));
        (s.texto, s.dados.unwrap())
    }

    fn recusa(&self, nome: &str, args: Value) -> String {
        match self.saida(nome, args) {
            Err(ErroFerramenta::Recusa(m)) => m,
            outro => panic!("{nome}: esperava recusa, veio {outro:?}"),
        }
    }

    /// Todas as páginas de uma consulta, seguindo o `proximo_cursor`.
    fn paginas(&self, nome: &str, args: Value, campo: &str) -> (Vec<Value>, usize, usize) {
        let mut todos = Vec::new();
        let mut cursor = 0;
        let mut paginas = 0;
        let total = loop {
            let mut a = args.clone();
            a["cursor"] = json!(cursor);
            let (texto, dados) = self.chamar(nome, a);
            paginas += 1;
            let itens = dados[campo].as_array().unwrap().clone();
            assert_eq!(dados["pagina"]["quantidade"], itens.len());
            assert_eq!(dados["pagina"]["cursor"], cursor);
            todos.extend(itens);
            match dados["pagina"]["proximo_cursor"].as_u64() {
                Some(p) => {
                    assert!(texto.contains(&format!("cursor: {p}")), "{texto}");
                    cursor = p as usize;
                }
                None => break dados["pagina"]["total"].as_u64().unwrap() as usize,
            }
            assert!(paginas < 100, "paginação sem fim");
        };
        (todos, total, paginas)
    }
}

/// Grava na base a regra do gabarito, com as evidências, e o termo "Carência".
fn gravar_regra_e_termo_do_gabarito(amb: &Ambiente, id: &str) {
    let caminho = raiz_do_repositorio().join("fixtures/gabaritos/faturamento-exemplo.yaml");
    let gabarito: serde_norway::Value =
        serde_norway::from_str(&std::fs::read_to_string(caminho).unwrap()).unwrap();
    let gabarito = serde_json::to_value(gabarito).unwrap();
    let regra = gabarito["regras"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == id)
        .unwrap()
        .clone();
    let texto = |v: &Value| v.as_str().unwrap().to_owned();
    let evidencias = regra["evidencias"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| Evidencia {
            arquivo: CaminhoRelativo::novo(e["arquivo"].as_str().unwrap()).unwrap(),
            linhas: Linhas::novo(
                e["linha_inicial"].as_u64().unwrap() as u32,
                e["linha_final"].as_u64().unwrap() as u32,
            )
            .unwrap(),
            hash: "0000".into(),
            commit: None,
        })
        .collect();
    let id = Id::analisar(id).unwrap();
    amb.kb
        .criar(
            &id.arquivo(),
            &Documento::novo(
                FrontmatterRegra {
                    tipo: TipoDocumento::Regra,
                    id: id.clone(),
                    titulo: texto(&regra["titulo"]),
                    area: texto(&regra["area"]),
                    status: StatusRegra::Confirmada,
                    evidencias,
                    origem: None,
                },
                format!("\n{}\n", texto(&regra["enunciado"])),
            ),
        )
        .unwrap();
    amb.kb
        .criar(
            &layout::termo("Carência").unwrap(),
            &Documento::novo(
                FrontmatterTermo {
                    tipo: TipoDocumento::Termo,
                    termo: "Carência".into(),
                    sinonimos: vec!["período de tolerância".into()],
                    area: Some("cobranca".into()),
                    origem: None,
                },
                "\nDias após o vencimento sem encargos: parâmetro DIAS_CARENCIA.\n",
            ),
        )
        .unwrap();
}

/// Critério de aceite: `referencias` da função de multa devolve quem a chama.
#[test]
fn referencias_da_funcao_de_multa_devolve_os_chamadores() {
    let c = cenario();
    let (texto, dados) = c.chamar(
        "referencias",
        json!({ "identificador": "PKG_COBRANCA.FN_CALCULAR_MULTA" }),
    );
    // Sem mapa gravado, a primeira chamada gera o mapa e avisa.
    assert_eq!(dados["mapa"]["gerado_agora"], true, "{dados}");
    assert!(texto.contains("foi gerado agora"), "{texto}");

    let chamadores: Vec<(String, u64, String, String)> = dados["referencias"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r["arquivo"].as_str().unwrap().to_owned(),
                r["linha"].as_u64().unwrap(),
                r["tipo"].as_str().unwrap().to_owned(),
                r["origem"].as_str().unwrap_or_default().to_owned(),
            )
        })
        .collect();
    assert_eq!(
        chamadores,
        [(
            "backend/src/main/java/exemplo/faturamento/EncargosService.java".to_owned(),
            28,
            "mencao".to_owned(),
            "exemplo.faturamento.EncargosService.calcular".to_owned()
        )],
        "{dados}"
    );
    assert!(
        texto.contains(
            "- backend/src/main/java/exemplo/faturamento/EncargosService.java:28 — mencao, em exemplo.faturamento.EncargosService.calcular"
        ),
        "{texto}"
    );
    // A definição (corpo e declaração no pacote) vem no cabeçalho.
    let definicoes: Vec<(&str, u64)> = dados["definicoes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| {
            (
                d["arquivo"].as_str().unwrap(),
                d["linha_inicial"].as_u64().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        definicoes,
        [
            ("db/03_pkg_cobranca.sql", 13),
            ("db/03_pkg_cobranca.sql", 50)
        ]
    );
    assert!(
        texto.contains("funcao PKG_COBRANCA.FN_CALCULAR_MULTA (db/03_pkg_cobranca.sql:50–73)"),
        "{texto}"
    );

    // Pelo nome simples, em minúsculas, o mesmo resultado; o mapa já existe.
    let (_, simples) = c.chamar(
        "referencias",
        json!({ "identificador": "fn_calcular_multa" }),
    );
    assert_eq!(simples["mapa"]["gerado_agora"], false);
    assert_eq!(simples["referencias"], dados["referencias"]);

    // A tela de cancelamento chama a procedure de cancelamento.
    let (_, cancelar) = c.chamar(
        "referencias",
        json!({ "identificador": "PRC_CANCELAR_FATURA" }),
    );
    assert!(
        cancelar["referencias"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["arquivo"] == "delphi/UFrmCancelamento.pas" && r["linha"] == 94),
        "{cancelar}"
    );

    // Filtro por tipo de uso.
    let (_, mencoes) = c.chamar(
        "referencias",
        json!({ "identificador": "FN_CALCULAR_MULTA", "tipos": ["mencao"] }),
    );
    assert_eq!(mencoes["pagina"]["total"], 1);
    let (_, chamadas) = c.chamar(
        "referencias",
        json!({ "identificador": "FN_CALCULAR_MULTA", "tipos": ["chamada"] }),
    );
    assert_eq!(chamadas["pagina"]["total"], 0);

    // No outro sentido: o que a função de multa chama.
    let (_, usa) = c.chamar(
        "referencias",
        json!({ "identificador": "PKG_COBRANCA.FN_CALCULAR_MULTA", "sentido": "o_que_usa" }),
    );
    let alvos: BTreeSet<&str> = usa["referencias"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["tipo"] == "chamada")
        .flat_map(|r| r["alvos"].as_array().unwrap())
        .map(|a| a.as_str().unwrap())
        .collect();
    assert!(alvos.contains("PKG_COBRANCA.FN_PARAMETRO"), "{usa}");
    assert!(alvos.contains("PKG_COBRANCA.FN_DIAS_ATRASO"), "{usa}");

    // Nome que não existe: resposta vazia, não erro.
    let (texto, nada) = c.chamar("referencias", json!({ "identificador": "NAO_EXISTE_X" }));
    assert_eq!(nada["pagina"]["total"], 0);
    assert!(
        texto.contains("não é definido no código mapeado"),
        "{texto}"
    );
}

/// Critério de aceite: respostas acima do limite vêm paginadas, com o total e o
/// cursor da próxima página, e as páginas juntas trazem tudo, sem repetir.
#[test]
fn respostas_acima_do_limite_vem_paginadas() {
    let c = cenario();
    let (_, inteira) = c.chamar("schema", json!({ "limite": 50 }));
    let total = inteira["pagina"]["total"].as_u64().unwrap() as usize;
    assert!(total > 3, "{inteira}");

    let (todos, total_paginado, paginas) = c.paginas("schema", json!({ "limite": 2 }), "objetos");
    assert_eq!(total_paginado, total);
    assert_eq!(todos.len(), total);
    assert_eq!(paginas, total.div_ceil(2));
    let nomes: BTreeSet<String> = todos.iter().map(|o| o["nome"].to_string()).collect();
    assert_eq!(nomes.len(), total, "sem repetição");
    assert_eq!(
        &todos[..],
        inteira["objetos"].as_array().unwrap().as_slice()
    );

    // Sem `limite`, o padrão: o script das tabelas tem mais símbolos (tabelas e
    // colunas) do que cabem numa página.
    let (texto, estrutura) = c.chamar("estrutura", json!({ "arquivo": "db/01_tabelas.sql" }));
    let quantidade = estrutura["pagina"]["quantidade"].as_u64().unwrap();
    let total = estrutura["pagina"]["total"].as_u64().unwrap();
    assert!(total > quantidade, "{estrutura}");
    assert_eq!(estrutura["pagina"]["proximo_cursor"], quantidade);
    assert!(texto.contains(&format!("de {total}.")), "{texto}");
    let (todos, total_paginado, _) = c.paginas(
        "estrutura",
        json!({ "arquivo": "db/01_tabelas.sql" }),
        "simbolos",
    );
    assert_eq!(total_paginado as u64, total);
    assert_eq!(todos.len() as u64, total);

    // Cursor além do fim: página vazia que diz o total.
    let (texto, vazia) = c.chamar("schema", json!({ "cursor": 999 }));
    assert_eq!(vazia["pagina"]["quantidade"], 0);
    assert!(texto.contains("passou do fim"), "{texto}");
}

/// Critério da issue: `conhecimento("carencia")` encontra a regra e o termo do
/// gabarito depois de gravados na base (o índice é sincronizado a cada chamada).
#[test]
fn conhecimento_encontra_a_carencia_depois_de_gravada() {
    let c = cenario();
    let (texto, antes) = c.chamar("conhecimento", json!({ "termo": "carencia" }));
    assert_eq!(antes["pagina"]["total"], 0, "{antes}");
    assert!(texto.contains("Nada na base de conhecimento"), "{texto}");

    gravar_regra_e_termo_do_gabarito(&c.amb, "RN-FAT-003");

    let (texto, depois) = c.chamar("conhecimento", json!({ "termo": "carencia" }));
    let caminhos: Vec<&str> = depois["resultados"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["caminho"].as_str().unwrap())
        .collect();
    assert!(
        caminhos.contains(&"faturamento-exemplo/regras/RN-FAT-003.md"),
        "{depois}"
    );
    assert!(
        caminhos.contains(&"faturamento-exemplo/glossario/carencia.md"),
        "{depois}"
    );
    assert!(
        texto.contains("RN-FAT-003 (regra, confirmada, cobranca): Carência de 3 dias sem encargos"),
        "{texto}"
    );
    assert_eq!(
        depois["base"],
        c.amb.base.raiz().to_string_lossy().as_ref(),
        "o agente sabe onde ler"
    );

    // Só regras; e o sinônimo do termo também acha.
    let (_, regras) = c.chamar(
        "conhecimento",
        json!({ "termo": "Carência", "tipos": ["regra"] }),
    );
    assert_eq!(regras["pagina"]["total"], 1, "{regras}");
    assert_eq!(regras["resultados"][0]["identificador"], "RN-FAT-003");
    let (_, sinonimo) = c.chamar("conhecimento", json!({ "termo": "tolerância" }));
    assert_eq!(
        sinonimo["resultados"][0]["caminho"],
        "faturamento-exemplo/glossario/carencia.md"
    );

    // Nenhum documento com todas as palavras: vale qualquer uma, e a resposta diz.
    let (texto, qualquer) = c.chamar("conhecimento", json!({ "termo": "carência xilofone" }));
    assert_eq!(qualquer["modo"], "qualquer", "{qualquer}");
    assert!(qualquer["pagina"]["total"].as_u64().unwrap() >= 2);
    assert!(texto.contains("estes têm ao menos uma"), "{texto}");
}

#[test]
fn iniciar_analise_traz_o_contexto_pelo_indice() {
    let c = cenario();
    gravar_regra_e_termo_do_gabarito(&c.amb, "RN-FAT-003");
    let a = c
        .amb
        .analises
        .criar(cba_core::analises::NovaAnalise {
            fluxo: cba_core::analise::Fluxo::Perguntar,
            titulo: "Carência".into(),
            entrada: "Quantos dias de carência a fatura em atraso tem?".into(),
            analista: None,
            base: None,
        })
        .unwrap();
    let (texto, dados) = c.chamar("iniciar_analise", json!({ "analise": a.id }));
    let ctx = &dados["contexto"];
    assert_eq!(ctx["regras"][0]["id"], "RN-FAT-003", "{ctx}");
    assert_eq!(ctx["termos"][0]["termo"], "Carência", "{ctx}");
    assert_eq!(ctx["termos"][0]["sinonimos"][0], "período de tolerância");
    assert_eq!(ctx["total_de_regras"], 1);
    assert_eq!(ctx["total_de_termos"], 1);
    assert!(
        texto.contains("- RN-FAT-003 (confirmada): Carência de 3 dias sem encargos"),
        "{texto}"
    );
}

#[test]
fn estrutura_lista_as_rotinas_com_linhas() {
    let c = cenario();
    let (texto, dados) = c.chamar(
        "estrutura",
        json!({ "arquivo": "./db/03_pkg_cobranca.sql", "limite": 50 }),
    );
    assert_eq!(dados["arquivo"], "db/03_pkg_cobranca.sql");
    let multa = dados["simbolos"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["qualificado"] == "PKG_COBRANCA.FN_CALCULAR_MULTA" && s["declaracao"] == false)
        .unwrap_or_else(|| panic!("{dados}"));
    assert_eq!(multa["linha_inicial"], 50);
    assert_eq!(multa["linha_final"], 73);
    assert_eq!(multa["tipo"], "funcao");
    assert_eq!(multa["pai"], "PKG_COBRANCA");
    assert!(
        texto.contains("  - funcao FN_CALCULAR_MULTA (linhas 50–73)"),
        "{texto}"
    );

    // Pelo caminho absoluto dentro do projeto também.
    let absoluto = fixture().join("db/03_pkg_cobranca.sql");
    let absoluto = dunce_canonico(&absoluto);
    let (_, mesmo) = c.chamar("estrutura", json!({ "arquivo": absoluto, "limite": 50 }));
    assert_eq!(mesmo["simbolos"], dados["simbolos"]);

    // Caminho errado: recusa com a sugestão do arquivo de mesmo nome.
    let m = c.recusa("estrutura", json!({ "arquivo": "03_pkg_cobranca.sql" }));
    assert!(m.contains("`db/03_pkg_cobranca.sql`"), "{m}");
    // Fora do projeto: recusa com o motivo.
    let m = c.recusa("estrutura", json!({ "arquivo": "../segredo.txt" }));
    assert!(m.contains("não pode ser consultado"), "{m}");
}

fn dunce_canonico(p: &Path) -> String {
    std::fs::canonicalize(p)
        .unwrap()
        .to_string_lossy()
        .trim_start_matches(r"\\?\")
        .to_owned()
}

#[test]
fn schema_da_tabela_de_faturas() {
    let c = cenario();
    let (texto, dados) = c.chamar("schema", json!({ "tabela": "fatura" }));
    let objeto = &dados["objetos"][0];
    assert_eq!(objeto["tipo"], "tabela", "{dados}");
    assert_eq!(objeto["arquivo"], "db/01_tabelas.sql");
    assert_eq!(objeto["linha_inicial"], 20);
    assert_eq!(objeto["colunas"], 10);
    assert_eq!(dados["colunas"][0]["nome"], "ID_FATURA");
    assert_eq!(dados["pagina"]["total"], 10);
    assert!(
        texto.contains("tabela FATURA (db/01_tabelas.sql:20–37)"),
        "{texto}"
    );
    assert!(texto.contains("Colunas:\n- ID_FATURA"), "{texto}");

    let m = c.recusa("schema", json!({ "tabela": "FATURAS_X" }));
    assert!(m.contains("não foi encontrada"), "{m}");
    let m = c.recusa("schema", json!({ "tabela": "FN_CALCULAR_MULTA" }));
    assert!(m.contains("não é tabela nem visão"), "{m}");
}

#[test]
fn mapa_do_projeto_e_de_um_modulo() {
    let c = cenario();
    let (texto, dados) = c.chamar("mapa", json!({}));
    let pastas: Vec<&str> = dados["entradas"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["tipo"] == "pasta")
        .map(|e| e["caminho"].as_str().unwrap())
        .collect();
    assert_eq!(pastas, ["backend", "cobol", "db", "delphi", "frontend"]);
    assert!(texto.starts_with("Projeto Faturamento Exemplo:"), "{texto}");
    let db = dados["entradas"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["caminho"] == "db")
        .unwrap();
    assert_eq!(db["arquivos"], 3);

    let (texto, modulo) = c.chamar("mapa", json!({ "modulo": "db/" }));
    assert_eq!(modulo["modulo"], "db");
    assert_eq!(modulo["arquivos"], 3);
    assert!(texto.contains("- db/03_pkg_cobranca.sql — sql"), "{texto}");

    let m = c.recusa("mapa", json!({ "modulo": "nao-existe" }));
    assert!(
        m.contains("Pastas na raiz: backend, cobol, db, delphi, frontend"),
        "{m}"
    );
}

#[test]
fn fora_do_schema_os_argumentos_sao_recusados() {
    let c = cenario();
    for (nome, args) in [
        ("referencias", json!({})),
        ("referencias", json!({ "identificador": "X", "limite": 0 })),
        (
            "referencias",
            json!({ "identificador": "X", "sentido": "outro" }),
        ),
        ("conhecimento", json!({ "termo": "x", "cursor": -1 })),
        ("conhecimento", json!({ "termo": "x", "tipos": ["outro"] })),
        ("estrutura", json!({ "arquivo": "x", "extra": 1 })),
    ] {
        assert!(
            matches!(
                c.saida(nome, args.clone()),
                Err(ErroFerramenta::Argumentos(_))
            ),
            "{nome} {args}"
        );
    }
}
