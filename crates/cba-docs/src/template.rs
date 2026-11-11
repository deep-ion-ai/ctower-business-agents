//! Templates em Markdown com marcadores `{{nome}}`.
//!
//! - O conjunto padrão é embutido no app ([`Templates::padrao`]).
//! - A cópia editável fica em `<base>/_templates/<tipo>.md`
//!   ([`Templates::copiar_padrao`]). Ao carregar ([`Templates::da_base`]), o
//!   template da base é usado no lugar do padrão.
//! - Um comentário HTML no início do template (`<!-- ... -->`) é instrução para
//!   quem edita e não vai para o documento.
//! - Marcador é `{{nome}}`, com `nome` de `a-z`, `0-9` e `_` (espaços em volta
//!   são aceitos). Marcador que o tipo não conhece gera [`Aviso`] e fica no texto
//!   como está, para o analista ver onde errou. Qualquer outro `{{` é texto.

use std::collections::BTreeMap;
use std::fmt;

use cba_kb::{Base, CaminhoRelativo, TipoDocumento, layout};

use crate::modelo::{MODELOS, Modelo, modelo};

/// De onde veio um template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origem {
    /// Conjunto padrão embutido no app.
    Padrao,
    /// Arquivo da base, relativo à raiz da base (`_templates/regra.md`).
    Base(CaminhoRelativo),
}

impl fmt::Display for Origem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Padrao => f.write_str("template padrão"),
            Self::Base(c) => write!(f, "{c}"),
        }
    }
}

/// Algo que o analista deve saber, sem impedir a montagem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Aviso {
    /// O template tem um marcador que o tipo não conhece.
    MarcadorDesconhecido {
        tipo: TipoDocumento,
        origem: Origem,
        marcador: String,
        /// Linha no template (a partir de 1).
        linha: usize,
    },
    /// Uma seção tem registros, mas o template não tem o marcador dela; o
    /// conteúdo foi para o fim do documento.
    SecaoSemMarcador { tipo: TipoDocumento, secao: String },
    /// Um registro aponta para uma seção que o tipo não tem; foi para o fim do
    /// documento.
    SecaoDesconhecida { tipo: TipoDocumento, secao: String },
    /// O template da base não pôde ser lido; o padrão foi usado.
    TemplateIlegivel {
        tipo: TipoDocumento,
        arquivo: CaminhoRelativo,
        erro: String,
    },
}

impl fmt::Display for Aviso {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MarcadorDesconhecido {
                tipo,
                origem,
                marcador,
                linha,
            } => write!(
                f,
                "{origem}, linha {linha}: marcador desconhecido `{{{{{marcador}}}}}` no template de {tipo}"
            ),
            Self::SecaoSemMarcador { tipo, secao } => write!(
                f,
                "o template de {tipo} não tem o marcador `{{{{{secao}}}}}`; a seção foi para o fim do documento"
            ),
            Self::SecaoDesconhecida { tipo, secao } => write!(
                f,
                "{tipo} não tem a seção `{secao}`; o registro foi para o fim do documento"
            ),
            Self::TemplateIlegivel {
                tipo,
                arquivo,
                erro,
            } => write!(
                f,
                "{arquivo}: template de {tipo} ilegível ({erro}); usado o padrão"
            ),
        }
    }
}

/// Pedaço de um template analisado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Parte {
    Texto(String),
    /// Marcador conhecido do tipo.
    Marcador(String),
}

/// Template analisado de um tipo de documento.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Template {
    tipo: TipoDocumento,
    origem: Origem,
    pub(crate) partes: Vec<Parte>,
}

impl Template {
    /// Analisa o texto de um template do tipo. Devolve os avisos de marcadores
    /// desconhecidos. `None` se o tipo não tiver template (`sistema`,
    /// `memoria-area`).
    pub fn analisar(
        tipo: TipoDocumento,
        texto: &str,
        origem: Origem,
    ) -> Option<(Self, Vec<Aviso>)> {
        let modelo = modelo(tipo)?;
        let texto = texto.replace("\r\n", "\n");
        let (deslocamento, corpo) = sem_comentario_inicial(&texto);
        let mut partes = Vec::new();
        let mut avisos = Vec::new();
        let mut literal = String::new();
        let mut resto = corpo;
        let mut linha = 1 + deslocamento;
        while let Some(inicio) = resto.find("{{") {
            let antes = &resto[..inicio];
            literal.push_str(antes);
            linha += antes.matches('\n').count();
            let depois = &resto[inicio + 2..];
            let nome = depois
                .find("}}")
                .map(|fim| (fim, depois[..fim].trim()))
                .filter(|(_, n)| e_nome_de_marcador(n));
            match nome {
                Some((fim, nome)) => {
                    let original = &resto[inicio..inicio + 2 + fim + 2];
                    if modelo.conhece(nome) {
                        if !literal.is_empty() {
                            partes.push(Parte::Texto(std::mem::take(&mut literal)));
                        }
                        partes.push(Parte::Marcador(nome.to_owned()));
                    } else {
                        avisos.push(Aviso::MarcadorDesconhecido {
                            tipo,
                            origem: origem.clone(),
                            marcador: nome.to_owned(),
                            linha,
                        });
                        literal.push_str(original);
                    }
                    linha += original.matches('\n').count();
                    resto = &resto[inicio + original.len()..];
                }
                None => {
                    literal.push_str("{{");
                    resto = depois;
                }
            }
        }
        literal.push_str(resto);
        if !literal.is_empty() {
            partes.push(Parte::Texto(literal));
        }
        Some((
            Self {
                tipo,
                origem,
                partes,
            },
            avisos,
        ))
    }

    pub fn tipo(&self) -> TipoDocumento {
        self.tipo
    }

    pub fn origem(&self) -> &Origem {
        &self.origem
    }

    pub(crate) fn modelo(&self) -> &'static Modelo {
        modelo(self.tipo).expect("template só existe para tipos com modelo")
    }

    /// Marcadores usados no template, na ordem em que aparecem.
    pub fn marcadores(&self) -> impl Iterator<Item = &str> {
        self.partes.iter().filter_map(|p| match p {
            Parte::Marcador(m) => Some(m.as_str()),
            Parte::Texto(_) => None,
        })
    }
}

fn e_nome_de_marcador(nome: &str) -> bool {
    !nome.is_empty()
        && nome
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

/// Remove o comentário HTML do início, se houver. Devolve quantas linhas foram
/// removidas (para as linhas dos avisos) e o resto.
fn sem_comentario_inicial(texto: &str) -> (usize, &str) {
    let sem_espacos = texto.trim_start();
    if !sem_espacos.starts_with("<!--") {
        return (0, texto);
    }
    let Some(fim) = sem_espacos.find("-->") else {
        return (0, texto);
    };
    let depois = &sem_espacos[fim + 3..];
    let resto = depois.trim_start_matches(['\n', ' ', '\t']);
    let removido = &texto[..texto.len() - resto.len()];
    (removido.matches('\n').count(), resto)
}

/// Um template por tipo de documento.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Templates {
    por_tipo: BTreeMap<&'static str, Template>,
}

impl Templates {
    /// Conjunto padrão embutido no app.
    pub fn padrao() -> Self {
        let por_tipo = MODELOS
            .iter()
            .map(|m| (m.tipo.como_str(), padrao_do(m)))
            .collect();
        Self { por_tipo }
    }

    /// Templates da base: `<base>/_templates/<tipo>.md` quando existir, o padrão
    /// para os demais. Devolve os avisos (marcadores desconhecidos, arquivos
    /// ilegíveis). Um template ilegível não impede a montagem: o padrão é usado.
    pub fn da_base(base: &Base) -> (Self, Vec<Aviso>) {
        let mut por_tipo = BTreeMap::new();
        let mut avisos = Vec::new();
        for m in &MODELOS {
            let relativo = arquivo_do_template(m.tipo);
            let template = if base.caminho(&relativo).is_file() {
                match base.ler_texto(&relativo) {
                    Ok(texto) => {
                        let (t, mut a) = Template::analisar(m.tipo, &texto, Origem::Base(relativo))
                            .expect("todo modelo tem template");
                        avisos.append(&mut a);
                        t
                    }
                    Err(erro) => {
                        avisos.push(Aviso::TemplateIlegivel {
                            tipo: m.tipo,
                            arquivo: relativo,
                            erro: erro.to_string(),
                        });
                        padrao_do(m)
                    }
                }
            } else {
                padrao_do(m)
            };
            por_tipo.insert(m.tipo.como_str(), template);
        }
        (Self { por_tipo }, avisos)
    }

    /// Grava em `<base>/_templates/` a cópia editável dos templates padrão que
    /// ainda não existem lá. Nunca sobrescreve o que o analista editou. Devolve os
    /// arquivos criados, relativos à raiz da base.
    pub fn copiar_padrao(base: &Base) -> cba_kb::Resultado<Vec<CaminhoRelativo>> {
        let mut criados = Vec::new();
        for m in &MODELOS {
            let relativo = arquivo_do_template(m.tipo);
            if base.caminho(&relativo).exists() {
                continue;
            }
            base.gravar_texto(&relativo, m.padrao)?;
            criados.push(relativo);
        }
        Ok(criados)
    }

    /// Template do tipo, se o tipo tiver template.
    pub fn template(&self, tipo: TipoDocumento) -> Option<&Template> {
        self.por_tipo.get(tipo.como_str())
    }
}

fn padrao_do(m: &Modelo) -> Template {
    let (t, avisos) =
        Template::analisar(m.tipo, m.padrao, Origem::Padrao).expect("todo modelo tem template");
    debug_assert!(
        avisos.is_empty(),
        "template padrão de {}: {avisos:?}",
        m.tipo
    );
    t
}

/// `_templates/<tipo>.md`, relativo à raiz da base.
pub fn arquivo_do_template(tipo: TipoDocumento) -> CaminhoRelativo {
    CaminhoRelativo::novo(&format!("{}/{}.md", layout::TEMPLATES, tipo.como_str()))
        .expect("pasta e tipo formam um caminho válido")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn padroes_sem_avisos_e_com_todas_as_secoes() {
        for m in &MODELOS {
            let (t, avisos) = Template::analisar(m.tipo, m.padrao, Origem::Padrao).unwrap();
            assert!(avisos.is_empty(), "{}: {avisos:?}", m.tipo);
            let usados: Vec<&str> = t.marcadores().collect();
            for s in m.secoes {
                assert!(
                    usados.contains(&s.marcador),
                    "{} sem a seção {}",
                    m.tipo,
                    s.marcador
                );
            }
            assert!(
                !t.partes
                    .iter()
                    .any(|p| matches!(p, Parte::Texto(x) if x.contains("<!--"))),
                "o comentário inicial não vai para o documento"
            );
        }
    }

    #[test]
    fn marcador_desconhecido_gera_aviso_com_linha() {
        let texto = "<!--\ninstrução\n-->\n# {{titulo}}\n\n{{ enunciado }}\n{{inventado}} e {{Nao}} e {{ }}\n";
        let (t, avisos) = Template::analisar(TipoDocumento::Regra, texto, Origem::Padrao).unwrap();
        assert_eq!(
            avisos,
            vec![Aviso::MarcadorDesconhecido {
                tipo: TipoDocumento::Regra,
                origem: Origem::Padrao,
                marcador: "inventado".into(),
                linha: 7,
            }]
        );
        assert_eq!(t.marcadores().collect::<Vec<_>>(), ["titulo", "enunciado"]);
        assert_eq!(
            t.partes.last().unwrap(),
            &Parte::Texto("\n{{inventado}} e {{Nao}} e {{ }}\n".into())
        );
        assert!(
            avisos[0]
                .to_string()
                .contains("linha 7: marcador desconhecido `{{inventado}}`")
        );
    }

    #[test]
    fn crlf_vira_lf() {
        let (t, _) =
            Template::analisar(TipoDocumento::Termo, "# {{termo}}\r\n", Origem::Padrao).unwrap();
        assert_eq!(
            t.partes,
            vec![
                Parte::Texto("# ".into()),
                Parte::Marcador("termo".into()),
                Parte::Texto("\n".into())
            ]
        );
    }

    #[test]
    fn tipos_sem_template() {
        assert!(Template::analisar(TipoDocumento::Sistema, "x", Origem::Padrao).is_none());
        assert!(
            Templates::padrao()
                .template(TipoDocumento::MemoriaArea)
                .is_none()
        );
    }
}
