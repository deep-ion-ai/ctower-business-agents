//! Demandas do projeto: uma pasta por demanda, `demandas/D-NNNN-<slug>/`, com o
//! `demanda.md` e, depois, as atas das reuniões (épico #93).
//!
//! As análises apontam para a demanda pelo campo `demanda.id` do frontmatter; a
//! pasta da demanda não guarda cópia delas (o índice responde quais são, em
//! [`crate::indice::Indice::documentos_da_demanda`]).
//!
//! ## Reserva do número
//!
//! O app e o servidor MCP podem criar demandas ao mesmo tempo. A pasta sem slug
//! (`demandas/D-NNNN`) serve de trava: `create_dir` é atômico, então só um
//! processo a obtém. Quem a obtém confere se o número já foi usado por outra pasta
//! (`D-NNNN-<slug>` de quem criou antes), grava o `demanda.md` dentro dela e só
//! então a renomeia para `D-NNNN-<slug>`. Quem chega depois vê a pasta renomeada e
//! tenta o número seguinte. Se a renomeação falhar, a demanda continua válida na
//! pasta sem slug.

use std::fs;
use std::io;

use crate::base::Projeto;
use crate::caminho::{CaminhoRelativo, slug};
use crate::documento::Documento;
use crate::erro::{Erro, Resultado};
use crate::frontmatter::{
    Data, ErroCampo, Frontmatter, FrontmatterDemanda, StatusDemanda, TipoDocumento,
};
use crate::id::IdDemanda;
use crate::layout;

/// Quantos números seguidos [`Projeto::criar_demanda`] tenta antes de desistir.
const TENTATIVAS_DE_RESERVA: u32 = 1000;

/// Tamanho máximo do slug no nome da pasta, para caber nos limites de caminho
/// do Windows.
const MAXIMO_DO_SLUG: usize = 60;

/// Dados para criar uma demanda.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NovaDemanda {
    pub titulo: String,
    /// O pedido do cliente, no corpo do documento.
    pub descricao: Option<String>,
    /// Chave no rastreador (`PROJ-123`, `#12`).
    pub chave: Option<String>,
    /// Endereço da demanda no rastreador (URL da issue).
    pub origem: Option<String>,
}

/// Demanda lida da base.
#[derive(Debug, Clone, PartialEq)]
pub struct DemandaLida {
    pub id: IdDemanda,
    /// Pasta da demanda, relativa à raiz do projeto (`demandas/D-0001-cancelar-nota`).
    pub pasta: CaminhoRelativo,
    /// `demanda.md`, relativo à raiz do projeto.
    pub arquivo: CaminhoRelativo,
    pub documento: Documento<FrontmatterDemanda>,
}

impl DemandaLida {
    pub fn aberta(&self) -> bool {
        self.documento.frontmatter.status == StatusDemanda::Aberta
    }
}

pub(crate) fn pasta_das_demandas() -> CaminhoRelativo {
    CaminhoRelativo::novo(layout::DEMANDAS).expect("nome fixo válido")
}

/// Slug do título para o nome da pasta, cortado em [`MAXIMO_DO_SLUG`].
fn slug_da_pasta(titulo: &str) -> String {
    let mut s = slug(titulo);
    if s.len() > MAXIMO_DO_SLUG {
        // O slug só tem a-z, 0-9 e `-`: qualquer posição é fronteira de caractere.
        s.truncate(MAXIMO_DO_SLUG);
        s.truncate(s.trim_end_matches('-').len());
    }
    s
}

fn corpo_inicial(nova: &NovaDemanda) -> String {
    let titulo = nova.titulo.trim();
    let descricao = match nova.descricao.as_deref().map(str::trim) {
        Some(d) if !d.is_empty() => d.to_owned(),
        _ => "<!-- O pedido do cliente, com as palavras dele. -->".to_owned(),
    };
    format!(
        "# {titulo}\n\n## Descrição\n\n{descricao}\n\n## Próximo passo\n\n\
         <!-- O que fazer a seguir nesta demanda. -->\n"
    )
}

fn texto_opcional(texto: &Option<String>) -> Option<String> {
    texto
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
}

impl Projeto {
    /// Nomes das pastas em `demandas/` com o identificador de cada uma, em ordem
    /// de nome. Sem a pasta `demandas/`, lista vazia.
    pub(crate) fn pastas_de_demanda(&self) -> Resultado<Vec<(IdDemanda, String)>> {
        let relativo = pasta_das_demandas();
        let entradas = match fs::read_dir(self.caminho(&relativo)) {
            Ok(e) => e,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(erro) => return Err(self.erro_io(&relativo, erro)),
        };
        let mut pastas: Vec<_> = entradas
            .flatten()
            .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
            .filter_map(|e| e.file_name().into_string().ok())
            .filter_map(|nome| IdDemanda::de_pasta(&nome).map(|id| (id, nome)))
            .collect();
        pastas.sort_by(|a, b| a.1.cmp(&b.1));
        Ok(pastas)
    }

    /// `demanda.md` da demanda, relativo à raiz do projeto. Sem a pasta, devolve o
    /// caminho da pasta sem slug (`demandas/D-NNNN/demanda.md`), que não existe:
    /// serve de destino para uma ligação quebrada no índice.
    pub fn arquivo_da_demanda(&self, id: IdDemanda) -> CaminhoRelativo {
        let nome = self
            .pastas_de_demanda()
            .unwrap_or_default()
            .into_iter()
            .filter(|(outro, _)| *outro == id)
            .map(|(_, nome)| nome)
            .find(|nome| {
                pasta_das_demandas()
                    .juntar(&format!("{nome}/{}", layout::ARQUIVO_DEMANDA))
                    .is_ok_and(|a| self.existe(&a))
            })
            .unwrap_or_else(|| id.to_string());
        pasta_das_demandas()
            .juntar(&format!("{nome}/{}", layout::ARQUIVO_DEMANDA))
            .expect("nome de pasta de demanda é caminho válido")
    }

    /// Próximo identificador livre, sem reservar. Para criar, use
    /// [`Projeto::criar_demanda`].
    pub fn proxima_demanda(&self) -> Resultado<IdDemanda> {
        let maior = self
            .pastas_de_demanda()?
            .iter()
            .map(|(id, _)| id.numero())
            .max()
            .unwrap_or(0);
        Ok(IdDemanda::novo(maior + 1))
    }

    /// Cria uma demanda aberta com o próximo número livre, em
    /// `demandas/D-NNNN-<slug do título>/demanda.md`. Veja a reserva no
    /// [módulo](self).
    pub fn criar_demanda(&self, nova: &NovaDemanda, hoje: Data) -> Resultado<DemandaLida> {
        let titulo = nova.titulo.trim().to_owned();
        let montar = |id: IdDemanda| {
            Documento::novo(
                FrontmatterDemanda {
                    tipo: TipoDocumento::Demanda,
                    id,
                    titulo: titulo.clone(),
                    projeto: self.nome().to_owned(),
                    status: StatusDemanda::Aberta,
                    chave: texto_opcional(&nova.chave),
                    origem: texto_opcional(&nova.origem),
                    criado: hoje,
                    encerrado: None,
                },
                corpo_inicial(nova),
            )
        };
        let demandas = pasta_das_demandas();
        // Valida antes de reservar, para não deixar pasta vazia por um título vazio.
        montar(IdDemanda::novo(1)).frontmatter.validar().map_err(
            |ErroCampo { campo, mensagem }| Erro::Frontmatter {
                arquivo: self.na_base(&demandas),
                erro: crate::ErroFrontmatter {
                    campo: Some(campo),
                    linha: None,
                    mensagem,
                },
            },
        )?;
        fs::create_dir_all(self.caminho(&demandas)).map_err(|e| self.erro_io(&demandas, e))?;

        let sufixo = slug_da_pasta(&titulo);
        let primeiro = self.proxima_demanda()?.numero();
        for numero in primeiro..primeiro.saturating_add(TENTATIVAS_DE_RESERVA) {
            let id = IdDemanda::novo(numero);
            let trava = demandas.juntar(&id.to_string())?;
            match fs::create_dir(self.caminho(&trava)) {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(erro) => return Err(self.erro_io(&trava, erro)),
            }
            let nome_da_trava = id.to_string();
            if self
                .pastas_de_demanda()?
                .iter()
                .any(|(outro, nome)| *outro == id && *nome != nome_da_trava)
            {
                // Número já usado por uma demanda criada antes da trava.
                let _ = fs::remove_dir(self.caminho(&trava));
                continue;
            }
            let documento = montar(id);
            if let Err(e) = self.criar(&trava.juntar(layout::ARQUIVO_DEMANDA)?, &documento) {
                let _ = fs::remove_dir(self.caminho(&trava));
                return Err(e);
            }
            let mut pasta = trava.clone();
            if !sufixo.is_empty() {
                let destino = demandas.juntar(&format!("{id}-{sufixo}"))?;
                match fs::rename(self.caminho(&trava), self.caminho(&destino)) {
                    Ok(()) => pasta = destino,
                    Err(erro) => tracing::warn!(
                        demanda = %id,
                        %erro,
                        "pasta da demanda ficou sem o slug do título"
                    ),
                }
            }
            let arquivo = pasta.juntar(layout::ARQUIVO_DEMANDA)?;
            return Ok(DemandaLida {
                id,
                pasta,
                arquivo,
                documento,
            });
        }
        Err(Erro::JaExiste(self.na_base(&demandas)))
    }

    /// Lê a demanda de uma pasta (relativa a `demandas/`). `None` se a pasta não
    /// tem `demanda.md` (uma criação em andamento).
    fn ler_demanda_da_pasta(&self, id: IdDemanda, nome: &str) -> Resultado<Option<DemandaLida>> {
        let pasta = pasta_das_demandas().juntar(nome)?;
        let arquivo = pasta.juntar(layout::ARQUIVO_DEMANDA)?;
        if !self.existe(&arquivo) {
            return Ok(None);
        }
        let documento: Documento<FrontmatterDemanda> = self.ler(&arquivo)?;
        if documento.frontmatter.id != id {
            return Err(Erro::Frontmatter {
                arquivo: self.na_base(&arquivo),
                erro: crate::ErroFrontmatter {
                    campo: Some("id".into()),
                    linha: None,
                    mensagem: format!(
                        "o id `{}` precisa ser igual ao da pasta (`{id}`)",
                        documento.frontmatter.id
                    ),
                },
            });
        }
        Ok(Some(DemandaLida {
            id,
            pasta,
            arquivo,
            documento,
        }))
    }

    /// Demandas do projeto: as abertas primeiro e, em cada grupo, as mais novas
    /// primeiro. Uma demanda que não pôde ser lida fica de fora, com um aviso no
    /// log (o índice a lista em `problemas`).
    pub fn demandas(&self) -> Resultado<Vec<DemandaLida>> {
        let mut saida = Vec::new();
        let mut vistas = std::collections::HashSet::new();
        for (id, nome) in self.pastas_de_demanda()? {
            if vistas.contains(&id) {
                continue;
            }
            match self.ler_demanda_da_pasta(id, &nome) {
                Ok(Some(d)) => {
                    vistas.insert(id);
                    saida.push(d);
                }
                Ok(None) => {}
                Err(erro) => tracing::warn!(demanda = %id, %erro, "demanda ilegível"),
            }
        }
        saida.sort_by(|a, b| b.aberta().cmp(&a.aberta()).then(b.id.cmp(&a.id)));
        Ok(saida)
    }

    /// Lê uma demanda pelo identificador.
    pub fn demanda(&self, id: IdDemanda) -> Resultado<DemandaLida> {
        for (outro, nome) in self.pastas_de_demanda()? {
            if outro == id
                && let Some(d) = self.ler_demanda_da_pasta(id, &nome)?
            {
                return Ok(d);
            }
        }
        Err(Erro::DemandaInexistente {
            projeto: self.nome().to_owned(),
            id,
        })
    }

    /// Encerra a demanda na data `hoje`. Encerrar uma demanda já encerrada mantém
    /// a data original.
    pub fn encerrar_demanda(&self, id: IdDemanda, hoje: Data) -> Resultado<DemandaLida> {
        self.mudar_status(id, |f| {
            if f.status != StatusDemanda::Encerrada {
                f.status = StatusDemanda::Encerrada;
                f.encerrado = Some(hoje);
            }
        })
    }

    /// Reabre uma demanda encerrada, apagando a data de encerramento.
    pub fn reabrir_demanda(&self, id: IdDemanda) -> Resultado<DemandaLida> {
        self.mudar_status(id, |f| {
            f.status = StatusDemanda::Aberta;
            f.encerrado = None;
        })
    }

    fn mudar_status(
        &self,
        id: IdDemanda,
        mudar: impl FnOnce(&mut FrontmatterDemanda),
    ) -> Resultado<DemandaLida> {
        let mut demanda = self.demanda(id)?;
        let antes = demanda.documento.frontmatter.clone();
        mudar(&mut demanda.documento.frontmatter);
        if demanda.documento.frontmatter != antes {
            self.gravar(&demanda.arquivo, &demanda.documento)?;
        }
        Ok(demanda)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_cortado_sem_hifen_no_fim() {
        assert_eq!(
            slug_da_pasta("Cancelar nota fiscal"),
            "cancelar-nota-fiscal"
        );
        let longo = "palavra ".repeat(20);
        let s = slug_da_pasta(&longo);
        assert!(s.len() <= MAXIMO_DO_SLUG);
        assert!(!s.ends_with('-'));
        assert_eq!(slug_da_pasta("??"), "");
    }
}
