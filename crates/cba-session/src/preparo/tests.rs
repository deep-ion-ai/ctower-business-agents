//! Testes do preparo com pastas temporárias: nada aqui precisa do Claude instalado
//! nem toca a configuração real do usuário.

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs;
use std::time::Duration;

use serde_json::{Value, json};
use tempfile::TempDir;

use super::*;
use crate::clientes::{ErroExecucao, Saida};

struct Cenario {
    _tmp: TempDir,
    raiz: PathBuf,
    config: ConfigClaude,
    exe: PathBuf,
}

impl Cenario {
    fn novo() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let raiz = dunce::canonicalize(tmp.path()).unwrap();
        let home = raiz.join("home");
        fs::create_dir_all(&home).unwrap();
        let exe = raiz
            .join("app")
            .join(if cfg!(windows) { "cba.exe" } else { "cba" });
        Self {
            config: ConfigClaude::da_pasta_do_usuario(&home),
            _tmp: tmp,
            raiz,
            exe,
        }
    }

    fn preparador(&self) -> Preparador {
        Preparador::novo(self.config.clone(), self.exe.clone(), Plataforma::atual()).unwrap()
    }

    fn preparador_com(&self, exe: PathBuf) -> Preparador {
        Preparador::novo(self.config.clone(), exe, Plataforma::atual()).unwrap()
    }

    /// Pasta de projeto fora de repositório git.
    fn pasta(&self, nome: &str) -> PathBuf {
        let p = self.raiz.join("projetos").join(nome);
        fs::create_dir_all(p.join("src")).unwrap();
        fs::write(p.join("src").join("a.sql"), "select 1;\n").unwrap();
        p
    }

    fn alvo(&self, id: &str, pasta: &Path) -> Alvo {
        Alvo::da_pasta(id, pasta).unwrap()
    }

    fn json(caminho: &Path) -> Value {
        serde_json::from_slice(&fs::read(caminho).unwrap()).unwrap()
    }

    fn estado(&self) -> Value {
        Self::json(&self.config.estado)
    }

    fn settings(&self) -> Value {
        Self::json(&self.config.settings)
    }
}

fn chave(p: &Path) -> String {
    chave_do_caminho(p, Plataforma::atual())
}

/// Todos os caminhos (relativos) dentro de uma pasta.
fn arvore(pasta: &Path) -> BTreeSet<PathBuf> {
    let mut v = BTreeSet::new();
    let mut pilha = vec![pasta.to_path_buf()];
    while let Some(p) = pilha.pop() {
        for e in fs::read_dir(&p).unwrap() {
            let e = e.unwrap().path();
            v.insert(e.strip_prefix(pasta).unwrap().to_path_buf());
            if e.is_dir() {
                pilha.push(e);
            }
        }
    }
    v
}

/// Repositório git mínimo (só os arquivos que `detectar_git` lê) com uma worktree.
fn repo_com_worktree(base: &Path) -> (PathBuf, PathBuf) {
    let repo = base.join("repo");
    let git = repo.join(".git");
    fs::create_dir_all(git.join("refs").join("heads")).unwrap();
    fs::write(git.join("HEAD"), "ref: refs/heads/main\n").unwrap();
    let wt = base.join("wt");
    let meta = git.join("worktrees").join("wt");
    fs::create_dir_all(&meta).unwrap();
    fs::write(meta.join("HEAD"), "ref: refs/heads/outra\n").unwrap();
    fs::write(meta.join("commondir"), "../..\n").unwrap();
    fs::create_dir_all(wt.join("modulo")).unwrap();
    fs::write(wt.join(".git"), format!("gitdir: {}\n", meta.display())).unwrap();
    (repo, wt)
}

fn comando(exe: &Path, evento: &str) -> String {
    let caminho = exe.to_string_lossy().replace('\\', "/");
    let caminho = if cfg!(windows) {
        caminho
    } else {
        exe.to_string_lossy().into_owned()
    };
    format!("\"{caminho}\" hook {evento}")
}

#[test]
fn prepara_em_configuracao_vazia() {
    let c = Cenario::novo();
    let pasta = c.pasta("faturamento");
    let alvo = c.alvo("faturamento-exemplo", &pasta);
    assert_eq!(alvo.chave, chave(&pasta));
    assert!(!alvo.worktree);

    let r = c.preparador().preparar(&alvo).unwrap();
    assert!(r.gravado);
    assert_eq!(r.diretorio, pasta);
    assert!(r.backups.is_empty(), "não havia arquivo antes");
    assert_eq!(r.alteracoes.len(), 2 + EVENTOS.len());
    assert!(r.alteracoes.iter().all(|a| a.acao == Acao::Adicionado));
    assert_eq!(
        r.alteracoes[1].item,
        Item::Confianca {
            chave: alvo.chave.clone()
        }
    );

    let estado = c.estado();
    assert_eq!(
        estado["projects"][&alvo.chave]["mcpServers"]["cba"],
        json!({
            "type": "stdio",
            "command": c.exe.to_string_lossy(),
            "args": ["mcp"],
            "env": { "CBA_PROJETO": "faturamento-exemplo" }
        })
    );
    assert_eq!(
        estado["projects"][&alvo.chave]["hasTrustDialogAccepted"],
        true
    );
    let settings = c.settings();
    for evento in EVENTOS {
        assert_eq!(
            settings["hooks"][evento],
            json!([{ "hooks": [{
                "type": "command",
                "command": comando(&c.exe, evento),
                "timeout": LIMITE_HOOK_S
            }]}]),
            "{evento}"
        );
    }
    assert!(c.preparador().verificar(&alvo).unwrap().pronto);
}

#[test]
fn preparar_de_novo_nao_muda_nada() {
    let c = Cenario::novo();
    let pasta = c.pasta("faturamento");
    let alvo = c.alvo("faturamento-exemplo", &pasta);
    c.preparador().preparar(&alvo).unwrap();
    let antes = (
        fs::read(&c.config.estado).unwrap(),
        fs::read(&c.config.settings).unwrap(),
    );
    let r = c.preparador().preparar(&alvo).unwrap();
    assert!(!r.houve_mudanca());
    assert!(r.backups.is_empty());
    let depois = (
        fs::read(&c.config.estado).unwrap(),
        fs::read(&c.config.settings).unwrap(),
    );
    assert_eq!(antes, depois);
}

#[test]
fn planejar_nao_grava() {
    let c = Cenario::novo();
    let pasta = c.pasta("faturamento");
    let alvo = c.alvo("faturamento-exemplo", &pasta);
    let plano = c.preparador().planejar(&alvo).unwrap();
    assert!(!plano.gravado);
    assert!(plano.houve_mudanca());
    assert!(!c.config.estado.exists());
    assert!(!c.config.settings.exists());
}

/// Configuração do analista com outros projetos, servidores e hooks.
const ESTADO_DO_ANALISTA: &str = r#"{
  "numStartups": 42,
  "oauthAccount": {
    "emailAddress": "nao-deve-ser-lido@exemplo.invalid"
  },
  "projects": {
    "/outro/projeto": {
      "allowedTools": [],
      "mcpServers": {
        "github": {
          "type": "stdio",
          "command": "npx",
          "args": [
            "-y",
            "servidor-github"
          ]
        }
      }
    },
    "CHAVE": {
      "allowedTools": [
        "Bash(ls:*)"
      ],
      "mcpServers": {
        "banco": {
          "command": "/usr/local/bin/banco-mcp",
          "args": []
        }
      },
      "hasTrustDialogAccepted": true
    }
  },
  "zUltimaChave": true
}
"#;

const SETTINGS_DO_ANALISTA: &str = r#"{
  "theme": "dark",
  "permissions": {
    "allow": [
      "Bash(git status)"
    ]
  },
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "command",
            "command": "/home/analista/bin/auditar.sh"
          }
        ]
      }
    ],
    "PostToolUse": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "fmt.sh"
          }
        ]
      }
    ]
  }
}
"#;

#[test]
fn preserva_a_configuracao_do_analista_e_desfaz_exatamente() {
    let c = Cenario::novo();
    let pasta = c.pasta("faturamento");
    let alvo = c.alvo("faturamento-exemplo", &pasta);
    // A chave entra como texto JSON (escapa as barras do Windows).
    let chave_json = serde_json::to_string(&alvo.chave).unwrap();
    let estado_original = ESTADO_DO_ANALISTA.replace("\"CHAVE\"", &chave_json);
    fs::write(&c.config.estado, &estado_original).unwrap();
    fs::create_dir_all(c.config.settings.parent().unwrap()).unwrap();
    fs::write(&c.config.settings, SETTINGS_DO_ANALISTA).unwrap();

    let r = c.preparador().preparar(&alvo).unwrap();
    assert_eq!(r.backups.len(), 2);
    let mut backup = c.config.estado.clone().into_os_string();
    backup.push(arquivo::SUFIXO_BACKUP);
    assert_eq!(fs::read_to_string(backup).unwrap(), estado_original);

    let estado = c.estado();
    let original: Value = serde_json::from_str(&estado_original).unwrap();
    assert_eq!(estado["numStartups"], 42);
    assert_eq!(estado["oauthAccount"], original["oauthAccount"]);
    assert_eq!(
        estado["projects"]["/outro/projeto"],
        original["projects"]["/outro/projeto"]
    );
    let projeto = &estado["projects"][&alvo.chave];
    assert_eq!(
        projeto["mcpServers"]["banco"],
        original["projects"][&alvo.chave]["mcpServers"]["banco"]
    );
    assert_eq!(projeto["hasTrustDialogAccepted"], true);
    assert!(projeto["mcpServers"]["cba"].is_object());
    // A ordem das chaves continua a do arquivo.
    let texto = fs::read_to_string(&c.config.estado).unwrap();
    let pos = |s: &str| texto.find(s).unwrap();
    assert!(pos("numStartups") < pos("oauthAccount"));
    assert!(pos("oauthAccount") < pos("projects"));
    assert!(pos("projects") < pos("zUltimaChave"));

    let settings = c.settings();
    assert_eq!(settings["theme"], "dark");
    assert_eq!(
        settings["hooks"]["PostToolUse"].as_array().unwrap().len(),
        1
    );
    let pre = settings["hooks"]["PreToolUse"].as_array().unwrap();
    assert_eq!(
        pre.len(),
        2,
        "o hook do analista continua, o nosso entra ao lado"
    );
    assert_eq!(pre[0]["matcher"], "Bash");

    assert_eq!(
        r.alteracoes[1].acao,
        Acao::SemMudanca,
        "o analista já confiava na pasta"
    );

    let r = c.preparador().desfazer(&alvo).unwrap();
    for a in &r.alteracoes {
        let esperada = match a.item {
            // A confiança não foi o app que deu: fica.
            Item::Confianca { .. } => Acao::SemMudanca,
            _ => Acao::Removido,
        };
        assert_eq!(a.acao, esperada, "{a:?}");
    }
    assert_eq!(
        fs::read_to_string(&c.config.estado).unwrap(),
        estado_original
    );
    assert_eq!(
        fs::read_to_string(&c.config.settings).unwrap(),
        SETTINGS_DO_ANALISTA
    );
}

#[test]
fn desfazer_sem_preparo_nao_muda_nada() {
    let c = Cenario::novo();
    let pasta = c.pasta("faturamento");
    let alvo = c.alvo("faturamento-exemplo", &pasta);
    fs::create_dir_all(c.config.settings.parent().unwrap()).unwrap();
    fs::write(&c.config.settings, SETTINGS_DO_ANALISTA).unwrap();
    let r = c.preparador().desfazer(&alvo).unwrap();
    assert!(!r.houve_mudanca());
    assert!(r.backups.is_empty());
    assert!(!c.config.estado.exists());
    assert_eq!(
        fs::read_to_string(&c.config.settings).unwrap(),
        SETTINGS_DO_ANALISTA
    );
}

#[test]
fn hooks_ficam_ate_o_ultimo_projeto_sair() {
    let c = Cenario::novo();
    let a = c.alvo("a", &c.pasta("a"));
    let b = c.alvo("b", &c.pasta("b"));
    let p = c.preparador();
    p.preparar(&a).unwrap();
    let r = p.preparar(&b).unwrap();
    assert!(
        r.alteracoes
            .iter()
            .filter(|x| matches!(x.item, Item::Hook { .. }))
            .all(|x| x.acao == Acao::SemMudanca),
        "os hooks são compartilhados"
    );

    p.desfazer(&a).unwrap();
    assert!(c.estado()["projects"].get(&a.chave).is_none());
    assert!(p.verificar(&b).unwrap().pronto, "b continua pronto");
    assert_eq!(p.verificar(&a).unwrap().servidor_mcp, Situacao::Ausente);

    p.desfazer(&b).unwrap();
    assert_eq!(c.estado(), json!({ "projects": {} }));
    assert_eq!(c.settings(), json!({}));
}

#[test]
fn worktree_usa_o_repositorio_principal_como_chave() {
    let c = Cenario::novo();
    let (repo, wt) = repo_com_worktree(&c.raiz.join("git"));
    let alvo = c.alvo("faturamento-exemplo", &wt.join("modulo"));
    assert!(alvo.worktree);
    assert_eq!(alvo.chave, chave(&repo));
    assert_eq!(alvo.diretorio, wt.join("modulo"));

    let antes_repo = arvore(&repo);
    let antes_wt = arvore(&wt);
    c.preparador().preparar(&alvo).unwrap();
    assert!(c.estado()["projects"][chave(&repo)]["mcpServers"]["cba"].is_object());
    assert!(c.estado()["projects"].get(chave(&wt)).is_none());
    assert!(c.preparador().verificar(&alvo).unwrap().pronto);
    c.preparador().desfazer(&alvo).unwrap();
    assert_eq!(arvore(&repo), antes_repo);
    assert_eq!(arvore(&wt), antes_wt);
}

#[test]
fn subpasta_de_repositorio_usa_a_raiz_do_repositorio() {
    let c = Cenario::novo();
    let (repo, _) = repo_com_worktree(&c.raiz.join("git"));
    let sub = repo.join("projeto");
    fs::create_dir_all(&sub).unwrap();
    let alvo = c.alvo("x", &sub);
    assert!(!alvo.worktree);
    assert_eq!(alvo.chave, chave(&repo));
    assert_eq!(alvo.diretorio, sub);
}

#[test]
fn nenhum_arquivo_e_criado_na_pasta_do_projeto() {
    let c = Cenario::novo();
    let pasta = c.pasta("faturamento");
    let antes = arvore(&pasta);
    let alvo = c.alvo("faturamento-exemplo", &pasta);
    let p = c.preparador();
    p.planejar(&alvo).unwrap();
    p.preparar(&alvo).unwrap();
    p.verificar(&alvo).unwrap();
    assert_eq!(arvore(&pasta), antes);
    p.desfazer(&alvo).unwrap();
    assert_eq!(arvore(&pasta), antes);
    // Fora da configuração do Claude, nada novo na pasta temporária.
    let tudo = arvore(&c.raiz);
    assert!(
        tudo.iter()
            .all(|p| p.starts_with("home") || p.starts_with("projetos")),
        "{tudo:?}"
    );
}

#[test]
fn outro_executavel_e_substituido_sem_duplicar() {
    let c = Cenario::novo();
    let alvo = c.alvo("s", &c.pasta("s"));
    let antigo = c
        .raiz
        .join("antigo")
        .join(if cfg!(windows) { "cba.exe" } else { "cba" });
    c.preparador_com(antigo.clone()).preparar(&alvo).unwrap();

    let p = c.preparador();
    let v = p.verificar(&alvo).unwrap();
    assert_eq!(v.servidor_mcp, Situacao::Desatualizado);
    assert!(
        v.hooks
            .iter()
            .all(|h| h.situacao == Situacao::Desatualizado)
    );

    let r = p.preparar(&alvo).unwrap();
    assert!(
        r.alteracoes
            .iter()
            .filter(|a| !matches!(a.item, Item::Confianca { .. }))
            .all(|a| a.acao == Acao::Atualizado)
    );
    assert_eq!(r.alteracoes[1].acao, Acao::SemMudanca, "já era confiável");
    let settings = c.settings();
    for evento in EVENTOS {
        let grupos = settings["hooks"][evento].as_array().unwrap();
        assert_eq!(grupos.len(), 1, "{evento}");
        assert_eq!(grupos[0]["hooks"][0]["command"], comando(&c.exe, evento));
    }
    assert!(p.verificar(&alvo).unwrap().pronto);
}

#[test]
fn servidor_cba_alheio_e_conflito() {
    let c = Cenario::novo();
    let alvo = c.alvo("s", &c.pasta("s"));
    let original = serde_json::to_string_pretty(&json!({
        "projects": { alvo.chave.clone(): { "mcpServers": {
            "cba": { "command": "/usr/bin/outra-coisa", "args": ["mcp"] }
        }}}
    }))
    .unwrap();
    fs::write(&c.config.estado, &original).unwrap();
    let p = c.preparador();
    assert!(matches!(p.preparar(&alvo), Err(Erro::Conflito { .. })));
    assert_eq!(fs::read_to_string(&c.config.estado).unwrap(), original);
    assert!(!c.config.settings.exists());
    assert_eq!(p.verificar(&alvo).unwrap().servidor_mcp, Situacao::Conflito);
    // E o descadastro não tira o que não é nosso.
    p.desfazer(&alvo).unwrap();
    assert_eq!(fs::read_to_string(&c.config.estado).unwrap(), original);
}

#[test]
fn json_invalido_nao_e_sobrescrito() {
    let c = Cenario::novo();
    let alvo = c.alvo("s", &c.pasta("s"));
    fs::write(&c.config.estado, "{ \"projects\": { segredo-quebrado").unwrap();
    let erro = c.preparador().preparar(&alvo).unwrap_err();
    assert!(matches!(erro, Erro::JsonInvalido { linha: 1, .. }));
    assert!(!erro.to_string().contains("segredo"));
    assert_eq!(
        fs::read_to_string(&c.config.estado).unwrap(),
        "{ \"projects\": { segredo-quebrado"
    );
}

#[test]
fn formato_inesperado_e_erro() {
    let c = Cenario::novo();
    let alvo = c.alvo("s", &c.pasta("s"));
    fs::create_dir_all(c.config.settings.parent().unwrap()).unwrap();
    fs::write(&c.config.settings, r#"{"hooks": {"Stop": "texto"}}"#).unwrap();
    let erro = c.preparador().preparar(&alvo).unwrap_err();
    assert!(matches!(erro, Erro::Formato { ref campo, .. } if campo == "hooks.Stop"));
}

#[test]
fn hooks_desativados_aparecem_na_verificacao() {
    let c = Cenario::novo();
    let alvo = c.alvo("s", &c.pasta("s"));
    fs::create_dir_all(c.config.settings.parent().unwrap()).unwrap();
    fs::write(&c.config.settings, r#"{"disableAllHooks": true}"#).unwrap();
    let p = c.preparador();
    p.preparar(&alvo).unwrap();
    let v = p.verificar(&alvo).unwrap();
    assert!(v.hooks_desativados);
    assert_eq!(v.servidor_mcp, Situacao::Ok);
    assert!(!v.pronto);
    // O ajuste do analista não é mexido, e o arquivo sem quebra de linha final
    // continua assim.
    assert_eq!(c.settings()["disableAllHooks"], true);
    assert!(
        !fs::read_to_string(&c.config.settings)
            .unwrap()
            .ends_with('\n')
    );
}

#[test]
fn executavel_relativo_ou_com_aspas_e_recusado() {
    let c = Cenario::novo();
    let p = Plataforma::atual();
    assert!(matches!(
        Preparador::novo(c.config.clone(), PathBuf::from("cba"), p),
        Err(Erro::ExecutavelInvalido(_))
    ));
    assert!(matches!(
        Preparador::novo(c.config.clone(), c.raiz.join("a\"b").join("cba"), p),
        Err(Erro::ExecutavelInvalido(_))
    ));
}

#[test]
fn reconhece_as_nossas_entradas() {
    assert!(e_nosso_hook("\"/opt/x/cba\" hook Stop"));
    assert!(e_nosso_hook(
        "\"C:/Users/a/AppData/Local/Ctower/cba.exe\" hook SessionStart"
    ));
    assert!(e_nosso_hook(
        "\"C:\\Program Files\\Ctower\\CBA.EXE\" hook Stop"
    ));
    assert!(!e_nosso_hook("/opt/x/cba hook Stop"));
    assert!(!e_nosso_hook("\"/opt/x/cbax\" hook Stop"));
    assert!(!e_nosso_hook("\"/opt/x/cba\" mcp"));
    assert!(e_nosso_servidor(
        &serde_json::from_str(r#"{"command":"/a/cba","args":["mcp"]}"#).unwrap()
    ));
    assert!(!e_nosso_servidor(
        &serde_json::from_str(r#"{"command":"/a/cba","args":[]}"#).unwrap()
    ));
    assert!(!e_nosso_servidor(
        &serde_json::from_str(r#"{"command":"npx","args":["mcp"]}"#).unwrap()
    ));
}

#[test]
fn chaves_no_windows() {
    let w = Plataforma::Windows;
    assert_eq!(
        chave_do_caminho(Path::new(r"C:\Users\ana\projetos\fat"), w),
        "C:/Users/ana/projetos/fat"
    );
    assert!(mesma_chave(r"C:\Users\Ana\Fat", "c:/users/ana/fat", w));
    assert!(mesma_chave("C:/x/", "C:/x", w));
    assert!(!mesma_chave("/home/Ana", "/home/ana", Plataforma::Linux));
    assert!(mesma_chave("/home/ana", "/home/ana", Plataforma::Linux));
}

#[test]
fn chave_existente_com_outra_grafia_e_reaproveitada() {
    let c = Cenario::novo();
    let alvo = c.alvo("s", &c.pasta("s"));
    let mut grafia = alvo.chave.clone();
    grafia.push('/');
    let original = serde_json::to_string_pretty(&json!({
        "projects": { grafia.clone(): { "allowedTools": [] } }
    }))
    .unwrap();
    fs::write(&c.config.estado, &original).unwrap();
    c.preparador().preparar(&alvo).unwrap();
    let estado = c.estado();
    let projetos = estado["projects"].as_object().unwrap();
    assert_eq!(projetos.len(), 1);
    assert!(projetos[&grafia]["mcpServers"]["cba"].is_object());
    let r = c.preparador().desfazer(&alvo).unwrap();
    assert!(r.voltou_ao_que_era(), "{:?}", r.mantidos);
    // O `mcpServers` não existia: sai junto.
    assert_eq!(fs::read_to_string(&c.config.estado).unwrap(), original);
}

#[cfg(unix)]
#[test]
fn permissoes_e_links_simbolicos_sao_preservados() {
    use std::os::unix::fs::PermissionsExt;

    let c = Cenario::novo();
    let alvo = c.alvo("s", &c.pasta("s"));
    fs::write(&c.config.estado, "{}").unwrap();
    fs::set_permissions(&c.config.estado, fs::Permissions::from_mode(0o600)).unwrap();
    // settings.json gerido por dotfiles: um link para outro arquivo.
    let dotfiles = c.raiz.join("home").join("dotfiles");
    fs::create_dir_all(&dotfiles).unwrap();
    fs::write(dotfiles.join("settings.json"), "{}\n").unwrap();
    fs::create_dir_all(c.config.settings.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(dotfiles.join("settings.json"), &c.config.settings).unwrap();

    c.preparador().preparar(&alvo).unwrap();
    let modo = fs::metadata(&c.config.estado).unwrap().permissions().mode() & 0o777;
    assert_eq!(modo, 0o600);
    assert!(
        fs::symlink_metadata(&c.config.settings)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    let destino: Value =
        serde_json::from_slice(&fs::read(dotfiles.join("settings.json")).unwrap()).unwrap();
    assert!(destino["hooks"]["Stop"].is_array());
}

#[cfg(unix)]
#[test]
fn arquivo_novo_so_para_o_dono() {
    use std::os::unix::fs::PermissionsExt;

    let c = Cenario::novo();
    let alvo = c.alvo("s", &c.pasta("s"));
    c.preparador().preparar(&alvo).unwrap();
    let modo = fs::metadata(&c.config.estado).unwrap().permissions().mode() & 0o777;
    assert_eq!(modo, 0o600);
}

#[test]
fn do_projeto_usa_a_pasta_raiz() {
    let c = Cenario::novo();
    let raiz = c.pasta("fat");
    let extra = c.pasta("telas");
    let projeto = cba_index::Projeto::novo("Faturamento Exemplo", [&raiz, &extra]).unwrap();
    let alvo = Alvo::do_projeto(&projeto).unwrap();
    assert_eq!(alvo.projeto, "faturamento-exemplo");
    assert_eq!(alvo.diretorio, raiz);
}

/// Ambiente falso só com variáveis e pasta do usuário.
struct Falso {
    vars: Vec<(&'static str, OsString)>,
    home: Option<PathBuf>,
}

impl Ambiente for Falso {
    fn plataforma(&self) -> Plataforma {
        Plataforma::Linux
    }
    fn var(&self, nome: &str) -> Option<OsString> {
        self.vars
            .iter()
            .find(|(n, _)| *n == nome)
            .map(|(_, v)| v.clone())
    }
    fn home(&self) -> Option<PathBuf> {
        self.home.clone()
    }
    fn e_arquivo(&self, _: &Path) -> bool {
        unreachable!("o preparo não consulta caminhos pelo ambiente")
    }
    fn e_pasta(&self, _: &Path) -> bool {
        unreachable!()
    }
    fn nomes_na_pasta(&self, _: &Path) -> Vec<OsString> {
        unreachable!()
    }
    fn executar(&self, _: &Path, _: &[&str], _: Duration) -> Result<Saida, ErroExecucao> {
        unreachable!("o preparo não executa o claude")
    }
}

#[test]
fn localiza_a_configuracao() {
    let padrao = Falso {
        vars: vec![],
        home: Some("/home/ana".into()),
    };
    assert_eq!(
        ConfigClaude::localizar(&padrao).unwrap(),
        ConfigClaude {
            estado: "/home/ana/.claude.json".into(),
            settings: "/home/ana/.claude/settings.json".into(),
        }
    );
    let outro = Falso {
        vars: vec![("CLAUDE_CONFIG_DIR", "/cfg/claude".into())],
        home: Some("/home/ana".into()),
    };
    assert_eq!(
        ConfigClaude::localizar(&outro).unwrap(),
        ConfigClaude {
            estado: "/cfg/claude/.claude.json".into(),
            settings: "/cfg/claude/settings.json".into(),
        }
    );
    let vazio = Falso {
        vars: vec![("CLAUDE_CONFIG_DIR", "".into())],
        home: None,
    };
    assert!(ConfigClaude::localizar(&vazio).is_none());
    assert!(matches!(
        Preparador::do_ambiente(&vazio, "/opt/cba".into()),
        Err(Erro::SemPastaDoUsuario)
    ));
}

// ---------------------------------------------------------------------------
// Proteção opcional contra edição (#29)

#[test]
fn regra_de_protecao_absoluta_em_cada_plataforma() {
    use protecao::regra_de_protecao;
    assert_eq!(
        regra_de_protecao(
            Path::new("/home/ana/projetos/faturamento"),
            Plataforma::Linux
        ),
        "Edit(//home/ana/projetos/faturamento/**)"
    );
    assert_eq!(
        regra_de_protecao(Path::new("/Users/ana/faturamento/"), Plataforma::MacOs),
        "Edit(//Users/ana/faturamento/**)"
    );
    assert_eq!(
        regra_de_protecao(Path::new(r"C:\Users\ana\faturamento"), Plataforma::Windows),
        "Edit(//c/Users/ana/faturamento/**)"
    );
    // Caracteres de padrão valem literalmente.
    assert_eq!(
        regra_de_protecao(Path::new("/srv/fat[2026]*"), Plataforma::Linux),
        r"Edit(//srv/fat\[2026\]\*/**)"
    );
}

#[test]
fn negar_e_liberar_preservam_o_resto_e_tiram_so_o_pedido() {
    let c = Cenario::novo();
    fs::create_dir_all(c.config.settings.parent().unwrap()).unwrap();
    fs::write(
        &c.config.settings,
        r#"{
  "theme": "dark",
  "permissions": {
    "allow": ["Bash(npm test)"],
    "deny": ["Read(./.env)", "Edit(//ja/era/do/analista/**)"]
  }
}"#,
    )
    .unwrap();
    let p = c.preparador();
    let pasta = c.pasta("faturamento");
    let mut regras = p.regras_de_protecao(&[pasta.clone(), pasta.clone()]);
    assert_eq!(regras.len(), 1);
    regras.push("Edit(//ja/era/do/analista/**)".into());

    assert_eq!(p.negacoes_presentes(&regras).unwrap(), [false, true]);
    let feito = p.negar(&regras).unwrap();
    assert_eq!(feito.alteradas, [regras[0].clone()]);
    assert_eq!(feito.backups.len(), 1);
    assert_eq!(p.negacoes_presentes(&regras).unwrap(), [true, true]);
    // Idempotente.
    assert!(p.negar(&regras).unwrap().alteradas.is_empty());

    let json = Cenario::json(&c.config.settings);
    assert_eq!(json["theme"], "dark");
    assert_eq!(json["permissions"]["allow"], json!(["Bash(npm test)"]));
    assert_eq!(
        json["permissions"]["deny"],
        json!(["Read(./.env)", "Edit(//ja/era/do/analista/**)", regras[0]])
    );

    let feito = p.liberar(&[regras[0].clone()]).unwrap();
    assert_eq!(feito.alteradas, [regras[0].clone()]);
    let json = Cenario::json(&c.config.settings);
    assert_eq!(
        json["permissions"]["deny"],
        json!(["Read(./.env)", "Edit(//ja/era/do/analista/**)"])
    );
}

#[test]
fn liberar_tira_listas_que_ficaram_vazias_e_aceita_arquivo_ausente() {
    let c = Cenario::novo();
    let p = c.preparador();
    let regras = p.regras_de_protecao(&[c.pasta("faturamento")]);
    assert!(p.liberar(&regras).unwrap().alteradas.is_empty());
    assert_eq!(p.negacoes_presentes(&regras).unwrap(), [false]);
    p.negar(&regras).unwrap();
    assert_eq!(
        Cenario::json(&c.config.settings),
        json!({"permissions": {"deny": [regras[0]]}})
    );
    p.liberar(&regras).unwrap();
    assert_eq!(Cenario::json(&c.config.settings), json!({}));
}

#[test]
fn protecao_desligada_tira_so_as_regras_do_app() {
    let c = Cenario::novo();
    let p = c.preparador();
    let dados = cba_core::dados::DiretorioDados::em(c.raiz.join("dados"));
    let pasta_a = c.pasta("faturamento");
    let pasta_b = c.pasta("faturamento-db");
    let projeto = cba_index::Projeto::novo("Faturamento Exemplo", [&pasta_a, &pasta_b]).unwrap();
    // O analista já tinha negado a primeira pasta por conta própria.
    let regras = p.regras_de_protecao(&[pasta_a, pasta_b]);
    p.negar(&regras[..1]).unwrap();

    let s = crate::protecao::situacao(&dados, &p, &projeto).unwrap();
    assert!(!s.ligada && !s.completa);
    assert_eq!(s.negadas_pelo_hook, ["Bash", "PowerShell"]);
    assert!(!crate::protecao::protegido(&dados, &projeto.id));

    let s = crate::protecao::ligar(&dados, &p, &projeto).unwrap();
    assert!(s.ligada && s.completa);
    assert_eq!(s.arquivo, c.config.settings);
    assert_eq!(
        s.regras
            .iter()
            .map(|r| (r.presente, r.do_app))
            .collect::<Vec<_>>(),
        [(true, false), (true, true)]
    );
    assert!(crate::protecao::protegido(&dados, &projeto.id));
    // Ligar de novo não muda nada nem esquece o que é do app.
    crate::protecao::ligar(&dados, &p, &projeto).unwrap();
    assert_eq!(
        crate::protecao::registro(&dados, &projeto.id)
            .unwrap()
            .adicionadas,
        [regras[1].clone()]
    );

    let tiradas = crate::protecao::desligar(&dados, &p, &projeto.id).unwrap();
    assert_eq!(tiradas, [regras[1].clone()]);
    assert!(!crate::protecao::protegido(&dados, &projeto.id));
    assert_eq!(p.negacoes_presentes(&regras).unwrap(), [true, false]);
    // Desligar o que já está desligado não faz nada.
    assert!(
        crate::protecao::desligar(&dados, &p, &projeto.id)
            .unwrap()
            .is_empty()
    );
}

// ---------------------------------------------------------------------------
// Outra instalação do app (#213)

/// Executável de outra instalação do app (a de desenvolvimento, por exemplo).
fn outra_instalacao(c: &Cenario) -> PathBuf {
    c.raiz
        .join("dev")
        .join("target")
        .join(if cfg!(windows) { "cba.exe" } else { "cba" })
}

/// Configuração com os hooks e o servidor de outra instalação, indentada com quatro
/// espaços (como no teste de uso completo).
fn com_outra_instalacao(c: &Cenario, com_projeto: bool) -> (String, String) {
    let dev = outra_instalacao(c);
    let mut hooks = serde_json::Map::new();
    for evento in EVENTOS {
        hooks.insert(
            evento.to_string(),
            json!([{ "hooks": [{
                "type": "command",
                "command": comando(&dev, evento),
                "timeout": LIMITE_HOOK_S
            }]}]),
        );
    }
    let settings = json!({
        "theme": "dark",
        "hooks": hooks,
        "zDepois": [1, 2]
    });
    let mut projetos = serde_json::Map::new();
    if com_projeto {
        projetos.insert(
            "/projetos/do-dev".into(),
            json!({ "mcpServers": { "cba": {
                "type": "stdio",
                "command": dev.to_string_lossy(),
                "args": ["mcp"],
                "env": { "CBA_PROJETO": "do-dev" }
            }}, "hasTrustDialogAccepted": true }),
        );
    }
    let estado = json!({ "numStartups": 3, "projects": projetos });
    let quatro = |v: &Value| {
        let mut b = Vec::new();
        let f = serde_json::ser::PrettyFormatter::with_indent(b"    ");
        let mut s = serde_json::Serializer::with_formatter(&mut b, f);
        serde::Serialize::serialize(v, &mut s).unwrap();
        String::from_utf8(b).unwrap() + "\n"
    };
    let (settings, estado) = (quatro(&settings), quatro(&estado));
    fs::create_dir_all(c.config.settings.parent().unwrap()).unwrap();
    fs::write(&c.config.settings, &settings).unwrap();
    fs::write(&c.config.estado, &estado).unwrap();
    (settings, estado)
}

#[test]
fn hooks_de_outra_instalacao_voltam_ao_desfazer() {
    for com_projeto in [true, false] {
        let c = Cenario::novo();
        let (settings, estado) = com_outra_instalacao(&c, com_projeto);
        let alvo = c.alvo("teste", &c.pasta("teste"));
        let dev = outra_instalacao(&c);
        let dev_no_hook = comando(&dev, "X")
            .trim_start_matches('"')
            .split('"')
            .next()
            .unwrap()
            .to_string();
        let p = c.preparador();

        // O plano avisa que os hooks vão para outra instalação hoje.
        let plano = p.planejar(&alvo).unwrap();
        assert_eq!(plano.outras_instalacoes(), [dev_no_hook.as_str()]);
        let hooks: Vec<_> = plano
            .alteracoes
            .iter()
            .filter(|a| matches!(a.item, Item::Hook { .. }))
            .collect();
        assert_eq!(hooks.len(), EVENTOS.len());
        assert!(hooks.iter().all(|a| a.acao == Acao::Atualizado
            && a.outra_instalacao.as_deref() == Some(dev_no_hook.as_str())));

        p.preparar(&alvo).unwrap();
        assert!(p.verificar(&alvo).unwrap().pronto);
        assert!(c.config.registro().exists());
        // A indentação do analista fica.
        assert!(
            fs::read_to_string(&c.config.settings)
                .unwrap()
                .contains("\n    \"theme\"")
        );

        let r = p.desfazer(&alvo).unwrap();
        assert!(r.voltou_ao_que_era(), "{:?}", r.mantidos);
        assert!(
            r.alteracoes
                .iter()
                .filter(|a| matches!(a.item, Item::Hook { .. }))
                .all(|a| a.acao == Acao::Restaurado),
            "{:?}",
            r.alteracoes
        );
        // Diff vazio.
        assert_eq!(fs::read_to_string(&c.config.settings).unwrap(), settings);
        assert_eq!(fs::read_to_string(&c.config.estado).unwrap(), estado);
        assert!(
            !c.config.registro().exists(),
            "o registro sai quando fica vazio"
        );
    }
}

#[test]
fn servidor_de_outra_instalacao_na_mesma_pasta_volta_ao_desfazer() {
    let c = Cenario::novo();
    let alvo = c.alvo("s", &c.pasta("s"));
    c.preparador_com(outra_instalacao(&c))
        .preparar(&alvo)
        .unwrap();
    // Sem registro da outra instalação (como num preparo de uma versão anterior).
    fs::remove_file(c.config.registro()).unwrap();
    let antes = (
        fs::read(&c.config.estado).unwrap(),
        fs::read(&c.config.settings).unwrap(),
    );

    let p = c.preparador();
    let r = p.preparar(&alvo).unwrap();
    assert_eq!(r.alteracoes[0].acao, Acao::Atualizado);
    assert!(r.alteracoes[0].outra_instalacao.is_some());
    let r = p.desfazer(&alvo).unwrap();
    assert_eq!(r.alteracoes[0].acao, Acao::Restaurado);
    assert!(r.voltou_ao_que_era(), "{:?}", r.mantidos);
    let depois = (
        fs::read(&c.config.estado).unwrap(),
        fs::read(&c.config.settings).unwrap(),
    );
    assert_eq!(antes, depois);
}

#[test]
fn hooks_em_uso_por_outra_pasta_ficam_e_sao_ditos() {
    let c = Cenario::novo();
    let (settings, estado) = com_outra_instalacao(&c, true);
    let a = c.alvo("a", &c.pasta("a"));
    let b = c.alvo("b", &c.pasta("b"));
    let p = c.preparador();
    p.preparar(&a).unwrap();
    p.preparar(&b).unwrap();

    let r = p.desfazer(&a).unwrap();
    assert!(!r.voltou_ao_que_era());
    assert!(matches!(
        &r.mantidos[..],
        [Mantido::HooksEmUso { pastas, .. }] if pastas == std::slice::from_ref(&b.chave)
    ));
    assert!(p.verificar(&b).unwrap().pronto, "b continua pronto");

    let r = p.desfazer(&b).unwrap();
    assert!(r.voltou_ao_que_era(), "{:?}", r.mantidos);
    assert_eq!(fs::read_to_string(&c.config.settings).unwrap(), settings);
    assert_eq!(fs::read_to_string(&c.config.estado).unwrap(), estado);
}

#[test]
fn hook_mudado_depois_do_preparo_nao_e_sobrescrito() {
    let c = Cenario::novo();
    com_outra_instalacao(&c, false);
    let alvo = c.alvo("s", &c.pasta("s"));
    let p = c.preparador();
    p.preparar(&alvo).unwrap();
    // O analista acrescenta um handler seu ao Stop depois do preparo.
    let mut s = c.settings();
    s["hooks"]["Stop"][0]["hooks"]
        .as_array_mut()
        .unwrap()
        .push(json!({ "type": "command", "command": "som.sh" }));
    fs::write(
        &c.config.settings,
        serde_json::to_string_pretty(&s).unwrap(),
    )
    .unwrap();

    let r = p.desfazer(&alvo).unwrap();
    assert!(!r.voltou_ao_que_era());
    assert!(
        r.mantidos.iter().any(|m| matches!(
            m,
            Mantido::HooksMudaramDepois { eventos, .. } if eventos == &["Stop".to_string()]
        )),
        "{:?}",
        r.mantidos
    );
    let stop = &c.settings()["hooks"]["Stop"];
    assert_eq!(
        stop,
        &json!([{ "hooks": [{ "type": "command", "command": "som.sh" }] }])
    );
    // Os outros eventos voltaram para a outra instalação.
    let dev = outra_instalacao(&c);
    assert_eq!(
        c.settings()["hooks"]["SessionStart"][0]["hooks"][0]["command"],
        comando(&dev, "SessionStart")
    );
}

#[test]
fn entrada_que_o_claude_gravou_na_pasta_fica_e_e_dita() {
    let c = Cenario::novo();
    let alvo = c.alvo("s", &c.pasta("s"));
    let p = c.preparador();
    p.preparar(&alvo).unwrap();
    // O Claude Code grava dados da pasta ao abrir uma conversa.
    let mut e = c.estado();
    e["projects"][&alvo.chave]["lastCost"] = json!(0);
    fs::write(&c.config.estado, serde_json::to_string_pretty(&e).unwrap()).unwrap();

    let r = p.desfazer(&alvo).unwrap();
    assert!(matches!(
        &r.mantidos[..],
        [Mantido::EntradaDoClaude { chave, .. }] if *chave == alvo.chave
    ));
    assert_eq!(
        c.estado()["projects"][&alvo.chave],
        json!({ "lastCost": 0 }),
        "o servidor, o mcpServers e a confiança, que não existiam, saem"
    );
}

#[test]
fn instalacoes_em_pilha_voltam_ao_original() {
    // A de desenvolvimento prepara P1; a de teste prepara P2 (toma os hooks); a de
    // desenvolvimento prepara P1 de novo (toma de volta). Desfazer as duas volta ao
    // arquivo original.
    let c = Cenario::novo();
    fs::create_dir_all(c.config.settings.parent().unwrap()).unwrap();
    fs::write(&c.config.settings, SETTINGS_DO_ANALISTA).unwrap();
    let p1 = c.alvo("p1", &c.pasta("p1"));
    let p2 = c.alvo("p2", &c.pasta("p2"));
    let dev = c.preparador_com(outra_instalacao(&c));
    let teste = c.preparador();
    dev.preparar(&p1).unwrap();
    teste.preparar(&p2).unwrap();
    dev.preparar(&p1).unwrap();
    assert!(dev.verificar(&p1).unwrap().pronto);

    let r = teste.desfazer(&p2).unwrap();
    assert!(r.voltou_ao_que_era(), "{:?}", r.mantidos);
    assert!(
        dev.verificar(&p1).unwrap().pronto,
        "os hooks ficam com a dev"
    );
    dev.desfazer(&p1).unwrap();
    assert_eq!(
        fs::read_to_string(&c.config.settings).unwrap(),
        SETTINGS_DO_ANALISTA
    );
    assert!(!c.config.registro().exists());
}

#[test]
fn sem_registro_os_hooks_ficam_se_outra_pasta_usa() {
    // Preparo de uma versão anterior do app: o desfazer não sabe o que havia antes.
    let c = Cenario::novo();
    let a = c.alvo("a", &c.pasta("a"));
    let b = c.alvo("b", &c.pasta("b"));
    c.preparador_com(outra_instalacao(&c)).preparar(&a).unwrap();
    let p = c.preparador();
    p.preparar(&b).unwrap();
    fs::remove_file(c.config.registro()).unwrap();

    let r = p.desfazer(&b).unwrap();
    assert!(matches!(
        &r.mantidos[..],
        [Mantido::HooksEmUso { pastas, .. }] if pastas == std::slice::from_ref(&a.chave)
    ));
    assert_eq!(p.verificar(&b).unwrap().servidor_mcp, Situacao::Ausente);
}

#[test]
fn registro_ilegivel_nao_impede_o_desfazer() {
    let c = Cenario::novo();
    let alvo = c.alvo("s", &c.pasta("s"));
    let p = c.preparador();
    p.preparar(&alvo).unwrap();
    fs::write(c.config.registro(), "{ quebrado").unwrap();
    let r = p.desfazer(&alvo).unwrap();
    assert!(
        r.alteracoes
            .iter()
            .filter(|a| !matches!(a.item, Item::Confianca { .. }))
            .all(|a| a.acao == Acao::Removido)
    );
    assert_eq!(c.settings(), json!({}));
}

// ---------------------------------------------------------------------------
// Confiança na pasta (#218)

#[test]
fn preparo_marca_a_pasta_como_confiavel_e_o_desfazer_tira() {
    let c = Cenario::novo();
    // O analista já tem outra pasta registrada (o arquivo não é criado pelo app).
    fs::write(
        &c.config.estado,
        "{\n  \"numStartups\": 1,\n  \"projects\": {}\n}\n",
    )
    .unwrap();
    let original = fs::read_to_string(&c.config.estado).unwrap();
    let alvo = c.alvo("s", &c.pasta("s"));
    let p = c.preparador();

    let antes = p.verificar(&alvo).unwrap();
    assert!(!antes.pasta_confiavel);
    let plano = p.planejar(&alvo).unwrap();
    assert!(plano.alteracoes.iter().any(|a| a.item
        == Item::Confianca {
            chave: alvo.chave.clone()
        }
        && a.acao == Acao::Adicionado));

    p.preparar(&alvo).unwrap();
    let v = p.verificar(&alvo).unwrap();
    assert!(v.pasta_confiavel && v.pronto);

    let r = p.desfazer(&alvo).unwrap();
    assert!(r.voltou_ao_que_era(), "{:?}", r.mantidos);
    assert!(
        r.alteracoes
            .iter()
            .any(|a| matches!(a.item, Item::Confianca { .. }) && a.acao == Acao::Removido)
    );
    assert_eq!(fs::read_to_string(&c.config.estado).unwrap(), original);
}

#[test]
fn sem_confianca_o_ambiente_nao_esta_pronto() {
    let c = Cenario::novo();
    let alvo = c.alvo("s", &c.pasta("s"));
    let p = c.preparador();
    p.preparar(&alvo).unwrap();
    // O analista (ou uma versão anterior do app) deixou a pasta sem confiança.
    let mut e = c.estado();
    e["projects"][&alvo.chave]
        .as_object_mut()
        .unwrap()
        .remove("hasTrustDialogAccepted");
    fs::write(&c.config.estado, serde_json::to_string_pretty(&e).unwrap()).unwrap();
    let v = p.verificar(&alvo).unwrap();
    assert!(!v.pasta_confiavel);
    assert!(!v.pronto);
    assert!(
        crate::terminal::claude::pendencias(&v)
            .iter()
            .any(|t| t.contains("confiável"))
    );
    let r = p.preparar(&alvo).unwrap();
    assert_eq!(r.alteracoes[1].acao, Acao::Adicionado);
    assert!(p.verificar(&alvo).unwrap().pronto);
}

#[test]
fn confianca_recusada_antes_volta_ao_desfazer() {
    let c = Cenario::novo();
    let alvo = c.alvo("s", &c.pasta("s"));
    let chave_json = serde_json::to_string(&alvo.chave).unwrap();
    let original = format!(
        "{{\n  \"projects\": {{\n    {chave_json}: {{\n      \"hasTrustDialogAccepted\": false\n    }}\n  }}\n}}\n"
    );
    fs::write(&c.config.estado, &original).unwrap();
    let p = c.preparador();
    p.preparar(&alvo).unwrap();
    assert_eq!(
        c.estado()["projects"][&alvo.chave]["hasTrustDialogAccepted"],
        true
    );
    p.desfazer(&alvo).unwrap();
    assert_eq!(fs::read_to_string(&c.config.estado).unwrap(), original);
}

#[test]
fn confianca_fica_com_a_outra_instalacao_que_ainda_usa_a_pasta() {
    let c = Cenario::novo();
    let alvo = c.alvo("s", &c.pasta("s"));
    let dev = c.preparador_com(outra_instalacao(&c));
    let teste = c.preparador();
    // A de teste prepara primeiro (dá a confiança); a dev prepara a mesma pasta.
    teste.preparar(&alvo).unwrap();
    let r = dev.preparar(&alvo).unwrap();
    assert_eq!(r.alteracoes[1].acao, Acao::SemMudanca);

    // Desfazer a de teste devolve o servidor da dev; a confiança continua.
    teste.desfazer(&alvo).unwrap();
    assert_eq!(
        c.estado()["projects"][&alvo.chave]["hasTrustDialogAccepted"],
        true
    );
    // A dev herdou a confiança e a tira ao desfazer.
    let r = dev.desfazer(&alvo).unwrap();
    assert!(
        r.alteracoes
            .iter()
            .any(|a| matches!(a.item, Item::Confianca { .. }) && a.acao == Acao::Removido)
    );
    assert!(c.estado()["projects"].get(&alvo.chave).is_none());
    assert!(!c.config.registro().exists());
}

#[test]
fn worktree_fica_confiavel_pela_chave_do_repositorio_principal() {
    let c = Cenario::novo();
    let (repo, wt) = repo_com_worktree(&c.raiz);
    let alvo = c.alvo("w", &wt.join("modulo"));
    c.preparador().preparar(&alvo).unwrap();
    assert_eq!(
        c.estado()["projects"][&chave(&repo)]["hasTrustDialogAccepted"],
        true
    );
}
