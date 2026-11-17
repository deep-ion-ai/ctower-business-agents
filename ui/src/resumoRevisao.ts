import type { Afirmacao } from "./analise";
import type { Pendencia } from "./revisao";

/** Filtros da faixa de revisão (#184). */
export type FiltroRevisao = "todos" | "pendentes" | "confirmadas" | "incorretas" | "aprofundar";

export const FILTROS: { id: FiltroRevisao; rotulo: string }[] = [
  { id: "todos", rotulo: "Todos" },
  { id: "pendentes", rotulo: "A decidir" },
  { id: "confirmadas", rotulo: "Confirmadas" },
  { id: "incorretas", rotulo: "Incorretas" },
  { id: "aprofundar", rotulo: "Aprofundar" },
];

export interface ResumoRevisao {
  total: number;
  confirmadas: number;
  incorretas: number;
  aprofundar: number;
  /** Afirmações que ainda impedem a aprovação: registradas, a aprofundar e a revalidar. */
  pendentes: number;
  /** Confirmadas e incorretas: as que já não impedem a aprovação. */
  decididas: number;
}

/**
 * Afirmação pendente, pela regra da aprovação (`cba_tools::revisao::pendencias`): sem
 * decisão, com o aprofundamento pedido (espera o agente ou a decisão do analista) ou
 * com o código alterado.
 */
export function pendente(a: Afirmacao): boolean {
  return a.estado === "registrada" || a.estado === "aprofundar" || a.estado === "a-revalidar";
}

/**
 * A conta única das pendências (#222), usada no cabeçalho da análise, na faixa de
 * revisão e no diálogo de aprovação: os itens que esperam a decisão do analista e
 * impedem a aprovação (afirmações, termos, histórias, critérios, tabelas, cenários),
 * como a prontidão os lista. O que é da análise como um todo (o agente ainda não
 * concluiu, nenhuma afirmação registrada) não é decisão do analista e não conta; as
 * perguntas em aberto também não, porque não impedem a aprovação (vão no documento).
 */
export function contarPendencias(pendencias: readonly Pendencia[]): number {
  return pendencias.filter((p) => p.tipo !== "analise").length;
}

export function resumir(afirmacoes: Afirmacao[]): ResumoRevisao {
  const conta = (estado: Afirmacao["estado"]) =>
    afirmacoes.filter((a) => a.estado === estado).length;
  const confirmadas = conta("confirmada");
  const incorretas = conta("incorreta");
  const aprofundar = conta("aprofundar");
  return {
    total: afirmacoes.length,
    confirmadas,
    incorretas,
    aprofundar,
    pendentes: afirmacoes.filter(pendente).length,
    decididas: confirmadas + incorretas,
  };
}

export function passaNoFiltro(a: Afirmacao, filtro: FiltroRevisao): boolean {
  switch (filtro) {
    case "todos":
      return true;
    case "pendentes":
      return pendente(a);
    case "confirmadas":
      return a.estado === "confirmada";
    case "incorretas":
      return a.estado === "incorreta";
    case "aprofundar":
      return a.estado === "aprofundar";
  }
}

/** Quantas afirmações cada filtro mostra (o número ao lado do rótulo). */
export function contagemPorFiltro(afirmacoes: Afirmacao[]): Record<FiltroRevisao, number> {
  return Object.fromEntries(
    FILTROS.map(({ id }) => [id, afirmacoes.filter((a) => passaNoFiltro(a, id)).length]),
  ) as Record<FiltroRevisao, number>;
}
