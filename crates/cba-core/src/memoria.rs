//! Memória de uma análise (#143): o contexto de trabalho do agente, para uma
//! conversa nova continuar de onde a anterior parou sem depender da conversa
//! gravada pelo Claude Code (que expira e pode nem existir).
//!
//! Fica em `analises/A-NNNN.memoria.md`, ao lado do documento e do estado. É
//! Markdown com seções fixas ([`SecaoMemoria`]), que o agente atualiza por
//! seção: acrescentando um item ou substituindo a seção inteira (o caminho para
//! resumir quando ela cresce). O arquivo pode ser lido e editado à mão: o texto
//! fora das seções conhecidas é preservado, e cabeçalhos `##` desconhecidos ficam
//! dentro da seção anterior.
//!
//! A memória não é documento da base: fica fora do índice e da busca
//! ([`cba_kb::layout::SUFIXO_MEMORIA_DA_ANALISE`]) e não vai para as regras nem
//! para o glossário na aprovação.

use std::fmt::Write as _;

use serde::{Deserialize, Serialize};

/// Tamanho máximo da memória inteira, em caracteres. Cabe com folga no contexto
/// da retomada; passar disso pede que o agente resuma uma seção.
pub const LIMITE_DA_MEMORIA: usize = 24_000;

/// A partir de que fração do limite a resposta avisa para resumir.
pub const AVISO_DA_MEMORIA: f64 = 0.8;

/// Seção da memória, na ordem em que aparece no arquivo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecaoMemoria {
    /// Onde a conversa parou e qual é o próximo passo.
    OndeParou,
    /// O que se entendeu do pedido e do sistema até aqui.
    Contexto,
    /// O que já foi investigado (e o que se concluiu).
    Investigado,
    /// Caminhos descartados e por quê.
    Descartado,
    /// Hipóteses ainda em aberto.
    Hipoteses,
    /// Onde está o código relevante, inclusive repositórios fora do projeto.
    Codigo,
    /// O que foi combinado com o analista na conversa.
    Combinados,
    /// O que ficou pendente e por quê.
    Pendencias,
}

impl SecaoMemoria {
    pub const TODAS: [SecaoMemoria; 8] = [
        Self::OndeParou,
        Self::Contexto,
        Self::Investigado,
        Self::Descartado,
        Self::Hipoteses,
        Self::Codigo,
        Self::Combinados,
        Self::Pendencias,
    ];

    /// Nome no argumento da ferramenta (`onde_parou`).
    pub fn como_str(self) -> &'static str {
        match self {
            Self::OndeParou => "onde_parou",
            Self::Contexto => "contexto",
            Self::Investigado => "investigado",
            Self::Descartado => "descartado",
            Self::Hipoteses => "hipoteses",
            Self::Codigo => "codigo",
            Self::Combinados => "combinados",
            Self::Pendencias => "pendencias",
        }
    }

    /// Título no arquivo (`## Onde parou`).
    pub fn titulo(self) -> &'static str {
        match self {
            Self::OndeParou => "Onde parou",
            Self::Contexto => "Contexto",
            Self::Investigado => "Investigado",
            Self::Descartado => "Descartado",
            Self::Hipoteses => "Hipóteses em aberto",
            Self::Codigo => "Onde está o código",
            Self::Combinados => "Combinados com o analista",
            Self::Pendencias => "Pendências",
        }
    }

    /// O que vai na seção, para o schema da ferramenta e o roteiro.
    pub fn descricao(self) -> &'static str {
        match self {
            Self::OndeParou => "onde a conversa parou e qual é o próximo passo",
            Self::Contexto => "o que se entendeu do pedido e do sistema até aqui",
            Self::Investigado => "o que já foi investigado e o que se concluiu",
            Self::Descartado => "caminhos descartados e por quê",
            Self::Hipoteses => "hipóteses ainda em aberto",
            Self::Codigo => "onde está o código relevante, inclusive repositórios fora do projeto",
            Self::Combinados => "o que foi combinado com o analista na conversa",
            Self::Pendencias => "o que ficou pendente e por quê",
        }
    }

    pub fn de_str(texto: &str) -> Option<Self> {
        Self::TODAS.into_iter().find(|s| s.como_str() == texto)
    }

    fn do_titulo(titulo: &str) -> Option<Self> {
        let titulo = titulo.trim();
        Self::TODAS
            .into_iter()
            .find(|s| s.titulo().eq_ignore_ascii_case(titulo))
    }
}

/// Como gravar numa seção.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModoMemoria {
    /// Acrescenta um item (`- ...`) no fim da seção.
    #[default]
    Acrescentar,
    /// Troca a seção inteira pelo texto (vazio limpa a seção).
    Substituir,
}

/// Erro ao alterar a memória.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ErroMemoria {
    #[error("o texto para acrescentar à memória não pode ficar vazio")]
    TextoVazio,
    #[error(
        "a memória passaria de {limite} caracteres (ficaria com {tamanho}); resuma \
         antes: chame `memorizar` com `modo: \"substituir\"` na seção mais longa \
         ({maior}), com um resumo do que ainda importa, e grave de novo"
    )]
    Cheia {
        tamanho: usize,
        limite: usize,
        maior: &'static str,
    },
}

/// Memória de uma análise.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Memoria {
    /// Texto antes da primeira seção conhecida, sem o título (preservado).
    preambulo: String,
    /// Texto de cada seção, na ordem de [`SecaoMemoria::TODAS`].
    secoes: [String; 8],
}

fn indice(secao: SecaoMemoria) -> usize {
    SecaoMemoria::TODAS
        .iter()
        .position(|s| *s == secao)
        .expect("toda seção está em TODAS")
}

impl Memoria {
    /// Lê a memória do arquivo. A primeira linha `# ` (o título gerado) é
    /// descartada; cada `## <título conhecido>` abre a sua seção.
    pub fn analisar(texto: &str) -> Self {
        let mut memoria = Self::default();
        let mut atual: Option<usize> = None;
        let mut titulo_visto = false;
        for linha in texto.lines() {
            if !titulo_visto && atual.is_none() && linha.starts_with("# ") {
                titulo_visto = true;
                continue;
            }
            if let Some(titulo) = linha.strip_prefix("## ")
                && let Some(secao) = SecaoMemoria::do_titulo(titulo)
            {
                atual = Some(indice(secao));
                continue;
            }
            let destino = match atual {
                Some(i) => &mut memoria.secoes[i],
                None => &mut memoria.preambulo,
            };
            destino.push_str(linha);
            destino.push('\n');
        }
        memoria.preambulo = memoria.preambulo.trim().to_owned();
        for s in &mut memoria.secoes {
            *s = s.trim().to_owned();
        }
        memoria
    }

    /// Texto de uma seção (vazio se não houver nada).
    pub fn secao(&self, secao: SecaoMemoria) -> &str {
        &self.secoes[indice(secao)]
    }

    /// Se não há nada registrado.
    pub fn vazia(&self) -> bool {
        self.preambulo.is_empty() && self.secoes.iter().all(String::is_empty)
    }

    /// Grava numa seção, recusando se a memória passar de `limite` caracteres.
    /// Em caso de recusa, a memória não muda.
    pub fn gravar(
        &mut self,
        secao: SecaoMemoria,
        modo: ModoMemoria,
        texto: &str,
        limite: usize,
    ) -> Result<(), ErroMemoria> {
        let texto = texto.trim();
        let novo = match modo {
            ModoMemoria::Acrescentar => {
                if texto.is_empty() {
                    return Err(ErroMemoria::TextoVazio);
                }
                let item = como_item(texto);
                let atual = self.secao(secao);
                if atual.is_empty() {
                    item
                } else {
                    format!("{atual}\n{item}")
                }
            }
            ModoMemoria::Substituir => texto.to_owned(),
        };
        let mut candidata = self.clone();
        candidata.secoes[indice(secao)] = novo;
        let tamanho = candidata.tamanho();
        // Substituir por algo menor sempre passa: é assim que se resume.
        let encolheu = candidata.secao(secao).chars().count() <= self.secao(secao).chars().count();
        if tamanho > limite && !encolheu {
            return Err(ErroMemoria::Cheia {
                tamanho,
                limite,
                maior: self.maior_secao().como_str(),
            });
        }
        *self = candidata;
        Ok(())
    }

    /// Tamanho do conteúdo, em caracteres (sem o título nem os cabeçalhos).
    pub fn tamanho(&self) -> usize {
        self.preambulo.chars().count()
            + self.secoes.iter().map(|s| s.chars().count()).sum::<usize>()
    }

    fn maior_secao(&self) -> SecaoMemoria {
        SecaoMemoria::TODAS
            .into_iter()
            .max_by_key(|s| self.secao(*s).chars().count())
            .expect("há seções")
    }

    /// O arquivo: título, preâmbulo (se houver) e as seções com conteúdo.
    pub fn markdown(&self, titulo: &str) -> String {
        let mut t = format!("# {titulo}\n");
        if !self.preambulo.is_empty() {
            let _ = write!(t, "\n{}\n", self.preambulo);
        }
        for secao in SecaoMemoria::TODAS {
            let texto = self.secao(secao);
            if !texto.is_empty() {
                let _ = write!(t, "\n## {}\n\n{texto}\n", secao.titulo());
            }
        }
        t
    }

    /// As seções com conteúdo, na ordem, para a retomada.
    pub fn secoes(&self) -> impl Iterator<Item = (SecaoMemoria, &str)> {
        SecaoMemoria::TODAS
            .into_iter()
            .map(|s| (s, self.secao(s)))
            .filter(|(_, t)| !t.is_empty())
    }

    /// Texto livre fora das seções (por exemplo, escrito à mão no arquivo).
    pub fn preambulo(&self) -> &str {
        &self.preambulo
    }
}

/// Um item de lista: o texto já começa com marcador ou vira `- texto`, com as
/// linhas seguintes recuadas para continuar o item.
fn como_item(texto: &str) -> String {
    if texto.starts_with("- ") || texto.starts_with("* ") {
        return texto.to_owned();
    }
    let mut linhas = texto.lines();
    let mut item = format!("- {}", linhas.next().unwrap_or_default());
    for linha in linhas {
        item.push('\n');
        if !linha.trim().is_empty() {
            item.push_str("  ");
            item.push_str(linha);
        }
    }
    item
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nomes_ida_e_volta() {
        for s in SecaoMemoria::TODAS {
            assert_eq!(SecaoMemoria::de_str(s.como_str()), Some(s));
            assert_eq!(SecaoMemoria::do_titulo(s.titulo()), Some(s));
            let json = serde_json::to_value(s).unwrap();
            assert_eq!(json, s.como_str());
        }
    }

    #[test]
    fn acrescenta_itens_e_substitui_secao() {
        let mut m = Memoria::default();
        assert!(m.vazia());
        m.gravar(
            SecaoMemoria::Investigado,
            ModoMemoria::Acrescentar,
            "Tela de parcelamento",
            1000,
        )
        .unwrap();
        m.gravar(
            SecaoMemoria::Investigado,
            ModoMemoria::Acrescentar,
            "Serviço de multa\ncalcula 2%",
            1000,
        )
        .unwrap();
        assert_eq!(
            m.secao(SecaoMemoria::Investigado),
            "- Tela de parcelamento\n- Serviço de multa\n  calcula 2%"
        );
        m.gravar(
            SecaoMemoria::OndeParou,
            ModoMemoria::Substituir,
            "Falta o lote noturno.",
            1000,
        )
        .unwrap();
        m.gravar(
            SecaoMemoria::OndeParou,
            ModoMemoria::Substituir,
            "Falta concluir.",
            1000,
        )
        .unwrap();
        assert_eq!(m.secao(SecaoMemoria::OndeParou), "Falta concluir.");
        assert_eq!(
            m.gravar(SecaoMemoria::Contexto, ModoMemoria::Acrescentar, "  ", 1000),
            Err(ErroMemoria::TextoVazio)
        );
        // Substituir por vazio limpa a seção.
        m.gravar(SecaoMemoria::OndeParou, ModoMemoria::Substituir, "", 1000)
            .unwrap();
        assert_eq!(m.secao(SecaoMemoria::OndeParou), "");
    }

    #[test]
    fn markdown_ida_e_volta_com_texto_a_mao() {
        let mut m = Memoria::default();
        m.gravar(
            SecaoMemoria::Codigo,
            ModoMemoria::Acrescentar,
            "bnmp-frontend em ../bnmp-frontend",
            1000,
        )
        .unwrap();
        m.gravar(
            SecaoMemoria::OndeParou,
            ModoMemoria::Substituir,
            "Revisar a seção de exceções.",
            1000,
        )
        .unwrap();
        let md = m.markdown("Memória da análise A-0001: Multa");
        assert_eq!(
            md,
            "# Memória da análise A-0001: Multa\n\n## Onde parou\n\nRevisar a seção de \
             exceções.\n\n## Onde está o código\n\n- bnmp-frontend em ../bnmp-frontend\n"
        );
        assert_eq!(Memoria::analisar(&md), m);

        // Editada à mão: nota solta, cabeçalho desconhecido e caixa diferente.
        let editada = "# Memória\n\nNota do analista.\n\n## onde parou\n\nX.\n\n### Detalhe\n\nY.\n\n## Outra coisa\n\nZ.\n";
        let m = Memoria::analisar(editada);
        assert_eq!(m.preambulo(), "Nota do analista.");
        assert_eq!(
            m.secao(SecaoMemoria::OndeParou),
            "X.\n\n### Detalhe\n\nY.\n\n## Outra coisa\n\nZ."
        );
        assert!(m.markdown("T").contains("Nota do analista."));
    }

    #[test]
    fn limite_recusa_crescer_mas_aceita_resumir() {
        let mut m = Memoria::default();
        m.gravar(
            SecaoMemoria::Investigado,
            ModoMemoria::Acrescentar,
            &"a".repeat(80),
            100,
        )
        .unwrap();
        let antes = m.clone();
        let e = m
            .gravar(
                SecaoMemoria::Contexto,
                ModoMemoria::Acrescentar,
                &"b".repeat(30),
                100,
            )
            .unwrap_err();
        assert!(
            matches!(
                e,
                ErroMemoria::Cheia {
                    maior: "investigado",
                    limite: 100,
                    ..
                }
            ),
            "{e}"
        );
        assert!(e.to_string().contains("substituir"), "{e}");
        assert_eq!(m, antes);
        // Resumir a seção maior passa e libera espaço.
        m.gravar(
            SecaoMemoria::Investigado,
            ModoMemoria::Substituir,
            "resumo",
            100,
        )
        .unwrap();
        m.gravar(
            SecaoMemoria::Contexto,
            ModoMemoria::Acrescentar,
            &"b".repeat(30),
            100,
        )
        .unwrap();
        assert!(m.tamanho() <= 100);
    }
}
