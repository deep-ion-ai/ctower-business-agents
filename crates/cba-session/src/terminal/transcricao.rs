//! Se o Claude Code gravou a conversa de uma sessão, para o app só oferecer a
//! retomada pelo identificador (`--resume <id>`) quando ela pode dar certo (#130).
//!
//! O Claude Code grava cada conversa em `<config>/projects/<pasta codificada>/<id>.jsonl`,
//! com `<config>` em `CLAUDE_CONFIG_DIR` ou em `~/.claude`
//! (<https://code.claude.com/docs/en/claude-directory.md>). A codificação do nome da
//! pasta não é documentada; por isso o app procura o `<id>.jsonl` em todas as pastas de
//! `projects`, sem depender dela. Sessões abertas sem gravação (por exemplo, com
//! `CLAUDE_CODE_CHILD_SESSION` herdada de uma sessão de fora) não têm o arquivo, e o
//! `--resume` falha com "No conversation found".

use std::path::PathBuf;

use crate::clientes::Ambiente;

/// Pasta `projects` do Claude Code, ou `None` sem pasta do usuário.
fn pasta_das_conversas(ambiente: &dyn Ambiente) -> Option<PathBuf> {
    let config = match ambiente.var("CLAUDE_CONFIG_DIR").filter(|d| !d.is_empty()) {
        Some(dir) => PathBuf::from(dir),
        None => ambiente.home()?.join(".claude"),
    };
    Some(config.join("projects"))
}

/// A conversa da sessão `id` está gravada e pode ser retomada. Sem como saber (sem
/// pasta do usuário ou sem a pasta `projects`), responde `true`: o app não esconde a
/// retomada por uma dúvida, e o próprio `claude` diz se ela não existe.
pub fn conversa_gravada(ambiente: &dyn Ambiente, id: &str) -> bool {
    let Some(projetos) = pasta_das_conversas(ambiente) else {
        return true;
    };
    if !ambiente.e_pasta(&projetos) {
        return true;
    }
    let arquivo = format!("{id}.jsonl");
    ambiente
        .nomes_na_pasta(&projetos)
        .iter()
        .any(|pasta| ambiente.e_arquivo(&projetos.join(pasta).join(&arquivo)))
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::Path;
    use std::time::Duration;

    use super::*;
    use crate::clientes::{ErroExecucao, Plataforma, Saida};

    struct Falso {
        config: Option<&'static str>,
        home: Option<&'static str>,
        pastas: Vec<&'static str>,
        arquivos: Vec<&'static str>,
    }

    impl Ambiente for Falso {
        fn plataforma(&self) -> Plataforma {
            Plataforma::MacOs
        }
        fn var(&self, nome: &str) -> Option<OsString> {
            (nome == "CLAUDE_CONFIG_DIR")
                .then(|| self.config.map(OsString::from))
                .flatten()
        }
        fn home(&self) -> Option<PathBuf> {
            self.home.map(PathBuf::from)
        }
        fn e_arquivo(&self, caminho: &Path) -> bool {
            self.arquivos.iter().any(|a| Path::new(a) == caminho)
        }
        fn e_pasta(&self, caminho: &Path) -> bool {
            self.pastas.iter().any(|p| Path::new(p) == caminho)
        }
        fn nomes_na_pasta(&self, pasta: &Path) -> Vec<OsString> {
            self.pastas
                .iter()
                .filter_map(|p| Path::new(p).strip_prefix(pasta).ok())
                .filter(|resto| resto.components().count() == 1)
                .map(|resto| resto.as_os_str().to_owned())
                .collect()
        }
        fn executar(&self, _: &Path, _: &[&str], _: Duration) -> Result<Saida, ErroExecucao> {
            unreachable!()
        }
    }

    #[test]
    fn procura_a_conversa_em_todas_as_pastas_de_projects() {
        let ambiente = Falso {
            config: None,
            home: Some("/home/ana"),
            pastas: vec![
                "/home/ana/.claude/projects",
                "/home/ana/.claude/projects/-home-ana-docs",
                "/home/ana/.claude/projects/-home-ana-outro",
            ],
            arquivos: vec!["/home/ana/.claude/projects/-home-ana-docs/abc-1.jsonl"],
        };
        assert!(conversa_gravada(&ambiente, "abc-1"));
        assert!(!conversa_gravada(&ambiente, "f435-sem-gravacao"));
    }

    #[test]
    fn usa_claude_config_dir() {
        let ambiente = Falso {
            config: Some("/cfg"),
            home: Some("/home/ana"),
            pastas: vec!["/cfg/projects", "/cfg/projects/x"],
            arquivos: vec!["/cfg/projects/x/abc.jsonl"],
        };
        assert!(conversa_gravada(&ambiente, "abc"));
    }

    #[test]
    fn sem_como_saber_nao_esconde_a_retomada() {
        let sem_home = Falso {
            config: None,
            home: None,
            pastas: vec![],
            arquivos: vec![],
        };
        assert!(conversa_gravada(&sem_home, "abc"));
        let sem_projects = Falso {
            config: None,
            home: Some("/home/ana"),
            pastas: vec![],
            arquivos: vec![],
        };
        assert!(conversa_gravada(&sem_projects, "abc"));
    }
}
