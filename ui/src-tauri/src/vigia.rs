//! Acompanhamento do diário de eventos (`<dados>/eventos/<sistema>.jsonl`) para a
//! tela da análise se atualizar ao vivo (spec, seção 4.2, passo 6).
//!
//! # Por que consulta periódica, e não notificação do sistema de arquivos
//!
//! O diário é um arquivo só por sistema, acrescentado por outros processos (`cba mcp`
//! e `cba hook`). Consultar o tamanho dele a cada [`INTERVALO`] custa uma chamada
//! `stat` e só abre o arquivo quando ele mudou, e dá a mesma latência (no máximo o
//! intervalo, bem abaixo do 1 s da issue) no Windows, no macOS e no Linux. A
//! notificação do sistema (crate `notify`) traria uma dependência e comportamentos
//! diferentes por plataforma: no macOS, o FSEvents agrupa os avisos com atraso
//! configurável e não avisa por arquivo de forma confiável; no Windows, o
//! `ReadDirectoryChangesW` vigia a pasta inteira e perde avisos quando o buffer
//! enche. Cada aviso ainda exigiria ler o arquivo do mesmo jeito.
//!
//! Há uma vigia por sistema, compartilhada pelos assinantes (as telas abertas): só
//! ela lê e esvazia o diário ([`cba_core::eventos::LeitorDiario`]), então nenhuma
//! tela do app perde evento por causa de outra.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use cba_core::dados::DiretorioDados;
use cba_core::eventos::{DiarioEventos, Mudanca};
use serde::Serialize;
use tauri::State;
use tauri::ipc::Channel;

/// Intervalo entre duas consultas ao diário.
pub const INTERVALO: Duration = Duration::from_millis(100);

/// O que a tela recebe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "tipo",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Aviso {
    /// Uma análise do sistema mudou: a tela relê se for a que está aberta.
    Mudanca {
        analise: String,
        mudanca: Mudanca,
        versao: u64,
    },
    /// Eventos podem ter se perdido (diário esvaziado por outro processo ou ilegível
    /// por um tempo): a tela relê o que mostra.
    Reinicio,
}

type Assinante = Box<dyn Fn(&Aviso) + Send + Sync>;

#[derive(Default)]
struct Vigia {
    assinantes: Mutex<HashMap<u64, Assinante>>,
    parar: AtomicBool,
}

impl Vigia {
    fn avisar(&self, aviso: &Aviso) {
        let assinantes = self.assinantes.lock().unwrap_or_else(|e| e.into_inner());
        for f in assinantes.values() {
            f(aviso);
        }
    }
}

/// Vigias abertas pelo app, uma por sistema.
pub struct Vigias {
    eventos: PathBuf,
    intervalo: Duration,
    proximo: AtomicU64,
    por_sistema: Mutex<HashMap<String, Arc<Vigia>>>,
    /// Sistema de cada assinatura.
    assinaturas: Mutex<HashMap<u64, String>>,
}

impl Vigias {
    /// Vigias do diário na pasta `eventos`.
    pub fn em(eventos: impl Into<PathBuf>, intervalo: Duration) -> Self {
        Self {
            eventos: eventos.into(),
            intervalo,
            proximo: AtomicU64::new(1),
            por_sistema: Mutex::new(HashMap::new()),
            assinaturas: Mutex::new(HashMap::new()),
        }
    }

    /// Vigias do diretório de dados padrão.
    pub fn padrao() -> Self {
        let pasta = DiretorioDados::padrao()
            .map(|d| d.eventos())
            .unwrap_or_else(|_| PathBuf::from("eventos"));
        Self::em(pasta, INTERVALO)
    }

    /// Passa a receber os avisos do sistema. Devolve o identificador da assinatura.
    pub fn assinar(&self, sistema: &str, f: impl Fn(&Aviso) + Send + Sync + 'static) -> u64 {
        let id = self.proximo.fetch_add(1, Ordering::Relaxed);
        let mut por_sistema = self.por_sistema.lock().unwrap_or_else(|e| e.into_inner());
        let vigia = por_sistema
            .entry(sistema.to_owned())
            .or_insert_with(|| self.iniciar(sistema));
        vigia
            .assinantes
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(id, Box::new(f));
        self.assinaturas
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(id, sistema.to_owned());
        id
    }

    /// Cancela uma assinatura. A vigia do sistema para com o último assinante.
    pub fn cancelar(&self, id: u64) {
        let Some(sistema) = self
            .assinaturas
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&id)
        else {
            return;
        };
        let mut por_sistema = self.por_sistema.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(vigia) = por_sistema.get(&sistema) {
            let mut assinantes = vigia.assinantes.lock().unwrap_or_else(|e| e.into_inner());
            assinantes.remove(&id);
            if assinantes.is_empty() {
                vigia.parar.store(true, Ordering::Relaxed);
                drop(assinantes);
                por_sistema.remove(&sistema);
            }
        }
    }

    /// Sistemas com vigia ativa (testes).
    #[cfg(test)]
    pub fn ativas(&self) -> usize {
        self.por_sistema
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .len()
    }

    fn iniciar(&self, sistema: &str) -> Arc<Vigia> {
        let vigia = Arc::new(Vigia::default());
        let mut leitor = DiarioEventos::em(&self.eventos).leitor(sistema);
        let intervalo = self.intervalo;
        let v = Arc::clone(&vigia);
        std::thread::Builder::new()
            .name("cba-vigia-eventos".into())
            .spawn(move || {
                let mut falhou = false;
                while !v.parar.load(Ordering::Relaxed) {
                    match leitor.ler_novos() {
                        Ok(leitura) => {
                            if leitura.reiniciado || falhou {
                                v.avisar(&Aviso::Reinicio);
                            }
                            falhou = false;
                            for e in leitura.eventos {
                                v.avisar(&Aviso::Mudanca {
                                    analise: e.analise,
                                    mudanca: e.mudanca,
                                    versao: e.versao,
                                });
                            }
                        }
                        Err(e) => {
                            if !falhou {
                                tracing::warn!(erro = %e.kind(), "diário de eventos ilegível");
                            }
                            falhou = true;
                        }
                    }
                    std::thread::sleep(intervalo);
                }
            })
            .expect("thread da vigia do diário");
        vigia
    }
}

/// Passa a mandar à tela, por `canal`, os avisos de mudança nas análises do sistema.
#[tauri::command]
pub fn vigiar_analises(
    vigias: State<'_, Arc<Vigias>>,
    sistema: String,
    canal: Channel<Aviso>,
) -> u64 {
    vigias.assinar(&sistema, move |aviso| {
        // A janela pode ter sido fechada; não há o que fazer com o erro.
        let _ = canal.send(aviso.clone());
    })
}

#[tauri::command]
pub fn parar_vigia(vigias: State<'_, Arc<Vigias>>, id: u64) {
    vigias.cancelar(id);
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::time::Instant;

    use cba_core::eventos::{Evento, Notificador};

    use super::*;

    fn evento(analise: &str, versao: u64) -> Evento {
        Evento {
            quando_ms: 0,
            sistema: "faturamento-exemplo".into(),
            analise: analise.into(),
            mudanca: Mudanca::Afirmacao,
            versao,
            processo: std::process::id(),
        }
    }

    #[test]
    fn aviso_chega_em_menos_de_um_segundo_e_para_com_o_ultimo_assinante() {
        let dir = tempfile::tempdir().unwrap();
        let diario = DiarioEventos::em(dir.path());
        let vigias = Vigias::em(dir.path(), INTERVALO);
        let (tx, rx) = mpsc::channel();
        let tx = Mutex::new(tx);
        let id = vigias.assinar("faturamento-exemplo", move |a| {
            let _ = tx.lock().unwrap().send((Instant::now(), a.clone()));
        });
        let mut pior = Duration::ZERO;
        for versao in 1..=5 {
            std::thread::sleep(Duration::from_millis(37));
            let antes = Instant::now();
            diario.notificar(&evento("A-0001", versao)).unwrap();
            let (quando, aviso) = rx.recv_timeout(Duration::from_secs(1)).expect("aviso");
            pior = pior.max(quando - antes);
            assert_eq!(
                aviso,
                Aviso::Mudanca {
                    analise: "A-0001".into(),
                    mudanca: Mudanca::Afirmacao,
                    versao
                }
            );
        }
        eprintln!("latência máxima do aviso: {pior:?}");
        assert!(pior < Duration::from_secs(1), "{pior:?}");
        assert_eq!(vigias.ativas(), 1);
        vigias.cancelar(id);
        assert_eq!(vigias.ativas(), 0);
        diario.notificar(&evento("A-0001", 6)).unwrap();
        assert!(rx.recv_timeout(Duration::from_millis(400)).is_err());
    }

    #[test]
    fn varias_telas_do_mesmo_sistema_compartilham_a_vigia() {
        let dir = tempfile::tempdir().unwrap();
        let diario = DiarioEventos::em(dir.path());
        let vigias = Vigias::em(dir.path(), Duration::from_millis(20));
        let (tx, rx) = mpsc::channel();
        let (tx1, tx2) = (Mutex::new(tx.clone()), Mutex::new(tx));
        let a = vigias.assinar("faturamento-exemplo", move |_| {
            let _ = tx1.lock().unwrap().send(1);
        });
        let _b = vigias.assinar("faturamento-exemplo", move |_| {
            let _ = tx2.lock().unwrap().send(2);
        });
        assert_eq!(vigias.ativas(), 1);
        diario.notificar(&evento("A-0001", 1)).unwrap();
        let mut quem = [
            rx.recv_timeout(Duration::from_secs(1)).unwrap(),
            rx.recv_timeout(Duration::from_secs(1)).unwrap(),
        ];
        quem.sort();
        assert_eq!(quem, [1, 2]);
        vigias.cancelar(a);
        assert_eq!(vigias.ativas(), 1);
        diario.notificar(&evento("A-0001", 2)).unwrap();
        assert_eq!(rx.recv_timeout(Duration::from_secs(1)).unwrap(), 2);
    }

    #[test]
    fn aviso_serializa_para_a_tela() {
        let aviso = Aviso::Mudanca {
            analise: "A-0001".into(),
            mudanca: Mudanca::Sessao,
            versao: 3,
        };
        assert_eq!(
            serde_json::to_value(&aviso).unwrap(),
            serde_json::json!({"tipo": "mudanca", "analise": "A-0001", "mudanca": "sessao", "versao": 3})
        );
        assert_eq!(
            serde_json::to_value(Aviso::Reinicio).unwrap(),
            serde_json::json!({"tipo": "reinicio"})
        );
    }
}
