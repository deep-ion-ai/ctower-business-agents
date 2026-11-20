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
//!   explicativo), CPF com dígitos verificadores válidos e endereço de e-mail.
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

    let n = EMAIL_RE.find_iter(&texto).count();
    conta(n);
    let texto = EMAIL_RE.replace_all(&texto, EMAIL).into_owned();
    (texto, total)
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
    fn cpf_pelos_digitos() {
        assert!(cpf_valido("529.982.247-25"));
        assert!(!cpf_valido("529.982.247-26"));
        assert!(!cpf_valido("111.111.111-11"));
    }
}
