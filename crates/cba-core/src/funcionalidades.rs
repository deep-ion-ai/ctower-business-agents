//! Funcionalidades mapeadas numa análise do fluxo `mapear` (#208).
//!
//! O agente parte dos pontos de entrada do mapa estrutural
//! ([`cba_index::Mapa::pontos_de_entrada`]) e os agrupa em funcionalidades com
//! nome de negócio. Cada funcionalidade tem identificador estável
//! ([`IdFuncionalidade`], `F-<SIGLA>-NNN`), reservado por quem registra: o mesmo
//! do catálogo quando a funcionalidade já existe nele ([`crate::catalogo`]).
//!
//! - [`Funcionalidade`]: nome, área, descrição, atores, pontos de entrada (cada um
//!   com evidência), dados lidos e gravados, relacionadas e a decisão do analista.
//!   Funcionalidade sem ponto de entrada é recusada.
//! - [`PontoJustificado`]: ponto de entrada do mapa que fica sem funcionalidade,
//!   com o motivo (código morto, utilitário ou outro).
//! - Decisões do analista: aceitar, descartar, editar, juntar duas e dividir uma.
//!
//! A completude (pontos do mapa sem funcionalidade nem justificativa) depende do
//! mapa e é calculada pelas ferramentas (`cba_tools::mapear`). Este módulo é só
//! domínio, sem disco.

use std::collections::BTreeSet;

use cba_index::TipoEntrada;
use cba_kb::IdFuncionalidade;
use serde::{Deserialize, Serialize};

use crate::analise::{Analise, ErroModelo, EstadoAnalise, Evidencia, Fluxo};

/// Ponto de entrada atribuído a uma funcionalidade.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PontoAtribuido {
    /// Chave do ponto no mapa (`<arquivo>#<nome qualificado>`), quando é um dos
    /// pontos que o app acha. Sem ela, o ponto está fora da lista (menu montado
    /// por tabela, job agendado fora do repositório) e só a evidência o sustenta.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chave: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tipo: Option<TipoEntrada>,
    /// Nome do ponto (`PKG_COBRANCA.PRC_CANCELAR_FATURA`, `FrmCancelamento`).
    pub nome: String,
    pub evidencia: Evidencia,
}

impl PontoAtribuido {
    /// O mesmo ponto: a mesma chave ou, sem chave, a mesma evidência.
    pub fn mesmo(&self, outro: &Self) -> bool {
        match (&self.chave, &outro.chave) {
            (Some(a), Some(b)) => a == b,
            _ => {
                self.evidencia.arquivo == outro.evidencia.arquivo
                    && self.evidencia.linhas == outro.evidencia.linhas
            }
        }
    }
}

/// Decisão do analista sobre uma funcionalidade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SituacaoFuncionalidade {
    /// Registrada pelo agente, sem decisão.
    #[default]
    Pendente,
    Aceita,
    Descartada,
    /// Juntada a outra ([`Funcionalidade::juntada_em`]).
    Juntada,
}

/// Funcionalidade de negócio registrada na análise.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Funcionalidade {
    pub id: IdFuncionalidade,
    /// Nome de negócio ("Parcelar fatura").
    pub nome: String,
    /// Slug da área (`cobranca`).
    pub area: String,
    /// Nome da área para quem lê ("Cobrança").
    pub area_nome: String,
    /// Uma ou duas frases.
    pub descricao: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub atores: Vec<String>,
    pub pontos: Vec<PontoAtribuido>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dados_lidos: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dados_gravados: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relacionadas: Vec<IdFuncionalidade>,
    #[serde(default)]
    pub situacao: SituacaoFuncionalidade,
    /// Funcionalidade em que esta foi juntada.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub juntada_em: Option<IdFuncionalidade>,
    /// O identificador veio do catálogo: a funcionalidade já existia.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub do_catalogo: bool,
    /// Nome, área ou descrição editados pelo analista.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub editada: bool,
    #[serde(default)]
    pub sessao: Option<String>,
    pub registrada_ms: u64,
}

impl Funcionalidade {
    /// Entra no documento e no catálogo (não foi descartada nem juntada).
    pub fn aberta(&self) -> bool {
        matches!(
            self.situacao,
            SituacaoFuncionalidade::Pendente | SituacaoFuncionalidade::Aceita
        )
    }

    /// Chaves dos pontos do mapa atribuídos.
    pub fn chaves(&self) -> impl Iterator<Item = &str> {
        self.pontos.iter().filter_map(|p| p.chave.as_deref())
    }
}

/// Dados de uma funcionalidade nova (ou da nova versão de uma já registrada).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NovaFuncionalidade {
    pub nome: String,
    pub area: String,
    pub area_nome: String,
    pub descricao: String,
    pub atores: Vec<String>,
    pub pontos: Vec<PontoAtribuido>,
    pub dados_lidos: Vec<String>,
    pub dados_gravados: Vec<String>,
    pub relacionadas: Vec<IdFuncionalidade>,
    pub do_catalogo: bool,
    pub sessao: Option<String>,
}

/// Por que um ponto de entrada fica sem funcionalidade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MotivoSemFuncionalidade {
    /// Ninguém chama e não é chamado de fora.
    CodigoMorto,
    /// Ajudante usado por várias funcionalidades (leitura de parâmetro, log).
    Utilitario,
    Outro,
}

impl MotivoSemFuncionalidade {
    pub fn nome(self) -> &'static str {
        match self {
            Self::CodigoMorto => "código morto",
            Self::Utilitario => "utilitário",
            Self::Outro => "outro motivo",
        }
    }
}

/// Ponto de entrada do mapa que fica sem funcionalidade, com a justificativa.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PontoJustificado {
    pub chave: String,
    pub tipo: TipoEntrada,
    pub nome: String,
    pub arquivo: String,
    pub motivo: MotivoSemFuncionalidade,
    pub justificativa: String,
    #[serde(default)]
    pub sessao: Option<String>,
    pub registrado_ms: u64,
}

/// Escopo de uma análise de mapeamento: o projeto todo, uma pasta ou uma área.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EscopoMapa {
    /// Pasta relativa à raiz do projeto (`db`, `backend/src`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pasta: Option<String>,
    /// Slug da área.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub area: Option<String>,
}

impl EscopoMapa {
    /// Escopo do projeto inteiro.
    pub fn e_projeto(&self) -> bool {
        self.pasta.is_none() && self.area.is_none()
    }

    /// Descrição para quem lê.
    pub fn descricao(&self) -> String {
        match (&self.pasta, &self.area) {
            (None, None) => "o projeto inteiro".into(),
            (Some(p), None) => format!("a pasta `{p}`"),
            (None, Some(a)) => format!("a área `{a}`"),
            (Some(p), Some(a)) => format!("a pasta `{p}`, na área `{a}`"),
        }
    }

    /// O arquivo está na pasta do escopo (qualquer arquivo, sem pasta).
    pub fn contem_arquivo(&self, arquivo: &str) -> bool {
        match self.pasta.as_deref().map(|p| p.trim().trim_matches('/')) {
            None | Some("") => true,
            Some(p) => arquivo == p || arquivo.strip_prefix(p).is_some_and(|r| r.starts_with('/')),
        }
    }
}

/// Decisão do analista sobre uma funcionalidade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DecisaoFuncionalidade {
    Aceitar,
    Descartar,
    /// Volta a pendente.
    Desfazer,
}

/// Edição do analista no nome, na área e na descrição.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EdicaoFuncionalidade {
    pub nome: String,
    pub area: String,
    pub area_nome: String,
    pub descricao: String,
}

fn exigir(campo: &'static str, valor: &str) -> Result<String, ErroModelo> {
    let v = valor.trim();
    if v.is_empty() {
        Err(ErroModelo::CampoVazio(campo))
    } else {
        Ok(v.to_owned())
    }
}

/// Textos aparados, sem vazios e sem repetidos (sem diferenciar maiúsculas).
fn lista(textos: Vec<String>) -> Vec<String> {
    let mut saida: Vec<String> = Vec::new();
    for t in textos {
        let t = t.trim();
        if !t.is_empty() && !saida.iter().any(|x| x.eq_ignore_ascii_case(t)) {
            saida.push(t.to_owned());
        }
    }
    saida
}

fn somar(destino: &mut Vec<String>, origem: &[String]) {
    let juntos = lista(destino.iter().chain(origem).cloned().collect());
    *destino = juntos;
}

fn somar_pontos(destino: &mut Vec<PontoAtribuido>, origem: Vec<PontoAtribuido>) {
    for p in origem {
        if !destino.iter().any(|x| x.mesmo(&p)) {
            destino.push(p);
        }
    }
}

impl Analise {
    fn exigir_fluxo_de_mapear(&self) -> Result<(), ErroModelo> {
        if self.fluxo == Fluxo::Mapear {
            Ok(())
        } else {
            Err(ErroModelo::ForaDoFluxoDeMapear(self.id.clone()))
        }
    }

    fn exigir_aberta_para_o_analista(&self) -> Result<(), ErroModelo> {
        self.exigir_fluxo_de_mapear()?;
        if self.estado == EstadoAnalise::Aprovada {
            Err(ErroModelo::AnaliseAprovada(self.id.clone()))
        } else {
            Ok(())
        }
    }

    pub fn funcionalidade(&self, id: &IdFuncionalidade) -> Option<&Funcionalidade> {
        self.funcionalidades.iter().find(|f| &f.id == id)
    }

    fn funcionalidade_mut(
        &mut self,
        id: &IdFuncionalidade,
    ) -> Result<&mut Funcionalidade, ErroModelo> {
        self.funcionalidades
            .iter_mut()
            .find(|f| &f.id == id)
            .ok_or_else(|| ErroModelo::FuncionalidadeDesconhecida(id.clone()))
    }

    fn funcionalidade_aberta_mut(
        &mut self,
        id: &IdFuncionalidade,
    ) -> Result<&mut Funcionalidade, ErroModelo> {
        let f = self.funcionalidade_mut(id)?;
        if !f.aberta() {
            return Err(ErroModelo::FuncionalidadeFechada(id.clone()));
        }
        Ok(f)
    }

    /// Maior número de funcionalidade desta análise com a sigla (0 sem nenhuma).
    pub fn maior_funcionalidade(&self, sigla: &str) -> u32 {
        self.funcionalidades
            .iter()
            .filter(|f| f.id.sigla() == sigla)
            .map(|f| f.id.numero())
            .max()
            .unwrap_or(0)
    }

    /// Funcionalidades que entram no documento (nem descartadas nem juntadas).
    pub fn funcionalidades_no_documento(&self) -> impl Iterator<Item = &Funcionalidade> {
        self.funcionalidades.iter().filter(|f| f.aberta())
    }

    /// Chaves dos pontos do mapa atribuídos a funcionalidades abertas.
    pub fn pontos_atribuidos(&self) -> BTreeSet<&str> {
        self.funcionalidades_no_documento()
            .flat_map(Funcionalidade::chaves)
            .collect()
    }

    /// Funcionalidade aberta que tem o ponto do mapa.
    pub fn dona_do_ponto(&self, chave: &str) -> Option<&Funcionalidade> {
        self.funcionalidades_no_documento()
            .find(|f| f.chaves().any(|c| c == chave))
    }

    /// Registra a funcionalidade com o identificador `id` (reservado por quem
    /// chama). Se o identificador já está na análise, a funcionalidade é
    /// substituída pela nova versão e volta a pendente. Os pontos atribuídos
    /// deixam de estar justificados. Devolve se substituiu.
    pub fn registrar_funcionalidade(
        &mut self,
        nova: NovaFuncionalidade,
        id: IdFuncionalidade,
        agora_ms: u64,
    ) -> Result<bool, ErroModelo> {
        self.exigir_fluxo_de_mapear()?;
        let nome = exigir("nome", &nova.nome)?;
        let area = exigir("area", &nova.area)?;
        let area_nome = exigir("area", &nova.area_nome)?;
        let descricao = exigir("descricao", &nova.descricao)?;
        if nova.pontos.is_empty() {
            return Err(ErroModelo::FuncionalidadeSemPontoDeEntrada);
        }
        let mut pontos = Vec::new();
        somar_pontos(&mut pontos, nova.pontos);
        if let Some(f) = self.funcionalidade(&id)
            && !f.aberta()
        {
            return Err(ErroModelo::FuncionalidadeFechada(id));
        }
        for p in pontos.iter().filter_map(|p| p.chave.as_deref()) {
            if let Some(dona) = self.dona_do_ponto(p)
                && dona.id != id
            {
                return Err(ErroModelo::PontoJaAtribuido {
                    chave: p.to_owned(),
                    funcionalidade: dona.id.clone(),
                });
            }
        }
        self.aceitar_registro()?;
        let mut relacionadas: Vec<IdFuncionalidade> = Vec::new();
        for r in nova.relacionadas {
            if r != id && !relacionadas.contains(&r) {
                relacionadas.push(r);
            }
        }
        let funcionalidade = Funcionalidade {
            id: id.clone(),
            nome,
            area,
            area_nome,
            descricao,
            atores: lista(nova.atores),
            pontos,
            dados_lidos: lista(nova.dados_lidos),
            dados_gravados: lista(nova.dados_gravados),
            relacionadas,
            situacao: SituacaoFuncionalidade::Pendente,
            juntada_em: None,
            do_catalogo: nova.do_catalogo,
            editada: false,
            sessao: nova.sessao,
            registrada_ms: agora_ms,
        };
        let atribuidas: BTreeSet<String> = funcionalidade.chaves().map(str::to_owned).collect();
        self.pontos_justificados
            .retain(|j| !atribuidas.contains(&j.chave));
        match self.funcionalidades.iter_mut().find(|f| f.id == id) {
            Some(f) => {
                let do_catalogo = f.do_catalogo || funcionalidade.do_catalogo;
                *f = funcionalidade;
                f.do_catalogo = do_catalogo;
                Ok(true)
            }
            None => {
                self.funcionalidades.push(funcionalidade);
                Ok(false)
            }
        }
    }

    /// Acrescenta um ponto de entrada a uma funcionalidade aberta (a
    /// justificativa do ponto, se houver, sai).
    pub fn atribuir_ponto(
        &mut self,
        id: &IdFuncionalidade,
        ponto: PontoAtribuido,
    ) -> Result<(), ErroModelo> {
        self.exigir_fluxo_de_mapear()?;
        if let Some(chave) = ponto.chave.as_deref()
            && let Some(dona) = self.dona_do_ponto(chave)
            && &dona.id != id
        {
            return Err(ErroModelo::PontoJaAtribuido {
                chave: chave.to_owned(),
                funcionalidade: dona.id.clone(),
            });
        }
        self.funcionalidade_aberta_mut(id)?;
        self.aceitar_registro()?;
        if let Some(chave) = ponto.chave.clone() {
            self.pontos_justificados.retain(|j| j.chave != chave);
        }
        let f = self.funcionalidade_aberta_mut(id)?;
        somar_pontos(&mut f.pontos, vec![ponto]);
        f.situacao = SituacaoFuncionalidade::Pendente;
        Ok(())
    }

    /// Justifica um ponto do mapa sem funcionalidade (substitui a justificativa
    /// anterior do mesmo ponto). Ponto já atribuído é recusado.
    pub fn justificar_ponto(&mut self, ponto: PontoJustificado) -> Result<(), ErroModelo> {
        self.exigir_fluxo_de_mapear()?;
        exigir("justificativa", &ponto.justificativa)?;
        if let Some(dona) = self.dona_do_ponto(&ponto.chave) {
            return Err(ErroModelo::PontoJaAtribuido {
                chave: ponto.chave.clone(),
                funcionalidade: dona.id.clone(),
            });
        }
        self.aceitar_registro()?;
        let ponto = PontoJustificado {
            justificativa: ponto.justificativa.trim().to_owned(),
            ..ponto
        };
        match self
            .pontos_justificados
            .iter_mut()
            .find(|j| j.chave == ponto.chave)
        {
            Some(j) => *j = ponto,
            None => self.pontos_justificados.push(ponto),
        }
        Ok(())
    }

    /// Decisão do analista.
    pub fn decidir_funcionalidade(
        &mut self,
        id: &IdFuncionalidade,
        decisao: DecisaoFuncionalidade,
    ) -> Result<(), ErroModelo> {
        self.exigir_aberta_para_o_analista()?;
        let f = self.funcionalidade_mut(id)?;
        if f.situacao == SituacaoFuncionalidade::Juntada {
            return Err(ErroModelo::FuncionalidadeFechada(id.clone()));
        }
        f.situacao = match decisao {
            DecisaoFuncionalidade::Aceitar => SituacaoFuncionalidade::Aceita,
            DecisaoFuncionalidade::Descartar => SituacaoFuncionalidade::Descartada,
            DecisaoFuncionalidade::Desfazer => SituacaoFuncionalidade::Pendente,
        };
        Ok(())
    }

    /// Edição do analista no nome, na área e na descrição. O identificador fica.
    pub fn editar_funcionalidade(
        &mut self,
        id: &IdFuncionalidade,
        edicao: EdicaoFuncionalidade,
    ) -> Result<(), ErroModelo> {
        self.exigir_aberta_para_o_analista()?;
        let nome = exigir("nome", &edicao.nome)?;
        let area = exigir("area", &edicao.area)?;
        let area_nome = exigir("area", &edicao.area_nome)?;
        let descricao = exigir("descricao", &edicao.descricao)?;
        let f = self.funcionalidade_aberta_mut(id)?;
        if f.nome == nome && f.area == area && f.area_nome == area_nome && f.descricao == descricao
        {
            return Ok(());
        }
        f.nome = nome;
        f.area = area;
        f.area_nome = area_nome;
        f.descricao = descricao;
        f.editada = true;
        Ok(())
    }

    /// Junta `origem` em `destino`: o destino recebe os pontos, os atores, os
    /// dados e as relacionadas da origem, que fica juntada (fora do documento e
    /// do catálogo).
    pub fn juntar_funcionalidades(
        &mut self,
        origem: &IdFuncionalidade,
        destino: &IdFuncionalidade,
    ) -> Result<(), ErroModelo> {
        self.exigir_aberta_para_o_analista()?;
        if origem == destino {
            return Err(ErroModelo::JuncaoInvalida(
                "escolha duas funcionalidades diferentes".into(),
            ));
        }
        self.funcionalidade_aberta_mut(destino)?;
        let o = self.funcionalidade_aberta_mut(origem)?.clone();
        let d = self.funcionalidade_aberta_mut(destino)?;
        somar_pontos(&mut d.pontos, o.pontos);
        somar(&mut d.atores, &o.atores);
        somar(&mut d.dados_lidos, &o.dados_lidos);
        somar(&mut d.dados_gravados, &o.dados_gravados);
        for r in o.relacionadas {
            if &r != destino && !d.relacionadas.contains(&r) {
                d.relacionadas.push(r);
            }
        }
        d.relacionadas.retain(|r| r != origem);
        d.situacao = SituacaoFuncionalidade::Pendente;
        let o = self.funcionalidade_mut(origem)?;
        o.situacao = SituacaoFuncionalidade::Juntada;
        o.juntada_em = Some(destino.clone());
        // Quem citava a origem passa a citar o destino.
        for f in &mut self.funcionalidades {
            if f.relacionadas.contains(origem) {
                f.relacionadas.retain(|r| r != origem);
                if &f.id != destino && !f.relacionadas.contains(destino) {
                    f.relacionadas.push(destino.clone());
                }
            }
        }
        Ok(())
    }

    /// Divide uma funcionalidade: os pontos de índice `pontos` (a partir de 0)
    /// passam para uma funcionalidade nova, `novo_id`, com o nome e a descrição
    /// dados. As duas ficam pendentes e citam uma à outra como relacionadas.
    pub fn dividir_funcionalidade(
        &mut self,
        id: &IdFuncionalidade,
        pontos: &[usize],
        nome: &str,
        descricao: &str,
        novo_id: IdFuncionalidade,
        agora_ms: u64,
    ) -> Result<IdFuncionalidade, ErroModelo> {
        self.exigir_aberta_para_o_analista()?;
        let nome = exigir("nome", nome)?;
        let descricao = exigir("descricao", descricao)?;
        if self.funcionalidade(&novo_id).is_some() {
            return Err(ErroModelo::DivisaoInvalida(format!(
                "o identificador {novo_id} já existe nesta análise"
            )));
        }
        let original = self.funcionalidade_aberta_mut(id)?;
        let escolhidos: BTreeSet<usize> = pontos.iter().copied().collect();
        if escolhidos.is_empty() || escolhidos.iter().any(|i| *i >= original.pontos.len()) {
            return Err(ErroModelo::DivisaoInvalida(
                "escolha ao menos um ponto de entrada da funcionalidade para levar".into(),
            ));
        }
        if escolhidos.len() == original.pontos.len() {
            return Err(ErroModelo::DivisaoInvalida(
                "deixe ao menos um ponto de entrada na funcionalidade original".into(),
            ));
        }
        let mut ficam = Vec::new();
        let mut vao = Vec::new();
        for (i, p) in original.pontos.drain(..).enumerate() {
            if escolhidos.contains(&i) {
                vao.push(p);
            } else {
                ficam.push(p);
            }
        }
        original.pontos = ficam;
        original.situacao = SituacaoFuncionalidade::Pendente;
        if !original.relacionadas.contains(&novo_id) {
            original.relacionadas.push(novo_id.clone());
        }
        let nova = Funcionalidade {
            id: novo_id.clone(),
            nome,
            area: original.area.clone(),
            area_nome: original.area_nome.clone(),
            descricao,
            atores: original.atores.clone(),
            pontos: vao,
            dados_lidos: Vec::new(),
            dados_gravados: Vec::new(),
            relacionadas: vec![id.clone()],
            situacao: SituacaoFuncionalidade::Pendente,
            juntada_em: None,
            do_catalogo: false,
            editada: true,
            sessao: None,
            registrada_ms: agora_ms,
        };
        self.funcionalidades.push(nova);
        Ok(novo_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analise::EstadoEvidencia;
    use cba_kb::frontmatter::Linhas;
    use cba_kb::{CaminhoRelativo, Serie};

    fn f(n: u32) -> IdFuncionalidade {
        IdFuncionalidade::novo("FAT", n).unwrap()
    }

    fn ponto(chave: &str, linha: u32) -> PontoAtribuido {
        PontoAtribuido {
            chave: Some(chave.into()),
            tipo: Some(TipoEntrada::RotinaBanco),
            nome: chave.rsplit('#').next().unwrap().into(),
            evidencia: Evidencia {
                arquivo: CaminhoRelativo::novo("db/x.sql").unwrap(),
                linhas: Linhas::novo(linha, linha).unwrap(),
                trecho: "X".into(),
                hash: "h".into(),
                commit: None,
                estado: EstadoEvidencia::Valida,
            },
        }
    }

    fn nova(nome: &str, pontos: Vec<PontoAtribuido>) -> NovaFuncionalidade {
        NovaFuncionalidade {
            nome: nome.into(),
            area: "cobranca".into(),
            area_nome: "Cobrança".into(),
            descricao: "Faz algo.".into(),
            atores: vec!["Atendente".into(), " atendente ".into()],
            pontos,
            dados_lidos: vec!["FATURA".into()],
            dados_gravados: vec![],
            relacionadas: vec![],
            do_catalogo: false,
            sessao: None,
        }
    }

    fn analise() -> Analise {
        Analise::nova(
            Serie::Analise.id(1),
            "faturamento-exemplo",
            Fluxo::Mapear,
            "Mapa",
            "",
            1,
        )
    }

    #[test]
    fn registra_substitui_e_recusa_sem_ponto() {
        let mut a = analise();
        assert_eq!(
            a.registrar_funcionalidade(nova("Cancelar", vec![]), f(1), 2),
            Err(ErroModelo::FuncionalidadeSemPontoDeEntrada)
        );
        assert!(
            !a.registrar_funcionalidade(nova("Cancelar", vec![ponto("db/x.sql#A", 1)]), f(1), 2)
                .unwrap()
        );
        assert_eq!(a.funcionalidades[0].atores, vec!["Atendente"]);
        assert_eq!(a.estado, EstadoAnalise::EmAndamento);
        a.decidir_funcionalidade(&f(1), DecisaoFuncionalidade::Aceitar)
            .unwrap();
        // De novo, com o mesmo id: substitui e volta a pendente.
        assert!(
            a.registrar_funcionalidade(
                nova(
                    "Cancelar fatura",
                    vec![ponto("db/x.sql#A", 1), ponto("db/x.sql#B", 2)]
                ),
                f(1),
                3
            )
            .unwrap()
        );
        assert_eq!(a.funcionalidades.len(), 1);
        assert_eq!(
            a.funcionalidades[0].situacao,
            SituacaoFuncionalidade::Pendente
        );
        assert_eq!(a.pontos_atribuidos().len(), 2);
        // O ponto de outra funcionalidade é recusado.
        assert!(matches!(
            a.registrar_funcionalidade(nova("Outra", vec![ponto("db/x.sql#B", 2)]), f(2), 4),
            Err(ErroModelo::PontoJaAtribuido { .. })
        ));
        // Fora do fluxo.
        let mut b = analise();
        b.fluxo = Fluxo::Explicar;
        assert!(matches!(
            b.registrar_funcionalidade(nova("X", vec![ponto("db/x.sql#A", 1)]), f(1), 2),
            Err(ErroModelo::ForaDoFluxoDeMapear(_))
        ));
    }

    #[test]
    fn justificativa_sai_quando_o_ponto_e_atribuido() {
        let mut a = analise();
        let just = |chave: &str| PontoJustificado {
            chave: chave.into(),
            tipo: TipoEntrada::RotinaBanco,
            nome: "FN_PARAMETRO".into(),
            arquivo: "db/x.sql".into(),
            motivo: MotivoSemFuncionalidade::Utilitario,
            justificativa: "lê parâmetro".into(),
            sessao: None,
            registrado_ms: 1,
        };
        a.justificar_ponto(just("db/x.sql#P")).unwrap();
        a.justificar_ponto(just("db/x.sql#P")).unwrap();
        assert_eq!(a.pontos_justificados.len(), 1);
        a.registrar_funcionalidade(nova("Cobrar", vec![ponto("db/x.sql#A", 1)]), f(1), 2)
            .unwrap();
        a.atribuir_ponto(&f(1), ponto("db/x.sql#P", 5)).unwrap();
        assert!(a.pontos_justificados.is_empty());
        assert!(matches!(
            a.justificar_ponto(just("db/x.sql#P")),
            Err(ErroModelo::PontoJaAtribuido { .. })
        ));
    }

    #[test]
    fn juntar_e_dividir() {
        let mut a = analise();
        a.registrar_funcionalidade(nova("A", vec![ponto("db/x.sql#A", 1)]), f(1), 2)
            .unwrap();
        a.registrar_funcionalidade(
            nova("B", vec![ponto("db/x.sql#B", 2), ponto("db/x.sql#C", 3)]),
            f(2),
            2,
        )
        .unwrap();
        let mut c = nova("C", vec![ponto("db/x.sql#D", 4)]);
        c.relacionadas = vec![f(2)];
        a.registrar_funcionalidade(c, f(3), 2).unwrap();

        a.juntar_funcionalidades(&f(2), &f(1)).unwrap();
        let um = a.funcionalidade(&f(1)).unwrap();
        assert_eq!(um.pontos.len(), 3);
        assert_eq!(a.funcionalidade(&f(2)).unwrap().juntada_em, Some(f(1)));
        assert_eq!(a.funcionalidade(&f(3)).unwrap().relacionadas, vec![f(1)]);
        assert_eq!(a.funcionalidades_no_documento().count(), 2);
        assert!(matches!(
            a.juntar_funcionalidades(&f(2), &f(1)),
            Err(ErroModelo::FuncionalidadeFechada(_))
        ));

        assert!(matches!(
            a.dividir_funcionalidade(&f(1), &[0, 1, 2], "N", "D", f(4), 5),
            Err(ErroModelo::DivisaoInvalida(_))
        ));
        a.dividir_funcionalidade(&f(1), &[2], "Nova", "Desc.", f(4), 5)
            .unwrap();
        assert_eq!(a.funcionalidade(&f(1)).unwrap().pontos.len(), 2);
        let nova = a.funcionalidade(&f(4)).unwrap();
        assert_eq!(nova.pontos[0].chave.as_deref(), Some("db/x.sql#C"));
        assert_eq!(nova.relacionadas, vec![f(1)]);
        assert!(
            a.funcionalidade(&f(1))
                .unwrap()
                .relacionadas
                .contains(&f(4))
        );

        a.editar_funcionalidade(
            &f(1),
            EdicaoFuncionalidade {
                nome: "Cobrar fatura".into(),
                area: "cobranca".into(),
                area_nome: "Cobrança".into(),
                descricao: "Cobra.".into(),
            },
        )
        .unwrap();
        assert!(a.funcionalidade(&f(1)).unwrap().editada);
    }

    #[test]
    fn escopo() {
        let e = EscopoMapa {
            pasta: Some("db/".into()),
            area: None,
        };
        assert!(e.contem_arquivo("db/x.sql"));
        assert!(!e.contem_arquivo("dbx/y.sql"));
        assert!(EscopoMapa::default().contem_arquivo("qualquer"));
        assert_eq!(EscopoMapa::default().descricao(), "o projeto inteiro");
    }
}
