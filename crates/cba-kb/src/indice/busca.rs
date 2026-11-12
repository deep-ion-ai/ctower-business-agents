//! Busca textual (FTS5) no índice.

use rusqlite::types::Value as Sql;
use serde::Serialize;

use crate::caminho::{CaminhoRelativo, sem_acento};

use super::{Indice, ResultadoIndice, Tipo};

/// Palavras ignoradas na consulta (comparadas sem acento): artigos, preposições e
/// palavras de pergunta, que aparecem em quase todo documento.
const IGNORADAS: &[&str] = &[
    "a", "o", "as", "os", "um", "uma", "uns", "umas", "de", "do", "da", "dos", "das", "e", "ou",
    "em", "no", "na", "nos", "nas", "por", "para", "pelo", "pela", "com", "sem", "que", "se", "ao",
    "aos", "como", "qual", "quais", "quando", "onde", "porque", "sao", "ser", "sobre", "entre",
];

/// Como combinar as palavras da consulta.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Modo {
    /// O documento precisa ter todas as palavras (caixa de busca da interface).
    #[default]
    Todas,
    /// Basta uma palavra; os que têm mais palavras, e mais raras, vêm antes
    /// (contexto de uma análise a partir da demanda, por exemplo).
    Qualquer,
}

/// Uma consulta ao índice.
///
/// O texto é dividido em palavras (letras e dígitos); cada palavra casa também
/// como prefixo (`multa` encontra `multas`), sem diferença de acentos nem de
/// maiúsculas. Palavras comuns (`de`, `como`, `qual`) são ignoradas, a não ser que
/// a consulta só tenha elas. A sintaxe do FTS5 não passa: aspas, `*`, `OR` e
/// parênteses no texto são só texto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Consulta {
    pub texto: String,
    pub modo: Modo,
    /// Só deste projeto (slug). `None`: todos.
    pub projeto: Option<String>,
    /// Só destes tipos. Vazio: todos.
    pub tipos: Vec<Tipo>,
    /// Resultados por página.
    pub limite: usize,
    /// Quantos resultados pular (paginação).
    pub deslocamento: usize,
}

impl Consulta {
    /// Consulta com todas as palavras, em todos os projetos e tipos, 20 por página.
    pub fn nova(texto: impl Into<String>) -> Self {
        Self {
            texto: texto.into(),
            modo: Modo::Todas,
            projeto: None,
            tipos: Vec::new(),
            limite: 20,
            deslocamento: 0,
        }
    }

    pub fn modo(mut self, modo: Modo) -> Self {
        self.modo = modo;
        self
    }

    pub fn projeto(mut self, projeto: impl Into<String>) -> Self {
        self.projeto = Some(projeto.into());
        self
    }

    pub fn tipos(mut self, tipos: impl IntoIterator<Item = Tipo>) -> Self {
        self.tipos = tipos.into_iter().collect();
        self
    }

    pub fn pagina(mut self, limite: usize, deslocamento: usize) -> Self {
        self.limite = limite;
        self.deslocamento = deslocamento;
        self
    }
}

/// Um documento encontrado.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Resultado {
    /// Caminho relativo à raiz da base (`faturamento-exemplo/regras/RN-FAT-003.md`).
    pub caminho: CaminhoRelativo,
    pub projeto: String,
    /// `None` se o frontmatter não tem um `tipo` conhecido.
    pub tipo: Option<Tipo>,
    /// `id` do frontmatter (`RN-FAT-003`, `A-0001`) ou, num termo, o termo.
    pub identificador: Option<String>,
    pub titulo: String,
    pub area: Option<String>,
    pub status: Option<String>,
    /// Trecho com as palavras encontradas entre `**`, em Markdown.
    pub trecho: String,
    /// Relevância (BM25; maior é melhor). O título pesa mais que as chaves
    /// (identificador, termo, sinônimos, área), e estas mais que o corpo.
    pub relevancia: f64,
}

/// Uma página de resultados.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Pagina {
    pub resultados: Vec<Resultado>,
    /// Total de documentos encontrados, em todas as páginas.
    pub total: usize,
}

/// Expressão FTS5 da consulta, ou `None` se não sobrar palavra.
pub(super) fn expressao(texto: &str, modo: Modo) -> Option<String> {
    let mut palavras: Vec<&str> = Vec::new();
    for p in texto.split(|c: char| !c.is_alphanumeric()) {
        if !p.is_empty() && !palavras.contains(&p) {
            palavras.push(p);
        }
    }
    let uteis: Vec<&str> = palavras.iter().copied().filter(|p| !ignorada(p)).collect();
    let escolhidas = if uteis.is_empty() { palavras } else { uteis };
    if escolhidas.is_empty() {
        return None;
    }
    let juncao = match modo {
        Modo::Todas => " AND ",
        Modo::Qualquer => " OR ",
    };
    Some(
        escolhidas
            .iter()
            .map(|p| format!("\"{p}\"*"))
            .collect::<Vec<_>>()
            .join(juncao),
    )
}

fn ignorada(palavra: &str) -> bool {
    let normal: String = palavra
        .chars()
        .flat_map(char::to_lowercase)
        .map(sem_acento)
        .collect();
    IGNORADAS.contains(&normal.as_str())
}

impl Indice {
    /// Busca no índice. Para refletir edições recentes na base, chame
    /// [`Indice::sincronizar`] antes (ou mantenha uma [`super::Vigia`]).
    pub fn buscar(&self, consulta: &Consulta) -> ResultadoIndice<Pagina> {
        let Some(expressao) = expressao(&consulta.texto, consulta.modo) else {
            return Ok(Pagina {
                resultados: Vec::new(),
                total: 0,
            });
        };
        let mut filtro = String::from("busca MATCH ?1 AND (?2 IS NULL OR d.projeto = ?2)");
        let mut parametros: Vec<Sql> = vec![
            Sql::Text(expressao),
            consulta.projeto.clone().map_or(Sql::Null, Sql::Text),
        ];
        if !consulta.tipos.is_empty() {
            let marcas: Vec<String> = consulta
                .tipos
                .iter()
                .map(|t| {
                    parametros.push(Sql::Text(t.como_str().to_owned()));
                    format!("?{}", parametros.len())
                })
                .collect();
            filtro.push_str(&format!(" AND d.tipo IN ({})", marcas.join(", ")));
        }

        let total: i64 = self.conexao.query_row(
            &format!(
                "SELECT count(*) FROM busca JOIN documento d ON d.id = busca.rowid WHERE {filtro}"
            ),
            rusqlite::params_from_iter(parametros.iter()),
            |l| l.get(0),
        )?;

        let n = parametros.len();
        parametros.push(Sql::Integer(
            i64::try_from(consulta.limite).unwrap_or(i64::MAX),
        ));
        parametros.push(Sql::Integer(
            i64::try_from(consulta.deslocamento).unwrap_or(i64::MAX),
        ));
        let sql = format!(
            "SELECT d.caminho, d.projeto, d.tipo, d.identificador, d.titulo, d.area, d.status, \
                    snippet(busca, -1, '**', '**', '…', 16), \
                    bm25(busca, 10.0, 5.0, 1.0) AS r \
             FROM busca JOIN documento d ON d.id = busca.rowid \
             WHERE {filtro} \
             ORDER BY r, d.caminho \
             LIMIT ?{} OFFSET ?{}",
            n + 1,
            n + 2
        );
        let mut comando = self.conexao.prepare(&sql)?;
        let linhas = comando.query_map(rusqlite::params_from_iter(parametros.iter()), |l| {
            Ok((
                l.get::<_, String>(0)?,
                l.get::<_, String>(1)?,
                l.get::<_, Option<String>>(2)?,
                l.get::<_, Option<String>>(3)?,
                l.get::<_, String>(4)?,
                l.get::<_, Option<String>>(5)?,
                l.get::<_, Option<String>>(6)?,
                l.get::<_, String>(7)?,
                l.get::<_, f64>(8)?,
            ))
        })?;
        let mut resultados = Vec::new();
        for linha in linhas {
            let (caminho, projeto, tipo, identificador, titulo, area, status, trecho, r) = linha?;
            let Ok(caminho) = CaminhoRelativo::novo(&caminho) else {
                continue;
            };
            resultados.push(Resultado {
                caminho,
                projeto,
                tipo: tipo.as_deref().and_then(Tipo::de_str),
                identificador,
                titulo,
                area,
                status,
                trecho: trecho.split_whitespace().collect::<Vec<_>>().join(" "),
                relevancia: -r,
            });
        }
        Ok(Pagina {
            resultados,
            total: usize::try_from(total).unwrap_or(0),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expressao_com_prefixo_e_sem_palavras_comuns() {
        assert_eq!(
            expressao("Como é calculada a multa?", Modo::Todas).as_deref(),
            Some(r#""calculada"* AND "multa"*"#)
        );
        assert_eq!(
            expressao("multa juros multa", Modo::Qualquer).as_deref(),
            Some(r#""multa"* OR "juros"*"#)
        );
    }

    #[test]
    fn expressao_nao_deixa_passar_a_sintaxe_do_fts() {
        assert_eq!(
            expressao(r#"multa" OR "x* NEAR(a)"#, Modo::Todas).as_deref(),
            Some(r#""multa"* AND "OR"* AND "x"* AND "NEAR"*"#)
        );
    }

    #[test]
    fn so_palavras_comuns_ficam() {
        assert_eq!(expressao("de", Modo::Todas).as_deref(), Some(r#""de"*"#));
        assert_eq!(expressao(" ?! ", Modo::Todas), None);
    }
}
