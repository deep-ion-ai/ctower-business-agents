import { invoke } from "@tauri-apps/api/core";
import type { Fluxo, SessaoVista } from "./analise";

/**
 * O resumo do projeto (#318): as contagens e a lista de atividades numa chamada só,
 * para a Visão geral, as Atividades e o selo do menu. Espelha `ResumoProjeto`
 * (`ui/src-tauri/src/resumo.rs`).
 */
export interface ResumoProjeto {
  projeto: string;
  nome: string;
  analises: {
    total: number;
    porEstado: Record<"criada" | "emAndamento" | "concluida" | "aprovada", number>;
    /** Todos os fluxos, com zero quando não há análise. */
    porFluxo: { fluxo: Fluxo; total: number }[];
  };
  /** Análises concluídas, à espera da revisão. */
  aguardandoRevisao: {
    id: string;
    titulo: string;
    fluxo: Fluxo;
    atualizadaMs: number;
    afirmacoesDecididas: number;
    afirmacoesTotal: number;
    /** Itens que impedem a aprovação. */
    pendencias: number;
  }[];
  perguntasEmAberto: {
    total: number;
    porAnalise: { analise: string; titulo: string; abertas: number }[];
  };
  demandas: { abertas: number; encerradas: number; semAnalise: number };
  base: { regras: number; aRevalidar: number; termos: number };
  sessoes: {
    total: number;
    ativas: number;
    aguardandoAnalista: number;
    concluidas: number;
    encerradas: number;
    /** A sessão mais recente de cada análise. */
    porAnalise: ({ analise: string } & SessaoVista)[];
  };
  /** Na ordem de prioridade; o grupo "Ambiente a preparar" a tela junta. */
  atividades: Atividade[];
  /** O número do selo do menu: quantas atividades esperam o analista. */
  pedemAcao: number;
  geradoMs: number;
}

export type TipoAtividade =
  | "responder"
  | "revisar"
  | "demanda-sem-analise"
  | "revalidar"
  | "retomar"
  | "primeira-analise";

export interface Atividade {
  tipo: TipoAtividade;
  ordem: number;
  analise: string | null;
  demanda: string | null;
  titulo: string | null;
  /** `revisar`: afirmações decididas e total. */
  decididas: number | null;
  total: number | null;
  /** `revalidar`: regras a revalidar. */
  regras: number | null;
  atualizadaMs: number | null;
}

export interface ApiResumo {
  resumoProjeto(projeto: string): Promise<ResumoProjeto>;
}

export const apiResumoTauri: ApiResumo = {
  resumoProjeto: (projeto) => invoke("resumo_projeto_app", { projeto }),
};
