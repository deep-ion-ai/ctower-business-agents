//! `registrar_verificacao` (#453): a consulta de verificação em dados escrita no
//! documento da análise de impacto, para quem tem acesso ao banco rodar.
//!
//! A consulta é conferida ao registrar:
//!
//! - **só leitura**: uma instrução `SELECT` (ou `WITH … SELECT`), pela mesma
//!   validação de `consultar_dados` ([`cba_sql`]), no dialeto genérico (sem
//!   conexão) e, com o banco ligado à sessão, também no dialeto dele;
//! - **nomes conhecidos**: as tabelas e as colunas existem no modelo de dados que
//!   `schema` conhece (os scripts de banco do projeto e, com o banco ligado, o
//!   catálogo dele);
//! - **sem dado real**: o caso entra por parâmetro nomeado (`:id_fatura`),
//!   declarado com a descrição do que pôr no lugar; identificador comparado a
//!   valor fixo e texto com cara de CPF, CNPJ ou e-mail são recusados.
//!
//! O documento leva o objetivo, o SQL, os parâmetros e o que cada resultado
//! significa, com a orientação de rodar numa sessão só de leitura. Executar a
//! consulta é de `consultar_dados` (#440), com o banco ligado e a aprovação do
//! analista; sem conexão, o leitor roda por conta própria.

use std::collections::BTreeSet;

use cba_core::ambiente::Ambiente;
use cba_core::analise::{Analise, Fluxo};
use cba_core::banco::SchemaBanco;
use cba_core::eventos::Mudanca;
use cba_core::verificacoes::{
    LeituraResultado, MAX_PARAMETROS, MAX_RESULTADOS, NovaVerificacao, ParametroVerificacao,
};
use cba_index::{Mapa, TipoSimbolo};
use cba_sql::ConsultaLida;
use serde::Deserialize;
use serde_json::json;

use crate::comum;
use crate::fontes;
use crate::registro::{ErroFerramenta, Ferramenta, ler_argumentos};

/// Nomes que a consulta pode usar sem ser coluna de uma tabela: as
/// pseudocolunas e funções sem parênteses dos bancos.
const PSEUDOCOLUNAS: &[&str] = &[
    "SYSDATE",
    "SYSTIMESTAMP",
    "CURRENT_DATE",
    "CURRENT_TIMESTAMP",
    "LOCALTIMESTAMP",
    "ROWNUM",
    "ROWID",
    "LEVEL",
    "USER",
    "NULL",
    "TRUE",
    "FALSE",
];

#[derive(Debug, Deserialize)]
struct Parametro {
    nome: String,
    descricao: String,
}

#[derive(Debug, Deserialize)]
struct Resultado {
    resultado: String,
    significado: String,
}

#[derive(Debug, Deserialize)]
struct Args {
    analise: String,
    objetivo: String,
    sql: String,
    #[serde(default)]
    parametros: Vec<Parametro>,
    resultados: Vec<Resultado>,
}

/// `registrar_verificacao`: a consulta que confirma o diagnóstico ou mede a
/// abrangência, pronta para quem tem acesso ao banco.
pub fn registrar_verificacao() -> Ferramenta {
    Ferramenta::nova(
        "registrar_verificacao",
        "Registrar verificação em dados",
        "Registra, na seção \"Verificações em dados\" da análise de impacto, uma \
         consulta SQL só de leitura que quem tem acesso ao banco roda para conferir a \
         análise: reproduzir a condição da regra no caso relatado, contar os registros \
         na mesma situação, separar por ano ou pelo valor do critério. Use quando a \
         demanda trata do estado ou do volume dos dados. A consulta é validada ao \
         registrar: uma instrução SELECT (ou WITH … SELECT); só tabelas e colunas que \
         `schema` conhece; o caso entra por parâmetro nomeado (`:id_fatura`), declarado \
         em `parametros`, nunca com um valor real. Explique em `resultados` o que cada \
         resultado significa para a análise. Para a pergunta que a consulta responde, \
         passe `verificacao` em `perguntar`. Não executa a consulta: com o banco ligado \
         à análise, `consultar_dados` roda, com a aprovação do analista.",
        json!({
            "type": "object",
            "properties": {
                "analise": comum::schema_da_analise(),
                "objetivo": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 500,
                    "description": "Para que serve a consulta, em uma frase (\"confirmar o diagnóstico no caso relatado\", \"contar as faturas na mesma situação\", \"separar por ano e por valor do critério\")."
                },
                "sql": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": cba_core::consultas::MAX_SQL,
                    "description": "Um SELECT (ou WITH … SELECT), formatado em várias linhas para o leitor, com os nomes de tabela e coluna do modelo de dados e parâmetros nomeados (`:id_fatura`) no lugar dos dados do caso."
                },
                "parametros": {
                    "type": "array",
                    "maxItems": MAX_PARAMETROS,
                    "description": "Cada parâmetro nomeado do SQL, com o que o leitor põe no lugar. Todo parâmetro do SQL precisa estar aqui, e só eles.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "nome": {
                                "type": "string",
                                "minLength": 1,
                                "maxLength": 60,
                                "description": "O nome, com ou sem os dois-pontos (`id_fatura`)."
                            },
                            "descricao": {
                                "type": "string",
                                "minLength": 1,
                                "maxLength": 300,
                                "description": "O que pôr no lugar (\"o número da fatura do caso relatado\")."
                            }
                        },
                        "required": ["nome", "descricao"],
                        "additionalProperties": false
                    }
                },
                "resultados": {
                    "type": "array",
                    "minItems": 1,
                    "maxItems": MAX_RESULTADOS,
                    "description": "Os resultados possíveis e o que cada um significa para a análise.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "resultado": {
                                "type": "string",
                                "minLength": 1,
                                "maxLength": 300,
                                "description": "O resultado (\"uma linha, com STATUS = CANCELADA e VL_PAGO > 0\", \"zero\", \"mais de zero\")."
                            },
                            "significado": {
                                "type": "string",
                                "minLength": 1,
                                "maxLength": 1000,
                                "description": "O que ele significa para a análise (\"confirma o diagnóstico\", \"o caso não é isolado: a abrangência cresce\")."
                            }
                        },
                        "required": ["resultado", "significado"],
                        "additionalProperties": false
                    }
                }
            },
            "required": ["analise", "objetivo", "sql", "resultados"],
            "additionalProperties": false
        }),
        |ambiente, args| {
            let args: Args = ler_argumentos(args)?;
            let id = comum::id_da_analise(&args.analise)?;
            let analise = ambiente.analises.carregar(&id)?;
            exigir_impacto(&analise)?;
            let consulta = conferir_sql(ambiente, &args.sql)?;
            conferir_parametros(&consulta, &args.parametros)?;
            let tabelas = conferir_nomes(ambiente, &consulta)?;
            let nova = NovaVerificacao {
                objetivo: args.objetivo,
                sql: consulta.sql.clone(),
                parametros: args
                    .parametros
                    .into_iter()
                    .map(|p| ParametroVerificacao {
                        nome: p.nome,
                        descricao: p.descricao,
                    })
                    .collect(),
                resultados: args
                    .resultados
                    .into_iter()
                    .map(|r| LeituraResultado {
                        resultado: r.resultado,
                        significado: r.significado,
                    })
                    .collect(),
                tabelas: tabelas.clone(),
                sessao: None,
            };
            let registrado = comum::alterar(ambiente, &id, Mudanca::Consulta, |a, agora| {
                a.registrar_verificacao(nova, agora)
            })?;
            let numero = registrado.resultado;
            Ok(comum::resposta(
                ambiente,
                &registrado,
                json!({
                    "verificacao": numero,
                    "rotulo": format!("V{numero}"),
                    "tabelas": tabelas,
                    "parametros": consulta.parametros,
                    "dica": "Para a pergunta que esta consulta responde, passe `verificacao` em `perguntar`.",
                }),
            ))
        },
    )
}

fn exigir_impacto(analise: &Analise) -> Result<(), ErroFerramenta> {
    if analise.fluxo == Fluxo::Impacto {
        Ok(())
    } else {
        Err(ErroFerramenta::Recusa(format!(
            "a análise {} não é uma análise de impacto: as verificações em dados só entram \
             no documento da análise de impacto",
            analise.id
        )))
    }
}

/// Só leitura, no dialeto genérico e, com o banco ligado, no dele também.
fn conferir_sql(ambiente: &Ambiente, sql: &str) -> Result<ConsultaLida, ErroFerramenta> {
    let corrigir = |e: String| {
        ErroFerramenta::Recusa(format!(
            "consulta recusada: {e}. Corrija o SQL e registre de novo"
        ))
    };
    let consulta = cba_sql::ler(None, sql).map_err(|e| corrigir(e.to_string()))?;
    if let Some(banco) = &ambiente.banco {
        banco.validar(&consulta.sql).map_err(corrigir)?;
    }
    Ok(consulta)
}

/// Os parâmetros do SQL e os declarados são os mesmos.
fn conferir_parametros(
    consulta: &ConsultaLida,
    declarados: &[Parametro],
) -> Result<(), ErroFerramenta> {
    let no_sql: BTreeSet<String> = consulta
        .parametros
        .iter()
        .map(|p| p.to_lowercase())
        .collect();
    let lista: BTreeSet<String> = declarados
        .iter()
        .map(|p| p.nome.trim().trim_start_matches(':').to_lowercase())
        .collect();
    let faltam: Vec<String> = no_sql.difference(&lista).map(|p| format!(":{p}")).collect();
    if !faltam.is_empty() {
        return Err(ErroFerramenta::Recusa(format!(
            "declare em `parametros` o que o leitor põe no lugar de {}",
            faltam.join(", ")
        )));
    }
    let sobram: Vec<String> = lista.difference(&no_sql).map(|p| format!(":{p}")).collect();
    if !sobram.is_empty() {
        return Err(ErroFerramenta::Recusa(format!(
            "o SQL não usa {}: tire de `parametros` ou use no SQL",
            sobram.join(", ")
        )));
    }
    Ok(())
}

/// Uma tabela do modelo de dados: o nome como o modelo o escreve e as colunas
/// (vazio quando o modelo não as conhece, como num sinônimo).
struct TabelaConhecida {
    nome: String,
    colunas: Vec<String>,
}

impl TabelaConhecida {
    fn tem(&self, coluna: &str) -> bool {
        self.colunas.is_empty() || self.colunas.iter().any(|c| c.eq_ignore_ascii_case(coluna))
    }
}

fn do_mapa(mapa: &Mapa, nome: &str) -> Option<TabelaConhecida> {
    let achados: Vec<_> = mapa
        .buscar(nome)
        .into_iter()
        .filter(|a| {
            matches!(
                a.simbolo.tipo,
                TipoSimbolo::Tabela | TipoSimbolo::Visao | TipoSimbolo::Sinonimo
            )
        })
        .collect();
    let primeiro = achados.first()?;
    let mut colunas: Vec<String> = Vec::new();
    for a in &achados {
        for c in mapa.filhos(a.id) {
            if !colunas.contains(&c.simbolo.nome) {
                colunas.push(c.simbolo.nome.clone());
            }
        }
    }
    Some(TabelaConhecida {
        nome: primeiro.simbolo.qualificado.clone(),
        colunas,
    })
}

fn do_banco(schema: &SchemaBanco, nome: &str) -> Option<TabelaConhecida> {
    let procura = nome.to_uppercase();
    let ultimo = procura.rsplit('.').next().unwrap_or(&procura).to_owned();
    let t = schema
        .tabelas
        .iter()
        .find(|t| t.qualificado().to_uppercase() == procura)
        .or_else(|| {
            schema
                .tabelas
                .iter()
                .find(|t| t.nome.to_uppercase() == ultimo)
        })?;
    Some(TabelaConhecida {
        nome: t.qualificado(),
        colunas: t.colunas.iter().map(|c| c.nome.clone()).collect(),
    })
}

/// As tabelas e as colunas da consulta existem no modelo de dados. Devolve as
/// tabelas como o modelo as escreve.
fn conferir_nomes(
    ambiente: &Ambiente,
    consulta: &ConsultaLida,
) -> Result<Vec<String>, ErroFerramenta> {
    let carregado = fontes::mapa(ambiente)?;
    let mapa = &carregado.mapa;
    let mut catalogo: Option<SchemaBanco> = None;
    let mut banco_lido = false;
    let mut falha_do_banco: Option<String> = None;
    let mut conhecidas: Vec<(String, TabelaConhecida)> = Vec::new();
    let mut desconhecidas: Vec<String> = Vec::new();
    for r in &consulta.relacoes {
        let mut achada = do_mapa(mapa, &r.nome);
        if achada.is_none()
            && let Some(banco) = &ambiente.banco
        {
            if !banco_lido {
                banco_lido = true;
                match banco.ler_schema() {
                    Ok(s) => catalogo = Some(s),
                    Err(e) => falha_do_banco = Some(e),
                }
            }
            achada = catalogo.as_ref().and_then(|s| do_banco(s, &r.nome));
        }
        match achada {
            Some(t) => conhecidas.push((r.nome.clone(), t)),
            None => desconhecidas.push(r.nome.clone()),
        }
    }
    if !desconhecidas.is_empty() {
        let sem_modelo = mapa.schema().is_empty() && ambiente.banco.is_none();
        return Err(ErroFerramenta::Recusa(if sem_modelo {
            "o projeto não tem modelo de dados conhecido (nenhuma tabela nos scripts de \
             banco, e nenhum banco ligado): sem ele, a consulta não pode ser conferida. \
             Registre a verificação como pergunta, com `perguntar`"
                .to_owned()
        } else {
            format!(
                "a consulta usa {} fora do modelo de dados que `schema` conhece: {}. Use \
                 só tabelas e visões que `schema` lista",
                if desconhecidas.len() == 1 {
                    "uma tabela"
                } else {
                    "tabelas"
                },
                desconhecidas.join(", ")
            ) + &falha_do_banco
                .map(|e| {
                    format!(
                        " (o catálogo do banco ligado não pôde ser lido: {e}; a tabela pode \
                         existir só no banco: tente de novo depois)"
                    )
                })
                .unwrap_or_default()
        }));
    }
    let tabela_de = |escrita: &str| {
        conhecidas
            .iter()
            .find(|(n, _)| n == escrita)
            .map(|(_, t)| t)
    };
    let mut erros: Vec<String> = Vec::new();
    for c in &consulta.colunas {
        if PSEUDOCOLUNAS.contains(&c.nome.to_uppercase().as_str()) {
            continue;
        }
        match &c.qualificador {
            Some(q) if consulta.e_virtual(q) => {}
            Some(q) => match consulta
                .tabela_do_qualificador(q)
                .and_then(|r| tabela_de(&r.nome))
            {
                Some(t) if t.tem(&c.nome) => {}
                Some(t) => erros.push(format!("`{c}` (a tabela {} não tem a coluna)", t.nome)),
                None => erros.push(format!(
                    "`{c}` (`{q}` não é tabela nem apelido da consulta)"
                )),
            },
            None => {
                let existe =
                    consulta.e_apelido(&c.nome) || conhecidas.iter().any(|(_, t)| t.tem(&c.nome));
                if !existe {
                    let nomes: Vec<&str> =
                        conhecidas.iter().map(|(_, t)| t.nome.as_str()).collect();
                    erros.push(format!(
                        "`{c}` (não é coluna de {})",
                        if nomes.is_empty() {
                            "nenhuma tabela da consulta".to_owned()
                        } else {
                            nomes.join(", ")
                        }
                    ));
                }
            }
        }
    }
    if !erros.is_empty() {
        return Err(ErroFerramenta::Recusa(format!(
            "a consulta usa colunas que o modelo de dados não tem: {}. Confira as colunas \
             com `schema` (\"tabela\") e registre de novo",
            erros.join("; ")
        )));
    }
    let mut tabelas: Vec<String> = Vec::new();
    for (_, t) in conhecidas {
        if !tabelas.contains(&t.nome) {
            tabelas.push(t.nome);
        }
    }
    Ok(tabelas)
}

/// Orientação no topo da seção, antes das consultas.
pub const ORIENTACAO: &str = "_Consultas só de leitura para quem tem acesso ao banco \
conferir a análise. Rode numa sessão só de leitura (transação `READ ONLY` ou um usuário \
que só tem `SELECT`), troque cada parâmetro (`:nome`) pelo valor do caso e não copie \
dados pessoais do resultado para a demanda. Com o banco ligado à análise, o app roda a \
consulta com a aprovação do analista._";

/// O rótulo da verificação no documento (`V1`).
pub fn rotulo(numero: u32) -> String {
    format!("V{numero}")
}

/// Texto da seção `verificacoes_em_dados`: a orientação e, para cada
/// verificação, o objetivo, o SQL, os parâmetros e o que cada resultado
/// significa. `None` sem verificações.
pub fn texto(analise: &Analise) -> Option<cba_docs::Texto> {
    use std::fmt::Write as _;
    if analise.verificacoes.is_empty() {
        return None;
    }
    let uma_linha = |t: &str| t.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut t = String::from(ORIENTACAO);
    for v in &analise.verificacoes {
        let _ = write!(
            t,
            "\n\n### {}. {}\n\n{}",
            rotulo(v.numero),
            uma_linha(&v.objetivo),
            cba_docs::bloco_de_codigo(&v.sql, "sql")
        );
        if !v.parametros.is_empty() {
            t.push_str("\n\n**Parâmetros:**\n");
            for p in &v.parametros {
                let _ = write!(t, "\n- `:{}`: {}", p.nome, uma_linha(&p.descricao));
            }
        }
        t.push_str("\n\n**O que o resultado significa:**\n");
        for r in &v.resultados {
            let _ = write!(
                t,
                "\n- **{}:** {}",
                uma_linha(&r.resultado).trim_end_matches(['.', ':']),
                uma_linha(&r.significado)
            );
        }
    }
    Some(cba_docs::Texto::novo(cba_docs::VERIFICACOES_EM_DADOS, t))
}

/// O texto da pergunta com a verificação que a responde, se houver.
pub fn pergunta_com_verificacao(
    analise: &Analise,
    texto: &str,
    verificacao: Option<u32>,
) -> String {
    match verificacao.and_then(|n| analise.verificacao(n)) {
        Some(v) => format!(
            "{} (Para responder com dados: consulta {} em \"Verificações em dados\".)",
            texto.trim_end(),
            rotulo(v.numero)
        ),
        None => texto.to_owned(),
    }
}
