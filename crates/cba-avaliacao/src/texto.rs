//! Comparação de texto por palavras-chave, sem depender de maiúsculas nem de acentos.
//!
//! Critério (o mesmo descrito no gabarito e em `docs/avaliacao.md`):
//!
//! - o texto e os termos são normalizados: minúsculas, sem acentos (á→a, ç→c…);
//! - um termo casa com a palavra inteira (`"12"` não casa com `"120"`) ou, quando
//!   termina em `*`, com o começo de uma palavra (`"parcel*"` casa com
//!   `"parcelamento"`); termos com espaço casam com a sequência inteira;
//! - um texto atende a uma lista de grupos quando **cada grupo** tem ao menos um termo
//!   presente.

/// Minúsculas e sem acentos do português.
pub fn normalizar(texto: &str) -> String {
    texto
        .chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            outro => outro,
        })
        .collect()
}

fn parte_de_palavra(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// O termo aparece no texto já normalizado.
pub fn contem(normalizado: &str, termo: &str) -> bool {
    let termo = normalizar(termo.trim());
    let (termo, prefixo) = match termo.strip_suffix('*') {
        Some(t) => (t.to_owned(), true),
        None => (termo, false),
    };
    if termo.is_empty() {
        return false;
    }
    let comeca_palavra = termo.chars().next().is_some_and(parte_de_palavra);
    let termina_palavra = termo.chars().last().is_some_and(parte_de_palavra);
    normalizado.match_indices(&termo).any(|(i, _)| {
        let antes = normalizado[..i].chars().next_back();
        let depois = normalizado[i + termo.len()..].chars().next();
        let inicio_ok = !comeca_palavra || !antes.is_some_and(parte_de_palavra);
        let fim_ok = prefixo || !termina_palavra || !depois.is_some_and(parte_de_palavra);
        inicio_ok && fim_ok
    })
}

/// Cada grupo tem ao menos um termo no texto. Sem grupos, não atende.
pub fn atende(texto: &str, grupos: &[Vec<String>]) -> bool {
    let normalizado = normalizar(texto);
    !grupos.is_empty()
        && grupos
            .iter()
            .all(|g| g.iter().any(|termo| contem(&normalizado, termo)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grupos(g: &[&[&str]]) -> Vec<Vec<String>> {
        g.iter()
            .map(|x| x.iter().map(|s| s.to_string()).collect())
            .collect()
    }

    #[test]
    fn ignora_acentos_e_maiusculas() {
        assert_eq!(normalizar("Órgão Público, Ação"), "orgao publico, acao");
        assert!(contem(&normalizar("O ÓRGÃO PÚBLICO paga"), "orgao publico"));
    }

    #[test]
    fn palavra_inteira_ou_prefixo() {
        let t = normalizar("Até 12 parcelas no parcelamento; 120 dias");
        assert!(contem(&t, "12"));
        assert!(!contem(&t, "1"));
        assert!(contem(&t, "120"));
        assert!(!contem(&t, "parcel"));
        assert!(contem(&t, "parcel*"));
        assert!(contem(&t, "parcelas"));
        assert!(!contem(&t, "*"));
        assert!(contem(&normalizar("multa de 2% fixa"), "2%"));
        assert!(!contem(&normalizar("multa de 12% fixa"), "2%"));
    }

    #[test]
    fn todos_os_grupos_precisam_de_um_termo() {
        let g = grupos(&[&["lote", "cobol"], &["fix*", "parametr*"]]);
        assert!(atende("O lote COBOL deveria ler o parâmetro?", &g));
        assert!(atende("O lote usa valor fixo?", &g));
        assert!(!atende("O lote roda todo mês?", &g));
        assert!(!atende("qualquer coisa", &[]));
    }
}
