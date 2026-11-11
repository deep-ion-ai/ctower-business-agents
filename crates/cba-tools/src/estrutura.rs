//! `estrutura(arquivo)`: rotinas, procedures, classes, telas e demais símbolos de um
//! arquivo, com as linhas, a partir do mapa estrutural (`cba_index::Mapa::arquivo`).

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::path::Path;

use cba_core::ambiente::Ambiente;
use cba_index::{Mapa, Simbolo};
use serde::Deserialize;
use serde_json::json;

use crate::fontes;
use crate::paginacao::{ArgsPagina, Item, Pagina, com_paginacao, curto, nome};
use crate::registro::{ErroFerramenta, Ferramenta, Saida, ler_argumentos};

/// Arquivos sugeridos quando o caminho informado não está no mapa.
const SUGESTOES: usize = 5;

/// Níveis de recuo no texto (pacote, rotina, ...).
const RECUO_MAXIMO: usize = 4;

#[derive(Debug, Deserialize)]
struct Args {
    arquivo: String,
    #[serde(flatten)]
    pagina: ArgsPagina,
}

/// `estrutura(arquivo)`: o que há num arquivo do sistema.
pub fn estrutura() -> Ferramenta {
    Ferramenta::nova(
        "estrutura",
        "Estrutura de um arquivo",
        "Lista as rotinas, procedures, funções, classes, métodos, telas, tabelas e \
         colunas de um arquivo do sistema, com as linhas de cada um, o pai (pacote, \
         classe) e um detalhe curto (assinatura, tipo da coluna, comentário). Use para \
         saber onde ler num arquivo grande antes de abri-lo. Resposta paginada.",
        com_paginacao(json!({
            "type": "object",
            "properties": {
                "arquivo": {
                    "type": "string",
                    "minLength": 1,
                    "description": "Caminho do arquivo relativo à raiz do sistema (\"db/03_pkg_cobranca.sql\"), o mesmo das evidências."
                }
            },
            "required": ["arquivo"],
            "additionalProperties": false
        })),
        |ambiente, args| {
            let args: Args = ler_argumentos(args)?;
            let carregado = fontes::mapa(ambiente)?;
            let mapa = &carregado.mapa;
            let caminho = relativo(ambiente, &args.arquivo);
            let Some(arquivo) = mapa.arquivo(&caminho) else {
                return Err(nao_mapeado(ambiente, mapa, &args.arquivo, &caminho));
            };

            let simbolos = &arquivo.extracao.simbolos;
            let pais: HashMap<&str, Option<&str>> = simbolos
                .iter()
                .map(|s| (s.qualificado.as_str(), s.pai.as_deref()))
                .collect();
            let itens: Vec<Item> = simbolos.iter().map(|s| item(s, &pais)).collect();
            let pagina = Pagina::da_lista(itens, args.pagina);

            let mut referencias: BTreeMap<String, usize> = BTreeMap::new();
            for r in &arquivo.extracao.referencias {
                *referencias.entry(nome(&r.tipo)).or_default() += 1;
            }

            let mut texto = String::new();
            let _ = writeln!(
                texto,
                "Arquivo `{}` ({}): {} símbolos, {} referências a outros símbolos{}.",
                arquivo.caminho,
                nome(&arquivo.linguagem),
                simbolos.len(),
                arquivo.extracao.referencias.len(),
                if referencias.is_empty() {
                    String::new()
                } else {
                    format!(
                        " ({})",
                        referencias
                            .iter()
                            .map(|(t, n)| format!("{n} {t}"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                }
            );
            if let Some(falha) = &arquivo.falha {
                let _ = writeln!(
                    texto,
                    "O extrator falhou neste arquivo ({}): leia-o diretamente.",
                    curto(falha)
                );
            } else if simbolos.is_empty() {
                let _ = writeln!(
                    texto,
                    "Nenhum símbolo extraído (arquivo de configuração, texto ou linguagem \
                     sem extrator específico): leia-o diretamente."
                );
            }
            let _ = writeln!(texto, "{}\n", carregado.nota());
            texto.push_str(&pagina.linhas());
            let _ = writeln!(texto, "\n{}", pagina.rodape());
            if !pagina.itens.is_empty() {
                let _ = writeln!(
                    texto,
                    "Para saber quem usa um símbolo, chame `referencias` com o nome dele."
                );
            }

            Ok(Saida {
                texto,
                dados: Some(json!({
                    "arquivo": arquivo.caminho,
                    "linguagem": arquivo.linguagem,
                    "extrator": arquivo.extrator,
                    "falha": arquivo.falha,
                    "simbolos": pagina.dados(),
                    "referencias": referencias,
                    "pagina": pagina.json(),
                    "mapa": carregado.json(),
                })),
            })
        },
    )
    .somente_leitura()
}

/// Caminho relativo à raiz do sistema, como o mapa guarda. Aceita também o caminho
/// absoluto de um arquivo dentro das pastas do sistema.
pub(crate) fn relativo(ambiente: &Ambiente, arquivo: &str) -> String {
    let bruto = arquivo.trim();
    if Path::new(bruto).is_absolute()
        && let Some(r) = ambiente.escopo.relativo(Path::new(bruto))
    {
        return r;
    }
    crate::mapa::normalizar(bruto)
}

fn nao_mapeado(ambiente: &Ambiente, mapa: &Mapa, informado: &str, caminho: &str) -> ErroFerramenta {
    if let Err(recusa) = ambiente.escopo.resolver(caminho)
        && recusa != cba_index::Recusa::NaoEncontrado
    {
        return ErroFerramenta::Recusa(format!(
            "`{informado}` não pode ser consultado: {recusa}. Informe o caminho relativo à \
             raiz do sistema, como nas evidências"
        ));
    }
    let nome_do_arquivo = caminho.rsplit('/').next().unwrap_or(caminho).to_lowercase();
    let parecidos: Vec<&str> = mapa
        .arquivos()
        .iter()
        .map(|a| a.caminho.as_str())
        .filter(|c| {
            c.rsplit('/')
                .next()
                .is_some_and(|n| n.to_lowercase() == nome_do_arquivo)
        })
        .take(SUGESTOES)
        .collect();
    let mut mensagem = format!("o arquivo `{caminho}` não está no mapa do sistema");
    if parecidos.is_empty() {
        mensagem.push_str(
            ". Ele pode ser novo (o mapa é refeito pelo app) ou não existir; confira o \
             caminho com `mapa` ou com as suas ferramentas de busca",
        );
    } else {
        let _ = write!(
            mensagem,
            ". Arquivos com esse nome: {}",
            parecidos
                .iter()
                .map(|c| format!("`{c}`"))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    ErroFerramenta::Recusa(mensagem)
}

fn item(s: &Simbolo, pais: &HashMap<&str, Option<&str>>) -> Item {
    let mut nivel = 0;
    let mut pai = s.pai.as_deref();
    while let Some(p) = pai {
        nivel += 1;
        if nivel >= RECUO_MAXIMO {
            break;
        }
        pai = pais.get(p).copied().flatten();
    }
    let mostrado = if s.pai.is_some() {
        &s.nome
    } else {
        &s.qualificado
    };
    let tipo = nome(&s.tipo);
    let mut linha = format!(
        "{}- {tipo} {mostrado} (linhas {}–{})",
        "  ".repeat(nivel),
        s.linhas.inicio,
        s.linhas.fim
    );
    if s.declaracao {
        linha.push_str(" [declaração]");
    }
    if let Some(d) = &s.detalhe {
        let _ = write!(linha, ": {}", curto(d));
    }
    if let Some(c) = &s.comentario {
        let _ = write!(linha, " — {}", curto(c));
    }
    Item {
        linha,
        dados: json!({
            "nome": s.nome,
            "qualificado": s.qualificado,
            "tipo": s.tipo,
            "linha_inicial": s.linhas.inicio,
            "linha_final": s.linhas.fim,
            "pai": s.pai,
            "detalhe": s.detalhe.as_deref().map(curto),
            "comentario": s.comentario.as_deref().map(curto),
            "declaracao": s.declaracao,
        }),
    }
}
