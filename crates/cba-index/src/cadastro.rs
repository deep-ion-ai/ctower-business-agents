//! Persistência do cadastro dos sistemas e do último inventário de cada um.
//!
//! Por ora em JSON, um diretório por sistema:
//!
//! ```text
//! <raiz>/<id>/sistema.json
//! <raiz>/<id>/inventario.json
//! <raiz>/<id>/mapa.json
//! ```
//!
//! O `mapa.json` guarda a extração de cada arquivo (símbolos e referências pelo
//! nome, com o hash do arquivo e o extrator usado), em JSON compacto. As ligações
//! entre referências e símbolos são refeitas ao carregar. Apagá-lo só faz o
//! próximo mapeamento ler todos os arquivos de novo.
//!
//! A raiz fica no diretório de dados do app (`cba_core::dados`), por exemplo
//! `<dados>/sistemas`. Quando o índice SQLite existir (#8), o cadastro pode migrar
//! para lá sem mudar a API de [`Sistema`] e [`Inventario`]. Toda gravação é atômica
//! (arquivo temporário e renomeação), para uma queda não corromper o cadastro.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::Erro;
use crate::extrator::Registro;
use crate::inventario::{Inventario, inventariar};
use crate::mapa::{Mapa, mapear};
use crate::sistema::Sistema;

const SISTEMA: &str = "sistema.json";
const INVENTARIO: &str = "inventario.json";
const MAPA: &str = "mapa.json";

/// Cadastro dos sistemas numa pasta.
#[derive(Debug, Clone)]
pub struct Cadastro {
    raiz: PathBuf,
}

impl Cadastro {
    /// Cadastro na pasta `raiz` (criada na primeira gravação).
    pub fn em(raiz: impl Into<PathBuf>) -> Self {
        Self { raiz: raiz.into() }
    }

    pub fn raiz(&self) -> &Path {
        &self.raiz
    }

    /// Sistemas cadastrados, ordenados pelo nome.
    pub fn listar(&self) -> Result<Vec<Sistema>, Erro> {
        let entradas = match fs::read_dir(&self.raiz) {
            Ok(e) => e,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(Erro::io(&self.raiz, e)),
        };
        let mut sistemas = Vec::new();
        for e in entradas {
            let e = e.map_err(|x| Erro::io(&self.raiz, x))?;
            let arq = e.path().join(SISTEMA);
            if arq.is_file() {
                sistemas.push(ler::<Sistema>(&arq)?);
            }
        }
        sistemas.sort_by_key(|a| a.nome.to_lowercase());
        Ok(sistemas)
    }

    pub fn obter(&self, id: &str) -> Result<Option<Sistema>, Erro> {
        let arq = self.dir(id)?.join(SISTEMA);
        if !arq.is_file() {
            return Ok(None);
        }
        ler(&arq).map(Some)
    }

    /// Cadastra um sistema novo. Falha se o identificador já existe.
    pub fn cadastrar(&self, sistema: &Sistema) -> Result<(), Erro> {
        if self.obter(&sistema.id)?.is_some() {
            return Err(Erro::JaCadastrado(sistema.id.clone()));
        }
        self.salvar(sistema)
    }

    /// Grava o sistema (novo ou alterado), depois de validar.
    pub fn salvar(&self, sistema: &Sistema) -> Result<(), Erro> {
        sistema.validar()?;
        gravar(&self.dir(&sistema.id)?.join(SISTEMA), sistema)
    }

    /// Remove o sistema, o inventário e o mapa dele. Devolve se existia.
    ///
    /// Não toca nas pastas do sistema nem na base de conhecimento.
    pub fn remover(&self, id: &str) -> Result<bool, Erro> {
        let dir = self.dir(id)?;
        match fs::remove_dir_all(&dir) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(Erro::io(dir, e)),
        }
    }

    /// Último inventário gravado do sistema.
    pub fn inventario(&self, id: &str) -> Result<Option<Inventario>, Erro> {
        let arq = self.dir(id)?.join(INVENTARIO);
        if !arq.is_file() {
            return Ok(None);
        }
        ler(&arq).map(Some)
    }

    /// Inventaria o sistema aproveitando o inventário anterior e grava o novo.
    pub fn inventariar(&self, id: &str) -> Result<Inventario, Erro> {
        let sistema = self
            .obter(id)?
            .ok_or_else(|| Erro::NaoCadastrado(id.to_string()))?;
        // Inventário anterior ilegível (formato antigo, arquivo corrompido) só faz
        // perder o cache.
        let anterior = self.inventario(id).ok().flatten();
        let novo = inventariar(&sistema, anterior.as_ref())?;
        gravar(&self.dir(id)?.join(INVENTARIO), &novo)?;
        Ok(novo)
    }

    /// Último mapa gravado do sistema.
    pub fn mapa(&self, id: &str) -> Result<Option<Mapa>, Erro> {
        let arq = self.dir(id)?.join(MAPA);
        if !arq.is_file() {
            return Ok(None);
        }
        ler(&arq).map(Some)
    }

    /// Inventaria e mapeia o sistema com todos os extratores, aproveitando o
    /// inventário e o mapa anteriores, e grava os dois.
    pub fn mapear(&self, id: &str) -> Result<Mapa, Erro> {
        self.mapear_com(id, &Registro::padrao())
    }

    /// Como [`Cadastro::mapear`], com outro conjunto de extratores.
    pub fn mapear_com(&self, id: &str, registro: &Registro) -> Result<Mapa, Erro> {
        let inventario = self.inventariar(id)?;
        let sistema = self
            .obter(id)?
            .ok_or_else(|| Erro::NaoCadastrado(id.to_string()))?;
        // Mapa anterior ilegível só faz perder o cache.
        let anterior = self.mapa(id).ok().flatten();
        let mapa = mapear(&inventario, &sistema.escopo()?, anterior.as_ref(), registro);
        gravar_json(&self.dir(id)?.join(MAPA), &mapa, false)?;
        Ok(mapa)
    }

    fn dir(&self, id: &str) -> Result<PathBuf, Erro> {
        // O id vira nome de pasta: só o formato de slug, sem `..` nem separadores.
        let valido = !id.is_empty()
            && id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        if !valido {
            return Err(Erro::IdInvalido(id.to_string()));
        }
        Ok(self.raiz.join(id))
    }
}

fn ler<T: DeserializeOwned>(arq: &Path) -> Result<T, Erro> {
    let bytes = fs::read(arq).map_err(|e| Erro::io(arq, e))?;
    serde_json::from_slice(&bytes).map_err(|fonte| Erro::Json {
        caminho: arq.to_path_buf(),
        fonte,
    })
}

fn gravar<T: Serialize>(arq: &Path, valor: &T) -> Result<(), Erro> {
    gravar_json(arq, valor, true)
}

fn gravar_json<T: Serialize>(arq: &Path, valor: &T, legivel: bool) -> Result<(), Erro> {
    let dir = arq.parent().expect("arquivo do cadastro tem pasta");
    fs::create_dir_all(dir).map_err(|e| Erro::io(dir, e))?;
    let json = if legivel {
        serde_json::to_vec_pretty(valor)
    } else {
        serde_json::to_vec(valor)
    }
    .map_err(|fonte| Erro::Json {
        caminho: arq.to_path_buf(),
        fonte,
    })?;
    let tmp = arq.with_extension("json.tmp");
    let resultado = (|| {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(&json)?;
        f.sync_all()?;
        drop(f);
        fs::rename(&tmp, arq)
    })();
    resultado.map_err(|e| {
        let _ = fs::remove_file(&tmp);
        Erro::io(arq, e)
    })
}
