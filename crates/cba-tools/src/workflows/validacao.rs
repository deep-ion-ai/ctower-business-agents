//! Validação de um workflow ao salvar a cópia (spec
//! `docs/specs/2026-11-25-workflows-nas-analises.md`, seção 3.5; #372).
//!
//! Em Rust, sem Node: o que a análise estática não pega (erro de sintaxe
//! JavaScript) aparece na execução. Erros impedem salvar; avisos, não.
//!
//! - o script começa por `export const meta = {...}` (comentários antes valem) e o
//!   objeto é literal puro, lido como JSON5;
//! - `name` é `cba-<tipo>` e `description` não é vazia;
//! - `phases` não é vazio, os títulos são únicos e toda chamada `phase('...')` com
//!   literal tem título em `meta.phases`;
//! - não há `Date.now()`, `Math.random()` nem `new Date()` sem argumento (quebram a
//!   retomada pelo cache);
//! - o script lê `args` (aviso) e não tem anotações de tipo TypeScript óbvias (aviso);
//! - até [`LIMITE_BYTES`].

use serde::Serialize;

/// Tamanho máximo do script.
pub const LIMITE_BYTES: usize = 256 * 1024;

/// Erro ou aviso da validação.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Problema {
    /// Linha no script (a partir de 1), quando há uma.
    pub linha: Option<usize>,
    pub mensagem: String,
    /// Erro (impede salvar) ou só aviso.
    pub erro: bool,
}

/// Fase declarada em `meta.phases`, para a prévia.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Fase {
    pub titulo: String,
    pub detalhe: Option<String>,
}

/// Resultado de [`validar`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Validacao {
    /// Na ordem das linhas (os sem linha no fim).
    pub problemas: Vec<Problema>,
    /// As fases de `meta.phases`, quando o meta pôde ser lido.
    pub fases: Vec<Fase>,
}

impl Validacao {
    /// Há algum erro: a cópia não pode ser salva nem usada.
    pub fn tem_erro(&self) -> bool {
        self.problemas.iter().any(|p| p.erro)
    }

    /// Os erros, um por linha (`linha 3: ...`), para mensagens de texto.
    pub fn resumo_dos_erros(&self) -> String {
        self.problemas
            .iter()
            .filter(|p| p.erro)
            .map(|p| match p.linha {
                Some(l) => format!("linha {l}: {}", p.mensagem),
                None => p.mensagem.clone(),
            })
            .collect::<Vec<_>>()
            .join("; ")
    }

    fn erro(&mut self, texto: &str, pos: Option<usize>, mensagem: impl Into<String>) {
        self.problemas.push(Problema {
            linha: pos.map(|p| linha_de(texto, p)),
            mensagem: mensagem.into(),
            erro: true,
        });
    }

    fn aviso(&mut self, texto: &str, pos: Option<usize>, mensagem: impl Into<String>) {
        self.problemas.push(Problema {
            linha: pos.map(|p| linha_de(texto, p)),
            mensagem: mensagem.into(),
            erro: false,
        });
    }
}

/// Valida o script do workflow de `tipo`.
pub fn validar(tipo: &str, script: &str) -> Validacao {
    let mut v = Validacao::default();
    if script.len() > LIMITE_BYTES {
        v.erro(
            script,
            None,
            format!(
                "o script tem {} KiB; o limite é {} KiB",
                script.len().div_ceil(1024),
                LIMITE_BYTES / 1024
            ),
        );
        return v;
    }
    let (mascara, literais) = mascarar(script);
    let fim_do_meta = ler_meta(tipo, script, &mascara, &mut v);
    let codigo_desde = fim_do_meta.unwrap_or(0);

    if fim_do_meta.is_some() {
        conferir_fases_chamadas(script, &mascara, &literais, codigo_desde, &mut v);
    }
    for (padrao, nome) in [
        (&["Date", ".", "now", "("][..], "Date.now()"),
        (&["Math", ".", "random", "("][..], "Math.random()"),
        (&["new", "Date", "(", ")"][..], "new Date()"),
    ] {
        for pos in sequencias(&mascara, padrao, 0) {
            v.erro(
                script,
                Some(pos),
                format!(
                    "`{nome}` muda a cada execução e quebra a retomada do workflow; \
                     peça o valor ao agente ou receba-o nos args"
                ),
            );
        }
    }
    if palavras(&mascara, "args", codigo_desde).next().is_none() {
        v.aviso(
            script,
            None,
            "o script não lê `args`: a análise (A-NNNN) não chega ao workflow",
        );
    }
    typescript(script, &mascara, codigo_desde, &mut v);
    v.problemas
        .sort_by_key(|p| (p.linha.is_none(), p.linha.unwrap_or(0)));
    v
}

/// Linha (a partir de 1) da posição `pos` (em bytes).
fn linha_de(texto: &str, pos: usize) -> usize {
    texto.as_bytes()[..pos.min(texto.len())]
        .iter()
        .filter(|b| **b == b'\n')
        .count()
        + 1
}

/// Literal de string no script: posição da aspa de abertura, valor e se é template
/// com interpolação (`${`).
#[derive(Debug)]
struct Literal {
    inicio: usize,
    valor: String,
    interpolado: bool,
}

/// O script com comentários e o conteúdo das strings trocados por espaços (as
/// quebras de linha ficam e as posições em bytes não mudam), e os literais.
fn mascarar(texto: &str) -> (String, Vec<Literal>) {
    let b = texto.as_bytes();
    let mut saida = b.to_vec();
    let mut literais = Vec::new();
    let apagar = |saida: &mut Vec<u8>, de: usize, ate: usize| {
        for c in &mut saida[de..ate] {
            if *c != b'\n' {
                *c = b' ';
            }
        }
    };
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'/' if b.get(i + 1) == Some(&b'/') => {
                let fim = b[i..]
                    .iter()
                    .position(|c| *c == b'\n')
                    .map_or(b.len(), |p| i + p);
                apagar(&mut saida, i, fim);
                i = fim;
            }
            b'/' if b.get(i + 1) == Some(&b'*') => {
                let fim = texto[i + 2..].find("*/").map_or(b.len(), |p| i + 2 + p + 2);
                apagar(&mut saida, i, fim);
                i = fim;
            }
            q @ (b'\'' | b'"' | b'`') => {
                let mut j = i + 1;
                let mut interpolado = false;
                while j < b.len() && b[j] != q {
                    if b[j] == b'\\' {
                        j += 1;
                    } else if q != b'`' && b[j] == b'\n' {
                        break;
                    } else if q == b'`' && b[j] == b'$' && b.get(j + 1) == Some(&b'{') {
                        interpolado = true;
                    }
                    j += 1;
                }
                let fim = j.min(b.len());
                literais.push(Literal {
                    inicio: i,
                    valor: texto[i + 1..fim].to_string(),
                    interpolado,
                });
                apagar(&mut saida, i + 1, fim);
                i = (fim + 1).min(b.len());
            }
            _ => i += 1,
        }
    }
    (
        String::from_utf8(saida).expect("só bytes ASCII trocados por espaços"),
        literais,
    )
}

fn e_ident(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'$'
}

/// Posições de `palavra` como identificador inteiro em `mascara`, a partir de `desde`.
fn palavras<'a>(
    mascara: &'a str,
    palavra: &'a str,
    desde: usize,
) -> impl Iterator<Item = usize> + 'a {
    let b = mascara.as_bytes();
    mascara[desde..]
        .match_indices(palavra)
        .map(move |(p, _)| p + desde)
        .filter(move |&p| {
            (p == 0 || !e_ident(b[p - 1])) && b.get(p + palavra.len()).is_none_or(|c| !e_ident(*c))
        })
}

fn pular_espacos(b: &[u8], mut i: usize) -> usize {
    while i < b.len() && b[i].is_ascii_whitespace() {
        i += 1;
    }
    i
}

/// Se a sequência de `tokens` (identificadores ou pontuação, com espaços entre eles)
/// começa em `i`, devolve a posição logo depois dela.
fn casa(b: &[u8], mut i: usize, tokens: &[&str]) -> Option<usize> {
    for (n, t) in tokens.iter().enumerate() {
        if n > 0 {
            i = pular_espacos(b, i);
        }
        let tb = t.as_bytes();
        if !b[i..].starts_with(tb) {
            return None;
        }
        if tb.iter().all(|c| e_ident(*c)) && b.get(i + tb.len()).is_some_and(|c| e_ident(*c)) {
            return None;
        }
        i += tb.len();
    }
    Some(i)
}

/// Onde a sequência de `tokens` aparece em `mascara` (fora de comentários e strings).
fn sequencias(mascara: &str, tokens: &[&str], desde: usize) -> Vec<usize> {
    let b = mascara.as_bytes();
    palavras(mascara, tokens[0], desde)
        .filter(|&p| p == 0 || b[p - 1] != b'.')
        .filter(|&p| casa(b, p, tokens).is_some())
        .collect()
}

/// Lê o `export const meta = {...}` do início; devolve a posição logo depois do
/// objeto, se ele foi encontrado e fechado.
fn ler_meta(tipo: &str, script: &str, mascara: &str, v: &mut Validacao) -> Option<usize> {
    let b = mascara.as_bytes();
    let inicio = pular_espacos(b, 0);
    let Some(depois) = casa(b, inicio, &["export", "const", "meta", "="]) else {
        v.erro(
            script,
            Some(inicio),
            "o script deve começar por `export const meta = {...}` (só comentários antes)",
        );
        return None;
    };
    let abre = pular_espacos(b, depois);
    if b.get(abre) != Some(&b'{') {
        v.erro(
            script,
            Some(abre),
            "`meta` deve ser um objeto literal: `export const meta = { ... }`",
        );
        return None;
    }
    let mut nivel = 0usize;
    let mut fecha = None;
    for (i, c) in b.iter().enumerate().skip(abre) {
        match c {
            b'{' => nivel += 1,
            b'}' => {
                nivel -= 1;
                if nivel == 0 {
                    fecha = Some(i);
                    break;
                }
            }
            _ => {}
        }
    }
    let Some(fecha) = fecha else {
        v.erro(script, Some(abre), "o objeto `meta` não fecha (falta `}`)");
        return None;
    };
    let fim = fecha + 1;
    let valor = match json5::ler(script, abre, fim) {
        Ok(valor) => valor,
        Err((pos, mensagem)) => {
            v.erro(
                script,
                Some(pos),
                format!("`meta` precisa ser um literal puro (JSON5): {mensagem}"),
            );
            return Some(fim);
        }
    };
    let json5::Tipo::Objeto(campos) = &valor.tipo else {
        unreachable!("começa por `{{`");
    };
    let campo = |nome: &str| campos.iter().find(|(k, _)| k == nome).map(|(_, v)| v);

    let esperado = super::nome(tipo);
    match campo("name") {
        Some(json5::Valor {
            tipo: json5::Tipo::Texto(n),
            ..
        }) if *n == esperado => {}
        Some(outro) => v.erro(
            script,
            Some(outro.pos),
            format!("`name` deve ser '{esperado}': é o nome que o app instala e libera"),
        ),
        None => v.erro(
            script,
            Some(abre),
            format!("falta `name: '{esperado}'` no `meta`"),
        ),
    }
    match campo("description") {
        Some(json5::Valor {
            tipo: json5::Tipo::Texto(d),
            ..
        }) if !d.trim().is_empty() => {}
        Some(outro) => v.erro(
            script,
            Some(outro.pos),
            "`description` deve ser um texto não vazio",
        ),
        None => v.erro(script, Some(abre), "falta `description` no `meta`"),
    }
    match campo("phases") {
        Some(json5::Valor {
            tipo: json5::Tipo::Lista(itens),
            pos,
        }) => {
            if itens.is_empty() {
                v.erro(script, Some(*pos), "`phases` não pode ser vazio");
            }
            for item in itens {
                let json5::Tipo::Objeto(c) = &item.tipo else {
                    v.erro(
                        script,
                        Some(item.pos),
                        "cada fase deve ser um objeto `{ title: '...' }`",
                    );
                    continue;
                };
                let texto = |nome: &str| {
                    c.iter()
                        .find(|(k, _)| k == nome)
                        .and_then(|(_, v)| match &v.tipo {
                            json5::Tipo::Texto(t) => Some(t.clone()),
                            _ => None,
                        })
                };
                match texto("title").filter(|t| !t.trim().is_empty()) {
                    None => v.erro(
                        script,
                        Some(item.pos),
                        "a fase precisa de `title` com um texto não vazio",
                    ),
                    Some(t) if v.fases.iter().any(|f| f.titulo == t) => v.erro(
                        script,
                        Some(item.pos),
                        format!("a fase '{t}' aparece mais de uma vez em `phases`"),
                    ),
                    Some(titulo) => v.fases.push(Fase {
                        titulo,
                        detalhe: texto("detail"),
                    }),
                }
            }
        }
        Some(outro) => v.erro(
            script,
            Some(outro.pos),
            "`phases` deve ser uma lista de fases `[{ title: '...' }]`",
        ),
        None => v.erro(script, Some(abre), "falta `phases` no `meta`"),
    }
    Some(fim)
}

/// Toda chamada `phase('...')` com literal tem título em `meta.phases`.
fn conferir_fases_chamadas(
    script: &str,
    mascara: &str,
    literais: &[Literal],
    desde: usize,
    v: &mut Validacao,
) {
    let b = mascara.as_bytes();
    let titulos: Vec<String> = v.fases.iter().map(|f| f.titulo.clone()).collect();
    for pos in sequencias(mascara, &["phase", "("], desde) {
        let depois = casa(b, pos, &["phase", "("]).expect("casou");
        let arg = pular_espacos(b, depois);
        let Some(lit) = literais.iter().find(|l| l.inicio == arg) else {
            continue;
        };
        if lit.interpolado || titulos.contains(&lit.valor) {
            continue;
        }
        v.erro(
            script,
            Some(pos),
            format!(
                "a fase '{}' não está em `meta.phases`; acrescente-a lá ou corrija o título",
                lit.valor
            ),
        );
    }
}

/// Anotações de tipo TypeScript óbvias: o workflow é JavaScript.
fn typescript(script: &str, mascara: &str, desde: usize, v: &mut Validacao) {
    const TIPOS: &[&str] = &[
        "string", "number", "boolean", "any", "unknown", "void", "never",
    ];
    let b = mascara.as_bytes();
    let mut linhas = Vec::new();
    for pos in palavras(mascara, "interface", desde) {
        let depois = pular_espacos(b, pos + "interface".len());
        if b.get(depois).is_some_and(|c| c.is_ascii_alphabetic()) {
            linhas.push(pos);
        }
    }
    for t in TIPOS {
        for pos in palavras(mascara, t, desde) {
            let mut i = pos;
            while i > 0 && b[i - 1].is_ascii_whitespace() && b[i - 1] != b'\n' {
                i -= 1;
            }
            if i > 0 && b[i - 1] == b':' {
                linhas.push(pos);
            }
        }
    }
    let mut vistas = Vec::new();
    linhas.sort_unstable();
    for pos in linhas {
        let linha = linha_de(script, pos);
        if vistas.contains(&linha) {
            continue;
        }
        vistas.push(linha);
        v.aviso(
            script,
            Some(pos),
            "parece uma anotação de tipo TypeScript; o workflow é JavaScript e não roda com ela",
        );
    }
}

/// Leitor mínimo de JSON5 para o objeto `meta`: objetos, listas, textos com aspas
/// simples ou duplas, números, `true`, `false`, `null`, chaves sem aspas, vírgula
/// no fim e comentários. Guarda a posição de cada valor, para as mensagens.
mod json5 {
    #[derive(Debug)]
    pub struct Valor {
        /// Posição em bytes no script.
        pub pos: usize,
        pub tipo: Tipo,
    }

    #[derive(Debug)]
    pub enum Tipo {
        Objeto(Vec<(String, Valor)>),
        Lista(Vec<Valor>),
        Texto(String),
        Numero,
        Logico,
        Nulo,
    }

    type Resultado<T> = Result<T, (usize, String)>;

    struct Leitor<'a> {
        texto: &'a str,
        b: &'a [u8],
        i: usize,
        fim: usize,
    }

    /// Lê o valor em `texto[inicio..fim]`, que deve ocupar o trecho inteiro.
    pub fn ler(texto: &str, inicio: usize, fim: usize) -> Resultado<Valor> {
        let mut l = Leitor {
            texto,
            b: texto.as_bytes(),
            i: inicio,
            fim,
        };
        let v = l.valor()?;
        l.espacos()?;
        if l.i < l.fim {
            return Err((l.i, "sobra texto depois do objeto".into()));
        }
        Ok(v)
    }

    impl Leitor<'_> {
        fn atual(&self) -> Option<u8> {
            (self.i < self.fim).then(|| self.b[self.i])
        }

        fn espacos(&mut self) -> Resultado<()> {
            loop {
                match self.atual() {
                    Some(c) if c.is_ascii_whitespace() => self.i += 1,
                    Some(b'/') if self.b.get(self.i + 1) == Some(&b'/') => {
                        while self.atual().is_some_and(|c| c != b'\n') {
                            self.i += 1;
                        }
                    }
                    Some(b'/') if self.b.get(self.i + 1) == Some(&b'*') => {
                        let Some(p) = self.texto[self.i + 2..self.fim].find("*/") else {
                            return Err((self.i, "comentário sem `*/`".into()));
                        };
                        self.i += 2 + p + 2;
                    }
                    _ => return Ok(()),
                }
            }
        }

        fn valor(&mut self) -> Resultado<Valor> {
            self.espacos()?;
            let pos = self.i;
            let tipo = match self.atual() {
                None => return Err((pos, "o objeto termina antes do valor".into())),
                Some(b'{') => self.objeto()?,
                Some(b'[') => self.lista()?,
                Some(q @ (b'\'' | b'"')) => Tipo::Texto(self.texto_entre(q)?),
                Some(b'`') => {
                    return Err((
                        pos,
                        "use aspas simples ou duplas; crase (template) não é JSON5".into(),
                    ));
                }
                Some(c) if c == b'-' || c == b'+' || c == b'.' || c.is_ascii_digit() => {
                    self.numero()?
                }
                Some(c) if super::e_ident(c) => {
                    let nome = self.identificador();
                    match nome.as_str() {
                        "true" | "false" => Tipo::Logico,
                        "null" => Tipo::Nulo,
                        "Infinity" | "NaN" => Tipo::Numero,
                        _ => {
                            return Err((
                                pos,
                                format!(
                                    "`{nome}` não é um valor literal; o `meta` não pode usar \
                                     variáveis nem chamadas"
                                ),
                            ));
                        }
                    }
                }
                Some(_) => {
                    let c = self.texto[pos..].chars().next().unwrap_or(' ');
                    return Err((pos, format!("`{c}` inesperado")));
                }
            };
            Ok(Valor { pos, tipo })
        }

        fn identificador(&mut self) -> String {
            let de = self.i;
            while self.atual().is_some_and(super::e_ident) {
                self.i += 1;
            }
            self.texto[de..self.i].to_string()
        }

        fn numero(&mut self) -> Resultado<Tipo> {
            let de = self.i;
            while self
                .atual()
                .is_some_and(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'+' | b'-'))
            {
                self.i += 1;
            }
            let t = self.texto[de..self.i].trim_start_matches(['+', '-']);
            let ok = t == "Infinity"
                || t == "NaN"
                || t.strip_prefix("0x")
                    .or_else(|| t.strip_prefix("0X"))
                    .map_or_else(
                        || t.parse::<f64>().is_ok(),
                        |h| !h.is_empty() && h.chars().all(|c| c.is_ascii_hexdigit()),
                    );
            if ok {
                Ok(Tipo::Numero)
            } else {
                Err((de, format!("número inválido `{}`", &self.texto[de..self.i])))
            }
        }

        fn texto_entre(&mut self, q: u8) -> Resultado<String> {
            let pos = self.i;
            self.i += 1;
            let mut saida = String::new();
            loop {
                let Some(c) = self.texto[self.i..self.fim].chars().next() else {
                    return Err((pos, "texto sem a aspa de fechamento".into()));
                };
                self.i += c.len_utf8();
                match c {
                    _ if c as u32 == q as u32 => return Ok(saida),
                    '\n' => return Err((pos, "texto sem a aspa de fechamento na linha".into())),
                    '\\' => {
                        let Some(e) = self.texto[self.i..self.fim].chars().next() else {
                            return Err((pos, "texto sem a aspa de fechamento".into()));
                        };
                        self.i += e.len_utf8();
                        match e {
                            'n' => saida.push('\n'),
                            't' => saida.push('\t'),
                            'r' => saida.push('\r'),
                            '0' => saida.push('\0'),
                            'b' => saida.push('\u{8}'),
                            'f' => saida.push('\u{c}'),
                            'v' => saida.push('\u{b}'),
                            '\n' => {}
                            'u' => {
                                let hex = self.texto.get(self.i..self.i + 4).unwrap_or("");
                                let Some(ch) =
                                    u32::from_str_radix(hex, 16).ok().and_then(char::from_u32)
                                else {
                                    return Err((self.i, "escape `\\u` inválido".into()));
                                };
                                saida.push(ch);
                                self.i += 4;
                            }
                            outro => saida.push(outro),
                        }
                    }
                    _ => saida.push(c),
                }
            }
        }

        fn objeto(&mut self) -> Resultado<Tipo> {
            self.i += 1;
            let mut campos: Vec<(String, Valor)> = Vec::new();
            loop {
                self.espacos()?;
                let pos = self.i;
                let chave = match self.atual() {
                    Some(b'}') => {
                        self.i += 1;
                        return Ok(Tipo::Objeto(campos));
                    }
                    Some(q @ (b'\'' | b'"')) => self.texto_entre(q)?,
                    Some(c) if super::e_ident(c) && !c.is_ascii_digit() => self.identificador(),
                    Some(b'[') => {
                        return Err((pos, "chave calculada (`[...]`) não é literal".into()));
                    }
                    Some(b'.') => return Err((pos, "espalhamento (`...`) não é literal".into())),
                    _ => return Err((pos, "esperava o nome de um campo ou `}`".into())),
                };
                self.espacos()?;
                if self.atual() != Some(b':') {
                    return Err((
                        self.i,
                        format!("esperava `:` depois de `{chave}` (métodos e atalhos não valem)"),
                    ));
                }
                self.i += 1;
                let valor = self.valor()?;
                if campos.iter().any(|(k, _)| *k == chave) {
                    return Err((pos, format!("o campo `{chave}` aparece duas vezes")));
                }
                campos.push((chave, valor));
                self.espacos()?;
                match self.atual() {
                    Some(b',') => self.i += 1,
                    Some(b'}') => {}
                    _ => return Err((self.i, "esperava `,` ou `}`".into())),
                }
            }
        }

        fn lista(&mut self) -> Resultado<Tipo> {
            self.i += 1;
            let mut itens = Vec::new();
            loop {
                self.espacos()?;
                if self.atual() == Some(b']') {
                    self.i += 1;
                    return Ok(Tipo::Lista(itens));
                }
                itens.push(self.valor()?);
                self.espacos()?;
                match self.atual() {
                    Some(b',') => self.i += 1,
                    Some(b']') => {}
                    _ => return Err((self.i, "esperava `,` ou `]`".into())),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOM: &str = "\
// comentário antes do meta
export const meta = {
  name: 'cba-impacto',
  description: \"Análise\", // comentário
  phases: [
    { title: 'Preparar', detail: 'retoma' },
    { title: 'Investigar' },
  ],
}

const a = typeof args === 'string' ? args : args.analise
phase('Preparar')
phase(`Investigar`)
phase(`fase ${a}`)
// phase('Comentada') não conta, nem Date.now()
const t = 'phase(\"Na string\") Math.random()'
const d = new Date(2026, 1, 1)
";

    fn mensagens(v: &Validacao) -> Vec<(Option<usize>, bool, String)> {
        v.problemas
            .iter()
            .map(|p| (p.linha, p.erro, p.mensagem.clone()))
            .collect()
    }

    #[test]
    fn script_bom_passa_e_devolve_as_fases() {
        let v = validar("impacto", BOM);
        assert!(v.problemas.is_empty(), "{:?}", mensagens(&v));
        assert_eq!(
            v.fases,
            vec![
                Fase {
                    titulo: "Preparar".into(),
                    detalhe: Some("retoma".into())
                },
                Fase {
                    titulo: "Investigar".into(),
                    detalhe: None
                },
            ]
        );
    }

    #[test]
    fn o_padrao_embutido_passa() {
        for tipo in super::super::TIPOS {
            let v = validar(tipo, super::super::padrao(tipo).unwrap());
            assert!(v.problemas.is_empty(), "{tipo}: {:?}", mensagens(&v));
            assert!(!v.fases.is_empty());
        }
    }

    fn erro_na_linha(v: &Validacao, linha: usize, trecho: &str) {
        assert!(
            v.problemas
                .iter()
                .any(|p| p.erro && p.linha == Some(linha) && p.mensagem.contains(trecho)),
            "esperava erro na linha {linha} com {trecho:?}: {:?}",
            mensagens(v)
        );
        assert!(v.tem_erro());
    }

    #[test]
    fn sem_meta_no_inicio() {
        let v = validar("impacto", "// oi\n\nconst x = 1\nexport const meta = {}\n");
        erro_na_linha(&v, 3, "export const meta");
        let v = validar("impacto", "export const meta = criar()\n");
        erro_na_linha(&v, 1, "objeto literal");
        let v = validar("impacto", "export const meta = {\n  name: 'cba-impacto',\n");
        erro_na_linha(&v, 1, "não fecha");
    }

    #[test]
    fn meta_que_nao_e_literal() {
        let v = validar(
            "impacto",
            "export const meta = {\n  name: NOME,\n  phases: [],\n}\nargs\n",
        );
        erro_na_linha(&v, 2, "não é um valor literal");
        let v = validar("impacto", "export const meta = {\n  description: `x`,\n}\n");
        erro_na_linha(&v, 2, "crase");
        let v = validar("impacto", "export const meta = {\n  name: 'a' 'b',\n}\n");
        erro_na_linha(&v, 2, "esperava `,` ou `}`");
    }

    #[test]
    fn nome_descricao_e_fases() {
        let texto = BOM
            .replace("'cba-impacto'", "'meu-impacto'")
            .replace("\"Análise\"", "'  '")
            .replace("{ title: 'Investigar' }", "{ title: 'Preparar' }");
        let v = validar("impacto", &texto);
        erro_na_linha(&v, 3, "`name` deve ser 'cba-impacto'");
        erro_na_linha(&v, 4, "`description`");
        erro_na_linha(&v, 7, "mais de uma vez");
        // `Investigar` saiu de `phases`, mas o script a chama.
        erro_na_linha(&v, 13, "'Investigar' não está em `meta.phases`");

        let v = validar(
            "impacto",
            "export const meta = { name: 'cba-impacto', description: 'd', phases: [] }\nargs\n",
        );
        erro_na_linha(&v, 1, "`phases` não pode ser vazio");
        let v = validar(
            "impacto",
            "export const meta = { name: 'cba-impacto', description: 'd' }\nargs\n",
        );
        erro_na_linha(&v, 1, "falta `phases`");
    }

    #[test]
    fn o_que_quebra_a_retomada() {
        let texto = format!(
            "{BOM}const a = Date.now()\nconst b = Math . random()\nconst c = new Date( )\n"
        );
        let v = validar("impacto", &texto);
        let base = BOM.lines().count();
        erro_na_linha(&v, base + 1, "Date.now()");
        erro_na_linha(&v, base + 2, "Math.random()");
        erro_na_linha(&v, base + 3, "new Date()");
        // `new Date(2026, 1, 1)`, comentários e strings não contam.
        assert_eq!(v.problemas.len(), 3, "{:?}", mensagens(&v));
    }

    #[test]
    fn avisos_de_args_e_typescript_nao_impedem() {
        let texto = "\
export const meta = { name: 'cba-impacto', description: 'd', phases: [{ title: 'A' }] }
interface Entrada { analise: string }
function f(x: string): void {}
phase('A')
";
        let v = validar("impacto", texto);
        assert!(!v.tem_erro(), "{:?}", mensagens(&v));
        let avisos = mensagens(&v);
        assert_eq!(avisos.len(), 3, "{avisos:?}");
        assert_eq!(avisos[0].0, Some(2));
        assert_eq!(avisos[1].0, Some(3));
        assert!(avisos[1].2.contains("TypeScript"));
        assert_eq!(avisos[2].0, None);
        assert!(avisos[2].2.contains("args"));
    }

    #[test]
    fn tamanho_maximo() {
        let grande = format!("{BOM}//{}\n", "x".repeat(LIMITE_BYTES));
        let v = validar("impacto", &grande);
        assert!(v.tem_erro());
        assert!(v.problemas[0].mensagem.contains("256 KiB"));
    }
}
