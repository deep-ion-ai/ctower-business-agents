//! Dados pessoais do texto da demanda na saída (#454).
//!
//! O texto da demanda entra inteiro em "Demanda como veio", às vezes com o nome,
//! o CPF e o e-mail de quem foi citado no chamado, e o documento é exportado e
//! anexado a outros sistemas. Na exportação (Markdown, Word e HTML) e na
//! publicação, [`mascarar_demanda`] passa as seções da demanda pela mesma
//! máscara dos documentos externos ([`cba_kb::externos::sensivel::mascarar`]:
//! CPF e CNPJ válidos, e-mail, telefone, tokens e credenciais). A base local
//! continua com o texto original: só a saída é mascarada.
//!
//! As seções são reconhecidas pelo título, como na exportação, porque o
//! documento pode ter vindo de um template editado.

use cba_kb::externos::sensivel;

/// Títulos das seções com o texto da demanda.
pub const SECOES_DA_DEMANDA: &[&str] = &["Demanda como veio", "Demanda como entendida"];

/// O documento com os dados pessoais das seções da demanda mascarados e
/// quantos trechos foram. O resto do documento (inclusive o frontmatter) fica
/// como está.
pub fn mascarar_demanda(documento: &str) -> (String, u32) {
    let mut saida = String::with_capacity(documento.len());
    let mut total = 0u32;
    // Nível do título da seção da demanda em que estamos, e o corpo dela até aqui.
    let mut secao: Option<usize> = None;
    let mut corpo = String::new();
    let mut cerca: Option<(char, usize)> = None;
    let mut fechar = |corpo: &mut String, saida: &mut String| {
        let (mascarado, n) = sensivel::mascarar(corpo);
        total = total.saturating_add(n);
        saida.push_str(&mascarado);
        corpo.clear();
    };
    for linha in documento.split_inclusive('\n') {
        let titulo = if cerca.is_none() { titulo(linha) } else { None };
        atualizar_cerca(linha, &mut cerca);
        if let Some((nivel, texto)) = titulo {
            if secao.is_some_and(|s| nivel <= s) {
                fechar(&mut corpo, &mut saida);
                secao = None;
            }
            if secao.is_none() {
                saida.push_str(linha);
                if SECOES_DA_DEMANDA
                    .iter()
                    .any(|s| s.eq_ignore_ascii_case(texto))
                {
                    secao = Some(nivel);
                }
                continue;
            }
        }
        if secao.is_some() {
            corpo.push_str(linha);
        } else {
            saida.push_str(linha);
        }
    }
    if secao.is_some() {
        fechar(&mut corpo, &mut saida);
    }
    (saida, total)
}

/// Nível e texto de um título ATX (`## Demanda como veio`).
fn titulo(linha: &str) -> Option<(usize, &str)> {
    let l = linha.trim_end_matches(['\n', '\r']);
    let sem_recuo = l.strip_prefix("   ").or_else(|| l.strip_prefix("  "));
    let l = sem_recuo.or_else(|| l.strip_prefix(' ')).unwrap_or(l);
    let nivel = l.bytes().take_while(|b| *b == b'#').count();
    if !(1..=6).contains(&nivel) {
        return None;
    }
    let resto = &l[nivel..];
    if !resto.is_empty() && !resto.starts_with([' ', '\t']) {
        return None;
    }
    let texto = resto.trim().trim_end_matches('#').trim();
    Some((nivel, texto))
}

/// Abre ou fecha um bloco de código cercado (```` ``` ```` ou `~~~`): dentro
/// dele, `#` não é título.
fn atualizar_cerca(linha: &str, cerca: &mut Option<(char, usize)>) {
    let l = linha.trim_start_matches(' ');
    let Some(c) = l.chars().next().filter(|c| *c == '`' || *c == '~') else {
        return;
    };
    let n = l.chars().take_while(|x| *x == c).count();
    if n < 3 {
        return;
    }
    match *cerca {
        None => *cerca = Some((c, n)),
        Some((aberta, m)) if aberta == c && n >= m && l[n..].trim().is_empty() => *cerca = None,
        Some(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOCUMENTO: &str = "\
---
id: A-0001
---
# A-0001 · Cobrança

## Demanda como veio

Cliente Ana Souza, CPF 529.982.247-25, empresa 11.222.333/0001-81,
e-mail ana.souza@example.com, telefone (11) 98765-4321.

### Anexo

Contato do gerente: (21) 3456-7890.

## Demanda como entendida

Premissa: o CPF 529.982.247-25 está na fatura.

## Como é hoje

O suporte atende em (11) 3456-7890; ver ana@example.com.

```text
## Demanda como veio
fora de seção: 529.982.247-25
```
";

    #[test]
    fn mascara_so_as_secoes_da_demanda() {
        let (saida, n) = mascarar_demanda(DOCUMENTO);
        assert_eq!(n, 6, "{saida}");
        assert!(saida.contains("CPF [CPF mascarado], empresa [CNPJ mascarado],"));
        assert!(saida.contains("e-mail [e-mail mascarado], telefone [telefone mascarado]."));
        assert!(saida.contains("gerente: [telefone mascarado]."), "{saida}");
        assert!(
            saida.contains("Premissa: o CPF [CPF mascarado] está"),
            "{saida}"
        );
        // Fora da demanda e no bloco de código, o texto fica.
        assert!(saida.contains("atende em (11) 3456-7890; ver ana@example.com."));
        assert!(saida.contains("fora de seção: 529.982.247-25"), "{saida}");
        assert!(saida.starts_with("---\nid: A-0001\n---\n# A-0001 · Cobrança\n"));
        assert!(saida.contains("## Demanda como veio\n\nCliente Ana Souza,"));
    }

    #[test]
    fn sem_secao_da_demanda_nada_muda() {
        let texto = "# Regra\n\nContato (11) 3456-7890.\n";
        assert_eq!(mascarar_demanda(texto), (texto.to_owned(), 0));
    }

    #[test]
    fn titulos() {
        assert_eq!(
            titulo("## Demanda como veio\n"),
            Some((2, "Demanda como veio"))
        );
        assert_eq!(titulo("  ### Anexo ###\r\n"), Some((3, "Anexo")));
        assert_eq!(titulo("#hashtag\n"), None);
        assert_eq!(titulo("####### sete\n"), None);
    }
}
