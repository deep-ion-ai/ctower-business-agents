//! Catálogo das conexões cadastradas nas Configurações do app (#437).
//!
//! As conexões ficam num arquivo da configuração do app
//! (`<dados>/config/conexoes.json`), nunca na base do projeto. Cada processo
//! (a janela e o servidor MCP) aponta o catálogo para esse arquivo na abertura
//! ([`Catalogo::usar_arquivo`]) e o relê quando ele muda, então uma conexão
//! editada na janela vale também para as ferramentas do agente. Os
//! consumidores (busca de issues, clone, publicação, prontidão) encontram a
//! conexão referenciada pelo projeto com [`Catalogo::obter`], sem saber onde
//! ela fica gravada.

use super::Conexao;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};
use std::time::SystemTime;

/// Forma gravada do arquivo de conexões.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArquivoConexoes {
    #[serde(default)]
    pub conexoes: Vec<Conexao>,
}

impl ArquivoConexoes {
    /// Lê o arquivo; sem ele, a lista vazia.
    pub fn ler(arquivo: &Path) -> Result<Self, String> {
        match std::fs::read(arquivo) {
            Ok(b) => serde_json::from_slice(&b)
                .map_err(|e| format!("arquivo de conexões ilegível ({}): {e}", arquivo.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(format!(
                "arquivo de conexões inacessível ({}): {e}",
                arquivo.display()
            )),
        }
    }
}

#[derive(Debug, Default)]
struct Estado {
    conexoes: Vec<Conexao>,
    arquivo: Option<PathBuf>,
    lido_em: Option<SystemTime>,
}

/// Lista de conexões do processo.
#[derive(Debug, Default)]
pub struct Catalogo {
    estado: RwLock<Estado>,
}

fn modificado(arquivo: &Path) -> Option<SystemTime> {
    std::fs::metadata(arquivo).and_then(|m| m.modified()).ok()
}

impl Catalogo {
    /// O catálogo do processo.
    pub fn global() -> &'static Catalogo {
        static C: OnceLock<Catalogo> = OnceLock::new();
        C.get_or_init(Catalogo::default)
    }

    /// Passa a ler as conexões de `arquivo` (e a relê-lo quando ele muda).
    pub fn usar_arquivo(&self, arquivo: PathBuf) -> Result<(), String> {
        let lido = ArquivoConexoes::ler(&arquivo)?;
        if let Ok(mut e) = self.estado.write() {
            e.lido_em = modificado(&arquivo);
            e.arquivo = Some(arquivo);
            e.conexoes = lido.conexoes;
        }
        Ok(())
    }

    /// Relê o arquivo se ele mudou depois da última leitura.
    fn atualizar(&self) {
        let (arquivo, lido_em) = match self.estado.read() {
            Ok(e) => match &e.arquivo {
                Some(a) => (a.clone(), e.lido_em),
                None => return,
            },
            Err(_) => return,
        };
        let agora = modificado(&arquivo);
        if agora == lido_em {
            return;
        }
        // Arquivo no meio de uma gravação ou corrompido: fica a última lista
        // lida, e a próxima chamada tenta de novo.
        if let Ok(l) = ArquivoConexoes::ler(&arquivo)
            && let Ok(mut e) = self.estado.write()
        {
            e.conexoes = l.conexoes;
            e.lido_em = agora;
        }
    }

    /// Troca a lista inteira (gravação nas Configurações).
    pub fn definir(&self, conexoes: Vec<Conexao>) {
        if let Ok(mut e) = self.estado.write() {
            e.lido_em = e.arquivo.as_deref().and_then(modificado);
            e.conexoes = conexoes;
        }
    }

    /// Acrescenta ou troca uma conexão pelo `id` (testes).
    pub fn gravar(&self, conexao: Conexao) {
        if let Ok(mut e) = self.estado.write() {
            match e.conexoes.iter_mut().find(|x| x.id == conexao.id) {
                Some(x) => *x = conexao,
                None => e.conexoes.push(conexao),
            }
        }
    }

    /// A conexão com o `id`, se cadastrada.
    pub fn obter(&self, id: &str) -> Option<Conexao> {
        self.atualizar();
        let id = id.trim();
        self.estado
            .read()
            .ok()
            .and_then(|e| e.conexoes.iter().find(|x| x.id == id).cloned())
    }

    /// Todas as conexões, na ordem do cadastro.
    pub fn todas(&self) -> Vec<Conexao> {
        self.atualizar();
        self.estado
            .read()
            .map(|e| e.conexoes.clone())
            .unwrap_or_default()
    }
}

/// A conexão referenciada por um projeto, ou o erro em pt-BR quando ela não
/// está mais cadastrada.
pub fn conexao_referenciada(id: &str) -> Result<Conexao, String> {
    Catalogo::global().obter(id).ok_or_else(|| {
        format!(
            "a conexão \"{}\" não está cadastrada: escolha outra em Configurações › Conexões",
            id.trim()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::FonteCredencial;

    fn c(id: &str) -> Conexao {
        Conexao {
            id: id.into(),
            nome: id.into(),
            tipo: "gitlab".into(),
            endereco: "gitlab.example.com".into(),
            credencial: FonteCredencial::Nenhuma,
            opcoes: Default::default(),
        }
    }

    #[test]
    fn rele_o_arquivo_quando_ele_muda() {
        let d = tempfile::tempdir().unwrap();
        let arq = d.path().join("conexoes.json");
        let cat = Catalogo::default();
        cat.usar_arquivo(arq.clone()).unwrap();
        assert!(cat.todas().is_empty());
        let um = ArquivoConexoes {
            conexoes: vec![c("a")],
        };
        std::fs::write(&arq, serde_json::to_vec(&um).unwrap()).unwrap();
        assert_eq!(cat.obter("a"), Some(c("a")));
        assert_eq!(cat.obter("b"), None);
        cat.gravar(c("b"));
        assert!(cat.obter("b").is_some());
    }

    #[test]
    fn arquivo_ilegivel_e_erro_em_portugues() {
        let d = tempfile::tempdir().unwrap();
        let arq = d.path().join("conexoes.json");
        std::fs::write(&arq, "{").unwrap();
        let e = Catalogo::default().usar_arquivo(arq).unwrap_err();
        assert!(e.contains("ilegível"), "{e}");
    }
}
