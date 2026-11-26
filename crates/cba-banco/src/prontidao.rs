//! Prontidão de uma conexão de banco (#440): os passos que o descritor declara,
//! executados de verdade por [`crate::testar`].
//!
//! `cba-repos` não pode depender desta crate, então o "Testar" da conexão (no
//! app) chama [`prontidao`] para os tipos com a capacidade `Banco`, no lugar dos
//! passos "ainda não verificados pelo app" de
//! [`cba_repos::providers::prontidao::testar`]. O resultado tem a mesma forma
//! ([`Integracao`]): o passo `cadastro`, o `credencial` e os do descritor, com a
//! orientação de quem falhou e os seguintes pulados.

use cba_repos::chaveiro::Chaveiro;
use cba_repos::prontidao::{Integracao, Passo, Situacao};
use cba_repos::providers::{Capacidade, Conexao, FonteCredencial, mascara, todos};

use crate::{ConfigBanco, ErroBanco, testar};

/// Se a conexão é de um tipo de banco (o "Testar" passa por [`prontidao`]).
pub fn e_de_banco(conexao: &Conexao) -> bool {
    todos()
        .get(&conexao.tipo)
        .is_some_and(|d| d.tem(Capacidade::Banco))
}

fn ok(id: &str, rotulo: &str, detalhe: String) -> Passo {
    Passo {
        id: id.into(),
        rotulo: rotulo.into(),
        situacao: Situacao::Ok,
        detalhe: Some(detalhe),
        orientacao: None,
    }
}

fn falhou(id: &str, rotulo: &str, detalhe: String, orientacao: &str) -> Passo {
    Passo {
        id: id.into(),
        rotulo: rotulo.into(),
        situacao: Situacao::Falhou,
        detalhe: Some(mascara::mascarar(&detalhe)),
        orientacao: Some(orientacao.into()),
    }
}

fn pulado(id: &str, rotulo: &str) -> Passo {
    Passo {
        id: id.into(),
        rotulo: rotulo.into(),
        situacao: Situacao::Pulado,
        detalhe: None,
        orientacao: None,
    }
}

/// O "Testar" de uma conexão de banco: cadastro, senha e os passos do
/// descritor (conectar, versão, só leitura, esquemas), executados no servidor.
pub fn prontidao(conexao: &Conexao, chaveiro: &dyn Chaveiro) -> Integracao {
    let registro = todos();
    let rotulo_tipo = registro
        .get(&conexao.tipo)
        .map_or(conexao.tipo.as_str(), |d| d.rotulo());
    let titulo = format!("{} ({rotulo_tipo})", conexao.nome.trim());
    let declarados = registro
        .get(&conexao.tipo)
        .map(|d| d.passos_prontidao())
        .unwrap_or_default();
    let mut passos = Vec::new();
    let config = match ConfigBanco::resolver(conexao, chaveiro) {
        Ok(c) => {
            passos.push(ok("cadastro", "Cadastro da conexão", "válido".into()));
            let fonte = match &conexao.credencial {
                FonteCredencial::Nenhuma => "sem senha".to_owned(),
                f => cba_repos::providers::prontidao::descrever_fonte(f),
            };
            passos.push(ok("credencial", "Senha do banco", fonte));
            Some(c)
        }
        Err(ErroBanco::Credencial(m)) => {
            passos.push(ok("cadastro", "Cadastro da conexão", "válido".into()));
            passos.push(falhou(
                "credencial",
                "Senha do banco",
                m,
                "Guarde a senha da conexão ou confira o comando ou o arquivo da credencial.",
            ));
            None
        }
        Err(e) => {
            passos.push(falhou(
                "cadastro",
                "Cadastro da conexão",
                e.to_string(),
                "Corrija a conexão em Configurações › Conexões.",
            ));
            None
        }
    };
    let resultados = config.as_ref().map(testar).unwrap_or_default();
    let mut parou = config.is_none();
    for p in declarados {
        if parou {
            passos.push(pulado(p.id, p.rotulo));
            continue;
        }
        match resultados.iter().find(|r| r.id == p.id) {
            Some(r) if r.ok => passos.push(ok(p.id, p.rotulo, r.detalhe.clone())),
            Some(r) => {
                passos.push(falhou(p.id, p.rotulo, r.detalhe.clone(), p.orientacao));
                parou = true;
            }
            None => {
                passos.push(pulado(p.id, p.rotulo));
                parou = true;
            }
        }
    }
    Integracao { titulo, passos }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cba_repos::chaveiro::ChaveiroMemoria;
    use std::collections::BTreeMap;

    fn conexao(endereco: &str, opcoes: &[(&str, &str)]) -> Conexao {
        Conexao {
            id: "banco-hml".into(),
            nome: "Banco de homologação".into(),
            tipo: "postgres".into(),
            endereco: endereco.into(),
            credencial: FonteCredencial::Nenhuma,
            opcoes: opcoes
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect::<BTreeMap<_, _>>(),
        }
    }

    #[test]
    fn cadastro_invalido_pula_os_passos_do_descritor() {
        let i = prontidao(
            &conexao("db.example.com/fat", &[]),
            &ChaveiroMemoria::default(),
        );
        assert_eq!(i.passos[0].id, "cadastro");
        assert_eq!(i.passos[0].situacao, Situacao::Falhou);
        assert!(i.passos[1..].iter().all(|p| p.situacao == Situacao::Pulado));
        assert_eq!(
            i.passos.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(),
            ["cadastro", "conectar", "versao", "so_leitura", "esquemas"]
        );
        assert!(!i.pronta());
    }

    #[test]
    fn servidor_inacessivel_falha_no_conectar_com_a_orientacao() {
        // Porta 1 do localhost: recusa a conexão na hora.
        let c = conexao(
            "127.0.0.1:1/fat",
            &[("usuario", "leitura"), ("tls", "desligar")],
        );
        let i = prontidao(&c, &ChaveiroMemoria::default());
        let ids: Vec<_> = i
            .passos
            .iter()
            .map(|p| (p.id.as_str(), p.situacao))
            .collect();
        assert_eq!(
            ids,
            [
                ("cadastro", Situacao::Ok),
                ("credencial", Situacao::Ok),
                ("conectar", Situacao::Falhou),
                ("versao", Situacao::Pulado),
                ("so_leitura", Situacao::Pulado),
                ("esquemas", Situacao::Pulado),
            ]
        );
        assert!(i.passos[2].orientacao.is_some());
        assert!(i.titulo.contains("PostgreSQL"), "{}", i.titulo);
    }
}
