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

pub const GLOSSARIO: &str = "glossario";
pub const REGRAS: &str = "regras";
pub const AREAS: &str = "areas";
pub const FUNCIONALIDADES: &str = "funcionalidades";
pub const ANALISES: &str = "analises";
pub const SUGESTOES: &str = "sugestoes";
/// Demandas, uma pasta por demanda (`demandas/D-NNNN-<slug>/`). Criada na
/// primeira demanda, não em todo projeto.
pub const DEMANDAS: &str = "demandas";
/// Documento da demanda, dentro da pasta dela.
pub const ARQUIVO_DEMANDA: &str = "demanda.md";
/// Catálogo de pessoas do projeto, um documento por pessoa (`pessoas/<slug>.md`).
/// Criada na primeira pessoa, não em todo projeto.
pub const PESSOAS: &str = "pessoas";
/// Catálogo de papéis das pessoas, editável, em `_templates/`.
pub const ARQUIVO_PAPEIS: &str = "papeis.yaml";
/// Atas das reuniões, dentro da pasta da demanda (`reunioes/R-NNNN.md`, com o
/// estado em `reunioes/R-NNNN.estado.json`).
pub const REUNIOES: &str = "reunioes";
/// Sufixo do arquivo de estado de uma ata (`R-NNNN.estado.json`).
pub const SUFIXO_ESTADO_ATA: &str = ".estado.json";

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

/// `_templates/papeis.yaml`, relativo à raiz da base.
pub fn papeis() -> CaminhoRelativo {
    caminho(format!("{TEMPLATES}/{ARQUIVO_PAPEIS}")).expect("nome fixo válido")
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
        assert_eq!(projeto().as_str(), "projeto.md");
        assert_eq!(regras_do_projeto().as_str(), "regras-do-projeto.md");
        assert_eq!(papeis().as_str(), "_templates/papeis.yaml");
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
