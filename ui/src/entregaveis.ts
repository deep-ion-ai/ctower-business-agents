/**
 * Entregáveis do projeto (#460): os tipos do catálogo (`cba_docs::entregaveis`), as
 * predefinições de Ágil e RUP e a configuração gravada no cadastro do projeto. O
 * catálogo e as predefinições vêm do app; aqui ficam só as regras da tela, as mesmas
 * do núcleo: escolher a metodologia aplica a predefinição, e habilitar, desabilitar
 * ou trocar o tipo padrão à mão passa para Personalizado. Template e destino são
 * ajustes do tipo e não mudam a metodologia.
 */

export type Metodologia = "agil" | "rup" | "orientado_agentes" | "personalizado";
export type Destino = "exportar" | "repositorio_docs" | "comentario_rastreador";
export type Grupo = "agil" | "rup" | "comuns";
export type CicloDeVida = "vivo" | "por_demanda";
export type Geracao = "app" | "agente";
export type Fonte = "analise" | "regras" | "termos" | "mapa" | "cenarios";
export type TipoCampo = "texto" | "texto_longo" | "lista" | "regras";

export interface Campo {
  nome: string;
  rotulo: string;
  tipo: TipoCampo;
  obrigatorio: boolean;
}

export interface TipoEntregavel {
  id: string;
  nome: string;
  descricao: string;
  grupo: Grupo;
  ciclo: CicloDeVida;
  geracao: Geracao;
  campos: Campo[];
  /** Template padrão: `_templates/<template>.md` ou o embutido. */
  template: string;
  fontes: Fonte[];
  /** O app já gera este tipo; os demais chegam nas próximas versões. */
  disponivel: boolean;
}

export interface Predefinicao {
  metodologia: Metodologia;
  tipos: string[];
  padrao: string;
}

export interface TipoHabilitado {
  tipo: string;
  /** Arquivo de `_templates/` no lugar do padrão; ausente = o padrão do app. */
  template?: string | null;
  destino: Destino;
}

export interface Entregaveis {
  metodologia: Metodologia;
  tipos: TipoHabilitado[];
  padrao?: string | null;
}

/** A aba "Entregáveis" como o app a devolve. */
export interface EntregaveisDoProjeto {
  catalogo: TipoEntregavel[];
  predefinicoes: Predefinicao[];
  entregaveis: Entregaveis;
  /** O projeto já tem a configuração gravada (senão, valem os tipos que o app já gerava). */
  configurada: boolean;
  /** `.md` de `_templates/` que podem substituir o template padrão. */
  templatesDaBase: string[];
}

export const METODOLOGIAS: { id: Metodologia; rotulo: string; emBreve?: boolean }[] = [
  { id: "agil", rotulo: "Ágil" },
  { id: "rup", rotulo: "RUP" },
  { id: "orientado_agentes", rotulo: "Orientado a agentes", emBreve: true },
  { id: "personalizado", rotulo: "Personalizado" },
];

export const GRUPOS: { id: Grupo; rotulo: string }[] = [
  { id: "agil", rotulo: "Ágil" },
  { id: "rup", rotulo: "RUP" },
  { id: "comuns", rotulo: "Comuns" },
];

export const DESTINOS: { id: Destino; rotulo: string }[] = [
  { id: "exportar", rotulo: "Exportar (Markdown, Word ou HTML)" },
  { id: "repositorio_docs", rotulo: "Repositório de documentação" },
  { id: "comentario_rastreador", rotulo: "Comentário no rastreador" },
];

export const CICLO: Record<CicloDeVida, string> = {
  vivo: "documento vivo",
  por_demanda: "um por demanda",
};

export const GERACAO: Record<Geracao, string> = {
  app: "montado pelo app",
  agente: "redigido pelo agente",
};

export function rotuloDaMetodologia(m: Metodologia): string {
  return METODOLOGIAS.find((x) => x.id === m)?.rotulo ?? m;
}

function novo(tipo: string): TipoHabilitado {
  return { tipo, template: null, destino: "exportar" };
}

/** Tipos na ordem do catálogo. */
function ordenados(catalogo: TipoEntregavel[], tipos: TipoHabilitado[]): TipoHabilitado[] {
  const pos = (t: TipoHabilitado) => {
    const i = catalogo.findIndex((c) => c.id === t.tipo);
    return i < 0 ? Number.MAX_SAFE_INTEGER : i;
  };
  return [...tipos].sort((a, b) => pos(a) - pos(b));
}

/** Se a configuração tem exatamente os tipos e o padrão da predefinição. */
export function confere(p: Predefinicao, e: Entregaveis): boolean {
  return (
    e.tipos.length === p.tipos.length &&
    p.tipos.every((t) => e.tipos.some((h) => h.tipo === t)) &&
    e.padrao === p.padrao
  );
}

/**
 * Escolhe a metodologia: Ágil e RUP aplicam a predefinição, mantendo o template e o
 * destino dos tipos que continuam; Personalizado fica com os tipos como estão.
 */
export function aplicarMetodologia(
  dados: Pick<EntregaveisDoProjeto, "catalogo" | "predefinicoes">,
  atual: Entregaveis,
  metodologia: Metodologia,
): Entregaveis {
  const p = dados.predefinicoes.find((x) => x.metodologia === metodologia);
  if (!p) return { ...atual, metodologia };
  const tipos = p.tipos.map((id) => atual.tipos.find((t) => t.tipo === id) ?? novo(id));
  return { metodologia, tipos: ordenados(dados.catalogo, tipos), padrao: p.padrao };
}

/**
 * Habilita ou desabilita um tipo à mão: passa para Personalizado. Sem o tipo padrão,
 * o primeiro habilitado vira o padrão.
 */
export function alternarTipo(
  catalogo: TipoEntregavel[],
  atual: Entregaveis,
  tipo: string,
  habilitar: boolean,
): Entregaveis {
  const tipos = habilitar
    ? atual.tipos.some((t) => t.tipo === tipo)
      ? atual.tipos
      : ordenados(catalogo, [...atual.tipos, novo(tipo)])
    : atual.tipos.filter((t) => t.tipo !== tipo);
  const padrao = tipos.some((t) => t.tipo === atual.padrao)
    ? atual.padrao
    : (tipos[0]?.tipo ?? null);
  return { metodologia: "personalizado", tipos, padrao };
}

/** Troca o tipo padrão à mão: passa para Personalizado se não for o da predefinição. */
export function escolherPadrao(
  predefinicoes: Predefinicao[],
  atual: Entregaveis,
  padrao: string,
): Entregaveis {
  const nova = { ...atual, padrao };
  const p = predefinicoes.find((x) => x.metodologia === atual.metodologia);
  return p && !confere(p, nova) ? { ...nova, metodologia: "personalizado" } : nova;
}

/** Muda o template ou o destino de um tipo habilitado; a metodologia fica. */
export function ajustarTipo(
  atual: Entregaveis,
  tipo: string,
  ajuste: Partial<Pick<TipoHabilitado, "template" | "destino">>,
): Entregaveis {
  return {
    ...atual,
    tipos: atual.tipos.map((t) => (t.tipo === tipo ? { ...t, ...ajuste } : t)),
  };
}

/** Texto comparável, para saber se há alteração não salva. */
export function assinatura(e: Entregaveis): string {
  return JSON.stringify({
    m: e.metodologia,
    p: e.padrao ?? null,
    t: e.tipos.map((t) => [t.tipo, t.template ?? null, t.destino]),
  });
}
