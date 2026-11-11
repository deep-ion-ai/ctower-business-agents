//! Garante que as fixtures não trazem nomes nem dados de sistemas reais.
//!
//! A lista de termos não é exaustiva: pega os nomes mais prováveis de
//! escapar para um exemplo (bancos, órgãos públicos, grandes empresas e
//! ERPs). A revisão humana continua necessária.

mod comum;

use std::fs;

/// Nomes de empresas, órgãos e sistemas reais que não podem aparecer nas
/// fixtures. Comparação sem acentos, sem diferenciar maiúsculas e por
/// palavra inteira (termos com várias palavras exigem a sequência exata).
const TERMOS_PROIBIDOS: &[&str] = &[
    // Bancos e meios de pagamento
    "banco do brasil",
    "bradesco",
    "itau",
    "unibanco",
    "santander",
    "caixa economica",
    "nubank",
    "banrisul",
    "sicoob",
    "sicredi",
    "btg",
    "bndes",
    "cielo",
    "pagseguro",
    "mercado pago",
    "febraban",
    // Órgãos e empresas públicas
    "receita federal",
    "serpro",
    "dataprev",
    "inss",
    "sefaz",
    "detran",
    "correios",
    "petrobras",
    "eletrobras",
    "sabesp",
    "cemig",
    "copel",
    "banco central",
    // Birôs de crédito
    "serasa",
    "boa vista",
    "spc brasil",
    // ERPs e fornecedores de sistemas de gestão
    "totvs",
    "protheus",
    "datasul",
    "sankhya",
    "senior sistemas",
    "sap",
    "netsuite",
    "dynamics",
    "salesforce",
    "linx",
    "omie",
    "bling",
    // Grandes empresas e operadoras
    "vale s.a",
    "ambev",
    "embratel",
    "telefonica",
    "magazine luiza",
    "americanas",
];

/// Minúsculas, sem acentos, com tudo que não é letra ou dígito virando
/// espaço, e com espaços nas pontas para comparar palavras inteiras.
fn normalizar(texto: &str) -> String {
    let mut saida = String::with_capacity(texto.len() + 2);
    saida.push(' ');
    let mut ultimo_espaco = true;
    for ch in texto.chars().flat_map(char::to_lowercase) {
        let ch = match ch {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            outro => outro,
        };
        if ch.is_ascii_alphanumeric() {
            saida.push(ch);
            ultimo_espaco = false;
        } else if !ultimo_espaco {
            saida.push(' ');
            ultimo_espaco = true;
        }
    }
    if !ultimo_espaco {
        saida.push(' ');
    }
    saida
}

#[test]
fn fixtures_nao_citam_empresas_nem_sistemas_reais() {
    let termos: Vec<(&str, String)> = TERMOS_PROIBIDOS
        .iter()
        .map(|t| (*t, normalizar(t)))
        .collect();
    let mut achados = Vec::new();
    for arquivo in comum::arquivos_recursivos(&comum::pasta_fixtures()) {
        let texto = fs::read_to_string(&arquivo)
            .unwrap_or_else(|e| panic!("{} não é texto UTF-8: {e}", arquivo.display()));
        for (numero, linha) in texto.lines().enumerate() {
            let normalizada = normalizar(linha);
            for (termo, termo_normalizado) in &termos {
                if normalizada.contains(termo_normalizado.as_str()) {
                    achados.push(format!(
                        "{}:{}: termo proibido `{termo}`",
                        arquivo.display(),
                        numero + 1
                    ));
                }
            }
        }
    }
    assert!(achados.is_empty(), "\n{}", achados.join("\n"));
}

/// Sequências com formato de CPF (`000.000.000-00` ou 11 dígitos) ou CNPJ
/// (`00.000.000/0000-00` ou 14 dígitos) encontradas no texto.
fn documentos(texto: &str) -> Vec<String> {
    const CPF: &str = "ddd.ddd.ddd-dd";
    const CNPJ: &str = "dd.ddd.ddd/dddd-dd";
    let encaixa = |token: &str, molde: &str| {
        token.len() == molde.len()
            && token.chars().zip(molde.chars()).all(|(t, m)| match m {
                'd' => t.is_ascii_digit(),
                _ => t == m,
            })
    };
    texto
        .split(|c: char| !(c.is_ascii_digit() || matches!(c, '.' | '/' | '-')))
        .map(|token| token.trim_matches(|c| matches!(c, '.' | '/' | '-')))
        .filter(|token| {
            encaixa(token, CPF)
                || encaixa(token, CNPJ)
                || (token.chars().all(|c| c.is_ascii_digit()) && matches!(token.len(), 11 | 14))
        })
        .map(str::to_string)
        .collect()
}

/// Pesos do segundo dígito verificador; os do primeiro são os mesmos sem o
/// primeiro elemento.
const PESOS_CPF: [u32; 10] = [11, 10, 9, 8, 7, 6, 5, 4, 3, 2];
const PESOS_CNPJ: [u32; 13] = [6, 5, 4, 3, 2, 9, 8, 7, 6, 5, 4, 3, 2];

fn digito(soma: u32) -> u32 {
    let resto = soma % 11;
    if resto < 2 { 0 } else { 11 - resto }
}

fn documento_valido(documento: &str) -> bool {
    let d: Vec<u32> = documento.chars().filter_map(|c| c.to_digit(10)).collect();
    if d.iter().all(|x| *x == d[0]) {
        return false;
    }
    let soma =
        |n: usize, pesos: &[u32]| -> u32 { d[..n].iter().zip(pesos).map(|(a, b)| a * b).sum() };
    match d.len() {
        11 => digito(soma(9, &PESOS_CPF[1..])) == d[9] && digito(soma(10, &PESOS_CPF)) == d[10],
        14 => digito(soma(12, &PESOS_CNPJ[1..])) == d[12] && digito(soma(13, &PESOS_CNPJ)) == d[13],
        _ => false,
    }
}

/// Completa uma base com os dígitos verificadores, para montar exemplos
/// válidos no teste sem escrever nenhum documento no código.
fn com_digitos(base: &[u32], pesos1: &[u32], pesos2: &[u32]) -> String {
    let mut d = base.to_vec();
    let soma = |d: &[u32], pesos: &[u32]| -> u32 { d.iter().zip(pesos).map(|(a, b)| a * b).sum() };
    d.push(digito(soma(&d, pesos1)));
    d.push(digito(soma(&d, pesos2)));
    d.iter()
        .map(|x| char::from_digit(*x, 10).unwrap())
        .collect()
}

#[test]
fn validacao_de_documentos_reconhece_exemplos() {
    let cpf = com_digitos(&[1, 2, 3, 4, 5, 6, 7, 8, 9], &PESOS_CPF[1..], &PESOS_CPF);
    let cnpj = com_digitos(
        &[1, 2, 3, 4, 5, 6, 7, 8, 0, 0, 0, 1],
        &PESOS_CNPJ[1..],
        &PESOS_CNPJ,
    );
    assert!(documento_valido(&cpf));
    assert!(documento_valido(&cnpj));
    assert!(!documento_valido("000.000.000-00"));
    assert!(!documento_valido("123.456.789-00"));
    let formatado = format!("{}.{}.{}-{}", &cpf[..3], &cpf[3..6], &cpf[6..9], &cpf[9..]);
    assert_eq!(
        documentos(&format!("doc {formatado}, x")),
        vec![formatado.clone()]
    );
    assert_eq!(documentos(&format!("doc {cnpj};")), vec![cnpj.clone()]);
}

#[test]
fn fixtures_so_tem_cpf_e_cnpj_invalidos() {
    let mut achados = Vec::new();
    for arquivo in comum::arquivos_recursivos(&comum::pasta_fixtures()) {
        let texto = fs::read_to_string(&arquivo).expect("texto UTF-8");
        for documento in documentos(&texto) {
            if documento_valido(&documento) {
                achados.push(format!(
                    "{}: `{documento}` é um CPF ou CNPJ válido; use um número inválido",
                    arquivo.display()
                ));
            }
        }
    }
    assert!(achados.is_empty(), "\n{}", achados.join("\n"));
}
