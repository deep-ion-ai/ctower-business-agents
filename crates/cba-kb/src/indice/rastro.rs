//! Rastreabilidade (#121): a matriz de uma demanda e a consulta reversa, derivadas
//! dos identificadores já gravados na base. Nada aqui é mantido à mão: apagar o
//! índice e sincronizar de novo recupera tudo.
//!
//! # De onde vem cada elo
//!
//! - **Demanda → análises:** o campo `demanda.id` do frontmatter das análises (a
//!   ligação `demanda` do índice) e, para uma análise sem demanda própria, a da
//!   análise de origem (`origem: A-NNNN`, a cadeia histórias → impacto).
//! - **Análise → histórias, critérios, cenários e afirmações:** o estado da análise
//!   (`analises/A-NNNN.estado.json`), que o índice passa a ler: cada história
//!   (`HU-NNNN`), critério (`HU-NNNN/CA-n`), cenário (`CT-NNNN`) e afirmação
//!   (`A-NNNN/n`) vira uma linha de `rastro_item`, com o que ela cita
//!   (`rastro_cita`: a regra `RN-…`, a afirmação `A-NNNN/n`, a história do cenário)
//!   e as evidências dela (`rastro_evidencia`).
//! - **Regras:** os documentos `regra` do índice, com as evidências do frontmatter
//!   e o `status` (`a-revalidar` quando a revalidação viu o código mudar).
//! - **Regras de uma história:** as citadas pelos critérios e, pelas afirmações
//!   citadas, as que a afirmação menciona no texto ou cuja evidência cobre o mesmo
//!   trecho de código (mesmo arquivo, linhas que se cruzam).
//! - **Regras afetadas pela demanda:** as mencionadas nas afirmações das análises
//!   da demanda, as que têm evidência no mesmo trecho de uma dessas afirmações, as
//!   criadas pela aprovação delas (`origem`) e as citadas por critérios e cenários.
//!
//! # Lacunas
//!
//! - história sem cenário de teste ([`Alerta::HistoriaSemCenario`]);
//! - história sem regra nem evidência que a sustente ([`Alerta::HistoriaSemRegra`]);
//! - regra afetada sem história ([`Alerta::RegraSemHistoria`]);
//! - evidência a revalidar sob uma história aprovada (aceita, ou de análise
//!   aprovada) ([`Alerta::EvidenciaARevalidar`]): a afirmação citada está "a
//!   revalidar" ou tem evidência "a revalidar", a regra citada está `a-revalidar`,
//!   ou o próprio critério ou um cenário da história traz a marca. A marca direta é
//!   lida de duas formas, a que a revalidação das histórias e cenários (#169) vier a
//!   gravar: `estado: "a-revalidar"` numa evidência do item (como já acontece no
//!   limite dos cenários) ou no próprio item.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::io;
use std::path::PathBuf;

use rusqlite::{Connection, Transaction, params};
use serde::{Deserialize, Serialize};

use crate::base::Base;
use crate::caminho::CaminhoRelativo;
use crate::id::{Id, IdDemanda, Serie};
use crate::layout;

use super::{Guardado, Indice, ResultadoIndice, Sincronizacao};

/// Sufixo do arquivo de estado de uma análise (`A-0001.estado.json`).
const SUFIXO_ESTADO: &str = ".estado.json";

/// Tabelas da rastreabilidade. `rastro_estado` acompanha os arquivos de estado
/// das análises do mesmo jeito que `documento` acompanha os Markdown (hash,
/// tamanho e data, para a atualização incremental).
pub(super) const CRIAR: &str = "
CREATE TABLE rastro_estado (
    id INTEGER PRIMARY KEY,
    caminho TEXT NOT NULL UNIQUE,
    projeto TEXT NOT NULL,
    analise TEXT NOT NULL,
    fluxo TEXT,
    estado TEXT,
    titulo TEXT NOT NULL,
    origem TEXT,
    erro TEXT,
    hash TEXT NOT NULL,
    tamanho INTEGER NOT NULL,
    modificado_ns INTEGER NOT NULL,
    verificado_ns INTEGER NOT NULL
);
CREATE INDEX rastro_estado_projeto ON rastro_estado (projeto, analise);
CREATE TABLE rastro_item (
    estado INTEGER NOT NULL REFERENCES rastro_estado (id),
    chave TEXT NOT NULL,
    tipo TEXT NOT NULL,
    historia TEXT,
    titulo TEXT NOT NULL,
    situacao TEXT,
    a_revalidar INTEGER NOT NULL,
    a_confirmar INTEGER NOT NULL,
    PRIMARY KEY (estado, chave)
);
CREATE TABLE rastro_cita (
    estado INTEGER NOT NULL REFERENCES rastro_estado (id),
    item TEXT NOT NULL,
    alvo TEXT NOT NULL,
    tipo TEXT NOT NULL,
    PRIMARY KEY (estado, item, alvo, tipo)
);
CREATE INDEX rastro_cita_alvo ON rastro_cita (alvo);
CREATE TABLE rastro_evidencia (
    estado INTEGER NOT NULL REFERENCES rastro_estado (id),
    item TEXT NOT NULL,
    ordem INTEGER NOT NULL,
    arquivo TEXT NOT NULL,
    linha_inicial INTEGER NOT NULL,
    linha_final INTEGER NOT NULL,
    a_revalidar INTEGER NOT NULL,
    PRIMARY KEY (estado, item, ordem)
);
CREATE INDEX rastro_evidencia_arquivo ON rastro_evidencia (arquivo);
";

pub(super) const APAGAR: &str = "
DROP TABLE IF EXISTS rastro_evidencia;
DROP TABLE IF EXISTS rastro_cita;
DROP TABLE IF EXISTS rastro_item;
DROP TABLE IF EXISTS rastro_estado;
";

pub(super) const LIMPAR: &str = "
DELETE FROM rastro_evidencia;
DELETE FROM rastro_cita;
DELETE FROM rastro_item;
DELETE FROM rastro_estado;
";

// ---------------------------------------------------------------------------
// Sincronização dos arquivos de estado

/// Arquivo de estado encontrado na base.
pub(super) struct EstadoEncontrado {
    caminho: String,
    projeto: String,
    disco: PathBuf,
    tamanho: i64,
    modificado_ns: i64,
}

impl EstadoEncontrado {
    fn igual(&self, g: &Guardado) -> bool {
        g.tamanho == self.tamanho
            && g.modificado_ns == self.modificado_ns
            && g.verificado_ns - g.modificado_ns > super::JANELA_DE_INCERTEZA_NS
    }
}

/// Arquivos `analises/A-NNNN.estado.json` de todos os projetos da base.
pub(super) fn listar(base: &Base) -> ResultadoIndice<Vec<EstadoEncontrado>> {
    let mut saida = Vec::new();
    for nome in base.projetos()? {
        let Ok(projeto) = base.projeto(&nome) else {
            continue;
        };
        let pasta = projeto.raiz().join(layout::ANALISES);
        let entradas = match fs::read_dir(&pasta) {
            Ok(e) => e,
            Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
            Err(erro) => {
                return Err(super::ErroIndice::Io {
                    caminho: pasta,
                    erro,
                });
            }
        };
        for entrada in entradas.flatten() {
            let arquivo = entrada.file_name().to_string_lossy().into_owned();
            let Some(id) = arquivo.strip_suffix(SUFIXO_ESTADO) else {
                continue;
            };
            if !Id::analisar(id).is_ok_and(|i| i.serie() == &Serie::Analise) {
                continue;
            }
            let Ok(relativo) = CaminhoRelativo::novo(&format!("{}/{arquivo}", layout::ANALISES))
            else {
                continue;
            };
            let disco = entrada.path();
            let Ok(meta) = fs::metadata(&disco) else {
                continue;
            };
            if !meta.is_file() {
                continue;
            }
            saida.push(EstadoEncontrado {
                caminho: projeto.na_base(&relativo).as_str().to_owned(),
                projeto: nome.clone(),
                disco,
                tamanho: i64::try_from(meta.len()).unwrap_or(i64::MAX),
                modificado_ns: meta.modified().map(super::ns_desde_epoca).unwrap_or(0),
            });
        }
    }
    saida.sort_by(|a, b| a.caminho.cmp(&b.caminho));
    Ok(saida)
}

fn guardados(conexao: &Connection) -> ResultadoIndice<HashMap<String, Guardado>> {
    let mut consulta = conexao.prepare(
        "SELECT caminho, id, hash, tamanho, modificado_ns, verificado_ns FROM rastro_estado",
    )?;
    let linhas = consulta.query_map([], |l| {
        Ok((
            l.get::<_, String>(0)?,
            Guardado {
                id: l.get(1)?,
                hash: l.get(2)?,
                tamanho: l.get(3)?,
                modificado_ns: l.get(4)?,
                verificado_ns: l.get(5)?,
            },
        ))
    })?;
    let mut saida = HashMap::new();
    for linha in linhas {
        let (caminho, g) = linha?;
        saida.insert(caminho, g);
    }
    Ok(saida)
}

/// Nenhum arquivo de estado mudou desde a última sincronização (sem gravar nada).
pub(super) fn nada_mudou(
    conexao: &Connection,
    encontrados: &[EstadoEncontrado],
) -> ResultadoIndice<bool> {
    let guardados = guardados(conexao)?;
    Ok(encontrados.len() == guardados.len()
        && encontrados
            .iter()
            .all(|e| guardados.get(&e.caminho).is_some_and(|g| e.igual(g))))
}

/// Atualiza os arquivos de estado no índice, dentro da transação da
/// sincronização, e soma o que mudou ao resumo.
pub(super) fn sincronizar(
    tx: &Transaction<'_>,
    encontrados: &[EstadoEncontrado],
    resumo: &mut Sincronizacao,
    agora: i64,
) -> ResultadoIndice<()> {
    let mut guardados = guardados(tx)?;
    for e in encontrados {
        let anterior = guardados.remove(&e.caminho);
        if let Some(g) = &anterior
            && e.igual(g)
        {
            resumo.inalterados += 1;
            continue;
        }
        let bytes = match fs::read(&e.disco) {
            Ok(b) => b,
            Err(erro) if erro.kind() == io::ErrorKind::NotFound => {
                if let Some(g) = anterior {
                    guardados.insert(e.caminho.clone(), g);
                }
                continue;
            }
            Err(erro) => {
                tracing::warn!(arquivo = %e.caminho, %erro, "índice: estado ilegível");
                continue;
            }
        };
        let hash = super::hash(&bytes);
        match anterior {
            Some(g) if g.hash == hash => {
                tx.execute(
                    "UPDATE rastro_estado SET tamanho = ?1, modificado_ns = ?2, verificado_ns = ?3 \
                     WHERE id = ?4",
                    params![e.tamanho, e.modificado_ns, agora, g.id],
                )?;
                resumo.inalterados += 1;
            }
            anterior => {
                if let Some(g) = &anterior {
                    remover(tx, g.id)?;
                    resumo.alterados += 1;
                } else {
                    resumo.novos += 1;
                }
                inserir(tx, e, &hash, agora, &bytes)?;
            }
        }
    }
    for g in guardados.values() {
        remover(tx, g.id)?;
        resumo.removidos += 1;
    }
    Ok(())
}

fn remover(tx: &Transaction<'_>, id: i64) -> rusqlite::Result<()> {
    tx.execute("DELETE FROM rastro_evidencia WHERE estado = ?1", [id])?;
    tx.execute("DELETE FROM rastro_cita WHERE estado = ?1", [id])?;
    tx.execute("DELETE FROM rastro_item WHERE estado = ?1", [id])?;
    tx.execute("DELETE FROM rastro_estado WHERE id = ?1", [id])?;
    Ok(())
}

fn inserir(
    tx: &Transaction<'_>,
    e: &EstadoEncontrado,
    hash: &str,
    agora: i64,
    bytes: &[u8],
) -> rusqlite::Result<()> {
    let nome = e.caminho.rsplit('/').next().unwrap_or_default();
    let id_do_arquivo = nome.strip_suffix(SUFIXO_ESTADO).unwrap_or(nome).to_owned();
    let (extraido, erro) = match serde_json::from_slice::<EstadoLido>(bytes) {
        Ok(lido) => (extrair(lido), None),
        Err(erro) => (Extraido::default(), Some(erro.to_string())),
    };
    tx.execute(
        "INSERT INTO rastro_estado (caminho, projeto, analise, fluxo, estado, titulo, origem, \
             erro, hash, tamanho, modificado_ns, verificado_ns) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            e.caminho,
            e.projeto,
            id_do_arquivo,
            extraido.fluxo,
            extraido.estado,
            extraido.titulo,
            extraido.origem,
            erro,
            hash,
            e.tamanho,
            e.modificado_ns,
            agora,
        ],
    )?;
    let estado = tx.last_insert_rowid();
    for item in &extraido.itens {
        tx.execute(
            "INSERT OR IGNORE INTO rastro_item (estado, chave, tipo, historia, titulo, situacao, \
                 a_revalidar, a_confirmar) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                estado,
                item.chave,
                item.tipo.como_str(),
                item.historia,
                item.titulo,
                item.situacao,
                item.a_revalidar,
                item.a_confirmar,
            ],
        )?;
        for (alvo, tipo) in &item.cita {
            tx.execute(
                "INSERT OR IGNORE INTO rastro_cita (estado, item, alvo, tipo) \
                 VALUES (?1, ?2, ?3, ?4)",
                params![estado, item.chave, alvo, tipo],
            )?;
        }
        for (ordem, ev) in item.evidencias.iter().enumerate() {
            tx.execute(
                "INSERT OR IGNORE INTO rastro_evidencia (estado, item, ordem, arquivo, \
                     linha_inicial, linha_final, a_revalidar) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    estado,
                    item.chave,
                    i64::try_from(ordem).unwrap_or(i64::MAX),
                    ev.arquivo,
                    ev.inicio,
                    ev.fim,
                    ev.a_revalidar,
                ],
            )?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Leitura do estado da análise
//
// Só os campos que a rastreabilidade usa, com valor padrão para todos: um estado
// de outra versão (campo novo, campo a menos) continua rastreável. O formato
// completo é o de `cba_core::analise::Analise`, que esta crate não conhece.

const A_REVALIDAR: &str = "a-revalidar";

#[derive(Debug, Default, Deserialize)]
struct EstadoLido {
    #[serde(default)]
    fluxo: Option<String>,
    #[serde(default)]
    estado: Option<String>,
    #[serde(default)]
    titulo: String,
    #[serde(default)]
    origem: Option<String>,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    afirmacoes: Vec<AfirmacaoLida>,
    #[serde(default)]
    historias: Vec<HistoriaLida>,
    #[serde(default)]
    cenarios: Vec<CenarioLido>,
}

#[derive(Debug, Default, Deserialize)]
struct EvidenciaLida {
    #[serde(default)]
    arquivo: String,
    #[serde(default)]
    linhas: serde_json::Value,
    #[serde(default)]
    estado: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct AfirmacaoLida {
    #[serde(default)]
    numero: u32,
    #[serde(default)]
    texto: String,
    #[serde(default)]
    estado: Option<String>,
    #[serde(default)]
    evidencias: Vec<EvidenciaLida>,
}

#[derive(Debug, Default, Deserialize)]
struct FundamentoLido {
    #[serde(default)]
    tipo: String,
    #[serde(default, rename = "ref")]
    referencia: Option<String>,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    evidencias: Vec<EvidenciaLida>,
}

#[derive(Debug, Default, Deserialize)]
struct CriterioLido {
    #[serde(default)]
    numero: u32,
    #[serde(default)]
    entao: String,
    #[serde(default)]
    fundamentos: Vec<FundamentoLido>,
    #[serde(default)]
    pergunta: Option<serde_json::Value>,
    #[serde(default)]
    situacao: Option<String>,
    /// Marca direta (#169), se vier.
    #[serde(default)]
    estado: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct HistoriaLida {
    #[serde(default)]
    id: String,
    #[serde(default)]
    titulo: String,
    #[serde(default)]
    situacao: Option<String>,
    #[serde(default)]
    criterios: Vec<CriterioLido>,
}

#[derive(Debug, Default, Deserialize)]
struct LimiteLido {
    #[serde(default)]
    evidencias: Vec<EvidenciaLida>,
}

#[derive(Debug, Default, Deserialize)]
struct CenarioLido {
    #[serde(default)]
    id: String,
    #[serde(default)]
    titulo: String,
    #[serde(default)]
    historia: Option<String>,
    #[serde(default)]
    fundamentos: Vec<FundamentoLido>,
    #[serde(default)]
    limite: Option<LimiteLido>,
    #[serde(default)]
    pergunta: Option<serde_json::Value>,
    #[serde(default)]
    situacao: Option<String>,
    /// Marca direta (#169), se vier.
    #[serde(default)]
    estado: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TipoItem {
    Afirmacao,
    Historia,
    Criterio,
    Cenario,
}

impl TipoItem {
    fn como_str(self) -> &'static str {
        match self {
            Self::Afirmacao => "afirmacao",
            Self::Historia => "historia",
            Self::Criterio => "criterio",
            Self::Cenario => "cenario",
        }
    }

    fn de_str(texto: &str) -> Option<Self> {
        [
            Self::Afirmacao,
            Self::Historia,
            Self::Criterio,
            Self::Cenario,
        ]
        .into_iter()
        .find(|t| t.como_str() == texto)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Ev {
    arquivo: String,
    inicio: u32,
    fim: u32,
    a_revalidar: bool,
}

impl Ev {
    fn cruza(&self, outra: &Ev) -> bool {
        self.arquivo == outra.arquivo && self.inicio <= outra.fim && outra.inicio <= self.fim
    }
}

#[derive(Debug)]
struct ItemExtraido {
    chave: String,
    tipo: TipoItem,
    historia: Option<String>,
    titulo: String,
    situacao: Option<String>,
    a_revalidar: bool,
    a_confirmar: bool,
    cita: Vec<(String, &'static str)>,
    evidencias: Vec<Ev>,
}

#[derive(Debug, Default)]
struct Extraido {
    fluxo: Option<String>,
    estado: Option<String>,
    titulo: String,
    origem: Option<String>,
    itens: Vec<ItemExtraido>,
}

fn linhas(valor: &serde_json::Value) -> Option<(u32, u32)> {
    let texto = match valor {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        _ => return None,
    };
    let (a, b) = texto.split_once('-').unwrap_or((&texto, &texto));
    let a: u32 = a.trim().parse().ok()?;
    let b: u32 = b.trim().parse().ok()?;
    (a >= 1 && b >= a).then_some((a, b))
}

fn evidencias(lidas: &[EvidenciaLida]) -> Vec<Ev> {
    lidas
        .iter()
        .filter_map(|e| {
            let (inicio, fim) = linhas(&e.linhas)?;
            (!e.arquivo.trim().is_empty()).then(|| Ev {
                arquivo: e.arquivo.trim().to_owned(),
                inicio,
                fim,
                a_revalidar: e.estado.as_deref() == Some(A_REVALIDAR),
            })
        })
        .collect()
}

/// Regras citadas num texto (`RN-FAT-007`), na ordem, sem repetir.
fn regras_no_texto(texto: &str) -> Vec<String> {
    let mut saida: Vec<String> = Vec::new();
    let mut resto = texto;
    while let Some(i) = resto.find("RN-") {
        let antes = resto[..i].chars().next_back();
        let depois = &resto[i..];
        let fim = depois
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
            .unwrap_or(depois.len());
        let candidato = depois[..fim].trim_end_matches('-');
        if !antes.is_some_and(|c| c.is_alphanumeric())
            && let Ok(id) = Id::analisar(candidato)
        {
            let id = id.to_string();
            if !saida.contains(&id) {
                saida.push(id);
            }
        }
        resto = &resto[i + 3..];
    }
    saida
}

fn fundamentos(lidos: &[FundamentoLido]) -> (Vec<(String, &'static str)>, Vec<Ev>) {
    let mut cita = Vec::new();
    let mut evs = Vec::new();
    for f in lidos {
        match f.tipo.as_str() {
            "afirmacao" => cita.extend(f.referencia.clone().map(|r| (r, "afirmacao"))),
            "regra" => cita.extend(f.id.clone().map(|r| (r, "regra"))),
            _ => {}
        }
        evs.extend(evidencias(&f.evidencias));
    }
    (cita, evs)
}

fn extrair(lido: EstadoLido) -> Extraido {
    let analise = lido.id.clone().unwrap_or_default();
    let mut itens = Vec::new();
    for a in &lido.afirmacoes {
        let evs = evidencias(&a.evidencias);
        itens.push(ItemExtraido {
            chave: format!("{analise}/{}", a.numero),
            tipo: TipoItem::Afirmacao,
            historia: None,
            titulo: a.texto.clone(),
            situacao: a.estado.clone(),
            a_revalidar: a.estado.as_deref() == Some(A_REVALIDAR)
                || evs.iter().any(|e| e.a_revalidar),
            a_confirmar: false,
            cita: regras_no_texto(&a.texto)
                .into_iter()
                .map(|r| (r, "regra"))
                .collect(),
            evidencias: evs,
        });
    }
    for h in &lido.historias {
        itens.push(ItemExtraido {
            chave: h.id.clone(),
            tipo: TipoItem::Historia,
            historia: Some(h.id.clone()),
            titulo: h.titulo.clone(),
            situacao: h.situacao.clone(),
            a_revalidar: false,
            a_confirmar: false,
            cita: Vec::new(),
            evidencias: Vec::new(),
        });
        for c in &h.criterios {
            let (cita, evs) = fundamentos(&c.fundamentos);
            itens.push(ItemExtraido {
                chave: format!("{}/CA-{}", h.id, c.numero),
                tipo: TipoItem::Criterio,
                historia: Some(h.id.clone()),
                titulo: c.entao.clone(),
                situacao: c.situacao.clone(),
                a_revalidar: c.estado.as_deref() == Some(A_REVALIDAR)
                    || evs.iter().any(|e| e.a_revalidar),
                a_confirmar: c.pergunta.as_ref().is_some_and(|p| !p.is_null()),
                cita,
                evidencias: evs,
            });
        }
    }
    for c in &lido.cenarios {
        let (mut cita, mut evs) = fundamentos(&c.fundamentos);
        if let Some(l) = &c.limite {
            let mut do_limite = evidencias(&l.evidencias);
            do_limite.append(&mut evs);
            evs = do_limite;
        }
        if let Some(h) = &c.historia {
            cita.push((h.clone(), "historia"));
        }
        itens.push(ItemExtraido {
            chave: c.id.clone(),
            tipo: TipoItem::Cenario,
            historia: c.historia.clone(),
            titulo: c.titulo.clone(),
            situacao: c.situacao.clone(),
            a_revalidar: c.estado.as_deref() == Some(A_REVALIDAR)
                || evs.iter().any(|e| e.a_revalidar),
            a_confirmar: c.pergunta.as_ref().is_some_and(|p| !p.is_null()),
            cita,
            evidencias: evs,
        });
    }
    itens.retain(|i| !i.chave.is_empty() && !i.chave.starts_with('/'));
    Extraido {
        fluxo: lido.fluxo,
        estado: lido.estado,
        titulo: lido.titulo,
        origem: lido.origem,
        itens,
    }
}

// ---------------------------------------------------------------------------
// O que as consultas devolvem

/// Lacuna ou alerta da rastreabilidade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Alerta {
    /// Evidência a revalidar sob uma história aprovada.
    EvidenciaARevalidar,
    /// Regra afetada pela demanda que não entrou em nenhuma história.
    RegraSemHistoria,
    /// História sem cenário de teste.
    HistoriaSemCenario,
    /// História sem regra nem evidência que a sustente.
    HistoriaSemRegra,
}

impl Alerta {
    /// Nome curto para o analista.
    pub fn rotulo(self) -> &'static str {
        match self {
            Self::EvidenciaARevalidar => "evidência a revalidar",
            Self::RegraSemHistoria => "regra afetada sem história",
            Self::HistoriaSemCenario => "sem cenário",
            Self::HistoriaSemRegra => "sem regra",
        }
    }
}

/// Demanda na rastreabilidade.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DemandaRastro {
    /// `D-0001`.
    pub id: String,
    pub titulo: String,
    /// `aberta` ou `encerrada`.
    pub status: Option<String>,
    /// `demanda.md`, relativo à base.
    pub documento: Option<String>,
}

/// Análise na rastreabilidade.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnaliseRastro {
    /// `A-0001`.
    pub id: String,
    pub titulo: String,
    /// Fluxo do estado (`impacto`, `historias`, `cenarios`…).
    pub fluxo: Option<String>,
    /// Estado da análise (`em-andamento`, `aprovada`…) ou, sem o arquivo de
    /// estado, o `status` do documento.
    pub estado: Option<String>,
    /// Documento, relativo à base.
    pub documento: Option<String>,
    /// Análise de origem (`A-NNNN`).
    pub origem: Option<String>,
    /// Demanda, própria ou herdada da origem.
    pub demanda: Option<String>,
}

/// Regra na rastreabilidade.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegraRastro {
    /// `RN-FAT-007`.
    pub id: String,
    pub titulo: String,
    /// `confirmada`, `a-revalidar` ou `obsoleta`; `None` se a regra não está na base.
    pub status: Option<String>,
    /// Documento, relativo à base; `None` se a regra não está na base.
    pub documento: Option<String>,
}

/// Trecho de código que sustenta um elo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenciaRastro {
    /// Relativo à raiz do projeto analisado.
    pub arquivo: String,
    pub linha_inicial: u32,
    pub linha_final: u32,
    pub a_revalidar: bool,
    /// De onde veio: a afirmação (`A-0001/2`), a regra (`RN-FAT-007`) ou o item
    /// (`HU-0001/CA-1`, `CT-0001`).
    pub de: String,
}

/// Critério de aceite na rastreabilidade.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CriterioRastro {
    /// `HU-0001`.
    pub historia: String,
    /// `CA-1`.
    pub criterio: String,
    /// O "então" do critério.
    pub texto: String,
    /// Análise de histórias.
    pub analise: String,
    pub situacao: Option<String>,
    /// O critério depende de algo a revalidar (ele mesmo, a afirmação ou a regra
    /// citada).
    pub a_revalidar: bool,
    pub a_confirmar: bool,
}

/// Cenário de teste na rastreabilidade.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CenarioRastro {
    /// `CT-0001`.
    pub id: String,
    pub titulo: String,
    /// Análise de cenários.
    pub analise: String,
    pub historia: Option<String>,
    pub situacao: Option<String>,
    pub a_revalidar: bool,
    pub a_confirmar: bool,
}

/// Linha da matriz por história.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinhaHistoria {
    /// `HU-0001`.
    pub id: String,
    pub titulo: String,
    /// Análise de histórias.
    pub analise: String,
    /// `pendente` ou `aceita`.
    pub situacao: Option<String>,
    /// Aceita, ou da análise aprovada.
    pub aprovada: bool,
    /// Algum critério depende de pergunta em aberto.
    pub a_confirmar: bool,
    /// Análises do elo: a das histórias, a de origem e as das afirmações citadas.
    pub analises: Vec<String>,
    pub criterios: Vec<CriterioRastro>,
    /// Afirmações citadas pelos critérios (`A-0001/2`).
    pub afirmacoes: Vec<String>,
    pub regras: Vec<RegraRastro>,
    pub evidencias: Vec<EvidenciaRastro>,
    pub cenarios: Vec<CenarioRastro>,
    pub alertas: Vec<Alerta>,
}

/// Linha da matriz por regra.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinhaRegra {
    pub regra: RegraRastro,
    /// Análises da demanda que afetam a regra.
    pub analises: Vec<String>,
    /// Histórias que se apoiam na regra.
    pub historias: Vec<String>,
    pub evidencias: Vec<EvidenciaRastro>,
    pub cenarios: Vec<CenarioRastro>,
    pub alertas: Vec<Alerta>,
}

/// Lacuna com a explicação para o analista.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Lacuna {
    pub alerta: Alerta,
    /// História (`HU-…`) ou regra (`RN-…`).
    pub alvo: String,
    pub descricao: String,
    /// Itens envolvidos (regras, afirmações, critérios, cenários, análises).
    pub relacionados: Vec<String>,
}

/// Matriz de rastreabilidade de uma demanda.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Matriz {
    pub projeto: String,
    pub demanda: DemandaRastro,
    pub analises: Vec<AnaliseRastro>,
    pub historias: Vec<LinhaHistoria>,
    pub regras: Vec<LinhaRegra>,
    pub lacunas: Vec<Lacuna>,
}

/// Ponto de partida da consulta reversa.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "tipo", content = "valor", rename_all = "kebab-case")]
pub enum AlvoReverso {
    /// Regra (`RN-FAT-007`).
    Regra(String),
    /// Arquivo do projeto analisado, relativo à raiz dele (`db/multa.sql`).
    Arquivo(String),
}

/// O que depende de uma regra ou de um arquivo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Dependentes {
    pub projeto: String,
    pub alvo: AlvoReverso,
    /// A regra consultada (consulta por regra).
    pub regra: Option<RegraRastro>,
    /// Arquivos das evidências da regra (consulta por regra), para consultar por
    /// arquivo em seguida.
    pub arquivos: Vec<String>,
    /// Regras com evidência no arquivo (consulta por arquivo).
    pub regras: Vec<RegraRastro>,
    /// Abertas primeiro.
    pub demandas: Vec<DemandaRastro>,
    /// Em andamento primeiro, aprovadas depois.
    pub analises: Vec<AnaliseRastro>,
    pub criterios: Vec<CriterioRastro>,
    pub cenarios: Vec<CenarioRastro>,
}

impl Dependentes {
    /// Nada depende do alvo.
    pub fn vazio(&self) -> bool {
        self.demandas.is_empty()
            && self.analises.is_empty()
            && self.criterios.is_empty()
            && self.cenarios.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Modelo em memória de um projeto

#[derive(Debug)]
struct ItemM {
    analise: String,
    chave: String,
    tipo: TipoItem,
    historia: Option<String>,
    titulo: String,
    situacao: Option<String>,
    a_revalidar: bool,
    a_confirmar: bool,
    cita: Vec<(String, String)>,
    evidencias: Vec<Ev>,
}

impl ItemM {
    fn cita(&self, tipo: &str) -> impl Iterator<Item = &str> {
        self.cita
            .iter()
            .filter(move |(_, t)| t == tipo)
            .map(|(a, _)| a.as_str())
    }

    fn valido(&self) -> bool {
        !matches!(
            self.situacao.as_deref(),
            Some("descartado" | "descartada" | "quebrada" | "incorreta")
        )
    }
}

#[derive(Debug)]
struct RegraM {
    vista: RegraRastro,
    origem: Option<String>,
    evidencias: Vec<Ev>,
}

#[derive(Debug)]
struct AnaliseM {
    vista: AnaliseRastro,
    demanda_propria: Option<String>,
}

#[derive(Debug, Default)]
struct Modelo {
    projeto: String,
    analises: BTreeMap<String, AnaliseM>,
    itens: Vec<ItemM>,
    regras: BTreeMap<String, RegraM>,
    demandas: BTreeMap<String, DemandaRastro>,
}

/// Identificador da análise de um caminho `…/analises/A-0001.md`.
fn analise_do_caminho(caminho: &str) -> Option<String> {
    let nome = caminho.rsplit('/').next()?;
    let id = nome.strip_suffix(".md")?;
    Id::analisar(id)
        .ok()
        .filter(|i| i.serie() == &Serie::Analise)
        .map(|i| i.to_string())
}

/// Demanda de um caminho `<projeto>/demandas/D-0001-slug/demanda.md`.
fn demanda_do_caminho(caminho: &str) -> Option<String> {
    let mut partes = caminho.split('/');
    partes.next()?;
    (partes.next()? == layout::DEMANDAS).then_some(())?;
    IdDemanda::de_pasta(partes.next()?).map(|d| d.to_string())
}

const TIPOS_DE_ANALISE: [&str; 7] = [
    "analise-impacto",
    "pergunta",
    "funcionalidade",
    "analise-reuniao",
    "parecer-revisao",
    "historias",
    "cenarios-teste",
];

impl Modelo {
    fn carregar(conexao: &Connection, projeto: &str) -> ResultadoIndice<Self> {
        let mut m = Modelo {
            projeto: projeto.to_owned(),
            ..Default::default()
        };
        // Documentos: regras, demandas e análises.
        let mut consulta = conexao.prepare(
            "SELECT caminho, tipo, identificador, titulo, status FROM documento \
             WHERE projeto = ?1 AND tipo IS NOT NULL ORDER BY caminho",
        )?;
        let linhas = consulta.query_map([projeto], |l| {
            Ok((
                l.get::<_, String>(0)?,
                l.get::<_, String>(1)?,
                l.get::<_, Option<String>>(2)?,
                l.get::<_, String>(3)?,
                l.get::<_, Option<String>>(4)?,
            ))
        })?;
        for linha in linhas {
            let (caminho, tipo, identificador, titulo, status) = linha?;
            match tipo.as_str() {
                "regra" => {
                    if let Some(id) = identificador {
                        m.regras.insert(
                            id.clone(),
                            RegraM {
                                vista: RegraRastro {
                                    id,
                                    titulo,
                                    status,
                                    documento: Some(caminho),
                                },
                                origem: None,
                                evidencias: Vec::new(),
                            },
                        );
                    }
                }
                "demanda" => {
                    if let Some(id) = demanda_do_caminho(&caminho) {
                        m.demandas.insert(
                            id.clone(),
                            DemandaRastro {
                                id,
                                titulo,
                                status,
                                documento: Some(caminho),
                            },
                        );
                    }
                }
                t if TIPOS_DE_ANALISE.contains(&t) => {
                    if let Some(id) = analise_do_caminho(&caminho) {
                        m.analises.insert(
                            id.clone(),
                            AnaliseM {
                                vista: AnaliseRastro {
                                    id,
                                    titulo,
                                    fluxo: None,
                                    estado: status,
                                    documento: Some(caminho),
                                    origem: None,
                                    demanda: None,
                                },
                                demanda_propria: None,
                            },
                        );
                    }
                }
                _ => {}
            }
        }
        drop(consulta);

        // Ligações: demanda das análises e origem das regras.
        let mut consulta = conexao.prepare(
            "SELECT d.caminho, d.tipo, l.para, l.campo FROM ligacao l \
             JOIN documento d ON d.id = l.documento \
             WHERE d.projeto = ?1 AND l.campo IN ('demanda', 'origem')",
        )?;
        let linhas = consulta.query_map([projeto], |l| {
            Ok((
                l.get::<_, String>(0)?,
                l.get::<_, Option<String>>(1)?,
                l.get::<_, String>(2)?,
                l.get::<_, String>(3)?,
            ))
        })?;
        for linha in linhas {
            let (de, tipo, para, campo) = linha?;
            match (campo.as_str(), tipo.as_deref()) {
                ("demanda", _) => {
                    if let (Some(a), Some(d)) = (analise_do_caminho(&de), demanda_do_caminho(&para))
                        && let Some(analise) = m.analises.get_mut(&a)
                    {
                        analise.demanda_propria = Some(d);
                    }
                }
                ("origem", Some("regra")) => {
                    if let Some(a) = analise_do_caminho(&para)
                        && let Some(r) = m
                            .regras
                            .values_mut()
                            .find(|r| r.vista.documento.as_deref() == Some(de.as_str()))
                    {
                        r.origem = Some(a);
                    }
                }
                _ => {}
            }
        }
        drop(consulta);

        // Evidências das regras.
        let mut consulta = conexao.prepare(
            "SELECT d.identificador, e.arquivo, e.linha_inicial, e.linha_final \
             FROM evidencia e JOIN documento d ON d.id = e.documento \
             WHERE d.projeto = ?1 AND d.tipo = 'regra' ORDER BY d.caminho, e.ordem",
        )?;
        let linhas = consulta.query_map([projeto], |l| {
            Ok((
                l.get::<_, Option<String>>(0)?,
                l.get::<_, String>(1)?,
                l.get::<_, u32>(2)?,
                l.get::<_, u32>(3)?,
            ))
        })?;
        for linha in linhas {
            let (id, arquivo, inicio, fim) = linha?;
            if let Some(r) = id.and_then(|id| m.regras.get_mut(&id)) {
                let a_revalidar = r.vista.status.as_deref() == Some(A_REVALIDAR);
                r.evidencias.push(Ev {
                    arquivo,
                    inicio,
                    fim,
                    a_revalidar,
                });
            }
        }
        drop(consulta);

        // Estados das análises.
        let mut consulta = conexao.prepare(
            "SELECT id, caminho, analise, fluxo, estado, titulo, origem FROM rastro_estado \
             WHERE projeto = ?1 ORDER BY analise",
        )?;
        let linhas = consulta.query_map([projeto], |l| {
            Ok((
                l.get::<_, i64>(0)?,
                l.get::<_, String>(1)?,
                l.get::<_, String>(2)?,
                l.get::<_, Option<String>>(3)?,
                l.get::<_, Option<String>>(4)?,
                l.get::<_, String>(5)?,
                l.get::<_, Option<String>>(6)?,
            ))
        })?;
        let mut por_estado: HashMap<i64, String> = HashMap::new();
        for linha in linhas {
            let (id, _caminho, analise, fluxo, estado, titulo, origem) = linha?;
            por_estado.insert(id, analise.clone());
            let a = m
                .analises
                .entry(analise.clone())
                .or_insert_with(|| AnaliseM {
                    vista: AnaliseRastro {
                        id: analise.clone(),
                        titulo: titulo.clone(),
                        fluxo: None,
                        estado: None,
                        documento: None,
                        origem: None,
                        demanda: None,
                    },
                    demanda_propria: None,
                });
            a.vista.fluxo = fluxo;
            if estado.is_some() {
                a.vista.estado = estado;
            }
            if a.vista.titulo.is_empty() {
                a.vista.titulo = titulo;
            }
            a.vista.origem = origem;
        }
        drop(consulta);

        let mut consulta = conexao.prepare(
            "SELECT i.estado, i.chave, i.tipo, i.historia, i.titulo, i.situacao, \
                    i.a_revalidar, i.a_confirmar \
             FROM rastro_item i JOIN rastro_estado s ON s.id = i.estado \
             WHERE s.projeto = ?1 ORDER BY s.analise, i.rowid",
        )?;
        let linhas = consulta.query_map([projeto], |l| {
            Ok((
                l.get::<_, i64>(0)?,
                l.get::<_, String>(1)?,
                l.get::<_, String>(2)?,
                l.get::<_, Option<String>>(3)?,
                l.get::<_, String>(4)?,
                l.get::<_, Option<String>>(5)?,
                l.get::<_, bool>(6)?,
                l.get::<_, bool>(7)?,
            ))
        })?;
        let mut indice_do_item: HashMap<(i64, String), usize> = HashMap::new();
        for linha in linhas {
            let (estado, chave, tipo, historia, titulo, situacao, a_revalidar, a_confirmar) =
                linha?;
            let (Some(analise), Some(tipo)) = (por_estado.get(&estado), TipoItem::de_str(&tipo))
            else {
                continue;
            };
            indice_do_item.insert((estado, chave.clone()), m.itens.len());
            m.itens.push(ItemM {
                analise: analise.clone(),
                chave,
                tipo,
                historia,
                titulo,
                situacao,
                a_revalidar,
                a_confirmar,
                cita: Vec::new(),
                evidencias: Vec::new(),
            });
        }
        drop(consulta);

        let mut consulta = conexao.prepare(
            "SELECT c.estado, c.item, c.alvo, c.tipo FROM rastro_cita c \
             JOIN rastro_estado s ON s.id = c.estado WHERE s.projeto = ?1 \
             ORDER BY c.rowid",
        )?;
        let linhas = consulta.query_map([projeto], |l| {
            Ok((
                l.get::<_, i64>(0)?,
                l.get::<_, String>(1)?,
                l.get::<_, String>(2)?,
                l.get::<_, String>(3)?,
            ))
        })?;
        for linha in linhas {
            let (estado, item, alvo, tipo) = linha?;
            if let Some(&i) = indice_do_item.get(&(estado, item)) {
                m.itens[i].cita.push((alvo, tipo));
            }
        }
        drop(consulta);

        let mut consulta = conexao.prepare(
            "SELECT e.estado, e.item, e.arquivo, e.linha_inicial, e.linha_final, e.a_revalidar \
             FROM rastro_evidencia e JOIN rastro_estado s ON s.id = e.estado \
             WHERE s.projeto = ?1 ORDER BY e.estado, e.item, e.ordem",
        )?;
        let linhas = consulta.query_map([projeto], |l| {
            Ok((
                l.get::<_, i64>(0)?,
                l.get::<_, String>(1)?,
                l.get::<_, String>(2)?,
                l.get::<_, u32>(3)?,
                l.get::<_, u32>(4)?,
                l.get::<_, bool>(5)?,
            ))
        })?;
        for linha in linhas {
            let (estado, item, arquivo, inicio, fim, a_revalidar) = linha?;
            if let Some(&i) = indice_do_item.get(&(estado, item)) {
                m.itens[i].evidencias.push(Ev {
                    arquivo,
                    inicio,
                    fim,
                    a_revalidar,
                });
            }
        }
        drop(consulta);

        let demandas: Vec<(String, Option<String>)> = m
            .analises
            .keys()
            .map(|a| (a.clone(), m.demanda_de(a)))
            .collect();
        for (a, d) in demandas {
            if let Some(x) = m.analises.get_mut(&a) {
                x.vista.demanda = d;
            }
        }
        Ok(m)
    }

    /// Demanda da análise: a própria ou a da cadeia de origem.
    fn demanda_de(&self, analise: &str) -> Option<String> {
        let mut atual = analise.to_owned();
        let mut vistas = BTreeSet::new();
        while vistas.insert(atual.clone()) {
            let a = self.analises.get(&atual)?;
            if let Some(d) = &a.demanda_propria {
                return Some(d.clone());
            }
            atual = a.vista.origem.clone()?;
        }
        None
    }

    fn item(&self, chave: &str) -> Option<&ItemM> {
        self.itens.iter().find(|i| i.chave == chave)
    }

    fn itens_do_tipo(&self, tipo: TipoItem) -> impl Iterator<Item = &ItemM> {
        self.itens
            .iter()
            .filter(move |i| i.tipo == tipo && i.valido())
    }

    /// Regras ligadas a uma afirmação: as do texto e as com evidência no mesmo
    /// trecho.
    fn regras_da_afirmacao(&self, afirmacao: &ItemM) -> BTreeSet<String> {
        let mut saida: BTreeSet<String> = afirmacao.cita("regra").map(str::to_owned).collect();
        for r in self.regras.values() {
            if r.evidencias
                .iter()
                .any(|e| afirmacao.evidencias.iter().any(|a| a.cruza(e)))
            {
                saida.insert(r.vista.id.clone());
            }
        }
        saida
    }

    fn regra_vista(&self, id: &str) -> RegraRastro {
        self.regras
            .get(id)
            .map(|r| r.vista.clone())
            .unwrap_or_else(|| RegraRastro {
                id: id.to_owned(),
                titulo: String::new(),
                status: None,
                documento: None,
            })
    }

    fn regra_a_revalidar(&self, id: &str) -> bool {
        self.regras
            .get(id)
            .is_some_and(|r| r.vista.status.as_deref() == Some(A_REVALIDAR))
    }

    /// O critério ou cenário depende de algo a revalidar: a marca dele, a
    /// afirmação citada ou a regra citada.
    fn depende_de_revalidar(&self, item: &ItemM) -> bool {
        item.a_revalidar
            || item
                .cita("afirmacao")
                .any(|a| self.item(a).is_some_and(|x| x.a_revalidar))
            || item.cita("regra").any(|r| self.regra_a_revalidar(r))
    }

    fn criterio_vista(&self, c: &ItemM) -> CriterioRastro {
        let historia = c.historia.clone().unwrap_or_default();
        let criterio = c
            .chave
            .strip_prefix(&format!("{historia}/"))
            .unwrap_or(&c.chave)
            .to_owned();
        CriterioRastro {
            historia,
            criterio,
            texto: c.titulo.clone(),
            analise: c.analise.clone(),
            situacao: c.situacao.clone(),
            a_revalidar: self.depende_de_revalidar(c),
            a_confirmar: c.a_confirmar,
        }
    }

    fn cenario_vista(&self, c: &ItemM) -> CenarioRastro {
        CenarioRastro {
            id: c.chave.clone(),
            titulo: c.titulo.clone(),
            analise: c.analise.clone(),
            historia: c.historia.clone(),
            situacao: c.situacao.clone(),
            a_revalidar: self.depende_de_revalidar(c),
            a_confirmar: c.a_confirmar,
        }
    }

    fn analise_aprovada(&self, analise: &str) -> bool {
        self.analises
            .get(analise)
            .is_some_and(|a| a.vista.estado.as_deref() == Some("aprovada"))
    }

    /// Ordena análises: as em andamento antes das aprovadas, depois pelo id.
    fn analises_vistas(&self, ids: &BTreeSet<String>) -> Vec<AnaliseRastro> {
        let mut saida: Vec<AnaliseRastro> = ids
            .iter()
            .filter_map(|id| self.analises.get(id).map(|a| a.vista.clone()))
            .collect();
        saida.sort_by_key(|a| (a.estado.as_deref() == Some("aprovada"), a.id.clone()));
        saida
    }

    // -----------------------------------------------------------------------
    // Matriz

    fn matriz(&self, demanda: IdDemanda) -> Matriz {
        let d = demanda.to_string();
        let mut analises: BTreeSet<String> = self
            .analises
            .values()
            .filter(|a| a.vista.demanda.as_deref() == Some(d.as_str()))
            .map(|a| a.vista.id.clone())
            .collect();
        let historias: Vec<&ItemM> = self
            .itens_do_tipo(TipoItem::Historia)
            .filter(|h| analises.contains(&h.analise))
            .collect();
        let ids_historias: BTreeSet<&str> = historias.iter().map(|h| h.chave.as_str()).collect();
        // Cenários de outras análises que verificam histórias da demanda.
        for c in self.itens_do_tipo(TipoItem::Cenario) {
            if c.historia
                .as_deref()
                .is_some_and(|h| ids_historias.contains(h))
            {
                analises.insert(c.analise.clone());
            }
        }

        let mut linhas_historia: Vec<LinhaHistoria> =
            historias.iter().map(|h| self.linha_historia(h)).collect();
        linhas_historia.sort_by(|a, b| a.id.cmp(&b.id));

        // Regras afetadas pela demanda, com as análises que as afetam.
        let mut afetadas: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for item in self.itens.iter().filter(|i| analises.contains(&i.analise)) {
            if !item.valido() {
                continue;
            }
            let regras: BTreeSet<String> = match item.tipo {
                TipoItem::Afirmacao => self.regras_da_afirmacao(item),
                TipoItem::Criterio | TipoItem::Cenario => {
                    item.cita("regra").map(str::to_owned).collect()
                }
                TipoItem::Historia => BTreeSet::new(),
            };
            for r in regras {
                afetadas.entry(r).or_default().insert(item.analise.clone());
            }
        }
        for r in self.regras.values() {
            if let Some(o) = &r.origem
                && analises.contains(o)
            {
                afetadas
                    .entry(r.vista.id.clone())
                    .or_default()
                    .insert(o.clone());
            }
        }
        let cenarios_da_demanda: Vec<&ItemM> = self
            .itens_do_tipo(TipoItem::Cenario)
            .filter(|c| analises.contains(&c.analise))
            .collect();
        let linhas_regra: Vec<LinhaRegra> = afetadas
            .into_iter()
            .map(|(id, analises_da_regra)| {
                let historias: Vec<String> = linhas_historia
                    .iter()
                    .filter(|h| h.regras.iter().any(|r| r.id == id))
                    .map(|h| h.id.clone())
                    .collect();
                let cenarios = cenarios_da_demanda
                    .iter()
                    .filter(|c| {
                        c.cita("regra").any(|r| r == id)
                            || c.historia.as_ref().is_some_and(|h| historias.contains(h))
                    })
                    .map(|c| self.cenario_vista(c))
                    .collect();
                let evidencias = self
                    .regras
                    .get(&id)
                    .map(|r| {
                        r.evidencias
                            .iter()
                            .map(|e| evidencia_vista(e, &id))
                            .collect()
                    })
                    .unwrap_or_default();
                let alertas = if historias.is_empty() {
                    vec![Alerta::RegraSemHistoria]
                } else {
                    Vec::new()
                };
                LinhaRegra {
                    regra: self.regra_vista(&id),
                    analises: analises_da_regra.into_iter().collect(),
                    historias,
                    evidencias,
                    cenarios,
                    alertas,
                }
            })
            .collect();

        let lacunas = lacunas(&linhas_historia, &linhas_regra);
        Matriz {
            projeto: self.projeto.clone(),
            demanda: self.demandas.get(&d).cloned().unwrap_or(DemandaRastro {
                id: d.clone(),
                titulo: String::new(),
                status: None,
                documento: None,
            }),
            analises: self.analises_vistas(&analises),
            historias: linhas_historia,
            regras: linhas_regra,
            lacunas,
        }
    }

    fn linha_historia(&self, h: &ItemM) -> LinhaHistoria {
        let criterios: Vec<&ItemM> = self
            .itens_do_tipo(TipoItem::Criterio)
            .filter(|c| c.analise == h.analise && c.historia.as_deref() == Some(&h.chave))
            .collect();
        let mut analises: BTreeSet<String> = BTreeSet::from([h.analise.clone()]);
        if let Some(o) = self
            .analises
            .get(&h.analise)
            .and_then(|a| a.vista.origem.clone())
        {
            analises.insert(o);
        }
        let mut afirmacoes: BTreeSet<String> = BTreeSet::new();
        let mut regras: BTreeSet<String> = BTreeSet::new();
        let mut evidencias: Vec<EvidenciaRastro> = Vec::new();
        let mut revalidar = false;
        fn juntar(e: EvidenciaRastro, lista: &mut Vec<EvidenciaRastro>) {
            if let Some(x) = lista.iter_mut().find(|x| {
                x.arquivo == e.arquivo
                    && x.linha_inicial == e.linha_inicial
                    && x.linha_final == e.linha_final
            }) {
                x.a_revalidar |= e.a_revalidar;
            } else {
                lista.push(e);
            }
        }
        for c in &criterios {
            revalidar |= c.a_revalidar;
            let mut com_origem = false;
            for a in c.cita("afirmacao") {
                afirmacoes.insert(a.to_owned());
                if let Some((analise, _)) = a.split_once('/') {
                    analises.insert(analise.to_owned());
                }
                if let Some(x) = self.item(a) {
                    com_origem = true;
                    revalidar |= x.a_revalidar;
                    regras.extend(self.regras_da_afirmacao(x));
                    for e in &x.evidencias {
                        juntar(
                            EvidenciaRastro {
                                a_revalidar: e.a_revalidar
                                    || x.situacao.as_deref() == Some(A_REVALIDAR),
                                ..evidencia_vista(e, a)
                            },
                            &mut evidencias,
                        );
                    }
                }
            }
            for r in c.cita("regra") {
                regras.insert(r.to_owned());
                if let Some(x) = self.regras.get(r) {
                    com_origem = true;
                    revalidar |= self.regra_a_revalidar(r);
                    for e in &x.evidencias {
                        juntar(evidencia_vista(e, r), &mut evidencias);
                    }
                }
            }
            // Sem a origem no índice, valem as cópias guardadas no critério.
            if !com_origem {
                for e in &c.evidencias {
                    juntar(evidencia_vista(e, &c.chave), &mut evidencias);
                }
            }
        }
        let cenarios: Vec<CenarioRastro> = self
            .itens_do_tipo(TipoItem::Cenario)
            .filter(|c| c.historia.as_deref() == Some(&h.chave))
            .map(|c| self.cenario_vista(c))
            .collect();
        revalidar |= cenarios.iter().any(|c| c.a_revalidar);
        let aprovada = h.situacao.as_deref() == Some("aceita") || self.analise_aprovada(&h.analise);
        let mut alertas = Vec::new();
        if aprovada && (revalidar || evidencias.iter().any(|e| e.a_revalidar)) {
            alertas.push(Alerta::EvidenciaARevalidar);
        }
        if cenarios.is_empty() {
            alertas.push(Alerta::HistoriaSemCenario);
        }
        if regras.is_empty() && evidencias.is_empty() {
            alertas.push(Alerta::HistoriaSemRegra);
        }
        LinhaHistoria {
            id: h.chave.clone(),
            titulo: h.titulo.clone(),
            analise: h.analise.clone(),
            situacao: h.situacao.clone(),
            aprovada,
            a_confirmar: criterios.iter().any(|c| c.a_confirmar),
            analises: analises.into_iter().collect(),
            criterios: criterios.iter().map(|c| self.criterio_vista(c)).collect(),
            afirmacoes: afirmacoes.into_iter().collect(),
            regras: regras.iter().map(|r| self.regra_vista(r)).collect(),
            evidencias,
            cenarios,
            alertas,
        }
    }

    // -----------------------------------------------------------------------
    // Consulta reversa

    fn dependentes(&self, alvo: &AlvoReverso) -> Dependentes {
        let mut regras_semente: BTreeSet<String> = BTreeSet::new();
        let mut afirmacoes: BTreeSet<String> = BTreeSet::new();
        let mut diretos: BTreeSet<String> = BTreeSet::new();
        let mut regra = None;
        let mut arquivos = Vec::new();
        match alvo {
            AlvoReverso::Regra(id) => {
                regras_semente.insert(id.clone());
                regra = Some(self.regra_vista(id));
                if let Some(r) = self.regras.get(id) {
                    for e in &r.evidencias {
                        if !arquivos.contains(&e.arquivo) {
                            arquivos.push(e.arquivo.clone());
                        }
                    }
                }
            }
            AlvoReverso::Arquivo(arquivo) => {
                for r in self.regras.values() {
                    if r.evidencias.iter().any(|e| &e.arquivo == arquivo) {
                        regras_semente.insert(r.vista.id.clone());
                    }
                }
                for i in self.itens.iter().filter(|i| i.valido()) {
                    if i.evidencias.iter().any(|e| &e.arquivo == arquivo) {
                        match i.tipo {
                            TipoItem::Afirmacao => {
                                afirmacoes.insert(i.chave.clone());
                            }
                            TipoItem::Criterio | TipoItem::Cenario => {
                                diretos.insert(i.chave.clone());
                            }
                            TipoItem::Historia => {}
                        }
                    }
                }
            }
        }
        for a in self.itens_do_tipo(TipoItem::Afirmacao) {
            if self
                .regras_da_afirmacao(a)
                .iter()
                .any(|r| regras_semente.contains(r))
            {
                afirmacoes.insert(a.chave.clone());
            }
        }
        for i in self.itens.iter().filter(|i| i.valido()) {
            if matches!(i.tipo, TipoItem::Criterio | TipoItem::Cenario)
                && (i.cita("regra").any(|r| regras_semente.contains(r))
                    || i.cita("afirmacao").any(|a| afirmacoes.contains(a)))
            {
                diretos.insert(i.chave.clone());
            }
        }
        let criterios: Vec<&ItemM> = self
            .itens_do_tipo(TipoItem::Criterio)
            .filter(|c| diretos.contains(&c.chave))
            .collect();
        let historias: BTreeSet<&str> = criterios
            .iter()
            .filter_map(|c| c.historia.as_deref())
            .collect();
        let cenarios: Vec<&ItemM> = self
            .itens_do_tipo(TipoItem::Cenario)
            .filter(|c| {
                diretos.contains(&c.chave)
                    || c.historia.as_deref().is_some_and(|h| historias.contains(h))
            })
            .collect();

        let mut analises: BTreeSet<String> = BTreeSet::new();
        for a in &afirmacoes {
            if let Some((analise, _)) = a.split_once('/') {
                analises.insert(analise.to_owned());
            }
        }
        for i in criterios.iter().chain(cenarios.iter()) {
            analises.insert(i.analise.clone());
        }
        for r in &regras_semente {
            if let Some(o) = self.regras.get(r).and_then(|r| r.origem.clone()) {
                analises.insert(o);
            }
        }
        analises.retain(|a| self.analises.contains_key(a));
        let mut demandas: Vec<DemandaRastro> = analises
            .iter()
            .filter_map(|a| self.demanda_de(a))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|d| {
                self.demandas.get(&d).cloned().unwrap_or(DemandaRastro {
                    id: d,
                    titulo: String::new(),
                    status: None,
                    documento: None,
                })
            })
            .collect();
        demandas.sort_by_key(|d| (d.status.as_deref() == Some("encerrada"), d.id.clone()));

        let mut criterios: Vec<CriterioRastro> =
            criterios.iter().map(|c| self.criterio_vista(c)).collect();
        let mut cenarios: Vec<CenarioRastro> =
            cenarios.iter().map(|c| self.cenario_vista(c)).collect();
        // A regra consultada está a revalidar: o que depende dela também.
        if let AlvoReverso::Regra(id) = alvo
            && self.regra_a_revalidar(id)
        {
            criterios.iter_mut().for_each(|c| c.a_revalidar = true);
            cenarios
                .iter_mut()
                .filter(|c| {
                    self.item(&c.id)
                        .is_some_and(|i| i.cita("regra").any(|r| r == id))
                })
                .for_each(|c| c.a_revalidar = true);
        }
        criterios.sort_by(|a, b| (&a.historia, &a.criterio).cmp(&(&b.historia, &b.criterio)));
        cenarios.sort_by(|a, b| a.id.cmp(&b.id));

        Dependentes {
            projeto: self.projeto.clone(),
            alvo: alvo.clone(),
            regra,
            arquivos,
            regras: match alvo {
                AlvoReverso::Arquivo(_) => {
                    regras_semente.iter().map(|r| self.regra_vista(r)).collect()
                }
                AlvoReverso::Regra(_) => Vec::new(),
            },
            demandas,
            analises: self.analises_vistas(&analises),
            criterios,
            cenarios,
        }
    }
}

fn evidencia_vista(e: &Ev, de: &str) -> EvidenciaRastro {
    EvidenciaRastro {
        arquivo: e.arquivo.clone(),
        linha_inicial: e.inicio,
        linha_final: e.fim,
        a_revalidar: e.a_revalidar,
        de: de.to_owned(),
    }
}

fn lacunas(historias: &[LinhaHistoria], regras: &[LinhaRegra]) -> Vec<Lacuna> {
    let mut saida = Vec::new();
    for h in historias {
        if h.alertas.contains(&Alerta::EvidenciaARevalidar) {
            let mut relacionados: Vec<String> = h
                .evidencias
                .iter()
                .filter(|e| e.a_revalidar)
                .map(|e| e.de.clone())
                .collect();
            relacionados.extend(
                h.criterios
                    .iter()
                    .filter(|c| c.a_revalidar)
                    .map(|c| format!("{}/{}", c.historia, c.criterio)),
            );
            relacionados.extend(
                h.cenarios
                    .iter()
                    .filter(|c| c.a_revalidar)
                    .map(|c| c.id.clone()),
            );
            relacionados.dedup();
            let trechos: Vec<String> = h
                .evidencias
                .iter()
                .filter(|e| e.a_revalidar)
                .map(|e| format!("{}:{}-{}", e.arquivo, e.linha_inicial, e.linha_final))
                .collect();
            let onde = if trechos.is_empty() {
                String::new()
            } else {
                format!(" ({})", trechos.join(", "))
            };
            saida.push(Lacuna {
                alerta: Alerta::EvidenciaARevalidar,
                alvo: h.id.clone(),
                descricao: format!(
                    "{} está aprovada e depende de evidência a revalidar{onde}: o código \
                     mudou desde o registro. Confira {}.",
                    h.id,
                    relacionados.join(", ")
                ),
                relacionados,
            });
        }
    }
    for r in regras {
        if r.alertas.contains(&Alerta::RegraSemHistoria) {
            saida.push(Lacuna {
                alerta: Alerta::RegraSemHistoria,
                alvo: r.regra.id.clone(),
                descricao: format!(
                    "{}{} aparece em {} e não entrou em nenhuma história.",
                    r.regra.id,
                    if r.regra.titulo.is_empty() {
                        String::new()
                    } else {
                        format!(" ({})", r.regra.titulo)
                    },
                    r.analises.join(", ")
                ),
                relacionados: r.analises.clone(),
            });
        }
    }
    for h in historias {
        if h.alertas.contains(&Alerta::HistoriaSemCenario) {
            saida.push(Lacuna {
                alerta: Alerta::HistoriaSemCenario,
                alvo: h.id.clone(),
                descricao: format!("{} não tem cenário de teste.", h.id),
                relacionados: vec![h.analise.clone()],
            });
        }
    }
    for h in historias {
        if h.alertas.contains(&Alerta::HistoriaSemRegra) {
            saida.push(Lacuna {
                alerta: Alerta::HistoriaSemRegra,
                alvo: h.id.clone(),
                descricao: format!(
                    "{} não se apoia em regra nem em afirmação com evidência{}.",
                    h.id,
                    if h.a_confirmar {
                        " (depende de pergunta em aberto)"
                    } else {
                        ""
                    }
                ),
                relacionados: h.afirmacoes.clone(),
            });
        }
    }
    saida
}

impl Indice {
    /// Matriz de rastreabilidade da demanda: análises, histórias (com critérios,
    /// afirmações, regras, evidências e cenários), regras afetadas e lacunas.
    /// Derivada do índice; uma demanda sem nada ligado dá a matriz vazia.
    pub fn matriz(&self, projeto: &str, demanda: IdDemanda) -> ResultadoIndice<Matriz> {
        Ok(Modelo::carregar(&self.conexao, projeto)?.matriz(demanda))
    }

    /// Consulta reversa: o que depende da regra ou do arquivo (demandas, análises,
    /// critérios de aceite e cenários de teste).
    pub fn dependentes(&self, projeto: &str, alvo: &AlvoReverso) -> ResultadoIndice<Dependentes> {
        Ok(Modelo::carregar(&self.conexao, projeto)?.dependentes(alvo))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regras_citadas_no_texto() {
        assert_eq!(
            regras_no_texto("Ver RN-FAT-007 e RN-FAT-012; xRN-FAT-001 não, RN-FAT-007 de novo."),
            ["RN-FAT-007", "RN-FAT-012"]
        );
        assert!(regras_no_texto("RN- solto, RN-FAT-7").is_empty());
    }

    #[test]
    fn linhas_em_texto_ou_numero() {
        assert_eq!(linhas(&serde_json::json!("10-12")), Some((10, 12)));
        assert_eq!(linhas(&serde_json::json!(7)), Some((7, 7)));
        assert_eq!(linhas(&serde_json::json!("12-10")), None);
        assert_eq!(linhas(&serde_json::Value::Null), None);
    }

    #[test]
    fn estado_com_marca_direta_da_169() {
        let lido: EstadoLido = serde_json::from_value(serde_json::json!({
            "id": "A-0002",
            "fluxo": "historias",
            "historias": [{
                "id": "HU-0001",
                "titulo": "Parcelar",
                "situacao": "aceita",
                "criterios": [
                    { "numero": 1, "entao": "ok", "estado": "a-revalidar",
                      "fundamentos": [{ "tipo": "regra", "id": "RN-FAT-008" }] },
                    { "numero": 2, "entao": "ok",
                      "fundamentos": [{ "tipo": "afirmacao", "ref": "A-0001/1",
                        "evidencias": [{ "arquivo": "a.sql", "linhas": "1-2",
                                         "estado": "a-revalidar" }] }] },
                    { "numero": 3, "entao": "ok", "pergunta": "A-0001/2",
                      "fundamentos": [{ "tipo": "regra", "id": "RN-FAT-009" }] }
                ]
            }],
            "campo_desconhecido": true
        }))
        .unwrap();
        let e = extrair(lido);
        let criterios: Vec<&ItemExtraido> = e
            .itens
            .iter()
            .filter(|i| i.tipo == TipoItem::Criterio)
            .collect();
        assert_eq!(criterios.len(), 3);
        assert!(criterios[0].a_revalidar);
        assert!(criterios[1].a_revalidar);
        assert!(!criterios[2].a_revalidar);
        assert!(criterios[2].a_confirmar);
        assert_eq!(criterios[1].chave, "HU-0001/CA-2");
        assert_eq!(criterios[1].cita, [("A-0001/1".to_owned(), "afirmacao")]);
    }

    #[test]
    fn demanda_e_analise_pelo_caminho() {
        assert_eq!(
            demanda_do_caminho("fat/demandas/D-0012-parcelamento/demanda.md").as_deref(),
            Some("D-0012")
        );
        assert_eq!(demanda_do_caminho("fat/regras/RN-FAT-001.md"), None);
        assert_eq!(
            analise_do_caminho("fat/analises/A-0193.md").as_deref(),
            Some("A-0193")
        );
        assert_eq!(analise_do_caminho("fat/regras/RN-FAT-001.md"), None);
    }
}
