//! Validação das evidências citadas pelo agente (spec, seção 6.5).
//!
//! Sem evidência, não é fato. Uma evidência é `(arquivo, linha inicial, linha final,
//! trecho citado)`, com o caminho relativo à raiz do projeto. No registro, o servidor
//! confere, nesta ordem:
//!
//! 1. o intervalo: linha inicial a partir de 1, final não menor que a inicial e no
//!    máximo [`LIMITE_DE_LINHAS`] linhas;
//! 2. o caminho, pelo [`Escopo`] do projeto: relativo, dentro das pastas do projeto
//!    (também depois de seguir links simbólicos), fora das exclusões e existente;
//! 3. o arquivo: texto (sem byte nulo), até [`LIMITE_DO_ARQUIVO`], em UTF-8 ou,
//!    se não for UTF-8 válido, Windows-1252 (comum em sistemas legados);
//! 4. as linhas: o intervalo existe no arquivo;
//! 5. o trecho: aparece dentro do intervalo. A comparação ignora os espaços nas
//!    pontas de cada linha, os espaços repetidos e as linhas em branco, e mais nada.
//!
//! Evidência recusada volta com o motivo e, quando ajuda, com a correção: as linhas
//! em que o trecho aparece de fato, ou o conteúdo atual do intervalo. A prova de
//! conceito mostrou que o agente corrige a chamada na hora com isso (spec, seção 13).
//!
//! A evidência aceita guarda o caminho como está no disco, o trecho citado, o hash
//! do intervalo ([`hash_das_linhas`]) e o commit da pasta, quando ela está num
//! repositório git. A revalidação (#12) recalcula o mesmo hash para saber se o
//! código mudou.

use std::fmt;
use std::fs;
use std::path::Path;

use cba_core::analise::{EstadoEvidencia, Evidencia};
use cba_index::{Escopo, Recusa as RecusaDoEscopo, detectar_git};
use cba_kb::CaminhoRelativo;
use cba_kb::frontmatter::Linhas;
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

/// Tamanho máximo do arquivo citado.
pub const LIMITE_DO_ARQUIVO: u64 = 10 * 1024 * 1024;
/// Linhas no máximo por evidência: a evidência aponta o trecho que decide, não o
/// arquivo inteiro.
pub const LIMITE_DE_LINHAS: u32 = 300;
/// Tamanho máximo do trecho citado, em caracteres (o mesmo do schema).
pub const LIMITE_DO_TRECHO: u64 = 20_000;
/// Linhas do conteúdo atual mostradas na recusa.
const LINHAS_NA_RECUSA: usize = 30;
/// Caracteres por linha mostrados na recusa.
const LARGURA_NA_RECUSA: usize = 200;

/// Evidência como o agente cita (argumento de `afirmar` e de `propor_termo`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct EvidenciaCitada {
    pub arquivo: String,
    pub linha_inicial: u32,
    pub linha_final: u32,
    pub trecho: String,
}

/// JSON Schema de uma evidência citada, para os schemas das ferramentas.
pub fn schema_da_evidencia() -> Value {
    json!({
        "type": "object",
        "properties": {
            "arquivo": {
                "type": "string",
                "minLength": 1,
                "description": "Caminho relativo à raiz do projeto, com / (ex.: db/03_pkg_cobranca.sql)."
            },
            "linha_inicial": {
                "type": "integer",
                "minimum": 1,
                "description": "Primeira linha citada, contando a partir de 1."
            },
            "linha_final": {
                "type": "integer",
                "minimum": 1,
                "description": "Última linha citada (inclusiva)."
            },
            "trecho": {
                "type": "string",
                "minLength": 1,
                "maxLength": LIMITE_DO_TRECHO,
                "description": "Texto copiado do arquivo, que aparece dentro do intervalo, sem os números de linha. Espaços nas pontas, espaços repetidos e linhas em branco são ignorados na conferência."
            }
        },
        "required": ["arquivo", "linha_inicial", "linha_final", "trecho"],
        "additionalProperties": false
    })
}

/// Por que uma evidência foi recusada. As mensagens se dirigem ao agente e dizem
/// como corrigir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Motivo {
    /// O escopo do projeto recusou o caminho.
    Caminho(RecusaDoEscopo),
    /// Linha final menor que a inicial (ou linha zero).
    IntervaloInvalido,
    /// Mais linhas que [`LIMITE_DE_LINHAS`].
    IntervaloGrande,
    /// O intervalo passa do fim do arquivo, que tem `total` linhas.
    ForaDoArquivo {
        total: usize,
    },
    TrechoVazio,
    Binario,
    Grande {
        bytes: u64,
    },
    Ilegivel(String),
    /// O trecho não aparece no intervalo.
    NaoConfere {
        /// Onde o trecho aparece no arquivo, se aparece.
        encontrado: Vec<Linhas>,
        /// Conteúdo atual do intervalo, resumido.
        atual: String,
        /// O trecho parece trazer os números de linha da leitura do arquivo.
        numeros_de_linha: bool,
    },
    /// O trecho não aparece no arquivo (trecho sem intervalo, como o trecho atual
    /// de uma sugestão).
    NaoEncontrado {
        numeros_de_linha: bool,
    },
    /// O trecho aparece em mais de um lugar e o intervalo não foi informado.
    Ambiguo {
        encontrado: Vec<Linhas>,
    },
}

fn lista_de_intervalos(intervalos: &[Linhas]) -> String {
    let mut nomes: Vec<String> = intervalos.iter().take(5).map(|l| l.to_string()).collect();
    if intervalos.len() > 5 {
        nomes.push(format!("e mais {}", intervalos.len() - 5));
    }
    nomes.join(", ")
}

const SEM_NUMEROS: &str =
    " Não inclua no trecho os números de linha mostrados na leitura do arquivo.";

impl fmt::Display for Motivo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Caminho(r) => match r {
                RecusaDoEscopo::CaminhoInvalido(_) => write!(
                    f,
                    "caminho inválido; use o caminho relativo à raiz do projeto, com / \
                     (como db/03_pkg_cobranca.sql), sem `..` nem caminho absoluto"
                ),
                RecusaDoEscopo::ForaDoProjeto => write!(
                    f,
                    "o arquivo fica fora das pastas do projeto; cite só arquivos do \
                     sistema analisado, com o caminho relativo à raiz dele"
                ),
                RecusaDoEscopo::Excluido(e) => write!(
                    f,
                    "o arquivo está excluído da análise ({}) e não pode ser citado como \
                     evidência",
                    e.motivo.descricao()
                ),
                RecusaDoEscopo::NaoEncontrado => write!(
                    f,
                    "arquivo não encontrado; confira o caminho, relativo à raiz do projeto"
                ),
                RecusaDoEscopo::NaoEhArquivo => {
                    write!(f, "o caminho não é um arquivo; cite um arquivo")
                }
            },
            Self::IntervaloInvalido => write!(
                f,
                "intervalo inválido; linha_inicial começa em 1 e linha_final não pode ser \
                 menor que linha_inicial"
            ),
            Self::IntervaloGrande => write!(
                f,
                "o intervalo passa de {LIMITE_DE_LINHAS} linhas; cite só as linhas que \
                 sustentam a afirmação, em mais de uma evidência se preciso"
            ),
            Self::ForaDoArquivo { total: 0 } => write!(f, "o arquivo está vazio"),
            Self::ForaDoArquivo { total } => write!(
                f,
                "o intervalo passa do fim do arquivo, que tem {total} linhas"
            ),
            Self::TrechoVazio => write!(f, "o trecho citado está vazio; copie o texto do arquivo"),
            Self::Binario => write!(f, "o arquivo é binário e não pode ser citado"),
            Self::Grande { bytes } => write!(
                f,
                "o arquivo tem {bytes} bytes, acima do limite de {LIMITE_DO_ARQUIVO} para \
                 evidências"
            ),
            Self::Ilegivel(e) => write!(f, "não foi possível ler o arquivo ({e})"),
            Self::NaoConfere {
                encontrado,
                atual,
                numeros_de_linha,
            } => {
                write!(f, "o trecho citado não aparece nessas linhas.")?;
                match encontrado.as_slice() {
                    [l] => write!(
                        f,
                        " Ele aparece nas linhas {l}: use linha_inicial {} e linha_final {}.",
                        l.inicio, l.fim
                    )?,
                    [] => write!(
                        f,
                        " Conteúdo atual dessas linhas:\n```\n{atual}\n```\nCite o texto como \
                         está no arquivo; só os espaços nas pontas, os espaços repetidos e as \
                         linhas em branco são ignorados."
                    )?,
                    varios => write!(
                        f,
                        " Ele aparece nas linhas {}; corrija linha_inicial e linha_final.",
                        lista_de_intervalos(varios)
                    )?,
                }
                if *numeros_de_linha {
                    f.write_str(SEM_NUMEROS)?;
                }
                Ok(())
            }
            Self::NaoEncontrado { numeros_de_linha } => {
                write!(
                    f,
                    "o trecho não aparece no arquivo; copie o texto como está no arquivo"
                )?;
                if *numeros_de_linha {
                    f.write_str(SEM_NUMEROS)?;
                }
                Ok(())
            }
            Self::Ambiguo { encontrado } => write!(
                f,
                "o trecho aparece em mais de um lugar (linhas {}); informe linha_inicial e \
                 linha_final",
                lista_de_intervalos(encontrado)
            ),
        }
    }
}

/// Evidência recusada: o que foi citado e o motivo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenciaRecusada {
    pub arquivo: String,
    /// Intervalo citado, quando houver.
    pub linhas: Option<(u32, u32)>,
    pub motivo: Motivo,
}

impl fmt::Display for EvidenciaRecusada {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.linhas {
            Some((i, j)) => write!(f, "`{}`, linhas {i}-{j}: {}", self.arquivo, self.motivo),
            None => write!(f, "`{}`: {}", self.arquivo, self.motivo),
        }
    }
}

impl std::error::Error for EvidenciaRecusada {}

/// Arquivo do projeto aberto para conferência.
#[derive(Debug, Clone)]
pub struct ArquivoDoProjeto {
    /// Caminho relativo à raiz do projeto, como está no disco.
    pub relativo: CaminhoRelativo,
    pub texto: String,
    /// Commit da pasta do arquivo, se ela estiver num repositório git.
    pub commit: Option<String>,
}

impl ArquivoDoProjeto {
    /// Abre um arquivo do projeto pelo escopo, com as mesmas regras das evidências.
    pub fn abrir(escopo: &Escopo, arquivo: &str) -> Result<Self, Motivo> {
        let absoluto = escopo.resolver(arquivo).map_err(Motivo::Caminho)?;
        let texto = ler_texto(&absoluto)?;
        let relativo = escopo
            .relativo(&absoluto)
            .and_then(|r| CaminhoRelativo::novo(&r).ok())
            .or_else(|| CaminhoRelativo::novo(&arquivo.trim().replace('\\', "/")).ok())
            .ok_or(Motivo::Caminho(RecusaDoEscopo::CaminhoInvalido(
                arquivo.to_owned(),
            )))?;
        let commit = escopo
            .raiz_de(relativo.as_str())
            .and_then(detectar_git)
            .and_then(|g| g.commit);
        Ok(Self {
            relativo,
            texto,
            commit,
        })
    }

    /// Número de linhas do arquivo.
    pub fn total_de_linhas(&self) -> usize {
        self.texto.lines().count()
    }

    /// Confere o trecho no intervalo e devolve a evidência.
    pub fn conferir(&self, linhas: Linhas, trecho: &str) -> Result<Evidencia, Motivo> {
        if trecho.trim().is_empty() {
            return Err(Motivo::TrechoVazio);
        }
        let total = self.total_de_linhas();
        if linhas.fim as usize > total {
            return Err(Motivo::ForaDoArquivo { total });
        }
        let encontrado = ocorrencias(&self.texto, trecho);
        if !encontrado
            .iter()
            .any(|o| o.inicio >= linhas.inicio && o.fim <= linhas.fim)
        {
            return Err(Motivo::NaoConfere {
                encontrado,
                atual: resumo_das_linhas(&self.texto, linhas),
                numeros_de_linha: tem_numeros_de_linha(trecho),
            });
        }
        Ok(self.evidencia(linhas, trecho))
    }

    /// Procura o trecho no arquivo inteiro e devolve a evidência do único lugar em
    /// que ele aparece.
    pub fn localizar(&self, trecho: &str) -> Result<Evidencia, Motivo> {
        if trecho.trim().is_empty() {
            return Err(Motivo::TrechoVazio);
        }
        match ocorrencias(&self.texto, trecho).as_slice() {
            [] => Err(Motivo::NaoEncontrado {
                numeros_de_linha: tem_numeros_de_linha(trecho),
            }),
            [l] => {
                if l.fim - l.inicio + 1 > LIMITE_DE_LINHAS {
                    return Err(Motivo::IntervaloGrande);
                }
                Ok(self.evidencia(*l, trecho))
            }
            varios => Err(Motivo::Ambiguo {
                encontrado: varios.to_vec(),
            }),
        }
    }

    fn evidencia(&self, linhas: Linhas, trecho: &str) -> Evidencia {
        Evidencia {
            arquivo: self.relativo.clone(),
            linhas,
            trecho: trecho.trim().to_owned(),
            hash: hash_das_linhas(&self.texto, linhas)
                .expect("intervalo conferido contra o arquivo"),
            commit: self.commit.clone(),
            estado: EstadoEvidencia::Valida,
        }
    }
}

/// Intervalo citado, conferido antes de abrir o arquivo: linhas a partir de 1, final
/// não menor que a inicial, no máximo [`LIMITE_DE_LINHAS`].
pub fn intervalo(inicial: u32, final_: u32) -> Result<Linhas, Motivo> {
    let linhas = Linhas::novo(inicial, final_).ok_or(Motivo::IntervaloInvalido)?;
    if linhas.fim - linhas.inicio + 1 > LIMITE_DE_LINHAS {
        return Err(Motivo::IntervaloGrande);
    }
    Ok(linhas)
}

/// Valida uma evidência citada contra o código atual do projeto.
pub fn validar(escopo: &Escopo, citada: &EvidenciaCitada) -> Result<Evidencia, EvidenciaRecusada> {
    let recusa = |motivo| EvidenciaRecusada {
        arquivo: citada.arquivo.clone(),
        linhas: Some((citada.linha_inicial, citada.linha_final)),
        motivo,
    };
    if citada.trecho.trim().is_empty() {
        return Err(recusa(Motivo::TrechoVazio));
    }
    let linhas = intervalo(citada.linha_inicial, citada.linha_final).map_err(recusa)?;
    let arquivo = ArquivoDoProjeto::abrir(escopo, &citada.arquivo).map_err(recusa)?;
    arquivo.conferir(linhas, &citada.trecho).map_err(recusa)
}

/// Valida todas as evidências. Se alguma for recusada, devolve a mensagem com todas
/// as recusas (cada uma com o número da evidência na lista), para o agente corrigir
/// tudo numa chamada só.
pub fn validar_todas(
    escopo: &Escopo,
    citadas: &[EvidenciaCitada],
) -> Result<Vec<Evidencia>, String> {
    let mut aceitas = Vec::new();
    let mut recusas = Vec::new();
    for (i, citada) in citadas.iter().enumerate() {
        match validar(escopo, citada) {
            Ok(e) => {
                // A mesma evidência citada duas vezes conta uma.
                if !aceitas
                    .iter()
                    .any(|a: &Evidencia| a.arquivo == e.arquivo && a.linhas == e.linhas)
                {
                    aceitas.push(e);
                }
            }
            Err(r) => recusas.push(format!("- evidências[{i}] ({r})")),
        }
    }
    if recusas.is_empty() {
        return Ok(aceitas);
    }
    Err(format!(
        "{} de {} evidências recusadas; nada foi registrado. Corrija e chame de novo:\n{}",
        recusas.len(),
        citadas.len(),
        recusas.join("\n")
    ))
}

/// Lê um arquivo de texto do projeto: recusa binário (byte nulo) e arquivo acima de
/// [`LIMITE_DO_ARQUIVO`]; decodifica UTF-8 (sem BOM) ou, se não for UTF-8 válido,
/// Windows-1252.
pub fn ler_texto(caminho: &Path) -> Result<String, Motivo> {
    let ilegivel = |e: std::io::Error| Motivo::Ilegivel(e.kind().to_string());
    let bytes = fs::metadata(caminho).map_err(ilegivel)?.len();
    if bytes > LIMITE_DO_ARQUIVO {
        return Err(Motivo::Grande { bytes });
    }
    decodificar(&fs::read(caminho).map_err(ilegivel)?)
}

/// Texto de um arquivo: UTF-8 (sem BOM) ou Windows-1252. Byte nulo é binário.
pub fn decodificar(bytes: &[u8]) -> Result<String, Motivo> {
    if bytes.contains(&0) {
        return Err(Motivo::Binario);
    }
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    Ok(match std::str::from_utf8(bytes) {
        Ok(s) => s.to_owned(),
        Err(_) => bytes.iter().map(|&b| windows_1252(b)).collect(),
    })
}

/// Caracteres de 0x80 a 0x9F no Windows-1252; os indefinidos ficam com o mesmo
/// código (como no padrão de codificações da web).
const WINDOWS_1252_ALTO: [u32; 32] = [
    0x20AC, 0x0081, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160, 0x2039,
    0x0152, 0x008D, 0x017D, 0x008F, 0x0090, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014,
    0x02DC, 0x2122, 0x0161, 0x203A, 0x0153, 0x009D, 0x017E, 0x0178,
];

fn windows_1252(b: u8) -> char {
    let codigo = match b {
        0x80..=0x9F => WINDOWS_1252_ALTO[usize::from(b - 0x80)],
        _ => u32::from(b),
    };
    char::from_u32(codigo).unwrap_or(char::REPLACEMENT_CHARACTER)
}

/// Linha normalizada para comparação: sem espaços nas pontas e com os espaços
/// internos repetidos reduzidos a um.
fn normalizar(linha: &str) -> String {
    linha.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Hash do intervalo citado: SHA-256, em hexadecimal minúsculo, das linhas do
/// intervalo normalizadas (sem espaços nas pontas, espaços repetidos reduzidos a
/// um), unidas por `\n`. Mudar a indentação ou o fim de linha (CRLF/LF) não muda o
/// hash; mudar o código, sim. `None` se o intervalo passa do fim do texto.
pub fn hash_das_linhas(texto: &str, linhas: Linhas) -> Option<String> {
    let todas: Vec<&str> = texto.lines().collect();
    let trecho = todas.get(linhas.inicio as usize - 1..linhas.fim as usize)?;
    let normalizado: Vec<String> = trecho.iter().map(|l| normalizar(l)).collect();
    let digest = Sha256::digest(normalizado.join("\n").as_bytes());
    Some(digest.iter().map(|b| format!("{b:02x}")).collect())
}

/// Intervalos de linhas em que o trecho aparece no texto, em ordem. Linhas em
/// branco são ignoradas dos dois lados; o trecho pode começar e terminar no meio de
/// uma linha.
pub fn ocorrencias(texto: &str, trecho: &str) -> Vec<Linhas> {
    let procurado: Vec<String> = trecho
        .lines()
        .map(normalizar)
        .filter(|l| !l.is_empty())
        .collect();
    if procurado.is_empty() {
        return Vec::new();
    }
    let procurado = procurado.join("\n");
    // O texto normalizado sem linhas em branco, com o número e o início de cada linha.
    let mut unido = String::new();
    let mut inicios: Vec<(usize, u32)> = Vec::new();
    for (i, linha) in texto.lines().enumerate() {
        let linha = normalizar(linha);
        if linha.is_empty() {
            continue;
        }
        if !unido.is_empty() {
            unido.push('\n');
        }
        inicios.push((unido.len(), i as u32 + 1));
        unido.push_str(&linha);
    }
    let linha_em = |pos: usize| {
        let i = inicios.partition_point(|(inicio, _)| *inicio <= pos);
        inicios[i.saturating_sub(1)].1
    };
    let mut saida = Vec::new();
    let mut desde = 0;
    while let Some(p) = unido[desde..].find(&procurado) {
        let inicio = desde + p;
        let fim = inicio + procurado.len() - 1;
        if let Some(l) = Linhas::novo(linha_em(inicio), linha_em(fim)) {
            saida.push(l);
        }
        // Avança um caractere, para achar também ocorrências sobrepostas.
        desde = inicio + unido[inicio..].chars().next().map_or(1, char::len_utf8);
    }
    saida
}

/// Conteúdo atual do intervalo, resumido para a mensagem de recusa.
fn resumo_das_linhas(texto: &str, linhas: Linhas) -> String {
    let todas: Vec<&str> = texto.lines().collect();
    let inicio = linhas.inicio as usize - 1;
    let fim = (linhas.fim as usize).min(todas.len());
    let mut saida: Vec<String> = todas[inicio..fim]
        .iter()
        .take(LINHAS_NA_RECUSA)
        .map(|l| {
            if l.chars().count() > LARGURA_NA_RECUSA {
                let corte: String = l.chars().take(LARGURA_NA_RECUSA).collect();
                format!("{corte}…")
            } else {
                (*l).to_owned()
            }
        })
        .collect();
    if fim - inicio > LINHAS_NA_RECUSA {
        saida.push(format!(
            "… (mais {} linhas)",
            fim - inicio - LINHAS_NA_RECUSA
        ));
    }
    saida.join("\n").replace("```", "`\u{200b}``")
}

/// Se o trecho parece copiado com os números de linha da leitura do arquivo
/// (`  12→`, `12\t`, `12: `, `12 | `) na maioria das linhas.
fn tem_numeros_de_linha(trecho: &str) -> bool {
    let linhas: Vec<&str> = trecho.lines().filter(|l| !l.trim().is_empty()).collect();
    let numeradas = linhas
        .iter()
        .filter(|l| {
            let l = l.trim_start();
            let digitos = l.bytes().take_while(u8::is_ascii_digit).count();
            if digitos == 0 {
                return false;
            }
            let resto = &l[digitos..];
            resto.starts_with('\t') || resto.trim_start().starts_with(['→', '|', ':'])
        })
        .count();
    !linhas.is_empty() && numeradas * 2 > linhas.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn l(a: u32, b: u32) -> Linhas {
        Linhas::novo(a, b).unwrap()
    }

    const TEXTO: &str = "linha um\r\n    IF x THEN\r\n\r\n        y := 1;   -- comentário\r\n    END IF;\r\nIF x THEN\r\n";

    #[test]
    fn ocorrencias_ignoram_espacos_e_linhas_em_branco() {
        assert_eq!(ocorrencias(TEXTO, "IF x THEN"), vec![l(2, 2), l(6, 6)]);
        assert_eq!(
            ocorrencias(TEXTO, "  IF   x THEN\n  y := 1;  "),
            vec![l(2, 4)]
        );
        // Começa e termina no meio de uma linha.
        assert_eq!(ocorrencias(TEXTO, "THEN\ny := 1"), vec![l(2, 4)]);
        assert_eq!(ocorrencias(TEXTO, "x then"), vec![]);
        assert_eq!(ocorrencias(TEXTO, " \n "), vec![]);
        // Sobrepostas.
        assert_eq!(ocorrencias("aaa\n", "aa"), vec![l(1, 1), l(1, 1)]);
    }

    #[test]
    fn hash_ignora_indentacao_e_fim_de_linha() {
        let lf = TEXTO.replace("\r\n", "\n");
        let reindentado = lf.replace("    IF", "\tIF");
        let h = hash_das_linhas(TEXTO, l(2, 5)).unwrap();
        assert_eq!(h.len(), 64);
        assert_eq!(hash_das_linhas(&lf, l(2, 5)).unwrap(), h);
        assert_eq!(hash_das_linhas(&reindentado, l(2, 5)).unwrap(), h);
        let mudado = lf.replace("y := 1", "y := 2");
        assert_ne!(hash_das_linhas(&mudado, l(2, 5)).unwrap(), h);
        assert_eq!(hash_das_linhas(TEXTO, l(6, 7)), None);
    }

    #[test]
    fn decodifica_utf8_com_bom_e_windows_1252() {
        assert_eq!(decodificar(b"\xEF\xBB\xBFa\xC3\xA7").unwrap(), "aç");
        // "Ação – €" em Windows-1252.
        assert_eq!(decodificar(b"A\xE7\xE3o \x96 \x80").unwrap(), "Ação – €");
        assert_eq!(decodificar(b"a\0b"), Err(Motivo::Binario));
    }

    #[test]
    fn detecta_numeros_de_linha() {
        assert!(tem_numeros_de_linha("  12→IF x THEN\n  13→END IF;"));
        assert!(tem_numeros_de_linha("12\tIF x THEN"));
        assert!(tem_numeros_de_linha("7: a\n8: b\nc"));
        assert!(!tem_numeros_de_linha("IF x THEN\n2.0000"));
        assert!(!tem_numeros_de_linha("100 / 2"));
    }

    #[test]
    fn mensagens_dizem_como_corrigir() {
        let m = Motivo::NaoConfere {
            encontrado: vec![l(14, 15)],
            atual: String::new(),
            numeros_de_linha: true,
        };
        let texto = m.to_string();
        assert!(
            texto.contains("linha_inicial 14 e linha_final 15"),
            "{texto}"
        );
        assert!(texto.contains("números de linha"), "{texto}");
        let m = Motivo::NaoConfere {
            encontrado: vec![],
            atual: "a := 1;".into(),
            numeros_de_linha: false,
        };
        assert!(m.to_string().contains("```\na := 1;\n```"));
        assert!(
            Motivo::ForaDoArquivo { total: 3 }
                .to_string()
                .contains("tem 3 linhas")
        );
    }

    #[test]
    fn resumo_corta_linhas_longas_e_muitas_linhas() {
        let texto: String = (1..=50)
            .map(|i| format!("{i}{}\n", "x".repeat(300)))
            .collect();
        let r = resumo_das_linhas(&texto, l(1, 50));
        assert_eq!(r.lines().count(), LINHAS_NA_RECUSA + 1);
        assert!(r.ends_with("(mais 20 linhas)"));
        assert!(r.lines().next().unwrap().ends_with('…'));
    }
}
