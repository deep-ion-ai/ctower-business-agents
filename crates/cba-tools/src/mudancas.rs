//! O que mudou no código desde a última análise (#124).
//!
//! A revalidação por hash ([`crate::revalidacao`]) diz **que** uma evidência deixou
//! de conferir; este módulo diz **o que** mudou entre dois pontos do repositório do
//! projeto, em termos do que a base já documenta. É conta, sem IA, e offline: lê o
//! histórico com o `git` instalado (o mesmo que o analista usa para clonar o
//! projeto) e o estado da base.
//!
//! # O relatório ([`relatorio`])
//!
//! - **Intervalo:** `de` e `ate` são commits, tags ou branches. Sem `de`, vale o
//!   commit gravado nas evidências da **última análise aprovada**; sem `ate`, a
//!   cópia de trabalho (o que está no disco, inclusive o que ainda não foi
//!   commitado e os arquivos novos fora do `.gitignore`). Cada pasta do projeto é
//!   lida no repositório que a contém; a pasta em que a referência não existe vai
//!   para os problemas.
//! - **Arquivos alterados com evidências na base:** os itens que citam o arquivo,
//!   as regras (menos as obsoletas), os termos aceitos e os itens das análises
//!   (afirmações, termos, sugestões, critérios, cenários e tabelas, com a mesma
//!   [`Origem`] da revalidação) e as funcionalidades do mapa (#208), pelos pontos
//!   de entrada.
//! - **Para cada regra afetada, o diff do trecho evidenciado:** só os blocos do
//!   diff que tocam as linhas citadas, e a [`Classe`] da mudança: o trecho mudou,
//!   foi só reformatado (diferença só de espaços), foi movido (o mesmo texto em
//!   outras linhas), foi removido, ou não mudou (o arquivo mudou em outro ponto).
//!   As linhas da evidência são as do ponto `de`; evidência registrada depois
//!   (que confere com `ate`) é levada de volta ao ponto `de` pelo próprio diff.
//! - **Arquivos alterados sem evidência**, agrupados pela área: a da funcionalidade
//!   que tem ponto de entrada no arquivo ou, sem ela, a área mais citada na pasta
//!   do arquivo (pelas regras e pelas funcionalidades); sem área, pela pasta. É a
//!   lista das possíveis regras novas, ainda não documentadas.
//!
//! [`nota_de_mudancas`] transforma o relatório numa nota de mudanças em Markdown,
//! para o negócio.
//!
//! # Explicar mudanças (fluxo de IA opcional)
//!
//! O agente lê o relatório (`relatorio_de_mudancas`) e, por regra afetada, registra
//! se o comportamento mudou, com a nova evidência e, quando mudou, o texto
//! atualizado (`propor_atualizacao_de_regra`). A proposta fica pendente no
//! diretório de dados do app ([`ARQUIVO_DAS_PROPOSTAS`]), até o analista
//! confirmar ([`confirmar_proposta`], com o texto ajustado se quiser) ou descartar
//! ([`descartar_proposta`]). Confirmada, a regra mantém o identificador, ganha o
//! texto e as evidências novas, volta a `confirmada` e guarda a versão que valia
//! no `historico` do frontmatter ([`VersaoDaRegra`]). O pedido que o app manda ao
//! agente é [`pedido_de_explicacao`].

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write as _;
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

use cba_core::ambiente::Ambiente;
use cba_core::analise::{Analise, EstadoAnalise, EstadoTermo};
use cba_core::funcionalidades::MapaFuncionalidades;
use cba_docs::{Conteudo, Templates};
use cba_kb::frontmatter::{Evidencia, Linhas, StatusRegra, VersaoDaRegra};
use cba_kb::{CaminhoRelativo, Id};
use serde::{Deserialize, Serialize};
use serde_json::json;
use similar::{Algorithm, DiffOp, DiffTag};

use crate::evidencias::{
    EvidenciaCitada, LIMITE_DO_ARQUIVO, decodificar, hash_das_linhas, schema_da_evidencia,
    validar_todas,
};
use crate::registro::{ErroFerramenta, Ferramenta, Saida, ler_argumentos};
use crate::revalidacao::{Origem, evidencias_da_analise, evidencias_dos_fundamentos, ler_regras};

/// Arquivo das propostas de atualização de regra, na pasta do projeto no cadastro
/// (`<dados>/projetos/<id>/`), ao lado do cache da revalidação.
pub const ARQUIVO_DAS_PROPOSTAS: &str = "mudancas.json";
const TRAVA_DAS_PROPOSTAS: &str = "mudancas.trava";
const VERSAO_DAS_PROPOSTAS: u32 = 1;

/// Linhas de contexto em volta de cada bloco do diff.
const CONTEXTO: usize = 2;
/// Linhas de diff por trecho; o resto é cortado.
const LIMITE_DO_DIFF: usize = 120;
/// Commits listados no relatório.
const LIMITE_DE_COMMITS: usize = 100;

// ---------------------------------------------------------------------------
// Tipos do relatório

/// Os dois pontos do repositório.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct Pedido {
    /// Commit, tag ou branch de partida; sem ele, a última análise aprovada.
    #[serde(default)]
    pub de: Option<String>,
    /// Commit, tag ou branch de chegada; sem ele, a cópia de trabalho.
    #[serde(default)]
    pub ate: Option<String>,
}

/// Análise que deu o ponto de partida.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AnaliseDeReferencia {
    pub id: Id,
    pub titulo: String,
}

/// Um ponto do intervalo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Ponto {
    /// Como foi pedido (`v1.2`, `abc123`) ou `cópia de trabalho`.
    pub referencia: String,
    /// Commit (hash completo) no primeiro repositório do projeto; sem ele na cópia
    /// de trabalho de um repositório sem commits.
    pub commit: Option<String>,
    /// A última análise aprovada, quando o ponto veio dela.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub analise: Option<AnaliseDeReferencia>,
}

/// Commit do intervalo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommitNoIntervalo {
    pub hash: String,
    /// `AAAA-MM-DD`.
    pub data: String,
    pub assunto: String,
}

/// Situação de um arquivo no intervalo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SituacaoArquivo {
    Adicionado,
    Modificado,
    Renomeado,
    Removido,
}

impl SituacaoArquivo {
    fn rotulo(self) -> &'static str {
        match self {
            Self::Adicionado => "adicionado",
            Self::Modificado => "modificado",
            Self::Renomeado => "renomeado",
            Self::Removido => "removido",
        }
    }
}

/// Arquivo alterado no intervalo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ArquivoAlterado {
    /// Relativo à raiz do projeto.
    pub arquivo: String,
    pub situacao: SituacaoArquivo,
    /// Caminho no ponto `de`, quando o arquivo foi renomeado.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anterior: Option<String>,
}

/// O que aconteceu com um trecho citado, da menor para a maior mudança.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Classe {
    /// O texto citado é o mesmo (o arquivo mudou em outro ponto; as linhas podem
    /// ter andado).
    Inalterado,
    /// Só mudaram espaços, quebras de linha ou indentação.
    Reformatado,
    /// O mesmo texto está em outras linhas.
    Movido,
    /// Evidência registrada depois do ponto `de`, num arquivo que não existia nele.
    Novo,
    /// O texto citado mudou.
    Alterado,
    /// O trecho ou o arquivo foi removido.
    Removido,
}

impl Classe {
    /// A mudança pode ter mudado o comportamento: pede a releitura da regra.
    pub fn mudou(self) -> bool {
        matches!(self, Self::Alterado | Self::Removido)
    }

    pub fn rotulo(self) -> &'static str {
        match self {
            Self::Inalterado => "trecho sem mudança",
            Self::Reformatado => "só reformatado",
            Self::Movido => "trecho movido",
            Self::Novo => "trecho novo",
            Self::Alterado => "trecho alterado",
            Self::Removido => "trecho removido",
        }
    }
}

/// Trecho citado por uma regra num arquivo alterado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TrechoAfetado {
    pub arquivo: String,
    /// Linhas citadas, como gravadas na evidência.
    pub linhas: Linhas,
    /// Onde o trecho está no ponto `ate` (ausente se foi removido).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub linhas_novas: Option<Linhas>,
    pub classe: Classe,
    /// Blocos do diff unificado que tocam o trecho (vazio sem mudança).
    pub diff: String,
}

/// Regra com evidência num arquivo alterado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RegraAfetada {
    pub id: String,
    pub titulo: String,
    pub area: String,
    pub status: StatusRegra,
    /// Documento, relativo à raiz do projeto na base.
    pub documento: CaminhoRelativo,
    /// A maior mudança entre os trechos.
    pub classe: Classe,
    pub trechos: Vec<TrechoAfetado>,
}

/// Termo do glossário (aceito numa análise) com evidência num arquivo alterado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TermoAfetado {
    pub termo: String,
    pub analise: Id,
    pub classe: Classe,
}

/// Funcionalidade do mapa com ponto de entrada num arquivo alterado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FuncionalidadeAfetada {
    pub id: String,
    pub nome: String,
    pub area: String,
    pub area_nome: String,
    pub classe: Classe,
}

/// Item de uma análise com evidência num arquivo alterado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ItemAfetado {
    pub origem: Origem,
    pub arquivo: String,
    pub linhas: Linhas,
    pub classe: Classe,
}

/// Análise com itens que citam arquivos alterados.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AnaliseAfetada {
    pub id: Id,
    pub titulo: String,
    pub estado: EstadoAnalise,
    pub itens: Vec<ItemAfetado>,
}

/// Arquivo alterado que a base cita, com quem o cita.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ArquivoComEvidencias {
    #[serde(flatten)]
    pub arquivo: ArquivoAlterado,
    pub regras: Vec<String>,
    pub termos: Vec<String>,
    pub funcionalidades: Vec<String>,
    pub analises: Vec<Id>,
}

/// Arquivos alterados sem evidência na base, de uma área (ou de uma pasta, sem
/// área conhecida).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GrupoSemEvidencia {
    /// Slug da área; ausente quando o grupo é a pasta.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub area: Option<String>,
    /// Nome da área ou a pasta.
    pub nome: String,
    pub arquivos: Vec<ArquivoAlterado>,
}

/// O que mudou no código entre dois pontos, em termos da base.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RelatorioDeMudancas {
    pub projeto: String,
    pub de: Ponto,
    pub ate: Ponto,
    /// Commits do intervalo (até [`LIMITE_DE_COMMITS`], do mais recente).
    pub commits: Vec<CommitNoIntervalo>,
    /// Arquivos alterados no intervalo, dentro do escopo do projeto.
    pub arquivos_alterados: usize,
    pub com_evidencias: Vec<ArquivoComEvidencias>,
    /// Das que mudaram para as que não mudaram.
    pub regras: Vec<RegraAfetada>,
    pub termos: Vec<TermoAfetado>,
    pub funcionalidades: Vec<FuncionalidadeAfetada>,
    pub analises: Vec<AnaliseAfetada>,
    pub sem_evidencias: Vec<GrupoSemEvidencia>,
    pub problemas: Vec<String>,
    pub duracao_ms: u64,
}

impl RelatorioDeMudancas {
    /// Regras cujo trecho mudou (alterado ou removido).
    pub fn regras_que_mudaram(&self) -> impl Iterator<Item = &RegraAfetada> {
        self.regras.iter().filter(|r| r.classe.mudou())
    }
}

// ---------------------------------------------------------------------------
// Git

/// Executa `git -C <dir> <args>` sem mexer na pasta do analista: sem travas
/// opcionais, saída em inglês e sem janela de console no Windows. `None` se o git
/// saiu com erro.
fn git(dir: &Path, args: &[&str]) -> Result<Option<Vec<u8>>, String> {
    let mut c = Command::new("git");
    c.arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000);
    }
    let saida = c.output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            "o git não foi encontrado; instale-o para comparar versões do código".to_owned()
        } else {
            format!("o git não pôde ser executado ({:?})", e.kind())
        }
    })?;
    Ok(saida.status.success().then_some(saida.stdout))
}

/// Referência aceitável: sem espaços nem controles, e não começa com `-` (não vira
/// opção do git).
fn referencia_valida(r: &str) -> bool {
    !r.is_empty()
        && r.len() <= 200
        && !r.starts_with('-')
        && !r.chars().any(|c| c.is_whitespace() || c.is_control())
}

/// Commit (hash completo) da referência no repositório da pasta.
fn resolver(dir: &Path, referencia: &str) -> Result<Option<String>, String> {
    let alvo = format!("{referencia}^{{commit}}");
    Ok(git(dir, &["rev-parse", "--verify", "-q", &alvo])?
        .map(|s| String::from_utf8_lossy(&s).trim().to_owned())
        .filter(|s| !s.is_empty()))
}

/// Repositório de uma pasta do projeto, com o intervalo resolvido.
struct Repositorio {
    /// Pasta do projeto (o `git -C` roda nela; os caminhos saem relativos a ela).
    pasta: PathBuf,
    prefixo: Option<String>,
    de: String,
    /// `None`: a cópia de trabalho.
    ate: Option<String>,
}

impl Repositorio {
    fn no_projeto(&self, relativo: &str) -> String {
        match &self.prefixo {
            Some(p) => format!("{p}/{relativo}"),
            None => relativo.to_owned(),
        }
    }

    /// Arquivos alterados: `(situação, caminho, anterior)`, relativos à pasta.
    fn alterados(&self) -> Result<Vec<(SituacaoArquivo, String, Option<String>)>, String> {
        let mut args = vec![
            "diff",
            "--name-status",
            "-z",
            "-M",
            "--relative",
            "--no-ext-diff",
            &self.de,
        ];
        if let Some(a) = &self.ate {
            args.push(a);
        }
        args.extend(["--", "."]);
        let saida = git(&self.pasta, &args)?.ok_or("o git diff falhou")?;
        let texto = String::from_utf8_lossy(&saida);
        let mut partes = texto.split('\0').filter(|p| !p.is_empty());
        let mut alterados = Vec::new();
        while let Some(status) = partes.next() {
            let Some(caminho) = partes.next() else { break };
            let caminho = caminho.to_owned();
            match status.chars().next() {
                Some('A') => alterados.push((SituacaoArquivo::Adicionado, caminho, None)),
                Some('D') => alterados.push((SituacaoArquivo::Removido, caminho, None)),
                Some('R') => {
                    let Some(novo) = partes.next() else { break };
                    alterados.push((SituacaoArquivo::Renomeado, novo.to_owned(), Some(caminho)));
                }
                Some('C') => {
                    let Some(novo) = partes.next() else { break };
                    alterados.push((SituacaoArquivo::Adicionado, novo.to_owned(), None));
                }
                _ => alterados.push((SituacaoArquivo::Modificado, caminho, None)),
            }
        }
        // Na cópia de trabalho, os arquivos novos ainda fora do git (menos os
        // ignorados) também mudaram.
        if self.ate.is_none()
            && let Some(saida) = git(
                &self.pasta,
                &[
                    "ls-files",
                    "-z",
                    "--others",
                    "--exclude-standard",
                    "--",
                    ".",
                ],
            )?
        {
            for caminho in String::from_utf8_lossy(&saida).split('\0') {
                if !caminho.is_empty() {
                    alterados.push((SituacaoArquivo::Adicionado, caminho.to_owned(), None));
                }
            }
        }
        Ok(alterados)
    }

    fn commits(&self) -> Vec<CommitNoIntervalo> {
        let intervalo = format!("{}..{}", self.de, self.ate.as_deref().unwrap_or("HEAD"));
        let limite = format!("-n{LIMITE_DE_COMMITS}");
        let Ok(Some(saida)) = git(
            &self.pasta,
            &[
                "log",
                &limite,
                "--format=%H%x1f%cs%x1f%s",
                &intervalo,
                "--",
                ".",
            ],
        ) else {
            return Vec::new();
        };
        String::from_utf8_lossy(&saida)
            .lines()
            .filter_map(|l| {
                let mut p = l.splitn(3, '\u{1f}');
                Some(CommitNoIntervalo {
                    hash: p.next()?.to_owned(),
                    data: p.next()?.to_owned(),
                    assunto: p.next().unwrap_or("").to_owned(),
                })
            })
            .collect()
    }

    /// Conteúdo do arquivo num commit (`None` se não existe nele ou não é texto).
    fn no_commit(&self, commit: &str, relativo: &str) -> Option<String> {
        let alvo = format!("{commit}:./{relativo}");
        let bytes = git(&self.pasta, &["show", "--no-textconv", &alvo]).ok()??;
        (bytes.len() as u64 <= LIMITE_DO_ARQUIVO)
            .then(|| decodificar(&bytes).ok())
            .flatten()
    }

    fn antigo(&self, relativo: &str) -> Option<String> {
        self.no_commit(&self.de, relativo)
    }

    fn novo(&self, relativo: &str) -> Option<String> {
        match &self.ate {
            Some(a) => self.no_commit(a, relativo),
            None => {
                let caminho = relativo
                    .split('/')
                    .fold(self.pasta.clone(), |c, parte| c.join(parte));
                crate::evidencias::ler_texto(&caminho).ok()
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Diff do trecho

/// Linha normalizada como no hash da evidência.
fn normalizada(l: &str) -> String {
    l.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn sem_espacos(linhas: &[String]) -> String {
    linhas
        .iter()
        .flat_map(|l| l.chars().filter(|c| !c.is_whitespace()))
        .collect()
}

/// Primeira linha (0-based) do ponto `ate` que corresponde à linha `i` do ponto
/// `de`.
fn novo_inicio(ops: &[DiffOp], i: usize) -> usize {
    for op in ops {
        let (o, n) = (op.old_range(), op.new_range());
        if o.contains(&i) {
            return match op.tag() {
                DiffTag::Equal => n.start + (i - o.start),
                _ => n.start,
            };
        }
    }
    ops.last().map_or(0, |op| op.new_range().end)
}

/// Fim (exclusivo) no ponto `ate` do trecho que termina na linha `i` do ponto `de`.
fn novo_fim(ops: &[DiffOp], i: usize) -> usize {
    for op in ops {
        let (o, n) = (op.old_range(), op.new_range());
        if o.contains(&i) {
            return match op.tag() {
                DiffTag::Equal => n.start + (i - o.start) + 1,
                _ => n.end,
            };
        }
    }
    ops.last().map_or(0, |op| op.new_range().end)
}

/// Intervalo `[a0, a1)` do ponto `de` levado ao ponto `ate`.
fn mapear(ops: &[DiffOp], a0: usize, a1: usize) -> (usize, usize) {
    if a1 <= a0 {
        let p = novo_inicio(ops, a0);
        return (p, p);
    }
    let b0 = novo_inicio(ops, a0);
    (b0, novo_fim(ops, a1 - 1).max(b0))
}

/// O bloco do diff toca o intervalo `[a0, a1)` do ponto `de`.
fn toca(op: &DiffOp, a0: usize, a1: usize) -> bool {
    if op.tag() == DiffTag::Equal {
        return false;
    }
    let o = op.old_range();
    if o.is_empty() {
        a0 <= o.start && o.start <= a1
    } else {
        o.start < a1 && o.end > a0
    }
}

/// Os blocos do diff unificado que tocam o intervalo, com [`CONTEXTO`] linhas em
/// volta.
fn diff_do_trecho(ops: &[DiffOp], antigo: &[&str], novo: &[&str], a0: usize, a1: usize) -> String {
    let mut saida = String::new();
    let mut linhas = 0;
    for grupo in similar::group_diff_ops(ops.to_vec(), CONTEXTO) {
        if !grupo.iter().any(|op| toca(op, a0, a1)) {
            continue;
        }
        let (Some(primeiro), Some(ultimo)) = (grupo.first(), grupo.last()) else {
            continue;
        };
        let (o0, n0) = (primeiro.old_range().start, primeiro.new_range().start);
        let (o1, n1) = (ultimo.old_range().end, ultimo.new_range().end);
        let _ = writeln!(
            saida,
            "@@ -{},{} +{},{} @@",
            o0 + 1,
            o1 - o0,
            n0 + 1,
            n1 - n0
        );
        for op in &grupo {
            let mut escrever = |sinal: char, linha: &str| {
                if linhas < LIMITE_DO_DIFF {
                    let _ = writeln!(saida, "{sinal}{linha}");
                }
                linhas += 1;
            };
            match op.tag() {
                DiffTag::Equal => op.old_range().for_each(|i| escrever(' ', antigo[i])),
                DiffTag::Delete => op.old_range().for_each(|i| escrever('-', antigo[i])),
                DiffTag::Insert => op.new_range().for_each(|i| escrever('+', novo[i])),
                DiffTag::Replace => {
                    op.old_range().for_each(|i| escrever('-', antigo[i]));
                    op.new_range().for_each(|i| escrever('+', novo[i]));
                }
            }
        }
    }
    if linhas > LIMITE_DO_DIFF {
        let _ = writeln!(
            saida,
            "… ({} linhas do diff cortadas)",
            linhas - LIMITE_DO_DIFF
        );
    }
    saida
}

/// Linhas a partir de 1, de um intervalo `[i, f)` não vazio.
fn linhas_de(i: usize, f: usize) -> Option<Linhas> {
    Linhas::novo(u32::try_from(i + 1).ok()?, u32::try_from(f).ok()?)
}

/// O que aconteceu com o trecho citado (`linhas`, com o `hash` da evidência)
/// entre o conteúdo `antigo` (ponto `de`) e o `novo` (ponto `ate`).
fn analisar_trecho(
    antigo: Option<&str>,
    novo: Option<&str>,
    linhas: Linhas,
    hash: &str,
) -> (Classe, Option<Linhas>, String) {
    let intervalo = |total: usize| {
        let a0 = (linhas.inicio as usize - 1).min(total);
        (a0, (linhas.fim as usize).min(total).max(a0))
    };
    let (antigo, novo) = match (antigo, novo) {
        (None, Some(_)) => return (Classe::Novo, Some(linhas), String::new()),
        (None, None) => return (Classe::Removido, None, String::new()),
        (Some(a), None) => {
            let a: Vec<&str> = a.lines().collect();
            let (a0, a1) = intervalo(a.len());
            let diff = a[a0..a1].iter().fold(String::new(), |mut s, l| {
                let _ = writeln!(s, "-{l}");
                s
            });
            return (Classe::Removido, None, diff);
        }
        (Some(a), Some(n)) => (a, n),
    };
    let a: Vec<&str> = antigo.lines().collect();
    let n: Vec<&str> = novo.lines().collect();
    let ops = similar::capture_diff_slices(Algorithm::Myers, &a, &n);

    // As linhas da evidência são as do ponto `de`, a não ser que o trecho só
    // confira no ponto `ate` (evidência registrada depois): aí o diff ao contrário
    // as leva de volta.
    let (a0, a1) = if hash_das_linhas(antigo, linhas).as_deref() == Some(hash) {
        intervalo(a.len())
    } else if hash_das_linhas(novo, linhas).as_deref() == Some(hash) {
        let volta = similar::capture_diff_slices(Algorithm::Myers, &n, &a);
        let (b0, b1) = intervalo(n.len());
        mapear(&volta, b0, b1)
    } else {
        intervalo(a.len())
    };
    let (b0, b1) = mapear(&ops, a0, a1);
    let velho: Vec<String> = a[a0..a1].iter().map(|l| normalizada(l)).collect();
    let atual: Vec<String> = n[b0..b1].iter().map(|l| normalizada(l)).collect();
    let diff = diff_do_trecho(&ops, &a, &n, a0, a1);

    if velho == atual {
        return (Classe::Inalterado, linhas_de(b0, b1), String::new());
    }
    if !velho.is_empty() && sem_espacos(&velho) == sem_espacos(&atual) {
        return (Classe::Reformatado, linhas_de(b0, b1), diff);
    }
    if !velho.is_empty() && velho.len() <= n.len() {
        let todas: Vec<String> = n.iter().map(|l| normalizada(l)).collect();
        if let Some(j) = todas
            .windows(velho.len())
            .position(|w| w == velho.as_slice())
        {
            return (Classe::Movido, linhas_de(j, j + velho.len()), diff);
        }
    }
    if b1 <= b0 {
        return (Classe::Removido, None, diff);
    }
    (Classe::Alterado, linhas_de(b0, b1), diff)
}

// ---------------------------------------------------------------------------
// Relatório

/// Commit da última análise aprovada: a análise aprovada mais recente e, nela, o
/// commit da última evidência que gravou um.
fn ultima_aprovada(analises: &[Analise]) -> Option<(&Analise, String)> {
    let mut aprovadas: Vec<&Analise> = analises
        .iter()
        .filter(|a| a.estado == EstadoAnalise::Aprovada)
        .collect();
    aprovadas.sort_by_key(|a| (a.atualizada_ms, a.id.numero()));
    aprovadas.into_iter().rev().find_map(|a| {
        let commits = evidencias_da_analise(a)
            .filter_map(|(_, e)| e.commit.clone())
            .chain(
                evidencias_dos_fundamentos(a)
                    .into_iter()
                    .filter_map(|(_, e)| e.commit.clone()),
            );
        commits.last().map(|c| (a, c))
    })
}

/// Quem cita um trecho.
enum Dono {
    Regra(usize),
    Analise(usize, Origem),
    Funcionalidade(usize),
}

/// Regras, termos, funcionalidades e análises que citam um arquivo.
type Citantes = (
    BTreeSet<String>,
    BTreeSet<String>,
    BTreeSet<String>,
    Vec<Id>,
);

/// Arquivo alterado e de onde ler as duas versões.
struct Alterado {
    arquivo: ArquivoAlterado,
    repositorio: usize,
    /// Relativo à pasta, no ponto `ate` e no ponto `de`.
    novo: String,
    antigo: String,
}

fn pasta_de(arquivo: &str) -> &str {
    arquivo.rsplit_once('/').map_or("", |(p, _)| p)
}

/// O relatório do que mudou entre `pedido.de` e `pedido.ate` (spec do módulo).
pub fn relatorio(
    ambiente: &Ambiente,
    pedido: &Pedido,
) -> Result<RelatorioDeMudancas, ErroFerramenta> {
    let inicio = Instant::now();
    let limpo = |v: &Option<String>| {
        v.as_deref()
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .map(str::to_owned)
    };
    let (de_pedido, ate_pedido) = (limpo(&pedido.de), limpo(&pedido.ate));
    for r in [&de_pedido, &ate_pedido].into_iter().flatten() {
        if !referencia_valida(r) {
            return Err(ErroFerramenta::Argumentos(format!(
                "`{r}` não é uma referência do git; use um commit, uma tag ou uma branch"
            )));
        }
    }
    let analises = ambiente
        .analises
        .listar()
        .map_err(|e| ErroFerramenta::Interno(e.to_string()))?;
    let (de, analise_de) = match de_pedido {
        Some(d) => (d, None),
        None => {
            let (a, commit) = ultima_aprovada(&analises).ok_or_else(|| {
                ErroFerramenta::Recusa(
                    "nenhuma análise aprovada tem o commit gravado nas evidências; informe \
                     `de` (um commit, uma tag ou uma branch) para comparar"
                        .into(),
                )
            })?;
            (
                commit,
                Some(AnaliseDeReferencia {
                    id: a.id.clone(),
                    titulo: a.titulo.clone(),
                }),
            )
        }
    };

    // Repositórios das pastas do projeto, com o intervalo resolvido.
    let mut problemas = Vec::new();
    let mut repositorios = Vec::new();
    for pasta in ambiente.escopo.pastas() {
        let nome = pasta
            .prefixo
            .clone()
            .unwrap_or_else(|| "raiz do projeto".to_owned());
        if cba_index::detectar_git(&pasta.caminho).is_none() {
            problemas.push(format!("a pasta {nome} não está num repositório git"));
            continue;
        }
        let Some(commit_de) = resolver(&pasta.caminho, &de).map_err(ErroFerramenta::Recusa)? else {
            problemas.push(format!("`{de}` não existe no repositório da pasta {nome}"));
            continue;
        };
        let commit_ate = match &ate_pedido {
            Some(a) => match resolver(&pasta.caminho, a).map_err(ErroFerramenta::Recusa)? {
                Some(c) => Some(c),
                None => {
                    problemas.push(format!("`{a}` não existe no repositório da pasta {nome}"));
                    continue;
                }
            },
            None => None,
        };
        repositorios.push(Repositorio {
            pasta: pasta.caminho.clone(),
            prefixo: pasta.prefixo.clone(),
            de: commit_de,
            ate: commit_ate,
        });
    }
    if repositorios.is_empty() {
        return Err(ErroFerramenta::Recusa(format!(
            "nenhuma pasta do projeto pôde ser comparada: {}",
            problemas.join("; ")
        )));
    }

    // Arquivos alterados, no escopo (fora das exclusões).
    let mut alterados: Vec<Alterado> = Vec::new();
    let mut commits = Vec::new();
    for (i, r) in repositorios.iter().enumerate() {
        commits.extend(r.commits());
        let lista = r.alterados().map_err(ErroFerramenta::Recusa)?;
        for (situacao, novo, anterior) in lista {
            let arquivo = r.no_projeto(&novo);
            if ambiente
                .escopo
                .exclusoes()
                .avaliar_caminho(&arquivo)
                .is_some()
            {
                continue;
            }
            let antigo = anterior.clone().unwrap_or_else(|| novo.clone());
            alterados.push(Alterado {
                arquivo: ArquivoAlterado {
                    arquivo,
                    situacao,
                    anterior: anterior.map(|a| r.no_projeto(&a)),
                },
                repositorio: i,
                novo,
                antigo,
            });
        }
    }
    alterados.sort_by(|a, b| a.arquivo.arquivo.cmp(&b.arquivo.arquivo));
    alterados.dedup_by(|a, b| a.arquivo.arquivo == b.arquivo.arquivo);
    // O arquivo pelo caminho de hoje e pelo de antes (evidência num renomeado).
    let mut por_caminho: HashMap<&str, usize> = HashMap::new();
    for (i, a) in alterados.iter().enumerate() {
        por_caminho.insert(&a.arquivo.arquivo, i);
        if let Some(anterior) = &a.arquivo.anterior {
            por_caminho.insert(anterior, i);
        }
    }

    // Quem cita os arquivos alterados.
    let mut problemas_da_base = Vec::new();
    let regras = ler_regras(&ambiente.kb, &mut problemas_da_base)
        .map_err(|e| ErroFerramenta::Interno(e.to_string()))?;
    problemas.extend(
        problemas_da_base
            .into_iter()
            .map(|p| format!("{}: {}", p.documento, p.erro)),
    );
    let mapa = MapaFuncionalidades::carregar(&ambiente.kb).unwrap_or_else(|e| {
        problemas.push(format!("mapa de funcionalidades ilegível: {e}"));
        MapaFuncionalidades::default()
    });
    let funcionalidades: Vec<_> = mapa.abertas().collect();

    let mut citacoes: Vec<(usize, Linhas, String, Dono)> = Vec::new();
    for (i, (_, regra)) in regras.iter().enumerate() {
        for e in &regra.frontmatter.evidencias {
            if let Some(&k) = por_caminho.get(e.arquivo.as_str()) {
                citacoes.push((k, e.linhas, e.hash.clone(), Dono::Regra(i)));
            }
        }
    }
    for (i, a) in analises.iter().enumerate() {
        let trechos = evidencias_da_analise(a)
            .map(|(o, e)| (o, e.arquivo.as_str(), e.linhas, e.hash.clone()))
            .chain(
                evidencias_dos_fundamentos(a)
                    .into_iter()
                    .map(|(o, e)| (o, e.arquivo.as_str(), e.linhas, e.hash.clone())),
            );
        for (origem, arquivo, linhas, hash) in trechos {
            if let Some(&k) = por_caminho.get(arquivo) {
                citacoes.push((k, linhas, hash, Dono::Analise(i, origem)));
            }
        }
    }
    for (i, f) in funcionalidades.iter().enumerate() {
        for p in &f.pontos {
            let e = &p.evidencia;
            if let Some(&k) = por_caminho.get(e.arquivo.as_str()) {
                citacoes.push((k, e.linhas, e.hash.clone(), Dono::Funcionalidade(i)));
            }
        }
    }

    // As duas versões de cada arquivo citado, lidas uma vez.
    let mut versoes: HashMap<usize, (Option<String>, Option<String>)> = HashMap::new();
    let mut regras_afetadas: BTreeMap<usize, Vec<TrechoAfetado>> = BTreeMap::new();
    let mut termos: BTreeMap<(String, String), TermoAfetado> = BTreeMap::new();
    let mut funcs: BTreeMap<usize, Classe> = BTreeMap::new();
    let mut itens: BTreeMap<usize, Vec<ItemAfetado>> = BTreeMap::new();
    let mut citantes: BTreeMap<usize, Citantes> = BTreeMap::new();
    for (k, linhas, hash, dono) in citacoes {
        let alt = &alterados[k];
        let (antigo, novo) = versoes.entry(k).or_insert_with(|| {
            let r = &repositorios[alt.repositorio];
            let antigo = match alt.arquivo.situacao {
                SituacaoArquivo::Adicionado => None,
                _ => r.antigo(&alt.antigo),
            };
            let novo = match alt.arquivo.situacao {
                SituacaoArquivo::Removido => None,
                _ => r.novo(&alt.novo),
            };
            (antigo, novo)
        });
        let (classe, linhas_novas, diff) =
            analisar_trecho(antigo.as_deref(), novo.as_deref(), linhas, &hash);
        let c = citantes.entry(k).or_default();
        match dono {
            Dono::Regra(i) => {
                c.0.insert(regras[i].1.frontmatter.id.to_string());
                regras_afetadas.entry(i).or_default().push(TrechoAfetado {
                    arquivo: alt.arquivo.arquivo.clone(),
                    linhas,
                    linhas_novas,
                    classe,
                    diff,
                });
            }
            Dono::Analise(i, origem) => {
                let a = &analises[i];
                if !c.3.contains(&a.id) {
                    c.3.push(a.id.clone());
                }
                if let Origem::Termo { numero, .. } = &origem
                    && let Some(t) = a
                        .termos
                        .iter()
                        .find(|t| t.numero == *numero && t.estado == EstadoTermo::Aceito)
                {
                    c.1.insert(t.termo.clone());
                    let entrada =
                        termos
                            .entry((t.termo.clone(), a.id.to_string()))
                            .or_insert(TermoAfetado {
                                termo: t.termo.clone(),
                                analise: a.id.clone(),
                                classe,
                            });
                    entrada.classe = entrada.classe.max(classe);
                }
                itens.entry(i).or_default().push(ItemAfetado {
                    origem,
                    arquivo: alt.arquivo.arquivo.clone(),
                    linhas,
                    classe,
                });
            }
            Dono::Funcionalidade(i) => {
                c.2.insert(funcionalidades[i].id.to_string());
                let entrada = funcs.entry(i).or_insert(classe);
                *entrada = (*entrada).max(classe);
            }
        }
    }

    let mut regras_saida: Vec<RegraAfetada> = regras_afetadas
        .into_iter()
        .map(|(i, trechos)| {
            let (documento, regra) = &regras[i];
            let fm = &regra.frontmatter;
            RegraAfetada {
                id: fm.id.to_string(),
                titulo: fm.titulo.clone(),
                area: fm.area.clone(),
                status: fm.status,
                documento: documento.clone(),
                classe: trechos
                    .iter()
                    .map(|t| t.classe)
                    .max()
                    .unwrap_or(Classe::Inalterado),
                trechos,
            }
        })
        .collect();
    regras_saida.sort_by(|a, b| b.classe.cmp(&a.classe).then(a.id.cmp(&b.id)));

    let com_evidencias = citantes
        .iter()
        .map(|(k, (r, t, f, a))| ArquivoComEvidencias {
            arquivo: alterados[*k].arquivo.clone(),
            regras: r.iter().cloned().collect(),
            termos: t.iter().cloned().collect(),
            funcionalidades: f.iter().cloned().collect(),
            analises: a.clone(),
        })
        .collect();

    // Arquivos sem evidência, pela área.
    let nomes_das_areas: HashMap<String, String> = crate::mapear::areas_do_projeto(&ambiente.kb)
        .into_iter()
        .map(|a| (a.id, a.nome))
        .collect();
    let mut area_do_arquivo: HashMap<String, (String, String)> = HashMap::new();
    let mut areas_da_pasta: HashMap<String, BTreeMap<(String, String), usize>> = HashMap::new();
    for f in &funcionalidades {
        for p in &f.pontos {
            let arquivo = p.evidencia.arquivo.as_str();
            area_do_arquivo
                .entry(arquivo.to_owned())
                .or_insert_with(|| (f.area.clone(), f.area_nome.clone()));
            *areas_da_pasta
                .entry(pasta_de(arquivo).to_owned())
                .or_default()
                .entry((f.area.clone(), f.area_nome.clone()))
                .or_default() += 1;
        }
    }
    for (_, regra) in &regras {
        let fm = &regra.frontmatter;
        let nome = nomes_das_areas
            .get(&fm.area)
            .cloned()
            .unwrap_or_else(|| fm.area.clone());
        for e in &fm.evidencias {
            *areas_da_pasta
                .entry(pasta_de(e.arquivo.as_str()).to_owned())
                .or_default()
                .entry((fm.area.clone(), nome.clone()))
                .or_default() += 1;
        }
    }
    let mut grupos: BTreeMap<(bool, String), GrupoSemEvidencia> = BTreeMap::new();
    for (k, alt) in alterados.iter().enumerate() {
        if citantes.contains_key(&k) {
            continue;
        }
        let arquivo = alt.arquivo.arquivo.as_str();
        let area = area_do_arquivo.get(arquivo).cloned().or_else(|| {
            areas_da_pasta.get(pasta_de(arquivo)).and_then(|votos| {
                votos
                    .iter()
                    .max_by(|(a, x), (b, y)| x.cmp(y).then(b.cmp(a)))
                    .map(|(area, _)| area.clone())
            })
        });
        let (chave, grupo) = match area {
            Some((id, nome)) => (
                (false, nome.clone()),
                GrupoSemEvidencia {
                    area: Some(id),
                    nome,
                    arquivos: Vec::new(),
                },
            ),
            None => {
                let pasta = match pasta_de(arquivo) {
                    "" => "(raiz)".to_owned(),
                    p => p.to_owned(),
                };
                (
                    (true, pasta.clone()),
                    GrupoSemEvidencia {
                        area: None,
                        nome: pasta,
                        arquivos: Vec::new(),
                    },
                )
            }
        };
        grupos
            .entry(chave)
            .or_insert(grupo)
            .arquivos
            .push(alt.arquivo.clone());
    }

    let relatorio = RelatorioDeMudancas {
        projeto: ambiente.kb.nome().to_owned(),
        de: Ponto {
            referencia: de,
            commit: repositorios.first().map(|r| r.de.clone()),
            analise: analise_de,
        },
        ate: Ponto {
            referencia: ate_pedido.unwrap_or_else(|| "cópia de trabalho".to_owned()),
            commit: repositorios.first().and_then(|r| r.ate.clone()),
            analise: None,
        },
        commits,
        arquivos_alterados: alterados.len(),
        com_evidencias,
        regras: regras_saida,
        termos: termos.into_values().collect(),
        funcionalidades: funcs
            .into_iter()
            .map(|(i, classe)| {
                let f = funcionalidades[i];
                FuncionalidadeAfetada {
                    id: f.id.to_string(),
                    nome: f.nome.clone(),
                    area: f.area.clone(),
                    area_nome: f.area_nome.clone(),
                    classe,
                }
            })
            .collect(),
        analises: itens
            .into_iter()
            .map(|(i, itens)| AnaliseAfetada {
                id: analises[i].id.clone(),
                titulo: analises[i].titulo.clone(),
                estado: analises[i].estado,
                itens,
            })
            .collect(),
        sem_evidencias: grupos.into_values().collect(),
        problemas,
        duracao_ms: u64::try_from(inicio.elapsed().as_millis()).unwrap_or(u64::MAX),
    };
    tracing::info!(
        alterados = relatorio.arquivos_alterados,
        com_evidencias = relatorio.com_evidencias.len(),
        regras = relatorio.regras.len(),
        duracao_ms = relatorio.duracao_ms,
        "relatório do que mudou no código"
    );
    Ok(relatorio)
}

// ---------------------------------------------------------------------------
// Nota de mudanças

fn curto(commit: Option<&str>) -> String {
    commit.map_or_else(String::new, |c| format!(" (`{}`)", &c[..c.len().min(8)]))
}

/// O relatório como nota de mudanças em Markdown, para o negócio: as regras cujo
/// código mudou (com o diff e, se houver, a explicação proposta), as que só
/// mudaram de forma, o que mais foi afetado e as possíveis regras novas.
pub fn nota_de_mudancas(r: &RelatorioDeMudancas, propostas: &[PropostaDeRegra]) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "# O que mudou no código · {}\n", r.projeto);
    let de = match &r.de.analise {
        Some(a) => format!(
            "a última análise aprovada, {} ({}){}",
            a.id,
            a.titulo,
            curto(r.de.commit.as_deref())
        ),
        None => format!("`{}`{}", r.de.referencia, curto(r.de.commit.as_deref())),
    };
    let ate = match &r.ate.commit {
        Some(c) => format!("`{}`{}", r.ate.referencia, curto(Some(c))),
        None => r.ate.referencia.clone(),
    };
    let _ = writeln!(s, "- **Desde:** {de}");
    let _ = writeln!(s, "- **Até:** {ate}");
    let _ = writeln!(
        s,
        "- **Arquivos alterados:** {} ({} citados pela base)",
        r.arquivos_alterados,
        r.com_evidencias.len()
    );
    let _ = writeln!(s, "- **Commits no intervalo:** {}\n", r.commits.len());

    let pendente = |id: &str| {
        propostas
            .iter()
            .rev()
            .find(|p| p.regra.to_string() == id && p.situacao != SituacaoProposta::Descartada)
    };
    let mudaram: Vec<&RegraAfetada> = r.regras_que_mudaram().collect();
    let _ = writeln!(s, "## Regras com mudança no código\n");
    if mudaram.is_empty() {
        let _ = writeln!(
            s,
            "Nenhuma regra documentada teve o trecho citado alterado.\n"
        );
    }
    for regra in &mudaram {
        let _ = writeln!(s, "### {} · {}\n", regra.id, regra.titulo);
        if let Some(p) = pendente(&regra.id) {
            let situacao = match p.situacao {
                SituacaoProposta::Pendente => "proposta, aguardando o analista",
                SituacaoProposta::Aplicada => "confirmada pelo analista",
                SituacaoProposta::Descartada => "descartada",
            };
            let _ = writeln!(s, "**Explicação ({situacao}):** {}\n", p.explicacao.trim());
            if let Some(t) = &p.enunciado {
                let _ = writeln!(s, "**Texto atualizado:** {}\n", t.trim());
            }
        }
        for t in &regra.trechos {
            let onde = match t.linhas_novas {
                Some(n) if n != t.linhas => format!("linhas {} (agora {n})", t.linhas),
                _ => format!("linhas {}", t.linhas),
            };
            let _ = writeln!(s, "- `{}`, {onde}: {}", t.arquivo, t.classe.rotulo());
            if !t.diff.is_empty() {
                let _ = writeln!(s, "\n```diff\n{}```\n", t.diff);
            }
        }
        s.push('\n');
    }

    let forma: Vec<&RegraAfetada> = r
        .regras
        .iter()
        .filter(|x| !x.classe.mudou() && x.classe != Classe::Inalterado)
        .collect();
    if !forma.is_empty() {
        let _ = writeln!(s, "## Regras com mudança só de forma\n");
        for regra in forma {
            let _ = writeln!(
                s,
                "- {} · {}: {}",
                regra.id,
                regra.titulo,
                regra.classe.rotulo()
            );
        }
        s.push('\n');
    }
    let intactas: Vec<&RegraAfetada> = r
        .regras
        .iter()
        .filter(|x| x.classe == Classe::Inalterado)
        .collect();
    if !intactas.is_empty() {
        let _ = writeln!(s, "## Regras em arquivos alterados, com o trecho intacto\n");
        for regra in intactas {
            let _ = writeln!(s, "- {} · {}", regra.id, regra.titulo);
        }
        s.push('\n');
    }

    let afetados = |c: Classe| c != Classe::Inalterado;
    let termos: Vec<_> = r.termos.iter().filter(|t| afetados(t.classe)).collect();
    let funcs: Vec<_> = r
        .funcionalidades
        .iter()
        .filter(|f| afetados(f.classe))
        .collect();
    let analises: Vec<_> = r
        .analises
        .iter()
        .filter(|a| a.itens.iter().any(|i| afetados(i.classe)))
        .collect();
    if !termos.is_empty() || !funcs.is_empty() || !analises.is_empty() {
        let _ = writeln!(s, "## Também afetados\n");
        for t in termos {
            let _ = writeln!(s, "- Termo **{}**: {}", t.termo, t.classe.rotulo());
        }
        for f in funcs {
            let _ = writeln!(
                s,
                "- Funcionalidade {} · {} ({}): {}",
                f.id,
                f.nome,
                f.area_nome,
                f.classe.rotulo()
            );
        }
        for a in analises {
            let n = a.itens.iter().filter(|i| afetados(i.classe)).count();
            let _ = writeln!(
                s,
                "- Análise {} · {}: {n} itens citam trechos que mudaram",
                a.id, a.titulo
            );
        }
        s.push('\n');
    }

    let _ = writeln!(s, "## Arquivos alterados sem evidência na base\n");
    if r.sem_evidencias.is_empty() {
        let _ = writeln!(s, "Nenhum.\n");
    } else {
        let _ = writeln!(
            s,
            "Possíveis regras novas, ainda não documentadas, por área (ou pela pasta, sem área conhecida).\n"
        );
        for g in &r.sem_evidencias {
            let titulo = if g.area.is_some() {
                format!("Área {}", g.nome)
            } else {
                format!("Pasta {}", g.nome)
            };
            let _ = writeln!(s, "### {titulo}\n");
            for a in &g.arquivos {
                let _ = writeln!(s, "- `{}` ({})", a.arquivo, a.situacao.rotulo());
            }
            s.push('\n');
        }
    }
    s
}

// ---------------------------------------------------------------------------
// Propostas de atualização de regra

/// Situação de uma proposta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SituacaoProposta {
    Pendente,
    Aplicada,
    Descartada,
}

/// Proposta do agente para uma regra afetada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PropostaDeRegra {
    pub numero: u32,
    pub regra: Id,
    /// O agente concluiu que o comportamento mudou.
    pub comportamento_mudou: bool,
    pub explicacao: String,
    /// Título novo, quando muda.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub titulo: Option<String>,
    /// Enunciado novo (quando o comportamento mudou).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enunciado: Option<String>,
    /// Evidências conferidas contra o código atual.
    pub evidencias: Vec<Evidencia>,
    pub situacao: SituacaoProposta,
    pub criada_ms: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Propostas {
    versao: u32,
    #[serde(default)]
    propostas: Vec<PropostaDeRegra>,
}

fn pasta_do_projeto(ambiente: &Ambiente) -> PathBuf {
    ambiente.dados.projetos().join(&ambiente.projeto.id)
}

/// Lê, altera e grava as propostas sob a trava do arquivo.
fn alterar_propostas<T>(
    ambiente: &Ambiente,
    f: impl FnOnce(&mut Vec<PropostaDeRegra>) -> Result<T, ErroFerramenta>,
) -> Result<T, ErroFerramenta> {
    let pasta = pasta_do_projeto(ambiente);
    let falha = |e: std::io::Error| ErroFerramenta::Interno(format!("propostas de mudança: {e}"));
    fs::create_dir_all(&pasta).map_err(falha)?;
    let trava: File = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(pasta.join(TRAVA_DAS_PROPOSTAS))
        .map_err(falha)?;
    trava.lock().map_err(falha)?;
    let caminho = pasta.join(ARQUIVO_DAS_PROPOSTAS);
    let mut lidas = ler_propostas_de(&caminho);
    let saida = f(&mut lidas.propostas)?;
    lidas.versao = VERSAO_DAS_PROPOSTAS;
    let json =
        serde_json::to_vec_pretty(&lidas).map_err(|e| ErroFerramenta::Interno(e.to_string()))?;
    cba_kb::arquivo::gravar(&caminho, &json).map_err(falha)?;
    Ok(saida)
}

fn ler_propostas_de(caminho: &Path) -> Propostas {
    fs::read(caminho)
        .ok()
        .and_then(|b| serde_json::from_slice::<Propostas>(&b).ok())
        .filter(|p| p.versao == VERSAO_DAS_PROPOSTAS)
        .unwrap_or_default()
}

/// As propostas do projeto, da mais antiga para a mais recente.
pub fn propostas(ambiente: &Ambiente) -> Vec<PropostaDeRegra> {
    ler_propostas_de(&pasta_do_projeto(ambiente).join(ARQUIVO_DAS_PROPOSTAS)).propostas
}

/// O que o agente propõe para uma regra.
#[derive(Debug, Clone, Deserialize)]
pub struct NovaProposta {
    pub regra: String,
    pub comportamento_mudou: bool,
    pub explicacao: String,
    #[serde(default)]
    pub titulo: Option<String>,
    #[serde(default)]
    pub enunciado: Option<String>,
    pub evidencias: Vec<EvidenciaCitada>,
}

/// Registra a proposta, com as evidências conferidas contra o código atual. Uma
/// proposta pendente da mesma regra é substituída (o número fica).
pub fn propor(ambiente: &Ambiente, nova: NovaProposta) -> Result<PropostaDeRegra, ErroFerramenta> {
    let id = Id::analisar(nova.regra.trim())
        .map_err(|e| ErroFerramenta::Argumentos(format!("`regra`: {e}; use RN-<SIGLA>-NNN")))?;
    let regras = ler_regras(&ambiente.kb, &mut Vec::new())
        .map_err(|e| ErroFerramenta::Interno(e.to_string()))?;
    if !regras.iter().any(|(_, r)| r.frontmatter.id == id) {
        return Err(ErroFerramenta::Recusa(format!(
            "a regra {id} não existe na base (ou está obsoleta); use um id de \
             `relatorio_de_mudancas`"
        )));
    }
    let limpo = |v: Option<String>| v.map(|t| t.trim().to_owned()).filter(|t| !t.is_empty());
    let (titulo, enunciado) = (limpo(nova.titulo), limpo(nova.enunciado));
    let explicacao = nova.explicacao.trim().to_owned();
    if explicacao.is_empty() {
        return Err(ErroFerramenta::Argumentos(
            "`explicacao` vazia: diga o que mudou no código e o efeito para o negócio".into(),
        ));
    }
    if nova.comportamento_mudou && enunciado.is_none() {
        return Err(ErroFerramenta::Argumentos(
            "o comportamento mudou: informe em `enunciado` o texto atualizado da regra".into(),
        ));
    }
    let evidencias: Vec<Evidencia> = validar_todas(&ambiente.escopo, &nova.evidencias)
        .map_err(ErroFerramenta::Recusa)?
        .into_iter()
        .map(|e| Evidencia {
            arquivo: e.arquivo,
            linhas: e.linhas,
            hash: e.hash,
            commit: e.commit,
        })
        .collect();
    alterar_propostas(ambiente, |lista| {
        let numero = match lista
            .iter()
            .position(|p| p.regra == id && p.situacao == SituacaoProposta::Pendente)
        {
            Some(i) => lista.remove(i).numero,
            None => lista.iter().map(|p| p.numero).max().unwrap_or(0) + 1,
        };
        let proposta = PropostaDeRegra {
            numero,
            regra: id.clone(),
            comportamento_mudou: nova.comportamento_mudou,
            explicacao,
            titulo,
            enunciado,
            evidencias,
            situacao: SituacaoProposta::Pendente,
            criada_ms: cba_core::tempo::agora_ms(),
        };
        lista.push(proposta.clone());
        Ok(proposta)
    })
}

/// O texto do analista no lugar do proposto.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct AjusteDaProposta {
    #[serde(default)]
    pub titulo: Option<String>,
    #[serde(default)]
    pub enunciado: Option<String>,
}

/// Regra atualizada pela confirmação.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RegraAtualizada {
    pub id: String,
    pub documento: CaminhoRelativo,
    /// Versões anteriores guardadas no histórico.
    pub versoes_anteriores: usize,
    pub avisos: Vec<String>,
}

/// Enunciado de uma regra montada: o texto da seção `## Enunciado`, sem as
/// chamadas de nota de rodapé. Sem a seção (template do analista), o corpo todo.
pub fn enunciado_da_regra(corpo: &str) -> String {
    let mut dentro = false;
    let mut achou = false;
    let mut linhas = Vec::new();
    for l in corpo.lines() {
        if let Some(titulo) = l.strip_prefix("## ") {
            if dentro {
                break;
            }
            dentro = titulo.trim().eq_ignore_ascii_case("enunciado");
            achou |= dentro;
            continue;
        }
        if dentro {
            linhas.push(l);
        }
    }
    let texto = if achou {
        linhas.join("\n")
    } else {
        corpo.to_owned()
    };
    sem_notas(texto.trim())
}

/// Tira as chamadas `[^n]` do texto.
fn sem_notas(texto: &str) -> String {
    let mut s = String::with_capacity(texto.len());
    let mut resto = texto;
    while let Some(i) = resto.find("[^") {
        s.push_str(&resto[..i]);
        match resto[i..].find(']') {
            Some(f) => resto = &resto[i + f + 1..],
            None => {
                resto = &resto[i..];
                break;
            }
        }
    }
    s.push_str(resto);
    s
}

/// Confirma a proposta `numero`: a regra mantém o identificador, ganha o texto
/// (o do ajuste, senão o proposto, senão o que já tinha) e as evidências novas,
/// volta a `confirmada` e guarda a versão anterior no `historico`.
pub fn confirmar_proposta(
    ambiente: &Ambiente,
    numero: u32,
    ajuste: &AjusteDaProposta,
) -> Result<RegraAtualizada, ErroFerramenta> {
    alterar_propostas(ambiente, |lista| {
        let proposta = lista
            .iter_mut()
            .find(|p| p.numero == numero && p.situacao == SituacaoProposta::Pendente)
            .ok_or_else(|| {
                ErroFerramenta::Recusa(format!("a proposta {numero} não está pendente"))
            })?;
        let regras = ler_regras(&ambiente.kb, &mut Vec::new())
            .map_err(|e| ErroFerramenta::Interno(e.to_string()))?;
        let (caminho, regra) = regras
            .into_iter()
            .find(|(_, r)| r.frontmatter.id == proposta.regra)
            .ok_or_else(|| {
                ErroFerramenta::Recusa(format!(
                    "a regra {} não existe mais na base (ou ficou obsoleta)",
                    proposta.regra
                ))
            })?;
        let limpo = |v: &Option<String>| {
            v.as_deref()
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .map(str::to_owned)
        };
        let anterior = enunciado_da_regra(&regra.corpo);
        let mut fm = regra.frontmatter.clone();
        fm.historico.push(VersaoDaRegra {
            ate: cba_core::tempo::hoje(),
            titulo: fm.titulo.clone(),
            enunciado: anterior.clone(),
            evidencias: fm.evidencias.clone(),
            motivo: Some(proposta.explicacao.clone()),
        });
        if let Some(t) = limpo(&ajuste.titulo).or_else(|| proposta.titulo.clone()) {
            fm.titulo = t;
        }
        let enunciado = limpo(&ajuste.enunciado)
            .or_else(|| proposta.enunciado.clone())
            .unwrap_or(anterior);
        fm.evidencias = proposta.evidencias.clone();
        fm.status = StatusRegra::Confirmada;
        let versoes = fm.historico.len();
        let id = fm.id.to_string();
        let (templates, _) = Templates::da_base(&ambiente.base);
        let mut conteudo = Conteudo::da_regra(fm, enunciado);
        conteudo.caminho = caminho.clone();
        let montado = templates
            .montar(&conteudo)
            .map_err(|e| ErroFerramenta::Interno(format!("regra não montada: {e}")))?;
        ambiente
            .kb
            .gravar(&caminho, &montado.documento)
            .map_err(|e| ErroFerramenta::Interno(e.to_string()))?;
        proposta.situacao = SituacaoProposta::Aplicada;
        Ok(RegraAtualizada {
            id,
            documento: caminho,
            versoes_anteriores: versoes,
            avisos: montado.avisos.iter().map(ToString::to_string).collect(),
        })
    })
}

/// Descarta a proposta `numero` (a regra fica como está).
pub fn descartar_proposta(ambiente: &Ambiente, numero: u32) -> Result<(), ErroFerramenta> {
    alterar_propostas(ambiente, |lista| {
        let p = lista
            .iter_mut()
            .find(|p| p.numero == numero && p.situacao == SituacaoProposta::Pendente)
            .ok_or_else(|| {
                ErroFerramenta::Recusa(format!("a proposta {numero} não está pendente"))
            })?;
        p.situacao = SituacaoProposta::Descartada;
        Ok(())
    })
}

/// O pedido que o app manda ao agente para explicar as mudanças do intervalo.
pub fn pedido_de_explicacao(pedido: &Pedido) -> String {
    let intervalo = match (&pedido.de, &pedido.ate) {
        (Some(d), Some(a)) => format!("entre `{d}` e `{a}`"),
        (Some(d), None) => format!("desde `{d}` até a cópia de trabalho"),
        (None, Some(a)) => format!("desde a última análise aprovada até `{a}`"),
        (None, None) => "desde a última análise aprovada".to_owned(),
    };
    format!(
        "Explique as mudanças no código {intervalo}.\n\n\
         1. Chame `relatorio_de_mudancas`{args} e leia o diff de cada regra afetada.\n\
         2. Para cada regra com o trecho alterado, removido, movido ou reformatado, \
         leia o código atual e chame `propor_atualizacao_de_regra` uma vez: diga se o \
         comportamento de negócio mudou, explique a mudança em linguagem de negócio, \
         cite a nova evidência (arquivo, linhas e trecho do código atual) e, se mudou, \
         proponha o enunciado atualizado. Reformatação ou trecho movido não mudam o \
         comportamento: proponha só a evidência nova.\n\
         3. Não altere as regras nem a base por outro caminho: o analista confirma \
         cada proposta no app.\n\
         4. Termine com um resumo: regras que mudaram de comportamento, as que só \
         mudaram de forma e os arquivos sem evidência que parecem trazer regra nova.",
        args = match (&pedido.de, &pedido.ate) {
            (None, None) => String::new(),
            _ => format!(
                " com {}",
                [
                    pedido.de.as_ref().map(|d| format!("`de: \"{d}\"`")),
                    pedido.ate.as_ref().map(|a| format!("`ate: \"{a}\"`")),
                ]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" e ")
            ),
        }
    )
}

// ---------------------------------------------------------------------------
// Ferramentas MCP

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ArgsRelatorio {
    #[serde(default)]
    de: Option<String>,
    #[serde(default)]
    ate: Option<String>,
    #[serde(default)]
    regra: Option<String>,
}

/// `relatorio_de_mudancas`: o relatório do que mudou, só de leitura.
pub fn relatorio_de_mudancas() -> Ferramenta {
    Ferramenta::nova(
        "relatorio_de_mudancas",
        "Relatório do que mudou no código",
        "Só leitura (#124): o que mudou no código do projeto entre dois pontos do \
         repositório (`de` e `ate`: commit, tag ou branch; sem `de`, desde a última \
         análise aprovada; sem `ate`, até a cópia de trabalho), em termos da base: \
         os arquivos alterados que a base cita, as regras afetadas com o diff do \
         trecho citado e a classe da mudança (alterado, removido, movido, \
         reformatado, inalterado), os termos, as funcionalidades e os itens das \
         análises afetados, e os arquivos alterados sem evidência, por área \
         (possíveis regras novas). Com `regra`, só ela. Para explicar as mudanças, \
         chame depois `propor_atualizacao_de_regra` por regra afetada.",
        json!({
            "type": "object",
            "properties": {
                "de": {"type": "string", "minLength": 1, "maxLength": 200,
                       "description": "Commit, tag ou branch de partida. Sem ele, o commit da última análise aprovada."},
                "ate": {"type": "string", "minLength": 1, "maxLength": 200,
                        "description": "Commit, tag ou branch de chegada. Sem ele, a cópia de trabalho."},
                "regra": {"type": "string", "minLength": 1, "maxLength": 40,
                          "description": "Só esta regra (RN-…)."}
            },
            "additionalProperties": false
        }),
        |ambiente, args| {
            let args: ArgsRelatorio = ler_argumentos(args)?;
            let mut r = relatorio(
                ambiente,
                &Pedido {
                    de: args.de,
                    ate: args.ate,
                },
            )?;
            if let Some(regra) = args.regra.map(|t| t.trim().to_owned()) {
                r.regras.retain(|x| x.id == regra);
                r.com_evidencias.retain(|a| a.regras.contains(&regra));
            }
            let dados = serde_json::to_value(&r).map_err(|e| ErroFerramenta::Interno(e.to_string()))?;
            Ok(Saida::json(dados))
        },
    )
    .somente_leitura()
}

/// `propor_atualizacao_de_regra`: a explicação do agente para uma regra afetada,
/// que o analista confirma no app.
pub fn propor_atualizacao_de_regra() -> Ferramenta {
    Ferramenta::nova(
        "propor_atualizacao_de_regra",
        "Propor a atualização de uma regra",
        "Registra, para uma regra afetada no `relatorio_de_mudancas` (#124), se o \
         comportamento de negócio mudou, a explicação, a evidência no código atual e, \
         quando mudou, o enunciado atualizado (e o título, se mudar). Não altera a \
         regra: a proposta fica pendente até o analista confirmar no app; confirmada, \
         a regra mantém o identificador e guarda a versão anterior no histórico. Uma \
         nova proposta para a mesma regra substitui a pendente.",
        json!({
            "type": "object",
            "properties": {
                "regra": {"type": "string", "minLength": 1, "maxLength": 40,
                          "description": "Regra afetada, como RN-COB-001."},
                "comportamento_mudou": {"type": "boolean",
                          "description": "O comportamento de negócio mudou (não só a forma do código)."},
                "explicacao": {"type": "string", "minLength": 1, "maxLength": 4000,
                          "description": "O que mudou e o efeito para o negócio, em linguagem de negócio."},
                "titulo": {"type": "string", "minLength": 1, "maxLength": 200,
                          "description": "Título novo, só se mudar."},
                "enunciado": {"type": "string", "minLength": 1, "maxLength": 8000,
                          "description": "Enunciado atualizado da regra. Obrigatório quando o comportamento mudou."},
                "evidencias": {"type": "array", "minItems": 1, "maxItems": 20,
                          "items": schema_da_evidencia(),
                          "description": "Onde a regra está no código atual."}
            },
            "required": ["regra", "comportamento_mudou", "explicacao", "evidencias"],
            "additionalProperties": false
        }),
        |ambiente, args| {
            let nova: NovaProposta = ler_argumentos(args)?;
            let p = propor(ambiente, nova)?;
            Ok(Saida::json(json!({
                "proposta": p.numero,
                "regra": p.regra.to_string(),
                "situacao": "pendente",
                "mensagem": "Proposta registrada; o analista confirma ou descarta no app."
            })))
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(texto: &str, i: u32, f: u32) -> String {
        hash_das_linhas(texto, Linhas::novo(i, f).unwrap()).unwrap()
    }

    const ANTIGO: &str = "a\nb\nc\nd\ne\nf\n";

    #[test]
    fn trecho_movido_e_achado_nas_linhas_novas() {
        let novo = "a\nd\ne\nf\nb\nc\n";
        let l = Linhas::novo(2, 3).unwrap();
        let (classe, linhas, diff) =
            analisar_trecho(Some(ANTIGO), Some(novo), l, &hash(ANTIGO, 2, 3));
        assert_eq!(classe, Classe::Movido);
        assert_eq!(linhas, Linhas::novo(5, 6));
        assert!(diff.contains("-b\n"), "{diff}");
    }

    #[test]
    fn trecho_e_arquivo_removidos_e_arquivo_novo() {
        let l = Linhas::novo(2, 3).unwrap();
        let h = hash(ANTIGO, 2, 3);
        let (classe, linhas, diff) = analisar_trecho(Some(ANTIGO), Some("a\nd\ne\nf\n"), l, &h);
        assert_eq!((classe, linhas), (Classe::Removido, None));
        assert!(diff.contains("-b\n-c\n"), "{diff}");
        let (classe, _, diff) = analisar_trecho(Some(ANTIGO), None, l, &h);
        assert_eq!(classe, Classe::Removido);
        assert_eq!(diff, "-b\n-c\n");
        let (classe, linhas, _) = analisar_trecho(None, Some(ANTIGO), l, &h);
        assert_eq!((classe, linhas), (Classe::Novo, Some(l)));
    }

    #[test]
    fn evidencia_registrada_depois_volta_ao_ponto_de_partida() {
        // A evidência cita as linhas 3-4 do texto novo ("b2" e "c"), que no antigo
        // eram as linhas 2-3 ("b" e "c").
        let novo = "a\nx\nb2\nc\nd\ne\nf\n";
        let l = Linhas::novo(3, 4).unwrap();
        let (classe, _, diff) = analisar_trecho(Some(ANTIGO), Some(novo), l, &hash(novo, 3, 4));
        assert_eq!(classe, Classe::Alterado);
        assert!(diff.contains("-b\n") && diff.contains("+b2\n"), "{diff}");
    }

    #[test]
    fn diff_so_dos_blocos_que_tocam_o_trecho() {
        let antigo: String = (1..=40).map(|i| format!("linha {i}\n")).collect();
        let novo = antigo
            .replace("linha 3\n", "linha três\n")
            .replace("linha 30\n", "linha trinta\n");
        let l = Linhas::novo(29, 31).unwrap();
        let (classe, _, diff) =
            analisar_trecho(Some(&antigo), Some(&novo), l, &hash(&antigo, 29, 31));
        assert_eq!(classe, Classe::Alterado);
        assert!(diff.contains("+linha trinta"), "{diff}");
        assert!(!diff.contains("três"), "{diff}");
    }

    #[test]
    fn enunciado_sai_da_secao_sem_as_notas() {
        let corpo = "# RN-COB-001 · Multa\n\n- **Status:** confirmada\n\n## Enunciado\n\n\
                     A multa é de 2%.[^1]\n\nSó depois da carência.\n\n## Evidências\n\n\
                     [^1]: `db/x.sql`, linhas 1-2\n";
        assert_eq!(
            enunciado_da_regra(corpo),
            "A multa é de 2%.\n\nSó depois da carência."
        );
        assert_eq!(enunciado_da_regra("Texto livre"), "Texto livre");
    }

    #[test]
    fn referencias_que_viram_opcao_sao_recusadas() {
        assert!(referencia_valida("v1.2"));
        assert!(referencia_valida("abc123~2"));
        assert!(!referencia_valida("--output=/tmp/x"));
        assert!(!referencia_valida("v1 v2"));
        assert!(!referencia_valida(""));
    }

    #[test]
    fn pedido_de_explicacao_cita_as_ferramentas_e_o_intervalo() {
        let p = pedido_de_explicacao(&Pedido {
            de: Some("v1".into()),
            ate: None,
        });
        assert!(p.contains("desde `v1`"), "{p}");
        assert!(
            p.contains("`relatorio_de_mudancas` com `de: \"v1\"`"),
            "{p}"
        );
        assert!(p.contains("`propor_atualizacao_de_regra`"), "{p}");
        assert!(pedido_de_explicacao(&Pedido::default()).contains("última análise aprovada"));
    }
}
