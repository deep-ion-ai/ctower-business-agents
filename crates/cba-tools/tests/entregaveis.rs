//! Gerar entregável a partir da demanda (#463) com o sistema fictício
//! "Faturamento Exemplo": sem análise validada, a demanda não oferece a geração;
//! com a análise de impacto aprovada, a história nasce como rascunho, cada item
//! cita uma regra confirmada da análise (a inexistente e a de fora são
//! recusadas, e a evidência vem da regra), o aviso de cobertura aponta a regra
//! sem história, e enviar ou publicar exige a revisão final, que uma edição ou
//! "Gerar de novo" desfazem. No caso de uso (vivo), a funcionalidade sem
//! documento gera antes a v1.0, e a demanda propõe a 1.1 com os trechos
//! marcados.

use std::path::{Path, PathBuf};

use cba_core::ambiente::Ambiente;
use cba_core::analise::{EstadoAfirmacao, EstadoAnalise, EstadoPergunta, Evidencia, Fluxo};
use cba_core::analises::NovaAnalise;
use cba_core::config::Config;
use cba_core::dados::DiretorioDados;
use cba_core::documentos_vivos::{DocumentosVivos, Dono, Marca, Versao};
use cba_core::entregaveis::{EntregaveisDaDemanda, Finalidade, Situacao};
use cba_core::eventos::Mudanca;
use cba_core::funcionalidades::{MapaFuncionalidades, NovaFuncionalidade, PontoAtribuido};
use cba_index::{Destino, Entregaveis, Metodologia, TipoHabilitado};
use cba_kb::frontmatter::{
    self, Demanda, FrontmatterEntregavel, FrontmatterRegra, Linhas, StatusRegra, TipoDocumento,
};
use cba_kb::{CaminhoRelativo, Documento, Id, IdDemanda, IdFuncionalidade, NovaDemanda};
use cba_tools::entregaveis::{
    PedidoDeGeracao, editar, exportar, gerar, marcar_revisado, preparar_comentario, publicar,
    vista_da_demanda,
};
use cba_tools::{ErroChamada, ErroFerramenta, registro_padrao};
use serde_json::{Value, json};

const SERVICO: &str = "backend/src/main/java/exemplo/faturamento/ParcelamentoService.java";
const PARAMETROS: &str = "db/02_parametros.sql";
const HISTORIA: &str = "historia-usuario";
const CASO: &str = "caso-de-uso";

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/faturamento-exemplo")
}

struct Cenario {
    dir: tempfile::TempDir,
    amb: Ambiente,
    demanda: IdDemanda,
}

fn fid() -> IdFuncionalidade {
    IdFuncionalidade::novo("FAT", 1).unwrap()
}

/// Projeto com a história (padrão), o caso de uso e o catálogo de regras
/// habilitados, uma demanda aberta e a funcionalidade "Parcelar fatura" no mapa.
fn cenario() -> Cenario {
    let dir = tempfile::tempdir().unwrap();
    let raiz = dir.path();
    let dados = DiretorioDados::em(raiz.join("dados"));
    let mut projeto = cba_index::Projeto::novo("Faturamento Exemplo", [fixture()]).unwrap();
    projeto.entregaveis = Some(Entregaveis {
        metodologia: Metodologia::Personalizado,
        tipos: vec![
            TipoHabilitado {
                destino: Destino::RepositorioDocs,
                ..TipoHabilitado::novo(HISTORIA)
            },
            TipoHabilitado::novo(CASO),
            TipoHabilitado::novo("catalogo-regras"),
        ],
        padrao: Some(HISTORIA.into()),
    });
    cba_index::Cadastro::em(dados.projetos())
        .cadastrar(&projeto)
        .unwrap();
    cba_kb::Base::criar(raiz.join("base")).unwrap();
    Config {
        base: Some(raiz.join("base")),
        ..Default::default()
    }
    .gravar(&dados)
    .unwrap();
    let amb = Ambiente::do_diretorio(&fixture(), &dados).unwrap();
    let demanda = amb
        .kb
        .criar_demanda(
            &NovaDemanda {
                titulo: "Parcelamento para pessoa física".into(),
                descricao: Some("Permitir que pessoa física parcele em até 3 vezes.".into()),
                chave: Some("FAT-12".into()),
                origem: Some("https://rastreador.exemplo.invalid/FAT-12".into()),
                tipo: None,
            },
            frontmatter::Data::nova(2026, 11, 28).unwrap(),
        )
        .unwrap()
        .id;
    MapaFuncionalidades::alterar(&amb.kb, |m, agora| {
        m.registrar(
            NovaFuncionalidade {
                nome: "Parcelar fatura".into(),
                area: "cobranca".into(),
                area_nome: "Cobrança".into(),
                descricao: "Divide a fatura em aberto em parcelas.".into(),
                atores: vec!["Atendente".into()],
                pontos: vec![PontoAtribuido {
                    chave: None,
                    tipo: None,
                    nome: "ParcelamentoService".into(),
                    evidencia: Evidencia {
                        arquivo: CaminhoRelativo::novo(SERVICO).unwrap(),
                        linhas: Linhas::novo(30, 32).unwrap(),
                        trecho: "X".into(),
                        hash: "h".into(),
                        commit: None,
                        estado: Default::default(),
                    },
                }],
                dados_lidos: vec![],
                dados_gravados: vec![],
                relacionadas: vec![],
                sessao: None,
            },
            fid(),
            agora,
        )
    })
    .unwrap();
    Cenario { dir, amb, demanda }
}

impl Cenario {
    fn chamar(&self, nome: &str, args: Value) -> Result<(String, Value), ErroFerramenta> {
        match registro_padrao().chamar(nome, Ok(&self.amb), Some(args)) {
            Ok(saida) => Ok((saida.texto, saida.dados.unwrap_or_default())),
            Err(ErroChamada::Ferramenta(e)) => Err(e),
            Err(e) => panic!("{e}"),
        }
    }

    fn recusa(&self, nome: &str, args: Value) -> String {
        match self.chamar(nome, args) {
            Err(ErroFerramenta::Recusa(m) | ErroFerramenta::Argumentos(m)) => m,
            outro => panic!("esperava recusa, veio {outro:?}"),
        }
    }

    fn gravar_regra(
        &self,
        id: &str,
        titulo: &str,
        evidencias: Vec<frontmatter::Evidencia>,
        origem: Option<CaminhoRelativo>,
        funcionalidades: Vec<IdFuncionalidade>,
    ) {
        let id = Id::analisar(id).unwrap();
        self.amb
            .kb
            .gravar(
                &id.arquivo(),
                &Documento::novo(
                    FrontmatterRegra {
                        tipo: TipoDocumento::Regra,
                        id: id.clone(),
                        titulo: titulo.into(),
                        area: "cobranca".into(),
                        status: StatusRegra::Confirmada,
                        evidencias,
                        origem,
                        informada: None,
                        historico: Vec::new(),
                        funcionalidades,
                    },
                    format!("\n# {id} · {titulo}\n\n{titulo}.\n"),
                ),
            )
            .unwrap();
    }

    /// Análise de impacto A-0001 ligada à demanda, com duas afirmações
    /// confirmadas, uma decisão respondida e uma pergunta em aberto; as regras
    /// RN-FAT-001 (origem na análise) e RN-FAT-002 (de outra análise, com as
    /// mesmas evidências da afirmação 2), e a RN-FAT-009, de fora.
    fn analise(&self) -> Id {
        let a = self
            .amb
            .analises
            .criar_com_demanda(
                NovaAnalise {
                    fluxo: Fluxo::Impacto,
                    titulo: "Parcelamento para pessoa física".into(),
                    entrada: "Permitir que pessoa física parcele em até 3 vezes.".into(),
                    analista: Some("Ana Exemplo".into()),
                    base: None,
                },
                Some(Demanda {
                    id: Some(self.demanda),
                    origem: "texto".into(),
                    chave: None,
                }),
            )
            .unwrap();
        let afirmar = |texto: &str, arquivo: &str, inicio: u32, fim: u32, trecho: &str| {
            self.chamar(
                "afirmar",
                json!({
                    "analise": a.id, "secao": "como_e_hoje", "texto": texto,
                    "confianca": "confirmado",
                    "evidencias": [{ "arquivo": arquivo, "linha_inicial": inicio,
                                     "linha_final": fim, "trecho": trecho }]
                }),
            )
            .unwrap();
        };
        afirmar(
            "Pessoa física não pode parcelar fatura: o backend recusa o pedido.",
            SERVICO,
            30,
            32,
            "if (fatura.tipoCliente() == TipoCliente.PF) {",
        );
        afirmar(
            "O valor mínimo da parcela vem do parâmetro VALOR_MIN_PARCELA.",
            PARAMETROS,
            18,
            19,
            "'VALOR_MIN_PARCELA', 50.0000",
        );
        self.chamar(
            "perguntar",
            json!({
                "analise": a.id, "tipo": "decisao",
                "texto": "A parcela mínima continua a mesma para pessoa física?",
                "opcoes": [
                    { "texto": "Manter", "efeito": "a parcela mínima não muda" },
                    { "texto": "Subir para R$ 100,00", "efeito": "muda o parâmetro" }
                ]
            }),
        )
        .unwrap();
        self.chamar(
            "perguntar",
            json!({ "analise": a.id, "texto": "O limite de 3 parcelas vale para todo cliente PF?" }),
        )
        .unwrap();
        let a = self
            .amb
            .analises
            .alterar(&a.id, Mudanca::Afirmacao, |an, _| {
                an.marcar_afirmacao(1, EstadoAfirmacao::Confirmada)?;
                an.marcar_afirmacao(2, EstadoAfirmacao::Confirmada)?;
                an.decidir_pergunta(
                    1,
                    EstadoPergunta::Respondida {
                        resposta: "Subir para R$ 100,00".into(),
                    },
                )
            })
            .unwrap()
            .1;
        let evidencias = |n: usize| -> Vec<frontmatter::Evidencia> {
            a.afirmacoes[n]
                .evidencias
                .iter()
                .map(|e| frontmatter::Evidencia {
                    arquivo: e.arquivo.clone(),
                    linhas: e.linhas,
                    hash: e.hash.clone(),
                    commit: e.commit.clone(),
                })
                .collect()
        };
        self.gravar_regra(
            "RN-FAT-001",
            "Pessoa física não parcela",
            evidencias(0),
            Some(a.documento.clone()),
            vec![fid()],
        );
        self.gravar_regra(
            "RN-FAT-002",
            "Parcela mínima por parâmetro",
            evidencias(1),
            Some(CaminhoRelativo::novo("analises/A-0099.md").unwrap()),
            vec![],
        );
        self.gravar_regra("RN-FAT-009", "Juros do parcelamento", vec![], None, vec![]);
        a.id
    }

    fn aprovar(&self, id: &Id) {
        self.amb
            .analises
            .alterar(id, Mudanca::Estado, |a, _| {
                a.estado = EstadoAnalise::Aprovada;
                Ok(())
            })
            .unwrap();
    }

    fn pedido(&self, tipo: &str, analise: &Id) -> PedidoDeGeracao {
        PedidoDeGeracao {
            demanda: self.demanda,
            tipo: tipo.into(),
            analise: analise.clone(),
            funcionalidade: None,
            finalidade: Finalidade::Demanda,
        }
    }

    fn registrar(&self, numero: u32, campos: Value) -> Result<(String, Value), ErroFerramenta> {
        self.chamar(
            "registrar_entregavel",
            json!({ "demanda": self.demanda.to_string(), "entregavel": numero, "campos": campos }),
        )
    }

    fn situacao(&self, numero: u32) -> Situacao {
        vista_da_demanda(&self.amb, self.demanda)
            .unwrap()
            .entregaveis
            .into_iter()
            .find(|e| e.numero == numero)
            .unwrap()
            .situacao
    }
}

fn historia(regras: &[&str]) -> Value {
    json!({
        "titulo": "Parcelar fatura de pessoa física",
        "papel": "atendente do faturamento",
        "acao": "parcelar a fatura de um cliente pessoa física em até 3 vezes",
        "beneficio": "reduzir a inadimplência",
        "criterios": ["A parcela mínima é de R$ 100,00"],
        "regras": regras,
    })
}

#[test]
fn sem_analise_validada_a_demanda_nao_oferece_a_geracao() {
    let c = cenario();
    let a = c.analise();
    let v = vista_da_demanda(&c.amb, c.demanda).unwrap();
    assert!(v.analises.is_empty(), "a análise ainda não foi aprovada");
    assert!(v.aberta);
    // Só os tipos habilitados no projeto, com o padrão marcado.
    let tipos: Vec<(&str, bool)> = v.tipos.iter().map(|t| (t.id.as_str(), t.padrao)).collect();
    assert_eq!(
        tipos,
        [(HISTORIA, true), (CASO, false), ("catalogo-regras", false)]
    );
    assert!(v.tipos[2].indisponivel.is_some(), "montado pelo app");
    let recusa = gerar(&c.amb, c.pedido(HISTORIA, &a)).unwrap_err();
    assert!(
        recusa.to_string().contains("não é uma análise validada"),
        "{recusa}"
    );

    c.aprovar(&a);
    let v = vista_da_demanda(&c.amb, c.demanda).unwrap();
    assert_eq!(v.analises.len(), 1);
    let base = &v.analises[0];
    let regras: Vec<&str> = base.regras.iter().map(|r| r.id.as_str()).collect();
    // A RN-FAT-002 tem a origem em outra análise, mas as mesmas evidências da
    // afirmação confirmada; a RN-FAT-009 é de fora.
    assert_eq!(regras, ["RN-FAT-001", "RN-FAT-002"]);
    assert_eq!(base.decisoes.len(), 1);
    assert_eq!(base.decisoes[0].resposta, "Subir para R$ 100,00");
    assert_eq!(base.perguntas.len(), 1);
    assert!(!base.perguntas[0].decisao);

    // Tipo fora do projeto e tipo montado pelo app: recusados.
    let fora = gerar(&c.amb, c.pedido("epico", &a)).unwrap_err();
    assert!(fora.to_string().contains("não está habilitado"), "{fora}");
    let app = gerar(&c.amb, c.pedido("catalogo-regras", &a)).unwrap_err();
    assert!(app.to_string().contains("montado pelo app"), "{app}");
}

#[test]
fn historia_rastreavel_com_revisao_final_obrigatoria() {
    let c = cenario();
    let a = c.analise();
    c.aprovar(&a);
    let g = gerar(&c.amb, c.pedido(HISTORIA, &a)).unwrap();
    assert_eq!((g.numero, g.versao), (1, 1));
    assert!(g.prompt.contains("iniciar_entregavel"));
    assert_eq!(c.situacao(1), Situacao::Gerando);

    let (roteiro, dados) = c
        .chamar(
            "iniciar_entregavel",
            json!({ "demanda": c.demanda.to_string(), "entregavel": 1 }),
        )
        .unwrap();
    assert!(
        roteiro.contains("RN-FAT-001: Pessoa física não parcela"),
        "{roteiro}"
    );
    assert!(!roteiro.contains("RN-FAT-009"), "{roteiro}");
    assert!(roteiro.contains("Subir para R$ 100,00"), "{roteiro}");
    assert!(roteiro.contains("O limite de 3 parcelas"), "{roteiro}");
    assert_eq!(dados["tipo"]["id"], HISTORIA);

    // O esquema do tipo.
    let r = c.recusa(
        "registrar_entregavel",
        json!({ "demanda": c.demanda.to_string(), "entregavel": 1,
                "campos": { "titulo": "Sem o resto", "regras": ["RN-FAT-001"], "extra": 1 } }),
    );
    assert!(r.contains("extra: campo desconhecido"), "{r}");
    assert!(r.contains("papel: obrigatório"), "{r}");
    // Regra que não existe e regra de fora da análise.
    let r = c.recusa(
        "registrar_entregavel",
        json!({ "demanda": c.demanda.to_string(), "entregavel": 1, "campos": historia(&["RN-FAT-404"]) }),
    );
    assert!(r.contains("a regra RN-FAT-404 não existe"), "{r}");
    let r = c.recusa(
        "registrar_entregavel",
        json!({ "demanda": c.demanda.to_string(), "entregavel": 1, "campos": historia(&["RN-FAT-009"]) }),
    );
    assert!(
        r.contains("a regra RN-FAT-009 não é uma regra confirmada da análise A-0001"),
        "{r}"
    );
    assert!(r.contains("RN-FAT-001, RN-FAT-002"), "{r}");
    assert_eq!(c.situacao(1), Situacao::Gerando, "nada registrado");

    // Item aceito, com a cobertura avisando a regra sem história.
    let (texto, dados) = c.registrar(1, historia(&["RN-FAT-001"])).unwrap();
    assert!(texto.contains("RN-FAT-002 sem história"), "{texto}");
    assert_eq!(dados["regras_sem_item"], json!(["RN-FAT-002"]));
    let v = vista_da_demanda(&c.amb, c.demanda).unwrap();
    let e = &v.entregaveis[0];
    assert_eq!(e.situacao, Situacao::Rascunho);
    assert_eq!(
        e.aviso_cobertura.as_deref(),
        Some("RN-FAT-002 sem história")
    );
    assert!(
        e.conferir
            .iter()
            .any(|c| c.contains("RN-FAT-002 sem história"))
    );
    assert!(
        e.texto.starts_with("## Parcelar fatura de pessoa física"),
        "{}",
        e.texto
    );
    assert!(e.texto.contains("- RN-FAT-001: Pessoa física não parcela"));
    // A evidência vem da regra, com o trecho do código.
    let regra = &e.itens[0].regras[0];
    assert_eq!(regra.id, "RN-FAT-001");
    assert_eq!(regra.evidencias[0].arquivo, SERVICO);
    assert!(
        regra.evidencias[0]
            .trecho
            .as_deref()
            .unwrap()
            .contains("TipoCliente.PF"),
        "{regra:?}"
    );

    // Segundo item cobre a RN-FAT-002.
    let (texto, _) = c.registrar(1, historia(&["RN-FAT-002"])).unwrap();
    assert!(
        texto.contains("Todas as regras confirmadas têm item"),
        "{texto}"
    );

    // Enviar e publicar exigem a revisão final.
    let destino = c.dir.path().join("historia.md");
    let r = exportar(&c.amb, c.demanda, 1, &destino, "ana").unwrap_err();
    assert!(r.to_string().contains("revisão final é obrigatória"), "{r}");
    assert!(publicar(&c.amb, c.demanda, 1, "ana").is_err());
    assert!(preparar_comentario(&c.amb, c.demanda, 1, "ana").is_err());
    assert!(!destino.exists());

    marcar_revisado(&c.amb, c.demanda, 1, "ana").unwrap();
    assert_eq!(c.situacao(1), Situacao::Revisado);
    // Uma edição depois da revisão volta o entregável para rascunho.
    let editado = format!(
        "{}\n\nObservação do analista.",
        vista_da_demanda(&c.amb, c.demanda).unwrap().entregaveis[0].texto
    );
    let e = editar(&c.amb, c.demanda, 1, &editado).unwrap();
    assert_eq!(e.situacao(), Situacao::Rascunho);
    assert!(exportar(&c.amb, c.demanda, 1, &destino, "ana").is_err());

    marcar_revisado(&c.amb, c.demanda, 1, "ana").unwrap();
    exportar(&c.amb, c.demanda, 1, &destino, "ana").unwrap();
    let exportado = std::fs::read_to_string(&destino).unwrap();
    assert!(exportado.contains("Observação do analista."));
    assert!(exportado.contains("revisado por ana"));
    assert_eq!(c.situacao(1), Situacao::Enviado);

    // Publicar grava o entregável na base, com frontmatter válido.
    let e = publicar(&c.amb, c.demanda, 1, "ana").unwrap();
    let onde = &e.envios.last().unwrap().onde;
    assert!(
        onde.ends_with("entregaveis/E-1-historia-usuario.md"),
        "{onde}"
    );
    let doc: Documento<FrontmatterEntregavel> =
        c.amb.kb.ler(&CaminhoRelativo::novo(onde).unwrap()).unwrap();
    assert_eq!(doc.frontmatter.analise, a);
    assert_eq!(doc.frontmatter.revisor, "ana");
    assert!(doc.corpo.contains("Observação do analista."));
    assert!(cba_tools::publicacao::aprovado(
        &std::fs::read_to_string(c.amb.kb.caminho(&CaminhoRelativo::novo(onde).unwrap())).unwrap()
    ));

    let (_, comentario) = preparar_comentario(&c.amb, c.demanda, 1, "ana").unwrap();
    assert_eq!(comentario.chave.as_deref(), Some("FAT-12"));
    assert!(comentario.link.unwrap().starts_with("https://"));

    // "Gerar de novo" cria a v2 do rascunho, sem sobrescrever a v1.
    let g = gerar(&c.amb, c.pedido(HISTORIA, &a)).unwrap();
    assert_eq!((g.numero, g.versao), (1, 2));
    assert_eq!(c.situacao(1), Situacao::Gerando);
    let pasta = c.amb.kb.demanda(c.demanda).unwrap().pasta;
    let estado = EntregaveisDaDemanda::carregar(&c.amb.kb, &pasta).unwrap();
    let e = estado.entregavel(1).unwrap();
    assert_eq!(e.rascunhos.len(), 2);
    assert_eq!(e.rascunho(1).unwrap().itens.len(), 2);
    assert!(
        e.rascunho(1)
            .unwrap()
            .texto()
            .contains("Observação do analista.")
    );
    assert_eq!(e.envios.len(), 3, "os envios da v1 ficam no histórico");
}

#[test]
fn caso_de_uso_gera_a_versao_inicial_antes_da_proposta_da_demanda() {
    let c = cenario();
    let a = c.analise();
    c.aprovar(&a);
    let mut pedido = c.pedido(CASO, &a);
    assert!(
        gerar(&c.amb, pedido.clone())
            .unwrap_err()
            .to_string()
            .contains("escolha a funcionalidade")
    );
    pedido.funcionalidade = Some(fid());
    let r = gerar(&c.amb, pedido.clone()).unwrap_err();
    assert!(r.to_string().contains("gere a v1.0 antes"), "{r}");
    let v = vista_da_demanda(&c.amb, c.demanda).unwrap();
    let f = &v.funcionalidades[0];
    assert_eq!(f.id, "F-FAT-001");
    assert_eq!(f.documentos[0].estado, "sem documento");

    // v1.0, por engenharia reversa.
    let mut inicial = pedido.clone();
    inicial.finalidade = Finalidade::VersaoInicial;
    let g = gerar(&c.amb, inicial).unwrap();
    let caso = |fluxo: &[&str], regras: &[&str]| {
        json!({
            "titulo": "Parcelar fatura",
            "objetivo": "Dividir a fatura em aberto em parcelas.",
            "atores": ["Atendente"],
            "fluxo_principal": fluxo,
            "regras": regras,
        })
    };
    let (texto, _) = c
        .registrar(
            g.numero,
            caso(
                &[
                    "O atendente abre a fatura.",
                    "O sistema recusa pessoa física.",
                ],
                &["RN-FAT-001"],
            ),
        )
        .unwrap();
    assert!(texto.contains("Proposta de versão"), "{texto}");
    marcar_revisado(&c.amb, c.demanda, g.numero, "ana").unwrap();
    let e = publicar(&c.amb, c.demanda, g.numero, "ana").unwrap();
    assert!(e.envios[0].onde.contains("(versão 1.0)"), "{:?}", e.envios);
    let docs = DocumentosVivos::carregar(&c.amb.kb).unwrap();
    let doc = docs.do_dono(CASO, &Dono::Funcionalidade(fid())).unwrap();
    assert_eq!(doc.versao_publicada(), Some(Versao::INICIAL));
    assert_eq!(
        doc.publicada().unwrap().mudancas,
        "Versão inicial, gerada por engenharia reversa"
    );

    // A demanda propõe a 1.1, com o fluxo alterado marcado.
    let g = gerar(&c.amb, pedido).unwrap();
    let r = c.recusa(
        "registrar_entregavel",
        json!({ "demanda": c.demanda.to_string(), "entregavel": g.numero,
                "campos": caso(&["O atendente abre a fatura.", "O sistema parcela em até 3 vezes."], &["RN-FAT-001"]) }),
    );
    assert!(r.contains("`mudancas`"), "{r}");
    c.chamar(
        "registrar_entregavel",
        json!({ "demanda": c.demanda.to_string(), "entregavel": g.numero,
                "mudancas": "Pessoa física passa a parcelar em até 3 vezes.",
                "campos": caso(&["O atendente abre a fatura.", "O sistema parcela em até 3 vezes."], &["RN-FAT-001", "RN-FAT-002"]) }),
    )
    .unwrap();
    let v = vista_da_demanda(&c.amb, c.demanda).unwrap();
    let e = v.entregaveis.iter().find(|e| e.numero == g.numero).unwrap();
    let p = e.proposta.as_ref().unwrap();
    assert_eq!(p.versao.to_string(), "1.1");
    let alterado = p
        .trechos
        .iter()
        .find(|t| t.marca == Marca::Alterado)
        .unwrap();
    assert_eq!(
        alterado.anterior.as_deref(),
        Some("- O sistema recusa pessoa física.")
    );
    assert_eq!(p.historico.len(), 1);
    assert_eq!(e.situacao, Situacao::Rascunho);
    assert!(v.funcionalidades[0].documentos[0].estado == "proposta pendente");

    // Publicar a 1.1 só depois da revisão.
    assert!(publicar(&c.amb, c.demanda, g.numero, "ana").is_err());
    marcar_revisado(&c.amb, c.demanda, g.numero, "ana").unwrap();
    publicar(&c.amb, c.demanda, g.numero, "ana").unwrap();
    let docs = DocumentosVivos::carregar(&c.amb.kb).unwrap();
    let doc = docs.do_dono(CASO, &Dono::Funcionalidade(fid())).unwrap();
    assert_eq!(doc.versao_publicada().unwrap().to_string(), "1.1");
    assert_eq!(doc.publicada().unwrap().demanda, Some(c.demanda));
    assert_eq!(doc.publicada().unwrap().revisor, "ana");
    assert!(
        doc.versao(Versao::INICIAL).is_some(),
        "a 1.0 continua acessível"
    );
}
