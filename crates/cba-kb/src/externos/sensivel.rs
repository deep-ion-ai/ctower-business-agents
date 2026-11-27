//! Segredos e dados pessoais nos documentos externos (#277), sem IA.
//!
//! A documentação de um repositório não passou pelo app: pode trazer uma senha
//! colada num exemplo, um token, o CPF de um cliente real. Antes de a cópia entrar
//! na base (e daí no índice e no contexto do agente):
//!
//! - **arquivo com chave privada** (`-----BEGIN ... PRIVATE KEY-----`) fica de fora
//!   inteiro ([`tem_chave_privada`]);
//! - **trechos** são mascarados ([`mascarar`]): credencial em URL
//!   (`https://ana:<senha>@host`), tokens com formato conhecido (GitHub, GitLab,
//!   AWS, Slack, JWT), valor de campo de senha ou segredo (`senha: <valor>`, só
//!   quando o valor tem dígito e 8 caracteres ou mais, para não mascarar texto
//!   explicativo), CPF e CNPJ com dígitos verificadores válidos (formatados:
//!   `529.982.247-25`, `11.222.333/0001-81`), endereço de e-mail e telefone
//!   (com DDD entre parênteses ou depois de `+55`, DDD e celular, ou só o
//!   celular com o 9 e o hífen: `(11) 3456-7890`, `+55 11 98765-4321`,
//!   `98765-4321`). Número solto, valor, data e código (`1234-5678`,
//!   `R$ 1.234,56`, `2026-11-26`) ficam.
//!
//! Também mascara o texto da demanda na exportação e na publicação dos
//! documentos (#454, `cba_docs::mascarar_demanda`).
//!
//! Como as exclusões de segredo do inventário (`cba_index::exclusoes`) e o
//! indício de dado pessoal no schema (`cba_tools::dados_pessoais`), é uma
//! heurística conservadora: reduz o risco, não prova que o texto está limpo.

use std::sync::LazyLock;

use regex::{Captures, Regex};

/// O que fica no lugar de um segredo.
pub const SEGREDO: &str = "[segredo mascarado]";
/// O que fica no lugar de um CPF.
pub const CPF: &str = "[CPF mascarado]";
/// O que fica no lugar de um e-mail.
pub const EMAIL: &str = "[e-mail mascarado]";
/// O que fica no lugar de um CNPJ.
pub const CNPJ: &str = "[CNPJ mascarado]";
/// O que fica no lugar de um telefone.
pub const TELEFONE: &str = "[telefone mascarado]";

static CHAVE_PRIVADA: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----").expect("regex fixa"));

static CREDENCIAL_EM_URL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b([a-z][a-z0-9+.-]*://[^\s:/@]+):([^\s@/]+)@").expect("regex fixa")
});

static TOKEN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"\b(?:",
        r"gh[pousr]_[A-Za-z0-9]{20,}",
        r"|github_pat_[A-Za-z0-9_]{20,}",
        r"|glpat-[A-Za-z0-9_-]{20,}",
        r"|AKIA[0-9A-Z]{16}",
        r"|xox[abprs]-[A-Za-z0-9-]{10,}",
        r"|eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}",
        r")"
    ))
    .expect("regex fixa")
});

static CAMPO_DE_SEGREDO: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?i)\b(password|passwd|pwd|senha|secret|segredo|token|api[_-]?key|",
        r"client[_-]?secret|access[_-]?key|private[_-]?key)\b",
        r#"(\s*[:=]\s*)(["'`]?)([^\s"'`]{8,})"#
    ))
    .expect("regex fixa")
});

static CPF_FORMATADO: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b\d{3}\.\d{3}\.\d{3}-\d{2}\b").expect("regex fixa"));

static CNPJ_FORMATADO: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b\d{2}\.\d{3}\.\d{3}/\d{4}-\d{2}\b").expect("regex fixa"));

/// Telefone brasileiro: DDD entre parênteses (com ou sem `+55`), `+55` e DDD, DDD
/// e celular (`11 98765-4321`) ou só o celular com o hífen (`98765-4321`). O
/// número sem DDD nem 9 na frente (`1234-5678`) fica: pode ser um código.
static TELEFONE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?:\+55\s?)?\([1-9]\d\)\s?9?\d{4}[-\s]?\d{4}\b",
        r"|\+55\s?[1-9]\d\s?9?\d{4}[-\s]?\d{4}\b",
        r"|\b[1-9]\d\s9\d{4}-\d{4}\b",
        r"|\b9\d{4}-\d{4}\b",
    ))
    .expect("regex fixa")
});

static EMAIL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)*\.[A-Za-z]{2,}\b")
        .expect("regex fixa")
});

/// Se o texto traz uma chave privada: o arquivo não é importado.
pub fn tem_chave_privada(texto: &str) -> bool {
    CHAVE_PRIVADA.is_match(texto)
}

/// O texto com os trechos sensíveis mascarados e quantos foram.
pub fn mascarar(texto: &str) -> (String, u32) {
    let mut total = 0u32;
    let mut conta = |n: usize| total += u32::try_from(n).unwrap_or(u32::MAX);

    let n = CREDENCIAL_EM_URL.find_iter(texto).count();
    conta(n);
    let texto = CREDENCIAL_EM_URL.replace_all(texto, format!("$1:{SEGREDO}@"));

    let n = TOKEN.find_iter(&texto).count();
    conta(n);
    let texto = TOKEN.replace_all(&texto, SEGREDO);

    let mut campos = 0;
    let texto = CAMPO_DE_SEGREDO.replace_all(&texto, |c: &Captures<'_>| {
        let valor = &c[4];
        if valor.bytes().any(|b| b.is_ascii_digit()) && valor != SEGREDO {
            campos += 1;
            format!("{}{}{}{SEGREDO}", &c[1], &c[2], &c[3])
        } else {
            c[0].to_string()
        }
    });
    conta(campos);

    let mut cpfs = 0;
    let texto = CPF_FORMATADO.replace_all(&texto, |c: &Captures<'_>| {
        if cpf_valido(&c[0]) {
            cpfs += 1;
            CPF.to_string()
        } else {
            c[0].to_string()
        }
    });
    conta(cpfs);

    let mut cnpjs = 0;
    let texto = CNPJ_FORMATADO.replace_all(&texto, |c: &Captures<'_>| {
        if cnpj_valido(&c[0]) {
            cnpjs += 1;
            CNPJ.to_string()
        } else {
            c[0].to_string()
        }
    });
    conta(cnpjs);

    let n = EMAIL_RE.find_iter(&texto).count();
    conta(n);
    let texto = EMAIL_RE.replace_all(&texto, EMAIL);

    let n = TELEFONE_RE.find_iter(&texto).count();
    conta(n);
    let texto = TELEFONE_RE.replace_all(&texto, TELEFONE).into_owned();
    (texto, total)
}

/// CNPJ com os dois dígitos verificadores certos (e não todos iguais).
fn cnpj_valido(cnpj: &str) -> bool {
    let d: Vec<u32> = cnpj.chars().filter_map(|c| c.to_digit(10)).collect();
    if d.len() != 14 || d.iter().all(|x| *x == d[0]) {
        return false;
    }
    // Pesos de 2 a 9, da direita para a esquerda, recomeçando depois do 9.
    let digito = |n: usize| {
        let soma: u32 = (0..n).map(|i| d[n - 1 - i] * (i as u32 % 8 + 2)).sum();
        let resto = soma % 11;
        if resto < 2 { 0 } else { 11 - resto }
    };
    digito(12) == d[12] && digito(13) == d[13]
}

/// CPF com os dois dígitos verificadores certos (e não todos iguais).
fn cpf_valido(cpf: &str) -> bool {
    let d: Vec<u32> = cpf.chars().filter_map(|c| c.to_digit(10)).collect();
    if d.len() != 11 || d.iter().all(|x| *x == d[0]) {
        return false;
    }
    let digito = |n: usize| {
        let soma: u32 = (0..n).map(|i| d[i] * (n as u32 + 1 - i as u32)).sum();
        let resto = (soma * 10) % 11;
        if resto == 10 { 0 } else { resto }
    };
    digito(9) == d[9] && digito(10) == d[10]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mascara_segredos_e_dados_pessoais() {
        let texto = "\
Acesse https://ana:S3nhaForte@git.example.com/fat.git para clonar.
Token: ghp_abcdefghijklmnopqrstuvwxyz0123456789
senha: Fatura2026!
A senha: obrigatória no primeiro acesso.
Cliente de teste: CPF 529.982.247-25, contato cobranca@example.com.
Código 123.456.789-00 não é CPF válido.
";
        let (saida, n) = mascarar(texto);
        assert_eq!(n, 5, "{saida}");
        assert!(!saida.contains("S3nhaForte"), "{saida}");
        assert!(saida.contains("https://ana:[segredo mascarado]@git.example.com"));
        assert!(!saida.contains("ghp_"), "{saida}");
        assert!(saida.contains("senha: [segredo mascarado]"), "{saida}");
        assert!(saida.contains("A senha: obrigatória"), "{saida}");
        assert!(saida.contains("CPF [CPF mascarado]"), "{saida}");
        assert!(saida.contains("contato [e-mail mascarado]."), "{saida}");
        assert!(saida.contains("123.456.789-00"), "{saida}");
    }

    #[test]
    fn texto_limpo_nao_muda() {
        let texto = "# Multa por atraso\n\nA multa é de 2% ao mês, conforme RN-FAT-001.\n";
        assert_eq!(mascarar(texto), (texto.to_string(), 0));
        assert!(!tem_chave_privada(texto));
        assert!(tem_chave_privada(
            "```\n-----BEGIN OPENSSH PRIVATE KEY-----\nabc\n```"
        ));
    }

    #[test]
    fn mascara_cnpj_e_telefone() {
        let texto = "\
Empresa 11.222.333/0001-81, filial 11.222.333/0001-82 (inválido).
Fixo (11) 3456-7890, celular +55 11 98765-4321, outro 21 99876-5432,
recado 98765-4321 e (61)912345678.
";
        let (saida, n) = mascarar(texto);
        assert_eq!(n, 6, "{saida}");
        assert!(saida.contains("Empresa [CNPJ mascarado],"), "{saida}");
        assert!(saida.contains("11.222.333/0001-82"), "{saida}");
        assert!(saida.contains("Fixo [telefone mascarado],"), "{saida}");
        assert!(saida.contains("celular [telefone mascarado],"), "{saida}");
        assert!(saida.contains("outro [telefone mascarado],"), "{saida}");
        assert!(saida.contains("recado [telefone mascarado] e [telefone mascarado]."));
    }

    #[test]
    fn valores_e_codigos_nao_viram_cnpj_nem_telefone() {
        let texto = "\
Multa de R$ 1.234,56 sobre R$ 98.765,43 em 2026-11-26 às 14:30.
Protocolo 1234-5678, processo 2024.0001/2025-11, RN-FAT-001, pedido 98765432100.
Versão 12.345.678/0001-00, nota 91234 e lote 9876-54321.
";
        assert_eq!(mascarar(texto), (texto.to_string(), 0));
    }

    #[test]
    fn cnpj_pelos_digitos() {
        assert!(cnpj_valido("11.222.333/0001-81"));
        assert!(!cnpj_valido("11.222.333/0001-80"));
        assert!(!cnpj_valido("00.000.000/0000-00"));
    }

    #[test]
    fn cpf_pelos_digitos() {
        assert!(cpf_valido("529.982.247-25"));
        assert!(!cpf_valido("529.982.247-26"));
        assert!(!cpf_valido("111.111.111-11"));
    }
}
