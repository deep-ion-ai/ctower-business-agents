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
//! `initialize`, `ping`, `tools/list` e `tools/call`, aceita as notificações (como
//! `notifications/initialized`) sem responder e devolve `-32601` para métodos
//! desconhecidos.
//!
//! As ferramentas vêm de um [`Registro`] (`cba-tools`): este módulo só traduz o
//! protocolo. Em `tools/call`:
//!
//! - ferramenta inexistente ou chamada sem `name` é erro de protocolo (`-32602`), com a
//!   lista das ferramentas disponíveis;
//! - argumentos fora do schema, recusa da ferramenta (evidência que não confere,
//!   análise inexistente) e processo fora de um projeto cadastrado são **resultado de
//!   erro** (`isError: true`) com a mensagem em texto, que o agente lê e usa para
//!   corrigir a chamada.
//!
//! O projeto é identificado uma vez, no início do processo, pelo diretório de trabalho
//! ([`cba_core::ambiente::Ambiente`]). Fora de um projeto cadastrado o servidor sobe
//! do mesmo jeito (o cliente conecta e lista as ferramentas), e cada chamada responde
//! com o erro explicando o que fazer.
//!
//! Por que não o SDK oficial (`rmcp`): ele exige um runtime assíncrono (tokio) e traz
//! macros e geração de schema que não usamos. O servidor atende um cliente por processo,
//! uma requisição por vez, e as ferramentas fazem E/S de arquivo síncrona; o laço
//! síncrono basta e mantém o processo leve (o cliente inicia um por sessão). O registro
//! de ferramentas fica em `cba-tools`, independente do protocolo, para o laço próprio
//! da fase 2 (spec, seção 6.6) usar as mesmas ferramentas.
//!
//! O log registra só o método, a ferramenta, o tipo de erro e a duração, nunca os
//! argumentos nem o resultado (spec, seção 9).
//!
//! # Ferramentas que dependem da análise (#440)
//!
//! `consultar_dados` só aparece quando o banco está ligado na análise da sessão.
//! O servidor guarda a análise corrente (a do último argumento `analise`, como em
//! `iniciar_analise` e `retomar_analise`) e, antes de cada chamada, preenche
//! [`Ambiente::banco`] com o banco ligado nela, aberto pela [`FonteBancos`]
//! ([`Servidor::com_bancos`]). Quando o conjunto de ferramentas visíveis muda, o
//! servidor manda `notifications/tools/list_changed` logo depois da resposta, e
//! o cliente pede a lista de novo.
//!
//! Enquanto `consultar_dados` espera a aprovação do analista, o servidor não
//! atende outra requisição: o cliente espera a resposta da chamada.

use std::io::{self, BufRead, Write};
use std::sync::Arc;
use std::time::Instant;

use cba_core::ambiente::{Ambiente, ErroAmbiente};
use cba_core::banco::FonteBancos;
use cba_tools::{ErroChamada, Registro};
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
    pub const PARAMETROS_INVALIDOS: i64 = -32602;
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
    registro: Registro,
    ambiente: Result<Ambiente, ErroAmbiente>,
    /// Abre o banco ligado na análise (#440). Sem ela, nenhum banco.
    bancos: Option<Arc<dyn FonteBancos>>,
    /// Análise da sessão: a do último argumento `analise`.
    analise: Option<String>,
    /// Ferramentas da última lista enviada ao cliente.
    anunciadas: Option<Vec<&'static str>>,
    /// Notificações a enviar depois da resposta.
    notificacoes: Vec<Value>,
}

impl Servidor {
    /// Servidor com as ferramentas de `registro`, trabalhando no projeto de
    /// `ambiente` (ou respondendo às chamadas com o erro, se ele não pôde ser montado).
    pub fn new(
        info: InfoServidor,
        registro: Registro,
        ambiente: Result<Ambiente, ErroAmbiente>,
    ) -> Self {
        Self {
            info,
            registro,
            ambiente,
            bancos: None,
            analise: None,
            anunciadas: None,
            notificacoes: Vec::new(),
        }
    }

    /// Usa `fonte` para abrir o banco ligado na análise da sessão (#440).
    pub fn com_bancos(mut self, fonte: Arc<dyn FonteBancos>) -> Self {
        self.bancos = Some(fonte);
        self
    }

    /// Notificações pendentes (como `notifications/tools/list_changed`), para
    /// enviar depois da resposta. Esvazia a fila.
    pub fn notificacoes(&mut self) -> Vec<Value> {
        std::mem::take(&mut self.notificacoes)
    }

    /// Preenche [`Ambiente::banco`] com o banco ligado na análise da sessão, ou
    /// o esvazia.
    fn atualizar_banco(&mut self) {
        let (Some(fonte), Ok(ambiente)) = (&self.bancos, &mut self.ambiente) else {
            return;
        };
        let conexao = self
            .analise
            .as_deref()
            .and_then(|a| cba_tools::consultar_dados::conexao_da_analise(ambiente, a));
        match conexao {
            None => ambiente.banco = None,
            Some(c) => {
                if ambiente.banco.as_ref().map(|b| b.origem().conexao) != Some(c.clone()) {
                    ambiente.banco = match fonte.abrir(&c) {
                        Ok(b) => Some(b),
                        Err(e) => {
                            tracing::warn!(erro = %e, "banco ligado na análise não abriu");
                            None
                        }
                    };
                }
            }
        }
    }

    fn visiveis(&self) -> Vec<&'static str> {
        self.registro
            .visiveis(self.ambiente.as_ref())
            .iter()
            .map(|f| f.nome())
            .collect()
    }

    /// Avisa o cliente se o conjunto de ferramentas mudou desde a última lista.
    fn avisar_mudanca_da_lista(&mut self) {
        let Some(anunciadas) = &self.anunciadas else {
            return;
        };
        let agora = self.visiveis();
        if *anunciadas != agora {
            self.anunciadas = Some(agora);
            self.notificacoes.push(json!({
                "jsonrpc": "2.0",
                "method": "notifications/tools/list_changed",
            }));
        }
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
            "tools/list" => {
                self.atualizar_banco();
                Ok(self.tools_list())
            }
            "tools/call" => self.tools_call(objeto.get("params")),
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

    fn tools_list(&mut self) -> Value {
        self.anunciadas = Some(self.visiveis());
        let tools: Vec<Value> = self
            .registro
            .visiveis(self.ambiente.as_ref())
            .iter()
            .map(|f| {
                json!({
                    "name": f.nome(),
                    "title": f.titulo(),
                    "description": f.descricao(),
                    "inputSchema": f.schema(),
                    "annotations": {
                        "title": f.titulo(),
                        "readOnlyHint": f.e_somente_leitura(),
                        "destructiveHint": false,
                        "openWorldHint": false,
                    },
                })
            })
            .collect();
        json!({ "tools": tools })
    }

    fn tools_call(&mut self, params: Option<&Value>) -> Result<Value, (i64, String)> {
        if let Some(a) = params
            .and_then(|p| p.get("arguments"))
            .and_then(|a| a.get("analise"))
            .and_then(Value::as_str)
        {
            self.analise = Some(a.trim().to_owned());
        }
        self.atualizar_banco();
        let r = self.tools_call_com_banco(params);
        // A chamada pode ter ligado a sessão a outra análise (ou o analista pode
        // ter ligado ou desligado o banco nela).
        self.atualizar_banco();
        self.avisar_mudanca_da_lista();
        r
    }

    fn tools_call_com_banco(&self, params: Option<&Value>) -> Result<Value, (i64, String)> {
        let nome = params
            .and_then(|p| p.get("name"))
            .and_then(Value::as_str)
            .ok_or_else(|| {
                (
                    erro::PARAMETROS_INVALIDOS,
                    "tools/call exige o parâmetro `name`".to_owned(),
                )
            })?;
        let argumentos = params.and_then(|p| p.get("arguments")).cloned();
        let inicio = Instant::now();
        let resultado = self
            .registro
            .chamar(nome, self.ambiente.as_ref(), argumentos);
        let duracao_us = inicio.elapsed().as_micros() as u64;
        match resultado {
            Ok(saida) => {
                tracing::info!(ferramenta = nome, duracao_us, "ferramenta executada");
                let mut r = json!({
                    "content": [{ "type": "text", "text": saida.texto }],
                    "isError": false,
                });
                if let Some(dados) = saida.dados.filter(Value::is_object) {
                    r["structuredContent"] = dados;
                }
                Ok(r)
            }
            Err(ErroChamada::Desconhecida(_)) => {
                let nomes: Vec<&str> = self
                    .registro
                    .ferramentas()
                    .iter()
                    .map(|f| f.nome())
                    .collect();
                Err((
                    erro::PARAMETROS_INVALIDOS,
                    format!(
                        "ferramenta desconhecida: {nome}; disponíveis: {}",
                        nomes.join(", ")
                    ),
                ))
            }
            Err(ErroChamada::Ferramenta(e)) => {
                tracing::warn!(
                    ferramenta = nome,
                    erro = e.tipo(),
                    duracao_us,
                    "ferramenta recusou"
                );
                Ok(json!({
                    "content": [{ "type": "text", "text": e.to_string() }],
                    "isError": true,
                }))
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
            "capabilities": { "tools": { "listChanged": true } },
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
        for n in servidor.notificacoes() {
            serde_json::to_writer(&mut saida, &n)?;
            saida.write_all(b"\n")?;
            saida.flush()?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info() -> InfoServidor {
        InfoServidor {
            nome: "cba".into(),
            titulo: "Teste".into(),
            versao: "1.2.3".into(),
        }
    }

    /// Servidor sem ferramentas e fora de um projeto.
    fn servidor() -> Servidor {
        Servidor::new(
            info(),
            Registro::new(),
            Err(ErroAmbiente::BaseNaoConfigurada),
        )
    }

    fn fixture() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/faturamento-exemplo")
    }

    /// Ambiente do sistema fictício, com dados e base em `raiz`.
    fn ambiente(raiz: &std::path::Path) -> Ambiente {
        use cba_core::config::Config;
        use cba_core::dados::DiretorioDados;
        let dados = DiretorioDados::em(raiz.join("dados"));
        let projeto = cba_index::Projeto::novo("Faturamento Exemplo", [fixture()]).unwrap();
        cba_index::Cadastro::em(dados.projetos())
            .cadastrar(&projeto)
            .unwrap();
        cba_kb::Base::criar(raiz.join("base")).unwrap();
        Config {
            base: Some(raiz.join("base")),
            ..Default::default()
        }
        .gravar(&dados)
        .unwrap();
        Ambiente::do_diretorio(&fixture(), &dados).unwrap()
    }

    fn chamar(s: &mut Servidor, params: Value) -> Value {
        pedir(
            s,
            json!({ "jsonrpc": "2.0", "id": 9, "method": "tools/call", "params": params }),
        )
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
    fn tools_list_traz_nome_schema_e_anotacoes() {
        let mut s = Servidor::new(
            info(),
            cba_tools::registro_padrao(),
            Err(ErroAmbiente::BaseNaoConfigurada),
        );
        let r = s.tools_list();
        let f = &r["tools"][0];
        assert_eq!(f["name"], "listar_analises");
        assert_eq!(f["inputSchema"]["type"], "object");
        assert_eq!(f["annotations"]["readOnlyHint"], true);
        assert!(f["description"].as_str().unwrap().contains("A-NNNN"));
    }

    #[test]
    fn tools_call_com_ambiente() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = Servidor::new(
            info(),
            cba_tools::registro_padrao(),
            Ok(ambiente(dir.path())),
        );
        let r = chamar(&mut s, json!({ "name": "listar_analises" }));
        assert_eq!(r["result"]["isError"], false);
        assert_eq!(
            r["result"]["structuredContent"]["projeto"]["id"],
            "faturamento-exemplo"
        );
        let texto = r["result"]["content"][0]["text"].as_str().unwrap();
        assert!(texto.contains("faturamento-exemplo"), "{texto}");

        // Argumento fora do schema: resultado de erro, para o agente corrigir.
        let r = chamar(
            &mut s,
            json!({ "name": "listar_analises", "arguments": { "estado": "x" } }),
        );
        assert_eq!(r["result"]["isError"], true);
        let texto = r["result"]["content"][0]["text"].as_str().unwrap();
        assert!(texto.contains("`estado`"), "{texto}");
    }

    #[test]
    fn tools_call_fora_de_projeto_e_erro_claro() {
        let mut s = Servidor::new(
            info(),
            cba_tools::registro_padrao(),
            Err(ErroAmbiente::ForaDeProjeto("/tmp/qualquer".into())),
        );
        let r = chamar(&mut s, json!({ "name": "listar_analises" }));
        assert_eq!(r["result"]["isError"], true);
        let texto = r["result"]["content"][0]["text"].as_str().unwrap();
        assert!(texto.contains("projeto cadastrado"), "{texto}");
    }

    #[test]
    fn tools_call_sem_nome_ou_desconhecida_e_erro_de_protocolo() {
        let mut s = Servidor::new(
            info(),
            cba_tools::registro_padrao(),
            Err(ErroAmbiente::BaseNaoConfigurada),
        );
        let r = chamar(&mut s, json!({}));
        assert_eq!(r["error"]["code"], erro::PARAMETROS_INVALIDOS);
        let r = chamar(&mut s, json!({ "name": "nao_existe" }));
        assert_eq!(r["error"]["code"], erro::PARAMETROS_INVALIDOS);
        let msg = r["error"]["message"].as_str().unwrap();
        assert!(msg.contains("listar_analises"), "{msg}");
    }

    /// Banco e fonte de teste: só a origem (#440).
    #[derive(Debug)]
    struct BancoDeTeste;

    impl cba_core::banco::BancoConectado for BancoDeTeste {
        fn origem(&self) -> cba_core::banco::OrigemBanco {
            cba_core::banco::OrigemBanco {
                conexao: "banco-hml".into(),
                nome: "Banco de homologação".into(),
                tipo: "PostgreSQL".into(),
                ambiente: None,
            }
        }
        fn ler_schema(&self) -> Result<cba_core::banco::SchemaBanco, String> {
            Ok(Default::default())
        }
        fn consultar(&self, _sql: &str) -> Result<cba_core::banco::ResultadoDados, String> {
            Err("não usado".into())
        }
    }

    #[derive(Debug)]
    struct FonteDeTeste;

    impl FonteBancos for FonteDeTeste {
        fn abrir(&self, conexao: &str) -> Result<Arc<dyn cba_core::banco::BancoConectado>, String> {
            assert_eq!(conexao, "banco-hml");
            Ok(Arc::new(BancoDeTeste))
        }
    }

    fn nomes(r: &Value) -> Vec<String> {
        r["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap().to_owned())
            .collect()
    }

    #[test]
    fn consultar_dados_aparece_quando_o_banco_e_ligado_na_analise() {
        use cba_core::eventos::Mudanca;
        let dir = tempfile::tempdir().unwrap();
        let amb = ambiente(dir.path());
        let mut projeto = amb.projeto.clone();
        projeto.bancos = vec!["banco-hml".into()];
        cba_index::Cadastro::em(amb.dados.projetos())
            .salvar(&projeto)
            .unwrap();
        let analise = amb
            .analises
            .criar(cba_core::analises::NovaAnalise {
                fluxo: cba_core::analise::Fluxo::Impacto,
                titulo: "Multa zerada".into(),
                entrada: "Faturas sem multa".into(),
                analista: None,
                base: None,
            })
            .unwrap();
        let analises = amb.analises.clone();
        let mut s = Servidor::new(info(), cba_tools::registro_padrao(), Ok(amb))
            .com_bancos(Arc::new(FonteDeTeste));
        let r = pedir(
            &mut s,
            json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize" }),
        );
        assert_eq!(r["result"]["capabilities"]["tools"]["listChanged"], true);
        let lista = pedir(
            &mut s,
            json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }),
        );
        assert!(!nomes(&lista).contains(&"consultar_dados".to_owned()));

        // A sessão passa a trabalhar na análise, ainda sem o banco: nada muda.
        let id = analise.id.to_string();
        chamar(
            &mut s,
            json!({ "name": "iniciar_analise", "arguments": { "analise": id } }),
        );
        assert!(s.notificacoes().is_empty());

        // O analista liga o banco: na chamada seguinte, o cliente é avisado.
        analises
            .alterar(&analise.id, Mudanca::Consulta, |a, agora| {
                a.ligar_banco("banco-hml", "Banco de homologação", None, agora)
            })
            .unwrap();
        chamar(
            &mut s,
            json!({ "name": "retomar_analise", "arguments": { "analise": id } }),
        );
        let n = s.notificacoes();
        assert_eq!(n.len(), 1);
        assert_eq!(n[0]["method"], "notifications/tools/list_changed");
        let lista = pedir(
            &mut s,
            json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/list" }),
        );
        assert!(nomes(&lista).contains(&"consultar_dados".to_owned()));

        // Desligado, some de novo.
        analises
            .alterar(&analise.id, Mudanca::Consulta, |a, agora| {
                a.desligar_banco(agora);
                Ok(())
            })
            .unwrap();
        chamar(
            &mut s,
            json!({ "name": "retomar_analise", "arguments": { "analise": id } }),
        );
        assert_eq!(s.notificacoes().len(), 1);
        let lista = pedir(
            &mut s,
            json!({ "jsonrpc": "2.0", "id": 4, "method": "tools/list" }),
        );
        assert!(!nomes(&lista).contains(&"consultar_dados".to_owned()));
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
