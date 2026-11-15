import type { Cliente, EstadoAnalise, EstadoSessao, Fluxo } from "./analise";

/**
 * Espelho dos comandos das demandas (`ui/src-tauri/src/demandas.rs`, campos em
 * camelCase): a lista do projeto e o painel da demanda (#138).
 */
export type StatusDemanda = "aberta" | "encerrada";
export type StatusAta = "rascunho" | "aprovada";

export interface DemandaVista {
  /** `D-0001`. */
  id: string;
  titulo: string;
  status: StatusDemanda;
  /** Chave no rastreador (`PROJ-123`). */
  chave: string | null;
  origem: string | null;
  /** `AAAA-MM-DD`. */
  criado: string;
  encerrado: string | null;
  /** `demanda.md`, relativo à raiz da base. */
  documento: string;
}

export interface NovaDemanda {
  titulo: string;
  descricao?: string;
  chave?: string;
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

export const ROTULO_STATUS_ATA: Record<StatusAta, string> = {
  rascunho: "rascunho",
  aprovada: "aprovada",
};
