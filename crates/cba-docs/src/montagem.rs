//! Montagem do documento a partir do template e dos registros.
//!
//! A montagem é uma função pura e determinística: o mesmo template e o mesmo
//! [`Conteudo`] dão sempre o mesmo texto. Por isso o documento pode ser montado
//! de novo a cada registro (gravação contínua, spec seção 7.1) e comparado com
//! um texto esperado nos testes.
//!
//! Regras:
//!
//! - Seções saem na ordem do template. Dentro de cada seção, primeiro os textos e
//!   depois as afirmações, na ordem de registro.
//! - Cada afirmação traz o selo de confiança e notas de rodapé `[^n]` com
//!   `arquivo:linhas`. A numeração segue a ordem em que as notas aparecem no
//!   documento; o mesmo trecho citado duas vezes usa a mesma nota.
//! - A seção `evidencias` lista as definições das notas.
//! - Registros de uma seção sem marcador no template, ou de uma seção que o tipo
//!   não tem, vão para o fim do documento, com aviso: nada registrado se perde.

use std::collections::BTreeMap;

use cba_kb::{Documento, Sistema, TipoDocumento};

use crate::conteudo::{Citacao, Conteudo};
use crate::link;
use crate::modelo::{EVIDENCIAS, Montavel, PERGUNTAS, SUGESTOES, texto_vazio};
use crate::template::{Aviso, Parte, Template, Templates};

/// Valor de um campo que o tipo conhece, mas este frontmatter não tem.
const CAMPO_AUSENTE: &str = "—";

/// Documento montado e os avisos da montagem.
#[derive(Debug, Clone, PartialEq)]
pub struct Montado<F> {
    pub documento: Documento<F>,
    pub avisos: Vec<Aviso>,
}

/// Erro que impede a montagem.
#[derive(Debug, thiserror::Error)]
pub enum ErroMontagem {
    #[error("o tipo {0} não tem template")]
    SemTemplate(TipoDocumento),
    #[error("o template é de {template}, mas o documento é de {documento}")]
    TipoDiferente {
        template: TipoDocumento,
        documento: TipoDocumento,
    },
    #[error("frontmatter inválido, campo `{campo}`: {mensagem}")]
    Frontmatter { campo: String, mensagem: String },
    /// Falha ao gravar o documento na base.
    #[error(transparent)]
    Base(#[from] cba_kb::Erro),
}

impl Templates {
    /// Monta o documento com o template do tipo do frontmatter.
    pub fn montar<F: Montavel>(&self, conteudo: &Conteudo<F>) -> Result<Montado<F>, ErroMontagem> {
        let tipo = conteudo.frontmatter.tipo();
        self.template(tipo)
            .ok_or(ErroMontagem::SemTemplate(tipo))?
            .montar(conteudo)
    }

    /// Monta e grava o documento em `conteudo.caminho`, no sistema (atômico,
    /// substituindo a versão anterior). Chamado a cada registro novo de uma
    /// análise em andamento. Devolve os avisos da montagem.
    pub fn gravar<F: Montavel>(
        &self,
        sistema: &Sistema,
        conteudo: &Conteudo<F>,
    ) -> Result<Vec<Aviso>, ErroMontagem> {
        let montado = self.montar(conteudo)?;
        sistema.gravar(&conteudo.caminho, &montado.documento)?;
        Ok(montado.avisos)
    }
}

impl Template {
    /// Monta o documento. O frontmatter precisa ser válido e do tipo do template.
    pub fn montar<F: Montavel>(&self, conteudo: &Conteudo<F>) -> Result<Montado<F>, ErroMontagem> {
        let frontmatter = &conteudo.frontmatter;
        let tipo = frontmatter.tipo();
        if tipo != self.tipo() {
            return Err(ErroMontagem::TipoDiferente {
                template: self.tipo(),
                documento: tipo,
            });
        }
        if !F::TIPOS.is_empty() && !F::TIPOS.contains(&tipo) {
            return Err(ErroMontagem::Frontmatter {
                campo: "tipo".into(),
                mensagem: format!("`{tipo}` não é aceito por este frontmatter"),
            });
        }
        frontmatter
            .validar()
            .map_err(|e| ErroMontagem::Frontmatter {
                campo: e.campo,
                mensagem: e.mensagem,
            })?;

        let modelo = self.modelo();
        let mut avisos = Vec::new();

        // Seções com registros, na ordem do primeiro registro.
        let mut com_registros: Vec<&str> = Vec::new();
        for secao in conteudo
            .textos
            .iter()
            .map(|t| t.secao.as_str())
            .chain(conteudo.afirmacoes.iter().map(|a| a.secao.as_str()))
        {
            if !com_registros.contains(&secao) {
                com_registros.push(secao);
            }
        }
        let tem_conteudo = |marcador: &str| {
            com_registros.contains(&marcador)
                || (marcador == PERGUNTAS && !conteudo.perguntas.is_empty())
                || (marcador == SUGESTOES && !conteudo.sugestoes.is_empty())
        };

        // Ordem de montagem: as seções do template, depois as que ficaram de
        // fora, e `evidencias` por último, porque lista as notas de todas.
        let no_template: Vec<&str> = self
            .marcadores()
            .filter(|m| modelo.secao(m).is_some())
            .collect();
        let mut ordem: Vec<&str> = Vec::new();
        for m in &no_template {
            if *m != EVIDENCIAS && !ordem.contains(m) {
                ordem.push(m);
            }
        }
        let mut no_fim: Vec<(String, &str)> = Vec::new();
        for s in modelo.secoes {
            if s.marcador != EVIDENCIAS
                && !no_template.contains(&s.marcador)
                && tem_conteudo(s.marcador)
            {
                avisos.push(Aviso::SecaoSemMarcador {
                    tipo,
                    secao: s.marcador.into(),
                });
                no_fim.push((s.titulo.into(), s.marcador));
                ordem.push(s.marcador);
            }
        }
        for secao in &com_registros {
            if modelo.secao(secao).is_none() {
                avisos.push(Aviso::SecaoDesconhecida {
                    tipo,
                    secao: (*secao).into(),
                });
                no_fim.push(((*secao).into(), secao));
                ordem.push(secao);
            }
        }

        let mut notas = Notas::default();
        let mut secoes: BTreeMap<&str, String> = BTreeMap::new();
        for m in ordem {
            let texto = montar_secao(m, conteudo, &mut notas);
            secoes.insert(m, texto);
        }
        let evidencias = {
            let mut texto = montar_secao(EVIDENCIAS, conteudo, &mut notas);
            let definicoes = notas.definicoes();
            if !definicoes.is_empty() {
                texto = if texto == texto_vazio(EVIDENCIAS) {
                    definicoes
                } else {
                    format!("{texto}\n\n{definicoes}")
                };
            }
            texto
        };
        let evidencias_tem_conteudo = !notas.lista.is_empty() || tem_conteudo(EVIDENCIAS);
        if !no_template.contains(&EVIDENCIAS) && evidencias_tem_conteudo {
            avisos.push(Aviso::SecaoSemMarcador {
                tipo,
                secao: EVIDENCIAS.into(),
            });
            no_fim.push(("Evidências".into(), EVIDENCIAS));
        }
        secoes.insert(EVIDENCIAS, evidencias);

        let campos: BTreeMap<&str, String> =
            frontmatter.campos(&conteudo.caminho).into_iter().collect();
        let mut corpo = String::from("\n");
        for parte in &self.partes {
            match parte {
                Parte::Texto(t) => corpo.push_str(t),
                Parte::Marcador(m) => match secoes.get(m.as_str()) {
                    Some(s) => corpo.push_str(s),
                    None => corpo.push_str(campos.get(m.as_str()).map_or(CAMPO_AUSENTE, |v| v)),
                },
            }
        }
        for (titulo, marcador) in no_fim {
            corpo.push_str(&format!("\n\n## {titulo}\n\n{}\n", secoes[marcador]));
        }

        Ok(Montado {
            documento: Documento::novo(frontmatter.clone(), normalizar(&corpo)),
            avisos,
        })
    }
}

/// Notas de rodapé do documento, na ordem em que aparecem.
#[derive(Default)]
struct Notas {
    lista: Vec<Citacao>,
}

impl Notas {
    /// Referências `[^n]` das citações, criando as notas novas.
    fn referencias(&mut self, citacoes: &[Citacao]) -> String {
        let mut numeros: Vec<usize> = Vec::new();
        for c in citacoes {
            let posicao = match self
                .lista
                .iter()
                .position(|n| n.arquivo == c.arquivo && n.linhas == c.linhas)
            {
                Some(p) => {
                    let nota = &mut self.lista[p];
                    if nota.hash.is_none() {
                        nota.hash.clone_from(&c.hash);
                    }
                    if nota.commit.is_none() {
                        nota.commit.clone_from(&c.commit);
                    }
                    p
                }
                None => {
                    self.lista.push(c.clone());
                    self.lista.len() - 1
                }
            };
            if !numeros.contains(&(posicao + 1)) {
                numeros.push(posicao + 1);
            }
        }
        numeros.iter().map(|n| format!("[^{n}]")).collect()
    }

    fn definicoes(&self) -> String {
        let mut linhas = Vec::new();
        for (i, c) in self.lista.iter().enumerate() {
            let mut linha = format!("[^{}]: `{}`", i + 1, c.local());
            if let Some(h) = &c.hash {
                linha.push_str(&format!(" · hash `{h}`"));
            }
            if let Some(commit) = &c.commit {
                linha.push_str(&format!(" · commit `{commit}`"));
            }
            linhas.push(linha);
        }
        linhas.join("\n")
    }
}

fn montar_secao<F>(marcador: &str, conteudo: &Conteudo<F>, notas: &mut Notas) -> String {
    let mut blocos: Vec<String> = Vec::new();
    for t in conteudo.textos.iter().filter(|t| t.secao == marcador) {
        let refs = notas.referencias(&t.citacoes);
        blocos.push(com_referencias(t.texto.trim(), &refs));
    }
    let afirmacoes: Vec<String> = conteudo
        .afirmacoes
        .iter()
        .filter(|a| a.secao == marcador)
        .map(|a| {
            let refs = notas.referencias(&a.evidencias);
            let texto = com_referencias(a.texto.trim(), &refs);
            format!("- **[{}]** {}", a.confianca.rotulo(), recuar(&texto, 2))
        })
        .collect();
    if !afirmacoes.is_empty() {
        blocos.push(afirmacoes.join("\n"));
    }
    if marcador == PERGUNTAS && !conteudo.perguntas.is_empty() {
        let itens: Vec<String> = conteudo
            .perguntas
            .iter()
            .enumerate()
            .map(|(i, p)| format!("{}. {}", i + 1, recuar(p.texto.trim(), 3)))
            .collect();
        blocos.push(itens.join("\n"));
    }
    if marcador == SUGESTOES && !conteudo.sugestoes.is_empty() {
        let itens: Vec<String> = conteudo
            .sugestoes
            .iter()
            .map(|s| {
                let mut item = format!(
                    "- {}",
                    link(
                        &format!("{} · {}", s.id, s.titulo),
                        &conteudo.caminho,
                        &s.id.arquivo()
                    )
                );
                if let Some(a) = &s.arquivo {
                    item.push_str(&format!(" (`{a}`)"));
                }
                item
            })
            .collect();
        blocos.push(itens.join("\n"));
    }
    if blocos.is_empty() {
        texto_vazio(marcador).to_owned()
    } else {
        blocos.join("\n\n")
    }
}

/// Acrescenta as referências ao fim do texto, ou num parágrafo próprio quando o
/// texto termina num bloco de código.
fn com_referencias(texto: &str, refs: &str) -> String {
    if refs.is_empty() {
        return texto.to_owned();
    }
    let ultima = texto.lines().last().unwrap_or("").trim_start();
    if ultima.starts_with("```") || ultima.starts_with("~~~") {
        format!("{texto}\n\n{refs}")
    } else {
        format!("{texto}{refs}")
    }
}

/// Recua as linhas depois da primeira, para continuarem no item da lista.
fn recuar(texto: &str, espacos: usize) -> String {
    let recuo = " ".repeat(espacos);
    let mut linhas = texto.lines();
    let mut saida = linhas.next().unwrap_or("").to_owned();
    for l in linhas {
        saida.push('\n');
        if !l.is_empty() {
            saida.push_str(&recuo);
            saida.push_str(l);
        }
    }
    saida
}

/// Junta linhas em branco repetidas (fora de blocos de código), tira espaços do
/// fim das linhas fora dos blocos e termina com uma quebra de linha.
fn normalizar(corpo: &str) -> String {
    let mut saida = String::with_capacity(corpo.len());
    let mut em_bloco = false;
    let mut anterior_vazia = false;
    for (i, linha) in corpo.split('\n').enumerate() {
        let cerca = {
            let l = linha.trim_start();
            l.starts_with("```") || l.starts_with("~~~")
        };
        let linha = if em_bloco { linha } else { linha.trim_end() };
        let vazia = linha.is_empty();
        if !em_bloco && vazia && anterior_vazia {
            continue;
        }
        if i > 0 {
            saida.push('\n');
        }
        saida.push_str(linha);
        anterior_vazia = vazia && !em_bloco;
        if cerca {
            em_bloco = !em_bloco;
        }
    }
    let fim = saida.trim_end().len();
    saida.truncate(fim);
    saida.push('\n');
    saida
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normaliza_fora_dos_blocos() {
        assert_eq!(
            normalizar("\n# T  \n\n\n\ntexto\n```\na\n\n\nb  \n```\n\n\n"),
            "\n# T\n\ntexto\n```\na\n\n\nb  \n```\n"
        );
    }

    #[test]
    fn referencia_em_paragrafo_proprio_depois_de_codigo() {
        assert_eq!(com_referencias("a\n```", "[^1]"), "a\n```\n\n[^1]");
        assert_eq!(com_referencias("frase.", "[^1]"), "frase.[^1]");
        assert_eq!(com_referencias("frase.", ""), "frase.");
    }

    #[test]
    fn recua_continuacao() {
        assert_eq!(recuar("a\nb\n\nc", 2), "a\n  b\n\n  c");
    }
}
