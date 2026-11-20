//! Cadastro de um projeto e o escopo de caminhos que as ferramentas podem ler.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use cba_repos::Repositorios;
use serde::{Deserialize, Serialize};

use crate::Erro;
use crate::ambiente_teste::AmbienteTeste;
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
    /// Onde ficam o código (forja) e as issues (rastreador). Opcional: sem eles o
    /// projeto é só um conjunto de pastas locais. Guarda o comando de credencial,
    /// nunca o token.
    #[serde(default, skip_serializing_if = "Repositorios::vazio")]
    pub repositorios: Repositorios,
    /// Ambiente de teste da aplicação (URL, usuários e perfis), para simular o
    /// uso. Opcional. Nunca guarda a senha, que fica no chaveiro do sistema.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ambiente_teste: Option<AmbienteTeste>,
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
            let prefixo = (i > 0).then(|| prefixo_novo(&caminho, &mut usados));
            lista.push(Pasta { caminho, prefixo });
        }
        let projeto = Self {
            id: slug(nome),
            nome: nome.to_string(),
            pastas: lista,
            exclusoes: Vec::new(),
            inclusoes: Vec::new(),
            repositorios: Repositorios::default(),
            ambiente_teste: None,
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
        self.repositorios.validar().map_err(Erro::Repositorios)?;
        // A documentação publicada não é código-fonte: nenhuma pasta do projeto fica
        // dentro dela (#274).
        for d in self.pastas_de_documentacao() {
            if let Some(p) = self.pastas.iter().find(|p| p.caminho.starts_with(&d)) {
                return Err(Erro::Repositorios(format!(
                    "a pasta {} do projeto fica dentro da pasta da documentação ({}); \
                     a documentação publicada não é código-fonte",
                    p.caminho.display(),
                    d.display()
                )));
            }
        }
        if let Some(a) = &self.ambiente_teste {
            a.validar().map_err(Erro::AmbienteTeste)?;
        }
        Ok(())
    }

    /// O projeto com outra lista de pastas (#192): adiciona e remove pastas sem
    /// descadastrar.
    ///
    /// As pastas que continuam guardam o prefixo (os caminhos das evidências não
    /// mudam); as novas entram sempre com prefixo, derivado do nome da pasta e sem
    /// repetir um prefixo em uso. Cada caminho novo precisa ser absoluto e existir; a
    /// lista vazia é recusada ([`Erro::UltimaPasta`]), e o resultado passa pela
    /// [`Projeto::validar`] (sem sobreposição, sem esconder item da raiz).
    pub fn com_pastas<P: AsRef<Path>>(
        &self,
        caminhos: impl IntoIterator<Item = P>,
    ) -> Result<Self, Erro> {
        let mut pedidas: Vec<PathBuf> = Vec::new();
        for p in caminhos {
            let p = p.as_ref();
            if !p.is_absolute() {
                return Err(Erro::CaminhoRelativo(p.to_path_buf()));
            }
            // Uma pasta cadastrada que sumiu do disco ainda pode ser mantida (e a
            // validação aponta que ela não existe) ou removida pelo caminho gravado.
            let caminho = match self.pastas.iter().find(|x| x.caminho == p) {
                Some(x) => x.caminho.clone(),
                None => canonica(p)?,
            };
            if !pedidas.contains(&caminho) {
                pedidas.push(caminho);
            }
        }
        if pedidas.is_empty() {
            return Err(Erro::UltimaPasta);
        }
        let mut pastas: Vec<Pasta> = self
            .pastas
            .iter()
            .filter(|x| pedidas.contains(&x.caminho))
            .cloned()
            .collect();
        let mut usados: HashSet<String> = self
            .pastas
            .iter()
            .filter_map(|x| x.prefixo.clone())
            .collect();
        for caminho in pedidas {
            if pastas.iter().any(|x| x.caminho == caminho) {
                continue;
            }
            let prefixo = Some(prefixo_novo(&caminho, &mut usados));
            pastas.push(Pasta { caminho, prefixo });
        }
        let projeto = Self {
            pastas,
            ..self.clone()
        };
        projeto.validar()?;
        Ok(projeto)
    }

    /// Exclusões compiladas do projeto: padrão, ajustes do analista e a pasta da
    /// documentação publicada, quando ela fica dentro das pastas do projeto (#274).
    pub fn regras_de_exclusao(&self) -> Result<Exclusoes, Erro> {
        Ok(
            Exclusoes::novas(&self.exclusoes, &self.inclusoes)?.com_documentacao(
                self.pastas_de_documentacao()
                    .iter()
                    .filter_map(|d| relativo_as_pastas(&self.pastas, d)),
            ),
        )
    }

    /// Pastas locais da documentação publicada (#274), canônicas no que já existe.
    ///
    /// No repositório próprio, o clone mais a subpasta. Numa subpasta do código, a
    /// pasta do repositório de código mais a subpasta; sem pasta no repositório
    /// (cadastro antigo, que vale para o projeto todo), a raiz do git da pasta raiz
    /// do projeto. Vazio sem destino ou sem clone local.
    pub fn pastas_de_documentacao(&self) -> Vec<PathBuf> {
        let Some(d) = self.repositorios.destino_docs() else {
            return Vec::new();
        };
        let base = match d.pasta_local() {
            Some(p) => p.to_path_buf(),
            None if !d.independente => {
                let Some(raiz) = self.pastas.iter().find(|p| p.prefixo.is_none()) else {
                    return Vec::new();
                };
                crate::detectar_git(&raiz.caminho).map_or_else(|| raiz.caminho.clone(), |g| g.raiz)
            }
            None => return Vec::new(),
        };
        vec![canonica_parcial(&cba_repos::docs::juntar(
            &base, d.subpasta,
        ))]
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
        relativo_as_pastas(&self.pastas, &absoluto)
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

/// Caminho relativo à raiz do projeto (com o prefixo da pasta) de um caminho
/// absoluto já canônico, se ele estiver dentro de alguma das `pastas`.
fn relativo_as_pastas(pastas: &[Pasta], absoluto: &Path) -> Option<String> {
    pastas.iter().find_map(|p| {
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

/// O caminho com a parte que já existe canônica e o resto como veio (a pasta da
/// documentação pode ainda não existir).
fn canonica_parcial(p: &Path) -> PathBuf {
    let mut resto = Vec::new();
    let mut atual = p;
    loop {
        if let Ok(c) = dunce::canonicalize(atual) {
            return resto.iter().rev().fold(c, |c, parte| c.join(parte));
        }
        match (atual.parent(), atual.file_name()) {
            (Some(pai), Some(nome)) => {
                resto.push(nome.to_os_string());
                atual = pai;
            }
            _ => return p.to_path_buf(),
        }
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

/// Prefixo de uma pasta nova: o nome dela em slug (ou `pasta`), com sufixo numérico
/// se já estiver em `usados`, onde ele é acrescentado.
fn prefixo_novo(caminho: &Path, usados: &mut HashSet<String>) -> String {
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
    candidato
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
    fn com_pastas_adiciona_e_remove_mantendo_os_prefixos() {
        let tmp = tempfile::tempdir().unwrap();
        let raiz = tmp.path().join("raiz");
        let telas = tmp.path().join("telas");
        let outra = tmp.path().join("outra/telas");
        let web = tmp.path().join("Web App");
        for p in [&raiz, &telas, &outra, &web] {
            fs::create_dir_all(p).unwrap();
        }
        let p = Projeto::novo("Faturamento", [&raiz, &telas]).unwrap();
        let canon = |x: &Path| dunce::canonicalize(x).unwrap();

        // Acrescentar: as que ficam guardam o prefixo; a nova não repete um em uso.
        let mais = p.com_pastas([&raiz, &telas, &outra, &web]).unwrap();
        assert_eq!(mais.pastas[0], p.pastas[0]);
        assert_eq!(mais.pastas[1], p.pastas[1]);
        assert_eq!(mais.pastas[2].caminho, canon(&outra));
        assert_eq!(mais.pastas[2].prefixo.as_deref(), Some("telas-2"));
        assert_eq!(mais.pastas[3].prefixo.as_deref(), Some("web-app"));
        assert_eq!(
            (mais.id.as_str(), mais.nome.as_str()),
            ("faturamento", "Faturamento")
        );

        // Remover a raiz: a outra continua com o prefixo dela.
        let menos = mais.com_pastas([&telas]).unwrap();
        assert_eq!(menos.pastas, vec![p.pastas[1].clone()]);

        // Repetida conta uma vez só.
        assert_eq!(p.com_pastas([&raiz, &raiz]).unwrap().pastas.len(), 1);
    }

    #[test]
    fn com_pastas_recusa_o_que_perderia_dados() {
        let tmp = tempfile::tempdir().unwrap();
        let raiz = tmp.path().join("raiz");
        fs::create_dir_all(raiz.join("db")).unwrap();
        let db = tmp.path().join("db");
        fs::create_dir_all(&db).unwrap();
        fs::write(tmp.path().join("arquivo.txt"), "").unwrap();
        let p = Projeto::novo("x", [&raiz]).unwrap();

        assert!(matches!(
            p.com_pastas(Vec::<PathBuf>::new()),
            Err(Erro::UltimaPasta)
        ));
        assert!(matches!(
            p.com_pastas([Path::new("relativa")]),
            Err(Erro::CaminhoRelativo(_))
        ));
        assert!(matches!(
            p.com_pastas([raiz.clone(), tmp.path().join("nao-existe")]),
            Err(Erro::PastaNaoEncontrada(_))
        ));
        assert!(matches!(
            p.com_pastas([raiz.clone(), tmp.path().join("arquivo.txt")]),
            Err(Erro::NaoEhPasta(_))
        ));
        assert!(matches!(
            p.com_pastas([raiz.clone(), raiz.join("db")]),
            Err(Erro::PastasSobrepostas(..))
        ));
        assert!(matches!(
            p.com_pastas([&raiz, &db]),
            Err(Erro::PrefixoConflitante { .. })
        ));
    }

    #[test]
    fn com_pastas_remove_a_que_sumiu_do_disco() {
        let tmp = tempfile::tempdir().unwrap();
        let a = tmp.path().join("a");
        let b = tmp.path().join("b");
        fs::create_dir_all(&a).unwrap();
        fs::create_dir_all(&b).unwrap();
        let p = Projeto::novo("x", [&a, &b]).unwrap();
        let gravada_b = p.pastas[1].caminho.clone();
        fs::remove_dir_all(&b).unwrap();
        // Manter a que sumiu é recusado; tirá-la (pelo caminho gravado) funciona.
        assert!(matches!(
            p.com_pastas([&p.pastas[0].caminho, &gravada_b]),
            Err(Erro::PastaNaoEncontrada(_))
        ));
        let so_a = p.com_pastas([&p.pastas[0].caminho]).unwrap();
        assert_eq!(so_a.pastas, vec![p.pastas[0].clone()]);
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

    fn codigo_em(pasta: &Path) -> cba_repos::RepositorioCodigo {
        cba_repos::RepositorioCodigo {
            caminho: "exemplo/faturamento".into(),
            pasta: Some(pasta.to_path_buf()),
            ..Default::default()
        }
    }

    #[test]
    fn projeto_antigo_abre_sem_destino_da_documentacao() {
        let antigo = r#"{
            "id": "faturamento-exemplo",
            "nome": "Faturamento Exemplo",
            "pastas": [{ "caminho": "/repos/faturamento" }],
            "repositorios": {
                "codigo": [{ "forja": "github", "host": "github.com", "caminho": "exemplo/faturamento" }],
                "tracker": null
            }
        }"#;
        let p: Projeto = serde_json::from_str(antigo).unwrap();
        assert_eq!(p.repositorios.docs, None);
        assert!(p.pastas_de_documentacao().is_empty());
        let v = serde_json::to_value(&p).unwrap();
        assert!(v["repositorios"].get("docs").is_none());
        assert_eq!(serde_json::from_value::<Projeto>(v).unwrap(), p);
    }

    #[test]
    fn subpasta_da_documentacao_fica_fora_do_escopo() {
        let tmp = tempfile::tempdir().unwrap();
        let raiz = tmp.path().join("faturamento");
        let telas = tmp.path().join("telas");
        fs::create_dir_all(raiz.join("docs/negocio")).unwrap();
        fs::create_dir_all(raiz.join("db")).unwrap();
        fs::create_dir_all(&telas).unwrap();
        fs::write(raiz.join("docs/negocio/regras.md"), "# Regras").unwrap();
        fs::write(raiz.join("docs/leiame.md"), "# Leia-me").unwrap();
        fs::write(raiz.join("db/01.sql"), "create table nota ();").unwrap();
        let mut p = Projeto::novo("Faturamento Exemplo", [&raiz, &telas]).unwrap();
        p.inclusoes = vec!["docs/".into()];
        let codigo = codigo_em(&dunce::canonicalize(&raiz).unwrap());
        p.repositorios.codigo = vec![codigo.clone()];
        p.repositorios.docs = Some(cba_repos::DestinoDocs::Subpasta {
            codigo: codigo.chave(),
            subpasta: "docs/negocio".into(),
        });
        p.validar().unwrap();
        assert_eq!(
            p.pastas_de_documentacao(),
            vec![dunce::canonicalize(raiz.join("docs/negocio")).unwrap()]
        );
        let e = p.escopo().unwrap();
        assert!(matches!(
            e.resolver("docs/negocio/regras.md"),
            Err(Recusa::Excluido(Exclusao {
                motivo: Motivo::Documentacao,
                ..
            }))
        ));
        assert!(e.resolver("docs/leiame.md").is_ok());
        assert!(e.resolver("db/01.sql").is_ok());
    }

    #[test]
    fn repositorio_proprio_da_documentacao() {
        let tmp = tempfile::tempdir().unwrap();
        let raiz = tmp.path().join("faturamento");
        let docs = raiz.join("faturamento-docs");
        fs::create_dir_all(&docs).unwrap();
        fs::write(docs.join("a.md"), "x").unwrap();
        let mut p = Projeto::novo("Faturamento Exemplo", [&raiz]).unwrap();
        let destino = |pasta: &Path| cba_repos::DestinoDocs::Repositorio {
            repositorio: cba_repos::RepositorioCodigo {
                caminho: "exemplo/faturamento-docs".into(),
                ..codigo_em(pasta)
            },
            subpasta: String::new(),
        };
        // O clone dentro de uma pasta do projeto: fica fora do escopo.
        p.repositorios.docs = Some(destino(&docs));
        let e = p.escopo().unwrap();
        assert!(matches!(
            e.resolver("faturamento-docs/a.md"),
            Err(Recusa::Excluido(_))
        ));
        // Uma pasta do projeto dentro do clone da documentação: recusa.
        p.repositorios.docs = Some(destino(&raiz));
        assert!(matches!(p.validar(), Err(Erro::Repositorios(m)) if m.contains("documentação")));
        // Clone fora do projeto, ainda sem pasta criada: nada muda no escopo.
        p.repositorios.docs = Some(destino(&tmp.path().join("ainda-nao")));
        p.validar().unwrap();
        assert!(
            p.escopo()
                .unwrap()
                .resolver("faturamento-docs/a.md")
                .is_ok()
        );
    }
}
