import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { AnaliseCriada } from "./analise";

/**
 * Espelho dos comandos das issues do projeto (`ui/src-tauri/src/issues_projeto.rs`,
 * campos em camelCase): a lista da área Demandas (#322, #323).
 */
export type OrigemIssue = "github" | "jira";
export type FiltroEstadoIssues = "abertas" | "encerradas" | "todas";
export type SituacaoNaBase = "sem-analise" | "em-andamento" | "em-revisao" | "aprovada";

export interface FiltroIssues {
  /** Número (`41`, `#41`, `PROJ-41`) ou trecho do título. */
  busca?: string;
  estado?: FiltroEstadoIssues;
  rotulo?: string;
  situacao?: SituacaoNaBase;
}

export interface IssueDoProjeto {
  origem: OrigemIssue;
  chave: string;
  titulo: string;
  descricao: string;
  url: string;
  /** ISO 8601. */
  criadaEm: string;
  /** `aberta` ou `fechada`, como o rastreador disse na última leitura. */
  estado: string;
  rotulos: string[];
  situacao: SituacaoNaBase;
  /** Análise que define a situação (a mais recente): `A-0003`. */
  analise: string | null;
  /** Demanda da base que representa a issue: `D-0001`. */
  demanda: string | null;
  /** Funcionalidades a que essa demanda está ligada. */
  funcionalidades: string[];
}

export interface ListaIssues {
  issues: IssueDoProjeto[];
  /** Issues da última leitura, antes do filtro. */
  total: number;
  rotulos: string[];
  /** Última leitura que deu certo (ISO 8601, UTC). */
  lidaEm: string;
  /** Há releitura em segundo plano; o evento avisa quando terminar. */
  atualizando: boolean;
  /** A última tentativa de leitura falhou (sem rede, sem token): a lista é a anterior. */
  aviso: string | null;
  excedeuTeto: boolean;
  /** Data de corte da leitura (`AAAA-MM-DD`). */
  desde: string;
}

/** O que "Analisar impacto" fez. */
export interface ImpactoIniciado {
  /** Demanda da base da issue (`D-0001`). */
  demanda: string;
  /** A demanda nasceu agora; senão, foi reaproveitada. */
  demandaCriada: boolean;
  criada: AnaliseCriada;
}

export interface ApiIssues {
  /** Com cache, não espera o rastreador; `forcar` ("Atualizar") relê na hora. */
  listar(projeto: string, filtro?: FiltroIssues, forcar?: boolean): Promise<ListaIssues>;
  /** Cria (ou reaproveita) a demanda da issue e cria a análise de impacto ligada a ela. */
  analisarImpacto(projeto: string, origem: OrigemIssue, chave: string): Promise<ImpactoIniciado>;
  /** Avisa quando a releitura em segundo plano termina (com o projeto); devolve a função que para. */
  aoAtualizar(aviso: (projeto: string) => void): Promise<() => void>;
}

export const apiIssuesTauri: ApiIssues = {
  listar: (projeto, filtro = {}, forcar = false) =>
    invoke("issues_do_projeto_app", { projeto, filtro, forcar }),
  analisarImpacto: (projeto, origem, chave) =>
    invoke("analisar_impacto_da_issue_app", { projeto, origem, chave }),
  aoAtualizar: (aviso) => listen<string>("issues-do-projeto-atualizadas", (e) => aviso(e.payload)),
};

export const ROTULO_SITUACAO: Record<SituacaoNaBase, string> = {
  "sem-analise": "sem análise",
  "em-andamento": "em andamento",
  "em-revisao": "em revisão",
  aprovada: "aprovada",
};

/** "há 5 min", "ontem", "há 3 dias"; vazio se a data é ilegível. */
export function idadeDe(iso: string, agora: number = Date.now()): string {
  const t = new Date(iso.length === 10 ? `${iso}T00:00:00` : iso).getTime();
  if (Number.isNaN(t)) return "";
  const min = Math.max(0, Math.floor((agora - t) / 60_000));
  if (min < 1) return "agora";
  if (min < 60) return `há ${min} min`;
  const h = Math.floor(min / 60);
  if (h < 24) return h === 1 ? "há 1 hora" : `há ${h} horas`;
  const d = Math.floor(h / 24);
  if (d === 1) return "ontem";
  if (d < 30) return `há ${d} dias`;
  const m = Math.floor(d / 30);
  if (m < 12) return m === 1 ? "há 1 mês" : `há ${m} meses`;
  const a = Math.floor(d / 365);
  return a <= 1 ? "há 1 ano" : `há ${a} anos`;
}

/** "conferido há 5 min": o frescor da última leitura. */
export function frescor(lidaEm: string, agora: number = Date.now()): string {
  const idade = idadeDe(lidaEm, agora);
  if (!idade) return "";
  return idade === "agora" ? "conferido agora" : `conferido ${idade}`;
}
