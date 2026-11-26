//! Descritores dos tipos que o app conhece: GitHub, GitLab, Jira, PostgreSQL e
//! MySQL/MariaDB. Os de banco descrevem a conexão; o acesso vem na sub-issue de
//! banco (#439).

use super::prontidao::{ContextoProntidao, versao_e_caminho};
use super::{
    CampoOpcao, Capacidade, Conexao, DescritorProvider, OpcaoEscolha, PassoProntidao, TipoCampo,
    TipoFonte, VariavelAmbiente, host_de, host_valido,
};
use crate::ForgeKind;
use crate::acessos::{ErroVerificacao, VerificacaoToken, verificar_token_host};
use std::path::{Path, PathBuf};

/// Resultado de um passo de prontidão: o detalhe ou o motivo da falha.
type Resultado = Option<Result<Option<String>, String>>;

/// O "quem sou eu" de uma forja pela API (`gh api user`, `glab api user`).
/// Sem credencial, não há o que conferir.
fn quem_sou_eu_na_forja(
    forja: ForgeKind,
    ctx: &ContextoProntidao<'_>,
) -> Result<Option<String>, String> {
    let Some(t) = ctx.credencial() else {
        return Ok(Some("sem credencial: acesso anônimo".into()));
    };
    let host = host_de(&ctx.conexao.endereco);
    match verificar_token_host(forja, &host, t, ctx.ferramentas) {
        Ok(VerificacaoToken { usuario, expira }) => {
            let mut d = usuario.map_or_else(
                || "token aceito".to_string(),
                |u| format!("conectado como {u}"),
            );
            if let Some(e) = expira {
                d.push_str(&format!(", token válido até {e}"));
            }
            Ok(Some(d))
        }
        Err(ErroVerificacao::SemFerramenta(cli)) => Err(format!("{cli} não encontrado")),
        Err(ErroVerificacao::Recusado(m)) => Err(m),
    }
}

/// `glab api version` (a versão do servidor) e, com token, quem é o dono dele.
fn versao_do_gitlab(glab: &Path, ctx: &ContextoProntidao<'_>) -> Result<Option<String>, String> {
    let mut c = ctx.comando(glab);
    c.args(["api", "version"]);
    let linha = ctx.saida(c)?;
    let versao = serde_json::from_str::<serde_json::Value>(&linha)
        .ok()
        .and_then(|v| v["version"].as_str().map(str::to_string))
        .unwrap_or(linha);
    Ok(Some(match quem_sou_eu_na_forja(ForgeKind::Gitlab, ctx)? {
        Some(q) => format!("GitLab {versao}, {q}"),
        None => format!("GitLab {versao}"),
    }))
}

/// Arquivo de configuração do jira-cli: o da conexão ou o padrão.
pub(crate) fn arquivo_de_config_jira(informado: Option<&str>) -> PathBuf {
    if let Some(f) = informado.map(str::trim).filter(|f| !f.is_empty()) {
        return f.into();
    }
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_default();
    home.join(".config/.jira/.config.yml")
}

/// O `server` do arquivo de configuração do jira-cli é a URL da conexão.
fn servidor_do_jira(cfg: &Path, url: &str) -> Result<Option<String>, String> {
    let yml = std::fs::read_to_string(cfg).map_err(|e| e.to_string())?;
    let s = crate::prontidao::servidor_do_config(&yml).ok_or("sem \"server\" na configuração")?;
    let url = url.trim().trim_end_matches('/');
    if s == url {
        Ok(Some(s))
    } else {
        Err(format!("a configuração usa {s}, a conexão usa {url}"))
    }
}

/// Normaliza um endereço que é só um host (`GitLab.Example.com` →
/// `gitlab.example.com`). Aceita a URL colada (`https://host/...`) e fica com o
/// host.
fn normalizar_host(endereco: &str, rotulo: &str) -> Result<String, String> {
    let host = host_de(endereco);
    if host_valido(&host) {
        Ok(host)
    } else if endereco.trim().is_empty() {
        Err(format!("informe o host do {rotulo}"))
    } else if endereco.contains('@') {
        // Pode ter usuário e senha embutidos: a mensagem não repete o endereço.
        Err(format!(
            "host do {rotulo} inválido: informe só o host, sem usuário nem senha"
        ))
    } else {
        Err(format!("host do {rotulo} inválido: {}", endereco.trim()))
    }
}

// ---------------------------------------------------------------- GitHub

/// GitHub (github.com ou GitHub Enterprise): forja e rastreador, pela `gh`.
#[derive(Debug, Clone, Copy, Default)]
pub struct DescritorGithub;

impl DescritorProvider for DescritorGithub {
    fn tipo(&self) -> &'static str {
        "github"
    }
    fn rotulo(&self) -> &'static str {
        "GitHub"
    }
    fn icone(&self) -> &'static str {
        "github"
    }
    fn capacidades(&self) -> &'static [Capacidade] {
        &[Capacidade::Forja, Capacidade::Rastreador]
    }
    fn endereco_padrao(&self) -> &'static str {
        crate::codigo::HOST_GITHUB
    }
    fn formato_endereco(&self) -> &'static str {
        "host, como github.com"
    }
    fn normalizar_endereco(&self, endereco: &str) -> Result<String, String> {
        normalizar_host(endereco, "GitHub")
    }
    fn fontes_aceitas(&self) -> &'static [TipoFonte] {
        &[
            TipoFonte::Chaveiro,
            TipoFonte::Cli,
            TipoFonte::Comando,
            TipoFonte::Arquivo,
            TipoFonte::Nenhuma,
        ]
    }
    fn variaveis_ambiente(&self, conexao: &Conexao) -> Vec<VariavelAmbiente> {
        vec![
            VariavelAmbiente::credencial("GH_TOKEN"),
            VariavelAmbiente::texto("GH_HOST", host_de(&conexao.endereco)),
        ]
    }
    fn credencial_pela_cli(&self, conexao: &Conexao) -> Option<(&'static str, Vec<String>)> {
        Some((
            "gh",
            vec![
                "auth".into(),
                "token".into(),
                "--hostname".into(),
                host_de(&conexao.endereco),
            ],
        ))
    }
    fn passos_prontidao(&self) -> &'static [PassoProntidao] {
        &[
            PassoProntidao {
                id: "cli",
                rotulo: "GitHub CLI (gh) instalada",
                orientacao: "Instale a GitHub CLI (https://cli.github.com) ou informe o caminho do executável.",
            },
            PassoProntidao {
                id: "credencial",
                rotulo: "Credencial disponível",
                orientacao: "Guarde um token de acesso na conexão ou rode \"gh auth login\".",
            },
            PassoProntidao {
                id: "quem_sou_eu",
                rotulo: "Acesso ao GitHub (quem sou eu)",
                orientacao: "Confira se o token vale e tem o escopo \"repo\".",
            },
        ]
    }
    fn executar_passo(&self, passo: &str, ctx: &mut ContextoProntidao<'_>) -> Resultado {
        let gh = ctx.ferramentas.gh.clone();
        Some(match passo {
            "cli" => versao_e_caminho(&gh, &["--version"]),
            "credencial" => ctx.resolver_credencial(Some(&gh)),
            "quem_sou_eu" => quem_sou_eu_na_forja(ForgeKind::Github, ctx),
            _ => return None,
        })
    }
}

// ---------------------------------------------------------------- GitLab

/// GitLab (gitlab.com ou instância própria): forja e rastreador, pela `glab`.
#[derive(Debug, Clone, Copy, Default)]
pub struct DescritorGitlab;

impl DescritorProvider for DescritorGitlab {
    fn tipo(&self) -> &'static str {
        "gitlab"
    }
    fn rotulo(&self) -> &'static str {
        "GitLab"
    }
    fn icone(&self) -> &'static str {
        "gitlab"
    }
    fn capacidades(&self) -> &'static [Capacidade] {
        &[Capacidade::Forja, Capacidade::Rastreador]
    }
    fn endereco_padrao(&self) -> &'static str {
        "gitlab.com"
    }
    fn formato_endereco(&self) -> &'static str {
        "host, como gitlab.example.com"
    }
    fn normalizar_endereco(&self, endereco: &str) -> Result<String, String> {
        normalizar_host(endereco, "GitLab")
    }
    fn fontes_aceitas(&self) -> &'static [TipoFonte] {
        &[
            TipoFonte::Chaveiro,
            TipoFonte::Comando,
            TipoFonte::Arquivo,
            TipoFonte::Nenhuma,
        ]
    }
    fn variaveis_ambiente(&self, conexao: &Conexao) -> Vec<VariavelAmbiente> {
        vec![
            VariavelAmbiente::credencial("GITLAB_TOKEN"),
            VariavelAmbiente::texto("GITLAB_HOST", host_de(&conexao.endereco)),
        ]
    }
    fn passos_prontidao(&self) -> &'static [PassoProntidao] {
        &[
            PassoProntidao {
                id: "cli",
                rotulo: "GitLab CLI (glab) instalada",
                orientacao: "Instale a GitLab CLI (https://gitlab.com/gitlab-org/cli) ou informe o caminho do executável.",
            },
            PassoProntidao {
                id: "credencial",
                rotulo: "Credencial disponível",
                orientacao: "Guarde um token de acesso pessoal na conexão, com o escopo \"api\".",
            },
            PassoProntidao {
                id: "versao_servidor",
                rotulo: "Acesso ao GitLab (versão do servidor)",
                orientacao: "Confira o host e se o token vale para ele.",
            },
        ]
    }
    fn executar_passo(&self, passo: &str, ctx: &mut ContextoProntidao<'_>) -> Resultado {
        let glab = ctx.ferramentas.glab.clone();
        Some(match passo {
            "cli" => versao_e_caminho(&glab, &["version"]),
            "credencial" => ctx.resolver_credencial(None),
            "versao_servidor" => versao_do_gitlab(&glab, ctx),
            _ => return None,
        })
    }
}

// ---------------------------------------------------------------- Jira

const CAMPOS_JIRA: &[CampoOpcao] = &[CampoOpcao {
    chave: "config_file",
    rotulo: "Arquivo de configuração do jira-cli",
    ajuda: "Vazio usa ~/.config/.jira/.config.yml.",
    tipo: TipoCampo::Texto,
    padrao: None,
    obrigatorio: false,
}];

/// Jira (Server, Data Center ou Cloud): rastreador, pela `jira` (jira-cli).
#[derive(Debug, Clone, Copy, Default)]
pub struct DescritorJira;

impl DescritorProvider for DescritorJira {
    fn tipo(&self) -> &'static str {
        "jira"
    }
    fn rotulo(&self) -> &'static str {
        "Jira"
    }
    fn icone(&self) -> &'static str {
        "jira"
    }
    fn capacidades(&self) -> &'static [Capacidade] {
        &[Capacidade::Rastreador]
    }
    fn endereco_padrao(&self) -> &'static str {
        ""
    }
    fn formato_endereco(&self) -> &'static str {
        "URL do servidor, como https://jira.example.com"
    }
    fn normalizar_endereco(&self, endereco: &str) -> Result<String, String> {
        let u = endereco.trim().trim_end_matches('/');
        if u.is_empty() {
            return Err("informe a URL do Jira".into());
        }
        let Some(resto) = u
            .strip_prefix("https://")
            .or_else(|| u.strip_prefix("http://"))
        else {
            return Err(format!("URL do Jira inválida: {u} (comece com https://)"));
        };
        if resto.contains('@') || u.contains(char::is_whitespace) {
            return Err(format!("URL do Jira inválida: {u}"));
        }
        if !host_valido(&host_de(u)) {
            return Err(format!("URL do Jira inválida: {u}"));
        }
        Ok(u.to_string())
    }
    fn fontes_aceitas(&self) -> &'static [TipoFonte] {
        &[TipoFonte::Chaveiro, TipoFonte::Comando, TipoFonte::Arquivo]
    }
    fn variaveis_ambiente(&self, conexao: &Conexao) -> Vec<VariavelAmbiente> {
        let mut v = vec![VariavelAmbiente::credencial("JIRA_API_TOKEN")];
        if let Some(f) = conexao.opcao(self, "config_file") {
            v.push(VariavelAmbiente::texto("JIRA_CONFIG_FILE", f));
        }
        v
    }
    fn campos_opcoes(&self) -> &'static [CampoOpcao] {
        CAMPOS_JIRA
    }
    fn passos_prontidao(&self) -> &'static [PassoProntidao] {
        &[
            PassoProntidao {
                id: "cli",
                rotulo: "jira-cli instalado",
                orientacao: "Instale o jira-cli (https://github.com/ankitpokhrel/jira-cli) ou informe o caminho do executável.",
            },
            PassoProntidao {
                id: "config",
                rotulo: "Configuração do jira-cli",
                orientacao: "Rode \"jira init\" ou informe o arquivo de configuração na conexão.",
            },
            PassoProntidao {
                id: "servidor",
                rotulo: "Servidor da configuração igual ao da conexão",
                orientacao: "O \"server\" do arquivo de configuração precisa ser a URL da conexão.",
            },
            PassoProntidao {
                id: "credencial",
                rotulo: "Credencial disponível",
                orientacao: "Guarde o token na conexão ou confira o comando que o imprime.",
            },
            PassoProntidao {
                id: "quem_sou_eu",
                rotulo: "Acesso ao Jira (jira me)",
                orientacao: "Confira se o token vale para o servidor.",
            },
        ]
    }
    fn executar_passo(&self, passo: &str, ctx: &mut ContextoProntidao<'_>) -> Resultado {
        let jira = ctx.ferramentas.jira.clone();
        let cfg = arquivo_de_config_jira(ctx.conexao.opcao(self, "config_file"));
        Some(match passo {
            "cli" => versao_e_caminho(&jira, &["version"]),
            "config" => cfg
                .is_file()
                .then(|| Some(cfg.display().to_string()))
                .ok_or_else(|| format!("{} não existe", cfg.display())),
            "servidor" => servidor_do_jira(&cfg, &ctx.conexao.endereco),
            "credencial" => ctx.resolver_credencial(None),
            "quem_sou_eu" => {
                let mut c = ctx.comando(&jira);
                c.arg("me");
                ctx.saida(c).map(|u| Some(format!("conectado como {u}")))
            }
            _ => return None,
        })
    }
}

// ---------------------------------------------------------------- Bancos

const TLS: &[OpcaoEscolha] = &[
    OpcaoEscolha {
        valor: "exigir",
        rotulo: "Exigir",
    },
    OpcaoEscolha {
        valor: "preferir",
        rotulo: "Preferir",
    },
    OpcaoEscolha {
        valor: "desligar",
        rotulo: "Desligar",
    },
];

/// Campos comuns às conexões de banco (#439).
const CAMPOS_BANCO: &[CampoOpcao] = &[
    CampoOpcao {
        chave: "usuario",
        rotulo: "Usuário",
        ajuda: "Usuário do banco, de preferência só com leitura.",
        tipo: TipoCampo::Texto,
        padrao: None,
        obrigatorio: true,
    },
    CampoOpcao {
        chave: "esquema",
        rotulo: "Esquema padrão",
        ajuda: "Vazio usa o padrão do usuário.",
        tipo: TipoCampo::Texto,
        padrao: None,
        obrigatorio: false,
    },
    CampoOpcao {
        chave: "tempo_limite_s",
        rotulo: "Tempo limite por consulta (s)",
        ajuda: "A consulta é cancelada depois desse tempo.",
        tipo: TipoCampo::Numero { min: 1, max: 600 },
        padrao: Some("30"),
        obrigatorio: false,
    },
    CampoOpcao {
        chave: "limite_linhas",
        rotulo: "Limite de linhas",
        ajuda: "Máximo de linhas devolvidas por consulta.",
        tipo: TipoCampo::Numero { min: 1, max: 10000 },
        padrao: Some("200"),
        obrigatorio: false,
    },
    CampoOpcao {
        chave: "tls",
        rotulo: "TLS",
        ajuda: "Criptografia da conexão com o servidor.",
        tipo: TipoCampo::Escolha { opcoes: TLS },
        padrao: Some("preferir"),
        obrigatorio: false,
    },
    CampoOpcao {
        chave: "ambiente",
        rotulo: "Ambiente",
        ajuda: "Rótulo do ambiente, como \"homologação\" ou \"sustentação\".",
        tipo: TipoCampo::Texto,
        padrao: None,
        obrigatorio: false,
    },
];

const PASSOS_BANCO: &[PassoProntidao] = &[
    PassoProntidao {
        id: "conectar",
        rotulo: "Conexão com o servidor",
        orientacao: "Confira host, porta, banco, usuário e a credencial.",
    },
    PassoProntidao {
        id: "versao",
        rotulo: "Versão do servidor",
        orientacao: "O servidor respondeu de um jeito inesperado; confira se o tipo da conexão está certo.",
    },
    PassoProntidao {
        id: "so_leitura",
        rotulo: "Sessão só de leitura",
        orientacao: "O app não conseguiu impor a só leitura; use um usuário sem permissão de escrita.",
    },
    PassoProntidao {
        id: "esquemas",
        rotulo: "Esquemas visíveis",
        orientacao: "O usuário não enxerga nenhum esquema; confira as permissões de leitura.",
    },
];

const FONTES_BANCO: &[TipoFonte] = &[
    TipoFonte::Chaveiro,
    TipoFonte::Comando,
    TipoFonte::Arquivo,
    TipoFonte::Nenhuma,
];

/// `host[:porta]/banco`; a porta ausente fica com a padrão.
fn normalizar_banco(endereco: &str, rotulo: &str, porta_padrao: u16) -> Result<String, String> {
    let e = endereco.trim();
    let formato = || format!("endereço do {rotulo} inválido: {e} (use host:porta/banco)");
    if e.is_empty() {
        return Err(format!("informe o endereço do {rotulo} (host:porta/banco)"));
    }
    if e.contains("://") || e.contains('@') {
        // String de conexão com usuário e senha não vai no endereço.
        return Err(format!(
            "informe só host:porta/banco no endereço do {rotulo}; o usuário e a senha vão nos campos próprios"
        ));
    }
    let (autoridade, banco) = e.split_once('/').ok_or_else(formato)?;
    let banco = banco.trim_matches('/');
    if banco.is_empty() || banco.contains(['/', ' ', '?']) {
        return Err(formato());
    }
    let (host, porta) = match autoridade.rsplit_once(':') {
        Some((h, p)) => (
            h,
            p.parse::<u16>()
                .ok()
                .filter(|p| *p > 0)
                .ok_or_else(formato)?,
        ),
        None => (autoridade, porta_padrao),
    };
    let host = host.to_ascii_lowercase();
    if !host_valido(&host) {
        return Err(formato());
    }
    Ok(format!("{host}:{porta}/{banco}"))
}

/// PostgreSQL, com driver nativo (#439).
#[derive(Debug, Clone, Copy, Default)]
pub struct DescritorPostgres;

impl DescritorProvider for DescritorPostgres {
    fn tipo(&self) -> &'static str {
        "postgres"
    }
    fn rotulo(&self) -> &'static str {
        "PostgreSQL"
    }
    fn icone(&self) -> &'static str {
        "database"
    }
    fn capacidades(&self) -> &'static [Capacidade] {
        &[Capacidade::Banco]
    }
    fn endereco_padrao(&self) -> &'static str {
        ""
    }
    fn formato_endereco(&self) -> &'static str {
        "host:porta/banco, como db.example.com:5432/faturamento"
    }
    fn normalizar_endereco(&self, endereco: &str) -> Result<String, String> {
        normalizar_banco(endereco, "PostgreSQL", 5432)
    }
    fn fontes_aceitas(&self) -> &'static [TipoFonte] {
        FONTES_BANCO
    }
    fn variaveis_ambiente(&self, _: &Conexao) -> Vec<VariavelAmbiente> {
        vec![]
    }
    fn campos_opcoes(&self) -> &'static [CampoOpcao] {
        CAMPOS_BANCO
    }
    fn passos_prontidao(&self) -> &'static [PassoProntidao] {
        PASSOS_BANCO
    }
}

/// MySQL e MariaDB, com driver nativo (#439).
#[derive(Debug, Clone, Copy, Default)]
pub struct DescritorMysql;

impl DescritorProvider for DescritorMysql {
    fn tipo(&self) -> &'static str {
        "mysql"
    }
    fn rotulo(&self) -> &'static str {
        "MySQL/MariaDB"
    }
    fn icone(&self) -> &'static str {
        "database"
    }
    fn capacidades(&self) -> &'static [Capacidade] {
        &[Capacidade::Banco]
    }
    fn endereco_padrao(&self) -> &'static str {
        ""
    }
    fn formato_endereco(&self) -> &'static str {
        "host:porta/banco, como db.example.com:3306/faturamento"
    }
    fn normalizar_endereco(&self, endereco: &str) -> Result<String, String> {
        normalizar_banco(endereco, "MySQL/MariaDB", 3306)
    }
    fn fontes_aceitas(&self) -> &'static [TipoFonte] {
        FONTES_BANCO
    }
    fn variaveis_ambiente(&self, _: &Conexao) -> Vec<VariavelAmbiente> {
        vec![]
    }
    fn campos_opcoes(&self) -> &'static [CampoOpcao] {
        CAMPOS_BANCO
    }
    fn passos_prontidao(&self) -> &'static [PassoProntidao] {
        PASSOS_BANCO
    }
}
