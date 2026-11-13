//! Comandos Tauri da tela de configurações (spec, seção 10; #39).
//!
//! Tudo o que o primeiro uso define pode ser mudado depois. Os passos que já existem
//! no primeiro uso ([`crate::primeiro_uso`]: cliente preferido, avaliação da base,
//! prévia e cadastro de projeto) e no preparo do ambiente ([`crate::analise`],
//! [`crate::alteracoes`]) são reaproveitados; aqui fica o que só as configurações
//! fazem:
//!
//! - **Trocar a pasta da base**, de dois jeitos, sempre com a confirmação da tela:
//!   - *reapontar*: o app passa a usar a outra pasta; os documentos da pasta antiga
//!     ficam onde estão, intactos;
//!   - *mover*: copia a base inteira para uma pasta nova ou vazia, confere a cópia
//!     byte a byte, só então troca a configuração e, por último, apaga a origem. Se a
//!     origem mudou durante a cópia (uma sessão gravou nela), ela não é apagada.
//!
//!   Nos dois casos o índice (`cba_kb::indice`) é reconstruído a partir da base
//!   nova.
//! - **Ajustar as exclusões** de um projeto cadastrado, com a prévia do inventário.
//! - **Desfazer o ambiente** de um projeto: desliga a proteção contra edição (tira só
//!   as regras que o app acrescentou) e tira o servidor MCP e, se for o último
//!   projeto, os hooks. A configuração do Claude volta a ser a de antes.
//! - **Descadastrar** um projeto: desfaz o ambiente como acima e só então apaga o
//!   cadastro. Os documentos do projeto na base ficam.
//! - **Textos editáveis**: templates, roteiros, prompts prontos e as regras de cada
//!   projeto, com a indicação de quando o arquivo da base sobrepõe o padrão embutido.
//!   O app não tem editor próprio: abre o arquivo no editor padrão do sistema operacional
//!   operacional, criando antes a cópia do padrão quando ela ainda não existe.

use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use cba_core::config::Config;
use cba_core::dados::DiretorioDados;
use cba_index::Cadastro;
use cba_kb::indice::{Indice, Sincronizacao};
use cba_kb::{Base, CaminhoRelativo, layout};
use cba_session::preparo::{self, Alvo, ConfigClaude, Preparador, Resultado};
use cba_session::protecao;
use cba_session::sessoes::Sessoes;
use cba_tools::editavel::{Grupo, textos_editaveis, validar, variaveis};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::State;
use tauri_plugin_opener::OpenerExt;

use crate::base::VigiaBase;
use crate::primeiro_uso::{
    self, AvaliacaoBase, ClientePreferido, EstadoPasta, PreviaProjeto, ProjetoCadastrado,
    Restricoes, canonica_aproximada, limpar, pastas_vista, resumo_inventario,
};

// ---------------------------------------------------------------------------
// Visão geral

/// Uma pasta de um projeto cadastrado.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PastaConfig {
    pub caminho: PathBuf,
    pub prefixo: Option<String>,
    /// A pasta ainda existe no disco.
    pub existe: bool,
}

/// Projeto cadastrado, como as configurações mostram.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjetoConfig {
    pub id: String,
    pub nome: String,
    pub pastas: Vec<PastaConfig>,
    pub exclusoes: Vec<String>,
    pub inclusoes: Vec<String>,
    /// Repositório de código e de issues; só o comando de credencial, nunca o token.
    pub repositorios: cba_repos::Repositorios,
}

/// O que a tela de configurações mostra ao abrir.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Configuracoes {
    pub cliente: Option<ClientePreferido>,
    pub base: Option<PathBuf>,
    pub base_sugerida: Option<PathBuf>,
    pub projetos: Vec<ProjetoConfig>,
    /// Arquivos de configuração do Claude Code que o app altera.
    pub config_claude: Option<ConfigClaude>,
}

pub fn configuracoes(
    dados: &DiretorioDados,
    preparador: Option<&Preparador>,
) -> Result<Configuracoes, String> {
    let situacao = primeiro_uso::situacao(dados)?;
    let projetos = Cadastro::em(dados.projetos())
        .listar()
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|s| ProjetoConfig {
            pastas: s
                .pastas
                .iter()
                .map(|p| PastaConfig {
                    existe: p.caminho.is_dir(),
                    caminho: p.caminho.clone(),
                    prefixo: p.prefixo.clone(),
                })
                .collect(),
            id: s.id,
            nome: s.nome,
            exclusoes: s.exclusoes,
            inclusoes: s.inclusoes,
            repositorios: s.repositorios,
        })
        .collect();
    Ok(Configuracoes {
        cliente: situacao.cliente,
        base: situacao.base,
        base_sugerida: situacao.base_sugerida,
        projetos,
        config_claude: preparador.map(|p| p.config.clone()),
    })
}

// ---------------------------------------------------------------------------
// Troca da pasta da base

/// Como trocar a pasta da base.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ModoTroca {
    /// Copia os documentos para a pasta nova, confere e apaga a origem.
    Mover,
    /// Passa a usar a outra pasta; os documentos da atual ficam onde estão.
    Reapontar,
}

/// O que a tela precisa para oferecer a troca.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvaliacaoTroca {
    /// A pasta de destino, com as recusas do primeiro uso.
    pub destino: AvaliacaoBase,
    pub atual: Option<PathBuf>,
    /// Arquivos e tamanho da base atual (o que mover levaria).
    pub arquivos: usize,
    pub bytes: u64,
    /// Por que não dá para mover; `None` se dá.
    pub recusa_mover: Option<String>,
    /// Por que não dá para reapontar; `None` se dá.
    pub recusa_reapontar: Option<String>,
    /// Sessões do Claude ainda abertas (podem gravar na base durante a troca).
    pub sessoes_abertas: usize,
}

/// Resultado da troca.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrocaBase {
    pub modo: ModoTroca,
    pub base: PathBuf,
    pub anterior: Option<PathBuf>,
    /// Arquivos copiados e conferidos (mover).
    pub copiados: usize,
    /// A pasta antiga foi apagada (mover, depois da conferência).
    pub origem_apagada: bool,
    /// Documentos no índice reconstruído (`None` se a reconstrução falhou).
    pub indexados: Option<usize>,
    /// O que o analista precisa saber (origem mantida, índice não reconstruído).
    pub avisos: Vec<String>,
}

/// Retrato de uma árvore de arquivos: caminho relativo → (tamanho, modificação).
#[derive(Debug, Default, PartialEq, Eq)]
struct Arvore {
    pastas: Vec<PathBuf>,
    arquivos: BTreeMap<PathBuf, (u64, Option<SystemTime>)>,
}

impl Arvore {
    fn bytes(&self) -> u64 {
        self.arquivos.values().map(|(t, _)| t).sum()
    }
}

/// Lista a árvore da base. Links simbólicos e itens especiais são recusados: a cópia
/// não saberia reproduzi-los com segurança em outro sistema operacional.
fn arvore(raiz: &Path) -> Result<Arvore, String> {
    let mut saida = Arvore::default();
    let mut pendentes = vec![PathBuf::new()];
    while let Some(rel) = pendentes.pop() {
        let pasta = raiz.join(&rel);
        let entradas = fs::read_dir(&pasta)
            .map_err(|e| format!("não foi possível ler {}: {e}", pasta.display()))?;
        for entrada in entradas {
            let entrada = entrada.map_err(|e| format!("{}: {e}", pasta.display()))?;
            let tipo = entrada
                .file_type()
                .map_err(|e| format!("{}: {e}", entrada.path().display()))?;
            let relativo = rel.join(entrada.file_name());
            if tipo.is_dir() {
                saida.pastas.push(relativo.clone());
                pendentes.push(relativo);
            } else if tipo.is_file() {
                let meta = entrada
                    .metadata()
                    .map_err(|e| format!("{}: {e}", entrada.path().display()))?;
                saida
                    .arquivos
                    .insert(relativo, (meta.len(), meta.modified().ok()));
            } else {
                return Err(format!(
                    "a base tem um link simbólico ou arquivo especial ({}), que o app não move; \
                     use \"reapontar\" ou mova a pasta à mão",
                    relativo.display()
                ));
            }
        }
    }
    saida.pastas.sort();
    Ok(saida)
}

fn ler_ate_encher(r: &mut impl Read, buf: &mut [u8]) -> io::Result<usize> {
    let mut lido = 0;
    while lido < buf.len() {
        match r.read(&mut buf[lido..])? {
            0 => break,
            n => lido += n,
        }
    }
    Ok(lido)
}

/// Os dois arquivos têm o mesmo conteúdo, byte a byte.
fn mesmo_conteudo(a: &Path, b: &Path) -> io::Result<bool> {
    if fs::metadata(a)?.len() != fs::metadata(b)?.len() {
        return Ok(false);
    }
    let (mut fa, mut fb) = (fs::File::open(a)?, fs::File::open(b)?);
    let (mut ba, mut bb) = (vec![0u8; 64 * 1024], vec![0u8; 64 * 1024]);
    loop {
        let n = ler_ate_encher(&mut fa, &mut ba)?;
        let m = ler_ate_encher(&mut fb, &mut bb)?;
        if n != m || ba[..n] != bb[..m] {
            return Ok(false);
        }
        if n == 0 {
            return Ok(true);
        }
    }
}

fn sessoes_abertas(dados: &DiretorioDados) -> usize {
    use cba_core::analise::EstadoSessao;
    Sessoes::do_diretorio(dados)
        .listar()
        .map(|l| {
            l.iter()
                .filter(|s| s.estado != EstadoSessao::Encerrada)
                .count()
        })
        .unwrap_or(0)
}

/// Confere a troca da base para `destino`, sem gravar nada.
pub fn avaliar_troca(
    dados: &DiretorioDados,
    restricoes: &Restricoes,
    destino: &str,
) -> Result<AvaliacaoTroca, String> {
    let avaliacao = primeiro_uso::avaliar_base(dados, restricoes, destino)?;
    let config = Config::ler(dados).map_err(|e| format!("configuração do app ilegível: {e}"))?;
    let atual = config.base.clone();
    let mut recusa_comum = avaliacao.recusa.clone();
    let mut recusa_mover = None;
    let (mut arquivos, mut bytes) = (0, 0);
    match &atual {
        None => recusa_mover = Some("ainda não há base configurada para mover".into()),
        Some(atual) => {
            let origem = canonica_aproximada(atual);
            let destino = &avaliacao.caminho;
            if recusa_comum.is_none() {
                if *destino == origem {
                    recusa_comum = Some("essa já é a pasta da base".into());
                } else if destino.starts_with(&origem) || origem.starts_with(destino) {
                    recusa_comum = Some(
                        "a pasta nova não pode ficar dentro da base atual, nem conter a base atual"
                            .into(),
                    );
                }
            }
            if !origem.is_dir() {
                recusa_mover = Some(format!(
                    "a base atual ({}) não existe mais; use \"reapontar\"",
                    origem.display()
                ));
            } else {
                match arvore(&origem) {
                    Ok(a) => {
                        arquivos = a.arquivos.len();
                        bytes = a.bytes();
                    }
                    Err(e) => recusa_mover = Some(e),
                }
            }
            if recusa_mover.is_none()
                && !matches!(
                    avaliacao.estado,
                    Some(EstadoPasta::Nova) | Some(EstadoPasta::Vazia)
                )
            {
                recusa_mover = Some(
                    "para mover, escolha uma pasta nova ou vazia: o app não mistura a base com \
                     outros arquivos. Para usar uma pasta que já tem arquivos (ou já é uma base), \
                     escolha \"reapontar\""
                        .into(),
                );
            }
        }
    }
    Ok(AvaliacaoTroca {
        recusa_mover: recusa_comum.clone().or(recusa_mover),
        recusa_reapontar: recusa_comum,
        destino: avaliacao,
        atual,
        arquivos,
        bytes,
        sessoes_abertas: sessoes_abertas(dados),
    })
}

/// Desfaz uma cópia que não chegou ao fim: apaga só o que estava no destino por causa
/// dela (o destino era novo ou vazio).
fn limpar_destino(destino: &Path, era_novo: bool) {
    if era_novo {
        let _ = fs::remove_dir_all(destino);
        return;
    }
    if let Ok(entradas) = fs::read_dir(destino) {
        for e in entradas.flatten() {
            let p = e.path();
            let _ = if p.is_dir() {
                fs::remove_dir_all(&p)
            } else {
                fs::remove_file(&p)
            };
        }
    }
}

/// Copia a árvore e confere cada arquivo byte a byte. Em caso de falha, desfaz a cópia.
fn copiar_e_conferir(origem: &Path, destino: &Path, arvore: &Arvore) -> Result<usize, String> {
    let era_novo = !destino.exists();
    let feito = (|| {
        fs::create_dir_all(destino)
            .map_err(|e| format!("não foi possível criar {}: {e}", destino.display()))?;
        for p in &arvore.pastas {
            fs::create_dir_all(destino.join(p))
                .map_err(|e| format!("não foi possível criar {}: {e}", p.display()))?;
        }
        for rel in arvore.arquivos.keys() {
            fs::copy(origem.join(rel), destino.join(rel))
                .map_err(|e| format!("não foi possível copiar {}: {e}", rel.display()))?;
        }
        for rel in arvore.arquivos.keys() {
            let igual = mesmo_conteudo(&origem.join(rel), &destino.join(rel))
                .map_err(|e| format!("não foi possível conferir {}: {e}", rel.display()))?;
            if !igual {
                return Err(format!(
                    "a cópia de {} não confere com o original",
                    rel.display()
                ));
            }
        }
        Ok(arvore.arquivos.len())
    })();
    if feito.is_err() {
        limpar_destino(destino, era_novo);
    }
    feito
}

/// Reconstrói o índice a partir da base configurada. Devolve quantos documentos.
fn reconstruir_indice(dados: &DiretorioDados, base: &Path) -> Result<usize, String> {
    let base = Base::abrir(base).map_err(|e| e.to_string())?;
    let mut indice =
        Indice::abrir(dados.arquivo_do_indice(), &base).map_err(|e| format!("índice: {e}"))?;
    let s: Sincronizacao = indice.reconstruir().map_err(|e| format!("índice: {e}"))?;
    Ok(s.novos + s.alterados + s.inalterados)
}

/// Troca a pasta da base. A confirmação é da tela.
pub fn trocar_base(
    dados: &DiretorioDados,
    restricoes: &Restricoes,
    destino: &str,
    modo: ModoTroca,
) -> Result<TrocaBase, String> {
    let avaliacao = avaliar_troca(dados, restricoes, destino)?;
    let recusa = match modo {
        ModoTroca::Mover => avaliacao.recusa_mover.clone(),
        ModoTroca::Reapontar => avaliacao.recusa_reapontar.clone(),
    };
    if let Some(r) = recusa {
        return Err(r);
    }
    let destino = avaliacao.destino.caminho.clone();
    let anterior = avaliacao.atual.clone();
    let mut avisos = Vec::new();
    let mut copiados = 0;
    let mut origem_apagada = false;
    let mut retrato = None;

    if modo == ModoTroca::Mover {
        let origem = canonica_aproximada(anterior.as_deref().ok_or("sem base atual")?);
        let antes = arvore(&origem)?;
        copiados = copiar_e_conferir(&origem, &destino, &antes)?;
        retrato = Some((origem, antes));
    }

    // Só depois da cópia conferida a configuração muda.
    let gravada = match primeiro_uso::configurar_base(
        dados,
        restricoes,
        destino.to_str().unwrap_or_default(),
    ) {
        Ok(g) => g,
        Err(e) => {
            if modo == ModoTroca::Mover {
                // A configuração continua na origem; a cópia é descartada.
                limpar_destino(
                    &destino,
                    avaliacao.destino.estado == Some(EstadoPasta::Nova),
                );
            }
            return Err(e);
        }
    };

    let indexados = match reconstruir_indice(dados, &gravada) {
        Ok(n) => Some(n),
        Err(e) => {
            avisos.push(format!(
                "o índice não foi reconstruído agora ({e}); ele se refaz sozinho na próxima busca"
            ));
            None
        }
    };

    if let Some((origem, antes)) = retrato {
        // Se algo gravou na origem durante a cópia, ela fica, para nada se perder.
        match arvore(&origem) {
            Ok(depois) if depois == antes => match fs::remove_dir_all(&origem) {
                Ok(()) => origem_apagada = true,
                Err(e) => avisos.push(format!(
                    "os documentos foram copiados e conferidos, mas a pasta antiga ({}) não pôde \
                     ser apagada ({e}); apague-a à mão",
                    origem.display()
                )),
            },
            _ => avisos.push(format!(
                "a pasta antiga ({}) mudou durante a cópia, então não foi apagada. Confira se \
                 algum documento novo ficou só nela antes de apagá-la",
                origem.display()
            )),
        }
    } else if let Some(a) = &anterior {
        avisos.push(format!(
            "os documentos da pasta anterior ({}) continuam lá e não aparecem mais no app",
            a.display()
        ));
    }

    tracing::info!(?modo, copiados, origem_apagada, "pasta da base trocada");
    Ok(TrocaBase {
        modo,
        base: gravada,
        anterior,
        copiados,
        origem_apagada,
        indexados,
        avisos,
    })
}

// ---------------------------------------------------------------------------
// Exclusões de um projeto cadastrado

/// Novas exclusões e reinclusões de um projeto.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AjusteExclusoes {
    pub projeto: String,
    #[serde(default)]
    pub exclusoes: Vec<String>,
    #[serde(default)]
    pub inclusoes: Vec<String>,
}

fn cadastrado(dados: &DiretorioDados, projeto: &str) -> Result<cba_index::Projeto, String> {
    Cadastro::em(dados.projetos())
        .obter(projeto)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("o projeto \"{projeto}\" não está cadastrado"))
}

fn ajustado(
    dados: &DiretorioDados,
    ajuste: &AjusteExclusoes,
) -> Result<cba_index::Projeto, String> {
    let mut projeto = cadastrado(dados, &ajuste.projeto)?;
    projeto.exclusoes = limpar(&ajuste.exclusoes);
    projeto.inclusoes = limpar(&ajuste.inclusoes);
    projeto.validar().map_err(|e| e.to_string())?;
    Ok(projeto)
}

/// O inventário com as exclusões ajustadas, sem gravar.
pub fn previa_exclusoes(
    dados: &DiretorioDados,
    ajuste: &AjusteExclusoes,
) -> Result<PreviaProjeto, String> {
    let projeto = ajustado(dados, ajuste)?;
    let inventario = cba_index::inventariar(&projeto, None).map_err(|e| e.to_string())?;
    Ok(PreviaProjeto {
        pastas: pastas_vista(&inventario),
        inventario: resumo_inventario(&inventario),
        id: projeto.id,
        nome: projeto.nome,
        exclusoes: projeto.exclusoes,
        inclusoes: projeto.inclusoes,
        ja_cadastrado: false,
    })
}

/// Grava as exclusões ajustadas e inventaria de novo.
pub fn salvar_exclusoes(
    dados: &DiretorioDados,
    ajuste: &AjusteExclusoes,
) -> Result<ProjetoCadastrado, String> {
    let projeto = ajustado(dados, ajuste)?;
    let cadastro = Cadastro::em(dados.projetos());
    cadastro.salvar(&projeto).map_err(|e| e.to_string())?;
    let inventario = cadastro
        .inventariar(&projeto.id)
        .map_err(|e| e.to_string())?;
    Ok(ProjetoCadastrado {
        pastas: pastas_vista(&inventario),
        inventario: resumo_inventario(&inventario),
        id: projeto.id,
        nome: projeto.nome,
    })
}

// ---------------------------------------------------------------------------
// Ambiente: desfazer e descadastrar

/// O que desfazer o ambiente fez.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AmbienteDesfeito {
    /// Regras da proteção tiradas do `settings.json`.
    pub protecao_tirada: Vec<String>,
    /// Servidor MCP e hooks tirados.
    pub preparo: Resultado,
    /// Limitações (pasta do projeto que não existe mais).
    pub avisos: Vec<String>,
}

/// Alvo do preparo de um projeto. Se a pasta não existe mais, usa o caminho gravado
/// (a chave fica a da pasta; se ela estava num repositório git, a entrada do
/// repositório pode sobrar e é avisada).
fn alvo_do_cadastrado(projeto: &cba_index::Projeto) -> Result<(Alvo, Option<String>), String> {
    match Alvo::do_projeto(projeto) {
        Ok(a) => Ok((a, None)),
        Err(preparo::Erro::Pasta(_)) => {
            let pasta = projeto
                .pastas
                .iter()
                .find(|p| p.prefixo.is_none())
                .or_else(|| projeto.pastas.first())
                .ok_or("o projeto não tem pastas cadastradas")?;
            let plataforma = cba_session::clientes::Plataforma::atual();
            Ok((
                Alvo {
                    projeto: projeto.id.clone(),
                    diretorio: pasta.caminho.clone(),
                    chave: preparo::chave_do_caminho(&pasta.caminho, plataforma),
                    worktree: false,
                },
                Some(format!(
                    "a pasta {} não existe mais; o servidor MCP foi procurado pelo caminho dela. \
                     Se ela ficava num repositório git, confira a entrada do repositório no {}",
                    pasta.caminho.display(),
                    ".claude.json"
                )),
            ))
        }
        Err(e) => Err(e.to_string()),
    }
}

fn desfazer_do_cadastrado(
    dados: &DiretorioDados,
    preparador: &Preparador,
    projeto: &cba_index::Projeto,
) -> Result<AmbienteDesfeito, String> {
    let protecao_tirada =
        protecao::desligar(dados, preparador, &projeto.id).map_err(|e| e.to_string())?;
    let (alvo, aviso) = alvo_do_cadastrado(projeto)?;
    let preparo = preparador.desfazer(&alvo).map_err(|e| e.to_string())?;
    Ok(AmbienteDesfeito {
        protecao_tirada,
        preparo,
        avisos: aviso.into_iter().collect(),
    })
}

/// Desfaz o ambiente do Claude de um projeto: proteção, servidor MCP e (se for o
/// último projeto) hooks. A confirmação é da tela.
pub fn desfazer_ambiente(
    dados: &DiretorioDados,
    preparador: &Preparador,
    projeto: &str,
) -> Result<AmbienteDesfeito, String> {
    desfazer_do_cadastrado(dados, preparador, &cadastrado(dados, projeto)?)
}

/// Resultado do descadastro.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Descadastro {
    pub ambiente: AmbienteDesfeito,
    /// Pasta do projeto na base, que fica (`None` sem base configurada).
    pub documentos: Option<PathBuf>,
}

/// Descadastra o projeto: desfaz o ambiente (com a proteção) e só então apaga o
/// cadastro, o inventário e o mapa. Se desfazer o ambiente falhar, o cadastro fica,
/// para o analista poder tentar de novo. Os documentos na base ficam.
pub fn descadastrar(
    dados: &DiretorioDados,
    preparador: &Preparador,
    projeto: &str,
) -> Result<Descadastro, String> {
    let cadastrado = cadastrado(dados, projeto)?;
    let ambiente = desfazer_do_cadastrado(dados, preparador, &cadastrado)?;
    Cadastro::em(dados.projetos())
        .remover(&cadastrado.id)
        .map_err(|e| e.to_string())?;
    let documentos = Config::ler(dados)
        .ok()
        .and_then(|c| c.base.map(|b| b.join(&cadastrado.id)))
        .filter(|p| p.is_dir());
    tracing::info!(projeto, "projeto descadastrado");
    Ok(Descadastro {
        ambiente,
        documentos,
    })
}

// ---------------------------------------------------------------------------
// Textos editáveis

/// Situação de um texto editável na base.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EstadoTexto {
    /// Sem arquivo na base: vale o padrão embutido no app.
    Padrao,
    /// O arquivo da base é igual ao padrão.
    IgualAoPadrao,
    /// O arquivo da base foi editado e sobrepõe o padrão.
    Editado,
    /// O arquivo existe mas não pôde ser lido (o app usa o padrão).
    Ilegivel,
}

/// Grupo de um texto editável, para a tela.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GrupoTexto {
    Template,
    Roteiro,
    Prompt,
    RegrasDoProjeto,
}

/// Um texto editável, como a tela mostra.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextoVista {
    pub grupo: GrupoTexto,
    pub id: String,
    pub rotulo: String,
    /// Relativo à raiz da base, com `/` (identifica o texto nos comandos).
    pub arquivo: String,
    pub estado: EstadoTexto,
}

/// Um texto do catálogo: (grupo, id, rótulo, arquivo, padrão).
type ItemCatalogo = (GrupoTexto, String, String, CaminhoRelativo, &'static str);

/// Textos editáveis possíveis.
fn catalogo(dados: &DiretorioDados) -> Result<Vec<ItemCatalogo>, String> {
    let mut lista: Vec<_> = textos_editaveis()
        .into_iter()
        .map(|t| {
            let grupo = match t.grupo {
                Grupo::Template => GrupoTexto::Template,
                Grupo::Roteiro => GrupoTexto::Roteiro,
                Grupo::Prompt => GrupoTexto::Prompt,
            };
            (
                grupo,
                t.id.to_owned(),
                t.rotulo.to_owned(),
                t.arquivo,
                t.padrao,
            )
        })
        .collect();
    for s in Cadastro::em(dados.projetos())
        .listar()
        .map_err(|e| e.to_string())?
    {
        let arquivo =
            CaminhoRelativo::novo(&format!("{}/{}", s.id, layout::ARQUIVO_REGRAS_DO_PROJETO))
                .map_err(|e| e.to_string())?;
        lista.push((
            GrupoTexto::RegrasDoProjeto,
            s.id,
            format!("Regras do projeto {}", s.nome),
            arquivo,
            cba_kb::PADRAO_REGRAS_DO_PROJETO,
        ));
    }
    Ok(lista)
}

fn base_configurada(dados: &DiretorioDados) -> Result<PathBuf, String> {
    Config::ler(dados)
        .map_err(|e| format!("configuração do app ilegível: {e}"))?
        .base
        .ok_or_else(|| "a pasta da base de conhecimento ainda não foi escolhida".into())
}

fn normalizar(texto: &str) -> String {
    texto.replace("\r\n", "\n")
}

fn estado_do_texto(caminho: &Path, padrao: &str) -> EstadoTexto {
    if !caminho.is_file() {
        return EstadoTexto::Padrao;
    }
    match fs::read_to_string(caminho) {
        Ok(t) if normalizar(&t) == normalizar(padrao) => EstadoTexto::IgualAoPadrao,
        Ok(_) => EstadoTexto::Editado,
        Err(_) => EstadoTexto::Ilegivel,
    }
}

/// Os textos editáveis com a situação de cada um na base.
pub fn textos(dados: &DiretorioDados) -> Result<Vec<TextoVista>, String> {
    let base = base_configurada(dados)?;
    Ok(catalogo(dados)?
        .into_iter()
        .map(|(grupo, id, rotulo, arquivo, padrao)| TextoVista {
            estado: estado_do_texto(&arquivo.em(&base), padrao),
            arquivo: arquivo.as_str().to_owned(),
            grupo,
            id,
            rotulo,
        })
        .collect())
}

/// Caminho no disco de um texto editável, criando antes a cópia do padrão se ela
/// ainda não existe (nunca sobrescreve). Só aceita os textos do catálogo.
pub fn preparar_texto(dados: &DiretorioDados, arquivo: &str) -> Result<PathBuf, String> {
    let base = base_configurada(dados)?;
    let (_, _, _, relativo, padrao) = catalogo(dados)?
        .into_iter()
        .find(|(_, _, _, a, _)| a.as_str() == arquivo)
        .ok_or_else(|| format!("{arquivo} não é um texto editável da base"))?;
    let caminho = relativo.em(&base);
    if !caminho.exists() {
        if let Some(pai) = caminho.parent() {
            fs::create_dir_all(pai)
                .map_err(|e| format!("não foi possível criar {}: {e}", pai.display()))?;
        }
        cba_kb::arquivo::gravar(&caminho, padrao.as_bytes())
            .map_err(|e| format!("não foi possível criar {arquivo}: {e}"))?;
    }
    Ok(caminho)
}

/// Prefixo do erro de `salvar_texto_editavel` quando o arquivo mudou desde a
/// leitura; a tela reconhece o conflito por ele.
pub const CONFLITO: &str = "conflito:";

/// Variável ou marcador aceito por um texto, com a descrição para a tela.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VariavelTexto {
    pub nome: String,
    pub descricao: String,
}

/// Aviso de validação de um texto, com a linha quando há uma.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvisoTexto {
    pub linha: Option<usize>,
    pub variavel: String,
    /// O texto não usa a variável (em vez de usar uma que não existe).
    pub ausente: bool,
    pub mensagem: String,
}

/// O texto de um editor: o atual, o padrão e o que ele aceita.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LeituraTexto {
    pub grupo: GrupoTexto,
    pub id: String,
    pub rotulo: String,
    pub arquivo: String,
    /// O da base, ou o padrão se não há cópia (ou ela é ilegível).
    pub texto: String,
    pub padrao: String,
    pub estado: EstadoTexto,
    pub variaveis: Vec<VariavelTexto>,
    /// Marca da versão do arquivo (hash do conteúdo), para `salvar_texto_editavel`.
    pub versao: String,
}

/// Resultado de gravar um texto.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SalvoTexto {
    pub estado: EstadoTexto,
    pub versao: String,
    pub avisos: Vec<AvisoTexto>,
}

fn catalogado(dados: &DiretorioDados, arquivo: &str) -> Result<ItemCatalogo, String> {
    catalogo(dados)?
        .into_iter()
        .find(|(_, _, _, a, _)| a.as_str() == arquivo)
        .ok_or_else(|| format!("{arquivo} não é um texto editável da base"))
}

/// Marca da versão de um arquivo: hash do conteúdo, ou `ausente` se não existe.
fn versao_do_arquivo(caminho: &Path) -> Result<String, String> {
    match fs::read(caminho) {
        Ok(bytes) => Ok(Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok("ausente".into()),
        Err(e) => Err(format!("não foi possível ler {}: {e}", caminho.display())),
    }
}

fn validar_item(item: &ItemCatalogo, texto: &str) -> Vec<AvisoTexto> {
    let (grupo, id, _, arquivo, _) = item;
    let grupo = match grupo {
        GrupoTexto::Template => Grupo::Template,
        GrupoTexto::Roteiro => Grupo::Roteiro,
        GrupoTexto::Prompt => Grupo::Prompt,
        // As regras do projeto são texto livre, sem variáveis.
        GrupoTexto::RegrasDoProjeto => return Vec::new(),
    };
    validar(grupo, id, arquivo, texto)
        .into_iter()
        .map(|p| AvisoTexto {
            linha: p.linha,
            variavel: p.variavel,
            ausente: p.ausente,
            mensagem: p.mensagem,
        })
        .collect()
}

fn ler_item(base: &Path, item: ItemCatalogo) -> Result<LeituraTexto, String> {
    let (grupo, id, rotulo, relativo, padrao) = item;
    let caminho = relativo.em(base);
    let estado = estado_do_texto(&caminho, padrao);
    let texto = match estado {
        EstadoTexto::Padrao | EstadoTexto::Ilegivel => padrao.to_owned(),
        _ => fs::read_to_string(&caminho)
            .map_err(|e| format!("não foi possível ler {}: {e}", caminho.display()))?,
    };
    let variaveis = match grupo {
        GrupoTexto::Template => variaveis(Grupo::Template, &id),
        GrupoTexto::Roteiro => variaveis(Grupo::Roteiro, &id),
        GrupoTexto::Prompt => variaveis(Grupo::Prompt, &id),
        GrupoTexto::RegrasDoProjeto => Vec::new(),
    }
    .into_iter()
    .map(|v| VariavelTexto {
        nome: v.nome,
        descricao: v.descricao,
    })
    .collect();
    Ok(LeituraTexto {
        grupo,
        id,
        rotulo,
        arquivo: relativo.as_str().to_owned(),
        texto,
        padrao: padrao.to_owned(),
        estado,
        variaveis,
        versao: versao_do_arquivo(&caminho)?,
    })
}

/// O texto editável para o editor embutido. Só os textos do catálogo.
pub fn ler_texto(dados: &DiretorioDados, arquivo: &str) -> Result<LeituraTexto, String> {
    let base = base_configurada(dados)?;
    ler_item(&base, catalogado(dados, arquivo)?)
}

/// Os avisos que o agente receberia com este texto, sem gravar nada.
pub fn validar_texto(
    dados: &DiretorioDados,
    arquivo: &str,
    texto: &str,
) -> Result<Vec<AvisoTexto>, String> {
    Ok(validar_item(&catalogado(dados, arquivo)?, texto))
}

/// Grava o texto na base, de forma atômica. Recusa com [`CONFLITO`] se o arquivo
/// mudou desde a leitura (`versao_lida`, de [`LeituraTexto::versao`]).
pub fn salvar_texto(
    dados: &DiretorioDados,
    arquivo: &str,
    texto: &str,
    versao_lida: &str,
) -> Result<SalvoTexto, String> {
    let base = base_configurada(dados)?;
    let item = catalogado(dados, arquivo)?;
    let caminho = item.3.em(&base);
    if versao_do_arquivo(&caminho)? != versao_lida {
        return Err(format!(
            "{CONFLITO} {arquivo} mudou desde que foi aberto (editado fora do app?); \
             recarregue para ver a versão atual"
        ));
    }
    if let Some(pai) = caminho.parent() {
        fs::create_dir_all(pai)
            .map_err(|e| format!("não foi possível criar {}: {e}", pai.display()))?;
    }
    cba_kb::arquivo::gravar(&caminho, texto.as_bytes())
        .map_err(|e| format!("não foi possível gravar {arquivo}: {e}"))?;
    Ok(SalvoTexto {
        estado: estado_do_texto(&caminho, item.4),
        versao: versao_do_arquivo(&caminho)?,
        avisos: validar_item(&item, texto),
    })
}

/// Volta ao texto padrão: remove a cópia da base (vale o padrão do app, inclusive o
/// de versões futuras). As regras do projeto precisam existir, então o padrão é
/// gravado por cima.
pub fn restaurar_texto(dados: &DiretorioDados, arquivo: &str) -> Result<LeituraTexto, String> {
    let base = base_configurada(dados)?;
    let item = catalogado(dados, arquivo)?;
    let caminho = item.3.em(&base);
    if item.0 == GrupoTexto::RegrasDoProjeto {
        cba_kb::arquivo::gravar(&caminho, item.4.as_bytes())
            .map_err(|e| format!("não foi possível gravar {arquivo}: {e}"))?;
    } else if let Err(e) = fs::remove_file(&caminho)
        && e.kind() != io::ErrorKind::NotFound
    {
        return Err(format!("não foi possível remover {arquivo}: {e}"));
    }
    ler_item(&base, item)
}

// ---------------------------------------------------------------------------
// Comandos

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

fn preparador() -> Result<Preparador, String> {
    let exe = std::env::current_exe().map_err(|e| format!("caminho do executável do app: {e}"))?;
    Preparador::padrao(exe).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn configuracoes_app() -> Result<Configuracoes, String> {
    let dados = dados()?;
    bloqueante(move || configuracoes(&dados, preparador().ok().as_ref())).await
}

#[tauri::command]
pub async fn avaliar_troca_base_app(destino: String) -> Result<AvaliacaoTroca, String> {
    let dados = dados()?;
    bloqueante(move || avaliar_troca(&dados, &Restricoes::padrao(), &destino)).await
}

/// Troca a pasta da base. A vigia do índice para antes (para não reindexar a base
/// antiga por cima da reconstrução) e volta na base nova.
#[tauri::command]
pub async fn trocar_base_app(
    destino: String,
    modo: ModoTroca,
    vigia: State<'_, Arc<VigiaBase>>,
) -> Result<TrocaBase, String> {
    let dados = dados()?;
    let vigia = Arc::clone(&vigia);
    bloqueante(move || {
        vigia.parar();
        let feito = trocar_base(&dados, &Restricoes::padrao(), &destino, modo);
        vigia.garantir(&dados, cba_kb::indice::Vigia::INTERVALO);
        feito
    })
    .await
}

#[tauri::command]
pub async fn previa_exclusoes_app(ajuste: AjusteExclusoes) -> Result<PreviaProjeto, String> {
    let dados = dados()?;
    bloqueante(move || previa_exclusoes(&dados, &ajuste)).await
}

#[tauri::command]
pub async fn salvar_exclusoes_app(ajuste: AjusteExclusoes) -> Result<ProjetoCadastrado, String> {
    let dados = dados()?;
    bloqueante(move || salvar_exclusoes(&dados, &ajuste)).await
}

#[tauri::command]
pub async fn desfazer_ambiente_projeto(projeto: String) -> Result<AmbienteDesfeito, String> {
    let dados = dados()?;
    bloqueante(move || desfazer_ambiente(&dados, &preparador()?, &projeto)).await
}

#[tauri::command]
pub async fn descadastrar_projeto_app(projeto: String) -> Result<Descadastro, String> {
    let dados = dados()?;
    bloqueante(move || descadastrar(&dados, &preparador()?, &projeto)).await
}

#[tauri::command]
pub async fn textos_editaveis_app() -> Result<Vec<TextoVista>, String> {
    let dados = dados()?;
    bloqueante(move || textos(&dados)).await
}

/// Abre o texto no editor padrão do sistema operacional (criando a cópia do padrão
/// se faltar). Devolve o caminho, para a tela mostrar.
#[tauri::command]
pub async fn abrir_texto_editavel(
    app: tauri::AppHandle,
    arquivo: String,
) -> Result<PathBuf, String> {
    let dados = dados()?;
    let caminho = bloqueante(move || preparar_texto(&dados, &arquivo)).await?;
    app.opener()
        .open_path(caminho.to_string_lossy(), None::<&str>)
        .map_err(|e| format!("não foi possível abrir {}: {e}", caminho.display()))?;
    Ok(caminho)
}

#[tauri::command]
pub async fn ler_texto_editavel(arquivo: String) -> Result<LeituraTexto, String> {
    let dados = dados()?;
    bloqueante(move || ler_texto(&dados, &arquivo)).await
}

#[tauri::command]
pub async fn validar_texto_editavel(
    arquivo: String,
    texto: String,
) -> Result<Vec<AvisoTexto>, String> {
    let dados = dados()?;
    bloqueante(move || validar_texto(&dados, &arquivo, &texto)).await
}

#[tauri::command]
pub async fn salvar_texto_editavel(
    arquivo: String,
    texto: String,
    versao_lida: String,
) -> Result<SalvoTexto, String> {
    let dados = dados()?;
    bloqueante(move || salvar_texto(&dados, &arquivo, &texto, &versao_lida)).await
}

#[tauri::command]
pub async fn restaurar_texto_padrao(arquivo: String) -> Result<LeituraTexto, String> {
    let dados = dados()?;
    bloqueante(move || restaurar_texto(&dados, &arquivo)).await
}

#[cfg(test)]
#[path = "configuracoes_testes.rs"]
mod tests;
