//! Mascaramento central (#436): todo segredo resolvido entra numa lista única, e
//! o terminal embutido, os logs e as mensagens de erro trocam o valor por `***`.
//!
//! A lista vive o processo inteiro. Ela guarda só os valores que o app já
//! resolveu, para reconhecê-los num texto; nunca é gravada nem exibida.

use crate::Secret;
use std::sync::RwLock;
use zeroize::Zeroize;

/// Valores curtos demais não entram: trocar `abc` em todo texto atrapalharia a
/// leitura sem proteger nada (um token real tem dezenas de caracteres).
pub const TAMANHO_MINIMO: usize = 6;

/// O que entra no lugar do segredo.
pub const MASCARA: &str = "***";

static SEGREDOS: RwLock<Vec<String>> = RwLock::new(Vec::new());

/// Acrescenta o valor à lista de mascaramento. Idempotente.
pub fn registrar(segredo: &Secret) {
    registrar_texto(segredo.expose());
}

/// Como [`registrar`], para um valor que ainda não é [`Secret`] (a senha numa
/// string de conexão de banco).
pub fn registrar_texto(valor: &str) {
    let v = valor.trim();
    if v.chars().count() < TAMANHO_MINIMO {
        return;
    }
    let Ok(mut lista) = SEGREDOS.write() else {
        return;
    };
    if !lista.iter().any(|s| s == v) {
        lista.push(v.to_string());
        // Os mais longos primeiro: um segredo que contém outro some inteiro.
        lista.sort_by_key(|s| std::cmp::Reverse(s.len()));
    }
}

/// Tira o valor da lista (o token foi trocado ou apagado).
pub fn esquecer(valor: &str) {
    if let Ok(mut lista) = SEGREDOS.write() {
        lista.retain_mut(|s| {
            if s == valor.trim() {
                s.zeroize();
                false
            } else {
                true
            }
        });
    }
}

/// O texto com cada segredo conhecido trocado por `***`.
pub fn mascarar(texto: &str) -> String {
    let Ok(lista) = SEGREDOS.read() else {
        return texto.to_string();
    };
    let mut saida = texto.to_string();
    for s in lista.iter() {
        if saida.contains(s.as_str()) {
            saida = saida.replace(s.as_str(), MASCARA);
        }
    }
    saida
}

/// Mascaramento de um fluxo em partes (a saída do terminal): um segredo pode
/// chegar dividido entre duas leituras. O fim de cada parte que pode ser o
/// começo de um segredo fica retido até a próxima.
#[derive(Debug, Default)]
pub struct MascaradorFluxo {
    retido: String,
}

impl MascaradorFluxo {
    /// A parte já mascarada que pode ser entregue.
    pub fn mascarar(&mut self, parte: &str) -> String {
        let mut texto = std::mem::take(&mut self.retido);
        texto.push_str(parte);
        let texto = mascarar(&texto);
        let corte = inicio_de_segredo_no_fim(&texto);
        self.retido = texto[corte..].to_string();
        texto[..corte].to_string()
    }

    /// O que restou retido, ao fim do fluxo.
    pub fn terminar(&mut self) -> String {
        mascarar(&std::mem::take(&mut self.retido))
    }
}

/// Índice do sufixo mais longo de `texto` que é prefixo próprio de algum
/// segredo; `texto.len()` quando não há.
fn inicio_de_segredo_no_fim(texto: &str) -> usize {
    let Ok(lista) = SEGREDOS.read() else {
        return texto.len();
    };
    let mut corte = texto.len();
    for s in lista.iter() {
        // Sufixos de tamanho 1..len(s)-1, do mais longo para o mais curto.
        let max = s.len().saturating_sub(1).min(texto.len());
        for n in (1..=max).rev() {
            let i = texto.len() - n;
            if i < corte && texto.is_char_boundary(i) && s.starts_with(&texto[i..]) {
                corte = i;
                break;
            }
        }
    }
    corte
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mascara_os_segredos_registrados() {
        registrar(&Secret::new("glpat-mascara-teste-1"));
        registrar_texto("senha-do-banco-teste");
        registrar_texto("abc");
        let t = mascarar("token glpat-mascara-teste-1 e senha-do-banco-teste e abc");
        assert_eq!(t, "token *** e *** e abc");
        esquecer("senha-do-banco-teste");
        assert!(mascarar("senha-do-banco-teste").contains("senha-do-banco"));
    }

    #[test]
    fn fluxo_com_segredo_dividido_entre_partes() {
        registrar(&Secret::new("ghp_fluxo_dividido_42"));
        let mut m = MascaradorFluxo::default();
        let mut saida = m.mascarar("export GH_TOKEN=ghp_flu");
        saida.push_str(&m.mascarar("xo_dividido_42\r\nok "));
        saida.push_str(&m.mascarar("ghp_"));
        saida.push_str(&m.terminar());
        assert_eq!(saida, "export GH_TOKEN=***\r\nok ghp_");
        assert!(!saida.contains("dividido"));
    }

    #[test]
    fn fluxo_sem_segredo_entrega_tudo() {
        let mut m = MascaradorFluxo::default();
        assert_eq!(m.mascarar("çãõ linha normal\n"), "çãõ linha normal\n");
        assert_eq!(m.terminar(), "");
    }
}
