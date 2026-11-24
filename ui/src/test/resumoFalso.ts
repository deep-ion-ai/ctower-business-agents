import type { ApiResumo, ResumoProjeto } from "../resumoProjeto";
import { PROJETO } from "./conhecimentoFalso";

/**
 * Resumo do projeto coerente com `inicioFicticio`: duas análises (a A-0001 aprovada e
 * a A-0002 com o agente à espera do analista), uma regra a revalidar e uma demanda
 * aberta sem análise.
 */
export function resumoFicticio(parcial: Partial<ResumoProjeto> = {}): ResumoProjeto {
  return {
    projeto: PROJETO,
    nome: "Faturamento Exemplo",
    analises: {
      total: 2,
      porEstado: { criada: 0, emAndamento: 1, concluida: 0, aprovada: 1 },
      porFluxo: [
        { fluxo: "perguntar", total: 1 },
        { fluxo: "explicar", total: 0 },
        { fluxo: "impacto", total: 1 },
        { fluxo: "reuniao", total: 0 },
        { fluxo: "revisar", total: 0 },
        { fluxo: "historias", total: 0 },
        { fluxo: "cenarios", total: 0 },
        { fluxo: "manual", total: 0 },
      ],
    },
    aguardandoRevisao: [],
    perguntasEmAberto: { total: 0, porAnalise: [] },
    demandas: { abertas: 1, encerradas: 1, semAnalise: 1 },
    base: { regras: 1, aRevalidar: 1, termos: 1 },
    sessoes: {
      total: 1,
      ativas: 0,
      aguardandoAnalista: 1,
      concluidas: 0,
      encerradas: 0,
      porAnalise: [
        { analise: "A-0002", id: "s1", estado: "aguardando-analista", cliente: "claude-code" },
      ],
    },
    atividades: [
      {
        tipo: "responder",
        ordem: 1,
        analise: "A-0002",
        demanda: null,
        titulo: "Parcelamento para pessoa jurídica",
        decididas: null,
        total: null,
        regras: null,
        atualizadaMs: 2,
      },
      {
        tipo: "revalidar",
        ordem: 4,
        analise: null,
        demanda: null,
        titulo: null,
        decididas: null,
        total: null,
        regras: 1,
        atualizadaMs: null,
      },
    ],
    pedemAcao: 2,
    geradoMs: 1_700_000_000_000,
    ...parcial,
  };
}

/** Um projeto sem análise, demanda, regra nem sessão. */
export function resumoVazio(): ResumoProjeto {
  const r = resumoFicticio();
  return {
    ...r,
    analises: {
      total: 0,
      porEstado: { criada: 0, emAndamento: 0, concluida: 0, aprovada: 0 },
      porFluxo: r.analises.porFluxo.map((f) => ({ ...f, total: 0 })),
    },
    demandas: { abertas: 0, encerradas: 0, semAnalise: 0 },
    base: { regras: 0, aRevalidar: 0, termos: 0 },
    sessoes: { total: 0, ativas: 0, aguardandoAnalista: 0, concluidas: 0, encerradas: 0, porAnalise: [] },
    atividades: [],
    pedemAcao: 0,
  };
}

export function apiResumoFalsa(resumo: ResumoProjeto = resumoFicticio()) {
  const api = { resumoProjeto: vi.fn((_: string) => Promise.resolve(resumo)) } satisfies ApiResumo;
  return api;
}
