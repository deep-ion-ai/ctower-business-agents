//! Comparação dos entregáveis da demanda (#464), o caso de uso e o épico com
//! quebra em histórias, com o gabarito.
//!
//! Determinística, como o resto da comparação. Lê os campos do item registrado
//! no rascunho atual do entregável (`demandas/<pasta>/entregaveis.estado.json`)
//! e mede:
//!
//! - **regras:** cada regra esperada é citada pelo item (no caso de uso, no
//!   campo `regras` ou no texto de uma etapa; no épico, por uma história);
//! - **caso de uso:** as etapas do fluxo principal (ao menos
//!   `minimo_de_etapas`), os fluxos alternativos e as exceções, as etapas sem
//!   regra citada e os atores, alternativos e exceções esperados (palavras-chave
//!   no texto);
//! - **épico:** as histórias (ao menos `minimo_de_historias`), as histórias sem
//!   regra, as esperadas (palavras-chave no título e na narrativa) e os riscos
//!   esperados (nos riscos, nas dependências ou no fora do escopo).
//!
//! Falhas graves: nada registrado; etapa ou história sem regra citada (a
//! ferramenta já recusa, então indica estado adulterado ou regressão); caso de
//! uso sem fluxo principal ou sem nenhum fluxo alternativo; épico sem história.

use cba_docs::entregaveis::{self as catalogo, regras_no_texto};
use serde::Serialize;
use serde_json::Value;

use crate::gabarito::{EntregavelEsperado, Gabarito, HistoriaEsperada, TextoEsperado};
use crate::texto;

/// Resultado da comparação de um entregável.
#[derive(Debug, Clone, Serialize)]
pub struct EntregavelAvaliado {
    pub tipo: String,
    /// O agente registrou o item.
    pub registrado: bool,
    pub regras: Vec<RegraAvaliada>,
    /// Caso de uso: etapas do fluxo principal, fluxos alternativos e exceções.
    pub etapas: usize,
    pub fluxos_alternativos: usize,
    pub excecoes: usize,
    pub minimo_de_etapas: usize,
    /// Etapas sem regra citada (`fluxo principal 2`, `FA1`).
    pub etapas_sem_regra: Vec<String>,
    /// Épico: histórias registradas e as que não citam regra (`H2`).
    pub historias: usize,
    pub minimo_de_historias: usize,
    pub historias_sem_regra: Vec<String>,
    /// Atores, alternativos, exceções, histórias e riscos esperados.
    pub esperados: Vec<ItemAvaliado>,
}

/// Regra esperada e onde o item a cita.
#[derive(Debug, Clone, Serialize)]
pub struct RegraAvaliada {
    pub id: String,
    /// `regras`, `fluxo principal 2`, `FE1`, `H1`.
    pub citada_em: Option<String>,
}

/// Um texto esperado e onde foi atendido.
#[derive(Debug, Clone, Serialize)]
pub struct ItemAvaliado {
    pub id: String,
    /// `ator`, `fluxo alternativo`, `exceção`, `história` ou `risco`.
    pub tipo: String,
    pub descricao: String,
    pub atendido_por: Option<String>,
}

impl EntregavelAvaliado {
    pub fn e_caso_de_uso(&self) -> bool {
        self.tipo == catalogo::CASO_DE_USO
    }

    /// Falhas graves.
    pub fn falhas(&self) -> Vec<String> {
        let mut f = Vec::new();
        if !self.registrado {
            f.push("nenhum item registrado no entregável".to_owned());
            return f;
        }
        if !self.etapas_sem_regra.is_empty() {
            f.push(format!(
                "etapas sem regra citada: {}",
                self.etapas_sem_regra.join(", ")
            ));
        }
        if !self.historias_sem_regra.is_empty() {
            f.push(format!(
                "histórias sem regra citada: {}",
                self.historias_sem_regra.join(", ")
            ));
        }
        if self.e_caso_de_uso() {
            if self.etapas == 0 {
                f.push("caso de uso sem fluxo principal".to_owned());
            }
            if self.fluxos_alternativos == 0 {
                f.push("caso de uso sem fluxo alternativo".to_owned());
            }
        } else if self.historias == 0 {
            f.push("épico sem história".to_owned());
        }
        f
    }

    pub fn regras_citadas(&self) -> usize {
        self.regras.iter().filter(|r| r.citada_em.is_some()).count()
    }

    pub fn esperados_atendidos(&self) -> usize {
        self.esperados
            .iter()
            .filter(|e| e.atendido_por.is_some())
            .count()
    }
}

fn textos(campos: &Value, nome: &str) -> Vec<String> {
    campos
        .get(nome)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}

fn texto(campos: &Value, nome: &str) -> String {
    campos
        .get(nome)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

/// Um trecho do item com o rótulo de onde ele está.
struct Trecho {
    rotulo: String,
    texto: String,
    /// Regras citadas no trecho.
    regras: Vec<String>,
}

fn etapas(campos: &Value, nome: &str, rotulo: impl Fn(usize) -> String) -> Vec<Trecho> {
    textos(campos, nome)
        .into_iter()
        .enumerate()
        .map(|(i, t)| Trecho {
            rotulo: rotulo(i + 1),
            regras: regras_no_texto(&t),
            texto: t,
        })
        .collect()
}

fn atendido<'a>(
    esperados: &'a [TextoEsperado],
    tipo: &'a str,
    trechos: &'a [Trecho],
) -> impl Iterator<Item = ItemAvaliado> + 'a {
    esperados.iter().map(move |e| ItemAvaliado {
        id: e.id.clone(),
        tipo: tipo.to_owned(),
        descricao: e.descricao.clone(),
        atendido_por: trechos
            .iter()
            .find(|t| texto::atende(&t.texto, &e.palavras_chave))
            .map(|t| t.rotulo.clone()),
    })
}

fn historia_esperada(e: &HistoriaEsperada) -> TextoEsperado {
    TextoEsperado {
        id: e.id.clone(),
        descricao: e.descricao.clone(),
        palavras_chave: e.palavras_chave.clone(),
    }
}

/// Compara os campos do item registrado (`None`, nada registrado) com o
/// esperado.
pub fn avaliar_entregavel(
    campos: Option<&Value>,
    esperado: &EntregavelEsperado,
) -> EntregavelAvaliado {
    let mut a = EntregavelAvaliado {
        tipo: esperado.tipo.clone(),
        registrado: campos.is_some(),
        regras: Vec::new(),
        etapas: 0,
        fluxos_alternativos: 0,
        excecoes: 0,
        minimo_de_etapas: esperado.minimo_de_etapas,
        etapas_sem_regra: Vec::new(),
        historias: 0,
        minimo_de_historias: esperado.minimo_de_historias,
        historias_sem_regra: Vec::new(),
        esperados: Vec::new(),
    };
    let vazio = Value::Null;
    let campos = campos.unwrap_or(&vazio);
    let mut trechos: Vec<Trecho> = Vec::new();
    if esperado.tipo == catalogo::CASO_DE_USO {
        let principal = etapas(campos, "fluxo_principal", |n| {
            format!("fluxo principal {n}")
        });
        let alternativos = etapas(campos, "fluxos_alternativos", |n| format!("FA{n}"));
        let excecoes = etapas(campos, "excecoes", |n| format!("FE{n}"));
        a.etapas = principal.len();
        a.fluxos_alternativos = alternativos.len();
        a.excecoes = excecoes.len();
        a.etapas_sem_regra = principal
            .iter()
            .chain(&alternativos)
            .chain(&excecoes)
            .filter(|t| t.regras.is_empty())
            .map(|t| t.rotulo.clone())
            .collect();
        let atores: Vec<Trecho> = textos(campos, "atores")
            .into_iter()
            .map(|t| Trecho {
                rotulo: "atores".into(),
                texto: t,
                regras: Vec::new(),
            })
            .collect();
        a.esperados
            .extend(atendido(&esperado.atores, "ator", &atores));
        a.esperados.extend(atendido(
            &esperado.fluxos_alternativos,
            "fluxo alternativo",
            &alternativos,
        ));
        a.esperados
            .extend(atendido(&esperado.excecoes, "exceção", &excecoes));
        trechos.push(Trecho {
            rotulo: "regras".into(),
            texto: String::new(),
            regras: textos(campos, "regras"),
        });
        trechos.extend(principal);
        trechos.extend(alternativos);
        trechos.extend(excecoes);
    } else {
        let historias: Vec<Trecho> = campos
            .get("historias")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .enumerate()
            .map(|(i, h)| Trecho {
                rotulo: format!("H{}", i + 1),
                texto: ["titulo", "papel", "acao", "beneficio"]
                    .iter()
                    .map(|c| texto(h, c))
                    .collect::<Vec<_>>()
                    .join(" "),
                regras: textos(h, "regras"),
            })
            .collect();
        a.historias = historias.len();
        a.historias_sem_regra = historias
            .iter()
            .filter(|h| h.regras.is_empty())
            .map(|h| h.rotulo.clone())
            .collect();
        let esperadas: Vec<TextoEsperado> = esperado
            .historias_esperadas
            .iter()
            .map(historia_esperada)
            .collect();
        a.esperados
            .extend(atendido(&esperadas, "história", &historias));
        let riscos: Vec<Trecho> = ["riscos", "dependencias", "fora_do_escopo"]
            .iter()
            .flat_map(|c| {
                textos(campos, c).into_iter().map(|t| Trecho {
                    rotulo: c.replace('_', " "),
                    texto: t,
                    regras: Vec::new(),
                })
            })
            .collect();
        a.esperados
            .extend(atendido(&esperado.riscos, "risco", &riscos));
        trechos = historias;
    }
    a.regras = esperado
        .regras
        .iter()
        .map(|id| RegraAvaliada {
            id: id.clone(),
            citada_em: trechos
                .iter()
                .find(|t| t.regras.contains(id))
                .map(|t| t.rotulo.clone()),
        })
        .collect();
    a
}

/// O que o preparo deixou para o agente: a demanda e o número do entregável.
#[derive(Debug, Clone)]
pub struct EntregavelPreparado {
    pub demanda: cba_kb::IdDemanda,
    pub numero: u32,
    pub prompt: String,
}

/// Prepara o entregável como o analista o teria deixado: a demanda do gabarito
/// na base, a análise de impacto de origem ligada a ela e aprovada (como nas
/// histórias: uma afirmação confirmada por regra da demanda, as regras na base
/// e uma pergunta em aberto por pergunta esperada), o tipo habilitado no
/// projeto e, no caso de uso, a funcionalidade do gabarito no mapa, com as
/// regras dela gravadas e ligadas. Depois, "Gerar" (a versão inicial no caso
/// de uso): devolve a demanda, o número do entregável e o prompt da sessão.
pub fn preparar_entregavel(
    ambiente: &mut cba_core::ambiente::Ambiente,
    dados: &cba_core::dados::DiretorioDados,
    gabarito: &Gabarito,
    esperado: &EntregavelEsperado,
) -> Result<EntregavelPreparado, String> {
    use cba_core::entregaveis::Finalidade;
    use cba_tools::entregaveis::{PedidoDeGeracao, gerar};

    let demanda = gabarito
        .demandas
        .iter()
        .find(|d| d.id == esperado.demanda)
        .ok_or_else(|| format!("a demanda {} não existe no gabarito", esperado.demanda))?;

    // O tipo habilitado no projeto.
    let tipo = catalogo::tipo(&esperado.tipo)
        .ok_or_else(|| format!("o tipo {} não está no catálogo", esperado.tipo))?;
    let mut projeto = ambiente.projeto.clone();
    let mut config = catalogo::efetiva(projeto.entregaveis.as_ref());
    if config.habilitado(tipo.id).is_none() {
        config.tipos.push(cba_index::TipoHabilitado::novo(tipo.id));
    }
    projeto.entregaveis = Some(catalogo::normalizar(config));
    cba_index::Cadastro::em(dados.projetos())
        .salvar(&projeto)
        .map_err(|e| format!("cadastro do projeto: {e}"))?;
    ambiente.projeto = projeto;

    let lida = ambiente
        .kb
        .criar_demanda(
            &cba_kb::NovaDemanda {
                titulo: crate::gabarito::titulo_da_demanda(&demanda.texto),
                descricao: Some(demanda.texto.clone()),
                chave: Some(demanda.id.clone()),
                origem: None,
                tipo: None,
            },
            cba_kb::frontmatter::Data::nova(2026, 11, 28).expect("data válida"),
        )
        .map_err(|e| format!("demanda: {e}"))?;
    let origem =
        crate::historias::preparar_origem_da_demanda(ambiente, gabarito, demanda, Some(lida.id))?;

    // O caso de uso é vivo: a funcionalidade vai para o mapa, com as regras
    // dela ligadas e, como ponto de entrada, a primeira evidência delas.
    let funcionalidade = match &esperado.funcionalidade {
        None => None,
        Some(id) => Some(registrar_no_mapa(ambiente, gabarito, id)?),
    };
    let g = gerar(
        ambiente,
        PedidoDeGeracao {
            demanda: lida.id,
            tipo: tipo.id.to_owned(),
            analise: origem,
            finalidade: if funcionalidade.is_some() {
                Finalidade::VersaoInicial
            } else {
                Finalidade::Demanda
            },
            funcionalidade,
        },
    )
    .map_err(|e| format!("gerar o entregável: {e}"))?;
    Ok(EntregavelPreparado {
        demanda: lida.id,
        numero: g.numero,
        prompt: g.prompt,
    })
}

/// Registra a funcionalidade do gabarito no mapa, com as regras dela gravadas
/// na base e ligadas a ela; o ponto de entrada é a primeira evidência das
/// regras. Devolve o identificador no mapa.
fn registrar_no_mapa(
    ambiente: &cba_core::ambiente::Ambiente,
    gabarito: &Gabarito,
    id: &str,
) -> Result<cba_kb::IdFuncionalidade, String> {
    use cba_core::funcionalidades::{MapaFuncionalidades, NovaFuncionalidade, PontoAtribuido};

    let f = gabarito
        .funcionalidades
        .iter()
        .find(|f| f.id == id)
        .ok_or_else(|| format!("a funcionalidade {id} não existe no gabarito"))?;
    let regras: Vec<&crate::gabarito::Regra> = f
        .regras
        .iter()
        .filter(|r| r.starts_with("RN-"))
        .map(|r| {
            gabarito
                .regra(r)
                .ok_or_else(|| format!("a regra {r} não existe no gabarito"))
        })
        .collect::<Result<_, _>>()?;
    let primeira = regras
        .first()
        .and_then(|r| r.evidencias.first())
        .ok_or_else(|| format!("{id}: sem regra com evidência para o ponto de entrada"))?;
    let evidencia = cba_tools::evidencias::validar(
        &ambiente.escopo,
        &cba_tools::evidencias::EvidenciaCitada {
            arquivo: primeira.arquivo.clone(),
            linha_inicial: primeira.linha_inicial,
            linha_final: primeira.linha_final,
            trecho: primeira.trecho.clone(),
        },
    )
    .map_err(|r| format!("{id}: ponto de entrada: {r:?}"))?;
    let nome_do_ponto = primeira
        .arquivo
        .rsplit('/')
        .next()
        .unwrap_or(&primeira.arquivo)
        .to_owned();
    let sigla = cba_tools::aprovacao::sigla_do_slug("parcelamento");
    let (_, fid) = MapaFuncionalidades::alterar(&ambiente.kb, |m, agora| {
        let fid = m.proximo_id(&sigla)?;
        m.registrar(
            NovaFuncionalidade {
                nome: f.nome.clone(),
                area: "parcelamento".into(),
                area_nome: "Parcelamento".into(),
                descricao: f.objetivo.clone(),
                atores: Vec::new(),
                pontos: vec![PontoAtribuido {
                    chave: None,
                    tipo: None,
                    nome: nome_do_ponto,
                    evidencia,
                }],
                dados_lidos: Vec::new(),
                dados_gravados: Vec::new(),
                relacionadas: Vec::new(),
                sessao: None,
            },
            fid.clone(),
            agora,
        )?;
        Ok(fid)
    })
    .map_err(|e| format!("mapa: {e}"))?;
    for regra in regras {
        crate::historias::gravar_regra_ligada(ambiente, regra, None, vec![fid.clone()])?;
    }
    Ok(fid)
}

/// Os campos do último item do rascunho atual do entregável.
pub fn campos_registrados(
    ambiente: &cba_core::ambiente::Ambiente,
    preparado: &EntregavelPreparado,
) -> Result<Option<Value>, String> {
    let lida = ambiente
        .kb
        .demanda(preparado.demanda)
        .map_err(|e| format!("demanda: {e}"))?;
    let estado = cba_core::entregaveis::EntregaveisDaDemanda::carregar(&ambiente.kb, &lida.pasta)
        .map_err(|e| format!("estado dos entregáveis: {e}"))?;
    Ok(estado
        .entregavel(preparado.numero)
        .and_then(|e| e.atual().itens.last())
        .map(|i| i.campos.clone()))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn gabarito() -> Gabarito {
        Gabarito::carregar(&crate::gabarito_padrao()).unwrap()
    }

    #[test]
    fn caso_de_uso_com_fluxo_alternativo_e_etapas_com_regra() {
        let g = gabarito();
        let e = &g.entregavel("ENT-FAT-001").unwrap().esperado;
        let campos = json!({
            "titulo": "Parcelar fatura",
            "objetivo": "Dividir a fatura em aberto em parcelas mensais.",
            "atores": ["Atendente do faturamento"],
            "fluxo_principal": [
                "O atendente escolhe uma fatura ABERTA (RN-FAT-010).",
                "O atendente informa de 2 a 6 parcelas (RN-FAT-008).",
                "O sistema confere que o cliente é pessoa jurídica ou órgão público (RN-FAT-007).",
                "O sistema confere a parcela mínima de R$ 50,00 (RN-FAT-009).",
                "O sistema gera as parcelas a cada 30 dias (RN-FAT-011)."
            ],
            "fluxos_alternativos": [
                "Na etapa 4, se o cliente é órgão público, não há valor mínimo de parcela (RN-FAT-009)."
            ],
            "excecoes": [
                "Na etapa 3, se o cliente é pessoa física, o sistema recusa o parcelamento (RN-FAT-007).",
                "Na etapa 2, quantidade fora do intervalo de 2 a MAX_PARCELAS é recusada (RN-FAT-008).",
                "Na etapa 1, fatura que não está ABERTA é recusada (RN-FAT-010).",
                "Na etapa 4, parcela abaixo do mínimo é recusada (RN-FAT-009)."
            ],
            "regras": ["RN-FAT-007", "RN-FAT-008", "RN-FAT-009", "RN-FAT-010", "RN-FAT-011"]
        });
        // O esquema do catálogo aceita o mesmo item.
        let uc = catalogo::tipo(catalogo::CASO_DE_USO).unwrap();
        assert_eq!(uc.validar_campos(&campos), Ok(()));
        let a = avaliar_entregavel(Some(&campos), e);
        assert!(a.falhas().is_empty(), "{:?}", a.falhas());
        assert_eq!((a.etapas, a.fluxos_alternativos, a.excecoes), (5, 1, 4));
        assert_eq!(a.regras_citadas(), 5);
        assert_eq!(a.regras[0].citada_em.as_deref(), Some("regras"));
        assert_eq!(
            a.esperados_atendidos(),
            a.esperados.len(),
            "{:#?}",
            a.esperados
        );
        let fa = a
            .esperados
            .iter()
            .find(|x| x.id == "ENT-FAT-001-FA1")
            .unwrap();
        assert_eq!(fa.atendido_por.as_deref(), Some("FA1"));

        // Sem alternativo e com etapa sem regra: falhas graves.
        let mut ruim = campos.clone();
        ruim["fluxos_alternativos"] = json!([]);
        ruim["fluxo_principal"][1] = json!("O atendente informa as parcelas.");
        let a = avaliar_entregavel(Some(&ruim), e);
        let falhas = a.falhas();
        assert!(
            falhas.contains(&"etapas sem regra citada: fluxo principal 2".to_owned()),
            "{falhas:?}"
        );
        assert!(
            falhas.contains(&"caso de uso sem fluxo alternativo".to_owned()),
            "{falhas:?}"
        );

        let nada = avaliar_entregavel(None, e);
        assert_eq!(nada.falhas(), ["nenhum item registrado no entregável"]);
        assert_eq!(nada.regras_citadas(), 0);
    }

    #[test]
    fn epico_cobre_as_regras_com_as_historias() {
        let g = gabarito();
        let e = &g.entregavel("ENT-FAT-002").unwrap().esperado;
        let historia = |titulo: &str, acao: &str, regras: &[&str]| {
            json!({
                "titulo": titulo, "papel": "atendente", "acao": acao,
                "beneficio": "atender o cliente pessoa física",
                "criterios": ["Dado um PF, quando pede 3 parcelas, então são geradas 3"],
                "regras": regras,
            })
        };
        let campos = json!({
            "titulo": "Parcelamento para pessoa física",
            "objetivo": "Permitir que pessoa física parcele em até 3 vezes.",
            "historias": [
                historia("Liberar o parcelamento para pessoa física", "parcelar a fatura de PF", &["RN-FAT-007"]),
                historia("Limite de 3 parcelas e mínimo de R$ 100,00 para PF", "usar os limites por tipo", &["RN-FAT-008", "RN-FAT-009"]),
            ],
            "riscos": ["MAX_PARCELAS e VALOR_MIN_PARCELA são parâmetros únicos: mudá-los afeta os outros tipos de cliente."],
        });
        let ep = catalogo::tipo(catalogo::EPICO).unwrap();
        assert_eq!(ep.validar_campos(&campos), Ok(()));
        let a = avaliar_entregavel(Some(&campos), e);
        assert!(a.falhas().is_empty(), "{:?}", a.falhas());
        assert_eq!(a.historias, 2);
        assert_eq!(a.regras_citadas(), 3);
        assert_eq!(a.regras[2].citada_em.as_deref(), Some("H2"));
        assert_eq!(a.esperados_atendidos(), 3, "{:#?}", a.esperados);

        // Uma história só, sem regra: a cobertura cai e a falha aparece.
        let uma = json!({
            "titulo": "x", "objetivo": "y",
            "historias": [historia("Liberar PF", "parcelar", &[])],
        });
        let a = avaliar_entregavel(Some(&uma), e);
        assert_eq!(a.regras_citadas(), 0);
        assert_eq!(a.falhas(), ["histórias sem regra citada: H1"]);
    }
}
