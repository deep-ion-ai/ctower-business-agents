//! Entregáveis gerados numa demanda (#463), a partir de uma análise validada.
//!
//! A análise investiga e reúne as evidências; o entregável (a história, o épico,
//! o caso de uso) é derivado dela e herda as regras, as evidências e as decisões.
//! O catálogo dos tipos (esquema, ciclo de vida, geração) fica em
//! `cba_docs::entregaveis`; aqui fica só o que a demanda guarda, com o tipo pelo
//! `id` em texto.
//!
//! - [`Entregavel`]: o tipo, a análise de origem, a funcionalidade (tipo vivo),
//!   o destino e os rascunhos. Cada geração ("Gerar" ou "Gerar de novo") abre um
//!   [`Rascunho`] novo, com a versão seguinte: o anterior continua guardado, nunca
//!   é sobrescrito.
//! - [`ItemEntregavel`]: o que o agente registrou (os campos, já conferidos contra
//!   o esquema do tipo), as regras citadas com a evidência copiada da regra (a
//!   evidência vem da regra, não do agente) e o texto montado pelo app.
//! - **Revisão final obrigatória**: todo entregável sai como rascunho. Enviar ou
//!   publicar exige [`Entregavel::marcar_revisado`]; um item novo, uma edição do
//!   analista ou "Gerar de novo" tiram a revisão e o entregável volta a rascunho
//!   ([`Situacao`]).
//! - [`EntregaveisDaDemanda`]: o estado, em
//!   `demandas/D-NNNN-<slug>/entregaveis.estado.json`, alterado sob trava
//!   ([`EntregaveisDaDemanda::alterar`]).
//!
//! No tipo vivo, o texto registrado vira a proposta de versão do documento vivo
//! da funcionalidade ([`crate::documentos_vivos`]), e [`Entregavel::documento`]
//! aponta para ele.

use std::fs::{self, OpenOptions};
use std::io;

use cba_index::Destino;
use cba_kb::frontmatter::Evidencia;
use cba_kb::{CaminhoRelativo, Id, IdFuncionalidade, layout};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Versão do formato do estado dos entregáveis da demanda.
pub const VERSAO_DOS_ENTREGAVEIS: u32 = 1;

// ---------------------------------------------------------------------------
// Modelo

/// Para que o rascunho é gerado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Finalidade {
    /// O entregável da demanda: no tipo por demanda, o rascunho a partir da
    /// análise; no tipo vivo, a proposta da versão seguinte do documento.
    #[default]
    Demanda,
    /// A v1.0 do documento vivo de uma funcionalidade que ainda não tem um,
    /// descrevendo o sistema como é hoje (engenharia reversa), antes da proposta
    /// da demanda.
    VersaoInicial,
}

/// Regra citada por um item, com a cópia do título e das evidências da regra
/// no registro.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegraCitada {
    pub id: Id,
    pub titulo: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidencias: Vec<Evidencia>,
}

/// Item registrado pelo agente num rascunho (uma história, o caso de uso).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemEntregavel {
    /// Número dentro do rascunho, a partir de 1.
    pub numero: u32,
    /// Os campos, como o esquema do tipo declara.
    pub campos: Value,
    pub regras: Vec<RegraCitada>,
    /// O item em Markdown, montado pelo app a partir dos campos.
    pub texto: String,
    pub registrado_ms: u64,
}

/// Uma geração do entregável.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rascunho {
    /// 1, 2, 3...: cada "Gerar de novo" abre a seguinte.
    pub versao: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub itens: Vec<ItemEntregavel>,
    /// Texto editado pelo analista. Sem ele, vale o texto montado dos itens.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub texto_editado: Option<String>,
    pub criado_ms: u64,
    #[serde(default)]
    pub atualizado_ms: u64,
}

impl Rascunho {
    /// Ainda sem item nem edição: o agente não registrou nada.
    pub fn vazio(&self) -> bool {
        self.itens.is_empty() && self.texto_editado.is_none()
    }

    /// O texto montado dos itens, um depois do outro.
    pub fn texto_dos_itens(&self) -> String {
        self.itens
            .iter()
            .map(|i| i.texto.trim())
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// O texto que vale: o editado pelo analista ou o montado dos itens.
    pub fn texto(&self) -> String {
        self.texto_editado
            .clone()
            .unwrap_or_else(|| self.texto_dos_itens())
    }

    /// Regras citadas pelos itens, sem repetição, na ordem em que aparecem.
    pub fn regras_citadas(&self) -> Vec<&Id> {
        let mut ids: Vec<&Id> = Vec::new();
        for r in self.itens.iter().flat_map(|i| &i.regras) {
            if !ids.contains(&&r.id) {
                ids.push(&r.id);
            }
        }
        ids
    }
}

/// Revisão final do analista.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Revisado {
    pub por: String,
    pub em_ms: u64,
    /// Versão do rascunho revisada.
    pub versao: u32,
}

/// Envio ou publicação de uma versão revisada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Envio {
    pub destino: Destino,
    /// Onde foi parar: o arquivo exportado, o caminho na base, a issue.
    pub onde: String,
    pub versao: u32,
    pub por: String,
    pub em_ms: u64,
}

/// Situação do entregável na tela da demanda.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Situacao {
    /// O rascunho atual ainda está vazio: aguarda o agente.
    Gerando,
    /// Tem conteúdo e ainda não passou pela revisão final.
    Rascunho,
    /// Revisado: pode ser enviado ou publicado.
    Revisado,
    /// A versão revisada já foi enviada ou publicada.
    Enviado,
}

/// Entregável gerado na demanda.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entregavel {
    /// Número dentro da demanda (`E-1`), nunca reaproveitado.
    pub numero: u32,
    /// Id do tipo no catálogo de entregáveis (`historia-usuario`).
    pub tipo: String,
    /// Análise validada de origem.
    pub analise: Id,
    #[serde(default)]
    pub finalidade: Finalidade,
    /// Funcionalidade do mapa (tipo vivo).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub funcionalidade: Option<IdFuncionalidade>,
    /// Documento vivo da proposta (tipo vivo), depois do primeiro registro.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub documento: Option<CaminhoRelativo>,
    /// Destino configurado para o tipo quando o entregável foi gerado.
    #[serde(default)]
    pub destino: Destino,
    /// Gerações, da mais antiga para a atual.
    pub rascunhos: Vec<Rascunho>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revisado: Option<Revisado>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub envios: Vec<Envio>,
    pub criado_ms: u64,
    #[serde(default)]
    pub atualizado_ms: u64,
}

/// O que gerar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NovoEntregavel {
    pub tipo: String,
    pub analise: Id,
    pub finalidade: Finalidade,
    pub funcionalidade: Option<IdFuncionalidade>,
    pub destino: Destino,
}

/// O que o agente registra num rascunho (já conferido por quem chama).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NovoItem {
    pub campos: Value,
    pub regras: Vec<RegraCitada>,
    pub texto: String,
}

/// Operação recusada num entregável. As mensagens se dirigem a quem chamou.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ErroEntregavel {
    #[error("o campo `{0}` não pode ficar vazio")]
    CampoVazio(&'static str),
    #[error("a demanda não tem o entregável E-{0}")]
    Desconhecido(u32),
    #[error(
        "o rascunho v{versao} do entregável E-{numero} ainda está vazio: espere o agente \
         registrar o conteúdo (ou gere de novo)"
    )]
    SemConteudo { numero: u32, versao: u32 },
    #[error(
        "o entregável E-{numero} ainda é rascunho: a revisão final é obrigatória antes de \
         enviar ou publicar (marque como revisado)"
    )]
    NaoRevisado { numero: u32 },
}

/// Falha ao ler ou gravar o estado dos entregáveis.
#[derive(Debug, thiserror::Error)]
pub enum ErroArquivoEntregaveis {
    #[error("não foi possível ler ou gravar `{arquivo}`: {erro}")]
    Io {
        arquivo: CaminhoRelativo,
        erro: io::Error,
    },
    #[error("`{arquivo}` não é um estado de entregáveis válido: {erro}")]
    Formato {
        arquivo: CaminhoRelativo,
        erro: serde_json::Error,
    },
}

fn exigir(campo: &'static str, valor: &str) -> Result<String, ErroEntregavel> {
    let v = valor.trim();
    if v.is_empty() {
        Err(ErroEntregavel::CampoVazio(campo))
    } else {
        Ok(v.to_owned())
    }
}

impl Entregavel {
    /// O rascunho atual (o da última geração).
    pub fn atual(&self) -> &Rascunho {
        self.rascunhos
            .last()
            .expect("todo entregável nasce com um rascunho")
    }

    fn atual_mut(&mut self) -> &mut Rascunho {
        self.rascunhos
            .last_mut()
            .expect("todo entregável nasce com um rascunho")
    }

    /// Um rascunho do histórico (os anteriores continuam acessíveis).
    pub fn rascunho(&self, versao: u32) -> Option<&Rascunho> {
        self.rascunhos.iter().find(|r| r.versao == versao)
    }

    pub fn situacao(&self) -> Situacao {
        let atual = self.atual();
        let Some(revisado) = &self.revisado else {
            return if atual.vazio() {
                Situacao::Gerando
            } else {
                Situacao::Rascunho
            };
        };
        let enviado = self
            .envios
            .iter()
            .any(|e| e.versao == revisado.versao && e.em_ms >= revisado.em_ms);
        if enviado {
            Situacao::Enviado
        } else {
            Situacao::Revisado
        }
    }

    /// Mesmo entregável: o mesmo tipo, a mesma análise, a mesma funcionalidade e
    /// a mesma finalidade. Gerar de novo um deles abre um rascunho novo nele.
    pub fn e_o_mesmo(&self, novo: &NovoEntregavel) -> bool {
        self.tipo == novo.tipo
            && self.analise == novo.analise
            && self.funcionalidade == novo.funcionalidade
            && self.finalidade == novo.finalidade
    }

    /// "Gerar de novo": abre o rascunho seguinte, vazio, sem apagar o atual. O
    /// entregável volta a rascunho.
    pub fn gerar_de_novo(&mut self, agora_ms: u64) -> u32 {
        let versao = self.atual().versao + 1;
        self.rascunhos.push(Rascunho {
            versao,
            itens: Vec::new(),
            texto_editado: None,
            criado_ms: agora_ms,
            atualizado_ms: agora_ms,
        });
        self.revisado = None;
        self.atualizado_ms = agora_ms;
        versao
    }

    /// Registra um item no rascunho atual e devolve o número dele. Com
    /// `substituir` (o tipo vivo, um documento só), o item toma o lugar dos que
    /// havia. O entregável volta a rascunho.
    pub fn registrar_item(
        &mut self,
        novo: NovoItem,
        substituir: bool,
        agora_ms: u64,
    ) -> Result<u32, ErroEntregavel> {
        let texto = exigir("texto", &novo.texto)?;
        let r = self.atual_mut();
        if substituir {
            r.itens.clear();
        }
        let numero = r.itens.iter().map(|i| i.numero).max().unwrap_or(0) + 1;
        r.itens.push(ItemEntregavel {
            numero,
            campos: novo.campos,
            regras: novo.regras,
            texto,
            registrado_ms: agora_ms,
        });
        // O texto montado muda: uma edição anterior deixaria o item de fora.
        r.texto_editado = None;
        r.atualizado_ms = agora_ms;
        self.revisado = None;
        self.atualizado_ms = agora_ms;
        Ok(numero)
    }

    /// Edição do analista no texto do rascunho atual. O entregável volta a
    /// rascunho, mesmo depois da revisão.
    pub fn editar(&mut self, texto: &str, agora_ms: u64) -> Result<(), ErroEntregavel> {
        let texto = exigir("texto", texto)?;
        let r = self.atual_mut();
        r.texto_editado = (texto != r.texto_dos_itens().trim()).then_some(texto);
        r.atualizado_ms = agora_ms;
        self.revisado = None;
        self.atualizado_ms = agora_ms;
        Ok(())
    }

    /// Revisão final do rascunho atual, por quem revisou.
    pub fn marcar_revisado(&mut self, por: &str, agora_ms: u64) -> Result<(), ErroEntregavel> {
        let por = exigir("revisor", por)?;
        let atual = self.atual();
        if atual.vazio() {
            return Err(ErroEntregavel::SemConteudo {
                numero: self.numero,
                versao: atual.versao,
            });
        }
        self.revisado = Some(Revisado {
            por,
            em_ms: agora_ms,
            versao: atual.versao,
        });
        self.atualizado_ms = agora_ms;
        Ok(())
    }

    /// Confere que o entregável pode ser enviado: revisado, na versão atual.
    pub fn conferir_envio(&self) -> Result<(), ErroEntregavel> {
        match &self.revisado {
            Some(r) if r.versao == self.atual().versao => Ok(()),
            _ => Err(ErroEntregavel::NaoRevisado {
                numero: self.numero,
            }),
        }
    }

    /// Registra o envio (ou a publicação) da versão revisada.
    pub fn registrar_envio(
        &mut self,
        destino: Destino,
        onde: &str,
        por: &str,
        agora_ms: u64,
    ) -> Result<&Envio, ErroEntregavel> {
        self.conferir_envio()?;
        let onde = exigir("onde", onde)?;
        let por = exigir("por", por)?;
        let versao = self.atual().versao;
        self.envios.push(Envio {
            destino,
            onde,
            versao,
            por,
            em_ms: agora_ms,
        });
        self.atualizado_ms = agora_ms;
        Ok(self.envios.last().expect("acabou de entrar"))
    }
}

// ---------------------------------------------------------------------------
// Estado em disco

/// Estado dos entregáveis de uma demanda.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntregaveisDaDemanda {
    pub formato: u32,
    #[serde(default)]
    pub entregaveis: Vec<Entregavel>,
    /// Sobe a cada alteração gravada.
    #[serde(default)]
    pub versao: u64,
    #[serde(default)]
    pub atualizado_ms: u64,
}

impl Default for EntregaveisDaDemanda {
    fn default() -> Self {
        Self {
            formato: VERSAO_DOS_ENTREGAVEIS,
            entregaveis: Vec::new(),
            versao: 0,
            atualizado_ms: 0,
        }
    }
}

/// `<pasta da demanda>/entregaveis.estado.json`.
pub fn arquivo_do_estado(pasta: &CaminhoRelativo) -> CaminhoRelativo {
    pasta
        .juntar(layout::ARQUIVO_ESTADO_ENTREGAVEIS)
        .expect("nome fixo válido")
}

fn arquivo_da_trava(pasta: &CaminhoRelativo) -> CaminhoRelativo {
    pasta
        .juntar(layout::ARQUIVO_TRAVA_ENTREGAVEIS)
        .expect("nome fixo válido")
}

impl EntregaveisDaDemanda {
    pub fn entregavel(&self, numero: u32) -> Option<&Entregavel> {
        self.entregaveis.iter().find(|e| e.numero == numero)
    }

    pub fn entregavel_mut(&mut self, numero: u32) -> Result<&mut Entregavel, ErroEntregavel> {
        self.entregaveis
            .iter_mut()
            .find(|e| e.numero == numero)
            .ok_or(ErroEntregavel::Desconhecido(numero))
    }

    /// "Gerar": abre o rascunho do entregável. Se a demanda já tem o mesmo
    /// entregável ([`Entregavel::e_o_mesmo`]), abre nele o rascunho seguinte
    /// ("Gerar de novo"), sem sobrescrever o anterior. Devolve o número.
    pub fn gerar(&mut self, novo: NovoEntregavel, agora_ms: u64) -> u32 {
        if let Some(e) = self.entregaveis.iter_mut().find(|e| e.e_o_mesmo(&novo)) {
            e.gerar_de_novo(agora_ms);
            e.destino = novo.destino;
            return e.numero;
        }
        let numero = self.entregaveis.iter().map(|e| e.numero).max().unwrap_or(0) + 1;
        self.entregaveis.push(Entregavel {
            numero,
            tipo: novo.tipo,
            analise: novo.analise,
            finalidade: novo.finalidade,
            funcionalidade: novo.funcionalidade,
            documento: None,
            destino: novo.destino,
            rascunhos: vec![Rascunho {
                versao: 1,
                itens: Vec::new(),
                texto_editado: None,
                criado_ms: agora_ms,
                atualizado_ms: agora_ms,
            }],
            revisado: None,
            envios: Vec::new(),
            criado_ms: agora_ms,
            atualizado_ms: agora_ms,
        });
        numero
    }

    /// Lê o estado da demanda na pasta (vazio, se ainda não existe).
    pub fn carregar(
        kb: &cba_kb::Projeto,
        pasta: &CaminhoRelativo,
    ) -> Result<Self, ErroArquivoEntregaveis> {
        let relativo = arquivo_do_estado(pasta);
        match fs::read(kb.caminho(&relativo)) {
            Ok(bytes) => {
                serde_json::from_slice(&bytes).map_err(|erro| ErroArquivoEntregaveis::Formato {
                    arquivo: kb.na_base(&relativo),
                    erro,
                })
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(erro) => Err(ErroArquivoEntregaveis::Io {
                arquivo: kb.na_base(&relativo),
                erro,
            }),
        }
    }

    /// Altera o estado sob a trava da demanda: lê, aplica `f` e, se ela não
    /// recusou, grava. Devolve o estado gravado e o resultado de `f`.
    pub fn alterar<T, E>(
        kb: &cba_kb::Projeto,
        pasta: &CaminhoRelativo,
        f: impl FnOnce(&mut Self, u64) -> Result<T, E>,
    ) -> Result<(Self, T), E>
    where
        E: From<ErroArquivoEntregaveis>,
    {
        let _trava = travar(kb, pasta)?;
        let mut estado = Self::carregar(kb, pasta)?;
        let agora = crate::tempo::agora_ms();
        let resultado = f(&mut estado, agora)?;
        estado.formato = VERSAO_DOS_ENTREGAVEIS;
        estado.versao += 1;
        estado.atualizado_ms = agora;
        estado.gravar(kb, pasta)?;
        Ok((estado, resultado))
    }

    fn gravar(
        &self,
        kb: &cba_kb::Projeto,
        pasta: &CaminhoRelativo,
    ) -> Result<(), ErroArquivoEntregaveis> {
        let relativo = arquivo_do_estado(pasta);
        let destino = kb.caminho(&relativo);
        let io_err = |erro| ErroArquivoEntregaveis::Io {
            arquivo: kb.na_base(&relativo),
            erro,
        };
        if let Some(p) = destino.parent() {
            fs::create_dir_all(p).map_err(io_err)?;
        }
        let mut json = serde_json::to_vec_pretty(self).expect("o estado serializa");
        json.push(b'\n');
        cba_kb::arquivo::gravar(&destino, &json).map_err(io_err)
    }
}

fn travar(
    kb: &cba_kb::Projeto,
    pasta: &CaminhoRelativo,
) -> Result<fs::File, ErroArquivoEntregaveis> {
    let relativo = arquivo_da_trava(pasta);
    let caminho = kb.caminho(&relativo);
    let io_err = |erro| ErroArquivoEntregaveis::Io {
        arquivo: kb.na_base(&relativo),
        erro,
    };
    if let Some(p) = caminho.parent() {
        fs::create_dir_all(p).map_err(io_err)?;
    }
    let arquivo = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&caminho)
        .map_err(io_err)?;
    // Liberada quando o arquivo é fechado.
    arquivo.lock().map_err(io_err)?;
    Ok(arquivo)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn novo(tipo: &str) -> NovoEntregavel {
        NovoEntregavel {
            tipo: tipo.into(),
            analise: Id::analisar("A-0001").unwrap(),
            finalidade: Finalidade::Demanda,
            funcionalidade: None,
            destino: Destino::Exportar,
        }
    }

    fn item(texto: &str) -> NovoItem {
        NovoItem {
            campos: json!({ "titulo": texto }),
            regras: vec![RegraCitada {
                id: Id::analisar("RN-FAT-001").unwrap(),
                titulo: "Multa de 2%".into(),
                evidencias: Vec::new(),
            }],
            texto: format!("## {texto}"),
        }
    }

    fn com_um() -> (EntregaveisDaDemanda, u32) {
        let mut d = EntregaveisDaDemanda::default();
        let n = d.gerar(novo("historia-usuario"), 10);
        (d, n)
    }

    #[test]
    fn nasce_gerando_e_vira_rascunho_com_o_primeiro_item() {
        let (mut d, n) = com_um();
        assert_eq!(n, 1);
        let e = d.entregavel_mut(n).unwrap();
        assert_eq!(e.situacao(), Situacao::Gerando);
        assert_eq!(e.registrar_item(item("Cancelar"), false, 11), Ok(1));
        assert_eq!(e.registrar_item(item("Reabrir"), false, 12), Ok(2));
        assert_eq!(e.situacao(), Situacao::Rascunho);
        assert_eq!(e.atual().texto(), "## Cancelar\n\n## Reabrir");
        assert_eq!(e.atual().regras_citadas().len(), 1, "sem repetir a regra");
        // O tipo vivo tem um documento só: o item novo toma o lugar.
        assert_eq!(e.registrar_item(item("Único"), true, 13), Ok(1));
        assert_eq!(e.atual().itens.len(), 1);
    }

    #[test]
    fn enviar_exige_a_revisao_final_e_a_edicao_volta_a_rascunho() {
        let (mut d, n) = com_um();
        let e = d.entregavel_mut(n).unwrap();
        assert_eq!(
            e.marcar_revisado("ana", 11),
            Err(ErroEntregavel::SemConteudo {
                numero: 1,
                versao: 1
            })
        );
        e.registrar_item(item("Cancelar"), false, 12).unwrap();
        assert_eq!(
            e.registrar_envio(Destino::Exportar, "/tmp/x.md", "ana", 13),
            Err(ErroEntregavel::NaoRevisado { numero: 1 })
        );
        e.marcar_revisado("ana", 14).unwrap();
        assert_eq!(e.situacao(), Situacao::Revisado);

        // Uma edição depois da revisão volta para rascunho.
        e.editar("## Cancelar a fatura", 15).unwrap();
        assert_eq!(e.situacao(), Situacao::Rascunho);
        assert_eq!(e.atual().texto(), "## Cancelar a fatura");
        assert!(e.conferir_envio().is_err());

        e.marcar_revisado("ana", 16).unwrap();
        let envio = e
            .registrar_envio(Destino::Exportar, "/tmp/x.md", "ana", 17)
            .unwrap();
        assert_eq!(envio.versao, 1);
        assert_eq!(e.situacao(), Situacao::Enviado);

        // Um item novo também tira a revisão (e a edição, que o deixaria de fora).
        e.registrar_item(item("Reabrir"), false, 18).unwrap();
        assert_eq!(e.situacao(), Situacao::Rascunho);
        assert_eq!(e.atual().texto_editado, None);
        assert_eq!(e.envios.len(), 1, "o envio anterior fica no histórico");
    }

    #[test]
    fn editar_para_o_texto_montado_desfaz_a_edicao() {
        let (mut d, n) = com_um();
        let e = d.entregavel_mut(n).unwrap();
        e.registrar_item(item("Cancelar"), false, 11).unwrap();
        e.editar("outro", 12).unwrap();
        e.editar("## Cancelar\n", 13).unwrap();
        assert_eq!(e.atual().texto_editado, None);
        assert_eq!(e.editar("  ", 14), Err(ErroEntregavel::CampoVazio("texto")));
    }

    #[test]
    fn gerar_de_novo_cria_versao_nova_sem_sobrescrever() {
        let (mut d, n) = com_um();
        {
            let e = d.entregavel_mut(n).unwrap();
            e.registrar_item(item("Cancelar"), false, 11).unwrap();
            e.marcar_revisado("ana", 12).unwrap();
        }
        // "Gerar" de novo o mesmo tipo da mesma análise: o mesmo entregável.
        assert_eq!(d.gerar(novo("historia-usuario"), 13), n);
        let e = d.entregavel(n).unwrap();
        assert_eq!(e.rascunhos.len(), 2);
        assert_eq!(e.atual().versao, 2);
        assert_eq!(e.situacao(), Situacao::Gerando);
        assert_eq!(e.revisado, None);
        assert_eq!(e.rascunho(1).unwrap().texto(), "## Cancelar", "a v1 fica");

        // Outro tipo, outro entregável.
        assert_eq!(d.gerar(novo("epico"), 14), 2);
        let mut vivo = novo("caso-de-uso");
        vivo.funcionalidade = Some(IdFuncionalidade::novo("FAT", 1).unwrap());
        assert_eq!(d.gerar(vivo.clone(), 15), 3);
        vivo.finalidade = Finalidade::VersaoInicial;
        assert_eq!(d.gerar(vivo, 16), 4, "a v1.0 é outro entregável");
        assert_eq!(
            d.entregavel_mut(9).unwrap_err(),
            ErroEntregavel::Desconhecido(9)
        );
    }

    #[test]
    fn o_estado_vai_e_volta_do_disco() {
        let dir = tempfile::tempdir().unwrap();
        let base = cba_kb::Base::criar(dir.path().join("base")).unwrap();
        let kb = base
            .criar_projeto(&cba_kb::Documento::novo(
                cba_kb::frontmatter::FrontmatterProjeto {
                    tipo: cba_kb::TipoDocumento::Projeto,
                    id: "p".into(),
                    nome: "Projeto P".into(),
                    areas: Vec::new(),
                    criado: None,
                },
                "# P\n",
            ))
            .unwrap();
        let pasta = CaminhoRelativo::novo("demandas/D-0001-cancelar").unwrap();
        assert_eq!(
            EntregaveisDaDemanda::carregar(&kb, &pasta).unwrap(),
            EntregaveisDaDemanda::default()
        );
        let (gravado, n) = EntregaveisDaDemanda::alterar(&kb, &pasta, |d, agora| {
            let n = d.gerar(novo("historia-usuario"), agora);
            d.entregavel_mut(n)?
                .registrar_item(item("Cancelar"), false, agora)?;
            Ok::<_, ErroAlteracao>(n)
        })
        .unwrap();
        assert_eq!(gravado.versao, 1);
        let lido = EntregaveisDaDemanda::carregar(&kb, &pasta).unwrap();
        assert_eq!(lido, gravado);
        assert_eq!(lido.entregavel(n).unwrap().atual().itens.len(), 1);

        // Uma recusa não grava nada.
        let recusa = EntregaveisDaDemanda::alterar(&kb, &pasta, |d, agora| {
            d.entregavel_mut(n)?.marcar_revisado("", agora)?;
            Ok::<_, ErroAlteracao>(())
        });
        assert!(recusa.is_err());
        assert_eq!(EntregaveisDaDemanda::carregar(&kb, &pasta).unwrap(), lido);
    }

    #[derive(Debug)]
    enum ErroAlteracao {
        #[allow(dead_code)]
        Recusa(ErroEntregavel),
        #[allow(dead_code)]
        Arquivo(ErroArquivoEntregaveis),
    }
    impl From<ErroEntregavel> for ErroAlteracao {
        fn from(e: ErroEntregavel) -> Self {
            Self::Recusa(e)
        }
    }
    impl From<ErroArquivoEntregaveis> for ErroAlteracao {
        fn from(e: ErroArquivoEntregaveis) -> Self {
            Self::Arquivo(e)
        }
    }
}
