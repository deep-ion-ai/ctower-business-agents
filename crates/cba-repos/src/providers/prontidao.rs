//! Prontidão de uma conexão (#437): roda, na ordem, os passos que o descritor
//! declara em [`DescritorProvider::passos_prontidao`], cada um executado pelo
//! próprio descritor ([`DescritorProvider::executar_passo`]).
//!
//! É o mesmo caminho do botão **Testar** da conexão, nas Configurações, e da
//! saúde do projeto: a prontidão de um repositório ou do rastreador que
//! referencia uma conexão começa pelos passos dela e acrescenta só o que é do
//! projeto (acesso ao repositório, issues ligadas).

use super::{Conexao, DescritorProvider, FonteCredencial, Resolvedor, aplicar_ambiente, todos};
use crate::Secret;
use crate::demanda::{Ferramentas, PRAZO_CLI};
use crate::process::{RunError, cite, run};
use crate::prontidao::{Construtor, Integracao};
use std::path::{Path, PathBuf};
use std::process::Command;

/// O que um passo precisa: a conexão, as CLIs e a credencial já lida por um
/// passo anterior.
pub struct ContextoProntidao<'a> {
    pub conexao: &'a Conexao,
    pub descritor: &'a dyn DescritorProvider,
    pub ferramentas: &'a Ferramentas,
    credencial: Option<Secret>,
}

impl ContextoProntidao<'_> {
    /// A credencial lida pelo passo `credencial`, se já rodou.
    pub fn credencial(&self) -> Option<&Secret> {
        self.credencial.as_ref()
    }

    /// O passo `credencial`: lê a fonte da conexão pelo resolvedor. O detalhe
    /// diz de onde veio, nunca o valor. `cli` é o executável da CLI do provider
    /// (fonte `Cli`).
    pub fn resolver_credencial(&mut self, cli: Option<&Path>) -> Result<Option<String>, String> {
        let fonte = &self.conexao.credencial;
        if matches!(fonte, FonteCredencial::Nenhuma) {
            return Ok(Some("sem credencial".into()));
        }
        match Resolvedor::global().resolver(self.conexao, self.ferramentas.chaveiro.as_ref(), cli) {
            Ok(Some(s)) => {
                self.credencial = Some(s);
                Ok(Some(descrever_fonte(fonte)))
            }
            Ok(None) => Err(match fonte {
                FonteCredencial::Chaveiro { conta } => {
                    format!("nada guardado na conta \"{conta}\" do chaveiro")
                }
                _ => "a fonte da credencial não devolveu nada".into(),
            }),
            Err(e) => Err(e.to_string()),
        }
    }

    /// Comando da CLI com as variáveis de ambiente do descritor (a credencial
    /// só aqui, nunca em argumento).
    pub fn comando(&self, programa: &Path) -> Command {
        let mut c = Command::new(programa);
        aplicar_ambiente(
            &mut c,
            self.descritor,
            self.conexao,
            self.credencial.as_ref(),
        );
        c
    }

    /// Roda o comando e devolve a primeira linha do `stdout`; os erros passam
    /// pelo mascaramento.
    pub fn saida(&self, c: Command) -> Result<String, String> {
        crate::prontidao::saida(c, self.credencial.as_ref())
    }
}

/// De onde veio a credencial, para o detalhe do passo.
pub fn descrever_fonte(fonte: &FonteCredencial) -> String {
    match fonte {
        FonteCredencial::Chaveiro { conta } => format!("chaveiro do sistema ({conta})"),
        FonteCredencial::Comando { .. } => "comando de credencial".into(),
        FonteCredencial::Cli => "login da própria CLI".into(),
        FonteCredencial::Arquivo { caminho } => format!("arquivo {caminho}"),
        FonteCredencial::Nenhuma => "sem credencial".into(),
    }
}

/// O executável no `PATH` (ou o próprio caminho, se informado com pastas).
pub fn localizar(programa: &Path) -> Option<PathBuf> {
    if programa.components().count() > 1 {
        return programa.is_file().then(|| programa.to_path_buf());
    }
    let path = std::env::var_os("PATH")?;
    let extensoes: &[&str] = if cfg!(windows) {
        &["", ".exe", ".cmd", ".bat"]
    } else {
        &[""]
    };
    std::env::split_paths(&path).find_map(|pasta| {
        extensoes.iter().find_map(|ext| {
            let mut nome = programa.as_os_str().to_os_string();
            nome.push(ext);
            let c = pasta.join(nome);
            c.is_file().then_some(c)
        })
    })
}

/// O passo `cli`: a versão (primeira linha da saída de `args`) e o caminho do
/// executável.
pub fn versao_e_caminho(programa: &Path, args: &[&str]) -> Result<Option<String>, String> {
    let mut c = Command::new(programa);
    c.args(args);
    let o = run(c, PRAZO_CLI).map_err(|e| match e {
        RunError::NotFound => format!("{} não encontrado", programa.display()),
        e => cite(&e.to_string(), None),
    })?;
    if !o.status.success() {
        return Err(format!(
            "{} falhou: {}",
            programa.display(),
            cite(String::from_utf8_lossy(&o.stderr).trim(), None)
        ));
    }
    let versao = String::from_utf8_lossy(&o.stdout)
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    let caminho = localizar(programa).unwrap_or_else(|| programa.to_path_buf());
    Ok(Some(format!("{versao} ({})", caminho.display())))
}

/// Roda os passos da conexão no construtor e devolve a credencial lida (para
/// os passos do projeto que vêm depois).
pub(crate) fn rodar(conexao: &Conexao, f: &Ferramentas, b: &mut Construtor) -> Option<Secret> {
    let descritor = match todos().de(conexao) {
        Ok(d) => d,
        Err(e) => {
            b.passo(
                "cadastro",
                "Tipo de conexão conhecido",
                "Atualize o app ou cadastre a conexão de novo.",
                || Err(e),
            );
            return None;
        }
    };
    b.passo(
        "cadastro",
        "Cadastro da conexão",
        "Corrija a conexão em Configurações › Conexões.",
        || descritor.validar(conexao).map(|()| None),
    );
    let mut ctx = ContextoProntidao {
        conexao,
        descritor,
        ferramentas: f,
        credencial: None,
    };
    for p in descritor.passos_prontidao() {
        if b.falhou {
            b.passo(p.id, p.rotulo, p.orientacao, || Ok(None));
            continue;
        }
        match descritor.executar_passo(p.id, &mut ctx) {
            Some(r) => b.passo(p.id, p.rotulo, p.orientacao, || r),
            None => b.pular(p.id, p.rotulo, "ainda não verificado pelo app"),
        }
    }
    ctx.credencial
}

/// O "Testar" de uma conexão: cada passo do descritor, com a orientação de
/// quem falhou. Os passos depois de uma falha ficam pulados.
pub fn testar(conexao: &Conexao, f: &Ferramentas) -> Integracao {
    let mut b = Construtor::novo();
    rodar(conexao, f, &mut b);
    let rotulo = todos()
        .get(&conexao.tipo)
        .map_or(conexao.tipo.as_str(), |d| d.rotulo());
    Integracao {
        titulo: format!("{} ({rotulo})", conexao.nome.trim()),
        passos: b.passos,
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::chaveiro::{Chaveiro, ChaveiroMemoria};
    use crate::demanda::falsas::script;
    use crate::prontidao::Situacao;
    use std::collections::BTreeMap;
    use std::sync::Arc;

    fn conexao(id: &str, tipo: &str, endereco: &str, conta: &str) -> Conexao {
        Conexao {
            id: id.into(),
            nome: format!("Conexão {id}"),
            tipo: tipo.into(),
            endereco: endereco.into(),
            credencial: FonteCredencial::Chaveiro {
                conta: conta.into(),
            },
            opcoes: BTreeMap::new(),
        }
    }

    fn glab_falso(dir: &Path) -> PathBuf {
        script(
            dir,
            "glab",
            r#"case "$1 $2" in
"version ") echo "glab 1.48.0";;
"api version") [ "$GITLAB_HOST" = gitlab.example.com ] && [ "$GITLAB_TOKEN" = tok-gitlab-prontidao ] && echo '{"version":"17.2.0"}' || { echo "401 Unauthorized" >&2; exit 1; };;
"api user") [ "$GITLAB_TOKEN" = tok-gitlab-prontidao ] && echo '{"username":"ana"}' || { echo "401 Unauthorized" >&2; exit 1; };;
*) exit 2;;
esac"#,
        )
    }

    #[test]
    fn testar_gitlab_mostra_cada_passo_com_o_detalhe() {
        let d = tempfile::tempdir().unwrap();
        let chaveiro = Arc::new(ChaveiroMemoria::default());
        chaveiro
            .gravar("conexao:gl-prontidao", &Secret::new("tok-gitlab-prontidao"))
            .unwrap();
        let f = Ferramentas {
            glab: glab_falso(d.path()),
            chaveiro: chaveiro.clone(),
            ..Default::default()
        };
        let c = conexao(
            "gl-prontidao",
            "gitlab",
            "gitlab.example.com",
            "conexao:gl-prontidao",
        );
        let i = testar(&c, &f);
        assert!(i.pronta(), "{i:?}");
        let ids: Vec<_> = i.passos.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["cadastro", "cli", "credencial", "versao_servidor"]);
        let cli = i.passos[1].detalhe.as_deref().unwrap();
        assert!(cli.starts_with("glab 1.48.0 ("), "{cli}");
        assert!(cli.contains("glab)"), "{cli}");
        assert_eq!(
            i.passos[3].detalhe.as_deref(),
            Some("GitLab 17.2.0, conectado como ana")
        );
        assert!(!format!("{i:?}").contains("tok-gitlab-prontidao"));
    }

    #[test]
    fn saude_do_projeto_comeca_pelos_passos_da_conexao() {
        use crate::{ForgeKind, RepositorioCodigo, Repositorios};
        let d = tempfile::tempdir().unwrap();
        let chaveiro = Arc::new(ChaveiroMemoria::default());
        chaveiro
            .gravar("conexao:gl-saude", &Secret::new("tok-gitlab-prontidao"))
            .unwrap();
        crate::providers::Catalogo::global().gravar(conexao(
            "gl-saude",
            "gitlab",
            "gitlab.example.com",
            "conexao:gl-saude",
        ));
        let git = script(
            d.path(),
            "git",
            r#"case "$*" in
*ls-remote*gitlab.example.com/g/app*) [ "$CBA_GIT_TOKEN" = tok-gitlab-prontidao ] || { echo "Authentication failed" >&2; exit 128; };;
*) exit 2;;
esac"#,
        );
        let glab = script(
            d.path(),
            "glab",
            r#"case "$1 $2" in
"version ") echo "glab 1.48.0";;
"api version") echo '{"version":"17.2.0"}';;
"api user") echo '{"username":"ana"}';;
"api projects/g%2Fapp") [ "$GITLAB_TOKEN" = tok-gitlab-prontidao ] && echo '{}' || exit 1;;
*) exit 2;;
esac"#,
        );
        let f = Ferramentas {
            git,
            glab,
            chaveiro,
            ..Default::default()
        };
        let r = Repositorios {
            codigo: vec![RepositorioCodigo {
                forja: ForgeKind::Gitlab,
                host: "gitlab.example.com".into(),
                caminho: "g/app".into(),
                url_clone: "https://gitlab.example.com/g/app.git".into(),
                conexao: "gl-saude".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let p = crate::prontidao::conferir(&r, &f);
        let i = &p.integracoes[0];
        assert!(p.pronta(), "{p:?}");
        assert_eq!(i.titulo, "Código g/app (Conexão gl-saude)");
        let ids: Vec<_> = i.passos.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(
            ids,
            ["cadastro", "cli", "credencial", "versao_servidor", "acesso"]
        );

        // Conexão que sumiu: um passo só, com a orientação.
        let mut sem = r.clone();
        sem.codigo[0].conexao = "gl-saude-sumiu".into();
        let i = &crate::prontidao::conferir(&sem, &f).integracoes[0];
        assert_eq!(i.passos.len(), 1);
        assert_eq!(i.passos[0].situacao, Situacao::Falhou);
        assert!(i.passos[0].orientacao.is_some());
    }

    #[test]
    fn falha_traz_a_orientacao_do_descritor_e_pula_o_resto() {
        let d = tempfile::tempdir().unwrap();
        let f = Ferramentas {
            glab: glab_falso(d.path()),
            chaveiro: Arc::new(ChaveiroMemoria::default()),
            ..Default::default()
        };
        let c = conexao(
            "gl-sem-token",
            "gitlab",
            "gitlab.example.com",
            "conexao:gl-sem-token",
        );
        let i = testar(&c, &f);
        let sit: Vec<_> = i.passos.iter().map(|p| p.situacao).collect();
        assert_eq!(
            sit,
            [
                Situacao::Ok,
                Situacao::Ok,
                Situacao::Falhou,
                Situacao::Pulado
            ]
        );
        assert!(
            i.passos[2]
                .detalhe
                .as_deref()
                .unwrap()
                .contains("conexao:gl-sem-token")
        );
        assert_eq!(
            i.passos[2].orientacao.as_deref(),
            todos()
                .get("gitlab")
                .unwrap()
                .passos_prontidao()
                .iter()
                .find(|p| p.id == "credencial")
                .map(|p| p.orientacao)
        );
    }

    #[test]
    fn jira_confere_configuracao_servidor_e_quem_sou_eu() {
        let d = tempfile::tempdir().unwrap();
        let yml = d.path().join("jira.yml");
        std::fs::write(&yml, "server: https://jira.example.com/\n").unwrap();
        let jira = script(
            d.path(),
            "jira",
            r#"case "$1" in
version) echo "jira version 1.7.0";;
me) [ "$JIRA_API_TOKEN" = tok-jira-prontidao ] && echo "ana@example.com" || { echo 401 >&2; exit 1; };;
esac"#,
        );
        let f = Ferramentas {
            jira,
            chaveiro: Arc::new(ChaveiroMemoria::default()),
            ..Default::default()
        };
        let mut c = conexao("jira-prontidao", "jira", "https://jira.example.com", "x");
        c.credencial = FonteCredencial::Comando {
            cmd: "echo tok-jira-prontidao".into(),
        };
        c.opcoes
            .insert("config_file".into(), yml.display().to_string());
        let i = testar(&c, &f);
        assert!(i.pronta(), "{i:?}");
        assert_eq!(
            i.passos.last().unwrap().detalhe.as_deref(),
            Some("conectado como ana@example.com")
        );

        std::fs::write(&yml, "server: https://outro.example.com\n").unwrap();
        let i = testar(&c, &f);
        let servidor = i.passos.iter().find(|p| p.id == "servidor").unwrap();
        assert_eq!(servidor.situacao, Situacao::Falhou);
        assert!(servidor.orientacao.is_some());
    }

    #[test]
    fn cadastro_invalido_falha_e_pula_os_passos_seguintes() {
        let f = Ferramentas {
            chaveiro: Arc::new(ChaveiroMemoria::default()),
            ..Default::default()
        };
        let c = conexao("gl-invalida", "gitlab", "https://u:senha@", "x");
        let i = testar(&c, &f);
        assert_eq!(i.passos[0].situacao, Situacao::Falhou);
        assert!(i.passos[1..].iter().all(|p| p.situacao == Situacao::Pulado));
        assert!(!format!("{i:?}").contains("u:senha"));
    }
}
