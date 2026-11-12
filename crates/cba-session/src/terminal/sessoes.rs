//! Terminais abertos pelo app, por identificador, e a última sessão de cada projeto.

use std::collections::{BTreeMap, HashMap};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::claude::limpar_prompt;
use super::pty::{Comando, ErroTerminal, Evento, Tamanho, Terminal};

/// Pausa entre o texto de um prompt e o Enter. O Claude Code trata texto que chega de
/// uma vez como colagem; o Enter separado é que envia a mensagem.
pub const PAUSA_ENVIO: Duration = Duration::from_millis(200);

/// Terminais abertos, por identificador (começa em 1). O app guarda um só `Sessoes`.
#[derive(Debug, Default)]
pub struct Sessoes {
    proximo: AtomicU64,
    abertas: Mutex<HashMap<u64, Arc<Terminal>>>,
}

impl Sessoes {
    pub fn new() -> Self {
        Self::default()
    }

    /// Abre `comando` num terminal novo e devolve o identificador.
    pub fn abrir(
        &self,
        comando: Comando,
        tamanho: Tamanho,
        ao_evento: impl Fn(Evento) + Send + Sync + 'static,
    ) -> Result<u64, ErroTerminal> {
        let terminal = Terminal::abrir(comando, tamanho, ao_evento)?;
        let id = self.proximo.fetch_add(1, Ordering::Relaxed) + 1;
        let mut abertas = self.abertas.lock().unwrap();
        // Os que já terminaram saem aqui, sem thread de limpeza.
        abertas.retain(|_, t| t.ativo());
        abertas.insert(id, Arc::new(terminal));
        Ok(id)
    }

    fn obter(&self, id: u64) -> Result<Arc<Terminal>, ErroTerminal> {
        self.abertas
            .lock()
            .unwrap()
            .get(&id)
            .cloned()
            .ok_or(ErroTerminal::Encerrado)
    }

    /// Entrada do teclado do analista, como veio da interface.
    pub fn escrever(&self, id: u64, dados: &[u8]) -> Result<(), ErroTerminal> {
        self.obter(id)?.escrever(dados)
    }

    pub fn redimensionar(&self, id: u64, tamanho: Tamanho) -> Result<(), ErroTerminal> {
        self.obter(id)?.redimensionar(tamanho)
    }

    /// Envia um prompt pronto à sessão viva, como se o analista o tivesse digitado e
    /// apertado Enter. Bloqueia por [`PAUSA_ENVIO`].
    ///
    /// Quebras de linha vão como `Ctrl+J`, que o Claude Code documenta como nova linha
    /// na mesma mensagem (<https://code.claude.com/docs/en/interactive-mode>); o
    /// `Enter` (`\r`) no fim envia. Caracteres de controle são retirados, para o texto
    /// não virar atalho ou sequência de terminal.
    pub fn enviar_prompt(&self, id: u64, texto: &str) -> Result<(), ErroTerminal> {
        let terminal = self.obter(id)?;
        let Some(texto) = limpar_prompt(texto) else {
            return Ok(());
        };
        terminal.escrever(texto.trim_start().as_bytes())?;
        std::thread::sleep(PAUSA_ENVIO);
        terminal.escrever(b"\r")
    }

    /// Encerra o processo e esquece o terminal. Sem efeito se já não existe.
    pub fn encerrar(&self, id: u64) -> Result<(), ErroTerminal> {
        let terminal = self.abertas.lock().unwrap().remove(&id);
        match terminal {
            Some(t) => t.encerrar(),
            None => Ok(()),
        }
    }

    /// Encerra todos (ao fechar o app).
    pub fn encerrar_todas(&self) {
        let todas: Vec<_> = self.abertas.lock().unwrap().drain().collect();
        for (_, t) in todas {
            let _ = t.encerrar();
        }
    }

    /// Identificadores dos terminais com o processo rodando.
    pub fn ativas(&self) -> Vec<u64> {
        let mut ids: Vec<u64> = self
            .abertas
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, t)| t.ativo())
            .map(|(id, _)| *id)
            .collect();
        ids.sort_unstable();
        ids
    }
}

/// A última sessão do terminal embutido de um projeto: o que o app oferece para
/// retomar quando é reaberto.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UltimaSessao {
    /// Identificador da conversa no Claude Code (`--resume`); `None` quando a sessão foi
    /// aberta com `--continue` e o app não soube o identificador.
    pub id_sessao: Option<String>,
    /// Segundos desde 1970-01-01 (UTC).
    pub aberta_em: u64,
}

/// Arquivo JSON com a [`UltimaSessao`] de cada projeto
/// (`<dados>/terminal/ultimas-sessoes.json`). Guarda só identificadores e datas.
#[derive(Debug, Clone)]
pub struct UltimasSessoes {
    arquivo: PathBuf,
}

impl UltimasSessoes {
    pub fn em(arquivo: impl Into<PathBuf>) -> Self {
        Self {
            arquivo: arquivo.into(),
        }
    }

    /// No diretório de dados do app.
    pub fn no_diretorio(dados: &Path) -> Self {
        Self::em(dados.join("terminal").join("ultimas-sessoes.json"))
    }

    fn ler(&self) -> BTreeMap<String, UltimaSessao> {
        std::fs::read(&self.arquivo)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn obter(&self, projeto: &str) -> Option<UltimaSessao> {
        self.ler().remove(projeto)
    }

    /// Registra que uma sessão foi aberta agora. Sem `id_sessao` (`--continue`), mantém
    /// o identificador que já havia.
    pub fn registrar(&self, projeto: &str, id_sessao: Option<String>) -> io::Result<()> {
        let mut todas = self.ler();
        let id_sessao = id_sessao.or_else(|| todas.get(projeto).and_then(|u| u.id_sessao.clone()));
        let aberta_em = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or_default();
        todas.insert(
            projeto.to_string(),
            UltimaSessao {
                id_sessao,
                aberta_em,
            },
        );
        let pasta = self.arquivo.parent().unwrap_or(Path::new("."));
        std::fs::create_dir_all(pasta)?;
        let temporario = self.arquivo.with_extension("json.tmp");
        std::fs::write(&temporario, serde_json::to_vec_pretty(&todas)?)?;
        std::fs::rename(&temporario, &self.arquivo)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ultima_sessao_por_projeto() {
        let tmp = tempfile::tempdir().unwrap();
        let ultimas = UltimasSessoes::no_diretorio(tmp.path());
        assert_eq!(ultimas.obter("fat"), None);
        ultimas.registrar("fat", Some("abc".into())).unwrap();
        ultimas.registrar("outro", None).unwrap();
        assert_eq!(
            ultimas.obter("fat").unwrap().id_sessao.as_deref(),
            Some("abc")
        );
        // `--continue` não apaga o identificador conhecido.
        ultimas.registrar("fat", None).unwrap();
        let u = ultimas.obter("fat").unwrap();
        assert_eq!(u.id_sessao.as_deref(), Some("abc"));
        assert!(u.aberta_em > 0);
        assert_eq!(ultimas.obter("outro").unwrap().id_sessao, None);
    }

    #[test]
    fn arquivo_invalido_vale_como_vazio() {
        let tmp = tempfile::tempdir().unwrap();
        let arquivo = tmp.path().join("u.json");
        std::fs::write(&arquivo, "nao json").unwrap();
        let ultimas = UltimasSessoes::em(&arquivo);
        assert_eq!(ultimas.obter("fat"), None);
        ultimas.registrar("fat", Some("x".into())).unwrap();
        assert!(ultimas.obter("fat").is_some());
    }

    #[test]
    fn terminal_desconhecido() {
        let s = Sessoes::new();
        assert!(matches!(s.escrever(9, b"x"), Err(ErroTerminal::Encerrado)));
        assert!(s.encerrar(9).is_ok());
        assert!(s.ativas().is_empty());
    }
}
