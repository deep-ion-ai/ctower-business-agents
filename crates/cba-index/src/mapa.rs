//! Mapa estrutural do projeto: símbolos de cada arquivo, referências entre eles e
//! agrupamento por pasta (módulos).
//!
//! O mapa guarda, por arquivo, o que o extrator achou (símbolos e referências pelo
//! nome, sem resolver). Ao carregar, monta um índice em memória que liga cada
//! referência aos símbolos de todo o projeto. Assim o que se grava depende só do
//! arquivo e do extrator, e o mapeamento é incremental: arquivo com o mesmo hash,
//! extraído pelo mesmo extrator na mesma versão, não é lido de novo.
//!
//! As consultas servem às ferramentas `mapa` ([`Mapa::modulo`]), `estrutura`
//! ([`Mapa::arquivo`], [`Mapa::filhos`]), `schema` ([`Mapa::schema`]) e
//! `referencias` ([`Mapa::usos`], [`Mapa::referencias_a`]).
//!
//! ## Resolução das referências
//!
//! Sem diferenciar maiúsculas (SQL, COBOL, Pascal e VB não diferenciam), o nome
//! usado casa com o nome qualificado de um símbolo, com o fim dele (`FN_X` casa com
//! `PKG.FN_X`; colunas só pelo nome qualificado) ou, se tiver esquema
//! (`APP.FATURA`), com o último segmento. Entre os candidatos, a implementação
//! ganha da declaração, a mesma grafia ganha das outras (a tabela `FATURA` e a
//! classe `Fatura` convivem) e o mesmo arquivo ganha dos outros. Menções
//! ([`TipoReferencia::Mencao`]) que não casam com nada são descartadas, e as que
//! não casam tentam de novo sem o último segmento (`SQ_X.NEXTVAL` → `SQ_X`). As
//! demais referências sem alvo ficam no mapa (tabela sem DDL, rotina externa).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::extrator::{
    Extracao, Fonte, Referencia, Registro, Simbolo, TipoReferencia, TipoSimbolo,
};
use crate::inventario::{ArquivoInventariado, Inventario};
use crate::linguagem::Linguagem;
use crate::projeto::Escopo;

/// Versão do formato gravado. Mapa de outro formato é refeito do zero.
pub const FORMATO_MAPA: u32 = 1;

/// Arquivo maior que isto não é extraído (fica registrado como falha).
pub const LIMITE_EXTRACAO: u64 = 16 * 1024 * 1024;

/// Extração de um arquivo, como fica gravada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArquivoMapeado {
    /// Caminho relativo à raiz do projeto, com `/`.
    pub caminho: String,
    pub linguagem: Linguagem,
    /// Hash do inventário quando foi extraído.
    pub hash: String,
    /// Extrator usado e a versão dele. Vazio quando o arquivo nem chegou ao
    /// extrator (erro de leitura): será tentado de novo.
    pub extrator: String,
    pub versao: u32,
    #[serde(flatten)]
    pub extracao: Extracao,
    /// Erro do extrator ou da leitura. Os símbolos ficam vazios.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub falha: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EstatisticasMapa {
    /// Arquivos extraídos nesta execução.
    pub processados: usize,
    /// Arquivos reaproveitados do mapa anterior.
    pub reaproveitados: usize,
    /// Arquivos com falha (leitura ou extrator).
    pub falhas: usize,
    pub duracao_ms: u64,
}

/// Identificador de um símbolo dentro de um [`Mapa`] carregado. Não é estável
/// entre mapeamentos: para guardar, use arquivo e nome qualificado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SimboloId {
    arquivo: u32,
    indice: u32,
}

/// Símbolo com o arquivo em que está.
#[derive(Debug, Clone, Copy)]
pub struct Achado<'a> {
    pub id: SimboloId,
    pub arquivo: &'a str,
    pub simbolo: &'a Simbolo,
}

/// Referência com o arquivo, o símbolo de origem e os alvos resolvidos.
#[derive(Debug, Clone)]
pub struct Uso<'a> {
    pub arquivo: &'a str,
    pub referencia: &'a Referencia,
    /// Símbolo de onde parte (a rotina que chama), se houver.
    pub origem: Option<Achado<'a>>,
    /// Símbolos a que a referência se liga (vazio quando o alvo é desconhecido;
    /// mais de um quando o nome é ambíguo).
    pub alvos: Vec<SimboloId>,
}

/// Uma pasta do projeto vista como módulo.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Modulo {
    /// Caminho relativo da pasta (`""` é a raiz do projeto).
    pub caminho: String,
    /// Arquivos na pasta e nas subpastas.
    pub arquivos: usize,
    pub linguagens: BTreeMap<Linguagem, usize>,
    pub simbolos: BTreeMap<TipoSimbolo, usize>,
    /// Arquivos com falha de extração.
    pub falhas: usize,
    /// Subpastas diretas.
    pub subpastas: Vec<String>,
    /// Arquivos diretamente na pasta.
    pub arquivos_diretos: Vec<String>,
    /// Referências entre as subpastas diretas (ou entre elas e os arquivos da
    /// própria pasta, que aparecem pelo caminho do módulo).
    pub dependencias: Vec<Dependencia>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Dependencia {
    pub de: String,
    pub para: String,
    pub referencias: usize,
}

/// Mapa estrutural de um projeto.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(from = "MapaGravado")]
pub struct Mapa {
    pub formato: u32,
    pub projeto: String,
    /// Segundos desde 1970 (UTC).
    pub gerado_em: u64,
    pub estatisticas: EstatisticasMapa,
    arquivos: Vec<ArquivoMapeado>,
    #[serde(skip_serializing)]
    indice: Indice,
}

#[derive(Deserialize)]
struct MapaGravado {
    formato: u32,
    projeto: String,
    gerado_em: u64,
    #[serde(default)]
    estatisticas: EstatisticasMapa,
    arquivos: Vec<ArquivoMapeado>,
}

impl From<MapaGravado> for Mapa {
    fn from(g: MapaGravado) -> Self {
        Mapa::novo(
            g.formato,
            g.projeto,
            g.gerado_em,
            g.estatisticas,
            g.arquivos,
        )
    }
}

impl PartialEq for Mapa {
    fn eq(&self, outro: &Self) -> bool {
        self.formato == outro.formato
            && self.projeto == outro.projeto
            && self.gerado_em == outro.gerado_em
            && self.estatisticas == outro.estatisticas
            && self.arquivos == outro.arquivos
    }
}

#[derive(Debug, Clone, Default)]
struct RefResolvida {
    arquivo: u32,
    indice: u32,
    origem: Option<SimboloId>,
    alvos: Vec<SimboloId>,
}

#[derive(Debug, Clone, Default)]
struct Indice {
    por_caminho: HashMap<String, u32>,
    /// Último segmento do nome, em maiúsculas.
    por_nome: HashMap<String, Vec<SimboloId>>,
    filhos: HashMap<SimboloId, Vec<SimboloId>>,
    refs: Vec<RefResolvida>,
    por_alvo: HashMap<SimboloId, Vec<u32>>,
    por_origem: HashMap<SimboloId, Vec<u32>>,
}

impl Mapa {
    fn novo(
        formato: u32,
        projeto: String,
        gerado_em: u64,
        estatisticas: EstatisticasMapa,
        arquivos: Vec<ArquivoMapeado>,
    ) -> Self {
        let mut m = Mapa {
            formato,
            projeto,
            gerado_em,
            estatisticas,
            arquivos,
            indice: Indice::default(),
        };
        m.indexar();
        m
    }

    fn indexar(&mut self) {
        let mut ind = Indice::default();
        for (a, arq) in self.arquivos.iter().enumerate() {
            let a = a as u32;
            ind.por_caminho.insert(arq.caminho.clone(), a);
            let mut por_qualificado: HashMap<&str, SimboloId> = HashMap::new();
            for (s, simb) in arq.extracao.simbolos.iter().enumerate() {
                let id = SimboloId {
                    arquivo: a,
                    indice: s as u32,
                };
                ind.por_nome
                    .entry(simb.nome.to_uppercase())
                    .or_default()
                    .push(id);
                por_qualificado.entry(&simb.qualificado).or_insert(id);
            }
            for (s, simb) in arq.extracao.simbolos.iter().enumerate() {
                if let Some(pai) = simb.pai.as_deref().and_then(|p| por_qualificado.get(p)) {
                    ind.filhos.entry(*pai).or_default().push(SimboloId {
                        arquivo: a,
                        indice: s as u32,
                    });
                }
            }
        }
        self.indice = ind;
        let mut refs = Vec::new();
        for (a, arq) in self.arquivos.iter().enumerate() {
            let a = a as u32;
            for (r, refe) in arq.extracao.referencias.iter().enumerate() {
                let mencao = refe.tipo == TipoReferencia::Mencao;
                let alvos = self.resolver(&refe.alvo, a, mencao);
                if mencao && alvos.is_empty() {
                    continue;
                }
                let origem = refe.origem.as_deref().and_then(|o| {
                    self.local(a, o, refe.linha)
                        .or_else(|| self.resolver(o, a, false).first().copied())
                });
                refs.push(RefResolvida {
                    arquivo: a,
                    indice: r as u32,
                    origem,
                    alvos,
                });
            }
        }
        for (k, r) in refs.iter().enumerate() {
            for alvo in &r.alvos {
                self.indice
                    .por_alvo
                    .entry(*alvo)
                    .or_default()
                    .push(k as u32);
            }
            if let Some(o) = r.origem {
                self.indice.por_origem.entry(o).or_default().push(k as u32);
            }
        }
        self.indice.refs = refs;
    }

    /// Símbolo do arquivo `a` com o nome qualificado `q`: o que contém a linha
    /// (especificação e corpo do pacote têm o mesmo nome) ou a implementação.
    fn local(&self, a: u32, q: &str, linha: u32) -> Option<SimboloId> {
        let simbolos = &self.arquivos[a as usize].extracao.simbolos;
        let mesmos: Vec<usize> = (0..simbolos.len())
            .filter(|&i| simbolos[i].qualificado.eq_ignore_ascii_case(q))
            .collect();
        let i = mesmos
            .iter()
            .find(|&&i| simbolos[i].linhas.contem(linha))
            .or_else(|| mesmos.iter().find(|&&i| !simbolos[i].declaracao))
            .or(mesmos.first())?;
        Some(SimboloId {
            arquivo: a,
            indice: *i as u32,
        })
    }

    fn resolver(&self, alvo: &str, arquivo: u32, mencao: bool) -> Vec<SimboloId> {
        let mut escrito = alvo;
        loop {
            let r = self.resolver_nome(&escrito.to_uppercase(), escrito, arquivo);
            if !r.is_empty() || !mencao {
                return r;
            }
            match escrito.rsplit_once('.') {
                Some((antes, _)) => escrito = antes,
                None => return r,
            }
        }
    }

    fn candidatos(&self, up: &str) -> Vec<SimboloId> {
        let ultimo = up.rsplit('.').next().unwrap_or(up);
        let Some(cands) = self.indice.por_nome.get(ultimo) else {
            return Vec::new();
        };
        let qualificado = up.contains('.');
        let sufixo = format!(".{up}");
        let mut achados: Vec<SimboloId> = cands
            .iter()
            .copied()
            .filter(|id| {
                let s = self.simbolo_bruto(*id);
                let q = s.qualificado.to_uppercase();
                q == up || (q.ends_with(&sufixo) && (qualificado || s.tipo != TipoSimbolo::Coluna))
            })
            .collect();
        if achados.is_empty() && qualificado {
            // Com esquema ou banco na frente: APP.FATURA.
            achados = cands
                .iter()
                .copied()
                .filter(|id| {
                    self.simbolo_bruto(*id)
                        .qualificado
                        .eq_ignore_ascii_case(ultimo)
                })
                .collect();
        }
        achados
    }

    fn resolver_nome(&self, up: &str, escrito: &str, arquivo: u32) -> Vec<SimboloId> {
        let mut achados = self.candidatos(up);
        if achados.iter().any(|id| !self.simbolo_bruto(*id).declaracao) {
            achados.retain(|id| !self.simbolo_bruto(*id).declaracao);
        }
        // Mesma grafia ganha: `FATURA` (tabela) e `Fatura` (classe) coexistem.
        let mesma_grafia = |id: &SimboloId| {
            let q = &self.simbolo_bruto(*id).qualificado;
            q == escrito || q.ends_with(&format!(".{escrito}"))
        };
        if achados.iter().any(mesma_grafia) {
            achados.retain(mesma_grafia);
        }
        if achados.iter().any(|id| id.arquivo == arquivo) {
            achados.retain(|id| id.arquivo == arquivo);
        }
        achados
    }

    fn simbolo_bruto(&self, id: SimboloId) -> &Simbolo {
        &self.arquivos[id.arquivo as usize].extracao.simbolos[id.indice as usize]
    }

    // -- Consultas ----------------------------------------------------------

    /// Arquivos mapeados, na ordem do caminho.
    pub fn arquivos(&self) -> &[ArquivoMapeado] {
        &self.arquivos
    }

    /// Extração de um arquivo: o que a ferramenta `estrutura` mostra.
    pub fn arquivo(&self, caminho: &str) -> Option<&ArquivoMapeado> {
        let a = *self.indice.por_caminho.get(caminho)?;
        Some(&self.arquivos[a as usize])
    }

    /// Arquivos em que o extrator falhou.
    pub fn falhas(&self) -> impl Iterator<Item = &ArquivoMapeado> {
        self.arquivos.iter().filter(|a| a.falha.is_some())
    }

    pub fn simbolo(&self, id: SimboloId) -> Option<Achado<'_>> {
        let arq = self.arquivos.get(id.arquivo as usize)?;
        let simbolo = arq.extracao.simbolos.get(id.indice as usize)?;
        Some(Achado {
            id,
            arquivo: &arq.caminho,
            simbolo,
        })
    }

    fn achado(&self, id: SimboloId) -> Achado<'_> {
        self.simbolo(id).expect("id do próprio mapa")
    }

    /// Todos os símbolos, arquivo a arquivo.
    pub fn simbolos(&self) -> impl Iterator<Item = Achado<'_>> {
        self.arquivos.iter().enumerate().flat_map(|(a, arq)| {
            arq.extracao
                .simbolos
                .iter()
                .enumerate()
                .map(move |(i, simbolo)| Achado {
                    id: SimboloId {
                        arquivo: a as u32,
                        indice: i as u32,
                    },
                    arquivo: &arq.caminho,
                    simbolo,
                })
        })
    }

    /// Símbolos com esse nome (simples ou qualificado, sem diferenciar
    /// maiúsculas), inclusive declarações, ordenados por arquivo e linha.
    pub fn buscar(&self, nome: &str) -> Vec<Achado<'_>> {
        let mut ids = self.candidatos(&nome.trim().to_uppercase());
        ids.sort();
        ids.into_iter().map(|id| self.achado(id)).collect()
    }

    /// Símbolos contidos neste (colunas da tabela, rotinas do pacote).
    pub fn filhos(&self, id: SimboloId) -> Vec<Achado<'_>> {
        self.indice
            .filhos
            .get(&id)
            .map(|v| v.iter().map(|f| self.achado(*f)).collect())
            .unwrap_or_default()
    }

    /// Tabelas, visões, sequências, índices e sinônimos, por nome. As colunas de
    /// cada um saem de [`Mapa::filhos`] e as chaves estrangeiras, de
    /// [`Mapa::referencias_de`].
    pub fn schema(&self) -> Vec<Achado<'_>> {
        let mut v: Vec<_> = self
            .simbolos()
            .filter(|a| a.simbolo.tipo.eh_schema())
            .collect();
        v.sort_by(|a, b| {
            a.simbolo
                .qualificado
                .to_uppercase()
                .cmp(&b.simbolo.qualificado.to_uppercase())
                .then(a.id.cmp(&b.id))
        });
        v
    }

    fn uso(&self, k: u32) -> Uso<'_> {
        let r = &self.indice.refs[k as usize];
        let arq = &self.arquivos[r.arquivo as usize];
        Uso {
            arquivo: &arq.caminho,
            referencia: &arq.extracao.referencias[r.indice as usize],
            origem: r.origem.map(|o| self.achado(o)),
            alvos: r.alvos.clone(),
        }
    }

    /// Quem usa o símbolo (quem chama a procedure, quem lê a tabela).
    pub fn referencias_a(&self, id: SimboloId) -> Vec<Uso<'_>> {
        self.indice
            .por_alvo
            .get(&id)
            .map(|v| v.iter().map(|k| self.uso(*k)).collect())
            .unwrap_or_default()
    }

    /// O que o símbolo usa (o que a procedure chama, a que tabela a chave aponta).
    pub fn referencias_de(&self, id: SimboloId) -> Vec<Uso<'_>> {
        self.indice
            .por_origem
            .get(&id)
            .map(|v| v.iter().map(|k| self.uso(*k)).collect())
            .unwrap_or_default()
    }

    /// Onde um nome é usado: referências ligadas a símbolos com esse nome e
    /// referências sem alvo conhecido escritas com esse nome. Para a ferramenta
    /// `referencias`.
    pub fn usos(&self, nome: &str) -> Vec<Uso<'_>> {
        let up = nome.trim().to_uppercase();
        let alvos: BTreeSet<SimboloId> = self.candidatos(&up).into_iter().collect();
        let ultimo = up.rsplit('.').next().unwrap_or(&up).to_string();
        let mut ks: BTreeSet<u32> = alvos
            .iter()
            .flat_map(|id| self.indice.por_alvo.get(id).into_iter().flatten().copied())
            .collect();
        for (k, r) in self.indice.refs.iter().enumerate() {
            if !r.alvos.is_empty() {
                continue;
            }
            let escrito = &self.arquivos[r.arquivo as usize].extracao.referencias
                [r.indice as usize]
                .alvo
                .to_uppercase();
            let ultimo_escrito = escrito.rsplit('.').next().unwrap_or(escrito);
            if *escrito == up || ultimo_escrito == ultimo {
                ks.insert(k as u32);
            }
        }
        let mut v: Vec<_> = ks.into_iter().map(|k| self.uso(k)).collect();
        v.sort_by(|a, b| (a.arquivo, a.referencia.linha).cmp(&(b.arquivo, b.referencia.linha)));
        v
    }

    /// Visão de uma pasta (`""` para o projeto todo). `None` se não houver
    /// arquivo mapeado nela.
    pub fn modulo(&self, caminho: &str) -> Option<Modulo> {
        let caminho = caminho.trim().trim_matches('/');
        let prefixo = if caminho.is_empty() {
            String::new()
        } else {
            format!("{caminho}/")
        };
        let dentro = |c: &str| c.starts_with(&prefixo);
        // Filho direto de `caminho` que contém o arquivo (subpasta ou o próprio módulo).
        let grupo = |c: &str| -> String {
            let resto = &c[prefixo.len()..];
            match resto.split_once('/') {
                Some((sub, _)) => format!("{prefixo}{sub}"),
                None => caminho.to_string(),
            }
        };
        let mut m = Modulo {
            caminho: caminho.to_string(),
            ..Modulo::default()
        };
        let mut subpastas = BTreeSet::new();
        for arq in self.arquivos.iter().filter(|a| dentro(&a.caminho)) {
            m.arquivos += 1;
            *m.linguagens.entry(arq.linguagem).or_default() += 1;
            for s in &arq.extracao.simbolos {
                *m.simbolos.entry(s.tipo).or_default() += 1;
            }
            if arq.falha.is_some() {
                m.falhas += 1;
            }
            let g = grupo(&arq.caminho);
            if g == caminho {
                m.arquivos_diretos.push(arq.caminho.clone());
            } else {
                subpastas.insert(g);
            }
        }
        if m.arquivos == 0 {
            return None;
        }
        m.subpastas = subpastas.into_iter().collect();
        let mut deps: BTreeMap<(String, String), usize> = BTreeMap::new();
        for r in &self.indice.refs {
            let de = &self.arquivos[r.arquivo as usize].caminho;
            if !dentro(de) {
                continue;
            }
            let destinos: BTreeSet<String> = r
                .alvos
                .iter()
                .map(|a| &self.arquivos[a.arquivo as usize].caminho)
                .filter(|c| dentro(c))
                .map(|c| grupo(c))
                .collect();
            let g = grupo(de);
            for d in destinos {
                if d != g {
                    *deps.entry((g.clone(), d)).or_default() += 1;
                }
            }
        }
        m.dependencias = deps
            .into_iter()
            .map(|((de, para), referencias)| Dependencia {
                de,
                para,
                referencias,
            })
            .collect();
        Some(m)
    }
}

/// Mapeia o projeto a partir do inventário. Com o mapa anterior, não lê de novo os
/// arquivos com o mesmo hash, a mesma linguagem e o mesmo extrator (na mesma
/// versão). Falha de leitura ou de extração num arquivo fica registrada nele e não
/// interrompe o resto.
pub fn mapear(
    inventario: &Inventario,
    escopo: &Escopo,
    anterior: Option<&Mapa>,
    registro: &Registro,
) -> Mapa {
    let inicio = Instant::now();
    let cache: HashMap<&str, &ArquivoMapeado> = anterior
        .filter(|m| m.formato == FORMATO_MAPA)
        .map(|m| m.arquivos.iter().map(|a| (a.caminho.as_str(), a)).collect())
        .unwrap_or_default();
    let itens = &inventario.arquivos;
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .clamp(1, 16)
        .min(itens.len().max(1));
    let proximo = AtomicUsize::new(0);
    let mut resultados: Vec<(usize, ArquivoMapeado, bool)> = std::thread::scope(|s| {
        let tarefas: Vec<_> = (0..threads)
            .map(|_| {
                s.spawn(|| {
                    let mut saida = Vec::new();
                    loop {
                        let i = proximo.fetch_add(1, Ordering::Relaxed);
                        let Some(item) = itens.get(i) else { break };
                        let extrator = registro.para(item.linguagem);
                        let reaproveitavel = cache.get(item.caminho.as_str()).filter(|a| {
                            a.hash == item.hash
                                && a.linguagem == item.linguagem
                                && a.extrator == extrator.nome()
                                && a.versao == extrator.versao()
                        });
                        match reaproveitavel {
                            Some(a) => saida.push((i, (*a).clone(), true)),
                            None => saida.push((i, extrair(item, escopo, registro), false)),
                        }
                    }
                    saida
                })
            })
            .collect();
        tarefas
            .into_iter()
            .flat_map(|t| t.join().expect("thread do mapeamento"))
            .collect()
    });
    resultados.sort_by_key(|(i, _, _)| *i);
    let mut est = EstatisticasMapa::default();
    let mut arquivos = Vec::with_capacity(resultados.len());
    for (_, a, reaproveitado) in resultados {
        if reaproveitado {
            est.reaproveitados += 1;
        } else {
            est.processados += 1;
        }
        if a.falha.is_some() {
            est.falhas += 1;
        }
        arquivos.push(a);
    }
    est.duracao_ms = inicio.elapsed().as_millis().try_into().unwrap_or(u64::MAX);
    Mapa::novo(
        FORMATO_MAPA,
        inventario.projeto.clone(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        est,
        arquivos,
    )
}

fn extrair(item: &ArquivoInventariado, escopo: &Escopo, registro: &Registro) -> ArquivoMapeado {
    let extrator = registro.para(item.linguagem);
    let mut saida = ArquivoMapeado {
        caminho: item.caminho.clone(),
        linguagem: item.linguagem,
        hash: item.hash.clone(),
        extrator: String::new(),
        versao: 0,
        extracao: Extracao::default(),
        falha: None,
    };
    let bytes = match escopo
        .resolver(&item.caminho)
        .map_err(|e| e.to_string())
        .and_then(|p| {
            let tamanho = fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
            if tamanho > LIMITE_EXTRACAO {
                return Err(format!(
                    "arquivo grande demais para extrair ({tamanho} bytes)"
                ));
            }
            fs::read(&p).map_err(|e| e.to_string())
        }) {
        Ok(b) => b,
        Err(e) => {
            saida.falha = Some(format!("leitura: {e}"));
            return saida;
        }
    };
    saida.extrator = extrator.nome().to_string();
    saida.versao = extrator.versao();
    let texto = String::from_utf8_lossy(&bytes);
    let texto = texto.strip_prefix('\u{feff}').unwrap_or(&texto);
    let fonte = Fonte {
        caminho: &item.caminho,
        linguagem: item.linguagem,
        texto,
    };
    match catch_unwind(AssertUnwindSafe(|| extrator.extrair(&fonte))) {
        Ok(Ok(e)) => saida.extracao = e,
        Ok(Err(e)) => saida.falha = Some(e),
        Err(panico) => {
            let msg = panico
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| panico.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "sem mensagem".into());
            saida.falha = Some(format!("pânico no extrator {}: {msg}", extrator.nome()));
        }
    }
    saida
}
