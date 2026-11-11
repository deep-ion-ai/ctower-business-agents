//! Paginação das ferramentas de consulta (`conhecimento`, `mapa`, `estrutura`,
//! `schema` e `referencias`).
//!
//! As respostas são curtas para não gastar o limite do plano do analista: cada
//! página traz no máximo `limite` itens (padrão [`LIMITE_PADRAO`], até
//! [`LIMITE_MAXIMO`]) e no máximo [`LIMITE_DE_TEXTO`] caracteres de itens no texto;
//! o que passar de um dos dois fica para a próxima página. O cursor é a posição do
//! primeiro item (começa em 0), e a resposta diz o total e o cursor da próxima
//! página, quando houver.

use serde::Deserialize;
use serde_json::{Value, json};

/// Itens por página quando o agente não informa `limite`.
pub(crate) const LIMITE_PADRAO: usize = 20;

/// Maior `limite` aceito.
pub(crate) const LIMITE_MAXIMO: usize = 50;

/// Caracteres de itens no texto de uma página (fora o cabeçalho e o rodapé).
pub(crate) const LIMITE_DE_TEXTO: usize = 6_000;

/// Tamanho máximo de um texto livre num item (comentário, trecho), em caracteres.
pub(crate) const LIMITE_DE_CAMPO: usize = 160;

/// Argumentos de paginação, comuns às ferramentas de consulta.
#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub(crate) struct ArgsPagina {
    #[serde(default)]
    pub cursor: Option<usize>,
    #[serde(default)]
    pub limite: Option<usize>,
}

impl ArgsPagina {
    pub fn inicio(&self) -> usize {
        self.cursor.unwrap_or(0)
    }

    pub fn limite(&self) -> usize {
        self.limite.unwrap_or(LIMITE_PADRAO).clamp(1, LIMITE_MAXIMO)
    }
}

/// Acrescenta `cursor` e `limite` às propriedades do schema de uma ferramenta.
pub(crate) fn com_paginacao(mut schema: Value) -> Value {
    let propriedades = schema["properties"]
        .as_object_mut()
        .expect("schema de ferramenta com properties");
    propriedades.insert(
        "cursor".into(),
        json!({
            "type": "integer",
            "minimum": 0,
            "description": "Posição do primeiro item (começa em 0). Para a próxima página, use o `proximo_cursor` da resposta anterior."
        }),
    );
    propriedades.insert(
        "limite".into(),
        json!({
            "type": "integer",
            "minimum": 1,
            "maximum": LIMITE_MAXIMO,
            "description": format!("Itens por página (padrão {LIMITE_PADRAO}). A página também é cortada por tamanho.")
        }),
    );
    schema
}

/// Um item de uma resposta: a linha do texto para o agente e os dados.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Item {
    pub linha: String,
    pub dados: Value,
}

/// Uma página de itens.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Pagina {
    pub itens: Vec<Item>,
    /// Total de itens, em todas as páginas.
    pub total: usize,
    /// Posição do primeiro item desta página.
    pub inicio: usize,
}

impl Pagina {
    /// Página a partir de uma janela que já começa em `inicio` (a busca no índice
    /// devolve só a janela): até `limite` itens e [`LIMITE_DE_TEXTO`] caracteres,
    /// com ao menos um item quando houver.
    pub fn da_janela(
        janela: impl IntoIterator<Item = Item>,
        inicio: usize,
        limite: usize,
        total: usize,
    ) -> Self {
        let mut itens = Vec::new();
        let mut tamanho = 0;
        for item in janela.into_iter().take(limite) {
            let n = item.linha.chars().count() + 1;
            if !itens.is_empty() && tamanho + n > LIMITE_DE_TEXTO {
                break;
            }
            tamanho += n;
            itens.push(item);
        }
        Self {
            itens,
            total,
            inicio,
        }
    }

    /// Página de uma lista completa.
    pub fn da_lista(todos: Vec<Item>, args: ArgsPagina) -> Self {
        let total = todos.len();
        let inicio = args.inicio();
        Self::da_janela(todos.into_iter().skip(inicio), inicio, args.limite(), total)
    }

    /// Cursor da próxima página, se houver.
    pub fn proximo(&self) -> Option<usize> {
        let fim = self.inicio + self.itens.len();
        (!self.itens.is_empty() && fim < self.total).then_some(fim)
    }

    /// Dados da paginação, para a resposta estruturada.
    pub fn json(&self) -> Value {
        json!({
            "total": self.total,
            "cursor": self.inicio,
            "quantidade": self.itens.len(),
            "proximo_cursor": self.proximo(),
        })
    }

    /// Os dados dos itens.
    pub fn dados(&self) -> Vec<Value> {
        self.itens.iter().map(|i| i.dados.clone()).collect()
    }

    /// As linhas dos itens, uma por linha.
    pub fn linhas(&self) -> String {
        let mut t = String::new();
        for item in &self.itens {
            t.push_str(&item.linha);
            t.push('\n');
        }
        t
    }

    /// Rodapé com a posição e como pedir a próxima página.
    pub fn rodape(&self) -> String {
        if self.itens.is_empty() {
            return if self.total > 0 {
                format!(
                    "O cursor {} passou do fim: há {} itens (cursor de 0 a {}).",
                    self.inicio,
                    self.total,
                    self.total - 1
                )
            } else {
                String::new()
            };
        }
        let mut t = format!(
            "Itens {} a {} de {}.",
            self.inicio + 1,
            self.inicio + self.itens.len(),
            self.total
        );
        if let Some(p) = self.proximo() {
            t.push_str(&format!(
                " Para ver os próximos, chame de novo com cursor: {p}."
            ));
        }
        t
    }
}

/// Texto livre cortado em [`LIMITE_DE_CAMPO`] caracteres, numa linha só.
pub(crate) fn curto(texto: &str) -> String {
    let linha = texto.split_whitespace().collect::<Vec<_>>().join(" ");
    if linha.chars().count() <= LIMITE_DE_CAMPO {
        return linha;
    }
    let corte: String = linha.chars().take(LIMITE_DE_CAMPO - 1).collect();
    format!("{}…", corte.trim_end())
}

/// Nome de um valor serializável como texto (`funcao`, `chave_estrangeira`).
pub(crate) fn nome<T: serde::Serialize>(valor: &T) -> String {
    match serde_json::to_value(valor) {
        Ok(Value::String(s)) => s,
        Ok(outro) => outro.to_string(),
        Err(_) => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn itens(n: usize, tamanho: usize) -> Vec<Item> {
        (0..n)
            .map(|i| Item {
                linha: "x".repeat(tamanho),
                dados: json!(i),
            })
            .collect()
    }

    #[test]
    fn corta_por_quantidade_e_aponta_a_proxima() {
        let p = Pagina::da_lista(itens(45, 10), ArgsPagina::default());
        assert_eq!(p.itens.len(), LIMITE_PADRAO);
        assert_eq!(p.proximo(), Some(20));
        assert_eq!(p.json()["total"], 45);
        assert!(p.rodape().contains("cursor: 20"), "{}", p.rodape());

        let ultima = Pagina::da_lista(
            itens(45, 10),
            ArgsPagina {
                cursor: Some(40),
                limite: Some(50),
            },
        );
        assert_eq!(ultima.itens.len(), 5);
        assert_eq!(ultima.itens[0].dados, json!(40));
        assert_eq!(ultima.proximo(), None);
        assert_eq!(ultima.json()["proximo_cursor"], Value::Null);
        assert_eq!(ultima.rodape(), "Itens 41 a 45 de 45.");
    }

    #[test]
    fn corta_por_tamanho_mas_traz_ao_menos_um() {
        let p = Pagina::da_lista(
            itens(10, 1_000),
            ArgsPagina {
                cursor: None,
                limite: Some(50),
            },
        );
        assert_eq!(p.itens.len(), 5);
        assert!(p.linhas().chars().count() <= LIMITE_DE_TEXTO);
        assert_eq!(p.proximo(), Some(5));

        let enorme = Pagina::da_lista(itens(2, 10_000), ArgsPagina::default());
        assert_eq!(enorme.itens.len(), 1);
        assert_eq!(enorme.proximo(), Some(1));
    }

    #[test]
    fn limite_fora_da_faixa_e_cursor_alem_do_fim() {
        assert_eq!(
            ArgsPagina {
                cursor: None,
                limite: Some(500)
            }
            .limite(),
            LIMITE_MAXIMO
        );
        let p = Pagina::da_lista(
            itens(3, 10),
            ArgsPagina {
                cursor: Some(7),
                limite: None,
            },
        );
        assert!(p.itens.is_empty());
        assert_eq!(p.proximo(), None);
        assert!(p.rodape().contains("passou do fim"), "{}", p.rodape());
    }

    #[test]
    fn curto_corta_e_junta_espacos() {
        assert_eq!(curto("a\n  b"), "a b");
        let longo = curto(&"palavra ".repeat(100));
        assert!(longo.ends_with('…'));
        assert!(longo.chars().count() <= LIMITE_DE_CAMPO);
    }
}
