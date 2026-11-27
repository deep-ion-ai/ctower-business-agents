//! Confere a integridade dos gabaritos em `fixtures/gabaritos/`.
//!
//! Para cada gabarito: todo arquivo citado existe dentro da raiz do projeto,
//! todo intervalo de linhas é válido, o trecho esperado aparece no
//! intervalo, os ids são únicos, toda regra citada existe e os três fluxos
//! da spec (pergunta, funcionalidade, impacto) têm ao menos um item. Nas
//! reuniões (`reunioes`, fluxo "Analisar reunião"), o transcript existe e tem
//! falas, todo participante é falante dele (e todo falante tem papel
//! esperado), e cada item esperado aponta para falas que existem.

mod comum;

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Gabarito {
    projeto: String,
    raiz: String,
    regras: Vec<Regra>,
    #[serde(default)]
    inconsistencias: Vec<Inconsistencia>,
    perguntas: Vec<Pergunta>,
    funcionalidades: Vec<Funcionalidade>,
    demandas: Vec<Demanda>,
    exclusoes: Vec<Exclusao>,
    #[serde(default)]
    reunioes: Vec<Reuniao>,
    #[serde(default)]
    revisoes: Vec<Revisao>,
    #[serde(default)]
    historias: Vec<Historias>,
    #[serde(default)]
    cenarios: Vec<Cenarios>,
    #[serde(default)]
    ciclos_vida: Vec<CicloVida>,
    #[serde(default)]
    mapeamentos: Vec<Mapeamento>,
    #[serde(default)]
    publicacoes: Vec<Publicacao>,
}

/// Ciclo de publicação da documentação (#279).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Publicacao {
    id: String,
    titulo: String,
    analise: String,
    reanalise: String,
    repositorio: String,
    #[serde(default)]
    subpasta: String,
}

/// Mapa de funcionalidades esperado (#208).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Mapeamento {
    id: String,
    titulo: String,
    pontos_de_entrada: Vec<PontoEsperado>,
    funcionalidades: Vec<FuncionalidadeMapeada>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PontoEsperado {
    chave: String,
    funcionalidades: Vec<String>,
    /// Lido pela avaliação (cba-avaliacao); aqui só o formato conta.
    #[serde(default)]
    #[allow(dead_code)]
    justificavel: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FuncionalidadeMapeada {
    id: String,
    #[serde(default)]
    nome: Option<String>,
    area: String,
    palavras_chave: Vec<Vec<String>>,
}

/// Ciclo de vida esperado de uma entidade com campo de status (#119).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CicloVida {
    id: String,
    titulo: String,
    entidade: String,
    pedido: String,
    campo: Evidencia,
    estados: Vec<EstadoEsperado>,
    transicoes: Vec<TransicaoEsperada>,
    #[serde(default)]
    achados: Vec<AchadoDeEstado>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EstadoEsperado {
    valor: String,
    #[serde(default)]
    inicial: bool,
    #[serde(default)]
    r#final: bool,
    #[serde(default)]
    evidencia: Option<Evidencia>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TransicaoEsperada {
    id: String,
    de: String,
    para: String,
    palavras_chave: Vec<Vec<String>>,
    evidencia: Evidencia,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AchadoDeEstado {
    tipo: String,
    estado: String,
}

/// Cenários de teste esperados a partir das regras (fluxo "Cenários de teste",
/// #117).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cenarios {
    id: String,
    titulo: String,
    pedido: String,
    regras: Vec<String>,
    #[serde(default)]
    limites: Vec<LimiteEsperado>,
    #[serde(default)]
    tabelas: Vec<TabelaEsperada>,
    #[serde(default)]
    negativos: Vec<NegativoEsperado>,
    #[serde(default)]
    a_confirmar: Vec<PerguntaEsperada>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LimiteEsperado {
    id: String,
    regra: String,
    grandeza: String,
    valor: f64,
    valores_testados: Vec<f64>,
    evidencia: Evidencia,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TabelaEsperada {
    id: String,
    regra: String,
    descricao: String,
    condicoes: usize,
    palavras_chave: Vec<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NegativoEsperado {
    id: String,
    regra: String,
    descricao: String,
    palavras_chave: Vec<Vec<String>>,
}

/// Números de um trecho de código (`10`, `10.00`, `2.0000`).
fn numeros_do_trecho(trecho: &str) -> Vec<f64> {
    trecho
        .split(|c: char| !(c.is_ascii_digit() || c == '.'))
        .filter_map(|t| t.trim_matches('.').parse::<f64>().ok())
        .collect()
}

/// Histórias esperadas a partir da análise de impacto de uma demanda (fluxo
/// "Histórias e critérios", #115).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Historias {
    id: String,
    titulo: String,
    #[serde(default)]
    pedido: Option<String>,
    demanda: String,
    regras: Vec<String>,
    #[serde(default)]
    quebra: bool,
    historias_esperadas: Vec<HistoriaEsperada>,
    #[serde(default)]
    a_confirmar: Vec<PerguntaEsperada>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct HistoriaEsperada {
    id: String,
    descricao: String,
    palavras_chave: Vec<Vec<String>>,
}

/// Requisito ou história fictícios com problemas plantados e o parecer esperado
/// (fluxo "Revisar requisito", #116).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Revisao {
    id: String,
    titulo: String,
    texto: String,
    veredito: String,
    #[serde(default)]
    termos_vagos: Vec<String>,
    achados: Vec<AchadoEsperado>,
    #[serde(default)]
    perguntas_esperadas: Vec<PerguntaEsperada>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AchadoEsperado {
    id: String,
    caracteristica: String,
    #[serde(default)]
    tambem_em: Vec<String>,
    descricao: String,
    #[serde(default)]
    regras: Vec<String>,
    palavras_chave: Vec<Vec<String>>,
}

const CARACTERISTICAS: [&str; 6] = [
    "ambiguidade",
    "verificabilidade",
    "completude",
    "consistencia",
    "correcao",
    "singularidade",
];

/// Ata esperada de um transcript de `fixtures/reunioes/` (avaliação, #137).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Reuniao {
    id: String,
    titulo: String,
    /// Relativo ao gabarito.
    transcript: String,
    demanda: String,
    participantes: Vec<ParticipanteEsperado>,
    itens: Vec<ItemDaAta>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ParticipanteEsperado {
    nome: String,
    papel: String,
    #[serde(default)]
    tambem_aceitos: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ItemDaAta {
    id: String,
    tipo: String,
    #[serde(default)]
    tambem_em: Vec<String>,
    descricao: String,
    palavras_chave: Vec<Vec<String>>,
    #[serde(default)]
    responsavel: Vec<Vec<String>>,
    #[serde(default)]
    prazo: Vec<Vec<String>>,
    falas: Vec<String>,
}

/// Tipos de item da ata (`cba_docs::TipoItem`).
const TIPOS_DE_ITEM: [&str; 6] = [
    "decisao",
    "necessidade",
    "regra",
    "pendencia",
    "duvida",
    "proximo_passo",
];

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
    #[serde(default)]
    perguntas_esperadas: Vec<PerguntaEsperada>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Funcionalidade {
    id: String,
    nome: String,
    objetivo: String,
    regras: Vec<String>,
    arquivos: Vec<String>,
    #[serde(default)]
    perguntas_esperadas: Vec<PerguntaEsperada>,
    #[serde(default)]
    processo: Option<ProcessoEsperado>,
    #[serde(default)]
    modelo_dados: Option<ModeloDadosEsperado>,
}

/// Fluxo do processo esperado no documento negocial (#120).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProcessoEsperado {
    raias: Vec<ItemDoProcesso>,
    principal: Vec<ItemDoProcesso>,
    decisoes: Vec<ItemDoProcesso>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ItemDoProcesso {
    id: String,
    descricao: String,
    palavras_chave: Vec<Vec<String>>,
    #[serde(default)]
    evidencias: Vec<Evidencia>,
}

/// Modelo de dados conceitual esperado (#120).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ModeloDadosEsperado {
    entidades: Vec<EntidadeEsperada>,
    relacoes: Vec<RelacaoEsperada>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EntidadeEsperada {
    id: String,
    tabela: String,
    palavras_chave: Vec<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RelacaoEsperada {
    id: String,
    de: String,
    para: String,
    origem: String,
    #[serde(default)]
    opcional: bool,
    evidencias: Vec<Evidencia>,
}

/// Bloco `CREATE TABLE <tabela> (...);` dos scripts de banco da fixture, em
/// maiúsculas.
fn bloco_da_tabela(raiz: &Path, tabela: &str) -> Option<String> {
    let pasta = raiz.join("db");
    let mut arquivos: Vec<PathBuf> = fs::read_dir(&pasta)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "sql"))
        .collect();
    arquivos.sort();
    let inicio = format!("CREATE TABLE {} (", tabela.to_uppercase());
    for arquivo in arquivos {
        let texto = fs::read_to_string(&arquivo).ok()?.to_uppercase();
        if let Some(p) = texto.find(&inicio) {
            let resto = &texto[p..];
            let fim = resto.find(");").map_or(resto.len(), |f| f + 2);
            return Some(resto[..fim].to_owned());
        }
    }
    None
}

/// Uma das tabelas declara chave estrangeira para a outra.
fn tem_chave_estrangeira(raiz: &Path, a: &str, b: &str) -> Option<bool> {
    let ba = bloco_da_tabela(raiz, a)?;
    let bb = bloco_da_tabela(raiz, b)?;
    let referencia = |t: &str| format!("REFERENCES {} ", t.to_uppercase());
    Some(ba.contains(&referencia(b)) || bb.contains(&referencia(a)))
}

fn conferir_modelagem(c: &mut Conferencia, raiz: &Path, f: &Funcionalidade) {
    let onde = format!("funcionalidade {}", f.id);
    if let Some(p) = &f.processo {
        if p.raias.is_empty() || p.principal.is_empty() {
            c.erro(format!(
                "{onde}: processo sem raias ou sem passos do principal"
            ));
        }
        for item in p.raias.iter().chain(&p.principal).chain(&p.decisoes) {
            let onde = format!("{onde}, processo {}", item.id);
            c.texto_preenchido(&onde, "descricao", &item.descricao);
            c.palavras_chave(&onde, &item.palavras_chave);
            for ev in &item.evidencias {
                c.evidencia(&onde, ev);
            }
        }
        for item in p.principal.iter().chain(&p.decisoes) {
            if item.evidencias.is_empty() {
                c.erro(format!("{onde}, processo {}: sem evidência", item.id));
            }
        }
    }
    if let Some(m) = &f.modelo_dados {
        let mut tabelas: Vec<&str> = Vec::new();
        for e in &m.entidades {
            let onde = format!("{onde}, entidade {}", e.id);
            c.palavras_chave(&onde, &e.palavras_chave);
            if bloco_da_tabela(raiz, &e.tabela).is_none() {
                c.erro(format!(
                    "{onde}: tabela `{}` não está nos scripts",
                    e.tabela
                ));
            }
            tabelas.push(&e.tabela);
        }
        for r in &m.relacoes {
            let onde = format!("{onde}, relação {}", r.id);
            for ev in &r.evidencias {
                c.evidencia(&onde, ev);
            }
            if r.evidencias.is_empty() {
                c.erro(format!("{onde}: sem evidência"));
            }
            if !r.opcional
                && !(tabelas.contains(&r.de.as_str()) && tabelas.contains(&r.para.as_str()))
            {
                c.erro(format!(
                    "{onde}: relação obrigatória entre tabelas que não estão em `entidades`"
                ));
            }
            match (
                r.origem.as_str(),
                tem_chave_estrangeira(raiz, &r.de, &r.para),
            ) {
                (_, None) => c.erro(format!(
                    "{onde}: `{}` ou `{}` não está nos scripts",
                    r.de, r.para
                )),
                ("chave_estrangeira", Some(true)) | ("join", Some(false)) => {}
                ("chave_estrangeira", Some(false)) => c.erro(format!(
                    "{onde}: origem chave_estrangeira, mas os scripts não declaram a chave"
                )),
                ("join", Some(true)) => c.erro(format!(
                    "{onde}: origem join, mas os scripts declaram a chave estrangeira"
                )),
                (outra, _) => c.erro(format!(
                    "{onde}: origem `{outra}` deve ser chave_estrangeira ou join"
                )),
            }
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Demanda {
    id: String,
    texto: String,
    premissa: String,
    abrangencia_esperada: String,
    #[serde(default)]
    verificacao_da_premissa: Option<VerificacaoDaPremissa>,
    regras: Vec<String>,
    arquivos_impactados: Vec<String>,
    #[serde(default)]
    perguntas_esperadas: Vec<PerguntaEsperada>,
    #[serde(default)]
    dados_pessoais: Option<DadosPessoais>,
    #[serde(default)]
    origem: Option<OrigemDaRegra>,
    #[serde(default)]
    caminhos_de_escrita: Option<CaminhosDeEscrita>,
}

/// Todos os caminhos que gravam um estado (#451): cada um com palavras-chave e
/// evidência; ao menos um não valida e diz, em `sem_validacao`, como reconhecer
/// a afirmação que aponta isso.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CaminhosDeEscrita {
    estado: String,
    caminhos: Vec<CaminhoDeEscrita>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CaminhoDeEscrita {
    nome: String,
    valida: bool,
    palavras_chave: Vec<Vec<String>>,
    evidencias: Vec<Evidencia>,
    #[serde(default)]
    sem_validacao: Vec<Vec<String>>,
}

/// Regra criada de propósito por uma demanda anterior, com o histórico git
/// sintético em `fixtures/historicos/<historico>/` (#450). A reprodução do
/// histórico é conferida pela crate `cba-avaliacao`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OrigemDaRegra {
    historico: String,
    chave: String,
    regra: String,
}

/// Pergunta que o agente deve fazer ao solicitante (avaliação dos fluxos, #33).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PerguntaEsperada {
    tema: String,
    palavras_chave: Vec<Vec<String>>,
}

/// Como o agente deve julgar a suposição do solicitante (avaliação dos fluxos, #33).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct VerificacaoDaPremissa {
    veredito: String,
    palavras_chave: Vec<Vec<String>>,
}

/// Seção "Dados pessoais e requisitos não funcionais" esperada (#126).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DadosPessoais {
    campos: Vec<CampoPessoal>,
    #[serde(default)]
    nao_pessoais: Vec<CampoNaoPessoal>,
    #[serde(default)]
    aspectos: Vec<AspectoEsperado>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CampoPessoal {
    campo: String,
    categoria: String,
    palavras_chave: Vec<Vec<String>>,
    evidencias: Vec<Evidencia>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CampoNaoPessoal {
    campo: String,
    palavras_chave: Vec<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AspectoEsperado {
    aspecto: String,
    tema: String,
    palavras_chave: Vec<Vec<String>>,
}

/// A coluna `TABELA.COLUNA` está no `CREATE TABLE` da tabela nos scripts.
fn coluna_nos_scripts(raiz: &Path, campo: &str) -> bool {
    let Some((tabela, coluna)) = campo.split_once('.') else {
        return false;
    };
    bloco_da_tabela(raiz, tabela).is_some_and(|bloco| {
        bloco
            .lines()
            .any(|l| l.split_whitespace().next() == Some(coluna))
    })
}

/// Caminhos que gravam um estado (#451): ao menos dois (o objetivo é fechar o
/// conjunto), nomes únicos, palavras-chave, evidências que conferem e, nos que
/// não validam, as palavras-chave do que falta validar; ao menos um não valida.
fn conferir_caminhos(c: &mut Conferencia, onde: &str, ce: &CaminhosDeEscrita) {
    let onde = format!("{onde}, caminhos de escrita");
    c.texto_preenchido(&onde, "estado", &ce.estado);
    if ce.caminhos.len() < 2 {
        c.erro(format!("{onde}: menos de dois caminhos"));
    }
    if ce.caminhos.iter().all(|k| k.valida) {
        c.erro(format!("{onde}: nenhum caminho sem validação"));
    }
    let mut nomes = HashSet::new();
    for k in &ce.caminhos {
        let onde = format!("{onde}, {}", k.nome);
        c.texto_preenchido(&onde, "nome", &k.nome);
        if !nomes.insert(k.nome.as_str()) {
            c.erro(format!("{onde}: caminho repetido"));
        }
        c.palavras_chave(&onde, &k.palavras_chave);
        if k.evidencias.is_empty() {
            c.erro(format!("{onde}: sem evidências"));
        }
        for ev in &k.evidencias {
            c.evidencia(&onde, ev);
        }
        if k.valida {
            if !k.sem_validacao.is_empty() {
                c.erro(format!("{onde}: `sem_validacao` num caminho que valida"));
            }
        } else if k.sem_validacao.is_empty() {
            c.erro(format!("{onde}: caminho sem validação sem `sem_validacao`"));
        } else {
            c.palavras_chave(&onde, &k.sem_validacao);
        }
    }
}

fn conferir_dados_pessoais(c: &mut Conferencia, raiz: &Path, onde: &str, d: &DadosPessoais) {
    if d.campos.is_empty() {
        c.erro(format!("{onde}: dados_pessoais sem campos"));
    }
    let mut vistos = HashSet::new();
    for campo in &d.campos {
        let onde = format!("{onde}, campo {}", campo.campo);
        if !vistos.insert(campo.campo.as_str()) {
            c.erro(format!("{onde}: campo repetido"));
        }
        if !coluna_nos_scripts(raiz, &campo.campo) {
            c.erro(format!("{onde}: coluna não está nos scripts"));
        }
        c.texto_preenchido(&onde, "categoria", &campo.categoria);
        c.palavras_chave(&onde, &campo.palavras_chave);
        if campo.evidencias.is_empty() {
            c.erro(format!("{onde}: sem evidências"));
        }
        for ev in &campo.evidencias {
            c.evidencia(&onde, ev);
        }
    }
    for campo in &d.nao_pessoais {
        let onde = format!("{onde}, não pessoal {}", campo.campo);
        if !vistos.insert(campo.campo.as_str()) {
            c.erro(format!("{onde}: campo repetido ou também pessoal"));
        }
        if !coluna_nos_scripts(raiz, &campo.campo) {
            c.erro(format!("{onde}: coluna não está nos scripts"));
        }
        c.palavras_chave(&onde, &campo.palavras_chave);
    }
    for a in &d.aspectos {
        let onde = format!("{onde}, aspecto {}", a.aspecto);
        if !["dados_pessoais", "auditoria", "desempenho", "acesso"].contains(&a.aspecto.as_str()) {
            c.erro(format!(
                "{onde}: aspecto deve ser dados_pessoais, auditoria, desempenho ou acesso"
            ));
        }
        c.texto_preenchido(&onde, "tema", &a.tema);
        c.palavras_chave(&onde, &a.palavras_chave);
    }
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
    /// Grupos de palavras-chave: ao menos um grupo, nenhum grupo nem termo vazio.
    fn palavras_chave(&mut self, onde: &str, grupos: &[Vec<String>]) {
        if grupos.is_empty() {
            self.erro(format!("{onde}: palavras_chave sem nenhum grupo"));
        }
        for grupo in grupos {
            if grupo.is_empty()
                || grupo
                    .iter()
                    .any(|t| t.trim().trim_end_matches('*').is_empty())
            {
                self.erro(format!(
                    "{onde}: grupo de palavras_chave vazio ou com termo vazio"
                ));
            }
        }
    }

    fn perguntas_esperadas(&mut self, onde: &str, perguntas: &[PerguntaEsperada]) {
        for p in perguntas {
            self.texto_preenchido(onde, "perguntas_esperadas.tema", &p.tema);
            self.palavras_chave(&format!("{onde}, pergunta esperada"), &p.palavras_chave);
        }
    }

    fn arquivo(&mut self, onde: &str, relativo: &str) -> Option<PathBuf> {
        let caminho = Path::new(relativo);
        let so_normais = caminho
            .components()
            .all(|c| matches!(c, Component::Normal(_)));
        if relativo.is_empty() || relativo.contains('\\') || !so_normais {
            self.erro(format!(
                "{onde}: caminho `{relativo}` deve ser relativo à raiz do projeto, \
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

    c.texto_preenchido("gabarito", "projeto", &gabarito.projeto);
    if !raiz.is_dir() {
        c.erro(format!("raiz `{}` não é uma pasta", gabarito.raiz));
        return c.erros;
    }
    // As respostas não podem morar dentro do sistema analisado.
    let raiz_canonica = raiz.canonicalize().expect("raiz canônica");
    let gabarito_canonico = caminho.canonicalize().expect("gabarito canônico");
    if gabarito_canonico.starts_with(&raiz_canonica) {
        c.erro("o gabarito não pode ficar dentro da raiz do projeto".to_string());
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
        .chain(gabarito.demandas.iter().map(|d| &d.id))
        .chain(gabarito.reunioes.iter().map(|r| &r.id))
        .chain(
            gabarito
                .reunioes
                .iter()
                .flat_map(|r| r.itens.iter().map(|i| &i.id)),
        )
        .chain(gabarito.historias.iter().map(|h| &h.id))
        .chain(gabarito.cenarios.iter().map(|c| &c.id))
        .chain(gabarito.ciclos_vida.iter().map(|c| &c.id))
        .chain(gabarito.mapeamentos.iter().map(|m| &m.id))
        .chain(gabarito.publicacoes.iter().map(|p| &p.id))
        .chain(
            gabarito
                .ciclos_vida
                .iter()
                .flat_map(|c| c.transicoes.iter().map(|t| &t.id)),
        )
        .chain(gabarito.cenarios.iter().flat_map(|c| {
            c.limites
                .iter()
                .map(|l| &l.id)
                .chain(c.tabelas.iter().map(|t| &t.id))
                .chain(c.negativos.iter().map(|n| &n.id))
        }))
        .chain(
            gabarito
                .historias
                .iter()
                .flat_map(|h| h.historias_esperadas.iter().map(|e| &e.id)),
        )
        .chain(gabarito.revisoes.iter().map(|r| &r.id))
        .chain(
            gabarito
                .revisoes
                .iter()
                .flat_map(|r| r.achados.iter().map(|a| &a.id)),
        );
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
        c.perguntas_esperadas(&onde, &p.perguntas_esperadas);
    }

    for f in &gabarito.funcionalidades {
        let onde = format!("funcionalidade {}", f.id);
        c.texto_preenchido(&onde, "nome", &f.nome);
        c.texto_preenchido(&onde, "objetivo", &f.objetivo);
        citar(&mut c, &onde, &f.regras);
        c.perguntas_esperadas(&onde, &f.perguntas_esperadas);
        if f.arquivos.is_empty() {
            c.erro(format!("{onde}: nenhum arquivo"));
        }
        for arquivo in &f.arquivos {
            c.arquivo(&onde, arquivo);
        }
        conferir_modelagem(&mut c, &raiz, f);
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
        c.perguntas_esperadas(&onde, &d.perguntas_esperadas);
        if let Some(v) = &d.verificacao_da_premissa {
            if !["nao_confere", "confere_em_parte"].contains(&v.veredito.as_str()) {
                c.erro(format!(
                    "{onde}: veredito `{}` deve ser nao_confere ou confere_em_parte",
                    v.veredito
                ));
            }
            c.palavras_chave(
                &format!("{onde}, verificação da premissa"),
                &v.palavras_chave,
            );
        }
        if let Some(dp) = &d.dados_pessoais {
            conferir_dados_pessoais(&mut c, &raiz, &onde, dp);
        }
        if let Some(o) = &d.origem {
            c.texto_preenchido(&onde, "origem.chave", &o.chave);
            if !d.regras.contains(&o.regra) {
                c.erro(format!(
                    "{onde}: a regra de origem {} não está nas regras da demanda",
                    o.regra
                ));
            }
            let historico = pasta_gabarito
                .join("../historicos")
                .join(&o.historico)
                .join("historico.yaml");
            if !historico.is_file() {
                c.erro(format!(
                    "{onde}: o histórico {} não existe",
                    historico.display()
                ));
            }
        }
        if let Some(ce) = &d.caminhos_de_escrita {
            conferir_caminhos(&mut c, &onde, ce);
        }
        if d.arquivos_impactados.is_empty() {
            c.erro(format!("{onde}: nenhum arquivo impactado"));
        }
        for arquivo in &d.arquivos_impactados {
            c.arquivo(&onde, arquivo);
        }
    }

    for r in &gabarito.reunioes {
        conferir_reuniao(&mut c, pasta_gabarito, &raiz_canonica, r);
    }

    for r in &gabarito.revisoes {
        let onde = format!("revisão {}", r.id);
        c.texto_preenchido(&onde, "titulo", &r.titulo);
        c.texto_preenchido(&onde, "texto", &r.texto);
        if !["pronta", "pronta_com_ressalvas", "precisa_de_retrabalho"]
            .contains(&r.veredito.as_str())
        {
            c.erro(format!(
                "{onde}: veredito `{}` deve ser pronta, pronta_com_ressalvas ou precisa_de_retrabalho",
                r.veredito
            ));
        }
        // Os termos vagos esperados são os que a lista de referência acha no texto.
        let achados_no_texto: Vec<&str> = cba_core::qualidade::termos_vagos(&r.texto)
            .into_iter()
            .map(|o| o.termo)
            .collect();
        for termo in &r.termos_vagos {
            if !achados_no_texto.contains(&termo.as_str()) {
                c.erro(format!(
                    "{onde}: termo vago `{termo}` não está na lista de referência ou no texto"
                ));
            }
        }
        for termo in &achados_no_texto {
            if !r.termos_vagos.iter().any(|t| t == termo) {
                c.erro(format!(
                    "{onde}: o texto tem o termo vago `{termo}`, que falta em termos_vagos"
                ));
            }
        }
        if r.achados.is_empty() {
            c.erro(format!("{onde}: nenhum achado esperado"));
        }
        for a in &r.achados {
            let onde = format!("{onde}, achado {}", a.id);
            c.texto_preenchido(&onde, "descricao", &a.descricao);
            for car in std::iter::once(&a.caracteristica).chain(&a.tambem_em) {
                if !CARACTERISTICAS.contains(&car.as_str()) {
                    c.erro(format!("{onde}: característica `{car}` desconhecida"));
                }
            }
            if a.caracteristica == "correcao" && a.regras.is_empty() {
                c.erro(format!(
                    "{onde}: a correção esperada cita a regra contrariada"
                ));
            }
            for id in &a.regras {
                if let Some(valida) = regras_validas.get(id.as_str()) {
                    citadas_nos_fluxos.insert(*valida);
                } else {
                    c.erro(format!("{onde}: regra `{id}` não existe"));
                }
            }
            c.palavras_chave(&onde, &a.palavras_chave);
        }
        c.perguntas_esperadas(&onde, &r.perguntas_esperadas);
    }

    for h in &gabarito.historias {
        let onde = format!("histórias {}", h.id);
        c.texto_preenchido(&onde, "titulo", &h.titulo);
        if let Some(p) = &h.pedido {
            c.texto_preenchido(&onde, "pedido", p);
        }
        // A origem é a análise de impacto da demanda, preparada com as regras dela:
        // as regras esperadas nos critérios são regras (RN-…) dessa demanda.
        match gabarito.demandas.iter().find(|d| d.id == h.demanda) {
            None => c.erro(format!("{onde}: demanda `{}` não existe", h.demanda)),
            Some(d) => {
                for id in &h.regras {
                    if !id.starts_with("RN-") || !d.regras.contains(id) {
                        c.erro(format!(
                            "{onde}: regra `{id}` não é uma regra (RN-…) da demanda {}",
                            d.id
                        ));
                    }
                }
                if h.quebra && d.abrangencia_esperada == "pequena" {
                    c.erro(format!(
                        "{onde}: quebra esperada numa demanda de abrangência pequena"
                    ));
                }
            }
        }
        if h.historias_esperadas.is_empty() {
            c.erro(format!("{onde}: nenhuma história esperada"));
        }
        for e in &h.historias_esperadas {
            let onde = format!("{onde}, {}", e.id);
            c.texto_preenchido(&onde, "descricao", &e.descricao);
            c.palavras_chave(&onde, &e.palavras_chave);
        }
        c.perguntas_esperadas(&onde, &h.a_confirmar);
    }

    for cen in &gabarito.cenarios {
        let onde = format!("cenários {}", cen.id);
        c.texto_preenchido(&onde, "titulo", &cen.titulo);
        c.texto_preenchido(&onde, "pedido", &cen.pedido);
        if cen.regras.is_empty() {
            c.erro(format!("{onde}: nenhuma regra"));
        }
        for id in &cen.regras {
            match gabarito.regras.iter().find(|r| &r.id == id) {
                None => c.erro(format!("{onde}: regra `{id}` não existe")),
                Some(r) => {
                    citadas_nos_fluxos.insert(r.id.as_str());
                }
            }
            if !cen.pedido.contains(id.as_str()) {
                c.erro(format!("{onde}: o pedido não cita a regra {id}"));
            }
        }
        let da_lista = |c: &mut Conferencia, onde: &str, regra: &str| {
            if !cen.regras.iter().any(|r| r == regra) {
                c.erro(format!("{onde}: regra `{regra}` fora das regras do item"));
            }
        };
        if cen.limites.is_empty() {
            c.erro(format!("{onde}: nenhum limite esperado"));
        }
        for l in &cen.limites {
            let onde = format!("{onde}, {}", l.id);
            da_lista(&mut c, &onde, &l.regra);
            c.texto_preenchido(&onde, "grandeza", &l.grandeza);
            c.evidencia(&onde, &l.evidencia);
            // O limite é o do código: o valor aparece no trecho citado.
            if !numeros_do_trecho(&l.evidencia.trecho)
                .iter()
                .any(|n| (n - l.valor).abs() < 1e-9)
            {
                c.erro(format!(
                    "{onde}: o valor {} não aparece no trecho `{}`",
                    l.valor, l.evidencia.trecho
                ));
            }
            // E está numa evidência da regra.
            if let Some(r) = gabarito.regras.iter().find(|r| r.id == l.regra)
                && !r.evidencias.iter().any(|e| {
                    e.arquivo == l.evidencia.arquivo
                        && e.linha_inicial <= l.evidencia.linha_final
                        && l.evidencia.linha_inicial <= e.linha_final
                })
            {
                c.erro(format!(
                    "{onde}: a evidência do limite não toca as evidências da regra {}",
                    l.regra
                ));
            }
            if !l
                .valores_testados
                .iter()
                .any(|v| (v - l.valor).abs() < 1e-9)
                || l.valores_testados.len() < 2
            {
                c.erro(format!(
                    "{onde}: valores_testados precisam do limite e do vizinho do outro lado"
                ));
            }
        }
        for t in &cen.tabelas {
            let onde = format!("{onde}, {}", t.id);
            da_lista(&mut c, &onde, &t.regra);
            c.texto_preenchido(&onde, "descricao", &t.descricao);
            if t.condicoes < 2 {
                c.erro(format!(
                    "{onde}: tabela de decisão com menos de 2 condições"
                ));
            }
            c.palavras_chave(&onde, &t.palavras_chave);
        }
        for n in &cen.negativos {
            let onde = format!("{onde}, {}", n.id);
            da_lista(&mut c, &onde, &n.regra);
            c.texto_preenchido(&onde, "descricao", &n.descricao);
            c.palavras_chave(&onde, &n.palavras_chave);
        }
        c.perguntas_esperadas(&onde, &cen.a_confirmar);
    }

    for cv in &gabarito.ciclos_vida {
        conferir_ciclo_vida(&mut c, cv);
    }

    let do_explicar: BTreeSet<&str> = gabarito
        .funcionalidades
        .iter()
        .map(|f| f.id.as_str())
        .collect();
    for m in &gabarito.mapeamentos {
        conferir_mapeamento(&mut c, &raiz, m, &do_explicar);
    }
    for p in &gabarito.publicacoes {
        conferir_publicacao(&mut c, &gabarito, p);
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

/// Ciclo de publicação (#279): os dois itens são de análise (pergunta,
/// funcionalidade ou demanda) e diferentes, o repositório é `dono/repo` e a
/// subpasta é relativa.
fn conferir_publicacao(c: &mut Conferencia, gabarito: &Gabarito, p: &Publicacao) {
    let onde = format!("publicação {}", p.id);
    c.texto_preenchido(&onde, "titulo", &p.titulo);
    let de_analise = |id: &str| {
        gabarito.perguntas.iter().any(|x| x.id == id)
            || gabarito.funcionalidades.iter().any(|x| x.id == id)
            || gabarito.demandas.iter().any(|x| x.id == id)
    };
    for (campo, id) in [("analise", &p.analise), ("reanalise", &p.reanalise)] {
        if !de_analise(id) {
            c.erro(format!(
                "{onde}: `{campo}` {id} não é pergunta, funcionalidade nem demanda"
            ));
        }
    }
    if p.analise == p.reanalise {
        c.erro(format!("{onde}: `analise` e `reanalise` são o mesmo item"));
    }
    let partes: Vec<&str> = p.repositorio.split('/').collect();
    if partes.len() != 2 || partes.iter().any(|x| x.trim().is_empty()) {
        c.erro(format!("{onde}: `repositorio` precisa ser dono/repo"));
    }
    if p.subpasta.starts_with('/') || p.subpasta.split('/').any(|x| x == "..") {
        c.erro(format!("{onde}: `subpasta` precisa ser relativa, sem `..`"));
    }
}

/// Mapeamento (#208): cada ponto de entrada existe na fixture (o arquivo e o
/// nome) e aponta para funcionalidades do mapeamento; cada funcionalidade tem um
/// ponto, área e palavras-chave e, se não é do fluxo explicar, nome.
fn conferir_mapeamento(
    c: &mut Conferencia,
    raiz: &Path,
    m: &Mapeamento,
    do_explicar: &BTreeSet<&str>,
) {
    let onde = format!("mapeamento {}", m.id);
    c.texto_preenchido(&onde, "titulo", &m.titulo);
    let ids: BTreeSet<&str> = m.funcionalidades.iter().map(|f| f.id.as_str()).collect();
    if ids.len() != m.funcionalidades.len() {
        c.erro(format!("{onde}: funcionalidade repetida"));
    }
    let mut chaves = BTreeSet::new();
    for p in &m.pontos_de_entrada {
        let onde = format!("{onde}, ponto {}", p.chave);
        if !chaves.insert(p.chave.as_str()) {
            c.erro(format!("{onde}: ponto repetido"));
        }
        let Some((arquivo, nome)) = p.chave.split_once('#') else {
            c.erro(format!("{onde}: chave fora do formato <arquivo>#<nome>"));
            continue;
        };
        // Endpoint: o método seguido da rota normalizada (`Classe.metodo./api/x/{}`);
        // confere-se o método.
        let nome = nome.split_once("./").map_or(nome, |(metodo, _)| metodo);
        let simples = nome.rsplit('.').next().unwrap_or(nome).to_uppercase();
        match fs::read(raiz.join(arquivo)) {
            Ok(bytes) => {
                if !String::from_utf8_lossy(&bytes)
                    .to_uppercase()
                    .contains(&simples)
                {
                    c.erro(format!("{onde}: `{simples}` não aparece em `{arquivo}`"));
                }
            }
            Err(_) => c.erro(format!("{onde}: arquivo `{arquivo}` não existe na fixture")),
        }
        if p.funcionalidades.is_empty() {
            c.erro(format!("{onde}: sem funcionalidade esperada"));
        }
        for f in &p.funcionalidades {
            if !ids.contains(f.as_str()) {
                c.erro(format!("{onde}: funcionalidade {f} fora do mapeamento"));
            }
        }
    }
    for f in &m.funcionalidades {
        let onde = format!("{onde}, {}", f.id);
        c.texto_preenchido(&onde, "area", &f.area);
        c.palavras_chave(&onde, &f.palavras_chave);
        if !do_explicar.contains(f.id.as_str())
            && f.nome.as_deref().is_none_or(|n| n.trim().is_empty())
        {
            c.erro(format!("{onde}: funcionalidade nova sem `nome`"));
        }
        if !m
            .pontos_de_entrada
            .iter()
            .any(|p| p.funcionalidades.iter().any(|x| x == &f.id))
        {
            c.erro(format!("{onde}: nenhum ponto de entrada esperado"));
        }
    }
}

/// Ciclo de vida (#119): estados únicos com um inicial, transições entre estados
/// declarados com evidência que grava o valor de destino, e os achados iguais aos
/// calculados das transições (inalcançável e sem saída).
fn conferir_ciclo_vida(c: &mut Conferencia, cv: &CicloVida) {
    let onde = format!("ciclo de vida {}", cv.id);
    c.texto_preenchido(&onde, "titulo", &cv.titulo);
    c.texto_preenchido(&onde, "entidade", &cv.entidade);
    c.texto_preenchido(&onde, "pedido", &cv.pedido);
    c.evidencia(&format!("{onde}, campo"), &cv.campo);
    let valores: Vec<&str> = cv.estados.iter().map(|e| e.valor.as_str()).collect();
    let unicos: HashSet<&str> = valores.iter().copied().collect();
    if unicos.len() != valores.len() {
        c.erro(format!("{onde}: estado repetido"));
    }
    if cv.estados.iter().filter(|e| e.inicial).count() != 1 {
        c.erro(format!("{onde}: precisa de exatamente um estado inicial"));
    }
    for e in &cv.estados {
        c.texto_preenchido(&onde, "estados.valor", &e.valor);
        if e.inicial && e.r#final {
            c.erro(format!("{onde}: {} é inicial e final", e.valor));
        }
        if let Some(ev) = &e.evidencia {
            c.evidencia(&format!("{onde}, estado {}", e.valor), ev);
        }
    }
    for t in &cv.transicoes {
        let onde_t = format!("{onde}, {}", t.id);
        for ponta in [&t.de, &t.para] {
            if !unicos.contains(ponta.as_str()) {
                c.erro(format!("{onde_t}: estado `{ponta}` não declarado"));
            }
        }
        c.palavras_chave(&onde_t, &t.palavras_chave);
        c.evidencia(&onde_t, &t.evidencia);
        if !t.evidencia.trecho.contains(t.para.as_str())
            && !t.evidencia.trecho.contains(t.de.as_str())
        {
            c.erro(format!(
                "{onde_t}: o trecho deve citar o valor gravado ({} ou {})",
                t.para, t.de
            ));
        }
    }
    // Os achados esperados são exatamente os que as transições implicam.
    let mut calculados: Vec<(String, String)> = Vec::new();
    for e in &cv.estados {
        let chega = cv.transicoes.iter().any(|t| t.para == e.valor);
        let sai = cv.transicoes.iter().any(|t| t.de == e.valor);
        if !e.inicial && !chega {
            calculados.push(("inalcancavel".into(), e.valor.clone()));
        }
        if !e.r#final && !sai && chega {
            calculados.push(("sem_saida".into(), e.valor.clone()));
        }
    }
    let mut esperados: Vec<(String, String)> = cv
        .achados
        .iter()
        .map(|a| (a.tipo.clone(), a.estado.clone()))
        .collect();
    calculados.sort();
    esperados.sort();
    if calculados != esperados {
        c.erro(format!(
            "{onde}: achados {esperados:?} diferem dos calculados das transições {calculados:?}"
        ));
    }
}

/// Início `M:SS` ou `H:MM:SS` em segundos.
fn segundos(horario: &str) -> Option<u64> {
    let partes: Vec<&str> = horario.split(':').collect();
    let numero = |s: &str| {
        (!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
            .then(|| s.parse::<u64>().ok())
            .flatten()
    };
    match partes.as_slice() {
        [m, s] if s.len() == 2 => Some(numero(m)? * 60 + numero(s)?),
        [h, m, s] if m.len() == 2 && s.len() == 2 => {
            Some(numero(h)? * 3600 + numero(m)? * 60 + numero(s)?)
        }
        _ => None,
    }
}

/// Palavras seguidas da fala que bastam para dizer que um texto a copia.
const PALAVRAS_COPIADAS: usize = 5;

fn palavras(texto: &str) -> Vec<String> {
    texto
        .split(|c: char| !c.is_alphanumeric())
        .filter(|p| !p.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// Primeira sequência de [`PALAVRAS_COPIADAS`] palavras de alguma fala que aparece
/// no texto.
fn copia_de_fala(texto: &str, falas: &[cba_reunioes::Fala]) -> Option<String> {
    let texto = format!(" {} ", palavras(texto).join(" "));
    falas.iter().find_map(|f| {
        palavras(&f.texto)
            .windows(PALAVRAS_COPIADAS)
            .map(|j| j.join(" "))
            .find(|s| texto.contains(&format!(" {s} ")))
    })
}

fn conferir_reuniao(c: &mut Conferencia, pasta_gabarito: &Path, raiz: &Path, r: &Reuniao) {
    let onde = format!("reunião {}", r.id);
    c.texto_preenchido(&onde, "titulo", &r.titulo);
    c.texto_preenchido(&onde, "demanda", &r.demanda);
    let caminho = r
        .transcript
        .split('/')
        .fold(pasta_gabarito.to_path_buf(), |acc, parte| acc.join(parte));
    let Ok(canonico) = caminho.canonicalize() else {
        c.erro(format!("{onde}: transcript `{}` não existe", r.transcript));
        return;
    };
    // O transcript não pode ficar dentro do projeto analisado.
    if canonico.starts_with(raiz) {
        c.erro(format!(
            "{onde}: o transcript não pode ficar dentro da raiz do projeto"
        ));
    }
    let nome = canonico
        .file_name()
        .map(|n| n.to_string_lossy().into_owned());
    let transcript = cba_reunioes::analisar(
        &fs::read(&canonico).expect("transcript legível"),
        nome.as_deref(),
    );
    if transcript.falas.is_empty() || !transcript.avisos.is_empty() {
        c.erro(format!(
            "{onde}: o transcript não foi lido sem avisos ({:?})",
            transcript.avisos
        ));
        return;
    }

    let falantes = transcript.falantes();
    for p in &r.participantes {
        c.texto_preenchido(&onde, "participantes.papel", &p.papel);
        if !falantes.contains(&p.nome.as_str()) {
            c.erro(format!(
                "{onde}: participante `{}` não é falante do transcript",
                p.nome
            ));
        }
        if p.tambem_aceitos.iter().any(|a| a.trim().is_empty()) {
            c.erro(format!("{onde}: papel aceito vazio para `{}`", p.nome));
        }
    }
    for f in &falantes {
        if !r.participantes.iter().any(|p| p.nome == *f) {
            c.erro(format!("{onde}: falante `{f}` sem papel esperado"));
        }
    }

    if r.itens.is_empty() {
        c.erro(format!("{onde}: nenhum item esperado"));
    }
    for item in &r.itens {
        let onde = format!("{onde}, item {}", item.id);
        c.texto_preenchido(&onde, "descricao", &item.descricao);
        // A descrição vai para o relatório da avaliação, que não leva texto de
        // fala: ela é escrita com outras palavras, sem copiar a fala.
        if let Some(copiado) = copia_de_fala(&item.descricao, &transcript.falas) {
            c.erro(format!(
                "{onde}: a descrição copia uma fala (`{copiado}`); reescreva com outras palavras"
            ));
        }
        for tipo in std::iter::once(&item.tipo).chain(&item.tambem_em) {
            if !TIPOS_DE_ITEM.contains(&tipo.as_str()) {
                c.erro(format!(
                    "{onde}: tipo `{tipo}` deve ser um de {}",
                    TIPOS_DE_ITEM.join(", ")
                ));
            }
        }
        c.palavras_chave(&onde, &item.palavras_chave);
        let pendencia = item.tipo == "pendencia";
        if !pendencia && (!item.responsavel.is_empty() || !item.prazo.is_empty()) {
            c.erro(format!(
                "{onde}: responsável e prazo só valem em pendências"
            ));
        }
        if !item.responsavel.is_empty() {
            c.palavras_chave(&format!("{onde}, responsável"), &item.responsavel);
        }
        if !item.prazo.is_empty() {
            c.palavras_chave(&format!("{onde}, prazo"), &item.prazo);
        }
        if item.falas.is_empty() {
            c.erro(format!("{onde}: nenhuma fala de origem"));
        }
        for horario in &item.falas {
            let existe = segundos(horario).is_some_and(|s| {
                transcript
                    .falas
                    .iter()
                    .any(|f| f.inicio.is_some_and(|i| i.segundos() == s))
            });
            if !existe {
                c.erro(format!(
                    "{onde}: nenhuma fala começa em `{horario}` no transcript"
                ));
            }
        }
    }
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
