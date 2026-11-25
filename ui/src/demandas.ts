import type { Cliente, EstadoAnalise, EstadoSessao, Fluxo } from "./analise";

/**
 * Espelho dos comandos das demandas (`ui/src-tauri/src/demandas.rs`, campos em
 * camelCase): a lista do projeto e o painel da demanda (#138).
 */
export type StatusDemanda = "aberta" | "encerrada";
/** Estado da issue no rastreador, normalizado (#292). */
export type EstadoRastreador = "aberta" | "em-andamento" | "encerrada";

/** Último estado conferido no rastreador. */
export interface RastreadorVista {
  estado: EstadoRastreador;
  /** Última conferência (ISO 8601, UTC). */
  conferido: string;
  /** Data de fechamento no rastreador (`AAAA-MM-DD`). */
  encerrado: string | null;
}
export type StatusAta = "rascunho" | "aprovada";

/**
 * Tipo do pedido do negócio (#358), no campo `tipo_demanda` do frontmatter. A
 * Demanda Negocial grava; as demandas criadas por outros caminhos podem não ter.
 */
export type TipoDemanda = "bug" | "funcionalidade" | "melhoria";

/** Os tipos na ordem da Demanda Negocial, com o nome mostrado no filtro e nos selos. */
export const TIPOS_DE_DEMANDA: TipoDemanda[] = ["bug", "funcionalidade", "melhoria"];
export const ROTULO_TIPO_DEMANDA: Record<TipoDemanda, string> = {
  bug: "Corrigir bug",
  funcionalidade: "Nova funcionalidade",
  melhoria: "Melhoria",
};

export interface DemandaVista {
  /** `D-0001`. */
  id: string;
  titulo: string;
  /** Status efetivo: o conferido no rastreador, quando há; senão, o local. */
  status: StatusDemanda;
  /** Status gravado pelo analista (encerrar e reabrir). */
  statusLocal: StatusDemanda;
  /**
   * Tipo do pedido: o do frontmatter ou, na demanda antiga, o da linha `**Tipo:**`
   * da descrição; `null` quando a demanda não tem tipo.
   */
  tipo: TipoDemanda | null;
  /** Estado conferido no rastreador; `null` na demanda sem chave ou ainda não conferida. */
  rastreador: RastreadorVista | null;
  /** Rastreador inacessível ou issue não encontrada na última conferência. */
  avisoRastreador: string | null;
  /** Chave no rastreador (`PROJ-123`). */
  chave: string | null;
  origem: string | null;
  /** `AAAA-MM-DD`. */
  criado: string;
  /** Data de encerramento que acompanha o status efetivo. */
  encerrado: string | null;
  /** `demanda.md`, relativo à raiz da base. */
  documento: string;
}

export interface NovaDemanda {
  titulo: string;
  descricao?: string;
  chave?: string;
  /** Gravado no frontmatter; a Demanda Negocial informa. */
  tipo?: TipoDemanda;
}

/** Transcript guardado sem a ata aprovada: até quando fica no diretório de dados. */
export interface TranscriptGuardado {
  /** `AAAA-MM-DD`. */
  apagaEm: string;
  /** Faltam 7 dias ou menos. */
  venceEmBreve: boolean;
}

export interface ReuniaoVista {
  /** `R-0001`. */
  id: string;
  data: string;
  status: StatusAta;
  objetivo: string | null;
  participantes: number;
  itens: number;
  /** A ata, relativa à raiz da base; `null` se ainda não foi montada. */
  documento: string | null;
  /** Transcript guardado no diretório de dados, com a ata ainda não aprovada. */
  transcriptGuardado: TranscriptGuardado | null;
}

export interface AnaliseDaDemanda {
  id: string;
  titulo: string;
  fluxo: Fluxo;
  estado: EstadoAnalise;
  criadaMs: number;
}

export interface SessaoDaDemanda {
  id: string;
  /** Análise a que a sessão está ligada. */
  analise: string;
  estado: EstadoSessao;
  cliente: Cliente;
  inicioMs: number;
}

/** Item da linha do tempo: a data, o instante (a ata só tem a data) e o evento. */
export type ItemLinhaDoTempo = { data: string; instanteMs: number | null } & (
  | ({ tipo: "reuniao" } & ReuniaoVista)
  | ({ tipo: "analise" } & AnaliseDaDemanda)
  | ({ tipo: "sessao" } & SessaoDaDemanda)
);

/** Pendência, dúvida ou próximo passo de uma ata aprovada. */
export interface ItemDaAta {
  /** `P-1`, `DV-2`, `PP-1`. */
  id: string;
  /** Reunião de origem. */
  reuniao: string;
  data: string;
  texto: string;
  responsavel: string | null;
  prazo: string | null;
  /** Itens da mesma ata a que este se refere (`N-3`). */
  referencias: string[];
}

export interface PainelDemanda {
  demanda: DemandaVista;
  /** A mais antiga primeiro. */
  linhaDoTempo: ItemLinhaDoTempo[];
  pendencias: ItemDaAta[];
  duvidas: ItemDaAta[];
  proximosPassos: ItemDaAta[];
  problemas: { caminho: string; erro: string }[];
}

export const ROTULO_STATUS_DEMANDA: Record<StatusDemanda, string> = {
  aberta: "aberta",
  encerrada: "encerrada",
};

export const ROTULO_ESTADO_RASTREADOR: Record<EstadoRastreador, string> = {
  aberta: "aberta",
  "em-andamento": "em andamento",
  encerrada: "encerrada",
};

/** Data e hora locais da conferência (`23/11/2026 14:05`); o texto como veio, se ilegível. */
export function quandoConferido(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleString("pt-BR", { dateStyle: "short", timeStyle: "short" });
}

export const ROTULO_STATUS_ATA: Record<StatusAta, string> = {
  rascunho: "rascunho",
  aprovada: "aprovada",
};
