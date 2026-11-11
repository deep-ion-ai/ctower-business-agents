//! Inventário dos arquivos de um sistema: o que será analisado e o que foi ignorado.
//!
//! Para cada arquivo analisado guarda caminho relativo, tamanho, data de
//! modificação, linguagem e hash SHA-256 do conteúdo. O inventário anterior serve
//! de cache: arquivo com o mesmo tamanho e a mesma data de modificação não é lido
//! de novo.
//!
//! A varredura das pastas é sequencial (a poda das pastas excluídas depende da
//! ordem), e a leitura dos arquivos roda em paralelo, uma thread por núcleo.

use std::collections::{BTreeMap, HashMap};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::Erro;
use crate::exclusoes::{Exclusoes, Motivo};
use crate::git::{RepositorioGit, detectar_git};
use crate::linguagem::Linguagem;
use crate::sistema::Sistema;

/// Bytes do início do arquivo usados para detectar binário e linguagem.
const INICIO: usize = 8 * 1024;

/// Arquivo que entra na análise.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArquivoInventariado {
    /// Caminho relativo à raiz do sistema, com `/`.
    pub caminho: String,
    pub tamanho: u64,
    /// Data de modificação em nanossegundos desde 1970 (UTC), quando disponível.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modificado: Option<u64>,
    pub linguagem: Linguagem,
    /// SHA-256 do conteúdo, em hexadecimal minúsculo.
    pub hash: String,
}

/// Arquivo ou pasta que ficou fora da análise.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ignorado {
    /// Caminho relativo à raiz do sistema, com `/`.
    pub caminho: String,
    /// Pasta inteira (o conteúdo não é listado).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub pasta: bool,
    pub motivo: Motivo,
    /// Padrão que excluiu, se a exclusão veio de um padrão.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub regra: Option<String>,
    /// Detalhe para diagnóstico (erro de leitura, binário pelo conteúdo).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detalhe: Option<String>,
    /// Tamanho e modificação de quem foi lido para decidir (binário pelo
    /// conteúdo), para não ler de novo no próximo inventário.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tamanho: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modificado: Option<u64>,
}

/// Pasta do sistema no momento do inventário, com o estado do git.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PastaInventariada {
    pub caminho: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefixo: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git: Option<RepositorioGit>,
}

/// Números da execução do inventário.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Estatisticas {
    /// Arquivos lidos nesta execução (novos ou alterados).
    pub processados: usize,
    /// Arquivos aproveitados do inventário anterior, sem leitura.
    pub reaproveitados: usize,
    pub duracao_ms: u64,
}

/// Inventário de um sistema.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Inventario {
    /// Identificador do sistema.
    pub sistema: String,
    /// Quando foi gerado, em segundos desde 1970 (UTC).
    pub gerado_em: u64,
    pub pastas: Vec<PastaInventariada>,
    /// Arquivos analisados, ordenados pelo caminho.
    pub arquivos: Vec<ArquivoInventariado>,
    /// Itens ignorados, ordenados pelo caminho.
    pub ignorados: Vec<Ignorado>,
    pub estatisticas: Estatisticas,
}

/// Resumo para a interface: contagem por tipo, analisado e ignorado.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resumo {
    pub analisados: usize,
    pub bytes_analisados: u64,
    pub ignorados: usize,
    pub por_linguagem: BTreeMap<Linguagem, usize>,
    pub ignorados_por_motivo: BTreeMap<Motivo, usize>,
}

impl Inventario {
    /// Arquivo analisado pelo caminho relativo.
    pub fn arquivo(&self, caminho: &str) -> Option<&ArquivoInventariado> {
        self.arquivos
            .binary_search_by(|a| a.caminho.as_str().cmp(caminho))
            .ok()
            .map(|i| &self.arquivos[i])
    }

    /// Item ignorado que cobre o caminho: o próprio arquivo ou uma pasta acima.
    pub fn ignorado(&self, caminho: &str) -> Option<&Ignorado> {
        self.ignorados.iter().find(|i| {
            i.caminho == caminho
                || (i.pasta
                    && caminho
                        .strip_prefix(i.caminho.as_str())
                        .is_some_and(|r| r.starts_with('/')))
        })
    }

    pub fn resumo(&self) -> Resumo {
        let mut r = Resumo {
            analisados: self.arquivos.len(),
            ignorados: self.ignorados.len(),
            ..Resumo::default()
        };
        for a in &self.arquivos {
            r.bytes_analisados += a.tamanho;
            *r.por_linguagem.entry(a.linguagem).or_default() += 1;
        }
        for i in &self.ignorados {
            *r.ignorados_por_motivo.entry(i.motivo).or_default() += 1;
        }
        r
    }
}

/// Candidato encontrado na varredura, a ser lido.
struct Candidato {
    relativo: String,
    caminho: PathBuf,
}

enum Resultado {
    Arquivo(ArquivoInventariado, bool),
    Ignorado(Ignorado, bool),
}

/// Inventaria o sistema. Com o inventário anterior, não lê de novo os arquivos de
/// mesmo tamanho e mesma data de modificação.
pub fn inventariar(sistema: &Sistema, anterior: Option<&Inventario>) -> Result<Inventario, Erro> {
    let inicio = Instant::now();
    let escopo = sistema.escopo()?;
    let exclusoes = escopo.exclusoes();

    let mut ignorados = Vec::new();
    let mut candidatos = Vec::new();
    let mut pastas = Vec::new();
    for p in escopo.pastas() {
        varrer(
            &p.caminho,
            p.prefixo.as_deref(),
            exclusoes,
            &mut candidatos,
            &mut ignorados,
        );
        pastas.push(PastaInventariada {
            caminho: p.caminho.clone(),
            prefixo: p.prefixo.clone(),
            git: detectar_git(&p.caminho),
        });
    }

    let cache = Cache::de(anterior);
    let resultados = processar_em_paralelo(&candidatos, &cache);

    let mut arquivos = Vec::with_capacity(resultados.len());
    let mut est = Estatisticas::default();
    for r in resultados {
        let (reaproveitado, ok) = match r {
            Resultado::Arquivo(a, re) => {
                arquivos.push(a);
                (re, true)
            }
            Resultado::Ignorado(i, re) => {
                // Só conta como processado o que foi aberto para decidir.
                let aberto = i.tamanho.is_some();
                ignorados.push(i);
                (re, aberto)
            }
        };
        if reaproveitado {
            est.reaproveitados += 1;
        } else if ok {
            est.processados += 1;
        }
    }
    arquivos.sort_by(|a, b| a.caminho.cmp(&b.caminho));
    ignorados.sort_by(|a, b| a.caminho.cmp(&b.caminho));
    est.duracao_ms = inicio.elapsed().as_millis().try_into().unwrap_or(u64::MAX);

    Ok(Inventario {
        sistema: sistema.id.clone(),
        gerado_em: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        pastas,
        arquivos,
        ignorados,
        estatisticas: est,
    })
}

fn varrer(
    raiz: &Path,
    prefixo: Option<&str>,
    exclusoes: &Exclusoes,
    candidatos: &mut Vec<Candidato>,
    ignorados: &mut Vec<Ignorado>,
) {
    let ignorar = |caminho: String, pasta: bool, motivo, regra, detalhe| Ignorado {
        caminho,
        pasta,
        motivo,
        regra,
        detalhe,
        tamanho: None,
        modificado: None,
    };
    let mut it = WalkDir::new(raiz)
        .follow_links(false)
        .sort_by_file_name()
        .min_depth(1)
        .into_iter();
    while let Some(entrada) = it.next() {
        let entrada = match entrada {
            Ok(e) => e,
            Err(e) => {
                let caminho = e
                    .path()
                    .and_then(|p| relativo(raiz, prefixo, p))
                    .unwrap_or_else(|| prefixo.unwrap_or("").to_string());
                ignorados.push(ignorar(
                    caminho,
                    false,
                    Motivo::Ilegivel,
                    None,
                    Some(e.to_string()),
                ));
                continue;
            }
        };
        let tipo = entrada.file_type();
        let Some(rel) = relativo(raiz, prefixo, entrada.path()) else {
            let nome = entrada.path().to_string_lossy().into_owned();
            ignorados.push(ignorar(
                relativo_lossy(raiz, prefixo, entrada.path()),
                tipo.is_dir(),
                Motivo::Ilegivel,
                None,
                Some(format!("nome não é UTF-8: {nome}")),
            ));
            if tipo.is_dir() {
                it.skip_current_dir();
            }
            continue;
        };
        if tipo.is_symlink() {
            ignorados.push(ignorar(rel, false, Motivo::LinkSimbolico, None, None));
            continue;
        }
        if let Some(e) = exclusoes.avaliar(&rel, tipo.is_dir()) {
            if tipo.is_dir() {
                it.skip_current_dir();
            }
            ignorados.push(ignorar(rel, tipo.is_dir(), e.motivo, e.regra, None));
            continue;
        }
        if tipo.is_file() {
            candidatos.push(Candidato {
                relativo: rel,
                caminho: entrada.into_path(),
            });
        }
    }
}

fn relativo(raiz: &Path, prefixo: Option<&str>, caminho: &Path) -> Option<String> {
    let resto = caminho.strip_prefix(raiz).ok()?;
    let mut s = prefixo.map(str::to_string).unwrap_or_default();
    for c in resto.components() {
        if !s.is_empty() {
            s.push('/');
        }
        s.push_str(c.as_os_str().to_str()?);
    }
    Some(s)
}

fn relativo_lossy(raiz: &Path, prefixo: Option<&str>, caminho: &Path) -> String {
    let resto = caminho.strip_prefix(raiz).unwrap_or(caminho);
    let resto = resto.to_string_lossy().replace('\\', "/");
    match prefixo {
        Some(p) => format!("{p}/{resto}"),
        None => resto,
    }
}

/// Entradas do inventário anterior que podem ser reaproveitadas.
struct Cache<'a> {
    arquivos: HashMap<&'a str, &'a ArquivoInventariado>,
    binarios: HashMap<&'a str, &'a Ignorado>,
}

impl<'a> Cache<'a> {
    fn de(anterior: Option<&'a Inventario>) -> Self {
        let mut c = Cache {
            arquivos: HashMap::new(),
            binarios: HashMap::new(),
        };
        if let Some(inv) = anterior {
            c.arquivos = inv
                .arquivos
                .iter()
                .map(|a| (a.caminho.as_str(), a))
                .collect();
            c.binarios = inv
                .ignorados
                .iter()
                .filter(|i| i.tamanho.is_some() && i.modificado.is_some())
                .map(|i| (i.caminho.as_str(), i))
                .collect();
        }
        c
    }
}

fn processar_em_paralelo(candidatos: &[Candidato], cache: &Cache) -> Vec<Resultado> {
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .clamp(1, 16)
        .min(candidatos.len().max(1));
    let proximo = AtomicUsize::new(0);
    std::thread::scope(|s| {
        let tarefas: Vec<_> = (0..threads)
            .map(|_| {
                s.spawn(|| {
                    let mut saida = Vec::new();
                    let mut buf = vec![0u8; 64 * 1024];
                    loop {
                        let i = proximo.fetch_add(1, Ordering::Relaxed);
                        let Some(c) = candidatos.get(i) else { break };
                        saida.push(processar(c, cache, &mut buf));
                    }
                    saida
                })
            })
            .collect();
        tarefas
            .into_iter()
            .flat_map(|t| t.join().expect("thread do inventário"))
            .collect()
    })
}

fn processar(c: &Candidato, cache: &Cache, buf: &mut [u8]) -> Resultado {
    let ilegivel = |detalhe: String| {
        Resultado::Ignorado(
            Ignorado {
                caminho: c.relativo.clone(),
                pasta: false,
                motivo: Motivo::Ilegivel,
                regra: None,
                detalhe: Some(detalhe),
                tamanho: None,
                modificado: None,
            },
            false,
        )
    };
    let meta = match fs::metadata(&c.caminho) {
        Ok(m) => m,
        Err(e) => return ilegivel(e.to_string()),
    };
    let tamanho = meta.len();
    let modificado = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .and_then(|d| u64::try_from(d.as_nanos()).ok());

    if modificado.is_some() {
        let mesmo = |t: u64, m: Option<u64>| t == tamanho && m == modificado;
        if let Some(a) = cache.arquivos.get(c.relativo.as_str())
            && mesmo(a.tamanho, a.modificado)
        {
            return Resultado::Arquivo((*a).clone(), true);
        }
        if let Some(i) = cache.binarios.get(c.relativo.as_str())
            && i.tamanho.is_some_and(|t| mesmo(t, i.modificado))
        {
            return Resultado::Ignorado((*i).clone(), true);
        }
    }

    let mut f = match File::open(&c.caminho) {
        Ok(f) => f,
        Err(e) => return ilegivel(e.to_string()),
    };
    // Início do arquivo: binário e linguagem.
    let mut lidos = 0;
    while lidos < INICIO {
        match f.read(&mut buf[lidos..INICIO]) {
            Ok(0) => break,
            Ok(n) => lidos += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return ilegivel(e.to_string()),
        }
    }
    let inicio = &buf[..lidos];
    if eh_binario(inicio) {
        return Resultado::Ignorado(
            Ignorado {
                caminho: c.relativo.clone(),
                pasta: false,
                motivo: Motivo::Binario,
                regra: None,
                detalhe: Some("conteúdo binário".into()),
                tamanho: Some(tamanho),
                modificado,
            },
            false,
        );
    }
    let nome = c.relativo.rsplit('/').next().unwrap_or(&c.relativo);
    let linguagem = Linguagem::detectar(nome, inicio);

    let mut hasher = Sha256::new();
    hasher.update(inicio);
    loop {
        match f.read(buf) {
            Ok(0) => break,
            Ok(n) => hasher.update(&buf[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return ilegivel(e.to_string()),
        }
    }
    let hash = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();

    Resultado::Arquivo(
        ArquivoInventariado {
            caminho: c.relativo.clone(),
            tamanho,
            modificado,
            linguagem,
            hash,
        },
        false,
    )
}

/// Binário: byte nulo no início, exceto texto UTF-16 com BOM (comum em scripts
/// exportados por ferramentas do Windows).
fn eh_binario(inicio: &[u8]) -> bool {
    let utf16 = inicio.starts_with(&[0xFF, 0xFE]) || inicio.starts_with(&[0xFE, 0xFF]);
    !utf16 && inicio.contains(&0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deteccao_de_binario() {
        assert!(!eh_binario(b"select 1 from dual;"));
        assert!(eh_binario(b"MZ\x90\x00\x03"));
        assert!(!eh_binario(b"\xFF\xFEs\x00e\x00l\x00"));
        assert!(!eh_binario(b""));
    }
}
