//! Revalidação das evidências por hash (spec, seção 6.5, e critério de aceite 8).
//!
//! Uma base que envelhece em silêncio vira mentira com cara de fato. Ao abrir um
//! projeto, e sob demanda, o app chama [`revalidar_projeto`] (ou [`revalidar`]),
//! que confere cada evidência das regras e das análises do projeto contra o código
//! atual:
//!
//! - o hash é o do intervalo citado ([`hash_das_linhas`]: SHA-256 das linhas
//!   normalizadas), o mesmo calculado no registro; mudar a indentação ou o fim de
//!   linha não desatualiza nada;
//! - evidência que não confere (trecho mudou, intervalo passou do fim do arquivo,
//!   arquivo removido, excluído ou ilegível) fica **desatualizada**: na análise, a
//!   evidência e a afirmação que depende dela ficam a revalidar
//!   ([`cba_core::analise::Analise::revalidar`]); na regra, o `status` do
//!   frontmatter passa a `a-revalidar`;
//! - quando o hash volta a conferir, a marca é desfeita: a evidência da análise
//!   volta a válida, a afirmação ao estado anterior e a regra a `confirmada`;
//! - o [`Relatorio`] diz o que mudou, com o commit gravado na evidência e o atual.
//!
//! As regras `obsoleta` ficam de fora: já não sustentam nada. A marca de uma regra
//! não é guardada por evidência (o frontmatter da regra não tem esse campo): a regra
//! fica `a-revalidar` e o relatório lista as evidências que não conferem.
//!
//! # Desempenho
//!
//! As evidências são agrupadas por arquivo, e cada arquivo é lido no máximo uma vez.
//! Com o [`Inventario`] do projeto, nem isso: o [`Cache`] guarda, por arquivo, o
//! SHA-256 do conteúdo e os hashes dos intervalos já calculados. Se o arquivo no
//! disco tem o tamanho e a data de modificação do inventário, e o SHA-256 do
//! inventário é o do cache, os hashes do cache valem e o arquivo não é lido. Um
//! inventário velho não engana a conferência: tamanho ou data diferentes fazem ler
//! o arquivo.
//!
//! # Concorrência
//!
//! Os hashes são calculados fora da trava. A análise é alterada sob a trava dela
//! ([`cba_core::analises::Analises::alterar`]), relida na hora; evidência registrada
//! no meio do caminho fica como está até a próxima revalidação. A regra é relida
//! antes de gravar, e só o `status` muda.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Instant, UNIX_EPOCH};

use cba_core::ambiente::Ambiente;
use cba_core::analise::{Analise, EstadoEvidencia, Evidencia};
use cba_core::dados::DiretorioDados;
use cba_core::eventos::Mudanca;
use cba_index::{Cadastro, Escopo, Inventario, detectar_git};
use cba_kb::frontmatter::{FrontmatterRegra, Linhas, StatusRegra};
use cba_kb::{CaminhoRelativo, Documento, Id, layout};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::evidencias::{LIMITE_DO_ARQUIVO, Motivo, decodificar, hash_das_linhas};

/// Nome do arquivo do cache, na pasta do projeto no cadastro
/// (`<dados>/projetos/<id>/`), ao lado do inventário.
pub const ARQUIVO_DO_CACHE: &str = "revalidacao.json";

/// Versão do formato do cache. Outra versão é descartada.
const VERSAO_DO_CACHE: u32 = 1;

/// Erro que impede a revalidação. Problemas num documento só (uma regra ilegível)
/// não interrompem: vão para [`Relatorio::problemas`].
#[derive(Debug, thiserror::Error)]
pub enum Erro {
    #[error(transparent)]
    Analises(#[from] cba_core::analises::Erro),
    #[error(transparent)]
    Base(#[from] cba_kb::Erro),
    #[error(transparent)]
    Cadastro(#[from] cba_index::Erro),
}

/// Hashes já calculados, por arquivo do projeto. Fica no diretório de dados do app,
/// não na base: perdê-lo só faz a próxima revalidação ler os arquivos.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cache {
    versao: u32,
    arquivos: BTreeMap<String, ArquivoEmCache>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ArquivoEmCache {
    /// SHA-256 do conteúdo do arquivo quando os hashes foram calculados.
    sha256: String,
    /// Hash de cada intervalo (`10-12`).
    intervalos: BTreeMap<String, String>,
}

impl Cache {
    /// Lê o cache; ausente, ilegível ou de outra versão vira um cache vazio.
    pub fn ler(caminho: &Path) -> Self {
        fs::read(caminho)
            .ok()
            .and_then(|b| serde_json::from_slice::<Cache>(&b).ok())
            .filter(|c| c.versao == VERSAO_DO_CACHE)
            .unwrap_or_default()
    }

    /// Grava o cache (atômico).
    pub fn gravar(&self, caminho: &Path) -> io::Result<()> {
        let json = serde_json::to_vec(&Cache {
            versao: VERSAO_DO_CACHE,
            arquivos: self.arquivos.clone(),
        })
        .map_err(io::Error::other)?;
        cba_kb::arquivo::gravar(caminho, &json)
    }

    /// Arquivos no cache.
    pub fn len(&self) -> usize {
        self.arquivos.len()
    }

    pub fn is_empty(&self) -> bool {
        self.arquivos.is_empty()
    }
}

/// Onde está a evidência.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "tipo", rename_all = "kebab-case")]
pub enum Origem {
    /// Evidência de uma regra (`regras/RN-COB-001.md`).
    Regra {
        id: String,
        /// Documento da regra, relativo à raiz do projeto na base.
        documento: CaminhoRelativo,
    },
    /// Evidência de uma afirmação de uma análise.
    Afirmacao { analise: Id, numero: u32 },
    /// Evidência de um termo proposto numa análise.
    Termo { analise: Id, numero: u32 },
    /// Trecho atual de uma sugestão registrada numa análise.
    Sugestao { analise: Id, numero: u32 },
}

/// Evidência no relatório.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EvidenciaNoRelatorio {
    pub origem: Origem,
    /// Caminho relativo à raiz do projeto.
    pub arquivo: CaminhoRelativo,
    pub linhas: Linhas,
    /// Por que não confere (para as desatualizadas).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub motivo: Option<String>,
    /// Commit gravado na evidência.
    pub commit_antigo: Option<String>,
    /// Commit atual da pasta do arquivo.
    pub commit_atual: Option<String>,
    /// Mudou nesta revalidação (deixou de conferir ou voltou a conferir agora).
    pub nova: bool,
}

/// Regra cujo `status` mudou.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MudancaDeRegra {
    pub id: String,
    pub documento: CaminhoRelativo,
    pub titulo: String,
    pub antes: StatusRegra,
    pub depois: StatusRegra,
}

/// Análise alterada pela revalidação.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MudancaDeAnalise {
    pub id: Id,
    pub titulo: String,
    /// Afirmações que passaram a a revalidar.
    pub afirmacoes_marcadas: Vec<u32>,
    /// Afirmações que voltaram ao estado anterior.
    pub afirmacoes_restauradas: Vec<u32>,
}

/// Documento que não pôde ser revalidado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Problema {
    /// Documento, relativo à raiz do projeto na base.
    pub documento: CaminhoRelativo,
    pub erro: String,
}

/// Resultado de uma revalidação: o que mudou e o que continua desatualizado.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Relatorio {
    /// Slug do projeto.
    pub projeto: String,
    /// Evidências conferidas (regras não obsoletas e análises).
    pub evidencias: usize,
    /// Arquivos do projeto lidos.
    pub arquivos_lidos: usize,
    /// Arquivos não lidos, com os hashes do cache (inventário sem mudança).
    pub arquivos_reaproveitados: usize,
    pub duracao_ms: u64,
    /// Todas as evidências que não conferem, as novas marcadas com `nova`.
    pub desatualizadas: Vec<EvidenciaNoRelatorio>,
    /// Evidências de análises que voltaram a conferir nesta revalidação.
    pub restauradas: Vec<EvidenciaNoRelatorio>,
    /// Regras cujo `status` mudou.
    pub regras: Vec<MudancaDeRegra>,
    /// Análises alteradas.
    pub analises: Vec<MudancaDeAnalise>,
    pub problemas: Vec<Problema>,
}

impl Relatorio {
    /// A revalidação mudou alguma regra ou análise.
    pub fn mudou(&self) -> bool {
        !self.regras.is_empty()
            || !self.analises.is_empty()
            || !self.restauradas.is_empty()
            || self.desatualizadas.iter().any(|e| e.nova)
    }
}

/// Caminho do cache do projeto no diretório de dados.
pub fn caminho_do_cache(dados: &DiretorioDados, projeto: &str) -> PathBuf {
    dados.projetos().join(projeto).join(ARQUIVO_DO_CACHE)
}

/// Revalida o projeto do ambiente com o último inventário gravado e o cache do
/// diretório de dados, e grava o cache. É o que o app chama ao abrir o projeto e
/// sob demanda. Sem inventário, lê todos os arquivos citados.
pub fn revalidar_projeto(ambiente: &Ambiente, dados: &DiretorioDados) -> Result<Relatorio, Erro> {
    let id = &ambiente.projeto.id;
    let inventario = Cadastro::em(dados.projetos()).inventario(id)?;
    let caminho = caminho_do_cache(dados, id);
    let mut cache = Cache::ler(&caminho);
    let relatorio = revalidar(ambiente, inventario.as_ref(), &mut cache)?;
    if let Err(e) = cache.gravar(&caminho) {
        // Só custa ler os arquivos de novo na próxima vez.
        tracing::warn!(erro = %e.kind(), "cache da revalidação não gravado");
    }
    Ok(relatorio)
}

/// Revalida as evidências das regras e das análises do projeto do ambiente.
pub fn revalidar(
    ambiente: &Ambiente,
    inventario: Option<&Inventario>,
    cache: &mut Cache,
) -> Result<Relatorio, Erro> {
    let inicio = Instant::now();
    let mut relatorio = Relatorio {
        projeto: ambiente.kb.nome().to_owned(),
        ..Relatorio::default()
    };

    let regras = ler_regras(&ambiente.kb, &mut relatorio.problemas)?;
    let analises = ambiente.analises.listar()?;

    // Intervalos citados, por arquivo.
    let mut citados: BTreeMap<String, BTreeSet<(u32, u32)>> = BTreeMap::new();
    let mut citar = |arquivo: &CaminhoRelativo, l: Linhas| {
        citados
            .entry(arquivo.as_str().to_owned())
            .or_default()
            .insert((l.inicio, l.fim));
    };
    for (_, regra) in &regras {
        for e in &regra.frontmatter.evidencias {
            citar(&e.arquivo, e.linhas);
            relatorio.evidencias += 1;
        }
    }
    for a in &analises {
        for e in evidencias_da_analise(a) {
            citar(&e.1.arquivo, e.1.linhas);
            relatorio.evidencias += 1;
        }
    }

    let mut conferidor = Conferidor::novo(&ambiente.escopo, inventario, cache);
    for (arquivo, intervalos) in &citados {
        conferidor.conferir(arquivo, intervalos);
    }
    relatorio.arquivos_lidos = conferidor.lidos;
    relatorio.arquivos_reaproveitados = conferidor.reaproveitados;
    let atual = conferidor.atual;
    let mut commits = Commits::novo(&ambiente.escopo);

    let situacao = |arquivo: &CaminhoRelativo, linhas: Linhas| {
        atual.get(&(arquivo.as_str().to_owned(), (linhas.inicio, linhas.fim)))
    };
    let confere = |e: &Evidencia| {
        situacao(&e.arquivo, e.linhas).map(|s| s.as_ref().is_ok_and(|h| *h == e.hash))
    };
    let motivo = |arquivo: &CaminhoRelativo, linhas: Linhas, hash: &str| {
        Some(match situacao(arquivo, linhas)? {
            Ok(h) if h == hash => return None,
            Ok(_) => "o trecho citado mudou".to_owned(),
            Err(m) => m.clone(),
        })
    };

    // Regras.
    for (documento, regra) in regras {
        let fm = &regra.frontmatter;
        let mut nao_conferem = Vec::new();
        for e in &fm.evidencias {
            if let Some(m) = motivo(&e.arquivo, e.linhas, &e.hash) {
                nao_conferem.push((e, m));
            }
        }
        let depois = match (fm.status, nao_conferem.is_empty()) {
            (StatusRegra::Confirmada, false) => StatusRegra::ARevalidar,
            (StatusRegra::ARevalidar, true) if !fm.evidencias.is_empty() => StatusRegra::Confirmada,
            (s, _) => s,
        };
        let nova = depois == StatusRegra::ARevalidar && fm.status != depois;
        for (e, m) in nao_conferem {
            relatorio.desatualizadas.push(EvidenciaNoRelatorio {
                origem: Origem::Regra {
                    id: fm.id.to_string(),
                    documento: documento.clone(),
                },
                arquivo: e.arquivo.clone(),
                linhas: e.linhas,
                motivo: Some(m),
                commit_antigo: e.commit.clone(),
                commit_atual: commits.de(&e.arquivo),
                nova,
            });
        }
        if depois != fm.status {
            match gravar_status(&ambiente.kb, &documento, fm.status, depois) {
                Ok(true) => relatorio.regras.push(MudancaDeRegra {
                    id: fm.id.to_string(),
                    documento: documento.clone(),
                    titulo: fm.titulo.clone(),
                    antes: fm.status,
                    depois,
                }),
                // Mudou no meio do caminho (o analista ou a aprovação gravaram).
                Ok(false) => {}
                Err(e) => relatorio.problemas.push(Problema {
                    documento,
                    erro: e.to_string(),
                }),
            }
        }
    }

    // Análises.
    for lida in analises {
        let mut previa = lida.clone();
        let (analise, estados_antes) = if previa.revalidar(confere).mudou() {
            let (antes, gravada) =
                ambiente
                    .analises
                    .alterar(&lida.id, Mudanca::Afirmacao, |a, _| {
                        let antes = a.clone();
                        a.revalidar(confere);
                        Ok(antes)
                    })?;
            let mut mudanca = MudancaDeAnalise {
                id: gravada.id.clone(),
                titulo: gravada.titulo.clone(),
                afirmacoes_marcadas: Vec::new(),
                afirmacoes_restauradas: Vec::new(),
            };
            for (de, para) in antes.afirmacoes.iter().zip(&gravada.afirmacoes) {
                if de.estado != para.estado {
                    if para.antes_de_revalidar.is_some() {
                        mudanca.afirmacoes_marcadas.push(para.numero);
                    } else {
                        mudanca.afirmacoes_restauradas.push(para.numero);
                    }
                }
            }
            let estados_antes: HashMap<_, _> = evidencias_da_analise(&antes)
                .map(|(o, e)| (chave(&o, e), e.estado))
                .collect();
            for (origem, e) in evidencias_da_analise(&gravada) {
                let anterior = estados_antes.get(&chave(&origem, e)).copied();
                if e.estado == EstadoEvidencia::Valida
                    && anterior == Some(EstadoEvidencia::ARevalidar)
                {
                    relatorio.restauradas.push(EvidenciaNoRelatorio {
                        origem,
                        arquivo: e.arquivo.clone(),
                        linhas: e.linhas,
                        motivo: None,
                        commit_antigo: e.commit.clone(),
                        commit_atual: commits.de(&e.arquivo),
                        nova: true,
                    });
                }
            }
            relatorio.analises.push(mudanca);
            (gravada, Some(estados_antes))
        } else {
            (lida, None)
        };
        for (origem, e) in evidencias_da_analise(&analise) {
            if e.estado != EstadoEvidencia::ARevalidar {
                continue;
            }
            let nova = estados_antes
                .as_ref()
                .and_then(|m| m.get(&chave(&origem, e)))
                .is_some_and(|s| *s == EstadoEvidencia::Valida);
            relatorio.desatualizadas.push(EvidenciaNoRelatorio {
                motivo: motivo(&e.arquivo, e.linhas, &e.hash)
                    .or_else(|| Some("o trecho citado mudou".to_owned())),
                origem,
                arquivo: e.arquivo.clone(),
                linhas: e.linhas,
                commit_antigo: e.commit.clone(),
                commit_atual: commits.de(&e.arquivo),
                nova,
            });
        }
    }

    relatorio.duracao_ms = u64::try_from(inicio.elapsed().as_millis()).unwrap_or(u64::MAX);
    tracing::info!(
        evidencias = relatorio.evidencias,
        desatualizadas = relatorio.desatualizadas.len(),
        regras = relatorio.regras.len(),
        analises = relatorio.analises.len(),
        lidos = relatorio.arquivos_lidos,
        reaproveitados = relatorio.arquivos_reaproveitados,
        duracao_ms = relatorio.duracao_ms,
        "revalidação das evidências"
    );
    Ok(relatorio)
}

/// Chave de uma evidência dentro da análise, para comparar antes e depois.
fn chave(origem: &Origem, e: &Evidencia) -> (String, String, u32, u32, String) {
    let origem = match origem {
        Origem::Regra { id, .. } => format!("regra {id}"),
        Origem::Afirmacao { numero, .. } => format!("afirmacao {numero}"),
        Origem::Termo { numero, .. } => format!("termo {numero}"),
        Origem::Sugestao { numero, .. } => format!("sugestao {numero}"),
    };
    (
        origem,
        e.arquivo.as_str().to_owned(),
        e.linhas.inicio,
        e.linhas.fim,
        e.hash.clone(),
    )
}

/// Evidências de uma análise com a origem: afirmações, termos e sugestões.
fn evidencias_da_analise(a: &Analise) -> impl Iterator<Item = (Origem, &Evidencia)> {
    let id = &a.id;
    let afirmacoes = a.afirmacoes.iter().flat_map(move |af| {
        af.evidencias.iter().map(move |e| {
            (
                Origem::Afirmacao {
                    analise: id.clone(),
                    numero: af.numero,
                },
                e,
            )
        })
    });
    let termos = a.termos.iter().flat_map(move |t| {
        t.evidencias.iter().map(move |e| {
            (
                Origem::Termo {
                    analise: id.clone(),
                    numero: t.numero,
                },
                e,
            )
        })
    });
    let sugestoes = a.sugestoes.iter().filter_map(move |s| {
        s.evidencia.as_ref().map(|e| {
            (
                Origem::Sugestao {
                    analise: id.clone(),
                    numero: s.numero,
                },
                e,
            )
        })
    });
    afirmacoes.chain(termos).chain(sugestoes)
}

/// Regras do projeto (`regras/*.md`), menos as obsoletas. As ilegíveis vão para os
/// problemas.
fn ler_regras(
    kb: &cba_kb::Projeto,
    problemas: &mut Vec<Problema>,
) -> Result<Vec<(CaminhoRelativo, Documento<FrontmatterRegra>)>, Erro> {
    let pasta = CaminhoRelativo::novo(layout::REGRAS).expect("nome fixo válido");
    let entradas = match fs::read_dir(kb.caminho(&pasta)) {
        Ok(e) => e,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(erro) => {
            return Err(Erro::Base(cba_kb::Erro::Io {
                arquivo: kb.na_base(&pasta),
                erro,
            }));
        }
    };
    let mut nomes: Vec<String> = entradas
        .flatten()
        .filter_map(|e| e.file_name().to_str().map(str::to_owned))
        .filter(|n| !n.starts_with('.') && n.ends_with(&format!(".{}", layout::EXTENSAO)))
        .collect();
    nomes.sort();
    let mut regras = Vec::new();
    for nome in nomes {
        let Ok(relativo) = pasta.juntar(&nome) else {
            continue;
        };
        match kb.ler::<FrontmatterRegra>(&relativo) {
            Ok(doc) if doc.frontmatter.status != StatusRegra::Obsoleta => {
                regras.push((relativo, doc));
            }
            Ok(_) => {}
            Err(e) => problemas.push(Problema {
                documento: relativo,
                erro: e.to_string(),
            }),
        }
    }
    Ok(regras)
}

/// Troca o `status` da regra, se ele ainda for `antes`. Devolve se gravou.
fn gravar_status(
    kb: &cba_kb::Projeto,
    documento: &CaminhoRelativo,
    antes: StatusRegra,
    depois: StatusRegra,
) -> Result<bool, cba_kb::Erro> {
    let mut doc: Documento<FrontmatterRegra> = kb.ler(documento)?;
    if doc.frontmatter.status != antes {
        return Ok(false);
    }
    doc.frontmatter.status = depois;
    kb.gravar(documento, &doc)?;
    Ok(true)
}

/// Situação atual de um intervalo: o hash, ou por que não há hash.
type Situacao = Result<String, String>;

/// Calcula o hash atual dos intervalos citados, arquivo a arquivo.
struct Conferidor<'a> {
    escopo: &'a Escopo,
    /// O inventário só vale se for das mesmas pastas do escopo (numa worktree, o
    /// escopo aponta para outros arquivos).
    inventario: Option<&'a Inventario>,
    cache: &'a mut Cache,
    atual: HashMap<(String, (u32, u32)), Situacao>,
    lidos: usize,
    reaproveitados: usize,
}

impl<'a> Conferidor<'a> {
    fn novo(escopo: &'a Escopo, inventario: Option<&'a Inventario>, cache: &'a mut Cache) -> Self {
        let mesmas_pastas = |i: &&Inventario| {
            i.pastas.len() == escopo.pastas().len()
                && i.pastas
                    .iter()
                    .zip(escopo.pastas())
                    .all(|(a, b)| a.caminho == b.caminho && a.prefixo == b.prefixo)
        };
        Self {
            escopo,
            inventario: inventario.filter(mesmas_pastas),
            cache,
            atual: HashMap::new(),
            lidos: 0,
            reaproveitados: 0,
        }
    }

    fn conferir(&mut self, arquivo: &str, intervalos: &BTreeSet<(u32, u32)>) {
        let resultado = self.hashes(arquivo, intervalos);
        for &(i, f) in intervalos {
            let situacao = match &resultado {
                Ok(hashes) => hashes
                    .get(&(i, f))
                    .cloned()
                    .unwrap_or_else(|| Err("o intervalo passa do fim do arquivo".to_owned())),
                Err(m) => Err(m.to_string()),
            };
            self.atual.insert((arquivo.to_owned(), (i, f)), situacao);
        }
    }

    /// Hash de cada intervalo do arquivo (ausente: passa do fim do arquivo).
    fn hashes(
        &mut self,
        arquivo: &str,
        intervalos: &BTreeSet<(u32, u32)>,
    ) -> Result<HashMap<(u32, u32), Situacao>, Motivo> {
        let absoluto = self.escopo.resolver(arquivo).map_err(Motivo::Caminho)?;
        if let Some(hashes) = self.do_cache(arquivo, &absoluto, intervalos) {
            self.reaproveitados += 1;
            return Ok(hashes);
        }
        let ilegivel = |e: io::Error| Motivo::Ilegivel(e.kind().to_string());
        let bytes = fs::metadata(&absoluto).map_err(ilegivel)?.len();
        if bytes > LIMITE_DO_ARQUIVO {
            return Err(Motivo::Grande { bytes });
        }
        let conteudo = fs::read(&absoluto).map_err(ilegivel)?;
        self.lidos += 1;
        let texto = decodificar(&conteudo)?;
        let sha256 = hex(&Sha256::digest(&conteudo));
        let mut saida = HashMap::new();
        let entrada = self
            .cache
            .arquivos
            .entry(arquivo.to_owned())
            .or_insert_with(|| ArquivoEmCache {
                sha256: sha256.clone(),
                intervalos: BTreeMap::new(),
            });
        if entrada.sha256 != sha256 {
            entrada.sha256 = sha256;
            entrada.intervalos.clear();
        }
        for &(i, f) in intervalos {
            let Some(linhas) = Linhas::novo(i, f) else {
                continue;
            };
            if let Some(h) = hash_das_linhas(&texto, linhas) {
                entrada.intervalos.insert(linhas.to_string(), h.clone());
                saida.insert((i, f), Ok(h));
            }
        }
        Ok(saida)
    }

    /// Hashes do cache, se o arquivo não mudou desde o inventário e o cache é do
    /// mesmo conteúdo e tem todos os intervalos.
    fn do_cache(
        &self,
        arquivo: &str,
        absoluto: &Path,
        intervalos: &BTreeSet<(u32, u32)>,
    ) -> Option<HashMap<(u32, u32), Situacao>> {
        let inventariado = self.inventario?.arquivo(arquivo)?;
        let meta = fs::metadata(absoluto).ok()?;
        let modificado = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .and_then(|d| u64::try_from(d.as_nanos()).ok())?;
        if meta.len() != inventariado.tamanho || Some(modificado) != inventariado.modificado {
            return None;
        }
        let em_cache = self.cache.arquivos.get(arquivo)?;
        if em_cache.sha256 != inventariado.hash {
            return None;
        }
        intervalos
            .iter()
            .map(|&(i, f)| {
                let chave = Linhas::novo(i, f)?.to_string();
                let h = em_cache.intervalos.get(&chave)?;
                Some(((i, f), Ok(h.clone())))
            })
            .collect()
    }
}

/// Commit atual da pasta de cada arquivo, detectado uma vez por pasta.
struct Commits<'a> {
    escopo: &'a Escopo,
    por_pasta: HashMap<PathBuf, Option<String>>,
}

impl<'a> Commits<'a> {
    fn novo(escopo: &'a Escopo) -> Self {
        Self {
            escopo,
            por_pasta: HashMap::new(),
        }
    }

    fn de(&mut self, arquivo: &CaminhoRelativo) -> Option<String> {
        let pasta = self.escopo.raiz_de(arquivo.as_str())?;
        self.por_pasta
            .entry(pasta.to_path_buf())
            .or_insert_with(|| detectar_git(pasta).and_then(|g| g.commit))
            .clone()
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
