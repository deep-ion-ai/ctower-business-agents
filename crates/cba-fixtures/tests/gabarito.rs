//! Confere a integridade dos gabaritos em `fixtures/gabaritos/`.
//!
//! Para cada gabarito: todo arquivo citado existe dentro da raiz do sistema,
//! todo intervalo de linhas é válido, o trecho esperado aparece no
//! intervalo, os ids são únicos, toda regra citada existe e os três fluxos
//! da spec (pergunta, funcionalidade, impacto) têm ao menos um item.

mod comum;

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Gabarito {
    sistema: String,
    raiz: String,
    regras: Vec<Regra>,
    #[serde(default)]
    inconsistencias: Vec<Inconsistencia>,
    perguntas: Vec<Pergunta>,
    funcionalidades: Vec<Funcionalidade>,
    demandas: Vec<Demanda>,
    exclusoes: Vec<Exclusao>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Regra {
    id: String,
    titulo: String,
    area: String,
    enunciado: String,
    evidencias: Vec<Evidencia>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Inconsistencia {
    id: String,
    titulo: String,
    enunciado: String,
    regras: Vec<String>,
    evidencias: Vec<Evidencia>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Evidencia {
    arquivo: String,
    linha_inicial: usize,
    linha_final: usize,
    trecho: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pergunta {
    id: String,
    pergunta: String,
    resposta_esperada: String,
    regras: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Funcionalidade {
    id: String,
    nome: String,
    objetivo: String,
    regras: Vec<String>,
    arquivos: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Demanda {
    id: String,
    texto: String,
    premissa: String,
    abrangencia_esperada: String,
    regras: Vec<String>,
    arquivos_impactados: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Exclusao {
    arquivo: String,
    motivo: String,
}

/// Acumula os problemas de um gabarito para relatar todos de uma vez.
struct Conferencia {
    nome: String,
    raiz: PathBuf,
    erros: Vec<String>,
}

impl Conferencia {
    fn erro(&mut self, mensagem: String) {
        self.erros.push(format!("{}: {mensagem}", self.nome));
    }

    fn texto_preenchido(&mut self, onde: &str, campo: &str, valor: &str) {
        if valor.trim().is_empty() {
            self.erro(format!("{onde}: campo `{campo}` vazio"));
        }
    }

    /// Valida o caminho relativo e devolve o arquivo resolvido, se existir.
    fn arquivo(&mut self, onde: &str, relativo: &str) -> Option<PathBuf> {
        let caminho = Path::new(relativo);
        let so_normais = caminho
            .components()
            .all(|c| matches!(c, Component::Normal(_)));
        if relativo.is_empty() || relativo.contains('\\') || !so_normais {
            self.erro(format!(
                "{onde}: caminho `{relativo}` deve ser relativo à raiz do sistema, \
                 com `/` e sem `..`"
            ));
            return None;
        }
        let completo = relativo
            .split('/')
            .fold(self.raiz.clone(), |acc, parte| acc.join(parte));
        if !completo.is_file() {
            self.erro(format!("{onde}: arquivo `{relativo}` não existe"));
            return None;
        }
        Some(completo)
    }

    fn evidencia(&mut self, onde: &str, ev: &Evidencia) {
        let Some(caminho) = self.arquivo(onde, &ev.arquivo) else {
            return;
        };
        let conteudo = match fs::read_to_string(&caminho) {
            Ok(c) => c,
            Err(e) => {
                self.erro(format!(
                    "{onde}: não foi possível ler `{}`: {e}",
                    ev.arquivo
                ));
                return;
            }
        };
        let linhas: Vec<&str> = conteudo.lines().collect();
        let total = linhas.len();
        if ev.linha_inicial < 1 || ev.linha_inicial > ev.linha_final || ev.linha_final > total {
            self.erro(format!(
                "{onde}: intervalo {}-{} inválido para `{}` ({total} linhas)",
                ev.linha_inicial, ev.linha_final, ev.arquivo
            ));
            return;
        }
        if ev.trecho.trim().is_empty() {
            self.erro(format!("{onde}: trecho vazio em `{}`", ev.arquivo));
            return;
        }
        let intervalo = linhas[ev.linha_inicial - 1..ev.linha_final].join("\n");
        if !intervalo.contains(&ev.trecho) {
            self.erro(format!(
                "{onde}: trecho `{}` não aparece em `{}` linhas {}-{}",
                ev.trecho, ev.arquivo, ev.linha_inicial, ev.linha_final
            ));
        }
    }
}

fn gabaritos() -> Vec<PathBuf> {
    let pasta = comum::pasta_fixtures().join("gabaritos");
    let lista: Vec<PathBuf> = comum::arquivos_recursivos(&pasta)
        .into_iter()
        .filter(|p| p.extension().is_some_and(|e| e == "yaml" || e == "yml"))
        .collect();
    assert!(!lista.is_empty(), "nenhum gabarito em {}", pasta.display());
    lista
}

fn conferir(caminho: &Path) -> Vec<String> {
    let nome = caminho
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let texto = fs::read_to_string(caminho).expect("gabarito legível");
    let gabarito: Gabarito = match serde_norway::from_str(&texto) {
        Ok(g) => g,
        Err(e) => return vec![format!("{nome}: YAML inválido: {e}")],
    };

    let pasta_gabarito = caminho.parent().expect("gabarito tem pasta");
    let raiz = gabarito
        .raiz
        .split('/')
        .fold(pasta_gabarito.to_path_buf(), |acc, parte| acc.join(parte));
    let mut c = Conferencia {
        nome,
        raiz: raiz.clone(),
        erros: Vec::new(),
    };

    c.texto_preenchido("gabarito", "sistema", &gabarito.sistema);
    if !raiz.is_dir() {
        c.erro(format!("raiz `{}` não é uma pasta", gabarito.raiz));
        return c.erros;
    }
    // As respostas não podem morar dentro do sistema analisado.
    let raiz_canonica = raiz.canonicalize().expect("raiz canônica");
    let gabarito_canonico = caminho.canonicalize().expect("gabarito canônico");
    if gabarito_canonico.starts_with(&raiz_canonica) {
        c.erro("o gabarito não pode ficar dentro da raiz do sistema".to_string());
    }

    // Os três fluxos da spec.
    if gabarito.perguntas.is_empty() {
        c.erro("nenhuma pergunta (fluxo perguntar ao sistema)".to_string());
    }
    if gabarito.funcionalidades.is_empty() {
        c.erro("nenhuma funcionalidade (fluxo explicar funcionalidade)".to_string());
    }
    if gabarito.demandas.is_empty() {
        c.erro("nenhuma demanda (fluxo analisar impacto)".to_string());
    }
    if gabarito.regras.is_empty() {
        c.erro("nenhuma regra".to_string());
    }

    // Ids únicos em todo o gabarito.
    let ids = gabarito
        .regras
        .iter()
        .map(|r| &r.id)
        .chain(gabarito.inconsistencias.iter().map(|i| &i.id))
        .chain(gabarito.perguntas.iter().map(|p| &p.id))
        .chain(gabarito.funcionalidades.iter().map(|f| &f.id))
        .chain(gabarito.demandas.iter().map(|d| &d.id));
    let mut contagem: HashMap<&str, usize> = HashMap::new();
    for id in ids {
        if id.trim().is_empty() {
            c.erro("id vazio".to_string());
        }
        *contagem.entry(id.as_str()).or_default() += 1;
    }
    let mut repetidos: Vec<_> = contagem.iter().filter(|(_, n)| **n > 1).collect();
    repetidos.sort();
    for (id, n) in repetidos {
        c.erro(format!("id `{id}` aparece {n} vezes"));
    }

    // Arquivos excluídos existem e nunca servem de evidência.
    let mut excluidos = HashSet::new();
    for ex in &gabarito.exclusoes {
        c.texto_preenchido(&format!("exclusão {}", ex.arquivo), "motivo", &ex.motivo);
        c.arquivo("exclusões", &ex.arquivo);
        excluidos.insert(ex.arquivo.as_str());
    }

    let regras_validas: HashSet<&str> = gabarito.regras.iter().map(|r| r.id.as_str()).collect();
    let citaveis: HashSet<&str> = regras_validas
        .iter()
        .copied()
        .chain(gabarito.inconsistencias.iter().map(|i| i.id.as_str()))
        .collect();
    let mut citadas_nos_fluxos: HashSet<&str> = HashSet::new();

    for regra in &gabarito.regras {
        let onde = format!("regra {}", regra.id);
        c.texto_preenchido(&onde, "titulo", &regra.titulo);
        c.texto_preenchido(&onde, "area", &regra.area);
        c.texto_preenchido(&onde, "enunciado", &regra.enunciado);
        if regra.evidencias.is_empty() {
            c.erro(format!("{onde}: sem evidências"));
        }
        for ev in &regra.evidencias {
            if excluidos.contains(ev.arquivo.as_str()) {
                c.erro(format!(
                    "{onde}: evidência em arquivo excluído `{}`",
                    ev.arquivo
                ));
            }
            c.evidencia(&onde, ev);
        }
    }

    for inc in &gabarito.inconsistencias {
        let onde = format!("inconsistência {}", inc.id);
        c.texto_preenchido(&onde, "titulo", &inc.titulo);
        c.texto_preenchido(&onde, "enunciado", &inc.enunciado);
        for id in &inc.regras {
            if !regras_validas.contains(id.as_str()) {
                c.erro(format!("{onde}: regra `{id}` não existe"));
            }
        }
        let arquivos: HashSet<&str> = inc.evidencias.iter().map(|e| e.arquivo.as_str()).collect();
        // Uma inconsistência é entre camadas: evidências em dois arquivos ou mais.
        if arquivos.len() < 2 {
            c.erro(format!(
                "{onde}: precisa de evidências em ao menos dois arquivos"
            ));
        }
        for ev in &inc.evidencias {
            if excluidos.contains(ev.arquivo.as_str()) {
                c.erro(format!(
                    "{onde}: evidência em arquivo excluído `{}`",
                    ev.arquivo
                ));
            }
            c.evidencia(&onde, ev);
        }
    }

    let mut citar = |c: &mut Conferencia, onde: &str, regras: &[String]| {
        if regras.is_empty() {
            c.erro(format!("{onde}: nenhuma regra citada"));
        }
        for id in regras {
            if let Some(valida) = citaveis.get(id.as_str()) {
                citadas_nos_fluxos.insert(*valida);
            } else {
                c.erro(format!("{onde}: regra `{id}` não existe"));
            }
        }
    };

    for p in &gabarito.perguntas {
        let onde = format!("pergunta {}", p.id);
        c.texto_preenchido(&onde, "pergunta", &p.pergunta);
        c.texto_preenchido(&onde, "resposta_esperada", &p.resposta_esperada);
        citar(&mut c, &onde, &p.regras);
    }

    for f in &gabarito.funcionalidades {
        let onde = format!("funcionalidade {}", f.id);
        c.texto_preenchido(&onde, "nome", &f.nome);
        c.texto_preenchido(&onde, "objetivo", &f.objetivo);
        citar(&mut c, &onde, &f.regras);
        if f.arquivos.is_empty() {
            c.erro(format!("{onde}: nenhum arquivo"));
        }
        for arquivo in &f.arquivos {
            c.arquivo(&onde, arquivo);
        }
    }

    for d in &gabarito.demandas {
        let onde = format!("demanda {}", d.id);
        c.texto_preenchido(&onde, "texto", &d.texto);
        c.texto_preenchido(&onde, "premissa", &d.premissa);
        if !["pequena", "media", "grande"].contains(&d.abrangencia_esperada.as_str()) {
            c.erro(format!(
                "{onde}: abrangência `{}` deve ser pequena, media ou grande",
                d.abrangencia_esperada
            ));
        }
        citar(&mut c, &onde, &d.regras);
        if d.arquivos_impactados.is_empty() {
            c.erro(format!("{onde}: nenhum arquivo impactado"));
        }
        for arquivo in &d.arquivos_impactados {
            c.arquivo(&onde, arquivo);
        }
    }

    // Toda regra é exercitada por ao menos um fluxo.
    for regra in &gabarito.regras {
        if !citadas_nos_fluxos.contains(regra.id.as_str()) {
            c.erro(format!(
                "regra {} não é citada por nenhuma pergunta, funcionalidade ou demanda",
                regra.id
            ));
        }
    }

    c.erros
}

#[test]
fn gabaritos_sao_consistentes_com_as_fixtures() {
    let erros: Vec<String> = gabaritos().iter().flat_map(|g| conferir(g)).collect();
    assert!(
        erros.is_empty(),
        "problemas nos gabaritos:\n  - {}",
        erros.join("\n  - ")
    );
}

#[test]
fn faturamento_exemplo_tem_gabarito() {
    let caminho = comum::pasta_fixtures()
        .join("gabaritos")
        .join("faturamento-exemplo.yaml");
    assert!(caminho.is_file(), "{} não existe", caminho.display());
}
