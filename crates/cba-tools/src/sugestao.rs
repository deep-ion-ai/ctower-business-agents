//! `registrar_sugestao`: sugestão de alteração do código, sem alterar o código
//! (spec, seção 4.6).
//!
//! A sugestão fica no estado da análise e ganha um documento próprio,
//! `sugestoes/S-NNNN.md`, montado pelo template de sugestão, que o documento da
//! análise cita com link. O trecho atual é conferido no arquivo como uma evidência
//! (com hash e commit), para a revalidação saber se o código mudou depois.

use cba_core::ambiente::Ambiente;
use cba_core::analise::NovaSugestao;
use cba_core::eventos::Mudanca;
use cba_core::tempo;
use cba_docs::{Conteudo, Templates, Texto, bloco_de_codigo};
use cba_kb::frontmatter::{FrontmatterSugestao, StatusSugestao};
use cba_kb::{Documento, Id, Serie, TipoDocumento};
use serde::Deserialize;
use serde_json::json;

use crate::comum;
use crate::documento::citacao;
use crate::evidencias::{self, ArquivoDoProjeto, EvidenciaRecusada, Motivo};
use crate::registro::{ErroFerramenta, Ferramenta, ler_argumentos};

#[derive(Debug, Deserialize)]
struct Args {
    analise: String,
    arquivo: String,
    #[serde(default)]
    titulo: Option<String>,
    trecho_atual: String,
    trecho_proposto: String,
    motivo: String,
    impacto: String,
    #[serde(default)]
    exemplo: Option<String>,
    #[serde(default)]
    linha_inicial: Option<u32>,
    #[serde(default)]
    linha_final: Option<u32>,
}

/// `registrar_sugestao`: quando o analista pede para corrigir ou implementar, o
/// agente explica o que mudaria e registra a sugestão para o time de
/// desenvolvimento, em vez de alterar o código.
pub fn registrar_sugestao() -> Ferramenta {
    Ferramenta::nova(
        "registrar_sugestao",
        "Registrar sugestão de alteração",
        "Registra uma sugestão de alteração do código para o time de desenvolvimento, \
         sem alterar o código: arquivo, trecho atual, trecho proposto, motivo e \
         impacto, de preferência com um exemplo com números do antes e depois. Use \
         quando o analista pedir para corrigir, mudar ou implementar algo. O trecho \
         atual é conferido no arquivo (como uma evidência); se ele aparecer em mais de \
         um lugar, informe linha_inicial e linha_final. A sugestão ganha um documento \
         próprio (S-NNNN) citado no documento da análise.",
        json!({
            "type": "object",
            "properties": {
                "analise": comum::schema_da_analise(),
                "arquivo": {
                    "type": "string",
                    "minLength": 1,
                    "description": "Arquivo que mudaria, relativo à raiz do projeto."
                },
                "titulo": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 200,
                    "description": "Título curto da sugestão. Se omitido, \"Alteração em <arquivo>\"."
                },
                "trecho_atual": {
                    "type": "string",
                    "maxLength": evidencias::LIMITE_DO_TRECHO,
                    "description": "Código atual, copiado do arquivo, que seria substituído. Vazio quando a sugestão só acrescenta código."
                },
                "trecho_proposto": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": evidencias::LIMITE_DO_TRECHO,
                    "description": "Código proposto no lugar do trecho atual."
                },
                "motivo": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 4000,
                    "description": "Por que mudar, em linguagem de negócio."
                },
                "impacto": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 4000,
                    "description": "O que muda para o negócio e para o projeto (dados já gravados, integrações, relatórios)."
                },
                "exemplo": {
                    "type": "string",
                    "maxLength": 4000,
                    "description": "Exemplo com números do antes e depois."
                },
                "linha_inicial": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Primeira linha do trecho atual, se ele aparece em mais de um lugar."
                },
                "linha_final": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Última linha do trecho atual (com linha_inicial)."
                }
            },
            "required": ["analise", "arquivo", "trecho_atual", "trecho_proposto", "motivo", "impacto"],
            "additionalProperties": false
        }),
        |ambiente, args| {
            let args: Args = ler_argumentos(args)?;
            let id = comum::id_da_analise(&args.analise)?;
            let analise = ambiente.analises.carregar(&id)?;

            let recusa = |motivo: Motivo, linhas| {
                ErroFerramenta::Recusa(format!(
                    "{}",
                    EvidenciaRecusada {
                        arquivo: args.arquivo.clone(),
                        linhas,
                        motivo,
                    }
                ))
            };
            let arquivo = ArquivoDoProjeto::abrir(&ambiente.escopo, &args.arquivo)
                .map_err(|m| recusa(m, None))?;
            let evidencia = if args.trecho_atual.trim().is_empty() {
                None
            } else {
                Some(match (args.linha_inicial, args.linha_final) {
                    (Some(i), Some(f)) => evidencias::intervalo(i, f)
                        .and_then(|l| arquivo.conferir(l, &args.trecho_atual))
                        .map_err(|m| recusa(m, Some((i, f))))?,
                    (None, None) => arquivo
                        .localizar(&args.trecho_atual)
                        .map_err(|m| recusa(m, None))?,
                    _ => {
                        return Err(ErroFerramenta::Argumentos(
                            "informe linha_inicial e linha_final juntas, ou nenhuma das duas"
                                .into(),
                        ));
                    }
                })
            };
            let titulo = args
                .titulo
                .as_deref()
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .map_or_else(
                    || format!("Alteração em {}", arquivo.relativo),
                    str::to_owned,
                );
            let mut nova = NovaSugestao {
                id: None,
                titulo,
                arquivo: arquivo.relativo.clone(),
                trecho_atual: args.trecho_atual,
                trecho_proposto: args.trecho_proposto,
                motivo: args.motivo,
                impacto: args.impacto,
                exemplo: args.exemplo,
                evidencia,
                sessao: None,
            };

            // Ensaio no estado atual: recusa (campo vazio, análise aprovada) antes de
            // reservar o S-NNNN.
            analise
                .clone()
                .registrar_sugestao(nova.clone(), 0)
                .map_err(comum::recusa)?;

            let sugestao = reservar(ambiente, &id, &nova)?;
            nova.id = Some(sugestao.clone());
            let aviso_do_documento = gravar_documento(ambiente, &sugestao, &id, &nova).err();

            let registrado = match comum::alterar(ambiente, &id, Mudanca::Sugestao, |a, agora| {
                a.registrar_sugestao(nova.clone(), agora)
            }) {
                Ok(r) => r,
                Err(e) => {
                    // A análise mudou entre o ensaio e o registro (aprovada, por
                    // exemplo): o documento reservado não serve a ninguém.
                    let _ = std::fs::remove_file(ambiente.kb.caminho(&sugestao.arquivo()));
                    return Err(e);
                }
            };
            let s = registrado
                .analise
                .sugestoes
                .iter()
                .find(|s| s.id.as_ref() == Some(&sugestao))
                .expect("sugestão recém-registrada");
            let mut dados = json!({
                "sugestao": s.numero,
                "id": sugestao,
                "documento_da_sugestao": ambiente.kb.na_base(&sugestao.arquivo()),
                "arquivo": s.arquivo,
                "trecho_atual": s.evidencia.as_ref().map(|e| json!({
                    "linhas": e.linhas,
                    "hash": e.hash,
                    "commit": e.commit,
                })),
            });
            if let Some(aviso) = aviso_do_documento {
                dados["aviso_da_sugestao"] = json!(aviso);
            }
            Ok(comum::resposta(ambiente, &registrado, dados))
        },
    )
}

fn frontmatter(
    ambiente: &Ambiente,
    id: &Id,
    analise: &Id,
    nova: &NovaSugestao,
) -> FrontmatterSugestao {
    FrontmatterSugestao {
        tipo: TipoDocumento::Sugestao,
        id: id.clone(),
        titulo: nova.titulo.trim().to_owned(),
        projeto: ambiente.kb.nome().to_owned(),
        status: StatusSugestao::Proposta,
        analise: Some(analise.clone()),
        arquivo: Some(nova.arquivo.clone()),
        criado: tempo::hoje(),
    }
}

/// Reserva o próximo `S-NNNN` com um documento provisório.
fn reservar(ambiente: &Ambiente, analise: &Id, nova: &NovaSugestao) -> Result<Id, ErroFerramenta> {
    let (id, _) = ambiente
        .kb
        .criar_numerado(&Serie::Sugestao, |id| {
            Documento::novo(
                frontmatter(ambiente, id, analise, nova),
                format!("# {id} · {}\n", nova.titulo.trim()),
            )
        })
        .map_err(|e| ErroFerramenta::Interno(e.to_string()))?;
    Ok(id)
}

/// Monta o documento da sugestão pelo template.
fn gravar_documento(
    ambiente: &Ambiente,
    id: &Id,
    analise: &Id,
    nova: &NovaSugestao,
) -> Result<(), String> {
    let linguagem = nova
        .arquivo
        .nome()
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .unwrap_or_default();
    let mut conteudo = Conteudo::novo(frontmatter(ambiente, id, analise, nova), id.arquivo());
    let documento_da_analise = analise.arquivo();
    conteudo.textos.push(Texto::novo(
        "contexto",
        format!(
            "Registrada na análise [{analise}](../{documento_da_analise}), sobre o arquivo \
             `{}`.",
            nova.arquivo
        ),
    ));
    let mut atual = if nova.trecho_atual.trim().is_empty() {
        Texto::novo("trecho_atual", "Nenhum: a sugestão só acrescenta código.")
    } else {
        Texto::novo(
            "trecho_atual",
            bloco_de_codigo(&nova.trecho_atual, &linguagem),
        )
    };
    atual.citacoes = nova.evidencia.iter().map(citacao).collect();
    conteudo.textos.push(atual);
    conteudo.textos.push(Texto::novo(
        "trecho_proposto",
        bloco_de_codigo(&nova.trecho_proposto, &linguagem),
    ));
    conteudo
        .textos
        .push(Texto::novo("motivo", nova.motivo.trim()));
    conteudo
        .textos
        .push(Texto::novo("impacto", nova.impacto.trim()));
    if let Some(exemplo) = nova.exemplo.as_deref().filter(|e| !e.trim().is_empty()) {
        conteudo.textos.push(Texto::novo("exemplo", exemplo.trim()));
    }
    let (templates, _) = Templates::da_base(&ambiente.base);
    templates
        .gravar(&ambiente.kb, &conteudo)
        .map(|_| ())
        .map_err(|e| {
            tracing::warn!("documento da sugestão não montado");
            format!("o documento da sugestão não foi montado: {e}")
        })
}
