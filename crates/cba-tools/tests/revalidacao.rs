//! Revalidação das evidências por hash (#12) com uma cópia do sistema fictício
//! "Faturamento Exemplo" numa pasta temporária.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use cba_core::ambiente::Ambiente;
use cba_core::analise::{
    Analise, Confianca, EstadoAfirmacao, EstadoEvidencia, Evidencia, Fluxo, NovaAfirmacao,
};
use cba_core::analises::NovaAnalise;
use cba_core::config::Config;
use cba_core::dados::DiretorioDados;
use cba_core::eventos::Mudanca;
use cba_kb::frontmatter::{FrontmatterRegra, Linhas, StatusRegra, TipoDocumento};
use cba_kb::{CaminhoRelativo, Documento, Id};
use cba_tools::evidencias::{EvidenciaCitada, hash_das_linhas, validar};
use cba_tools::revalidacao::{Origem, Relatorio, revalidar_projeto};

const COMMIT_1: &str = "1111111111111111111111111111111111111111";
const COMMIT_2: &str = "2222222222222222222222222222222222222222";
const PKG: &str = "db/03_pkg_cobranca.sql";
const JAVA: &str = "backend/src/main/java/exemplo/faturamento/EncargosService.java";

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

struct Cenario {
    _dir: tempfile::TempDir,
    codigo: PathBuf,
    dados: DiretorioDados,
    amb: Ambiente,
}

/// Cópia da fixture num repositório git falso (só `HEAD` e a ref), cadastrada, com
/// uma base vazia.
fn cenario() -> Cenario {
    let dir = tempfile::tempdir().unwrap();
    let raiz = dir.path();
    let codigo = raiz.join("faturamento");
    copiar(&fixture(), &codigo);
    fs::create_dir_all(codigo.join(".git/refs/heads")).unwrap();
    fs::write(codigo.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
    fs::write(codigo.join(".git/refs/heads/main"), format!("{COMMIT_1}\n")).unwrap();

    let dados = DiretorioDados::em(raiz.join("dados"));
    let projeto = cba_index::Projeto::novo("Faturamento Exemplo", [&codigo]).unwrap();
    cba_index::Cadastro::em(dados.projetos())
        .cadastrar(&projeto)
        .unwrap();
    cba_kb::Base::criar(raiz.join("base")).unwrap();
    Config {
        base: Some(raiz.join("base")),
    }
    .gravar(&dados)
    .unwrap();
    let amb = Ambiente::do_diretorio(&codigo, &dados).unwrap();
    Cenario {
        _dir: dir,
        codigo,
        dados,
        amb,
    }
}

impl Cenario {
    fn evidencia(&self, arquivo: &str, inicial: u32, final_: u32, trecho: &str) -> Evidencia {
        validar(
            &self.amb.escopo,
            &EvidenciaCitada {
                arquivo: arquivo.into(),
                linha_inicial: inicial,
                linha_final: final_,
                trecho: trecho.into(),
            },
        )
        .unwrap()
    }

    fn analise(&self, titulo: &str) -> Id {
        self.amb
            .analises
            .criar(NovaAnalise {
                fluxo: Fluxo::Perguntar,
                titulo: titulo.into(),
                entrada: "Como são calculados os encargos por atraso?".into(),
                analista: None,
                base: None,
            })
            .unwrap()
            .id
    }

    fn afirmar(&self, id: &Id, evidencias: Vec<Evidencia>) -> u32 {
        self.amb
            .analises
            .alterar(id, Mudanca::Afirmacao, |a, agora| {
                a.registrar_afirmacao(
                    NovaAfirmacao {
                        secao: "resposta".into(),
                        texto: "Afirmação de exemplo.".into(),
                        confianca: Confianca::Confirmado,
                        evidencias,
                        sessao: None,
                    },
                    agora,
                )
            })
            .unwrap()
            .0
    }

    fn regra(&self, id: &str, evidencias: &[Evidencia]) -> CaminhoRelativo {
        let id = Id::analisar(id).unwrap();
        let caminho = id.arquivo();
        let doc = Documento::novo(
            FrontmatterRegra {
                tipo: TipoDocumento::Regra,
                id,
                titulo: "Regra de exemplo".into(),
                area: "cobranca".into(),
                status: StatusRegra::Confirmada,
                evidencias: evidencias
                    .iter()
                    .map(|e| cba_kb::frontmatter::Evidencia {
                        arquivo: e.arquivo.clone(),
                        linhas: e.linhas,
                        hash: e.hash.clone(),
                        commit: e.commit.clone(),
                    })
                    .collect(),
                origem: None,
            },
            "# Regra de exemplo\n",
        );
        self.amb.kb.gravar(&caminho, &doc).unwrap();
        caminho
    }

    fn status(&self, regra: &CaminhoRelativo) -> StatusRegra {
        self.amb
            .kb
            .ler::<FrontmatterRegra>(regra)
            .unwrap()
            .frontmatter
            .status
    }

    fn carregar(&self, id: &Id) -> Analise {
        self.amb.analises.carregar(id).unwrap()
    }

    fn revalidar(&self) -> Relatorio {
        revalidar_projeto(&self.amb, &self.dados).unwrap()
    }

    fn inventariar(&self) {
        cba_index::Cadastro::em(self.dados.projetos())
            .inventariar(&self.amb.projeto.id)
            .unwrap();
    }

    fn arquivo(&self, relativo: &str) -> PathBuf {
        self.codigo.join(relativo)
    }

    /// Troca a linha `n` (a partir de 1) do arquivo.
    fn trocar_linha(&self, relativo: &str, n: usize, nova: &str) {
        let caminho = self.arquivo(relativo);
        let texto = fs::read_to_string(&caminho).unwrap();
        let mut linhas: Vec<&str> = texto.lines().collect();
        linhas[n - 1] = nova;
        fs::write(&caminho, linhas.join("\n") + "\n").unwrap();
    }

    fn novo_commit(&self) {
        fs::write(
            self.codigo.join(".git/refs/heads/main"),
            format!("{COMMIT_2}\n"),
        )
        .unwrap();
    }
}

#[test]
fn alterar_uma_linha_marca_exatamente_o_afetado_e_desfazer_devolve_o_estado() {
    let c = cenario();
    let multa = c.evidencia(PKG, 71, 72, "FN_PARAMETRO('PERC_MULTA_ATRASO') / 100, 2)");
    let carencia = c.evidencia(
        PKG,
        67,
        69,
        "IF V_DIAS <= FN_PARAMETRO('DIAS_CARENCIA') THEN",
    );
    let java = c.evidencia(JAVA, 28, 29, "PKG_COBRANCA.FN_CALCULAR_JUROS");
    assert_eq!(multa.commit.as_deref(), Some(COMMIT_1));

    let a1 = c.analise("Multa por atraso");
    let n_multa = c.afirmar(&a1, vec![multa.clone()]);
    let n_carencia = c.afirmar(&a1, vec![carencia.clone()]);
    let n_misto = c.afirmar(&a1, vec![java.clone(), multa.clone()]);
    // O analista confirma a da multa: o estado anterior volta depois.
    c.amb
        .analises
        .alterar(&a1, Mudanca::Afirmacao, |a, _| {
            a.marcar_afirmacao(n_multa, EstadoAfirmacao::Confirmada)
        })
        .unwrap();
    let a2 = c.analise("Juros");
    c.afirmar(&a2, vec![java.clone(), carencia.clone()]);

    let rn_multa = c.regra("RN-COB-001", std::slice::from_ref(&multa));
    let rn_carencia = c.regra("RN-COB-002", &[carencia.clone(), java.clone()]);
    let rn_obsoleta = c.regra("RN-COB-003", std::slice::from_ref(&multa));
    let mut doc: Documento<FrontmatterRegra> = c.amb.kb.ler(&rn_obsoleta).unwrap();
    doc.frontmatter.status = StatusRegra::Obsoleta;
    c.amb.kb.gravar(&rn_obsoleta, &doc).unwrap();

    c.inventariar();
    let inicial = c.revalidar();
    assert!(!inicial.mudou(), "{inicial:#?}");
    assert!(inicial.desatualizadas.is_empty());
    // Regras não obsoletas (3) e análises (4 + 2).
    assert_eq!(inicial.evidencias, 9);
    let a1_antes = c.carregar(&a1);
    let a2_antes = c.carregar(&a2);
    let rn_multa_antes = fs::read_to_string(c.amb.kb.caminho(&rn_multa)).unwrap();
    let pkg_original = fs::read(c.arquivo(PKG)).unwrap();

    // Mudar só a indentação não desatualiza nada.
    c.trocar_linha(PKG, 67, "IF   V_DIAS <= FN_PARAMETRO('DIAS_CARENCIA') THEN");
    let r = c.revalidar();
    assert!(!r.mudou(), "{r:#?}");

    // A multa passa a 3%: muda a linha 72.
    c.novo_commit();
    c.trocar_linha(
        PKG,
        72,
        "        RETURN ROUND(V_FATURA.VL_ORIGINAL * 3 / 100, 2);",
    );
    let r = c.revalidar();
    assert!(r.mudou());
    assert_eq!(r.regras.len(), 1, "{r:#?}");
    assert_eq!(r.regras[0].documento, rn_multa);
    assert_eq!(r.regras[0].antes, StatusRegra::Confirmada);
    assert_eq!(r.regras[0].depois, StatusRegra::ARevalidar);
    assert_eq!(c.status(&rn_multa), StatusRegra::ARevalidar);
    assert_eq!(c.status(&rn_carencia), StatusRegra::Confirmada);
    assert_eq!(c.status(&rn_obsoleta), StatusRegra::Obsoleta);

    assert_eq!(r.analises.len(), 1);
    assert_eq!(r.analises[0].id, a1);
    assert_eq!(r.analises[0].afirmacoes_marcadas, [n_multa, n_misto]);
    let a1_depois = c.carregar(&a1);
    let estado = |a: &Analise, n: u32| {
        a.afirmacoes
            .iter()
            .find(|x| x.numero == n)
            .unwrap()
            .estado
            .clone()
    };
    assert_eq!(estado(&a1_depois, n_multa), EstadoAfirmacao::ARevalidar);
    assert_eq!(estado(&a1_depois, n_misto), EstadoAfirmacao::ARevalidar);
    assert_eq!(estado(&a1_depois, n_carencia), EstadoAfirmacao::Registrada);
    let estados: Vec<EstadoEvidencia> = a1_depois
        .afirmacoes
        .iter()
        .flat_map(|a| a.evidencias.iter().map(|e| e.estado))
        .collect();
    assert_eq!(
        estados,
        [
            EstadoEvidencia::ARevalidar,
            EstadoEvidencia::Valida,
            EstadoEvidencia::Valida,
            EstadoEvidencia::ARevalidar
        ]
    );
    assert_eq!(c.carregar(&a2), a2_antes);

    // Desatualizadas: a regra e as duas afirmações, todas novas, com os commits.
    assert_eq!(r.desatualizadas.len(), 3, "{r:#?}");
    for e in &r.desatualizadas {
        assert!(e.nova);
        assert_eq!(e.arquivo.as_str(), PKG);
        assert_eq!(e.linhas, Linhas::novo(71, 72).unwrap());
        assert_eq!(e.commit_antigo.as_deref(), Some(COMMIT_1));
        assert_eq!(e.commit_atual.as_deref(), Some(COMMIT_2));
        assert_eq!(e.motivo.as_deref(), Some("o trecho citado mudou"));
    }
    assert!(matches!(&r.desatualizadas[0].origem, Origem::Regra { id, .. } if id == "RN-COB-001"));

    // Revalidar de novo sem mudança: continua desatualizado, nada é novo.
    let r = c.revalidar();
    assert!(!r.mudou(), "{r:#?}");
    assert_eq!(r.desatualizadas.len(), 3);
    assert!(r.desatualizadas.iter().all(|e| !e.nova));

    // Desfazer a alteração devolve o estado anterior.
    fs::write(c.arquivo(PKG), &pkg_original).unwrap();
    let r = c.revalidar();
    assert_eq!(r.regras.len(), 1);
    assert_eq!(r.regras[0].depois, StatusRegra::Confirmada);
    assert_eq!(r.analises[0].afirmacoes_restauradas, [n_multa, n_misto]);
    assert_eq!(r.restauradas.len(), 2);
    assert!(r.desatualizadas.is_empty());
    assert_eq!(
        fs::read_to_string(c.amb.kb.caminho(&rn_multa)).unwrap(),
        rn_multa_antes
    );
    let a1_restaurada = c.carregar(&a1);
    assert_eq!(a1_restaurada.afirmacoes, a1_antes.afirmacoes);
    assert_eq!(estado(&a1_restaurada, n_multa), EstadoAfirmacao::Confirmada);
}

#[test]
fn arquivo_removido_desatualiza_com_o_motivo() {
    let c = cenario();
    let java = c.evidencia(JAVA, 28, 29, "PKG_COBRANCA.FN_CALCULAR_JUROS");
    let rn = c.regra("RN-COB-001", &[java]);
    let original = fs::read(c.arquivo(JAVA)).unwrap();
    fs::remove_file(c.arquivo(JAVA)).unwrap();
    let r = c.revalidar();
    assert_eq!(c.status(&rn), StatusRegra::ARevalidar);
    assert_eq!(r.desatualizadas.len(), 1);
    let motivo = r.desatualizadas[0].motivo.as_deref().unwrap();
    assert!(motivo.contains("não encontrado"), "{motivo}");

    // Arquivo truncado: o intervalo passa do fim.
    fs::write(c.arquivo(JAVA), "package exemplo.faturamento;\n").unwrap();
    let r = c.revalidar();
    assert_eq!(
        r.desatualizadas[0].motivo.as_deref(),
        Some("o intervalo passa do fim do arquivo")
    );

    fs::write(c.arquivo(JAVA), original).unwrap();
    c.revalidar();
    assert_eq!(c.status(&rn), StatusRegra::Confirmada);
}

/// Todas as linhas não vazias dos arquivos de texto da fixture, como evidências de
/// uma linha, com o hash calculado do jeito do registro.
fn evidencias_de_uma_linha(c: &Cenario) -> Vec<Evidencia> {
    let mut arquivos = Vec::new();
    let mut pendentes = vec![c.codigo.clone()];
    while let Some(pasta) = pendentes.pop() {
        for e in fs::read_dir(pasta).unwrap().flatten() {
            if e.file_name() == ".git" {
                continue;
            }
            if e.file_type().unwrap().is_dir() {
                pendentes.push(e.path());
            } else {
                arquivos.push(e.path());
            }
        }
    }
    arquivos.sort();
    let mut saida = Vec::new();
    for caminho in arquivos {
        let relativo = c.amb.escopo.relativo(&caminho).unwrap();
        if c.amb.escopo.resolver(&relativo).is_err() {
            continue;
        }
        let Ok(texto) = fs::read_to_string(&caminho) else {
            continue;
        };
        for (i, linha) in texto.lines().enumerate() {
            if linha.trim().is_empty() {
                continue;
            }
            let linhas = Linhas::novo(i as u32 + 1, i as u32 + 1).unwrap();
            saida.push(Evidencia {
                arquivo: CaminhoRelativo::novo(&relativo).unwrap(),
                linhas,
                trecho: linha.trim().to_owned(),
                hash: hash_das_linhas(&texto, linhas).unwrap(),
                commit: Some(COMMIT_1.into()),
                estado: EstadoEvidencia::Valida,
            });
        }
    }
    saida
}

#[test]
fn mil_evidencias_em_menos_de_cinco_segundos() {
    let c = cenario();
    let disponiveis = evidencias_de_uma_linha(&c);
    assert!(disponiveis.len() > 500, "{}", disponiveis.len());
    let mil: Vec<Evidencia> = disponiveis.iter().cycle().take(1000).cloned().collect();

    // Metade em 50 regras de 10 evidências; metade em 50 afirmações de uma análise.
    let (das_regras, das_analises) = mil.split_at(500);
    for (i, grupo) in das_regras.chunks(10).enumerate() {
        c.regra(&format!("RN-COB-{:03}", i + 1), grupo);
    }
    let a = c.analise("Muitas evidências");
    for grupo in das_analises.chunks(10) {
        c.afirmar(&a, grupo.to_vec());
    }

    // Sem inventário nem cache: lê cada arquivo citado uma vez.
    let inicio = Instant::now();
    let frio = c.revalidar();
    let tempo_frio = inicio.elapsed();
    assert_eq!(frio.evidencias, 1000);
    assert!(frio.desatualizadas.is_empty(), "{:#?}", frio.desatualizadas);
    assert!(frio.arquivos_lidos > 0);

    // Com o inventário e o cache: nenhum arquivo é lido.
    c.inventariar();
    let inicio = Instant::now();
    let quente = c.revalidar();
    let tempo_quente = inicio.elapsed();
    assert_eq!(quente.arquivos_lidos, 0, "{quente:#?}");
    assert_eq!(quente.arquivos_reaproveitados, frio.arquivos_lidos);

    // Uma linha alterada, sem inventariar de novo: o inventário velho não engana.
    c.trocar_linha(PKG, 72, "        RETURN 0;");
    let inicio = Instant::now();
    let alterado = c.revalidar();
    let tempo_alterado = inicio.elapsed();
    assert_eq!(alterado.arquivos_lidos, 1);
    let esperadas = mil
        .iter()
        .filter(|e| e.arquivo.as_str() == PKG && e.linhas.inicio == 72)
        .count();
    assert!(esperadas > 0);
    assert_eq!(alterado.desatualizadas.len(), esperadas);

    eprintln!(
        "revalidação de 1000 evidências: {tempo_frio:?} sem cache, {tempo_quente:?} com \
         inventário e cache, {tempo_alterado:?} com um arquivo alterado"
    );
    for t in [tempo_frio, tempo_quente, tempo_alterado] {
        assert!(t.as_secs_f64() < 5.0, "{t:?}");
    }
}
