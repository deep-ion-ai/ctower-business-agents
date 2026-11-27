//! Árvore de quem chama (`referencias` com `profundidade` 2 ou 3, #451).
//!
//! O primeiro nível são os usos do identificador, agrupados pela rotina de onde
//! partem. Cada rotina sobe pelos usos que a invocam (chamada, menção numa string,
//! instanciação, disparo, uso), até a profundidade pedida. A árvore serve para
//! fechar o conjunto de caminhos que gravam um estado: a partir do campo ou do
//! método de escrita, até os pontos de entrada (telas, rotas, lotes), inclusive
//! chamadas internas e edições de outros registros.
//!
//! - Todo nó sobe, mesmo quando já é ponto de entrada: um método de serviço é
//!   ponto de entrada pela heurística do mapa, mas costuma ser chamado por uma
//!   rota, e parar nele esconderia o caminho.
//! - Uma rotina que já apareceu em outro ramo entra de novo como repetida, sem
//!   subir outra vez; a que chama um ancestral fecha um ciclo.
//! - A construção é por nível: ao passar do limite de nós, cortam-se os mais
//!   distantes, e a resposta avisa.

use std::collections::{BTreeSet, HashMap};
use std::fmt::Write as _;

use cba_index::{Achado, Mapa, PontoEntrada, SimboloId, TipoReferencia, Uso};
use serde_json::{Value, json};

use crate::paginacao::nome;

/// Maior profundidade aceita.
pub(crate) const PROFUNDIDADE_MAXIMA: u8 = 3;
/// Nós da árvore quando o agente não informa `limite_de_nos`.
pub(crate) const LIMITE_DE_NOS: usize = 40;
/// Maior `limite_de_nos` aceito.
pub(crate) const LIMITE_DE_NOS_MAXIMO: usize = 200;

/// Usos que fazem uma rotina ser executada a partir de outra: é por eles que a
/// árvore sobe a partir do segundo nível.
const SOBE_POR: [TipoReferencia; 5] = [
    TipoReferencia::Chamada,
    TipoReferencia::Mencao,
    TipoReferencia::Instanciacao,
    TipoReferencia::Disparo,
    TipoReferencia::Uso,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fim {
    /// Subiu: os chamadores estão nos filhos (ou não há nenhum no mapa).
    Expandido,
    /// Já apareceu em outro ramo, que mostra os chamadores.
    Repetido,
    /// Chama um ancestral.
    Ciclo,
    /// Na profundidade pedida; `tem_chamadores` diz se subiria mais.
    NoLimite,
    /// Fora de rotina (código solto no script): não há o que subir.
    ForaDeRotina,
    /// Não subiu porque a árvore chegou ao limite de nós.
    Cortado,
}

#[derive(Debug, Clone)]
struct No<'a> {
    nivel: u8,
    pai: Option<usize>,
    origem: Option<Achado<'a>>,
    arquivo: &'a str,
    /// Linhas e tipos dos usos do nó de cima, no arquivo do nó.
    usos: BTreeSet<(u32, String)>,
    fim: Fim,
    tem_chamadores: bool,
    filhos: Vec<usize>,
}

/// Árvore montada, pronta para o texto e os dados.
pub(crate) struct Arvore<'a> {
    mapa: &'a Mapa,
    pontos: HashMap<String, PontoEntrada>,
    nos: Vec<No<'a>>,
    raizes: Vec<usize>,
    profundidade: u8,
    limite: usize,
    cortada: bool,
}

/// Usos de um nó de cima que partem da mesma rotina (ou do mesmo arquivo, fora
/// de rotina): viram um nó.
struct Grupo<'a> {
    origem: Option<Achado<'a>>,
    arquivo: &'a str,
    usos: BTreeSet<(u32, String)>,
}

fn agrupar<'a>(usos: &[Uso<'a>]) -> Vec<Grupo<'a>> {
    let mut grupos: Vec<Grupo<'a>> = Vec::new();
    let mut indice: HashMap<(Option<SimboloId>, &'a str), usize> = HashMap::new();
    for u in usos {
        let chave = (u.origem.map(|o| o.id), u.arquivo);
        let i = *indice.entry(chave).or_insert_with(|| {
            grupos.push(Grupo {
                origem: u.origem,
                arquivo: u.arquivo,
                usos: BTreeSet::new(),
            });
            grupos.len() - 1
        });
        grupos[i]
            .usos
            .insert((u.referencia.linha, nome(&u.referencia.tipo)));
    }
    // Na ordem do arquivo e da primeira linha de uso.
    grupos.sort_by_key(|g| (g.arquivo, g.usos.first().map(|l| l.0)));
    grupos
}

/// Quem invoca a rotina: usos ligados a ela (declaração e implementação, pelo
/// nome qualificado) e aos endpoints dela, sem a própria rotina (recursão).
fn chamadores<'a>(mapa: &'a Mapa, rotina: Achado<'a>) -> Vec<Uso<'a>> {
    let qualificado = &rotina.simbolo.qualificado;
    let mut ids: BTreeSet<SimboloId> = mapa
        .buscar(qualificado)
        .into_iter()
        .filter(|a| a.simbolo.qualificado.eq_ignore_ascii_case(qualificado))
        .map(|a| a.id)
        .collect();
    ids.insert(rotina.id);
    let endpoints: Vec<SimboloId> = ids
        .iter()
        .flat_map(|id| mapa.filhos(*id))
        .filter(eh_endpoint)
        .map(|f| f.id)
        .collect();
    ids.extend(endpoints);
    let mut vistos = BTreeSet::new();
    let mut usos: Vec<Uso<'a>> = ids
        .iter()
        .flat_map(|id| mapa.referencias_a(*id))
        .filter(|u| SOBE_POR.contains(&u.referencia.tipo))
        .filter(|u| u.origem.is_none_or(|o| !ids.contains(&o.id)))
        .filter(|u| vistos.insert((u.arquivo, u.referencia.linha, u.origem.map(|o| o.id))))
        .collect();
    usos.sort_by_key(|u| (u.arquivo, u.referencia.linha));
    usos
}

fn eh_endpoint(a: &Achado<'_>) -> bool {
    a.simbolo
        .detalhe
        .as_deref()
        .is_some_and(|d| d.starts_with("endpoint "))
}

impl<'a> Arvore<'a> {
    /// Monta a árvore a partir dos usos do identificador (o primeiro nível, já
    /// filtrado pelos tipos pedidos).
    pub fn montar(mapa: &'a Mapa, usos: &[Uso<'a>], profundidade: u8, limite: usize) -> Self {
        let pontos = mapa
            .pontos_de_entrada()
            .into_iter()
            .map(|p| (p.chave.clone(), p))
            .collect();
        let mut a = Self {
            mapa,
            pontos,
            nos: Vec::new(),
            raizes: Vec::new(),
            profundidade,
            limite,
            cortada: false,
        };
        // Rotina → o nó em que ela sobe (a primeira vez que aparece na árvore).
        let mut primeiro: HashMap<SimboloId, usize> = HashMap::new();
        let mut fila: Vec<usize> = Vec::new();
        for g in agrupar(usos) {
            let Some(i) = a.novo(1, None, g) else {
                break;
            };
            a.raizes.push(i);
            fila.push(i);
        }
        let mut nivel: u8 = 1;
        while !fila.is_empty() {
            for &i in &fila {
                if let Some(o) = a.nos[i].origem {
                    primeiro.entry(o.id).or_insert(i);
                }
            }
            let mut proxima = Vec::new();
            for i in fila {
                let Some(origem) = a.nos[i].origem else {
                    a.nos[i].fim = Fim::ForaDeRotina;
                    continue;
                };
                let usos = chamadores(mapa, origem);
                a.nos[i].tem_chamadores = !usos.is_empty();
                if primeiro.get(&origem.id).is_some_and(|&p| p != i) {
                    a.nos[i].fim = Fim::Repetido;
                    continue;
                }
                if usos.is_empty() {
                    a.nos[i].fim = Fim::Expandido;
                    continue;
                }
                if nivel >= profundidade {
                    a.nos[i].fim = Fim::NoLimite;
                    continue;
                }
                if a.cortada {
                    a.nos[i].fim = Fim::Cortado;
                    continue;
                }
                a.nos[i].fim = Fim::Expandido;
                for g in agrupar(&usos) {
                    let ciclo = g.origem.is_some_and(|o| a.ancestral(i, o.id));
                    let Some(f) = a.novo(nivel + 1, Some(i), g) else {
                        // Os chamadores que não couberam ficam de fora.
                        a.nos[i].fim = Fim::Cortado;
                        break;
                    };
                    a.nos[i].filhos.push(f);
                    if ciclo {
                        a.nos[f].fim = Fim::Ciclo;
                        a.nos[f].tem_chamadores = true;
                    } else {
                        proxima.push(f);
                    }
                }
            }
            fila = proxima;
            nivel += 1;
        }
        a
    }

    /// Acrescenta um nó, se couber no limite.
    fn novo(&mut self, nivel: u8, pai: Option<usize>, grupo: Grupo<'a>) -> Option<usize> {
        if self.nos.len() >= self.limite {
            self.cortada = true;
            return None;
        }
        self.nos.push(No {
            nivel,
            pai,
            origem: grupo.origem,
            arquivo: grupo.arquivo,
            usos: grupo.usos,
            fim: Fim::Expandido,
            tem_chamadores: false,
            filhos: Vec::new(),
        });
        Some(self.nos.len() - 1)
    }

    fn ancestral(&self, mut i: usize, id: SimboloId) -> bool {
        loop {
            if self.nos[i].origem.is_some_and(|o| o.id == id) {
                return true;
            }
            match self.nos[i].pai {
                Some(p) => i = p,
                None => return false,
            }
        }
    }

    /// O ponto de entrada do nó: a própria rotina ou um endpoint dela.
    fn ponto(&self, origem: Achado<'_>) -> Option<&PontoEntrada> {
        let chave = PontoEntrada::chave_de(origem.arquivo, &origem.simbolo.qualificado);
        self.pontos.get(&chave).or_else(|| {
            self.mapa
                .filhos(origem.id)
                .into_iter()
                .filter(eh_endpoint)
                .find_map(|f| {
                    self.pontos
                        .get(&PontoEntrada::chave_de(f.arquivo, &f.simbolo.qualificado))
                })
        })
    }

    fn rotulo_ponto(p: &PontoEntrada) -> String {
        match &p.detalhe {
            Some(d) if p.tipo == cba_index::TipoEntrada::Endpoint => {
                d.trim_start_matches("endpoint ").to_owned()
            }
            _ => String::new(),
        }
    }

    /// Texto da árvore, um nó por linha, recuado pelo nível.
    pub fn texto(&self, identificador: &str) -> String {
        let mut t = String::new();
        let _ = writeln!(
            t,
            "\nQuem chama, até {} níveis a partir de `{identificador}` ({} nós):",
            self.profundidade,
            self.nos.len()
        );
        if self.nos.is_empty() {
            let _ = writeln!(t, "- nenhum uso no mapa.");
        }
        for &r in &self.raizes {
            self.linha(&mut t, r);
        }
        let pontos: BTreeSet<String> = self
            .nos
            .iter()
            .filter(|n| n.fim != Fim::Repetido)
            .filter_map(|n| n.origem.and_then(|o| self.ponto(o)))
            .map(|p| p.chave.clone())
            .collect();
        let topos = self
            .nos
            .iter()
            .filter(|n| n.fim == Fim::Expandido && !n.tem_chamadores && n.origem.is_some())
            .count();
        let _ = writeln!(
            t,
            "\nPontos de entrada alcançados: {}. Rotinas sem chamador no mapa: {topos} \
             (ponto de entrada, código morto ou chamada dinâmica: confirme com as suas \
             ferramentas de busca).",
            pontos.len()
        );
        if self.cortada {
            let _ = writeln!(
                t,
                "Aviso: a árvore foi cortada em {} nós (`limite_de_nos`). Os níveis mais \
                 distantes ficaram de fora: peça de novo com um limite maior ou parta de \
                 uma das rotinas listadas.",
                self.limite
            );
        }
        if self
            .nos
            .iter()
            .any(|n| n.fim == Fim::NoLimite && n.tem_chamadores)
        {
            let _ = writeln!(
                t,
                "Nós marcados \"sobe mais\" têm chamadores acima da profundidade pedida: \
                 chame `referencias` com o nome deles."
            );
        }
        t
    }

    fn linha(&self, t: &mut String, i: usize) {
        let n = &self.nos[i];
        let recuo = "  ".repeat(usize::from(n.nivel - 1));
        let linhas: Vec<String> = n.usos.iter().map(|(l, _)| l.to_string()).collect();
        let tipos: BTreeSet<&str> = n.usos.iter().map(|(_, t)| t.as_str()).collect();
        let tipos: Vec<&str> = tipos.into_iter().collect();
        let mut s = match n.origem {
            Some(o) => format!(
                "{recuo}- {} ({}:{}–{}), {} {} ({})",
                o.simbolo.qualificado,
                o.arquivo,
                o.simbolo.linhas.inicio,
                o.simbolo.linhas.fim,
                if linhas.len() == 1 { "linha" } else { "linhas" },
                linhas.join(", "),
                tipos.join(", ")
            ),
            None => format!(
                "{recuo}- {}, {} {} ({}), fora de rotina",
                n.arquivo,
                if linhas.len() == 1 { "linha" } else { "linhas" },
                linhas.join(", "),
                tipos.join(", ")
            ),
        };
        if let Some(p) = n.origem.and_then(|o| self.ponto(o)) {
            let rotulo = Self::rotulo_ponto(p);
            if rotulo.is_empty() {
                let _ = write!(s, " [ponto de entrada: {}]", p.tipo.nome());
            } else {
                let _ = write!(s, " [ponto de entrada: {} {rotulo}]", p.tipo.nome());
            }
        }
        match n.fim {
            Fim::Expandido if n.origem.is_some() && !n.tem_chamadores => {
                s.push_str(" [sem chamador no mapa]");
            }
            Fim::Repetido => s.push_str(" [já listado acima]"),
            Fim::Ciclo => s.push_str(" [ciclo: chama um nível de cima]"),
            Fim::NoLimite if n.tem_chamadores => s.push_str(" [sobe mais]"),
            Fim::Cortado => s.push_str(" [cortado: limite de nós]"),
            _ => {}
        }
        let _ = writeln!(t, "{s}");
        for &f in &n.filhos {
            self.linha(t, f);
        }
    }

    /// Dados da árvore: os nós de primeiro nível, com os chamadores aninhados.
    pub fn dados(&self) -> Value {
        json!({
            "profundidade": self.profundidade,
            "nos": self.nos.len(),
            "limite_de_nos": self.limite,
            "cortada": self.cortada,
            "chamadores": self.raizes.iter().map(|&r| self.no_json(r)).collect::<Vec<_>>(),
        })
    }

    fn no_json(&self, i: usize) -> Value {
        let n = &self.nos[i];
        let ponto = n
            .origem
            .and_then(|o| self.ponto(o))
            .map(|p| json!({ "chave": p.chave, "tipo": p.tipo, "detalhe": p.detalhe }));
        json!({
            "nivel": n.nivel,
            "nome": n.origem.map(|o| o.simbolo.qualificado.clone()),
            "tipo": n.origem.map(|o| nome(&o.simbolo.tipo)),
            "arquivo": n.arquivo,
            "linha_inicial": n.origem.map(|o| o.simbolo.linhas.inicio),
            "linha_final": n.origem.map(|o| o.simbolo.linhas.fim),
            "usos": n.usos.iter().map(|(l, t)| json!({ "linha": l, "tipo": t })).collect::<Vec<_>>(),
            "ponto_de_entrada": ponto,
            "situacao": match n.fim {
                Fim::Expandido if n.origem.is_some() && !n.tem_chamadores => "sem_chamador",
                Fim::Expandido => "expandido",
                Fim::Repetido => "repetido",
                Fim::Ciclo => "ciclo",
                Fim::NoLimite => "no_limite",
                Fim::ForaDeRotina => "fora_de_rotina",
                Fim::Cortado => "cortado",
            },
            "sobe_mais": n.fim == Fim::NoLimite && n.tem_chamadores,
            "chamadores": n.filhos.iter().map(|&f| self.no_json(f)).collect::<Vec<_>>(),
        })
    }
}
