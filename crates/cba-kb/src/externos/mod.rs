//! Documentação externa (#277): cópia, na base, dos `.md` do repositório de docs.
//!
//! O repositório de docs (o destino da publicação, #274) é sincronizado e os seus
//! `.md` entram na base como documentos do tipo [`TipoDocumento::DocExterno`], em
//! `externos/<slug do repositório>/` ([`layout::EXTERNOS`]), com a mesma estrutura
//! de pastas do original. Quem busca o repositório (fetch) e lê o commit é o
//! chamador (`cba_tools::docs_externos`); aqui fica o que acontece na base.
//!
//! # Por que copiar para a base (e não indexar direto do clone)
//!
//! - **O índice continua sendo só o espelho da base**: apagá-lo e reconstruí-lo
//!   ([`crate::indice::Indice::reconstruir`]) traz os externos de volta sem rede
//!   nem clone, e o servidor MCP, o app e a vigia enxergam os mesmos arquivos.
//! - **O conteúdo é o do commit sincronizado**, não o da pasta do clone (que pode
//!   estar noutra branch ou com edições do analista), e já sai com os segredos e
//!   dados pessoais mascarados ([`sensivel`]): o que entra no índice e no contexto
//!   do agente é a cópia tratada, nunca o original.
//! - **Remoção no repositório remove do índice**: a cada importação, as cópias que
//!   não vieram do commit são apagadas; a sincronização seguinte do índice tira os
//!   documentos que sumiram.
//!
//! # A cópia
//!
//! O frontmatter da cópia é gerado ([`FrontmatterDocExterno`]): título, origem
//! (repositório, caminho no repositório, commit e branch), confiança e, se havia,
//! o `tipo` e o `id` do original e o frontmatter original inteiro em
//! `frontmatter_original`. O corpo é o do original. Original sem frontmatter (ou
//! com YAML inválido) é aceito: o texto inteiro vira o corpo, e o título vem do
//! primeiro `# título` ou do nome do arquivo.
//!
//! # Documentos que o próprio app publicou
//!
//! A publicação (#276) grava os documentos aprovados da base com o frontmatter
//! deles. Quando um `.md` do repositório tem `tipo` conhecido e o mesmo `id` (ou
//! `termo`) de um documento desta base, ele **não** é copiado
//! ([`Importacao::ja_na_base`]): a base é a fonte da verdade e a cópia só
//! duplicaria a regra ou o termo na busca. Os publicados de outra base (outro
//! analista, outra máquina) entram com confiança [`Confianca::Publicada`]; o
//! restante, com [`Confianca::Externa`].
//!
//! # O que não entra
//!
//! Registrado em [`Importacao::pulados`] e no estado: caminhos que o chamador
//! exclui (os padrões de segredo do inventário), arquivos com chave privada,
//! maiores que [`LIMITE_DE_TAMANHO`], que não são UTF-8, em pastas ocultas ou com
//! nome que não vale no Windows.
//!
//! # Estado
//!
//! `externos/.sincronizacao.json` ([`EstadoExternos`]) guarda a última
//! sincronização (repositório, commit, quando, erro da última tentativa), o que
//! foi pulado e mascarado e, por arquivo, a versão (o blob do git) já importada:
//! a importação seguinte só lê o que mudou.

pub mod sensivel;

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::io;
use std::path::Path;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::arquivo;
use crate::base::Projeto;
use crate::caminho::CaminhoRelativo;
use crate::documento::Documento;
use crate::erro::{Erro, Resultado};
use crate::frontmatter::{
    Confianca, FrontmatterDocExterno, FrontmatterLivre, OrigemExterna, TipoDocumento,
};
use crate::layout;
use crate::yaml::{Mapping, Value};

/// Arquivo maior que isto não é importado (documentação, não anexo).
pub const LIMITE_DE_TAMANHO: u64 = 1024 * 1024;

/// Versão do formato do estado.
const FORMATO: u32 = 1;

/// Trava mais velha que isto é de uma sincronização que morreu no meio.
const TRAVA_ABANDONADA: Duration = Duration::from_secs(15 * 60);

/// O repositório e o commit importados.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fonte {
    /// Rótulo do repositório (`GitHub exemplo/faturamento-docs`); dá o nome da
    /// pasta das cópias.
    pub repositorio: String,
    /// Branch sincronizada.
    pub branch: String,
    /// Commit sincronizado.
    pub commit: String,
}

/// Um `.md` do repositório a importar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArquivoExterno {
    /// Relativo à pasta da documentação (a subpasta do destino), com `/`: é o
    /// caminho da cópia dentro de `externos/<repositório>/`.
    pub caminho: String,
    /// Relativo à raiz do repositório (vai para a origem da cópia).
    pub caminho_no_repositorio: String,
    /// Versão do conteúdo (o blob do git): igual, o arquivo não é lido de novo.
    pub versao: String,
    pub tamanho: u64,
}

/// Arquivo que não foi importado, e por quê.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pulado {
    /// Caminho no repositório.
    pub caminho: String,
    pub motivo: String,
}

/// Arquivo importado com trechos mascarados.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mascarado {
    /// Caminho no repositório.
    pub caminho: String,
    pub trechos: u32,
}

/// O que uma importação fez.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Importacao {
    /// Cópias criadas.
    pub novos: usize,
    /// Cópias regravadas (o original mudou ou a cópia foi mexida).
    pub alterados: usize,
    /// Cópias apagadas (o original saiu do repositório, foi pulado ou já está na
    /// base).
    pub removidos: usize,
    pub inalterados: usize,
    /// Caminhos no repositório de documentos que esta base já tem (publicados
    /// por ela): não copiados.
    pub ja_na_base: Vec<String>,
    pub pulados: Vec<Pulado>,
    pub mascarados: Vec<Mascarado>,
}

impl Importacao {
    /// Se alguma cópia mudou.
    pub fn mudou(&self) -> bool {
        self.novos + self.alterados + self.removidos > 0
    }
}

/// O que o estado guarda de um arquivo do repositório.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Registro {
    versao: String,
    /// `tipo` conhecido e `id` (ou `termo`) do original, para reconhecer o que esta
    /// base publicou sem ler de novo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tipo_original: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    id_original: Option<String>,
    /// Pulado pelo conteúdo (chave privada, não UTF-8): vale enquanto a versão
    /// for a mesma.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pulado: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    copia: Option<Copia>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Copia {
    /// Relativo à raiz do projeto.
    caminho: String,
    /// SHA-256 do arquivo gravado: diferente no disco, a cópia é regravada.
    hash: String,
    #[serde(default)]
    trechos_mascarados: u32,
}

/// Estado da sincronização, em `externos/.sincronizacao.json`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EstadoExternos {
    pub formato: u32,
    /// Fonte da última importação bem-sucedida.
    #[serde(default)]
    pub fonte: Option<Fonte>,
    /// Quando foi a última importação bem-sucedida (ms desde 1970, UTC).
    #[serde(default)]
    pub sincronizado_ms: Option<u64>,
    /// Quando foi a última tentativa, com sucesso ou não.
    #[serde(default)]
    pub tentativa_ms: Option<u64>,
    /// Por que a última tentativa falhou (`None` se deu certo). As cópias da
    /// importação anterior continuam valendo.
    #[serde(default)]
    pub erro: Option<String>,
    #[serde(default)]
    pub ja_na_base: Vec<String>,
    #[serde(default)]
    pub pulados: Vec<Pulado>,
    #[serde(default)]
    pub mascarados: Vec<Mascarado>,
    #[serde(default)]
    arquivos: BTreeMap<String, Registro>,
}

impl EstadoExternos {
    /// O estado do projeto; `None` se ainda não houve sincronização (ou se o
    /// arquivo está ilegível, o que a próxima sincronização corrige).
    pub fn ler(projeto: &Projeto) -> Option<Self> {
        let texto = fs::read_to_string(projeto.caminho(&layout::estado_dos_externos())).ok()?;
        serde_json::from_str(&texto).ok()
    }

    fn gravar(&self, projeto: &Projeto) -> Resultado<()> {
        let relativo = layout::estado_dos_externos();
        let json = serde_json::to_vec_pretty(self).expect("estado serializável");
        arquivo::gravar(&projeto.caminho(&relativo), &json)
            .map_err(|erro| projeto.erro_io(&relativo, erro))
    }

    /// Quantos documentos foram copiados para a base.
    pub fn importados(&self) -> usize {
        self.arquivos.values().filter(|r| r.copia.is_some()).count()
    }

    /// Por que a cópia pode estar desatualizada, ou `None` se a última tentativa
    /// deu certo há menos de `limite`.
    pub fn desatualizado(&self, agora_ms: u64, limite: Duration) -> Option<String> {
        if let Some(erro) = &self.erro {
            return Some(match self.sincronizado_ms {
                Some(_) => format!(
                    "a última sincronização do repositório de docs falhou ({erro}); valem \
                     as cópias de {}",
                    self.fonte
                        .as_ref()
                        .map(|f| format!("{} no commit {}", f.repositorio, curto(&f.commit)))
                        .unwrap_or_else(|| "antes".into())
                ),
                None => format!("o repositório de docs ainda não foi sincronizado ({erro})"),
            });
        }
        let Some(quando) = self.sincronizado_ms else {
            return Some("o repositório de docs ainda não foi sincronizado".into());
        };
        let idade = Duration::from_millis(agora_ms.saturating_sub(quando));
        (idade > limite).then(|| {
            format!(
                "a última sincronização do repositório de docs foi há {}",
                descrever_idade(idade)
            )
        })
    }
}

fn curto(commit: &str) -> &str {
    commit.get(..10).unwrap_or(commit)
}

fn descrever_idade(idade: Duration) -> String {
    let horas = idade.as_secs() / 3600;
    match horas {
        0 => format!("{} minutos", idade.as_secs() / 60),
        1 => "1 hora".into(),
        2..48 => format!("{horas} horas"),
        _ => format!("{} dias", horas / 24),
    }
}

/// Milissegundos desde 1970 (UTC).
pub fn agora_ms() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

/// Trava da sincronização (pasta `externos/.sincronizacao.trava`): uma por vez,
/// entre o app e o servidor MCP. Solta ao sair de escopo.
#[derive(Debug)]
pub struct Trava {
    pasta: std::path::PathBuf,
}

impl Drop for Trava {
    fn drop(&mut self) {
        let _ = fs::remove_dir(&self.pasta);
    }
}

/// Toma a trava da sincronização; `None` se outra sincronização está em
/// andamento. Trava esquecida por uma sincronização que morreu no meio é
/// retomada depois de 15 minutos.
pub fn travar(projeto: &Projeto) -> Resultado<Option<Trava>> {
    let externos = layout::externos();
    let raiz = projeto.caminho(&externos);
    fs::create_dir_all(&raiz).map_err(|erro| projeto.erro_io(&externos, erro))?;
    let pasta = raiz.join(layout::TRAVA_EXTERNOS);
    for _ in 0..2 {
        match fs::create_dir(&pasta) {
            Ok(()) => return Ok(Some(Trava { pasta })),
            Err(e) if arquivo::trava_ocupada(&e) => {
                let abandonada = fs::metadata(&pasta)
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|m| m.elapsed().ok())
                    .is_some_and(|idade| idade > TRAVA_ABANDONADA);
                if !abandonada {
                    return Ok(None);
                }
                let _ = fs::remove_dir(&pasta);
            }
            Err(erro) => return Err(projeto.erro_io(&externos, erro)),
        }
    }
    Ok(None)
}

/// Registra no estado uma sincronização que falhou (rede, credencial, clone),
/// sem mexer nas cópias.
pub fn registrar_falha(projeto: &Projeto, erro: &str, agora_ms: u64) -> Resultado<()> {
    let mut estado = EstadoExternos::ler(projeto).unwrap_or_default();
    estado.formato = FORMATO;
    estado.tentativa_ms = Some(agora_ms);
    estado.erro = Some(erro.trim().to_string());
    estado.gravar(projeto)
}

/// Apaga a pasta `externos/` inteira (o destino da documentação foi removido).
/// Devolve se havia algo.
pub fn remover_tudo(projeto: &Projeto) -> Resultado<bool> {
    let externos = layout::externos();
    match fs::remove_dir_all(projeto.caminho(&externos)) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(erro) => Err(projeto.erro_io(&externos, erro)),
    }
}

/// O que se sabe de um original depois de lido.
struct Lido {
    tipo_original: Option<String>,
    id_original: Option<String>,
}

/// Importa os `.md` do repositório para `externos/<repositório>/`: cria e
/// regrava as cópias do que mudou, apaga as que não vieram e grava o estado.
///
/// - `arquivos`: os `.md` do commit (só os `.md` são considerados);
/// - `pular`: exclusões do chamador pelo caminho no repositório (os padrões de
///   segredo do inventário); devolve o motivo;
/// - `ler`: o conteúdo de um arquivo (do commit), chamado só para o que mudou.
///
/// Erro de leitura de um arquivo vira [`Pulado`]; erro de gravação na base
/// interrompe (o estado anterior continua valendo).
pub fn importar(
    projeto: &Projeto,
    fonte: &Fonte,
    arquivos: &[ArquivoExterno],
    pular: &dyn Fn(&str) -> Option<String>,
    ler: &mut dyn FnMut(&ArquivoExterno) -> Result<Vec<u8>, String>,
    agora_ms: u64,
) -> Resultado<Importacao> {
    let anterior = EstadoExternos::ler(projeto).unwrap_or_default();
    let pasta = layout::externos_do_repositorio(&fonte.repositorio)?;
    let da_base = identificadores_da_base(projeto)?;
    let mut resumo = Importacao::default();
    let mut registros: BTreeMap<String, Registro> = BTreeMap::new();
    let mut copias: HashSet<String> = HashSet::new();
    let mut nomes_sem_caixa: HashSet<String> = HashSet::new();

    for a in arquivos {
        if !a.caminho.to_ascii_lowercase().ends_with(".md") {
            continue;
        }
        let pulado = |resumo: &mut Importacao, motivo: String| {
            resumo.pulados.push(Pulado {
                caminho: a.caminho_no_repositorio.clone(),
                motivo,
            })
        };
        if let Some(motivo) = pular(&a.caminho_no_repositorio) {
            pulado(&mut resumo, motivo);
            continue;
        }
        let Some(relativo) = caminho_da_copia(&pasta, &a.caminho) else {
            pulado(
                &mut resumo,
                "nome de arquivo que não vale em todos os sistemas (ou em pasta oculta)".into(),
            );
            continue;
        };
        if !nomes_sem_caixa.insert(relativo.as_str().to_lowercase()) {
            pulado(
                &mut resumo,
                "outro arquivo tem o mesmo nome, só com maiúsculas diferentes".into(),
            );
            continue;
        }
        if a.tamanho > LIMITE_DE_TAMANHO {
            pulado(
                &mut resumo,
                format!(
                    "arquivo grande demais ({} KiB; o limite é {} KiB)",
                    a.tamanho / 1024,
                    LIMITE_DE_TAMANHO / 1024
                ),
            );
            continue;
        }
        let antes = anterior
            .arquivos
            .get(&a.caminho_no_repositorio)
            .filter(|r| r.versao == a.versao);

        // Pulado pelo conteúdo nesta mesma versão: não lê de novo.
        if let Some(motivo) = antes.and_then(|r| r.pulado.clone()) {
            pulado(&mut resumo, motivo.clone());
            registros.insert(
                a.caminho_no_repositorio.clone(),
                Registro {
                    versao: a.versao.clone(),
                    tipo_original: None,
                    id_original: None,
                    pulado: Some(motivo),
                    copia: None,
                },
            );
            continue;
        }

        let mut conteudo: Option<String> = None;
        let lido = match antes {
            Some(r) => Lido {
                tipo_original: r.tipo_original.clone(),
                id_original: r.id_original.clone(),
            },
            None => match ler_texto(a, ler) {
                Ok(texto) => {
                    let lido = identificar(&texto);
                    conteudo = Some(texto);
                    lido
                }
                Err(Pulo::Leitura(motivo)) => {
                    pulado(&mut resumo, motivo);
                    continue;
                }
                Err(Pulo::Conteudo(motivo)) => {
                    pulado(&mut resumo, motivo.clone());
                    registros.insert(
                        a.caminho_no_repositorio.clone(),
                        Registro {
                            versao: a.versao.clone(),
                            tipo_original: None,
                            id_original: None,
                            pulado: Some(motivo),
                            copia: None,
                        },
                    );
                    continue;
                }
            },
        };
        let mut registro = Registro {
            versao: a.versao.clone(),
            tipo_original: lido.tipo_original.clone(),
            id_original: lido.id_original.clone(),
            pulado: None,
            copia: None,
        };

        if let (Some(t), Some(id)) = (&lido.tipo_original, &lido.id_original)
            && da_base.contains(&(t.clone(), id.clone()))
        {
            resumo.ja_na_base.push(a.caminho_no_repositorio.clone());
            registros.insert(a.caminho_no_repositorio.clone(), registro);
            continue;
        }

        let destino = projeto.caminho(&relativo);
        let copia_antes = antes
            .and_then(|r| r.copia.as_ref())
            .filter(|c| c.caminho == relativo.as_str());
        if let Some(c) = copia_antes
            && hash_do_arquivo(&destino).as_deref() == Some(c.hash.as_str())
        {
            if c.trechos_mascarados > 0 {
                resumo.mascarados.push(Mascarado {
                    caminho: a.caminho_no_repositorio.clone(),
                    trechos: c.trechos_mascarados,
                });
            }
            registro.copia = Some(c.clone());
            registros.insert(a.caminho_no_repositorio.clone(), registro);
            copias.insert(relativo.as_str().to_owned());
            resumo.inalterados += 1;
            continue;
        }

        let texto = match conteudo {
            Some(t) => t,
            None => match ler_texto(a, ler) {
                Ok(t) => t,
                Err(Pulo::Leitura(m) | Pulo::Conteudo(m)) => {
                    pulado(&mut resumo, m);
                    continue;
                }
            },
        };
        let (bytes, trechos) =
            montar_copia(fonte, a, &texto, &lido).map_err(|erro| Erro::Frontmatter {
                arquivo: projeto.na_base(&relativo),
                erro,
            })?;
        let existia = destino.exists();
        arquivo::gravar(&destino, &bytes).map_err(|erro| projeto.erro_io(&relativo, erro))?;
        if existia {
            resumo.alterados += 1;
        } else {
            resumo.novos += 1;
        }
        if trechos > 0 {
            resumo.mascarados.push(Mascarado {
                caminho: a.caminho_no_repositorio.clone(),
                trechos,
            });
        }
        registro.copia = Some(Copia {
            caminho: relativo.as_str().to_owned(),
            hash: hash(&bytes),
            trechos_mascarados: trechos,
        });
        registros.insert(a.caminho_no_repositorio.clone(), registro);
        copias.insert(relativo.as_str().to_owned());
    }

    resumo.removidos = apagar_sobras(projeto, &copias)?;

    let estado = EstadoExternos {
        formato: FORMATO,
        fonte: Some(fonte.clone()),
        sincronizado_ms: Some(agora_ms),
        tentativa_ms: Some(agora_ms),
        erro: None,
        ja_na_base: resumo.ja_na_base.clone(),
        pulados: resumo.pulados.clone(),
        mascarados: resumo.mascarados.clone(),
        arquivos: registros,
    };
    estado.gravar(projeto)?;
    Ok(resumo)
}

enum Pulo {
    /// Não deu para ler agora (tenta de novo na próxima).
    Leitura(String),
    /// O conteúdo não pode entrar (vale enquanto a versão for a mesma).
    Conteudo(String),
}

fn ler_texto(
    a: &ArquivoExterno,
    ler: &mut dyn FnMut(&ArquivoExterno) -> Result<Vec<u8>, String>,
) -> Result<String, Pulo> {
    let bytes = ler(a).map_err(|e| Pulo::Leitura(format!("não foi possível ler: {e}")))?;
    let texto = String::from_utf8(bytes)
        .map_err(|_| Pulo::Conteudo("o arquivo não está em UTF-8".into()))?;
    if sensivel::tem_chave_privada(&texto) {
        return Err(Pulo::Conteudo(
            "possível segredo: traz uma chave privada".into(),
        ));
    }
    Ok(texto)
}

/// `tipo` conhecido e `id` (ou `termo`) do frontmatter do original, se houver.
fn identificar(texto: &str) -> Lido {
    let Ok(doc) = Documento::<FrontmatterLivre>::analisar(texto) else {
        return Lido {
            tipo_original: None,
            id_original: None,
        };
    };
    let tipo = doc
        .frontmatter
        .tipo()
        .filter(|t| *t != TipoDocumento::DocExterno);
    Lido {
        tipo_original: tipo.map(|t| t.como_str().to_owned()),
        id_original: tipo.and_then(|_| identificador(&doc.frontmatter.0)),
    }
}

fn texto_de(mapa: &Mapping, campo: &str) -> Option<String> {
    let texto = match mapa.get(campo)? {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        _ => return None,
    };
    let texto = texto.trim();
    (!texto.is_empty()).then(|| texto.to_owned())
}

fn identificador(mapa: &Mapping) -> Option<String> {
    texto_de(mapa, "id").or_else(|| texto_de(mapa, "termo"))
}

/// `(tipo, id)` dos documentos da base do projeto (fora de `externos/`).
fn identificadores_da_base(projeto: &Projeto) -> Resultado<HashSet<(String, String)>> {
    let documentos = match projeto.documentos() {
        Ok(d) => d,
        Err(Erro::Io { erro, .. }) if erro.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(e),
    };
    let mut saida = HashSet::new();
    for relativo in documentos {
        if layout::e_externo(&relativo) {
            continue;
        }
        let Ok(texto) = projeto.ler_texto(&relativo) else {
            continue;
        };
        let Ok(doc) = Documento::<FrontmatterLivre>::analisar(&texto) else {
            continue;
        };
        if let (Some(tipo), Some(id)) = (doc.frontmatter.tipo(), identificador(&doc.frontmatter.0))
        {
            saida.insert((tipo.como_str().to_owned(), id));
        }
    }
    Ok(saida)
}

/// Nomes reservados do Windows (sem diferença de maiúsculas, com ou sem extensão).
const RESERVADOS: &[&str] = &[
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// `externos/<repositório>/<caminho>`, se o caminho servir em qualquer sistema e
/// não estiver numa pasta oculta (que a base não lista).
fn caminho_da_copia(pasta: &CaminhoRelativo, caminho: &str) -> Option<CaminhoRelativo> {
    let relativo = CaminhoRelativo::novo(caminho).ok()?;
    for parte in relativo.partes() {
        let reservado = parte
            .split('.')
            .next()
            .is_some_and(|n| RESERVADOS.contains(&n.to_ascii_lowercase().as_str()));
        if parte.starts_with('.')
            || parte.ends_with('.')
            || parte.ends_with(' ')
            || reservado
            || parte
                .chars()
                .any(|c| c.is_control() || matches!(c, '<' | '>' | '"' | '|' | '?' | '*'))
        {
            return None;
        }
    }
    if layout::e_memoria_da_analise(relativo.nome()) {
        return None;
    }
    pasta.juntar(relativo.as_str()).ok()
}

/// O texto da cópia e quantos trechos foram mascarados.
fn montar_copia(
    fonte: &Fonte,
    a: &ArquivoExterno,
    texto: &str,
    lido: &Lido,
) -> Result<(Vec<u8>, u32), crate::ErroFrontmatter> {
    let (texto, trechos) = sensivel::mascarar(texto);
    let (original, corpo) = match Documento::<FrontmatterLivre>::analisar(&texto) {
        Ok(d) => (Some(d.frontmatter.0), d.corpo),
        Err(_) => (None, texto.clone()),
    };
    let titulo = original
        .as_ref()
        .and_then(|m| {
            texto_de(m, "titulo")
                .or_else(|| texto_de(m, "termo"))
                .or_else(|| texto_de(m, "nome"))
        })
        .or_else(|| primeiro_titulo(&corpo))
        .unwrap_or_else(|| {
            let nome = a.caminho.rsplit('/').next().unwrap_or(&a.caminho);
            let sem_extensao = nome.len().saturating_sub(3);
            nome.get(..sem_extensao).unwrap_or(nome).to_owned()
        });
    let frontmatter = FrontmatterDocExterno {
        tipo: TipoDocumento::DocExterno,
        titulo,
        origem: OrigemExterna {
            repositorio: fonte.repositorio.clone(),
            caminho: a.caminho_no_repositorio.clone(),
            commit: fonte.commit.clone(),
            branch: Some(fonte.branch.clone()).filter(|b| !b.is_empty()),
        },
        confianca: if lido.tipo_original.is_some() {
            Confianca::Publicada
        } else {
            Confianca::Externa
        },
        tipo_original: lido.tipo_original.clone(),
        id_original: lido.id_original.clone(),
        trechos_mascarados: trechos,
        frontmatter_original: original.filter(|m| !m.is_empty()),
    };
    let texto = Documento::novo(frontmatter, corpo).para_texto()?;
    Ok((texto.into_bytes(), trechos))
}

fn primeiro_titulo(corpo: &str) -> Option<String> {
    corpo.lines().find_map(|linha| {
        let resto = linha.trim_start().strip_prefix('#')?;
        let titulo = resto.trim_start_matches('#').trim();
        (!titulo.is_empty()).then(|| titulo.to_owned())
    })
}

fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn hash_do_arquivo(caminho: &Path) -> Option<String> {
    fs::read(caminho).ok().map(|b| hash(&b))
}

/// Apaga de `externos/` os arquivos que não são cópias desta importação (nem o
/// estado e a trava) e as pastas que ficarem vazias. Devolve quantos `.md` saíram.
fn apagar_sobras(projeto: &Projeto, copias: &HashSet<String>) -> Resultado<usize> {
    let externos = layout::externos();
    let raiz = projeto.raiz();
    let mut removidos = 0;
    let mut pastas: BTreeSet<std::path::PathBuf> = BTreeSet::new();
    let mut pendentes = vec![projeto.caminho(&externos)];
    while let Some(pasta) = pendentes.pop() {
        let entradas = match fs::read_dir(&pasta) {
            Ok(e) => e,
            Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
            Err(erro) => return Err(projeto.erro_io(&externos, erro)),
        };
        for entrada in entradas.flatten() {
            let caminho = entrada.path();
            let nome = entrada.file_name().to_string_lossy().into_owned();
            let Ok(tipo) = entrada.file_type() else {
                continue;
            };
            let na_raiz = pasta == projeto.caminho(&externos);
            if na_raiz
                && (nome == layout::ARQUIVO_ESTADO_EXTERNOS || nome == layout::TRAVA_EXTERNOS)
            {
                continue;
            }
            if tipo.is_dir() {
                pastas.insert(caminho.clone());
                pendentes.push(caminho);
                continue;
            }
            let relativo = caminho
                .strip_prefix(&raiz)
                .ok()
                .and_then(|p| CaminhoRelativo::de_caminho(p).ok());
            if relativo.is_some_and(|r| copias.contains(r.as_str())) {
                continue;
            }
            // Temporário de uma gravação em andamento noutro processo: deixa.
            if nome.starts_with(arquivo::PREFIXO_TEMPORARIO) {
                continue;
            }
            match fs::remove_file(&caminho) {
                Ok(()) => {
                    if nome.to_ascii_lowercase().ends_with(".md") {
                        removidos += 1;
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(erro) => return Err(projeto.erro_io(&externos, erro)),
            }
        }
    }
    // Das mais fundas para as mais rasas; pasta com algo dentro fica.
    for pasta in pastas.iter().rev() {
        let _ = fs::remove_dir(pasta);
    }
    Ok(removidos)
}

#[cfg(test)]
mod tests;
