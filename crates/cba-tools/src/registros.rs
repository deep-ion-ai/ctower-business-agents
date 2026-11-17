//! Premissas, decisões, riscos e restrições da demanda (#122): as propostas do
//! agente e a seção dos documentos exportados.
//!
//! ## Propostas
//!
//! O agente não grava registro sozinho: ele propõe e o analista aceita ou recusa
//! no painel da demanda. As propostas são derivadas, na hora, do que o agente já
//! registra nos fluxos existentes, sem ferramenta nova:
//!
//! - **análise de impacto** ligada à demanda: cada afirmação da seção
//!   `demanda_como_entendida` no formato "Premissa do solicitante: …. Verificação:
//!   …" vira uma premissa (o veredito sugere o status: confere → validada, não
//!   confere → invalidada, confere em parte ou `a_confirmar` → não validada), e
//!   cada afirmação da seção `riscos` vira um risco. Afirmações marcadas como
//!   incorretas pelo analista ficam de fora;
//! - **ata** de reunião da demanda: cada item de decisão (`D-1`) vira uma decisão,
//!   com a data da reunião e quem decidiu (o falante da primeira citação e, pelo
//!   participante, a pessoa do catálogo).
//!
//! Cada proposta tem uma referência estável (`A-0001/3`, a afirmação 3 da análise;
//! `R-0001/D-1`, o item da ata). Aceitar cria o registro com a referência e a
//! origem ([`aceitar_proposta`]); recusar guarda a referência
//! ([`cba_kb::Projeto::recusar_proposta`]). Em ambos os casos a proposta não volta.
//!
//! ## Exportação
//!
//! O documento exportado de uma análise ou ata ligada a uma demanda ganha a seção
//! "Premissas, decisões, riscos e restrições" com os registros da demanda
//! ([`com_registros`]).

use std::collections::HashSet;

use cba_core::ambiente::Ambiente;
use cba_core::analise::{Analise, Confianca, EstadoAfirmacao, Fluxo};
use cba_core::analises;
use cba_docs::{EstadoAta, TipoItem};
use cba_kb::frontmatter::{Data, FrontmatterAnalise};
use cba_kb::registros::{
    MAXIMO_DO_TITULO, Nivel, StatusPremissa, TipoRestricao, ValidacaoDePremissa,
};
use cba_kb::{
    CaminhoRelativo, Categoria, DadosRegistro, IdDemanda, IdRegistro, NovoRegistro, Projeto,
    RegistroLido,
};
use serde::Serialize;

/// Seção do fluxo de impacto com as premissas do solicitante verificadas.
pub const SECAO_PREMISSAS: &str = "demanda_como_entendida";
/// Seção do fluxo de impacto com os riscos.
pub const SECAO_RISCOS: &str = "riscos";
/// Título da seção nos documentos exportados.
pub const TITULO_DA_SECAO: &str = "Premissas, decisões, riscos e restrições";

/// Quem assume as premissas que vêm do texto da demanda.
const SOLICITANTE: &str = "Solicitante da demanda";

/// Erro ao derivar, aceitar ou recusar propostas.
#[derive(Debug, thiserror::Error)]
pub enum ErroRegistros {
    #[error(transparent)]
    Base(#[from] cba_kb::Erro),
    #[error(transparent)]
    Analise(#[from] analises::Erro),
    #[error(
        "a proposta {0} não está entre as pendentes desta demanda (já foi aceita, recusada \
         ou a origem mudou)"
    )]
    PropostaInexistente(String),
}

/// De onde vem uma proposta.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "tipo", rename_all = "camelCase")]
pub enum FonteProposta {
    /// Análise de impacto (`A-0001`).
    Analise { id: String, titulo: String },
    /// Ata de reunião (`R-0001`), com a data da reunião.
    Ata { id: String, data: String },
}

/// Registro proposto pelo agente, à espera do analista.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Proposta {
    /// `A-0001/3` (afirmação 3 da análise) ou `R-0001/D-1` (item da ata).
    pub referencia: String,
    pub categoria: Categoria,
    pub fonte: FonteProposta,
    /// Documento de origem, relativo à raiz do projeto.
    #[serde(skip)]
    pub origem: CaminhoRelativo,
    /// Valores sugeridos; o analista pode ajustar antes de aceitar.
    pub dados: DadosRegistro,
}

/// Texto de uma linha que cabe no enunciado de um registro.
fn uma_linha(texto: &str) -> String {
    let linha = texto.split_whitespace().collect::<Vec<_>>().join(" ");
    if linha.chars().count() <= MAXIMO_DO_TITULO {
        return linha;
    }
    let mut curto: String = linha.chars().take(MAXIMO_DO_TITULO - 1).collect();
    curto.truncate(curto.trim_end().len());
    curto.push('…');
    curto
}

/// Primeira frase do texto (até o primeiro ". "), em uma linha.
fn primeira_frase(texto: &str) -> String {
    let texto = texto.trim();
    let frase = match texto.find(". ") {
        Some(fim) => &texto[..=fim],
        None => texto,
    };
    uma_linha(frase)
}

/// Busca sem diferença de maiúsculas (os textos são curtos): a posição em bytes
/// de `agulha` em `texto`.
fn achar(texto: &str, agulha: &str) -> Option<usize> {
    let minusculo = texto.to_lowercase();
    // Minúsculas podem mudar o tamanho de alguns caracteres; na dúvida, não acha.
    if minusculo.len() != texto.len() {
        return None;
    }
    minusculo.find(&agulha.to_lowercase())
}

/// Premissa do solicitante e o veredito, de uma afirmação no formato "Premissa do
/// solicitante: <premissa>. Verificação: <veredito>. <o que o código mostra>".
fn premissa_da_afirmacao(texto: &str) -> Option<(String, Option<StatusPremissa>)> {
    let texto = texto.trim();
    if !texto.to_lowercase().starts_with("premissa") {
        return None;
    }
    let inicio = texto.find(':')? + 1;
    let (enunciado, veredito) = match achar(texto, "verificação:") {
        Some(v) if v >= inicio => (&texto[inicio..v], Some(&texto[v + "verificação:".len()..])),
        _ => (&texto[inicio..], None),
    };
    let enunciado = uma_linha(enunciado.trim());
    if enunciado.is_empty() {
        return None;
    }
    let status = veredito.map(|v| {
        let v = v.trim_start().to_lowercase();
        if v.starts_with("não confere") || v.starts_with("nao confere") {
            StatusPremissa::Invalidada
        } else if v.starts_with("confere em parte") {
            StatusPremissa::NaoValidada
        } else if v.starts_with("confere") {
            StatusPremissa::Validada
        } else {
            StatusPremissa::NaoValidada
        }
    });
    Some((enunciado, status))
}

/// Propostas de uma análise de impacto.
fn propostas_da_analise(analise: &Analise) -> Vec<Proposta> {
    let fonte = FonteProposta::Analise {
        id: analise.id.to_string(),
        titulo: analise.titulo.clone(),
    };
    let mut saida = Vec::new();
    for a in &analise.afirmacoes {
        if matches!(a.estado, EstadoAfirmacao::Incorreta { .. }) {
            continue;
        }
        let referencia = format!("{}/{}", analise.id, a.numero);
        let dados = match a.secao.as_str() {
            SECAO_PREMISSAS => {
                let Some((titulo, veredito)) = premissa_da_afirmacao(&a.texto) else {
                    continue;
                };
                let status = match (a.confianca, veredito) {
                    (Confianca::AConfirmar, _) | (_, None) => StatusPremissa::NaoValidada,
                    (_, Some(s)) => s,
                };
                let como_validada = (status != StatusPremissa::NaoValidada).then(|| {
                    format!(
                        "Verificada no código pela análise {} (afirmação {})",
                        analise.id, a.numero
                    )
                });
                DadosRegistro {
                    titulo,
                    detalhe: Some(a.texto.clone()),
                    quem: Some(SOLICITANTE.into()),
                    status: Some(status),
                    como_validada,
                    ..Default::default()
                }
            }
            SECAO_RISCOS => {
                let titulo = primeira_frase(&a.texto);
                if titulo.is_empty() {
                    continue;
                }
                DadosRegistro {
                    detalhe: (titulo != a.texto.trim()).then(|| a.texto.clone()),
                    titulo,
                    probabilidade: Some(Nivel::Medio),
                    impacto: Some(Nivel::Medio),
                    ..Default::default()
                }
            }
            _ => continue,
        };
        let categoria = if a.secao == SECAO_RISCOS {
            Categoria::Risco
        } else {
            Categoria::Premissa
        };
        saida.push(Proposta {
            referencia,
            categoria,
            fonte: fonte.clone(),
            origem: analise.documento.clone(),
            dados,
        });
    }
    saida
}

/// Propostas de decisão de uma ata.
fn propostas_da_ata(kb: &Projeto, estado: &EstadoAta, ata: &CaminhoRelativo) -> Vec<Proposta> {
    let fonte = FonteProposta::Ata {
        id: estado.id.to_string(),
        data: estado.data.to_string(),
    };
    estado
        .itens_do_tipo(TipoItem::Decisao)
        .filter_map(|item| {
            let titulo = uma_linha(&item.texto);
            if titulo.is_empty() {
                return None;
            }
            let falante = item.citacoes.first().map(|c| c.falante.clone());
            let pessoa = falante.as_ref().and_then(|f| {
                estado
                    .participantes
                    .iter()
                    .find(|p| p.nome == *f)
                    .and_then(|p| p.pessoa.clone())
                    .filter(|slug| kb.pessoa(slug).is_ok())
            });
            let quem = match &pessoa {
                Some(slug) => kb.pessoa(slug).ok().map(|p| p.nome().to_owned()),
                None => falante.map(|f| uma_linha(&f)),
            };
            let falas: Vec<String> = item
                .citacoes
                .iter()
                .map(|c| format!("- {} às {}: \"{}\"", c.falante, c.inicio, c.trecho))
                .collect();
            let detalhe = (!falas.is_empty()).then(|| {
                format!(
                    "Falas de origem na ata {}:\n\n{}",
                    estado.id,
                    falas.join("\n")
                )
            });
            Some(Proposta {
                referencia: format!("{}/{}", estado.id, item.id),
                categoria: Categoria::Decisao,
                fonte: fonte.clone(),
                origem: ata.clone(),
                dados: DadosRegistro {
                    titulo,
                    detalhe,
                    quem,
                    pessoa,
                    decidido_em: Some(estado.data),
                    ..Default::default()
                },
            })
        })
        .collect()
}

/// Demanda a que a análise está ligada (`demanda.id` do frontmatter) ou, na
/// análise de uma reunião, a demanda da ata.
pub fn demanda_da_analise(amb: &Ambiente, analise: &Analise) -> Option<IdDemanda> {
    amb.kb
        .ler::<FrontmatterAnalise>(&analise.documento)
        .ok()
        .and_then(|d| d.frontmatter.demanda.id)
        .or_else(|| {
            analise
                .reuniao
                .and_then(|r| amb.kb.reuniao(r).ok())
                .map(|r| r.demanda)
        })
}

/// Propostas pendentes da demanda: as das análises de impacto e das atas, menos
/// as já aceitas (há registro com a referência) e as recusadas. Análise ou ata
/// ilegível fica de fora, com um aviso no log.
pub fn propostas(amb: &Ambiente, demanda: IdDemanda) -> Result<Vec<Proposta>, ErroRegistros> {
    let kb = &amb.kb;
    let mut tratadas: HashSet<String> = kb
        .registros_da_demanda(demanda)?
        .into_iter()
        .filter_map(|r| r.documento.frontmatter.referencia)
        .collect();
    tratadas.extend(kb.propostas_recusadas(demanda)?);

    let mut saida = Vec::new();
    for analise in amb.analises.listar()? {
        if analise.fluxo == Fluxo::Impacto && demanda_da_analise(amb, &analise) == Some(demanda) {
            saida.extend(propostas_da_analise(&analise));
        }
    }
    for arquivos in kb.reunioes_da_demanda(demanda)? {
        match EstadoAta::ler(kb, arquivos.id) {
            Ok((estado, _)) => saida.extend(propostas_da_ata(kb, &estado, &arquivos.ata)),
            Err(erro) => tracing::warn!(reuniao = %arquivos.id, %erro, "ata ilegível"),
        }
    }
    saida.retain(|p| !tratadas.contains(&p.referencia));
    Ok(saida)
}

fn proposta(
    amb: &Ambiente,
    demanda: IdDemanda,
    referencia: &str,
) -> Result<Proposta, ErroRegistros> {
    let referencia = referencia.trim();
    propostas(amb, demanda)?
        .into_iter()
        .find(|p| p.referencia == referencia)
        .ok_or_else(|| ErroRegistros::PropostaInexistente(referencia.to_owned()))
}

/// Aceita uma proposta pendente: cria o registro com a referência e a origem.
/// `ajustes` substitui os valores sugeridos (o analista mudou o enunciado, a
/// probabilidade, a pessoa…).
pub fn aceitar_proposta(
    amb: &Ambiente,
    demanda: IdDemanda,
    referencia: &str,
    ajustes: Option<DadosRegistro>,
    hoje: Data,
) -> Result<RegistroLido, ErroRegistros> {
    let p = proposta(amb, demanda, referencia)?;
    let novo = NovoRegistro {
        categoria: p.categoria,
        dados: ajustes.unwrap_or(p.dados),
        origem: Some(p.origem),
        evento: format!("aceita a proposta do agente ({})", p.referencia),
        referencia: Some(p.referencia),
    };
    Ok(amb.kb.criar_registro(demanda, &novo, hoje)?)
}

/// Recusa uma proposta pendente: ela não volta a aparecer.
pub fn recusar_proposta(
    amb: &Ambiente,
    demanda: IdDemanda,
    referencia: &str,
) -> Result<(), ErroRegistros> {
    let p = proposta(amb, demanda, referencia)?;
    Ok(amb.kb.recusar_proposta(demanda, &p.referencia)?)
}

/// Valida (ou invalida) uma premissa. Atalho para
/// [`cba_kb::Projeto::validar_premissa`], para quem já tem o ambiente (a pauta de
/// perguntas, #123).
pub fn validar_premissa(
    amb: &Ambiente,
    premissa: IdRegistro,
    validacao: &ValidacaoDePremissa,
    hoje: Data,
) -> Result<RegistroLido, ErroRegistros> {
    Ok(amb.kb.validar_premissa(premissa, validacao, hoje)?)
}

// ---------------------------------------------------------------------------
// Exportação

fn rotulo_status(s: StatusPremissa) -> &'static str {
    match s {
        StatusPremissa::NaoValidada => "não validada",
        StatusPremissa::Validada => "validada",
        StatusPremissa::Invalidada => "invalidada",
    }
}

fn rotulo_nivel(n: Nivel) -> &'static str {
    match n {
        Nivel::Baixo => "baixo",
        Nivel::Medio => "médio",
        Nivel::Alto => "alto",
    }
}

fn rotulo_restricao(t: TipoRestricao) -> &'static str {
    match t {
        TipoRestricao::Prazo => "prazo",
        TipoRestricao::Legal => "legal",
        TipoRestricao::Tecnica => "técnica",
    }
}

/// Uma linha da lista de um registro, com os campos da categoria.
fn linha_do_registro(r: &RegistroLido) -> String {
    let f = r.frontmatter();
    let mut partes: Vec<String> = Vec::new();
    match f.categoria() {
        Categoria::Premissa => {
            partes.push(rotulo_status(f.status.unwrap_or_default()).into());
            if let Some(q) = &f.quem {
                partes.push(format!("assumida por {q}"));
            }
            if let Some(v) = &f.validacao {
                partes.push(format!("como foi validada: {} ({})", v.como, v.em));
            }
        }
        Categoria::Decisao => {
            if let Some(q) = &f.quem {
                partes.push(format!("decidiu {q}"));
            }
            if let Some(d) = f.decidido_em {
                partes.push(format!("em {d}"));
            }
            if !f.alternativas.is_empty() {
                partes.push(format!(
                    "alternativas consideradas: {}",
                    f.alternativas.join("; ")
                ));
            }
        }
        Categoria::Risco => {
            if let (Some(p), Some(i)) = (f.probabilidade, f.impacto) {
                partes.push(format!(
                    "probabilidade {}, impacto {}",
                    rotulo_nivel(p),
                    rotulo_nivel(i)
                ));
            }
            if let Some(r) = &f.resposta {
                partes.push(format!("resposta: {r}"));
            }
            if let Some(q) = &f.quem {
                partes.push(format!("responsável: {q}"));
            }
        }
        Categoria::Restricao => {
            if let Some(t) = f.restricao {
                partes.push(rotulo_restricao(t).into());
            }
            if let Some(q) = &f.quem {
                partes.push(format!("informada por {q}"));
            }
        }
    }
    if let Some(r) = &f.referencia {
        partes.push(format!("origem: {r}"));
    }
    let mut linha = format!("- **{}** {}", r.id, f.titulo);
    if !partes.is_empty() {
        linha.push_str(&format!(" ({})", partes.join("; ")));
    }
    linha
}

/// Seção "Premissas, decisões, riscos e restrições" em Markdown, com os registros
/// da demanda. `None` sem registros.
pub fn secao_markdown(kb: &Projeto, demanda: IdDemanda) -> Result<Option<String>, cba_kb::Erro> {
    let registros = kb.registros_da_demanda(demanda)?;
    if registros.is_empty() {
        return Ok(None);
    }
    let mut texto = format!("## {TITULO_DA_SECAO}\n\nRegistros da demanda {demanda}.\n");
    for (categoria, titulo) in [
        (Categoria::Premissa, "Premissas"),
        (Categoria::Decisao, "Decisões"),
        (Categoria::Risco, "Riscos"),
        (Categoria::Restricao, "Restrições"),
    ] {
        let linhas: Vec<String> = registros
            .iter()
            .filter(|r| r.id.categoria == categoria)
            .map(linha_do_registro)
            .collect();
        if !linhas.is_empty() {
            texto.push_str(&format!("\n### {titulo}\n\n{}\n", linhas.join("\n")));
        }
    }
    Ok(Some(texto))
}

/// Acrescenta a seção dos registros da demanda ao texto exportado de uma análise
/// (ou da ata de uma análise de reunião), antes da seção "Evidências" quando ela
/// existe. Análise sem demanda, ou demanda sem registros, volta como está.
pub fn com_registros(amb: &Ambiente, analise: &Analise, texto: String) -> String {
    let Some(demanda) = demanda_da_analise(amb, analise) else {
        return texto;
    };
    let secao = match secao_markdown(&amb.kb, demanda) {
        Ok(Some(s)) => s,
        Ok(None) => return texto,
        Err(erro) => {
            tracing::warn!(%erro, "registros da demanda fora da exportação");
            return texto;
        }
    };
    let posicao = texto
        .match_indices("\n## Evidências")
        .map(|(i, _)| i + 1)
        .next();
    match posicao {
        Some(i) => format!("{}{secao}\n{}", &texto[..i], &texto[i..]),
        None => {
            let mut saida = texto;
            if !saida.ends_with('\n') {
                saida.push('\n');
            }
            saida.push('\n');
            saida.push_str(&secao);
            saida
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn premissa_do_formato_do_roteiro() {
        let (t, s) = premissa_da_afirmacao(
            "Premissa do solicitante: hoje o projeto não parcela fatura nenhuma. \
             Verificação: não confere. O parcelamento existe para PJ.",
        )
        .unwrap();
        assert_eq!(t, "hoje o projeto não parcela fatura nenhuma.");
        assert_eq!(s, Some(StatusPremissa::Invalidada));
        let (_, s) =
            premissa_da_afirmacao("Premissa do solicitante: x. Verificação: confere em parte.")
                .unwrap();
        assert_eq!(s, Some(StatusPremissa::NaoValidada));
        let (_, s) =
            premissa_da_afirmacao("Premissa do solicitante: x. Verificação: confere.").unwrap();
        assert_eq!(s, Some(StatusPremissa::Validada));
        assert_eq!(
            premissa_da_afirmacao("Premissa do solicitante: x.")
                .unwrap()
                .1,
            None
        );
        assert!(premissa_da_afirmacao("Demanda como entendida: limitar a 3.").is_none());
    }

    #[test]
    fn enunciado_de_uma_linha_e_curto() {
        assert_eq!(primeira_frase("Um risco. Mais detalhe."), "Um risco.");
        let longo = "palavra ".repeat(100);
        let curto = uma_linha(&longo);
        assert!(curto.chars().count() <= MAXIMO_DO_TITULO);
        assert!(curto.ends_with('…'));
        assert_eq!(uma_linha("a\nb"), "a b");
    }
}
