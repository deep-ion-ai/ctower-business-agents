import { invoke } from "@tauri-apps/api/core";
import type { Fluxo } from "./analise";

/** Cobertura da base por área e por pasta (#125): conta, sem IA. */

export interface CoberturaArea {
  id: string;
  nome: string;
  regras: number;
  aRevalidar: number;
  /** Regras sem modificação há mais de 90 dias. */
  semConfirmacaoRecente: number;
  termos: number;
  documentos: number;
  funcionalidades: number;
  funcionalidadesSemDocumento: number;
  arquivos: number;
  centralidade: number;
  idadeRecenteDias: number | null;
  idadeAntigaDias: number | null;
  lacuna: boolean;
  /** De 0 a 1: a lacuna relativa à maior. */
  calor: number;
}

export interface CoberturaPasta {
  /** Vazio é a raiz do projeto. */
  pasta: string;
  arquivos: number;
  centralidade: number;
  regras: number;
  funcionalidades: number;
  documentadas: number;
  lacuna: boolean;
  calor: number;
}

export interface SugestaoCobertura {
  tipo: "area" | "pasta";
  alvo: string;
  nome: string;
  fluxo: Fluxo;
  funcionalidade: string | null;
  entrada: string;
  motivo: string;
}

export interface Cobertura {
  areas: CoberturaArea[];
  pastas: CoberturaPasta[];
  sugestao: SugestaoCobertura | null;
  totais: { regras: number; termos: number; documentos: number; aRevalidar: number };
}

export interface ApiCobertura {
  cobertura(projeto: string): Promise<Cobertura>;
}

export const apiCoberturaTauri: ApiCobertura = {
  cobertura: (projeto) => invoke("cobertura_projeto_app", { projeto }),
};

/** "hoje", "1 dia", "37 dias". */
export function idadeEmTexto(dias: number | null): string {
  if (dias === null) return "sem documentos";
  if (dias === 0) return "hoje";
  return dias === 1 ? "1 dia" : `${dias} dias`;
}
