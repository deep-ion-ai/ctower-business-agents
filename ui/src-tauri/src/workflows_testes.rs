//! Testes de Configurações › Workflows com a fixture "Faturamento Exemplo"
//! ([`crate::teste_base`]) e a configuração do Claude numa pasta temporária.

use std::fs;

use cba_session::clientes::Plataforma;
use cba_session::preparo::{ConfigClaude, Preparador, Situacao};

use super::*;
use crate::teste_base::{Cenario, PROJETO, cenario};

fn preparador(c: &Cenario) -> Preparador {
    let dir = c.dir.path().join("config-claude");
    fs::create_dir_all(&dir).unwrap();
    let exe = dunce::canonicalize(c.dir.path())
        .unwrap()
        .join(if cfg!(windows) { "cba.exe" } else { "cba" });
    Preparador::novo(
        ConfigClaude {
            estado: dir.join(".claude.json"),
            settings: dir.join("settings.json"),
        },
        exe,
        Plataforma::atual(),
    )
    .unwrap()
}

fn impacto(c: &Cenario, p: &Preparador) -> WorkflowVista {
    listar(&c.dados, Some(p)).unwrap().remove(0)
}

/// O arquivo instalado no projeto (a fixture tem uma pasta só) e o cabeçalho dele.
fn instalado(v: &WorkflowVista) -> &ArquivoInstalado {
    let i = &v.instalacoes[0];
    assert_eq!(i.projeto, PROJETO);
    assert_eq!(i.arquivos.len(), 1, "{:?}", i);
    &i.arquivos[0]
}

fn corpo_instalado(v: &WorkflowVista) -> String {
    let texto = fs::read_to_string(&instalado(v).arquivo).unwrap();
    instalados::corpo(&texto).unwrap().to_string()
}

#[test]
fn duplicar_editar_e_restaurar_reinstalam_a_versao_efetiva() {
    let c = cenario();
    let p = preparador(&c);
    analise::preparar_ambiente(&c.dados, &p, PROJETO).unwrap();

    let v = impacto(&c, &p);
    assert_eq!(v.nome, "cba-impacto");
    assert_eq!(v.origem_efetiva, OrigemWorkflow::Padrao);
    assert!(!v.fases.is_empty());
    assert!(!v.projeto.as_ref().unwrap().existe() && !v.global.existe());
    assert_eq!(instalado(&v).situacao, Situacao::Ok);
    assert_eq!(instalado(&v).origem, Some(OrigemWorkflow::Padrao));

    // Para todos os projetos: a cópia do padrão, já instalada.
    let feito = duplicar(&c.dados, Some(&p), "impacto", Destino::Global).unwrap();
    assert!(feito.salvo && feito.leitura.existe);
    assert_eq!(feito.reinstalados, 1);
    assert!(feito.avisos.is_empty(), "{:?}", feito.avisos);
    assert_eq!(feito.leitura.texto, padrao("impacto").unwrap());
    let v = impacto(&c, &p);
    assert_eq!(v.origem_efetiva, OrigemWorkflow::Global);
    assert_eq!(instalado(&v).situacao, Situacao::Ok);
    assert_eq!(instalado(&v).origem, Some(OrigemWorkflow::Global));
    assert!(
        duplicar(&c.dados, Some(&p), "impacto", Destino::Global)
            .unwrap_err()
            .contains("já existe")
    );

    // Na base: parte da cópia para todos os projetos.
    let l = duplicar(&c.dados, Some(&p), "impacto", Destino::Projeto)
        .unwrap()
        .leitura;
    assert!(
        l.arquivo
            .as_ref()
            .unwrap()
            .ends_with("_templates/workflows/impacto.js")
    );

    // Script com erro de fases: recusado, com a linha, e nada muda.
    let quebrado = format!("{}phase('Preparo')\n", l.texto);
    let r = salvar(
        &c.dados,
        Some(&p),
        "impacto",
        Destino::Projeto,
        &quebrado,
        &l.versao,
        false,
    )
    .unwrap();
    assert!(!r.salvo);
    let erro = r
        .leitura
        .validacao
        .problemas
        .iter()
        .find(|p| p.erro)
        .unwrap();
    assert!(erro.mensagem.contains("'Preparo'"), "{erro:?}");
    let linha = quebrado
        .lines()
        .position(|l| l.contains("phase('Preparo')"))
        .unwrap()
        + 1;
    assert_eq!(erro.linha, Some(linha));
    assert_eq!(r.leitura.versao, l.versao, "o arquivo não mudou");

    // Versão lida antiga: conflito.
    assert!(
        salvar(
            &c.dados,
            Some(&p),
            "impacto",
            Destino::Projeto,
            &l.texto,
            "outra",
            false
        )
        .unwrap_err()
        .starts_with(CONFLITO)
    );

    // Script válido: salvo, e a análise seguinte usa a cópia da base.
    let meu = format!("{}// minha frente\n", l.texto);
    let r = salvar(
        &c.dados,
        Some(&p),
        "impacto",
        Destino::Projeto,
        &meu,
        &l.versao,
        false,
    )
    .unwrap();
    assert!(r.salvo && r.reinstalados == 1, "{r:?}");
    let alvo = analise::alvo(&c.dados, PROJETO).unwrap();
    assert_eq!(alvo.workflows[0].origem, OrigemWorkflow::Projeto);
    assert_eq!(alvo.workflows[0].conteudo, meu);
    let v = impacto(&c, &p);
    assert_eq!(v.origem_efetiva, OrigemWorkflow::Projeto);
    assert_eq!(instalado(&v).origem, Some(OrigemWorkflow::Projeto));
    assert_eq!(corpo_instalado(&v), meu);
    assert!(
        analise::verificar_ambiente(&c.dados, &p, PROJETO)
            .unwrap()
            .verificacao
            .workflows_prontos
    );

    // A diferença com o padrão é a linha acrescentada.
    let d = diferenca("impacto", &meu).unwrap();
    let mudancas: Vec<_> = d.iter().filter(|l| l.tipo != "igual").collect();
    assert_eq!(mudancas.len(), 1);
    assert_eq!(mudancas[0].tipo, "acrescentada");
    assert_eq!(mudancas[0].texto, "// minha frente");

    // Restaurar a da base: volta a valer a cópia para todos os projetos.
    let r = restaurar(&c.dados, Some(&p), "impacto", Destino::Projeto).unwrap();
    assert!(!r.leitura.existe && r.reinstalados == 1);
    let v = impacto(&c, &p);
    assert_eq!(v.origem_efetiva, OrigemWorkflow::Global);
    assert_eq!(corpo_instalado(&v), padrao("impacto").unwrap());
}

#[test]
fn padrao_novo_nao_toca_a_copia_e_a_tela_avisa() {
    let c = cenario();
    let p = preparador(&c);
    let l = duplicar(&c.dados, Some(&p), "impacto", Destino::Global)
        .unwrap()
        .leitura;
    assert!(!l.padrao_mudou);
    // Uma cópia feita de um padrão antigo (como depois de atualizar o app).
    let arquivo = l.arquivo.unwrap();
    let minha = format!("{}// minha\n", l.texto);
    fs::write(
        &arquivo,
        copias::montar(&minha, &copias::sha256("padrão antigo")),
    )
    .unwrap();
    let antes = fs::read(&arquivo).unwrap();
    let v = impacto(&c, &p);
    assert!(v.global.padrao_mudou);
    assert_eq!(v.origem_efetiva, OrigemWorkflow::Global);
    let l = ler(&c.dados, "impacto", Some(Destino::Global)).unwrap();
    assert!(l.padrao_mudou);
    assert_eq!(l.texto, minha);
    assert_eq!(fs::read(&arquivo).unwrap(), antes, "ler não muda a cópia");

    // Salvar sem dizer que reviu mantém o aviso; com, ele some.
    let r = salvar(
        &c.dados,
        Some(&p),
        "impacto",
        Destino::Global,
        &minha,
        &l.versao,
        false,
    )
    .unwrap();
    assert!(r.leitura.padrao_mudou);
    let r = salvar(
        &c.dados,
        Some(&p),
        "impacto",
        Destino::Global,
        &minha,
        &r.leitura.versao,
        true,
    )
    .unwrap();
    assert!(!r.leitura.padrao_mudou);
    assert_eq!(r.leitura.texto, minha);
}

#[test]
fn edicao_no_arquivo_instalado_vai_para_uma_copia() {
    let c = cenario();
    let p = preparador(&c);
    analise::preparar_ambiente(&c.dados, &p, PROJETO).unwrap();
    let arquivo = instalado(&impacto(&c, &p)).arquivo.clone();
    let editado = format!(
        "{}// editado à mão\n",
        fs::read_to_string(&arquivo).unwrap()
    );
    fs::write(&arquivo, &editado).unwrap();
    assert_eq!(instalado(&impacto(&c, &p)).situacao, Situacao::Divergente);

    // Só o arquivo instalado do projeto.
    assert!(
        levar_edicao(
            &c.dados,
            Some(&p),
            PROJETO,
            "impacto",
            &c.dir.path().join("outro.js"),
            Destino::Projeto
        )
        .is_err()
    );

    let r = levar_edicao(
        &c.dados,
        Some(&p),
        PROJETO,
        "impacto",
        &arquivo,
        Destino::Projeto,
    )
    .unwrap();
    assert!(r.leitura.existe);
    assert!(r.leitura.texto.ends_with("// editado à mão\n"));
    // Partiu do padrão instalado: o padrão não mudou.
    assert!(!r.leitura.padrao_mudou);
    let v = impacto(&c, &p);
    assert_eq!(v.origem_efetiva, OrigemWorkflow::Projeto);
    assert_eq!(instalado(&v).situacao, Situacao::Ok);
    assert_eq!(instalado(&v).origem, Some(OrigemWorkflow::Projeto));
    assert_eq!(corpo_instalado(&v), r.leitura.texto);
}

#[test]
fn padrao_so_leitura_e_tipo_desconhecido() {
    let c = cenario();
    let l = ler(&c.dados, "impacto", None).unwrap();
    assert!(l.destino.is_none() && l.arquivo.is_none());
    assert_eq!(l.texto, l.padrao);
    assert!(!l.validacao.tem_erro());
    assert!(ler(&c.dados, "perguntar", None).is_err());
    assert!(validar_texto("impacto", "x").unwrap().tem_erro());
}
