//! Providers (#435): conexões externas e de banco com configuração e
//! credencial padronizadas.
//!
//! Uma [`Conexao`] é cadastrada **uma vez**, nas Configurações do app (nunca na
//! base do projeto, que pode ser publicada), e referenciada pelos projetos pelo
//! `id`. A credencial vem de uma [`FonteCredencial`]: a conexão guarda só de onde
//! ela vem, nunca o valor.
//!
//! O que depende do tipo da conexão (rótulo, capacidades, endereço padrão e
//! validação, fontes de credencial aceitas, variáveis de ambiente das CLIs,
//! campos de opções e passos de prontidão) fica num [`DescritorProvider`], um
//! por tipo. O [`Registro`] padrão ([`todos`]) é a única lista de tipos que o
//! app conhece: a UI recebe os descritores pelo backend ([`ResumoDescritor`]) e
//! não mantém uma lista própria. Adicionar um tipo é implementar um descritor e
//! registrá-lo, seguindo o `docs/providers.md`, sem `match` por tipo fora dele.

mod catalogo;
mod descritores;
pub mod legado;
pub mod mascara;
pub mod prontidao;
mod resolvedor;

pub use descritores::{
    DescritorGithub, DescritorGitlab, DescritorJira, DescritorMysql, DescritorPostgres,
};

pub use catalogo::{ArquivoConexoes, Catalogo, conexao_referenciada};
pub use resolvedor::{ErroCredencial, Recusada, Resolvedor, com_nova_tentativa, texto_de_recusa};

use crate::{ParseEnumError, Secret};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;
use std::sync::OnceLock;

/// Uma conexão externa nomeada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Conexao {
    /// Identificador estável, referenciado pelos projetos (`gitlab-interno`).
    pub id: String,
    /// Nome que o analista vê ("GitLab interno", "Banco de homologação").
    pub nome: String,
    /// Tipo, igual ao [`DescritorProvider::tipo`] de um descritor registrado.
    pub tipo: String,
    /// Endereço no formato do descritor: host (`gitlab.example.com`), URL
    /// (`https://jira.example.com`) ou `host:porta/banco`.
    pub endereco: String,
    /// De onde vem a credencial; nunca o valor.
    #[serde(default)]
    pub credencial: FonteCredencial,
    /// Opções do tipo, com as chaves de [`DescritorProvider::campos_opcoes`].
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub opcoes: BTreeMap<String, String>,
}

impl Conexao {
    /// O valor da opção, ou o padrão do campo no descritor.
    pub fn opcao<'a>(
        &'a self,
        descritor: &'a dyn DescritorProvider,
        chave: &str,
    ) -> Option<&'a str> {
        if let Some(v) = self
            .opcoes
            .get(chave)
            .map(|v| v.trim())
            .filter(|v| !v.is_empty())
        {
            return Some(v);
        }
        descritor
            .campos_opcoes()
            .iter()
            .find(|c| c.chave == chave)
            .and_then(|c| c.padrao)
    }
}

/// De onde vem a credencial de uma conexão. O cadastro guarda só a fonte.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "fonte", rename_all = "snake_case")]
pub enum FonteCredencial {
    /// Conta no chaveiro do sistema (o padrão), no serviço do app.
    Chaveiro { conta: String },
    /// Comando que imprime a credencial na primeira linha do `stdout`, sem
    /// TTY e com prazo. Texto do cadastro, não segredo.
    Comando { cmd: String },
    /// A própria CLI do provider já autenticada (`gh auth token`).
    Cli,
    /// Arquivo cuja primeira linha é a credencial.
    Arquivo { caminho: String },
    /// Sem credencial (repositório público, banco sem senha).
    #[default]
    Nenhuma,
}

str_enum!(TipoFonte, "tipo_fonte", {
    Chaveiro => "chaveiro",
    Comando => "comando",
    Cli => "cli",
    Arquivo => "arquivo",
    Nenhuma => "nenhuma",
});

impl FonteCredencial {
    pub fn tipo(&self) -> TipoFonte {
        match self {
            FonteCredencial::Chaveiro { .. } => TipoFonte::Chaveiro,
            FonteCredencial::Comando { .. } => TipoFonte::Comando,
            FonteCredencial::Cli => TipoFonte::Cli,
            FonteCredencial::Arquivo { .. } => TipoFonte::Arquivo,
            FonteCredencial::Nenhuma => TipoFonte::Nenhuma,
        }
    }

    /// Confere os campos da fonte, sem ler a credencial.
    pub fn validar(&self) -> Result<(), String> {
        match self {
            FonteCredencial::Chaveiro { conta } if conta.trim().is_empty() => {
                Err("informe a conta do chaveiro".into())
            }
            FonteCredencial::Comando { cmd } if cmd.trim().is_empty() => {
                Err("informe o comando que imprime a credencial".into())
            }
            FonteCredencial::Arquivo { caminho } if caminho.trim().is_empty() => {
                Err("informe o arquivo da credencial".into())
            }
            _ => Ok(()),
        }
    }
}

impl TipoFonte {
    /// Nome da fonte na tela.
    pub fn rotulo(self) -> &'static str {
        match self {
            TipoFonte::Chaveiro => "Chaveiro do sistema",
            TipoFonte::Comando => "Comando que imprime a credencial",
            TipoFonte::Cli => "Login da própria CLI",
            TipoFonte::Arquivo => "Arquivo",
            TipoFonte::Nenhuma => "Sem credencial",
        }
    }
}

str_enum!(Capacidade, "capacidade", {
    Rastreador => "rastreador",
    Forja => "forja",
    Banco => "banco",
});

/// Valor de uma variável de ambiente que a CLI do provider lê.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValorVariavel {
    /// A credencial resolvida. O descritor só declara a variável: o valor é
    /// posto por [`aplicar_ambiente`], e só no ambiente do processo filho.
    Credencial,
    /// Texto do cadastro (host, arquivo de configuração).
    Texto(String),
}

/// Variável de ambiente para a CLI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariavelAmbiente {
    pub nome: &'static str,
    pub valor: ValorVariavel,
}

impl VariavelAmbiente {
    pub fn credencial(nome: &'static str) -> Self {
        VariavelAmbiente {
            nome,
            valor: ValorVariavel::Credencial,
        }
    }

    pub fn texto(nome: &'static str, valor: impl Into<String>) -> Self {
        VariavelAmbiente {
            nome,
            valor: ValorVariavel::Texto(valor.into()),
        }
    }
}

/// Tipo de um campo de opções, para a UI montar o formulário.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "tipo", rename_all = "snake_case")]
pub enum TipoCampo {
    Texto,
    /// Inteiro entre `min` e `max`.
    Numero {
        min: i64,
        max: i64,
    },
    /// Uma das opções.
    Escolha {
        opcoes: &'static [OpcaoEscolha],
    },
    /// `true` ou `false`.
    Booleano,
}

/// Uma opção de um campo de escolha.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct OpcaoEscolha {
    pub valor: &'static str,
    pub rotulo: &'static str,
}

/// Um campo de `opcoes` de um tipo de conexão.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CampoOpcao {
    pub chave: &'static str,
    pub rotulo: &'static str,
    /// Explicação curta abaixo do campo.
    pub ajuda: &'static str,
    #[serde(flatten)]
    pub tipo: TipoCampo,
    pub padrao: Option<&'static str>,
    pub obrigatorio: bool,
}

impl CampoOpcao {
    /// Confere um valor; vazio vale quando o campo não é obrigatório.
    pub fn validar(&self, valor: &str) -> Result<(), String> {
        let v = valor.trim();
        if v.is_empty() {
            return if self.obrigatorio && self.padrao.is_none() {
                Err(format!("informe {}", self.rotulo.to_lowercase()))
            } else {
                Ok(())
            };
        }
        match &self.tipo {
            TipoCampo::Texto => Ok(()),
            TipoCampo::Numero { min, max } => match v.parse::<i64>() {
                Ok(n) if (*min..=*max).contains(&n) => Ok(()),
                _ => Err(format!(
                    "{}: use um número inteiro entre {min} e {max}",
                    self.rotulo
                )),
            },
            TipoCampo::Escolha { opcoes } => {
                if opcoes.iter().any(|o| o.valor == v) {
                    Ok(())
                } else {
                    Err(format!("{}: opção inválida \"{v}\"", self.rotulo))
                }
            }
            TipoCampo::Booleano => match v {
                "true" | "false" => Ok(()),
                _ => Err(format!("{}: use true ou false", self.rotulo)),
            },
        }
    }
}

/// Um passo da prontidão de um tipo de conexão: o que é verificado e o que o
/// analista faz quando falha. A execução fica com o descritor
/// ([`DescritorProvider::executar_passo`], #437).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PassoProntidao {
    pub id: &'static str,
    pub rotulo: &'static str,
    pub orientacao: &'static str,
}

/// O que depende do tipo de uma conexão. Um por tipo; registrado no
/// [`Registro`].
pub trait DescritorProvider: Send + Sync {
    /// Identificador do tipo, gravado em [`Conexao::tipo`] (`gitlab`).
    fn tipo(&self) -> &'static str;
    /// Nome na tela (`GitLab`).
    fn rotulo(&self) -> &'static str;
    /// Nome do ícone na UI (`git-branch`, `database`).
    fn icone(&self) -> &'static str;
    fn capacidades(&self) -> &'static [Capacidade];
    /// Endereço sugerido numa conexão nova; vazio quando não há padrão.
    fn endereco_padrao(&self) -> &'static str;
    /// Dica do formato do endereço, para o campo da tela.
    fn formato_endereco(&self) -> &'static str;
    /// Confere e normaliza o endereço (sem barra final, host em minúsculas).
    fn normalizar_endereco(&self, endereco: &str) -> Result<String, String>;
    /// Fontes de credencial aceitas, a primeira é a padrão.
    fn fontes_aceitas(&self) -> &'static [TipoFonte];
    /// Variáveis de ambiente que a CLI do provider lê. Vazio para os tipos sem
    /// CLI (bancos, com driver nativo).
    fn variaveis_ambiente(&self, conexao: &Conexao) -> Vec<VariavelAmbiente>;
    /// Campos de `opcoes`.
    fn campos_opcoes(&self) -> &'static [CampoOpcao] {
        &[]
    }
    /// Passos de prontidão, na ordem em que rodam.
    fn passos_prontidao(&self) -> &'static [PassoProntidao];
    /// Executa o passo `passo` (um `id` de [`DescritorProvider::passos_prontidao`])
    /// e devolve o detalhe (versão, usuário, caminho), nunca a credencial.
    /// `None`: o app ainda não executa esse passo para o tipo, e ele aparece
    /// como pulado.
    fn executar_passo(
        &self,
        _passo: &str,
        _ctx: &mut prontidao::ContextoProntidao<'_>,
    ) -> Option<Result<Option<String>, String>> {
        None
    }
    /// Programa e argumentos que imprimem a credencial guardada pela própria CLI
    /// do provider (fonte `Cli`). `None` nos tipos que não aceitam `Cli`.
    fn credencial_pela_cli(&self, _conexao: &Conexao) -> Option<(&'static str, Vec<String>)> {
        None
    }
    /// Validação própria do tipo, além do endereço, da fonte e das opções.
    fn validar_extra(&self, _conexao: &Conexao) -> Result<(), String> {
        Ok(())
    }

    /// Fonte de credencial padrão de uma conexão nova.
    fn fonte_padrao(&self) -> TipoFonte {
        self.fontes_aceitas()
            .first()
            .copied()
            .unwrap_or(TipoFonte::Nenhuma)
    }

    fn tem(&self, capacidade: Capacidade) -> bool {
        self.capacidades().contains(&capacidade)
    }

    /// Confere a conexão sem rede: nome, endereço, fonte aceita e opções.
    fn validar(&self, conexao: &Conexao) -> Result<(), String> {
        if conexao.nome.trim().is_empty() {
            return Err("informe o nome da conexão".into());
        }
        self.normalizar_endereco(&conexao.endereco)?;
        let fonte = conexao.credencial.tipo();
        if !self.fontes_aceitas().contains(&fonte) {
            return Err(format!(
                "{} não aceita a credencial \"{}\"",
                self.rotulo(),
                fonte.rotulo()
            ));
        }
        conexao.credencial.validar()?;
        let campos = self.campos_opcoes();
        for chave in conexao.opcoes.keys() {
            if !campos.iter().any(|c| c.chave == chave) {
                return Err(format!(
                    "opção desconhecida para {}: {chave}",
                    self.rotulo()
                ));
            }
        }
        for c in campos {
            c.validar(conexao.opcoes.get(c.chave).map_or("", String::as_str))?;
        }
        self.validar_extra(conexao)
    }
}

impl fmt::Debug for dyn DescritorProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DescritorProvider({})", self.tipo())
    }
}

/// Põe as variáveis do descritor no ambiente do comando. A credencial só entra
/// aqui, nunca em argumento nem em URL.
pub fn aplicar_ambiente(
    cmd: &mut std::process::Command,
    descritor: &dyn DescritorProvider,
    conexao: &Conexao,
    credencial: Option<&Secret>,
) {
    for v in descritor.variaveis_ambiente(conexao) {
        match v.valor {
            ValorVariavel::Credencial => {
                if let Some(s) = credencial {
                    cmd.env(v.nome, s.expose());
                }
            }
            ValorVariavel::Texto(t) => {
                cmd.env(v.nome, t);
            }
        }
    }
}

/// O descritor como a UI recebe: tudo o que ela precisa para listar os tipos e
/// montar o formulário, sem literais de tipo no front.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResumoDescritor {
    pub tipo: &'static str,
    pub rotulo: &'static str,
    pub icone: &'static str,
    pub capacidades: Vec<Capacidade>,
    pub endereco_padrao: &'static str,
    pub formato_endereco: &'static str,
    pub fontes_aceitas: Vec<ResumoFonte>,
    pub fonte_padrao: TipoFonte,
    pub campos_opcoes: Vec<CampoOpcao>,
    pub passos_prontidao: Vec<PassoProntidao>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResumoFonte {
    pub fonte: TipoFonte,
    pub rotulo: &'static str,
}

impl ResumoDescritor {
    pub fn de(d: &dyn DescritorProvider) -> Self {
        ResumoDescritor {
            tipo: d.tipo(),
            rotulo: d.rotulo(),
            icone: d.icone(),
            capacidades: d.capacidades().to_vec(),
            endereco_padrao: d.endereco_padrao(),
            formato_endereco: d.formato_endereco(),
            fontes_aceitas: d
                .fontes_aceitas()
                .iter()
                .map(|&fonte| ResumoFonte {
                    fonte,
                    rotulo: fonte.rotulo(),
                })
                .collect(),
            fonte_padrao: d.fonte_padrao(),
            campos_opcoes: d.campos_opcoes().to_vec(),
            passos_prontidao: d.passos_prontidao().to_vec(),
        }
    }
}

/// Os descritores conhecidos, um por tipo.
#[derive(Debug, Default)]
pub struct Registro {
    descritores: Vec<Box<dyn DescritorProvider>>,
}

impl Registro {
    /// O registro do app: GitHub, GitLab, Jira, PostgreSQL e MySQL/MariaDB.
    pub fn padrao() -> Self {
        Registro::default()
            .com(DescritorGithub)
            .com(DescritorGitlab)
            .com(DescritorJira)
            .com(DescritorPostgres)
            .com(DescritorMysql)
    }

    /// Acrescenta um descritor. Um tipo repetido é erro de programação.
    pub fn com(mut self, d: impl DescritorProvider + 'static) -> Self {
        assert!(
            self.get(d.tipo()).is_none(),
            "tipo de conexão repetido: {}",
            d.tipo()
        );
        self.descritores.push(Box::new(d));
        self
    }

    pub fn get(&self, tipo: &str) -> Option<&dyn DescritorProvider> {
        self.descritores
            .iter()
            .find(|d| d.tipo() == tipo)
            .map(Box::as_ref)
    }

    /// O descritor da conexão, ou erro se o tipo não é conhecido.
    pub fn de(&self, conexao: &Conexao) -> Result<&dyn DescritorProvider, String> {
        self.get(&conexao.tipo)
            .ok_or_else(|| format!("tipo de conexão desconhecido: {}", conexao.tipo))
    }

    pub fn iter(&self) -> impl Iterator<Item = &dyn DescritorProvider> {
        self.descritores.iter().map(Box::as_ref)
    }

    /// Os tipos com a capacidade (`Banco` → postgres, mysql).
    pub fn com_capacidade(&self, c: Capacidade) -> impl Iterator<Item = &dyn DescritorProvider> {
        self.iter().filter(move |d| d.tem(c))
    }

    pub fn resumos(&self) -> Vec<ResumoDescritor> {
        self.iter().map(ResumoDescritor::de).collect()
    }

    /// Confere uma conexão pelo descritor do tipo dela.
    pub fn validar(&self, conexao: &Conexao) -> Result<(), String> {
        if conexao.id.trim().is_empty() {
            return Err("a conexão não tem identificador".into());
        }
        self.de(conexao)?.validar(conexao)
    }

    /// Confere uma lista de conexões: cada uma, e id e nome sem repetição.
    pub fn validar_lista(&self, conexoes: &[Conexao]) -> Result<(), String> {
        let mut ids = std::collections::HashSet::new();
        let mut nomes = std::collections::HashSet::new();
        for c in conexoes {
            self.validar(c)
                .map_err(|e| format!("conexão \"{}\": {e}", c.nome.trim()))?;
            if !ids.insert(c.id.trim()) {
                return Err(format!(
                    "identificador de conexão repetido: {}",
                    c.id.trim()
                ));
            }
            if !nomes.insert(c.nome.trim().to_lowercase()) {
                return Err(format!(
                    "já existe uma conexão chamada \"{}\"",
                    c.nome.trim()
                ));
            }
        }
        Ok(())
    }
}

/// O registro do app, a única lista de tipos de conexão.
pub fn todos() -> &'static Registro {
    static REGISTRO: OnceLock<Registro> = OnceLock::new();
    REGISTRO.get_or_init(Registro::padrao)
}

/// Host de um endereço que pode vir com esquema, usuário, porta e caminho
/// (`https://u@Host:8443/x` → `host`).
pub fn host_de(endereco: &str) -> String {
    let u = endereco.trim();
    let resto = u.split_once("://").map_or(u, |(_, r)| r);
    let autoridade = resto.split('/').next().unwrap_or("");
    let autoridade = autoridade.rsplit_once('@').map_or(autoridade, |(_, h)| h);
    autoridade
        .split(':')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
}

/// Um host válido: letras, dígitos, `.` e `-`, sem espaços.
pub(crate) fn host_valido(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 253
        && host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
        && !host.starts_with(['.', '-'])
        && !host.ends_with(['.', '-'])
}

#[cfg(test)]
mod tests;
