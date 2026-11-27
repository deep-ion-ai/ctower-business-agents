//! Histórico git sintético de uma fixture (#450).
//!
//! A cópia da fixture vira um repositório git com um commit só. Para avaliar a
//! leitura do histórico (`historico_do_trecho`), um item do gabarito pode pedir um
//! histórico sintético (`origem.historico`): a pasta `fixtures/historicos/<nome>/`
//! tem o `historico.yaml`, com os commits em ordem, e as versões anteriores dos
//! arquivos que eles mudam:
//!
//! ```yaml
//! commits:
//!   - data: 2023-02-06
//!     mensagem: Carga inicial do faturamento
//!     versoes:
//!       db/03_pkg_cobranca.sql: versoes/03_pkg_cobranca.v1.sql
//!   - data: 2024-03-12
//!     mensagem: "[DEM-12] Impedir o cancelamento de fatura com pagamento parcial"
//!     versoes:
//!       db/03_pkg_cobranca.sql: atual
//! ```
//!
//! O primeiro commit é a fixture inteira, com os arquivos de `versoes` na versão
//! indicada; cada commit seguinte troca os arquivos que lista (`atual` é o
//! arquivo como está na fixture). O último estado de cada arquivo listado é
//! sempre `atual`: o histórico termina exatamente na fixture, e as linhas citadas
//! pelo gabarito continuam valendo ([`Historico::validar`]).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

/// Nome do arquivo do histórico na pasta.
pub const ARQUIVO: &str = "historico.yaml";
/// Versão que é o arquivo como está na fixture.
pub const ATUAL: &str = "atual";

/// Os commits do histórico, do mais antigo ao mais recente.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Historico {
    pub commits: Vec<CommitSintetico>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommitSintetico {
    /// `AAAA-MM-DD`.
    pub data: String,
    /// Mensagem inteira (assunto e corpo).
    pub mensagem: String,
    /// Arquivo do projeto → versão (caminho relativo à pasta do histórico, ou
    /// [`ATUAL`]).
    #[serde(default)]
    pub versoes: BTreeMap<String, String>,
}

fn caminho(base: &Path, relativo: &str) -> PathBuf {
    relativo
        .split('/')
        .fold(base.to_path_buf(), |c, p| c.join(p))
}

impl Historico {
    /// Lê `historico.yaml` da pasta.
    pub fn ler(pasta: &Path) -> Result<Self, String> {
        let arquivo = pasta.join(ARQUIVO);
        let texto = fs::read_to_string(&arquivo)
            .map_err(|e| format!("histórico {}: {e}", arquivo.display()))?;
        serde_norway::from_str(&texto)
            .map_err(|e| format!("histórico {}: YAML inválido: {e}", arquivo.display()))
    }

    /// Confere o histórico contra a fixture: ao menos um commit, datas no formato,
    /// versões que existem, arquivos que existem na fixture e todo arquivo listado
    /// terminando em [`ATUAL`].
    pub fn validar(&self, pasta: &Path, projeto: &Path) -> Result<(), String> {
        if self.commits.is_empty() {
            return Err("o histórico não tem commits".into());
        }
        let mut ultima: BTreeMap<&str, &str> = BTreeMap::new();
        for (i, c) in self.commits.iter().enumerate() {
            let n = i + 1;
            let data_ok = c.data.len() == 10
                && c.data.bytes().enumerate().all(|(j, b)| {
                    if j == 4 || j == 7 {
                        b == b'-'
                    } else {
                        b.is_ascii_digit()
                    }
                });
            if !data_ok {
                return Err(format!(
                    "commit {n}: data `{}` fora do formato AAAA-MM-DD",
                    c.data
                ));
            }
            if c.mensagem.trim().is_empty() {
                return Err(format!("commit {n}: mensagem vazia"));
            }
            if i > 0 && c.versoes.is_empty() {
                return Err(format!("commit {n}: não muda nenhum arquivo"));
            }
            for (arquivo, versao) in &c.versoes {
                if !caminho(projeto, arquivo).is_file() {
                    return Err(format!("commit {n}: `{arquivo}` não existe na fixture"));
                }
                if versao != ATUAL && !caminho(pasta, versao).is_file() {
                    return Err(format!("commit {n}: a versão `{versao}` não existe"));
                }
                if i == 0 && versao == ATUAL {
                    return Err(format!(
                        "commit 1: `{arquivo}` já entra como está na fixture; liste só as \
                         versões anteriores"
                    ));
                }
                ultima.insert(arquivo.as_str(), versao.as_str());
            }
        }
        for (arquivo, versao) in ultima {
            if versao != ATUAL {
                return Err(format!(
                    "`{arquivo}` termina na versão `{versao}`, e não na da fixture: o último \
                     commit que o muda deve usar `{ATUAL}`"
                ));
            }
        }
        Ok(())
    }

    /// Reproduz o histórico em `codigo`, uma cópia da fixture `projeto` ainda sem
    /// git: cria o repositório e faz os commits, com a data de cada um. No fim, a
    /// cópia é igual à fixture.
    pub fn reproduzir(&self, pasta: &Path, projeto: &Path, codigo: &Path) -> Result<(), String> {
        self.validar(pasta, projeto)?;
        git(codigo, &["init", "-q"], None)?;
        for c in &self.commits {
            for (arquivo, versao) in &c.versoes {
                let origem = if versao == ATUAL {
                    caminho(projeto, arquivo)
                } else {
                    caminho(pasta, versao)
                };
                fs::copy(&origem, caminho(codigo, arquivo))
                    .map_err(|e| format!("histórico: cópia de {}: {e}", origem.display()))?;
            }
            git(codigo, &["add", "-A"], None)?;
            git(
                codigo,
                &["commit", "-q", "--allow-empty", "-m", c.mensagem.trim()],
                Some(c.data.as_str()),
            )?;
        }
        Ok(())
    }
}

fn git(pasta: &Path, args: &[&str], data: Option<&str>) -> Result<(), String> {
    let mut c = Command::new("git");
    c.args([
        "-c",
        "user.name=Equipe Faturamento Exemplo",
        "-c",
        "user.email=equipe@exemplo.com",
        "-c",
        "commit.gpgsign=false",
        "-c",
        "core.autocrlf=false",
    ])
    .args(args)
    .current_dir(pasta);
    if let Some(d) = data {
        let instante = format!("{d}T12:00:00+00:00");
        c.env("GIT_AUTHOR_DATE", &instante)
            .env("GIT_COMMITTER_DATE", &instante);
    }
    let saida = c.output().map_err(|e| format!("git: {e}"))?;
    if !saida.status.success() {
        return Err(format!(
            "git {}: {}",
            args.first().copied().unwrap_or_default(),
            String::from_utf8_lossy(&saida.stderr).trim()
        ));
    }
    Ok(())
}
