import type { AnaliseInicio, Inicio, ProjetoInicio } from "./conhecimento";
import type { IconName } from "./shell/icons";

/**
 * Seções do menu lateral (#319, §5.2 da revisão de UX): trocam no lugar, sem abrir
 * aba, e mostram o projeto escolhido na barra de título. "configurar-projeto" não
 * tem item no menu: abre pelo cabeçalho da Visão geral.
 */
export type Secao =
  | "visao-geral"
  | "atividades"
  | "analises"
  | "demandas"
  | "base"
  | "funcionalidades"
  | "codigo"
  | "configuracoes"
  | "configurar-projeto";

export interface ItemDoMenu {
  id: Secao;
  rotulo: string;
  icone: IconName;
}

export const GRUPOS_DE_SECOES: { titulo: string; itens: ItemDoMenu[] }[] = [
  {
    titulo: "Projeto",
    itens: [
      { id: "visao-geral", rotulo: "Visão geral", icone: "overview" },
      { id: "atividades", rotulo: "Atividades", icone: "inbox" },
      { id: "analises", rotulo: "Análises", icone: "analysis" },
      { id: "demandas", rotulo: "Demandas", icone: "demand" },
    ],
  },
  {
    titulo: "Conhecimento",
    itens: [
      { id: "base", rotulo: "Base", icone: "book" },
      { id: "funcionalidades", rotulo: "Funcionalidades", icone: "features" },
      { id: "codigo", rotulo: "Código", icone: "code" },
    ],
  },
];

export const ITEM_CONFIGURACOES: ItemDoMenu = {
  id: "configuracoes",
  rotulo: "Configurações",
  icone: "settings",
};

export const ROTULO_SECAO: Record<Secao, string> = {
  ...(Object.fromEntries(
    [...GRUPOS_DE_SECOES.flatMap((g) => g.itens), ITEM_CONFIGURACOES].map((i) => [i.id, i.rotulo]),
  ) as Record<Exclude<Secao, "configurar-projeto">, string>),
  "configurar-projeto": "Configurar projeto",
};

/** O item do menu marcado em cada seção: "Configurar projeto" abre da Visão geral. */
export const ITEM_DA_SECAO = (s: Secao): Secao => (s === "configurar-projeto" ? "visao-geral" : s);

/** Contagem ao lado do rótulo; `atencao` pinta o selo em âmbar (algo espera o analista). */
export interface Contagem {
  n: number;
  atencao: boolean;
}

/** Análise que espera o analista: o agente perguntou, ou terminou e falta revisar. */
const esperaOAnalista = (a: AnaliseInicio) =>
  a.sessao?.estado === "aguardando-analista" || a.estado === "concluida";

/** As análises do projeto vistas no início, sem repetir (em andamento e recentes). */
export function analisesDoProjeto(inicio: Inicio, projeto: string): AnaliseInicio[] {
  const vistas = new Map<string, AnaliseInicio>();
  for (const a of [...inicio.emAndamento, ...inicio.recentes]) {
    if (a.projeto === projeto) vistas.set(a.id, a);
  }
  return [...vistas.values()];
}

/**
 * O que espera o analista no projeto, com os dados do início (`inicio_app`): cada
 * análise a responder ou a revisar e, havendo regras a revalidar, mais um item.
 * Alimenta o selo de Atividades e as pendências do seletor de projeto. Quando a
 * #318 trouxer o resumo do projeto, a conta passa a vir dele.
 */
export function pendenciasNoInicio(inicio: Inicio, p: ProjetoInicio): number {
  const analises = analisesDoProjeto(inicio, p.id).filter(esperaOAnalista).length;
  return analises + (p.aRevalidar > 0 ? 1 : 0);
}

/** Contagens do menu para o projeto escolhido; seção sem dado fica sem selo. */
export function contagensDoMenu(
  inicio: Inicio | null,
  projeto: string,
): Partial<Record<Secao, Contagem>> {
  const p = inicio?.projetos.find((x) => x.id === projeto);
  if (!inicio || !p) return {};
  const pendencias = pendenciasNoInicio(inicio, p);
  return {
    atividades: { n: pendencias, atencao: pendencias > 0 },
    analises: { n: p.analises, atencao: false },
  };
}

const CHAVE_ULTIMO = "cba.projeto.ultimo";

/** O último projeto usado, lembrado entre aberturas do app. */
export function ultimoProjeto(): string {
  try {
    return localStorage.getItem(CHAVE_ULTIMO) ?? "";
  } catch {
    return "";
  }
}

export function lembrarProjeto(id: string) {
  try {
    localStorage.setItem(CHAVE_ULTIMO, id);
  } catch {
    // Sem armazenamento: vale só nesta abertura.
  }
}
