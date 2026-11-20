//! Publicação dos documentos aprovados no repositório (#276).
//!
//! A base aprovada vai, como está (`.md` com frontmatter, no mesmo layout da
//! base), para a pasta do destino da documentação do projeto
//! ([`cba_repos::DestinoDocs`]), numa branch própria e com PR/MR
//! ([`cba_repos::escrita`]).
//!
//! - **Só o aprovado** ([`aprovado`]): análises, documentos de funcionalidade e
//!   mapa aprovados, regras confirmadas, termos do glossário e sugestões aceitas.
//!   Demandas, atas, análises de reunião, registros da demanda, pessoas, memórias
//!   e o `projeto.md` ficam de fora: são trabalho em andamento ou trazem dados
//!   pessoais e caminhos locais.
//! - **Manifesto** ([`MANIFESTO`]): a pasta publicada leva a lista do que o app
//!   publicou. Só os arquivos listados nele são removidos quando deixam de estar
//!   aprovados; um arquivo que já existe na pasta sem estar no manifesto (escrito
//!   à mão) nunca é apagado nem sobrescrito: a publicação para com um conflito.
//! - **Prévia** ([`previa`]): o que muda na branch base do destino (novos,
//!   alterados e removidos), lido do remoto sem tocar no checkout do analista.
//! - **Idempotente**: sem mudança, não há commit nem PR/MR
//!   ([`Situacao::SemMudancas`]).
//! - **Branch determinística por projeto** ([`branch_da_publicacao`]):
//!   `docs/cba-<projeto>`, conferida por [`validar_branch_de_escrita`].

use std::collections::{BTreeMap, BTreeSet};

use cba_kb::frontmatter::FrontmatterLivre;
use cba_kb::{Documento, Projeto, TipoDocumento};
use cba_repos::chaveiro::token_do_repositorio;
use cba_repos::demanda::Ferramentas;
use cba_repos::docs::normalizar_subpasta;
use cba_repos::escrita::{
    self, Leitura, LeituraRemota, Mudanca, Pedido, Publicado, Situacao, validar_branch_de_escrita,
};
use cba_repos::{DestinoResolvido, Repositorios, Secret};
use serde::{Deserialize, Serialize};

/// Manifesto da publicação, na pasta publicada: os documentos que o app publicou
/// lá (relativos à pasta). Começa com `.` para ficar fora das listas de
/// documentos.
pub const MANIFESTO: &str = ".cba-publicacao.json";

/// Quem gera o manifesto.
const GERADO_POR: &str = "ctower-business-agents";

/// Quantos arquivos o corpo do PR/MR lista por tipo de mudança.
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
    /// Todos os documentos aprovados, o manifesto e as remoções; vazio quando
    /// não há nada publicado nem a publicar.
    pub mudancas: Vec<Mudanca>,
    /// Novos, alterados e removidos em relação à branch base (sem o manifesto).
    pub previa: Vec<MudancaPrevista>,
    /// A publicação muda algo na branch base (inclusive só o manifesto).
    pub muda_a_base: bool,
    /// A branch da publicação existe no remoto com a pasta diferente da base
    /// (PR/MR pendente).
    pub pendente_na_branch: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct Manifesto {
    gerado_por: String,
    projeto: String,
    documentos: Vec<String>,
}

/// Lê o manifesto de uma branch: os caminhos (relativos à raiz do repositório)
/// que o app publicou. Recusa o manifesto de outro projeto.
fn publicados(
    conteudo: Option<&Vec<u8>>,
    caminho: &str,
    projeto: &str,
    destino: &DestinoResolvido<'_>,
) -> Result<BTreeSet<String>, String> {
    let Some(conteudo) = conteudo else {
        return Ok(BTreeSet::new());
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
    // Só caminhos válidos e dentro da pasta: um manifesto editado não apaga nada
    // fora dela.
    Ok(m.documentos
        .iter()
        .filter_map(|d| normalizar_subpasta(d).ok().filter(|d| !d.is_empty()))
        .filter(|d| d != MANIFESTO)
        .map(|d| destino.caminho_no_repositorio(&d))
        .collect())
}

/// Calcula as mudanças da publicação comparando os documentos aprovados com o
/// que está no remoto (sem rede: a leitura já foi feita).
pub fn planejar(
    projeto: &str,
    documentos: &[DocumentoAprovado],
    destino: &DestinoResolvido<'_>,
    remoto: &LeituraRemota,
) -> Result<Plano, String> {
    let manifesto = destino.caminho_no_repositorio(MANIFESTO);
    let base = &remoto.na_base;
    let mut anteriores = publicados(base.conteudos.get(&manifesto), &manifesto, projeto, destino)?;
    if let Some(b) = &remoto.na_branch {
        anteriores.extend(publicados(
            b.conteudos.get(&manifesto),
            &manifesto,
            projeto,
            destino,
        )?);
    }
    let id = |c: &[u8]| remoto.formato.id_do_blob(c);

    let novos: BTreeMap<String, &[u8]> = documentos
        .iter()
        .map(|d| {
            (
                destino.caminho_no_repositorio(&d.caminho),
                d.conteudo.as_slice(),
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
        });
    }
    let texto_manifesto = {
        let m = Manifesto {
            gerado_por: GERADO_POR.into(),
            projeto: projeto.into(),
            documentos: documentos.iter().map(|d| d.caminho.clone()).collect(),
        };
        let mut t = serde_json::to_string_pretty(&m).expect("manifesto serializável");
        t.push('\n');
        t.into_bytes()
    };
    let manifesto_muda = base.arquivos.get(&manifesto) != Some(&id(&texto_manifesto));
    let mut mudancas: Vec<Mudanca> = novos
        .iter()
        .map(|(c, conteudo)| Mudanca::gravar(c.clone(), conteudo.to_vec()))
        .collect();
    mudancas.push(Mudanca::gravar(manifesto, texto_manifesto));
    mudancas.extend(removidos.into_iter().map(|c| Mudanca::remover(c.clone())));
    Ok(Plano {
        mudancas,
        muda_a_base: manifesto_muda || !previa.is_empty(),
        previa,
        pendente_na_branch,
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
    /// Novos, alterados e removidos na branch base.
    pub mudancas: Vec<MudancaPrevista>,
    /// A branch da publicação já existe com conteúdo diferente da base (um PR/MR
    /// pendente): publicar pode atualizá-lo mesmo sem mudança na base.
    pub pendente_na_branch: bool,
    /// Nada a publicar: a base já tem os documentos aprovados e não há PR/MR
    /// pendente.
    pub nada_a_publicar: bool,
}

/// Resultado de [`publicar`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Publicacao {
    pub previa: Previa,
    pub publicado: Publicado,
}

/// O que a prévia e a publicação usam: o projeto na base, o nome legível e o
/// cadastro dos repositórios.
#[derive(Debug, Clone, Copy)]
pub struct Alvo<'a> {
    pub projeto: &'a Projeto,
    /// Nome legível do projeto (título do PR/MR).
    pub nome: &'a str,
    pub repositorios: &'a Repositorios,
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
    let documentos = documentos_aprovados(alvo.projeto)?;
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
    let plano = planejar(projeto, &documentos, &destino, &remoto)?;
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
        aprovados: documentos.len(),
        mudancas: plano.previa.clone(),
        pendente_na_branch: plano.pendente_na_branch,
        nada_a_publicar: !plano.muda_a_base && !plano.pendente_na_branch,
    };
    Ok(Preparo {
        destino,
        token,
        plano,
        previa,
    })
}

/// O que muda na branch base do destino se a documentação aprovada for
/// publicada agora. Busca o remoto, mas não escreve nada.
pub fn previa(f: &Ferramentas, alvo: &Alvo<'_>) -> Result<Previa, String> {
    preparar(f, alvo).map(|p| p.previa)
}

/// Publica os documentos aprovados na branch da publicação e abre (ou atualiza)
/// o PR/MR. Sem mudança, nada é comitado nem aberto.
pub fn publicar(f: &Ferramentas, alvo: &Alvo<'_>) -> Result<Publicacao, String> {
    let p = preparar(f, alvo)?;
    // Nada na base nem PR/MR pendente: sem push nem chamada à forja.
    if p.previa.nada_a_publicar {
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
    let corpo = corpo_do_pr(&p.previa);
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

/// Corpo do PR/MR: o que mudou, em listas curtas.
fn corpo_do_pr(p: &Previa) -> String {
    let mut s = format!(
        "Documentação aprovada publicada pelo ctower-business-agents ({} documentos \
         aprovados). Só entram documentos aprovados pelo analista; revise antes do merge, \
         inclusive quanto a segredos e dados pessoais.\n",
        p.aprovados
    );
    for (tipo, titulo) in [
        (TipoMudanca::Novo, "Novos"),
        (TipoMudanca::Alterado, "Alterados"),
        (TipoMudanca::Removido, "Removidos"),
    ] {
        let itens: Vec<&str> = p
            .mudancas
            .iter()
            .filter(|m| m.tipo == tipo)
            .map(|m| m.caminho.as_str())
            .collect();
        if itens.is_empty() {
            continue;
        }
        s.push_str(&format!("\n## {titulo} ({})\n\n", itens.len()));
        for i in itens.iter().take(MAX_NO_CORPO) {
            s.push_str(&format!("- `{i}`\n"));
        }
        if itens.len() > MAX_NO_CORPO {
            s.push_str(&format!("- e mais {}\n", itens.len() - MAX_NO_CORPO));
        }
    }
    s
}

#[cfg(test)]
#[path = "publicacao_testes.rs"]
mod tests;
