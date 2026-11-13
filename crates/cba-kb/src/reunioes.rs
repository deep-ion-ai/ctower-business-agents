//! Reuniões de uma demanda (épico #93): a ata em
//! `demandas/D-NNNN-<slug>/reunioes/R-NNNN.md` e o estado dela ao lado, em
//! `R-NNNN.estado.json`.
//!
//! O estado (participantes, itens, citações) é a fonte da ata: quem registra um
//! item grava o estado e monta a ata de novo. O formato do estado e a montagem
//! ficam em `cba-docs`; aqui ficam só os caminhos e a reserva do número.
//!
//! ## Reserva do número
//!
//! O número `R-NNNN` é sequencial por projeto, não por demanda, e o app e o
//! servidor MCP podem criar reuniões ao mesmo tempo, em demandas diferentes. Por
//! isso a trava fica em `demandas/`, fora das pastas das demandas: a pasta oculta
//! `demandas/.R-NNNN.reserva`. `create_dir` é atômico, então só um processo a
//! obtém. Quem a obtém confere se o número já foi usado em alguma demanda, grava
//! o `R-NNNN.estado.json` na pasta da demanda e só então solta a trava. Quem
//! chega depois e obtém a mesma trava vê o estado gravado e tenta o número
//! seguinte. Uma trava que sobrar de uma queda só faz pular aquele número.

use std::fs;
use std::io;

use crate::arquivo;
use crate::base::Projeto;
use crate::caminho::CaminhoRelativo;
use crate::demandas::pasta_das_demandas;
use crate::erro::{Erro, Resultado};
use crate::id::{IdDemanda, IdReuniao};
use crate::layout;

/// Quantos números seguidos [`Projeto::criar_reuniao`] tenta antes de desistir.
const TENTATIVAS_DE_RESERVA: u32 = 1000;

/// Sufixo da pasta oculta que trava um número durante a criação.
const SUFIXO_DA_TRAVA: &str = ".reserva";

/// Arquivos de uma reunião, relativos à raiz do projeto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArquivosDaReuniao {
    pub id: IdReuniao,
    pub demanda: IdDemanda,
    /// `demandas/D-NNNN-<slug>/reunioes`.
    pub pasta: CaminhoRelativo,
    /// `demandas/D-NNNN-<slug>/reunioes/R-NNNN.md`.
    pub ata: CaminhoRelativo,
    /// `demandas/D-NNNN-<slug>/reunioes/R-NNNN.estado.json`.
    pub estado: CaminhoRelativo,
}

impl ArquivosDaReuniao {
    fn novo(id: IdReuniao, demanda: IdDemanda, pasta: CaminhoRelativo) -> Resultado<Self> {
        Ok(Self {
            ata: pasta.juntar(&format!("{id}.{}", layout::EXTENSAO))?,
            estado: pasta.juntar(&format!("{id}{}", layout::SUFIXO_ESTADO_ATA))?,
            id,
            demanda,
            pasta,
        })
    }
}

/// Identificador da reunião pelo nome de um arquivo da pasta `reunioes/`
/// (`R-0003.md` ou `R-0003.estado.json`).
fn id_do_arquivo(nome: &str) -> Option<IdReuniao> {
    let raiz = nome
        .strip_suffix(layout::SUFIXO_ESTADO_ATA)
        .or_else(|| nome.strip_suffix(&format!(".{}", layout::EXTENSAO)))?;
    IdReuniao::analisar(raiz).ok()
}

/// Remove a trava ao sair, em qualquer caminho.
struct Trava(std::path::PathBuf);

impl Drop for Trava {
    fn drop(&mut self) {
        let _ = fs::remove_dir(&self.0);
    }
}

impl Projeto {
    /// Reuniões de uma pasta de demanda (relativa a `demandas/`), em ordem de
    /// número. Sem a pasta `reunioes/`, lista vazia.
    fn reunioes_da_pasta(
        &self,
        demanda: IdDemanda,
        nome: &str,
    ) -> Resultado<Vec<ArquivosDaReuniao>> {
        let pasta = pasta_das_demandas().juntar(&format!("{nome}/{}", layout::REUNIOES))?;
        let entradas = match fs::read_dir(self.caminho(&pasta)) {
            Ok(e) => e,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(erro) => return Err(self.erro_io(&pasta, erro)),
        };
        let mut ids: Vec<IdReuniao> = entradas
            .flatten()
            .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
            .filter_map(|e| e.file_name().into_string().ok())
            .filter_map(|n| id_do_arquivo(&n))
            .collect();
        ids.sort_unstable();
        ids.dedup();
        ids.into_iter()
            .map(|id| ArquivosDaReuniao::novo(id, demanda, pasta.clone()))
            .collect()
    }

    /// Todas as reuniões do projeto, em ordem de número. Uma reunião aparece
    /// quando a pasta `reunioes/` da demanda tem o estado ou a ata dela.
    pub fn reunioes(&self) -> Resultado<Vec<ArquivosDaReuniao>> {
        let mut saida = Vec::new();
        for (demanda, nome) in self.pastas_de_demanda()? {
            saida.extend(self.reunioes_da_pasta(demanda, &nome)?);
        }
        saida.sort_by_key(|r| r.id);
        saida.dedup_by_key(|r| r.id);
        Ok(saida)
    }

    /// Reuniões de uma demanda, em ordem de número.
    pub fn reunioes_da_demanda(&self, demanda: IdDemanda) -> Resultado<Vec<ArquivosDaReuniao>> {
        let pasta = self.demanda(demanda)?.pasta;
        self.reunioes_da_pasta(demanda, pasta.nome())
    }

    /// Arquivos da reunião, em qualquer demanda do projeto.
    pub fn reuniao(&self, id: IdReuniao) -> Resultado<ArquivosDaReuniao> {
        self.reunioes()?
            .into_iter()
            .find(|r| r.id == id)
            .ok_or_else(|| Erro::ReuniaoInexistente {
                projeto: self.nome().to_owned(),
                id,
            })
    }

    /// Próximo identificador livre, sem reservar. Para criar, use
    /// [`Projeto::criar_reuniao`].
    pub fn proxima_reuniao(&self) -> Resultado<IdReuniao> {
        let maior = self.reunioes()?.last().map(|r| r.id.numero()).unwrap_or(0);
        Ok(IdReuniao::novo(maior + 1))
    }

    /// Cria uma reunião na demanda com o próximo número livre do projeto. Grava
    /// em `reunioes/R-NNNN.estado.json` o texto que `estado` devolve para o
    /// número reservado (a ata é montada depois, a partir dele). Veja a reserva
    /// no [módulo](self).
    pub fn criar_reuniao(
        &self,
        demanda: IdDemanda,
        mut estado: impl FnMut(IdReuniao) -> String,
    ) -> Resultado<ArquivosDaReuniao> {
        let pasta = self.demanda(demanda)?.pasta.juntar(layout::REUNIOES)?;
        let demandas = pasta_das_demandas();
        let primeiro = self.proxima_reuniao()?.numero();
        for numero in primeiro..primeiro.saturating_add(TENTATIVAS_DE_RESERVA) {
            let id = IdReuniao::novo(numero);
            let trava = demandas.juntar(&format!(".{id}{SUFIXO_DA_TRAVA}"))?;
            match fs::create_dir(self.caminho(&trava)) {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(erro) => return Err(self.erro_io(&trava, erro)),
            }
            let _trava = Trava(self.caminho(&trava));
            if self.reunioes()?.iter().any(|r| r.id == id) {
                continue;
            }
            let arquivos = ArquivosDaReuniao::novo(id, demanda, pasta.clone())?;
            match arquivo::criar(&self.caminho(&arquivos.estado), estado(id).as_bytes()) {
                Ok(()) => return Ok(arquivos),
                // Criado por fora da reserva (à mão, por exemplo).
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(erro) => return Err(self.erro_io(&arquivos.estado, erro)),
            }
        }
        Err(Erro::JaExiste(self.na_base(&pasta)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_pelo_nome_do_arquivo() {
        assert_eq!(id_do_arquivo("R-0003.md"), Some(IdReuniao::novo(3)));
        assert_eq!(
            id_do_arquivo("R-0003.estado.json"),
            Some(IdReuniao::novo(3))
        );
        for ruim in ["R-3.md", "R-0003.json", "R-0003", "D-0001.md", ".R-0003.md"] {
            assert_eq!(id_do_arquivo(ruim), None, "{ruim}");
        }
    }
}
