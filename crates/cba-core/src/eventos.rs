//! Notificação ao app a cada mudança numa análise, para os painéis ao vivo.
//!
//! O servidor MCP e o receptor de hooks são processos separados do app (spec, seção
//! 5). Cada mudança gravada vira uma linha JSON acrescentada ao diário do projeto,
//! em `<dados>/eventos/<projeto>.jsonl`; o app acompanha o arquivo e relê a análise
//! citada. O diário guarda só identificadores (qual análise, que tipo de mudança,
//! versão, processo), nunca conteúdo do sistema analisado.
//!
//! Falha ao notificar nunca desfaz nem impede a gravação: o estado já está na base,
//! e o app o encontra ao reler.
//!
//! # Leitura e compactação
//!
//! O app acompanha o diário com um [`LeitorDiario`]: cada leitura devolve só as
//! linhas acrescentadas desde a anterior. Como o diário só serve para avisar o app
//! (o estado completo está na base), ele não precisa guardar histórico: quando o
//! leitor já leu tudo e o arquivo passou de [`LIMITE_DO_DIARIO`], ele o esvazia,
//! sob a mesma trava que os processos usam para acrescentar linhas. Assim nenhuma
//! linha é perdida entre a leitura e o esvaziamento, e o diário não cresce sem
//! limite. Um leitor que encontra o arquivo menor do que a posição em que parou
//! (outro leitor o esvaziou) recomeça do início e avisa com
//! [`Leitura::reiniciado`], para quem o usa reler o estado.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Tipo de mudança numa análise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mudanca {
    /// Análise criada.
    Criada,
    /// Mudança de estado da análise (concluir, aprovar).
    Estado,
    /// Sessão ligada ou com estado novo.
    Sessao,
    Afirmacao,
    Pergunta,
    Sugestao,
    /// Termo proposto para o glossário.
    Termo,
    /// História de usuário ou critério de aceite registrado, decidido ou editado
    /// (fluxo `historias`).
    Historia,
    /// Arquivos da pasta do projeto alterados por uma sessão ligada à análise (ou
    /// alteração desfeita ou mantida pelo analista). Não muda o estado da análise:
    /// a `versao` do evento é 0, e a tela relê o aviso de alteração
    /// (`cba_session::alteracoes`), não a análise.
    Alteracao,
    /// Transcript de uma reunião guardado ou apagado (descarte ou retenção). O
    /// campo `analise` do evento traz o identificador da reunião (`R-0001`), e a
    /// `versao` é 0: a tela relê a situação da guarda (`cba_reunioes::guarda`).
    Transcript,
    /// Ata de uma reunião alterada (item registrado, papel proposto). O campo
    /// `analise` do evento traz o identificador da reunião (`R-0001`), e a
    /// `versao` é 0: a tela relê o estado da ata (`R-NNNN.estado.json`).
    Ata,
    /// Parecer de revisão alterado (achado, INVEST ou veredito registrado,
    /// achado descartado pelo analista).
    Parecer,
}

/// Uma linha do diário.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evento {
    pub quando_ms: u64,
    /// Slug do projeto.
    pub projeto: String,
    /// Identificador da análise (`A-0001`), ou da reunião (`R-0001`) nas
    /// mudanças [`Mudanca::Transcript`] e [`Mudanca::Ata`].
    pub analise: String,
    pub mudanca: Mudanca,
    /// Versão da análise depois da mudança.
    pub versao: u64,
    /// Processo que gravou (app, `cba mcp` ou `cba hook`).
    pub processo: u32,
}

/// Destino das notificações.
pub trait Notificador: Send + Sync + std::fmt::Debug {
    fn notificar(&self, evento: &Evento) -> io::Result<()>;
}

/// Não notifica ninguém (testes e ferramentas de linha de comando).
#[derive(Debug, Clone, Copy, Default)]
pub struct SemNotificacao;

impl Notificador for SemNotificacao {
    fn notificar(&self, _: &Evento) -> io::Result<()> {
        Ok(())
    }
}

/// Diário de eventos em arquivos JSONL, um por projeto.
#[derive(Debug, Clone)]
pub struct DiarioEventos {
    pasta: PathBuf,
}

impl DiarioEventos {
    /// Diário na pasta informada (normalmente `DiretorioDados::eventos`).
    pub fn em(pasta: impl Into<PathBuf>) -> Self {
        Self {
            pasta: pasta.into(),
        }
    }

    /// Arquivo do diário de um projeto.
    pub fn arquivo(&self, projeto: &str) -> PathBuf {
        self.pasta.join(format!("{projeto}.jsonl"))
    }

    /// Eventos já gravados de um projeto, na ordem. Linhas ilegíveis (uma gravação
    /// interrompida) são ignoradas.
    pub fn ler(&self, projeto: &str) -> io::Result<Vec<Evento>> {
        ler(&self.arquivo(projeto))
    }

    /// Leitor das linhas que forem acrescentadas daqui em diante ao diário de um
    /// projeto (o que já está no arquivo é ignorado: o estado atual está na base).
    pub fn leitor(&self, projeto: &str) -> LeitorDiario {
        let arquivo = self.arquivo(projeto);
        let posicao = fs::metadata(&arquivo).map(|m| m.len()).unwrap_or(0);
        LeitorDiario {
            arquivo,
            posicao,
            limite: LIMITE_DO_DIARIO,
        }
    }
}

/// Tamanho a partir do qual o leitor esvazia o diário depois de lê-lo (1 MiB, uns
/// 8 mil eventos).
pub const LIMITE_DO_DIARIO: u64 = 1024 * 1024;

/// Resultado de [`LeitorDiario::ler_novos`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Leitura {
    /// Eventos acrescentados desde a leitura anterior, na ordem.
    pub eventos: Vec<Evento>,
    /// O arquivo encolheu ou sumiu desde a leitura anterior (outro leitor o
    /// esvaziou): eventos podem ter se perdido, e quem lê deve reler o estado.
    pub reiniciado: bool,
}

/// Acompanha o diário de um projeto, lendo só o que foi acrescentado.
#[derive(Debug, Clone)]
pub struct LeitorDiario {
    arquivo: PathBuf,
    posicao: u64,
    limite: u64,
}

impl LeitorDiario {
    /// Troca o tamanho a partir do qual o diário é esvaziado (testes).
    pub fn com_limite(mut self, limite: u64) -> Self {
        self.limite = limite;
        self
    }

    /// Posição (em bytes) até onde o diário já foi lido.
    pub fn posicao(&self) -> u64 {
        self.posicao
    }

    /// Lê as linhas acrescentadas desde a leitura anterior. Sem mudança no tamanho
    /// do arquivo, não abre nem trava nada (é chamado a cada poucos milissegundos).
    pub fn ler_novos(&mut self) -> io::Result<Leitura> {
        let tamanho = match fs::metadata(&self.arquivo) {
            Ok(m) => m.len(),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                let reiniciado = self.posicao > 0;
                self.posicao = 0;
                return Ok(Leitura {
                    eventos: Vec::new(),
                    reiniciado,
                });
            }
            Err(e) => return Err(e),
        };
        if tamanho == self.posicao {
            return Ok(Leitura::default());
        }
        // Leitura e escrita: a trava no Windows exige uma das duas, e o esvaziamento
        // precisa da escrita.
        let mut arquivo = match OpenOptions::new()
            .read(true)
            .write(true)
            .open(&self.arquivo)
        {
            Ok(a) => a,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Leitura::default()),
            Err(e) => return Err(e),
        };
        arquivo.lock()?;
        self.ler_travado(&mut arquivo)
    }

    fn ler_travado(&mut self, arquivo: &mut File) -> io::Result<Leitura> {
        let mut leitura = Leitura::default();
        let tamanho = arquivo.metadata()?.len();
        if tamanho < self.posicao {
            self.posicao = 0;
            leitura.reiniciado = true;
        }
        arquivo.seek(SeekFrom::Start(self.posicao))?;
        let mut bytes = Vec::new();
        arquivo.read_to_end(&mut bytes)?;
        // Só linhas completas; uma gravação interrompida fica para a próxima leitura
        // (e, se nunca terminar, é ignorada como linha ilegível quando terminar).
        let consumido = bytes.iter().rposition(|&b| b == b'\n').map_or(0, |i| i + 1);
        leitura.eventos = String::from_utf8_lossy(&bytes[..consumido])
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect();
        self.posicao += consumido as u64;
        if self.posicao >= self.limite && self.posicao == tamanho {
            // Tudo lido e ninguém grava enquanto a trava é nossa: pode esvaziar.
            arquivo.set_len(0)?;
            self.posicao = 0;
        }
        Ok(leitura)
    }
}

fn ler(arquivo: &Path) -> io::Result<Vec<Evento>> {
    let texto = match fs::read_to_string(arquivo) {
        Ok(t) => t,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    Ok(texto
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect())
}

impl Notificador for DiarioEventos {
    fn notificar(&self, evento: &Evento) -> io::Result<()> {
        fs::create_dir_all(&self.pasta)?;
        let mut linha = serde_json::to_vec(evento).map_err(io::Error::other)?;
        linha.push(b'\n');
        // No Windows, a trava (LockFileEx) exige acesso de leitura ou de escrita
        // genérico, e o modo só de acréscimo não tem nenhum dos dois: sem o
        // `read(true)`, `lock()` falha com acesso negado.
        let mut arquivo = OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .open(self.arquivo(&evento.projeto))?;
        // A trava exclusiva garante uma linha inteira por vez, mesmo com vários
        // processos gravando; é liberada quando o arquivo é fechado.
        arquivo.lock()?;
        arquivo.write_all(&linha)?;
        arquivo.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn evento(n: u64) -> Evento {
        Evento {
            quando_ms: n,
            projeto: "faturamento-exemplo".into(),
            analise: "A-0001".into(),
            mudanca: Mudanca::Afirmacao,
            versao: n,
            processo: std::process::id(),
        }
    }

    #[test]
    fn acrescenta_uma_linha_por_evento() {
        let dir = tempfile::tempdir().unwrap();
        let diario = DiarioEventos::em(dir.path().join("eventos"));
        assert!(diario.ler("faturamento-exemplo").unwrap().is_empty());
        diario.notificar(&evento(1)).unwrap();
        diario.notificar(&evento(2)).unwrap();
        let lidos = diario.ler("faturamento-exemplo").unwrap();
        assert_eq!(lidos, vec![evento(1), evento(2)]);
        let texto = fs::read_to_string(diario.arquivo("faturamento-exemplo")).unwrap();
        assert!(texto.contains(r#""mudanca":"afirmacao""#), "{texto}");
    }

    #[test]
    fn leitor_devolve_so_o_que_foi_acrescentado() {
        let dir = tempfile::tempdir().unwrap();
        let diario = DiarioEventos::em(dir.path());
        diario.notificar(&evento(1)).unwrap();
        let mut leitor = diario.leitor("faturamento-exemplo");
        assert_eq!(leitor.ler_novos().unwrap(), Leitura::default());
        diario.notificar(&evento(2)).unwrap();
        diario.notificar(&evento(3)).unwrap();
        assert_eq!(
            leitor.ler_novos().unwrap().eventos,
            vec![evento(2), evento(3)]
        );
        assert!(leitor.ler_novos().unwrap().eventos.is_empty());
    }

    #[test]
    fn leitor_sem_arquivo_espera_o_primeiro_evento() {
        let dir = tempfile::tempdir().unwrap();
        let diario = DiarioEventos::em(dir.path().join("eventos"));
        let mut leitor = diario.leitor("faturamento-exemplo");
        assert_eq!(leitor.ler_novos().unwrap(), Leitura::default());
        diario.notificar(&evento(1)).unwrap();
        assert_eq!(leitor.ler_novos().unwrap().eventos, vec![evento(1)]);
    }

    #[test]
    fn linha_incompleta_fica_para_a_proxima_leitura() {
        let dir = tempfile::tempdir().unwrap();
        let diario = DiarioEventos::em(dir.path());
        let mut leitor = diario.leitor("faturamento-exemplo");
        let linha = serde_json::to_string(&evento(1)).unwrap();
        let (a, b) = linha.split_at(10);
        let caminho = diario.arquivo("faturamento-exemplo");
        fs::write(&caminho, a).unwrap();
        assert!(leitor.ler_novos().unwrap().eventos.is_empty());
        let mut f = OpenOptions::new().append(true).open(&caminho).unwrap();
        writeln!(f, "{b}").unwrap();
        assert_eq!(leitor.ler_novos().unwrap().eventos, vec![evento(1)]);
    }

    #[test]
    fn leitor_esvazia_o_diario_acima_do_limite_sem_perder_eventos() {
        let dir = tempfile::tempdir().unwrap();
        let diario = DiarioEventos::em(dir.path());
        let mut leitor = diario.leitor("faturamento-exemplo").com_limite(500);
        let mut recebidos = Vec::new();
        for i in 0..40 {
            diario.notificar(&evento(i)).unwrap();
            let leitura = leitor.ler_novos().unwrap();
            assert!(!leitura.reiniciado);
            recebidos.extend(leitura.eventos);
            let tamanho = fs::metadata(diario.arquivo("faturamento-exemplo"))
                .unwrap()
                .len();
            assert!(
                tamanho < 500 + 200,
                "o diário cresceu sem limite: {tamanho}"
            );
        }
        assert_eq!(recebidos, (0..40).map(evento).collect::<Vec<_>>());
    }

    #[test]
    fn diario_esvaziado_por_outro_leitor_reinicia() {
        let dir = tempfile::tempdir().unwrap();
        let diario = DiarioEventos::em(dir.path());
        let mut leitor = diario.leitor("faturamento-exemplo");
        diario.notificar(&evento(1)).unwrap();
        diario.notificar(&evento(2)).unwrap();
        leitor.ler_novos().unwrap();
        fs::write(diario.arquivo("faturamento-exemplo"), "").unwrap();
        diario.notificar(&evento(3)).unwrap();
        let leitura = leitor.ler_novos().unwrap();
        assert!(leitura.reiniciado);
        assert_eq!(leitura.eventos, vec![evento(3)]);
        fs::remove_file(diario.arquivo("faturamento-exemplo")).unwrap();
        assert!(leitor.ler_novos().unwrap().reiniciado);
    }

    #[test]
    fn varias_threads_nao_misturam_linhas() {
        let dir = tempfile::tempdir().unwrap();
        let diario = DiarioEventos::em(dir.path());
        std::thread::scope(|s| {
            for t in 0..4 {
                let diario = diario.clone();
                s.spawn(move || {
                    for i in 0..50 {
                        diario.notificar(&evento(t * 1000 + i)).unwrap();
                    }
                });
            }
        });
        let texto = fs::read_to_string(diario.arquivo("faturamento-exemplo")).unwrap();
        assert_eq!(texto.lines().count(), 200);
        assert_eq!(diario.ler("faturamento-exemplo").unwrap().len(), 200);
    }
}
