//! Ata de reunião (épico #93): o estado registrado durante a análise da reunião
//! e a montagem da ata pelo template `ata`, como as análises.
//!
//! - O estado ([`EstadoAta`]) fica em
//!   `demandas/D-NNNN-<slug>/reunioes/R-NNNN.estado.json`. Ele é escrito pela
//!   ferramenta `registrar_item_ata` do MCP e lido pela UI (ata ao vivo, painel
//!   da demanda, aprovação).
//! - A cada mudança, [`Templates::gravar_ata`] grava o estado e monta de novo a
//!   ata (`R-NNNN.md`, ao lado), com o template de `<base>/_templates/ata.md`
//!   quando existir.
//! - A exportação para Markdown e Word é a mesma das análises
//!   ([`crate::exportar`]), a partir do texto da ata.
//!
//! Cada item (decisão, necessidade, regra citada, pendência, dúvida ou próximo
//! passo) tem um identificador estável dentro da ata ([`IdItemAta`]: `D-1`,
//! `N-3`, `P-2`), que os próximos passos e a UI usam para apontar para ele, e as
//! falas de origem ([`CitacaoFala`]): falante, horário e trecho curto. Enquanto o
//! transcript existir, a citação é conferida contra a fala; depois do descarte,
//! fica como "conferida em <data>" ([`EstadoAta::marcar_conferidas_em`]).

use std::fmt;

use cba_kb::frontmatter::{Data, FrontmatterAta, Horario, StatusAta};
use cba_kb::{
    ArquivosDaReuniao, CaminhoRelativo, IdDemanda, IdReuniao, Projeto, TipoDocumento, e_slug,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::conteudo::{Citacao, Conteudo, Texto};
use crate::link;
use crate::modelo::{PARTICIPANTES, REFERENCIAS_FALAS};
use crate::montagem::{ErroMontagem, Montado};
use crate::template::{Aviso, Templates};

/// Pasta do catálogo de pessoas, na raiz do projeto (`pessoas/<slug>.md`).
const PESSOAS: &str = "pessoas";

/// Tipo de um item da ata. Cada tipo vai para uma seção do template.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TipoItem {
    Decisao,
    Necessidade,
    /// Regra de negócio citada na reunião.
    Regra,
    /// Pendência: o texto é a ação, com responsável e prazo.
    Pendencia,
    /// Dúvida em aberto.
    Duvida,
    ProximoPasso,
}

impl TipoItem {
    /// Na ordem das seções do template padrão.
    pub const TODOS: [TipoItem; 6] = [
        Self::Decisao,
        Self::Necessidade,
        Self::Regra,
        Self::Pendencia,
        Self::Duvida,
        Self::ProximoPasso,
    ];

    /// Prefixo do identificador do item (`D` em `D-1`).
    pub fn prefixo(self) -> &'static str {
        match self {
            Self::Decisao => "D",
            Self::Necessidade => "N",
            Self::Regra => "R",
            Self::Pendencia => "P",
            Self::Duvida => "DV",
            Self::ProximoPasso => "PP",
        }
    }

    /// Marcador da seção do template.
    pub fn secao(self) -> &'static str {
        match self {
            Self::Decisao => "decisoes",
            Self::Necessidade => "necessidades",
            Self::Regra => "regras",
            Self::Pendencia => "pendencias",
            Self::Duvida => "duvidas",
            Self::ProximoPasso => "proximos_passos",
        }
    }

    /// Valor no estado (`proximo_passo`).
    pub fn como_str(self) -> &'static str {
        match self {
            Self::Decisao => "decisao",
            Self::Necessidade => "necessidade",
            Self::Regra => "regra",
            Self::Pendencia => "pendencia",
            Self::Duvida => "duvida",
            Self::ProximoPasso => "proximo_passo",
        }
    }
}

impl fmt::Display for TipoItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.como_str())
    }
}

/// Identificador de um item dentro da ata: prefixo do tipo e número sequencial
/// por tipo, a partir de 1 (`D-1`, `N-3`, `DV-2`, `PP-1`). Nunca é reaproveitado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IdItemAta {
    pub tipo: TipoItem,
    pub numero: u32,
}

impl IdItemAta {
    /// Lê `D-1`, `PP-12`. Sem zeros à esquerda.
    pub fn analisar(texto: &str) -> Option<Self> {
        let (prefixo, numero) = texto.split_once('-')?;
        let tipo = TipoItem::TODOS
            .into_iter()
            .find(|t| t.prefixo() == prefixo)?;
        if numero.is_empty()
            || numero.starts_with('0')
            || !numero.bytes().all(|b| b.is_ascii_digit())
        {
            return None;
        }
        Some(Self {
            tipo,
            numero: numero.parse().ok()?,
        })
    }
}

impl fmt::Display for IdItemAta {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}", self.tipo.prefixo(), self.numero)
    }
}

impl std::str::FromStr for IdItemAta {
    type Err = ErroAta;

    fn from_str(texto: &str) -> Result<Self, Self::Err> {
        Self::analisar(texto).ok_or_else(|| ErroAta::IdDeItem(texto.to_owned()))
    }
}

impl Serialize for IdItemAta {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for IdItemAta {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let texto = String::deserialize(d)?;
        Self::analisar(&texto).ok_or_else(|| {
            serde::de::Error::custom(format!(
                "identificador de item inválido `{texto}`; use D-1, N-1, R-1, P-1, DV-1 ou PP-1"
            ))
        })
    }
}

/// Conferência de uma citação contra o transcript.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StatusCitacao {
    /// O trecho aparece na fala citada (o transcript ainda existe).
    Conferida,
    /// Conferida enquanto o transcript existia, que foi descartado nesta data.
    ConferidaEm(Data),
    /// O trecho não aparece na fala citada.
    NaoConfere,
}

impl StatusCitacao {
    /// Texto mostrado na ata.
    pub fn rotulo(self) -> String {
        match self {
            Self::Conferida => "conferida".into(),
            Self::ConferidaEm(d) => format!("conferida em {d}"),
            Self::NaoConfere => "não confere".into(),
        }
    }
}

/// Fala de origem de um item: falante, horário de início e trecho curto.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CitacaoFala {
    pub falante: String,
    /// Início da fala, desde o começo da reunião.
    pub inicio: Horario,
    pub trecho: String,
    pub status: StatusCitacao,
}

impl CitacaoFala {
    pub fn nova(
        falante: impl Into<String>,
        inicio: Horario,
        trecho: impl Into<String>,
        status: StatusCitacao,
    ) -> Self {
        Self {
            falante: falante.into(),
            inicio,
            trecho: trecho.into(),
            status,
        }
    }
}

/// Participante da reunião e o papel dele nela.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Participante {
    pub nome: String,
    /// Papel nesta reunião (PO, gerente, usuário-chave ou texto livre).
    #[serde(default)]
    pub papel: Option<String>,
    /// Slug da pessoa no catálogo do projeto (`pessoas/<slug>.md`).
    #[serde(default)]
    pub pessoa: Option<String>,
}

/// Item registrado na ata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemAta {
    pub id: IdItemAta,
    pub tipo: TipoItem,
    /// O que foi decidido, levantado ou perguntado. Na pendência, a ação.
    pub texto: String,
    /// Responsável pela pendência.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub responsavel: Option<String>,
    /// Prazo da pendência, como foi dito (`2026-11-20`, `até sexta`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prazo: Option<String>,
    /// Itens a que este se refere (o próximo passo "Analisar o impacto de N-3").
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub referencias: Vec<IdItemAta>,
    /// Falas de origem.
    #[serde(default)]
    pub citacoes: Vec<CitacaoFala>,
    /// Trechos de código que sustentam o item ("o sistema já faz isso"), como
    /// notas de rodapé.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidencias: Vec<Citacao>,
}

/// Item a registrar; o identificador é dado por [`EstadoAta::adicionar_item`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NovoItem {
    pub tipo: TipoItem,
    pub texto: String,
    pub responsavel: Option<String>,
    pub prazo: Option<String>,
    pub referencias: Vec<IdItemAta>,
    pub citacoes: Vec<CitacaoFala>,
    pub evidencias: Vec<Citacao>,
}

impl NovoItem {
    pub fn novo(tipo: TipoItem, texto: impl Into<String>) -> Self {
        Self {
            tipo,
            texto: texto.into(),
            responsavel: None,
            prazo: None,
            referencias: Vec::new(),
            citacoes: Vec::new(),
            evidencias: Vec::new(),
        }
    }

    pub fn com_citacao(mut self, citacao: CitacaoFala) -> Self {
        self.citacoes.push(citacao);
        self
    }
}

/// Erro ao registrar, ler ou gravar uma ata.
#[derive(Debug, thiserror::Error)]
pub enum ErroAta {
    #[error("o item precisa de um texto")]
    TextoVazio,
    #[error("identificador de item inválido `{0}`; use D-1, N-1, R-1, P-1, DV-1 ou PP-1")]
    IdDeItem(String),
    #[error("o item {0} não existe nesta ata")]
    ItemInexistente(IdItemAta),
    #[error("o item {id} está registrado como {tipo}, mas o identificador é de outro tipo")]
    TipoDoItem { id: IdItemAta, tipo: TipoItem },
    #[error("o item {0} aparece mais de uma vez")]
    ItemRepetido(IdItemAta),
    #[error("{arquivo}: o estado é da reunião {estado}, não de {esperada}")]
    ReuniaoDiferente {
        arquivo: CaminhoRelativo,
        estado: IdReuniao,
        esperada: IdReuniao,
    },
    #[error("a reunião {id} é da demanda {arquivos}, mas o estado diz {estado}")]
    DemandaDiferente {
        id: IdReuniao,
        arquivos: IdDemanda,
        estado: IdDemanda,
    },
    #[error("{arquivo}: estado da ata inválido: {erro}")]
    Json {
        arquivo: CaminhoRelativo,
        #[source]
        erro: serde_json::Error,
    },
    #[error(transparent)]
    Montagem(#[from] ErroMontagem),
    #[error(transparent)]
    Base(#[from] cba_kb::Erro),
}

/// Estado de uma ata: tudo o que a montagem precisa. Gravado em
/// `R-NNNN.estado.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EstadoAta {
    pub id: IdReuniao,
    /// Slug do projeto.
    pub projeto: String,
    pub demanda: IdDemanda,
    /// Data da reunião.
    pub data: Data,
    #[serde(default)]
    pub duracao: Option<Horario>,
    pub status: StatusAta,
    #[serde(default)]
    pub participantes: Vec<Participante>,
    #[serde(default)]
    pub objetivo: Option<String>,
    #[serde(default)]
    pub resumo: Option<String>,
    /// Itens na ordem de registro.
    #[serde(default)]
    pub itens: Vec<ItemAta>,
}

impl EstadoAta {
    /// Ata em rascunho, sem itens.
    pub fn nova(id: IdReuniao, projeto: impl Into<String>, demanda: IdDemanda, data: Data) -> Self {
        Self {
            id,
            projeto: projeto.into(),
            demanda,
            data,
            duracao: None,
            status: StatusAta::Rascunho,
            participantes: Vec::new(),
            objetivo: None,
            resumo: None,
            itens: Vec::new(),
        }
    }

    pub fn definir_participantes(&mut self, participantes: Vec<Participante>) {
        self.participantes = participantes;
    }

    pub fn definir_objetivo(&mut self, objetivo: impl Into<String>) {
        self.objetivo = Some(objetivo.into());
    }

    pub fn definir_resumo(&mut self, resumo: impl Into<String>) {
        self.resumo = Some(resumo.into());
    }

    pub fn definir_duracao(&mut self, duracao: Option<Horario>) {
        self.duracao = duracao;
    }

    /// Próximo identificador livre do tipo (o maior número já usado mais um).
    pub fn proximo_id(&self, tipo: TipoItem) -> IdItemAta {
        let maior = self
            .itens
            .iter()
            .filter(|i| i.id.tipo == tipo)
            .map(|i| i.id.numero)
            .max()
            .unwrap_or(0);
        IdItemAta {
            tipo,
            numero: maior + 1,
        }
    }

    /// Registra um item com o próximo identificador do tipo e o devolve. As
    /// referências precisam apontar para itens já registrados.
    pub fn adicionar_item(&mut self, novo: NovoItem) -> Result<IdItemAta, ErroAta> {
        if novo.texto.trim().is_empty() {
            return Err(ErroAta::TextoVazio);
        }
        if let Some(r) = novo.referencias.iter().find(|r| self.item(**r).is_none()) {
            return Err(ErroAta::ItemInexistente(*r));
        }
        let id = self.proximo_id(novo.tipo);
        self.itens.push(ItemAta {
            id,
            tipo: novo.tipo,
            texto: novo.texto,
            responsavel: novo.responsavel,
            prazo: novo.prazo,
            referencias: novo.referencias,
            citacoes: novo.citacoes,
            evidencias: novo.evidencias,
        });
        Ok(id)
    }

    pub fn item(&self, id: IdItemAta) -> Option<&ItemAta> {
        self.itens.iter().find(|i| i.id == id)
    }

    pub fn item_mut(&mut self, id: IdItemAta) -> Option<&mut ItemAta> {
        self.itens.iter_mut().find(|i| i.id == id)
    }

    /// Itens do tipo, na ordem de registro.
    pub fn itens_do_tipo(&self, tipo: TipoItem) -> impl Iterator<Item = &ItemAta> {
        self.itens.iter().filter(move |i| i.tipo == tipo)
    }

    /// Marca as citações conferidas como "conferida em `data`", ao descartar o
    /// transcript. As que não conferem continuam assim. Devolve quantas mudaram.
    pub fn marcar_conferidas_em(&mut self, data: Data) -> usize {
        let mut mudadas = 0;
        for c in self.itens.iter_mut().flat_map(|i| i.citacoes.iter_mut()) {
            if c.status == StatusCitacao::Conferida {
                c.status = StatusCitacao::ConferidaEm(data);
                mudadas += 1;
            }
        }
        mudadas
    }

    /// Confere a coerência dos itens: o identificador é do tipo do item, não se
    /// repete, e as referências apontam para itens da ata.
    pub fn validar(&self) -> Result<(), ErroAta> {
        for (i, item) in self.itens.iter().enumerate() {
            if item.id.tipo != item.tipo {
                return Err(ErroAta::TipoDoItem {
                    id: item.id,
                    tipo: item.tipo,
                });
            }
            if self.itens[..i].iter().any(|o| o.id == item.id) {
                return Err(ErroAta::ItemRepetido(item.id));
            }
            if let Some(r) = item.referencias.iter().find(|r| self.item(**r).is_none()) {
                return Err(ErroAta::ItemInexistente(*r));
            }
        }
        Ok(())
    }

    /// Estado em JSON, com recuo, como fica no arquivo.
    pub fn para_json(&self) -> String {
        let mut texto =
            serde_json::to_string_pretty(self).expect("o estado da ata sempre vira JSON");
        texto.push('\n');
        texto
    }

    /// Lê o estado de um JSON. `arquivo` só entra nas mensagens de erro.
    pub fn de_json(texto: &str, arquivo: &CaminhoRelativo) -> Result<Self, ErroAta> {
        let estado: Self = serde_json::from_str(texto).map_err(|erro| ErroAta::Json {
            arquivo: arquivo.clone(),
            erro,
        })?;
        estado.validar()?;
        Ok(estado)
    }

    /// Cria uma reunião na demanda, com o próximo `R-NNNN` livre do projeto, e
    /// grava o estado inicial (rascunho, sem itens). A ata é montada no primeiro
    /// [`Templates::gravar_ata`].
    pub fn criar(
        projeto: &Projeto,
        demanda: IdDemanda,
        data: Data,
    ) -> Result<(Self, ArquivosDaReuniao), ErroAta> {
        let arquivos = projeto.criar_reuniao(demanda, |id| {
            Self::nova(id, projeto.nome(), demanda, data).para_json()
        })?;
        let estado = Self::nova(arquivos.id, projeto.nome(), demanda, data);
        Ok((estado, arquivos))
    }

    /// Lê o estado da reunião, em qualquer demanda do projeto.
    pub fn ler(projeto: &Projeto, id: IdReuniao) -> Result<(Self, ArquivosDaReuniao), ErroAta> {
        let arquivos = projeto.reuniao(id)?;
        let texto = projeto.ler_texto(&arquivos.estado)?;
        let estado = Self::de_json(&texto, &projeto.na_base(&arquivos.estado))?;
        if estado.id != id {
            return Err(ErroAta::ReuniaoDiferente {
                arquivo: projeto.na_base(&arquivos.estado),
                estado: estado.id,
                esperada: id,
            });
        }
        Ok((estado, arquivos))
    }

    /// Grava o estado (atômico), sem montar a ata. Para gravar e montar, use
    /// [`Templates::gravar_ata`].
    pub fn gravar(&self, projeto: &Projeto) -> Result<ArquivosDaReuniao, ErroAta> {
        self.validar()?;
        let arquivos = projeto.reuniao(self.id)?;
        if arquivos.demanda != self.demanda {
            return Err(ErroAta::DemandaDiferente {
                id: self.id,
                arquivos: arquivos.demanda,
                estado: self.demanda,
            });
        }
        projeto.gravar_texto(&arquivos.estado, &self.para_json())?;
        Ok(arquivos)
    }

    /// Frontmatter da ata.
    pub fn frontmatter(&self) -> FrontmatterAta {
        FrontmatterAta {
            tipo: TipoDocumento::Ata,
            id: self.id,
            projeto: self.projeto.clone(),
            demanda: self.demanda,
            data: self.data,
            duracao: self.duracao,
            status: self.status,
        }
    }

    /// Conteúdo da ata para a montagem. `caminho` é onde a ata fica, relativo à
    /// raiz do projeto (`demandas/D-0001-x/reunioes/R-0001.md`), para os links.
    pub fn conteudo(&self, caminho: CaminhoRelativo) -> Conteudo<FrontmatterAta> {
        let mut conteudo = Conteudo::novo(self.frontmatter(), caminho);
        if !self.participantes.is_empty() {
            let linhas: Vec<String> = self
                .participantes
                .iter()
                .map(|p| participante(p, &conteudo.caminho))
                .collect();
            conteudo
                .textos
                .push(Texto::novo(PARTICIPANTES, linhas.join("\n")));
        }
        for (secao, texto) in [("objetivo", &self.objetivo), ("resumo", &self.resumo)] {
            if let Some(t) = texto.as_deref().filter(|t| !t.trim().is_empty()) {
                conteudo.textos.push(Texto::novo(secao, t));
            }
        }
        for tipo in TipoItem::TODOS {
            for item in self.itens_do_tipo(tipo) {
                conteudo.textos.push(Texto {
                    secao: tipo.secao().into(),
                    texto: linha_do_item(item),
                    citacoes: item.evidencias.clone(),
                });
            }
        }
        let referencias: Vec<String> = TipoItem::TODOS
            .into_iter()
            .flat_map(|t| self.itens_do_tipo(t))
            .flat_map(|item| item.citacoes.iter().map(move |c| referencia(item.id, c)))
            .collect();
        if !referencias.is_empty() {
            conteudo
                .textos
                .push(Texto::novo(REFERENCIAS_FALAS, referencias.join("\n")));
        }
        conteudo
    }
}

impl Templates {
    /// Monta a ata do estado com o template `ata`. `caminho` é onde a ata fica,
    /// relativo à raiz do projeto.
    pub fn montar_ata(
        &self,
        estado: &EstadoAta,
        caminho: CaminhoRelativo,
    ) -> Result<Montado<FrontmatterAta>, ErroMontagem> {
        self.montar(&estado.conteudo(caminho))
    }

    /// Grava o estado e monta de novo a ata, ao lado dele (`R-NNNN.md`).
    /// Chamado a cada mudança no estado. Devolve os avisos da montagem.
    pub fn gravar_ata(&self, projeto: &Projeto, estado: &EstadoAta) -> Result<Vec<Aviso>, ErroAta> {
        let arquivos = estado.gravar(projeto)?;
        Ok(self.gravar(projeto, &estado.conteudo(arquivos.ata))?)
    }
}

/// Texto numa linha só, para caber num item de lista.
fn em_uma_linha(texto: &str) -> String {
    texto.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn participante(p: &Participante, ata: &CaminhoRelativo) -> String {
    let nome = em_uma_linha(&p.nome);
    let nome = match p.pessoa.as_deref().filter(|s| e_slug(s)) {
        Some(slug) => CaminhoRelativo::novo(&format!("{PESSOAS}/{slug}.md"))
            .map_or_else(|_| nome.clone(), |destino| link(&nome, ata, &destino)),
        None => nome,
    };
    let papel = p
        .papel
        .as_deref()
        .map(em_uma_linha)
        .filter(|p| !p.is_empty())
        .unwrap_or_else(|| "papel a definir".into());
    format!("- {nome} · {papel}")
}

/// `- **D-1** Texto — _Maria Silva, 00:12:34_`, com os dados da pendência e as
/// referências do próximo passo. As linhas seguintes do texto ficam recuadas,
/// no mesmo item.
fn linha_do_item(item: &ItemAta) -> String {
    let mut linha = format!("- **{}** {}", item.id, item.texto.trim());
    if item.tipo == TipoItem::Pendencia {
        let ou_a_definir = |v: &Option<String>| {
            v.as_deref()
                .map(em_uma_linha)
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| "a definir".into())
        };
        linha.push_str(&format!(
            " · responsável: {} · prazo: {}",
            ou_a_definir(&item.responsavel),
            ou_a_definir(&item.prazo)
        ));
    }
    if !item.referencias.is_empty() {
        let ids: Vec<String> = item.referencias.iter().map(ToString::to_string).collect();
        linha.push_str(&format!(" (sobre {})", ids.join(", ")));
    }
    let falas: Vec<String> = item
        .citacoes
        .iter()
        .map(|c| format!("{}, {}", em_uma_linha(&c.falante), c.inicio))
        .collect();
    if falas.is_empty() {
        linha.push_str(" — _sem fala citada_");
    } else {
        linha.push_str(&format!(" — _{}_", falas.join("; ")));
    }
    let mut saida = String::new();
    for (i, l) in linha.lines().enumerate() {
        if i > 0 {
            saida.push('\n');
            if !l.trim().is_empty() {
                saida.push_str("  ");
            }
        }
        saida.push_str(l);
    }
    saida
}

/// `- **D-1** · Maria Silva, 00:12:34: “trecho” _(conferida)_`.
fn referencia(item: IdItemAta, c: &CitacaoFala) -> String {
    format!(
        "- **{item}** · {}, {}: “{}” _({})_",
        em_uma_linha(&c.falante),
        c.inicio,
        em_uma_linha(&c.trecho),
        c.status.rotulo()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(dia: u8) -> Data {
        Data::nova(2026, 11, dia).unwrap()
    }

    #[test]
    fn id_de_item_ida_e_volta() {
        for texto in ["D-1", "N-3", "R-2", "P-10", "DV-1", "PP-4"] {
            assert_eq!(IdItemAta::analisar(texto).unwrap().to_string(), texto);
        }
        for ruim in ["D-0", "D-01", "X-1", "D1", "D-", "PP-1a", "R-0001"] {
            assert!(IdItemAta::analisar(ruim).is_none(), "{ruim}");
        }
    }

    #[test]
    fn ids_sequenciais_por_tipo_e_referencias_conferidas() {
        let mut e = EstadoAta::nova(
            IdReuniao::novo(1),
            "faturamento-exemplo",
            IdDemanda::novo(1),
            d(12),
        );
        let n1 = e
            .adicionar_item(NovoItem::novo(TipoItem::Necessidade, "a"))
            .unwrap();
        let d1 = e
            .adicionar_item(NovoItem::novo(TipoItem::Decisao, "b"))
            .unwrap();
        let n2 = e
            .adicionar_item(NovoItem::novo(TipoItem::Necessidade, "c"))
            .unwrap();
        assert_eq!([n1, d1, n2].map(|i| i.to_string()), ["N-1", "D-1", "N-2"]);
        let mut passo = NovoItem::novo(TipoItem::ProximoPasso, "Analisar impacto");
        passo.referencias = vec![IdItemAta::analisar("N-9").unwrap()];
        assert!(matches!(
            e.adicionar_item(passo.clone()),
            Err(ErroAta::ItemInexistente(_))
        ));
        passo.referencias = vec![n2];
        assert_eq!(e.adicionar_item(passo).unwrap().to_string(), "PP-1");
        assert!(matches!(
            e.adicionar_item(NovoItem::novo(TipoItem::Duvida, "  ")),
            Err(ErroAta::TextoVazio)
        ));
    }

    #[test]
    fn status_da_citacao_em_json() {
        let c = CitacaoFala::nova(
            "Maria",
            Horario::de_segundos(754),
            "x",
            StatusCitacao::ConferidaEm(d(20)),
        );
        let json = serde_json::to_string(&c).unwrap();
        assert_eq!(
            json,
            r#"{"falante":"Maria","inicio":"00:12:34","trecho":"x","status":{"conferida_em":"2026-11-20"}}"#
        );
        assert_eq!(serde_json::from_str::<CitacaoFala>(&json).unwrap(), c);
        let s: StatusCitacao = serde_json::from_str(r#""nao_confere""#).unwrap();
        assert_eq!(s.rotulo(), "não confere");
    }
}
