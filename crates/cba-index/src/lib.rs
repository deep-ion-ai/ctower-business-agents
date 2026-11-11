//! Mapeamento do sistema analisado.
//!
//! Cadastro do sistema, inventário dos arquivos, exclusões, extratores por
//! linguagem ([`extrator`]) e mapa estrutural do sistema ([`Mapa`]).
//!
//! Papel na arquitetura: seção 5 de `docs/specs/2026-11-11-mvp-standalone.md`. O
//! cadastro e o inventário seguem a seção 4.1 (passo 4) e as exclusões padrão, a
//! seção 9.
//!
//! Uso típico:
//!
//! ```no_run
//! use cba_index::{Cadastro, Sistema};
//!
//! # fn main() -> Result<(), cba_index::Erro> {
//! let cadastro = Cadastro::em("/dados/sistemas");
//! let sistema = Sistema::novo("Faturamento Exemplo", ["/codigo/faturamento"])?;
//! cadastro.cadastrar(&sistema)?;
//! let inventario = cadastro.inventariar(&sistema.id)?;
//! println!("{:?}", inventario.resumo());
//!
//! // Mapa estrutural: símbolos, referências e módulos (incremental, pelo hash).
//! let mapa = cadastro.mapear(&sistema.id)?;
//! for multa in mapa.buscar("PKG_COBRANCA.FN_CALCULAR_MULTA") {
//!     for uso in mapa.referencias_a(multa.id) {
//!         println!("{}:{}", uso.arquivo, uso.referencia.linha);
//!     }
//! }
//!
//! // Ferramentas (validação de evidências, leitura) resolvem caminhos pelo escopo,
//! // que recusa caminhos fora das pastas do sistema e arquivos excluídos.
//! let escopo = sistema.escopo()?;
//! let arquivo = escopo.resolver("db/03_pkg_cobranca.sql");
//! # Ok(())
//! # }
//! ```
//!
//! ## Caminhos relativos
//!
//! Todo caminho que sai desta crate (inventário, exclusões, evidências) é relativo à
//! raiz do sistema, com `/` como separador em qualquer sistema operacional. Um
//! sistema com mais de uma pasta tem no máximo uma pasta sem prefixo (a raiz); as
//! demais aparecem sob o seu prefixo (`prefixo/caminho/no/arquivo`). Veja [`Pasta`].

mod cadastro;
mod erro;
mod exclusoes;
pub mod extrator;
mod git;
mod inventario;
mod linguagem;
mod mapa;
mod sistema;

pub use cadastro::Cadastro;
pub use erro::Erro;
pub use exclusoes::{Exclusao, Exclusoes, Motivo, REGRAS_PADRAO};
pub use extrator::{
    Extracao, Extrator, Fonte, Linhas, Referencia, Registro, Simbolo, TipoReferencia, TipoSimbolo,
};
pub use git::{RepositorioGit, detectar_git};
pub use inventario::{
    ArquivoInventariado, Estatisticas, Ignorado, Inventario, PastaInventariada, Resumo, inventariar,
};
pub use linguagem::Linguagem;
pub use mapa::{
    Achado, ArquivoMapeado, Dependencia, EstatisticasMapa, FORMATO_MAPA, LIMITE_EXTRACAO, Mapa,
    Modulo, SimboloId, Uso, mapear,
};
pub use sistema::{Escopo, Pasta, Recusa, Sistema, slug};

/// Nome da crate, para diagnóstico.
pub const NOME: &str = env!("CARGO_PKG_NAME");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nome_da_crate() {
        assert_eq!(NOME, "cba-index");
    }
}
