//! Catálogo de funcionalidades do projeto (#208).
//!
//! O estado fica em `funcionalidades/catalogo.estado.json` (a fonte de verdade
//! dos identificadores); o documento `funcionalidades/catalogo.md` é montado dele
//! pelo template. Cada aprovação de uma análise do fluxo `mapear` funde o
//! resultado no catálogo ([`Catalogo::fundir`]):
//!
//! - a funcionalidade aceita com identificador do catálogo atualiza o item (o
//!   identificador, o documento negocial e a origem ficam); a que não existia
//!   entra como **nova**;
//! - a funcionalidade do catálogo descartada ou juntada a outra na análise sai do
//!   catálogo, e o identificador vai para os retirados (nunca é reaproveitado);
//! - o item do escopo da análise que ela não registrou continua, como
//!   **mantido** se algum ponto de entrada dele ainda existe no mapa, ou
//!   **sem ponto de entrada** se nenhum existe mais;
//! - itens fora do escopo não mudam.
//!
//! A numeração de um identificador novo é por sigla: o maior número do catálogo
//! (com os retirados) e das análises de mapeamento, mais um.

use std::collections::BTreeSet;
use std::fs;
use std::io;

use cba_kb::{CaminhoRelativo, Id, IdFuncionalidade, layout};
use serde::{Deserialize, Serialize};

use crate::analise::Analise;
use crate::funcionalidades::{EscopoMapa, Funcionalidade, PontoAtribuido, SituacaoFuncionalidade};

/// Versão do formato do estado do catálogo.
pub const VERSAO_DO_CATALOGO: u32 = 1;

/// Situação do item em relação ao último mapeamento que o tocou.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SituacaoNoCatalogo {
    /// Entrou no último mapeamento.
    Nova,
    /// Continua, com ponto de entrada.
    Mantida,
    /// Nenhum ponto de entrada dela existe mais no código.
    SemPontoDeEntrada,
}

impl SituacaoNoCatalogo {
    pub fn nome(self) -> &'static str {
        match self {
            Self::Nova => "nova",
            Self::Mantida => "mantida",
            Self::SemPontoDeEntrada => "sem ponto de entrada",
        }
    }
}

/// Funcionalidade do catálogo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemCatalogo {
    pub id: IdFuncionalidade,
    pub nome: String,
    pub area: String,
    pub area_nome: String,
    pub descricao: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub atores: Vec<String>,
    pub pontos: Vec<PontoAtribuido>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dados_lidos: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dados_gravados: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relacionadas: Vec<IdFuncionalidade>,
    pub situacao: SituacaoNoCatalogo,
    /// Documento negocial (`funcionalidades/<area>/<tema>/negocial.md`), quando a
    /// funcionalidade foi explicada e a explicação aprovada.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub documento: Option<CaminhoRelativo>,
    /// Análise do fluxo `explicar` ligada ao item (a mais recente).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explicacao: Option<Id>,
    /// Análise de mapeamento que criou o item.
    pub origem: Id,
    /// Última análise de mapeamento que o atualizou.
    pub atualizada_por: Id,
    pub atualizada_ms: u64,
}

impl ItemCatalogo {
    /// Já tem documento negocial aprovado.
    pub fn documentada(&self) -> bool {
        self.documento.is_some()
    }

    /// O item está no escopo de um mapeamento.
    pub fn no_escopo(&self, escopo: &EscopoMapa) -> bool {
        let na_area = escopo.area.as_ref().is_none_or(|a| a == &self.area);
        let na_pasta = escopo.pasta.is_none()
            || self
                .pontos
                .iter()
                .any(|p| escopo.contem_arquivo(p.evidencia.arquivo.as_str()));
        na_area && na_pasta
    }

    fn de(f: &Funcionalidade, analise: &Id, agora_ms: u64) -> Self {
        Self {
            id: f.id.clone(),
            nome: f.nome.clone(),
            area: f.area.clone(),
            area_nome: f.area_nome.clone(),
            descricao: f.descricao.clone(),
            atores: f.atores.clone(),
            pontos: f.pontos.clone(),
            dados_lidos: f.dados_lidos.clone(),
            dados_gravados: f.dados_gravados.clone(),
            relacionadas: f.relacionadas.clone(),
            situacao: SituacaoNoCatalogo::Nova,
            documento: None,
            explicacao: None,
            origem: analise.clone(),
            atualizada_por: analise.clone(),
            atualizada_ms: agora_ms,
        }
    }
}

/// Estado do catálogo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Catalogo {
    pub formato: u32,
    #[serde(default)]
    pub funcionalidades: Vec<ItemCatalogo>,
    /// Identificadores que saíram do catálogo (não são reaproveitados).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub retirados: Vec<IdFuncionalidade>,
    /// Última análise de mapeamento fundida.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origem: Option<Id>,
    #[serde(default)]
    pub atualizado_ms: u64,
}

impl Default for Catalogo {
    fn default() -> Self {
        Self {
            formato: VERSAO_DO_CATALOGO,
            funcionalidades: Vec::new(),
            retirados: Vec::new(),
            origem: None,
            atualizado_ms: 0,
        }
    }
}

/// O que uma fusão mudou no catálogo.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fusao {
    pub novas: Vec<IdFuncionalidade>,
    pub atualizadas: Vec<IdFuncionalidade>,
    pub retiradas: Vec<IdFuncionalidade>,
    pub sem_ponto_de_entrada: Vec<IdFuncionalidade>,
}

/// Falha ao ler ou gravar o estado do catálogo.
#[derive(Debug, thiserror::Error)]
pub enum ErroCatalogo {
    #[error("não foi possível ler ou gravar `{arquivo}`: {erro}")]
    Io {
        arquivo: CaminhoRelativo,
        erro: io::Error,
    },
    #[error("`{arquivo}` não é um estado de catálogo válido: {erro}")]
    Formato {
        arquivo: CaminhoRelativo,
        erro: serde_json::Error,
    },
}

impl Catalogo {
    pub fn item(&self, id: &IdFuncionalidade) -> Option<&ItemCatalogo> {
        self.funcionalidades.iter().find(|f| &f.id == id)
    }

    pub fn item_mut(&mut self, id: &IdFuncionalidade) -> Option<&mut ItemCatalogo> {
        self.funcionalidades.iter_mut().find(|f| &f.id == id)
    }

    /// Item que tem o ponto do mapa.
    pub fn por_chave(&self, chave: &str) -> Option<&ItemCatalogo> {
        self.funcionalidades
            .iter()
            .find(|f| f.pontos.iter().any(|p| p.chave.as_deref() == Some(chave)))
    }

    /// Maior número usado com a sigla, contando os retirados.
    pub fn maior_numero(&self, sigla: &str) -> u32 {
        self.funcionalidades
            .iter()
            .map(|f| &f.id)
            .chain(&self.retirados)
            .filter(|id| id.sigla() == sigla)
            .map(IdFuncionalidade::numero)
            .max()
            .unwrap_or(0)
    }

    /// Funcionalidades em ordem de área e identificador.
    pub fn ordenadas(&self) -> Vec<&ItemCatalogo> {
        let mut v: Vec<&ItemCatalogo> = self.funcionalidades.iter().collect();
        v.sort_by(|a, b| (&a.area_nome, &a.id).cmp(&(&b.area_nome, &b.id)));
        v
    }

    /// Funde uma análise de mapeamento aprovada. `chaves_do_mapa` são as chaves
    /// dos pontos de entrada que o mapa atual acha (para saber quem perdeu o
    /// ponto de entrada). Só as funcionalidades aceitas entram.
    pub fn fundir(
        &mut self,
        analise: &Analise,
        chaves_do_mapa: &BTreeSet<String>,
        agora_ms: u64,
    ) -> Fusao {
        let mut fusao = Fusao::default();
        let mut tocados: BTreeSet<IdFuncionalidade> = BTreeSet::new();
        for f in &analise.funcionalidades {
            match f.situacao {
                SituacaoFuncionalidade::Aceita => {
                    tocados.insert(f.id.clone());
                    match self.item_mut(&f.id) {
                        Some(item) => {
                            let novo = ItemCatalogo::de(f, &analise.id, agora_ms);
                            *item = ItemCatalogo {
                                situacao: SituacaoNoCatalogo::Mantida,
                                documento: item.documento.take(),
                                explicacao: item.explicacao.take(),
                                origem: item.origem.clone(),
                                ..novo
                            };
                            fusao.atualizadas.push(f.id.clone());
                        }
                        None => {
                            self.funcionalidades
                                .push(ItemCatalogo::de(f, &analise.id, agora_ms));
                            self.retirados.retain(|r| r != &f.id);
                            fusao.novas.push(f.id.clone());
                        }
                    }
                }
                SituacaoFuncionalidade::Descartada | SituacaoFuncionalidade::Juntada => {
                    if self.item(&f.id).is_some() {
                        tocados.insert(f.id.clone());
                        self.funcionalidades.retain(|i| i.id != f.id);
                        if !self.retirados.contains(&f.id) {
                            self.retirados.push(f.id.clone());
                        }
                        fusao.retiradas.push(f.id.clone());
                    }
                }
                SituacaoFuncionalidade::Pendente => {}
            }
        }
        let escopo = analise.escopo.clone().unwrap_or_default();
        for item in &mut self.funcionalidades {
            if tocados.contains(&item.id) || !item.no_escopo(&escopo) {
                continue;
            }
            let com_chave: Vec<&str> = item
                .pontos
                .iter()
                .filter_map(|p| p.chave.as_deref())
                .collect();
            let sem_chave = item.pontos.iter().any(|p| p.chave.is_none());
            let algum_existe = sem_chave || com_chave.iter().any(|c| chaves_do_mapa.contains(*c));
            item.situacao = if algum_existe {
                SituacaoNoCatalogo::Mantida
            } else {
                fusao.sem_ponto_de_entrada.push(item.id.clone());
                SituacaoNoCatalogo::SemPontoDeEntrada
            };
        }
        // Relacionadas que saíram do catálogo deixam de ser citadas.
        let existentes: BTreeSet<IdFuncionalidade> =
            self.funcionalidades.iter().map(|f| f.id.clone()).collect();
        for item in &mut self.funcionalidades {
            item.relacionadas.retain(|r| existentes.contains(r));
        }
        self.origem = Some(analise.id.clone());
        self.atualizado_ms = agora_ms;
        fusao
    }

    /// Liga o documento negocial aprovado de uma explicação ao item. Devolve se
    /// o item existe.
    pub fn ligar_documento(
        &mut self,
        id: &IdFuncionalidade,
        explicacao: &Id,
        documento: CaminhoRelativo,
    ) -> bool {
        match self.item_mut(id) {
            Some(item) => {
                item.explicacao = Some(explicacao.clone());
                item.documento = Some(documento);
                true
            }
            None => false,
        }
    }

    /// Lê o estado do catálogo do projeto (vazio se ainda não existe).
    pub fn carregar(kb: &cba_kb::Projeto) -> Result<Self, ErroCatalogo> {
        let relativo = layout::estado_do_catalogo();
        match fs::read(kb.caminho(&relativo)) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|erro| ErroCatalogo::Formato {
                arquivo: kb.na_base(&relativo),
                erro,
            }),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(erro) => Err(ErroCatalogo::Io {
                arquivo: kb.na_base(&relativo),
                erro,
            }),
        }
    }

    /// Grava o estado (gravação atômica).
    pub fn gravar(&self, kb: &cba_kb::Projeto) -> Result<(), ErroCatalogo> {
        let relativo = layout::estado_do_catalogo();
        let destino = kb.caminho(&relativo);
        let io_err = |erro| ErroCatalogo::Io {
            arquivo: kb.na_base(&relativo),
            erro,
        };
        if let Some(pasta) = destino.parent() {
            fs::create_dir_all(pasta).map_err(io_err)?;
        }
        let mut json = serde_json::to_vec_pretty(self).expect("o catálogo serializa");
        json.push(b'\n');
        cba_kb::arquivo::gravar(&destino, &json).map_err(io_err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analise::{EstadoEvidencia, Evidencia, Fluxo};
    use crate::funcionalidades::{DecisaoFuncionalidade, NovaFuncionalidade};
    use cba_kb::Serie;
    use cba_kb::frontmatter::Linhas;

    fn f(n: u32) -> IdFuncionalidade {
        IdFuncionalidade::novo("FAT", n).unwrap()
    }

    fn ponto(chave: &str) -> PontoAtribuido {
        let (arquivo, _) = chave.split_once('#').unwrap();
        PontoAtribuido {
            chave: Some(chave.into()),
            tipo: None,
            nome: chave.into(),
            evidencia: Evidencia {
                arquivo: CaminhoRelativo::novo(arquivo).unwrap(),
                linhas: Linhas::novo(1, 1).unwrap(),
                trecho: "x".into(),
                hash: "h".into(),
                commit: None,
                estado: EstadoEvidencia::Valida,
            },
        }
    }

    fn mapeamento(n: u32, itens: &[(u32, &str, &[&str], DecisaoFuncionalidade)]) -> Analise {
        let mut a = Analise::nova(Serie::Analise.id(n), "p", Fluxo::Mapear, "Mapa", "", 1);
        for (id, nome, pontos, decisao) in itens {
            a.registrar_funcionalidade(
                NovaFuncionalidade {
                    nome: (*nome).into(),
                    area: "cobranca".into(),
                    area_nome: "Cobrança".into(),
                    descricao: "D.".into(),
                    atores: vec![],
                    pontos: pontos.iter().map(|c| ponto(c)).collect(),
                    dados_lidos: vec![],
                    dados_gravados: vec![],
                    relacionadas: vec![],
                    do_catalogo: false,
                    sessao: None,
                },
                f(*id),
                1,
            )
            .unwrap();
            a.decidir_funcionalidade(&f(*id), *decisao).unwrap();
        }
        a
    }

    fn chaves(c: &[&str]) -> BTreeSet<String> {
        c.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn fundir_mantem_ids_e_marca_novas_e_sem_ponto() {
        use DecisaoFuncionalidade::*;
        let mut cat = Catalogo::default();
        let primeira = mapeamento(
            1,
            &[
                (1, "Cobrar", &["db/a.sql#A"], Aceitar),
                (2, "Cancelar", &["db/a.sql#B"], Aceitar),
                (3, "Lixo", &["db/a.sql#C"], Descartar),
            ],
        );
        let fusao = cat.fundir(&primeira, &chaves(&["db/a.sql#A", "db/a.sql#B"]), 10);
        assert_eq!(fusao.novas, vec![f(1), f(2)]);
        assert_eq!(cat.funcionalidades.len(), 2);
        assert_eq!(cat.maior_numero("FAT"), 2);

        // Segunda rodada: F-FAT-001 de novo, F-FAT-002 não registrada e sem ponto
        // no mapa, F-FAT-004 nova.
        cat.ligar_documento(
            &f(1),
            &Serie::Analise.id(5),
            CaminhoRelativo::novo("funcionalidades/cobranca/cobrar/negocial.md").unwrap(),
        );
        let segunda = mapeamento(
            2,
            &[
                (1, "Cobrar fatura", &["db/a.sql#A"], Aceitar),
                (4, "Parcelar", &["db/a.sql#D"], Aceitar),
            ],
        );
        let fusao = cat.fundir(&segunda, &chaves(&["db/a.sql#A", "db/a.sql#D"]), 20);
        assert_eq!(fusao.atualizadas, vec![f(1)]);
        assert_eq!(fusao.novas, vec![f(4)]);
        assert_eq!(fusao.sem_ponto_de_entrada, vec![f(2)]);
        let um = cat.item(&f(1)).unwrap();
        assert_eq!(um.nome, "Cobrar fatura");
        assert_eq!(um.situacao, SituacaoNoCatalogo::Mantida);
        assert!(um.documentada());
        assert_eq!(um.origem, Serie::Analise.id(1));
        assert_eq!(um.atualizada_por, Serie::Analise.id(2));
        assert_eq!(
            cat.item(&f(2)).unwrap().situacao,
            SituacaoNoCatalogo::SemPontoDeEntrada
        );

        // Terceira: F-FAT-002 descartada sai e o id não volta.
        let mut terceira = mapeamento(3, &[(2, "Cancelar", &["db/a.sql#B"], Aceitar)]);
        terceira.funcionalidades[0].do_catalogo = true;
        terceira.decidir_funcionalidade(&f(2), Descartar).unwrap();
        let fusao = cat.fundir(&terceira, &chaves(&["db/a.sql#A", "db/a.sql#D"]), 30);
        assert_eq!(fusao.retiradas, vec![f(2)]);
        assert!(cat.item(&f(2)).is_none());
        assert_eq!(cat.maior_numero("FAT"), 4);
        assert!(cat.retirados.contains(&f(2)));
    }

    #[test]
    fn escopo_por_pasta_nao_toca_o_resto() {
        use DecisaoFuncionalidade::*;
        let mut cat = Catalogo::default();
        let primeira = mapeamento(
            1,
            &[
                (1, "Cobrar", &["db/a.sql#A"], Aceitar),
                (2, "Tela", &["front/t.ts#T"], Aceitar),
            ],
        );
        cat.fundir(&primeira, &chaves(&["db/a.sql#A", "front/t.ts#T"]), 1);
        let mut segunda = mapeamento(2, &[]);
        segunda.escopo = Some(EscopoMapa {
            pasta: Some("db".into()),
            area: None,
        });
        // O mapa não tem mais nada, mas só `db` está no escopo.
        let fusao = cat.fundir(&segunda, &BTreeSet::new(), 2);
        assert_eq!(fusao.sem_ponto_de_entrada, vec![f(1)]);
        assert_eq!(cat.item(&f(2)).unwrap().situacao, SituacaoNoCatalogo::Nova);
    }

    #[test]
    fn grava_e_le_o_estado() {
        let tmp = tempfile::tempdir().unwrap();
        let base = cba_kb::Base::criar(tmp.path().join("base")).unwrap();
        let doc = cba_kb::Documento::novo(
            cba_kb::frontmatter::FrontmatterProjeto {
                tipo: cba_kb::TipoDocumento::Projeto,
                id: "p".into(),
                nome: "Projeto P".into(),
                areas: Vec::new(),
                criado: None,
            },
            "# P\n",
        );
        let kb = base.criar_projeto(&doc).unwrap();
        assert_eq!(Catalogo::carregar(&kb).unwrap(), Catalogo::default());
        let mut cat = Catalogo::default();
        let a = mapeamento(
            1,
            &[(1, "Cobrar", &["db/a.sql#A"], DecisaoFuncionalidade::Aceitar)],
        );
        cat.fundir(&a, &BTreeSet::new(), 1);
        cat.gravar(&kb).unwrap();
        assert_eq!(Catalogo::carregar(&kb).unwrap(), cat);
    }
}
