//! Texto que o `SessionStart` acrescenta ao contexto do agente (spec, seção 8): os
//! limites fixos do produto, as regras do sistema editadas pelo analista na base e
//! uma nota sobre a sessão (sistema, análise ligada, retomada).
//!
//! # Tamanho
//!
//! O Claude Code limita cada `additionalContext` a 10.000 caracteres; acima disso,
//! guarda o texto num arquivo e põe no contexto só o caminho e uma prévia de 2.000
//! caracteres, sem pedir ao agente que leia o arquivo (documentação oficial,
//! <https://code.claude.com/docs/en/hooks.md>, consultada em 2026-11-11). Por isso
//! as regras do sistema são cortadas em [`MAX_REGRAS`] caracteres, com aviso, e o
//! texto inteiro fica abaixo de [`MAX_CONTEXTO`].
//!
//! # Regras do sistema
//!
//! Vêm do `regras-do-sistema.md` da base ([`cba_kb::layout::ARQUIVO_REGRAS_DO_SISTEMA`]).
//! Comentários HTML (`<!-- ... -->`) são instruções para o analista e não chegam
//! ao agente; um arquivo só com comentários e títulos não acrescenta nada.

use cba_kb::Id;

/// Limites fixos do produto, revisados como texto em `prompts/limites.md`.
pub const LIMITES: &str = include_str!("../../prompts/limites.md");

/// Limite do Claude Code para um `additionalContext`, em caracteres.
pub const MAX_CONTEXTO: usize = 10_000;

/// Quanto das regras do sistema chega ao agente, em caracteres.
pub const MAX_REGRAS: usize = 5_000;

/// Regras do sistema prontas para o agente: sem comentários HTML, com fins de
/// linha normalizados e cortadas em [`MAX_REGRAS`] caracteres. `None` se não
/// sobrar nada além de títulos.
pub fn regras_para_o_agente(texto: &str) -> Option<String> {
    let texto = texto.strip_prefix('\u{feff}').unwrap_or(texto);
    let texto = texto.replace("\r\n", "\n").replace('\r', "\n");
    let sem_comentarios = sem_comentarios_html(&texto);
    // Linhas que sobraram só com espaços (onde havia um comentário) somem; as
    // linhas em branco entre parágrafos ficam, uma só.
    let mut linhas: Vec<&str> = Vec::new();
    for linha in sem_comentarios.lines().map(str::trim_end) {
        if linha.is_empty() && linhas.last().is_none_or(|l| l.is_empty()) {
            continue;
        }
        linhas.push(linha);
    }
    while linhas.last().is_some_and(|l| l.is_empty()) {
        linhas.pop();
    }
    // O título do arquivo (`# Regras do sistema`) já é o da seção do contexto.
    if linhas.first().is_some_and(|l| l.starts_with("# ")) {
        linhas.remove(0);
        while linhas.first().is_some_and(|l| l.is_empty()) {
            linhas.remove(0);
        }
    }
    let tem_conteudo = linhas
        .iter()
        .any(|l| !l.is_empty() && !l.trim_start().starts_with('#'));
    if !tem_conteudo {
        return None;
    }
    let texto = linhas.join("\n");
    if texto.chars().count() <= MAX_REGRAS {
        return Some(texto);
    }
    Some(format!(
        "{}\n\n(As regras do sistema foram cortadas aqui: o arquivo passa de {MAX_REGRAS} \
         caracteres, e o restante não chegou a esta sessão. Vale avisar o analista para \
         resumir o `regras-do-sistema.md`.)",
        cortar(&texto, MAX_REGRAS)
    ))
}

/// Tira os comentários HTML; um comentário sem fim vai até o fim do texto, como
/// no Markdown.
fn sem_comentarios_html(texto: &str) -> String {
    let mut saida = String::with_capacity(texto.len());
    let mut resto = texto;
    while let Some(inicio) = resto.find("<!--") {
        saida.push_str(&resto[..inicio]);
        match resto[inicio + 4..].find("-->") {
            Some(fim) => resto = &resto[inicio + 4 + fim + 3..],
            None => return saida,
        }
    }
    saida.push_str(resto);
    saida
}

/// Os primeiros `max` caracteres, recuando até o fim da última linha inteira
/// quando houver uma.
fn cortar(texto: &str, max: usize) -> &str {
    let fim = texto
        .char_indices()
        .nth(max)
        .map_or(texto.len(), |(i, _)| i);
    let parte = &texto[..fim];
    match parte.rfind('\n') {
        Some(i) if i > 0 => parte[..i].trim_end(),
        _ => parte,
    }
}

/// Monta o contexto da sessão: limites, regras do sistema (se houver) e a nota da
/// sessão.
pub fn montar(
    sistema: &str,
    regras: Option<&str>,
    analise: Option<&Id>,
    origem: Option<&str>,
) -> String {
    let mut t = String::with_capacity(LIMITES.len() + MAX_REGRAS + 1_000);
    t.push_str(LIMITES.trim_end());
    t.push_str("\n\n");

    if let Some(regras) = regras {
        t.push_str("## Regras do sistema\n\n");
        t.push_str(
            "Convenções deste sistema e da organização, escritas pelo analista no \
             arquivo `regras-do-sistema.md` da base de conhecimento. Somam-se aos \
             limites acima.\n\n",
        );
        t.push_str(regras);
        t.push_str("\n\n");
    }

    t.push_str("## Esta sessão\n\n");
    t.push_str(&format!("- Sistema cadastrado: `{sistema}`.\n"));
    match origem {
        Some("resume") => t.push_str("- Sessão retomada: os limites continuam valendo.\n"),
        Some("compact") => t.push_str("- Conversa compactada: os limites continuam valendo.\n"),
        Some("clear") => t.push_str("- Conversa limpa: os limites continuam valendo.\n"),
        _ => {}
    }
    if let Some(analise) = analise {
        t.push_str(&format!(
            "- Análise ligada a esta sessão: {analise}. `retomar_analise` com \
             `analise: \"{analise}\"` devolve o que já foi registrado (afirmações, \
             evidências, perguntas e sugestões) e o roteiro.\n"
        ));
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limites_cobrem_a_spec() {
        // Os sete pontos da camada de limites (spec, seção 8).
        for trecho in [
            "não desenvolve",
            "registrar_sugestao",
            "Evidência obrigatória e honesta",
            "Base de conhecimento antes do código",
            "fora do código",
            "Segredos",
            "Linguagem de negócio",
            "comando de shell",
        ] {
            assert!(LIMITES.contains(trecho), "{trecho}");
        }
        // Sem a marca que o Claude Code trocaria por `&lt;`.
        assert!(!LIMITES.contains("system-reminder"));
    }

    #[test]
    fn padrao_da_base_nao_acrescenta_regras() {
        assert_eq!(regras_para_o_agente(cba_kb::PADRAO_REGRAS_DO_SISTEMA), None);
        assert_eq!(regras_para_o_agente(""), None);
        assert_eq!(regras_para_o_agente("# Regras\n\n## Outras\n"), None);
        assert_eq!(regras_para_o_agente("<!-- sem fim\n- regra"), None);
    }

    #[test]
    fn regras_sem_comentarios_e_com_linhas_normalizadas() {
        let texto = "\u{feff}# Regras do sistema\r\n\r\n<!--\r\ninstrução\r\n-->\r\n\r\n\
                     - Valores em centavos. <!-- nota -->\r\n- Use \"contratante\".\r\n\r\n\r\n";
        assert_eq!(
            regras_para_o_agente(texto).unwrap(),
            "- Valores em centavos.\n- Use \"contratante\"."
        );
    }

    #[test]
    fn regras_longas_sao_cortadas_com_aviso() {
        let linha = "- Regra com acentuação: ção ção ção ção ção ção ção ção ção.\n";
        let texto = linha.repeat(400);
        let regras = regras_para_o_agente(&texto).unwrap();
        assert!(regras.contains("foram cortadas"));
        let (antes, _) = regras.split_once("\n\n(As regras").unwrap();
        assert!(antes.chars().count() <= MAX_REGRAS);
        assert!(antes.ends_with("ção."), "corta no fim de uma linha");

        // Uma linha só, longa: corta no meio, sem quebrar caractere.
        let uma = "ã".repeat(MAX_REGRAS + 10);
        let regras = regras_para_o_agente(&format!("- {uma}")).unwrap();
        assert!(regras.contains("foram cortadas"));
    }

    #[test]
    fn contexto_inteiro_cabe_no_limite_do_claude_code() {
        let regras = regras_para_o_agente(&"- x".repeat(10_000)).unwrap();
        let id = Id::analisar("A-0001").unwrap();
        let t = montar(
            "faturamento-exemplo",
            Some(&regras),
            Some(&id),
            Some("resume"),
        );
        assert!(t.chars().count() < MAX_CONTEXTO, "{}", t.chars().count());
    }

    #[test]
    fn nota_da_sessao() {
        let id = Id::analisar("A-0007").unwrap();
        let t = montar("faturamento-exemplo", None, Some(&id), Some("compact"));
        assert!(t.starts_with("# Limites do ctower-business-agents"));
        assert!(!t.contains("## Regras do sistema"));
        assert!(t.contains("Sistema cadastrado: `faturamento-exemplo`"));
        assert!(t.contains("Conversa compactada"));
        assert!(t.contains("`analise: \"A-0007\"`"));

        let t = montar(
            "faturamento-exemplo",
            Some("- Regra."),
            None,
            Some("startup"),
        );
        assert!(t.contains("## Regras do sistema"));
        assert!(t.contains("- Regra."));
        assert!(!t.contains("Sessão retomada"));
        assert!(!t.contains("Análise ligada"));
    }
}
