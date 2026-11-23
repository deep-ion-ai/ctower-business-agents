//! Testes da migração com pastas temporárias e cópias das fixtures: nada aqui toca a
//! configuração real do usuário nem prepara pastas dentro do repositório.
//!
//! As fixtures reproduzem o que as versões antigas deixavam: `v0.1.0` (hooks e
//! proteção no `settings.json`, sem registro do estado anterior) e `v0.5.0` (com o
//! `settings.json.cba-anterior` na versão 1). Os caminhos são marcadores (`@EXE@`,
//! `@CHAVE_A@`, `@HOOK:<evento>@`, `@REGRA_A@`) trocados pelos do cenário.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use tempfile::TempDir;

use super::*;
use crate::clientes::Plataforma;
use crate::hooks::EVENTOS;
use crate::preparo::{ConfigClaude, arquivo_local, protecao::regra_de_protecao};

const V010: &[(&str, &str)] = &[
    (
        "settings.json",
        include_str!("fixtures/v0.1.0/settings.json"),
    ),
    ("claude.json", include_str!("fixtures/v0.1.0/claude.json")),
    (
        "protecao-faturamento.json",
        include_str!("fixtures/v0.1.0/protecao-faturamento.json"),
    ),
];

const V050: &[(&str, &str)] = &[
    (
        "settings.json",
        include_str!("fixtures/v0.5.0/settings.json"),
    ),
    ("claude.json", include_str!("fixtures/v0.5.0/claude.json")),
    (
        "protecao-faturamento.json",
        include_str!("fixtures/v0.5.0/protecao-faturamento.json"),
    ),
    (
        "settings.json.cba-anterior",
        include_str!("fixtures/v0.5.0/settings.json.cba-anterior"),
    ),
];

struct Cenario {
    _tmp: TempDir,
    dados: DiretorioDados,
    config: ConfigClaude,
    exe: PathBuf,
    /// Pasta do projeto `faturamento` (protegido) e do `estoque`.
    a: PathBuf,
    b: PathBuf,
}

impl Cenario {
    fn novo() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let raiz = dunce::canonicalize(tmp.path()).unwrap();
        let home = raiz.join("home");
        fs::create_dir_all(home.join(".claude")).unwrap();
        let exe = raiz
            .join("app")
            .join(if cfg!(windows) { "cba.exe" } else { "cba" });
        let dados = DiretorioDados::em(raiz.join("dados"));
        let a = raiz.join("projetos").join("faturamento");
        let b = raiz.join("projetos").join("estoque");
        let cadastro = Cadastro::em(dados.projetos());
        for (nome, pasta) in [("Faturamento", &a), ("Estoque", &b)] {
            fs::create_dir_all(pasta.join("src")).unwrap();
            fs::write(pasta.join("src").join("a.sql"), "select 1;\n").unwrap();
            cadastro
                .cadastrar(&cba_index::Projeto::novo(nome, [pasta]).unwrap())
                .unwrap();
        }
        Self {
            config: ConfigClaude::da_pasta_do_usuario(&home),
            _tmp: tmp,
            dados,
            exe,
            a: dunce::canonicalize(a).unwrap(),
            b: dunce::canonicalize(b).unwrap(),
        }
    }

    fn preparador(&self) -> Preparador {
        Preparador::novo(self.config.clone(), self.exe.clone(), Plataforma::atual()).unwrap()
    }

    fn regra_a(&self) -> String {
        regra_de_protecao(&self.a, Plataforma::atual())
    }

    fn comando(&self, evento: &str) -> String {
        let exe = self.exe.to_string_lossy();
        let exe = if cfg!(windows) {
            exe.replace('\\', "/")
        } else {
            exe.into_owned()
        };
        format!("\"{exe}\" hook {evento}")
    }

    /// Troca os marcadores (strings JSON inteiras) pelos valores do cenário.
    fn instanciar(&self, texto: &str) -> String {
        let s = |v: &str| serde_json::to_string(v).unwrap();
        let chave = |p: &Path| crate::preparo::chave_do_caminho(p, Plataforma::atual());
        let mut t = texto
            .replace("\"@EXE@\"", &s(&self.exe.to_string_lossy()))
            .replace("\"@CHAVE_A@\"", &s(&chave(&self.a)))
            .replace("\"@CHAVE_B@\"", &s(&chave(&self.b)))
            .replace("\"@REGRA_A@\"", &s(&self.regra_a()));
        for e in EVENTOS {
            t = t.replace(&format!("\"@HOOK:{e}@\""), &s(&self.comando(e)));
        }
        assert!(!t.contains('@'), "marcador sem troca");
        t
    }

    /// Copia uma instalação antiga para o cenário.
    fn instalar(&self, fixture: &[(&str, &str)]) {
        for (nome, texto) in fixture {
            let destino = match *nome {
                "settings.json" => self.config.settings.clone(),
                "claude.json" => self.config.estado.clone(),
                "settings.json.cba-anterior" => self.config.registro(),
                "protecao-faturamento.json" => self
                    .dados
                    .raiz()
                    .join(protecao::PASTA)
                    .join("faturamento.json"),
                outro => panic!("fixture desconhecida: {outro}"),
            };
            fs::create_dir_all(destino.parent().unwrap()).unwrap();
            fs::write(destino, self.instanciar(texto)).unwrap();
        }
    }

    fn json(caminho: &Path) -> Value {
        serde_json::from_slice(&fs::read(caminho).unwrap()).unwrap()
    }

    fn settings(&self) -> Value {
        Self::json(&self.config.settings)
    }

    fn projeto(&self, id: &str) -> cba_index::Projeto {
        Cadastro::em(self.dados.projetos())
            .obter(id)
            .unwrap()
            .unwrap()
    }
}

/// Nenhuma menção ao executável do app no `settings.json` do usuário.
fn sem_nada_do_app(c: &Cenario) {
    let texto = fs::read_to_string(&c.config.settings).unwrap();
    let exe = serde_json::to_string(&c.exe.to_string_lossy()).unwrap();
    let exe = exe.trim_matches('"');
    assert!(!texto.contains(exe), "sobrou entrada do app: {texto}");
    assert!(!texto.contains(&c.regra_a()), "sobrou a regra da proteção");
}

/// O que o analista tinha (fora do app) continua no `settings.json`.
fn analista_intacto(c: &Cenario) {
    let s = c.settings();
    assert_eq!(s["theme"], "dark");
    assert_eq!(s["permissions"]["allow"], json!(["Bash(ls:*)"]));
    assert_eq!(s["permissions"]["deny"], json!(["Read(./.env)"]));
    assert_eq!(
        s["hooks"]["PreToolUse"],
        json!([{"matcher":"Bash","hooks":[{"type":"command","command":"/usr/local/bin/meu-log"}]}])
    );
    assert_eq!(
        s["hooks"]["Stop"],
        json!([{"hooks":[{"type":"command","command":"say pronto"}]}])
    );
    for e in [
        "SessionStart",
        "UserPromptSubmit",
        "Notification",
        "SessionEnd",
    ] {
        assert!(s["hooks"].get(e).is_none(), "{e} devia ter saído");
    }
}

fn pastas_prontas(c: &Cenario) {
    let p = c.preparador();
    for id in ["faturamento", "estoque"] {
        let alvo = Alvo::do_projeto(&c.projeto(id)).unwrap();
        let v = p.verificar(&alvo).unwrap();
        assert!(v.pronto, "{id} não ficou pronto: {v:?}");
    }
    for pasta in [&c.a, &c.b] {
        let local = Cenario::json(&arquivo_local(pasta));
        for e in EVENTOS {
            assert_eq!(
                local["hooks"][e][0]["hooks"][0]["command"],
                json!(c.comando(e)),
                "{e} em {}",
                pasta.display()
            );
        }
    }
}

fn protecao_mantida(c: &Cenario) {
    let s = protecao::situacao(&c.dados, &c.preparador(), &c.projeto("faturamento")).unwrap();
    assert!(s.ligada && s.completa, "{s:?}");
    assert!(s.regras.iter().all(|r| r.do_app));
    assert_eq!(s.regras[0].arquivo, arquivo_local(&c.a));
    let reg = protecao::registro(&c.dados, "faturamento").unwrap();
    assert!(reg.adicionadas.iter().all(|r| r.arquivo.is_some()));
    // A outra pasta não ganha regra.
    let local_b = Cenario::json(&arquivo_local(&c.b));
    assert!(local_b.get("permissions").is_none());
}

#[test]
fn instalacao_da_0_1_0_migra_para_as_pastas() {
    let c = Cenario::novo();
    c.instalar(V010);

    let aviso = migrar(&c.dados, &c.preparador()).unwrap().expect("aviso");
    assert_eq!(aviso.mensagem, MENSAGEM);
    assert_eq!(aviso.projetos.len(), 2);
    assert_eq!(aviso.eventos_retirados.len(), EVENTOS.len());
    assert_eq!(aviso.regras_retiradas, vec![c.regra_a()]);
    assert!(aviso.ficaram.is_empty());
    assert!(aviso.pendentes.is_empty());
    assert!(aviso.arquivos_locais.contains(&arquivo_local(&c.a)));
    assert!(aviso.arquivos_locais.contains(&arquivo_local(&c.b)));

    sem_nada_do_app(&c);
    analista_intacto(&c);
    pastas_prontas(&c);
    protecao_mantida(&c);
    // Os outros campos do `.claude.json` ficam.
    assert_eq!(Cenario::json(&c.config.estado)["numStartups"], 42);
}

#[test]
fn instalacao_com_registro_v1_devolve_o_valor_de_antes() {
    let c = Cenario::novo();
    c.instalar(V050);

    let aviso = migrar(&c.dados, &c.preparador()).unwrap().expect("aviso");
    assert_eq!(aviso.eventos_retirados.len(), EVENTOS.len());
    sem_nada_do_app(&c);
    analista_intacto(&c);
    pastas_prontas(&c);
    protecao_mantida(&c);
    // As trocas globais saíram do registro; as das pastas ficaram.
    let reg = crate::preparo::registro::Registro::ler(&c.config.registro());
    let inst = &reg.instalacoes[&c.exe.to_string_lossy().into_owned()];
    assert!(inst.hooks.is_empty());
    assert!(inst.hooks_existia.is_none());
    assert!(!inst.locais.is_empty());
}

#[test]
fn aviso_aparece_uma_vez_e_migrar_de_novo_nao_muda_nada() {
    let c = Cenario::novo();
    c.instalar(V010);
    let p = c.preparador();
    let primeiro = migrar(&c.dados, &p).unwrap();
    assert!(primeiro.is_some());
    let settings = fs::read(&c.config.settings).unwrap();
    let local = fs::read(arquivo_local(&c.a)).unwrap();

    // Ainda não visto: volta o mesmo aviso, sem mexer em nada.
    assert_eq!(migrar(&c.dados, &p).unwrap(), primeiro);
    marcar_visto(&c.dados).unwrap();
    assert_eq!(migrar(&c.dados, &p).unwrap(), None);
    assert_eq!(fs::read(&c.config.settings).unwrap(), settings);
    assert_eq!(fs::read(arquivo_local(&c.a)).unwrap(), local);
}

#[test]
fn instalacao_nova_nao_tem_aviso() {
    let c = Cenario::novo();
    // Preparo já no formato novo (#304).
    let p = c.preparador();
    for id in ["faturamento", "estoque"] {
        p.preparar(&Alvo::do_projeto(&c.projeto(id)).unwrap())
            .unwrap();
    }
    protecao::ligar(&c.dados, &p, &c.projeto("faturamento")).unwrap();
    let antes = fs::read(arquivo_local(&c.a)).unwrap();

    assert_eq!(migrar(&c.dados, &p).unwrap(), None);
    assert!(concluida(&c.dados));
    assert!(!c.config.settings.exists());
    assert_eq!(fs::read(arquivo_local(&c.a)).unwrap(), antes);
}

#[test]
fn migracao_interrompida_termina_na_abertura_seguinte() {
    let c = Cenario::novo();
    c.instalar(V010);
    let p = c.preparador();

    // Interrompida depois de preparar as pastas e levar a proteção, antes de tirar os
    // hooks globais: o estado da migração não foi gravado.
    for id in ["faturamento", "estoque"] {
        p.preparar(&Alvo::do_projeto(&c.projeto(id)).unwrap())
            .unwrap();
    }
    protecao::migrar(&c.dados, &p, &c.projeto("faturamento")).unwrap();
    assert!(!concluida(&c.dados));
    assert_eq!(p.hooks_globais().unwrap().len(), EVENTOS.len());

    let aviso = migrar(&c.dados, &p).unwrap().expect("aviso");
    assert_eq!(aviso.eventos_retirados.len(), EVENTOS.len());
    // A proteção já tinha ido; não há regra repetida em lugar nenhum.
    assert!(aviso.regras_retiradas.is_empty());
    sem_nada_do_app(&c);
    analista_intacto(&c);
    pastas_prontas(&c);
    protecao_mantida(&c);
    let local_a = Cenario::json(&arquivo_local(&c.a));
    assert_eq!(local_a["permissions"]["deny"], json!([c.regra_a()]));
    for e in EVENTOS {
        assert_eq!(local_a["hooks"][e].as_array().unwrap().len(), 1, "{e}");
    }
}

#[test]
fn falha_no_preparo_nao_tira_nada_do_global() {
    let c = Cenario::novo();
    c.instalar(V010);
    // O arquivo local de uma pasta é ilegível: o preparo dela falha.
    let local = arquivo_local(&c.b);
    fs::create_dir_all(local.parent().unwrap()).unwrap();
    fs::write(&local, "{ isto não é json").unwrap();
    let p = c.preparador();
    let settings = fs::read(&c.config.settings).unwrap();

    assert!(migrar(&c.dados, &p).is_err());
    assert!(!concluida(&c.dados));
    assert_eq!(fs::read(&c.config.settings).unwrap(), settings);

    // O analista conserta o arquivo: a próxima abertura termina.
    fs::write(&local, "{}\n").unwrap();
    assert!(migrar(&c.dados, &p).unwrap().is_some());
    sem_nada_do_app(&c);
    pastas_prontas(&c);
}

#[test]
fn hook_do_app_mudado_pelo_analista_fica_e_e_dito() {
    let c = Cenario::novo();
    c.instalar(V010);
    let mut s = c.settings();
    s["hooks"]["Stop"][1]["hooks"][0]["timeout"] = json!(30);
    fs::write(&c.config.settings, serde_json::to_vec_pretty(&s).unwrap()).unwrap();

    let aviso = migrar(&c.dados, &c.preparador()).unwrap().expect("aviso");
    assert_eq!(aviso.ficaram, vec!["Stop".to_string()]);
    assert!(!aviso.eventos_retirados.contains(&"Stop".to_string()));
    let s = c.settings();
    assert_eq!(s["hooks"]["Stop"].as_array().unwrap().len(), 2);
    assert_eq!(s["hooks"]["Stop"][1]["hooks"][0]["timeout"], 30);
    assert!(s["hooks"].get("SessionStart").is_none());
}

#[test]
fn hooks_de_outra_instalacao_nao_sao_tocados() {
    let c = Cenario::novo();
    c.instalar(V010);
    let outro = format!(
        "\"{}\" hook SessionStart",
        c.a.parent()
            .unwrap()
            .join("dev")
            .join("cba")
            .to_string_lossy()
            .replace('\\', "/")
    );
    let mut s = c.settings();
    s["hooks"]["SessionStart"]
        .as_array_mut()
        .unwrap()
        .push(json!({"hooks":[{"type":"command","command":outro,"timeout":10}]}));
    fs::write(&c.config.settings, serde_json::to_vec_pretty(&s).unwrap()).unwrap();

    migrar(&c.dados, &c.preparador()).unwrap();
    let s = c.settings();
    assert_eq!(
        s["hooks"]["SessionStart"],
        json!([{"hooks":[{"type":"command","command":outro,"timeout":10}]}])
    );
}

#[test]
fn pasta_inacessivel_fica_pendente_e_nao_impede() {
    let c = Cenario::novo();
    c.instalar(V010);
    fs::remove_dir_all(&c.b).unwrap();

    let aviso = migrar(&c.dados, &c.preparador()).unwrap().expect("aviso");
    assert_eq!(
        aviso.pendentes,
        vec![Pendente {
            projeto: "estoque".into(),
            motivo: "pasta do projeto inacessível".into(),
        }]
    );
    sem_nada_do_app(&c);
    protecao_mantida_sem_b(&c);
}

fn protecao_mantida_sem_b(c: &Cenario) {
    let local_a = Cenario::json(&arquivo_local(&c.a));
    assert_eq!(local_a["permissions"]["deny"], json!([c.regra_a()]));
}

#[test]
fn vale_para_claude_config_dir() {
    // Com `CLAUDE_CONFIG_DIR`, os arquivos ficam na pasta indicada: a migração usa o
    // preparador que `ConfigClaude::localizar` monta.
    let c = Cenario::novo();
    let dir = c.dados.raiz().parent().unwrap().join("config-claude");
    fs::create_dir_all(&dir).unwrap();
    struct ComConfigDir(PathBuf);
    impl crate::clientes::Ambiente for ComConfigDir {
        fn plataforma(&self) -> Plataforma {
            Plataforma::atual()
        }
        fn var(&self, nome: &str) -> Option<std::ffi::OsString> {
            (nome == "CLAUDE_CONFIG_DIR").then(|| self.0.clone().into_os_string())
        }
        fn home(&self) -> Option<PathBuf> {
            None
        }
        fn e_arquivo(&self, _: &Path) -> bool {
            unreachable!()
        }
        fn e_pasta(&self, _: &Path) -> bool {
            unreachable!()
        }
        fn nomes_na_pasta(&self, _: &Path) -> Vec<std::ffi::OsString> {
            unreachable!()
        }
        fn executar(
            &self,
            _: &Path,
            _: &[&str],
            _: std::time::Duration,
        ) -> Result<crate::clientes::Saida, crate::clientes::ErroExecucao> {
            unreachable!()
        }
    }
    let p = Preparador::do_ambiente(&ComConfigDir(dir.clone()), c.exe.clone()).unwrap();
    assert_eq!(p.config.settings, dir.join("settings.json"));
    let c = Cenario {
        config: p.config.clone(),
        ..c
    };
    c.instalar(V010);
    assert!(migrar(&c.dados, &p).unwrap().is_some());
    sem_nada_do_app(&c);
    pastas_prontas(&c);
}
