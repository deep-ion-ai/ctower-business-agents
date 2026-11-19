//! O que mudou no código desde a última análise (#124), com uma cópia do sistema
//! fictício "Faturamento Exemplo" num repositório git temporário.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use cba_core::ambiente::Ambiente;
use cba_core::analise::{Analise, EstadoAfirmacao, EstadoTermo, Fluxo};
use cba_core::analises::NovaAnalise;
use cba_core::config::Config;
use cba_core::dados::DiretorioDados;
use cba_core::eventos::Mudanca;
use cba_kb::frontmatter::{FrontmatterRegra, Linhas, StatusRegra};
use cba_kb::{CaminhoRelativo, Documento};
use cba_tools::aprovacao::{OpcoesAprovacao, aprovar};
use cba_tools::mudancas::{
    AjusteDaProposta, Classe, Pedido, SituacaoArquivo, SituacaoProposta, confirmar_proposta,
    descartar_proposta, enunciado_da_regra, nota_de_mudancas, propostas, relatorio,
};
use cba_tools::revalidacao::revalidar_projeto;
use cba_tools::{ErroChamada, ErroFerramenta, registro_padrao};
use serde_json::{Value, json};

const PKG: &str = "db/03_pkg_cobranca.sql";
const SITUACAO: &str = "db/04_pkg_situacao.sql";
const LINHA_DA_MULTA: &str =
    "        RETURN ROUND(V_FATURA.VL_ORIGINAL * FN_PARAMETRO('PERC_MULTA_ATRASO') / 100, 2);";
const LINHA_NOVA: &str =
    "        RETURN ROUND(V_FATURA.VL_ORIGINAL * FN_PARAMETRO('PERC_MULTA_ATRASO_NOVA') / 100, 4);";
const ENUNCIADO: &str = "Passada a carência, a multa é de 2% sobre o valor original.";

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

fn git(dir: &Path, args: &[&str]) -> String {
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
            "-c",
            "tag.gpgsign=false",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(
        saida.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&saida.stderr)
    );
    String::from_utf8_lossy(&saida.stdout).trim().to_owned()
}

struct Cenario {
    _dir: tempfile::TempDir,
    codigo: PathBuf,
    amb: Ambiente,
    dados: DiretorioDados,
}

/// Cópia da fixture num repositório git de verdade, com o commit inicial na tag
/// `v1`, cadastrada, com uma base vazia.
fn cenario() -> Cenario {
    let dir = tempfile::tempdir().unwrap();
    let raiz = dir.path();
    let codigo = raiz.join("faturamento");
    copiar(&fixture(), &codigo);
    git(&codigo, &["init", "-q", "-b", "main"]);
    git(&codigo, &["add", "-A"]);
    git(&codigo, &["commit", "-q", "-m", "Versão inicial"]);
    git(&codigo, &["tag", "v1"]);

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
    Cenario {
        _dir: dir,
        codigo,
        amb,
        dados,
    }
}

impl Cenario {
    fn chamar(&self, nome: &str, args: Value) -> Result<Value, ErroFerramenta> {
        match registro_padrao().chamar(nome, Ok(&self.amb), Some(args)) {
            Ok(saida) => Ok(saida.dados.unwrap()),
            Err(ErroChamada::Ferramenta(e)) => Err(e),
            Err(e) => panic!("{e}"),
        }
    }

    /// A análise da multa aprovada: a regra (linhas 71-72 do pacote de cobrança) e
    /// o termo "Multa por atraso", com o commit da tag `v1` nas evidências.
    fn aprovar_a_multa(&self) -> (Analise, CaminhoRelativo) {
        let a = self
            .amb
            .analises
            .criar(NovaAnalise {
                fluxo: Fluxo::Perguntar,
                titulo: "Multa por atraso".into(),
                entrada: "Como é calculada a multa por atraso?".into(),
                analista: Some("Ana Exemplo".into()),
                base: None,
            })
            .unwrap();
        let evidencia = json!([{
            "arquivo": PKG,
            "linha_inicial": 71,
            "linha_final": 72,
            "trecho": "FN_PARAMETRO('PERC_MULTA_ATRASO') / 100, 2)"
        }]);
        self.chamar("iniciar_analise", json!({"analise": a.id}))
            .unwrap();
        self.chamar(
            "afirmar",
            json!({
                "analise": a.id,
                "secao": "resposta",
                "texto": ENUNCIADO,
                "confianca": "confirmado",
                "evidencias": evidencia,
            }),
        )
        .unwrap();
        self.chamar(
            "propor_termo",
            json!({
                "analise": a.id,
                "termo": "Multa por atraso",
                "definicao": "Valor único cobrado sobre a fatura paga depois da carência.",
                "area": "cobranca",
                "evidencias": evidencia,
            }),
        )
        .unwrap();
        self.chamar("concluir_analise", json!({"analise": a.id}))
            .unwrap();
        self.amb
            .analises
            .alterar(&a.id, Mudanca::Afirmacao, |x, _| {
                x.marcar_afirmacao(1, EstadoAfirmacao::Confirmada)?;
                x.decidir_termo(1, EstadoTermo::Aceito)
            })
            .unwrap();
        let aprovada = aprovar(
            &self.amb.base,
            &self.amb.analises,
            &a.id,
            &OpcoesAprovacao {
                area: Some("cobranca".into()),
                // A afirmação está na Resposta, que só vira regra se escolhida.
                regras: Some(BTreeSet::from([1])),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(aprovada.regras.len(), 1);
        let caminho = aprovada.regras[0].arquivo.clone();
        (self.amb.analises.carregar(&a.id).unwrap(), caminho)
    }

    fn arquivo(&self, relativo: &str) -> PathBuf {
        self.codigo.join(relativo)
    }

    fn trocar(&self, relativo: &str, de: &str, para: &str) {
        let caminho = self.arquivo(relativo);
        let texto = fs::read_to_string(&caminho).unwrap();
        assert!(texto.contains(de), "{de}");
        fs::write(&caminho, texto.replacen(de, para, 1)).unwrap();
    }

    fn inserir_no_inicio(&self, relativo: &str, linhas: &str) {
        let caminho = self.arquivo(relativo);
        let texto = fs::read_to_string(&caminho).unwrap();
        fs::write(&caminho, format!("{linhas}{texto}")).unwrap();
    }

    /// A multa muda (2% vira o parâmetro novo, com 4 casas), o pacote de situação
    /// ganha um comentário e uma pasta nova aparece; tudo na tag `v2`.
    fn mudar_e_commitar(&self) {
        self.trocar(PKG, LINHA_DA_MULTA, LINHA_NOVA);
        self.inserir_no_inicio(SITUACAO, "-- Situação revista na versão 2\n");
        fs::create_dir_all(self.arquivo("relatorios")).unwrap();
        fs::write(
            self.arquivo("relatorios/inadimplencia.sql"),
            "SELECT ID_FATURA FROM FATURA WHERE DT_VENCIMENTO < SYSDATE;\n",
        )
        .unwrap();
        git(&self.codigo, &["add", "-A"]);
        git(
            &self.codigo,
            &["commit", "-q", "-m", "Multa pelo parâmetro novo"],
        );
        git(&self.codigo, &["tag", "v2"]);
    }

    fn regra(&self, caminho: &CaminhoRelativo) -> Documento<FrontmatterRegra> {
        self.amb.kb.ler(caminho).unwrap()
    }
}

fn pedido(de: &str, ate: &str) -> Pedido {
    Pedido {
        de: Some(de.into()),
        ate: Some(ate.into()),
    }
}

#[test]
fn alterar_o_trecho_da_regra_entre_dois_commits_lista_a_regra_com_o_diff() {
    let c = cenario();
    let (analise, _) = c.aprovar_a_multa();
    c.mudar_e_commitar();

    let inicio = Instant::now();
    let r = relatorio(&c.amb, &pedido("v1", "v2")).unwrap();
    // Sem IA e offline: só o git local e a base.
    assert!(inicio.elapsed().as_secs() < 10, "{:?}", inicio.elapsed());
    assert_eq!(r.de.referencia, "v1");
    assert_eq!(
        r.de.commit.as_deref(),
        Some(git(&c.codigo, &["rev-parse", "v1"]).as_str())
    );
    assert_eq!(
        r.ate.commit.as_deref(),
        Some(git(&c.codigo, &["rev-parse", "v2"]).as_str())
    );
    assert_eq!(r.commits.len(), 1);
    assert_eq!(r.commits[0].assunto, "Multa pelo parâmetro novo");
    assert_eq!(r.arquivos_alterados, 3);
    assert!(r.problemas.is_empty(), "{:?}", r.problemas);

    // O arquivo citado, com quem o cita.
    assert_eq!(r.com_evidencias.len(), 1);
    let pkg = &r.com_evidencias[0];
    assert_eq!(pkg.arquivo.arquivo, PKG);
    assert_eq!(pkg.arquivo.situacao, SituacaoArquivo::Modificado);
    assert_eq!(pkg.regras.len(), 1);
    assert_eq!(pkg.termos, ["Multa por atraso"]);
    assert_eq!(pkg.analises, std::slice::from_ref(&analise.id));

    // A regra, com o diff do trecho citado.
    assert_eq!(r.regras.len(), 1);
    let regra = &r.regras[0];
    assert_eq!(regra.classe, Classe::Alterado);
    assert_eq!(regra.area, "cobranca");
    let trecho = &regra.trechos[0];
    assert_eq!(trecho.linhas, Linhas::novo(71, 72).unwrap());
    assert_eq!(trecho.linhas_novas, Linhas::novo(71, 72));
    assert!(
        trecho.diff.contains(&format!("-{LINHA_DA_MULTA}\n")),
        "{}",
        trecho.diff
    );
    assert!(
        trecho.diff.contains(&format!("+{LINHA_NOVA}\n")),
        "{}",
        trecho.diff
    );
    assert!(trecho.diff.starts_with("@@ "), "{}", trecho.diff);
    assert_eq!(r.regras_que_mudaram().count(), 1);

    // O termo e os itens da análise também.
    assert_eq!(r.termos.len(), 1);
    assert_eq!(r.termos[0].classe, Classe::Alterado);
    assert_eq!(r.analises.len(), 1);
    assert_eq!(r.analises[0].itens.len(), 2, "afirmação e termo");

    // Sem evidência: o pacote de situação fica na área das regras da pasta `db`;
    // a pasta nova, sem área, pelo nome.
    assert_eq!(r.sem_evidencias.len(), 2, "{:?}", r.sem_evidencias);
    let cobranca = &r.sem_evidencias[0];
    assert_eq!(cobranca.area.as_deref(), Some("cobranca"));
    assert_eq!(cobranca.arquivos[0].arquivo, SITUACAO);
    let pasta = &r.sem_evidencias[1];
    assert_eq!(pasta.area, None);
    assert_eq!(pasta.nome, "relatorios");
    assert_eq!(pasta.arquivos[0].situacao, SituacaoArquivo::Adicionado);

    // Exportável como nota de mudanças.
    let nota = nota_de_mudancas(&r, &[]);
    assert!(
        nota.contains("# O que mudou no código · faturamento-exemplo"),
        "{nota}"
    );
    assert!(nota.contains(&format!("### {} · ", regra.id)), "{nota}");
    assert!(nota.contains("```diff\n"), "{nota}");
    assert!(
        nota.contains("- Termo **Multa por atraso**: trecho alterado"),
        "{nota}"
    );
    assert!(nota.contains("### Pasta relatorios"), "{nota}");
    assert!(
        nota.contains(&format!("- `{SITUACAO}` (modificado)")),
        "{nota}"
    );

    // A ferramenta MCP devolve o mesmo relatório, e filtra por regra.
    let dados = c
        .chamar(
            "relatorio_de_mudancas",
            json!({"de": "v1", "ate": "v2", "regra": regra.id}),
        )
        .unwrap();
    assert_eq!(dados["regras"][0]["classe"], "alterado");
    assert_eq!(dados["regras"].as_array().unwrap().len(), 1);
}

#[test]
fn sem_de_compara_desde_a_ultima_analise_aprovada_ate_a_copia_de_trabalho() {
    let c = cenario();
    let (analise, _) = c.aprovar_a_multa();
    // Duas linhas novas no topo: o trecho só andou, na cópia de trabalho.
    c.inserir_no_inicio(PKG, "-- Revisão\n-- Sem mudança de regra\n");

    let r = relatorio(&c.amb, &Pedido::default()).unwrap();
    let de = r.de.analise.as_ref().expect("veio da análise aprovada");
    assert_eq!(de.id, analise.id);
    assert_eq!(
        r.de.commit.as_deref(),
        Some(git(&c.codigo, &["rev-parse", "v1"]).as_str())
    );
    assert_eq!(r.ate.referencia, "cópia de trabalho");
    assert_eq!(r.ate.commit, None);
    let regra = &r.regras[0];
    assert_eq!(regra.classe, Classe::Inalterado);
    assert_eq!(regra.trechos[0].linhas_novas, Linhas::novo(73, 74));
    assert!(regra.trechos[0].diff.is_empty());
    assert_eq!(r.regras_que_mudaram().count(), 0);

    // Quebrar a linha em duas é só reformatação.
    c.trocar(
        PKG,
        LINHA_DA_MULTA,
        "        RETURN ROUND(V_FATURA.VL_ORIGINAL *\n                     FN_PARAMETRO('PERC_MULTA_ATRASO') / 100, 2);",
    );
    let r = relatorio(&c.amb, &Pedido::default()).unwrap();
    assert_eq!(r.regras[0].classe, Classe::Reformatado);
    assert!(!r.regras[0].trechos[0].diff.is_empty());
    let nota = nota_de_mudancas(&r, &[]);
    assert!(nota.contains("## Regras com mudança só de forma"), "{nota}");

    // Arquivo novo fora do git também conta, na cópia de trabalho.
    fs::write(c.arquivo("db/06_rascunho.sql"), "-- rascunho\n").unwrap();
    let r = relatorio(&c.amb, &Pedido::default()).unwrap();
    assert!(
        r.sem_evidencias
            .iter()
            .flat_map(|g| &g.arquivos)
            .any(|a| a.arquivo == "db/06_rascunho.sql" && a.situacao == SituacaoArquivo::Adicionado)
    );
}

#[test]
fn referencias_invalidas_e_sem_analise_aprovada_sao_recusadas() {
    let c = cenario();
    let erro = relatorio(&c.amb, &Pedido::default()).unwrap_err();
    assert!(
        matches!(erro, ErroFerramenta::Recusa(ref m) if m.contains("informe")),
        "{erro}"
    );
    let erro = relatorio(&c.amb, &pedido("--output=x", "v1")).unwrap_err();
    assert!(matches!(erro, ErroFerramenta::Argumentos(_)), "{erro}");
    let erro = relatorio(&c.amb, &pedido("v9", "v1")).unwrap_err();
    assert!(
        matches!(erro, ErroFerramenta::Recusa(ref m) if m.contains("`v9` não existe")),
        "{erro}"
    );
}

#[test]
fn regra_atualizada_mantem_o_identificador_e_guarda_a_versao_anterior() {
    let c = cenario();
    let (_, caminho) = c.aprovar_a_multa();
    let antes = c.regra(&caminho);
    c.mudar_e_commitar();
    // A revalidação marca a regra; a confirmação do analista a devolve a
    // confirmada.
    revalidar_projeto(&c.amb, &c.dados).unwrap();
    assert_eq!(
        c.regra(&caminho).frontmatter.status,
        StatusRegra::ARevalidar
    );

    let id = antes.frontmatter.id.to_string();
    // O agente explica: o comportamento mudou, com a evidência nova.
    let explicacao = "A multa passou a usar o parâmetro novo, com quatro casas decimais.";
    let novo_texto = "Passada a carência, a multa usa o percentual do parâmetro PERC_MULTA_ATRASO_NOVA, com quatro casas.";
    let proposta = c
        .chamar(
            "propor_atualizacao_de_regra",
            json!({
                "regra": id,
                "comportamento_mudou": true,
                "explicacao": explicacao,
                "enunciado": novo_texto,
                "evidencias": [{
                    "arquivo": PKG,
                    "linha_inicial": 71,
                    "linha_final": 72,
                    "trecho": "FN_PARAMETRO('PERC_MULTA_ATRASO_NOVA') / 100, 4)"
                }]
            }),
        )
        .unwrap();
    assert_eq!(proposta["proposta"], 1);
    // A proposta não mexe na regra.
    assert_eq!(c.regra(&caminho).corpo, antes.corpo);

    // Comportamento mudou sem texto novo, evidência que não confere e regra
    // inexistente são recusados.
    let sem_texto = c.chamar(
        "propor_atualizacao_de_regra",
        json!({
            "regra": id, "comportamento_mudou": true, "explicacao": "Mudou.",
            "evidencias": [{"arquivo": PKG, "linha_inicial": 71, "linha_final": 72,
                            "trecho": "PERC_MULTA_ATRASO_NOVA"}]
        }),
    );
    assert!(matches!(sem_texto, Err(ErroFerramenta::Argumentos(_))));
    let velha = c.chamar(
        "propor_atualizacao_de_regra",
        json!({
            "regra": id, "comportamento_mudou": false, "explicacao": "Nada mudou.",
            "evidencias": [{"arquivo": PKG, "linha_inicial": 71, "linha_final": 72,
                            "trecho": "FN_PARAMETRO('PERC_MULTA_ATRASO') / 100, 2)"}]
        }),
    );
    assert!(matches!(velha, Err(ErroFerramenta::Recusa(_))), "{velha:?}");
    let inexistente = c.chamar(
        "propor_atualizacao_de_regra",
        json!({
            "regra": "RN-COB-999", "comportamento_mudou": false, "explicacao": "x",
            "evidencias": [{"arquivo": PKG, "linha_inicial": 72, "linha_final": 72,
                            "trecho": "PERC_MULTA_ATRASO_NOVA"}]
        }),
    );
    assert!(matches!(inexistente, Err(ErroFerramenta::Recusa(_))));

    let pendentes = propostas(&c.amb);
    assert_eq!(pendentes.len(), 1);
    assert_eq!(pendentes[0].situacao, SituacaoProposta::Pendente);
    // A nota mostra a explicação ao lado do diff.
    let r = relatorio(&c.amb, &pedido("v1", "v2")).unwrap();
    let nota = nota_de_mudancas(&r, &pendentes);
    assert!(nota.contains(explicacao), "{nota}");

    // O analista confirma, ajustando o texto.
    let ajustado = "Passada a carência, a multa é calculada pelo percentual do parâmetro PERC_MULTA_ATRASO_NOVA, arredondada em quatro casas.";
    let atualizada = confirmar_proposta(
        &c.amb,
        1,
        &AjusteDaProposta {
            enunciado: Some(ajustado.into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(atualizada.id, id);
    assert_eq!(atualizada.documento, caminho);
    assert_eq!(atualizada.versoes_anteriores, 1);

    let depois = c.regra(&caminho);
    let fm = &depois.frontmatter;
    assert_eq!(fm.id, antes.frontmatter.id, "mantém o identificador");
    assert_eq!(fm.status, StatusRegra::Confirmada);
    assert_eq!(fm.area, antes.frontmatter.area);
    assert_eq!(fm.origem, antes.frontmatter.origem);
    assert_ne!(fm.evidencias[0].hash, antes.frontmatter.evidencias[0].hash);
    assert_eq!(enunciado_da_regra(&depois.corpo), ajustado);
    // A versão anterior fica no histórico.
    assert_eq!(fm.historico.len(), 1);
    let v = &fm.historico[0];
    assert_eq!(v.titulo, antes.frontmatter.titulo);
    assert_eq!(v.enunciado, ENUNCIADO);
    assert_eq!(v.evidencias, antes.frontmatter.evidencias);
    assert_eq!(v.motivo.as_deref(), Some(explicacao));
    // A revalidação não volta a marcar a regra: a evidência é a do código atual.
    revalidar_projeto(&c.amb, &c.dados).unwrap();
    assert_eq!(
        c.regra(&caminho).frontmatter.status,
        StatusRegra::Confirmada
    );

    // Confirmada, a proposta não se confirma de novo; outra proposta pode ser
    // descartada sem mexer na regra.
    assert!(confirmar_proposta(&c.amb, 1, &AjusteDaProposta::default()).is_err());
    c.chamar(
        "propor_atualizacao_de_regra",
        json!({
            "regra": id, "comportamento_mudou": false, "explicacao": "Só conferência.",
            "evidencias": [{"arquivo": PKG, "linha_inicial": 72, "linha_final": 72,
                            "trecho": "PERC_MULTA_ATRASO_NOVA"}]
        }),
    )
    .unwrap();
    descartar_proposta(&c.amb, 2).unwrap();
    let todas = propostas(&c.amb);
    assert_eq!(
        todas.iter().map(|p| p.situacao).collect::<Vec<_>>(),
        [SituacaoProposta::Aplicada, SituacaoProposta::Descartada]
    );
    assert_eq!(c.regra(&caminho).frontmatter.historico.len(), 1);
}
