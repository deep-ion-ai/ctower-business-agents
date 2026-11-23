//! Proteção técnica opcional da pasta do projeto (spec, princípios; #29).
//!
//! Desligada por padrão: o app avisa em vez de bloquear ([`crate::alteracoes`]).
//! Ligada (nas configurações, #39, ou no preparo do ambiente), nega a edição da
//! pasta do projeto mesmo que o analista aprove:
//!
//! - **`Edit`, `Write` e `NotebookEdit`:** regras `deny` com o caminho absoluto de
//!   cada pasta no `.claude/settings.local.json` da raiz da pasta
//!   ([`crate::preparo::protecao`], #304).
//! - **`Bash` e `PowerShell`:** o receptor de hooks responde `deny` no `PreToolUse`
//!   dessas ferramentas nas sessões do projeto ([`protegido`]). O Claude Code não tem
//!   regra de Bash por pasta, e uma regra `Bash` pura tiraria a ferramenta de todas
//!   as sessões do analista. O hook decide antes da pergunta de permissão, então a
//!   aprovação do analista não chega a ser pedida.
//!
//! O registro `<dados>/protecao/<projeto>.json` diz que a proteção está ligada (o
//! hook só confere se ele existe) e guarda as regras que o app acrescentou, cada uma
//! com o arquivo em que está: desligar tira só essas, nunca uma regra igual que o
//! analista já tinha. No formato 1 (anterior à #304) as regras eram só texto e
//! estavam no `settings.json` do usuário ([`RegraAdicionada::arquivo`] `None`).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use cba_core::dados::DiretorioDados;
use cba_core::tempo;
use serde::{Deserialize, Serialize};

use crate::preparo::{Erro, Preparador};

/// Pasta dos registros de proteção no diretório de dados.
pub const PASTA: &str = "protecao";
const VERSAO_DO_FORMATO: u32 = 2;
/// Ferramentas negadas pelo hook nas sessões de um projeto protegido.
pub const FERRAMENTAS_NEGADAS_PELO_HOOK: &[&str] = &["Bash", "PowerShell"];

/// Motivo que o agente recebe quando o hook nega a ferramenta.
pub const MOTIVO: &str = "A proteção contra edição está ligada para este projeto no ctower: \
comandos de shell estão desligados nesta pasta. Use as ferramentas do cba e a leitura de \
arquivos, e registre o que mudaria como sugestão de alteração.";

/// Regra que o app acrescentou e o arquivo em que ela está.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "FormatoRegra")]
pub struct RegraAdicionada {
    pub regra: String,
    /// `None`: o `settings.json` do usuário (registro do formato 1, anterior à #304).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arquivo: Option<PathBuf>,
}

/// Uma regra no registro: texto (formato 1) ou com o arquivo.
#[derive(Deserialize)]
#[serde(untagged)]
enum FormatoRegra {
    Texto(String),
    Completa {
        regra: String,
        #[serde(default)]
        arquivo: Option<PathBuf>,
    },
}

impl From<FormatoRegra> for RegraAdicionada {
    fn from(f: FormatoRegra) -> Self {
        match f {
            FormatoRegra::Texto(regra) => Self {
                regra,
                arquivo: None,
            },
            FormatoRegra::Completa { regra, arquivo } => Self { regra, arquivo },
        }
    }
}

/// Registro da proteção ligada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistroProtecao {
    pub formato: u32,
    pub projeto: String,
    /// Regras que o app acrescentou (e que desligar tira).
    pub adicionadas: Vec<RegraAdicionada>,
    pub ligada_ms: u64,
}

impl RegistroProtecao {
    /// A regra foi acrescentada pelo app em `arquivo` (`settings` é o do usuário,
    /// para as regras do formato 1).
    fn tem(&self, regra: &str, arquivo: &Path, settings: &Path) -> bool {
        self.adicionadas
            .iter()
            .any(|a| a.regra == regra && a.arquivo.as_deref().unwrap_or(settings) == arquivo)
    }
}

/// Uma regra da proteção e onde ela está.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegraProtecao {
    pub regra: String,
    /// O `settings.local.json` em que a regra vai.
    pub arquivo: PathBuf,
    /// Está no arquivo.
    pub presente: bool,
    /// Foi o app que a acrescentou.
    pub do_app: bool,
}

/// Situação da proteção de um projeto, para as configurações.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SituacaoProtecao {
    pub projeto: String,
    /// Registro presente: o hook nega o Bash nas sessões do projeto.
    pub ligada: bool,
    /// Ligada e com todas as regras no lugar.
    pub completa: bool,
    /// Arquivo da primeira pasta (o das outras está em cada regra).
    pub arquivo: PathBuf,
    pub regras: Vec<RegraProtecao>,
    /// Ferramentas negadas pelo hook quando ligada.
    pub negadas_pelo_hook: Vec<String>,
}

fn arquivo(dados: &DiretorioDados, projeto: &str) -> Option<PathBuf> {
    crate::sessoes::id_valido(projeto)
        .then(|| dados.raiz().join(PASTA).join(format!("{projeto}.json")))
}

fn erro_io(arquivo: PathBuf, e: io::Error) -> Erro {
    Erro::Arquivo {
        arquivo,
        tipo: e.kind(),
    }
}

/// Se a proteção do projeto está ligada (só confere o arquivo; para o hook).
pub fn protegido(dados: &DiretorioDados, projeto: &str) -> bool {
    arquivo(dados, projeto).is_some_and(|a| a.is_file())
}

/// Registro da proteção, se ligada.
pub fn registro(dados: &DiretorioDados, projeto: &str) -> Option<RegistroProtecao> {
    let bytes = fs::read(arquivo(dados, projeto)?).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn pastas(projeto: &cba_index::Projeto) -> Vec<PathBuf> {
    projeto.pastas.iter().map(|p| p.caminho.clone()).collect()
}

/// Situação da proteção, sem gravar nada.
pub fn situacao(
    dados: &DiretorioDados,
    preparador: &Preparador,
    projeto: &cba_index::Projeto,
) -> Result<SituacaoProtecao, Erro> {
    let registro = registro(dados, &projeto.id);
    let ligada = protegido(dados, &projeto.id);
    let mut regras = Vec::new();
    for (arquivo, regra) in preparador.regras_por_arquivo(&pastas(projeto)) {
        let presente = preparador.negacoes_presentes_em(&arquivo, std::slice::from_ref(&regra))?[0];
        regras.push(RegraProtecao {
            do_app: registro
                .as_ref()
                .is_some_and(|r| r.tem(&regra, &arquivo, &preparador.config.settings)),
            regra,
            arquivo,
            presente,
        });
    }
    Ok(SituacaoProtecao {
        projeto: projeto.id.clone(),
        completa: ligada && regras.iter().all(|r| r.presente),
        ligada,
        arquivo: regras
            .first()
            .map(|r| r.arquivo.clone())
            .unwrap_or_else(|| preparador.config.settings.clone()),
        regras,
        negadas_pelo_hook: FERRAMENTAS_NEGADAS_PELO_HOOK
            .iter()
            .map(|s| s.to_string())
            .collect(),
    })
}

/// Liga a proteção: grava as regras `deny` que faltam e o registro. Idempotente.
pub fn ligar(
    dados: &DiretorioDados,
    preparador: &Preparador,
    projeto: &cba_index::Projeto,
) -> Result<SituacaoProtecao, Erro> {
    let destino =
        arquivo(dados, &projeto.id).ok_or_else(|| Erro::Pasta(PathBuf::from(&projeto.id)))?;
    let mut adicionadas = registro(dados, &projeto.id)
        .map(|r| r.adicionadas)
        .unwrap_or_default();
    let mut por_arquivo: Vec<(PathBuf, Vec<String>)> = Vec::new();
    for (arquivo, regra) in preparador.regras_por_arquivo(&pastas(projeto)) {
        match por_arquivo.iter_mut().find(|(a, _)| *a == arquivo) {
            Some((_, v)) => v.push(regra),
            None => por_arquivo.push((arquivo, vec![regra])),
        }
    }
    for (arquivo, regras) in por_arquivo {
        let feito = preparador.negar_em(&arquivo, &regras)?;
        for regra in feito.alteradas {
            let nova = RegraAdicionada {
                regra,
                arquivo: Some(arquivo.clone()),
            };
            if !adicionadas.contains(&nova) {
                adicionadas.push(nova);
            }
        }
    }
    let novo = RegistroProtecao {
        formato: VERSAO_DO_FORMATO,
        projeto: projeto.id.clone(),
        adicionadas,
        ligada_ms: tempo::agora_ms(),
    };
    let mut json = serde_json::to_vec_pretty(&novo)
        .map_err(io::Error::other)
        .map_err(|e| erro_io(destino.clone(), e))?;
    json.push(b'\n');
    cba_kb::arquivo::gravar(&destino, &json).map_err(|e| erro_io(destino.clone(), e))?;
    tracing::info!(projeto = projeto.id, "proteção contra edição ligada");
    situacao(dados, preparador, projeto)
}

/// Desliga a proteção: tira de cada arquivo só as regras que o app acrescentou nele e
/// apaga o registro. Também serve ao descadastro do projeto.
pub fn desligar(
    dados: &DiretorioDados,
    preparador: &Preparador,
    projeto: &str,
) -> Result<Vec<String>, Erro> {
    let Some(destino) = arquivo(dados, projeto) else {
        return Ok(Vec::new());
    };
    let Some(reg) = registro(dados, projeto) else {
        // Registro ilegível ou ausente: nenhuma regra é tirada.
        match fs::remove_file(&destino) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(erro_io(destino, e)),
        }
        return Ok(Vec::new());
    };
    let mut por_arquivo: Vec<(PathBuf, Vec<String>)> = Vec::new();
    for a in reg.adicionadas {
        let arquivo = a
            .arquivo
            .unwrap_or_else(|| preparador.config.settings.clone());
        match por_arquivo.iter_mut().find(|(x, _)| *x == arquivo) {
            Some((_, v)) => v.push(a.regra),
            None => por_arquivo.push((arquivo, vec![a.regra])),
        }
    }
    let mut tiradas = Vec::new();
    for (arquivo, regras) in por_arquivo {
        tiradas.extend(preparador.liberar_em(&arquivo, &regras)?.alteradas);
    }
    match fs::remove_file(&destino) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(erro_io(destino, e)),
    }
    tracing::info!(projeto, "proteção contra edição desligada");
    Ok(tiradas)
}
