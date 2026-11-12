//! Documento da análise montado a partir do estado (spec, seção 7.3).
//!
//! O estado (`analises/A-NNNN.estado.json`) é a fonte de verdade; o documento
//! (`analises/A-NNNN.md`) é remontado a partir dele, pelo template do tipo, a cada
//! registro. As ferramentas de registro chamam [`remontar`] sob a trava da análise
//! (`Analises::alterar_e`), antes de notificar o app, para o documento ao vivo
//! (#36) mostrar o registro assim que o app recebe o evento.
//!
//! O que entra no documento:
//!
//! - a entrada do analista, na seção da entrada do fluxo ([`secao_da_entrada`]);
//! - as afirmações, menos as marcadas como incorretas pelo analista, com o selo de
//!   confiança e as evidências como notas de rodapé;
//! - na análise de impacto, a abrangência estimada no frontmatter, tirada da
//!   afirmação mais recente da seção `abrangencia` ([`abrangencia_do_texto`]);
//! - as perguntas abertas;
//! - as sugestões que têm documento e não foram recusadas, com link para ele.
//!
//! Os termos propostos ficam no estado e viram glossário na aprovação (#11).

use cba_core::analise::{Analise, Confianca, EstadoAfirmacao, EstadoPergunta, Evidencia, Fluxo};
use cba_docs::{Citacao, Conteudo, Templates, Texto};
use cba_kb::frontmatter::{Abrangencia, FrontmatterAnalise, StatusSugestao};
use cba_kb::{Base, Documento};

/// Seções com papel especial, que não recebem afirmações.
const ESPECIAIS: [&str; 3] = ["perguntas", "sugestoes", "evidencias"];

/// Seção do documento em que entra a entrada do analista: a pergunta ou a demanda
/// como veio. O documento negocial não tem: o título já diz a funcionalidade.
pub fn secao_da_entrada(fluxo: Fluxo) -> Option<&'static str> {
    match fluxo {
        Fluxo::Perguntar => Some("pergunta"),
        Fluxo::Impacto => Some("demanda_como_veio"),
        Fluxo::Explicar => None,
    }
}

/// Seções em que o agente registra afirmações, na ordem do template padrão: as do
/// tipo do documento, menos a da entrada e as especiais (perguntas, sugestões e
/// evidências, que têm ferramenta própria ou são montadas pelo app).
pub fn secoes_de_afirmacao(fluxo: Fluxo) -> Vec<&'static str> {
    let entrada = secao_da_entrada(fluxo);
    cba_docs::modelo(fluxo.tipo_documento())
        .map(|m| {
            m.secoes
                .iter()
                .map(|s| s.marcador)
                .filter(|s| !ESPECIAIS.contains(s) && Some(*s) != entrada)
                .collect()
        })
        .unwrap_or_default()
}

/// Seção da análise de impacto com a estimativa da abrangência.
pub const SECAO_DA_ABRANGENCIA: &str = "abrangencia";

/// A estimativa com que começa o texto de uma afirmação da seção `abrangencia`
/// ("Média: muda o parâmetro e o lote COBOL"), sem diferenciar maiúsculas, com ou
/// sem acento e com ou sem negrito. `None` se o texto não começa por ela.
pub fn abrangencia_do_texto(texto: &str) -> Option<Abrangencia> {
    let inicio = texto.trim_start().trim_start_matches(['*', '_']);
    let palavra: String = inicio
        .chars()
        .take_while(|c| c.is_alphabetic())
        .collect::<String>()
        .to_lowercase();
    match palavra.as_str() {
        "pequena" => Some(Abrangencia::Pequena),
        "média" | "media" => Some(Abrangencia::Media),
        "grande" => Some(Abrangencia::Grande),
        _ => None,
    }
}

/// Citação do documento a partir de uma evidência.
pub fn citacao(e: &Evidencia) -> Citacao {
    Citacao {
        arquivo: e.arquivo.clone(),
        linhas: e.linhas,
        hash: Some(e.hash.clone()),
        commit: e.commit.clone(),
    }
}

fn confianca(c: Confianca) -> cba_docs::Confianca {
    match c {
        Confianca::Confirmado => cba_docs::Confianca::Confirmado,
        Confianca::Inferido => cba_docs::Confianca::Inferido,
        Confianca::AConfirmar => cba_docs::Confianca::AConfirmar,
    }
}

/// Conteúdo do documento da análise, com o frontmatter atual do documento (que
/// guarda analista, demanda e versão do código) e o `status` do estado.
pub fn conteudo(
    analise: &Analise,
    mut frontmatter: FrontmatterAnalise,
) -> Conteudo<FrontmatterAnalise> {
    frontmatter.status = analise.estado.status_documento();
    frontmatter.titulo.clone_from(&analise.titulo);
    if analise.fluxo == Fluxo::Impacto {
        frontmatter.abrangencia = analise
            .afirmacoes
            .iter()
            .rev()
            .filter(|a| !matches!(a.estado, EstadoAfirmacao::Incorreta { .. }))
            .filter(|a| a.secao == SECAO_DA_ABRANGENCIA)
            .find_map(|a| abrangencia_do_texto(&a.texto));
    }
    let mut conteudo = Conteudo::novo(frontmatter, analise.documento.clone());
    if let Some(secao) = secao_da_entrada(analise.fluxo) {
        conteudo
            .textos
            .push(Texto::novo(secao, analise.entrada.clone()));
    }
    conteudo.afirmacoes = analise
        .afirmacoes
        .iter()
        .filter(|a| !matches!(a.estado, EstadoAfirmacao::Incorreta { .. }))
        .map(|a| cba_docs::Afirmacao {
            secao: a.secao.clone(),
            texto: a.texto.clone(),
            confianca: confianca(a.confianca),
            evidencias: a.evidencias.iter().map(citacao).collect(),
        })
        .collect();
    conteudo.perguntas = analise
        .perguntas
        .iter()
        .filter(|p| p.estado == EstadoPergunta::Aberta)
        .map(|p| cba_docs::Pergunta {
            texto: p.texto.clone(),
        })
        .collect();
    conteudo.sugestoes = analise
        .sugestoes
        .iter()
        .filter(|s| s.estado != StatusSugestao::Recusada)
        .filter_map(|s| {
            Some(cba_docs::ReferenciaSugestao {
                id: s.id.clone()?,
                titulo: s.titulo.clone(),
                arquivo: Some(s.arquivo.clone()),
            })
        })
        .collect();
    conteudo
}

/// Remonta e grava o documento da análise a partir do estado, com os templates da
/// base. Os avisos da montagem (marcador desconhecido num template editado) vão
/// para o log, sem conteúdo.
pub fn remontar(base: &Base, kb: &cba_kb::Sistema, analise: &Analise) -> Result<(), String> {
    let atual: Documento<FrontmatterAnalise> =
        kb.ler(&analise.documento).map_err(|e| e.to_string())?;
    let (templates, mut avisos) = Templates::da_base(base);
    let conteudo = conteudo(analise, atual.frontmatter);
    avisos.extend(templates.gravar(kb, &conteudo).map_err(|e| e.to_string())?);
    if !avisos.is_empty() {
        tracing::warn!(avisos = avisos.len(), "avisos na montagem do documento");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secoes_por_fluxo() {
        assert_eq!(
            secoes_de_afirmacao(Fluxo::Perguntar),
            ["resposta", "regras_encontradas", "exemplo"]
        );
        let impacto = secoes_de_afirmacao(Fluxo::Impacto);
        assert_eq!(impacto.first(), Some(&"demanda_como_entendida"));
        assert!(impacto.contains(&"abrangencia"));
        assert!(!impacto.contains(&"demanda_como_veio"));
        let explicar = secoes_de_afirmacao(Fluxo::Explicar);
        assert_eq!(explicar.first(), Some(&"objetivo"));
        for fluxo in [Fluxo::Perguntar, Fluxo::Impacto, Fluxo::Explicar] {
            for s in secoes_de_afirmacao(fluxo) {
                assert!(!ESPECIAIS.contains(&s));
            }
        }
        assert!(secoes_de_afirmacao(Fluxo::Impacto).contains(&SECAO_DA_ABRANGENCIA));
    }

    #[test]
    fn abrangencia_vem_do_inicio_do_texto() {
        for (texto, esperada) in [
            ("Pequena: só o parâmetro.", Some(Abrangencia::Pequena)),
            ("Média: parâmetro e lote COBOL.", Some(Abrangencia::Media)),
            ("media - duas camadas", Some(Abrangencia::Media)),
            ("  **GRANDE**: migração de dados", Some(Abrangencia::Grande)),
            ("A abrangência é pequena.", None),
            ("Pequenas mudanças", None),
            ("", None),
        ] {
            assert_eq!(abrangencia_do_texto(texto), esperada, "{texto}");
        }
    }
}
