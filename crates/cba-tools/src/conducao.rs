//! Condução da análise pelo workflow do Claude Code (spec
//! `docs/specs/2026-11-25-workflows-nas-analises.md`, seções 3.2, 3.6, 3.7 e 3.8;
//! #371): os pedidos que o app digita para a sessão, o roteiro curto que o
//! `iniciar_analise` devolve com `conducao: workflow`, os limites padrão e a
//! estimativa de custo mostrada em Nova análise.
//!
//! # Pedidos ao Claude Code
//!
//! A ferramenta Workflow só aceita ser chamada a pedido do analista, nas palavras
//! dele (spike da #352, R4): o pedido fica na mensagem que o app digita
//! ([`pedido_de_inicio`]), e não no roteiro. Os pedidos não têm aspas, `%`, `!`,
//! parênteses nem quebras de linha, que o `claude.cmd` do Windows recusa (spec da
//! #1, seção 6.4), e nomeiam a ferramenta Workflow, para o modelo não desviar pela
//! ferramenta Skill (R3).
//!
//! Os args do workflow vão como texto: `A-0042` no início e `A-0042 retomada` numa
//! conversa nova sobre uma análise já começada (o script aceita o texto e o objeto
//! `{analise, retomada, limites}`). A retomada na mesma sessão usa o nome e o
//! `resumeFromRunId` gravado pelo hook, com os mesmos args, para o cache valer (R8).

use std::fmt::Write as _;

use cba_core::analise::{Analise, Fluxo};
use cba_kb::Id;

use crate::roteiro::{nome_do_arquivo, secoes_obrigatorias};
use crate::workflows;

/// Frentes de investigação por padrão (spec, seção 3.6).
pub const FRENTES_PADRAO: u32 = 6;
/// Frentes no máximo.
pub const FRENTES_MAXIMO: u32 = 12;

/// Tipo de workflow (`impacto`) do fluxo, se o fluxo tem workflow padrão.
pub fn tipo_do_fluxo(fluxo: Fluxo) -> Option<&'static str> {
    let tipo = nome_do_arquivo(fluxo);
    workflows::padrao(tipo).map(|_| tipo)
}

/// Nome do workflow do fluxo (`cba-impacto`), se houver.
pub fn workflow_do_fluxo(fluxo: Fluxo) -> Option<String> {
    tipo_do_fluxo(fluxo).map(workflows::nome)
}

/// Agentes no pior caso, com `frentes` frentes: Preparar (1), Investigar e
/// Verificar (1 por frente cada) e Consolidar (1).
pub fn agentes_no_maximo(frentes: u32) -> u32 {
    2 + 2 * frentes
}

/// Pedido que abre a conversa de uma análise conduzida pelo workflow.
pub fn pedido_de_inicio(id: &Id, nome: &str) -> String {
    format!(
        "Inicie a análise {id} pelo ctower: chame a ferramenta iniciar_analise do cba \
         com analise {id} e conducao workflow. Depois chame a ferramenta Workflow com \
         name {nome} e args {id} e siga o roteiro devolvido."
    )
}

/// Pedido de uma conversa nova sobre uma análise conduzida pelo workflow que não
/// terminou: o `resumeFromRunId` só vale na mesma sessão, então o workflow roda de
/// novo com `retomada`, e a fase Preparar devolve só as frentes sem `investigado`
/// na memória (spec, seção 3.7).
pub fn pedido_de_retomada(id: &Id, nome: &str) -> String {
    format!(
        "Retome a análise {id} pelo ctower: chame a ferramenta retomar_analise do cba \
         com analise {id}. Depois chame a ferramenta Workflow com name {nome} e args \
         {id} retomada e siga o roteiro da análise."
    )
}

/// Pedido para a sessão que foi interrompida no meio do workflow e reaberta com
/// `--resume`: o que terminou volta do cache, sem registro duplicado (R8).
pub fn pedido_de_retomada_na_sessao(id: &Id, nome: &str, run_id: &str) -> String {
    format!(
        "Retome a análise {id} pelo ctower: a sessão foi interrompida no meio do \
         workflow. Chame a ferramenta Workflow com name {nome}, args {id} e \
         resumeFromRunId {run_id}, sem scriptPath, e siga o roteiro da análise."
    )
}

/// Pedido para continuar pela condução simples (spec, seção 3.8).
pub fn pedido_sem_workflow(id: &Id) -> String {
    format!(
        "Continue a análise {id} pelo ctower sem workflow: chame a ferramenta \
         iniciar_analise do cba com analise {id} e conducao simples e siga o roteiro \
         devolvido."
    )
}

/// O `runId` de um workflow é seguro para ir num pedido (só letras, números, `_` e
/// `-`, como `wf_3b0de87a-363`).
pub fn run_id_valido(run_id: &str) -> bool {
    !run_id.is_empty()
        && run_id.len() <= 100
        && run_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// A seção "Resposta no chat" do roteiro completo (o da base, se editado), sem o
/// título. `None` se o roteiro não a tem.
fn resposta_no_chat(roteiro: &str) -> Option<&str> {
    let inicio = roteiro.find("## Resposta no chat")?;
    let corpo = &roteiro[inicio..];
    let corpo = &corpo[corpo.find('\n')? + 1..];
    let fim = corpo.find("\n## ").unwrap_or(corpo.len());
    Some(corpo[..fim].trim())
}

/// Roteiro curto do `iniciar_analise` com `conducao: workflow` (spec, seção 3.2): o
/// workflow investiga, a sessão espera, conclui e responde no chat como o roteiro do
/// tipo manda, e volta à condução simples se a ferramenta faltar.
pub fn roteiro_curto(analise: &Analise, nome: &str, documento: &str, completo: &str) -> String {
    let id = &analise.id;
    let obrigatorias = secoes_obrigatorias(analise.fluxo)
        .iter()
        .map(|s| format!("`{s}`"))
        .collect::<Vec<_>>()
        .join(", ");
    let mut t = String::new();
    let _ = writeln!(
        t,
        "# Roteiro da análise {id}: {} (conduzida pelo workflow {nome})\n",
        analise.titulo
    );
    let _ = writeln!(
        t,
        "Documento na base: `{documento}` (montado pelo app a partir dos registros; não o \
         edite).\n"
    );
    let _ = writeln!(
        t,
        "Agentes do workflow: este roteiro é da sessão principal. Ignore-o e siga o seu \
         pedido.\n"
    );
    let _ = writeln!(t, "## Como conduzir\n");
    let revisao = analise.fluxo == Fluxo::Revisar;
    if revisao {
        let _ = writeln!(
            t,
            "O workflow `{nome}` faz a revisão: separa os critérios de aceite e as \
             afirmações do texto, avalia cada um contra a base e o código com um agente, \
             verifica os achados de cada critério com um agente cético independente e \
             consolida (a completude do texto inteiro, o INVEST, se for uma história, e o \
             veredito). Os agentes registram direto no parecer (`registrar_achado`, \
             `perguntar`, `afirmar`, `memorizar`, `contestar_achado`, `registrar_invest`, \
             `registrar_veredito`). Você não repete a revisão.\n"
        );
    } else {
        let _ = writeln!(
            t,
            "O workflow `{nome}` faz a investigação: separa as frentes, investiga cada uma \
             com um agente, verifica cada frente com um agente cético independente e \
             consolida. Os agentes registram direto nesta análise (`afirmar`, `perguntar`, \
             `propor_termo`, `memorizar`, `contestar_afirmacao`). Você não repete a \
             investigação.\n"
        );
    }
    let _ = writeln!(
        t,
        "1. Chame a ferramenta Workflow com name `{nome}` e os args que o analista pediu \
         (`{id}`), se ainda não chamou. Ela roda em segundo plano: espere o aviso de fim \
         sem investigar nem registrar nada enquanto isso."
    );
    if revisao {
        let _ = writeln!(
            t,
            "2. Quando o workflow terminar, chame `concluir_analise` (analise: \"{id}\"). Os \
             achados contestados pela verificação independente aparecem como pendência: \
             eles ficam para o analista decidir; não os apague nem os registre de novo. Se \
             faltar o veredito ou houver termo vago sem achado, registre o que falta \
             seguindo o roteiro completo (`retomar_analise` mostra o estado) e chame \
             `concluir_analise` de novo."
        );
    } else {
        let _ = writeln!(
            t,
            "2. Quando o workflow terminar, chame `concluir_analise` (analise: \"{id}\"). As \
             afirmações contestadas pela verificação independente aparecem como pendência: \
             elas ficam para o analista decidir; não as apague nem as registre de novo. Se \
             faltar afirmação numa seção obrigatória ({obrigatorias}), registre-a com \
             `afirmar`, seguindo o roteiro completo (`retomar_analise` mostra o estado), e \
             chame `concluir_analise` de novo."
        );
    }
    let _ = writeln!(
        t,
        "3. Se o workflow terminar com agentes com erro, chame `retomar_analise`, \
         complete o que faltar pelo roteiro completo e conclua."
    );
    let _ = writeln!(t, "4. Responda no chat como abaixo.\n");
    let _ = writeln!(t, "## Se o workflow não puder rodar\n");
    let _ = writeln!(
        t,
        "Se a ferramenta Workflow não existir nesta sessão, for recusada ou falhar ao \
         carregar o script, não tente outro caminho (ferramenta Skill, scriptPath ou um \
         script seu): chame `iniciar_analise` de novo com analise \"{id}\", conducao \
         \"simples\" e `motivo` (o que aconteceu, em uma frase), e siga o roteiro completo \
         devolvido. Os registros que já existem ficam.\n"
    );
    let _ = writeln!(t, "## Resposta no chat\n");
    match resposta_no_chat(completo) {
        Some(r) => {
            let _ = writeln!(t, "{r}");
        }
        None => {
            let _ = writeln!(
                t,
                "Depois de `concluir_analise`, dê ao analista um resumo curto e o caminho \
                 do documento. Não copie o documento para o chat."
            );
        }
    }
    let _ = writeln!(
        t,
        "\nDiga também {} a verificação independente contestou: o analista {} revê no app.",
        if revisao {
            "quantos achados"
        } else {
            "quantas afirmações"
        },
        if revisao { "os" } else { "as" }
    );
    t
}

/// Tokens de um agente de uma fase, medidos com o `cba-avaliacao` (`rodar
/// --conducao workflow`): a faixa vai do menor ao maior agente da fase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokensDaFase {
    pub fase: &'static str,
    pub minimo: u64,
    pub maximo: u64,
}

/// Estimativa de um tipo de workflow, a partir da medição (spec, seção 3.6).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Estimativa {
    pub tipo: &'static str,
    /// Tokens por agente, por fase.
    pub fases: &'static [TokensDaFase],
    /// Razão do total de tokens em relação à condução simples do mesmo tipo.
    pub razao_simples: f64,
    /// Duração medida, em minutos (do menor ao maior caso).
    pub minutos: (u32, u32),
    /// Relatório da medição, em `docs/avaliacao/`.
    pub relatorio: &'static str,
}

/// Estimativas medidas, por tipo. Antes da medição de um tipo, a tela mostra só o
/// número máximo de agentes.
///
/// `impacto`: os quatro casos do gabarito (D-FAT-001 a 004), uma rodada com Sonnet e o
/// Claude Code 2.1.332 (2 a 4 frentes por caso): por agente, do menor ao maior. O
/// total foi cerca de 4,7 vezes o da condução simples, e a duração de relógio, de 3
/// a 4 min (a simples levou cerca de 1,5 min).
///
/// `revisar`: o caso REV-FAT-001, uma rodada com Sonnet e o Claude Code 2.1.332 (5
/// critérios): cerca de 2,9 vezes os tokens da condução simples e 2 min 23 s de
/// relógio (a simples levou 1 min 20 s).
pub const ESTIMATIVAS: &[Estimativa] = &[
    Estimativa {
        tipo: "impacto",
        fases: &[
            TokensDaFase {
                fase: "Preparar",
                minimo: 116_000,
                maximo: 170_000,
            },
            TokensDaFase {
                fase: "Investigar",
                minimo: 371_000,
                maximo: 1_300_000,
            },
            TokensDaFase {
                fase: "Verificar",
                minimo: 30_000,
                maximo: 84_000,
            },
            TokensDaFase {
                fase: "Consolidar",
                minimo: 100_000,
                maximo: 267_000,
            },
        ],
        razao_simples: 4.7,
        minutos: (3, 4),
        relatorio: "docs/avaliacao/2026-11-25-workflow-impacto-sonnet.md",
    },
    Estimativa {
        tipo: "revisar",
        fases: &[
            TokensDaFase {
                fase: "Separar",
                minimo: 121_000,
                maximo: 121_000,
            },
            TokensDaFase {
                fase: "Avaliar",
                minimo: 134_000,
                maximo: 258_000,
            },
            TokensDaFase {
                fase: "Verificar",
                minimo: 24_000,
                maximo: 40_000,
            },
            TokensDaFase {
                fase: "Consolidar",
                minimo: 81_000,
                maximo: 81_000,
            },
        ],
        razao_simples: 2.9,
        minutos: (2, 3),
        relatorio: "docs/avaliacao/2026-11-25-workflow-revisar-sonnet.md",
    },
];

/// Estimativa do tipo, se já medida.
pub fn estimativa(tipo: &str) -> Option<&'static Estimativa> {
    ESTIMATIVAS.iter().find(|e| e.tipo == tipo)
}

/// Tipo com workflow que Nova análise não oferece: a medição não mostrou ganho sobre
/// a condução simples. O workflow continua instalado pelo preparo e customizável em
/// Configurações › Workflows, e o `iniciar_analise` aceita `conducao: workflow` (a
/// avaliação usa esse caminho).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NaoOferecido {
    pub tipo: &'static str,
    /// Relatório da medição, em `docs/avaliacao/`.
    pub relatorio: &'static str,
}

/// `revisar` (#373): no REV-FAT-001, a condução simples já achou os 3 achados
/// esperados, os 4 termos vagos, a pergunta esperada e o veredito; o workflow repetiu
/// o resultado com cerca de 2,9 vezes os tokens e 3 vezes o custo.
pub const NAO_OFERECIDOS: &[NaoOferecido] = &[NaoOferecido {
    tipo: "revisar",
    relatorio: "docs/avaliacao/2026-11-25-workflow-revisar-sonnet.md",
}];

/// O tipo tem workflow e Nova análise oferece a condução Aprofundada.
pub fn oferecido(tipo: &str) -> bool {
    workflows::padrao(tipo).is_some() && !NAO_OFERECIDOS.iter().any(|n| n.tipo == tipo)
}

impl Estimativa {
    /// Faixa de tokens do workflow inteiro com `frentes` frentes: a primeira e a
    /// última fase uma vez; as por frente (Investigar ou Avaliar, e Verificar) uma
    /// vez por frente.
    pub fn faixa(&self, frentes: u32) -> (u64, u64) {
        self.fases
            .iter()
            .map(|f| f.faixa(frentes))
            .fold((0, 0), |a, f| (a.0 + f.0, a.1 + f.1))
    }
}

/// A fase roda um agente por frente (Investigar ou Avaliar, e Verificar); as outras,
/// um agente só.
pub fn por_frente(fase: &str) -> bool {
    matches!(fase, "Investigar" | "Avaliar" | "Verificar")
}

impl TokensDaFase {
    /// Faixa de tokens da fase inteira com `frentes` frentes.
    pub fn faixa(&self, frentes: u32) -> (u64, u64) {
        let vezes = if por_frente(self.fase) {
            u64::from(frentes)
        } else {
            1
        };
        (self.minimo * vezes, self.maximo * vezes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cba_kb::Serie;

    /// O pedido vai como argumento do `claude` e como texto do link: sem o que o
    /// `claude.cmd` do Windows recusa.
    fn seguro(pedido: &str) {
        for proibido in ['"', '\'', '%', '!', '(', ')', '\n', '\r', '`'] {
            assert!(!pedido.contains(proibido), "{proibido:?} em {pedido}");
        }
    }

    #[test]
    fn pedidos_sem_caracteres_recusados_e_com_a_ferramenta_workflow() {
        let id = Serie::Analise.id(42);
        let nome = workflow_do_fluxo(Fluxo::Impacto).unwrap();
        assert_eq!(nome, "cba-impacto");
        for p in [
            pedido_de_inicio(&id, &nome),
            pedido_de_retomada(&id, &nome),
            pedido_de_retomada_na_sessao(&id, &nome, "wf_3b0de87a-363"),
            pedido_sem_workflow(&id),
        ] {
            seguro(&p);
            assert!(p.contains("A-0042"), "{p}");
        }
        let inicio = pedido_de_inicio(&id, &nome);
        assert!(inicio.contains("conducao workflow"));
        assert!(inicio.contains("ferramenta Workflow com name cba-impacto e args A-0042"));
        assert!(pedido_de_retomada(&id, &nome).contains("args A-0042 retomada"));
        assert!(pedido_de_retomada_na_sessao(&id, &nome, "wf_1").contains("resumeFromRunId wf_1"));
        assert!(pedido_sem_workflow(&id).contains("conducao simples"));
    }

    #[test]
    fn impacto_e_revisar_tem_workflow_e_o_maximo_de_agentes() {
        assert_eq!(tipo_do_fluxo(Fluxo::Impacto), Some("impacto"));
        assert_eq!(
            workflow_do_fluxo(Fluxo::Revisar).as_deref(),
            Some("cba-revisar")
        );
        // Histórias e cenários ficam sem workflow (spec, seção 4-D; #373).
        for f in [
            Fluxo::Perguntar,
            Fluxo::Explicar,
            Fluxo::Reuniao,
            Fluxo::Historias,
            Fluxo::Cenarios,
        ] {
            assert_eq!(tipo_do_fluxo(f), None);
        }
        assert_eq!(agentes_no_maximo(FRENTES_PADRAO), 14);
        assert_eq!(agentes_no_maximo(FRENTES_MAXIMO), 26);
        let e = estimativa("impacto").unwrap();
        let (min, max) = e.faixa(FRENTES_PADRAO);
        assert!(min < max && e.razao_simples > 1.0);
        assert!(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join(e.relatorio)
                .is_file()
        );
        assert!(estimativa("perguntar").is_none());
        let raiz = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        // A revisão foi medida, e a Aprofundada não é oferecida nela (#373).
        let r = estimativa("revisar").unwrap();
        assert!(raiz.join(r.relatorio).is_file());
        let (min, _) = r.faixa(2);
        assert_eq!(min, 121_000 + 2 * (134_000 + 24_000) + 81_000);
        assert!(oferecido("impacto"));
        assert!(!oferecido("revisar") && !oferecido("perguntar"));
        for n in NAO_OFERECIDOS {
            assert!(workflows::padrao(n.tipo).is_some(), "{}", n.tipo);
            assert!(raiz.join(n.relatorio).is_file(), "{}", n.relatorio);
        }
    }

    #[test]
    fn run_id_seguro_para_o_pedido() {
        assert!(run_id_valido("wf_3b0de87a-363"));
        for errado in ["", "wf 1", "wf_1;rm", "wf_(1)", "wf_\"1"] {
            assert!(!run_id_valido(errado), "{errado}");
        }
    }

    #[test]
    fn roteiro_curto_reaproveita_a_resposta_no_chat_e_traz_o_caminho_de_volta() {
        let a = Analise::nova(
            Serie::Analise.id(3),
            "p",
            Fluxo::Impacto,
            "Parcelamento PF",
            "Permitir parcelar",
            1,
        );
        let completo = "# R\n\n## Como conduzir\n\n1. x\n\n## Resposta no chat\n\nO chat \
                        não é o documento.\n- a abrangência\n\n## Contexto da base\n\nnada";
        let r = roteiro_curto(&a, "cba-impacto", "analises/A-0003.md", completo);
        assert!(
            r.contains("O chat não é o documento.\n- a abrangência"),
            "{r}"
        );
        assert!(!r.contains("Contexto da base"), "{r}");
        assert!(r.contains("conducao \"simples\""));
        assert!(r.contains("`concluir_analise`"));
        assert!(r.contains("`demanda_como_entendida`"));
        let sem = roteiro_curto(&a, "cba-impacto", "d", "# só o título");
        assert!(sem.contains("Não copie o documento"));
    }

    #[test]
    fn faixa_multiplica_as_fases_por_frente() {
        const FASES: &[TokensDaFase] = &[
            TokensDaFase {
                fase: "Preparar",
                minimo: 10,
                maximo: 20,
            },
            TokensDaFase {
                fase: "Investigar",
                minimo: 100,
                maximo: 200,
            },
            TokensDaFase {
                fase: "Verificar",
                minimo: 50,
                maximo: 60,
            },
            TokensDaFase {
                fase: "Consolidar",
                minimo: 30,
                maximo: 40,
            },
        ];
        let e = Estimativa {
            tipo: "impacto",
            fases: FASES,
            razao_simples: 3.0,
            minutos: (5, 10),
            relatorio: "",
        };
        assert_eq!(e.faixa(2), (10 + 200 + 100 + 30, 20 + 400 + 120 + 40));
    }
}
