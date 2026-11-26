//! Resolvedor único de credenciais (#436): lê a [`FonteCredencial`] de uma
//! [`Conexao`] e devolve um [`Secret`].
//!
//! - `Chaveiro` (o padrão) lê a conta no chaveiro do sistema;
//! - `Comando` roda o comando sem TTY, com prazo, e fica com a primeira linha;
//! - `Cli` pergunta à CLI do provider (`gh auth token`), quando o descritor
//!   sabe como;
//! - `Arquivo` lê a primeira linha do arquivo;
//! - `Nenhuma` não tem credencial.
//!
//! O resultado fica em cache em memória durante a vida do processo. Uma
//! resposta 401 ou 403 descarta o cache ([`Resolvedor::descartar`]) e o
//! consumidor tenta mais uma vez ([`com_nova_tentativa`]). O token próprio de um
//! repositório é uma **sobrescrita** da credencial da conexão. Todo segredo
//! resolvido entra no [mascaramento central](super::mascara).
//!
//! O token nunca chega ao agente: as ferramentas pedem ao resolvedor pela
//! conexão do projeto e passam o valor só ao ambiente do processo filho ou ao
//! driver.

use super::{Conexao, FonteCredencial, mascara, todos};
use crate::chaveiro::Chaveiro;
use crate::process::{RunError, cite, run};
use crate::{CREDENTIAL_DEADLINE, CredentialError, Secret, run_credential};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use zeroize::Zeroize;

/// Por que a credencial de uma conexão não pôde ser lida. As mensagens citam a
/// conta, o comando ou o arquivo, nunca o valor.
#[derive(Debug, thiserror::Error)]
pub enum ErroCredencial {
    #[error("{0}")]
    Chaveiro(String),
    #[error("{0}")]
    Comando(#[from] CredentialError),
    #[error("{rotulo}: {detalhe}")]
    Cli {
        rotulo: &'static str,
        detalhe: String,
    },
    #[error("credencial no arquivo {caminho}: {detalhe}")]
    Arquivo { caminho: String, detalhe: String },
    #[error("{0}")]
    Conexao(String),
}

/// Cache das credenciais resolvidas.
#[derive(Default)]
pub struct Resolvedor {
    cache: Mutex<HashMap<String, Secret>>,
}

impl std::fmt::Debug for Resolvedor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Resolvedor(***)")
    }
}

impl Resolvedor {
    /// O resolvedor do processo, com o cache compartilhado por todos os
    /// consumidores.
    pub fn global() -> &'static Resolvedor {
        static R: OnceLock<Resolvedor> = OnceLock::new();
        R.get_or_init(Resolvedor::default)
    }

    /// A credencial da conexão. `None` com a fonte `Nenhuma`, ou com uma conta do
    /// chaveiro vazia (quem chama decide se é erro).
    ///
    /// `cli` troca o executável da CLI do provider (caminho manual, testes).
    pub fn resolver(
        &self,
        conexao: &Conexao,
        chaveiro: &dyn Chaveiro,
        cli: Option<&Path>,
    ) -> Result<Option<Secret>, ErroCredencial> {
        let chave = chave_cache(conexao, chaveiro);
        if let Some(k) = &chave
            && let Some(s) = self.cache.lock().ok().and_then(|c| c.get(k).cloned())
        {
            return Ok(Some(s));
        }
        let lido = ler_fonte(conexao, chaveiro, cli)?;
        if let Some(s) = &lido {
            mascara::registrar(s);
            if let Some(k) = chave
                && let Ok(mut c) = self.cache.lock()
            {
                c.insert(k, s.clone());
            }
        }
        Ok(lido)
    }

    /// Como [`Resolvedor::resolver`], com a sobrescrita: a conta `sobrescrita`
    /// do chaveiro (o token próprio de um repositório), se houver, vale no lugar
    /// da credencial da conexão.
    pub fn resolver_com_sobrescrita(
        &self,
        conexao: &Conexao,
        sobrescrita: Option<&str>,
        chaveiro: &dyn Chaveiro,
        cli: Option<&Path>,
    ) -> Result<Option<Secret>, ErroCredencial> {
        if let Some(conta) = sobrescrita.filter(|c| !c.trim().is_empty())
            && let Some(s) = chaveiro.ler(conta).map_err(ErroCredencial::Chaveiro)?
        {
            mascara::registrar(&s);
            return Ok(Some(s));
        }
        self.resolver(conexao, chaveiro, cli)
    }

    /// Descarta do cache toda entrada com esse valor (o servidor o recusou com
    /// 401 ou 403). A próxima resolução lê a fonte de novo.
    pub fn descartar(&self, segredo: &Secret) {
        if let Ok(mut c) = self.cache.lock() {
            c.retain(|_, s| s.expose() != segredo.expose());
        }
    }

    /// Esvazia o cache (um token foi gravado ou apagado no chaveiro).
    pub fn esquecer_tudo(&self) {
        if let Ok(mut c) = self.cache.lock() {
            c.clear();
        }
    }

    /// Quantas credenciais estão em cache (testes e diagnóstico).
    pub fn em_cache(&self) -> usize {
        self.cache.lock().map(|c| c.len()).unwrap_or(0)
    }
}

/// Chave do cache: o tipo e a fonte (duas conexões com a mesma fonte dividem a
/// entrada). `None`: a fonte não vai para o cache (arquivo, que o analista pode
/// trocar a qualquer momento, e chaveiros que não pedem cache, como o dos
/// testes).
fn chave_cache(conexao: &Conexao, chaveiro: &dyn Chaveiro) -> Option<String> {
    let fonte = match &conexao.credencial {
        FonteCredencial::Chaveiro { conta } if chaveiro.cacheavel() => format!("chaveiro:{conta}"),
        FonteCredencial::Comando { cmd } => format!("comando:{cmd}"),
        FonteCredencial::Cli => format!("cli:{}", conexao.endereco.trim().to_ascii_lowercase()),
        _ => return None,
    };
    Some(format!("{}|{fonte}", conexao.tipo))
}

fn ler_fonte(
    conexao: &Conexao,
    chaveiro: &dyn Chaveiro,
    cli: Option<&Path>,
) -> Result<Option<Secret>, ErroCredencial> {
    let descritor = todos().de(conexao).map_err(ErroCredencial::Conexao)?;
    let rotulo = descritor.rotulo();
    match &conexao.credencial {
        FonteCredencial::Nenhuma => Ok(None),
        FonteCredencial::Chaveiro { conta } => {
            chaveiro.ler(conta).map_err(ErroCredencial::Chaveiro)
        }
        FonteCredencial::Comando { cmd } => {
            Ok(Some(run_credential(rotulo, cmd, CREDENTIAL_DEADLINE)?))
        }
        FonteCredencial::Arquivo { caminho } => ler_arquivo(caminho).map(Some),
        FonteCredencial::Cli => {
            let Some((programa, args)) = descritor.credencial_pela_cli(conexao) else {
                return Err(ErroCredencial::Cli {
                    rotulo,
                    detalhe: "a CLI deste tipo não fornece credencial".into(),
                });
            };
            let mut c = Command::new(cli.map_or_else(|| programa.into(), Path::to_path_buf));
            c.args(args);
            ler_cli(rotulo, c).map(Some)
        }
    }
}

fn ler_arquivo(caminho: &str) -> Result<Secret, ErroCredencial> {
    let erro = |detalhe: String| ErroCredencial::Arquivo {
        caminho: caminho.to_string(),
        detalhe,
    };
    let mut conteudo =
        std::fs::read_to_string(caminho.trim()).map_err(|e| erro(e.kind().to_string()))?;
    let linha = conteudo.lines().next().unwrap_or("").trim().to_string();
    conteudo.zeroize();
    if linha.is_empty() {
        return Err(erro("arquivo vazio".into()));
    }
    Ok(Secret::new(linha))
}

fn ler_cli(rotulo: &'static str, c: Command) -> Result<Secret, ErroCredencial> {
    let erro = |detalhe: String| ErroCredencial::Cli { rotulo, detalhe };
    let mut o = run(c, CREDENTIAL_DEADLINE).map_err(|e| match e {
        RunError::NotFound => erro("CLI não encontrada".into()),
        e => erro(e.to_string()),
    })?;
    if !o.status.success() {
        o.stdout.zeroize();
        let detalhe = cite(&String::from_utf8_lossy(&o.stderr), None);
        return Err(erro(if detalhe.is_empty() {
            "a CLI não está autenticada".into()
        } else {
            detalhe
        }));
    }
    o.stderr.zeroize();
    let linha = String::from_utf8_lossy(&o.stdout)
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    o.stdout.zeroize();
    if linha.is_empty() {
        return Err(erro("a CLI não devolveu credencial".into()));
    }
    Ok(Secret::new(linha))
}

/// O servidor recusou a credencial (401 ou 403)?
pub trait Recusada {
    fn recusada(&self) -> bool;
}

/// Um texto de erro que indica credencial recusada: `401`, `403`,
/// `Unauthorized`, `Forbidden`, `Bad credentials`.
pub fn texto_de_recusa(texto: &str) -> bool {
    let t = texto.to_ascii_lowercase();
    [
        "401",
        "403",
        "unauthorized",
        "forbidden",
        "bad credentials",
        "invalid token",
    ]
    .iter()
    .any(|p| t.contains(p))
}

/// Roda `operacao` com a credencial de `obter`. Se o servidor recusar (401 ou
/// 403), descarta a credencial do cache e tenta **uma** vez mais, com a
/// credencial lida de novo da fonte.
pub fn com_nova_tentativa<T, E: Recusada>(
    mut obter: impl FnMut() -> Result<Option<Secret>, E>,
    mut operacao: impl FnMut(Option<&Secret>) -> Result<T, E>,
) -> Result<T, E> {
    let token = obter()?;
    match operacao(token.as_ref()) {
        Err(e) if e.recusada() => {
            let Some(t) = token else {
                return Err(e);
            };
            Resolvedor::global().descartar(&t);
            let novo = obter()?;
            operacao(novo.as_ref())
        }
        r => r,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chaveiro::ChaveiroMemoria;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Chaveiro que conta as leituras e pede cache, como o nativo.
    #[derive(Default)]
    struct ChaveiroContado {
        dentro: ChaveiroMemoria,
        leituras: AtomicUsize,
    }

    impl Chaveiro for ChaveiroContado {
        fn ler(&self, conta: &str) -> Result<Option<Secret>, String> {
            self.leituras.fetch_add(1, Ordering::SeqCst);
            self.dentro.ler(conta)
        }
        fn gravar(&self, conta: &str, token: &Secret) -> Result<(), String> {
            self.dentro.gravar(conta, token)
        }
        fn apagar(&self, conta: &str) -> Result<(), String> {
            self.dentro.apagar(conta)
        }
        fn cacheavel(&self) -> bool {
            true
        }
    }

    fn conexao(tipo: &str, credencial: FonteCredencial) -> Conexao {
        Conexao {
            id: "c1".into(),
            nome: "C1".into(),
            tipo: tipo.into(),
            endereco: "gitlab.example.com".into(),
            credencial,
            opcoes: Default::default(),
        }
    }

    #[test]
    fn cache_e_descarte() {
        let r = Resolvedor::default();
        let c = ChaveiroContado::default();
        c.gravar("conexao:cache-teste", &Secret::new("tok-cache-1"))
            .unwrap();
        let x = conexao(
            "gitlab",
            FonteCredencial::Chaveiro {
                conta: "conexao:cache-teste".into(),
            },
        );
        for _ in 0..3 {
            assert_eq!(
                r.resolver(&x, &c, None).unwrap().unwrap().expose(),
                "tok-cache-1"
            );
        }
        assert_eq!(
            c.leituras.load(Ordering::SeqCst),
            1,
            "lê o chaveiro uma vez só"
        );
        assert_eq!(r.em_cache(), 1);
        // O token foi trocado e o servidor recusou o antigo.
        c.gravar("conexao:cache-teste", &Secret::new("tok-cache-2"))
            .unwrap();
        r.descartar(&Secret::new("tok-cache-1"));
        assert_eq!(
            r.resolver(&x, &c, None).unwrap().unwrap().expose(),
            "tok-cache-2"
        );
        assert_eq!(c.leituras.load(Ordering::SeqCst), 2);
        r.esquecer_tudo();
        assert_eq!(r.em_cache(), 0);
    }

    #[test]
    fn chaveiro_de_teste_nao_vai_para_o_cache() {
        let r = Resolvedor::default();
        let c = ChaveiroMemoria::default();
        let x = conexao(
            "gitlab",
            FonteCredencial::Chaveiro {
                conta: "host:gitlab.example.com".into(),
            },
        );
        assert!(r.resolver(&x, &c, None).unwrap().is_none());
        c.gravar("host:gitlab.example.com", &Secret::new("t"))
            .unwrap();
        assert_eq!(r.resolver(&x, &c, None).unwrap().unwrap().expose(), "t");
        assert_eq!(r.em_cache(), 0);
    }

    #[test]
    fn sobrescrita_do_repositorio() {
        let r = Resolvedor::default();
        let c = ChaveiroMemoria::default();
        c.gravar(
            "host:gitlab.example.com",
            &Secret::new("global-sobrescrita"),
        )
        .unwrap();
        let x = conexao(
            "gitlab",
            FonteCredencial::Chaveiro {
                conta: "host:gitlab.example.com".into(),
            },
        );
        let conta = "repo:gitlab.example.com/g/app";
        let t = r
            .resolver_com_sobrescrita(&x, Some(conta), &c, None)
            .unwrap();
        assert_eq!(t.unwrap().expose(), "global-sobrescrita");
        c.gravar(conta, &Secret::new("proprio-sobrescrita"))
            .unwrap();
        let t = r
            .resolver_com_sobrescrita(&x, Some(conta), &c, None)
            .unwrap();
        assert_eq!(t.unwrap().expose(), "proprio-sobrescrita");
    }

    #[test]
    fn nenhuma_e_arquivo() {
        let r = Resolvedor::default();
        let c = ChaveiroMemoria::default();
        assert!(
            r.resolver(&conexao("gitlab", FonteCredencial::Nenhuma), &c, None)
                .unwrap()
                .is_none()
        );
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("token");
        std::fs::write(&p, "  tok-do-arquivo  \noutra\n").unwrap();
        let x = conexao(
            "gitlab",
            FonteCredencial::Arquivo {
                caminho: p.to_string_lossy().into(),
            },
        );
        assert_eq!(
            r.resolver(&x, &c, None).unwrap().unwrap().expose(),
            "tok-do-arquivo"
        );
        // O resolvido entra no mascaramento.
        assert_eq!(mascara::mascarar("x tok-do-arquivo y"), "x *** y");
        std::fs::write(&p, "\n").unwrap();
        let e = r.resolver(&x, &c, None).unwrap_err().to_string();
        assert!(e.contains("vazio"), "{e}");
        let y = conexao(
            "gitlab",
            FonteCredencial::Arquivo {
                caminho: dir.path().join("nao-existe").to_string_lossy().into(),
            },
        );
        assert!(r.resolver(&y, &c, None).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn comando_e_cli() {
        use std::os::unix::fs::PermissionsExt;
        let r = Resolvedor::default();
        let c = ChaveiroMemoria::default();
        let x = conexao(
            "gitlab",
            FonteCredencial::Comando {
                cmd: "echo tok-do-comando-resolvedor".into(),
            },
        );
        assert_eq!(
            r.resolver(&x, &c, None).unwrap().unwrap().expose(),
            "tok-do-comando-resolvedor"
        );
        assert_eq!(r.em_cache(), 1);

        let dir = tempfile::tempdir().unwrap();
        let gh = dir.path().join("gh");
        std::fs::write(
            &gh,
            "#!/bin/sh\n[ \"$1 $2\" = \"auth token\" ] && [ \"$4\" = \"github.com\" ] && echo gho_da_cli_resolvedor\n",
        )
        .unwrap();
        std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();
        let mut y = conexao("github", FonteCredencial::Cli);
        y.endereco = "github.com".into();
        assert_eq!(
            r.resolver(&y, &c, Some(&gh)).unwrap().unwrap().expose(),
            "gho_da_cli_resolvedor"
        );
        // `Cli` num tipo sem CLI de credencial.
        let z = conexao("gitlab", FonteCredencial::Cli);
        assert!(r.resolver(&z, &c, Some(&gh)).is_err());
    }

    #[derive(Debug)]
    struct Erro(&'static str);
    impl Recusada for Erro {
        fn recusada(&self) -> bool {
            texto_de_recusa(self.0)
        }
    }

    #[test]
    fn nova_tentativa_em_401() {
        let c = ChaveiroContado::default();
        c.gravar("conexao:retry-teste", &Secret::new("tok-velho-retry"))
            .unwrap();
        let x = conexao(
            "gitlab",
            FonteCredencial::Chaveiro {
                conta: "conexao:retry-teste".into(),
            },
        );
        let obter = || {
            Resolvedor::global()
                .resolver(&x, &c, None)
                .map_err(|_| Erro("chaveiro"))
        };
        // Primeiro uso põe o token velho no cache.
        assert_eq!(obter().unwrap().unwrap().expose(), "tok-velho-retry");
        // O analista trocou o token; o cache ainda tem o velho.
        c.gravar("conexao:retry-teste", &Secret::new("tok-novo-retry"))
            .unwrap();
        let mut usados = vec![];
        let r = com_nova_tentativa(obter, |t| {
            let t = t.unwrap().expose().to_string();
            usados.push(t.clone());
            if t == "tok-velho-retry" {
                Err(Erro("HTTP 401 Unauthorized"))
            } else {
                Ok(t)
            }
        })
        .unwrap();
        assert_eq!(r, "tok-novo-retry");
        assert_eq!(usados, ["tok-velho-retry", "tok-novo-retry"]);
        // Outro erro não tenta de novo.
        let mut n = 0;
        let e = com_nova_tentativa(obter, |_| -> Result<(), Erro> {
            n += 1;
            Err(Erro("timeout"))
        });
        assert!(e.is_err());
        assert_eq!(n, 1);
    }
}
