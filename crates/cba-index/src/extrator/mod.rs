//! Extratores por linguagem: o que cada arquivo define (símbolos) e o que ele usa
//! (referências).
//!
//! Cada linguagem com extrator próprio é um módulo deste diretório que implementa
//! [`Extrator`]. As linguagens sem extrator próprio passam pelo [`generico`], que
//! acha definições por heurística e menções a nomes qualificados.
//!
//! ## Como acrescentar uma linguagem
//!
//! 1. Crie `src/extrator/<linguagem>.rs` com um tipo que implemente [`Extrator`]
//!    (veja [`sql`] como exemplo) e os testes dele no próprio módulo.
//! 2. Descomente as duas linhas reservadas para a linguagem neste arquivo: a
//!    declaração `mod` e a linha em [`Registro::padrao`]. Cada linguagem tem a sua
//!    linha, separada das outras, para que extratores feitos em paralelo não entrem
//!    em conflito no merge.
//!
//! Os tipos de símbolo e de referência ([`TipoSimbolo`], [`TipoReferencia`]) já
//! cobrem as linguagens do MVP; prefira usá-los a criar variantes novas. O léxico
//! do genérico ([`generico::Sintaxe`]) serve para separar comentários e textos de
//! linguagens com sintaxe parecida com C, Pascal ou Basic.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::linguagem::Linguagem;

pub mod generico;
pub mod sql;
// Linhas reservadas (#16). Descomente a da sua linguagem.
// pub mod java;
//
// pub mod csharp;
//
// pub mod typescript;
//
// Linhas reservadas (#17). Descomente a da sua linguagem.
pub mod cobol;
//
pub mod delphi;
//
pub mod vb6;

/// Arquivo entregue ao extrator.
#[derive(Debug, Clone, Copy)]
pub struct Fonte<'a> {
    /// Caminho relativo à raiz do sistema, com `/`.
    pub caminho: &'a str,
    pub linguagem: Linguagem,
    /// Conteúdo. Bytes que não são UTF-8 válido chegam como `U+FFFD` (os sistemas
    /// legados costumam usar Latin-1; os nomes de símbolos são ASCII).
    pub texto: &'a str,
}

/// Extrator de uma ou mais linguagens.
///
/// Recebe um arquivo e devolve os símbolos que ele define e as referências que
/// faz. Não precisa resolver as referências: o mapa liga cada uma aos símbolos de
/// todo o sistema pelo nome. Pode falhar (com uma mensagem) ou até entrar em
/// pânico: o mapeamento registra a falha no arquivo e segue para o próximo.
pub trait Extrator: Send + Sync {
    /// Nome estável, gravado no mapa (`"sql"`, `"java"`).
    fn nome(&self) -> &'static str;

    /// Versão do extrator. Aumente quando a saída mudar: o mapa reprocessa os
    /// arquivos extraídos por outra versão.
    fn versao(&self) -> u32;

    /// Linguagens que este extrator trata.
    fn linguagens(&self) -> &'static [Linguagem];

    fn extrair(&self, fonte: &Fonte<'_>) -> Result<Extracao, String>;
}

/// Extratores disponíveis, por linguagem.
pub struct Registro {
    dedicados: Vec<Box<dyn Extrator>>,
    generico: Box<dyn Extrator>,
}

impl Registro {
    /// Todos os extratores do produto.
    pub fn padrao() -> Self {
        let mut r = Self::so_generico();
        r.registrar(sql::ExtratorSql);
        // Linhas reservadas (#16). Descomente a da sua linguagem.
        // r.registrar(java::ExtratorJava);
        //
        // r.registrar(csharp::ExtratorCSharp);
        //
        // r.registrar(typescript::ExtratorTypeScript);
        //
        // Linhas reservadas (#17). Descomente a da sua linguagem.
        r.registrar(cobol::ExtratorCobol);
        //
        r.registrar(delphi::ExtratorDelphi);
        //
        r.registrar(vb6::ExtratorVb6);
        r
    }

    /// Só o extrator genérico, para todas as linguagens.
    pub fn so_generico() -> Self {
        Self {
            dedicados: Vec::new(),
            generico: Box::new(generico::ExtratorGenerico),
        }
    }

    /// Acrescenta um extrator. Para uma linguagem que já tem extrator, vale o
    /// último registrado.
    pub fn registrar(&mut self, extrator: impl Extrator + 'static) -> &mut Self {
        self.dedicados.insert(0, Box::new(extrator));
        self
    }

    /// Extrator para a linguagem: o dedicado, se houver, ou o genérico.
    pub fn para(&self, linguagem: Linguagem) -> &dyn Extrator {
        self.dedicados
            .iter()
            .find(|e| e.linguagens().contains(&linguagem))
            .unwrap_or(&self.generico)
            .as_ref()
    }
}

impl Default for Registro {
    fn default() -> Self {
        Self::padrao()
    }
}

impl fmt::Debug for Registro {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let nomes: Vec<_> = self.dedicados.iter().map(|e| e.nome()).collect();
        f.debug_struct("Registro")
            .field("dedicados", &nomes)
            .field("generico", &self.generico.nome())
            .finish()
    }
}

/// O que um extrator achou num arquivo.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Extracao {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub simbolos: Vec<Simbolo>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub referencias: Vec<Referencia>,
}

/// Intervalo de linhas, contadas a partir de 1, inclusivo nas duas pontas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Linhas {
    pub inicio: u32,
    pub fim: u32,
}

impl Linhas {
    pub fn new(inicio: u32, fim: u32) -> Self {
        Self {
            inicio,
            fim: fim.max(inicio),
        }
    }

    pub fn contem(&self, linha: u32) -> bool {
        (self.inicio..=self.fim).contains(&linha)
    }
}

impl fmt::Display for Linhas {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.inicio == self.fim {
            write!(f, "{}", self.inicio)
        } else {
            write!(f, "{}-{}", self.inicio, self.fim)
        }
    }
}

/// Algo que um arquivo define: tabela, coluna, procedure, classe, método, tela...
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Simbolo {
    /// Nome como aparece no código, sem qualificação (`FN_CALCULAR_MULTA`).
    pub nome: String,
    /// Nome qualificado pelo contêiner, com `.` (`PKG_COBRANCA.FN_CALCULAR_MULTA`,
    /// `FATURA.STATUS`, `exemplo.faturamento.Fatura.id`). Igual a `nome` quando
    /// não há contêiner. É por ele que as referências e o `pai` apontam.
    pub qualificado: String,
    pub tipo: TipoSimbolo,
    pub linhas: Linhas,
    /// Nome qualificado do símbolo que contém este, no mesmo arquivo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pai: Option<String>,
    /// Detalhe curto: tipo da coluna, assinatura da rotina, evento do trigger.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detalhe: Option<String>,
    /// Comentário que documenta o símbolo (o que vem logo antes dele ou um
    /// `COMMENT ON`), resumido.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comentario: Option<String>,
    /// Só a declaração (especificação de pacote, protótipo, declaração antecipada).
    /// Ao resolver referências, a implementação tem preferência.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub declaracao: bool,
}

impl Simbolo {
    /// Símbolo sem pai, detalhe nem comentário.
    pub fn novo(nome: impl Into<String>, tipo: TipoSimbolo, linhas: Linhas) -> Self {
        let nome = nome.into();
        Self {
            qualificado: nome.clone(),
            nome,
            tipo,
            linhas,
            pai: None,
            detalhe: None,
            comentario: None,
            declaracao: false,
        }
    }

    /// Coloca o símbolo dentro de `pai`, qualificando o nome.
    pub fn dentro_de(mut self, pai: &str) -> Self {
        self.qualificado = format!("{pai}.{}", self.nome);
        self.pai = Some(pai.to_string());
        self
    }
}

/// Tipo de símbolo. A lista cobre as linguagens do MVP; `Outro` fica para o resto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TipoSimbolo {
    // Banco de dados.
    Tabela,
    Coluna,
    Visao,
    Sequencia,
    Indice,
    Sinonimo,
    Pacote,
    Procedure,
    Funcao,
    Trigger,
    /// Tipo definido pelo usuário (`CREATE TYPE`, `type` do Pascal, `Type` do VB).
    Tipo,
    // Orientação a objetos e módulos.
    Namespace,
    Modulo,
    Classe,
    Interface,
    Enum,
    Registro,
    Metodo,
    Construtor,
    Propriedade,
    Campo,
    Constante,
    Variavel,
    // Legado e telas.
    /// Programa COBOL ou projeto Delphi/VB6.
    Programa,
    Secao,
    Paragrafo,
    Copybook,
    /// Formulário, tela ou página.
    Tela,
    /// Componente de tela (botão, campo, grade, data source).
    Componente,
    /// Rotina sem classificação melhor (sub, função de script).
    Rotina,
    Outro,
}

impl TipoSimbolo {
    /// Tabelas, visões e sequências: o que a ferramenta `schema` lista.
    pub fn eh_schema(self) -> bool {
        matches!(
            self,
            TipoSimbolo::Tabela
                | TipoSimbolo::Visao
                | TipoSimbolo::Sequencia
                | TipoSimbolo::Indice
                | TipoSimbolo::Sinonimo
        )
    }
}

/// Uso de um nome num arquivo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Referencia {
    /// Nome qualificado do símbolo deste arquivo de onde parte a referência (a
    /// rotina que chama). Ausente quando é do arquivo como um todo (script solto).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origem: Option<String>,
    /// Nome usado, como escrito (`PKG_COBRANCA.FN_CALCULAR_MULTA`, `FATURA`).
    pub alvo: String,
    pub tipo: TipoReferencia,
    pub linha: u32,
}

/// Natureza do uso.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TipoReferencia {
    /// Chamada de rotina, procedure, função ou método.
    Chamada,
    /// Leitura de tabela ou visão (`SELECT ... FROM`, `JOIN`, `USING`).
    Leitura,
    /// Escrita em tabela (`INSERT`, `UPDATE`, `DELETE`, `MERGE`).
    Escrita,
    /// Chave estrangeira de uma tabela para outra.
    ChaveEstrangeira,
    /// Uso como tipo (`%TYPE`, `%ROWTYPE`, declaração de variável).
    Tipo,
    Heranca,
    Implementacao,
    /// `import`, `uses`, `using`.
    Importacao,
    /// `COPY` do COBOL, `{$I}` do Delphi, `#include`.
    Inclusao,
    /// `new`, criação de objeto ou formulário.
    Instanciacao,
    /// Tabela que dispara o trigger.
    Disparo,
    /// Uso sem classificação melhor (sequência, constante).
    Uso,
    /// Nome que aparece no texto (em geral numa string, como o SQL embutido num
    /// programa). Só fica no mapa se o nome for de um símbolo conhecido.
    Mencao,
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Falso;
    impl Extrator for Falso {
        fn nome(&self) -> &'static str {
            "falso"
        }
        fn versao(&self) -> u32 {
            1
        }
        fn linguagens(&self) -> &'static [Linguagem] {
            &[Linguagem::Sql]
        }
        fn extrair(&self, _: &Fonte<'_>) -> Result<Extracao, String> {
            Err("falso".into())
        }
    }

    #[test]
    fn registro_escolhe_dedicado_ou_generico() {
        let r = Registro::padrao();
        assert_eq!(r.para(Linguagem::Sql).nome(), "sql");
        assert_eq!(r.para(Linguagem::Python).nome(), "generico");
        let mut r = Registro::so_generico();
        r.registrar(sql::ExtratorSql).registrar(Falso);
        assert_eq!(r.para(Linguagem::Sql).nome(), "falso");
        assert_eq!(
            format!("{r:?}"),
            r#"Registro { dedicados: ["falso", "sql"], generico: "generico" }"#
        );
    }

    #[test]
    fn linhas() {
        assert_eq!(Linhas::new(5, 3), Linhas::new(5, 5));
        assert_eq!(Linhas::new(2, 4).to_string(), "2-4");
        assert!(Linhas::new(2, 4).contem(4));
        let s = Simbolo::novo("FN", TipoSimbolo::Funcao, Linhas::new(1, 2)).dentro_de("PKG");
        assert_eq!(s.qualificado, "PKG.FN");
        assert_eq!(s.pai.as_deref(), Some("PKG"));
    }
}
