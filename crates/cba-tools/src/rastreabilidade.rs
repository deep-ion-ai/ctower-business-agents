//! Rastreabilidade (#121): a ferramenta MCP `rastreabilidade`, só de leitura, e a
//! exportação da matriz da demanda para Excel (`.xlsx`) e Word.
//!
//! A matriz e a consulta reversa vêm do índice da base
//! ([`cba_kb::indice::Indice::matriz`] e [`cba_kb::indice::Indice::dependentes`]),
//! derivadas dos identificadores gravados nos documentos e nos estados das
//! análises: nada aqui é mantido à mão.
//!
//! - `rastreabilidade(demanda: "D-0012")`: a matriz da demanda, com as lacunas.
//! - `rastreabilidade(regra: "RN-FAT-007")` ou `rastreabilidade(arquivo:
//!   "db/multa.sql")`: o que depende da regra ou do arquivo (demandas, análises,
//!   critérios de aceite e cenários de teste). É o que o agente consulta ao
//!   analisar impacto, para dizer "esta regra já foi tratada na demanda D-0012".

use std::fmt::Write as _;

use cba_docs::{Aba, Formato, OpcoesExportacao};
use cba_kb::IdDemanda;
use cba_kb::indice::{
    Alerta, AlvoReverso, Dependentes, EvidenciaRastro, Indice, Matriz, ResultadoIndice,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::fontes;
use crate::registro::{ErroFerramenta, Ferramenta, Saida, ler_argumentos};

// ---------------------------------------------------------------------------
// Consultas

/// Matriz da demanda, com o índice já sincronizado.
pub fn matriz(indice: &Indice, projeto: &str, demanda: IdDemanda) -> ResultadoIndice<Matriz> {
    indice.matriz(projeto, demanda)
}

/// O que depende da regra ou do arquivo.
pub fn dependentes(
    indice: &Indice,
    projeto: &str,
    alvo: &AlvoReverso,
) -> ResultadoIndice<Dependentes> {
    indice.dependentes(projeto, alvo)
}

// ---------------------------------------------------------------------------
// Exportação

/// Formato da exportação da matriz.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FormatoMatriz {
    /// Planilha do Excel (`.xlsx`).
    Xlsx,
    /// Documento do Word (`.docx`).
    Word,
}

impl FormatoMatriz {
    pub fn extensao(self) -> &'static str {
        match self {
            Self::Xlsx => "xlsx",
            Self::Word => "docx",
        }
    }
}

fn evidencia(e: &EvidenciaRastro) -> String {
    let marca = if e.a_revalidar { " (a revalidar)" } else { "" };
    if e.linha_inicial == e.linha_final {
        format!("{}:{}{marca}", e.arquivo, e.linha_inicial)
    } else {
        format!("{}:{}-{}{marca}", e.arquivo, e.linha_inicial, e.linha_final)
    }
}

fn situacao_historia(s: Option<&str>, aprovada: bool) -> String {
    match s {
        _ if aprovada => "aprovada".into(),
        Some(s) => s.to_owned(),
        None => "pendente".into(),
    }
}

fn alertas(lista: &[Alerta]) -> String {
    if lista.is_empty() {
        "ok".into()
    } else {
        lista
            .iter()
            .map(|a| a.rotulo())
            .collect::<Vec<_>>()
            .join("; ")
    }
}

/// Abas da planilha: por história, por regra e as lacunas.
pub fn abas(m: &Matriz) -> Vec<Aba> {
    let mut historias = Aba::nova(
        "Por história",
        &[
            "História",
            "Título",
            "Situação",
            "Análises",
            "Critérios",
            "Afirmações",
            "Regras",
            "Evidências",
            "Cenários",
            "Lacunas e alertas",
        ],
    );
    for h in &m.historias {
        historias.linha(vec![
            h.id.clone(),
            h.titulo.clone(),
            situacao_historia(h.situacao.as_deref(), h.aprovada)
                + if h.a_confirmar { " (a confirmar)" } else { "" },
            h.analises.join("\n"),
            h.criterios
                .iter()
                .map(|c| format!("{}: {}", c.criterio, c.texto))
                .collect::<Vec<_>>()
                .join("\n"),
            h.afirmacoes.join("\n"),
            h.regras
                .iter()
                .map(|r| r.id.clone())
                .collect::<Vec<_>>()
                .join("\n"),
            h.evidencias
                .iter()
                .map(evidencia)
                .collect::<Vec<_>>()
                .join("\n"),
            h.cenarios
                .iter()
                .map(|c| c.id.clone())
                .collect::<Vec<_>>()
                .join("\n"),
            alertas(&h.alertas),
        ]);
    }
    let mut regras = Aba::nova(
        "Por regra",
        &[
            "Regra",
            "Título",
            "Status",
            "Análises",
            "Histórias",
            "Evidências",
            "Cenários",
            "Lacunas e alertas",
        ],
    );
    for r in &m.regras {
        regras.linha(vec![
            r.regra.id.clone(),
            r.regra.titulo.clone(),
            r.regra
                .status
                .clone()
                .unwrap_or_else(|| "fora da base".into()),
            r.analises.join("\n"),
            r.historias.join("\n"),
            r.evidencias
                .iter()
                .map(evidencia)
                .collect::<Vec<_>>()
                .join("\n"),
            r.cenarios
                .iter()
                .map(|c| c.id.clone())
                .collect::<Vec<_>>()
                .join("\n"),
            alertas(&r.alertas),
        ]);
    }
    let mut lacunas = Aba::nova("Lacunas", &["Lacuna", "Item", "Descrição", "Relacionados"]);
    for l in &m.lacunas {
        lacunas.linha(vec![
            l.alerta.rotulo().to_owned(),
            l.alvo.clone(),
            l.descricao.clone(),
            l.relacionados.join("\n"),
        ]);
    }
    vec![historias, regras, lacunas]
}

/// Célula de tabela Markdown: sem `|` nem quebra de linha.
fn celula(texto: &str) -> String {
    let t = texto.replace('|', "\\|").replace('\n', "<br>");
    if t.is_empty() { "—".into() } else { t }
}

fn celula_lista(itens: impl IntoIterator<Item = String>) -> String {
    let lista: Vec<String> = itens.into_iter().collect();
    if lista.is_empty() {
        "—".into()
    } else {
        lista
            .iter()
            .map(|i| i.replace('|', "\\|"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// A matriz em Markdown, para o Word.
pub fn markdown(m: &Matriz) -> String {
    let mut s = String::new();
    let titulo = if m.demanda.titulo.is_empty() {
        String::new()
    } else {
        format!(": {}", m.demanda.titulo)
    };
    let _ = writeln!(
        s,
        "# Matriz de rastreabilidade da demanda {}{titulo}\n",
        m.demanda.id
    );
    let _ = writeln!(
        s,
        "Projeto `{}`. Derivada do índice da base a partir dos identificadores gravados \
         nas análises, histórias, regras e cenários; nada nesta matriz é mantido à mão.\n",
        m.projeto
    );
    let _ = writeln!(
        s,
        "{} análise(s), {} história(s), {} regra(s) afetada(s) e {} lacuna(s).\n",
        m.analises.len(),
        m.historias.len(),
        m.regras.len(),
        m.lacunas.len()
    );

    s.push_str("## Lacunas e alertas\n\n");
    if m.lacunas.is_empty() {
        s.push_str("Nenhuma lacuna.\n\n");
    } else {
        for l in &m.lacunas {
            let _ = writeln!(s, "- **{}**: {}", l.alerta.rotulo(), l.descricao);
        }
        s.push('\n');
    }

    s.push_str("## Análises\n\n");
    if m.analises.is_empty() {
        s.push_str("Nenhuma análise ligada à demanda.\n\n");
    } else {
        s.push_str("| Análise | Título | Fluxo | Estado |\n|---|---|---|---|\n");
        for a in &m.analises {
            let _ = writeln!(
                s,
                "| {} | {} | {} | {} |",
                a.id,
                celula(&a.titulo),
                celula(a.fluxo.as_deref().unwrap_or("")),
                celula(a.estado.as_deref().unwrap_or(""))
            );
        }
        s.push('\n');
    }

    s.push_str("## Por história\n\n");
    if m.historias.is_empty() {
        s.push_str("Nenhuma história ligada à demanda.\n\n");
    } else {
        s.push_str(
            "| História | Análises | Regras | Evidências | Cenários | Situação |\n\
             |---|---|---|---|---|---|\n",
        );
        for h in &m.historias {
            let _ = writeln!(
                s,
                "| {} {} | {} | {} | {} | {} | {} |",
                h.id,
                celula(&h.titulo),
                celula_lista(h.analises.iter().cloned()),
                celula_lista(h.regras.iter().map(|r| r.id.clone())),
                celula_lista(h.evidencias.iter().map(evidencia)),
                celula_lista(h.cenarios.iter().map(|c| c.id.clone())),
                celula(&format!(
                    "{}; {}",
                    situacao_historia(h.situacao.as_deref(), h.aprovada),
                    alertas(&h.alertas)
                ))
            );
        }
        s.push('\n');
    }

    s.push_str("## Por regra\n\n");
    if m.regras.is_empty() {
        s.push_str("Nenhuma regra afetada.\n");
    } else {
        s.push_str(
            "| Regra | Análises | Histórias | Evidências | Cenários | Situação |\n\
             |---|---|---|---|---|---|\n",
        );
        for r in &m.regras {
            let _ = writeln!(
                s,
                "| {} {} | {} | {} | {} | {} | {} |",
                r.regra.id,
                celula(&r.regra.titulo),
                celula_lista(r.analises.iter().cloned()),
                celula_lista(r.historias.iter().cloned()),
                celula_lista(r.evidencias.iter().map(evidencia)),
                celula_lista(r.cenarios.iter().map(|c| c.id.clone())),
                celula(&alertas(&r.alertas))
            );
        }
    }
    s
}

/// Erro da exportação da matriz.
#[derive(Debug, thiserror::Error)]
pub enum ErroExportacaoMatriz {
    #[error("falha ao gerar a planilha: {0}")]
    Planilha(String),
    #[error(transparent)]
    Word(#[from] cba_docs::ErroExportacao),
}

/// Bytes da matriz no formato pedido.
pub fn exportar(m: &Matriz, formato: FormatoMatriz) -> Result<Vec<u8>, ErroExportacaoMatriz> {
    match formato {
        FormatoMatriz::Xlsx => {
            cba_docs::xlsx(&abas(m)).map_err(|e| ErroExportacaoMatriz::Planilha(e.to_string()))
        }
        FormatoMatriz::Word => Ok(cba_docs::exportar(
            &markdown(m),
            Formato::Word,
            &OpcoesExportacao::default(),
            None,
        )?),
    }
}

// ---------------------------------------------------------------------------
// Ferramenta MCP

#[derive(Debug, Deserialize)]
struct Args {
    #[serde(default)]
    demanda: Option<String>,
    #[serde(default)]
    regra: Option<String>,
    #[serde(default)]
    arquivo: Option<String>,
}

/// `rastreabilidade`: matriz da demanda ou consulta reversa por regra ou arquivo.
pub fn rastreabilidade() -> Ferramenta {
    Ferramenta::nova(
        "rastreabilidade",
        "Consultar a rastreabilidade",
        "Rastreabilidade da base, só de leitura. Com `regra` (RN-…) ou `arquivo` \
         (caminho relativo à raiz do projeto): o que já depende dela (demandas, \
         análises, critérios de aceite das histórias e cenários de teste). Use ao \
         analisar impacto, para avisar \"esta regra já foi tratada na demanda D-0012\" \
         e listar o que precisaria ser revisto. Com `demanda` (D-NNNN): a matriz da \
         demanda (análises, histórias, regras, evidências, cenários) e as lacunas \
         (história sem cenário, regra afetada sem história, evidência a revalidar sob \
         história aprovada). Informe só um dos três.",
        json!({
            "type": "object",
            "properties": {
                "demanda": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 20,
                    "description": "Demanda, como D-0012."
                },
                "regra": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 40,
                    "description": "Regra de negócio, como RN-FAT-007."
                },
                "arquivo": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 500,
                    "description": "Arquivo do projeto, relativo à raiz dele, como db/03_pkg_cobranca.sql."
                }
            },
            "additionalProperties": false
        }),
        |ambiente, args| {
            let args: Args = ler_argumentos(args)?;
            let limpo = |v: Option<String>| v.map(|t| t.trim().to_owned()).filter(|t| !t.is_empty());
            let (demanda, regra, arquivo) =
                (limpo(args.demanda), limpo(args.regra), limpo(args.arquivo));
            let informados =
                usize::from(demanda.is_some()) + usize::from(regra.is_some()) + usize::from(arquivo.is_some());
            if informados != 1 {
                return Err(ErroFerramenta::Argumentos(
                    "informe exatamente um entre `demanda` (D-NNNN), `regra` (RN-…) e \
                     `arquivo` (caminho relativo à raiz do projeto)"
                        .into(),
                ));
            }
            let indice = fontes::indice(ambiente)?;
            let projeto = ambiente.kb.nome();
            let falha = |e: cba_kb::indice::ErroIndice| {
                ErroFerramenta::Interno(format!("rastreabilidade no índice da base: {e}"))
            };
            if let Some(d) = demanda {
                let id = IdDemanda::analisar(&d).map_err(|e| {
                    ErroFerramenta::Argumentos(format!("`demanda`: {e}; use D-NNNN, como D-0012"))
                })?;
                let m = matriz(&indice, projeto, id).map_err(falha)?;
                return Ok(Saida {
                    texto: texto_da_matriz(&m),
                    dados: Some(serde_json::to_value(&m).unwrap_or_default()),
                });
            }
            let alvo = match (regra, arquivo) {
                (Some(r), _) => {
                    let id = cba_kb::Id::analisar(&r)
                        .ok()
                        .filter(|i| matches!(i.serie(), cba_kb::Serie::Regra { .. }))
                        .ok_or_else(|| {
                            ErroFerramenta::Argumentos(format!(
                                "`regra`: `{r}` não é um identificador de regra; use \
                                 RN-<SIGLA>-NNN, como RN-FAT-007"
                            ))
                        })?;
                    AlvoReverso::Regra(id.to_string())
                }
                (None, Some(a)) => {
                    let a = a.replace('\\', "/");
                    let a = a.trim_start_matches("./").to_owned();
                    cba_kb::CaminhoRelativo::novo(&a).map_err(|e| {
                        ErroFerramenta::Argumentos(format!(
                            "`arquivo`: {e}; use o caminho relativo à raiz do projeto"
                        ))
                    })?;
                    AlvoReverso::Arquivo(a)
                }
                (None, None) => unreachable!("conferido acima"),
            };
            let d = dependentes(&indice, projeto, &alvo).map_err(falha)?;
            Ok(Saida {
                texto: texto_dos_dependentes(&d),
                dados: Some(serde_json::to_value(&d).unwrap_or_default()),
            })
        },
    )
    .somente_leitura()
}

fn texto_da_matriz(m: &Matriz) -> String {
    let mut s = String::new();
    let _ = writeln!(
        s,
        "Matriz de rastreabilidade da demanda {}{} (derivada do índice da base).",
        m.demanda.id,
        if m.demanda.titulo.is_empty() {
            String::new()
        } else {
            format!(": {}", m.demanda.titulo)
        }
    );
    if m.analises.is_empty() {
        let _ = writeln!(
            s,
            "Nenhuma análise está ligada à demanda (campo `demanda.id` do frontmatter)."
        );
        return s;
    }
    let _ = writeln!(
        s,
        "Análises: {}.",
        m.analises
            .iter()
            .map(|a| format!(
                "{} ({}, {})",
                a.id,
                a.fluxo.as_deref().unwrap_or("?"),
                a.estado.as_deref().unwrap_or("?")
            ))
            .collect::<Vec<_>>()
            .join(", ")
    );
    s.push_str("\nHistórias:\n");
    if m.historias.is_empty() {
        s.push_str("- nenhuma\n");
    }
    for h in &m.historias {
        let _ = writeln!(
            s,
            "- {} {} [{}]: regras {}; cenários {}; {}",
            h.id,
            h.titulo,
            situacao_historia(h.situacao.as_deref(), h.aprovada),
            celula_lista(h.regras.iter().map(|r| r.id.clone())),
            celula_lista(h.cenarios.iter().map(|c| c.id.clone())),
            alertas(&h.alertas)
        );
    }
    s.push_str("\nRegras afetadas:\n");
    if m.regras.is_empty() {
        s.push_str("- nenhuma\n");
    }
    for r in &m.regras {
        let _ = writeln!(
            s,
            "- {} {}: em {}; histórias {}; {}",
            r.regra.id,
            r.regra.titulo,
            celula_lista(r.analises.iter().cloned()),
            celula_lista(r.historias.iter().cloned()),
            alertas(&r.alertas)
        );
    }
    s.push_str("\nLacunas:\n");
    if m.lacunas.is_empty() {
        s.push_str("- nenhuma\n");
    }
    for l in &m.lacunas {
        let _ = writeln!(s, "- {}: {}", l.alerta.rotulo(), l.descricao);
    }
    s
}

fn texto_dos_dependentes(d: &Dependentes) -> String {
    let mut s = String::new();
    let alvo = match &d.alvo {
        AlvoReverso::Regra(r) => format!("a regra {r}"),
        AlvoReverso::Arquivo(a) => format!("o arquivo `{a}`"),
    };
    if d.vazio() {
        let _ = writeln!(
            s,
            "Nada na base depende de {alvo}: nenhuma demanda, análise, critério ou cenário \
             a cita."
        );
        if let AlvoReverso::Arquivo(_) = d.alvo
            && !d.regras.is_empty()
        {
            let _ = writeln!(
                s,
                "Regras com evidência no arquivo: {}.",
                celula_lista(d.regras.iter().map(|r| r.id.clone()))
            );
        }
        return s;
    }
    let _ = writeln!(
        s,
        "O que depende de {alvo} (se mudar, estes itens precisam ser revistos):"
    );
    if let Some(r) = &d.regra
        && r.status.as_deref() == Some("a-revalidar")
    {
        let _ = writeln!(
            s,
            "A regra está a revalidar: o código das evidências mudou desde a confirmação."
        );
    }
    if !d.regras.is_empty() {
        let _ = writeln!(
            s,
            "- Regras com evidência no arquivo: {}",
            celula_lista(d.regras.iter().map(|r| r.id.clone()))
        );
    }
    let _ = writeln!(
        s,
        "- Demandas: {}",
        celula_lista(d.demandas.iter().map(|x| format!(
            "{}{} ({})",
            x.id,
            if x.titulo.is_empty() {
                String::new()
            } else {
                format!(" {}", x.titulo)
            },
            x.status.as_deref().unwrap_or("?")
        )))
    );
    let _ = writeln!(
        s,
        "- Análises: {}",
        celula_lista(d.analises.iter().map(|a| format!(
            "{} {} ({})",
            a.id,
            a.titulo,
            a.estado.as_deref().unwrap_or("?")
        )))
    );
    let _ = writeln!(
        s,
        "- Critérios de aceite: {}",
        celula_lista(d.criterios.iter().map(|c| format!(
            "{} {}{}",
            c.historia,
            c.criterio,
            if c.a_revalidar { " (revisar)" } else { "" }
        )))
    );
    let _ = writeln!(
        s,
        "- Cenários de teste: {}",
        celula_lista(d.cenarios.iter().map(|c| format!(
            "{}{}",
            c.id,
            if c.a_revalidar { " (revisar)" } else { "" }
        )))
    );
    if let Some(primeira) = d.demandas.first() {
        let _ = writeln!(
            s,
            "\nAo analisar impacto, cite: \"{} já foi tratada na demanda {}\".",
            match &d.alvo {
                AlvoReverso::Regra(r) => r.clone(),
                AlvoReverso::Arquivo(a) => format!("`{a}`"),
            },
            primeira.id
        );
    }
    s
}
