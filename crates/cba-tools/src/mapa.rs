//! `mapa(modulo?)`: visão do sistema ou de uma pasta (módulo), a partir do mapa
//! estrutural (`cba_index::Mapa::modulo`).
//!
//! A resposta traz, da pasta, quantos arquivos há, por linguagem, quantos símbolos
//! por tipo e as falhas de extração; a lista paginada das subpastas (com a contagem
//! de arquivos e as linguagens de cada uma) e dos arquivos diretos; e, na primeira
//! página, as dependências mais fortes entre as subpastas.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use cba_index::{Mapa, Modulo};
use serde::Deserialize;
use serde_json::json;

use crate::fontes;
use crate::paginacao::{ArgsPagina, Item, Pagina, com_paginacao, nome};
use crate::registro::{ErroFerramenta, Ferramenta, Saida, ler_argumentos};

/// Dependências entre subpastas mostradas, as de mais referências.
const DEPENDENCIAS: usize = 15;

#[derive(Debug, Deserialize)]
struct Args {
    #[serde(default)]
    modulo: Option<String>,
    #[serde(flatten)]
    pagina: ArgsPagina,
}

/// `mapa(modulo?)`: o mapa do sistema ou de um módulo.
pub fn mapa() -> Ferramenta {
    Ferramenta::nova(
        "mapa",
        "Mapa do sistema",
        "Mostra o mapa estrutural do sistema (sem `modulo`) ou de uma pasta: arquivos \
         por linguagem, rotinas, tabelas e classes por tipo, subpastas e arquivos, e as \
         dependências entre as subpastas. Use para se orientar num sistema que você \
         ainda não conhece, antes de abrir arquivos. Resposta paginada.",
        com_paginacao(json!({
            "type": "object",
            "properties": {
                "modulo": {
                    "type": "string",
                    "description": "Pasta relativa à raiz do sistema (\"db\", \"backend/src\"). Sem o campo: o sistema todo."
                }
            },
            "additionalProperties": false
        })),
        |ambiente, args| {
            let args: Args = ler_argumentos(args)?;
            let carregado = fontes::mapa(ambiente)?;
            let mapa = &carregado.mapa;
            let caminho = normalizar(args.modulo.as_deref().unwrap_or_default());
            let Some(modulo) = mapa.modulo(&caminho) else {
                let raiz = mapa.modulo("").map(|m| m.subpastas).unwrap_or_default();
                return Err(ErroFerramenta::Recusa(format!(
                    "a pasta `{caminho}` não tem arquivo mapeado no sistema. Pastas na raiz: \
                     {}. Chame `mapa` sem `modulo` para ver o sistema todo",
                    if raiz.is_empty() {
                        "nenhuma".to_owned()
                    } else {
                        raiz.join(", ")
                    }
                )));
            };

            let itens = entradas(mapa, &modulo);
            let pagina = Pagina::da_lista(itens, args.pagina);
            let primeira = args.pagina.inicio() == 0;
            let mut dependencias = modulo.dependencias.clone();
            dependencias.sort_by_key(|a| std::cmp::Reverse(a.referencias));
            let total_de_dependencias = dependencias.len();
            dependencias.truncate(DEPENDENCIAS);

            let titulo = if modulo.caminho.is_empty() {
                format!("Sistema {}", ambiente.sistema.nome)
            } else {
                format!("Módulo `{}`", modulo.caminho)
            };
            let mut texto = String::new();
            let _ = writeln!(
                texto,
                "{titulo}: {} arquivos ({}); {}.",
                modulo.arquivos,
                contagem(&modulo.linguagens),
                if modulo.simbolos.is_empty() {
                    "nenhum símbolo extraído".to_owned()
                } else {
                    contagem(&modulo.simbolos)
                }
            );
            if modulo.falhas > 0 {
                let _ = writeln!(
                    texto,
                    "{} arquivo(s) sem extração (falha do extrator): leia-os diretamente.",
                    modulo.falhas
                );
            }
            let _ = writeln!(texto, "{}\n", carregado.nota());
            texto.push_str(&pagina.linhas());
            let _ = writeln!(texto, "\n{}", pagina.rodape());
            if primeira && !dependencias.is_empty() {
                let _ = writeln!(
                    texto,
                    "\nDependências entre as partes (referências), {} de {}:",
                    dependencias.len(),
                    total_de_dependencias
                );
                for d in &dependencias {
                    let _ = writeln!(texto, "- {} → {}: {}", d.de, d.para, d.referencias);
                }
            }

            Ok(Saida {
                texto,
                dados: Some(json!({
                    "modulo": modulo.caminho,
                    "arquivos": modulo.arquivos,
                    "linguagens": modulo.linguagens,
                    "simbolos": modulo.simbolos,
                    "falhas": modulo.falhas,
                    "entradas": pagina.dados(),
                    "pagina": pagina.json(),
                    "dependencias": if primeira { json!(dependencias) } else { json!([]) },
                    "total_de_dependencias": total_de_dependencias,
                    "mapa": carregado.json(),
                })),
            })
        },
    )
    .somente_leitura()
}

/// Caminho de pasta como o mapa guarda: com `/`, sem `./` nem barras nas pontas.
pub(crate) fn normalizar(caminho: &str) -> String {
    let c = caminho.trim().replace('\\', "/");
    let c = c.trim_start_matches("./");
    c.trim_matches('/').to_owned()
}

/// `3 sql, 2 java`.
fn contagem<K: serde::Serialize>(mapa: &BTreeMap<K, usize>) -> String {
    let mut v: Vec<(String, usize)> = mapa.iter().map(|(k, n)| (nome(k), *n)).collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    v.iter()
        .map(|(k, n)| format!("{n} {k}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Resumo de uma subpasta: arquivos, linguagens, símbolos e falhas.
#[derive(Default)]
struct Resumo {
    arquivos: usize,
    linguagens: BTreeMap<cba_index::Linguagem, usize>,
    simbolos: usize,
    falhas: usize,
}

/// Subpastas (com o resumo de cada uma) e arquivos diretos da pasta. O resumo sai
/// de uma passada pelos arquivos, sem as dependências de [`Mapa::modulo`], que
/// percorrem todas as referências.
fn entradas(mapa: &Mapa, modulo: &Modulo) -> Vec<Item> {
    let prefixo = if modulo.caminho.is_empty() {
        String::new()
    } else {
        format!("{}/", modulo.caminho)
    };
    let mut resumos: BTreeMap<String, Resumo> = modulo
        .subpastas
        .iter()
        .map(|s| (s.clone(), Resumo::default()))
        .collect();
    for a in mapa.arquivos() {
        let Some((sub, _)) = a
            .caminho
            .strip_prefix(&prefixo)
            .and_then(|resto| resto.split_once('/'))
        else {
            continue;
        };
        let Some(r) = resumos.get_mut(&format!("{prefixo}{sub}")) else {
            continue;
        };
        r.arquivos += 1;
        *r.linguagens.entry(a.linguagem).or_default() += 1;
        r.simbolos += a.extracao.simbolos.len();
        r.falhas += usize::from(a.falha.is_some());
    }
    let mut itens = Vec::new();
    for (sub, m) in &resumos {
        itens.push(Item {
            linha: format!(
                "- {sub}/ — {} arquivos ({}), {} símbolos",
                m.arquivos,
                contagem(&m.linguagens),
                m.simbolos
            ),
            dados: json!({
                "tipo": "pasta",
                "caminho": sub,
                "arquivos": m.arquivos,
                "linguagens": m.linguagens,
                "simbolos": m.simbolos,
                "falhas": m.falhas,
            }),
        });
    }
    for caminho in &modulo.arquivos_diretos {
        let Some(a) = mapa.arquivo(caminho) else {
            continue;
        };
        let simbolos = a.extracao.simbolos.len();
        let falha = a
            .falha
            .as_ref()
            .map(|_| " (sem extração)")
            .unwrap_or_default();
        itens.push(Item {
            linha: format!(
                "- {caminho} — {}, {simbolos} símbolos{falha}",
                nome(&a.linguagem)
            ),
            dados: json!({
                "tipo": "arquivo",
                "caminho": caminho,
                "linguagem": a.linguagem,
                "simbolos": simbolos,
                "falha": a.falha.is_some(),
            }),
        });
    }
    itens
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normaliza_caminho_de_pasta() {
        assert_eq!(normalizar(" ./db/ "), "db");
        assert_eq!(normalizar("backend\\src\\"), "backend/src");
        assert_eq!(normalizar(""), "");
    }
}
