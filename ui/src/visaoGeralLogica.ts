import { atividadeVisivel } from "./atividades";
import { ROTULO_FLUXO } from "./conhecimento";
import { contar } from "./formatacao";
import type { Atividade, ResumoProjeto } from "./resumoProjeto";

/**
 * Lógica pura da Visão geral do projeto (#320, §6 da revisão de UX): os próximos
 * passos, a atividade recente e as barras saem do `ResumoProjeto` (#318), sem ler
 * mais nada do disco.
 */

/** Cor só para estado: âmbar pede ação do analista, azul é o agente ou item aberto, verde é concluído. */
export type Tom = "attention" | "running" | "done" | "neutro";

/** Para onde o passo leva. */
export type DestinoPasso =
  | { tipo: "analise"; id: string }
  | { tipo: "demandas" }
  | { tipo: "base" }
  | { tipo: "configurar" }
  | { tipo: "nova-analise" };

export interface Passo {
  chave: string;
  tom: Tom;
  titulo: string;
  descricao: string;
  /** Rótulo do botão. */
  acao: string;
  destino: DestinoPasso;
}

/** Quantos passos a Visão geral mostra; as outras atividades ficam em Atividades. */
export const PASSOS_VISIVEIS = 3;

const nomeDaAnalise = (a: Atividade) => a.analise ?? "";

/** Título da análise, quando o resumo o traz (análises a revisar e perguntas em aberto). */
function tituloDaAnalise(resumo: ResumoProjeto, id: string | null): string | null {
  if (!id) return null;
  return (
    resumo.aguardandoRevisao.find((a) => a.id === id)?.titulo ??
    resumo.perguntasEmAberto.porAnalise.find((a) => a.analise === id)?.titulo ??
    null
  );
}

function passoDaAtividade(resumo: ResumoProjeto, a: Atividade): Passo | null {
  const id = nomeDaAnalise(a);
  const titulo = a.titulo ?? tituloDaAnalise(resumo, a.analise);
  const nome = titulo ? `${id} · ${titulo}` : id;
  switch (a.tipo) {
    case "responder":
      return {
        chave: `responder:${id}`,
        tom: "attention",
        titulo: `O agente aguarda você na ${id}`,
        descricao: titulo ?? "O agente fez uma pergunta e espera a sua resposta.",
        acao: "Responder ao agente",
        destino: { tipo: "analise", id },
      };
    case "revisar": {
      const pendente =
        a.decididas !== null && a.total !== null
          ? `${a.decididas} de ${contar(a.total, "afirmação decidida", "afirmações decididas")}.`
          : "A análise foi concluída pelo agente e espera a sua decisão.";
      return {
        chave: `revisar:${id}`,
        tom: "attention",
        titulo: `Revisar e aprovar a ${id}`,
        descricao: titulo ? `${titulo}: ${pendente}` : pendente,
        acao: "Revisar a análise",
        destino: { tipo: "analise", id },
      };
    }
    case "demanda-sem-analise":
      return {
        chave: `demanda:${a.demanda ?? ""}`,
        tom: "running",
        titulo: `Demanda ${a.demanda ?? ""} sem análise`,
        descricao: a.titulo ?? "Ainda não há análise ligada a esta demanda.",
        acao: "Ver as demandas",
        destino: { tipo: "demandas" },
      };
    case "revalidar":
      return {
        chave: "revalidar",
        tom: "attention",
        titulo: "Regras a revalidar",
        descricao: `${contar(a.regras ?? 0, "regra", "regras")} cujo código citado mudou desde a aprovação.`,
        acao: "Ver as regras",
        destino: { tipo: "base" },
      };
    case "retomar":
      return {
        chave: `retomar:${id}`,
        tom: "running",
        titulo: `Retomar a ${nome}`,
        descricao: "A análise está parada, sem sessão do agente ativa.",
        acao: "Retomar a análise",
        destino: { tipo: "analise", id },
      };
    case "primeira-analise":
      return {
        chave: "primeira-analise",
        tom: "running",
        titulo: "Faça a primeira análise",
        descricao: "Pergunte algo ao sistema: o agente lê o código e a resposta vira conhecimento na base.",
        acao: "Nova análise",
        destino: { tipo: "nova-analise" },
      };
  }
}

/**
 * Os passos mais urgentes, na ordem do resumo. O ambiente a preparar (que o resumo
 * deixa de fora) entra antes das análises paradas e da primeira análise.
 */
export function proximosPassos(
  resumo: ResumoProjeto,
  ambienteAPreparar: boolean,
): { visiveis: Passo[]; total: number } {
  const passos: Passo[] = [];
  let ambiente = ambienteAPreparar;
  const pedirAmbiente = () => {
    ambiente = false;
    passos.push({
      chave: "ambiente",
      tom: "attention",
      titulo: "Preparar o ambiente",
      descricao: "O Claude ainda não está ligado ao projeto: sem isso o agente não lê a base nem o código.",
      acao: "Preparar",
      destino: { tipo: "configurar" },
    });
  };
  for (const a of resumo.atividades.filter(atividadeVisivel)) {
    if (ambiente && (a.tipo === "retomar" || a.tipo === "primeira-analise")) pedirAmbiente();
    const p = passoDaAtividade(resumo, a);
    if (p) passos.push(p);
  }
  if (ambiente) pedirAmbiente();
  return { visiveis: passos.slice(0, PASSOS_VISIVEIS), total: passos.length };
}

/** Uma linha da atividade recente. */
export interface Recente {
  chave: string;
  ms: number;
  texto: string;
}

/**
 * A atividade recente com as datas que já existem (o jornal de eventos é truncado e
 * não serve de histórico): análises concluídas e atividades com `atualizadaMs`.
 */
export function atividadeRecente(resumo: ResumoProjeto, limite = 5): Recente[] {
  const linhas = new Map<string, Recente>();
  for (const a of resumo.aguardandoRevisao) {
    linhas.set(`revisar:${a.id}`, {
      chave: `revisar:${a.id}`,
      ms: a.atualizadaMs,
      texto: `${a.id} concluída pelo agente`,
    });
  }
  for (const a of resumo.atividades) {
    if (a.atualizadaMs === null) continue;
    const id = nomeDaAnalise(a);
    const texto =
      a.tipo === "responder"
        ? `${id}: o agente aguarda você`
        : a.tipo === "retomar"
          ? `${id} parada, sem sessão ativa`
          : a.tipo === "demanda-sem-analise"
            ? `Demanda ${a.demanda ?? ""} sem análise`
            : null;
    if (texto === null) continue;
    linhas.set(`${a.tipo}:${id || a.demanda}`, {
      chave: `${a.tipo}:${id || a.demanda}`,
      ms: a.atualizadaMs,
      texto,
    });
  }
  return [...linhas.values()].sort((x, y) => y.ms - x.ms).slice(0, limite);
}

/** "10:42" hoje, "ontem", "16/11" antes disso. */
export function quando(ms: number, agora: number = Date.now()): string {
  const d = new Date(ms);
  const a = new Date(agora);
  const dia = (x: Date) => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime();
  const dias = Math.round((dia(a) - dia(d)) / 86_400_000);
  if (dias <= 0) return d.toLocaleTimeString("pt-BR", { hour: "2-digit", minute: "2-digit" });
  if (dias === 1) return "ontem";
  return d.toLocaleDateString("pt-BR", { day: "2-digit", month: "2-digit" });
}

/** Hora do resumo: "às 10:42". */
export const horaDoResumo = (ms: number) =>
  new Date(ms).toLocaleTimeString("pt-BR", { hour: "2-digit", minute: "2-digit" });

/** Um segmento da barra empilhada. */
export interface Segmento {
  chave: string;
  tom: Tom;
  rotulo: string;
  n: number;
}

/** Análises por situação: só os segmentos com análise, cada um com rótulo e número. */
export function segmentosDeAnalises(resumo: ResumoProjeto): Segmento[] {
  const e = resumo.analises.porEstado;
  const todos: Segmento[] = [
    { chave: "revisao", tom: "attention", rotulo: "Aguardando revisão", n: e.concluida },
    { chave: "andamento", tom: "running", rotulo: "Em andamento", n: e.emAndamento },
    { chave: "criada", tom: "neutro", rotulo: "Criadas", n: e.criada },
    { chave: "aprovada", tom: "done", rotulo: "Aprovadas", n: e.aprovada },
  ];
  return todos.filter((s) => s.n > 0);
}

/** Demandas abertas e encerradas. */
export function segmentosDeDemandas(resumo: ResumoProjeto): Segmento[] {
  const d = resumo.demandas;
  return (
    [
      { chave: "abertas", tom: "running", rotulo: "Abertas", n: d.abertas },
      { chave: "encerradas", tom: "neutro", rotulo: "Encerradas", n: d.encerradas },
    ] satisfies Segmento[]
  ).filter((s) => s.n > 0);
}

export interface BarraTipo {
  fluxo: string;
  rotulo: string;
  n: number;
  /** De 0 a 100, relativo ao maior. */
  largura: number;
}

/** Análises por tipo, do mais para o menos usado; tipo sem análise não aparece. */
export function barrasPorTipo(resumo: ResumoProjeto): BarraTipo[] {
  const com = resumo.analises.porFluxo.filter((f) => f.total > 0);
  const maior = Math.max(1, ...com.map((f) => f.total));
  return com
    .map((f, i) => ({ f, i }))
    .sort((a, b) => b.f.total - a.f.total || a.i - b.i)
    .map(({ f }) => ({
      fluxo: f.fluxo,
      rotulo: ROTULO_FLUXO[f.fluxo],
      n: f.total,
      largura: Math.round((f.total / maior) * 100),
    }));
}
