//! Exportação de um documento da base para quem não usa o app (spec, seção 3 e
//! critério de aceite 9).
//!
//! A entrada é o texto do documento como está na base (com frontmatter). A saída
//! é autocontida:
//!
//! - **Markdown** ([`exportar_markdown`]): sem o frontmatter interno, com os links
//!   para outros documentos da base trocados pelo texto do link (quem recebe não
//!   tem a base), o selo de confiança e as evidências como notas de rodapé
//!   `[^n]` com `arquivo:linhas`, e, se houver de onde ler o código, um apêndice
//!   com os trechos citados.
//! - **Word** ([`exportar_word`]): o mesmo conteúdo num `.docx` com estilos
//!   simples (títulos, listas, tabelas, citações e blocos de código), as
//!   evidências como notas de rodapé do Word e o apêndice com os trechos. O
//!   `.docx` é gerado por [`crate::word`], sem dependências nativas.
//!
//! As seções são reconhecidas pelo título, porque o documento na base pode ter
//! vindo de um template editado: a seção de perguntas é a de título que começa
//! com "Perguntas", e a de evidências é a de título "Evidências".

use std::ops::Range;
use std::path::{Path, PathBuf};

use cba_kb::CaminhoRelativo;
use cba_kb::frontmatter::Linhas;
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use serde::{Deserialize, Serialize};

use crate::conteudo::{Citacao, bloco_de_codigo};
use crate::montagem::normalizar;

/// O que entra na exportação. Por padrão, tudo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct OpcoesExportacao {
    /// Notas de rodapé com as evidências e a seção "Evidências". Sem elas, as
    /// referências `[^n]` saem do texto e o apêndice de trechos também.
    pub evidencias: bool,
    /// Seção de perguntas em aberto (para o solicitante).
    pub perguntas: bool,
    /// Apêndice com os trechos de código citados nas evidências. Só sai quando
    /// há uma [`FonteDeTrechos`] e `evidencias` está ligado.
    pub trechos: bool,
}

impl Default for OpcoesExportacao {
    fn default() -> Self {
        Self {
            evidencias: true,
            perguntas: true,
            trechos: true,
        }
    }
}

/// Formato de exportação.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Formato {
    Markdown,
    Word,
}

impl Formato {
    /// Extensão do arquivo exportado, sem o ponto.
    pub fn extensao(self) -> &'static str {
        match self {
            Self::Markdown => "md",
            Self::Word => "docx",
        }
    }
}

/// De onde ler os trechos de código citados, para o apêndice.
pub trait FonteDeTrechos {
    /// Linhas `linhas` de `arquivo` (relativo à raiz do projeto), sem a quebra
    /// de linha final. `None` quando o arquivo ou as linhas não existem.
    fn trecho(&self, arquivo: &CaminhoRelativo, linhas: Linhas) -> Option<String>;
}

/// Código do projeto numa pasta local (o clone do repositório).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodigoEmPasta {
    raiz: PathBuf,
}

impl CodigoEmPasta {
    pub fn nova(raiz: impl Into<PathBuf>) -> Self {
        Self { raiz: raiz.into() }
    }

    pub fn raiz(&self) -> &Path {
        &self.raiz
    }
}

impl FonteDeTrechos for CodigoEmPasta {
    fn trecho(&self, arquivo: &CaminhoRelativo, linhas: Linhas) -> Option<String> {
        let bytes = std::fs::read(arquivo.em(&self.raiz)).ok()?;
        let texto = decodificar(bytes);
        let todas: Vec<&str> = texto.lines().collect();
        let inicio = linhas.inicio as usize;
        if inicio > todas.len() {
            return None;
        }
        let fim = (linhas.fim as usize).min(todas.len());
        Some(todas[inicio - 1..fim].join("\n"))
    }
}

/// UTF-8 ou, se não for, Latin-1 (comum em código legado: COBOL, Delphi).
fn decodificar(bytes: Vec<u8>) -> String {
    match String::from_utf8(bytes) {
        Ok(t) => t,
        Err(e) => e.into_bytes().iter().map(|&b| char::from(b)).collect(),
    }
}

/// Erro da exportação.
#[derive(Debug, thiserror::Error)]
pub enum ErroExportacao {
    #[error("falha ao gerar o .docx: {0}")]
    Docx(#[from] zip::result::ZipError),
    #[error("falha ao gravar {}: {erro}", caminho.display())]
    Gravar {
        caminho: PathBuf,
        #[source]
        erro: std::io::Error,
    },
}

/// Documento em Markdown autocontido. `documento` é o texto como está na base.
pub fn exportar_markdown(
    documento: &str,
    opcoes: &OpcoesExportacao,
    codigo: Option<&dyn FonteDeTrechos>,
) -> String {
    let mut texto = limpar(sem_frontmatter(documento), opcoes);
    if opcoes.evidencias
        && opcoes.trechos
        && let Some(codigo) = codigo
    {
        texto = com_apendice(&texto, codigo);
    }
    texto
}

/// Documento em Word (`.docx`), com o mesmo conteúdo de [`exportar_markdown`].
pub fn exportar_word(
    documento: &str,
    opcoes: &OpcoesExportacao,
    codigo: Option<&dyn FonteDeTrechos>,
) -> Result<Vec<u8>, ErroExportacao> {
    let markdown = exportar_markdown(documento, opcoes, codigo);
    Ok(crate::word::docx(&markdown)?)
}

/// Exporta no formato pedido.
pub fn exportar(
    documento: &str,
    formato: Formato,
    opcoes: &OpcoesExportacao,
    codigo: Option<&dyn FonteDeTrechos>,
) -> Result<Vec<u8>, ErroExportacao> {
    match formato {
        Formato::Markdown => Ok(exportar_markdown(documento, opcoes, codigo).into_bytes()),
        Formato::Word => exportar_word(documento, opcoes, codigo),
    }
}

/// Exporta e grava em `destino` (atômico, substituindo o arquivo se existir).
pub fn gravar_exportacao(
    destino: &Path,
    documento: &str,
    formato: Formato,
    opcoes: &OpcoesExportacao,
    codigo: Option<&dyn FonteDeTrechos>,
) -> Result<(), ErroExportacao> {
    let bytes = exportar(documento, formato, opcoes, codigo)?;
    cba_kb::arquivo::gravar(destino, &bytes).map_err(|erro| ErroExportacao::Gravar {
        caminho: destino.to_owned(),
        erro,
    })
}

/// Opções do Markdown que os documentos usam (e o que um template editado pode
/// usar).
pub(crate) fn opcoes_markdown() -> Options {
    Options::ENABLE_FOOTNOTES
        | Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
}

/// Corpo sem o frontmatter `---` do início, se houver.
fn sem_frontmatter(texto: &str) -> &str {
    let texto = texto.strip_prefix('\u{feff}').unwrap_or(texto);
    let Some(resto) = texto
        .strip_prefix("---\n")
        .or_else(|| texto.strip_prefix("---\r\n"))
    else {
        return texto;
    };
    let mut pos = 0;
    for linha in resto.split_inclusive('\n') {
        pos += linha.len();
        if linha.trim_end() == "---" {
            return &resto[pos..];
        }
    }
    texto
}

/// Se o destino de um link aponta para dentro da base (caminho relativo), e não
/// para um endereço externo ou uma âncora do próprio documento.
fn e_link_da_base(destino: &str) -> bool {
    !(destino.is_empty()
        || destino.starts_with('#')
        || destino.contains("://")
        || destino.starts_with("mailto:"))
}

/// Tira o que só faz sentido dentro da base e o que as opções excluem.
fn limpar(corpo: &str, opcoes: &OpcoesExportacao) -> String {
    let eventos: Vec<(Event, Range<usize>)> = Parser::new_ext(corpo, opcoes_markdown())
        .into_offset_iter()
        .collect();
    let mut edicoes: Vec<(Range<usize>, String)> = Vec::new();

    // Seções removidas pelas opções.
    let titulos = titulos(&eventos);
    for (i, t) in titulos.iter().enumerate() {
        let remover = (!opcoes.perguntas && t.texto.to_lowercase().starts_with("perguntas"))
            || (!opcoes.evidencias && t.texto.trim() == "Evidências");
        if remover {
            let fim = titulos[i + 1..]
                .iter()
                .find(|o| o.nivel <= t.nivel)
                .map_or(corpo.len(), |o| o.inicio);
            edicoes.push((t.inicio..fim, String::new()));
        }
    }

    // Links para a base e notas. O texto do link vai do fim do `[` (ou `![`)
    // ao fim do último evento dentro do link, com os escapes do Markdown.
    let mut link: Option<(usize, usize, Option<usize>)> = None;
    for (evento, r) in &eventos {
        match evento {
            Event::Start(Tag::Link { dest_url, .. } | Tag::Image { dest_url, .. })
                if e_link_da_base(dest_url) =>
            {
                let abre = if corpo[r.start..].starts_with('!') {
                    2
                } else {
                    1
                };
                link = Some((r.start, r.start + abre, None));
            }
            Event::End(TagEnd::Link | TagEnd::Image) => {
                if let Some((inicio, texto, fim)) = link.take() {
                    let texto = fim.map_or("", |fim| &corpo[texto..fim]);
                    edicoes.push((inicio..r.end, texto.to_owned()));
                }
            }
            Event::FootnoteReference(_) if !opcoes.evidencias => {
                edicoes.push((r.clone(), String::new()));
            }
            Event::Start(Tag::FootnoteDefinition(_)) if !opcoes.evidencias => {
                edicoes.push((r.clone(), String::new()));
            }
            _ => {
                if let Some((_, _, fim)) = &mut link {
                    *fim = Some(fim.unwrap_or(0).max(r.end));
                }
            }
        }
    }

    normalizar(aplicar(corpo, edicoes).trim_start_matches(['\n', '\r']))
}

struct Titulo {
    nivel: HeadingLevel,
    inicio: usize,
    texto: String,
}

fn titulos(eventos: &[(Event, Range<usize>)]) -> Vec<Titulo> {
    let mut lista = Vec::new();
    let mut atual: Option<Titulo> = None;
    for (evento, r) in eventos {
        match evento {
            Event::Start(Tag::Heading { level, .. }) => {
                atual = Some(Titulo {
                    nivel: *level,
                    inicio: r.start,
                    texto: String::new(),
                });
            }
            Event::End(TagEnd::Heading(_)) => lista.extend(atual.take()),
            Event::Text(t) | Event::Code(t) => {
                if let Some(a) = &mut atual {
                    a.texto.push_str(t);
                }
            }
            _ => {}
        }
    }
    lista
}

/// Aplica as substituições; uma edição dentro de outra maior é descartada.
fn aplicar(texto: &str, mut edicoes: Vec<(Range<usize>, String)>) -> String {
    edicoes.sort_by(|a, b| a.0.start.cmp(&b.0.start).then(b.0.end.cmp(&a.0.end)));
    let mut saida = String::with_capacity(texto.len());
    let mut pos = 0;
    for (r, novo) in edicoes {
        if r.start < pos {
            continue;
        }
        saida.push_str(&texto[pos..r.start]);
        saida.push_str(&novo);
        pos = r.end;
    }
    saida.push_str(&texto[pos..]);
    saida
}

/// Nota de rodapé do documento: rótulo e o trecho citado, se a definição começa
/// com `arquivo:linhas` em código.
pub(crate) struct Nota {
    pub rotulo: String,
    pub citacao: Option<Citacao>,
}

/// Notas de rodapé na ordem em que são definidas.
pub(crate) fn notas(texto: &str) -> Vec<Nota> {
    let mut lista: Vec<Nota> = Vec::new();
    let mut dentro = false;
    for evento in Parser::new_ext(texto, opcoes_markdown()) {
        match evento {
            Event::Start(Tag::FootnoteDefinition(rotulo)) => {
                dentro = true;
                lista.push(Nota {
                    rotulo: rotulo.to_string(),
                    citacao: None,
                });
            }
            Event::End(TagEnd::FootnoteDefinition) => dentro = false,
            Event::Code(c) if dentro => {
                let nota = lista.last_mut().expect("dentro de uma definição");
                if nota.citacao.is_none() {
                    nota.citacao = citacao(&c);
                }
            }
            _ => {}
        }
    }
    lista
}

/// `arquivo:10-12` ou `arquivo:10`.
fn citacao(texto: &str) -> Option<Citacao> {
    let (arquivo, linhas) = texto.rsplit_once(':')?;
    let (a, b) = linhas.split_once('-').unwrap_or((linhas, linhas));
    let linhas = Linhas::novo(a.trim().parse().ok()?, b.trim().parse().ok()?)?;
    Some(Citacao::nova(CaminhoRelativo::novo(arquivo).ok()?, linhas))
}

/// Título do apêndice com os trechos citados.
pub(crate) const TITULO_APENDICE: &str = "Apêndice: trechos citados";

/// Acrescenta o apêndice com o código de cada nota que cita um trecho.
fn com_apendice(texto: &str, codigo: &dyn FonteDeTrechos) -> String {
    let notas: Vec<(String, Citacao)> = notas(texto)
        .into_iter()
        .filter_map(|n| Some((n.rotulo, n.citacao?)))
        .collect();
    if notas.is_empty() {
        return texto.to_owned();
    }
    let mut saida = texto.trim_end().to_owned();
    saida.push_str(&format!("\n\n## {TITULO_APENDICE}\n"));
    for (rotulo, c) in notas {
        saida.push_str(&format!("\n### [{rotulo}] `{}`\n\n", c.local()));
        match codigo.trecho(&c.arquivo, c.linhas) {
            Some(trecho) => {
                let largura = c.linhas.fim.to_string().len();
                let numerado: Vec<String> = trecho
                    .lines()
                    .zip(c.linhas.inicio..)
                    .map(|(l, n)| format!("{n:>largura$} | {l}").trim_end().to_owned())
                    .collect();
                saida.push_str(&bloco_de_codigo(
                    &numerado.join("\n"),
                    linguagem(c.arquivo.as_str()),
                ));
                saida.push('\n');
            }
            None => saida.push_str("_Trecho indisponível no código consultado._\n"),
        }
    }
    saida
}

/// Linguagem do bloco de código pela extensão do arquivo.
fn linguagem(arquivo: &str) -> &'static str {
    let extensao = arquivo
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    match extensao.as_str() {
        "java" => "java",
        "ts" | "tsx" => "typescript",
        "js" | "jsx" => "javascript",
        "sql" | "pks" | "pkb" | "pck" => "sql",
        "cbl" | "cob" | "cpy" => "cobol",
        "pas" | "dpr" | "dfm" => "pascal",
        "py" => "python",
        "cs" => "csharp",
        "xml" => "xml",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tira_frontmatter() {
        assert_eq!(sem_frontmatter("---\na: 1\n---\n\n# T\n"), "\n# T\n");
        assert_eq!(sem_frontmatter("# T\n"), "# T\n");
        assert_eq!(sem_frontmatter("---\nsem fim\n"), "---\nsem fim\n");
    }

    #[test]
    fn links_da_base_viram_texto() {
        let md = "Ver [A-0001](../analises/A-0001.md), [site](https://exemplo.com) e [\\[x\\]](<a b.md>).\n";
        assert_eq!(
            limpar(md, &OpcoesExportacao::default()),
            "Ver A-0001, [site](https://exemplo.com) e \\[x\\].\n"
        );
    }

    #[test]
    fn opcoes_removem_secoes_e_notas() {
        let md = "# T\n\n## A\n\nTexto.[^1]\n\n## Perguntas em aberto\n\n1. Q?\n\n## Evidências\n\n[^1]: `a.sql:1`\n";
        let opcoes = OpcoesExportacao {
            evidencias: false,
            perguntas: false,
            trechos: true,
        };
        assert_eq!(limpar(md, &opcoes), "# T\n\n## A\n\nTexto.\n");
    }

    #[test]
    fn citacao_da_nota() {
        let c = citacao("db/02_parametros.sql:15-16").unwrap();
        assert_eq!(c.local(), "db/02_parametros.sql:15-16");
        assert_eq!(citacao("a.sql:7").unwrap().local(), "a.sql:7");
        assert!(citacao("sem linhas").is_none());
        assert!(citacao("a.sql:0").is_none());
    }

    #[test]
    fn latin1_quando_nao_e_utf8() {
        assert_eq!(decodificar(vec![b'a', 0xe7, 0xe3]), "açã");
    }
}
