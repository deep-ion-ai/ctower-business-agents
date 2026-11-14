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

/// Os fluxos, na ordem em que aparecem para o analista.
pub const FLUXOS: [Fluxo; 4] = [
    Fluxo::Perguntar,
    Fluxo::Explicar,
    Fluxo::Impacto,
    Fluxo::Reuniao,
];

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
pub(crate) const ESSENCIAIS: [&str; 3] = ["entrada", "secoes", "contexto"];

/// Nome do fluxo para o agente e para o analista.
pub fn nome_do_fluxo(fluxo: Fluxo) -> &'static str {
    match fluxo {
        Fluxo::Perguntar => "pergunta ao sistema",
        Fluxo::Explicar => "explicação de funcionalidade (documento negocial)",
        Fluxo::Impacto => "análise de impacto de uma demanda",
        Fluxo::Reuniao => "análise de reunião",
    }
}

/// Nome do arquivo do roteiro do fluxo, sem a extensão (o mesmo valor do `fluxo`
/// nas ferramentas).
pub fn nome_do_arquivo(fluxo: Fluxo) -> &'static str {
    match fluxo {
        Fluxo::Perguntar => "perguntar",
        Fluxo::Explicar => "explicar",
        Fluxo::Impacto => "impacto",
        Fluxo::Reuniao => "reuniao",
    }
}

/// Se o roteiro padrão do fluxo já foi revisado. Só os revisados são copiados
/// para a base. Os três primeiros foram revisados (#32): perguntar no M1, explicar
/// e impacto no M2; o de reunião, na #140. Um fluxo novo entra aqui só depois de
/// ter o roteiro revisado.
pub fn revisado(fluxo: Fluxo) -> bool {
    matches!(
        fluxo,
        Fluxo::Perguntar | Fluxo::Explicar | Fluxo::Impacto | Fluxo::Reuniao
    )
}

pub(crate) fn padrao(fluxo: Fluxo) -> Padrao {
    let texto = match fluxo {
        Fluxo::Perguntar => include_str!("../textos/roteiros/perguntar.md"),
        Fluxo::Explicar => include_str!("../textos/roteiros/explicar.md"),
        Fluxo::Impacto => include_str!("../textos/roteiros/impacto.md"),
        Fluxo::Reuniao => include_str!("../textos/roteiros/reuniao.md"),
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
            "uma afirmação por premissa do solicitante (\"Premissa do solicitante: ... Verificação: confere | não confere | confere em parte. ...\"), verificada no código, e uma afirmação final com a demanda como entendida, já com as correções"
        }
        (Fluxo::Impacto, "como_e_hoje") => {
            "como o projeto trata o assunto hoje: as regras atuais, com valores e condições"
        }
        (Fluxo::Impacto, "funcionalidades") => {
            "o que muda em cada funcionalidade ou regra, com o lugar no código; itens repetidos agrupados, com a contagem"
        }
        (Fluxo::Impacto, "dados") => {
            "tabelas, colunas e parâmetros afetados, inclusive dados já gravados no formato atual"
        }
        (Fluxo::Impacto, "integracoes") => {
            "lotes, arquivos e serviços que recebem ou enviam o que muda"
        }
        (Fluxo::Impacto, "documentos_e_telas") => {
            "relatórios, documentos emitidos e telas afetados; itens repetidos agrupados, com a contagem"
        }
        (Fluxo::Impacto, "riscos") => {
            "riscos, como dados antigos, integrações no formato atual, relatórios que agrupam pelos dados alterados, regras repetidas em outra camada e outros públicos afetados pela mesma mudança"
        }
        (Fluxo::Impacto, "abrangencia") => {
            "uma afirmação inferida que começa pela estimativa (\"Pequena:\", \"Média:\" ou \"Grande:\"), seguida da justificativa com a contagem de lugares afetados"
        }
        (Fluxo::Explicar, "objetivo") => {
            "para que a funcionalidade serve, para o negócio, e o que ficou dentro e fora do documento"
        }
        (Fluxo::Explicar, "atores_e_telas") => {
            "quem usa e por onde: telas, serviços, lotes e rotinas que iniciam a funcionalidade"
        }
        (Fluxo::Explicar, "fluxo_principal") => {
            "um passo do caminho normal por afirmação, na ordem, começando pelo número do passo"
        }
        (Fluxo::Explicar, "fluxos_alternativos") => {
            "caminhos que mudam por uma condição do negócio, com a condição"
        }
        (Fluxo::Explicar, "excecoes") => "recusas, erros e bloqueios, com a condição e a mensagem",
        (Fluxo::Explicar, "regras") => {
            "uma regra de negócio por afirmação, com valores e de onde vêm; a mesma regra em mais de uma camada vai numa afirmação só, com as evidências de cada uma e as divergências"
        }
        (Fluxo::Explicar, "dados") => "tabelas e campos lidos e gravados, e em que passo",
        (Fluxo::Explicar, "integracoes") => "projetos, lotes e serviços chamados ou que chamam",
        (Fluxo::Reuniao, "ligacao_com_o_codigo") => {
            "o que o código já faz, ou com que regra conflita, sobre uma necessidade ou regra citada na reunião (cite o item da ata, como N-3, e a evidência)"
        }
        _ => "",
    }
}

/// Seções que precisam de ao menos uma afirmação para a análise ser concluída: o
/// que o documento do fluxo não pode deixar de dizer.
pub fn secoes_obrigatorias(fluxo: Fluxo) -> &'static [&'static str] {
    match fluxo {
        Fluxo::Perguntar => &["resposta"],
        Fluxo::Impacto => &["demanda_como_entendida", "como_e_hoje", "abrangencia"],
        Fluxo::Explicar => &["objetivo", "fluxo_principal", "regras"],
        // Os itens da reunião ficam na ata; `concluir_analise` confere a ata.
        Fluxo::Reuniao => &[],
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

/// Os roteiros dos fluxos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Roteiros {
    por_fluxo: [Texto; 4],
}

fn indice(fluxo: Fluxo) -> usize {
    match fluxo {
        Fluxo::Perguntar => 0,
        Fluxo::Explicar => 1,
        Fluxo::Impacto => 2,
        Fluxo::Reuniao => 3,
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
                "secoes_obrigatorias" if secoes_obrigatorias(fluxo).is_empty() => {
                    "nenhuma".to_owned()
                }
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
    fn roteiro_de_impacto_verifica_premissas_agrupa_e_estima() {
        let (r, avisos) = Roteiros::padrao().montar(
            &analise(Fluxo::Impacto),
            "faturamento-exemplo/analises/A-0003.md",
            "Nada.\n",
        );
        assert!(avisos.is_empty());
        assert!(
            r.contains("Fluxo: análise de impacto de uma demanda. Documento na base:"),
            "{r}"
        );
        assert!(
            r.contains("## Demanda, como o solicitante escreveu\n\n> Como é calculada"),
            "{r}"
        );
        // Fato e suposição separados e cada afirmação verificada no código.
        assert!(r.contains("**Separe fato de suposição.**"), "{r}");
        assert!(
            r.contains("**Verifique cada afirmação no código, uma por uma.**"),
            "{r}"
        );
        assert!(r.contains("\"basta alterar o parâmetro\""), "{r}");
        // O lugar da premissa corrigida no documento.
        assert!(
            r.contains("\"Premissa do solicitante: <o que ele afirmou ou supôs>. Verificação: <confere | não confere | confere em parte>."),
            "{r}"
        );
        assert!(r.contains("\"Demanda como entendida: "), "{r}");
        assert!(r.contains("**Agrupe o que se repete.**"), "{r}");
        assert!(r.contains("**começa pela estimativa**"), "{r}");
        for nivel in ["pequena:", "média:", "grande:"] {
            assert!(r.contains(&format!("   - {nivel} ")), "{nivel}: {r}");
        }
        for secao in ["demanda_como_entendida", "como_e_hoje", "abrangencia"] {
            assert!(
                r.contains(&format!(
                    "`{secao}` ({}) (obrigatória)",
                    titulo_da_secao(Fluxo::Impacto, secao)
                )),
                "{secao}: {r}"
            );
        }
        assert!(r.contains("a premissa corrigida"), "{r}");
        assert!(r.ends_with("## Contexto da base\n\nNada.\n"), "{r}");
    }

    #[test]
    fn roteiro_de_explicar_segue_o_caminho_e_unifica_regras_entre_camadas() {
        let (r, avisos) = Roteiros::padrao().montar(
            &analise(Fluxo::Explicar),
            "faturamento-exemplo/analises/A-0003.md",
            "Nada.\n",
        );
        assert!(avisos.is_empty());
        assert!(
            r.contains("Fluxo: explicação de funcionalidade (documento negocial). Documento"),
            "{r}"
        );
        assert!(
            r.contains("## Funcionalidade pedida pelo analista\n\n> Como é calculada"),
            "{r}"
        );
        assert!(r.contains("**Ache os pontos de entrada.**"), "{r}");
        assert!(r.contains("**Siga o caminho do começo ao fim.**"), "{r}");
        assert!(r.contains("aponte se os valores divergem"), "{r}");
        assert!(r.contains("**Agrupe o que se repete.**"), "{r}");
        assert!(
            r.contains("seções obrigatórias (`objetivo`, `fluxo_principal`, `regras`)"),
            "{r}"
        );
        assert!(r.ends_with("## Contexto da base\n\nNada.\n"), "{r}");
    }

    #[test]
    fn roteiro_de_reuniao_le_tudo_cita_as_falas_e_cobre_as_secoes_da_ata() {
        let (r, avisos) = Roteiros::padrao().montar(
            &analise(Fluxo::Reuniao),
            "faturamento-exemplo/analises/A-0003.md",
            "Nada.\n",
        );
        assert!(avisos.is_empty(), "{avisos:?}");
        assert!(
            r.contains("Fluxo: análise de reunião. Documento da análise na base:"),
            "{r}"
        );
        assert!(
            r.contains("## Reunião\n\n> Como é calculada a multa?"),
            "{r}"
        );
        // Todas as ferramentas do fluxo.
        for ferramenta in [
            "ler_transcript",
            "propor_papel",
            "registrar_item_ata",
            "registrar_resumo_ata",
            "sugerir_demanda",
            "afirmar",
            "perguntar",
            "conhecimento",
            "referencias",
            "registrar_sugestao",
            "concluir_analise",
        ] {
            assert!(r.contains(&format!("`{ferramenta}`")), "{ferramenta}: {r}");
        }
        // Os parâmetros que o roteiro manda usar existem nas ferramentas.
        let registro = crate::registro_padrao();
        let schema = |nome: &str| {
            registro
                .obter(nome)
                .unwrap_or_else(|| panic!("{nome}"))
                .schema()
                .to_string()
        };
        for (ferramenta, parametro) in [
            ("ler_transcript", "cursor"),
            ("ler_transcript", "falante"),
            ("ler_transcript", "desde"),
            ("ler_transcript", "ate"),
            ("registrar_item_ata", "citacoes"),
            ("registrar_item_ata", "indice"),
            ("registrar_item_ata", "trecho"),
            ("registrar_item_ata", "responsavel"),
            ("registrar_item_ata", "prazo"),
            ("registrar_item_ata", "referencias"),
            ("registrar_resumo_ata", "objetivo"),
            ("registrar_resumo_ata", "resumo"),
        ] {
            assert!(r.contains(&format!("`{parametro}`")), "{parametro}: {r}");
            assert!(
                schema(ferramenta).contains(&format!("\"{parametro}\"")),
                "{ferramenta} sem {parametro}"
            );
        }
        assert!(r.contains("`proximo_cursor`"), "{r}");
        // Todas as seções da ata: os tipos de item, com o título da seção.
        let ata = cba_docs::modelo(cba_kb::TipoDocumento::Ata).unwrap();
        for tipo in cba_docs::TipoItem::TODOS {
            let titulo = ata.secao(tipo.secao()).unwrap().titulo;
            assert!(
                r.contains(&format!("   - `{}` ({titulo})", tipo.como_str())),
                "{tipo}: {r}"
            );
        }
        for secao in ["objetivo", "resumo"] {
            assert!(r.contains(secao), "{secao}: {r}");
        }
        // Lê até o fim, cita curto e literal, não inventa.
        assert!(
            r.contains("**Leia o transcript inteiro com `ler_transcript`**"),
            "{r}"
        );
        assert!(r.contains("até não haver próxima página"), "{r}");
        assert!(
            r.contains("**Citações curtas e literais; nunca invente.**"),
            "{r}"
        );
        assert!(r.contains("palavra por palavra"), "{r}");
        assert!(r.contains("O que ficou ambíguo vira `duvida`"), "{r}");
        // A ligação com o código é opcional e sem análise de impacto.
        assert!(r.contains("**Ligação com o código (opcional).**"), "{r}");
        assert!(r.contains("**Não faça a análise de impacto aqui**"), "{r}");
        assert!(r.contains("`ligacao_com_o_codigo`"), "{r}");
        // O contexto da demanda e a privacidade.
        assert!(r.contains("atas anteriores"), "{r}");
        assert!(r.contains("**Privacidade.**"), "{r}");
        assert!(r.contains("não copie falas para o chat"), "{r}");
        assert!(r.contains("não vai ao cliente"), "{r}");
        assert!(
            r.contains("Seções obrigatórias do documento da análise: nenhuma."),
            "{r}"
        );
        assert!(
            r.ends_with("## Contexto da demanda e da base\n\nNada.\n"),
            "{r}"
        );
    }

    #[test]
    fn roteiros_revisados_sao_copiados_e_o_da_base_sobrepoe() {
        let dir = tempfile::tempdir().unwrap();
        let base = Base::criar(dir.path()).unwrap();
        let criados = Roteiros::copiar_padrao(&base).unwrap();
        // Todos os roteiros já foram revisados; o de reunião, na #140.
        assert!(FLUXOS.into_iter().all(revisado));
        assert_eq!(
            criados,
            ["perguntar", "explicar", "impacto", "reuniao"]
                .map(|f| CaminhoRelativo::novo(&format!("_templates/roteiros/{f}.md")).unwrap())
        );
        // A cópia é o padrão embutido, byte a byte.
        for fluxo in FLUXOS.into_iter().filter(|f| revisado(*f)) {
            assert_eq!(
                base.ler_texto(&arquivo_do_roteiro(fluxo)).unwrap(),
                padrao(fluxo).texto
            );
        }
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
        // A cópia não editada dá o mesmo roteiro do padrão.
        assert_eq!(
            roteiros.origem(Fluxo::Impacto),
            &editavel::Origem::Base(arquivo_do_roteiro(Fluxo::Impacto))
        );
        assert_eq!(
            roteiros.montar(&analise(Fluxo::Impacto), "doc.md", "Nada.\n"),
            Roteiros::padrao().montar(&analise(Fluxo::Impacto), "doc.md", "Nada.\n")
        );
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
