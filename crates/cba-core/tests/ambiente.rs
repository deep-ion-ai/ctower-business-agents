//! Identificação do sistema pelo `CBA_SISTEMA` e por worktrees do git.

use std::fs;
use std::path::Path;

use cba_core::ambiente::{Ambiente, ErroAmbiente};
use cba_core::config::Config;
use cba_core::dados::DiretorioDados;
use cba_index::{Cadastro, Sistema};

const C1: &str = "1111111111111111111111111111111111111111";
const C2: &str = "2222222222222222222222222222222222222222";

fn escrever(caminho: &Path, texto: &str) {
    fs::create_dir_all(caminho.parent().unwrap()).unwrap();
    fs::write(caminho, texto).unwrap();
}

/// Dados do app com o sistema de pasta `pasta` cadastrado e a base configurada.
fn preparar(raiz: &Path, pasta: &Path) -> DiretorioDados {
    let dados = DiretorioDados::em(raiz.join("dados"));
    let sistema = Sistema::novo("Faturamento Exemplo", [pasta]).unwrap();
    Cadastro::em(dados.sistemas()).cadastrar(&sistema).unwrap();
    Config {
        base: Some(raiz.join("base")),
    }
    .gravar(&dados)
    .unwrap();
    cba_kb::Base::criar(raiz.join("base")).unwrap();
    dados
}

/// Repositório `principal` com a pasta `sistema/`, e uma worktree em `wt` com a
/// mesma pasta em outra versão. Os `.git` são escritos à mão, como o git faz.
fn repositorio_com_worktree(principal: &Path, wt: &Path) {
    escrever(&principal.join(".git/HEAD"), "ref: refs/heads/main\n");
    escrever(&principal.join(".git/refs/heads/main"), C1);
    escrever(&principal.join(".git/refs/heads/outra"), C2);
    escrever(
        &principal.join("sistema/regra.sql"),
        "-- versao principal\n",
    );
    let gd = principal.join(".git/worktrees/wt");
    escrever(&gd.join("HEAD"), "ref: refs/heads/outra\n");
    escrever(&gd.join("commondir"), "../..\n");
    escrever(&wt.join(".git"), &format!("gitdir: {}\n", gd.display()));
    escrever(&wt.join("sistema/regra.sql"), "-- versao da worktree\n");
}

#[test]
fn cba_sistema_identifica_o_sistema_em_qualquer_diretorio() {
    let dir = tempfile::tempdir().unwrap();
    let codigo = dir.path().join("codigo");
    escrever(&codigo.join("a.sql"), "select 1;\n");
    let dados = preparar(dir.path(), &codigo);
    let fora = tempfile::tempdir().unwrap();

    // Sem a variável, fora das pastas do sistema: fora de sistema.
    assert!(matches!(
        Ambiente::do_processo(fora.path(), None, &dados).unwrap_err(),
        ErroAmbiente::ForaDeSistema(_)
    ));
    // Variável vazia conta como ausente.
    assert!(matches!(
        Ambiente::do_processo(fora.path(), Some(" "), &dados).unwrap_err(),
        ErroAmbiente::ForaDeSistema(_)
    ));
    // Com a variável, o sistema vem dela.
    let amb = Ambiente::do_processo(fora.path(), Some("faturamento-exemplo"), &dados).unwrap();
    assert_eq!(amb.sistema.id, "faturamento-exemplo");
    assert!(amb.escopo.resolver("a.sql").is_ok());

    // Sistema descadastrado: erro que diz o que fazer.
    let erro = Ambiente::do_processo(&codigo, Some("outro-sistema"), &dados).unwrap_err();
    assert!(
        matches!(&erro, ErroAmbiente::SistemaDesconhecido(id) if id == "outro-sistema"),
        "{erro:?}"
    );
    assert_eq!(erro.tipo(), "sistema_desconhecido");
    assert!(erro.to_string().contains("CBA_SISTEMA"));
}

#[test]
fn worktree_fora_da_pasta_cadastrada_usa_o_sistema_do_repositorio_principal() {
    let dir = tempfile::tempdir().unwrap();
    let principal = dir.path().join("principal");
    let wt = dir.path().join("wt");
    repositorio_com_worktree(&principal, &wt);
    let dados = preparar(dir.path(), &principal.join("sistema"));

    for (diretorio, sistema) in [
        (wt.join("sistema"), None),
        (wt.clone(), Some("faturamento-exemplo")),
        (wt.join("sistema"), Some("faturamento-exemplo")),
    ] {
        let amb = Ambiente::do_processo(&diretorio, sistema, &dados).unwrap();
        assert_eq!(amb.sistema.id, "faturamento-exemplo");
        // O cadastro continua apontando para o repositório principal...
        assert!(
            amb.sistema.pastas[0]
                .caminho
                .starts_with(dunce::canonicalize(&principal).unwrap())
        );
        // ...e o escopo lê os arquivos da worktree, que são os que o agente vê.
        let arquivo = amb.escopo.resolver("regra.sql").unwrap();
        assert_eq!(
            fs::read_to_string(&arquivo).unwrap(),
            "-- versao da worktree\n",
            "{diretorio:?}"
        );
    }

    // A raiz da worktree está fora da pasta do sistema (`sistema/`): sem a
    // variável, continua fora de sistema.
    assert!(matches!(
        Ambiente::do_processo(&wt, None, &dados).unwrap_err(),
        ErroAmbiente::ForaDeSistema(_)
    ));

    // No repositório principal, nada muda.
    let amb = Ambiente::do_processo(&principal.join("sistema"), None, &dados).unwrap();
    let arquivo = amb.escopo.resolver("regra.sql").unwrap();
    assert_eq!(
        fs::read_to_string(arquivo).unwrap(),
        "-- versao principal\n"
    );
}

#[test]
fn worktree_dentro_da_pasta_cadastrada_le_a_worktree() {
    // Como o `.claude/worktrees/<nome>` do Claude Code: a worktree fica dentro da
    // pasta do repositório principal, que é a pasta cadastrada.
    let dir = tempfile::tempdir().unwrap();
    let principal = dir.path().join("principal");
    let wt = principal.join(".claude/worktrees/wt");
    repositorio_com_worktree(&principal, &wt);
    let dados = preparar(dir.path(), &principal);

    let amb = Ambiente::do_processo(&wt.join("sistema"), None, &dados).unwrap();
    let arquivo = amb.escopo.resolver("sistema/regra.sql").unwrap();
    assert_eq!(
        fs::read_to_string(&arquivo).unwrap(),
        "-- versao da worktree\n"
    );
    assert!(arquivo.starts_with(dunce::canonicalize(&wt).unwrap()));
}
