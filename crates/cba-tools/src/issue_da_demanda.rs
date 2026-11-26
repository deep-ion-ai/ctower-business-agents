//! `ler_issue_da_demanda` (#351): a issue da demanda da análise, lida no
//! rastreador do projeto (Jira, GitHub ou GitLab), com situação, responsáveis,
//! rótulos, comentários, anexos e issues ligadas.
//!
//! A ferramenta só lê a issue ligada à análise em curso (`demanda.chave` no
//! frontmatter do documento): o agente não escolhe outra issue. O token sai do
//! resolvedor e vai só no ambiente da CLI ([`cba_repos::demanda::ler_issue`]); o
//! agente recebe o conteúdo da issue, nunca a credencial. Os comentários vêm do
//! mais antigo ao mais recente, cortados pelos limites com aviso. Sem rede ou sem
//! token, a recusa diz o motivo e pede para seguir com a entrada que já existe.

use std::fmt::Write as _;

use cba_core::ambiente::Ambiente;
use cba_core::analise::Analise;
use cba_kb::frontmatter::FrontmatterAnalise;
use cba_repos::demanda::{Ferramentas, IssueLida, LimitesIssue, ler_issue};
use cba_repos::estado::{chave_normalizada, origem_do_rastreador};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::comum;
use crate::registro::{ErroFerramenta, Ferramenta, Saida, ler_argumentos};

/// Maior número de comentários que o agente pode pedir.
const MAXIMO_DE_COMENTARIOS: usize = 100;

#[derive(Debug, Deserialize)]
struct Args {
    analise: String,
    #[serde(default)]
    comentarios: Option<usize>,
}

fn schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "analise": comum::schema_da_analise(),
            "comentarios": {
                "type": "integer",
                "minimum": 1,
                "maximum": MAXIMO_DE_COMENTARIOS,
                "description": "Quantos comentários mais recentes trazer (padrão 30)."
            }
        },
        "required": ["analise"],
        "additionalProperties": false
    })
}

/// Chave da issue da análise no rastreador do projeto, quando a análise nasceu
/// de uma issue (ou de uma demanda com chave) deste rastreador.
pub(crate) fn chave_no_rastreador(ambiente: &Ambiente, analise: &Analise) -> Option<String> {
    let tracker = ambiente.projeto.repositorios.tracker.as_ref()?;
    let doc = ambiente
        .kb
        .ler::<FrontmatterAnalise>(&analise.documento)
        .ok()?;
    let demanda = doc.frontmatter.demanda;
    let chave = chave_normalizada(tracker, demanda.chave.as_deref()?)?;
    origem_do_rastreador(tracker, Some(&demanda.origem)).then_some(chave)
}

/// `ler_issue_da_demanda` com as CLIs e o chaveiro padrão.
pub fn ler_issue_da_demanda() -> Ferramenta {
    ler_issue_da_demanda_com(Ferramentas::default())
}

/// `ler_issue_da_demanda` com as CLIs e o chaveiro informados (os testes usam
/// CLIs falsas).
pub fn ler_issue_da_demanda_com(f: Ferramentas) -> Ferramenta {
    Ferramenta::nova(
        "ler_issue_da_demanda",
        "Ler a issue da demanda no rastreador",
        "Lê no rastreador do projeto (Jira, GitHub ou GitLab) a issue de onde a análise \
         nasceu: título, descrição, situação, responsáveis, rótulos, comentários (autor, \
         data e texto em Markdown, do mais antigo ao mais recente), anexos (nome e tipo, \
         sem baixar) e issues ligadas. Só lê a issue ligada à análise informada. Os \
         comentários trazem decisões e mudanças de escopo que não estão na descrição: \
         leia-os no início e cite o comentário (autor e data) quando ele mudar o \
         entendimento do pedido. Não chame `gh`, `glab` ou `jira` direto: a credencial \
         fica no app.",
        schema(),
        move |ambiente, args| {
            let args: Args = ler_argumentos(args)?;
            let id = comum::id_da_analise(&args.analise)?;
            let analise = ambiente.analises.carregar(&id)?;
            let tracker = ambiente
                .projeto
                .repositorios
                .tracker
                .as_ref()
                .ok_or_else(|| {
                    ErroFerramenta::Recusa(
                        "o projeto não tem rastreador de issues cadastrado; siga com a entrada \
                     da análise"
                            .into(),
                    )
                })?;
            let chave = chave_no_rastreador(ambiente, &analise).ok_or_else(|| {
                ErroFerramenta::Recusa(format!(
                    "a análise {id} não nasceu de uma issue do rastreador do projeto; a \
                     entrada da análise é tudo o que há do pedido"
                ))
            })?;
            let limites = LimitesIssue {
                comentarios: args
                    .comentarios
                    .unwrap_or(LimitesIssue::default().comentarios)
                    .clamp(1, MAXIMO_DE_COMENTARIOS),
                ..LimitesIssue::default()
            };
            let issue = ler_issue(tracker, &chave, &limites, &f).map_err(|e| {
                tracing::warn!(analise = %id, "issue da demanda não lida");
                ErroFerramenta::Recusa(format!(
                    "não foi possível ler a issue {chave} no rastreador: {e}. Siga a análise \
                     com a entrada que já tem e avise o analista de que os comentários da \
                     issue não foram lidos"
                ))
            })?;
            tracing::info!(
                analise = %id,
                comentarios = issue.comentarios.len(),
                "issue da demanda lida"
            );
            Ok(Saida {
                texto: markdown(&issue),
                dados: Some(serde_json::to_value(&issue).unwrap_or_default()),
            })
        },
    )
    .somente_leitura()
}

fn lista(itens: &[String]) -> String {
    if itens.is_empty() {
        "nenhum".into()
    } else {
        itens.join(", ")
    }
}

/// A issue em Markdown, para o agente ler.
fn markdown(i: &IssueLida) -> String {
    let mut t = String::new();
    let _ = writeln!(t, "# {} {}\n", i.chave, i.titulo);
    let _ = writeln!(t, "- Link: {}", i.url);
    let _ = writeln!(t, "- Situação: {}", i.situacao);
    let _ = writeln!(t, "- Responsáveis: {}", lista(&i.responsaveis));
    let _ = writeln!(t, "- Rótulos: {}", lista(&i.rotulos));
    let _ = writeln!(t, "\n## Descrição\n");
    let _ = writeln!(
        t,
        "{}\n",
        if i.descricao.trim().is_empty() {
            "(sem descrição)"
        } else {
            i.descricao.trim()
        }
    );
    let _ = writeln!(
        t,
        "## Comentários ({} de {})\n",
        i.comentarios.len(),
        i.total_de_comentarios
    );
    if i.comentarios.is_empty() {
        let _ = writeln!(t, "(sem comentários)\n");
    }
    for (n, c) in i.comentarios.iter().enumerate() {
        let _ = writeln!(t, "### {}. {} em {}\n", n + 1, c.autor, c.data);
        let _ = writeln!(t, "{}\n", c.texto.trim());
    }
    if !i.anexos.is_empty() {
        let _ = writeln!(t, "## Anexos\n");
        for a in &i.anexos {
            let _ = writeln!(t, "- {} ({})", a.nome, a.tipo);
        }
        let _ = writeln!(t);
    }
    if !i.ligadas.is_empty() {
        let _ = writeln!(t, "## Issues ligadas\n");
        for l in &i.ligadas {
            let _ = writeln!(t, "- {} {}: {} {}", l.relacao, l.chave, l.titulo, l.url);
        }
        let _ = writeln!(t);
    }
    for a in &i.avisos {
        let _ = writeln!(t, "Aviso: {a}");
    }
    t.trim_end().to_string()
}
