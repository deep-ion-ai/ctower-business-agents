//! `mudancas_em_andamento` (#441): os PRs e MRs abertos que alteram os arquivos
//! da análise (por padrão, os citados nas evidências), para o agente registrar o
//! risco de dependência ou de conflito.
//!
//! Cada arquivo vai para o repositório de código que o contém
//! ([`cba_repos::codigo::repositorio_do_arquivo`]); de cada repositório saem os
//! abertos mais recentes (até `limite`) e os arquivos de cada um
//! ([`cba_repos::andamento`]), com o token do resolvedor só no ambiente da CLI e
//! um cache curto por sessão. Sem repositório cadastrado, sem rede ou sem token,
//! a resposta traz o aviso e a análise segue.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::sync::{Arc, OnceLock};

use cba_core::ambiente::Ambiente;
use cba_repos::RepositorioCodigo;
use cba_repos::andamento::{Cache, LIMITE_PADRAO, POR_PAGINA, chaves_de_demanda};
use cba_repos::codigo::repositorio_do_arquivo;
use cba_repos::demanda::Ferramentas;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::comum;
use crate::registro::{Ferramenta, Saida, ler_argumentos};

/// Arquivos consultados numa chamada, no máximo.
const MAXIMO_DE_ARQUIVOS: usize = 200;

#[derive(Debug, Deserialize)]
struct Args {
    analise: String,
    #[serde(default)]
    arquivos: Vec<String>,
    #[serde(default)]
    limite: Option<usize>,
}

fn schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "analise": comum::schema_da_analise(),
            "arquivos": {
                "type": "array",
                "maxItems": MAXIMO_DE_ARQUIVOS,
                "items": { "type": "string", "minLength": 1 },
                "description": "Arquivos (relativos à raiz do projeto) a conferir, como os da correção. Sem eles, os citados nas evidências da análise."
            },
            "limite": {
                "type": "integer",
                "minimum": 1,
                "maximum": 100,
                "description": "PRs/MRs abertos mais recentes lidos por repositório (padrão 50)."
            }
        },
        "required": ["analise"],
        "additionalProperties": false
    })
}

/// PR/MR aberto que altera arquivos da análise.
#[derive(Debug, Clone, Serialize)]
struct Encontrada {
    repositorio: String,
    numero: String,
    titulo: String,
    branch: String,
    autor: String,
    data: String,
    url: String,
    /// Chaves de demanda no título e na branch.
    chaves: Vec<String>,
    /// Arquivos da análise que ele altera (relativos ao projeto).
    arquivos: Vec<String>,
}

fn cache_da_sessao() -> Arc<Cache> {
    static CACHE: OnceLock<Arc<Cache>> = OnceLock::new();
    CACHE.get_or_init(|| Arc::new(Cache::default())).clone()
}

/// `mudancas_em_andamento` com as CLIs, o chaveiro e o cache da sessão.
pub fn mudancas_em_andamento() -> Ferramenta {
    mudancas_em_andamento_com(Ferramentas::default(), cache_da_sessao())
}

/// `mudancas_em_andamento` com as CLIs e o cache informados (os testes usam CLIs
/// falsas).
pub fn mudancas_em_andamento_com(f: Ferramentas, cache: Arc<Cache>) -> Ferramenta {
    Ferramenta::nova(
        "mudancas_em_andamento",
        "Mudanças em andamento nos arquivos",
        "Lista os PRs (GitHub) e MRs (GitLab) abertos que alteram os arquivos informados \
         (por padrão, os citados nas evidências da análise): número, título, branch, \
         autor, data, chaves de demanda no título e na branch, link e os arquivos em \
         comum. Use antes de concluir uma análise de impacto, com os arquivos da \
         correção: um PR aberto no mesmo arquivo é risco de dependência ou de conflito \
         (registre em `riscos`, com número, título e link) e pode já cobrir a demanda. \
         Sem rede ou sem acesso ao repositório, a resposta traz o aviso; siga a análise.",
        schema(),
        move |ambiente, args| {
            let args: Args = ler_argumentos(args)?;
            let id = comum::id_da_analise(&args.analise)?;
            let analise = ambiente.analises.carregar(&id)?;
            let arquivos: BTreeSet<String> = if args.arquivos.is_empty() {
                analise
                    .afirmacoes
                    .iter()
                    .flat_map(|a| &a.evidencias)
                    .map(|e| e.arquivo.as_str().to_string())
                    .collect()
            } else {
                args.arquivos
                    .iter()
                    .map(|a| a.trim().trim_start_matches("./").replace('\\', "/"))
                    .collect()
            };
            let limite = args.limite.unwrap_or(LIMITE_PADRAO).clamp(1, 100);
            let (encontradas, avisos, consultados) =
                consultar(ambiente, &arquivos, limite, &f, &cache);
            tracing::info!(
                analise = %id,
                arquivos = arquivos.len(),
                encontradas = encontradas.len(),
                "mudanças em andamento consultadas"
            );
            let dados = json!({
                "analise": id,
                "arquivos": arquivos,
                "repositorios_consultados": consultados,
                "mudancas": encontradas,
                "avisos": avisos,
            });
            Ok(Saida {
                texto: markdown(&encontradas, &avisos, arquivos.len()),
                dados: Some(dados),
            })
        },
    )
    .somente_leitura()
}

fn consultar(
    ambiente: &Ambiente,
    arquivos: &BTreeSet<String>,
    limite: usize,
    f: &Ferramentas,
    cache: &Cache,
) -> (Vec<Encontrada>, Vec<String>, Vec<String>) {
    let mut avisos = Vec::new();
    if arquivos.is_empty() {
        avisos.push(
            "nenhum arquivo para conferir: a análise ainda não tem evidências; informe \
             `arquivos`"
                .into(),
        );
        return (Vec::new(), avisos, Vec::new());
    }
    let repos: Vec<RepositorioCodigo> = ambiente
        .projeto
        .repositorios
        .codigo
        .iter()
        .cloned()
        .map(|mut r| {
            r.pasta = r.pasta.map(|p| dunce::canonicalize(&p).unwrap_or(p));
            r
        })
        .collect();
    if repos.is_empty() {
        avisos.push(
            "o projeto não tem repositório de código cadastrado (GitHub ou GitLab): as \
             mudanças em andamento não foram consultadas; siga a análise"
                .into(),
        );
        return (Vec::new(), avisos, Vec::new());
    }
    // Arquivo do projeto → caminho no repositório, por repositório.
    let mut por_repo: BTreeMap<String, (&RepositorioCodigo, BTreeMap<String, String>)> =
        BTreeMap::new();
    let mut fora = Vec::new();
    for a in arquivos {
        let absoluto = ambiente
            .escopo
            .resolver(a)
            .ok()
            .map(|p| dunce::canonicalize(&p).unwrap_or(p));
        match repositorio_do_arquivo(&repos, absoluto.as_deref(), a) {
            Some((r, no_repo)) => {
                por_repo
                    .entry(r.chave())
                    .or_insert_with(|| (r, BTreeMap::new()))
                    .1
                    .insert(no_repo, a.clone());
            }
            None => fora.push(a.clone()),
        }
    }
    if !fora.is_empty() {
        avisos.push(format!(
            "{} arquivo(s) fora dos repositórios cadastrados, não conferidos: {}",
            fora.len(),
            fora.join(", ")
        ));
    }
    let tracker = ambiente.projeto.repositorios.tracker.as_ref();
    let mut encontradas = Vec::new();
    let mut consultados = Vec::new();
    for (repo, alvo) in por_repo.values() {
        let rotulo = repo.rotulo();
        let abertas = match cache.abertas(repo, limite, POR_PAGINA, f) {
            Ok(a) => a,
            Err(e) => {
                avisos.push(format!(
                    "mudanças em andamento de {rotulo} não consultadas: {e}; siga a análise \
                     e diga ao analista que este repositório não foi conferido"
                ));
                continue;
            }
        };
        consultados.push(rotulo.clone());
        if abertas.cortada {
            avisos.push(format!(
                "{rotulo}: só os {limite} PRs/MRs abertos mais recentes foram conferidos; há \
                 mais abertos"
            ));
        }
        let mut falhas = 0;
        for m in &abertas.mudancas {
            let alterados = match cache.arquivos(repo, m, 100, f) {
                Ok(a) => a,
                Err(_) => {
                    falhas += 1;
                    continue;
                }
            };
            if alterados.cortada {
                avisos.push(format!(
                    "{rotulo} {}: a forja não devolveu a lista inteira de arquivos",
                    m.numero
                ));
            }
            let comuns: Vec<String> = alterados
                .arquivos
                .iter()
                .filter_map(|a| alvo.get(a).cloned())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            if comuns.is_empty() {
                continue;
            }
            encontradas.push(Encontrada {
                repositorio: rotulo.clone(),
                numero: m.numero.clone(),
                titulo: m.titulo.clone(),
                branch: m.branch.clone(),
                autor: m.autor.clone(),
                data: m.data.clone(),
                url: m.url.clone(),
                chaves: chaves_de_demanda(&m.titulo, &m.branch, tracker),
                arquivos: comuns,
            });
        }
        if falhas > 0 {
            avisos.push(format!(
                "{rotulo}: os arquivos de {falhas} PR(s)/MR(s) não puderam ser lidos"
            ));
        }
    }
    (encontradas, avisos, consultados)
}

fn markdown(encontradas: &[Encontrada], avisos: &[String], arquivos: usize) -> String {
    let mut t = String::new();
    if encontradas.is_empty() {
        let _ = writeln!(
            t,
            "Nenhum PR/MR aberto altera os {arquivos} arquivo(s) conferido(s)."
        );
    } else {
        let _ = writeln!(
            t,
            "# Mudanças em andamento nos arquivos da análise\n\nRegistre cada uma em \
             `riscos` (dependência ou conflito, com número, título e link) e confira se \
             ela já cobre a demanda.\n"
        );
        for e in encontradas {
            let _ = writeln!(
                t,
                "- {} {} \"{}\" (branch `{}`, de {}, em {}): {}",
                e.repositorio, e.numero, e.titulo, e.branch, e.autor, e.data, e.url
            );
            if !e.chaves.is_empty() {
                let _ = writeln!(t, "  - Demandas citadas: {}", e.chaves.join(", "));
            }
            let _ = writeln!(t, "  - Arquivos em comum: {}", e.arquivos.join(", "));
        }
    }
    for a in avisos {
        let _ = writeln!(t, "\nAviso: {a}");
    }
    t.trim_end().to_string()
}
