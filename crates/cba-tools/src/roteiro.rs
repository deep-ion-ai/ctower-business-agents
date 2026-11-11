//! Roteiro do fluxo, que chega ao agente pela resposta de `iniciar_analise` e de
//! `retomar_analise` (spec, seção 8: "Roteiro do fluxo").
//!
//! Este é o roteiro padrão embutido, com as variáveis da análise (identificador,
//! entrada e seções do documento). A versão editável na base e os prompts prontos
//! são da #32; aqui ficam só o mínimo para o agente conduzir cada fluxo e as
//! seções que [`concluir_analise`](crate::ciclo::concluir_analise) exige.

use std::fmt::Write as _;

use cba_core::analise::{Analise, Fluxo};

use crate::documento::secoes_de_afirmacao;

/// Nome do fluxo para o agente e para o analista.
pub fn nome_do_fluxo(fluxo: Fluxo) -> &'static str {
    match fluxo {
        Fluxo::Perguntar => "pergunta ao sistema",
        Fluxo::Explicar => "explicação de funcionalidade (documento negocial)",
        Fluxo::Impacto => "análise de impacto de uma demanda",
    }
}

/// Título da seção no template padrão (`regras_encontradas` → "Regras encontradas").
pub fn titulo_da_secao(fluxo: Fluxo, marcador: &str) -> &'static str {
    cba_docs::modelo(fluxo.tipo_documento())
        .and_then(|m| m.secao(marcador))
        .map_or("", |s| s.titulo)
}

/// O que vai em cada seção que recebe afirmações.
fn dica_da_secao(fluxo: Fluxo, marcador: &str) -> &'static str {
    match (fluxo, marcador) {
        (Fluxo::Perguntar, "resposta") => {
            "a resposta direta à pergunta, curta, em linguagem de negócio"
        }
        (Fluxo::Perguntar, "regras_encontradas") => {
            "uma regra de negócio por afirmação, com valores e condições"
        }
        (Fluxo::Perguntar, "exemplo") => {
            "um exemplo com números que aplica as regras (inferido, citando as evidências das regras usadas)"
        }
        (Fluxo::Impacto, "demanda_como_entendida") => {
            "o que a demanda pede de fato; separe o que o solicitante afirmou como fato do que ele supôs e corrija a premissa quando o código mostrar outra coisa"
        }
        (Fluxo::Impacto, "como_e_hoje") => "como o sistema trata o assunto hoje",
        (Fluxo::Impacto, "funcionalidades") => "o que muda em cada funcionalidade",
        (Fluxo::Impacto, "dados") => {
            "tabelas e campos afetados, inclusive dados já gravados no formato atual"
        }
        (Fluxo::Impacto, "integracoes") => "integrações que recebem ou enviam o que muda",
        (Fluxo::Impacto, "documentos_e_telas") => "relatórios, documentos e telas afetados",
        (Fluxo::Impacto, "riscos") => {
            "riscos, como dados antigos, integrações no formato atual e relatórios que agrupam pelos dados alterados"
        }
        (Fluxo::Impacto, "abrangencia") => "pequena, média ou grande, com a justificativa",
        (Fluxo::Explicar, "objetivo") => "para que a funcionalidade serve, para o negócio",
        (Fluxo::Explicar, "atores_e_telas") => "quem usa e por quais telas",
        (Fluxo::Explicar, "fluxo_principal") => "os passos do caminho normal",
        (Fluxo::Explicar, "fluxos_alternativos") => "caminhos alternativos",
        (Fluxo::Explicar, "excecoes") => "erros e situações de exceção",
        (Fluxo::Explicar, "regras") => "uma regra de negócio por afirmação",
        (Fluxo::Explicar, "dados") => "dados lidos e gravados",
        (Fluxo::Explicar, "integracoes") => "sistemas e serviços chamados ou que chamam",
        _ => "",
    }
}

/// Seções que precisam de ao menos uma afirmação para a análise ser concluída: o
/// que o documento do fluxo não pode deixar de dizer.
pub fn secoes_obrigatorias(fluxo: Fluxo) -> &'static [&'static str] {
    match fluxo {
        Fluxo::Perguntar => &["resposta"],
        Fluxo::Impacto => &["demanda_como_entendida", "como_e_hoje", "abrangencia"],
        Fluxo::Explicar => &["objetivo", "fluxo_principal"],
    }
}

/// Orientação própria do fluxo (spec, seções 4.3 a 4.5).
fn orientacao_do_fluxo(fluxo: Fluxo) -> &'static str {
    match fluxo {
        Fluxo::Perguntar => {
            "Responda à pergunta a partir do código: localize os trechos que decidem o \
             comportamento, leia-os e registre cada regra encontrada. A resposta vem \
             antes das regras; o exemplo com números ajuda o solicitante a conferir."
        }
        Fluxo::Explicar => {
            "Descreva a funcionalidade para quem não lê código: objetivo, quem usa, \
             fluxo principal, alternativos e exceções, regras (cada uma com evidência), \
             dados lidos e gravados e integrações."
        }
        Fluxo::Impacto => {
            "Separe o que o solicitante afirmou como fato do que ele supôs (\"acho que\", \
             \"salvo engano\") e verifique cada afirmação dele no código. Descreva como é \
             hoje, levante o que muda (funcionalidades, dados, integrações, documentos e \
             telas), registre os riscos e estime a abrangência com justificativa. Uma boa \
             análise de impacto costuma corrigir a premissa da demanda: deixe isso \
             explícito em demanda_como_entendida."
        }
    }
}

/// Roteiro do fluxo da análise, em Markdown. `documento` é o caminho do documento
/// relativo à base; `contexto` é o contexto da base já formatado.
pub fn roteiro(analise: &Analise, documento: &str, contexto: &str) -> String {
    let id = &analise.id;
    let fluxo = analise.fluxo;
    let mut t = String::new();
    let _ = writeln!(t, "# Roteiro da análise {id}: {}", analise.titulo);
    let _ = writeln!(t);
    let _ = writeln!(
        t,
        "Fluxo: {}. Documento na base: `{documento}` (montado pelo app a partir dos \
         registros; não o edite).",
        nome_do_fluxo(fluxo)
    );
    let _ = writeln!(t);
    let _ = writeln!(t, "## Entrada do analista");
    let _ = writeln!(t);
    for linha in analise.entrada.lines() {
        let _ = writeln!(t, "> {linha}");
    }
    let _ = writeln!(t);
    let _ = writeln!(t, "## Como conduzir");
    let _ = writeln!(t);
    let _ = writeln!(t, "{}", orientacao_do_fluxo(fluxo));
    let _ = writeln!(t);
    let _ = writeln!(
        t,
        "1. Consulte primeiro o contexto da base (abaixo): regras já confirmadas e \
         termos do glossário poupam exploração e não precisam ser redescobertos."
    );
    let _ = writeln!(
        t,
        "2. Leia o código do sistema com as suas ferramentas de leitura e busca. O que \
         não está no código (parâmetros em tabela, jobs agendados, sistemas vizinhos) \
         vira pergunta, não suposição."
    );
    let _ = writeln!(
        t,
        "3. Registre o conteúdo com `afirmar` (analise: \"{id}\"), uma regra ou fato por \
         chamada, com a confiança honesta e as evidências (arquivo relativo à raiz do \
         sistema, linhas e o trecho copiado do código). Evidência recusada volta com o \
         motivo e a correção: corrija e chame de novo. Seções desta análise:"
    );
    for secao in secoes_de_afirmacao(fluxo) {
        let obrigatoria = if secoes_obrigatorias(fluxo).contains(&secao) {
            " (obrigatória)"
        } else {
            ""
        };
        let _ = writeln!(
            t,
            "   - `{secao}` ({}){obrigatoria}: {}",
            titulo_da_secao(fluxo, secao),
            dica_da_secao(fluxo, secao)
        );
    }
    let _ = writeln!(
        t,
        "4. O que o código não responde, ou uma premissa a confirmar, vai para \
         `perguntar`."
    );
    let _ = writeln!(
        t,
        "5. Você não altera o código. Se o analista pedir correção ou mudança, explique \
         o que mudaria e registre com `registrar_sugestao` (trecho atual, trecho \
         proposto, motivo, impacto e um exemplo com números)."
    );
    let _ = writeln!(
        t,
        "6. Termos de negócio que você ligou ao código vão para `propor_termo`."
    );
    let _ = writeln!(
        t,
        "7. Ao terminar, chame `concluir_analise` (analise: \"{id}\"). No chat, dê ao \
         analista só um resumo ({}) e o caminho do documento devolvido; o documento \
         completo fica na base.",
        match fluxo {
            Fluxo::Perguntar => "regras numeradas, um exemplo com números e as perguntas em aberto",
            Fluxo::Explicar => "objetivo, regras principais e as perguntas em aberto",
            Fluxo::Impacto =>
                "demanda como entendida, abrangência, principais riscos e as perguntas para o solicitante",
        }
    );
    let _ = writeln!(t);
    let _ = writeln!(t, "## Contexto da base");
    let _ = writeln!(t);
    let _ = write!(t, "{contexto}");
    t
}

#[cfg(test)]
mod tests {
    use super::*;
    use cba_kb::Serie;

    #[test]
    fn todas_as_secoes_tem_titulo_e_dica_e_as_obrigatorias_existem() {
        for fluxo in [Fluxo::Perguntar, Fluxo::Explicar, Fluxo::Impacto] {
            let secoes = secoes_de_afirmacao(fluxo);
            for s in &secoes {
                assert!(!titulo_da_secao(fluxo, s).is_empty(), "{fluxo:?} {s}");
                assert!(!dica_da_secao(fluxo, s).is_empty(), "{fluxo:?} {s}");
            }
            for s in secoes_obrigatorias(fluxo) {
                assert!(secoes.contains(s), "{fluxo:?} {s}");
            }
        }
    }

    #[test]
    fn roteiro_traz_identificador_entrada_secoes_e_contexto() {
        let a = Analise::nova(
            Serie::Analise.id(3),
            "faturamento-exemplo",
            Fluxo::Perguntar,
            "Multa por atraso",
            "Como é calculada a multa?\nE os juros?",
            1,
        );
        let r = roteiro(&a, "faturamento-exemplo/analises/A-0003.md", "Nada.\n");
        assert!(r.contains("analise: \"A-0003\""), "{r}");
        assert!(
            r.contains("> Como é calculada a multa?\n> E os juros?"),
            "{r}"
        );
        assert!(r.contains("`resposta` (Resposta) (obrigatória)"), "{r}");
        assert!(r.contains("`regras_encontradas`"), "{r}");
        assert!(r.contains("concluir_analise"), "{r}");
        assert!(r.ends_with("## Contexto da base\n\nNada.\n"), "{r}");
    }
}
