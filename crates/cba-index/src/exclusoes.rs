//! Exclusões: o que o inventário e as ferramentas ignoram.
//!
//! Spec, seção 9: dependências de terceiros, binários e arquivos com padrão de
//! segredo ficam fora por padrão; as ferramentas MCP respeitam a lista. O analista
//! ajusta pelo cadastro do projeto: acrescenta exclusões próprias e pode reincluir
//! o que uma regra padrão tirou.
//!
//! ## Padrões
//!
//! Os padrões seguem o estilo do `.gitignore`, aplicados ao caminho relativo à raiz
//! do projeto (com `/`), sem diferenciar maiúsculas de minúsculas:
//!
//! - sem `/` (`node_modules`, `*.pem`): casa com o nome em qualquer profundidade;
//! - com `/` no meio ou no início (`docs/antigos`, `/build`): casa com o caminho a
//!   partir da raiz; `*` não atravessa `/` e `**` atravessa;
//! - terminado em `/` (`target/`): só casa com pastas.
//!
//! Uma pasta excluída leva junto tudo o que está dentro, e o inventário nem entra
//! nela. Para reincluir algo dentro de uma pasta excluída, reinclua a pasta.
//!
//! Binários também são detectados pelo conteúdo no inventário (byte nulo no
//! início do arquivo); essa verificação não tem padrão e não pode ser desfeita,
//! porque as ferramentas só leem texto.

use std::collections::BTreeMap;

use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Serialize};

use crate::Erro;

/// Por que um caminho ficou fora da análise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Motivo {
    /// Arquivo com padrão de segredo: `.env`, chaves, certificados.
    Segredo,
    /// Metadados de controle de versão (`.git`, `.svn`).
    ControleVersao,
    /// Dependências de terceiros (`node_modules`, `vendor`).
    Dependencia,
    /// Saídas de build e cópias de IDE (`target`, `*.class`, `__history`).
    SaidaBuild,
    /// Binário, pela extensão ou pelo conteúdo.
    Binario,
    /// Link simbólico: não é seguido, para não sair das pastas do projeto.
    LinkSimbolico,
    /// Não foi possível ler (permissão, nome não UTF-8).
    Ilegivel,
    /// Exclusão cadastrada pelo analista.
    Usuario,
    /// Pasta da documentação publicada (#274): não é código-fonte. Não pode ser
    /// reincluída.
    Documentacao,
}

impl Motivo {
    /// Descrição curta para a interface.
    pub fn descricao(self) -> &'static str {
        match self {
            Motivo::Segredo => "possível segredo",
            Motivo::ControleVersao => "controle de versão",
            Motivo::Dependencia => "dependência de terceiros",
            Motivo::SaidaBuild => "saída de build",
            Motivo::Binario => "binário",
            Motivo::LinkSimbolico => "link simbólico",
            Motivo::Ilegivel => "ilegível",
            Motivo::Usuario => "excluído pelo analista",
            Motivo::Documentacao => "documentação publicada",
        }
    }
}

/// Resultado da avaliação de um caminho excluído.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Exclusao {
    pub motivo: Motivo,
    /// Padrão que excluiu o caminho; `None` quando a exclusão veio do conteúdo ou
    /// do tipo do arquivo (binário pelo conteúdo, link, erro de leitura).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub regra: Option<String>,
}

/// Regras padrão, na ordem de prioridade (a primeira que casa decide o motivo).
pub const REGRAS_PADRAO: &[(&str, Motivo)] = &[
    // Segredos primeiro: se um caminho casa com mais de uma regra, o motivo mostrado
    // é o mais sensível.
    (".env", Motivo::Segredo),
    (".env.*", Motivo::Segredo),
    ("*.pem", Motivo::Segredo),
    ("*.key", Motivo::Segredo),
    ("*.p12", Motivo::Segredo),
    ("*.pfx", Motivo::Segredo),
    ("*.jks", Motivo::Segredo),
    ("*.keystore", Motivo::Segredo),
    ("*.crt", Motivo::Segredo),
    ("*.cer", Motivo::Segredo),
    ("*.der", Motivo::Segredo),
    ("*.ppk", Motivo::Segredo),
    ("*.kdbx", Motivo::Segredo),
    ("id_rsa*", Motivo::Segredo),
    ("id_dsa*", Motivo::Segredo),
    ("id_ecdsa*", Motivo::Segredo),
    ("id_ed25519*", Motivo::Segredo),
    (".netrc", Motivo::Segredo),
    (".npmrc", Motivo::Segredo),
    (".pypirc", Motivo::Segredo),
    (".pgpass", Motivo::Segredo),
    (".htpasswd", Motivo::Segredo),
    ("credentials.json", Motivo::Segredo),
    // Controle de versão. `.git` sem barra: numa worktree ele é um arquivo.
    (".git", Motivo::ControleVersao),
    (".svn/", Motivo::ControleVersao),
    (".hg/", Motivo::ControleVersao),
    (".bzr/", Motivo::ControleVersao),
    ("CVS/", Motivo::ControleVersao),
    // Dependências de terceiros.
    ("node_modules/", Motivo::Dependencia),
    ("bower_components/", Motivo::Dependencia),
    ("jspm_packages/", Motivo::Dependencia),
    ("vendor/", Motivo::Dependencia),
    (".venv/", Motivo::Dependencia),
    ("venv/", Motivo::Dependencia),
    ("site-packages/", Motivo::Dependencia),
    (".gradle/", Motivo::Dependencia),
    (".m2/", Motivo::Dependencia),
    (".nuget/", Motivo::Dependencia),
    ("Pods/", Motivo::Dependencia),
    // Saídas de build e cópias de IDE. `bin/` fica de fora de propósito: em projetos
    // legados costuma ter scripts; os binários dentro dela caem pela extensão ou pelo
    // conteúdo.
    ("target/", Motivo::SaidaBuild),
    ("build/", Motivo::SaidaBuild),
    ("dist/", Motivo::SaidaBuild),
    ("out/", Motivo::SaidaBuild),
    ("obj/", Motivo::SaidaBuild),
    (".next/", Motivo::SaidaBuild),
    (".nuxt/", Motivo::SaidaBuild),
    ("coverage/", Motivo::SaidaBuild),
    ("__pycache__/", Motivo::SaidaBuild),
    (".pytest_cache/", Motivo::SaidaBuild),
    ("__history/", Motivo::SaidaBuild),
    ("__recovery/", Motivo::SaidaBuild),
    ("*.class", Motivo::SaidaBuild),
    ("*.o", Motivo::SaidaBuild),
    ("*.obj", Motivo::SaidaBuild),
    ("*.pyc", Motivo::SaidaBuild),
    ("*.pyo", Motivo::SaidaBuild),
    ("*.dcu", Motivo::SaidaBuild),
    ("*.~*", Motivo::SaidaBuild),
    ("*.min.js", Motivo::SaidaBuild),
    ("*.min.css", Motivo::SaidaBuild),
    ("*.map", Motivo::SaidaBuild),
    // Binários pela extensão (executáveis, pacotes, mídia e documentos binários).
    ("*.exe", Motivo::Binario),
    ("*.dll", Motivo::Binario),
    ("*.so", Motivo::Binario),
    ("*.dylib", Motivo::Binario),
    ("*.lib", Motivo::Binario),
    ("*.a", Motivo::Binario),
    ("*.bpl", Motivo::Binario),
    ("*.res", Motivo::Binario),
    ("*.bin", Motivo::Binario),
    ("*.jar", Motivo::Binario),
    ("*.war", Motivo::Binario),
    ("*.ear", Motivo::Binario),
    ("*.zip", Motivo::Binario),
    ("*.gz", Motivo::Binario),
    ("*.tgz", Motivo::Binario),
    ("*.bz2", Motivo::Binario),
    ("*.xz", Motivo::Binario),
    ("*.7z", Motivo::Binario),
    ("*.rar", Motivo::Binario),
    ("*.tar", Motivo::Binario),
    ("*.png", Motivo::Binario),
    ("*.jpg", Motivo::Binario),
    ("*.jpeg", Motivo::Binario),
    ("*.gif", Motivo::Binario),
    ("*.bmp", Motivo::Binario),
    ("*.ico", Motivo::Binario),
    ("*.tif", Motivo::Binario),
    ("*.tiff", Motivo::Binario),
    ("*.webp", Motivo::Binario),
    ("*.psd", Motivo::Binario),
    ("*.mp3", Motivo::Binario),
    ("*.mp4", Motivo::Binario),
    ("*.avi", Motivo::Binario),
    ("*.mov", Motivo::Binario),
    ("*.wav", Motivo::Binario),
    ("*.ogg", Motivo::Binario),
    ("*.woff", Motivo::Binario),
    ("*.woff2", Motivo::Binario),
    ("*.ttf", Motivo::Binario),
    ("*.otf", Motivo::Binario),
    ("*.eot", Motivo::Binario),
    ("*.pdf", Motivo::Binario),
    ("*.doc", Motivo::Binario),
    ("*.docx", Motivo::Binario),
    ("*.xls", Motivo::Binario),
    ("*.xlsx", Motivo::Binario),
    ("*.ppt", Motivo::Binario),
    ("*.pptx", Motivo::Binario),
    ("*.odt", Motivo::Binario),
    ("*.ods", Motivo::Binario),
    ("*.odp", Motivo::Binario),
    ("*.mdb", Motivo::Binario),
    ("*.accdb", Motivo::Binario),
    ("*.sqlite", Motivo::Binario),
    ("*.db", Motivo::Binario),
];

#[derive(Debug, Clone)]
struct Regra {
    padrao: String,
    motivo: Motivo,
    so_pasta: bool,
}

/// Conjunto compilado de exclusões de um projeto: regras padrão, exclusões do
/// analista e reinclusões.
#[derive(Debug, Clone)]
pub struct Exclusoes {
    regras: Vec<Regra>,
    conjunto: GlobSet,
    inclusoes: Vec<Regra>,
    conjunto_inclusoes: GlobSet,
    /// Pastas da documentação publicada, relativas à raiz do projeto (#274).
    documentacao: Vec<String>,
}

impl Exclusoes {
    /// Compila as regras padrão mais as exclusões e reinclusões do analista.
    ///
    /// As reinclusões têm precedência sobre qualquer padrão (padrão ou do
    /// analista), mas não sobre a detecção de binário pelo conteúdo.
    pub fn novas(exclusoes: &[String], inclusoes: &[String]) -> Result<Self, Erro> {
        let regras: Vec<Regra> = REGRAS_PADRAO
            .iter()
            .map(|(p, m)| regra(p, *m))
            .chain(exclusoes.iter().map(|p| regra(p, Motivo::Usuario)))
            .collect();
        let inclusoes: Vec<Regra> = inclusoes
            .iter()
            .map(|p| regra(p, Motivo::Usuario))
            .collect();
        Ok(Self {
            conjunto: compilar(&regras)?,
            conjunto_inclusoes: compilar(&inclusoes)?,
            regras,
            inclusoes,
            documentacao: Vec::new(),
        })
    }

    /// Acrescenta as pastas da documentação publicada (#274), relativas à raiz do
    /// projeto e com `/`: elas e tudo dentro delas ficam fora, com o motivo
    /// [`Motivo::Documentacao`], mesmo que uma reinclusão as cubra.
    pub fn com_documentacao(mut self, pastas: impl IntoIterator<Item = String>) -> Self {
        for p in pastas {
            let p = p.replace('\\', "/").trim_matches('/').to_string();
            if !p.is_empty() && !self.documentacao.contains(&p) {
                self.documentacao.push(p);
            }
        }
        self
    }

    /// A pasta da documentação que contém `relativo` (ou é ele).
    fn documentacao_de(&self, relativo: &str) -> Option<&String> {
        self.documentacao.iter().find(|d| {
            relativo.len() >= d.len()
                && relativo.is_char_boundary(d.len())
                && relativo[..d.len()].eq_ignore_ascii_case(d)
                && (relativo.len() == d.len() || relativo.as_bytes()[d.len()] == b'/')
        })
    }

    /// Só as regras padrão.
    pub fn padrao() -> Self {
        Self::novas(&[], &[]).expect("as regras padrão compilam")
    }

    /// Avalia um único item (arquivo ou pasta), sem olhar as pastas acima dele.
    ///
    /// `relativo` é o caminho relativo à raiz do projeto, com `/`.
    pub fn avaliar(&self, relativo: &str, pasta: bool) -> Option<Exclusao> {
        let relativo = relativo.trim_matches('/');
        if relativo.is_empty() {
            return None;
        }
        if let Some(d) = self.documentacao_de(relativo) {
            return Some(Exclusao {
                motivo: Motivo::Documentacao,
                regra: Some(format!("/{d}/")),
            });
        }
        let casa = |regras: &[Regra], indices: Vec<usize>| {
            indices
                .into_iter()
                .filter(|&i| pasta || !regras[i].so_pasta)
                .min()
        };
        if casa(&self.inclusoes, self.conjunto_inclusoes.matches(relativo)).is_some() {
            return None;
        }
        let i = casa(&self.regras, self.conjunto.matches(relativo))?;
        Some(Exclusao {
            motivo: self.regras[i].motivo,
            regra: Some(self.regras[i].padrao.clone()),
        })
    }

    /// Avalia o caminho de um arquivo, considerando também as pastas acima dele: um
    /// arquivo dentro de `node_modules/` está excluído mesmo que o nome dele não
    /// case com nada.
    pub fn avaliar_caminho(&self, relativo: &str) -> Option<Exclusao> {
        let relativo = relativo.trim_matches('/');
        let mut fim = 0;
        while let Some(pos) = relativo[fim..].find('/') {
            fim += pos;
            if let Some(e) = self.avaliar(&relativo[..fim], true) {
                return Some(e);
            }
            fim += 1;
        }
        self.avaliar(relativo, false)
    }

    /// Alguma reinclusão cobre o caminho de um arquivo (ele mesmo ou uma pasta
    /// acima)? Serve para contar o que uma reinclusão faz entrar.
    pub fn reincluido(&self, relativo: &str) -> bool {
        let relativo = relativo.trim_matches('/');
        let casa = |rel: &str, pasta: bool| {
            self.conjunto_inclusoes
                .matches(rel)
                .into_iter()
                .any(|i| pasta || !self.inclusoes[i].so_pasta)
        };
        let mut fim = 0;
        while let Some(pos) = relativo[fim..].find('/') {
            fim += pos;
            if casa(&relativo[..fim], true) {
                return true;
            }
            fim += 1;
        }
        casa(relativo, false)
    }

    /// Padrões do analista e regras padrão, para mostrar na interface.
    pub fn regras(&self) -> BTreeMap<Motivo, Vec<&str>> {
        let mut mapa: BTreeMap<Motivo, Vec<&str>> = BTreeMap::new();
        for r in &self.regras {
            mapa.entry(r.motivo).or_default().push(&r.padrao);
        }
        for d in &self.documentacao {
            mapa.entry(Motivo::Documentacao).or_default().push(d);
        }
        mapa
    }
}

fn regra(padrao: &str, motivo: Motivo) -> Regra {
    Regra {
        padrao: padrao.to_string(),
        motivo,
        so_pasta: padrao.trim().ends_with('/'),
    }
}

/// Traduz o padrão no estilo `.gitignore` para um glob sobre o caminho relativo.
fn glob(padrao: &str) -> Result<String, Erro> {
    let p = padrao.trim().replace('\\', "/");
    let p = p.trim_end_matches('/');
    let ancorado = p.starts_with('/') || p.contains('/');
    let p = p.trim_start_matches('/');
    if p.is_empty() {
        return Err(Erro::PadraoInvalido {
            padrao: padrao.to_string(),
            detalhe: "padrão vazio".into(),
        });
    }
    Ok(if ancorado {
        p.to_string()
    } else {
        format!("**/{p}")
    })
}

fn compilar(regras: &[Regra]) -> Result<GlobSet, Erro> {
    let mut b = GlobSetBuilder::new();
    for r in regras {
        let g = GlobBuilder::new(&glob(&r.padrao)?)
            .literal_separator(true)
            .case_insensitive(true)
            .backslash_escape(false)
            .build()
            .map_err(|e| Erro::PadraoInvalido {
                padrao: r.padrao.clone(),
                detalhe: e.kind().to_string(),
            })?;
        b.add(g);
    }
    b.build().map_err(|e| Erro::PadraoInvalido {
        padrao: String::new(),
        detalhe: e.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn motivo(e: &Exclusoes, caminho: &str) -> Option<Motivo> {
        e.avaliar_caminho(caminho).map(|x| x.motivo)
    }

    #[test]
    fn pasta_da_documentacao_fica_fora_mesmo_reincluida() {
        let e = Exclusoes::novas(&[], &["docs/".into(), "*.md".into()])
            .unwrap()
            .com_documentacao(["/docs/negocio/".to_string(), String::new()]);
        assert_eq!(
            motivo(&e, "docs/negocio/regras.md"),
            Some(Motivo::Documentacao)
        );
        assert_eq!(
            motivo(&e, "Docs/Negocio/a/b.md"),
            Some(Motivo::Documentacao)
        );
        assert_eq!(
            e.avaliar("docs/negocio", true).unwrap().regra.as_deref(),
            Some("/docs/negocio/")
        );
        // Só a pasta, não o que começa com o mesmo nome.
        assert_eq!(motivo(&e, "docs/negocios/a.md"), None);
        assert_eq!(motivo(&e, "docs/a.md"), None);
        assert_eq!(e.regras()[&Motivo::Documentacao], vec!["docs/negocio"]);
    }

    #[test]
    fn reinclusao_cobre_o_arquivo_ou_a_pasta_acima() {
        let e = Exclusoes::novas(&[], &["/docs/*.png".into(), "/vendor".into()]).unwrap();
        assert!(e.reincluido("docs/logo.png"));
        assert!(!e.reincluido("docs/sub/logo.png"));
        assert!(!e.reincluido("docs/logo.jpg"));
        assert!(e.reincluido("vendor/lib/a.php"));
        assert!(!e.reincluido("src/a.php"));
    }

    #[test]
    fn segredos_em_qualquer_profundidade() {
        let e = Exclusoes::padrao();
        assert_eq!(motivo(&e, ".env"), Some(Motivo::Segredo));
        assert_eq!(motivo(&e, "config/.env.producao"), Some(Motivo::Segredo));
        assert_eq!(motivo(&e, "certs/servidor.PEM"), Some(Motivo::Segredo));
        assert_eq!(motivo(&e, "home/id_rsa"), Some(Motivo::Segredo));
        assert_eq!(motivo(&e, "src/environment.ts"), None);
    }

    #[test]
    fn pastas_de_dependencia_excluem_o_conteudo() {
        let e = Exclusoes::padrao();
        assert_eq!(
            motivo(&e, "frontend/node_modules/lib/index.js"),
            Some(Motivo::Dependencia)
        );
        assert_eq!(motivo(&e, "vendor/x.php"), Some(Motivo::Dependencia));
        // Um arquivo chamado `vendor` não é a pasta de dependências.
        assert_eq!(motivo(&e, "docs/vendor"), None);
        assert_eq!(
            motivo(&e, "backend/target/classes/A.class"),
            Some(Motivo::SaidaBuild)
        );
        assert_eq!(motivo(&e, ".git/HEAD"), Some(Motivo::ControleVersao));
        assert_eq!(motivo(&e, "bin/rodar.sh"), None);
    }

    #[test]
    fn binarios_pela_extensao() {
        let e = Exclusoes::padrao();
        assert_eq!(motivo(&e, "lib/driver.DLL"), Some(Motivo::Binario));
        assert_eq!(motivo(&e, "UFrm.dcu"), Some(Motivo::SaidaBuild));
        assert_eq!(motivo(&e, "UFrm.~pas"), Some(Motivo::SaidaBuild));
        assert_eq!(motivo(&e, "UFrm.pas"), None);
    }

    #[test]
    fn exclusoes_e_reinclusoes_do_analista() {
        let e = Exclusoes::novas(
            &["docs/antigos/".into(), "*.bak".into()],
            &["vendor".into(), "/build".into()],
        )
        .unwrap();
        assert_eq!(motivo(&e, "docs/antigos/a.txt"), Some(Motivo::Usuario));
        assert_eq!(motivo(&e, "x/docs/antigos/a.txt"), None);
        assert_eq!(motivo(&e, "db/copia.BAK"), Some(Motivo::Usuario));
        assert_eq!(motivo(&e, "vendor/lib/a.php"), None);
        assert_eq!(motivo(&e, "build/script.sh"), None);
        assert_eq!(motivo(&e, "app/build/x.js"), Some(Motivo::SaidaBuild));
        let x = e.avaliar_caminho("db/copia.bak").unwrap();
        assert_eq!(x.regra.as_deref(), Some("*.bak"));
    }

    #[test]
    fn padrao_invalido_e_recusado() {
        let r = Exclusoes::novas(&["a/[".into()], &[]);
        assert!(matches!(r, Err(Erro::PadraoInvalido { .. })));
        let r = Exclusoes::novas(&["/".into()], &[]);
        assert!(matches!(r, Err(Erro::PadraoInvalido { .. })));
    }
}
