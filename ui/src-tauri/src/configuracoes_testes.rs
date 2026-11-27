//! Testes das configurações com a fixture "Faturamento Exemplo" (cópia numa pasta
//! temporária, [`crate::teste_base`]) e a configuração do Claude num
//! `CLAUDE_CONFIG_DIR` temporário: nada aqui toca a configuração real do usuário.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use cba_core::config::Config;
use cba_session::clientes::{Ambiente, ErroExecucao, Plataforma, Saida};
use cba_session::preparo::{Preparador, Situacao};

use super::*;
use crate::teste_base::{PROJETO, cenario};
use crate::{alteracoes, analise};

/// Conteúdo de todos os arquivos de uma pasta, pelo caminho relativo.
fn conteudo(raiz: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let a = arvore(raiz).unwrap();
    a.arquivos
        .keys()
        .map(|r| (r.clone(), fs::read(raiz.join(r)).unwrap()))
        .collect()
}

fn base_atual(dados: &DiretorioDados) -> PathBuf {
    Config::ler(dados).unwrap().base.unwrap()
}

fn sem_restricoes() -> Restricoes {
    Restricoes::default()
}

#[test]
fn mover_copia_confere_apaga_a_origem_e_reconstroi_o_indice() {
    let c = cenario();
    c.aprovar_analise_da_multa();
    let origem = base_atual(&c.dados);
    let antes = conteudo(&origem);
    assert!(
        antes
            .keys()
            .any(|p| p.starts_with("faturamento-exemplo/regras"))
    );
    let destino = c.dir.path().join("documentos").join("base-nova");

    let avaliacao = avaliar_troca(&c.dados, &sem_restricoes(), destino.to_str().unwrap()).unwrap();
    assert!(
        avaliacao.recusa_mover.is_none(),
        "{:?}",
        avaliacao.recusa_mover
    );
    assert_eq!(avaliacao.arquivos, antes.len());
    assert!(!destino.exists(), "avaliar não cria nada");

    let feito = trocar_base(
        &c.dados,
        &sem_restricoes(),
        destino.to_str().unwrap(),
        ModoTroca::Mover,
    )
    .unwrap();
    assert!(feito.avisos.is_empty(), "{:?}", feito.avisos);
    assert_eq!(feito.copiados, antes.len());
    assert!(feito.origem_apagada);
    assert!(!origem.exists());
    let nova = base_atual(&c.dados);
    assert_eq!(nova, feito.base);
    assert_eq!(
        conteudo(&nova),
        antes,
        "nenhum documento perdido ou alterado"
    );

    // O índice foi reconstruído a partir da base nova.
    assert!(feito.indexados.unwrap() > 0);
    let busca = crate::base::buscar(&c.dados, "carência", None, &[], 0).unwrap();
    assert!(busca.total > 0);
    // A análise continua acessível pelo app.
    assert_eq!(
        analise::listar_analises(&c.dados, PROJETO).unwrap().len(),
        1
    );
}

#[test]
fn reapontar_mantem_os_documentos_da_pasta_antiga() {
    let c = cenario();
    c.aprovar_analise_da_multa();
    let origem = base_atual(&c.dados);
    let antes = conteudo(&origem);
    // Uma pasta que já tem outros arquivos: só dá para reapontar.
    let outra = c.dir.path().join("sincronizada");
    fs::create_dir_all(&outra).unwrap();
    fs::write(outra.join("notas.txt"), "anotações").unwrap();
    let avaliacao = avaliar_troca(&c.dados, &sem_restricoes(), outra.to_str().unwrap()).unwrap();
    assert!(avaliacao.recusa_mover.is_some());
    assert!(avaliacao.recusa_reapontar.is_none());
    assert!(
        trocar_base(
            &c.dados,
            &sem_restricoes(),
            outra.to_str().unwrap(),
            ModoTroca::Mover
        )
        .is_err()
    );
    assert_eq!(base_atual(&c.dados), origem);

    let feito = trocar_base(
        &c.dados,
        &sem_restricoes(),
        outra.to_str().unwrap(),
        ModoTroca::Reapontar,
    )
    .unwrap();
    assert!(!feito.origem_apagada);
    assert_eq!(feito.avisos.len(), 1, "avisa onde ficaram os documentos");
    assert_eq!(conteudo(&origem), antes, "a pasta antiga fica intacta");
    assert_eq!(
        fs::read_to_string(outra.join("notas.txt")).unwrap(),
        "anotações"
    );
    assert_eq!(base_atual(&c.dados), dunce::canonicalize(&outra).unwrap());
    // O índice agora é o da base nova (sem a regra da multa).
    assert_eq!(feito.indexados, Some(0));
    let busca = crate::base::buscar(&c.dados, "carência", None, &[], 0).unwrap();
    assert_eq!(busca.total, 0);

    // E reapontar de volta encontra tudo de novo.
    let volta = trocar_base(
        &c.dados,
        &sem_restricoes(),
        origem.to_str().unwrap(),
        ModoTroca::Reapontar,
    )
    .unwrap();
    assert!(volta.indexados.unwrap() > 0);
}

#[test]
fn troca_recusa_a_mesma_pasta_e_pastas_aninhadas() {
    let c = cenario();
    let origem = base_atual(&c.dados);
    let r = sem_restricoes();
    for destino in [
        origem.clone(),
        origem.join("sub"),
        c.dir.path().to_path_buf(),
    ] {
        let a = avaliar_troca(&c.dados, &r, destino.to_str().unwrap()).unwrap();
        assert!(a.recusa_mover.is_some(), "{}", destino.display());
        assert!(a.recusa_reapontar.is_some(), "{}", destino.display());
        assert!(
            trocar_base(
                &c.dados,
                &r,
                destino.to_str().unwrap(),
                ModoTroca::Reapontar
            )
            .is_err()
        );
    }
    assert_eq!(base_atual(&c.dados), origem);
}

#[cfg(unix)]
#[test]
fn link_simbolico_na_base_impede_mover_sem_tocar_em_nada() {
    let c = cenario();
    let origem = base_atual(&c.dados);
    std::os::unix::fs::symlink(c.dir.path(), origem.join("atalho")).unwrap();
    let destino = c.dir.path().join("base-nova");
    let a = avaliar_troca(&c.dados, &sem_restricoes(), destino.to_str().unwrap()).unwrap();
    assert!(a.recusa_mover.unwrap().contains("link simbólico"));
    assert!(
        trocar_base(
            &c.dados,
            &sem_restricoes(),
            destino.to_str().unwrap(),
            ModoTroca::Mover
        )
        .is_err()
    );
    assert!(!destino.exists());
    assert_eq!(base_atual(&c.dados), origem);
}

#[test]
fn copia_que_nao_confere_e_desfeita() {
    let dir = tempfile::tempdir().unwrap();
    let origem = dir.path().join("origem");
    fs::create_dir_all(origem.join("a")).unwrap();
    fs::write(origem.join("a/x.md"), "um").unwrap();
    let a = arvore(&origem).unwrap();
    // Um arquivo que some depois do retrato faz a cópia falhar; o destino é limpo.
    fs::remove_file(origem.join("a/x.md")).unwrap();
    let destino = dir.path().join("destino");
    assert!(copiar_e_conferir(&origem, &destino, &a).is_err());
    assert!(!destino.exists());
    // Conteúdo diferente com o mesmo tamanho é detectado.
    fs::write(dir.path().join("p"), "abc").unwrap();
    fs::write(dir.path().join("q"), "abd").unwrap();
    assert!(!mesmo_conteudo(&dir.path().join("p"), &dir.path().join("q")).unwrap());
    assert!(mesmo_conteudo(&dir.path().join("p"), &dir.path().join("p")).unwrap());
}

#[test]
fn exclusoes_com_previa_e_gravacao() {
    let c = cenario();
    let ajuste = AjusteLeitura {
        projeto: PROJETO.into(),
        pastas: vec![c.codigo.to_str().unwrap().into()],
        exclusoes: vec!["cobol/".into(), " ".into()],
        inclusoes: vec![],
    };
    let previa = previa_leitura(&c.dados, &ajuste).unwrap();
    assert_eq!(previa.exclusoes, ["cobol/"]);
    assert!(
        !previa
            .inventario
            .por_linguagem
            .iter()
            .any(|l| l.chave == "cobol")
    );
    // A prévia não grava.
    let cadastro = Cadastro::em(c.dados.projetos());
    assert!(
        cadastro
            .obter(PROJETO)
            .unwrap()
            .unwrap()
            .exclusoes
            .is_empty()
    );

    let feito = salvar_leitura(&c.dados, &ajuste).unwrap();
    assert!(
        !feito
            .inventario
            .por_linguagem
            .iter()
            .any(|l| l.chave == "cobol")
    );
    assert_eq!(
        cadastro.obter(PROJETO).unwrap().unwrap().exclusoes,
        ["cobol/"]
    );
    let gravado = cadastro.inventario(PROJETO).unwrap().unwrap();
    assert!(
        !gravado
            .resumo()
            .por_linguagem
            .keys()
            .any(|l| l.nome() == "COBOL")
    );

    let invalido = AjusteLeitura {
        exclusoes: vec!["[".into()],
        ..ajuste.clone()
    };
    assert!(salvar_leitura(&c.dados, &invalido).is_err());
    let desconhecido = AjusteLeitura {
        projeto: "outro".into(),
        ..ajuste
    };
    assert!(previa_leitura(&c.dados, &desconhecido).is_err());
}

#[test]
fn pastas_adicionadas_e_removidas_sem_descadastrar() {
    let c = cenario();
    let telas = c.dir.path().join("codigo").join("telas");
    fs::create_dir_all(&telas).unwrap();
    fs::write(telas.join("UFrmMulta.pas"), "unit UFrmMulta;").unwrap();
    let raiz = c.codigo.to_str().unwrap().to_string();
    let ajuste = AjusteLeitura {
        projeto: PROJETO.into(),
        pastas: vec![raiz.clone(), telas.to_str().unwrap().into()],
        ..Default::default()
    };
    let cadastro = Cadastro::em(c.dados.projetos());

    // A prévia mostra a pasta nova sem gravar.
    let previa = previa_leitura(&c.dados, &ajuste).unwrap();
    assert_eq!(previa.pastas.len(), 2);
    assert_eq!(previa.pastas[1].prefixo.as_deref(), Some("telas"));
    assert_eq!(cadastro.obter(PROJETO).unwrap().unwrap().pastas.len(), 1);

    let feito = salvar_leitura(&c.dados, &ajuste).unwrap();
    assert_eq!(feito.pastas.len(), 2);
    let gravado = cadastro.obter(PROJETO).unwrap().unwrap();
    assert_eq!(gravado.pastas[1].prefixo.as_deref(), Some("telas"));
    assert!(
        cadastro
            .inventario(PROJETO)
            .unwrap()
            .unwrap()
            .arquivo("telas/UFrmMulta.pas")
            .is_some()
    );

    // Remover a pasta nova volta ao que era; a raiz nunca muda de prefixo.
    let so_raiz = AjusteLeitura {
        pastas: vec![raiz.clone()],
        ..ajuste.clone()
    };
    salvar_leitura(&c.dados, &so_raiz).unwrap();
    let gravado = cadastro.obter(PROJETO).unwrap().unwrap();
    assert_eq!(gravado.pastas.len(), 1);
    assert_eq!(gravado.pastas[0].prefixo, None);
    assert!(
        cadastro
            .inventario(PROJETO)
            .unwrap()
            .unwrap()
            .arquivo("telas/UFrmMulta.pas")
            .is_none()
    );

    // A última pasta não sai, e nada é gravado.
    let sem_pastas = AjusteLeitura {
        pastas: vec![" ".into()],
        ..ajuste.clone()
    };
    let erro = salvar_leitura(&c.dados, &sem_pastas).unwrap_err();
    assert!(erro.contains("última"), "{erro}");
    assert_eq!(cadastro.obter(PROJETO).unwrap().unwrap(), gravado);

    // Caminho relativo ou que não existe: recusado, cadastro intacto.
    for ruim in ["relativa/pasta", "/nao/existe/mesmo"] {
        let pedido = AjusteLeitura {
            pastas: vec![raiz.clone(), ruim.into()],
            ..ajuste.clone()
        };
        assert!(salvar_leitura(&c.dados, &pedido).is_err(), "{ruim}");
        assert_eq!(cadastro.obter(PROJETO).unwrap().unwrap(), gravado);
    }

    // A pasta da base não pode virar pasta do projeto.
    let base = base_atual(&c.dados);
    let com_base = AjusteLeitura {
        pastas: vec![raiz, base.to_str().unwrap().into()],
        ..ajuste
    };
    assert!(salvar_leitura(&c.dados, &com_base).is_err());
    assert_eq!(cadastro.obter(PROJETO).unwrap().unwrap(), gravado);
}

// ---------------------------------------------------------------------------
// Ambiente do Claude num CLAUDE_CONFIG_DIR temporário

/// Ambiente falso: só o `CLAUDE_CONFIG_DIR` e a pasta do usuário.
struct ComConfigDir(PathBuf);

impl Ambiente for ComConfigDir {
    fn plataforma(&self) -> Plataforma {
        Plataforma::atual()
    }
    fn var(&self, nome: &str) -> Option<OsString> {
        (nome == "CLAUDE_CONFIG_DIR").then(|| self.0.clone().into_os_string())
    }
    fn home(&self) -> Option<PathBuf> {
        Some(self.0.join("home-que-nao-deve-ser-usada"))
    }
    fn e_arquivo(&self, _: &Path) -> bool {
        unreachable!()
    }
    fn e_pasta(&self, _: &Path) -> bool {
        unreachable!()
    }
    fn nomes_na_pasta(&self, _: &Path) -> Vec<OsString> {
        unreachable!()
    }
    fn executar(&self, _: &Path, _: &[&str], _: Duration) -> Result<Saida, ErroExecucao> {
        unreachable!("o preparo não executa o claude")
    }
}

// No formato em que o Claude Code grava esses arquivos (JSON com 2 espaços), como no
// teste do preparo (#24). Um arquivo editado à mão em outro formato volta com o mesmo
// conteúdo, a mesma ordem das chaves e a mesma indentação, mas um JSON numa linha só
// volta reformatado.
const ESTADO_DO_ANALISTA: &str = r#"{
  "numStartups": 7,
  "projects": {
    "/outro/projeto": {
      "mcpServers": {
        "banco": {
          "command": "banco-mcp"
        }
      },
      "hasTrustDialogAccepted": true
    }
  },
  "zUltimaChave": "fica no fim"
}
"#;

const SETTINGS_DO_ANALISTA: &str = r#"{
  "theme": "dark",
  "permissions": {
    "deny": [
      "Read(./.env)"
    ]
  },
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "command",
            "command": "meu-hook.sh"
          }
        ]
      }
    ]
  }
}
"#;

/// Configuração do Claude do analista num `CLAUDE_CONFIG_DIR` temporário.
fn configuracao_do_analista(raiz: &Path) -> (Preparador, PathBuf, PathBuf) {
    let config_dir = raiz.join("config-claude");
    fs::create_dir_all(&config_dir).unwrap();
    let estado = config_dir.join(".claude.json");
    let settings = config_dir.join("settings.json");
    fs::write(&estado, ESTADO_DO_ANALISTA).unwrap();
    fs::write(&settings, SETTINGS_DO_ANALISTA).unwrap();
    let exe = raiz.join(if cfg!(windows) { "cba.exe" } else { "cba" });
    let preparador = Preparador::do_ambiente(&ComConfigDir(config_dir), exe).unwrap();
    assert_eq!(preparador.config.estado, estado);
    assert_eq!(preparador.config.settings, settings);
    (preparador, estado, settings)
}

/// Prepara o ambiente e liga a proteção, conferindo que a configuração mudou.
fn preparar_com_protecao(dados: &DiretorioDados, preparador: &Preparador, e: &Path, s: &Path) {
    analise::preparar_ambiente(dados, preparador, PROJETO).unwrap();
    let p = alteracoes::mudar_protecao(dados, preparador, PROJETO, true).unwrap();
    assert!(p.ligada && p.completa);
    assert!(
        analise::verificar_ambiente(dados, preparador, PROJETO)
            .unwrap()
            .pronto
    );
    assert_ne!(fs::read(e).unwrap(), ESTADO_DO_ANALISTA.as_bytes());
    // Os hooks e a proteção ficam no arquivo local da pasta (#304), não no do usuário.
    assert_eq!(fs::read(s).unwrap(), SETTINGS_DO_ANALISTA.as_bytes());
}

#[test]
fn desfazer_o_ambiente_deixa_a_configuracao_do_claude_como_antes() {
    let c = cenario();
    let raiz = dunce::canonicalize(c.dir.path()).unwrap();
    let (preparador, estado, settings) = configuracao_do_analista(&raiz);
    preparar_com_protecao(&c.dados, &preparador, &estado, &settings);
    let local = c.codigo.join(".claude").join("settings.local.json");
    assert!(local.is_file());

    let feito = desfazer_ambiente(&c.dados, &preparador, PROJETO).unwrap();
    assert!(
        !c.codigo.join(".claude").exists(),
        "o arquivo e a pasta saem"
    );
    assert_eq!(feito.protecao_tirada.len(), 1);
    assert!(feito.avisos.is_empty(), "{:?}", feito.avisos);
    assert!(feito.igual_ao_antes);
    // Byte a byte, como antes do preparo.
    assert_eq!(fs::read_to_string(&estado).unwrap(), ESTADO_DO_ANALISTA);
    assert_eq!(fs::read_to_string(&settings).unwrap(), SETTINGS_DO_ANALISTA);
    let v = analise::verificar_ambiente(&c.dados, &preparador, PROJETO).unwrap();
    assert_eq!(v.verificacao.servidor_mcp, Situacao::Ausente);
    assert!(!protecao::protegido(&c.dados, PROJETO));
    // O projeto continua cadastrado; desfazer de novo não muda nada.
    assert!(
        Cadastro::em(c.dados.projetos())
            .obter(PROJETO)
            .unwrap()
            .is_some()
    );
    desfazer_ambiente(&c.dados, &preparador, PROJETO).unwrap();
    assert_eq!(fs::read_to_string(&settings).unwrap(), SETTINGS_DO_ANALISTA);
}

#[test]
fn desfazer_devolve_os_hooks_de_outra_instalacao_do_app() {
    // #213: o servidor e os hooks da pasta apontavam para outra instalação (a de
    // desenvolvimento).
    let c = cenario();
    let raiz = dunce::canonicalize(c.dir.path()).unwrap();
    let (preparador, estado, _) = configuracao_do_analista(&raiz);
    let dev = raiz
        .join("dev")
        .join(if cfg!(windows) { "cba.exe" } else { "cba" });
    let dev = Preparador::novo(preparador.config.clone(), dev, Plataforma::atual()).unwrap();
    dev.preparar(&cba_session::preparo::Alvo::da_pasta(PROJETO, &c.codigo).unwrap())
        .unwrap();
    let local = c.codigo.join(".claude").join("settings.local.json");
    // A configuração "de antes", com 4 espaços de indentação.
    let quatro = |caminho: &Path| {
        let v: serde_json::Value = serde_json::from_slice(&fs::read(caminho).unwrap()).unwrap();
        let mut b = Vec::new();
        let f = serde_json::ser::PrettyFormatter::with_indent(b"    ");
        let mut ser = serde_json::Serializer::with_formatter(&mut b, f);
        serde::Serialize::serialize(&v, &mut ser).unwrap();
        b.push(b'\n');
        fs::write(caminho, &b).unwrap();
        b
    };
    let (estado_antes, local_antes) = (quatro(&estado), quatro(&local));

    let plano = analise::planejar_preparo(&c.dados, &preparador, PROJETO).unwrap();
    assert_eq!(
        plano.outras_instalacoes().len(),
        1,
        "o plano avisa da troca"
    );
    analise::preparar_ambiente(&c.dados, &preparador, PROJETO).unwrap();
    assert_ne!(fs::read(&local).unwrap(), local_antes);

    let feito = desfazer_ambiente(&c.dados, &preparador, PROJETO).unwrap();
    assert!(feito.igual_ao_antes, "{:?}", feito.avisos);
    assert_eq!(fs::read(&local).unwrap(), local_antes, "diff vazio");
    assert_eq!(fs::read(&estado).unwrap(), estado_antes, "diff vazio");
}

#[test]
fn desfazer_diz_o_que_ficou_e_por_que() {
    let c = cenario();
    let raiz = dunce::canonicalize(c.dir.path()).unwrap();
    let (preparador, estado, _) = configuracao_do_analista(&raiz);
    analise::preparar_ambiente(&c.dados, &preparador, PROJETO).unwrap();
    // O Claude Code grava dados da pasta ao abrir uma conversa.
    let mut v: serde_json::Value = serde_json::from_slice(&fs::read(&estado).unwrap()).unwrap();
    for (_, projeto) in v["projects"].as_object_mut().unwrap().iter_mut() {
        if projeto["mcpServers"].get("cba").is_some() {
            projeto["lastCost"] = 0.into();
        }
    }
    fs::write(&estado, serde_json::to_vec_pretty(&v).unwrap()).unwrap();

    let feito = desfazer_ambiente(&c.dados, &preparador, PROJETO).unwrap();
    assert!(!feito.igual_ao_antes);
    assert_eq!(feito.avisos.len(), 1, "{:?}", feito.avisos);
    assert!(
        feito.avisos[0].contains("histórico de uso"),
        "{}",
        feito.avisos[0]
    );
}

#[test]
fn descadastro_desfaz_o_ambiente_e_a_protecao_e_mantem_os_documentos() {
    let c = cenario();
    c.aprovar_analise_da_multa();
    let raiz = dunce::canonicalize(c.dir.path()).unwrap();
    let (preparador, estado, settings) = configuracao_do_analista(&raiz);
    preparar_com_protecao(&c.dados, &preparador, &estado, &settings);
    let base = base_atual(&c.dados);
    let documentos = conteudo(&base.join(PROJETO));

    let feito = descadastrar(&c.dados, &preparador, PROJETO).unwrap();
    assert_eq!(fs::read_to_string(&estado).unwrap(), ESTADO_DO_ANALISTA);
    assert_eq!(fs::read_to_string(&settings).unwrap(), SETTINGS_DO_ANALISTA);
    assert!(!protecao::protegido(&c.dados, PROJETO));
    assert!(
        Cadastro::em(c.dados.projetos())
            .obter(PROJETO)
            .unwrap()
            .is_none()
    );
    assert_eq!(feito.documentos, Some(base.join(PROJETO)));
    assert_eq!(conteudo(&base.join(PROJETO)), documentos);
    assert!(descadastrar(&c.dados, &preparador, PROJETO).is_err());
    assert!(
        configuracoes(&c.dados, Some(&preparador))
            .unwrap()
            .projetos
            .is_empty()
    );
}

#[test]
fn descadastro_com_a_pasta_do_projeto_apagada() {
    let c = cenario();
    let raiz = dunce::canonicalize(c.dir.path()).unwrap();
    let (preparador, estado, settings) = configuracao_do_analista(&raiz);
    preparar_com_protecao(&c.dados, &preparador, &estado, &settings);
    fs::remove_dir_all(&c.codigo).unwrap();

    let feito = descadastrar(&c.dados, &preparador, PROJETO).unwrap();
    assert_eq!(feito.ambiente.avisos.len(), 1);
    assert_eq!(fs::read_to_string(&estado).unwrap(), ESTADO_DO_ANALISTA);
    assert_eq!(fs::read_to_string(&settings).unwrap(), SETTINGS_DO_ANALISTA);
}

#[test]
fn descadastro_de_um_projeto_mantem_o_ambiente_do_outro() {
    let c = cenario();
    let raiz = dunce::canonicalize(c.dir.path()).unwrap();
    let (preparador, _, _) = configuracao_do_analista(&raiz);
    let outro = raiz.join("codigo").join("outro");
    fs::create_dir_all(&outro).unwrap();
    fs::write(outro.join("a.sql"), "select 1;\n").unwrap();
    primeiro_uso::cadastrar_projeto(
        &c.dados,
        &primeiro_uso::PedidoProjeto {
            nome: "Outro".into(),
            pastas: vec![outro.to_str().unwrap().into()],
            ..Default::default()
        },
    )
    .unwrap();
    analise::preparar_ambiente(&c.dados, &preparador, PROJETO).unwrap();
    analise::preparar_ambiente(&c.dados, &preparador, "outro").unwrap();
    descadastrar(&c.dados, &preparador, PROJETO).unwrap();
    assert!(
        analise::verificar_ambiente(&c.dados, &preparador, "outro")
            .unwrap()
            .pronto
    );
}

// ---------------------------------------------------------------------------
// Textos editáveis

#[test]
fn textos_mostram_quando_a_base_sobrepoe_o_padrao() {
    let c = cenario();
    let lista = textos(&c.dados).unwrap();
    let de = |g: GrupoTexto| lista.iter().filter(|t| t.grupo == g).count();
    assert!(de(GrupoTexto::Template) > 0);
    // Um roteiro por fluxo: todos revisados (os de revisão, histórias e cenários
    // na #168).
    assert_eq!(de(GrupoTexto::Roteiro), 7);
    assert_eq!(de(GrupoTexto::Prompt), 9);
    assert_eq!(de(GrupoTexto::RegrasDoProjeto), 1);
    assert_eq!(de(GrupoTexto::Exportacao), 3);
    let json = serde_json::to_value(&lista).unwrap();
    assert!(json[0]["grupo"].is_string() && json[0]["estado"].is_string());

    let roteiro = "_templates/roteiros/perguntar.md";
    let estado = |a: &str| {
        textos(&c.dados)
            .unwrap()
            .into_iter()
            .find(|t| t.arquivo == a)
            .unwrap()
            .estado
    };
    let base = base_atual(&c.dados);
    let _ = fs::remove_file(base.join(roteiro));
    assert_eq!(estado(roteiro), EstadoTexto::Padrao);
    // Abrir cria a cópia do padrão, sem sobrescrever depois.
    let caminho = preparar_texto(&c.dados, roteiro).unwrap();
    assert_eq!(caminho, base.join(roteiro));
    assert_eq!(estado(roteiro), EstadoTexto::IgualAoPadrao);
    fs::write(&caminho, "# Meu roteiro\n\n{{entrada}}\n").unwrap();
    assert_eq!(estado(roteiro), EstadoTexto::Editado);
    preparar_texto(&c.dados, roteiro).unwrap();
    assert_eq!(
        fs::read_to_string(&caminho).unwrap(),
        "# Meu roteiro\n\n{{entrada}}\n"
    );

    let regras = format!("{PROJETO}/regras-do-projeto.md");
    let caminho = preparar_texto(&c.dados, &regras).unwrap();
    fs::write(&caminho, "# Regras\n\nNunca citar o banco de produção.\n").unwrap();
    assert_eq!(estado(&regras), EstadoTexto::Editado);

    // Só os textos do catálogo.
    for fora in [
        "../fora.md",
        "faturamento-exemplo/projeto.md",
        "_templates/inexistente.md",
        "/etc/passwd",
    ] {
        assert!(preparar_texto(&c.dados, fora).is_err(), "{fora}");
    }
}

#[test]
fn configuracoes_reunem_cliente_base_e_projetos() {
    let c = cenario();
    primeiro_uso::escolher_cliente(&c.dados, ClientePreferido::ClaudeDesktop).unwrap();
    let cfg = configuracoes(&c.dados, None).unwrap();
    assert_eq!(cfg.cliente, Some(ClientePreferido::ClaudeDesktop));
    assert_eq!(cfg.base, Some(base_atual(&c.dados)));
    assert_eq!(cfg.projetos.len(), 1);
    assert!(cfg.projetos[0].pastas[0].existe);
    let json = serde_json::to_value(&cfg).unwrap();
    assert_eq!(json["projetos"][0]["id"], PROJETO);
    assert!(json["configClaude"].is_null());
}

#[test]
fn info_da_base_conta_arquivos_sem_o_git_e_reconhece_repositorio() {
    let c = cenario();
    let base = base_atual(&c.dados);
    let antes = info_base(&c.dados).unwrap().unwrap();
    assert_eq!(antes.caminho, base);
    assert!(antes.existe);
    assert!(!antes.git);

    fs::write(base.join("nota.md"), "# nota\n").unwrap();
    fs::create_dir_all(base.join(".git")).unwrap();
    fs::write(base.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
    fs::write(base.join(".git/objeto"), "x".repeat(1000)).unwrap();

    let depois = info_base(&c.dados).unwrap().unwrap();
    assert!(depois.git);
    // A pasta `.git` não entra na conta; a nota nova entra.
    assert_eq!(depois.arquivos, antes.arquivos + 1);
    assert_eq!(depois.bytes, antes.bytes + "# nota\n".len() as u64);
    let json = serde_json::to_value(&depois).unwrap();
    assert_eq!(json["arquivos"], depois.arquivos);
    assert!(json.get("indiceAtualizadoEm").is_some());
}

#[test]
fn info_da_base_sem_base_configurada_e_vazia() {
    let dir = tempfile::tempdir().unwrap();
    let dados = DiretorioDados::em(dir.path());
    assert_eq!(info_base(&dados).unwrap(), None);
}

// ---------------------------------------------------------------------------
// Editor embutido dos textos

const ROTEIRO: &str = "_templates/roteiros/perguntar.md";
const TEMPLATE: &str = "_templates/regra.md";

#[test]
fn comandos_dos_textos_recusam_arquivo_fora_do_catalogo() {
    let c = cenario();
    for fora in [
        "../fora.md",
        "faturamento-exemplo/projeto.md",
        "_templates/inexistente.md",
        "/etc/passwd",
    ] {
        assert!(ler_texto(&c.dados, fora).is_err(), "{fora}");
        assert!(validar_texto(&c.dados, fora, "x").is_err(), "{fora}");
        assert!(
            salvar_texto(&c.dados, fora, "x", "ausente").is_err(),
            "{fora}"
        );
        assert!(restaurar_texto(&c.dados, fora).is_err(), "{fora}");
    }
    assert!(
        !base_atual(&c.dados)
            .join("_templates/inexistente.md")
            .exists()
    );
}

#[test]
fn ler_devolve_o_padrao_sem_copia_e_o_da_base_com_copia() {
    let c = cenario();
    let base = base_atual(&c.dados);
    let _ = fs::remove_file(base.join(ROTEIRO));

    let l = ler_texto(&c.dados, ROTEIRO).unwrap();
    assert_eq!(l.estado, EstadoTexto::Padrao);
    assert_eq!(l.texto, l.padrao);
    assert_eq!(l.versao, "ausente");
    assert!(
        l.variaveis
            .iter()
            .any(|v| v.nome == "entrada" && !v.descricao.is_empty())
    );
    assert!(!base.join(ROTEIRO).exists(), "ler não cria arquivo");

    fs::create_dir_all(base.join(ROTEIRO).parent().unwrap()).unwrap();
    fs::write(base.join(ROTEIRO), "# Meu\n{{entrada}}\n").unwrap();
    let l = ler_texto(&c.dados, ROTEIRO).unwrap();
    assert_eq!(l.estado, EstadoTexto::Editado);
    assert_eq!(l.texto, "# Meu\n{{entrada}}\n");
    assert_ne!(l.versao, "ausente");

    let t = ler_texto(&c.dados, TEMPLATE).unwrap();
    assert!(t.variaveis.iter().any(|v| v.nome == "titulo"));
    let json = serde_json::to_value(&l).unwrap();
    assert!(json["versao"].is_string() && json["variaveis"].is_array());
}

#[test]
fn validar_da_os_avisos_que_o_agente_receberia_sem_gravar() {
    let c = cenario();
    let base = base_atual(&c.dados);
    let _ = fs::remove_file(base.join(ROTEIRO));

    let avisos = validar_texto(&c.dados, ROTEIRO, "Linha 1\n{{inventada}}\n").unwrap();
    assert!(
        avisos
            .iter()
            .any(|a| a.variavel == "inventada" && a.linha == Some(2) && !a.ausente)
    );
    for essencial in ["entrada", "secoes", "contexto"] {
        assert!(
            avisos.iter().any(|a| a.variavel == essencial && a.ausente),
            "{essencial}"
        );
    }

    let ok = validar_texto(&c.dados, ROTEIRO, "{{entrada}} {{secoes}} {{contexto}}").unwrap();
    assert!(ok.is_empty(), "{ok:?}");

    let t = validar_texto(&c.dados, TEMPLATE, "# {{titulo}}\n{{marcador_falso}}\n").unwrap();
    assert_eq!(t.len(), 1);
    assert_eq!(
        (t[0].variavel.as_str(), t[0].linha),
        ("marcador_falso", Some(2))
    );

    let regras = format!("{PROJETO}/regras-do-projeto.md");
    assert!(
        validar_texto(&c.dados, &regras, "{{qualquer}}")
            .unwrap()
            .is_empty()
    );
    assert!(!base.join(ROTEIRO).exists(), "validar não grava");
}

#[test]
fn salvar_grava_e_recusa_versao_desatualizada() {
    let c = cenario();
    let base = base_atual(&c.dados);
    let _ = fs::remove_file(base.join(ROTEIRO));

    let lido = ler_texto(&c.dados, ROTEIRO).unwrap();
    let salvo = salvar_texto(&c.dados, ROTEIRO, "# Meu\n{{x}}\n", &lido.versao).unwrap();
    assert_eq!(salvo.estado, EstadoTexto::Editado);
    assert_eq!(salvo.avisos.iter().filter(|a| a.variavel == "x").count(), 1);
    assert_eq!(
        fs::read_to_string(base.join(ROTEIRO)).unwrap(),
        "# Meu\n{{x}}\n"
    );
    assert_eq!(salvo.versao, ler_texto(&c.dados, ROTEIRO).unwrap().versao);

    // Editado fora do app depois da leitura: conflito, nada gravado.
    fs::write(base.join(ROTEIRO), "de fora\n").unwrap();
    let e = salvar_texto(&c.dados, ROTEIRO, "do app\n", &salvo.versao).unwrap_err();
    assert!(e.starts_with(CONFLITO), "{e}");
    assert_eq!(fs::read_to_string(base.join(ROTEIRO)).unwrap(), "de fora\n");

    // Versão `ausente` não vale se o arquivo apareceu.
    assert!(
        salvar_texto(&c.dados, ROTEIRO, "x", "ausente")
            .unwrap_err()
            .starts_with(CONFLITO)
    );
    let atual = ler_texto(&c.dados, ROTEIRO).unwrap().versao;
    salvar_texto(&c.dados, ROTEIRO, "do app\n", &atual).unwrap();
}

#[test]
fn restaurar_remove_a_copia_e_regras_voltam_ao_padrao() {
    let c = cenario();
    let base = base_atual(&c.dados);
    let estado = |a: &str| {
        textos(&c.dados)
            .unwrap()
            .into_iter()
            .find(|t| t.arquivo == a)
            .unwrap()
            .estado
    };

    fs::create_dir_all(base.join(ROTEIRO).parent().unwrap()).unwrap();
    fs::write(base.join(ROTEIRO), "editado\n").unwrap();
    assert_eq!(estado(ROTEIRO), EstadoTexto::Editado);
    let l = restaurar_texto(&c.dados, ROTEIRO).unwrap();
    assert!(!base.join(ROTEIRO).exists());
    assert_eq!(
        (l.estado, l.versao.as_str()),
        (EstadoTexto::Padrao, "ausente")
    );
    assert_eq!(estado(ROTEIRO), EstadoTexto::Padrao);
    // Sem cópia, restaurar de novo não falha.
    restaurar_texto(&c.dados, ROTEIRO).unwrap();

    let regras = format!("{PROJETO}/regras-do-projeto.md");
    fs::write(base.join(&regras), "# Minhas regras\n").unwrap();
    assert_eq!(estado(&regras), EstadoTexto::Editado);
    let l = restaurar_texto(&c.dados, &regras).unwrap();
    assert!(base.join(&regras).is_file(), "as regras precisam existir");
    assert_eq!(l.estado, EstadoTexto::IgualAoPadrao);
    assert_eq!(estado(&regras), EstadoTexto::IgualAoPadrao);
}

// ---------------------------------------------------------------------------
// Exportação em HTML (#166)

const TEMPLATE_HTML: &str = "_templates/exportacao/documento.html";
const TOKENS_HTML: &str = "_templates/exportacao/tokens.css";

#[test]
fn editor_da_exportacao_html_lista_variaveis_valida_e_mostra_a_previa() {
    let c = cenario();
    let base = base_atual(&c.dados);

    let l = ler_texto(&c.dados, TEMPLATE_HTML).unwrap();
    assert_eq!(l.grupo, GrupoTexto::Exportacao);
    assert_eq!(l.estado, EstadoTexto::Padrao);
    assert!(l.variaveis.iter().any(|v| v.nome == "conteudo"));
    assert!(
        ler_texto(&c.dados, TOKENS_HTML)
            .unwrap()
            .variaveis
            .is_empty()
    );
    let json = serde_json::to_value(&l).unwrap();
    assert_eq!(json["grupo"], "exportacao");

    let avisos =
        validar_texto(&c.dados, TEMPLATE_HTML, "<h1>{{titulo}}</h1>\n{{marca}}\n").unwrap();
    assert_eq!(avisos.len(), 2, "{avisos:?}");
    assert_eq!(avisos[0].linha, Some(2));
    assert!(avisos[1].ausente && avisos[1].variavel == "conteudo");

    // Prévia com o texto em edição, sem gravar.
    let p = previa_exportacao(&c.dados, TOKENS_HTML, ":root { --cor-destaque: #123456; }").unwrap();
    assert!(p.html.contains("--cor-destaque: #123456;"));
    assert!(p.html.contains("Documento de exemplo"));
    assert!(p.avisos.is_empty(), "{:?}", p.avisos);
    assert!(!base.join(TOKENS_HTML).exists(), "a prévia não grava");

    let p = previa_exportacao(&c.dados, TEMPLATE_HTML, "<body>{{marca}}</body>").unwrap();
    assert_eq!(p.avisos.len(), 2, "{:?}", p.avisos);
    assert!(p.html.contains("<main class=\"documento\">"));

    // O template salvo na base com problema não repete o aviso na prévia dos
    // tokens só quando é o arquivo em edição.
    salvar_texto(
        &c.dados,
        TEMPLATE_HTML,
        "<body>{{marca}}{{conteudo}}</body>",
        "ausente",
    )
    .unwrap();
    let p = previa_exportacao(&c.dados, TOKENS_HTML, ":root {}").unwrap();
    assert_eq!(p.avisos.len(), 1, "{:?}", p.avisos);
    let p = previa_exportacao(&c.dados, TEMPLATE_HTML, "<body>{{conteudo}}</body>").unwrap();
    assert!(p.avisos.is_empty(), "{:?}", p.avisos);

    // Restaurar o padrão remove a cópia.
    restaurar_texto(&c.dados, TEMPLATE_HTML).unwrap();
    assert!(!base.join(TEMPLATE_HTML).exists());

    // Só os arquivos da exportação têm prévia.
    assert!(previa_exportacao(&c.dados, ROTEIRO, "x").is_err());
    assert!(previa_exportacao(&c.dados, "../fora.html", "x").is_err());
}

#[test]
fn mascara_da_demanda_ligada_por_padrao_e_desligavel_por_projeto() {
    let c = cenario();
    let cfg = configuracoes(&c.dados, None).unwrap();
    assert!(cfg.projetos[0].mascarar_demanda);
    let json = serde_json::to_value(&cfg).unwrap();
    assert_eq!(json["projetos"][0]["mascararDemanda"], true);

    assert!(!salvar_mascarar_demanda(&c.dados, PROJETO, false).unwrap());
    assert!(!configuracoes(&c.dados, None).unwrap().projetos[0].mascarar_demanda);
    assert!(!cadastrado(&c.dados, PROJETO).unwrap().mascarar_demanda);
    assert!(salvar_mascarar_demanda(&c.dados, PROJETO, true).unwrap());
    assert!(configuracoes(&c.dados, None).unwrap().projetos[0].mascarar_demanda);
    assert!(salvar_mascarar_demanda(&c.dados, "nao-existe", false).is_err());
}
