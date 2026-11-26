//! Bancos do projeto e consultas ao banco numa análise (#440).
//!
//! - **Bancos do projeto:** as conexões de banco (Configurações › Conexões) que o
//!   projeto usa, gravadas no cadastro (`projeto.json`, campo `bancos`).
//! - **Banco da análise:** "Usar o banco <nome> nesta análise" liga uma delas na
//!   análise; só então o agente vê `consultar_dados` (o servidor MCP lê o estado da
//!   análise antes de cada chamada).
//! - **Aprovação:** cada consulta pedida pelo agente fica pendente no estado da
//!   análise até o analista decidir aqui: Executar, Editar e executar ou Recusar
//!   (com o motivo, que volta ao agente). Quem decide é o usuário do sistema.

use cba_core::consultas::DecisaoConsulta;
use cba_core::dados::DiretorioDados;
use cba_core::eventos::Mudanca;
use cba_index::{Cadastro, Projeto};
use cba_kb::Id;
use cba_repos::providers::{Capacidade, Conexao, todos};
use serde::Serialize;

/// Uma conexão de banco do projeto, como a tela mostra.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BancoDoProjeto {
    pub id: String,
    pub nome: String,
    pub rotulo_tipo: String,
    pub ambiente: Option<String>,
    /// A conexão ainda está cadastrada.
    pub cadastrada: bool,
}

fn cadastrado(dados: &DiretorioDados, projeto: &str) -> Result<Projeto, String> {
    Cadastro::em(dados.projetos())
        .obter(projeto)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("o projeto \"{projeto}\" não está cadastrado"))
}

fn de_banco(c: &Conexao) -> bool {
    todos()
        .get(&c.tipo)
        .is_some_and(|d| d.tem(Capacidade::Banco))
}

fn vista(id: &str, lista: &[Conexao]) -> BancoDoProjeto {
    match lista.iter().find(|c| c.id == id) {
        Some(c) => {
            let d = todos().get(&c.tipo);
            BancoDoProjeto {
                id: c.id.clone(),
                nome: c.nome.clone(),
                rotulo_tipo: d.map_or_else(|| c.tipo.clone(), |d| d.rotulo().to_owned()),
                ambiente: d.and_then(|d| c.opcao(d, "ambiente")).map(str::to_owned),
                cadastrada: true,
            }
        }
        None => BancoDoProjeto {
            id: id.to_owned(),
            nome: id.to_owned(),
            rotulo_tipo: String::new(),
            ambiente: None,
            cadastrada: false,
        },
    }
}

/// As conexões de banco do projeto.
pub fn bancos(dados: &DiretorioDados, projeto: &str) -> Result<Vec<BancoDoProjeto>, String> {
    let p = cadastrado(dados, projeto)?;
    let lista = crate::conexoes::conexoes(dados)?;
    Ok(p.bancos.iter().map(|id| vista(id, &lista)).collect())
}

/// Grava as conexões de banco do projeto. Recusa a conexão que não existe ou que
/// não é de banco.
pub fn salvar_bancos(
    dados: &DiretorioDados,
    projeto: &str,
    ids: Vec<String>,
) -> Result<Vec<BancoDoProjeto>, String> {
    let mut p = cadastrado(dados, projeto)?;
    let lista = crate::conexoes::conexoes(dados)?;
    let mut bancos: Vec<String> = Vec::new();
    for id in ids {
        let id = id.trim().to_owned();
        let c = lista
            .iter()
            .find(|c| c.id == id)
            .ok_or_else(|| format!("a conexão \"{id}\" não está cadastrada"))?;
        if !de_banco(c) {
            return Err(format!("a conexão \"{}\" não é de banco", c.nome));
        }
        if !bancos.contains(&id) {
            bancos.push(id);
        }
    }
    p.bancos = bancos;
    Cadastro::em(dados.projetos())
        .salvar(&p)
        .map_err(|e| e.to_string())?;
    Ok(p.bancos.iter().map(|id| vista(id, &lista)).collect())
}

/// Quem decide: o usuário do sistema.
pub fn quem() -> Option<String> {
    ["USER", "USERNAME"]
        .iter()
        .find_map(|v| std::env::var(v).ok())
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
}

fn id_da_analise(analise: &str) -> Result<Id, String> {
    Id::analisar(analise.trim()).map_err(|e| e.to_string())
}

/// Liga (`Some`) ou desliga (`None`) o banco na análise. Só aceita uma conexão
/// de banco do projeto.
pub fn ligar_banco(
    dados: &DiretorioDados,
    projeto: &str,
    analise: &str,
    conexao: Option<&str>,
) -> Result<(), String> {
    let ambiente = crate::analise::ambiente(dados, projeto)?;
    let id = id_da_analise(analise)?;
    let nome = match conexao {
        Some(c) => {
            let c = c.trim();
            if !ambiente.projeto.bancos.iter().any(|b| b == c) {
                return Err(format!(
                    "a conexão \"{c}\" não é um banco deste projeto: inclua-a nas \
                     configurações do projeto"
                ));
            }
            let lista = crate::conexoes::conexoes(dados)?;
            Some((c.to_owned(), vista(c, &lista).nome))
        }
        None => None,
    };
    let por = quem();
    ambiente
        .analises
        .alterar(&id, Mudanca::Consulta, |a, agora| match &nome {
            Some((c, n)) => a.ligar_banco(c, n, por.clone(), agora),
            None => {
                a.desligar_banco(agora);
                Ok(())
            }
        })
        .map_err(|e| e.to_string())?;
    tracing::info!(ligado = nome.is_some(), "banco da análise alterado");
    Ok(())
}

/// A decisão do analista sobre uma consulta pendente.
pub fn decidir_consulta(
    dados: &DiretorioDados,
    projeto: &str,
    analise: &str,
    numero: u32,
    decisao: DecisaoConsulta,
) -> Result<(), String> {
    let ambiente = crate::analise::ambiente(dados, projeto)?;
    let id = id_da_analise(analise)?;
    let tipo = match &decisao {
        DecisaoConsulta::Executar => "executar",
        DecisaoConsulta::EditarEExecutar { .. } => "editar_e_executar",
        DecisaoConsulta::Recusar { .. } => "recusar",
    };
    let por = quem();
    ambiente
        .analises
        .alterar(&id, Mudanca::Consulta, |a, agora| {
            a.decidir_consulta(numero, decisao, por, agora)
        })
        .map_err(|e| e.to_string())?;
    tracing::info!(
        consulta = numero,
        decisao = tipo,
        "consulta ao banco decidida"
    );
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

#[tauri::command]
pub async fn bancos_projeto_app(projeto: String) -> Result<Vec<BancoDoProjeto>, String> {
    let dados = dados()?;
    bloqueante(move || bancos(&dados, &projeto)).await
}

#[tauri::command]
pub async fn salvar_bancos_projeto_app(
    projeto: String,
    bancos: Vec<String>,
) -> Result<Vec<BancoDoProjeto>, String> {
    let dados = dados()?;
    bloqueante(move || salvar_bancos(&dados, &projeto, bancos)).await
}

#[tauri::command]
pub async fn ligar_banco_analise_app(
    projeto: String,
    analise: String,
    conexao: Option<String>,
) -> Result<(), String> {
    let dados = dados()?;
    bloqueante(move || ligar_banco(&dados, &projeto, &analise, conexao.as_deref())).await
}

#[tauri::command]
pub async fn decidir_consulta_app(
    projeto: String,
    analise: String,
    numero: u32,
    decisao: DecisaoConsulta,
) -> Result<(), String> {
    let dados = dados()?;
    bloqueante(move || decidir_consulta(&dados, &projeto, &analise, numero, decisao)).await
}

#[cfg(test)]
#[path = "consultas_testes.rs"]
mod tests;
