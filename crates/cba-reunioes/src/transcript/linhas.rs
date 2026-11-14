//! Leitura por linhas: texto copiado da tela do Teams e parágrafos do `.docx`.
//!
//! Uma fala começa numa linha de cabeçalho e vai até o próximo cabeçalho. Há
//! dois tipos de cabeçalho:
//!
//! 1. **Nome e horário na mesma linha** (tela do Teams e export atual do
//!    `.docx`): `Silva, Maria  0:03`, `Ana Souza 00:00:03`,
//!    `João Pereira\t1:02:03`. O horário é `M:SS`, `MM:SS` ou `H:MM:SS` no fim
//!    da linha. Para não confundir uma fala que termina em horário ("vamos às
//!    10:30") com um cabeçalho:
//!    - com um espaço só entre o nome e o horário, o horário precisa ter as
//!      horas (`H:MM:SS`); com dois ou mais espaços ou tabulação, basta
//!      `M:SS`;
//!    - o nome tem até 100 caracteres, ao menos uma letra e não termina em
//!      `,`, `;`, `:`, `!` ou `?`;
//!    - o horário não volta para trás em relação ao cabeçalho anterior.
//! 2. **Bloco do export antigo**: linha `0:0:0.0 --> 0:0:3.250`, seguida da
//!    linha com o nome. Esse é o único formato que traz o fim da fala.
//!
//! As linhas da fala são aparadas, as vazias são descartadas e o resto fica
//! separado por `\n`. Linhas de sistema do Teams ("Maria Silva started
//! transcription", "… iniciou a transcrição", "… parou a transcrição") são
//! descartadas.

use super::texto::juntar_linhas;
use super::{FalaLida, Horario, vtt};

/// Resultado da leitura por linhas.
pub(super) struct Lido {
    pub falas: Vec<FalaLida>,
    /// Linhas não vazias antes do primeiro cabeçalho (descartadas).
    pub linhas_antes: usize,
}

/// Fala em montagem: cabeçalho e linhas de texto.
struct Aberta<'a> {
    falante: String,
    inicio: Horario,
    fim: Option<Horario>,
    linhas: Vec<&'a str>,
}

impl Aberta<'_> {
    fn fechar(self) -> FalaLida {
        FalaLida {
            falante: Some(self.falante),
            inicio: Some(self.inicio),
            fim: self.fim,
            texto: juntar_linhas(self.linhas),
        }
    }
}

pub(super) fn ler<'a>(linhas: &[&'a str]) -> Lido {
    let mut falas = Vec::new();
    let mut aberta: Option<Aberta<'a>> = None;
    let mut linhas_antes = 0;
    let mut i = 0;
    while i < linhas.len() {
        let linha = linhas[i].trim();
        let ultimo = aberta.as_ref().map(|a| a.inicio);
        let avanca = |h: Horario| ultimo.is_none_or(|u| h >= u);

        // Bloco do export antigo: horário e, na próxima linha não vazia, o nome.
        if let Some((inicio, fim)) = vtt::horarios(linha).filter(|(h, _)| avanca(*h)) {
            let proxima = (i + 1..linhas.len()).find(|&j| !linhas[j].trim().is_empty());
            if let Some(j) = proxima {
                let nome = linhas[j].trim();
                if !nome.contains("-->") {
                    if let Some(a) = aberta.take() {
                        falas.push(a.fechar());
                    }
                    aberta = Some(Aberta {
                        falante: nome.to_string(),
                        inicio,
                        fim: Some(fim),
                        linhas: Vec::new(),
                    });
                    i = j + 1;
                    continue;
                }
            }
        }

        if let Some((nome, inicio)) = cabecalho(linha).filter(|(_, h)| avanca(*h)) {
            if let Some(a) = aberta.take() {
                falas.push(a.fechar());
            }
            aberta = Some(Aberta {
                falante: nome.to_string(),
                inicio,
                fim: None,
                linhas: Vec::new(),
            });
        } else if linha_de_sistema(linha) {
            // Descartada.
        } else if let Some(a) = aberta.as_mut() {
            a.linhas.push(linha);
        } else if !linha.is_empty() {
            linhas_antes += 1;
        }
        i += 1;
    }
    if let Some(a) = aberta {
        falas.push(a.fechar());
    }
    Lido {
        falas,
        linhas_antes,
    }
}

/// Nome e horário de uma linha de cabeçalho `Nome  0:03`.
fn cabecalho(linha: &str) -> Option<(&str, Horario)> {
    let (antes, hora) = linha.rsplit_once(char::is_whitespace)?;
    let nome = antes.trim_end();
    let separador = &linha[nome.len()..linha.len() - hora.len()];
    let (horario, com_horas) = horario_curto(hora)?;
    if separador == " " && !com_horas {
        return None;
    }
    let ultimo = nome.chars().last()?;
    if nome.chars().count() > 100
        || !nome.chars().any(char::is_alphabetic)
        || matches!(ultimo, ',' | ';' | ':' | '!' | '?')
        || nome.contains("-->")
    {
        return None;
    }
    Some((nome, horario))
}

/// Horário `M:SS`, `MM:SS` ou `H:MM:SS` (sem fração). Devolve também se tem as
/// horas.
fn horario_curto(texto: &str) -> Option<(Horario, bool)> {
    let partes: Vec<&str> = texto.split(':').collect();
    let numero = |p: &str, min: usize, max: usize| -> Option<u64> {
        if p.len() < min || p.len() > max || !p.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        p.parse().ok()
    };
    let (h, m, s) = match partes.as_slice() {
        [h, m, s] => (numero(h, 1, 2)?, numero(m, 2, 2)?, numero(s, 2, 2)?),
        [m, s] => (0, numero(m, 1, 2)?, numero(s, 2, 2)?),
        _ => return None,
    };
    if m >= 60 || s >= 60 {
        return None;
    }
    let ms = ((h * 60 + m) * 60 + s) * 1000;
    Some((Horario::de_milissegundos(ms), partes.len() == 3))
}

/// Linha de sistema do Teams sobre a transcrição (não é fala).
fn linha_de_sistema(linha: &str) -> bool {
    let minuscula = linha.trim_end_matches('.').to_lowercase();
    [
        "started transcription",
        "stopped transcription",
        "iniciou a transcrição",
        "parou a transcrição",
        "interrompeu a transcrição",
    ]
    .iter()
    .any(|fim| minuscula.ends_with(fim))
}

#[cfg(test)]
mod testes {
    use super::super::{Aviso, Formato, analisar_texto};
    use super::*;

    fn segundos(s: u64) -> Horario {
        Horario::de_milissegundos(s * 1000)
    }

    #[test]
    fn cabecalhos_aceitos() {
        assert_eq!(cabecalho("Nome  0:03"), Some(("Nome", segundos(3))));
        assert_eq!(cabecalho("Nome 00:00:03"), Some(("Nome", segundos(3))));
        assert_eq!(cabecalho("Nome   1:02:03"), Some(("Nome", segundos(3723))));
        assert_eq!(cabecalho("Nome\t12:34"), Some(("Nome", segundos(754))));
        assert_eq!(
            cabecalho("Silva, Maria  0:03"),
            Some(("Silva, Maria", segundos(3)))
        );
        assert_eq!(
            cabecalho("carlos.lima@exemplo.com  0:03"),
            Some(("carlos.lima@exemplo.com", segundos(3)))
        );
        assert_eq!(cabecalho("Maria S.  0:03"), Some(("Maria S.", segundos(3))));
    }

    #[test]
    fn falas_que_terminam_em_horario_nao_sao_cabecalho() {
        assert_eq!(cabecalho("Vamos marcar às 10:30"), None);
        assert_eq!(cabecalho("10 de novembro de 2026, 10:00"), None);
        assert_eq!(cabecalho("Pode ser:  10:30"), None);
        assert_eq!(cabecalho("  0:03"), None);
        assert_eq!(cabecalho("123  0:03"), None);
        assert_eq!(cabecalho("Nome  0:3"), None);
        assert_eq!(cabecalho("Nome  0:60"), None);
        assert_eq!(cabecalho("Nome  1:2:3"), None);
        assert_eq!(cabecalho("Nome  5:00PM"), None);
    }

    #[test]
    fn texto_copiado_da_tela() {
        let t = analisar_texto(
            "Silva, Maria  0:03\nBom dia.\n\nSilva, Maria  0:13\nSegunda fala,\ncom duas linhas.\n\nAna Souza 00:01:05\nVamos marcar às 10:30\n\nJoão Pereira   1:02:03\nAté.\n",
        );
        assert_eq!(t.formato, Formato::TextoTeams);
        assert!(t.avisos.is_empty(), "{:?}", t.avisos);
        let falas: Vec<_> = t
            .falas
            .iter()
            .map(|f| {
                (
                    f.indice,
                    f.falante.as_deref().unwrap(),
                    f.inicio.unwrap().segundos(),
                    f.fim,
                    f.texto.as_str(),
                )
            })
            .collect();
        assert_eq!(
            falas,
            vec![
                (1, "Silva, Maria", 3, None, "Bom dia."),
                (
                    2,
                    "Silva, Maria",
                    13,
                    None,
                    "Segunda fala,\ncom duas linhas."
                ),
                (3, "Ana Souza", 65, None, "Vamos marcar às 10:30"),
                (4, "João Pereira", 3723, None, "Até."),
            ]
        );
    }

    #[test]
    fn horario_que_volta_e_texto() {
        let t = analisar_texto("Ana  0:10\nOi\nPedro  0:05\nBeto  0:12\nTchau\n");
        let falas: Vec<_> = t.falas.iter().map(|f| f.texto.as_str()).collect();
        assert_eq!(falas, vec!["Oi\nPedro  0:05", "Tchau"]);
    }

    #[test]
    fn texto_antes_da_primeira_fala_avisa() {
        let t = analisar_texto("Transcrição\nReunião semanal\n\nAna  0:03\nOi\n");
        assert_eq!(t.formato, Formato::TextoTeams);
        assert_eq!(
            t.avisos,
            vec![Aviso::TextoAntesDaPrimeiraFala { linhas: 2 }]
        );
        assert_eq!(t.falas.len(), 1);
    }

    #[test]
    fn linhas_de_sistema_sao_descartadas() {
        let t = analisar_texto(
            "Ana Souza iniciou a transcrição\nAna  0:03\nOi\nAna Souza started transcription.\nAna Souza parou a transcrição\n",
        );
        assert!(t.avisos.is_empty());
        assert_eq!(t.falas.len(), 1);
        assert_eq!(t.falas[0].texto, "Oi");
    }

    #[test]
    fn bloco_do_export_antigo() {
        let t = analisar_texto(
            "0:0:0.0 --> 0:0:3.250\nSilva, Maria\nBom dia.\n\n0:0:3.250 --> 0:0:7.10\nAna Souza\nOi.\n",
        );
        assert_eq!(t.formato, Formato::TextoTeams);
        assert_eq!(t.falas.len(), 2);
        assert_eq!(t.falas[0].falante.as_deref(), Some("Silva, Maria"));
        assert_eq!(t.falas[0].inicio, Some(Horario::de_milissegundos(0)));
        assert_eq!(t.falas[0].fim, Some(Horario::de_milissegundos(3_250)));
        assert_eq!(t.falas[0].texto, "Bom dia.");
        assert_eq!(t.falas[1].fim, Some(Horario::de_milissegundos(7_100)));
    }
}
