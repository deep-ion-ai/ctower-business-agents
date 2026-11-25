//! Fluxo "Protótipo de telas" (#381) com o sistema fictício "Faturamento
//! Exemplo": a partir de uma análise de impacto, `iniciar_analise` cria a análise
//! de protótipo ligada à origem, com o pedido do analista (telas, fidelidade e o
//! que deixar de fora) e o aviso quando a origem ainda não foi aprovada;
//! `registrar_tela` e `registrar_elemento_tela` gravam as telas afetadas (nova ou
//! alterada, com o motivo ligado à origem) e os elementos ligados às afirmações,
//! às regras e aos critérios de aceite; o que não tem fundamento fica "a
//! confirmar"; referência que não existe é recusada. `gravar_html_tela` grava o
//! protótipo HTML de cada tela (#385), com os quatro estados, os tokens CSS
//! aprovados ou o wireframe neutro, e recusa script e endereço externo.

use std::path::{Path, PathBuf};

use cba_core::ambiente::Ambiente;
use cba_core::analise::{Analise, EstadoAfirmacao, EstadoAnalise, Fluxo};
use cba_core::analises::NovaAnalise;
use cba_core::config::Config;
use cba_core::dados::DiretorioDados;
use cba_core::prototipo::{Fidelidade, MudancaTela, TipoElemento};
use cba_core::tokens::TokensProjeto;
use cba_kb::frontmatter::{
    Evidencia, FrontmatterAnalise, FrontmatterRegra, Linhas, StatusRegra, TipoDocumento,
};
use cba_kb::{CaminhoRelativo, Documento, Id};
use cba_tools::{ErroChamada, ErroFerramenta, registro_padrao};
use serde_json::{Value, json};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/faturamento-exemplo")
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
    fn chamar(&self, nome: &str, args: Value) -> Result<Value, ErroFerramenta> {
        match registro_padrao().chamar(nome, Ok(&self.amb), Some(args)) {
            Ok(saida) => Ok(saida.dados.expect("resposta estruturada")),
            Err(ErroChamada::Ferramenta(e)) => Err(e),
            Err(e) => panic!("{e}"),
        }
    }

    fn texto(&self, nome: &str, args: Value) -> String {
        registro_padrao()
            .chamar(nome, Ok(&self.amb), Some(args))
            .unwrap()
            .texto
    }

    fn recusa(&self, nome: &str, args: Value) -> String {
        match self.chamar(nome, args) {
            Err(ErroFerramenta::Recusa(m) | ErroFerramenta::Argumentos(m)) => m,
            outro => panic!("esperava recusa, veio {outro:?}"),
        }
    }

    fn analise(&self, id: &str) -> Analise {
        self.amb
            .analises
            .carregar(&Id::analisar(id).unwrap())
            .unwrap()
    }

    /// Análise de impacto A-0001 (D-FAT-003, parcelamento para pessoa física) com
    /// três afirmações (a 3 marcada como incorreta) e uma pergunta em aberto, e a
    /// regra RN-FAT-008 na base.
    fn origem(&self) -> Analise {
        let a = self
            .amb
            .analises
            .criar(NovaAnalise {
                fluxo: Fluxo::Impacto,
                titulo: "Parcelamento para pessoa física".into(),
                entrada: "Permitir que pessoa física parcele faturas em até 3 vezes, com \
                          parcela mínima de R$ 100,00."
                    .into(),
                analista: Some("Ana Exemplo".into()),
                base: None,
            })
            .unwrap();
        let afirmar = |texto: &str, arquivo: &str, inicio: u32, fim: u32, trecho: &str| {
            self.chamar(
                "afirmar",
                json!({
                    "analise": a.id,
                    "secao": "como_e_hoje",
                    "texto": texto,
                    "confianca": "confirmado",
                    "evidencias": [{
                        "arquivo": arquivo,
                        "linha_inicial": inicio,
                        "linha_final": fim,
                        "trecho": trecho
                    }]
                }),
            )
            .unwrap();
        };
        afirmar(
            "Pessoa física não pode parcelar fatura: o backend recusa o pedido.",
            "backend/src/main/java/exemplo/faturamento/ParcelamentoService.java",
            30,
            32,
            "if (fatura.tipoCliente() == TipoCliente.PF) {",
        );
        afirmar(
            "O valor mínimo da parcela vem do parâmetro VALOR_MIN_PARCELA (hoje R$ 50,00).",
            "db/02_parametros.sql",
            18,
            19,
            "'VALOR_MIN_PARCELA', 50.0000",
        );
        afirmar(
            "O parcelamento aceita até 12 parcelas.",
            "db/02_parametros.sql",
            15,
            16,
            "'MAX_PARCELAS', 6.0000",
        );
        self.chamar(
            "perguntar",
            json!({
                "analise": a.id,
                "texto": "A parcela mínima de R$ 100,00 vale só para pessoa física?"
            }),
        )
        .unwrap();
        self.amb
            .analises
            .alterar(&a.id, cba_core::eventos::Mudanca::Afirmacao, |an, _| {
                an.marcar_afirmacao(1, EstadoAfirmacao::Confirmada)?;
                an.marcar_afirmacao(
                    3,
                    EstadoAfirmacao::Incorreta {
                        comentario: "o limite é 6".into(),
                    },
                )
            })
            .unwrap();
        // A regra RN-FAT-008, como a aprovação de outra análise a deixaria.
        let regra = Id::analisar("RN-FAT-008").unwrap();
        self.amb
            .kb
            .gravar(
                &regra.arquivo(),
                &Documento::novo(
                    FrontmatterRegra {
                        tipo: TipoDocumento::Regra,
                        id: regra.clone(),
                        titulo: "Quantidade de parcelas entre 2 e MAX_PARCELAS".into(),
                        area: "parcelamento".into(),
                        status: StatusRegra::Confirmada,
                        evidencias: vec![Evidencia {
                            arquivo: CaminhoRelativo::novo("db/02_parametros.sql").unwrap(),
                            linhas: Linhas::novo(15, 16).unwrap(),
                            hash: "h".into(),
                            commit: None,
                        }],
                        origem: None,
                        informada: None,
                        historico: Vec::new(),
                        funcionalidades: Vec::new(),
                    },
                    "\n# RN-FAT-008\n",
                ),
            )
            .unwrap();
        self.analise(&a.id.to_string())
    }
}

impl Cenario {
    fn aprovar(&self, id: &Id) {
        self.amb
            .analises
            .alterar(id, cba_core::eventos::Mudanca::Estado, |a, _| {
                a.estado = EstadoAnalise::Aprovada;
                Ok(())
            })
            .unwrap();
    }

    /// Análise de histórias da origem, com a HU-0001 e dois critérios (CA-2
    /// descartado pelo analista).
    fn historias(&self, origem: &Id) -> Id {
        let r = self
            .chamar(
                "iniciar_analise",
                json!({ "fluxo": "historias", "origem": origem }),
            )
            .unwrap();
        let id = r["analise"].as_str().unwrap().to_owned();
        self.chamar(
            "registrar_historia",
            json!({
                "analise": id,
                "titulo": "Parcelar fatura de pessoa física em até 3 vezes",
                "como": "atendente do faturamento",
                "quero": "parcelar a fatura em aberto de um cliente pessoa física",
                "para": "reduzir a inadimplência",
                "criterios": [
                    {
                        "dado": "uma pessoa física com uma fatura em aberto de R$ 300,00",
                        "quando": "ela pede o parcelamento em 3 vezes",
                        "entao": "são geradas 3 parcelas de R$ 100,00",
                        "regras": ["RN-FAT-008"]
                    },
                    {
                        "dado": "uma pessoa física com uma fatura de R$ 150,00",
                        "quando": "ela pede 3 parcelas",
                        "entao": "o pedido é recusado",
                        "afirmacoes": [2]
                    }
                ]
            }),
        )
        .unwrap();
        let id = Id::analisar(&id).unwrap();
        self.amb
            .analises
            .alterar(&id, cba_core::eventos::Mudanca::Historia, |a, _| {
                a.decidir_criterio(
                    cba_kb::IdHistoria::novo(1),
                    2,
                    cba_core::requisito::SituacaoItem::Descartado,
                )
            })
            .unwrap();
        id
    }
}

fn tela(analise: &str, nome: &str, elementos: Value) -> Value {
    json!({
        "analise": analise,
        "nome": nome,
        "mudanca": "alterada",
        "motivo": "O parcelamento passa a aceitar pessoa física",
        "afirmacoes": [1],
        "elementos": elementos
    })
}

#[test]
fn prototipo_a_partir_de_uma_analise_de_impacto() {
    let c = cenario();
    let origem = c.origem();

    // Origem ainda não aprovada: a análise é criada, com aviso.
    let r = c
        .chamar(
            "iniciar_analise",
            json!({
                "fluxo": "prototipo",
                "origem": origem.id,
                "telas": ["Parcelar fatura", " parcelar fatura "],
                "fidelidade": "alta_fidelidade",
                "fora": "o extrato de parcelas"
            }),
        )
        .unwrap();
    assert_eq!(r["analise"], "A-0002");
    assert_eq!(r["fluxo"], "prototipo");
    assert_eq!(r["origem"], "A-0001");
    let aviso = r["avisos"][0].as_str().unwrap();
    assert!(aviso.contains("A-0001 ainda não foi aprovada"), "{aviso}");
    assert!(
        r["roteiro"].as_str().unwrap().contains("registrar_tela"),
        "{}",
        r["roteiro"]
    );
    let a = c.analise("A-0002");
    assert_eq!(a.fluxo, Fluxo::Prototipo);
    assert_eq!(a.origem, Some(origem.id.clone()));
    let pedido = &a.prototipo.as_ref().unwrap().pedido;
    assert_eq!(pedido.telas, ["Parcelar fatura"]);
    assert_eq!(pedido.fidelidade, Fidelidade::AltaFidelidade);
    assert_eq!(pedido.fora.as_deref(), Some("o extrato de parcelas"));
    assert!(
        a.entrada.contains("Fidelidade: alta fidelidade.") && a.entrada.contains("A-0001"),
        "{}",
        a.entrada
    );
    assert!(a.telas().is_empty());
    let doc: Documento<FrontmatterAnalise> = c.amb.kb.ler(&a.documento).unwrap();
    assert_eq!(doc.frontmatter.tipo, TipoDocumento::Prototipo);
    assert_eq!(doc.frontmatter.origem, Some(origem.id.clone()));

    // Uma análise de histórias da mesma origem, cujos critérios a tela cita.
    let hu = c.historias(&origem.id);
    assert_eq!(hu.to_string(), "A-0003");

    let r = c
        .chamar(
            "registrar_tela",
            tela(
                "A-0002",
                "Parcelar fatura",
                json!([
                    {
                        "tipo": "campo",
                        "nome": "Número de parcelas",
                        "descricao": "de 2 a 3",
                        "regras": ["RN-FAT-008"],
                        "criterios": ["HU-0001/CA-1"]
                    },
                    {
                        "tipo": "mensagem",
                        "nome": "Parcela abaixo do mínimo",
                        "descricao": "A parcela mínima é de R$ 100,00.",
                        "afirmacoes": ["A-0001/2"],
                        "pergunta": 1
                    },
                    { "tipo": "acao", "nome": "Simular parcelas" }
                ]),
            ),
        )
        .unwrap();
    assert_eq!(r["tela"], "TL-1");
    assert_eq!(r["elementos"], json!(["E-1", "E-2", "E-3"]));
    assert_eq!(r["a_confirmar"], json!(["TL-1/E-2", "TL-1/E-3"]));
    let r = c
        .chamar(
            "registrar_elemento_tela",
            json!({
                "analise": "A-0002",
                "tela": "TL-1",
                "tipo": "validacao",
                "nome": "Só pessoa física",
                "afirmacoes": [1]
            }),
        )
        .unwrap();
    assert_eq!(r["elemento"], "E-4");
    assert_eq!(r["a_confirmar"], false);
    // Tela nova sem fundamento: aceita, a confirmar.
    let r = c
        .chamar(
            "registrar_tela",
            json!({
                "analise": "A-0002",
                "nome": "Consultar parcelas",
                "mudanca": "nova",
                "motivo": "Ver as parcelas geradas"
            }),
        )
        .unwrap();
    assert_eq!(r["tela"], "TL-2");
    assert_eq!(r["a_confirmar"], json!(["TL-2 (motivo)"]));

    let a = c.analise("A-0002");
    assert_eq!(a.telas().len(), 2);
    let t1 = a.tela(1).unwrap();
    assert_eq!(t1.mudanca, MudancaTela::Alterada);
    assert!(t1.html.is_none());
    assert_eq!(t1.elementos[0].tipo, TipoElemento::Campo);
    assert_eq!(
        t1.elementos[0].rastro.rotulos(),
        ["RN-FAT-008", "HU-0001/CA-1"]
    );
    assert_eq!(t1.elementos[0].rastro.criterios[0].analise, hu);
    assert!(
        t1.elementos[0].rastro.criterios[0]
            .texto
            .starts_with("Dado uma pessoa física"),
    );
    assert!(t1.elementos[1].a_confirmar());
    assert!(t1.elementos[2].a_confirmar());
    assert!(a.tela(2).unwrap().a_confirmar());

    // Recusas: nada é gravado.
    let m = c.recusa(
        "registrar_tela",
        tela(
            "A-0002",
            "Outra",
            json!([{ "tipo": "campo", "nome": "X", "afirmacoes": [3] }]),
        ),
    );
    assert!(m.contains("incorreta"), "{m}");
    let m = c.recusa(
        "registrar_tela",
        tela(
            "A-0002",
            "Outra",
            json!([{ "tipo": "campo", "nome": "X", "criterios": ["HU-0009/CA-1"] }]),
        ),
    );
    assert!(m.contains("HU-0009 não existe"), "{m}");
    let m = c.recusa(
        "registrar_tela",
        tela(
            "A-0002",
            "Outra",
            json!([{ "tipo": "campo", "nome": "X", "criterios": ["HU-0001/CA-2"] }]),
        ),
    );
    assert!(m.contains("descartado"), "{m}");
    let m = c.recusa(
        "registrar_tela",
        tela(
            "A-0002",
            "Outra",
            json!([{ "tipo": "campo", "nome": "X", "criterios": ["HU-0001/CA-9"] }]),
        ),
    );
    assert!(m.contains("CA-9 não existe"), "{m}");
    let m = c.recusa(
        "registrar_tela",
        tela(
            "A-0002",
            "Outra",
            json!([{ "tipo": "campo", "nome": "X", "criterios": ["HU-0001"] }]),
        ),
    );
    assert!(m.contains("HU-NNNN/CA-n"), "{m}");
    let m = c.recusa(
        "registrar_tela",
        tela("A-0002", "PARCELAR fatura", json!([])),
    );
    assert!(m.contains("TL-1"), "{m}");
    let m = c.recusa(
        "registrar_tela",
        tela(&hu.to_string(), "Parcelar fatura", json!([])),
    );
    assert!(m.contains("não é do fluxo `prototipo`"), "{m}");
    let m = c.recusa(
        "registrar_elemento_tela",
        json!({ "analise": "A-0002", "tela": "TL-9", "tipo": "campo", "nome": "X" }),
    );
    assert!(m.contains("tela"), "{m}");
    let m = c.recusa(
        "afirmar",
        json!({
            "analise": "A-0002", "secao": "telas", "texto": "x", "confianca": "inferido"
        }),
    );
    assert!(m.contains("registrar_tela"), "{m}");
    assert_eq!(c.analise("A-0002").telas().len(), 2);

    // O documento segue o template, com a rastreabilidade.
    let texto = std::fs::read_to_string(c.amb.kb.caminho(&a.documento)).unwrap();
    assert!(texto.contains("## Telas afetadas"), "{texto}");
    assert!(
        texto.contains("### TL-1 · Parcelar fatura (alterada)"),
        "{texto}"
    );
    assert!(
        texto.contains("- **E-3** · ação · Simular parcelas · _a confirmar: sem fundamento na análise de origem_"),
        "{texto}"
    );
    assert!(
        texto.contains("| TL-1 | E-1 (campo) | RN-FAT-008, HU-0001/CA-1 |  |"),
        "{texto}"
    );
    assert!(texto.contains("Fidelidade: alta fidelidade."), "{texto}");

    // A retomada traz o pedido e as telas.
    let t = c.texto("retomar_analise", json!({ "analise": "A-0002" }));
    assert!(t.contains("## Protótipo"), "{t}");
    assert!(t.contains("### TL-2 · Consultar parcelas [nova]"), "{t}");

    // Concluir exige elementos em toda tela.
    let m = c.recusa("concluir_analise", json!({ "analise": "A-0002" }));
    assert!(m.contains("TL-2"), "{m}");
    c.chamar(
        "registrar_elemento_tela",
        json!({ "analise": "A-0002", "tela": 2, "tipo": "campo", "nome": "Parcela" }),
    )
    .unwrap();
    // E o protótipo HTML de toda tela.
    let m = c.recusa("concluir_analise", json!({ "analise": "A-0002" }));
    assert!(m.contains("TL-1: sem protótipo HTML"), "{m}");
    c.chamar(
        "gravar_html_tela",
        json!({ "analise": "A-0002", "tela": "TL-1", "corpo": corpo(&[1, 2, 3, 4]) }),
    )
    .unwrap();
    c.chamar(
        "gravar_html_tela",
        json!({ "analise": "A-0002", "tela": "TL-2", "corpo": corpo(&[1]) }),
    )
    .unwrap();
    let r = c
        .chamar("concluir_analise", json!({ "analise": "A-0002" }))
        .unwrap();
    assert_eq!(r["estado"], "concluida");
}

/// Corpo de tela com os quatro estados e os elementos dados marcados.
fn corpo(elementos: &[u32]) -> String {
    let marcas: String = elementos
        .iter()
        .map(|n| format!("<p class=\"mudou\" data-elemento=\"E-{n}\">E-{n}</p>"))
        .collect();
    format!(
        "<section data-estado=\"vazio\"><form>{marcas}</form></section>\n\
         <section data-estado=\"preenchido\"><p>3 parcelas</p></section>\n\
         <section data-estado=\"erro\"><p class=\"erro\">Parcela abaixo do mínimo</p></section>\n\
         <section data-estado=\"sucesso\"><p class=\"sucesso\">Parcelamento registrado</p></section>\n"
    )
}

#[test]
fn html_das_telas_com_estados_tokens_e_sem_rede() {
    let c = cenario();
    let origem = c.origem();
    c.aprovar(&origem.id);
    let r = c
        .chamar(
            "iniciar_analise",
            json!({ "fluxo": "prototipo", "origem": origem.id, "fidelidade": "alta_fidelidade" }),
        )
        .unwrap();
    let id = r["analise"].as_str().unwrap().to_owned();
    c.chamar(
        "registrar_tela",
        tela(
            &id,
            "Parcelar fatura",
            json!([
                {
                    "tipo": "campo",
                    "nome": "Número de parcelas",
                    "descricao": "de 2 a 3",
                    "regras": ["RN-FAT-008"]
                },
                {
                    "tipo": "mensagem",
                    "nome": "Parcela abaixo do mínimo",
                    "descricao": "A parcela mínima é de R$ 100,00.",
                    "pergunta": 1
                }
            ]),
        ),
    )
    .unwrap();

    let corpo_tela = r#"<section data-estado="vazio">
  <form>
    <label for="parcelas">Número de parcelas</label>
    <input id="parcelas" class="mudou" data-elemento="E-1" placeholder="2 a 3">
    <div class="acoes"><button type="button">Simular parcelas</button></div>
  </form>
</section>
<section data-estado="preenchido">
  <input data-elemento="E-1" value="3"> <a href="TL-2.html">Consultar parcelas</a>
</section>
<section data-estado="erro">
  <input class="campo-erro" value="3">
  <p class="erro" data-elemento="E-2">A parcela mínima é de R$ 100,00.</p>
</section>
<section data-estado="sucesso"><p class="sucesso">Parcelamento registrado em 3 vezes.</p></section>"#;

    // Sem tokens aprovados: wireframe neutro, com aviso.
    let r = c
        .chamar(
            "gravar_html_tela",
            json!({ "analise": id, "tela": "TL-1", "corpo": corpo_tela }),
        )
        .unwrap();
    assert_eq!(r["tela"], "TL-1");
    assert_eq!(r["visual"], "sem-tokens");
    assert_eq!(r["html"], format!("analises/{id}/prototipo/TL-1.html"));
    assert_eq!(r["elementos_marcados"], json!(["E-1", "E-2"]));
    assert!(
        r["avisos"].to_string().contains("não estão aprovados"),
        "{r}"
    );
    let a = c.analise(&id);
    let caminho = a.tela(1).unwrap().html.clone().unwrap();
    let html = std::fs::read_to_string(c.amb.kb.caminho(&caminho)).unwrap();
    assert!(html.contains("data-visual=\"sem-tokens\""), "{html}");
    assert!(html.contains("<section id=\"estado-erro\" data-estado=\"erro\">"));
    assert!(html.contains("var(--prototipo-cor-primaria, #5c5c5c)"));
    assert!(!html.contains("--prototipo-cor-primaria:"));
    assert!(html.contains("<strong>A confirmar:</strong> E-2 Parcela abaixo do mínimo"));

    // O documento aponta para o HTML.
    let texto = std::fs::read_to_string(c.amb.kb.caminho(&a.documento)).unwrap();
    assert!(
        texto.contains(&format!(
            "**Protótipo:** [TL-1.html]({id}/prototipo/TL-1.html)"
        )),
        "{texto}"
    );

    // Script, handler e endereço externo são recusados, e nada muda.
    let m = c.recusa(
        "gravar_html_tela",
        json!({
            "analise": id,
            "tela": 1,
            "corpo": corpo_tela.replace(
                "<p class=\"sucesso\">",
                "<script>fetch('https://exemplo.invalid')</script>\
                 <img src=\"https://exemplo.invalid/logo.png\" onerror=\"x()\"><p class=\"sucesso\">"
            )
        }),
    );
    assert!(m.contains("`<script>`"), "{m}");
    assert!(m.contains("`onerror`"), "{m}");
    assert!(m.contains("https://exemplo.invalid/logo.png"), "{m}");
    let m = c.recusa(
        "gravar_html_tela",
        json!({
            "analise": id,
            "tela": 1,
            "corpo": corpo_tela.replace("data-estado=\"sucesso\"", "data-estado=\"pronto\"")
        }),
    );
    assert!(m.contains("falta a seção do(s) estado(s) sucesso"), "{m}");
    let m = c.recusa(
        "gravar_html_tela",
        json!({ "analise": id, "tela": "TL-7", "corpo": corpo_tela }),
    );
    assert!(m.contains("tela"), "{m}");
    assert_eq!(
        std::fs::read_to_string(c.amb.kb.caminho(&caminho)).unwrap(),
        html
    );

    // Tokens aprovados pelo analista: o CSS deles vai inline.
    TokensProjeto::extrair(&c.amb.kb, &c.amb.escopo).unwrap();
    TokensProjeto::alterar(&c.amb.kb, |t, agora| {
        t.aprovar(agora);
        Ok(())
    })
    .unwrap();
    let r = c
        .chamar(
            "gravar_html_tela",
            json!({ "analise": id, "tela": "TL-1", "corpo": corpo_tela }),
        )
        .unwrap();
    assert_eq!(r["visual"], "tokens");
    assert!(
        r["avisos"].to_string().contains("--prototipo-cor-sucesso"),
        "{r}"
    );
    let html = std::fs::read_to_string(c.amb.kb.caminho(&caminho)).unwrap();
    assert!(html.contains("data-visual=\"tokens\""));
    assert!(
        html.contains("--prototipo-cor-primaria: var(--cor-primaria);"),
        "{html}"
    );
    assert!(html.contains("--cor-primaria: #1f5fa8;"), "{html}");
    assert!(
        html.contains("/* --prototipo-cor-sucesso: a confirmar"),
        "{html}"
    );
    assert!(
        !html.contains("http:") && !html.contains("https:"),
        "{html}"
    );

    let r = c
        .chamar("concluir_analise", json!({ "analise": id }))
        .unwrap();
    assert_eq!(r["estado"], "concluida");

    // Wireframe com tokens aprovados: a mesma identidade, só outro acabamento.
    let r = c
        .chamar(
            "iniciar_analise",
            json!({ "fluxo": "prototipo", "origem": origem.id, "fidelidade": "wireframe" }),
        )
        .unwrap();
    let wire = r["analise"].as_str().unwrap().to_owned();
    c.chamar(
        "registrar_tela",
        tela(
            &wire,
            "Parcelar fatura",
            json!([{ "tipo": "campo", "nome": "Número de parcelas", "regras": ["RN-FAT-008"] }]),
        ),
    )
    .unwrap();
    let r = c
        .chamar(
            "gravar_html_tela",
            json!({ "analise": wire, "tela": "TL-1", "corpo": corpo(&[1]) }),
        )
        .unwrap();
    assert_eq!(r["visual"], "tokens");
    let html = std::fs::read_to_string(c.amb.kb.caminho(&cba_kb::layout::html_da_tela(
        &Id::analisar(&wire).unwrap(),
        1,
    )))
    .unwrap();
    assert!(html.contains("data-fidelidade=\"wireframe\""), "{html}");
    assert!(html.contains("--cor-primaria: #1f5fa8;"), "{html}");

    // Aprovada, não aceita HTML novo.
    c.aprovar(&Id::analisar(&id).unwrap());
    let m = c.recusa(
        "gravar_html_tela",
        json!({ "analise": id, "tela": "TL-1", "corpo": corpo_tela }),
    );
    assert!(m.contains("aprovada"), "{m}");
}

#[test]
fn origem_aprovada_nao_avisa() {
    let c = cenario();
    let origem = c.origem();
    c.aprovar(&origem.id);
    let r = c
        .chamar(
            "iniciar_analise",
            json!({ "fluxo": "prototipo", "origem": origem.id }),
        )
        .unwrap();
    assert!(
        r.get("avisos")
            .is_none_or(|a| !a.to_string().contains("ainda não foi aprovada"))
    );
    let a = c.analise(r["analise"].as_str().unwrap());
    // Sem pedido do analista: wireframe, sem telas pedidas.
    let pedido = &a.prototipo.as_ref().unwrap().pedido;
    assert_eq!(pedido.fidelidade, Fidelidade::Wireframe);
    assert!(pedido.telas.is_empty());
    assert_eq!(a.titulo, "Protótipo · Parcelamento para pessoa física");
}

#[test]
fn criacao_recusada() {
    let c = cenario();
    let origem = c.origem();
    let m = c.recusa("iniciar_analise", json!({ "fluxo": "prototipo" }));
    assert!(m.contains("informe `origem`"), "{m}");
    let m = c.recusa(
        "iniciar_analise",
        json!({ "fluxo": "historias", "origem": origem.id, "fidelidade": "wireframe" }),
    );
    assert!(m.contains("`prototipo`"), "{m}");
    let r = c
        .chamar(
            "iniciar_analise",
            json!({ "fluxo": "prototipo", "origem": origem.id }),
        )
        .unwrap();
    let m = c.recusa(
        "iniciar_analise",
        json!({ "fluxo": "prototipo", "origem": r["analise"] }),
    );
    assert!(m.contains("a origem do protótipo"), "{m}");
    let m = c.recusa(
        "iniciar_analise",
        json!({ "fluxo": "prototipo", "origem": "A-0099" }),
    );
    assert!(m.contains("A-0099"), "{m}");
}
