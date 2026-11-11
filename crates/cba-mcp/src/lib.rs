//! Servidor MCP local.
//!
//! Expõe as ferramentas de `cba-tools` aos clientes de IA (Claude Code e Claude Desktop) por
//! MCP sobre stdio. É o centro do produto: todos os clientes escrevem na mesma análise.
//!
//! Papel na arquitetura: seção 5 de `docs/specs/2026-11-11-mvp-standalone.md`.
//!
//! # Protocolo
//!
//! Implementação própria e mínima do transporte stdio do MCP: uma mensagem JSON-RPC 2.0
//! por linha na entrada padrão, uma resposta por linha na saída padrão. Responde a
//! `initialize`, `ping` e `tools/list` (ainda sem ferramentas, que chegam com #19),
//! aceita as notificações (como `notifications/initialized`) sem responder e devolve
//! `-32601` para métodos desconhecidos.
//!
//! Por que não o SDK oficial (`rmcp`): ele exige um runtime assíncrono (tokio) e traz
//! macros e geração de schema que ainda não usamos; para o ciclo de vida e a listagem,
//! um laço síncrono de poucas linhas basta e mantém o processo leve. A troca continua
//! aberta quando as ferramentas chegarem (#19): a interface pública aqui é pequena.
//!
//! O log registra só o método e a duração, nunca os parâmetros (spec, seção 9).

use std::io::{self, BufRead, Write};
use std::time::Instant;

use serde_json::{Value, json};

/// Nome da crate, para diagnóstico.
pub const NOME: &str = env!("CARGO_PKG_NAME");

/// Versões do protocolo MCP que o servidor aceita, da mais nova para a mais antiga.
/// Se o cliente pedir uma delas, o servidor responde com a mesma; senão, com a primeira.
pub const VERSOES_DO_PROTOCOLO: &[&str] = &["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

/// Códigos de erro do JSON-RPC 2.0.
pub mod erro {
    pub const JSON_INVALIDO: i64 = -32700;
    pub const REQUISICAO_INVALIDA: i64 = -32600;
    pub const METODO_NAO_ENCONTRADO: i64 = -32601;
}

/// Identificação do servidor na resposta do `initialize` (`serverInfo`).
#[derive(Debug, Clone)]
pub struct InfoServidor {
    pub nome: String,
    pub titulo: String,
    pub versao: String,
}

/// Estado de uma conexão MCP.
#[derive(Debug)]
pub struct Servidor {
    info: InfoServidor,
}

impl Servidor {
    pub fn new(info: InfoServidor) -> Self {
        Self { info }
    }

    /// Trata uma linha recebida. Devolve a resposta, ou `None` para notificações e
    /// linhas em branco.
    pub fn tratar_linha(&mut self, linha: &str) -> Option<Value> {
        if linha.trim().is_empty() {
            return None;
        }
        match serde_json::from_str::<Value>(linha) {
            Ok(mensagem) => self.tratar(mensagem),
            Err(_) => Some(resposta_erro(
                Value::Null,
                erro::JSON_INVALIDO,
                "JSON inválido",
            )),
        }
    }

    /// Trata uma mensagem JSON-RPC já decodificada.
    pub fn tratar(&mut self, mensagem: Value) -> Option<Value> {
        let Some(objeto) = mensagem.as_object() else {
            return Some(resposta_erro(
                Value::Null,
                erro::REQUISICAO_INVALIDA,
                "a mensagem deve ser um objeto JSON-RPC",
            ));
        };
        let id = objeto.get("id").cloned();
        let metodo = objeto.get("method").and_then(Value::as_str);

        let (Some(metodo), true) = (metodo, objeto.get("jsonrpc") == Some(&json!("2.0"))) else {
            // Resposta a uma requisição nossa (ainda não fazemos nenhuma) ou mensagem malformada.
            if objeto.contains_key("result") || objeto.contains_key("error") {
                return None;
            }
            return Some(resposta_erro(
                id.unwrap_or(Value::Null),
                erro::REQUISICAO_INVALIDA,
                "requisição JSON-RPC 2.0 inválida",
            ));
        };

        let Some(id) = id else {
            // Notificação: não tem resposta.
            tracing::debug!(metodo, "notificação MCP");
            return None;
        };

        let inicio = Instant::now();
        let resultado = match metodo {
            "initialize" => Ok(self.initialize(objeto.get("params"))),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": [] })),
            _ => Err((
                erro::METODO_NAO_ENCONTRADO,
                format!("método desconhecido: {metodo}"),
            )),
        };
        let duracao_us = inicio.elapsed().as_micros() as u64;
        match resultado {
            Ok(resultado) => {
                tracing::info!(metodo, duracao_us, "requisição MCP");
                Some(json!({ "jsonrpc": "2.0", "id": id, "result": resultado }))
            }
            Err((codigo, texto)) => {
                tracing::warn!(metodo, codigo, "requisição MCP recusada");
                Some(resposta_erro(id, codigo, &texto))
            }
        }
    }

    fn initialize(&self, params: Option<&Value>) -> Value {
        let pedida = params
            .and_then(|p| p.get("protocolVersion"))
            .and_then(Value::as_str);
        let versao = pedida
            .filter(|v| VERSOES_DO_PROTOCOLO.contains(v))
            .unwrap_or(VERSOES_DO_PROTOCOLO[0]);
        json!({
            "protocolVersion": versao,
            "capabilities": { "tools": { "listChanged": false } },
            "serverInfo": {
                "name": self.info.nome,
                "title": self.info.titulo,
                "version": self.info.versao,
            },
        })
    }
}

fn resposta_erro(id: Value, codigo: i64, texto: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": codigo, "message": texto },
    })
}

/// Atende uma conexão: lê mensagens de `entrada` até o fim e escreve as respostas em
/// `saida`, uma por linha.
pub fn servir(
    entrada: impl BufRead,
    mut saida: impl Write,
    servidor: &mut Servidor,
) -> io::Result<()> {
    for linha in entrada.lines() {
        if let Some(resposta) = servidor.tratar_linha(&linha?) {
            serde_json::to_writer(&mut saida, &resposta)?;
            saida.write_all(b"\n")?;
            saida.flush()?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn servidor() -> Servidor {
        Servidor::new(InfoServidor {
            nome: "cba".into(),
            titulo: "Teste".into(),
            versao: "1.2.3".into(),
        })
    }

    fn pedir(s: &mut Servidor, mensagem: Value) -> Value {
        s.tratar(mensagem).expect("resposta")
    }

    #[test]
    fn nome_da_crate() {
        assert_eq!(NOME, "cba-mcp");
    }

    #[test]
    fn initialize_devolve_versao_e_servidor() {
        let r = pedir(
            &mut servidor(),
            json!({
                "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": {
                    "protocolVersion": "2025-06-18",
                    "capabilities": {},
                    "clientInfo": { "name": "teste", "version": "0" }
                }
            }),
        );
        assert_eq!(r["id"], 1);
        assert_eq!(r["result"]["protocolVersion"], "2025-06-18");
        assert_eq!(r["result"]["serverInfo"]["name"], "cba");
        assert_eq!(r["result"]["serverInfo"]["version"], "1.2.3");
        assert!(r["result"]["capabilities"]["tools"].is_object());
    }

    #[test]
    fn initialize_com_versao_desconhecida_propoe_a_mais_nova() {
        let r = pedir(
            &mut servidor(),
            json!({ "jsonrpc": "2.0", "id": "a", "method": "initialize",
                    "params": { "protocolVersion": "1999-01-01" } }),
        );
        assert_eq!(r["id"], "a");
        assert_eq!(r["result"]["protocolVersion"], VERSOES_DO_PROTOCOLO[0]);
    }

    #[test]
    fn notificacao_nao_tem_resposta() {
        let mut s = servidor();
        assert!(
            s.tratar(json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }))
                .is_none()
        );
    }

    #[test]
    fn tools_list_vazia() {
        let r = pedir(
            &mut servidor(),
            json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }),
        );
        assert_eq!(r["result"]["tools"], json!([]));
    }

    #[test]
    fn ping() {
        let r = pedir(
            &mut servidor(),
            json!({ "jsonrpc": "2.0", "id": 3, "method": "ping" }),
        );
        assert_eq!(r["result"], json!({}));
    }

    #[test]
    fn metodo_desconhecido() {
        let r = pedir(
            &mut servidor(),
            json!({ "jsonrpc": "2.0", "id": 4, "method": "nao/existe" }),
        );
        assert_eq!(r["id"], 4);
        assert_eq!(r["error"]["code"], erro::METODO_NAO_ENCONTRADO);
    }

    #[test]
    fn json_invalido_e_requisicao_invalida() {
        let mut s = servidor();
        let r = s.tratar_linha("{nao é json").unwrap();
        assert_eq!(r["error"]["code"], erro::JSON_INVALIDO);
        assert_eq!(r["id"], Value::Null);
        let r = pedir(&mut s, json!({ "id": 5, "method": "ping" }));
        assert_eq!(r["error"]["code"], erro::REQUISICAO_INVALIDA);
        assert_eq!(r["id"], 5);
        let r = pedir(&mut s, json!([1, 2]));
        assert_eq!(r["error"]["code"], erro::REQUISICAO_INVALIDA);
    }

    #[test]
    fn servir_responde_uma_linha_por_requisicao() {
        let entrada = concat!(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}"#,
            "\n",
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            "\n\n",
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#,
            "\n",
        );
        let mut saida = Vec::new();
        servir(entrada.as_bytes(), &mut saida, &mut servidor()).unwrap();
        let linhas: Vec<Value> = String::from_utf8(saida)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(linhas.len(), 2);
        assert_eq!(linhas[0]["id"], 1);
        assert_eq!(linhas[1]["result"]["tools"], json!([]));
    }
}
