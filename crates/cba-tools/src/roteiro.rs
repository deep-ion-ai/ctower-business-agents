//! Roteiro do fluxo, que chega ao agente pela resposta de `iniciar_analise` e de
//! `retomar_analise` (spec, seção 8: "Roteiro do fluxo").
//!
//! Um roteiro por fluxo, em Markdown com variáveis (identificador, entrada,
//! contexto da base e seções do documento; [`VARIAVEIS`]). O padrão é embutido no
//! app (`crates/cba-tools/textos/roteiros/<fluxo>.md`); o arquivo
//! `<base>/_templates/roteiros/<fluxo>.md`, quando existe, é usado no lugar dele
//! ([`Roteiros::da_base`]), no mesmo padrão dos templates dos documentos
//! ([`crate::editavel`]).
//!
//! [`Roteiros::copiar_padrao`] grava na base só a cópia dos roteiros já revisados
//! ([`revisado`]): uma cópia de um roteiro provisório nunca seria atualizada e
//! esconderia a versão revisada que vier depois.

use std::fmt::Write as _;

use cba_core::analise::{Analise, Fluxo};
use cba_kb::{Base, CaminhoRelativo};

use crate::documento::secoes_de_afirmacao;
use crate::editavel::{self, Aviso, Padrao, Texto};

/// Os três fluxos, na ordem em que aparecem para o analista.
pub const FLUXOS: [Fluxo; 3] = [Fluxo::Perguntar, Fluxo::Explicar, Fluxo::Impacto];

/// Pasta dos roteiros dentro de `_templates/`.
pub const PASTA: &str = "roteiros";

/// Variáveis que um roteiro pode usar.
pub const VARIAVEIS: [&str; 8] = [
    "id",
    "titulo",
    "fluxo",
    "documento",
    "entrada",
    "secoes",
    "secoes_obrigatorias",
    "contexto",
];

/// Variáveis sem as quais o agente não conduz a análise: se o roteiro da base não
/// as usa, o valor vai para o fim, com aviso.
const ESSENCIAIS: [&str; 3] = ["entrada", "secoes", "contexto"];

/// Nome do fluxo para o agente e para o analista.
pub fn nome_do_fluxo(fluxo: Fluxo) -> &'static str {
    match fluxo {
        Fluxo::Perguntar => "pergunta ao sistema",
        Fluxo::Explicar => "explicação de funcionalidade (documento negocial)",
        Fluxo::Impacto => "análise de impacto de uma demanda",
    }
}

/// Nome do arquivo do roteiro do fluxo, sem a extensão (o mesmo valor do `fluxo`
/// nas ferramentas).
pub fn nome_do_arquivo(fluxo: Fluxo) -> &'static str {
    match fluxo {
        Fluxo::Perguntar => "perguntar",
        Fluxo::Explicar => "explicar",
        Fluxo::Impacto => "impacto",
    }
}

/// Se o roteiro padrão do fluxo já foi revisado. Só os revisados são copiados
/// para a base. Os de explicar e de impacto são revisados no M2 (#32).
pub fn revisado(fluxo: Fluxo) -> bool {
    matches!(fluxo, Fluxo::Perguntar)
}

fn padrao(fluxo: Fluxo) -> Padrao {
    let texto = match fluxo {
        Fluxo::Perguntar => include_str!("../textos/roteiros/perguntar.md"),
        Fluxo::Explicar => include_str!("../textos/roteiros/explicar.md"),
        Fluxo::Impacto => include_str!("../textos/roteiros/impacto.md"),
    };
    Padrao {
        pasta: PASTA,
        nome: nome_do_arquivo(fluxo),
        texto,
    }
}

/// `_templates/roteiros/<fluxo>.md`, relativo à raiz da base.
pub fn arquivo_do_roteiro(fluxo: Fluxo) -> CaminhoRelativo {
    padrao(fluxo).arquivo()
}

/// Título da seção no template padrão (`regras_encontradas` → "Regras encontradas").
pub fn titulo_da_secao(fluxo: Fluxo, marcador: &str) -> &'static str {
    cba_docs::modelo(fluxo.tipo_documento())
        .and_then(|m| m.secao(marcador))
        .map_or("", |s| s.titulo)
}

/// O que vai em cada seção que recebe afirmações.
fn dica_da_secao(fluxo: Fluxo, marcador: &str) -> &'static str {
    match (fluxo, marcador) {
        (Fluxo::Perguntar, "resposta") => {
            "a resposta direta à pergunta, em uma ou duas frases, em linguagem de negócio"
        }
        (Fluxo::Perguntar, "regras_encontradas") => {
            "uma regra de negócio por afirmação, com valores, condições e de onde vêm os valores (constante, parâmetro, tabela)"
        }
        (Fluxo::Perguntar, "exemplo") => {
            "um exemplo com números que aplica as regras (inferido, citando as evidências das regras usadas)"
        }
        (Fluxo::Impacto, "demanda_como_entendida") => {
            "o que a demanda pede de fato; separe o que o solicitante afirmou como fato do que ele supôs e corrija a premissa quando o código mostrar outra coisa"
        }
        (Fluxo::Impacto, "como_e_hoje") => "como o sistema trata o assunto hoje",
        (Fluxo::Impacto, "funcionalidades") => "o que muda em cada funcionalidade",
        (Fluxo::Impacto, "dados") => {
            "tabelas e campos afetados, inclusive dados já gravados no formato atual"
        }
        (Fluxo::Impacto, "integracoes") => "integrações que recebem ou enviam o que muda",
        (Fluxo::Impacto, "documentos_e_telas") => "relatórios, documentos e telas afetados",
        (Fluxo::Impacto, "riscos") => {
            "riscos, como dados antigos, integrações no formato atual e relatórios que agrupam pelos dados alterados"
        }
        (Fluxo::Impacto, "abrangencia") => "pequena, média ou grande, com a justificativa",
        (Fluxo::Explicar, "objetivo") => "para que a funcionalidade serve, para o negócio",
        (Fluxo::Explicar, "atores_e_telas") => "quem usa e por quais telas",
        (Fluxo::Explicar, "fluxo_principal") => "os passos do caminho normal",
        (Fluxo::Explicar, "fluxos_alternativos") => "caminhos alternativos",
        (Fluxo::Explicar, "excecoes") => "erros e situações de exceção",
        (Fluxo::Explicar, "regras") => "uma regra de negócio por afirmação",
        (Fluxo::Explicar, "dados") => "dados lidos e gravados",
        (Fluxo::Explicar, "integracoes") => "sistemas e serviços chamados ou que chamam",
        _ => "",
    }
}

/// Seções que precisam de ao menos uma afirmação para a análise ser concluída: o
/// que o documento do fluxo não pode deixar de dizer.
pub fn secoes_obrigatorias(fluxo: Fluxo) -> &'static [&'static str] {
    match fluxo {
        Fluxo::Perguntar => &["resposta"],
        Fluxo::Impacto => &["demanda_como_entendida", "como_e_hoje", "abrangencia"],
        Fluxo::Explicar => &["objetivo", "fluxo_principal"],
    }
}

/// Valor de `{{secoes}}`: uma linha por seção que recebe afirmações, recuada para
/// caber como sublista de um item numerado.
fn lista_de_secoes(fluxo: Fluxo) -> String {
    let mut t = String::new();
    for (i, secao) in secoes_de_afirmacao(fluxo).into_iter().enumerate() {
        if i > 0 {
            t.push('\n');
        }
        let obrigatoria = if secoes_obrigatorias(fluxo).contains(&secao) {
            " (obrigatória)"
        } else {
            ""
        };
        let _ = write!(
            t,
            "   - `{secao}` ({}){obrigatoria}: {}",
            titulo_da_secao(fluxo, secao),
            dica_da_secao(fluxo, secao)
        );
    }
    t
}

/// Valor de `{{entrada}}`: a entrada como citação Markdown.
fn citacao(entrada: &str) -> String {
    entrada
        .trim_end()
        .lines()
        .map(|l| {
            if l.trim().is_empty() {
                ">".to_owned()
            } else {
                format!("> {l}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Título da seção acrescentada no fim quando o roteiro não usa a variável.
fn titulo_ao_fim(variavel: &str) -> &'static str {
    match variavel {
        "entrada" => "Entrada do analista",
        "secoes" => "Seções do documento",
        _ => "Contexto da base",
    }
}

/// Os roteiros dos três fluxos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Roteiros {
    por_fluxo: [Texto; 3],
}

fn indice(fluxo: Fluxo) -> usize {
    match fluxo {
        Fluxo::Perguntar => 0,
        Fluxo::Explicar => 1,
        Fluxo::Impacto => 2,
    }
}

impl Roteiros {
    /// Os roteiros padrão embutidos no app.
    pub fn padrao() -> Self {
        Self {
            por_fluxo: FLUXOS.map(|f| padrao(f).embutido()),
        }
    }

    /// Roteiros da base: `<base>/_templates/roteiros/<fluxo>.md` quando existir, o
    /// padrão para os demais. Arquivo ilegível gera aviso e usa o padrão.
    pub fn da_base(base: &Base) -> (Self, Vec<Aviso>) {
        let mut avisos = Vec::new();
        let por_fluxo = FLUXOS.map(|f| {
            let (texto, aviso) = padrao(f).da_base(base);
            avisos.extend(aviso);
            texto
        });
        (Self { por_fluxo }, avisos)
    }

    /// Grava em `<base>/_templates/roteiros/` a cópia editável dos roteiros
    /// revisados que ainda não existem lá. Nunca sobrescreve o que o analista
    /// editou. Devolve os arquivos criados, relativos à raiz da base.
    pub fn copiar_padrao(base: &Base) -> cba_kb::Resultado<Vec<CaminhoRelativo>> {
        editavel::copiar(
            base,
            FLUXOS.into_iter().filter(|f| revisado(*f)).map(padrao),
        )
    }

    /// De onde vem o roteiro do fluxo.
    pub fn origem(&self, fluxo: Fluxo) -> &editavel::Origem {
        &self.por_fluxo[indice(fluxo)].origem
    }

    /// O roteiro da análise, em Markdown, com as variáveis preenchidas.
    /// `documento` é o caminho do documento relativo à base; `contexto` é o
    /// contexto da base já formatado. Devolve também os avisos (variável
    /// desconhecida ou essencial ausente no roteiro da base).
    pub fn montar(
        &self,
        analise: &Analise,
        documento: &str,
        contexto: &str,
    ) -> (String, Vec<Aviso>) {
        let fluxo = analise.fluxo;
        let valor = |nome: &str| -> Option<String> {
            Some(match nome {
                "id" => analise.id.to_string(),
                "titulo" => analise.titulo.clone(),
                "fluxo" => nome_do_fluxo(fluxo).to_owned(),
                "documento" => documento.to_owned(),
                "entrada" => citacao(&analise.entrada),
                "secoes" => lista_de_secoes(fluxo),
                "secoes_obrigatorias" => secoes_obrigatorias(fluxo)
                    .iter()
                    .map(|s| format!("`{s}`"))
                    .collect::<Vec<_>>()
                    .join(", "),
                "contexto" => contexto.to_owned(),
                _ => return None,
            })
        };
        self.por_fluxo[indice(fluxo)].preencher(valor, &ESSENCIAIS, |variavel, v| {
            format!("\n\n## {}\n\n{}\n", titulo_ao_fim(variavel), v.trim_end())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cba_kb::Serie;

    fn analise(fluxo: Fluxo) -> Analise {
        Analise::nova(
            Serie::Analise.id(3),
            "faturamento-exemplo",
            fluxo,
            "Multa por atraso",
            "Como é calculada a multa?\n\nE os juros?",
            1,
        )
    }

    #[test]
    fn todas_as_secoes_tem_titulo_e_dica_e_as_obrigatorias_existem() {
        for fluxo in FLUXOS {
            let secoes = secoes_de_afirmacao(fluxo);
            for s in &secoes {
                assert!(!titulo_da_secao(fluxo, s).is_empty(), "{fluxo:?} {s}");
                assert!(!dica_da_secao(fluxo, s).is_empty(), "{fluxo:?} {s}");
            }
            for s in secoes_obrigatorias(fluxo) {
                assert!(secoes.contains(s), "{fluxo:?} {s}");
            }
        }
    }

    #[test]
    fn roteiros_padrao_usam_todas_as_variaveis_sem_avisos() {
        let roteiros = Roteiros::padrao();
        for fluxo in FLUXOS {
            let texto = padrao(fluxo).texto;
            let (_, corpo) = texto.split_once("-->").expect("comentário inicial");
            for v in VARIAVEIS {
                assert!(
                    corpo.contains(&format!("{{{{{v}}}}}")),
                    "{fluxo:?} sem {{{{{v}}}}}"
                );
            }
            let (r, avisos) = roteiros.montar(&analise(fluxo), "doc.md", "Nada.\n");
            assert!(avisos.is_empty(), "{fluxo:?}: {avisos:?}");
            assert!(!r.contains("{{"), "{fluxo:?}: {r}");
            assert!(!r.contains("<!--"), "{fluxo:?}: {r}");
            for s in secoes_de_afirmacao(fluxo) {
                assert!(r.contains(&format!("`{s}`")), "{fluxo:?} sem {s}: {r}");
            }
        }
    }

    #[test]
    fn roteiro_de_perguntar_traz_identificador_entrada_secoes_e_contexto() {
        let (r, avisos) = Roteiros::padrao().montar(
            &analise(Fluxo::Perguntar),
            "faturamento-exemplo/analises/A-0003.md",
            "Nada.\n",
        );
        assert!(avisos.is_empty());
        assert!(
            r.starts_with("# Roteiro da análise A-0003: Multa por atraso\n"),
            "{r}"
        );
        assert!(
            r.contains("Fluxo: pergunta ao sistema. Documento na base: `faturamento-exemplo/analises/A-0003.md`"),
            "{r}"
        );
        assert!(r.contains("analise: \"A-0003\""), "{r}");
        assert!(
            r.contains("> Como é calculada a multa?\n>\n> E os juros?"),
            "{r}"
        );
        assert!(
            r.contains("   - `resposta` (Resposta) (obrigatória): a resposta direta"),
            "{r}"
        );
        assert!(
            r.contains("   - `regras_encontradas` (Regras encontradas): "),
            "{r}"
        );
        assert!(r.contains("   - `exemplo` (Exemplo): "), "{r}");
        assert!(r.contains("seções obrigatórias (`resposta`)"), "{r}");
        for ferramenta in [
            "afirmar",
            "perguntar",
            "propor_termo",
            "registrar_sugestao",
            "concluir_analise",
        ] {
            assert!(r.contains(&format!("`{ferramenta}`")), "{ferramenta}: {r}");
        }
        assert!(r.ends_with("## Contexto da base\n\nNada.\n"), "{r}");
    }

    #[test]
    fn so_o_roteiro_revisado_e_copiado_e_o_da_base_sobrepoe() {
        let dir = tempfile::tempdir().unwrap();
        let base = Base::criar(dir.path()).unwrap();
        let criados = Roteiros::copiar_padrao(&base).unwrap();
        assert_eq!(
            criados,
            vec![CaminhoRelativo::novo("_templates/roteiros/perguntar.md").unwrap()]
        );
        // Não sobrescreve o que o analista editou.
        let editado = "<!-- meu -->\nRoteiro da casa para {{id}} ({{fluxo}}).\n{{inventada}}\n";
        base.gravar_texto(&arquivo_do_roteiro(Fluxo::Perguntar), editado)
            .unwrap();
        assert!(Roteiros::copiar_padrao(&base).unwrap().is_empty());

        let (roteiros, avisos) = Roteiros::da_base(&base);
        assert!(avisos.is_empty(), "{avisos:?}");
        assert_eq!(
            roteiros.origem(Fluxo::Perguntar),
            &editavel::Origem::Base(arquivo_do_roteiro(Fluxo::Perguntar))
        );
        assert_eq!(roteiros.origem(Fluxo::Impacto), &editavel::Origem::Padrao);
        let (r, avisos) = roteiros.montar(&analise(Fluxo::Perguntar), "doc.md", "Nada.\n");
        assert!(
            r.starts_with("Roteiro da casa para A-0003 (pergunta ao sistema).\n{{inventada}}\n"),
            "{r}"
        );
        // O essencial que o roteiro da base esqueceu vai para o fim.
        assert!(
            r.contains("\n## Entrada do analista\n\n> Como é calculada"),
            "{r}"
        );
        assert!(
            r.contains("\n## Seções do documento\n\n   - `resposta`"),
            "{r}"
        );
        assert!(r.ends_with("\n## Contexto da base\n\nNada.\n"), "{r}");
        let mensagens: Vec<String> = avisos.iter().map(ToString::to_string).collect();
        assert_eq!(
            mensagens,
            [
                "_templates/roteiros/perguntar.md, linha 3: variável desconhecida `{{inventada}}`",
                "_templates/roteiros/perguntar.md não usa a variável `{{entrada}}`; o valor foi para o fim",
                "_templates/roteiros/perguntar.md não usa a variável `{{secoes}}`; o valor foi para o fim",
                "_templates/roteiros/perguntar.md não usa a variável `{{contexto}}`; o valor foi para o fim",
            ]
        );
    }

    #[test]
    fn roteiro_ilegivel_usa_o_padrao_com_aviso() {
        let dir = tempfile::tempdir().unwrap();
        let base = Base::criar(dir.path()).unwrap();
        // Bytes que não são UTF-8.
        let arquivo = arquivo_do_roteiro(Fluxo::Impacto);
        std::fs::create_dir_all(base.caminho(&arquivo).parent().unwrap()).unwrap();
        std::fs::write(base.caminho(&arquivo), [0xff, 0xfe, 0x00]).unwrap();
        let (roteiros, avisos) = Roteiros::da_base(&base);
        assert_eq!(roteiros.origem(Fluxo::Impacto), &editavel::Origem::Padrao);
        assert_eq!(avisos.len(), 1, "{avisos:?}");
        assert!(
            avisos[0]
                .to_string()
                .starts_with("_templates/roteiros/impacto.md: arquivo ilegível"),
            "{}",
            avisos[0]
        );
    }
}
