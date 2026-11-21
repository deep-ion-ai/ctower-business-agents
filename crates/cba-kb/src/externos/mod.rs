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
use crate::marcador::{self, Bloco};
use crate::yaml::{Mapping, Value};

/// Arquivo maior que isto não é importado (documentação, não anexo).
pub const LIMITE_DE_TAMANHO: u64 = 1024 * 1024;

const CHAVE_PRIVADA: &str = "possível segredo: traz uma chave privada";

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
    /// Arquivos importados com ressalva (página com marcadores malformados,
    /// importada inteira): o caminho e o aviso.
    pub avisos: Vec<Pulado>,
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
    /// Página com marcadores (#290): um registro por bloco, e nenhuma cópia do
    /// arquivo inteiro.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    blocos: Vec<RegistroBloco>,
    /// Aviso da leitura (marcador malformado), repetido enquanto a versão for a
    /// mesma.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    aviso: Option<String>,
}

/// Um bloco de uma página com marcadores.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct RegistroBloco {
    /// Hash do marcador e do texto do bloco.
    versao: String,
    tipo_original: String,
    id_original: String,
    /// Caminho do documento na base de origem (do marcador).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    caminho_original: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ancora: Option<String>,
    /// Onde a cópia fica (ou ficaria), relativo à raiz do projeto.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    copia_em: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pulado: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    copia: Option<Copia>,
}

impl Registro {
    fn pulado(versao: String, motivo: String) -> Self {
        Self {
            versao,
            tipo_original: None,
            id_original: None,
            pulado: Some(motivo),
            copia: None,
            blocos: Vec::new(),
            aviso: None,
        }
    }

    /// As partes registradas, sem o conteúdo.
    fn partes(&self, a: &ArquivoExterno, relativo: &CaminhoRelativo) -> Vec<Parte> {
        if self.blocos.is_empty() {
            return vec![Parte {
                rotulo: a.caminho_no_repositorio.clone(),
                versao: self.versao.clone(),
                tipo_original: self.tipo_original.clone(),
                id_original: self.id_original.clone(),
                caminho_original: None,
                ancora: None,
                relativo: Some(relativo.clone()),
                pulado: None,
                texto: None,
            }];
        }
        self.blocos
            .iter()
            .map(|b| Parte {
                rotulo: format!(
                    "{}#{}",
                    a.caminho_no_repositorio,
                    b.ancora.as_deref().unwrap_or(&b.id_original)
                ),
                versao: b.versao.clone(),
                tipo_original: Some(b.tipo_original.clone()),
                id_original: Some(b.id_original.clone()),
                caminho_original: Some(b.caminho_original.clone().unwrap_or_default()),
                ancora: b.ancora.clone(),
                relativo: b
                    .copia_em
                    .as_deref()
                    .and_then(|c| CaminhoRelativo::novo(c).ok()),
                pulado: b.pulado.clone(),
                texto: None,
            })
            .collect()
    }

    /// A cópia gravada em `relativo` a partir do conteúdo na `versao`.
    fn copia_de(&self, relativo: &str, versao: &str) -> Option<&Copia> {
        if self.blocos.is_empty() {
            return self
                .copia
                .as_ref()
                .filter(|c| self.versao == versao && c.caminho == relativo);
        }
        self.blocos
            .iter()
            .filter(|b| b.versao == versao)
            .filter_map(|b| b.copia.as_ref())
            .find(|c| c.caminho == relativo)
    }
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
    pub avisos: Vec<Pulado>,
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

    /// Quantos documentos foram copiados para a base (cada bloco de uma página
    /// com marcadores conta como um).
    pub fn importados(&self) -> usize {
        self.arquivos
            .values()
            .map(|r| {
                usize::from(r.copia.is_some())
                    + r.blocos.iter().filter(|b| b.copia.is_some()).count()
            })
            .sum()
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

/// Um documento a copiar: o arquivo inteiro (formato antigo, ou página sem
/// marcador) ou um bloco recortado de uma página com marcadores.
struct Parte {
    /// Como aparece no resumo: o caminho no repositório, com `#âncora` no bloco.
    rotulo: String,
    /// Versão do conteúdo: o blob do arquivo, ou o hash do bloco.
    versao: String,
    tipo_original: Option<String>,
    id_original: Option<String>,
    /// Caminho do documento na base de origem (do marcador).
    caminho_original: Option<String>,
    ancora: Option<String>,
    /// Caminho da cópia; `None` se o nome não serve (bloco).
    relativo: Option<CaminhoRelativo>,
    /// Pulado pelo conteúdo (chave privada no bloco).
    pulado: Option<String>,
    /// O conteúdo, quando o arquivo foi lido nesta importação.
    texto: Option<Texto>,
}

enum Texto {
    Inteiro(String),
    Bloco(Bloco),
}

impl Parte {
    fn bloco(&self) -> bool {
        self.caminho_original.is_some()
    }
}

/// `(tipo, id)` e `(tipo, caminho)` dos documentos da base do projeto.
struct DaBase {
    ids: HashSet<(String, String)>,
    caminhos: HashSet<(String, String)>,
}

impl DaBase {
    /// Se a base já tem o documento: mesmo `tipo` e `id`, ou (bloco) mesmo `tipo`
    /// e caminho na base de origem (o negocial não tem `id`).
    fn tem(&self, p: &Parte) -> bool {
        let Some(tipo) = &p.tipo_original else {
            return false;
        };
        p.id_original
            .as_ref()
            .is_some_and(|id| self.ids.contains(&(tipo.clone(), id.clone())))
            || p.caminho_original
                .as_ref()
                .is_some_and(|c| self.caminhos.contains(&(tipo.clone(), c.clone())))
    }
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
    importar_com_links(projeto, fonte, arquivos, pular, ler, &|_| None, agora_ms)
}

/// Como [`importar`], com `link`: o endereço de um arquivo (pelo caminho no
/// repositório) na forja, no commit sincronizado, para a origem das cópias. No
/// bloco recortado de uma página, a âncora vai junto (`...#rn-cob-001`).
pub fn importar_com_links(
    projeto: &Projeto,
    fonte: &Fonte,
    arquivos: &[ArquivoExterno],
    pular: &dyn Fn(&str) -> Option<String>,
    ler: &mut dyn FnMut(&ArquivoExterno) -> Result<Vec<u8>, String>,
    link: &dyn Fn(&str) -> Option<String>,
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
        let pulado = |resumo: &mut Importacao, caminho: &str, motivo: String| {
            resumo.pulados.push(Pulado {
                caminho: caminho.to_owned(),
                motivo,
            })
        };
        let caminho = a.caminho_no_repositorio.as_str();
        if let Some(motivo) = pular(caminho) {
            pulado(&mut resumo, caminho, motivo);
            continue;
        }
        let Some(relativo) = caminho_da_copia(&pasta, &a.caminho) else {
            pulado(
                &mut resumo,
                caminho,
                "nome de arquivo que não vale em todos os sistemas (ou em pasta oculta)".into(),
            );
            continue;
        };
        if a.tamanho > LIMITE_DE_TAMANHO {
            pulado(
                &mut resumo,
                caminho,
                format!(
                    "arquivo grande demais ({} KiB; o limite é {} KiB)",
                    a.tamanho / 1024,
                    LIMITE_DE_TAMANHO / 1024
                ),
            );
            continue;
        }
        // O registro da importação anterior, de qualquer versão: o bloco que não
        // mudou numa página que mudou continua com a cópia (e o commit) de antes.
        let ultimo = anterior.arquivos.get(caminho);
        let antes = ultimo.filter(|r| r.versao == a.versao);

        // Pulado pelo conteúdo nesta mesma versão: não lê de novo.
        if let Some(motivo) = antes.and_then(|r| r.pulado.clone()) {
            pulado(&mut resumo, caminho, motivo.clone());
            registros.insert(
                caminho.to_owned(),
                Registro::pulado(a.versao.clone(), motivo),
            );
            continue;
        }

        let reaproveitavel = |p: &Parte| -> Option<Copia> {
            let relativo = p.relativo.as_ref()?;
            let c = ultimo?.copia_de(relativo.as_str(), &p.versao)?;
            (hash_do_arquivo(&projeto.caminho(relativo)).as_deref() == Some(c.hash.as_str()))
                .then(|| c.clone())
        };

        // Mesma versão e todas as cópias intactas: nada a ler.
        let registradas = antes.map(|r| r.partes(a, &relativo));
        let precisa_ler = registradas.as_ref().is_none_or(|partes| {
            partes.iter().any(|p| {
                p.pulado.is_none()
                    && p.relativo.is_some()
                    && !da_base.tem(p)
                    && reaproveitavel(p).is_none()
            })
        });
        let mut aviso = antes.and_then(|r| r.aviso.clone());
        let partes = match registradas {
            Some(partes) if !precisa_ler => partes,
            _ => match ler_texto(a, ler) {
                Ok(texto) => {
                    let (partes, a_aviso) = recortar(a, &relativo, &pasta, texto);
                    aviso = a_aviso;
                    partes
                }
                Err(Pulo::Leitura(motivo)) => {
                    pulado(&mut resumo, caminho, motivo);
                    continue;
                }
                Err(Pulo::Conteudo(motivo)) => {
                    pulado(&mut resumo, caminho, motivo.clone());
                    registros.insert(
                        caminho.to_owned(),
                        Registro::pulado(a.versao.clone(), motivo),
                    );
                    continue;
                }
            },
        };
        if let Some(aviso) = &aviso {
            resumo.avisos.push(Pulado {
                caminho: caminho.to_owned(),
                motivo: aviso.clone(),
            });
        }

        let mut registro = Registro {
            versao: a.versao.clone(),
            tipo_original: None,
            id_original: None,
            pulado: None,
            copia: None,
            blocos: Vec::new(),
            aviso,
        };
        for parte in partes {
            let copia = copiar_parte(
                projeto,
                fonte,
                a,
                &parte,
                &da_base,
                &reaproveitavel,
                link,
                &mut nomes_sem_caixa,
                &mut resumo,
            )?;
            if let Some(c) = &copia {
                copias.insert(c.caminho.clone());
            }
            if parte.bloco() {
                registro.blocos.push(RegistroBloco {
                    versao: parte.versao,
                    tipo_original: parte.tipo_original.unwrap_or_default(),
                    id_original: parte.id_original.unwrap_or_default(),
                    caminho_original: parte.caminho_original,
                    ancora: parte.ancora,
                    copia_em: parte.relativo.map(|r| r.as_str().to_owned()),
                    pulado: parte.pulado,
                    copia,
                });
            } else {
                registro.tipo_original = parte.tipo_original;
                registro.id_original = parte.id_original;
                registro.pulado = parte.pulado;
                registro.copia = copia;
            }
        }
        registros.insert(caminho.to_owned(), registro);
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
        avisos: resumo.avisos.clone(),
        arquivos: registros,
    };
    estado.gravar(projeto)?;
    Ok(resumo)
}

/// Copia uma parte (ou reaproveita a cópia de antes) e devolve a cópia; `None`
/// se ela não entra (pulada, já na base, nome repetido).
#[allow(clippy::too_many_arguments)]
fn copiar_parte(
    projeto: &Projeto,
    fonte: &Fonte,
    a: &ArquivoExterno,
    parte: &Parte,
    da_base: &DaBase,
    reaproveitavel: &dyn Fn(&Parte) -> Option<Copia>,
    link: &dyn Fn(&str) -> Option<String>,
    nomes_sem_caixa: &mut HashSet<String>,
    resumo: &mut Importacao,
) -> Resultado<Option<Copia>> {
    let pular = |resumo: &mut Importacao, motivo: String| {
        resumo.pulados.push(Pulado {
            caminho: parte.rotulo.clone(),
            motivo,
        });
    };
    if let Some(motivo) = &parte.pulado {
        pular(resumo, motivo.clone());
        return Ok(None);
    }
    let Some(relativo) = &parte.relativo else {
        pular(
            resumo,
            "identificador que não serve de nome de arquivo em todos os sistemas".into(),
        );
        return Ok(None);
    };
    if da_base.tem(parte) {
        resumo.ja_na_base.push(parte.rotulo.clone());
        return Ok(None);
    }
    if !nomes_sem_caixa.insert(relativo.as_str().to_lowercase()) {
        pular(
            resumo,
            "outro arquivo tem o mesmo nome, só com maiúsculas diferentes".into(),
        );
        return Ok(None);
    }
    if let Some(c) = reaproveitavel(parte) {
        if c.trechos_mascarados > 0 {
            resumo.mascarados.push(Mascarado {
                caminho: parte.rotulo.clone(),
                trechos: c.trechos_mascarados,
            });
        }
        resumo.inalterados += 1;
        return Ok(Some(c));
    }
    let Some(texto) = &parte.texto else {
        // Não acontece: sem cópia aproveitável, o arquivo é lido.
        pular(resumo, "conteúdo não lido".into());
        return Ok(None);
    };
    let url = link(&a.caminho_no_repositorio).map(|u| match &parte.ancora {
        Some(ancora) => format!("{u}#{ancora}"),
        None => u,
    });
    let origem = OrigemExterna {
        repositorio: fonte.repositorio.clone(),
        caminho: a.caminho_no_repositorio.clone(),
        commit: fonte.commit.clone(),
        branch: Some(fonte.branch.clone()).filter(|b| !b.is_empty()),
        ancora: parte.ancora.clone(),
        url,
    };
    let montado = match texto {
        Texto::Inteiro(t) => montar_copia(origem, a, t, parte),
        Texto::Bloco(b) => montar_bloco(origem, b, parte),
    };
    let (bytes, trechos) = montado.map_err(|erro| Erro::Frontmatter {
        arquivo: projeto.na_base(relativo),
        erro,
    })?;
    let destino = projeto.caminho(relativo);
    let existia = destino.exists();
    arquivo::gravar(&destino, &bytes).map_err(|erro| projeto.erro_io(relativo, erro))?;
    if existia {
        resumo.alterados += 1;
    } else {
        resumo.novos += 1;
    }
    if trechos > 0 {
        resumo.mascarados.push(Mascarado {
            caminho: parte.rotulo.clone(),
            trechos,
        });
    }
    Ok(Some(Copia {
        caminho: relativo.as_str().to_owned(),
        hash: hash(&bytes),
        trechos_mascarados: trechos,
    }))
}

/// As partes de um arquivo lido: um bloco por marcador ou, sem marcador (ou
/// com marcador malformado, devolvido como aviso), o arquivo inteiro.
fn recortar(
    a: &ArquivoExterno,
    relativo: &CaminhoRelativo,
    pasta: &CaminhoRelativo,
    texto: String,
) -> (Vec<Parte>, Option<String>) {
    let (blocos, aviso) = match marcador::blocos(&texto) {
        Ok(b) => (b, None),
        Err(e) => (
            Vec::new(),
            Some(format!(
                "marcadores da documentação ilegíveis ({e}): a página entrou inteira, como \
                 um documento só"
            )),
        ),
    };
    if blocos.is_empty() {
        let lido = identificar(&texto);
        return (
            vec![Parte {
                rotulo: a.caminho_no_repositorio.clone(),
                versao: a.versao.clone(),
                tipo_original: lido.tipo_original,
                id_original: lido.id_original,
                caminho_original: None,
                ancora: None,
                relativo: Some(relativo.clone()),
                pulado: sensivel::tem_chave_privada(&texto).then(|| CHAVE_PRIVADA.to_string()),
                texto: Some(Texto::Inteiro(texto)),
            }],
            aviso,
        );
    }
    let partes = blocos
        .into_iter()
        .map(|b| {
            let m = &b.marcador;
            let frontmatter = yaml_do_json(&m.metadados.frontmatter);
            let id = frontmatter
                .as_ref()
                .and_then(identificador)
                .unwrap_or_else(|| m.id.clone());
            let ancora = b.ancora().map(str::to_owned);
            let rotulo = format!(
                "{}#{}",
                a.caminho_no_repositorio,
                ancora.clone().unwrap_or_else(|| id.clone())
            );
            let versao = hash(format!("{}\n{}", m.abertura(), b.conteudo).as_bytes());
            let pulado =
                sensivel::tem_chave_privada(&b.conteudo).then(|| CHAVE_PRIVADA.to_string());
            Parte {
                rotulo,
                versao,
                tipo_original: Some(m.tipo.clone()),
                id_original: Some(id),
                caminho_original: Some(m.metadados.caminho.clone()),
                relativo: caminho_do_bloco(pasta, &a.caminho, &m.tipo, &m.id),
                ancora,
                pulado,
                texto: Some(Texto::Bloco(b)),
            }
        })
        .collect();
    (partes, aviso)
}

/// `externos/<repositório>/<página sem .md>/<tipo>-<id>.md`: estável enquanto o
/// documento ficar na mesma página, para a importação seguinte reconhecer a
/// cópia (e apagá-la quando o bloco sair da página).
fn caminho_do_bloco(
    pasta: &CaminhoRelativo,
    pagina: &str,
    tipo: &str,
    id: &str,
) -> Option<CaminhoRelativo> {
    let sem_extensao = pagina.get(..pagina.len().checked_sub(3)?)?;
    let id: String = id
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '-'
            }
        })
        .collect();
    caminho_da_copia(pasta, &format!("{sem_extensao}/{tipo}-{id}.md"))
}

/// O frontmatter do marcador (JSON) como YAML; `None` se não for um objeto.
fn yaml_do_json(json: &serde_json::Value) -> Option<Mapping> {
    match serde_norway::to_value(json).ok()? {
        Value::Mapping(m) => Some(m),
        _ => None,
    }
}

/// Mascara os textos de um valor YAML; devolve quantos trechos.
fn mascarar_yaml(valor: &mut Value) -> u32 {
    match valor {
        Value::String(s) => {
            let (mascarado, n) = sensivel::mascarar(s);
            if n > 0 {
                *s = mascarado;
            }
            n
        }
        Value::Sequence(itens) => itens.iter_mut().map(mascarar_yaml).sum(),
        Value::Mapping(m) => m.values_mut().map(mascarar_yaml).sum(),
        Value::Tagged(t) => mascarar_yaml(&mut t.value),
        _ => 0,
    }
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
    String::from_utf8(bytes).map_err(|_| Pulo::Conteudo("o arquivo não está em UTF-8".into()))
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

/// `(tipo, id)` e `(tipo, caminho)` dos documentos da base do projeto (fora de
/// `externos/`).
fn identificadores_da_base(projeto: &Projeto) -> Resultado<DaBase> {
    let documentos = match projeto.documentos() {
        Ok(d) => d,
        Err(Erro::Io { erro, .. }) if erro.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(e),
    };
    let mut saida = DaBase {
        ids: HashSet::new(),
        caminhos: HashSet::new(),
    };
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
        let Some(tipo) = doc.frontmatter.tipo() else {
            continue;
        };
        let tipo = tipo.como_str().to_owned();
        if let Some(id) = identificador(&doc.frontmatter.0) {
            saida.ids.insert((tipo.clone(), id));
        }
        saida.caminhos.insert((tipo, relativo.as_str().to_owned()));
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

/// O texto da cópia de um arquivo inteiro e quantos trechos foram mascarados.
fn montar_copia(
    origem: OrigemExterna,
    a: &ArquivoExterno,
    texto: &str,
    parte: &Parte,
) -> Result<(Vec<u8>, u32), crate::ErroFrontmatter> {
    let (texto, trechos) = sensivel::mascarar(texto);
    let (original, corpo) = match Documento::<FrontmatterLivre>::analisar(&texto) {
        Ok(d) => (Some(d.frontmatter.0), d.corpo),
        Err(_) => (None, texto.clone()),
    };
    let titulo = original
        .as_ref()
        .and_then(titulo_do_frontmatter)
        .or_else(|| primeiro_titulo(&corpo))
        .unwrap_or_else(|| {
            let nome = a.caminho.rsplit('/').next().unwrap_or(&a.caminho);
            let sem_extensao = nome.len().saturating_sub(3);
            nome.get(..sem_extensao).unwrap_or(nome).to_owned()
        });
    let frontmatter = FrontmatterDocExterno {
        tipo: TipoDocumento::DocExterno,
        titulo,
        origem,
        confianca: if parte.tipo_original.is_some() {
            Confianca::Publicada
        } else {
            Confianca::Externa
        },
        tipo_original: parte.tipo_original.clone(),
        id_original: parte.id_original.clone(),
        trechos_mascarados: trechos,
        frontmatter_original: original.filter(|m| !m.is_empty()),
    };
    let texto = Documento::novo(frontmatter, corpo).para_texto()?;
    Ok((texto.into_bytes(), trechos))
}

/// O texto da cópia de um bloco recortado de uma página: o frontmatter do
/// marcador vira o `frontmatter_original` e o texto legível do bloco (sem a
/// linha da âncora), o corpo. Os dois saem mascarados.
fn montar_bloco(
    origem: OrigemExterna,
    bloco: &Bloco,
    parte: &Parte,
) -> Result<(Vec<u8>, u32), crate::ErroFrontmatter> {
    let mut original = yaml_do_json(&bloco.marcador.metadados.frontmatter);
    let mut trechos: u32 = original
        .as_mut()
        .map(|m| m.values_mut().map(mascarar_yaml).sum())
        .unwrap_or(0);
    let corpo = corpo_do_bloco(bloco);
    let (corpo, n) = sensivel::mascarar(&corpo);
    trechos += n;
    let titulo = original
        .as_ref()
        .and_then(titulo_do_frontmatter)
        .or_else(|| primeiro_titulo(&corpo))
        .or_else(|| parte.id_original.clone())
        .unwrap_or_else(|| bloco.marcador.id.clone());
    let frontmatter = FrontmatterDocExterno {
        tipo: TipoDocumento::DocExterno,
        titulo,
        origem,
        confianca: Confianca::Publicada,
        tipo_original: parte.tipo_original.clone(),
        id_original: parte.id_original.clone(),
        trechos_mascarados: trechos,
        frontmatter_original: original.filter(|m| !m.is_empty()),
    };
    let texto = Documento::novo(frontmatter, format!("\n{corpo}\n")).para_texto()?;
    Ok((texto.into_bytes(), trechos))
}

/// O texto legível do bloco, sem a linha da âncora (`<a id="..."></a>`).
fn corpo_do_bloco(bloco: &Bloco) -> String {
    let mut linhas = bloco
        .conteudo
        .lines()
        .skip_while(|l| l.trim().is_empty())
        .peekable();
    if bloco.ancora().is_some() {
        linhas.next();
    }
    let corpo: Vec<&str> = linhas.collect();
    corpo.join("\n").trim().to_owned()
}

fn titulo_do_frontmatter(m: &Mapping) -> Option<String> {
    texto_de(m, "titulo")
        .or_else(|| texto_de(m, "termo"))
        .or_else(|| texto_de(m, "nome"))
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
