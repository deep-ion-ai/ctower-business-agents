//! Documentos das análises.
//!
//! Templates por tipo de documento e montagem dos documentos (spec, seção 7.3).
//! Exportação para Markdown e Word: [`exportar_markdown`], [`exportar_word`] e
//! [`gravar_exportacao`] (módulo `exportacao`).
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
//! A ata de reunião segue o mesmo caminho a partir de um estado próprio
//! ([`EstadoAta`], em `R-NNNN.estado.json`), convertido num [`Conteudo`] e
//! montado pelo template `ata` ([`Templates::gravar_ata`]).
//!
//! Papel na arquitetura: seção 5 de `docs/specs/2026-11-11-mvp-standalone.md`.

mod ata;
mod conteudo;
mod exportacao;
mod modelo;
mod montagem;
mod template;
mod word;

use cba_kb::CaminhoRelativo;

pub use ata::{
    CitacaoFala, ErroAta, EstadoAta, IdItemAta, ItemAta, NovoItem, Participante, StatusCitacao,
    TipoItem,
};
pub use conteudo::{
    Afirmacao, Citacao, Confianca, Conteudo, Pergunta, ReferenciaSugestao, Texto, bloco_de_codigo,
};
pub use exportacao::{
    CodigoEmPasta, ErroExportacao, FonteDeTrechos, Formato, LinksDeEvidencia, OpcoesExportacao,
    exportar, exportar_com_links, exportar_markdown, exportar_markdown_com_links, exportar_word,
    gravar_exportacao, gravar_exportacao_com_links,
};
pub use modelo::{MODELOS, Modelo, Montavel, Secao, modelo};
pub use montagem::{ErroMontagem, Montado};
pub use template::{Aviso, Origem, Template, Templates, arquivo_do_template};

/// Nome da crate, para diagnóstico.
pub const NOME: &str = env!("CARGO_PKG_NAME");

/// Link Markdown de um documento da base para outro, com caminho relativo ao
/// documento de origem (`de`). Os dois caminhos são relativos à raiz do projeto.
pub(crate) fn link(texto: &str, de: &CaminhoRelativo, para: &CaminhoRelativo) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

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
