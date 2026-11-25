//! Retirada dos hooks que preparos anteriores à #304 gravaram no `settings.json` do
//! usuário (#305). A orquestração (preparar as pastas antes, a proteção, o aviso) é
//! de [`crate::migracao`]; aqui fica só a edição do `settings.json`.
//!
//! Sai exatamente o que esta instalação gravou:
//!
//! - **Com registro** ([`super::registro`]) e o evento ainda igual ao que ela gravou:
//!   o evento volta ao valor de antes (que pode ter os hooks de outra instalação do
//!   app, que os tira na migração dela).
//! - **Sem registro** (preparos da 0.1.0 a 0.4.0) ou com o evento mudado depois do
//!   preparo: saem só os grupos idênticos ao que o app grava
//!   (`{"hooks":[{"type":"command","command":"\"<exe>\" hook <evento>","timeout":10}]}`)
//!   com o executável desta instalação. Os grupos do analista ficam no lugar, mesmo
//!   no mesmo evento.
//! - Um handler desta instalação que o analista mudou (outro `timeout`, junto com
//!   outros handlers no mesmo grupo) fica, e o evento é dito em
//!   [`RetiradaGlobal::ficaram`].
//!
//! Hooks de outras instalações do app não são tocados.

use std::path::PathBuf;

use serde::Serialize;

use super::json::{Json, Objeto};
use super::registro::{Registro, TrocaHook};
use super::{
    Acao, Alteracao, EVENTOS, Erro, Item, Modo, Preparador, executaveis_dos_hooks, formato,
    grupo_hook, mesmo,
};

/// Resultado de [`Preparador::retirar_hooks_globais`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RetiradaGlobal {
    /// O que mudou no `settings.json` (só os eventos com mudança).
    pub alteracoes: Vec<Alteracao>,
    /// Eventos em que ficou um hook desta instalação que o analista mudou.
    pub ficaram: Vec<String>,
    pub backups: Vec<PathBuf>,
}

impl RetiradaGlobal {
    pub fn houve_mudanca(&self) -> bool {
        self.alteracoes.iter().any(|a| a.acao != Acao::SemMudanca)
    }
}

impl Preparador {
    /// Hooks desta instalação no `settings.json` do usuário (preparo anterior à #304),
    /// por evento. Não grava nada.
    pub fn hooks_globais(&self) -> Result<Vec<String>, Erro> {
        let lido = super::arquivo::ler(&self.config.settings)?;
        let meu = self.executavel_no_hook();
        let mut eventos = Vec::new();
        let Some(Json::Objeto(hooks)) = lido.raiz.get("hooks") else {
            return Ok(eventos);
        };
        for evento in EVENTOS {
            if let Some(Json::Lista(grupos)) = hooks.get(evento)
                && executaveis_dos_hooks(grupos).contains(&meu.as_str())
            {
                eventos.push(evento.to_string());
            }
        }
        Ok(eventos)
    }

    /// Tira do `settings.json` do usuário os hooks que esta instalação gravou lá
    /// (veja o módulo). Idempotente: sem nada do app, não grava.
    pub fn retirar_hooks_globais(&self) -> Result<RetiradaGlobal, Erro> {
        let arq = self.config.settings.clone();
        let exe = self.executavel_texto();
        let mut registro = Registro::ler(&self.config.registro());
        let mut acoes: Vec<(String, Acao, Option<String>)> = Vec::new();
        let mut ficaram = Vec::new();
        let backup = self.editar_registrando(
            &arq,
            Modo::Gravar,
            &mut registro,
            None,
            false,
            |raiz, caminho, reg| {
                acoes.clear();
                ficaram.clear();
                let (existia, trocas) = reg.tirar_hooks(&exe).unwrap_or((None, Default::default()));
                let Some(hooks) = raiz.get_mut("hooks") else {
                    return Ok(());
                };
                let Json::Objeto(hooks) = hooks else {
                    return Err(formato(caminho, "hooks"));
                };
                for evento in EVENTOS {
                    let (acao, outra, ficou) =
                        self.retirar_evento(hooks, evento, trocas.get(*evento), caminho)?;
                    if ficou {
                        ficaram.push(evento.to_string());
                    }
                    acoes.push((evento.to_string(), acao, outra));
                }
                let houve = acoes.iter().any(|(_, a, _)| *a != Acao::SemMudanca);
                if hooks.is_empty() && houve && existia != Some(true) {
                    raiz.remover("hooks");
                }
                Ok(())
            },
        )?;
        let alteracoes = acoes
            .into_iter()
            .filter(|(_, acao, _)| *acao != Acao::SemMudanca)
            .map(|(evento, acao, outra)| Alteracao {
                arquivo: arq.clone(),
                item: Item::Hook { evento },
                acao,
                outra_instalacao: outra,
            })
            .collect::<Vec<_>>();
        tracing::info!(
            eventos = alteracoes.len(),
            ficaram = ficaram.len(),
            "hooks globais do app retirados"
        );
        Ok(RetiradaGlobal {
            alteracoes,
            ficaram,
            backups: backup.into_iter().collect(),
        })
    }

    /// Retira os hooks desta instalação de `hooks.<evento>`. Devolve a ação, a outra
    /// instalação cujos hooks voltaram (se for o caso) e se ficou um hook desta
    /// instalação que o analista mudou.
    fn retirar_evento(
        &self,
        hooks: &mut Objeto,
        evento: &str,
        troca: Option<&TrocaHook>,
        caminho: &std::path::Path,
    ) -> Result<(Acao, Option<String>, bool), Erro> {
        let meu = self.executavel_no_hook();
        let Some(atual) = hooks.get(evento) else {
            return Ok((Acao::SemMudanca, None, false));
        };
        if atual.como_lista().is_none() {
            return Err(formato(caminho, &format!("hooks.{evento}")));
        }
        if let Some(troca) = troca {
            if mesmo(atual, &troca.depois) {
                return Ok(match &troca.antes {
                    None => {
                        hooks.remover(evento);
                        (Acao::Removido, None, false)
                    }
                    Some(antes) => {
                        hooks.inserir(evento, antes.clone());
                        let outra = antes
                            .como_lista()
                            .and_then(|g| executaveis_dos_hooks(g).into_iter().next())
                            .map(str::to_string);
                        let acao = if outra.is_some() {
                            Acao::Restaurado
                        } else {
                            Acao::Removido
                        };
                        (acao, outra, false)
                    }
                });
            }
            if troca.antes.as_ref().is_some_and(|a| mesmo(a, atual)) {
                return Ok((Acao::SemMudanca, None, false));
            }
        }
        // Sem registro, ou mudou depois do preparo: só o que o app grava, igual.
        let Some(Json::Lista(grupos)) = hooks.get_mut(evento) else {
            return Err(formato(caminho, &format!("hooks.{evento}")));
        };
        let nosso = grupo_hook(evento, &self.comando_hook(evento));
        let antes = grupos.len();
        grupos.retain(|g| !mesmo(g, &nosso));
        let tirados = antes - grupos.len();
        let ficou = executaveis_dos_hooks(grupos).contains(&meu.as_str());
        if tirados == 0 {
            return Ok((Acao::SemMudanca, None, ficou));
        }
        if grupos.is_empty() {
            hooks.remover(evento);
        }
        Ok((Acao::Removido, None, ficou))
    }
}
