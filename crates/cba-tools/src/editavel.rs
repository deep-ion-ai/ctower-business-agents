//! Textos do produto editáveis na base: os roteiros dos fluxos ([`crate::roteiro`])
//! e os prompts prontos ([`crate::prompts`]).
//!
//! Seguem o padrão dos templates dos documentos (`cba_docs::Templates`):
//!
//! - o texto padrão é embutido no app (arquivos `.md` em `crates/cba-tools/textos/`,
//!   incluídos com `include_str!`);
//! - um arquivo com o mesmo nome em `<base>/_templates/<pasta>/` é usado no lugar do
//!   padrão; [`copiar`] grava a cópia editável sem sobrescrever o que já existe;
//! - um comentário HTML no início é instrução para quem edita e não vai para o
//!   agente;
//! - variáveis são `{{nome}}`, com `nome` de `a-z`, `0-9` e `_` (espaços em volta são
//!   aceitos). Variável que o texto não conhece gera [`Aviso`] e fica como está;
//!   variável essencial ausente vai para o fim, com aviso, para nada se perder;
//! - arquivo da base ilegível gera aviso e o padrão é usado.

use std::fmt;

use cba_kb::{Base, CaminhoRelativo, layout};

/// De onde veio um texto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origem {
    /// Texto padrão embutido no app.
    Padrao,
    /// Arquivo da base, relativo à raiz da base (`_templates/roteiros/perguntar.md`).
    Base(CaminhoRelativo),
}

impl fmt::Display for Origem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Padrao => f.write_str("texto padrão"),
            Self::Base(c) => write!(f, "{c}"),
        }
    }
}

/// Algo que o analista deve saber sobre um roteiro ou prompt editado na base, sem
/// impedir o uso.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Aviso {
    /// O texto tem uma variável que não existe; ficou no texto como está.
    VariavelDesconhecida {
        origem: Origem,
        variavel: String,
        /// Linha no arquivo (a partir de 1).
        linha: usize,
    },
    /// O texto não usa uma variável essencial; o valor foi para o fim.
    VariavelAusente { origem: Origem, variavel: String },
    /// O arquivo da base não pôde ser lido; o padrão foi usado.
    Ilegivel {
        arquivo: CaminhoRelativo,
        erro: String,
    },
}

impl fmt::Display for Aviso {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::VariavelDesconhecida {
                origem,
                variavel,
                linha,
            } => write!(
                f,
                "{origem}, linha {linha}: variável desconhecida `{{{{{variavel}}}}}`"
            ),
            Self::VariavelAusente { origem, variavel } => write!(
                f,
                "{origem} não usa a variável `{{{{{variavel}}}}}`; o valor foi para o fim"
            ),
            Self::Ilegivel { arquivo, erro } => {
                write!(f, "{arquivo}: arquivo ilegível ({erro}); usado o padrão")
            }
        }
    }
}

/// Um texto editável: o padrão embutido e o arquivo na base que o substitui.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Padrao {
    /// Pasta dentro de `_templates/` (`roteiros`, `prompts`).
    pub pasta: &'static str,
    /// Nome do arquivo, sem a extensão.
    pub nome: &'static str,
    /// Texto padrão.
    pub texto: &'static str,
}

impl Padrao {
    /// `_templates/<pasta>/<nome>.md`, relativo à raiz da base.
    pub fn arquivo(&self) -> CaminhoRelativo {
        CaminhoRelativo::novo(&format!(
            "{}/{}/{}.{}",
            layout::TEMPLATES,
            self.pasta,
            self.nome,
            layout::EXTENSAO
        ))
        .expect("pasta e nome fixos formam um caminho válido")
    }

    /// O texto da base, quando existe e é legível; senão, o padrão.
    pub fn da_base(&self, base: &Base) -> (Texto, Option<Aviso>) {
        let relativo = self.arquivo();
        if !base.caminho(&relativo).is_file() {
            return (self.embutido(), None);
        }
        match base.ler_texto(&relativo) {
            Ok(texto) => (
                Texto {
                    texto,
                    origem: Origem::Base(relativo),
                },
                None,
            ),
            Err(erro) => (
                self.embutido(),
                Some(Aviso::Ilegivel {
                    arquivo: relativo,
                    erro: erro.to_string(),
                }),
            ),
        }
    }

    pub fn embutido(&self) -> Texto {
        Texto {
            texto: self.texto.to_owned(),
            origem: Origem::Padrao,
        }
    }
}

/// Grava em `<base>/_templates/` a cópia editável dos textos padrão que ainda não
/// existem lá. Nunca sobrescreve o que o analista editou. Devolve os arquivos
/// criados, relativos à raiz da base.
pub(crate) fn copiar(
    base: &Base,
    padroes: impl IntoIterator<Item = Padrao>,
) -> cba_kb::Resultado<Vec<CaminhoRelativo>> {
    let mut criados = Vec::new();
    for p in padroes {
        let relativo = p.arquivo();
        if base.caminho(&relativo).exists() {
            continue;
        }
        base.gravar_texto(&relativo, p.texto)?;
        criados.push(relativo);
    }
    Ok(criados)
}

/// Grupo de um texto editável na base, para a tela de configurações (#39).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grupo {
    /// Template de um tipo de documento (`_templates/<tipo>.md`).
    Template,
    /// Roteiro de um fluxo (`_templates/roteiros/<fluxo>.md`).
    Roteiro,
    /// Prompt pronto (`_templates/prompts/<id>.md`).
    Prompt,
}

/// Um texto que o analista pode editar na base, com o padrão embutido no app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextoEditavel {
    pub grupo: Grupo,
    /// `analise-impacto`, `perguntar`, `aprofundar`.
    pub id: &'static str,
    /// Nome para o analista.
    pub rotulo: &'static str,
    /// Relativo à raiz da base.
    pub arquivo: CaminhoRelativo,
    /// Texto padrão embutido no app.
    pub padrao: &'static str,
}

fn rotulo_do_template(tipo: cba_kb::frontmatter::TipoDocumento) -> &'static str {
    use cba_kb::frontmatter::TipoDocumento as T;
    match tipo {
        T::AnaliseImpacto => "Análise de impacto",
        T::Pergunta => "Pergunta ao sistema",
        T::Funcionalidade => "Documento negocial da funcionalidade",
        T::CasoDeUso => "Caso de uso",
        T::Regra => "Regra de negócio",
        T::Termo => "Termo do glossário",
        T::Sugestao => "Sugestão de alteração",
        T::Projeto => "Projeto",
        T::MemoriaArea => "Memória da área",
        T::Demanda => "Demanda",
        T::Pessoa => "Pessoa",
        T::Ata => "Ata de reunião",
    }
}

fn rotulo_do_roteiro(fluxo: cba_core::analise::Fluxo) -> &'static str {
    use cba_core::analise::Fluxo;
    match fluxo {
        Fluxo::Perguntar => "Perguntar ao sistema",
        Fluxo::Explicar => "Explicar funcionalidade",
        Fluxo::Impacto => "Analisar impacto",
    }
}

/// Todos os textos editáveis na base: os templates dos documentos, os roteiros dos
/// fluxos e os prompts prontos, nessa ordem. Um arquivo da base com o mesmo caminho
/// sobrepõe o padrão.
pub fn textos_editaveis() -> Vec<TextoEditavel> {
    let mut lista: Vec<TextoEditavel> = cba_docs::MODELOS
        .iter()
        .map(|m| TextoEditavel {
            grupo: Grupo::Template,
            id: m.tipo.como_str(),
            rotulo: rotulo_do_template(m.tipo),
            arquivo: cba_docs::arquivo_do_template(m.tipo),
            padrao: m.padrao,
        })
        .collect();
    lista.extend(crate::roteiro::FLUXOS.iter().map(|f| {
        let p = crate::roteiro::padrao(*f);
        TextoEditavel {
            grupo: Grupo::Roteiro,
            id: p.nome,
            rotulo: rotulo_do_roteiro(*f),
            arquivo: p.arquivo(),
            padrao: p.texto,
        }
    }));
    lista.extend(crate::prompts::DEFINICOES.iter().map(|d| {
        let p = crate::prompts::padrao(d);
        TextoEditavel {
            grupo: Grupo::Prompt,
            id: d.id,
            rotulo: d.rotulo,
            arquivo: p.arquivo(),
            padrao: p.texto,
        }
    }));
    lista
}

/// Variável ou marcador que um texto editável aceita, com a descrição para a tela.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variavel {
    pub nome: String,
    pub descricao: String,
}

/// Problema encontrado ao validar o texto de um editor, sem impedir o uso: o mesmo
/// aviso que o agente receberia ao carregar o texto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problema {
    /// Linha no texto (a partir de 1), quando o problema tem uma.
    pub linha: Option<usize>,
    /// A variável ou o marcador a que o problema se refere.
    pub variavel: String,
    /// Se o texto não usa a variável (em vez de usar uma que não existe).
    pub ausente: bool,
    pub mensagem: String,
}

fn variavel(nome: &str, descricao: &str) -> Variavel {
    Variavel {
        nome: nome.to_owned(),
        descricao: descricao.to_owned(),
    }
}

fn descricao_da_variavel(nome: &str) -> &'static str {
    match nome {
        "id" => "Identificador da análise (A-NNNN).",
        "titulo" => "Título da análise.",
        "fluxo" => "Nome do fluxo da análise (pergunta, explicação ou impacto).",
        "documento" => "Caminho do documento da análise, relativo à base.",
        "entrada" => "A entrada do analista (pergunta, funcionalidade ou demanda).",
        "secoes" => "Lista das seções do documento, com o que vai em cada uma.",
        "secoes_obrigatorias" => "Seções que a análise precisa preencher para ser aprovada.",
        "contexto" => "Contexto da base de conhecimento para o fluxo.",
        _ => "",
    }
}

fn modelo_do_template(id: &str) -> Option<&'static cba_docs::Modelo> {
    cba_docs::MODELOS.iter().find(|m| m.tipo.como_str() == id)
}

/// Variáveis ou marcadores que o texto aceita. Vazio para um id desconhecido.
pub fn variaveis(grupo: Grupo, id: &str) -> Vec<Variavel> {
    let nomes: &[&str] = match grupo {
        Grupo::Roteiro => &crate::roteiro::VARIAVEIS[..],
        Grupo::Prompt => &crate::prompts::VARIAVEIS[..],
        Grupo::Template => {
            return modelo_do_template(id)
                .map(|m| {
                    m.secoes
                        .iter()
                        .map(|s| variavel(s.marcador, &format!("Seção \"{}\".", s.titulo)))
                        .chain(
                            m.campos
                                .iter()
                                .map(|c| variavel(c, "Campo do cabeçalho do documento.")),
                        )
                        .collect()
                })
                .unwrap_or_default();
        }
    };
    nomes
        .iter()
        .map(|n| variavel(n, descricao_da_variavel(n)))
        .collect()
}

/// Valida o texto de um editor, sem gravar, com as mesmas regras de quando o agente
/// o carrega: variável desconhecida e variável essencial ausente (roteiros e
/// prompts), marcador desconhecido (templates). `arquivo` é o caminho do texto na
/// base, para as mensagens.
pub fn validar(grupo: Grupo, id: &str, arquivo: &CaminhoRelativo, texto: &str) -> Vec<Problema> {
    let (nomes, essenciais): (&[&str], &[&str]) = match grupo {
        Grupo::Roteiro => (
            &crate::roteiro::VARIAVEIS[..],
            &crate::roteiro::ESSENCIAIS[..],
        ),
        Grupo::Prompt => (
            &crate::prompts::VARIAVEIS[..],
            &crate::prompts::ESSENCIAIS[..],
        ),
        Grupo::Template => {
            let Some(tipo) = modelo_do_template(id).map(|m| m.tipo) else {
                return Vec::new();
            };
            let origem = cba_docs::Origem::Base(arquivo.clone());
            let Some((_, avisos)) = cba_docs::Template::analisar(tipo, texto, origem) else {
                return Vec::new();
            };
            return avisos
                .into_iter()
                .filter_map(|a| match &a {
                    cba_docs::Aviso::MarcadorDesconhecido {
                        marcador, linha, ..
                    } => Some(Problema {
                        linha: Some(*linha),
                        variavel: marcador.clone(),
                        ausente: false,
                        mensagem: a.to_string(),
                    }),
                    _ => None,
                })
                .collect();
        }
    };
    let texto = Texto {
        texto: texto.to_owned(),
        origem: Origem::Base(arquivo.clone()),
    };
    let (_, avisos) = texto.preencher(
        |n| nomes.contains(&n).then(String::new),
        essenciais,
        |_, _| String::new(),
    );
    avisos
        .into_iter()
        .filter_map(|a| match &a {
            Aviso::VariavelDesconhecida {
                variavel, linha, ..
            } => Some(Problema {
                linha: Some(*linha),
                variavel: variavel.clone(),
                ausente: false,
                mensagem: a.to_string(),
            }),
            Aviso::VariavelAusente { variavel, .. } => Some(Problema {
                linha: None,
                variavel: variavel.clone(),
                ausente: true,
                mensagem: a.to_string(),
            }),
            Aviso::Ilegivel { .. } => None,
        })
        .collect()
}

/// Um texto carregado, ainda com as variáveis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Texto {
    pub texto: String,
    pub origem: Origem,
}

impl Texto {
    /// Substitui as variáveis conhecidas pelo valor de `valor`. As de `essenciais`
    /// que o texto não usa vão para o fim, por `ao_fim(variavel, valor)`, com aviso.
    /// Os valores não são reexaminados: um `{{` dentro deles fica como está.
    pub fn preencher(
        &self,
        valor: impl Fn(&str) -> Option<String>,
        essenciais: &[&str],
        ao_fim: impl Fn(&str, &str) -> String,
    ) -> (String, Vec<Aviso>) {
        let texto = self.texto.replace("\r\n", "\n");
        let (deslocamento, corpo) = sem_comentario_inicial(&texto);
        let mut saida = String::with_capacity(corpo.len());
        let mut avisos = Vec::new();
        let mut usadas: Vec<String> = Vec::new();
        let mut resto = corpo;
        let mut linha = 1 + deslocamento;
        while let Some(inicio) = resto.find("{{") {
            let antes = &resto[..inicio];
            saida.push_str(antes);
            linha += antes.matches('\n').count();
            let depois = &resto[inicio + 2..];
            let nome = depois
                .find("}}")
                .map(|fim| (fim, depois[..fim].trim()))
                .filter(|(_, n)| e_nome_de_variavel(n));
            match nome {
                Some((fim, nome)) => {
                    let original = &resto[inicio..inicio + 2 + fim + 2];
                    match valor(nome) {
                        Some(v) => {
                            saida.push_str(&v);
                            usadas.push(nome.to_owned());
                        }
                        None => {
                            avisos.push(Aviso::VariavelDesconhecida {
                                origem: self.origem.clone(),
                                variavel: nome.to_owned(),
                                linha,
                            });
                            saida.push_str(original);
                        }
                    }
                    linha += original.matches('\n').count();
                    resto = &resto[inicio + original.len()..];
                }
                None => {
                    saida.push_str("{{");
                    resto = depois;
                }
            }
        }
        saida.push_str(resto);
        for e in essenciais {
            if usadas.iter().any(|u| u == e) {
                continue;
            }
            let Some(v) = valor(e) else { continue };
            avisos.push(Aviso::VariavelAusente {
                origem: self.origem.clone(),
                variavel: (*e).to_owned(),
            });
            saida.push_str(&ao_fim(e, &v));
        }
        (saida, avisos)
    }
}

fn e_nome_de_variavel(nome: &str) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn textos_editaveis_cobrem_templates_roteiros_e_prompts() {
        let lista = textos_editaveis();
        let de = |g: Grupo| lista.iter().filter(|t| t.grupo == g).count();
        assert_eq!(de(Grupo::Template), cba_docs::MODELOS.len());
        assert_eq!(de(Grupo::Roteiro), 3);
        assert_eq!(de(Grupo::Prompt), 5);
        let arquivos: std::collections::BTreeSet<&str> =
            lista.iter().map(|t| t.arquivo.as_str()).collect();
        assert_eq!(arquivos.len(), lista.len(), "caminhos distintos");
        assert!(arquivos.contains("_templates/analise-impacto.md"));
        assert!(arquivos.contains("_templates/roteiros/perguntar.md"));
        assert!(arquivos.contains("_templates/prompts/aprofundar.md"));
        assert!(lista.iter().all(|t| !t.padrao.trim().is_empty()));
    }

    fn texto(t: &str) -> Texto {
        Texto {
            texto: t.into(),
            origem: Origem::Padrao,
        }
    }

    fn valor(nome: &str) -> Option<String> {
        match nome {
            "id" => Some("A-0001".into()),
            "contexto" => Some("{{id}} literal".into()),
            _ => None,
        }
    }

    #[test]
    fn substitui_tira_o_comentario_e_avisa_do_desconhecido() {
        let t =
            texto("<!--\nnota\n-->\nAnálise {{ id }}.\n{{inventada}} e {{Nao}} e {{contexto}}\n");
        let (s, avisos) = t.preencher(valor, &[], |_, v| v.into());
        assert_eq!(
            s,
            "Análise A-0001.\n{{inventada}} e {{Nao}} e {{id}} literal\n"
        );
        assert_eq!(
            avisos,
            vec![Aviso::VariavelDesconhecida {
                origem: Origem::Padrao,
                variavel: "inventada".into(),
                linha: 5,
            }]
        );
        assert!(
            avisos[0]
                .to_string()
                .contains("linha 5: variável desconhecida `{{inventada}}`")
        );
    }

    #[test]
    fn essencial_ausente_vai_para_o_fim() {
        let t = texto("Só {{id}}.\r\n");
        let (s, avisos) = t.preencher(valor, &["id", "contexto"], |n, v| format!("\n[{n}] {v}\n"));
        assert_eq!(s, "Só A-0001.\n\n[contexto] {{id}} literal\n");
        assert_eq!(
            avisos,
            vec![Aviso::VariavelAusente {
                origem: Origem::Padrao,
                variavel: "contexto".into()
            }]
        );
    }
}
