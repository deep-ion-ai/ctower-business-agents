//! `consultar_dados` (#440): o agente consulta o banco de leitura numa análise,
//! com a aprovação do analista para cada consulta e os dados pessoais sempre
//! mascarados.
//!
//! - **Habilitada por análise:** a ferramenta só aparece na sessão quando o
//!   projeto tem a conexão de banco e o analista a ligou naquela análise ("Usar o
//!   banco <nome> nesta análise"). O servidor MCP preenche
//!   [`Ambiente::banco`] a partir disso ([`conexao_ligada`]); sem banco, o agente
//!   não vê a ferramenta ([`crate::Ferramenta::visivel_se`]).
//! - **Aprovação por consulta:** a chamada grava o pedido na análise (objetivo,
//!   SQL, conexão e ambiente) e espera a decisão do analista no app: Executar,
//!   Editar e executar ou Recusar, com um motivo que volta ao agente. Sem resposta
//!   no prazo, a consulta expira.
//! - **Execução** pela conexão só leitura do banco ([`BancoConectado::consultar`]),
//!   com a validação, o tempo limite e o limite de linhas da conexão.
//! - **Mascaramento** sempre, antes de o resultado chegar ao agente
//!   ([`crate::mascaramento`]); a resposta diz quais colunas foram mascaradas.
//! - **Registro:** toda consulta, aprovada ou recusada, fica no estado da análise
//!   ([`cba_core::consultas`]) com quem decidiu, quando, o SQL e a quantidade de
//!   linhas, sem os valores.
//!
//! O resultado citado numa afirmação (`afirmar` com `dados`) vira a evidência de
//! dados do documento.

use std::fmt::Write as _;
use std::time::{Duration, Instant};

use cba_core::ambiente::Ambiente;
use cba_core::analise::Analise;
use cba_core::banco::BancoConectado;
use cba_core::consultas::{ExecucaoConsulta, SituacaoConsulta};
use cba_core::eventos::Mudanca;
use cba_kb::Id;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::comum;
use crate::mascaramento;
use crate::registro::{ErroFerramenta, Ferramenta, Saida, ler_argumentos};

/// Quanto tempo a chamada espera a decisão do analista.
pub const PRAZO_APROVACAO: Duration = Duration::from_secs(15 * 60);
/// Intervalo entre as leituras do estado da análise durante a espera.
const INTERVALO: Duration = Duration::from_millis(250);
/// Linhas mostradas no texto da resposta (todas vão nos dados estruturados).
const LINHAS_NO_TEXTO: usize = 50;
/// Tamanho máximo de uma célula no texto da resposta.
const MAX_CELULA: usize = 80;

/// A conexão de banco ligada na análise, se o projeto ainda a tem (o cadastro é
/// relido: o analista pode ter mudado os bancos do projeto com a sessão aberta).
pub fn conexao_ligada(ambiente: &Ambiente, analise: &Analise) -> Option<String> {
    let ligado = analise.banco.as_ref()?;
    let bancos = cba_index::Cadastro::em(ambiente.dados.projetos())
        .obter(&ambiente.projeto.id)
        .ok()
        .flatten()
        .map_or_else(|| ambiente.projeto.bancos.clone(), |p| p.bancos);
    bancos
        .contains(&ligado.conexao)
        .then(|| ligado.conexao.clone())
}

/// A conexão ligada na análise `analise` (`A-NNNN`), para o servidor MCP
/// preencher [`Ambiente::banco`] antes de cada chamada.
pub fn conexao_da_analise(ambiente: &Ambiente, analise: &str) -> Option<String> {
    let id = comum::id_da_analise(analise).ok()?;
    let analise = ambiente.analises.carregar(&id).ok()?;
    conexao_ligada(ambiente, &analise)
}

#[derive(Debug, Deserialize)]
struct Args {
    analise: String,
    objetivo: String,
    sql: String,
}

/// `consultar_dados`, com o prazo padrão de aprovação.
pub fn consultar_dados() -> Ferramenta {
    consultar_dados_com_prazo(PRAZO_APROVACAO)
}

/// `consultar_dados` com outro prazo de aprovação (testes).
pub fn consultar_dados_com_prazo(prazo: Duration) -> Ferramenta {
    Ferramenta::nova(
        "consultar_dados",
        "Consultar o banco (com aprovação do analista)",
        "Roda uma consulta só de leitura no banco ligado a esta análise, depois que o \
         analista a aprova no app (ele pode executar, editar e executar, ou recusar com \
         um motivo). A chamada espera a decisão. Prefira agregados (COUNT, GROUP BY, \
         SUM) e amostras pequenas (LIMIT); não traga linhas inteiras de pessoas. Os \
         dados pessoais (CPF, CNPJ, e-mail, telefone, nomes junto de documentos e \
         colunas pessoais) chegam sempre mascarados, e a resposta diz quais colunas \
         foram mascaradas. Para citar o resultado no documento, use `afirmar` com \
         `dados: [{consulta, resumo}]`: vira uma evidência de dados com a data.",
        json!({
            "type": "object",
            "properties": {
                "analise": comum::schema_da_analise(),
                "objetivo": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 500,
                    "description": "Para que serve a consulta, em uma frase para o analista (\"confirmar o diagnóstico no caso relatado\", \"medir quantas faturas estão na mesma situação\")."
                },
                "sql": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": cba_core::consultas::MAX_SQL,
                    "description": "Um SELECT (ou WITH … SELECT) no dialeto do banco, formatado em várias linhas para o analista ler."
                }
            },
            "required": ["analise", "objetivo", "sql"],
            "additionalProperties": false
        }),
        move |ambiente, args| {
            let args: Args = ler_argumentos(args)?;
            consultar(ambiente, args, prazo)
        },
    )
    .visivel_se(
        |ambiente| ambiente.banco.is_some(),
        "o banco não está ligado nesta sessão: o analista liga com \"Usar o banco <nome> \
         nesta análise\" no app. Siga só com o código e deixe o SQL pronto no documento",
    )
}

fn consultar(ambiente: &Ambiente, args: Args, prazo: Duration) -> Result<Saida, ErroFerramenta> {
    let id = comum::id_da_analise(&args.analise)?;
    let banco = ambiente.banco.as_ref().ok_or_else(|| {
        ErroFerramenta::Recusa(
            "o banco não está ligado nesta sessão: o analista liga com \"Usar o banco \
             <nome> nesta análise\" no app. Siga só com o código e deixe o SQL pronto no \
             documento"
                .into(),
        )
    })?;
    let origem = banco.origem();
    let analise = ambiente.analises.carregar(&id)?;
    if conexao_ligada(ambiente, &analise).as_deref() != Some(origem.conexao.as_str()) {
        return Err(ErroFerramenta::Recusa(format!(
            "o banco {origem} não está ligado na análise {id}: o analista precisa marcar \
             \"Usar o banco nesta análise\" nela. Siga só com o código e deixe o SQL pronto \
             no documento"
        )));
    }
    banco
        .validar(&args.sql)
        .map_err(|e| ErroFerramenta::Recusa(format!("{e}. Corrija o SQL e chame de novo")))?;

    let pedido = comum::alterar(ambiente, &id, Mudanca::Consulta, |a, agora| {
        a.pedir_consulta(&args.objetivo, &args.sql, origem.clone(), agora)
    })?;
    let numero = pedido.resultado;
    tracing::info!(
        consulta = numero,
        "consulta ao banco aguardando a aprovação"
    );

    let consulta = esperar(ambiente, &id, numero, prazo)?;
    match consulta.situacao {
        SituacaoConsulta::Recusada => {
            let motivo = consulta.motivo.unwrap_or_default();
            return Err(ErroFerramenta::Recusa(format!(
                "o analista recusou a consulta {numero}: {motivo}. Leve o motivo em conta: \
                 ajuste a consulta (prefira agregados) ou siga sem ela"
            )));
        }
        SituacaoConsulta::Expirada => {
            return Err(ErroFerramenta::Recusa(format!(
                "a consulta {numero} não foi aprovada a tempo{}: siga com o código e deixe o \
                 SQL pronto no documento, ou peça de novo quando o analista puder responder",
                consulta
                    .motivo
                    .map(|m| format!(" ({m})"))
                    .unwrap_or_default()
            )));
        }
        SituacaoConsulta::Aprovada => {}
        outra => {
            return Err(ErroFerramenta::Interno(format!(
                "a consulta {numero} está em `{outra:?}`, esperado `aprovada`"
            )));
        }
    }
    let sql = consulta.sql().to_owned();
    executar(
        ambiente,
        &id,
        numero,
        banco.as_ref(),
        &sql,
        consulta.editada,
    )
}

/// Espera o analista decidir: relê o estado até a consulta sair de pendente. No
/// fim do prazo, ela expira.
fn esperar(
    ambiente: &Ambiente,
    id: &Id,
    numero: u32,
    prazo: Duration,
) -> Result<cba_core::consultas::ConsultaDados, ErroFerramenta> {
    let inicio = Instant::now();
    loop {
        let analise = ambiente.analises.carregar(id)?;
        let c = analise
            .consulta(numero)
            .cloned()
            .ok_or_else(|| ErroFerramenta::Interno(format!("a consulta {numero} sumiu")))?;
        if c.situacao != SituacaoConsulta::Pendente {
            return Ok(c);
        }
        if inicio.elapsed() >= prazo {
            let r = comum::alterar(ambiente, id, Mudanca::Consulta, |a, agora| {
                a.expirar_consulta(numero, agora)
            })?;
            return r
                .analise
                .consulta(numero)
                .cloned()
                .ok_or_else(|| ErroFerramenta::Interno(format!("a consulta {numero} sumiu")));
        }
        std::thread::sleep(INTERVALO);
    }
}

fn executar(
    ambiente: &Ambiente,
    id: &Id,
    numero: u32,
    banco: &dyn BancoConectado,
    sql: &str,
    editada: bool,
) -> Result<Saida, ErroFerramenta> {
    let origem = banco.origem();
    let resultado = match banco.consultar(sql) {
        Ok(r) => r,
        Err(e) => {
            comum::alterar(ambiente, id, Mudanca::Consulta, |a, agora| {
                a.registrar_execucao(numero, Err(e.clone()), agora)
            })?;
            return Err(ErroFerramenta::Recusa(format!(
                "a consulta {numero} foi aprovada, mas falhou: {e}"
            )));
        }
    };
    let m = mascaramento::mascarar(&resultado, sql);
    let execucao = ExecucaoConsulta {
        linhas: m.linhas.len(),
        cortada: resultado.cortada,
        colunas: m.colunas.iter().map(|c| c.nome.clone()).collect(),
        mascaradas: m.mascaradas.iter().map(|c| c.coluna.clone()).collect(),
        duracao_ms: resultado.duracao_ms,
    };
    let registrado = comum::alterar(ambiente, id, Mudanca::Consulta, |a, agora| {
        a.registrar_execucao(numero, Ok(execucao), agora)
    })?;
    tracing::info!(
        consulta = numero,
        linhas = m.linhas.len(),
        mascaradas = m.mascaradas.len(),
        "consulta ao banco executada"
    );

    let mut texto = String::new();
    let _ = writeln!(
        texto,
        "Consulta {numero} executada em {origem}, aprovada pelo analista{}.",
        if editada {
            " com edição (o SQL abaixo é o editado)"
        } else {
            ""
        }
    );
    let _ = writeln!(texto, "\n```sql\n{}\n```\n", sql.trim());
    let _ = write!(
        texto,
        "{} linha(s) em {} ms",
        m.linhas.len(),
        resultado.duracao_ms
    );
    if resultado.cortada {
        let _ = write!(
            texto,
            ", cortada no limite de {} linhas da conexão: prefira um agregado",
            resultado.limite_linhas
        );
    }
    texto.push_str(".\n");
    if m.mascaradas.is_empty() {
        texto.push_str("Nenhuma coluna mascarada.\n");
    } else {
        texto.push_str("Colunas mascaradas (dados pessoais, sem exceção):\n");
        for c in &m.mascaradas {
            let _ = writeln!(texto, "- {} ({})", c.coluna, c.motivo);
        }
    }
    texto.push('\n');
    texto.push_str(&tabela(&m.colunas, &m.linhas));
    let _ = writeln!(
        texto,
        "\nPara citar este resultado no documento, use `afirmar` com `dados: \
         [{{\"consulta\": {numero}, \"resumo\": \"o que o resultado mostra\"}}]`. A \
         evidência de dados vale para a data de hoje: a revalidação não reexecuta a \
         consulta."
    );
    let dados = json!({
        "analise": id,
        "consulta": numero,
        "conexao": origem,
        "sql": sql,
        "editada": editada,
        "colunas": m.colunas,
        "linhas": m.linhas,
        "cortada": resultado.cortada,
        "mascaradas": m.mascaradas,
        "documento": ambiente.kb.na_base(&registrado.analise.documento),
    });
    Ok(Saida {
        texto,
        dados: Some(dados),
    })
}

fn celula(v: &Value) -> String {
    let t = match v {
        Value::Null => "NULL".to_owned(),
        Value::String(s) => s.clone(),
        outro => outro.to_string(),
    };
    let t = t.replace(['\n', '\r'], " ").replace('|', "\\|");
    if t.chars().count() > MAX_CELULA {
        let mut c: String = t.chars().take(MAX_CELULA).collect();
        c.push('…');
        c
    } else {
        t
    }
}

/// O resultado em tabela Markdown, com as primeiras linhas.
fn tabela(colunas: &[cba_core::banco::ColunaDados], linhas: &[Vec<Value>]) -> String {
    if colunas.is_empty() {
        return "Sem colunas no resultado.\n".into();
    }
    let mut t = String::new();
    let nomes: Vec<String> = colunas.iter().map(|c| c.nome.replace('|', "\\|")).collect();
    let _ = writeln!(t, "| {} |", nomes.join(" | "));
    let _ = writeln!(t, "|{}", " --- |".repeat(colunas.len()));
    for l in linhas.iter().take(LINHAS_NO_TEXTO) {
        let cels: Vec<String> = l.iter().map(celula).collect();
        let _ = writeln!(t, "| {} |", cels.join(" | "));
    }
    if linhas.len() > LINHAS_NO_TEXTO {
        let _ = writeln!(
            t,
            "\n({} linhas a mais nos dados estruturados.)",
            linhas.len() - LINHAS_NO_TEXTO
        );
    }
    t
}
