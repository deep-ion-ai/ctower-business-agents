//! Documentação por funcionalidade no mapa (#462), com uma cópia do sistema
//! fictício "Faturamento Exemplo": num projeto RUP com o manual habilitado, a
//! tabela tem as colunas caso de uso, catálogo de regras, especificação
//! suplementar e manual, com o estado de cada célula; a cobertura por tipo bate
//! com as células; a geração em lote de casos de uso para três funcionalidades
//! deixa três rascunhos 1.0 e nenhum é publicado sem a revisão final; os tipos
//! montados pelo app saem na hora, iguais em duas gerações; e um documento que
//! já existe não é recriado.

use std::fs;
use std::path::{Path, PathBuf};

use cba_core::ambiente::Ambiente;
use cba_core::analise::{EstadoEvidencia, Evidencia};
use cba_core::config::Config;
use cba_core::dados::DiretorioDados;
use cba_core::documentos_vivos::{DocumentosVivos, Dono, EstadoDocumento, Versao};
use cba_core::funcionalidades::{MapaFuncionalidades, NovaFuncionalidade, PontoAtribuido};
use cba_docs::entregaveis::{self, Geracao};
use cba_index::{Entregaveis, Metodologia, TipoHabilitado};
use cba_kb::frontmatter::{FrontmatterRegra, Linhas, StatusRegra};
use cba_kb::{CaminhoRelativo, Documento, Id, IdFuncionalidade, TipoDocumento};
use cba_tools::cobertura::CoberturaDoTipo;
use cba_tools::documentacao_do_mapa::{
    ItemDoLote, MUDANCAS_DA_VERSAO_INICIAL, contexto, gerar_em_lote, montar, propor_atualizacao,
    tabela,
};
use cba_tools::documentos_vivos::{marcar_revisada, publicar};
use cba_tools::registro_padrao;
use cba_tools::{ErroChamada, ErroFerramenta};
use serde_json::{Value, json};

const CASO: &str = entregaveis::CASO_DE_USO;
const REGRAS: &str = entregaveis::CATALOGO_REGRAS;
const SUPLEMENTAR: &str = entregaveis::ESPECIFICACAO_SUPLEMENTAR;
const MANUAL: &str = entregaveis::MANUAL;

/// Arquivos dos pontos de entrada das três funcionalidades.
const ARQUIVOS: [&str; 3] = [
    "db/04_pkg_situacao.sql",
    "db/03_pkg_cobranca.sql",
    "db/05_cliente_contato.sql",
];

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

fn fid(n: u32) -> IdFuncionalidade {
    IdFuncionalidade::novo("FAT", n).unwrap()
}

fn evidencia(arquivo: &str) -> Evidencia {
    Evidencia {
        arquivo: CaminhoRelativo::novo(arquivo).unwrap(),
        linhas: Linhas::novo(1, 1).unwrap(),
        trecho: "X".into(),
        hash: "h".into(),
        commit: None,
        estado: EstadoEvidencia::Valida,
    }
}

struct Cenario {
    _dir: tempfile::TempDir,
    codigo: PathBuf,
    amb: Ambiente,
}

/// RUP com o manual habilitado (como no desenho do mapa).
fn rup_com_manual() -> Entregaveis {
    let mut e = entregaveis::predefinicao(Metodologia::Rup)
        .unwrap()
        .entregaveis();
    e.tipos.push(TipoHabilitado::novo(MANUAL));
    e
}

/// Cópia da fixture cadastrada com os entregáveis, uma base vazia, três
/// funcionalidades no mapa e a regra RN-FAT-001 com evidência no arquivo da
/// primeira.
fn cenario(config: Entregaveis) -> Cenario {
    let dir = tempfile::tempdir().unwrap();
    let raiz = dir.path();
    let codigo = raiz.join("faturamento");
    copiar(&fixture(), &codigo);
    let dados = DiretorioDados::em(raiz.join("dados"));
    let mut projeto = cba_index::Projeto::novo("Faturamento Exemplo", [&codigo]).unwrap();
    projeto.entregaveis = Some(config);
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
    let nomes = ["Cancelar fatura", "Calcular multa", "Atualizar contato"];
    MapaFuncionalidades::alterar(&amb.kb, |m, agora| {
        for (i, (nome, arquivo)) in nomes.iter().zip(ARQUIVOS).enumerate() {
            m.registrar(
                NovaFuncionalidade {
                    nome: (*nome).into(),
                    area: "cobranca".into(),
                    area_nome: "Cobrança".into(),
                    descricao: format!("{nome} no faturamento."),
                    atores: vec!["Atendente".into()],
                    pontos: vec![PontoAtribuido {
                        chave: None,
                        tipo: None,
                        nome: format!("PONTO_{}", i + 1),
                        evidencia: evidencia(arquivo),
                    }],
                    dados_lidos: vec!["FATURA".into()],
                    dados_gravados: vec![],
                    relacionadas: vec![],
                    sessao: None,
                },
                fid(u32::try_from(i).unwrap() + 1),
                agora,
            )?;
        }
        Ok(())
    })
    .unwrap();
    gravar_regra(
        &amb,
        "RN-FAT-001",
        "Fatura paga não é cancelada",
        "Uma fatura paga não pode ser cancelada",
    );
    Cenario {
        _dir: dir,
        codigo,
        amb,
    }
}

/// Regra confirmada com evidência no arquivo da primeira funcionalidade.
fn gravar_regra(amb: &Ambiente, id: &str, titulo: &str, enunciado: &str) {
    let id = Id::analisar(id).unwrap();
    let regra = Documento::novo(
        FrontmatterRegra {
            tipo: TipoDocumento::Regra,
            id: id.clone(),
            titulo: titulo.into(),
            area: "cobranca".into(),
            status: StatusRegra::Confirmada,
            evidencias: vec![cba_kb::frontmatter::Evidencia {
                arquivo: CaminhoRelativo::novo(ARQUIVOS[0]).unwrap(),
                linhas: Linhas::novo(1, 2).unwrap(),
                hash: "h".into(),
                commit: None,
            }],
            origem: None,
            informada: None,
            historico: Vec::new(),
            funcionalidades: Vec::new(),
        },
        format!(
            "# {titulo}\n\n## Enunciado\n\n{enunciado}[^ev-1].\n\n## Evidências\n\n[^ev-1]: x\n"
        ),
    );
    amb.kb.gravar(&id.arquivo(), &regra).unwrap();
}

impl Cenario {
    fn chamar(&self, nome: &str, args: Value) -> Result<String, ErroFerramenta> {
        match registro_padrao().chamar(nome, Ok(&self.amb), Some(args)) {
            Ok(saida) => Ok(saida.texto),
            Err(ErroChamada::Ferramenta(e)) => Err(e),
            Err(e) => panic!("{e}"),
        }
    }

    fn estado(&self, tipo: &str, n: u32) -> EstadoDocumento {
        let t = tabela(&self.amb).unwrap();
        let col = t.colunas.iter().position(|c| c.tipo == tipo).unwrap();
        let linha = t
            .linhas
            .iter()
            .find(|l| l.funcionalidade == fid(n).to_string())
            .unwrap();
        linha.documentos[col].estado.clone()
    }

    fn caminho(&self, tipo: &str, n: u32) -> CaminhoRelativo {
        DocumentosVivos::carregar(&self.amb.kb)
            .unwrap()
            .do_dono(tipo, &Dono::Funcionalidade(fid(n)))
            .unwrap()
            .caminho
            .clone()
    }

    /// O agente: inicia e registra a versão inicial do caso de uso pelos campos
    /// do esquema, citando a regra `regra` da funcionalidade.
    fn agente_registra(&self, n: u32, regra: &str) {
        let alvo = json!({ "tipo": CASO, "funcionalidade": fid(n).to_string() });
        let roteiro = self.chamar("iniciar_versao_inicial", alvo).unwrap();
        assert!(roteiro.contains("registrar_versao_inicial"), "{roteiro}");
        self.chamar(
            "registrar_versao_inicial",
            json!({ "tipo": CASO, "funcionalidade": fid(n).to_string(),
                    "campos": caso_de_uso(n, regra) }),
        )
        .unwrap();
    }
}

/// Os campos de um caso de uso com uma etapa e uma exceção, ambas com a regra.
fn caso_de_uso(n: u32, regra: &str) -> Value {
    json!({
        "titulo": format!("Funcionalidade {n}"),
        "objetivo": "Atender o atendente.",
        "atores": ["Atendente"],
        "fluxo_principal": [format!("O atendente pede ({regra}).")],
        "excecoes": [format!("Na etapa 1, se a fatura está paga, o sistema recusa ({regra}).")],
        "regras": [regra],
    })
}

/// Liga uma regra nova à funcionalidade pelo vínculo da regra.
fn ligar_regra(amb: &Ambiente, id: &str, f: IdFuncionalidade) {
    let id = Id::analisar(id).unwrap();
    let regra = Documento::novo(
        FrontmatterRegra {
            tipo: TipoDocumento::Regra,
            id: id.clone(),
            titulo: format!("Regra de {f}"),
            area: "cobranca".into(),
            status: StatusRegra::Confirmada,
            evidencias: Vec::new(),
            origem: None,
            informada: None,
            historico: Vec::new(),
            funcionalidades: vec![f],
        },
        format!("# {id}\n\n## Enunciado\n\nRegra de exemplo.\n"),
    );
    amb.kb.gravar(&id.arquivo(), &regra).unwrap();
}

impl Cenario {}

#[test]
fn rup_mostra_as_colunas_vivas_com_o_estado_e_a_cobertura_bate_com_as_celulas() {
    let c = cenario(rup_com_manual());
    let t = tabela(&c.amb).unwrap();
    assert_eq!(t.metodologia, Metodologia::Rup);
    let colunas: Vec<&str> = t.colunas.iter().map(|c| c.tipo).collect();
    assert_eq!(colunas, [CASO, REGRAS, SUPLEMENTAR, MANUAL]);
    assert_eq!(t.linhas.len(), 3);
    assert!(t.linhas.iter().flat_map(|l| &l.documentos).all(|d| d.estado
        == EstadoDocumento::SemDocumento
        && d.acao == Some("Gerar versão inicial")));

    // F1: catálogo de regras publicado (em dia) e depois desatualizado; F2: o
    // catálogo em rascunho (proposta pendente); F3: sem documento.
    let lote = gerar_em_lote(&c.amb, REGRAS, &[fid(1).to_string(), fid(2).to_string()]).unwrap();
    assert_eq!(lote.geracao, Geracao::App);
    let caminho = c.caminho(REGRAS, 1);
    marcar_revisada(&c.amb, &caminho, "Ana Exemplo").unwrap();
    let p = publicar(&c.amb, &caminho).unwrap();
    assert_eq!(p.revisao.versao, Versao::INICIAL);
    assert_eq!(p.revisao.mudancas, MUDANCAS_DA_VERSAO_INICIAL);
    assert!(
        fs::read_to_string(c.amb.kb.caminho(&caminho))
            .unwrap()
            .contains(MUDANCAS_DA_VERSAO_INICIAL)
    );
    assert_eq!(
        c.estado(REGRAS, 1),
        EstadoDocumento::EmDia {
            versao: Versao::INICIAL
        }
    );
    assert!(matches!(
        c.estado(REGRAS, 2),
        EstadoDocumento::PropostaPendente { versao, revisada: false } if versao == Versao::INICIAL
    ));
    assert_eq!(c.estado(REGRAS, 3), EstadoDocumento::SemDocumento);

    // O arquivo da regra (e do ponto de entrada) muda: desatualizado.
    let sql = c.codigo.join(ARQUIVOS[0]);
    let mut texto = fs::read_to_string(&sql).unwrap();
    texto.push_str("\n-- mudança\nSELECT 1 FROM DUAL;\n");
    fs::write(&sql, texto).unwrap();
    assert!(matches!(
        c.estado(REGRAS, 1),
        EstadoDocumento::Desatualizado { ref arquivos, .. } if arquivos.contains(&ARQUIVOS[0].to_owned())
    ));

    // "Propor atualização" remonta: só o código mudou, o texto montado é o
    // mesmo e o app explica por quê.
    let erro = propor_atualizacao(&c.amb, REGRAS, &fid(1)).unwrap_err();
    assert!(erro.to_string().contains("ainda não chegou"), "{erro}");
    // Com uma regra nova no arquivo, propõe a 1.1, com o arquivo que mudou.
    gravar_regra(
        &c.amb,
        "RN-FAT-002",
        "Multa na reabertura",
        "A reabertura cobra multa",
    );
    let p = propor_atualizacao(&c.amb, REGRAS, &fid(1)).unwrap();
    assert!(p.proposta.texto.contains("RN-FAT-002"));
    assert_eq!(p.proposta.versao, Versao::nova(1, 1));
    assert!(
        p.proposta.mudancas.contains(ARQUIVOS[0]),
        "{}",
        p.proposta.mudancas
    );
    assert!(
        propor_atualizacao(&c.amb, REGRAS, &fid(1)).is_err(),
        "já há proposta"
    );
    assert!(
        propor_atualizacao(&c.amb, REGRAS, &fid(3)).is_err(),
        "sem documento"
    );
    assert!(
        propor_atualizacao(&c.amb, CASO, &fid(1)).is_err(),
        "redigido pelo agente"
    );
    use cba_tools::documentos_vivos::descartar_proposta;
    descartar_proposta(&c.amb, &caminho).unwrap();

    // A cobertura por tipo é a contagem das células.
    let t = tabela(&c.amb).unwrap();
    for (i, cob) in t.cobertura.iter().enumerate() {
        let celulas: Vec<&EstadoDocumento> =
            t.linhas.iter().map(|l| &l.documentos[i].estado).collect();
        let contar = |f: fn(&EstadoDocumento) -> bool| celulas.iter().filter(|e| f(e)).count();
        assert_eq!(cob.tipo, t.colunas[i].tipo);
        assert_eq!(cob.funcionalidades, 3);
        assert_eq!(
            cob.em_dia,
            contar(|e| matches!(e, EstadoDocumento::EmDia { .. }))
        );
        assert_eq!(
            cob.desatualizados,
            contar(|e| matches!(e, EstadoDocumento::Desatualizado { .. }))
        );
        assert_eq!(
            cob.propostas_pendentes,
            contar(|e| matches!(e, EstadoDocumento::PropostaPendente { .. }))
        );
        assert_eq!(
            cob.sem_documento,
            contar(|e| *e == EstadoDocumento::SemDocumento)
        );
        assert_eq!(cob.com_documento + cob.sem_documento, 3);
    }
    assert_eq!(
        t.cobertura[1],
        CoberturaDoTipo {
            tipo: REGRAS.into(),
            nome: "Catálogo de regras de negócio".into(),
            funcionalidades: 3,
            com_documento: 2,
            em_dia: 0,
            desatualizados: 1,
            propostas_pendentes: 1,
            sem_documento: 1,
        }
    );
    assert_eq!(t.cobertura[0].com_documento, 0);
}

#[test]
fn lote_de_casos_de_uso_deixa_tres_rascunhos_e_nenhum_e_publicado_sem_revisao() {
    let c = cenario(rup_com_manual());
    let todas: Vec<String> = (1..=3).map(|n| fid(n).to_string()).collect();
    let lote = gerar_em_lote(&c.amb, CASO, &todas).unwrap();
    // Redigido pelo agente: uma sessão por funcionalidade.
    assert_eq!(lote.geracao, Geracao::Agente);
    assert_eq!(lote.itens.len(), 3);
    for (item, f) in lote.itens.iter().zip(&todas) {
        match item {
            ItemDoLote::Sessao {
                funcionalidade,
                pedido,
            } => {
                assert_eq!(funcionalidade, f);
                assert!(pedido.contains("iniciar_versao_inicial"), "{pedido}");
                assert!(pedido.contains(f.as_str()), "{pedido}");
            }
            outro => panic!("esperava a sessão: {outro:?}"),
        }
    }
    // Ainda nada foi gerado: as sessões é que registram.
    assert_eq!(c.estado(CASO, 1), EstadoDocumento::SemDocumento);

    ligar_regra(&c.amb, "RN-FAT-002", fid(2));
    ligar_regra(&c.amb, "RN-FAT-003", fid(3));
    for n in 1..=3 {
        c.agente_registra(n, &format!("RN-FAT-00{n}"));
    }
    let docs = DocumentosVivos::carregar(&c.amb.kb).unwrap();
    for n in 1..=3 {
        let d = docs.do_dono(CASO, &Dono::Funcionalidade(fid(n))).unwrap();
        let p = d.proposta.as_ref().unwrap();
        assert_eq!(p.versao, Versao::INICIAL);
        assert_eq!(p.mudancas, MUDANCAS_DA_VERSAO_INICIAL);
        assert!(p.revisada.is_none());
        assert!(d.historico.is_empty(), "nada publicado");
        // Os arquivos dos pontos de entrada (e das regras) entram citados.
        assert!(p.arquivos.contains(&ARQUIVOS[n as usize - 1].to_owned()));
        // O texto é montado pelo app a partir dos campos.
        assert!(
            p.texto.starts_with(&format!("# Funcionalidade {n}")),
            "{}",
            p.texto
        );
        assert!(
            p.texto.contains("## Exceções\n\n- **FE1.** Na etapa 1"),
            "{}",
            p.texto
        );
        assert!(!c.amb.kb.caminho(&d.caminho).exists());
        assert!(matches!(
            c.estado(CASO, n),
            EstadoDocumento::PropostaPendente {
                revisada: false,
                ..
            }
        ));
        let erro = publicar(&c.amb, &d.caminho).unwrap_err();
        assert!(erro.to_string().contains("revisão final"), "{erro}");
    }
    let t = tabela(&c.amb).unwrap();
    assert_eq!(t.cobertura[0].com_documento, 3);
    assert_eq!(t.cobertura[0].propostas_pendentes, 3);
    assert_eq!(t.cobertura[0].em_dia, 0);

    // Gerar de novo não recria: as três ficam de fora.
    let de_novo = gerar_em_lote(&c.amb, CASO, &todas).unwrap();
    assert!(de_novo.itens.iter().all(
        |i| matches!(i, ItemDoLote::Pulada { motivo, .. } if motivo.contains("nunca é recriado"))
    ));
}

#[test]
fn tipos_do_app_saem_na_hora_iguais_em_duas_geracoes_e_com_a_origem_do_mapa() {
    let c = cenario(rup_com_manual());
    let ctx = contexto(&c.amb, SUPLEMENTAR, &fid(1)).unwrap();
    let texto = montar(&ctx);
    assert_eq!(
        montar(&contexto(&c.amb, SUPLEMENTAR, &fid(1)).unwrap()),
        texto
    );
    for trecho in [
        "# Especificação suplementar: Cancelar fatura",
        MUDANCAS_DA_VERSAO_INICIAL,
        "## Atores",
        "- Atendente",
        "## Pontos de entrada",
        "`PONTO_1`",
        "## Dados lidos",
        "- FATURA",
        "## Dados gravados",
        "**RN-FAT-001**: Uma fatura paga não pode ser cancelada.",
    ] {
        assert!(texto.contains(trecho), "{trecho}\n{texto}");
    }
    // A regra não tem evidência no arquivo da F2.
    assert!(
        montar(&contexto(&c.amb, SUPLEMENTAR, &fid(2)).unwrap())
            .contains("Nenhuma regra da base ligada a esta funcionalidade.")
    );

    let lote = gerar_em_lote(
        &c.amb,
        SUPLEMENTAR,
        &[
            fid(1).to_string(),
            fid(1).to_string(),
            "F-FAT-099".into(),
            "x".into(),
        ],
    )
    .unwrap();
    assert_eq!(
        lote.itens.len(),
        3,
        "a repetida conta uma vez: {:?}",
        lote.itens
    );
    assert!(
        matches!(&lote.itens[0], ItemDoLote::Gerada { funcionalidade, .. } if *funcionalidade == fid(1).to_string())
    );
    assert!(
        matches!(&lote.itens[1], ItemDoLote::Pulada { motivo, .. } if motivo.contains("não está no mapa"))
    );
    assert!(matches!(&lote.itens[2], ItemDoLote::Pulada { .. }));
    let docs = DocumentosVivos::carregar(&c.amb.kb).unwrap();
    let d = docs
        .do_dono(SUPLEMENTAR, &Dono::Funcionalidade(fid(1)))
        .unwrap();
    assert_eq!(d.proposta.as_ref().unwrap().texto, texto.trim());
    assert!(d.historico.is_empty());
}

#[test]
fn registro_recusa_regra_inexistente_documento_publicado_e_tipo_que_nao_e_da_funcionalidade() {
    let c = cenario(rup_com_manual());
    let f = fid(1).to_string();
    let erro = c
        .chamar(
            "registrar_versao_inicial",
            json!({ "tipo": CASO, "funcionalidade": f, "campos": caso_de_uso(1, "RN-FAT-042") }),
        )
        .unwrap_err();
    assert!(erro.to_string().contains("RN-FAT-042"), "{erro}");
    // O caso de uso vem pelos campos, cada etapa com a regra citada.
    let erro = c
        .chamar(
            "registrar_versao_inicial",
            json!({ "tipo": CASO, "funcionalidade": f, "texto": "# Caso\n\nPasso (RN-FAT-001)." }),
        )
        .unwrap_err();
    assert!(erro.to_string().contains("`campos`"), "{erro}");
    let mut sem_regra = caso_de_uso(1, "RN-FAT-001");
    sem_regra["fluxo_principal"] = json!(["O atendente pede (RN-FAT-001).", "O sistema cancela."]);
    let erro = c
        .chamar(
            "registrar_versao_inicial",
            json!({ "tipo": CASO, "funcionalidade": f, "campos": sem_regra }),
        )
        .unwrap_err();
    assert!(
        erro.to_string().contains("etapa 2 sem regra citada"),
        "{erro}"
    );
    // O manual continua pelo texto.
    let erro = c
        .chamar(
            "registrar_versao_inicial",
            json!({ "tipo": MANUAL, "funcionalidade": f }),
        )
        .unwrap_err();
    assert!(erro.to_string().contains("`texto`"), "{erro}");

    let erro = c
        .chamar(
            "iniciar_versao_inicial",
            json!({ "tipo": entregaveis::GLOSSARIO, "funcionalidade": f }),
        )
        .unwrap_err();
    assert!(matches!(erro, ErroFerramenta::Recusa(_)), "{erro}");
    assert!(
        gerar_em_lote(&c.amb, entregaveis::HISTORIA, std::slice::from_ref(&f)).is_err(),
        "a história é por demanda"
    );
    // O documento negocial não está habilitado no RUP.
    assert!(
        gerar_em_lote(
            &c.amb,
            entregaveis::DOCUMENTO_NEGOCIAL,
            std::slice::from_ref(&f)
        )
        .is_err()
    );

    // O roteiro traz as seções do tipo, o contexto e o template.
    let roteiro = c
        .chamar(
            "iniciar_versao_inicial",
            json!({ "tipo": CASO, "funcionalidade": f }),
        )
        .unwrap();
    for trecho in [
        "## Esquema (campos)",
        "`fluxo_principal` (Fluxo principal)",
        "`fluxos_alternativos` (Fluxos alternativos)",
        "`pos_condicoes` (Pós-condições)",
        "## Como redigir o caso de uso",
        "com os `campos` do esquema",
        "RN-FAT-001",
        "PONTO_1",
    ] {
        assert!(roteiro.contains(trecho), "{trecho}\n{roteiro}");
    }
    assert!(!roteiro.contains("{{"), "{roteiro}");
    // O manual continua pelas seções do template, sem o esquema.
    let manual = c
        .chamar(
            "iniciar_versao_inicial",
            json!({ "tipo": MANUAL, "funcionalidade": f }),
        )
        .unwrap();
    assert!(
        manual.contains("## Seções") && manual.contains("com o texto completo"),
        "{manual}"
    );
    assert!(!manual.contains("{{"), "{manual}");

    // Publicado, não se registra outra 1.0.
    c.agente_registra(1, "RN-FAT-001");
    let caminho = c.caminho(CASO, 1);
    marcar_revisada(&c.amb, &caminho, "Ana Exemplo").unwrap();
    publicar(&c.amb, &caminho).unwrap();
    let erro = c
        .chamar(
            "registrar_versao_inicial",
            json!({ "tipo": CASO, "funcionalidade": f, "campos": caso_de_uso(1, "RN-FAT-001") }),
        )
        .unwrap_err();
    assert!(erro.to_string().contains("nunca é recriado"), "{erro}");
    assert!(
        c.chamar(
            "iniciar_versao_inicial",
            json!({ "tipo": CASO, "funcionalidade": f })
        )
        .is_err()
    );
}
