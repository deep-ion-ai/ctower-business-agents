//! Consultas ao banco numa análise (#440): o banco ligado pelo analista, cada
//! consulta com a aprovação dele e a evidência de dados que o agente cita.
//!
//! ```text
//! Consulta:  pendente ──► aprovada ──► executada | falhou
//!               │
//!               ├──► recusada (com o motivo, que volta ao agente)
//!               └──► expirada (o analista não respondeu a tempo)
//! ```
//!
//! O estado guarda o registro de **todas** as consultas, aprovadas ou recusadas:
//! quem decidiu, quando, o SQL e a quantidade de linhas. Os valores do resultado
//! nunca são gravados: só os nomes das colunas e quais foram mascaradas.
//!
//! A evidência de dados ([`EvidenciaDados`]) é o resultado de uma consulta
//! executada, citado pelo agente numa afirmação, com o resumo que ele escreveu.
//! Ela vale para a data em que foi tirada: a revalidação não reexecuta consultas.

use serde::{Deserialize, Serialize};

use crate::analise::{Analise, ErroModelo};
use crate::banco::OrigemBanco;

/// Tamanho máximo do SQL de uma consulta.
pub const MAX_SQL: usize = 20_000;

/// Banco ligado à análise pelo analista ("Usar o banco <nome> nesta análise").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BancoLigado {
    /// `id` da conexão de banco do projeto.
    pub conexao: String,
    /// Nome da conexão quando foi ligada, para a tela e o registro.
    pub nome: String,
    pub ligado_ms: u64,
    /// Quem ligou, quando conhecido.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub por: Option<String>,
}

/// Situação de uma consulta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SituacaoConsulta {
    /// Aguardando a aprovação do analista.
    Pendente,
    /// Aprovada (com ou sem edição), a caminho do banco.
    Aprovada,
    Recusada,
    Executada,
    /// O banco (ou a validação do app) recusou a consulta aprovada.
    Falhou,
    /// O analista não respondeu dentro do prazo.
    Expirada,
}

/// Decisão do analista sobre uma consulta pendente.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "decisao", rename_all = "snake_case")]
pub enum DecisaoConsulta {
    Executar,
    /// Executar o SQL editado pelo analista no lugar do proposto.
    EditarEExecutar {
        sql: String,
    },
    /// Recusar; o motivo volta ao agente.
    Recusar {
        motivo: String,
    },
}

/// Uma consulta pedida pelo agente (`consultar_dados`), com a decisão do analista
/// e o resumo da execução, sem os valores.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsultaDados {
    pub numero: u32,
    /// O que o agente quer saber ("medir quantas faturas estão na mesma situação").
    pub objetivo: String,
    /// Conexão e ambiente em que roda.
    pub origem: OrigemBanco,
    /// O SQL que o agente propôs.
    pub sql_proposto: String,
    /// O SQL aprovado: o proposto ou o editado pelo analista.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sql_aprovado: Option<String>,
    pub situacao: SituacaoConsulta,
    pub pedida_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decidida_ms: Option<u64>,
    /// Quem aprovou ou recusou.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decidida_por: Option<String>,
    /// Motivo da recusa.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motivo: Option<String>,
    #[serde(default)]
    pub editada: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub executada_ms: Option<u64>,
    /// Linhas devolvidas (até o limite da conexão).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub linhas: Option<usize>,
    /// Havia mais linhas que o limite da conexão.
    #[serde(default)]
    pub cortada: bool,
    /// Nomes das colunas do resultado (nunca os valores).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub colunas: Vec<String>,
    /// Colunas mascaradas antes de o resultado chegar ao agente.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mascaradas: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duracao_ms: Option<u64>,
    /// Por que a execução falhou (já mascarado, sem senha).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub erro: Option<String>,
}

impl ConsultaDados {
    /// O SQL que vale: o aprovado ou, antes da aprovação, o proposto.
    pub fn sql(&self) -> &str {
        self.sql_aprovado.as_deref().unwrap_or(&self.sql_proposto)
    }
}

/// Resumo de uma execução, sem os valores.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecucaoConsulta {
    pub linhas: usize,
    pub cortada: bool,
    pub colunas: Vec<String>,
    pub mascaradas: Vec<String>,
    pub duracao_ms: u64,
}

/// Resultado de uma consulta citado numa afirmação: a evidência do tipo dados,
/// separada da evidência de código.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenciaDados {
    /// Número da consulta na análise.
    pub consulta: u32,
    pub origem: OrigemBanco,
    pub sql: String,
    pub executada_ms: u64,
    pub linhas: usize,
    /// O que o resultado mostra, escrito pelo agente ("312 faturas vencidas com
    /// multa zerada").
    pub resumo: String,
}

const CONSULTA: &str = "consulta";

impl Analise {
    /// Liga o banco à análise ("Usar o banco <nome> nesta análise"): só então a
    /// ferramenta `consultar_dados` aparece para o agente.
    pub fn ligar_banco(
        &mut self,
        conexao: &str,
        nome: &str,
        por: Option<String>,
        agora_ms: u64,
    ) -> Result<(), ErroModelo> {
        if conexao.trim().is_empty() {
            return Err(ErroModelo::CampoVazio("conexao"));
        }
        if self.estado == crate::analise::EstadoAnalise::Aprovada {
            return Err(ErroModelo::AnaliseAprovada(self.id.clone()));
        }
        self.banco = Some(BancoLigado {
            conexao: conexao.trim().to_owned(),
            nome: nome.trim().to_owned(),
            ligado_ms: agora_ms,
            por,
        });
        Ok(())
    }

    /// Desliga o banco: a ferramenta some da sessão e as consultas pendentes
    /// expiram (o agente recebe a recusa).
    pub fn desligar_banco(&mut self, agora_ms: u64) {
        self.banco = None;
        for c in &mut self.consultas {
            if c.situacao == SituacaoConsulta::Pendente {
                c.situacao = SituacaoConsulta::Expirada;
                c.decidida_ms = Some(agora_ms);
                c.motivo = Some("o banco foi desligado desta análise".into());
            }
        }
    }

    /// Registra o pedido de uma consulta, pendente da aprovação do analista.
    pub fn pedir_consulta(
        &mut self,
        objetivo: &str,
        sql: &str,
        origem: OrigemBanco,
        agora_ms: u64,
    ) -> Result<u32, ErroModelo> {
        if objetivo.trim().is_empty() {
            return Err(ErroModelo::CampoVazio("objetivo"));
        }
        if sql.trim().is_empty() {
            return Err(ErroModelo::CampoVazio("sql"));
        }
        match &self.banco {
            Some(b) if b.conexao == origem.conexao => {}
            _ => {
                return Err(ErroModelo::ConsultaRecusada(
                    "o banco não está ligado nesta análise: o analista precisa marcar \
                     \"Usar o banco nesta análise\" no app; siga só com o código e deixe \
                     o SQL pronto no documento"
                        .into(),
                ));
            }
        }
        self.aceitar_registro()?;
        let numero = self.consultas.iter().map(|c| c.numero).max().unwrap_or(0) + 1;
        self.consultas.push(ConsultaDados {
            numero,
            objetivo: objetivo.trim().to_owned(),
            origem,
            sql_proposto: sql.trim().to_owned(),
            sql_aprovado: None,
            situacao: SituacaoConsulta::Pendente,
            pedida_ms: agora_ms,
            decidida_ms: None,
            decidida_por: None,
            motivo: None,
            editada: false,
            executada_ms: None,
            linhas: None,
            cortada: false,
            colunas: Vec::new(),
            mascaradas: Vec::new(),
            duracao_ms: None,
            erro: None,
        });
        Ok(numero)
    }

    pub fn consulta(&self, numero: u32) -> Option<&ConsultaDados> {
        self.consultas.iter().find(|c| c.numero == numero)
    }

    fn consulta_mut(&mut self, numero: u32) -> Result<&mut ConsultaDados, ErroModelo> {
        self.consultas
            .iter_mut()
            .find(|c| c.numero == numero)
            .ok_or(ErroModelo::ItemDesconhecido {
                tipo: CONSULTA,
                numero,
            })
    }

    /// Decisão do analista sobre uma consulta pendente.
    pub fn decidir_consulta(
        &mut self,
        numero: u32,
        decisao: DecisaoConsulta,
        por: Option<String>,
        agora_ms: u64,
    ) -> Result<(), ErroModelo> {
        let c = self.consulta_mut(numero)?;
        if c.situacao != SituacaoConsulta::Pendente {
            return Err(ErroModelo::ItemDecidido {
                tipo: CONSULTA,
                numero,
            });
        }
        match decisao {
            DecisaoConsulta::Executar => {
                c.sql_aprovado = Some(c.sql_proposto.clone());
                c.situacao = SituacaoConsulta::Aprovada;
            }
            DecisaoConsulta::EditarEExecutar { sql } => {
                let sql = sql.trim();
                if sql.is_empty() {
                    return Err(ErroModelo::CampoVazio("sql"));
                }
                if sql.len() > MAX_SQL {
                    return Err(ErroModelo::ConsultaRecusada(format!(
                        "o SQL passa de {MAX_SQL} caracteres"
                    )));
                }
                c.editada = sql != c.sql_proposto;
                c.sql_aprovado = Some(sql.to_owned());
                c.situacao = SituacaoConsulta::Aprovada;
            }
            DecisaoConsulta::Recusar { motivo } => {
                if motivo.trim().is_empty() {
                    return Err(ErroModelo::CampoVazio("motivo"));
                }
                c.motivo = Some(motivo.trim().to_owned());
                c.situacao = SituacaoConsulta::Recusada;
            }
        }
        c.decidida_ms = Some(agora_ms);
        c.decidida_por = por;
        Ok(())
    }

    /// Resultado da execução de uma consulta aprovada: o resumo (sem valores) ou
    /// o erro.
    pub fn registrar_execucao(
        &mut self,
        numero: u32,
        resultado: Result<ExecucaoConsulta, String>,
        agora_ms: u64,
    ) -> Result<(), ErroModelo> {
        let c = self.consulta_mut(numero)?;
        if c.situacao != SituacaoConsulta::Aprovada {
            return Err(ErroModelo::ItemDecidido {
                tipo: CONSULTA,
                numero,
            });
        }
        c.executada_ms = Some(agora_ms);
        match resultado {
            Ok(e) => {
                c.situacao = SituacaoConsulta::Executada;
                c.linhas = Some(e.linhas);
                c.cortada = e.cortada;
                c.colunas = e.colunas;
                c.mascaradas = e.mascaradas;
                c.duracao_ms = Some(e.duracao_ms);
            }
            Err(erro) => {
                c.situacao = SituacaoConsulta::Falhou;
                c.erro = Some(erro);
            }
        }
        Ok(())
    }

    /// O analista não respondeu a tempo: a consulta pendente expira.
    pub fn expirar_consulta(&mut self, numero: u32, agora_ms: u64) -> Result<(), ErroModelo> {
        let c = self.consulta_mut(numero)?;
        if c.situacao == SituacaoConsulta::Pendente {
            c.situacao = SituacaoConsulta::Expirada;
            c.decidida_ms = Some(agora_ms);
        }
        Ok(())
    }

    /// Consultas aguardando a aprovação do analista.
    pub fn consultas_pendentes(&self) -> impl Iterator<Item = &ConsultaDados> {
        self.consultas
            .iter()
            .filter(|c| c.situacao == SituacaoConsulta::Pendente)
    }

    /// A evidência de dados de uma consulta executada, com o resumo do agente.
    pub fn evidencia_de_dados(
        &self,
        numero: u32,
        resumo: &str,
    ) -> Result<EvidenciaDados, ErroModelo> {
        if resumo.trim().is_empty() {
            return Err(ErroModelo::CampoVazio("resumo"));
        }
        let c = self.consulta(numero).ok_or(ErroModelo::ItemDesconhecido {
            tipo: CONSULTA,
            numero,
        })?;
        match (c.situacao, c.executada_ms, c.linhas) {
            (SituacaoConsulta::Executada, Some(executada_ms), Some(linhas)) => Ok(EvidenciaDados {
                consulta: numero,
                origem: c.origem.clone(),
                sql: c.sql().to_owned(),
                executada_ms,
                linhas,
                resumo: resumo.trim().to_owned(),
            }),
            _ => Err(ErroModelo::ConsultaRecusada(format!(
                "a consulta {numero} não foi executada (situação: {:?}); só o resultado \
                 de uma consulta executada vira evidência de dados",
                c.situacao
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analise::{Confianca, Fluxo, NovaAfirmacao};
    use cba_kb::Serie;

    fn origem() -> OrigemBanco {
        OrigemBanco {
            conexao: "banco-hml".into(),
            nome: "Banco de homologação".into(),
            tipo: "PostgreSQL".into(),
            ambiente: Some("homologação".into()),
        }
    }

    fn analise() -> Analise {
        Analise::nova(
            Serie::Analise.id(1),
            "faturamento-exemplo",
            Fluxo::Impacto,
            "Multa",
            "Multa zerada",
            1,
        )
    }

    #[test]
    fn sem_banco_ligado_recusa_o_pedido() {
        let mut a = analise();
        let e = a
            .pedir_consulta("medir", "SELECT 1", origem(), 2)
            .unwrap_err();
        assert!(e.to_string().contains("não está ligado"), "{e}");
        a.ligar_banco("outro", "Outro", None, 2).unwrap();
        assert!(a.pedir_consulta("medir", "SELECT 1", origem(), 2).is_err());
    }

    #[test]
    fn aprovar_editar_recusar_e_executar() {
        let mut a = analise();
        a.ligar_banco("banco-hml", "Banco de homologação", Some("ana".into()), 2)
            .unwrap();
        let n1 = a
            .pedir_consulta("medir", "SELECT count(*) FROM fatura", origem(), 3)
            .unwrap();
        let n2 = a
            .pedir_consulta("ver", "SELECT * FROM cliente", origem(), 3)
            .unwrap();
        let n3 = a.pedir_consulta("ver", "SELECT 2", origem(), 3).unwrap();
        assert_eq!((n1, n2, n3), (1, 2, 3));
        assert_eq!(a.consultas_pendentes().count(), 3);

        a.decidir_consulta(n1, DecisaoConsulta::Executar, Some("ana".into()), 4)
            .unwrap();
        assert_eq!(a.consulta(n1).unwrap().sql(), "SELECT count(*) FROM fatura");
        assert!(!a.consulta(n1).unwrap().editada);

        let vazio = DecisaoConsulta::Recusar { motivo: " ".into() };
        assert!(a.decidir_consulta(n2, vazio, None, 4).is_err());
        a.decidir_consulta(
            n2,
            DecisaoConsulta::Recusar {
                motivo: "traz linhas inteiras de clientes; use COUNT".into(),
            },
            Some("ana".into()),
            4,
        )
        .unwrap();
        assert_eq!(a.consulta(n2).unwrap().situacao, SituacaoConsulta::Recusada);
        assert!(
            a.decidir_consulta(n2, DecisaoConsulta::Executar, None, 5)
                .is_err()
        );

        a.decidir_consulta(
            n3,
            DecisaoConsulta::EditarEExecutar {
                sql: "SELECT 3".into(),
            },
            Some("ana".into()),
            4,
        )
        .unwrap();
        let c3 = a.consulta(n3).unwrap();
        assert!(c3.editada);
        assert_eq!(
            (c3.sql_proposto.as_str(), c3.sql()),
            ("SELECT 2", "SELECT 3")
        );

        // Sem execução, não há evidência.
        assert!(a.evidencia_de_dados(n1, "312 faturas").is_err());
        a.registrar_execucao(
            n1,
            Ok(ExecucaoConsulta {
                linhas: 1,
                cortada: false,
                colunas: vec!["count".into()],
                mascaradas: vec![],
                duracao_ms: 12,
            }),
            6,
        )
        .unwrap();
        a.registrar_execucao(n3, Err("o banco recusou".into()), 6)
            .unwrap();
        assert_eq!(a.consulta(n3).unwrap().situacao, SituacaoConsulta::Falhou);
        let ev = a.evidencia_de_dados(n1, " 312 faturas ").unwrap();
        assert_eq!((ev.consulta, ev.linhas, ev.executada_ms), (1, 1, 6));
        assert_eq!(ev.resumo, "312 faturas");

        // A afirmação com evidência de dados pode ser confirmada sem trecho de código.
        let n = a
            .registrar_afirmacao_com_dados(
                NovaAfirmacao {
                    secao: "abrangencia".into(),
                    texto: "Média: 312 faturas afetadas.".into(),
                    confianca: Confianca::Confirmado,
                    evidencias: vec![],
                    sessao: None,
                },
                vec![ev],
                7,
            )
            .unwrap();
        assert_eq!(a.afirmacoes[(n - 1) as usize].dados.len(), 1);
    }

    #[test]
    fn desligar_expira_as_pendentes_e_estado_sobrevive_ao_json() {
        let mut a = analise();
        a.ligar_banco("banco-hml", "Banco", None, 2).unwrap();
        a.pedir_consulta("medir", "SELECT 1", origem(), 3).unwrap();
        let json = serde_json::to_string(&a).unwrap();
        let lida: Analise = serde_json::from_str(&json).unwrap();
        assert_eq!(lida, a);
        a.desligar_banco(4);
        assert!(a.banco.is_none());
        assert_eq!(a.consultas[0].situacao, SituacaoConsulta::Expirada);
    }
}
