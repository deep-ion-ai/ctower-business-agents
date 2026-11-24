//! Histórico de atividade de cada projeto, persistente e com rotação (#331).
//!
//! O diário de eventos ([`crate::eventos`]) só avisa as telas abertas: a vigia do
//! app o esvazia depois de ler, então ele não serve para mostrar "o que aconteceu"
//! depois de reiniciar o app. O histórico recebe a mesma linha de cada evento,
//! gravada pelo mesmo processo que a gravou no diário (app, `cba mcp` ou `cba
//! hook`), em `<dados>/historico/<projeto>.jsonl`, e ninguém o esvazia.
//!
//! Como o diário, guarda só identificadores (qual análise, que tipo de mudança,
//! versão, estado), nunca conteúdo do sistema analisado.
//!
//! # Rotação
//!
//! Quando o arquivo de um projeto passa de [`LIMITE_DO_HISTORICO`], ele vira
//! `<projeto>.1.jsonl` (substituindo o anterior) e um arquivo novo começa. O
//! histórico de um projeto ocupa no máximo uns dois limites, e a leitura das últimas
//! linhas olha o arquivo anterior quando o atual ainda tem poucas. Acrescentar e
//! girar acontecem sob a trava `<dados>/historico/.trava`, então nenhuma linha se
//! perde entre a rotação e a gravação de outro processo.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::eventos::Evento;

/// Tamanho a partir do qual o arquivo do projeto gira (256 KiB, uns 2 mil eventos).
pub const LIMITE_DO_HISTORICO: u64 = 256 * 1024;

/// Arquivo de trava, na pasta do histórico.
const TRAVA: &str = ".trava";

/// Histórico de atividade, um arquivo JSONL por projeto.
#[derive(Debug, Clone)]
pub struct Historico {
    pasta: PathBuf,
    limite: u64,
}

impl Historico {
    /// Histórico na pasta informada (normalmente `DiretorioDados::historico`).
    pub fn em(pasta: impl Into<PathBuf>) -> Self {
        Self {
            pasta: pasta.into(),
            limite: LIMITE_DO_HISTORICO,
        }
    }

    /// Troca o tamanho a partir do qual o arquivo gira (testes).
    pub fn com_limite(mut self, limite: u64) -> Self {
        self.limite = limite;
        self
    }

    pub fn pasta(&self) -> &Path {
        &self.pasta
    }

    /// Arquivo atual do histórico de um projeto.
    pub fn arquivo(&self, projeto: &str) -> PathBuf {
        self.pasta.join(format!("{projeto}.jsonl"))
    }

    /// Arquivo anterior (o que girou por último).
    pub fn anterior(&self, projeto: &str) -> PathBuf {
        self.pasta.join(format!("{projeto}.1.jsonl"))
    }

    /// Acrescenta um evento ao histórico do projeto, girando o arquivo antes se ele
    /// passou do limite.
    pub fn registrar(&self, evento: &Evento) -> io::Result<()> {
        let mut linha = serde_json::to_vec(evento).map_err(io::Error::other)?;
        linha.push(b'\n');
        let _trava = self.travar()?;
        let atual = self.arquivo(&evento.projeto);
        if fs::metadata(&atual).is_ok_and(|m| m.len() >= self.limite) {
            fs::rename(&atual, self.anterior(&evento.projeto))?;
        }
        let mut arquivo = OpenOptions::new().create(true).append(true).open(&atual)?;
        arquivo.write_all(&linha)?;
        arquivo.flush()
    }

    /// Os últimos `n` eventos do projeto (ou menos, se não houver tantos), do mais
    /// antigo para o mais recente. Linhas ilegíveis (uma gravação interrompida) são
    /// ignoradas.
    pub fn ultimos(&self, projeto: &str, n: usize) -> io::Result<Vec<Evento>> {
        let mut eventos = ler(&self.arquivo(projeto))?;
        if eventos.len() < n {
            let mut antes = ler(&self.anterior(projeto))?;
            antes.append(&mut eventos);
            eventos = antes;
        }
        let corte = eventos.len().saturating_sub(n);
        Ok(eventos.split_off(corte))
    }

    fn travar(&self) -> io::Result<File> {
        fs::create_dir_all(&self.pasta)?;
        let arquivo = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(self.pasta.join(TRAVA))?;
        // Liberada quando o arquivo é fechado.
        arquivo.lock()?;
        Ok(arquivo)
    }
}

fn ler(arquivo: &Path) -> io::Result<Vec<Evento>> {
    let texto = match fs::read(arquivo) {
        Ok(t) => t,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    Ok(String::from_utf8_lossy(&texto)
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eventos::Mudanca;

    fn evento(n: u64) -> Evento {
        Evento {
            quando_ms: n,
            projeto: "faturamento-exemplo".into(),
            analise: "A-0001".into(),
            mudanca: Mudanca::Afirmacao,
            versao: n,
            processo: std::process::id(),
            estado: None,
        }
    }

    #[test]
    fn sem_arquivo_nao_ha_eventos() {
        let dir = tempfile::tempdir().unwrap();
        let h = Historico::em(dir.path().join("historico"));
        assert!(h.ultimos("faturamento-exemplo", 20).unwrap().is_empty());
    }

    #[test]
    fn devolve_os_ultimos_na_ordem() {
        let dir = tempfile::tempdir().unwrap();
        let h = Historico::em(dir.path());
        for i in 0..5 {
            h.registrar(&evento(i)).unwrap();
        }
        let ultimos = h.ultimos("faturamento-exemplo", 3).unwrap();
        assert_eq!(ultimos, (2..5).map(evento).collect::<Vec<_>>());
        assert_eq!(h.ultimos("faturamento-exemplo", 10).unwrap().len(), 5);
        assert!(h.ultimos("outro", 10).unwrap().is_empty());
    }

    #[test]
    fn gira_acima_do_limite_sem_perder_os_recentes() {
        let dir = tempfile::tempdir().unwrap();
        // Cada arquivo guarda 15 linhas antes de girar.
        let linha = serde_json::to_vec(&evento(10)).unwrap().len() as u64 + 1;
        let h = Historico::em(dir.path()).com_limite(linha * 15);
        for i in 10..110 {
            h.registrar(&evento(i)).unwrap();
            let tamanho = fs::metadata(h.arquivo("faturamento-exemplo"))
                .unwrap()
                .len();
            assert!(
                tamanho <= linha * 16,
                "o histórico cresceu sem limite: {tamanho}"
            );
        }
        assert!(h.anterior("faturamento-exemplo").exists());
        // Os últimos 20 atravessam a rotação: parte no anterior, parte no atual.
        let atual = ler(&h.arquivo("faturamento-exemplo")).unwrap().len();
        assert!(atual < 20, "{atual}");
        assert_eq!(
            h.ultimos("faturamento-exemplo", 20).unwrap(),
            (90..110).map(evento).collect::<Vec<_>>()
        );
    }

    #[test]
    fn linha_ilegivel_e_ignorada() {
        let dir = tempfile::tempdir().unwrap();
        let h = Historico::em(dir.path());
        h.registrar(&evento(1)).unwrap();
        let mut f = OpenOptions::new()
            .append(true)
            .open(h.arquivo("faturamento-exemplo"))
            .unwrap();
        writeln!(f, "{{\"quando_ms\":").unwrap();
        h.registrar(&evento(2)).unwrap();
        assert_eq!(
            h.ultimos("faturamento-exemplo", 20).unwrap(),
            vec![evento(1), evento(2)]
        );
    }

    #[test]
    fn varias_threads_nao_misturam_linhas() {
        let dir = tempfile::tempdir().unwrap();
        let h = Historico::em(dir.path()).com_limite(4000);
        std::thread::scope(|s| {
            for t in 0..4 {
                let h = h.clone();
                s.spawn(move || {
                    for i in 0..50 {
                        h.registrar(&evento(t * 1000 + i)).unwrap();
                    }
                });
            }
        });
        // Com rotação, só os dois últimos arquivos ficam; todas as linhas inteiras.
        for arquivo in [
            h.anterior("faturamento-exemplo"),
            h.arquivo("faturamento-exemplo"),
        ] {
            let texto = fs::read_to_string(&arquivo).unwrap_or_default();
            for linha in texto.lines() {
                serde_json::from_str::<Evento>(linha).expect(linha);
            }
        }
        assert_eq!(
            h.ultimos("faturamento-exemplo", 1).unwrap().len(),
            1,
            "o último evento está no histórico"
        );
    }
}
