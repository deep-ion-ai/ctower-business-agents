//! Configurações › Conexões (#437): as conexões externas (GitHub, GitLab, Jira,
//! bancos) cadastradas uma vez e referenciadas pelos projetos pelo `id`.
//!
//! A lista fica em `<dados>/config/conexoes.json`, na configuração do app e
//! nunca na base do projeto. A credencial não: o cadastro guarda só a fonte
//! (conta do chaveiro, comando, arquivo, CLI), e o valor guardado pelo app vai
//! direto para o chaveiro do sistema. Nada aqui devolve uma credencial para a
//! tela.
//!
//! O formulário da tela é montado a partir dos descritores
//! ([`cba_repos::providers::ResumoDescritor`]), e o "Testar" roda os passos de
//! prontidão do descritor ([`cba_repos::providers::prontidao::testar`]).

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::str::FromStr;

use cba_core::dados::DiretorioDados;
use cba_index::Cadastro;
use cba_repos::chaveiro::Chaveiro;
use cba_repos::demanda::Ferramentas;
use cba_repos::prontidao::Integracao;
use cba_repos::providers::{
    ArquivoConexoes, Capacidade, Catalogo, Conexao, FonteCredencial, Resolvedor, ResumoDescritor,
    host_de, todos,
};
use cba_repos::{
    ForgeKind, OrigemToken, RepositorioCodigo, Repositorios, Secret, TrackerConfig, TrackerKind,
};
use serde::{Deserialize, Serialize};

const ARQUIVO: &str = "conexoes.json";
const TESTES: &str = "conexoes_testes.json";

/// Arquivo das conexões, na configuração do app.
pub fn arquivo(dados: &DiretorioDados) -> PathBuf {
    dados.config().join(ARQUIVO)
}

fn arquivo_testes(dados: &DiretorioDados) -> PathBuf {
    dados.config().join(TESTES)
}

/// As conexões cadastradas.
pub fn conexoes(dados: &DiretorioDados) -> Result<Vec<Conexao>, String> {
    Ok(ArquivoConexoes::ler(&arquivo(dados))?.conexoes)
}

/// Aponta o catálogo do processo para o arquivo de conexões (abertura da
/// janela e do servidor MCP).
pub fn carregar(dados: &DiretorioDados) -> Result<(), String> {
    Catalogo::global().usar_arquivo(arquivo(dados))
}

fn gravar(dados: &DiretorioDados, lista: &[Conexao]) -> Result<(), String> {
    todos().validar_lista(lista)?;
    let json = serde_json::to_vec_pretty(&ArquivoConexoes {
        conexoes: lista.to_vec(),
    })
    .map_err(|e| e.to_string())?;
    cba_kb::arquivo::gravar(&arquivo(dados), &json)
        .map_err(|e| format!("as conexões não foram gravadas: {e}"))
}

/// Resultado do último "Testar" de uma conexão.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UltimoTeste {
    pub pronta: bool,
    pub quando_ms: u64,
    /// O passo que falhou, ou o detalhe do último passo.
    pub resumo: String,
}

fn ler_testes(dados: &DiretorioDados) -> BTreeMap<String, UltimoTeste> {
    std::fs::read(arquivo_testes(dados))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn gravar_testes(dados: &DiretorioDados, t: &BTreeMap<String, UltimoTeste>) {
    if let Ok(json) = serde_json::to_vec_pretty(t)
        && let Err(e) = cba_kb::arquivo::gravar(&arquivo_testes(dados), &json)
    {
        tracing::warn!(erro = %e, "resultado do teste da conexão não gravado");
    }
}

/// Onde um projeto usa uma conexão.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsoConexao {
    pub projeto: String,
    pub nome_projeto: String,
    /// `issues`, `código g/p` ou `documentação`.
    pub onde: String,
}

/// As conexões referenciadas por um cadastro de repositórios, com onde.
pub fn referencias(r: &Repositorios) -> Vec<(String, String)> {
    let mut v = Vec::new();
    for repo in &r.codigo {
        if !repo.conexao.trim().is_empty() {
            v.push((
                repo.conexao.trim().to_string(),
                format!("código {}", repo.caminho.trim()),
            ));
        }
    }
    let rastreador = match &r.tracker {
        Some(TrackerConfig::Github(g)) => g.conexao.as_str(),
        Some(TrackerConfig::Gitlab(g)) => g.conexao.as_str(),
        Some(TrackerConfig::Jira(j)) => j.conexao.as_str(),
        None => "",
    };
    if !rastreador.trim().is_empty() {
        v.push((rastreador.trim().to_string(), "issues".into()));
    }
    if let Some(repo) = r.docs.as_ref().and_then(|d| d.repositorio_proprio())
        && !repo.conexao.trim().is_empty()
    {
        v.push((repo.conexao.trim().to_string(), "documentação".into()));
    }
    v
}

/// Quem usa cada conexão, pelo `id`.
pub fn usos(dados: &DiretorioDados) -> Result<BTreeMap<String, Vec<UsoConexao>>, String> {
    let mut m: BTreeMap<String, Vec<UsoConexao>> = BTreeMap::new();
    for p in Cadastro::em(dados.projetos())
        .listar()
        .map_err(|e| e.to_string())?
    {
        for (id, onde) in referencias(&p.repositorios) {
            m.entry(id).or_default().push(UsoConexao {
                projeto: p.id.clone(),
                nome_projeto: p.nome.clone(),
                onde,
            });
        }
        for id in &p.bancos {
            m.entry(id.clone()).or_default().push(UsoConexao {
                projeto: p.id.clone(),
                nome_projeto: p.nome.clone(),
                onde: "banco de dados".into(),
            });
        }
    }
    Ok(m)
}

/// Uma conexão como a tela mostra: o cadastro (sem credencial), o tipo, se há
/// credencial guardada no chaveiro, os projetos que a usam e o último teste.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConexaoVista {
    pub conexao: Conexao,
    pub rotulo_tipo: String,
    pub icone: String,
    /// Com a fonte `Chaveiro`: se a conta tem valor guardado. `None` nas outras.
    pub credencial_guardada: Option<bool>,
    pub usos: Vec<UsoConexao>,
    pub ultimo_teste: Option<UltimoTeste>,
}

/// As conexões cadastradas, como a tela mostra.
pub fn listar(
    dados: &DiretorioDados,
    chaveiro: &dyn Chaveiro,
) -> Result<Vec<ConexaoVista>, String> {
    let mut usos = usos(dados)?;
    let testes = ler_testes(dados);
    Ok(conexoes(dados)?
        .into_iter()
        .map(|c| {
            let d = todos().get(&c.tipo);
            ConexaoVista {
                rotulo_tipo: d.map_or_else(|| c.tipo.clone(), |d| d.rotulo().to_string()),
                icone: d.map_or("link", |d| d.icone()).to_string(),
                credencial_guardada: match &c.credencial {
                    FonteCredencial::Chaveiro { conta } => {
                        Some(matches!(chaveiro.ler(conta), Ok(Some(_))))
                    }
                    _ => None,
                },
                usos: usos.remove(&c.id).unwrap_or_default(),
                ultimo_teste: testes.get(&c.id).cloned(),
                conexao: c,
            }
        })
        .collect())
}

/// Conta do chaveiro de uma conexão nova com a fonte `Chaveiro`.
pub fn conta_da_conexao(id: &str) -> String {
    format!("conexao:{id}")
}

fn id_novo(nome: &str, lista: &[Conexao]) -> String {
    let base = cba_index::slug(nome);
    let base = if base.is_empty() {
        "conexao".into()
    } else {
        base
    };
    let mut id = base.clone();
    let mut n = 2;
    while lista.iter().any(|c| c.id == id) {
        id = format!("{base}-{n}");
        n += 1;
    }
    id
}

fn em_lista(usos: &[UsoConexao]) -> String {
    let mut nomes: Vec<&str> = usos.iter().map(|u| u.nome_projeto.as_str()).collect();
    nomes.sort_unstable();
    nomes.dedup();
    nomes.join(", ")
}

/// Cria (sem `id`) ou edita uma conexão. O endereço sai normalizado pelo
/// descritor; a fonte `Chaveiro` sem conta ganha a conta da conexão. O tipo e o
/// endereço de uma conexão em uso não mudam: os projetos guardam o endereço
/// dela.
pub fn salvar(dados: &DiretorioDados, mut nova: Conexao) -> Result<Conexao, String> {
    let mut lista = conexoes(dados)?;
    let d = todos().de(&nova)?;
    nova.nome = nova.nome.trim().to_string();
    nova.endereco = d.normalizar_endereco(&nova.endereco)?;
    nova.opcoes.retain(|_, v| !v.trim().is_empty());
    if nova.id.trim().is_empty() {
        nova.id = id_novo(&nova.nome, &lista);
    }
    if let FonteCredencial::Chaveiro { conta } = &mut nova.credencial
        && conta.trim().is_empty()
    {
        *conta = conta_da_conexao(&nova.id);
    }
    match lista.iter_mut().find(|c| c.id == nova.id) {
        Some(atual) => {
            if atual.tipo != nova.tipo || atual.endereco != nova.endereco {
                let usos = usos(dados)?.remove(&nova.id).unwrap_or_default();
                if !usos.is_empty() {
                    return Err(format!(
                        "a conexão \"{}\" é usada por {}: o tipo e o endereço dela não mudam \
                         enquanto ela estiver em uso (cadastre uma conexão nova e troque nos \
                         projetos)",
                        atual.nome,
                        em_lista(&usos)
                    ));
                }
            }
            *atual = nova.clone();
        }
        None => lista.push(nova.clone()),
    }
    gravar(dados, &lista)?;
    Ok(nova)
}

/// Remove a conexão. Recusa quando algum projeto a usa e diz quais.
pub fn remover(dados: &DiretorioDados, id: &str, chaveiro: &dyn Chaveiro) -> Result<(), String> {
    let mut lista = conexoes(dados)?;
    let Some(i) = lista.iter().position(|c| c.id == id) else {
        return Err(format!("a conexão \"{id}\" não está cadastrada"));
    };
    let usos = usos(dados)?.remove(id).unwrap_or_default();
    if !usos.is_empty() {
        return Err(format!(
            "a conexão \"{}\" não pode ser removida: ela é usada por {}. Troque a conexão nesses \
             projetos antes",
            lista[i].nome,
            em_lista(&usos)
        ));
    }
    let removida = lista.remove(i);
    gravar(dados, &lista)?;
    // O token guardado pelo app para esta conexão sai junto; contas antigas
    // (`host:`) podem ser usadas por cadastros ainda não migrados e ficam.
    if let FonteCredencial::Chaveiro { conta } = &removida.credencial
        && *conta == conta_da_conexao(id)
        && let Err(e) = chaveiro.apagar(conta)
    {
        tracing::warn!(erro = %e, "token da conexão removida não apagado do chaveiro");
    }
    let mut testes = ler_testes(dados);
    if testes.remove(id).is_some() {
        gravar_testes(dados, &testes);
    }
    Ok(())
}

/// Guarda no chaveiro a credencial de uma conexão com a fonte `Chaveiro`. O
/// valor vai direto para o chaveiro e nunca volta para a tela.
pub fn guardar_credencial(
    dados: &DiretorioDados,
    id: &str,
    valor: &str,
    chaveiro: &dyn Chaveiro,
) -> Result<(), String> {
    let c = conexoes(dados)?
        .into_iter()
        .find(|c| c.id == id)
        .ok_or_else(|| format!("a conexão \"{id}\" não está cadastrada"))?;
    let FonteCredencial::Chaveiro { conta } = &c.credencial else {
        return Err("a credencial desta conexão não fica no chaveiro do app".into());
    };
    let valor = valor.trim();
    if valor.is_empty() {
        chaveiro.apagar(conta)?;
    } else {
        chaveiro.gravar(conta, &Secret::new(valor))?;
    }
    Resolvedor::global().esquecer_tudo();
    Ok(())
}

/// O "Testar" da conexão: cada passo da prontidão do descritor, com a
/// orientação de quem falhou. Guarda o resumo para a lista.
pub fn testar(dados: &DiretorioDados, id: &str, f: &Ferramentas) -> Result<Integracao, String> {
    let c = conexoes(dados)?
        .into_iter()
        .find(|c| c.id == id)
        .ok_or_else(|| format!("a conexão \"{id}\" não está cadastrada"))?;
    // Os bancos rodam os passos de verdade pelo driver (#440); os demais tipos,
    // pelo descritor.
    let i = if cba_banco::prontidao::e_de_banco(&c) {
        cba_banco::prontidao::prontidao(&c, f.chaveiro.as_ref())
    } else {
        cba_repos::providers::prontidao::testar(&c, f)
    };
    let falha = i
        .passos
        .iter()
        .find(|p| p.situacao == cba_repos::prontidao::Situacao::Falhou);
    let resumo = match falha {
        Some(p) => format!("{}: {}", p.rotulo, p.detalhe.clone().unwrap_or_default()),
        None => i
            .passos
            .iter()
            .rev()
            .find_map(|p| p.detalhe.clone())
            .unwrap_or_else(|| "pronta".into()),
    };
    let mut testes = ler_testes(dados);
    testes.insert(
        id.to_string(),
        UltimoTeste {
            pronta: i.pronta(),
            quando_ms: cba_core::tempo::agora_ms(),
            resumo,
        },
    );
    gravar_testes(dados, &testes);
    Ok(i)
}

/// Põe nos cadastros de repositórios que referenciam conexões o que vem delas
/// (forja e host, URL do Jira, arquivo do jira-cli) e tira a credencial antiga
/// (comando de credencial), que deixa de valer. Recusa a conexão que não existe
/// ou que não serve para o lugar (um banco como rastreador).
pub fn aplicar_conexoes(r: &mut Repositorios, lista: &[Conexao]) -> Result<(), String> {
    let achar = |id: &str, capacidade: Capacidade, onde: &str| -> Result<Conexao, String> {
        let c = lista
            .iter()
            .find(|c| c.id == id.trim())
            .cloned()
            .ok_or_else(|| {
                format!(
                    "{onde}: a conexão \"{}\" não está cadastrada em Configurações › Conexões",
                    id.trim()
                )
            })?;
        let d = todos().de(&c)?;
        if !d.tem(capacidade) {
            return Err(format!(
                "{onde}: a conexão \"{}\" ({}) não serve para isto",
                c.nome,
                d.rotulo()
            ));
        }
        Ok(c)
    };
    let repo = |repo: &mut RepositorioCodigo, onde: &str| -> Result<(), String> {
        if repo.conexao.trim().is_empty() {
            return Ok(());
        }
        let c = achar(&repo.conexao, Capacidade::Forja, onde)?;
        repo.conexao = c.id.clone();
        repo.forja = ForgeKind::from_str(&c.tipo).map_err(|e| format!("{onde}: {e}"))?;
        repo.host = host_de(&c.endereco);
        repo.comando_credencial.clear();
        if repo.token == OrigemToken::Comando {
            repo.token = OrigemToken::Global;
        }
        Ok(())
    };
    for (i, r) in r.codigo.iter_mut().enumerate() {
        repo(r, &format!("repositório de código {}", i + 1))?;
    }
    if let Some(cba_repos::docs::DestinoDocs::Repositorio { repositorio, .. }) = &mut r.docs {
        repo(repositorio, "repositório da documentação")?;
    }
    if let Some(t) = &mut r.tracker {
        let id = match t {
            TrackerConfig::Github(g) => g.conexao.clone(),
            TrackerConfig::Gitlab(g) => g.conexao.clone(),
            TrackerConfig::Jira(j) => j.conexao.clone(),
        };
        if !id.trim().is_empty() {
            let c = achar(&id, Capacidade::Rastreador, "repositório de issues")?;
            let kind = TrackerKind::from_str(&c.tipo).map_err(|e| e.to_string())?;
            if kind != t.kind() {
                return Err(format!(
                    "repositório de issues: a conexão \"{}\" é de outro tipo",
                    c.nome
                ));
            }
            let d = todos().de(&c)?;
            match t {
                TrackerConfig::Github(g) => g.conexao = c.id.clone(),
                TrackerConfig::Gitlab(g) => {
                    g.conexao = c.id.clone();
                    g.host = host_de(&c.endereco);
                    g.credential_cmd.clear();
                }
                TrackerConfig::Jira(j) => {
                    j.conexao = c.id.clone();
                    j.url = c.endereco.clone();
                    j.credential_cmd.clear();
                    j.config_file = c.opcao(d, "config_file").unwrap_or("").to_string();
                }
            }
        }
    }
    Ok(())
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

/// Atualiza o catálogo do processo depois de uma gravação.
fn recarregar(dados: &DiretorioDados) {
    match conexoes(dados) {
        Ok(l) => Catalogo::global().definir(l),
        Err(e) => tracing::warn!(erro = %e, "catálogo de conexões não recarregado"),
    }
}

#[tauri::command]
pub fn descritores_conexao_app() -> Vec<ResumoDescritor> {
    todos().resumos()
}

#[tauri::command]
pub async fn listar_conexoes_app() -> Result<Vec<ConexaoVista>, String> {
    let dados = dados()?;
    bloqueante(move || listar(&dados, Ferramentas::default().chaveiro.as_ref())).await
}

#[tauri::command]
pub async fn salvar_conexao_app(conexao: Conexao) -> Result<Conexao, String> {
    let dados = dados()?;
    bloqueante(move || {
        let c = salvar(&dados, conexao)?;
        recarregar(&dados);
        Ok(c)
    })
    .await
}

#[tauri::command]
pub async fn remover_conexao_app(id: String) -> Result<(), String> {
    let dados = dados()?;
    bloqueante(move || {
        remover(&dados, &id, Ferramentas::default().chaveiro.as_ref())?;
        recarregar(&dados);
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn guardar_credencial_conexao_app(id: String, valor: String) -> Result<(), String> {
    let dados = dados()?;
    bloqueante(move || {
        guardar_credencial(
            &dados,
            &id,
            &valor,
            Ferramentas::default().chaveiro.as_ref(),
        )
    })
    .await
}

#[tauri::command]
pub async fn testar_conexao_app(id: String) -> Result<Integracao, String> {
    let dados = dados()?;
    bloqueante(move || testar(&dados, &id, &Ferramentas::default())).await
}

#[cfg(all(test, unix))]
#[path = "conexoes_testes.rs"]
mod tests;
