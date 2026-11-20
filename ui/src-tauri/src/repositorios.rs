//! Repositório de código e de issues de um projeto (#86): cadastro, prontidão e
//! análise a partir de uma issue.
//!
//! O cadastro guarda só o comando de credencial. O token é lido na hora de usar,
//! passa ao `jira`, ao `glab` ou ao `gh` só no ambiente do processo filho e não vai
//! a disco, a log nem à configuração do Claude.

use cba_core::analise::Fluxo;
use cba_core::dados::DiretorioDados;
use cba_index::Cadastro;
use cba_kb::frontmatter::Demanda;
use cba_repos::Repositorios;
use cba_repos::demanda::{Ferramentas, buscar_demanda};
use cba_repos::prontidao::{Prontidao, conferir};

use crate::analise::{AnaliseCriada, criar_analise_com_demanda};

fn cadastrado(dados: &DiretorioDados, projeto: &str) -> Result<cba_index::Projeto, String> {
    Cadastro::em(dados.projetos())
        .obter(projeto)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("o projeto \"{projeto}\" não está cadastrado"))
}

/// Grava os repositórios do projeto. O tipo de cada um e, no GitLab, host e
/// caminho não mudam depois do cadastro; o que ainda não foi cadastrado pode ser
/// acrescentado. O destino da documentação (#274) pode ser trocado ou removido;
/// quando ele muda, o inventário é refeito, porque a pasta da documentação fica
/// fora do que é analisado.
pub fn salvar_repositorios(
    dados: &DiretorioDados,
    projeto: &str,
    mut novos: Repositorios,
) -> Result<Repositorios, String> {
    let mut p = cadastrado(dados, projeto)?;
    novos.docs = novos.docs.as_ref().map(|d| d.normalizado());
    p.repositorios.validar_troca(&novos)?;
    novos.validar()?;
    let docs_mudou = p.repositorios.docs != novos.docs;
    p.repositorios = novos;
    let cadastro = Cadastro::em(dados.projetos());
    if docs_mudou {
        cadastro.ajustar(&p).map_err(|e| e.to_string())?;
    } else {
        cadastro.salvar(&p).map_err(|e| e.to_string())?;
    }
    Ok(p.repositorios)
}

/// Confere as integrações cadastradas, passo a passo.
pub fn prontidao(
    dados: &DiretorioDados,
    projeto: &str,
    ferramentas: &Ferramentas,
) -> Result<Prontidao, String> {
    Ok(conferir(
        &cadastrado(dados, projeto)?.repositorios,
        ferramentas,
    ))
}

/// Repositório git encontrado numa pasta escolhida pelo analista, com a forja
/// deduzida do remoto.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoDetectado {
    pub caminho: std::path::PathBuf,
    pub branch: Option<String>,
    pub remoto: Option<String>,
    pub forja: Option<cba_repos::ForgeKind>,
    pub host: Option<String>,
    pub caminho_repo: Option<String>,
}

/// Repositórios git na pasta: ela mesma, se é um; senão, as subpastas imediatas
/// que são (#178).
pub fn detectar_repositorios(pasta: &std::path::Path) -> Result<Vec<RepoDetectado>, String> {
    if !pasta.is_absolute() {
        return Err(format!(
            "informe o caminho completo da pasta: {}",
            pasta.display()
        ));
    }
    if !pasta.is_dir() {
        return Err(format!("pasta não encontrada: {}", pasta.display()));
    }
    Ok(cba_index::repositorios_em(pasta)
        .into_iter()
        .map(|g| {
            let remoto = cba_index::remotos_git(&g.raiz).into_iter().next();
            let r = remoto
                .as_deref()
                .and_then(cba_repos::RepositorioCodigo::da_url);
            RepoDetectado {
                forja: r.as_ref().map(|r| r.forja),
                host: r.as_ref().map(|r| r.host.clone()),
                caminho_repo: r.map(|r| r.caminho),
                remoto,
                branch: g.branch,
                caminho: g.raiz,
            }
        })
        .collect())
}

/// Cria a análise com título e descrição da issue como entrada e o link dela em
/// `demanda.origem`.
pub fn criar_analise_de_issue(
    dados: &DiretorioDados,
    projeto: &str,
    fluxo: Fluxo,
    referencia: &str,
    ferramentas: &Ferramentas,
) -> Result<AnaliseCriada, String> {
    if fluxo == Fluxo::Reuniao {
        return Err(crate::demandas::FLUXO_REUNIAO.into());
    }
    let p = cadastrado(dados, projeto)?;
    let tracker = p
        .repositorios
        .tracker
        .as_ref()
        .ok_or("o projeto não tem repositório de issues cadastrado")?;
    let d = buscar_demanda(tracker, referencia, ferramentas).map_err(|e| e.to_string())?;
    let entrada = if d.descricao.trim().is_empty() {
        d.titulo.clone()
    } else {
        format!("{}\n\n{}", d.titulo, d.descricao.trim())
    };
    criar_analise_com_demanda(
        dados,
        projeto,
        fluxo,
        &entrada,
        Some(&d.titulo),
        Some(Demanda {
            id: None,
            origem: d.url,
            chave: Some(d.chave),
        }),
    )
}

fn dados() -> Result<DiretorioDados, String> {
    DiretorioDados::padrao().map_err(|e| format!("diretório de dados do app inacessível: {e}"))
}

async fn bloqueante<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| format!("falha interna: {e}"))?
}

#[tauri::command]
pub async fn salvar_repositorios_app(
    projeto: String,
    repositorios: Repositorios,
) -> Result<Repositorios, String> {
    let dados = dados()?;
    bloqueante(move || salvar_repositorios(&dados, &projeto, repositorios)).await
}

#[tauri::command]
pub async fn detectar_repositorios_app(pasta: String) -> Result<Vec<RepoDetectado>, String> {
    bloqueante(move || detectar_repositorios(std::path::Path::new(&pasta))).await
}

#[tauri::command]
pub async fn prontidao_repositorios_app(projeto: String) -> Result<Prontidao, String> {
    let dados = dados()?;
    bloqueante(move || prontidao(&dados, &projeto, &Ferramentas::default())).await
}

#[tauri::command]
pub async fn criar_analise_de_demanda(
    projeto: String,
    fluxo: Fluxo,
    referencia: String,
) -> Result<AnaliseCriada, String> {
    let dados = dados()?;
    bloqueante(move || {
        criar_analise_de_issue(
            &dados,
            &projeto,
            fluxo,
            &referencia,
            &Ferramentas::default(),
        )
    })
    .await
}

#[cfg(all(test, unix))]
#[path = "repositorios_testes.rs"]
mod tests;
