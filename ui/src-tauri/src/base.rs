//! Comandos Tauri da base de conhecimento (spec, seções 7.5 e 10): busca em regras,
//! termos e análises pelo índice FTS5 (`cba_kb::indice`), abertura de um documento
//! (o Markdown, os campos do frontmatter e as ligações com outros documentos) e a
//! vigia da base, que mantém o índice em dia com as edições feitas fora do app.
//!
//! # Índice
//!
//! Cada busca abre o índice e o sincroniza antes (custa pouco quando nada mudou),
//! para refletir na hora o que o app ou o servidor MCP acabaram de gravar. A
//! [`VigiaBase`] (a `cba_kb::indice::Vigia`, a cada 2 s) cobre o que muda fora do
//! app, num editor de texto ou por `git pull`, e avisa as telas abertas para elas
//! buscarem de novo.
//!
//! # Abertura de documento
//!
//! Só documentos `.md` dentro da pasta da base: o caminho é relativo e validado
//! ([`CaminhoRelativo`], sem `..` nem caminho absoluto), e o arquivo, depois de
//! seguir links simbólicos, tem de continuar dentro da base.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use cba_core::config::Config;
use cba_core::dados::DiretorioDados;
use cba_kb::frontmatter::{FrontmatterLivre, TipoDocumento};
use cba_kb::indice::{Consulta, Indice, Modo, Resultado, Sincronizacao, Tipo, Vigia};
use cba_kb::yaml::Value;
use cba_kb::{Base, CaminhoRelativo, Documento, layout};
use serde::Serialize;
use tauri::State;
use tauri::ipc::Channel;

/// Resultados por página da busca.
pub const POR_PAGINA: usize = 20;

fn base_configurada(dados: &DiretorioDados) -> Result<Base, String> {
    let config = Config::ler(dados).map_err(|e| format!("configuração do app ilegível: {e}"))?;
    let raiz = config
        .base()
        .ok_or("a pasta da base de conhecimento ainda não foi escolhida")?;
    Base::abrir(raiz).map_err(|e| e.to_string())
}

fn indice(dados: &DiretorioDados, base: &Base) -> Result<Indice, String> {
    let mut indice = Indice::abrir(dados.arquivo_do_indice(), base)
        .map_err(|e| format!("índice da base: {e}"))?;
    indice
        .sincronizar()
        .map_err(|e| format!("índice da base: {e}"))?;
    Ok(indice)
}

// ---------------------------------------------------------------------------
// Busca

/// Grupo de tipos que a tela filtra.
fn tipos_do_filtro(nome: &str) -> Vec<Tipo> {
    match nome {
        "regra" => vec![Tipo::Documento(TipoDocumento::Regra)],
        "termo" => vec![Tipo::Documento(TipoDocumento::Termo)],
        "analise" => Tipo::ANALISES.to_vec(),
        _ => Vec::new(),
    }
}

/// Um documento encontrado.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Encontrado {
    /// Relativo à raiz da base.
    pub caminho: String,
    pub projeto: String,
    /// `regra`, `termo`, `pergunta`, `analise-impacto`… (`None` sem tipo conhecido).
    pub tipo: Option<String>,
    pub identificador: Option<String>,
    pub titulo: String,
    pub area: Option<String>,
    pub status: Option<String>,
    /// Trecho com as palavras encontradas entre `**`.
    pub trecho: String,
}

impl From<&Resultado> for Encontrado {
    fn from(r: &Resultado) -> Self {
        Self {
            caminho: r.caminho.as_str().to_owned(),
            projeto: r.projeto.clone(),
            tipo: r.tipo.map(|t| t.como_str().to_owned()),
            identificador: r.identificador.clone(),
            titulo: r.titulo.clone(),
            area: r.area.clone(),
            status: r.status.clone(),
            trecho: r.trecho.clone(),
        }
    }
}

/// Uma página de resultados.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Busca {
    pub resultados: Vec<Encontrado>,
    pub total: usize,
    pub deslocamento: usize,
    /// Nenhum documento tinha todas as palavras; estes têm ao menos uma.
    pub qualquer_palavra: bool,
}

/// Busca na base: primeiro com todas as palavras; se nada tiver todas, com qualquer
/// uma (como a ferramenta `conhecimento`). `tipos` filtra por `regra`, `termo` e
/// `analise`; vazio, todos. `projeto` vazio busca em todos.
pub fn buscar(
    dados: &DiretorioDados,
    texto: &str,
    projeto: Option<&str>,
    tipos: &[String],
    deslocamento: usize,
) -> Result<Busca, String> {
    let texto = texto.trim();
    if texto.is_empty() {
        return Ok(Busca {
            resultados: Vec::new(),
            total: 0,
            deslocamento: 0,
            qualquer_palavra: false,
        });
    }
    let base = base_configurada(dados)?;
    let indice = indice(dados, &base)?;
    let tipos: Vec<Tipo> = tipos.iter().flat_map(|t| tipos_do_filtro(t)).collect();
    let consulta = |modo| {
        let mut c = Consulta::nova(texto)
            .modo(modo)
            .tipos(tipos.iter().copied())
            .pagina(POR_PAGINA, deslocamento);
        if let Some(s) = projeto.map(str::trim).filter(|s| !s.is_empty()) {
            c = c.projeto(s);
        }
        c
    };
    let buscar = |modo| {
        indice
            .buscar(&consulta(modo))
            .map_err(|e| format!("busca no índice da base: {e}"))
    };
    let mut qualquer = false;
    let mut pagina = buscar(Modo::Todas)?;
    if pagina.total == 0 && texto.split_whitespace().nth(1).is_some() {
        qualquer = true;
        pagina = buscar(Modo::Qualquer)?;
    }
    Ok(Busca {
        resultados: pagina.resultados.iter().map(Encontrado::from).collect(),
        total: pagina.total,
        deslocamento,
        qualquer_palavra: qualquer,
    })
}

// ---------------------------------------------------------------------------
// Documento

/// Campo do frontmatter mostrado como texto.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Campo {
    pub chave: String,
    pub valor: String,
}

/// Ligação do documento com outro.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LigacaoVista {
    /// O outro documento, relativo à raiz da base.
    pub caminho: String,
    /// Título do outro documento, se ele estiver no índice.
    pub titulo: Option<String>,
    /// `origem` ou `analise`.
    pub campo: String,
    /// `true`: este documento cita o outro (a regra cita a análise de origem);
    /// `false`: o outro cita este (a análise é a origem da regra).
    pub sai: bool,
    pub existe: bool,
}

/// Análise que o documento representa, para abrir na tela da análise.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AnaliseDoDocumento {
    pub projeto: String,
    pub id: String,
}

/// Documento aberto.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentoAberto {
    /// Relativo à raiz da base.
    pub caminho: String,
    pub projeto: Option<String>,
    pub tipo: Option<String>,
    pub titulo: String,
    pub status: Option<String>,
    /// Campos simples do frontmatter, na ordem do arquivo.
    pub campos: Vec<Campo>,
    /// Corpo em Markdown, sem o frontmatter.
    pub markdown: String,
    pub ligacoes: Vec<LigacaoVista>,
    pub analise: Option<AnaliseDoDocumento>,
    /// Frontmatter ilegível (o corpo aparece mesmo assim).
    pub problema: Option<String>,
}

fn como_texto(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(if *b { "sim" } else { "não" }.to_owned()),
        Value::Sequence(itens) => {
            let partes: Vec<String> = itens.iter().filter_map(como_texto).collect();
            (partes.len() == itens.len() && !partes.is_empty()).then(|| partes.join(", "))
        }
        _ => None,
    }
}

/// Texto do frontmatter e corpo, sem validar os campos: um `---` inicial até o
/// próximo `---` numa linha só.
fn separar(texto: &str) -> (Option<&str>, &str) {
    let sem_bom = texto.strip_prefix('\u{feff}').unwrap_or(texto);
    let Some(resto) = sem_bom
        .strip_prefix("---\n")
        .or_else(|| sem_bom.strip_prefix("---\r\n"))
    else {
        return (None, sem_bom);
    };
    let mut inicio = 0;
    for linha in resto.split_inclusive('\n') {
        if linha.trim_end() == "---" {
            return (Some(&resto[..inicio]), &resto[inicio + linha.len()..]);
        }
        inicio += linha.len();
    }
    (None, sem_bom)
}

/// Lê um documento `.md` da base, com as ligações do índice.
pub fn abrir_documento(dados: &DiretorioDados, caminho: &str) -> Result<DocumentoAberto, String> {
    let base = base_configurada(dados)?;
    let rel = CaminhoRelativo::novo(caminho).map_err(|e| format!("`{caminho}`: {e}"))?;
    if !rel.as_str().ends_with(&format!(".{}", layout::EXTENSAO)) {
        return Err(format!("`{rel}` não é um documento Markdown da base"));
    }
    if rel.partes().next() == Some(layout::TEMPLATES) {
        return Err(format!("`{rel}` é um template, não um documento da base"));
    }
    let disco = base.caminho(&rel);
    let raiz = dunce::canonicalize(base.raiz()).map_err(|e| format!("pasta da base: {e}"))?;
    let real = dunce::canonicalize(&disco).map_err(|_| format!("`{rel}` não existe na base"))?;
    if !real.starts_with(&raiz) {
        return Err(format!("`{rel}` aponta para fora da base"));
    }
    let texto = base.ler_texto(&rel).map_err(|e| e.to_string())?;

    let (yaml, corpo) = separar(&texto);
    let mut problema = None;
    let mut campos = Vec::new();
    let mut fm = FrontmatterLivre::default();
    if yaml.is_some() {
        match Documento::<FrontmatterLivre>::analisar(&texto) {
            Ok(d) => fm = d.frontmatter,
            Err(e) => problema = Some(e.to_string()),
        }
    }
    for (k, v) in &fm.0 {
        if let (Some(k), Some(v)) = (k.as_str(), como_texto(v)) {
            campos.push(Campo {
                chave: k.to_owned(),
                valor: v,
            });
        }
    }
    let campo = |nome: &str| {
        campos
            .iter()
            .find(|c| c.chave == nome)
            .map(|c| c.valor.clone())
    };
    let tipo = fm.tipo();
    let projeto = rel
        .partes()
        .next()
        .filter(|_| rel.partes().nth(1).is_some())
        .map(str::to_owned);
    let titulo = campo("titulo")
        .or_else(|| campo("termo"))
        .or_else(|| campo("nome"))
        .unwrap_or_else(|| {
            if rel.nome() == layout::ARQUIVO_REGRAS_DO_PROJETO {
                "Regras do projeto".to_owned()
            } else {
                rel.nome().trim_end_matches(".md").to_owned()
            }
        });
    let analise = match (tipo, &projeto, campo("id")) {
        (Some(t), Some(s), Some(id)) if Tipo::ANALISES.contains(&Tipo::Documento(t)) => {
            Some(AnaliseDoDocumento {
                projeto: s.clone(),
                id,
            })
        }
        _ => None,
    };

    let ligacoes = ligacoes(dados, &base, &rel);

    Ok(DocumentoAberto {
        caminho: rel.as_str().to_owned(),
        projeto,
        tipo: tipo.map(|t| t.como_str().to_owned()),
        status: campo("status"),
        titulo,
        campos,
        markdown: corpo.to_owned(),
        ligacoes,
        analise,
        problema,
    })
}

/// Ligações do índice, com o título do outro documento. Falha do índice só deixa a
/// lista vazia: o documento abre mesmo assim.
fn ligacoes(dados: &DiretorioDados, base: &Base, rel: &CaminhoRelativo) -> Vec<LigacaoVista> {
    let Ok(indice) = indice(dados, base) else {
        return Vec::new();
    };
    let Ok(lista) = indice.ligacoes(rel) else {
        return Vec::new();
    };
    let mut titulos: HashMap<String, Option<String>> = HashMap::new();
    let mut titulo = |caminho: &CaminhoRelativo| {
        titulos
            .entry(caminho.as_str().to_owned())
            .or_insert_with(|| titulo_do_documento(base, caminho))
            .clone()
    };
    lista
        .into_iter()
        .map(|l| {
            let sai = l.de == *rel;
            let outro = if sai { &l.para } else { &l.de };
            LigacaoVista {
                caminho: outro.as_str().to_owned(),
                titulo: titulo(outro),
                campo: l.campo,
                sai,
                existe: if sai { l.existe } else { true },
            }
        })
        .collect()
}

/// Título de outro documento da base, pelo frontmatter (`titulo` ou `termo`).
fn titulo_do_documento(base: &Base, caminho: &CaminhoRelativo) -> Option<String> {
    let texto = base.ler_texto(caminho).ok()?;
    let d = Documento::<FrontmatterLivre>::analisar(&texto).ok()?;
    ["titulo", "termo"]
        .iter()
        .find_map(|k| d.frontmatter.0.get(*k).and_then(como_texto))
}

// ---------------------------------------------------------------------------
// Vigia da base

type Assinante = Box<dyn Fn(&Sincronizacao) + Send + Sync>;

/// A vigia do índice da base, ligada à base da configuração, e as telas que querem
/// saber quando a base mudou fora do app.
#[derive(Default)]
pub struct VigiaBase {
    vigia: Mutex<Option<(PathBuf, Vigia)>>,
    assinantes: Arc<Mutex<HashMap<u64, Assinante>>>,
    proximo: AtomicU64,
}

impl VigiaBase {
    /// Inicia a vigia da base configurada (ou troca, se a base mudou). Sem base, não
    /// faz nada.
    pub fn garantir(&self, dados: &DiretorioDados, intervalo: std::time::Duration) {
        let Ok(base) = base_configurada(dados) else {
            return;
        };
        let raiz = base.raiz().to_path_buf();
        let mut atual = self.vigia.lock().unwrap_or_else(|e| e.into_inner());
        if atual.as_ref().is_some_and(|(r, _)| *r == raiz) {
            return;
        }
        // A anterior para ao ser descartada.
        *atual = None;
        let assinantes = Arc::clone(&self.assinantes);
        match Vigia::iniciar(dados.arquivo_do_indice(), &base, intervalo, move |s| {
            let lista = assinantes.lock().unwrap_or_else(|e| e.into_inner());
            for f in lista.values() {
                f(s);
            }
        }) {
            Ok(v) => *atual = Some((raiz, v)),
            Err(e) => tracing::warn!(erro = %e, "vigia da base não iniciada"),
        }
    }

    /// Para a vigia (espera a rodada em andamento). Os assinantes continuam; a
    /// próxima [`VigiaBase::garantir`] volta a vigiar a base configurada.
    pub fn parar(&self) {
        *self.vigia.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }

    /// Passa a receber os avisos de mudança na base. Devolve o identificador.
    pub fn assinar(&self, f: impl Fn(&Sincronizacao) + Send + Sync + 'static) -> u64 {
        let id = self.proximo.fetch_add(1, Ordering::Relaxed) + 1;
        self.assinantes
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(id, Box::new(f));
        id
    }

    pub fn cancelar(&self, id: u64) {
        self.assinantes
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&id);
    }
}

// ---------------------------------------------------------------------------
// Comandos

fn dados() -> Result<DiretorioDados, String> {
    DiretorioDados::padrao().map_err(|e| format!("diretório de dados do app inacessível: {e}"))
}

async fn bloqueante<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| format!("falha interna: {e}"))?
}

#[tauri::command]
pub async fn buscar_base(
    texto: String,
    projeto: Option<String>,
    tipos: Option<Vec<String>>,
    deslocamento: Option<usize>,
) -> Result<Busca, String> {
    let dados = dados()?;
    bloqueante(move || {
        buscar(
            &dados,
            &texto,
            projeto.as_deref(),
            &tipos.unwrap_or_default(),
            deslocamento.unwrap_or(0),
        )
    })
    .await
}

#[tauri::command]
pub async fn abrir_documento_base(caminho: String) -> Result<DocumentoAberto, String> {
    let dados = dados()?;
    bloqueante(move || abrir_documento(&dados, &caminho)).await
}

/// Avisa a tela quando a base mudou fora do app (a vigia sincronizou algo).
#[tauri::command]
pub async fn vigiar_base(
    canal: Channel<Sincronizacao>,
    estado: State<'_, Arc<VigiaBase>>,
) -> Result<u64, String> {
    let dados = dados()?;
    let vigia = Arc::clone(&estado);
    bloqueante(move || {
        vigia.garantir(&dados, Vigia::INTERVALO);
        Ok(vigia.assinar(move |s| {
            let _ = canal.send(*s);
        }))
    })
    .await
}

#[tauri::command]
pub fn parar_vigia_base(id: u64, estado: State<'_, Arc<VigiaBase>>) {
    estado.cancelar(id);
}

#[cfg(test)]
#[path = "base_testes.rs"]
mod tests;
