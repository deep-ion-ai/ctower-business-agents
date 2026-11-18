//! Fluxo "Mapear funcionalidades" (#208): o inventário das funcionalidades do
//! projeto em linguagem de negócio, a partir do código.
//!
//! - `pontos_de_entrada` (só leitura): os pontos de entrada do mapa estrutural no
//!   escopo da análise ([`cba_index::Mapa::pontos_de_entrada`]), com o que já foi
//!   feito com cada um (atribuído, justificado ou sem tratamento).
//! - `registrar_funcionalidade`: uma funcionalidade com nome de negócio, área,
//!   descrição, atores, pontos de entrada (cada um com evidência conferida contra
//!   o código), dados lidos e gravados e relacionadas. Sem ponto de entrada com
//!   evidência válida, é recusada. O identificador (`F-<SIGLA>-NNN`) é o do
//!   catálogo quando a funcionalidade já existe nele (pelo `id` informado ou por
//!   um ponto de entrada em comum); senão, o próximo da sigla da área.
//! - `tratar_ponto_de_entrada`: atribui um ponto do mapa a uma funcionalidade já
//!   registrada ou o justifica (código morto, utilitário, outro motivo).
//!
//! **Completude, sem IA:** `concluir_analise` cruza os pontos do mapa no escopo
//! com os atribuídos e os justificados ([`nao_mapeados`]) e recusa enquanto
//! sobrar algum.
//!
//! **Catálogo:** a aprovação funde as funcionalidades aceitas no catálogo
//! ([`cba_core::catalogo::Catalogo::fundir`]) e remonta
//! `funcionalidades/catalogo.md` ([`gravar_catalogo`]). A aprovação de uma
//! explicação ligada a uma funcionalidade grava o documento negocial em
//! `funcionalidades/<area>/<tema>/negocial.md` e o liga ao item
//! ([`ligar_explicacao`]).

use std::collections::BTreeSet;
use std::fmt::Write as _;

use cba_core::ambiente::Ambiente;
use cba_core::analise::{Analise, ErroModelo, Fluxo};
use cba_core::analises::NovaAnalise;
use cba_core::catalogo::{Catalogo, ItemCatalogo};
use cba_core::eventos::Mudanca;
use cba_core::funcionalidades::{
    EscopoMapa, Funcionalidade, MotivoSemFuncionalidade, NovaFuncionalidade, PontoAtribuido,
    PontoJustificado, SituacaoFuncionalidade,
};
use cba_docs::{
    Citacao, Conteudo, FUNCIONALIDADES_MAPEADAS, INDICE_CATALOGO, NAO_MAPEADOS, Templates, Texto,
};
use cba_index::{Mapa, PontoEntrada};
use cba_kb::frontmatter::{
    Area, FrontmatterCatalogo, FrontmatterFuncionalidade, StatusAnalise, TipoDocumento,
};
use cba_kb::{Base, CaminhoRelativo, Documento, Id, IdFuncionalidade, layout, slug};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::aprovacao::sigla_do_slug;
use crate::comum;
use crate::documento::citacao;
use crate::evidencias::{self, ArquivoDoProjeto, EvidenciaCitada};
use crate::fontes;
use crate::paginacao::{ArgsPagina, Item, Pagina, com_paginacao};
use crate::registro::{ErroFerramenta, Ferramenta, Saida, ler_argumentos};

/// Arquivo do documento negocial dentro da pasta da funcionalidade.
pub const ARQUIVO_NEGOCIAL: &str = "negocial.md";

fn em_uma_linha(texto: &str) -> String {
    texto.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ---------------------------------------------------------------------------
// Áreas

/// Área resolvida: slug, nome para quem lê e sigla dos identificadores.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AreaResolvida {
    pub slug: String,
    pub nome: String,
    pub sigla: String,
}

/// Área a partir do texto informado e das áreas do `projeto.md`, como na
/// aprovação das regras: procurada pelo id ou pelo nome (sem acentos nem
/// maiúsculas); fora delas, vale o slug do texto, com o texto como nome.
pub fn resolver_area(areas: &[Area], texto: &str) -> Option<AreaResolvida> {
    let chave = slug(texto);
    if chave.is_empty() {
        return None;
    }
    Some(
        match areas
            .iter()
            .find(|a| a.id == chave || slug(&a.nome) == chave)
        {
            Some(a) => AreaResolvida {
                slug: a.id.clone(),
                nome: a.nome.clone(),
                sigla: a.sigla.clone().unwrap_or_else(|| sigla_do_slug(&a.id)),
            },
            None => AreaResolvida {
                sigla: sigla_do_slug(&chave),
                slug: chave,
                nome: texto.trim().to_owned(),
            },
        },
    )
}

fn areas_do_projeto(ambiente: &Ambiente) -> Vec<Area> {
    ambiente
        .kb
        .ler_projeto()
        .map(|d| d.frontmatter.areas)
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Catálogo e pontos de entrada

fn catalogo(ambiente: &Ambiente) -> Result<Catalogo, ErroFerramenta> {
    Catalogo::carregar(&ambiente.kb).map_err(|e| ErroFerramenta::Interno(e.to_string()))
}

fn exigir_fluxo(analise: &Analise) -> Result<(), ErroFerramenta> {
    if analise.fluxo == Fluxo::Mapear {
        Ok(())
    } else {
        Err(comum::recusa(ErroModelo::ForaDoFluxoDeMapear(
            analise.id.clone(),
        )))
    }
}

/// Pontos de entrada do mapa no escopo da análise. No escopo de uma área, contam
/// os pontos que o catálogo já põe nela e os que a análise atribuiu ou justificou.
pub fn no_escopo(
    analise: &Analise,
    pontos: &[PontoEntrada],
    catalogo: &Catalogo,
) -> Vec<PontoEntrada> {
    let escopo = analise.escopo.clone().unwrap_or_default();
    let da_area: BTreeSet<&str> = match &escopo.area {
        None => BTreeSet::new(),
        Some(area) => catalogo
            .funcionalidades
            .iter()
            .filter(|f| &f.area == area)
            .flat_map(|f| f.pontos.iter().filter_map(|p| p.chave.as_deref()))
            .chain(analise.pontos_atribuidos())
            .chain(analise.pontos_justificados.iter().map(|j| j.chave.as_str()))
            .collect(),
    };
    pontos
        .iter()
        .filter(|p| escopo.contem_arquivo(&p.arquivo))
        .filter(|p| escopo.area.is_none() || da_area.contains(p.chave.as_str()))
        .cloned()
        .collect()
}

/// Pontos do escopo sem funcionalidade aberta nem justificativa.
pub fn nao_mapeados<'a>(analise: &Analise, pontos: &'a [PontoEntrada]) -> Vec<&'a PontoEntrada> {
    let atribuidos = analise.pontos_atribuidos();
    let justificados: BTreeSet<&str> = analise
        .pontos_justificados
        .iter()
        .map(|j| j.chave.as_str())
        .collect();
    pontos
        .iter()
        .filter(|p| {
            !atribuidos.contains(p.chave.as_str()) && !justificados.contains(p.chave.as_str())
        })
        .collect()
}

/// Pontos do escopo e os não mapeados, a partir do mapa atual do projeto.
pub fn completude(
    ambiente: &Ambiente,
    analise: &Analise,
) -> Result<(Vec<PontoEntrada>, Vec<PontoEntrada>), ErroFerramenta> {
    let mapa = fontes::mapa(ambiente)?;
    let catalogo = catalogo(ambiente)?;
    let escopo = no_escopo(analise, &mapa.mapa.pontos_de_entrada(), &catalogo);
    let faltam = nao_mapeados(analise, &escopo)
        .into_iter()
        .cloned()
        .collect();
    Ok((escopo, faltam))
}

fn linha_do_ponto(p: &PontoEntrada) -> String {
    format!(
        "`{}` ({}, `{}`:{})",
        p.nome,
        p.tipo.nome(),
        p.arquivo,
        p.linhas.inicio
    )
}

/// Ponto do mapa que o agente citou: pela chave, pelo nome (único, de preferência
/// no arquivo da evidência) ou pela evidência dentro das linhas do símbolo.
fn casar_ponto<'a>(
    pontos: &'a [PontoEntrada],
    citado: Option<&str>,
    evidencia: &cba_core::analise::Evidencia,
) -> Option<&'a PontoEntrada> {
    let arquivo = evidencia.arquivo.as_str();
    if let Some(c) = citado.map(str::trim).filter(|c| !c.is_empty()) {
        if let Some(p) = pontos.iter().find(|p| p.chave == c) {
            return Some(p);
        }
        let up = c.to_uppercase();
        let pelo_nome: Vec<&PontoEntrada> = pontos
            .iter()
            .filter(|p| {
                let n = p.nome.to_uppercase();
                n == up || n.ends_with(&format!(".{up}"))
            })
            .collect();
        let no_arquivo: Vec<&&PontoEntrada> =
            pelo_nome.iter().filter(|p| p.arquivo == arquivo).collect();
        match (no_arquivo.as_slice(), pelo_nome.as_slice()) {
            ([um], _) => return Some(um),
            ([], [um]) => return Some(um),
            _ => {}
        }
    }
    let (i, f) = (evidencia.linhas.inicio, evidencia.linhas.fim);
    pontos
        .iter()
        .filter(|p| p.arquivo == arquivo && p.linhas.inicio <= f && i <= p.linhas.fim)
        .min_by_key(|p| p.linhas.fim - p.linhas.inicio)
}

// ---------------------------------------------------------------------------
// Criação da análise

/// Cria uma análise do fluxo `mapear` com o escopo (pasta e área opcionais) e,
/// se o analista escreveu, a orientação dele. Usada por `iniciar_analise` e pela
/// tela de nova análise do app.
pub fn criar_analise_de_mapeamento(
    ambiente: &Ambiente,
    pasta: Option<String>,
    area: Option<String>,
    entrada: Option<String>,
    titulo: Option<String>,
) -> Result<Analise, ErroFerramenta> {
    let pasta = pasta
        .map(|p| crate::mapa::normalizar(&p))
        .filter(|p| !p.is_empty());
    let area = match area.filter(|a| !a.trim().is_empty()) {
        None => None,
        Some(a) => Some(
            resolver_area(&areas_do_projeto(ambiente), &a)
                .ok_or_else(|| {
                    ErroFerramenta::Argumentos(format!(
                        "`area`: `{a}` não tem letras nem dígitos para formar a área"
                    ))
                })?
                .slug,
        ),
    };
    let escopo = EscopoMapa { pasta, area };
    let mut texto = format!("Mapear as funcionalidades de {}.", escopo.descricao());
    if let Some(e) = entrada.filter(|e| !e.trim().is_empty()) {
        let _ = write!(texto, "\n\n{}", e.trim());
    }
    let titulo = match titulo {
        Some(t) if !t.trim().is_empty() => t,
        _ => {
            if escopo.e_projeto() {
                "Mapa de funcionalidades".to_owned()
            } else {
                format!("Mapa de funcionalidades · {}", escopo.descricao())
            }
        }
    };
    let analise = ambiente.analises.criar_com_demanda(
        NovaAnalise {
            fluxo: Fluxo::Mapear,
            titulo,
            entrada: texto,
            analista: None,
            base: crate::ciclo::versao_do_codigo(ambiente),
        },
        None,
    )?;
    if escopo.e_projeto() {
        return Ok(analise);
    }
    let registrado = comum::alterar(ambiente, &analise.id, Mudanca::Funcionalidade, |a, _| {
        a.escopo = Some(escopo);
        Ok(())
    })?;
    Ok(registrado.analise)
}

/// Entrada de uma explicação aberta a partir do catálogo: o nome, a descrição e
/// os pontos de entrada da funcionalidade.
pub fn entrada_da_explicacao(item: &ItemCatalogo) -> String {
    let mut t = format!(
        "Explicar a funcionalidade {} · {} (área {}).\n\n{}",
        item.id,
        item.nome,
        item.area_nome,
        em_uma_linha(&item.descricao)
    );
    if !item.pontos.is_empty() {
        t.push_str("\n\nPontos de entrada:");
        for p in &item.pontos {
            let _ = write!(
                t,
                "\n- {} (`{}`, linhas {}-{})",
                p.nome, p.evidencia.arquivo, p.evidencia.linhas.inicio, p.evidencia.linhas.fim
            );
        }
    }
    t
}

/// Cria uma análise do fluxo `explicar` para uma funcionalidade do catálogo,
/// ligada a ela ([`Analise::funcionalidade`]).
pub fn criar_explicacao(
    ambiente: &Ambiente,
    id: &IdFuncionalidade,
) -> Result<Analise, ErroFerramenta> {
    let catalogo = catalogo(ambiente)?;
    let item = catalogo.item(id).ok_or_else(|| {
        ErroFerramenta::Recusa(format!(
            "a funcionalidade {id} não está no catálogo do projeto; aprove o mapeamento \
             antes de explicá-la"
        ))
    })?;
    let analise = ambiente.analises.criar_com_demanda(
        NovaAnalise {
            fluxo: Fluxo::Explicar,
            titulo: format!("{} · {}", item.id, item.nome),
            entrada: entrada_da_explicacao(item),
            analista: None,
            base: crate::ciclo::versao_do_codigo(ambiente),
        },
        None,
    )?;
    let id = id.clone();
    let registrado = comum::alterar(ambiente, &analise.id, Mudanca::Estado, |a, _| {
        a.funcionalidade = Some(id);
        Ok(())
    })?;
    Ok(registrado.analise)
}

// ---------------------------------------------------------------------------
// Documento da análise

fn rotulo_situacao(f: &Funcionalidade) -> &'static str {
    match f.situacao {
        SituacaoFuncionalidade::Pendente => "sem decisão",
        SituacaoFuncionalidade::Aceita => "aceita",
        SituacaoFuncionalidade::Descartada => "descartada",
        SituacaoFuncionalidade::Juntada => "juntada",
    }
}

/// Bloco de uma funcionalidade (do mapa ou do catálogo), com os pontos de entrada
/// como notas.
#[allow(clippy::too_many_arguments)]
fn bloco(
    secao: &str,
    id: &IdFuncionalidade,
    nome: &str,
    descricao: &str,
    atores: &[String],
    pontos: &[PontoAtribuido],
    lidos: &[String],
    gravados: &[String],
    relacionadas: &[IdFuncionalidade],
    extra: &str,
) -> Texto {
    let mut t = format!(
        "#### {id} · {}\n\n{}",
        em_uma_linha(nome),
        em_uma_linha(descricao)
    );
    if !extra.is_empty() {
        let _ = write!(t, "\n\n{extra}");
    }
    if !atores.is_empty() {
        let _ = write!(t, "\n\n**Atores:** {}", atores.join(", "));
    }
    t.push_str("\n\n**Pontos de entrada:**\n");
    for p in pontos {
        let tipo = p
            .tipo
            .map(|t| format!(" ({})", t.nome()))
            .unwrap_or_default();
        let fora = if p.chave.is_none() {
            " · _fora da lista do mapa_"
        } else {
            ""
        };
        let _ = write!(t, "\n- `{}`{tipo}{fora}", p.nome);
    }
    if !lidos.is_empty() {
        let _ = write!(t, "\n\n**Dados lidos:** {}", lidos.join(", "));
    }
    if !gravados.is_empty() {
        let _ = write!(t, "\n\n**Dados gravados:** {}", gravados.join(", "));
    }
    if !relacionadas.is_empty() {
        let r: Vec<String> = relacionadas.iter().map(ToString::to_string).collect();
        let _ = write!(t, "\n\n**Relacionadas:** {}", r.join(", "));
    }
    let mut citacoes: Vec<Citacao> = Vec::new();
    for p in pontos {
        let c = citacao(&p.evidencia);
        if !citacoes
            .iter()
            .any(|x| x.arquivo == c.arquivo && x.linhas == c.linhas)
        {
            citacoes.push(c);
        }
    }
    Texto {
        secao: secao.into(),
        texto: t,
        citacoes,
    }
}

/// Textos das seções `funcionalidades_mapeadas` (por área) e `nao_mapeados` (os
/// pontos justificados) do documento da análise.
pub fn textos(analise: &Analise) -> Vec<Texto> {
    let mut textos = Vec::new();
    let mut abertas: Vec<&Funcionalidade> = analise.funcionalidades_no_documento().collect();
    abertas.sort_by(|a, b| (&a.area_nome, &a.id).cmp(&(&b.area_nome, &b.id)));
    let mut area_atual: Option<&str> = None;
    for f in abertas {
        if area_atual != Some(f.area_nome.as_str()) {
            textos.push(Texto::novo(
                FUNCIONALIDADES_MAPEADAS,
                format!("### {}", f.area_nome),
            ));
            area_atual = Some(f.area_nome.as_str());
        }
        let mut extra = format!("**Situação:** {}", rotulo_situacao(f));
        if f.do_catalogo {
            extra.push_str(" · já no catálogo");
        }
        textos.push(bloco(
            FUNCIONALIDADES_MAPEADAS,
            &f.id,
            &f.nome,
            &f.descricao,
            &f.atores,
            &f.pontos,
            &f.dados_lidos,
            &f.dados_gravados,
            &f.relacionadas,
            &extra,
        ));
    }
    if !analise.pontos_justificados.is_empty() {
        let mut t = String::from(
            "| Ponto de entrada | Tipo | Arquivo | Motivo | Justificativa |\n|---|---|---|---|---|",
        );
        for j in &analise.pontos_justificados {
            let _ = write!(
                t,
                "\n| `{}` | {} | `{}` | {} | {} |",
                j.nome,
                j.tipo.nome(),
                j.arquivo,
                j.motivo.nome(),
                em_uma_linha(&j.justificativa).replace('|', "\\|")
            );
        }
        textos.push(Texto::novo(NAO_MAPEADOS, t));
    }
    textos
}

// ---------------------------------------------------------------------------
// Documento do catálogo

fn situacao_no_catalogo(item: &ItemCatalogo) -> String {
    let doc = if item.documentada() {
        "documentada"
    } else {
        "só mapeada"
    };
    format!("{doc} · {}", item.situacao.nome())
}

/// Textos das seções do catálogo: o índice por área e as funcionalidades.
pub fn textos_do_catalogo(catalogo: &Catalogo, caminho: &CaminhoRelativo) -> Vec<Texto> {
    let mut textos = Vec::new();
    let itens = catalogo.ordenadas();
    if itens.is_empty() {
        return textos;
    }
    let mut indice = String::new();
    let mut area_atual: Option<&str> = None;
    for item in &itens {
        if area_atual != Some(item.area_nome.as_str()) {
            if area_atual.is_some() {
                indice.push_str("\n\n");
            }
            let _ = write!(
                indice,
                "### {}\n\n| Funcionalidade | Nome | Situação | Pontos de entrada |\n|---|---|---|---|",
                item.area_nome
            );
            area_atual = Some(item.area_nome.as_str());
        }
        let nome = match &item.documento {
            Some(d) => cba_docs::link(&item.nome, caminho, d),
            None => item.nome.clone(),
        };
        let _ = write!(
            indice,
            "\n| {} | {} | {} | {} |",
            item.id,
            nome.replace('|', "\\|"),
            situacao_no_catalogo(item),
            item.pontos.len()
        );
    }
    textos.push(Texto::novo(INDICE_CATALOGO, indice));
    let mut area_atual: Option<&str> = None;
    for item in itens {
        if area_atual != Some(item.area_nome.as_str()) {
            textos.push(Texto::novo(
                FUNCIONALIDADES_MAPEADAS,
                format!("### {}", item.area_nome),
            ));
            area_atual = Some(item.area_nome.as_str());
        }
        let mut extra = format!("**Situação:** {}", situacao_no_catalogo(item));
        if let Some(d) = &item.documento {
            let _ = write!(
                extra,
                " · documento negocial: {}",
                cba_docs::link(d.as_str(), caminho, d)
            );
        }
        textos.push(bloco(
            FUNCIONALIDADES_MAPEADAS,
            &item.id,
            &item.nome,
            &item.descricao,
            &item.atores,
            &item.pontos,
            &item.dados_lidos,
            &item.dados_gravados,
            &item.relacionadas,
            &extra,
        ));
    }
    textos
}

/// Grava o estado do catálogo e remonta `funcionalidades/catalogo.md`.
pub fn gravar_catalogo(
    base: &Base,
    kb: &cba_kb::Projeto,
    catalogo: &Catalogo,
) -> Result<Vec<String>, String> {
    catalogo.gravar(kb).map_err(|e| e.to_string())?;
    let caminho = layout::catalogo();
    let projeto = kb
        .ler_projeto()
        .map(|d| (d.frontmatter.id, d.frontmatter.nome))
        .map_err(|e| e.to_string())?;
    let frontmatter = FrontmatterCatalogo {
        tipo: TipoDocumento::CatalogoFuncionalidades,
        titulo: format!("Catálogo de funcionalidades · {}", projeto.1),
        projeto: projeto.0,
        status: StatusAnalise::Aprovada,
        funcionalidades: catalogo.funcionalidades.len() as u32,
        atualizado: Some(cba_core::tempo::hoje()),
        origem: catalogo.origem.as_ref().map(Id::arquivo),
    };
    let mut conteudo = Conteudo::novo(frontmatter, caminho.clone());
    conteudo.textos = textos_do_catalogo(catalogo, &caminho);
    let (templates, avisos) = Templates::da_base(base);
    let mut saida: Vec<String> = avisos.iter().map(ToString::to_string).collect();
    saida.extend(
        templates
            .gravar(kb, &conteudo)
            .map_err(|e| e.to_string())?
            .iter()
            .map(ToString::to_string),
    );
    Ok(saida)
}

/// O que a aprovação de uma análise de mapeamento fez no catálogo.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, Deserialize)]
pub struct AprovacaoDoMapa {
    pub fusao: cba_core::catalogo::Fusao,
    pub catalogo: Option<CaminhoRelativo>,
    pub avisos: Vec<String>,
}

/// Funde a análise de mapeamento aprovada no catálogo. `pontos_do_mapa` são as
/// chaves dos pontos de entrada do mapa atual (sem mapa, os itens fora da análise
/// ficam como estão).
pub fn fundir_no_catalogo(
    base: &Base,
    kb: &cba_kb::Projeto,
    analise: &Analise,
    pontos_do_mapa: Option<&BTreeSet<String>>,
) -> Result<AprovacaoDoMapa, String> {
    let mut cat = Catalogo::carregar(kb).map_err(|e| e.to_string())?;
    let mut avisos = Vec::new();
    let todos: BTreeSet<String>;
    let chaves = match pontos_do_mapa {
        Some(c) => c,
        None => {
            avisos.push(
                "o mapa do projeto não foi lido: as funcionalidades que a análise não \
                 registrou ficam como estavam no catálogo"
                    .to_owned(),
            );
            todos = cat
                .funcionalidades
                .iter()
                .flat_map(|f| f.pontos.iter().filter_map(|p| p.chave.clone()))
                .collect();
            &todos
        }
    };
    let fusao = cat.fundir(analise, chaves, cba_core::tempo::agora_ms());
    avisos.extend(gravar_catalogo(base, kb, &cat)?);
    Ok(AprovacaoDoMapa {
        fusao,
        catalogo: Some(layout::catalogo()),
        avisos,
    })
}

/// Aprovada uma explicação ligada a uma funcionalidade do catálogo, grava o
/// documento negocial em `funcionalidades/<area>/<tema>/negocial.md` (o corpo do
/// documento da análise, com o frontmatter de funcionalidade) e liga o item a ele.
/// Devolve o caminho gravado.
pub fn ligar_explicacao(
    base: &Base,
    kb: &cba_kb::Projeto,
    analise: &Analise,
) -> Result<Option<CaminhoRelativo>, String> {
    let Some(id) = &analise.funcionalidade else {
        return Ok(None);
    };
    let mut cat = Catalogo::carregar(kb).map_err(|e| e.to_string())?;
    let Some(item) = cat.item(id).cloned() else {
        return Err(format!(
            "a funcionalidade {id} não está mais no catálogo; o documento negocial ficou só \
             na análise"
        ));
    };
    let pasta = layout::funcionalidade(&item.area, &item.nome).map_err(|e| e.to_string())?;
    let destino = pasta.juntar(ARQUIVO_NEGOCIAL).map_err(|e| e.to_string())?;
    let corpo = kb
        .ler::<cba_kb::frontmatter::FrontmatterAnalise>(&analise.documento)
        .map_err(|e| e.to_string())?
        .corpo;
    let doc = Documento::novo(
        FrontmatterFuncionalidade {
            tipo: TipoDocumento::Funcionalidade,
            titulo: item.nome.clone(),
            area: item.area.clone(),
            tema: slug(&item.nome),
            status: StatusAnalise::Aprovada,
            origem: Some(analise.documento.clone()),
        },
        corpo,
    );
    kb.gravar(&destino, &doc).map_err(|e| e.to_string())?;
    cat.ligar_documento(id, &analise.id, destino.clone());
    gravar_catalogo(base, kb, &cat)?;
    Ok(Some(destino))
}

// ---------------------------------------------------------------------------
// Ferramentas

#[derive(Debug, Deserialize)]
struct PontoCitado {
    #[serde(default)]
    ponto: Option<String>,
    evidencia: EvidenciaCitada,
}

#[derive(Debug, Deserialize)]
struct ArgsRegistrar {
    analise: String,
    #[serde(default)]
    id: Option<String>,
    nome: String,
    area: String,
    descricao: String,
    #[serde(default)]
    atores: Vec<String>,
    pontos_de_entrada: Vec<PontoCitado>,
    #[serde(default)]
    dados_lidos: Vec<String>,
    #[serde(default)]
    dados_gravados: Vec<String>,
    #[serde(default)]
    relacionadas: Vec<String>,
}

fn id_da_funcionalidade(texto: &str, campo: &str) -> Result<IdFuncionalidade, ErroFerramenta> {
    IdFuncionalidade::analisar(texto.trim()).map_err(|_| {
        ErroFerramenta::Argumentos(format!(
            "`{campo}`: `{texto}` não é o identificador de uma funcionalidade; use \
             F-<SIGLA>-NNN, como F-FAT-001"
        ))
    })
}

fn lista_de_textos(descricao: &str) -> Value {
    json!({
        "type": "array",
        "maxItems": 30,
        "items": { "type": "string", "minLength": 1, "maxLength": 200 },
        "description": descricao
    })
}

fn schema_do_ponto() -> Value {
    json!({
        "type": "object",
        "properties": {
            "ponto": {
                "type": "string",
                "minLength": 1,
                "maxLength": 500,
                "description": "O ponto de entrada como `pontos_de_entrada` mostra: a chave (`db/03_pkg.sql#PKG.PRC_X`) ou o nome (`PKG.PRC_X`, `FrmCancelamento`). Para um ponto fora da lista (menu montado por tabela, job agendado fora do repositório), descreva-o."
            },
            "evidencia": evidencias::schema_da_evidencia()
        },
        "required": ["evidencia"],
        "additionalProperties": false
    })
}

/// `pontos_de_entrada`: os pontos do mapa no escopo da análise e o tratamento de
/// cada um.
pub fn pontos_de_entrada() -> Ferramenta {
    Ferramenta::nova(
        "pontos_de_entrada",
        "Pontos de entrada",
        "Lista os pontos de entrada do projeto no escopo da análise de mapeamento, \
         calculados do mapa estrutural: telas, endpoints, lotes, rotinas de banco \
         públicas, serviços e integrações, cada um com a chave, o tipo, o arquivo e as \
         linhas, e o que já foi feito com ele (atribuído a uma funcionalidade, \
         justificado ou sem tratamento). Use para planejar o agrupamento em \
         funcionalidades e, antes de concluir, com `so_sem_tratamento`, para ver o que \
         falta. Resposta paginada.",
        com_paginacao(json!({
            "type": "object",
            "properties": {
                "analise": comum::schema_da_analise(),
                "so_sem_tratamento": {
                    "type": "boolean",
                    "description": "Só os pontos sem funcionalidade nem justificativa."
                }
            },
            "required": ["analise"],
            "additionalProperties": false
        })),
        |ambiente, args| {
            #[derive(Deserialize)]
            struct Args {
                analise: String,
                #[serde(default)]
                so_sem_tratamento: bool,
                #[serde(flatten)]
                pagina: ArgsPagina,
            }
            let args: Args = ler_argumentos(args)?;
            let id = comum::id_da_analise(&args.analise)?;
            let analise = ambiente.analises.carregar(&id)?;
            exigir_fluxo(&analise)?;
            let carregado = fontes::mapa(ambiente)?;
            let cat = catalogo(ambiente)?;
            let escopo = no_escopo(&analise, &carregado.mapa.pontos_de_entrada(), &cat);
            let faltam: BTreeSet<String> = nao_mapeados(&analise, &escopo)
                .into_iter()
                .map(|p| p.chave.clone())
                .collect();
            let itens: Vec<Item> = escopo
                .iter()
                .filter(|p| !args.so_sem_tratamento || faltam.contains(&p.chave))
                .map(|p| {
                    let (situacao, detalhe) = if let Some(f) = analise.dona_do_ponto(&p.chave) {
                        ("atribuido", format!("→ {} · {}", f.id, f.nome))
                    } else if let Some(j) = analise
                        .pontos_justificados
                        .iter()
                        .find(|j| j.chave == p.chave)
                    {
                        (
                            "justificado",
                            format!("→ {}: {}", j.motivo.nome(), j.justificativa),
                        )
                    } else {
                        let no_catalogo = cat
                            .por_chave(&p.chave)
                            .map(|i| format!("sem tratamento (no catálogo: {} · {})", i.id, i.nome))
                            .unwrap_or_else(|| "sem tratamento".into());
                        ("sem_tratamento", no_catalogo)
                    };
                    Item {
                        linha: format!(
                            "- `{}` [{}] linhas {}-{}{} {}",
                            p.chave,
                            p.tipo.nome(),
                            p.linhas.inicio,
                            p.linhas.fim,
                            p.detalhe
                                .as_deref()
                                .map(|d| format!(" ({})", em_uma_linha(d)))
                                .unwrap_or_default(),
                            detalhe
                        ),
                        dados: json!({
                            "chave": p.chave,
                            "tipo": p.tipo,
                            "nome": p.nome,
                            "arquivo": p.arquivo,
                            "linha_inicial": p.linhas.inicio,
                            "linha_final": p.linhas.fim,
                            "detalhe": p.detalhe,
                            "situacao": situacao,
                            "funcionalidade": analise.dona_do_ponto(&p.chave).map(|f| f.id.clone()),
                            "no_catalogo": cat.por_chave(&p.chave).map(|i| i.id.clone()),
                        }),
                    }
                })
                .collect();
            let pagina = Pagina::da_lista(itens, args.pagina);
            let escopo_txt = analise.escopo.clone().unwrap_or_default().descricao();
            let mut texto = format!(
                "Pontos de entrada de {escopo_txt}: {} no total, {} sem tratamento.\n{}\n\n",
                escopo.len(),
                faltam.len(),
                carregado.nota()
            );
            texto.push_str(&pagina.linhas());
            let _ = writeln!(texto, "\n{}", pagina.rodape());
            Ok(Saida {
                texto,
                dados: Some(json!({
                    "total": escopo.len(),
                    "sem_tratamento": faltam.len(),
                    "pontos": pagina.dados(),
                    "pagina": pagina.json(),
                    "mapa": carregado.json(),
                })),
            })
        },
    )
    .somente_leitura()
}

/// Identificador para registrar: o informado (do catálogo ou desta análise), o
/// do item do catálogo que tem um dos pontos ou nenhum (reservar um novo).
fn escolher_id(
    analise: &Analise,
    cat: &Catalogo,
    informado: Option<IdFuncionalidade>,
    pontos: &[PontoAtribuido],
) -> Result<Option<(IdFuncionalidade, bool)>, ErroFerramenta> {
    if let Some(id) = informado {
        let no_catalogo = cat.item(&id).is_some();
        if !no_catalogo && analise.funcionalidade(&id).is_none() {
            return Err(ErroFerramenta::Recusa(format!(
                "a funcionalidade {id} não existe no catálogo nem nesta análise; para uma \
                 funcionalidade nova, omita `id` (o identificador é reservado no registro)"
            )));
        }
        return Ok(Some((id, no_catalogo)));
    }
    for chave in pontos.iter().filter_map(|p| p.chave.as_deref()) {
        if let Some(item) = cat.por_chave(chave) {
            let livre = analise
                .funcionalidade(&item.id)
                .is_none_or(|f| !f.aberta() || f.chaves().any(|c| c == chave));
            if livre {
                return Ok(Some((item.id.clone(), true)));
            }
        }
    }
    Ok(None)
}

/// `registrar_funcionalidade`: uma funcionalidade com os pontos de entrada.
pub fn registrar_funcionalidade() -> Ferramenta {
    Ferramenta::nova(
        "registrar_funcionalidade",
        "Registrar funcionalidade",
        "Registra uma funcionalidade de negócio na análise de mapeamento: `nome` de \
         negócio (\"Parcelar fatura\", não `PKG_FAT.PRC_PARCELA`), `area` (de preferência \
         uma área do projeto.md), `descricao` em uma ou duas frases, `atores`, \
         `pontos_de_entrada` (cada um com o `ponto`, como `pontos_de_entrada` mostra, e a \
         `evidencia` do trecho que o define), `dados_lidos`, `dados_gravados` e \
         `relacionadas` (F-<SIGLA>-NNN). Uma funcionalidade é um objetivo do usuário e \
         pode ter vários pontos de entrada; um ponto fica numa funcionalidade só. Sem \
         ponto de entrada com evidência válida, é recusada. O identificador é o do \
         catálogo quando a funcionalidade já existe (informe `id` ou cite um ponto que o \
         catálogo já atribui a ela); senão, um novo F-<SIGLA>-NNN é reservado. \
         Registrar de novo com o mesmo `id` substitui a versão anterior.",
        json!({
            "type": "object",
            "properties": {
                "analise": comum::schema_da_analise(),
                "id": {
                    "type": "string",
                    "minLength": 1,
                    "description": "F-<SIGLA>-NNN de uma funcionalidade do catálogo ou desta análise, para atualizá-la. Omita para uma funcionalidade nova."
                },
                "nome": { "type": "string", "minLength": 1, "maxLength": 200, "description": "Nome de negócio, com verbo quando couber (\"Cancelar fatura\")." },
                "area": { "type": "string", "minLength": 1, "maxLength": 100, "description": "Área de negócio (\"Cobrança\"). Use as áreas do projeto.md quando houver; senão, proponha uma." },
                "descricao": { "type": "string", "minLength": 1, "maxLength": 1000, "description": "O que a funcionalidade faz para o usuário, em uma ou duas frases." },
                "atores": lista_de_textos("Quem usa (\"Atendente da retaguarda\", \"Lote noturno\")."),
                "pontos_de_entrada": {
                    "type": "array",
                    "minItems": 1,
                    "maxItems": 40,
                    "items": schema_do_ponto(),
                    "description": "Pontos de entrada, cada um com a evidência do trecho que o define."
                },
                "dados_lidos": lista_de_textos("Principais tabelas, arquivos ou entidades lidos."),
                "dados_gravados": lista_de_textos("Principais tabelas, arquivos ou entidades gravados."),
                "relacionadas": {
                    "type": "array",
                    "maxItems": 20,
                    "items": { "type": "string", "minLength": 1 },
                    "description": "Funcionalidades relacionadas (F-<SIGLA>-NNN)."
                }
            },
            "required": ["analise", "nome", "area", "descricao", "pontos_de_entrada"],
            "additionalProperties": false
        }),
        |ambiente, args| {
            let args: ArgsRegistrar = ler_argumentos(args)?;
            let id = comum::id_da_analise(&args.analise)?;
            let atual = ambiente.analises.carregar(&id)?;
            exigir_fluxo(&atual)?;
            let informado = args
                .id
                .as_deref()
                .map(|t| id_da_funcionalidade(t, "id"))
                .transpose()?;
            let relacionadas = args
                .relacionadas
                .iter()
                .map(|t| id_da_funcionalidade(t, "relacionadas"))
                .collect::<Result<Vec<_>, _>>()?;
            let area = resolver_area(&areas_do_projeto(ambiente), &args.area).ok_or_else(|| {
                ErroFerramenta::Argumentos(format!(
                    "`area`: `{}` não tem letras nem dígitos para formar a área",
                    args.area
                ))
            })?;
            if args.pontos_de_entrada.is_empty() {
                return Err(comum::recusa(ErroModelo::FuncionalidadeSemPontoDeEntrada));
            }
            let citadas: Vec<EvidenciaCitada> = args
                .pontos_de_entrada
                .iter()
                .map(|p| p.evidencia.clone())
                .collect();
            let mut recusas = Vec::new();
            let mut evidencias_ok = Vec::new();
            for (i, c) in citadas.iter().enumerate() {
                match evidencias::validar(&ambiente.escopo, c) {
                    Ok(e) => evidencias_ok.push(e),
                    Err(r) => recusas.push(format!("- pontos_de_entrada[{i}].evidencia ({r})")),
                }
            }
            if !recusas.is_empty() {
                return Err(ErroFerramenta::Recusa(format!(
                    "{} de {} pontos de entrada com evidência recusada; a funcionalidade não \
                     foi registrada. Toda funcionalidade precisa de pontos de entrada com \
                     evidência válida. Corrija e chame de novo:\n{}",
                    recusas.len(),
                    citadas.len(),
                    recusas.join("\n")
                )));
            }
            let carregado = fontes::mapa(ambiente).ok();
            let todos = carregado
                .as_ref()
                .map(|c| c.mapa.pontos_de_entrada())
                .unwrap_or_default();
            let pontos: Vec<PontoAtribuido> = args
                .pontos_de_entrada
                .iter()
                .zip(evidencias_ok)
                .map(|(citado, evidencia)| {
                    match casar_ponto(&todos, citado.ponto.as_deref(), &evidencia) {
                        Some(p) => PontoAtribuido {
                            chave: Some(p.chave.clone()),
                            tipo: Some(p.tipo),
                            nome: p.nome.clone(),
                            evidencia,
                        },
                        None => PontoAtribuido {
                            chave: None,
                            tipo: None,
                            nome: citado
                                .ponto
                                .clone()
                                .filter(|p| !p.trim().is_empty())
                                .map(|p| em_uma_linha(&p))
                                .unwrap_or_else(|| {
                                    em_uma_linha(&evidencia.trecho).chars().take(80).collect()
                                }),
                            evidencia,
                        },
                    }
                })
                .collect();
            let cat = catalogo(ambiente)?;
            let escolhido = escolher_id(&atual, &cat, informado, &pontos)?;
            let nova = NovaFuncionalidade {
                nome: args.nome,
                area: area.slug.clone(),
                area_nome: area.nome.clone(),
                descricao: args.descricao,
                atores: args.atores,
                pontos,
                dados_lidos: args.dados_lidos,
                dados_gravados: args.dados_gravados,
                relacionadas: relacionadas.clone(),
                do_catalogo: escolhido.as_ref().is_some_and(|(_, c)| *c),
                sessao: None,
            };
            let sigla = area.sigla.clone();
            let registrado = comum::alterar(ambiente, &id, Mudanca::Funcionalidade, |a, agora| {
                let fid = match escolhido {
                    Some((fid, _)) => fid,
                    None => {
                        let maior = ambiente
                            .analises
                            .maior_numero(&a.id, |x| x.maior_funcionalidade(&sigla))
                            .max(a.maior_funcionalidade(&sigla))
                            .max(cat.maior_numero(&sigla));
                        IdFuncionalidade::novo(&sigla, maior + 1)
                            .map_err(|_| ErroModelo::CampoVazio("area"))?
                    }
                };
                let substituida = a.registrar_funcionalidade(nova, fid.clone(), agora)?;
                Ok((fid, substituida))
            })?;
            let (fid, substituida) = registrado.resultado.clone();
            let f = registrado
                .analise
                .funcionalidade(&fid)
                .ok_or_else(|| ErroFerramenta::Interno(format!("{fid} não foi gravada")))?;
            let desconhecidas: Vec<String> = relacionadas
                .iter()
                .filter(|r| registrado.analise.funcionalidade(r).is_none() && cat.item(r).is_none())
                .map(ToString::to_string)
                .collect();
            let restantes = carregado.as_ref().map(|c| {
                let escopo = no_escopo(&registrado.analise, &c.mapa.pontos_de_entrada(), &cat);
                nao_mapeados(&registrado.analise, &escopo).len()
            });
            let mut dados = json!({
                "funcionalidade": fid,
                "substituida": substituida,
                "do_catalogo": f.do_catalogo,
                "area": { "slug": f.area, "nome": f.area_nome },
                "pontos_de_entrada": f.pontos.iter().map(|p| json!({
                    "ponto": p.nome,
                    "chave": p.chave,
                    "tipo": p.tipo,
                    "no_mapa": p.chave.is_some(),
                })).collect::<Vec<_>>(),
                "pontos_sem_tratamento": restantes,
            });
            if !desconhecidas.is_empty() {
                dados["aviso_relacionadas"] = json!(format!(
                    "relacionadas ainda não registradas nem no catálogo: {}; registre-as \
                     ou corrija",
                    desconhecidas.join(", ")
                ));
            }
            Ok(comum::resposta(ambiente, &registrado, dados))
        },
    )
}

/// `tratar_ponto_de_entrada`: atribui a uma funcionalidade ou justifica.
pub fn tratar_ponto_de_entrada() -> Ferramenta {
    Ferramenta::nova(
        "tratar_ponto_de_entrada",
        "Tratar ponto de entrada",
        "Trata um ponto de entrada do mapa que está sem funcionalidade: `atribuir` à \
         funcionalidade `funcionalidade` (F-<SIGLA>-NNN, já registrada nesta análise; a \
         evidência é a declaração do ponto no código) ou justificar por que fica sem \
         funcionalidade, com `motivo` (`codigo_morto`, `utilitario` ou `outro`) e \
         `justificativa`. Antes de concluir, todo ponto do escopo precisa estar numa \
         funcionalidade ou justificado.",
        json!({
            "type": "object",
            "properties": {
                "analise": comum::schema_da_analise(),
                "ponto": {
                    "type": "string",
                    "minLength": 1,
                    "description": "Chave do ponto, como `pontos_de_entrada` mostra (`db/03_pkg.sql#PKG.FN_X`)."
                },
                "tratamento": { "type": "string", "enum": ["atribuir", "justificar"] },
                "funcionalidade": { "type": "string", "minLength": 1, "description": "No `atribuir`: F-<SIGLA>-NNN." },
                "motivo": { "type": "string", "enum": ["codigo_morto", "utilitario", "outro"], "description": "No `justificar`." },
                "justificativa": { "type": "string", "minLength": 1, "maxLength": 1000, "description": "No `justificar`: por que o ponto fica sem funcionalidade (\"lê parâmetros para as rotinas de cobrança\")." }
            },
            "required": ["analise", "ponto", "tratamento"],
            "additionalProperties": false
        }),
        |ambiente, args| {
            #[derive(Deserialize)]
            struct Args {
                analise: String,
                ponto: String,
                tratamento: String,
                #[serde(default)]
                funcionalidade: Option<String>,
                #[serde(default)]
                motivo: Option<MotivoSemFuncionalidade>,
                #[serde(default)]
                justificativa: Option<String>,
            }
            let args: Args = ler_argumentos(args)?;
            let id = comum::id_da_analise(&args.analise)?;
            let atual = ambiente.analises.carregar(&id)?;
            exigir_fluxo(&atual)?;
            let carregado = fontes::mapa(ambiente)?;
            let todos = carregado.mapa.pontos_de_entrada();
            let chave = args.ponto.trim();
            let ponto = todos.iter().find(|p| p.chave == chave).ok_or_else(|| {
                ErroFerramenta::Recusa(format!(
                    "o ponto `{chave}` não está entre os pontos de entrada do mapa; use a chave \
                     como `pontos_de_entrada` mostra. Um ponto fora da lista entra numa \
                     funcionalidade por `registrar_funcionalidade`, com a evidência"
                ))
            })?;
            let registrado = match args.tratamento.as_str() {
                "atribuir" => {
                    let fid = id_da_funcionalidade(
                        args.funcionalidade.as_deref().ok_or_else(|| {
                            ErroFerramenta::Argumentos(
                                "no `atribuir`, informe `funcionalidade` (F-<SIGLA>-NNN)".into(),
                            )
                        })?,
                        "funcionalidade",
                    )?;
                    let evidencia = evidencia_da_declaracao(ambiente, ponto)?;
                    let atribuido = PontoAtribuido {
                        chave: Some(ponto.chave.clone()),
                        tipo: Some(ponto.tipo),
                        nome: ponto.nome.clone(),
                        evidencia,
                    };
                    comum::alterar(ambiente, &id, Mudanca::Funcionalidade, |a, _| {
                        a.atribuir_ponto(&fid, atribuido)
                    })?
                }
                "justificar" => {
                    let motivo = args.motivo.ok_or_else(|| {
                        ErroFerramenta::Argumentos(
                            "no `justificar`, informe `motivo` (codigo_morto, utilitario ou \
                             outro) e `justificativa`"
                                .into(),
                        )
                    })?;
                    let justificativa = args.justificativa.unwrap_or_default();
                    let justificado = PontoJustificado {
                        chave: ponto.chave.clone(),
                        tipo: ponto.tipo,
                        nome: ponto.nome.clone(),
                        arquivo: ponto.arquivo.clone(),
                        motivo,
                        justificativa,
                        sessao: None,
                        registrado_ms: 0,
                    };
                    comum::alterar(ambiente, &id, Mudanca::Funcionalidade, |a, agora| {
                        a.justificar_ponto(PontoJustificado {
                            registrado_ms: agora,
                            ..justificado
                        })
                    })?
                }
                outro => {
                    return Err(ErroFerramenta::Argumentos(format!(
                        "`tratamento`: `{outro}` não é atribuir nem justificar"
                    )));
                }
            };
            let cat = catalogo(ambiente)?;
            let escopo = no_escopo(&registrado.analise, &todos, &cat);
            let restantes = nao_mapeados(&registrado.analise, &escopo).len();
            Ok(comum::resposta(
                ambiente,
                &registrado,
                json!({
                    "ponto": ponto.chave,
                    "tratamento": args.tratamento,
                    "pontos_sem_tratamento": restantes,
                }),
            ))
        },
    )
}

/// Evidência da declaração de um ponto do mapa: a primeira linha não vazia do
/// símbolo.
fn evidencia_da_declaracao(
    ambiente: &Ambiente,
    ponto: &PontoEntrada,
) -> Result<cba_core::analise::Evidencia, ErroFerramenta> {
    let arquivo = ArquivoDoProjeto::abrir(&ambiente.escopo, &ponto.arquivo).map_err(|m| {
        ErroFerramenta::Recusa(format!(
            "o arquivo `{}` não pôde ser lido: {m}",
            ponto.arquivo
        ))
    })?;
    let linhas: Vec<&str> = arquivo.texto.lines().collect();
    let (inicio, fim) = (ponto.linhas.inicio as usize, ponto.linhas.fim as usize);
    for n in inicio..=fim.min(linhas.len()) {
        let texto = linhas[n - 1].trim();
        if texto.is_empty() {
            continue;
        }
        let l = cba_kb::frontmatter::Linhas::novo(n as u32, n as u32).expect("linha válida");
        return arquivo.conferir(l, texto).map_err(|m| {
            ErroFerramenta::Recusa(format!(
                "a declaração de `{}` não confere com o código ({m}); atribua com \
                 `registrar_funcionalidade`, citando a evidência",
                ponto.nome
            ))
        });
    }
    Err(ErroFerramenta::Recusa(format!(
        "a declaração de `{}` não foi achada em `{}`; atribua com `registrar_funcionalidade`, \
         citando a evidência",
        ponto.nome, ponto.arquivo
    )))
}

/// No fluxo `mapear`: ao menos uma funcionalidade e nenhum ponto do escopo sem
/// tratamento.
pub fn conferir_para_concluir(
    ambiente: &Ambiente,
    analise: &Analise,
) -> Result<(), ErroFerramenta> {
    let id = &analise.id;
    if analise.funcionalidades_no_documento().next().is_none() {
        return Err(ErroFerramenta::Recusa(format!(
            "a análise {id} ainda não tem funcionalidades. Veja os pontos de entrada com \
             `pontos_de_entrada`, agrupe-os em funcionalidades de negócio com \
             `registrar_funcionalidade` e chame `concluir_analise` de novo"
        )));
    }
    let (_, faltam) = completude(ambiente, analise)?;
    if faltam.is_empty() {
        return Ok(());
    }
    const MOSTRADOS: usize = 30;
    let mut t = format!(
        "{} ponto(s) de entrada sem funcionalidade nem justificativa. Para cada um, \
         atribua a uma funcionalidade (`tratar_ponto_de_entrada` com `atribuir`, ou \
         `registrar_funcionalidade`), crie uma funcionalidade nova ou justifique \
         (`tratar_ponto_de_entrada` com `justificar`: código morto, utilitário ou outro \
         motivo), e chame `concluir_analise` de novo:",
        faltam.len()
    );
    for p in faltam.iter().take(MOSTRADOS) {
        let _ = write!(t, "\n- {} · chave `{}`", linha_do_ponto(p), p.chave);
    }
    if faltam.len() > MOSTRADOS {
        let _ = write!(
            t,
            "\n- … e mais {}; veja todos com `pontos_de_entrada` (so_sem_tratamento: true)",
            faltam.len() - MOSTRADOS
        );
    }
    Err(ErroFerramenta::Recusa(t))
}

/// Resumo para `retomar_analise`: o escopo, as funcionalidades, os pontos
/// justificados e o catálogo atual (para manter os identificadores).
pub fn para_retomar(ambiente: &Ambiente, analise: &Analise) -> (String, Value) {
    let mut t = String::from("## Funcionalidades\n\n");
    let escopo = analise.escopo.clone().unwrap_or_default();
    let _ = writeln!(t, "Escopo: {}.\n", escopo.descricao());
    if analise.funcionalidades.is_empty() {
        t.push_str("Nenhuma funcionalidade registrada.\n\n");
    }
    for f in &analise.funcionalidades {
        let _ = writeln!(
            t,
            "### {} · {} [{}{}] (área {})\n\n{}",
            f.id,
            f.nome,
            rotulo_situacao(f),
            f.juntada_em
                .as_ref()
                .map(|j| format!(" em {j}"))
                .unwrap_or_default(),
            f.area_nome,
            f.descricao
        );
        for p in &f.pontos {
            let _ = writeln!(
                t,
                "- {} (`{}`:{}){}",
                p.nome,
                p.evidencia.arquivo,
                p.evidencia.linhas.inicio,
                if p.chave.is_none() {
                    " · fora da lista do mapa"
                } else {
                    ""
                }
            );
        }
        t.push('\n');
    }
    if !analise.pontos_justificados.is_empty() {
        t.push_str("### Pontos justificados\n\n");
        for j in &analise.pontos_justificados {
            let _ = writeln!(
                t,
                "- `{}`: {} · {}",
                j.chave,
                j.motivo.nome(),
                j.justificativa
            );
        }
        t.push('\n');
    }
    let cat = Catalogo::carregar(&ambiente.kb).unwrap_or_default();
    if !cat.funcionalidades.is_empty() {
        let _ = writeln!(
            t,
            "### Catálogo atual ({} funcionalidades)\n\nMantenha os identificadores: \
             registre com `id` as que continuam existindo.\n",
            cat.funcionalidades.len()
        );
        for i in cat.ordenadas() {
            let _ = writeln!(
                t,
                "- {} · {} (área {}, {})",
                i.id,
                i.nome,
                i.area_nome,
                i.situacao.nome()
            );
        }
        t.push('\n');
    }
    let dados = json!({
        "escopo": escopo,
        "funcionalidades": analise.funcionalidades.iter().map(|f| json!({
            "id": f.id,
            "nome": f.nome,
            "area": f.area,
            "situacao": f.situacao,
            "juntada_em": f.juntada_em,
            "pontos": f.pontos.iter().map(|p| json!({"nome": p.nome, "chave": p.chave})).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "pontos_justificados": analise.pontos_justificados.iter().map(|j| json!({
            "chave": j.chave, "motivo": j.motivo, "justificativa": j.justificativa
        })).collect::<Vec<_>>(),
        "catalogo": cat.ordenadas().iter().map(|i| json!({
            "id": i.id, "nome": i.nome, "area": i.area, "situacao": i.situacao,
        })).collect::<Vec<_>>(),
    });
    (t, dados)
}

/// O catálogo atual em Markdown, para o contexto do roteiro.
pub fn catalogo_para_o_contexto(ambiente: &Ambiente) -> String {
    let cat = Catalogo::carregar(&ambiente.kb).unwrap_or_default();
    if cat.funcionalidades.is_empty() {
        return "O catálogo do projeto ainda está vazio: todas as funcionalidades são novas."
            .into();
    }
    let mut t = format!(
        "O catálogo do projeto tem {} funcionalidades. Mantenha os identificadores das que \
         continuam existindo (registre com `id`):\n",
        cat.funcionalidades.len()
    );
    for i in cat.ordenadas() {
        let pontos: Vec<&str> = i.pontos.iter().map(|p| p.nome.as_str()).collect();
        let _ = write!(
            t,
            "\n- {} · {} (área {}): {}",
            i.id,
            i.nome,
            i.area_nome,
            pontos.join(", ")
        );
    }
    t
}

/// Mapa usado na aprovação: as chaves dos pontos de entrada atuais.
pub fn chaves_do_mapa(mapa: &Mapa) -> BTreeSet<String> {
    mapa.pontos_de_entrada()
        .into_iter()
        .map(|p| p.chave)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cba_index::TipoEntrada;
    use cba_kb::frontmatter::Linhas;

    fn pe(chave: &str, tipo: TipoEntrada, i: u32, f: u32) -> PontoEntrada {
        let (arquivo, nome) = chave.split_once('#').unwrap();
        PontoEntrada {
            chave: chave.into(),
            tipo,
            nome: nome.into(),
            arquivo: arquivo.into(),
            linhas: cba_index::Linhas::new(i, f),
            detalhe: None,
        }
    }

    fn ev(arquivo: &str, i: u32, f: u32) -> cba_core::analise::Evidencia {
        cba_core::analise::Evidencia {
            arquivo: CaminhoRelativo::novo(arquivo).unwrap(),
            linhas: Linhas::novo(i, f).unwrap(),
            trecho: "x".into(),
            hash: "h".into(),
            commit: None,
            estado: Default::default(),
        }
    }

    #[test]
    fn casa_pela_chave_pelo_nome_e_pela_evidencia() {
        let pontos = vec![
            pe("db/a.sql#PKG.PRC_A", TipoEntrada::RotinaBanco, 10, 20),
            pe("db/a.sql#PKG.PRC_B", TipoEntrada::RotinaBanco, 22, 30),
            pe("db/b.sql#OUTRO.PRC_A", TipoEntrada::RotinaBanco, 1, 5),
        ];
        let e = ev("db/a.sql", 12, 12);
        assert_eq!(
            casar_ponto(&pontos, Some("db/a.sql#PKG.PRC_B"), &e)
                .unwrap()
                .nome,
            "PKG.PRC_B"
        );
        // Ambíguo pelo nome: vale o do arquivo da evidência.
        assert_eq!(
            casar_ponto(&pontos, Some("prc_a"), &e).unwrap().chave,
            "db/a.sql#PKG.PRC_A"
        );
        // Sem nome: pela evidência.
        assert_eq!(
            casar_ponto(&pontos, None, &ev("db/a.sql", 25, 26))
                .unwrap()
                .nome,
            "PKG.PRC_B"
        );
        assert!(casar_ponto(&pontos, Some("menu por tabela"), &ev("db/c.sql", 1, 1)).is_none());
    }

    #[test]
    fn areas_do_projeto_e_propostas() {
        let areas = vec![Area {
            id: "cobranca".into(),
            nome: "Cobrança".into(),
            sigla: Some("FAT".into()),
        }];
        let a = resolver_area(&areas, "cobrança").unwrap();
        assert_eq!(
            (a.slug.as_str(), a.nome.as_str(), a.sigla.as_str()),
            ("cobranca", "Cobrança", "FAT")
        );
        let b = resolver_area(&areas, "Parcelamento").unwrap();
        assert_eq!(
            (b.slug.as_str(), b.nome.as_str(), b.sigla.as_str()),
            ("parcelamento", "Parcelamento", "PAR")
        );
        assert!(resolver_area(&areas, "??").is_none());
    }

    #[test]
    fn escopo_e_nao_mapeados() {
        use cba_core::funcionalidades::NovaFuncionalidade;
        let pontos = vec![
            pe("db/a.sql#A", TipoEntrada::RotinaBanco, 1, 2),
            pe("db/a.sql#B", TipoEntrada::RotinaBanco, 3, 4),
            pe("front/t.ts#T", TipoEntrada::Tela, 1, 9),
        ];
        let mut a = Analise::nova(
            cba_kb::Serie::Analise.id(1),
            "p",
            Fluxo::Mapear,
            "Mapa",
            "",
            1,
        );
        a.registrar_funcionalidade(
            NovaFuncionalidade {
                nome: "A".into(),
                area: "cobranca".into(),
                area_nome: "Cobrança".into(),
                descricao: "D.".into(),
                atores: vec![],
                pontos: vec![PontoAtribuido {
                    chave: Some("db/a.sql#A".into()),
                    tipo: None,
                    nome: "A".into(),
                    evidencia: ev("db/a.sql", 1, 1),
                }],
                dados_lidos: vec![],
                dados_gravados: vec![],
                relacionadas: vec![],
                do_catalogo: false,
                sessao: None,
            },
            IdFuncionalidade::novo("COB", 1).unwrap(),
            1,
        )
        .unwrap();
        let cat = Catalogo::default();
        let todos = no_escopo(&a, &pontos, &cat);
        assert_eq!(todos.len(), 3);
        let faltam: Vec<&str> = nao_mapeados(&a, &todos)
            .iter()
            .map(|p| p.chave.as_str())
            .collect();
        assert_eq!(faltam, ["db/a.sql#B", "front/t.ts#T"]);
        a.escopo = Some(EscopoMapa {
            pasta: Some("db".into()),
            area: None,
        });
        assert_eq!(no_escopo(&a, &pontos, &cat).len(), 2);
        a.escopo = Some(EscopoMapa {
            pasta: None,
            area: Some("cobranca".into()),
        });
        // Na área, só o que a análise atribuiu (o catálogo está vazio).
        assert_eq!(no_escopo(&a, &pontos, &cat).len(), 1);

        let textos = textos(&a);
        assert_eq!(textos[0].texto, "### Cobrança");
        assert!(
            textos[1].texto.starts_with("#### F-COB-001 · A"),
            "{}",
            textos[1].texto
        );
        assert_eq!(textos[1].citacoes.len(), 1);
    }
}
