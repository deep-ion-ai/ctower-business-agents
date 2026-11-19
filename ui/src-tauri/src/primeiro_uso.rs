//! Comandos Tauri do primeiro uso (spec, seção 4.1, #35).
//!
//! O assistente da tela passa por: cliente de IA preferido (a detecção é a de
//! [`cba_session::clientes`]), pasta da base de conhecimento, cadastro do primeiro
//! projeto (pastas, inventário com o que fica fora e ajuste das exclusões), preparo do
//! ambiente com consentimento ([`crate::analise`]) e a ida para a primeira análise.
//! As mesmas funções servem à tela de configurações (#39), para refazer cada passo.
//!
//! Conferências que o núcleo deixou para o app (#7): a base não pode ficar numa pasta
//! temporária, no diretório de dados do app nem dentro do repositório (ou da pasta) de
//! um projeto cadastrado; e o projeto não pode conter a base.
//!
//! A preferência do cliente e a marca de primeiro uso concluído ficam em
//! `<dados>/config/preferencias.json`, do app, e não no `app.json` de
//! [`Config`], que os outros modos do executável também leem e regravam por inteiro.

use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use cba_core::ambiente::Ambiente;
use cba_core::config::Config;
use cba_core::dados::DiretorioDados;
use cba_core::eventos::DiarioEventos;
use cba_index::{Cadastro, Ignorado, Inventario, detectar_git};
use serde::{Deserialize, Serialize};

use crate::terminal::ProjetoResumo;

const PREFERENCIAS: &str = "preferencias.json";

/// Quantos itens ignorados a prévia lista um a um (o resumo conta todos, e os
/// grupos cobrem todos).
pub const MAX_IGNORADOS_NA_PREVIA: usize = 500;

/// Como o analista prefere conversar com a IA (spec, seção 4.1, passo 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ClientePreferido {
    /// Claude Code no terminal embutido do app (recomendado).
    TerminalEmbutido,
    /// Claude Desktop, com o documento no app.
    ClaudeDesktop,
}

/// Preferências do app gravadas pelo primeiro uso.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Preferencias {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cliente: Option<ClientePreferido>,
    /// `None` quando o arquivo é anterior ao assistente.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primeiro_uso_concluido: Option<bool>,
    /// Pasta base dos clones dos repositórios de código (#179); `None` = a
    /// sugerida.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pasta_clones: Option<PathBuf>,
    /// Hosts acrescentados na seção Acessos e o último teste de cada token
    /// (#193); nunca o token.
    #[serde(default, skip_serializing_if = "crate::acessos::AcessosSalvos::vazio")]
    pub acessos: crate::acessos::AcessosSalvos,
}

impl Preferencias {
    pub fn arquivo(dados: &DiretorioDados) -> PathBuf {
        dados.config().join(PREFERENCIAS)
    }

    /// Lê as preferências; sem arquivo, as padrão. Campos desconhecidos são ignorados.
    pub fn ler(dados: &DiretorioDados) -> Result<Self, String> {
        match std::fs::read(Self::arquivo(dados)) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|e| format!("preferências do app ilegíveis: {e}")),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(format!("preferências do app inacessíveis: {e}")),
        }
    }

    pub fn gravar(&self, dados: &DiretorioDados) -> Result<(), String> {
        let json = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        cba_kb::arquivo::gravar(&Self::arquivo(dados), &json)
            .map_err(|e| format!("não foi possível gravar as preferências: {e}"))
    }
}

// ---------------------------------------------------------------------------
// Situação

/// O que a tela precisa saber antes de tudo.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Situacao {
    /// Pasta da base de conhecimento, se já escolhida.
    pub base: Option<PathBuf>,
    /// Sugestão para a base (`Documentos/ctower-business`).
    pub base_sugerida: Option<PathBuf>,
    pub projetos: Vec<ProjetoResumo>,
    pub cliente: Option<ClientePreferido>,
    /// Se o assistente do primeiro uso já foi concluído. Quem já tinha base e projeto
    /// antes do assistente (versões anteriores do app) não passa por ele de novo.
    pub primeiro_uso_concluido: bool,
}

/// Pasta sugerida para a base: `Documentos/ctower-business`.
pub fn base_sugerida() -> Option<PathBuf> {
    dirs::document_dir()
        .or_else(|| dirs::home_dir().map(|h| h.join("Documentos")))
        .map(|d| d.join("ctower-business"))
}

pub fn situacao(dados: &DiretorioDados) -> Result<Situacao, String> {
    let config = Config::ler(dados).map_err(|e| format!("configuração do app ilegível: {e}"))?;
    let preferencias = Preferencias::ler(dados)?;
    let projetos: Vec<ProjetoResumo> = Cadastro::em(dados.projetos())
        .listar()
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|s| ProjetoResumo {
            id: s.id,
            nome: s.nome,
        })
        .collect();
    let concluido = preferencias
        .primeiro_uso_concluido
        .unwrap_or(config.base.is_some() && !projetos.is_empty());
    Ok(Situacao {
        base: config.base,
        base_sugerida: base_sugerida(),
        projetos,
        cliente: preferencias.cliente,
        primeiro_uso_concluido: concluido,
    })
}

/// Grava o cliente preferido.
pub fn escolher_cliente(dados: &DiretorioDados, cliente: ClientePreferido) -> Result<(), String> {
    let mut p = Preferencias::ler(dados)?;
    p.cliente = Some(cliente);
    if p.primeiro_uso_concluido.is_none() {
        p.primeiro_uso_concluido = Some(false);
    }
    p.gravar(dados)
}

/// Marca o primeiro uso como concluído. Exige a base e um projeto cadastrado.
pub fn concluir_primeiro_uso(dados: &DiretorioDados) -> Result<(), String> {
    let s = situacao(dados)?;
    if s.base.is_none() {
        return Err("escolha a pasta da base de conhecimento antes de concluir".into());
    }
    if s.projetos.is_empty() {
        return Err("cadastre um projeto antes de concluir".into());
    }
    let mut p = Preferencias::ler(dados)?;
    p.primeiro_uso_concluido = Some(true);
    p.gravar(dados)
}

// ---------------------------------------------------------------------------
// Base de conhecimento

/// Pastas onde a base não pode ficar, além das pastas dos projetos.
#[derive(Debug, Clone, Default)]
pub struct Restricoes {
    /// Pastas temporárias do sistema operacional.
    pub temporarias: Vec<PathBuf>,
}

impl Restricoes {
    /// As do sistema operacional: a pasta temporária do processo e, no Unix,
    /// `/tmp` e `/var/tmp`.
    pub fn padrao() -> Self {
        let mut temporarias = vec![std::env::temp_dir()];
        if cfg!(unix) {
            temporarias.push(PathBuf::from("/tmp"));
            temporarias.push(PathBuf::from("/var/tmp"));
        }
        Self { temporarias }
    }
}

/// Como está a pasta escolhida para a base.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EstadoPasta {
    /// Ainda não existe; o app cria.
    Nova,
    /// Existe e está vazia.
    Vazia,
    /// Já é uma base (tem `_templates/`).
    Base,
    /// Existe e tem outros arquivos; a base fica junto deles.
    ComArquivos,
}

/// Resultado da conferência da pasta da base.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvaliacaoBase {
    /// Caminho como será gravado.
    pub caminho: PathBuf,
    pub estado: Option<EstadoPasta>,
    /// Por que a pasta não pode ser usada; `None` se pode.
    pub recusa: Option<String>,
    /// A pasta está num repositório git (permitido, desde que não seja o de um projeto).
    pub git: bool,
}

/// Caminho canônico de algo que talvez ainda não exista: a parte que existe é
/// resolvida (links, `..`), e o resto é acrescentado como está.
pub(crate) fn canonica_aproximada(caminho: &Path) -> PathBuf {
    let mut existente = caminho.to_path_buf();
    let mut resto = Vec::new();
    loop {
        if let Ok(c) = dunce::canonicalize(&existente) {
            let mut r = c;
            for parte in resto.iter().rev() {
                r.push(parte);
            }
            return r;
        }
        match (
            existente.file_name().map(ToOwned::to_owned),
            existente.parent(),
        ) {
            (Some(nome), Some(pai)) => {
                resto.push(nome);
                existente = pai.to_path_buf();
            }
            _ => return caminho.to_path_buf(),
        }
    }
}

/// Um repositório git na pasta do usuário (ou acima dela), como o de quem versiona
/// os próprios arquivos de configuração, não é o repositório do projeto.
fn repositorio_do_usuario(raiz: &Path) -> bool {
    dirs::home_dir().is_some_and(|h| canonica_aproximada(&h).starts_with(raiz))
}

/// Raízes que a base não pode invadir por causa de um projeto: as pastas dele e os
/// repositórios git que as contêm (o principal, numa worktree).
fn raizes_do_projeto(projeto: &cba_index::Projeto) -> Vec<(PathBuf, String)> {
    let mut raizes = Vec::new();
    for p in &projeto.pastas {
        let pasta = canonica_aproximada(&p.caminho);
        if let Some(git) = detectar_git(&pasta) {
            for r in [git.raiz, git.raiz_principal] {
                let r = canonica_aproximada(&r);
                if !repositorio_do_usuario(&r) {
                    raizes.push((r, projeto.nome.clone()));
                }
            }
        }
        raizes.push((pasta, projeto.nome.clone()));
    }
    raizes
}

/// Recusa da base diante de um projeto (cadastrado ou em cadastro).
pub(crate) fn conflito_com_projeto(base: &Path, projeto: &cba_index::Projeto) -> Option<String> {
    let base = canonica_aproximada(base);
    let repo_da_base = detectar_git(&base)
        .map(|g| canonica_aproximada(&g.raiz_principal))
        .filter(|r| !repositorio_do_usuario(r));
    for (raiz, nome) in raizes_do_projeto(projeto) {
        if base.starts_with(&raiz) || repo_da_base.as_ref() == Some(&raiz) {
            return Some(format!(
                "a base não pode ficar dentro do repositório ou das pastas do projeto \
                 \"{nome}\" ({}): os documentos se misturariam ao código analisado",
                raiz.display()
            ));
        }
    }
    for p in &projeto.pastas {
        if canonica_aproximada(&p.caminho).starts_with(&base) {
            return Some(format!(
                "a pasta do projeto \"{}\" ({}) fica dentro da base; escolha pastas \
                 separadas",
                projeto.nome,
                p.caminho.display()
            ));
        }
    }
    None
}

/// Confere a pasta escolhida para a base, sem criar nada.
pub fn avaliar_base(
    dados: &DiretorioDados,
    restricoes: &Restricoes,
    pasta: &str,
) -> Result<AvaliacaoBase, String> {
    let bruto = PathBuf::from(pasta.trim());
    if bruto.as_os_str().is_empty() || !bruto.is_absolute() {
        return Err("informe o caminho completo da pasta da base".into());
    }
    if bruto.components().any(|c| c == Component::ParentDir) {
        return Err("o caminho da base não pode ter \"..\"".into());
    }
    let caminho = canonica_aproximada(&bruto);
    let estado = if !caminho.exists() {
        Some(EstadoPasta::Nova)
    } else if !caminho.is_dir() {
        None
    } else if caminho.join(cba_kb::layout::TEMPLATES).is_dir() {
        Some(EstadoPasta::Base)
    } else {
        let vazia = std::fs::read_dir(&caminho)
            .map(|mut l| l.next().is_none())
            .unwrap_or(false);
        Some(if vazia {
            EstadoPasta::Vazia
        } else {
            EstadoPasta::ComArquivos
        })
    };
    let recusa = recusa_da_base(dados, restricoes, &caminho, estado);
    Ok(AvaliacaoBase {
        git: detectar_git(&caminho).is_some(),
        caminho,
        estado,
        recusa,
    })
}

/// Se dá para criar uma pasta dentro do ancestral existente mais próximo de
/// `caminho` (que ainda não existe): ele precisa ser uma pasta com permissão de
/// escrita e de travessia. No Unix, o teste é o do próprio sistema (`access`), que
/// vale para dono, grupo, ACLs e sistema de arquivos só de leitura.
fn ancestral_gravavel(caminho: &Path) -> bool {
    let Some(ancestral) = caminho.ancestors().find(|a| a.exists()) else {
        return true;
    };
    if !ancestral.is_dir() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        let Ok(c) = std::ffi::CString::new(ancestral.as_os_str().as_bytes()) else {
            return false;
        };
        // SAFETY: `c` é uma string C válida e termina em NUL.
        unsafe { libc::access(c.as_ptr(), libc::W_OK | libc::X_OK) == 0 }
    }
    #[cfg(not(unix))]
    {
        std::fs::metadata(ancestral).is_ok_and(|m| !m.permissions().readonly())
    }
}

/// Texto em português para uma falha de E/S ao criar a pasta da base.
fn erro_de_criacao(caminho: &Path, erro: &std::io::Error) -> String {
    use std::io::ErrorKind;
    let motivo = match erro.kind() {
        ErrorKind::PermissionDenied => "Sem permissão para criar a pasta aqui".to_string(),
        ErrorKind::ReadOnlyFilesystem => {
            "o disco ou a pasta é somente leitura; escolha outro local".to_string()
        }
        ErrorKind::StorageFull | ErrorKind::QuotaExceeded => {
            "não há espaço livre no disco; libere espaço ou escolha outro local".to_string()
        }
        ErrorKind::NotADirectory => "uma parte do caminho é um arquivo, não uma pasta".to_string(),
        ErrorKind::InvalidFilename => "o nome da pasta não é válido neste sistema".to_string(),
        _ => format!("não foi possível criar a pasta ({erro})"),
    };
    format!("{}: {motivo}", caminho.display())
}

fn recusa_da_base(
    dados: &DiretorioDados,
    restricoes: &Restricoes,
    caminho: &Path,
    estado: Option<EstadoPasta>,
) -> Option<String> {
    if estado.is_none() {
        return Some("o caminho existe e não é uma pasta".into());
    }
    if caminho.parent().is_none() {
        return Some("a base não pode ser a raiz do disco".into());
    }
    for t in &restricoes.temporarias {
        let t = canonica_aproximada(t);
        if caminho.starts_with(&t) {
            return Some(format!(
                "a base não pode ficar numa pasta temporária ({}), que o projeto \
                 operacional pode apagar",
                t.display()
            ));
        }
    }
    if estado == Some(EstadoPasta::Nova) && !ancestral_gravavel(caminho) {
        return Some("Sem permissão para criar a pasta aqui".into());
    }
    let app = canonica_aproximada(dados.raiz());
    if caminho.starts_with(&app) || app.starts_with(caminho) {
        return Some(
            "a base não pode ficar no diretório de dados do app, que a desinstalação apaga".into(),
        );
    }
    let projetos = Cadastro::em(dados.projetos()).listar().ok()?;
    projetos
        .iter()
        .find_map(|s| conflito_com_projeto(caminho, s))
}

/// Escolhe a pasta da base: confere, cria (ou completa) e grava em `Config.base`.
pub fn configurar_base(
    dados: &DiretorioDados,
    restricoes: &Restricoes,
    pasta: &str,
) -> Result<PathBuf, String> {
    let avaliacao = avaliar_base(dados, restricoes, pasta)?;
    if let Some(r) = avaliacao.recusa {
        return Err(r);
    }
    let pasta = avaliacao.caminho;
    cba_kb::Base::criar(&pasta).map_err(|e| match &e {
        cba_kb::Erro::PastaDaBase { caminho, erro } => erro_de_criacao(caminho, erro),
        _ => e.to_string(),
    })?;
    let mut config = Config::ler(dados).map_err(|e| e.to_string())?;
    config.base = Some(pasta.clone());
    config
        .gravar(dados)
        .map_err(|e| format!("não foi possível gravar a configuração: {e}"))?;
    Ok(pasta)
}

// ---------------------------------------------------------------------------
// Cadastro do projeto

/// O que o analista informa no cadastro.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PedidoProjeto {
    pub nome: String,
    /// Caminhos completos; a primeira é a raiz.
    pub pastas: Vec<String>,
    /// Padrões a excluir além dos padrão.
    #[serde(default)]
    pub exclusoes: Vec<String>,
    /// Padrões reincluídos.
    #[serde(default)]
    pub inclusoes: Vec<String>,
    /// Repositório de código e de issues (opcional).
    #[serde(default)]
    pub repositorios: cba_repos::Repositorios,
}

pub(crate) fn limpar(lista: &[String]) -> Vec<String> {
    let mut saida: Vec<String> = Vec::new();
    for x in lista.iter().map(|x| x.trim()).filter(|x| !x.is_empty()) {
        if !saida.iter().any(|y| y == x) {
            saida.push(x.to_owned());
        }
    }
    saida
}

/// Monta e confere o projeto do pedido, inclusive diante da base configurada.
fn montar_projeto(
    dados: &DiretorioDados,
    pedido: &PedidoProjeto,
) -> Result<cba_index::Projeto, String> {
    let pastas = limpar(&pedido.pastas);
    if pastas.is_empty() {
        return Err("informe ao menos uma pasta do código".into());
    }
    for p in &pastas {
        if !Path::new(p).is_absolute() {
            return Err(format!(
                "informe o caminho completo da pasta do projeto: {p}"
            ));
        }
    }
    let mut projeto = cba_index::Projeto::novo(&pedido.nome, &pastas).map_err(|e| e.to_string())?;
    projeto.exclusoes = limpar(&pedido.exclusoes);
    projeto.inclusoes = limpar(&pedido.inclusoes);
    projeto.repositorios = pedido.repositorios.clone();
    projeto.validar().map_err(|e| e.to_string())?;
    let config = Config::ler(dados).map_err(|e| e.to_string())?;
    if let Some(base) = config.base()
        && let Some(r) = conflito_com_projeto(base, &projeto)
    {
        return Err(r);
    }
    Ok(projeto)
}

/// Uma pasta do projeto, como a tela mostra.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PastaVista {
    pub caminho: PathBuf,
    pub prefixo: Option<String>,
    /// Branch, se a pasta está num repositório git.
    pub branch: Option<String>,
    pub git: bool,
    /// URL do primeiro remoto do repositório git da pasta.
    pub remoto: Option<String>,
    /// Forja, host e caminho deduzidos do remoto (#178).
    pub forja: Option<cba_repos::ForgeKind>,
    pub host: Option<String>,
    pub caminho_repo: Option<String>,
}

/// Contagem com rótulo para a interface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Contagem {
    /// Valor no formato do `cba-index` (`sql`, `segredo`).
    pub chave: String,
    pub rotulo: String,
    pub quantidade: usize,
}

/// Item que fica fora da análise, como a tela mostra.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IgnoradoVista {
    pub caminho: String,
    pub pasta: bool,
    /// Arquivos que o item cobre (numa pasta, os que há dentro dela).
    pub arquivos: usize,
    pub motivo: String,
    pub rotulo: String,
    pub regra: Option<String>,
    /// Pode ser reincluído (exclusão por padrão; binário pelo conteúdo, link e
    /// ilegível não).
    pub ajustavel: bool,
}

/// Itens que ficam fora, agrupados pela pasta em que estão e pelo tipo (extensão),
/// para a tela não listar centenas de arquivos soltos.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrupoIgnorado {
    /// Pasta que contém os itens (vazio = a raiz do projeto).
    pub pasta: String,
    /// Extensão (`png`), `sem extensão` ou `pasta` (pastas inteiras que ficam fora).
    pub tipo: String,
    pub motivo: String,
    pub rotulo: String,
    /// Itens do grupo (uma pasta ignorada é um item).
    pub itens: usize,
    /// Arquivos que o grupo cobre.
    pub arquivos: usize,
    /// Até três caminhos do grupo, para reconhecê-lo.
    pub exemplos: Vec<String>,
    /// Padrões que reincluem o grupo todo; vazio quando não dá para reincluir.
    pub padroes: Vec<String>,
    pub ajustavel: bool,
}

/// O efeito de uma reinclusão do analista no inventário.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InclusaoVista {
    pub padrao: String,
    /// Arquivos que a reinclusão faz entrar na análise.
    pub entram: usize,
    /// Arquivos que ela cobre e que continuam fora (binário pelo conteúdo, por exemplo).
    pub continuam_fora: usize,
}

/// Resumo do inventário: o que será analisado e o que fica fora. As contagens são
/// sempre em arquivos.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumoInventario {
    pub analisados: usize,
    pub bytes_analisados: u64,
    pub ignorados: usize,
    /// Itens ignorados (uma pasta conta 1), para o "e mais N itens" de `itens_ignorados`.
    pub total_itens_ignorados: usize,
    /// Todos os itens ignorados, agrupados por pasta e tipo.
    pub grupos_ignorados: Vec<GrupoIgnorado>,
    /// Da maior contagem para a menor.
    pub por_linguagem: Vec<Contagem>,
    pub por_motivo: Vec<Contagem>,
    /// Os primeiros [`MAX_IGNORADOS_NA_PREVIA`] itens ignorados.
    pub itens_ignorados: Vec<IgnoradoVista>,
    pub duracao_ms: u64,
}

fn chave<T: Serialize>(valor: &T) -> String {
    serde_json::to_value(valor)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

fn ignorado_vista(i: &Ignorado) -> IgnoradoVista {
    IgnoradoVista {
        caminho: i.caminho.clone(),
        pasta: i.pasta,
        arquivos: i.arquivos,
        motivo: chave(&i.motivo),
        rotulo: i.motivo.descricao().to_owned(),
        regra: i.regra.clone(),
        ajustavel: i.regra.is_some(),
    }
}

/// Escapa os curingas do glob (`*?[]{}`) de um trecho de caminho.
fn escapar_glob(texto: &str) -> String {
    texto
        .chars()
        .map(|c| {
            if matches!(c, '*' | '?' | '[' | ']' | '{' | '}') {
                format!("[{c}]")
            } else {
                c.to_string()
            }
        })
        .collect()
}

/// Pasta que contém o item e tipo dele: a extensão, `sem extensão` ou `pasta`.
fn pasta_e_tipo(i: &Ignorado) -> (String, String) {
    let (pai, nome) = match i.caminho.rsplit_once('/') {
        Some((p, n)) => (p.to_owned(), n),
        None => (String::new(), i.caminho.as_str()),
    };
    let tipo = if i.pasta {
        "pasta".to_owned()
    } else {
        match nome.rsplit_once('.') {
            Some((antes, ext)) if !antes.is_empty() && !ext.is_empty() => ext.to_lowercase(),
            _ => "sem extensão".to_owned(),
        }
    };
    (pai, tipo)
}

/// Agrupa os itens ignorados por (pasta, tipo, motivo). Dos maiores (em arquivos)
/// para os menores.
pub fn agrupar_ignorados(ignorados: &[Ignorado]) -> Vec<GrupoIgnorado> {
    use std::collections::BTreeMap;
    let mut mapa: BTreeMap<(String, String, String, bool), Vec<&Ignorado>> = BTreeMap::new();
    for i in ignorados {
        let (pai, tipo) = pasta_e_tipo(i);
        mapa.entry((pai, tipo, chave(&i.motivo), i.regra.is_some()))
            .or_default()
            .push(i);
    }
    let mut grupos: Vec<GrupoIgnorado> = mapa
        .into_iter()
        .map(|((pasta, tipo, motivo, ajustavel), itens)| {
            let exato = |i: &&Ignorado| format!("/{}", escapar_glob(&i.caminho));
            // Vários arquivos da mesma pasta e extensão: um só padrão cobre todos.
            let padroes = if !ajustavel {
                Vec::new()
            } else if itens.len() > 1 && !itens[0].pasta && tipo != "sem extensão" {
                let base = if pasta.is_empty() {
                    String::new()
                } else {
                    format!("/{}", escapar_glob(&pasta))
                };
                vec![format!("{base}/*.{}", escapar_glob(&tipo))]
            } else {
                itens.iter().map(exato).collect()
            };
            GrupoIgnorado {
                rotulo: itens[0].motivo.descricao().to_owned(),
                itens: itens.len(),
                arquivos: itens.iter().map(|i| i.arquivos).sum(),
                exemplos: itens.iter().take(3).map(|i| i.caminho.clone()).collect(),
                pasta,
                tipo,
                motivo,
                padroes,
                ajustavel,
            }
        })
        .collect();
    grupos.sort_by(|a, b| {
        b.arquivos
            .cmp(&a.arquivos)
            .then_with(|| a.pasta.cmp(&b.pasta))
            .then_with(|| a.tipo.cmp(&b.tipo))
    });
    grupos
}

/// O que cada reinclusão faz: arquivos que entram e arquivos que continuam fora.
pub fn inclusoes_vistas(inventario: &Inventario, inclusoes: &[String]) -> Vec<InclusaoVista> {
    inclusoes
        .iter()
        .filter_map(|padrao| {
            let e = cba_index::Exclusoes::novas(&[], std::slice::from_ref(padrao)).ok()?;
            let entram = inventario
                .arquivos
                .iter()
                .filter(|a| e.reincluido(&a.caminho))
                .count();
            let continuam_fora = inventario
                .ignorados
                .iter()
                .filter(|i| !i.pasta && e.reincluido(&i.caminho))
                .map(|i| i.arquivos)
                .sum();
            Some(InclusaoVista {
                padrao: padrao.clone(),
                entram,
                continuam_fora,
            })
        })
        .collect()
}

pub fn resumo_inventario(inventario: &Inventario) -> ResumoInventario {
    let r = inventario.resumo();
    let mut por_linguagem: Vec<Contagem> = r
        .por_linguagem
        .iter()
        .map(|(l, n)| Contagem {
            chave: chave(l),
            rotulo: l.nome().to_owned(),
            quantidade: *n,
        })
        .collect();
    por_linguagem.sort_by(|a, b| {
        b.quantidade
            .cmp(&a.quantidade)
            .then(a.rotulo.cmp(&b.rotulo))
    });
    let mut por_motivo: Vec<Contagem> = r
        .ignorados_por_motivo
        .iter()
        .map(|(m, n)| Contagem {
            chave: chave(m),
            rotulo: m.descricao().to_owned(),
            quantidade: *n,
        })
        .collect();
    por_motivo.sort_by(|a, b| {
        b.quantidade
            .cmp(&a.quantidade)
            .then(a.rotulo.cmp(&b.rotulo))
    });
    // Os possíveis segredos sempre entram na lista (a tela os destaca); o resto, até o limite.
    let mut ignorados: Vec<&Ignorado> = inventario.ignorados.iter().collect();
    ignorados.sort_by(|a, b| {
        (a.motivo != cba_index::Motivo::Segredo)
            .cmp(&(b.motivo != cba_index::Motivo::Segredo))
            .then_with(|| a.caminho.cmp(&b.caminho))
    });
    let segredos = ignorados
        .iter()
        .take_while(|i| i.motivo == cba_index::Motivo::Segredo)
        .count();
    ignorados.truncate(segredos.max(MAX_IGNORADOS_NA_PREVIA));
    ignorados.sort_by(|a, b| a.caminho.cmp(&b.caminho));
    ResumoInventario {
        analisados: r.analisados,
        bytes_analisados: r.bytes_analisados,
        ignorados: r.ignorados,
        total_itens_ignorados: inventario.ignorados.len(),
        grupos_ignorados: agrupar_ignorados(&inventario.ignorados),
        por_linguagem,
        por_motivo,
        itens_ignorados: ignorados.into_iter().map(ignorado_vista).collect(),
        duracao_ms: inventario.estatisticas.duracao_ms,
    }
}

/// Prévia do cadastro: o projeto como ficaria e o inventário, sem gravar nada.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviaProjeto {
    pub id: String,
    pub nome: String,
    pub pastas: Vec<PastaVista>,
    pub exclusoes: Vec<String>,
    pub inclusoes: Vec<String>,
    /// O que cada reinclusão faz entrar (ou não) na análise.
    pub inclusoes_vistas: Vec<InclusaoVista>,
    /// Já existe um projeto com o mesmo identificador.
    pub ja_cadastrado: bool,
    pub inventario: ResumoInventario,
}

pub(crate) fn pastas_vista(inventario: &Inventario) -> Vec<PastaVista> {
    inventario
        .pastas
        .iter()
        .map(|p| {
            let remoto = p
                .git
                .as_ref()
                .and_then(|_| cba_index::remotos_git(&p.caminho).into_iter().next());
            let deduzido = remoto
                .as_deref()
                .and_then(cba_repos::RepositorioCodigo::da_url);
            PastaVista {
                caminho: p.caminho.clone(),
                prefixo: p.prefixo.clone(),
                branch: p.git.as_ref().and_then(|g| g.branch.clone()),
                git: p.git.is_some(),
                forja: deduzido.as_ref().map(|r| r.forja),
                host: deduzido.as_ref().map(|r| r.host.clone()),
                caminho_repo: deduzido.map(|r| r.caminho),
                remoto,
            }
        })
        .collect()
}

pub fn previa_projeto(
    dados: &DiretorioDados,
    pedido: &PedidoProjeto,
) -> Result<PreviaProjeto, String> {
    let projeto = montar_projeto(dados, pedido)?;
    let inventario = cba_index::inventariar(&projeto, None).map_err(|e| e.to_string())?;
    let ja_cadastrado = Cadastro::em(dados.projetos())
        .obter(&projeto.id)
        .map_err(|e| e.to_string())?
        .is_some();
    Ok(PreviaProjeto {
        pastas: pastas_vista(&inventario),
        inclusoes_vistas: inclusoes_vistas(&inventario, &projeto.inclusoes),
        inventario: resumo_inventario(&inventario),
        id: projeto.id,
        nome: projeto.nome,
        exclusoes: projeto.exclusoes,
        inclusoes: projeto.inclusoes,
        ja_cadastrado,
    })
}

/// Já existe um projeto cadastrado com este nome (ou com o mesmo identificador)?
pub fn nome_em_uso(dados: &DiretorioDados, nome: &str) -> Result<bool, String> {
    let id = cba_index::slug(nome);
    if id.is_empty() {
        return Ok(false);
    }
    Ok(Cadastro::em(dados.projetos())
        .obter(&id)
        .map_err(|e| e.to_string())?
        .is_some())
}

/// Projeto cadastrado, com o inventário gravado.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjetoCadastrado {
    pub id: String,
    pub nome: String,
    pub pastas: Vec<PastaVista>,
    pub inventario: ResumoInventario,
}

/// Cadastra o projeto, grava o inventário e cria o projeto na base. Se algo falhar
/// depois de gravado o cadastro, ele é desfeito.
pub fn cadastrar_projeto(
    dados: &DiretorioDados,
    pedido: &PedidoProjeto,
) -> Result<ProjetoCadastrado, String> {
    let config = Config::ler(dados).map_err(|e| e.to_string())?;
    if config.base.is_none() {
        return Err("escolha a pasta da base de conhecimento antes de cadastrar o projeto".into());
    }
    let projeto = montar_projeto(dados, pedido)?;
    let cadastro = Cadastro::em(dados.projetos());
    cadastro.cadastrar(&projeto).map_err(|e| e.to_string())?;
    let feito = (|| {
        let inventario = cadastro
            .inventariar(&projeto.id)
            .map_err(|e| e.to_string())?;
        let pasta = &projeto.pastas[0].caminho;
        let notificador = Arc::new(DiarioEventos::em(dados.eventos()));
        Ambiente::do_processo_com(pasta, Some(&projeto.id), dados, notificador)
            .map_err(|e| e.to_string())?;
        Ok::<_, String>(inventario)
    })();
    match feito {
        Ok(inventario) => Ok(ProjetoCadastrado {
            pastas: pastas_vista(&inventario),
            inventario: resumo_inventario(&inventario),
            id: projeto.id,
            nome: projeto.nome,
        }),
        Err(e) => {
            let _ = cadastro.remover(&projeto.id);
            Err(e)
        }
    }
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

#[tauri::command]
pub async fn situacao_app() -> Result<Situacao, String> {
    let dados = dados()?;
    bloqueante(move || situacao(&dados)).await
}

#[tauri::command]
pub async fn escolher_cliente_app(cliente: ClientePreferido) -> Result<(), String> {
    let dados = dados()?;
    bloqueante(move || escolher_cliente(&dados, cliente)).await
}

#[tauri::command]
pub async fn avaliar_base_app(pasta: String) -> Result<AvaliacaoBase, String> {
    let dados = dados()?;
    bloqueante(move || avaliar_base(&dados, &Restricoes::padrao(), &pasta)).await
}

#[tauri::command]
pub async fn configurar_base_app(pasta: String) -> Result<PathBuf, String> {
    let dados = dados()?;
    bloqueante(move || configurar_base(&dados, &Restricoes::padrao(), &pasta)).await
}

#[tauri::command]
pub async fn previa_projeto_app(pedido: PedidoProjeto) -> Result<PreviaProjeto, String> {
    let dados = dados()?;
    bloqueante(move || previa_projeto(&dados, &pedido)).await
}

#[tauri::command]
pub async fn nome_projeto_em_uso_app(nome: String) -> Result<bool, String> {
    let dados = dados()?;
    bloqueante(move || nome_em_uso(&dados, &nome)).await
}

#[tauri::command]
pub async fn cadastrar_projeto_app(pedido: PedidoProjeto) -> Result<ProjetoCadastrado, String> {
    let dados = dados()?;
    bloqueante(move || {
        let feito = cadastrar_projeto(&dados, &pedido)?;
        tracing::info!(
            pastas = feito.pastas.len(),
            analisados = feito.inventario.analisados,
            ignorados = feito.inventario.ignorados,
            "projeto cadastrado"
        );
        Ok(feito)
    })
    .await
}

#[tauri::command]
pub async fn concluir_primeiro_uso_app() -> Result<(), String> {
    let dados = dados()?;
    bloqueante(move || concluir_primeiro_uso(&dados)).await
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    pub(crate) fn fixture() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/faturamento-exemplo")
            .canonicalize()
            .unwrap()
    }

    fn pedido(pastas: &[&Path]) -> PedidoProjeto {
        PedidoProjeto {
            nome: "Faturamento Exemplo".into(),
            pastas: pastas
                .iter()
                .map(|p| p.to_str().unwrap().to_owned())
                .collect(),
            ..Default::default()
        }
    }

    fn sem_restricoes() -> Restricoes {
        Restricoes::default()
    }

    /// Repositório git falso (só o `HEAD`), que o `detectar_git` reconhece.
    fn repositorio(raiz: &Path) {
        fs::create_dir_all(raiz.join(".git")).unwrap();
        fs::write(raiz.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
        fs::create_dir_all(raiz.join("src")).unwrap();
        fs::write(raiz.join("src/Fatura.java"), "class Fatura {}\n").unwrap();
    }

    #[test]
    fn situacao_inicial_e_preferencias() {
        let dir = tempfile::tempdir().unwrap();
        let dados = DiretorioDados::em(dir.path().join("dados"));
        let s = situacao(&dados).unwrap();
        assert!(s.base.is_none() && s.projetos.is_empty() && s.cliente.is_none());
        assert!(!s.primeiro_uso_concluido);
        escolher_cliente(&dados, ClientePreferido::ClaudeDesktop).unwrap();
        let s = situacao(&dados).unwrap();
        assert_eq!(s.cliente, Some(ClientePreferido::ClaudeDesktop));
        assert!(!s.primeiro_uso_concluido);
        // Formato que a tela espera (ui/src/primeiroUso.ts).
        let json = serde_json::to_value(&s).unwrap();
        assert_eq!(json["cliente"], "claudeDesktop");
        assert_eq!(json["primeiroUsoConcluido"], false);
        // Sem base e projeto não conclui.
        assert!(concluir_primeiro_uso(&dados).is_err());
    }

    #[test]
    fn quem_ja_tinha_base_e_projeto_nao_passa_pelo_assistente() {
        let dir = tempfile::tempdir().unwrap();
        let dados = DiretorioDados::em(dir.path().join("dados"));
        configurar_base(
            &dados,
            &sem_restricoes(),
            dir.path().join("base").to_str().unwrap(),
        )
        .unwrap();
        cadastrar_projeto(&dados, &pedido(&[&fixture()])).unwrap();
        assert!(situacao(&dados).unwrap().primeiro_uso_concluido);
    }

    #[test]
    fn base_avaliada_criada_e_gravada_na_config() {
        let dir = tempfile::tempdir().unwrap();
        let dados = DiretorioDados::em(dir.path().join("dados"));
        let r = sem_restricoes();
        assert!(avaliar_base(&dados, &r, "relativa/base").is_err());
        assert!(avaliar_base(&dados, &r, "").is_err());
        let nova = dir.path().join("docs").join("base");
        let a = avaliar_base(&dados, &r, nova.to_str().unwrap()).unwrap();
        assert_eq!(a.estado, Some(EstadoPasta::Nova));
        assert!(a.recusa.is_none());
        assert!(!nova.exists(), "avaliar não cria nada");

        let com_arquivos = dir.path().join("sincronizada");
        fs::create_dir_all(&com_arquivos).unwrap();
        fs::write(com_arquivos.join("notas.txt"), "x").unwrap();
        let a = avaliar_base(&dados, &r, com_arquivos.to_str().unwrap()).unwrap();
        assert_eq!(a.estado, Some(EstadoPasta::ComArquivos));
        assert!(a.recusa.is_none());

        let arquivo = com_arquivos.join("notas.txt");
        let a = avaliar_base(&dados, &r, arquivo.to_str().unwrap()).unwrap();
        assert!(a.recusa.is_some());

        let gravada = configurar_base(&dados, &r, nova.to_str().unwrap()).unwrap();
        assert!(gravada.join("_templates").is_dir());
        assert_eq!(Config::ler(&dados).unwrap().base, Some(gravada.clone()));
        let a = avaliar_base(&dados, &r, nova.to_str().unwrap()).unwrap();
        assert_eq!(a.estado, Some(EstadoPasta::Base));
    }

    #[cfg(unix)]
    #[test]
    fn base_sob_pasta_somente_leitura_e_recusada() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let dados = DiretorioDados::em(dir.path().join("dados"));
        let r = sem_restricoes();
        let ro = dir.path().join("somente-leitura");
        fs::create_dir_all(&ro).unwrap();
        fs::set_permissions(&ro, fs::Permissions::from_mode(0o555)).unwrap();
        // Como root o sistema ignora a permissão: nada a verificar.
        let root = fs::create_dir(ro.join("sonda")).is_ok();
        if !root {
            let alvo = ro.join("a").join("base");
            let a = avaliar_base(&dados, &r, alvo.to_str().unwrap()).unwrap();
            assert_eq!(a.estado, Some(EstadoPasta::Nova));
            assert_eq!(
                a.recusa.as_deref(),
                Some("Sem permissão para criar a pasta aqui")
            );
            let e = configurar_base(&dados, &r, alvo.to_str().unwrap()).unwrap_err();
            assert!(e.contains("Sem permissão"), "{e}");
            // Uma pasta gravável continua aceita.
            let ok = dir.path().join("livre").join("base");
            let a = avaliar_base(&dados, &r, ok.to_str().unwrap()).unwrap();
            assert!(a.recusa.is_none());
        }
        fs::set_permissions(&ro, fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[test]
    fn erros_de_criacao_sao_traduzidos() {
        use std::io::{Error, ErrorKind};
        let p = Path::new("/x/base");
        let t = |k| erro_de_criacao(p, &Error::from(k));
        assert!(t(ErrorKind::PermissionDenied).contains("Sem permissão"));
        assert!(t(ErrorKind::StorageFull).contains("espaço"));
        assert!(t(ErrorKind::ReadOnlyFilesystem).contains("somente leitura"));
        assert!(t(ErrorKind::NotADirectory).contains("arquivo"));
    }

    #[test]
    fn base_em_pasta_temporaria_ou_nos_dados_do_app_e_recusada() {
        let dir = tempfile::tempdir().unwrap();
        let dados = DiretorioDados::em(dir.path().join("dados"));
        let temporaria = dir.path().join("tmp-do-projeto");
        let r = Restricoes {
            temporarias: vec![temporaria.clone()],
        };
        let erro =
            configurar_base(&dados, &r, temporaria.join("base").to_str().unwrap()).unwrap_err();
        assert!(erro.contains("temporária"), "{erro}");
        let erro =
            configurar_base(&dados, &r, dados.raiz().join("base").to_str().unwrap()).unwrap_err();
        assert!(erro.contains("dados do app"), "{erro}");
        assert!(Config::ler(&dados).unwrap().base.is_none());
        // O padrão inclui a pasta temporária do processo.
        let erro = configurar_base(
            &dados,
            &Restricoes::padrao(),
            std::env::temp_dir()
                .join("cba-base-teste")
                .to_str()
                .unwrap(),
        )
        .unwrap_err();
        assert!(erro.contains("temporária"), "{erro}");
    }

    #[test]
    fn base_dentro_do_repositorio_do_projeto_e_recusada() {
        let dir = tempfile::tempdir().unwrap();
        let dados = DiretorioDados::em(dir.path().join("dados"));
        let r = sem_restricoes();
        let repo = dir.path().join("repo-faturamento");
        repositorio(&repo);
        // Base fora; projeto cadastrado na pasta src/ do repositório.
        configurar_base(&dados, &r, dir.path().join("base").to_str().unwrap()).unwrap();
        cadastrar_projeto(&dados, &pedido(&[&repo.join("src")])).unwrap();
        // Trocar a base para dentro do repositório (fora da pasta cadastrada) é recusado.
        let dentro = repo.join("docs").join("base");
        let a = avaliar_base(&dados, &r, dentro.to_str().unwrap()).unwrap();
        let recusa = a.recusa.expect("recusada");
        assert!(recusa.contains("repositório"), "{recusa}");
        assert!(configurar_base(&dados, &r, dentro.to_str().unwrap()).is_err());
        // Uma base que contém o projeto também.
        assert!(
            avaliar_base(&dados, &r, dir.path().to_str().unwrap())
                .unwrap()
                .recusa
                .is_some()
        );
    }

    #[test]
    fn cadastro_recusa_projeto_cujo_repositorio_contem_a_base() {
        let dir = tempfile::tempdir().unwrap();
        let dados = DiretorioDados::em(dir.path().join("dados"));
        let repo = dir.path().join("repo");
        repositorio(&repo);
        configurar_base(
            &dados,
            &sem_restricoes(),
            repo.join("base").to_str().unwrap(),
        )
        .unwrap();
        let erro = previa_projeto(&dados, &pedido(&[&repo.join("src")])).unwrap_err();
        assert!(erro.contains("repositório"), "{erro}");
        assert!(cadastrar_projeto(&dados, &pedido(&[&repo.join("src")])).is_err());
        assert!(situacao(&dados).unwrap().projetos.is_empty());
    }

    #[test]
    fn previa_mostra_o_inventario_e_as_exclusoes_sem_gravar() {
        let dir = tempfile::tempdir().unwrap();
        let dados = DiretorioDados::em(dir.path().join("dados"));
        configurar_base(
            &dados,
            &sem_restricoes(),
            dir.path().join("base").to_str().unwrap(),
        )
        .unwrap();
        let p = previa_projeto(&dados, &pedido(&[&fixture()])).unwrap();
        assert_eq!(p.id, "faturamento-exemplo");
        assert!(!p.ja_cadastrado);
        let inv = &p.inventario;
        assert!(inv.analisados > 0);
        let sql = inv.por_linguagem.iter().find(|c| c.chave == "sql").unwrap();
        assert_eq!(sql.rotulo, "SQL");
        assert_eq!(sql.quantidade, 5);
        let env = inv
            .itens_ignorados
            .iter()
            .find(|i| i.caminho == ".env")
            .unwrap();
        assert_eq!(env.motivo, "segredo");
        assert_eq!(env.rotulo, "possível segredo");
        assert!(env.ajustavel);
        assert!(inv.por_motivo.iter().any(|c| c.chave == "segredo"));
        // Nada foi gravado.
        assert!(situacao(&dados).unwrap().projetos.is_empty());

        // Exclusão acrescentada pelo analista: a pasta cobol sai da análise.
        let mut ajustado = pedido(&[&fixture()]);
        ajustado.exclusoes = vec!["cobol/".into(), " ".into(), "cobol/".into()];
        let p = previa_projeto(&dados, &ajustado).unwrap();
        assert_eq!(p.exclusoes, ["cobol/"]);
        let cobol = p
            .inventario
            .itens_ignorados
            .iter()
            .find(|i| i.caminho == "cobol")
            .unwrap();
        assert!(cobol.pasta);
        assert_eq!(cobol.motivo, "usuario");
        assert!(
            !p.inventario
                .por_linguagem
                .iter()
                .any(|c| c.chave == "cobol")
        );

        // Padrão inválido é recusado com a mensagem do cadastro.
        let mut invalido = pedido(&[&fixture()]);
        invalido.exclusoes = vec!["[".into()];
        assert!(previa_projeto(&dados, &invalido).is_err());
    }

    /// Projeto de mentira: ícones PNG (binários de verdade), uma pasta de dependências
    /// com 3 arquivos e um texto com a extensão de binário.
    fn projeto_com_icones(raiz: &Path) {
        let ic = raiz.join("web/icones");
        fs::create_dir_all(&ic).unwrap();
        for n in ["a", "b", "c"] {
            fs::write(ic.join(format!("{n}.png")), b"\x89PNG\x00\x01").unwrap();
        }
        fs::write(raiz.join("web/app.ts"), "export const x = 1;\n").unwrap();
        fs::write(raiz.join("web/notas.db"), "texto simples\n").unwrap();
        let dep = raiz.join("web/node_modules/lib");
        fs::create_dir_all(&dep).unwrap();
        for n in ["1.js", "2.js", "3.js"] {
            fs::write(dep.join(n), "x").unwrap();
        }
    }

    #[test]
    fn previa_conta_em_arquivos_e_agrupa_por_pasta_e_tipo() {
        let dir = tempfile::tempdir().unwrap();
        let dados = DiretorioDados::em(dir.path().join("dados"));
        let raiz = dir.path().join("proj");
        projeto_com_icones(&raiz);
        let p = previa_projeto(&dados, &pedido(&[&raiz])).unwrap();
        let inv = &p.inventario;
        // node_modules conta os 3 arquivos, não 1 pasta; os 3 PNG e o .db também.
        assert_eq!(inv.ignorados, 3 + 3 + 1);
        assert_eq!(inv.total_itens_ignorados, 1 + 3 + 1);
        let motivo = |m: &str| inv.por_motivo.iter().find(|c| c.chave == m).unwrap();
        assert_eq!(motivo("dependencia").quantidade, 3);
        assert_eq!(motivo("binario").quantidade, 4);
        assert_eq!(
            inv.por_motivo.iter().map(|c| c.quantidade).sum::<usize>(),
            inv.ignorados
        );
        // Agrupado por pasta e tipo: os ícones são um grupo só, com um padrão só.
        let g = inv
            .grupos_ignorados
            .iter()
            .find(|g| g.tipo == "png")
            .unwrap();
        assert_eq!(
            (g.pasta.as_str(), g.itens, g.arquivos),
            ("web/icones", 3, 3)
        );
        assert_eq!(g.padroes, ["/web/icones/*.png"]);
        assert!(g.ajustavel && g.exemplos.len() == 3);
        let dep = inv
            .grupos_ignorados
            .iter()
            .find(|g| g.tipo == "pasta")
            .unwrap();
        assert_eq!((dep.pasta.as_str(), dep.itens, dep.arquivos), ("web", 1, 3));
        assert_eq!(dep.padroes, ["/web/node_modules"]);
        assert_eq!(
            inv.grupos_ignorados
                .iter()
                .map(|g| g.arquivos)
                .sum::<usize>(),
            inv.ignorados
        );
    }

    #[test]
    fn reincluir_muda_as_contagens_ou_explica_que_continua_fora() {
        let dir = tempfile::tempdir().unwrap();
        let dados = DiretorioDados::em(dir.path().join("dados"));
        let raiz = dir.path().join("proj");
        projeto_com_icones(&raiz);
        let antes = previa_projeto(&dados, &pedido(&[&raiz])).unwrap();
        let mut ped = pedido(&[&raiz]);
        // Dependências (os 3 arquivos entram), o .db de texto (entra) e os PNG
        // (binários pelo conteúdo: continuam fora).
        ped.inclusoes = vec![
            "/web/node_modules".into(),
            "/web/notas.db".into(),
            "/web/icones/*.png".into(),
        ];
        let depois = previa_projeto(&dados, &ped).unwrap();
        assert_eq!(
            depois.inventario.analisados,
            antes.inventario.analisados + 3 + 1
        );
        assert_eq!(depois.inventario.ignorados, 3);
        let v = |padrao: &str| {
            depois
                .inclusoes_vistas
                .iter()
                .find(|i| i.padrao == padrao)
                .unwrap()
        };
        assert_eq!(
            (
                v("/web/node_modules").entram,
                v("/web/node_modules").continuam_fora
            ),
            (3, 0)
        );
        assert_eq!(
            (v("/web/notas.db").entram, v("/web/notas.db").continuam_fora),
            (1, 0)
        );
        assert_eq!(
            (
                v("/web/icones/*.png").entram,
                v("/web/icones/*.png").continuam_fora
            ),
            (0, 3)
        );
    }

    #[test]
    fn grupos_escapam_curingas_e_nao_agrupam_o_que_nao_se_reinclui() {
        let item = |caminho: &str, regra: Option<&str>| Ignorado {
            caminho: caminho.into(),
            pasta: false,
            arquivos: 1,
            motivo: cba_index::Motivo::Binario,
            regra: regra.map(str::to_owned),
            detalhe: None,
            tamanho: None,
            modificado: None,
        };
        let grupos = agrupar_ignorados(&[
            item("a[1]/x.png", Some("*.png")),
            item("a[1]/y.png", Some("*.png")),
            item("LEIA", Some("LEIA")),
            item("a[1]/z.bin", None),
        ]);
        assert_eq!(grupos.len(), 3);
        let png = grupos.iter().find(|g| g.tipo == "png").unwrap();
        assert_eq!(png.padroes, ["/a[[]1[]]/*.png"]);
        let e = cba_index::Exclusoes::novas(&[], &png.padroes).unwrap();
        assert!(e.reincluido("a[1]/x.png") && !e.reincluido("a1/x.png"));
        let sem = grupos.iter().find(|g| g.tipo == "sem extensão").unwrap();
        assert_eq!(
            (sem.pasta.as_str(), sem.padroes.as_slice()),
            ("", ["/LEIA".to_owned()].as_slice())
        );
        let fixo = grupos.iter().find(|g| g.tipo == "bin").unwrap();
        assert!(!fixo.ajustavel && fixo.padroes.is_empty());
    }

    #[test]
    fn nome_em_uso_compara_pelo_identificador() {
        let dir = tempfile::tempdir().unwrap();
        let dados = DiretorioDados::em(dir.path().join("dados"));
        configurar_base(
            &dados,
            &sem_restricoes(),
            dir.path().join("base").to_str().unwrap(),
        )
        .unwrap();
        assert!(!nome_em_uso(&dados, "Faturamento Exemplo").unwrap());
        cadastrar_projeto(&dados, &pedido(&[&fixture()])).unwrap();
        assert!(nome_em_uso(&dados, "Faturamento Exemplo").unwrap());
        assert!(nome_em_uso(&dados, "  faturamento-exemplo ").unwrap());
        assert!(!nome_em_uso(&dados, "Outro").unwrap());
        assert!(!nome_em_uso(&dados, "  ").unwrap());
    }

    #[test]
    fn cadastro_com_exclusoes_ajustadas_grava_inventario_e_cria_na_base() {
        let dir = tempfile::tempdir().unwrap();
        let dados = DiretorioDados::em(dir.path().join("dados"));
        let base = dir.path().join("base");
        assert!(
            cadastrar_projeto(&dados, &pedido(&[&fixture()])).is_err(),
            "sem base"
        );
        configurar_base(&dados, &sem_restricoes(), base.to_str().unwrap()).unwrap();
        let mut p = pedido(&[&fixture()]);
        p.exclusoes = vec!["delphi/".into()];
        p.inclusoes = vec!["/.env".into()];
        let feito = cadastrar_projeto(&dados, &p).unwrap();
        assert_eq!(feito.id, "faturamento-exemplo");
        assert!(
            !feito
                .inventario
                .itens_ignorados
                .iter()
                .any(|i| i.caminho == ".env"),
            "o .env foi reincluído pelo analista"
        );
        let cadastro = Cadastro::em(dados.projetos());
        let gravado = cadastro.obter("faturamento-exemplo").unwrap().unwrap();
        assert_eq!(gravado.exclusoes, ["delphi/"]);
        assert_eq!(gravado.inclusoes, ["/.env"]);
        assert!(
            cadastro
                .inventario("faturamento-exemplo")
                .unwrap()
                .is_some()
        );
        assert!(
            base.join("faturamento-exemplo")
                .join("projeto.md")
                .is_file()
        );
        // De novo: recusado, e a prévia avisa.
        assert!(cadastrar_projeto(&dados, &pedido(&[&fixture()])).is_err());
        assert!(
            previa_projeto(&dados, &pedido(&[&fixture()]))
                .unwrap()
                .ja_cadastrado
        );
        // Com base e projeto, o primeiro uso pode ser concluído.
        concluir_primeiro_uso(&dados).unwrap();
        assert!(situacao(&dados).unwrap().primeiro_uso_concluido);
    }

    #[test]
    fn cadastro_com_varias_pastas_e_pasta_invalida() {
        let dir = tempfile::tempdir().unwrap();
        let dados = DiretorioDados::em(dir.path().join("dados"));
        configurar_base(
            &dados,
            &sem_restricoes(),
            dir.path().join("base").to_str().unwrap(),
        )
        .unwrap();
        let telas = dir.path().join("telas");
        fs::create_dir_all(&telas).unwrap();
        fs::write(telas.join("UFrmFatura.pas"), "unit UFrmFatura;\n").unwrap();
        let feito = cadastrar_projeto(&dados, &pedido(&[&fixture(), &telas])).unwrap();
        assert_eq!(feito.pastas.len(), 2);
        assert_eq!(feito.pastas[1].prefixo.as_deref(), Some("telas"));
        let mut sem_pastas = pedido(&[]);
        sem_pastas.nome = "Outro".into();
        assert!(previa_projeto(&dados, &sem_pastas).is_err());
        let mut relativa = sem_pastas.clone();
        relativa.pastas = vec!["relativa".into()];
        assert!(previa_projeto(&dados, &relativa).is_err());
        let mut inexistente = sem_pastas;
        inexistente.pastas = vec![dir.path().join("nao-existe").to_str().unwrap().into()];
        assert!(previa_projeto(&dados, &inexistente).is_err());
    }
}
