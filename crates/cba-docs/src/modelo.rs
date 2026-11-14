//! Seções e campos de cada tipo de documento (spec, seção 7.3) e os valores dos
//! campos tirados do frontmatter ([`Montavel`]).

use cba_kb::frontmatter::{
    Abrangencia, Frontmatter, FrontmatterAnalise, FrontmatterAta, FrontmatterFuncionalidade,
    FrontmatterRegra, FrontmatterSugestao, FrontmatterTermo, StatusAnalise, StatusAta, StatusRegra,
    StatusSugestao,
};
use cba_kb::{CaminhoRelativo, TipoDocumento, layout};

use crate::link;

/// Seção fixa de um tipo de documento.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Secao {
    /// Nome do marcador no template (`como_e_hoje`) e da seção nos registros.
    pub marcador: &'static str,
    /// Título usado quando a seção vai para o fim do documento por falta do
    /// marcador no template.
    pub titulo: &'static str,
}

/// Seções e campos de um tipo de documento, e o template padrão embutido.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Modelo {
    pub tipo: TipoDocumento,
    /// Seções na ordem do template padrão.
    pub secoes: &'static [Secao],
    /// Marcadores de campo, preenchidos pelo frontmatter ([`Montavel::campos`]).
    pub campos: &'static [&'static str],
    /// Template padrão, embutido no app.
    pub padrao: &'static str,
}

impl Modelo {
    pub fn secao(&self, marcador: &str) -> Option<&'static Secao> {
        self.secoes.iter().find(|s| s.marcador == marcador)
    }

    /// Se o marcador existe neste tipo (seção ou campo).
    pub fn conhece(&self, marcador: &str) -> bool {
        self.secao(marcador).is_some() || self.campos.contains(&marcador)
    }
}

/// Marcadores com papel especial em qualquer tipo.
pub(crate) const PERGUNTAS: &str = "perguntas";
pub(crate) const SUGESTOES: &str = "sugestoes";
pub(crate) const EVIDENCIAS: &str = "evidencias";
/// Seções da ata com texto próprio quando vazias.
pub(crate) const PARTICIPANTES: &str = "participantes";
pub(crate) const REFERENCIAS_FALAS: &str = "referencias_falas";

const fn s(marcador: &'static str, titulo: &'static str) -> Secao {
    Secao { marcador, titulo }
}

const SUGESTOES_SECAO: Secao = s(SUGESTOES, "Sugestões de alteração");
const EVIDENCIAS_SECAO: Secao = s(EVIDENCIAS, "Evidências");

const CAMPOS_ANALISE: &[&str] = &[
    "id",
    "titulo",
    "projeto",
    "status",
    "criado",
    "analista",
    "abrangencia_estimada",
    "versao_do_codigo",
];

const CAMPOS_FUNCIONALIDADE: &[&str] = &["titulo", "area", "tema", "status", "origem"];

/// O documento negocial pode vir de uma análise (`analises/A-NNNN.md`) ou da
/// pasta da funcionalidade; o template conhece os campos dos dois.
const CAMPOS_FUNCIONALIDADE_OU_ANALISE: &[&str] = &[
    "id",
    "titulo",
    "projeto",
    "status",
    "criado",
    "analista",
    "abrangencia_estimada",
    "versao_do_codigo",
    "area",
    "tema",
    "origem",
];

/// Tipos que têm template, na ordem da spec (seção 7.3).
pub const MODELOS: [Modelo; 8] = [
    Modelo {
        tipo: TipoDocumento::AnaliseImpacto,
        secoes: &[
            s("demanda_como_veio", "Demanda como veio"),
            s("demanda_como_entendida", "Demanda como entendida"),
            s("como_e_hoje", "Como é hoje"),
            s("funcionalidades", "Funcionalidades"),
            s("dados", "Dados"),
            s("integracoes", "Integrações"),
            s("documentos_e_telas", "Documentos e telas"),
            s("riscos", "Riscos"),
            s("abrangencia", "Abrangência"),
            s(PERGUNTAS, "Perguntas para o solicitante"),
            SUGESTOES_SECAO,
            EVIDENCIAS_SECAO,
        ],
        campos: CAMPOS_ANALISE,
        padrao: include_str!("../templates/analise-impacto.md"),
    },
    Modelo {
        tipo: TipoDocumento::Pergunta,
        secoes: &[
            s("pergunta", "Pergunta"),
            s("resposta", "Resposta"),
            s("regras_encontradas", "Regras encontradas"),
            s("exemplo", "Exemplo"),
            s(PERGUNTAS, "Perguntas em aberto"),
            SUGESTOES_SECAO,
            EVIDENCIAS_SECAO,
        ],
        campos: CAMPOS_ANALISE,
        padrao: include_str!("../templates/pergunta.md"),
    },
    Modelo {
        tipo: TipoDocumento::Funcionalidade,
        secoes: &[
            s("objetivo", "Objetivo"),
            s("atores_e_telas", "Atores e telas"),
            s("fluxo_principal", "Fluxo principal"),
            s("fluxos_alternativos", "Fluxos alternativos"),
            s("excecoes", "Exceções"),
            s("regras", "Regras de negócio"),
            s("dados", "Dados lidos e gravados"),
            s("integracoes", "Integrações"),
            s(PERGUNTAS, "Perguntas em aberto"),
            SUGESTOES_SECAO,
            EVIDENCIAS_SECAO,
        ],
        campos: CAMPOS_FUNCIONALIDADE_OU_ANALISE,
        padrao: include_str!("../templates/funcionalidade.md"),
    },
    Modelo {
        tipo: TipoDocumento::CasoDeUso,
        secoes: &[
            s("identificacao", "Identificação"),
            s("objetivo", "Objetivo"),
            s("gatilho", "Gatilho"),
            s("pre_condicoes", "Pré-condições"),
            s("fluxo_principal", "Fluxo principal"),
            s("fluxos_alternativos", "Fluxos alternativos"),
            s("excecoes", "Exceções"),
            s("regras", "Regras de negócio"),
            EVIDENCIAS_SECAO,
        ],
        campos: CAMPOS_FUNCIONALIDADE,
        padrao: include_str!("../templates/caso-de-uso.md"),
    },
    Modelo {
        tipo: TipoDocumento::Regra,
        secoes: &[s("enunciado", "Enunciado"), EVIDENCIAS_SECAO],
        campos: &["id", "titulo", "area", "status", "origem"],
        padrao: include_str!("../templates/regra.md"),
    },
    Modelo {
        tipo: TipoDocumento::Termo,
        secoes: &[
            s("definicao", "Definição de negócio"),
            s("onde_vive_no_codigo", "Onde vive no código"),
            EVIDENCIAS_SECAO,
        ],
        campos: &["termo", "sinonimos", "area", "origem"],
        padrao: include_str!("../templates/termo.md"),
    },
    Modelo {
        tipo: TipoDocumento::Sugestao,
        secoes: &[
            s("contexto", "Contexto"),
            s("trecho_atual", "Trecho atual"),
            s("trecho_proposto", "Trecho proposto"),
            s("motivo", "Motivo"),
            s("impacto", "Impacto"),
            s("exemplo", "Exemplo"),
            EVIDENCIAS_SECAO,
        ],
        campos: &[
            "id", "titulo", "projeto", "status", "analise", "arquivo", "criado",
        ],
        padrao: include_str!("../templates/sugestao.md"),
    },
    Modelo {
        tipo: TipoDocumento::Ata,
        secoes: &[
            s("identificacao", "Identificação"),
            s(PARTICIPANTES, "Participantes e papéis"),
            s("objetivo", "Objetivo"),
            s("resumo", "Resumo"),
            s("decisoes", "Decisões"),
            s("necessidades", "Necessidades e requisitos levantados"),
            s("regras", "Regras de negócio citadas"),
            s("pendencias", "Pendências"),
            s("duvidas", "Dúvidas em aberto"),
            s("proximos_passos", "Próximos passos"),
            s(REFERENCIAS_FALAS, "Referências às falas"),
            EVIDENCIAS_SECAO,
        ],
        campos: &["id", "projeto", "demanda", "data", "duracao", "status"],
        padrao: include_str!("../templates/ata.md"),
    },
];

/// Modelo do tipo, se o tipo tiver template (`projeto` e `memoria-area` não têm).
pub fn modelo(tipo: TipoDocumento) -> Option<&'static Modelo> {
    MODELOS.iter().find(|m| m.tipo == tipo)
}

/// Texto da seção vazia.
pub(crate) fn texto_vazio(marcador: &str) -> &'static str {
    match marcador {
        PERGUNTAS => "_Nenhuma pergunta em aberto._",
        SUGESTOES => "_Nenhuma sugestão registrada._",
        EVIDENCIAS => "_Nenhuma evidência registrada._",
        // Os campos da identificação já estão no template padrão.
        "identificacao" => "",
        PARTICIPANTES => "_Nenhum participante registrado._",
        REFERENCIAS_FALAS => "_Nenhuma fala citada._",
        _ => "_Nada registrado._",
    }
}

/// Frontmatter que pode ser montado: diz o tipo e os valores dos campos.
pub trait Montavel: Frontmatter + Clone {
    /// Tipo do documento (o campo `tipo`).
    fn tipo(&self) -> TipoDocumento;

    /// Valores dos marcadores de campo, já em Markdown. `caminho` é onde o
    /// documento fica, relativo à raiz do projeto, para os links relativos.
    fn campos(&self, caminho: &CaminhoRelativo) -> Vec<(&'static str, String)>;
}

const NAO_INFORMADO: &str = "não informado";
const NAO_INFORMADA: &str = "não informada";

fn status_analise(s: StatusAnalise) -> &'static str {
    match s {
        StatusAnalise::Rascunho => "rascunho",
        StatusAnalise::EmRevisao => "em revisão",
        StatusAnalise::Aprovada => "aprovada",
    }
}

fn origem(caminho: &CaminhoRelativo, origem: Option<&CaminhoRelativo>) -> String {
    match origem {
        Some(o) => link(o.as_str(), caminho, o),
        None => NAO_INFORMADA.into(),
    }
}

impl Montavel for FrontmatterAnalise {
    fn tipo(&self) -> TipoDocumento {
        self.tipo
    }

    fn campos(&self, _caminho: &CaminhoRelativo) -> Vec<(&'static str, String)> {
        let abrangencia = match self.abrangencia {
            Some(Abrangencia::Pequena) => "pequena",
            Some(Abrangencia::Media) => "média",
            Some(Abrangencia::Grande) => "grande",
            None => "a definir",
        };
        let versao = match &self.base {
            Some(v) => match (&v.branch, &v.commit) {
                (Some(b), Some(c)) => format!("`{b}` @ `{c}`"),
                (Some(b), None) => format!("`{b}`"),
                (None, Some(c)) => format!("`{c}`"),
                (None, None) => NAO_INFORMADA.into(),
            },
            None => NAO_INFORMADA.into(),
        };
        vec![
            ("id", self.id.to_string()),
            ("titulo", self.titulo.clone()),
            ("projeto", self.projeto.clone()),
            ("status", status_analise(self.status).into()),
            ("criado", self.criado.to_string()),
            (
                "analista",
                self.analista
                    .clone()
                    .unwrap_or_else(|| NAO_INFORMADO.into()),
            ),
            ("abrangencia_estimada", abrangencia.into()),
            ("versao_do_codigo", versao),
        ]
    }
}

impl Montavel for FrontmatterFuncionalidade {
    fn tipo(&self) -> TipoDocumento {
        self.tipo
    }

    fn campos(&self, caminho: &CaminhoRelativo) -> Vec<(&'static str, String)> {
        vec![
            ("titulo", self.titulo.clone()),
            ("area", self.area.clone()),
            ("tema", self.tema.clone()),
            ("status", status_analise(self.status).into()),
            ("origem", origem(caminho, self.origem.as_ref())),
        ]
    }
}

impl Montavel for FrontmatterRegra {
    fn tipo(&self) -> TipoDocumento {
        self.tipo
    }

    fn campos(&self, caminho: &CaminhoRelativo) -> Vec<(&'static str, String)> {
        let status = match self.status {
            StatusRegra::Confirmada => "confirmada",
            StatusRegra::ARevalidar => "a revalidar",
            StatusRegra::Obsoleta => "obsoleta",
        };
        vec![
            ("id", self.id.to_string()),
            ("titulo", self.titulo.clone()),
            ("area", self.area.clone()),
            ("status", status.into()),
            ("origem", origem(caminho, self.origem.as_ref())),
        ]
    }
}

impl Montavel for FrontmatterTermo {
    fn tipo(&self) -> TipoDocumento {
        self.tipo
    }

    fn campos(&self, caminho: &CaminhoRelativo) -> Vec<(&'static str, String)> {
        let sinonimos = if self.sinonimos.is_empty() {
            "nenhum".into()
        } else {
            self.sinonimos.join(", ")
        };
        vec![
            ("termo", self.termo.clone()),
            ("sinonimos", sinonimos),
            (
                "area",
                self.area.clone().unwrap_or_else(|| NAO_INFORMADA.into()),
            ),
            ("origem", origem(caminho, self.origem.as_ref())),
        ]
    }
}

impl Montavel for FrontmatterSugestao {
    fn tipo(&self) -> TipoDocumento {
        self.tipo
    }

    fn campos(&self, caminho: &CaminhoRelativo) -> Vec<(&'static str, String)> {
        let status = match self.status {
            StatusSugestao::Proposta => "proposta",
            StatusSugestao::Aceita => "aceita",
            StatusSugestao::Recusada => "recusada",
        };
        let analise = match &self.analise {
            Some(a) => link(&a.to_string(), caminho, &a.arquivo()),
            None => NAO_INFORMADA.into(),
        };
        let arquivo = match &self.arquivo {
            Some(a) => format!("`{a}`"),
            None => NAO_INFORMADO.into(),
        };
        vec![
            ("id", self.id.to_string()),
            ("titulo", self.titulo.clone()),
            ("projeto", self.projeto.clone()),
            ("status", status.into()),
            ("analise", analise),
            ("arquivo", arquivo),
            ("criado", self.criado.to_string()),
        ]
    }
}

impl Montavel for FrontmatterAta {
    fn tipo(&self) -> TipoDocumento {
        self.tipo
    }

    fn campos(&self, caminho: &CaminhoRelativo) -> Vec<(&'static str, String)> {
        let status = match self.status {
            StatusAta::Rascunho => "rascunho",
            StatusAta::Aprovada => "aprovada",
        };
        let id = self.demanda.to_string();
        let demanda = match demanda_da_ata(caminho) {
            Some(d) => link(&id, caminho, &d),
            None => id,
        };
        vec![
            ("id", self.id.to_string()),
            ("projeto", self.projeto.clone()),
            ("demanda", demanda),
            ("data", self.data.to_string()),
            (
                "duracao",
                self.duracao
                    .map_or_else(|| NAO_INFORMADA.into(), |d| d.to_string()),
            ),
            ("status", status.into()),
        ]
    }
}

/// `demanda.md` da demanda de uma ata em `<pasta da demanda>/reunioes/R-NNNN.md`.
fn demanda_da_ata(ata: &CaminhoRelativo) -> Option<CaminhoRelativo> {
    let partes: Vec<&str> = ata.partes().collect();
    match partes.as_slice() {
        [pasta @ .., reunioes, _] if *reunioes == layout::REUNIOES && !pasta.is_empty() => {
            CaminhoRelativo::novo(&format!("{}/{}", pasta.join("/"), layout::ARQUIVO_DEMANDA)).ok()
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn todo_tipo_da_issue_tem_modelo() {
        for tipo in [
            TipoDocumento::AnaliseImpacto,
            TipoDocumento::Pergunta,
            TipoDocumento::Funcionalidade,
            TipoDocumento::CasoDeUso,
            TipoDocumento::Regra,
            TipoDocumento::Termo,
            TipoDocumento::Sugestao,
            TipoDocumento::Ata,
        ] {
            assert_eq!(modelo(tipo).unwrap().tipo, tipo);
        }
        assert!(modelo(TipoDocumento::Projeto).is_none());
        assert!(modelo(TipoDocumento::MemoriaArea).is_none());
    }

    #[test]
    fn todo_modelo_termina_em_evidencias_e_nao_repete_marcador() {
        for m in &MODELOS {
            assert_eq!(m.secoes.last().unwrap().marcador, EVIDENCIAS, "{}", m.tipo);
            let mut nomes: Vec<&str> = m.secoes.iter().map(|s| s.marcador).collect();
            nomes.extend(m.campos);
            let total = nomes.len();
            nomes.sort_unstable();
            nomes.dedup();
            assert_eq!(nomes.len(), total, "marcador repetido em {}", m.tipo);
        }
    }
}
