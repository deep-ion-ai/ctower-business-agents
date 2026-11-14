//! `cba-avaliacao`: avalia os fluxos contra o gabarito (docs/avaliacao.md).
//!
//! ```text
//! cargo run -p cba-avaliacao -- rodar [opções]       # com o Claude real
//! cargo run -p cba-avaliacao -- comparar --estado <A-NNNN.estado.json> --item <ID>
//! cargo run -p cba-avaliacao -- comparar --ata <R-NNNN.estado.json> --reuniao <ID>
//! ```

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::Duration;

use cba_avaliacao::ata::{Transcripts, avaliar_ata};
use cba_avaliacao::comparacao::{avaliar, nome_do_fluxo};
use cba_avaliacao::execucao::{
    Opcoes, escopo, executar, executar_reuniao, falas_da_reuniao, versao_do_claude,
};
use cba_avaliacao::gabarito::{Gabarito, Item, Reuniao};
use cba_avaliacao::longa::{self, OpcoesLonga, TamanhoTranscript};
use cba_avaliacao::relatorio::{Execucao, Parametros, Relatorio};
use cba_avaliacao::{gabarito_padrao, raiz_do_repositorio};
use cba_core::analise::{Analise, Fluxo};

const AJUDA: &str = "\
cba-avaliacao: avalia os fluxos do ctower contra o gabarito da fixture.

USO
  cba-avaliacao rodar [opções]
      Roda cada fluxo com o Claude Code real (usa o login de quem roda) numa cópia
      temporária da fixture, sem tocar na configuração do usuário, e grava o
      relatório (relatorio.md e relatorio.json) na pasta de saída.

      --modelo <m>            modelo do claude (padrão: sonnet)
      --orcamento <usd>       --max-budget-usd de cada execução (padrão: 1.5)
      --fluxos <lista>        perguntar,explicar,impacto,reuniao,reuniao-longa
                              (padrão: perguntar,explicar,impacto)
      --repeticoes <n>        execuções de cada fluxo (padrão: 1)
      --pergunta <id>         item do fluxo perguntar (padrão: P-FAT-001)
      --funcionalidade <id>   item do fluxo explicar (padrão: F-FAT-002)
      --demanda <id>          item do fluxo impacto (padrão: D-FAT-001)
      --reuniao <id>          reunião dos fluxos reuniao e reuniao-longa (padrão: REU-FAT-001)
      --orcamento-longa <usd> --max-budget-usd da reunião longa (padrão: 3)
      --longa-minutos <n>     duração da reunião longa sintética (padrão: 120)
      --longa-palavras <n>    palavras da reunião longa sintética (padrão: 25000)
      --saida <pasta>         pasta do relatório (padrão: target/avaliacao/<data-hora>)
      --cba <executável>      cba compilado (padrão: target/debug/cba)
      --claude <executável>   claude (padrão: claude, pelo PATH)
      --gabarito <arquivo>    gabarito (padrão: fixtures/gabaritos/faturamento-exemplo.yaml)
      --tempo-limite <s>      limite de cada execução, em segundos (padrão: 900)
      --permitir-edicao       libera Edit, Write e Bash para o agente
      --manter                mantém as pastas temporárias

  cba-avaliacao comparar --estado <A-NNNN.estado.json> --item <id> [--gabarito <arquivo>] [--saida <pasta>]
      Compara um estado já gravado com o item do gabarito, sem chamar o Claude.

  cba-avaliacao comparar --ata <R-NNNN.estado.json> --reuniao <id> [--longa] [--gabarito <arquivo>] [--saida <pasta>]
      Compara o estado de uma ata com a reunião do gabarito, sem chamar o Claude. As
      citações são conferidas de novo contra o transcript da fixture (com --longa,
      contra a reunião longa sintética, com --longa-minutos e --longa-palavras).

Sai com 0 sem falhas graves, 1 com falha grave e 2 em erro de uso.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let resultado = match args.first().map(String::as_str) {
        Some("rodar") => rodar(&args[1..]),
        Some("comparar") => comparar(&args[1..]),
        Some("-h" | "--help" | "ajuda") => {
            println!("{AJUDA}");
            return ExitCode::SUCCESS;
        }
        _ => Err(format!("comando ausente ou desconhecido\n\n{AJUDA}")),
    };
    match resultado {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(e) => {
            eprintln!("erro: {e}");
            ExitCode::from(2)
        }
    }
}

/// Opções `--nome valor` e chaves `--nome`.
struct Args<'a> {
    resto: &'a [String],
    chaves: &'static [&'static str],
}

impl<'a> Args<'a> {
    fn conferir(&self, aceitas: &[&str]) -> Result<(), String> {
        let mut i = 0;
        while i < self.resto.len() {
            let a = &self.resto[i];
            if self.chaves.contains(&a.as_str()) {
                i += 1;
            } else if aceitas.contains(&a.as_str()) {
                if i + 1 >= self.resto.len() {
                    return Err(format!("{a} precisa de um valor"));
                }
                i += 2;
            } else {
                return Err(format!("opção desconhecida: {a}"));
            }
        }
        Ok(())
    }

    fn valor(&self, nome: &str) -> Option<&'a str> {
        self.resto
            .iter()
            .position(|a| a == nome)
            .and_then(|i| self.resto.get(i + 1))
            .map(String::as_str)
    }

    fn chave(&self, nome: &str) -> bool {
        self.resto.iter().any(|a| a == nome)
    }
}

fn carregar_gabarito(args: &Args) -> Result<Gabarito, String> {
    let caminho = args
        .valor("--gabarito")
        .map(PathBuf::from)
        .unwrap_or_else(gabarito_padrao);
    Gabarito::carregar(&caminho).map_err(|e| e.to_string())
}

/// Tamanho da reunião longa pelas opções.
fn opcoes_da_longa(args: &Args) -> Result<OpcoesLonga, String> {
    let padrao = OpcoesLonga::default();
    Ok(OpcoesLonga {
        minutos: match args.valor("--longa-minutos") {
            Some(v) => v
                .parse()
                .ok()
                .filter(|n| *n >= 1)
                .ok_or("--longa-minutos precisa ser um inteiro a partir de 1")?,
            None => padrao.minutos,
        },
        palavras: match args.valor("--longa-palavras") {
            Some(v) => v
                .parse()
                .map_err(|_| "--longa-palavras precisa ser um inteiro")?,
            None => padrao.palavras,
        },
    })
}

fn comparar(resto: &[String]) -> Result<bool, String> {
    let args = Args {
        resto,
        chaves: &["--longa"],
    };
    args.conferir(&[
        "--estado",
        "--item",
        "--ata",
        "--reuniao",
        "--gabarito",
        "--saida",
        "--longa-minutos",
        "--longa-palavras",
    ])?;
    if args.valor("--ata").is_some() {
        return comparar_ata(&args);
    }
    let estado = args
        .valor("--estado")
        .ok_or("informe --estado (ou --ata)")?;
    let item = args.valor("--item").ok_or("informe --item")?;
    let gabarito = carregar_gabarito(&args)?;
    let item = gabarito.item(item).map_err(|e| e.to_string())?;
    let bytes = std::fs::read(estado).map_err(|e| format!("{estado}: {e}"))?;
    let analise: Analise =
        serde_json::from_slice(&bytes).map_err(|e| format!("{estado}: estado ilegível: {e}"))?;
    if analise.fluxo != item.fluxo {
        return Err(format!(
            "a análise é do fluxo {}, e o item {} é do fluxo {}",
            nome_do_fluxo(analise.fluxo),
            item.id,
            nome_do_fluxo(item.fluxo)
        ));
    }
    let avaliacao = avaliar(&analise, &item, &gabarito, &escopo(&gabarito)?);
    let relatorio = Relatorio::novo(
        None,
        vec![Execucao {
            fluxo: nome_do_fluxo(item.fluxo).to_owned(),
            item: item.id.clone(),
            repeticao: 1,
            avaliacao: Some(avaliacao),
            ata: None,
            transcript: None,
            arquivos_alterados: Vec::new(),
            claude: None,
            erro: None,
        }],
    );
    imprimir(&args, &relatorio)
}

fn imprimir(args: &Args, relatorio: &Relatorio) -> Result<bool, String> {
    if let Some(saida) = args.valor("--saida") {
        relatorio
            .gravar(Path::new(saida))
            .map_err(|e| format!("{saida}: {e}"))?;
    }
    print!("{}", relatorio.markdown());
    Ok(relatorio.sem_falhas_graves)
}

fn comparar_ata(args: &Args) -> Result<bool, String> {
    let caminho = args.valor("--ata").ok_or("informe --ata")?;
    let id = args.valor("--reuniao").ok_or("informe --reuniao")?;
    let gabarito = carregar_gabarito(args)?;
    let reuniao: &Reuniao = gabarito.reuniao(id).map_err(|e| e.to_string())?;
    let texto = std::fs::read_to_string(caminho).map_err(|e| format!("{caminho}: {e}"))?;
    let estado: cba_docs::EstadoAta =
        serde_json::from_str(&texto).map_err(|e| format!("{caminho}: estado ilegível: {e}"))?;
    let original = falas_da_reuniao(&gabarito, reuniao)?.falas;
    let longa = args
        .chave("--longa")
        .then(|| opcoes_da_longa(args))
        .transpose()?;
    let lido = match longa {
        Some(o) => longa::gerar(&original, o),
        None => original.clone(),
    };
    let ata = avaliar_ata(
        &estado,
        reuniao,
        Some(Transcripts {
            original: &original,
            lido: &lido,
        }),
    );
    let relatorio = Relatorio::novo(
        None,
        vec![Execucao {
            fluxo: if longa.is_some() {
                "reuniao-longa"
            } else {
                "reuniao"
            }
            .to_owned(),
            item: reuniao.id.clone(),
            repeticao: 1,
            avaliacao: None,
            ata: Some(ata),
            transcript: Some(TamanhoTranscript::de(&lido)),
            arquivos_alterados: Vec::new(),
            claude: None,
            erro: None,
        }],
    );
    imprimir(args, &relatorio)
}

fn executavel_padrao_do_cba() -> PathBuf {
    let nome = if cfg!(windows) { "cba.exe" } else { "cba" };
    raiz_do_repositorio()
        .join("target")
        .join("debug")
        .join(nome)
}

fn carimbo() -> String {
    let ms = cba_core::tempo::agora_ms();
    let s = (ms / 1000) % 86_400;
    format!(
        "{}-{:02}{:02}{:02}",
        cba_core::tempo::data_utc(ms),
        s / 3600,
        (s / 60) % 60,
        s % 60
    )
}

fn commit_do_repositorio() -> Option<String> {
    let saida = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(raiz_do_repositorio())
        .output()
        .ok()?;
    saida
        .status
        .success()
        .then(|| String::from_utf8_lossy(&saida.stdout).trim().to_owned())
}

fn rodar(resto: &[String]) -> Result<bool, String> {
    let args = Args {
        resto,
        chaves: &["--permitir-edicao", "--manter"],
    };
    args.conferir(&[
        "--modelo",
        "--orcamento",
        "--fluxos",
        "--repeticoes",
        "--pergunta",
        "--funcionalidade",
        "--demanda",
        "--saida",
        "--cba",
        "--claude",
        "--gabarito",
        "--tempo-limite",
        "--reuniao",
        "--orcamento-longa",
        "--longa-minutos",
        "--longa-palavras",
    ])?;
    let gabarito = carregar_gabarito(&args)?;
    let modelo = args.valor("--modelo").unwrap_or("sonnet").to_owned();
    let orcamento: f64 = args
        .valor("--orcamento")
        .unwrap_or("1.5")
        .parse()
        .map_err(|_| "--orcamento precisa ser um número (US$, com ponto)")?;
    if orcamento.is_nan() || orcamento <= 0.0 {
        return Err("--orcamento precisa ser maior que zero".into());
    }
    let repeticoes: u32 = args
        .valor("--repeticoes")
        .unwrap_or("1")
        .parse()
        .ok()
        .filter(|n| *n >= 1)
        .ok_or("--repeticoes precisa ser um inteiro a partir de 1")?;
    let tempo_limite: u64 = args
        .valor("--tempo-limite")
        .unwrap_or("900")
        .parse()
        .map_err(|_| "--tempo-limite precisa ser um inteiro (segundos)")?;

    let orcamento_longa: f64 = args
        .valor("--orcamento-longa")
        .unwrap_or("3")
        .parse()
        .map_err(|_| "--orcamento-longa precisa ser um número (US$, com ponto)")?;
    if orcamento_longa.is_nan() || orcamento_longa <= 0.0 {
        return Err("--orcamento-longa precisa ser maior que zero".into());
    }
    let longa = opcoes_da_longa(&args)?;
    let reuniao = gabarito
        .reuniao(args.valor("--reuniao").unwrap_or("REU-FAT-001"))
        .map_err(|e| e.to_string())?;

    let mut itens: Vec<Item> = Vec::new();
    // Fluxo `reuniao`: (normal, longa).
    let mut reunioes = (false, false);
    for fluxo in args
        .valor("--fluxos")
        .unwrap_or("perguntar,explicar,impacto")
        .split(',')
        .map(str::trim)
        .filter(|f| !f.is_empty())
    {
        let (opcao, padrao, esperado) = match fluxo {
            "perguntar" => ("--pergunta", "P-FAT-001", Fluxo::Perguntar),
            "explicar" => ("--funcionalidade", "F-FAT-002", Fluxo::Explicar),
            "impacto" => ("--demanda", "D-FAT-001", Fluxo::Impacto),
            "reuniao" => {
                reunioes.0 = true;
                continue;
            }
            "reuniao-longa" => {
                reunioes.1 = true;
                continue;
            }
            outro => {
                return Err(format!(
                    "fluxo desconhecido: {outro} (use perguntar, explicar, impacto, reuniao ou \
                     reuniao-longa)"
                ));
            }
        };
        let id = args.valor(opcao).unwrap_or(padrao);
        let item = gabarito.item(id).map_err(|e| e.to_string())?;
        if item.fluxo != esperado {
            return Err(format!("{opcao} {id}: o item é de outro fluxo"));
        }
        itens.push(item);
    }
    if itens.is_empty() && reunioes == (false, false) {
        return Err("nenhum fluxo escolhido".into());
    }

    let cba = args
        .valor("--cba")
        .map(PathBuf::from)
        .unwrap_or_else(executavel_padrao_do_cba);
    let cba = dunce::canonicalize(&cba).map_err(|_| {
        format!(
            "{} não existe. Compile antes: (cd ui && npm ci && npm run build) && cargo build -p cba-app",
            cba.display()
        )
    })?;
    let claude = PathBuf::from(args.valor("--claude").unwrap_or("claude"));
    let versao = versao_do_claude(&claude).ok_or(
        "o claude não foi encontrado; instale o Claude Code e faça login (claude auth status)",
    )?;
    let saida = args.valor("--saida").map(PathBuf::from).unwrap_or_else(|| {
        raiz_do_repositorio()
            .join("target")
            .join("avaliacao")
            .join(carimbo())
    });

    let opcoes = Opcoes {
        claude,
        cba,
        modelo: modelo.clone(),
        orcamento_usd: orcamento,
        permitir_edicao: args.chave("--permitir-edicao"),
        tempo_limite: Duration::from_secs(tempo_limite),
        manter: args.chave("--manter"),
    };
    let normais = (itens.len() as u32 + u32::from(reunioes.0)) * repeticoes;
    let longas = u32::from(reunioes.1) * repeticoes;
    let total = normais + longas;
    eprintln!(
        "{total} execução(ões) com {modelo} ({versao}); orçamento de US$ {orcamento:.2} cada{}, até cerca de US$ {:.2} no total.",
        if longas > 0 {
            format!(" (US$ {orcamento_longa:.2} na reunião longa)")
        } else {
            String::new()
        },
        orcamento * f64::from(normais) + orcamento_longa * f64::from(longas)
    );

    let mut execucoes = Vec::new();
    for item in &itens {
        for r in 1..=repeticoes {
            let fluxo = nome_do_fluxo(item.fluxo);
            eprintln!("- {fluxo} {} (repetição {r})...", item.id);
            let pasta = saida.join(format!("{fluxo}-{}-{r}", item.id));
            let e = executar(&gabarito, item, r, &opcoes, &pasta);
            let custo = e
                .claude
                .as_ref()
                .and_then(|c| c.custo_usd)
                .map_or("?".to_owned(), |c| format!("{c:.2}"));
            let regras = e.avaliacao.as_ref().map_or("-".to_owned(), |a| {
                format!("{} de {}", a.regras_encontradas, a.regras_esperadas)
            });
            let falhas = e.falhas();
            eprintln!(
                "  regras {regras}, custo US$ {custo}{}",
                if falhas.is_empty() {
                    String::new()
                } else {
                    format!(", falhas: {}", falhas.join("; "))
                }
            );
            execucoes.push(e);
        }
    }
    for (ligado, longa) in [(reunioes.0, None), (reunioes.1, Some(longa))] {
        if !ligado {
            continue;
        }
        let opcoes = Opcoes {
            orcamento_usd: if longa.is_some() {
                orcamento_longa
            } else {
                orcamento
            },
            ..opcoes.clone()
        };
        for r in 1..=repeticoes {
            let fluxo = if longa.is_some() {
                "reuniao-longa"
            } else {
                "reuniao"
            };
            eprintln!("- {fluxo} {} (repetição {r})...", reuniao.id);
            let pasta = saida.join(format!("{fluxo}-{}-{r}", reuniao.id));
            let e = executar_reuniao(&gabarito, reuniao, longa, r, &opcoes, &pasta);
            let custo = e
                .claude
                .as_ref()
                .and_then(|c| c.custo_usd)
                .map_or("?".to_owned(), |c| format!("{c:.2}"));
            let resultado = e.ata.as_ref().map_or("-".to_owned(), |a| {
                format!(
                    "papéis {} de {}, itens {} de {}",
                    a.papeis_acertados, a.papeis_esperados, a.itens_atendidos, a.itens_esperados
                )
            });
            let falhas = e.falhas();
            eprintln!(
                "  {resultado}, custo US$ {custo}{}",
                if falhas.is_empty() {
                    String::new()
                } else {
                    format!(", falhas: {}", falhas.join("; "))
                }
            );
            execucoes.push(e);
        }
    }

    let mut nomes: Vec<String> = itens.iter().map(|i| i.id.clone()).collect();
    if reunioes.0 {
        nomes.push(reuniao.id.clone());
    }
    if reunioes.1 {
        nomes.push(format!("{} (longa)", reuniao.id));
    }
    let relatorio = Relatorio::novo(
        Some(Parametros {
            data: carimbo(),
            modelo,
            orcamento_por_execucao_usd: orcamento,
            repeticoes,
            itens: nomes,
            permitir_edicao: opcoes.permitir_edicao,
            versao_do_claude: Some(versao),
            commit: commit_do_repositorio(),
            orcamento_reuniao_longa_usd: reunioes.1.then_some(orcamento_longa),
            reuniao_longa: reunioes.1.then_some(longa),
        }),
        execucoes,
    );
    relatorio
        .gravar(&saida)
        .map_err(|e| format!("{}: {e}", saida.display()))?;
    eprintln!(
        "Relatório em {} (custo total US$ {:.2}).",
        saida.join("relatorio.md").display(),
        relatorio.custo_total_usd
    );
    Ok(relatorio.sem_falhas_graves)
}
