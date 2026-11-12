//! Proteção técnica opcional da pasta do sistema (spec, princípios; #29).
//!
//! Desligada por padrão: o app avisa em vez de bloquear ([`crate::alteracoes`]).
//! Ligada (nas configurações, #39, ou no preparo do ambiente), nega a edição da
//! pasta do sistema mesmo que o analista aprove:
//!
//! - **`Edit`, `Write` e `NotebookEdit`:** regras `deny` com o caminho absoluto de
//!   cada pasta no `~/.claude/settings.json` ([`crate::preparo::protecao`]).
//! - **`Bash` e `PowerShell`:** o receptor de hooks responde `deny` no `PreToolUse`
//!   dessas ferramentas nas sessões do sistema ([`protegido`]). O Claude Code não tem
//!   regra de Bash por pasta, e uma regra `Bash` pura tiraria a ferramenta de todas
//!   as sessões do analista. O hook decide antes da pergunta de permissão, então a
//!   aprovação do analista não chega a ser pedida.
//!
//! O registro `<dados>/protecao/<sistema>.json` diz que a proteção está ligada (o
//! hook só confere se ele existe) e guarda as regras que o app acrescentou: desligar
//! tira só essas, nunca uma regra igual que o analista já tinha.

use std::fs;
use std::io;
use std::path::PathBuf;

use cba_core::dados::DiretorioDados;
use cba_core::tempo;
use serde::{Deserialize, Serialize};

use crate::preparo::{Erro, Preparador};

/// Pasta dos registros de proteção no diretório de dados.
pub const PASTA: &str = "protecao";
const VERSAO_DO_FORMATO: u32 = 1;
/// Ferramentas negadas pelo hook nas sessões de um sistema protegido.
pub const FERRAMENTAS_NEGADAS_PELO_HOOK: &[&str] = &["Bash", "PowerShell"];

/// Motivo que o agente recebe quando o hook nega a ferramenta.
pub const MOTIVO: &str = "A proteção contra edição está ligada para este sistema no ctower: \
comandos de shell estão desligados nesta pasta. Use as ferramentas do cba e a leitura de \
arquivos, e registre o que mudaria como sugestão de alteração.";

/// Registro da proteção ligada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistroProtecao {
    pub formato: u32,
    pub sistema: String,
    /// Regras que o app acrescentou (e que desligar tira).
    pub adicionadas: Vec<String>,
    pub ligada_ms: u64,
}

/// Uma regra da proteção e onde ela está.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegraProtecao {
    pub regra: String,
    /// Está no `settings.json`.
    pub presente: bool,
    /// Foi o app que a acrescentou.
    pub do_app: bool,
}

/// Situação da proteção de um sistema, para as configurações.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SituacaoProtecao {
    pub sistema: String,
    /// Registro presente: o hook nega o Bash nas sessões do sistema.
    pub ligada: bool,
    /// Ligada e com todas as regras no `settings.json`.
    pub completa: bool,
    /// `~/.claude/settings.json`.
    pub arquivo: PathBuf,
    pub regras: Vec<RegraProtecao>,
    /// Ferramentas negadas pelo hook quando ligada.
    pub negadas_pelo_hook: Vec<String>,
}

fn arquivo(dados: &DiretorioDados, sistema: &str) -> Option<PathBuf> {
    crate::sessoes::id_valido(sistema)
        .then(|| dados.raiz().join(PASTA).join(format!("{sistema}.json")))
}

fn erro_io(arquivo: PathBuf, e: io::Error) -> Erro {
    Erro::Arquivo {
        arquivo,
        tipo: e.kind(),
    }
}

/// Se a proteção do sistema está ligada (só confere o arquivo; para o hook).
pub fn protegido(dados: &DiretorioDados, sistema: &str) -> bool {
    arquivo(dados, sistema).is_some_and(|a| a.is_file())
}

/// Registro da proteção, se ligada.
pub fn registro(dados: &DiretorioDados, sistema: &str) -> Option<RegistroProtecao> {
    let bytes = fs::read(arquivo(dados, sistema)?).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn pastas(sistema: &cba_index::Sistema) -> Vec<PathBuf> {
    sistema.pastas.iter().map(|p| p.caminho.clone()).collect()
}

/// Situação da proteção, sem gravar nada.
pub fn situacao(
    dados: &DiretorioDados,
    preparador: &Preparador,
    sistema: &cba_index::Sistema,
) -> Result<SituacaoProtecao, Erro> {
    let registro = registro(dados, &sistema.id);
    let ligada = protegido(dados, &sistema.id);
    let esperadas = preparador.regras_de_protecao(&pastas(sistema));
    let presentes = preparador.negacoes_presentes(&esperadas)?;
    let regras: Vec<RegraProtecao> = esperadas
        .into_iter()
        .zip(presentes)
        .map(|(regra, presente)| RegraProtecao {
            do_app: registro
                .as_ref()
                .is_some_and(|r| r.adicionadas.contains(&regra)),
            regra,
            presente,
        })
        .collect();
    Ok(SituacaoProtecao {
        sistema: sistema.id.clone(),
        completa: ligada && regras.iter().all(|r| r.presente),
        ligada,
        arquivo: preparador.config.settings.clone(),
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
    sistema: &cba_index::Sistema,
) -> Result<SituacaoProtecao, Erro> {
    let destino =
        arquivo(dados, &sistema.id).ok_or_else(|| Erro::Pasta(PathBuf::from(&sistema.id)))?;
    let regras = preparador.regras_de_protecao(&pastas(sistema));
    let feito = preparador.negar(&regras)?;
    let mut adicionadas = registro(dados, &sistema.id)
        .map(|r| r.adicionadas)
        .unwrap_or_default();
    for r in feito.alteradas {
        if !adicionadas.contains(&r) {
            adicionadas.push(r);
        }
    }
    let novo = RegistroProtecao {
        formato: VERSAO_DO_FORMATO,
        sistema: sistema.id.clone(),
        adicionadas,
        ligada_ms: tempo::agora_ms(),
    };
    let mut json = serde_json::to_vec_pretty(&novo)
        .map_err(io::Error::other)
        .map_err(|e| erro_io(destino.clone(), e))?;
    json.push(b'\n');
    cba_kb::arquivo::gravar(&destino, &json).map_err(|e| erro_io(destino.clone(), e))?;
    tracing::info!(sistema = sistema.id, "proteção contra edição ligada");
    situacao(dados, preparador, sistema)
}

/// Desliga a proteção: tira do `settings.json` só as regras que o app acrescentou e
/// apaga o registro. Também serve ao descadastro do sistema.
pub fn desligar(
    dados: &DiretorioDados,
    preparador: &Preparador,
    sistema: &str,
) -> Result<Vec<String>, Erro> {
    let Some(destino) = arquivo(dados, sistema) else {
        return Ok(Vec::new());
    };
    let Some(reg) = registro(dados, sistema) else {
        // Registro ilegível ou ausente: nada é tirado do settings.json.
        match fs::remove_file(&destino) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(erro_io(destino, e)),
        }
        return Ok(Vec::new());
    };
    let feito = preparador.liberar(&reg.adicionadas)?;
    match fs::remove_file(&destino) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(erro_io(destino, e)),
    }
    tracing::info!(sistema, "proteção contra edição desligada");
    Ok(feito.alteradas)
}
