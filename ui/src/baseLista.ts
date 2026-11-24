import type { ApiConhecimento, AreaVista, Busca, Encontrado, StatusRegra } from "./conhecimento";

/** As três abas da Base (#328, §9.4 da revisão de UX). */
export type AbaBase = "regras" | "glossario" | "documentos";

export const ROTULO_ABA: Record<AbaBase, string> = {
  regras: "Regras",
  glossario: "Glossário",
  documentos: "Documentos",
};

/** Tudo o que a busca achou, separado nas três abas. */
export interface ResultadoBase {
  regras: Encontrado[];
  termos: Encontrado[];
  /** Análises aprovadas, atas, histórias, cenários, demandas e externos. */
  documentos: Encontrado[];
  /** Nenhum documento tinha todas as palavras; estes têm ao menos uma. */
  qualquerPalavra: boolean;
}

/** Páginas lidas no máximo por busca (20 por página): trava contra um índice sem fim. */
const MAX_PAGINAS = 100;

/**
 * Lê todas as páginas da busca (ou da lista, sem texto): a Base agrupa e filtra no
 * app, então precisa do conjunto inteiro, não de uma página de 20.
 */
export async function carregarTudo(
  api: Pick<ApiConhecimento, "buscar">,
  texto: string,
  projeto: string,
): Promise<ResultadoBase> {
  const todos: Encontrado[] = [];
  let qualquerPalavra = false;
  let deslocamento = 0;
  for (let i = 0; i < MAX_PAGINAS; i++) {
    const pagina: Busca = await api.buscar(texto, {
      projeto: projeto || undefined,
      tipos: [],
      deslocamento,
    });
    if (i === 0) qualquerPalavra = pagina.qualquerPalavra;
    todos.push(...pagina.resultados);
    deslocamento += pagina.resultados.length;
    if (pagina.resultados.length === 0 || deslocamento >= pagina.total) break;
  }
  return {
    regras: todos.filter((r) => r.tipo === "regra"),
    termos: todos.filter((r) => r.tipo === "termo"),
    documentos: todos.filter((r) => r.tipo !== "regra" && r.tipo !== "termo"),
    qualquerPalavra,
  };
}

/** `multas-por-atraso` vira "Multas por atraso"; sem área, "Sem área". */
export function nomeLegivel(id: string | null): string {
  if (!id) return "Sem área";
  const texto = id.replace(/[-_]+/g, " ").trim();
  return texto ? texto.charAt(0).toUpperCase() + texto.slice(1) : "Sem área";
}

/** Nome da área: o declarado no projeto.md ou, nas não declaradas, o id legível. */
export function nomeDaArea(id: string | null, areas: AreaVista[] = []): string {
  const a = id ? areas.find((x) => x.id === id) : undefined;
  return a?.nome?.trim() ? a.nome : nomeLegivel(id);
}

/**
 * Título curto do documento: o trecho em negrito do começo da afirmação
 * (`**Carência.** Enquanto…` vira "Carência") ou, sem ele, o texto sem a marcação
 * de Markdown e cortado numa frase curta.
 */
export function tituloLimpo(titulo: string): string {
  const negrito = /^\s*\*\*(.+?)\*\*/.exec(titulo);
  const base = negrito ? negrito[1] : titulo;
  const limpo = base
    .replace(/\*\*|__|`|~~/g, "")
    .replace(/\[([^\]]+)\]\([^)]*\)/g, "$1")
    .replace(/[\s.:;,]+$/, "")
    .replace(/\s+/g, " ")
    .trim();
  return limpo || titulo.trim();
}

/** O mesmo da busca no índice: minúsculas e sem acento. */
export const semAcento = (s: string) => s.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();

export interface Grupo {
  chave: string;
  titulo: string;
  itens: Encontrado[];
}

const PESO_STATUS: Record<string, number> = { "a-revalidar": 0, confirmada: 1, obsoleta: 2 };

/**
 * Agrupa por área (nome legível, em ordem alfabética; "Sem área" por último). Sem
 * texto na busca, dentro do grupo as "a revalidar" vêm primeiro e o resto por
 * identificador; com texto, vale a ordem de relevância da busca.
 */
export function agruparPorArea(
  itens: Encontrado[],
  areas: AreaVista[],
  porRelevancia: boolean,
): Grupo[] {
  const mapa = new Map<string, Encontrado[]>();
  for (const i of itens) {
    const chave = i.area ?? "";
    mapa.set(chave, [...(mapa.get(chave) ?? []), i]);
  }
  const grupos = [...mapa.entries()].map(([chave, lista]) => ({
    chave,
    titulo: nomeDaArea(chave || null, areas),
    itens: porRelevancia
      ? lista
      : [...lista].sort(
          (a, b) =>
            (PESO_STATUS[a.status ?? ""] ?? 1) - (PESO_STATUS[b.status ?? ""] ?? 1) ||
            (a.identificador ?? a.titulo).localeCompare(b.identificador ?? b.titulo, "pt-BR", {
              numeric: true,
            }),
        ),
  }));
  return grupos.sort((a, b) => {
    if (!a.chave !== !b.chave) return a.chave ? -1 : 1;
    return a.titulo.localeCompare(b.titulo, "pt-BR");
  });
}

/** Agrupa os documentos pelo tipo ("Ata de reunião", "Histórias e critérios de aceite"…). */
export function agruparPorTipo(itens: Encontrado[], rotulo: (t: string | null) => string): Grupo[] {
  const mapa = new Map<string, Encontrado[]>();
  for (const i of itens) {
    const titulo = i.externo ? "Documento externo" : rotulo(i.tipo);
    mapa.set(titulo, [...(mapa.get(titulo) ?? []), i]);
  }
  return [...mapa.entries()]
    .map(([titulo, lista]) => ({ chave: titulo, titulo, itens: lista }))
    .sort((a, b) => a.titulo.localeCompare(b.titulo, "pt-BR"));
}

export const SITUACOES: { valor: StatusRegra; rotulo: string }[] = [
  { valor: "confirmada", rotulo: "Confirmadas" },
  { valor: "a-revalidar", rotulo: "A revalidar" },
  { valor: "obsoleta", rotulo: "Obsoletas" },
];
