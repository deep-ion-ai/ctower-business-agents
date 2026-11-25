//! Migração das instalações anteriores à #304: as entradas que o app gravou no
//! `settings.json` do usuário passam para o `.claude/settings.local.json` das pastas
//! dos projetos cadastrados (#305, épico #302).
//!
//! Roda na abertura do app ([`migrar`]) até terminar uma vez:
//!
//! 1. Para cada projeto cadastrado com o ambiente preparado por esta instalação
//!    (servidor MCP dela na pasta), prepara de novo ([`Preparador::preparar`]): os
//!    hooks vão para os arquivos locais das pastas. Com a proteção ligada no formato
//!    antigo, as regras `deny` também vão para os arquivos locais e só então saem do
//!    `settings.json` ([`crate::protecao::migrar`]).
//! 2. Só se tudo isso deu certo, tira do `settings.json` exatamente os hooks que esta
//!    instalação gravou lá ([`Preparador::retirar_hooks_globais`]), devolvendo o valor
//!    de antes pelo registro `.cba-anterior`. Os hooks do analista, de outras
//!    instalações e os nossos que o analista mudou ficam.
//! 3. Grava o estado em `<dados>/preparo/migracao-pastas.json`, com o aviso para o
//!    analista, mostrado uma vez ([`marcar_visto`]).
//!
//! Cada passo é idempotente: interrompida no meio (falha ao gravar, app fechado), a
//! próxima abertura refaz o que falta sem duplicar. Enquanto a entrada global e a
//! local coexistem, o Claude Code roda o comando uma vez só (os dois são idênticos;
//! spike da #303). A configuração é a que [`Preparador::padrao`] acha, inclusive com
//! `CLAUDE_CONFIG_DIR`.
//!
//! Um projeto cuja pasta não está mais acessível não impede a migração: fica em
//! [`AvisoMigracao::pendentes`] e será preparado quando o analista abrir o terminal.
//!
//! **Eventos novos** (#370): o estado guarda os eventos de [`EVENTOS`] que a
//! migração levou às pastas. Quando o app passa a registrar eventos novos (o
//! `PostToolUse` da ferramenta `Workflow`, o `SubagentStart` e o `SubagentStop`), a
//! abertura seguinte refaz só o passo 1, sem aviso: os hooks dos eventos que faltam
//! entram, e os que já estão ficam como estão, sem duplicar. A migração só leva os
//! hooks ([`Alvo::com_workflows`] vazio): os workflows e as regras de permissão
//! deles são itens novos do plano, que pedem o consentimento do analista no preparo.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use cba_core::dados::DiretorioDados;
use cba_core::tempo;
use cba_index::Cadastro;
use serde::{Deserialize, Serialize};

use crate::hooks::EVENTOS;
use crate::preparo::{Acao, Alvo, Erro, Item, Preparador, Situacao};
use crate::protecao;

/// Estado da migração, em `<dados>/<ARQUIVO>`.
pub const ARQUIVO: &str = "preparo/migracao-pastas.json";
const VERSAO_DO_FORMATO: u32 = 1;

/// Mensagem do aviso único.
pub const MENSAGEM: &str =
    "As configurações do CBA no Claude Code agora valem só nas pastas dos projetos cadastrados";

/// Um projeto que ficou para depois.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pendente {
    pub projeto: String,
    pub motivo: String,
}

/// O que a migração mudou, para o aviso ao analista.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AvisoMigracao {
    pub mensagem: String,
    /// Projetos preparados de novo, nas pastas.
    pub projetos: Vec<String>,
    /// Arquivos locais criados ou alterados.
    pub arquivos_locais: Vec<PathBuf>,
    /// O `settings.json` do usuário.
    pub settings: PathBuf,
    /// Eventos cujos hooks do app saíram do `settings.json`.
    pub eventos_retirados: Vec<String>,
    /// Regras `deny` da proteção que saíram do `settings.json` (foram para as pastas).
    pub regras_retiradas: Vec<String>,
    /// Eventos em que ficou no `settings.json` um hook do app que o analista mudou:
    /// o app não o tocou.
    pub ficaram: Vec<String>,
    pub pendentes: Vec<Pendente>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Estado {
    formato: u32,
    concluida_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    aviso: Option<AvisoMigracao>,
    #[serde(default)]
    visto: bool,
    /// Eventos cujos hooks a migração levou às pastas (#370). Ausente nos estados
    /// gravados antes: a abertura seguinte leva os eventos que faltam.
    #[serde(default)]
    eventos: Vec<String>,
}

/// Uma migração por vez no processo (a tela pode pedir duas vezes ao abrir).
static EM_CURSO: Mutex<()> = Mutex::new(());

fn caminho(dados: &DiretorioDados) -> PathBuf {
    dados.raiz().join(ARQUIVO)
}

fn erro_io(arquivo: &Path, e: io::Error) -> Erro {
    Erro::Arquivo {
        arquivo: arquivo.to_path_buf(),
        tipo: e.kind(),
    }
}

fn ler(dados: &DiretorioDados) -> Option<Estado> {
    let bytes = fs::read(caminho(dados)).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn gravar(dados: &DiretorioDados, estado: &Estado) -> Result<(), Erro> {
    let destino = caminho(dados);
    let mut json = serde_json::to_vec_pretty(estado)
        .map_err(io::Error::other)
        .map_err(|e| erro_io(&destino, e))?;
    json.push(b'\n');
    cba_kb::arquivo::gravar(&destino, &json).map_err(|e| erro_io(&destino, e))
}

/// Se a migração já terminou nesta instalação.
pub fn concluida(dados: &DiretorioDados) -> bool {
    ler(dados).is_some()
}

fn cadastrados(dados: &DiretorioDados) -> Result<Vec<cba_index::Projeto>, Erro> {
    Cadastro::em(dados.projetos())
        .listar()
        .map_err(|e| erro_io(&dados.projetos(), io::Error::other(e.to_string())))
}

/// Eventos atuais, como vão no estado.
fn eventos_atuais() -> Vec<String> {
    EVENTOS.iter().map(|e| e.to_string()).collect()
}

/// A migração foi feita com menos eventos do que o app registra hoje.
fn faltam_eventos(estado: &Estado) -> bool {
    EVENTOS
        .iter()
        .any(|e| !estado.eventos.iter().any(|x| x == e))
}

/// Prepara de novo, só com os hooks, o projeto com o ambiente preparado por esta
/// instalação (servidor MCP dela na pasta), anotando em `aviso` o que mudou.
/// Devolve `false` se a pasta do projeto está inacessível (fica pendente).
fn levar_hooks(
    preparador: &Preparador,
    projeto: &cba_index::Projeto,
    aviso: &mut AvisoMigracao,
) -> Result<bool, Erro> {
    let alvo = match Alvo::do_projeto(projeto) {
        // Só os hooks: os workflows pedem consentimento no preparo.
        Ok(a) => a.com_workflows(Vec::new()),
        Err(Erro::Pasta(_)) => {
            // Sem a pasta, não há sessão a servir; o terminal prepara quando ela
            // voltar.
            aviso.pendentes.push(Pendente {
                projeto: projeto.id.clone(),
                motivo: "pasta do projeto inacessível".into(),
            });
            return Ok(false);
        }
        Err(e) => return Err(e),
    };
    if preparador.verificar(&alvo)?.servidor_mcp == Situacao::Ok {
        let feito = preparador.preparar(&alvo)?;
        for a in &feito.alteracoes {
            if matches!(a.item, Item::Hook { .. })
                && a.acao != Acao::SemMudanca
                && !aviso.arquivos_locais.contains(&a.arquivo)
            {
                aviso.arquivos_locais.push(a.arquivo.clone());
            }
        }
        aviso.projetos.push(projeto.id.clone());
    }
    Ok(true)
}

/// Migra, se ainda não migrou. Devolve o aviso, enquanto o analista não o viu (sem
/// nada a migrar, não há aviso). Com erro, nada foi tirado do `settings.json` e a
/// próxima chamada tenta de novo. Já migrada, leva às pastas os hooks dos eventos
/// novos de [`EVENTOS`], se houver (#370), sem aviso novo.
pub fn migrar(
    dados: &DiretorioDados,
    preparador: &Preparador,
) -> Result<Option<AvisoMigracao>, Erro> {
    let _vez = EM_CURSO.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(mut estado) = ler(dados) {
        if faltam_eventos(&estado) {
            let mut levados = AvisoMigracao::default();
            for projeto in &cadastrados(dados)? {
                levar_hooks(preparador, projeto, &mut levados)?;
            }
            estado.eventos = eventos_atuais();
            gravar(dados, &estado)?;
            tracing::info!(
                projetos = levados.projetos.len(),
                "hooks dos eventos novos levados às pastas"
            );
        }
        return Ok(estado.aviso.filter(|_| !estado.visto));
    }
    let cadastrados = cadastrados(dados)?;

    let mut aviso = AvisoMigracao {
        mensagem: MENSAGEM.to_string(),
        settings: preparador.config.settings.clone(),
        ..Default::default()
    };
    for projeto in &cadastrados {
        if levar_hooks(preparador, projeto, &mut aviso)? {
            aviso
                .regras_retiradas
                .extend(protecao::migrar(dados, preparador, projeto)?);
        }
    }

    // Só depois de as pastas terem as entradas novas.
    let retirada = preparador.retirar_hooks_globais()?;
    aviso.eventos_retirados = retirada
        .alteracoes
        .iter()
        .filter_map(|a| match &a.item {
            Item::Hook { evento } => Some(evento.clone()),
            _ => None,
        })
        .collect();
    aviso.ficaram = retirada.ficaram;

    let houve = !aviso.eventos_retirados.is_empty()
        || !aviso.regras_retiradas.is_empty()
        || !aviso.ficaram.is_empty();
    let estado = Estado {
        formato: VERSAO_DO_FORMATO,
        concluida_ms: tempo::agora_ms(),
        aviso: houve.then_some(aviso),
        visto: false,
        eventos: eventos_atuais(),
    };
    gravar(dados, &estado)?;
    tracing::info!(
        projetos = estado.aviso.as_ref().map_or(0, |a| a.projetos.len()),
        mudou = houve,
        "migração das configurações para as pastas concluída"
    );
    Ok(estado.aviso)
}

/// O analista viu o aviso: não aparece de novo.
pub fn marcar_visto(dados: &DiretorioDados) -> Result<(), Erro> {
    let _vez = EM_CURSO.lock().unwrap_or_else(|e| e.into_inner());
    let Some(mut estado) = ler(dados) else {
        return Ok(());
    };
    if estado.visto {
        return Ok(());
    }
    estado.visto = true;
    gravar(dados, &estado)
}

#[cfg(test)]
mod tests;
