//! Prompts prontos da tela da análise (spec, seção 8): aprofundar, resumir para o
//! solicitante, gerar histórias e critérios, gerar cenários de teste, conferir
//! evidências e retomar; na
//! análise de reunião (#140), resumir para o cliente e listar dúvidas para enviar.
//!
//! Cada prompt vale para alguns fluxos ([`Fluxos`]): os de resumo para o
//! solicitante e de gerar histórias são das análises do código; os de resumo para
//! o cliente e de dúvidas, da análise de reunião. A análise de histórias (#115)
//! não tem afirmações: fica com gerar cenários e retomar; a de cenários (#117), só
//! com retomar. "Gerar histórias e critérios" abre o fluxo `historias` a partir da
//! análise (substituiu o "Gerar critérios de aceite", que só escrevia no chat), e
//! "Gerar cenários de teste" abre o fluxo `cenarios`.
//!
//! Cada um é um pedido curto com variáveis da análise (`{{id}}`, `{{titulo}}`,
//! `{{fluxo}}`, `{{documento}}`), que o app envia ao cliente de IA como se o
//! analista o tivesse digitado. O padrão é embutido no app
//! (`crates/cba-tools/textos/prompts/<id>.md`); o arquivo
//! `<base>/_templates/prompts/<id>.md`, quando existe, é usado no lugar dele, no
//! mesmo padrão dos roteiros e dos templates ([`crate::editavel`]). O rótulo do
//! botão é do app.
//!
//! O texto vai numa linha só: as quebras de linha do arquivo viram espaços, porque
//! no terminal uma quebra de linha enviaria o pedido pela metade.
//!
//! - [`prontos`]: os prompts de uma análise, a partir da base;
//! - [`da_analise`]: o mesmo, a partir do diretório de dados do app, do
//!   identificador do projeto e da análise (o comando `prompts_prontos` da
//!   janela).

use std::sync::Arc;

use cba_core::analise::{Analise, EstadoAnalise, Fluxo};
use cba_core::analises::Analises;
use cba_core::config::Config;
use cba_core::dados::DiretorioDados;
use cba_core::eventos::SemNotificacao;
use cba_index::Cadastro;
use cba_kb::{Base, CaminhoRelativo, Id, Serie};
use serde::{Deserialize, Serialize};

use crate::editavel::{self, Aviso, Padrao, Texto};
use crate::roteiro::nome_do_fluxo;

/// Pasta dos prompts prontos dentro de `_templates/`.
pub const PASTA: &str = "prompts";

/// Variáveis que um prompt pronto pode usar.
pub const VARIAVEIS: [&str; 4] = ["id", "titulo", "fluxo", "documento"];

/// Variável sem a qual o pedido perde o sentido: se o prompt da base não a usa, o
/// valor vai para o fim, com aviso.
pub(crate) const ESSENCIAIS: [&str; 1] = ["id"];

/// Um prompt pronto, como a tela da análise mostra e envia.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptPronto {
    /// Identificador estável (`aprofundar`), também o nome do arquivo na base.
    pub id: String,
    /// Rótulo do botão.
    pub rotulo: String,
    /// Dica do botão: o que o pedido faz.
    pub descricao: String,
    /// O pedido, com as variáveis preenchidas, numa linha só.
    pub texto: String,
}

/// Fluxos em que um prompt pronto aparece.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Fluxos {
    /// Todos.
    Todos,
    /// Os que têm afirmações com evidências: todos menos os de histórias e de
    /// cenários.
    ComAfirmacoes,
    /// As análises do código: perguntar, explicar e impacto.
    DoCodigo,
    /// As análises do código e a de histórias: as origens dos cenários de teste.
    OrigemDeCenarios,
    /// Só a análise de reunião.
    Reuniao,
}

impl Fluxos {
    fn inclui(self, fluxo: Fluxo) -> bool {
        match self {
            Self::Todos => true,
            Self::ComAfirmacoes => !matches!(fluxo, Fluxo::Historias | Fluxo::Cenarios),
            Self::DoCodigo => matches!(fluxo, Fluxo::Perguntar | Fluxo::Explicar | Fluxo::Impacto),
            Self::OrigemDeCenarios => matches!(
                fluxo,
                Fluxo::Perguntar | Fluxo::Explicar | Fluxo::Impacto | Fluxo::Historias
            ),
            Self::Reuniao => fluxo == Fluxo::Reuniao,
        }
    }
}

/// Definição de um prompt pronto.
pub(crate) struct Definicao {
    pub(crate) id: &'static str,
    pub(crate) rotulo: &'static str,
    descricao: &'static str,
    texto: &'static str,
    /// Pede ao agente que registre na análise; fica de fora numa análise aprovada,
    /// que não aceita registros.
    registra: bool,
    /// Fluxos em que o prompt aparece.
    fluxos: Fluxos,
}

pub(crate) const DEFINICOES: [Definicao; 8] = [
    Definicao {
        id: "aprofundar",
        rotulo: "Aprofundar",
        descricao: "Pede ao agente que busque evidência para o que você marcou para aprofundar e para o que ainda é inferido ou a confirmar.",
        texto: include_str!("../textos/prompts/aprofundar.md"),
        registra: true,
        fluxos: Fluxos::ComAfirmacoes,
    },
    Definicao {
        id: "resumir",
        rotulo: "Resumir para o solicitante",
        descricao: "Pede um resumo curto, em linguagem de negócio, pronto para enviar ao solicitante.",
        texto: include_str!("../textos/prompts/resumir.md"),
        registra: false,
        fluxos: Fluxos::DoCodigo,
    },
    Definicao {
        id: "resumir-para-o-cliente",
        rotulo: "Resumir para o cliente",
        descricao: "Pede um resumo da reunião pronto para enviar ao cliente, sem referências internas (citações, identificadores, evidências de código): a ata não vai ao cliente.",
        texto: include_str!("../textos/prompts/resumir-para-o-cliente.md"),
        registra: false,
        fluxos: Fluxos::Reuniao,
    },
    Definicao {
        id: "listar-duvidas",
        rotulo: "Listar dúvidas para enviar",
        descricao: "Pede a lista das dúvidas em aberto da reunião, uma pergunta por item, pronta para enviar ao cliente.",
        texto: include_str!("../textos/prompts/listar-duvidas.md"),
        registra: false,
        fluxos: Fluxos::Reuniao,
    },
    Definicao {
        id: "gerar-historias",
        rotulo: "Gerar histórias e critérios",
        descricao: "Abre uma análise de histórias a partir desta: histórias com critérios de aceite (Dado / Quando / Então) com valores, cada critério ligado às afirmações e às regras, para revisar e aprovar no app.",
        texto: include_str!("../textos/prompts/gerar-historias.md"),
        // Registra numa análise nova, não nesta: vale também na aprovada.
        registra: false,
        fluxos: Fluxos::DoCodigo,
    },
    Definicao {
        id: "gerar-cenarios",
        rotulo: "Gerar cenários de teste",
        descricao: "Abre uma análise de cenários a partir desta: tabela de decisão, partição de equivalência e valores-limite lidos do código, cada cenário ligado à regra e à evidência do limite, com exportação em CSV.",
        texto: include_str!("../textos/prompts/gerar-cenarios.md"),
        // Registra numa análise nova, não nesta: vale também na aprovada.
        registra: false,
        fluxos: Fluxos::OrigemDeCenarios,
    },
    Definicao {
        id: "conferir-evidencias",
        rotulo: "Conferir evidências",
        descricao: "Pede ao agente que releia no código atual cada trecho citado e diga o que confere e o que não se sustenta.",
        texto: include_str!("../textos/prompts/conferir-evidencias.md"),
        registra: true,
        fluxos: Fluxos::ComAfirmacoes,
    },
    Definicao {
        id: "retomar",
        rotulo: "Retomar",
        descricao: "Continua a análise numa conversa nova, a partir de tudo o que já foi registrado.",
        texto: include_str!("../textos/prompts/retomar.md"),
        registra: false,
        fluxos: Fluxos::Todos,
    },
];

pub(crate) fn padrao(d: &Definicao) -> Padrao {
    Padrao {
        pasta: PASTA,
        nome: d.id,
        texto: d.texto,
    }
}

/// `_templates/prompts/<id>.md`, relativo à raiz da base. `None` se o prompt não
/// existir.
pub fn arquivo_do_prompt(id: &str) -> Option<CaminhoRelativo> {
    DEFINICOES
        .iter()
        .find(|d| d.id == id)
        .map(|d| padrao(d).arquivo())
}

/// Os prompts prontos, com o texto da base ou o padrão.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prompts {
    textos: Vec<Texto>,
}

impl Prompts {
    /// Os prompts padrão embutidos no app.
    pub fn padrao() -> Self {
        Self {
            textos: DEFINICOES.iter().map(|d| padrao(d).embutido()).collect(),
        }
    }

    /// Prompts da base: `<base>/_templates/prompts/<id>.md` quando existir, o
    /// padrão para os demais. Arquivo ilegível gera aviso e usa o padrão.
    pub fn da_base(base: &Base) -> (Self, Vec<Aviso>) {
        let mut avisos = Vec::new();
        let textos = DEFINICOES
            .iter()
            .map(|d| {
                let (texto, aviso) = padrao(d).da_base(base);
                avisos.extend(aviso);
                texto
            })
            .collect();
        (Self { textos }, avisos)
    }

    /// Grava em `<base>/_templates/prompts/` a cópia editável dos prompts que
    /// ainda não existem lá. Nunca sobrescreve o que o analista editou. Devolve os
    /// arquivos criados, relativos à raiz da base.
    pub fn copiar_padrao(base: &Base) -> cba_kb::Resultado<Vec<CaminhoRelativo>> {
        editavel::copiar(base, DEFINICOES.iter().map(padrao))
    }

    /// Os prompts da análise, com as variáveis preenchidas: só os do fluxo dela.
    /// `documento` é o caminho do documento relativo à base. Numa análise
    /// aprovada, só os que não registram nada (os resumos, gerar histórias e cenários,
    /// as dúvidas e retomar).
    pub fn para(&self, analise: &Analise, documento: &str) -> (Vec<PromptPronto>, Vec<Aviso>) {
        let valor = |nome: &str| -> Option<String> {
            Some(match nome {
                "id" => analise.id.to_string(),
                "titulo" => analise.titulo.clone(),
                "fluxo" => nome_do_fluxo(analise.fluxo).to_owned(),
                "documento" => documento.to_owned(),
                _ => return None,
            })
        };
        let aprovada = analise.estado == EstadoAnalise::Aprovada;
        let mut avisos = Vec::new();
        let mut prontos = Vec::new();
        for (d, texto) in DEFINICOES.iter().zip(&self.textos) {
            if !d.fluxos.inclui(analise.fluxo) || (aprovada && d.registra) {
                continue;
            }
            let (texto, mut a) =
                texto.preencher(valor, &ESSENCIAIS, |_, v| format!(" (análise {v})"));
            avisos.append(&mut a);
            prontos.push(PromptPronto {
                id: d.id.to_owned(),
                rotulo: d.rotulo.to_owned(),
                descricao: d.descricao.to_owned(),
                texto: numa_linha(&texto),
            });
        }
        (prontos, avisos)
    }
}

/// O texto numa linha só, com os espaços repetidos reduzidos a um.
fn numa_linha(texto: &str) -> String {
    texto.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Os prompts prontos da análise, com os textos da base. Os avisos (prompt da base
/// ilegível, variável desconhecida) vão para o log, sem o conteúdo; o prompt sai
/// mesmo assim.
pub fn prontos(base: &Base, kb: &cba_kb::Projeto, analise: &Analise) -> Vec<PromptPronto> {
    let (prompts, mut avisos) = Prompts::da_base(base);
    let documento = kb.na_base(&analise.documento).to_string();
    let (prontos, mut a) = prompts.para(analise, &documento);
    avisos.append(&mut a);
    for aviso in &avisos {
        tracing::warn!(%aviso, "prompt pronto da base com problema");
    }
    prontos
}

/// Por que os prompts de uma análise não puderam ser montados. As mensagens vão
/// para a tela do analista.
#[derive(Debug, thiserror::Error)]
pub enum ErroPrompts {
    #[error("o projeto `{0}` não está cadastrado")]
    ProjetoDesconhecido(String),
    #[error("`{0}` não é o identificador de uma análise (A-NNNN)")]
    IdInvalido(String),
    #[error("a pasta da base de conhecimento ainda não foi escolhida; conclua o primeiro uso")]
    BaseNaoConfigurada,
    #[error("não foi possível ler a configuração do app: {0}")]
    Config(std::io::Error),
    #[error("não foi possível ler o cadastro dos projetos: {0}")]
    Cadastro(cba_index::Erro),
    #[error("a base de conhecimento não está acessível: {0}")]
    Base(cba_kb::Erro),
    #[error("{0}")]
    Analise(cba_core::analises::Erro),
}

/// Os prompts prontos da análise `analise` do projeto cadastrado `projeto`, com os
/// dados do app em `dados`. Só lê: nada é criado na base.
pub fn da_analise(
    dados: &DiretorioDados,
    projeto: &str,
    analise: &str,
) -> Result<Vec<PromptPronto>, ErroPrompts> {
    let id = Id::analisar(analise.trim())
        .ok()
        .filter(|id| id.serie() == &Serie::Analise)
        .ok_or_else(|| ErroPrompts::IdInvalido(analise.to_owned()))?;
    let cadastrado = Cadastro::em(dados.projetos())
        .obter(projeto)
        .map_err(ErroPrompts::Cadastro)?
        .ok_or_else(|| ErroPrompts::ProjetoDesconhecido(projeto.to_owned()))?;
    let config = Config::ler(dados).map_err(ErroPrompts::Config)?;
    let raiz = config.base().ok_or(ErroPrompts::BaseNaoConfigurada)?;
    let base = Base::abrir(raiz).map_err(ErroPrompts::Base)?;
    let kb = base.projeto(&cadastrado.id).map_err(ErroPrompts::Base)?;
    let analise = Analises::new(kb.clone(), Arc::new(SemNotificacao))
        .carregar(&id)
        .map_err(ErroPrompts::Analise)?;
    Ok(prontos(&base, &kb, &analise))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cba_core::analise::Fluxo;

    fn analise() -> Analise {
        Analise::nova(
            Serie::Analise.id(7),
            "faturamento-exemplo",
            Fluxo::Perguntar,
            "Multa por atraso",
            "Como é calculada a multa?",
            1,
        )
    }

    #[test]
    fn padroes_tem_ids_unicos_sem_avisos_e_numa_linha() {
        let (prontos, avisos) =
            Prompts::padrao().para(&analise(), "faturamento-exemplo/analises/A-0007.md");
        assert!(avisos.is_empty(), "{avisos:?}");
        let ids: Vec<&str> = prontos.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "aprofundar",
                "resumir",
                "gerar-historias",
                "gerar-cenarios",
                "conferir-evidencias",
                "retomar"
            ]
        );
        for p in &prontos {
            assert!(p.texto.contains("A-0007"), "{}: {}", p.id, p.texto);
            assert!(!p.texto.contains("{{"), "{}: {}", p.id, p.texto);
            assert!(!p.texto.contains('\n'), "{}: {}", p.id, p.texto);
            assert!(!p.rotulo.is_empty() && !p.descricao.is_empty());
            assert!(arquivo_do_prompt(&p.id).is_some());
        }
        assert_eq!(prontos[5].texto, "Retome a análise A-0007 pelo ctower.");
        assert!(
            prontos[1]
                .texto
                .starts_with("Resuma a análise A-0007 (Multa por atraso) para o solicitante"),
            "{}",
            prontos[1].texto
        );
    }

    #[test]
    fn analise_aprovada_so_tem_os_que_nao_registram() {
        let mut a = analise();
        a.estado = EstadoAnalise::Aprovada;
        let (prontos, _) = Prompts::padrao().para(&a, "x.md");
        let ids: Vec<&str> = prontos.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(
            ids,
            ["resumir", "gerar-historias", "gerar-cenarios", "retomar"]
        );

        let mut r = reuniao();
        r.estado = EstadoAnalise::Aprovada;
        let (prontos, _) = Prompts::padrao().para(&r, "x.md");
        let ids: Vec<&str> = prontos.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["resumir-para-o-cliente", "listar-duvidas", "retomar"]);
    }

    fn reuniao() -> Analise {
        Analise::nova(
            Serie::Analise.id(8),
            "faturamento-exemplo",
            Fluxo::Reuniao,
            "Reunião R-0001 · Trava de cancelamento de fatura",
            "Reunião R-0001 da demanda D-0001.",
            1,
        )
    }

    #[test]
    fn cada_fluxo_tem_os_seus_prompts() {
        let ids = |a: &Analise| -> Vec<String> {
            let (prontos, avisos) = Prompts::padrao().para(a, "x.md");
            assert!(avisos.is_empty(), "{avisos:?}");
            prontos.into_iter().map(|p| p.id).collect()
        };
        for fluxo in [Fluxo::Perguntar, Fluxo::Explicar, Fluxo::Impacto] {
            let mut a = analise();
            a.fluxo = fluxo;
            assert_eq!(
                ids(&a),
                [
                    "aprofundar",
                    "resumir",
                    "gerar-historias",
                    "gerar-cenarios",
                    "conferir-evidencias",
                    "retomar"
                ],
                "{fluxo:?}"
            );
        }
        // Na reunião, os resumos e os critérios das análises do código dão lugar
        // ao resumo para o cliente e à lista de dúvidas.
        assert_eq!(
            ids(&reuniao()),
            [
                "aprofundar",
                "resumir-para-o-cliente",
                "listar-duvidas",
                "conferir-evidencias",
                "retomar"
            ]
        );
        // A análise de histórias não tem afirmações: gera cenários e retoma; a de
        // cenários só retoma.
        let mut historias = analise();
        historias.fluxo = Fluxo::Historias;
        assert_eq!(ids(&historias), ["gerar-cenarios", "retomar"]);
        historias.fluxo = Fluxo::Cenarios;
        assert_eq!(ids(&historias), ["retomar"]);
    }

    #[test]
    fn gerar_cenarios_abre_o_fluxo_a_partir_da_analise() {
        let (prontos, _) = Prompts::padrao().para(&analise(), "x.md");
        let p = prontos.iter().find(|p| p.id == "gerar-cenarios").unwrap();
        assert!(
            p.texto.contains("fluxo \"cenarios\" e origem \"A-0007\""),
            "{}",
            p.texto
        );
        assert!(p.texto.contains("registrar_cenario"), "{}", p.texto);
    }

    #[test]
    fn gerar_historias_abre_o_fluxo_a_partir_da_analise() {
        let (prontos, _) = Prompts::padrao().para(&analise(), "x.md");
        let p = prontos.iter().find(|p| p.id == "gerar-historias").unwrap();
        assert!(
            p.texto.contains("fluxo \"historias\" e origem \"A-0007\""),
            "{}",
            p.texto
        );
        assert!(p.texto.contains("registrar_historia"), "{}", p.texto);
    }

    /// O texto do prompt, sem o comentário inicial, numa linha só.
    fn pedido(id: &str) -> String {
        let (prontos, _) = Prompts::padrao().para(&reuniao(), "x.md");
        prontos
            .into_iter()
            .find(|p| p.id == id)
            .unwrap_or_else(|| panic!("{id}"))
            .texto
    }

    #[test]
    fn resumo_para_o_cliente_omite_as_referencias_internas() {
        let t = pedido("resumir-para-o-cliente");
        assert!(
            t.starts_with("Escreva um resumo da reunião da análise A-0008 para enviar ao cliente."),
            "{t}"
        );
        assert!(t.contains("`retomar_analise`"), "{t}");
        assert!(t.contains("A ata é interna e não vai ao cliente"), "{t}");
        // Pede explicitamente para omitir citações, identificadores e evidências.
        assert!(
            t.contains("não leva nenhuma referência interna: omita"),
            "{t}"
        );
        for omitir in [
            "as citações e os trechos de falas",
            "os identificadores (dos itens da ata, como D-1, N-3 ou PP-2, das reuniões, das demandas",
            "as evidências de código",
            "os nomes de arquivos, rotinas e tabelas",
            "os papéis internos dos participantes",
        ] {
            assert!(t.contains(omitir), "{omitir}: {t}");
        }
        assert!(t.contains("releia o texto e tire o que restar"), "{t}");
        assert!(t.contains("Não registre nada"), "{t}");
    }

    #[test]
    fn duvidas_para_enviar_viram_lista_sem_referencias_internas() {
        let t = pedido("listar-duvidas");
        assert!(
            t.starts_with("Liste as dúvidas em aberto da reunião da análise A-0008"),
            "{t}"
        );
        assert!(t.contains("`retomar_analise`"), "{t}");
        assert!(t.contains("as dúvidas em aberto da ata"), "{t}");
        assert!(
            t.contains("pendências ainda sem responsável ou sem prazo"),
            "{t}"
        );
        assert!(t.contains("lista numerada, uma pergunta por item"), "{t}");
        for omitir in [
            "as citações e os trechos de falas",
            "os identificadores",
            "as evidências de código",
        ] {
            assert!(t.contains(omitir), "{omitir}: {t}");
        }
        assert!(t.contains("Não registre nada"), "{t}");
    }

    #[test]
    fn prompt_da_base_sobrepoe_e_variaveis_sao_substituidas() {
        let dir = tempfile::tempdir().unwrap();
        let base = Base::criar(dir.path()).unwrap();
        let criados = Prompts::copiar_padrao(&base).unwrap();
        assert_eq!(criados.len(), DEFINICOES.len());
        assert!(base.raiz().join("_templates/prompts/retomar.md").is_file());

        let arquivo = arquivo_do_prompt("resumir").unwrap();
        base.gravar_texto(
            &arquivo,
            "<!-- da casa -->\nResumo de {{titulo}}\n  ({{fluxo}}),\nem `{{documento}}`. {{x}}\n",
        )
        .unwrap();
        // A cópia não sobrescreve o que foi editado.
        assert!(Prompts::copiar_padrao(&base).unwrap().is_empty());

        let (prompts, avisos) = Prompts::da_base(&base);
        assert!(avisos.is_empty());
        let (prontos, avisos) = prompts.para(&analise(), "f/analises/A-0007.md");
        let resumir = prontos.iter().find(|p| p.id == "resumir").unwrap();
        assert_eq!(
            resumir.texto,
            "Resumo de Multa por atraso (pergunta ao sistema), em `f/analises/A-0007.md`. {{x}} (análise A-0007)"
        );
        assert_eq!(resumir.rotulo, "Resumir para o solicitante");
        let mensagens: Vec<String> = avisos.iter().map(ToString::to_string).collect();
        assert_eq!(
            mensagens,
            [
                "_templates/prompts/resumir.md, linha 4: variável desconhecida `{{x}}`",
                "_templates/prompts/resumir.md não usa a variável `{{id}}`; o valor foi para o fim",
            ]
        );
        // Os demais continuam com o padrão.
        let retomar = prontos.iter().find(|p| p.id == "retomar").unwrap();
        assert_eq!(retomar.texto, "Retome a análise A-0007 pelo ctower.");
    }

    #[test]
    fn json_em_camel_case() {
        let (prontos, _) = Prompts::padrao().para(&analise(), "x.md");
        let v = serde_json::to_value(&prontos[0]).unwrap();
        let mut chaves: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
        chaves.sort_unstable();
        assert_eq!(chaves, ["descricao", "id", "rotulo", "texto"]);
    }
}
