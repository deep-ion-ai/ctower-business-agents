//! Um processo num PTY: entrada, saída em texto, redimensionamento e fim.
//!
//! Origem: copiado e adaptado de `crates/ct-pty/src/lib.rs` do ctower-agents
//! (github.com/deep-ion-ai/ctower-agents, commit 23b7c74, 2026-10-30). Mudanças: a saída
//! sai por um callback, já como texto UTF-8 ([`DecodificadorUtf8`]), em vez de
//! scrollback com assinantes e mascaramento; o fim do processo fecha o PTY, o que no
//! Windows é preciso para o leitor ver o fim da saída (o ConPTY só fecha o pipe de
//! saída quando o pseudoconsole é fechado).

use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use portable_pty::{ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};
use serde::{Deserialize, Serialize};

/// Depois que o processo sai, quanto esperar o leitor entregar a saída restante. Um
/// neto que herdou o terminal e continua vivo não segura o aviso de fim.
const ESPERA_DRENAGEM: Duration = Duration::from_secs(2);

/// Tamanho do terminal, em caracteres.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tamanho {
    pub linhas: u16,
    pub colunas: u16,
}

impl Tamanho {
    /// Valores fora de 1..=1000 viram o limite mais próximo: o PTY não aceita zero, e
    /// a interface pode medir zero com a janela minimizada.
    pub fn ajustado(self) -> Self {
        Self {
            linhas: self.linhas.clamp(1, 1000),
            colunas: self.colunas.clamp(1, 1000),
        }
    }

    fn pty(self) -> PtySize {
        let t = self.ajustado();
        PtySize {
            rows: t.linhas,
            cols: t.colunas,
            pixel_width: 0,
            pixel_height: 0,
        }
    }
}

impl Default for Tamanho {
    fn default() -> Self {
        Self {
            linhas: 24,
            colunas: 80,
        }
    }
}

/// O que executar no terminal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comando {
    /// Caminho absoluto do programa.
    pub programa: PathBuf,
    pub args: Vec<OsString>,
    /// Diretório de trabalho.
    pub diretorio: PathBuf,
    /// Variáveis acrescentadas (ou trocadas) no ambiente herdado do app.
    pub env: Vec<(OsString, OsString)>,
    /// Variáveis do ambiente do app que o processo não herda.
    pub env_remover: Vec<OsString>,
}

/// Como o processo terminou.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Fim {
    /// Código de saída. Encerrado por sinal (macOS, Linux), o código é 1.
    pub codigo: u32,
    /// Nome do sinal que encerrou o processo, se foi o caso.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sinal: Option<String>,
}

/// Evento do terminal, entregue na ordem: toda a saída e, por último, o fim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "tipo",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Evento {
    /// Texto escrito pelo processo, com as sequências de controle do terminal.
    Saida { texto: String },
    /// O processo terminou. É o último evento.
    Encerrado { fim: Fim },
}

/// Erro do terminal.
#[derive(Debug, thiserror::Error)]
pub enum ErroTerminal {
    #[error("não foi possível abrir o terminal: {0}")]
    Abrir(String),
    #[error("não foi possível iniciar {programa}: {motivo}")]
    Iniciar { programa: PathBuf, motivo: String },
    #[error("o processo do terminal já terminou")]
    Encerrado,
    #[error("falha de entrada e saída no terminal: {0}")]
    Io(#[from] std::io::Error),
    #[error("falha ao redimensionar o terminal: {0}")]
    Redimensionar(String),
}

type AoEvento = Box<dyn Fn(Evento) + Send + Sync>;

struct Interno {
    /// `None` depois que o processo sai: o PTY é fechado.
    master: Mutex<Option<Box<dyn MasterPty + Send>>>,
    escritor: Mutex<Option<Box<dyn Write + Send>>>,
    matador: Mutex<Box<dyn ChildKiller + Send + Sync>>,
    fim: Mutex<Option<Fim>>,
    terminou: Condvar,
}

/// Processo rodando num PTY. Ao ser descartado, encerra o processo se ele ainda roda.
pub struct Terminal {
    interno: Arc<Interno>,
    pid: Option<u32>,
}

impl std::fmt::Debug for Terminal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Terminal")
            .field("pid", &self.pid)
            .field("fim", &self.fim())
            .finish()
    }
}

impl Terminal {
    /// Inicia `comando` num PTY de `tamanho`. `ao_evento` recebe a saída e o fim, numa
    /// thread do terminal: não deve bloquear por muito tempo.
    pub fn abrir(
        comando: Comando,
        tamanho: Tamanho,
        ao_evento: impl Fn(Evento) + Send + Sync + 'static,
    ) -> Result<Self, ErroTerminal> {
        let ao_evento: Arc<AoEvento> = Arc::new(Box::new(ao_evento));
        let par = native_pty_system()
            .openpty(tamanho.pty())
            .map_err(|e| ErroTerminal::Abrir(e.to_string()))?;
        let mut cmd = CommandBuilder::new(comando.programa.as_os_str());
        cmd.args(&comando.args);
        cmd.cwd(comando.diretorio.as_os_str());
        for k in &comando.env_remover {
            cmd.env_remove(k);
        }
        for (k, v) in &comando.env {
            cmd.env(k, v);
        }
        let mut filho = par
            .slave
            .spawn_command(cmd)
            .map_err(|e| ErroTerminal::Iniciar {
                programa: comando.programa.clone(),
                motivo: e.to_string(),
            })?;
        // Sem fechar o lado escravo no app, o leitor nunca vê o fim da saída.
        drop(par.slave);

        let leitor = par
            .master
            .try_clone_reader()
            .map_err(|e| ErroTerminal::Abrir(e.to_string()))?;
        let escritor = par
            .master
            .take_writer()
            .map_err(|e| ErroTerminal::Abrir(e.to_string()))?;
        let pid = filho.process_id();
        let interno = Arc::new(Interno {
            matador: Mutex::new(filho.clone_killer()),
            master: Mutex::new(Some(par.master)),
            escritor: Mutex::new(Some(escritor)),
            fim: Mutex::new(None),
            terminou: Condvar::new(),
        });

        let (drenado_tx, drenado_rx) = mpsc::channel::<()>();
        let saida = Arc::clone(&ao_evento);
        std::thread::Builder::new()
            .name("terminal-leitor".into())
            .spawn(move || {
                ler(leitor, &saida);
                let _ = drenado_tx.send(());
            })?;

        let esperando = Arc::clone(&interno);
        std::thread::Builder::new()
            .name("terminal-espera".into())
            .spawn(move || {
                let fim = match filho.wait() {
                    Ok(st) => Fim {
                        codigo: st.exit_code(),
                        sinal: st.signal().map(str::to_string),
                    },
                    Err(_) => Fim {
                        codigo: 1,
                        sinal: None,
                    },
                };
                // Fechar o PTY: no Windows é o que faz o ConPTY entregar o resto da
                // saída e fechar o pipe; no macOS e no Linux o leitor tem a própria
                // cópia do descritor e continua até o fim.
                esperando.escritor.lock().unwrap().take();
                esperando.master.lock().unwrap().take();
                let _ = drenado_rx.recv_timeout(ESPERA_DRENAGEM);
                *esperando.fim.lock().unwrap() = Some(fim.clone());
                esperando.terminou.notify_all();
                tracing::info!(codigo = fim.codigo, "processo do terminal terminou");
                ao_evento(Evento::Encerrado { fim });
            })?;

        tracing::info!(pid, "terminal aberto");
        Ok(Self { interno, pid })
    }

    /// Identificador do processo, se o sistema informou.
    pub fn pid(&self) -> Option<u32> {
        self.pid
    }

    /// Escreve na entrada do processo, como se fosse digitado.
    pub fn escrever(&self, dados: &[u8]) -> Result<(), ErroTerminal> {
        let mut escritor = self.interno.escritor.lock().unwrap();
        let escritor = escritor.as_mut().ok_or(ErroTerminal::Encerrado)?;
        escritor.write_all(dados)?;
        escritor.flush()?;
        Ok(())
    }

    /// Muda o tamanho do terminal; o processo recebe o aviso (SIGWINCH ou o
    /// equivalente do ConPTY).
    pub fn redimensionar(&self, tamanho: Tamanho) -> Result<(), ErroTerminal> {
        let master = self.interno.master.lock().unwrap();
        master
            .as_ref()
            .ok_or(ErroTerminal::Encerrado)?
            .resize(tamanho.pty())
            .map_err(|e| ErroTerminal::Redimensionar(e.to_string()))
    }

    /// Encerra o processo (sem efeito se já terminou). O fim chega pelo evento
    /// [`Evento::Encerrado`].
    pub fn encerrar(&self) -> Result<(), ErroTerminal> {
        if self.fim().is_some() {
            return Ok(());
        }
        match self.interno.matador.lock().unwrap().kill() {
            Ok(()) => Ok(()),
            // Terminou entre a verificação e o pedido.
            Err(_) if self.fim().is_some() => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    /// `None` enquanto o processo roda.
    pub fn fim(&self) -> Option<Fim> {
        self.interno.fim.lock().unwrap().clone()
    }

    pub fn ativo(&self) -> bool {
        self.fim().is_none()
    }

    /// Espera o processo terminar, até `limite`. `None` se ainda roda.
    pub fn esperar(&self, limite: Duration) -> Option<Fim> {
        let fim = self.interno.fim.lock().unwrap();
        let (fim, _) = self
            .interno
            .terminou
            .wait_timeout_while(fim, limite, |f| f.is_none())
            .unwrap();
        fim.clone()
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        let _ = self.encerrar();
    }
}

fn ler(mut leitor: Box<dyn Read + Send>, ao_evento: &AoEvento) {
    let mut buf = [0u8; 16 * 1024];
    let mut decodificador = DecodificadorUtf8::default();
    loop {
        match leitor.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                let texto = decodificador.decodificar(&buf[..n]);
                if !texto.is_empty() {
                    ao_evento(Evento::Saida { texto });
                }
            }
        }
    }
    let resto = decodificador.terminar();
    if !resto.is_empty() {
        ao_evento(Evento::Saida { texto: resto });
    }
}

/// Converte a saída do PTY em texto UTF-8 por partes. Um caractere de vários bytes
/// pode chegar dividido entre duas leituras: o começo fica guardado até a próxima.
/// Bytes inválidos viram U+FFFD.
#[derive(Debug, Default)]
pub struct DecodificadorUtf8 {
    pendente: Vec<u8>,
}

impl DecodificadorUtf8 {
    pub fn decodificar(&mut self, bytes: &[u8]) -> String {
        let mut dados = std::mem::take(&mut self.pendente);
        dados.extend_from_slice(bytes);
        let mut texto = String::with_capacity(dados.len());
        let mut resto = dados.as_slice();
        loop {
            match std::str::from_utf8(resto) {
                Ok(s) => {
                    texto.push_str(s);
                    break;
                }
                Err(e) => {
                    let (validos, depois) = resto.split_at(e.valid_up_to());
                    // `valid_up_to` garante que este trecho é UTF-8 válido.
                    texto.push_str(std::str::from_utf8(validos).unwrap_or_default());
                    match e.error_len() {
                        // Sequência incompleta no fim: espera a próxima leitura.
                        None => {
                            self.pendente = depois.to_vec();
                            break;
                        }
                        Some(n) => {
                            texto.push(char::REPLACEMENT_CHARACTER);
                            resto = &depois[n..];
                        }
                    }
                }
            }
        }
        texto
    }

    /// O que sobrou ao fim da saída.
    pub fn terminar(&mut self) -> String {
        String::from_utf8_lossy(&std::mem::take(&mut self.pendente)).into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caractere_dividido_entre_leituras() {
        let mut d = DecodificadorUtf8::default();
        let bytes = "ação €".as_bytes();
        let mut texto = String::new();
        for b in bytes {
            texto.push_str(&d.decodificar(std::slice::from_ref(b)));
        }
        texto.push_str(&d.terminar());
        assert_eq!(texto, "ação €");
    }

    #[test]
    fn bytes_invalidos_viram_substituto() {
        let mut d = DecodificadorUtf8::default();
        assert_eq!(d.decodificar(b"a\xffb"), "a\u{FFFD}b");
        assert_eq!(d.decodificar(b"c\xe2\x82"), "c");
        assert_eq!(d.terminar(), "\u{FFFD}");
    }

    #[test]
    fn tamanho_ajustado() {
        let t = Tamanho {
            linhas: 0,
            colunas: 5000,
        }
        .ajustado();
        assert_eq!(
            t,
            Tamanho {
                linhas: 1,
                colunas: 1000
            }
        );
    }

    #[test]
    fn evento_em_json() {
        let e = Evento::Saida { texto: "oi".into() };
        assert_eq!(
            serde_json::to_string(&e).unwrap(),
            r#"{"tipo":"saida","texto":"oi"}"#
        );
        let e = Evento::Encerrado {
            fim: Fim {
                codigo: 7,
                sinal: None,
            },
        };
        assert_eq!(
            serde_json::to_string(&e).unwrap(),
            r#"{"tipo":"encerrado","fim":{"codigo":7}}"#
        );
    }
}
