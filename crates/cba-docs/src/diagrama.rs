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
//!
//! No fluxo do processo (#120), os passos com raia ficam num `subgraph` por ator,
//! na ordem em que cada ator aparece. O modelo de dados conceitual (tipo `dados`)
//! sai como `erDiagram`: cada entidade com o nome de negócio e a tabela de origem
//! (`e1["Fatura (FATURA)"]`) e os atributos de negócio (a coluna e o nome); cada
//! relação com a cardinalidade das duas pontas, em linha contínua quando vem de
//! chave estrangeira confirmada e tracejada quando é inferida de um join no código
//! (ou não confirmada).

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
    /// Modelo de dados conceitual (#120).
    Dados,
}

impl TipoDiagrama {
    /// Nome do nó, da aresta e do rótulo da aresta nas tabelas.
    fn nomes(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Self::Estados => ("Estado", "Transição", "Evento"),
            Self::Fluxo => ("Passo", "Caminho", "Condição"),
            Self::Dados => ("Entidade", "Relação", "Verbo"),
        }
    }
}

/// Entidade de negócio de um nó do modelo de dados.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntidadeDiagrama {
    /// Tabela de origem.
    pub tabela: String,
    #[serde(default)]
    pub atributos: Vec<AtributoDiagrama>,
    /// Termo do glossário que vive na tabela, se houver.
    #[serde(default)]
    pub termo: Option<String>,
}

/// Atributo de negócio: o nome e a coluna de origem.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtributoDiagrama {
    pub nome: String,
    pub coluna: String,
}

/// Multiplicidade de uma ponta da relação
/// (`cba_core::diagramas::Multiplicidade`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Multiplicidade {
    ZeroOuUm,
    Um,
    ZeroOuMuitos,
    UmOuMuitos,
}

impl Multiplicidade {
    fn rotulo(self) -> &'static str {
        match self {
            Self::ZeroOuUm => "0..1",
            Self::Um => "1",
            Self::ZeroOuMuitos => "0..N",
            Self::UmOuMuitos => "1..N",
        }
    }

    /// Marca do `erDiagram` na ponta esquerda (a entidade de origem).
    fn marca_esquerda(self) -> &'static str {
        match self {
            Self::ZeroOuUm => "|o",
            Self::Um => "||",
            Self::ZeroOuMuitos => "}o",
            Self::UmOuMuitos => "}|",
        }
    }

    /// Marca do `erDiagram` na ponta direita (a entidade de destino).
    fn marca_direita(self) -> &'static str {
        match self {
            Self::ZeroOuUm => "o|",
            Self::Um => "||",
            Self::ZeroOuMuitos => "o{",
            Self::UmOuMuitos => "|{",
        }
    }
}

/// Origem da relação (`cba_core::diagramas::OrigemRelacao`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrigemRelacao {
    ChaveEstrangeira,
    Join,
}

impl OrigemRelacao {
    fn rotulo(self) -> &'static str {
        match self {
            Self::ChaveEstrangeira => "chave estrangeira",
            Self::Join => "inferida (join no código)",
        }
    }
}

/// Relação entre entidades de uma aresta do modelo de dados.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelacaoDiagrama {
    pub de: Multiplicidade,
    pub para: Multiplicidade,
    pub origem: OrigemRelacao,
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
    /// Raia (ator) do passo, no fluxo do processo.
    #[serde(default)]
    pub raia: Option<String>,
    /// Entidade, no modelo de dados.
    #[serde(default)]
    pub entidade: Option<EntidadeDiagrama>,
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
    /// Guarda da transição (#119).
    #[serde(default)]
    pub condicao: Option<String>,
    /// Efeitos colaterais da transição (#119).
    #[serde(default)]
    pub efeitos: Option<String>,
    pub confianca: Confianca,
    #[serde(default)]
    pub evidencias: Vec<Citacao>,
    /// Relação, no modelo de dados.
    #[serde(default)]
    pub relacao: Option<RelacaoDiagrama>,
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
    /// Achados do ciclo de vida (`cba_core::diagramas::Diagrama::achados`), em
    /// linguagem de negócio, listados abaixo das tabelas (#119).
    #[serde(default)]
    pub achados: Vec<String>,
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
        TipoDiagrama::Dados => return mermaid_dados(diagrama),
    };
    let mut linhas = vec![format!("flowchart {direcao}")];
    let mut inicio = Vec::new();
    let mut fim = Vec::new();
    let mut declaracoes: Vec<(Option<&str>, String)> = Vec::new();
    for (i, no) in diagrama.nos.iter().enumerate() {
        let id = format!("n{}", i + 1);
        let rotulo = rotulo_mermaid(&no.rotulo);
        let forma = match (no.papel, diagrama.tipo) {
            (PapelNo::Decisao, _) => format!("{{{rotulo}}}"),
            (PapelNo::Inicio | PapelNo::Fim, _) => format!("([{rotulo}])"),
            (PapelNo::Etapa, TipoDiagrama::Estados) => format!("({rotulo})"),
            (PapelNo::Etapa, _) => format!("[{rotulo}]"),
        };
        match no.papel {
            PapelNo::Inicio => inicio.push(id.clone()),
            PapelNo::Fim => fim.push(id.clone()),
            _ => {}
        }
        let raia = no
            .raia
            .as_deref()
            .map(str::trim)
            .filter(|r| !r.is_empty() && diagrama.tipo == TipoDiagrama::Fluxo);
        declaracoes.push((raia, format!("{id}{forma}")));
    }
    // Raias na ordem em que cada ator aparece; os passos sem raia ficam fora.
    let mut raias: Vec<&str> = Vec::new();
    for (raia, _) in &declaracoes {
        if let Some(r) = raia
            && !raias.contains(r)
        {
            raias.push(r);
        }
    }
    for (i, raia) in raias.iter().enumerate() {
        linhas.push(format!("  subgraph r{}[{}]", i + 1, rotulo_mermaid(raia)));
        for (_, d) in declaracoes.iter().filter(|(r, _)| r == &Some(*raia)) {
            linhas.push(format!("    {d}"));
        }
        linhas.push("  end".into());
    }
    for (_, d) in declaracoes.iter().filter(|(r, _)| r.is_none()) {
        linhas.push(format!("  {d}"));
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

/// Palavra do `erDiagram` (tipo e nome do atributo): letras sem acento, dígitos,
/// `_` e `-`, começando por letra ou `_`.
fn palavra_er(texto: &str) -> String {
    let mut saida = String::new();
    for c in texto.trim().chars() {
        let c = sem_acento(c);
        if c.is_ascii_alphanumeric() || c == '-' {
            saida.push(c);
        } else if !saida.ends_with('_') {
            saida.push('_');
        }
    }
    let saida = saida.trim_matches('_').to_owned();
    match saida.chars().next() {
        None => "_".to_owned(),
        Some(c) if c.is_ascii_digit() || c == '-' => format!("_{saida}"),
        Some(_) => saida,
    }
}

fn sem_acento(c: char) -> char {
    match c {
        'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
        'Á' | 'À' | 'Â' | 'Ã' | 'Ä' => 'A',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'É' | 'È' | 'Ê' | 'Ë' => 'E',
        'í' | 'ì' | 'î' | 'ï' => 'i',
        'Í' | 'Ì' | 'Î' | 'Ï' => 'I',
        'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
        'Ó' | 'Ò' | 'Ô' | 'Õ' | 'Ö' => 'O',
        'ú' | 'ù' | 'û' | 'ü' => 'u',
        'Ú' | 'Ù' | 'Û' | 'Ü' => 'U',
        'ç' => 'c',
        'Ç' => 'C',
        'ñ' => 'n',
        'Ñ' => 'N',
        c => c,
    }
}

/// Nome da entidade na figura: o nome de negócio e a tabela de origem.
fn nome_da_entidade(no: &NoDiagrama) -> String {
    match &no.entidade {
        Some(e) if !e.tabela.trim().eq_ignore_ascii_case(no.rotulo.trim()) => {
            format!("{} ({})", no.rotulo.trim(), e.tabela.trim())
        }
        _ => no.rotulo.trim().to_owned(),
    }
}

/// Modelo de dados conceitual como `erDiagram`: entidades `e1`…`eN` com o nome de
/// negócio e a tabela, os atributos (`COLUNA Nome_de_negocio`) e as relações com a
/// cardinalidade, contínuas (chave estrangeira confirmada) ou tracejadas.
fn mermaid_dados(diagrama: &Diagrama) -> String {
    let mut linhas = vec!["erDiagram".to_owned()];
    for (i, no) in diagrama.nos.iter().enumerate() {
        let id = format!("e{}", i + 1);
        let nome = rotulo_mermaid(&nome_da_entidade(no));
        let atributos = no
            .entidade
            .as_ref()
            .map(|e| e.atributos.as_slice())
            .unwrap_or_default();
        if atributos.is_empty() {
            linhas.push(format!("  {id}[{nome}]"));
        } else {
            linhas.push(format!("  {id}[{nome}] {{"));
            for a in atributos {
                linhas.push(format!(
                    "    {} {}",
                    palavra_er(&a.coluna),
                    palavra_er(&a.nome)
                ));
            }
            linhas.push("  }".into());
        }
    }
    for aresta in &diagrama.arestas {
        let (Some(de), Some(para)) = (
            diagrama.nos.iter().position(|n| n.chave == aresta.de),
            diagrama.nos.iter().position(|n| n.chave == aresta.para),
        ) else {
            continue;
        };
        let relacao = aresta.relacao.unwrap_or(RelacaoDiagrama {
            de: Multiplicidade::Um,
            para: Multiplicidade::ZeroOuMuitos,
            origem: OrigemRelacao::Join,
        });
        let linha = if relacao.origem == OrigemRelacao::ChaveEstrangeira
            && aresta.confianca == Confianca::Confirmado
        {
            "--"
        } else {
            ".."
        };
        let rotulo = aresta
            .rotulo
            .as_deref()
            .map(str::trim)
            .filter(|r| !r.is_empty())
            .unwrap_or("relaciona");
        linhas.push(format!(
            "  e{} {}{linha}{} e{} : {}",
            de + 1,
            relacao.de.marca_esquerda(),
            relacao.para.marca_direita(),
            para + 1,
            rotulo_mermaid(rotulo)
        ));
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
    let mut blocos = vec![
        crate::conteudo::bloco_de_codigo(&mermaid(diagrama), LINGUAGEM_MERMAID),
        format!("*Figura {figura}: {}*", celula(&diagrama.titulo)),
    ];
    if diagrama.tipo == TipoDiagrama::Dados {
        blocos.extend(tabelas_dos_dados(diagrama, referencias));
        return blocos.join("\n\n");
    }
    let (nome_no, nome_aresta, nome_rotulo) = diagrama.tipo.nomes();
    let com_raia = diagrama.nos.iter().any(|n| n.raia.is_some());
    let detalhados: Vec<&NoDiagrama> = diagrama
        .nos
        .iter()
        .filter(|n| n.descricao.is_some() || !n.evidencias.is_empty())
        .collect();
    if !detalhados.is_empty() {
        let mut tabela = if com_raia {
            vec![
                format!("| {nome_no} | Raia | Descrição |"),
                "| --- | --- | --- |".to_owned(),
            ]
        } else {
            vec![
                format!("| {nome_no} | Descrição |"),
                "| --- | --- |".to_owned(),
            ]
        };
        for no in detalhados {
            let refs = referencias(&no.evidencias);
            let raia = if com_raia {
                format!(" {} |", celula(no.raia.as_deref().unwrap_or("")))
            } else {
                String::new()
            };
            tabela.push(format!(
                "| {}{} |{raia} {} |",
                celula(&no.rotulo),
                no.papel.sufixo(),
                com_notas(no.descricao.as_deref(), &refs)
            ));
        }
        blocos.push(tabela.join("\n"));
    }
    if !diagrama.arestas.is_empty() {
        // Condição e efeitos (#119) só ganham coluna quando alguma transição os tem.
        let guarda = diagrama
            .arestas
            .iter()
            .any(|a| a.condicao.is_some() || a.efeitos.is_some());
        let mut tabela = if guarda {
            vec![
                format!(
                    "| {nome_aresta} | {nome_rotulo} | Condição | Efeitos | Descrição | Confiança |"
                ),
                "| --- | --- | --- | --- | --- | --- |".to_owned(),
            ]
        } else {
            vec![
                format!("| {nome_aresta} | {nome_rotulo} | Descrição | Confiança |"),
                "| --- | --- | --- | --- |".to_owned(),
            ]
        };
        for aresta in &diagrama.arestas {
            let refs = referencias(&aresta.evidencias);
            let extra = if guarda {
                format!(
                    "{} | {} | ",
                    celula(aresta.condicao.as_deref().unwrap_or("")),
                    celula(aresta.efeitos.as_deref().unwrap_or(""))
                )
            } else {
                String::new()
            };
            tabela.push(format!(
                "| {} → {} | {} | {extra}{} | {} |",
                celula(diagrama.rotulo_do_no(&aresta.de)),
                celula(diagrama.rotulo_do_no(&aresta.para)),
                celula(aresta.rotulo.as_deref().unwrap_or("")),
                com_notas(aresta.descricao.as_deref(), &refs),
                aresta.confianca.rotulo()
            ));
        }
        blocos.push(tabela.join("\n"));
    }
    if !diagrama.achados.is_empty() {
        let mut lista = vec![format!(
            "**Achados do ciclo de vida ({}):**",
            celula(&diagrama.titulo)
        )];
        lista.extend(diagrama.achados.iter().map(|a| format!("- {}", celula(a))));
        blocos.push(lista.join("\n"));
    }
    blocos.join("\n\n")
}

/// Tabelas do modelo de dados: as entidades (nome, tabela, atributos de negócio e
/// termo do glossário) e as relações (cardinalidade e origem).
fn tabelas_dos_dados(
    diagrama: &Diagrama,
    referencias: &mut dyn FnMut(&[Citacao]) -> String,
) -> Vec<String> {
    let mut blocos = Vec::new();
    let mut tabela = vec![
        "| Entidade | Tabela | Atributos de negócio | Glossário | Descrição |".to_owned(),
        "| --- | --- | --- | --- | --- |".to_owned(),
    ];
    for no in &diagrama.nos {
        let refs = referencias(&no.evidencias);
        let (tabela_de_origem, atributos, termo) = match &no.entidade {
            Some(e) => (
                format!("`{}`", celula(&e.tabela)),
                e.atributos
                    .iter()
                    .map(|a| format!("{} (`{}`)", celula(&a.nome), celula(&a.coluna)))
                    .collect::<Vec<_>>()
                    .join("; "),
                e.termo
                    .as_deref()
                    .map_or_else(|| "não está no glossário".to_owned(), celula),
            ),
            None => (String::new(), String::new(), String::new()),
        };
        tabela.push(format!(
            "| {} | {tabela_de_origem} | {atributos} | {termo} | {} |",
            celula(&no.rotulo),
            com_notas(no.descricao.as_deref(), &refs)
        ));
    }
    blocos.push(tabela.join("\n"));
    if !diagrama.arestas.is_empty() {
        let mut tabela = vec![
            "| Relação | Cardinalidade | Origem | Descrição | Confiança |".to_owned(),
            "| --- | --- | --- | --- | --- |".to_owned(),
        ];
        for aresta in &diagrama.arestas {
            let refs = referencias(&aresta.evidencias);
            let verbo = aresta
                .rotulo
                .as_deref()
                .map(|r| format!(" ({})", celula(r)))
                .unwrap_or_default();
            let (cardinalidade, origem) = aresta.relacao.map_or_else(
                || (String::new(), String::new()),
                |r| {
                    (
                        format!("{}:{}", r.de.rotulo(), r.para.rotulo()),
                        r.origem.rotulo().to_owned(),
                    )
                },
            );
            tabela.push(format!(
                "| {} → {}{verbo} | {cardinalidade} | {origem} | {} | {} |",
                celula(diagrama.rotulo_do_no(&aresta.de)),
                celula(diagrama.rotulo_do_no(&aresta.para)),
                com_notas(aresta.descricao.as_deref(), &refs),
                aresta.confianca.rotulo()
            ));
        }
        blocos.push(tabela.join("\n"));
    }
    blocos
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
            raia: None,
            entidade: None,
        }
    }

    fn aresta(de: &str, para: &str, rotulo: Option<&str>, confianca: Confianca) -> ArestaDiagrama {
        ArestaDiagrama {
            de: de.into(),
            para: para.into(),
            rotulo: rotulo.map(Into::into),
            descricao: None,
            condicao: None,
            efeitos: None,
            confianca,
            evidencias: vec![],
            relacao: None,
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
            achados: vec![],
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

    #[test]
    fn condicao_efeitos_e_achados_do_ciclo_de_vida() {
        let mut d = fatura();
        d.arestas[0].condicao = Some("itens | cliente ativo".into());
        d.arestas[1].efeitos = Some("grava o log".into());
        d.achados = vec!["O estado Protestada é inalcançável.".into()];
        let md = markdown(&d, 1, &mut |_: &[Citacao]| String::new());
        assert!(
            md.contains(
                "| Transição | Evento | Condição | Efeitos | Descrição | Confiança |\n\
                 | --- | --- | --- | --- | --- | --- |\n\
                 | Rascunho → Emitida | emitir | itens \\| cliente ativo |  |  | confirmado |\n\
                 | Emitida → Paga | pagar com juros |  | grava o log |  | inferido |"
            ),
            "{md}"
        );
        assert!(
            md.ends_with(
                "**Achados do ciclo de vida (Ciclo de vida da fatura):**\n\
                 - O estado Protestada é inalcançável."
            ),
            "{md}"
        );
    }

    fn raia(n: NoDiagrama, raia: &str) -> NoDiagrama {
        NoDiagrama {
            raia: Some(raia.into()),
            ..n
        }
    }

    #[test]
    fn fluxo_com_raias_em_subgraph() {
        let d = Diagrama {
            tipo: TipoDiagrama::Fluxo,
            titulo: "Cancelamento de fatura".into(),
            nos: vec![
                raia(
                    no("i", "Pedir o cancelamento", PapelNo::Inicio),
                    "Atendente",
                ),
                raia(
                    no("m", "Motivo com 10 caracteres?", PapelNo::Decisao),
                    "Atendente",
                ),
                raia(
                    no("g", "Gravar o cancelamento", PapelNo::Etapa),
                    "Banco de dados",
                ),
                no("f", "Fim", PapelNo::Fim),
            ],
            arestas: vec![
                aresta("i", "m", None, Confianca::Confirmado),
                aresta("m", "g", Some("sim"), Confianca::Confirmado),
                aresta("g", "f", None, Confianca::Inferido),
            ],
            ..fatura()
        };
        assert_eq!(
            mermaid(&d),
            "flowchart TD\n  subgraph r1[\"Atendente\"]\n    n1([\"Pedir o cancelamento\"])\n    \
             n2{\"Motivo com 10 caracteres?\"}\n  end\n  subgraph r2[\"Banco de dados\"]\n    \
             n3[\"Gravar o cancelamento\"]\n  end\n  n4([\"Fim\"])\n  n1 --> n2\n  \
             n2 -->|\"sim\"| n3\n  n3 -.-> n4\n  classDef inicio stroke-width:3px\n  \
             class n1 inicio\n  classDef fim stroke-width:3px\n  class n4 fim\n"
        );
        let mut d = d;
        d.nos[2].descricao = Some("PRC_CANCELAR_FATURA".into());
        let md = markdown(&d, 1, &mut |_: &[Citacao]| String::new());
        assert!(
            md.contains(
                "| Passo | Raia | Descrição |\n| --- | --- | --- |\n\
                 | Gravar o cancelamento | Banco de dados | PRC_CANCELAR_FATURA |"
            ),
            "{md}"
        );
    }

    fn entidade(chave: &str, rotulo: &str, tabela: &str, atributos: &[(&str, &str)]) -> NoDiagrama {
        NoDiagrama {
            entidade: Some(EntidadeDiagrama {
                tabela: tabela.into(),
                atributos: atributos
                    .iter()
                    .map(|(nome, coluna)| AtributoDiagrama {
                        nome: (*nome).into(),
                        coluna: (*coluna).into(),
                    })
                    .collect(),
                termo: None,
            }),
            ..no(chave, rotulo, PapelNo::Etapa)
        }
    }

    fn relacao(
        de: &str,
        para: &str,
        verbo: &str,
        origem: OrigemRelacao,
        confianca: Confianca,
    ) -> ArestaDiagrama {
        ArestaDiagrama {
            relacao: Some(RelacaoDiagrama {
                de: Multiplicidade::Um,
                para: Multiplicidade::ZeroOuMuitos,
                origem,
            }),
            ..aresta(de, para, Some(verbo), confianca)
        }
    }

    fn modelo() -> Diagrama {
        Diagrama {
            secao: "modelo_dados".into(),
            tipo: TipoDiagrama::Dados,
            titulo: "Modelo de dados do faturamento".into(),
            nos: vec![
                entidade(
                    "CLIENTE",
                    "Cliente",
                    "CLIENTE",
                    &[("Tipo de cliente", "TIPO_CLIENTE")],
                ),
                entidade(
                    "FATURA",
                    "Fatura",
                    "FATURA",
                    &[("Data de emissão", "DT_EMISSAO"), ("Situação", "STATUS")],
                ),
                entidade("LOG", "Histórico da fatura", "LOG_FATURA", &[]),
            ],
            arestas: vec![
                relacao(
                    "CLIENTE",
                    "FATURA",
                    "recebe",
                    OrigemRelacao::ChaveEstrangeira,
                    Confianca::Confirmado,
                ),
                relacao(
                    "FATURA",
                    "LOG",
                    "registra",
                    OrigemRelacao::Join,
                    Confianca::Inferido,
                ),
            ],
            achados: vec![],
        }
    }

    #[test]
    fn modelo_de_dados_em_er_com_relacao_inferida_tracejada() {
        assert_eq!(
            mermaid(&modelo()),
            "erDiagram\n  e1[\"Cliente\"] {\n    TIPO_CLIENTE Tipo_de_cliente\n  }\n  \
             e2[\"Fatura\"] {\n    DT_EMISSAO Data_de_emissao\n    STATUS Situacao\n  }\n  \
             e3[\"Histórico da fatura (LOG_FATURA)\"]\n  \
             e1 ||--o{ e2 : \"recebe\"\n  e2 ||..o{ e3 : \"registra\"\n"
        );
        assert_eq!(palavra_er(" 1ª parcela "), "_1_parcela");
        assert_eq!(palavra_er("%%"), "_");
    }

    #[test]
    fn tabelas_do_modelo_de_dados() {
        let mut d = modelo();
        d.nos[1].entidade.as_mut().unwrap().termo = Some("Fatura".into());
        d.arestas[1].evidencias = vec![Citacao::nova(
            CaminhoRelativo::novo("db/03_pkg_cobranca.sql").unwrap(),
            Linhas::novo(130, 131).unwrap(),
        )];
        let md = markdown(&d, 3, &mut |c: &[Citacao]| {
            if c.is_empty() {
                String::new()
            } else {
                "[^7]".into()
            }
        });
        assert!(md.starts_with("```mermaid\nerDiagram\n"), "{md}");
        assert!(
            md.contains("*Figura 3: Modelo de dados do faturamento*"),
            "{md}"
        );
        assert!(
            md.contains(
                "| Fatura | `FATURA` | Data de emissão (`DT_EMISSAO`); Situação (`STATUS`) | Fatura |  |"
            ),
            "{md}"
        );
        assert!(md.contains("| Cliente | `CLIENTE` | Tipo de cliente (`TIPO_CLIENTE`) | não está no glossário |  |"), "{md}");
        assert!(
            md.contains(
                "| Cliente → Fatura (recebe) | 1:0..N | chave estrangeira |  | confirmado |\n\
                 | Fatura → Histórico da fatura (registra) | 1:0..N | inferida (join no código) | [^7] | inferido |"
            ),
            "{md}"
        );
    }
}
