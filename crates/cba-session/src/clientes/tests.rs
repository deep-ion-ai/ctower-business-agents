//! Testes da detecção com um ambiente falso: nada aqui precisa do Claude instalado.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use super::*;

/// Ambiente falso que registra cada caminho consultado e cada comando executado.
struct Falso {
    plataforma: Plataforma,
    vars: HashMap<String, String>,
    home: Option<PathBuf>,
    arquivos: HashSet<PathBuf>,
    pastas: HashSet<PathBuf>,
    /// Resposta por argumentos (`"--version"`, `"auth status --json"`).
    respostas: HashMap<String, Result<Saida, ErroExecucao>>,
    consultados: RefCell<Vec<PathBuf>>,
    executados: RefCell<Vec<(PathBuf, String)>>,
}

impl Falso {
    fn new(plataforma: Plataforma) -> Self {
        let home = match plataforma {
            Plataforma::Windows => "C:/Users/analista",
            Plataforma::MacOs => "/Users/analista",
            Plataforma::Linux => "/home/analista",
        };
        Self {
            plataforma,
            vars: HashMap::new(),
            home: Some(PathBuf::from(home)),
            arquivos: HashSet::new(),
            pastas: HashSet::new(),
            respostas: HashMap::new(),
            consultados: RefCell::default(),
            executados: RefCell::default(),
        }
    }

    fn var(mut self, nome: &str, valor: &str) -> Self {
        self.vars.insert(nome.into(), valor.into());
        self
    }

    fn arquivo(mut self, caminho: &str) -> Self {
        self.arquivos.insert(caminho.into());
        self
    }

    fn pasta(mut self, caminho: &str) -> Self {
        self.pastas.insert(caminho.into());
        self
    }

    fn responde(mut self, args: &str, codigo: i32, stdout: &str) -> Self {
        self.respostas.insert(
            args.into(),
            Ok(Saida {
                codigo: Some(codigo),
                stdout: stdout.into(),
                stderr: String::new(),
            }),
        );
        self
    }

    fn falha(mut self, args: &str, erro: ErroExecucao) -> Self {
        self.respostas.insert(args.into(), Err(erro));
        self
    }

    /// Claude Code no PATH, versão 2.1.0, com a resposta de status dada.
    fn com_code(self, status: &str) -> Self {
        self.var("PATH", "/usr/bin:/opt/claude/bin")
            .arquivo("/opt/claude/bin/claude")
            .responde("--version", 0, "2.1.0 (Claude Code)\n")
            .responde("auth status --json", 0, status)
    }
}

impl Ambiente for Falso {
    fn plataforma(&self) -> Plataforma {
        self.plataforma
    }
    fn var(&self, nome: &str) -> Option<OsString> {
        self.vars.get(nome).map(OsString::from)
    }
    fn home(&self) -> Option<PathBuf> {
        self.home.clone()
    }
    fn e_arquivo(&self, caminho: &Path) -> bool {
        self.consultados.borrow_mut().push(caminho.into());
        self.arquivos.contains(caminho)
    }
    fn e_pasta(&self, caminho: &Path) -> bool {
        self.consultados.borrow_mut().push(caminho.into());
        self.pastas.contains(caminho)
    }
    fn nomes_na_pasta(&self, pasta: &Path) -> Vec<OsString> {
        self.consultados.borrow_mut().push(pasta.into());
        self.pastas
            .iter()
            .filter(|p| p.parent() == Some(pasta))
            .filter_map(|p| p.file_name().map(OsString::from))
            .collect()
    }
    fn executar(
        &self,
        programa: &Path,
        args: &[&str],
        _limite: Duration,
    ) -> Result<Saida, ErroExecucao> {
        let args = args.join(" ");
        self.executados
            .borrow_mut()
            .push((programa.into(), args.clone()));
        self.respostas
            .get(&args)
            .cloned()
            .unwrap_or(Err(ErroExecucao::NaoIniciou("NotFound".into())))
    }
}

const LOGADO: &str = r#"{"loggedIn":true,"authMethod":"claude.ai","apiProvider":"firstParty",
    "email":"analista@exemplo.invalid","orgName":"Faturamento Exemplo","subscriptionType":"pro"}"#;
const DESLOGADO: &str = r#"{"loggedIn":false,"authMethod":"none","apiProvider":"firstParty"}"#;

#[test]
fn code_no_path_logado() {
    let amb = Falso::new(Plataforma::Linux).com_code(LOGADO);
    let estado = detectar_claude_code(&amb);
    assert_eq!(
        estado.instalacao,
        InstalacaoClaudeCode::Encontrado {
            caminho: "/opt/claude/bin/claude".into(),
            origem: OrigemCaminho::Path,
            versao: Some("2.1.0".into()),
        }
    );
    assert_eq!(
        estado.login,
        LoginClaudeCode::Logado {
            metodo: Some("claude.ai".into()),
            provedor: Some("firstParty".into()),
        }
    );
    assert!(estado.orientacao.is_empty());
}

#[test]
fn estado_logado_nao_carrega_dados_da_conta() {
    let amb = Falso::new(Plataforma::Linux).com_code(LOGADO);
    let json = serde_json::to_string(&detectar_em(&amb)).unwrap();
    assert!(!json.contains("exemplo.invalid"), "{json}");
    assert!(!json.contains("Faturamento"), "{json}");
    assert!(!json.contains("pro\""), "{json}");
}

#[test]
fn code_sem_login() {
    // O CLI sai com código 1 quando não há login, mas a resposta JSON vale.
    let amb =
        Falso::new(Plataforma::Linux)
            .com_code("")
            .responde("auth status --json", 1, DESLOGADO);
    let estado = detectar_claude_code(&amb);
    assert_eq!(estado.login, LoginClaudeCode::NaoLogado);
    assert_eq!(estado.orientacao, orientacao::claude_code_login());
}

#[test]
fn code_antigo_sem_comando_de_status() {
    let amb = Falso::new(Plataforma::Linux)
        .com_code("")
        .responde("auth status --json", 1, "");
    let estado = detectar_claude_code(&amb);
    assert!(matches!(
        estado.login,
        LoginClaudeCode::Indeterminado { .. }
    ));
    assert!(
        estado
            .orientacao
            .iter()
            .any(|p| p.comando.as_deref() == Some("claude update"))
    );
}

#[test]
fn status_que_nao_responde_fica_indeterminado() {
    let amb = Falso::new(Plataforma::Linux)
        .com_code("")
        .falha("auth status --json", ErroExecucao::TempoEsgotado);
    let estado = detectar_claude_code(&amb);
    assert!(matches!(
        estado.login,
        LoginClaudeCode::Indeterminado { .. }
    ));
}

#[test]
fn versao_que_falha_nao_impede_a_deteccao() {
    let amb = Falso::new(Plataforma::Linux)
        .com_code(LOGADO)
        .responde("--version", 1, "");
    let estado = detectar_claude_code(&amb);
    assert!(matches!(
        estado.instalacao,
        InstalacaoClaudeCode::Encontrado { versao: None, .. }
    ));
    assert!(matches!(estado.login, LoginClaudeCode::Logado { .. }));
}

#[test]
fn code_nao_instalado() {
    let amb = Falso::new(Plataforma::MacOs).var("PATH", "/usr/bin:/bin");
    let estado = detectar_claude_code(&amb);
    assert_eq!(estado.instalacao, InstalacaoClaudeCode::NaoEncontrado);
    assert_eq!(estado.login, LoginClaudeCode::NaoVerificado);
    assert_eq!(
        estado.orientacao,
        orientacao::claude_code_instalar(Plataforma::MacOs)
    );
    assert!(
        estado.orientacao[0].comando.as_deref() == Some(orientacao::INSTALAR_CODE_SH),
        "{:?}",
        estado.orientacao
    );
    assert!(amb.executados.borrow().is_empty());
}

#[test]
fn code_fora_do_path_em_local_padrao() {
    // App aberto pelo Finder: o PATH não tem ~/.local/bin.
    let amb = Falso::new(Plataforma::MacOs)
        .var("PATH", "/usr/bin:/bin")
        .arquivo("/Users/analista/.local/bin/claude")
        .responde("--version", 0, "2.1.0 (Claude Code)")
        .responde("auth status --json", 0, LOGADO);
    let estado = detectar_claude_code(&amb);
    assert_eq!(
        estado.instalacao,
        InstalacaoClaudeCode::Encontrado {
            caminho: "/Users/analista/.local/bin/claude".into(),
            origem: OrigemCaminho::LocalPadrao,
            versao: Some("2.1.0".into()),
        }
    );
}

#[test]
fn code_no_windows_usa_pathext_e_separador() {
    let amb = Falso::new(Plataforma::Windows)
        .var(
            "PATH",
            r#"C:/Windows;"C:/Users/analista/AppData/Roaming/npm""#,
        )
        .var("PATHEXT", ".COM;.EXE;.BAT;.CMD;.VBS")
        .arquivo("C:/Users/analista/AppData/Roaming/npm/claude.cmd")
        .responde("--version", 0, "2.1.0 (Claude Code)")
        .responde("auth status --json", 0, LOGADO);
    let (caminho, origem) = localizar_claude(&amb).unwrap();
    assert_eq!(
        caminho,
        PathBuf::from("C:/Users/analista/AppData/Roaming/npm/claude.cmd")
    );
    assert_eq!(origem, OrigemCaminho::Path);
}

#[test]
fn code_no_windows_em_local_padrao() {
    let amb = Falso::new(Plataforma::Windows)
        .var("PATH", "C:/Windows")
        .arquivo("C:/Users/analista/.local/bin/claude.exe");
    let (caminho, origem) = localizar_claude(&amb).unwrap();
    assert_eq!(
        caminho,
        PathBuf::from("C:/Users/analista/.local/bin/claude.exe")
    );
    assert_eq!(origem, OrigemCaminho::LocalPadrao);
    assert_eq!(
        orientacao::claude_code_instalar(Plataforma::Windows)[0]
            .comando
            .as_deref(),
        Some(orientacao::INSTALAR_CODE_PS1)
    );
}

#[test]
fn desktop_no_macos() {
    let sem = Falso::new(Plataforma::MacOs);
    let estado = detectar_claude_desktop(&sem);
    assert_eq!(estado.instalacao, InstalacaoClaudeDesktop::NaoEncontrado);
    assert_eq!(estado.orientacao, orientacao::claude_desktop_instalar());

    let com = Falso::new(Plataforma::MacOs).pasta("/Applications/Claude.app");
    assert_eq!(
        detectar_claude_desktop(&com).instalacao,
        InstalacaoClaudeDesktop::Encontrado {
            caminho: "/Applications/Claude.app".into()
        }
    );

    let do_usuario = Falso::new(Plataforma::MacOs).pasta("/Users/analista/Applications/Claude.app");
    let estado = detectar_claude_desktop(&do_usuario);
    assert!(matches!(
        estado.instalacao,
        InstalacaoClaudeDesktop::Encontrado { .. }
    ));
    assert_eq!(estado.orientacao, orientacao::claude_desktop_login());
}

#[test]
fn desktop_no_windows() {
    let local = "C:/Users/analista/AppData/Local";
    let sem = Falso::new(Plataforma::Windows)
        .var("LOCALAPPDATA", local)
        .pasta(
            "C:/Users/analista/AppData/Local/Packages/Microsoft.WindowsCalculator_8wekyb3d8bbwe",
        );
    assert_eq!(
        detectar_claude_desktop(&sem).instalacao,
        InstalacaoClaudeDesktop::NaoEncontrado
    );

    let squirrel = Falso::new(Plataforma::Windows)
        .var("LOCALAPPDATA", local)
        .pasta("C:/Users/analista/AppData/Local/AnthropicClaude");
    assert_eq!(
        detectar_claude_desktop(&squirrel).instalacao,
        InstalacaoClaudeDesktop::Encontrado {
            caminho: "C:/Users/analista/AppData/Local/AnthropicClaude".into()
        }
    );

    let msix = Falso::new(Plataforma::Windows)
        .var("LOCALAPPDATA", local)
        .pasta("C:/Users/analista/AppData/Local/Packages/Claude_pzs8sxrjxfjjc");
    assert_eq!(
        detectar_claude_desktop(&msix).instalacao,
        InstalacaoClaudeDesktop::Encontrado {
            caminho: "C:/Users/analista/AppData/Local/Packages/Claude_pzs8sxrjxfjjc".into()
        }
    );
}

#[test]
fn desktop_nao_existe_no_linux() {
    let amb = Falso::new(Plataforma::Linux).pasta("/Applications/Claude.app");
    let estado = detectar_claude_desktop(&amb);
    assert_eq!(estado.instalacao, InstalacaoClaudeDesktop::NaoSuportado);
    assert!(estado.orientacao.is_empty());
    assert!(amb.consultados.borrow().is_empty());
}

#[test]
fn versao_da_saida_aceita_so_numero_de_versao() {
    assert_eq!(
        versao_da_saida("2.1.0 (Claude Code)\n"),
        Some("2.1.0".into())
    );
    assert_eq!(versao_da_saida("1.0.0-beta.2"), Some("1.0.0-beta.2".into()));
    assert_eq!(versao_da_saida(""), None);
    assert_eq!(versao_da_saida("error: unknown option"), None);
    assert_eq!(versao_da_saida("2.1.0;rm"), None);
}

#[test]
fn serializacao_para_a_interface() {
    let amb = Falso::new(Plataforma::MacOs)
        .com_code(DESLOGADO)
        .pasta("/Applications/Claude.app");
    let json = serde_json::to_value(detectar_em(&amb)).unwrap();
    assert_eq!(json["plataforma"], "macOs");
    assert_eq!(json["claudeCode"]["instalacao"]["estado"], "encontrado");
    assert_eq!(json["claudeCode"]["instalacao"]["origem"], "path");
    assert_eq!(json["claudeCode"]["instalacao"]["versao"], "2.1.0");
    assert_eq!(json["claudeCode"]["login"]["estado"], "naoLogado");
    assert_eq!(json["claudeDesktop"]["instalacao"]["estado"], "encontrado");
    assert!(json["claudeCode"]["orientacao"][0]["texto"].is_string());
}

/// Critério de aceite 7 da spec: a detecção não abre arquivo de credencial nem de
/// configuração do Claude. O [`Ambiente`] não tem operação de leitura; aqui se confere
/// que nem os metadados desses arquivos são consultados e que os únicos comandos
/// executados são os oficiais de versão e de status.
#[test]
fn nenhum_arquivo_de_credencial_e_consultado() {
    for plataforma in [Plataforma::Linux, Plataforma::MacOs, Plataforma::Windows] {
        for status in [LOGADO, DESLOGADO] {
            let amb = Falso::new(plataforma)
                .com_code(status)
                .var("APPDATA", "C:/Users/analista/AppData/Roaming")
                .var("LOCALAPPDATA", "C:/Users/analista/AppData/Local")
                .pasta("C:/Users/analista/AppData/Local/Packages/Claude_pzs8sxrjxfjjc");
            detectar_em(&amb);
            for caminho in amb.consultados.borrow().iter() {
                let texto = caminho.to_string_lossy().to_lowercase();
                for proibido in [
                    "credential",
                    ".claude.json",
                    "settings.json",
                    "claude_desktop_config",
                    "keychain",
                    "application support/claude",
                    "roaming/claude",
                    "localcache",
                ] {
                    assert!(
                        !texto.contains(proibido),
                        "{plataforma:?}: consultou {caminho:?}"
                    );
                }
            }
            for (_, args) in amb.executados.borrow().iter() {
                assert!(
                    args == "--version" || args == "auth status --json",
                    "comando inesperado: {args}"
                );
            }
        }
    }
}

/// O código da detecção não tem chamadas que abrem arquivos nem nomes de arquivos de
/// credencial (revisão automática, complementar ao teste acima).
#[test]
fn codigo_da_deteccao_nao_abre_arquivos() {
    let fontes = [
        ("mod.rs", include_str!("mod.rs")),
        ("ambiente.rs", include_str!("ambiente.rs")),
        ("orientacao.rs", include_str!("orientacao.rs")),
    ];
    let proibidos = [
        "File::open",
        "File::create",
        "OpenOptions",
        "fs::read(",
        "fs::read_to_string",
        "fs::write",
        "fs::copy",
        ".credentials",
        "claude_desktop_config",
        "find-generic-password",
        "keyring",
        "CredRead",
    ];
    for (nome, fonte) in fontes {
        for proibido in proibidos {
            assert!(!fonte.contains(proibido), "{nome} contém {proibido}");
        }
    }
}
