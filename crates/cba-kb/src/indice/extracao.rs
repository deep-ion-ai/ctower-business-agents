//! Do arquivo da base ao registro do índice.
//!
//! Os campos comuns (identificador, título, área, status, ligações) vêm do
//! frontmatter livre, para um documento com um campo inválido continuar na busca.
//! A validação do tipo (o frontmatter tipado de cada tipo) só decide se o
//! documento entra em [`super::Indice::problemas`].

use crate::base::Projeto;
use crate::caminho::CaminhoRelativo;
use crate::documento::Documento;
use crate::frontmatter::{
    self, Frontmatter, FrontmatterAnalise, FrontmatterDemanda, FrontmatterFuncionalidade,
    FrontmatterLivre, FrontmatterMemoria, FrontmatterProjeto, FrontmatterRegra,
    FrontmatterSugestao, FrontmatterTermo, TipoDocumento,
};
use crate::id::{Id, IdDemanda};
use crate::layout;
use crate::yaml::{Mapping, Value};

use super::Tipo;

/// O que o índice guarda de um documento.
#[derive(Debug, Default)]
pub(super) struct Registro {
    /// Relativo à raiz da base.
    pub caminho: String,
    pub projeto: String,
    pub tipo: Option<Tipo>,
    pub identificador: Option<String>,
    pub titulo: String,
    pub area: Option<String>,
    pub status: Option<String>,
    /// Frontmatter em JSON (`{}` se não houver).
    pub frontmatter: String,
    pub erro: Option<String>,
    /// Entra na busca textual.
    pub buscavel: bool,
    /// Identificador, termo, sinônimos e área, para a busca.
    pub chaves: String,
    /// Corpo sem comentários HTML.
    pub corpo: String,
    pub evidencias: Vec<frontmatter::Evidencia>,
    /// (documento citado, relativo à base; campo).
    pub ligacoes: Vec<(String, &'static str)>,
}

/// Monta o registro de um documento a partir dos bytes do arquivo.
pub(super) fn extrair(projeto: &Projeto, relativo: &CaminhoRelativo, bytes: Vec<u8>) -> Registro {
    let mut r = Registro {
        caminho: projeto.na_base(relativo).as_str().to_owned(),
        projeto: projeto.nome().to_owned(),
        frontmatter: "{}".into(),
        ..Default::default()
    };
    let texto = match String::from_utf8(bytes) {
        Ok(t) => t,
        Err(_) => {
            r.titulo = titulo_do_arquivo(relativo);
            r.erro = Some("o arquivo não está em UTF-8".into());
            return r;
        }
    };

    if relativo.as_str() == layout::ARQUIVO_REGRAS_DO_PROJETO {
        r.tipo = Some(Tipo::RegrasDoProjeto);
        r.corpo = sem_comentarios_html(&texto);
        r.titulo = primeiro_titulo(&r.corpo).unwrap_or_else(|| "Regras do projeto".into());
        r.buscavel = true;
        return r;
    }

    let documento = match Documento::<FrontmatterLivre>::analisar(&texto) {
        Ok(d) => d,
        Err(erro) => {
            r.titulo = titulo_do_arquivo(relativo);
            r.erro = Some(erro.to_string());
            return r;
        }
    };
    let mapa = &documento.frontmatter.0;
    r.frontmatter = serde_json::to_string(mapa).unwrap_or_else(|_| "{}".into());
    r.corpo = sem_comentarios_html(&documento.corpo);
    r.buscavel = true;
    let tipo = documento.frontmatter.tipo();
    r.tipo = tipo.map(Tipo::Documento);
    r.identificador = texto_do_campo(mapa, "id").or_else(|| texto_do_campo(mapa, "termo"));
    r.titulo = texto_do_campo(mapa, "titulo")
        .or_else(|| texto_do_campo(mapa, "termo"))
        .or_else(|| texto_do_campo(mapa, "nome"))
        .or_else(|| primeiro_titulo(&r.corpo))
        .unwrap_or_else(|| titulo_do_arquivo(relativo));
    r.area = texto_do_campo(mapa, "area");
    r.status = texto_do_campo(mapa, "status");

    let mut chaves: Vec<String> = Vec::new();
    chaves.extend(r.identificador.clone());
    chaves.extend(textos_da_lista(mapa, "sinonimos"));
    chaves.extend(r.area.clone());
    r.chaves = chaves.join("\n");

    if let Some(origem) =
        texto_do_campo(mapa, "origem").and_then(|o| CaminhoRelativo::novo(&o).ok())
    {
        r.ligacoes
            .push((projeto.na_base(&origem).as_str().to_owned(), "origem"));
    }
    if let Some(analise) = texto_do_campo(mapa, "analise").and_then(|a| Id::analisar(&a).ok()) {
        r.ligacoes.push((
            projeto.na_base(&analise.arquivo()).as_str().to_owned(),
            "analise",
        ));
    }
    if let Some(id) = mapa
        .get("demanda")
        .and_then(Value::as_mapping)
        .and_then(|d| texto_do_campo(d, "id"))
        .and_then(|id| IdDemanda::analisar(&id).ok())
    {
        let arquivo = projeto.arquivo_da_demanda(id);
        r.ligacoes
            .push((projeto.na_base(&arquivo).as_str().to_owned(), "demanda"));
    }
    if let Some(valor) = mapa.get("evidencias") {
        r.evidencias = serde_norway::from_value(valor.clone()).unwrap_or_default();
    }

    r.erro = match tipo {
        None => Some(match mapa.get("tipo") {
            None => "frontmatter sem o campo `tipo`".into(),
            Some(_) => "campo `tipo` com valor desconhecido".into(),
        }),
        Some(tipo) => validar(tipo, relativo, &texto).err(),
    };
    r
}

/// Valida o documento com o frontmatter tipado do seu tipo.
fn validar(tipo: TipoDocumento, relativo: &CaminhoRelativo, texto: &str) -> Result<(), String> {
    fn com<F: Frontmatter>(texto: &str) -> Result<(), String> {
        Documento::<F>::analisar(texto)
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
    match tipo {
        TipoDocumento::Projeto => com::<FrontmatterProjeto>(texto),
        TipoDocumento::AnaliseImpacto | TipoDocumento::Pergunta => com::<FrontmatterAnalise>(texto),
        // Explicação de funcionalidade: análise em `analises/`, documento negocial
        // em `funcionalidades/`.
        TipoDocumento::Funcionalidade if relativo.partes().next() == Some(layout::ANALISES) => {
            com::<FrontmatterAnalise>(texto)
        }
        TipoDocumento::Funcionalidade | TipoDocumento::CasoDeUso => {
            com::<FrontmatterFuncionalidade>(texto)
        }
        TipoDocumento::Regra => com::<FrontmatterRegra>(texto),
        TipoDocumento::Termo => com::<FrontmatterTermo>(texto),
        TipoDocumento::Sugestao => com::<FrontmatterSugestao>(texto),
        TipoDocumento::MemoriaArea => com::<FrontmatterMemoria>(texto),
        TipoDocumento::Demanda => com::<FrontmatterDemanda>(texto),
    }
}

fn texto_de(valor: &Value) -> Option<String> {
    let texto = match valor {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        _ => return None,
    };
    let texto = texto.trim();
    (!texto.is_empty()).then(|| texto.to_owned())
}

fn texto_do_campo(mapa: &Mapping, campo: &str) -> Option<String> {
    mapa.get(campo).and_then(texto_de)
}

fn textos_da_lista(mapa: &Mapping, campo: &str) -> Vec<String> {
    match mapa.get(campo) {
        Some(Value::Sequence(itens)) => itens.iter().filter_map(texto_de).collect(),
        Some(outro) => texto_de(outro).into_iter().collect(),
        None => Vec::new(),
    }
}

/// Texto do primeiro título Markdown (`# ...`).
fn primeiro_titulo(corpo: &str) -> Option<String> {
    corpo.lines().find_map(|linha| {
        let resto = linha.trim_start().strip_prefix('#')?;
        let titulo = resto.trim_start_matches('#').trim();
        (!titulo.is_empty()).then(|| titulo.to_owned())
    })
}

fn titulo_do_arquivo(relativo: &CaminhoRelativo) -> String {
    let nome = relativo.nome();
    nome.strip_suffix(".md").unwrap_or(nome).to_owned()
}

/// Remove os comentários HTML (`<!-- ... -->`), que são instruções para quem
/// edita. Um comentário sem fechamento vai até o fim.
pub(super) fn sem_comentarios_html(texto: &str) -> String {
    let mut saida = String::with_capacity(texto.len());
    let mut resto = texto;
    while let Some(inicio) = resto.find("<!--") {
        saida.push_str(&resto[..inicio]);
        match resto[inicio + 4..].find("-->") {
            Some(fim) => resto = &resto[inicio + 4 + fim + 3..],
            None => {
                resto = "";
            }
        }
    }
    saida.push_str(resto);
    saida
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remove_comentarios() {
        assert_eq!(sem_comentarios_html("a<!-- x -->b<!-- y"), "ab");
        assert_eq!(sem_comentarios_html("sem comentário"), "sem comentário");
    }

    #[test]
    fn titulo_do_corpo() {
        assert_eq!(
            primeiro_titulo("\ntexto\n## Regras do projeto\n").as_deref(),
            Some("Regras do projeto")
        );
        assert_eq!(primeiro_titulo("#\n"), None);
    }
}
