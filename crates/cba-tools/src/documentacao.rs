//! Documentação legível do projeto (#288): lê a base aprovada e o mapa e chama
//! o renderizador puro [`cba_docs::gerar_documentacao`].
//!
//! O renderizador fica em `cba-docs`, ao lado da exportação de que reaproveita a
//! limpeza do Markdown e os links das notas de evidência; aqui só se lê o que
//! está no disco e se resolvem os permalinks pelo cadastro dos repositórios
//! ([`LinksNaForja`]). A publicação (#289) grava as páginas.

use cba_core::funcionalidades::MapaFuncionalidades;
use cba_docs::{ContextoDaDocumentacao, Documentacao, DocumentoDaBase, LinksDeEvidencia};
use cba_kb::Projeto;
use cba_kb::frontmatter::Linhas;
use cba_repos::RepositorioCodigo;

use crate::publicacao::documentos_aprovados;

/// Links das evidências nos repositórios de código do projeto (GitHub ou
/// GitLab), no commit da evidência (ou no padrão de quem chama). Cada arquivo
/// vai para o repositório cuja pasta o contém, resolvida pelo escopo do
/// projeto; no cadastro antigo sem pasta, para o único repositório.
pub struct LinksNaForja {
    repos: Vec<RepositorioCodigo>,
    escopo: cba_index::Escopo,
}

impl LinksNaForja {
    /// `None` sem repositório de código cadastrado.
    pub fn novo(repos: &[RepositorioCodigo], escopo: &cba_index::Escopo) -> Option<Self> {
        if repos.is_empty() {
            return None;
        }
        // Pastas canônicas, para comparar com o caminho resolvido pelo escopo.
        let repos = repos
            .iter()
            .cloned()
            .map(|mut r| {
                r.pasta = r.pasta.map(|p| dunce::canonicalize(&p).unwrap_or(p));
                r
            })
            .collect();
        Some(Self {
            repos,
            escopo: escopo.clone(),
        })
    }
}

impl LinksDeEvidencia for LinksNaForja {
    fn link(&self, arquivo: &str, linhas: Linhas, commit: Option<&str>) -> Option<String> {
        let absoluto = self.escopo.resolver(arquivo).ok();
        cba_repos::link::link_evidencia(
            &self.repos,
            absoluto.as_deref(),
            commit?,
            arquivo,
            Some((linhas.inicio, linhas.fim)),
        )
    }
}

/// Gera a documentação legível do projeto a partir dos documentos aprovados
/// ([`crate::publicacao::aprovado`]) e do mapa. O nome do projeto e das áreas
/// vem do `projeto.md`; `data` (AAAA-MM-DD) vai só no README.
pub fn documentacao_do_projeto(
    projeto: &Projeto,
    links: Option<&dyn LinksDeEvidencia>,
    data: Option<String>,
) -> Result<Documentacao, String> {
    let documentos: Vec<DocumentoDaBase> = documentos_aprovados(projeto)?
        .into_iter()
        .filter_map(|d| {
            Some(DocumentoDaBase {
                texto: String::from_utf8(d.conteudo).ok()?,
                caminho: d.caminho,
            })
        })
        .collect();
    let mapa = MapaFuncionalidades::carregar(projeto).map_err(|e| e.to_string())?;
    let (nome, areas) = match projeto.ler_projeto() {
        Ok(p) => (
            p.frontmatter.nome,
            p.frontmatter
                .areas
                .into_iter()
                .map(|a| (a.id, a.nome))
                .collect(),
        ),
        Err(_) => (projeto.nome().to_owned(), Default::default()),
    };
    let contexto = ContextoDaDocumentacao {
        projeto: nome,
        areas,
        codigo: None,
        data,
    };
    Ok(cba_docs::gerar_documentacao(
        &documentos,
        Some(&mapa),
        links,
        &contexto,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn copiar(de: &Path, para: &Path) {
        std::fs::create_dir_all(para).unwrap();
        for e in std::fs::read_dir(de).unwrap() {
            let p = e.unwrap().path();
            let destino = para.join(p.file_name().unwrap());
            if p.is_dir() {
                copiar(&p, &destino);
            } else {
                std::fs::copy(&p, destino).unwrap();
            }
        }
    }

    #[test]
    fn le_a_base_aprovada_e_o_mapa() {
        let tmp = tempfile::tempdir().unwrap();
        let raiz = tmp.path().join("faturamento");
        copiar(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("../cba-docs/tests/documentacao/base"),
            &raiz,
        );
        std::fs::write(
            raiz.join("projeto.md"),
            "---\ntipo: projeto\nid: faturamento\nnome: Faturamento Exemplo\nareas:\n- id: cobranca\n  nome: Cobrança e multas\n---\n# Faturamento\n",
        )
        .unwrap();
        let projeto = cba_kb::Base::abrir(tmp.path())
            .unwrap()
            .projeto("faturamento")
            .unwrap();
        let d = documentacao_do_projeto(&projeto, None, None).unwrap();
        let caminhos: Vec<&str> = d.paginas.iter().map(|p| p.caminho.as_str()).collect();
        assert!(
            caminhos.contains(&"funcionalidades/cobranca/f-cob-001-cobrar-multa-por-atraso.md"),
            "{caminhos:?}"
        );
        // O documento sem status não é aprovado: nem página nem aviso.
        assert!(d.avisos.is_empty(), "{:?}", d.avisos);
        let readme = &d.paginas[0].conteudo;
        assert!(readme.contains("# Faturamento Exemplo · documentação de negócio"));
        assert!(readme.contains("### Cobrança e multas"), "{readme}");
    }
}
