use std::fs;

use cba_core::analise::{EstadoAnalise, Fluxo};
use cba_core::analises::NovaAnalise;
use cba_core::eventos::DiarioEventos;
use serde_json::json;

use super::*;

const SESSAO: &str = "3f2b8c1e-9a7d-4e2f-b1c3-0d4e5f6a7b8c";

/// Diretório de dados com um sistema cadastrado (a pasta `codigo`), a base
/// configurada e uma análise criada pelo app.
struct Cenario {
    _dir: tempfile::TempDir,
    codigo: PathBuf,
    fora: PathBuf,
    dados: DiretorioDados,
    analises: Analises,
    analise: Id,
}

impl Cenario {
    fn novo() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let raiz = dunce::canonicalize(dir.path()).unwrap();
        let codigo = raiz.join("codigo");
        let fora = raiz.join("outro-projeto");
        fs::create_dir_all(codigo.join("db")).unwrap();
        fs::create_dir_all(&fora).unwrap();
        let dados = DiretorioDados::em(raiz.join("dados"));
        let sistema = cba_index::Sistema::novo("Faturamento Exemplo", [&codigo]).unwrap();
        cba_index::Cadastro::em(dados.sistemas())
            .cadastrar(&sistema)
            .unwrap();
        let ambiente = {
            Base::criar(raiz.join("base")).unwrap();
            Config {
                base: Some(raiz.join("base")),
            }
            .gravar(&dados)
            .unwrap();
            cba_core::ambiente::Ambiente::do_diretorio(&codigo, &dados).unwrap()
        };
        let analise = ambiente
            .analises
            .criar(NovaAnalise {
                fluxo: Fluxo::Perguntar,
                titulo: "Multa por atraso".into(),
                entrada: "Como é calculada a multa?".into(),
                analista: None,
                base: None,
            })
            .unwrap()
            .id;
        Self {
            _dir: dir,
            codigo,
            fora,
            dados,
            analises: ambiente.analises,
            analise,
        }
    }

    fn contexto(&self) -> Contexto {
        Contexto {
            dados: Some(self.dados.clone()),
            diretorio_projeto: None,
        }
    }

    fn sessoes(&self) -> Sessoes {
        Sessoes::do_diretorio(&self.dados)
    }

    fn registro(&self) -> Option<RegistroSessao> {
        self.sessoes().obter(SESSAO).unwrap()
    }

    fn estado_na_analise(&self) -> Option<EstadoSessao> {
        let a = self.analises.carregar(&self.analise).unwrap();
        a.sessoes.iter().find(|s| s.id == SESSAO).map(|s| s.estado)
    }

    fn enviar(&self, evento: &str, extra: Value, cwd: &Path) -> Resposta {
        self.enviar_com(evento, extra, cwd, &self.contexto())
    }

    fn enviar_com(&self, evento: &str, extra: Value, cwd: &Path, ctx: &Contexto) -> Resposta {
        let mut entrada = json!({
            "session_id": SESSAO,
            "transcript_path": "/tmp/transcricao.jsonl",
            "cwd": cwd,
            "permission_mode": "default",
            "hook_event_name": evento,
        });
        for (k, v) in extra.as_object().unwrap() {
            entrada[k] = v.clone();
        }
        tratar(evento, entrada.to_string().as_bytes(), ctx)
    }
}

#[test]
fn resposta_padrao_e_contexto_do_session_start() {
    assert_eq!(
        serde_json::to_string(&Resposta::seguir().saida).unwrap(),
        "{}"
    );
    let r = Resposta::com_contexto("SessionStart", "limites");
    assert_eq!(r.codigo, 0);
    assert_eq!(
        r.saida["hookSpecificOutput"]["hookEventName"],
        "SessionStart"
    );
    assert_eq!(
        r.saida["hookSpecificOutput"]["additionalContext"],
        "limites"
    );
    // A #31 preenche o conteúdo; até lá, nada é acrescentado.
    let inicio = InicioDeSessao {
        sistema: "x".into(),
        analise: None,
        origem: None,
    };
    assert_eq!(contexto_inicial(&inicio), None);
}

#[test]
fn entradas_ruins_nao_bloqueiam() {
    let ctx = Contexto::default();
    for (evento, entrada) in [
        ("SessionStart", &b"nao json"[..]),
        ("SessionStart", &b""[..]),
        ("Inventado", &b"{}"[..]),
        ("Stop", br#"{"hook_event_name":"SessionStart"}"#),
    ] {
        assert_eq!(tratar(evento, entrada, &ctx), Resposta::seguir());
    }
    for evento in EVENTOS {
        let entrada = format!(r#"{{"hook_event_name":"{evento}"}}"#);
        assert_eq!(tratar(evento, entrada.as_bytes(), &ctx), Resposta::seguir());
    }
}

#[test]
fn sessao_fora_de_sistema_nao_grava_nada() {
    let c = Cenario::novo();
    for evento in EVENTOS {
        let r = c.enviar(evento, json!({"source": "startup"}), &c.fora);
        assert_eq!(r, Resposta::seguir());
    }
    // Pasta inexistente também é fora.
    c.enviar(
        "SessionStart",
        json!({}),
        Path::new("/caminho/que/nao/existe"),
    );
    assert!(!c.dados.sessoes().exists());
    assert!(
        DiarioEventos::em(c.dados.eventos())
            .ler("faturamento-exemplo")
            .unwrap()
            .iter()
            .all(|e| e.mudanca == Mudanca::Criada)
    );
}

#[test]
fn identificador_de_sessao_invalido_e_ignorado() {
    let c = Cenario::novo();
    let entrada = json!({
        "session_id": "../../fora",
        "cwd": c.codigo,
        "hook_event_name": "SessionStart",
    });
    tratar(
        "SessionStart",
        entrada.to_string().as_bytes(),
        &c.contexto(),
    );
    assert!(c.sessoes().listar().unwrap().is_empty());
}

#[test]
fn ciclo_completo_de_uma_sessao_ligada_a_analise() {
    let c = Cenario::novo();
    let id = c.analise.to_string();

    // Início: registrada, ainda sem análise.
    let r = c.enviar(
        "SessionStart",
        json!({"source": "startup"}),
        &c.codigo.join("db"),
    );
    assert_eq!(r, Resposta::seguir());
    let reg = c.registro().unwrap();
    assert_eq!(reg.sistema, "faturamento-exemplo");
    assert_eq!(reg.estado, EstadoSessao::Ativa);
    assert_eq!(reg.analise, None);
    assert_eq!(reg.origem.as_deref(), Some("startup"));
    assert_eq!(
        reg.diretorio.as_deref(),
        Some(c.codigo.join("db").as_path())
    );
    assert_eq!(c.estado_na_analise(), None);

    // Ferramenta comum: nada muda, nada é gravado.
    let antes = fs::read(c.sessoes().arquivo(SESSAO).unwrap()).unwrap();
    c.enviar(
        "PreToolUse",
        json!({"tool_name": "Bash", "tool_input": {"command": "ls"}}),
        &c.codigo,
    );
    assert_eq!(
        fs::read(c.sessoes().arquivo(SESSAO).unwrap()).unwrap(),
        antes
    );

    // Ferramenta de outro servidor com `analise` não liga.
    c.enviar(
        "PreToolUse",
        json!({"tool_name": "mcp__outro__iniciar_analise", "tool_input": {"analise": id}}),
        &c.codigo,
    );
    assert_eq!(c.registro().unwrap().analise, None);

    // `iniciar_analise` liga a sessão à análise.
    c.enviar(
        "PreToolUse",
        json!({"tool_name": "mcp__cba__iniciar_analise", "tool_input": {"analise": id}}),
        &c.codigo,
    );
    assert_eq!(c.registro().unwrap().analise, Some(c.analise.clone()));
    assert_eq!(c.estado_na_analise(), Some(EstadoSessao::Ativa));
    let a = c.analises.carregar(&c.analise).unwrap();
    assert_eq!(a.estado, EstadoAnalise::EmAndamento);
    assert_eq!(a.sessoes[0].cliente, Cliente::Desconhecido);

    // Permissão pedida: aguardando o analista.
    c.enviar(
        "Notification",
        json!({"notification_type": "permission_prompt", "message": "Claude needs your permission"}),
        &c.codigo,
    );
    assert_eq!(
        c.registro().unwrap().estado,
        EstadoSessao::AguardandoAnalista
    );
    assert_eq!(
        c.estado_na_analise(),
        Some(EstadoSessao::AguardandoAnalista)
    );

    // De volta ao trabalho.
    c.enviar("UserPromptSubmit", json!({"prompt": "continue"}), &c.codigo);
    assert_eq!(c.estado_na_analise(), Some(EstadoSessao::Ativa));

    // Resposta terminada; aviso de ociosidade não muda o estado.
    c.enviar("Stop", json!({"stop_hook_active": false}), &c.codigo);
    assert_eq!(c.estado_na_analise(), Some(EstadoSessao::Concluida));
    c.enviar(
        "Notification",
        json!({"notification_type": "idle_prompt"}),
        &c.codigo,
    );
    assert_eq!(c.registro().unwrap().estado, EstadoSessao::Concluida);

    // Fim da sessão.
    c.enviar(
        "SessionEnd",
        json!({"reason": "prompt_input_exit"}),
        &c.codigo,
    );
    let reg = c.registro().unwrap();
    assert_eq!(reg.estado, EstadoSessao::Encerrada);
    assert_eq!(reg.motivo_fim.as_deref(), Some("prompt_input_exit"));
    assert!(reg.fim_ms.is_some());
    assert_eq!(c.estado_na_analise(), Some(EstadoSessao::Encerrada));

    // Evento atrasado não reabre; retomada (`claude --resume`) reabre.
    c.enviar("Stop", json!({}), &c.codigo);
    assert_eq!(c.registro().unwrap().estado, EstadoSessao::Encerrada);
    c.enviar("SessionStart", json!({"source": "resume"}), &c.fora);
    let reg = c.registro().unwrap();
    assert_eq!(reg.estado, EstadoSessao::Ativa);
    assert_eq!(reg.origem.as_deref(), Some("resume"));
    assert_eq!(reg.fim_ms, None);
    assert_eq!(c.estado_na_analise(), Some(EstadoSessao::Ativa));
    let a = c.analises.carregar(&c.analise).unwrap();
    assert_eq!(a.sessoes.len(), 1);

    // O app foi notificado a cada mudança da sessão na análise.
    let eventos = DiarioEventos::em(c.dados.eventos())
        .ler("faturamento-exemplo")
        .unwrap();
    let da_sessao = eventos
        .iter()
        .filter(|e| e.mudanca == Mudanca::Sessao)
        .count();
    assert_eq!(da_sessao, 6, "{eventos:?}");

    // Consultável pelo app.
    let da_analise = c
        .sessoes()
        .da_analise("faturamento-exemplo", &c.analise)
        .unwrap();
    assert_eq!(da_analise.len(), 1);
    assert_eq!(da_analise[0].id, SESSAO);
}

#[test]
fn trocar_de_analise_conclui_a_anterior() {
    let c = Cenario::novo();
    let outra = c
        .analises
        .criar(NovaAnalise {
            fluxo: Fluxo::Explicar,
            titulo: "Cancelamento".into(),
            entrada: "Explique o cancelamento".into(),
            analista: None,
            base: None,
        })
        .unwrap()
        .id;
    for id in [&c.analise, &outra] {
        c.enviar(
            "PreToolUse",
            json!({"tool_name": "mcp__cba__retomar_analise", "tool_input": {"analise": id.to_string()}}),
            &c.codigo,
        );
    }
    assert_eq!(c.registro().unwrap().analise, Some(outra.clone()));
    assert_eq!(c.estado_na_analise(), Some(EstadoSessao::Concluida));
    let b = c.analises.carregar(&outra).unwrap();
    assert_eq!(b.sessoes[0].estado, EstadoSessao::Ativa);
}

#[test]
fn analise_aprovada_ou_inexistente_nao_impede_o_registro() {
    let c = Cenario::novo();
    c.enviar(
        "PreToolUse",
        json!({"tool_name": "mcp__cba__afirmar", "tool_input": {"analise": "A-0999"}}),
        &c.codigo,
    );
    let reg = c.registro().unwrap();
    assert_eq!(
        reg.analise.map(|a| a.to_string()).as_deref(),
        Some("A-0999")
    );
    // Identificador que não é de análise não liga.
    c.enviar(
        "PreToolUse",
        json!({"tool_name": "mcp__cba__afirmar", "tool_input": {"analise": "S-0001"}}),
        &c.codigo,
    );
    assert_eq!(
        c.registro()
            .unwrap()
            .analise
            .map(|a| a.to_string())
            .as_deref(),
        Some("A-0999")
    );
}

#[test]
fn claude_project_dir_identifica_quando_o_cwd_saiu_da_pasta() {
    let c = Cenario::novo();
    let ctx = Contexto {
        diretorio_projeto: Some(c.codigo.clone()),
        ..c.contexto()
    };
    c.enviar_com("SessionStart", json!({}), &c.fora, &ctx);
    let reg = c.registro().unwrap();
    assert_eq!(reg.sistema, "faturamento-exemplo");
    assert_eq!(reg.diretorio.as_deref(), Some(c.codigo.as_path()));
}

#[test]
fn worktree_do_git_conta_como_o_sistema() {
    let c = Cenario::novo();
    // Repositório principal na pasta do sistema e uma worktree fora dela.
    let git = c.codigo.join(".git");
    fs::create_dir_all(git.join("worktrees/wt")).unwrap();
    fs::write(git.join("HEAD"), "ref: refs/heads/main\n").unwrap();
    fs::write(git.join("worktrees/wt/HEAD"), "ref: refs/heads/wt\n").unwrap();
    fs::write(git.join("worktrees/wt/commondir"), "../..\n").unwrap();
    let wt = c.fora.join("wt");
    fs::create_dir_all(wt.join("db")).unwrap();
    fs::write(
        wt.join(".git"),
        format!("gitdir: {}\n", git.join("worktrees/wt").display()),
    )
    .unwrap();

    let sistemas = cba_index::Cadastro::em(c.dados.sistemas())
        .listar()
        .unwrap();
    assert_eq!(
        sistema_da_pasta(&wt.join("db"), &sistemas).as_deref(),
        Some("faturamento-exemplo")
    );
    assert_eq!(sistema_da_pasta(&c.fora, &sistemas), None);

    c.enviar("SessionStart", json!({}), &wt.join("db"));
    assert_eq!(c.registro().unwrap().sistema, "faturamento-exemplo");
}

#[test]
fn pasta_mais_especifica_vence() {
    let dir = tempfile::tempdir().unwrap();
    let raiz = dunce::canonicalize(dir.path()).unwrap();
    fs::create_dir_all(raiz.join("a/b/c")).unwrap();
    let externo = cba_index::Sistema::novo("Externo", [raiz.join("a")]).unwrap();
    let interno = cba_index::Sistema::novo("Interno", [raiz.join("a/b")]).unwrap();
    let sistemas = [externo, interno];
    assert_eq!(
        sistema_da_pasta(&raiz.join("a/b/c"), &sistemas).as_deref(),
        Some("interno")
    );
    assert_eq!(
        sistema_da_pasta(&raiz.join("a"), &sistemas).as_deref(),
        Some("externo")
    );
}

#[test]
fn sincronizar_liga_reabre_e_respeita_o_modelo() {
    let mut a = Analise::nova(Serie::Analise.id(1), "x", Fluxo::Perguntar, "t", "e", 0);
    sincronizar(
        &mut a,
        "s",
        Cliente::Desconhecido,
        EstadoSessao::AguardandoAnalista,
        1,
    )
    .unwrap();
    assert_eq!(a.sessoes[0].estado, EstadoSessao::AguardandoAnalista);
    sincronizar(
        &mut a,
        "s",
        Cliente::Desconhecido,
        EstadoSessao::Encerrada,
        2,
    )
    .unwrap();
    sincronizar(
        &mut a,
        "s",
        Cliente::Desconhecido,
        EstadoSessao::Encerrada,
        3,
    )
    .unwrap();
    assert_eq!(a.sessoes[0].fim_ms, Some(2));
    sincronizar(
        &mut a,
        "s",
        Cliente::Desconhecido,
        EstadoSessao::Concluida,
        4,
    )
    .unwrap();
    assert_eq!(a.sessoes[0].estado, EstadoSessao::Concluida);
    assert_eq!(a.sessoes.len(), 1);
}

#[test]
fn notificacoes() {
    let tipo = |t: Value| destino("Notification", &json!({ "notification_type": t }));
    assert_eq!(
        tipo(json!("permission_prompt")),
        Some(EstadoSessao::AguardandoAnalista)
    );
    assert_eq!(
        tipo(json!("elicitation_dialog")),
        Some(EstadoSessao::AguardandoAnalista)
    );
    assert_eq!(tipo(Value::Null), Some(EstadoSessao::AguardandoAnalista));
    assert_eq!(tipo(json!("idle_prompt")), None);
    assert_eq!(tipo(json!("auth_success")), None);
}
