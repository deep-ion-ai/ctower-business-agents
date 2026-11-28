//! Catálogo dos tipos de entregável (#460) e as predefinições de cada
//! metodologia.
//!
//! A análise investiga; o entregável é o artefato da metodologia do projeto,
//! derivado de uma análise validada ou do mapa de funcionalidades. Cada tipo do
//! [`CATALOGO`] declara:
//!
//! - o grupo ([`Grupo`]: Ágil, RUP ou Comuns);
//! - o ciclo de vida ([`CicloDeVida`]): o documento **vivo** pertence a uma
//!   funcionalidade do mapa, tem histórico de revisões e nunca é recriado; o
//!   **por demanda** nasce de uma demanda;
//! - a geração ([`Geracao`]): montado pelo app, sem IA, ou redigido pelo agente,
//!   preso ao esquema;
//! - o esquema dos campos ([`Campo`]), que o app valida
//!   ([`TipoEntregavel::validar_campos`]);
//! - o template padrão (`_templates/<template>.md`, embutido em `cba-docs`);
//! - de onde lê o conteúdo ([`Fonte`]).
//!
//! A configuração do projeto ([`cba_index::Entregaveis`]) guarda a metodologia e
//! os tipos habilitados. [`predefinicao`] dá os tipos de Ágil e RUP,
//! [`efetiva`] a configuração que vale para um cadastro (o legado quando o
//! projeto ainda não tem uma) e [`normalizar`] passa para Personalizado o que foi
//! mudado à mão.

use cba_index::{Entregaveis, Metodologia, TipoHabilitado};
use cba_kb::{Id, Serie};
use serde::Serialize;
use serde_json::Value;

/// Grupo do catálogo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Grupo {
    Agil,
    Rup,
    Comuns,
}

/// Ciclo de vida do entregável.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CicloDeVida {
    /// Pertence a uma funcionalidade do mapa: a v1.0 vem da engenharia reversa e
    /// cada demanda propõe a versão seguinte. Nunca é recriado.
    Vivo,
    /// Um por demanda.
    PorDemanda,
}

/// Quem monta o entregável.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Geracao {
    /// O app monta, de forma determinística, sem IA.
    App,
    /// O agente redige por um roteiro e registra pela ferramenta, preso ao
    /// esquema e à validação de rastreabilidade.
    Agente,
}

/// De onde o entregável lê o conteúdo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Fonte {
    /// A análise validada de origem.
    Analise,
    /// As regras de negócio da base.
    Regras,
    /// Os termos do glossário.
    Termos,
    /// O mapa de funcionalidades.
    Mapa,
    /// Os cenários de teste já registrados.
    Cenarios,
}

/// Tipo do valor de um campo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TipoCampo {
    /// Uma linha de texto.
    Texto,
    /// Texto de vários parágrafos.
    TextoLongo,
    /// Lista de textos.
    Lista,
    /// Lista de identificadores de regra (`RN-FAT-001`): a rastreabilidade.
    Regras,
}

/// Campo do esquema de um tipo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Campo {
    pub nome: &'static str,
    pub rotulo: &'static str,
    pub tipo: TipoCampo,
    /// Sem ele (ou vazio), o entregável não passa na validação.
    pub obrigatorio: bool,
}

/// Um tipo do catálogo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TipoEntregavel {
    /// Identificador estável (`caso-de-uso`), guardado no cadastro do projeto.
    pub id: &'static str,
    pub nome: &'static str,
    /// O que o tipo é, numa frase, para a tela de configuração.
    pub descricao: &'static str,
    pub grupo: Grupo,
    pub ciclo: CicloDeVida,
    pub geracao: Geracao,
    pub campos: &'static [Campo],
    /// Template padrão: `_templates/<template>.md` na base, ou o embutido.
    pub template: &'static str,
    pub fontes: &'static [Fonte],
    /// O app já gera este tipo. Os demais estão no catálogo e podem ser
    /// habilitados, mas a geração chega nas próximas versões.
    pub disponivel: bool,
}

/// Um campo com problema na validação do esquema.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, thiserror::Error)]
#[serde(rename_all = "camelCase")]
#[error("{campo}: {motivo}")]
pub struct ErroCampo {
    pub campo: String,
    pub motivo: String,
}

impl TipoEntregavel {
    pub fn campo(&self, nome: &str) -> Option<&'static Campo> {
        self.campos.iter().find(|c| c.nome == nome)
    }

    /// Confere os campos registrados contra o esquema do tipo: um objeto, sem
    /// campo desconhecido, cada valor do tipo declarado e os obrigatórios
    /// preenchidos. Devolve todos os problemas, não só o primeiro.
    pub fn validar_campos(&self, valor: &Value) -> Result<(), Vec<ErroCampo>> {
        let erro = |campo: &str, motivo: String| ErroCampo {
            campo: campo.to_owned(),
            motivo,
        };
        let Some(objeto) = valor.as_object() else {
            return Err(vec![erro("", "os campos vêm num objeto".into())]);
        };
        let mut erros = Vec::new();
        for nome in objeto.keys() {
            if self.campo(nome).is_none() {
                erros.push(erro(nome, format!("campo desconhecido em {}", self.nome)));
            }
        }
        for c in self.campos {
            match objeto.get(c.nome) {
                None | Some(Value::Null) => {
                    if c.obrigatorio {
                        erros.push(erro(c.nome, "obrigatório".into()));
                    }
                }
                Some(v) => {
                    if let Err(motivo) = valor_do_tipo(c, v) {
                        erros.push(erro(c.nome, motivo));
                    }
                }
            }
        }
        if erros.is_empty() { Ok(()) } else { Err(erros) }
    }
}

fn valor_do_tipo(c: &Campo, v: &Value) -> Result<(), String> {
    match c.tipo {
        TipoCampo::Texto | TipoCampo::TextoLongo => {
            let texto = v.as_str().ok_or("esperado um texto")?;
            if c.tipo == TipoCampo::Texto && texto.contains('\n') {
                return Err("esperada uma linha só".into());
            }
            if c.obrigatorio && texto.trim().is_empty() {
                return Err("obrigatório".into());
            }
        }
        TipoCampo::Lista | TipoCampo::Regras => {
            let itens = v.as_array().ok_or("esperada uma lista")?;
            if c.obrigatorio && itens.is_empty() {
                return Err("precisa de pelo menos um item".into());
            }
            for item in itens {
                let texto = item.as_str().ok_or("cada item é um texto")?;
                if texto.trim().is_empty() {
                    return Err("item vazio".into());
                }
                if c.tipo == TipoCampo::Regras
                    && !matches!(Id::analisar(texto), Ok(id) if matches!(id.serie(), Serie::Regra { .. }))
                {
                    return Err(format!(
                        "`{texto}` não é um identificador de regra (RN-<SIGLA>-NNN)"
                    ));
                }
            }
        }
    }
    Ok(())
}

const fn campo(nome: &'static str, rotulo: &'static str, tipo: TipoCampo) -> Campo {
    Campo {
        nome,
        rotulo,
        tipo,
        obrigatorio: true,
    }
}

const fn opcional(nome: &'static str, rotulo: &'static str, tipo: TipoCampo) -> Campo {
    Campo {
        nome,
        rotulo,
        tipo,
        obrigatorio: false,
    }
}

use Fonte as F;
use TipoCampo as C;

/// Regras de origem: todo entregável cita de onde vem cada item.
const REGRAS: Campo = campo("regras", "Regras de origem", C::Regras);
const FUNCIONALIDADE: Campo = campo("funcionalidade", "Funcionalidade do mapa", C::Texto);
const TITULO: Campo = campo("titulo", "Título", C::Texto);

/// Ids dos tipos que as predefinições e o legado usam.
pub const HISTORIA: &str = "historia-usuario";
pub const CRITERIOS_GHERKIN: &str = "criterios-gherkin";
pub const CENARIOS_FEATURE: &str = "cenarios-feature";
pub const EPICO: &str = "epico";
pub const RESUMO_REFINAMENTO: &str = "resumo-refinamento";
pub const DEFINITION_OF_READY: &str = "definition-of-ready";
pub const CASO_DE_USO: &str = "caso-de-uso";
pub const CATALOGO_REGRAS: &str = "catalogo-regras";
pub const ESPECIFICACAO_SUPLEMENTAR: &str = "especificacao-suplementar";
pub const CASOS_DE_TESTE: &str = "casos-de-teste";
pub const DOCUMENTO_VISAO: &str = "documento-visao";
pub const PROTOTIPO: &str = "prototipo";
pub const GLOSSARIO: &str = "glossario";
pub const MANUAL: &str = "manual";
/// O mesmo id que o mapa usa no vínculo do documento negocial.
pub const DOCUMENTO_NEGOCIAL: &str = cba_core::funcionalidades::TIPO_DOCUMENTO_NEGOCIAL;

/// Os tipos embutidos, na ordem da tela: Ágil, RUP e Comuns.
pub const CATALOGO: [TipoEntregavel; 15] = [
    // Ágil
    TipoEntregavel {
        id: HISTORIA,
        nome: "História de usuário",
        descricao: "Como, quero, para, com os critérios de aceite e as regras de origem.",
        grupo: Grupo::Agil,
        ciclo: CicloDeVida::PorDemanda,
        geracao: Geracao::Agente,
        campos: &[
            TITULO,
            campo("papel", "Como (papel)", C::Texto),
            campo("acao", "Quero (ação)", C::Texto),
            campo("beneficio", "Para (benefício)", C::Texto),
            campo("criterios", "Critérios de aceite", C::Lista),
            REGRAS,
        ],
        template: "historias",
        fontes: &[F::Analise, F::Regras, F::Termos],
        disponivel: true,
    },
    TipoEntregavel {
        id: CRITERIOS_GHERKIN,
        nome: "Critérios em Gherkin",
        descricao: "Dado, quando, então de uma história, montados dos cenários existentes.",
        grupo: Grupo::Agil,
        ciclo: CicloDeVida::PorDemanda,
        geracao: Geracao::App,
        campos: &[
            campo("historia", "História", C::Texto),
            campo("cenarios", "Cenários (Dado, Quando, Então)", C::Lista),
            REGRAS,
        ],
        template: "criterios-gherkin",
        fontes: &[F::Cenarios, F::Regras],
        disponivel: false,
    },
    TipoEntregavel {
        id: CENARIOS_FEATURE,
        nome: "Cenários .feature",
        descricao: "Arquivo .feature da funcionalidade, atualizado a cada demanda.",
        grupo: Grupo::Agil,
        ciclo: CicloDeVida::Vivo,
        geracao: Geracao::App,
        campos: &[
            FUNCIONALIDADE,
            campo("cenarios", "Cenários (Dado, Quando, Então)", C::Lista),
            REGRAS,
        ],
        template: "cenarios-feature",
        fontes: &[F::Cenarios, F::Regras, F::Mapa],
        disponivel: false,
    },
    TipoEntregavel {
        id: EPICO,
        nome: "Épico com quebra em histórias",
        descricao: "O objetivo da demanda e as histórias em que ela se divide.",
        grupo: Grupo::Agil,
        ciclo: CicloDeVida::PorDemanda,
        geracao: Geracao::Agente,
        campos: &[
            TITULO,
            campo("objetivo", "Objetivo", C::TextoLongo),
            campo("historias", "Histórias", C::Lista),
            opcional("fora_do_escopo", "Fora do escopo", C::Lista),
            REGRAS,
        ],
        template: "epico",
        fontes: &[F::Analise, F::Regras, F::Mapa],
        disponivel: false,
    },
    TipoEntregavel {
        id: RESUMO_REFINAMENTO,
        nome: "Resumo para refinamento",
        descricao: "O contexto, as dúvidas e os riscos para levar à reunião de refinamento.",
        grupo: Grupo::Agil,
        ciclo: CicloDeVida::PorDemanda,
        geracao: Geracao::Agente,
        campos: &[
            TITULO,
            campo("contexto", "Contexto", C::TextoLongo),
            opcional("duvidas", "Dúvidas em aberto", C::Lista),
            opcional("riscos", "Riscos", C::Lista),
            REGRAS,
        ],
        template: "resumo-refinamento",
        fontes: &[F::Analise, F::Regras],
        disponivel: false,
    },
    TipoEntregavel {
        id: DEFINITION_OF_READY,
        nome: "Definition of Ready",
        descricao: "A lista do que a história precisa ter antes de entrar na sprint.",
        grupo: Grupo::Agil,
        ciclo: CicloDeVida::PorDemanda,
        geracao: Geracao::App,
        campos: &[
            campo("historia", "História", C::Texto),
            campo("itens", "Itens atendidos", C::Lista),
            opcional("pendencias", "Pendências", C::Lista),
        ],
        template: "definition-of-ready",
        fontes: &[F::Analise, F::Regras, F::Cenarios],
        disponivel: false,
    },
    // RUP
    TipoEntregavel {
        id: CASO_DE_USO,
        nome: "Especificação de caso de uso",
        descricao: "Atores, pré-condições, fluxo principal, alternativos e exceções da funcionalidade.",
        grupo: Grupo::Rup,
        ciclo: CicloDeVida::Vivo,
        geracao: Geracao::Agente,
        campos: &[
            TITULO,
            campo("objetivo", "Objetivo", C::TextoLongo),
            campo("atores", "Atores", C::Lista),
            opcional("gatilho", "Gatilho", C::Texto),
            opcional("pre_condicoes", "Pré-condições", C::Lista),
            campo("fluxo_principal", "Fluxo principal", C::Lista),
            opcional("fluxos_alternativos", "Fluxos alternativos", C::Lista),
            opcional("excecoes", "Exceções", C::Lista),
            REGRAS,
        ],
        template: "caso-de-uso",
        fontes: &[F::Mapa, F::Analise, F::Regras, F::Termos],
        disponivel: false,
    },
    TipoEntregavel {
        id: CATALOGO_REGRAS,
        nome: "Catálogo de regras de negócio",
        descricao: "As regras da funcionalidade, com o enunciado e as evidências.",
        grupo: Grupo::Rup,
        ciclo: CicloDeVida::Vivo,
        geracao: Geracao::App,
        campos: &[FUNCIONALIDADE, REGRAS],
        template: "catalogo-regras",
        fontes: &[F::Regras, F::Mapa],
        disponivel: false,
    },
    TipoEntregavel {
        id: ESPECIFICACAO_SUPLEMENTAR,
        nome: "Especificação suplementar",
        descricao: "Requisitos não funcionais da funcionalidade: dados pessoais, auditoria, desempenho e acesso.",
        grupo: Grupo::Rup,
        ciclo: CicloDeVida::Vivo,
        geracao: Geracao::App,
        campos: &[
            FUNCIONALIDADE,
            campo("requisitos", "Requisitos não funcionais", C::Lista),
            REGRAS,
        ],
        template: "especificacao-suplementar",
        fontes: &[F::Analise, F::Regras, F::Mapa],
        disponivel: false,
    },
    TipoEntregavel {
        id: CASOS_DE_TESTE,
        nome: "Casos de teste",
        descricao: "Tabelas de decisão, valores-limite e cenários derivados das regras.",
        grupo: Grupo::Rup,
        ciclo: CicloDeVida::PorDemanda,
        geracao: Geracao::Agente,
        campos: &[
            campo("cenarios", "Cenários", C::Lista),
            opcional("tabelas_decisao", "Tabelas de decisão", C::Lista),
            opcional("valores_limite", "Valores-limite", C::Lista),
            REGRAS,
        ],
        template: "cenarios-teste",
        fontes: &[F::Analise, F::Regras, F::Cenarios],
        disponivel: true,
    },
    TipoEntregavel {
        id: DOCUMENTO_VISAO,
        nome: "Documento de visão",
        descricao: "O problema, as partes interessadas e as necessidades atendidas pela demanda.",
        grupo: Grupo::Rup,
        ciclo: CicloDeVida::PorDemanda,
        geracao: Geracao::Agente,
        campos: &[
            TITULO,
            campo("problema", "Problema", C::TextoLongo),
            campo("partes_interessadas", "Partes interessadas", C::Lista),
            campo("necessidades", "Necessidades", C::Lista),
            opcional("fora_do_escopo", "Fora do escopo", C::Lista),
            REGRAS,
        ],
        template: "documento-visao",
        fontes: &[F::Analise, F::Mapa, F::Regras],
        disponivel: false,
    },
    // Comuns
    TipoEntregavel {
        id: PROTOTIPO,
        nome: "Protótipo",
        descricao: "As telas afetadas, com os elementos ligados às regras e aos critérios.",
        grupo: Grupo::Comuns,
        ciclo: CicloDeVida::PorDemanda,
        geracao: Geracao::Agente,
        campos: &[campo("telas", "Telas", C::Lista), REGRAS],
        template: "prototipo",
        fontes: &[F::Analise, F::Regras],
        disponivel: true,
    },
    TipoEntregavel {
        id: GLOSSARIO,
        nome: "Glossário",
        descricao: "Os termos de negócio da funcionalidade e onde cada um vive no código.",
        grupo: Grupo::Comuns,
        ciclo: CicloDeVida::Vivo,
        geracao: Geracao::App,
        campos: &[FUNCIONALIDADE, campo("termos", "Termos", C::Lista)],
        template: "glossario",
        fontes: &[F::Termos, F::Mapa],
        disponivel: false,
    },
    TipoEntregavel {
        id: MANUAL,
        nome: "Manual do usuário",
        descricao: "O passo a passo da funcionalidade, com os prints de tela.",
        grupo: Grupo::Comuns,
        ciclo: CicloDeVida::Vivo,
        geracao: Geracao::Agente,
        campos: &[
            FUNCIONALIDADE,
            campo("objetivo", "Objetivo", C::TextoLongo),
            campo("passos", "Passo a passo", C::Lista),
            opcional("mensagens", "Mensagens e erros comuns", C::Lista),
            REGRAS,
        ],
        template: "manual",
        fontes: &[F::Mapa, F::Analise, F::Regras],
        disponivel: true,
    },
    TipoEntregavel {
        id: DOCUMENTO_NEGOCIAL,
        nome: "Documento negocial",
        descricao: "A explicação da funcionalidade em linguagem de negócio (o \"Explicar\").",
        grupo: Grupo::Comuns,
        ciclo: CicloDeVida::Vivo,
        geracao: Geracao::Agente,
        campos: &[
            FUNCIONALIDADE,
            campo("objetivo", "Objetivo", C::TextoLongo),
            campo("fluxo_principal", "Fluxo principal", C::Lista),
            opcional("excecoes", "Exceções", C::Lista),
            REGRAS,
        ],
        template: "funcionalidade",
        fontes: &[F::Mapa, F::Analise, F::Regras, F::Termos],
        disponivel: true,
    },
];

/// Tipo do catálogo pelo `id`.
pub fn tipo(id: &str) -> Option<&'static TipoEntregavel> {
    CATALOGO.iter().find(|t| t.id == id)
}

/// Tipos e tipo padrão de uma predefinição.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Predefinicao {
    pub metodologia: Metodologia,
    pub tipos: &'static [&'static str],
    pub padrao: &'static str,
}

/// As predefinições de Ágil e RUP. Personalizado não tem: fica com o que o
/// analista escolheu.
pub const PREDEFINICOES: [Predefinicao; 2] = [
    Predefinicao {
        metodologia: Metodologia::Agil,
        tipos: &[
            EPICO,
            HISTORIA,
            CRITERIOS_GHERKIN,
            CENARIOS_FEATURE,
            PROTOTIPO,
        ],
        padrao: EPICO,
    },
    Predefinicao {
        metodologia: Metodologia::Rup,
        tipos: &[
            CASO_DE_USO,
            CATALOGO_REGRAS,
            ESPECIFICACAO_SUPLEMENTAR,
            PROTOTIPO,
            GLOSSARIO,
        ],
        padrao: CASO_DE_USO,
    },
];

/// Os tipos que o app já gerava antes do catálogo, habilitados num projeto que
/// ainda não tem a configuração: histórias, cenários de teste, manual,
/// protótipo e documento negocial.
pub const LEGADO: Predefinicao = Predefinicao {
    metodologia: Metodologia::Personalizado,
    tipos: &[
        HISTORIA,
        CASOS_DE_TESTE,
        MANUAL,
        PROTOTIPO,
        DOCUMENTO_NEGOCIAL,
    ],
    padrao: HISTORIA,
};

impl Predefinicao {
    /// A configuração da predefinição, com o template e o destino padrão.
    pub fn entregaveis(&self) -> Entregaveis {
        Entregaveis {
            metodologia: self.metodologia,
            tipos: ordenados(self.tipos.iter().map(|t| TipoHabilitado::novo(t)).collect()),
            padrao: Some(self.padrao.to_owned()),
        }
    }

    /// Se a configuração tem exatamente os tipos e o padrão desta predefinição
    /// (template e destino não contam).
    pub fn confere(&self, e: &Entregaveis) -> bool {
        e.tipos.len() == self.tipos.len()
            && self.tipos.iter().all(|t| e.habilitado(t).is_some())
            && e.padrao.as_deref() == Some(self.padrao)
    }
}

/// Predefinição da metodologia, se ela tiver uma (Ágil e RUP).
pub fn predefinicao(metodologia: Metodologia) -> Option<&'static Predefinicao> {
    PREDEFINICOES.iter().find(|p| p.metodologia == metodologia)
}

/// A configuração que vale para o cadastro: a gravada ou, num projeto que
/// ainda não tem uma, o [`LEGADO`].
pub fn efetiva(gravada: Option<&Entregaveis>) -> Entregaveis {
    gravada.cloned().unwrap_or_else(|| LEGADO.entregaveis())
}

/// Escolhe a metodologia: aplica a predefinição dela, mantendo o template e o
/// destino dos tipos que continuam habilitados. Personalizado mantém os tipos
/// como estão.
pub fn aplicar(atual: &Entregaveis, metodologia: Metodologia) -> Entregaveis {
    let Some(p) = predefinicao(metodologia) else {
        return Entregaveis {
            metodologia,
            ..atual.clone()
        };
    };
    let mut nova = p.entregaveis();
    for t in &mut nova.tipos {
        if let Some(antes) = atual.habilitado(&t.tipo) {
            *t = antes.clone();
        }
    }
    nova
}

/// Põe os tipos na ordem do catálogo e passa a metodologia para Personalizado
/// quando os tipos ou o padrão não batem com a predefinição dela (um item
/// mudado à mão).
pub fn normalizar(mut e: Entregaveis) -> Entregaveis {
    e.tipos = ordenados(e.tipos);
    if let Some(p) = predefinicao(e.metodologia)
        && !p.confere(&e)
    {
        e.metodologia = Metodologia::Personalizado;
    }
    e
}

fn ordenados(mut tipos: Vec<TipoHabilitado>) -> Vec<TipoHabilitado> {
    let posicao = |t: &TipoHabilitado| CATALOGO.iter().position(|c| c.id == t.tipo);
    tipos.sort_by_key(|t| posicao(t).unwrap_or(usize::MAX));
    tipos
}

/// Configuração recusada ao salvar.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ErroEntregaveis {
    #[error("a metodologia “Orientado a agentes” ainda não está disponível")]
    MetodologiaIndisponivel,
    #[error("tipo de entregável desconhecido: {0}")]
    TipoDesconhecido(String),
    #[error("o tipo {0} aparece mais de uma vez")]
    TipoRepetido(String),
    #[error("habilite pelo menos um tipo de entregável")]
    SemTipos,
    #[error("escolha o tipo padrão entre os habilitados")]
    SemPadrao,
    #[error("o tipo padrão ({0}) não está habilitado")]
    PadraoDesabilitado(String),
    #[error(
        "template inválido para {tipo}: `{template}` (use o nome de um arquivo .md de _templates/, sem pastas)"
    )]
    TemplateInvalido { tipo: String, template: String },
}

/// Confere a configuração contra o catálogo: tipos conhecidos e sem
/// repetição, pelo menos um, o padrão entre eles e o template, quando há, só o
/// nome de um `.md` (a existência do arquivo é conferida por quem conhece a
/// base).
pub fn validar(e: &Entregaveis) -> Result<(), ErroEntregaveis> {
    if e.metodologia == Metodologia::OrientadoAgentes {
        return Err(ErroEntregaveis::MetodologiaIndisponivel);
    }
    if e.tipos.is_empty() {
        return Err(ErroEntregaveis::SemTipos);
    }
    for (i, t) in e.tipos.iter().enumerate() {
        if tipo(&t.tipo).is_none() {
            return Err(ErroEntregaveis::TipoDesconhecido(t.tipo.clone()));
        }
        if e.tipos[..i].iter().any(|a| a.tipo == t.tipo) {
            return Err(ErroEntregaveis::TipoRepetido(t.tipo.clone()));
        }
        if let Some(template) = &t.template
            && !nome_de_template(template)
        {
            return Err(ErroEntregaveis::TemplateInvalido {
                tipo: t.tipo.clone(),
                template: template.clone(),
            });
        }
    }
    match &e.padrao {
        None => Err(ErroEntregaveis::SemPadrao),
        Some(p) if e.habilitado(p).is_none() => Err(ErroEntregaveis::PadraoDesabilitado(p.clone())),
        Some(_) => Ok(()),
    }
}

/// Nome de arquivo `.md` simples, sem pasta nem caminho relativo.
fn nome_de_template(nome: &str) -> bool {
    nome.len() > ".md".len()
        && nome.len() <= 120
        && nome.ends_with(".md")
        && !nome.starts_with('.')
        && nome
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use cba_index::Destino;
    use cba_kb::TipoDocumento;
    use serde_json::json;

    use super::*;
    use crate::modelo;

    #[test]
    fn catalogo_tem_os_tipos_iniciais_por_grupo() {
        let do_grupo = |g| -> Vec<&str> {
            CATALOGO
                .iter()
                .filter(|t| t.grupo == g)
                .map(|t| t.id)
                .collect()
        };
        assert_eq!(
            do_grupo(Grupo::Agil),
            [
                HISTORIA,
                CRITERIOS_GHERKIN,
                CENARIOS_FEATURE,
                EPICO,
                RESUMO_REFINAMENTO,
                DEFINITION_OF_READY
            ]
        );
        assert_eq!(
            do_grupo(Grupo::Rup),
            [
                CASO_DE_USO,
                CATALOGO_REGRAS,
                ESPECIFICACAO_SUPLEMENTAR,
                CASOS_DE_TESTE,
                DOCUMENTO_VISAO
            ]
        );
        assert_eq!(
            do_grupo(Grupo::Comuns),
            [PROTOTIPO, GLOSSARIO, MANUAL, DOCUMENTO_NEGOCIAL]
        );
        let ids: HashSet<_> = CATALOGO.iter().map(|t| t.id).collect();
        assert_eq!(ids.len(), CATALOGO.len(), "ids únicos");
        let templates: HashSet<_> = CATALOGO.iter().map(|t| t.template).collect();
        assert_eq!(templates.len(), CATALOGO.len(), "um template por tipo");
    }

    #[test]
    fn ciclo_de_vida_e_geracao_por_tipo() {
        use CicloDeVida::{PorDemanda, Vivo};
        use Geracao::{Agente, App};
        let esperado = [
            (HISTORIA, PorDemanda, Agente),
            (CRITERIOS_GHERKIN, PorDemanda, App),
            (CENARIOS_FEATURE, Vivo, App),
            (EPICO, PorDemanda, Agente),
            (RESUMO_REFINAMENTO, PorDemanda, Agente),
            (DEFINITION_OF_READY, PorDemanda, App),
            (CASO_DE_USO, Vivo, Agente),
            (CATALOGO_REGRAS, Vivo, App),
            (ESPECIFICACAO_SUPLEMENTAR, Vivo, App),
            (CASOS_DE_TESTE, PorDemanda, Agente),
            (DOCUMENTO_VISAO, PorDemanda, Agente),
            (PROTOTIPO, PorDemanda, Agente),
            (GLOSSARIO, Vivo, App),
            (MANUAL, Vivo, Agente),
            (DOCUMENTO_NEGOCIAL, Vivo, Agente),
        ];
        assert_eq!(esperado.len(), CATALOGO.len());
        for (id, ciclo, geracao) in esperado {
            let t = tipo(id).unwrap();
            assert_eq!((t.ciclo, t.geracao), (ciclo, geracao), "{id}");
        }
    }

    #[test]
    fn esquema_de_cada_tipo() {
        for t in &CATALOGO {
            assert!(!t.fontes.is_empty(), "{}: sem fonte", t.id);
            assert!(
                t.campos.iter().any(|c| c.obrigatorio),
                "{}: sem campo obrigatório",
                t.id
            );
            let nomes: HashSet<_> = t.campos.iter().map(|c| c.nome).collect();
            assert_eq!(nomes.len(), t.campos.len(), "{}: campos repetidos", t.id);
            // O documento vivo pertence a uma funcionalidade do mapa.
            if t.ciclo == CicloDeVida::Vivo && t.id != CASO_DE_USO {
                assert!(
                    t.campo("funcionalidade").is_some(),
                    "{}: vivo sem funcionalidade",
                    t.id
                );
                assert!(t.fontes.contains(&Fonte::Mapa), "{}: vivo sem o mapa", t.id);
            }
            // O que o agente redige cita a regra de origem de cada item.
            if t.geracao == Geracao::Agente {
                let regras = t
                    .campo("regras")
                    .unwrap_or_else(|| panic!("{}: sem regras", t.id));
                assert_eq!(
                    (regras.tipo, regras.obrigatorio),
                    (TipoCampo::Regras, true),
                    "{}",
                    t.id
                );
            }
        }
    }

    #[test]
    fn os_tipos_disponiveis_tem_o_template_embutido() {
        for t in CATALOGO.iter().filter(|t| t.disponivel) {
            let doc = TipoDocumento::de_str(t.template).unwrap_or_else(|| {
                panic!("{}: template {} sem tipo de documento", t.id, t.template)
            });
            assert!(
                modelo::modelo(doc).is_some(),
                "{}: sem template embutido",
                t.id
            );
        }
        // O caso de uso já tem template, mas ainda nenhum fluxo o gera.
        assert!(modelo::modelo(TipoDocumento::CasoDeUso).is_some());
        assert!(!tipo(CASO_DE_USO).unwrap().disponivel);
    }

    #[test]
    fn validar_campos_aceita_o_esquema_e_aponta_cada_problema() {
        let h = tipo(HISTORIA).unwrap();
        let ok = json!({
            "titulo": "Cancelar fatura em atraso",
            "papel": "atendente",
            "acao": "cancelar a fatura",
            "beneficio": "corrigir a cobrança",
            "criterios": ["A multa é recalculada"],
            "regras": ["RN-FAT-001"],
        });
        assert_eq!(h.validar_campos(&ok), Ok(()));

        let ruim = json!({
            "titulo": "Duas\nlinhas",
            "papel": "",
            "criterios": [],
            "regras": ["A-0001", "RN-FAT-1"],
            "extra": 1,
        });
        let erros = h.validar_campos(&ruim).unwrap_err();
        let campos: Vec<_> = erros.iter().map(|e| e.campo.as_str()).collect();
        assert_eq!(
            campos,
            [
                "extra",
                "titulo",
                "papel",
                "acao",
                "beneficio",
                "criterios",
                "regras"
            ]
        );
        assert!(erros[6].motivo.contains("A-0001"), "{}", erros[6]);

        // Opcionais podem faltar ou vir nulos; o tipo do valor ainda é conferido.
        let uc = tipo(CASO_DE_USO).unwrap();
        let base = json!({
            "titulo": "Cancelar fatura",
            "objetivo": "Cancelar.\n\nCom multa.",
            "atores": ["Atendente"],
            "fluxo_principal": ["Abre a fatura", "Cancela"],
            "excecoes": null,
            "regras": ["RN-FAT-001"],
        });
        assert_eq!(uc.validar_campos(&base), Ok(()));
        let mut com_gatilho = base.clone();
        com_gatilho["gatilho"] = json!(["não é texto"]);
        assert_eq!(
            uc.validar_campos(&com_gatilho).unwrap_err()[0].campo,
            "gatilho"
        );
        assert!(uc.validar_campos(&json!([])).is_err());
    }

    #[test]
    fn rup_habilita_caso_de_uso_como_padrao_regras_suplementar_prototipo_e_glossario() {
        let rup = aplicar(&efetiva(None), Metodologia::Rup);
        assert_eq!(rup.metodologia, Metodologia::Rup);
        assert_eq!(
            rup.ids().collect::<Vec<_>>(),
            [
                CASO_DE_USO,
                CATALOGO_REGRAS,
                ESPECIFICACAO_SUPLEMENTAR,
                PROTOTIPO,
                GLOSSARIO
            ]
        );
        assert_eq!(rup.padrao.as_deref(), Some(CASO_DE_USO));
        assert_eq!(validar(&rup), Ok(()));
        assert_eq!(normalizar(rup.clone()), rup);
    }

    #[test]
    fn agil_habilita_o_epico_como_padrao() {
        let agil = aplicar(&efetiva(None), Metodologia::Agil);
        assert_eq!(
            agil.ids().collect::<Vec<_>>(),
            [
                HISTORIA,
                CRITERIOS_GHERKIN,
                CENARIOS_FEATURE,
                EPICO,
                PROTOTIPO
            ]
        );
        assert_eq!(agil.padrao.as_deref(), Some(EPICO));
        assert_eq!(validar(&agil), Ok(()));
    }

    #[test]
    fn aplicar_mantem_template_e_destino_dos_que_continuam() {
        let mut atual = efetiva(None);
        let p = atual
            .tipos
            .iter_mut()
            .find(|t| t.tipo == PROTOTIPO)
            .unwrap();
        p.template = Some("prototipo-empresa.md".into());
        p.destino = Destino::ComentarioRastreador;
        let rup = aplicar(&atual, Metodologia::Rup);
        let p = rup.habilitado(PROTOTIPO).unwrap();
        assert_eq!(p.template.as_deref(), Some("prototipo-empresa.md"));
        assert_eq!(p.destino, Destino::ComentarioRastreador);
        assert_eq!(
            rup.habilitado(GLOSSARIO).unwrap(),
            &TipoHabilitado::novo(GLOSSARIO)
        );
        // Template e destino não tiram da metodologia.
        assert_eq!(normalizar(rup).metodologia, Metodologia::Rup);

        // Personalizado fica com os tipos que estavam.
        let pers = aplicar(&atual, Metodologia::Personalizado);
        assert_eq!(pers.tipos, atual.tipos);
        assert_eq!(pers.metodologia, Metodologia::Personalizado);
    }

    #[test]
    fn mudar_um_item_a_mao_passa_para_personalizado() {
        let rup = aplicar(&efetiva(None), Metodologia::Rup);
        let mut sem_glossario = rup.clone();
        sem_glossario.tipos.retain(|t| t.tipo != GLOSSARIO);
        assert_eq!(
            normalizar(sem_glossario).metodologia,
            Metodologia::Personalizado
        );

        let mut com_manual = rup.clone();
        com_manual.tipos.insert(0, TipoHabilitado::novo(MANUAL));
        let n = normalizar(com_manual);
        assert_eq!(n.metodologia, Metodologia::Personalizado);
        assert_eq!(n.tipos.last().unwrap().tipo, MANUAL, "na ordem do catálogo");

        let mut outro_padrao = rup;
        outro_padrao.padrao = Some(PROTOTIPO.into());
        assert_eq!(
            normalizar(outro_padrao).metodologia,
            Metodologia::Personalizado
        );
    }

    #[test]
    fn projeto_antigo_abre_com_os_tipos_que_ja_usava() {
        let e = efetiva(None);
        assert_eq!(e.metodologia, Metodologia::Personalizado);
        assert_eq!(
            e.ids().collect::<Vec<_>>(),
            [
                HISTORIA,
                CASOS_DE_TESTE,
                PROTOTIPO,
                MANUAL,
                DOCUMENTO_NEGOCIAL
            ]
        );
        assert_eq!(e.padrao.as_deref(), Some(HISTORIA));
        assert!(
            e.ids().all(|id| tipo(id).unwrap().disponivel),
            "só tipos que o app já gera"
        );
        assert_eq!(validar(&e), Ok(()));
        let gravada = aplicar(&e, Metodologia::Agil);
        assert_eq!(efetiva(Some(&gravada)), gravada);
    }

    #[test]
    fn validar_recusa_o_que_nao_cabe() {
        let ok = efetiva(None);
        let com = |f: &dyn Fn(&mut Entregaveis)| {
            let mut e = ok.clone();
            f(&mut e);
            validar(&e)
        };
        assert_eq!(
            com(&|e| e.metodologia = Metodologia::OrientadoAgentes),
            Err(ErroEntregaveis::MetodologiaIndisponivel)
        );
        assert_eq!(com(&|e| e.tipos.clear()), Err(ErroEntregaveis::SemTipos));
        assert_eq!(
            com(&|e| e.tipos.push(TipoHabilitado::novo("openspec"))),
            Err(ErroEntregaveis::TipoDesconhecido("openspec".into()))
        );
        assert_eq!(
            com(&|e| e.tipos.push(TipoHabilitado::novo(MANUAL))),
            Err(ErroEntregaveis::TipoRepetido(MANUAL.into()))
        );
        assert_eq!(com(&|e| e.padrao = None), Err(ErroEntregaveis::SemPadrao));
        assert_eq!(
            com(&|e| e.padrao = Some(GLOSSARIO.into())),
            Err(ErroEntregaveis::PadraoDesabilitado(GLOSSARIO.into()))
        );
        for ruim in ["../segredo.md", "pasta/x.md", "x.txt", ".md", ".oculto.md"] {
            assert!(
                matches!(
                    com(&|e| e.tipos[0].template = Some(ruim.into())),
                    Err(ErroEntregaveis::TemplateInvalido { .. })
                ),
                "{ruim}"
            );
        }
        assert_eq!(
            com(&|e| e.tipos[0].template = Some("historias_empresa-v2.md".into())),
            Ok(())
        );
    }
}
