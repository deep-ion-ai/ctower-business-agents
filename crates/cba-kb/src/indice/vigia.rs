//! Vigia da base: sincroniza o índice em intervalos curtos, numa thread própria,
//! para refletir edições feitas fora do app (num editor de texto, por `git pull`).
//!
//! É por consulta, sem eventos do sistema de arquivos: cada rodada compara tamanho
//! e data de modificação dos arquivos com os guardados, o que custa pouco numa base
//! curada de Markdown, e funciona igual em Windows, macOS, Linux e pastas de rede,
//! inclusive com editores que gravam num temporário e renomeiam.

use std::path::PathBuf;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::base::Base;

use super::{Indice, ResultadoIndice, Sincronizacao};

/// Vigia em execução. Para quando é descartada (ou por [`Vigia::parar`]).
#[derive(Debug)]
pub struct Vigia {
    parar: Option<mpsc::Sender<()>>,
    thread: Option<JoinHandle<()>>,
}

impl Vigia {
    /// Intervalo padrão entre as rodadas.
    pub const INTERVALO: Duration = Duration::from_secs(2);

    /// Abre o índice (com uma conexão própria), sincroniza de imediato e depois a
    /// cada `intervalo`. `ao_mudar` é chamada, na thread da vigia, a cada
    /// sincronização que mudou o índice. Falhas de uma rodada vão para o log e a
    /// vigia continua.
    pub fn iniciar(
        arquivo: impl Into<PathBuf>,
        base: &Base,
        intervalo: Duration,
        mut ao_mudar: impl FnMut(&Sincronizacao) + Send + 'static,
    ) -> ResultadoIndice<Self> {
        let mut indice = Indice::abrir(arquivo, base)?;
        let (parar, sinal) = mpsc::channel::<()>();
        let thread = thread::Builder::new()
            .name("cba-kb-vigia".into())
            .spawn(move || {
                loop {
                    match indice.sincronizar() {
                        Ok(resumo) if resumo.mudou() => ao_mudar(&resumo),
                        Ok(_) => {}
                        Err(erro) => tracing::warn!(%erro, "vigia: falha ao sincronizar o índice"),
                    }
                    match sinal.recv_timeout(intervalo) {
                        Err(RecvTimeoutError::Timeout) => {}
                        Ok(()) | Err(RecvTimeoutError::Disconnected) => break,
                    }
                }
            })
            .map_err(|erro| super::ErroIndice::Io {
                caminho: PathBuf::from("cba-kb-vigia"),
                erro,
            })?;
        Ok(Self {
            parar: Some(parar),
            thread: Some(thread),
        })
    }

    /// Para a vigia e espera a rodada em andamento terminar.
    pub fn parar(mut self) {
        self.encerrar();
    }

    fn encerrar(&mut self) {
        drop(self.parar.take());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for Vigia {
    fn drop(&mut self) {
        self.encerrar();
    }
}
