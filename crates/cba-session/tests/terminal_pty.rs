//! Terminal embutido com PTY de verdade e um `claude` falso.
//!
//! Os programas falsos são scripts (shell no macOS e no Linux, `.cmd` no Windows, e o
//! PowerShell para ler o tamanho do console). O CI não precisa do Claude nem de login.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use cba_session::clientes::{
    Ambiente, AmbienteReal, ErroExecucao, Plataforma, Saida, orientacao::Passo,
};
use cba_session::preparo::{ConfigClaude, Preparador};
use cba_session::terminal::{
    Comando, ErroAbertura, Evento, Fim, PedidoClaude, Retomada, Sessoes, Tamanho, Terminal,
    lancamento,
};

const LIMITE: Duration = Duration::from_secs(20);

/// Recebe os eventos do terminal e responde à consulta de posição do cursor
/// (`ESC[6n`) que o ConPTY faz ao abrir, como o xterm.js faria na interface.
struct Coletor {
    rx: Receiver<Evento>,
    texto: String,
    respondidas: usize,
    fim: Option<Fim>,
}

impl Coletor {
    fn novo() -> (Self, impl Fn(Evento) + Send + Sync + 'static) {
        let (tx, rx) = mpsc::channel();
        let tx = std::sync::Mutex::new(tx);
        (
            Self {
                rx,
                texto: String::new(),
                respondidas: 0,
                fim: None,
            },
            move |e| {
                let _ = tx.lock().unwrap().send(e);
            },
        )
    }

    fn receber(&mut self, escrever: &dyn Fn(&[u8]), limite: Duration) -> bool {
        match self.rx.recv_timeout(limite) {
            Ok(Evento::Saida { texto }) => {
                self.texto.push_str(&texto);
                while self.texto.matches("\x1b[6n").count() > self.respondidas {
                    escrever(b"\x1b[1;1R");
                    self.respondidas += 1;
                }
                true
            }
            Ok(Evento::Encerrado { fim }) => {
                self.fim = Some(fim);
                true
            }
            Err(_) => false,
        }
    }

    /// Espera `trecho` na saída.
    fn esperar_texto(&mut self, escrever: &dyn Fn(&[u8]), trecho: &str) {
        let prazo = Instant::now() + LIMITE;
        while !self.texto.contains(trecho) {
            // O fim é o último evento: depois dele não chega mais saída.
            assert!(
                self.fim.is_none() && Instant::now() < prazo,
                "não apareceu {trecho:?}; saída: {:?}; fim: {:?}",
                self.texto,
                self.fim
            );
            self.receber(escrever, Duration::from_millis(200));
        }
    }

    /// Espera o evento de fim (o último).
    fn esperar_fim(&mut self, escrever: &dyn Fn(&[u8])) -> Fim {
        let prazo = Instant::now() + LIMITE;
        while self.fim.is_none() {
            assert!(
                Instant::now() < prazo,
                "o terminal não terminou; saída: {:?}",
                self.texto
            );
            self.receber(escrever, Duration::from_millis(200));
        }
        // Nada chega depois do fim.
        assert!(self.rx.recv_timeout(Duration::from_millis(100)).is_err());
        self.fim.clone().unwrap()
    }
}

/// Grava um script executável: `sh` no macOS e no Linux, `.cmd` no Windows.
fn script(pasta: &Path, nome: &str, sh: &str, cmd: &str) -> PathBuf {
    #[cfg(windows)]
    {
        let _ = sh;
        let caminho = pasta.join(format!("{nome}.cmd"));
        std::fs::write(
            &caminho,
            format!("@echo off\r\n{}\r\n", cmd.replace('\n', "\r\n")),
        )
        .unwrap();
        caminho
    }
    #[cfg(not(windows))]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = cmd;
        let caminho = pasta.join(nome);
        std::fs::write(&caminho, format!("#!/bin/sh\n{sh}\n")).unwrap();
        std::fs::set_permissions(&caminho, std::fs::Permissions::from_mode(0o755)).unwrap();
        caminho
    }
}

fn comando(programa: PathBuf, args: &[&str], diretorio: &Path) -> Comando {
    Comando {
        programa,
        args: args.iter().map(OsString::from).collect(),
        diretorio: diretorio.to_path_buf(),
        env: Vec::new(),
        env_remover: Vec::new(),
    }
}

fn abrir(comando: Comando, tamanho: Tamanho) -> (Terminal, Coletor) {
    let (coletor, ao_evento) = Coletor::novo();
    let terminal = Terminal::abrir(comando, tamanho, ao_evento).unwrap();
    (terminal, coletor)
}

#[test]
fn eco_da_entrada_e_codigo_de_saida() {
    let tmp = tempfile::tempdir().unwrap();
    let programa = script(
        tmp.path(),
        "eco",
        "printf 'pronto\\n'; read linha; echo \"eco:$linha\"; exit 7",
        "echo pronto\nset /p linha=\necho eco:%linha%\nexit 7",
    );
    let (t, mut c) = abrir(comando(programa, &[], tmp.path()), Tamanho::default());
    assert!(t.pid().is_some());
    let escrever = |b: &[u8]| t.escrever(b).unwrap();
    c.esperar_texto(&escrever, "pronto");
    // O `cmd.exe` lê o que é digitado na página de código do console; no Windows o
    // teste fica em ASCII.
    let linha = if cfg!(windows) {
        "ola mundo"
    } else {
        "olá mundo"
    };
    t.escrever(format!("{linha}\r").as_bytes()).unwrap();
    c.esperar_texto(
        &|b| {
            let _ = t.escrever(b);
        },
        &format!("eco:{linha}"),
    );
    let fim = c.esperar_fim(&|b| {
        let _ = t.escrever(b);
    });
    assert_eq!(fim.codigo, 7);
    assert_eq!(t.fim(), Some(fim));
    assert!(!t.ativo());
    // Depois do fim, escrever e redimensionar falham sem pânico.
    assert!(t.escrever(b"x").is_err());
    assert!(t.redimensionar(Tamanho::default()).is_err());
}

#[test]
fn redimensionar_chega_ao_processo() {
    let tmp = tempfile::tempdir().unwrap();
    #[cfg(windows)]
    let cmd = comando(
        PathBuf::from("powershell.exe"),
        &[
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Write-Output pronto; [void][Console]::ReadLine(); Write-Output ('tam:{0}x{1}' -f [Console]::WindowHeight, [Console]::WindowWidth)",
        ],
        tmp.path(),
    );
    #[cfg(not(windows))]
    let cmd = comando(
        script(
            tmp.path(),
            "tamanho",
            "echo pronto; read linha; echo \"tam:$(stty size | tr ' ' x)\"",
            "",
        ),
        &[],
        tmp.path(),
    );
    let (t, mut c) = abrir(
        cmd,
        Tamanho {
            linhas: 24,
            colunas: 80,
        },
    );
    let responder = |b: &[u8]| {
        let _ = t.escrever(b);
    };
    c.esperar_texto(&responder, "pronto");
    t.redimensionar(Tamanho {
        linhas: 30,
        colunas: 100,
    })
    .unwrap();
    t.escrever(b"\r").unwrap();
    c.esperar_texto(&responder, "tam:30x100");
    assert_eq!(c.esperar_fim(&responder).codigo, 0);
}

#[test]
fn encerrar_termina_o_processo() {
    let tmp = tempfile::tempdir().unwrap();
    let programa = script(
        tmp.path(),
        "lento",
        "echo pronto; sleep 60",
        "echo pronto\nping -n 60 127.0.0.1 >nul",
    );
    let (t, mut c) = abrir(comando(programa, &[], tmp.path()), Tamanho::default());
    let responder = |b: &[u8]| {
        let _ = t.escrever(b);
    };
    c.esperar_texto(&responder, "pronto");
    let inicio = Instant::now();
    t.encerrar().unwrap();
    let fim = c.esperar_fim(&responder);
    assert!(inicio.elapsed() < Duration::from_secs(10));
    assert_ne!((fim.codigo, fim.sinal.is_some()), (0, false), "{fim:?}");
    // Encerrar de novo não falha.
    t.encerrar().unwrap();
}

#[test]
fn programa_inexistente_nao_abre() {
    let tmp = tempfile::tempdir().unwrap();
    let r = Terminal::abrir(
        comando(tmp.path().join("nao-existe"), &[], tmp.path()),
        Tamanho::default(),
        |_| {},
    );
    assert!(r.is_err());
}

/// Ambiente de verdade com `PATH` e pasta do usuário controlados pelo teste.
struct Isolado {
    path: OsString,
    home: PathBuf,
}

impl Ambiente for Isolado {
    fn plataforma(&self) -> Plataforma {
        AmbienteReal.plataforma()
    }
    fn var(&self, nome: &str) -> Option<OsString> {
        match nome {
            "PATH" => Some(self.path.clone()),
            "PATHEXT" => Some(".COM;.EXE;.BAT;.CMD".into()),
            _ => None,
        }
    }
    fn home(&self) -> Option<PathBuf> {
        Some(self.home.clone())
    }
    fn e_arquivo(&self, caminho: &Path) -> bool {
        AmbienteReal.e_arquivo(caminho)
    }
    fn e_pasta(&self, caminho: &Path) -> bool {
        AmbienteReal.e_pasta(caminho)
    }
    fn nomes_na_pasta(&self, pasta: &Path) -> Vec<OsString> {
        AmbienteReal.nomes_na_pasta(pasta)
    }
    fn executar(
        &self,
        programa: &Path,
        args: &[&str],
        limite: Duration,
    ) -> Result<Saida, ErroExecucao> {
        AmbienteReal.executar(programa, args, limite)
    }
}

/// Um projeto cadastrado numa pasta temporária, o preparador com a configuração do
/// Claude noutra e um `claude` falso numa terceira.
struct Cenario {
    _tmp: tempfile::TempDir,
    projeto: cba_index::Projeto,
    preparador: Preparador,
    ambiente: Isolado,
    bin: PathBuf,
}

fn cenario() -> Cenario {
    let tmp = tempfile::tempdir().unwrap();
    let raiz = tmp.path().join("faturamento");
    let home = tmp.path().join("home");
    let bin = tmp.path().join("bin");
    for p in [&raiz, &home, &bin] {
        std::fs::create_dir_all(p).unwrap();
    }
    let projeto = cba_index::Projeto::novo("Faturamento Exemplo", [&raiz]).unwrap();
    let exe = tmp
        .path()
        .join(if cfg!(windows) { "cba.exe" } else { "cba" });
    let preparador = Preparador::novo(
        ConfigClaude::da_pasta_do_usuario(&home),
        exe,
        Plataforma::atual(),
    )
    .unwrap();
    let ambiente = Isolado {
        path: std::env::join_paths([&bin]).unwrap(),
        home,
    };
    Cenario {
        _tmp: tmp,
        projeto,
        preparador,
        ambiente,
        bin,
    }
}

/// `claude` falso: mostra os argumentos, a pasta e o começo do `PATH`, lê uma linha e
/// a repete.
fn claude_falso(bin: &Path) -> PathBuf {
    script(
        bin,
        "claude",
        "echo \"args:$*\"; echo \"pasta:$(pwd -P)\"; echo \"path:${PATH%%:*}\"; read linha; echo \"recebido:$linha\"",
        "echo args:%*\necho pasta:%CD%\necho path:%PATH%\nset /p linha=\necho recebido:%linha%",
    )
}

#[test]
fn recusa_sem_ambiente_preparado_e_sem_claude() {
    let c = cenario();
    let pedido = PedidoClaude::default();
    match lancamento(&c.projeto, &pedido, &c.preparador, &c.ambiente) {
        Err(ErroAbertura::AmbienteNaoPreparado { pendencias }) => {
            assert!(!pendencias.is_empty());
        }
        outro => panic!("esperava ambiente não preparado: {outro:?}"),
    }
    let alvo = cba_session::preparo::Alvo::do_projeto(&c.projeto).unwrap();
    c.preparador.preparar(&alvo).unwrap();
    // Preparado, mas sem o Claude Code instalado.
    match lancamento(&c.projeto, &pedido, &c.preparador, &c.ambiente) {
        Err(ErroAbertura::ClaudeNaoEncontrado { orientacao }) => {
            assert!(orientacao.iter().any(|p: &Passo| p.comando.is_some()));
        }
        outro => panic!("esperava Claude Code não encontrado: {outro:?}"),
    }
}

#[test]
fn claude_falso_na_pasta_do_projeto_com_prompt_inicial_e_prompt_pronto() {
    let c = cenario();
    let alvo = cba_session::preparo::Alvo::do_projeto(&c.projeto).unwrap();
    c.preparador.preparar(&alvo).unwrap();
    let claude = claude_falso(&c.bin);
    let pedido = PedidoClaude {
        prompt_inicial: Some("Inicie a análise pelo ctower".into()),
        retomada: Retomada::Nova,
        argumentos: Vec::new(),
    };
    let l = lancamento(&c.projeto, &pedido, &c.preparador, &c.ambiente).unwrap();
    assert_eq!(l.comando.programa, claude);
    assert_eq!(l.comando.diretorio, alvo.diretorio);
    let id = l.id_sessao.clone().unwrap();
    assert_eq!(id.len(), 36, "{id}");

    let sessoes = Sessoes::new();
    let (mut coletor, ao_evento) = Coletor::novo();
    let n = sessoes
        .abrir(l.comando, Tamanho::default(), ao_evento)
        .unwrap();
    assert_eq!(sessoes.ativas(), vec![n]);
    let responder = |b: &[u8]| {
        let _ = sessoes.escrever(n, b);
    };
    coletor.esperar_texto(&responder, &format!("--session-id {id}"));
    coletor.esperar_texto(&responder, "Inicie a análise pelo ctower");
    coletor.esperar_texto(&responder, &format!("pasta:{}", alvo.diretorio.display()));
    let bin = c.bin.to_string_lossy().into_owned();
    let separador = if cfg!(windows) { ";" } else { "" };
    coletor.esperar_texto(&responder, &format!("path:{bin}{separador}"));
    sessoes
        .enviar_prompt(n, "Resuma para o solicitante")
        .unwrap();
    coletor.esperar_texto(&responder, "recebido:Resuma para o solicitante");
    assert_eq!(coletor.esperar_fim(&responder).codigo, 0);
    assert!(sessoes.ativas().is_empty());
    assert!(sessoes.escrever(n, b"x").is_err());
}
