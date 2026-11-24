import type { Funcionalidade, MapaFuncionalidades, PontoSemTratamento } from "./funcionalidades";

/** Progresso do mapa em quatro etapas, sobre as funcionalidades em aberto (#327). */
export interface ProgressoDoMapa {
  mapeadas: number;
  aceitas: number;
  explicadas: number;
  /** `null` enquanto o mapa não informa o manual de cada funcionalidade. */
  comManual: number | null;
  /** Percentual do código (pontos de entrada) já ligado a uma funcionalidade; `null` sem completude. */
  codigoMapeado: number | null;
}

export function progressoDoMapa(mapa: MapaFuncionalidades): ProgressoDoMapa {
  const abertas = mapa.areas.flatMap((a) => a.funcionalidades);
  const sabeManual = abertas.some((f) => f.comManual !== undefined);
  const c = mapa.completude;
  return {
    mapeadas: abertas.length,
    aceitas: abertas.filter((f) => f.situacao === "aceita").length,
    explicadas: abertas.filter((f) => f.explicacao !== null).length,
    comManual: sabeManual ? abertas.filter((f) => f.comManual).length : null,
    codigoMapeado:
      c && c.total > 0 ? Math.round(((c.total - c.semTratamento.length) / c.total) * 100) : null,
  };
}

/** Lacuna da cobertura: uma pasta com pontos de entrada ainda sem funcionalidade. */
export interface LacunaDeCobertura {
  pasta: string;
  pontos: number;
}

/** Agrupa os pontos sem funcionalidade pela pasta de topo, da maior lacuna para a menor. */
export function lacunasPorPasta(pontos: PontoSemTratamento[]): LacunaDeCobertura[] {
  const por = new Map<string, number>();
  for (const p of pontos) {
    const i = p.arquivo.indexOf("/");
    const pasta = i > 0 ? p.arquivo.slice(0, i) : ".";
    por.set(pasta, (por.get(pasta) ?? 0) + 1);
  }
  return [...por.entries()]
    .map(([pasta, n]) => ({ pasta, pontos: n }))
    .sort((a, b) => b.pontos - a.pontos || a.pasta.localeCompare(b.pasta));
}

export type FiltroDoMapa = "todas" | "pendentes" | "aceitas" | "sem-ponto";

export function passaNoFiltro(f: Funcionalidade, filtro: FiltroDoMapa): boolean {
  if (filtro === "pendentes") return f.situacao === "pendente";
  if (filtro === "aceitas") return f.situacao === "aceita";
  if (filtro === "sem-ponto") return f.semPontoDeEntrada;
  return true;
}
