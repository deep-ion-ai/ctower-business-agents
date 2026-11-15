//! Catálogo de pessoas do projeto (épico #93): um documento por pessoa,
//! `pessoas/<slug>.md`, com o nome, os apelidos, o papel atual e o histórico de
//! papéis confirmados por reunião ([`FrontmatterPessoa`]).
//!
//! ```yaml
//! tipo: pessoa
//! nome: Maria Silva
//! apelidos: [Silva, Maria, maria.silva]
//! papel: PO
//! historico:
//!   - reuniao: D-0001/R-0001
//!     papel: PO
//!     confirmado_em: 2026-11-10
//! ```
//!
//! ## O que não fica
//!
//! O catálogo guarda só o nome, os apelidos, os papéis e as referências das
//! reuniões; **nunca as falas**. A API não recebe texto além disso, e o nome, os
//! apelidos e os papéis são textos curtos de uma linha, sem pontuação de frase
//! ([`crate::frontmatter::MAXIMO_DO_NOME`], [`crate::frontmatter::MAXIMO_DO_PAPEL`]);
//! a referência da reunião é um identificador. O e-mail perde o domínio antes de
//! ser gravado.
//!
//! ## Casamento de nomes
//!
//! Sem IA: [`normalizar_nome`] reduz cada variação a uma chave (sem acento, em
//! minúsculas, "Sobrenome, Nome" na ordem "Nome Sobrenome", e-mail sem o domínio,
//! `.`/`_`/`-` como espaço, sem o que vem entre parênteses, como "(Convidado)").
//! Um falante casa com a pessoa quando a chave dele é igual à do nome ou à de um
//! apelido. "Silva, Maria", "maria.silva@empresa" e "Maria Silva" viram
//! `maria silva`. Variações que a normalização não alcança ("Maria S.") entram
//! como apelido ([`Projeto::acrescentar_apelido`]).
//!
//! ## Sugestão de papel
//!
//! O papel sugerido é o confirmado mais recente do histórico
//! ([`FrontmatterPessoa::ultimo_confirmado`]); sem histórico, o papel atual
//! informado ao registrar a pessoa, marcado como não confirmado.
//!
//! ## Gravação e concorrência
//!
//! Gravação atômica, como nos demais documentos. O nome do arquivo vem da chave
//! normalizada do nome, então duas criações da mesma pessoa ao mesmo tempo (app e
//! servidor MCP) disputam o mesmo arquivo: a criação não sobrescreve, e quem perde
//! recebe [`Erro::PessoaJaCadastrada`]. As alterações (apelido, papel, junção)
//! leem, alteram e gravam o documento inteiro, como as da demanda.

use std::collections::HashSet;
use std::fs;
use std::io;

use crate::base::Projeto;
use crate::caminho::{CaminhoRelativo, e_slug, sem_acento, slug};
use crate::documento::Documento;
use crate::erro::{Erro, Resultado};
use crate::frontmatter::{
    Data, ErroCampo, Frontmatter, FrontmatterPessoa, PapelConfirmado, TipoDocumento,
    conferir_reuniao,
};
use crate::layout;

/// Tamanho máximo do slug no nome do arquivo.
const MAXIMO_DO_SLUG: usize = 60;

/// Quantos sufixos (`-2`, `-3`…) [`Projeto::registrar_pessoa`] tenta quando dois
/// nomes diferentes dão o mesmo slug.
const TENTATIVAS_DE_SLUG: u32 = 100;

/// Dados para registrar uma pessoa no catálogo.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NovaPessoa {
    /// Nome como o analista quer ver (`Maria Silva`).
    pub nome: String,
    /// Outras grafias do nome (`Silva, Maria`, `maria.silva@empresa`).
    pub apelidos: Vec<String>,
    /// Papel atual, ainda sem reunião que o confirme.
    pub papel: Option<String>,
}

/// Pessoa lida do catálogo.
#[derive(Debug, Clone, PartialEq)]
pub struct PessoaLida {
    /// Slug da pessoa, o nome do arquivo sem `.md` (`maria-silva`). Identifica a
    /// pessoa nas operações.
    pub slug: String,
    /// `pessoas/<slug>.md`, relativo à raiz do projeto.
    pub arquivo: CaminhoRelativo,
    pub documento: Documento<FrontmatterPessoa>,
}

impl PessoaLida {
    pub fn nome(&self) -> &str {
        &self.documento.frontmatter.nome
    }

    pub fn apelidos(&self) -> &[String] {
        &self.documento.frontmatter.apelidos
    }

    /// Papel sugerido para a próxima reunião: o confirmado mais recente ou, sem
    /// histórico, o papel atual (não confirmado).
    pub fn papel_sugerido(&self) -> Option<PapelSugerido> {
        let f = &self.documento.frontmatter;
        match f.ultimo_confirmado() {
            Some(p) => Some(PapelSugerido {
                papel: p.papel.clone(),
                reuniao: Some(p.reuniao.clone()),
                confirmado_em: Some(p.confirmado_em),
            }),
            None => f.papel.clone().map(|papel| PapelSugerido {
                papel,
                reuniao: None,
                confirmado_em: None,
            }),
        }
    }

    /// Chaves normalizadas do nome e dos apelidos.
    fn chaves(&self) -> impl Iterator<Item = String> + '_ {
        std::iter::once(self.nome())
            .chain(self.apelidos().iter().map(String::as_str))
            .map(normalizar_nome)
            .filter(|c| !c.is_empty())
    }

    /// Se o nome casa com o nome ou um apelido da pessoa.
    pub fn casa_com(&self, nome: &str) -> bool {
        let chave = normalizar_nome(nome);
        !chave.is_empty() && self.chaves().any(|c| c == chave)
    }
}

/// Papel sugerido para uma pessoa.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PapelSugerido {
    pub papel: String,
    /// Reunião em que o papel foi confirmado; `None` quando é o papel atual
    /// informado no registro, sem confirmação.
    pub reuniao: Option<String>,
    pub confirmado_em: Option<Data>,
}

/// Resultado do casamento de um nome com o catálogo.
#[derive(Debug, Clone, PartialEq)]
pub enum Casamento {
    /// Nenhuma pessoa tem o nome ou um apelido com a mesma chave.
    Nenhuma,
    /// Uma pessoa casou.
    Unica(Box<PessoaLida>),
    /// Mais de uma pessoa casou (homônimos ou apelido repetido): o analista escolhe
    /// ou junta as pessoas.
    Ambigua(Vec<PessoaLida>),
}

impl Casamento {
    /// A pessoa, quando o casamento é único.
    pub fn pessoa(&self) -> Option<&PessoaLida> {
        match self {
            Self::Unica(p) => Some(p),
            _ => None,
        }
    }
}

/// Um falante da reunião com a pessoa do catálogo e o papel sugerido.
#[derive(Debug, Clone, PartialEq)]
pub struct Participante {
    /// Nome do falante como veio do transcript.
    pub falante: String,
    pub casamento: Casamento,
    /// Papel sugerido pela pessoa casada (só no casamento único).
    pub papel_sugerido: Option<PapelSugerido>,
}

/// Retrato do catálogo de pessoas de um projeto, para casar vários nomes sem ler
/// os arquivos de novo.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CatalogoDePessoas {
    pessoas: Vec<PessoaLida>,
}

impl CatalogoDePessoas {
    pub fn novo(pessoas: Vec<PessoaLida>) -> Self {
        Self { pessoas }
    }

    /// Pessoas do catálogo, em ordem de slug.
    pub fn pessoas(&self) -> &[PessoaLida] {
        &self.pessoas
    }

    /// Pessoas cujo nome ou apelido casa com `nome`.
    pub fn casar(&self, nome: &str) -> Casamento {
        let mut achadas: Vec<PessoaLida> = self
            .pessoas
            .iter()
            .filter(|p| p.casa_com(nome))
            .cloned()
            .collect();
        match achadas.len() {
            0 => Casamento::Nenhuma,
            1 => Casamento::Unica(Box::new(achadas.remove(0))),
            _ => Casamento::Ambigua(achadas),
        }
    }

    /// Para cada falante, na ordem recebida, a pessoa casada (se houver) e o papel
    /// sugerido pelo histórico dela.
    pub fn sugerir<S: AsRef<str>>(&self, falantes: &[S]) -> Vec<Participante> {
        falantes
            .iter()
            .map(|f| {
                let casamento = self.casar(f.as_ref());
                let papel_sugerido = casamento.pessoa().and_then(PessoaLida::papel_sugerido);
                Participante {
                    falante: f.as_ref().to_owned(),
                    casamento,
                    papel_sugerido,
                }
            })
            .collect()
    }
}

/// Texto em minúsculas, sem acento, com os separadores (pontuação e espaços
/// repetidos) trocados por um espaço. Base do casamento de nomes e de papéis.
pub fn normalizar_texto(texto: &str) -> String {
    let mut saida = String::with_capacity(texto.len());
    let mut separar = false;
    for c in texto.chars().flat_map(char::to_lowercase).map(sem_acento) {
        if c.is_alphanumeric() {
            if separar && !saida.is_empty() {
                saida.push(' ');
            }
            separar = false;
            saida.push(c);
        } else {
            separar = true;
        }
    }
    saida
}

/// Parte local de um e-mail (`maria.silva@empresa` vira `maria.silva`); outros
/// textos voltam sem os espaços das pontas.
pub fn sem_dominio(texto: &str) -> &str {
    let texto = texto.trim();
    match texto.split_once('@') {
        Some((local, _)) if !local.trim().is_empty() => local.trim(),
        _ => texto,
    }
}

/// Chave de casamento de um nome de pessoa. Veja o [módulo](self).
///
/// ```
/// use cba_kb::normalizar_nome;
/// assert_eq!(normalizar_nome("Silva, Maria"), "maria silva");
/// assert_eq!(normalizar_nome("maria.silva@empresa"), "maria silva");
/// assert_eq!(normalizar_nome("MÁRIA  Silva (Convidado)"), "maria silva");
/// ```
pub fn normalizar_nome(texto: &str) -> String {
    let local = sem_dominio(texto);
    // Sem o que vem entre parênteses ou colchetes: "(Convidado)", "[Externo]".
    let mut limpo = String::with_capacity(local.len());
    let mut profundidade = 0usize;
    for c in local.chars() {
        match c {
            '(' | '[' => profundidade += 1,
            ')' | ']' => profundidade = profundidade.saturating_sub(1),
            _ if profundidade == 0 => limpo.push(c),
            _ => {}
        }
    }
    let ordenado = match limpo.split_once(',') {
        Some((sobrenome, nome))
            if !normalizar_texto(sobrenome).is_empty() && !normalizar_texto(nome).is_empty() =>
        {
            format!("{nome} {sobrenome}")
        }
        _ => limpo,
    };
    normalizar_texto(&ordenado)
}

/// Texto de uma linha com os espaços normalizados e o e-mail sem o domínio.
fn limpar(texto: &str) -> String {
    sem_dominio(texto)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn pasta_das_pessoas() -> CaminhoRelativo {
    CaminhoRelativo::novo(layout::PESSOAS).expect("nome fixo válido")
}

/// Slug do arquivo, a partir da chave normalizada do nome.
fn slug_do_nome(nome: &str) -> String {
    let mut s = slug(&normalizar_nome(nome));
    if s.len() > MAXIMO_DO_SLUG {
        s.truncate(MAXIMO_DO_SLUG);
        s.truncate(s.trim_end_matches('-').len());
    }
    s
}

fn corpo_inicial(nome: &str) -> String {
    format!(
        "# {nome}\n\n<!-- Catálogo de pessoas do projeto: só o nome, os apelidos e os \
         papéis por reunião. Nunca as falas. -->\n"
    )
}

/// Acrescenta os apelidos que ainda não casam com o nome nem com outro apelido.
fn acrescentar_apelidos(f: &mut FrontmatterPessoa, apelidos: impl IntoIterator<Item = String>) {
    let mut chaves: HashSet<String> = std::iter::once(&f.nome)
        .chain(&f.apelidos)
        .map(|t| normalizar_nome(t))
        .collect();
    for apelido in apelidos {
        let apelido = limpar(&apelido);
        let chave = normalizar_nome(&apelido);
        if !chave.is_empty() && chaves.insert(chave) {
            f.apelidos.push(apelido);
        }
    }
}

/// Papel atual depois de uma mudança no histórico.
fn atualizar_papel(f: &mut FrontmatterPessoa) {
    if let Some(p) = f.ultimo_confirmado() {
        f.papel = Some(p.papel.clone());
    }
}

impl Projeto {
    fn erro_de_campo(&self, arquivo: &CaminhoRelativo, erro: ErroCampo) -> Erro {
        Erro::Frontmatter {
            arquivo: self.na_base(arquivo),
            erro: crate::ErroFrontmatter {
                campo: Some(erro.campo),
                linha: None,
                mensagem: erro.mensagem,
            },
        }
    }

    fn arquivo_da_pessoa(&self, slug: &str) -> Resultado<CaminhoRelativo> {
        Ok(pasta_das_pessoas().juntar(&format!("{slug}.{}", layout::EXTENSAO))?)
    }

    /// Pessoas do catálogo, em ordem de slug. Sem a pasta `pessoas/`, lista vazia.
    /// Um documento que não pôde ser lido fica de fora, com um aviso no log (o
    /// índice o lista em `problemas`).
    pub fn pessoas(&self) -> Resultado<Vec<PessoaLida>> {
        let pasta = pasta_das_pessoas();
        let entradas = match fs::read_dir(self.caminho(&pasta)) {
            Ok(e) => e,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(erro) => return Err(self.erro_io(&pasta, erro)),
        };
        let mut slugs: Vec<String> = entradas
            .flatten()
            .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
            .filter_map(|e| e.file_name().into_string().ok())
            .filter_map(|n| {
                n.strip_suffix(&format!(".{}", layout::EXTENSAO))
                    .map(str::to_owned)
            })
            .filter(|s| e_slug(s))
            .collect();
        slugs.sort();
        let mut saida = Vec::with_capacity(slugs.len());
        for s in slugs {
            match self.pessoa(&s) {
                Ok(p) => saida.push(p),
                Err(erro) => tracing::warn!(pessoa = %s, %erro, "pessoa ilegível"),
            }
        }
        Ok(saida)
    }

    /// Retrato do catálogo, para casar os falantes de uma reunião.
    pub fn catalogo_de_pessoas(&self) -> Resultado<CatalogoDePessoas> {
        Ok(CatalogoDePessoas::novo(self.pessoas()?))
    }

    /// Para cada falante, a pessoa casada (se houver) e o papel sugerido. Atalho
    /// para [`CatalogoDePessoas::sugerir`] sobre o catálogo atual.
    pub fn sugerir_participantes<S: AsRef<str>>(
        &self,
        falantes: &[S],
    ) -> Resultado<Vec<Participante>> {
        Ok(self.catalogo_de_pessoas()?.sugerir(falantes))
    }

    /// Lê uma pessoa pelo slug.
    pub fn pessoa(&self, slug: &str) -> Resultado<PessoaLida> {
        let inexistente = || Erro::PessoaInexistente {
            projeto: self.nome().to_owned(),
            pessoa: slug.to_owned(),
        };
        if !e_slug(slug) {
            return Err(inexistente());
        }
        let arquivo = self.arquivo_da_pessoa(slug)?;
        if !self.caminho(&arquivo).is_file() {
            return Err(inexistente());
        }
        let documento = self.ler(&arquivo)?;
        Ok(PessoaLida {
            slug: slug.to_owned(),
            arquivo,
            documento,
        })
    }

    /// Registra uma pessoa em `pessoas/<slug do nome>.md`. Recusa com
    /// [`Erro::PessoaJaCadastrada`] se o nome ou um apelido casar com uma pessoa do
    /// catálogo (use [`Projeto::acrescentar_apelido`] ou
    /// [`Projeto::juntar_pessoas`]).
    pub fn registrar_pessoa(&self, nova: &NovaPessoa) -> Resultado<PessoaLida> {
        let nome = limpar(&nova.nome);
        let mut frontmatter = FrontmatterPessoa {
            tipo: TipoDocumento::Pessoa,
            nome: nome.clone(),
            apelidos: Vec::new(),
            papel: nova.papel.as_deref().map(limpar).filter(|p| !p.is_empty()),
            historico: Vec::new(),
        };
        acrescentar_apelidos(&mut frontmatter, nova.apelidos.iter().cloned());
        let pasta = pasta_das_pessoas();
        frontmatter
            .validar()
            .map_err(|e| self.erro_de_campo(&pasta, e))?;

        let catalogo = self.catalogo_de_pessoas()?;
        let ja_cadastrada = |catalogo: &CatalogoDePessoas| {
            std::iter::once(&frontmatter.nome)
                .chain(&frontmatter.apelidos)
                .find_map(|n| match catalogo.casar(n) {
                    Casamento::Nenhuma => None,
                    Casamento::Unica(p) => Some(p.slug.clone()),
                    Casamento::Ambigua(mut v) => Some(v.remove(0).slug),
                })
        };
        if let Some(pessoa) = ja_cadastrada(&catalogo) {
            return Err(Erro::PessoaJaCadastrada {
                projeto: self.nome().to_owned(),
                pessoa,
                nome,
            });
        }

        let base = slug_do_nome(&nome);
        let documento = Documento::novo(frontmatter.clone(), corpo_inicial(&nome));
        for n in 1..=TENTATIVAS_DE_SLUG {
            let s = if n == 1 {
                base.clone()
            } else {
                format!("{base}-{n}")
            };
            let arquivo = self.arquivo_da_pessoa(&s)?;
            match self.criar(&arquivo, &documento) {
                Ok(()) => {
                    return Ok(PessoaLida {
                        slug: s,
                        arquivo,
                        documento,
                    });
                }
                Err(Erro::JaExiste(_)) => {
                    // Criada por outro processo agora há pouco, ou outro nome com o
                    // mesmo slug: só o primeiro caso é a mesma pessoa.
                    if let Ok(outra) = self.pessoa(&s)
                        && std::iter::once(&frontmatter.nome)
                            .chain(&frontmatter.apelidos)
                            .any(|n| outra.casa_com(n))
                    {
                        return Err(Erro::PessoaJaCadastrada {
                            projeto: self.nome().to_owned(),
                            pessoa: s,
                            nome,
                        });
                    }
                }
                Err(e) => return Err(e),
            }
        }
        Err(Erro::JaExiste(
            self.na_base(&self.arquivo_da_pessoa(&base)?),
        ))
    }

    /// Acrescenta um apelido à pessoa (o e-mail perde o domínio). Um apelido que já
    /// casa com o nome ou outro apelido dela não muda nada; um que casa com outra
    /// pessoa é recusado com [`Erro::ApelidoDeOutraPessoa`].
    pub fn acrescentar_apelido(&self, pessoa: &str, apelido: &str) -> Resultado<PessoaLida> {
        let mut lida = self.pessoa(pessoa)?;
        if let Some(outra) = self
            .pessoas()?
            .into_iter()
            .find(|p| p.slug != lida.slug && p.casa_com(apelido))
        {
            return Err(Erro::ApelidoDeOutraPessoa {
                projeto: self.nome().to_owned(),
                apelido: limpar(apelido),
                outra: outra.slug,
            });
        }
        let antes = lida.documento.frontmatter.clone();
        acrescentar_apelidos(&mut lida.documento.frontmatter, [apelido.to_owned()]);
        if lida.documento.frontmatter != antes {
            self.gravar(&lida.arquivo, &lida.documento)?;
        }
        Ok(lida)
    }

    /// Registra o papel da pessoa confirmado pelo analista na reunião `reuniao`
    /// (`D-0001/R-0002`), na data `em`. Confirmar de novo na mesma reunião troca o
    /// papel dela. O papel atual passa a ser o confirmado mais recente.
    pub fn registrar_papel(
        &self,
        pessoa: &str,
        reuniao: &str,
        papel: &str,
        em: Data,
    ) -> Resultado<PessoaLida> {
        let mut lida = self.pessoa(pessoa)?;
        let reuniao = reuniao.trim();
        conferir_reuniao("reuniao", reuniao).map_err(|e| self.erro_de_campo(&lida.arquivo, e))?;
        let papel = limpar(papel);
        let antes = lida.documento.frontmatter.clone();
        let f = &mut lida.documento.frontmatter;
        let confirmado = PapelConfirmado {
            reuniao: reuniao.to_owned(),
            papel,
            confirmado_em: em,
        };
        match f.historico.iter_mut().find(|p| p.reuniao == reuniao) {
            Some(p) => *p = confirmado,
            None => f.historico.push(confirmado),
        }
        atualizar_papel(f);
        if lida.documento.frontmatter != antes {
            self.gravar(&lida.arquivo, &lida.documento)?;
        }
        Ok(lida)
    }

    /// Junta duas pessoas que são a mesma: `remover` some do catálogo e `manter`
    /// fica com o nome e os apelidos das duas e o histórico das duas. Numa reunião
    /// presente nos dois históricos, vale a confirmação mais recente (no mesmo dia,
    /// a de `manter`). Grava `manter` antes de apagar `remover`: uma falha no meio
    /// deixa as duas, nunca perde dados.
    pub fn juntar_pessoas(&self, manter: &str, remover: &str) -> Resultado<PessoaLida> {
        if manter == remover {
            return Err(Erro::JuntarMesmaPessoa(manter.to_owned()));
        }
        let mut fica = self.pessoa(manter)?;
        let sai = self.pessoa(remover)?;
        let s = sai.documento.frontmatter;
        let f = &mut fica.documento.frontmatter;

        acrescentar_apelidos(f, std::iter::once(s.nome).chain(s.apelidos));
        for p in s.historico {
            match f.historico.iter_mut().find(|q| q.reuniao == p.reuniao) {
                Some(q) if p.confirmado_em > q.confirmado_em => *q = p,
                Some(_) => {}
                None => f.historico.push(p),
            }
        }
        // Ordem cronológica estável: no mesmo dia, a ordem de registro.
        f.historico.sort_by_key(|p| p.confirmado_em);
        if f.papel.is_none() {
            f.papel = s.papel;
        }
        atualizar_papel(f);

        self.gravar(&fica.arquivo, &fica.documento)?;
        fs::remove_file(self.caminho(&sai.arquivo)).map_err(|e| self.erro_io(&sai.arquivo, e))?;
        Ok(fica)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normaliza_variacoes_do_teams() {
        for v in [
            "Maria Silva",
            "Silva, Maria",
            "maria.silva@empresa",
            "maria.silva@empresa.com.br",
            "MARIA SILVA",
            "Mária Sílva",
            "maria_silva",
            "Silva, Maria (Convidado)",
            "  Maria   Silva [Externo] ",
        ] {
            assert_eq!(normalizar_nome(v), "maria silva", "{v}");
        }
        assert_eq!(normalizar_nome("João da Conceição"), "joao da conceicao");
        assert_eq!(normalizar_nome("Maria S."), "maria s");
        assert_eq!(normalizar_nome(", Maria"), "maria");
        assert_eq!(normalizar_nome("??"), "");
    }

    #[test]
    fn slug_vem_da_chave() {
        assert_eq!(slug_do_nome("Silva, Maria"), "maria-silva");
        assert_eq!(slug_do_nome("José Ângelo"), "jose-angelo");
        let longo = "Nome ".repeat(20);
        assert!(slug_do_nome(&longo).len() <= MAXIMO_DO_SLUG);
    }

    #[test]
    fn email_sem_dominio() {
        assert_eq!(sem_dominio(" maria.silva@empresa "), "maria.silva");
        assert_eq!(sem_dominio("@empresa"), "@empresa");
        assert_eq!(limpar("Maria   Silva"), "Maria Silva");
    }
}
