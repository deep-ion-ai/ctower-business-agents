//! Vínculo dos documentos da base com as funcionalidades do mapa (#287).
//!
//! Para a documentação publicada se organizar por funcionalidade (#288), a
//! análise aprovada, as regras e os termos guardam no frontmatter as
//! funcionalidades do mapa a que se referem (`funcionalidades: [F-COB-001]`), e o
//! documento negocial guarda a que explica (`funcionalidade: F-COB-001`).
//!
//! - [`propostas`]: as funcionalidades que a janela de aprovação propõe, nesta
//!   ordem: as já gravadas no documento da análise (uma aprovação anterior), a da
//!   sessão ([`Analise::funcionalidade`], a análise aberta pelo painel da
//!   funcionalidade ou a explicação) e, sem nenhuma delas, a sugestão pelas
//!   evidências ([`sugerir_por_evidencias`]). O analista confirma ou troca.
//! - [`sugerir_por_evidencias`]: as funcionalidades **aceitas** cujos pontos de
//!   entrada têm evidência num arquivo citado pelas afirmações confirmadas ou
//!   pelos termos aceitos, da que mais arquivos tem em comum para a que menos.
//! - [`religar`]: liga os documentos aprovados antes do vínculo, sem adivinhar: a
//!   análise pela funcionalidade da sessão, a regra e o termo pela análise de
//!   origem e o documento negocial pela funcionalidade do mapa que aponta para
//!   ele. Documento que já tem vínculo não muda.
//!
//! Funcionalidade juntada a outra vale pela outra ([`resolver`]); descartada (ou
//! fora do mapa) não entra.

use std::collections::{BTreeMap, BTreeSet};

use cba_core::analise::{Analise, EstadoAfirmacao, EstadoTermo};
use cba_core::analises::Analises;
use cba_core::funcionalidades::{Funcionalidade, MapaFuncionalidades, SituacaoFuncionalidade};
use cba_kb::frontmatter::{
    FrontmatterAnalise, FrontmatterFuncionalidade, FrontmatterLivre, FrontmatterRegra,
    FrontmatterTermo, StatusAnalise, TipoDocumento,
};
use cba_kb::{CaminhoRelativo, IdFuncionalidade};
use serde::Serialize;

/// De onde vieram as funcionalidades propostas na aprovação.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum OrigemDoVinculo {
    /// Gravadas no documento da análise por uma aprovação anterior.
    Documento,
    /// A funcionalidade da sessão (painel da funcionalidade ou explicação).
    Sessao,
    /// Sugeridas pelas evidências: o analista precisa conferir.
    Evidencias,
    /// Nenhuma funcionalidade a propor.
    Nenhuma,
}

/// Funcionalidade do mapa, para a janela de aprovação.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FuncionalidadeResumida {
    pub id: IdFuncionalidade,
    pub nome: String,
    /// Slug da área (`cobranca`).
    pub area: String,
    pub area_nome: String,
}

impl FuncionalidadeResumida {
    pub fn de(f: &Funcionalidade) -> Self {
        Self {
            id: f.id.clone(),
            nome: f.nome.clone(),
            area: f.area.clone(),
            area_nome: f.area_nome.clone(),
        }
    }
}

/// A funcionalidade aberta que vale por `id`: ela mesma ou, se foi juntada, a
/// funcionalidade em que foi juntada. `None` se descartada ou fora do mapa.
pub fn resolver<'m>(
    mapa: &'m MapaFuncionalidades,
    id: &IdFuncionalidade,
) -> Option<&'m Funcionalidade> {
    let mut atual = mapa.funcionalidade(id)?;
    // Limite contra ciclos num estado editado à mão.
    for _ in 0..mapa.funcionalidades.len() {
        match (&atual.situacao, &atual.juntada_em) {
            (SituacaoFuncionalidade::Juntada, Some(destino)) => {
                atual = mapa.funcionalidade(destino)?;
            }
            _ => break,
        }
    }
    atual.aberta().then_some(atual)
}

/// Os identificadores que valem (juntadas trocadas pela de destino), sem
/// repetição e na ordem dada. Os que não estão no mapa vão para `fora`.
pub fn normalizar(
    mapa: &MapaFuncionalidades,
    ids: &[IdFuncionalidade],
) -> (Vec<IdFuncionalidade>, Vec<IdFuncionalidade>) {
    let mut validos = Vec::new();
    let mut fora = Vec::new();
    for id in ids {
        match resolver(mapa, id) {
            Some(f) if !validos.contains(&f.id) => validos.push(f.id.clone()),
            Some(_) => {}
            None => fora.push(id.clone()),
        }
    }
    (validos, fora)
}

/// Funcionalidades aceitas cujos pontos de entrada têm evidência num arquivo
/// citado pelas afirmações confirmadas ou pelos termos aceitos da análise, da
/// que mais arquivos tem em comum para a que menos (empate: pelo identificador).
pub fn sugerir_por_evidencias(
    analise: &Analise,
    mapa: &MapaFuncionalidades,
) -> Vec<IdFuncionalidade> {
    let afirmacoes = analise
        .afirmacoes
        .iter()
        .filter(|a| a.estado == EstadoAfirmacao::Confirmada)
        .flat_map(|a| &a.evidencias);
    let termos = analise
        .termos
        .iter()
        .filter(|t| t.estado == EstadoTermo::Aceito)
        .flat_map(|t| &t.evidencias);
    let citados: BTreeSet<&str> = afirmacoes
        .chain(termos)
        .map(|e| e.arquivo.as_str())
        .collect();
    if citados.is_empty() {
        return Vec::new();
    }
    let mut pontuadas: Vec<(usize, &IdFuncionalidade)> = mapa
        .funcionalidades
        .iter()
        .filter(|f| f.situacao == SituacaoFuncionalidade::Aceita)
        .filter_map(|f| {
            let comuns: BTreeSet<&str> = f
                .pontos
                .iter()
                .map(|p| p.evidencia.arquivo.as_str())
                .filter(|a| citados.contains(a))
                .collect();
            (!comuns.is_empty()).then_some((comuns.len(), &f.id))
        })
        .collect();
    pontuadas.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(b.1)));
    pontuadas.into_iter().map(|(_, id)| id.clone()).collect()
}

/// Funcionalidades gravadas no documento da análise (vazio se ele não abre).
pub fn do_documento(kb: &cba_kb::Projeto, analise: &Analise) -> Vec<IdFuncionalidade> {
    kb.ler::<FrontmatterAnalise>(&analise.documento)
        .map(|d| d.frontmatter.funcionalidades)
        .unwrap_or_default()
}

/// As funcionalidades que a aprovação propõe para a análise (veja o módulo).
/// `no_documento` são as já gravadas no documento da análise ([`do_documento`]).
pub fn propostas(
    analise: &Analise,
    no_documento: &[IdFuncionalidade],
    mapa: &MapaFuncionalidades,
) -> (Vec<IdFuncionalidade>, OrigemDoVinculo) {
    let (gravadas, _) = normalizar(mapa, no_documento);
    if !gravadas.is_empty() {
        return (gravadas, OrigemDoVinculo::Documento);
    }
    if let Some(f) = analise
        .funcionalidade
        .as_ref()
        .and_then(|id| resolver(mapa, id))
    {
        return (vec![f.id.clone()], OrigemDoVinculo::Sessao);
    }
    let sugeridas = sugerir_por_evidencias(analise, mapa);
    if sugeridas.is_empty() {
        (sugeridas, OrigemDoVinculo::Nenhuma)
    } else {
        (sugeridas, OrigemDoVinculo::Evidencias)
    }
}

/// Área (slug) da primeira funcionalidade que vale entre `ids`.
pub fn area_das(mapa: &MapaFuncionalidades, ids: &[IdFuncionalidade]) -> Option<String> {
    ids.iter()
        .find_map(|id| resolver(mapa, id))
        .map(|f| f.area.clone())
}

/// `existentes` seguidas das `novas` que ainda não estão nelas.
pub fn somar(existentes: &[IdFuncionalidade], novas: &[IdFuncionalidade]) -> Vec<IdFuncionalidade> {
    let mut saida = existentes.to_vec();
    for id in novas {
        if !saida.contains(id) {
            saida.push(id.clone());
        }
    }
    saida
}

// ---------------------------------------------------------------------------
// Religar documentos antigos

/// O que [`religar`] gravou.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Religacao {
    pub analises: Vec<CaminhoRelativo>,
    pub regras: Vec<CaminhoRelativo>,
    pub termos: Vec<CaminhoRelativo>,
    /// Documentos negociais que ganharam o `funcionalidade`.
    pub negociais: Vec<CaminhoRelativo>,
    /// Regras que continuam sem funcionalidade (a origem não leva a uma): a
    /// documentação publicada as agrupa pela área.
    pub regras_sem_vinculo: usize,
    /// Documentos ilegíveis ou que não puderam ser gravados (o resto segue).
    pub avisos: Vec<String>,
}

impl Religacao {
    /// Quantos documentos ganharam vínculo.
    pub fn total(&self) -> usize {
        self.analises.len() + self.regras.len() + self.termos.len() + self.negociais.len()
    }
}

/// Liga às funcionalidades do mapa os documentos aprovados antes do vínculo
/// (#287): análises aprovadas pela funcionalidade da sessão; regras e termos pela
/// análise de origem; documentos negociais pela funcionalidade do mapa que
/// aponta para eles (ou pela análise de origem). Só preenche o que está vazio e é
/// idempotente. Falha só se o mapa ou a lista de documentos não puderem ser lidos.
pub fn religar(kb: &cba_kb::Projeto, analises: &Analises) -> Result<Religacao, String> {
    let mapa = MapaFuncionalidades::carregar(kb).map_err(|e| e.to_string())?;
    let mut saida = Religacao::default();
    let mut por_tipo: BTreeMap<&'static str, Vec<CaminhoRelativo>> = BTreeMap::new();
    for caminho in kb.documentos().map_err(|e| e.to_string())? {
        let Ok(livre) = kb.ler::<FrontmatterLivre>(&caminho) else {
            continue;
        };
        let tipo = match livre.frontmatter.tipo() {
            Some(TipoDocumento::Regra) => "regra",
            Some(TipoDocumento::Termo) => "termo",
            Some(TipoDocumento::Funcionalidade | TipoDocumento::CasoDeUso)
                if caminho.partes().next() == Some(cba_kb::layout::FUNCIONALIDADES) =>
            {
                "negocial"
            }
            Some(t) if <FrontmatterAnalise as cba_kb::Frontmatter>::TIPOS.contains(&t) => "analise",
            _ => continue,
        };
        por_tipo.entry(tipo).or_default().push(caminho);
    }

    // Análises primeiro: as regras e os termos herdam delas.
    let mut da_analise: BTreeMap<CaminhoRelativo, Vec<IdFuncionalidade>> = BTreeMap::new();
    for caminho in por_tipo.remove("analise").unwrap_or_default() {
        let mut doc = match kb.ler::<FrontmatterAnalise>(&caminho) {
            Ok(d) => d,
            Err(e) => {
                saida.avisos.push(e.to_string());
                continue;
            }
        };
        let (gravadas, _) = normalizar(&mapa, &doc.frontmatter.funcionalidades);
        if !doc.frontmatter.funcionalidades.is_empty() {
            da_analise.insert(caminho, gravadas);
            continue;
        }
        if doc.frontmatter.status != StatusAnalise::Aprovada {
            continue;
        }
        let da_sessao = analises
            .carregar(&doc.frontmatter.id)
            .ok()
            .and_then(|a| a.funcionalidade)
            .and_then(|id| resolver(&mapa, &id).map(|f| f.id.clone()));
        let Some(id) = da_sessao else {
            continue;
        };
        doc.frontmatter.funcionalidades = vec![id.clone()];
        match kb.gravar(&caminho, &doc) {
            Ok(()) => {
                saida.analises.push(caminho.clone());
                da_analise.insert(caminho, vec![id]);
            }
            Err(e) => saida.avisos.push(e.to_string()),
        }
    }
    let pela_origem = |origem: Option<&CaminhoRelativo>| {
        origem
            .and_then(|o| da_analise.get(o))
            .filter(|l| !l.is_empty())
            .cloned()
    };

    for caminho in por_tipo.remove("regra").unwrap_or_default() {
        let mut doc = match kb.ler::<FrontmatterRegra>(&caminho) {
            Ok(d) => d,
            Err(e) => {
                saida.avisos.push(e.to_string());
                continue;
            }
        };
        if !doc.frontmatter.funcionalidades.is_empty() {
            continue;
        }
        let Some(lista) = pela_origem(doc.frontmatter.origem.as_ref()) else {
            saida.regras_sem_vinculo += 1;
            continue;
        };
        doc.frontmatter.funcionalidades = lista;
        match kb.gravar(&caminho, &doc) {
            Ok(()) => saida.regras.push(caminho),
            Err(e) => saida.avisos.push(e.to_string()),
        }
    }

    for caminho in por_tipo.remove("termo").unwrap_or_default() {
        let mut doc = match kb.ler::<FrontmatterTermo>(&caminho) {
            Ok(d) => d,
            Err(e) => {
                saida.avisos.push(e.to_string());
                continue;
            }
        };
        if !doc.frontmatter.funcionalidades.is_empty() {
            continue;
        }
        let Some(lista) = pela_origem(doc.frontmatter.origem.as_ref()) else {
            continue;
        };
        doc.frontmatter.funcionalidades = lista;
        match kb.gravar(&caminho, &doc) {
            Ok(()) => saida.termos.push(caminho),
            Err(e) => saida.avisos.push(e.to_string()),
        }
    }

    for caminho in por_tipo.remove("negocial").unwrap_or_default() {
        let mut doc = match kb.ler::<FrontmatterFuncionalidade>(&caminho) {
            Ok(d) => d,
            Err(e) => {
                saida.avisos.push(e.to_string());
                continue;
            }
        };
        if doc.frontmatter.funcionalidade.is_some() {
            continue;
        }
        let do_mapa = mapa
            .funcionalidades
            .iter()
            .filter(|f| f.aberta())
            .find(|f| f.documentos.iter().any(|d| d.caminho == caminho))
            .map(|f| f.id.clone());
        let Some(id) = do_mapa.or_else(|| {
            pela_origem(doc.frontmatter.origem.as_ref()).and_then(|l| l.first().cloned())
        }) else {
            continue;
        };
        doc.frontmatter.funcionalidade = Some(id);
        match kb.gravar(&caminho, &doc) {
            Ok(()) => saida.negociais.push(caminho),
            Err(e) => saida.avisos.push(e.to_string()),
        }
    }
    Ok(saida)
}

/// Grava `funcionalidades` no documento da análise, se mudar. Devolve se gravou.
pub fn gravar_na_analise(
    kb: &cba_kb::Projeto,
    documento: &CaminhoRelativo,
    funcionalidades: &[IdFuncionalidade],
) -> Result<bool, cba_kb::Erro> {
    let mut doc = kb.ler::<FrontmatterAnalise>(documento)?;
    if doc.frontmatter.funcionalidades == funcionalidades {
        return Ok(false);
    }
    doc.frontmatter.funcionalidades = funcionalidades.to_vec();
    if funcionalidades.is_empty() {
        doc.tirar_campo("funcionalidades");
    }
    kb.gravar(documento, &doc)?;
    Ok(true)
}

/// [`religar`] no projeto do ambiente e, se algo mudou, a sincronização do
/// índice da base (falha vira aviso).
pub fn religar_projeto(amb: &cba_core::ambiente::Ambiente) -> Result<Religacao, String> {
    let mut feita = religar(&amb.kb, &amb.analises)?;
    if feita.total() > 0 {
        let sincronizado = cba_kb::indice::Indice::abrir(amb.dados.arquivo_do_indice(), &amb.base)
            .and_then(|mut i| i.sincronizar());
        if let Err(e) = sincronizado {
            tracing::warn!("índice não sincronizado depois de religar as funcionalidades");
            feita.avisos.push(format!(
                "o índice da base não foi atualizado ({e}); ele se atualiza na próxima \
                 sincronização"
            ));
        }
    }
    tracing::info!(
        analises = feita.analises.len(),
        regras = feita.regras.len(),
        termos = feita.termos.len(),
        negociais = feita.negociais.len(),
        "documentos religados às funcionalidades"
    );
    Ok(feita)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(id: &str) -> IdFuncionalidade {
        IdFuncionalidade::analisar(id).unwrap()
    }

    fn funcionalidade(id: &str, situacao: &str, juntada_em: Option<&str>) -> serde_json::Value {
        serde_json::json!({
            "id": id, "nome": id, "area": "cobranca", "area_nome": "Cobrança",
            "descricao": "D.", "situacao": situacao, "juntada_em": juntada_em,
            "registrada_ms": 1,
            "pontos": [{
                "nome": "p",
                "evidencia": {
                    "arquivo": "a.sql", "linhas": "1-1", "trecho": "x", "hash": "h"
                }
            }],
        })
    }

    #[test]
    fn juntada_vale_pela_de_destino_e_descartada_fica_de_fora() {
        let mapa: MapaFuncionalidades = serde_json::from_value(serde_json::json!({
            "formato": 1,
            "funcionalidades": [
                funcionalidade("F-COB-001", "aceita", None),
                funcionalidade("F-COB-002", "juntada", Some("F-COB-001")),
                funcionalidade("F-COB-003", "descartada", None),
            ],
        }))
        .unwrap();
        let (validos, fora) = normalizar(
            &mapa,
            &[
                f("F-COB-002"),
                f("F-COB-001"),
                f("F-COB-003"),
                f("F-XYZ-001"),
            ],
        );
        assert_eq!(validos, [f("F-COB-001")]);
        assert_eq!(fora, [f("F-COB-003"), f("F-XYZ-001")]);
        assert_eq!(
            area_das(&mapa, &[f("F-COB-002")]).as_deref(),
            Some("cobranca")
        );
        assert_eq!(
            somar(&[f("F-COB-002")], &[f("F-COB-001"), f("F-COB-002")]),
            [f("F-COB-002"), f("F-COB-001")]
        );
    }
}
