//! Guarda temporária do transcript (#136): as falas analisadas de uma reunião
//! ficam no diretório de dados do app até a ata ser aprovada, a reunião ser
//! descartada ou o prazo de retenção vencer.
//!
//! # Onde fica
//!
//! ```text
//! <dados>/reunioes/<projeto>/<R-NNNN>/
//!   falas.json     # as falas (Vec<Fala>), com o texto
//!   guarda.json    # metadados (Metadados): criada_em_ms, formato, avisos, quantidade de falas
//! ```
//!
//! `<dados>` é o diretório de dados do app ([`DiretorioDados::reunioes`]). O
//! transcript **nunca** vai para a base de conhecimento nem para o repositório
//! do projeto: na base fica só a ata, com trechos curtos. O identificador da
//! reunião (`R-NNNN`) é único por projeto, por isso a pasta fica sob o slug do
//! projeto.
//!
//! # Escrita segura
//!
//! - **Guardar** monta a pasta num temporário (`.cba-R-NNNN-…`, ao lado) e a
//!   renomeia para o lugar no fim: quem lê nunca vê uma guarda pela metade. Se
//!   a reunião já tem transcript, a guarda é recusada ([`ErroGuarda::JaGuardado`]);
//!   para trocar, descarte antes.
//! - **Apagar** renomeia a pasta para `.apagando-R-NNNN-…` e só então apaga o
//!   conteúdo: um leitor ao mesmo tempo vê a guarda inteira ou nenhuma. Apagar
//!   o que não existe não é erro (idempotente). Sobras de uma queda (pastas
//!   ocultas com mais de uma hora) são limpas pela retenção.
//!
//! # Descarte e retenção
//!
//! - [`Guarda::descartar`]: ao aprovar a ata ou descartar a reunião, apaga a
//!   guarda e marca as citações conferidas da ata como "conferida em <data>"
//!   ([`EstadoAta::marcar_conferidas_em`]), gravando a ata de novo. Pode ser
//!   repetido sem efeito.
//! - [`aplicar_retencao`]: apaga as guardas com [`prazo`](cba_core::config::Config::prazo_retencao_transcript)
//!   vencido (padrão 30 dias desde a guarda) e sinaliza as que vencem em até
//!   [`DIAS_DE_AVISO`] dias. A ata dessas reuniões não é tocada aqui (a rotina
//!   só conhece o diretório de dados): quem tem a base chama
//!   [`marcar_citacoes_descartadas`] para cada reunião apagada.
//!
//! # Privacidade
//!
//! O diário de eventos ([`Mudanca::Transcript`]) e o log recebem só o slug do
//! projeto, o identificador da reunião, contagens e o motivo. Nenhuma mensagem
//! de erro desta crate traz texto de fala.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use cba_core::config::Config;
use cba_core::dados::DiretorioDados;
use cba_core::eventos::{DiarioEventos, Evento, Mudanca, Notificador};
use cba_core::tempo::{agora_ms, data_utc, dias_entre, somar_dias};
use cba_docs::{ErroAta, EstadoAta, Templates};
use cba_kb::frontmatter::Data;
use cba_kb::{Base, IdReuniao, Projeto, e_slug, normalizar_nome};
use serde::{Deserialize, Serialize};

use crate::citacao::{
    CitacaoConferida, PedidoCitacao, RecusaCitacao, conferir_citacao, contem_palavras, palavras,
};
use crate::transcript::{Aviso, Fala, Formato, Horario, Transcript};

/// Arquivo com as falas, dentro da pasta da reunião.
pub const ARQUIVO_FALAS: &str = "falas.json";
/// Arquivo com os metadados, dentro da pasta da reunião.
pub const ARQUIVO_METADADOS: &str = "guarda.json";
/// Dias antes do vencimento em que a guarda passa a ser sinalizada.
pub const DIAS_DE_AVISO: i64 = 7;
/// Falas por página em [`Guarda::ler_falas`], sem limite informado.
pub const LIMITE_PADRAO: usize = 50;
/// Máximo de falas por página em [`Guarda::ler_falas`].
pub const LIMITE_MAXIMO: usize = 200;

/// Prefixo da pasta temporária de uma guarda em montagem.
const PREFIXO_MONTAGEM: &str = ".cba-";
/// Prefixo da pasta de uma guarda sendo apagada.
const PREFIXO_APAGANDO: &str = ".apagando-";
/// Idade a partir da qual uma pasta temporária é sobra de uma queda.
const IDADE_DE_SOBRA: Duration = Duration::from_secs(3600);
/// Versão do formato de `guarda.json`.
const VERSAO: u32 = 1;

/// Metadados da guarda (`guarda.json`). Sem texto de fala.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Metadados {
    pub versao: u32,
    /// Slug do projeto.
    pub projeto: String,
    pub reuniao: IdReuniao,
    /// Quando o transcript foi guardado (milissegundos desde 1970, UTC).
    pub criada_em_ms: u64,
    pub formato: Formato,
    /// Avisos do parser (só códigos e contagens).
    pub avisos: Vec<Aviso>,
    /// Quantidade de falas.
    pub falas: usize,
}

impl Metadados {
    /// Data (UTC) da guarda.
    pub fn criada_em(&self) -> Data {
        data_utc(self.criada_em_ms)
    }
}

/// Uma reunião com transcript guardado.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
pub struct ReuniaoGuardada {
    pub projeto: String,
    pub reuniao: IdReuniao,
}

/// Situação de uma guarda diante do prazo de retenção.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Situacao {
    pub projeto: String,
    pub reuniao: IdReuniao,
    /// Data da guarda.
    pub criada_em: Data,
    /// Data em que a retenção apaga o transcript (`criada_em` + prazo).
    pub apaga_em: Data,
    /// Dias de hoje até `apaga_em` (0 ou menos: vencida).
    pub dias_restantes: i64,
    /// Vence em até [`DIAS_DE_AVISO`] dias (e ainda não venceu).
    pub vence_em_breve: bool,
    /// O prazo venceu: a próxima retenção apaga.
    pub vencida: bool,
    /// Quantidade de falas.
    pub falas: usize,
}

/// Por que o transcript foi apagado. Vai para o log.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MotivoDescarte {
    /// A ata foi aprovada.
    AtaAprovada,
    /// O analista descartou a reunião.
    ReuniaoDescartada,
    /// O prazo de retenção venceu.
    Retencao,
}

impl MotivoDescarte {
    fn como_str(self) -> &'static str {
        match self {
            Self::AtaAprovada => "ata_aprovada",
            Self::ReuniaoDescartada => "reuniao_descartada",
            Self::Retencao => "retencao",
        }
    }
}

/// Resultado de [`Guarda::descartar`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Descarte {
    /// A pasta existia e foi apagada (falso na repetição).
    pub transcript_apagado: bool,
    /// A reunião tem ata (estado) na base.
    pub ata_encontrada: bool,
    /// Citações que passaram a "conferida em <data>".
    pub citacoes_marcadas: usize,
}

/// Resultado de [`aplicar_retencao`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct RelatorioRetencao {
    /// Guardas apagadas por prazo vencido.
    pub apagados: Vec<ReuniaoGuardada>,
    /// Guardas que vencem em até [`DIAS_DE_AVISO`] dias.
    pub a_vencer: Vec<Situacao>,
    /// Guardas que não deu para avaliar ou apagar (o motivo vai para o log).
    pub falhas: Vec<ReuniaoGuardada>,
}

/// Filtro e paginação de [`Guarda::ler_falas`]. O que a ferramenta
/// `ler_transcript` do MCP (#139) recebe.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FiltroFalas {
    /// Só as falas desse falante. Compara pela chave de
    /// [`cba_kb::normalizar_nome`] como sequência de palavras: "maria" pega
    /// "Silva, Maria", e "Maria Silva" pega "maria.silva@exemplo.com".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub falante: Option<String>,
    /// Só as falas que começam neste horário ou depois. Falas sem horário
    /// ficam de fora quando há `desde` ou `ate`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desde: Option<Horario>,
    /// Só as falas que começam neste horário ou antes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ate: Option<Horario>,
    /// Quantas falas (das que passam no filtro) pular.
    #[serde(default)]
    pub pular: usize,
    /// Falas por página: padrão [`LIMITE_PADRAO`], no máximo [`LIMITE_MAXIMO`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limite: Option<usize>,
}

/// Uma página de falas.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PaginaFalas {
    pub falas: Vec<Fala>,
    /// Falas que passam no filtro, em todas as páginas.
    pub total: usize,
    /// Valor de `pular` para a próxima página, se houver.
    pub proxima: Option<usize>,
}

/// Erro da guarda. As mensagens trazem só identificadores.
#[derive(Debug, thiserror::Error)]
pub enum ErroGuarda {
    #[error("nome de projeto inválido `{0}`: use só a-z, 0-9 e -")]
    Projeto(String),
    #[error(
        "a reunião {reuniao} do projeto {projeto} não tem transcript guardado \
         (não foi guardado, ou já foi descartado)"
    )]
    NaoGuardado { projeto: String, reuniao: IdReuniao },
    #[error(
        "a reunião {reuniao} do projeto {projeto} já tem transcript guardado; \
         descarte-o antes de guardar outro"
    )]
    JaGuardado { projeto: String, reuniao: IdReuniao },
    #[error("transcript da reunião {reuniao}: {erro}")]
    Io {
        reuniao: IdReuniao,
        #[source]
        erro: io::Error,
    },
    /// Arquivo da guarda ilegível. O detalhe tem só a categoria e a posição do
    /// erro, nunca o conteúdo.
    #[error("transcript da reunião {reuniao} ilegível: {detalhe}")]
    Ilegivel { reuniao: IdReuniao, detalhe: String },
    #[error(transparent)]
    Recusada(#[from] RecusaCitacao),
    #[error(transparent)]
    Ata(#[from] ErroAta),
}

/// Guarda temporária dos transcripts, no diretório de dados. Veja o
/// [módulo](self).
#[derive(Debug, Clone)]
pub struct Guarda {
    raiz: PathBuf,
    notificador: Arc<dyn Notificador>,
}

impl Guarda {
    /// Guarda em `<dados>/reunioes`, notificando no diário de eventos do app
    /// (`<dados>/eventos`).
    pub fn nova(dados: &DiretorioDados) -> Self {
        Self::com_notificador(dados, Arc::new(DiarioEventos::em(dados.eventos())))
    }

    /// Guarda em `<dados>/reunioes`, com outro destino das notificações.
    pub fn com_notificador(dados: &DiretorioDados, notificador: Arc<dyn Notificador>) -> Self {
        Self {
            raiz: dados.reunioes(),
            notificador,
        }
    }

    /// Pasta da guarda de uma reunião (`<dados>/reunioes/<projeto>/<R-NNNN>`).
    pub fn pasta(&self, projeto: &str, reuniao: IdReuniao) -> Result<PathBuf, ErroGuarda> {
        Ok(self.pasta_do_projeto(projeto)?.join(reuniao.to_string()))
    }

    fn pasta_do_projeto(&self, projeto: &str) -> Result<PathBuf, ErroGuarda> {
        if !e_slug(projeto) {
            return Err(ErroGuarda::Projeto(projeto.to_owned()));
        }
        Ok(self.raiz.join(projeto))
    }

    /// Guarda as falas analisadas. `agora_ms` é o instante da guarda, de onde
    /// conta o prazo de retenção ([`cba_core::tempo::agora_ms`] no app).
    pub fn guardar(
        &self,
        projeto: &str,
        reuniao: IdReuniao,
        transcript: &Transcript,
        agora_ms: u64,
    ) -> Result<Metadados, ErroGuarda> {
        let pasta = self.pasta(projeto, reuniao)?;
        let io_err = |erro| ErroGuarda::Io { reuniao, erro };
        if pasta.exists() {
            return Err(ErroGuarda::JaGuardado {
                projeto: projeto.to_owned(),
                reuniao,
            });
        }
        let metadados = Metadados {
            versao: VERSAO,
            projeto: projeto.to_owned(),
            reuniao,
            criada_em_ms: agora_ms,
            formato: transcript.formato,
            avisos: transcript.avisos.clone(),
            falas: transcript.falas.len(),
        };
        let pai = pasta
            .parent()
            .expect("a pasta da reunião fica na do projeto");
        fs::create_dir_all(pai).map_err(io_err)?;
        let montagem = pai.join(nome_temporario(PREFIXO_MONTAGEM, reuniao));
        let montar = || -> io::Result<()> {
            fs::create_dir(&montagem)?;
            let falas = serde_json::to_vec(&transcript.falas).map_err(io::Error::other)?;
            gravar_sincronizado(&montagem.join(ARQUIVO_FALAS), &falas)?;
            let meta = serde_json::to_vec_pretty(&metadados).map_err(io::Error::other)?;
            gravar_sincronizado(&montagem.join(ARQUIVO_METADADOS), &meta)?;
            Ok(())
        };
        if let Err(e) = montar() {
            let _ = fs::remove_dir_all(&montagem);
            return Err(io_err(e));
        }
        if pasta.exists() {
            let _ = fs::remove_dir_all(&montagem);
            return Err(ErroGuarda::JaGuardado {
                projeto: projeto.to_owned(),
                reuniao,
            });
        }
        if let Err(e) = renomear(&montagem, &pasta) {
            let _ = fs::remove_dir_all(&montagem);
            if pasta.exists() {
                return Err(ErroGuarda::JaGuardado {
                    projeto: projeto.to_owned(),
                    reuniao,
                });
            }
            return Err(io_err(e));
        }
        tracing::info!(
            projeto,
            reuniao = %reuniao,
            falas = metadados.falas,
            formato = ?metadados.formato,
            avisos = metadados.avisos.len(),
            "transcript guardado"
        );
        self.notificar(projeto, reuniao);
        Ok(metadados)
    }

    /// A reunião tem transcript guardado.
    pub fn existe(&self, projeto: &str, reuniao: IdReuniao) -> bool {
        self.pasta(projeto, reuniao)
            .is_ok_and(|p| p.join(ARQUIVO_METADADOS).is_file())
    }

    /// Metadados da guarda.
    pub fn metadados(&self, projeto: &str, reuniao: IdReuniao) -> Result<Metadados, ErroGuarda> {
        let bytes = self.ler_arquivo(projeto, reuniao, ARQUIVO_METADADOS)?;
        serde_json::from_slice(&bytes).map_err(|e| ilegivel(reuniao, &e))
    }

    /// Todas as falas guardadas, na ordem.
    pub fn falas(&self, projeto: &str, reuniao: IdReuniao) -> Result<Vec<Fala>, ErroGuarda> {
        let bytes = self.ler_arquivo(projeto, reuniao, ARQUIVO_FALAS)?;
        serde_json::from_slice(&bytes).map_err(|e| ilegivel(reuniao, &e))
    }

    /// Uma página das falas que passam no filtro, na ordem do transcript.
    pub fn ler_falas(
        &self,
        projeto: &str,
        reuniao: IdReuniao,
        filtro: &FiltroFalas,
    ) -> Result<PaginaFalas, ErroGuarda> {
        let falas = self.falas(projeto, reuniao)?;
        Ok(filtrar_falas(falas, filtro))
    }

    /// Confere uma citação contra a fala guardada. Veja [`crate::citacao`].
    pub fn conferir_citacao(
        &self,
        projeto: &str,
        reuniao: IdReuniao,
        pedido: &PedidoCitacao,
    ) -> Result<CitacaoConferida, ErroGuarda> {
        let falas = self.falas(projeto, reuniao)?;
        let resultado = conferir_citacao(&falas, pedido);
        match &resultado {
            Ok(c) => {
                tracing::debug!(projeto, reuniao = %reuniao, fala = c.indice, "citação conferida")
            }
            Err(e) => tracing::debug!(
                projeto,
                reuniao = %reuniao,
                fala = ?pedido.indice,
                motivo = recusa_como_str(e),
                "citação recusada"
            ),
        }
        Ok(resultado?)
    }

    /// Apaga a guarda. Devolve se havia o que apagar (apagar de novo não é
    /// erro). Não mexe na ata: para o descarte completo, use
    /// [`Guarda::descartar`].
    pub fn apagar(
        &self,
        projeto: &str,
        reuniao: IdReuniao,
        motivo: MotivoDescarte,
    ) -> Result<bool, ErroGuarda> {
        let pasta = self.pasta(projeto, reuniao)?;
        let io_err = |erro| ErroGuarda::Io { reuniao, erro };
        if !pasta.exists() {
            return Ok(false);
        }
        let pai = pasta
            .parent()
            .expect("a pasta da reunião fica na do projeto");
        let lixo = pai.join(nome_temporario(PREFIXO_APAGANDO, reuniao));
        match renomear(&pasta, &lixo) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(e) => return Err(io_err(e)),
        }
        if let Err(e) = fs::remove_dir_all(&lixo) {
            // Já está fora do lugar: ninguém lê mais. A retenção limpa depois.
            tracing::warn!(projeto, reuniao = %reuniao, erro = %e.kind(), "sobra do transcript apagado");
        }
        // A pasta do projeto some quando fica vazia (falha se não estiver).
        let _ = fs::remove_dir(pai);
        tracing::info!(
            projeto,
            reuniao = %reuniao,
            motivo = motivo.como_str(),
            "transcript apagado"
        );
        self.notificar(projeto, reuniao);
        Ok(true)
    }

    /// Descarte ao aprovar a ata ou descartar a reunião: apaga a guarda e
    /// marca as citações conferidas da ata como "conferida em `hoje`",
    /// gravando a ata de novo. Idempotente: repetir não muda nada. Uma reunião
    /// sem ata na base só tem a guarda apagada.
    ///
    /// A guarda é apagada antes de mexer na ata: se a gravação da ata falhar, o
    /// transcript já não existe, e repetir o descarte marca as citações.
    pub fn descartar(
        &self,
        projeto: &Projeto,
        templates: &Templates,
        reuniao: IdReuniao,
        hoje: Data,
        motivo: MotivoDescarte,
    ) -> Result<Descarte, ErroGuarda> {
        let transcript_apagado = self.apagar(projeto.nome(), reuniao, motivo)?;
        let marcadas = marcar_citacoes_descartadas(projeto, templates, reuniao, hoje)?;
        Ok(Descarte {
            transcript_apagado,
            ata_encontrada: marcadas.is_some(),
            citacoes_marcadas: marcadas.unwrap_or(0),
        })
    }

    /// Situação da guarda diante do prazo de retenção (`prazo_dias` desde a
    /// guarda), ou `None` sem transcript guardado.
    pub fn situacao(
        &self,
        projeto: &str,
        reuniao: IdReuniao,
        hoje: Data,
        prazo_dias: u32,
    ) -> Result<Option<Situacao>, ErroGuarda> {
        match self.metadados(projeto, reuniao) {
            Ok(m) => Ok(Some(situacao(
                projeto,
                reuniao,
                m.criada_em_ms,
                m.falas,
                hoje,
                prazo_dias,
            ))),
            Err(ErroGuarda::NaoGuardado { .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Reuniões com transcript guardado, de todos os projetos, em ordem.
    pub fn listar(&self) -> io::Result<Vec<ReuniaoGuardada>> {
        let mut saida = Vec::new();
        for projeto in subpastas(&self.raiz)? {
            if !e_slug(&projeto) {
                continue;
            }
            for nome in subpastas(&self.raiz.join(&projeto))? {
                if let Ok(reuniao) = IdReuniao::analisar(&nome) {
                    saida.push(ReuniaoGuardada {
                        projeto: projeto.clone(),
                        reuniao,
                    });
                }
            }
        }
        saida.sort();
        Ok(saida)
    }

    /// Retenção: apaga as guardas com o prazo vencido e sinaliza as que vencem
    /// em até [`DIAS_DE_AVISO`] dias. Veja [`aplicar_retencao`].
    pub fn aplicar_retencao(&self, hoje: Data, prazo_dias: u32) -> RelatorioRetencao {
        let mut relatorio = RelatorioRetencao::default();
        let guardadas = match self.listar() {
            Ok(g) => g,
            Err(e) => {
                tracing::warn!(erro = %e.kind(), "retenção dos transcripts: pasta ilegível");
                return relatorio;
            }
        };
        for g in guardadas {
            match self.avaliar_retencao(&g, hoje, prazo_dias) {
                Ok(Some(s)) if s.vencida => {
                    match self.apagar(&g.projeto, g.reuniao, MotivoDescarte::Retencao) {
                        Ok(_) => relatorio.apagados.push(g),
                        Err(e) => {
                            tracing::warn!(projeto = g.projeto, reuniao = %g.reuniao, erro = %e, "retenção: falha ao apagar");
                            relatorio.falhas.push(g);
                        }
                    }
                }
                Ok(Some(s)) if s.vence_em_breve => relatorio.a_vencer.push(s),
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!(projeto = g.projeto, reuniao = %g.reuniao, erro = %e, "retenção: guarda ilegível");
                    relatorio.falhas.push(g);
                }
            }
        }
        self.limpar_sobras();
        tracing::info!(
            apagados = relatorio.apagados.len(),
            a_vencer = relatorio.a_vencer.len(),
            falhas = relatorio.falhas.len(),
            prazo_dias,
            "retenção dos transcripts aplicada"
        );
        relatorio
    }

    /// Situação para a retenção. Com os metadados ilegíveis, a data da guarda
    /// é a da última modificação da pasta: o transcript não fica para sempre
    /// por causa de um arquivo estragado.
    fn avaliar_retencao(
        &self,
        g: &ReuniaoGuardada,
        hoje: Data,
        prazo_dias: u32,
    ) -> Result<Option<Situacao>, ErroGuarda> {
        match self.situacao(&g.projeto, g.reuniao, hoje, prazo_dias) {
            Ok(s) => Ok(s),
            Err(ErroGuarda::Ilegivel { .. }) => {
                let pasta = self.pasta(&g.projeto, g.reuniao)?;
                let ms = fs::metadata(&pasta)
                    .and_then(|m| m.modified())
                    .map(ms_de)
                    .map_err(|erro| ErroGuarda::Io {
                        reuniao: g.reuniao,
                        erro,
                    })?;
                Ok(Some(situacao(
                    &g.projeto, g.reuniao, ms, 0, hoje, prazo_dias,
                )))
            }
            Err(e) => Err(e),
        }
    }

    /// Apaga pastas temporárias com mais de uma hora (sobras de uma queda).
    fn limpar_sobras(&self) {
        let Ok(projetos) = subpastas(&self.raiz) else {
            return;
        };
        for projeto in projetos {
            let pasta = self.raiz.join(&projeto);
            let Ok(nomes) = subpastas(&pasta) else {
                continue;
            };
            for nome in nomes {
                if !(nome.starts_with(PREFIXO_MONTAGEM) || nome.starts_with(PREFIXO_APAGANDO)) {
                    continue;
                }
                let caminho = pasta.join(&nome);
                let velha = fs::metadata(&caminho)
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| t.elapsed().ok())
                    .is_some_and(|idade| idade >= IDADE_DE_SOBRA);
                if velha && fs::remove_dir_all(&caminho).is_ok() {
                    tracing::info!(projeto, "sobra de transcript apagada");
                }
            }
            let _ = fs::remove_dir(&pasta);
        }
    }

    fn ler_arquivo(
        &self,
        projeto: &str,
        reuniao: IdReuniao,
        arquivo: &str,
    ) -> Result<Vec<u8>, ErroGuarda> {
        let caminho = self.pasta(projeto, reuniao)?.join(arquivo);
        fs::read(&caminho).map_err(|erro| {
            if erro.kind() == io::ErrorKind::NotFound {
                ErroGuarda::NaoGuardado {
                    projeto: projeto.to_owned(),
                    reuniao,
                }
            } else {
                ErroGuarda::Io { reuniao, erro }
            }
        })
    }

    fn notificar(&self, projeto: &str, reuniao: IdReuniao) {
        let evento = Evento {
            quando_ms: agora_ms(),
            projeto: projeto.to_owned(),
            analise: reuniao.to_string(),
            mudanca: Mudanca::Transcript,
            versao: 0,
            processo: std::process::id(),
        };
        if let Err(e) = self.notificador.notificar(&evento) {
            tracing::warn!(projeto, reuniao = %reuniao, erro = %e.kind(), "diário de eventos: falha ao notificar");
        }
    }
}

/// Rotina de retenção dos transcripts: apaga os guardados há `prazo_dias` dias
/// ou mais (a ata não foi aprovada a tempo) e sinaliza os que vencem em até
/// [`DIAS_DE_AVISO`] dias. Notifica no diário de eventos do app. Falhas numa
/// guarda não param as demais: vão para [`RelatorioRetencao::falhas`] e para o
/// log.
pub fn aplicar_retencao(dados: &DiretorioDados, hoje: Data, prazo_dias: u32) -> RelatorioRetencao {
    Guarda::nova(dados).aplicar_retencao(hoje, prazo_dias)
}

/// Retenção no início do app: lê o prazo da configuração
/// ([`Config::prazo_retencao_transcript`]), aplica [`aplicar_retencao`] com a
/// data de hoje e, com a base configurada, marca as citações das atas das
/// reuniões apagadas como "conferida em <hoje>"
/// ([`marcar_citacoes_descartadas`]). Falhas vão para o log e não impedem o
/// app de abrir.
pub fn aplicar_retencao_no_inicio(dados: &DiretorioDados) -> RelatorioRetencao {
    let config = Config::ler(dados).unwrap_or_else(|e| {
        tracing::warn!(erro = %e.kind(), "retenção: configuração ilegível, prazo padrão");
        Config::default()
    });
    let hoje = cba_core::tempo::hoje();
    let relatorio = aplicar_retencao(dados, hoje, config.prazo_retencao_transcript());
    if relatorio.apagados.is_empty() {
        return relatorio;
    }
    let Some(base) = config.base().and_then(|b| Base::abrir(b).ok()) else {
        return relatorio;
    };
    let (templates, _) = Templates::da_base(&base);
    for g in &relatorio.apagados {
        let marcado = base
            .projeto(&g.projeto)
            .map_err(ErroAta::from)
            .and_then(|p| marcar_citacoes_descartadas(&p, &templates, g.reuniao, hoje));
        if let Err(e) = marcado {
            tracing::warn!(projeto = g.projeto, reuniao = %g.reuniao, erro = %e, "retenção: ata não atualizada");
        }
    }
    relatorio
}

/// Marca as citações conferidas da ata da reunião como "conferida em `data`" e
/// grava a ata, depois que o transcript foi apagado. Devolve quantas citações
/// mudaram, ou `None` se a reunião não tem ata na base. Sem mudança, não grava.
pub fn marcar_citacoes_descartadas(
    projeto: &Projeto,
    templates: &Templates,
    reuniao: IdReuniao,
    data: Data,
) -> Result<Option<usize>, ErroAta> {
    let mut estado = match EstadoAta::ler(projeto, reuniao) {
        Ok((estado, _)) => estado,
        Err(ErroAta::Base(cba_kb::Erro::ReuniaoInexistente { .. })) => return Ok(None),
        Err(e) => return Err(e),
    };
    let marcadas = estado.marcar_conferidas_em(data);
    if marcadas > 0 {
        templates.gravar_ata(projeto, &estado)?;
        tracing::info!(
            projeto = projeto.nome(),
            reuniao = %reuniao,
            citacoes = marcadas,
            "citações da ata marcadas como conferidas no descarte"
        );
    }
    Ok(Some(marcadas))
}

/// Aplica o filtro e a paginação de [`FiltroFalas`].
pub fn filtrar_falas(falas: Vec<Fala>, filtro: &FiltroFalas) -> PaginaFalas {
    let falante = filtro
        .falante
        .as_deref()
        .map(|f| palavras(&normalizar_nome(f)))
        .filter(|p| !p.is_empty());
    let com_horario = filtro.desde.is_some() || filtro.ate.is_some();
    let passa = |f: &Fala| {
        if let Some(chave) = &falante {
            let ok = f
                .falante
                .as_deref()
                .is_some_and(|n| contem_palavras(&palavras(&normalizar_nome(n)), chave));
            if !ok {
                return false;
            }
        }
        if com_horario {
            let Some(inicio) = f.inicio else {
                return false;
            };
            if filtro.desde.is_some_and(|d| inicio < d) || filtro.ate.is_some_and(|a| inicio > a) {
                return false;
            }
        }
        true
    };
    let filtradas: Vec<Fala> = falas.into_iter().filter(passa).collect();
    let total = filtradas.len();
    let limite = filtro
        .limite
        .unwrap_or(LIMITE_PADRAO)
        .clamp(1, LIMITE_MAXIMO);
    let falas: Vec<Fala> = filtradas
        .into_iter()
        .skip(filtro.pular)
        .take(limite)
        .collect();
    let fim = filtro.pular.saturating_add(falas.len());
    PaginaFalas {
        proxima: (fim < total).then_some(fim),
        falas,
        total,
    }
}

fn situacao(
    projeto: &str,
    reuniao: IdReuniao,
    criada_em_ms: u64,
    falas: usize,
    hoje: Data,
    prazo_dias: u32,
) -> Situacao {
    let criada_em = data_utc(criada_em_ms);
    let apaga_em = somar_dias(criada_em, i64::from(prazo_dias.max(1)));
    let dias_restantes = dias_entre(hoje, apaga_em);
    let vencida = dias_restantes <= 0;
    Situacao {
        projeto: projeto.to_owned(),
        reuniao,
        criada_em,
        apaga_em,
        dias_restantes,
        vence_em_breve: !vencida && dias_restantes <= DIAS_DE_AVISO,
        vencida,
        falas,
    }
}

fn recusa_como_str(e: &RecusaCitacao) -> &'static str {
    match e {
        RecusaCitacao::TrechoVazio => "trecho_vazio",
        RecusaCitacao::TrechoLongo { .. } => "trecho_longo",
        RecusaCitacao::PedidoIncompleto => "pedido_incompleto",
        RecusaCitacao::FalaInexistente { .. } => "fala_inexistente",
        RecusaCitacao::FalanteDiferente { .. } => "falante_diferente",
        RecusaCitacao::HorarioDiferente { .. } => "horario_diferente",
        RecusaCitacao::FalaNaoEncontrada { .. } => "fala_nao_encontrada",
        RecusaCitacao::NaoConfere { .. } => "nao_confere",
    }
}

/// Erro de JSON sem o conteúdo: só a categoria e a posição.
fn ilegivel(reuniao: IdReuniao, e: &serde_json::Error) -> ErroGuarda {
    let categoria = match e.classify() {
        serde_json::error::Category::Io => "falha de leitura",
        serde_json::error::Category::Syntax => "JSON inválido",
        serde_json::error::Category::Data => "formato inesperado",
        serde_json::error::Category::Eof => "arquivo incompleto",
    };
    ErroGuarda::Ilegivel {
        reuniao,
        detalhe: format!("{categoria} (linha {}, coluna {})", e.line(), e.column()),
    }
}

fn nome_temporario(prefixo: &str, reuniao: IdReuniao) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{prefixo}{reuniao}-{}-{nanos}", std::process::id())
}

fn ms_de(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn gravar_sincronizado(caminho: &Path, conteudo: &[u8]) -> io::Result<()> {
    use std::io::Write;
    let mut f = fs::File::create(caminho)?;
    f.write_all(conteudo)?;
    f.sync_all()
}

/// Renomeia, com algumas tentativas: no Windows, uma pasta com arquivo aberto
/// por outro processo (o servidor MCP lendo, o antivírus) não pode ser
/// renomeada por alguns instantes.
fn renomear(de: &Path, para: &Path) -> io::Result<()> {
    const TENTATIVAS: u32 = 10;
    let mut tentativa = 1;
    loop {
        match fs::rename(de, para) {
            Err(e) if e.kind() == io::ErrorKind::PermissionDenied && tentativa < TENTATIVAS => {
                tentativa += 1;
                std::thread::sleep(Duration::from_millis(50));
            }
            r => return r,
        }
    }
}

/// Nomes das subpastas (inclusive as ocultas). Pasta inexistente: lista vazia.
fn subpastas(pasta: &Path) -> io::Result<Vec<String>> {
    let entradas = match fs::read_dir(pasta) {
        Ok(e) => e,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut nomes = Vec::new();
    for entrada in entradas {
        let entrada = entrada?;
        if entrada.file_type()?.is_dir()
            && let Ok(nome) = entrada.file_name().into_string()
        {
            nomes.push(nome);
        }
    }
    Ok(nomes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fala(indice: usize, falante: &str, s: u64) -> Fala {
        Fala {
            indice,
            falante: Some(falante.into()),
            inicio: Some(Horario::de_milissegundos(s * 1000)),
            fim: None,
            texto: format!("fala {indice}"),
        }
    }

    fn falas() -> Vec<Fala> {
        vec![
            fala(1, "Silva, Maria", 3),
            fala(2, "João Pereira", 30),
            fala(3, "maria.silva@exemplo.com", 60),
            fala(4, "Ana Souza", 90),
            Fala {
                indice: 5,
                falante: None,
                inicio: None,
                fim: None,
                texto: "sem horário".into(),
            },
        ]
    }

    fn indices(p: &PaginaFalas) -> Vec<usize> {
        p.falas.iter().map(|f| f.indice).collect()
    }

    #[test]
    fn filtro_por_falante_horario_e_paginacao() {
        let tudo = filtrar_falas(falas(), &FiltroFalas::default());
        assert_eq!(
            (indices(&tudo), tudo.total, tudo.proxima),
            (vec![1, 2, 3, 4, 5], 5, None)
        );

        let maria = filtrar_falas(
            falas(),
            &FiltroFalas {
                falante: Some("Maria Silva".into()),
                ..Default::default()
            },
        );
        assert_eq!(indices(&maria), [1, 3]);
        let so_nome = filtrar_falas(
            falas(),
            &FiltroFalas {
                falante: Some("MARIA".into()),
                ..Default::default()
            },
        );
        assert_eq!(indices(&so_nome), [1, 3]);

        let intervalo = filtrar_falas(
            falas(),
            &FiltroFalas {
                desde: Some(Horario::de_milissegundos(30_000)),
                ate: Some(Horario::de_milissegundos(60_000)),
                ..Default::default()
            },
        );
        assert_eq!(indices(&intervalo), [2, 3]);

        let p1 = filtrar_falas(
            falas(),
            &FiltroFalas {
                limite: Some(2),
                ..Default::default()
            },
        );
        assert_eq!(
            (indices(&p1), p1.total, p1.proxima),
            (vec![1, 2], 5, Some(2))
        );
        let p3 = filtrar_falas(
            falas(),
            &FiltroFalas {
                limite: Some(2),
                pular: 4,
                ..Default::default()
            },
        );
        assert_eq!((indices(&p3), p3.proxima), (vec![5], None));
        let alem = filtrar_falas(
            falas(),
            &FiltroFalas {
                pular: 99,
                ..Default::default()
            },
        );
        assert!(alem.falas.is_empty() && alem.proxima.is_none());
        let zero = filtrar_falas(
            falas(),
            &FiltroFalas {
                limite: Some(0),
                ..Default::default()
            },
        );
        assert_eq!(zero.falas.len(), 1);
    }

    #[test]
    fn situacao_conta_o_prazo_e_o_aviso() {
        let d = |t: &str| Data::analisar(t).unwrap();
        // 2026-11-01 00:00 UTC.
        let criada = cba_core::tempo::dias_desde_1970(d("2026-11-01")) as u64 * 86_400_000;
        let r = IdReuniao::novo(1);
        let s = situacao("p", r, criada, 3, d("2026-11-14"), 30);
        assert_eq!(s.apaga_em, d("2026-12-01"));
        assert_eq!(s.dias_restantes, 17);
        assert!(!s.vence_em_breve && !s.vencida);
        let s = situacao("p", r, criada, 3, d("2026-11-24"), 30);
        assert_eq!(s.dias_restantes, 7);
        assert!(s.vence_em_breve && !s.vencida);
        let s = situacao("p", r, criada, 3, d("2026-11-23"), 30);
        assert!(!s.vence_em_breve);
        let s = situacao("p", r, criada, 3, d("2026-11-30"), 30);
        assert!(s.vence_em_breve && !s.vencida);
        let s = situacao("p", r, criada, 3, d("2026-12-01"), 30);
        assert!(s.vencida && !s.vence_em_breve);
    }

    #[test]
    fn projeto_que_nao_e_slug_e_recusado() {
        let dir = tempfile::tempdir().unwrap();
        let g = Guarda::com_notificador(
            &DiretorioDados::em(dir.path()),
            Arc::new(cba_core::eventos::SemNotificacao),
        );
        for ruim in ["../fora", "a/b", "", "Projeto"] {
            assert!(matches!(
                g.pasta(ruim, IdReuniao::novo(1)),
                Err(ErroGuarda::Projeto(_))
            ));
        }
    }
}
