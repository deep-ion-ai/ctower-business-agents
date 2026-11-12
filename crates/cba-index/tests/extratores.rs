//! Extratores de Java, C# e TypeScript no mapa: ligações entre tela, endpoint,
//! entidade e tabela, com projetos sintéticos e com a fixture "Faturamento
//! Exemplo".

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use cba_index::{Mapa, Projeto, Registro, TipoReferencia, TipoSimbolo, inventariar, mapear};

fn escrever(raiz: &Path, caminho: &str, conteudo: &str) {
    let alvo = raiz.join(caminho);
    fs::create_dir_all(alvo.parent().unwrap()).unwrap();
    fs::write(alvo, conteudo).unwrap();
}

fn mapa_de(raiz: &Path) -> Mapa {
    let projeto = Projeto::novo("Sintetico", [raiz]).unwrap();
    let inv = inventariar(&projeto, None).unwrap();
    mapear(&inv, &projeto.escopo().unwrap(), None, &Registro::padrao())
}

const DDL: &str =
    "CREATE TABLE FATURA (\n  ID_FATURA NUMBER PRIMARY KEY,\n  STATUS VARCHAR2(10)\n);\n";

/// Tela → endpoint → serviço/entidade → tabela, num projeto Spring + TypeScript
/// e noutro ASP.NET + Angular.
#[test]
fn tela_endpoint_entidade_e_tabela_ligados() {
    let tmp = tempfile::tempdir().unwrap();
    let raiz = tmp.path().join("projeto");
    escrever(&raiz, "db/tabelas.sql", DDL);
    escrever(
        &raiz,
        "backend/src/main/java/ex/Fatura.java",
        r#"package ex;

import jakarta.persistence.*;

@Entity
@Table(name = "FATURA")
public class Fatura {
    @Id
    @Column(name = "ID_FATURA")
    private Long id;
}
"#,
    );
    escrever(
        &raiz,
        "backend/src/main/java/ex/FaturaController.java",
        r#"package ex;

import org.springframework.web.bind.annotation.*;

@RestController
@RequestMapping("/api/faturas")
public class FaturaController {
    private final FaturaRepository repositorio;

    @PostMapping("/{id}/parcelamento")
    public Fatura parcelar(@PathVariable long id) {
        return repositorio.buscar(id);
    }
}
"#,
    );
    escrever(
        &raiz,
        "backend/src/main/java/ex/FaturaRepository.java",
        "package ex;\n\npublic interface FaturaRepository {\n    Fatura buscar(long id);\n}\n",
    );
    escrever(
        &raiz,
        "frontend/src/telas/parcelamento.ts",
        r#"const API = '/api/faturas';

export async function enviarParcelamento(id: number): Promise<void> {
  await fetch(`${API}/${id}/parcelamento`, { method: 'POST' });
}
"#,
    );
    escrever(
        &raiz,
        "portal/Controllers/FaturasController.cs",
        r#"namespace Portal.Controllers;

[ApiController]
[Route("api/[controller]")]
public class FaturasController : ControllerBase
{
    [HttpPost("{id:long}/parcelamento")]
    public IActionResult Parcelar(long id) => Ok();
}
"#,
    );
    escrever(
        &raiz,
        "portal/app/parcelamento.component.ts",
        r#"@Component({ selector: 'app-parcelamento', templateUrl: './parcelamento.html' })
export class ParcelamentoComponent {
  constructor(private http: HttpClient) {}

  parcelar(id: number) {
    return this.http.post(`/api/faturas/${id}/parcelamento`, {});
  }
}
"#,
    );
    let mapa = mapa_de(&raiz);
    assert_eq!(mapa.falhas().count(), 0);

    // Os dois backends expõem a mesma rota.
    let endpoints = mapa.buscar("/api/faturas/{}/parcelamento");
    let mut onde: Vec<_> = endpoints
        .iter()
        .map(|a| (a.arquivo, a.simbolo.pai.as_deref().unwrap()))
        .collect();
    onde.sort();
    assert_eq!(
        onde,
        [
            (
                "backend/src/main/java/ex/FaturaController.java",
                "ex.FaturaController.parcelar"
            ),
            (
                "portal/Controllers/FaturasController.cs",
                "Portal.Controllers.FaturasController.Parcelar"
            ),
        ]
    );

    // As duas telas chamam o endpoint (as duas implementações, pela rota).
    let java = endpoints
        .iter()
        .find(|a| a.arquivo.ends_with(".java"))
        .unwrap();
    let mut telas: Vec<_> = mapa
        .referencias_a(java.id)
        .into_iter()
        .map(|u| {
            (
                u.arquivo,
                u.origem.map(|o| o.simbolo.qualificado.clone()).unwrap(),
                u.referencia.tipo,
            )
        })
        .collect();
    telas.sort();
    assert_eq!(
        telas,
        [
            (
                "frontend/src/telas/parcelamento.ts",
                "enviarParcelamento".to_string(),
                TipoReferencia::Chamada
            ),
            (
                "portal/app/parcelamento.component.ts",
                "ParcelamentoComponent.parcelar".to_string(),
                TipoReferencia::Chamada
            ),
        ]
    );
    let tela = mapa.buscar("ParcelamentoComponent");
    assert_eq!(tela[0].simbolo.tipo, TipoSimbolo::Tela);

    // O método do endpoint usa a entidade (tipo de retorno) e o repositório.
    let metodo = mapa.buscar("ex.FaturaController.parcelar");
    assert_eq!(metodo.len(), 1);
    let usa: Vec<_> = mapa
        .referencias_de(metodo[0].id)
        .into_iter()
        .filter(|u| !u.alvos.is_empty())
        .map(|u| {
            let alvo = mapa.simbolo(u.alvos[0]).unwrap();
            (u.referencia.tipo, alvo.simbolo.qualificado.clone())
        })
        .collect();
    assert!(usa.contains(&(TipoReferencia::Tipo, "ex.Fatura".to_string())));
    assert!(usa.contains(&(
        TipoReferencia::Chamada,
        "ex.FaturaRepository.buscar".to_string()
    )));

    // A entidade aponta para a tabela e a coluna do DDL.
    let entidade = mapa
        .buscar("ex.Fatura")
        .into_iter()
        .find(|a| a.simbolo.tipo == TipoSimbolo::Classe)
        .unwrap();
    let tabela = mapa
        .referencias_de(entidade.id)
        .into_iter()
        .find(|u| u.referencia.tipo == TipoReferencia::Uso)
        .unwrap();
    let alvo = mapa.simbolo(tabela.alvos[0]).unwrap();
    assert_eq!(
        (alvo.arquivo, alvo.simbolo.tipo),
        ("db/tabelas.sql", TipoSimbolo::Tabela)
    );
    let id = mapa.buscar("ex.Fatura.id");
    let coluna = &mapa.referencias_de(id[0].id)[0];
    assert_eq!(
        mapa.simbolo(coluna.alvos[0]).unwrap().simbolo.qualificado,
        "FATURA.ID_FATURA"
    );
}

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/faturamento-exemplo")
}

/// Na fixture: chamadas entre as classes Java resolvidas pelo tipo do campo, e a
/// tela TypeScript chamando a rota de parcelamento (o backend da fixture não tem
/// controlador, então a referência fica sem alvo).
#[test]
fn fixture_java_e_tela() {
    let mapa = mapa_de(&fixture());
    assert_eq!(mapa.falhas().count(), 0);
    let parcelar = mapa.buscar("ParcelamentoService.parcelar");
    assert_eq!(parcelar.len(), 1);
    assert_eq!(
        parcelar[0].simbolo.qualificado,
        "exemplo.faturamento.ParcelamentoService.parcelar"
    );
    let chama: Vec<String> = mapa
        .referencias_de(parcelar[0].id)
        .into_iter()
        .filter(|u| u.referencia.tipo == TipoReferencia::Chamada && !u.alvos.is_empty())
        .map(|u| {
            mapa.simbolo(u.alvos[0])
                .unwrap()
                .simbolo
                .qualificado
                .clone()
        })
        .collect();
    for alvo in [
        "exemplo.faturamento.EncargosService.calcular",
        "exemplo.faturamento.ParametroRepository.inteiro",
        "exemplo.faturamento.ParametroRepository.valor",
        "exemplo.faturamento.Fatura.isAberta",
    ] {
        assert!(chama.iter().any(|c| c == alvo), "{alvo} fora de {chama:?}");
    }

    let tela = "frontend/src/telas/parcelamento-form.ts";
    let simbolos = &mapa.arquivo(tela).unwrap().extracao.simbolos;
    let enviar = simbolos
        .iter()
        .find(|s| s.nome == "enviarParcelamento")
        .unwrap();
    assert_eq!(
        (enviar.tipo, enviar.linhas.inicio),
        (TipoSimbolo::Funcao, 63)
    );
    let usos = mapa.usos("/api/faturas/{}/parcelamento");
    assert_eq!(usos.len(), 1);
    assert_eq!((usos[0].arquivo, usos[0].referencia.linha), (tela, 69));
}

/// Critério de aceite: desempenho num projeto Java grande. Aponte
/// `CBA_PROJETO_JAVA` para o checkout de um projeto de código aberto (o
/// Spring Framework, por exemplo) e rode
/// `cargo test -p cba-index --release --test extratores -- --ignored --nocapture`.
/// Sem a variável, gera um projeto sintético de 2 mil classes.
#[test]
#[ignore]
fn projeto_java_grande() {
    let tmp = tempfile::tempdir().unwrap();
    let raiz = match std::env::var_os("CBA_PROJETO_JAVA") {
        Some(p) => PathBuf::from(p),
        None => {
            let raiz = tmp.path().join("grande");
            for i in 0..2_000 {
                let mut s = format!(
                    "package ex.m{};\n\nimport java.util.List;\n\n/** Classe {i}. */\npublic class C{i} extends Base implements I{i} {{\n    private final C{} vizinho;\n",
                    i / 100,
                    (i + 1) % 2_000
                );
                for m in 0..40 {
                    s.push_str(&format!(
                        "    /** Método {m}. */\n    public List<String> m{m}(int a, String b) {{\n        String sql = \"select * from TABELA_{m} where x = ?\";\n        if (a > {m}) {{ return vizinho.m{m}(a - 1, b); }}\n        return new java.util.ArrayList<>();\n    }}\n\n"
                    ));
                }
                s.push_str("}\n");
                escrever(&raiz, &format!("src/ex/m{}/C{i}.java", i / 100), &s);
            }
            raiz
        }
    };
    let projeto = Projeto::novo("Grande", [&raiz]).unwrap();
    let inv = inventariar(&projeto, None).unwrap();
    let java = inv
        .arquivos
        .iter()
        .filter(|a| a.caminho.ends_with(".java"))
        .count();
    let t = Instant::now();
    let mapa = mapear(&inv, &projeto.escopo().unwrap(), None, &Registro::padrao());
    let tempo = t.elapsed();
    let simbolos = mapa.simbolos().count();
    eprintln!(
        "{} arquivos ({java} Java), {simbolos} símbolos, {} falhas: {tempo:?}",
        mapa.arquivos().len(),
        mapa.falhas().count()
    );
    assert_eq!(mapa.falhas().count(), 0);
    assert!(tempo < Duration::from_secs(120), "{tempo:?}");
}
