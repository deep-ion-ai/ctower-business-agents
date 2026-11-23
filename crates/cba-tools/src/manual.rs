//! Fluxo "Manual do usuário" (#272): o manual de uma funcionalidade do mapa, com
//! o passo a passo e os prints de tela.
//!
//! - [`criar_manual`] abre a análise do fluxo `manual` a partir do painel de uma
//!   funcionalidade aceita, com a entrada montada do mapa ([`entrada_do_manual`]:
//!   descrição, atores, pontos de entrada, explicação aprovada, regras
//!   confirmadas e demandas ligadas). Uma funcionalidade tem no máximo um manual
//!   em andamento; quando já há um aprovado, o novo começa com os passos e os
//!   prints dele, para revisar sem perder as imagens.
//! - `registrar_passo_manual` (ferramenta MCP) registra ou reescreve um passo; o
//!   analista inclui, troca e remove os prints no app ([`anexar_print`],
//!   [`remover_print`]). Os prints são PNG, ficam em
//!   `funcionalidades/<area>/<tema>/manual/` e o estado guarda o caminho.
//! - [`texto`] monta a seção `passo_a_passo` do documento; passo sem print mostra
//!   o marcador "Print sugerido: tela X". [`pendencias`] impede a aprovação com
//!   passo sem print e sem dispensa.
//! - Aprovado, [`ligar_manual`] grava `funcionalidades/<area>/<tema>/manual.md`,
//!   com os links refeitos para a nova pasta. [`situacao`] diz se o manual
//!   aprovado está "a revisar" porque a funcionalidade mudou depois dele.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use cba_core::ambiente::Ambiente;
use cba_core::analise::{Analise, EstadoAfirmacao, EstadoAnalise, Fluxo};
use cba_core::analises::NovaAnalise;
use cba_core::eventos::Mudanca;
use cba_core::funcionalidades::{Funcionalidade, MapaFuncionalidades, SituacaoFuncionalidade};
use cba_core::manual::{NovoPasso, PassoManual, PrintDoPasso};
use cba_docs::{PASSO_A_PASSO, Texto};
use cba_index::TipoEntrada;
use cba_kb::frontmatter::{
    FrontmatterAnalise, FrontmatterFuncionalidade, FrontmatterRegra, StatusAnalise, StatusRegra,
    TipoDocumento,
};
use cba_kb::{CaminhoRelativo, Documento, Id, IdFuncionalidade, layout};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::comum;
use crate::registro::{ErroFerramenta, Ferramenta, ler_argumentos};

/// Nome do documento aprovado na pasta da funcionalidade.
pub const ARQUIVO_MANUAL: &str = "manual.md";
/// Pasta dos prints, ao lado do `manual.md`.
pub const PASTA_PRINTS: &str = "manual";
/// Tamanho máximo de um print (o mesmo das figuras dos diagramas).
pub const TAMANHO_MAXIMO_DO_PRINT: usize = cba_docs::TAMANHO_MAXIMO_DA_FIGURA;

/// Texto numa linha só.
fn em_uma_linha(texto: &str) -> String {
    texto.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Texto alternativo e legenda sem o que quebra o Markdown da imagem.
fn limpo(texto: &str) -> String {
    em_uma_linha(texto).replace(['[', ']'], "")
}

// ---------------------------------------------------------------------------
// Documento

/// Seção `passo_a_passo` do documento em `caminho` (os links dos prints são
/// relativos a ele).
pub fn texto(analise: &Analise, caminho: &CaminhoRelativo) -> Texto {
    let mut t = String::new();
    for p in &analise.passos {
        if !t.is_empty() {
            t.push_str("\n\n");
        }
        let _ = write!(
            t,
            "### Passo {} · {}\n\n{}",
            p.numero,
            em_uma_linha(&p.titulo),
            p.instrucao.trim()
        );
        match (&p.print, p.print_dispensado) {
            (Some(print), _) => {
                let imagem =
                    cba_docs::link(&limpo(&print.texto_alternativo), caminho, &print.arquivo);
                let _ = write!(
                    t,
                    "\n\n!{imagem}\n\n_Figura {}. {}_",
                    p.numero,
                    em_uma_linha(&print.legenda)
                );
            }
            (None, false) => {
                let tela = p.tela.as_deref().map_or_else(
                    || "a tela deste passo".to_owned(),
                    |t| format!("tela {}", em_uma_linha(t)),
                );
                let _ = write!(t, "\n\n> **Print sugerido:** {tela}");
            }
            (None, true) => {}
        }
        if let Some(r) = &p.resultado {
            let _ = write!(t, "\n\n**Resultado:** {}", r.trim());
        }
    }
    Texto::novo(PASSO_A_PASSO, t)
}

/// Resumo dos passos para `retomar_analise`.
pub fn para_retomar(analise: &Analise) -> String {
    let mut t = String::from("## Passo a passo\n\n");
    if analise.passos.is_empty() {
        t.push_str("Nenhum passo registrado.\n\n");
        return t;
    }
    for p in &analise.passos {
        let print = match (&p.print, p.print_dispensado) {
            (Some(_), _) => "com print",
            (None, true) => "print dispensado",
            (None, false) => "sem print",
        };
        let _ = writeln!(
            t,
            "{}. {} [{print}]{}",
            p.numero,
            em_uma_linha(&p.titulo),
            p.tela
                .as_deref()
                .map(|x| format!(" · tela: {}", em_uma_linha(x)))
                .unwrap_or_default()
        );
        let _ = writeln!(t, "   {}", em_uma_linha(&p.instrucao));
    }
    t.push('\n');
    t
}

// ---------------------------------------------------------------------------
// Ferramenta

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArgsPasso {
    analise: String,
    #[serde(default)]
    numero: Option<u32>,
    titulo: String,
    instrucao: String,
    #[serde(default)]
    tela: Option<String>,
    #[serde(default)]
    resultado: Option<String>,
}

/// `registrar_passo_manual`: um passo do manual do usuário.
pub fn registrar_passo_manual() -> Ferramenta {
    Ferramenta::nova(
        "registrar_passo_manual",
        "Registrar passo do manual",
        "Registra um passo do manual do usuário (fluxo `manual`), na ordem: o título \
         curto no imperativo, a instrução em linguagem de usuário final (o campo, o \
         botão, o valor), a tela em que o passo acontece (vira o marcador \"print \
         sugerido\" que o analista preenche com a imagem no app) e o resultado que o \
         usuário vê. Sem `numero`, o passo entra no fim; com o número de um passo já \
         registrado, reescreve o texto dele e mantém o print. Não descreva imagens: os \
         prints são incluídos pelo analista. Devolve o número do passo.",
        json!({
            "type": "object",
            "properties": {
                "analise": comum::schema_da_analise(),
                "numero": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Opcional: o número do passo a reescrever. Sem ele, o passo entra no fim."
                },
                "titulo": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 200,
                    "description": "Título curto, no imperativo (\"Informe o motivo do cancelamento\")."
                },
                "instrucao": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": cba_core::manual::LIMITE_DO_TEXTO,
                    "description": "O que o usuário faz, em linguagem de usuário final: os campos, os botões e os valores, como aparecem na tela. Sem nome de tabela, classe ou rotina. Aceita Markdown simples (lista para subpassos)."
                },
                "tela": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 200,
                    "description": "Nome de negócio da tela em que o passo acontece (dos pontos de entrada de tipo tela): o print sugerido."
                },
                "resultado": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": cba_core::manual::LIMITE_DO_TEXTO,
                    "description": "O que o usuário vê depois do passo (mensagem, tela seguinte, situação nova)."
                }
            },
            "required": ["analise", "titulo", "instrucao"],
            "additionalProperties": false
        }),
        |ambiente, args| {
            let args: ArgsPasso = ler_argumentos(args)?;
            let id = comum::id_da_analise(&args.analise)?;
            let registrado = comum::alterar(ambiente, &id, Mudanca::Passo, |a, _| {
                a.registrar_passo(NovoPasso {
                    numero: args.numero,
                    titulo: args.titulo,
                    instrucao: args.instrucao,
                    tela: args.tela,
                    resultado: args.resultado,
                })
            })?;
            let n = registrado.resultado;
            let total = registrado.analise.passos.len();
            Ok(comum::resposta(
                ambiente,
                &registrado,
                json!({ "passo": n, "passos": total }),
            ))
        },
    )
}

/// No fluxo `manual`, `concluir_analise` exige ao menos um passo.
pub fn conferir_para_concluir(analise: &Analise) -> Result<(), ErroFerramenta> {
    if analise.passos.is_empty() {
        return Err(ErroFerramenta::Recusa(format!(
            "o manual {} ainda não tem passos. Registre o passo a passo com \
             `registrar_passo_manual`, um passo por chamada, na ordem, e chame \
             `concluir_analise` de novo",
            analise.id
        )));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Criação

/// Manuais (análises do fluxo `manual`) de cada funcionalidade, do mais antigo
/// para o mais novo.
pub fn manuais(
    analises: &cba_core::analises::Analises,
) -> BTreeMap<IdFuncionalidade, Vec<Analise>> {
    let mut mapa: BTreeMap<IdFuncionalidade, Vec<Analise>> = BTreeMap::new();
    for a in analises.listar().unwrap_or_default() {
        if a.fluxo != Fluxo::Manual {
            continue;
        }
        if let Some(f) = a.funcionalidade.clone() {
            mapa.entry(f).or_default().push(a);
        }
    }
    mapa
}

/// Regras confirmadas da base ligadas à funcionalidade (#287): identificador e
/// título.
fn regras_da_funcionalidade(kb: &cba_kb::Projeto, id: &IdFuncionalidade) -> Vec<FrontmatterRegra> {
    let mut lista = Vec::new();
    for rel in kb.documentos().unwrap_or_default() {
        if rel.partes().next() != Some(layout::REGRAS) || !rel.as_str().ends_with(".md") {
            continue;
        }
        let Ok(doc) = kb.ler::<FrontmatterRegra>(&rel) else {
            continue;
        };
        if doc.frontmatter.funcionalidades.contains(id) {
            lista.push(doc.frontmatter);
        }
    }
    lista.sort_by_key(|r| r.id.to_string());
    lista
}

/// Entrada do manual: a funcionalidade como o mapa a descreve e o que já está
/// aprovado sobre ela.
pub fn entrada_do_manual(ambiente: &Ambiente, f: &Funcionalidade) -> String {
    let mut t = format!(
        "Manual do usuário da funcionalidade {} · {} (área {}).\n\n{}",
        f.id,
        f.nome,
        f.area_nome,
        em_uma_linha(&f.descricao)
    );
    if !f.atores.is_empty() {
        let _ = write!(t, "\n\nAtores: {}.", f.atores.join(", "));
    }
    if !f.pontos.is_empty() {
        t.push_str("\n\nPontos de entrada:");
        for p in &f.pontos {
            let tipo = p.tipo.map(TipoEntrada::nome).unwrap_or("ponto");
            let _ = write!(
                t,
                "\n- {tipo}: {} (`{}`, linhas {}-{})",
                p.nome, p.evidencia.arquivo, p.evidencia.linhas.inicio, p.evidencia.linhas.fim
            );
        }
        let telas: Vec<&str> = f
            .pontos
            .iter()
            .filter(|p| p.tipo == Some(TipoEntrada::Tela))
            .map(|p| p.nome.as_str())
            .collect();
        if !telas.is_empty() {
            let _ = write!(
                t,
                "\n\nTelas (prints sugeridos dos passos): {}.",
                telas.join(", ")
            );
        }
    }
    if let Some(a) = crate::mapear::explicacoes(&ambiente.analises)
        .get(&f.id)
        .filter(|a| a.estado == EstadoAnalise::Aprovada)
    {
        let _ = write!(
            t,
            "\n\nExplicação aprovada: {} (leia com `retomar_analise`, analise: \"{}\").",
            a.id, a.id
        );
    }
    let regras: Vec<FrontmatterRegra> = regras_da_funcionalidade(&ambiente.kb, &f.id)
        .into_iter()
        .filter(|r| r.status == StatusRegra::Confirmada)
        .collect();
    if !regras.is_empty() {
        t.push_str("\n\nRegras confirmadas:");
        for r in &regras {
            let _ = write!(t, "\n- {}: {}", r.id, em_uma_linha(&r.titulo));
        }
    }
    if !f.demandas.is_empty() {
        let lidas = ambiente.kb.demandas().unwrap_or_default();
        t.push_str("\n\nDemandas ligadas:");
        for d in &f.demandas {
            let titulo = lidas
                .iter()
                .find(|l| l.id == *d)
                .map(|l| l.documento.frontmatter.titulo.clone())
                .unwrap_or_default();
            let _ = write!(t, "\n- {d}: {}", em_uma_linha(&titulo));
        }
    }
    t
}

/// Cria o manual da funcionalidade: análise do fluxo `manual` ligada a ela. A
/// funcionalidade precisa estar aceita no mapa e sem manual em andamento. Com um
/// manual aprovado, o novo começa com os passos e os prints dele.
pub fn criar_manual(ambiente: &Ambiente, id: &IdFuncionalidade) -> Result<Analise, ErroFerramenta> {
    let m = crate::mapear::mapa(ambiente)?;
    let f = m.funcionalidade(id).filter(|f| f.aberta()).ok_or_else(|| {
        ErroFerramenta::Recusa(format!(
            "a funcionalidade {id} não está no mapa de funcionalidades do projeto"
        ))
    })?;
    if f.situacao != SituacaoFuncionalidade::Aceita {
        return Err(ErroFerramenta::Recusa(format!(
            "a funcionalidade {id} ainda não foi aceita no mapa; revise o mapeamento antes \
             de gerar o manual"
        )));
    }
    let anteriores = manuais(&ambiente.analises).remove(id).unwrap_or_default();
    if let Some(a) = anteriores
        .iter()
        .find(|a| a.estado != EstadoAnalise::Aprovada)
    {
        return Err(ErroFerramenta::Recusa(format!(
            "a funcionalidade {id} já tem o manual {} em andamento; continue nele",
            a.id
        )));
    }
    let passos: Vec<PassoManual> = anteriores
        .iter()
        .rev()
        .find(|a| a.estado == EstadoAnalise::Aprovada)
        .map(|a| a.passos.clone())
        .unwrap_or_default();
    let mut entrada = entrada_do_manual(ambiente, f);
    if !passos.is_empty() {
        let _ = write!(
            entrada,
            "\n\nRevisão do manual aprovado: os {} passos (e os prints) dele já estão \
             registrados; reescreva com `registrar_passo_manual` (com `numero`) o que \
             mudou.",
            passos.len()
        );
    }
    let analise = ambiente.analises.criar_com_demanda(
        NovaAnalise {
            fluxo: Fluxo::Manual,
            titulo: format!("{} · {} · manual do usuário", f.id, f.nome),
            entrada,
            analista: None,
            base: crate::ciclo::versao_do_codigo(ambiente),
        },
        None,
    )?;
    let id = id.clone();
    let registrado = comum::alterar(ambiente, &analise.id, Mudanca::Estado, |a, _| {
        a.funcionalidade = Some(id);
        a.passos = passos;
        Ok(())
    })?;
    Ok(registrado.analise)
}

// ---------------------------------------------------------------------------
// Prints

/// Print a incluir num passo, como o app envia.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NovoPrint {
    /// Bytes do PNG (a tela converte outros formatos antes de enviar).
    pub png: Vec<u8>,
    pub legenda: String,
    pub texto_alternativo: String,
}

/// Por que o print foi recusado ou não foi gravado.
#[derive(Debug, thiserror::Error)]
pub enum ErroPrint {
    #[error(transparent)]
    Analise(#[from] cba_core::analises::Erro),
    #[error("{0}")]
    Recusa(String),
    #[error("o print não foi gravado na base: {0}")]
    Gravar(std::io::Error),
}

/// Pasta dos prints da funcionalidade do manual.
fn pasta_dos_prints(ambiente: &Ambiente, analise: &Analise) -> Result<CaminhoRelativo, ErroPrint> {
    let fid = analise.funcionalidade.as_ref().ok_or_else(|| {
        ErroPrint::Recusa(format!(
            "o manual {} não está ligado a uma funcionalidade do mapa",
            analise.id
        ))
    })?;
    let mapa = MapaFuncionalidades::carregar(&ambiente.kb)
        .map_err(|e| ErroPrint::Recusa(e.to_string()))?;
    let f = mapa.funcionalidade(fid).ok_or_else(|| {
        ErroPrint::Recusa(format!("a funcionalidade {fid} não está mais no mapa"))
    })?;
    layout::funcionalidade(&f.area, &f.nome)
        .and_then(|p| p.juntar(PASTA_PRINTS))
        .map_err(|e| ErroPrint::Recusa(e.to_string()))
}

/// Se algum passo de algum manual usa o arquivo.
fn em_uso(ambiente: &Ambiente, arquivo: &CaminhoRelativo) -> bool {
    manuais(&ambiente.analises)
        .values()
        .flatten()
        .any(|a| a.prints().any(|p| p == arquivo))
}

/// Apaga o print se nenhum passo de nenhum manual o usa mais.
fn apagar_se_livre(ambiente: &Ambiente, arquivo: &CaminhoRelativo) {
    if em_uso(ambiente, arquivo) {
        return;
    }
    if let Err(e) = std::fs::remove_file(ambiente.kb.caminho(arquivo))
        && e.kind() != std::io::ErrorKind::NotFound
    {
        tracing::warn!("print antigo não apagado");
    }
}

/// Inclui ou troca o print do passo `numero`: confere o PNG (assinatura,
/// cabeçalho e tamanho), grava em `funcionalidades/<area>/<tema>/manual/` e
/// remonta o documento. O print anterior é apagado se nenhum outro manual o usa.
pub fn anexar_print(
    ambiente: &Ambiente,
    id: &Id,
    numero: u32,
    novo: NovoPrint,
) -> Result<Analise, ErroPrint> {
    if novo.png.len() > TAMANHO_MAXIMO_DO_PRINT {
        return Err(ErroPrint::Recusa(format!(
            "o print passa de {} MiB; reduza a imagem (recorte só a tela) e envie de novo",
            TAMANHO_MAXIMO_DO_PRINT / 1024 / 1024
        )));
    }
    cba_docs::Figura::nova(novo.png.clone(), 0, 0)
        .map_err(|_| ErroPrint::Recusa("o print precisa ser uma imagem PNG válida".into()))?;
    let atual = ambiente.analises.carregar(id)?;
    if atual.fluxo != Fluxo::Manual {
        return Err(ErroPrint::Recusa(format!(
            "a análise {id} não é um manual do usuário"
        )));
    }
    if atual.estado == EstadoAnalise::Aprovada {
        return Err(ErroPrint::Recusa(format!(
            "o manual {id} já foi aprovado; gere a revisão do manual no painel da \
             funcionalidade para trocar os prints"
        )));
    }
    if !atual.passos.iter().any(|p| p.numero == numero) {
        return Err(ErroPrint::Recusa(format!(
            "o manual {id} não tem o passo {numero}"
        )));
    }
    let pasta = pasta_dos_prints(ambiente, &atual)?;
    let hash = Sha256::digest(&novo.png);
    let nome = format!(
        "passo-{numero:02}-{}.png",
        hash.iter()
            .take(4)
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    );
    let arquivo = pasta
        .juntar(&nome)
        .map_err(|e| ErroPrint::Recusa(e.to_string()))?;
    cba_kb::arquivo::gravar(&ambiente.kb.caminho(&arquivo), &novo.png)
        .map_err(ErroPrint::Gravar)?;
    let print = PrintDoPasso {
        arquivo: arquivo.clone(),
        legenda: novo.legenda,
        texto_alternativo: novo.texto_alternativo,
    };
    let registrado = comum::alterar(ambiente, id, Mudanca::Passo, |a, _| {
        a.definir_print(numero, Some(print))
    });
    let registrado = match registrado {
        Ok(r) => r,
        Err(e) => {
            apagar_se_livre(ambiente, &arquivo);
            return Err(ErroPrint::Recusa(e.to_string()));
        }
    };
    if let Some(antigo) = registrado.resultado
        && antigo.arquivo != arquivo
    {
        apagar_se_livre(ambiente, &antigo.arquivo);
    }
    Ok(registrado.analise)
}

/// Remove o print do passo; o arquivo é apagado se nenhum outro manual o usa.
pub fn remover_print(ambiente: &Ambiente, id: &Id, numero: u32) -> Result<Analise, ErroPrint> {
    let registrado = comum::alterar(ambiente, id, Mudanca::Passo, |a, _| {
        a.definir_print(numero, None)
    })
    .map_err(|e| ErroPrint::Recusa(e.to_string()))?;
    if let Some(antigo) = registrado.resultado {
        apagar_se_livre(ambiente, &antigo.arquivo);
    }
    Ok(registrado.analise)
}

/// Remove o passo (do analista) e, se for o caso, o arquivo do print dele.
pub fn remover_passo(ambiente: &Ambiente, id: &Id, numero: u32) -> Result<Analise, ErroPrint> {
    let registrado = comum::alterar(ambiente, id, Mudanca::Passo, |a, _| a.remover_passo(numero))
        .map_err(|e| ErroPrint::Recusa(e.to_string()))?;
    if let Some(antigo) = registrado.resultado {
        apagar_se_livre(ambiente, &antigo.arquivo);
    }
    Ok(registrado.analise)
}

/// Bytes do print, para a prévia no app: só PNG dentro da pasta
/// `funcionalidades/`, no limite de tamanho.
pub fn ler_print(kb: &cba_kb::Projeto, arquivo: &CaminhoRelativo) -> Result<Vec<u8>, String> {
    if arquivo.partes().next() != Some(layout::FUNCIONALIDADES)
        || !arquivo.as_str().to_ascii_lowercase().ends_with(".png")
    {
        return Err(format!("{arquivo} não é um print do manual"));
    }
    let bytes = std::fs::read(kb.caminho(arquivo)).map_err(|e| format!("{arquivo}: {e}"))?;
    if bytes.len() > TAMANHO_MAXIMO_DO_PRINT {
        return Err(format!("{arquivo} passa do tamanho máximo de um print"));
    }
    Ok(bytes)
}

// ---------------------------------------------------------------------------
// Pendências e aprovação

/// Pendências do manual que impedem a aprovação: nenhum passo, passo sem print
/// (e sem dispensa) e print cujo arquivo sumiu da base.
pub fn pendencias(
    ambiente_kb: Option<&cba_kb::Projeto>,
    analise: &Analise,
) -> Vec<(Option<u32>, String)> {
    let mut lista = Vec::new();
    if analise.passos.is_empty() {
        lista.push((None, "o manual não tem passos registrados".to_owned()));
    }
    for p in &analise.passos {
        if p.sem_print() {
            lista.push((
                Some(p.numero),
                format!(
                    "passo {}: sem print; inclua o print{} ou dispense-o",
                    p.numero,
                    p.tela
                        .as_deref()
                        .map(|t| format!(" da tela {}", em_uma_linha(t)))
                        .unwrap_or_default()
                ),
            ));
        }
        if let (Some(kb), Some(print)) = (ambiente_kb, &p.print)
            && !kb.existe(&print.arquivo)
        {
            lista.push((
                Some(p.numero),
                format!(
                    "passo {}: o arquivo do print ({}) não está mais na base; inclua de novo",
                    p.numero, print.arquivo
                ),
            ));
        }
    }
    lista
}

/// Aprovado o manual ligado a uma funcionalidade do mapa, grava
/// `funcionalidades/<area>/<tema>/manual.md`: o corpo do documento da análise, com
/// os links e as imagens refeitos para a nova pasta, e o frontmatter de
/// funcionalidade (tipo `manual`). Devolve o caminho gravado.
pub fn ligar_manual(
    kb: &cba_kb::Projeto,
    analise: &Analise,
) -> Result<Option<CaminhoRelativo>, String> {
    if analise.fluxo != Fluxo::Manual {
        return Ok(None);
    }
    let Some(id) = &analise.funcionalidade else {
        return Ok(None);
    };
    let m = MapaFuncionalidades::carregar(kb).map_err(|e| e.to_string())?;
    let Some(f) = m.funcionalidade(id).filter(|f| f.aberta()).cloned() else {
        return Err(format!(
            "a funcionalidade {id} não está mais no mapa; o manual ficou só na análise"
        ));
    };
    let destino = layout::funcionalidade(&f.area, &f.nome)
        .and_then(|p| p.juntar(ARQUIVO_MANUAL))
        .map_err(|e| e.to_string())?;
    let corpo = kb
        .ler::<FrontmatterAnalise>(&analise.documento)
        .map_err(|e| e.to_string())?
        .corpo;
    let corpo = cba_docs::relocalizar(&corpo, &analise.documento, &destino);
    let doc = Documento::novo(
        FrontmatterFuncionalidade {
            tipo: TipoDocumento::Manual,
            titulo: f.nome.clone(),
            area: f.area.clone(),
            tema: cba_kb::slug(&f.nome),
            status: StatusAnalise::Aprovada,
            origem: Some(analise.documento.clone()),
            funcionalidade: Some(id.clone()),
        },
        corpo,
    );
    kb.gravar(&destino, &doc).map_err(|e| e.to_string())?;
    Ok(Some(destino))
}

// ---------------------------------------------------------------------------
// Situação no painel

/// Situação do manual de uma funcionalidade.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "situacao")]
pub enum SituacaoManual {
    /// Ainda não há manual.
    Nenhum,
    /// Manual em andamento (criado, com o agente, ou à espera da revisão).
    EmAndamento {
        analise: String,
        estado: EstadoAnalise,
        passos: usize,
        /// Passos sem print e sem dispensa.
        sem_print: usize,
    },
    /// Manual aprovado. `a_revisar` traz os motivos, quando a funcionalidade mudou
    /// depois da aprovação; os prints ficam.
    Aprovado {
        analise: String,
        documento: Option<String>,
        a_revisar: Vec<String>,
    },
}

/// Motivos para revisar o manual aprovado `manual`: o mapeamento da
/// funcionalidade mudou depois, há explicação aprovada mais nova, alguma regra
/// ligada a ela está a revalidar ou alguma afirmação do manual ficou a
/// revalidar (o código citado mudou).
pub fn motivos_para_revisar(
    ambiente: &Ambiente,
    f: &Funcionalidade,
    manual: &Analise,
) -> Vec<String> {
    let mut motivos = Vec::new();
    let aprovado_ms = manual.atualizada_ms;
    if f.atualizada_ms > aprovado_ms {
        motivos.push("o mapeamento da funcionalidade mudou depois da aprovação".to_owned());
    }
    if let Some(e) = crate::mapear::explicacoes(&ambiente.analises)
        .get(&f.id)
        .filter(|e| e.estado == EstadoAnalise::Aprovada && e.atualizada_ms > aprovado_ms)
    {
        motivos.push(format!(
            "a explicação {} foi aprovada depois do manual",
            e.id
        ));
    }
    let a_revalidar: Vec<String> = regras_da_funcionalidade(&ambiente.kb, &f.id)
        .into_iter()
        .filter(|r| r.status == StatusRegra::ARevalidar)
        .map(|r| r.id.to_string())
        .collect();
    if !a_revalidar.is_empty() {
        motivos.push(format!(
            "regra(s) da funcionalidade a revalidar: {}",
            a_revalidar.join(", ")
        ));
    }
    let afirmacoes = manual
        .afirmacoes
        .iter()
        .filter(|a| a.estado == EstadoAfirmacao::ARevalidar)
        .count();
    if afirmacoes > 0 {
        motivos.push(format!(
            "{afirmacoes} afirmação(ões) do manual citam código que mudou"
        ));
    }
    motivos
}

/// Situação do manual da funcionalidade `f`.
pub fn situacao(ambiente: &Ambiente, f: &Funcionalidade) -> SituacaoManual {
    let lista = manuais(&ambiente.analises)
        .remove(&f.id)
        .unwrap_or_default();
    if let Some(a) = lista
        .iter()
        .rev()
        .find(|a| a.estado != EstadoAnalise::Aprovada)
    {
        return SituacaoManual::EmAndamento {
            analise: a.id.to_string(),
            estado: a.estado,
            passos: a.passos.len(),
            sem_print: a.passos.iter().filter(|p| p.sem_print()).count(),
        };
    }
    match lista
        .iter()
        .rev()
        .find(|a| a.estado == EstadoAnalise::Aprovada)
    {
        Some(a) => {
            let documento = layout::funcionalidade(&f.area, &f.nome)
                .and_then(|p| p.juntar(ARQUIVO_MANUAL))
                .ok()
                .filter(|d| ambiente.kb.existe(d))
                .map(|d| ambiente.kb.na_base(&d).to_string());
            SituacaoManual::Aprovado {
                analise: a.id.to_string(),
                documento,
                a_revisar: motivos_para_revisar(ambiente, f, a),
            }
        }
        None => SituacaoManual::Nenhum,
    }
}

/// Os prints do documento como figuras para a exportação em Word e HTML: cada
/// imagem do texto que aponta para um PNG da base (relativo a `documento`).
/// Imagem que não abre fica de fora (sai como texto alternativo).
pub fn figuras_dos_prints(
    kb: &cba_kb::Projeto,
    documento: &CaminhoRelativo,
    texto: &str,
    figuras: &mut cba_docs::Figuras,
) {
    for destino in cba_docs::imagens_da_base(texto) {
        let Some(arquivo) = cba_docs::resolver(documento, &destino) else {
            continue;
        };
        let Ok(bytes) = ler_print(kb, &arquivo) else {
            continue;
        };
        if let Ok(figura) = cba_docs::Figura::nova(bytes, 0, 0)
            && figuras.inserir_imagem(&destino, figura).is_err()
        {
            break;
        }
    }
}

/// Exportação em Markdown com os prints ao lado: copia cada PNG da base citado
/// no texto para `<pasta do destino>/<nome do arquivo>-imagens/` e troca o
/// destino da imagem pelo caminho da cópia. Devolve o texto.
pub fn com_prints_ao_lado(
    kb: &cba_kb::Projeto,
    documento: &CaminhoRelativo,
    texto: &str,
    destino: &std::path::Path,
) -> Result<String, std::io::Error> {
    let imagens = cba_docs::imagens_da_base(texto);
    if imagens.is_empty() {
        return Ok(texto.to_owned());
    }
    let stem = destino
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("documento");
    let nome_pasta = format!("{stem}-imagens");
    let pasta = destino
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join(&nome_pasta);
    let mut trocas: Vec<(String, String)> = Vec::new();
    for d in imagens {
        let Some(arquivo) = cba_docs::resolver(documento, &d) else {
            continue;
        };
        let Ok(bytes) = ler_print(kb, &arquivo) else {
            continue;
        };
        let nome = arquivo.partes().last().unwrap_or("print.png").to_owned();
        std::fs::create_dir_all(&pasta)?;
        cba_kb::arquivo::gravar(&pasta.join(&nome), &bytes)?;
        trocas.push((d, format!("{nome_pasta}/{nome}")));
    }
    let mut saida = texto.to_owned();
    for (de, para) in trocas {
        saida = saida.replace(&format!("]({de})"), &format!("]({para})"));
        saida = saida.replace(&format!("](<{de}>)"), &format!("](<{para}>)"));
    }
    Ok(saida)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manual() -> Analise {
        let mut a = Analise::nova(
            Id::analisar("A-0003").unwrap(),
            "p",
            Fluxo::Manual,
            "F-CAN-001 · Cancelar fatura · manual do usuário",
            "",
            1,
        );
        for (t, tela) in [("Abra a tela", Some("Cancelamento")), ("Confirme", None)] {
            a.registrar_passo(NovoPasso {
                titulo: t.into(),
                instrucao: "Clique em **Cancelar**.".into(),
                tela: tela.map(Into::into),
                resultado: Some("A fatura fica cancelada.".into()),
                ..Default::default()
            })
            .unwrap();
        }
        a
    }

    #[test]
    fn passo_com_print_marcador_e_dispensa() {
        let mut a = manual();
        a.definir_print(
            1,
            Some(PrintDoPasso {
                arquivo: CaminhoRelativo::novo(
                    "funcionalidades/can/cancelar-fatura/manual/passo-01-abcd.png",
                )
                .unwrap(),
                legenda: "Tela de cancelamento".into(),
                texto_alternativo: "Formulário [com] o motivo".into(),
            }),
        )
        .unwrap();
        let t = texto(&a, &a.documento).texto;
        assert!(t.contains("### Passo 1 · Abra a tela"), "{t}");
        assert!(
            t.contains("![Formulário com o motivo](../funcionalidades/can/cancelar-fatura/manual/passo-01-abcd.png)"),
            "{t}"
        );
        assert!(t.contains("_Figura 1. Tela de cancelamento_"), "{t}");
        assert!(
            t.contains("> **Print sugerido:** a tela deste passo"),
            "{t}"
        );
        assert!(t.contains("**Resultado:** A fatura fica cancelada."), "{t}");
        assert_eq!(pendencias(None, &a).len(), 1);
        a.dispensar_print(2, true).unwrap();
        assert!(!texto(&a, &a.documento).texto.contains("Print sugerido"));
        assert!(pendencias(None, &a).is_empty());
    }
}
