//! Workflows das análises instalados pelo preparo (spec
//! `docs/specs/2026-11-25-workflows-nas-analises.md`, seção 3.3; #370).
//!
//! | Item | Arquivo |
//! |---|---|
//! | A versão efetiva de cada workflow, com o cabeçalho ([`conteudo_instalado`]) | `<raiz>/.claude/workflows/cba-<tipo>.js`, para cada raiz de [`super::Alvo::raizes_workflows`] |
//! | Linha [`super::LINHA_EXCLUDE_WORKFLOWS`], se a raiz é de um repositório git | `info/exclude` do diretório comum do git |
//! | `Workflow(cba-<tipo>)` e `Skill(cba-<tipo>)` em `permissions.allow` ([`regras_de_permissao`]) | `<raiz>/.claude/settings.local.json`, para cada raiz de [`super::Alvo::raizes`] |
//!
//! A raiz dos workflows é o `git rev-parse --show-toplevel` de cada pasta cadastrada
//! (ou a própria pasta fora do git), e não a raiz do repositório principal: pelo
//! spike da #352 (R2), uma sessão numa worktree não acha os workflows da raiz do
//! repositório principal.
//!
//! O cabeçalho diz que o arquivo é gerado pelo app, a origem da versão efetiva e o
//! SHA-256 do conteúdo que vem depois dele. Com ele, o preparo e o
//! [`super::Preparador::verificar`] distinguem um arquivo do app intacto (regravado
//! quando a versão efetiva muda) de um editado à mão ou de um arquivo que não é do app
//! (nunca sobrescritos nem apagados: [`super::Acao::Preservado`],
//! [`super::Situacao::Divergente`], [`super::Situacao::Conflito`]).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use cba_tools::workflows::{OrigemWorkflow, WorkflowEfetivo};
use sha2::{Digest, Sha256};

use super::{Erro, PASTA_LOCAL, PASTA_WORKFLOWS, erro_io};

/// Primeira linha do cabeçalho de um arquivo instalado pelo app.
const PRIMEIRA_LINHA: &str =
    "// Gerado pelo CBA ao preparar o projeto. Não edite: o app regrava este arquivo.";
const MARCA_NOME: &str = "// cba-workflow: ";
const MARCA_ORIGEM: &str = "// cba-origem: ";
const MARCA_HASH: &str = "// cba-sha256: ";

/// `<raiz>/.claude/workflows`.
pub fn pasta_workflows(raiz: &Path) -> PathBuf {
    raiz.join(PASTA_LOCAL).join(PASTA_WORKFLOWS)
}

/// `<raiz>/.claude/workflows/<nome>.js`.
pub fn arquivo_workflow(raiz: &Path, nome: &str) -> PathBuf {
    pasta_workflows(raiz).join(format!("{nome}.js"))
}

/// Regras de permissão que liberam a chamada dos workflows pelo nome (spike da #352,
/// R3): `Workflow(<nome>)` e `Skill(<nome>)`. Nunca `Workflow` sem nome, que
/// liberaria qualquer script dinâmico.
pub fn regras_de_permissao(workflows: &[WorkflowEfetivo]) -> Vec<String> {
    let mut regras = Vec::new();
    for w in workflows {
        for r in [
            format!("Workflow({})", w.nome()),
            format!("Skill({})", w.nome()),
        ] {
            if !regras.contains(&r) {
                regras.push(r);
            }
        }
    }
    regras
}

/// SHA-256 em hexadecimal.
pub fn sha256(texto: &str) -> String {
    Sha256::digest(texto.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// O arquivo que o preparo grava: o cabeçalho e o conteúdo da versão efetiva.
pub fn conteudo_instalado(w: &WorkflowEfetivo) -> String {
    format!(
        "{PRIMEIRA_LINHA}\n\
         // Para mudar o workflow, use uma cópia customizada (Configurações › Workflows).\n\
         {MARCA_NOME}{}\n\
         {MARCA_ORIGEM}{}\n\
         {MARCA_HASH}{}\n\
         {}",
        w.nome(),
        w.origem.nome(),
        sha256(&w.conteudo),
        w.conteudo
    )
}

/// Cabeçalho lido de um arquivo instalado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cabecalho {
    pub nome: String,
    pub origem: Option<OrigemWorkflow>,
    /// SHA-256 gravado no cabeçalho.
    pub hash: String,
    /// O conteúdo depois do cabeçalho confere com o hash: ninguém o editou.
    pub intacto: bool,
}

/// Lê o cabeçalho de um arquivo instalado pelo app; `None` se o texto não começa
/// por ele (não é do app).
pub fn ler_cabecalho(texto: &str) -> Option<Cabecalho> {
    let resto = texto.strip_prefix(PRIMEIRA_LINHA)?.strip_prefix('\n')?;
    let mut nome = None;
    let mut origem = None;
    let mut posicao = 0;
    // O cabeçalho tem poucas linhas: o hash fecha.
    for linha in resto.split_inclusive('\n').take(8) {
        posicao += linha.len();
        let linha = linha.trim_end_matches(['\n', '\r']);
        if let Some(n) = linha.strip_prefix(MARCA_NOME) {
            nome = Some(n.to_string());
        } else if let Some(o) = linha.strip_prefix(MARCA_ORIGEM) {
            origem = OrigemWorkflow::do_nome(o);
        } else if let Some(h) = linha.strip_prefix(MARCA_HASH) {
            let corpo = &resto[posicao..];
            return Some(Cabecalho {
                nome: nome?,
                origem,
                hash: h.to_string(),
                intacto: sha256(corpo) == h,
            });
        }
    }
    None
}

/// Situação de um arquivo de workflow na raiz.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EstadoArquivo {
    Ausente,
    /// Sem o cabeçalho do app: não é nosso.
    Alheio,
    /// Do app, mas o conteúdo não confere com o hash do cabeçalho: editado à mão.
    Editado,
    /// Do app e intacto; `igual` se é exatamente o arquivo esperado.
    Intacto {
        igual: bool,
        texto: String,
    },
}

/// Lê o arquivo `arquivo` e o compara com o `esperado`.
pub fn estado(arquivo: &Path, esperado: &str) -> Result<EstadoArquivo, Erro> {
    let bytes = match fs::read(arquivo) {
        Ok(b) => b,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(EstadoArquivo::Ausente),
        Err(e) => return Err(erro_io(arquivo, e)),
    };
    let Ok(texto) = String::from_utf8(bytes) else {
        return Ok(EstadoArquivo::Alheio);
    };
    Ok(match ler_cabecalho(&texto) {
        None => EstadoArquivo::Alheio,
        Some(c) if !c.intacto => EstadoArquivo::Editado,
        Some(_) => EstadoArquivo::Intacto {
            igual: texto == esperado,
            texto,
        },
    })
}

/// Grava o arquivo, criando a pasta (atômico).
pub fn gravar(arquivo: &Path, texto: &str) -> Result<(), Erro> {
    if let Some(pasta) = arquivo.parent() {
        fs::create_dir_all(pasta).map_err(|e| erro_io(pasta, e))?;
    }
    cba_kb::arquivo::gravar(arquivo, texto.as_bytes()).map_err(|e| erro_io(arquivo, e))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn padrao() -> WorkflowEfetivo {
        WorkflowEfetivo::padrao("impacto").unwrap()
    }

    #[test]
    fn cabecalho_com_nome_origem_e_hash_do_conteudo() {
        let w = padrao();
        let texto = conteudo_instalado(&w);
        assert!(texto.ends_with(&w.conteudo));
        let c = ler_cabecalho(&texto).unwrap();
        assert_eq!(c.nome, "cba-impacto");
        assert_eq!(c.origem, Some(OrigemWorkflow::Padrao));
        assert_eq!(c.hash, sha256(&w.conteudo));
        assert!(c.intacto);
    }

    #[test]
    fn editado_a_mao_nao_confere_e_alheio_nao_tem_cabecalho() {
        let texto = conteudo_instalado(&padrao());
        let editado = texto.replace("Preparar ' + analise", "Preparar a análise ' + analise");
        assert_ne!(editado, texto);
        assert!(!ler_cabecalho(&editado).unwrap().intacto);
        assert!(ler_cabecalho(&padrao().conteudo).is_none());
        // Sem a linha do hash, não é um cabeçalho completo.
        let sem_hash: String = texto
            .lines()
            .filter(|l| !l.starts_with(MARCA_HASH))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(ler_cabecalho(&sem_hash).is_none());
    }

    #[test]
    fn regras_pelo_nome_sem_repetir() {
        let w = padrao();
        assert_eq!(
            regras_de_permissao(&[w.clone(), w]),
            vec!["Workflow(cba-impacto)", "Skill(cba-impacto)"]
        );
    }
}
