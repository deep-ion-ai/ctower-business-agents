use std::path::Path;

use cba_core::analise::Cliente;
use cba_kb::Id;

use super::*;
use crate::sessoes::RegistroSessao;

const PEDIDO: &str = "Inicie a análise A-0193 pelo ctower: chame a ferramenta iniciar_analise do cba com analise A-0193 e siga o roteiro devolvido.";

/// Decodificação de URL, para conferir a volta.
fn decodificar(texto: &str) -> String {
    let b = texto.as_bytes();
    let mut saida = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let hex = std::str::from_utf8(&b[i + 1..i + 3]).unwrap();
            saida.push(u8::from_str_radix(hex, 16).unwrap());
            i += 3;
        } else {
            saida.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(saida).unwrap()
}

/// Parâmetros da consulta de um link, decodificados.
fn parametros(link: &str) -> Vec<(String, String)> {
    let (_, consulta) = link.split_once('?').unwrap();
    consulta
        .split('&')
        .map(|par| {
            let (k, v) = par.split_once('=').unwrap();
            (k.to_owned(), decodificar(v))
        })
        .collect()
}

#[test]
fn link_do_desktop_com_pasta_e_pedido() {
    let link = link_desktop(
        Path::new("/Users/ana/sistemas/faturamento exemplo"),
        Some(PEDIDO),
        Plataforma::MacOs,
    )
    .unwrap();
    assert_eq!(
        link,
        "claude://code/new?folder=/Users/ana/sistemas/faturamento%20exemplo\
         &q=Inicie%20a%20an%C3%A1lise%20A-0193%20pelo%20ctower%3A%20chame%20a%20ferramenta\
         %20iniciar_analise%20do%20cba%20com%20analise%20A-0193%20e%20siga%20o%20roteiro\
         %20devolvido."
    );
    assert_eq!(
        parametros(&link),
        [
            (
                "folder".into(),
                "/Users/ana/sistemas/faturamento exemplo".into()
            ),
            ("q".into(), PEDIDO.into())
        ]
    );
}

#[test]
fn link_do_desktop_no_windows_codifica_a_pasta() {
    let link = link_desktop(
        Path::new(r"C:\Users\Ana\Sistemas\Faturamento & Cia"),
        Some("Retome a análise A-0001"),
        Plataforma::Windows,
    )
    .unwrap();
    assert_eq!(
        link,
        "claude://code/new?folder=C%3A%5CUsers%5CAna%5CSistemas%5CFaturamento%20%26%20Cia\
         &q=Retome%20a%20an%C3%A1lise%20A-0001"
    );
}

#[test]
fn caracteres_especiais_do_pedido_nao_quebram_o_link() {
    let pedido = "multa & juros = 2% + 1%?\n#segunda linha 'aspas' \"duplas\" ação/ção";
    let link = link_desktop(Path::new("/s"), Some(pedido), Plataforma::Linux).unwrap();
    let (_, q) = link.split_once("&q=").unwrap();
    // Só caracteres que não mudam o sentido da URL.
    assert!(
        q.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-._~%".contains(&b)),
        "{q}"
    );
    assert!(q.contains("%0A"), "quebra de linha como %0A: {q}");
    assert_eq!(parametros(&link)[1].1, pedido);
    // O mesmo no link do terminal.
    let link = link_terminal(Path::new("/s"), Some(pedido), Plataforma::Linux).unwrap();
    assert_eq!(parametros(&link)[1], ("q".into(), pedido.into()));
}

#[test]
fn pedido_limpo_vazio_e_longo() {
    // Caracteres de controle saem; espaços nas pontas também.
    let link = link_desktop(
        Path::new("/s"),
        Some("  oi\x1b[2J\u{7}  "),
        Plataforma::MacOs,
    )
    .unwrap();
    assert_eq!(link, "claude://code/new?folder=/s&q=oi%5B2J");
    // Começando com `-`, sem o espaço de proteção dos argumentos.
    let link = link_terminal(Path::new("/s"), Some("--help"), Plataforma::MacOs).unwrap();
    assert_eq!(link, "claude-cli://open?cwd=/s&q=--help");
    // Vazio: sem `q`.
    assert_eq!(
        link_desktop(Path::new("/s"), Some(" \n "), Plataforma::MacOs).unwrap(),
        "claude://code/new?folder=/s"
    );
    assert_eq!(
        link_terminal(Path::new("/s"), None, Plataforma::MacOs).unwrap(),
        "claude-cli://open?cwd=/s"
    );
    // 5.000 caracteres passam (contados como caracteres, não bytes); 5.001, não.
    let limite = "á".repeat(MAX_PEDIDO);
    assert!(link_terminal(Path::new("/s"), Some(&limite), Plataforma::MacOs).is_ok());
    let longo = limite + "a";
    assert_eq!(
        link_terminal(Path::new("/s"), Some(&longo), Plataforma::MacOs),
        Err(ErroPassagem::PedidoLongo)
    );
    assert_eq!(
        link_desktop(Path::new("/s"), Some(&longo), Plataforma::MacOs),
        Err(ErroPassagem::PedidoLongo)
    );
}

#[test]
fn link_do_terminal_do_analista() {
    let link = link_terminal(
        Path::new("/home/ana/faturamento"),
        Some("Retome a análise A-0001 pelo ctower"),
        Plataforma::Linux,
    )
    .unwrap();
    assert_eq!(
        link,
        "claude-cli://open?cwd=/home/ana/faturamento&q=Retome%20a%20an%C3%A1lise%20A-0001%20pelo%20ctower"
    );
    let link = link_terminal(Path::new(r"D:\fat"), Some("oi"), Plataforma::Windows).unwrap();
    assert_eq!(link, "claude-cli://open?cwd=D%3A%5Cfat&q=oi");
}

#[test]
fn pasta_recusada() {
    let recusa = |p: &str, plataforma| {
        assert!(
            matches!(
                link_desktop(Path::new(p), Some("x"), plataforma),
                Err(ErroPassagem::PastaInvalida(_))
            ),
            "{p}"
        );
        assert!(
            link_terminal(Path::new(p), Some("x"), plataforma).is_err(),
            "{p}"
        );
    };
    recusa("relativa/pasta", Plataforma::MacOs);
    recusa("//servidor/compartilhada", Plataforma::Linux);
    recusa("/s/../etc", Plataforma::Linux);
    recusa("/s/a\u{202E}b", Plataforma::MacOs);
    recusa("/s/a\u{200B}b", Plataforma::MacOs);
    recusa("/s/a\nb", Plataforma::MacOs);
    recusa(r"\\servidor\pasta", Plataforma::Windows);
    recusa(r"C:\s\..\x", Plataforma::Windows);
    recusa(r"C:relativa", Plataforma::Windows);
    recusa("/s", Plataforma::Windows);
    recusa(r"\\?\UNC\servidor\pasta", Plataforma::Windows);
    // Caminho canônico do Windows, sem o prefixo.
    assert_eq!(
        link_terminal(Path::new(r"\\?\C:\fat"), None, Plataforma::Windows).unwrap(),
        "claude-cli://open?cwd=C%3A%5Cfat"
    );
    // `..` dentro de um nome não é segmento.
    assert!(link_desktop(Path::new("/s/a..b"), None, Plataforma::Linux).is_ok());
}

#[test]
fn comando_para_copiar_no_macos_e_no_linux() {
    assert_eq!(
        comando_terminal(
            Path::new("/Users/ana/faturamento"),
            AcaoTerminal::Pedido("Inicie a análise A-0193 pelo ctower"),
            Plataforma::MacOs
        )
        .unwrap(),
        "cd /Users/ana/faturamento && claude 'Inicie a análise A-0193 pelo ctower'"
    );
    assert_eq!(
        comando_terminal(
            Path::new("/home/ana/meus sistemas/fat"),
            AcaoTerminal::Pedido("o cliente d'água\nsegunda linha"),
            Plataforma::Linux
        )
        .unwrap(),
        r"cd '/home/ana/meus sistemas/fat' && claude 'o cliente d'\''água segunda linha'"
    );
    assert_eq!(
        comando_terminal(
            Path::new("/s"),
            AcaoTerminal::Retomar("550e8400-e29b-41d4-a716-446655440000"),
            Plataforma::Linux
        )
        .unwrap(),
        "cd /s && claude --resume 550e8400-e29b-41d4-a716-446655440000"
    );
    // `$`, crase e `~` ficam entre aspas.
    assert_eq!(
        comando_terminal(
            Path::new("/s/$HOME/`x`"),
            AcaoTerminal::Pedido("$(rm -rf /)"),
            Plataforma::Linux
        )
        .unwrap(),
        "cd '/s/$HOME/`x`' && claude '$(rm -rf /)'"
    );
    assert_eq!(
        comando_terminal(
            Path::new("/s"),
            AcaoTerminal::Pedido(" "),
            Plataforma::Linux
        )
        .unwrap(),
        "cd /s && claude"
    );
}

#[test]
fn comando_para_copiar_no_windows() {
    assert_eq!(
        comando_terminal(
            Path::new(r"C:\Users\Ana\Faturamento [2024]"),
            AcaoTerminal::Pedido("Inicie a análise A-0193 pelo ctower"),
            Plataforma::Windows
        )
        .unwrap(),
        r"Set-Location -LiteralPath 'C:\Users\Ana\Faturamento [2024]'; claude 'Inicie a análise A-0193 pelo ctower'"
    );
    assert_eq!(
        comando_terminal(
            Path::new(r"C:\O'Brien"),
            AcaoTerminal::Pedido("d’água $env:X"),
            Plataforma::Windows
        )
        .unwrap(),
        "Set-Location -LiteralPath 'C:\\O''Brien'; claude 'd’’água $env:X'"
    );
    assert_eq!(
        comando_terminal(
            Path::new(r"C:\s"),
            AcaoTerminal::Retomar("abc-1"),
            Plataforma::Windows
        )
        .unwrap(),
        r"Set-Location -LiteralPath 'C:\s'; claude --resume abc-1"
    );
}

#[test]
fn comando_recusa_id_invalido() {
    assert_eq!(
        comando_terminal(
            Path::new("/s"),
            AcaoTerminal::Retomar("a; rm"),
            Plataforma::Linux
        ),
        Err(ErroPassagem::IdInvalido)
    );
    assert!(
        comando_terminal(
            Path::new("/s\n"),
            AcaoTerminal::Retomar("a"),
            Plataforma::Linux
        )
        .is_err()
    );
}

#[test]
fn escolha_do_cliente_pelo_preferido() {
    let ordem = |preferido, desktop| -> Vec<(ClienteConversa, bool)> {
        opcoes(preferido, desktop)
            .into_iter()
            .map(|o| (o.cliente, o.disponivel))
            .collect()
    };
    use ClienteConversa::*;
    // Sem preferência: o terminal embutido (recomendado) primeiro.
    assert_eq!(
        ordem(None, Desktop::Instalado),
        [
            (TerminalEmbutido, true),
            (ClaudeDesktop, true),
            (TerminalDoAnalista, true)
        ]
    );
    assert_eq!(
        ordem(Some(ClaudeDesktop), Desktop::Instalado),
        [
            (ClaudeDesktop, true),
            (TerminalEmbutido, true),
            (TerminalDoAnalista, true)
        ]
    );
    assert_eq!(
        ordem(Some(TerminalDoAnalista), Desktop::Instalado),
        [
            (TerminalDoAnalista, true),
            (TerminalEmbutido, true),
            (ClaudeDesktop, true)
        ]
    );
    // Desktop preferido, mas ausente: vai para o fim, com o motivo.
    let lista = opcoes(Some(ClaudeDesktop), Desktop::NaoInstalado);
    assert_eq!(
        lista.iter().map(|o| o.cliente).collect::<Vec<_>>(),
        [TerminalEmbutido, TerminalDoAnalista, ClaudeDesktop]
    );
    assert!(!lista[2].disponivel);
    assert!(
        lista[2]
            .motivo
            .as_deref()
            .unwrap()
            .contains("não foi encontrado")
    );
    let lista = opcoes(None, Desktop::NaoSuportado);
    assert!(lista[2].motivo.as_deref().unwrap().contains("Linux"));
    assert!(
        lista[..2]
            .iter()
            .all(|o| o.disponivel && o.motivo.is_none())
    );
    // JSON como a tela recebe.
    let v = serde_json::to_value(&lista[2]).unwrap();
    assert_eq!(v["cliente"], "claudeDesktop");
    assert_eq!(v["disponivel"], false);
    assert_eq!(ClaudeDesktop.cliente(), Cliente::ClaudeDesktop);
    assert_eq!(TerminalDoAnalista.cliente(), Cliente::ClaudeCode);
}

fn registro(id: &str, analise: Option<&str>, inicio_ms: u64) -> RegistroSessao {
    let mut r = RegistroSessao::novo(id, "fat", inicio_ms);
    r.analise = analise.map(|a| Id::analisar(a).unwrap());
    r
}

#[test]
fn cliente_informado_pelo_app() {
    let tmp = tempfile::tempdir().unwrap();
    let marcas = ClientesDasSessoes::no_diretorio(tmp.path());
    let r = registro("s-1", Some("A-0001"), 10_000);
    assert_eq!(marcas.cliente_de(&r), Cliente::Desconhecido);

    // Terminal embutido, depois `/desktop`, depois de volta ao terminal.
    marcas
        .marcar_sessao("s-1", Cliente::ClaudeCode, 10_000)
        .unwrap();
    assert_eq!(marcas.cliente_de(&r), Cliente::ClaudeCode);
    marcas
        .marcar_sessao("s-1", Cliente::ClaudeDesktop, 20_000)
        .unwrap();
    assert_eq!(marcas.cliente_de(&r), Cliente::ClaudeDesktop);
    marcas
        .marcar_sessao("s-1", Cliente::ClaudeCode, 30_000)
        .unwrap();
    assert_eq!(marcas.cliente_de(&r), Cliente::ClaudeCode);

    // O que os hooks souberem vale mais.
    let mut doshooks = r.clone();
    doshooks.cliente = Cliente::ClaudeDesktop;
    assert_eq!(marcas.cliente_de(&doshooks), Cliente::ClaudeDesktop);
}

#[test]
fn link_marca_a_proxima_sessao_da_analise() {
    let tmp = tempfile::tempdir().unwrap();
    let marcas = ClientesDasSessoes::no_diretorio(tmp.path());
    marcas
        .marcar_analise("fat", "A-0002", Cliente::ClaudeDesktop, 50_000)
        .unwrap();
    // Sessão da análise que começou depois do link: Desktop.
    assert_eq!(
        marcas.cliente_de(&registro("s-2", Some("A-0002"), 60_000)),
        Cliente::ClaudeDesktop
    );
    // Anterior ao link, de outra análise ou sem análise: desconhecido.
    assert_eq!(
        marcas.cliente_de(&registro("s-3", Some("A-0002"), 40_000)),
        Cliente::Desconhecido
    );
    assert_eq!(
        marcas.cliente_de(&registro("s-4", Some("A-0003"), 60_000)),
        Cliente::Desconhecido
    );
    assert_eq!(
        marcas.cliente_de(&registro("s-5", None, 60_000)),
        Cliente::Desconhecido
    );
    // A marca da sessão vale mais que a da análise.
    marcas
        .marcar_sessao("s-2", Cliente::ClaudeCode, 70_000)
        .unwrap();
    assert_eq!(
        marcas.cliente_de(&registro("s-2", Some("A-0002"), 60_000)),
        Cliente::ClaudeCode
    );
}

#[test]
fn marcas_antigas_saem_e_arquivo_invalido_vale_como_vazio() {
    let tmp = tempfile::tempdir().unwrap();
    let arquivo = tmp.path().join("clientes.json");
    std::fs::write(&arquivo, "não é json").unwrap();
    let marcas = ClientesDasSessoes::em(&arquivo);
    assert_eq!(
        marcas.cliente_de(&registro("x", None, 1)),
        Cliente::Desconhecido
    );
    for i in 0..(marcas::MAX_MARCAS as u64 + 3) {
        marcas
            .marcar_sessao(&format!("s-{i}"), Cliente::ClaudeCode, 1_000 + i)
            .unwrap();
    }
    let v: serde_json::Value = serde_json::from_slice(&std::fs::read(&arquivo).unwrap()).unwrap();
    assert_eq!(v["sessoes"].as_object().unwrap().len(), marcas::MAX_MARCAS);
    assert!(v["sessoes"].get("s-0").is_none());
    assert_eq!(v["sessoes"]["s-502"]["cliente"], "claude-code");
}
