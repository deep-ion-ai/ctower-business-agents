//! Nomes de pastas e arquivos da base (spec, seção 7.1) e os caminhos padrão de
//! cada tipo de documento, relativos à raiz do sistema.

use crate::caminho::{CaminhoInvalido, CaminhoRelativo, slug};

/// Templates dos documentos, na raiz da base.
pub const TEMPLATES: &str = "_templates";
/// Documento do sistema, na raiz do sistema.
pub const ARQUIVO_SISTEMA: &str = "sistema.md";
/// Regras do sistema para o agente (convenções do sistema e da organização), na
/// raiz do sistema. Editáveis pelo analista e versionadas com a base; o hook
/// `SessionStart` as soma aos limites fixos (spec, seção 8). Não confundir com
/// `regras/`, onde ficam as regras de negócio (`RN-<AREA>-<NNN>.md`).
pub const ARQUIVO_REGRAS_DO_SISTEMA: &str = "regras-do-sistema.md";
/// Memória de uma área, em `areas/<area>/`.
pub const ARQUIVO_MEMORIA: &str = "memoria.md";

pub const GLOSSARIO: &str = "glossario";
pub const REGRAS: &str = "regras";
pub const AREAS: &str = "areas";
pub const FUNCIONALIDADES: &str = "funcionalidades";
pub const ANALISES: &str = "analises";
pub const SUGESTOES: &str = "sugestoes";

/// Pastas criadas em todo sistema.
pub const PASTAS_DO_SISTEMA: [&str; 6] = [
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

/// `sistema.md`.
pub fn sistema() -> CaminhoRelativo {
    CaminhoRelativo::novo(ARQUIVO_SISTEMA).expect("nome fixo válido")
}

/// `regras-do-sistema.md`.
pub fn regras_do_sistema() -> CaminhoRelativo {
    CaminhoRelativo::novo(ARQUIVO_REGRAS_DO_SISTEMA).expect("nome fixo válido")
}

/// `glossario/<slug do termo>.md`.
pub fn termo(termo: &str) -> Result<CaminhoRelativo, CaminhoInvalido> {
    caminho(format!("{GLOSSARIO}/{}.{EXTENSAO}", exigir_slug(termo)?))
}

/// `areas/<slug da área>/memoria.md`.
pub fn memoria(area: &str) -> Result<CaminhoRelativo, CaminhoInvalido> {
    caminho(format!("{AREAS}/{}/{ARQUIVO_MEMORIA}", exigir_slug(area)?))
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
        assert_eq!(sistema().as_str(), "sistema.md");
        assert_eq!(regras_do_sistema().as_str(), "regras-do-sistema.md");
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
    }
}
