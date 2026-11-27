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
        ..Default::default()
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
                    informada: None,
                    historico: Vec::new(),
                    funcionalidades: Vec::new(),
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
                    funcionalidades: Vec::new(),
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

fn nomes(nos: &Value) -> Vec<&str> {
    nos.as_array()
        .unwrap()
        .iter()
        .map(|n| n["nome"].as_str().unwrap())
        .collect()
}

/// Critério de aceite (#451): com `profundidade`, `referencias` devolve a árvore
/// de quem chama até os pontos de entrada, inclusive a edição de outro registro
/// que chama o método com a validação desligada, e respeita o limite de nós.
#[test]
fn referencias_com_profundidade_sobem_ate_os_pontos_de_entrada() {
    let c = cenario();
    let (texto, dados) = c.chamar(
        "referencias",
        json!({ "identificador": "CancelamentoService.cancelar", "profundidade": 2 }),
    );
    // Os usos diretos continuam na lista paginada.
    assert_eq!(dados["pagina"]["total"], 2, "{dados}");
    let arvore = &dados["arvore"];
    assert_eq!(arvore["profundidade"], 2, "{arvore:#}");
    assert_eq!(arvore["nos"], 3, "{arvore:#}");
    assert_eq!(arvore["cortada"], false);
    let primeiro = &arvore["chamadores"];
    assert_eq!(
        nomes(primeiro),
        [
            "exemplo.faturamento.ClienteService.editar",
            "exemplo.faturamento.FaturaController.cancelar"
        ],
        "{arvore:#}"
    );
    // A edição do cliente chama o cancelamento e é chamada pela rota do cadastro.
    let edicao = &primeiro[0];
    assert_eq!(edicao["nivel"], 1);
    assert_eq!(edicao["usos"], json!([{ "linha": 47, "tipo": "chamada" }]));
    assert_eq!(edicao["ponto_de_entrada"]["tipo"], "servico", "{edicao:#}");
    assert_eq!(
        nomes(&edicao["chamadores"]),
        ["exemplo.faturamento.ClienteController.editar"]
    );
    let rota = &edicao["chamadores"][0];
    assert_eq!(rota["nivel"], 2);
    assert_eq!(rota["usos"], json!([{ "linha": 30, "tipo": "chamada" }]));
    assert_eq!(rota["ponto_de_entrada"]["tipo"], "endpoint", "{rota:#}");
    assert_eq!(
        rota["ponto_de_entrada"]["detalhe"],
        "endpoint PUT /api/clientes/{id}"
    );
    // O portal: a rota com validação, sem chamador no mapa.
    let portal = &primeiro[1];
    assert_eq!(portal["usos"], json!([{ "linha": 27, "tipo": "chamada" }]));
    assert_eq!(
        portal["ponto_de_entrada"]["detalhe"],
        "endpoint POST /api/faturas/{id}/cancelamento"
    );
    assert_eq!(portal["situacao"], "sem_chamador", "{portal:#}");
    assert!(
        texto.contains(
            "Quem chama, até 2 níveis a partir de `CancelamentoService.cancelar` (3 nós):"
        ),
        "{texto}"
    );
    assert!(
        texto.contains("\n  - exemplo.faturamento.ClienteController.editar ("),
        "{texto}"
    );
    assert!(
        texto.contains("[ponto de entrada: endpoint PUT /api/clientes/{id}]"),
        "{texto}"
    );
    assert!(!texto.contains("Aviso: a árvore foi cortada"), "{texto}");

    // Do método que grava, três níveis: o cancelamento, quem o chama e a rota.
    let (_, inteira) = c.chamar(
        "referencias",
        json!({ "identificador": "FaturaRepository.gravarCancelamento", "profundidade": 3 }),
    );
    assert_eq!(inteira["arvore"]["nos"], 4, "{inteira:#}");
    assert_eq!(
        nomes(&inteira["arvore"]["chamadores"]),
        ["exemplo.faturamento.CancelamentoService.cancelar"]
    );
    assert_eq!(
        nomes(&inteira["arvore"]["chamadores"][0]["chamadores"][0]["chamadores"]),
        ["exemplo.faturamento.ClienteController.editar"]
    );

    // Com limite de 2 nós: corta os mais distantes e avisa.
    let (texto, cortada) = c.chamar(
        "referencias",
        json!({
            "identificador": "FaturaRepository.gravarCancelamento",
            "profundidade": 3,
            "limite_de_nos": 2
        }),
    );
    assert_eq!(cortada["arvore"]["nos"], 2, "{cortada:#}");
    assert_eq!(cortada["arvore"]["cortada"], true);
    assert_eq!(cortada["arvore"]["limite_de_nos"], 2);
    assert!(
        texto.contains("Aviso: a árvore foi cortada em 2 nós"),
        "{texto}"
    );

    // Sem profundidade, nada de árvore; e ela só sobe (quem usa).
    let (_, direto) = c.chamar(
        "referencias",
        json!({ "identificador": "CancelamentoService.cancelar" }),
    );
    assert!(direto["arvore"].is_null(), "{direto}");
    let m = c.recusa(
        "referencias",
        json!({
            "identificador": "CancelamentoService.cancelar",
            "sentido": "o_que_usa",
            "profundidade": 2
        }),
    );
    assert!(m.contains("só vale com"), "{m}");
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

/// Indícios de dado pessoal pelo nome e pelo tipo da coluna (#126): os campos
/// pessoais dos clientes, nenhum nas colunas de negócio.
#[test]
fn schema_aponta_indicios_de_dado_pessoal() {
    let c = cenario();
    let indicios = |tabela: &str| -> Vec<(String, String)> {
        let (_, dados) = c.chamar("schema", json!({ "tabela": tabela, "limite": 50 }));
        dados["colunas"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|col| !col["dado_pessoal"].is_null())
            .map(|col| {
                (
                    col["nome"].as_str().unwrap().to_owned(),
                    col["dado_pessoal"]["categoria"]
                        .as_str()
                        .unwrap()
                        .to_owned(),
                )
            })
            .collect()
    };
    let par = |a: &str, b: &str| (a.to_owned(), b.to_owned());
    assert_eq!(
        indicios("CLIENTE"),
        [
            par("NOME", "nome"),
            par("DOCUMENTO", "documento (CPF ou CNPJ)")
        ]
    );
    assert_eq!(
        indicios("CLIENTE_CONTATO"),
        [
            par("EMAIL", "contato"),
            par("TELEFONE", "contato"),
            par("LOGRADOURO", "endereço"),
            par("CEP", "endereço"),
            par("DT_NASCIMENTO", "data de nascimento"),
            par(
                "OBS_ATENDIMENTO",
                "texto livre (pode conter dado pessoal ou sensível)"
            ),
        ]
    );
    for tabela in ["FATURA", "PARCELA", "PARAMETRO", "LOG_FATURA"] {
        assert_eq!(indicios(tabela), [], "{tabela}");
    }

    let (texto, _) = c.chamar("schema", json!({ "tabela": "CLIENTE" }));
    assert!(
        texto.contains("- NOME VARCHAR2(120)") && texto.contains("[indício de dado pessoal: nome]"),
        "{texto}"
    );
    assert!(
        texto.contains("2 coluna(s) com indício de dado pessoal"),
        "{texto}"
    );
    let (texto, todas) = c.chamar("schema", json!({ "limite": 50 }));
    assert!(
        texto.contains("tabela CLIENTE_CONTATO (db/05_cliente_contato.sql:6–22), 9 colunas (6 com indício de dado pessoal)"),
        "{texto}"
    );
    let fatura = todas["objetos"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["nome"] == "FATURA")
        .unwrap();
    assert_eq!(fatura["colunas_com_dado_pessoal"], 0);
}

/// Banco conectado de teste: o schema fixo de um banco de clientes.
#[derive(Debug)]
struct BancoFalso {
    falha: bool,
}

impl cba_core::banco::BancoConectado for BancoFalso {
    fn origem(&self) -> cba_core::banco::OrigemBanco {
        cba_core::banco::OrigemBanco {
            conexao: "banco-hml".into(),
            nome: "Banco de homologação".into(),
            tipo: "PostgreSQL".into(),
            ambiente: Some("homologação".into()),
        }
    }

    fn consultar(&self, _sql: &str) -> Result<cba_core::banco::ResultadoDados, String> {
        Err("não usado".into())
    }

    fn ler_schema(&self) -> Result<cba_core::banco::SchemaBanco, String> {
        use cba_core::banco::{ChaveEstrangeira, ColunaBanco, SchemaBanco, TabelaBanco};
        if self.falha {
            return Err("o servidor não respondeu".into());
        }
        let col = |nome: &str, tipo: &str| ColunaBanco {
            nome: nome.into(),
            tipo: tipo.into(),
            nula: true,
            comentario: None,
        };
        Ok(SchemaBanco {
            tabelas: vec![
                TabelaBanco {
                    esquema: "app".into(),
                    nome: "cliente".into(),
                    tipo: "tabela".into(),
                    comentario: Some("Clientes".into()),
                    colunas: vec![
                        col("id", "integer"),
                        col("nome", "character varying(120)"),
                        col("cpf", "character(11)"),
                        col("dt_nascimento", "date"),
                    ],
                    chave_primaria: vec!["id".into()],
                    chaves_estrangeiras: vec![],
                },
                TabelaBanco {
                    esquema: "app".into(),
                    nome: "fatura".into(),
                    tipo: "tabela".into(),
                    comentario: None,
                    colunas: vec![col("id", "integer"), col("cliente_id", "integer")],
                    chave_primaria: vec!["id".into()],
                    chaves_estrangeiras: vec![ChaveEstrangeira {
                        nome: "fk_fatura_cliente".into(),
                        colunas: vec!["cliente_id".into()],
                        referencia: "app.cliente".into(),
                        colunas_referencia: vec!["id".into()],
                    }],
                },
            ],
        })
    }
}

/// `schema` com `origem: "banco"` lê o banco conectado e marca os indícios de
/// dado pessoal pelos nomes e tipos reais (#439).
#[test]
fn schema_do_banco_conectado() {
    let mut c = cenario();
    let m = c.recusa("schema", json!({ "origem": "banco" }));
    assert!(m.contains("nenhum banco está conectado"), "{m}");
    let (_, dados) = c.chamar("schema", json!({}));
    assert_eq!(dados["origem"]["tipo"], "scripts");

    c.amb.banco = Some(std::sync::Arc::new(BancoFalso { falha: false }));
    let (texto, _) = c.chamar("schema", json!({}));
    assert!(texto.contains("origem: \"banco\""), "{texto}");

    let (texto, dados) = c.chamar("schema", json!({ "origem": "banco" }));
    assert_eq!(dados["origem"]["tipo"], "banco");
    assert_eq!(dados["origem"]["conexao"]["conexao"], "banco-hml");
    assert!(
        texto.starts_with("Origem: banco conectado Banco de homologação (PostgreSQL, homologação)"),
        "{texto}"
    );
    assert!(
        texto.contains("tabela app.cliente, 4 colunas (3 com indício de dado pessoal) — Clientes"),
        "{texto}"
    );
    assert_eq!(dados["pagina"]["total"], 2);

    let (texto, dados) = c.chamar("schema", json!({ "origem": "banco", "tabela": "CLIENTE" }));
    let pessoais: Vec<&str> = dados["colunas"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| !c["dado_pessoal"].is_null())
        .map(|c| c["nome"].as_str().unwrap())
        .collect();
    assert_eq!(pessoais, ["nome", "cpf", "dt_nascimento"]);
    assert!(texto.contains("Chave primária: id"), "{texto}");
    assert!(texto.contains("Referenciada por: app.fatura"), "{texto}");
    assert_eq!(dados["colunas"][0]["chave_primaria"], true);

    let (texto, _) = c.chamar(
        "schema",
        json!({ "origem": "banco", "tabela": "app.fatura" }),
    );
    assert!(texto.contains("(cliente_id) → app.cliente (id)"), "{texto}");
    let m = c.recusa("schema", json!({ "origem": "banco", "tabela": "clientes" }));
    assert!(m.contains("Nomes parecidos: app.cliente"), "{m}");

    c.amb.banco = Some(std::sync::Arc::new(BancoFalso { falha: true }));
    let m = c.recusa("schema", json!({ "origem": "banco" }));
    assert!(m.contains("não respondeu"), "{m}");
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
    assert_eq!(db["arquivos"], 6);

    let (texto, modulo) = c.chamar("mapa", json!({ "modulo": "db/" }));
    assert_eq!(modulo["modulo"], "db");
    assert_eq!(modulo["arquivos"], 6);
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
        (
            "referencias",
            json!({ "identificador": "X", "profundidade": 4 }),
        ),
        (
            "referencias",
            json!({ "identificador": "X", "profundidade": 2, "limite_de_nos": 0 }),
        ),
        ("conhecimento", json!({ "termo": "x", "cursor": -1 })),
        ("conhecimento", json!({ "termo": "x", "tipos": ["outro"] })),
        ("conhecimento", json!({ "termo": "x", "externos": "todos" })),
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

const REPOSITORIO_DE_DOCS: &str = "GitHub exemplo/faturamento-docs";
const PASTA_DOS_EXTERNOS: &str = "faturamento-exemplo/externos/github-exemplo-faturamento-docs";

/// Importa `arquivos` (caminho, texto) como a documentação publicada do projeto,
/// como a sincronização faria (#277), sem git.
fn importar_externos(amb: &Ambiente, arquivos: &[(String, String)]) {
    use cba_kb::externos::{ArquivoExterno, Fonte, importar};
    let lista: Vec<ArquivoExterno> = arquivos
        .iter()
        .enumerate()
        .map(|(i, (c, t))| ArquivoExterno {
            caminho: c.clone(),
            caminho_no_repositorio: format!("docs/{c}"),
            versao: format!("v{i}-{}", t.len()),
            tamanho: t.len() as u64,
        })
        .collect();
    importar(
        &amb.kb,
        &Fonte {
            repositorio: REPOSITORIO_DE_DOCS.into(),
            branch: "main".into(),
            commit: "c0ffee1234567890".into(),
        },
        &lista,
        &|_| None,
        &mut |a| {
            Ok(arquivos
                .iter()
                .find(|(c, _)| *c == a.caminho)
                .unwrap()
                .1
                .as_bytes()
                .to_vec())
        },
        1,
    )
    .unwrap();
}

/// Critérios da #278: a busca devolve a documentação publicada só quando pedida,
/// rotulada (origem, commit, confiança) e distinta da base própria.
#[test]
fn conhecimento_traz_a_documentacao_publicada_rotulada_e_sob_demanda() {
    let c = cenario();
    gravar_regra_e_termo_do_gabarito(&c.amb, "RN-FAT-003");
    importar_externos(
        &c.amb,
        &[
            (
                "processos/carencia.md".into(),
                "# Carência na cobrança\n\nA carência é contada em dias corridos a partir \
                 do vencimento.\n"
                    .into(),
            ),
            (
                "regras/RN-FAT-951.md".into(),
                "---\ntipo: regra\nid: RN-FAT-951\ntitulo: Carência de outra equipe\n---\n\n\
                 Carência diferenciada para contratos antigos.\n"
                    .into(),
            ),
        ],
    );

    // Padrão: só a base própria, com a contagem do que a documentação publicada tem.
    let (texto, padrao) = c.chamar("conhecimento", json!({ "termo": "carência" }));
    assert_eq!(padrao["externos"], "excluir");
    assert_eq!(padrao["externos_disponiveis"], 2, "{padrao}");
    let resultados = padrao["resultados"].as_array().unwrap();
    assert!(!resultados.is_empty());
    assert!(resultados.iter().all(|r| r["fonte"] == "base"), "{padrao}");
    assert!(!texto.contains("[doc. publicada]"), "{texto}");
    assert!(
        texto.contains("tem 2 documento(s)") && texto.contains("externos: \"incluir\""),
        "{texto}"
    );

    // Juntas: a base própria antes, os externos rotulados com a origem.
    let (texto, juntas) = c.chamar(
        "conhecimento",
        json!({ "termo": "carência", "externos": "incluir" }),
    );
    assert!(juntas.get("externos_disponiveis").is_none(), "{juntas}");
    let resultados = juntas["resultados"].as_array().unwrap();
    let fontes: Vec<&str> = resultados
        .iter()
        .map(|r| r["fonte"].as_str().unwrap())
        .collect();
    assert_eq!(fontes.iter().filter(|f| **f == "externa").count(), 2);
    assert_eq!(fontes[0], "base", "{juntas}");
    let externo = resultados
        .iter()
        .find(|r| r["identificador"] == "RN-FAT-951")
        .unwrap();
    assert_eq!(externo["fonte"], "externa");
    assert_eq!(externo["origem"]["repositorio"], REPOSITORIO_DE_DOCS);
    assert_eq!(externo["origem"]["commit"], "c0ffee1234567890");
    assert_eq!(externo["origem"]["confianca"], "publicada");
    assert_eq!(externo["origem"]["tipo_original"], "regra");
    assert_eq!(
        externo["caminho"],
        format!("{PASTA_DOS_EXTERNOS}/regras/RN-FAT-951.md")
    );
    assert!(
        texto.contains(
            "- [doc. publicada] RN-FAT-951 (regra; GitHub exemplo/faturamento-docs @ c0ffee1, \
             confiança publicada): Carência de outra equipe"
        ),
        "{texto}"
    );
    assert!(texto.contains("são pista, não prova"), "{texto}");
    assert!(
        texto.contains("confiança externa): Carência na cobrança"),
        "{texto}"
    );

    // Só a documentação publicada; `tipos` não a esconde.
    let (texto, somente) = c.chamar(
        "conhecimento",
        json!({ "termo": "carência", "externos": "somente", "tipos": ["regra"] }),
    );
    assert_eq!(somente["pagina"]["total"], 2, "{somente}");
    assert!(
        somente["resultados"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["fonte"] == "externa")
    );
    assert!(texto.starts_with("Documentação publicada: 2"), "{texto}");
    // E o filtro de tipo continua valendo para a base própria.
    let (_, regras) = c.chamar(
        "conhecimento",
        json!({ "termo": "carência", "externos": "incluir", "tipos": ["termo"] }),
    );
    let tipos: BTreeSet<&str> = regras["resultados"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["tipo"].as_str().unwrap())
        .collect();
    assert_eq!(tipos, BTreeSet::from(["termo", "doc-externo"]), "{regras}");
}

/// Critério da #278: o contexto automático não é inundado pela documentação
/// publicada. Traz poucos títulos, rotulados, e diz quanto há.
#[test]
fn contexto_menciona_pouca_documentacao_publicada() {
    let c = cenario();
    gravar_regra_e_termo_do_gabarito(&c.amb, "RN-FAT-003");
    let docs: Vec<(String, String)> = (1..=12)
        .map(|i| {
            (
                format!("processos/carencia-{i:02}.md"),
                format!(
                    "# Carência, parte {i}\n\n{}\n",
                    "A carência da fatura em atraso conta dias corridos. ".repeat(40)
                ),
            )
        })
        .collect();
    importar_externos(&c.amb, &docs);
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
    // A base própria continua como antes, sem nenhum externo nas listas dela.
    assert_eq!(ctx["regras"][0]["id"], "RN-FAT-003", "{ctx}");
    for lista in ["regras", "termos", "memorias", "analises"] {
        let texto = ctx[lista].to_string();
        assert!(!texto.contains("/externos/"), "{lista}: {texto}");
    }
    let externos = ctx["externos"].as_array().unwrap();
    assert_eq!(externos.len(), cba_tools::contexto::LIMITE_DE_EXTERNOS);
    assert_eq!(ctx["total_de_externos"], 12);
    assert_eq!(externos[0]["origem"]["repositorio"], REPOSITORIO_DE_DOCS);
    assert!(
        externos[0]["arquivo"]
            .as_str()
            .unwrap()
            .starts_with(PASTA_DOS_EXTERNOS)
    );
    // No roteiro, só os títulos (nada do corpo), com o rótulo e o lembrete.
    assert!(
        texto.contains(
            "Documentação publicada relacionada (12 no repositório de docs; 3 abaixo, só o título)"
        ),
        "{texto}"
    );
    assert_eq!(
        texto.matches("- [doc. publicada] Carência, parte").count(),
        3
    );
    assert!(!texto.contains("conta dias corridos"), "{texto}");
    assert!(texto.contains("pista, não prova"));
}

/// Critério da #278: `afirmar` continua exigindo evidência de código; a
/// documentação publicada não serve de evidência, e a recusa explica.
#[test]
fn afirmar_recusa_a_documentacao_publicada_como_evidencia() {
    let c = cenario();
    importar_externos(
        &c.amb,
        &[(
            "processos/carencia.md".into(),
            "# Carência\n\nA carência é de 3 dias.\n".into(),
        )],
    );
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
    let copia = format!("{PASTA_DOS_EXTERNOS}/processos/carencia.md");
    let absoluto = c
        .amb
        .base
        .raiz()
        .join(&copia)
        .to_string_lossy()
        .into_owned();
    assert!(std::path::Path::new(&absoluto).is_file());
    for arquivo in [
        copia.clone(),
        copia.replace("faturamento-exemplo/", ""),
        absoluto,
    ] {
        let m = c.recusa(
            "afirmar",
            json!({
                "analise": a.id,
                "secao": "resposta",
                "texto": "A carência é de 3 dias.",
                "confianca": "confirmado",
                "evidencias": [{
                    "arquivo": arquivo,
                    "linha_inicial": 3,
                    "linha_final": 3,
                    "trecho": "A carência é de 3 dias."
                }]
            }),
        );
        assert!(m.contains("documentação publicada"), "{arquivo}: {m}");
        assert!(m.contains("pista, não prova"), "{m}");
        assert!(m.contains("cite o trecho do código"), "{m}");
    }
    // Sem evidência, `confirmado` continua recusado.
    c.recusa(
        "afirmar",
        json!({
            "analise": a.id,
            "secao": "resposta",
            "texto": "A carência é de 3 dias, segundo a documentação publicada.",
            "confianca": "confirmado"
        }),
    );
    assert!(
        c.amb
            .analises
            .carregar(&a.id)
            .unwrap()
            .afirmacoes
            .is_empty()
    );
    // Com o código, vale.
    c.chamar(
        "afirmar",
        json!({
            "analise": a.id,
            "secao": "resposta",
            "texto": "Dentro da carência não há multa.",
            "confianca": "confirmado",
            "evidencias": [{
                "arquivo": "db/03_pkg_cobranca.sql",
                "linha_inicial": 66,
                "linha_final": 68,
                "trecho": "IF V_DIAS <= FN_PARAMETRO('DIAS_CARENCIA') THEN\n    RETURN 0;"
            }]
        }),
    );
}
