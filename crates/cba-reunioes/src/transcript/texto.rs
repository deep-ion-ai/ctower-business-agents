//! Utilidades de texto comuns aos formatos: decodificação dos bytes, finais
//! de linha e entidades.

use std::borrow::Cow;

/// Decodifica os bytes em texto. Aceita UTF-8 (com ou sem BOM) e UTF-16 com
/// BOM (alguns editores do Windows gravam assim). Devolve também se houve
/// trecho inválido, trocado por `�`.
pub(super) fn decodificar(bytes: &[u8]) -> (String, bool) {
    if let Some(resto) = bytes.strip_prefix(b"\xef\xbb\xbf") {
        return utf8(resto);
    }
    if let Some(resto) = bytes.strip_prefix(b"\xff\xfe") {
        return utf16(resto, u16::from_le_bytes);
    }
    if let Some(resto) = bytes.strip_prefix(b"\xfe\xff") {
        return utf16(resto, u16::from_be_bytes);
    }
    utf8(bytes)
}

fn utf8(bytes: &[u8]) -> (String, bool) {
    match String::from_utf8_lossy(bytes) {
        Cow::Borrowed(s) => (s.to_string(), false),
        Cow::Owned(s) => (s, true),
    }
}

fn utf16(bytes: &[u8], unidade: fn([u8; 2]) -> u16) -> (String, bool) {
    let mut invalido = !bytes.len().is_multiple_of(2);
    let unidades = bytes.as_chunks::<2>().0.iter().map(|par| unidade(*par));
    let texto = char::decode_utf16(unidades)
        .map(|c| {
            c.unwrap_or_else(|_| {
                invalido = true;
                char::REPLACEMENT_CHARACTER
            })
        })
        .collect();
    (texto, invalido)
}

/// Tira o BOM e troca `\r\n` e `\r` por `\n`.
pub(super) fn normalizar(texto: &str) -> Cow<'_, str> {
    let texto = texto.strip_prefix('\u{feff}').unwrap_or(texto);
    if texto.contains('\r') {
        Cow::Owned(texto.replace("\r\n", "\n").replace('\r', "\n"))
    } else {
        Cow::Borrowed(texto)
    }
}

/// Resolve as entidades de caractere mais comuns (`&amp;`, `&lt;`, `&gt;`,
/// `&quot;`, `&apos;`, `&nbsp;`, `&lrm;`, `&rlm;` e as numéricas). O que não
/// for reconhecido fica como está.
pub(super) fn decodificar_entidades(texto: &str) -> Cow<'_, str> {
    if !texto.contains('&') {
        return Cow::Borrowed(texto);
    }
    let mut saida = String::with_capacity(texto.len());
    let mut resto = texto;
    while let Some(pos) = resto.find('&') {
        saida.push_str(&resto[..pos]);
        let depois = &resto[pos + 1..];
        let resolvido = depois
            .find(';')
            .filter(|fim| *fim <= 10)
            .and_then(|fim| entidade(&depois[..fim]).map(|c| (c, fim)));
        match resolvido {
            Some((c, fim)) => {
                if let Some(c) = c {
                    saida.push(c);
                }
                resto = &depois[fim + 1..];
            }
            None => {
                saida.push('&');
                resto = depois;
            }
        }
    }
    saida.push_str(resto);
    Cow::Owned(saida)
}

/// Caractere de uma entidade pelo nome (sem `&` e `;`). `Some(None)` quando a
/// entidade é reconhecida mas não produz texto (marcas de direção).
fn entidade(nome: &str) -> Option<Option<char>> {
    let c = match nome {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        "nbsp" => ' ',
        "lrm" | "rlm" => return Some(None),
        _ => {
            let numero = nome.strip_prefix('#')?;
            let codigo = match numero.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => numero.parse().ok()?,
            };
            char::from_u32(codigo)?
        }
    };
    Some(Some(c))
}

/// Aparas e junta as linhas de uma fala, sem as vazias, separadas por `\n`.
pub(super) fn juntar_linhas<'a>(linhas: impl IntoIterator<Item = &'a str>) -> String {
    let mut saida = String::new();
    for linha in linhas.into_iter().map(str::trim).filter(|l| !l.is_empty()) {
        if !saida.is_empty() {
            saida.push('\n');
        }
        saida.push_str(linha);
    }
    saida
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn entidades() {
        assert_eq!(
            decodificar_entidades("a &amp; b &lt;c&gt; &#233; &#xE7; &nbsp;x&lrm;"),
            "a & b <c> é ç  x"
        );
        assert_eq!(decodificar_entidades("P&D; R&D e &"), "P&D; R&D e &");
        assert_eq!(
            decodificar_entidades("&#xFFFFFFFF; &#;"),
            "&#xFFFFFFFF; &#;"
        );
    }

    #[test]
    fn finais_de_linha_e_bom() {
        assert_eq!(normalizar("\u{feff}a\r\nb\rc\n"), "a\nb\nc\n");
    }

    #[test]
    fn utf16_impar_marca_invalido() {
        let (texto, invalido) = decodificar(b"\xff\xfeA\x00B");
        assert_eq!(texto, "A");
        assert!(invalido);
    }
}
