//! Operações dos documentos vivos (#461) sobre o projeto: o estado de cada
//! documento, calculado sem IA, e o ciclo da proposta de versão até a
//! publicação.
//!
//! O modelo (versões, trechos marcados, revisão final, histórico) está em
//! [`cba_core::documentos_vivos`]; este módulo junta o que depende do projeto:
//!
//! - **Estado** ([`estado`], [`documentos_da_funcionalidade`]): o documento fica
//!   **desatualizado** quando um arquivo citado nas evidências da versão
//!   publicada mudou (ou sumiu) depois da publicação. Cada versão guarda o hash
//!   do arquivo inteiro ([`hash_do_arquivo`]), e a conta é como a da revalidação
//!   das regras: lê o arquivo pelo escopo do projeto e compara.
//! - **Proposta** ([`propor`]): valida o tipo (vivo, do catálogo) e o dono
//!   (funcionalidade aberta do mapa ou área), acha o caminho (o do documento que
//!   já existe, o vinculado à funcionalidade ou o padrão) e registra o rascunho.
//!   Um documento que já estava na base sem histórico (o negocial de antes) entra
//!   com o texto atual como v1.0.
//! - **Publicação** ([`publicar`]): só depois da revisão final. Grava a revisão
//!   no histórico, com os hashes dos arquivos citados, reescreve o arquivo na
//!   base com a tabela "Histórico de revisões" (o frontmatter que havia fica) e
//!   vincula o documento à funcionalidade.

use cba_core::ambiente::Ambiente;
use cba_core::documentos_vivos::{
    ArquivoCitado, DocumentoVivo, DocumentosVivos, Dono, ErroDocumento, EstadoDocumento,
    NovaProposta, Proposta, Revisao, Versao, sem_historico,
};
use cba_core::funcionalidades::{Funcionalidade, MapaFuncionalidades};
use cba_docs::entregaveis::{self, CicloDeVida};
use cba_index::Escopo;
use cba_kb::frontmatter::{FrontmatterFuncionalidade, StatusAnalise};
use cba_kb::{CaminhoRelativo, Documento, IdFuncionalidade, TipoDocumento, layout, slug};
use serde::Serialize;

use crate::evidencias::ArquivoDoProjeto;
pub use crate::evidencias::hash_do_arquivo;
use crate::registro::ErroFerramenta;

/// O que o histórico registra para o documento que já estava na base quando o
/// histórico de revisões chegou.
pub const MUDANCAS_DO_IMPORTADO: &str = "Versão anterior ao histórico de revisões";

// ---------------------------------------------------------------------------
// Estado

/// Hash atual de um arquivo do projeto; `None` se não pôde ser lido (sumiu,
/// saiu do escopo, ficou binário).
fn hash_atual(escopo: &Escopo, arquivo: &str) -> Option<String> {
    ArquivoDoProjeto::abrir(escopo, arquivo)
        .ok()
        .map(|a| hash_do_arquivo(&a.texto))
}

/// O arquivo citado mudou (ou não pôde mais ser lido) desde a publicação.
pub fn arquivo_mudou(escopo: &Escopo, citado: &ArquivoCitado) -> bool {
    hash_atual(escopo, &citado.arquivo).as_deref() != Some(citado.hash.as_str())
}

/// Os arquivos, com o hash de agora. Os que não podem ser lidos ficam de fora e
/// voltam nos avisos.
pub fn citar(escopo: &Escopo, arquivos: &[String]) -> (Vec<ArquivoCitado>, Vec<String>) {
    let mut citados = Vec::new();
    let mut avisos = Vec::new();
    for arquivo in arquivos {
        match ArquivoDoProjeto::abrir(escopo, arquivo) {
            Ok(a) => citados.push(ArquivoCitado {
                arquivo: a.relativo.as_str().to_owned(),
                hash: hash_do_arquivo(&a.texto),
            }),
            Err(motivo) => avisos.push(format!(
                "`{arquivo}` não entrou nos arquivos citados ({motivo}); a mudança nele não \
                 marca o documento como desatualizado"
            )),
        }
    }
    (citados, avisos)
}

/// O estado de um documento (ou da falta dele).
pub fn estado(escopo: &Escopo, documento: Option<&DocumentoVivo>) -> EstadoDocumento {
    match documento {
        Some(d) => d.estado(|a| arquivo_mudou(escopo, a)),
        None => EstadoDocumento::SemDocumento,
    }
}

/// Um tipo vivo numa funcionalidade: o documento, a versão e o estado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DocumentoDoTipo {
    pub tipo: String,
    /// Caminho na base (o do documento vivo ou, sem ele, o vinculado à
    /// funcionalidade).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caminho: Option<CaminhoRelativo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub versao: Option<Versao>,
    pub estado: EstadoDocumento,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acao: Option<&'static str>,
}

/// Os documentos dos tipos `tipos` de uma funcionalidade, na ordem pedida. Um
/// documento vinculado que ainda não tem histórico (o negocial de antes) conta
/// como em dia, na versão 1.0.
pub fn documentos_da_funcionalidade(
    escopo: &Escopo,
    estado_dos_documentos: &DocumentosVivos,
    funcionalidade: &Funcionalidade,
    tipos: &[&str],
) -> Vec<DocumentoDoTipo> {
    let dono = Dono::Funcionalidade(funcionalidade.id.clone());
    tipos
        .iter()
        .map(|&tipo| {
            let vivo = estado_dos_documentos.do_dono(tipo, &dono);
            let vinculado = funcionalidade.documento(tipo).cloned();
            let est = match (vivo, &vinculado) {
                (None, Some(_)) => EstadoDocumento::EmDia {
                    versao: Versao::INICIAL,
                },
                _ => estado(escopo, vivo),
            };
            let versao = match &est {
                EstadoDocumento::EmDia { versao }
                | EstadoDocumento::Desatualizado { versao, .. } => Some(*versao),
                _ => vivo.and_then(DocumentoVivo::versao_publicada),
            };
            DocumentoDoTipo {
                tipo: tipo.to_owned(),
                caminho: vivo.map(|d| d.caminho.clone()).or(vinculado),
                versao,
                acao: est.acao(),
                estado: est,
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Proposta

/// Pedido de proposta de versão de um documento vivo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PedidoDeProposta {
    /// Id do tipo no catálogo de entregáveis.
    pub tipo: String,
    pub dono: Dono,
    /// Caminho na base; sem ele, o do documento que já existe ou o padrão.
    pub caminho: Option<CaminhoRelativo>,
    pub proposta: NovaProposta,
}

/// Proposta registrada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PropostaRegistrada {
    pub caminho: CaminhoRelativo,
    pub proposta: Proposta,
}

fn interno(e: impl std::fmt::Display) -> ErroFerramenta {
    ErroFerramenta::Interno(e.to_string())
}

impl From<cba_core::documentos_vivos::ErroArquivoDocumentos> for ErroFerramenta {
    fn from(e: cba_core::documentos_vivos::ErroArquivoDocumentos) -> Self {
        ErroFerramenta::Interno(e.to_string())
    }
}

fn recusa(e: ErroDocumento) -> ErroFerramenta {
    ErroFerramenta::Recusa(e.to_string())
}

/// O tipo do catálogo, se for vivo.
fn tipo_vivo(tipo: &str) -> Result<&'static entregaveis::TipoEntregavel, ErroFerramenta> {
    let t = entregaveis::tipo(tipo).ok_or_else(|| {
        ErroFerramenta::Argumentos(format!(
            "o tipo `{tipo}` não está no catálogo de entregáveis"
        ))
    })?;
    if t.ciclo != CicloDeVida::Vivo {
        return Err(ErroFerramenta::Recusa(format!(
            "{} é um entregável por demanda, não um documento vivo: não tem versões",
            t.nome
        )));
    }
    Ok(t)
}

/// A funcionalidade aberta do mapa.
fn funcionalidade_aberta(
    mapa: &MapaFuncionalidades,
    id: &cba_kb::IdFuncionalidade,
) -> Result<Funcionalidade, ErroFerramenta> {
    mapa.funcionalidade(id)
        .filter(|f| f.aberta())
        .cloned()
        .ok_or_else(|| {
            ErroFerramenta::Recusa(format!(
                "a funcionalidade {id} não está no mapa (ou foi descartada ou juntada)"
            ))
        })
}

/// Caminho padrão do documento: `funcionalidades/<area>/<tema>/<tipo>.md` para
/// a funcionalidade, `areas/<area>/<tipo>.md` para a área.
pub fn caminho_padrao(
    tipo: &str,
    dono: &Dono,
    mapa: &MapaFuncionalidades,
) -> Result<CaminhoRelativo, ErroFerramenta> {
    let arquivo = format!("{}.{}", slug(tipo), layout::EXTENSAO);
    match dono {
        Dono::Funcionalidade(id) => {
            let f = funcionalidade_aberta(mapa, id)?;
            layout::funcionalidade(&f.area, &f.nome)
                .and_then(|p| p.juntar(&arquivo))
                .map_err(interno)
        }
        Dono::Area(area) => {
            let area = slug(area);
            if area.is_empty() {
                return Err(ErroFerramenta::Argumentos(
                    "a área não pode ficar vazia".into(),
                ));
            }
            CaminhoRelativo::novo(&format!("{}/{area}/{arquivo}", layout::AREAS)).map_err(interno)
        }
    }
}

/// O texto de um arquivo da base, sem o frontmatter; `None` se ele não existe.
fn corpo_na_base(ambiente: &Ambiente, caminho: &CaminhoRelativo) -> Option<String> {
    corpo_no_projeto(&ambiente.kb, caminho)
}

fn corpo_no_projeto(kb: &cba_kb::Projeto, caminho: &CaminhoRelativo) -> Option<String> {
    let texto = std::fs::read_to_string(kb.caminho(caminho)).ok()?;
    Some(separar(&texto).1.trim().to_owned())
}

// ---------------------------------------------------------------------------
// Documento negocial de antes do histórico (#465)

/// Documento negocial vinculado a uma funcionalidade aberta que ainda não é um
/// documento vivo: a migração o importa como v1.0.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NegocialAImportar {
    pub funcionalidade: IdFuncionalidade,
    pub caminho: CaminhoRelativo,
}

/// Os documentos negociais (o "Explicar" de antes do catálogo) vinculados a uma
/// funcionalidade aberta, com texto na base, e ainda sem documento vivo do tipo
/// nem outro documento vivo no mesmo caminho. Só lê.
pub fn negociais_sem_historico(kb: &cba_kb::Projeto) -> Result<Vec<NegocialAImportar>, String> {
    let mapa = MapaFuncionalidades::carregar(kb).map_err(|e| e.to_string())?;
    let docs = DocumentosVivos::carregar(kb).map_err(|e| e.to_string())?;
    Ok(pendentes(kb, &mapa, &docs)
        .into_iter()
        .map(|(f, _)| NegocialAImportar {
            funcionalidade: f.id.clone(),
            caminho: f
                .documento_negocial()
                .cloned()
                .expect("filtrada pelo vínculo"),
        })
        .collect())
}

fn pendentes<'a>(
    kb: &cba_kb::Projeto,
    mapa: &'a MapaFuncionalidades,
    docs: &DocumentosVivos,
) -> Vec<(&'a Funcionalidade, String)> {
    mapa.funcionalidades
        .iter()
        .filter(|f| f.aberta())
        .filter_map(|f| {
            let caminho = f.documento_negocial()?;
            let dono = Dono::Funcionalidade(f.id.clone());
            if docs
                .do_dono(entregaveis::DOCUMENTO_NEGOCIAL, &dono)
                .is_some()
                || docs.documento(caminho).is_some()
            {
                return None;
            }
            let texto = corpo_no_projeto(kb, caminho).filter(|t| !t.is_empty())?;
            Some((f, texto))
        })
        .collect()
}

/// Importa como v1.0 de um documento vivo do tipo "Documento negocial" cada
/// documento de [`negociais_sem_historico`]: o texto do arquivo (sem o
/// frontmatter) vira a versão, com [`MUDANCAS_DO_IMPORTADO`], a explicação de
/// origem e o dia em que o arquivo foi gravado. O arquivo na base não muda.
/// Idempotente: sem pendência, não grava nada. Devolve os importados.
pub fn importar_negociais(kb: &cba_kb::Projeto) -> Result<Vec<NegocialAImportar>, String> {
    if negociais_sem_historico(kb)?.is_empty() {
        return Ok(Vec::new());
    }
    let mapa = MapaFuncionalidades::carregar(kb).map_err(|e| e.to_string())?;
    let (_, importados) = DocumentosVivos::alterar(kb, |docs, agora| {
        let mut importados = Vec::new();
        for (f, texto) in pendentes(kb, &mapa, docs) {
            let caminho = f
                .documento_negocial()
                .cloned()
                .expect("filtrada pelo vínculo");
            let gravado_ms = std::fs::metadata(kb.caminho(&caminho))
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(agora, |d| d.as_millis() as u64);
            let mut doc = DocumentoVivo::importado(
                entregaveis::DOCUMENTO_NEGOCIAL,
                Dono::Funcionalidade(f.id.clone()),
                caminho.clone(),
                &texto,
                MUDANCAS_DO_IMPORTADO,
                cba_core::tempo::data_utc(gravado_ms),
                agora,
            );
            if let Some(r) = doc.historico.first_mut() {
                r.analise = f.explicacao.clone();
            }
            if docs.importar(doc).map_err(recusa)? {
                importados.push(NegocialAImportar {
                    funcionalidade: f.id.clone(),
                    caminho,
                });
            }
        }
        Ok::<_, ErroFerramenta>(importados)
    })
    .map_err(|e| e.to_string())?;
    Ok(importados)
}

/// Registra a proposta da versão seguinte (ou da 1.0) do documento. Uma proposta
/// pendente é substituída e volta a rascunho.
pub fn propor(
    ambiente: &Ambiente,
    pedido: PedidoDeProposta,
) -> Result<PropostaRegistrada, ErroFerramenta> {
    let tipo = tipo_vivo(&pedido.tipo)?.id;
    let mapa = MapaFuncionalidades::carregar(&ambiente.kb).map_err(interno)?;
    let vinculado = match &pedido.dono {
        Dono::Funcionalidade(id) => funcionalidade_aberta(&mapa, id)?.documento(tipo).cloned(),
        Dono::Area(_) => None,
    };
    let dono = match pedido.dono {
        Dono::Area(a) => Dono::Area(slug(&a)),
        d => d,
    };
    // O caminho é decidido sob a trava, com o estado que vale: o do documento
    // que já existe, o pedido, o vinculado à funcionalidade ou o padrão.
    let (_, registrada) = DocumentosVivos::alterar(&ambiente.kb, |docs, agora| {
        let caminho = match pedido
            .caminho
            .or_else(|| docs.do_dono(tipo, &dono).map(|d| d.caminho.clone()))
            .or(vinculado)
        {
            Some(c) => c,
            None => caminho_padrao(tipo, &dono, &mapa)?,
        };
        if docs.documento(&caminho).is_none()
            && docs.do_dono(tipo, &dono).is_none()
            && let Some(texto) = corpo_na_base(ambiente, &caminho).filter(|t| !t.is_empty())
        {
            docs.importar(DocumentoVivo::importado(
                tipo,
                dono.clone(),
                caminho.clone(),
                &texto,
                MUDANCAS_DO_IMPORTADO,
                cba_core::tempo::data_utc(agora),
                agora,
            ))
            .map_err(recusa)?;
        }
        docs.garantir(tipo, &dono, &caminho, agora)
            .map_err(recusa)?;
        let proposta = docs
            .propor(&caminho, pedido.proposta, agora)
            .map_err(recusa)?;
        Ok::<_, ErroFerramenta>(PropostaRegistrada { caminho, proposta })
    })?;
    Ok(registrada)
}

/// Edição do analista na proposta: os trechos são marcados de novo e ela volta
/// a rascunho.
pub fn editar_proposta(
    ambiente: &Ambiente,
    caminho: &CaminhoRelativo,
    texto: &str,
    mudancas: Option<&str>,
) -> Result<Proposta, ErroFerramenta> {
    let (_, p) = DocumentosVivos::alterar(&ambiente.kb, |docs, agora| {
        docs.editar_proposta(caminho, texto, mudancas, agora)
            .map_err(recusa)
    })?;
    Ok(p)
}

/// Revisão final da proposta.
pub fn marcar_revisada(
    ambiente: &Ambiente,
    caminho: &CaminhoRelativo,
    por: &str,
) -> Result<Proposta, ErroFerramenta> {
    let (_, p) = DocumentosVivos::alterar(&ambiente.kb, |docs, agora| {
        docs.marcar_revisada(caminho, por, agora).map_err(recusa)
    })?;
    Ok(p)
}

/// Descarta a proposta; o documento fica na versão publicada.
pub fn descartar_proposta(
    ambiente: &Ambiente,
    caminho: &CaminhoRelativo,
) -> Result<(), ErroFerramenta> {
    DocumentosVivos::alterar(&ambiente.kb, |docs, _| {
        docs.descartar_proposta(caminho).map_err(recusa)
    })?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Publicação

/// Versão publicada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Publicacao {
    pub caminho: CaminhoRelativo,
    pub revisao: Revisao,
    /// Versões no histórico, com a publicada.
    pub versoes: usize,
    pub avisos: Vec<String>,
}

/// Separa o frontmatter (do início até a linha `---` de fechamento, inclusive)
/// do corpo. Sem frontmatter, o cabeçalho é `None`.
fn separar(texto: &str) -> (Option<&str>, &str) {
    let sem_bom = texto.strip_prefix('\u{feff}').unwrap_or(texto);
    let Some(resto) = sem_bom
        .strip_prefix("---\n")
        .or_else(|| sem_bom.strip_prefix("---\r\n"))
    else {
        return (None, texto);
    };
    let mut pos = texto.len() - resto.len();
    while pos < texto.len() {
        let fim_da_linha = texto[pos..].find('\n').map_or(texto.len(), |i| pos + i + 1);
        if texto[pos..fim_da_linha].trim_end() == "---" {
            return (Some(&texto[..fim_da_linha]), &texto[fim_da_linha..]);
        }
        pos = fim_da_linha;
    }
    (None, texto)
}

/// O tipo de documento da base para o frontmatter de um documento vivo novo.
fn tipo_do_documento(tipo: &str) -> TipoDocumento {
    match tipo {
        entregaveis::CASO_DE_USO => TipoDocumento::CasoDeUso,
        entregaveis::MANUAL => TipoDocumento::Manual,
        _ => TipoDocumento::Funcionalidade,
    }
}

/// Grava o corpo publicado no arquivo: o frontmatter que havia fica; sem
/// arquivo, ele nasce com o frontmatter de funcionalidade.
fn gravar_publicado(
    ambiente: &Ambiente,
    documento: &DocumentoVivo,
    mapa: &MapaFuncionalidades,
) -> Result<(), ErroFerramenta> {
    let corpo = documento
        .markdown_publicado()
        .expect("só grava o que tem versão publicada");
    let destino = ambiente.kb.caminho(&documento.caminho);
    if let Ok(texto) = std::fs::read_to_string(&destino)
        && let (Some(cabecalho), _) = separar(&texto)
    {
        let novo = format!("{cabecalho}\n{corpo}");
        return cba_kb::arquivo::gravar(&destino, novo.as_bytes()).map_err(interno);
    }
    let nome = entregaveis::tipo(&documento.tipo).map_or(documento.tipo.as_str(), |t| t.nome);
    let (titulo, area, tema, funcionalidade) = match &documento.dono {
        Dono::Funcionalidade(id) => {
            let f = mapa.funcionalidade(id);
            (
                format!("{nome} · {}", f.map_or(id.to_string(), |f| f.nome.clone())),
                f.map(|f| f.area.clone()).unwrap_or_default(),
                f.map_or(id.to_string(), |f| slug(&f.nome)),
                Some(id.clone()),
            )
        }
        Dono::Area(a) => (
            format!("{nome} · {a}"),
            a.clone(),
            slug(&documento.tipo),
            None,
        ),
    };
    let doc = Documento::novo(
        FrontmatterFuncionalidade {
            tipo: tipo_do_documento(&documento.tipo),
            titulo,
            area,
            tema,
            status: StatusAnalise::Aprovada,
            origem: None,
            funcionalidade,
        },
        format!("\n{corpo}"),
    );
    ambiente
        .kb
        .gravar(&documento.caminho, &doc)
        .map_err(interno)
}

/// Publica a proposta revisada: a revisão entra no histórico (as versões
/// anteriores continuam acessíveis), o arquivo na base ganha o texto novo e a
/// tabela "Histórico de revisões", e o documento fica vinculado à funcionalidade.
pub fn publicar(
    ambiente: &Ambiente,
    caminho: &CaminhoRelativo,
) -> Result<Publicacao, ErroFerramenta> {
    let mapa = MapaFuncionalidades::carregar(&ambiente.kb).map_err(interno)?;
    let (docs, (revisao, avisos)) = DocumentosVivos::alterar(&ambiente.kb, |docs, agora| {
        let arquivos = docs
            .documento(caminho)
            .and_then(|d| d.proposta.as_ref())
            .map(|p| p.arquivos.clone())
            .unwrap_or_default();
        let (citados, avisos) = citar(&ambiente.escopo, &arquivos);
        let revisao = docs
            .publicar(caminho, cba_core::tempo::data_utc(agora), citados, agora)
            .map_err(recusa)?;
        let doc = docs.documento(caminho).expect("acabou de publicar");
        gravar_publicado(ambiente, doc, &mapa)?;
        Ok::<_, ErroFerramenta>((revisao, avisos))
    })?;
    let doc = docs.documento(caminho).expect("publicado");
    if let Dono::Funcionalidade(id) = &doc.dono {
        let vinculo = MapaFuncionalidades::alterar(&ambiente.kb, |m, agora| {
            m.vincular_documento(id, &doc.tipo, caminho.clone(), agora)
        });
        if let Err(e) = vinculo {
            return Err(ErroFerramenta::Interno(format!(
                "a versão {} foi publicada, mas o vínculo com a funcionalidade falhou: {e}",
                revisao.versao
            )));
        }
    }
    Ok(Publicacao {
        caminho: caminho.clone(),
        versoes: doc.historico.len(),
        revisao,
        avisos,
    })
}

/// O texto de uma versão do documento (a publicada ou uma anterior).
pub fn texto_da_versao(
    estado_dos_documentos: &DocumentosVivos,
    caminho: &CaminhoRelativo,
    versao: Versao,
) -> Option<String> {
    estado_dos_documentos
        .documento(caminho)?
        .versao(versao)
        .map(|r| sem_historico(&r.texto))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separa_o_frontmatter_do_corpo() {
        let (c, corpo) = separar("---\ntipo: funcionalidade\n---\n# Título\n\nTexto\n");
        assert_eq!(c, Some("---\ntipo: funcionalidade\n---\n"));
        assert_eq!(corpo, "# Título\n\nTexto\n");
        assert_eq!(separar("# Sem\n"), (None, "# Sem\n"));
        assert_eq!(separar("---\nsem fechamento\n").0, None);
    }

    #[test]
    fn hash_do_arquivo_ignora_espacos_e_fim_de_linha() {
        assert_eq!(hash_do_arquivo("a  b\r\nc\n"), hash_do_arquivo("  a b\nc"));
        assert_ne!(hash_do_arquivo("a\nb"), hash_do_arquivo("a\nc"));
    }
}
