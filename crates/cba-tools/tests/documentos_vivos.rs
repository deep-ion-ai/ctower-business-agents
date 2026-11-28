//! Documentos vivos (#461) com uma cópia do sistema fictício "Faturamento
//! Exemplo" num repositório git temporário: o caso de uso da funcionalidade
//! "Cancelar fatura" nasce na 1.0, ganha a 1.1 por uma demanda (com a revisão
//! final obrigatória), o arquivo publicado traz o histórico de revisões, a
//! versão anterior continua acessível e um commit no arquivo citado marca o
//! documento como desatualizado. O documento negocial de antes do histórico
//! entra como 1.0 na primeira proposta.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use cba_core::ambiente::Ambiente;
use cba_core::analise::{EstadoEvidencia, Evidencia};
use cba_core::config::Config;
use cba_core::dados::DiretorioDados;
use cba_core::documentos_vivos::{
    DocumentosVivos, Dono, EstadoDocumento, Marca, NovaProposta, Versao,
};
use cba_core::funcionalidades::{MapaFuncionalidades, NovaFuncionalidade, PontoAtribuido};
use cba_kb::frontmatter::Linhas;
use cba_kb::{CaminhoRelativo, IdDemanda, IdFuncionalidade};
use cba_tools::ErroFerramenta;
use cba_tools::documentos_vivos::{
    MUDANCAS_DO_IMPORTADO, PedidoDeProposta, descartar_proposta, documentos_da_funcionalidade,
    editar_proposta, marcar_revisada, propor, publicar, texto_da_versao,
};

const SITUACAO: &str = "db/04_pkg_situacao.sql";
const CASO: &str = "caso-de-uso";
const NEGOCIAL: &str = "documento-negocial";

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/faturamento-exemplo")
}

fn copiar(de: &Path, para: &Path) {
    fs::create_dir_all(para).unwrap();
    for entrada in fs::read_dir(de).unwrap() {
        let entrada = entrada.unwrap();
        let destino = para.join(entrada.file_name());
        if entrada.file_type().unwrap().is_dir() {
            copiar(&entrada.path(), &destino);
        } else {
            fs::copy(entrada.path(), destino).unwrap();
        }
    }
}

fn git(dir: &Path, args: &[&str]) {
    let saida = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args([
            "-c",
            "user.name=Ana Exemplo",
            "-c",
            "user.email=ana@exemplo.invalid",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(
        saida.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&saida.stderr)
    );
}

struct Cenario {
    _dir: tempfile::TempDir,
    codigo: PathBuf,
    amb: Ambiente,
}

fn fid() -> IdFuncionalidade {
    IdFuncionalidade::novo("FAT", 1).unwrap()
}

/// Cópia da fixture num repositório git, cadastrada, com uma base vazia e a
/// funcionalidade "Cancelar fatura" no mapa.
fn cenario() -> Cenario {
    let dir = tempfile::tempdir().unwrap();
    let raiz = dir.path();
    let codigo = raiz.join("faturamento");
    copiar(&fixture(), &codigo);
    git(&codigo, &["init", "-q", "-b", "main"]);
    git(&codigo, &["add", "-A"]);
    git(&codigo, &["commit", "-q", "-m", "Versão inicial"]);

    let dados = DiretorioDados::em(raiz.join("dados"));
    let projeto = cba_index::Projeto::novo("Faturamento Exemplo", [&codigo]).unwrap();
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
    let amb = Ambiente::do_diretorio(&codigo, &dados).unwrap();
    MapaFuncionalidades::alterar(&amb.kb, |m, agora| {
        m.registrar(
            NovaFuncionalidade {
                nome: "Cancelar fatura".into(),
                area: "cobranca".into(),
                area_nome: "Cobrança".into(),
                descricao: "Cancela a fatura em aberto.".into(),
                atores: vec!["Atendente".into()],
                pontos: vec![PontoAtribuido {
                    chave: None,
                    tipo: None,
                    nome: "PKG_SITUACAO".into(),
                    evidencia: Evidencia {
                        arquivo: CaminhoRelativo::novo(SITUACAO).unwrap(),
                        linhas: Linhas::novo(1, 1).unwrap(),
                        trecho: "X".into(),
                        hash: "h".into(),
                        commit: None,
                        estado: EstadoEvidencia::Valida,
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
    Cenario {
        _dir: dir,
        codigo,
        amb,
    }
}

impl Cenario {
    fn ler(&self, caminho: &CaminhoRelativo) -> String {
        fs::read_to_string(self.amb.kb.caminho(caminho)).unwrap()
    }

    fn documentos(&self) -> Vec<cba_tools::documentos_vivos::DocumentoDoTipo> {
        let mapa = MapaFuncionalidades::carregar(&self.amb.kb).unwrap();
        let docs = DocumentosVivos::carregar(&self.amb.kb).unwrap();
        documentos_da_funcionalidade(
            &self.amb.escopo,
            &docs,
            mapa.funcionalidade(&fid()).unwrap(),
            &[CASO, NEGOCIAL],
        )
    }

    fn estado_do_caso(&self) -> EstadoDocumento {
        self.documentos().remove(0).estado
    }
}

fn pedido(tipo: &str, texto: &str, mudancas: &str, demanda: Option<u32>) -> PedidoDeProposta {
    PedidoDeProposta {
        tipo: tipo.into(),
        dono: Dono::Funcionalidade(fid()),
        caminho: None,
        proposta: NovaProposta {
            texto: texto.into(),
            mudancas: mudancas.into(),
            demanda: demanda.map(IdDemanda::novo),
            analise: None,
            arquivos: vec![SITUACAO.into()],
        },
    }
}

const V10: &str = "# Cancelar fatura\n\n## Fluxo principal\n\n1. O atendente abre a fatura.\n2. O sistema cancela.";

#[test]
fn caso_de_uso_versionado_do_rascunho_a_desatualizado() {
    let c = cenario();
    assert_eq!(c.estado_do_caso(), EstadoDocumento::SemDocumento);

    // v1.0: rascunho até a revisão final.
    let r = propor(
        &c.amb,
        pedido(
            CASO,
            V10,
            "Versão inicial, gerada por engenharia reversa",
            None,
        ),
    )
    .unwrap();
    let caminho = r.caminho.clone();
    assert_eq!(
        caminho.as_str(),
        "funcionalidades/cobranca/cancelar-fatura/caso-de-uso.md"
    );
    assert_eq!(r.proposta.versao, Versao::INICIAL);
    assert!(matches!(
        publicar(&c.amb, &caminho),
        Err(ErroFerramenta::Recusa(m)) if m.contains("revisão final")
    ));
    assert!(!c.amb.kb.caminho(&caminho).exists());
    marcar_revisada(&c.amb, &caminho, "Ana Exemplo").unwrap();
    let p = publicar(&c.amb, &caminho).unwrap();
    assert_eq!(p.revisao.versao, Versao::INICIAL);
    assert!(p.avisos.is_empty(), "{:?}", p.avisos);
    assert_eq!(p.revisao.arquivos[0].arquivo, SITUACAO);
    let arquivo = c.ler(&caminho);
    assert!(arquivo.starts_with("---\n"), "{arquivo}");
    assert!(arquivo.contains("tipo: caso-de-uso"), "{arquivo}");
    assert!(arquivo.contains("funcionalidade: F-FAT-001"), "{arquivo}");
    assert!(arquivo.contains("## Histórico de revisões"), "{arquivo}");
    let mapa = MapaFuncionalidades::carregar(&c.amb.kb).unwrap();
    assert_eq!(
        mapa.funcionalidade(&fid()).unwrap().documento(CASO),
        Some(&caminho)
    );
    assert_eq!(
        c.estado_do_caso(),
        EstadoDocumento::EmDia {
            versao: Versao::INICIAL
        }
    );

    // v1.1 por uma demanda: o mesmo documento, com os trechos marcados.
    let v11 = V10.replace(
        "2. O sistema cancela.",
        "2. O sistema confere se está em aberto.\n3. O sistema cancela.",
    );
    let r = propor(
        &c.amb,
        pedido(CASO, &v11, "Confere a situação antes", Some(42)),
    )
    .unwrap();
    assert_eq!(r.caminho, caminho);
    assert_eq!(r.proposta.versao, Versao::nova(1, 1));
    let marcas: Vec<Marca> = r.proposta.mudancas_marcadas().map(|t| t.marca).collect();
    assert_eq!(marcas, vec![Marca::Alterado, Marca::Novo]);
    marcar_revisada(&c.amb, &caminho, "Bruno Exemplo").unwrap();
    // A edição depois da revisão volta a rascunho.
    let editada =
        editar_proposta(&c.amb, &caminho, &format!("{v11}\n\nObservação."), None).unwrap();
    assert!(editada.revisada.is_none());
    assert!(publicar(&c.amb, &caminho).is_err());
    marcar_revisada(&c.amb, &caminho, "Bruno Exemplo").unwrap();
    let p = publicar(&c.amb, &caminho).unwrap();
    assert_eq!(p.revisao.versao, Versao::nova(1, 1));
    assert_eq!(p.revisao.demanda, Some(IdDemanda::novo(42)));
    assert_eq!(p.revisao.revisor, "Bruno Exemplo");
    assert_eq!(p.versoes, 2);
    let arquivo = c.ler(&caminho);
    assert!(arquivo.contains("Observação."));
    assert!(arquivo.contains("| 1.0 |"), "{arquivo}");
    assert!(
        arquivo.contains("| 1.1 |")
            && arquivo.contains("| D-0042 | — | Bruno Exemplo | Confere a situação antes |"),
        "{arquivo}"
    );
    // O frontmatter ficou e o histórico não se repete.
    assert_eq!(arquivo.matches("tipo: caso-de-uso").count(), 1);
    assert_eq!(arquivo.matches("## Histórico de revisões").count(), 1);
    let docs = DocumentosVivos::carregar(&c.amb.kb).unwrap();
    assert_eq!(
        texto_da_versao(&docs, &caminho, Versao::INICIAL).unwrap(),
        V10
    );

    // Uma proposta descartada deixa a versão publicada.
    propor(&c.amb, pedido(CASO, "Outra coisa", "m", None)).unwrap();
    assert!(matches!(
        c.estado_do_caso(),
        EstadoDocumento::PropostaPendente { .. }
    ));
    descartar_proposta(&c.amb, &caminho).unwrap();
    assert_eq!(
        c.estado_do_caso(),
        EstadoDocumento::EmDia {
            versao: Versao::nova(1, 1)
        }
    );

    // Só reformatar o arquivo citado não marca; um commit que muda o código, sim.
    let citado = c.codigo.join(SITUACAO);
    let original = fs::read_to_string(&citado).unwrap();
    fs::write(&citado, original.replace('\n', "  \r\n")).unwrap();
    assert!(matches!(c.estado_do_caso(), EstadoDocumento::EmDia { .. }));
    fs::write(
        &citado,
        format!("{original}\n-- cancela também a fatura vencida\n"),
    )
    .unwrap();
    git(&c.codigo, &["commit", "-q", "-am", "Muda o cancelamento"]);
    let doc = c.documentos().remove(0);
    assert_eq!(
        doc.estado,
        EstadoDocumento::Desatualizado {
            versao: Versao::nova(1, 1),
            arquivos: vec![SITUACAO.into()]
        }
    );
    assert_eq!(doc.acao, Some("Propor atualização"));
    // Não se reescreve sozinho.
    assert!(c.ler(&caminho).contains("Observação."));
    assert_eq!(
        DocumentosVivos::carregar(&c.amb.kb)
            .unwrap()
            .documento(&caminho)
            .unwrap()
            .historico
            .len(),
        2
    );
}

#[test]
fn negocial_de_antes_entra_como_1_0_e_tipos_que_nao_sao_vivos_sao_recusados() {
    let c = cenario();
    // O documento negocial do formato antigo: vinculado, sem histórico.
    let negocial =
        CaminhoRelativo::novo("funcionalidades/cobranca/cancelar-fatura/negocial.md").unwrap();
    let destino = c.amb.kb.caminho(&negocial);
    fs::create_dir_all(destino.parent().unwrap()).unwrap();
    fs::write(
        &destino,
        "---\ntipo: funcionalidade\ntitulo: Cancelar fatura\narea: cobranca\ntema: cancelar-fatura\nstatus: aprovada\n---\n\n# Cancelar fatura\n\nTexto aprovado.\n",
    )
    .unwrap();
    MapaFuncionalidades::alterar(&c.amb.kb, |m, _| {
        m.ligar_documento(&fid(), &cba_kb::Serie::Analise.id(3), negocial.clone());
        Ok(())
    })
    .unwrap();
    let docs = c.documentos();
    assert_eq!(docs[1].caminho.as_ref(), Some(&negocial));
    assert_eq!(docs[1].versao, Some(Versao::INICIAL));
    assert!(matches!(docs[1].estado, EstadoDocumento::EmDia { .. }));

    let r = propor(
        &c.amb,
        pedido(
            NEGOCIAL,
            "# Cancelar fatura\n\nTexto revisto.",
            "Revisão do texto",
            Some(7),
        ),
    )
    .unwrap();
    assert_eq!(r.caminho, negocial);
    assert_eq!(r.proposta.versao, Versao::nova(1, 1));
    let docs = DocumentosVivos::carregar(&c.amb.kb).unwrap();
    let d = docs.documento(&negocial).unwrap();
    assert_eq!(d.historico[0].texto, "# Cancelar fatura\n\nTexto aprovado.");
    assert_eq!(d.historico[0].mudancas, MUDANCAS_DO_IMPORTADO);
    marcar_revisada(&c.amb, &negocial, "Ana Exemplo").unwrap();
    publicar(&c.amb, &negocial).unwrap();
    let arquivo = c.ler(&negocial);
    assert!(
        arquivo.starts_with("---\ntipo: funcionalidade\n"),
        "{arquivo}"
    );
    assert!(arquivo.contains("Texto revisto."));
    assert!(
        arquivo.contains("| 1.0 |") && arquivo.contains("| 1.1 |"),
        "{arquivo}"
    );

    // Um documento vivo nunca é recriado em outro caminho.
    let mut outro = pedido(CASO, "Texto", "m", None);
    outro.caminho = Some(negocial.clone());
    assert!(matches!(
        propor(&c.amb, outro),
        Err(ErroFerramenta::Recusa(_))
    ));
    assert!(matches!(
        propor(&c.amb, pedido("historia-usuario", "T", "m", None)),
        Err(ErroFerramenta::Recusa(m)) if m.contains("por demanda")
    ));
    assert!(matches!(
        propor(&c.amb, pedido("inexistente", "T", "m", None)),
        Err(ErroFerramenta::Argumentos(_))
    ));
    let mut descartada = pedido(CASO, "T", "m", None);
    descartada.dono = Dono::Funcionalidade(IdFuncionalidade::novo("FAT", 9).unwrap());
    assert!(matches!(
        propor(&c.amb, descartada),
        Err(ErroFerramenta::Recusa(_))
    ));
}
