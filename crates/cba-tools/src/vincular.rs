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
use cba_repos::listagem::{IssueListada, OrigemIssue};

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
    /// A demanda da base de cada issue confirmada, na ordem do lote.
    pub por_issue: Vec<DemandaDaIssue>,
}

/// Demanda da base que representa uma issue confirmada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DemandaDaIssue {
    pub origem: OrigemIssue,
    pub chave: String,
    pub demanda: IdDemanda,
}

fn interno(e: impl ToString) -> ErroFerramenta {
    ErroFerramenta::Interno(e.to_string())
}

/// Demanda da base com a `chave` (ou o endereço) da issue, se houver.
pub fn demanda_da_issue(
    existentes: &[cba_kb::DemandaLida],
    issue: &IssueListada,
) -> Option<IdDemanda> {
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
        resultado.por_issue.push(DemandaDaIssue {
            origem: item.issue.origem,
            chave: item.issue.chave.clone(),
            demanda: id,
        });
        ligacoes.push((id, &item.funcionalidades));
    }
    let mut vinculos_novos = 0;
    let mut vinculos_existentes = 0;
    let mut alterado = alterar_no_projeto(
        &ambiente.base,
        &ambiente.kb,
        &ambiente.dados,
        |m: &mut MapaFuncionalidades, agora| {
            // Issue que ganha funcionalidade deixa de estar justificada e de ter proposta.
            for item in lote.iter().filter(|i| !i.funcionalidades.is_empty()) {
                m.desfazer_justificativa_da_issue(item.issue.origem.nome(), &item.issue.chave);
                m.limpar_proposta_da_issue(item.issue.origem.nome(), &item.issue.chave);
            }
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

/// Justifica uma issue que fica sem funcionalidade (fora do escopo,
/// infraestrutura etc.) ou, com `justificativa` vazia e `None`, tira a
/// justificativa. Uma gravação do mapa.
pub fn justificar_issue(
    ambiente: &Ambiente,
    issue: &IssueListada,
    justificativa: Option<(cba_core::funcionalidades::MotivoIssue, &str)>,
) -> Result<Alterado<()>, ErroFerramenta> {
    let origem = issue.origem.nome();
    let mut alterado = alterar_no_projeto(
        &ambiente.base,
        &ambiente.kb,
        &ambiente.dados,
        |m: &mut MapaFuncionalidades, agora| {
            match justificativa {
                Some((motivo, texto)) => {
                    m.justificar_issue(cba_core::funcionalidades::IssueJustificada {
                        origem: origem.to_owned(),
                        chave: issue.chave.clone(),
                        url: issue.url.clone(),
                        titulo: issue.titulo.clone(),
                        motivo,
                        justificativa: texto.to_owned(),
                        registrado_ms: agora,
                    })?;
                }
                None => {
                    m.desfazer_justificativa_da_issue(origem, &issue.chave);
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
    Ok(Alterado {
        mapa: std::mem::take(&mut alterado.mapa),
        resultado: (),
        avisos: alterado.avisos,
    })
}

/// Pedido que abre a sessão do Claude para propor o vínculo das issues sem
/// sugestão clara (#270). A resposta do agente são chamadas de
/// `propor_vinculo_de_issue`; quem confirma é o analista na tela. As
/// funcionalidades abertas vão no próprio pedido: as ferramentas de consulta
/// leem o mapa estrutural do código, não o mapa de funcionalidades.
pub fn pedido_de_vinculo_com_ia(mapa: &MapaFuncionalidades, issues: &[IssueListada]) -> String {
    use std::fmt::Write as _;
    let linha = |t: &str, max: usize| -> String {
        t.split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .chars()
            .take(max)
            .collect()
    };
    let mut t = String::from(
        "Proponha a funcionalidade das issues abaixo pelo ctower. Compare cada issue com as \
         funcionalidades abertas do mapa (listadas abaixo) e, quando a issue tratar do \
         comportamento, da tela, da rotina ou do dado de uma ou mais delas, chame \
         `propor_vinculo_de_issue` com `origem`, `chave`, os IDs das funcionalidades e a \
         evidência (o ponto de entrada ou o trecho da issue que sustenta o vínculo). Se \
         precisar, confirme no código com `pontos_de_entrada` e `conhecimento` do cba. \
         Relação razoável vale uma proposta: o analista confirma ou descarta na tela \
         \"Vincular demandas\", então é melhor propor com a evidência do que deixar passar. \
         Só deixe sem chamada a issue que não tem nada a ver com nenhuma funcionalidade \
         (e diga em uma frase por quê).\n\nFuncionalidades abertas:\n",
    );
    for f in mapa.ordenadas() {
        let pontos: Vec<&str> = f.pontos.iter().take(6).map(|p| p.nome.as_str()).collect();
        let _ = write!(
            t,
            "\n- {} · {} ({}): {}",
            f.id,
            f.nome.trim(),
            f.area_nome,
            linha(&f.descricao, 300)
        );
        if !f.atores.is_empty() {
            let _ = write!(t, " Atores: {}.", f.atores.join(", "));
        }
        if !pontos.is_empty() {
            let _ = write!(t, " Pontos: {}.", pontos.join(", "));
        }
    }
    t.push_str("\n\nIssues:\n");
    for i in issues {
        let _ = write!(
            t,
            "\n- origem `{}`, chave `{}`: {}\n  {}\n",
            i.origem.nome(),
            i.chave,
            i.titulo.trim(),
            linha(&i.descricao, 800)
        );
    }
    t
}
