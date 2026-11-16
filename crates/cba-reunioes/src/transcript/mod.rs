//! Parser determinístico do transcript do Teams (#134).
//!
//! Transforma o transcript numa lista de [`Fala`]s, sem IA. Formatos aceitos:
//!
//! - **WebVTT** ([`Formato::Vtt`]): o `.vtt` baixado do Teams. Cabeçalho
//!   `WEBVTT`, cues com id opcional, horário `HH:MM:SS.mmm --> HH:MM:SS.mmm` e
//!   o falante na marca de voz `<v Nome>texto</v>`. Cue sem `<v>` vira fala
//!   sem falante. Cue com várias marcas de voz vira uma fala por marca, com o
//!   horário do cue. Blocos `NOTE`, `STYLE` e `REGION` são ignorados.
//! - **`.docx` exportado** ([`Formato::Docx`]): lê o texto dos parágrafos de
//!   `word/document.xml` e aplica a leitura por linhas do texto copiado (abaixo).
//!   Suposição sobre o layout (não há especificação publicada; ver
//!   [`paragrafos_docx`]): o export atual do Teams traz um cabeçalho (título,
//!   data e duração), depois, para cada fala, um parágrafo com o nome e o
//!   horário relativo (`Maria Silva   0:03`, separados por tabulação ou
//!   espaços) seguido de um ou mais parágrafos com a fala. O export antigo
//!   trazia blocos `0:0:0.0 --> 0:0:3.250`, nome e fala, também aceitos. O
//!   cabeçalho antes da primeira fala é descartado sem aviso.
//! - **Texto copiado da tela** ([`Formato::TextoTeams`]): linhas de cabeçalho
//!   `Nome  0:03`, `Nome 00:00:03` ou `Nome   1:02:03` seguidas das linhas da
//!   fala. Ver as regras de reconhecimento do cabeçalho no módulo `linhas`.
//!
//! Formato não reconhecido vira uma única fala sem falante e sem horário, com
//! o texto corrido, e o aviso [`Aviso::FormatoDesconhecido`]. Entrada vazia
//! devolve [`Aviso::EntradaVazia`] e nenhuma fala. O parser nunca entra em
//! pânico: entrada estranha vira aviso.
//!
//! Os nomes dos falantes ficam como aparecem ("Silva, Maria",
//! "maria.silva@exemplo.com", "João Pereira"): o casamento com o catálogo de
//! pessoas é da #133. Falas consecutivas do mesmo falante continuam separadas,
//! como estão no transcript.

mod docx;
mod linhas;
mod texto;
mod vtt;

use std::fmt;

use serde::{Deserialize, Serialize};

pub use docx::{ErroDocx, paragrafos_docx};

/// Horário relativo ao início da reunião, em milissegundos.
///
/// Serializa como o número de milissegundos. `Display` mostra no estilo do
/// Teams: `M:SS` abaixo de uma hora e `H:MM:SS` a partir dela.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Horario(u64);

impl Horario {
    /// Horário a partir dos milissegundos desde o início da reunião.
    pub const fn de_milissegundos(ms: u64) -> Self {
        Self(ms)
    }

    /// Milissegundos desde o início da reunião.
    pub const fn milissegundos(self) -> u64 {
        self.0
    }

    /// Segundos inteiros desde o início da reunião (arredonda para baixo).
    pub const fn segundos(self) -> u64 {
        self.0 / 1000
    }
}

impl fmt::Display for Horario {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let total = self.segundos();
        let (h, m, s) = (total / 3600, (total / 60) % 60, total % 60);
        if h > 0 {
            write!(f, "{h}:{m:02}:{s:02}")
        } else {
            write!(f, "{m}:{s:02}")
        }
    }
}

/// Uma fala do transcript, na ordem em que aparece.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fala {
    /// Posição da fala no transcript, a partir de 1. É o número que as
    /// citações da ata e as ferramentas do MCP usam.
    pub indice: usize,
    /// Nome do falante como aparece no transcript, sem normalização. `None`
    /// quando o formato não traz o falante (cue sem `<v>`, texto desconhecido).
    pub falante: Option<String>,
    /// Início da fala. `None` só no texto desconhecido.
    pub inicio: Option<Horario>,
    /// Fim da fala. Só o WebVTT traz o fim; o `.docx` atual e o texto copiado
    /// trazem só o início, e o parser não inventa o fim.
    pub fim: Option<Horario>,
    /// Texto da fala, sem marcas. Linhas aparadas e sem linhas vazias; quando
    /// a fala tem várias linhas, elas ficam separadas por `\n`.
    pub texto: String,
}

/// Formato detectado do transcript.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Formato {
    /// WebVTT (`.vtt`) do Teams.
    Vtt,
    /// `.docx` exportado do Teams.
    Docx,
    /// Texto copiado da tela do Teams (ou outro texto no mesmo layout).
    TextoTeams,
    /// Nada reconhecido: texto corrido, entrada vazia ou arquivo ilegível.
    Desconhecido,
}

/// Aviso sobre a leitura do transcript.
///
/// Nunca carrega trechos das falas, só contagens e motivos técnicos: pode ir
/// para o log e para o diário de eventos. Serializa com o campo `codigo`
/// (`{"codigo": "formato_desconhecido"}`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "codigo", rename_all = "snake_case")]
pub enum Aviso {
    /// A entrada está vazia (ou só tem espaços).
    EntradaVazia,
    /// O formato não foi reconhecido; o texto inteiro virou uma fala sem
    /// falante.
    FormatoDesconhecido,
    /// O formato foi reconhecido, mas nenhuma fala foi encontrada.
    SemFalas,
    /// O arquivo parece um `.docx`, mas não deu para ler o texto.
    DocxInvalido {
        /// Motivo técnico (zip corrompido, falta `word/document.xml`, XML
        /// inválido).
        motivo: String,
    },
    /// O texto não era UTF-8 válido; os trechos inválidos viraram `�`.
    CodificacaoInvalida,
    /// Linhas com texto antes da primeira fala do texto copiado foram
    /// descartadas.
    TextoAntesDaPrimeiraFala {
        /// Quantidade de linhas não vazias descartadas.
        linhas: usize,
    },
    /// Blocos do WebVTT que não são cues válidos (sem horário ou com horário
    /// malformado) foram descartados.
    BlocosIgnorados {
        /// Quantidade de blocos descartados.
        quantidade: usize,
    },
}

impl Aviso {
    /// Código estável do aviso (o mesmo do campo `codigo` serializado).
    pub fn codigo(&self) -> &'static str {
        match self {
            Aviso::EntradaVazia => "entrada_vazia",
            Aviso::FormatoDesconhecido => "formato_desconhecido",
            Aviso::SemFalas => "sem_falas",
            Aviso::DocxInvalido { .. } => "docx_invalido",
            Aviso::CodificacaoInvalida => "codificacao_invalida",
            Aviso::TextoAntesDaPrimeiraFala { .. } => "texto_antes_da_primeira_fala",
            Aviso::BlocosIgnorados { .. } => "blocos_ignorados",
        }
    }
}

impl fmt::Display for Aviso {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Aviso::EntradaVazia => write!(f, "O transcript está vazio."),
            Aviso::FormatoDesconhecido => write!(
                f,
                "Formato do transcript não reconhecido: o texto foi lido como texto corrido, sem falantes nem horários."
            ),
            Aviso::SemFalas => write!(f, "Nenhuma fala encontrada no transcript."),
            Aviso::DocxInvalido { motivo } => {
                write!(f, "Não foi possível ler o .docx: {motivo}.")
            }
            Aviso::CodificacaoInvalida => write!(
                f,
                "O arquivo não está em UTF-8; caracteres inválidos foram substituídos."
            ),
            Aviso::TextoAntesDaPrimeiraFala { linhas } => write!(
                f,
                "{linhas} linha(s) antes da primeira fala foram descartadas."
            ),
            Aviso::BlocosIgnorados { quantidade } => write!(
                f,
                "{quantidade} bloco(s) do WebVTT sem horário válido foram descartados."
            ),
        }
    }
}

/// Resultado da leitura do transcript.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transcript {
    /// Formato detectado.
    pub formato: Formato,
    /// Falas, na ordem do transcript, com `indice` de 1 em diante.
    pub falas: Vec<Fala>,
    /// Avisos da leitura; vazio quando tudo foi reconhecido.
    pub avisos: Vec<Aviso>,
}

impl Transcript {
    /// Falantes distintos, na ordem da primeira fala de cada um, como
    /// aparecem no transcript.
    pub fn falantes(&self) -> Vec<&str> {
        let mut vistos: Vec<&str> = Vec::new();
        for nome in self.falas.iter().filter_map(|f| f.falante.as_deref()) {
            if !vistos.contains(&nome) {
                vistos.push(nome);
            }
        }
        vistos
    }

    fn novo(formato: Formato, falas: Vec<FalaLida>, mut avisos: Vec<Aviso>) -> Self {
        let falas: Vec<Fala> = falas
            .into_iter()
            .enumerate()
            .map(|(i, f)| Fala {
                indice: i + 1,
                falante: f.falante,
                inicio: f.inicio,
                fim: f.fim,
                texto: f.texto,
            })
            .collect();
        if falas.is_empty() && !avisos.iter().any(sem_conteudo) {
            avisos.push(Aviso::SemFalas);
        }
        Self {
            formato,
            falas,
            avisos,
        }
    }

    fn vazio(avisos: Vec<Aviso>) -> Self {
        Self::novo(Formato::Desconhecido, Vec::new(), avisos)
    }
}

/// Avisos que já explicam a ausência de falas.
fn sem_conteudo(aviso: &Aviso) -> bool {
    matches!(aviso, Aviso::EntradaVazia | Aviso::DocxInvalido { .. })
}

/// Fala antes de receber o índice.
#[derive(Debug, Clone, PartialEq, Eq)]
struct FalaLida {
    falante: Option<String>,
    inicio: Option<Horario>,
    fim: Option<Horario>,
    texto: String,
}

/// Detecta o formato pelos bytes e, como desempate, pela extensão do nome do
/// arquivo (opcional). Não lê o transcript inteiro: para as falas, use
/// [`analisar`], que devolve o mesmo formato (ou `Desconhecido` quando o
/// formato reconhecido não tem falas legíveis).
pub fn detectar_formato(bytes: &[u8], nome_arquivo: Option<&str>) -> Formato {
    let extensao = extensao(nome_arquivo);
    if parece_zip(bytes) || extensao.as_deref() == Some("docx") {
        return Formato::Docx;
    }
    let (texto, _) = texto::decodificar(bytes);
    texto_detectado(&texto, extensao.as_deref())
}

/// Lê o transcript a partir dos bytes de um arquivo (`.vtt`, `.docx`, `.txt`)
/// ou do texto colado convertido em bytes. `nome_arquivo` é opcional e só
/// desempata a detecção (`.vtt` sem o cabeçalho `WEBVTT`, `.docx`).
///
/// Nunca falha: o que não dá para ler vira aviso no [`Transcript`].
pub fn analisar(bytes: &[u8], nome_arquivo: Option<&str>) -> Transcript {
    let extensao = extensao(nome_arquivo);
    if parece_zip(bytes) || extensao.as_deref() == Some("docx") {
        return analisar_docx(bytes);
    }
    let (texto, invalido) = texto::decodificar(bytes);
    let mut transcript = analisar_texto_com(&texto, extensao.as_deref());
    if invalido {
        transcript.avisos.insert(0, Aviso::CodificacaoInvalida);
    }
    transcript
}

/// Lê o transcript a partir de texto (o que o analista colou na tela). Aceita
/// também o conteúdo de um `.vtt`.
pub fn analisar_texto(texto: &str) -> Transcript {
    analisar_texto_com(texto, None)
}

fn analisar_texto_com(texto: &str, extensao: Option<&str>) -> Transcript {
    let texto = texto::normalizar(texto);
    if texto.trim().is_empty() {
        return Transcript::vazio(vec![Aviso::EntradaVazia]);
    }
    match texto_detectado(&texto, extensao) {
        Formato::Vtt => {
            let (falas, ignorados) = vtt::ler(&texto);
            let avisos = if ignorados > 0 {
                vec![Aviso::BlocosIgnorados {
                    quantidade: ignorados,
                }]
            } else {
                Vec::new()
            };
            Transcript::novo(Formato::Vtt, falas, avisos)
        }
        Formato::TextoTeams => {
            let linhas: Vec<&str> = texto.lines().collect();
            let lido = linhas::ler(&linhas);
            let avisos = if lido.linhas_antes > 0 {
                vec![Aviso::TextoAntesDaPrimeiraFala {
                    linhas: lido.linhas_antes,
                }]
            } else {
                Vec::new()
            };
            Transcript::novo(Formato::TextoTeams, lido.falas, avisos)
        }
        Formato::Docx | Formato::Desconhecido => texto_corrido(&texto),
    }
}

fn analisar_docx(bytes: &[u8]) -> Transcript {
    let paragrafos = match paragrafos_docx(bytes) {
        Ok(p) => p,
        Err(erro) => {
            return Transcript::vazio(vec![Aviso::DocxInvalido {
                motivo: erro.to_string(),
            }]);
        }
    };
    let juntos = paragrafos.join("\n");
    let texto = texto::normalizar(&juntos);
    if texto.trim().is_empty() {
        return Transcript::vazio(vec![Aviso::EntradaVazia]);
    }
    let linhas: Vec<&str> = texto.lines().collect();
    let lido = linhas::ler(&linhas);
    if lido.falas.is_empty() {
        return texto_corrido(&texto);
    }
    // O cabeçalho do export (título, data, duração) fica antes da primeira
    // fala e é descartado sem aviso.
    Transcript::novo(Formato::Docx, lido.falas, Vec::new())
}

/// Texto não reconhecido: uma fala só, sem falante nem horário.
fn texto_corrido(texto: &str) -> Transcript {
    let fala = FalaLida {
        falante: None,
        inicio: None,
        fim: None,
        texto: texto.trim().to_string(),
    };
    Transcript::novo(
        Formato::Desconhecido,
        vec![fala],
        vec![Aviso::FormatoDesconhecido],
    )
}

/// Formato de um texto já normalizado (sem BOM, finais de linha `\n`).
fn texto_detectado(texto: &str, extensao: Option<&str>) -> Formato {
    let texto = texto::normalizar(texto);
    let inicio = texto.trim_start();
    if inicio.is_empty() {
        return Formato::Desconhecido;
    }
    let primeira = inicio.lines().next().unwrap_or("");
    let tem_cue = texto.lines().any(vtt::linha_de_horario);
    if primeira == "WEBVTT" || primeira.starts_with("WEBVTT ") || primeira.starts_with("WEBVTT\t") {
        return Formato::Vtt;
    }
    if tem_cue && (extensao == Some("vtt") || texto.contains("<v ") || texto.contains("<v.")) {
        return Formato::Vtt;
    }
    let linhas: Vec<&str> = texto.lines().collect();
    if linhas::ler(&linhas).falas.is_empty() {
        Formato::Desconhecido
    } else {
        Formato::TextoTeams
    }
}

fn parece_zip(bytes: &[u8]) -> bool {
    bytes.starts_with(b"PK\x03\x04")
}

fn extensao(nome_arquivo: Option<&str>) -> Option<String> {
    let nome = nome_arquivo?;
    let (_, ext) = nome.rsplit_once('.')?;
    Some(ext.to_ascii_lowercase())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn horario_no_estilo_do_teams() {
        assert_eq!(Horario::de_milissegundos(3_120).to_string(), "0:03");
        assert_eq!(Horario::de_milissegundos(62_999).to_string(), "1:02");
        assert_eq!(Horario::de_milissegundos(3_723_000).to_string(), "1:02:03");
    }

    #[test]
    fn entrada_vazia_devolve_aviso() {
        for entrada in ["", "   ", "\n\r\n\t", "\u{feff}"] {
            let t = analisar_texto(entrada);
            assert_eq!(t.formato, Formato::Desconhecido);
            assert!(t.falas.is_empty());
            assert_eq!(t.avisos, vec![Aviso::EntradaVazia], "{entrada:?}");
        }
        let t = analisar(b"", None);
        assert_eq!(t.avisos, vec![Aviso::EntradaVazia]);
    }

    #[test]
    fn texto_desconhecido_vira_texto_corrido() {
        let t = analisar_texto("  Ata rascunhada à mão.\nSem nomes nem horários.\n");
        assert_eq!(t.formato, Formato::Desconhecido);
        assert_eq!(t.avisos, vec![Aviso::FormatoDesconhecido]);
        assert_eq!(t.falas.len(), 1);
        let fala = &t.falas[0];
        assert_eq!(fala.indice, 1);
        assert_eq!(fala.falante, None);
        assert_eq!(fala.inicio, None);
        assert_eq!(fala.texto, "Ata rascunhada à mão.\nSem nomes nem horários.");
    }

    #[test]
    fn aviso_serializa_com_codigo() {
        let json = serde_json::to_value(Aviso::FormatoDesconhecido).unwrap();
        assert_eq!(json, serde_json::json!({"codigo": "formato_desconhecido"}));
        let json = serde_json::to_value(Aviso::TextoAntesDaPrimeiraFala { linhas: 2 }).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"codigo": "texto_antes_da_primeira_fala", "linhas": 2})
        );
        for aviso in [
            Aviso::EntradaVazia,
            Aviso::SemFalas,
            Aviso::CodificacaoInvalida,
            Aviso::DocxInvalido { motivo: "x".into() },
            Aviso::BlocosIgnorados { quantidade: 1 },
        ] {
            let json = serde_json::to_value(&aviso).unwrap();
            assert_eq!(json["codigo"], aviso.codigo());
            assert!(!aviso.to_string().is_empty());
        }
    }

    #[test]
    fn transcript_serializa_horarios_em_milissegundos() {
        let t = analisar_texto("WEBVTT\n\n00:00:01.500 --> 00:00:02.000\n<v Ana>Oi</v>\n");
        let json = serde_json::to_value(&t).unwrap();
        assert_eq!(json["formato"], "vtt");
        assert_eq!(json["falas"][0]["inicio"], 1500);
        assert_eq!(json["falas"][0]["fim"], 2000);
        assert_eq!(json["falas"][0]["falante"], "Ana");
        let volta: Transcript = serde_json::from_value(json).unwrap();
        assert_eq!(volta, t);
    }

    #[test]
    fn deteccao_por_bytes_e_extensao() {
        assert_eq!(
            detectar_formato(b"WEBVTT\n\n00:01.000 --> 00:02.000\nOi\n", None),
            Formato::Vtt
        );
        // Sem o cabeçalho, a extensão desempata.
        let sem_cabecalho = b"00:00:01.000 --> 00:00:02.000\nOi\n";
        assert_eq!(
            detectar_formato(sem_cabecalho, Some("Reuniao.VTT")),
            Formato::Vtt
        );
        assert_eq!(
            analisar(sem_cabecalho, Some("reuniao.vtt")).falas[0].texto,
            "Oi"
        );
        assert_eq!(detectar_formato(b"PK\x03\x04resto", None), Formato::Docx);
        assert_eq!(detectar_formato(b"qualquer", Some("a.docx")), Formato::Docx);
        assert_eq!(
            detectar_formato("Ana Souza  0:03\nOi\n".as_bytes(), None),
            Formato::TextoTeams
        );
        assert_eq!(detectar_formato(b"so texto", None), Formato::Desconhecido);
    }

    #[test]
    fn docx_invalido_devolve_aviso() {
        let t = analisar(b"PK\x03\x04isto nao e um zip", None);
        assert_eq!(t.formato, Formato::Desconhecido);
        assert!(t.falas.is_empty());
        assert!(matches!(t.avisos.as_slice(), [Aviso::DocxInvalido { .. }]));
        let t = analisar(b"texto qualquer", Some("reuniao.docx"));
        assert!(matches!(t.avisos.as_slice(), [Aviso::DocxInvalido { .. }]));
    }

    #[test]
    fn bytes_que_nao_sao_utf8_viram_aviso() {
        let t = analisar(b"Ana Souza  0:03\nol\xe1\n", None);
        assert_eq!(t.formato, Formato::TextoTeams);
        assert_eq!(t.avisos, vec![Aviso::CodificacaoInvalida]);
        assert_eq!(t.falas[0].texto, "ol\u{fffd}");
    }

    #[test]
    fn utf16_com_bom_e_lido() {
        let mut bytes = vec![0xff, 0xfe];
        for u in "WEBVTT\n\n00:00:01.000 --> 00:00:02.000\n<v João>Olá</v>\n".encode_utf16() {
            bytes.extend_from_slice(&u.to_le_bytes());
        }
        let t = analisar(&bytes, None);
        assert_eq!(t.formato, Formato::Vtt);
        assert_eq!(t.falas[0].falante.as_deref(), Some("João"));
        assert_eq!(t.falas[0].texto, "Olá");
    }

    #[test]
    fn vtt_sem_cues_avisa_sem_falas() {
        let t = analisar_texto("WEBVTT\n\nNOTE só uma nota\n");
        assert_eq!(t.formato, Formato::Vtt);
        assert!(t.falas.is_empty());
        assert_eq!(t.avisos, vec![Aviso::SemFalas]);
    }

    #[test]
    fn nunca_entra_em_panico() {
        // Entradas estranhas e cortadas no meio de caracteres e de marcas.
        let base = "WEBVTT\r\n\r\n1\r\n00:00:01.000 --> 00:00:02.000\r\n<v Jo\u{e3}o>Ol\u{e1} &amp; <b>at\u{e9}</b></v>\r\n\r\nAna  0:03\nfala\n0:0:0.0 --> 0:0:1.0\nNome\n";
        let bytes = base.as_bytes();
        for fim in 0..=bytes.len() {
            let _ = analisar(&bytes[..fim], None);
            let _ = analisar(&bytes[..fim], Some("x.vtt"));
            let _ = analisar(&bytes[fim..], None);
        }
        let mut lixo = Vec::new();
        let mut x: u32 = 12345;
        for _ in 0..4096 {
            x = x.wrapping_mul(1_103_515_245).wrapping_add(12345);
            lixo.push((x >> 16) as u8);
        }
        let _ = analisar(&lixo, None);
        let _ = analisar(&lixo, Some("a.docx"));
        let mut zipado = b"PK\x03\x04".to_vec();
        zipado.extend_from_slice(&lixo);
        let _ = analisar(&zipado, None);
        for estranho in [
            "<v",
            "<v ",
            "<v>",
            "</v>",
            "-->",
            " --> ",
            "WEBVTT\n\n-->\n<v",
            "WEBVTT\n\n99999999999999999999:00:00.000 --> 1\nx",
            "a 0:00",
            "  0:00",
            "\t0:00:00",
            "&",
            "&#xFFFFFFFF;",
            "&#;",
        ] {
            let _ = analisar_texto(estranho);
        }
    }
}
