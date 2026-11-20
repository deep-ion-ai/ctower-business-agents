//! Sincronização do repositório de docs (#277): traz a documentação publicada de
//! volta para a base, como fonte externa rotulada.
//!
//! [`sincronizar`] faz, nesta ordem:
//!
//! 1. resolve o destino da documentação do projeto (#274) e o clone dele;
//! 2. atualiza o clone com [`cba_repos::escrita::atualizar_clone`] (fetch e
//!    avanço da branch base só por fast-forward, token do chaveiro só no ambiente
//!    do git);
//! 3. lista os `.md` da subpasta no commit da branch base do remoto, direto do
//!    banco de objetos ([`LeitorDeCommit`]): o conteúdo é o do commit, mesmo com o
//!    clone noutra branch;
//! 4. importa para `externos/<repositório>/` ([`cba_kb::externos::importar`]),
//!    pulando os caminhos com padrão de segredo do inventário
//!    ([`cba_index::Exclusoes`]) e mascarando segredos e dados pessoais;
//! 5. sincroniza o índice da base.
//!
//! Falha de rede, credencial ou clone fica registrada no estado
//! (`externos/.sincronizacao.json`) e as cópias anteriores continuam valendo.
//!
//! Quando sincroniza:
//!
//! - **sob demanda**, pelo botão do cartão do destino no app;
//! - **ao iniciar uma análise** ([`ao_iniciar_analise`], chamado por
//!   `iniciar_analise`): se a última tentativa tem mais de
//!   [`INTERVALO_MINIMO`], a sincronização roda em segundo plano, sem atrasar a
//!   análise; se a cópia está desatualizada (mais de [`LIMITE_DE_IDADE`], nunca
//!   sincronizada ou com a última tentativa falha), o agente recebe um aviso.
//!
//! Para consultar a fonte externa (a ferramenta `conhecimento` e o contexto da
//! análise, #278), use o índice com [`cba_kb::indice::Externos`] e a origem em
//! [`cba_kb::indice::Resultado::externo`].

use std::thread;
use std::time::Duration;

use cba_core::ambiente::Ambiente;
use cba_index::Motivo;
use cba_kb::externos::{self, ArquivoExterno, EstadoExternos, Fonte, Importacao};
use cba_kb::indice::{Indice, Sincronizacao};
use cba_repos::chaveiro::token_do_repositorio;
use cba_repos::demanda::Ferramentas;
use cba_repos::escrita::{LeitorDeCommit, atualizar_clone};
use serde::Serialize;

/// Cópia mais velha que isto gera aviso ao iniciar uma análise.
pub const LIMITE_DE_IDADE: Duration = Duration::from_secs(24 * 3600);

/// Ao iniciar uma análise, não sincroniza de novo se a última tentativa tem
/// menos que isto (várias análises seguidas não buscam o remoto a cada uma).
pub const INTERVALO_MINIMO: Duration = Duration::from_secs(10 * 60);

/// Por que a sincronização não aconteceu.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ErroSincronizacao {
    #[error("o projeto não tem destino da documentação cadastrado")]
    SemDestino,
    #[error(
        "o repositório da documentação ({0}) ainda não tem clone local: clone-o no cadastro \
         do projeto"
    )]
    SemClone(String),
    #[error("outra sincronização do repositório de docs está em andamento")]
    EmAndamento,
    /// Rede, credencial, clone ou gravação na base. Registrado no estado.
    #[error("{0}")]
    Falhou(String),
}

/// Resultado de [`sincronizar`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Sincronizada {
    pub fonte: Fonte,
    /// A branch base do clone andou.
    pub avancou: bool,
    pub importacao: Importacao,
    /// O índice da base não pôde ser sincronizado (a vigia tenta de novo).
    pub aviso_do_indice: Option<String>,
}

/// Situação da documentação externa do projeto, para o app.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Situacao {
    /// Rótulo do destino (`docs/negocio em GitHub exemplo/faturamento`); `None`
    /// sem destino.
    pub destino: Option<String>,
    pub estado: Option<EstadoExternos>,
    pub importados: usize,
    /// Por que a cópia pode estar desatualizada.
    pub aviso: Option<String>,
}

/// Situação atual, sem rede.
pub fn situacao(ambiente: &Ambiente) -> Situacao {
    let repos = &ambiente.projeto.repositorios;
    let destino = repos.docs.as_ref().map(|d| d.rotulo(&repos.codigo));
    let estado = EstadoExternos::ler(&ambiente.kb);
    let aviso = destino.as_ref().and_then(|_| match &estado {
        Some(e) => e.desatualizado(externos::agora_ms(), LIMITE_DE_IDADE),
        None => Some("o repositório de docs ainda não foi sincronizado".into()),
    });
    Situacao {
        destino,
        importados: estado.as_ref().map_or(0, EstadoExternos::importados),
        estado,
        aviso,
    }
}

/// Sincroniza o repositório de docs do projeto e importa os `.md` para a base.
pub fn sincronizar(
    ambiente: &Ambiente,
    f: &Ferramentas,
) -> Result<Sincronizada, ErroSincronizacao> {
    let destino = ambiente
        .projeto
        .repositorios
        .destino_docs()
        .ok_or(ErroSincronizacao::SemDestino)?;
    let repo = destino.repositorio;
    let pasta = destino
        .pasta_local()
        .ok_or_else(|| ErroSincronizacao::SemClone(repo.rotulo()))?
        .to_path_buf();
    let kb = &ambiente.kb;
    let _trava = externos::travar(kb)
        .map_err(|e| ErroSincronizacao::Falhou(e.to_string()))?
        .ok_or(ErroSincronizacao::EmAndamento)?;

    let falhou = |erro: String| {
        if let Err(e) = externos::registrar_falha(kb, &erro, externos::agora_ms()) {
            tracing::warn!(erro = %e, "estado da sincronização não gravado");
        }
        ErroSincronizacao::Falhou(erro)
    };

    let token = token_do_repositorio(repo, f.chaveiro.as_ref())
        .map_err(|e| falhou(format!("credencial de {}: {e}", repo.rotulo())))?;
    let atualizacao = atualizar_clone(&f.git, repo, &pasta, destino.branch_base(), token.as_ref())
        .map_err(|e| falhou(e.to_string()))?;
    let leitor = LeitorDeCommit::abrir(&f.git, repo, &pasta).map_err(|e| falhou(e.to_string()))?;
    let arquivos = leitor
        .listar(&atualizacao.commit, destino.subpasta)
        .map_err(|e| falhou(e.to_string()))?;
    let prefixo = if destino.subpasta.is_empty() {
        String::new()
    } else {
        format!("{}/", destino.subpasta)
    };
    let arquivos: Vec<(ArquivoExterno, String)> = arquivos
        .into_iter()
        .filter(|a| a.caminho.to_ascii_lowercase().ends_with(".md"))
        .filter_map(|a| {
            let caminho = a.caminho.strip_prefix(&prefixo)?.to_string();
            Some((
                ArquivoExterno {
                    caminho,
                    caminho_no_repositorio: a.caminho,
                    versao: a.blob.clone(),
                    tamanho: a.tamanho,
                },
                a.blob,
            ))
        })
        .collect();
    let blobs: std::collections::HashMap<String, String> = arquivos
        .iter()
        .map(|(a, b)| (a.caminho_no_repositorio.clone(), b.clone()))
        .collect();
    let lista: Vec<ArquivoExterno> = arquivos.into_iter().map(|(a, _)| a).collect();

    let fonte = Fonte {
        repositorio: repo.rotulo(),
        branch: atualizacao.base.clone(),
        commit: atualizacao.commit.clone(),
    };
    let exclusoes = cba_index::Exclusoes::padrao();
    let pular = |caminho: &str| {
        exclusoes.avaliar_caminho(caminho).and_then(|e| {
            matches!(
                e.motivo,
                Motivo::Segredo | Motivo::ControleVersao | Motivo::Dependencia
            )
            .then(|| match e.regra {
                Some(r) => format!("{} (padrão `{r}`)", e.motivo.descricao()),
                None => e.motivo.descricao().to_string(),
            })
        })
    };
    let mut ler = |a: &ArquivoExterno| {
        let blob = blobs
            .get(&a.caminho_no_repositorio)
            .ok_or_else(|| "arquivo fora do commit".to_string())?;
        leitor.ler(blob).map_err(|e| e.to_string())
    };
    let importacao = externos::importar(kb, &fonte, &lista, &pular, &mut ler, externos::agora_ms())
        .map_err(|e| falhou(e.to_string()))?;

    let aviso_do_indice = sincronizar_indice(ambiente).err();
    tracing::info!(
        novos = importacao.novos,
        alterados = importacao.alterados,
        removidos = importacao.removidos,
        pulados = importacao.pulados.len(),
        mascarados = importacao.mascarados.len(),
        "repositório de docs sincronizado"
    );
    Ok(Sincronizada {
        fonte,
        avancou: atualizacao.avancou,
        importacao,
        aviso_do_indice,
    })
}

fn sincronizar_indice(ambiente: &Ambiente) -> Result<Sincronizacao, String> {
    let mut indice = Indice::abrir(ambiente.dados.arquivo_do_indice(), &ambiente.base)
        .map_err(|e| e.to_string())?;
    indice.sincronizar().map_err(|e| e.to_string())
}

/// Ao iniciar uma análise: pede a sincronização em segundo plano (se o projeto
/// tem destino e a última tentativa não é recente) e devolve o aviso para o
/// agente quando a cópia está desatualizada. Nunca bloqueia nem falha a análise.
pub fn ao_iniciar_analise(ambiente: &Ambiente) -> Option<String> {
    ao_iniciar_analise_com(ambiente, Ferramentas::default())
}

/// Como [`ao_iniciar_analise`], com outras ferramentas (testes).
pub fn ao_iniciar_analise_com(ambiente: &Ambiente, f: Ferramentas) -> Option<String> {
    let destino = ambiente.projeto.repositorios.destino_docs()?;
    destino.pasta_local()?;
    let estado = EstadoExternos::ler(&ambiente.kb);
    let agora = externos::agora_ms();
    let recente = estado
        .as_ref()
        .and_then(|e| e.tentativa_ms)
        .is_some_and(|t| {
            agora.saturating_sub(t) < u64::try_from(INTERVALO_MINIMO.as_millis()).unwrap_or(0)
        });
    let aviso = match &estado {
        Some(e) => e.desatualizado(agora, LIMITE_DE_IDADE),
        None => Some("o repositório de docs ainda não foi sincronizado".into()),
    };
    if !recente {
        let ambiente = ambiente.clone();
        let iniciada = thread::Builder::new()
            .name("cba-sincronizar-docs".into())
            .spawn(move || match sincronizar(&ambiente, &f) {
                Ok(_) | Err(ErroSincronizacao::EmAndamento) => {}
                Err(e) => tracing::warn!(erro = %e, "sincronização do repositório de docs"),
            });
        if let Err(e) = iniciada {
            tracing::warn!(erro = %e, "sincronização do repositório de docs não iniciada");
        }
    }
    aviso.map(|a| {
        format!(
            "{a}. A documentação publicada (documentos externos, em `externos/`) pode estar \
             desatualizada: a sincronização foi pedida em segundo plano. Ela nunca substitui \
             a evidência do código"
        )
    })
}
