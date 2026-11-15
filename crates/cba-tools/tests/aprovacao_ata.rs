//! Aprovação da ata de uma reunião (#142): o segundo aviso, o descarte do
//! transcript, a ordem das etapas e a repetição idempotente, com a reunião fictícia
//! de `fixtures/reunioes/`.

use std::fs;
use std::path::{Path, PathBuf};

use cba_core::ambiente::Ambiente;
use cba_core::analise::EstadoAnalise;
use cba_core::config::Config;
use cba_core::dados::DiretorioDados;
use cba_docs::{EstadoAta, StatusCitacao, Templates};
use cba_kb::frontmatter::{Data, StatusAta};
use cba_kb::{Id, IdReuniao, NovaDemanda};
use cba_reunioes::{Guarda, MotivoDescarte, analisar};
use cba_tools::aprovacao::OpcoesAprovacao;
use cba_tools::aprovacao_ata::{AVISO_DESCARTE, ErroAprovacaoAta, aprovar_ata, prontidao_ata};
use cba_tools::revisao::{ErroRevisao, aprovar_revisada};
use cba_tools::{ErroChamada, registro_padrao};
use serde_json::{Value, json};

const PROJETO: &str = "faturamento-exemplo";

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/faturamento-exemplo")
}

fn hoje() -> Data {
    Data::analisar("2026-11-14").unwrap()
}

struct Cenario {
    _dir: tempfile::TempDir,
    amb: Ambiente,
    reuniao: IdReuniao,
    /// A análise do fluxo `reuniao`.
    analise: Id,
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
                ..Default::default()
            },
            Data::analisar("2026-11-10").unwrap(),
        )
        .unwrap()
        .id;
    let (estado, _) =
        EstadoAta::criar(&amb.kb, demanda, Data::analisar("2026-11-12").unwrap()).unwrap();
    let nome = "reuniao-01-cancelamento.vtt";
    let caminho = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/reunioes")
        .join(nome);
    let transcript = analisar(&fs::read(caminho).unwrap(), Some(nome));
    Guarda::nova(&amb.dados)
        .guardar(PROJETO, estado.id, &transcript, 1)
        .unwrap();
    let mut c = Cenario {
        _dir: dir,
        amb,
        reuniao: estado.id,
        analise: Id::analisar("A-0001").unwrap(),
    };
    let d = c.chamar(
        "iniciar_analise",
        json!({ "fluxo": "reuniao", "reuniao": estado.id.to_string() }),
    );
    c.analise = Id::analisar(d["analise"].as_str().unwrap()).unwrap();
    c
}

impl Cenario {
    fn tentar(&self, nome: &str, args: Value) -> Result<Value, String> {
        match registro_padrao().chamar(nome, Ok(&self.amb), Some(args)) {
            Ok(s) => Ok(s.dados.unwrap()),
            Err(ErroChamada::Ferramenta(e)) => Err(e.to_string()),
            Err(e) => panic!("{e}"),
        }
    }

    fn chamar(&self, nome: &str, args: Value) -> Value {
        self.tentar(nome, args)
            .unwrap_or_else(|e| panic!("{nome}: {e}"))
    }

    /// Necessidade, próximo passo que aponta para ela e a conclusão do agente.
    fn registrar_e_concluir(&self) {
        self.chamar(
            "registrar_item_ata",
            json!({
                "reuniao": "R-0001", "tipo": "necessidade", "texto": "Trava em todos os canais.",
                "citacoes": [{ "indice": 8, "trecho": "a checagem do supervisor precisa valer também fora da tela Delphi" }]
            }),
        );
        self.chamar(
            "registrar_item_ata",
            json!({
                "reuniao": "R-0001", "tipo": "proximo_passo",
                "texto": "Analisar o impacto da trava nos outros canais",
                "referencias": ["N-1"],
                "citacoes": [{ "indice": 4, "trecho": "passa direto, sem checar o perfil" }]
            }),
        );
        self.chamar("concluir_analise", json!({ "analise": self.analise }));
    }

    fn ata(&self) -> EstadoAta {
        EstadoAta::ler(&self.amb.kb, self.reuniao).unwrap().0
    }

    fn guardado(&self) -> bool {
        Guarda::nova(&self.amb.dados).existe(PROJETO, self.reuniao)
    }

    fn estado_da_analise(&self) -> EstadoAnalise {
        self.amb.analises.carregar(&self.analise).unwrap().estado
    }
}

/// Critério de aceite da #142: sem confirmar o aviso, a ata não é aprovada e o
/// transcript continua.
#[test]
fn sem_confirmar_o_aviso_a_ata_nao_e_aprovada_e_o_transcript_continua() {
    let c = cenario();
    c.registrar_e_concluir();
    let antes = c.ata();

    let e = aprovar_ata(&c.amb, &c.analise, false, hoje()).unwrap_err();
    assert!(matches!(e, ErroAprovacaoAta::SemConfirmacao), "{e}");
    assert!(e.to_string().contains("transcript será apagado"), "{e}");

    assert_eq!(c.ata(), antes);
    assert_eq!(c.ata().status, StatusAta::Rascunho);
    assert!(c.guardado());
    assert_eq!(c.estado_da_analise(), EstadoAnalise::Concluida);
    // O agente continua lendo o transcript.
    c.chamar(
        "ler_transcript",
        json!({ "reuniao": "R-0001", "limite": 1 }),
    );
}

#[test]
fn aprovar_apaga_o_transcript_marca_as_citacoes_e_aprova_a_analise() {
    let c = cenario();
    c.registrar_e_concluir();

    let p = prontidao_ata(&c.amb, &c.analise).unwrap();
    assert!(p.pode_aprovar, "{p:?}");
    assert!(!p.concluida);
    assert!(p.transcript_guardado);
    assert_eq!(p.citacoes_conferiveis, 2);
    assert_eq!(p.itens, 2);
    assert_eq!(p.aviso_descarte, AVISO_DESCARTE);
    assert!(p.avisos.is_empty(), "{:?}", p.avisos);

    let feita = aprovar_ata(&c.amb, &c.analise, true, hoje()).unwrap();
    assert!(!feita.ja_aprovada);
    assert!(feita.transcript_apagado);
    assert_eq!(feita.citacoes_marcadas, 2);

    let ata = c.ata();
    assert_eq!(ata.status, StatusAta::Aprovada);
    assert_eq!(
        ata.itens[0].citacoes[0].status,
        StatusCitacao::ConferidaEm(hoje())
    );
    assert!(!c.guardado());
    assert_eq!(c.estado_da_analise(), EstadoAnalise::Aprovada);
    let arquivos = c.amb.kb.reuniao(c.reuniao).unwrap();
    let texto = c.amb.kb.ler_texto(&arquivos.ata).unwrap();
    assert!(texto.contains("status: aprovada"), "{texto}");
    assert!(texto.contains("conferida em 2026-11-14"), "{texto}");

    let p = prontidao_ata(&c.amb, &c.analise).unwrap();
    assert!(p.concluida);
    assert!(!p.pode_aprovar);
    assert!(!p.transcript_guardado);

    // O agente não lê mais o transcript nem registra itens na ata aprovada.
    assert!(
        c.tentar("ler_transcript", json!({ "reuniao": "R-0001" }))
            .is_err()
    );
    assert!(
        c.tentar(
            "registrar_item_ata",
            json!({
                "reuniao": "R-0001", "tipo": "decisao", "texto": "Outra.",
                "citacoes": [{ "indice": 4, "trecho": "passa direto, sem checar o perfil" }]
            }),
        )
        .is_err()
    );
    assert_eq!(c.ata(), ata);

    // Repetir não muda nada.
    let de_novo = aprovar_ata(&c.amb, &c.analise, true, hoje()).unwrap();
    assert!(de_novo.ja_aprovada);
    assert!(!de_novo.transcript_apagado);
    assert_eq!(de_novo.citacoes_marcadas, 0);
    assert_eq!(c.ata(), ata);
}

#[test]
fn pendencias_impedem_a_aprovacao_sem_apagar_o_transcript() {
    let c = cenario();
    let p = prontidao_ata(&c.amb, &c.analise).unwrap();
    assert!(!p.pode_aprovar);
    assert_eq!(p.pendencias.len(), 2, "{:?}", p.pendencias);

    let e = aprovar_ata(&c.amb, &c.analise, true, hoje()).unwrap_err();
    assert!(matches!(e, ErroAprovacaoAta::Pendente(_)), "{e}");
    assert!(e.to_string().contains("concluir_analise"), "{e}");
    assert!(e.to_string().contains("não tem itens"), "{e}");
    assert!(c.guardado());
    assert_eq!(c.ata().status, StatusAta::Rascunho);
}

#[test]
fn a_analise_de_reuniao_so_e_aprovada_pela_ata() {
    let c = cenario();
    c.registrar_e_concluir();
    let e = aprovar_revisada(&c.amb, &c.analise, &OpcoesAprovacao::default()).unwrap_err();
    assert!(matches!(e, ErroRevisao::Reuniao(_)), "{e}");
    assert!(c.guardado());
    assert_eq!(c.estado_da_analise(), EstadoAnalise::Concluida);

    // E a análise de outro fluxo não passa pela aprovação da ata.
    let d = c.chamar(
        "iniciar_analise",
        json!({ "fluxo": "perguntar", "entrada": "Como cancelar uma fatura?" }),
    );
    let outra = Id::analisar(d["analise"].as_str().unwrap()).unwrap();
    let e = aprovar_ata(&c.amb, &outra, true, hoje()).unwrap_err();
    assert!(matches!(e, ErroAprovacaoAta::NaoEReuniao(_)), "{e}");
    assert!(c.guardado());
}

/// Aprovação que parou depois do descarte (a ata ficou em rascunho, sem o
/// transcript): aprovar de novo conclui.
#[test]
fn aprovar_de_novo_completa_a_aprovacao_que_parou_depois_do_descarte() {
    let c = cenario();
    c.registrar_e_concluir();
    let templates = Templates::da_base(&c.amb.base).0;
    Guarda::nova(&c.amb.dados)
        .descartar(
            &c.amb.kb,
            &templates,
            c.reuniao,
            hoje(),
            MotivoDescarte::AtaAprovada,
        )
        .unwrap();
    assert_eq!(c.ata().status, StatusAta::Rascunho);
    let p = prontidao_ata(&c.amb, &c.analise).unwrap();
    assert!(p.pode_aprovar);
    assert!(
        p.avisos.iter().any(|a| a.contains("já não está guardado")),
        "{:?}",
        p.avisos
    );

    let feita = aprovar_ata(&c.amb, &c.analise, true, hoje()).unwrap();
    assert!(!feita.transcript_apagado);
    assert_eq!(c.ata().status, StatusAta::Aprovada);
    assert_eq!(c.estado_da_analise(), EstadoAnalise::Aprovada);
}

/// Ata aprovada com o transcript ainda guardado (gravada por fora): a aprovação
/// continua oferecida, e repetir apaga o transcript.
#[test]
fn ata_aprovada_com_transcript_guardado_e_descartada_na_repeticao() {
    let c = cenario();
    c.registrar_e_concluir();
    Templates::da_base(&c.amb.base)
        .0
        .alterar_ata(&c.amb.kb, c.reuniao, |e: &mut EstadoAta| {
            e.status = StatusAta::Aprovada;
            Ok::<_, cba_docs::ErroAta>(())
        })
        .unwrap();
    let p = prontidao_ata(&c.amb, &c.analise).unwrap();
    assert!(!p.concluida);
    assert!(p.pode_aprovar, "{p:?}");

    let feita = aprovar_ata(&c.amb, &c.analise, true, hoje()).unwrap();
    assert!(feita.ja_aprovada);
    assert!(feita.transcript_apagado);
    assert!(!c.guardado());
    assert!(prontidao_ata(&c.amb, &c.analise).unwrap().concluida);
}
