import type { AmbienteClaude } from "./analise";
import type { Atividade, ResumoProjeto, TipoAtividade } from "./resumoProjeto";

/**
 * Grupos das Atividades (#321, §7.1 da revisão de UX), na ordem de prioridade do
 * resumo do projeto (#318). "Ambiente a preparar" não vem no resumo: a tela junta o
 * `verificar_ambiente_projeto`.
 */
export type GrupoAtividade = "responder" | "revisar" | "demandas" | "revalidar" | "ambiente" | "paradas";

export interface DefinicaoGrupo {
  id: GrupoAtividade;
  titulo: string;
  /** O que é o grupo, no "?" ao lado do título. */
  ajuda: string;
  /** Grupo vazio não some: a frase ensina o que aparece ali. */
  vazio: string;
}

/**
 * "Demandas novas sem análise" fica oculto até a #322 (issues do rastreador com
 * cache): hoje o resumo só conhece as demandas da base.
 */
export const MOSTRAR_DEMANDAS_NOVAS = false;

export const GRUPOS_ATIVIDADES: DefinicaoGrupo[] = [
  {
    id: "responder",
    titulo: "O agente aguarda você",
    ajuda: "O agente parou porque precisa de uma resposta ou de uma decisão sua.",
    vazio: "Quando o agente parar para fazer uma pergunta ou pedir uma decisão sua, a análise aparece aqui.",
  },
  {
    id: "revisar",
    titulo: "Revisar e aprovar",
    ajuda:
      "O agente concluiu. Confirme ou marque como incorreta cada afirmação; ao aprovar, as confirmadas viram regras da base.",
    vazio: "Quando o agente concluir uma análise, ela aparece aqui para você conferir e aprovar.",
  },
  {
    id: "demandas",
    titulo: "Demandas novas sem análise",
    ajuda: "Demandas abertas que ainda não têm nenhuma análise ligada.",
    vazio: "Quando chegar uma demanda nova, ela aparece aqui para você analisar o impacto.",
  },
  {
    id: "revalidar",
    titulo: "Regras a revalidar",
    ajuda: "Regras aprovadas cujo código citado mudou desde a aprovação.",
    vazio: "Quando o código citado por uma regra mudar, ela aparece aqui para você conferir.",
  },
  {
    id: "ambiente",
    titulo: "Ambiente a preparar",
    ajuda: "O preparo liga o agente ao projeto, para ele registrar as afirmações no documento da análise.",
    vazio: "Quando o agente ainda não estiver ligado a este projeto, o preparo aparece aqui.",
  },
  {
    id: "paradas",
    titulo: "Análises paradas",
    ajuda: "Análises criadas ou em andamento sem o agente trabalhando nelas.",
    vazio: "Quando uma análise ficar sem o agente trabalhando antes de concluir, ela aparece aqui para você retomar.",
  },
];

/** Os grupos à vista, na ordem. */
export const gruposVisiveis = (): DefinicaoGrupo[] =>
  GRUPOS_ATIVIDADES.filter((g) => g.id !== "demandas" || MOSTRAR_DEMANDAS_NOVAS);

/** O grupo de cada atividade do resumo; `null` para a que não entra em grupo. */
const GRUPO_DO_TIPO: Record<TipoAtividade, GrupoAtividade | null> = {
  responder: "responder",
  revisar: "revisar",
  "demanda-sem-analise": "demandas",
  revalidar: "revalidar",
  retomar: "paradas",
  // Projeto sem análise: a tela mostra o convite à parte, antes dos grupos.
  "primeira-analise": null,
};

export const grupoDaAtividade = (a: Atividade): GrupoAtividade | null => GRUPO_DO_TIPO[a.tipo];

/** `undefined`: ainda não conferido; `null`: a conferência falhou. */
export type AmbienteConferido = AmbienteClaude | null | undefined;

/** O ambiente pede ação só quando foi conferido e não está pronto. */
export const ambienteAPreparar = (a: AmbienteConferido): boolean => a?.pronto === false;

/** A atividade está à vista (o grupo das demandas novas fica oculto até a #322). */
export const atividadeVisivel = (a: Atividade): boolean =>
  MOSTRAR_DEMANDAS_NOVAS || a.tipo !== "demanda-sem-analise";

/**
 * O número do selo de Atividades: os itens à vista que pedem ação, mais o ambiente
 * por preparar. O `pedemAcao` do resumo conta também as demandas novas, ocultas até
 * a #322, e não conta o ambiente.
 */
export function seloAtividades(resumo: ResumoProjeto, ambiente: AmbienteConferido): number {
  return resumo.atividades.filter(atividadeVisivel).length + (ambienteAPreparar(ambiente) ? 1 : 0);
}
