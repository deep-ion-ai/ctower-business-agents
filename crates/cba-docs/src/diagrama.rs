//! Diagramas: dos elementos registrados para o Mermaid (#118).
//!
//! O agente registra nós e arestas com evidências; a montagem chama [`mermaid`]
//! para gerar o código e [`markdown`] para o bloco completo do documento:
//!
//! ````text
//! ```mermaid
//! flowchart LR
//!   n1(["Rascunho"])
//!   n2("Emitida")
//!   n1 -->|"emitir"| n2
//! ```
//!
//! *Figura 1: Ciclo de vida da fatura*
//!
//! | Transição | Evento | Descrição | Confiança |
//! ...
//! ````
//!
//! O Mermaid é gerado por construção: ids `n1`…`nN` na ordem dos nós, rótulos
//! entre aspas numa linha só, com o que o Mermaid interpretaria trocado
//! ([`rotulo_mermaid`]), e nada do texto do agente fora das aspas. Por isso o código
//! gerado é sempre válido; um bloco inválido só aparece se alguém editar o
//! documento à mão, e a prévia e a exportação tratam esse caso (aviso e código).
//!
//! Os dois tipos usam `flowchart`, porque o `stateDiagram` do Mermaid não tem
//! transição tracejada: aresta `inferido` ou `a_confirmar` sai como `-.->`. Os
//! estados saem da esquerda para a direita (`LR`) e o fluxo de cima para baixo
//! (`TD`). A tabela de elementos logo abaixo da figura leva a descrição, a
//! confiança e as evidências (como notas de rodapé) de cada estado e transição.

use serde::{Deserialize, Serialize};

use crate::conteudo::{Citacao, Confianca};

/// Linguagem do bloco de código dos diagramas.
pub const LINGUAGEM_MERMAID: &str = "mermaid";

/// O que o diagrama mostra (`cba_core::diagramas::TipoDiagrama`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TipoDiagrama {
    Estados,
    Fluxo,
}

impl TipoDiagrama {
    /// Nome do nó, da aresta e do rótulo da aresta nas tabelas.
    fn nomes(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Self::Estados => ("Estado", "Transição", "Evento"),
            Self::Fluxo => ("Passo", "Caminho", "Condição"),
        }
    }
}

/// Papel do nó (`cba_core::diagramas::PapelNo`), que define a forma.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PapelNo {
    #[default]
    Etapa,
    Inicio,
    Fim,
    Decisao,
}

impl PapelNo {
    fn sufixo(self) -> &'static str {
        match self {
            Self::Etapa => "",
            Self::Inicio => " (início)",
            Self::Fim => " (fim)",
            Self::Decisao => " (decisão)",
        }
    }
}

/// Nó do diagrama.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoDiagrama {
    pub chave: String,
    pub rotulo: String,
    #[serde(default)]
    pub papel: PapelNo,
    #[serde(default)]
    pub descricao: Option<String>,
    #[serde(default)]
    pub evidencias: Vec<Citacao>,
}

/// Aresta do diagrama.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArestaDiagrama {
    pub de: String,
    pub para: String,
    #[serde(default)]
    pub rotulo: Option<String>,
    #[serde(default)]
    pub descricao: Option<String>,
    pub confianca: Confianca,
    #[serde(default)]
    pub evidencias: Vec<Citacao>,
}

/// Diagrama de uma seção do documento.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagrama {
    /// Marcador da seção.
    pub secao: String,
    pub tipo: TipoDiagrama,
    /// Legenda da figura.
    pub titulo: String,
    pub nos: Vec<NoDiagrama>,
    #[serde(default)]
    pub arestas: Vec<ArestaDiagrama>,
}

impl Diagrama {
    /// Id do nó no Mermaid (`n1`…), pela chave.
    pub fn id_do_no(&self, chave: &str) -> Option<String> {
        self.nos
            .iter()
            .position(|n| n.chave == chave)
            .map(|i| format!("n{}", i + 1))
    }

    fn rotulo_do_no<'a>(&'a self, chave: &'a str) -> &'a str {
        self.nos
            .iter()
            .find(|n| n.chave == chave)
            .map_or(chave, |n| n.rotulo.as_str())
    }
}

/// Texto entre aspas no Mermaid, numa linha só.
///
/// Dentro das aspas o Mermaid aceita `;`, `|`, `&` e `#` como texto; o que ele
/// interpretaria vira outra coisa: a aspa vira aspa curva (“ ”), `<` e `>` viram
/// `#lt;` e `#gt;` (o texto aparece igual), a crase vira apóstrofo (crase no
/// começo faria do rótulo Markdown) e o `#` que formaria uma entidade (`#quot;`)
/// vira `＃`. As outras entidades não servem: com os rótulos em SVG puro, o
/// Mermaid mostraria `&#35;` ao pé da letra.
fn rotulo_mermaid(texto: &str) -> String {
    let caracteres: Vec<char> = texto.trim().chars().collect();
    let mut saida = String::with_capacity(texto.len() + 2);
    saida.push('"');
    let mut espaco = false;
    let mut aspa_aberta = false;
    for (i, &c) in caracteres.iter().enumerate() {
        if c.is_whitespace() || c.is_control() {
            espaco = true;
            continue;
        }
        if espaco {
            saida.push(' ');
            espaco = false;
        }
        match c {
            '"' => {
                saida.push(if aspa_aberta { '”' } else { '“' });
                aspa_aberta = !aspa_aberta;
            }
            '<' => saida.push_str("#lt;"),
            '>' => saida.push_str("#gt;"),
            '`' => saida.push('\''),
            '#' if forma_entidade(&caracteres[i + 1..]) => saida.push('＃'),
            c => saida.push(c),
        }
    }
    saida.push('"');
    saida
}

/// O texto depois de um `#` forma uma entidade do Mermaid (`quot;`, `35;`).
fn forma_entidade(resto: &[char]) -> bool {
    let nome = resto
        .iter()
        .take_while(|c| c.is_ascii_alphanumeric())
        .count();
    nome > 0 && resto.get(nome) == Some(&';')
}

/// Código Mermaid do diagrama, sem a cerca do bloco e terminado em `\n`.
///
/// Arestas cujas pontas não são chaves de nós (o modelo já recusa isso no
/// registro) ficam de fora, para o código nunca citar um nó inexistente.
pub fn mermaid(diagrama: &Diagrama) -> String {
    let direcao = match diagrama.tipo {
        TipoDiagrama::Estados => "LR",
        TipoDiagrama::Fluxo => "TD",
    };
    let mut linhas = vec![format!("flowchart {direcao}")];
    let mut inicio = Vec::new();
    let mut fim = Vec::new();
    for (i, no) in diagrama.nos.iter().enumerate() {
        let id = format!("n{}", i + 1);
        let rotulo = rotulo_mermaid(&no.rotulo);
        let forma = match (no.papel, diagrama.tipo) {
            (PapelNo::Decisao, _) => format!("{{{rotulo}}}"),
            (PapelNo::Inicio | PapelNo::Fim, _) => format!("([{rotulo}])"),
            (PapelNo::Etapa, TipoDiagrama::Estados) => format!("({rotulo})"),
            (PapelNo::Etapa, TipoDiagrama::Fluxo) => format!("[{rotulo}]"),
        };
        match no.papel {
            PapelNo::Inicio => inicio.push(id.clone()),
            PapelNo::Fim => fim.push(id.clone()),
            _ => {}
        }
        linhas.push(format!("  {id}{forma}"));
    }
    for aresta in &diagrama.arestas {
        let (Some(de), Some(para)) = (
            diagrama.id_do_no(&aresta.de),
            diagrama.id_do_no(&aresta.para),
        ) else {
            continue;
        };
        let seta = match aresta.confianca {
            Confianca::Confirmado => "-->",
            Confianca::Inferido | Confianca::AConfirmar => "-.->",
        };
        let rotulo = aresta
            .rotulo
            .as_deref()
            .map(str::trim)
            .filter(|r| !r.is_empty())
            .map(|r| format!("|{}|", rotulo_mermaid(r)))
            .unwrap_or_default();
        linhas.push(format!("  {de} {seta}{rotulo} {para}"));
    }
    if !inicio.is_empty() {
        linhas.push("  classDef inicio stroke-width:3px".into());
        linhas.push(format!("  class {} inicio", inicio.join(",")));
    }
    if !fim.is_empty() {
        linhas.push("  classDef fim stroke-width:3px".into());
        linhas.push(format!("  class {} fim", fim.join(",")));
    }
    let mut codigo = linhas.join("\n");
    codigo.push('\n');
    codigo
}

/// Texto de uma célula de tabela: numa linha só, com `|` escapado.
fn celula(texto: &str) -> String {
    texto
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace('|', "\\|")
}

/// Descrição com as referências das notas no fim.
fn com_notas(descricao: Option<&str>, refs: &str) -> String {
    format!("{}{refs}", celula(descricao.unwrap_or("")))
}

/// Bloco do diagrama no documento: o código Mermaid, a legenda numerada e as
/// tabelas de elementos. `referencias` devolve as notas `[^n]` das evidências.
pub(crate) fn markdown(
    diagrama: &Diagrama,
    figura: u32,
    referencias: &mut dyn FnMut(&[Citacao]) -> String,
) -> String {
    let (nome_no, nome_aresta, nome_rotulo) = diagrama.tipo.nomes();
    let mut blocos = vec![
        crate::conteudo::bloco_de_codigo(&mermaid(diagrama), LINGUAGEM_MERMAID),
        format!("*Figura {figura}: {}*", celula(&diagrama.titulo)),
    ];
    let detalhados: Vec<&NoDiagrama> = diagrama
        .nos
        .iter()
        .filter(|n| n.descricao.is_some() || !n.evidencias.is_empty())
        .collect();
    if !detalhados.is_empty() {
        let mut tabela = vec![
            format!("| {nome_no} | Descrição |"),
            "| --- | --- |".to_owned(),
        ];
        for no in detalhados {
            let refs = referencias(&no.evidencias);
            tabela.push(format!(
                "| {}{} | {} |",
                celula(&no.rotulo),
                no.papel.sufixo(),
                com_notas(no.descricao.as_deref(), &refs)
            ));
        }
        blocos.push(tabela.join("\n"));
    }
    if !diagrama.arestas.is_empty() {
        let mut tabela = vec![
            format!("| {nome_aresta} | {nome_rotulo} | Descrição | Confiança |"),
            "| --- | --- | --- | --- |".to_owned(),
        ];
        for aresta in &diagrama.arestas {
            let refs = referencias(&aresta.evidencias);
            tabela.push(format!(
                "| {} → {} | {} | {} | {} |",
                celula(diagrama.rotulo_do_no(&aresta.de)),
                celula(diagrama.rotulo_do_no(&aresta.para)),
                celula(aresta.rotulo.as_deref().unwrap_or("")),
                com_notas(aresta.descricao.as_deref(), &refs),
                aresta.confianca.rotulo()
            ));
        }
        blocos.push(tabela.join("\n"));
    }
    blocos.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use cba_kb::CaminhoRelativo;
    use cba_kb::frontmatter::Linhas;

    fn no(chave: &str, rotulo: &str, papel: PapelNo) -> NoDiagrama {
        NoDiagrama {
            chave: chave.into(),
            rotulo: rotulo.into(),
            papel,
            descricao: None,
            evidencias: vec![],
        }
    }

    fn aresta(de: &str, para: &str, rotulo: Option<&str>, confianca: Confianca) -> ArestaDiagrama {
        ArestaDiagrama {
            de: de.into(),
            para: para.into(),
            rotulo: rotulo.map(Into::into),
            descricao: None,
            confianca,
            evidencias: vec![],
        }
    }

    fn fatura() -> Diagrama {
        Diagrama {
            secao: "fluxo_principal".into(),
            tipo: TipoDiagrama::Estados,
            titulo: "Ciclo de vida da fatura".into(),
            nos: vec![
                no("R", "Rascunho", PapelNo::Inicio),
                no("E", "Emitida", PapelNo::Etapa),
                no("P", "Paga", PapelNo::Fim),
            ],
            arestas: vec![
                aresta("R", "E", Some("emitir"), Confianca::Confirmado),
                aresta("E", "P", Some("pagar com juros"), Confianca::Inferido),
                aresta("E", "X", None, Confianca::Inferido),
            ],
        }
    }

    #[test]
    fn gera_flowchart_com_aresta_inferida_tracejada() {
        assert_eq!(
            mermaid(&fatura()),
            "flowchart LR\n  n1([\"Rascunho\"])\n  n2(\"Emitida\")\n  n3([\"Paga\"])\n  \
             n1 -->|\"emitir\"| n2\n  n2 -.->|\"pagar com juros\"| n3\n  \
             classDef inicio stroke-width:3px\n  class n1 inicio\n  \
             classDef fim stroke-width:3px\n  class n3 fim\n"
        );
    }

    #[test]
    fn fluxo_de_cima_para_baixo_com_decisao() {
        let d = Diagrama {
            tipo: TipoDiagrama::Fluxo,
            nos: vec![
                no("a", "Receber pedido", PapelNo::Etapa),
                no("b", "Cliente ativo?", PapelNo::Decisao),
            ],
            arestas: vec![aresta("a", "b", None, Confianca::Confirmado)],
            ..fatura()
        };
        assert_eq!(
            mermaid(&d),
            "flowchart TD\n  n1[\"Receber pedido\"]\n  n2{\"Cliente ativo?\"}\n  n1 --> n2\n"
        );
    }

    #[test]
    fn rotulos_nao_escapam_das_aspas() {
        assert_eq!(
            rotulo_mermaid(" a \"b\"\n<c> #1; `d` | e & #f "),
            "\"a “b” #lt;c#gt; ＃1; 'd' | e & #f\""
        );
        let d = Diagrama {
            nos: vec![no("x", "end\"]\nclick x call alert()", PapelNo::Etapa)],
            arestas: vec![],
            ..fatura()
        };
        let codigo = mermaid(&d);
        assert_eq!(codigo.lines().count(), 2, "{codigo}");
        assert!(!codigo.contains("\"]"), "{codigo}");
        assert!(
            codigo.contains("n1(\"end“] click x call alert()\")"),
            "{codigo}"
        );
    }

    #[test]
    fn bloco_com_legenda_e_tabelas() {
        let mut d = fatura();
        d.nos[1].descricao = Some("Fatura | enviada\nao cliente".into());
        d.arestas[0].evidencias = vec![Citacao::nova(
            CaminhoRelativo::novo("emissao.pas").unwrap(),
            Linhas::novo(41, 55).unwrap(),
        )];
        d.arestas[0].descricao = Some("itens e cliente ativo".into());
        let mut n = 0;
        let md = markdown(&d, 2, &mut |c: &[Citacao]| {
            if c.is_empty() {
                String::new()
            } else {
                n += 1;
                format!("[^{n}]")
            }
        });
        assert!(md.starts_with("```mermaid\nflowchart LR\n"), "{md}");
        assert!(
            md.contains("```\n\n*Figura 2: Ciclo de vida da fatura*\n\n"),
            "{md}"
        );
        assert!(
            md.contains(
                "| Estado | Descrição |\n| --- | --- |\n| Emitida | Fatura \\| enviada ao cliente |"
            ),
            "{md}"
        );
        assert!(
            md.contains(
                "| Rascunho → Emitida | emitir | itens e cliente ativo[^1] | confirmado |\n\
                 | Emitida → Paga | pagar com juros |  | inferido |"
            ),
            "{md}"
        );
    }
}
