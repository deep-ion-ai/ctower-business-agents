//! Cópias customizadas dos workflows e a versão efetiva de cada tipo (spec
//! `docs/specs/2026-11-25-workflows-nas-analises.md`, seção 3.5; #372).
//!
//! | Origem | Arquivo |
//! |---|---|
//! | [`OrigemWorkflow::Projeto`] | `<base>/_templates/workflows/<tipo>.js` (versionada com a base) |
//! | [`OrigemWorkflow::Global`] | `<dados do app>/workflows/<tipo>.js` (todos os projetos) |
//! | [`OrigemWorkflow::Padrao`] | embutido no app |
//!
//! A precedência é projeto > global > padrão, no molde do `editavel.rs`, com uma
//! camada a mais. Uma cópia ilegível ou com erro de validação ([`super::validar`])
//! não é usada: vale a camada de baixo, e a tela de Configurações mostra o motivo.
//!
//! A cópia começa por duas linhas de comentário do app com o SHA-256 do padrão de
//! que ela partiu ([`montar`]). Atualizar o app nunca toca a cópia; quando o padrão
//! embutido muda, o hash não confere e a tela avisa, sem mesclar. O editor mostra o
//! script sem essas linhas ([`desmontar`]) e o app as repõe ao salvar; a versão
//! efetiva também vai sem elas.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;
use sha2::{Digest, Sha256};

use super::{OrigemWorkflow, TIPOS, WorkflowEfetivo, padrao, validar};

/// Pasta das cópias, dentro de `<base>/_templates/` e de `<dados do app>/`.
pub const PASTA: &str = "workflows";

const LINHA_DO_APP: &str = "// Cópia customizada de um workflow do CBA (Configurações › Workflows). \
A linha abaixo guarda o padrão de que ela partiu; não a apague.";
const MARCA_PADRAO: &str = "// cba-padrao-sha256: ";

/// SHA-256 em hexadecimal.
pub fn sha256(texto: &str) -> String {
    Sha256::digest(texto.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// SHA-256 do padrão embutido de `tipo`.
pub fn hash_do_padrao(tipo: &str) -> Option<String> {
    padrao(tipo).map(sha256)
}

/// `<base>/_templates/workflows/<tipo>.js`.
pub fn arquivo_do_projeto(base: &Path, tipo: &str) -> PathBuf {
    base.join(cba_kb::layout::TEMPLATES)
        .join(PASTA)
        .join(format!("{tipo}.js"))
}

/// `<pasta global>/<tipo>.js`; a pasta global é `<dados do app>/workflows`
/// (`DiretorioDados::workflows`).
pub fn arquivo_global(pasta_global: &Path, tipo: &str) -> PathBuf {
    pasta_global.join(format!("{tipo}.js"))
}

/// O arquivo da cópia: as linhas do app, com o hash do padrão de origem, e o script.
pub fn montar(script: &str, hash_padrao: &str) -> String {
    format!("{LINHA_DO_APP}\n{MARCA_PADRAO}{hash_padrao}\n{script}")
}

/// O script e o hash do padrão de origem de um arquivo de cópia. Sem as linhas do
/// app (apagadas ou editadas fora), o texto inteiro é o script e o hash é `None`.
pub fn desmontar(texto: &str) -> (String, Option<String>) {
    let texto = texto.replace("\r\n", "\n");
    let mut resto = texto.as_str();
    if let Some(r) = resto
        .strip_prefix(LINHA_DO_APP)
        .and_then(|r| r.strip_prefix('\n'))
    {
        resto = r;
    }
    if let Some(r) = resto.strip_prefix(MARCA_PADRAO) {
        let (hash, depois) = r.split_once('\n').unwrap_or((r, ""));
        let hash = hash.trim();
        if hash.len() == 64 && hash.chars().all(|c| c.is_ascii_hexdigit()) {
            return (depois.to_string(), Some(hash.to_string()));
        }
    }
    (resto.to_string(), None)
}

/// Situação de uma cópia.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "situacao")]
pub enum SituacaoCopia {
    /// Não há cópia.
    Ausente,
    /// Cópia válida, usada se nenhuma camada acima a substitui.
    Valida,
    /// Cópia com erro de validação: não é usada.
    Invalida { erros: String },
    /// Arquivo que não pôde ser lido (ou não é UTF-8): não é usado.
    Ilegivel { erro: String },
}

/// Uma cópia de um workflow (do projeto ou global).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Copia {
    pub origem: OrigemWorkflow,
    pub arquivo: PathBuf,
    #[serde(flatten)]
    pub situacao: SituacaoCopia,
    /// O script, sem as linhas do app (vazio se ausente ou ilegível).
    #[serde(skip)]
    pub script: String,
    /// Hash do padrão de que a cópia partiu, se as linhas do app estão lá.
    pub hash_padrao: Option<String>,
    /// O padrão embutido mudou desde a cópia (ou não se sabe de que padrão ela
    /// partiu): a tela avisa e mostra a diferença.
    pub padrao_mudou: bool,
}

impl Copia {
    /// Lê a cópia de `tipo` em `arquivo`.
    pub fn ler(tipo: &str, origem: OrigemWorkflow, arquivo: PathBuf) -> Self {
        let mut copia = Self {
            origem,
            arquivo,
            situacao: SituacaoCopia::Ausente,
            script: String::new(),
            hash_padrao: None,
            padrao_mudou: false,
        };
        let bytes = match fs::read(&copia.arquivo) {
            Ok(b) => b,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return copia,
            Err(e) => {
                copia.situacao = SituacaoCopia::Ilegivel {
                    erro: e.to_string(),
                };
                return copia;
            }
        };
        let Ok(texto) = String::from_utf8(bytes) else {
            copia.situacao = SituacaoCopia::Ilegivel {
                erro: "o arquivo não está em UTF-8".into(),
            };
            return copia;
        };
        let (script, hash) = desmontar(&texto);
        let validacao = validar(tipo, &script);
        copia.situacao = if validacao.tem_erro() {
            SituacaoCopia::Invalida {
                erros: validacao.resumo_dos_erros(),
            }
        } else {
            SituacaoCopia::Valida
        };
        copia.padrao_mudou = hash != hash_do_padrao(tipo);
        copia.hash_padrao = hash;
        copia.script = script;
        copia
    }

    /// Existe um arquivo (válido ou não).
    pub fn existe(&self) -> bool {
        self.situacao != SituacaoCopia::Ausente
    }

    /// A cópia pode ser a versão efetiva.
    pub fn usavel(&self) -> bool {
        self.situacao == SituacaoCopia::Valida
    }
}

/// As camadas de um tipo e a versão efetiva que resulta delas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolucao {
    pub tipo: String,
    /// Sem base configurada, não há cópia do projeto.
    pub projeto: Option<Copia>,
    pub global: Copia,
    pub efetivo: WorkflowEfetivo,
}

/// Resolve o workflow de `tipo`: projeto (`<base>/_templates/workflows`) > global
/// (`pasta_global`) > padrão. `None` se o tipo não tem padrão.
pub fn resolver(tipo: &str, base: Option<&Path>, pasta_global: &Path) -> Option<Resolucao> {
    let padrao = WorkflowEfetivo::padrao(tipo)?;
    let projeto =
        base.map(|b| Copia::ler(tipo, OrigemWorkflow::Projeto, arquivo_do_projeto(b, tipo)));
    let global = Copia::ler(
        tipo,
        OrigemWorkflow::Global,
        arquivo_global(pasta_global, tipo),
    );
    let efetivo = projeto
        .iter()
        .chain(std::iter::once(&global))
        .find(|c| c.usavel())
        .map_or(padrao, |c| WorkflowEfetivo {
            tipo: tipo.to_string(),
            origem: c.origem,
            conteudo: c.script.clone(),
        });
    Some(Resolucao {
        tipo: tipo.to_string(),
        projeto,
        global,
        efetivo,
    })
}

/// A versão efetiva de cada um dos [`TIPOS`]: é o que o preparo instala
/// (`Alvo::com_workflows`).
pub fn efetivos(base: Option<&Path>, pasta_global: &Path) -> Vec<WorkflowEfetivo> {
    TIPOS
        .iter()
        .filter_map(|t| resolver(t, base, pasta_global))
        .map(|r| r.efetivo)
        .collect()
}

/// Uma linha da diferença entre dois textos.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinhaDiferenca {
    /// `igual`, `removida` (só no padrão) ou `acrescentada` (só na cópia).
    pub tipo: &'static str,
    pub texto: String,
}

/// Diferença linha a linha de `antes` (o padrão) para `depois` (a cópia).
pub fn diferenca(antes: &str, depois: &str) -> Vec<LinhaDiferenca> {
    similar::TextDiff::from_lines(antes, depois)
        .iter_all_changes()
        .map(|c| LinhaDiferenca {
            tipo: match c.tag() {
                similar::ChangeTag::Equal => "igual",
                similar::ChangeTag::Delete => "removida",
                similar::ChangeTag::Insert => "acrescentada",
            },
            texto: c.value().trim_end_matches(['\n', '\r']).to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn script(descricao: &str) -> String {
        padrao("impacto").unwrap().replacen(
            "description: '",
            &format!("description: '{descricao} "),
            1,
        )
    }

    fn gravar(arquivo: &Path, texto: &str) {
        fs::create_dir_all(arquivo.parent().unwrap()).unwrap();
        fs::write(arquivo, texto).unwrap();
    }

    #[test]
    fn montar_e_desmontar_guardam_o_hash_do_padrao() {
        let hash = hash_do_padrao("impacto").unwrap();
        let texto = montar("export const meta = {}\n", &hash);
        assert!(texto.starts_with("// "));
        assert_eq!(
            desmontar(&texto),
            ("export const meta = {}\n".to_string(), Some(hash.clone()))
        );
        // Com CRLF (editada no Windows), o mesmo.
        assert_eq!(desmontar(&texto.replace('\n', "\r\n")).1, Some(hash));
        // Sem as linhas do app, o texto inteiro é o script.
        assert_eq!(desmontar("abc\n"), ("abc\n".to_string(), None));
    }

    #[test]
    fn precedencia_projeto_global_padrao() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("base");
        let global = dir.path().join("dados").join(PASTA);
        let hash = hash_do_padrao("impacto").unwrap();

        // Sem cópias: o padrão, também sem base.
        let r = resolver("impacto", Some(&base), &global).unwrap();
        assert_eq!(r.efetivo, WorkflowEfetivo::padrao("impacto").unwrap());
        assert!(!r.projeto.unwrap().existe());
        assert_eq!(
            efetivos(None, &global),
            vec![
                WorkflowEfetivo::padrao("impacto").unwrap(),
                WorkflowEfetivo::padrao("revisar").unwrap()
            ]
        );
        assert!(resolver("perguntar", None, &global).is_none());

        // Global vale para todos.
        gravar(
            &arquivo_global(&global, "impacto"),
            &montar(&script("global"), &hash),
        );
        let r = resolver("impacto", Some(&base), &global).unwrap();
        assert_eq!(r.efetivo.origem, OrigemWorkflow::Global);
        assert_eq!(r.efetivo.conteudo, script("global"));
        assert!(!r.global.padrao_mudou);

        // O projeto vence o global.
        let do_projeto = arquivo_do_projeto(&base, "impacto");
        assert!(do_projeto.ends_with("_templates/workflows/impacto.js"));
        gravar(&do_projeto, &montar(&script("projeto"), &hash));
        let r = resolver("impacto", Some(&base), &global).unwrap();
        assert_eq!(r.efetivo.origem, OrigemWorkflow::Projeto);
        assert_eq!(r.efetivo.conteudo, script("projeto"));
        assert_eq!(
            efetivos(Some(&base), &global),
            vec![r.efetivo, WorkflowEfetivo::padrao("revisar").unwrap()]
        );

        // Cópia do projeto com erro: não é usada, vale a global.
        gravar(
            &do_projeto,
            &montar(&script("projeto").replace("cba-impacto", "outro"), &hash),
        );
        let r = resolver("impacto", Some(&base), &global).unwrap();
        assert_eq!(r.efetivo.origem, OrigemWorkflow::Global);
        let projeto = r.projeto.unwrap();
        assert!(projeto.existe() && !projeto.usavel());
        assert!(
            matches!(&projeto.situacao, SituacaoCopia::Invalida { erros } if erros.contains("linha 2"))
        );
    }

    #[test]
    fn padrao_mudou_quando_o_hash_nao_confere() {
        let dir = tempfile::tempdir().unwrap();
        let global = dir.path().to_path_buf();
        // Partiu de um padrão antigo (outro hash): a cópia não muda e é usada.
        gravar(
            &arquivo_global(&global, "impacto"),
            &montar(&script("minha"), &sha256("padrão antigo")),
        );
        let r = resolver("impacto", None, &global).unwrap();
        assert!(r.global.padrao_mudou);
        assert_eq!(r.efetivo.origem, OrigemWorkflow::Global);
        assert_eq!(r.efetivo.conteudo, script("minha"));
        // Sem as linhas do app, não se sabe de que padrão partiu.
        gravar(&arquivo_global(&global, "impacto"), &script("minha"));
        let r = resolver("impacto", None, &global).unwrap();
        assert!(r.global.padrao_mudou);
        assert_eq!(r.global.hash_padrao, None);
    }

    #[test]
    fn diferenca_por_linha() {
        let d = diferenca("a\nb\nc\n", "a\nB\nc\nd\n");
        let tipos: Vec<_> = d.iter().map(|l| (l.tipo, l.texto.as_str())).collect();
        assert_eq!(
            tipos,
            vec![
                ("igual", "a"),
                ("removida", "b"),
                ("acrescentada", "B"),
                ("igual", "c"),
                ("acrescentada", "d"),
            ]
        );
    }
}
