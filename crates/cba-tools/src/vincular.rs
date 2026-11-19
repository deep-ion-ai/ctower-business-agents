//! Vínculo das issues do rastreador às funcionalidades do mapa (#270).
//!
//! A leitura é de [`cba_repos::listagem`] e a sugestão, de
//! [`cba_core::vinculos`]. Aqui fica a confirmação do analista: cada issue
//! confirmada vira (ou reaproveita, pela `chave`) uma demanda da base `D-NNNN` e
//! é ligada às funcionalidades por [`MapaFuncionalidades::ligar_demanda`], com
//! **uma única gravação do mapa por lote**. Repetir a confirmação não duplica
//! demandas nem vínculos. Nada é escrito no rastreador.

use cba_core::ambiente::Ambiente;
use cba_core::funcionalidades::MapaFuncionalidades;
use cba_kb::{IdDemanda, IdFuncionalidade, NovaDemanda};
use cba_repos::listagem::IssueListada;

use crate::ErroFerramenta;
use crate::mapear::{Alterado, alterar_no_projeto};

/// Issue confirmada pelo analista, com as funcionalidades a que pertence.
#[derive(Debug, Clone)]
pub struct IssueConfirmada {
    pub issue: IssueListada,
    pub funcionalidades: Vec<IdFuncionalidade>,
}

/// O que o lote fez.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResultadoDoLote {
    /// Demandas novas criadas na base.
    pub criadas: Vec<IdDemanda>,
    /// Demandas que já existiam (pela `chave`) e foram reaproveitadas.
    pub reaproveitadas: Vec<IdDemanda>,
    /// Vínculos novos gravados no mapa.
    pub vinculos_novos: usize,
    /// Vínculos que já existiam.
    pub vinculos_existentes: usize,
}

fn interno(e: impl ToString) -> ErroFerramenta {
    ErroFerramenta::Interno(e.to_string())
}

/// Demanda da base com a `chave` (ou o endereço) da issue, se houver.
fn demanda_da_issue(existentes: &[cba_kb::DemandaLida], issue: &IssueListada) -> Option<IdDemanda> {
    existentes
        .iter()
        .find(|d| {
            let fm = &d.documento.frontmatter;
            let chave = fm
                .chave
                .as_deref()
                .is_some_and(|c| c.trim().eq_ignore_ascii_case(&issue.chave));
            // `#12` repete entre repositórios; o endereço desempata.
            let mesmo_endereco = fm
                .origem
                .as_deref()
                .is_none_or(|o| issue.url.is_empty() || o == issue.url);
            chave && mesmo_endereco
        })
        .map(|d| d.id)
}

/// Confirma o lote: cria ou reaproveita as demandas e liga cada uma às
/// funcionalidades escolhidas. Issue sem funcionalidade fica de fora (continua
/// "sem funcionalidade", sem demanda).
pub fn confirmar_vinculos(
    ambiente: &Ambiente,
    lote: &[IssueConfirmada],
) -> Result<Alterado<ResultadoDoLote>, ErroFerramenta> {
    let mut resultado = ResultadoDoLote::default();
    let mut existentes = ambiente.kb.demandas().map_err(interno)?;
    let mut ligacoes: Vec<(IdDemanda, &[IdFuncionalidade])> = Vec::new();
    for item in lote.iter().filter(|i| !i.funcionalidades.is_empty()) {
        let id = match demanda_da_issue(&existentes, &item.issue) {
            Some(id) => {
                if !resultado.reaproveitadas.contains(&id) {
                    resultado.reaproveitadas.push(id);
                }
                id
            }
            None => {
                let nova = ambiente
                    .kb
                    .criar_demanda(
                        &NovaDemanda {
                            titulo: item.issue.titulo.clone(),
                            descricao: Some(item.issue.descricao.clone()),
                            chave: Some(item.issue.chave.clone()),
                            origem: Some(item.issue.url.clone()).filter(|u| !u.is_empty()),
                        },
                        cba_core::tempo::hoje(),
                    )
                    .map_err(interno)?;
                resultado.criadas.push(nova.id);
                existentes.push(nova.clone());
                nova.id
            }
        };
        ligacoes.push((id, &item.funcionalidades));
    }
    let mut vinculos_novos = 0;
    let mut vinculos_existentes = 0;
    let mut alterado = alterar_no_projeto(
        &ambiente.base,
        &ambiente.kb,
        &ambiente.dados,
        |m: &mut MapaFuncionalidades, agora| {
            for (id, funcionalidades) in &ligacoes {
                for f in *funcionalidades {
                    let ja = m.funcionalidade(f).is_some_and(|x| x.demandas.contains(id));
                    if ja {
                        vinculos_existentes += 1;
                        continue;
                    }
                    m.ligar_demanda(f, *id, true)?;
                    if let Some(x) = m.funcionalidades.iter_mut().find(|x| x.id == *f) {
                        x.atualizada_ms = agora;
                    }
                    vinculos_novos += 1;
                }
            }
            Ok(())
        },
    )
    .map_err(|e| match e {
        cba_core::funcionalidades::ErroAlteracaoMapa::Recusa(r) => {
            ErroFerramenta::Recusa(r.to_string())
        }
        e => interno(e),
    })?;
    resultado.vinculos_novos = vinculos_novos;
    resultado.vinculos_existentes = vinculos_existentes;
    Ok(Alterado {
        mapa: std::mem::take(&mut alterado.mapa),
        resultado,
        avisos: alterado.avisos,
    })
}
