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
//! - as sugestões que têm documento e não foram recusadas, com link para ele;
//! - os diagramas registrados ([`diagrama`]), que a montagem transforma em
//!   Mermaid, com legenda e tabela de elementos.
//!
//! - as evidências de dados (#440): o resumo entra no texto da afirmação, com uma
//!   nota de rodapé (`[^dados-n]`) que leva a conexão, o ambiente, a data e hora, o
//!   SQL e as linhas; as notas ficam na seção de evidências, avisando que valem
//!   para a data em que foram tiradas. A exportação sem evidências tira as notas
//!   e fica só o resumo.
//! - os commits citados como origem da regra (#450): "(origem: commit <hash curto>,
//!   <chaves>)" no texto da afirmação, com a nota de rodapé (`[^origem-<hash>]`)
//!   que leva o hash, a data e a mensagem, na seção de evidências. O autor do
//!   commit fica só no estado.
//!
//! Os termos propostos ficam no estado e viram glossário na aprovação (#11).
//!
//! No parecer de revisão (fluxo `revisar`), o veredito, os achados e a
//! verificação INVEST vêm do parecer do estado ([`crate::parecer::textos`]); os
//! achados descartados pelo analista ficam de fora.

use cba_core::analise::{
    Analise, Confianca, EstadoAfirmacao, EstadoPergunta, Evidencia, EvidenciaOrigem, Fluxo,
};
use cba_core::consultas::EvidenciaDados;
use cba_docs::{Citacao, Conteudo, Templates, Texto};
use cba_kb::frontmatter::{Abrangencia, FrontmatterAnalise, StatusSugestao};
use cba_kb::{Base, Documento};

/// Seções com papel especial, que não recebem afirmações.
const ESPECIAIS: [&str; 3] = ["perguntas", "sugestoes", "evidencias"];

/// Seções montadas a partir de registros próprios do fluxo, que também não
/// recebem afirmações (o parecer de revisão e as histórias têm ferramentas
/// próprias).
fn secoes_estruturadas(fluxo: Fluxo) -> &'static [&'static str] {
    match fluxo {
        Fluxo::Revisar => crate::parecer::SECOES,
        Fluxo::Historias => &[cba_docs::HISTORIAS, cba_docs::RASTREABILIDADE],
        Fluxo::Cenarios => &[
            cba_docs::TABELAS_DECISAO,
            cba_docs::VALORES_LIMITE,
            cba_docs::CENARIOS,
            cba_docs::RASTREABILIDADE,
        ],
        Fluxo::Manual => &[cba_docs::PASSO_A_PASSO],
        Fluxo::Prototipo => &[cba_docs::TELAS, cba_docs::RASTREABILIDADE],
        _ => &[],
    }
}

/// Seção do documento em que entra a entrada do analista: a pergunta, a demanda
/// como veio, a reunião analisada ou o pedido de histórias. O documento negocial
/// não tem: o título já diz a funcionalidade.
pub fn secao_da_entrada(fluxo: Fluxo) -> Option<&'static str> {
    match fluxo {
        Fluxo::Perguntar => Some("pergunta"),
        Fluxo::Impacto => Some("demanda_como_veio"),
        Fluxo::Reuniao => Some("reuniao"),
        Fluxo::Revisar => Some("requisito"),
        Fluxo::Historias | Fluxo::Cenarios | Fluxo::Prototipo => Some("pedido"),
        Fluxo::Explicar | Fluxo::Manual => None,
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
                .filter(|s| {
                    !ESPECIAIS.contains(s)
                        && !secoes_estruturadas(fluxo).contains(s)
                        && Some(*s) != entrada
                })
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

/// Diagrama do estado como a montagem recebe.
pub fn diagrama(d: &cba_core::diagramas::Diagrama) -> cba_docs::Diagrama {
    use cba_core::diagramas::{Multiplicidade, OrigemRelacao, PapelNo, TipoDiagrama};
    let multiplicidade = |m: Multiplicidade| match m {
        Multiplicidade::ZeroOuUm => cba_docs::Multiplicidade::ZeroOuUm,
        Multiplicidade::Um => cba_docs::Multiplicidade::Um,
        Multiplicidade::ZeroOuMuitos => cba_docs::Multiplicidade::ZeroOuMuitos,
        Multiplicidade::UmOuMuitos => cba_docs::Multiplicidade::UmOuMuitos,
    };
    cba_docs::Diagrama {
        secao: d.secao.clone(),
        tipo: match d.tipo {
            TipoDiagrama::Estados => cba_docs::TipoDiagrama::Estados,
            TipoDiagrama::Fluxo => cba_docs::TipoDiagrama::Fluxo,
            TipoDiagrama::Dados => cba_docs::TipoDiagrama::Dados,
        },
        titulo: d.titulo.clone(),
        nos: d
            .nos
            .iter()
            .map(|n| cba_docs::NoDiagrama {
                chave: n.chave.clone(),
                rotulo: n.rotulo.clone(),
                papel: match n.papel {
                    PapelNo::Etapa => cba_docs::PapelNo::Etapa,
                    PapelNo::Inicio => cba_docs::PapelNo::Inicio,
                    PapelNo::Fim => cba_docs::PapelNo::Fim,
                    PapelNo::Decisao => cba_docs::PapelNo::Decisao,
                },
                descricao: n.descricao.clone(),
                evidencias: n.evidencias.iter().map(citacao).collect(),
                raia: n.raia.clone(),
                entidade: n.entidade.as_ref().map(|e| cba_docs::EntidadeDiagrama {
                    tabela: e.tabela.clone(),
                    atributos: e
                        .atributos
                        .iter()
                        .map(|a| cba_docs::AtributoDiagrama {
                            nome: a.nome.clone(),
                            coluna: a.coluna.clone(),
                        })
                        .collect(),
                    termo: e.termo.clone(),
                }),
            })
            .collect(),
        arestas: d
            .arestas
            .iter()
            .map(|a| cba_docs::ArestaDiagrama {
                de: a.de.clone(),
                para: a.para.clone(),
                rotulo: a.rotulo.clone(),
                descricao: a.descricao.clone(),
                condicao: a.condicao.clone(),
                efeitos: a.efeitos.clone(),
                confianca: confianca(a.confianca),
                evidencias: a.evidencias.iter().map(citacao).collect(),
                relacao: a.relacao.map(|r| cba_docs::RelacaoDiagrama {
                    de: multiplicidade(r.de),
                    para: multiplicidade(r.para),
                    origem: match r.origem {
                        OrigemRelacao::ChaveEstrangeira => {
                            cba_docs::OrigemRelacao::ChaveEstrangeira
                        }
                        OrigemRelacao::Join => cba_docs::OrigemRelacao::Join,
                    },
                }),
            })
            .collect(),
        achados: d.achados().into_iter().map(|a| a.texto).collect(),
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
    if analise.origem.is_some() {
        frontmatter.origem.clone_from(&analise.origem);
    }
    if analise.fluxo == Fluxo::Impacto {
        frontmatter.abrangencia = analise
            .afirmacoes
            .iter()
            .rev()
            .filter(|a| !matches!(a.estado, EstadoAfirmacao::Incorreta { .. }))
            .filter(|a| a.secao == SECAO_DA_ABRANGENCIA)
            .find_map(|a| abrangencia_do_texto(&a.texto));
    }
    let fonte = (analise.fluxo == Fluxo::Revisar)
        .then(|| crate::parecer::fonte(&frontmatter.demanda))
        .flatten();
    let mut conteudo = Conteudo::novo(frontmatter, analise.documento.clone());
    if let Some(fonte) = fonte {
        conteudo.textos.push(Texto::novo("requisito", fonte));
    }
    if let Some(secao) = secao_da_entrada(analise.fluxo) {
        conteudo
            .textos
            .push(Texto::novo(secao, analise.entrada.clone()));
    }
    if analise.fluxo == Fluxo::Revisar {
        conteudo.textos.extend(crate::parecer::textos(analise));
    }
    if analise.fluxo == Fluxo::Historias {
        conteudo
            .textos
            .extend(crate::historias::textos(analise, &analise.documento));
    }
    if analise.fluxo == Fluxo::Cenarios {
        conteudo
            .textos
            .extend(crate::cenarios::textos(analise, &analise.documento));
    }
    if analise.fluxo == Fluxo::Manual {
        conteudo
            .textos
            .push(crate::manual::texto(analise, &analise.documento));
    }
    if analise.fluxo == Fluxo::Prototipo {
        conteudo
            .textos
            .extend(crate::prototipo::textos(analise, &analise.documento));
    }
    let mut notas_de_dados: Vec<String> = Vec::new();
    let mut citadas: Vec<u32> = Vec::new();
    let mut notas_de_origem: Vec<String> = Vec::new();
    let mut commits_citados: Vec<String> = Vec::new();
    conteudo.afirmacoes = analise
        .afirmacoes
        .iter()
        .filter(|a| !matches!(a.estado, EstadoAfirmacao::Incorreta { .. }))
        .map(|a| {
            let mut texto = a.texto.clone();
            for d in &a.dados {
                texto.push_str(&em_dados(d));
                if !citadas.contains(&d.consulta) {
                    citadas.push(d.consulta);
                    notas_de_dados.push(nota_de_dados(d));
                }
            }
            for o in &a.origens {
                texto.push_str(&em_origem(o));
                if !commits_citados.contains(&o.commit) {
                    commits_citados.push(o.commit.clone());
                    notas_de_origem.push(nota_de_origem(o));
                }
            }
            cba_docs::Afirmacao {
                secao: a.secao.clone(),
                texto,
                confianca: confianca(a.confianca),
                evidencias: a.evidencias.iter().map(citacao).collect(),
            }
        })
        .collect();
    if !notas_de_dados.is_empty() {
        conteudo.textos.push(Texto::novo(
            "evidencias",
            format!(
                "**Evidências de dados.** Consultas ao banco aprovadas pelo analista. Cada \
                 uma vale para a data e hora em que foi tirada: a revalidação confere o \
                 código, não reexecuta consultas.\n\n{}",
                notas_de_dados.join("\n\n")
            ),
        ));
    }
    if !notas_de_origem.is_empty() {
        conteudo.textos.push(Texto::novo(
            "evidencias",
            format!(
                "**Origem das regras.** Commits que criaram ou mudaram o trecho, achados no \
                 histórico do código. Complementam as evidências de código: dizem qual \
                 demanda pediu o comportamento.\n\n{}",
                notas_de_origem.join("\n\n")
            ),
        ));
    }
    conteudo.diagramas = analise.diagramas.iter().map(diagrama).collect();
    conteudo.perguntas = analise
        .perguntas
        .iter()
        .filter(|p| p.estado == EstadoPergunta::Aberta)
        .map(|p| cba_docs::Pergunta {
            texto: p.texto.clone(),
            evidencias: p.evidencias.iter().map(citacao).collect(),
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

/// Data e hora de uma evidência de dados (`2026-11-26 14:03 UTC`).
fn quando(ms: u64) -> String {
    let iso = cba_core::tempo::instante_iso(ms);
    format!("{} {} UTC", &iso[..10], &iso[11..16])
}

/// O resumo de uma evidência de dados no texto da afirmação, com a nota de
/// rodapé: a exportação sem evidências tira a nota e fica só o resumo.
fn em_dados(d: &EvidenciaDados) -> String {
    format!(
        " (em dados, {}: {})[^dados-{}]",
        &quando(d.executada_ms)[..10],
        d.resumo.trim(),
        d.consulta
    )
}

/// Hash curto de um commit citado como origem.
fn commit_curto(o: &EvidenciaOrigem) -> &str {
    o.commit.get(..10).unwrap_or(&o.commit)
}

/// A origem no texto da afirmação, com a nota de rodapé: o commit curto e as
/// chaves de issue da mensagem. A exportação sem evidências tira a nota.
fn em_origem(o: &EvidenciaOrigem) -> String {
    let curto = commit_curto(o);
    let chaves = if o.chaves.is_empty() {
        String::new()
    } else {
        format!(", {}", o.chaves.join(", "))
    };
    format!(" (origem: commit {curto}{chaves})[^origem-{curto}]")
}

/// A nota de rodapé da origem: hash, data e mensagem. O autor do commit não entra
/// no documento.
fn nota_de_origem(o: &EvidenciaOrigem) -> String {
    format!(
        "[^origem-{curto}]: Origem, commit {hash} de {data}: {assunto}",
        curto = commit_curto(o),
        hash = codigo_em_linha(&o.commit),
        data = o.data,
        assunto = codigo_em_linha(o.assunto.trim()),
    )
}

/// Código em linha com uma cerca de crases maior que qualquer sequência dentro.
fn codigo_em_linha(texto: &str) -> String {
    let maior = texto.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    let cerca = "`".repeat(maior + 1);
    format!("{cerca} {texto} {cerca}")
}

/// A nota de rodapé da evidência de dados: conexão, ambiente, data e hora, SQL e
/// quantidade de linhas.
fn nota_de_dados(d: &EvidenciaDados) -> String {
    let sql = d.sql.split_whitespace().collect::<Vec<_>>().join(" ");
    let quando = quando(d.executada_ms);
    format!(
        "[^dados-{n}]: Evidência de dados, consulta {n} em {origem}, {quando}: {linhas} \
         linha(s). SQL: {sql}. Vale para {dia}.",
        n = d.consulta,
        origem = d.origem,
        linhas = d.linhas,
        sql = codigo_em_linha(&sql),
        dia = &quando[..10],
    )
}

/// Remonta e grava o documento da análise a partir do estado, com os templates da
/// base. Os avisos da montagem (marcador desconhecido num template editado) vão
/// para o log, sem conteúdo.
pub fn remontar(base: &Base, kb: &cba_kb::Projeto, analise: &Analise) -> Result<(), String> {
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
        assert_eq!(secoes_de_afirmacao(Fluxo::Revisar), ["contexto_no_codigo"]);
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
