//! `tokens_css()`: os tokens CSS do projeto analisado (#384), para o agente
//! usar nas telas do protótipo (#356).
//!
//! Só leitura. Devolve o arquivo de variáveis CSS (`:root`) e o estado da
//! revisão ([`cba_core::tokens`]). Só o CSS aprovado pelo analista vai para as
//! telas: em rascunho, a resposta traz o CSS para conferência e diz para pedir
//! a revisão ao analista. Antes da primeira extração, mostra uma prévia
//! extraída na hora, sem gravar: a extração e a revisão são do analista, no app.

use cba_core::tokens::{EstadoTokens, TokensProjeto};
use serde_json::json;

use crate::registro::{ErroFerramenta, Ferramenta, Saida};

/// `tokens_css()`: o CSS dos tokens do projeto e o estado da revisão.
pub fn tokens_css() -> Ferramenta {
    Ferramenta::nova(
        "tokens_css",
        "Tokens CSS do projeto",
        "Devolve os tokens CSS (cores, tipografia, espaçamento e raio) extraídos do código \
         da interface do projeto, num arquivo de variáveis em `:root`, e o estado da revisão \
         do analista. As telas do protótipo usam os papéis `--prototipo-*` (por exemplo \
         `var(--prototipo-cor-primaria)`), e só quando os tokens estão aprovados. O que não \
         foi encontrado no código vem como \"a confirmar\", comentado e sem valor: não \
         invente o valor.",
        json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        }),
        |ambiente, _args| {
            let estado = TokensProjeto::carregar(&ambiente.kb)
                .map_err(|e| ErroFerramenta::Interno(e.to_string()))?;
            let (tokens, gravado) = match estado {
                Some(t) => (t, true),
                None => (
                    TokensProjeto::novos(
                        cba_index::extrair_estilo(&ambiente.escopo),
                        cba_core::tempo::agora_ms(),
                    ),
                    false,
                ),
            };
            let aprovado = gravado && tokens.estado == EstadoTokens::Aprovado;
            let a_confirmar: Vec<String> = tokens
                .papeis_a_confirmar()
                .map(|p| p.propriedade())
                .collect();
            let orientacao = if aprovado {
                "Tokens aprovados pelo analista: use este CSS nas telas do protótipo, pelos \
                 papéis `--prototipo-*`. Papel a confirmar não tem valor: mostre-o como \"a \
                 confirmar\" na tela, sem inventar o valor."
            } else if gravado {
                "Tokens em rascunho: ainda não use nas telas. Peça ao analista para revisar \
                 e aprovar os tokens CSS no app."
            } else {
                "Prévia extraída agora do código, sem gravar: os tokens ainda não foram \
                 extraídos nem revisados no app. Não use nas telas; peça ao analista para \
                 extrair, revisar e aprovar os tokens CSS no app."
            };
            let css = tokens.css();
            let texto = format!(
                "{orientacao}\n\nEstado: {}. Tokens do código: {}. Papéis a confirmar: {}.\n\n\
                 ```css\n{css}```\n",
                if gravado {
                    tokens.estado.nome()
                } else {
                    "prévia"
                },
                tokens.tokens.len(),
                if a_confirmar.is_empty() {
                    "nenhum".to_owned()
                } else {
                    a_confirmar.join(", ")
                },
            );
            Ok(Saida {
                texto,
                dados: Some(json!({
                    "estado": if gravado { tokens.estado.nome() } else { "previa" },
                    "aprovado": aprovado,
                    "css": css,
                    "papeis": tokens.papeis,
                    "a_confirmar": a_confirmar,
                    "arquivo": gravado.then(|| ambiente.kb.na_base(&cba_kb::layout::tokens_css())),
                })),
            })
        },
    )
    .somente_leitura()
}
