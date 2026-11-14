//! WebVTT do Teams.
//!
//! ```text
//! WEBVTT
//!
//! 6d1f0c2a-8b3e-4f5a-9c7d-2e4b6a8c0d1f/11-0
//! 00:00:03.120 --> 00:00:11.952
//! <v Silva, Maria>Bom dia, pessoal.</v>
//! ```
//!
//! Os blocos são separados por linha vazia. O primeiro (`WEBVTT`) e os blocos
//! `NOTE`, `STYLE` e `REGION` são ignorados; num cue, a linha antes do
//! horário é o id (opcional) e as seguintes são o texto.

use super::texto::{decodificar_entidades, juntar_linhas};
use super::{FalaLida, Horario};

/// Lê as falas de um WebVTT já normalizado. Devolve também quantos blocos
/// foram descartados por não serem cues válidos.
pub(super) fn ler(texto: &str) -> (Vec<FalaLida>, usize) {
    let mut falas = Vec::new();
    let mut ignorados = 0;
    for (n, bloco) in blocos(texto).into_iter().enumerate() {
        let primeira = bloco[0].trim_start();
        if n == 0 && primeira.starts_with("WEBVTT") {
            continue;
        }
        if ["NOTE", "STYLE", "REGION"].iter().any(|p| {
            primeira
                .strip_prefix(p)
                .is_some_and(|resto| resto.is_empty() || resto.starts_with(char::is_whitespace))
        }) {
            continue;
        }
        // A linha de horário é a primeira ou a segunda (depois do id).
        let Some(pos) = bloco.iter().take(2).position(|l| l.contains("-->")) else {
            ignorados += 1;
            continue;
        };
        let Some((inicio, fim)) = horarios(bloco[pos]) else {
            ignorados += 1;
            continue;
        };
        let carga = bloco[pos + 1..].join("\n");
        for (falante, texto) in vozes(&carga) {
            falas.push(FalaLida {
                falante,
                inicio: Some(inicio),
                fim: Some(fim),
                texto,
            });
        }
    }
    (falas, ignorados)
}

/// Blocos não vazios, separados por linhas em branco.
fn blocos(texto: &str) -> Vec<Vec<&str>> {
    let mut blocos = Vec::new();
    let mut atual: Vec<&str> = Vec::new();
    for linha in texto.lines() {
        if linha.trim().is_empty() {
            if !atual.is_empty() {
                blocos.push(std::mem::take(&mut atual));
            }
        } else {
            atual.push(linha);
        }
    }
    if !atual.is_empty() {
        blocos.push(atual);
    }
    blocos
}

/// Se a linha é uma linha de horário de cue (`início --> fim [ajustes]`).
pub(super) fn linha_de_horario(linha: &str) -> bool {
    horarios(linha).is_some()
}

/// Início e fim de uma linha `início --> fim`, ignorando os ajustes do cue
/// depois do fim.
pub(super) fn horarios(linha: &str) -> Option<(Horario, Horario)> {
    let (antes, depois) = linha.split_once("-->")?;
    let inicio = horario(antes.trim())?;
    let fim = horario(depois.split_whitespace().next()?)?;
    Some((inicio, fim))
}

/// Horário `[H:]M:S[.fração]`. Aceita o formato do WebVTT (`00:00:03.120`,
/// `00:03.120`) e o do export antigo do Teams (`0:0:3.25`). A fração aceita
/// `.` ou `,` e de 1 a 3 dígitos (dígitos a mais são desprezados).
pub(super) fn horario(texto: &str) -> Option<Horario> {
    let (inteiro, fracao) = match texto.split_once(['.', ',']) {
        Some((i, f)) => (i, Some(f)),
        None => (texto, None),
    };
    let partes: Vec<&str> = inteiro.split(':').collect();
    let digitos = |p: &str, max: usize| {
        !p.is_empty() && p.len() <= max && p.bytes().all(|b| b.is_ascii_digit())
    };
    let (h, m, s) = match partes.as_slice() {
        [h, m, s] if digitos(h, 6) && digitos(m, 2) && digitos(s, 2) => (*h, *m, *s),
        [m, s] if digitos(m, 4) && digitos(s, 2) => ("0", *m, *s),
        _ => return None,
    };
    let h: u64 = h.parse().ok()?;
    let m: u64 = m.parse().ok()?;
    let s: u64 = s.parse().ok()?;
    if s >= 60 || (partes.len() == 3 && m >= 60) {
        return None;
    }
    let ms = match fracao {
        None => 0,
        Some(f) if !f.is_empty() && f.bytes().all(|b| b.is_ascii_digit()) => {
            let tres: String = f.chars().chain("00".chars()).take(3).collect();
            tres.parse::<u64>().ok()?
        }
        Some(_) => return None,
    };
    Some(Horario::de_milissegundos(
        ((h * 60 + m) * 60 + s) * 1000 + ms,
    ))
}

/// Separa o texto de um cue pelas marcas de voz. Cada `<v Nome>` abre um
/// trecho do falante, que vai até o `</v>` ou a próxima marca de voz. Texto
/// fora de marca de voz fica sem falante. As demais marcas (`<c>`, `<i>`,
/// `<b>`, `<u>`, `<lang>`, horários internos) são retiradas. Trechos vazios
/// são descartados.
fn vozes(carga: &str) -> Vec<(Option<String>, String)> {
    let mut trechos = Vec::new();
    let mut falante: Option<String> = None;
    let mut atual = String::new();
    fn fechar(
        falante: &Option<String>,
        atual: &mut String,
        trechos: &mut Vec<(Option<String>, String)>,
    ) {
        let texto = juntar_linhas(decodificar_entidades(atual).lines());
        if !texto.is_empty() {
            trechos.push((falante.clone(), texto));
        }
        atual.clear();
    }
    let mut resto = carga;
    while let Some(pos) = resto.find('<') {
        atual.push_str(&resto[..pos]);
        let depois = &resto[pos + 1..];
        let Some(fim) = depois.find('>') else {
            // `<` sem fechamento: texto comum.
            atual.push('<');
            resto = depois;
            continue;
        };
        let marca = depois[..fim].trim();
        resto = &depois[fim + 1..];
        if let Some(nome) = marca_de_voz(marca) {
            fechar(&falante, &mut atual, &mut trechos);
            falante = nome;
        } else if marca == "/v" {
            fechar(&falante, &mut atual, &mut trechos);
            falante = None;
        }
    }
    atual.push_str(resto);
    fechar(&falante, &mut atual, &mut trechos);
    trechos
}

/// Nome da marca de voz (`v Nome` ou `v.classe Nome`). `None` se a marca não é
/// de voz; `Some(None)` se é de voz sem nome.
fn marca_de_voz(marca: &str) -> Option<Option<String>> {
    let resto = marca.strip_prefix('v')?;
    let nome = if let Some(com_classe) = resto.strip_prefix('.') {
        // `v.classe1.classe2 Nome`: o nome vem depois do primeiro espaço.
        com_classe
            .split_once(char::is_whitespace)
            .map_or("", |(_, nome)| nome)
    } else if resto.starts_with(char::is_whitespace) {
        resto
    } else if resto.is_empty() {
        ""
    } else {
        // `<video>` e afins não são marca de voz.
        return None;
    };
    let nome = decodificar_entidades(nome.trim()).trim().to_string();
    Some((!nome.is_empty()).then_some(nome))
}

#[cfg(test)]
mod testes {
    use super::super::{Aviso, Formato, analisar, analisar_texto};
    use super::*;

    fn ms(ms: u64) -> Option<Horario> {
        Some(Horario::de_milissegundos(ms))
    }

    #[test]
    fn horarios_aceitos() {
        assert_eq!(horario("00:00:03.120"), ms(3_120));
        assert_eq!(horario("01:02:03.004"), ms(3_723_004));
        assert_eq!(horario("02:03.5"), ms(123_500));
        assert_eq!(horario("0:0:3.25"), ms(3_250));
        assert_eq!(horario("0:0:0.0"), ms(0));
        assert_eq!(horario("00:00:01,000"), ms(1_000));
        assert_eq!(horario("00:00:01.12345"), ms(1_123));
        assert_eq!(horario("00:61:00.000"), None);
        assert_eq!(horario("00:00:60.000"), None);
        assert_eq!(horario("a:00:00.000"), None);
        assert_eq!(horario("00:00:01."), None);
        assert_eq!(horario(""), None);
    }

    #[test]
    fn cue_do_teams_com_id_e_voz() {
        let t = analisar_texto(
            "WEBVTT\n\n6d1f/11-0\n00:00:03.120 --> 00:00:11.952\n<v Silva, Maria>Bom dia.</v>\n\n6d1f/14-0\n00:00:13.594 --> 00:00:22.282 align:start\n<v Silva, Maria>Segunda fala.</v>\n",
        );
        assert_eq!(t.formato, Formato::Vtt);
        assert!(t.avisos.is_empty());
        assert_eq!(t.falas.len(), 2);
        assert_eq!(t.falas[0].indice, 1);
        assert_eq!(t.falas[0].falante.as_deref(), Some("Silva, Maria"));
        assert_eq!(t.falas[0].inicio, ms(3_120));
        assert_eq!(t.falas[0].fim, ms(11_952));
        assert_eq!(t.falas[0].texto, "Bom dia.");
        // Falas consecutivas do mesmo falante continuam separadas.
        assert_eq!(t.falas[1].indice, 2);
        assert_eq!(t.falas[1].falante.as_deref(), Some("Silva, Maria"));
        assert_eq!(t.falas[1].texto, "Segunda fala.");
    }

    #[test]
    fn cue_multilinha() {
        let t = analisar_texto(
            "WEBVTT\n\n00:00:01.000 --> 00:00:05.000\n<v João Pereira>Primeira linha\n  segunda linha  </v>\n",
        );
        assert_eq!(t.falas.len(), 1);
        assert_eq!(t.falas[0].texto, "Primeira linha\nsegunda linha");
    }

    #[test]
    fn cue_sem_voz_fica_sem_falante() {
        let t = analisar_texto("WEBVTT\n\n00:01.000 --> 00:02.000\nTexto sem marca de voz\n");
        assert_eq!(t.falas.len(), 1);
        assert_eq!(t.falas[0].falante, None);
        assert_eq!(t.falas[0].texto, "Texto sem marca de voz");
    }

    #[test]
    fn bom_e_crlf() {
        let entrada = "\u{feff}WEBVTT\r\n\r\n1\r\n00:00:01.000 --> 00:00:02.000\r\n<v Ana Souza>Olá\r\ntodos</v>\r\n\r\n";
        let t = analisar(entrada.as_bytes(), None);
        assert_eq!(t.formato, Formato::Vtt);
        assert!(t.avisos.is_empty(), "{:?}", t.avisos);
        assert_eq!(t.falas.len(), 1);
        assert_eq!(t.falas[0].falante.as_deref(), Some("Ana Souza"));
        assert_eq!(t.falas[0].texto, "Olá\ntodos");
        // Só CR (finais de linha antigos do Mac).
        let t = analisar_texto(&entrada.replace("\r\n", "\r"));
        assert_eq!(t.falas[0].texto, "Olá\ntodos");
    }

    #[test]
    fn nomes_preservados_como_aparecem() {
        let t = analisar_texto(
            "WEBVTT\n\n00:01.000 --> 00:02.000\n<v carlos.lima@exemplo.com>Oi</v>\n\n00:03.000 --> 00:04.000\n<v.loud Conceição, Ângela (Externa)>Oi</v>\n\n00:05.000 --> 00:06.000\n<v Tom &amp; Jerry>Oi</v>\n",
        );
        assert_eq!(
            t.falantes(),
            vec![
                "carlos.lima@exemplo.com",
                "Conceição, Ângela (Externa)",
                "Tom & Jerry"
            ]
        );
    }

    #[test]
    fn varias_vozes_e_marcas_internas() {
        let t = analisar_texto(
            "WEBVTT\n\n00:01.000 --> 00:02.000\n<v Ana>Pode ser <i>hoje</i>?</v> <v Beto>Pode, &lt;sim&gt;.</v>\n\n00:03.000 --> 00:04.000\n<v Ana>sem fechar <00:03.500><c>marca</c>\n",
        );
        let pares: Vec<_> = t
            .falas
            .iter()
            .map(|f| (f.falante.as_deref(), f.texto.as_str()))
            .collect();
        assert_eq!(
            pares,
            vec![
                (Some("Ana"), "Pode ser hoje?"),
                (Some("Beto"), "Pode, <sim>."),
                (Some("Ana"), "sem fechar marca"),
            ]
        );
    }

    #[test]
    fn notas_estilos_e_blocos_invalidos() {
        let t = analisar_texto(
            "WEBVTT - Reunião\n\nNOTE\ngravada no Teams\n\nSTYLE\n::cue { color: red }\n\nsem horario\nnada\n\n1\n00:00:xx --> 00:00:02.000\n<v Ana>quebrado</v>\n\n00:00:03.000 --> 00:00:04.000\n<v Ana>ok</v>\n\n00:00:05.000 --> 00:00:06.000\n\n",
        );
        assert_eq!(t.falas.len(), 1);
        assert_eq!(t.falas[0].texto, "ok");
        assert_eq!(t.avisos, vec![Aviso::BlocosIgnorados { quantidade: 2 }]);
    }

    #[test]
    fn vtt_sem_cabecalho_com_voz_e_reconhecido() {
        let t = analisar_texto("00:00:01.000 --> 00:00:02.000\n<v Ana>Oi</v>\n");
        assert_eq!(t.formato, Formato::Vtt);
        assert_eq!(t.falas[0].falante.as_deref(), Some("Ana"));
    }
}
