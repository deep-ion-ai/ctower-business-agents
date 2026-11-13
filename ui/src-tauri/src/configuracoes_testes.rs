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
    let ajuste = AjusteExclusoes {
        projeto: PROJETO.into(),
        exclusoes: vec!["cobol/".into(), " ".into()],
        inclusoes: vec![],
    };
    let previa = previa_exclusoes(&c.dados, &ajuste).unwrap();
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

    let feito = salvar_exclusoes(&c.dados, &ajuste).unwrap();
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

    let invalido = AjusteExclusoes {
        exclusoes: vec!["[".into()],
        ..ajuste.clone()
    };
    assert!(salvar_exclusoes(&c.dados, &invalido).is_err());
    let desconhecido = AjusteExclusoes {
        projeto: "outro".into(),
        ..ajuste
    };
    assert!(previa_exclusoes(&c.dados, &desconhecido).is_err());
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
// conteúdo e a mesma ordem das chaves, mas reformatado.
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
    assert_ne!(fs::read(s).unwrap(), SETTINGS_DO_ANALISTA.as_bytes());
}

#[test]
fn desfazer_o_ambiente_deixa_a_configuracao_do_claude_como_antes() {
    let c = cenario();
    let raiz = dunce::canonicalize(c.dir.path()).unwrap();
    let (preparador, estado, settings) = configuracao_do_analista(&raiz);
    preparar_com_protecao(&c.dados, &preparador, &estado, &settings);

    let feito = desfazer_ambiente(&c.dados, &preparador, PROJETO).unwrap();
    assert_eq!(feito.protecao_tirada.len(), 1);
    assert!(feito.avisos.is_empty());
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
    assert_eq!(de(GrupoTexto::Roteiro), 3);
    assert_eq!(de(GrupoTexto::Prompt), 5);
    assert_eq!(de(GrupoTexto::RegrasDoProjeto), 1);
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
