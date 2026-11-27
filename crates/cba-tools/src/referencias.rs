//! `referencias(identificador)`: onde uma rotina, tabela, campo ou componente é
//! usado (ou, com `sentido: "o_que_usa"`, o que ele usa), a partir do mapa
//! estrutural (`cba_index::Mapa::usos` e `referencias_de`).
//!
//! As referências do mapa são resolvidas pelo nome, sem compilar o código: um nome
//! ambíguo pode ligar a mais de um símbolo, e uma menção numa string (SQL embutido,
//! chamada por nome) conta como uso. A resposta diz o tipo de cada uso para o agente
//! decidir o que ler.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use cba_index::{Achado, Mapa, Uso};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::fontes;
use crate::paginacao::{ArgsPagina, Item, Pagina, com_paginacao, nome};
use crate::registro::{ErroFerramenta, Ferramenta, Saida, ler_argumentos};

mod arvore;

use arvore::{Arvore, LIMITE_DE_NOS, LIMITE_DE_NOS_MAXIMO, PROFUNDIDADE_MAXIMA};

/// Definições listadas no cabeçalho.
const DEFINICOES: usize = 10;

/// Tipos de referência, como aparecem no schema e na resposta.
const TIPOS: [&str; 13] = [
    "chamada",
    "leitura",
    "escrita",
    "chave_estrangeira",
    "tipo",
    "heranca",
    "implementacao",
    "importacao",
    "inclusao",
    "instanciacao",
    "disparo",
    "uso",
    "mencao",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Sentido {
    #[default]
    QuemUsa,
    OQueUsa,
}

#[derive(Debug, Deserialize)]
struct Args {
    identificador: String,
    #[serde(default)]
    sentido: Sentido,
    #[serde(default)]
    tipos: Vec<String>,
    #[serde(default)]
    profundidade: Option<u8>,
    #[serde(default)]
    limite_de_nos: Option<usize>,
    #[serde(flatten)]
    pagina: ArgsPagina,
}

/// `referencias(identificador)`: onde um identificador é usado.
pub fn referencias() -> Ferramenta {
    Ferramenta::nova(
        "referencias",
        "Referências de um identificador",
        "Mostra onde uma rotina, procedure, função, tabela, coluna, classe ou tela é \
         usada no projeto: arquivo, linha, tipo de uso (chamada, leitura, escrita, \
         chave estrangeira, menção numa string...) e a rotina de onde parte; e onde o \
         identificador é definido. Com `sentido: \"o_que_usa\"`, mostra o que ele chama \
         e lê. Use para achar quem calcula ou grava um valor antes de explorar o código \
         às cegas; confirme lendo o trecho antes de afirmar. Resposta paginada.",
        com_paginacao(json!({
            "type": "object",
            "properties": {
                "identificador": {
                    "type": "string",
                    "minLength": 1,
                    "description": "Nome simples ou qualificado, sem diferença de maiúsculas (\"FN_CALCULAR_MULTA\", \"PKG_COBRANCA.FN_CALCULAR_MULTA\", \"FATURA\", \"FATURA.STATUS\")."
                },
                "sentido": {
                    "type": "string",
                    "enum": ["quem_usa", "o_que_usa"],
                    "description": "quem_usa (padrão): onde o identificador é usado. o_que_usa: o que ele usa."
                },
                "tipos": {
                    "type": "array",
                    "items": { "type": "string", "enum": TIPOS },
                    "description": "Só estes tipos de uso (\"escrita\" para quem grava numa tabela, por exemplo). Sem o campo: todos. Com `profundidade`, filtra só o primeiro nível."
                },
                "profundidade": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": PROFUNDIDADE_MAXIMA,
                    "description": "Só com quem_usa. 1 (padrão): os usos diretos. 2 ou 3: também a árvore de quem chama, subindo pela rotina de cada uso até os pontos de entrada (telas, rotas, lotes), inclusive chamadas internas e edições de outros registros. Use para listar todos os caminhos que gravam um estado: parta do campo (com tipos [\"escrita\"]) ou do método de escrita."
                },
                "limite_de_nos": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": LIMITE_DE_NOS_MAXIMO,
                    "description": format!("Nós da árvore (padrão {LIMITE_DE_NOS}). Passando dele, os níveis mais distantes são cortados, com aviso.")
                }
            },
            "required": ["identificador"],
            "additionalProperties": false
        })),
        |ambiente, args| {
            let args: Args = ler_argumentos(args)?;
            let identificador = args.identificador.trim().to_owned();
            let carregado = fontes::mapa(ambiente)?;
            let mapa = &carregado.mapa;

            let definicoes = mapa.buscar(&identificador);
            let usos: Vec<Uso<'_>> = match args.sentido {
                Sentido::QuemUsa => mapa.usos(&identificador),
                Sentido::OQueUsa => o_que_usa(mapa, &definicoes),
            };
            let tipos: BTreeSet<&str> = args.tipos.iter().map(String::as_str).collect();
            let usos: Vec<Uso<'_>> = usos
                .into_iter()
                .filter(|u| tipos.is_empty() || tipos.contains(nome(&u.referencia.tipo).as_str()))
                .collect();
            let itens: Vec<Item> = usos
                .iter()
                .map(|u| item(mapa, u, args.sentido, &identificador))
                .collect();
            let profundidade = args.profundidade.unwrap_or(1).clamp(1, PROFUNDIDADE_MAXIMA);
            if profundidade > 1 && args.sentido == Sentido::OQueUsa {
                return Err(ErroFerramenta::Recusa(
                    "`profundidade` sobe pelos chamadores e só vale com `sentido: \"quem_usa\"`. \
                     Para descer pelo que a rotina usa, chame `referencias` de novo com cada \
                     rotina usada."
                        .into(),
                ));
            }
            let arvore = (profundidade > 1).then(|| {
                let limite = args
                    .limite_de_nos
                    .unwrap_or(LIMITE_DE_NOS)
                    .clamp(1, LIMITE_DE_NOS_MAXIMO);
                Arvore::montar(mapa, &usos, profundidade, limite)
            });
            let pagina = Pagina::da_lista(itens, args.pagina);

            let mut texto = String::new();
            if definicoes.is_empty() {
                let _ = writeln!(
                    texto,
                    "`{identificador}` não é definido no código mapeado (pode ser externo, \
                     de uma biblioteca ou de um banco sem scripts)."
                );
            } else {
                let _ = writeln!(texto, "`{identificador}` é definido em:");
                for d in definicoes.iter().take(DEFINICOES) {
                    let _ = writeln!(texto, "- {}", definicao(d));
                }
                if definicoes.len() > DEFINICOES {
                    let _ = writeln!(
                        texto,
                        "- e mais {}; use o nome qualificado para restringir.",
                        definicoes.len() - DEFINICOES
                    );
                }
            }
            let rotulo = match args.sentido {
                Sentido::QuemUsa => "Usos",
                Sentido::OQueUsa => "O que usa",
            };
            if pagina.total == 0 {
                let _ = writeln!(
                    texto,
                    "\n{rotulo}: nenhum no mapa{}. O mapa é estrutural e pode não ver \
                     chamadas dinâmicas; confirme com as suas ferramentas de busca.",
                    if tipos.is_empty() {
                        String::new()
                    } else {
                        " com os tipos pedidos".to_owned()
                    }
                );
            } else {
                let _ = writeln!(texto, "\n{rotulo} ({}):", pagina.total);
                texto.push_str(&pagina.linhas());
                let _ = writeln!(texto, "\n{}", pagina.rodape());
            }
            if let Some(a) = arvore.as_ref().filter(|_| pagina.inicio == 0) {
                texto.push_str(&a.texto(&identificador));
            }
            let _ = writeln!(texto, "{}", carregado.nota());

            Ok(Saida {
                texto,
                dados: Some(json!({
                    "identificador": identificador,
                    "sentido": match args.sentido {
                        Sentido::QuemUsa => "quem_usa",
                        Sentido::OQueUsa => "o_que_usa",
                    },
                    "definicoes": definicoes.iter().take(DEFINICOES).map(|d| json!({
                        "nome": d.simbolo.qualificado,
                        "tipo": d.simbolo.tipo,
                        "arquivo": d.arquivo,
                        "linha_inicial": d.simbolo.linhas.inicio,
                        "linha_final": d.simbolo.linhas.fim,
                        "declaracao": d.simbolo.declaracao,
                    })).collect::<Vec<_>>(),
                    "total_de_definicoes": definicoes.len(),
                    "referencias": pagina.dados(),
                    "pagina": pagina.json(),
                    "arvore": arvore.as_ref().map(Arvore::dados),
                    "mapa": carregado.json(),
                })),
            })
        },
    )
    .somente_leitura()
}

fn definicao(d: &Achado<'_>) -> String {
    let s = d.simbolo;
    format!(
        "{} {} ({}:{}–{}){}",
        nome(&s.tipo),
        s.qualificado,
        d.arquivo,
        s.linhas.inicio,
        s.linhas.fim,
        if s.declaracao { " [declaração]" } else { "" }
    )
}

/// O que os símbolos com esse nome usam, por arquivo e linha, sem repetir.
fn o_que_usa<'a>(mapa: &'a Mapa, definicoes: &[Achado<'a>]) -> Vec<Uso<'a>> {
    let mut vistos = BTreeSet::new();
    let mut usos: Vec<Uso<'a>> = definicoes
        .iter()
        .flat_map(|d| mapa.referencias_de(d.id))
        .filter(|u| {
            vistos.insert((
                u.arquivo.to_owned(),
                u.referencia.linha,
                u.referencia.alvo.clone(),
                u.referencia.tipo,
            ))
        })
        .collect();
    usos.sort_by(|a, b| (a.arquivo, a.referencia.linha).cmp(&(b.arquivo, b.referencia.linha)));
    usos
}

fn item(mapa: &Mapa, u: &Uso<'_>, sentido: Sentido, identificador: &str) -> Item {
    let r = u.referencia;
    let tipo = nome(&r.tipo);
    let origem = u.origem.map(|o| o.simbolo.qualificado.clone());
    let alvos: Vec<String> = u
        .alvos
        .iter()
        .filter_map(|id| mapa.simbolo(*id))
        .map(|a| a.simbolo.qualificado.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let mut linha = format!("- {}:{} — {tipo}", u.arquivo, r.linha);
    match sentido {
        Sentido::QuemUsa => {
            if let Some(o) = &origem {
                let _ = write!(linha, ", em {o}");
            }
            if !r.alvo.eq_ignore_ascii_case(identificador) {
                let _ = write!(linha, " (escrito `{}`)", r.alvo);
            }
        }
        Sentido::OQueUsa => {
            let _ = write!(linha, " de {}", r.alvo);
            if let Some(o) = &origem {
                let _ = write!(linha, ", em {o}");
            }
        }
    }
    if alvos.is_empty() {
        linha.push_str(" [sem definição no mapa]");
    } else if alvos.len() > 1 {
        let _ = write!(linha, " [ambíguo: {}]", alvos.join(", "));
    }
    let dados: Value = json!({
        "arquivo": u.arquivo,
        "linha": r.linha,
        "tipo": r.tipo,
        "escrito": r.alvo,
        "origem": origem,
        "alvos": alvos,
    });
    Item { linha, dados }
}
