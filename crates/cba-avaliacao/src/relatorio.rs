//! Relatório da avaliação, em JSON (para comparar execuções) e Markdown (para ler).

use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::Path;

use serde::Serialize;

use crate::ata::AvaliacaoAta;
use crate::comparacao::{Avaliacao, LINHAS_PARA_CASAR, fracao, lista_de_numeros};
use crate::longa::{OpcoesLonga, TamanhoTranscript};
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
    /// `--max-budget-usd` da reunião longa, quando ela roda.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orcamento_reuniao_longa_usd: Option<f64>,
    /// Tamanho pedido da reunião longa, quando ela roda.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reuniao_longa: Option<OpcoesLonga>,
}

/// Tokens de uma execução (`usage` da saída do `claude`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Tokens {
    pub entrada: u64,
    pub saida: u64,
    /// Gravados no cache de prompt.
    pub cache_criacao: u64,
    /// Lidos do cache de prompt.
    pub cache_leitura: u64,
}

impl Tokens {
    /// Todos os tokens de entrada, com os do cache.
    pub fn entrada_total(&self) -> u64 {
        self.entrada + self.cache_criacao + self.cache_leitura
    }

    fn somar(self, outro: Self) -> Self {
        Self {
            entrada: self.entrada + outro.entrada,
            saida: self.saida + outro.saida,
            cache_criacao: self.cache_criacao + outro.cache_criacao,
            cache_leitura: self.cache_leitura + outro.cache_leitura,
        }
    }
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
    /// `usage` da saída.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens: Option<Tokens>,
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
            tokens: v.get("usage").filter(|u| u.is_object()).map(|u| {
                let n = |campo: &str| u.get(campo).and_then(|x| x.as_u64()).unwrap_or(0);
                Tokens {
                    entrada: n("input_tokens"),
                    saida: n("output_tokens"),
                    cache_criacao: n("cache_creation_input_tokens"),
                    cache_leitura: n("cache_read_input_tokens"),
                }
            }),
        }
    }
}

/// Uma execução de um fluxo (um item, uma repetição).
#[derive(Debug, Clone, Serialize)]
pub struct Execucao {
    pub fluxo: String,
    pub item: String,
    pub repeticao: u32,
    /// Comparação com o gabarito; `None` quando não há estado da análise (e no
    /// fluxo `reuniao`, que compara a ata).
    pub avaliacao: Option<Avaliacao>,
    /// Fluxo `reuniao`: comparação da ata com o gabarito.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ata: Option<AvaliacaoAta>,
    /// Fluxo `reuniao`: tamanho do transcript analisado.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transcript: Option<TamanhoTranscript>,
    /// Arquivos do projeto criados, removidos ou alterados na cópia.
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
        match (&self.avaliacao, &self.ata) {
            (Some(a), _) => f.extend(a.falhas()),
            (None, Some(a)) => f.extend(a.falhas()),
            (None, None) if self.erro.is_none() => f.push(
                if self.e_reuniao() {
                    "sem estado da ata"
                } else {
                    "sem estado da análise"
                }
                .to_owned(),
            ),
            (None, None) => {}
        }
        if !self.arquivos_alterados.is_empty() {
            f.push(format!(
                "{} arquivo(s) do projeto alterado(s)",
                self.arquivos_alterados.len()
            ));
        }
        f
    }

    /// Execução do fluxo `reuniao` (normal ou longa).
    pub fn e_reuniao(&self) -> bool {
        self.fluxo.starts_with("reuniao")
    }
}

/// Resumo das atas de um item do fluxo `reuniao`.
#[derive(Debug, Clone, Serialize)]
pub struct ResumoAta {
    pub papeis_esperados: usize,
    /// Fração média dos papéis acertados entre as execuções.
    pub papeis_media: f64,
    pub itens_esperados: usize,
    /// Fração dos itens esperados atendidos: média, mínima e máxima.
    pub itens_media: f64,
    pub itens_min: f64,
    pub itens_max: f64,
    /// Itens atendidos e esperados por seção (soma das execuções), na ordem das
    /// seções.
    pub por_secao: Vec<SecaoResumida>,
    pub citacoes: usize,
    /// Conferidas de novo contra o transcript.
    pub citacoes_reconferidas: usize,
    pub citacoes_recusadas: usize,
    /// Execuções com alguma seção vazia em que o gabarito espera itens.
    pub execucoes_com_secao_vazia: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct SecaoResumida {
    pub secao: String,
    pub atendidos: usize,
    pub esperados: usize,
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
    /// Soma da duração das execuções (`duration_ms` do `claude`), em segundos.
    pub duracao_s: f64,
    pub tokens: Tokens,
    /// Fluxo `reuniao`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ata: Option<ResumoAta>,
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
        // No fluxo `reuniao`, a análise da reunião concluída (`concluir_analise`).
        concluidas: avaliacoes.iter().filter(|a| a.concluida).count()
            + execucoes
                .iter()
                .filter(|e| e.ata.as_ref().is_some_and(|a| a.concluida == Some(true)))
                .count(),
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
        duracao_s: execucoes
            .iter()
            .filter_map(|e| e.claude.as_ref().and_then(|c| c.duracao_ms))
            .map(|ms| ms as f64 / 1000.0)
            .sum(),
        tokens: execucoes
            .iter()
            .filter_map(|e| e.claude.as_ref().and_then(|c| c.tokens))
            .fold(Tokens::default(), Tokens::somar),
        ata: primeira.e_reuniao().then(|| resumir_atas(execucoes)),
    }
}

fn resumir_atas(execucoes: &[&Execucao]) -> ResumoAta {
    let atas: Vec<&AvaliacaoAta> = execucoes.iter().filter_map(|e| e.ata.as_ref()).collect();
    // Execução sem ata conta como zero.
    let papeis: Vec<f64> = execucoes
        .iter()
        .map(|e| {
            e.ata
                .as_ref()
                .map_or(0.0, AvaliacaoAta::cobertura_de_papeis)
        })
        .collect();
    let itens: Vec<f64> = execucoes
        .iter()
        .map(|e| e.ata.as_ref().map_or(0.0, AvaliacaoAta::cobertura_de_itens))
        .collect();
    let por_secao = cba_docs::TipoItem::TODOS
        .into_iter()
        .map(|t| {
            let (atendidos, esperados) = atas
                .iter()
                .flat_map(|a| a.por_secao.iter().filter(move |s| s.secao == t))
                .fold((0, 0), |(a, e), s| (a + s.atendidos, e + s.esperados));
            SecaoResumida {
                secao: t.secao().to_owned(),
                atendidos,
                esperados,
            }
        })
        .collect();
    ResumoAta {
        papeis_esperados: atas.first().map_or(0, |a| a.papeis_esperados),
        papeis_media: media(&papeis),
        itens_esperados: atas.first().map_or(0, |a| a.itens_esperados),
        itens_media: media(&itens),
        itens_min: itens.iter().copied().fold(f64::INFINITY, f64::min).min(1.0),
        itens_max: itens.iter().copied().fold(0.0, f64::max),
        por_secao,
        citacoes: atas.iter().map(|a| a.citacoes.total).sum(),
        citacoes_reconferidas: atas.iter().filter_map(|a| a.citacoes.reconferidas).sum(),
        citacoes_recusadas: atas.iter().map(|a| a.citacoes.recusadas.len()).sum(),
        execucoes_com_secao_vazia: atas
            .iter()
            .filter(|a| !a.secoes_vazias_com_itens_esperados.is_empty())
            .count(),
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
            if let (Some(o), Some(l)) = (p.orcamento_reuniao_longa_usd, p.reuniao_longa) {
                let _ = writeln!(
                    s,
                    "- Reunião longa sintética: cerca de {} min e {} palavras; orçamento de US$ {o:.2}",
                    l.minutos, l.palavras
                );
            }
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

        let (atas, analises): (Vec<&Resumo>, Vec<&Resumo>) =
            self.resumo.iter().partition(|r| r.ata.is_some());
        if !analises.is_empty() {
            tabela_de_analises(&mut s, &analises);
        }
        if !atas.is_empty() {
            tabela_de_atas(&mut s, &atas);
        }

        let _ = writeln!(s, "## Execuções\n");
        for e in &self.execucoes {
            execucao_md(&mut s, e);
        }

        let _ = writeln!(s, "## Critérios\n");
        if !analises.is_empty() {
            criterios_das_analises(&mut s);
        }
        if !atas.is_empty() {
            criterios_das_atas(&mut s);
        }
        let _ = writeln!(s, "\nDetalhes em `docs/avaliacao.md`.");
        s
    }
}

fn tabela_de_analises(s: &mut String, resumo: &[&Resumo]) {
    let _ = writeln!(s, "## Cobertura por fluxo\n");
    let _ = writeln!(
        s,
        "| Fluxo | Item | Execuções | Regras (média; mín–máx) | Evidências válidas | `confirmado` sem evidência válida | Perguntas esperadas | Premissa corrigida | Abrangência | Arquivos esperados citados | Arquivos alterados | Concluídas | Custo (US$) |"
    );
    let _ = writeln!(s, "|---|---|---|---|---|---|---|---|---|---|---|---|---|");
    for r in resumo {
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
}

fn execucao_md(s: &mut String, e: &Execucao) {
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
        if let Some(t) = c.tokens {
            let _ = writeln!(s, "- Tokens: {}", tokens_md(&t));
        }
    }
    if let Some(t) = &e.transcript {
        let _ = writeln!(
            s,
            "- Transcript: {} falas, {} palavras, {}",
            t.falas,
            t.palavras,
            duracao_md(t.duracao_s as f64)
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
        "- Arquivos do projeto alterados: {}",
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
    if let Some(a) = &e.ata {
        ata_md(s, a);
        return;
    }
    analise_md(s, e);
}

fn criterios_das_analises(s: &mut String) {
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
        "- **Falha grave:** afirmação `confirmado` sem evidência válida, nenhuma afirmação, estado ausente ou arquivo do projeto alterado."
    );
}

fn analise_md(s: &mut String, e: &Execucao) {
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

fn tabela_de_atas(s: &mut String, resumo: &[&Resumo]) {
    let _ = writeln!(s, "## Reuniões\n");
    let _ = writeln!(
        s,
        "| Fluxo | Reunião | Execuções | Papéis (média) | Itens da ata (média; mín–máx) | Citações conferidas de novo | Seção vazia com itens esperados | Concluídas | Custo (US$) | Tempo | Tokens |"
    );
    let _ = writeln!(s, "|---|---|---|---|---|---|---|---|---|---|---|");
    for r in resumo {
        let Some(a) = &r.ata else { continue };
        let _ = writeln!(
            s,
            "| {} | {} | {} | {} de {} | {} de {} ({}–{}) | {} de {} | {} | {} de {} | {:.2} | {} | {} |",
            r.fluxo,
            r.item,
            r.execucoes,
            pct(a.papeis_media),
            a.papeis_esperados,
            pct(a.itens_media),
            a.itens_esperados,
            pct(a.itens_min),
            pct(a.itens_max),
            a.citacoes_reconferidas,
            a.citacoes,
            a.execucoes_com_secao_vazia,
            r.concluidas,
            r.execucoes,
            r.custo_usd,
            duracao_md(r.duracao_s),
            tokens_md(&r.tokens),
        );
    }
    let _ = writeln!(s);
    let _ = writeln!(s, "Itens atendidos por seção (soma das execuções):\n");
    let _ = writeln!(s, "| Fluxo | Reunião | Seção | Atendidos |");
    let _ = writeln!(s, "|---|---|---|---|");
    for r in resumo {
        let Some(a) = &r.ata else { continue };
        for sec in &a.por_secao {
            if sec.esperados > 0 {
                let _ = writeln!(
                    s,
                    "| {} | {} | {} | {} de {} |",
                    r.fluxo, r.item, sec.secao, sec.atendidos, sec.esperados
                );
            }
        }
    }
    let _ = writeln!(s);
}

fn ata_md(s: &mut String, a: &crate::ata::AvaliacaoAta) {
    let _ = writeln!(
        s,
        "- Ata {} ({}){}: {} itens registrados; objetivo: {}; resumo: {}",
        a.ata,
        a.status,
        match (&a.analise, a.concluida) {
            (Some(id), Some(c)) => format!(
                ", análise {id} {}",
                if c { "concluída" } else { "não concluída" }
            ),
            _ => String::new(),
        },
        a.itens_registrados,
        sim_nao(a.objetivo),
        sim_nao(a.resumo)
    );
    let _ = writeln!(
        s,
        "- Papéis: {} de {} ({})",
        a.papeis_acertados,
        a.papeis_esperados,
        pct(a.cobertura_de_papeis())
    );
    for p in &a.papeis {
        let _ = writeln!(
            s,
            "  - {}: esperado {}, proposto {}{}",
            p.nome,
            p.esperado,
            p.proposto.as_deref().unwrap_or("nenhum"),
            if p.acertou { " (acertou)" } else { "" }
        );
    }
    if !a.outros_participantes.is_empty() {
        let _ = writeln!(
            s,
            "- Propostas para participantes fora do gabarito: {}",
            a.outros_participantes.join(", ")
        );
    }
    let _ = writeln!(
        s,
        "- Itens esperados: {} de {} ({})",
        a.itens_atendidos,
        a.itens_esperados,
        pct(a.cobertura_de_itens())
    );
    for i in &a.itens {
        let mut detalhes = Vec::new();
        if let Some(c) = i.responsavel_confere {
            detalhes.push(format!(
                "responsável {}",
                if c { "confere" } else { "não confere" }
            ));
        }
        if let Some(c) = i.prazo_confere {
            detalhes.push(format!(
                "prazo {}",
                if c { "confere" } else { "não confere" }
            ));
        }
        if let Some(c) = i.cita_fala_esperada {
            detalhes.push(format!(
                "{} a fala esperada",
                if c { "cita" } else { "não cita" }
            ));
        }
        let _ = writeln!(
            s,
            "  - {} ({}) {}: {}{}",
            i.id,
            i.tipo,
            i.descricao,
            i.atendido_por
                .as_deref()
                .map_or("não atendido".to_owned(), |id| format!(
                    "atendido por {id}"
                )),
            if detalhes.is_empty() {
                String::new()
            } else {
                format!(" ({})", detalhes.join(", "))
            }
        );
    }
    let secoes: Vec<String> = a
        .por_secao
        .iter()
        .map(|sec| format!("{} {}", sec.secao.secao(), sec.registrados))
        .collect();
    let _ = writeln!(s, "- Itens registrados por seção: {}", secoes.join(", "));
    let _ = writeln!(
        s,
        "- Seções vazias: {}{}",
        if a.secoes_vazias.is_empty() {
            "nenhuma".to_owned()
        } else {
            a.secoes_vazias.join(", ")
        },
        if a.secoes_vazias_com_itens_esperados.is_empty() {
            String::new()
        } else {
            format!(
                " (com itens esperados: {})",
                a.secoes_vazias_com_itens_esperados.join(", ")
            )
        }
    );
    let c = &a.citacoes;
    let _ = writeln!(
        s,
        "- Citações: {} ({} `conferida`, {} `conferida em`, {} `não confere`){}",
        c.total,
        c.conferidas,
        c.conferidas_em,
        c.nao_conferem,
        c.reconferidas.map_or(String::new(), |n| format!(
            "; conferidas de novo: {n} de {}",
            c.total
        ))
    );
    for r in &c.recusadas {
        let _ = writeln!(s, "  - recusada ({}): {}", r.onde, r.motivo);
    }
    if !a.itens_sem_citacao.is_empty() {
        let _ = writeln!(s, "- Itens sem citação: {}", a.itens_sem_citacao.join(", "));
    }
    let _ = writeln!(s);
}

fn criterios_das_atas(s: &mut String) {
    let _ = writeln!(
        s,
        "- **Papel acertado:** a proposta do agente (`propor_papel`) para o participante é o papel esperado ou um dos aceitos no gabarito (sem acentos nem maiúsculas; o nome casa como no catálogo de pessoas)."
    );
    let _ = writeln!(
        s,
        "- **Item da ata atendido:** um item registrado na seção do item esperado (ou em uma das seções aceitas) tem um termo de cada grupo de palavras-chave do gabarito. Nas pendências, o responsável e o prazo registrados são conferidos à parte."
    );
    let _ = writeln!(
        s,
        "- **Citação conferida de novo:** o trecho aparece na fala citada (falante e início) do transcript analisado."
    );
    let _ = writeln!(
        s,
        "- **Falha grave (reunião):** estado da ata ausente, nenhum item, item sem citação ou citação que não confere."
    );
}

/// Tokens em uma linha: entrada (com o cache), saída.
pub fn tokens_md(t: &Tokens) -> String {
    format!(
        "{} de entrada ({} do cache, {} gravados no cache), {} de saída",
        t.entrada_total(),
        t.cache_leitura,
        t.cache_criacao,
        t.saida
    )
}

/// Duração em minutos e segundos.
pub fn duracao_md(segundos: f64) -> String {
    let total = segundos.round() as u64;
    if total >= 60 {
        format!("{} min {:02} s", total / 60, total % 60)
    } else {
        format!("{total} s")
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
            ata: None,
            transcript: None,
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
