//! Notificação ao app a cada mudança numa análise, para os painéis ao vivo.
//!
//! O servidor MCP e o receptor de hooks são processos separados do app (spec, seção
//! 5). Cada mudança gravada vira uma linha JSON acrescentada ao diário do sistema,
//! em `<dados>/eventos/<sistema>.jsonl`; o app acompanha o arquivo e relê a análise
//! citada. O diário guarda só identificadores (qual análise, que tipo de mudança,
//! versão, processo), nunca conteúdo do sistema analisado.
//!
//! Falha ao notificar nunca desfaz nem impede a gravação: o estado já está na base,
//! e o app o encontra ao reler.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Tipo de mudança numa análise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mudanca {
    /// Análise criada.
    Criada,
    /// Mudança de estado da análise (concluir, aprovar).
    Estado,
    /// Sessão ligada ou com estado novo.
    Sessao,
    Afirmacao,
    Pergunta,
    Sugestao,
    /// Termo proposto para o glossário.
    Termo,
}

/// Uma linha do diário.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evento {
    pub quando_ms: u64,
    /// Slug do sistema.
    pub sistema: String,
    /// Identificador da análise (`A-0001`).
    pub analise: String,
    pub mudanca: Mudanca,
    /// Versão da análise depois da mudança.
    pub versao: u64,
    /// Processo que gravou (app, `cba mcp` ou `cba hook`).
    pub processo: u32,
}

/// Destino das notificações.
pub trait Notificador: Send + Sync + std::fmt::Debug {
    fn notificar(&self, evento: &Evento) -> io::Result<()>;
}

/// Não notifica ninguém (testes e ferramentas de linha de comando).
#[derive(Debug, Clone, Copy, Default)]
pub struct SemNotificacao;

impl Notificador for SemNotificacao {
    fn notificar(&self, _: &Evento) -> io::Result<()> {
        Ok(())
    }
}

/// Diário de eventos em arquivos JSONL, um por sistema.
#[derive(Debug, Clone)]
pub struct DiarioEventos {
    pasta: PathBuf,
}

impl DiarioEventos {
    /// Diário na pasta informada (normalmente `DiretorioDados::eventos`).
    pub fn em(pasta: impl Into<PathBuf>) -> Self {
        Self {
            pasta: pasta.into(),
        }
    }

    /// Arquivo do diário de um sistema.
    pub fn arquivo(&self, sistema: &str) -> PathBuf {
        self.pasta.join(format!("{sistema}.jsonl"))
    }

    /// Eventos já gravados de um sistema, na ordem. Linhas ilegíveis (uma gravação
    /// interrompida) são ignoradas.
    pub fn ler(&self, sistema: &str) -> io::Result<Vec<Evento>> {
        ler(&self.arquivo(sistema))
    }
}

fn ler(arquivo: &Path) -> io::Result<Vec<Evento>> {
    let texto = match fs::read_to_string(arquivo) {
        Ok(t) => t,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    Ok(texto
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect())
}

impl Notificador for DiarioEventos {
    fn notificar(&self, evento: &Evento) -> io::Result<()> {
        fs::create_dir_all(&self.pasta)?;
        let mut linha = serde_json::to_vec(evento).map_err(io::Error::other)?;
        linha.push(b'\n');
        // No Windows, a trava (LockFileEx) exige acesso de leitura ou de escrita
        // completa; um arquivo aberto só para acréscimo dá "acesso negado".
        let mut arquivo = OpenOptions::new()
            .create(true)
            .read(true)
            .append(true)
            .open(self.arquivo(&evento.sistema))?;
        // A trava exclusiva garante uma linha inteira por vez, mesmo com vários
        // processos gravando; é liberada quando o arquivo é fechado.
        arquivo.lock()?;
        arquivo.write_all(&linha)?;
        arquivo.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn evento(n: u64) -> Evento {
        Evento {
            quando_ms: n,
            sistema: "faturamento-exemplo".into(),
            analise: "A-0001".into(),
            mudanca: Mudanca::Afirmacao,
            versao: n,
            processo: std::process::id(),
        }
    }

    #[test]
    fn acrescenta_uma_linha_por_evento() {
        let dir = tempfile::tempdir().unwrap();
        let diario = DiarioEventos::em(dir.path().join("eventos"));
        assert!(diario.ler("faturamento-exemplo").unwrap().is_empty());
        diario.notificar(&evento(1)).unwrap();
        diario.notificar(&evento(2)).unwrap();
        let lidos = diario.ler("faturamento-exemplo").unwrap();
        assert_eq!(lidos, vec![evento(1), evento(2)]);
        let texto = fs::read_to_string(diario.arquivo("faturamento-exemplo")).unwrap();
        assert!(texto.contains(r#""mudanca":"afirmacao""#), "{texto}");
    }

    #[test]
    fn varias_threads_nao_misturam_linhas() {
        let dir = tempfile::tempdir().unwrap();
        let diario = DiarioEventos::em(dir.path());
        std::thread::scope(|s| {
            for t in 0..4 {
                let diario = diario.clone();
                s.spawn(move || {
                    for i in 0..50 {
                        diario.notificar(&evento(t * 1000 + i)).unwrap();
                    }
                });
            }
        });
        let texto = fs::read_to_string(diario.arquivo("faturamento-exemplo")).unwrap();
        assert_eq!(texto.lines().count(), 200);
        assert_eq!(diario.ler("faturamento-exemplo").unwrap().len(), 200);
    }
}
