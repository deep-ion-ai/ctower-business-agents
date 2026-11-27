//! Documentos das análises.
//!
//! Templates por tipo de documento e montagem dos documentos (spec, seção 7.3).
//! Exportação para Markdown, Word e HTML: [`exportar_markdown`],
//! [`exportar_word`], [`exportar_html`] (com o template e os tokens CSS de
//! [`ModeloHtml`]) e [`gravar_exportacao`] (módulo `exportacao`). Planilha do
//! Excel: [`xlsx`]. Dados pessoais do texto da demanda, mascarados na saída
//! (#454): [`mascarar_demanda`].
//!
//! Documentação legível para a forja (#288): [`gerar_documentacao`] monta as
//! páginas (README, funcionalidades, regras por área, glossário e análises) e
//! [`marcador`] define os marcadores de ida e volta de cada documento.
//!
//! O agente registra dados estruturados pelas ferramentas MCP, e o app monta cada
//! documento a partir do template do tipo:
//!
//! 1. Quem monta converte os registros num [`Conteudo`]: o frontmatter tipado de
//!    `cba-kb`, o caminho do documento e os textos, afirmações (com confiança e
//!    evidências), perguntas e sugestões, cada um ligado a uma seção pelo nome do
//!    marcador.
//! 2. [`Templates::da_base`] carrega os templates (o de `<base>/_templates/` no
//!    lugar do padrão embutido) e avisa de marcadores desconhecidos.
//! 3. [`Templates::montar`] devolve o [`cba_kb::Documento`] e os avisos;
//!    [`Templates::gravar`] monta e grava, a cada registro novo.
//!
//! Seções e campos de cada tipo: [`MODELOS`].
//!
//! Diagramas (#118): o agente registra nós e arestas com evidências e a montagem
//! gera o bloco ` ```mermaid ` ([`mermaid`]), com legenda e tabela de elementos.
//! Na exportação para Word, o bloco vira a figura renderizada pelo app
//! ([`Figuras`]) ou, sem ela, um aviso e o código.
//!
//! A ata de reunião segue o mesmo caminho a partir de um estado próprio
//! ([`EstadoAta`], em `R-NNNN.estado.json`), convertido num [`Conteudo`] e
//! montado pelo template `ata` ([`Templates::gravar_ata`]).
//!
//! Papel na arquitetura: seção 5 de `docs/specs/2026-11-11-mvp-standalone.md`.

mod ata;
mod conteudo;
mod demanda;
mod diagrama;
mod documentacao;
mod exportacao;
mod figura;
mod html;
mod modelo;
mod modelo_html;
mod montagem;
mod planilha;
mod template;
mod word;

/// Marcadores de ida e volta (#288). Vivem em `cba-kb`, que recorta as páginas
/// publicadas na importação (#290); reexportados aqui para o renderizador.
pub use cba_kb::marcador;

use cba_kb::CaminhoRelativo;

pub use ata::{
    CitacaoFala, ErroAta, EstadoAta, IdItemAta, ItemAta, NovoItem, Participante, PropostaDePapel,
    StatusCitacao, TipoItem,
};
pub use conteudo::{
    Afirmacao, Citacao, Confianca, Conteudo, Pergunta, ReferenciaSugestao, Texto, bloco_de_codigo,
};
pub use demanda::{SECOES_DA_DEMANDA, mascarar_demanda};
pub use diagrama::{
    ArestaDiagrama, AtributoDiagrama, Diagrama, EntidadeDiagrama, LINGUAGEM_MERMAID,
    Multiplicidade, NoDiagrama, OrigemRelacao, PapelNo, RelacaoDiagrama, TipoDiagrama, mermaid,
};
pub use documentacao::{
    ContextoDaDocumentacao, Documentacao, DocumentoDaBase, GLOSSARIO, INDICE_ANALISES, Pagina,
    README, gerar_documentacao,
};
pub use exportacao::{
    CodigoEmPasta, ErroExportacao, FonteDeTrechos, Formato, HtmlExportado, LinksDeEvidencia,
    OpcoesExportacao, exportar, exportar_com_links, exportar_html, exportar_markdown,
    exportar_markdown_com_links, exportar_word, gravar_exportacao, gravar_exportacao_com_links,
};
pub use figura::{
    ErroFigura, Figura, Figuras, MAXIMO_DE_FIGURAS, TAMANHO_MAXIMO_DA_FIGURA, blocos_mermaid,
    imagens_da_base,
};
pub use modelo::{
    CENARIOS, DADOS_PESSOAIS_E_RNF, FLUXO_PROCESSO, FUNCIONALIDADES_MAPEADAS, HISTORIAS,
    INDICE_CATALOGO, MODELO_DADOS, MODELOS, Modelo, Montavel, NAO_MAPEADOS, PASSO_A_PASSO,
    RASTREABILIDADE, Secao, TABELAS_DECISAO, TELAS, VALORES_LIMITE, modelo,
};
pub use modelo_html::{
    ArquivoHtml, AvisoHtml, FOLHA_PADRAO, ModeloHtml, PASTA_EXPORTACAO, VARIAVEIS_HTML,
    previa_html, rotulo_do_tipo, validar_template_html,
};
pub use montagem::{ErroMontagem, Montado};
pub use planilha::{Aba, xlsx};
pub use template::{Aviso, Origem, Template, Templates, arquivo_do_template};

/// Nome da crate, para diagnóstico.
pub const NOME: &str = env!("CARGO_PKG_NAME");

/// Link Markdown de um documento da base para outro, com caminho relativo ao
/// documento de origem (`de`). Os dois caminhos são relativos à raiz do projeto.
pub fn link(texto: &str, de: &CaminhoRelativo, para: &CaminhoRelativo) -> String {
    let texto = texto.replace('[', "\\[").replace(']', "\\]");
    let destino = caminho_relativo(de, para);
    if destino.contains([' ', '(', ')', '<', '>']) {
        format!("[{texto}](<{destino}>)")
    } else {
        format!("[{texto}]({destino})")
    }
}

/// Caminho de `para` visto da pasta de `de`.
fn caminho_relativo(de: &CaminhoRelativo, para: &CaminhoRelativo) -> String {
    let pasta: Vec<&str> = {
        let mut p: Vec<&str> = de.partes().collect();
        p.pop();
        p
    };
    let alvo: Vec<&str> = para.partes().collect();
    let comum = pasta
        .iter()
        .zip(&alvo[..alvo.len() - 1])
        .take_while(|(a, b)| a == b)
        .count();
    let mut partes: Vec<&str> = vec![".."; pasta.len() - comum];
    partes.extend(&alvo[comum..]);
    partes.join("/")
}

/// Caminho, relativo à raiz do projeto, para onde aponta o destino de um link ou
/// imagem do documento `de` (`../regras/RN-FAT-001.md`, sem a âncora). `None` se
/// o destino não é da base (endereço, âncora) ou sai da raiz.
pub fn resolver(de: &CaminhoRelativo, destino: &str) -> Option<CaminhoRelativo> {
    let destino = destino.trim();
    let destino = destino
        .strip_prefix('<')
        .and_then(|d| d.strip_suffix('>'))
        .unwrap_or(destino);
    let destino = destino.split('#').next().unwrap_or_default();
    if !exportacao::e_link_da_base(destino) || destino.starts_with('/') {
        return None;
    }
    let mut partes: Vec<&str> = de.partes().collect();
    partes.pop();
    for parte in destino.split('/') {
        match parte {
            "" | "." => {}
            ".." => {
                partes.pop()?;
            }
            p => partes.push(p),
        }
    }
    CaminhoRelativo::novo(&partes.join("/")).ok()
}

/// O corpo de um documento gravado em `de` como fica gravado em `para`: os
/// destinos relativos dos links e das imagens que apontam para a base são
/// refeitos a partir da nova pasta (a âncora fica). Endereços externos, âncoras
/// do próprio documento e destinos que saem da raiz ficam como estão.
pub fn relocalizar(corpo: &str, de: &CaminhoRelativo, para: &CaminhoRelativo) -> String {
    use pulldown_cmark::{Event, Parser, Tag};
    let mut edicoes: Vec<(std::ops::Range<usize>, String)> = Vec::new();
    for (evento, r) in Parser::new_ext(corpo, exportacao::opcoes_markdown()).into_offset_iter() {
        let (Event::Start(Tag::Link { dest_url, .. }) | Event::Start(Tag::Image { dest_url, .. })) =
            evento
        else {
            continue;
        };
        let Some(alvo) = resolver(de, &dest_url) else {
            continue;
        };
        let ancora = dest_url
            .split_once('#')
            .map(|(_, a)| format!("#{a}"))
            .unwrap_or_default();
        // O destino fica depois do último `](` do trecho do link.
        let trecho = &corpo[r.clone()];
        let Some(abre) = trecho.rfind("](") else {
            continue;
        };
        let inicio = r.start + abre + 2;
        let resto = &corpo[inicio..r.end];
        let (desloc, tamanho) = if resto.starts_with('<') {
            match resto.find('>') {
                Some(f) => (0, f + 1),
                None => continue,
            }
        } else {
            (0, resto.find([' ', ')', '\t', '\n']).unwrap_or(resto.len()))
        };
        let novo = caminho_relativo(para, &alvo) + &ancora;
        let novo = if novo.contains([' ', '(', ')', '<', '>']) {
            format!("<{novo}>")
        } else {
            novo
        };
        edicoes.push((inicio + desloc..inicio + desloc + tamanho, novo));
    }
    exportacao::aplicar(corpo, edicoes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_e_relocaliza_links_e_imagens() {
        let de = c("analises/A-0001.md");
        assert_eq!(
            resolver(&de, "../funcionalidades/can/cancelar/manual/p.png")
                .unwrap()
                .as_str(),
            "funcionalidades/can/cancelar/manual/p.png"
        );
        assert_eq!(resolver(&de, "../../fora.md"), None);
        assert_eq!(resolver(&de, "https://x.com/a.png"), None);
        assert_eq!(resolver(&de, "#secao"), None);
        let corpo = "Veja [RN](../regras/RN-FAT-001.md#enunciado) e ![Tela](../funcionalidades/can/cancelar/manual/p%201.png) e [site](https://x.com) e [doc](<../a b.md>).\n";
        let para = c("funcionalidades/can/cancelar/manual.md");
        assert_eq!(
            relocalizar(corpo, &de, &para),
            "Veja [RN](../../../regras/RN-FAT-001.md#enunciado) e ![Tela](manual/p%201.png) e [site](https://x.com) e [doc](<../../../a b.md>).\n"
        );
    }

    fn c(s: &str) -> CaminhoRelativo {
        CaminhoRelativo::novo(s).unwrap()
    }

    #[test]
    fn nome_da_crate() {
        assert_eq!(NOME, "cba-docs");
    }

    #[test]
    fn caminhos_relativos_entre_documentos() {
        assert_eq!(
            caminho_relativo(&c("analises/A-0001.md"), &c("sugestoes/S-0001.md")),
            "../sugestoes/S-0001.md"
        );
        assert_eq!(
            caminho_relativo(&c("analises/A-0001.md"), &c("analises/A-0002.md")),
            "A-0002.md"
        );
        assert_eq!(
            caminho_relativo(
                &c("funcionalidades/cobranca/multa/negocial.md"),
                &c("analises/A-0001.md")
            ),
            "../../../analises/A-0001.md"
        );
        assert_eq!(
            caminho_relativo(&c("projeto.md"), &c("regras/RN-FAT-001.md")),
            "regras/RN-FAT-001.md"
        );
        assert_eq!(
            link("a [b]", &c("x.md"), &c("pasta com espaço/y.md")),
            "[a \\[b\\]](<pasta com espaço/y.md>)"
        );
    }
}
