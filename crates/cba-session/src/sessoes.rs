//! Estado das sessões dos clientes de IA, gravado pelo receptor de hooks e consultado
//! pelo app.
//!
//! Cada sessão vista nas pastas de um projeto cadastrado tem um arquivo em
//! `<dados>/sessoes/<session_id>.json` ([`RegistroSessao`]): projeto, análise ligada,
//! estado, diretório (para a retomada, #28) e instantes. O registro existe desde o
//! primeiro evento, antes de a sessão ser ligada a uma análise; quando é ligada, o
//! estado também vai para a análise ([`cba_core::analise::Sessao`]), que é o que a
//! tela da análise mostra e o que o app acompanha pelo diário de eventos.
//!
//! O arquivo fica no diretório de dados, e não na base, porque descreve a máquina do
//! analista (diretório, sessões abertas), não o conhecimento. Guarda só
//! identificadores, estados e o diretório da sessão, nunca prompts, mensagens ou
//! argumentos de ferramentas.
//!
//! # Concorrência
//!
//! O Claude Code roda hooks em paralelo (por exemplo, `PreToolUse` de ferramentas
//! chamadas juntas). Toda alteração é ler, mudar e gravar sob a trava exclusiva
//! `<dados>/sessoes/.trava`, e a gravação é atômica, então quem lê sem trava nunca vê
//! um arquivo pela metade.

use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

use cba_core::analise::{Cliente, EstadoSessao};
use cba_core::dados::DiretorioDados;
use cba_kb::Id;
use serde::{Deserialize, Serialize};

/// Versão do formato do registro. Campos novos com valor padrão não exigem mudança.
pub const VERSAO_DO_FORMATO: u32 = 1;

/// Arquivo de trava, na pasta das sessões.
const TRAVA: &str = ".trava";
/// Tamanho máximo aceito para o identificador de uma sessão.
const TAMANHO_MAXIMO_DO_ID: usize = 128;

/// Estado de uma sessão de cliente de IA numa pasta de projeto cadastrado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistroSessao {
    pub formato: u32,
    /// `session_id` dos hooks: o identificador da retomada (`claude --resume <id>`).
    pub id: String,
    /// Slug do projeto cadastrado.
    pub projeto: String,
    /// Análise ligada à sessão (pela primeira ferramenta do `cba` chamada com o
    /// argumento `analise`, normalmente `iniciar_analise`).
    #[serde(default)]
    pub analise: Option<Id>,
    pub cliente: Cliente,
    pub estado: EstadoSessao,
    /// Pasta em que a sessão foi aberta (`CLAUDE_PROJECT_DIR`, ou o `cwd` do primeiro
    /// evento). A retomada roda o cliente nela, porque o histórico do Claude Code é
    /// guardado por pasta.
    #[serde(default)]
    pub diretorio: Option<PathBuf>,
    /// `source` do último `SessionStart` (`startup`, `resume`, `clear`, `compact`,
    /// `fork`).
    #[serde(default)]
    pub origem: Option<String>,
    /// `reason` do `SessionEnd` (`clear`, `resume`, `logout`, `prompt_input_exit`,
    /// `other`).
    #[serde(default)]
    pub motivo_fim: Option<String>,
    pub inicio_ms: u64,
    pub atualizada_ms: u64,
    #[serde(default)]
    pub fim_ms: Option<u64>,
}

impl RegistroSessao {
    /// Registro de uma sessão vista agora pela primeira vez.
    pub fn novo(id: &str, projeto: &str, agora_ms: u64) -> Self {
        Self {
            formato: VERSAO_DO_FORMATO,
            id: id.to_owned(),
            projeto: projeto.to_owned(),
            analise: None,
            cliente: Cliente::Desconhecido,
            estado: EstadoSessao::Ativa,
            diretorio: None,
            origem: None,
            motivo_fim: None,
            inicio_ms: agora_ms,
            atualizada_ms: agora_ms,
            fim_ms: None,
        }
    }

    /// Leva a sessão ao estado `destino`, com as mesmas regras do modelo da análise:
    /// encerrada é final, exceto por um `SessionStart` (retomada), que reativa.
    /// Devolve se o estado mudou.
    pub fn mudar_estado(&mut self, destino: EstadoSessao, reabrir: bool, agora_ms: u64) -> bool {
        if self.estado == destino {
            return false;
        }
        if self.estado == EstadoSessao::Encerrada && !reabrir {
            return false;
        }
        self.estado = destino;
        if destino == EstadoSessao::Encerrada {
            self.fim_ms = Some(agora_ms);
        } else {
            self.fim_ms = None;
            self.motivo_fim = None;
        }
        true
    }
}

/// Se `id` serve como nome de arquivo: letras, dígitos, `-` e `_` (os `session_id`
/// do Claude Code são UUIDs). Outros identificadores são ignorados pelo receptor.
pub fn id_valido(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= TAMANHO_MAXIMO_DO_ID
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Registros das sessões numa pasta.
#[derive(Debug, Clone)]
pub struct Sessoes {
    pasta: PathBuf,
}

impl Sessoes {
    /// Sessões na pasta informada.
    pub fn em(pasta: impl Into<PathBuf>) -> Self {
        Self {
            pasta: pasta.into(),
        }
    }

    /// Sessões no diretório de dados do app (`<dados>/sessoes`).
    pub fn do_diretorio(dados: &DiretorioDados) -> Self {
        Self::em(dados.sessoes())
    }

    pub fn pasta(&self) -> &Path {
        &self.pasta
    }

    /// Arquivo do registro de uma sessão, ou `None` se o identificador não serve
    /// como nome de arquivo.
    pub fn arquivo(&self, id: &str) -> Option<PathBuf> {
        id_valido(id).then(|| self.pasta.join(format!("{id}.json")))
    }

    /// Registro de uma sessão, se existir. Um arquivo ilegível conta como ausente.
    pub fn obter(&self, id: &str) -> io::Result<Option<RegistroSessao>> {
        let Some(arquivo) = self.arquivo(id) else {
            return Ok(None);
        };
        match fs::read(&arquivo) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes).ok()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Todas as sessões registradas, da mais recente para a mais antiga (pela última
    /// atualização). Arquivos ilegíveis são ignorados.
    pub fn listar(&self) -> io::Result<Vec<RegistroSessao>> {
        let entradas = match fs::read_dir(&self.pasta) {
            Ok(e) => e,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };
        let mut registros = Vec::new();
        for entrada in entradas.flatten() {
            let nome = entrada.file_name();
            let Some(id) = nome.to_str().and_then(|n| n.strip_suffix(".json")) else {
                continue;
            };
            if let Some(r) = self.obter(id)? {
                registros.push(r);
            }
        }
        registros.sort_by(|a, b| {
            b.atualizada_ms
                .cmp(&a.atualizada_ms)
                .then_with(|| a.id.cmp(&b.id))
        });
        Ok(registros)
    }

    /// Sessões de um projeto, da mais recente para a mais antiga.
    pub fn do_projeto(&self, projeto: &str) -> io::Result<Vec<RegistroSessao>> {
        let mut r = self.listar()?;
        r.retain(|s| s.projeto == projeto);
        Ok(r)
    }

    /// Sessões ligadas a uma análise, da mais recente para a mais antiga. A primeira
    /// é a candidata à retomada (#28).
    pub fn da_analise(&self, projeto: &str, analise: &Id) -> io::Result<Vec<RegistroSessao>> {
        let mut r = self.do_projeto(projeto)?;
        r.retain(|s| s.analise.as_ref() == Some(analise));
        Ok(r)
    }

    /// Altera o registro de uma sessão sob a trava: `alterar` recebe o registro atual
    /// (ou `None`) e devolve o novo, ou `None` para não gravar nada. Devolve o
    /// registro anterior e o gravado.
    pub fn alterar(
        &self,
        id: &str,
        alterar: impl FnOnce(Option<RegistroSessao>) -> Option<RegistroSessao>,
    ) -> io::Result<Alteracao> {
        self.alterar_com(id, alterar, |_| ()).map(|(a, ())| a)
    }

    /// Como [`Sessoes::alterar`], rodando `depois` ainda sob a trava, com o
    /// resultado da alteração. O receptor de hooks leva o estado para a análise
    /// aqui, para dois eventos da mesma sessão chegarem à análise na mesma ordem em
    /// que chegaram ao registro.
    pub fn alterar_com<T>(
        &self,
        id: &str,
        alterar: impl FnOnce(Option<RegistroSessao>) -> Option<RegistroSessao>,
        depois: impl FnOnce(&Alteracao) -> T,
    ) -> io::Result<(Alteracao, T)> {
        let arquivo = self.arquivo(id).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "identificador de sessão inválido",
            )
        })?;
        let _trava = self.travar()?;
        let anterior = self.obter(id)?;
        let novo = alterar(anterior.clone());
        if let Some(novo) = &novo
            && Some(novo) != anterior.as_ref()
        {
            let mut json = serde_json::to_vec_pretty(novo).map_err(io::Error::other)?;
            json.push(b'\n');
            cba_kb::arquivo::gravar(&arquivo, &json)?;
        }
        let alteracao = Alteracao { anterior, novo };
        let resultado = depois(&alteracao);
        Ok((alteracao, resultado))
    }

    fn travar(&self) -> io::Result<File> {
        fs::create_dir_all(&self.pasta)?;
        let arquivo = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(self.pasta.join(TRAVA))?;
        // Liberada quando o arquivo é fechado.
        arquivo.lock()?;
        Ok(arquivo)
    }
}

/// Resultado de [`Sessoes::alterar`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Alteracao {
    pub anterior: Option<RegistroSessao>,
    /// O registro depois da alteração (`None` se `alterar` não quis gravar).
    pub novo: Option<RegistroSessao>,
}

impl Alteracao {
    /// Se algo foi gravado.
    pub fn mudou(&self) -> bool {
        self.novo.is_some() && self.novo != self.anterior
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cba_kb::Serie;

    #[test]
    fn ids_validos() {
        assert!(id_valido("3f2b8c1e-9a7d-4e2f-b1c3-0d4e5f6a7b8c"));
        assert!(id_valido("abc_123"));
        for invalido in ["", "../x", "a/b", "a b", "a.json", &"x".repeat(129)] {
            assert!(!id_valido(invalido), "{invalido}");
        }
    }

    #[test]
    fn grava_so_quando_muda_e_lista_por_recencia() {
        let dir = tempfile::tempdir().unwrap();
        let sessoes = Sessoes::em(dir.path().join("sessoes"));
        assert!(sessoes.listar().unwrap().is_empty());
        assert_eq!(sessoes.obter("s1").unwrap(), None);
        assert_eq!(sessoes.obter("../fora").unwrap(), None);

        let feito = sessoes
            .alterar("s1", |r| {
                assert!(r.is_none());
                Some(RegistroSessao::novo("s1", "faturamento-exemplo", 10))
            })
            .unwrap();
        assert!(feito.mudou());
        let feito = sessoes.alterar("s1", |r| r).unwrap();
        assert!(!feito.mudou());
        let feito = sessoes.alterar("s1", |_| None).unwrap();
        assert!(!feito.mudou());

        sessoes
            .alterar("s2", |_| {
                let mut r = RegistroSessao::novo("s2", "faturamento-exemplo", 20);
                r.analise = Some(Serie::Analise.id(1));
                Some(r)
            })
            .unwrap();
        sessoes
            .alterar("s3", |_| Some(RegistroSessao::novo("s3", "outro", 30)))
            .unwrap();
        let ids: Vec<_> = sessoes
            .listar()
            .unwrap()
            .into_iter()
            .map(|r| r.id)
            .collect();
        assert_eq!(ids, ["s3", "s2", "s1"]);
        let ids: Vec<_> = sessoes
            .do_projeto("faturamento-exemplo")
            .unwrap()
            .into_iter()
            .map(|r| r.id)
            .collect();
        assert_eq!(ids, ["s2", "s1"]);
        let da = sessoes
            .da_analise("faturamento-exemplo", &Serie::Analise.id(1))
            .unwrap();
        assert_eq!(da.len(), 1);
        assert_eq!(da[0].id, "s2");

        // A trava e arquivos estranhos não aparecem na lista.
        fs::write(sessoes.pasta().join("lixo.json"), "{").unwrap();
        assert_eq!(sessoes.listar().unwrap().len(), 3);
        assert!(sessoes.alterar("../x", |r| r).is_err());
    }

    #[test]
    fn encerrada_so_volta_com_reabertura() {
        let mut r = RegistroSessao::novo("s1", "x", 1);
        assert!(!r.mudar_estado(EstadoSessao::Ativa, false, 2));
        assert!(r.mudar_estado(EstadoSessao::AguardandoAnalista, false, 2));
        assert!(r.mudar_estado(EstadoSessao::Encerrada, false, 3));
        assert_eq!(r.fim_ms, Some(3));
        assert!(!r.mudar_estado(EstadoSessao::Ativa, false, 4));
        assert_eq!(r.estado, EstadoSessao::Encerrada);
        assert!(r.mudar_estado(EstadoSessao::Ativa, true, 5));
        assert_eq!(r.fim_ms, None);
    }

    #[test]
    fn json_legivel() {
        let mut r = RegistroSessao::novo("s1", "faturamento-exemplo", 1);
        r.analise = Some(Serie::Analise.id(193));
        r.estado = EstadoSessao::AguardandoAnalista;
        let json = serde_json::to_value(&r).unwrap();
        assert_eq!(json["analise"], "A-0193");
        assert_eq!(json["estado"], "aguardando-analista");
        assert_eq!(json["cliente"], "desconhecido");
        let volta: RegistroSessao = serde_json::from_value(json).unwrap();
        assert_eq!(volta, r);
    }

    #[test]
    fn threads_simultaneas_nao_perdem_alteracoes() {
        let dir = tempfile::tempdir().unwrap();
        let sessoes = Sessoes::em(dir.path());
        sessoes
            .alterar("s1", |_| Some(RegistroSessao::novo("s1", "x", 0)))
            .unwrap();
        std::thread::scope(|s| {
            for _ in 0..4 {
                let sessoes = sessoes.clone();
                s.spawn(move || {
                    for _ in 0..25 {
                        sessoes
                            .alterar("s1", |r| {
                                let mut r = r.unwrap();
                                r.atualizada_ms += 1;
                                Some(r)
                            })
                            .unwrap();
                    }
                });
            }
        });
        assert_eq!(sessoes.obter("s1").unwrap().unwrap().atualizada_ms, 100);
    }
}
