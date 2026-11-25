//! `registrar_diagrama`: registra um diagrama pelos elementos (#118).
//!
//! O agente não escreve Mermaid: informa os nós (estados ou passos) e as
//! arestas (transições ou caminhos), cada um com as evidências do código, e o
//! app gera o Mermaid na montagem do documento ([`cba_docs::mermaid`]). As
//! evidências são conferidas como as de `afirmar`.
//!
//! No fluxo do processo, cada passo pode ter a raia (o ator). No modelo de dados
//! conceitual (tipo `dados`, #120), cada nó é uma entidade (tabela de origem e
//! atributos de negócio) e cada aresta uma relação (cardinalidade e origem): a
//! chave estrangeira é conferida contra os scripts do banco e a entidade é ligada
//! ao termo do glossário ([`crate::modelo_dados`]).

use cba_core::analise::{Confianca, Fluxo};
use cba_core::diagramas::{
    Aresta, Atributo, Entidade, MAXIMO_DE_ARESTAS, MAXIMO_DE_ATRIBUTOS, MAXIMO_DE_NOS,
    Multiplicidade, No, NovoDiagrama, OrigemRelacao, PapelNo, Relacao, TAMANHO_DA_CHAVE,
    TipoDiagrama,
};
use cba_core::eventos::Mudanca;
use serde::Deserialize;
use serde_json::json;

use crate::comum;
use crate::documento::secoes_de_afirmacao;
use crate::evidencias::{self, EvidenciaCitada};
use crate::fontes;
use crate::modelo_dados;
use crate::registro::{ErroFerramenta, Ferramenta, ler_argumentos};

/// Evidências no máximo por elemento do diagrama.
pub const LIMITE_DE_EVIDENCIAS: u64 = 10;

#[derive(Debug, Deserialize)]
struct ArgsNo {
    chave: String,
    rotulo: String,
    #[serde(default)]
    papel: PapelNo,
    #[serde(default)]
    descricao: Option<String>,
    #[serde(default)]
    evidencias: Vec<EvidenciaCitada>,
    #[serde(default)]
    raia: Option<String>,
    #[serde(default)]
    tabela: Option<String>,
    #[serde(default)]
    atributos: Vec<Atributo>,
}

#[derive(Debug, Deserialize)]
struct ArgsCardinalidade {
    de: Multiplicidade,
    para: Multiplicidade,
}

#[derive(Debug, Deserialize)]
struct ArgsAresta {
    de: String,
    para: String,
    #[serde(default)]
    rotulo: Option<String>,
    #[serde(default)]
    descricao: Option<String>,
    #[serde(default)]
    condicao: Option<String>,
    #[serde(default)]
    efeitos: Option<String>,
    confianca: Confianca,
    #[serde(default)]
    evidencias: Vec<EvidenciaCitada>,
    #[serde(default)]
    cardinalidade: Option<ArgsCardinalidade>,
    #[serde(default)]
    origem: Option<OrigemRelacao>,
}

impl ArgsAresta {
    /// Relação do modelo de dados: cardinalidade e origem vêm juntas.
    fn relacao(&self, i: usize) -> Result<Option<Relacao>, ErroFerramenta> {
        match (&self.cardinalidade, self.origem) {
            (None, None) => Ok(None),
            (Some(c), Some(origem)) => Ok(Some(Relacao {
                de: c.de,
                para: c.para,
                origem,
            })),
            _ => Err(ErroFerramenta::Argumentos(format!(
                "arestas[{i}]: a relação do modelo de dados precisa de `cardinalidade` e de \
                 `origem` juntas"
            ))),
        }
    }
}

#[derive(Debug, Deserialize)]
struct Args {
    analise: String,
    secao: String,
    tipo: TipoDiagrama,
    titulo: String,
    nos: Vec<ArgsNo>,
    #[serde(default)]
    arestas: Vec<ArgsAresta>,
    #[serde(default)]
    substituir: Option<u32>,
}

fn conferidas(
    ambiente: &cba_core::ambiente::Ambiente,
    citadas: &[EvidenciaCitada],
    onde: &str,
) -> Result<Vec<cba_core::analise::Evidencia>, ErroFerramenta> {
    evidencias::validar_todas(&ambiente.escopo, citadas)
        .map_err(|m| ErroFerramenta::Recusa(format!("{onde}: {m}")))
}

/// `registrar_diagrama`.
pub fn registrar_diagrama() -> Ferramenta {
    let evidencias = json!({
        "type": "array",
        "maxItems": LIMITE_DE_EVIDENCIAS,
        "items": evidencias::schema_da_evidencia(),
        "description": "Trechos do código que sustentam o elemento."
    });
    let multiplicidade = json!({
        "type": "string",
        "enum": ["zero_ou_um", "um", "zero_ou_muitos", "um_ou_muitos"]
    });
    Ferramenta::nova(
        "registrar_diagrama",
        "Registrar diagrama",
        "Registra um diagrama na análise pelos elementos: os nós (estados de uma \
         entidade, passos de um processo ou entidades de negócio) e as arestas \
         (transições, caminhos ou relações), cada um com as evidências do código. Não \
         escreva Mermaid: o app gera o diagrama, a legenda e a tabela de elementos no \
         documento. Tipo `estados`: ciclo de vida de uma entidade (nós são os estados; o \
         rótulo da aresta é o evento que muda o estado). Tipo `fluxo`: passos de um \
         processo (nó `decisao` para as escolhas; o rótulo da aresta é a condição do \
         caminho; `raia` em cada passo diz o ator que o executa, e a figura agrupa os \
         passos por raia). Tipo `dados`: modelo de dados conceitual (cada nó é uma \
         entidade de negócio, com o nome do glossário como rótulo, a `tabela` de origem \
         e os `atributos` de negócio, sem colunas técnicas; cada aresta é uma relação \
         com `cardinalidade` e `origem`: `chave_estrangeira`, conferida contra os \
         scripts do banco, ou `join`, quando a relação só aparece num join ou numa \
         gravação do código, sempre `inferido`). Use rótulos de negócio, curtos, e uma \
         aresta por transição. Aresta `confirmado` exige evidência; `inferido` e \
         `a_confirmar` aparecem tracejadas. No ciclo de vida (`estados`), a chave do \
         nó é o valor que o código grava no campo de status (`ABERTA`), o papel marca \
         o estado inicial e os finais, e cada transição traz a `condicao` (guarda) e \
         os `efeitos` que o código mostra; o app aponta estados inalcançáveis, sem \
         saída e transições sem condição como achados, devolvidos na resposta. Para \
         corrigir um diagrama, registre de novo com `substituir` igual ao número dele.",
        json!({
            "type": "object",
            "properties": {
                "analise": comum::schema_da_analise(),
                "secao": {
                    "type": "string",
                    "minLength": 1,
                    "description": "Seção do documento em que o diagrama entra; as mesmas de `afirmar` (ex.: fluxo_principal, como_e_hoje, resposta)."
                },
                "tipo": {
                    "type": "string",
                    "enum": ["estados", "fluxo", "dados"],
                    "description": "estados: ciclo de vida de uma entidade. fluxo: passos de um processo, em raias por ator. dados: modelo de dados conceitual (entidades e relações)."
                },
                "titulo": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 200,
                    "description": "Legenda da figura (\"Ciclo de vida da fatura\")."
                },
                "nos": {
                    "type": "array",
                    "minItems": 1,
                    "maxItems": MAXIMO_DE_NOS,
                    "items": {
                        "type": "object",
                        "properties": {
                            "chave": {
                                "type": "string",
                                "minLength": 1,
                                "maxLength": TAMANHO_DA_CHAVE,
                                "description": "Identificador do nó neste diagrama, usado em `de` e `para` das arestas (ex.: o código do estado no banco, `ABERTA`)."
                            },
                            "rotulo": {
                                "type": "string",
                                "minLength": 1,
                                "maxLength": 80,
                                "description": "Nome de negócio do estado ou passo (\"Aberta\")."
                            },
                            "papel": {
                                "type": "string",
                                "enum": ["etapa", "inicio", "fim", "decisao"],
                                "description": "etapa (padrão), inicio (estado inicial ou começo do processo), fim (estado final), decisao (escolha no fluxo)."
                            },
                            "descricao": {
                                "type": "string",
                                "maxLength": 1000,
                                "description": "O que o estado ou passo significa, em linguagem de negócio."
                            },
                            "evidencias": evidencias,
                            "raia": {
                                "type": "string",
                                "maxLength": 60,
                                "description": "Só no tipo fluxo: o ator que executa o passo (\"Atendente\", \"Banco de dados\", \"Lote mensal\")."
                            },
                            "tabela": {
                                "type": "string",
                                "minLength": 1,
                                "maxLength": 128,
                                "description": "Só no tipo dados, e obrigatória nele: a tabela de origem da entidade (\"FATURA\"). O `rotulo` é o nome de negócio, o do glossário."
                            },
                            "atributos": {
                                "type": "array",
                                "maxItems": MAXIMO_DE_ATRIBUTOS,
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "nome": {"type": "string", "minLength": 1, "maxLength": 80, "description": "Nome de negócio (\"Data de emissão\")."},
                                        "coluna": {"type": "string", "minLength": 1, "maxLength": 128, "description": "Coluna de origem (\"DT_EMISSAO\")."}
                                    },
                                    "required": ["nome", "coluna"],
                                    "additionalProperties": false
                                },
                                "description": "Só no tipo dados: os atributos relevantes ao negócio, sem chaves e colunas técnicas."
                            }
                        },
                        "required": ["chave", "rotulo"],
                        "additionalProperties": false
                    }
                },
                "arestas": {
                    "type": "array",
                    "maxItems": MAXIMO_DE_ARESTAS,
                    "items": {
                        "type": "object",
                        "properties": {
                            "de": {"type": "string", "minLength": 1, "description": "Chave do nó de origem."},
                            "para": {"type": "string", "minLength": 1, "description": "Chave do nó de destino."},
                            "rotulo": {
                                "type": "string",
                                "maxLength": 80,
                                "description": "Evento da transição (\"cancelar\") ou condição do caminho (\"sim\")."
                            },
                            "descricao": {
                                "type": "string",
                                "maxLength": 1000,
                                "description": "Condição e efeito da transição, em linguagem de negócio."
                            },
                            "condicao": {
                                "type": "string",
                                "maxLength": 500,
                                "description": "Estados: guarda da transição, o que o código confere antes de mudar o estado (\"sem pagamento e motivo com 10 caracteres\"). Omita se o código não confere nada."
                            },
                            "efeitos": {
                                "type": "string",
                                "maxLength": 500,
                                "description": "Estados: o que mais a rotina grava ou dispara na transição (\"grava o log CANCELAMENTO\")."
                            },
                            "confianca": {
                                "type": "string",
                                "enum": ["confirmado", "inferido", "a_confirmar"],
                                "description": "confirmado: o código mostra a transição (exige evidência). inferido: dedução. a_confirmar: sem evidência suficiente."
                            },
                            "evidencias": evidencias,
                            "cardinalidade": {
                                "type": "object",
                                "properties": {
                                    "de": multiplicidade.clone(),
                                    "para": multiplicidade
                                },
                                "required": ["de", "para"],
                                "additionalProperties": false,
                                "description": "Só no tipo dados, e obrigatória nele. `de`: quantos registros da entidade de origem para cada registro da de destino; `para`: quantos da de destino para cada um da de origem. Cliente → Fatura: de \"um\", para \"zero_ou_muitos\"."
                            },
                            "origem": {
                                "type": "string",
                                "enum": ["chave_estrangeira", "join"],
                                "description": "Só no tipo dados, e obrigatória nele. chave_estrangeira: declarada nos scripts do banco (a evidência é a constraint). join: só aparece num join ou numa gravação do código, sem chave estrangeira; é relação inferida (`inferido`)."
                            }
                        },
                        "required": ["de", "para", "confianca"],
                        "additionalProperties": false
                    }
                },
                "substituir": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Número de um diagrama já registrado nesta análise, para trocá-lo por este."
                }
            },
            "required": ["analise", "secao", "tipo", "titulo", "nos"],
            "additionalProperties": false
        }),
        |ambiente, args| {
            let args: Args = ler_argumentos(args)?;
            let id = comum::id_da_analise(&args.analise)?;
            let analise = ambiente.analises.carregar(&id)?;
            if matches!(
                analise.fluxo,
                Fluxo::Historias | Fluxo::Cenarios | Fluxo::Prototipo
            ) {
                return Err(ErroFerramenta::Recusa(format!(
                    "a análise {id} é do fluxo `{}`, que não recebe diagramas; registre o \
                     diagrama numa análise que lê o código (pergunta, impacto, explicar)",
                    analise.fluxo.tipo_documento()
                )));
            }
            let secao = args.secao.trim().to_owned();
            let secoes = secoes_de_afirmacao(analise.fluxo);
            if !secoes.contains(&secao.as_str()) {
                return Err(ErroFerramenta::Recusa(format!(
                    "a seção `{secao}` não recebe diagramas no documento desta análise ({}). \
                     Use uma destas: {}.",
                    analise.fluxo.tipo_documento(),
                    secoes.join(", ")
                )));
            }
            let mut nos = Vec::new();
            for (i, n) in args.nos.into_iter().enumerate() {
                let onde = format!("nó {} (`{}`)", i + 1, n.chave.trim());
                let entidade = match n.tabela {
                    Some(tabela) => Some(Entidade {
                        tabela,
                        atributos: n.atributos,
                        termo: None,
                    }),
                    None if n.atributos.is_empty() => None,
                    None => {
                        return Err(ErroFerramenta::Argumentos(format!(
                            "{onde}: `atributos` são da entidade do tipo dados; informe a \
                             `tabela` de origem"
                        )));
                    }
                };
                nos.push(No {
                    evidencias: conferidas(ambiente, &n.evidencias, &onde)?,
                    chave: n.chave,
                    rotulo: n.rotulo,
                    papel: n.papel,
                    descricao: n.descricao,
                    raia: n.raia,
                    entidade,
                });
            }
            let mut arestas = Vec::new();
            for (i, a) in args.arestas.into_iter().enumerate() {
                let onde = format!("aresta {} (`{}` → `{}`)", i + 1, a.de.trim(), a.para.trim());
                let relacao = a.relacao(i)?;
                arestas.push(Aresta {
                    evidencias: conferidas(ambiente, &a.evidencias, &onde)?,
                    de: a.de,
                    para: a.para,
                    rotulo: a.rotulo,
                    descricao: a.descricao,
                    condicao: a.condicao,
                    efeitos: a.efeitos,
                    confianca: a.confianca,
                    relacao,
                });
            }
            let mut novo = NovoDiagrama {
                tipo: args.tipo,
                titulo: args.titulo,
                secao,
                nos,
                arestas,
                sessao: None,
            };
            let mut avisos = Vec::new();
            if novo.tipo == TipoDiagrama::Dados {
                let mapa = match fontes::mapa(ambiente) {
                    Ok(m) => Some(m.mapa),
                    Err(_) => {
                        avisos.push(
                            "o mapa do projeto não está disponível: tabelas, colunas e chaves \
                             estrangeiras não foram conferidas"
                                .to_owned(),
                        );
                        None
                    }
                };
                avisos.extend(modelo_dados::conferir_no_schema(mapa.as_deref(), &novo)?);
                let termos = modelo_dados::termos(ambiente, &analise);
                avisos.extend(modelo_dados::ligar_ao_glossario(&mut novo, &termos));
            }
            let substituir = args.substituir;
            let registrado = comum::alterar(ambiente, &id, Mudanca::Diagrama, |a, agora| {
                a.registrar_diagrama(novo, substituir, agora)
            })?;
            let numero = registrado.resultado;
            let diagrama = registrado.analise.diagrama(numero).ok_or_else(|| {
                ErroFerramenta::Interno(format!("o diagrama D-{numero} não foi gravado"))
            })?;
            let sem_evidencia: Vec<String> = diagrama
                .arestas
                .iter()
                .filter(|a| a.evidencias.is_empty())
                .map(|a| format!("{} → {}", a.de, a.para))
                .collect();
            let isolados: Vec<&str> = diagrama
                .nos
                .iter()
                .filter(|n| {
                    !diagrama
                        .arestas
                        .iter()
                        .any(|a| a.de == n.chave || a.para == n.chave)
                })
                .map(|n| n.chave.as_str())
                .collect();
            if !sem_evidencia.is_empty() {
                avisos.push(format!(
                    "aresta(s) sem evidência do código: {}; cite o trecho que faz cada \
                     transição, se houver",
                    sem_evidencia.join(", ")
                ));
            }
            if !isolados.is_empty() && diagrama.nos.len() > 1 {
                avisos.push(format!(
                    "nó(s) sem nenhuma aresta: {}; confira se falta transição ou se o nó \
                     nunca é alcançado no código",
                    isolados.join(", ")
                ));
            }
            Ok(comum::resposta(
                ambiente,
                &registrado,
                json!({
                    "diagrama": diagrama.rotulo(),
                    "numero": numero,
                    "secao": diagrama.secao,
                    "nos": diagrama.nos.len(),
                    "arestas": diagrama.arestas.len(),
                    "avisos": avisos,
                    "achados": diagrama
                        .achados()
                        .into_iter()
                        .map(|a| json!({"id": a.id, "texto": a.texto}))
                        .collect::<Vec<_>>(),
                    "entidades": diagrama
                        .nos
                        .iter()
                        .filter_map(|n| n.entidade.as_ref().map(|e| json!({
                            "chave": n.chave,
                            "tabela": e.tabela,
                            "termo_do_glossario": e.termo,
                        })))
                        .collect::<Vec<_>>(),
                }),
            ))
        },
    )
}
