//! Leitura do formato "Nome: fala", uma fala por linha (comum ao copiar de
//! chat), com ou sem horário antes do nome:
//!
//! ```text
//! Marina: Bom dia.
//! [00:01:02] Rafael: Vamos começar.
//! 00:01 Camila: Concordo.
//! ```
//!
//! Horário aceito antes do nome: `[H:MM:SS]`, `[M:SS]`, `(…)` ou sem
//! delimitador (`00:01 Nome: fala`), opcionalmente seguido de `-`.
//!
//! Heurística contra falso positivo (linhas como "Obs: ..." ou
//! "http://..."). Uma linha é de fala quando:
//!
//! - depois do horário opcional vem um nome, os dois-pontos e então espaço ou
//!   o fim da linha (`http://` e `10:30` não casam);
//! - o nome tem até 40 caracteres e 5 palavras, começa com letra maiúscula,
//!   só tem letras, dígitos, espaço, `.`, `'` e `-`, e não é um rótulo comum
//!   ("Obs", "Nota", "Pauta", "Assunto", "Participantes"…).
//!
//! O texto só é lido assim quando ao menos 2 linhas casam e elas são a maioria
//! das linhas não vazias. As linhas que não casam depois da primeira fala
//! continuam a fala anterior (fala de várias linhas); as de antes dela são
//! descartadas com aviso.

use super::linhas::horario_curto;
use super::texto::juntar_linhas;
use super::{FalaLida, Horario};

/// Resultado da leitura.
pub(super) struct Lido {
    pub falas: Vec<FalaLida>,
    /// Linhas não vazias antes da primeira fala (descartadas).
    pub linhas_antes: usize,
}

/// Rótulos que parecem nome mas não são falante (comparação em minúsculas).
const ROTULOS: &[&str] = &[
    "obs",
    "obs.",
    "observação",
    "observacao",
    "observações",
    "nota",
    "notas",
    "pauta",
    "ata",
    "data",
    "hora",
    "local",
    "assunto",
    "tema",
    "título",
    "titulo",
    "participantes",
    "presentes",
    "ausentes",
    "resumo",
    "decisão",
    "decisao",
    "pendência",
    "pendencia",
    "ação",
    "acao",
    "atenção",
    "atencao",
    "importante",
    "exemplo",
    "reunião",
    "reuniao",
    "link",
    "url",
    "http",
    "https",
    "re",
    "fwd",
    "de",
    "para",
    "cc",
    "ps",
    "nb",
    "att",
    "ref",
    "fonte",
    "legenda",
];

/// Lê o texto no formato "Nome: fala". Devolve `None` quando o texto não passa
/// na heurística (ver a documentação do módulo).
pub(super) fn ler(linhas: &[&str]) -> Option<Lido> {
    let nao_vazias = linhas.iter().filter(|l| !l.trim().is_empty()).count();
    let casadas = linhas
        .iter()
        .filter(|l| linha_de_fala(l.trim()).is_some())
        .count();
    if casadas < 2 || casadas * 2 <= nao_vazias {
        return None;
    }

    let mut falas: Vec<FalaLida> = Vec::new();
    let mut atual: Option<(FalaLida, Vec<&str>)> = None;
    let mut linhas_antes = 0;
    for linha in linhas {
        let linha = linha.trim();
        if let Some((inicio, nome, resto)) = linha_de_fala(linha) {
            if let Some((fala, partes)) = atual.take() {
                falas.push(fechar(fala, partes));
            }
            let fala = FalaLida {
                falante: Some(nome.to_string()),
                inicio,
                fim: None,
                texto: String::new(),
            };
            let partes = if resto.is_empty() {
                vec![]
            } else {
                vec![resto]
            };
            atual = Some((fala, partes));
        } else if linha.is_empty() {
            // Linha em branco separa nada: a fala continua até a próxima.
        } else if let Some((_, partes)) = atual.as_mut() {
            partes.push(linha);
        } else {
            linhas_antes += 1;
        }
    }
    if let Some((fala, partes)) = atual {
        falas.push(fechar(fala, partes));
    }
    Some(Lido {
        falas,
        linhas_antes,
    })
}

fn fechar(mut fala: FalaLida, partes: Vec<&str>) -> FalaLida {
    fala.texto = juntar_linhas(partes);
    fala
}

/// Horário, nome e texto de uma linha aparada no formato "[horário] Nome: fala".
fn linha_de_fala(linha: &str) -> Option<(Option<Horario>, &str, &str)> {
    let (inicio, resto) = horario_inicial(linha);
    let (nome, depois) = resto.split_once(':')?;
    if !depois.is_empty() && !depois.starts_with(char::is_whitespace) {
        return None;
    }
    let nome = nome.trim();
    if !nome_valido(nome) {
        return None;
    }
    Some((inicio, nome, depois.trim()))
}

/// Separa o horário opcional do começo da linha.
fn horario_inicial(linha: &str) -> (Option<Horario>, &str) {
    let fechamento = match linha.chars().next() {
        Some('[') => Some(']'),
        Some('(') => Some(')'),
        _ => None,
    };
    let (token, resto) = match fechamento {
        Some(f) => match linha[1..].split_once(f) {
            Some((dentro, resto)) => (dentro.trim(), resto),
            None => return (None, linha),
        },
        None => match linha.split_once(char::is_whitespace) {
            Some((token, resto)) => (token, resto),
            None => return (None, linha),
        },
    };
    match horario_curto(token) {
        Some((h, _)) => {
            let resto = resto.trim_start();
            let resto = resto
                .strip_prefix(['-', '–', '—'])
                .filter(|r| r.starts_with(char::is_whitespace))
                .unwrap_or(resto);
            (Some(h), resto.trim_start())
        }
        None => (None, linha),
    }
}

fn nome_valido(nome: &str) -> bool {
    let mut letras = nome.chars();
    let Some(primeira) = letras.next() else {
        return false;
    };
    primeira.is_alphabetic()
        && primeira.is_uppercase()
        && nome.chars().count() <= 40
        && nome.split_whitespace().count() <= 5
        && nome
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, ' ' | '.' | '\'' | '-'))
        && !ROTULOS.contains(&nome.to_lowercase().as_str())
}

#[cfg(test)]
mod testes {
    use super::super::{Aviso, Formato, analisar_texto};
    use super::*;

    fn lido(texto: &str) -> Option<Lido> {
        let linhas: Vec<&str> = texto.lines().collect();
        ler(&linhas)
    }

    #[test]
    fn linhas_de_fala_com_e_sem_horario() {
        let seg = |s: u64| Some(Horario::de_milissegundos(s * 1000));
        assert_eq!(
            linha_de_fala("Marina: Bom dia."),
            Some((None, "Marina", "Bom dia."))
        );
        assert_eq!(
            linha_de_fala("[00:01:02] Rafael Souza: Vamos."),
            Some((seg(62), "Rafael Souza", "Vamos."))
        );
        assert_eq!(
            linha_de_fala("00:01 Camila: Ok."),
            Some((seg(1), "Camila", "Ok."))
        );
        assert_eq!(
            linha_de_fala("(1:05) - Camila: Ok."),
            Some((seg(65), "Camila", "Ok."))
        );
        assert_eq!(linha_de_fala("Maria S.:"), Some((None, "Maria S.", "")));
    }

    #[test]
    fn falsos_positivos_nao_casam() {
        for linha in [
            "Obs: confirmar o prazo",
            "http://exemplo.com/fatura",
            "https://exemplo.com",
            "Vamos às 10:30",
            "marina: minúscula",
            "Marina:sem espaço",
            "Pauta: cancelamento",
            "Uma frase longa demais para ser nome de gente de verdade: oi",
            "1. Item: valor",
            "00:01 sem dois pontos",
            "[nota] Ana: x",
        ] {
            assert_eq!(linha_de_fala(linha), None, "{linha}");
        }
    }

    #[test]
    fn exige_maioria_e_ao_menos_duas_linhas() {
        assert!(lido("Obs: só isso\n").is_none());
        assert!(lido("Marina: uma só\n").is_none());
        assert!(lido("Obs: a\nObs: b\nObs: c\n").is_none());
        // Uma linha casando entre várias que não casam.
        assert!(lido("Texto solto\nOutro texto\nMarina: oi\nMais texto\n").is_none());
    }

    #[test]
    fn tres_participantes_do_faturamento_exemplo() {
        let t = analisar_texto(
            "Marina: Bom dia, vamos falar do cancelamento de fatura.\n\
             Rafael: A trava dos 30 dias fica na tela de retaguarda.\n\
             \n\
             Camila: Decidimos manter a trava e registrar o motivo.\n\
             Quem aprova o cancelamento após 30 dias?\n\
             Marina: Aprova o gerente financeiro.\n\
             Obs: confirmar com o jurídico.\n",
        );
        assert_eq!(t.formato, Formato::NomeFala);
        assert!(t.avisos.is_empty(), "{:?}", t.avisos);
        assert_eq!(t.falantes(), vec!["Marina", "Rafael", "Camila"]);
        assert_eq!(t.falas.len(), 4);
        assert_eq!(t.falas[2].indice, 3);
        assert_eq!(t.falas[2].inicio, None);
        assert_eq!(
            t.falas[2].texto,
            "Decidimos manter a trava e registrar o motivo.\nQuem aprova o cancelamento após 30 dias?"
        );
        assert_eq!(
            t.falas[3].texto,
            "Aprova o gerente financeiro.\nObs: confirmar com o jurídico."
        );
    }

    #[test]
    fn com_horario_e_texto_antes_da_primeira_fala() {
        let t = analisar_texto(
            "Reunião de cancelamento\n[00:00:05] Marina: Oi.\n[00:00:09] Rafael: Oi, Marina.\n",
        );
        assert_eq!(t.formato, Formato::NomeFala);
        assert_eq!(
            t.avisos,
            vec![Aviso::TextoAntesDaPrimeiraFala { linhas: 1 }]
        );
        assert_eq!(t.falas[1].inicio.map(|h| h.segundos()), Some(9));
        assert_eq!(t.falas[1].falante.as_deref(), Some("Rafael"));
    }

    #[test]
    fn texto_do_teams_continua_com_prioridade() {
        let t = analisar_texto("Ana Souza  0:03\nNota: ok\n\nBeto  0:09\nTchau\n");
        assert_eq!(t.formato, Formato::TextoTeams);
    }
}
