//! Extrator de Java.
//!
//! Usa o léxico e a análise estrutural comuns às linguagens de chaves
//! ([`chaves`]: comentários, strings e text blocks, blocos por chaves). Reconhece:
//!
//! - `package` (namespace que qualifica os tipos do arquivo), `import`;
//! - classes, interfaces, `@interface`, enums (com as constantes) e records (com
//!   os componentes), aninhados ou não;
//! - métodos, construtores, campos e constantes (`static final`), com a
//!   assinatura no detalhe e o Javadoc como comentário;
//! - entidades JPA (`@Entity`, `@Table`, `@Column`, `@JoinColumn`): a classe
//!   referencia a tabela e o campo, a coluna;
//! - endpoints do Spring (`@RequestMapping`, `@GetMapping`...) e do JAX-RS
//!   (`@Path`, `@GET`...): um símbolo filho do método, com a rota normalizada
//!   como nome (`/api/faturas/{}/parcelamento`), que é o alvo das chamadas HTTP
//!   das telas;
//! - referências: herança, implementação, tipos de campos e parâmetros, `new`,
//!   chamadas (pelo tipo declarado do receptor quando ele é conhecido:
//!   `encargosService.calcular` vira `EncargosService.calcular`), constantes de
//!   outras classes (`TipoCliente.PUB`), `Classe::metodo`, chamadas do
//!   `RestTemplate` e menções ao SQL embutido nas strings.
//!
//! O que vem de bibliotecas (JDK, Jakarta, Spring) não vira referência, só a
//! importação.

// O módulo comum fica aqui para que `mod.rs` só mude nas linhas reservadas.
#[path = "chaves.rs"]
pub(crate) mod chaves;

use chaves::{Anotacao, Dialeto, Http, InfoClasse, Ling, juntar_rota};

use super::{Extracao, Extrator, Fonte};
use crate::linguagem::Linguagem;

/// Extrator de Java.
#[derive(Debug, Clone, Copy, Default)]
pub struct ExtratorJava;

impl Extrator for ExtratorJava {
    fn nome(&self) -> &'static str {
        "java"
    }

    fn versao(&self) -> u32 {
        1
    }

    fn linguagens(&self) -> &'static [Linguagem] {
        &[Linguagem::Java]
    }

    fn extrair(&self, fonte: &Fonte<'_>) -> Result<Extracao, String> {
        Ok(chaves::analisar(fonte, &DIALETO))
    }
}

const DIALETO: Dialeto = Dialeto {
    ling: Ling::Java,
    classe,
    rotas,
    coluna,
    http,
    externo,
};

fn anotacao<'a>(anots: &'a [Anotacao], nome: &str) -> Option<&'a Anotacao> {
    anots.iter().find(|a| a.nome == nome)
}

/// `ParcelaFatura` → `PARCELA_FATURA` (nome físico padrão do Spring Boot).
pub(crate) fn snake_maiusculo(nome: &str) -> String {
    let c: Vec<char> = nome.chars().collect();
    let mut s = String::new();
    for (i, &ch) in c.iter().enumerate() {
        if i > 0
            && ch.is_uppercase()
            && c[i - 1].is_lowercase()
            && c.get(i + 1).is_some_and(|n| n.is_lowercase())
        {
            s.push('_');
        }
        s.extend(ch.to_uppercase());
    }
    s
}

fn classe(anots: &[Anotacao], nome: &str, _bases: &[String]) -> InfoClasse {
    let mut info = InfoClasse::default();
    if anotacao(anots, "Entity").is_some() {
        let tabela = anotacao(anots, "Table");
        let mut nome_tabela = tabela
            .and_then(|t| t.textos(&["name"]).into_iter().next())
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| snake_maiusculo(nome));
        if let Some(esquema) = tabela
            .and_then(|t| t.par("schema"))
            .filter(|e| !e.is_empty())
        {
            nome_tabela = format!("{esquema}.{nome_tabela}");
        }
        info.detalhe = Some(format!("entidade JPA (tabela {nome_tabela})"));
        info.tabela = Some(nome_tabela);
    }
    let prefixos: Vec<String> = ["RequestMapping", "Path"]
        .iter()
        .filter_map(|n| anotacao(anots, n))
        .flat_map(|a| a.textos(&["value", "path"]))
        .collect();
    let controlador =
        anotacao(anots, "RestController").is_some() || anotacao(anots, "Controller").is_some();
    if controlador || !prefixos.is_empty() {
        info.rotas = Some(if prefixos.is_empty() {
            vec![String::new()]
        } else {
            prefixos
        });
    }
    info
}

const VERBOS: &[&str] = &["GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS"];

fn rotas(anots: &[Anotacao], _metodo: &str, classe: &InfoClasse) -> Vec<(String, String)> {
    let prefixos = classe.rotas.clone().unwrap_or_else(|| vec![String::new()]);
    let mut v = Vec::new();
    let mut combinar = |verbo: &str, caminhos: Vec<String>| {
        let caminhos = if caminhos.is_empty() {
            vec![String::new()]
        } else {
            caminhos
        };
        for p in &prefixos {
            for c in &caminhos {
                v.push((verbo.to_string(), juntar_rota(p, c)));
            }
        }
    };
    // Spring.
    for a in anots {
        let verbo = match a.nome.as_str() {
            "GetMapping" => "GET",
            "PostMapping" => "POST",
            "PutMapping" => "PUT",
            "DeleteMapping" => "DELETE",
            "PatchMapping" => "PATCH",
            "RequestMapping" => a
                .args
                .iter()
                .filter(|x| x.nome.as_deref() == Some("method"))
                .find_map(|x| VERBOS.iter().find(|v| x.bruto.contains(&format!(".{v}"))))
                .copied()
                .unwrap_or(""),
            _ => continue,
        };
        combinar(verbo, a.textos(&["value", "path"]));
    }
    // JAX-RS.
    if let Some(a) = anots.iter().find(|a| VERBOS.contains(&a.nome.as_str())) {
        let caminhos = anotacao(anots, "Path")
            .map(|p| p.textos(&["value"]))
            .unwrap_or_default();
        combinar(&a.nome, caminhos);
    }
    v
}

fn coluna(anots: &[Anotacao]) -> Option<String> {
    ["Column", "JoinColumn"]
        .iter()
        .filter_map(|n| anotacao(anots, n))
        .find_map(|a| a.textos(&["name"]).into_iter().next())
        .filter(|c| !c.is_empty())
}

fn http(receptor: Option<&str>, tipo: Option<&str>, metodo: &str) -> Option<Http> {
    let receptor = receptor?;
    let rest = tipo.is_some_and(|t| t.ends_with("RestTemplate") || t.ends_with("RestOperations"))
        || receptor.to_ascii_lowercase().contains("rest");
    match metodo {
        "getForObject" | "getForEntity" | "postForObject" | "postForEntity" | "postForLocation"
        | "patchForObject" => Some(Http::Chamada),
        "put" | "delete" | "exchange" if rest => Some(Http::Chamada),
        _ => None,
    }
}

/// Pacotes de bibliotecas e da plataforma.
fn externo(pacote: &str) -> bool {
    const PREFIXOS: &[&str] = &[
        "java.",
        "javax.",
        "jakarta.",
        "org.springframework.",
        "org.slf4j.",
        "org.apache.",
        "org.hibernate.",
        "org.junit.",
        "org.mockito.",
        "org.assertj.",
        "com.fasterxml.",
        "com.google.",
        "io.swagger.",
        "lombok.",
    ];
    PREFIXOS.iter().any(|p| pacote.starts_with(p))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extrator::{Linhas, Referencia, Simbolo, TipoReferencia, TipoSimbolo};

    fn extrair(texto: &str) -> Extracao {
        ExtratorJava
            .extrair(&Fonte {
                caminho: "src/X.java",
                linguagem: Linguagem::Java,
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

    const SERVICO: &str = r#"package exemplo.faturamento;

import java.math.BigDecimal;
import java.sql.Connection;
import javax.sql.DataSource;

/**
 * Calcula os encargos.
 *
 * @author ninguém
 */
public class EncargosService extends BaseService implements Calculadora, Auditavel {

    private static final int MAX = 3;
    private final DataSource dataSource;
    private final ParametroRepository parametros;

    public EncargosService(DataSource dataSource, ParametroRepository parametros) {
        this.dataSource = dataSource;
        this.parametros = parametros;
    }

    // Multa por atraso.
    @Override
    public BigDecimal calcular(Fatura fatura, int dias) throws RegraException {
        BigDecimal multa = chamar("PKG_COBRANCA.FN_CALCULAR_MULTA", fatura.id());
        if (fatura.tipoCliente() == TipoCliente.PUB) {
            multa = BigDecimal.ZERO;
        }
        int max = parametros.inteiro(ParametroRepository.MAX_PARCELAS);
        String sql = """
            select valor
              from parametro
             where chave = ?
            """;
        Connection con = dataSource.getConnection();
        return TetoRegra.limitar(multa, new Limite(max), Fatura::valor);
    }

    private BigDecimal chamar(String funcao, long id) {
        return null; // "PKG_X" em comentário não conta
    }

    public record Encargos(BigDecimal multa, TipoCliente tipo) {
    }

    enum Situacao {
        /** Em aberto. */
        ABERTA,
        PAGA("p") {
            void f() {}
        };

        Situacao() {}
        Situacao(String s) {}
    }

    public interface Calculo {
        BigDecimal calcular(Fatura f);
        default int zero() { return 0; }
    }
}
"#;

    #[test]
    fn classes_metodos_e_campos() {
        let e = extrair(SERVICO);
        let ns = simbolo(&e, "exemplo.faturamento");
        assert_eq!(ns.tipo, TipoSimbolo::Namespace);
        assert_eq!(ns.nome, "faturamento");
        let c = simbolo(&e, "exemplo.faturamento.EncargosService");
        assert_eq!(c.tipo, TipoSimbolo::Classe);
        assert_eq!(c.linhas, Linhas::new(12, 62));
        assert_eq!(c.comentario.as_deref(), Some("Calcula os encargos."));
        assert_eq!(c.pai.as_deref(), Some("exemplo.faturamento"));

        let q = |s: &str| format!("exemplo.faturamento.EncargosService.{s}");
        assert_eq!(simbolo(&e, &q("MAX")).tipo, TipoSimbolo::Constante);
        assert_eq!(simbolo(&e, &q("dataSource")).tipo, TipoSimbolo::Campo);
        let ctor = e
            .simbolos
            .iter()
            .filter(|s| s.qualificado == q("EncargosService"))
            .collect::<Vec<_>>();
        assert_eq!(ctor.len(), 1);
        assert_eq!(ctor[0].tipo, TipoSimbolo::Construtor);
        assert_eq!(ctor[0].linhas, Linhas::new(18, 21));

        let m = simbolo(&e, &q("calcular"));
        assert_eq!(m.tipo, TipoSimbolo::Metodo);
        assert_eq!(m.linhas, Linhas::new(24, 38));
        assert_eq!(m.comentario.as_deref(), Some("Multa por atraso."));
        assert_eq!(
            m.detalhe.as_deref(),
            Some("BigDecimal calcular(Fatura fatura, int dias) throws RegraException")
        );
        assert_eq!(simbolo(&e, &q("chamar")).linhas, Linhas::new(40, 42));

        let r = simbolo(&e, &q("Encargos"));
        assert_eq!(r.tipo, TipoSimbolo::Registro);
        assert_eq!(simbolo(&e, &q("Encargos.multa")).tipo, TipoSimbolo::Campo);
        assert_eq!(
            simbolo(&e, &q("Encargos.tipo")).detalhe.as_deref(),
            Some("TipoCliente")
        );

        let en = simbolo(&e, &q("Situacao"));
        assert_eq!(en.tipo, TipoSimbolo::Enum);
        assert_eq!(en.linhas, Linhas::new(47, 56));
        let aberta = simbolo(&e, &q("Situacao.ABERTA"));
        assert_eq!(aberta.tipo, TipoSimbolo::Constante);
        assert_eq!(aberta.comentario.as_deref(), Some("Em aberto."));
        assert_eq!(simbolo(&e, &q("Situacao.PAGA")).linhas, Linhas::new(50, 52));
        let ctors: Vec<_> = e
            .simbolos
            .iter()
            .filter(|s| s.qualificado == q("Situacao.Situacao"))
            .collect();
        assert_eq!(ctors.len(), 2);

        let i = simbolo(&e, &q("Calculo"));
        assert_eq!(i.tipo, TipoSimbolo::Interface);
        assert!(simbolo(&e, &q("Calculo.calcular")).declaracao);
        assert!(!simbolo(&e, &q("Calculo.zero")).declaracao);
        // O corpo da constante é uma classe anônima.
        assert_eq!(simbolo(&e, &q("Situacao.PAGA.f")).tipo, TipoSimbolo::Metodo);
    }

    #[test]
    fn referencias() {
        let e = extrair(SERVICO);
        let classe = Some("exemplo.faturamento.EncargosService");
        let calc = Some("exemplo.faturamento.EncargosService.calcular");
        assert_eq!(
            refs(&e, TipoReferencia::Importacao),
            [
                (None, "java.math.BigDecimal", 3),
                (None, "java.sql.Connection", 4),
                (None, "javax.sql.DataSource", 5)
            ]
        );
        assert_eq!(
            refs(&e, TipoReferencia::Heranca),
            [(classe, "BaseService", 12)]
        );
        assert_eq!(
            refs(&e, TipoReferencia::Implementacao),
            [(classe, "Calculadora", 12), (classe, "Auditavel", 12)]
        );
        let chamadas = refs(&e, TipoReferencia::Chamada);
        assert_eq!(
            chamadas,
            [
                (calc, "chamar", 26),
                (calc, "Fatura.id", 26),
                (calc, "Fatura.tipoCliente", 27),
                (calc, "ParametroRepository.inteiro", 30),
                (calc, "TetoRegra.limitar", 37),
                (calc, "Fatura.valor", 37),
            ]
        );
        assert_eq!(
            refs(&e, TipoReferencia::Uso),
            [
                (calc, "TipoCliente.PUB", 27),
                (calc, "ParametroRepository.MAX_PARCELAS", 30)
            ]
        );
        assert_eq!(
            refs(&e, TipoReferencia::Instanciacao),
            [(calc, "Limite", 37)]
        );
        let tipos = refs(&e, TipoReferencia::Tipo);
        assert!(tipos.contains(&(calc, "Fatura", 25)));
        assert!(tipos.contains(&(
            Some("exemplo.faturamento.EncargosService.parametros"),
            "ParametroRepository",
            16
        )));
        // Nada da biblioteca: BigDecimal, DataSource, Connection.
        assert!(
            !e.referencias
                .iter()
                .any(|r| r.tipo != TipoReferencia::Importacao
                    && (r.alvo.starts_with("BigDecimal")
                        || r.alvo.starts_with("DataSource")
                        || r.alvo.starts_with("Connection")))
        );

        // SQL embutido: na string e no text block, não no comentário.
        let mencoes = refs(&e, TipoReferencia::Mencao);
        assert!(mencoes.contains(&(calc, "PKG_COBRANCA.FN_CALCULAR_MULTA", 26)));
        assert!(mencoes.contains(&(calc, "parametro", 33)));
        assert!(mencoes.contains(&(calc, "valor", 32)));
        assert!(!mencoes.iter().any(|m| m.1 == "PKG_X"));
    }

    #[test]
    fn entidade_e_endpoints_spring() {
        let e = extrair(
            r#"package x;

import jakarta.persistence.*;
import org.springframework.web.bind.annotation.*;

@Entity
@Table(name = "FATURA", schema = "APP")
public class Fatura {
    @Id
    @Column(name = "ID_FATURA")
    private Long id;

    @Column(name = "VL_ORIGINAL", nullable = false)
    private BigDecimal valorOriginal;

    @ManyToOne
    @JoinColumn(name = "ID_CLIENTE")
    private Cliente cliente;

    private String status;
}

@Entity
class ParcelaFatura {
}

@RestController
@RequestMapping("/api/faturas")
class FaturaController {
    private final ParcelamentoService servico;

    @GetMapping("/{id}")
    public Fatura buscar(@PathVariable long id) {
        return null;
    }

    @PostMapping(value = {"/{id}/parcelamento", "/{id}/parcelar"})
    public List<Parcela> parcelar(@PathVariable("id") long id, @RequestBody Pedido pedido) {
        return servico.parcelar(id, pedido.quantidade());
    }

    @RequestMapping(path = "/busca", method = RequestMethod.POST)
    Object busca() { return null; }

    @DeleteMapping
    void remover() {}
}

@Path("/clientes")
class ClienteRecurso {
    @GET
    @Path("{id}")
    public Cliente um(@PathParam("id") long id) { return null; }

    @POST
    public void novo(Cliente c) {}

    public void auxiliar() {}
}
"#,
        );
        let f = simbolo(&e, "x.Fatura");
        assert_eq!(
            f.detalhe.as_deref(),
            Some("entidade JPA (tabela APP.FATURA)")
        );
        let usos = refs(&e, TipoReferencia::Uso);
        assert_eq!(
            usos,
            [
                (Some("x.Fatura"), "APP.FATURA", 8),
                (Some("x.Fatura.id"), "APP.FATURA.ID_FATURA", 11),
                (Some("x.Fatura.valorOriginal"), "APP.FATURA.VL_ORIGINAL", 14),
                (Some("x.Fatura.cliente"), "APP.FATURA.ID_CLIENTE", 18),
                (Some("x.ParcelaFatura"), "PARCELA_FATURA", 24),
            ]
        );
        // Strings de anotação não viram menção.
        assert!(refs(&e, TipoReferencia::Mencao).is_empty());

        let endpoints: Vec<_> = e
            .simbolos
            .iter()
            .filter(|s| s.tipo == TipoSimbolo::Outro)
            .map(|s| {
                (
                    s.qualificado.as_str(),
                    s.detalhe.as_deref().unwrap(),
                    s.linhas,
                )
            })
            .collect();
        assert_eq!(
            endpoints,
            [
                (
                    "x.FaturaController.buscar./api/faturas/{}",
                    "endpoint GET /api/faturas/{id}",
                    Linhas::new(32, 35)
                ),
                (
                    "x.FaturaController.parcelar./api/faturas/{}/parcelamento",
                    "endpoint POST /api/faturas/{id}/parcelamento",
                    Linhas::new(37, 40)
                ),
                (
                    "x.FaturaController.parcelar./api/faturas/{}/parcelar",
                    "endpoint POST /api/faturas/{id}/parcelar",
                    Linhas::new(37, 40)
                ),
                (
                    "x.FaturaController.busca./api/faturas/busca",
                    "endpoint POST /api/faturas/busca",
                    Linhas::new(42, 43)
                ),
                (
                    "x.FaturaController.remover./api/faturas",
                    "endpoint DELETE /api/faturas",
                    Linhas::new(45, 46)
                ),
                (
                    "x.ClienteRecurso.um./clientes/{}",
                    "endpoint GET /clientes/{id}",
                    Linhas::new(51, 53)
                ),
                (
                    "x.ClienteRecurso.novo./clientes",
                    "endpoint POST /clientes",
                    Linhas::new(55, 56)
                ),
            ]
        );
        assert!(e.referencias.contains(&Referencia {
            origem: Some("x.FaturaController.parcelar".into()),
            alvo: "ParcelamentoService.parcelar".into(),
            tipo: TipoReferencia::Chamada,
            linha: 39,
        }));
    }

    #[test]
    fn rest_template_chama_endpoint() {
        let e = extrair(
            "class C {\n  private RestTemplate rest;\n  void f(long id) {\n    rest.getForObject(\"http://faturas/api/faturas/\" + id, Fatura.class);\n    rest.put(BASE + \"/x\", null);\n  }\n}\n",
        );
        let chamadas = refs(&e, TipoReferencia::Chamada);
        assert!(chamadas.contains(&(Some("C.f"), "/api/faturas/{}", 4)));
        assert!(chamadas.contains(&(Some("C.f"), "/x", 5)));
    }

    #[test]
    fn nomes() {
        assert_eq!(snake_maiusculo("ParcelaFatura"), "PARCELA_FATURA");
        assert_eq!(snake_maiusculo("Fatura"), "FATURA");
        assert_eq!(snake_maiusculo("NFeItem"), "NFE_ITEM");
    }

    #[test]
    fn codigo_quebrado_nao_derruba() {
        let textos = [
            "class {",
            "class A { void f( { } ",
            "}}}} class B {",
            "@",
            "package",
            "import static",
            "enum E { A(, B",
            "record R(",
            "class A { int x = \"\"\"",
            "class A { String s = \"sem fim\n; }",
        ];
        for t in textos {
            let _ = extrair(t);
        }
        // Todos os prefixos de um arquivo real.
        for (i, _) in SERVICO.char_indices() {
            let _ = extrair(&SERVICO[..i]);
        }
    }
}
