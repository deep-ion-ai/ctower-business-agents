//! Acesso só de leitura a bancos PostgreSQL e MySQL/MariaDB (#439).
//!
//! Uma conexão de banco é uma [`Conexao`] de um tipo com a capacidade `Banco`
//! (`postgres`, `mysql`): o descritor dá o dialeto, o endereço `host:porta/banco`
//! e as opções (usuário, esquema, tempo limite, limite de linhas, TLS,
//! ambiente); a senha vem do resolvedor de credenciais. [`ConfigBanco`] junta
//! tudo isso, e as operações abrem uma sessão por vez, com driver nativo
//! (`sqlx`), sem cliente instalado na máquina:
//!
//! - [`consultar`]: uma consulta validada ([`validacao`]) numa sessão só de
//!   leitura, com tempo limite e limite de linhas;
//! - [`testar`]: os passos de prontidão do descritor (conectar, versão, só
//!   leitura, esquemas);
//! - [`ler_schema`]: tabelas, colunas, tipos e chaves do banco.
//!
//! **Só leitura imposta pelo app**, além do que o usuário do banco permite:
//! no PostgreSQL, a sessão abre com `default_transaction_read_only = on` e
//! `statement_timeout`, e cada consulta roda em `BEGIN READ ONLY … ROLLBACK`; no
//! MySQL/MariaDB, `SET SESSION TRANSACTION READ ONLY` e `max_execution_time`
//! (`max_statement_time` no MariaDB), com cada consulta em
//! `START TRANSACTION READ ONLY … ROLLBACK`.
//!
//! **Sigilo:** a senha só existe dentro do [`Secret`] e das opções do driver. O
//! `Debug` da configuração não a mostra, ela é registrada no mascaramento do
//! processo e toda mensagem de erro passa pelo mascaramento antes de sair. O app
//! nunca monta uma string de conexão.

mod conectado;
mod introspeccao;
pub mod prontidao;
mod sessao;

use std::fmt;
use std::time::Duration;

use cba_repos::Secret;
use cba_repos::chaveiro::Chaveiro;
use cba_repos::providers::{
    self, Capacidade, Conexao, DescritorProvider, DialetoSql, Resolvedor, mascara,
};
use serde::Serialize;

/// A validação da instrução (só uma, e só leitura) fica na crate `cba-sql`, sem o
/// driver, para as ferramentas também validarem o SQL que o agente escreve no
/// documento (#453).
pub use cba_sql::validacao;
pub use conectado::{BancoDaConexao, FonteDoCatalogo};
pub use introspeccao::ler_schema;
pub use sessao::{ColunaResultado, ResultadoConsulta, consultar, testar};
pub use validacao::{Recusa, validar};

/// Tempo para abrir a conexão (resolver o host, TLS e autenticar).
const PRAZO_CONEXAO: Duration = Duration::from_secs(15);

/// Configuração de acesso a um banco, montada da conexão cadastrada.
#[derive(Clone)]
pub struct ConfigBanco {
    pub dialeto: DialetoSql,
    pub host: String,
    pub porta: u16,
    pub banco: String,
    pub usuario: String,
    senha: Option<Secret>,
    /// Esquema padrão (`search_path` no PostgreSQL, `USE` no MySQL).
    pub esquema: Option<String>,
    pub tempo_limite: Duration,
    pub limite_linhas: usize,
    pub tls: Tls,
    /// A conexão, para a origem dos resultados (sem endereço nem usuário).
    pub origem: cba_core::banco::OrigemBanco,
}

impl fmt::Debug for ConfigBanco {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConfigBanco")
            .field("dialeto", &self.dialeto)
            .field("host", &self.host)
            .field("porta", &self.porta)
            .field("banco", &self.banco)
            .field("usuario", &self.usuario)
            .field("senha", &self.senha.as_ref().map(|_| "***"))
            .field("esquema", &self.esquema)
            .field("tempo_limite", &self.tempo_limite)
            .field("limite_linhas", &self.limite_linhas)
            .field("tls", &self.tls)
            .finish()
    }
}

/// Modo de TLS da opção `tls` do descritor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tls {
    Exigir,
    Preferir,
    Desligar,
}

/// Por que a operação no banco falhou. As mensagens já saem mascaradas: sem
/// senha e sem string de conexão.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ErroBanco {
    #[error("conexão de banco inválida: {0}")]
    Configuracao(String),
    #[error("não foi possível obter a senha do banco: {0}")]
    Credencial(String),
    #[error("não foi possível conectar ao banco: {0}")]
    Conexao(String),
    #[error("consulta recusada pelo app: {0}")]
    Recusada(#[from] Recusa),
    #[error("o banco recusou a consulta: {0}")]
    Banco(String),
    #[error("a consulta passou do tempo limite de {0} s e foi cancelada")]
    TempoEsgotado(u64),
}

impl ErroBanco {
    /// Nome curto, para o log.
    pub fn tipo(&self) -> &'static str {
        match self {
            Self::Configuracao(_) => "configuracao",
            Self::Credencial(_) => "credencial",
            Self::Conexao(_) => "conexao",
            Self::Recusada(_) => "recusada",
            Self::Banco(_) => "banco",
            Self::TempoEsgotado(_) => "tempo_esgotado",
        }
    }
}

/// Resultado de um passo de prontidão: `Ok(detalhe)` ou `Err(motivo)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResultadoPasso {
    /// `id` do [`cba_repos::providers::PassoProntidao`] do descritor.
    pub id: &'static str,
    pub ok: bool,
    pub detalhe: String,
}

fn numero(conexao: &Conexao, d: &dyn DescritorProvider, chave: &str) -> Result<u64, ErroBanco> {
    let v = conexao.opcao(d, chave).unwrap_or_default();
    v.parse()
        .map_err(|_| ErroBanco::Configuracao(format!("a opção `{chave}` não é um número: {v}")))
}

/// Identificador simples (esquema): letras, dígitos, `_` e `$`. O esquema vai
/// para as opções da sessão e para `USE`, então nada além disso é aceito.
fn identificador_simples(nome: &str) -> bool {
    !nome.is_empty()
        && nome.len() <= 63
        && nome
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '$')
}

impl ConfigBanco {
    /// A configuração da conexão `conexao`, com a senha já resolvida.
    pub fn da_conexao(conexao: &Conexao, senha: Option<Secret>) -> Result<Self, ErroBanco> {
        let registro = providers::todos();
        let d = registro.get(&conexao.tipo).ok_or_else(|| {
            ErroBanco::Configuracao(format!("tipo de conexão desconhecido: {}", conexao.tipo))
        })?;
        if !d.tem(Capacidade::Banco) {
            return Err(ErroBanco::Configuracao(format!(
                "a conexão \"{}\" não é de banco ({})",
                conexao.nome,
                d.rotulo()
            )));
        }
        let dialeto = d
            .dialeto_sql()
            .ok_or_else(|| ErroBanco::Configuracao(format!("{} sem dialeto SQL", d.rotulo())))?;
        registro.validar(conexao).map_err(ErroBanco::Configuracao)?;
        let endereco = d
            .normalizar_endereco(&conexao.endereco)
            .map_err(ErroBanco::Configuracao)?;
        let (autoridade, banco) = endereco
            .split_once('/')
            .ok_or_else(|| ErroBanco::Configuracao("endereço sem o banco".into()))?;
        let (host, porta) = autoridade
            .rsplit_once(':')
            .ok_or_else(|| ErroBanco::Configuracao("endereço sem a porta".into()))?;
        let porta = porta
            .parse()
            .map_err(|_| ErroBanco::Configuracao(format!("porta inválida: {porta}")))?;
        let usuario = conexao
            .opcao(d, "usuario")
            .ok_or_else(|| ErroBanco::Configuracao("informe o usuário do banco".into()))?
            .to_owned();
        let esquema = conexao.opcao(d, "esquema").map(str::to_owned);
        if let Some(e) = &esquema
            && !identificador_simples(e)
        {
            return Err(ErroBanco::Configuracao(format!(
                "esquema inválido: use só letras, números e `_` ({e})"
            )));
        }
        let tls = match conexao.opcao(d, "tls").unwrap_or("preferir") {
            "exigir" => Tls::Exigir,
            "desligar" => Tls::Desligar,
            _ => Tls::Preferir,
        };
        if let Some(s) = &senha {
            mascara::registrar(s);
        }
        Ok(Self {
            dialeto,
            host: host.to_owned(),
            porta,
            banco: banco.to_owned(),
            usuario,
            senha,
            esquema,
            tempo_limite: Duration::from_secs(numero(conexao, d, "tempo_limite_s")?.max(1)),
            limite_linhas: numero(conexao, d, "limite_linhas")?.max(1) as usize,
            tls,
            origem: cba_core::banco::OrigemBanco {
                conexao: conexao.id.clone(),
                nome: conexao.nome.clone(),
                tipo: d.rotulo().to_owned(),
                ambiente: conexao.opcao(d, "ambiente").map(str::to_owned),
            },
        })
    }

    /// A configuração da conexão, com a senha lida pelo resolvedor do processo.
    pub fn resolver(conexao: &Conexao, chaveiro: &dyn Chaveiro) -> Result<Self, ErroBanco> {
        let senha = Resolvedor::global()
            .resolver(conexao, chaveiro, None)
            .map_err(|e| ErroBanco::Credencial(mascara::mascarar(&e.to_string())))?;
        Self::da_conexao(conexao, senha)
    }

    /// Texto sem a senha (e sem qualquer credencial registrada no processo).
    fn mascarar(&self, texto: &str) -> String {
        let texto = match &self.senha {
            Some(s) => s.redact(texto),
            None => texto.to_owned(),
        };
        mascara::mascarar(&texto)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cba_repos::providers::FonteCredencial;
    use std::collections::BTreeMap;

    fn conexao(tipo: &str, endereco: &str) -> Conexao {
        let mut opcoes = BTreeMap::new();
        opcoes.insert("usuario".to_owned(), "leitura".to_owned());
        Conexao {
            id: "banco-hml".into(),
            nome: "Banco de homologação".into(),
            tipo: tipo.into(),
            endereco: endereco.into(),
            credencial: FonteCredencial::Nenhuma,
            opcoes,
        }
    }

    #[test]
    fn monta_da_conexao_com_os_padroes() {
        let c = ConfigBanco::da_conexao(&conexao("postgres", "DB.example.com/fat"), None).unwrap();
        assert_eq!(c.dialeto, DialetoSql::Postgres);
        assert_eq!(
            (c.host.as_str(), c.porta, c.banco.as_str()),
            ("db.example.com", 5432, "fat")
        );
        assert_eq!(c.usuario, "leitura");
        assert_eq!(c.tempo_limite, Duration::from_secs(30));
        assert_eq!(c.limite_linhas, 200);
        assert_eq!(c.tls, Tls::Preferir);
        assert_eq!(c.origem.tipo, "PostgreSQL");

        let mut m = conexao("mysql", "db:3307/fat");
        m.opcoes.insert("tempo_limite_s".into(), "5".into());
        m.opcoes.insert("limite_linhas".into(), "10".into());
        m.opcoes.insert("tls".into(), "exigir".into());
        m.opcoes.insert("ambiente".into(), "sustentação".into());
        let c = ConfigBanco::da_conexao(&m, None).unwrap();
        assert_eq!((c.dialeto, c.porta), (DialetoSql::Mysql, 3307));
        assert_eq!((c.tempo_limite.as_secs(), c.limite_linhas), (5, 10));
        assert_eq!(c.tls, Tls::Exigir);
        assert_eq!(
            c.origem.to_string(),
            "Banco de homologação (MySQL/MariaDB, sustentação)"
        );
    }

    #[test]
    fn recusa_conexao_que_nao_e_de_banco_e_esquema_estranho() {
        let e = ConfigBanco::da_conexao(&conexao("github", "github.com"), None).unwrap_err();
        assert!(matches!(e, ErroBanco::Configuracao(_)), "{e}");
        let mut c = conexao("postgres", "db/fat");
        c.opcoes.insert("esquema".into(), "app; drop".into());
        let e = ConfigBanco::da_conexao(&c, None).unwrap_err();
        assert!(e.to_string().contains("esquema inválido"), "{e}");
    }

    #[test]
    fn debug_e_erros_sem_a_senha() {
        let senha = Secret::new("senha-super-secreta-439");
        let c = ConfigBanco::da_conexao(&conexao("postgres", "db/fat"), Some(senha)).unwrap();
        let debug = format!("{c:?}");
        assert!(!debug.contains("senha-super-secreta-439"), "{debug}");
        assert!(debug.contains("***"));
        let m = c.mascarar("falhou: password=senha-super-secreta-439");
        assert!(!m.contains("senha-super-secreta-439"), "{m}");
    }
}
