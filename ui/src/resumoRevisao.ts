import type { Afirmacao } from "./analise";

/** Filtros da faixa de revisão (#184). */
export type FiltroRevisao = "todos" | "pendentes" | "confirmadas" | "incorretas" | "aprofundar";

export const FILTROS: { id: FiltroRevisao; rotulo: string }[] = [
  { id: "todos", rotulo: "Todos" },
  { id: "pendentes", rotulo: "Pendentes" },
  { id: "confirmadas", rotulo: "Confirmadas" },
  { id: "incorretas", rotulo: "Incorretas" },
  { id: "aprofundar", rotulo: "Aprofundar" },
];

export interface ResumoRevisao {
  total: number;
  confirmadas: number;
  incorretas: number;
  aprofundar: number;
  /** Sem decisão do analista: registradas e a revalidar. */
  pendentes: number;
  /** Confirmadas, incorretas e a aprofundar. */
  decididas: number;
}

/** Pendente é o que ainda espera o analista: sem decisão ou com o código alterado. */
export function pendente(a: Afirmacao): boolean {
  return a.estado === "registrada" || a.estado === "a-revalidar";
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
    decididas: confirmadas + incorretas + aprofundar,
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
