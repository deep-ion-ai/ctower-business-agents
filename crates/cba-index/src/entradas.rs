//! Pontos de entrada do sistema, calculados do mapa estrutural sem IA (#208).
//!
//! O fluxo "Mapear funcionalidades" agrupa os pontos de entrada em
//! funcionalidades de negócio. Esta lista é a referência da completude: ao
//! concluir, todo ponto que o agente não atribuiu a uma funcionalidade precisa ser
//! tratado (justificado como código morto, utilitário ou outro motivo).
//!
//! # O que conta como ponto de entrada
//!
//! - **Tela**: símbolo do tipo [`TipoSimbolo::Tela`] (formulário Delphi ou VB6,
//!   página) e, no front, funções e classes de topo dos arquivos de pastas de tela
//!   (`telas`, `pages`, `views`, `screens`, `paginas`) que não são chamadas só de
//!   dentro do próprio arquivo.
//! - **Endpoint**: símbolo com o detalhe `endpoint VERBO rota` (os extratores de
//!   Java, C# e TypeScript, #16).
//! - **Lote**: programa ([`TipoSimbolo::Programa`]: COBOL, projeto Delphi ou VB6)
//!   e métodos de classes `*Job`, `*Batch`, `*Scheduler`, `*Task`.
//! - **Rotina de banco**: procedure ou função de SQL pública, declarada na
//!   especificação do pacote ou fora de pacote.
//! - **Serviço**: métodos de classes `*Controller`, `*Resource`, `*Endpoint`,
//!   `*Service`, `*Facade`, `*Handler`, `*Api`.
//! - **Integração**: métodos de classes `*Listener`, `*Consumer`, `*Subscriber`,
//!   `*Receiver`, `*Importer`.
//!
//! Métodos, funções e rotinas que só são chamados de dentro do próprio arquivo
//! (ajudantes internos) não entram. A lista é conservadora: na dúvida, o ponto
//! entra e o agente justifica.
//!
//! # Chave estável
//!
//! `<arquivo>#<nome qualificado>` ([`PontoEntrada::chave`]): é o que a análise
//! guarda para saber o que foi atribuído. Os ids de símbolo do mapa não são
//! estáveis entre mapeamentos.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::extrator::{Linhas, TipoSimbolo};
use crate::linguagem::Linguagem;
use crate::mapa::{Achado, Mapa};

/// Natureza de um ponto de entrada.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TipoEntrada {
    Tela,
    Endpoint,
    Lote,
    RotinaBanco,
    Servico,
    Integracao,
}

impl TipoEntrada {
    /// Nome para quem lê (o agente e o analista).
    pub fn nome(self) -> &'static str {
        match self {
            Self::Tela => "tela",
            Self::Endpoint => "endpoint",
            Self::Lote => "lote",
            Self::RotinaBanco => "rotina de banco",
            Self::Servico => "serviço",
            Self::Integracao => "integração",
        }
    }
}

/// Ponto de entrada achado no mapa.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PontoEntrada {
    /// `<arquivo>#<nome qualificado>`, estável entre mapeamentos.
    pub chave: String,
    pub tipo: TipoEntrada,
    /// Nome qualificado do símbolo.
    pub nome: String,
    /// Caminho relativo à raiz do projeto.
    pub arquivo: String,
    pub linhas: Linhas,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detalhe: Option<String>,
}

impl PontoEntrada {
    /// Chave de um símbolo: `<arquivo>#<nome qualificado>`.
    pub fn chave_de(arquivo: &str, qualificado: &str) -> String {
        format!("{arquivo}#{qualificado}")
    }

    /// O ponto está na pasta `escopo` (`""` é o projeto todo).
    pub fn na_pasta(&self, escopo: &str) -> bool {
        let escopo = escopo.trim().trim_matches('/');
        escopo.is_empty()
            || self.arquivo == escopo
            || self
                .arquivo
                .strip_prefix(escopo)
                .is_some_and(|r| r.starts_with('/'))
    }
}

const SUFIXOS_SERVICO: &[&str] = &[
    "CONTROLLER",
    "RESOURCE",
    "ENDPOINT",
    "SERVICE",
    "FACADE",
    "HANDLER",
    "API",
];
const SUFIXOS_LOTE: &[&str] = &["JOB", "BATCH", "SCHEDULER", "TASK"];
const SUFIXOS_INTEGRACAO: &[&str] = &["LISTENER", "CONSUMER", "SUBSCRIBER", "RECEIVER", "IMPORTER"];
const PASTAS_DE_TELA: &[&str] = &["telas", "pages", "views", "screens", "paginas"];

fn ultimo(qualificado: &str) -> &str {
    qualificado.rsplit('.').next().unwrap_or(qualificado)
}

fn tipo_pela_classe(classe: &str) -> Option<TipoEntrada> {
    let c = ultimo(classe).to_uppercase();
    let termina = |lista: &[&str]| lista.iter().any(|s| c.ends_with(s) && c.len() > s.len());
    if termina(SUFIXOS_INTEGRACAO) {
        Some(TipoEntrada::Integracao)
    } else if termina(SUFIXOS_LOTE) {
        Some(TipoEntrada::Lote)
    } else if termina(SUFIXOS_SERVICO) {
        Some(TipoEntrada::Servico)
    } else {
        None
    }
}

fn em_pasta_de_tela(arquivo: &str) -> bool {
    let mut partes: Vec<&str> = arquivo.split('/').collect();
    partes.pop();
    partes
        .iter()
        .any(|p| PASTAS_DE_TELA.contains(&p.to_lowercase().as_str()))
}

impl Mapa {
    /// O símbolo só é usado de dentro do próprio arquivo: tem referências e todas
    /// partem do arquivo dele.
    fn so_interno(&self, a: &Achado<'_>) -> bool {
        let refs = self.referencias_a(a.id);
        !refs.is_empty() && refs.iter().all(|u| u.arquivo == a.arquivo)
    }

    fn linguagem_de(&self, arquivo: &str) -> Option<Linguagem> {
        self.arquivo(arquivo).map(|a| a.linguagem)
    }

    fn classificar(&self, a: &Achado<'_>) -> Option<TipoEntrada> {
        let s = a.simbolo;
        let linguagem = self.linguagem_de(a.arquivo)?;
        match s.tipo {
            TipoSimbolo::Tela => Some(TipoEntrada::Tela),
            TipoSimbolo::Programa => Some(TipoEntrada::Lote),
            TipoSimbolo::Outro
                if s.detalhe
                    .as_deref()
                    .is_some_and(|d| d.starts_with("endpoint ")) =>
            {
                Some(TipoEntrada::Endpoint)
            }
            TipoSimbolo::Procedure | TipoSimbolo::Funcao if linguagem == Linguagem::Sql => {
                let publica = s.declaracao || s.pai.is_none();
                publica.then_some(TipoEntrada::RotinaBanco)
            }
            TipoSimbolo::Metodo => {
                let tipo = tipo_pela_classe(s.pai.as_deref()?)?;
                // O endpoint do método já é o ponto de entrada.
                let tem_endpoint = self.filhos(a.id).iter().any(|f| {
                    f.simbolo
                        .detalhe
                        .as_deref()
                        .is_some_and(|d| d.starts_with("endpoint "))
                });
                (!tem_endpoint && !self.so_interno(a)).then_some(tipo)
            }
            TipoSimbolo::Funcao | TipoSimbolo::Classe
                if matches!(linguagem, Linguagem::TypeScript | Linguagem::JavaScript)
                    && s.pai.is_none()
                    && em_pasta_de_tela(a.arquivo) =>
            {
                (!self.so_interno(a)).then_some(TipoEntrada::Tela)
            }
            _ => None,
        }
    }

    /// Onde o ponto fica: a implementação, quando o achado é só a declaração
    /// (especificação do pacote, protótipo do método).
    fn implementacao<'a>(&'a self, a: Achado<'a>) -> Achado<'a> {
        if !a.simbolo.declaracao {
            return a;
        }
        self.buscar(&a.simbolo.qualificado)
            .into_iter()
            .find(|b| {
                !b.simbolo.declaracao
                    && b.simbolo.tipo == a.simbolo.tipo
                    && b.simbolo
                        .qualificado
                        .eq_ignore_ascii_case(&a.simbolo.qualificado)
            })
            .unwrap_or(a)
    }

    /// Pontos de entrada do projeto, por arquivo e linha, sem repetição (a
    /// declaração e a implementação da mesma rotina contam uma vez, na
    /// implementação).
    pub fn pontos_de_entrada(&self) -> Vec<PontoEntrada> {
        let mut por_chave: BTreeMap<String, PontoEntrada> = BTreeMap::new();
        for a in self.simbolos() {
            let Some(tipo) = self.classificar(&a) else {
                continue;
            };
            let local = self.implementacao(a);
            let chave = PontoEntrada::chave_de(local.arquivo, &local.simbolo.qualificado);
            por_chave
                .entry(chave.clone())
                .or_insert_with(|| PontoEntrada {
                    chave,
                    tipo,
                    nome: local.simbolo.qualificado.clone(),
                    arquivo: local.arquivo.to_owned(),
                    linhas: local.simbolo.linhas,
                    detalhe: local.simbolo.detalhe.clone(),
                });
        }
        let mut v: Vec<PontoEntrada> = por_chave.into_values().collect();
        v.sort_by(|a, b| {
            (a.arquivo.as_str(), a.linhas.inicio, a.nome.as_str()).cmp(&(
                b.arquivo.as_str(),
                b.linhas.inicio,
                b.nome.as_str(),
            ))
        });
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extrator::{Extracao, Referencia, Simbolo, TipoReferencia};
    use crate::mapa::ArquivoMapeado;

    fn arquivo(caminho: &str, linguagem: Linguagem, extracao: Extracao) -> ArquivoMapeado {
        ArquivoMapeado {
            caminho: caminho.into(),
            linguagem,
            hash: "h".into(),
            extrator: "teste".into(),
            versao: 1,
            extracao,
            falha: None,
        }
    }

    fn l(i: u32, f: u32) -> Linhas {
        Linhas::new(i, f)
    }

    fn chamada(origem: &str, alvo: &str, linha: u32) -> Referencia {
        Referencia {
            origem: Some(origem.into()),
            alvo: alvo.into(),
            tipo: TipoReferencia::Chamada,
            linha,
        }
    }

    fn mapa() -> Mapa {
        let mut spec =
            Simbolo::novo("PRC_PUBLICA", TipoSimbolo::Procedure, l(3, 3)).dentro_de("PKG");
        spec.declaracao = true;
        let sql = Extracao {
            simbolos: vec![
                Simbolo::novo("PKG", TipoSimbolo::Pacote, l(1, 4)),
                spec,
                Simbolo::novo("PKG", TipoSimbolo::Pacote, l(6, 30)),
                Simbolo::novo("PRC_PUBLICA", TipoSimbolo::Procedure, l(8, 15)).dentro_de("PKG"),
                Simbolo::novo("PRC_PRIVADA", TipoSimbolo::Procedure, l(17, 20)).dentro_de("PKG"),
                Simbolo::novo("FN_SOLTA", TipoSimbolo::Funcao, l(32, 40)),
                Simbolo::novo("FATURA", TipoSimbolo::Tabela, l(42, 50)),
            ],
            referencias: vec![chamada("PKG.PRC_PUBLICA", "PRC_PRIVADA", 9)],
        };
        let java = Extracao {
            simbolos: vec![
                Simbolo::novo("CobrancaService", TipoSimbolo::Classe, l(5, 40)).dentro_de("ex"),
                Simbolo::novo("CobrancaService", TipoSimbolo::Construtor, l(6, 8))
                    .dentro_de("ex.CobrancaService"),
                Simbolo::novo("cobrar", TipoSimbolo::Metodo, l(10, 20))
                    .dentro_de("ex.CobrancaService"),
                Simbolo::novo("ajudar", TipoSimbolo::Metodo, l(22, 30))
                    .dentro_de("ex.CobrancaService"),
                Simbolo::novo("consultar", TipoSimbolo::Metodo, l(32, 38))
                    .dentro_de("ex.CobrancaService"),
                Simbolo::novo("Fatura", TipoSimbolo::Classe, l(42, 50)).dentro_de("ex"),
                Simbolo::novo("total", TipoSimbolo::Metodo, l(44, 46)).dentro_de("ex.Fatura"),
                Simbolo::novo("/api/faturas", TipoSimbolo::Outro, l(12, 12))
                    .dentro_de("ex.CobrancaService.cobrar"),
            ],
            referencias: vec![chamada("ex.CobrancaService.cobrar", "ajudar", 15)],
        };
        let mut endpoint = java;
        endpoint.simbolos[7].detalhe = Some("endpoint POST /api/faturas".into());
        let ts = Extracao {
            simbolos: vec![
                Simbolo::novo("validar", TipoSimbolo::Funcao, l(1, 5)),
                Simbolo::novo("enviar", TipoSimbolo::Funcao, l(7, 12)),
                Simbolo::novo("Pedido", TipoSimbolo::Interface, l(14, 16)),
            ],
            referencias: vec![chamada("enviar", "validar", 8)],
        };
        let util_ts = Extracao {
            simbolos: vec![Simbolo::novo("formatar", TipoSimbolo::Funcao, l(1, 3))],
            referencias: vec![],
        };
        let cobol = Extracao {
            simbolos: vec![Simbolo::novo("CALC", TipoSimbolo::Programa, l(1, 90))],
            referencias: vec![],
        };
        let dfm = Extracao {
            simbolos: vec![Simbolo::novo("FrmX", TipoSimbolo::Tela, l(1, 20))],
            referencias: vec![],
        };
        Mapa::de_arquivos(vec![
            arquivo("app/CobrancaService.java", Linguagem::Java, endpoint),
            arquivo("cobol/CALC.cbl", Linguagem::Cobol, cobol),
            arquivo("db/pkg.sql", Linguagem::Sql, sql),
            arquivo("delphi/FrmX.dfm", Linguagem::Delphi, dfm),
            arquivo("front/src/telas/pedido.ts", Linguagem::TypeScript, ts),
            arquivo("front/src/util/formatar.ts", Linguagem::TypeScript, util_ts),
        ])
    }

    #[test]
    fn classifica_os_pontos_de_entrada() {
        let pontos = mapa().pontos_de_entrada();
        let resumo: Vec<(TipoEntrada, &str)> =
            pontos.iter().map(|p| (p.tipo, p.chave.as_str())).collect();
        assert_eq!(
            resumo,
            vec![
                // `cobrar` tem endpoint: só o endpoint conta. `ajudar` é interno.
                (
                    TipoEntrada::Endpoint,
                    "app/CobrancaService.java#ex.CobrancaService.cobrar./api/faturas"
                ),
                (
                    TipoEntrada::Servico,
                    "app/CobrancaService.java#ex.CobrancaService.consultar"
                ),
                (TipoEntrada::Lote, "cobol/CALC.cbl#CALC"),
                // A declaração na especificação aponta para a implementação.
                (TipoEntrada::RotinaBanco, "db/pkg.sql#PKG.PRC_PUBLICA"),
                (TipoEntrada::RotinaBanco, "db/pkg.sql#FN_SOLTA"),
                (TipoEntrada::Tela, "delphi/FrmX.dfm#FrmX"),
                // `validar` só é chamada de dentro do arquivo; a interface não conta.
                (TipoEntrada::Tela, "front/src/telas/pedido.ts#enviar"),
            ]
        );
        let publica = pontos.iter().find(|p| p.nome == "PKG.PRC_PUBLICA").unwrap();
        assert_eq!(publica.linhas, l(8, 15));
    }

    #[test]
    fn filtra_por_pasta() {
        let pontos = mapa().pontos_de_entrada();
        let db: Vec<_> = pontos.iter().filter(|p| p.na_pasta("db/")).collect();
        assert_eq!(db.len(), 2);
        assert!(pontos.iter().all(|p| p.na_pasta("")));
        assert!(!pontos[0].na_pasta("ap"));
    }

    #[test]
    fn sufixos_das_classes() {
        assert_eq!(
            tipo_pela_classe("a.b.FaturaController"),
            Some(TipoEntrada::Servico)
        );
        assert_eq!(tipo_pela_classe("CobrancaJob"), Some(TipoEntrada::Lote));
        assert_eq!(
            tipo_pela_classe("PagamentoListener"),
            Some(TipoEntrada::Integracao)
        );
        assert_eq!(tipo_pela_classe("Service"), None);
        assert_eq!(tipo_pela_classe("Fatura"), None);
    }
}
