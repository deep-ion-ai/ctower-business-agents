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

/// Mapa de funcionalidades do projeto (#208), em `funcionalidades/`.
pub const ARQUIVO_MAPA: &str = "mapa.md";
/// Estado do mapa, ao lado dele: a fonte de verdade das funcionalidades e dos
/// identificadores.
pub const ARQUIVO_ESTADO_MAPA: &str = "mapa.estado.json";
/// Trava das alterações do estado do mapa (app e servidor MCP).
pub const ARQUIVO_TRAVA_MAPA: &str = ".mapa.trava";
/// Estado do catálogo da primeira versão do #208, quando o mapa era aprovado por
/// análise: só lido, para importar.
pub const ARQUIVO_ESTADO_CATALOGO_LEGADO: &str = "catalogo.estado.json";

/// Documentação externa importada do repositório de docs (#277), uma pasta por
/// repositório (`externos/<slug do repositório>/`), com a mesma estrutura de
/// pastas do original. Gerida pela sincronização: o que não veio do repositório é
/// apagado, e editar uma cópia não adianta (a próxima sincronização desfaz).
/// Criada na primeira sincronização, não em todo projeto.
pub const EXTERNOS: &str = "externos";
/// Estado da sincronização da documentação externa, em `externos/` (arquivo
/// oculto: fica fora de [`crate::Projeto::documentos`]).
pub const ARQUIVO_ESTADO_EXTERNOS: &str = ".sincronizacao.json";
/// Trava da sincronização (app e servidor MCP), em `externos/`.
pub const TRAVA_EXTERNOS: &str = ".sincronizacao.trava";

/// Protótipo de telas (#356): os tokens CSS do projeto (#384), extraídos do
/// código da interface e revisados pelo analista. Criada na primeira extração,
/// não em todo projeto.
pub const PROTOTIPO: &str = "prototipo";
/// Estado dos tokens CSS, em `prototipo/`: a fonte de verdade da revisão.
pub const ARQUIVO_ESTADO_TOKENS: &str = "tokens.estado.json";
/// Arquivo de variáveis CSS (`:root`) gerado do estado, em `prototipo/`.
pub const ARQUIVO_TOKENS_CSS: &str = "tokens.css";
/// Trava das alterações dos tokens (app e servidor MCP), em `prototipo/`.
pub const ARQUIVO_TRAVA_TOKENS: &str = ".tokens.trava";
/// Extensão do protótipo HTML de uma tela (#385).
pub const EXTENSAO_HTML: &str = "html";

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

/// `analises/<id>.memoria.md`, a memória de trabalho de uma análise.
pub fn memoria_da_analise(id: &crate::Id) -> CaminhoRelativo {
    caminho(format!("{ANALISES}/{id}{SUFIXO_MEMORIA_DA_ANALISE}"))
        .expect("pasta e identificador formam um caminho válido")
}

/// Se o nome é o de uma memória de análise (não é documento da base).
pub fn e_memoria_da_analise(nome: &str) -> bool {
    nome.ends_with(SUFIXO_MEMORIA_DA_ANALISE)
}

/// `funcionalidades/mapa.md`.
pub fn mapa_de_funcionalidades() -> CaminhoRelativo {
    caminho(format!("{FUNCIONALIDADES}/{ARQUIVO_MAPA}")).expect("nome fixo válido")
}

/// `funcionalidades/mapa.estado.json`.
pub fn estado_do_mapa() -> CaminhoRelativo {
    caminho(format!("{FUNCIONALIDADES}/{ARQUIVO_ESTADO_MAPA}")).expect("nome fixo válido")
}

/// `funcionalidades/.mapa.trava`.
pub fn trava_do_mapa() -> CaminhoRelativo {
    caminho(format!("{FUNCIONALIDADES}/{ARQUIVO_TRAVA_MAPA}")).expect("nome fixo válido")
}

/// `funcionalidades/catalogo.estado.json` (legado, só para importar).
pub fn estado_do_catalogo_legado() -> CaminhoRelativo {
    caminho(format!(
        "{FUNCIONALIDADES}/{ARQUIVO_ESTADO_CATALOGO_LEGADO}"
    ))
    .expect("nome fixo válido")
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

/// `prototipo/tokens.estado.json`.
pub fn estado_dos_tokens() -> CaminhoRelativo {
    caminho(format!("{PROTOTIPO}/{ARQUIVO_ESTADO_TOKENS}")).expect("nome fixo válido")
}

/// `prototipo/tokens.css`.
pub fn tokens_css() -> CaminhoRelativo {
    caminho(format!("{PROTOTIPO}/{ARQUIVO_TOKENS_CSS}")).expect("nome fixo válido")
}

/// `prototipo/.tokens.trava`.
pub fn trava_dos_tokens() -> CaminhoRelativo {
    caminho(format!("{PROTOTIPO}/{ARQUIVO_TRAVA_TOKENS}")).expect("nome fixo válido")
}

/// `analises/<id>/prototipo/`: os protótipos HTML das telas de uma análise de
/// protótipo (#385), um arquivo por tela. Criada no primeiro HTML gravado.
pub fn prototipo_da_analise(id: &crate::Id) -> CaminhoRelativo {
    caminho(format!("{ANALISES}/{id}/{PROTOTIPO}"))
        .expect("pasta e identificador formam um caminho válido")
}

/// `analises/<id>/prototipo/TL-<numero>.html`: o protótipo HTML de uma tela.
pub fn html_da_tela(id: &crate::Id, numero: u32) -> CaminhoRelativo {
    caminho(format!(
        "{ANALISES}/{id}/{PROTOTIPO}/TL-{numero}.{EXTENSAO_HTML}"
    ))
    .expect("pasta, identificador e número formam um caminho válido")
}

/// `externos/`.
pub fn externos() -> CaminhoRelativo {
    CaminhoRelativo::novo(EXTERNOS).expect("nome fixo válido")
}

/// `externos/<slug do repositório>/`, a pasta das cópias de um repositório.
pub fn externos_do_repositorio(repositorio: &str) -> Result<CaminhoRelativo, CaminhoInvalido> {
    caminho(format!("{EXTERNOS}/{}", exigir_slug(repositorio)?))
}

/// `externos/.sincronizacao.json`.
pub fn estado_dos_externos() -> CaminhoRelativo {
    caminho(format!("{EXTERNOS}/{ARQUIVO_ESTADO_EXTERNOS}")).expect("nome fixo válido")
}

/// Se o caminho (relativo à raiz do projeto) é de um documento externo: só
/// leitura no app, nunca regra nem termo da base.
pub fn e_externo(relativo: &CaminhoRelativo) -> bool {
    relativo.partes().next() == Some(EXTERNOS)
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
        assert_eq!(
            mapa_de_funcionalidades().as_str(),
            "funcionalidades/mapa.md"
        );
        assert_eq!(
            estado_do_mapa().as_str(),
            "funcionalidades/mapa.estado.json"
        );
        let id = crate::Id::analisar("A-0007").unwrap();
        assert_eq!(
            memoria_da_analise(&id).as_str(),
            "analises/A-0007.memoria.md"
        );
        assert!(e_memoria_da_analise("A-0007.memoria.md"));
        assert!(!e_memoria_da_analise("A-0007.md"));
        assert!(!e_memoria_da_analise("memoria.md"));
        let externos = externos_do_repositorio("GitHub exemplo/faturamento-docs").unwrap();
        assert_eq!(
            externos.as_str(),
            "externos/github-exemplo-faturamento-docs"
        );
        assert!(e_externo(&externos.juntar("regras/multa.md").unwrap()));
        assert!(!e_externo(&termo("multa").unwrap()));
        assert_eq!(
            estado_dos_externos().as_str(),
            "externos/.sincronizacao.json"
        );
        assert_eq!(estado_dos_tokens().as_str(), "prototipo/tokens.estado.json");
        assert_eq!(tokens_css().as_str(), "prototipo/tokens.css");
        assert_eq!(trava_dos_tokens().as_str(), "prototipo/.tokens.trava");
        let id = crate::Id::analisar("A-0002").unwrap();
        assert_eq!(
            prototipo_da_analise(&id).as_str(),
            "analises/A-0002/prototipo"
        );
        assert_eq!(
            html_da_tela(&id, 3).as_str(),
            "analises/A-0002/prototipo/TL-3.html"
        );
    }
}
