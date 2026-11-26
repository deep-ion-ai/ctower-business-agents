//! Descritores dos tipos que o app conhece: GitHub, GitLab, Jira, PostgreSQL e
//! MySQL/MariaDB. Os de banco descrevem a conexão; o acesso vem na sub-issue de
//! banco (#439).

use super::{
    CampoOpcao, Capacidade, Conexao, DescritorProvider, OpcaoEscolha, PassoProntidao, TipoCampo,
    TipoFonte, VariavelAmbiente, host_de, host_valido,
};

/// Normaliza um endereço que é só um host (`GitLab.Example.com` →
/// `gitlab.example.com`). Aceita a URL colada (`https://host/...`) e fica com o
/// host.
fn normalizar_host(endereco: &str, rotulo: &str) -> Result<String, String> {
    let host = host_de(endereco);
    if host_valido(&host) {
        Ok(host)
    } else if endereco.trim().is_empty() {
        Err(format!("informe o host do {rotulo}"))
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
