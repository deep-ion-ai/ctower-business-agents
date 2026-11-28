import type {
  CicloDeVida,
  EntregaveisDoProjeto,
  Geracao,
  Grupo,
  TipoEntregavel,
} from "../entregaveis";

/** Tipo do catálogo fictício, no formato que o app devolve (`cba_docs::entregaveis`). */
function tipo(
  id: string,
  nome: string,
  grupo: Grupo,
  ciclo: CicloDeVida,
  geracao: Geracao,
  disponivel = false,
  template = id,
): TipoEntregavel {
  return {
    id,
    nome,
    descricao: `Descrição de ${nome}.`,
    grupo,
    ciclo,
    geracao,
    campos: [{ nome: "regras", rotulo: "Regras de origem", tipo: "regras", obrigatorio: true }],
    template,
    fontes: ["analise", "regras"],
    disponivel,
  };
}

/** O catálogo e as predefinições como o núcleo os define. */
export function entregaveisFicticios(
  parcial: Partial<EntregaveisDoProjeto> = {},
): EntregaveisDoProjeto {
  return {
    catalogo: [
      tipo("historia-usuario", "História de usuário", "agil", "por_demanda", "agente", true, "historias"),
      tipo("criterios-gherkin", "Critérios em Gherkin", "agil", "por_demanda", "app"),
      tipo("cenarios-feature", "Cenários .feature", "agil", "vivo", "app"),
      tipo("epico", "Épico com quebra em histórias", "agil", "por_demanda", "agente"),
      tipo("resumo-refinamento", "Resumo para refinamento", "agil", "por_demanda", "agente"),
      tipo("definition-of-ready", "Definition of Ready", "agil", "por_demanda", "app"),
      tipo("caso-de-uso", "Especificação de caso de uso", "rup", "vivo", "agente"),
      tipo("catalogo-regras", "Catálogo de regras de negócio", "rup", "vivo", "app"),
      tipo("especificacao-suplementar", "Especificação suplementar", "rup", "vivo", "app"),
      tipo("casos-de-teste", "Casos de teste", "rup", "por_demanda", "agente", true, "cenarios-teste"),
      tipo("documento-visao", "Documento de visão", "rup", "por_demanda", "agente"),
      tipo("prototipo", "Protótipo", "comuns", "por_demanda", "agente", true),
      tipo("glossario", "Glossário", "comuns", "vivo", "app"),
      tipo("manual", "Manual do usuário", "comuns", "vivo", "agente", true),
      tipo("documento-negocial", "Documento negocial", "comuns", "vivo", "agente", true, "funcionalidade"),
    ],
    predefinicoes: [
      {
        metodologia: "agil",
        tipos: ["epico", "historia-usuario", "criterios-gherkin", "cenarios-feature", "prototipo"],
        padrao: "epico",
      },
      {
        metodologia: "rup",
        tipos: ["caso-de-uso", "catalogo-regras", "especificacao-suplementar", "prototipo", "glossario"],
        padrao: "caso-de-uso",
      },
    ],
    // Projeto antigo: os tipos que o app já gerava.
    entregaveis: {
      metodologia: "personalizado",
      tipos: ["historia-usuario", "casos-de-teste", "prototipo", "manual", "documento-negocial"].map(
        (t) => ({ tipo: t, destino: "exportar" as const }),
      ),
      padrao: "historia-usuario",
    },
    configurada: false,
    templatesDaBase: ["caso-de-uso-empresa.md"],
    ...parcial,
  };
}
