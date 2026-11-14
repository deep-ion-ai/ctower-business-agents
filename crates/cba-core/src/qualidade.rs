//! Critérios de qualidade de requisitos e histórias: as características da
//! ISO/IEC/IEEE 29148 que a revisão aponta, a verificação INVEST e a lista de
//! referência de termos vagos.
//!
//! Só vocabulário e funções puras, sem estado nem disco, para qualquer fluxo usar:
//! a revisão de requisito (`revisar`, #116) registra achados por
//! [`Caracteristica`] e a avaliação INVEST por [`CriterioInvest`]; um fluxo que
//! escreve histórias pode conferir as suas com os mesmos critérios.
//!
//! [`termos_vagos`] acha no texto os termos da lista [`TERMOS_VAGOS`] ("rápido",
//! "adequado", "etc.", "e/ou"...), sem diferenciar maiúsculas nem acentos, para o
//! agente e o app apontarem ambiguidade de forma determinística.

use serde::{Deserialize, Serialize};

/// Característica de um requisito bem escrito (ISO/IEC/IEEE 29148) que um achado
/// da revisão diz estar violada.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Caracteristica {
    /// Termos vagos, que cada leitor entende de um jeito.
    Ambiguidade,
    /// Critério que não se pode testar.
    Verificabilidade,
    /// Casos de exceção e perfis não tratados, comparando com o que o código trata.
    Completude,
    /// Termos que divergem do glossário do projeto.
    Consistencia,
    /// Contradição com regras confirmadas da base ou com o código.
    Correcao,
    /// Mais de uma necessidade no mesmo requisito ou história.
    Singularidade,
}

impl Caracteristica {
    pub const TODAS: [Caracteristica; 6] = [
        Self::Correcao,
        Self::Ambiguidade,
        Self::Verificabilidade,
        Self::Completude,
        Self::Consistencia,
        Self::Singularidade,
    ];

    /// Valor nas ferramentas e no estado (`correcao`).
    pub fn como_str(self) -> &'static str {
        match self {
            Self::Ambiguidade => "ambiguidade",
            Self::Verificabilidade => "verificabilidade",
            Self::Completude => "completude",
            Self::Consistencia => "consistencia",
            Self::Correcao => "correcao",
            Self::Singularidade => "singularidade",
        }
    }

    /// Nome para o analista (`Correção`).
    pub fn rotulo(self) -> &'static str {
        match self {
            Self::Ambiguidade => "Ambiguidade",
            Self::Verificabilidade => "Verificabilidade",
            Self::Completude => "Completude",
            Self::Consistencia => "Consistência",
            Self::Correcao => "Correção",
            Self::Singularidade => "Singularidade",
        }
    }

    /// O que a característica confere, para o agente.
    pub fn descricao(self) -> &'static str {
        match self {
            Self::Ambiguidade => {
                "termos vagos (\"rápido\", \"adequado\", \"etc.\", \"normalmente\", \"e/ou\") que cada leitor entende de um jeito"
            }
            Self::Verificabilidade => "critério que não se pode testar com um resultado objetivo",
            Self::Completude => {
                "casos de exceção e perfis não tratados, comparando com as regras e exceções que o código trata"
            }
            Self::Consistencia => "termos que divergem do glossário do projeto",
            Self::Correcao => {
                "contradição com regras confirmadas da base ou com o código, sempre com evidência"
            }
            Self::Singularidade => "mais de uma necessidade no mesmo requisito ou história",
        }
    }

    /// Se o achado precisa citar o trecho do texto revisado. Só a completude pode
    /// apontar o que falta, sem trecho.
    pub fn exige_trecho(self) -> bool {
        self != Self::Completude
    }

    /// Se o achado precisa de evidência do código: a correção afirma que o texto
    /// contradiz o que o sistema faz.
    pub fn exige_evidencia(self) -> bool {
        self == Self::Correcao
    }
}

/// Severidade de um achado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severidade {
    /// Impede o desenvolvimento como está escrito (contradição, regra errada).
    Alta,
    /// Gera dúvida ou retrabalho provável, mas não bloqueia.
    Media,
    /// Melhoria de redação.
    Baixa,
}

impl Severidade {
    pub const TODAS: [Severidade; 3] = [Self::Alta, Self::Media, Self::Baixa];

    pub fn rotulo(self) -> &'static str {
        match self {
            Self::Alta => "alta",
            Self::Media => "média",
            Self::Baixa => "baixa",
        }
    }
}

/// Critério INVEST de uma história de usuário.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CriterioInvest {
    Independente,
    Negociavel,
    Valiosa,
    Estimavel,
    Pequena,
    Testavel,
}

impl CriterioInvest {
    /// Na ordem da sigla.
    pub const TODOS: [CriterioInvest; 6] = [
        Self::Independente,
        Self::Negociavel,
        Self::Valiosa,
        Self::Estimavel,
        Self::Pequena,
        Self::Testavel,
    ];

    /// Letra da sigla INVEST (do inglês: Independent, Negotiable, Valuable,
    /// Estimable, Small, Testable).
    pub fn letra(self) -> char {
        match self {
            Self::Independente => 'I',
            Self::Negociavel => 'N',
            Self::Valiosa => 'V',
            Self::Estimavel => 'E',
            Self::Pequena => 'S',
            Self::Testavel => 'T',
        }
    }

    pub fn rotulo(self) -> &'static str {
        match self {
            Self::Independente => "Independente",
            Self::Negociavel => "Negociável",
            Self::Valiosa => "Valiosa",
            Self::Estimavel => "Estimável",
            Self::Pequena => "Pequena",
            Self::Testavel => "Testável",
        }
    }

    /// A pergunta que o critério faz à história.
    pub fn pergunta(self) -> &'static str {
        match self {
            Self::Independente => "Pode ser feita e entregue sem depender de outra história?",
            Self::Negociavel => {
                "Descreve a necessidade, deixando o como para a conversa com o time?"
            }
            Self::Valiosa => "Diz o valor para o usuário ou para o negócio (o \"para quê\")?",
            Self::Estimavel => "O time tem informação suficiente para estimar o esforço?",
            Self::Pequena => "Cabe numa iteração, sem precisar ser quebrada?",
            Self::Testavel => "Os critérios de aceite permitem um teste com resultado objetivo?",
        }
    }
}

/// Resultado de um critério INVEST.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResultadoInvest {
    Atende,
    AtendeEmParte,
    NaoAtende,
}

impl ResultadoInvest {
    pub fn rotulo(self) -> &'static str {
        match self {
            Self::Atende => "atende",
            Self::AtendeEmParte => "atende em parte",
            Self::NaoAtende => "não atende",
        }
    }
}

/// Termo da lista de referência de termos vagos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TermoVago {
    /// Como o termo aparece para o analista ("rápido").
    pub termo: &'static str,
    /// Palavras do padrão, sem acento e em minúsculas. Terminada em `*`, a palavra
    /// casa com o começo de uma palavra do texto (`rapid*` casa com "rapidamente").
    pub padrao: &'static [&'static str],
    /// Por que o termo é vago, para a reescrita.
    pub motivo: &'static str,
}

const fn t(
    termo: &'static str,
    padrao: &'static [&'static str],
    motivo: &'static str,
) -> TermoVago {
    TermoVago {
        termo,
        padrao,
        motivo,
    }
}

const SEM_MEDIDA: &str =
    "não diz a medida; troque por um valor verificável (tempo, quantidade, limite)";
const SEM_CRITERIO: &str = "não diz o critério; descreva o resultado esperado";
const LISTA_ABERTA: &str = "deixa a lista aberta; enumere todos os casos";
const QUANDO: &str = "não diz quando vale; escreva a condição";

/// Lista de referência de termos vagos em requisitos e histórias, em português.
pub const TERMOS_VAGOS: &[TermoVago] = &[
    t("rápido", &["rapid*"], SEM_MEDIDA),
    t("lento", &["lent*"], SEM_MEDIDA),
    t("adequado", &["adequad*"], SEM_CRITERIO),
    t("apropriado", &["apropriad*"], SEM_CRITERIO),
    t("satisfatório", &["satisfatori*"], SEM_CRITERIO),
    t("eficiente", &["eficient*"], SEM_MEDIDA),
    t("fácil", &["facil", "faceis", "facilmente"], SEM_CRITERIO),
    t("simples", &["simples"], SEM_CRITERIO),
    t("amigável", &["amigave*"], SEM_CRITERIO),
    t("intuitivo", &["intuitiv*"], SEM_CRITERIO),
    t("flexível", &["flexive*"], SEM_CRITERIO),
    t("robusto", &["robust*"], SEM_CRITERIO),
    t("otimizado", &["otimizad*"], SEM_MEDIDA),
    t("suficiente", &["suficient*"], SEM_MEDIDA),
    t("razoável", &["razoave*"], SEM_MEDIDA),
    t("aproximadamente", &["aproximadamente"], SEM_MEDIDA),
    t("vários", &["varios", "varias"], SEM_MEDIDA),
    t("alguns", &["alguns", "algumas"], SEM_MEDIDA),
    t("muitos", &["muitos", "muitas"], SEM_MEDIDA),
    t("poucos", &["poucos", "poucas"], SEM_MEDIDA),
    t("normalmente", &["normalmente"], QUANDO),
    t("geralmente", &["geralmente"], QUANDO),
    t("em geral", &["em", "geral"], QUANDO),
    t("se possível", &["se", "possivel"], QUANDO),
    t("quando possível", &["quando", "possivel"], QUANDO),
    t("se necessário", &["se", "necessario"], QUANDO),
    t("quando necessário", &["quando", "necessario"], QUANDO),
    t("conforme necessário", &["conforme", "necessario"], QUANDO),
    t("corretamente", &["corretamente"], SEM_CRITERIO),
    t("etc.", &["etc"], LISTA_ABERTA),
    t("entre outros", &["entre", "outros"], LISTA_ABERTA),
    t(
        "e/ou",
        &["e/ou"],
        "não diz se vale um, outro ou os dois; escreva cada caso",
    ),
];

/// Ocorrência de um termo vago no texto.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Ocorrencia {
    /// O termo da lista ("rápido").
    pub termo: &'static str,
    /// O trecho como está no texto ("Rapidamente").
    pub trecho: String,
    /// Posição do trecho no texto, em caracteres a partir de 0.
    pub inicio: usize,
    pub motivo: &'static str,
}

/// Letra sem acento e em minúscula; o resto fica como está. Um caractere vira um
/// caractere, para as posições do texto normalizado valerem no original.
fn sem_acento(c: char) -> char {
    let c = c.to_lowercase().next().unwrap_or(c);
    match c {
        'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'í' | 'ì' | 'î' | 'ï' => 'i',
        'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
        'ú' | 'ù' | 'û' | 'ü' => 'u',
        'ç' => 'c',
        'ñ' => 'n',
        _ => c,
    }
}

/// Palavra do texto: o texto normalizado e o intervalo, em caracteres, no
/// original. `e/ou` é uma palavra só.
struct Palavra {
    texto: String,
    inicio: usize,
    fim: usize,
}

fn palavras(chars: &[char]) -> Vec<Palavra> {
    let mut lista = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if !chars[i].is_alphanumeric() {
            i += 1;
            continue;
        }
        let inicio = i;
        while i < chars.len() && chars[i].is_alphanumeric() {
            i += 1;
        }
        let mut fim = i;
        let mut texto: String = chars[inicio..fim].iter().map(|c| sem_acento(*c)).collect();
        // "e/ou" (sem espaços) é um termo só.
        if texto == "e"
            && chars.get(fim) == Some(&'/')
            && chars.get(fim + 1).is_some_and(|c| sem_acento(*c) == 'o')
            && chars.get(fim + 2).is_some_and(|c| sem_acento(*c) == 'u')
            && !chars.get(fim + 3).is_some_and(|c| c.is_alphanumeric())
        {
            fim += 3;
            i = fim;
            texto = "e/ou".into();
        }
        lista.push(Palavra { texto, inicio, fim });
    }
    lista
}

fn casa(padrao: &str, palavra: &str) -> bool {
    match padrao.strip_suffix('*') {
        Some(prefixo) => palavra.starts_with(prefixo),
        None => palavra == padrao,
    }
}

/// Termos vagos da lista de referência no texto, na ordem em que aparecem. A
/// comparação ignora maiúsculas e acentos, e cada termo casa com palavras
/// inteiras (ou com o começo, quando o padrão termina em `*`).
pub fn termos_vagos(texto: &str) -> Vec<Ocorrencia> {
    let chars: Vec<char> = texto.chars().collect();
    let palavras = palavras(&chars);
    let mut achadas: Vec<Ocorrencia> = Vec::new();
    let mut usadas = vec![false; palavras.len()];
    for (i, _) in palavras.iter().enumerate() {
        for termo in TERMOS_VAGOS {
            let n = termo.padrao.len();
            if i + n > palavras.len() || usadas[i..i + n].iter().any(|u| *u) {
                continue;
            }
            let confere = termo
                .padrao
                .iter()
                .zip(&palavras[i..i + n])
                .all(|(p, w)| casa(p, &w.texto));
            if confere {
                let (inicio, fim) = (palavras[i].inicio, palavras[i + n - 1].fim);
                // "etc." leva o ponto junto.
                let fim = if termo.termo.ends_with('.') && chars.get(fim) == Some(&'.') {
                    fim + 1
                } else {
                    fim
                };
                usadas[i..i + n].iter_mut().for_each(|u| *u = true);
                achadas.push(Ocorrencia {
                    termo: termo.termo,
                    trecho: chars[inicio..fim].iter().collect(),
                    inicio,
                    motivo: termo.motivo,
                });
                break;
            }
        }
    }
    achadas
}

/// Texto normalizado para comparar trechos: sem acento, em minúsculas e com os
/// espaços (e quebras de linha) juntados num só.
pub fn normalizar(texto: &str) -> String {
    texto
        .split_whitespace()
        .map(|p| p.chars().map(sem_acento).collect::<String>())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Se `trecho` aparece em `texto`, sem diferenciar maiúsculas, acentos nem
/// espaços. Aspas e marcadores de lista nas pontas do trecho são ignorados.
pub fn contem_trecho(texto: &str, trecho: &str) -> bool {
    let trecho = trecho.trim().trim_matches(|c: char| {
        matches!(
            c,
            '"' | '\'' | '“' | '”' | '‘' | '’' | '«' | '»' | '-' | '*'
        )
    });
    let trecho = normalizar(trecho);
    !trecho.is_empty() && normalizar(texto).contains(&trecho)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn termos(texto: &str) -> Vec<&'static str> {
        termos_vagos(texto).into_iter().map(|o| o.termo).collect()
    }

    #[test]
    fn acha_os_termos_da_issue() {
        let texto = "O cancelamento deve ser Rápido e a mensagem, adequada. \
                     Vale para faturas avulsas e/ou parceladas, etc. Normalmente o \
                     atendente confirma.";
        assert_eq!(
            termos(texto),
            ["rápido", "adequado", "e/ou", "etc.", "normalmente"]
        );
        let o = &termos_vagos(texto)[0];
        assert_eq!(o.trecho, "Rápido");
        assert_eq!(
            texto.chars().skip(o.inicio).take(6).collect::<String>(),
            "Rápido"
        );
        let etc = termos_vagos(texto)
            .into_iter()
            .find(|o| o.termo == "etc.")
            .unwrap();
        assert_eq!(etc.trecho, "etc.");
    }

    #[test]
    fn palavra_inteira_ou_comeco_com_asterisco() {
        assert_eq!(termos("Processar rapidamente."), ["rápido"]);
        // "simplesmente" não é "simples"; "alguém" não é "alguns".
        assert!(termos("simplesmente alguém").is_empty());
        assert_eq!(termos("Se possível, em geral"), ["se possível", "em geral"]);
        assert!(termos("e / ou").is_empty());
        assert!(termos("e/outro").is_empty());
    }

    #[test]
    fn trecho_ignora_caixa_acento_e_espacos() {
        let texto =
            "O atendente pode cancelar fatura aberta,\ninclusive a que já tem pagamento parcial.";
        assert!(contem_trecho(
            texto,
            "\"inclusive a que ja tem  pagamento parcial\""
        ));
        assert!(contem_trecho(texto, "fatura aberta, inclusive"));
        assert!(!contem_trecho(texto, "fatura vencida"));
        assert!(!contem_trecho(texto, "  \"\" "));
    }

    #[test]
    fn listas_completas() {
        assert_eq!(
            CriterioInvest::TODOS.map(CriterioInvest::letra),
            ['I', 'N', 'V', 'E', 'S', 'T']
        );
        assert_eq!(Caracteristica::TODAS.len(), 6);
        for c in Caracteristica::TODAS {
            let v = serde_json::to_value(c).unwrap();
            assert_eq!(v, c.como_str());
        }
        // Os padrões estão normalizados.
        for t in TERMOS_VAGOS {
            for p in t.padrao {
                assert_eq!(&normalizar(p), p, "{}", t.termo);
            }
        }
    }
}
