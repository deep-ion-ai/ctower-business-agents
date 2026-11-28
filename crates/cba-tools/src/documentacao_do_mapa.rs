//! Documentação por funcionalidade no mapa (#462): a tabela dos documentos vivos
//! (uma linha por funcionalidade, uma coluna por tipo vivo habilitado), a
//! cobertura por tipo e a geração da versão inicial (v1.0) em lote.
//!
//! - **Tabela** ([`tabela`]): as colunas são os tipos habilitados no projeto que
//!   pertencem à funcionalidade ([`entregaveis::vivos_da_funcionalidade`]); cada
//!   célula é o [`DocumentoDoTipo`] de [`documentos_vivos`], com o estado (em dia,
//!   proposta pendente, desatualizado, sem documento) e a ação. A cobertura por
//!   tipo ([`crate::cobertura::cobertura_por_tipo`]) é contada sobre as mesmas
//!   células, então bate com a tabela.
//! - **Origem da v1.0**: a funcionalidade do mapa (descrição, atores, pontos de
//!   entrada, dados lidos e gravados) mais as regras da base ligadas a ela (pelo
//!   vínculo da regra ou por evidência num arquivo dos pontos), em [`Contexto`].
//!   O histórico registra [`MUDANCAS_DA_VERSAO_INICIAL`].
//! - **Geração em lote** ([`gerar_em_lote`]): os tipos montados pelo app
//!   ([`Geracao::App`]) saem na hora, sem IA ([`montar`]); os redigidos pelo
//!   agente devolvem o pedido de uma sessão por funcionalidade
//!   ([`pedido_de_versao_inicial`]), que a tela roda em sequência. Uma
//!   funcionalidade que já tem o documento fica de fora: um documento vivo nunca
//!   é recriado.
//! - **Ferramentas** do agente: `iniciar_versao_inicial` (só leitura: o roteiro,
//!   as seções do tipo e o contexto) e `registrar_versao_inicial` (registra a
//!   v1.0).
//!
//! Todo documento gerado entra como **proposta** (rascunho) da versão 1.0;
//! publicar exige a revisão final ([`documentos_vivos::publicar`]).

use std::collections::BTreeSet;
use std::fmt::Write as _;

use cba_core::ambiente::Ambiente;
use cba_core::documentos_vivos::{
    DocumentosVivos, Dono, EstadoDocumento, NovaProposta, sem_historico,
};
use cba_core::funcionalidades::{Funcionalidade, MapaFuncionalidades, SituacaoFuncionalidade};
use cba_docs::entregaveis::{self, Geracao, TipoCampo, TipoEntregavel};

use cba_index::Metodologia;
use cba_kb::frontmatter::{FrontmatterRegra, StatusRegra};
use cba_kb::{CaminhoRelativo, IdFuncionalidade, TipoDocumento, layout};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::cobertura::{CoberturaDoTipo, cobertura_por_tipo};
use crate::documentos_vivos::{
    self, DocumentoDoTipo, PedidoDeProposta, PropostaRegistrada, documentos_da_funcionalidade,
};
use crate::registro::{ErroFerramenta, Ferramenta, Saida, ler_argumentos};

/// O que o histórico de revisões registra na versão inicial.
pub const MUDANCAS_DA_VERSAO_INICIAL: &str = "Versão inicial, gerada por engenharia reversa";

/// Roteiro devolvido por `iniciar_versao_inicial`.
const ROTEIRO: &str = include_str!("../textos/roteiros/versao-inicial.md");

fn interno(e: impl std::fmt::Display) -> ErroFerramenta {
    ErroFerramenta::Interno(e.to_string())
}

// ---------------------------------------------------------------------------
// Tabela

/// Uma coluna: um tipo vivo habilitado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColunaDeDocumentacao {
    pub tipo: &'static str,
    pub nome: &'static str,
    /// Montado pelo app na hora ou redigido pelo agente numa sessão.
    pub geracao: Geracao,
}

impl From<&'static TipoEntregavel> for ColunaDeDocumentacao {
    fn from(t: &'static TipoEntregavel) -> Self {
        Self {
            tipo: t.id,
            nome: t.nome,
            geracao: t.geracao,
        }
    }
}

/// Uma linha: uma funcionalidade aberta do mapa, com uma célula por coluna.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinhaDeDocumentacao {
    pub funcionalidade: String,
    pub nome: String,
    pub area: String,
    pub area_nome: String,
    pub situacao: SituacaoFuncionalidade,
    /// Na ordem das colunas.
    pub documentos: Vec<DocumentoDoTipo>,
}

/// A visão "Documentação" do mapa.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TabelaDeDocumentacao {
    pub metodologia: Metodologia,
    pub colunas: Vec<ColunaDeDocumentacao>,
    pub linhas: Vec<LinhaDeDocumentacao>,
    /// Uma por coluna, contada sobre as células.
    pub cobertura: Vec<CoberturaDoTipo>,
}

/// Monta a tabela com o estado já lido. Sem IA: o estado "desatualizado" é a
/// comparação dos hashes dos arquivos citados.
pub fn montar_tabela(
    ambiente: &Ambiente,
    mapa: &MapaFuncionalidades,
    documentos: &DocumentosVivos,
) -> TabelaDeDocumentacao {
    let config = entregaveis::efetiva(ambiente.projeto.entregaveis.as_ref());
    let tipos = entregaveis::vivos_da_funcionalidade(&config);
    let ids: Vec<&str> = tipos.iter().map(|t| t.id).collect();
    let colunas: Vec<ColunaDeDocumentacao> = tipos.into_iter().map(Into::into).collect();
    let linhas: Vec<LinhaDeDocumentacao> = mapa
        .ordenadas()
        .into_iter()
        .map(|f| LinhaDeDocumentacao {
            funcionalidade: f.id.to_string(),
            nome: f.nome.clone(),
            area: f.area.clone(),
            area_nome: f.area_nome.clone(),
            situacao: f.situacao,
            documentos: documentos_da_funcionalidade(&ambiente.escopo, documentos, f, &ids),
        })
        .collect();
    let cobertura = cobertura_por_tipo(&colunas, &linhas);
    TabelaDeDocumentacao {
        metodologia: config.metodologia,
        colunas,
        linhas,
        cobertura,
    }
}

/// A tabela do projeto.
pub fn tabela(ambiente: &Ambiente) -> Result<TabelaDeDocumentacao, ErroFerramenta> {
    let mapa = MapaFuncionalidades::carregar(&ambiente.kb).map_err(interno)?;
    let documentos = DocumentosVivos::carregar(&ambiente.kb)?;
    Ok(montar_tabela(ambiente, &mapa, &documentos))
}

// ---------------------------------------------------------------------------
// Contexto da versão inicial

/// Regra da base ligada à funcionalidade.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegraDaFuncionalidade {
    pub id: String,
    pub titulo: String,
    pub enunciado: String,
    pub status: StatusRegra,
    /// Arquivos das evidências.
    pub arquivos: Vec<String>,
}

/// De onde a versão inicial sai: o tipo, a funcionalidade e as regras.
#[derive(Debug, Clone)]
pub struct Contexto {
    pub tipo: &'static TipoEntregavel,
    pub funcionalidade: Funcionalidade,
    /// Em ordem de identificador; sem as obsoletas.
    pub regras: Vec<RegraDaFuncionalidade>,
}

/// O texto da seção "Enunciado" do corpo de uma regra, sem as chamadas de nota.
fn enunciado(corpo: &str) -> Option<String> {
    let mut dentro = false;
    let mut linhas: Vec<&str> = Vec::new();
    for l in corpo.lines() {
        let t = l.trim_start();
        if let Some(titulo) = t.strip_prefix("## ") {
            if dentro {
                break;
            }
            dentro = titulo.trim().eq_ignore_ascii_case("enunciado");
            continue;
        }
        if dentro && !t.starts_with("[^") {
            linhas.push(l);
        }
    }
    let mut texto = linhas.join("\n");
    while let Some(i) = texto.find("[^") {
        match texto[i..].find(']') {
            Some(j) => texto.replace_range(i..i + j + 1, ""),
            None => break,
        }
    }
    let texto = texto.split_whitespace().collect::<Vec<_>>().join(" ");
    (!texto.is_empty()).then_some(texto)
}

/// As regras da base, com o enunciado.
fn regras_da_base(ambiente: &Ambiente) -> Vec<(FrontmatterRegra, String)> {
    let mut regras = Vec::new();
    for rel in ambiente.kb.documentos().unwrap_or_default() {
        if rel.partes().next() != Some(layout::REGRAS) || !rel.as_str().ends_with(".md") {
            continue;
        }
        let Ok(doc) = ambiente.kb.ler::<FrontmatterRegra>(&rel) else {
            continue;
        };
        let texto = enunciado(&doc.corpo).unwrap_or_else(|| doc.frontmatter.titulo.clone());
        regras.push((doc.frontmatter, texto));
    }
    regras
}

/// As regras ligadas à funcionalidade: pelo vínculo da regra (#287) ou por uma
/// evidência num arquivo dos pontos de entrada. As obsoletas ficam de fora.
fn regras_da_funcionalidade(
    regras: Vec<(FrontmatterRegra, String)>,
    f: &Funcionalidade,
) -> Vec<RegraDaFuncionalidade> {
    let arquivos: BTreeSet<&str> = f
        .pontos
        .iter()
        .map(|p| p.evidencia.arquivo.as_str())
        .collect();
    let mut saida: Vec<RegraDaFuncionalidade> = regras
        .into_iter()
        .filter(|(r, _)| r.status != StatusRegra::Obsoleta)
        .filter(|(r, _)| {
            r.funcionalidades.contains(&f.id)
                || r.evidencias
                    .iter()
                    .any(|e| arquivos.contains(e.arquivo.as_str()))
        })
        .map(|(r, enunciado)| {
            let mut arquivos: Vec<String> =
                r.evidencias.iter().map(|e| e.arquivo.to_string()).collect();
            arquivos.sort();
            arquivos.dedup();
            RegraDaFuncionalidade {
                id: r.id.to_string(),
                titulo: r.titulo,
                enunciado,
                status: r.status,
                arquivos,
            }
        })
        .collect();
    saida.sort_by(|a, b| a.id.cmp(&b.id));
    saida
}

/// O tipo do catálogo, se for um documento vivo de funcionalidade.
fn tipo_da_funcionalidade(tipo: &str) -> Result<&'static TipoEntregavel, ErroFerramenta> {
    let t = entregaveis::tipo(tipo).ok_or_else(|| {
        ErroFerramenta::Argumentos(format!(
            "o tipo `{tipo}` não está no catálogo de entregáveis"
        ))
    })?;
    if !t.da_funcionalidade() {
        return Err(ErroFerramenta::Recusa(format!(
            "{} não é um documento vivo de funcionalidade: não tem versão inicial pelo mapa",
            t.nome
        )));
    }
    Ok(t)
}

fn id_da_funcionalidade(texto: &str) -> Result<IdFuncionalidade, ErroFerramenta> {
    IdFuncionalidade::analisar(texto.trim()).map_err(|_| {
        ErroFerramenta::Argumentos(format!(
            "`{texto}` não é uma funcionalidade do mapa (F-<SIGLA>-NNN)"
        ))
    })
}

fn aberta(
    mapa: &MapaFuncionalidades,
    id: &IdFuncionalidade,
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

/// O contexto da versão inicial do tipo na funcionalidade.
pub fn contexto(
    ambiente: &Ambiente,
    tipo: &str,
    funcionalidade: &IdFuncionalidade,
) -> Result<Contexto, ErroFerramenta> {
    let tipo = tipo_da_funcionalidade(tipo)?;
    let mapa = MapaFuncionalidades::carregar(&ambiente.kb).map_err(interno)?;
    let f = aberta(&mapa, funcionalidade)?;
    let regras = regras_da_funcionalidade(regras_da_base(ambiente), &f);
    Ok(Contexto {
        tipo,
        funcionalidade: f,
        regras,
    })
}

impl Contexto {
    /// Os arquivos em que a versão se apoia: os dos pontos de entrada e os das
    /// evidências das regras. Uma mudança num deles marca o documento como
    /// desatualizado.
    pub fn arquivos(&self) -> Vec<String> {
        let mut arquivos: BTreeSet<String> = self
            .funcionalidade
            .pontos
            .iter()
            .map(|p| p.evidencia.arquivo.to_string())
            .collect();
        for r in &self.regras {
            arquivos.extend(r.arquivos.iter().cloned());
        }
        arquivos.into_iter().collect()
    }
}

/// As seções do tipo, na ordem do template embutido; sem ele, os campos do
/// esquema. A funcionalidade, o título e as seções que o app monta (evidências,
/// perguntas, sugestões) ficam de fora.
pub fn secoes(tipo: &TipoEntregavel) -> Vec<&'static str> {
    const DO_APP: [&str; 4] = ["evidencias", "perguntas", "sugestoes", "identificacao"];
    if let Some(m) = TipoDocumento::de_str(tipo.template).and_then(cba_docs::modelo) {
        return m
            .secoes
            .iter()
            .filter(|s| !DO_APP.contains(&s.marcador))
            .map(|s| s.titulo)
            .collect();
    }
    tipo.campos
        .iter()
        .filter(|c| !matches!(c.nome, "funcionalidade" | "titulo"))
        .map(|c| c.rotulo)
        .collect()
}

fn lista(itens: &[String], vazio: &str) -> String {
    if itens.is_empty() {
        return format!("{vazio}\n");
    }
    itens.iter().map(|i| format!("- {i}\n")).collect()
}

/// O contexto em Markdown: o que o mapa registra e as regras, com os títulos no
/// nível `titulo` (`##`, `###`).
fn contexto_em_markdown(c: &Contexto, titulo: &str) -> String {
    let f = &c.funcionalidade;
    let mut t = String::new();
    let _ = writeln!(t, "{titulo} Funcionalidade\n");
    let _ = writeln!(t, "- **Identificador:** {}", f.id);
    let _ = writeln!(t, "- **Nome:** {}", f.nome);
    let _ = writeln!(t, "- **Área:** {}", f.area_nome);
    let descricao = f.descricao.trim();
    if !descricao.is_empty() {
        let _ = writeln!(t, "\n{descricao}");
    }
    let _ = writeln!(t, "\n{titulo} Atores\n");
    t.push_str(&lista(&f.atores, "Nenhum ator registrado no mapa."));
    let _ = writeln!(t, "\n{titulo} Pontos de entrada\n");
    let pontos: Vec<String> = f
        .pontos
        .iter()
        .map(|p| {
            let tipo = p
                .tipo
                .map(|t| format!(" ({})", t.nome()))
                .unwrap_or_default();
            format!(
                "`{}`{tipo}: `{}`, linhas {}-{}",
                p.nome, p.evidencia.arquivo, p.evidencia.linhas.inicio, p.evidencia.linhas.fim
            )
        })
        .collect();
    t.push_str(&lista(&pontos, "Nenhum ponto de entrada registrado."));
    let _ = writeln!(t, "\n{titulo} Dados lidos\n");
    t.push_str(&lista(&f.dados_lidos, "Nenhum dado lido registrado."));
    let _ = writeln!(t, "\n{titulo} Dados gravados\n");
    t.push_str(&lista(&f.dados_gravados, "Nenhum dado gravado registrado."));
    let _ = writeln!(t, "\n{titulo} Regras de negócio\n");
    let regras: Vec<String> = c
        .regras
        .iter()
        .map(|r| {
            let revalidar = if r.status == StatusRegra::ARevalidar {
                " _(a revalidar)_"
            } else {
                ""
            };
            format!("**{}**: {}{revalidar}", r.id, r.enunciado)
        })
        .collect();
    t.push_str(&lista(
        &regras,
        "Nenhuma regra da base ligada a esta funcionalidade.",
    ));
    t
}

/// A versão inicial montada pelo app, sem IA, para os tipos
/// [`Geracao::App`]: as seções do tipo preenchidas com o que o mapa e a base
/// sustentam. Determinística: o mesmo mapa e as mesmas regras dão o mesmo
/// texto. Os montadores próprios de cada tipo (catálogo de regras,
/// especificação suplementar, cenários `.feature`, #481) entram aqui.
pub fn montar(c: &Contexto) -> String {
    let f = &c.funcionalidade;
    let mut t = format!("# {}: {}\n\n", c.tipo.nome, f.nome);
    let _ = writeln!(
        t,
        "> {MUDANCAS_DA_VERSAO_INICIAL}, a partir do mapa de funcionalidades ({}) e das \
         regras da base.\n",
        f.id
    );
    t.push_str(&contexto_em_markdown(c, "##"));
    t
}

// ---------------------------------------------------------------------------
// Registro

/// Identificadores de regra (`RN-FAT-001`) citados no texto.
fn regras_citadas(texto: &str) -> BTreeSet<String> {
    entregaveis::regras_no_texto(texto).into_iter().collect()
}

/// O tipo registra a versão inicial pelos campos do esquema, e não por um texto
/// livre: o caso de uso, cujas etapas citam a regra de origem.
pub fn registro_pelo_esquema(tipo: &TipoEntregavel) -> bool {
    tipo.campos.iter().any(|c| c.tipo == TipoCampo::Etapas)
}

/// Registra a versão inicial a partir dos `campos` do esquema: confere o
/// esquema (cada etapa com a regra citada) e as regras citadas contra as da
/// funcionalidade, monta o texto sem IA ([`TipoEntregavel::markdown_do_item`]) e
/// registra como em [`registrar`].
pub fn registrar_pelos_campos(
    ambiente: &Ambiente,
    tipo: &str,
    funcionalidade: &IdFuncionalidade,
    campos: &serde_json::Value,
    arquivos: Vec<String>,
) -> Result<PropostaRegistrada, ErroFerramenta> {
    let c = contexto(ambiente, tipo, funcionalidade)?;
    if let Err(erros) = c.tipo.validar_campos(campos) {
        let lista: Vec<String> = erros.iter().map(ToString::to_string).collect();
        return Err(ErroFerramenta::Recusa(format!(
            "os campos não seguem o esquema de {}: {}. Confira o esquema devolvido por \
             iniciar_versao_inicial",
            c.tipo.nome,
            lista.join("; ")
        )));
    }
    let citadas = c.tipo.regras_citadas(campos);
    let de_fora: Vec<&String> = citadas
        .iter()
        .filter(|id| !c.regras.iter().any(|r| &r.id == *id))
        .collect();
    if !de_fora.is_empty() {
        let permitidas: Vec<&str> = c.regras.iter().map(|r| r.id.as_str()).collect();
        return Err(ErroFerramenta::Recusa(format!(
            "{} não {} da funcionalidade {funcionalidade}: cite só as regras do contexto ({}) \
             e registre de novo",
            de_fora
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", "),
            if de_fora.len() == 1 {
                "é regra"
            } else {
                "são regras"
            },
            if permitidas.is_empty() {
                "nenhuma".to_owned()
            } else {
                permitidas.join(", ")
            }
        )));
    }
    let titulos: Vec<(String, String)> = citadas
        .iter()
        .filter_map(|id| c.regras.iter().find(|r| &r.id == id))
        .map(|r| (r.id.clone(), r.titulo.clone()))
        .collect();
    let texto = c.tipo.markdown_do_item(campos, &titulos, 1);
    registrar(ambiente, c.tipo.id, funcionalidade, &texto, arquivos)
}

/// Registra a versão inicial (1.0) como proposta. Recusa se a funcionalidade
/// já tem o documento publicado ou vinculado (um documento vivo nunca é
/// recriado) ou se o texto cita uma regra que não existe na base. Uma 1.0 ainda
/// em rascunho é refeita e volta a rascunho.
pub fn registrar(
    ambiente: &Ambiente,
    tipo: &str,
    funcionalidade: &IdFuncionalidade,
    texto: &str,
    arquivos: Vec<String>,
) -> Result<PropostaRegistrada, ErroFerramenta> {
    let c = contexto(ambiente, tipo, funcionalidade)?;
    let tipo = c.tipo;
    let estado = DocumentosVivos::carregar(&ambiente.kb)?;
    let dono = Dono::Funcionalidade(funcionalidade.clone());
    let publicada = estado
        .do_dono(tipo.id, &dono)
        .and_then(|d| d.versao_publicada());
    if let Some(v) = publicada {
        return Err(ErroFerramenta::Recusa(format!(
            "{} de {funcionalidade} já está na versão {v}: um documento vivo nunca é recriado; \
             a versão seguinte vem de uma demanda (\"Propor atualização\")",
            tipo.nome
        )));
    }
    if estado.do_dono(tipo.id, &dono).is_none()
        && let Some(caminho) = c.funcionalidade.documento(tipo.id)
    {
        return Err(ErroFerramenta::Recusa(format!(
            "{funcionalidade} já tem o documento {} em `{caminho}`: um documento vivo nunca \
             é recriado; proponha a versão seguinte dele",
            tipo.nome
        )));
    }
    let existentes: BTreeSet<String> = regras_da_base(ambiente)
        .into_iter()
        .map(|(r, _)| r.id.to_string())
        .collect();
    let inexistentes: Vec<String> = regras_citadas(texto)
        .into_iter()
        .filter(|r| !existentes.contains(r))
        .collect();
    if !inexistentes.is_empty() {
        return Err(ErroFerramenta::Recusa(format!(
            "o texto cita regras que não existem na base: {}. Cite só as regras do contexto \
             (ou tire a citação) e registre de novo",
            inexistentes.join(", ")
        )));
    }
    let mut todos = c.arquivos();
    for a in arquivos {
        let a = a.trim().replace('\\', "/");
        if !a.is_empty() && !todos.contains(&a) {
            todos.push(a);
        }
    }
    documentos_vivos::propor(
        ambiente,
        PedidoDeProposta {
            tipo: tipo.id.to_owned(),
            dono,
            caminho: None,
            proposta: NovaProposta {
                texto: texto.to_owned(),
                mudancas: MUDANCAS_DA_VERSAO_INICIAL.to_owned(),
                demanda: None,
                analise: None,
                arquivos: todos,
            },
        },
    )
}

/// "Propor atualização" de um documento montado pelo app ([`Geracao::App`]):
/// monta de novo a partir do mapa e das regras de agora e registra a proposta
/// da versão seguinte, com os arquivos que mudaram no que mudou. Os documentos
/// redigidos pelo agente são atualizados a partir de uma demanda.
pub fn propor_atualizacao(
    ambiente: &Ambiente,
    tipo: &str,
    funcionalidade: &IdFuncionalidade,
) -> Result<PropostaRegistrada, ErroFerramenta> {
    let c = contexto(ambiente, tipo, funcionalidade)?;
    if c.tipo.geracao != Geracao::App {
        return Err(ErroFerramenta::Recusa(format!(
            "{} é redigido pelo agente: a atualização vem de uma demanda que muda a \
             funcionalidade",
            c.tipo.nome
        )));
    }
    let estado = DocumentosVivos::carregar(&ambiente.kb)?;
    let dono = Dono::Funcionalidade(funcionalidade.clone());
    let documento = estado.do_dono(c.tipo.id, &dono);
    let mudancas = match documentos_vivos::estado(&ambiente.escopo, documento) {
        EstadoDocumento::Desatualizado { arquivos, .. } => {
            format!(
                "Atualização a partir do código: mudou {}",
                arquivos.join(", ")
            )
        }
        EstadoDocumento::EmDia { .. } => "Atualização a partir do mapa e das regras".to_owned(),
        EstadoDocumento::PropostaPendente { versao, .. } => {
            return Err(ErroFerramenta::Recusa(format!(
                "{} de {funcionalidade} já tem a proposta {versao} pendente: revise-a",
                c.tipo.nome
            )));
        }
        EstadoDocumento::SemDocumento => {
            return Err(ErroFerramenta::Recusa(format!(
                "{funcionalidade} ainda não tem {}: gere a versão inicial",
                c.tipo.nome
            )));
        }
    };
    let pedido = PedidoDeProposta {
        tipo: c.tipo.id.to_owned(),
        dono,
        caminho: None,
        proposta: NovaProposta {
            texto: montar(&c),
            mudancas,
            demanda: None,
            analise: None,
            arquivos: c.arquivos(),
        },
    };
    documentos_vivos::propor(ambiente, pedido).map_err(|e| {
        if documento
            .and_then(|d| d.publicada())
            .is_some_and(|r| r.texto == sem_historico(&montar(&c)))
        {
            ErroFerramenta::Recusa(format!(
                "{} montado do mapa e das regras de agora é igual à versão publicada: a \
                 mudança no código ainda não chegou ao mapa nem às regras. Revalide as regras \
                 ou registre a funcionalidade de novo e proponha depois",
                c.tipo.nome
            ))
        } else {
            e
        }
    })
}

// ---------------------------------------------------------------------------
// Lote

/// Pedido digitado na sessão do Claude que redige a versão inicial de uma
/// funcionalidade.
pub fn pedido_de_versao_inicial(
    tipo: &TipoEntregavel,
    funcionalidade: &IdFuncionalidade,
) -> String {
    format!(
        "Gere a versão inicial do documento \"{}\" da funcionalidade {funcionalidade} pelo \
         ctower: chame a ferramenta iniciar_versao_inicial do cba com tipo \"{}\" e \
         funcionalidade \"{funcionalidade}\" e siga o roteiro devolvido.",
        tipo.nome, tipo.id
    )
}

/// O que aconteceu com uma funcionalidade do lote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "resultado", rename_all = "camelCase")]
pub enum ItemDoLote {
    /// Montada pelo app: a 1.0 ficou como rascunho.
    #[serde(rename_all = "camelCase")]
    Gerada {
        funcionalidade: String,
        caminho: CaminhoRelativo,
    },
    /// Redigida pelo agente: o pedido da sessão desta funcionalidade.
    #[serde(rename_all = "camelCase")]
    Sessao {
        funcionalidade: String,
        pedido: String,
    },
    /// Ficou de fora (já tem o documento, saiu do mapa, falhou).
    #[serde(rename_all = "camelCase")]
    Pulada {
        funcionalidade: String,
        motivo: String,
    },
}

/// Resultado da geração em lote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Lote {
    pub tipo: &'static str,
    pub geracao: Geracao,
    pub itens: Vec<ItemDoLote>,
}

/// Gera a versão inicial do tipo para as funcionalidades: os tipos do app saem
/// na hora; os do agente devolvem o pedido de uma sessão por funcionalidade. Só
/// as funcionalidades sem documento do tipo entram.
pub fn gerar_em_lote(
    ambiente: &Ambiente,
    tipo: &str,
    funcionalidades: &[String],
) -> Result<Lote, ErroFerramenta> {
    let t = tipo_da_funcionalidade(tipo)?;
    let config = entregaveis::efetiva(ambiente.projeto.entregaveis.as_ref());
    if config.habilitado(t.id).is_none() {
        return Err(ErroFerramenta::Recusa(format!(
            "{} não está habilitado nos entregáveis do projeto",
            t.nome
        )));
    }
    if funcionalidades.is_empty() {
        return Err(ErroFerramenta::Argumentos(
            "escolha pelo menos uma funcionalidade".into(),
        ));
    }
    let mapa = MapaFuncionalidades::carregar(&ambiente.kb).map_err(interno)?;
    let estado = DocumentosVivos::carregar(&ambiente.kb)?;
    let mut vistas = BTreeSet::new();
    let mut itens = Vec::new();
    for texto in funcionalidades {
        let pulada = |motivo: String| ItemDoLote::Pulada {
            funcionalidade: texto.trim().to_owned(),
            motivo,
        };
        let id = match id_da_funcionalidade(texto) {
            Ok(id) => id,
            Err(e) => {
                itens.push(pulada(e.to_string()));
                continue;
            }
        };
        if !vistas.insert(id.clone()) {
            continue;
        }
        let f = match aberta(&mapa, &id) {
            Ok(f) => f,
            Err(e) => {
                itens.push(pulada(e.to_string()));
                continue;
            }
        };
        let celula = documentos_da_funcionalidade(&ambiente.escopo, &estado, &f, &[t.id]);
        if let Some(d) = celula.first()
            && d.estado != EstadoDocumento::SemDocumento
        {
            itens.push(pulada(format!(
                "já tem o documento ({}): um documento vivo nunca é recriado",
                d.estado.rotulo()
            )));
            continue;
        }
        let item = match t.geracao {
            Geracao::App => {
                match contexto(ambiente, t.id, &id)
                    .and_then(|c| registrar(ambiente, t.id, &id, &montar(&c), Vec::new()))
                {
                    Ok(r) => ItemDoLote::Gerada {
                        funcionalidade: id.to_string(),
                        caminho: r.caminho,
                    },
                    Err(e) => pulada(e.to_string()),
                }
            }
            Geracao::Agente => ItemDoLote::Sessao {
                funcionalidade: id.to_string(),
                pedido: pedido_de_versao_inicial(t, &id),
            },
        };
        itens.push(item);
    }
    let geradas = itens
        .iter()
        .filter(|i| matches!(i, ItemDoLote::Gerada { .. }))
        .count();
    tracing::info!(
        tipo = t.id,
        pedidas = funcionalidades.len(),
        geradas,
        "versão inicial em lote"
    );
    Ok(Lote {
        tipo: t.id,
        geracao: t.geracao,
        itens,
    })
}

// ---------------------------------------------------------------------------
// Ferramentas

#[derive(Deserialize)]
struct ArgsDoAlvo {
    tipo: String,
    funcionalidade: String,
}

fn schema_do_alvo(extra: serde_json::Map<String, serde_json::Value>) -> serde_json::Value {
    let mut propriedades = serde_json::Map::new();
    propriedades.insert(
        "tipo".into(),
        json!({
            "type": "string",
            "minLength": 1,
            "description": "Id do tipo no catálogo de entregáveis (`caso-de-uso`, `manual`)."
        }),
    );
    propriedades.insert(
        "funcionalidade".into(),
        json!({
            "type": "string",
            "minLength": 1,
            "description": "Funcionalidade do mapa (`F-FAT-001`)."
        }),
    );
    let obrigatorios = vec![json!("tipo"), json!("funcionalidade")];
    for (nome, valor) in extra {
        propriedades.insert(nome, valor);
    }
    json!({
        "type": "object",
        "properties": propriedades,
        "required": obrigatorios,
        "additionalProperties": false
    })
}

/// `iniciar_versao_inicial`: o roteiro, as seções do tipo e o contexto.
pub fn iniciar_versao_inicial() -> Ferramenta {
    Ferramenta::nova(
        "iniciar_versao_inicial",
        "Iniciar a versão inicial de um documento vivo",
        "Começa a versão inicial (1.0) de um documento vivo de uma funcionalidade do mapa \
         (caso de uso, manual, documento negocial), por engenharia reversa, e devolve o \
         roteiro: as seções do tipo, a funcionalidade (atores, pontos de entrada, dados \
         lidos e gravados) e as regras da base ligadas a ela. Recusa se a funcionalidade já \
         tem o documento. Siga o roteiro devolvido e registre com \
         `registrar_versao_inicial`.",
        schema_do_alvo(serde_json::Map::new()),
        |ambiente, args| {
            let args: ArgsDoAlvo = ler_argumentos(args)?;
            let id = id_da_funcionalidade(&args.funcionalidade)?;
            let c = contexto(ambiente, &args.tipo, &id)?;
            let estado = DocumentosVivos::carregar(&ambiente.kb)?;
            if let Some(v) = estado
                .do_dono(c.tipo.id, &Dono::Funcionalidade(id.clone()))
                .and_then(|d| d.versao_publicada())
            {
                return Err(ErroFerramenta::Recusa(format!(
                    "{} de {id} já está na versão {v}: um documento vivo nunca é recriado",
                    c.tipo.nome
                )));
            }
            let pelo_esquema = registro_pelo_esquema(c.tipo);
            let secoes = if pelo_esquema {
                format!(
                    "## Esquema (campos)\n\n{}",
                    crate::entregaveis::esquema_em_markdown(c.tipo)
                )
            } else {
                let lista: String = secoes(c.tipo)
                    .iter()
                    .enumerate()
                    .map(|(i, s)| format!("{}. {s}\n", i + 1))
                    .collect();
                format!("## Seções\n\n{lista}")
            };
            let (redacao, registro) = if pelo_esquema {
                (
                    "**Preencha os campos do esquema** abaixo, em português e em linguagem de \
                     negócio. Não invente: o que nem o código nem a base sustentam fica de fora, \
                     e um campo opcional sem conteúdo não vai. O app monta o documento a partir \
                     dos campos.",
                    "com os `campos` do esquema (não mande `texto`).",
                )
            } else {
                (
                    "**Redija em Markdown**, em português e em linguagem de negócio, com as \
                     seções abaixo, nesta ordem. Não invente: o que nem o código nem a base \
                     sustentam fica de fora. Uma seção sem conteúdo leva \"Não identificado no \
                     código.\"",
                    "com o texto completo.",
                )
            };
            let guia = crate::entregaveis::guia_do_tipo(c.tipo.id)
                .map(|g| format!("\n{}\n", g.trim_end()))
                .unwrap_or_default();
            let template = template_da_base(ambiente, c.tipo)
                .map(|(nome, texto)| {
                    format!(
                        "\nO projeto usa o template `{nome}` para este tipo: siga a estrutura \
                         dele (os marcadores `{{{{...}}}}` indicam o que vai em cada lugar).\n\n\
                         ```markdown\n{}\n```\n",
                        texto.trim()
                    )
                })
                .unwrap_or_default();
            let argumentos = format!(
                " (tipo \"{}\", funcionalidade \"{}\")",
                c.tipo.id, c.funcionalidade.id
            );
            let texto = ROTEIRO
                .replace("{{tipo}}", c.tipo.nome)
                .replace("{{funcionalidade}}", &c.funcionalidade.nome)
                .replace("{{id}}", &c.funcionalidade.id.to_string())
                .replace("{{argumentos}}", &argumentos)
                .replace("{{redacao}}", redacao)
                .replace("{{registro}}", registro)
                .replace("{{secoes}}", &secoes)
                .replace("{{template}}", &template)
                .replace("{{guia}}", &guia)
                .replace("{{contexto}}", &contexto_em_markdown(&c, "###"));
            Ok(Saida::texto(texto))
        },
    )
    .somente_leitura()
}

/// O template escolhido para o tipo no projeto, quando é um arquivo da base.
fn template_da_base(ambiente: &Ambiente, tipo: &TipoEntregavel) -> Option<(String, String)> {
    let config = entregaveis::efetiva(ambiente.projeto.entregaveis.as_ref());
    let nome = config.habilitado(tipo.id)?.template.clone()?;
    let texto =
        std::fs::read_to_string(ambiente.base.raiz().join(layout::TEMPLATES).join(&nome)).ok()?;
    Some((format!("{}/{nome}", layout::TEMPLATES), texto))
}

/// `registrar_versao_inicial`: registra a 1.0 como rascunho.
pub fn registrar_versao_inicial() -> Ferramenta {
    let mut extra = serde_json::Map::new();
    extra.insert(
        "texto".into(),
        json!({
            "type": "string",
            "minLength": 1,
            "description": "O documento completo, em Markdown, com as seções do roteiro. \
                            Não vale para o caso de uso, que vem em `campos`."
        }),
    );
    extra.insert(
        "campos".into(),
        json!({
            "type": "object",
            "description": "Só nos tipos registrados pelo esquema (o caso de uso): os campos \
                            do esquema devolvido por iniciar_versao_inicial. O app confere o \
                            esquema e as regras citadas e monta o documento."
        }),
    );
    extra.insert(
        "arquivos".into(),
        json!({
            "type": "array",
            "items": { "type": "string", "minLength": 1 },
            "description": "Opcional: arquivos do código em que o texto se apoia, além dos \
                            pontos de entrada e das evidências das regras (que o app já inclui)."
        }),
    );
    Ferramenta::nova(
        "registrar_versao_inicial",
        "Registrar a versão inicial de um documento vivo",
        "Registra a versão inicial (1.0) de um documento vivo da funcionalidade, como \
         rascunho: o analista faz a revisão final antes de publicar. Recusa se a \
         funcionalidade já tem o documento publicado (um documento vivo nunca é recriado) ou \
         se o texto cita uma regra que não existe na base. O caso de uso vem em `campos`, pelo \
         esquema, com a regra citada em cada etapa. Chamar de novo refaz o rascunho.",
        schema_do_alvo(extra),
        |ambiente, args| {
            #[derive(Deserialize)]
            struct Args {
                #[serde(flatten)]
                alvo: ArgsDoAlvo,
                #[serde(default)]
                texto: Option<String>,
                #[serde(default)]
                campos: Option<serde_json::Value>,
                #[serde(default)]
                arquivos: Vec<String>,
            }
            let args: Args = ler_argumentos(args)?;
            let id = id_da_funcionalidade(&args.alvo.funcionalidade)?;
            let tipo = tipo_da_funcionalidade(&args.alvo.tipo)?;
            let r = match (registro_pelo_esquema(tipo), args.texto, args.campos) {
                (true, _, Some(campos)) => {
                    registrar_pelos_campos(ambiente, tipo.id, &id, &campos, args.arquivos)?
                }
                (true, _, None) => {
                    return Err(ErroFerramenta::Argumentos(format!(
                        "`campos`: {} é registrado pelos campos do esquema devolvido por \
                         iniciar_versao_inicial, não por `texto`",
                        tipo.nome
                    )));
                }
                (false, Some(texto), _) if !texto.trim().is_empty() => {
                    registrar(ambiente, tipo.id, &id, &texto, args.arquivos)?
                }
                (false, _, _) => {
                    return Err(ErroFerramenta::Argumentos(format!(
                        "`texto`: mande o documento completo de {} em Markdown",
                        tipo.nome
                    )));
                }
            };
            Ok(Saida::json(json!({
                "caminho": ambiente.kb.na_base(&r.caminho).to_string(),
                "versao": r.proposta.versao,
                "estado": "rascunho",
                "proximo_passo": "O rascunho está na visão Documentação do mapa, à espera da \
                                  revisão final do analista. Diga isso numa frase e encerre."
            })))
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regras_citadas_acha_so_identificadores_de_regra() {
        let t = "Passo 1 (RN-FAT-001). Ver RN-FAT-012, F-FAT-001 e RN-sem.";
        assert_eq!(
            regras_citadas(t).into_iter().collect::<Vec<_>>(),
            ["RN-FAT-001", "RN-FAT-012"]
        );
    }

    #[test]
    fn enunciado_tira_as_notas() {
        let corpo = "# T\n\n## Enunciado\n\nA multa é 2%[^ev-1].\n\n[^ev-1]: x\n\n## Evidências\n";
        assert_eq!(enunciado(corpo).as_deref(), Some("A multa é 2%."));
    }

    #[test]
    fn secoes_vem_do_template_ou_dos_campos() {
        let caso = entregaveis::tipo(entregaveis::CASO_DE_USO).unwrap();
        assert_eq!(
            secoes(caso),
            [
                "Objetivo",
                "Atores",
                "Gatilho",
                "Pré-condições",
                "Fluxo principal",
                "Fluxos alternativos",
                "Exceções",
                "Pós-condições",
                "Regras de negócio"
            ]
        );
        let sup = entregaveis::tipo(entregaveis::ESPECIFICACAO_SUPLEMENTAR).unwrap();
        assert_eq!(
            secoes(sup),
            ["Requisitos não funcionais", "Regras de origem"]
        );
    }
}
