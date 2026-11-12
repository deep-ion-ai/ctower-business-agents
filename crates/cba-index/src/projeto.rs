//! Cadastro de um projeto e o escopo de caminhos que as ferramentas podem ler.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::Erro;
use crate::exclusoes::{Exclusao, Exclusoes, Motivo};

/// Uma pasta local do projeto.
///
/// No máximo uma pasta fica sem prefixo: é a raiz, e os caminhos dentro dela
/// aparecem sem nada na frente (`db/01_tabelas.sql`). As demais aparecem sob o
/// prefixo (`telas/UFrmCancelamento.pas` para a pasta de prefixo `telas`). O
/// prefixo é fixo depois do cadastro, para os caminhos das evidências não mudarem
/// quando o analista acrescenta uma pasta.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pasta {
    /// Caminho absoluto, canônico.
    pub caminho: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefixo: Option<String>,
}

/// Projeto cadastrado: nome, pastas e ajustes de exclusão (spec, seção 4.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Projeto {
    /// Identificador legível e estável, derivado do nome (`faturamento-exemplo`).
    /// Dá nome à pasta do projeto na base de conhecimento e no cadastro.
    pub id: String,
    pub nome: String,
    pub pastas: Vec<Pasta>,
    /// Exclusões acrescentadas pelo analista (padrões descritos em [`Exclusoes`]).
    #[serde(default)]
    pub exclusoes: Vec<String>,
    /// Padrões reincluídos pelo analista, com precedência sobre as exclusões.
    #[serde(default)]
    pub inclusoes: Vec<String>,
}

impl Projeto {
    /// Monta um projeto novo a partir do nome e das pastas.
    ///
    /// A primeira pasta é a raiz (sem prefixo); as demais recebem o nome da pasta
    /// como prefixo, com sufixo numérico se repetir. Os caminhos são guardados
    /// canônicos, sem o prefixo `\\?\` do Windows.
    pub fn novo<P: AsRef<Path>>(
        nome: &str,
        pastas: impl IntoIterator<Item = P>,
    ) -> Result<Self, Erro> {
        let nome = nome.trim();
        if nome.is_empty() {
            return Err(Erro::NomeVazio);
        }
        let mut usados = HashSet::new();
        let mut lista = Vec::new();
        for (i, p) in pastas.into_iter().enumerate() {
            let caminho = canonica(p.as_ref())?;
            let prefixo = if i == 0 {
                None
            } else {
                let base = caminho
                    .file_name()
                    .map(|n| slug(&n.to_string_lossy()))
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| "pasta".into());
                let mut candidato = base.clone();
                let mut n = 2;
                while !usados.insert(candidato.clone()) {
                    candidato = format!("{base}-{n}");
                    n += 1;
                }
                Some(candidato)
            };
            lista.push(Pasta { caminho, prefixo });
        }
        let projeto = Self {
            id: slug(nome),
            nome: nome.to_string(),
            pastas: lista,
            exclusoes: Vec::new(),
            inclusoes: Vec::new(),
        };
        projeto.validar()?;
        Ok(projeto)
    }

    /// Confere o cadastro: identificador, pastas existentes e sem sobreposição,
    /// prefixos válidos e padrões de exclusão que compilam.
    pub fn validar(&self) -> Result<(), Erro> {
        if self.nome.trim().is_empty() {
            return Err(Erro::NomeVazio);
        }
        if !id_valido(&self.id) {
            return Err(Erro::IdInvalido(self.id.clone()));
        }
        if self.pastas.is_empty() {
            return Err(Erro::SemPastas);
        }
        let mut prefixos = HashSet::new();
        let mut sem_prefixo = None;
        for p in &self.pastas {
            if !p.caminho.exists() {
                return Err(Erro::PastaNaoEncontrada(p.caminho.clone()));
            }
            if !p.caminho.is_dir() {
                return Err(Erro::NaoEhPasta(p.caminho.clone()));
            }
            match &p.prefixo {
                None if sem_prefixo.is_some() => return Err(Erro::VariasPastasSemPrefixo),
                None => sem_prefixo = Some(&p.caminho),
                Some(x) if !id_valido(x) || !prefixos.insert(x.as_str()) => {
                    return Err(Erro::PrefixoInvalido(x.clone()));
                }
                Some(_) => {}
            }
        }
        for (i, a) in self.pastas.iter().enumerate() {
            for b in &self.pastas[i + 1..] {
                if a.caminho.starts_with(&b.caminho) || b.caminho.starts_with(&a.caminho) {
                    return Err(Erro::PastasSobrepostas(
                        a.caminho.clone(),
                        b.caminho.clone(),
                    ));
                }
            }
        }
        // Um item da raiz com o mesmo nome de um prefixo ficaria escondido.
        if let Some(raiz) = sem_prefixo {
            for x in &prefixos {
                let conflito = fs::read_dir(raiz)
                    .map_err(|e| Erro::io(raiz, e))?
                    .flatten()
                    .any(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(x));
                if conflito {
                    return Err(Erro::PrefixoConflitante {
                        prefixo: x.to_string(),
                        raiz: raiz.clone(),
                    });
                }
            }
        }
        Exclusoes::novas(&self.exclusoes, &self.inclusoes)?;
        Ok(())
    }

    /// Exclusões compiladas do projeto (padrão + ajustes do analista).
    pub fn regras_de_exclusao(&self) -> Result<Exclusoes, Erro> {
        Exclusoes::novas(&self.exclusoes, &self.inclusoes)
    }

    /// Escopo de leitura do projeto: resolve caminhos relativos e aplica as
    /// exclusões. Falha se alguma pasta não existe mais.
    pub fn escopo(&self) -> Result<Escopo, Erro> {
        self.validar()?;
        let pastas = self
            .pastas
            .iter()
            .map(|p| {
                Ok(Pasta {
                    caminho: canonica(&p.caminho)?,
                    prefixo: p.prefixo.clone(),
                })
            })
            .collect::<Result<_, Erro>>()?;
        Ok(Escopo {
            pastas,
            exclusoes: self.regras_de_exclusao()?,
        })
    }
}

/// Por que um caminho foi recusado pelo [`Escopo`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Recusa {
    #[error("caminho inválido: {0} (use um caminho relativo à raiz do projeto, com /)")]
    CaminhoInvalido(String),
    #[error("caminho fora das pastas do projeto")]
    ForaDoProjeto,
    #[error("caminho excluído da análise ({})", .0.motivo.descricao())]
    Excluido(Exclusao),
    #[error("arquivo não encontrado")]
    NaoEncontrado,
    #[error("não é um arquivo")]
    NaoEhArquivo,
}

/// Pastas e exclusões de um projeto, prontas para resolver caminhos.
///
/// É o que as ferramentas MCP e a validação de evidências (#20) usam para garantir
/// que só leem dentro das pastas do projeto e fora das exclusões (spec, seção 6.1).
#[derive(Debug, Clone)]
pub struct Escopo {
    pastas: Vec<Pasta>,
    exclusoes: Exclusoes,
}

impl Escopo {
    pub fn pastas(&self) -> &[Pasta] {
        &self.pastas
    }

    pub fn exclusoes(&self) -> &Exclusoes {
        &self.exclusoes
    }

    /// Resolve um caminho relativo à raiz do projeto para o arquivo no disco.
    ///
    /// Recusa caminhos absolutos ou com `..`, fora das pastas do projeto (inclusive
    /// por link simbólico), excluídos (pelos padrões) e inexistentes. Aceita `\`
    /// como separador. Não lê o conteúdo: binário pelo conteúdo fica para quem lê.
    pub fn resolver(&self, relativo: &str) -> Result<PathBuf, Recusa> {
        let rel = normalizar(relativo).ok_or_else(|| Recusa::CaminhoInvalido(relativo.into()))?;
        if let Some(e) = self.exclusoes.avaliar_caminho(&rel) {
            return Err(Recusa::Excluido(e));
        }
        let (pasta, interno) = self.localizar(&rel).ok_or(Recusa::ForaDoProjeto)?;
        let caminho = interno
            .split('/')
            .fold(pasta.caminho.clone(), |c, parte| c.join(parte));
        let meta = fs::symlink_metadata(&caminho).map_err(|_| Recusa::NaoEncontrado)?;
        if meta.file_type().is_symlink() {
            return Err(Recusa::Excluido(Exclusao {
                motivo: Motivo::LinkSimbolico,
                regra: None,
            }));
        }
        // Uma pasta intermediária pode ser um link para fora do projeto.
        let canonico = dunce::canonicalize(&caminho).map_err(|_| Recusa::NaoEncontrado)?;
        if !canonico.starts_with(&pasta.caminho) {
            return Err(Recusa::ForaDoProjeto);
        }
        if !meta.is_file() {
            return Err(Recusa::NaoEhArquivo);
        }
        Ok(canonico)
    }

    /// Caminho relativo à raiz do projeto de um caminho absoluto, se ele estiver
    /// dentro de alguma pasta do projeto. Não aplica as exclusões.
    pub fn relativo(&self, absoluto: &Path) -> Option<String> {
        let absoluto = dunce::canonicalize(absoluto).unwrap_or_else(|_| absoluto.to_path_buf());
        self.pastas.iter().find_map(|p| {
            let resto = absoluto.strip_prefix(&p.caminho).ok()?;
            let mut partes: Vec<String> = Vec::new();
            if let Some(x) = &p.prefixo {
                partes.push(x.clone());
            }
            for c in resto.components() {
                partes.push(c.as_os_str().to_str()?.to_string());
            }
            Some(partes.join("/"))
        })
    }

    /// Pasta do projeto que contém o caminho relativo (para detectar o git dela).
    pub fn raiz_de(&self, relativo: &str) -> Option<&Path> {
        let rel = normalizar(relativo)?;
        self.localizar(&rel).map(|(p, _)| p.caminho.as_path())
    }

    /// Separa o caminho relativo do projeto em (pasta, caminho dentro dela).
    fn localizar<'a>(&self, rel: &'a str) -> Option<(&Pasta, &'a str)> {
        let com_prefixo = self.pastas.iter().find_map(|p| {
            let x = p.prefixo.as_deref()?;
            match rel.strip_prefix(x) {
                Some("") => Some((p, "")),
                Some(r) => r.strip_prefix('/').map(|r| (p, r)),
                None => None,
            }
        });
        com_prefixo.or_else(|| {
            self.pastas
                .iter()
                .find(|p| p.prefixo.is_none())
                .map(|p| (p, rel))
        })
    }
}

/// Normaliza um caminho relativo vindo de fora (agente, interface): troca `\` por
/// `/` e recusa absolutos, unidades do Windows e componentes `.`, `..` ou vazios.
fn normalizar(relativo: &str) -> Option<String> {
    let rel = relativo.trim().replace('\\', "/");
    if rel.is_empty() || rel.starts_with('/') || rel.contains(':') {
        return None;
    }
    let rel = rel.trim_end_matches('/');
    rel.split('/')
        .all(|p| !p.is_empty() && p != "." && p != "..")
        .then(|| rel.to_string())
}

fn canonica(p: &Path) -> Result<PathBuf, Erro> {
    match dunce::canonicalize(p) {
        Ok(c) => Ok(c),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Err(Erro::PastaNaoEncontrada(p.to_path_buf()))
        }
        Err(e) => Err(Erro::io(p, e)),
    }
}

fn id_valido(id: &str) -> bool {
    !id.is_empty()
        && !id.starts_with('-')
        && !id.ends_with('-')
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// Identificador legível a partir de um nome: minúsculas sem acento, dígitos e
/// hífens (`"Faturamento Exemplo"` → `faturamento-exemplo`).
pub fn slug(nome: &str) -> String {
    let mut s = String::new();
    for c in nome.chars().flat_map(char::to_lowercase) {
        let c = match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            c => c,
        };
        if c.is_ascii_alphanumeric() {
            s.push(c);
        } else if !s.is_empty() && !s.ends_with('-') {
            s.push('-');
        }
    }
    s.trim_end_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_sem_acentos() {
        assert_eq!(slug("Faturamento Exemplo"), "faturamento-exemplo");
        assert_eq!(slug("  Cobrança / Ação 2 "), "cobranca-acao-2");
        assert_eq!(slug("!!!"), "");
    }

    #[test]
    fn normaliza_caminhos_relativos() {
        assert_eq!(normalizar("db\\01.sql").as_deref(), Some("db/01.sql"));
        assert_eq!(normalizar("db/").as_deref(), Some("db"));
        for ruim in [
            "",
            "/etc/passwd",
            "../x",
            "a/../b",
            "a//b",
            "./a",
            "C:/x",
            "c:x",
        ] {
            assert_eq!(normalizar(ruim), None, "{ruim}");
        }
    }

    #[test]
    fn novo_valida_e_atribui_prefixos() {
        let tmp = tempfile::tempdir().unwrap();
        let a = tmp.path().join("codigo");
        let b = tmp.path().join("Telas Antigas");
        let c = tmp.path().join("outra/Telas Antigas");
        for p in [&a, &b, &c] {
            fs::create_dir_all(p).unwrap();
        }
        let s = Projeto::novo("Faturamento", [&a, &b, &c]).unwrap();
        assert_eq!(s.id, "faturamento");
        assert_eq!(s.pastas[0].prefixo, None);
        assert_eq!(s.pastas[1].prefixo.as_deref(), Some("telas-antigas"));
        assert_eq!(s.pastas[2].prefixo.as_deref(), Some("telas-antigas-2"));

        assert!(matches!(
            Projeto::novo("x", [tmp.path().join("nao-existe")]),
            Err(Erro::PastaNaoEncontrada(_))
        ));
        assert!(matches!(
            Projeto::novo("x", [tmp.path(), a.as_path()]),
            Err(Erro::PastasSobrepostas(..))
        ));
        assert!(matches!(Projeto::novo("  ", [&a]), Err(Erro::NomeVazio)));
        assert!(matches!(
            Projeto::novo("???", [&a]),
            Err(Erro::IdInvalido(_))
        ));
        assert!(matches!(
            Projeto::novo("x", Vec::<PathBuf>::new()),
            Err(Erro::SemPastas)
        ));
    }

    #[test]
    fn prefixo_nao_pode_esconder_item_da_raiz() {
        let tmp = tempfile::tempdir().unwrap();
        let raiz = tmp.path().join("raiz");
        let extra = tmp.path().join("db");
        fs::create_dir_all(raiz.join("db")).unwrap();
        fs::create_dir_all(&extra).unwrap();
        assert!(matches!(
            Projeto::novo("x", [&raiz, &extra]),
            Err(Erro::PrefixoConflitante { .. })
        ));
    }

    #[test]
    fn escopo_resolve_e_recusa() {
        let tmp = tempfile::tempdir().unwrap();
        let raiz = tmp.path().join("raiz");
        let telas = tmp.path().join("telas");
        fs::create_dir_all(raiz.join("db")).unwrap();
        fs::create_dir_all(raiz.join("node_modules/x")).unwrap();
        fs::create_dir_all(&telas).unwrap();
        fs::write(raiz.join("db/01.sql"), "select 1;").unwrap();
        fs::write(raiz.join(".env"), "SENHA=ficticia").unwrap();
        fs::write(raiz.join("node_modules/x/i.js"), "").unwrap();
        fs::write(telas.join("UFrm.pas"), "unit UFrm;").unwrap();
        fs::write(tmp.path().join("fora.txt"), "").unwrap();

        let s = Projeto::novo("Projeto", [&raiz, &telas]).unwrap();
        let e = s.escopo().unwrap();
        let canon = |p: PathBuf| dunce::canonicalize(p).unwrap();

        assert_eq!(
            e.resolver("db/01.sql").unwrap(),
            canon(raiz.join("db/01.sql"))
        );
        assert_eq!(
            e.resolver("db\\01.sql").unwrap(),
            canon(raiz.join("db/01.sql"))
        );
        assert_eq!(
            e.resolver("telas/UFrm.pas").unwrap(),
            canon(telas.join("UFrm.pas"))
        );
        assert!(matches!(
            e.resolver(".env"),
            Err(Recusa::Excluido(Exclusao {
                motivo: Motivo::Segredo,
                ..
            }))
        ));
        assert!(matches!(
            e.resolver("node_modules/x/i.js"),
            Err(Recusa::Excluido(Exclusao {
                motivo: Motivo::Dependencia,
                ..
            }))
        ));
        assert_eq!(
            e.resolver("../fora.txt"),
            Err(Recusa::CaminhoInvalido("../fora.txt".into()))
        );
        assert!(matches!(
            e.resolver("/etc/passwd"),
            Err(Recusa::CaminhoInvalido(_))
        ));
        assert_eq!(e.resolver("db/nao.sql"), Err(Recusa::NaoEncontrado));
        assert_eq!(e.resolver("db"), Err(Recusa::NaoEhArquivo));

        assert_eq!(
            e.relativo(&telas.join("UFrm.pas")).as_deref(),
            Some("telas/UFrm.pas")
        );
        assert_eq!(
            e.relativo(&raiz.join("db/01.sql")).as_deref(),
            Some("db/01.sql")
        );
        assert_eq!(e.relativo(&tmp.path().join("fora.txt")), None);
        assert_eq!(
            e.raiz_de("telas/UFrm.pas"),
            Some(canon(telas.clone()).as_path())
        );
    }

    #[cfg(unix)]
    #[test]
    fn link_simbolico_para_fora_e_recusado() {
        let tmp = tempfile::tempdir().unwrap();
        let raiz = tmp.path().join("raiz");
        let fora = tmp.path().join("fora");
        fs::create_dir_all(&raiz).unwrap();
        fs::create_dir_all(&fora).unwrap();
        fs::write(fora.join("segredo.txt"), "x").unwrap();
        std::os::unix::fs::symlink(&fora, raiz.join("atalho")).unwrap();
        std::os::unix::fs::symlink(fora.join("segredo.txt"), raiz.join("link.txt")).unwrap();
        let e = Projeto::novo("x", [&raiz]).unwrap().escopo().unwrap();
        assert_eq!(e.resolver("atalho/segredo.txt"), Err(Recusa::ForaDoProjeto));
        assert!(matches!(
            e.resolver("link.txt"),
            Err(Recusa::Excluido(Exclusao {
                motivo: Motivo::LinkSimbolico,
                ..
            }))
        ));
    }
}
