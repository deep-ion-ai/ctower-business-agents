//! Exportação do protótipo de telas numa pasta HTML (#380), para compartilhar
//! com o negócio: abre em qualquer navegador, sem o app e sem rede.
//!
//! A pasta ([`nome_da_pasta`]: id e título da análise) leva:
//!
//! - `index.html`: o título da análise, a origem, a lista das telas (`TL-n`,
//!   nome, nova ou alterada, motivo, quantos itens estão "a confirmar") com
//!   links para cada tela e para cada estado (`TL-n.html#estado-erro`). Tela
//!   ainda sem HTML aparece como "sem protótipo ainda".
//! - `TL-<n>.html`: a cópia do protótipo de cada tela, lida por
//!   [`crate::prototipo::ler_html_da_tela`] (confinada à pasta da análise), com
//!   um `<nav>` logo depois do `<body>` para voltar ao índice e ir à tela
//!   anterior ou à próxima. Só links relativos: nada de script, e a
//!   Content-Security-Policy da tela fica como está.
//! - `tokens.css`: cópia dos tokens CSS aprovados, para referência (as telas
//!   já trazem os tokens inline). Sem tokens aprovados, o arquivo não vai e o
//!   índice avisa que as telas estão em wireframe neutro.
//!
//! Pasta que já existe não é sobrescrita sem o analista confirmar
//! ([`ErroExportacaoPrototipo::PastaExiste`]); confirmado, os arquivos da
//! exportação são gravados por cima e o resto da pasta fica como está.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use cba_core::analise::{Analise, Fluxo};
use cba_core::prototipo::Tela;
use cba_core::prototipo_html::{EstadoTela, POLITICA_DE_CONTEUDO, escapar};
use cba_core::tokens::TokensProjeto;

use crate::prototipo::ler_html_da_tela;

/// Arquivo do índice na pasta exportada.
pub const INDICE: &str = "index.html";
/// Cópia dos tokens CSS aprovados na pasta exportada.
pub const TOKENS: &str = "tokens.css";
/// Classe do `<nav>` acrescentado a cada tela exportada.
pub const CLASSE_NAVEGACAO: &str = "prototipo-exportacao";

/// Erro da exportação do protótipo.
#[derive(Debug, thiserror::Error)]
pub enum ErroExportacaoPrototipo {
    /// A pasta de destino já existe: o analista precisa confirmar.
    #[error("a pasta {} já existe", .0.display())]
    PastaExiste(PathBuf),
    #[error("{0}")]
    Recusada(String),
    #[error("falha ao gravar {}: {erro}", caminho.display())]
    Gravar {
        caminho: PathBuf,
        #[source]
        erro: std::io::Error,
    },
}

/// Pasta exportada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrototipoExportado {
    pub pasta: PathBuf,
    /// Telas com protótipo copiado.
    pub telas: usize,
    /// Telas listadas no índice sem protótipo ainda.
    pub sem_prototipo: usize,
}

/// `A-0003-prototipo-do-parcelamento`: o id e o título da análise, sem acento
/// nem espaço.
pub fn nome_da_pasta(analise: &Analise) -> String {
    let titulo = cba_kb::slug(&analise.titulo);
    let titulo: String = titulo.chars().take(60).collect();
    let titulo = titulo.trim_end_matches('-');
    if titulo.is_empty() {
        format!("{}-prototipo", analise.id)
    } else {
        format!("{}-{titulo}", analise.id)
    }
}

/// Itens "a confirmar" da tela: o motivo e os elementos.
fn a_confirmar(tela: &Tela) -> usize {
    usize::from(tela.a_confirmar()) + tela.elementos_a_confirmar().count()
}

fn arquivo_da_tela(tela: &Tela) -> String {
    format!("{}.html", tela.rotulo())
}

/// Exporta o protótipo da análise em `destino/<nome_da_pasta>`. Com a pasta já
/// existindo, recusa com [`ErroExportacaoPrototipo::PastaExiste`] a menos que
/// `substituir`.
pub fn exportar_prototipo(
    kb: &cba_kb::Projeto,
    analise: &Analise,
    destino: &Path,
    substituir: bool,
) -> Result<PrototipoExportado, ErroExportacaoPrototipo> {
    if analise.fluxo != Fluxo::Prototipo {
        return Err(ErroExportacaoPrototipo::Recusada(format!(
            "a análise {} não é do fluxo `prototipo`",
            analise.id
        )));
    }
    if analise.telas().is_empty() {
        return Err(ErroExportacaoPrototipo::Recusada(format!(
            "a análise {} ainda não tem telas para exportar",
            analise.id
        )));
    }
    let pasta = destino.join(nome_da_pasta(analise));
    // `symlink_metadata`: um link com o nome da pasta também conta como existente.
    if let Ok(meta) = std::fs::symlink_metadata(&pasta) {
        if !substituir {
            return Err(ErroExportacaoPrototipo::PastaExiste(pasta));
        }
        if !meta.is_dir() {
            return Err(ErroExportacaoPrototipo::Recusada(format!(
                "{} já existe e não é uma pasta",
                pasta.display()
            )));
        }
    }

    // Tudo lido antes de gravar: uma tela ilegível não deixa pasta pela metade.
    let mut htmls: Vec<(&Tela, Option<String>)> = Vec::new();
    for tela in analise.telas() {
        let html = ler_html_da_tela(kb, analise, tela.numero)
            .map_err(ErroExportacaoPrototipo::Recusada)?;
        htmls.push((tela, html));
    }
    let tokens = TokensProjeto::css_aprovado(kb)
        .map_err(|e| ErroExportacaoPrototipo::Recusada(e.to_string()))?;

    let gravar = |nome: &str, conteudo: &str| {
        let caminho = pasta.join(nome);
        cba_kb::arquivo::gravar(&caminho, conteudo.as_bytes())
            .map_err(|erro| ErroExportacaoPrototipo::Gravar { caminho, erro })
    };
    std::fs::create_dir_all(&pasta).map_err(|erro| ErroExportacaoPrototipo::Gravar {
        caminho: pasta.clone(),
        erro,
    })?;

    let com_html: Vec<&Tela> = htmls
        .iter()
        .filter(|(_, h)| h.is_some())
        .map(|(t, _)| *t)
        .collect();
    for (i, (tela, html)) in htmls
        .iter()
        .filter_map(|(t, h)| h.as_ref().map(|h| (t, h)))
        .enumerate()
    {
        let anterior = i.checked_sub(1).map(|j| com_html[j]);
        let proxima = com_html.get(i + 1).copied();
        gravar(
            &arquivo_da_tela(tela),
            &com_navegacao(html, anterior, proxima),
        )?;
    }
    if let Some(css) = &tokens {
        gravar(TOKENS, &cba_core::prototipo_html::css_dos_tokens(css))?;
    }
    gravar(
        INDICE,
        &indice(
            analise,
            &htmls
                .iter()
                .map(|(t, h)| (*t, h.is_some()))
                .collect::<Vec<_>>(),
            tokens.is_some(),
        ),
    )?;
    Ok(PrototipoExportado {
        pasta,
        telas: com_html.len(),
        sem_prototipo: htmls.len() - com_html.len(),
    })
}

/// A tela com o `<nav>` da exportação logo depois da abertura do `<body>`.
/// Sem `<body>` (não acontece com o HTML montado pelo app), vai como está.
fn com_navegacao(html: &str, anterior: Option<&Tela>, proxima: Option<&Tela>) -> String {
    let minusculo = html.to_ascii_lowercase();
    let Some(inicio) = minusculo.find("<body") else {
        return html.to_owned();
    };
    let Some(fim) = minusculo[inicio..].find('>').map(|f| inicio + f + 1) else {
        return html.to_owned();
    };
    let link = |rotulo: &str, tela: &Tela| {
        format!(
            "<a href=\"{}\">{rotulo}: {} · {}</a>",
            arquivo_da_tela(tela),
            tela.rotulo(),
            escapar(&tela.nome)
        )
    };
    let mut partes = vec![format!("<a href=\"{INDICE}\">Índice das telas</a>")];
    partes.extend(anterior.map(|t| link("Anterior", t)));
    partes.extend(proxima.map(|t| link("Próxima", t)));
    let nav = format!(
        "\n<nav class=\"{CLASSE_NAVEGACAO}\" style=\"font: 13px/1.45 system-ui, -apple-system, \
         'Segoe UI', sans-serif; padding: 8px 16px; background: #f2f2f2; \
         border-bottom: 1px solid #d0d0d0;\">{}</nav>",
        partes.join(" · ")
    );
    let mut saida = String::with_capacity(html.len() + nav.len());
    saida.push_str(&html[..fim]);
    saida.push_str(&nav);
    saida.push_str(&html[fim..]);
    saida
}

const CSS_INDICE: &str = "body { margin: 0 auto; max-width: 960px; padding: 24px 16px; \
font: 14px/1.5 system-ui, -apple-system, 'Segoe UI', sans-serif; color: #2b2b2b; background: #ffffff; }
h1 { font-size: 22px; margin: 0 0 4px; }
.muted { color: #6b6b6b; }
table { border-collapse: collapse; width: 100%; margin-top: 16px; }
th, td { border: 1px solid #d0d0d0; padding: 6px 8px; text-align: left; vertical-align: top; }
th { background: #f2f2f2; }
.a-confirmar { font-style: italic; }
";

/// O `index.html`: a análise, a origem e as telas com os links.
fn indice(analise: &Analise, telas: &[(&Tela, bool)], com_tokens: bool) -> String {
    let mut h = String::new();
    let titulo = format!("{} · {}", analise.id, analise.titulo);
    let _ = write!(
        h,
        "<!doctype html>\n<html lang=\"pt-BR\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <meta http-equiv=\"Content-Security-Policy\" content=\"{POLITICA_DE_CONTEUDO}\">\n\
         <meta name=\"generator\" content=\"Ctower Business Agents\">\n\
         <title>Protótipo · {}</title>\n<style>\n{CSS_INDICE}</style>\n</head>\n<body>\n\
         <p class=\"muted\">Protótipo de telas · {} · abre sem o app e sem rede</p>\n\
         <h1>{}</h1>\n",
        escapar(&titulo),
        escapar(&analise.projeto),
        escapar(&titulo)
    );
    match &analise.origem {
        Some(origem) => {
            let _ = writeln!(h, "<p>Análise de origem: {origem}</p>");
        }
        None => h.push_str("<p>Análise de origem: não informada</p>\n"),
    }
    let total: usize = telas.iter().map(|(t, _)| a_confirmar(t)).sum();
    let _ = writeln!(
        h,
        "<p>{} · {}</p>",
        match telas.len() {
            1 => "1 tela".to_owned(),
            n => format!("{n} telas"),
        },
        match total {
            0 => "nada a confirmar".to_owned(),
            1 => "1 item a confirmar com o negócio".to_owned(),
            n => format!("{n} itens a confirmar com o negócio"),
        }
    );
    if com_tokens {
        let _ = writeln!(
            h,
            "<p class=\"muted\">Tokens CSS aprovados: <a href=\"{TOKENS}\">{TOKENS}</a> \
             (referência; as telas já trazem os tokens).</p>"
        );
    } else {
        h.push_str(
            "<p class=\"muted\">Sem tokens CSS aprovados: as telas estão em wireframe neutro.</p>\n",
        );
    }
    h.push_str(
        "<table>\n<thead><tr><th>Tela</th><th>Nome</th><th>Mudança</th><th>Motivo</th>\
         <th>A confirmar</th><th>Estados</th></tr></thead>\n<tbody>\n",
    );
    for (tela, tem_html) in telas {
        let arquivo = arquivo_da_tela(tela);
        let (rotulo, nome, estados) = if *tem_html {
            let estados: Vec<String> = EstadoTela::TODOS
                .iter()
                .map(|e| format!("<a href=\"{arquivo}#{}\">{}</a>", e.id(), e.rotulo()))
                .collect();
            (
                format!("<a href=\"{arquivo}\">{}</a>", tela.rotulo()),
                format!("<a href=\"{arquivo}\">{}</a>", escapar(&tela.nome)),
                estados.join(" · "),
            )
        } else {
            (
                tela.rotulo(),
                escapar(&tela.nome),
                "<span class=\"muted\">sem protótipo ainda</span>".to_owned(),
            )
        };
        let n = a_confirmar(tela);
        let confirmar = if n == 0 {
            "—".to_owned()
        } else {
            format!("<span class=\"a-confirmar\">{n}</span>")
        };
        let _ = writeln!(
            h,
            "<tr><td>{rotulo}</td><td>{nome}</td><td>{}</td><td>{}</td><td>{confirmar}</td>\
             <td>{estados}</td></tr>",
            tela.mudanca.rotulo(),
            escapar(&tela.motivo)
        );
    }
    h.push_str("</tbody>\n</table>\n</body>\n</html>\n");
    h
}

#[cfg(test)]
#[path = "prototipo_exportacao_testes.rs"]
mod tests;
