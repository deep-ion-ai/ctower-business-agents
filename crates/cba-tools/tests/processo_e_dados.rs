//! Fluxo do processo e modelo de dados conceitual no documento negocial (#120),
//! com o sistema fictício "Faturamento Exemplo": o cancelamento de fatura em raias
//! (tela Delphi e banco) e o modelo de dados com as chaves estrangeiras dos
//! scripts e a relação do histórico, inferida do `INSERT` no PL/SQL. Confere o
//! documento montado, a recusa da chave estrangeira que não existe, a ligação com
//! o glossário, o "Virar pergunta" e a exportação para Word.

use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use cba_core::ambiente::Ambiente;
use cba_core::analise::{Analise, EstadoPergunta, Fluxo, OrigemPergunta};
use cba_core::analises::NovaAnalise;
use cba_core::config::Config;
use cba_core::dados::DiretorioDados;
use cba_core::diagramas::{OrigemRelacao, TipoDiagrama};
use cba_docs::{Figura, Figuras};
use cba_tools::revisao::{self, Decisao, Formato, OpcoesExportacao};
use cba_tools::{ErroChamada, ErroFerramenta, registro_padrao};
use serde_json::{Value, json};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/faturamento-exemplo")
}

struct Cenario {
    dir: tempfile::TempDir,
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
    Cenario { dir, amb }
}

impl Cenario {
    fn criar(&self) -> Analise {
        self.amb
            .analises
            .criar(NovaAnalise {
                fluxo: Fluxo::Explicar,
                titulo: "Cancelamento de fatura".into(),
                entrada: "Cancelamento de fatura".into(),
                analista: Some("Ana Exemplo".into()),
                base: None,
            })
            .unwrap()
    }

    fn chamar(&self, nome: &str, args: Value) -> Result<Value, ErroFerramenta> {
        match registro_padrao().chamar(nome, Ok(&self.amb), Some(args)) {
            Ok(saida) => Ok(saida.dados.expect("resposta estruturada")),
            Err(ErroChamada::Ferramenta(e)) => Err(e),
            Err(e) => panic!("{e}"),
        }
    }

    fn recusa(&self, nome: &str, args: Value) -> String {
        match self.chamar(nome, args) {
            Err(ErroFerramenta::Recusa(m) | ErroFerramenta::Argumentos(m)) => m,
            outro => panic!("esperava recusa, veio {outro:?}"),
        }
    }

    fn documento(&self, analise: &Analise) -> String {
        std::fs::read_to_string(self.amb.kb.caminho(&analise.documento)).unwrap()
    }

    /// Termo do glossário gravado na base, como a aprovação grava.
    fn termo_na_base(&self, slug: &str, termo: &str, sinonimos: &[&str], onde_vive: &[&str]) {
        let caminho = self
            .amb
            .kb
            .caminho(&cba_kb::CaminhoRelativo::novo(&format!("glossario/{slug}.md")).unwrap());
        std::fs::create_dir_all(caminho.parent().unwrap()).unwrap();
        let itens: Vec<String> = onde_vive.iter().map(|o| format!("- {o}")).collect();
        std::fs::write(
            caminho,
            format!(
                "---\ntipo: termo\ntermo: {termo}\nsinonimos: [{}]\n---\n# {termo}\n\n\
                 ## Definição de negócio\n\nQuem recebe as faturas.\n\n\
                 ## Onde vive no código\n\n{}\n",
                sinonimos.join(", "),
                itens.join("\n")
            ),
        )
        .unwrap();
    }
}

fn ev(arquivo: &str, inicial: u32, final_: u32, trecho: &str) -> Value {
    json!({
        "arquivo": arquivo,
        "linha_inicial": inicial,
        "linha_final": final_,
        "trecho": trecho
    })
}

const TELA: &str = "delphi/UFrmCancelamento.pas";
const PKG: &str = "db/03_pkg_cobranca.sql";
const TABELAS: &str = "db/01_tabelas.sql";

/// O cancelamento de fatura em duas raias: a tela da retaguarda valida o motivo
/// e a regra dos 30 dias; o banco confere a situação e grava.
fn processo(analise: &Analise) -> Value {
    let atendente = "Atendente (tela de cancelamento)";
    let banco = "Banco de dados";
    json!({
        "analise": analise.id,
        "secao": "fluxo_processo",
        "tipo": "fluxo",
        "titulo": "Cancelamento de fatura",
        "nos": [
            {"chave": "inicio", "rotulo": "Informar a fatura e o motivo", "papel": "inicio", "raia": atendente,
             "evidencias": [ev(TELA, 84, 84, "IdFatura := StrToIntDef(edtFatura.Text, 0);")]},
            {"chave": "motivo", "rotulo": "Motivo com 10 caracteres ou mais?", "papel": "decisao", "raia": atendente,
             "evidencias": [ev(TELA, 63, 63, "Length(Trim(memMotivo.Text)) < TAM_MINIMO_MOTIVO")]},
            {"chave": "prazo", "rotulo": "Emitida há mais de 30 dias e sem supervisor?", "papel": "decisao", "raia": atendente,
             "evidencias": [ev(TELA, 70, 71, "DIAS_LIMITE_SEM_SUPERVISOR")]},
            {"chave": "recusa_tela", "rotulo": "Recusado na tela", "papel": "fim", "raia": atendente},
            {"chave": "situacao", "rotulo": "Fatura aberta e sem pagamento?", "papel": "decisao", "raia": banco,
             "evidencias": [ev(PKG, 111, 117, "IF V_VL_PAGO > 0 THEN")]},
            {"chave": "gravar", "rotulo": "Gravar o cancelamento e o histórico", "raia": banco,
             "evidencias": [ev(PKG, 123, 131, "INSERT INTO LOG_FATURA")]},
            {"chave": "cancelada", "rotulo": "Fatura cancelada", "papel": "fim", "raia": banco},
            {"chave": "recusa_banco", "rotulo": "Recusado pelo banco", "papel": "fim", "raia": banco}
        ],
        "arestas": [
            {"de": "inicio", "para": "motivo", "confianca": "confirmado", "evidencias": [ev(TELA, 91, 91, "ValidarCancelamento(IdFatura)")]},
            {"de": "motivo", "para": "recusa_tela", "rotulo": "não", "confianca": "confirmado", "evidencias": [ev(TELA, 65, 65, "minimo de 10 caracteres")]},
            {"de": "motivo", "para": "prazo", "rotulo": "sim", "confianca": "inferido"},
            {"de": "prazo", "para": "recusa_tela", "rotulo": "sim", "confianca": "confirmado", "evidencias": [ev(TELA, 73, 73, "Solicite a um supervisor")]},
            {"de": "prazo", "para": "situacao", "rotulo": "não", "confianca": "confirmado", "evidencias": [ev(TELA, 94, 94, "PKG_COBRANCA.PRC_CANCELAR_FATURA")]},
            {"de": "situacao", "para": "recusa_banco", "rotulo": "não", "confianca": "confirmado", "evidencias": [ev(PKG, 112, 112, "-20010")]},
            {"de": "situacao", "para": "gravar", "rotulo": "sim", "confianca": "inferido"},
            {"de": "gravar", "para": "cancelada", "confianca": "inferido"}
        ]
    })
}

fn atributo(nome: &str, coluna: &str) -> Value {
    json!({"nome": nome, "coluna": coluna})
}

fn cardinalidade(de: &str, para: &str) -> Value {
    json!({"de": de, "para": para})
}

/// Modelo de dados conceitual do faturamento: as relações com chave estrangeira
/// nos scripts e a do histórico, que só aparece no `INSERT` do PL/SQL.
fn modelo(analise: &Analise, cliente: &str) -> Value {
    json!({
        "analise": analise.id,
        "secao": "modelo_dados",
        "tipo": "dados",
        "titulo": "Modelo de dados do cancelamento",
        "nos": [
            {"chave": "cliente", "rotulo": cliente, "tabela": "CLIENTE",
             "atributos": [atributo("Nome", "NOME"), atributo("Tipo de cliente", "TIPO_CLIENTE")]},
            {"chave": "fatura", "rotulo": "Fatura", "tabela": "FATURA",
             "atributos": [atributo("Data de emissão", "DT_EMISSAO"), atributo("Situação", "STATUS"),
                           atributo("Motivo do cancelamento", "MOTIVO_CANCEL")]},
            {"chave": "parcela", "rotulo": "Parcela", "tabela": "PARCELA",
             "atributos": [atributo("Número da parcela", "NR_PARCELA"), atributo("Valor da parcela", "VL_PARCELA")]},
            {"chave": "historico", "rotulo": "Histórico da fatura", "tabela": "LOG_FATURA",
             "atributos": [atributo("Evento", "EVENTO"), atributo("Usuário", "USUARIO")]}
        ],
        "arestas": [
            {"de": "cliente", "para": "fatura", "rotulo": "recebe", "confianca": "confirmado",
             "cardinalidade": cardinalidade("um", "zero_ou_muitos"), "origem": "chave_estrangeira",
             "evidencias": [ev(TABELAS, 33, 34, "FOREIGN KEY (ID_CLIENTE)")]},
            {"de": "fatura", "para": "parcela", "rotulo": "é dividida em", "confianca": "confirmado",
             "cardinalidade": cardinalidade("um", "zero_ou_muitos"), "origem": "chave_estrangeira",
             "evidencias": [ev(TABELAS, 46, 47, "FOREIGN KEY (ID_FATURA)")]},
            {"de": "fatura", "para": "historico", "rotulo": "registra", "confianca": "inferido",
             "cardinalidade": cardinalidade("um", "zero_ou_muitos"), "origem": "join",
             "evidencias": [ev(PKG, 130, 131, "INSERT INTO LOG_FATURA")]}
        ]
    })
}

#[test]
fn fluxo_do_processo_em_raias_no_documento_negocial() {
    let c = cenario();
    let a = c.criar();
    let r = c.chamar("registrar_diagrama", processo(&a)).unwrap();
    assert_eq!(r["diagrama"], "D-1");
    assert_eq!(r["secao"], "fluxo_processo");

    let estado = c.amb.analises.carregar(&a.id).unwrap();
    let d = estado.diagrama(1).unwrap();
    assert_eq!(d.tipo, TipoDiagrama::Fluxo);
    assert_eq!(d.nos[4].raia.as_deref(), Some("Banco de dados"));

    let doc = c.documento(&estado);
    let secao = doc
        .split("## Fluxo do processo")
        .nth(1)
        .and_then(|s| s.split("\n## ").next())
        .unwrap();
    assert!(
        secao.contains(
            "```mermaid\nflowchart TD\n  subgraph r1[\"Atendente (tela de cancelamento)\"]\n    \
             n1([\"Informar a fatura e o motivo\"])\n    n2{\"Motivo com 10 caracteres ou mais?\"}\n"
        ),
        "{secao}"
    );
    assert!(
        secao.contains(
            "  subgraph r2[\"Banco de dados\"]\n    n5{\"Fatura aberta e sem pagamento?\"}"
        ),
        "{secao}"
    );
    assert!(secao.contains("  n2 -->|\"não\"| n4\n"), "{secao}");
    assert!(secao.contains("  n6 -.-> n7\n"), "{secao}");
    assert!(
        secao.contains("*Figura 1: Cancelamento de fatura*"),
        "{secao}"
    );
    assert!(secao.contains("| Passo | Raia | Descrição |"), "{secao}");
    // A seção do modelo ainda está vazia.
    assert!(doc.contains("_Modelo de dados conceitual ainda não registrado._"));
}

#[test]
fn modelo_de_dados_com_glossario_e_relacao_inferida() {
    let c = cenario();
    let a = c.criar();
    // Glossário: "Cliente" na base (com o sinônimo "Pagador"); "Fatura" proposto na
    // análise; o histórico sem termo.
    c.termo_na_base(
        "cliente",
        "Cliente",
        &["Pagador"],
        &["tabela CLIENTE", "CLIENTE.TIPO_CLIENTE"],
    );
    c.chamar(
        "propor_termo",
        json!({
            "analise": a.id,
            "termo": "Fatura",
            "definicao": "Cobrança emitida para um cliente.",
            "onde_vive_no_codigo": ["tabela FATURA"]
        }),
    )
    .unwrap();

    // Chave estrangeira que os scripts não declaram: recusada.
    let mut errado = modelo(&a, "Pagador");
    errado["arestas"][2]["origem"] = json!("chave_estrangeira");
    let m = c.recusa("registrar_diagrama", errado);
    assert!(
        m.contains("não declaram chave estrangeira entre FATURA e LOG_FATURA"),
        "{m}"
    );
    // Join não é confirmado.
    let mut confirmado = modelo(&a, "Pagador");
    confirmado["arestas"][2]["confianca"] = json!("confirmado");
    assert!(
        c.recusa("registrar_diagrama", confirmado)
            .contains("relação inferida")
    );
    // Cardinalidade sem origem.
    let mut metade = modelo(&a, "Pagador");
    metade["arestas"][0]
        .as_object_mut()
        .unwrap()
        .remove("origem");
    assert!(c.recusa("registrar_diagrama", metade).contains("juntas"));
    assert!(c.amb.analises.carregar(&a.id).unwrap().diagramas.is_empty());

    let r = c
        .chamar("registrar_diagrama", modelo(&a, "Pagador"))
        .unwrap();
    let entidades = r["entidades"].as_array().unwrap();
    assert_eq!(entidades[0]["termo_do_glossario"], "Cliente");
    assert_eq!(entidades[1]["termo_do_glossario"], "Fatura");
    assert!(entidades[3]["termo_do_glossario"].is_null());
    let avisos: Vec<&str> = r["avisos"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert!(
        avisos
            .iter()
            .any(|a| a.contains("LOG_FATURA não tem termo no glossário")),
        "{avisos:?}"
    );

    let estado = c.amb.analises.carregar(&a.id).unwrap();
    let d = estado.diagrama(1).unwrap();
    assert_eq!(d.tipo, TipoDiagrama::Dados);
    assert_eq!(d.arestas[2].relacao.unwrap().origem, OrigemRelacao::Join);
    let doc = c.documento(&estado);
    let secao = doc
        .split("## Modelo de dados conceitual")
        .nth(1)
        .and_then(|s| s.split("\n## ").next())
        .unwrap();
    assert!(
        secao.contains("```mermaid\nerDiagram\n  e1[\"Pagador (CLIENTE)\"] {\n    NOME Nome\n"),
        "{secao}"
    );
    // FK contínua, join tracejado.
    assert!(secao.contains("  e1 ||--o{ e2 : \"recebe\"\n"), "{secao}");
    assert!(
        secao.contains("  e2 ||--o{ e3 : \"é dividida em\"\n"),
        "{secao}"
    );
    assert!(secao.contains("  e2 ||..o{ e4 : \"registra\"\n"), "{secao}");
    assert!(
        secao.contains(
            "| Pagador | `CLIENTE` | Nome (`NOME`); Tipo de cliente (`TIPO_CLIENTE`) | Cliente |"
        ),
        "{secao}"
    );
    assert!(secao.contains("| Histórico da fatura | `LOG_FATURA` | Evento (`EVENTO`); Usuário (`USUARIO`) | não está no glossário |"), "{secao}");
    assert!(
        secao.contains(
            "| Fatura → Histórico da fatura (registra) | 1:0..N | inferida (join no código) |"
        ),
        "{secao}"
    );
    assert!(
        secao.contains("| Pagador → Fatura (recebe) | 1:0..N | chave estrangeira |"),
        "{secao}"
    );

    // Nome diferente do glossário: aviso com o termo.
    let mut outro = modelo(&a, "Consumidor");
    outro["substituir"] = json!(1);
    let r = c.chamar("registrar_diagrama", outro).unwrap();
    assert!(
        r["avisos"]
            .to_string()
            .contains("o termo que vive nessa tabela é \\\"Cliente\\\""),
        "{r}"
    );
}

#[test]
fn relacao_inferida_vira_pergunta_sem_duplicar() {
    let c = cenario();
    let a = c.criar();
    c.chamar("registrar_diagrama", modelo(&a, "Cliente"))
        .unwrap();
    let origem = OrigemPergunta::Aresta {
        diagrama: 1,
        de: "fatura".into(),
        para: "historico".into(),
    };
    // O app manda a decisão como a tela: `virar-pergunta`, com a origem.
    let decisao: Decisao = serde_json::from_value(json!({
        "tipo": "virar-pergunta",
        "texto": "O histórico (LOG_FATURA) é sempre de uma fatura? Não há chave estrangeira.",
        "origem": {"tipo": "aresta", "diagrama": 1, "de": "fatura", "para": "historico"}
    }))
    .unwrap();
    let estado = revisao::revisar(&c.amb, &a.id, decisao.clone()).unwrap();
    let p = estado.pergunta_de(&origem).unwrap();
    assert_eq!(p.estado, EstadoPergunta::Aberta);
    assert_eq!(p.evidencias[0].arquivo.as_str(), PKG);
    assert!(
        c.documento(&estado)
            .contains("O histórico (LOG_FATURA) é sempre de uma fatura?")
    );
    // Não impede a aprovação (pergunta em aberto não é pendência).
    assert!(
        revisao::pendencias(&estado)
            .iter()
            .all(|p| !p.motivo.contains("pergunta")),
        "{:?}",
        revisao::pendencias(&estado)
    );
    // Sem duplicar.
    let erro = revisao::revisar(&c.amb, &a.id, decisao).unwrap_err();
    assert!(erro.to_string().contains("já virou a pergunta 1"), "{erro}");
    assert_eq!(c.amb.analises.carregar(&a.id).unwrap().perguntas.len(), 1);
}

#[test]
fn concluir_aponta_os_diagramas_que_faltam() {
    let c = cenario();
    let a = c.criar();
    for (secao, texto) in [
        (
            "objetivo",
            "Cancelar uma fatura emitida, com motivo e responsável.",
        ),
        (
            "fluxo_principal",
            "1. O atendente informa a fatura e o motivo na tela de cancelamento.",
        ),
        (
            "regras",
            "Só fatura aberta e sem pagamento pode ser cancelada.",
        ),
    ] {
        c.chamar(
            "afirmar",
            json!({"analise": a.id, "secao": secao, "texto": texto, "confianca": "inferido"}),
        )
        .unwrap();
    }
    let r = c
        .chamar("concluir_analise", json!({"analise": a.id}))
        .unwrap();
    let pendencias = r.to_string();
    assert!(pendencias.contains("não tem o fluxo do processo"), "{r}");
    assert!(
        pendencias.contains("não tem o modelo de dados conceitual"),
        "{r}"
    );

    c.chamar("registrar_diagrama", processo(&a)).unwrap();
    c.chamar("registrar_diagrama", modelo(&a, "Cliente"))
        .unwrap();
    let r = c
        .chamar("concluir_analise", json!({"analise": a.id}))
        .unwrap();
    assert!(
        !r.to_string().contains("não tem o fluxo do processo"),
        "{r}"
    );
    assert!(!r.to_string().contains("modelo de dados conceitual"), "{r}");
}

fn partes_do_docx(caminho: &Path) -> Vec<(String, Vec<u8>)> {
    let bytes = std::fs::read(caminho).unwrap();
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    (0..zip.len())
        .map(|i| {
            let mut f = zip.by_index(i).unwrap();
            let mut conteudo = Vec::new();
            f.read_to_end(&mut conteudo).unwrap();
            (f.name().unwrap().into_owned(), conteudo)
        })
        .collect()
}

/// PNG mínimo (assinatura e IHDR).
fn png(largura: u32) -> Vec<u8> {
    let mut p = b"\x89PNG\r\n\x1a\n".to_vec();
    p.extend_from_slice(&13u32.to_be_bytes());
    p.extend_from_slice(b"IHDR");
    p.extend_from_slice(&largura.to_be_bytes());
    p.extend_from_slice(&240u32.to_be_bytes());
    p.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
    p
}

#[test]
fn os_dois_diagramas_na_exportacao_word() {
    let c = cenario();
    let a = c.criar();
    c.chamar("registrar_diagrama", processo(&a)).unwrap();
    c.chamar("registrar_diagrama", modelo(&a, "Cliente"))
        .unwrap();

    let blocos = revisao::diagramas_para_exportar(&c.amb, &a.id).unwrap();
    assert_eq!(blocos.len(), 2);
    assert!(blocos[0].starts_with("flowchart TD\n  subgraph r1"));
    assert!(blocos[1].starts_with("erDiagram\n"));

    let mut figuras = Figuras::novas();
    for (i, bloco) in blocos.iter().enumerate() {
        figuras
            .inserir(bloco, Figura::nova(png(600 + i as u32), 0, 0).unwrap())
            .unwrap();
    }
    let docx = revisao::exportar_analise(
        &c.amb,
        &a.id,
        &c.dir.path().join("saida"),
        Formato::Word,
        &OpcoesExportacao::default(),
        &figuras,
    )
    .unwrap();
    let partes = partes_do_docx(&docx);
    let midias: Vec<&(String, Vec<u8>)> = partes
        .iter()
        .filter(|(n, _)| n.starts_with("word/media/"))
        .collect();
    assert_eq!(
        midias.len(),
        2,
        "{:?}",
        midias.iter().map(|m| &m.0).collect::<Vec<_>>()
    );
    let documento = partes
        .iter()
        .find(|(n, _)| n == "word/document.xml")
        .map(|(_, x)| String::from_utf8(x.clone()).unwrap())
        .unwrap();
    assert!(documento.contains("Fluxo do processo"));
    assert!(documento.contains("Modelo de dados conceitual"));
    assert!(documento.contains("Figura 1: Cancelamento de fatura"));
    assert!(documento.contains("Figura 2: Modelo de dados do cancelamento"));
    assert!(documento.contains("inferida (join no código)"));
    assert!(!documento.contains("Diagrama não renderizado"));
}
