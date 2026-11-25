//! Tokens CSS do projeto analisado (#384), para o protótipo de telas (#356)
//! seguir a identidade visual do próprio projeto, não a do Control Tower.
//!
//! O app extrai do código da interface os tokens de cor, tipografia,
//! espaçamento e raio ([`cba_index::extrair_estilo`]) e liga a cada **papel**
//! que as telas do protótipo usam ([`PAPEIS`]: cor primária, cor do texto,
//! família da fonte...) o token do projeto que o representa, pelo nome. Papel
//! sem token no código fica **a confirmar**, sem valor inventado; o analista
//! informa o valor ou deixa como está.
//!
//! O resultado é um arquivo de variáveis CSS em `:root`
//! ([`TokensProjeto::css`]): os tokens do projeto com o valor e a origem
//! (arquivo e linha) e os papéis do protótipo (`--prototipo-cor-primaria:
//! var(--cor-primaria)`); o que está a confirmar fica comentado.
//!
//! Revisão: os tokens nascem em [`EstadoTokens::Rascunho`] e o analista os
//! aprova antes do uso nas telas. Extrair de novo ou mudar um papel volta ao
//! rascunho. Só o CSS aprovado vai para as telas ([`TokensProjeto::css_aprovado`]).
//!
//! O estado fica em `prototipo/tokens.estado.json` e o CSS, regravado a cada
//! alteração, em `prototipo/tokens.css` ([`cba_kb::layout`]). As alterações
//! passam pela trava do projeto ([`TokensProjeto::alterar`]), como o mapa de
//! funcionalidades.

use std::fmt::Write as _;
use std::fs::{self, OpenOptions};
use std::io;

use cba_index::{CategoriaToken, Escopo, OrigemToken, TokenEstilo};
use cba_kb::{CaminhoRelativo, layout};
use serde::{Deserialize, Serialize};

/// Versão do formato do estado dos tokens.
pub const FORMATO_DOS_TOKENS: u32 = 1;

/// Prefixo dos papéis no CSS, para não colidir com os nomes do projeto.
pub const PREFIXO_DO_PAPEL: &str = "--prototipo-";

/// Texto que marca o que não foi encontrado no código.
pub const A_CONFIRMAR: &str = "a confirmar";

/// Que tipo de valor um papel aceita na ligação automática.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TipoValor {
    Qualquer,
    /// Medida (`14px`, `1rem`, `var(...)`, `calc(...)`).
    Medida,
    /// Texto que não é medida (família da fonte).
    Texto,
}

/// Papel que as telas do protótipo usam.
#[derive(Debug, Clone, Copy)]
pub struct DefinicaoPapel {
    /// Nome sem o prefixo (`cor-primaria`).
    pub nome: &'static str,
    pub categoria: CategoriaToken,
    pub descricao: &'static str,
    /// Palavras do nome do token que indicam o papel, na ordem de preferência.
    palavras: &'static [&'static str],
    tipo: TipoValor,
}

/// Os papéis do protótipo, na ordem do CSS.
pub const PAPEIS: &[DefinicaoPapel] = &[
    DefinicaoPapel {
        nome: "cor-primaria",
        categoria: CategoriaToken::Cor,
        descricao: "Cor principal: botões, links e destaques",
        palavras: &[
            "primaria",
            "primario",
            "primary",
            "brand",
            "marca",
            "principal",
            "destaque",
        ],
        tipo: TipoValor::Qualquer,
    },
    DefinicaoPapel {
        nome: "cor-texto",
        categoria: CategoriaToken::Cor,
        descricao: "Cor do texto",
        palavras: &[
            "texto",
            "text",
            "foreground",
            "fg",
            "body-color",
            "root-color",
            "html-color",
        ],
        tipo: TipoValor::Qualquer,
    },
    DefinicaoPapel {
        nome: "cor-fundo",
        categoria: CategoriaToken::Cor,
        descricao: "Fundo da página",
        palavras: &[
            "fundo",
            "background",
            "bg",
            "body-background-color",
            "body-background",
        ],
        tipo: TipoValor::Qualquer,
    },
    DefinicaoPapel {
        nome: "cor-borda",
        categoria: CategoriaToken::Cor,
        descricao: "Bordas e divisores",
        palavras: &["borda", "border", "contorno", "divisor", "divider"],
        tipo: TipoValor::Qualquer,
    },
    DefinicaoPapel {
        nome: "cor-erro",
        categoria: CategoriaToken::Cor,
        descricao: "Mensagens de erro e de validação",
        palavras: &["erro", "error", "danger", "perigo", "invalido", "invalid"],
        tipo: TipoValor::Qualquer,
    },
    DefinicaoPapel {
        nome: "cor-alerta",
        categoria: CategoriaToken::Cor,
        descricao: "Avisos",
        palavras: &["alerta", "aviso", "warning", "atencao"],
        tipo: TipoValor::Qualquer,
    },
    DefinicaoPapel {
        nome: "cor-sucesso",
        categoria: CategoriaToken::Cor,
        descricao: "Confirmações de sucesso",
        palavras: &["sucesso", "success"],
        tipo: TipoValor::Qualquer,
    },
    DefinicaoPapel {
        nome: "fonte-familia",
        categoria: CategoriaToken::Tipografia,
        descricao: "Família da fonte do texto",
        palavras: &["familia", "family", "fonte", "font"],
        tipo: TipoValor::Texto,
    },
    DefinicaoPapel {
        nome: "fonte-tamanho-base",
        categoria: CategoriaToken::Tipografia,
        descricao: "Tamanho do texto",
        palavras: &[
            "tamanho-base",
            "size-base",
            "font-size",
            "base",
            "md",
            "tamanho",
            "size",
        ],
        tipo: TipoValor::Medida,
    },
    DefinicaoPapel {
        nome: "fonte-tamanho-titulo",
        categoria: CategoriaToken::Tipografia,
        descricao: "Tamanho dos títulos",
        palavras: &["titulo", "title", "heading", "h1", "xl", "lg"],
        tipo: TipoValor::Medida,
    },
    DefinicaoPapel {
        nome: "espaco-pequeno",
        categoria: CategoriaToken::Espacamento,
        descricao: "Espaço pequeno entre elementos",
        palavras: &["pequeno", "sm", "small", "xs"],
        tipo: TipoValor::Medida,
    },
    DefinicaoPapel {
        nome: "espaco-medio",
        categoria: CategoriaToken::Espacamento,
        descricao: "Espaço padrão entre elementos",
        palavras: &["base", "medio", "md", "medium", "padrao", "default"],
        tipo: TipoValor::Medida,
    },
    DefinicaoPapel {
        nome: "espaco-grande",
        categoria: CategoriaToken::Espacamento,
        descricao: "Espaço grande, entre seções",
        palavras: &["grande", "lg", "large", "xl"],
        tipo: TipoValor::Medida,
    },
    DefinicaoPapel {
        nome: "raio-borda",
        categoria: CategoriaToken::Raio,
        descricao: "Raio dos cantos (botões, campos, cartões)",
        palavras: &[
            "base", "md", "padrao", "default", "raio", "radius", "rounded",
        ],
        tipo: TipoValor::Medida,
    },
];

/// Estado da revisão dos tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EstadoTokens {
    /// Extraídos ou alterados, aguardam a revisão do analista. Não vão para as
    /// telas.
    #[default]
    Rascunho,
    /// Revisados e aprovados pelo analista: as telas usam.
    Aprovado,
}

impl EstadoTokens {
    pub fn nome(self) -> &'static str {
        match self {
            Self::Rascunho => "rascunho",
            Self::Aprovado => "aprovado",
        }
    }
}

/// De onde vem o valor de um papel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SituacaoPapel {
    /// Ligado a um token achado no código.
    Encontrado,
    /// Não achado no código (ou achado sem valor literal): sem valor.
    AConfirmar,
    /// Valor informado pelo analista na revisão.
    Informado,
}

/// Papel do protótipo com o valor da revisão.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PapelToken {
    /// Nome sem o prefixo (`cor-primaria`); no CSS, `--prototipo-cor-primaria`.
    pub nome: String,
    pub categoria: CategoriaToken,
    pub descricao: String,
    pub situacao: SituacaoPapel,
    /// Valor no CSS: `var(--token-do-projeto)` quando encontrado, o que o
    /// analista informou ou nada (a confirmar).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valor: Option<String>,
    /// Token do projeto ligado ao papel (`--cor-primaria`), quando encontrado.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origem: Option<OrigemToken>,
    /// Por que está a confirmar (o token achado não tem valor literal).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nota: Option<String>,
}

impl PapelToken {
    /// Nome da propriedade no CSS (`--prototipo-cor-primaria`).
    pub fn propriedade(&self) -> String {
        format!("{PREFIXO_DO_PAPEL}{}", self.nome)
    }

    pub fn a_confirmar(&self) -> bool {
        self.situacao == SituacaoPapel::AConfirmar
    }
}

/// Por que uma alteração dos tokens foi recusada ou falhou.
#[derive(Debug, thiserror::Error)]
pub enum ErroTokens {
    #[error(
        "os tokens CSS do projeto ainda não foram extraídos do código. Extraia-os \
         primeiro"
    )]
    SemTokens,
    #[error("papel desconhecido: `{0}`. Papéis: {papeis}", papeis = nomes_dos_papeis())]
    PapelDesconhecido(String),
    #[error(
        "valor inválido para o papel `{papel}`: {motivo}. Informe um valor de CSS, \
         como `#1f5fa8`, `14px` ou `var(--cor-primaria)`"
    )]
    ValorInvalido { papel: String, motivo: &'static str },
    #[error("não foi possível ler o estado dos tokens ({arquivo}): {erro}")]
    Formato {
        arquivo: CaminhoRelativo,
        erro: serde_json::Error,
    },
    #[error("não foi possível acessar {arquivo}: {erro}")]
    Io {
        arquivo: CaminhoRelativo,
        erro: io::Error,
    },
}

fn nomes_dos_papeis() -> String {
    PAPEIS.iter().map(|p| p.nome).collect::<Vec<_>>().join(", ")
}

/// Tokens CSS do projeto: os do código, os papéis do protótipo e a revisão.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokensProjeto {
    pub formato: u32,
    /// Aumenta a cada gravação.
    pub versao: u64,
    pub estado: EstadoTokens,
    /// Tokens achados no código, sem nomes repetidos (vale o primeiro), na
    /// ordem da extração.
    pub tokens: Vec<TokenEstilo>,
    /// Os papéis do protótipo, na ordem de [`PAPEIS`].
    pub papeis: Vec<PapelToken>,
    /// Última extração do código.
    pub extraido_ms: u64,
    /// Última alteração (extração ou revisão).
    pub atualizado_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aprovado_ms: Option<u64>,
}

impl TokensProjeto {
    /// Tokens novos, em rascunho, a partir dos achados no código.
    pub fn novos(extraidos: Vec<TokenEstilo>, agora: u64) -> Self {
        let tokens = sem_repetidos(extraidos);
        let papeis = PAPEIS.iter().map(|d| ligar(d, &tokens)).collect();
        Self {
            formato: FORMATO_DOS_TOKENS,
            versao: 0,
            estado: EstadoTokens::Rascunho,
            tokens,
            papeis,
            extraido_ms: agora,
            atualizado_ms: agora,
            aprovado_ms: None,
        }
    }

    /// Extração nova do código: troca os tokens e religa os papéis, mantendo o
    /// que o analista informou. Volta ao rascunho.
    pub fn reextrair(&mut self, extraidos: Vec<TokenEstilo>, agora: u64) {
        let anteriores = std::mem::take(&mut self.papeis);
        let mut novos = Self::novos(extraidos, agora);
        for papel in &mut novos.papeis {
            if let Some(a) = anteriores
                .iter()
                .find(|a| a.nome == papel.nome && a.situacao == SituacaoPapel::Informado)
            {
                *papel = a.clone();
            }
        }
        self.tokens = novos.tokens;
        self.papeis = novos.papeis;
        self.extraido_ms = agora;
        self.voltar_ao_rascunho();
    }

    /// Revisão de um papel (`cor-primaria` ou `--prototipo-cor-primaria`): com
    /// valor, o analista o informa; sem valor, volta ao que veio do código
    /// (token ligado ou a confirmar). Volta ao rascunho.
    pub fn informar(&mut self, papel: &str, valor: Option<&str>) -> Result<(), ErroTokens> {
        let nome = papel.trim().trim_start_matches(PREFIXO_DO_PAPEL).to_owned();
        let def = PAPEIS
            .iter()
            .find(|d| d.nome == nome)
            .ok_or_else(|| ErroTokens::PapelDesconhecido(papel.to_owned()))?;
        let novo = match valor.map(str::trim) {
            Some(v) => {
                validar_valor(v).map_err(|motivo| ErroTokens::ValorInvalido {
                    papel: nome.clone(),
                    motivo,
                })?;
                PapelToken {
                    nome: def.nome.to_owned(),
                    categoria: def.categoria,
                    descricao: def.descricao.to_owned(),
                    situacao: SituacaoPapel::Informado,
                    valor: Some(v.to_owned()),
                    token: None,
                    origem: None,
                    nota: None,
                }
            }
            None => ligar(def, &self.tokens),
        };
        if let Some(p) = self.papeis.iter_mut().find(|p| p.nome == nome) {
            *p = novo;
        } else {
            self.papeis.push(novo);
        }
        self.voltar_ao_rascunho();
        Ok(())
    }

    /// Aprovação do analista: as telas passam a usar estes tokens.
    pub fn aprovar(&mut self, agora: u64) {
        self.estado = EstadoTokens::Aprovado;
        self.aprovado_ms = Some(agora);
    }

    fn voltar_ao_rascunho(&mut self) {
        self.estado = EstadoTokens::Rascunho;
        self.aprovado_ms = None;
    }

    /// Papéis a confirmar (sem valor).
    pub fn papeis_a_confirmar(&self) -> impl Iterator<Item = &PapelToken> {
        self.papeis.iter().filter(|p| p.a_confirmar())
    }

    /// Tokens do código sem valor literal (a confirmar).
    pub fn tokens_a_confirmar(&self) -> impl Iterator<Item = &TokenEstilo> {
        self.tokens.iter().filter(|t| t.valor.is_none())
    }

    /// O arquivo de variáveis CSS (`:root`), com o estado da revisão no
    /// cabeçalho. O que está a confirmar fica comentado, sem valor.
    pub fn css(&self) -> String {
        let mut css = String::new();
        let _ = writeln!(
            css,
            "/*\n * Tokens CSS do projeto, extraídos do código da interface pelo Ctower \
             Business Agents.\n * Revisão: {}.\n * O que está \"{A_CONFIRMAR}\" não foi \
             encontrado no código e fica comentado, sem valor.\n */",
            match self.estado {
                EstadoTokens::Rascunho => "rascunho, aguarda a revisão do analista",
                EstadoTokens::Aprovado => "aprovado pelo analista",
            }
        );
        css.push_str(":root {\n");
        for cat in CategoriaToken::TODAS {
            let doscodigo: Vec<&TokenEstilo> =
                self.tokens.iter().filter(|t| t.categoria == cat).collect();
            if doscodigo.is_empty() {
                continue;
            }
            let _ = writeln!(css, "  /* Do código: {} */", cat.nome());
            for t in doscodigo {
                match &t.valor {
                    Some(v) => {
                        let _ = writeln!(css, "  {}: {v}; /* {} */", t.nome, t.origem);
                    }
                    None => {
                        let _ = writeln!(
                            css,
                            "  /* {}: {A_CONFIRMAR} ({}; {}) */",
                            t.nome,
                            t.origem,
                            comentario(t.nota.as_deref().unwrap_or("sem valor literal"))
                        );
                    }
                }
            }
            css.push('\n');
        }
        css.push_str("  /* Papéis usados nas telas do protótipo */\n");
        for p in &self.papeis {
            let prop = p.propriedade();
            match (&p.valor, p.situacao) {
                (Some(v), SituacaoPapel::Informado) => {
                    let _ = writeln!(css, "  {prop}: {v}; /* informado pelo analista */");
                }
                (Some(v), _) => {
                    let origem = p.origem.as_ref().map(ToString::to_string);
                    let _ = writeln!(
                        css,
                        "  {prop}: {v}; /* {} */",
                        origem.as_deref().unwrap_or("do código")
                    );
                }
                (None, _) => {
                    let _ = writeln!(
                        css,
                        "  /* {prop}: {A_CONFIRMAR} ({}) */",
                        comentario(p.nota.as_deref().unwrap_or(&p.descricao.to_lowercase()))
                    );
                }
            }
        }
        css.push_str("}\n");
        css
    }

    // -----------------------------------------------------------------------
    // Disco

    /// Lê o estado dos tokens do projeto; `None` antes da primeira extração.
    pub fn carregar(kb: &cba_kb::Projeto) -> Result<Option<Self>, ErroTokens> {
        let relativo = layout::estado_dos_tokens();
        match fs::read(kb.caminho(&relativo)) {
            Ok(bytes) => {
                serde_json::from_slice(&bytes)
                    .map(Some)
                    .map_err(|erro| ErroTokens::Formato {
                        arquivo: kb.na_base(&relativo),
                        erro,
                    })
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(erro) => Err(ErroTokens::Io {
                arquivo: kb.na_base(&relativo),
                erro,
            }),
        }
    }

    /// O CSS dos tokens aprovados pelo analista, para as telas do protótipo.
    /// `None` antes da extração e enquanto estiverem em rascunho.
    pub fn css_aprovado(kb: &cba_kb::Projeto) -> Result<Option<String>, ErroTokens> {
        Ok(Self::carregar(kb)?
            .filter(|t| t.estado == EstadoTokens::Aprovado)
            .map(|t| t.css()))
    }

    /// Extrai os tokens do código do projeto (`escopo`) e grava: cria o estado
    /// na primeira vez ou faz a nova extração ([`Self::reextrair`]).
    pub fn extrair(kb: &cba_kb::Projeto, escopo: &Escopo) -> Result<Self, ErroTokens> {
        let extraidos = cba_index::extrair_estilo(escopo);
        Self::gravar_extracao(kb, extraidos)
    }

    /// Grava uma extração já feita (os testes passam os tokens direto).
    pub fn gravar_extracao(
        kb: &cba_kb::Projeto,
        extraidos: Vec<TokenEstilo>,
    ) -> Result<Self, ErroTokens> {
        let _trava = travar(kb)?;
        let agora = crate::tempo::agora_ms();
        let mut tokens = match Self::carregar(kb)? {
            Some(mut t) => {
                t.reextrair(extraidos, agora);
                t
            }
            None => Self::novos(extraidos, agora),
        };
        tokens.atualizado_ms = agora;
        tokens.versao += 1;
        tokens.gravar(kb)?;
        Ok(tokens)
    }

    /// Altera os tokens sob a trava do projeto: lê, aplica `f` e, se ela não
    /// recusou, grava com a versão seguinte (estado e CSS).
    pub fn alterar<T>(
        kb: &cba_kb::Projeto,
        f: impl FnOnce(&mut Self, u64) -> Result<T, ErroTokens>,
    ) -> Result<(Self, T), ErroTokens> {
        let _trava = travar(kb)?;
        let mut tokens = Self::carregar(kb)?.ok_or(ErroTokens::SemTokens)?;
        let agora = crate::tempo::agora_ms();
        let resultado = f(&mut tokens, agora)?;
        tokens.versao += 1;
        tokens.atualizado_ms = agora;
        tokens.gravar(kb)?;
        Ok((tokens, resultado))
    }

    /// Grava o estado e o CSS (gravação atômica). Use [`Self::alterar`], que trava.
    fn gravar(&self, kb: &cba_kb::Projeto) -> Result<(), ErroTokens> {
        let mut json = serde_json::to_vec_pretty(self).expect("os tokens serializam");
        json.push(b'\n');
        gravar_arquivo(kb, &layout::estado_dos_tokens(), &json)?;
        gravar_arquivo(kb, &layout::tokens_css(), self.css().as_bytes())
    }
}

fn gravar_arquivo(
    kb: &cba_kb::Projeto,
    relativo: &CaminhoRelativo,
    conteudo: &[u8],
) -> Result<(), ErroTokens> {
    let destino = kb.caminho(relativo);
    let io_err = |erro| ErroTokens::Io {
        arquivo: kb.na_base(relativo),
        erro,
    };
    if let Some(pasta) = destino.parent() {
        fs::create_dir_all(pasta).map_err(io_err)?;
    }
    cba_kb::arquivo::gravar(&destino, conteudo).map_err(io_err)
}

fn travar(kb: &cba_kb::Projeto) -> Result<fs::File, ErroTokens> {
    let relativo = layout::trava_dos_tokens();
    let caminho = kb.caminho(&relativo);
    let io_err = |erro| ErroTokens::Io {
        arquivo: kb.na_base(&relativo),
        erro,
    };
    if let Some(pasta) = caminho.parent() {
        fs::create_dir_all(pasta).map_err(io_err)?;
    }
    let arquivo = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&caminho)
        .map_err(io_err)?;
    // Liberada quando o arquivo é fechado.
    arquivo.lock().map_err(io_err)?;
    Ok(arquivo)
}

/// Texto seguro dentro de um comentário do CSS.
fn comentario(texto: &str) -> String {
    texto.replace("*/", "* /")
}

/// Recusa o valor que quebraria o arquivo CSS (declaração, regra ou comentário
/// a mais).
fn validar_valor(valor: &str) -> Result<(), &'static str> {
    if valor.is_empty() {
        return Err("vazio (para deixar a confirmar, limpe o valor)");
    }
    if valor.contains(['\n', '\r']) {
        return Err("tem quebra de linha");
    }
    if valor.contains([';', '{', '}', '<', '>']) || valor.contains("/*") || valor.contains("*/") {
        return Err("tem `;`, chaves, `<`, `>` ou comentário");
    }
    Ok(())
}

/// Tokens sem nomes repetidos: vale o primeiro (a ordem da extração segue as
/// pastas e os nomes dos arquivos).
fn sem_repetidos(extraidos: Vec<TokenEstilo>) -> Vec<TokenEstilo> {
    let mut vistos = std::collections::HashSet::new();
    extraidos
        .into_iter()
        .filter(|t| vistos.insert(t.nome.clone()))
        .collect()
}

/// Se o valor de um token serve ao papel.
fn serve(tipo: TipoValor, valor: Option<&str>) -> bool {
    let Some(v) = valor else {
        // Sem valor literal: a ligação vale, e o papel fica a confirmar.
        return true;
    };
    let referencia = ["var(", "calc(", "clamp(", "min(", "max("]
        .iter()
        .any(|f| v.starts_with(f));
    let medida = v.starts_with(|c: char| c.is_ascii_digit() || c == '.') || referencia;
    match tipo {
        TipoValor::Qualquer => true,
        TipoValor::Medida => medida,
        TipoValor::Texto => !medida || referencia,
    }
}

/// Liga o papel ao token do projeto que o representa: o de mesma categoria
/// cujo nome tem a palavra de maior preferência; no empate, o de nome mais
/// curto e, depois, o primeiro. Sem token, o papel fica a confirmar.
fn ligar(def: &DefinicaoPapel, tokens: &[TokenEstilo]) -> PapelToken {
    let melhor = tokens
        .iter()
        .enumerate()
        .filter(|(_, t)| t.categoria == def.categoria && serve(def.tipo, t.valor.as_deref()))
        .filter_map(|(ordem, t)| {
            let partes: Vec<&str> = t
                .nome
                .trim_start_matches('-')
                .split('-')
                .filter(|p| !p.is_empty())
                .collect();
            let junto = format!("-{}-", partes.join("-"));
            def.palavras
                .iter()
                .position(|p| junto.contains(&format!("-{p}-")))
                .map(|preferencia| ((preferencia, partes.len(), ordem), t))
        })
        .min_by_key(|(chave, _)| *chave)
        .map(|(_, t)| t);
    let base = PapelToken {
        nome: def.nome.to_owned(),
        categoria: def.categoria,
        descricao: def.descricao.to_owned(),
        situacao: SituacaoPapel::AConfirmar,
        valor: None,
        token: None,
        origem: None,
        nota: None,
    };
    match melhor {
        None => base,
        Some(t) if t.valor.is_none() => PapelToken {
            token: Some(t.nome.clone()),
            origem: Some(t.origem.clone()),
            nota: Some(format!(
                "o token `{}` do código não tem valor literal: {}",
                t.nome,
                t.nota.as_deref().unwrap_or("sem valor")
            )),
            ..base
        },
        Some(t) => PapelToken {
            situacao: SituacaoPapel::Encontrado,
            valor: Some(format!("var({})", t.nome)),
            token: Some(t.nome.clone()),
            origem: Some(t.origem.clone()),
            ..base
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tk(nome: &str, categoria: CategoriaToken, valor: Option<&str>, linha: u32) -> TokenEstilo {
        TokenEstilo {
            nome: nome.into(),
            original: nome.into(),
            categoria,
            valor: valor.map(Into::into),
            origem: OrigemToken {
                arquivo: "frontend/src/estilos/tema.css".into(),
                linha,
            },
            nota: valor.is_none().then(|| "valor calculado".into()),
        }
    }

    fn papel<'a>(t: &'a TokensProjeto, nome: &str) -> &'a PapelToken {
        t.papeis.iter().find(|p| p.nome == nome).unwrap()
    }

    fn exemplo() -> Vec<TokenEstilo> {
        use CategoriaToken::*;
        vec![
            tk("--cor-alerta-fundo", Cor, None, 1),
            tk("--cor-fundo", Cor, Some("#ffffff"), 2),
            tk("--cor-primaria", Cor, Some("#1f5fa8"), 3),
            tk("--cor-primaria", Cor, Some("#000000"), 4),
            tk("--fonte-base", Tipografia, Some("\"Inter\", Arial"), 5),
            tk("--fonte-tamanho-base", Tipografia, Some("14px"), 6),
            tk("--raio-base", Raio, Some("4px"), 7),
        ]
    }

    #[test]
    fn liga_os_papeis_e_marca_o_que_falta_a_confirmar() {
        let t = TokensProjeto::novos(exemplo(), 10);
        assert_eq!(t.estado, EstadoTokens::Rascunho);
        // Nome repetido: vale o primeiro.
        assert_eq!(t.tokens.len(), 6);
        let p = papel(&t, "cor-primaria");
        assert_eq!(p.situacao, SituacaoPapel::Encontrado);
        assert_eq!(p.valor.as_deref(), Some("var(--cor-primaria)"));
        assert_eq!(p.origem.as_ref().unwrap().linha, 3);
        // Nome mais curto ganha no empate: o fundo da página, não o do alerta.
        assert_eq!(papel(&t, "cor-fundo").token.as_deref(), Some("--cor-fundo"));
        assert_eq!(
            papel(&t, "fonte-familia").token.as_deref(),
            Some("--fonte-base")
        );
        assert_eq!(
            papel(&t, "fonte-tamanho-base").token.as_deref(),
            Some("--fonte-tamanho-base")
        );
        assert_eq!(
            papel(&t, "raio-borda").token.as_deref(),
            Some("--raio-base")
        );
        // Token sem valor literal: ligado, mas a confirmar.
        let alerta = papel(&t, "cor-alerta");
        assert_eq!(alerta.situacao, SituacaoPapel::AConfirmar);
        assert_eq!(alerta.token.as_deref(), Some("--cor-alerta-fundo"));
        let sucesso = papel(&t, "cor-sucesso");
        assert_eq!(
            (sucesso.situacao, sucesso.valor.as_ref()),
            (SituacaoPapel::AConfirmar, None)
        );
        assert!(t.papeis_a_confirmar().any(|p| p.nome == "espaco-pequeno"));
        assert_eq!(t.tokens_a_confirmar().count(), 1);
    }

    #[test]
    fn css_em_root_com_a_confirmar_comentado() {
        let t = TokensProjeto::novos(exemplo(), 10);
        let css = t.css();
        assert!(css.contains("Revisão: rascunho"));
        assert!(css.contains(":root {\n"));
        assert!(css.contains("  --cor-primaria: #1f5fa8; /* frontend/src/estilos/tema.css:3 */"));
        assert!(css.contains("  /* --cor-alerta-fundo: a confirmar (frontend/src/estilos/tema.css:1; valor calculado) */"));
        assert!(css.contains("  --prototipo-cor-primaria: var(--cor-primaria);"));
        assert!(
            css.contains("  /* --prototipo-cor-sucesso: a confirmar (confirmações de sucesso) */")
        );
        assert!(!css.contains("#000000"));
        assert!(css.trim_end().ends_with('}'));
        // Toda declaração ativa tem valor: nada inventado para o que falta.
        for linha in css.lines().filter(|l| l.trim_start().starts_with("--")) {
            assert!(!linha.contains(A_CONFIRMAR), "{linha}");
        }
    }

    #[test]
    fn revisao_informa_restaura_aprova_e_reextrai() {
        let mut t = TokensProjeto::novos(exemplo(), 10);
        t.aprovar(20);
        assert_eq!(t.estado, EstadoTokens::Aprovado);
        assert!(t.css().contains("Revisão: aprovado"));

        t.informar("--prototipo-cor-sucesso", Some(" #2e7d32 "))
            .unwrap();
        assert_eq!(t.estado, EstadoTokens::Rascunho);
        assert_eq!(t.aprovado_ms, None);
        let s = papel(&t, "cor-sucesso");
        assert_eq!(
            (s.situacao, s.valor.as_deref()),
            (SituacaoPapel::Informado, Some("#2e7d32"))
        );
        assert!(
            t.css()
                .contains("--prototipo-cor-sucesso: #2e7d32; /* informado pelo analista */")
        );

        assert!(matches!(
            t.informar("cor-sucesso", Some("red; } body { x")),
            Err(ErroTokens::ValorInvalido { .. })
        ));
        assert!(matches!(
            t.informar("cor-inexistente", Some("red")),
            Err(ErroTokens::PapelDesconhecido(_))
        ));

        // A nova extração mantém o que o analista informou.
        t.reextrair(
            vec![tk("--cor-sucesso", CategoriaToken::Cor, Some("#0a0"), 1)],
            30,
        );
        assert_eq!(papel(&t, "cor-sucesso").valor.as_deref(), Some("#2e7d32"));
        assert_eq!(
            papel(&t, "cor-primaria").situacao,
            SituacaoPapel::AConfirmar
        );
        // Sem valor, volta ao que veio do código.
        t.informar("cor-sucesso", None).unwrap();
        assert_eq!(
            papel(&t, "cor-sucesso").valor.as_deref(),
            Some("var(--cor-sucesso)")
        );
    }

    fn projeto(tmp: &tempfile::TempDir) -> cba_kb::Projeto {
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
        base.criar_projeto(&doc).unwrap()
    }

    #[test]
    fn grava_o_estado_e_o_css_e_so_libera_o_aprovado() {
        let tmp = tempfile::tempdir().unwrap();
        let kb = projeto(&tmp);
        assert_eq!(TokensProjeto::carregar(&kb).unwrap(), None);
        assert!(matches!(
            TokensProjeto::alterar(&kb, |t, a| {
                t.aprovar(a);
                Ok(())
            }),
            Err(ErroTokens::SemTokens)
        ));

        let t = TokensProjeto::gravar_extracao(&kb, exemplo()).unwrap();
        assert_eq!(t.versao, 1);
        let css = fs::read_to_string(kb.caminho(&layout::tokens_css())).unwrap();
        assert_eq!(css, t.css());
        assert_eq!(TokensProjeto::css_aprovado(&kb).unwrap(), None);

        // Recusa não grava.
        assert!(TokensProjeto::alterar(&kb, |t, _| t.informar("x", None)).is_err());
        assert_eq!(TokensProjeto::carregar(&kb).unwrap().unwrap().versao, 1);

        let (t, ()) = TokensProjeto::alterar(&kb, |t, agora| {
            t.aprovar(agora);
            Ok(())
        })
        .unwrap();
        assert_eq!((t.versao, t.estado), (2, EstadoTokens::Aprovado));
        let aprovado = TokensProjeto::css_aprovado(&kb).unwrap().unwrap();
        assert!(aprovado.contains("Revisão: aprovado"));

        // Extrair de novo volta ao rascunho.
        let t = TokensProjeto::gravar_extracao(&kb, exemplo()).unwrap();
        assert_eq!((t.versao, t.estado), (3, EstadoTokens::Rascunho));
        assert_eq!(TokensProjeto::css_aprovado(&kb).unwrap(), None);
    }
}
