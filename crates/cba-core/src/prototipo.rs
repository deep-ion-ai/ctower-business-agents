//! Protótipo de telas, no estado da análise do fluxo `prototipo` (#381).
//!
//! O protótipo parte de uma análise de origem validada (de preferência aprovada):
//! uma análise do código (impacto, explicação, pergunta) ou uma análise de
//! histórias, cujos critérios de aceite os elementos podem citar.
//!
//! # Modelo
//!
//! - [`PedidoPrototipo`]: a entrada opcional do analista: quais telas, a
//!   fidelidade ([`Fidelidade`]: wireframe ou alta fidelidade) e o que deixar de
//!   fora. Fica em [`Prototipo::pedido`], gravado na criação da análise.
//! - [`Tela`]: tela afetada `TL-n` (número dentro da análise), marcada como nova
//!   ou alterada ([`MudancaTela`]), com o motivo ligado à análise de origem
//!   ([`Rastro`]: afirmações, regras e critérios de aceite) e os elementos.
//!   [`Tela::html`] aponta para o protótipo HTML da tela, gravado depois da
//!   lista de telas (#385, [`crate::prototipo_html`]).
//! - [`ElementoTela`]: campo, validação, mensagem ou ação da tela (`E-n`, número
//!   dentro da tela), com o [`Rastro`] da regra ou do critério de aceite de que
//!   vem.
//!
//! # "A confirmar"
//!
//! Tela ou elemento sem nenhum fundamento (nem afirmação, nem regra, nem critério
//! de aceite) é aceito, mas fica "a confirmar": o protótipo mostra o que o
//! analista precisa validar com o negócio em vez de esconder o que foi inventado.
//! Depender de uma pergunta em aberto também deixa o item "a confirmar".

use cba_kb::{CaminhoRelativo, Id, IdHistoria};
use serde::{Deserialize, Serialize};

use crate::analise::{Analise, ErroModelo, EstadoAnalise, Fluxo};
use crate::requisito::{Fundamento, RefItem, sem_repetidos};

/// Telas no máximo num protótipo.
pub const MAXIMO_DE_TELAS: usize = 30;
/// Elementos no máximo numa tela.
pub const MAXIMO_DE_ELEMENTOS: usize = 80;

/// Fidelidade pedida para o protótipo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Fidelidade {
    /// Estrutura e conteúdo, sem a identidade visual.
    #[default]
    Wireframe,
    /// Com a identidade visual do projeto (tokens CSS, #384).
    AltaFidelidade,
}

impl Fidelidade {
    /// Nome para o analista ("alta fidelidade").
    pub fn rotulo(self) -> &'static str {
        match self {
            Self::Wireframe => "wireframe",
            Self::AltaFidelidade => "alta fidelidade",
        }
    }
}

/// Entrada opcional do analista para o protótipo.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PedidoPrototipo {
    /// Telas que o analista quer ver; vazio deixa o agente listar as afetadas.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub telas: Vec<String>,
    #[serde(default)]
    pub fidelidade: Fidelidade,
    /// O que deixar de fora do protótipo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fora: Option<String>,
}

impl PedidoPrototipo {
    /// Pedido com os textos aparados, sem telas vazias nem repetidas.
    pub fn conferido(self) -> Self {
        let mut telas: Vec<String> = Vec::new();
        for t in self.telas {
            let t = t.trim().to_owned();
            if !t.is_empty() && !telas.iter().any(|x| mesmo_nome(x, &t)) {
                telas.push(t);
            }
        }
        Self {
            telas,
            fidelidade: self.fidelidade,
            fora: texto_opcional(self.fora),
        }
    }

    /// O pedido em frases, para a entrada da análise e o roteiro.
    pub fn descrever(&self) -> String {
        let mut partes = vec![format!("Fidelidade: {}.", self.fidelidade.rotulo())];
        if !self.telas.is_empty() {
            partes.push(format!("Telas pedidas: {}.", self.telas.join("; ")));
        }
        if let Some(f) = &self.fora {
            partes.push(format!("Deixar de fora: {f}."));
        }
        partes.join(" ")
    }
}

/// A tela é nova ou alteração de uma que já existe no sistema.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MudancaTela {
    Nova,
    Alterada,
}

impl MudancaTela {
    pub fn rotulo(self) -> &'static str {
        match self {
            Self::Nova => "nova",
            Self::Alterada => "alterada",
        }
    }
}

/// Decisão do analista sobre uma tela, na revisão (#382).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "decisao", rename_all = "kebab-case")]
pub enum DecisaoTela {
    /// Pede ao agente um ajuste na tela, com o que ajustar.
    Ajustar { pedido: String },
    /// Retira o pedido de ajuste.
    Desfazer,
}

/// Tipo de um elemento da tela.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TipoElemento {
    /// Campo de entrada ou de exibição.
    Campo,
    /// Validação aplicada a um ou mais campos.
    Validacao,
    /// Mensagem ao usuário (erro, alerta, confirmação).
    Mensagem,
    /// Ação do usuário (botão, link, comando).
    Acao,
}

impl TipoElemento {
    pub const TODOS: [TipoElemento; 4] = [Self::Campo, Self::Validacao, Self::Mensagem, Self::Acao];

    pub fn rotulo(self) -> &'static str {
        match self {
            Self::Campo => "campo",
            Self::Validacao => "validação",
            Self::Mensagem => "mensagem",
            Self::Acao => "ação",
        }
    }
}

/// Critério de aceite de uma história (`HU-0001/CA-2`) citado pelo protótipo,
/// com a cópia do texto no registro.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CriterioCitado {
    /// Análise de histórias em que o critério está.
    pub analise: Id,
    pub historia: IdHistoria,
    /// Número do critério na história (`CA-<criterio>`).
    pub criterio: u32,
    /// O critério em uma linha ("Dado …, quando …, então …").
    pub texto: String,
}

impl CriterioCitado {
    /// `HU-0001/CA-2`.
    pub fn rotulo(&self) -> String {
        format!("{}/CA-{}", self.historia, self.criterio)
    }
}

/// De onde vem uma tela ou um elemento: afirmações e regras ([`Fundamento`]) e
/// critérios de aceite. Sem nada, o item fica "a confirmar".
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rastro {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fundamentos: Vec<Fundamento>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub criterios: Vec<CriterioCitado>,
    /// Pergunta em aberto de que o item depende: com ela, fica "a confirmar".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pergunta: Option<RefItem>,
}

impl Rastro {
    /// Sem fundamento nem critério, ou dependente de pergunta em aberto.
    pub fn a_confirmar(&self) -> bool {
        self.sem_fundamento() || self.pergunta.is_some()
    }

    /// Nenhuma afirmação, regra ou critério sustenta o item.
    pub fn sem_fundamento(&self) -> bool {
        self.fundamentos.is_empty() && self.criterios.is_empty()
    }

    /// Rótulos das referências, na ordem: afirmações e regras, depois critérios
    /// (`A-0001/3`, `RN-FAT-007`, `HU-0001/CA-2`).
    pub fn rotulos(&self) -> Vec<String> {
        self.fundamentos
            .iter()
            .map(Fundamento::rotulo)
            .chain(self.criterios.iter().map(CriterioCitado::rotulo))
            .collect()
    }

    fn conferido(self) -> Self {
        let mut criterios: Vec<CriterioCitado> = Vec::new();
        for c in self.criterios {
            if !criterios
                .iter()
                .any(|x| x.historia == c.historia && x.criterio == c.criterio)
            {
                criterios.push(c);
            }
        }
        Self {
            fundamentos: sem_repetidos(self.fundamentos),
            criterios,
            pergunta: self.pergunta,
        }
    }
}

/// Elemento de uma tela.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ElementoTela {
    /// `E-<numero>`, sequencial dentro da tela.
    pub numero: u32,
    pub tipo: TipoElemento,
    /// Nome do elemento como aparece na tela ("Número de parcelas").
    pub nome: String,
    /// Detalhe: formato, valores, texto da mensagem, regra da validação.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub descricao: Option<String>,
    #[serde(flatten)]
    pub rastro: Rastro,
    #[serde(default)]
    pub sessao: Option<String>,
    pub registrado_ms: u64,
}

impl ElementoTela {
    /// `E-3`.
    pub fn rotulo(&self) -> String {
        format!("E-{}", self.numero)
    }

    pub fn a_confirmar(&self) -> bool {
        self.rastro.a_confirmar()
    }
}

/// Tela afetada pela análise de origem.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tela {
    /// `TL-<numero>`, sequencial dentro da análise.
    pub numero: u32,
    pub nome: String,
    pub mudanca: MudancaTela,
    /// Por que a tela entra no protótipo, a partir da análise de origem.
    pub motivo: String,
    /// De onde vem o motivo.
    #[serde(flatten)]
    pub rastro: Rastro,
    #[serde(default)]
    pub elementos: Vec<ElementoTela>,
    /// Protótipo HTML da tela, relativo à raiz do projeto na base
    /// (`analises/<id>/prototipo/TL-<n>.html`, [`cba_kb::layout::html_da_tela`]).
    /// Gravado depois da lista de telas (#385); `None` enquanto não houver.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub html: Option<CaminhoRelativo>,
    /// Ajuste pedido pelo analista na revisão (#382), ainda não atendido: o
    /// agente ajusta a tela e grava o HTML de novo, o que limpa o pedido.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ajuste_pedido: Option<String>,
    #[serde(default)]
    pub sessao: Option<String>,
    pub registrada_ms: u64,
}

impl Tela {
    /// `TL-1`.
    pub fn rotulo(&self) -> String {
        format!("TL-{}", self.numero)
    }

    /// O motivo da tela não tem fundamento (ou depende de pergunta em aberto).
    pub fn a_confirmar(&self) -> bool {
        self.rastro.a_confirmar()
    }

    pub fn elemento(&self, numero: u32) -> Option<&ElementoTela> {
        self.elementos.iter().find(|e| e.numero == numero)
    }

    /// Elementos "a confirmar".
    pub fn elementos_a_confirmar(&self) -> impl Iterator<Item = &ElementoTela> {
        self.elementos.iter().filter(|e| e.a_confirmar())
    }
}

/// Protótipo da análise: o pedido do analista e as telas registradas.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Prototipo {
    #[serde(default)]
    pub pedido: PedidoPrototipo,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub telas: Vec<Tela>,
}

/// Dados de um elemento novo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NovoElemento {
    pub tipo: TipoElemento,
    pub nome: String,
    pub descricao: Option<String>,
    pub rastro: Rastro,
    pub sessao: Option<String>,
}

/// Dados de uma tela nova.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NovaTela {
    pub nome: String,
    pub mudanca: MudancaTela,
    pub motivo: String,
    pub rastro: Rastro,
    pub elementos: Vec<NovoElemento>,
    pub sessao: Option<String>,
}

fn exigir(campo: &'static str, valor: &str) -> Result<(), ErroModelo> {
    if valor.trim().is_empty() {
        Err(ErroModelo::CampoVazio(campo))
    } else {
        Ok(())
    }
}

fn texto_opcional(texto: Option<String>) -> Option<String> {
    texto.map(|t| t.trim().to_owned()).filter(|t| !t.is_empty())
}

/// Mesmo nome de tela, sem diferença de caixa nem de espaços.
fn mesmo_nome(a: &str, b: &str) -> bool {
    let normal = |t: &str| {
        t.split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase()
    };
    normal(a) == normal(b)
}

impl NovoElemento {
    fn criar(self, numero: u32, agora_ms: u64) -> Result<ElementoTela, ErroModelo> {
        exigir("elementos.nome", &self.nome)?;
        Ok(ElementoTela {
            numero,
            tipo: self.tipo,
            nome: self.nome.trim().to_owned(),
            descricao: texto_opcional(self.descricao),
            rastro: self.rastro.conferido(),
            sessao: self.sessao,
            registrado_ms: agora_ms,
        })
    }
}

impl Analise {
    fn exigir_fluxo_de_prototipo(&self) -> Result<(), ErroModelo> {
        if self.fluxo == Fluxo::Prototipo {
            Ok(())
        } else {
            Err(ErroModelo::ForaDoFluxoDePrototipo(self.id.clone()))
        }
    }

    /// Telas do protótipo (vazio fora do fluxo `prototipo`).
    pub fn telas(&self) -> &[Tela] {
        self.prototipo.as_ref().map_or(&[], |p| p.telas.as_slice())
    }

    pub fn tela(&self, numero: u32) -> Option<&Tela> {
        self.telas().iter().find(|t| t.numero == numero)
    }

    fn telas_mut(&mut self) -> &mut Vec<Tela> {
        &mut self.prototipo.get_or_insert_with(Prototipo::default).telas
    }

    /// Grava o pedido do analista (na criação da análise). Não muda o estado.
    pub fn pedir_prototipo(&mut self, pedido: PedidoPrototipo) -> Result<(), ErroModelo> {
        self.exigir_fluxo_de_prototipo()?;
        if self.estado == EstadoAnalise::Aprovada {
            return Err(ErroModelo::AnaliseAprovada(self.id.clone()));
        }
        self.prototipo.get_or_insert_with(Prototipo::default).pedido = pedido.conferido();
        Ok(())
    }

    /// Registra uma tela afetada, com os elementos numerados a partir de 1, e
    /// devolve o número dela. Tudo ou nada: um elemento recusado recusa a tela.
    /// Duas telas com o mesmo nome são recusadas: acrescente elementos com
    /// [`Analise::registrar_elemento`].
    pub fn registrar_tela(&mut self, nova: NovaTela, agora_ms: u64) -> Result<u32, ErroModelo> {
        self.exigir_fluxo_de_prototipo()?;
        exigir("nome", &nova.nome)?;
        exigir("motivo", &nova.motivo)?;
        let nome = nova.nome.trim().to_owned();
        if let Some(t) = self.telas().iter().find(|t| mesmo_nome(&t.nome, &nome)) {
            return Err(ErroModelo::TelaRepetida {
                nome,
                numero: t.numero,
            });
        }
        if self.telas().len() >= MAXIMO_DE_TELAS {
            return Err(ErroModelo::TelaInvalida(format!(
                "o protótipo já tem {MAXIMO_DE_TELAS} telas, o máximo"
            )));
        }
        if nova.elementos.len() > MAXIMO_DE_ELEMENTOS {
            return Err(ErroModelo::TelaInvalida(format!(
                "uma tela tem no máximo {MAXIMO_DE_ELEMENTOS} elementos"
            )));
        }
        let elementos = nova
            .elementos
            .into_iter()
            .zip(1..)
            .map(|(e, n)| e.criar(n, agora_ms))
            .collect::<Result<Vec<_>, _>>()?;
        self.aceitar_registro()?;
        let numero = self.telas().iter().map(|t| t.numero).max().unwrap_or(0) + 1;
        self.telas_mut().push(Tela {
            numero,
            nome,
            mudanca: nova.mudanca,
            motivo: nova.motivo.trim().to_owned(),
            rastro: nova.rastro.conferido(),
            elementos,
            html: None,
            ajuste_pedido: None,
            sessao: nova.sessao,
            registrada_ms: agora_ms,
        });
        Ok(numero)
    }

    /// Acrescenta um elemento a uma tela e devolve o número dele.
    pub fn registrar_elemento(
        &mut self,
        tela: u32,
        novo: NovoElemento,
        agora_ms: u64,
    ) -> Result<u32, ErroModelo> {
        self.exigir_fluxo_de_prototipo()?;
        let t = self.tela(tela).ok_or(ErroModelo::ItemDesconhecido {
            tipo: "tela",
            numero: tela,
        })?;
        if t.elementos.len() >= MAXIMO_DE_ELEMENTOS {
            return Err(ErroModelo::TelaInvalida(format!(
                "a tela {} já tem {MAXIMO_DE_ELEMENTOS} elementos, o máximo",
                t.rotulo()
            )));
        }
        let numero = t.elementos.iter().map(|e| e.numero).max().unwrap_or(0) + 1;
        let elemento = novo.criar(numero, agora_ms)?;
        self.aceitar_registro()?;
        let t = self
            .telas_mut()
            .iter_mut()
            .find(|t| t.numero == tela)
            .ok_or(ErroModelo::ItemDesconhecido {
                tipo: "tela",
                numero: tela,
            })?;
        t.elementos.push(elemento);
        Ok(numero)
    }

    /// Aponta a tela para o protótipo HTML dela (ou tira, com `None`) e devolve
    /// o caminho anterior. O arquivo é gravado antes, por quem chama
    /// ([`crate::prototipo_html::montar`]).
    pub fn definir_html_da_tela(
        &mut self,
        tela: u32,
        html: Option<CaminhoRelativo>,
    ) -> Result<Option<CaminhoRelativo>, ErroModelo> {
        self.exigir_fluxo_de_prototipo()?;
        if self.tela(tela).is_none() {
            return Err(ErroModelo::ItemDesconhecido {
                tipo: "tela",
                numero: tela,
            });
        }
        self.aceitar_registro()?;
        let t = self
            .telas_mut()
            .iter_mut()
            .find(|t| t.numero == tela)
            .ok_or(ErroModelo::ItemDesconhecido {
                tipo: "tela",
                numero: tela,
            })?;
        if html.is_some() {
            // O HTML gravado de novo atende ao ajuste pedido pelo analista.
            t.ajuste_pedido = None;
        }
        Ok(std::mem::replace(&mut t.html, html))
    }

    /// Decisão do analista sobre uma tela (#382): pedir um ajuste ou retirar o
    /// pedido. Não muda o estado da análise; o pedido em aberto impede a
    /// aprovação e a conclusão até o agente gravar o HTML da tela de novo.
    pub fn decidir_tela(&mut self, tela: u32, decisao: DecisaoTela) -> Result<(), ErroModelo> {
        self.exigir_fluxo_de_prototipo()?;
        if self.estado == EstadoAnalise::Aprovada {
            return Err(ErroModelo::AnaliseAprovada(self.id.clone()));
        }
        let t = self
            .telas_mut()
            .iter_mut()
            .find(|t| t.numero == tela)
            .ok_or(ErroModelo::ItemDesconhecido {
                tipo: "tela",
                numero: tela,
            })?;
        t.ajuste_pedido = match decisao {
            DecisaoTela::Ajustar { pedido } => {
                exigir("pedido", &pedido)?;
                Some(pedido.trim().to_owned())
            }
            DecisaoTela::Desfazer => None,
        };
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cba_kb::Serie;

    fn analise() -> Analise {
        let mut a = Analise::nova(
            Serie::Analise.id(3),
            "faturamento-exemplo",
            Fluxo::Prototipo,
            "Protótipo do parcelamento",
            "Protótipo de telas da análise A-0001",
            1,
        );
        a.origem = Some(Serie::Analise.id(1));
        a
    }

    fn regra() -> Fundamento {
        Fundamento::Regra {
            id: Id::analisar("RN-FAT-007").unwrap(),
            titulo: "Parcelamento de fatura PJ".into(),
            evidencias: Vec::new(),
        }
    }

    fn criterio() -> CriterioCitado {
        CriterioCitado {
            analise: Serie::Analise.id(2),
            historia: IdHistoria::novo(1),
            criterio: 2,
            texto: "Dado uma fatura PJ, quando o cliente pede 13 parcelas, então o sistema recusa"
                .into(),
        }
    }

    fn elemento(tipo: TipoElemento, nome: &str, rastro: Rastro) -> NovoElemento {
        NovoElemento {
            tipo,
            nome: nome.into(),
            descricao: None,
            rastro,
            sessao: None,
        }
    }

    fn tela(nome: &str, elementos: Vec<NovoElemento>) -> NovaTela {
        NovaTela {
            nome: nome.into(),
            mudanca: MudancaTela::Alterada,
            motivo: "O parcelamento de fatura PJ passa a aceitar até 12 parcelas".into(),
            rastro: Rastro {
                fundamentos: vec![regra()],
                ..Rastro::default()
            },
            elementos,
            sessao: None,
        }
    }

    #[test]
    fn tela_com_elementos_rastreados_e_a_confirmar() {
        let mut a = analise();
        let n = a
            .registrar_tela(
                tela(
                    "Parcelar fatura",
                    vec![
                        elemento(
                            TipoElemento::Campo,
                            " Número de parcelas ",
                            Rastro {
                                fundamentos: vec![regra(), regra()],
                                ..Rastro::default()
                            },
                        ),
                        elemento(
                            TipoElemento::Mensagem,
                            "Máximo de 12 parcelas",
                            Rastro {
                                criterios: vec![criterio(), criterio()],
                                ..Rastro::default()
                            },
                        ),
                        elemento(TipoElemento::Acao, "Simular", Rastro::default()),
                    ],
                ),
                2,
            )
            .unwrap();
        assert_eq!(n, 1);
        assert_eq!(a.estado, EstadoAnalise::EmAndamento);
        let t = a.tela(1).unwrap();
        assert_eq!(t.rotulo(), "TL-1");
        assert!(!t.a_confirmar());
        assert!(t.html.is_none());
        assert_eq!(t.elementos[0].nome, "Número de parcelas");
        assert_eq!(t.elementos[0].rastro.fundamentos.len(), 1);
        assert_eq!(t.elementos[1].rastro.rotulos(), ["HU-0001/CA-2"]);
        let a_confirmar: Vec<_> = t.elementos_a_confirmar().map(|e| e.rotulo()).collect();
        assert_eq!(a_confirmar, ["E-3"]);

        let e = a
            .registrar_elemento(
                1,
                elemento(
                    TipoElemento::Validacao,
                    "Parcelas entre 2 e 12",
                    Rastro {
                        fundamentos: vec![regra()],
                        pergunta: Some(RefItem::analisar("A-0001/2").unwrap()),
                        ..Rastro::default()
                    },
                ),
                3,
            )
            .unwrap();
        assert_eq!(e, 4);
        assert!(a.tela(1).unwrap().elemento(4).unwrap().a_confirmar());
    }

    #[test]
    fn html_da_tela() {
        let mut a = analise();
        a.registrar_tela(tela("Parcelar fatura", Vec::new()), 2)
            .unwrap();
        let caminho = cba_kb::layout::html_da_tela(&a.id, 1);
        assert_eq!(a.definir_html_da_tela(1, Some(caminho.clone())), Ok(None));
        assert_eq!(a.tela(1).unwrap().html.as_ref(), Some(&caminho));
        assert_eq!(
            a.definir_html_da_tela(2, Some(caminho.clone())),
            Err(ErroModelo::ItemDesconhecido {
                tipo: "tela",
                numero: 2
            })
        );
        a.estado = EstadoAnalise::Aprovada;
        assert_eq!(
            a.definir_html_da_tela(1, None),
            Err(ErroModelo::AnaliseAprovada(a.id.clone()))
        );
    }

    #[test]
    fn tela_sem_fundamento_fica_a_confirmar() {
        let mut a = analise();
        let mut t = tela("Consultar parcelas", Vec::new());
        t.mudanca = MudancaTela::Nova;
        t.rastro = Rastro::default();
        a.registrar_tela(t, 2).unwrap();
        assert!(a.tela(1).unwrap().a_confirmar());
    }

    #[test]
    fn recusas() {
        let mut a = analise();
        a.registrar_tela(tela("Parcelar fatura", Vec::new()), 2)
            .unwrap();
        assert_eq!(
            a.registrar_tela(tela(" parcelar  FATURA ", Vec::new()), 3),
            Err(ErroModelo::TelaRepetida {
                nome: "parcelar  FATURA".into(),
                numero: 1
            })
        );
        let mut sem_motivo = tela("Outra", Vec::new());
        sem_motivo.motivo = " ".into();
        assert_eq!(
            a.registrar_tela(sem_motivo, 3),
            Err(ErroModelo::CampoVazio("motivo"))
        );
        assert_eq!(
            a.registrar_tela(
                tela(
                    "Outra",
                    vec![elemento(TipoElemento::Campo, "", Rastro::default())]
                ),
                3
            ),
            Err(ErroModelo::CampoVazio("elementos.nome"))
        );
        assert_eq!(a.telas().len(), 1);
        assert_eq!(
            a.registrar_elemento(9, elemento(TipoElemento::Campo, "X", Rastro::default()), 3),
            Err(ErroModelo::ItemDesconhecido {
                tipo: "tela",
                numero: 9
            })
        );
        let mut historias = analise();
        historias.fluxo = Fluxo::Historias;
        assert_eq!(
            historias.registrar_tela(tela("Parcelar fatura", Vec::new()), 2),
            Err(ErroModelo::ForaDoFluxoDePrototipo(historias.id.clone()))
        );
        a.estado = EstadoAnalise::Aprovada;
        assert_eq!(
            a.registrar_tela(tela("Outra", Vec::new()), 4),
            Err(ErroModelo::AnaliseAprovada(a.id.clone()))
        );
    }

    #[test]
    fn pedido_conferido_e_descrito() {
        let mut a = analise();
        a.pedir_prototipo(PedidoPrototipo {
            telas: vec![
                " Parcelar fatura ".into(),
                "parcelar fatura".into(),
                " ".into(),
            ],
            fidelidade: Fidelidade::AltaFidelidade,
            fora: Some("  ".into()),
        })
        .unwrap();
        assert_eq!(a.estado, EstadoAnalise::Criada);
        let pedido = &a.prototipo.as_ref().unwrap().pedido;
        assert_eq!(pedido.telas, ["Parcelar fatura"]);
        assert!(pedido.fora.is_none());
        assert_eq!(
            pedido.descrever(),
            "Fidelidade: alta fidelidade. Telas pedidas: Parcelar fatura."
        );
    }

    #[test]
    fn estado_ida_e_volta_em_json() {
        let mut a = analise();
        a.pedir_prototipo(PedidoPrototipo::default()).unwrap();
        a.registrar_tela(
            tela(
                "Parcelar fatura",
                vec![elemento(
                    TipoElemento::Mensagem,
                    "Máximo de 12 parcelas",
                    Rastro {
                        criterios: vec![criterio()],
                        ..Rastro::default()
                    },
                )],
            ),
            2,
        )
        .unwrap();
        let json = serde_json::to_string(&a).unwrap();
        assert!(json.contains("\"fluxo\":\"prototipo\""), "{json}");
        assert!(json.contains("\"mudanca\":\"alterada\""), "{json}");
        assert!(json.contains("\"fidelidade\":\"wireframe\""), "{json}");
        assert!(json.contains("\"historia\":\"HU-0001\""), "{json}");
        let lida: Analise = serde_json::from_str(&json).unwrap();
        assert_eq!(lida, a);
    }

    #[test]
    fn ajuste_pedido_numa_tela() {
        let mut a = analise();
        a.registrar_tela(tela("Parcelar fatura", Vec::new()), 2)
            .unwrap();
        let ajustar = |pedido: &str| DecisaoTela::Ajustar {
            pedido: pedido.into(),
        };
        assert_eq!(
            a.decidir_tela(1, ajustar("  ")),
            Err(ErroModelo::CampoVazio("pedido"))
        );
        assert_eq!(
            a.decidir_tela(9, ajustar("Mostrar o total")),
            Err(ErroModelo::ItemDesconhecido {
                tipo: "tela",
                numero: 9
            })
        );
        a.decidir_tela(1, ajustar(" Mostrar o total das parcelas "))
            .unwrap();
        assert_eq!(
            a.tela(1).unwrap().ajuste_pedido.as_deref(),
            Some("Mostrar o total das parcelas")
        );
        let json = serde_json::to_value(a.tela(1).unwrap()).unwrap();
        assert_eq!(json["ajuste_pedido"], "Mostrar o total das parcelas");
        let d: DecisaoTela =
            serde_json::from_value(serde_json::json!({ "decisao": "ajustar", "pedido": "x" }))
                .unwrap();
        assert_eq!(d, ajustar("x"));

        a.decidir_tela(1, DecisaoTela::Desfazer).unwrap();
        assert!(a.tela(1).unwrap().ajuste_pedido.is_none());

        // Gravar o HTML de novo atende ao pedido.
        a.decidir_tela(1, ajustar("Mostrar o total")).unwrap();
        let caminho = cba_kb::layout::html_da_tela(&a.id, 1);
        a.definir_html_da_tela(1, Some(caminho)).unwrap();
        assert!(a.tela(1).unwrap().ajuste_pedido.is_none());

        a.estado = EstadoAnalise::Aprovada;
        assert!(matches!(
            a.decidir_tela(1, ajustar("Mais um")),
            Err(ErroModelo::AnaliseAprovada(_))
        ));
    }
}
