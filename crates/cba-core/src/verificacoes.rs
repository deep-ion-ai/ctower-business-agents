//! Verificações em dados de uma análise de impacto (#453): as consultas que
//! confirmam o diagnóstico e medem a abrangência, escritas no documento para quem
//! tem acesso ao banco rodar.
//!
//! Sem acesso ao banco, a análise devolve as perguntas de verificação ao
//! solicitante ("existe tal registro?", "quantos casos estão nessa situação?").
//! A verificação leva a consulta pronta: o objetivo, o SQL só de leitura, os
//! parâmetros nomeados (o identificador do caso, nunca um dado real) e o que
//! cada resultado significa para a análise. Uma pergunta (`perguntar`) pode
//! apontar para a verificação que a responde.
//!
//! O SQL é conferido por quem registra (`registrar_verificacao`, em
//! `cba-tools`): só leitura, uma instrução, tabelas e colunas do modelo de dados
//! conhecido e os parâmetros declarados. Aqui ficam o registro e as regras que
//! não dependem do SQL. Executar a consulta é outra coisa: com o banco ligado à
//! análise, `consultar_dados` ([`crate::consultas`]) roda a mesma consulta, com a
//! aprovação do analista.

use serde::{Deserialize, Serialize};

use crate::analise::{Analise, ErroModelo, Fluxo};

/// Quantos resultados com significado uma verificação explica, no máximo.
pub const MAX_RESULTADOS: usize = 6;
/// Quantos parâmetros uma verificação declara, no máximo.
pub const MAX_PARAMETROS: usize = 10;

/// Um parâmetro nomeado da consulta (`:id_fatura`), que o leitor troca pelo valor
/// do caso.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParametroVerificacao {
    /// O nome, sem os dois-pontos (`id_fatura`).
    pub nome: String,
    /// O que pôr no lugar ("o número da fatura do caso relatado").
    pub descricao: String,
}

/// Um resultado possível da consulta e o que ele significa para a análise.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeituraResultado {
    /// O resultado ("uma linha com STATUS = 'PARCELADA'", "zero").
    pub resultado: String,
    /// O que ele significa ("confirma o diagnóstico: a fatura ficou parcelada
    /// depois de quitada").
    pub significado: String,
}

/// Uma consulta de verificação registrada na análise.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificacaoDados {
    pub numero: u32,
    /// Para que serve ("confirmar o diagnóstico no caso relatado", "contar os
    /// registros na mesma situação").
    pub objetivo: String,
    /// O SQL só de leitura, validado ao registrar, sem o `;` final.
    pub sql: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parametros: Vec<ParametroVerificacao>,
    pub resultados: Vec<LeituraResultado>,
    /// Tabelas que a consulta lê, como no modelo de dados (para a tela e a
    /// rastreabilidade).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tabelas: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sessao: Option<String>,
    pub registrada_ms: u64,
}

/// Dados de uma verificação nova ([`Analise::registrar_verificacao`]), com o SQL
/// já validado.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NovaVerificacao {
    pub objetivo: String,
    pub sql: String,
    pub parametros: Vec<ParametroVerificacao>,
    pub resultados: Vec<LeituraResultado>,
    pub tabelas: Vec<String>,
    pub sessao: Option<String>,
}

const VERIFICACAO: &str = "verificação";

fn invalida(texto: impl Into<String>) -> ErroModelo {
    ErroModelo::VerificacaoInvalida(texto.into())
}

impl Analise {
    /// Registra uma verificação em dados e devolve o número dela. Só na análise de
    /// impacto, com o objetivo, o SQL e ao menos um resultado com significado.
    pub fn registrar_verificacao(
        &mut self,
        nova: NovaVerificacao,
        agora_ms: u64,
    ) -> Result<u32, ErroModelo> {
        if self.fluxo != Fluxo::Impacto {
            return Err(ErroModelo::ForaDoImpacto(self.id.clone()));
        }
        if nova.objetivo.trim().is_empty() {
            return Err(ErroModelo::CampoVazio("objetivo"));
        }
        if nova.sql.trim().is_empty() {
            return Err(ErroModelo::CampoVazio("sql"));
        }
        if nova.resultados.is_empty() || nova.resultados.len() > MAX_RESULTADOS {
            return Err(invalida(format!(
                "explique de 1 a {MAX_RESULTADOS} resultados, cada um com o que significa \
                 para a análise (recebidos: {})",
                nova.resultados.len()
            )));
        }
        for r in &nova.resultados {
            if r.resultado.trim().is_empty() || r.significado.trim().is_empty() {
                return Err(invalida(
                    "cada resultado precisa do `resultado` e do `significado`",
                ));
            }
        }
        if nova.parametros.len() > MAX_PARAMETROS {
            return Err(invalida(format!(
                "no máximo {MAX_PARAMETROS} parâmetros (recebidos: {})",
                nova.parametros.len()
            )));
        }
        let mut nomes: Vec<String> = Vec::new();
        for p in &nova.parametros {
            let nome = p.nome.trim().trim_start_matches(':').to_lowercase();
            if nome.is_empty() || p.descricao.trim().is_empty() {
                return Err(invalida(
                    "cada parâmetro precisa do `nome` e da `descricao`",
                ));
            }
            if nomes.contains(&nome) {
                return Err(invalida(format!("o parâmetro `{nome}` aparece duas vezes")));
            }
            nomes.push(nome);
        }
        let sql = nova.sql.trim().trim_end_matches(';').trim_end().to_owned();
        if let Some(n) = self
            .verificacoes
            .iter()
            .find(|v| mesmo_sql(&v.sql, &sql))
            .map(|v| v.numero)
        {
            return Err(invalida(format!(
                "a mesma consulta já é a verificação V{n}; aponte para ela"
            )));
        }
        self.aceitar_registro()?;
        let numero = self
            .verificacoes
            .iter()
            .map(|v| v.numero)
            .max()
            .unwrap_or(0)
            + 1;
        self.verificacoes.push(VerificacaoDados {
            numero,
            objetivo: nova.objetivo.trim().to_owned(),
            sql,
            parametros: nova
                .parametros
                .into_iter()
                .map(|p| ParametroVerificacao {
                    nome: p.nome.trim().trim_start_matches(':').to_owned(),
                    descricao: p.descricao.trim().to_owned(),
                })
                .collect(),
            resultados: nova
                .resultados
                .into_iter()
                .map(|r| LeituraResultado {
                    resultado: r.resultado.trim().to_owned(),
                    significado: r.significado.trim().to_owned(),
                })
                .collect(),
            tabelas: nova.tabelas,
            sessao: nova.sessao,
            registrada_ms: agora_ms,
        });
        Ok(numero)
    }

    pub fn verificacao(&self, numero: u32) -> Option<&VerificacaoDados> {
        self.verificacoes.iter().find(|v| v.numero == numero)
    }

    /// Aponta a pergunta para a verificação que a responde (`perguntar` com
    /// `verificacao`).
    pub fn apontar_verificacao(
        &mut self,
        pergunta: u32,
        verificacao: u32,
    ) -> Result<(), ErroModelo> {
        if self.verificacao(verificacao).is_none() {
            return Err(ErroModelo::ItemDesconhecido {
                tipo: VERIFICACAO,
                numero: verificacao,
            });
        }
        let p = self
            .perguntas
            .iter_mut()
            .find(|p| p.numero == pergunta)
            .ok_or(ErroModelo::ItemDesconhecido {
                tipo: "pergunta",
                numero: pergunta,
            })?;
        p.verificacao = Some(verificacao);
        Ok(())
    }
}

/// A mesma consulta com outra quebra de linha, outros espaços ou outras
/// maiúsculas (#453): o SQL comparado sem diferenciar espaços nem maiúsculas.
fn mesmo_sql(a: &str, b: &str) -> bool {
    let normal = |s: &str| {
        s.split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase()
    };
    normal(a) == normal(b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analise::EstadoAnalise;
    use cba_kb::Serie;

    fn analise(fluxo: Fluxo) -> Analise {
        Analise::nova(
            Serie::Analise.id(1),
            "faturamento-exemplo",
            fluxo,
            "t",
            "e",
            1,
        )
    }

    fn nova() -> NovaVerificacao {
        NovaVerificacao {
            objetivo: " Confirmar o diagnóstico no caso relatado ".into(),
            sql: "SELECT STATUS FROM FATURA WHERE ID_FATURA = :id_fatura;".into(),
            parametros: vec![ParametroVerificacao {
                nome: ":id_fatura".into(),
                descricao: "a fatura do caso relatado".into(),
            }],
            resultados: vec![LeituraResultado {
                resultado: "PARCELADA".into(),
                significado: "confirma o diagnóstico".into(),
            }],
            tabelas: vec!["FATURA".into()],
            sessao: None,
        }
    }

    #[test]
    fn registra_numera_e_limpa() {
        let mut a = analise(Fluxo::Impacto);
        assert_eq!(a.registrar_verificacao(nova(), 5), Ok(1));
        let v = a.verificacao(1).unwrap();
        assert_eq!(v.objetivo, "Confirmar o diagnóstico no caso relatado");
        assert_eq!(
            v.sql,
            "SELECT STATUS FROM FATURA WHERE ID_FATURA = :id_fatura"
        );
        assert_eq!(v.parametros[0].nome, "id_fatura");
        assert_eq!(a.estado, EstadoAnalise::EmAndamento);
        let mut outra = nova();
        outra.sql = "SELECT COUNT(*) FROM FATURA".into();
        outra.parametros.clear();
        assert_eq!(a.registrar_verificacao(outra, 6), Ok(2));
    }

    #[test]
    fn recusa_fora_do_impacto_e_incompleta() {
        let mut a = analise(Fluxo::Perguntar);
        assert!(matches!(
            a.registrar_verificacao(nova(), 1),
            Err(ErroModelo::ForaDoImpacto(_))
        ));
        let mut a = analise(Fluxo::Impacto);
        let mut sem_resultado = nova();
        sem_resultado.resultados.clear();
        assert!(matches!(
            a.registrar_verificacao(sem_resultado, 1),
            Err(ErroModelo::VerificacaoInvalida(_))
        ));
        let mut repetido = nova();
        repetido.parametros.push(repetido.parametros[0].clone());
        assert!(matches!(
            a.registrar_verificacao(repetido, 1),
            Err(ErroModelo::VerificacaoInvalida(_))
        ));
        a.registrar_verificacao(nova(), 1).unwrap();
        let erro = a.registrar_verificacao(nova(), 2).unwrap_err();
        assert!(erro.to_string().contains("V1"), "{erro}");
        // Só outra formatação: é a mesma consulta.
        let mut reformatada = nova();
        reformatada.sql = "select STATUS\n  from FATURA   where ID_FATURA = :id_fatura".into();
        let erro = a.registrar_verificacao(reformatada, 3).unwrap_err();
        assert!(erro.to_string().contains("V1"), "{erro}");
    }

    #[test]
    fn aponta_a_pergunta_para_a_verificacao() {
        let mut a = analise(Fluxo::Impacto);
        a.registrar_verificacao(nova(), 1).unwrap();
        let p = a
            .registrar_pergunta("A fatura ficou parcelada?", None, None, 2)
            .unwrap();
        assert!(matches!(
            a.apontar_verificacao(p, 9),
            Err(ErroModelo::ItemDesconhecido { numero: 9, .. })
        ));
        a.apontar_verificacao(p, 1).unwrap();
        assert_eq!(a.perguntas[0].verificacao, Some(1));
        // Gravada e relida, a ligação fica; sem ela, o campo não aparece.
        let json = serde_json::to_value(&a).unwrap();
        assert_eq!(json["perguntas"][0]["verificacao"], 1);
        assert_eq!(json["verificacoes"][0]["numero"], 1);
        let relida: Analise = serde_json::from_value(json).unwrap();
        assert_eq!(relida, a);
    }
}
