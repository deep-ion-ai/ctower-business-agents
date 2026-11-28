//! Testes da migração para o catálogo de entregáveis (#465), com uma base de
//! exemplo: o "Faturamento Exemplo" com histórias e cenários gravados e um
//! documento negocial de antes do histórico (mapa no formato 1).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use cba_core::analise::Fluxo;
use cba_core::documentos_vivos::{DocumentosVivos, Dono, Versao};
use cba_core::funcionalidades::TIPO_DOCUMENTO_NEGOCIAL;
use cba_docs::entregaveis::{self, CASOS_DE_TESTE, HISTORIA};
use cba_index::Metodologia;
use cba_kb::{CaminhoRelativo, IdFuncionalidade, layout};
use cba_tools::documentos_vivos::MUDANCAS_DO_IMPORTADO;
use cba_tools::revisao::{Formato, OpcoesExportacao};
use serde_json::json;

use super::*;
use crate::analise;
use crate::configuracoes::{fluxos_ocultos, salvar_entregaveis};
use crate::revisao::{PedidoExportacao, exportar};
use crate::teste_base::{Cenario, PKG, PROJETO, cenario};

const NEGOCIAL: &str = "funcionalidades/cobranca/cobrar-multa/negocial.md";
const TEXTO_NEGOCIAL: &str = "---\ntipo: funcionalidade\ntitulo: Cobrar multa\narea: cobranca\ntema: cobrar-multa\nstatus: aprovada\nfuncionalidade: F-COB-001\n---\n\n# Cobrar multa\n\nPassada a carência, a multa é de 2% sobre o valor original.\n";

fn kb(c: &Cenario) -> cba_kb::Projeto {
    c.ambiente().kb
}

fn fid() -> IdFuncionalidade {
    IdFuncionalidade::analisar("F-COB-001").unwrap()
}

/// O mapa como uma versão anterior gravava (formato 1, com o `documento` único)
/// e o documento negocial aprovado pelo "Explicar", sem histórico.
fn negocial_de_antes(c: &Cenario) {
    let kb = kb(c);
    let ev = json!({"arquivo": PKG, "linhas": "71-72", "trecho": "X", "hash": "h"});
    let mapa = json!({
        "formato": 1,
        "funcionalidades": [{
            "id": "F-COB-001", "nome": "Cobrar multa", "area": "cobranca",
            "area_nome": "Cobrança", "descricao": "Multa por atraso.",
            "pontos": [{"chave": "db/03_pkg_cobranca.sql#MULTA", "nome": "Multa", "evidencia": ev}],
            "situacao": "aceita", "registrada_ms": 1,
            "documento": NEGOCIAL, "explicacao": "A-0009"
        }, {
            "id": "F-COB-002", "nome": "Parcelar", "area": "cobranca",
            "area_nome": "Cobrança", "descricao": "Sem documento.",
            "pontos": [{"chave": "db/03_pkg_cobranca.sql#PARC", "nome": "Parcelar", "evidencia": ev}],
            "registrada_ms": 1
        }],
        "versao": 3
    });
    let destino = kb.caminho(&layout::estado_do_mapa());
    fs::create_dir_all(destino.parent().unwrap()).unwrap();
    fs::write(&destino, mapa.to_string()).unwrap();
    let negocial = kb.caminho(&CaminhoRelativo::novo(NEGOCIAL).unwrap());
    fs::create_dir_all(negocial.parent().unwrap()).unwrap();
    fs::write(&negocial, TEXTO_NEGOCIAL).unwrap();
}

/// Uma pergunta com uma afirmação, as histórias dela (A-0002, com uma história)
/// e os cenários das histórias (A-0003).
fn historias_e_cenarios(c: &Cenario) {
    analise::criar_analise(&c.dados, PROJETO, "Como é calculada a multa?", None).unwrap();
    c.chamar("iniciar_analise", json!({"analise": "A-0001"}));
    c.chamar(
        "afirmar",
        json!({
            "analise": "A-0001",
            "secao": "resposta",
            "texto": "Passada a carência, a multa é de 2% sobre o valor original.",
            "confianca": "confirmado",
            "evidencias": [{
                "arquivo": PKG, "linha_inicial": 71, "linha_final": 72,
                "trecho": "FN_PARAMETRO('PERC_MULTA_ATRASO') / 100, 2)"
            }]
        }),
    );
    let h =
        analise::criar_analise_de_historias(&c.dados, PROJETO, Some("A-0001"), None, None, None)
            .unwrap();
    assert_eq!(h.analise.id, "A-0002");
    c.chamar(
        "registrar_historia",
        json!({
            "analise": "A-0002",
            "titulo": "Cobrar a multa depois da carência",
            "como": "atendente", "quero": "ver a multa", "para": "cobrar certo",
            "criterios": [{
                "dado": "uma fatura vencida há 10 dias", "quando": "o atendente consulta",
                "entao": "a multa é de 2%", "afirmacoes": [1]
            }]
        }),
    );
    let cen =
        analise::criar_analise_de_cenarios(&c.dados, PROJETO, Some("A-0002"), None, None, None)
            .unwrap();
    assert_eq!(cen.analise.id, "A-0003");
}

/// Todos os arquivos de uma pasta, com o conteúdo.
fn arquivos(raiz: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn juntar(raiz: &Path, pasta: &Path, saida: &mut BTreeMap<PathBuf, Vec<u8>>) {
        let Ok(itens) = fs::read_dir(pasta) else {
            return;
        };
        for e in itens.flatten() {
            let p = e.path();
            if p.is_dir() {
                juntar(raiz, &p, saida);
            } else {
                saida.insert(
                    p.strip_prefix(raiz).unwrap().to_owned(),
                    fs::read(&p).unwrap(),
                );
            }
        }
    }
    let mut saida = BTreeMap::new();
    juntar(raiz, raiz, &mut saida);
    saida
}

fn exportar_md(c: &Cenario, analise: &str, nome: &str) -> String {
    let destino = c.dir.path().join("exportados").join(nome);
    fs::create_dir_all(destino.parent().unwrap()).unwrap();
    let gravado = exportar(
        &c.dados,
        PROJETO,
        analise,
        &PedidoExportacao {
            destino,
            formato: Formato::Markdown,
            opcoes: OpcoesExportacao::default(),
            figuras: Vec::new(),
        },
    )
    .unwrap();
    fs::read_to_string(gravado).unwrap()
}

fn rup(c: &Cenario) {
    let config = entregaveis::aplicar(&entregaveis::efetiva(None), Metodologia::Rup);
    salvar_entregaveis(&c.dados, PROJETO, config).unwrap();
}

#[test]
fn projeto_com_historias_cenarios_e_negocial_migra_sem_perda_e_a_segunda_vez_nao_muda_nada() {
    let c = cenario();
    historias_e_cenarios(&c);
    negocial_de_antes(&c);
    let base = c.dir.path().join("base");
    let analises: Vec<_> = ["A-0002", "A-0003"]
        .iter()
        .map(|a| {
            let vista = analise::carregar_analise(&c.dados, PROJETO, a).unwrap();
            (
                fs::read(&vista.documento).unwrap(),
                exportar_md(&c, a, &format!("{a}-antes.md")),
            )
        })
        .collect();
    // O projeto passa a RUP, sem histórias nem casos de teste.
    rup(&c);
    let antes = arquivos(&base);

    let r = migrar(&c.dados).unwrap();
    assert!(r.pulados.is_empty(), "{:?}", r.pulados);
    assert_eq!(r.importados.len(), 1);
    assert_eq!(r.importados[0].0, PROJETO);
    assert_eq!(r.importados[0].1.len(), 1);
    assert_eq!(r.importados[0].1[0].funcionalidade, fid());

    // O negocial vira a v1.0 do documento vivo, com o texto do arquivo.
    let docs = DocumentosVivos::carregar(&kb(&c)).unwrap();
    let vivo = docs
        .do_dono(TIPO_DOCUMENTO_NEGOCIAL, &Dono::Funcionalidade(fid()))
        .unwrap();
    assert_eq!(vivo.caminho.as_str(), NEGOCIAL);
    assert_eq!(vivo.historico.len(), 1);
    let v1 = &vivo.historico[0];
    assert_eq!(v1.versao, Versao::INICIAL);
    assert_eq!(v1.mudancas, MUDANCAS_DO_IMPORTADO);
    assert_eq!(
        v1.analise.as_ref().map(ToString::to_string).as_deref(),
        Some("A-0009")
    );
    assert_eq!(
        v1.texto,
        "# Cobrar multa\n\nPassada a carência, a multa é de 2% sobre o valor original."
    );
    assert!(vivo.proposta.is_none());

    // Na base, só o estado dos documentos vivos é novo: o negocial, o mapa e as
    // análises ficam iguais, byte a byte.
    let depois = arquivos(&base);
    let estado = PathBuf::from(PROJETO).join(layout::estado_dos_documentos_vivos().as_str());
    let mudados: Vec<_> = depois
        .iter()
        .filter(|(p, b)| antes.get(*p) != Some(*b))
        .map(|(p, _)| p.clone())
        .collect();
    assert!(
        mudados
            .iter()
            .all(|p| *p == estado || p.to_string_lossy().ends_with(".trava")),
        "{mudados:?}"
    );
    assert!(antes.keys().all(|p| depois.contains_key(p)), "nada some");
    assert_eq!(
        fs::read_to_string(base.join(PROJETO).join(NEGOCIAL)).unwrap(),
        TEXTO_NEGOCIAL
    );

    // O backup ficou nos dados do app, fora da base: o estado não existia.
    let backup = r.backup.clone().unwrap();
    assert!(backup.starts_with(c.dados.raiz()), "{}", backup.display());
    assert_eq!(
        fs::read(backup.join(PROJETO).join("documentos.estado.json")).unwrap(),
        AUSENTE
    );

    // As histórias e os cenários gravados abrem e exportam iguais, mesmo sem
    // os tipos habilitados.
    for ((documento, exportado), a) in analises.iter().zip(["A-0002", "A-0003"]) {
        let vista = analise::carregar_analise(&c.dados, PROJETO, a).unwrap();
        assert_eq!(&fs::read(&vista.documento).unwrap(), documento, "{a}");
        assert_eq!(
            &exportar_md(&c, a, &format!("{a}-depois.md")),
            exportado,
            "{a}"
        );
    }
    let historias = analise::carregar_analise(&c.dados, PROJETO, "A-0002").unwrap();
    assert_eq!(historias.analise.fluxo, Fluxo::Historias);
    assert_eq!(historias.analise.historias.len(), 1);

    // A segunda vez não muda nada (nem grava outro backup).
    let backups = arquivos(&c.dados.raiz().join("backups"));
    let antes = arquivos(&base);
    let r = migrar(&c.dados).unwrap();
    assert_eq!(r, ResumoMigracao::default());
    assert_eq!(arquivos(&base), antes);
    assert_eq!(arquivos(&c.dados.raiz().join("backups")), backups);
}

#[test]
fn projeto_rup_sem_historias_nao_oferece_historias_nem_cenarios() {
    let c = cenario();
    // O projeto sem configuração (legado) oferece os dois.
    assert!(fluxos_ocultos(&c.dados, PROJETO).unwrap().is_empty());
    historias_e_cenarios(&c);

    rup(&c);
    assert_eq!(
        fluxos_ocultos(&c.dados, PROJETO).unwrap(),
        [Fluxo::Historias, Fluxo::Cenarios]
    );
    let json = serde_json::to_value(fluxos_ocultos(&c.dados, PROJETO).unwrap()).unwrap();
    assert_eq!(json, json!(["historias", "cenarios"]));
    let erro =
        analise::criar_analise_de_historias(&c.dados, PROJETO, Some("A-0001"), None, None, None)
            .unwrap_err();
    assert!(
        erro.contains("Histórias") && erro.contains("Entregáveis"),
        "{erro}"
    );
    assert!(
        analise::criar_analise_de_cenarios(&c.dados, PROJETO, Some("A-0001"), None, None, None)
            .is_err()
    );
    // Nem pelo caminho genérico de criar, nem pelo agente.
    assert!(
        analise::criar_analise_com_demanda(&c.dados, PROJETO, Fluxo::Historias, "x", None, None)
            .is_err()
    );
    let pelo_agente = cba_tools::registro_padrao().chamar(
        "iniciar_analise",
        Ok(&c.ambiente()),
        Some(json!({"fluxo": "historias", "origem": "A-0001"})),
    );
    assert!(pelo_agente.is_err());
    // Os outros fluxos continuam.
    analise::criar_analise(&c.dados, PROJETO, "Qual o teto de encargos?", None).unwrap();

    // Com os casos de teste habilitados, cenários volta; histórias, não.
    let mut config = entregaveis::aplicar(&entregaveis::efetiva(None), Metodologia::Rup);
    config
        .tipos
        .push(cba_index::TipoHabilitado::novo(CASOS_DE_TESTE));
    salvar_entregaveis(&c.dados, PROJETO, config.clone()).unwrap();
    assert_eq!(
        fluxos_ocultos(&c.dados, PROJETO).unwrap(),
        [Fluxo::Historias]
    );
    analise::criar_analise_de_cenarios(&c.dados, PROJETO, Some("A-0001"), None, None, None)
        .unwrap();
    config.tipos.push(cba_index::TipoHabilitado::novo(HISTORIA));
    salvar_entregaveis(&c.dados, PROJETO, config).unwrap();
    assert!(fluxos_ocultos(&c.dados, PROJETO).unwrap().is_empty());
}

#[test]
fn falha_no_meio_volta_ao_backup() {
    let c = cenario();
    negocial_de_antes(&c);
    // Um estado dos documentos vivos que já existia, com outro documento.
    DocumentosVivos::alterar(&kb(&c), |docs, agora| {
        docs.garantir(
            "manual",
            &Dono::Funcionalidade(IdFuncionalidade::analisar("F-COB-002").unwrap()),
            &CaminhoRelativo::novo("funcionalidades/cobranca/parcelar/manual.md").unwrap(),
            agora,
        )
        .map(|_| ())
        .map_err(|e| cba_tools::ErroFerramenta::Recusa(e.to_string()))
    })
    .unwrap();
    let estado = kb(&c).caminho(&layout::estado_dos_documentos_vivos());
    let original = fs::read(&estado).unwrap();

    let erro = migrar_com(&c.dados, |kb| {
        cba_tools::documentos_vivos::importar_negociais(kb)?;
        Err("disco cheio".into())
    })
    .unwrap_err();
    assert!(
        erro.contains("desfeita") && erro.contains("disco cheio"),
        "{erro}"
    );
    assert_eq!(fs::read(&estado).unwrap(), original);

    // Depois, a migração de verdade passa.
    let r = migrar(&c.dados).unwrap();
    assert_eq!(r.importados[0].1.len(), 1);
    let docs = DocumentosVivos::carregar(&kb(&c)).unwrap();
    assert_eq!(docs.documentos.len(), 2);
}

#[test]
fn sem_base_ou_sem_negocial_nao_grava_nada() {
    let dir = tempfile::tempdir().unwrap();
    let dados = cba_core::dados::DiretorioDados::em(dir.path().join("dados"));
    assert_eq!(migrar(&dados).unwrap(), ResumoMigracao::default());

    let c = cenario();
    let r = migrar(&c.dados).unwrap();
    assert_eq!(r, ResumoMigracao::default());
    assert!(!c.dados.raiz().join("backups").exists());
    assert!(
        !kb(&c)
            .caminho(&layout::estado_dos_documentos_vivos())
            .exists()
    );
}
