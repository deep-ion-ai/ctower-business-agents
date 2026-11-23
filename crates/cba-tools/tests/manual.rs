//! Manual do usuário (#272) com o sistema fictício "Faturamento Exemplo": o
//! manual de "Cancelar fatura" é aberto a partir do mapa, o agente registra as
//! seções e os passos, o analista inclui um print e dispensa outro, a aprovação
//! grava `manual.md` ao lado dos prints, a exportação leva a imagem com a legenda
//! e, quando o mapeamento muda, o manual fica "a revisar" sem perder os prints.

use std::path::{Path, PathBuf};

use cba_core::ambiente::Ambiente;
use cba_core::analise::EstadoAfirmacao;
use cba_core::config::Config;
use cba_core::dados::DiretorioDados;
use cba_core::funcionalidades::{DecisaoFuncionalidade, MapaFuncionalidades};
use cba_index::PontoEntrada;
use cba_kb::frontmatter::{FrontmatterFuncionalidade, TipoDocumento};
use cba_kb::{CaminhoRelativo, Id, IdFuncionalidade};
use cba_tools::aprovacao::OpcoesAprovacao;
use cba_tools::manual::{self, NovoPrint, SituacaoManual};
use cba_tools::mapear::{RevisaoDoMapa, revisar_mapa};
use cba_tools::revisao::{self, Decisao, Formato, OpcoesExportacao};
use cba_tools::{ErroChamada, ErroFerramenta, registro_padrao};
use serde_json::{Value, json};

/// PNG 1×1 válido.
const PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
    0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE,
    0x42, 0x60, 0x82,
];

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/faturamento-exemplo")
}

struct Cenario {
    dir: tempfile::TempDir,
    amb: Ambiente,
    pontos: Vec<PontoEntrada>,
}

fn cenario() -> Cenario {
    let dir = tempfile::tempdir().unwrap();
    let raiz = dir.path();
    let dados = DiretorioDados::em(raiz.join("dados"));
    let projeto = cba_index::Projeto::novo("Faturamento Exemplo", [fixture()]).unwrap();
    let cadastro = cba_index::Cadastro::em(dados.projetos());
    cadastro.cadastrar(&projeto).unwrap();
    cadastro.inventariar(&projeto.id).unwrap();
    let pontos = cadastro.mapear(&projeto.id).unwrap().pontos_de_entrada();
    cba_kb::Base::criar(raiz.join("base")).unwrap();
    Config {
        base: Some(raiz.join("base")),
        ..Default::default()
    }
    .gravar(&dados)
    .unwrap();
    let amb = Ambiente::do_diretorio(&fixture(), &dados).unwrap();
    Cenario { dir, amb, pontos }
}

impl Cenario {
    fn chamar(&self, ferramenta: &str, args: Value) -> Result<Value, ErroFerramenta> {
        match registro_padrao().chamar(ferramenta, Ok(&self.amb), Some(args)) {
            Ok(saida) => Ok(saida.dados.expect("resposta estruturada")),
            Err(ErroChamada::Ferramenta(e)) => Err(e),
            Err(e) => panic!("{e}"),
        }
    }

    fn ok(&self, ferramenta: &str, args: Value) -> Value {
        self.chamar(ferramenta, args)
            .unwrap_or_else(|e| panic!("{ferramenta}: {e}"))
    }

    fn recusa(&self, ferramenta: &str, args: Value) -> String {
        match self.chamar(ferramenta, args) {
            Err(ErroFerramenta::Recusa(m) | ErroFerramenta::Argumentos(m)) => m,
            outro => panic!("esperava recusa, veio {outro:?}"),
        }
    }

    fn ponto(&self, sufixo: &str) -> Value {
        let p = self
            .pontos
            .iter()
            .find(|p| p.chave.ends_with(sufixo))
            .unwrap_or_else(|| panic!("ponto {sufixo}"));
        let texto = std::fs::read_to_string(fixture().join(&p.arquivo)).unwrap();
        let linha = texto
            .lines()
            .nth(p.linhas.inicio as usize - 1)
            .unwrap()
            .trim();
        json!({
            "ponto": p.chave,
            "evidencia": {
                "arquivo": p.arquivo,
                "linha_inicial": p.linhas.inicio,
                "linha_final": p.linhas.inicio,
                "trecho": linha,
            }
        })
    }

    fn cancelar_fatura(&self, id: Option<&str>) {
        let mut args = json!({
            "nome": "Cancelar fatura",
            "area": "Cancelamento",
            "descricao": "O atendente cancela uma fatura aberta, informando o motivo.",
            "atores": ["Atendente"],
            "pontos_de_entrada": [self.ponto("#FrmCancelamento"), self.ponto("PKG_COBRANCA.PRC_CANCELAR_FATURA")],
            "dados_lidos": ["FATURA"],
        });
        if let Some(id) = id {
            args["id"] = json!(id);
        }
        self.ok("registrar_funcionalidade", args);
    }

    fn aceitar(&self) {
        let m = MapaFuncionalidades::carregar(&self.amb.kb).unwrap();
        for f in m.abertas() {
            revisar_mapa(
                &self.amb.base,
                &self.amb.kb,
                &self.amb.dados,
                RevisaoDoMapa::Decidir {
                    funcionalidade: f.id.clone(),
                    decisao: DecisaoFuncionalidade::Aceitar,
                },
            )
            .unwrap();
        }
    }

    fn funcionalidade(&self) -> cba_core::funcionalidades::Funcionalidade {
        MapaFuncionalidades::carregar(&self.amb.kb)
            .unwrap()
            .funcionalidade(&fid())
            .unwrap()
            .clone()
    }
}

fn fid() -> IdFuncionalidade {
    IdFuncionalidade::analisar("F-CAN-001").unwrap()
}

fn print(legenda: &str) -> NovoPrint {
    NovoPrint {
        png: PNG.to_vec(),
        legenda: legenda.into(),
        texto_alternativo: format!("Print: {legenda}"),
    }
}

fn mapeado() -> Cenario {
    let c = cenario();
    c.cancelar_fatura(None);
    c
}

#[test]
fn manual_de_cancelar_fatura_com_print_aprovado_e_exportado() {
    let c = mapeado();
    // Funcionalidade ainda não aceita: o manual espera a revisão do mapa.
    let m = c.recusa(
        "iniciar_analise",
        json!({ "fluxo": "manual", "funcionalidade": "F-CAN-001" }),
    );
    assert!(m.contains("ainda não foi aceita"), "{m}");
    c.aceitar();

    let r = c.ok(
        "iniciar_analise",
        json!({ "fluxo": "manual", "funcionalidade": "F-CAN-001" }),
    );
    let id = Id::analisar(r["analise"].as_str().unwrap()).unwrap();
    let a = c.amb.analises.carregar(&id).unwrap();
    assert_eq!(a.funcionalidade, Some(fid()));
    assert!(
        a.entrada.contains("Telas (prints sugeridos dos passos)"),
        "{}",
        a.entrada
    );
    assert!(a.entrada.contains("FrmCancelamento"), "{}", a.entrada);
    // Só um manual em andamento por funcionalidade.
    let m = c.recusa(
        "iniciar_analise",
        json!({ "fluxo": "manual", "funcionalidade": "F-CAN-001" }),
    );
    assert!(m.contains("em andamento"), "{m}");

    // O manual não recebe seções de outro fluxo.
    let m = c.recusa(
        "afirmar",
        json!({ "analise": id, "secao": "regras", "texto": "x", "confianca": "inferido" }),
    );
    assert!(m.contains("quem_usa"), "{m}");
    for (secao, texto) in [
        (
            "objetivo",
            "Cancelar uma fatura lançada por engano, informando o motivo.",
        ),
        ("quem_usa", "Atendentes da central."),
        (
            "mensagens_e_erros",
            "**Fatura já paga:** a tela recusa o cancelamento.",
        ),
    ] {
        c.ok(
            "afirmar",
            json!({ "analise": id, "secao": secao, "texto": texto, "confianca": "inferido" }),
        );
    }
    // Sem passos, não conclui.
    let m = c.recusa("concluir_analise", json!({ "analise": id }));
    assert!(m.contains("registrar_passo_manual"), "{m}");
    for (titulo, tela) in [
        (
            "Abra a tela de cancelamento",
            Some("Cancelamento de fatura"),
        ),
        ("Informe o motivo e confirme", None),
    ] {
        let mut args = json!({
            "analise": id,
            "titulo": titulo,
            "instrucao": "Escolha a fatura e clique em **Cancelar**.",
            "resultado": "A fatura aparece como cancelada.",
        });
        if let Some(t) = tela {
            args["tela"] = json!(t);
        }
        c.ok("registrar_passo_manual", args);
    }
    // Reescreve o passo 2 pelo número.
    let r = c.ok(
        "registrar_passo_manual",
        json!({ "analise": id, "numero": 2, "titulo": "Informe o motivo", "instrucao": "Digite o motivo." }),
    );
    assert_eq!(r["passo"], 2);
    assert_eq!(r["passos"], 2);
    // Ferramenta só no fluxo manual.
    let outra = c.ok(
        "iniciar_analise",
        json!({ "fluxo": "perguntar", "entrada": "Quem cancela fatura?" }),
    );
    let m = c.recusa(
        "registrar_passo_manual",
        json!({ "analise": outra["analise"], "titulo": "x", "instrucao": "y" }),
    );
    assert!(m.contains("não é do fluxo `manual`"), "{m}");

    c.ok("concluir_analise", json!({ "analise": id }));
    let doc = std::fs::read_to_string(c.amb.kb.caminho(&id.arquivo())).unwrap();
    assert!(doc.contains("tipo: manual"), "{doc}");
    assert!(doc.contains("## Passo a passo"), "{doc}");
    assert!(
        doc.contains("> **Print sugerido:** tela Cancelamento de fatura"),
        "{doc}"
    );

    // Confirma as afirmações; ainda falta o print de cada passo.
    let a = c.amb.analises.carregar(&id).unwrap();
    for af in &a.afirmacoes {
        revisao::revisar(
            &c.amb,
            &id,
            Decisao::Afirmacao {
                numero: af.numero,
                estado: EstadoAfirmacao::Confirmada,
            },
        )
        .unwrap();
    }
    let p = revisao::prontidao(&c.amb, &id).unwrap();
    assert!(!p.pode_aprovar);
    let motivos: Vec<&str> = p.pendencias.iter().map(|p| p.motivo.as_str()).collect();
    assert_eq!(motivos.len(), 2, "{motivos:?}");
    assert!(motivos[0].contains("passo 1: sem print"), "{motivos:?}");
    assert!(
        revisao::aprovar_revisada(&c.amb, &id, &OpcoesAprovacao::default()).is_err(),
        "não aprova com print pendente"
    );

    // Print inválido é recusado.
    let erro = manual::anexar_print(
        &c.amb,
        &id,
        1,
        NovoPrint {
            png: b"GIF89a".to_vec(),
            ..print("x")
        },
    )
    .unwrap_err();
    assert!(erro.to_string().contains("PNG"), "{erro}");
    // O analista inclui o print do passo 1 e dispensa o do passo 2.
    let a = manual::anexar_print(&c.amb, &id, 1, print("Tela de cancelamento")).unwrap();
    let arquivo = a.passos[0].print.as_ref().unwrap().arquivo.clone();
    assert!(
        arquivo
            .as_str()
            .starts_with("funcionalidades/cancelamento/cancelar-fatura/manual/passo-01-"),
        "{arquivo}"
    );
    assert!(c.amb.kb.existe(&arquivo));
    revisao::revisar(
        &c.amb,
        &id,
        Decisao::DispensarPrint {
            numero: 2,
            dispensado: true,
        },
    )
    .unwrap();
    let doc = std::fs::read_to_string(c.amb.kb.caminho(&id.arquivo())).unwrap();
    assert!(
        doc.contains(&format!(
            "![Print: Tela de cancelamento](../{})",
            arquivo.as_str()
        )),
        "{doc}"
    );
    assert!(doc.contains("_Figura 1. Tela de cancelamento_"), "{doc}");
    let p = revisao::prontidao(&c.amb, &id).unwrap();
    assert!(p.pode_aprovar, "{:?}", p.pendencias);

    // Aprovação: manual.md na pasta da funcionalidade, com o print ao lado.
    let feita = revisao::aprovar_revisada(&c.amb, &id, &OpcoesAprovacao::default()).unwrap();
    assert!(
        feita.regras.is_empty(),
        "o manual não cria regras por padrão"
    );
    let destino =
        CaminhoRelativo::novo("funcionalidades/cancelamento/cancelar-fatura/manual.md").unwrap();
    assert_eq!(feita.documento_negocial.as_ref(), Some(&destino));
    let aprovado = c.amb.kb.ler::<FrontmatterFuncionalidade>(&destino).unwrap();
    assert_eq!(aprovado.frontmatter.tipo, TipoDocumento::Manual);
    assert_eq!(aprovado.frontmatter.funcionalidade, Some(fid()));
    let nome = arquivo.as_str().rsplit('/').next().unwrap();
    assert!(
        aprovado.corpo.contains(&format!("](manual/{nome})")),
        "{}",
        aprovado.corpo
    );

    // Exportação: Markdown com a imagem copiada ao lado, HTML e Word embutidos.
    let saida = c.dir.path().join("saida");
    std::fs::create_dir_all(&saida).unwrap();
    let opcoes = OpcoesExportacao::default();
    let md = revisao::exportar_analise(
        &c.amb,
        &id,
        &saida.join("manual.md"),
        Formato::Markdown,
        &opcoes,
        &cba_docs::Figuras::default(),
    )
    .unwrap();
    let texto = std::fs::read_to_string(&md).unwrap();
    assert!(
        texto.contains(&format!("](manual-imagens/{nome})")),
        "{texto}"
    );
    assert!(texto.contains("Figura 1. Tela de cancelamento"), "{texto}");
    assert_eq!(
        std::fs::read(saida.join("manual-imagens").join(nome)).unwrap(),
        PNG
    );
    let html = revisao::exportar_analise(
        &c.amb,
        &id,
        &saida.join("manual.html"),
        Formato::Html,
        &opcoes,
        &cba_docs::Figuras::default(),
    )
    .unwrap();
    let html = std::fs::read_to_string(html).unwrap();
    assert!(
        html.contains("<img class=\"print\" alt=\"Print: Tela de cancelamento\""),
        "{html}"
    );
    let docx = revisao::exportar_analise(
        &c.amb,
        &id,
        &saida.join("manual.docx"),
        Formato::Word,
        &opcoes,
        &cba_docs::Figuras::default(),
    )
    .unwrap();
    let mut zip = zip::ZipArchive::new(std::fs::File::open(docx).unwrap()).unwrap();
    assert!(zip.by_name("word/media/figura1.png").is_ok());
    let mut corpo = String::new();
    std::io::Read::read_to_string(&mut zip.by_name("word/document.xml").unwrap(), &mut corpo)
        .unwrap();
    assert!(corpo.contains("descr=\"Print: Tela de cancelamento\""));
    assert!(corpo.contains("Figura 1. Tela de cancelamento"));

    // Aprovado e sem mudança: nada a revisar.
    let f = c.funcionalidade();
    match manual::situacao(&c.amb, &f) {
        SituacaoManual::Aprovado {
            a_revisar,
            documento,
            ..
        } => {
            assert!(a_revisar.is_empty(), "{a_revisar:?}");
            assert!(documento.unwrap().ends_with("cancelar-fatura/manual.md"));
        }
        outra => panic!("{outra:?}"),
    }

    // O mapeamento muda: o manual fica a revisar e os prints ficam.
    std::thread::sleep(std::time::Duration::from_millis(5));
    c.cancelar_fatura(Some("F-CAN-001"));
    let f = c.funcionalidade();
    match manual::situacao(&c.amb, &f) {
        SituacaoManual::Aprovado { a_revisar, .. } => {
            assert!(
                a_revisar.iter().any(|m| m.contains("mapeamento")),
                "{a_revisar:?}"
            );
        }
        outra => panic!("{outra:?}"),
    }
    assert!(c.amb.kb.existe(&arquivo));
    assert!(c.amb.kb.existe(&destino));

    // A revisão do manual começa com os passos e os prints do aprovado.
    c.aceitar();
    let r = c.ok(
        "iniciar_analise",
        json!({ "fluxo": "manual", "funcionalidade": "F-CAN-001" }),
    );
    let nova = Id::analisar(r["analise"].as_str().unwrap()).unwrap();
    let a = c.amb.analises.carregar(&nova).unwrap();
    assert_eq!(a.passos.len(), 2);
    assert_eq!(a.passos[0].print.as_ref().unwrap().arquivo, arquivo);
    assert!(a.passos[1].print_dispensado);
    // Trocar o print na revisão não apaga o arquivo que o aprovado usa.
    manual::anexar_print(
        &c.amb,
        &nova,
        1,
        NovoPrint {
            png: [PNG, b"extra"].concat(),
            ..print("Tela nova")
        },
    )
    .unwrap();
    assert!(c.amb.kb.existe(&arquivo), "o aprovado ainda usa o print");
    // Remover o passo novo apaga o arquivo que só ele usava.
    let a = c.amb.analises.carregar(&nova).unwrap();
    let novo_arquivo = a.passos[0].print.as_ref().unwrap().arquivo.clone();
    assert_ne!(novo_arquivo, arquivo);
    manual::remover_passo(&c.amb, &nova, 1).unwrap();
    assert!(!c.amb.kb.existe(&novo_arquivo));
    assert!(c.amb.kb.existe(&arquivo));
}
