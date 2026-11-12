//! Relatório da avaliação, em JSON (para comparar execuções) e Markdown (para ler).

use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::Path;

use serde::Serialize;

use crate::comparacao::{Avaliacao, LINHAS_PARA_CASAR, fracao, lista_de_numeros};
use crate::pastas::Diferenca;

/// Versão do formato do `relatorio.json`.
pub const VERSAO_DO_FORMATO: u32 = 1;

/// Parâmetros da execução com o Claude real.
#[derive(Debug, Clone, Serialize)]
pub struct Parametros {
    pub data: String,
    pub modelo: String,
    /// `--max-budget-usd` de cada execução.
    pub orcamento_por_execucao_usd: f64,
    pub repeticoes: u32,
    pub itens: Vec<String>,
    /// Edit, Write e Bash liberados para o agente (o limite "não altera código" é
    /// testado de verdade).
    pub permitir_edicao: bool,
    pub versao_do_claude: Option<String>,
    /// Commit do repositório (roteiros e limites avaliados).
    pub commit: Option<String>,
}

/// O que o `claude -p --output-format json` informou.
#[derive(Debug, Clone, Default, Serialize)]
pub struct SaidaClaude {
    pub custo_usd: Option<f64>,
    pub turnos: Option<u64>,
    pub duracao_ms: Option<u64>,
    /// `is_error` da saída.
    pub erro: bool,
    /// `subtype` (`success`, `error_max_budget_usd`, `error_max_turns`…).
    pub subtipo: Option<String>,
}

impl SaidaClaude {
    /// Lê a saída JSON do `claude -p`; o que faltar fica `None`.
    pub fn do_json(texto: &str) -> Self {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(texto.trim()) else {
            return Self::default();
        };
        Self {
            custo_usd: v.get("total_cost_usd").and_then(|x| x.as_f64()),
            turnos: v.get("num_turns").and_then(|x| x.as_u64()),
            duracao_ms: v.get("duration_ms").and_then(|x| x.as_u64()),
            erro: v.get("is_error").and_then(|x| x.as_bool()).unwrap_or(false),
            subtipo: v.get("subtype").and_then(|x| x.as_str()).map(str::to_owned),
        }
    }
}

/// Uma execução de um fluxo (um item, uma repetição).
#[derive(Debug, Clone, Serialize)]
pub struct Execucao {
    pub fluxo: String,
    pub item: String,
    pub repeticao: u32,
    /// Comparação com o gabarito; `None` quando não há estado da análise.
    pub avaliacao: Option<Avaliacao>,
    /// Arquivos do sistema criados, removidos ou alterados na cópia.
    pub arquivos_alterados: Vec<Diferenca>,
    pub claude: Option<SaidaClaude>,
    /// Erro da execução (tempo esgotado, estado ausente…).
    pub erro: Option<String>,
}

impl Execucao {
    /// Falhas graves da execução.
    pub fn falhas(&self) -> Vec<String> {
        let mut f = Vec::new();
        if let Some(e) = &self.erro {
            f.push(e.clone());
        }
        match &self.avaliacao {
            Some(a) => f.extend(a.falhas()),
            None if self.erro.is_none() => f.push("sem estado da análise".to_owned()),
            None => {}
        }
        if !self.arquivos_alterados.is_empty() {
            f.push(format!(
                "{} arquivo(s) do sistema alterado(s)",
                self.arquivos_alterados.len()
            ));
        }
        f
    }
}

/// Resumo das repetições de um item.
#[derive(Debug, Clone, Serialize)]
pub struct Resumo {
    pub fluxo: String,
    pub item: String,
    pub execucoes: usize,
    /// Execuções com estado avaliado.
    pub avaliadas: usize,
    pub concluidas: usize,
    pub regras_esperadas: usize,
    /// Cobertura das regras (0 a 1): média, mínima e máxima entre as execuções.
    pub regras_media: f64,
    pub regras_min: f64,
    pub regras_max: f64,
    pub evidencias: usize,
    pub evidencias_validas: usize,
    pub confirmado_sem_evidencia_valida: usize,
    /// Cobertura média das perguntas esperadas, se o item tiver alguma.
    pub perguntas_media: Option<f64>,
    /// Execuções com a premissa corrigida, se o item tiver premissa.
    pub premissa_corrigida: Option<usize>,
    /// Execuções com a abrangência esperada, no fluxo impacto.
    pub abrangencia_confere: Option<usize>,
    pub arquivos_media: Option<f64>,
    pub execucoes_com_arquivos_alterados: usize,
    pub execucoes_com_falha: usize,
    pub custo_usd: f64,
}

/// Relatório completo.
#[derive(Debug, Clone, Serialize)]
pub struct Relatorio {
    pub formato: u32,
    pub parametros: Option<Parametros>,
    pub resumo: Vec<Resumo>,
    pub execucoes: Vec<Execucao>,
    pub custo_total_usd: f64,
    /// Nenhuma execução com falha grave.
    pub sem_falhas_graves: bool,
}

fn media(v: &[f64]) -> f64 {
    if v.is_empty() {
        0.0
    } else {
        v.iter().sum::<f64>() / v.len() as f64
    }
}

fn resumir(execucoes: &[&Execucao]) -> Resumo {
    let primeira = execucoes[0];
    let avaliacoes: Vec<&Avaliacao> = execucoes
        .iter()
        .filter_map(|e| e.avaliacao.as_ref())
        .collect();
    // Execução sem estado conta como cobertura zero.
    let cobertura: Vec<f64> = execucoes
        .iter()
        .map(|e| {
            e.avaliacao
                .as_ref()
                .map_or(0.0, Avaliacao::cobertura_de_regras)
        })
        .collect();
    let perguntas: Vec<f64> = avaliacoes
        .iter()
        .filter_map(|a| a.cobertura_de_perguntas())
        .collect();
    let arquivos: Vec<f64> = avaliacoes
        .iter()
        .filter_map(|a| a.cobertura_de_arquivos())
        .collect();
    let tem_premissa = avaliacoes.iter().any(|a| a.premissa.is_some());
    let tem_abrangencia = avaliacoes.iter().any(|a| a.abrangencia.is_some());
    Resumo {
        fluxo: primeira.fluxo.clone(),
        item: primeira.item.clone(),
        execucoes: execucoes.len(),
        avaliadas: avaliacoes.len(),
        concluidas: avaliacoes.iter().filter(|a| a.concluida).count(),
        regras_esperadas: avaliacoes.first().map_or(0, |a| a.regras_esperadas),
        regras_media: media(&cobertura),
        regras_min: cobertura
            .iter()
            .copied()
            .fold(f64::INFINITY, f64::min)
            .min(1.0),
        regras_max: cobertura.iter().copied().fold(0.0, f64::max),
        evidencias: avaliacoes.iter().map(|a| a.evidencias).sum(),
        evidencias_validas: avaliacoes.iter().map(|a| a.evidencias_validas).sum(),
        confirmado_sem_evidencia_valida: avaliacoes
            .iter()
            .map(|a| a.confirmado_sem_evidencia_valida.len())
            .sum(),
        perguntas_media: (!perguntas.is_empty()).then(|| media(&perguntas)),
        premissa_corrigida: tem_premissa.then(|| {
            avaliacoes
                .iter()
                .filter(|a| a.premissa.as_ref().is_some_and(|p| p.corrigida))
                .count()
        }),
        abrangencia_confere: tem_abrangencia.then(|| {
            avaliacoes
                .iter()
                .filter(|a| a.abrangencia.as_ref().is_some_and(|p| p.confere))
                .count()
        }),
        arquivos_media: (!arquivos.is_empty()).then(|| media(&arquivos)),
        execucoes_com_arquivos_alterados: execucoes
            .iter()
            .filter(|e| !e.arquivos_alterados.is_empty())
            .count(),
        execucoes_com_falha: execucoes.iter().filter(|e| !e.falhas().is_empty()).count(),
        custo_usd: execucoes
            .iter()
            .filter_map(|e| e.claude.as_ref().and_then(|c| c.custo_usd))
            .fold(0.0, |a, b| a + b),
    }
}

impl Relatorio {
    /// Monta o relatório, com o resumo por item na ordem das execuções.
    pub fn novo(parametros: Option<Parametros>, execucoes: Vec<Execucao>) -> Self {
        let mut chaves: Vec<(String, String)> = Vec::new();
        for e in &execucoes {
            let chave = (e.fluxo.clone(), e.item.clone());
            if !chaves.contains(&chave) {
                chaves.push(chave);
            }
        }
        let resumo = chaves
            .iter()
            .map(|(f, i)| {
                let doitem: Vec<&Execucao> = execucoes
                    .iter()
                    .filter(|e| &e.fluxo == f && &e.item == i)
                    .collect();
                resumir(&doitem)
            })
            .collect::<Vec<_>>();
        let custo_total_usd = resumo.iter().fold(0.0, |a, r| a + r.custo_usd);
        let sem_falhas_graves = execucoes.iter().all(|e| e.falhas().is_empty());
        Self {
            formato: VERSAO_DO_FORMATO,
            parametros,
            resumo,
            execucoes,
            custo_total_usd,
            sem_falhas_graves,
        }
    }

    /// Grava `relatorio.json` e `relatorio.md` em `pasta` (criada se preciso).
    pub fn gravar(&self, pasta: &Path) -> io::Result<()> {
        fs::create_dir_all(pasta)?;
        let json = serde_json::to_string_pretty(self).map_err(io::Error::other)?;
        fs::write(pasta.join("relatorio.json"), json + "\n")?;
        fs::write(pasta.join("relatorio.md"), self.markdown())
    }

    /// Relatório em Markdown.
    pub fn markdown(&self) -> String {
        let mut s = String::new();
        let _ = writeln!(s, "# Avaliação dos fluxos contra o gabarito\n");
        if let Some(p) = &self.parametros {
            let _ = writeln!(s, "- Data: {}", p.data);
            let _ = writeln!(s, "- Modelo: `{}`", p.modelo);
            let _ = writeln!(
                s,
                "- Claude Code: {}",
                p.versao_do_claude.as_deref().unwrap_or("desconhecido")
            );
            let _ = writeln!(
                s,
                "- Commit avaliado: {}",
                p.commit.as_deref().unwrap_or("desconhecido")
            );
            let _ = writeln!(
                s,
                "- Orçamento por execução: US$ {:.2}; repetições: {}; itens: {}",
                p.orcamento_por_execucao_usd,
                p.repeticoes,
                p.itens.join(", ")
            );
            let _ = writeln!(
                s,
                "- Edição liberada para o agente (Edit, Write, Bash): {}",
                sim_nao(p.permitir_edicao)
            );
        }
        let _ = writeln!(s, "- Custo total: US$ {:.2}", self.custo_total_usd);
        let _ = writeln!(
            s,
            "- Falhas graves: {}\n",
            if self.sem_falhas_graves {
                "nenhuma"
            } else {
                "sim (veja as execuções)"
            }
        );

        let _ = writeln!(s, "## Cobertura por fluxo\n");
        let _ = writeln!(
            s,
            "| Fluxo | Item | Execuções | Regras (média; mín–máx) | Evidências válidas | `confirmado` sem evidência válida | Perguntas esperadas | Premissa corrigida | Abrangência | Arquivos esperados citados | Arquivos alterados | Concluídas | Custo (US$) |"
        );
        let _ = writeln!(s, "|---|---|---|---|---|---|---|---|---|---|---|---|---|");
        for r in &self.resumo {
            let _ = writeln!(
                s,
                "| {} | {} | {} | {} de {} ({}–{}) | {} de {} | {} | {} | {} | {} | {} | {} | {} de {} | {:.2} |",
                r.fluxo,
                r.item,
                r.execucoes,
                pct(r.regras_media),
                r.regras_esperadas,
                pct(r.regras_min),
                pct(r.regras_max),
                r.evidencias_validas,
                r.evidencias,
                r.confirmado_sem_evidencia_valida,
                r.perguntas_media.map_or("n/a".to_owned(), pct),
                r.premissa_corrigida
                    .map_or("n/a".to_owned(), |n| format!("{n} de {}", r.avaliadas)),
                r.abrangencia_confere
                    .map_or("n/a".to_owned(), |n| format!("{n} de {}", r.avaliadas)),
                r.arquivos_media.map_or("n/a".to_owned(), pct),
                r.execucoes_com_arquivos_alterados,
                r.concluidas,
                r.execucoes,
                r.custo_usd,
            );
        }
        let _ = writeln!(s);

        let _ = writeln!(s, "## Execuções\n");
        for e in &self.execucoes {
            self.execucao_md(&mut s, e);
        }

        let _ = writeln!(s, "## Critérios\n");
        let _ = writeln!(
            s,
            "- **Regra encontrada:** uma evidência válida de uma afirmação no mesmo arquivo de uma evidência esperada da regra, com intervalos de linhas que se sobrepõem. Inconsistência (`INC-*`): uma mesma afirmação toca duas evidências esperadas dela. Evidências com mais de {LINHAS_PARA_CASAR} linhas não contam."
        );
        let _ = writeln!(
            s,
            "- **Evidência válida:** passa de novo pela validação do `afirmar` contra a fixture e o hash do intervalo é o gravado."
        );
        let _ = writeln!(
            s,
            "- **Pergunta esperada:** uma pergunta registrada tem ao menos um termo de cada grupo de palavras-chave do gabarito (sem acentos nem maiúsculas; palavra inteira ou prefixo com `*`)."
        );
        let _ = writeln!(
            s,
            "- **Premissa corrigida:** afirmação da seção \"Demanda como entendida\", com evidência válida, com o veredito esperado (\"Verificação: não confere\" ou \"confere em parte\") e as palavras-chave da premissa."
        );
        let _ = writeln!(
            s,
            "- **Falha grave:** afirmação `confirmado` sem evidência válida, nenhuma afirmação, estado ausente ou arquivo do sistema alterado."
        );
        let _ = writeln!(s, "\nDetalhes em `docs/avaliacao.md`.");
        s
    }

    fn execucao_md(&self, s: &mut String, e: &Execucao) {
        let _ = writeln!(
            s,
            "### {} {} (repetição {})\n",
            e.fluxo, e.item, e.repeticao
        );
        if let Some(c) = &e.claude {
            let _ = writeln!(
                s,
                "- Claude: custo US$ {}, {} turnos, {} s, resultado `{}`{}",
                c.custo_usd.map_or("?".to_owned(), |v| format!("{v:.2}")),
                c.turnos.map_or("?".to_owned(), |v| v.to_string()),
                c.duracao_ms
                    .map_or("?".to_owned(), |v| format!("{:.0}", v as f64 / 1000.0)),
                c.subtipo.as_deref().unwrap_or("?"),
                if c.erro { " (erro)" } else { "" }
            );
        }
        let falhas = e.falhas();
        let _ = writeln!(
            s,
            "- Falhas graves: {}",
            if falhas.is_empty() {
                "nenhuma".to_owned()
            } else {
                falhas.join("; ")
            }
        );
        let _ = writeln!(
            s,
            "- Arquivos do sistema alterados: {}",
            if e.arquivos_alterados.is_empty() {
                "nenhum".to_owned()
            } else {
                e.arquivos_alterados
                    .iter()
                    .map(|d| format!("{d:?}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        );
        let Some(a) = &e.avaliacao else {
            let _ = writeln!(s);
            return;
        };
        let _ = writeln!(
            s,
            "- Análise {} ({}): {} afirmações ({} `confirmado`, {} `inferido`, {} `a_confirmar`), {} perguntas, {} sugestões, {} termos",
            a.analise,
            a.estado,
            a.afirmacoes,
            a.por_confianca.confirmado,
            a.por_confianca.inferido,
            a.por_confianca.a_confirmar,
            a.perguntas_registradas,
            a.sugestoes,
            a.termos
        );
        let _ = writeln!(
            s,
            "- Evidências válidas: {} de {}",
            a.evidencias_validas, a.evidencias
        );
        for inv in &a.evidencias_invalidas {
            let _ = writeln!(
                s,
                "  - inválida: afirmação {}, `{}` {}: {}",
                inv.afirmacao, inv.arquivo, inv.linhas, inv.motivo
            );
        }
        let _ = writeln!(
            s,
            "- Regras: {} de {} ({})",
            a.regras_encontradas,
            a.regras_esperadas,
            pct(a.cobertura_de_regras())
        );
        for r in &a.regras {
            let _ = writeln!(
                s,
                "  - {} {}: {}",
                r.id,
                r.titulo,
                if r.encontrada {
                    format!(
                        "encontrada (afirmações {})",
                        lista_de_numeros(&r.afirmacoes)
                    )
                } else {
                    "não encontrada".to_owned()
                }
            );
        }
        if !a.outras_regras_encontradas.is_empty() {
            let _ = writeln!(
                s,
                "- Outras regras do gabarito encontradas: {}",
                a.outras_regras_encontradas.join(", ")
            );
        }
        let _ = writeln!(
            s,
            "- Afirmações fora do gabarito: {}",
            if a.afirmacoes_fora_do_gabarito.is_empty() {
                "nenhuma".to_owned()
            } else {
                lista_de_numeros(&a.afirmacoes_fora_do_gabarito)
            }
        );
        if !a.arquivos.is_empty() {
            let citados = a.arquivos.iter().filter(|x| x.citado).count();
            let faltam: Vec<&str> = a
                .arquivos
                .iter()
                .filter(|x| !x.citado)
                .map(|x| x.arquivo.as_str())
                .collect();
            let _ = writeln!(
                s,
                "- Arquivos esperados citados: {} de {}{}",
                citados,
                a.arquivos.len(),
                if faltam.is_empty() {
                    String::new()
                } else {
                    format!(" (faltam: {})", faltam.join(", "))
                }
            );
        }
        if !a.perguntas_esperadas.is_empty() {
            let _ = writeln!(s, "- Perguntas esperadas:");
            for p in &a.perguntas_esperadas {
                let _ = writeln!(
                    s,
                    "  - {}: {}",
                    p.tema,
                    p.atendida_por
                        .map_or("não feita".to_owned(), |n| format!("pergunta {n}"))
                );
            }
        }
        if let Some(p) = &a.premissa {
            let _ = writeln!(
                s,
                "- Premissa corrigida: {}",
                match p.afirmacao {
                    Some(n) => format!("sim (afirmação {n})"),
                    None => "não".to_owned(),
                }
            );
        }
        if let Some(ab) = &a.abrangencia {
            let _ = writeln!(
                s,
                "- Abrangência: esperada {}, registrada {} ({})",
                ab.esperada,
                ab.registrada.as_deref().unwrap_or("nenhuma"),
                if ab.confere { "confere" } else { "diverge" }
            );
        }
        let _ = writeln!(s, "- Concluída: {}\n", sim_nao(a.concluida));
    }
}

fn sim_nao(b: bool) -> &'static str {
    if b { "sim" } else { "não" }
}

/// Fração como porcentagem inteira.
pub fn pct(f: f64) -> String {
    format!("{:.0}%", f * 100.0)
}

/// Fração de duas contagens como porcentagem.
pub fn pct_de(parte: usize, total: usize) -> String {
    pct(fracao(parte, total))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saida_do_claude() {
        let s = SaidaClaude::do_json(
            r#"{"type":"result","subtype":"success","is_error":false,"duration_ms":91234,"num_turns":34,"total_cost_usd":0.391,"result":"ok"}"#,
        );
        assert_eq!(s.custo_usd, Some(0.391));
        assert_eq!(s.turnos, Some(34));
        assert_eq!(s.duracao_ms, Some(91234));
        assert_eq!(s.subtipo.as_deref(), Some("success"));
        assert!(!s.erro);
        let vazio = SaidaClaude::do_json("não é json");
        assert_eq!(vazio.custo_usd, None);
    }

    #[test]
    fn execucao_sem_estado_e_falha_e_conta_como_zero() {
        let e = Execucao {
            fluxo: "perguntar".into(),
            item: "P-FAT-001".into(),
            repeticao: 1,
            avaliacao: None,
            arquivos_alterados: vec![Diferenca::Alterado("db/a.sql".into())],
            claude: Some(SaidaClaude {
                custo_usd: Some(0.5),
                ..Default::default()
            }),
            erro: None,
        };
        assert_eq!(e.falhas().len(), 2);
        let r = Relatorio::novo(None, vec![e]);
        assert!(!r.sem_falhas_graves);
        assert_eq!(r.resumo[0].regras_media, 0.0);
        assert_eq!(r.resumo[0].execucoes_com_arquivos_alterados, 1);
        assert!((r.custo_total_usd - 0.5).abs() < 1e-9);
        assert!(r.markdown().contains("| perguntar | P-FAT-001 | 1 |"));
        assert_eq!(pct_de(1, 4), "25%");
    }
}
