//! Extrator de TypeScript (e JavaScript, que tem a mesma sintaxe de interesse).
//!
//! Usa o léxico e a análise estrutural comuns às linguagens de chaves
//! ([`super::java::chaves`]: comentários, strings, template literals, regex,
//! blocos por chaves e fim de instrução sem `;`). Reconhece:
//!
//! - `import` (os nomes importados de módulos do próprio sistema; de bibliotecas,
//!   só o módulo);
//! - classes, interfaces, enums, `type`, `namespace`/`module`;
//! - funções (`function` e `const f = () => ...`), métodos, construtores,
//!   propriedades e campos, constantes e variáveis do módulo, com o JSDoc como
//!   comentário;
//! - telas: componentes do Angular (`@Component`, com o seletor no detalhe), do
//!   React (funções com nome em PascalCase em `.tsx`/`.jsx` e classes que estendem
//!   `Component`); serviços do Angular (`@Injectable`);
//! - chamadas HTTP: `fetch`, `axios`, `HttpClient` do Angular (`this.http.get`),
//!   `$.ajax` e clientes com cara de cliente HTTP (`api.post`). A URL é montada
//!   das strings, dos templates e das constantes de string do arquivo, e a
//!   referência vai para a rota normalizada (`/api/faturas/{}/parcelamento`), o
//!   mesmo nome do endpoint do backend;
//! - endpoints do NestJS (`@Controller`, `@Get`...) e do Express
//!   (`app.get('/x', ...)`, `router.post`), e entidades do TypeORM (`@Entity`);
//! - referências: herança, implementação, tipos, `new`, chamadas (pelo tipo
//!   declarado do receptor) e menções ao SQL embutido nas strings.

use super::java::chaves::{self, Anotacao, Dialeto, Http, InfoClasse, Ling, juntar_rota};
use super::java::snake_maiusculo;
use super::{Extracao, Extrator, Fonte};
use crate::linguagem::Linguagem;

/// Extrator de TypeScript e JavaScript.
#[derive(Debug, Clone, Copy, Default)]
pub struct ExtratorTypeScript;

impl Extrator for ExtratorTypeScript {
    fn nome(&self) -> &'static str {
        "typescript"
    }

    fn versao(&self) -> u32 {
        1
    }

    fn linguagens(&self) -> &'static [Linguagem] {
        &[Linguagem::TypeScript, Linguagem::JavaScript]
    }

    fn extrair(&self, fonte: &Fonte<'_>) -> Result<Extracao, String> {
        Ok(chaves::analisar(fonte, &DIALETO))
    }
}

const DIALETO: Dialeto = Dialeto {
    ling: Ling::TypeScript,
    classe,
    rotas,
    coluna,
    http,
    externo,
};

fn decorator<'a>(anots: &'a [Anotacao], nome: &str) -> Option<&'a Anotacao> {
    anots.iter().find(|a| a.nome == nome)
}

fn classe(anots: &[Anotacao], nome: &str, bases: &[String]) -> InfoClasse {
    let mut info = InfoClasse::default();
    if let Some(c) = decorator(anots, "Component") {
        info.tipo = Some(super::TipoSimbolo::Tela);
        info.detalhe = Some(match c.par("selector") {
            Some(s) => format!("componente Angular <{s}>"),
            None => "componente Angular".into(),
        });
    } else if decorator(anots, "Injectable").is_some() {
        info.detalhe = Some("serviço Angular".into());
    } else if bases.iter().any(|b| {
        matches!(
            b.as_str(),
            "Component" | "PureComponent" | "React.Component" | "React.PureComponent"
        )
    }) {
        info.tipo = Some(super::TipoSimbolo::Tela);
        info.detalhe = Some("componente React".into());
    }
    if let Some(e) = decorator(anots, "Entity") {
        let tabela = e
            .par("name")
            .map(str::to_string)
            .or_else(|| e.textos(&[]).into_iter().next())
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| snake_maiusculo(nome));
        info.detalhe = Some(format!("entidade TypeORM (tabela {tabela})"));
        info.tabela = Some(tabela);
    }
    if let Some(c) = decorator(anots, "Controller") {
        let mut prefixos = c.textos(&[]);
        if let Some(p) = c.par("path") {
            prefixos.push(p.to_string());
        }
        info.rotas = Some(if prefixos.is_empty() {
            vec![String::new()]
        } else {
            prefixos
        });
    }
    info
}

fn rotas(anots: &[Anotacao], _metodo: &str, classe: &InfoClasse) -> Vec<(String, String)> {
    let Some(prefixos) = &classe.rotas else {
        return Vec::new();
    };
    let mut v = Vec::new();
    for a in anots {
        let verbo = match a.nome.as_str() {
            "Get" | "Post" | "Put" | "Delete" | "Patch" | "Head" | "Options" | "All" => {
                a.nome.to_ascii_uppercase()
            }
            _ => continue,
        };
        let mut caminhos = a.textos(&[]);
        if caminhos.is_empty() {
            caminhos.push(String::new());
        }
        for p in prefixos {
            for c in &caminhos {
                v.push((verbo.clone(), juntar_rota(p, c)));
            }
        }
    }
    v
}

fn coluna(anots: &[Anotacao]) -> Option<String> {
    [
        "Column",
        "JoinColumn",
        "PrimaryColumn",
        "PrimaryGeneratedColumn",
    ]
    .iter()
    .filter_map(|n| decorator(anots, n))
    .find_map(|a| a.par("name").map(str::to_string))
}

const VERBOS: &[&str] = &["get", "post", "put", "delete", "patch", "head", "options"];

fn http(receptor: Option<&str>, tipo: Option<&str>, metodo: &str) -> Option<Http> {
    let Some(r) = receptor else {
        return matches!(metodo, "fetch" | "axios" | "$fetch" | "ky").then_some(Http::Chamada);
    };
    let r = r.to_ascii_lowercase();
    if VERBOS.contains(&metodo) || metodo == "request" || metodo == "ajax" || metodo == "getJSON" {
        if matches!(r.as_str(), "app" | "router" | "server" | "rotas" | "routes")
            && VERBOS.contains(&metodo)
        {
            return Some(Http::Rota(metodo.to_ascii_uppercase()));
        }
        let cliente = tipo
            .is_some_and(|t| matches!(t, "HttpClient" | "AxiosInstance" | "AxiosStatic" | "Axios"))
            || r == "$"
            || r.contains("http")
            || r.contains("axios")
            || r == "api"
            || r.ends_with("api")
            || r.ends_with("client")
            || r.ends_with("cliente");
        return cliente.then_some(Http::Chamada);
    }
    None
}

/// Módulo de biblioteca: o que não é caminho relativo nem apelido do projeto.
fn externo(modulo: &str) -> bool {
    !(modulo.starts_with('.')
        || modulo.starts_with('/')
        || modulo.starts_with("@/")
        || modulo.starts_with("~/")
        || modulo.starts_with("src/")
        || modulo.starts_with("app/"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extrator::{Linhas, Simbolo, TipoReferencia, TipoSimbolo};

    fn extrair_em(caminho: &str, texto: &str) -> Extracao {
        ExtratorTypeScript
            .extrair(&Fonte {
                caminho,
                linguagem: Linguagem::TypeScript,
                texto,
            })
            .unwrap()
    }

    fn extrair(texto: &str) -> Extracao {
        extrair_em("src/x.ts", texto)
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

    const SERVICO: &str = r#"import { Injectable } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { Fatura, Parcela as P } from './modelos';
import * as util from '../util';

const API = '/api/faturas';

/** Acesso às faturas. */
@Injectable({ providedIn: 'root' })
export class FaturaService extends BaseService implements Servico {
  private cache = new Map<number, Fatura>();
  readonly LIMITE = 10
  total: number

  constructor(private http: HttpClient, private log: Logger) {
    super();
  }

  buscar(id: number): Observable<Fatura> {
    return this.http.get<Fatura>(`${API}/${id}`);
  }

  async parcelar(id: number, quantidade: number): Promise<P[]> {
    const url = API + '/' + id + '/parcelamento';
    this.log.registrar('parcelando');
    return this.http.post<P[]>(`${API}/${id}/parcelamento`, { quantidade }).toPromise();
  }

  salvar = async (f: Fatura) => {
    await fetch('/api/faturas', { method: 'POST', body: JSON.stringify(f) });
    util.formatar(f);
  };

  get vazio(): boolean {
    return this.total === 0;
  }
}

export enum Situacao {
  Aberta = 'ABERTA',
  Paga = 'PAGA',
}

export default function somar(a: number, b: number) {
  return a + b
}

export const dobrar = (x: number): number => x * 2;
let contador = 0
"#;

    #[test]
    fn simbolos() {
        let e = extrair(SERVICO);
        let c = simbolo(&e, "FaturaService");
        assert_eq!(c.tipo, TipoSimbolo::Classe);
        assert_eq!(c.linhas, Linhas::new(9, 37));
        assert_eq!(c.comentario.as_deref(), Some("Acesso às faturas."));
        assert_eq!(c.detalhe.as_deref(), Some("serviço Angular"));
        assert_eq!(simbolo(&e, "API").tipo, TipoSimbolo::Constante);
        let q = |s: &str| format!("FaturaService.{s}");
        assert_eq!(simbolo(&e, &q("cache")).tipo, TipoSimbolo::Campo);
        assert_eq!(simbolo(&e, &q("LIMITE")).tipo, TipoSimbolo::Constante);
        assert_eq!(simbolo(&e, &q("total")).linhas, Linhas::new(13, 13));
        assert_eq!(simbolo(&e, &q("constructor")).tipo, TipoSimbolo::Construtor);
        let b = simbolo(&e, &q("buscar"));
        assert_eq!(b.tipo, TipoSimbolo::Metodo);
        assert_eq!(b.linhas, Linhas::new(19, 21));
        assert_eq!(simbolo(&e, &q("parcelar")).linhas, Linhas::new(23, 27));
        let s = simbolo(&e, &q("salvar"));
        assert_eq!(s.tipo, TipoSimbolo::Metodo);
        assert_eq!(s.linhas, Linhas::new(29, 32));
        assert_eq!(simbolo(&e, &q("vazio")).tipo, TipoSimbolo::Propriedade);
        assert_eq!(simbolo(&e, "Situacao").tipo, TipoSimbolo::Enum);
        assert_eq!(simbolo(&e, "Situacao.Paga").tipo, TipoSimbolo::Constante);
        let f = simbolo(&e, "somar");
        assert_eq!(f.tipo, TipoSimbolo::Funcao);
        assert_eq!(f.linhas, Linhas::new(44, 46));
        assert_eq!(simbolo(&e, "dobrar").tipo, TipoSimbolo::Funcao);
        assert_eq!(simbolo(&e, "contador").tipo, TipoSimbolo::Variavel);
    }

    #[test]
    fn referencias_e_http() {
        let e = extrair(SERVICO);
        assert_eq!(
            refs(&e, TipoReferencia::Importacao),
            [
                (None, "@angular/core", 1),
                (None, "@angular/common/http", 2),
                (None, "Fatura", 3),
                (None, "Parcela", 3),
                (None, "../util", 4),
            ]
        );
        let classe = Some("FaturaService");
        assert_eq!(
            refs(&e, TipoReferencia::Heranca),
            [(classe, "BaseService", 10)]
        );
        assert_eq!(
            refs(&e, TipoReferencia::Implementacao),
            [(classe, "Servico", 10)]
        );
        let chamadas = refs(&e, TipoReferencia::Chamada);
        assert_eq!(
            chamadas,
            [
                (Some("FaturaService.buscar"), "/api/faturas/{}", 20),
                (Some("FaturaService.parcelar"), "Logger.registrar", 25),
                (
                    Some("FaturaService.parcelar"),
                    "/api/faturas/{}/parcelamento",
                    26
                ),
                (Some("FaturaService.salvar"), "/api/faturas", 30),
            ]
        );
        // O que vem de bibliotecas não vira referência (Injectable, HttpClient).
        assert!(
            !e.referencias
                .iter()
                .any(|r| r.tipo != TipoReferencia::Importacao && r.alvo.contains("HttpClient"))
        );
        // Strings de import e de decorator não viram menção.
        assert!(
            !refs(&e, TipoReferencia::Mencao)
                .iter()
                .any(|m| m.1 == "modelos" || m.1 == "root")
        );
    }

    #[test]
    fn telas_angular_e_react() {
        let e = extrair(
            "@Component({\n  selector: 'app-parcelamento',\n  templateUrl: './parcelamento.html',\n})\nexport class ParcelamentoComponent implements OnInit {\n  ngOnInit(): void {}\n}\n",
        );
        let t = simbolo(&e, "ParcelamentoComponent");
        assert_eq!(t.tipo, TipoSimbolo::Tela);
        assert_eq!(t.linhas, Linhas::new(1, 7));
        assert_eq!(
            t.detalhe.as_deref(),
            Some("componente Angular <app-parcelamento>")
        );

        let e = extrair_em(
            "src/Parcelamento.tsx",
            "export function Parcelamento({ id }: Props) {\n  const salvar = () => api.post(`/api/faturas/${id}/parcelamento`, {});\n  return <div className='x'>Não é </div>;\n}\n\nexport const Botao = (p: Props) => <button onClick={() => salvar()}>Ok</button>;\n\nclass Antiga extends React.Component<Props> {\n  render() { return null; }\n}\n",
        );
        assert_eq!(simbolo(&e, "Parcelamento").tipo, TipoSimbolo::Tela);
        assert_eq!(simbolo(&e, "Parcelamento").linhas, Linhas::new(1, 4));
        assert_eq!(simbolo(&e, "Botao").tipo, TipoSimbolo::Tela);
        assert_eq!(simbolo(&e, "Antiga").tipo, TipoSimbolo::Tela);
        assert!(refs(&e, TipoReferencia::Chamada).contains(&(
            Some("Parcelamento"),
            "/api/faturas/{}/parcelamento",
            2
        )));
    }

    #[test]
    fn backend_nest_e_express() {
        let e = extrair(
            "@Entity('FATURA')\nexport class Fatura {\n  @PrimaryColumn({ name: 'ID_FATURA' })\n  id: number;\n}\n\n@Controller('api/faturas')\nexport class FaturasController {\n  @Post(':id/parcelamento')\n  parcelar(@Param('id') id: string) {}\n}\n\nrouter.get('/api/clientes/:id', (req, res) => res.json({}));\n",
        );
        let usos = refs(&e, TipoReferencia::Uso);
        assert_eq!(
            usos,
            [
                (Some("Fatura"), "FATURA", 2),
                (Some("Fatura.id"), "FATURA.ID_FATURA", 4)
            ]
        );
        let ep = simbolo(
            &e,
            "FaturasController.parcelar./api/faturas/{}/parcelamento",
        );
        assert_eq!(
            ep.detalhe.as_deref(),
            Some("endpoint POST /api/faturas/:id/parcelamento")
        );
        let ex = simbolo(&e, "/api/clientes/{}");
        assert_eq!(
            ex.detalhe.as_deref(),
            Some("endpoint GET /api/clientes/:id")
        );
    }

    #[test]
    fn javascript_sem_ponto_e_virgula() {
        let e = ExtratorTypeScript
            .extrair(&Fonte {
                caminho: "a.js",
                linguagem: Linguagem::JavaScript,
                texto: "const base = '/api'\nfunction carregar(id) {\n  return $.ajax({ url: base + '/faturas/' + id, type: 'GET' })\n}\nconst re = /}/g\nfunction depois() {\n  axios.delete(`${base}/faturas/${id}`)\n}\n",
            })
            .unwrap();
        assert_eq!(simbolo(&e, "carregar").linhas, Linhas::new(2, 4));
        assert_eq!(simbolo(&e, "depois").linhas, Linhas::new(6, 8));
        let chamadas = refs(&e, TipoReferencia::Chamada);
        assert!(chamadas.contains(&(Some("carregar"), "/api/faturas/{}", 3)));
        assert!(
            chamadas.contains(&(Some("depois"), "/api/faturas/{}", 7)),
            "{chamadas:?} {:#?}",
            e.simbolos
        );
    }

    #[test]
    fn codigo_quebrado_nao_derruba() {
        for t in [
            "class {",
            "function f( {",
            "`sem fim ${",
            "const a = /sem fim",
            "@Component(",
            "import {",
            "export default {",
            "}}}{{{",
        ] {
            let _ = extrair(t);
        }
        for (i, _) in SERVICO.char_indices() {
            let _ = extrair(&SERVICO[..i]);
        }
    }
}
