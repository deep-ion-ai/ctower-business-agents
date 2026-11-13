//! Comandos Tauri da tela da análise (spec, seções 4.2, 6.4 e 10).
//!
//! A tela escolhe o projeto, cria a análise de uma pergunta e abre o terminal com o
//! pedido que faz o agente chamar `iniciar_analise` com o identificador criado aqui
//! (a sessão fica ligada à análise pelos hooks desde a primeira chamada). Enquanto o
//! agente trabalha, a tela relê o estado a cada aviso do diário de eventos
//! ([`crate::vigia`]) e abre o trecho das evidências.
//!
//! A lógica fica em funções comuns que recebem o [`DiretorioDados`], testadas sem a
//! janela; os comandos só as levam para fora da thread da interface. A verificação e o
//! preparo do ambiente com consentimento ficam aqui e servem também ao assistente do
//! primeiro uso ([`crate::primeiro_uso`]), que cuida da base e do cadastro.

use std::path::PathBuf;
use std::sync::Arc;

use cba_core::ambiente::Ambiente;
use cba_core::analise::{Analise, Cliente, EstadoAnalise, EstadoSessao, Fluxo};
use cba_core::analises::NovaAnalise;
use cba_core::dados::DiretorioDados;
use cba_core::eventos::DiarioEventos;
use cba_index::{Cadastro, detectar_git};
use cba_kb::Id;
use cba_kb::frontmatter::VersaoCodigo;
use cba_session::passagem::ClientesDasSessoes;
use cba_session::preparo::{Alvo, Preparador, Resultado, Verificacao};
use cba_session::sessoes::Sessoes;
use cba_session::terminal::pendencias;
use cba_tools::evidencias::{ArquivoDoProjeto, intervalo};
use serde::Serialize;

/// Linhas mostradas antes e depois do intervalo de uma evidência.
pub const CONTEXTO_DO_TRECHO: u32 = 8;

fn dados() -> Result<DiretorioDados, String> {
    DiretorioDados::padrao().map_err(|e| format!("diretório de dados do app inacessível: {e}"))
}

async fn bloqueante<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| format!("falha interna: {e}"))?
}

fn preparador() -> Result<Preparador, String> {
    let exe = std::env::current_exe().map_err(|e| format!("caminho do executável do app: {e}"))?;
    Preparador::padrao(exe).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Preparo do ambiente

/// Situação do ambiente do Claude para o projeto.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AmbienteClaude {
    pub pronto: bool,
    /// O que falta, em frases curtas.
    pub pendencias: Vec<String>,
    pub verificacao: Verificacao,
}

pub(crate) fn alvo(dados: &DiretorioDados, projeto: &str) -> Result<Alvo, String> {
    let cadastrado = Cadastro::em(dados.projetos())
        .obter(projeto)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("o projeto \"{projeto}\" não está cadastrado"))?;
    Alvo::do_projeto(&cadastrado).map_err(|e| e.to_string())
}

pub fn verificar_ambiente(
    dados: &DiretorioDados,
    preparador: &Preparador,
    projeto: &str,
) -> Result<AmbienteClaude, String> {
    let verificacao = preparador
        .verificar(&alvo(dados, projeto)?)
        .map_err(|e| e.to_string())?;
    Ok(AmbienteClaude {
        pronto: verificacao.pronto,
        pendencias: pendencias(&verificacao),
        verificacao,
    })
}

/// O que o preparo mudaria, sem gravar (tela de consentimento).
pub fn planejar_preparo(
    dados: &DiretorioDados,
    preparador: &Preparador,
    projeto: &str,
) -> Result<Resultado, String> {
    preparador
        .planejar(&alvo(dados, projeto)?)
        .map_err(|e| e.to_string())
}

/// Prepara o ambiente. Só chamado depois do consentimento do analista na tela.
pub fn preparar_ambiente(
    dados: &DiretorioDados,
    preparador: &Preparador,
    projeto: &str,
) -> Result<Resultado, String> {
    preparador
        .preparar(&alvo(dados, projeto)?)
        .map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Análises

pub(crate) fn ambiente(dados: &DiretorioDados, projeto: &str) -> Result<Ambiente, String> {
    let cadastrado = Cadastro::em(dados.projetos())
        .obter(projeto)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("o projeto \"{projeto}\" não está cadastrado"))?;
    let pasta = cadastrado
        .pastas
        .first()
        .map(|p| p.caminho.clone())
        .ok_or("o projeto não tem pastas cadastradas")?;
    let notificador = Arc::new(DiarioEventos::em(dados.eventos()));
    Ambiente::do_processo_com(&pasta, Some(projeto), dados, notificador).map_err(|e| e.to_string())
}

fn id_da_analise(analise: &str) -> Result<Id, String> {
    Id::analisar(analise.trim()).map_err(|e| e.to_string())
}

/// Análise na lista do projeto.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnaliseResumo {
    pub id: String,
    pub titulo: String,
    pub fluxo: Fluxo,
    pub estado: EstadoAnalise,
    pub atualizada_ms: u64,
}

impl From<&Analise> for AnaliseResumo {
    fn from(a: &Analise) -> Self {
        Self {
            id: a.id.to_string(),
            titulo: a.titulo.clone(),
            fluxo: a.fluxo,
            estado: a.estado,
            atualizada_ms: a.atualizada_ms,
        }
    }
}

/// Análises do projeto, da mais recente para a mais antiga.
pub fn listar_analises(
    dados: &DiretorioDados,
    projeto: &str,
) -> Result<Vec<AnaliseResumo>, String> {
    let mut lista: Vec<AnaliseResumo> = ambiente(dados, projeto)?
        .analises
        .listar()
        .map_err(|e| e.to_string())?
        .iter()
        .map(AnaliseResumo::from)
        .collect();
    lista.reverse();
    Ok(lista)
}

/// Pedido inicial da conversa nova de uma análise criada pelo app. Sem aspas, `%`,
/// `!`, parênteses nem quebras de linha, que o `claude.cmd` do Windows recusa
/// (spec, seção 6.4). Passar `analise` liga a sessão à análise pelo hook
/// `PreToolUse` (#25).
pub fn prompt_de_inicio(id: &Id) -> String {
    format!(
        "Inicie a análise {id} pelo ctower: chame a ferramenta iniciar_analise do cba \
         com analise {id} e siga o roteiro devolvido."
    )
}

/// Pedido de uma conversa nova sobre uma análise que já existe (spec, seção 4.7).
pub fn prompt_de_retomada(id: &Id) -> String {
    format!(
        "Retome a análise {id} pelo ctower: chame a ferramenta retomar_analise do cba \
         com analise {id} e continue de onde parou."
    )
}

/// Análise criada pela tela, com o pedido que abre a conversa.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnaliseCriada {
    pub analise: AnaliseResumo,
    pub prompt_inicial: String,
}

/// Título a partir da pergunta: a primeira linha, até 80 caracteres.
fn titulo_da_entrada(entrada: &str) -> String {
    let linha = entrada.trim().lines().next().unwrap_or_default().trim();
    if linha.chars().count() <= 80 {
        return linha.to_owned();
    }
    let corte: String = linha.chars().take(79).collect();
    format!("{}…", corte.trim_end())
}

/// Cria a análise de uma pergunta ao sistema (fluxo "perguntar").
pub fn criar_analise(
    dados: &DiretorioDados,
    projeto: &str,
    entrada: &str,
    titulo: Option<&str>,
) -> Result<AnaliseCriada, String> {
    criar_analise_do_fluxo(dados, projeto, Fluxo::Perguntar, entrada, titulo)
}

/// Cria a análise de qualquer fluxo (perguntar, explicar ou impacto), com o pedido
/// que abre a conversa. Usada pelos botões dos fluxos do início ([`crate::inicio`]).
pub fn criar_analise_do_fluxo(
    dados: &DiretorioDados,
    projeto: &str,
    fluxo: Fluxo,
    entrada: &str,
    titulo: Option<&str>,
) -> Result<AnaliseCriada, String> {
    criar_analise_com_demanda(dados, projeto, fluxo, entrada, titulo, None)
}

/// Como [`criar_analise_do_fluxo`], com a origem da demanda (a issue do rastreador).
pub fn criar_analise_com_demanda(
    dados: &DiretorioDados,
    projeto: &str,
    fluxo: Fluxo,
    entrada: &str,
    titulo: Option<&str>,
    demanda: Option<cba_kb::frontmatter::Demanda>,
) -> Result<AnaliseCriada, String> {
    let ambiente = ambiente(dados, projeto)?;
    let titulo = titulo
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| titulo_da_entrada(entrada));
    let base = ambiente
        .escopo
        .pastas()
        .first()
        .and_then(|p| detectar_git(&p.caminho))
        .map(|g| VersaoCodigo {
            branch: g.branch,
            commit: g.commit,
        });
    let analise = ambiente
        .analises
        .criar_com_demanda(
            NovaAnalise {
                fluxo,
                titulo,
                entrada: entrada.to_owned(),
                analista: None,
                base,
            },
            demanda,
        )
        .map_err(|e| e.to_string())?;
    Ok(AnaliseCriada {
        prompt_inicial: prompt_de_inicio(&analise.id),
        analise: AnaliseResumo::from(&analise),
    })
}

/// Sessão do cliente de IA mostrada na tela.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessaoVista {
    pub id: String,
    pub estado: EstadoSessao,
    pub cliente: Cliente,
}

/// Estado completo da análise, como a tela mostra.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VistaAnalise {
    /// O estado gravado (`A-NNNN.estado.json`), no formato do arquivo.
    pub analise: Analise,
    /// Caminho absoluto do documento `A-NNNN.md`.
    pub documento: PathBuf,
    /// Sessão mais recente ligada à análise.
    pub sessao: Option<SessaoVista>,
    /// Pedido para uma conversa nova continuar a análise.
    pub prompt_retomada: String,
}

/// Sessão mais recente: o registro dos hooks (#25), que guarda o instante da última
/// mudança, com o cliente informado pelo app (#28), ou, sem ele, a última sessão
/// gravada na análise.
pub(crate) fn sessao_recente(dados: &DiretorioDados, analise: &Analise) -> Option<SessaoVista> {
    let registros = Sessoes::do_diretorio(dados)
        .da_analise(&analise.projeto, &analise.id)
        .unwrap_or_default();
    if let Some(r) = registros.into_iter().next() {
        // Os hooks não sabem o cliente; o app informa pelo que abriu (#28).
        let cliente = ClientesDasSessoes::no_diretorio(dados.raiz()).cliente_de(&r);
        return Some(SessaoVista {
            id: r.id,
            estado: r.estado,
            cliente,
        });
    }
    analise
        .sessoes
        .iter()
        .max_by_key(|s| s.inicio_ms)
        .map(|s| SessaoVista {
            id: s.id.clone(),
            estado: s.estado,
            cliente: s.cliente,
        })
}

pub fn carregar_analise(
    dados: &DiretorioDados,
    projeto: &str,
    analise: &str,
) -> Result<VistaAnalise, String> {
    let ambiente = ambiente(dados, projeto)?;
    let id = id_da_analise(analise)?;
    let analise = ambiente.analises.carregar(&id).map_err(|e| e.to_string())?;
    Ok(VistaAnalise {
        documento: ambiente.kb.caminho(&analise.documento),
        sessao: sessao_recente(dados, &analise),
        prompt_retomada: prompt_de_retomada(&id),
        analise,
    })
}

// ---------------------------------------------------------------------------
// Trecho de uma evidência

/// Trecho do código de uma evidência, com linhas de contexto.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Trecho {
    /// Caminho relativo à raiz do projeto.
    pub arquivo: String,
    /// Intervalo da evidência (destacado).
    pub inicio: u32,
    pub fim: u32,
    /// Número da primeira linha de `linhas`.
    pub primeira: u32,
    pub linhas: Vec<String>,
    /// Linhas do arquivo.
    pub total: u32,
}

/// Lê o intervalo de uma evidência pelo escopo do projeto: recusa o que estiver
/// fora das pastas cadastradas, excluído (segredos, binários) ou fora do arquivo,
/// com as mesmas regras da validação das evidências (#14, #20).
pub fn trecho_evidencia(
    dados: &DiretorioDados,
    projeto: &str,
    arquivo: &str,
    inicio: u32,
    fim: u32,
) -> Result<Trecho, String> {
    let cadastrado = Cadastro::em(dados.projetos())
        .obter(projeto)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("o projeto \"{projeto}\" não está cadastrado"))?;
    let escopo = cadastrado.escopo().map_err(|e| e.to_string())?;
    let recusa = |m: cba_tools::evidencias::Motivo| format!("`{arquivo}`: {m}");
    let linhas = intervalo(inicio, fim).map_err(recusa)?;
    let aberto = ArquivoDoProjeto::abrir(&escopo, arquivo).map_err(recusa)?;
    let todas: Vec<&str> = aberto.texto.lines().collect();
    let total = u32::try_from(todas.len()).unwrap_or(u32::MAX);
    if linhas.fim > total {
        return Err(format!(
            "`{arquivo}`: as linhas {linhas} passam do fim do arquivo, que tem {total} \
             linhas; o código pode ter mudado desde o registro"
        ));
    }
    let primeira = linhas.inicio.saturating_sub(CONTEXTO_DO_TRECHO).max(1);
    let ultima = linhas.fim.saturating_add(CONTEXTO_DO_TRECHO).min(total);
    Ok(Trecho {
        arquivo: aberto.relativo.to_string(),
        inicio: linhas.inicio,
        fim: linhas.fim,
        primeira,
        linhas: todas[primeira as usize - 1..ultima as usize]
            .iter()
            .map(|l| (*l).to_owned())
            .collect(),
        total,
    })
}

// ---------------------------------------------------------------------------
// Comandos

#[tauri::command]
pub async fn verificar_ambiente_projeto(projeto: String) -> Result<AmbienteClaude, String> {
    let dados = dados()?;
    bloqueante(move || verificar_ambiente(&dados, &preparador()?, &projeto)).await
}

#[tauri::command]
pub async fn planejar_preparo_projeto(projeto: String) -> Result<Resultado, String> {
    let dados = dados()?;
    bloqueante(move || planejar_preparo(&dados, &preparador()?, &projeto)).await
}

#[tauri::command]
pub async fn preparar_ambiente_projeto(projeto: String) -> Result<Resultado, String> {
    let dados = dados()?;
    bloqueante(move || {
        let feito = preparar_ambiente(&dados, &preparador()?, &projeto)?;
        tracing::info!(
            mudou = feito.houve_mudanca(),
            "ambiente do projeto preparado"
        );
        Ok(feito)
    })
    .await
}

#[tauri::command]
pub async fn listar_analises_projeto(projeto: String) -> Result<Vec<AnaliseResumo>, String> {
    let dados = dados()?;
    bloqueante(move || listar_analises(&dados, &projeto)).await
}

#[tauri::command]
pub async fn criar_analise_pergunta(
    projeto: String,
    entrada: String,
    titulo: Option<String>,
) -> Result<AnaliseCriada, String> {
    let dados = dados()?;
    bloqueante(move || criar_analise(&dados, &projeto, &entrada, titulo.as_deref())).await
}

#[tauri::command]
pub async fn carregar_analise_projeto(
    projeto: String,
    analise: String,
) -> Result<VistaAnalise, String> {
    let dados = dados()?;
    bloqueante(move || carregar_analise(&dados, &projeto, &analise)).await
}

#[tauri::command]
pub async fn trecho_evidencia_projeto(
    projeto: String,
    arquivo: String,
    inicio: u32,
    fim: u32,
) -> Result<Trecho, String> {
    let dados = dados()?;
    bloqueante(move || trecho_evidencia(&dados, &projeto, &arquivo, inicio, fim)).await
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    use cba_core::analise::{Confianca, EstadoAfirmacao};
    use cba_core::eventos::Mudanca;
    use cba_session::clientes::Plataforma;
    use cba_session::preparo::ConfigClaude;
    use serde_json::json;

    use std::path::Path;

    use super::*;
    use crate::primeiro_uso;
    use crate::vigia::{Aviso, INTERVALO, Vigias};

    const PROJETO: &str = "faturamento-exemplo";

    fn fixture() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/faturamento-exemplo")
            .canonicalize()
            .unwrap()
    }

    struct Cenario {
        dir: tempfile::TempDir,
        dados: DiretorioDados,
    }

    fn cenario() -> Cenario {
        let dir = tempfile::tempdir().unwrap();
        let dados = DiretorioDados::em(dir.path().join("dados"));
        let base = dir.path().join("base");
        primeiro_uso::configurar_base(
            &dados,
            &primeiro_uso::Restricoes::default(),
            base.to_str().unwrap(),
        )
        .unwrap();
        primeiro_uso::cadastrar_projeto(
            &dados,
            &primeiro_uso::PedidoProjeto {
                nome: "Faturamento Exemplo".into(),
                pastas: vec![fixture().to_str().unwrap().into()],
                ..Default::default()
            },
        )
        .unwrap();
        Cenario { dir, dados }
    }

    impl Cenario {
        fn ambiente(&self) -> Ambiente {
            ambiente(&self.dados, PROJETO).unwrap()
        }

        fn chamar(&self, ferramenta: &str, args: serde_json::Value) {
            if let Err(e) =
                cba_tools::registro_padrao().chamar(ferramenta, Ok(&self.ambiente()), Some(args))
            {
                panic!("{ferramenta}: {e}");
            }
        }

        fn afirmar(&self, analise: &str) {
            self.chamar(
                "afirmar",
                json!({
                    "analise": analise,
                    "secao": "resposta",
                    "texto": "Passada a carência, a multa é de 2% sobre o valor original.",
                    "confianca": "confirmado",
                    "evidencias": [{
                        "arquivo": "db/03_pkg_cobranca.sql",
                        "linha_inicial": 71,
                        "linha_final": 72,
                        "trecho": "FN_PARAMETRO('PERC_MULTA_ATRASO') / 100, 2)"
                    }]
                }),
            );
        }

        fn preparador(&self) -> Preparador {
            let exe = self.dir.path().join("bin").join("cba");
            Preparador::novo(
                ConfigClaude::da_pasta_do_usuario(&self.dir.path().join("home")),
                exe,
                Plataforma::atual(),
            )
            .unwrap()
        }
    }

    #[test]
    fn cria_a_analise_da_pergunta_com_o_pedido_de_inicio() {
        let c = cenario();
        let criada = criar_analise(
            &c.dados,
            PROJETO,
            "Como é calculada a multa por atraso de pagamento?",
            None,
        )
        .unwrap();
        assert_eq!(criada.analise.id, "A-0001");
        assert_eq!(
            criada.analise.titulo,
            "Como é calculada a multa por atraso de pagamento?"
        );
        assert_eq!(criada.analise.fluxo, Fluxo::Perguntar);
        assert_eq!(criada.analise.estado, EstadoAnalise::Criada);
        let p = &criada.prompt_inicial;
        assert!(
            p.contains("iniciar_analise") && p.contains("analise A-0001"),
            "{p}"
        );
        // Nada que o claude.cmd do Windows recusaria.
        assert!(
            !p.contains(['"', '%', '!', '(', ')', '\n', '&', '|', '<', '>', '^']),
            "{p}"
        );
        let pasta = c.dir.path().join("base").join(PROJETO).join("analises");
        assert!(pasta.join("A-0001.estado.json").is_file());
        assert!(pasta.join("A-0001.md").is_file());
        // O app avisou a tela pelo diário.
        let eventos = DiarioEventos::em(c.dados.eventos()).ler(PROJETO).unwrap();
        assert_eq!(eventos.len(), 1);
        assert_eq!(eventos[0].mudanca, Mudanca::Criada);

        let segunda =
            criar_analise(&c.dados, PROJETO, "Qual o teto de encargos?", Some("Teto")).unwrap();
        assert_eq!(segunda.analise.id, "A-0002");
        assert_eq!(segunda.analise.titulo, "Teto");
        let lista = listar_analises(&c.dados, PROJETO).unwrap();
        let ids: Vec<_> = lista.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(ids, ["A-0002", "A-0001"]);

        assert!(criar_analise(&c.dados, PROJETO, "   ", None).is_err());
        assert!(criar_analise(&c.dados, "inexistente", "Pergunta?", None).is_err());
    }

    #[test]
    fn le_o_estado_com_afirmacoes_evidencias_e_sessao() {
        let c = cenario();
        criar_analise(&c.dados, PROJETO, "Como é calculada a multa?", None).unwrap();
        c.chamar("iniciar_analise", json!({"analise": "A-0001"}));
        c.afirmar("A-0001");
        c.chamar(
            "perguntar",
            json!({"analise": "A-0001", "texto": "A carência vale também para pessoa jurídica?"}),
        );
        let id = Id::analisar("A-0001").unwrap();
        c.ambiente()
            .analises
            .alterar(&id, Mudanca::Sessao, |a, t| {
                a.iniciar_sessao("sessao-1", Cliente::ClaudeCode, t)?;
                a.atualizar_sessao("sessao-1", EstadoSessao::AguardandoAnalista, t)
            })
            .unwrap();

        let vista = carregar_analise(&c.dados, PROJETO, "A-0001").unwrap();
        assert_eq!(vista.analise.estado, EstadoAnalise::EmAndamento);
        let a = &vista.analise.afirmacoes[0];
        assert_eq!(a.confianca, Confianca::Confirmado);
        assert_eq!(a.estado, EstadoAfirmacao::Registrada);
        assert_eq!(a.evidencias[0].arquivo.as_str(), "db/03_pkg_cobranca.sql");
        assert_eq!(vista.analise.perguntas.len(), 1);
        assert_eq!(
            vista.sessao,
            Some(SessaoVista {
                id: "sessao-1".into(),
                estado: EstadoSessao::AguardandoAnalista,
                cliente: Cliente::ClaudeCode,
            })
        );
        assert!(vista.prompt_retomada.contains("retomar_analise"));
        let documento = std::fs::read_to_string(&vista.documento).unwrap();
        assert!(
            documento.contains("2% sobre o valor original"),
            "{documento}"
        );

        // Formato que a tela espera (ui/src/analise.ts).
        let json = serde_json::to_value(&vista).unwrap();
        let afirmacao = &json["analise"]["afirmacoes"][0];
        assert_eq!(afirmacao["evidencias"][0]["linhas"], "71-72");
        assert_eq!(afirmacao["estado"], "registrada");
        assert_eq!(afirmacao["confianca"], "confirmado");
        assert_eq!(json["sessao"]["estado"], "aguardando-analista");
        assert!(json["promptRetomada"].is_string());

        assert!(carregar_analise(&c.dados, PROJETO, "A-0009").is_err());
        assert!(carregar_analise(&c.dados, PROJETO, "S-0001").is_err());
    }

    #[test]
    fn trecho_da_evidencia_com_as_linhas_certas() {
        let c = cenario();
        let t = trecho_evidencia(&c.dados, PROJETO, "db/03_pkg_cobranca.sql", 71, 72).unwrap();
        assert_eq!(t.arquivo, "db/03_pkg_cobranca.sql");
        assert_eq!((t.inicio, t.fim, t.primeira), (71, 72, 63));
        let texto = std::fs::read_to_string(fixture().join("db/03_pkg_cobranca.sql")).unwrap();
        let todas: Vec<&str> = texto.lines().collect();
        assert_eq!(t.linhas.len(), 8 + 2 + 8);
        assert_eq!(t.linhas[0], todas[62]);
        // A linha 72 do arquivo fica na posição 72 - 63 do trecho.
        let l72 = &t.linhas[(72 - t.primeira) as usize];
        assert_eq!(l72, todas[71]);
        assert!(l72.contains("PERC_MULTA_ATRASO"), "{l72}");
        assert_eq!(t.total as usize, todas.len());

        // No começo do arquivo, o contexto não passa da linha 1.
        let t = trecho_evidencia(&c.dados, PROJETO, "db/03_pkg_cobranca.sql", 2, 2).unwrap();
        assert_eq!(t.primeira, 1);
        assert_eq!(t.linhas[1], todas[1]);
    }

    #[test]
    fn trecho_fora_do_escopo_e_recusado() {
        let c = cenario();
        for arquivo in [
            "../../Cargo.toml",
            "/etc/passwd",
            "C:/Windows/win.ini",
            ".env",
            "db/nao-existe.sql",
            "",
        ] {
            assert!(
                trecho_evidencia(&c.dados, PROJETO, arquivo, 1, 1).is_err(),
                "{arquivo}"
            );
        }
        let erro = trecho_evidencia(&c.dados, PROJETO, ".env", 1, 1).unwrap_err();
        assert!(erro.contains("excluído"), "{erro}");
        let erro = trecho_evidencia(&c.dados, PROJETO, "db/03_pkg_cobranca.sql", 99_000, 99_001)
            .unwrap_err();
        assert!(erro.contains("fim do arquivo"), "{erro}");
        assert!(trecho_evidencia(&c.dados, PROJETO, "db/03_pkg_cobranca.sql", 5, 4).is_err());
        assert!(trecho_evidencia(&c.dados, PROJETO, "db/03_pkg_cobranca.sql", 1, 400).is_err());
    }

    #[test]
    fn preparo_mostra_o_plano_antes_de_gravar() {
        let c = cenario();
        let p = c.preparador();
        let antes = verificar_ambiente(&c.dados, &p, PROJETO).unwrap();
        assert!(!antes.pronto);
        assert!(!antes.pendencias.is_empty());
        let plano = planejar_preparo(&c.dados, &p, PROJETO).unwrap();
        assert!(!plano.gravado);
        assert!(plano.houve_mudanca());
        assert!(!p.config.estado.exists(), "o plano não pode gravar nada");
        let feito = preparar_ambiente(&c.dados, &p, PROJETO).unwrap();
        assert!(feito.gravado);
        let depois = verificar_ambiente(&c.dados, &p, PROJETO).unwrap();
        assert!(depois.pronto, "{:?}", depois.pendencias);
        assert!(depois.pendencias.is_empty());
        assert!(verificar_ambiente(&c.dados, &p, "inexistente").is_err());
    }

    /// Critério de aceite da #36: cada afirmação registrada chega à tela em menos de
    /// 1 s (aviso da vigia mais a releitura do estado).
    #[test]
    fn afirmacao_registrada_chega_a_tela_em_menos_de_um_segundo() {
        let c = cenario();
        criar_analise(&c.dados, PROJETO, "Como é calculada a multa?", None).unwrap();
        c.chamar("iniciar_analise", json!({"analise": "A-0001"}));
        let vigias = Vigias::em(c.dados.eventos(), INTERVALO);
        let (tx, rx) = mpsc::channel();
        let tx = Mutex::new(tx);
        let assinatura = vigias.assinar(PROJETO, move |aviso| {
            let _ = tx.lock().unwrap().send(aviso.clone());
        });
        let mut pior = Duration::ZERO;
        for i in 0..3 {
            std::thread::sleep(Duration::from_millis(50));
            let antes = Instant::now();
            c.afirmar("A-0001");
            let aviso = rx.recv_timeout(Duration::from_secs(1)).expect("aviso");
            assert!(
                matches!(
                    &aviso,
                    Aviso::Mudanca { analise, mudanca: Mudanca::Afirmacao, .. } if analise == "A-0001"
                ),
                "{aviso:?}"
            );
            let vista = carregar_analise(&c.dados, PROJETO, "A-0001").unwrap();
            pior = pior.max(antes.elapsed());
            assert_eq!(vista.analise.afirmacoes.len(), i + 1);
        }
        vigias.cancelar(assinatura);
        eprintln!("registro até o estado relido pela tela: pior caso {pior:?}");
        assert!(pior < Duration::from_secs(1), "{pior:?}");
    }
}
