//! Nomes de pastas e arquivos da base (spec, seção 7.1) e os caminhos padrão de
//! cada tipo de documento, relativos à raiz do projeto.

use crate::caminho::{CaminhoInvalido, CaminhoRelativo, slug};

/// Templates dos documentos, na raiz da base.
pub const TEMPLATES: &str = "_templates";
/// Documento do projeto, na raiz do projeto.
pub const ARQUIVO_PROJETO: &str = "projeto.md";
/// Regras do projeto para o agente (convenções do projeto e da organização), na
/// raiz do projeto. Editáveis pelo analista e versionadas com a base; o hook
/// `SessionStart` as soma aos limites fixos (spec, seção 8). Não confundir com
/// `regras/`, onde ficam as regras de negócio (`RN-<AREA>-<NNN>.md`).
pub const ARQUIVO_REGRAS_DO_PROJETO: &str = "regras-do-projeto.md";
/// Memória de uma área, em `areas/<area>/`.
pub const ARQUIVO_MEMORIA: &str = "memoria.md";
/// Sufixo da memória de uma análise (`analises/A-0001.memoria.md`): texto de
/// trabalho do agente, ao lado do documento. Não é documento da base: fica fora
/// de [`crate::Projeto::documentos`] e, por isso, do índice e da busca.
pub const SUFIXO_MEMORIA_DA_ANALISE: &str = ".memoria.md";

pub const GLOSSARIO: &str = "glossario";
pub const REGRAS: &str = "regras";
pub const AREAS: &str = "areas";
pub const FUNCIONALIDADES: &str = "funcionalidades";
pub const ANALISES: &str = "analises";
pub const SUGESTOES: &str = "sugestoes";

/// Pastas criadas em todo projeto.
pub const PASTAS_DO_PROJETO: [&str; 6] = [
    GLOSSARIO,
    REGRAS,
    AREAS,
    FUNCIONALIDADES,
    ANALISES,
    SUGESTOES,
];

/// Extensão dos documentos.
pub const EXTENSAO: &str = "md";

fn caminho(texto: String) -> Result<CaminhoRelativo, CaminhoInvalido> {
    CaminhoRelativo::novo(&texto)
}

fn exigir_slug(texto: &str) -> Result<String, CaminhoInvalido> {
    let s = slug(texto);
    if s.is_empty() {
        return Err(CaminhoInvalido {
            caminho: texto.to_owned(),
            motivo: "sem letras nem dígitos para formar o nome do arquivo",
        });
    }
    Ok(s)
}

/// `projeto.md`.
pub fn projeto() -> CaminhoRelativo {
    CaminhoRelativo::novo(ARQUIVO_PROJETO).expect("nome fixo válido")
}

/// `regras-do-projeto.md`.
pub fn regras_do_projeto() -> CaminhoRelativo {
    CaminhoRelativo::novo(ARQUIVO_REGRAS_DO_PROJETO).expect("nome fixo válido")
}

/// `glossario/<slug do termo>.md`.
pub fn termo(termo: &str) -> Result<CaminhoRelativo, CaminhoInvalido> {
    caminho(format!("{GLOSSARIO}/{}.{EXTENSAO}", exigir_slug(termo)?))
}

/// `areas/<slug da área>/memoria.md`.
pub fn memoria(area: &str) -> Result<CaminhoRelativo, CaminhoInvalido> {
    caminho(format!("{AREAS}/{}/{ARQUIVO_MEMORIA}", exigir_slug(area)?))
}

/// `analises/<id>.memoria.md`, a memória de trabalho de uma análise.
pub fn memoria_da_analise(id: &crate::Id) -> CaminhoRelativo {
    caminho(format!("{ANALISES}/{id}{SUFIXO_MEMORIA_DA_ANALISE}"))
        .expect("pasta e identificador formam um caminho válido")
}

/// Se o nome é o de uma memória de análise (não é documento da base).
pub fn e_memoria_da_analise(nome: &str) -> bool {
    nome.ends_with(SUFIXO_MEMORIA_DA_ANALISE)
}

/// `funcionalidades/<slug da área>/<slug do tema>/`, pasta dos documentos de uma
/// funcionalidade.
pub fn funcionalidade(area: &str, tema: &str) -> Result<CaminhoRelativo, CaminhoInvalido> {
    caminho(format!(
        "{FUNCIONALIDADES}/{}/{}",
        exigir_slug(area)?,
        exigir_slug(tema)?
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caminhos_padrao() {
        assert_eq!(projeto().as_str(), "projeto.md");
        assert_eq!(regras_do_projeto().as_str(), "regras-do-projeto.md");
        assert_eq!(
            termo("Nota Fiscal").unwrap().as_str(),
            "glossario/nota-fiscal.md"
        );
        assert_eq!(
            memoria("Cobrança").unwrap().as_str(),
            "areas/cobranca/memoria.md"
        );
        assert_eq!(
            funcionalidade("cobranca", "Cancelamento de fatura")
                .unwrap()
                .as_str(),
            "funcionalidades/cobranca/cancelamento-de-fatura"
        );
        assert!(termo("??").is_err());
        let id = crate::Id::analisar("A-0007").unwrap();
        assert_eq!(
            memoria_da_analise(&id).as_str(),
            "analises/A-0007.memoria.md"
        );
        assert!(e_memoria_da_analise("A-0007.memoria.md"));
        assert!(!e_memoria_da_analise("A-0007.md"));
        assert!(!e_memoria_da_analise("memoria.md"));
    }
}
