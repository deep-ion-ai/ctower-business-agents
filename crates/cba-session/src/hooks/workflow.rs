//! Acompanhamento dos workflows das análises pelos hooks (spec
//! `docs/specs/2026-11-25-workflows-nas-analises.md`, seções 3.4 e 3.8; #371).
//!
//! | Evento | O que o receptor faz na análise ligada à sessão |
//! |---|---|
//! | `PostToolUse` da ferramenta `Workflow` com `name` `cba-*` | grava a execução ([`ExecucaoWorkflow`]): nome, `runId`, `taskId`, sessão, origem e hash do cabeçalho do script e as fases do `meta.phases`; a análise passa a `conducao: workflow` |
//! | `SubagentStart`/`SubagentStop` com `agent_type: workflow-subagent` | soma os agentes iniciados e concluídos e relê as fases do `journal.jsonl` |
//! | `UserPromptSubmit` com `<task-notification>` da tarefa do workflow | fecha a execução: situação e uso (`<usage>`) |
//! | `Stop` sem workflow em segundo plano, numa análise com `conducao: workflow` que nunca lançou o workflow | grava o aviso "Conduzida sem workflow" (rede de segurança da seção 3.8) |
//! | `SessionEnd` com o workflow rodando | a execução fica interrompida (cabe a retomada) |
//!
//! O `Stop` com um workflow `running` em `background_tasks` mantém a sessão ativa
//! ([`rodando_em_segundo_plano`], usado em `destino`).
//!
//! O `journal.jsonl` (uma linha `started` por agente, com `phase`, e uma `result`) e
//! o texto do `<task-notification>` são formato interno do Claude Code (spike da
//! #352, R6 e R7; payloads conferidos com o Claude Code 2.1.331). Uma leitura que
//! falha deixa só as contagens dos hooks, e nunca é erro.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use cba_core::analise::Analise;
use cba_core::conducao::{Conducao, ExecucaoWorkflow, FaseWorkflow, SituacaoWorkflow, UsoWorkflow};
use serde_json::Value;

use crate::preparo::workflows::ler_cabecalho;

/// `agent_type` dos agentes de um workflow nos hooks `SubagentStart`/`SubagentStop`.
const TIPO_DO_AGENTE: &str = "workflow-subagent";
/// Prefixo dos workflows do app.
const PREFIXO: &str = cba_tools::workflows::PREFIXO;
/// Arquivo com o progresso dos agentes, na pasta da execução.
const JOURNAL: &str = "journal.jsonl";
/// Tamanho máximo do `journal.jsonl` lido (o hook precisa ser rápido).
const JOURNAL_MAXIMO: u64 = 4 * 1024 * 1024;

/// Aviso da rede de segurança: a sessão parou sem lançar o workflow.
pub const SEM_WORKFLOW: &str = "a sessão terminou a resposta sem iniciar o workflow (a ferramenta Workflow pode não existir nesta sessão ou ter sido recusada)";

fn texto<'a>(v: &'a Value, campo: &str) -> Option<&'a str> {
    v.get(campo).and_then(Value::as_str)
}

/// `background_tasks` do `Stop` (e do `SubagentStop`) com um workflow rodando.
pub fn rodando_em_segundo_plano(dados: &Value) -> bool {
    dados
        .get("background_tasks")
        .and_then(Value::as_array)
        .is_some_and(|tarefas| {
            tarefas.iter().any(|t| {
                texto(t, "type") == Some("workflow") && texto(t, "status") == Some("running")
            })
        })
}

/// Lançamento de um workflow do app, do `PostToolUse` da ferramenta `Workflow`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lancamento {
    pub nome: String,
    pub run_id: String,
    pub tarefa: Option<String>,
    pub transcricao: Option<PathBuf>,
    pub origem: Option<String>,
    pub hash: Option<String>,
    /// Títulos do `meta.phases` do script.
    pub fases: Vec<String>,
    pub retomada: bool,
}

/// Fim de uma tarefa em segundo plano, do `<task-notification>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fim {
    pub tarefa: Option<String>,
    pub situacao: SituacaoWorkflow,
    pub uso: Option<UsoWorkflow>,
}

/// O que um evento diz sobre o workflow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Evento {
    Lancado(Lancamento),
    /// `SubagentStart` de um agente do workflow.
    AgenteIniciou,
    /// `SubagentStop` de um agente do workflow, com a pasta da execução (pasta do
    /// `agent_transcript_path`).
    AgenteTerminou {
        transcricao: Option<PathBuf>,
    },
    Fim(Fim),
    /// `Stop` sem workflow em segundo plano.
    Parada,
    /// `SessionEnd`.
    Encerrada,
}

/// Lê o evento do hook, se ele diz respeito a um workflow.
pub fn evento(evento: &str, dados: &Value) -> Option<Evento> {
    match evento {
        "PostToolUse" if texto(dados, "tool_name") == Some("Workflow") => {
            lancamento(dados).map(Evento::Lancado)
        }
        "SubagentStart" if texto(dados, "agent_type") == Some(TIPO_DO_AGENTE) => {
            Some(Evento::AgenteIniciou)
        }
        "SubagentStop" if texto(dados, "agent_type") == Some(TIPO_DO_AGENTE) => {
            Some(Evento::AgenteTerminou {
                transcricao: texto(dados, "agent_transcript_path")
                    .and_then(|p| Path::new(p).parent())
                    .map(Path::to_path_buf),
            })
        }
        "UserPromptSubmit" => texto(dados, "prompt")
            .and_then(aviso_de_fim)
            .map(Evento::Fim),
        "Stop" if !rodando_em_segundo_plano(dados) => Some(Evento::Parada),
        "SessionEnd" => Some(Evento::Encerrada),
        _ => None,
    }
}

fn lancamento(dados: &Value) -> Option<Lancamento> {
    let entrada = dados.get("tool_input")?;
    let resposta = dados.get("tool_response")?;
    let nome = texto(entrada, "name")?;
    if !nome.starts_with(PREFIXO) {
        return None;
    }
    let run_id = texto(resposta, "runId")?;
    let script = texto(entrada, "script").unwrap_or_default();
    let cabecalho = ler_cabecalho(script);
    Some(Lancamento {
        nome: nome.to_owned(),
        run_id: run_id.to_owned(),
        tarefa: texto(resposta, "taskId").map(str::to_owned),
        transcricao: texto(resposta, "transcriptDir").map(PathBuf::from),
        origem: cabecalho
            .as_ref()
            .and_then(|c| c.origem)
            .map(|o| o.nome().to_owned()),
        hash: cabecalho.map(|c| c.hash),
        fases: fases_do_meta(script),
        retomada: entrada.get("resumeFromRunId").is_some(),
    })
}

/// Títulos de `meta.phases`, na ordem, lidos sem interpretar o JavaScript: os
/// `title: '...'` entre `export const meta` e o fim do objeto. Melhor esforço.
pub fn fases_do_meta(script: &str) -> Vec<String> {
    let Some(inicio) = script.find("export const meta") else {
        return Vec::new();
    };
    let meta = &script[inicio..];
    let meta = &meta[..meta.find("\n}").unwrap_or(meta.len())];
    let Some(fases) = meta.find("phases") else {
        return Vec::new();
    };
    let mut titulos = Vec::new();
    let mut resto = &meta[fases..];
    while let Some(i) = resto.find("title") {
        resto = resto[i + "title".len()..].trim_start();
        let Some(depois) = resto.strip_prefix(':') else {
            continue;
        };
        let depois = depois.trim_start();
        let Some(aspa) = depois
            .chars()
            .next()
            .filter(|c| matches!(c, '\'' | '"' | '`'))
        else {
            continue;
        };
        let corpo = &depois[1..];
        if let Some(fim) = corpo.find(aspa) {
            titulos.push(corpo[..fim].to_owned());
            resto = &corpo[fim + 1..];
        }
    }
    titulos
}

/// Conteúdo de `<tag>...</tag>` em `texto`.
fn tag<'a>(texto: &'a str, nome: &str) -> Option<&'a str> {
    let abre = format!("<{nome}>");
    let fecha = format!("</{nome}>");
    let inicio = texto.find(&abre)? + abre.len();
    let fim = texto[inicio..].find(&fecha)? + inicio;
    Some(texto[inicio..fim].trim())
}

/// Aviso de fim de uma tarefa em segundo plano (`<task-notification>`), como o
/// Claude Code 2.1.331 o põe na mensagem seguinte.
pub fn aviso_de_fim(prompt: &str) -> Option<Fim> {
    let corpo = tag(prompt.trim_start(), "task-notification")?;
    let situacao = match tag(corpo, "status")? {
        "completed" => SituacaoWorkflow::Concluido,
        "failed" | "error" => SituacaoWorkflow::Falhou,
        _ => SituacaoWorkflow::Interrompido,
    };
    let uso = tag(corpo, "usage").map(|u| {
        let n = |campo: &str| tag(u, campo).and_then(|v| v.parse::<u64>().ok());
        UsoWorkflow {
            agentes: n("agent_count").unwrap_or(0) as u32,
            agentes_com_erro: n("agents_error").unwrap_or(0) as u32,
            tokens: n("subagent_tokens").unwrap_or(0),
            ferramentas: n("tool_uses").unwrap_or(0) as u32,
            duracao_ms: n("duration_ms").unwrap_or(0),
        }
    });
    Some(Fim {
        tarefa: tag(corpo, "task-id").map(str::to_owned),
        situacao,
        uso,
    })
}

/// Agentes iniciados e concluídos por fase, do `journal.jsonl` da pasta da
/// execução. `None` se não deu para ler.
pub fn fases_do_journal(pasta: &Path) -> Option<Vec<FaseWorkflow>> {
    let arquivo = pasta.join(JOURNAL);
    if std::fs::metadata(&arquivo).ok()?.len() > JOURNAL_MAXIMO {
        return None;
    }
    let texto = std::fs::read_to_string(&arquivo).ok()?;
    Some(fases_das_linhas(&texto))
}

/// [`fases_do_journal`] a partir do texto: as fases na ordem em que aparecem.
pub fn fases_das_linhas(texto: &str) -> Vec<FaseWorkflow> {
    let mut fases: Vec<FaseWorkflow> = Vec::new();
    let mut fase_do_agente: HashMap<String, usize> = HashMap::new();
    for linha in texto.lines() {
        let Ok(v) = serde_json::from_str::<Value>(linha) else {
            continue;
        };
        let agente = texto_de(&v, "agentId").or_else(|| texto_de(&v, "key"));
        match texto_de(&v, "type") {
            Some("started") => {
                let titulo = texto_de(&v, "phase").unwrap_or_default();
                let i = match fases.iter().position(|f| f.titulo == titulo) {
                    Some(i) => i,
                    None => {
                        fases.push(FaseWorkflow {
                            titulo: titulo.to_owned(),
                            ..Default::default()
                        });
                        fases.len() - 1
                    }
                };
                fases[i].iniciados += 1;
                if let Some(a) = agente {
                    fase_do_agente.insert(a.to_owned(), i);
                }
            }
            Some("result") => {
                if let Some(&i) = agente.and_then(|a| fase_do_agente.get(a)) {
                    fases[i].concluidos += 1;
                }
            }
            _ => {}
        }
    }
    fases
}

fn texto_de<'a>(v: &'a Value, campo: &str) -> Option<&'a str> {
    v.get(campo).and_then(Value::as_str)
}

/// Junta as contagens lidas do journal às fases do `meta` (que vêm primeiro, na
/// ordem do script). Fase do journal fora do `meta` entra no fim.
fn juntar(fases: &mut Vec<FaseWorkflow>, lidas: Vec<FaseWorkflow>) {
    for lida in lidas {
        if lida.titulo.is_empty() {
            continue;
        }
        match fases.iter_mut().find(|f| f.titulo == lida.titulo) {
            Some(f) => {
                f.iniciados = lida.iniciados;
                f.concluidos = lida.concluidos;
            }
            None => fases.push(lida),
        }
    }
}

/// Leva o evento à análise. `journal` são as fases lidas da pasta da execução, se
/// houve leitura. Devolve se a análise mudou.
pub fn aplicar(
    a: &mut Analise,
    evento: &Evento,
    sessao: &str,
    agora_ms: u64,
    journal: Option<Vec<FaseWorkflow>>,
) -> bool {
    let antes = (a.workflow.clone(), a.conducao, a.aviso_conducao.clone());
    match evento {
        Evento::Lancado(l) => {
            let fases = l
                .fases
                .iter()
                .map(|t| FaseWorkflow {
                    titulo: t.clone(),
                    ..Default::default()
                })
                .collect();
            a.workflow = Some(ExecucaoWorkflow {
                nome: l.nome.clone(),
                origem: l.origem.clone(),
                hash: l.hash.clone(),
                run_id: l.run_id.clone(),
                tarefa: l.tarefa.clone(),
                sessao: sessao.to_owned(),
                situacao: SituacaoWorkflow::Rodando,
                fases,
                iniciados: 0,
                concluidos: 0,
                uso: None,
                retomada: l.retomada,
                iniciado_ms: agora_ms,
                atualizado_ms: agora_ms,
            });
            a.conducao = Conducao::Workflow;
            a.aviso_conducao = None;
        }
        Evento::AgenteIniciou | Evento::AgenteTerminou { .. } => {
            let Some(w) = a.workflow.as_mut().filter(|w| w.sessao == sessao) else {
                return false;
            };
            if matches!(evento, Evento::AgenteIniciou) {
                w.iniciados += 1;
            } else {
                w.concluidos += 1;
            }
            if let Some(lidas) = journal {
                juntar(&mut w.fases, lidas);
            }
            w.atualizado_ms = agora_ms;
        }
        Evento::Fim(fim) => {
            let Some(w) = a.workflow.as_mut().filter(|w| {
                w.sessao == sessao
                    && match (&fim.tarefa, &w.tarefa) {
                        (Some(f), Some(t)) => f == t,
                        _ => true,
                    }
            }) else {
                return false;
            };
            w.situacao = fim.situacao;
            w.uso = fim.uso.clone().or(w.uso.take());
            if let Some(lidas) = journal {
                juntar(&mut w.fases, lidas);
            }
            w.atualizado_ms = agora_ms;
        }
        Evento::Parada => {
            if a.conducao == Conducao::Workflow
                && a.workflow.is_none()
                && a.aviso_conducao.is_none()
            {
                a.aviso_conducao = Some(SEM_WORKFLOW.to_owned());
            }
        }
        Evento::Encerrada => {
            if let Some(w) = a
                .workflow
                .as_mut()
                .filter(|w| w.sessao == sessao && w.rodando())
            {
                w.situacao = SituacaoWorkflow::Interrompido;
                w.atualizado_ms = agora_ms;
            }
        }
    }
    antes != (a.workflow.clone(), a.conducao, a.aviso_conducao.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cba_core::analise::Fluxo;
    use cba_kb::Serie;
    use serde_json::json;

    fn analise() -> Analise {
        let mut a = Analise::nova(Serie::Analise.id(1), "p", Fluxo::Impacto, "t", "e", 1);
        a.conducao = Conducao::Workflow;
        a
    }

    /// O `PostToolUse` real do Claude Code 2.1.331 (spike da #352, resumido).
    fn lancado() -> Value {
        let script = cba_session_script();
        json!({
            "session_id": "s1",
            "hook_event_name": "PostToolUse",
            "tool_name": "Workflow",
            "tool_input": {"name": "cba-impacto", "args": "A-0001", "script": script},
            "tool_response": {
                "status": "async_launched",
                "taskId": "wd8yad8pz",
                "taskType": "local_workflow",
                "workflowName": "cba-impacto",
                "runId": "wf_3b0de87a-363",
                "transcriptDir": "/tmp/x/subagents/workflows/wf_3b0de87a-363",
                "scriptPath": "/tmp/x/workflows/scripts/cba-impacto-wf_3b0de87a-363.js"
            }
        })
    }

    fn cba_session_script() -> String {
        let w = cba_tools::workflows::WorkflowEfetivo::padrao("impacto").unwrap();
        crate::preparo::workflows::conteudo_instalado(&w)
    }

    const NOTIFICACAO: &str = "<task-notification>\n<task-id>wd8yad8pz</task-id>\n<tool-use-id>toolu_01</tool-use-id>\n<status>completed</status>\n<summary>Dynamic workflow \"x\" completed</summary>\n<result>{\"a\":1}</result>\n<usage><agent_count>3</agent_count><agents_done>3</agents_done><agents_error>0</agents_error><agents_skipped>0</agents_skipped><agents_empty_result>0</agents_empty_result><subagent_tokens>32172</subagent_tokens><tool_uses>6</tool_uses><duration_ms>5112</duration_ms></usage>\n</task-notification>";

    #[test]
    fn le_o_lancamento_com_o_cabecalho_e_as_fases_do_meta() {
        let Some(Evento::Lancado(l)) = evento("PostToolUse", &lancado()) else {
            panic!()
        };
        assert_eq!(l.nome, "cba-impacto");
        assert_eq!(l.run_id, "wf_3b0de87a-363");
        assert_eq!(l.tarefa.as_deref(), Some("wd8yad8pz"));
        assert_eq!(l.origem.as_deref(), Some("padrao"));
        assert_eq!(l.hash.as_deref().map(str::len), Some(64));
        assert_eq!(
            l.fases,
            ["Preparar", "Investigar", "Verificar", "Consolidar"]
        );
        assert!(!l.retomada);
        // Outro workflow, ou sem o runId: não é do app.
        let mut outro = lancado();
        outro["tool_input"]["name"] = json!("deep-research");
        assert_eq!(evento("PostToolUse", &outro), None);
        let mut sem = lancado();
        sem["tool_response"] = json!({"status": "error"});
        assert_eq!(evento("PostToolUse", &sem), None);
    }

    #[test]
    fn stop_com_workflow_rodando_nao_e_parada() {
        let rodando = json!({"background_tasks": [
            {"id": "w", "type": "workflow", "status": "running", "name": "cba-impacto"}
        ]});
        assert!(rodando_em_segundo_plano(&rodando));
        assert_eq!(evento("Stop", &rodando), None);
        let fim = json!({"background_tasks": []});
        assert!(!rodando_em_segundo_plano(&fim));
        assert_eq!(evento("Stop", &fim), Some(Evento::Parada));
        let outra = json!({"background_tasks": [{"type": "shell", "status": "running"}]});
        assert!(!rodando_em_segundo_plano(&outra));
    }

    #[test]
    fn le_o_aviso_de_fim() {
        let fim = aviso_de_fim(NOTIFICACAO).unwrap();
        assert_eq!(fim.tarefa.as_deref(), Some("wd8yad8pz"));
        assert_eq!(fim.situacao, SituacaoWorkflow::Concluido);
        let uso = fim.uso.unwrap();
        assert_eq!((uso.agentes, uso.tokens, uso.duracao_ms), (3, 32172, 5112));
        assert!(aviso_de_fim("Inicie a análise A-0001").is_none());
        let falhou = NOTIFICACAO.replace("completed</status>", "failed</status>");
        assert_eq!(
            aviso_de_fim(&falhou).unwrap().situacao,
            SituacaoWorkflow::Falhou
        );
    }

    #[test]
    fn fases_do_journal_por_agente() {
        let journal = r#"{"type":"launched"}
{"type":"started","key":"v2:1","agentId":"a1","label":"Preparar","phase":"Preparar"}
{"type":"result","key":"v2:1","agentId":"a1","result":"ok"}
{"type":"started","key":"v2:2","agentId":"a2","label":"Investigar: x","phase":"Investigar"}
{"type":"started","key":"v2:3","agentId":"a3","label":"Investigar: y","phase":"Investigar"}
{"type":"result","key":"v2:3","agentId":"a3","result":{}}
linha quebrada
"#;
        let fases = fases_das_linhas(journal);
        assert_eq!(fases.len(), 2);
        assert_eq!(
            (
                fases[1].titulo.as_str(),
                fases[1].iniciados,
                fases[1].concluidos
            ),
            ("Investigar", 2, 1)
        );
        assert_eq!(fases_do_journal(Path::new("/nao/existe")), None);
    }

    #[test]
    fn execucao_completa_na_analise() {
        let mut a = analise();
        let Some(l) = evento("PostToolUse", &lancado()) else {
            panic!()
        };
        assert!(aplicar(&mut a, &l, "s1", 10, None));
        let w = a.workflow.clone().unwrap();
        assert!(w.rodando());
        assert_eq!(w.fases.len(), 4);
        assert!(aplicar(&mut a, &Evento::AgenteIniciou, "s1", 11, None));
        let journal = vec![FaseWorkflow {
            titulo: "Preparar".into(),
            iniciados: 1,
            concluidos: 1,
        }];
        assert!(aplicar(
            &mut a,
            &Evento::AgenteTerminou { transcricao: None },
            "s1",
            12,
            Some(journal)
        ));
        let w = a.workflow.clone().unwrap();
        assert_eq!((w.iniciados, w.concluidos), (1, 1));
        assert_eq!((w.fases[0].iniciados, w.fases[0].concluidos), (1, 1));
        // Outra sessão não mexe na execução.
        assert!(!aplicar(&mut a, &Evento::AgenteIniciou, "s2", 13, None));
        // A parada com o workflow lançado não gera aviso.
        assert!(!aplicar(&mut a, &Evento::Parada, "s1", 13, None));
        let fim = Evento::Fim(aviso_de_fim(NOTIFICACAO).unwrap());
        assert!(aplicar(&mut a, &fim, "s1", 14, None));
        let w = a.workflow.clone().unwrap();
        assert_eq!(w.situacao, SituacaoWorkflow::Concluido);
        assert_eq!(w.uso.unwrap().tokens, 32172);
        // Encerrar depois do fim não muda nada.
        assert!(!aplicar(&mut a, &Evento::Encerrada, "s1", 15, None));
    }

    #[test]
    fn sessao_encerrada_no_meio_interrompe_e_parada_sem_workflow_avisa() {
        let mut a = analise();
        assert!(aplicar(&mut a, &Evento::Parada, "s1", 1, None));
        assert_eq!(a.aviso_conducao.as_deref(), Some(SEM_WORKFLOW));
        let Some(l) = evento("PostToolUse", &lancado()) else {
            panic!()
        };
        aplicar(&mut a, &l, "s1", 2, None);
        assert_eq!(a.aviso_conducao, None);
        assert!(aplicar(&mut a, &Evento::Encerrada, "s1", 3, None));
        assert_eq!(a.workflow.unwrap().situacao, SituacaoWorkflow::Interrompido);
        // Condução simples: a parada não avisa.
        let mut s = analise();
        s.conducao = Conducao::Simples;
        assert!(!aplicar(&mut s, &Evento::Parada, "s1", 1, None));
    }

    #[test]
    fn fases_do_meta_sem_interpretar_o_script() {
        assert_eq!(
            fases_do_meta(
                "export const meta = {\n  name: 'x',\n  phases: [{ title: \"A\" }, { title: 'B', detail: 'title: C' }],\n}\nphase('A')"
            ),
            ["A", "B"]
        );
        assert!(fases_do_meta("phase('A')").is_empty());
    }
}
