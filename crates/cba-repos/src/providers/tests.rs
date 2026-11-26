use super::*;

fn conexao(tipo: &str, endereco: &str) -> Conexao {
    Conexao {
        id: format!("{tipo}-1"),
        nome: format!("Conexão {tipo}"),
        tipo: tipo.into(),
        endereco: endereco.into(),
        credencial: FonteCredencial::Chaveiro {
            conta: format!("conexao:{tipo}-1"),
        },
        opcoes: BTreeMap::new(),
    }
}

#[test]
fn registro_padrao_tem_os_cinco_tipos() {
    let tipos: Vec<_> = todos().iter().map(|d| d.tipo()).collect();
    assert_eq!(tipos, ["github", "gitlab", "jira", "postgres", "mysql"]);
    let bancos: Vec<_> = todos()
        .com_capacidade(Capacidade::Banco)
        .map(|d| d.tipo())
        .collect();
    assert_eq!(bancos, ["postgres", "mysql"]);
    let rastreadores: Vec<_> = todos()
        .com_capacidade(Capacidade::Rastreador)
        .map(|d| d.tipo())
        .collect();
    assert_eq!(rastreadores, ["github", "gitlab", "jira"]);
    assert!(todos().get("svn").is_none());
}

#[test]
fn conexao_serializa_e_volta() {
    let mut c = conexao("gitlab", "gitlab.example.com");
    c.credencial = FonteCredencial::Comando {
        cmd: "pass show gitlab | head -1".into(),
    };
    let json = serde_json::to_value(&c).unwrap();
    assert_eq!(json["credencial"]["fonte"], "comando");
    assert_eq!(json["credencial"]["cmd"], "pass show gitlab | head -1");
    assert!(
        json.get("opcoes").is_none(),
        "opções vazias não são gravadas"
    );
    let volta: Conexao = serde_json::from_value(json).unwrap();
    assert_eq!(volta, c);

    for fonte in [
        FonteCredencial::Chaveiro { conta: "x".into() },
        FonteCredencial::Comando { cmd: "c".into() },
        FonteCredencial::Cli,
        FonteCredencial::Arquivo {
            caminho: "/tmp/t".into(),
        },
        FonteCredencial::Nenhuma,
    ] {
        let s = serde_json::to_string(&fonte).unwrap();
        assert!(
            s.contains(&format!("\"fonte\":\"{}\"", fonte.tipo())),
            "{s}"
        );
        assert_eq!(serde_json::from_str::<FonteCredencial>(&s).unwrap(), fonte);
    }
    // Sem credencial no cadastro: Nenhuma.
    let c: Conexao =
        serde_json::from_str(r#"{"id":"a","nome":"A","tipo":"github","endereco":"github.com"}"#)
            .unwrap();
    assert_eq!(c.credencial, FonteCredencial::Nenhuma);
}

#[test]
fn endereco_normalizado_por_tipo() {
    let r = todos();
    let n = |tipo: &str, e: &str| r.get(tipo).unwrap().normalizar_endereco(e);
    assert_eq!(n("github", "GitHub.com").unwrap(), "github.com");
    assert_eq!(
        n("gitlab", "https://GitLab.Example.com/grupo/app.git").unwrap(),
        "gitlab.example.com"
    );
    assert!(n("gitlab", "").is_err());
    assert!(n("gitlab", "git lab").is_err());
    assert_eq!(
        n("jira", "https://jira.example.com/").unwrap(),
        "https://jira.example.com"
    );
    assert!(n("jira", "jira.example.com").is_err());
    assert!(n("jira", "https://u:senha@jira.example.com").is_err());
    assert_eq!(
        n("postgres", "DB.example.com/faturamento").unwrap(),
        "db.example.com:5432/faturamento"
    );
    assert_eq!(
        n("mysql", "db.example.com:3307/faturamento").unwrap(),
        "db.example.com:3307/faturamento"
    );
    assert_eq!(
        n("mysql", "db.example.com/faturamento").unwrap(),
        "db.example.com:3306/faturamento"
    );
    for ruim in [
        "db.example.com",
        "db.example.com:abc/x",
        "db.example.com:0/x",
        "postgres://u:s3gr3d0@db.example.com/x",
        "u@db.example.com/x",
    ] {
        let e = n("postgres", ruim).unwrap_err();
        assert!(!e.contains("s3gr3d0"), "{e}");
    }
}

#[test]
fn fontes_aceitas_por_tipo() {
    let r = todos();
    let mut c = conexao("github", "github.com");
    c.credencial = FonteCredencial::Cli;
    r.validar(&c).unwrap();
    // `Cli` só vale para o GitHub.
    let mut g = conexao("gitlab", "gitlab.example.com");
    g.credencial = FonteCredencial::Cli;
    let e = r.validar(&g).unwrap_err();
    assert!(e.contains("não aceita"), "{e}");
    // Jira sempre tem credencial.
    let mut j = conexao("jira", "https://jira.example.com");
    j.credencial = FonteCredencial::Nenhuma;
    assert!(r.validar(&j).is_err());
    assert_eq!(r.get("github").unwrap().fonte_padrao(), TipoFonte::Chaveiro);
    // Fonte sem os campos.
    g.credencial = FonteCredencial::Comando { cmd: " ".into() };
    assert!(r.validar(&g).unwrap_err().contains("comando"));
}

#[test]
fn opcoes_validadas_pelos_campos_do_descritor() {
    let r = todos();
    let mut c = conexao("postgres", "db.example.com/faturamento");
    // `usuario` é obrigatório.
    assert!(r.validar(&c).unwrap_err().contains("usuário"));
    c.opcoes.insert("usuario".into(), "leitura".into());
    r.validar(&c).unwrap();
    let d = r.de(&c).unwrap();
    assert_eq!(c.opcao(d, "tempo_limite_s"), Some("30"));
    assert_eq!(c.opcao(d, "limite_linhas"), Some("200"));
    assert_eq!(c.opcao(d, "tls"), Some("preferir"));
    c.opcoes.insert("limite_linhas".into(), "50000".into());
    assert!(r.validar(&c).unwrap_err().contains("entre 1 e 10000"));
    c.opcoes.insert("limite_linhas".into(), "50".into());
    c.opcoes.insert("tls".into(), "talvez".into());
    assert!(r.validar(&c).unwrap_err().contains("opção inválida"));
    c.opcoes.insert("tls".into(), "exigir".into());
    c.opcoes.insert("cor".into(), "azul".into());
    assert!(r.validar(&c).unwrap_err().contains("opção desconhecida"));
}

#[test]
fn lista_sem_id_nem_nome_repetido() {
    let r = todos();
    let a = conexao("gitlab", "gitlab.example.com");
    let mut b = conexao("github", "github.com");
    r.validar_lista(&[a.clone(), b.clone()]).unwrap();
    b.nome = a.nome.to_uppercase();
    assert!(
        r.validar_lista(&[a.clone(), b.clone()])
            .unwrap_err()
            .contains("já existe")
    );
    b.nome = "outra".into();
    b.id = a.id.clone();
    assert!(
        r.validar_lista(&[a.clone(), b])
            .unwrap_err()
            .contains("repetido")
    );
    let mut d = conexao("svn", "svn.example.com");
    d.id = "svn".into();
    assert!(r.validar_lista(&[d]).unwrap_err().contains("desconhecido"));
}

#[test]
fn variaveis_de_ambiente_das_clis() {
    let r = todos();
    let nomes = |c: &Conexao| -> Vec<(&'static str, ValorVariavel)> {
        r.de(c)
            .unwrap()
            .variaveis_ambiente(c)
            .into_iter()
            .map(|v| (v.nome, v.valor))
            .collect()
    };
    assert_eq!(
        nomes(&conexao("gitlab", "GitLab.example.com")),
        [
            ("GITLAB_TOKEN", ValorVariavel::Credencial),
            (
                "GITLAB_HOST",
                ValorVariavel::Texto("gitlab.example.com".into())
            )
        ]
    );
    assert_eq!(nomes(&conexao("github", "github.com"))[0].0, "GH_TOKEN");
    let mut j = conexao("jira", "https://jira.example.com");
    assert_eq!(nomes(&j), [("JIRA_API_TOKEN", ValorVariavel::Credencial)]);
    j.opcoes
        .insert("config_file".into(), "/etc/jira.yml".into());
    assert_eq!(
        nomes(&j)[1],
        (
            "JIRA_CONFIG_FILE",
            ValorVariavel::Texto("/etc/jira.yml".into())
        )
    );
    assert!(nomes(&conexao("postgres", "db/x")).is_empty());
}

#[cfg(unix)]
#[test]
fn credencial_so_no_ambiente_do_filho() {
    use std::process::Command;
    let c = conexao("gitlab", "gitlab.example.com");
    let d = todos().de(&c).unwrap();
    let mut cmd = Command::new("/bin/sh");
    cmd.args([
        "-c",
        r#"[ "$GITLAB_TOKEN" = "tok" ] && [ "$GITLAB_HOST" = "gitlab.example.com" ]"#,
    ]);
    let s = Secret::new("tok");
    aplicar_ambiente(&mut cmd, d, &c, Some(&s));
    assert!(cmd.get_args().all(|a| a != "tok"));
    assert!(cmd.status().unwrap().success());
    // Sem credencial, a variável do token não é posta.
    let mut cmd = Command::new("/bin/sh");
    cmd.args(["-c", r#"[ -z "${GITLAB_TOKEN+x}" ]"#])
        .env_remove("GITLAB_TOKEN");
    aplicar_ambiente(&mut cmd, d, &c, None);
    assert!(cmd.status().unwrap().success());
}

#[test]
fn resumo_para_a_ui() {
    let resumos = todos().resumos();
    let json = serde_json::to_value(&resumos).unwrap();
    let pg = json
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["tipo"] == "postgres")
        .unwrap();
    assert_eq!(pg["rotulo"], "PostgreSQL");
    assert_eq!(pg["capacidades"], serde_json::json!(["banco"]));
    assert_eq!(pg["fonte_padrao"], "chaveiro");
    let tls = pg["campos_opcoes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["chave"] == "tls")
        .unwrap();
    assert_eq!(tls["tipo"], "escolha");
    assert_eq!(tls["opcoes"][0]["valor"], "exigir");
    assert_eq!(tls["padrao"], "preferir");
    let gh = &json[0];
    assert_eq!(gh["fontes_aceitas"][1]["fonte"], "cli");
    assert!(gh["passos_prontidao"].as_array().unwrap().len() >= 2);
}

// ------------------------------------------------- provider fictício

/// Um tipo novo, como o guia `docs/providers.md` descreve: só o descritor e o
/// registro, sem mexer em nada fora dele.
struct DescritorFicticio;

const CAMPOS_FICTICIO: &[CampoOpcao] = &[CampoOpcao {
    chave: "organizacao",
    rotulo: "Organização",
    ajuda: "Organização no Rastreador Exemplo.",
    tipo: TipoCampo::Texto,
    padrao: None,
    obrigatorio: true,
}];

impl DescritorProvider for DescritorFicticio {
    fn tipo(&self) -> &'static str {
        "rastreador_exemplo"
    }
    fn rotulo(&self) -> &'static str {
        "Rastreador Exemplo"
    }
    fn icone(&self) -> &'static str {
        "list-checks"
    }
    fn capacidades(&self) -> &'static [Capacidade] {
        &[Capacidade::Rastreador]
    }
    fn endereco_padrao(&self) -> &'static str {
        "rastreador.example.com"
    }
    fn formato_endereco(&self) -> &'static str {
        "host, como rastreador.example.com"
    }
    fn normalizar_endereco(&self, endereco: &str) -> Result<String, String> {
        let h = host_de(endereco);
        if host_valido(&h) {
            Ok(h)
        } else {
            Err(format!("host inválido: {endereco}"))
        }
    }
    fn fontes_aceitas(&self) -> &'static [TipoFonte] {
        &[TipoFonte::Chaveiro, TipoFonte::Comando]
    }
    fn variaveis_ambiente(&self, conexao: &Conexao) -> Vec<VariavelAmbiente> {
        vec![
            VariavelAmbiente::credencial("EXEMPLO_TOKEN"),
            VariavelAmbiente::texto("EXEMPLO_HOST", host_de(&conexao.endereco)),
        ]
    }
    fn campos_opcoes(&self) -> &'static [CampoOpcao] {
        CAMPOS_FICTICIO
    }
    fn passos_prontidao(&self) -> &'static [PassoProntidao] {
        &[PassoProntidao {
            id: "quem_sou_eu",
            rotulo: "Acesso ao Rastreador Exemplo",
            orientacao: "Confira o token.",
        }]
    }
}

#[test]
fn tipo_novo_so_com_um_descritor() {
    let r = Registro::padrao().com(DescritorFicticio);
    let mut c = conexao("rastreador_exemplo", "https://Rastreador.example.com/x");
    assert!(r.validar(&c).unwrap_err().contains("organização"));
    c.opcoes.insert("organizacao".into(), "faturamento".into());
    r.validar(&c).unwrap();
    let d = r.de(&c).unwrap();
    assert_eq!(
        d.normalizar_endereco(&c.endereco).unwrap(),
        "rastreador.example.com"
    );
    assert!(
        r.com_capacidade(Capacidade::Rastreador)
            .any(|d| d.tipo() == "rastreador_exemplo")
    );
    assert!(r.resumos().iter().any(|s| s.rotulo == "Rastreador Exemplo"));
    c.credencial = FonteCredencial::Cli;
    assert!(r.validar(&c).is_err());
    // O registro do app não muda.
    assert!(todos().get("rastreador_exemplo").is_none());
}

#[test]
#[should_panic(expected = "repetido")]
fn tipo_repetido_no_registro() {
    let _ = Registro::padrao().com(DescritorGithub);
}
