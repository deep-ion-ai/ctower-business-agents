//! Extrator de C#.
//!
//! Usa o léxico e a análise estrutural comuns às linguagens de chaves
//! ([`super::java::chaves`]: comentários, diretivas `#`, strings verbatim,
//! interpoladas e raw, blocos por chaves). Reconhece:
//!
//! - `namespace` (com chaves ou do arquivo), `using`;
//! - classes, interfaces, structs, records (com os parâmetros posicionais), enums
//!   (com as constantes), delegates;
//! - métodos, construtores, propriedades (`{ get; set; }` e `=>`), campos e
//!   constantes (`const`, `static readonly`), com a assinatura no detalhe e o
//!   comentário XML (`///`) como comentário;
//! - entidades do Entity Framework (`[Table]`, `[Column]`): a classe referencia a
//!   tabela e a propriedade, a coluna;
//! - endpoints do ASP.NET: atributos (`[Route]`, `[HttpGet]`..., com
//!   `[controller]` e `[action]`) e minimal APIs (`app.MapGet(...)`), como
//!   símbolos com a rota normalizada no nome;
//! - referências: herança e interfaces, tipos, `new`, chamadas (pelo tipo
//!   declarado do receptor), constantes de outras classes, chamadas do
//!   `HttpClient` (`GetAsync`, `PostAsJsonAsync`...) e menções ao SQL embutido
//!   nas strings.

use super::java::chaves::{self, Anotacao, Dialeto, Http, InfoClasse, Ling, juntar_rota};
use super::{Extracao, Extrator, Fonte};
use crate::linguagem::Linguagem;

/// Extrator de C#.
#[derive(Debug, Clone, Copy, Default)]
pub struct ExtratorCSharp;

impl Extrator for ExtratorCSharp {
    fn nome(&self) -> &'static str {
        "csharp"
    }

    fn versao(&self) -> u32 {
        1
    }

    fn linguagens(&self) -> &'static [Linguagem] {
        &[Linguagem::CSharp]
    }

    fn extrair(&self, fonte: &Fonte<'_>) -> Result<Extracao, String> {
        Ok(chaves::analisar(fonte, &DIALETO))
    }
}

const DIALETO: Dialeto = Dialeto {
    ling: Ling::CSharp,
    classe,
    rotas,
    coluna,
    http,
    externo: |_| false,
};

fn atributo<'a>(anots: &'a [Anotacao], nome: &str) -> Option<&'a Anotacao> {
    anots.iter().find(|a| a.nome == nome)
}

/// Nome do controlador para `[controller]`: sem o sufixo `Controller`.
fn controlador(nome: &str) -> &str {
    nome.strip_suffix("Controller").unwrap_or(nome)
}

fn classe(anots: &[Anotacao], nome: &str, bases: &[String]) -> InfoClasse {
    let mut info = InfoClasse::default();
    if let Some(t) = atributo(anots, "Table") {
        let mut tabela = t.textos(&["Name"]).into_iter().next().unwrap_or_default();
        if let Some(esquema) = t.par("Schema").filter(|e| !e.is_empty()) {
            tabela = format!("{esquema}.{tabela}");
        }
        if !tabela.is_empty() {
            info.detalhe = Some(format!("entidade EF (tabela {tabela})"));
            info.tabela = Some(tabela);
        }
    }
    let eh_controlador = atributo(anots, "ApiController").is_some()
        || nome.ends_with("Controller")
        || bases
            .iter()
            .any(|b| matches!(b.rsplit('.').next(), Some("ControllerBase" | "Controller")));
    let prefixos: Vec<String> = atributo(anots, "Route")
        .map(|r| {
            r.textos(&["Template"])
                .into_iter()
                .map(|t| t.replace("[controller]", controlador(nome)))
                .collect()
        })
        .unwrap_or_default();
    if eh_controlador || !prefixos.is_empty() {
        info.rotas = Some(if prefixos.is_empty() {
            vec![String::new()]
        } else {
            prefixos
        });
    }
    info
}

fn rotas(anots: &[Anotacao], metodo: &str, classe: &InfoClasse) -> Vec<(String, String)> {
    let substituir = |t: &str| {
        t.replace("[controller]", controlador(&classe.nome))
            .replace("[action]", metodo)
    };
    let rotas_metodo: Vec<String> = anots
        .iter()
        .filter(|a| a.nome == "Route")
        .flat_map(|a| a.textos(&["Template"]))
        .collect();
    let mut verbos: Vec<(String, Option<String>)> = Vec::new();
    for a in anots {
        let Some(v) = a.nome.strip_prefix("Http") else {
            continue;
        };
        if !matches!(
            v,
            "Get" | "Post" | "Put" | "Delete" | "Patch" | "Head" | "Options"
        ) {
            continue;
        }
        let verbo = v.to_ascii_uppercase();
        let templates = a.textos(&["Template"]);
        if templates.is_empty() {
            verbos.push((verbo, None));
        } else {
            verbos.extend(templates.into_iter().map(|t| (verbo.clone(), Some(t))));
        }
    }
    if verbos.is_empty() && !rotas_metodo.is_empty() {
        verbos.push((String::new(), None));
    }
    let prefixos = classe.rotas.clone().unwrap_or_else(|| vec![String::new()]);
    let mut v = Vec::new();
    for (verbo, template) in verbos {
        let templates = match template {
            Some(t) => vec![t],
            None if rotas_metodo.is_empty() => vec![String::new()],
            None => rotas_metodo.clone(),
        };
        for t in templates {
            let t = substituir(&t);
            // Template que começa com `/` ou `~/` ignora o prefixo da classe.
            if t.starts_with('/') || t.starts_with("~/") {
                v.push((verbo.clone(), juntar_rota("", t.trim_start_matches('~'))));
                continue;
            }
            for p in &prefixos {
                v.push((verbo.clone(), juntar_rota(&substituir(p), &t)));
            }
        }
    }
    v
}

fn coluna(anots: &[Anotacao]) -> Option<String> {
    atributo(anots, "Column")
        .and_then(|a| a.textos(&["Name"]).into_iter().next())
        .filter(|c| !c.is_empty())
}

fn http(receptor: Option<&str>, _tipo: Option<&str>, metodo: &str) -> Option<Http> {
    receptor?;
    match metodo {
        "GetAsync"
        | "PostAsync"
        | "PutAsync"
        | "DeleteAsync"
        | "PatchAsync"
        | "GetStringAsync"
        | "GetStreamAsync"
        | "GetByteArrayAsync"
        | "GetFromJsonAsync"
        | "PostAsJsonAsync"
        | "PutAsJsonAsync"
        | "PatchAsJsonAsync"
        | "DeleteFromJsonAsync" => Some(Http::Chamada),
        "MapGet" | "MapPost" | "MapPut" | "MapDelete" | "MapPatch" => Some(Http::Rota(
            metodo.trim_start_matches("Map").to_ascii_uppercase(),
        )),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extrator::{Linhas, Simbolo, TipoReferencia, TipoSimbolo};

    fn extrair(texto: &str) -> Extracao {
        ExtratorCSharp
            .extrair(&Fonte {
                caminho: "src/X.cs",
                linguagem: Linguagem::CSharp,
                texto,
            })
            .unwrap()
    }

    fn simbolo<'a>(e: &'a Extracao, q: &str) -> &'a Simbolo {
        e.simbolos
            .iter()
            .find(|s| s.qualificado == q)
            .unwrap_or_else(|| panic!("{q} não achado em {:#?}", e.simbolos))
    }

    fn refs(e: &Extracao, tipo: TipoReferencia) -> Vec<(Option<&str>, &str, u32)> {
        e.referencias
            .iter()
            .filter(|r| r.tipo == tipo)
            .map(|r| (r.origem.as_deref(), r.alvo.as_str(), r.linha))
            .collect()
    }

    const CONTROLADOR: &str = r#"using System;
using System.ComponentModel.DataAnnotations.Schema;
using Microsoft.AspNetCore.Mvc;

namespace Exemplo.Faturamento
{
    #region Entidades
    /// <summary>
    /// Fatura emitida.
    /// </summary>
    [Table("FATURA", Schema = "APP")]
    public class Fatura : EntidadeBase, IAuditavel
    {
        public const int MaxParcelas = 12;

        [Column("ID_FATURA")]
        public long Id { get; set; }

        [Column("VL_ORIGINAL")]
        public decimal ValorOriginal { get; set; } = 0m;

        public string Status => "ABERTA";

        public Fatura() { }
    }
    #endregion

    [ApiController]
    [Route("api/[controller]")]
    public class FaturasController : ControllerBase
    {
        private readonly IParcelamentoService _servico;

        public FaturasController(IParcelamentoService servico) => _servico = servico;

        [HttpGet("{id:long}")]
        public ActionResult<Fatura> Buscar(long id)
        {
            var sql = @"select *
                          from FATURA
                         where ID_FATURA = @id";
            return Ok(Repositorio.Ler<Fatura>(sql, id));
        }

        [HttpPost("{id}/parcelamento")]
        public async Task<IActionResult> Parcelar(long id, [FromBody] Pedido pedido)
        {
            var parcelas = await _servico.ParcelarAsync(id, pedido.Quantidade);
            var log = $"parcelado {id} em PKG_COBRANCA.FN_CALCULAR_MULTA";
            return Ok(new Resultado(parcelas));
        }

        [HttpDelete("~/api/v2/faturas/{id}")]
        [Route("[action]")]
        public void Remover(long id) { }
    }

    public record Pedido(int Quantidade, string Motivo);

    public interface IParcelamentoService
    {
        Task<Parcela[]> ParcelarAsync(long id, int quantidade);
    }

    public enum TipoCliente { PF = 1, PJ, [Obsoleto] PUB }

    public delegate void Notificar(string msg);
}
"#;

    #[test]
    fn tipos_membros_e_entidade() {
        let e = extrair(CONTROLADOR);
        let ns = simbolo(&e, "Exemplo.Faturamento");
        assert_eq!(ns.tipo, TipoSimbolo::Namespace);
        assert_eq!(ns.linhas, Linhas::new(5, 68));
        let f = simbolo(&e, "Exemplo.Faturamento.Fatura");
        assert_eq!(f.tipo, TipoSimbolo::Classe);
        assert_eq!(f.linhas, Linhas::new(11, 25));
        assert_eq!(f.comentario.as_deref(), Some("Fatura emitida."));
        assert_eq!(
            f.detalhe.as_deref(),
            Some("entidade EF (tabela APP.FATURA)")
        );
        let q = |s: &str| format!("Exemplo.Faturamento.Fatura.{s}");
        assert_eq!(simbolo(&e, &q("MaxParcelas")).tipo, TipoSimbolo::Constante);
        let id = simbolo(&e, &q("Id"));
        assert_eq!(id.tipo, TipoSimbolo::Propriedade);
        assert_eq!(id.linhas, Linhas::new(16, 17));
        assert_eq!(
            simbolo(&e, &q("ValorOriginal")).tipo,
            TipoSimbolo::Propriedade
        );
        assert_eq!(simbolo(&e, &q("Status")).tipo, TipoSimbolo::Propriedade);
        assert_eq!(simbolo(&e, &q("Fatura")).tipo, TipoSimbolo::Construtor);

        let classe = Some("Exemplo.Faturamento.Fatura");
        assert_eq!(
            refs(&e, TipoReferencia::Heranca)[0],
            (classe, "EntidadeBase", 12)
        );
        assert!(refs(&e, TipoReferencia::Implementacao).contains(&(classe, "IAuditavel", 12)));
        let usos = refs(&e, TipoReferencia::Uso);
        assert!(usos.contains(&(classe, "APP.FATURA", 12)));
        assert!(usos.contains(&(
            Some("Exemplo.Faturamento.Fatura.Id"),
            "APP.FATURA.ID_FATURA",
            17
        )));
        assert!(usos.contains(&(
            Some("Exemplo.Faturamento.Fatura.ValorOriginal"),
            "APP.FATURA.VL_ORIGINAL",
            20
        )));
        assert_eq!(
            refs(&e, TipoReferencia::Importacao),
            [
                (None, "System", 1),
                (None, "System.ComponentModel.DataAnnotations.Schema", 2),
                (None, "Microsoft.AspNetCore.Mvc", 3)
            ]
        );

        let p = simbolo(&e, "Exemplo.Faturamento.Pedido");
        assert_eq!(p.tipo, TipoSimbolo::Registro);
        assert_eq!(
            simbolo(&e, "Exemplo.Faturamento.Pedido.Quantidade").tipo,
            TipoSimbolo::Propriedade
        );
        let m = simbolo(&e, "Exemplo.Faturamento.IParcelamentoService.ParcelarAsync");
        assert!(m.declaracao);
        let en = simbolo(&e, "Exemplo.Faturamento.TipoCliente");
        assert_eq!(en.tipo, TipoSimbolo::Enum);
        for c in ["PF", "PJ", "PUB"] {
            assert_eq!(
                simbolo(&e, &format!("Exemplo.Faturamento.TipoCliente.{c}")).tipo,
                TipoSimbolo::Constante
            );
        }
        assert_eq!(
            simbolo(&e, "Exemplo.Faturamento.Notificar").tipo,
            TipoSimbolo::Tipo
        );
    }

    #[test]
    fn endpoints_e_chamadas() {
        let e = extrair(CONTROLADOR);
        let c = "Exemplo.Faturamento.FaturasController";
        let ctor = simbolo(&e, &format!("{c}.FaturasController"));
        assert_eq!(ctor.tipo, TipoSimbolo::Construtor);
        assert_eq!(ctor.linhas, Linhas::new(34, 34));
        let endpoints: Vec<_> = e
            .simbolos
            .iter()
            .filter(|s| s.tipo == TipoSimbolo::Outro)
            .map(|s| (s.qualificado.as_str(), s.detalhe.as_deref().unwrap()))
            .collect();
        assert_eq!(
            endpoints,
            [
                (
                    "Exemplo.Faturamento.FaturasController.Buscar./api/faturas/{}",
                    "endpoint GET /api/Faturas/{id:long}"
                ),
                (
                    "Exemplo.Faturamento.FaturasController.Parcelar./api/faturas/{}/parcelamento",
                    "endpoint POST /api/Faturas/{id}/parcelamento"
                ),
                (
                    "Exemplo.Faturamento.FaturasController.Remover./api/v2/faturas/{}",
                    "endpoint DELETE /api/v2/faturas/{id}"
                ),
            ]
        );
        let parcelar = Some("Exemplo.Faturamento.FaturasController.Parcelar");
        let chamadas = refs(&e, TipoReferencia::Chamada);
        assert!(chamadas.contains(&(parcelar, "IParcelamentoService.ParcelarAsync", 48)));
        assert!(refs(&e, TipoReferencia::Instanciacao).contains(&(parcelar, "Resultado", 50)));
        let mencoes = refs(&e, TipoReferencia::Mencao);
        let buscar = Some("Exemplo.Faturamento.FaturasController.Buscar");
        assert!(mencoes.contains(&(buscar, "FATURA", 40)));
        assert!(mencoes.contains(&(buscar, "ID_FATURA", 41)));
        assert!(mencoes.contains(&(parcelar, "PKG_COBRANCA.FN_CALCULAR_MULTA", 49)));
        // Interpolação não é menção; atributos também não.
        assert!(!mencoes.iter().any(|m| m.1 == "id" || m.1 == "parcelamento"));
    }

    #[test]
    fn namespace_do_arquivo_minimal_api_e_http_client() {
        let e = extrair(
            r#"namespace Exemplo.Portal;

var app = WebApplication.CreateBuilder(args).Build();
app.MapGet("/api/faturas/{id}", (long id) => Results.Ok());

public class ClienteFaturas
{
    private readonly HttpClient _http;

    public async Task<Fatura?> Buscar(long id) =>
        await _http.GetFromJsonAsync<Fatura>($"api/faturas/{id}");

    public Task Parcelar(long id) => _http.PostAsJsonAsync(Base + "/" + id + "/parcelamento", new { });

    private const string Base = "/api/faturas";
}
"#,
        );
        let ns = simbolo(&e, "Exemplo.Portal");
        assert_eq!(ns.linhas, Linhas::new(1, 16));
        let rota = simbolo(&e, "/api/faturas/{}");
        assert_eq!(
            rota.detalhe.as_deref(),
            Some("endpoint GET /api/faturas/{id}")
        );
        assert_eq!(rota.linhas, Linhas::new(4, 4));
        let c = simbolo(&e, "Exemplo.Portal.ClienteFaturas");
        assert_eq!(c.linhas, Linhas::new(6, 16));
        let chamadas = refs(&e, TipoReferencia::Chamada);
        assert!(chamadas.contains(&(
            Some("Exemplo.Portal.ClienteFaturas.Buscar"),
            "/api/faturas/{}",
            11
        )));
        // A constante declarada depois ainda não é conhecida: vira parâmetro, que cai.
        assert!(chamadas.contains(&(
            Some("Exemplo.Portal.ClienteFaturas.Parcelar"),
            "/{}/parcelamento",
            13
        )));
    }

    #[test]
    fn codigo_quebrado_nao_derruba() {
        for t in [
            "namespace",
            "class A : { void F( { }",
            "[",
            "[Route(",
            "var s = @\"sem fim",
            "var s = $\"{x\"",
            "var s = \"\"\"\nraw",
            "#if X\nclass A {}\n#endif",
            "enum E { A = , }",
        ] {
            let _ = extrair(t);
        }
        for (i, _) in CONTROLADOR.char_indices() {
            let _ = extrair(&CONTROLADOR[..i]);
        }
    }
}
