import { invoke } from "@tauri-apps/api/core";
import type { Conducao, ExecucaoWorkflow, Fluxo } from "./analise";

/**
 * Espelho de `ui/src-tauri/src/conducao.rs` (#371): se a condução pelo workflow pode
 * ser oferecida em Nova análise, com o motivo quando não pode, e a escolha gravada na
 * análise recém-criada.
 */
export interface EstimativaConducao {
  tokensMinimo: number;
  tokensMaximo: number;
  /** Razão em relação à condução simples do mesmo tipo. */
  razaoSimples: number;
  minutosMinimo: number;
  minutosMaximo: number;
}

export interface FaseConducao {
  titulo: string;
  detalhe: string | null;
  /** Um agente por frente (senão, um agente só). */
  porFrente: boolean;
  /** Faixa de tokens da fase inteira, com as frentes padrão, depois da medição. */
  tokensMinimo: number | null;
  tokensMaximo: number | null;
}

export interface ConducaoDisponivel {
  temWorkflow: boolean;
  workflow: string | null;
  disponivel: boolean;
  motivo: string | null;
  /** Os workflows não estão instalados: "Preparar de novo". */
  prepararDeNovo: boolean;
  frentes: number;
  maxAgentes: number;
  /** Faixa de tokens, depois da medição com o `cba-avaliacao`. */
  estimativa: EstimativaConducao | null;
  /** As fases do workflow efetivo, na ordem de `meta.phases`. */
  fases: FaseConducao[];
}

export interface ApiConducao {
  disponivel(projeto: string, fluxo: Fluxo): Promise<ConducaoDisponivel>;
  /** Grava a condução na análise recém-criada e devolve o pedido que abre a conversa. */
  escolher(projeto: string, analise: string, conducao: Conducao): Promise<string>;
}

export const apiConducaoTauri: ApiConducao = {
  disponivel: (projeto, fluxo) => invoke("conducao_disponivel_projeto", { projeto, fluxo }),
  escolher: (projeto, analise, conducao) =>
    invoke("escolher_conducao_analise", { projeto, analise, conducao }),
};

const CHAVE_ULTIMA = "cba.conducao.ultima";

/** A última condução escolhida para o fluxo; na primeira vez, a simples. */
export function ultimaConducao(fluxo: Fluxo): Conducao {
  try {
    const salvas = JSON.parse(localStorage.getItem(CHAVE_ULTIMA) ?? "{}") as Record<string, unknown>;
    return salvas[fluxo] === "workflow" ? "workflow" : "simples";
  } catch {
    return "simples";
  }
}

export function lembrarConducao(fluxo: Fluxo, conducao: Conducao): void {
  try {
    const salvas = JSON.parse(localStorage.getItem(CHAVE_ULTIMA) ?? "{}") as Record<string, unknown>;
    salvas[fluxo] = conducao;
    localStorage.setItem(CHAVE_ULTIMA, JSON.stringify(salvas));
  } catch {
    // Sem armazenamento, a escolha volta à simples na próxima vez.
  }
}

function milhares(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toLocaleString("pt-BR", { maximumFractionDigits: 1 })} mi`;
  return `${Math.round(n / 1000).toLocaleString("pt-BR")} mil`;
}

/** Texto da estimativa ao lado da opção Aprofundada. */
export function textoDaEstimativa(c: ConducaoDisponivel): string {
  const agentes = `até ${c.maxAgentes} agentes (${c.frentes} frentes)`;
  const e = c.estimativa;
  if (!e) return agentes;
  const razao = e.razaoSimples.toLocaleString("pt-BR", { maximumFractionDigits: 1 });
  return (
    `${agentes}; cerca de ${milhares(e.tokensMinimo)} a ${milhares(e.tokensMaximo)} tokens ` +
    `e ${e.minutosMinimo} a ${e.minutosMaximo} min, cerca de ${razao}× a simples`
  );
}

/** Agentes e tokens de uma fase, ao lado do título. */
export function textoDaFase(f: FaseConducao, frentes: number): string {
  const agentes = f.porFrente ? `${frentes} agentes, um por frente` : "1 agente";
  if (f.tokensMinimo == null || f.tokensMaximo == null) return agentes;
  const tokens =
    f.tokensMinimo === f.tokensMaximo
      ? milhares(f.tokensMaximo)
      : `${milhares(f.tokensMinimo)} a ${milhares(f.tokensMaximo)}`;
  return `${agentes}; ${tokens} tokens`;
}

/** "n de m agentes" de uma fase, ou do workflow inteiro sem as fases. */
export function agentesDaFase(iniciados: number, concluidos: number): string {
  return `${concluidos} de ${iniciados} agente${iniciados === 1 ? "" : "s"}`;
}

export const ROTULO_SITUACAO: Record<ExecucaoWorkflow["situacao"], string> = {
  rodando: "rodando",
  concluido: "concluído",
  falhou: "terminou com erro",
  interrompido: "interrompido",
};
