//! Condução da análise na avaliação (`rodar --conducao simples,workflow`, #371).
//!
//! Na condução pelo workflow, o pedido ao `claude -p` é o mesmo do app
//! ([`cba_tools::conducao::pedido_de_inicio`]); o `-p` espera o workflow terminar
//! (spike da #352, R9). Para medir os agentes, a sessão fica gravada (sem
//! `--no-session-persistence`) com um identificador conhecido: depois da execução,
//! [`medir`] lê o `journal.jsonl` da execução (a fase de cada agente) e as
//! transcrições `agent-<id>.jsonl` (o `usage` de cada mensagem), e [`apagar_sessao`]
//! apaga a sessão gravada no `~/.claude/projects`. Só identificadores, contagens e
//! tokens vão para o relatório.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::hash::{BuildHasher, Hasher};
use std::path::{Path, PathBuf};

use cba_core::analise::{Analise, EstadoAfirmacao};
use cba_core::conducao::Conducao;
use serde::Serialize;

use crate::relatorio::Tokens;

/// Como a análise foi conduzida, e o que o workflow fez.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct MedicaoConducao {
    /// `simples` ou `workflow`: a pedida.
    pub pedida: String,
    /// A gravada na análise no fim (`simples` quando a sessão voltou à condução
    /// simples).
    pub efetiva: String,
    /// "Conduzida sem workflow: <motivo>", quando houve.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aviso: Option<String>,
    /// Afirmações (impacto) ou achados (revisar) contestados pela verificação
    /// independente.
    pub contestadas: usize,
    /// Das contestadas, as que ficaram a revalidar.
    pub a_revalidar: usize,
    /// Situação final do workflow na análise (pelos hooks).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub situacao: Option<String>,
    /// Medição dos agentes do workflow, pelas transcrições.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workflow: Option<MedicaoWorkflow>,
}

/// Agentes e tokens do workflow, por fase.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct MedicaoWorkflow {
    pub agentes: usize,
    pub fases: Vec<FaseMedida>,
    /// Soma dos tokens dos agentes.
    pub tokens: Tokens,
}

/// Tokens de uma fase. `por_agente` é o total de cada agente (entrada, cache e
/// saída), para a faixa da estimativa.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct FaseMedida {
    pub fase: String,
    pub agentes: usize,
    pub tokens: Tokens,
    pub por_agente: Vec<u64>,
}

impl Tokens {
    /// Todos os tokens: entrada, cache e saída.
    pub fn total(&self) -> u64 {
        self.entrada_total() + self.saida
    }
}

/// Mede a condução a partir do estado final da análise e, no workflow, da pasta da
/// sessão gravada.
pub fn medir(pedida: Conducao, analise: &Analise, sessao: Option<&Path>) -> MedicaoConducao {
    let nome = |c: Conducao| match c {
        Conducao::Simples => "simples",
        Conducao::Workflow => "workflow",
    };
    // Afirmações (impacto, `contestar_afirmacao`) e achados do parecer (revisar,
    // `contestar_achado`): (contestados, a revalidar).
    let afirmacoes = analise
        .afirmacoes
        .iter()
        .filter(|a| a.contestacao.is_some())
        .map(|a| a.estado == EstadoAfirmacao::ARevalidar);
    let achados = analise
        .parecer
        .iter()
        .flat_map(|p| p.achados.iter())
        .filter(|a| a.contestacao.is_some())
        .map(|a| a.a_revalidar());
    let contestados: Vec<bool> = afirmacoes.chain(achados).collect();
    MedicaoConducao {
        pedida: nome(pedida).into(),
        efetiva: nome(analise.conducao).into(),
        aviso: analise.aviso_conducao.clone(),
        contestadas: contestados.len(),
        a_revalidar: contestados.iter().filter(|r| **r).count(),
        situacao: analise.workflow.as_ref().map(|w| {
            serde_json::to_value(w.situacao)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default()
        }),
        workflow: sessao.and_then(medir_workflow),
    }
}

/// Lê as execuções de workflow da sessão (`<sessao>/subagents/workflows/*/`).
pub fn medir_workflow(sessao: &Path) -> Option<MedicaoWorkflow> {
    let pasta = sessao.join("subagents").join("workflows");
    let mut fases: BTreeMap<usize, FaseMedida> = BTreeMap::new();
    let mut ordem: Vec<String> = Vec::new();
    let mut total = Tokens::default();
    let mut agentes = 0;
    for execucao in fs::read_dir(&pasta).ok()?.flatten() {
        let dir = execucao.path();
        let Ok(journal) = fs::read_to_string(dir.join("journal.jsonl")) else {
            continue;
        };
        for (agente, fase) in agentes_do_journal(&journal) {
            let Some(tokens) = tokens_do_agente(&dir.join(format!("agent-{agente}.jsonl"))) else {
                continue;
            };
            let i = match ordem.iter().position(|f| *f == fase) {
                Some(i) => i,
                None => {
                    ordem.push(fase.clone());
                    ordem.len() - 1
                }
            };
            let f = fases.entry(i).or_insert_with(|| FaseMedida {
                fase: fase.clone(),
                ..Default::default()
            });
            f.agentes += 1;
            f.tokens = f.tokens.somar(tokens);
            f.por_agente.push(tokens.total());
            total = total.somar(tokens);
            agentes += 1;
        }
    }
    (agentes > 0).then(|| MedicaoWorkflow {
        agentes,
        fases: fases.into_values().collect(),
        tokens: total,
    })
}

/// Agente e fase de cada linha `started` do journal, na ordem.
pub fn agentes_do_journal(texto: &str) -> Vec<(String, String)> {
    let mut vistos = Vec::new();
    for linha in texto.lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(linha) else {
            continue;
        };
        if v.get("type").and_then(|t| t.as_str()) != Some("started") {
            continue;
        }
        if let Some(agente) = v.get("agentId").and_then(|a| a.as_str()) {
            let fase = v
                .get("phase")
                .and_then(|p| p.as_str())
                .unwrap_or("(sem fase)");
            if !vistos.iter().any(|(a, _): &(String, String)| a == agente) {
                vistos.push((agente.to_owned(), fase.to_owned()));
            }
        }
    }
    vistos
}

/// Soma do `usage` das mensagens do assistente de uma transcrição, uma vez por
/// mensagem (a transcrição repete a mensagem a cada bloco).
pub fn tokens_do_agente(arquivo: &Path) -> Option<Tokens> {
    let texto = fs::read_to_string(arquivo).ok()?;
    Some(tokens_das_linhas(&texto))
}

/// [`tokens_do_agente`] a partir do texto.
pub fn tokens_das_linhas(texto: &str) -> Tokens {
    let mut por_mensagem: HashMap<String, Tokens> = HashMap::new();
    let mut sem_id = Tokens::default();
    for linha in texto.lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(linha) else {
            continue;
        };
        let Some(m) = v.get("message") else { continue };
        let Some(u) = m.get("usage").filter(|u| u.is_object()) else {
            continue;
        };
        let n = |campo: &str| u.get(campo).and_then(|x| x.as_u64()).unwrap_or(0);
        let t = Tokens {
            entrada: n("input_tokens"),
            saida: n("output_tokens"),
            cache_criacao: n("cache_creation_input_tokens"),
            cache_leitura: n("cache_read_input_tokens"),
        };
        match m.get("id").and_then(|i| i.as_str()) {
            Some(id) => {
                por_mensagem.insert(id.to_owned(), t);
            }
            None => sem_id = sem_id.somar(t),
        }
    }
    por_mensagem
        .into_values()
        .fold(sem_id, |acc, t| acc.somar(t))
}

/// Identificador novo para `--session-id` (formato UUID v4).
pub fn novo_id_de_sessao() -> String {
    let mut bytes = [0u8; 16];
    for (i, parte) in bytes.chunks_mut(8).enumerate() {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u128(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos()),
        );
        h.write_u32(std::process::id());
        h.write_usize(i);
        parte.copy_from_slice(&h.finish().to_le_bytes());
    }
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    )
}

/// `~/.claude` (ou `CLAUDE_CONFIG_DIR`).
fn pasta_do_claude() -> Option<PathBuf> {
    std::env::var_os("CLAUDE_CONFIG_DIR")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
                .map(|h| PathBuf::from(h).join(".claude"))
        })
}

/// Pasta do projeto, em `~/.claude/projects`, que guarda a sessão `id` de uma
/// execução da avaliação (o nome tem o da pasta temporária, `cba-avaliacao-`).
pub fn pasta_da_sessao(id: &str) -> Option<PathBuf> {
    let projetos = pasta_do_claude()?.join("projects");
    fs::read_dir(projetos)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().contains("cba-avaliacao-"))
        })
        .find(|p| p.join(id).is_dir() || p.join(format!("{id}.jsonl")).is_file())
}

/// Apaga a sessão gravada pela execução: a pasta do projeto temporário em
/// `~/.claude/projects` e a das tarefas em segundo plano, na pasta temporária do
/// sistema (`claude-<uid>/<projeto>`). Só apaga pastas de projeto da avaliação.
pub fn apagar_sessao(projeto: &Path) {
    let Some(nome) = projeto
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
    else {
        return;
    };
    if !nome.contains("cba-avaliacao-") {
        return;
    }
    let _ = fs::remove_dir_all(projeto);
    if let Ok(temporarias) = fs::read_dir(std::env::temp_dir()) {
        for t in temporarias.flatten() {
            let p = t.path().join(&nome);
            if t.file_name().to_string_lossy().starts_with("claude-") && p.is_dir() {
                let _ = fs::remove_dir_all(p);
            }
        }
    }
}

/// Copia o `journal.jsonl` de cada execução para a pasta de saída
/// (`journal-<runId>.jsonl`): só identificadores, rótulos das frentes e o que cada
/// agente devolveu.
pub fn copiar_journals(sessao: &Path, saida: &Path) {
    let Ok(execucoes) = fs::read_dir(sessao.join("subagents").join("workflows")) else {
        return;
    };
    for e in execucoes.flatten() {
        let journal = e.path().join("journal.jsonl");
        if journal.is_file() {
            let _ = fs::copy(
                &journal,
                saida.join(format!("journal-{}.jsonl", e.file_name().to_string_lossy())),
            );
        }
    }
}

/// Total de tokens da execução: o `modelUsage` do `claude -p`, que soma a sessão e
/// os subagentes do workflow. Sem ele, o `usage` da sessão e os agentes medidos.
pub fn tokens_da_execucao(e: &crate::relatorio::Execucao) -> Tokens {
    if let Some(t) = e.claude.as_ref().and_then(|c| c.tokens_dos_modelos) {
        return t;
    }
    let sessao = e.claude.as_ref().and_then(|c| c.tokens).unwrap_or_default();
    let agentes = e
        .conducao
        .as_ref()
        .and_then(|c| c.workflow.as_ref())
        .map(|w| w.tokens)
        .unwrap_or_default();
    sessao.somar(agentes)
}

fn mil(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1} mi", n as f64 / 1_000_000.0)
    } else {
        format!("{} mil", (n as f64 / 1000.0).round() as u64)
    }
}

/// Seção "Condução simples e workflow" do relatório: uma linha por execução com
/// condução, e os tokens por fase de cada execução pelo workflow.
pub fn secao_md(s: &mut String, execucoes: &[crate::relatorio::Execucao]) {
    use std::fmt::Write as _;
    let com: Vec<_> = execucoes.iter().filter(|e| e.conducao.is_some()).collect();
    if com.is_empty() {
        return;
    }
    let _ = writeln!(s, "## Condução simples e workflow\n");
    // Colunas comuns do fim da linha: contestados, agentes, tokens, custo e duração.
    let fim = |e: &crate::relatorio::Execucao, c: &MedicaoConducao| {
        format!(
            "{} | {} | {} | {} | {}",
            c.contestadas,
            c.workflow
                .as_ref()
                .map_or("-".into(), |w| w.agentes.to_string()),
            mil(tokens_da_execucao(e).total()),
            e.claude
                .as_ref()
                .and_then(|c| c.custo_usd)
                .map_or("?".into(), |v| format!("{v:.2}")),
            e.claude
                .as_ref()
                .and_then(|c| c.duracao_ms)
                .map_or("?".into(), |d| crate::relatorio::duracao_md(
                    d as f64 / 1000.0
                )),
        )
    };
    let (revisoes, outras): (Vec<&&crate::relatorio::Execucao>, Vec<_>) = com
        .iter()
        .partition(|e| e.avaliacao.as_ref().is_some_and(|a| a.parecer.is_some()));
    if !revisoes.is_empty() {
        let _ = writeln!(
            s,
            "| Item | Condução (pedida → efetiva) | Achados esperados | Termos vagos apontados | Veredito (esperado) | Perguntas esperadas | Achados | Contestados | Agentes | Tokens (todos os modelos) | Custo (US$) | Duração (relógio) |"
        );
        let _ = writeln!(s, "|---|---|---|---|---|---|---|---|---|---|---|---|");
        for e in &revisoes {
            let c = e.conducao.as_ref().expect("filtrado acima");
            let a = e.avaliacao.as_ref().expect("filtrado acima");
            let p = a.parecer.as_ref().expect("filtrado acima");
            let nome = |v: cba_core::parecer::Veredito| v.rotulo().to_lowercase();
            let _ = writeln!(
                s,
                "| {} | {} → {} | {} de {} | {} de {} | {} ({}) | {} de {} | {} | {} |",
                e.item,
                c.pedida,
                c.efetiva,
                p.achados
                    .iter()
                    .filter(|x| x.atendido_por.is_some())
                    .count(),
                p.achados.len(),
                p.termos_vagos
                    .iter()
                    .filter(|t| t.apontado_por.is_some())
                    .count(),
                p.termos_vagos.len(),
                p.veredito_registrado.map_or("nenhum".into(), nome),
                nome(p.veredito_esperado),
                a.perguntas_esperadas
                    .iter()
                    .filter(|q| q.atendida_por.is_some())
                    .count(),
                a.perguntas_esperadas.len(),
                p.achados_registrados,
                fim(e, c),
            );
        }
        let _ = writeln!(s);
    }
    if !outras.is_empty() {
        let _ = writeln!(
            s,
            "| Item | Condução (pedida → efetiva) | Regras | Perguntas esperadas | Premissa corrigida | Abrangência | Afirmações | Contestadas | Agentes | Tokens (todos os modelos) | Custo (US$) | Duração (relógio) |"
        );
        let _ = writeln!(s, "|---|---|---|---|---|---|---|---|---|---|---|---|");
    }
    for e in &outras {
        let c = e.conducao.as_ref().expect("filtrado acima");
        let a = e.avaliacao.as_ref();
        let _ = writeln!(
            s,
            "| {} | {} → {} | {} | {} | {} | {} | {} | {} |",
            e.item,
            c.pedida,
            c.efetiva,
            a.map_or("-".into(), |a| format!(
                "{} de {}",
                a.regras_encontradas, a.regras_esperadas
            )),
            a.map_or("-".into(), |a| format!(
                "{} de {}",
                a.perguntas_esperadas
                    .iter()
                    .filter(|p| p.atendida_por.is_some())
                    .count(),
                a.perguntas_esperadas.len()
            )),
            a.and_then(|a| a.premissa.as_ref())
                .map_or("n/a".into(), |p| crate::relatorio::sim_nao(p.corrigida)
                    .to_owned()),
            a.and_then(|a| a.abrangencia.as_ref())
                .map_or("n/a".into(), |b| format!(
                    "{} ({})",
                    crate::relatorio::sim_nao(b.confere),
                    b.registrada.as_deref().unwrap_or("nenhuma")
                )),
            a.map_or("-".into(), |a| a.afirmacoes.to_string()),
            fim(e, c),
        );
    }
    if !outras.is_empty() {
        let _ = writeln!(s);
    }
    for e in &com {
        let c = e.conducao.as_ref().expect("filtrado acima");
        if let Some(aviso) = &c.aviso {
            let _ = writeln!(s, "- {}: conduzida sem workflow: {aviso}", e.item);
        }
        let Some(w) = &c.workflow else { continue };
        let _ = writeln!(
            s,
            "\n### Tokens por fase: {} (situação do workflow: {})\n",
            e.item,
            c.situacao.as_deref().unwrap_or("?")
        );
        let _ = writeln!(
            s,
            "| Fase | Agentes | Tokens | Por agente (mín–máx) | Saída |"
        );
        let _ = writeln!(s, "|---|---|---|---|---|");
        for f in &w.fases {
            let min = f.por_agente.iter().min().copied().unwrap_or(0);
            let max = f.por_agente.iter().max().copied().unwrap_or(0);
            let _ = writeln!(
                s,
                "| {} | {} | {} | {}–{} | {} |",
                f.fase,
                f.agentes,
                mil(f.tokens.total()),
                mil(min),
                mil(max),
                mil(f.tokens.saida)
            );
        }
    }
    let _ = writeln!(s);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agentes_e_fases_do_journal() {
        let j = r#"{"type":"launched"}
{"type":"started","key":"k1","agentId":"a1","label":"Preparar","phase":"Preparar"}
{"type":"result","key":"k1","agentId":"a1","result":"x"}
{"type":"started","key":"k2","agentId":"a2","label":"Investigar: x","phase":"Investigar"}
"#;
        assert_eq!(
            agentes_do_journal(j),
            [
                ("a1".to_owned(), "Preparar".to_owned()),
                ("a2".to_owned(), "Investigar".to_owned())
            ]
        );
    }

    #[test]
    fn tokens_uma_vez_por_mensagem() {
        let t = r#"{"type":"assistant","message":{"id":"m1","usage":{"input_tokens":3,"output_tokens":10,"cache_creation_input_tokens":100,"cache_read_input_tokens":1000}}}
{"type":"assistant","message":{"id":"m1","usage":{"input_tokens":3,"output_tokens":12,"cache_creation_input_tokens":100,"cache_read_input_tokens":1000}}}
{"type":"user","message":{"content":"x"}}
{"type":"assistant","message":{"id":"m2","usage":{"input_tokens":1,"output_tokens":5,"cache_creation_input_tokens":0,"cache_read_input_tokens":1100}}}
"#;
        let tokens = tokens_das_linhas(t);
        assert_eq!(tokens.entrada, 4);
        assert_eq!(tokens.saida, 17);
        assert_eq!(tokens.cache_leitura, 2100);
        assert_eq!(tokens.total(), 4 + 17 + 100 + 2100);
    }

    #[test]
    fn id_de_sessao_no_formato_uuid() {
        let id = novo_id_de_sessao();
        assert_eq!(id.len(), 36);
        assert_eq!(&id[14..15], "4");
        assert_ne!(id, novo_id_de_sessao());
    }

    #[test]
    fn so_apaga_pasta_da_avaliacao() {
        let t = tempfile::tempdir().unwrap();
        let outra = t.path().join("-home-x-projeto");
        fs::create_dir_all(&outra).unwrap();
        apagar_sessao(&outra);
        assert!(outra.is_dir());
        let minha = t.path().join("-tmp-cba-avaliacao-abc-faturamento");
        fs::create_dir_all(minha.join("s")).unwrap();
        apagar_sessao(&minha);
        assert!(!minha.exists());
    }
}
