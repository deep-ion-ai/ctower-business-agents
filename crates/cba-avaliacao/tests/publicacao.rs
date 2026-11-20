//! Ciclo de publicação da documentação (#279), sem modelo: a leitura da
//! transcrição, a forja falsa e, no Unix (git de verdade, como os testes de
//! escrita do `cba-repos`), as etapas aprovar, publicar, sincronizar numa base
//! nova e consultar a documentação publicada, com o sistema fictício Faturamento
//! Exemplo.

use cba_avaliacao::publicacao::{
    CHAMADAS_DA_FORJA, CicloAvaliado, UsoDaDocumentacao, chamado_como_forja_falsa, forja_falsa,
    saida_da_transcricao, uso_da_documentacao,
};
use cba_avaliacao::{gabarito::Gabarito, gabarito_padrao};
use serde_json::json;

/// Transcrição `stream-json` com as chamadas e respostas dadas.
fn transcricao(mensagens: &[serde_json::Value], resposta: &str) -> String {
    let mut linhas: Vec<String> = mensagens.iter().map(|m| m.to_string()).collect();
    linhas.push(
        json!({
            "type": "result", "subtype": "success", "is_error": false,
            "duration_ms": 1000, "num_turns": 4, "total_cost_usd": 0.12,
            "result": resposta
        })
        .to_string(),
    );
    linhas.join("\n")
}

fn chamada(id: &str, nome: &str, entrada: serde_json::Value) -> serde_json::Value {
    json!({ "type": "assistant", "message": { "content": [
        { "type": "text", "text": "Vou consultar." },
        { "type": "tool_use", "id": id, "name": nome, "input": entrada }
    ]}})
}

fn resposta(id: &str, texto: &str) -> serde_json::Value {
    json!({ "type": "user", "message": { "content": [
        { "type": "tool_result", "tool_use_id": id, "content": [{ "type": "text", "text": texto }] }
    ]}})
}

#[test]
fn transcricao_mostra_o_uso_da_documentacao_publicada() {
    let t = transcricao(
        &[
            chamada(
                "t1",
                "mcp__cba__conhecimento",
                json!({ "termo": "carência" }),
            ),
            resposta(
                "t1",
                "Base de conhecimento: 0 documento(s). A documentação publicada tem 2 documento(s)",
            ),
            chamada(
                "t2",
                "mcp__cba__conhecimento",
                json!({ "termo": "carência", "externos": "incluir" }),
            ),
            resposta(
                "t2",
                "- [doc. publicada] RN-GER-001 (regra; exemplo/faturamento-docs @ abc1234, confiança publicada): Carência",
            ),
            chamada(
                "t3",
                "Read",
                json!({ "file_path": "/tmp/base-b/faturamento-exemplo/externos/github-exemplo-faturamento-docs/regras/RN-GER-001.md" }),
            ),
            chamada(
                "t4",
                "Read",
                json!({ "file_path": "db/03_pkg_cobranca.sql" }),
            ),
        ],
        "Não paga juros. Fonte externa usada: exemplo/faturamento-docs @ abc1234.",
    );
    let uso = uso_da_documentacao(&t, "exemplo/faturamento-docs", "abc1234def");
    assert_eq!(
        uso,
        UsoDaDocumentacao {
            consultas: 2,
            consultas_com_externos: 1,
            respostas_com_externos: 1,
            leituras_de_externos: 1,
            fonte_citada: true,
        }
    );
    assert!(uso.usada());
    let s = saida_da_transcricao(&t);
    assert_eq!(s.custo_usd, Some(0.12));
    assert_eq!(s.turnos, Some(4));
}

#[test]
fn sem_pedir_externos_a_documentacao_nao_conta_como_usada() {
    // A contagem de `externos_disponiveis` e o contexto de iniciar_analise não
    // são uso: só o pedido com `externos` e um documento devolvido.
    let t = transcricao(
        &[
            chamada("t1", "mcp__cba__conhecimento", json!({ "termo": "juros" })),
            resposta(
                "t1",
                "A documentação publicada (repositório de docs) tem 3 documento(s)",
            ),
            chamada(
                "t2",
                "mcp__cba__conhecimento",
                json!({ "termo": "xyz", "externos": "somente" }),
            ),
            resposta("t2", "Nada na documentação publicada para \"xyz\"."),
        ],
        "Não paga juros nos 3 primeiros dias.",
    );
    let uso = uso_da_documentacao(&t, "exemplo/faturamento-docs", "abc1234def");
    assert_eq!(uso.consultas, 2);
    assert_eq!(uso.consultas_com_externos, 1);
    assert_eq!(uso.respostas_com_externos, 0);
    assert!(!uso.usada());
    assert!(!uso.fonte_citada);

    let ciclo = CicloAvaliado {
        primeira_analise: Some("A-0001".into()),
        aprovada: true,
        publicados: vec!["analises/A-0001.md".into()],
        commit: Some("abc1234def".into()),
        importados: 1,
        uso: Some(uso),
        ..Default::default()
    };
    let falhas = ciclo.falhas();
    assert_eq!(falhas.len(), 1, "{falhas:?}");
    assert!(falhas[0].contains("não consultou"), "{falhas:?}");
}

#[test]
fn ciclo_interrompido_aponta_a_etapa() {
    let nao_aprovada = CicloAvaliado {
        primeira_analise: Some("A-0001".into()),
        ..Default::default()
    };
    assert_eq!(
        nao_aprovada.falhas(),
        ["a primeira análise não foi aprovada"]
    );
    let nada_sincronizado = CicloAvaliado {
        primeira_analise: Some("A-0001".into()),
        aprovada: true,
        publicados: vec!["regras/RN-GER-001.md".into()],
        commit: Some("abc".into()),
        ..Default::default()
    };
    assert!(nada_sincronizado.falhas()[0].contains("sincronização"));
}

#[test]
fn forja_falsa_abre_e_procura_o_pr() {
    let dir = tempfile::tempdir().unwrap();
    let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let procurar = args(&[
        "api",
        "repos/exemplo/faturamento-docs/pulls?state=open&head=exemplo%3Adocs%2Fcba",
    ]);
    assert_eq!(forja_falsa(&procurar, dir.path()), ("[]".to_owned(), 0));
    let (saida, codigo) = forja_falsa(
        &args(&[
            "api",
            "--method",
            "POST",
            "repos/exemplo/faturamento-docs/pulls",
            "-f",
            "head=docs/cba-faturamento-exemplo",
        ]),
        dir.path(),
    );
    assert_eq!(codigo, 0);
    assert!(
        saida.contains("https://github.com/exemplo/faturamento-docs/pull/1"),
        "{saida}"
    );
    let (lista, _) = forja_falsa(&procurar, dir.path());
    assert!(lista.contains("pull/1"), "{lista}");
    assert_eq!(forja_falsa(&args(&["auth", "token"]), dir.path()).1, 3);
    let chamadas = std::fs::read_to_string(dir.path().join(CHAMADAS_DA_FORJA)).unwrap();
    assert_eq!(chamadas.lines().count(), 4);

    assert!(chamado_como_forja_falsa("/tmp/forja/gh"));
    assert!(chamado_como_forja_falsa("C:\\forja\\gh.exe"));
    assert!(!chamado_como_forja_falsa("target/debug/cba-avaliacao"));
}

#[test]
fn gabarito_tem_o_ciclo_de_publicacao() {
    let g = Gabarito::carregar(&gabarito_padrao()).unwrap();
    let p = g.publicacao("PUB-FAT-001").unwrap();
    assert_eq!(p.analise, "P-FAT-001");
    assert_eq!(p.reanalise, "P-FAT-003");
    g.item(&p.analise).unwrap();
    g.item(&p.reanalise).unwrap();
    assert!(g.publicacao("PUB-FAT-999").is_err());
}

#[cfg(unix)]
mod com_git {
    //! As etapas sem modelo com o git de verdade (sem a configuração global de
    //! quem roda), um repositório bare local e o executável da avaliação como
    //! forja falsa.
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};

    use cba_avaliacao::gabarito::Gabarito;
    use cba_avaliacao::gabarito_padrao;
    use cba_avaliacao::publicacao::{
        CHAMADAS_DA_FORJA, RemotoDeDocs, aprovar_como_o_analista, destino, ferramentas,
        instalar_forja_falsa, publicados, publicar, sincronizar, uso_da_documentacao,
    };
    use cba_core::ambiente::Ambiente;
    use cba_core::analise::{Confianca, NovaAfirmacao};
    use cba_core::analises::NovaAnalise;
    use cba_core::config::Config;
    use cba_core::dados::DiretorioDados;
    use cba_core::eventos::Mudanca;
    use cba_repos::Repositorios;
    use cba_tools::evidencias::{EvidenciaCitada, validar};
    use cba_tools::publicacao::branch_da_publicacao;
    use cba_tools::registro_padrao;
    use serde_json::json;

    fn git_sem_config_global(raiz: &Path) -> PathBuf {
        let bin = raiz.join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let p = bin.join("git");
        std::fs::write(
            &p,
            "#!/bin/sh\nGIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 exec git \"$@\"\n",
        )
        .unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p
    }

    /// Projeto do Faturamento Exemplo com dados e base próprios em `raiz`.
    fn ambiente(raiz: &Path, dados: &str, base: &str, repositorios: Repositorios) -> Ambiente {
        let g = Gabarito::carregar(&gabarito_padrao()).unwrap();
        let codigo = dunce::canonicalize(g.raiz_do_projeto()).unwrap();
        let dados = DiretorioDados::em(raiz.join(dados));
        let mut cadastro = cba_index::Projeto::novo("Faturamento Exemplo", [&codigo]).unwrap();
        cadastro.repositorios = repositorios;
        cba_index::Cadastro::em(dados.projetos())
            .cadastrar(&cadastro)
            .unwrap();
        cba_kb::Base::criar(raiz.join(base)).unwrap();
        Config {
            base: Some(raiz.join(base)),
            ..Default::default()
        }
        .gravar(&dados)
        .unwrap();
        Ambiente::do_diretorio(&codigo, &dados).unwrap()
    }

    /// A primeira análise como o agente a deixaria: P-FAT-001, com uma afirmação
    /// `confirmado` por regra do gabarito (as evidências esperadas, conferidas
    /// como no `afirmar`), uma `inferido` e concluída.
    fn analisar(amb: &Ambiente, g: &Gabarito) -> cba_kb::Id {
        let item = g.item("P-FAT-001").unwrap();
        let analise = amb
            .analises
            .criar(NovaAnalise {
                fluxo: item.fluxo,
                titulo: item.titulo.clone(),
                entrada: item.entrada.clone(),
                analista: None,
                base: None,
            })
            .unwrap();
        let mut novas = Vec::new();
        for id in ["RN-FAT-001", "RN-FAT-003"] {
            let regra = g.regras.iter().find(|r| r.id == id).unwrap();
            let evidencias = regra
                .evidencias
                .iter()
                .map(|e| {
                    validar(
                        &amb.escopo,
                        &EvidenciaCitada {
                            arquivo: e.arquivo.clone(),
                            linha_inicial: e.linha_inicial,
                            linha_final: e.linha_final,
                            trecho: e.trecho.clone(),
                        },
                    )
                    .unwrap()
                })
                .collect();
            novas.push(NovaAfirmacao {
                secao: "regras".into(),
                texto: format!("{}: {}", regra.titulo, regra.enunciado.trim()),
                confianca: Confianca::Confirmado,
                evidencias,
                sessao: None,
            });
        }
        novas.push(NovaAfirmacao {
            secao: "regras".into(),
            texto: "O lote de cobrança parece calcular a multa com valor fixo.".into(),
            confianca: Confianca::Inferido,
            evidencias: Vec::new(),
            sessao: None,
        });
        amb.analises
            .alterar(&analise.id, Mudanca::Estado, |a, agora| {
                a.iniciar()?;
                for n in novas {
                    a.registrar_afirmacao(n, agora)?;
                }
                a.concluir()
            })
            .unwrap();
        analise.id
    }

    #[test]
    fn analisar_aprovar_publicar_sincronizar_e_consultar() {
        let tmp = tempfile::tempdir().unwrap();
        let raiz = dunce::canonicalize(tmp.path()).unwrap();
        let git = git_sem_config_global(&raiz);
        let g = Gabarito::carregar(&gabarito_padrao()).unwrap();
        let esperada = g.publicacao("PUB-FAT-001").unwrap();

        // Analisar e aprovar na base A.
        let a = ambiente(&raiz, "dados", "base", Repositorios::default());
        let id = analisar(&a, &g);
        let aprovacao = aprovar_como_o_analista(&a.dados, &a, &id).unwrap();
        assert_eq!(
            aprovacao.regras.len(),
            2,
            "só as afirmações confirmado viram regra"
        );
        assert_eq!(
            aprovacao.analise.estado,
            cba_core::analise::EstadoAnalise::Aprovada
        );
        // Aprovar de novo não muda nada (idempotente).
        aprovar_como_o_analista(&a.dados, &a, &id).unwrap();

        // Publicar no repositório de docs local, com o PR na forja falsa.
        let forja = instalar_forja_falsa(
            Path::new(env!("CARGO_BIN_EXE_cba-avaliacao")),
            &raiz.join("forja"),
        )
        .unwrap();
        let remoto = RemotoDeDocs::criar(&git, &raiz).unwrap();
        let clone_a = remoto.clonar(&raiz, "docs-a").unwrap();
        let f = ferramentas(&git, &forja);
        let p = publicar(&a, &destino(esperada, &remoto, &clone_a), &f).unwrap();
        assert_eq!(
            p.publicado.link.as_deref(),
            Some("https://github.com/exemplo/faturamento-docs/pull/1")
        );
        assert_eq!(
            p.publicado.branch,
            branch_da_publicacao("faturamento-exemplo")
        );
        let novos = publicados(&p);
        assert!(novos.contains(&format!("analises/{id}.md")), "{novos:?}");
        assert_eq!(
            novos.iter().filter(|c| c.starts_with("regras/")).count(),
            2,
            "{novos:?}"
        );
        let chamadas = std::fs::read_to_string(raiz.join("forja").join(CHAMADAS_DA_FORJA)).unwrap();
        assert!(chamadas.contains("--method POST"), "{chamadas}");

        // O PR é mesclado; a base do repositório de docs tem a documentação, o
        // manifesto e o leia-me escrito à mão.
        let commit = remoto.mesclar(&p.publicado.branch, &forja).unwrap();
        let arquivos = remoto.arquivos().unwrap();
        for esperado in [
            ".cba-publicacao.json",
            "leia-me.md",
            &format!("analises/{id}.md"),
        ] {
            assert!(arquivos.iter().any(|a| a == esperado), "{arquivos:?}");
        }

        // Base nova, outro clone: a sincronização traz a documentação publicada.
        let clone_b = remoto.clonar(&raiz, "docs-b").unwrap();
        let b = ambiente(
            &raiz,
            "dados-b",
            "base-b",
            destino(esperada, &remoto, &clone_b),
        );
        let s = sincronizar(&b, &f).unwrap();
        assert_eq!(s.fonte.commit, commit);
        // Os documentos publicados e o leia-me; o manifesto não é `.md`.
        assert_eq!(s.importacao.novos, novos.len() + 1, "{:?}", s.importacao);

        // O agente consulta a documentação publicada como faria na segunda
        // análise; a resposta, numa transcrição, conta como uso.
        let saida = registro_padrao()
            .chamar(
                "conhecimento",
                Ok(&b),
                Some(json!({ "termo": "carência", "externos": "incluir" })),
            )
            .unwrap();
        assert!(saida.texto.contains("[doc. publicada]"), "{}", saida.texto);
        assert!(
            saida.texto.contains("exemplo/faturamento-docs"),
            "{}",
            saida.texto
        );
        let transcricao = [
            json!({ "type": "assistant", "message": { "content": [{ "type": "tool_use", "id": "t1",
                "name": "mcp__cba__conhecimento", "input": { "termo": "carência", "externos": "incluir" } }] } }),
            json!({ "type": "user", "message": { "content": [{ "type": "tool_result", "tool_use_id": "t1",
                "content": [{ "type": "text", "text": saida.texto }] }] } }),
            json!({ "type": "result", "result": format!("Fonte: faturamento-docs @ {}", &commit[..7]) }),
        ]
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join("\n");
        let uso = uso_da_documentacao(&transcricao, &esperada.repositorio, &commit);
        assert!(uso.usada() && uso.fonte_citada, "{uso:?}");

        // Na base A, que publicou, a sincronização não copia o que ela já tem.
        let mut a2 = a.clone();
        a2.projeto.repositorios = destino(esperada, &remoto, &clone_a);
        let s = sincronizar(&a2, &f).unwrap();
        assert_eq!(s.importacao.novos, 1, "só o leia-me: {:?}", s.importacao);
    }
}
