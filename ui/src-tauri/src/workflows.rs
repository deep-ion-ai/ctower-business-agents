//! Configurações › Workflows (spec `docs/specs/2026-11-25-workflows-nas-analises.md`,
//! seção 3.5; #372): ver o padrão, duplicar para a base ou para todos os projetos,
//! editar a cópia no editor embutido, ver a diferença com o padrão atual, restaurar
//! o padrão e levar para uma cópia a edição feita à mão num workflow instalado.
//!
//! A resolução projeto > global > padrão e a validação ficam em
//! [`cba_tools::workflows`]. Aqui ficam os comandos, a leitura do que está instalado
//! em cada projeto e a reinstalação: depois de salvar, duplicar, restaurar ou levar
//! uma edição, a versão efetiva nova é regravada nas raízes dos projetos onde o
//! workflow já estava instalado ([`Preparador::reinstalar_workflows`]), sem novo
//! consentimento; onde não estava, fica para o preparo.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use cba_core::config::Config;
use cba_core::dados::DiretorioDados;
use cba_index::Cadastro;
use cba_session::preparo::workflows as instalados;
use cba_session::preparo::{Preparador, Situacao};
use cba_tools::workflows::copias::{self, LinhaDiferenca};
use cba_tools::workflows::{
    Copia, Fase, OrigemWorkflow, TIPOS, Validacao, WorkflowEfetivo, nome, padrao, resolver, validar,
};
use serde::{Deserialize, Serialize};

use crate::analise;

/// Prefixo do erro de salvar quando a cópia mudou desde a leitura; a tela reconhece
/// o conflito por ele (o mesmo dos textos editáveis).
pub const CONFLITO: &str = crate::configuracoes::CONFLITO;

/// Nome do tipo para o analista. Um tipo novo em `cba_tools::workflows::TIPOS`
/// ganha o rótulo aqui.
fn rotulo(tipo: &str) -> String {
    match tipo {
        "impacto" => "Analisar impacto".into(),
        "revisar" => "Revisar requisito".into(),
        outro => outro.into(),
    }
}

/// A pasta da base, se escolhida.
fn base(dados: &DiretorioDados) -> Option<PathBuf> {
    Config::ler(dados).ok().and_then(|c| c.base)
}

/// A versão efetiva de cada tipo, para o preparo.
pub(crate) fn efetivos(dados: &DiretorioDados) -> Vec<WorkflowEfetivo> {
    cba_tools::workflows::efetivos(base(dados).as_deref(), &dados.workflows())
}

/// Onde fica a cópia que a tela edita.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Destino {
    /// `<base>/_templates/workflows/<tipo>.js` ([`OrigemWorkflow::Projeto`]).
    Projeto,
    /// `<dados do app>/workflows/<tipo>.js` ([`OrigemWorkflow::Global`]).
    Global,
}

impl Destino {
    fn origem(self) -> OrigemWorkflow {
        match self {
            Self::Projeto => OrigemWorkflow::Projeto,
            Self::Global => OrigemWorkflow::Global,
        }
    }
}

fn tipo_conhecido(tipo: &str) -> Result<&'static str, String> {
    TIPOS
        .iter()
        .find(|t| **t == tipo)
        .copied()
        .ok_or_else(|| format!("\"{tipo}\" não é um tipo de análise com workflow"))
}

fn arquivo_da_copia(
    dados: &DiretorioDados,
    tipo: &str,
    destino: Destino,
) -> Result<PathBuf, String> {
    Ok(match destino {
        Destino::Projeto => copias::arquivo_do_projeto(
            &base(dados).ok_or("a pasta da base de conhecimento ainda não foi escolhida")?,
            tipo,
        ),
        Destino::Global => copias::arquivo_global(&dados.workflows(), tipo),
    })
}

fn ler_copia(dados: &DiretorioDados, tipo: &str, destino: Destino) -> Result<Copia, String> {
    Ok(Copia::ler(
        tipo,
        destino.origem(),
        arquivo_da_copia(dados, tipo, destino)?,
    ))
}

/// Marca da versão de um arquivo: hash do conteúdo, ou `ausente`.
fn versao_do_arquivo(caminho: &Path) -> Result<String, String> {
    match fs::read(caminho) {
        Ok(bytes) => Ok(copias::sha256(&String::from_utf8_lossy(&bytes))),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok("ausente".into()),
        Err(e) => Err(format!("não foi possível ler {}: {e}", caminho.display())),
    }
}

fn gravar(arquivo: &Path, texto: &str) -> Result<(), String> {
    if let Some(pai) = arquivo.parent() {
        fs::create_dir_all(pai)
            .map_err(|e| format!("não foi possível criar {}: {e}", pai.display()))?;
    }
    cba_kb::arquivo::gravar(arquivo, texto.as_bytes())
        .map_err(|e| format!("não foi possível gravar {}: {e}", arquivo.display()))
}

// ---------------------------------------------------------------------------
// Visão geral

/// Um workflow instalado numa raiz de um projeto.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArquivoInstalado {
    pub arquivo: PathBuf,
    /// `ok`, `ausente`, `desatualizado` (outra versão: a próxima reinstalação ou o
    /// preparo regrava), `divergente` (editado à mão) ou `conflito` (não é do app).
    pub situacao: Situacao,
    /// A origem gravada no cabeçalho do arquivo instalado.
    pub origem: Option<OrigemWorkflow>,
}

/// O workflow nos projetos cadastrados.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalacaoProjeto {
    pub projeto: String,
    pub nome: String,
    pub arquivos: Vec<ArquivoInstalado>,
    /// Não foi possível conferir (pasta inacessível, configuração ilegível).
    pub erro: Option<String>,
}

/// Um tipo de análise com workflow, como a tela mostra.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowVista {
    pub tipo: String,
    /// `cba-<tipo>`.
    pub nome: String,
    pub rotulo: String,
    /// De onde vem a versão que as análises usam.
    pub origem_efetiva: OrigemWorkflow,
    /// Fases da versão efetiva.
    pub fases: Vec<Fase>,
    /// A cópia na base; `None` sem base escolhida.
    pub projeto: Option<Copia>,
    pub global: Copia,
    pub instalacoes: Vec<InstalacaoProjeto>,
}

fn instalacoes(
    dados: &DiretorioDados,
    preparador: Option<&Preparador>,
    nome_do_workflow: &str,
) -> Vec<InstalacaoProjeto> {
    let Ok(projetos) = Cadastro::em(dados.projetos()).listar() else {
        return Vec::new();
    };
    projetos
        .into_iter()
        .map(|s| {
            let mut i = InstalacaoProjeto {
                projeto: s.id.clone(),
                nome: s.nome.clone(),
                arquivos: Vec::new(),
                erro: None,
            };
            let verificacao = preparador
                .ok_or_else(|| "o executável do app não foi encontrado".to_string())
                .and_then(|p| {
                    p.verificar(&analise::alvo(dados, &s.id)?)
                        .map_err(|e| e.to_string())
                });
            match verificacao {
                Ok(v) => {
                    i.arquivos = v
                        .workflows
                        .into_iter()
                        .filter(|w| w.nome == nome_do_workflow)
                        .map(|w| ArquivoInstalado {
                            origem: fs::read_to_string(&w.arquivo)
                                .ok()
                                .and_then(|t| instalados::ler_cabecalho(&t))
                                .and_then(|c| c.origem),
                            arquivo: w.arquivo,
                            situacao: w.situacao,
                        })
                        .collect();
                }
                Err(e) => i.erro = Some(e),
            }
            i
        })
        .collect()
}

/// Os tipos com workflow, a origem efetiva, as cópias e o que está instalado em cada
/// projeto.
pub fn listar(
    dados: &DiretorioDados,
    preparador: Option<&Preparador>,
) -> Result<Vec<WorkflowVista>, String> {
    let base = base(dados);
    Ok(TIPOS
        .iter()
        .filter_map(|t| resolver(t, base.as_deref(), &dados.workflows()))
        .map(|r| WorkflowVista {
            nome: nome(&r.tipo),
            rotulo: rotulo(&r.tipo),
            origem_efetiva: r.efetivo.origem,
            fases: validar(&r.tipo, &r.efetivo.conteudo).fases,
            instalacoes: instalacoes(dados, preparador, &nome(&r.tipo)),
            projeto: r.projeto,
            global: r.global,
            tipo: r.tipo,
        })
        .collect())
}

// ---------------------------------------------------------------------------
// Editor

/// O workflow aberto no editor: a cópia (ou o padrão, só leitura).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LeituraWorkflow {
    pub tipo: String,
    pub nome: String,
    pub rotulo: String,
    /// `None`: o padrão do app, só leitura.
    pub destino: Option<Destino>,
    pub arquivo: Option<PathBuf>,
    /// O script da cópia, sem as linhas do app (ou o padrão).
    pub texto: String,
    /// O padrão atual do app.
    pub padrao: String,
    /// A cópia existe (sem ela, o editor mostra o padrão e só oferece duplicar).
    pub existe: bool,
    /// O padrão mudou desde que a cópia foi feita (ou não se sabe de qual partiu).
    pub padrao_mudou: bool,
    /// Marca da versão do arquivo, para salvar ([`CONFLITO`]).
    pub versao: String,
    pub validacao: Validacao,
}

/// Abre o padrão (`destino` `None`) ou a cópia.
pub fn ler(
    dados: &DiretorioDados,
    tipo: &str,
    destino: Option<Destino>,
) -> Result<LeituraWorkflow, String> {
    let tipo = tipo_conhecido(tipo)?;
    let padrao = padrao(tipo).expect("tipo conhecido tem padrão").to_string();
    let (texto, arquivo, existe, padrao_mudou, versao) = match destino {
        None => (padrao.clone(), None, true, false, String::new()),
        Some(d) => {
            let c = ler_copia(dados, tipo, d)?;
            let versao = versao_do_arquivo(&c.arquivo)?;
            let existe = c.existe();
            let texto = if existe { c.script } else { padrao.clone() };
            (
                texto,
                Some(c.arquivo),
                existe,
                existe && c.padrao_mudou,
                versao,
            )
        }
    };
    Ok(LeituraWorkflow {
        nome: nome(tipo),
        rotulo: rotulo(tipo),
        validacao: validar(tipo, &texto),
        tipo: tipo.to_string(),
        destino,
        arquivo,
        texto,
        padrao,
        existe,
        padrao_mudou,
        versao,
    })
}

/// O texto do editor validado (erros, avisos e as fases para a prévia).
pub fn validar_texto(tipo: &str, texto: &str) -> Result<Validacao, String> {
    Ok(validar(tipo_conhecido(tipo)?, &texto.replace("\r\n", "\n")))
}

/// Diferença do padrão atual para `texto`.
pub fn diferenca(tipo: &str, texto: &str) -> Result<Vec<LinhaDiferenca>, String> {
    let tipo = tipo_conhecido(tipo)?;
    Ok(copias::diferenca(
        padrao(tipo).expect("tipo conhecido tem padrão"),
        &texto.replace("\r\n", "\n"),
    ))
}

/// O resultado de uma mudança numa cópia, com a reinstalação nos projetos.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MudancaWorkflow {
    /// Gravado. `false` quando a validação recusou o texto (nada mudou).
    pub salvo: bool,
    pub leitura: LeituraWorkflow,
    /// Arquivos instalados regravados com a versão efetiva nova.
    pub reinstalados: usize,
    /// O que não pôde ser reinstalado, por projeto.
    pub avisos: Vec<String>,
}

/// Regrava a versão efetiva nova onde o workflow já estava instalado.
fn reinstalar(dados: &DiretorioDados, preparador: Option<&Preparador>) -> (usize, Vec<String>) {
    let Some(preparador) = preparador else {
        return (
            0,
            vec![
                "o executável do app não foi encontrado; os workflows instalados \
                         serão atualizados no próximo preparo"
                    .into(),
            ],
        );
    };
    let mut total = 0;
    let mut avisos = Vec::new();
    let Ok(projetos) = Cadastro::em(dados.projetos()).listar() else {
        return (0, vec!["o cadastro dos projetos está ilegível".into()]);
    };
    for s in projetos {
        let feito = analise::alvo(dados, &s.id).and_then(|alvo| {
            preparador
                .reinstalar_workflows(&alvo)
                .map_err(|e| e.to_string())
        });
        match feito {
            Ok(a) => total += a.len(),
            Err(e) => avisos.push(format!("{}: {e}", s.nome)),
        }
    }
    (total, avisos)
}

fn mudanca(
    dados: &DiretorioDados,
    preparador: Option<&Preparador>,
    tipo: &str,
    destino: Destino,
) -> Result<MudancaWorkflow, String> {
    let (reinstalados, avisos) = reinstalar(dados, preparador);
    Ok(MudancaWorkflow {
        salvo: true,
        leitura: ler(dados, tipo, Some(destino))?,
        reinstalados,
        avisos,
    })
}

/// Cria a cópia a partir da versão de baixo: para a base, a cópia para todos os
/// projetos (se válida) ou o padrão; para todos os projetos, o padrão. Nunca
/// sobrescreve uma cópia.
pub fn duplicar(
    dados: &DiretorioDados,
    preparador: Option<&Preparador>,
    tipo: &str,
    destino: Destino,
) -> Result<MudancaWorkflow, String> {
    let tipo = tipo_conhecido(tipo)?;
    let copia = ler_copia(dados, tipo, destino)?;
    if copia.existe() {
        return Err(format!(
            "já existe uma cópia em {}; edite-a ou restaure o padrão antes",
            copia.arquivo.display()
        ));
    }
    let global = ler_copia(dados, tipo, Destino::Global)?;
    let (script, hash) = if destino == Destino::Projeto && global.usavel() {
        (
            global.script,
            global
                .hash_padrao
                .or_else(|| copias::hash_do_padrao(tipo))
                .unwrap_or_default(),
        )
    } else {
        (
            padrao(tipo).expect("tipo conhecido tem padrão").to_string(),
            copias::hash_do_padrao(tipo).unwrap_or_default(),
        )
    };
    gravar(&copia.arquivo, &copias::montar(&script, &hash))?;
    mudanca(dados, preparador, tipo, destino)
}

/// Grava o texto do editor na cópia, se passar na validação. Recusa com
/// [`CONFLITO`] se o arquivo mudou desde a leitura. A cópia guarda o hash do padrão
/// de que partiu; `padrao_revisto` o troca pelo do padrão atual (o analista viu a
/// mudança do padrão e ajustou a cópia).
pub fn salvar(
    dados: &DiretorioDados,
    preparador: Option<&Preparador>,
    tipo: &str,
    destino: Destino,
    texto: &str,
    versao_lida: &str,
    padrao_revisto: bool,
) -> Result<MudancaWorkflow, String> {
    let tipo = tipo_conhecido(tipo)?;
    let copia = ler_copia(dados, tipo, destino)?;
    if versao_do_arquivo(&copia.arquivo)? != versao_lida {
        return Err(format!(
            "{CONFLITO} {} mudou desde que foi aberto (editado fora do app?); \
             recarregue para ver a versão atual",
            copia.arquivo.display()
        ));
    }
    let texto = texto.replace("\r\n", "\n");
    let validacao = validar(tipo, &texto);
    if validacao.tem_erro() {
        let mut leitura = ler(dados, tipo, Some(destino))?;
        leitura.validacao = validacao;
        return Ok(MudancaWorkflow {
            salvo: false,
            leitura,
            reinstalados: 0,
            avisos: Vec::new(),
        });
    }
    let hash = copia
        .hash_padrao
        .filter(|_| !padrao_revisto)
        .or_else(|| copias::hash_do_padrao(tipo))
        .unwrap_or_default();
    gravar(&copia.arquivo, &copias::montar(&texto, &hash))?;
    mudanca(dados, preparador, tipo, destino)
}

/// Apaga a cópia (vale a camada de baixo).
pub fn restaurar(
    dados: &DiretorioDados,
    preparador: Option<&Preparador>,
    tipo: &str,
    destino: Destino,
) -> Result<MudancaWorkflow, String> {
    let tipo = tipo_conhecido(tipo)?;
    let arquivo = arquivo_da_copia(dados, tipo, destino)?;
    if let Err(e) = fs::remove_file(&arquivo)
        && e.kind() != io::ErrorKind::NotFound
    {
        return Err(format!(
            "não foi possível remover {}: {e}",
            arquivo.display()
        ));
    }
    mudanca(dados, preparador, tipo, destino)
}

/// Leva para a cópia de `destino` o workflow instalado em `arquivo` e editado à mão
/// num projeto (spec, seção 3.3: o preparo não sobrescreve a edição e oferece levá-la
/// para uma cópia). A cópia é gravada mesmo com erros de validação (a tela os mostra
/// e a cópia só é usada depois de corrigida); o arquivo instalado volta a ser do app,
/// com a edição agora na cópia, e a reinstalação o põe na versão efetiva.
pub fn levar_edicao(
    dados: &DiretorioDados,
    preparador: Option<&Preparador>,
    projeto: &str,
    tipo: &str,
    arquivo: &Path,
    destino: Destino,
) -> Result<MudancaWorkflow, String> {
    let tipo = tipo_conhecido(tipo)?;
    let alvo = analise::alvo(dados, projeto)?;
    let nome = nome(tipo);
    // Só um workflow instalado deste projeto.
    if !alvo
        .raizes_workflows
        .iter()
        .any(|r| instalados::arquivo_workflow(r, &nome) == arquivo)
    {
        return Err(format!(
            "{} não é o workflow {nome} instalado no projeto",
            arquivo.display()
        ));
    }
    let texto = fs::read_to_string(arquivo)
        .map_err(|e| format!("não foi possível ler {}: {e}", arquivo.display()))?;
    let cabecalho = instalados::ler_cabecalho(&texto)
        .ok_or_else(|| format!("{} não foi gerado pelo app", arquivo.display()))?;
    let corpo = instalados::corpo(&texto)
        .unwrap_or_default()
        .replace("\r\n", "\n");
    let hash = match cabecalho.origem {
        // Instalado do padrão: o hash do cabeçalho é o do padrão de que partiu.
        Some(OrigemWorkflow::Padrao) => cabecalho.hash,
        _ => copias::hash_do_padrao(tipo).unwrap_or_default(),
    };
    let copia = arquivo_da_copia(dados, tipo, destino)?;
    gravar(&copia, &copias::montar(&corpo, &hash))?;
    // O arquivo instalado volta a ser do app (intacto), com a edição na cópia.
    instalados::gravar(
        arquivo,
        &instalados::conteudo_instalado(&WorkflowEfetivo {
            tipo: tipo.to_string(),
            origem: destino.origem(),
            conteudo: corpo,
        }),
    )
    .map_err(|e| e.to_string())?;
    mudanca(dados, preparador, tipo, destino)
}

// ---------------------------------------------------------------------------
// Comandos

fn dados() -> Result<DiretorioDados, String> {
    DiretorioDados::padrao().map_err(|e| format!("diretório de dados do app inacessível: {e}"))
}

fn preparador() -> Option<Preparador> {
    let exe = std::env::current_exe().ok()?;
    Preparador::padrao(exe).ok()
}

async fn bloqueante<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| format!("falha interna: {e}"))?
}

#[tauri::command]
pub async fn workflows_app() -> Result<Vec<WorkflowVista>, String> {
    let dados = dados()?;
    bloqueante(move || listar(&dados, preparador().as_ref())).await
}

#[tauri::command]
pub async fn ler_workflow_app(
    tipo: String,
    destino: Option<Destino>,
) -> Result<LeituraWorkflow, String> {
    let dados = dados()?;
    bloqueante(move || ler(&dados, &tipo, destino)).await
}

#[tauri::command]
pub async fn validar_workflow_app(tipo: String, texto: String) -> Result<Validacao, String> {
    bloqueante(move || validar_texto(&tipo, &texto)).await
}

#[tauri::command]
pub async fn diferenca_workflow_app(
    tipo: String,
    texto: String,
) -> Result<Vec<LinhaDiferenca>, String> {
    bloqueante(move || diferenca(&tipo, &texto)).await
}

#[tauri::command]
pub async fn duplicar_workflow_app(
    tipo: String,
    destino: Destino,
) -> Result<MudancaWorkflow, String> {
    let dados = dados()?;
    bloqueante(move || duplicar(&dados, preparador().as_ref(), &tipo, destino)).await
}

#[tauri::command]
pub async fn salvar_workflow_app(
    tipo: String,
    destino: Destino,
    texto: String,
    versao_lida: String,
    padrao_revisto: bool,
) -> Result<MudancaWorkflow, String> {
    let dados = dados()?;
    bloqueante(move || {
        salvar(
            &dados,
            preparador().as_ref(),
            &tipo,
            destino,
            &texto,
            &versao_lida,
            padrao_revisto,
        )
    })
    .await
}

#[tauri::command]
pub async fn restaurar_workflow_app(
    tipo: String,
    destino: Destino,
) -> Result<MudancaWorkflow, String> {
    let dados = dados()?;
    bloqueante(move || restaurar(&dados, preparador().as_ref(), &tipo, destino)).await
}

#[tauri::command]
pub async fn levar_edicao_workflow_app(
    projeto: String,
    tipo: String,
    arquivo: PathBuf,
    destino: Destino,
) -> Result<MudancaWorkflow, String> {
    let dados = dados()?;
    bloqueante(move || {
        levar_edicao(
            &dados,
            preparador().as_ref(),
            &projeto,
            &tipo,
            &arquivo,
            destino,
        )
    })
    .await
}

#[cfg(test)]
#[path = "workflows_testes.rs"]
mod tests;
