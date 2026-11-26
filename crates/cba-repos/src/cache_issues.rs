//! Cache da listagem das issues do rastreador por projeto (#322).
//!
//! A última leitura de [`crate::listagem::listar_issues`] fica num arquivo JSON
//! em `<dados>/issues/<projeto>.json`, com o instante da leitura e a identidade
//! do rastreador (trocar o rastreador do projeto descarta o cache). Abrir a lista
//! lê só o arquivo; a releitura do rastreador fica para quem chama, em segundo
//! plano, e uma [`Reserva`] impede duas releituras do mesmo projeto ao mesmo
//! tempo. Sem rede, o cache continua valendo e guarda o aviso da falha.
//!
//! Nada é escrito no rastreador.

use crate::TrackerConfig;
use crate::demanda::{DemandaErro, Ferramentas};
use crate::listagem::{IssueListada, Listagem, Periodo, listar_issues};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Formato do arquivo; outro formato é descartado como cache ausente.
pub const FORMATO: u32 = 1;
/// Idade a partir da qual a lista pede releitura em segundo plano.
pub const VALIDADE_PADRAO: Duration = Duration::from_secs(5 * 60);

/// Última tentativa de leitura que falhou.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Falha {
    pub em_ms: u64,
    pub mensagem: String,
}

/// Conteúdo do cache de um projeto.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheIssues {
    pub formato: u32,
    /// [`identidade`] do rastreador lido.
    pub rastreador: String,
    /// Instante da última leitura que deu certo (ms desde 1970, UTC).
    pub lida_em_ms: u64,
    pub listagem: Listagem,
    /// Falha posterior à leitura (sem rede, sem token): a lista mostra a última
    /// leitura com o aviso.
    #[serde(default)]
    pub falha: Option<Falha>,
}

impl CacheIssues {
    /// A leitura tem pelo menos `validade` de idade em `agora_ms`?
    pub fn vencido(&self, validade: Duration, agora_ms: u64) -> bool {
        agora_ms.saturating_sub(self.lida_em_ms) >= validade.as_millis() as u64
    }
}

pub fn agora_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Identifica o rastreador de que a listagem veio.
pub fn identidade(tracker: &TrackerConfig) -> String {
    match tracker {
        TrackerConfig::Github(g) => format!("github:{}", g.repo.trim().to_ascii_lowercase()),
        TrackerConfig::Gitlab(g) => format!("gitlab:{}/{}", g.host.trim(), g.path.trim()),
        TrackerConfig::Jira(j) => format!(
            "jira:{}|{}|{}|{}",
            j.url.trim().trim_end_matches('/'),
            j.project_key.trim(),
            j.jql.trim(),
            j.active_sprint_only
        ),
    }
}

/// Arquivo do cache do projeto dentro de `pasta` (`<dados>/issues`).
pub fn arquivo(pasta: &Path, projeto: &str) -> PathBuf {
    let nome: String = projeto
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    pasta.join(format!("{nome}.json"))
}

/// Lê o cache. Ausente, ilegível, de outro formato ou de outro rastreador: `None`.
pub fn ler(arquivo: &Path, tracker: &TrackerConfig) -> Option<CacheIssues> {
    let bytes = std::fs::read(arquivo).ok()?;
    let cache: CacheIssues = serde_json::from_slice(&bytes).ok()?;
    (cache.formato == FORMATO && cache.rastreador == identidade(tracker)).then_some(cache)
}

/// Grava o cache de uma vez (arquivo temporário e `rename`), para a leitura
/// concorrente nunca ver o arquivo pela metade.
pub fn gravar(arquivo: &Path, cache: &CacheIssues) -> io::Result<()> {
    if let Some(pasta) = arquivo.parent() {
        std::fs::create_dir_all(pasta)?;
    }
    let tmp = arquivo.with_extension(format!("json.{}.tmp", std::process::id()));
    let json = serde_json::to_vec_pretty(cache).map_err(io::Error::other)?;
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, arquivo).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

static EM_LEITURA: LazyLock<Mutex<HashSet<PathBuf>>> = LazyLock::new(Default::default);

/// Releitura em curso de um cache; liberada ao sair de escopo.
#[derive(Debug)]
pub struct Reserva {
    arquivo: PathBuf,
}

impl Drop for Reserva {
    fn drop(&mut self) {
        EM_LEITURA
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&self.arquivo);
    }
}

/// Reserva a releitura do cache; `None` se outra já está em curso.
pub fn reservar(arquivo: &Path) -> Option<Reserva> {
    let mut em = EM_LEITURA.lock().unwrap_or_else(|e| e.into_inner());
    em.insert(arquivo.to_path_buf()).then(|| Reserva {
        arquivo: arquivo.to_path_buf(),
    })
}

/// Há releitura em curso deste cache?
pub fn em_leitura(arquivo: &Path) -> bool {
    EM_LEITURA
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .contains(arquivo)
}

/// Relê o rastreador (período padrão) e grava o cache. Na falha, o cache
/// anterior (se houver, do mesmo rastreador) fica com o aviso e o erro volta.
/// Quem chama deve ter a [`Reserva`].
pub fn atualizar(
    arquivo: &Path,
    tracker: &TrackerConfig,
    f: &Ferramentas,
) -> Result<CacheIssues, DemandaErro> {
    match listar_issues(tracker, &Periodo::default(), None, f) {
        Ok(listagem) => {
            let cache = CacheIssues {
                formato: FORMATO,
                rastreador: identidade(tracker),
                lida_em_ms: agora_ms(),
                listagem,
                falha: None,
            };
            if let Err(e) = gravar(arquivo, &cache) {
                return Err(DemandaErro::Falhou {
                    ferramenta: "cache",
                    detalhe: format!("o cache das issues não pôde ser gravado: {e}"),
                });
            }
            Ok(cache)
        }
        Err(erro) => {
            if let Some(mut anterior) = ler(arquivo, tracker) {
                anterior.falha = Some(Falha {
                    em_ms: agora_ms(),
                    mensagem: erro.to_string(),
                });
                // Sem gravar o aviso, a próxima abertura só não o mostra.
                let _ = gravar(arquivo, &anterior);
            }
            Err(erro)
        }
    }
}

/// Estado no rastreador pedido no filtro.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FiltroEstado {
    /// O padrão: as abertas no rastreador.
    #[default]
    Abertas,
    Encerradas,
    Todas,
}

/// Busca e filtros sobre a listagem.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FiltroIssues {
    /// Número (`41`, `#41`, `PROJ-41`) ou trecho do título, sem diferença de
    /// maiúsculas nem de acentos.
    pub busca: Option<String>,
    pub estado: FiltroEstado,
    /// Rótulo exato (sem diferença de maiúsculas).
    pub rotulo: Option<String>,
}

fn dobrar(texto: &str) -> String {
    texto
        .chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            c => c,
        })
        .collect()
}

/// Número da chave (`#41` → `41`, `PROJ-41` → `41`).
fn numero_da_chave(chave: &str) -> &str {
    let c = chave.trim().trim_start_matches('#');
    c.rsplit_once('-').map_or(c, |(_, n)| n)
}

impl FiltroIssues {
    /// A issue passa no estado, no rótulo e na busca? O estado é o do rastreador
    /// na última leitura.
    pub fn aceita(&self, i: &IssueListada) -> bool {
        let aberta = i.estado == "aberta";
        let estado = match self.estado {
            FiltroEstado::Abertas => aberta,
            FiltroEstado::Encerradas => !aberta,
            FiltroEstado::Todas => true,
        };
        let rotulo = self
            .rotulo
            .as_deref()
            .map(str::trim)
            .filter(|r| !r.is_empty())
            .is_none_or(|r| i.rotulos.iter().any(|x| x.eq_ignore_ascii_case(r)));
        estado && rotulo && self.busca_aceita(i)
    }

    fn busca_aceita(&self, i: &IssueListada) -> bool {
        let Some(b) = self
            .busca
            .as_deref()
            .map(str::trim)
            .filter(|b| !b.is_empty())
        else {
            return true;
        };
        let sem_cerquilha = b.trim_start_matches('#');
        if !sem_cerquilha.is_empty() && sem_cerquilha.bytes().all(|c| c.is_ascii_digit()) {
            let pedido = sem_cerquilha.parse::<u64>().ok();
            if pedido.is_some() && numero_da_chave(&i.chave).parse::<u64>().ok() == pedido {
                return true;
            }
        } else if i.chave.eq_ignore_ascii_case(b) {
            return true;
        }
        dobrar(&i.titulo).contains(&dobrar(b))
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::demanda::falsas::script;
    use crate::listagem::OrigemIssue;
    use crate::{GithubConfig, JiraConfig};

    fn issue(chave: &str, titulo: &str, estado: &str, rotulos: &[&str]) -> IssueListada {
        IssueListada {
            origem: OrigemIssue::Github,
            chave: chave.into(),
            titulo: titulo.into(),
            descricao: String::new(),
            url: String::new(),
            criada_em: String::new(),
            estado: estado.into(),
            rotulos: rotulos.iter().map(|r| r.to_string()).collect(),
            arquivos: vec![],
        }
    }

    #[test]
    fn filtro_por_estado_rotulo_numero_e_titulo() {
        let a = issue("#41", "Parcelamento calcula errado", "aberta", &["bug"]);
        let f = issue("#7", "Trocar a cor do botão", "fechada", &[]);
        let padrao = FiltroIssues::default();
        assert!(padrao.aceita(&a) && !padrao.aceita(&f));
        let enc = FiltroIssues {
            estado: FiltroEstado::Encerradas,
            ..Default::default()
        };
        assert!(!enc.aceita(&a) && enc.aceita(&f));
        let todas = |busca: &str| FiltroIssues {
            busca: Some(busca.into()),
            estado: FiltroEstado::Todas,
            rotulo: None,
        };
        assert!(todas("41").aceita(&a) && !todas("41").aceita(&f));
        assert!(todas("#7").aceita(&f));
        assert!(!todas("4").aceita(&a), "número é exato");
        assert!(todas("BOTAO").aceita(&f), "sem acento nem maiúscula");
        assert!(todas("parcelamento").aceita(&a));
        let rot = FiltroIssues {
            rotulo: Some("BUG".into()),
            estado: FiltroEstado::Todas,
            ..Default::default()
        };
        assert!(rot.aceita(&a) && !rot.aceita(&f));
        let jira = IssueListada {
            origem: OrigemIssue::Jira,
            ..issue("PROJ-12", "Nota", "aberta", &[])
        };
        assert!(todas("12").aceita(&jira) && todas("proj-12").aceita(&jira));
        let filtro: FiltroIssues = serde_json::from_str(r#"{"estado":"todas"}"#).unwrap();
        assert_eq!(filtro.estado, FiltroEstado::Todas);
    }

    #[test]
    fn atualiza_grava_e_sem_rede_mantem_a_ultima_leitura() {
        let d = tempfile::tempdir().unwrap();
        let marca = d.path().join("offline");
        let gh = script(
            d.path(),
            "gh",
            &format!(
                r#"[ -e "{}" ] && {{ echo "error connecting to api.github.com" >&2; exit 1; }}
echo '[{{"number":3,"title":"C","body":"","url":"u3","createdAt":"2026-10-01T00:00:00Z","state":"OPEN","labels":[]}}]'"#,
                marca.display()
            ),
        );
        let f = Ferramentas {
            gh,
            ..Default::default()
        };
        let t = TrackerConfig::Github(GithubConfig {
            repo: "o/r".into(),
            ..Default::default()
        });
        let arq = arquivo(&d.path().join("issues"), "faturamento/exemplo");
        assert!(arq.ends_with("issues/faturamento_exemplo.json"));
        assert!(ler(&arq, &t).is_none());

        let c = atualizar(&arq, &t, &f).unwrap();
        assert_eq!(c.listagem.issues.len(), 1);
        assert_eq!(ler(&arq, &t).unwrap(), c);
        assert!(!c.vencido(VALIDADE_PADRAO, c.lida_em_ms + 1000));
        assert!(c.vencido(VALIDADE_PADRAO, c.lida_em_ms + 5 * 60 * 1000));

        std::fs::write(&marca, "").unwrap();
        let e = atualizar(&arq, &t, &f).unwrap_err();
        assert!(e.to_string().contains("error connecting"), "{e}");
        let depois = ler(&arq, &t).unwrap();
        assert_eq!(depois.listagem, c.listagem);
        assert_eq!(depois.lida_em_ms, c.lida_em_ms);
        assert!(depois.falha.unwrap().mensagem.contains("error connecting"));

        // Outro rastreador: o cache não vale.
        let outro = TrackerConfig::Jira(JiraConfig {
            url: "https://jira.example.com".into(),
            project_key: "PROJ".into(),
            ..Default::default()
        });
        assert!(ler(&arq, &outro).is_none());
    }

    #[test]
    fn reserva_impede_duas_releituras_do_mesmo_cache() {
        let d = tempfile::tempdir().unwrap();
        let arq = d.path().join("p.json");
        let r = reservar(&arq).unwrap();
        assert!(em_leitura(&arq));
        assert!(reservar(&arq).is_none());
        assert!(reservar(&d.path().join("q.json")).is_some());
        drop(r);
        assert!(!em_leitura(&arq));
        assert!(reservar(&arq).is_some());
    }
}
