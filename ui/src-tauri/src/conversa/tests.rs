use std::path::{Path, PathBuf};

use cba_kb::Id;
use cba_session::sessoes::{RegistroSessao, Sessoes};

use super::*;
use crate::primeiro_uso;

const SISTEMA: &str = "faturamento-exemplo";

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/faturamento-exemplo")
        .canonicalize()
        .unwrap()
}

struct Cenario {
    _dir: tempfile::TempDir,
    dados: DiretorioDados,
    /// Pasta do sistema como os links a levam.
    pasta: String,
}

fn cenario() -> Cenario {
    let dir = tempfile::tempdir().unwrap();
    let dados = DiretorioDados::em(dir.path().join("dados"));
    primeiro_uso::configurar_base(
        &dados,
        &primeiro_uso::Restricoes::default(),
        dir.path().join("base").to_str().unwrap(),
    )
    .unwrap();
    primeiro_uso::cadastrar_sistema(
        &dados,
        &primeiro_uso::PedidoSistema {
            nome: "Faturamento Exemplo".into(),
            pastas: vec![fixture().to_str().unwrap().into()],
            ..Default::default()
        },
    )
    .unwrap();
    analise::criar_analise(&dados, SISTEMA, "Como é calculada a multa?", None).unwrap();
    let pasta = analise::alvo(&dados, SISTEMA)
        .unwrap()
        .diretorio
        .to_str()
        .unwrap()
        .to_owned();
    Cenario {
        _dir: dir,
        dados,
        pasta,
    }
}

fn plataforma() -> Plataforma {
    Plataforma::atual()
}

/// Uma sessão vista pelos hooks e ligada à análise.
fn sessao_ligada(dados: &DiretorioDados, id: &str, inicio_ms: u64) {
    Sessoes::do_diretorio(dados)
        .alterar(id, |_| {
            let mut r = RegistroSessao::novo(id, SISTEMA, inicio_ms);
            r.analise = Some(Id::analisar("A-0001").unwrap());
            Some(r)
        })
        .unwrap();
}

#[test]
fn analise_nova_sem_preferencia_oferece_os_tres_clientes_com_o_pedido_de_inicio() {
    let c = cenario();
    let o = opcoes_conversa(
        &c.dados,
        SISTEMA,
        "A-0001",
        Desktop::Instalado,
        plataforma(),
    )
    .unwrap();
    assert_eq!(o.preferido, None);
    assert_eq!(
        o.opcoes.iter().map(|o| o.cliente).collect::<Vec<_>>(),
        ClienteConversa::TODOS
    );
    assert!(o.pedido.contains("iniciar_analise"), "{}", o.pedido);
    assert_eq!(o.sessao, None);
    assert_eq!(o.comando_retomada, None);
    assert_eq!(o.problema, None);
    let pasta = std::path::Path::new(&c.pasta);
    assert_eq!(
        o.link_desktop.as_deref(),
        Some(
            passagem::link_desktop(pasta, Some(&o.pedido), plataforma())
                .unwrap()
                .as_str()
        )
    );
    let link = o.link_desktop.unwrap();
    assert!(link.starts_with("claude://code/new?folder="), "{link}");
    assert!(
        link.contains("&q=Inicie%20a%20an%C3%A1lise%20A-0001"),
        "{link}"
    );
    let link = o.link_terminal.unwrap();
    assert!(link.starts_with("claude-cli://open?cwd="), "{link}");
    let comando = o.comando_terminal.unwrap();
    assert!(comando.contains(&c.pasta), "{comando}");
    assert!(
        comando.contains("claude 'Inicie a análise A-0001"),
        "{comando}"
    );
}

#[test]
fn desktop_preferido_vem_primeiro_e_desktop_ausente_vai_para_o_fim() {
    let c = cenario();
    primeiro_uso::escolher_cliente(&c.dados, ClientePreferido::ClaudeDesktop).unwrap();
    let o = opcoes_conversa(
        &c.dados,
        SISTEMA,
        "A-0001",
        Desktop::Instalado,
        plataforma(),
    )
    .unwrap();
    assert_eq!(o.preferido, Some(ClienteConversa::ClaudeDesktop));
    assert_eq!(o.opcoes[0].cliente, ClienteConversa::ClaudeDesktop);
    let o = opcoes_conversa(
        &c.dados,
        SISTEMA,
        "A-0001",
        Desktop::NaoSuportado,
        plataforma(),
    )
    .unwrap();
    assert_eq!(o.opcoes[0].cliente, ClienteConversa::TerminalEmbutido);
    assert!(!o.opcoes[2].disponivel);
}

#[test]
fn com_sessao_oferece_a_retomada_pelo_identificador() {
    let c = cenario();
    sessao_ligada(&c.dados, "sessao-1", 1);
    let o = opcoes_conversa(
        &c.dados,
        SISTEMA,
        "A-0001",
        Desktop::Instalado,
        plataforma(),
    )
    .unwrap();
    assert_eq!(o.sessao.as_deref(), Some("sessao-1"));
    assert!(o.pedido.contains("retomar_analise"), "{}", o.pedido);
    let retomada = o.comando_retomada.unwrap();
    assert!(retomada.ends_with("claude --resume sessao-1"), "{retomada}");
    assert!(retomada.contains(&c.pasta), "{retomada}");
}

#[test]
fn link_do_desktop_marca_a_sessao_que_nascer_dele() {
    let c = cenario();
    let link = link_da_conversa(
        &c.dados,
        SISTEMA,
        "A-0001",
        ClienteConversa::ClaudeDesktop,
        plataforma(),
    )
    .unwrap();
    assert!(link.starts_with("claude://code/new?"), "{link}");
    assert!(
        link_da_conversa(
            &c.dados,
            SISTEMA,
            "A-0001",
            ClienteConversa::TerminalEmbutido,
            plataforma()
        )
        .is_err()
    );
    // A sessão do Desktop aparece depois, ligada pelos hooks, com cliente desconhecido.
    sessao_ligada(&c.dados, "sessao-desktop", agora_ms() + 5);
    let vista = analise::carregar_analise(&c.dados, SISTEMA, "A-0001").unwrap();
    assert_eq!(vista.sessao.unwrap().cliente, Cliente::ClaudeDesktop);

    let link = link_da_conversa(
        &c.dados,
        SISTEMA,
        "A-0001",
        ClienteConversa::TerminalDoAnalista,
        plataforma(),
    )
    .unwrap();
    assert!(link.starts_with("claude-cli://open?cwd="), "{link}");
}

#[test]
fn passagem_do_terminal_embutido_para_o_desktop() {
    let c = cenario();
    // O terminal embutido marcou o identificador que passou ao `claude`.
    ClientesDasSessoes::no_diretorio(c.dados.raiz())
        .marcar_sessao("sessao-2", Cliente::ClaudeCode, 1)
        .unwrap();
    sessao_ligada(&c.dados, "sessao-2", 1);
    let cliente = || {
        analise::carregar_analise(&c.dados, SISTEMA, "A-0001")
            .unwrap()
            .sessao
            .unwrap()
            .cliente
    };
    assert_eq!(cliente(), Cliente::ClaudeCode);
    // `/desktop`.
    marcar_cliente(&c.dados, SISTEMA, "A-0001", Cliente::ClaudeDesktop).unwrap();
    assert_eq!(cliente(), Cliente::ClaudeDesktop);
}
