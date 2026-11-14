//! Ferramentas do fluxo "Analisar reunião" (#139): `ler_transcript`,
//! `propor_papel`, `registrar_item_ata`, `sugerir_demanda` e `iniciar_analise`
//! com demanda e reunião, com a reunião fictícia de `fixtures/reunioes/`.

use std::fs;
use std::path::{Path, PathBuf};

use cba_core::ambiente::Ambiente;
use cba_core::analise::Fluxo;
use cba_core::config::Config;
use cba_core::dados::DiretorioDados;
use cba_docs::{EstadoAta, NovoItem, TipoItem};
use cba_kb::frontmatter::{Data, FrontmatterAnalise};
use cba_kb::{Documento, Id, IdDemanda, IdReuniao, NovaDemanda, NovaPessoa};
use cba_reunioes::{Fala, Formato, Guarda, Horario, MotivoDescarte, Transcript, analisar};
use cba_tools::reuniao::LIMITE_DE_TEXTO_DO_TRANSCRIPT;
use cba_tools::{ErroChamada, ErroFerramenta, Saida, registro_padrao};
use serde_json::{Value, json};

const PROJETO: &str = "faturamento-exemplo";

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/faturamento-exemplo")
}

fn fixture_da_reuniao(nome: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/reunioes")
        .join(nome)
}

fn data(texto: &str) -> Data {
    Data::analisar(texto).unwrap()
}

/// Transcript da reunião 1 (`.vtt`): cancelamento de fatura, 29 falas.
fn reuniao_1() -> Transcript {
    let nome = "reuniao-01-cancelamento.vtt";
    analisar(&fs::read(fixture_da_reuniao(nome)).unwrap(), Some(nome))
}

struct Cenario {
    _dir: tempfile::TempDir,
    amb: Ambiente,
    /// Demanda D-0001, aberta, com a reunião R-0001 (transcript guardado).
    demanda: IdDemanda,
    reuniao: IdReuniao,
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
    let demanda = amb
        .kb
        .criar_demanda(
            &NovaDemanda {
                titulo: "Trava de cancelamento de fatura".into(),
                descricao: Some(
                    "A auditoria pediu que o cancelamento de faturas antigas exija \
                     supervisor em todos os canais."
                        .into(),
                ),
                chave: Some("FAT-12".into()),
                origem: None,
            },
            data("2026-11-10"),
        )
        .unwrap()
        .id;
    let (estado, _) = EstadoAta::criar(&amb.kb, demanda, data("2026-11-12")).unwrap();
    Guarda::nova(&amb.dados)
        .guardar(PROJETO, estado.id, &reuniao_1(), 1)
        .unwrap();
    Cenario {
        _dir: dir,
        amb,
        demanda,
        reuniao: estado.id,
    }
}

impl Cenario {
    fn saida(&self, nome: &str, args: Value) -> Result<Saida, ErroFerramenta> {
        match registro_padrao().chamar(nome, Ok(&self.amb), Some(args)) {
            Ok(saida) => Ok(saida),
            Err(ErroChamada::Ferramenta(e)) => Err(e),
            Err(e) => panic!("{e}"),
        }
    }

    fn chamar(&self, nome: &str, args: Value) -> Value {
        self.saida(nome, args)
            .unwrap_or_else(|e| panic!("{nome}: {e}"))
            .dados
            .unwrap()
    }

    fn erro(&self, nome: &str, args: Value) -> ErroFerramenta {
        self.saida(nome, args).unwrap_err()
    }

    fn ata(&self) -> EstadoAta {
        EstadoAta::ler(&self.amb.kb, self.reuniao).unwrap().0
    }

    fn texto_da_ata(&self) -> String {
        let arquivos = self.amb.kb.reuniao(self.reuniao).unwrap();
        self.amb.kb.ler_texto(&arquivos.ata).unwrap()
    }

    fn documento(&self, id: &str) -> Documento<FrontmatterAnalise> {
        let a = self
            .amb
            .analises
            .carregar(&Id::analisar(id).unwrap())
            .unwrap();
        self.amb.kb.ler(&a.documento).unwrap()
    }

    fn eventos(&self) -> Vec<Value> {
        let arquivo = self.amb.dados.eventos().join(format!("{PROJETO}.jsonl"));
        fs::read_to_string(arquivo)
            .unwrap_or_default()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }
}

/// Citação da fala 8 (Maria Silva: a necessidade da checagem do supervisor).
fn citacao_da_necessidade() -> Value {
    json!({ "indice": 8, "trecho": "a checagem do supervisor precisa valer também fora da tela Delphi" })
}

// ---------------------------------------------------------------------------
// ler_transcript

#[test]
fn ler_transcript_pagina_e_filtra_por_falante_e_horario() {
    let c = cenario();
    let d = c.chamar(
        "ler_transcript",
        json!({ "reuniao": "R-0001", "limite": 5 }),
    );
    assert_eq!(d["falas_no_transcript"], 29);
    assert_eq!(d["total"], 29);
    assert_eq!(d["quantidade"], 5);
    assert_eq!(d["proximo_cursor"], 5);
    assert_eq!(d["falas"][0]["indice"], 1);
    assert_eq!(d["falas"][0]["falante"], "Silva, Maria");
    assert_eq!(d["falas"][0]["inicio"], "00:00:03");
    assert_eq!(d["falantes"][0]["nome"], "Silva, Maria");
    assert_eq!(d["falantes"].as_array().unwrap().len(), 4);

    let s = c
        .saida(
            "ler_transcript",
            json!({ "reuniao": "R-0001", "limite": 5, "cursor": 5 }),
        )
        .unwrap();
    assert!(s.texto.contains("[6] "), "{}", s.texto);
    assert!(s.texto.contains("cursor: 10"), "{}", s.texto);

    // Por falante: parte do nome, sem acento nem caixa.
    let d = c.chamar(
        "ler_transcript",
        json!({ "reuniao": "R-0001", "falante": "joao", "limite": 200 }),
    );
    assert_eq!(d["total"], 6);
    for f in d["falas"].as_array().unwrap() {
        assert_eq!(f["falante"], "João Pereira");
    }

    // Por horário.
    let d = c.chamar(
        "ler_transcript",
        json!({ "reuniao": "R-0001", "desde": "00:01:00", "ate": "00:02:00", "limite": 200 }),
    );
    let falas = d["falas"].as_array().unwrap();
    assert!(!falas.is_empty());
    for f in falas {
        let inicio = f["inicio"].as_str().unwrap();
        assert!(("00:01:00"..="00:02:00").contains(&inicio), "{inicio}");
    }

    // Horário inválido é erro de argumento.
    let e = c.erro(
        "ler_transcript",
        json!({ "reuniao": "R-0001", "desde": "uma hora" }),
    );
    assert!(matches!(e, ErroFerramenta::Argumentos(_)), "{e}");
}

#[test]
fn ler_transcript_recusa_reuniao_inexistente_e_transcript_descartado() {
    let c = cenario();
    let e = c.erro("ler_transcript", json!({ "reuniao": "R-0009" }));
    assert!(matches!(e, ErroFerramenta::Recusa(_)), "{e}");
    assert!(e.to_string().contains("R-0009 não existe"), "{e}");

    Guarda::nova(&c.amb.dados)
        .apagar(PROJETO, c.reuniao, MotivoDescarte::AtaAprovada)
        .unwrap();
    let e = c.erro("ler_transcript", json!({ "reuniao": "R-0001" }));
    assert!(matches!(e, ErroFerramenta::Recusa(_)), "{e}");
    assert!(e.to_string().contains("não está mais guardado"), "{e}");
    // A mesma recusa ao registrar um item.
    let e = c.erro(
        "registrar_item_ata",
        json!({
            "reuniao": "R-0001", "tipo": "necessidade", "texto": "x",
            "citacoes": [citacao_da_necessidade()]
        }),
    );
    assert!(e.to_string().contains("não está mais guardado"), "{e}");
}

/// Reunião sintética de 2 h com ~25 mil palavras: cada página cabe no limite de
/// resposta, e as páginas cobrem todas as falas, cada uma uma vez.
#[test]
fn ler_transcript_pagina_uma_reuniao_de_duas_horas_sem_estourar_a_resposta() {
    let c = cenario();
    let palavras = [
        "fatura",
        "cancelamento",
        "supervisor",
        "auditoria",
        "parcela",
        "boleto",
        "cliente",
        "relatório",
        "prazo",
        "pagamento",
        "contrato",
        "perfil",
        "processo",
        "banco",
        "tela",
    ];
    let falantes = ["Silva, Maria", "João Pereira", "Ana Souza", "Carlos Lima"];
    let n = 720;
    let falas: Vec<Fala> = (0..n)
        .map(|i| {
            let texto: Vec<&str> = (0..35)
                .map(|j| palavras[(i * 7 + j * 3) % palavras.len()])
                .collect();
            Fala {
                indice: i + 1,
                falante: Some(falantes[i % falantes.len()].into()),
                inicio: Some(Horario::de_milissegundos(i as u64 * 10_000)),
                fim: None,
                texto: texto.join(" "),
            }
        })
        .collect();
    let total_de_palavras: usize = falas.iter().map(|f| f.texto.split(' ').count()).sum();
    assert!(total_de_palavras >= 25_000, "{total_de_palavras}");
    assert!(falas.last().unwrap().inicio.unwrap().segundos() >= 7_000);
    let (longa, _) = EstadoAta::criar(&c.amb.kb, c.demanda, data("2026-11-13")).unwrap();
    Guarda::nova(&c.amb.dados)
        .guardar(
            PROJETO,
            longa.id,
            &Transcript {
                formato: Formato::Vtt,
                falas,
                avisos: vec![],
            },
            1,
        )
        .unwrap();

    let mut vistas = Vec::new();
    let mut cursor = Some(0);
    let mut paginas = 0;
    while let Some(atual) = cursor {
        let s = c
            .saida(
                "ler_transcript",
                json!({ "reuniao": longa.id.to_string(), "cursor": atual, "limite": 200 }),
            )
            .unwrap();
        let dados = s.dados.unwrap();
        // O que o MCP envia: o texto e os dados estruturados.
        let tamanho = s.texto.chars().count() + serde_json::to_string(&dados).unwrap().len();
        assert!(
            tamanho <= 4 * LIMITE_DE_TEXTO_DO_TRANSCRIPT,
            "página {paginas}: {tamanho} caracteres"
        );
        let falas = dados["falas"].as_array().unwrap();
        assert!(!falas.is_empty());
        vistas.extend(falas.iter().map(|f| f["indice"].as_u64().unwrap()));
        cursor = dados["proximo_cursor"].as_u64().map(|c| c as usize);
        paginas += 1;
        assert!(paginas < 200, "paginação não termina");
    }
    assert_eq!(vistas, (1..=n as u64).collect::<Vec<_>>());
    assert!(paginas > 10, "{paginas}");
}

// ---------------------------------------------------------------------------
// registrar_item_ata

#[test]
fn registrar_item_ata_com_citacao_conferida_devolve_o_id_e_monta_a_ata() {
    let c = cenario();
    let d = c.chamar(
        "registrar_item_ata",
        json!({
            "reuniao": "R-0001",
            "tipo": "necessidade",
            "texto": "A trava do supervisor para faturas antigas vale em todos os canais.",
            "citacoes": [citacao_da_necessidade()]
        }),
    );
    assert_eq!(d["item"], "N-1");
    assert_eq!(d["citacoes"][0]["indice"], 8);
    assert_eq!(d["citacoes"][0]["falante"], "Silva, Maria");

    // Citação pelo falante e horário, sem índice.
    let d = c.chamar(
        "registrar_item_ata",
        json!({
            "reuniao": "R-0001",
            "tipo": "regra",
            "texto": "Fatura com pagamento parcial não pode ser cancelada.",
            "citacoes": [{
                "falante": "João Pereira",
                "inicio": "00:01:11",
                "trecho": "fatura com pagamento parcial não pode ser cancelada"
            }]
        }),
    );
    assert_eq!(d["item"], "R-1");

    // Pendência com responsável e prazo, e próximo passo apontando para N-1.
    let d = c.chamar(
        "registrar_item_ata",
        json!({
            "reuniao": "R-0001",
            "tipo": "pendencia",
            "texto": "Levantar os canais de cancelamento",
            "responsavel": "João Pereira",
            "prazo": "até sexta",
            "citacoes": [{ "indice": 4, "trecho": "passa direto, sem checar o perfil" }]
        }),
    );
    assert_eq!(d["item"], "P-1");
    let d = c.chamar(
        "registrar_item_ata",
        json!({
            "reuniao": "R-0001",
            "tipo": "proximo_passo",
            "texto": "Analisar o impacto da necessidade N-1",
            "referencias": ["N-1"],
            "citacoes": [citacao_da_necessidade()]
        }),
    );
    assert_eq!(d["item"], "PP-1");
    assert_eq!(d["referencias"], json!(["N-1"]));

    let ata = c.ata();
    assert_eq!(ata.itens.len(), 4);
    let p = ata
        .itens
        .iter()
        .find(|i| i.tipo == TipoItem::Pendencia)
        .unwrap();
    assert_eq!(p.responsavel.as_deref(), Some("João Pereira"));
    assert_eq!(p.prazo.as_deref(), Some("até sexta"));
    let texto = c.texto_da_ata();
    assert!(texto.contains("**N-1** A trava do supervisor"), "{texto}");
    assert!(texto.contains("(sobre N-1)"), "{texto}");
    assert!(texto.contains("Silva, Maria, 00:01:43"), "{texto}");

    // A ata ao vivo é avisada pelo diário, só com identificadores.
    let eventos = c.eventos();
    assert!(
        eventos
            .iter()
            .any(|e| e["mudanca"] == "ata" && e["analise"] == "R-0001"),
        "{eventos:?}"
    );
}

#[test]
fn registrar_item_ata_recusa_citacao_que_nao_confere_dizendo_o_motivo() {
    let c = cenario();
    let fala_8 = "Então a necessidade é clara: a checagem do supervisor precisa valer também fora da tela Delphi.";

    let e = c.erro(
        "registrar_item_ata",
        json!({
            "reuniao": "R-0001",
            "tipo": "decisao",
            "texto": "Decidido liberar o cancelamento.",
            "citacoes": [
                citacao_da_necessidade(),
                { "indice": 8, "trecho": "o supervisor pode ser dispensado" }
            ]
        }),
    );
    assert!(matches!(e, ErroFerramenta::Recusa(_)), "{e}");
    let msg = e.to_string();
    assert!(msg.contains("citação 2"), "{msg}");
    assert!(msg.contains("não aparece na fala 8"), "{msg}");
    assert!(msg.contains("nada foi registrado"), "{msg}");
    // A recusa não ecoa o texto da fala.
    assert!(!msg.contains("tela Delphi"), "{msg}");
    assert!(!msg.contains(fala_8), "{msg}");
    assert!(c.ata().itens.is_empty());

    // Fala inexistente, falante errado e trecho longo também dizem o motivo.
    for (citacao, motivo) in [
        (
            json!({ "indice": 99, "trecho": "x y" }),
            "a fala 99 não existe",
        ),
        (
            json!({ "indice": 8, "falante": "João Pereira", "trecho": "checagem" }),
            "é de Silva, Maria",
        ),
        (
            json!({ "indice": 8, "trecho": "palavra ".repeat(40) }),
            "cite um trecho curto",
        ),
        (json!({ "trecho": "checagem" }), "informe o índice da fala"),
    ] {
        let e = c.erro(
            "registrar_item_ata",
            json!({ "reuniao": "R-0001", "tipo": "duvida", "texto": "x", "citacoes": [citacao] }),
        );
        assert!(e.to_string().contains(motivo), "{e}");
    }

    // Referência a item que não existe.
    let e = c.erro(
        "registrar_item_ata",
        json!({
            "reuniao": "R-0001", "tipo": "proximo_passo", "texto": "Analisar N-3",
            "referencias": ["N-3"], "citacoes": [citacao_da_necessidade()]
        }),
    );
    assert!(e.to_string().contains("N-3 não existe"), "{e}");
    // Responsável fora da pendência.
    let e = c.erro(
        "registrar_item_ata",
        json!({
            "reuniao": "R-0001", "tipo": "decisao", "texto": "x", "responsavel": "Ana",
            "citacoes": [citacao_da_necessidade()]
        }),
    );
    assert!(matches!(e, ErroFerramenta::Argumentos(_)), "{e}");
    assert!(c.ata().itens.is_empty());
}

#[test]
fn registrar_item_ata_aceita_evidencia_de_codigo_conferida() {
    let c = cenario();
    let evidencia = json!({
        "arquivo": "db/03_pkg_cobranca.sql",
        "linha_inicial": 71,
        "linha_final": 72,
        "trecho": "FN_PARAMETRO('PERC_MULTA_ATRASO') / 100, 2)"
    });
    let d = c.chamar(
        "registrar_item_ata",
        json!({
            "reuniao": "R-0001", "tipo": "regra", "texto": "O sistema já calcula a multa.",
            "citacoes": [citacao_da_necessidade()], "evidencias": [evidencia]
        }),
    );
    assert_eq!(d["item"], "R-1");
    assert_eq!(d["evidencias"][0]["arquivo"], "db/03_pkg_cobranca.sql");
    assert_eq!(c.ata().itens[0].evidencias.len(), 1);

    let mut errada = evidencia.clone();
    errada["trecho"] = json!("NAO_EXISTE_NO_CODIGO");
    let e = c.erro(
        "registrar_item_ata",
        json!({
            "reuniao": "R-0001", "tipo": "regra", "texto": "x",
            "citacoes": [citacao_da_necessidade()], "evidencias": [errada]
        }),
    );
    assert!(matches!(e, ErroFerramenta::Recusa(_)), "{e}");
    assert_eq!(c.ata().itens.len(), 1);
}

#[test]
fn ata_aprovada_nao_aceita_registros() {
    let c = cenario();
    let mut ata = c.ata();
    ata.status = cba_kb::frontmatter::StatusAta::Aprovada;
    ata.gravar(&c.amb.kb).unwrap();
    let e = c.erro(
        "registrar_item_ata",
        json!({
            "reuniao": "R-0001", "tipo": "decisao", "texto": "x",
            "citacoes": [citacao_da_necessidade()]
        }),
    );
    assert!(e.to_string().contains("já foi aprovada"), "{e}");
}

// ---------------------------------------------------------------------------
// propor_papel

#[test]
fn propor_papel_grava_a_proposta_na_ata_com_a_sugestao_do_catalogo_sem_mudar_o_catalogo() {
    let c = cenario();
    let maria = c
        .amb
        .kb
        .registrar_pessoa(&NovaPessoa {
            nome: "Maria Silva".into(),
            apelidos: vec!["Silva, Maria".into()],
            papel: None,
        })
        .unwrap();
    c.amb
        .kb
        .registrar_papel(&maria.slug, "D-0001/R-0000", "PO", data("2026-11-01"))
        .unwrap();
    let antes = fs::read_to_string(c.amb.kb.caminho(&maria.arquivo)).unwrap();

    let d = c.chamar(
        "propor_papel",
        json!({
            "reuniao": "R-0001",
            "participante": "Silva, Maria",
            "papel": "gerente",
            "justificativa": "Conduz a reunião e decide as prioridades.",
            "citacoes": [{ "indice": 1, "trecho": "O assunto de hoje é o cancelamento de fatura" }]
        }),
    );
    assert_eq!(d["participante"], "Silva, Maria");
    assert_eq!(d["papel_proposto"], "gerente");
    assert_eq!(d["catalogo"]["casamento"], "unica");
    assert_eq!(d["catalogo"]["pessoa"], maria.slug.as_str());
    assert_eq!(d["catalogo"]["papel_sugerido"], "PO");

    let ata = c.ata();
    let p = &ata.participantes[0];
    assert_eq!(p.nome, "Silva, Maria");
    assert_eq!(p.papel, None, "quem confirma é o analista");
    assert_eq!(p.pessoa.as_deref(), Some(maria.slug.as_str()));
    let proposta = p.proposta.as_ref().unwrap();
    assert_eq!(proposta.papel, "gerente");
    assert_eq!(proposta.sugerido_pelo_catalogo.as_deref(), Some("PO"));
    assert_eq!(proposta.citacoes.len(), 1);
    assert!(c.texto_da_ata().contains("(proposto pelo agente: gerente)"));
    // O catálogo de pessoas não mudou.
    assert_eq!(
        fs::read_to_string(c.amb.kb.caminho(&maria.arquivo)).unwrap(),
        antes
    );

    // Papel do catálogo com outra grafia vira a grafia do catálogo; quem não está
    // no catálogo recebe a proposta sem sugestão. Propor de novo troca a proposta.
    let d = c.chamar(
        "propor_papel",
        json!({
            "reuniao": "R-0001",
            "participante": "joão pereira",
            "papel": "Desenvolvedor",
            "citacoes": [{ "indice": 3, "trecho": "a trava dos 30 dias só existe na tela de retaguarda" }]
        }),
    );
    assert_eq!(d["participante"], "João Pereira");
    assert_eq!(d["papel_proposto"], "desenvolvedor");
    assert_eq!(d["catalogo"]["casamento"], "nenhuma");
    c.chamar(
        "propor_papel",
        json!({
            "reuniao": "R-0001",
            "participante": "João Pereira",
            "papel": "arquiteto",
            "citacoes": [{ "indice": 3, "trecho": "tela Delphi" }]
        }),
    );
    let ata = c.ata();
    assert_eq!(ata.participantes.len(), 2);
    assert_eq!(
        ata.participantes[1].proposta.as_ref().unwrap().papel,
        "arquiteto"
    );
}

#[test]
fn propor_papel_recusa_quem_nao_fala_citacao_errada_e_papel_que_parece_fala() {
    let c = cenario();
    let e = c.erro(
        "propor_papel",
        json!({
            "reuniao": "R-0001", "participante": "Pedro Ninguém", "papel": "PO",
            "citacoes": [{ "indice": 1, "trecho": "Bom dia" }]
        }),
    );
    assert!(e.to_string().contains("não é falante"), "{e}");
    assert!(e.to_string().contains("Silva, Maria"), "{e}");

    let e = c.erro(
        "propor_papel",
        json!({
            "reuniao": "R-0001", "participante": "Ana Souza", "papel": "QA",
            "citacoes": [{ "indice": 5, "trecho": "trecho que ninguém disse" }]
        }),
    );
    assert!(e.to_string().contains("não aparece na fala 5"), "{e}");

    let e = c.erro(
        "propor_papel",
        json!({
            "reuniao": "R-0001", "participante": "Ana Souza", "papel": "quem pergunta: tudo?",
            "citacoes": [{ "indice": 5, "trecho": "registrar direito" }]
        }),
    );
    assert!(matches!(e, ErroFerramenta::Argumentos(_)), "{e}");
    assert!(c.ata().participantes.is_empty());
}

// ---------------------------------------------------------------------------
// sugerir_demanda

#[test]
fn sugerir_demanda_traz_abertas_parecidas_com_o_motivo() {
    let c = cenario();
    let kb = &c.amb.kb;
    kb.criar_demanda(
        &NovaDemanda {
            titulo: "Relatório de inadimplência por região".into(),
            ..Default::default()
        },
        data("2026-11-10"),
    )
    .unwrap();
    let encerrada = kb
        .criar_demanda(
            &NovaDemanda {
                titulo: "Cancelamento de fatura em lote".into(),
                ..Default::default()
            },
            data("2026-11-10"),
        )
        .unwrap();
    kb.encerrar_demanda(encerrada.id, data("2026-11-11"))
        .unwrap();

    let d = c.chamar(
        "sugerir_demanda",
        json!({ "texto": "Cancelar fatura antiga sem supervisor" }),
    );
    assert_eq!(d["abertas"], 2);
    let sugestoes = d["sugestoes"].as_array().unwrap();
    assert_eq!(sugestoes[0]["id"], "D-0001", "{d}");
    assert_eq!(sugestoes[0]["titulo"], "Trava de cancelamento de fatura");
    assert_eq!(sugestoes[0]["chave"], "FAT-12");
    assert!(
        sugestoes[0]["motivo"]
            .as_str()
            .unwrap()
            .contains("título parecido"),
        "{d}"
    );
    // Encerrada e sem relação ficam de fora.
    for s in sugestoes {
        assert_ne!(s["id"], encerrada.id.to_string());
        assert_ne!(s["id"], "D-0002");
    }

    // A descrição também conta, pela busca na base.
    let d = c.chamar(
        "sugerir_demanda",
        json!({ "texto": "pedido da auditoria interna" }),
    );
    let sugestoes = d["sugestoes"].as_array().unwrap();
    assert_eq!(sugestoes[0]["id"], "D-0001", "{d}");
    assert!(
        sugestoes[0]["motivo"]
            .as_str()
            .unwrap()
            .contains("busca na base"),
        "{d}"
    );

    let d = c.chamar("sugerir_demanda", json!({ "texto": "integração contábil" }));
    assert!(d["sugestoes"].as_array().unwrap().is_empty(), "{d}");
}

// ---------------------------------------------------------------------------
// iniciar_analise com demanda e reunião

#[test]
fn iniciar_analise_com_demanda_grava_demanda_id_na_analise() {
    let c = cenario();
    let d = c.chamar(
        "iniciar_analise",
        json!({
            "fluxo": "perguntar",
            "entrada": "Quem pode cancelar uma fatura antiga?",
            "demanda": "D-0001"
        }),
    );
    let id = d["analise"].as_str().unwrap();
    let doc = c.documento(id);
    assert_eq!(doc.frontmatter.demanda.id, Some(c.demanda));
    assert_eq!(doc.frontmatter.demanda.chave.as_deref(), Some("FAT-12"));
    assert_eq!(d["contexto"]["demanda"]["id"], "D-0001");

    // Análise que já existe, sem demanda: a ligação é gravada ao iniciar.
    let a = c
        .amb
        .analises
        .criar(cba_core::analises::NovaAnalise {
            fluxo: Fluxo::Impacto,
            titulo: "Trava em todos os canais".into(),
            entrada: "Exigir supervisor também no banco.".into(),
            analista: None,
            base: None,
        })
        .unwrap();
    assert_eq!(c.documento(&a.id.to_string()).frontmatter.demanda.id, None);
    let s = c
        .saida(
            "iniciar_analise",
            json!({ "analise": a.id.to_string(), "demanda": "D-0001" }),
        )
        .unwrap();
    assert!(s.texto.contains("Demanda D-0001 [FAT-12]"), "{}", s.texto);
    let doc = c.documento(&a.id.to_string());
    assert_eq!(doc.frontmatter.demanda.id, Some(c.demanda));
    // O documento foi remontado com o frontmatter novo.
    assert!(doc.corpo.contains("Trava em todos os canais"));

    // Outra demanda para a mesma análise é recusada; demanda inexistente também.
    let outra = c
        .amb
        .kb
        .criar_demanda(
            &NovaDemanda {
                titulo: "Outra".into(),
                ..Default::default()
            },
            data("2026-11-10"),
        )
        .unwrap();
    let e = c.erro(
        "iniciar_analise",
        json!({ "analise": a.id.to_string(), "demanda": outra.id.to_string() }),
    );
    assert!(
        e.to_string().contains("já está ligada à demanda D-0001"),
        "{e}"
    );
    let e = c.erro(
        "iniciar_analise",
        json!({ "fluxo": "perguntar", "entrada": "x", "demanda": "D-0042" }),
    );
    assert!(e.to_string().contains("D-0042 não existe"), "{e}");
    c.amb
        .kb
        .encerrar_demanda(outra.id, data("2026-11-11"))
        .unwrap();
    let e = c.erro(
        "iniciar_analise",
        json!({ "fluxo": "perguntar", "entrada": "x", "demanda": outra.id.to_string() }),
    );
    assert!(e.to_string().contains("encerrada"), "{e}");
}

#[test]
fn iniciar_analise_no_fluxo_reuniao_traz_atas_anteriores_pendencias_e_pessoas() {
    let c = cenario();
    // Uma reunião anterior da mesma demanda, aprovada, com decisão e pendência.
    let (mut anterior, _) = EstadoAta::criar(&c.amb.kb, c.demanda, data("2026-11-05")).unwrap();
    anterior.definir_resumo("Primeira conversa sobre a trava do supervisor.");
    anterior
        .adicionar_item(NovoItem::novo(
            TipoItem::Decisao,
            "Manter a trava de 30 dias.",
        ))
        .unwrap();
    let mut pendencia = NovoItem::novo(TipoItem::Pendencia, "Listar os canais de cancelamento");
    pendencia.responsavel = Some("João Pereira".into());
    anterior.adicionar_item(pendencia).unwrap();
    anterior.status = cba_kb::frontmatter::StatusAta::Aprovada;
    cba_docs::Templates::padrao()
        .gravar_ata(&c.amb.kb, &anterior)
        .unwrap();
    c.amb
        .kb
        .registrar_pessoa(&NovaPessoa {
            nome: "Ana Souza".into(),
            apelidos: vec![],
            papel: Some("analista de negócio".into()),
        })
        .unwrap();

    // Sem `reuniao`, o fluxo reuniao é recusado.
    let e = c.erro("iniciar_analise", json!({ "fluxo": "reuniao" }));
    assert!(matches!(e, ErroFerramenta::Argumentos(_)), "{e}");
    // Demanda diferente da reunião, também.
    let e = c.erro(
        "iniciar_analise",
        json!({ "fluxo": "reuniao", "reuniao": "R-0001", "demanda": "D-0002" }),
    );
    assert!(e.to_string().contains("é da demanda D-0001"), "{e}");

    let s = c
        .saida(
            "iniciar_analise",
            json!({ "fluxo": "reuniao", "reuniao": "R-0001" }),
        )
        .unwrap();
    let d = s.dados.unwrap();
    assert_eq!(d["fluxo"], "reuniao");
    let id = d["analise"].as_str().unwrap().to_owned();
    let analise = c
        .amb
        .analises
        .carregar(&Id::analisar(&id).unwrap())
        .unwrap();
    assert_eq!(analise.reuniao, Some(c.reuniao));
    assert_eq!(c.documento(&id).frontmatter.demanda.id, Some(c.demanda));
    assert_eq!(
        c.documento(&id).frontmatter.tipo,
        cba_kb::TipoDocumento::AnaliseReuniao
    );

    let ctx = &d["contexto"]["demanda"];
    assert_eq!(ctx["atas_anteriores"][0]["id"], anterior.id.to_string());
    assert_eq!(
        ctx["atas_anteriores"][0]["decisoes"][0]["texto"],
        "Manter a trava de 30 dias."
    );
    assert_eq!(
        ctx["pendencias"][0]["texto"],
        "Listar os canais de cancelamento"
    );
    assert_eq!(ctx["pessoas"][0]["nome"], "Ana Souza");
    assert_eq!(ctx["pessoas"][0]["papel"], "analista de negócio");
    let participantes = ctx["participantes"].as_array().unwrap();
    assert_eq!(participantes.len(), 4);
    let ana = participantes
        .iter()
        .find(|p| p["falante"] == "Ana Souza")
        .unwrap();
    assert_eq!(ana["papel_sugerido"], "analista de negócio");
    // A reunião analisada não entra entre as anteriores.
    assert_eq!(ctx["total_de_atas_anteriores"], 1);

    // O roteiro (provisório) e o contexto chegam no texto.
    assert!(s.texto.contains("ler_transcript"), "{}", s.texto);
    assert!(
        s.texto.contains("Manter a trava de 30 dias."),
        "{}",
        s.texto
    );
    assert!(
        s.texto.contains("Listar os canais de cancelamento"),
        "{}",
        s.texto
    );

    // Concluir exige itens na ata.
    let e = c.erro("concluir_analise", json!({ "analise": id }));
    assert!(e.to_string().contains("ainda não tem itens"), "{e}");
    c.chamar(
        "registrar_item_ata",
        json!({
            "reuniao": "R-0001", "tipo": "necessidade", "texto": "Trava em todos os canais.",
            "citacoes": [citacao_da_necessidade()]
        }),
    );
    let d = c.chamar("concluir_analise", json!({ "analise": id }));
    assert_eq!(d["estado"], "concluida");

    // Retomar a análise de novo, sem `reuniao`, mantém a ligação.
    c.chamar("iniciar_analise", json!({ "analise": id }));
    // A análise de reunião não aceita outra reunião.
    let e = c.erro(
        "iniciar_analise",
        json!({ "analise": id, "reuniao": anterior.id.to_string() }),
    );
    assert!(e.to_string().contains("é da reunião R-0001"), "{e}");
}
