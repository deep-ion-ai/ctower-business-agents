//! Publicação da documentação aprovada no repositório (#276, #289).
//!
//! A base aprovada vira as páginas legíveis do renderizador
//! ([`crate::documentacao`], #288: README, páginas por funcionalidade, regras por
//! área, glossário e análises, com os marcadores de ida e volta) e elas vão para
//! a pasta do destino da documentação do projeto ([`cba_repos::DestinoDocs`]),
//! numa branch própria e com PR/MR ([`cba_repos::escrita`]).
//!
//! - **Só o aprovado** ([`aprovado`]): análises, documentos de funcionalidade e
//!   mapa aprovados, regras confirmadas, termos do glossário e sugestões aceitas.
//!   Demandas, atas, análises de reunião, registros da demanda, pessoas, memórias
//!   e o `projeto.md` ficam de fora: são trabalho em andamento ou trazem dados
//!   pessoais e caminhos locais.
//! - **Manifesto** ([`MANIFESTO`], versão [`VERSAO_DO_MANIFESTO`]): a pasta
//!   publicada leva a lista das páginas que o app publicou. Só os arquivos
//!   listados nele são removidos quando deixam de ser gerados; um arquivo que já
//!   existe na pasta sem estar no manifesto (escrito à mão) nunca é apagado nem
//!   sobrescrito: a publicação para com um conflito. O manifesto da versão 1
//!   (#276) listava os `.md` da base copiados como estavam: publicar por cima
//!   dele remove esses arquivos e grava as páginas.
//! - **Prévia** ([`previa`]): o que muda na branch base do destino, por documento
//!   ("2 regras novas, 1 alterada; 1 termo novo", comparando os blocos marcados
//!   das páginas, [`cba_docs::marcador`]) e por página, lido do remoto sem tocar
//!   no checkout do analista. Com PR/MR pendente, diz também se a branch da
//!   publicação já tem esta versão ([`Previa::branch_atualizada`]).
//! - **Idempotente**: sem mudança, não há commit nem PR/MR
//!   ([`Situacao::SemMudancas`]). Por isso o README não leva a data da
//!   publicação, e sim a da última mudança registrada nos documentos aprovados
//!   ([`data_da_base`]).
//! - **Branch determinística por projeto** ([`branch_da_publicacao`]):
//!   `docs/cba-<projeto>`, conferida por [`validar_branch_de_escrita`].

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use cba_docs::LinksDeEvidencia;
use cba_index::Escopo;
use cba_kb::frontmatter::FrontmatterLivre;
use cba_kb::{Documento, Projeto, TipoDocumento};
use cba_repos::chaveiro::token_do_repositorio;
use cba_repos::demanda::Ferramentas;
use cba_repos::docs::normalizar_subpasta;
use cba_repos::escrita::{
    self, LeitorDeCommit, Leitura, LeituraRemota, Mudanca, Pedido, Publicado, Situacao,
    validar_branch_de_escrita,
};
use cba_repos::{DestinoResolvido, Repositorios, Secret};
use serde::{Deserialize, Serialize};

#[path = "publicacao_documentos.rs"]
mod documentos;
pub use documentos::{MudancaDeDocumento, PaginaLida, ResumoDosDocumentos, comparar_documentos};

/// Manifesto da publicação, na pasta publicada: as páginas que o app publicou
/// lá (relativas à pasta). Começa com `.` para ficar fora das listas de
/// documentos.
pub const MANIFESTO: &str = ".cba-publicacao.json";

/// Versão do manifesto: 1 (#276) listava os `.md` da base em `documentos`; 2
/// (#289) lista as páginas legíveis em `paginas`.
pub const VERSAO_DO_MANIFESTO: u32 = 2;

/// Página de entrada da documentação publicada.
pub const LEIA_ME: &str = cba_docs::README;

/// Quem gera o manifesto.
const GERADO_POR: &str = "ctower-business-agents";

/// Quantos itens o corpo do PR/MR lista por tipo de mudança.
const MAX_NO_CORPO: usize = 30;

/// Branch da publicação do projeto: `docs/cba-<projeto>`.
pub fn branch_da_publicacao(projeto: &str) -> String {
    format!("docs/cba-{projeto}")
}

/// Se o documento (texto com frontmatter) está aprovado para publicação. Sem
/// frontmatter legível ou de tipo desconhecido, não.
pub fn aprovado(texto: &str) -> bool {
    let Ok(doc) = Documento::<FrontmatterLivre>::analisar(texto) else {
        return false;
    };
    let Some(tipo) = doc.frontmatter.tipo() else {
        return false;
    };
    let status = doc
        .frontmatter
        .0
        .get("status")
        .and_then(|v| v.as_str())
        .map(str::trim);
    use TipoDocumento as T;
    match tipo {
        // O glossário só recebe termos aceitos na aprovação da análise.
        T::Termo => true,
        T::Regra => status == Some("confirmada"),
        T::Sugestao => status == Some("aceita"),
        T::AnaliseImpacto
        | T::Pergunta
        | T::ParecerRevisao
        | T::Historias
        | T::CenariosTeste
        | T::Prototipo
        | T::Funcionalidade
        | T::CasoDeUso
        | T::MapaFuncionalidades => status == Some("aprovada"),
        // Trabalho em andamento, dados pessoais (pessoas, participantes das
        // reuniões) ou configuração local.
        T::Projeto
        | T::MemoriaArea
        | T::Demanda
        | T::Pessoa
        | T::Ata
        | T::AnaliseReuniao
        | T::Premissa
        | T::Decisao
        | T::Risco
        | T::Restricao => false,
        // A documentação externa (#277) volta do repositório de docs: publicá-la
        // de novo duplicaria o que já está lá.
        T::DocExterno => false,
        // O manual do usuário (#272) leva os prints, que a documentação legível
        // ainda não publica: fica de fora até as páginas levarem as imagens.
        T::Manual => false,
    }
}

/// Um documento aprovado da base, com o caminho relativo à raiz do projeto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentoAprovado {
    pub caminho: String,
    pub conteudo: Vec<u8>,
}

/// Os documentos aprovados do projeto, em ordem, com o conteúdo como está no
/// arquivo.
pub fn documentos_aprovados(projeto: &Projeto) -> Result<Vec<DocumentoAprovado>, String> {
    let mut saida = Vec::new();
    for relativo in projeto.documentos().map_err(|e| e.to_string())? {
        let conteudo = std::fs::read(projeto.caminho(&relativo))
            .map_err(|e| format!("não foi possível ler {relativo}: {e}"))?;
        if std::str::from_utf8(&conteudo).is_ok_and(aprovado) {
            saida.push(DocumentoAprovado {
                caminho: relativo.as_str().to_string(),
                conteudo,
            });
        }
    }
    Ok(saida)
}

/// Data (AAAA-MM-DD) da última mudança registrada nos documentos aprovados: a
/// maior entre os campos `criado`, `atualizado` e `data` dos frontmatters.
/// Determinística (depende só da base), para o README não mudar a cada
/// publicação; `None` se nenhum documento traz data.
pub fn data_da_base(documentos: &[DocumentoAprovado]) -> Option<String> {
    let mut maior: Option<String> = None;
    for d in documentos {
        let Some(doc) = std::str::from_utf8(&d.conteudo)
            .ok()
            .and_then(|t| Documento::<FrontmatterLivre>::analisar(t).ok())
        else {
            continue;
        };
        for campo in ["criado", "atualizado", "data"] {
            let Some(v) = doc.frontmatter.0.get(campo).and_then(|v| v.as_str()) else {
                continue;
            };
            let Some(dia) = v.trim().get(..10).filter(|d| e_data(d)) else {
                continue;
            };
            if maior.as_deref().is_none_or(|m| dia > m) {
                maior = Some(dia.to_owned());
            }
        }
    }
    maior
}

/// `AAAA-MM-DD`, só dígitos e hífens nos lugares.
fn e_data(t: &str) -> bool {
    t.len() == 10
        && t.bytes().enumerate().all(|(i, b)| {
            if i == 4 || i == 7 {
                b == b'-'
            } else {
                b.is_ascii_digit()
            }
        })
}

/// Página da documentação legível a publicar, com o caminho relativo à pasta
/// da documentação (`README.md`, `regras/cobranca.md`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaginaPublicada {
    pub caminho: String,
    pub conteudo: Vec<u8>,
}

/// As páginas geradas da base aprovada.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Paginas {
    pub paginas: Vec<PaginaPublicada>,
    /// Documentos aprovados na base.
    pub aprovados: usize,
    /// Documentos que o renderizador deixou de fora (frontmatter ilegível...).
    pub avisos: Vec<String>,
}

/// Gera as páginas legíveis da base aprovada. Sem documento aprovado, nenhuma
/// página (nem o README): não há o que publicar.
pub fn paginas_do_projeto(
    projeto: &Projeto,
    links: Option<&dyn LinksDeEvidencia>,
) -> Result<Paginas, String> {
    let documentos = documentos_aprovados(projeto)?;
    let aprovados = documentos.len();
    if aprovados == 0 {
        return Ok(Paginas::default());
    }
    let data = data_da_base(&documentos);
    let d = crate::documentacao::documentacao_dos_aprovados(projeto, documentos, links, data)?;
    Ok(Paginas {
        paginas: d
            .paginas
            .into_iter()
            .map(|p| PaginaPublicada {
                caminho: p.caminho,
                conteudo: p.conteudo.into_bytes(),
            })
            .collect(),
        aprovados,
        avisos: d.avisos,
    })
}

/// Tipo de mudança na prévia.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TipoMudanca {
    Novo,
    Alterado,
    Removido,
}

/// Um arquivo que muda na branch base, com o caminho relativo à raiz do
/// repositório.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MudancaPrevista {
    pub caminho: String,
    pub tipo: TipoMudanca,
}

/// O que publicar: as mudanças para a escrita e a prévia em relação à base.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plano {
    /// Todas as páginas, o manifesto e as remoções; vazio quando não há nada
    /// publicado nem a publicar.
    pub mudancas: Vec<Mudanca>,
    /// Páginas novas, alteradas e removidas em relação à branch base (sem o
    /// manifesto).
    pub previa: Vec<MudancaPrevista>,
    /// A publicação muda algo na branch base (inclusive só o manifesto).
    pub muda_a_base: bool,
    /// A branch da publicação existe no remoto com a pasta diferente da base
    /// (PR/MR pendente).
    pub pendente_na_branch: bool,
    /// A branch pendente já tem exatamente o que seria publicado: o PR/MR já
    /// tem esta versão.
    pub branch_atualizada: bool,
    /// A base tem uma publicação no formato antigo (manifesto versão 1).
    pub formato_antigo: bool,
    /// Arquivos que o app publicou na branch base (pelo manifesto de lá),
    /// relativos à raiz do repositório.
    pub publicados_na_base: BTreeSet<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Manifesto {
    /// Ausente no manifesto da versão 1.
    #[serde(default = "versao_1")]
    versao: u32,
    gerado_por: String,
    projeto: String,
    /// Versão 1: os `.md` da base publicados como estavam.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    documentos: Vec<String>,
    /// Versão 2: as páginas legíveis.
    #[serde(default)]
    paginas: Vec<String>,
}

fn versao_1() -> u32 {
    1
}

/// O que o manifesto de uma branch diz que o app publicou.
#[derive(Debug, Default)]
struct Publicados {
    /// Caminhos relativos à raiz do repositório.
    arquivos: BTreeSet<String>,
    /// Manifesto da versão 1.
    formato_antigo: bool,
}

/// Lê o manifesto de uma branch. Recusa o manifesto de outro projeto ou de uma
/// versão mais nova do app.
fn publicados(
    conteudo: Option<&Vec<u8>>,
    caminho: &str,
    projeto: &str,
    destino: &DestinoResolvido<'_>,
) -> Result<Publicados, String> {
    let Some(conteudo) = conteudo else {
        return Ok(Publicados::default());
    };
    let m: Manifesto = serde_json::from_slice(conteudo).map_err(|e| {
        format!(
            "o manifesto da publicação ({caminho}) está ilegível no repositório: {e}; corrija ou \
             remova o arquivo antes de publicar"
        )
    })?;
    if m.projeto != projeto {
        return Err(format!(
            "a pasta {} já recebe a publicação do projeto {}: escolha outra pasta para o \
             destino da documentação",
            if destino.subpasta.is_empty() {
                "raiz do repositório"
            } else {
                destino.subpasta
            },
            m.projeto
        ));
    }
    if m.versao > VERSAO_DO_MANIFESTO {
        return Err(format!(
            "a documentação em {caminho} foi publicada por uma versão mais nova do app \
             (manifesto versão {}): atualize o app antes de publicar",
            m.versao
        ));
    }
    // Só caminhos válidos e dentro da pasta: um manifesto editado não apaga nada
    // fora dela.
    Ok(Publicados {
        arquivos: m
            .documentos
            .iter()
            .chain(&m.paginas)
            .filter_map(|d| normalizar_subpasta(d).ok().filter(|d| !d.is_empty()))
            .filter(|d| d != MANIFESTO)
            .map(|d| destino.caminho_no_repositorio(&d))
            .collect(),
        formato_antigo: m.versao < VERSAO_DO_MANIFESTO,
    })
}

/// O manifesto (versão atual) das páginas publicadas.
fn manifesto_das_paginas(projeto: &str, paginas: &[PaginaPublicada]) -> Vec<u8> {
    let m = Manifesto {
        versao: VERSAO_DO_MANIFESTO,
        gerado_por: GERADO_POR.into(),
        projeto: projeto.into(),
        documentos: Vec::new(),
        paginas: paginas.iter().map(|p| p.caminho.clone()).collect(),
    };
    let mut t = serde_json::to_string_pretty(&m).expect("manifesto serializável");
    t.push('\n');
    t.into_bytes()
}

/// Calcula as mudanças da publicação comparando as páginas geradas com o que
/// está no remoto (sem rede: a leitura já foi feita).
pub fn planejar(
    projeto: &str,
    paginas: &[PaginaPublicada],
    destino: &DestinoResolvido<'_>,
    remoto: &LeituraRemota,
) -> Result<Plano, String> {
    let manifesto = destino.caminho_no_repositorio(MANIFESTO);
    let base = &remoto.na_base;
    let na_base = publicados(base.conteudos.get(&manifesto), &manifesto, projeto, destino)?;
    let mut anteriores = na_base.arquivos.clone();
    if let Some(b) = &remoto.na_branch {
        anteriores.extend(
            publicados(b.conteudos.get(&manifesto), &manifesto, projeto, destino)?.arquivos,
        );
    }
    let id = |c: &[u8]| remoto.formato.id_do_blob(c);

    let novos: BTreeMap<String, &[u8]> = paginas
        .iter()
        .map(|p| {
            (
                destino.caminho_no_repositorio(&p.caminho),
                p.conteudo.as_slice(),
            )
        })
        .collect();

    // Arquivo que já está na base sem ter sido publicado pelo app: não é nosso.
    let conflitos: Vec<&str> = novos
        .iter()
        .filter(|(c, conteudo)| {
            !anteriores.contains(*c) && base.arquivos.get(*c).is_some_and(|b| *b != id(conteudo))
        })
        .map(|(c, _)| c.as_str())
        .collect();
    if !conflitos.is_empty() {
        return Err(format!(
            "já existem no repositório, sem terem sido publicados pelo app: {}. Para não \
             sobrescrever documentos escritos à mão, mova-os ou escolha outra pasta para o \
             destino da documentação",
            conflitos.join(", ")
        ));
    }

    let mut previa = Vec::new();
    for (caminho, conteudo) in &novos {
        match base.arquivos.get(caminho) {
            None => previa.push(MudancaPrevista {
                caminho: caminho.clone(),
                tipo: TipoMudanca::Novo,
            }),
            Some(b) if *b != id(conteudo) => previa.push(MudancaPrevista {
                caminho: caminho.clone(),
                tipo: TipoMudanca::Alterado,
            }),
            Some(_) => {}
        }
    }
    let removidos: Vec<&String> = anteriores
        .iter()
        .filter(|c| !novos.contains_key(*c))
        .collect();
    previa.extend(
        removidos
            .iter()
            .filter(|c| base.arquivos.contains_key(**c))
            .map(|c| MudancaPrevista {
                caminho: (*c).clone(),
                tipo: TipoMudanca::Removido,
            }),
    );
    previa.sort_by(|a, b| (a.tipo, &a.caminho).cmp(&(b.tipo, &b.caminho)));

    let pendente_na_branch = remoto
        .na_branch
        .as_ref()
        .is_some_and(|b| b.arquivos != base.arquivos && b.commit != base.commit);

    if novos.is_empty() && anteriores.is_empty() {
        return Ok(Plano {
            mudancas: Vec::new(),
            previa,
            muda_a_base: false,
            pendente_na_branch,
            branch_atualizada: false,
            formato_antigo: na_base.formato_antigo,
            publicados_na_base: na_base.arquivos,
        });
    }
    let texto_manifesto = manifesto_das_paginas(projeto, paginas);
    let manifesto_muda = base.arquivos.get(&manifesto) != Some(&id(&texto_manifesto));
    let mut mudancas: Vec<Mudanca> = novos
        .iter()
        .map(|(c, conteudo)| Mudanca::gravar(c.clone(), conteudo.to_vec()))
        .collect();
    mudancas.push(Mudanca::gravar(manifesto, texto_manifesto));
    mudancas.extend(removidos.into_iter().map(|c| Mudanca::remover(c.clone())));

    // A ponta da branch pendente já tem tudo o que seria gravado e nada do que
    // seria removido: o PR/MR já tem esta versão.
    let branch_atualizada = pendente_na_branch
        && remoto.na_branch.as_ref().is_some_and(|b| {
            mudancas.iter().all(|m| match m {
                Mudanca::Gravar { caminho, conteudo } => {
                    b.arquivos.get(caminho) == Some(&id(conteudo))
                }
                Mudanca::Remover { caminho } => !b.arquivos.contains_key(caminho),
            })
        });
    Ok(Plano {
        mudancas,
        muda_a_base: manifesto_muda || !previa.is_empty(),
        previa,
        pendente_na_branch,
        branch_atualizada,
        formato_antigo: na_base.formato_antigo,
        publicados_na_base: na_base.arquivos,
    })
}

/// Prévia da publicação, para o analista confirmar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Previa {
    /// Onde a documentação é publicada (`docs/negocio em GitHub dono/repo`).
    pub destino: String,
    /// Repositório onde se escreve.
    pub repositorio: String,
    /// Repositório próprio da documentação (e não uma pasta do código).
    pub independente: bool,
    /// Branch base (resolvida) e branch da publicação.
    pub base: String,
    pub branch: String,
    /// Documentos aprovados na base.
    pub aprovados: usize,
    /// Páginas geradas.
    pub paginas: usize,
    /// O que muda por documento na branch base ("2 regras novas, 1 alterada;
    /// 1 termo novo"), com a lista dos documentos.
    pub documentos: ResumoDosDocumentos,
    /// Páginas novas, alteradas e removidas na branch base (as páginas
    /// afetadas).
    pub mudancas: Vec<MudancaPrevista>,
    /// A base tem a publicação no formato antigo (os `.md` da base): os arquivos
    /// dela são removidos e as páginas entram no lugar.
    pub formato_antigo: bool,
    /// A branch da publicação já existe com conteúdo diferente da base (um PR/MR
    /// pendente): publicar pode atualizá-lo mesmo sem mudança na base.
    pub pendente_na_branch: bool,
    /// O PR/MR pendente já tem esta versão: não há o que enviar.
    pub branch_atualizada: bool,
    /// Nada a publicar: a base já tem esta versão (ou não há nada aprovado nem
    /// publicado) e não há PR/MR pendente, ou o PR/MR pendente já a tem.
    pub nada_a_publicar: bool,
    /// Documentos aprovados que ficaram fora das páginas.
    pub avisos: Vec<String>,
}

/// Resultado de [`publicar`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Publicacao {
    pub previa: Previa,
    pub publicado: Publicado,
}

/// O que a prévia e a publicação usam: o projeto na base, o nome legível, o
/// cadastro dos repositórios e o escopo do projeto.
#[derive(Debug, Clone, Copy)]
pub struct Alvo<'a> {
    pub projeto: &'a Projeto,
    /// Nome legível do projeto (título do PR/MR).
    pub nome: &'a str,
    pub repositorios: &'a Repositorios,
    /// Escopo do projeto (pastas do código), para os permalinks das evidências
    /// nas páginas ([`crate::documentacao::LinksNaForja`]). Sem ele, as
    /// evidências ficam como texto.
    pub escopo: Option<&'a Escopo>,
}

struct Preparo<'a> {
    destino: DestinoResolvido<'a>,
    token: Option<Secret>,
    plano: Plano,
    previa: Previa,
}

fn preparar<'a>(f: &Ferramentas, alvo: &Alvo<'a>) -> Result<Preparo<'a>, String> {
    let destino = alvo
        .repositorios
        .destino_docs()
        .ok_or("o projeto não tem destino da documentação: cadastre-o na tela de repositórios")?;
    let projeto = alvo.projeto.nome();
    let branch = branch_da_publicacao(projeto);
    validar_branch_de_escrita(&branch, destino.branch_base()).map_err(|e| e.to_string())?;
    let pasta = destino.pasta_local().ok_or_else(|| {
        format!(
            "{} não tem pasta local: clone o repositório antes de publicar",
            destino.repositorio.rotulo()
        )
    })?;
    let token = token_do_repositorio(destino.repositorio, f.chaveiro.as_ref())
        .map_err(|e| e.to_string())?;
    let links = alvo
        .escopo
        .and_then(|e| crate::documentacao::LinksNaForja::novo(&alvo.repositorios.codigo, e));
    let geradas = paginas_do_projeto(
        alvo.projeto,
        links.as_ref().map(|l| l as &dyn LinksDeEvidencia),
    )?;
    let manifesto = destino.caminho_no_repositorio(MANIFESTO);
    let remoto = escrita::ler_remoto(
        &f.git,
        token.as_ref(),
        &Leitura {
            repositorio: destino.repositorio,
            pasta,
            base: destino.branch_base(),
            branch: &branch,
            pasta_no_repositorio: destino.subpasta,
            ler: &[manifesto.as_str()],
        },
    )
    .map_err(|e| e.to_string())?;
    validar_branch_de_escrita(&branch, Some(&remoto.base)).map_err(|e| e.to_string())?;
    let plano = planejar(projeto, &geradas.paginas, &destino, &remoto)?;
    let documentos = documentos_da_previa(f, &destino, pasta, &geradas.paginas, &remoto, &plano)?;
    let previa = Previa {
        destino: alvo
            .repositorios
            .docs
            .as_ref()
            .map(|d| d.rotulo(&alvo.repositorios.codigo))
            .unwrap_or_default(),
        repositorio: destino.repositorio.rotulo(),
        independente: destino.independente,
        base: remoto.base.clone(),
        branch,
        aprovados: geradas.aprovados,
        paginas: geradas.paginas.len(),
        documentos,
        mudancas: plano.previa.clone(),
        formato_antigo: plano.formato_antigo,
        pendente_na_branch: plano.pendente_na_branch,
        branch_atualizada: plano.branch_atualizada,
        nada_a_publicar: (!plano.muda_a_base && !plano.pendente_na_branch)
            || plano.branch_atualizada,
        avisos: geradas.avisos,
    };
    Ok(Preparo {
        destino,
        token,
        plano,
        previa,
    })
}

/// Compara, por documento, as páginas geradas com as que o app publicou na
/// branch base. Só lê do git as páginas que mudaram (as iguais vêm das
/// geradas).
fn documentos_da_previa(
    f: &Ferramentas,
    destino: &DestinoResolvido<'_>,
    pasta: &std::path::Path,
    paginas: &[PaginaPublicada],
    remoto: &LeituraRemota,
    plano: &Plano,
) -> Result<ResumoDosDocumentos, String> {
    let por_caminho: BTreeMap<String, &PaginaPublicada> = paginas
        .iter()
        .map(|p| (destino.caminho_no_repositorio(&p.caminho), p))
        .collect();
    let mut leitor = None;
    let mut anteriores: Vec<(String, String, String)> = Vec::new();
    for caminho in &plano.publicados_na_base {
        if !caminho.ends_with(".md") {
            continue;
        }
        let Some(blob) = remoto.na_base.arquivos.get(caminho) else {
            continue;
        };
        let bytes = match por_caminho.get(caminho) {
            Some(p) if remoto.formato.id_do_blob(&p.conteudo) == *blob => p.conteudo.clone(),
            _ => {
                if leitor.is_none() {
                    leitor = Some(
                        LeitorDeCommit::abrir(&f.git, destino.repositorio, pasta)
                            .map_err(|e| e.to_string())?,
                    );
                }
                leitor
                    .as_ref()
                    .expect("aberto acima")
                    .ler(blob)
                    .map_err(|e| e.to_string())?
            }
        };
        let Ok(texto) = String::from_utf8(bytes) else {
            continue;
        };
        anteriores.push((caminho.clone(), relativo(destino, caminho), texto));
    }
    let novas: Vec<(String, String, String)> = paginas
        .iter()
        .filter_map(|p| {
            Some((
                destino.caminho_no_repositorio(&p.caminho),
                p.caminho.clone(),
                String::from_utf8(p.conteudo.clone()).ok()?,
            ))
        })
        .collect();
    Ok(comparar_documentos(
        &lidas(&novas),
        &lidas(&anteriores),
        plano.formato_antigo,
    ))
}

fn lidas(v: &[(String, String, String)]) -> Vec<PaginaLida<'_>> {
    v.iter()
        .map(|(c, r, t)| PaginaLida {
            caminho: c,
            relativo: r,
            texto: t,
        })
        .collect()
}

/// Caminho relativo à pasta da documentação.
fn relativo(destino: &DestinoResolvido<'_>, caminho: &str) -> String {
    if destino.subpasta.is_empty() {
        caminho.to_owned()
    } else {
        caminho
            .strip_prefix(destino.subpasta)
            .and_then(|r| r.strip_prefix('/'))
            .unwrap_or(caminho)
            .to_owned()
    }
}

/// O que muda na branch base do destino se a documentação aprovada for
/// publicada agora. Busca o remoto, mas não escreve nada.
pub fn previa(f: &Ferramentas, alvo: &Alvo<'_>) -> Result<Previa, String> {
    preparar(f, alvo).map(|p| p.previa)
}

/// Publica as páginas da documentação aprovada na branch da publicação e abre
/// (ou atualiza) o PR/MR. Sem mudança, nada é comitado nem aberto.
pub fn publicar(f: &Ferramentas, alvo: &Alvo<'_>) -> Result<Publicacao, String> {
    let p = preparar(f, alvo)?;
    // Nada na base nem PR/MR pendente: sem push nem chamada à forja. Com a branch
    // pendente já atualizada, a escrita confere o PR/MR (e o abre, se a
    // publicação anterior parou antes dele), sem commit.
    if p.previa.nada_a_publicar && !p.previa.branch_atualizada {
        return Ok(Publicacao {
            publicado: Publicado {
                situacao: Situacao::SemMudancas,
                branch: p.previa.branch.clone(),
                base: p.previa.base.clone(),
                commit: None,
                link: None,
            },
            previa: p.previa,
        });
    }
    let pasta = p.destino.pasta_local().expect("conferida na preparação");
    let titulo = format!("docs: documentação aprovada de {}", alvo.nome.trim());
    let leia_me = p.destino.repositorio.link(
        &p.previa.branch,
        &p.destino.caminho_no_repositorio(LEIA_ME),
        None,
    );
    let corpo = corpo_do_pr(&p.previa, leia_me.as_deref());
    let publicado = escrita::publicar(
        f,
        p.token.as_ref(),
        &Pedido {
            repositorio: p.destino.repositorio,
            pasta,
            base: Some(&p.previa.base),
            branch: &p.previa.branch,
            mudancas: &p.plano.mudancas,
            mensagem: &titulo,
            titulo: &titulo,
            corpo: &corpo,
        },
    )
    .map_err(|e| e.to_string())?;
    Ok(Publicacao {
        previa: p.previa,
        publicado,
    })
}

/// Corpo do PR/MR: o resumo por documento, o link para o README publicado na
/// branch e as páginas afetadas, em listas curtas.
fn corpo_do_pr(p: &Previa, leia_me: Option<&str>) -> String {
    let mut s = format!(
        "Documentação aprovada publicada pelo ctower-business-agents: {} páginas geradas de {} \
         documentos aprovados. Só entram documentos aprovados pelo analista; revise antes do \
         merge, inclusive quanto a segredos e dados pessoais. As páginas são geradas pelo app: \
         correções vão na base e numa nova publicação.\n",
        p.paginas, p.aprovados
    );
    if let Some(l) = leia_me {
        let _ = writeln!(s, "\n**Leia a documentação publicada:** [README]({l})");
    }
    let d = &p.documentos;
    if !d.texto.is_empty() {
        let _ = writeln!(s, "\n## O que muda\n\n{}", d.texto);
    }
    if p.formato_antigo {
        s.push_str(
            "\nA publicação anterior estava no formato antigo (os arquivos da base): eles saem e \
             as páginas legíveis entram no lugar.\n",
        );
    }
    for (tipo, titulo) in [
        (TipoMudanca::Novo, "Documentos novos"),
        (TipoMudanca::Alterado, "Documentos alterados"),
        (TipoMudanca::Removido, "Documentos removidos"),
    ] {
        let itens: Vec<&MudancaDeDocumento> =
            d.documentos.iter().filter(|m| m.mudanca == tipo).collect();
        if itens.is_empty() {
            continue;
        }
        let _ = writeln!(s, "\n### {titulo} ({})\n", itens.len());
        for m in itens.iter().take(MAX_NO_CORPO) {
            let _ = writeln!(s, "- {} · {} (`{}`)", m.id, m.titulo, m.pagina);
        }
        if itens.len() > MAX_NO_CORPO {
            let _ = writeln!(s, "- e mais {}", itens.len() - MAX_NO_CORPO);
        }
    }
    if !p.mudancas.is_empty() {
        let _ = writeln!(s, "\n## Páginas afetadas ({})\n", p.mudancas.len());
        for m in p.mudancas.iter().take(MAX_NO_CORPO) {
            let rotulo = match m.tipo {
                TipoMudanca::Novo => "nova",
                TipoMudanca::Alterado => "alterada",
                TipoMudanca::Removido => "removida",
            };
            let _ = writeln!(s, "- `{}` ({rotulo})", m.caminho);
        }
        if p.mudancas.len() > MAX_NO_CORPO {
            let _ = writeln!(s, "- e mais {}", p.mudancas.len() - MAX_NO_CORPO);
        }
    }
    s
}

#[cfg(test)]
#[path = "publicacao_testes.rs"]
mod tests;
