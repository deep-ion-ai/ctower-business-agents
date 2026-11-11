//! Contexto da base para o roteiro de `iniciar_analise`: regras, termos do
//! glossário, memórias de área e análises anteriores relacionados à entrada
//! (spec, seções 6.1 e 8).
//!
//! Sem o índice (#8) e a ferramenta `conhecimento` (#22), a relação é feita por
//! palavras em comum entre o título e a entrada da análise e o título da regra, o
//! termo e seus sinônimos, a área ou o título da análise anterior. Documentos
//! ilegíveis são ignorados. A lista é curta de propósito: o agente lê o documento
//! completo na base quando precisar.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use cba_core::ambiente::Ambiente;
use cba_core::analise::Analise;
use cba_kb::frontmatter::{FrontmatterRegra, FrontmatterTermo, StatusRegra};
use cba_kb::{CaminhoRelativo, layout};
use serde_json::{Value, json};

/// Itens no máximo por tipo.
const LIMITE: usize = 10;

/// Palavras curtas e comuns demais para relacionar documentos.
const IGNORADAS: [&str; 16] = [
    "como", "qual", "quais", "quando", "onde", "para", "pela", "pelo", "sobre", "entre", "deve",
    "pode", "esta", "este", "isso", "sistema",
];

/// Palavras de um texto, em minúsculas e sem acentos, com 4 letras ou mais.
fn palavras(texto: &str) -> BTreeSet<String> {
    texto
        .chars()
        .map(sem_acento)
        .collect::<String>()
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|p| p.chars().count() >= 4 && !IGNORADAS.contains(p))
        .map(str::to_owned)
        .collect()
}

fn sem_acento(c: char) -> char {
    match c {
        'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
        'Á' | 'À' | 'Â' | 'Ã' | 'Ä' => 'A',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'É' | 'È' | 'Ê' | 'Ë' => 'E',
        'í' | 'ì' | 'î' | 'ï' => 'i',
        'Í' | 'Ì' | 'Î' | 'Ï' => 'I',
        'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
        'Ó' | 'Ò' | 'Ô' | 'Õ' | 'Ö' => 'O',
        'ú' | 'ù' | 'û' | 'ü' => 'u',
        'Ú' | 'Ù' | 'Û' | 'Ü' => 'U',
        'ç' => 'c',
        'Ç' => 'C',
        outro => outro,
    }
}

/// Quantas palavras de `chave` aparecem em `texto`. Uma palavra que começa com a
/// outra conta (multa, multas), para plural e flexões simples.
fn pontos(chave: &BTreeSet<String>, texto: &str) -> usize {
    let alvo = palavras(texto);
    chave
        .iter()
        .filter(|p| {
            alvo.iter()
                .any(|a| a.starts_with(p.as_str()) || p.starts_with(a.as_str()))
        })
        .count()
}

/// Os `LIMITE` itens de maior pontuação (maior que zero), na ordem original em
/// caso de empate.
fn melhores<T>(mut itens: Vec<(usize, T)>) -> Vec<T> {
    itens.retain(|(p, _)| *p > 0);
    itens.sort_by_key(|a| std::cmp::Reverse(a.0));
    itens.into_iter().take(LIMITE).map(|(_, t)| t).collect()
}

/// Contexto da base relacionado a uma análise.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Contexto {
    pub regras: Vec<Value>,
    pub termos: Vec<Value>,
    pub memorias: Vec<Value>,
    pub analises: Vec<Value>,
    /// Totais na base, para o agente saber o tamanho do que não veio.
    pub total_de_regras: usize,
    pub total_de_termos: usize,
}

impl Contexto {
    /// Monta o contexto da análise com o que há na base do sistema.
    pub fn da_analise(ambiente: &Ambiente, analise: &Analise) -> Self {
        let kb = &ambiente.kb;
        let chave = palavras(&format!("{} {}", analise.titulo, analise.entrada));
        let documentos = kb.documentos().unwrap_or_default();
        let na_pasta = |pasta: &'static str| -> Vec<CaminhoRelativo> {
            documentos
                .iter()
                .filter(|d| d.partes().next() == Some(pasta))
                .cloned()
                .collect()
        };
        let mut contexto = Contexto::default();
        let mut areas = BTreeSet::new();

        let mut regras = Vec::new();
        for caminho in na_pasta(layout::REGRAS) {
            let Ok(doc) = kb.ler::<FrontmatterRegra>(&caminho) else {
                continue;
            };
            let f = doc.frontmatter;
            if f.status == StatusRegra::Obsoleta {
                continue;
            }
            contexto.total_de_regras += 1;
            let p = pontos(&chave, &format!("{} {}", f.titulo, f.area));
            regras.push((p, (f, kb.na_base(&caminho))));
        }
        for (f, arquivo) in melhores(regras) {
            areas.insert(f.area.clone());
            contexto.regras.push(json!({
                "id": f.id,
                "titulo": f.titulo,
                "area": f.area,
                "status": f.status,
                "arquivo": arquivo,
            }));
        }

        let mut termos = Vec::new();
        for caminho in na_pasta(layout::GLOSSARIO) {
            let Ok(doc) = kb.ler::<FrontmatterTermo>(&caminho) else {
                continue;
            };
            let f = doc.frontmatter;
            contexto.total_de_termos += 1;
            let p = pontos(&chave, &format!("{} {}", f.termo, f.sinonimos.join(" ")));
            termos.push((p, (f, kb.na_base(&caminho))));
        }
        for (f, arquivo) in melhores(termos) {
            if let Some(area) = &f.area {
                areas.insert(area.clone());
            }
            contexto.termos.push(json!({
                "termo": f.termo,
                "sinonimos": f.sinonimos,
                "arquivo": arquivo,
            }));
        }

        // Memória da área: das áreas das regras e termos relacionados, ou cujo nome
        // aparece na entrada.
        for caminho in na_pasta(layout::AREAS) {
            let partes: Vec<&str> = caminho.partes().collect();
            let [_, area, arquivo] = partes[..] else {
                continue;
            };
            if arquivo != layout::ARQUIVO_MEMORIA {
                continue;
            }
            if areas.contains(area) || pontos(&chave, &area.replace('-', " ")) > 0 {
                contexto.memorias.push(json!({
                    "area": area,
                    "arquivo": kb.na_base(&caminho),
                }));
            }
        }
        contexto.memorias.truncate(LIMITE);

        let anteriores: Vec<_> = ambiente
            .analises
            .listar()
            .unwrap_or_default()
            .into_iter()
            .filter(|a| a.id != analise.id)
            .map(|a| (pontos(&chave, &format!("{} {}", a.titulo, a.entrada)), a))
            .collect();
        contexto.analises = melhores(anteriores)
            .into_iter()
            .map(|a| {
                json!({
                    "id": a.id,
                    "titulo": a.titulo,
                    "fluxo": a.fluxo,
                    "estado": a.estado,
                    "documento": kb.na_base(&a.documento),
                })
            })
            .collect();
        contexto
    }

    pub fn json(&self) -> Value {
        json!({
            "regras": self.regras,
            "termos": self.termos,
            "memorias": self.memorias,
            "analises": self.analises,
            "total_de_regras": self.total_de_regras,
            "total_de_termos": self.total_de_termos,
        })
    }

    /// O contexto em Markdown, para o roteiro. Caminhos relativos à base.
    pub fn markdown(&self) -> String {
        let mut t = String::new();
        let texto = |v: &Value, campo: &str| v[campo].as_str().unwrap_or_default().to_owned();
        if self.regras.is_empty()
            && self.termos.is_empty()
            && self.memorias.is_empty()
            && self.analises.is_empty()
        {
            let _ = writeln!(
                t,
                "Nada relacionado na base ({} regras e {} termos no total). Trabalhe a \
                 partir do código.",
                self.total_de_regras, self.total_de_termos
            );
            return t;
        }
        if !self.regras.is_empty() {
            let _ = writeln!(t, "Regras relacionadas ({} na base):", self.total_de_regras);
            for r in &self.regras {
                let _ = writeln!(
                    t,
                    "- {} ({}): {} — `{}`",
                    texto(r, "id"),
                    texto(r, "status"),
                    texto(r, "titulo"),
                    texto(r, "arquivo")
                );
            }
        }
        if !self.termos.is_empty() {
            let _ = writeln!(t, "Termos do glossário ({} na base):", self.total_de_termos);
            for r in &self.termos {
                let _ = writeln!(t, "- {} — `{}`", texto(r, "termo"), texto(r, "arquivo"));
            }
        }
        if !self.memorias.is_empty() {
            let _ = writeln!(t, "Memória das áreas:");
            for r in &self.memorias {
                let _ = writeln!(t, "- {} — `{}`", texto(r, "area"), texto(r, "arquivo"));
            }
        }
        if !self.analises.is_empty() {
            let _ = writeln!(t, "Análises anteriores sobre o assunto:");
            for r in &self.analises {
                let _ = writeln!(
                    t,
                    "- {} ({}): {} — `{}`",
                    texto(r, "id"),
                    texto(r, "estado"),
                    texto(r, "titulo"),
                    texto(r, "documento")
                );
            }
        }
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palavras_sem_acento_e_sem_as_comuns() {
        let p = palavras("Como é calculada a MULTA por atraso? Ação!");
        assert_eq!(
            p.into_iter().collect::<Vec<_>>(),
            ["acao", "atraso", "calculada", "multa"]
        );
    }

    #[test]
    fn pontos_aceitam_plural() {
        let chave = palavras("multa por atraso");
        assert_eq!(pontos(&chave, "Multas de atrasos"), 2);
        assert_eq!(pontos(&chave, "Juros de mora"), 0);
    }

    #[test]
    fn melhores_descarta_zero_e_ordena() {
        let v = melhores(vec![(0, "a"), (2, "b"), (1, "c"), (2, "d")]);
        assert_eq!(v, ["b", "d", "c"]);
    }
}
