import { Channel, invoke } from "@tauri-apps/api/core";
import type { Passo } from "./clientes";

/** Espelho de `cba_session::terminal` (JSON dos comandos do terminal embutido). */
export interface Tamanho {
  linhas: number;
  colunas: number;
}

/** Como a conversa começa: nova, a última da pasta (`--continue`) ou uma conhecida (`--resume`). */
export type Retomada = { modo: "nova" } | { modo: "continuar" } | { modo: "sessao"; id: string };

/** O que pedir ao `claude`. Tudo é opcional; o padrão é uma conversa nova sem prompt. */
export interface PedidoClaude {
  /** Primeira mensagem da conversa, como se o analista a tivesse digitado. */
  promptInicial?: string;
  retomada?: Retomada;
  /** Opções extras do `claude`. */
  argumentos?: string[];
}

export interface Fim {
  codigo: number;
  sinal?: string;
}

export type EventoTerminal = { tipo: "saida"; texto: string } | { tipo: "encerrado"; fim: Fim };

/** Por que o terminal não abriu. */
export type ErroAbertura = { mensagem: string } & (
  | { motivo: "projetoNaoCadastrado"; projeto: string }
  | { motivo: "ambienteNaoPreparado"; pendencias: string[] }
  | { motivo: "claudeNaoEncontrado"; orientacao: Passo[] }
  | { motivo: "pedidoInvalido" }
  | { motivo: "falha" }
);

export interface UltimaSessao {
  idSessao: string | null;
  /** Segundos desde 1970-01-01 (UTC). */
  abertaEm: number;
}

export interface ProjetoResumo {
  id: string;
  nome: string;
}

/** Prompt que move a sessão para a aba de código do Claude Desktop. */
export const PROMPT_DESKTOP = "/desktop";

/** Operações do terminal embutido. A interface usa {@link apiTauri}; os testes, uma falsa. */
export interface ApiTerminal {
  /** Abre o `claude` na pasta do projeto; devolve o identificador do terminal. */
  abrir(
    projeto: string,
    pedido: PedidoClaude,
    tamanho: Tamanho,
    aoEvento: (e: EventoTerminal) => void,
  ): Promise<number>;
  escrever(id: number, dados: string): Promise<void>;
  redimensionar(id: number, tamanho: Tamanho): Promise<void>;
  /** Envia um prompt pronto à sessão viva, como se o analista o tivesse digitado. */
  enviarPrompt(id: number, texto: string): Promise<void>;
  encerrar(id: number): Promise<void>;
}

export const apiTauri: ApiTerminal = {
  abrir(projeto, pedido, tamanho, aoEvento) {
    const canal = new Channel<EventoTerminal>();
    canal.onmessage = aoEvento;
    return invoke<number>("abrir_terminal_claude", { projeto, pedido, tamanho, canal });
  },
  escrever: (id, dados) => invoke("escrever_terminal", { id, dados }),
  redimensionar: (id, tamanho) => invoke("redimensionar_terminal", { id, tamanho }),
  enviarPrompt: (id, texto) => invoke("enviar_prompt_terminal", { id, texto }),
  encerrar: (id) => invoke("encerrar_terminal", { id }),
};

export function listarProjetos(): Promise<ProjetoResumo[]> {
  return invoke<ProjetoResumo[]>("listar_projetos");
}

export function ultimaSessao(projeto: string): Promise<UltimaSessao | null> {
  return invoke<UltimaSessao | null>("ultima_sessao_terminal", { projeto });
}

/** Erro de abertura vindo do Rust, ou um genérico para qualquer outra falha. */
export function comoErroAbertura(e: unknown): ErroAbertura {
  if (typeof e === "object" && e !== null && "motivo" in e && "mensagem" in e) {
    return e as ErroAbertura;
  }
  return { motivo: "falha", mensagem: e instanceof Error ? e.message : String(e) };
}
