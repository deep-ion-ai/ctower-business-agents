//! Receptor dos hooks do Claude Code (e da aba de código do Claude Desktop).
//!
//! O cliente de IA executa `cba hook <evento>` com o JSON do evento na entrada padrão
//! e lê a resposta na saída padrão (spec, seções 5 e 6.2). Por enquanto o receptor só
//! confirma o recebimento: responde `{}` com código de saída 0, que para o cliente
//! significa "seguir normalmente". O estado das sessões chega com #25 e os limites do
//! `SessionStart` com #31.
//!
//! Regras:
//! - Um hook nunca bloqueia o cliente por erro nosso: entrada inválida ou evento
//!   desconhecido também respondem `{}` com código 0, e o problema vai para o log.
//! - Rápido: o cliente espera o hook terminar (meta: menos de 200 ms).
//! - O log registra o nome do evento e a duração, nunca o JSON recebido (spec, seção 9).

use std::io::{self, Read};
use std::time::Instant;

use serde_json::Value;

/// Eventos que o app registra na configuração do cliente (spec, seção 6.2).
pub const EVENTOS: &[&str] = &[
    "SessionStart",
    "UserPromptSubmit",
    "PreToolUse",
    "Notification",
    "Stop",
    "SessionEnd",
];

/// O que o processo do hook devolve ao cliente.
#[derive(Debug, Clone, PartialEq)]
pub struct Resposta {
    /// JSON escrito na saída padrão.
    pub saida: Value,
    /// Código de saída do processo.
    pub codigo: i32,
}

impl Resposta {
    /// `{}` com código 0: o cliente segue normalmente.
    pub fn seguir() -> Self {
        Self {
            saida: Value::Object(Default::default()),
            codigo: 0,
        }
    }
}

/// Lê o JSON do evento em `entrada` e decide a resposta.
pub fn tratar(evento: &str, mut entrada: impl Read) -> Resposta {
    let inicio = Instant::now();
    let mut texto = String::new();
    let lido = entrada.read_to_string(&mut texto);
    let conhecido = EVENTOS.contains(&evento);

    let situacao = match lido {
        Err(e) => Err(format!("falha ao ler a entrada: {}", e.kind())),
        Ok(_) => match serde_json::from_str::<Value>(&texto) {
            Err(_) => Err("JSON inválido na entrada".to_string()),
            Ok(dados) => {
                // O cliente repete o nome do evento no JSON; divergência indica
                // configuração errada.
                match dados.get("hook_event_name").and_then(Value::as_str) {
                    Some(nome) if nome != evento => {
                        Err(format!("hook_event_name diferente do argumento ({nome})"))
                    }
                    _ => Ok(()),
                }
            }
        },
    };

    let duracao_us = inicio.elapsed().as_micros() as u64;
    match situacao {
        Ok(()) if conhecido => tracing::info!(evento, duracao_us, "hook"),
        Ok(()) => tracing::warn!(evento, duracao_us, "hook de evento desconhecido"),
        Err(motivo) => tracing::warn!(evento, duracao_us, motivo, "hook com entrada inesperada"),
    }
    Resposta::seguir()
}

/// Atalho para o modo `cba hook`: lê da entrada padrão.
pub fn tratar_stdin(evento: &str) -> Resposta {
    tratar(evento, io::stdin().lock())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SESSION_START: &str = r#"{
        "session_id": "abc123",
        "transcript_path": "/tmp/t.jsonl",
        "cwd": "/tmp/sistema",
        "hook_event_name": "SessionStart",
        "source": "startup"
    }"#;

    #[test]
    fn session_start_segue() {
        let r = tratar("SessionStart", SESSION_START.as_bytes());
        assert_eq!(r, Resposta::seguir());
        assert_eq!(serde_json::to_string(&r.saida).unwrap(), "{}");
    }

    #[test]
    fn todos_os_eventos_seguem() {
        for evento in EVENTOS {
            let entrada = format!(r#"{{"hook_event_name":"{evento}"}}"#);
            assert_eq!(tratar(evento, entrada.as_bytes()), Resposta::seguir());
        }
    }

    #[test]
    fn entrada_invalida_ou_evento_desconhecido_nao_bloqueiam() {
        assert_eq!(tratar("SessionStart", &b"nao json"[..]), Resposta::seguir());
        assert_eq!(tratar("SessionStart", &b""[..]), Resposta::seguir());
        assert_eq!(tratar("Inventado", &b"{}"[..]), Resposta::seguir());
        assert_eq!(tratar("Stop", SESSION_START.as_bytes()), Resposta::seguir());
    }
}
