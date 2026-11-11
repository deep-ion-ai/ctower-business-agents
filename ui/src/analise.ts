import { Channel, invoke } from "@tauri-apps/api/core";
import type { SistemaResumo } from "./terminal";

/**
 * Espelho do estado da análise (`cba_core::analise::Analise`), no formato do arquivo
 * `A-NNNN.estado.json` (campos em snake_case), e dos comandos da tela da análise
 * (`ui/src-tauri/src/analise.rs` e `vigia.rs`, campos em camelCase).
 */
export type Fluxo = "perguntar" | "explicar" | "impacto";
export type EstadoAnalise = "criada" | "em-andamento" | "concluida" | "aprovada";
export type EstadoSessao = "ativa" | "aguardando-analista" | "concluida" | "encerrada";
export type Cliente = "claude-code" | "claude-desktop" | "desconhecido";
export type Confianca = "confirmado" | "inferido" | "a_confirmar";

export interface Evidencia {
  arquivo: string;
  /** Intervalo `inicio-fim`, como `71-72`. */
  linhas: string;
  trecho: string;
  hash: string;
  commit?: string | null;
  estado: "valida" | "a-revalidar";
}

export type EstadoAfirmacao =
  | { estado: "registrada" }
  | { estado: "confirmada" }
  | { estado: "incorreta"; comentario: string }
  | { estado: "aprofundar" }
  | { estado: "a-revalidar" };

export type Afirmacao = {
  numero: number;
  secao: string;
  texto: string;
  confianca: Confianca;
  evidencias: Evidencia[];
  sessao?: string | null;
  registrada_ms: number;
} & EstadoAfirmacao;

export type Pergunta = {
  numero: number;
  texto: string;
  afirmacao?: number | null;
  registrada_ms: number;
} & ({ estado: "aberta" } | { estado: "respondida"; resposta: string } | { estado: "descartada" });

export interface Sugestao {
  numero: number;
  id?: string | null;
  titulo: string;
  arquivo: string;
  trecho_atual: string;
  trecho_proposto: string;
  motivo: string;
  impacto: string;
  exemplo?: string | null;
  evidencia?: Evidencia | null;
  estado: "proposta" | "aceita" | "recusada";
  registrada_ms: number;
}

export interface TermoProposto {
  numero: number;
  termo: string;
  definicao: string;
  estado: "proposto" | "aceito" | "recusado";
}

export interface Sessao {
  id: string;
  cliente: Cliente;
  estado: EstadoSessao;
  inicio_ms: number;
  fim_ms?: number | null;
}

export interface Analise {
  id: string;
  sistema: string;
  fluxo: Fluxo;
  titulo: string;
  entrada: string;
  estado: EstadoAnalise;
  documento: string;
  versao: number;
  criada_ms: number;
  atualizada_ms: number;
  sessoes: Sessao[];
  afirmacoes: Afirmacao[];
  perguntas: Pergunta[];
  sugestoes: Sugestao[];
  termos: TermoProposto[];
}

export interface SessaoVista {
  id: string;
  estado: EstadoSessao;
  cliente: Cliente;
}

export interface VistaAnalise {
  analise: Analise;
  /** Caminho absoluto do documento `A-NNNN.md`. */
  documento: string;
  sessao: SessaoVista | null;
  /** Pedido para uma conversa nova continuar a análise (`retomar_analise`). */
  promptRetomada: string;
}

export interface AnaliseResumo {
  id: string;
  titulo: string;
  fluxo: Fluxo;
  estado: EstadoAnalise;
  atualizadaMs: number;
}

export interface AnaliseCriada {
  analise: AnaliseResumo;
  /** Pedido que faz o agente chamar `iniciar_analise` com o A-NNNN criado. */
  promptInicial: string;
}

export interface Trecho {
  arquivo: string;
  inicio: number;
  fim: number;
  /** Número da primeira linha de `linhas`. */
  primeira: number;
  linhas: string[];
  total: number;
}

export interface Situacao {
  base: string | null;
  sistemas: SistemaResumo[];
}

export interface AmbienteClaude {
  pronto: boolean;
  pendencias: string[];
}

export interface AlteracaoPreparo {
  arquivo: string;
  item: { tipo: "servidorMcp"; chave: string } | { tipo: "hook"; evento: string };
  acao: "adicionado" | "atualizado" | "removido" | "semMudanca";
}

export interface PlanoPreparo {
  diretorio: string;
  alteracoes: AlteracaoPreparo[];
  gravado: boolean;
}

/** Aviso da vigia do diário de eventos. */
export type AvisoAnalise =
  { tipo: "mudanca"; analise: string; mudanca: string; versao: number } | { tipo: "reinicio" };

/** Prompt pronto (#32): `prompts_prontos { sistema, analise }`. */
export interface PromptPronto {
  /** aprofundar, resumir, criterios-de-aceite, conferir-evidencias, retomar. */
  id: string;
  /** Rótulo do botão. */
  rotulo: string;
  /** Dica do botão: o que o pedido faz. */
  descricao: string;
  /** O pedido, numa linha só, com as variáveis preenchidas. */
  texto: string;
}

/** Intervalo `71-72` (ou `71`) em números. */
export function intervaloDe(linhas: string): { inicio: number; fim: number } {
  const [a, b] = linhas.split("-");
  const inicio = Number(a);
  const fim = b === undefined ? inicio : Number(b);
  return { inicio, fim };
}

/** Mensagem de um erro vindo do Rust (texto) ou do JavaScript. */
export function mensagemDe(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return String(e);
}

/** Verificação e preparo do ambiente do Claude, usados pela análise e pelo primeiro uso. */
export interface ApiAmbiente {
  verificarAmbiente(sistema: string): Promise<AmbienteClaude>;
  /** O que o preparo mudaria, sem gravar. */
  planejarPreparo(sistema: string): Promise<PlanoPreparo>;
  /** Prepara; só depois do consentimento do analista. */
  prepararAmbiente(sistema: string): Promise<PlanoPreparo>;
}

/** Operações da tela da análise. A interface usa {@link apiAnaliseTauri}; os testes, uma falsa. */
export interface ApiAnalise extends ApiAmbiente {
  /** A base e os sistemas (o comando devolve também os campos do primeiro uso). */
  situacao(): Promise<Situacao>;
  listarAnalises(sistema: string): Promise<AnaliseResumo[]>;
  criarAnalise(sistema: string, entrada: string, titulo?: string): Promise<AnaliseCriada>;
  carregarAnalise(sistema: string, analise: string): Promise<VistaAnalise>;
  trechoEvidencia(sistema: string, arquivo: string, inicio: number, fim: number): Promise<Trecho>;
  /** Passa a receber os avisos do sistema; devolve a função que para. */
  vigiar(sistema: string, aoAviso: (a: AvisoAnalise) => void): Promise<() => void>;
  /** Prompts prontos da análise; lista vazia se o comando não existir (antes da #32). */
  promptsProntos(sistema: string, analise: string): Promise<PromptPronto[]>;
}

export const apiAnaliseTauri: ApiAnalise = {
  situacao: () => invoke("situacao_app"),
  verificarAmbiente: (sistema) => invoke("verificar_ambiente_sistema", { sistema }),
  planejarPreparo: (sistema) => invoke("planejar_preparo_sistema", { sistema }),
  prepararAmbiente: (sistema) => invoke("preparar_ambiente_sistema", { sistema }),
  listarAnalises: (sistema) => invoke("listar_analises_sistema", { sistema }),
  criarAnalise: (sistema, entrada, titulo) =>
    invoke("criar_analise_pergunta", { sistema, entrada, titulo: titulo ?? null }),
  carregarAnalise: (sistema, analise) => invoke("carregar_analise_sistema", { sistema, analise }),
  trechoEvidencia: (sistema, arquivo, inicio, fim) =>
    invoke("trecho_evidencia_sistema", { sistema, arquivo, inicio, fim }),
  async vigiar(sistema, aoAviso) {
    const canal = new Channel<AvisoAnalise>();
    canal.onmessage = aoAviso;
    const id = await invoke<number>("vigiar_analises", { sistema, canal });
    return () => {
      canal.onmessage = () => undefined;
      invoke("parar_vigia", { id }).catch(() => undefined);
    };
  },
  async promptsProntos(sistema, analise) {
    try {
      const lista = await invoke<PromptPronto[]>("prompts_prontos", { sistema, analise });
      return Array.isArray(lista) ? lista : [];
    } catch {
      // O comando chega com a #32; sem ele (ou com falha), a tela fica sem os botões.
      return [];
    }
  },
};
